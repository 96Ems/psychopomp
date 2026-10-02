//! Narration clips produced by `scripts/narrate.ts`: exact durations plus word
//! timings, so choreography is keyed to what is said rather than to seconds.
use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result};
use kinograph::{
    author::PlanBuilder,
    plan::{MediaKindPlan, MediaPlan, MediaRolePlan},
    transcript::Transcript,
};
use serde::Deserialize;

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

pub struct Narration {
    clips: HashMap<String, Clip>,
}

pub struct Clip {
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
                    Clip {
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

    pub fn clip(&self, id: &str) -> Result<&Clip> {
        self.clips
            .get(id)
            .with_context(|| format!("narration has no clip '{id}'"))
    }
}

impl Clip {
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
    clip: &'a Clip,
    start: u64,
}

impl Spoken<'_> {
    pub fn end(&self) -> u64 {
        self.start + self.clip.duration
    }

    /// When `phrase` starts being spoken. Panics with the clip and phrase if the
    /// narration no longer says it: the choreography must be updated with the words.
    pub fn at(&self, phrase: &str) -> u64 {
        self.cue(phrase, 0.0).0
    }

    fn cue(&self, phrase: &str, after_seconds: f64) -> (u64, u64) {
        let cue = self
            .clip
            .transcript
            .phrase_after(phrase, after_seconds)
            .unwrap_or_else(|error| panic!("clip '{}': {error:#}", self.clip.id));
        (
            self.start + cue.start().as_nanos(),
            self.start + cue.end().as_nanos(),
        )
    }
}
