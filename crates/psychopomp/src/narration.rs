//! Narration clips: exact durations plus word timings, so choreography is
//! keyed to what is said rather than to seconds. `Narration::load` reads the
//! manifests `scripts/narrate.ts` writes (clips at `narration/<file>`);
//! `psychopomp-media` builds clips from Generated Resources with
//! [`NarrationClip::new`]. A placed clip is a Script Clip whose phrase lookups
//! panic with the clip and phrase when the narration no longer says them.
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

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
#[derive(Clone, Debug)]
pub struct NarrationClip {
    id: String,
    path: PathBuf,
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
                        path: format!("narration/{}", clip.file).into(),
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
    /// A clip of the audio at `path` (relative to the plan file), lasting
    /// `duration` nanoseconds, whose words are `transcript`.
    pub fn new(
        id: impl Into<String>,
        path: impl Into<PathBuf>,
        duration: u64,
        transcript: Transcript,
    ) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            duration,
            transcript,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn duration(&self) -> u64 {
        self.duration
    }

    pub fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    /// Place this clip in a plan, starting at `start` on the plan clock.
    pub fn place(&self, scene: &mut PlanBuilder, start: u64) -> Spoken<'_> {
        scene.media(MediaPlan {
            id: format!("narration-{}", self.id),
            path: self.path.clone(),
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

    /// When every occurrence of `phrase` starts, in order: one beat per word
    /// of a chant. Panics like [`at`](Self::at) when it is never said.
    pub fn words(&self, phrase: &str) -> Vec<u64> {
        self.clip
            .transcript
            .phrases(phrase)
            .unwrap_or_else(|error| panic!("clip '{}': {error:#}", self.clip.id))
            .iter()
            .map(|cue| self.start + cue.start().as_nanos())
            .collect()
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
