//! The film's clock and its sound. Every narration clip is placed by phrase
//! (the rant sped up by `balls-with-dots/audio/tempo.ts`), every effect on the
//! word or cut it answers, and from the theory on a tribal pulse accelerates
//! until, in the climax, every cut lands on a drum hit. Times are nanoseconds
//! on the film clock; each segment takes the sound inside its window.
use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result, ensure};
use psychopomp::{
    author::seconds,
    plan::{MediaKindPlan, MediaPlan, MediaRolePlan},
    transcript::Transcript,
};
use serde::Deserialize;

/// The first cut's scene, relative to this one: its narration and stems.
const V2: &str = "../balls-with-dots";

/// A narration clip on the film clock. Its word timings and beats come from
/// the original recording, divided by its tempo.
pub struct Voice {
    pub id: &'static str,
    file: String,
    pub start: u64,
    duration: u64,
    tempo: f64,
    transcript: Transcript,
    /// Syllable beats (start, end) in clip seconds of the original recording.
    beats: Vec<(f64, f64)>,
}

impl Voice {
    fn time(&self, clip_seconds: f64) -> u64 {
        self.start + seconds(clip_seconds / self.tempo)
    }

    /// When `phrase` starts. Panics with the clip if it is not said.
    pub fn at(&self, phrase: &str) -> u64 {
        self.after(phrase, 0.0)
    }

    /// `phrase`, searching only after `earlier` starts.
    pub fn at_after(&self, phrase: &str, earlier: &str) -> u64 {
        let from = self.cue(earlier, 0.0).0;
        self.after(phrase, from)
    }

    /// When `phrase` ends.
    pub fn end_of(&self, phrase: &str) -> u64 {
        self.time(self.cue(phrase, 0.0).1)
    }

    fn after(&self, phrase: &str, after: f64) -> u64 {
        self.time(self.cue(phrase, after).0)
    }

    fn cue(&self, phrase: &str, after: f64) -> (f64, f64) {
        let cue = self
            .transcript
            .phrase_after(phrase, after)
            .unwrap_or_else(|error| panic!("clip '{}': {error:#}", self.id));
        (cue.start().as_seconds(), cue.end().as_seconds())
    }

    /// Every syllable beat's start and end on the film clock.
    pub fn beats(&self) -> Vec<(u64, u64)> {
        self.beats
            .iter()
            .map(|&(start, end)| (self.time(start), self.time(end)))
            .collect()
    }

    pub fn voice_start(&self) -> u64 {
        self.time(self.beats[0].0)
    }

    pub fn voice_end(&self) -> u64 {
        self.time(self.beats[self.beats.len() - 1].1)
    }

    pub fn end(&self) -> u64 {
        self.start + self.duration
    }

    /// Every word and its start and end on the film clock.
    pub fn words(&self) -> Vec<(String, u64, u64)> {
        self.transcript
            .words()
            .iter()
            .map(|word| {
                (
                    word.word.clone(),
                    self.time(word.start),
                    self.time(word.end),
                )
            })
            .collect()
    }
}

/// One placement of a sound file on the film clock.
#[derive(Clone, Debug)]
pub struct Sound {
    pub id: String,
    /// Relative to this scene's directory, where the reel is written.
    pub file: String,
    pub at: u64,
    pub from: u64,
    pub to: u64,
    pub gain_db: f32,
    pub script: bool,
}

impl Sound {
    fn end(&self) -> u64 {
        self.at + (self.to - self.from)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    clips: Vec<ManifestClip>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestClip {
    id: String,
    file: String,
    #[serde(default)]
    words: String,
    #[serde(default = "one")]
    tempo: f64,
    text_hash: String,
    duration_nanos: u64,
}

fn one() -> f64 {
    1.0
}

#[derive(Deserialize)]
struct Beats {
    clips: HashMap<String, BeatClip>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BeatClip {
    text_hash: String,
    beats: Vec<Beat>,
}

#[derive(Deserialize)]
struct Beat {
    start: f64,
    end: f64,
}

#[derive(Deserialize)]
struct Stems {
    stems: Vec<Stem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stem {
    file: String,
    duration_nanos: u64,
}

const RISER: &str = "../../assets/psychopomp-intro/riser.wav";
const SPARKLE: &str = "../../assets/psychopomp-intro/sparkle.wav";
const REUSED: [(&str, f64); 2] = [(RISER, 5.669), (SPARKLE, 1.479)];

fn old(stem: &str) -> String {
    format!("{V2}/audio/sfx/{stem}.flac")
}

fn new(stem: &str) -> String {
    format!("audio/sfx/{stem}.flac")
}

/// Where the riser in `balls-with-dots/audio/sfx/riser.flac` peaks.
const RISER_PEAK: f64 = 4.15;
/// A heartbeat's thump lands this far into its file.
const THUMP: f64 = 0.18;
/// The title holds the dark this long before the whisper.
pub const OPENING: f64 = 1.45;
/// The shouted words of `narration/cubes.mp3` (clip seconds, measured from
/// its envelope: the transcript's word times are wrong), each placed on its
/// own drum hit.
const CUBES: [(f64, f64); 3] = [(0.08, 0.82), (0.98, 1.66), (2.0, 2.92)];
/// The climax, as shot lengths in seconds: three openers, CUBES ARE BALLS,
/// a montage that accelerates into a blur, and the convergence that the
/// colossal hit implodes.
pub const OPENERS: [f64; 3] = [0.42, 0.38, 0.34];
pub const CUBES_SHOT: f64 = 1.72;
/// When each shouted word lands, into the CUBES ARE BALLS shot.
pub const CUBES_WORDS: [f64; 3] = [0.06, 0.62, 1.18];
pub const MONTAGE: [f64; 14] = [
    0.28, 0.26, 0.24, 0.22, 0.2, 0.19, 0.18, 0.17, 0.16, 0.15, 0.14, 0.13, 0.13, 0.12,
];
pub const CONVERGE: f64 = 0.74;
/// The colossal hit lands this far into the convergence.
pub const HIT: f64 = 0.44;
/// How much louder than the rant the climax's wails and drums play.
const CHAOS_GAIN: f32 = 3.0;
/// Dead silence on black after the implosion.
const SILENCE: f64 = 0.45;
/// The shaky exhale (a range of the `after` clip), then the words.
const EXHALE: (f64, f64) = (0.25, 1.15);
const WORDS_FROM: f64 = 1.6;
/// The pull back through balls of balls, one phase per level.
pub const CHAIN: [f64; 3] = [0.95, 0.7, 0.65];
/// The last title fades in, holds, and fades out.
pub const ENDING: (f64, f64, f64) = (0.35, 1.7, 0.45);

pub struct Film {
    pub hush: Voice,
    pub another: Voice,
    pub chant: Voice,
    pub theory: Voice,
    pub andy: Voice,
    pub spaghetti: Voice,
    pub after: Voice,
    /// The voices pile in as "WILL SMITH" lands.
    pub chaos: u64,
    /// Every climax cut: openers, the cubes shot, the montage, convergence.
    pub shots: Vec<u64>,
    /// When "CUBES", "ARE", and "BALLS" land.
    pub cubes: [u64; 3],
    /// The colossal drum hit: every ball implodes.
    pub implosion: u64,
    /// The hard cut to silence.
    pub cut: u64,
    /// The breath before the last whisper.
    pub exhale: u64,
    /// The pull back through balls of balls starts.
    pub chain: u64,
    /// "In the end, there were balls."
    pub title: u64,
    pub end: u64,
    /// Every heartbeat thump, from "another" to the end of the chant.
    pub heartbeats: Vec<u64>,
    /// The tribal pulse, accelerating from the theory into the climax.
    pub pulse: Vec<u64>,
    pub sounds: Vec<Sound>,
    lengths: HashMap<String, u64>,
}

impl Film {
    pub fn load(root: &Path) -> Result<Self> {
        let read = |path: &str| -> Result<Vec<u8>> {
            fs::read(root.join(path)).with_context(|| format!("read {path}"))
        };
        let narration: Manifest =
            serde_json::from_slice(&read(&format!("{V2}/narration/narration.json"))?)?;
        let tempo: Manifest =
            serde_json::from_slice(&read(&format!("{V2}/audio/tempo/tempo.json"))?)?;
        let beats: Beats = serde_json::from_slice(&read(&format!("{V2}/audio/beats.json"))?)?;
        let mut lengths = HashMap::new();
        for (manifest, prefix) in [
            (format!("{V2}/audio/manifest.json"), format!("{V2}/")),
            ("audio/manifest.json".to_owned(), String::new()),
        ] {
            let stems: Stems = serde_json::from_slice(&read(&manifest)?)?;
            for stem in stems.stems {
                // Stems reused from `assets` resolve the same from both scenes.
                let file = if stem.file.starts_with("../") {
                    stem.file
                } else {
                    format!("{prefix}{}", stem.file)
                };
                lengths.insert(file, stem.duration_nanos);
            }
        }
        for (file, length) in REUSED {
            lengths.insert(file.to_owned(), seconds(length));
        }
        for clip in &narration.clips {
            lengths.insert(format!("{V2}/narration/{}", clip.file), clip.duration_nanos);
        }
        let shouted: Manifest = serde_json::from_slice(&read("narration/narration.json")?)?;
        for clip in &shouted.clips {
            lengths.insert(format!("narration/{}", clip.file), clip.duration_nanos);
        }

        let voice = |id: &'static str, start: u64| -> Result<Voice> {
            let original = narration
                .clips
                .iter()
                .find(|clip| clip.id == id)
                .with_context(|| format!("narration has no clip '{id}'"))?;
            let faster = tempo.clips.iter().find(|clip| clip.id == id);
            let beat = beats
                .clips
                .get(id)
                .with_context(|| format!("beats.json has no clip '{id}'"))?;
            ensure!(
                beat.text_hash == original.text_hash
                    && faster.is_none_or(|clip| clip.text_hash == original.text_hash),
                "clip '{id}' changed: rerun its beats and tempo"
            );
            let (file, duration, tempo) = match faster {
                Some(clip) => (
                    format!("{V2}/{}", clip.file),
                    clip.duration_nanos,
                    clip.tempo,
                ),
                None => (
                    format!("{V2}/narration/{}", original.file),
                    original.duration_nanos,
                    1.0,
                ),
            };
            Ok(Voice {
                id,
                file,
                start,
                duration,
                tempo,
                transcript: Transcript::load(
                    &root.join(V2).join("narration").join(&original.words),
                )?,
                beats: beat.beats.iter().map(|b| (b.start, b.end)).collect(),
            })
        };
        // Each clip starts `gap` after the previous voice falls silent.
        let next = |previous: &Voice, id: &'static str, gap: f64| -> Result<Voice> {
            let mut voice = voice(id, 0)?;
            let lead = voice.voice_start();
            voice.start = (previous.voice_end() + seconds(gap)).saturating_sub(lead);
            Ok(voice)
        };
        let hush = voice("hush", seconds(OPENING))?;
        let another = next(&hush, "another", 0.24)?;
        let chant = next(&another, "chant", 0.2)?;
        let theory = next(&chant, "theory", 0.15)?;
        let andy = next(&theory, "andy", 0.12)?;
        let spaghetti = next(&andy, "spaghetti", 0.08)?;
        let chaos = spaghetti.end_of("will smith") + seconds(0.1);

        let mut shots = Vec::new();
        let mut at = chaos;
        for length in OPENERS
            .iter()
            .chain([CUBES_SHOT].iter())
            .chain(MONTAGE.iter())
        {
            shots.push(at);
            at += seconds(*length);
        }
        shots.push(at);
        let converge = at;
        let cubes_shot = shots[OPENERS.len()];
        let cubes = CUBES_WORDS.map(|offset| cubes_shot + seconds(offset));
        let implosion = converge + seconds(HIT);
        let cut = converge + seconds(CONVERGE);
        let exhale = cut + seconds(SILENCE);
        let mut after = voice("after", 0)?;
        after.start =
            (exhale + seconds(EXHALE.1 - EXHALE.0 + 0.1)).saturating_sub(seconds(WORDS_FROM));
        let chain = after.voice_end() + seconds(0.15);
        let title = chain + seconds(CHAIN.iter().sum::<f64>() + 0.05);
        let end = title + seconds(ENDING.0 + ENDING.1 + ENDING.2);

        let mut film = Self {
            hush,
            another,
            chant,
            theory,
            andy,
            spaghetti,
            after,
            chaos,
            shots,
            cubes,
            implosion,
            cut,
            exhale,
            chain,
            title,
            end,
            heartbeats: Vec::new(),
            pulse: Vec::new(),
            sounds: Vec::new(),
            lengths,
        };
        film.score();
        Ok(film)
    }

    fn length(&self, file: &str) -> u64 {
        *self
            .lengths
            .get(file)
            .unwrap_or_else(|| panic!("no length for {file}; is it in a manifest?"))
    }

    fn speak(&mut self, id: &str, gain_db: f32) {
        let voice = [
            &self.hush,
            &self.another,
            &self.chant,
            &self.theory,
            &self.andy,
            &self.spaghetti,
        ]
        .into_iter()
        .find(|voice| voice.id == id)
        .expect("a placed voice");
        let sound = Sound {
            id: format!("narration-{}", voice.id),
            file: voice.file.clone(),
            at: voice.start,
            from: 0,
            to: voice.duration,
            gain_db,
            script: true,
        };
        self.sounds.push(sound);
    }

    /// Place `file` whole at `at`; trim the returned placement as needed.
    fn layer(&mut self, id: &str, file: &str, at: u64, gain_db: f32) -> &mut Sound {
        let to = self.length(file);
        self.sounds.push(Sound {
            id: id.to_owned(),
            file: file.to_owned(),
            at,
            from: 0,
            to,
            gain_db,
            script: false,
        });
        self.sounds.last_mut().unwrap()
    }

    fn score(&mut self) {
        self.genesis();
        self.chant_score();
        self.rant();
        self.climax();
        self.coda();
        self.sounds.sort_by_key(|sound| sound.at);
    }

    /// The dark, the ignition, the whisper, another ball, and a heart.
    fn genesis(&mut self) {
        self.speak("hush", -2.5);
        let room = old("room-tone");
        let rest = self.layer("room-tone-a", &room, 0, -25.0).to;
        let chant = self.chant.voice_start();
        self.layer("room-tone-b", &room, rest, -25.0).to = chant.saturating_sub(rest);
        let pad = new("pad");
        self.layer("pad", &pad, 0, -9.0).to = chant + seconds(0.5);
        self.layer("sub-open", &new("boom-low"), seconds(0.05), -16.0);
        self.layer("mic-rustle", &old("mic-rustle"), seconds(0.3), -18.0);
        let balls = self.hush.at("balls");
        self.layer("ignite", &new("ignite"), balls - seconds(0.05), -8.0);
        self.layer("pop-ball-1", &old("pop-00"), balls, -14.0);
        let dots = self.hush.at("dots");
        self.layer("sparkle-dots", SPARKLE, dots, -15.0);
        self.layer("chime-dots", &new("chime"), dots, -20.0);

        self.speak("another", -2.0);
        let what = self.another.start;
        self.layer("drone-swell", &old("drone-swell"), what, -13.0);
        let another = self.another.at("another");
        self.layer(
            "whoosh-another",
            &new("whoosh-cut"),
            another - seconds(0.18),
            -16.0,
        );
        self.layer("pop-ball-2", &old("pop-01"), another, -9.0);
        self.layer("boom-another", &new("boom"), another, -17.0);
        let more = self.another.at("more dots");
        self.layer("sparkle-more", SPARKLE, more, -13.0);
        self.layer("swell-more", &new("swell"), more - seconds(0.5), -16.0);

        // The heart starts on "another" and races to the end of the chant.
        let (from, to) = (another + seconds(0.5), self.chant.end());
        let mut time = from;
        while time < to {
            self.heartbeats.push(time);
            let progress = (time - from) as f64 / (to - from) as f64;
            time += seconds(0.95 + (0.34 - 0.95) * progress);
        }
        for (index, thump) in self.heartbeats.clone().into_iter().enumerate() {
            let file = old("heartbeat");
            self.layer(
                &format!("heartbeat-{index}"),
                &file,
                thump - seconds(THUMP),
                -9.0,
            );
        }
    }

    /// One pop per "balls", a semitone higher each time, and from the third
    /// a drum under each; then the dive into a dot.
    fn chant_score(&mut self) {
        self.speak("chant", 0.0);
        let beats = self.chant.beats();
        for (index, (beat, _)) in beats.iter().copied().enumerate() {
            let pop = old(&format!("pop-{:02}", (index + 2).min(15)));
            self.layer(
                &format!("pop-chant-{}", index + 1),
                &pop,
                beat,
                -11.0 + index as f32 * 0.4,
            );
            if index >= 2 && index + 1 < beats.len() {
                let drum = if index % 2 == 0 { "tom" } else { "slap" };
                self.layer(
                    &format!("drum-chant-{index}"),
                    &new(drum),
                    beat,
                    -15.0 + index as f32 * 0.4,
                );
            }
        }
        let last = beats[beats.len() - 1].0;
        self.layer("sub-chant", &new("sub-drop"), last, -6.0);
        self.layer("taiko-chant", &new("taiko-low"), last, -6.0);
        self.layer("boom-chant", &new("boom"), last, -11.0);
        // The dive: air rushing past, into the first word of the theory.
        let dive = self.dive();
        self.layer("swell-dive", &new("swell"), dive.1 - seconds(0.7), -9.0);
        self.layer(
            "whoosh-dive",
            &new("whoosh-cut"),
            dive.1 - seconds(0.12),
            -9.0,
        );
        self.layer(
            "drone",
            &old("drone"),
            self.chant.start + seconds(1.0),
            -17.0,
        );
    }

    /// The chant's last "BALLS!" and where the dive lands, on the theory.
    pub fn dive(&self) -> (u64, u64) {
        let beats = self.chant.beats();
        (
            beats[beats.len() - 1].0,
            self.theory.voice_start().saturating_sub(seconds(0.12)),
        )
    }

    /// The theory, Andy, and the spaghetti: a tribal pulse that accelerates
    /// from the theory to the climax, and every slammed word.
    fn rant(&mut self) {
        self.speak("theory", 0.5);
        self.speak("andy", 2.0);
        self.speak("spaghetti", 1.5);
        let (from, to) = (self.theory.voice_start(), self.chaos);
        let mut time = from;
        while time < to {
            self.pulse.push(time);
            let progress = (time - from) as f64 / (to - from) as f64;
            time += seconds(0.58 + (0.24 - 0.58) * progress.powf(0.8));
        }
        for (index, hit) in self.pulse.clone().into_iter().enumerate() {
            let progress = (hit - from) as f32 / (to - from) as f32;
            let (file, gain) = match index % 4 {
                0 => ("taiko-low", -9.0 + 6.0 * progress),
                2 => ("taiko", -13.0 + 6.0 * progress),
                _ => ("tom", -16.0 + 7.0 * progress),
            };
            self.layer(&format!("pulse-{index}"), &new(file), hit, gain);
            if progress > 0.45 {
                self.layer(
                    &format!("pulse-slap-{index}"),
                    &new("slap"),
                    hit + (self.pulse_gap(index) / 2),
                    -17.0 + 6.0 * progress,
                );
            }
        }
        let andy = self.andy.voice_start();
        self.layer("drum-war-a", &old("drum-war"), andy, -9.0);
        let lasagna = self.spaghetti.at("lasagna");
        self.layer("drum-war-b", &old("drum-war"), lasagna, -8.0);

        // The theory's exhibits.
        let atoms = self.theory.at("atoms");
        self.layer("zap-atoms", &old("zap"), atoms - seconds(0.1), -12.0);
        self.layer("thunder-atoms", &new("thunder"), atoms, -16.0);
        for (index, at) in self.whips().into_iter().enumerate() {
            self.layer(&format!("whip-{index}"), &new("whoosh-cut"), at, -11.0);
        }
        for phrase in ["hopes", "dreams"] {
            let at = self.theory.at(phrase);
            self.layer(&format!("sparkle-{phrase}"), SPARKLE, at, -14.0);
        }
        let same = self.theory.at_after("balls with dots", "dreams");
        self.layer("glitch-dots", &old("glitch"), same, -12.0);
        let basketballs = self.theory.at("basketballs");
        self.layer("bounce-1", &new("bounce"), basketballs - seconds(0.3), -9.0);
        self.layer(
            "bounce-2",
            &new("bounce"),
            basketballs + seconds(0.02),
            -9.0,
        );
        self.layer("swish", &new("swish"), basketballs + seconds(0.42), -7.0);
        self.layer("crowd", &new("crowd"), basketballs + seconds(0.45), -15.0)
            .to = seconds(1.3);
        self.layer("riser", &old("riser"), andy - seconds(RISER_PEAK), -9.0);

        // ANDY SERKIS.
        self.layer("braam", &new("braam"), andy, -11.0).to = seconds(1.4);
        self.layer("sub-andy", &new("sub-drop"), andy, -9.0);
        self.layer("impact-andy", &old("impact"), andy, -12.0);
        self.layer("beep", &new("beep"), andy + seconds(0.55), -14.0);
        let cover = self.andy.at("balls with dots");
        for index in 0..8 {
            let pop = old(&format!("pop-{:02}", 4 + index));
            self.layer(
                &format!("pop-marker-{index}"),
                &pop,
                cover - seconds(0.4) + seconds(0.07 * index as f64),
                -15.0,
            );
        }
        let anything = self.andy.at("anything");
        self.layer("whoosh-anything", &new("whoosh-cut"), anything, -9.0);
        let chimp = self.andy.end_of("chimp");
        self.layer(
            "chimp-cameo",
            &format!("{V2}/narration/chaos-chimp.mp3"),
            chimp,
            -14.0,
        )
        .to = seconds(0.7);
        let shriveled = self.andy.at("shriveled");
        self.layer("pop-shrivel", &old("pop-00"), shriveled, -12.0);

        // Spaghetti, Will Smith.
        for phrase in ["lasagna", "spaghetti"] {
            let at = self.spaghetti.at(phrase);
            self.layer(
                &format!("whoosh-{phrase}"),
                &new("whoosh-cut"),
                at - seconds(0.05),
                -9.0,
            );
            self.layer(&format!("boom-{phrase}"), &new("boom"), at, -14.0);
        }
        let woo = self.spaghetti.at("woo");
        self.layer("thunder-woo", &new("thunder"), woo, -12.0);
        self.layer(
            "slurp",
            &old("slurp"),
            self.spaghetti.at("spaghetti kid"),
            -15.0,
        );
        let eaten = self.spaghetti.at("eaten");
        self.layer("swell-fork", &new("swell"), eaten - seconds(0.2), -10.0);
        let smith = self.spaghetti.at("will smith");
        let named = self.spaghetti.end_of("will smith");
        self.layer(
            "chomp",
            &new("chomp"),
            named.saturating_sub(seconds(0.05)),
            -5.0,
        );
        self.layer("impact-smith", &old("impact"), smith, -12.0);
        self.layer("sub-smith", &new("sub-drop"), smith, -11.0);
        self.layer("choir", &new("choir"), eaten - seconds(0.6), -13.0);
        for (index, (at, strength)) in self.slams().into_iter().enumerate() {
            self.layer(
                &format!("slam-{index}"),
                &new("taiko"),
                at,
                -15.0 + 6.0 * strength,
            );
        }
    }

    fn pulse_gap(&self, index: usize) -> u64 {
        match self.pulse.get(index + 1) {
            Some(next) => next - self.pulse[index],
            None => seconds(0.24),
        }
    }

    /// Every voice at once, and a drum on every cut.
    fn climax(&mut self) {
        let chaos = self.chaos;
        let implosion = self.implosion;
        let cut = self.cut;
        let voices: [(&str, &str, f64, f32, f64); 16] = [
            // id, file, offset after the chaos cue, gain, source start
            ("babble", "narration/chaos-babble.mp3", 0.0, -8.0, 0.0),
            ("yowl", "narration/chaos-yowl.mp3", 0.08, -7.0, 0.0),
            ("laugh", "narration/chaos-laugh.mp3", 0.18, -9.0, 0.0),
            (
                "babble-chipmunk",
                "audio/voices/babble-chipmunk.mp3",
                0.3,
                -12.0,
                0.0,
            ),
            ("yowl-demon", "audio/voices/yowl-demon.mp3", 0.42, -9.0, 0.0),
            (
                "laugh-demon",
                "audio/voices/laugh-demon.mp3",
                0.55,
                -9.0,
                0.0,
            ),
            ("howl", "narration/chaos-howl.mp3", 0.66, -8.0, 0.0),
            (
                "spaghetti-demon",
                "audio/voices/spaghetti-demon.mp3",
                0.75,
                -10.0,
                3.5,
            ),
            (
                "babble-demon",
                "audio/voices/babble-demon.mp3",
                0.85,
                -10.0,
                0.0,
            ),
            (
                "laugh-chipmunk",
                "audio/voices/laugh-chipmunk.mp3",
                1.0,
                -13.0,
                0.0,
            ),
            ("howl-low", "audio/voices/howl-low.mp3", 2.9, -9.0, 0.0),
            ("chimp", "narration/chaos-chimp.mp3", 2.95, -9.0, 0.0),
            (
                "theory-reversed",
                "audio/voices/theory-reversed.mp3",
                3.0,
                -13.0,
                0.0,
            ),
            (
                "chant-demon",
                "audio/voices/chant-demon.mp3",
                3.05,
                -9.0,
                4.1,
            ),
            (
                "spaghetti-chipmunk",
                "audio/voices/spaghetti-chipmunk.mp3",
                3.1,
                -12.0,
                5.8,
            ),
            (
                "babble-reversed",
                "audio/voices/babble-reversed.mp3",
                3.2,
                -12.0,
                0.0,
            ),
        ];
        for (id, file, offset, gain, from) in voices {
            let file = format!("{V2}/{file}");
            let sound = self.layer(
                &format!("chaos-{id}"),
                &file,
                chaos + seconds(offset),
                gain + CHAOS_GAIN,
            );
            sound.from = seconds(from);
        }
        // CUBES! ARE! BALLS! Each word on its own hit; the wails duck under it.
        let words = self.cubes;
        for (index, (at, (from, to))) in words.into_iter().zip(CUBES).enumerate() {
            let shout = self.layer(&format!("cubes-{index}"), "narration/cubes.mp3", at, 3.0);
            shout.from = seconds(from);
            shout.to = seconds(to);
            shout.script = true;
            self.layer(&format!("cubes-taiko-{index}"), &new("taiko-low"), at, -3.0);
            self.layer(&format!("cubes-boom-{index}"), &new("boom"), at, -12.0);
        }
        let quiet = (
            words[0].saturating_sub(seconds(0.1)),
            words[2] + seconds(0.9),
        );
        self.duck("chaos-", quiet, -10.0);
        // A drum on every cut, alternating voices; a whoosh under most.
        for (index, at) in self.shots.clone().into_iter().enumerate() {
            let (file, gain) = match index % 4 {
                0 => ("taiko-low", -4.0),
                1 => ("tom", -6.0),
                2 => ("taiko-high", -7.0),
                _ => ("slap", -8.0),
            };
            // The cubes shot opens on its first shouted word's drum.
            if index != OPENERS.len() {
                self.layer(
                    &format!("cut-drum-{index}"),
                    &new(file),
                    at,
                    gain + CHAOS_GAIN,
                );
            }
            if index % 2 == 1 {
                self.layer(
                    &format!("cut-whoosh-{index}"),
                    &new("whoosh-cut"),
                    at,
                    -14.0,
                );
            }
            let pop = old(&format!("pop-{:02}", 3 + (index * 5) % 13));
            self.layer(&format!("cut-pop-{index}"), &pop, at + seconds(0.05), -15.0);
        }
        // Frenzy beds and a stadium.
        for (index, offset) in [0.0, 2.7].into_iter().enumerate() {
            let at = chaos + seconds(offset);
            self.layer(
                &format!("frenzy-{index}"),
                &old("drum-frenzy"),
                at,
                -6.0 + index as f32 + CHAOS_GAIN,
            );
        }
        self.layer("crowd-chaos", &new("crowd"), chaos + seconds(3.0), -14.0);
        self.layer("thunder-chaos", &new("thunder"), chaos, -12.0);
        self.layer("riser-chaos", RISER, implosion - seconds(5.669), -11.0);
        self.layer("suck", &new("suck"), implosion - seconds(0.42), -6.0);
        // Everything stops on the colossal hit but the hit, a stem that
        // stops dead (with a short fade) at the cut.
        for sound in &mut self.sounds {
            if !sound.script && sound.at < implosion && sound.end() > implosion {
                sound.to = sound.from + (implosion - sound.at);
            }
        }
        self.sounds
            .retain(|sound| sound.script || sound.at < implosion);
        let slam = self.layer("colossal", &new("slam-cut"), implosion, 2.0);
        debug_assert!(slam.end() + seconds(0.002) >= cut);
    }

    /// Split every placement whose id starts with `prefix` around `range`
    /// so the part inside plays `by` dB quieter.
    fn duck(&mut self, prefix: &str, (from, until): (u64, u64), by: f32) {
        let mut added = Vec::new();
        for sound in &mut self.sounds {
            if !sound.id.starts_with(prefix) || sound.end() <= from || sound.at >= until {
                continue;
            }
            let original = sound.clone();
            let start = original.at.max(from);
            let stop = original.end().min(until);
            // Before the range stays as it was.
            if original.at < start {
                sound.to = original.from + (start - original.at);
            } else {
                sound.to = sound.from;
            }
            added.push(Sound {
                id: format!("{}-ducked", original.id),
                at: start,
                from: original.from + (start - original.at),
                to: original.from + (stop - original.at),
                gain_db: original.gain_db + by,
                ..original.clone()
            });
            if original.end() > stop {
                added.push(Sound {
                    id: format!("{}-after", original.id),
                    at: stop,
                    from: original.from + (stop - original.at),
                    ..original
                });
            }
        }
        self.sounds.retain(|sound| sound.to > sound.from);
        self.sounds.extend(added);
    }

    /// Silence, a breath, the whisper, and the pull back.
    fn coda(&mut self) {
        let cut = self.cut;
        self.layer("tinnitus", &old("tinnitus"), cut, -34.0).to = seconds(2.2);
        let exhale = self.exhale;
        let file = self.after.file.clone();
        let breath = self.layer("after-exhale", &file, exhale, -3.0);
        breath.from = seconds(EXHALE.0);
        breath.to = seconds(EXHALE.1);
        let words = self.after.start + seconds(WORDS_FROM);
        let sound = self.layer("narration-after", &file, words, -5.0);
        sound.from = seconds(WORDS_FROM);
        sound.script = true;
        let chain = self.chain;
        self.layer(
            "shimmer",
            &old("shimmer"),
            chain.saturating_sub(seconds(0.1)),
            -13.0,
        );
        self.layer(
            "pad-end",
            &new("pad"),
            chain.saturating_sub(seconds(0.4)),
            -19.0,
        )
        .from = seconds(4.0);
        let title = self.title;
        self.layer("drum-soft", &old("drum-soft"), title, -6.0);
        self.layer("chime-period", &new("chime"), self.period(), -12.0);
        let end = self.end;
        for sound in &mut self.sounds {
            if sound.at < end && sound.end() > end {
                sound.to = sound.from + (end - sound.at);
            }
        }
    }

    /// The last title's period pops in as a tiny ball.
    pub fn period(&self) -> u64 {
        self.title + seconds(ENDING.0 + 0.45)
    }

    /// The theory's whip pans, each landing on its claim's subject.
    pub fn whips(&self) -> [u64; 3] {
        [
            self.theory.at("thoughts"),
            self.theory.at_after("thoughts", "dreams"),
            self.theory.at("dots represent basketballs"),
        ]
        .map(|at| at.saturating_sub(seconds(crate::theory::WHIP)))
    }

    /// Every slammed word and how hard it lands, on the film clock.
    pub fn slams(&self) -> Vec<(u64, f32)> {
        let (theory, andy, spaghetti) = (&self.theory, &self.andy, &self.spaghetti);
        vec![
            (theory.at("atoms"), 0.7),
            (theory.at("hopes"), 0.5),
            (theory.at("dreams"), 0.5),
            (theory.at_after("balls with dots", "dreams"), 0.7),
            (theory.at("basketballs"), 0.9),
            (andy.at("anything"), 0.8),
            (andy.at("chimp"), 0.7),
            (andy.at("shriveled"), 0.5),
            (spaghetti.at("lasagna"), 0.8),
            (spaghetti.at("spaghetti"), 0.8),
            (spaghetti.at("woo"), 1.0),
            (spaghetti.at("spaghetti kid"), 0.8),
            (spaghetti.at("will smith"), 1.0),
        ]
    }

    /// The sounds inside `[from, until)` on the film clock, as media of a
    /// segment that starts at `from`.
    pub fn media(&self, from: u64, until: u64) -> Vec<MediaPlan> {
        let mut media = Vec::new();
        for sound in &self.sounds {
            let start = sound.at.max(from);
            let end = sound.end().min(until);
            if start >= end {
                continue;
            }
            let skip = start - sound.at;
            media.push(MediaPlan {
                id: sound.id.clone(),
                path: sound.file.clone().into(),
                kind: MediaKindPlan::Audio,
                role: if sound.script {
                    MediaRolePlan::Script
                } else {
                    MediaRolePlan::Layer
                },
                source_start_nanos: sound.from + skip,
                source_end_nanos: sound.from + skip + (end - start),
                timeline_start_nanos: start - from,
                timeline_end_nanos: end - from,
                gain_db: sound.gain_db,
            });
        }
        media
    }
}
