//! ffmpeg and ffprobe subprocesses: loudness, sound-effect finishing, silence,
//! effects, and exact durations.
use std::{path::Path, process::Command};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::{spec::Effect, studio::run};

fn ffmpeg() -> Command {
    let mut command = Command::new("ffmpeg");
    command.args(["-y", "-loglevel", "error"]);
    command
}

fn arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// One loudness target for every voice keeps narration even across clips
/// (`scripts/narrate.ts`): -16 LUFS, 48 kHz mono, 160 kbps MP3.
pub(crate) fn loudness(raw: &Path, out: &Path) -> Result<()> {
    run(ffmpeg().args([
        "-i",
        &arg(raw),
        "-af",
        "loudnorm=I=-16:TP=-1.5:LRA=11",
        "-ar",
        "48000",
        "-ac",
        "1",
        "-b:a",
        "160k",
        &arg(out),
    ]))?;
    Ok(())
}

/// Mono 48 kHz samples as `f32`.
fn samples(file: &Path) -> Result<Vec<f32>> {
    let pcm = run(ffmpeg().args([
        "-i",
        &arg(file),
        "-ac",
        "1",
        "-ar",
        "48000",
        "-f",
        "f32le",
        "-",
    ]))?;
    Ok(pcm
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| f32::from_le_bytes(*bytes))
        .collect())
}

/// A sound effect ready to cue: leading silence trimmed to 3 ms before the
/// onset (2% of peak), peak at -6 dBFS, and a 60 ms fade on the tail, as
/// `scenes/psychopomp-intro/sfx/generate.ts` finished its stems.
pub(crate) fn finish_sound(raw: &Path, out: &Path) -> Result<()> {
    let samples = samples(raw)?;
    let peak = samples.iter().fold(0f32, |peak, s| peak.max(s.abs()));
    if peak < 0.0001 {
        bail!("the generated sound is silent");
    }
    let onset = samples
        .iter()
        .position(|s| s.abs() > peak * 0.02)
        .unwrap_or(0);
    let start = (onset as f64 / 48_000.0 - 0.003).max(0.0);
    let duration = samples.len() as f64 / 48_000.0 - start;
    let filter = format!(
        "atrim=start={start}:duration={duration},asetpts=PTS-STARTPTS,volume={},afade=t=out:st={}:d=0.06",
        0.5 / f64::from(peak),
        (duration - 0.06).max(0.0)
    );
    run(ffmpeg().args([
        "-i",
        &arg(raw),
        "-af",
        &filter,
        "-ar",
        "48000",
        "-ac",
        "1",
        "-c:a",
        "pcm_s16le",
        &arg(out),
    ]))?;
    Ok(())
}

/// `nanos` of mono 48 kHz silence.
pub(crate) fn silence(nanos: u64, out: &Path) -> Result<()> {
    run(ffmpeg().args([
        "-f",
        "lavfi",
        "-i",
        "anullsrc=r=48000:cl=mono",
        "-t",
        &format!("{:.9}", nanos as f64 / 1e9),
        "-c:a",
        "pcm_s16le",
        &arg(out),
    ]))?;
    Ok(())
}

/// `input` transformed by `effect`, re-encoded like its source.
pub(crate) fn derive(input: &Path, effect: &Effect, out: &Path) -> Result<()> {
    let rate = probe(input)?
        .sample_rate
        .context("the source has no sample rate")?
        .parse::<u32>()?;
    let filter = match *effect {
        Effect::Pitch { semitones } => {
            let shifted = (f64::from(rate) * 2f64.powf(semitones / 12.0)).round();
            format!(
                "asetrate={shifted},aresample={rate},{}",
                tempo(f64::from(rate) / shifted)
            )
        }
        Effect::Tempo { factor } => tempo(factor),
        Effect::Reverse => "areverse".to_owned(),
        Effect::Trim {
            start_nanos,
            end_nanos,
        } => format!(
            "atrim=start={}:end={},asetpts=PTS-STARTPTS",
            start_nanos as f64 / 1e9,
            end_nanos as f64 / 1e9
        ),
        Effect::Gain { db } => format!("volume={db}dB"),
    };
    let codec: &[&str] = match out.extension().and_then(|ext| ext.to_str()) {
        Some("mp3") => &["-b:a", "160k"],
        _ => &["-c:a", "pcm_s16le"],
    };
    run(ffmpeg()
        .args([
            "-i",
            &arg(input),
            "-af",
            &filter,
            "-ar",
            "48000",
            "-ac",
            "1",
        ])
        .args(codec)
        .arg(arg(out)))?;
    Ok(())
}

/// `atempo` stages: each handles 0.5 to 100.
fn tempo(mut factor: f64) -> String {
    let mut stages = Vec::new();
    while factor < 0.5 {
        stages.push("atempo=0.5".to_owned());
        factor /= 0.5;
    }
    stages.push(format!("atempo={factor}"));
    stages.join(",")
}

#[derive(Deserialize)]
struct Probe {
    codec_name: Option<String>,
    sample_rate: Option<String>,
    duration_ts: Option<u64>,
    time_base: Option<String>,
}

#[derive(Deserialize)]
struct Probed {
    streams: Vec<Probe>,
}

fn probe(file: &Path) -> Result<Probe> {
    let out = run(Command::new("ffprobe").args([
        "-v",
        "error",
        "-select_streams",
        "a:0",
        "-show_entries",
        "stream=codec_name,sample_rate,duration_ts,time_base",
        "-of",
        "json",
        &arg(file),
    ]))?;
    serde_json::from_slice::<Probed>(&out)?
        .streams
        .into_iter()
        .next()
        .with_context(|| format!("{} has no audio stream", file.display()))
}

/// Exact decoded length in nanoseconds, from the stream's sample count rather
/// than container metadata. PCM is exact; compressed audio stays a millisecond
/// inside its decoded length, as `scripts/narrate.ts` measures, because MP3
/// padding makes the last frame's end imprecise.
pub(crate) fn duration(file: &Path) -> Result<u64> {
    let probe = probe(file)?;
    let ticks = probe
        .duration_ts
        .with_context(|| format!("{} has no duration", file.display()))?;
    let (num, den) = probe
        .time_base
        .as_deref()
        .and_then(|base| base.split_once('/'))
        .and_then(|(num, den)| Some((num.parse::<u64>().ok()?, den.parse::<u64>().ok()?)))
        .with_context(|| format!("{} has no time base", file.display()))?;
    if probe
        .codec_name
        .as_deref()
        .is_some_and(|codec| codec.starts_with("pcm_"))
    {
        return Ok((u128::from(ticks) * u128::from(num) * 1_000_000_000 / u128::from(den)) as u64);
    }
    let seconds = (ticks as f64 * num as f64) / den as f64;
    Ok(((seconds - 0.001) * 1e9).floor() as u64)
}

#[cfg(test)]
mod tests {
    use super::tempo;

    #[test]
    fn slow_tempos_chain_atempo_stages() {
        assert_eq!(tempo(1.5), "atempo=1.5");
        assert_eq!(tempo(0.25), "atempo=0.5,atempo=0.5");
    }
}
