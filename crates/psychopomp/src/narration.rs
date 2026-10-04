//! Narration clips produced by `scripts/narrate.ts`: exact durations plus word
//! timings, so choreography is keyed to what is said rather than to seconds.
//! A placed clip is a Script Clip at `narration/<file>`; its phrase lookups
//! panic with the clip and phrase when the narration no longer says them.
use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::{
    author::PlanBuilder,
    plan::{MediaKindPlan, MediaPlan, MediaRolePlan},
    transcript::Transcript,
};

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
    words: String,
    duration_nanos: u64,
}

/// Every clip in a narration directory's `narration.json`.
pub struct Narration {
    clips: HashMap<String, NarrationClip>,
}

/// One voiced clip and its word timings.
pub struct NarrationClip {
    id: String,
    file: String,
    duration: u64,
    transcript: Transcript,
}

impl Narration {
    pub fn load(dir: &Path) -> Result<Self> {
        let manifest: Manifest =
            serde_json::from_slice(&fs::read(dir.join("narration.json")).with_context(|| {
                format!(
                    "read {}/narration.json; run scripts/narrate.ts",
                    dir.display()
                )
            })?)?;
        let clips = manifest
            .clips
            .into_iter()
            .map(|clip| {
                let transcript = Transcript::load(&dir.join(&clip.words))?;
                Ok((
                    clip.id.clone(),
                    NarrationClip {
                        id: clip.id,
                        file: clip.file,
                        duration: clip.duration_nanos,
                        transcript,
                    },
                ))
            })
            .collect::<Result<_>>()?;
        Ok(Self { clips })
    }

    pub fn clip(&self, id: &str) -> Result<&NarrationClip> {
        self.clips
            .get(id)
            .with_context(|| format!("narration has no clip '{id}'"))
    }
}

impl NarrationClip {
    pub fn duration(&self) -> u64 {
        self.duration
    }

    /// Place this clip in a plan, starting at `start` on the plan clock.
    pub fn place(&self, scene: &mut PlanBuilder, start: u64) -> Spoken<'_> {
        scene.media(MediaPlan {
            id: format!("narration-{}", self.id),
            path: format!("narration/{}", self.file).into(),
            kind: MediaKindPlan::Audio,
            role: MediaRolePlan::Script,
            source_start_nanos: 0,
            source_end_nanos: self.duration,
            timeline_start_nanos: start,
            timeline_end_nanos: start + self.duration,
            gain_db: 0.0,
        });
        Spoken { clip: self, start }
    }
}

/// A placed clip: phrase lookups return plan-clock times.
pub struct Spoken<'a> {
    clip: &'a NarrationClip,
    start: u64,
}

impl Spoken<'_> {
    pub fn end(&self) -> u64 {
        self.start + self.clip.duration
    }

    /// Every spoken word with its start and end on the plan clock, as
    /// subtitles show them.
    pub fn words(&self) -> impl Iterator<Item = (&str, u64, u64)> + '_ {
        self.clip.transcript.words().iter().map(|timing| {
            let at = |seconds: f64| self.start + (seconds * 1e9).round() as u64;
            (timing.word.as_str(), at(timing.start), at(timing.end))
        })
    }

    /// When `phrase` starts being spoken. Panics with the clip and phrase if the
    /// narration no longer says it: the choreography must be updated with the words.
    pub fn at(&self, phrase: &str) -> u64 {
        self.cue(phrase, 0.0)
    }

    /// The first of several spellings or segmentations speech recognition may
    /// produce, such as `["sig term", "sigterm"]`.
    pub fn at_any(&self, phrases: &[&str]) -> u64 {
        phrases
            .iter()
            .find_map(|phrase| self.clip.transcript.phrase_after(phrase, 0.0).ok())
            .map(|cue| self.start + cue.start().as_nanos())
            .unwrap_or_else(|| {
                panic!(
                    "clip '{}': transcript contains none of {phrases:?}",
                    self.clip.id
                )
            })
    }

    /// `phrase`, searching only after `earlier` is said.
    pub fn at_after(&self, phrase: &str, earlier: &str) -> u64 {
        let from = self.cue(earlier, 0.0) - self.start;
        self.cue(phrase, from as f64 / 1e9)
    }

    fn cue(&self, phrase: &str, after_seconds: f64) -> u64 {
        let cue = self
            .clip
            .transcript
            .phrase_after(phrase, after_seconds)
            .unwrap_or_else(|error| panic!("clip '{}': {error:#}", self.clip.id));
        self.start + cue.start().as_nanos()
    }
}
