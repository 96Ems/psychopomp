//! Narration clips produced by `scripts/narrate.ts`: exact durations plus word
//! timings, so choreography is keyed to what is said rather than to seconds.
//! A clip can also be split at a pause and placed in two segments, so a zoom
//! into the code happens mid-sentence without adding silence.
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

    /// Place this whole clip in a plan, starting at `start` on the plan clock.
    pub fn place(&self, scene: &mut PlanBuilder, start: u64) -> Spoken<'_> {
        self.place_range(scene, start, 0, self.duration, "")
    }

    /// Place the source range `from..to` at `start`. Phrase lookups still use
    /// the whole transcript, mapped onto this placement's clock.
    pub fn place_range(
        &self,
        scene: &mut PlanBuilder,
        start: u64,
        from: u64,
        to: u64,
        suffix: &str,
    ) -> Spoken<'_> {
        scene.media(MediaPlan {
            id: format!("narration-{}{suffix}", self.id),
            path: format!("narration/{}", self.file).into(),
            kind: MediaKindPlan::Audio,
            role: MediaRolePlan::Script,
            source_start_nanos: from,
            source_end_nanos: to,
            timeline_start_nanos: start,
            timeline_end_nanos: start + (to - from),
            gain_db: 0.0,
        });
        Spoken {
            clip: self,
            origin: start as i64 - from as i64,
            end: start + (to - from),
        }
    }

    /// A split point at `seconds` of source time, checked to fall after
    /// `earlier` ends and before `later` starts. Word timings can place a
    /// word's start well inside the previous word's tail, so the point itself
    /// is measured from the audio; the check catches a re-voiced clip.
    pub fn split(&self, seconds: f64, earlier: &str, later: &str) -> u64 {
        let at = (seconds * 1e9).round() as u64;
        let cue = |phrase: &str| {
            self.transcript
                .phrase_after(phrase, 0.0)
                .unwrap_or_else(|error| panic!("clip '{}': {error:#}", self.id))
        };
        let (after, before) = (cue(earlier).end().as_nanos(), cue(later).start().as_nanos());
        assert!(
            after <= at && at < before,
            "clip '{}': split {seconds}s is not between '{earlier}' and '{later}'; re-measure it",
            self.id
        );
        at
    }
}

/// A placed clip: phrase lookups return plan-clock times.
pub struct Spoken<'a> {
    clip: &'a Clip,
    /// The plan time of source time zero (negative for a late range).
    origin: i64,
    end: u64,
}

impl Spoken<'_> {
    pub fn end(&self) -> u64 {
        self.end
    }

    /// When `phrase` starts being spoken. Panics with the clip and phrase if the
    /// narration no longer says it: the choreography must be updated with the words.
    pub fn at(&self, phrase: &str) -> u64 {
        self.cue(phrase, 0.0)
    }

    fn cue(&self, phrase: &str, after_seconds: f64) -> u64 {
        let cue = self
            .clip
            .transcript
            .phrase_after(phrase, after_seconds)
            .unwrap_or_else(|error| panic!("clip '{}': {error:#}", self.clip.id));
        (self.origin + cue.start().as_nanos() as i64).max(0) as u64
    }
}
