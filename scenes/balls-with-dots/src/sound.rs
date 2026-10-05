//! The film's clock and its sound: every narration clip placed by phrase,
//! the later clips sped up (`audio/tempo.ts`), and every sound effect placed
//! on the words it answers. Times are nanoseconds on the film clock; each
//! segment takes the placements that fall inside its window.
use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result, ensure};
use psychopomp::{
    author::seconds,
    plan::{MediaKindPlan, MediaPlan, MediaRolePlan},
    transcript::Transcript,
};
use serde::Deserialize;

/// A narration clip on the film clock. Its word timings and beats come from
/// the original recording, divided by its tempo.
pub struct Voice {
    pub id: &'static str,
    file: String,
    pub start: u64,
    duration: u64,
    tempo: f64,
    transcript: Transcript,
    /// Syllable beats (start, end) from `audio/beats.json`, in clip seconds
    /// of the original recording.
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
    /// Relative to the scene directory, where the reel is written.
    pub file: String,
    pub at: u64,
    pub from: u64,
    pub to: u64,
    pub gain_db: f32,
    pub script: bool,
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

/// Lengths of the sounds reused from `assets`.
const REUSED: [(&str, f64); 4] = [
    ("psychopomp-intro/riser", 5.669),
    ("psychopomp-intro/whoosh", 0.611),
    ("psychopomp-intro/sparkle", 1.479),
    ("opencode-hot-reload/impact", 0.52),
];
const RISER: &str = "../../assets/psychopomp-intro/riser.wav";
const WHOOSH: &str = "../../assets/psychopomp-intro/whoosh.wav";
const SPARKLE: &str = "../../assets/psychopomp-intro/sparkle.wav";
const IMPACT: &str = "../../assets/opencode-hot-reload/impact.wav";

/// Where the riser in `audio/sfx/riser.flac` peaks, just before it cuts off.
const RISER_PEAK: f64 = 4.15;
/// A heartbeat's thump lands this far into its file.
const THUMP: f64 = 0.18;
/// A whip pan between the theory's exhibits takes this long.
pub const WHIP: f64 = 0.42;
/// The opening title holds the dark this long before the whisper.
const OPENING: f64 = 2.3;
/// The climax: a montage of shots this long each, every voice at once, then
/// everything stops.
/// The last shot holds the colossal hit and the implosion.
pub const MONTAGE: [f64; 13] = [
    0.34, 0.3, 0.28, 0.28, 0.26, 0.24, 0.26, 0.62, 0.24, 0.22, 0.3, 0.22, 0.8,
];
/// The colossal hit lands this far into the last shot.
pub const HIT: f64 = 0.3;
/// Dead silence on black after the whiteout.
const SILENCE: f64 = 0.7;
/// The shaky exhale (a range of the `after` clip), then the words a breath
/// later: the clip's own pause between them is cut short.
const EXHALE: (f64, f64) = (0.1, 1.3);
const WORDS_FROM: f64 = 1.6;
/// The pull back through balls of balls: one phase per level, faster each time.
pub const CHAIN: [f64; 3] = [1.4, 0.95, 0.75];
/// The last title fades in, holds, and fades out.
pub const ENDING: (f64, f64, f64) = (0.35, 1.5, 0.5);

/// The sound a cut is marked with.
#[derive(Clone, Copy, Debug)]
pub enum Accent {
    Dive,
    Whip,
    Glitch,
    Flash,
    /// The montage's `n`th cut, on a drum.
    Montage(usize),
}

pub struct Film {
    pub hush: Voice,
    pub another: Voice,
    pub chant: Voice,
    pub theory: Voice,
    pub andy: Voice,
    pub spaghetti: Voice,
    pub after: Voice,
    /// The voices pile in as "WILL SMITH" trails off.
    pub chaos: u64,
    /// The colossal drum hit: every ball implodes.
    pub implosion: u64,
    /// The hard cut to silence.
    pub cut: u64,
    /// The pull back through balls of balls starts.
    pub chain: u64,
    /// "In the end, there were balls."
    pub title: u64,
    pub end: u64,
    /// The breath before the last whisper.
    pub exhale: u64,
    /// Every heartbeat thump, from "another" to the end of the chant.
    pub heartbeats: Vec<u64>,
    pub sounds: Vec<Sound>,
    lengths: HashMap<String, u64>,
}

impl Film {
    /// When each montage shot starts, on the film clock.
    pub fn montage(&self) -> Vec<u64> {
        MONTAGE
            .iter()
            .scan(self.chaos, |at, length| {
                let start = *at;
                *at += seconds(*length);
                Some(start)
            })
            .collect()
    }

    pub fn load(root: &Path) -> Result<Self> {
        let read = |path: &str| -> Result<Vec<u8>> {
            fs::read(root.join(path)).with_context(|| format!("read {path}"))
        };
        let narration: Manifest = serde_json::from_slice(&read("narration/narration.json")?)?;
        let tempo: Manifest = serde_json::from_slice(&read("audio/tempo/tempo.json")?)?;
        let beats: Beats = serde_json::from_slice(&read("audio/beats.json")?)?;
        let stems: Stems = serde_json::from_slice(&read("audio/manifest.json")?)?;
        let mut lengths = stems
            .stems
            .into_iter()
            .map(|stem| (stem.file, stem.duration_nanos))
            .collect::<HashMap<_, _>>();
        for (name, length) in REUSED {
            lengths.insert(format!("../../assets/{name}.wav"), seconds(length));
        }
        for clip in &narration.clips {
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
                .with_context(|| format!("audio/beats.json has no clip '{id}'"))?;
            ensure!(
                beat.text_hash == original.text_hash
                    && faster.is_none_or(|clip| clip.text_hash == original.text_hash),
                "clip '{id}' changed: rerun audio/beats.ts and audio/tempo.ts"
            );
            let (file, duration, tempo) = match faster {
                Some(clip) => (clip.file.clone(), clip.duration_nanos, clip.tempo),
                None => (
                    format!("narration/{}", original.file),
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
                transcript: Transcript::load(&root.join("narration").join(&original.words))?,
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
        let another = next(&hush, "another", 0.3)?;
        let chant = next(&another, "chant", 0.25)?;
        let theory = next(&chant, "theory", 0.15)?;
        let andy = next(&theory, "andy", 0.12)?;
        let spaghetti = next(&andy, "spaghetti", 0.08)?;
        let chaos = spaghetti.end_of("will smith") + seconds(0.42);
        let cut = chaos + seconds(MONTAGE.iter().sum());
        let implosion = cut - seconds(MONTAGE[MONTAGE.len() - 1] - HIT);
        let exhale = cut + seconds(SILENCE);
        let mut after = voice("after", 0)?;
        after.start = exhale + seconds(EXHALE.1 - EXHALE.0 + 0.15 - WORDS_FROM);
        let chain = after.voice_end() + seconds(0.25);
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
            cut,
            end,
            heartbeats: Vec::new(),
            sounds: Vec::new(),
            lengths,
            exhale,
            implosion,
            chain,
            title,
        };
        film.score();
        Ok(film)
    }

    fn length(&self, file: &str) -> u64 {
        *self
            .lengths
            .get(file)
            .unwrap_or_else(|| panic!("no length for {file}; is it in audio/manifest.json?"))
    }

    fn speak(&mut self, voice: &str, gain_db: f32) {
        let voice = match voice {
            "hush" => &self.hush,
            "another" => &self.another,
            "chant" => &self.chant,
            "theory" => &self.theory,
            "andy" => &self.andy,
            "spaghetti" => &self.spaghetti,
            _ => &self.after,
        };
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

    /// Place `file` at `at`, playing `from` (seconds into it) for at most
    /// `until` on the film clock.
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
        // ── One ball, whispered ──
        self.speak("hush", -2.0);
        let rest = self
            .layer("room-tone-a", "audio/sfx/room-tone.flac", 0, -26.0)
            .to;
        let chant_start = self.chant.voice_start();
        self.layer("room-tone-b", "audio/sfx/room-tone.flac", rest, -26.0)
            .to = (chant_start + seconds(0.3)).saturating_sub(rest);
        self.layer(
            "mic-rustle-open",
            "audio/sfx/mic-rustle.flac",
            seconds(0.05),
            -16.0,
        );
        let first = self.hush.at("balls");
        self.layer(
            "ignite",
            "audio/sfx/ignite.flac",
            first.saturating_sub(seconds(0.05)),
            -6.0,
        );
        self.layer("sparkle-ball-1", SPARKLE, first, -18.0);
        let dots = self.hush.at("dots");
        self.layer("sparkle-dots", SPARKLE, dots, -22.0);

        // ── Another ball, and the heart starts ──
        self.speak("another", -2.0);
        let turn = self.another.start.saturating_sub(seconds(0.3));
        self.layer("mic-rustle-turn", "audio/sfx/mic-rustle.flac", turn, -18.0);
        let another = self.another.at("another");
        self.layer("pop-ball-2", "audio/sfx/pop-01.flac", another, -10.0);
        self.layer("drone-swell", "audio/sfx/drone-swell.flac", another, -14.0);
        let more = self.another.at("more dots");
        self.layer("sparkle-more", SPARKLE, more, -16.0);

        // ── The chant: one ball per "balls", a semitone higher each time ──
        self.speak("chant", 0.0);
        for (index, (beat, _)) in self.chant.beats().into_iter().enumerate() {
            let pop = format!("audio/sfx/pop-{:02}.flac", (index + 2).min(15));
            self.layer(
                &format!("pop-chant-{}", index + 1),
                &pop,
                beat,
                -11.0 + index as f32 * 0.5,
            );
        }
        // A drum under every "balls", growing.
        for (index, (beat, _)) in self.chant.beats().into_iter().enumerate() {
            let gain = -20.0 + 1.0 * index as f32;
            self.layer(
                &format!("chant-tom-{index}"),
                "audio/sfx/hit-tom.flac",
                beat,
                gain,
            );
        }
        // The heart accelerates from "another" to the end of the chant.
        let (from, to) = (another + seconds(0.6), self.chant.end());
        let mut time = from;
        while time < to {
            self.heartbeats.push(time);
            let progress = (time - from) as f64 / (to - from) as f64;
            time += seconds(1.0 + (0.36 - 1.0) * progress);
        }
        for (index, thump) in self.heartbeats.clone().into_iter().enumerate() {
            self.layer(
                &format!("heartbeat-{index}"),
                "audio/sfx/heartbeat.flac",
                thump - seconds(THUMP),
                -10.0,
            );
        }

        // ── The theory, rising into ANDY SERKIS ──
        self.speak("theory", 0.0);
        let andy = self.andy.voice_start();
        let theory = self.theory.start;
        self.layer(
            "heartbeat-race",
            "audio/sfx/heartbeat-race.flac",
            theory,
            -14.0,
        )
        .to = andy - theory;
        self.layer(
            "drone",
            "audio/sfx/drone.flac",
            another + seconds(15.0),
            -14.0,
        );
        self.layer(
            "zap-atoms",
            "audio/sfx/zap.flac",
            self.theory.at("represent"),
            -14.0,
        );
        for (index, phrase) in ["thoughts", "dreams"].into_iter().enumerate() {
            let at = self.theory.at(phrase);
            self.layer(
                &format!("sparkle-{phrase}"),
                SPARKLE,
                at,
                -15.0 + index as f32,
            );
        }
        let rewind = self.theory.at_after("balls with dots", "dreams");
        self.layer("glitch-dots", "audio/sfx/glitch.flac", rewind, -12.0);
        for (index, at) in self.whips().into_iter().enumerate() {
            self.layer(&format!("whip-{index}"), WHOOSH, at, -12.0);
        }
        self.layer(
            "riser",
            "audio/sfx/riser.flac",
            andy - seconds(RISER_PEAK),
            -10.0,
        );

        // ── Andy Serkis in a suit of balls ──
        self.speak("andy", 1.0);
        self.layer("impact-andy", "audio/sfx/impact.flac", andy, -2.0);
        self.layer("glitch-andy", "audio/sfx/glitch.flac", andy, -10.0);
        let cover = self.andy.at("balls with dots");
        self.layer("sparkle-cover", SPARKLE, cover, -14.0);
        self.layer(
            "flyby-anything",
            "audio/sfx/flyby.flac",
            self.andy.at("anything"),
            -10.0,
        );
        let chimp = self.andy.end_of("chimp");
        self.layer("chimp-cameo", "narration/chaos-chimp.mp3", chimp, -14.0)
            .to = seconds(0.75);

        // ── Spaghetti, Will Smith ──
        self.speak("spaghetti", 1.0);
        self.layer(
            "drone-late",
            "audio/sfx/drone.flac",
            another + seconds(30.0),
            -14.0,
        );
        for phrase in ["lasagna", "spaghetti"] {
            let at = self.spaghetti.at(phrase);
            self.layer(&format!("whoosh-{phrase}"), WHOOSH, at, -10.0);
        }
        self.layer(
            "zap-woo",
            "audio/sfx/zap.flac",
            self.spaghetti.at("woo"),
            -13.0,
        );
        let eaten = self.spaghetti.at("eaten");
        self.layer(
            "whoosh-fork",
            WHOOSH,
            eaten.saturating_sub(seconds(0.35)),
            -9.0,
        );
        let smith = self.spaghetti.at("will smith");
        self.layer("impact-smith", "audio/sfx/impact.flac", smith, -7.0);
        for (index, (at, strength)) in self.slams().into_iter().enumerate() {
            self.layer(&format!("slam-{index}"), IMPACT, at, -14.0 + 5.0 * strength);
            self.layer(
                &format!("slam-taiko-{index}"),
                "audio/sfx/hit-taiko.flac",
                at,
                -16.0 + 6.0 * strength,
            );
            if strength >= 0.8 {
                self.layer(
                    &format!("slam-sub-{index}"),
                    "audio/sfx/sub-drop.flac",
                    at,
                    -15.0,
                );
            }
        }

        // ── Every voice at once ──
        let cut = self.cut;
        let implosion = self.implosion;
        self.layer(
            "intro-riser",
            RISER,
            implosion - seconds(REUSED[0].1),
            -12.0,
        );
        let chaos = self.chaos;
        let voices: [(&str, &str, f64, f32, f64); 16] = [
            // id, file, offset after the chaos cue, gain, source start
            ("babble", "narration/chaos-babble.mp3", 0.0, -7.0, 0.0),
            ("yowl", "narration/chaos-yowl.mp3", 0.1, -6.0, 0.0),
            ("laugh", "narration/chaos-laugh.mp3", 0.2, -8.0, 0.0),
            (
                "babble-chipmunk",
                "audio/voices/babble-chipmunk.mp3",
                0.3,
                -11.0,
                0.0,
            ),
            ("yowl-demon", "audio/voices/yowl-demon.mp3", 0.42, -8.0, 0.0),
            (
                "laugh-demon",
                "audio/voices/laugh-demon.mp3",
                0.52,
                -8.0,
                0.0,
            ),
            ("howl", "narration/chaos-howl.mp3", 0.62, -7.0, 0.0),
            (
                "spaghetti-demon",
                "audio/voices/spaghetti-demon.mp3",
                0.72,
                -9.0,
                3.5,
            ),
            (
                "babble-demon",
                "audio/voices/babble-demon.mp3",
                0.82,
                -9.0,
                0.0,
            ),
            (
                "laugh-chipmunk",
                "audio/voices/laugh-chipmunk.mp3",
                0.95,
                -12.0,
                0.0,
            ),
            ("howl-low", "audio/voices/howl-low.mp3", 1.05, -9.0, 0.0),
            ("chimp", "narration/chaos-chimp.mp3", 1.18, -9.0, 0.0),
            (
                "theory-reversed",
                "audio/voices/theory-reversed.mp3",
                1.3,
                -13.0,
                0.0,
            ),
            (
                "chant-demon",
                "audio/voices/chant-demon.mp3",
                1.42,
                -9.0,
                4.1,
            ),
            (
                "spaghetti-chipmunk",
                "audio/voices/spaghetti-chipmunk.mp3",
                1.55,
                -12.0,
                5.8,
            ),
            (
                "babble-reversed",
                "audio/voices/babble-reversed.mp3",
                1.68,
                -12.0,
                0.0,
            ),
        ];
        for (id, file, offset, gain, from) in voices {
            let sound = self.layer(&format!("chaos-{id}"), file, chaos + seconds(offset), gain);
            sound.from = seconds(from);
        }
        // ── Tribal drums: a ritual pulse under the theory, war drums under
        // Andy and the spaghetti, and a frenzy under every voice at once ──
        let theory_start = self.theory.voice_start();
        let second = self.theory.at_after("thoughts", "dreams");
        self.layer(
            "drum-pulse-a",
            "audio/sfx/drum-pulse.flac",
            theory_start,
            -2.0,
        );
        self.layer("drum-pulse-b", "audio/sfx/drum-pulse.flac", second, 0.0);
        self.layer("drum-war-a", "audio/sfx/drum-war.flac", andy, -7.0);
        let lasagna = self.spaghetti.at("lasagna");
        self.layer("drum-war-b", "audio/sfx/drum-war.flac", lasagna, -4.0);
        for (index, offset) in [0.0, 1.45, 2.4].into_iter().enumerate() {
            let at = chaos + seconds(offset);
            self.layer(
                &format!("drum-frenzy-{index}"),
                "audio/sfx/drum-frenzy.flac",
                at,
                -4.0 + index as f32,
            );
        }
        // Everything stops on the colossal hit but the hit itself, and it
        // stops at the cut.
        self.layer(
            "drum-colossal",
            "audio/sfx/drum-colossal.flac",
            implosion,
            0.0,
        );
        for sound in &mut self.sounds {
            let stop = if sound.id == "drum-colossal" {
                cut
            } else {
                implosion
            };
            if !sound.script && sound.at < stop && sound.at + (sound.to - sound.from) > stop {
                sound.to = sound.from + (stop - sound.at);
            }
        }
        self.sounds
            .retain(|sound| sound.script || sound.id == "drum-colossal" || sound.at < implosion);

        // ── Silence; an exhale; tenderness; and the pull back ──
        let exhale = self.exhale;
        let file = self.after.file.clone();
        let breath = self.layer("after-exhale", &file, exhale, -3.0);
        breath.from = seconds(EXHALE.0);
        breath.to = seconds(EXHALE.1);
        let words = self.after.start + seconds(WORDS_FROM);
        let sound = self.layer("narration-after", &file, words, -3.0);
        sound.from = seconds(WORDS_FROM);
        sound.script = true;
        let chain = self.chain;
        self.layer(
            "shimmer",
            "audio/sfx/shimmer.flac",
            chain.saturating_sub(seconds(0.1)),
            -13.0,
        );
        let title = self.title;
        self.layer("drum-soft", "audio/sfx/drum-soft.flac", title, -5.0);
        self.sounds.sort_by_key(|sound| sound.at);
    }

    /// Mark every cut with a sound of its kind: a dive is sucked in, a whip
    /// swishes, a glitch stutters, a flash booms; montage cuts are drum hits.
    pub fn accent(&mut self, cuts: &[(u64, Accent)]) {
        let (stop, cut) = (self.implosion, self.cut);
        for (index, (at, accent)) in cuts.iter().copied().enumerate() {
            let id = |name: &str| format!("cut-{index}-{name}");
            match accent {
                Accent::Dive => {
                    let suck = self.length("audio/sfx/suck.flac");
                    self.layer(
                        &id("suck"),
                        "audio/sfx/suck.flac",
                        at.saturating_sub(suck),
                        -9.0,
                    );
                    self.layer(&id("taiko"), "audio/sfx/hit-taiko.flac", at, -10.0);
                }
                Accent::Whip => {
                    self.layer(
                        &id("swish"),
                        "audio/sfx/swish.flac",
                        at.saturating_sub(seconds(0.06)),
                        -8.0,
                    );
                }
                Accent::Glitch => {
                    self.layer(&id("glitch"), "audio/sfx/glitch.flac", at, -12.0)
                        .to = seconds(0.4);
                }
                Accent::Flash => {
                    self.layer(&id("sub"), "audio/sfx/sub-drop.flac", at, -10.0);
                }
                Accent::Montage(beat) => {
                    let (file, gain) = match beat % 3 {
                        0 => ("audio/sfx/hit-taiko.flac", -5.0),
                        1 => ("audio/sfx/hit-tom.flac", -6.0),
                        _ => ("audio/sfx/hit-taiko.flac", -7.0),
                    };
                    self.layer(&id("drum"), file, at, gain);
                    self.layer(
                        &id("slap"),
                        "audio/sfx/hit-slap.flac",
                        at + seconds(0.11),
                        -11.0,
                    );
                    self.layer(
                        &id("swish"),
                        "audio/sfx/swish.flac",
                        at.saturating_sub(seconds(0.05)),
                        -14.0,
                    );
                    let pop = format!("audio/sfx/pop-{:02}.flac", 4 + (beat * 7) % 12);
                    self.layer(&id("pop"), &pop, at + seconds(0.06), -15.0);
                }
            }
        }
        // Risers into the theory and into the climax.
        let theory = self.theory.voice_start();
        let riser = self.length("audio/sfx/riser-short.flac");
        self.layer(
            "riser-theory",
            "audio/sfx/riser-short.flac",
            theory.saturating_sub(riser),
            -12.0,
        );
        self.layer(
            "riser-chaos",
            "audio/sfx/riser-short.flac",
            self.chaos.saturating_sub(riser),
            -11.0,
        );
        for sound in &mut self.sounds {
            if !sound.script
                && sound.id != "drum-colossal"
                && sound.at < stop
                && sound.at + (sound.to - sound.from) > stop
            {
                sound.to = sound.from + (stop - sound.at);
            }
        }
        self.sounds.retain(|sound| {
            sound.script || sound.id == "drum-colossal" || sound.at < stop || sound.at >= cut
        });
        self.sounds.sort_by_key(|sound| sound.at);
    }

    /// The theory's whip pans, each landing on its claim's subject.
    pub fn whips(&self) -> [u64; 3] {
        [
            self.theory.at("thoughts"),
            self.theory.at_after("thoughts", "dreams"),
            self.theory.at("dots represent basketballs"),
        ]
        .map(|at| at.saturating_sub(seconds(WHIP)))
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
            (andy.at("shriveled"), 0.6),
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
            let end = (sound.at + (sound.to - sound.from)).min(until);
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
