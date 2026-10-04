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

    /// The clips named `clips` read one after another; see [`Reading::new`].
    pub fn reading<const N: usize>(
        &self,
        lead: u64,
        clips: [(&str, u64); N],
    ) -> Result<Reading<'_, N>> {
        let mut resolved = Vec::with_capacity(N);
        for (id, gap) in clips {
            resolved.push((self.clip(id)?, gap));
        }
        Ok(Reading::new(
            lead,
            resolved.try_into().unwrap_or_else(|_| unreachable!()),
        ))
    }
}

/// Narration clips scheduled back to back with gaps; see [`Reading::new`].
pub struct Reading<'a, const N: usize> {
    clips: [(&'a NarrationClip, u64); N],
    duration: u64,
}

impl<'a, const N: usize> Reading<'a, N> {
    /// Clips read one after another: `lead` of silence, then each clip
    /// followed by its gap (the last gap is the tail). The reading knows the
    /// scene's duration before the scene exists; place it once the scene does.
    pub fn new(lead: u64, clips: [(&'a NarrationClip, u64); N]) -> Self {
        let mut at = lead;
        let clips = clips.map(|(clip, gap)| {
            let start = at;
            at += clip.duration + gap;
            (clip, start)
        });
        Self {
            clips,
            duration: at,
        }
    }

    /// From time zero through the last clip's gap: the scene's duration.
    pub fn duration(&self) -> u64 {
        self.duration
    }

    /// Place every clip as a Script Clip, in order.
    pub fn place(&self, scene: &mut PlanBuilder) -> [Spoken<'a>; N] {
        self.clips.map(|(clip, start)| clip.place(scene, start))
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
        self.place_range(scene, start, 0, self.duration, "")
    }

    /// Place only the source range `from..to` at `start`, as when a clip is
    /// [split](Self::split) across two reel segments; `suffix` keeps the media
    /// ID unique. Phrase lookups still search the whole transcript, mapped
    /// onto this placement's clock.
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
            path: self.path.clone(),
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
            start,
            end: start + (to - from),
        }
    }

    /// A split point at `seconds` of source time, checked to fall after
    /// `earlier` ends and before `later` starts. Word timings can place a
    /// word's start well inside the previous word's tail, so the point itself
    /// is measured from the audio; the check catches a re-voiced clip.
    pub fn split(&self, seconds: f64, earlier: &str, later: &str) -> u64 {
        let at = crate::author::seconds(seconds);
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
    clip: &'a NarrationClip,
    /// The plan time of source time zero (negative for a late range).
    origin: i64,
    start: u64,
    end: u64,
}

impl Spoken<'_> {
    /// When this placement starts on the plan clock.
    pub fn start(&self) -> u64 {
        self.start
    }

    pub fn end(&self) -> u64 {
        self.end
    }

    /// The plan time of a source time, never before the plan starts.
    fn plan_time(&self, source: u64) -> u64 {
        (self.origin + source as i64).max(0) as u64
    }

    /// Every word this placement speaks (for a [range](NarrationClip::place_range),
    /// those that overlap it) with its start and end on the plan clock, as
    /// subtitles show them.
    pub fn words(&self) -> impl Iterator<Item = (&str, u64, u64)> + '_ {
        let nanos = |seconds: f64| (seconds * 1e9).round() as u64;
        let from = (self.start as i64 - self.origin) as u64;
        let to = from + (self.end - self.start);
        self.clip
            .transcript
            .words()
            .iter()
            .filter(move |timing| from == 0 || nanos(timing.end) > from)
            .filter(move |timing| to == self.clip.duration || nanos(timing.start) < to)
            .map(move |timing| {
                (
                    timing.word.as_str(),
                    self.plan_time(nanos(timing.start)),
                    self.plan_time(nanos(timing.end)),
                )
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
            .map(|cue| self.plan_time(cue.start().as_nanos()))
            .unwrap_or_else(|| {
                panic!(
                    "clip '{}': transcript contains none of {phrases:?}",
                    self.clip.id
                )
            })
    }

    /// When every occurrence of `phrase` starts, in order: one beat per word
    /// of a chant. Panics like [`at`](Self::at) when it is never said.
    pub fn at_every(&self, phrase: &str) -> Vec<u64> {
        self.clip
            .transcript
            .phrases(phrase)
            .unwrap_or_else(|error| panic!("clip '{}': {error:#}", self.clip.id))
            .iter()
            .map(|cue| self.plan_time(cue.start().as_nanos()))
            .collect()
    }

    /// `phrase`, searching only after `earlier` is said.
    pub fn at_after(&self, phrase: &str, earlier: &str) -> u64 {
        let from = self.source_cue(earlier, 0.0);
        self.cue(phrase, from as f64 / 1e9)
    }

    fn cue(&self, phrase: &str, after_seconds: f64) -> u64 {
        self.plan_time(self.source_cue(phrase, after_seconds))
    }

    /// When `phrase` starts, in source time.
    fn source_cue(&self, phrase: &str, after_seconds: f64) -> u64 {
        self.clip
            .transcript
            .phrase_after(phrase, after_seconds)
            .unwrap_or_else(|error| panic!("clip '{}': {error:#}", self.clip.id))
            .start()
            .as_nanos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{author::SECOND, transcript::WordTiming};

    fn clip(id: &str, seconds: u64, words: &[(&str, f64, f64)]) -> (String, NarrationClip) {
        let words = words
            .iter()
            .map(|&(word, start, end)| WordTiming {
                word: word.into(),
                start,
                end,
            })
            .collect();
        (
            id.into(),
            NarrationClip {
                id: id.into(),
                path: format!("narration/{id}.mp3").into(),
                duration: seconds * SECOND,
                transcript: Transcript::new(words).unwrap(),
            },
        )
    }

    fn narration() -> Narration {
        Narration {
            clips: HashMap::from([
                clip("intro", 2, &[("hello", 0.5, 0.9), ("world", 1.2, 1.5)]),
                clip("outro", 3, &[("bye", 1.0, 1.4), ("now", 2.0, 2.3)]),
            ]),
        }
    }

    #[test]
    fn a_reading_places_clips_back_to_back_with_their_gaps() {
        let narration = narration();
        let reading = narration
            .reading(SECOND, [("intro", SECOND / 2), ("outro", 2 * SECOND)])
            .unwrap();
        assert_eq!(
            reading.duration(),
            (1 + 2) * SECOND + SECOND / 2 + (3 + 2) * SECOND
        );
        let mut scene = PlanBuilder::new("reading", reading.duration());
        let [intro, outro] = reading.place(&mut scene);
        assert_eq!(intro.at("world"), SECOND + 1_200_000_000);
        assert_eq!(intro.end(), 3 * SECOND);
        assert_eq!(outro.at("bye"), 3 * SECOND + SECOND / 2 + SECOND);
        let plan = scene.finish().unwrap();
        let starts = plan
            .media
            .iter()
            .map(|media| (media.id.as_str(), media.timeline_start_nanos))
            .collect::<Vec<_>>();
        assert_eq!(
            starts,
            [
                ("narration-intro", SECOND),
                ("narration-outro", 3 * SECOND + SECOND / 2)
            ]
        );
        assert!(narration.reading(0, [("missing", 0)]).is_err());
    }

    #[test]
    fn a_split_clip_keeps_its_words_on_the_plan_clock() {
        let narration = narration();
        let outro = narration.clip("outro").unwrap();
        let split = outro.split(1.7, "bye", "now");
        let mut scene = PlanBuilder::new("split", 10 * SECOND);
        let first = outro.place_range(&mut scene, 0, 0, split, "");
        let second = outro.place_range(&mut scene, SECOND / 2, split, outro.duration(), "-rest");
        assert_eq!(first.at("bye"), SECOND);
        assert_eq!(first.end(), 1_700_000_000);
        assert_eq!(second.at("now"), 800_000_000);
        assert_eq!(
            second.at("bye"),
            0,
            "a word before the range clamps to zero"
        );
        assert_eq!(second.end(), 1_800_000_000);
        assert_eq!(
            first.words().collect::<Vec<_>>(),
            [("bye", SECOND, 1_400_000_000)]
        );
        assert_eq!(
            second.words().collect::<Vec<_>>(),
            [("now", 800_000_000, 1_100_000_000)]
        );
        let plan = scene.finish().unwrap();
        assert_eq!(plan.media[1].id, "narration-outro-rest");
        assert_eq!(plan.media[1].source_start_nanos, 1_700_000_000);
    }

    #[test]
    #[should_panic(expected = "re-measure it")]
    fn a_split_between_the_wrong_words_panics() {
        narration().clip("outro").unwrap().split(0.5, "bye", "now");
    }
}
