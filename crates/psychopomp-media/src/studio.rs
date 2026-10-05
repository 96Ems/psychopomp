//! Producing audio for one spec. [`Generator`] is the seam between the
//! reconciler and the outside world; [`Studio`] is the real implementation:
//! ElevenLabs, Fish Audio, or `say` for the raw take, ffmpeg for loudness and
//! effects, and the provider's alignment or Whisper for word timings.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use psychopomp::transcript::WordTiming;

use crate::{
    eleven, ffmpeg, fish,
    spec::{Backend, PROVIDER_ALIGN, Spec, SpeechSpec, WHISPER},
    words,
};

/// One resource to produce.
pub(crate) struct Job<'a> {
    pub id: &'a str,
    pub spec: &'a Spec,
    /// The source file of a derived resource.
    pub input: Option<PathBuf>,
    /// Request IDs of the line this one is stitched after.
    pub previous_requests: Vec<String>,
}

/// What generating a resource yielded.
pub(crate) struct Generated {
    pub duration: u64,
    pub words: Vec<WordTiming>,
    pub requests: Vec<String>,
    /// What the provider reported billing, when it does.
    pub credits: Option<u64>,
    /// Requests made to paid providers.
    pub api_calls: u32,
}

/// Produce `job`'s audio at `out`.
pub(crate) trait Generator {
    fn generate(&mut self, job: &Job<'_>, out: &Path) -> Result<Generated>;
}

/// The real generator.
pub(crate) struct Studio {
    root: PathBuf,
    agent: Option<ureq::Agent>,
}

impl Studio {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_owned(),
            agent: None,
        }
    }

    fn agent(&mut self) -> &ureq::Agent {
        self.agent.get_or_insert_with(|| {
            ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(300)))
                .http_status_as_error(false)
                .build()
                .into()
        })
    }

    fn speech(
        &mut self,
        speech: &SpeechSpec,
        job: &Job<'_>,
        work: &Work,
        out: &Path,
    ) -> Result<Generated> {
        let (raw, alignment, requests, credits) = match speech.backend {
            Backend::ElevenLabs => {
                let key = credential("ELEVENLABS_API_KEY", &self.root)?;
                let reply = eleven::speech(self.agent(), &key, speech, &job.previous_requests)?;
                let raw = work.path("raw.mp3");
                fs::write(&raw, &reply.audio)?;
                let requests = reply.request.into_iter().collect();
                (raw, reply.alignment, requests, reply.credits)
            }
            Backend::Fish => {
                let key = credential("FISH_AUDIO_API_KEY", &self.root)?;
                let audio = fish::speech(self.agent(), &key, speech)?;
                let raw = work.path("raw.wav");
                fs::write(&raw, audio)?;
                (raw, None, Vec::new(), None)
            }
            Backend::Say => {
                let text = work.path("text.txt");
                fs::write(&text, words::strip_tags(&speech.text()))?;
                let raw = work.path("raw.wav");
                run(Command::new("say").args([
                    "-v",
                    &speech.lines[0].voice,
                    "-f",
                    &text.to_string_lossy(),
                    "-o",
                    &raw.to_string_lossy(),
                    "--data-format=LEI16@48000",
                ]))?;
                (raw, None, Vec::new(), None)
            }
        };
        let finished = work.path("speech.mp3");
        ffmpeg::loudness(&raw, &finished)?;
        let duration = ffmpeg::duration(&finished)?;
        let words = match alignment {
            Some(alignment) if speech.align == PROVIDER_ALIGN => words::from_alignment(&alignment),
            _ => whisper(&finished, work)?,
        };
        install(&finished, out)?;
        let api_calls = u32::from(speech.backend != Backend::Say);
        Ok(Generated {
            duration,
            words,
            requests,
            credits,
            api_calls,
        })
    }
}

impl Generator for Studio {
    fn generate(&mut self, job: &Job<'_>, out: &Path) -> Result<Generated> {
        let work = Work::new(job.id)?;
        match job.spec {
            Spec::Speech(speech) => self.speech(speech, job, &work, out),
            Spec::Sound(sound) => {
                let key = credential("ELEVENLABS_API_KEY", &self.root)?;
                let reply = eleven::sound(self.agent(), &key, sound)?;
                let raw = work.path("raw.mp3");
                fs::write(&raw, &reply.audio)?;
                let finished = work.path("sound.wav");
                ffmpeg::finish_sound(&raw, &finished)?;
                install(&finished, out)?;
                Ok(Generated {
                    duration: ffmpeg::duration(out)?,
                    words: Vec::new(),
                    requests: reply.request.into_iter().collect(),
                    credits: reply.credits,
                    api_calls: 1,
                })
            }
            Spec::Silence { duration_nanos, .. } => {
                let finished = work.path("silence.wav");
                ffmpeg::silence(*duration_nanos, &finished)?;
                install(&finished, out)?;
                Ok(local(ffmpeg::duration(out)?))
            }
            Spec::Derive(derive) => {
                let input = job
                    .input
                    .as_deref()
                    .context("a derived resource needs its source")?;
                let extension = out.extension().unwrap_or_default().to_string_lossy();
                let finished = work.path(&format!("derived.{extension}"));
                ffmpeg::derive(input, &derive.effect, &finished)?;
                install(&finished, out)?;
                Ok(local(ffmpeg::duration(out)?))
            }
        }
    }
}

fn local(duration: u64) -> Generated {
    Generated {
        duration,
        words: Vec::new(),
        requests: Vec::new(),
        credits: None,
        api_calls: 0,
    }
}

/// Word timings for `audio` from Whisper, as `scripts/narrate.ts` runs it.
fn whisper(audio: &Path, work: &Work) -> Result<Vec<WordTiming>> {
    let model = env::var("WHISPER_MODEL").unwrap_or_else(|_| WHISPER.to_owned());
    if model != WHISPER {
        bail!(
            "WHISPER_MODEL={model} would time words differently than the key records ({WHISPER})"
        );
    }
    run(Command::new("uvx").args([
        "--from",
        "mlx-whisper",
        "mlx_whisper",
        "--model",
        &model,
        "--word-timestamps",
        "True",
        "--output-format",
        "json",
        "--output-dir",
        &work.dir.to_string_lossy(),
        "--output-name",
        "words",
        &audio.to_string_lossy(),
    ]))?;
    words::from_whisper(&fs::read(work.path("words.json"))?)
}

/// Copy a finished file into the store. Files appear complete or not at all.
fn install(finished: &Path, out: &Path) -> Result<()> {
    let partial = out.with_extension("partial");
    fs::copy(finished, &partial).with_context(|| format!("write {}", partial.display()))?;
    fs::rename(&partial, out).with_context(|| format!("install {}", out.display()))
}

/// Run a command to completion, failing with its stderr.
pub(crate) fn run(command: &mut Command) -> Result<Vec<u8>> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("run {program}"))?;
    if !output.status.success() {
        bail!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}

/// An API key from the environment, else from the nearest `.env` file above
/// `root`. Never printed.
pub(crate) fn credential(name: &str, root: &Path) -> Result<String> {
    if let Ok(value) = env::var(name)
        && !value.trim().is_empty()
    {
        return Ok(value.trim().to_owned());
    }
    let root = root.canonicalize().unwrap_or_else(|_| root.to_owned());
    for dir in root.ancestors() {
        if let Ok(text) = fs::read_to_string(dir.join(".env"))
            && let Some(value) = dotenv(&text, name)
        {
            return Ok(value);
        }
    }
    bail!(
        "{name} is not set; export it or add it to a .env file above {}",
        root.display()
    )
}

fn dotenv(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.trim().trim_start_matches("export ");
        let (key, value) = line.split_once('=')?;
        (key.trim() == name)
            .then(|| value.trim().trim_matches(['"', '\'']).to_owned())
            .filter(|value| !value.is_empty())
    })
}

/// A scratch directory for one job, removed when dropped.
pub(crate) struct Work {
    dir: PathBuf,
}

impl Work {
    pub fn new(id: &str) -> Result<Self> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = env::temp_dir().join(format!(
            "psychopomp-media-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            id.replace(|c: char| !c.is_ascii_alphanumeric(), "-")
        ));
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

impl Drop for Work {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[cfg(test)]
mod tests {
    use super::dotenv;

    #[test]
    fn dotenv_reads_one_variable() {
        let text = "# keys\nexport ELEVENLABS_API_KEY=\"abc\"\nFISH_AUDIO_API_KEY=\nOTHER=x=y\n";
        assert_eq!(dotenv(text, "ELEVENLABS_API_KEY").as_deref(), Some("abc"));
        assert_eq!(dotenv(text, "FISH_AUDIO_API_KEY"), None);
        assert_eq!(dotenv(text, "OTHER").as_deref(), Some("x=y"));
        assert_eq!(dotenv(text, "MISSING"), None);
    }
}
