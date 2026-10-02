use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};

use crate::dsl::{Annotation, Motion, TaskChange, TaskPoseChange};

const NANOS_PER_SECOND: u64 = 1_000_000_000;
const MAX_SECONDS: f64 = u64::MAX as f64 / NANOS_PER_SECOND as f64;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Time(u64);

impl Time {
    pub const ZERO: Self = Self(0);

    pub fn seconds(value: f64) -> Self {
        Self::try_seconds(value).expect("time must be finite, non-negative, and representable")
    }

    pub fn try_seconds(value: f64) -> Option<Self> {
        (value.is_finite() && (0.0..=MAX_SECONDS).contains(&value))
            .then(|| Self((value * NANOS_PER_SECOND as f64).round() as u64))
    }

    pub fn as_seconds(self) -> f64 {
        self.0 as f64 / NANOS_PER_SECOND as f64
    }

    pub fn from_nanos(value: u64) -> Self {
        Self(value)
    }

    pub fn as_nanos(self) -> u64 {
        self.0
    }

    pub fn after(self, duration: Duration) -> Self {
        Self(
            self.0
                .checked_add(duration.0)
                .expect("timeline time overflowed"),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd)]
pub struct Duration(u64);

impl Duration {
    pub const ZERO: Self = Self(0);

    pub fn seconds(value: f64) -> Self {
        Self::try_seconds(value).expect("duration must be finite, non-negative, and representable")
    }

    pub fn try_seconds(value: f64) -> Option<Self> {
        (value.is_finite() && (0.0..=MAX_SECONDS).contains(&value))
            .then(|| Self((value * NANOS_PER_SECOND as f64).round() as u64))
    }

    pub fn milliseconds(value: f64) -> Self {
        Self::seconds(value / 1_000.0)
    }

    pub fn as_seconds(self) -> f64 {
        self.0 as f64 / NANOS_PER_SECOND as f64
    }

    pub fn from_nanos(value: u64) -> Self {
        Self(value)
    }

    pub fn frame_count(self, frames_per_second: u32) -> u64 {
        assert!(frames_per_second > 0, "frame rate must be positive");
        let scaled = u128::from(self.0) * u128::from(frames_per_second);
        u64::try_from(scaled.div_ceil(u128::from(NANOS_PER_SECOND)))
            .expect("frame count overflowed")
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TimeRange {
    start: Time,
    end: Time,
}

impl TimeRange {
    pub fn new(start: Time, end: Time) -> Self {
        assert!(end >= start, "time range must not end before it starts");
        Self { start, end }
    }

    pub fn start(self) -> Time {
        self.start
    }

    pub fn end(self) -> Time {
        self.end
    }

    pub fn duration(self) -> Duration {
        Duration(self.end.0 - self.start.0)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AssetId(String);

impl AssetId {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        assert!(!value.is_empty(), "asset ID must not be empty");
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A source audio file; clips select time ranges from it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Asset {
    id: AssetId,
    path: PathBuf,
}

impl Asset {
    pub fn audio(id: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            id: AssetId::new(id),
            path: path.into(),
        }
    }

    pub fn id(&self) -> &AssetId {
        &self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn clip(&self, source_range: TimeRange) -> Clip {
        Clip::new(self.clone(), source_range)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    asset: Asset,
    source_range: TimeRange,
    gain_db: f32,
}

impl Clip {
    pub fn new(asset: Asset, source_range: TimeRange) -> Self {
        assert!(
            source_range.duration() > Duration::ZERO,
            "clip source range must have positive duration"
        );
        Self {
            asset,
            source_range,
            gain_db: 0.0,
        }
    }

    pub fn gain_db(mut self, gain_db: f32) -> Self {
        assert!(gain_db.is_finite(), "clip gain must be finite");
        self.gain_db = gain_db;
        self
    }

    pub fn asset(&self) -> &Asset {
        &self.asset
    }

    pub fn source_range(&self) -> TimeRange {
        self.source_range
    }

    pub fn duration(&self) -> Duration {
        self.source_range.duration()
    }

    pub fn audio_gain_db(&self) -> f32 {
        self.gain_db
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaRole {
    /// Primary spoken media whose transcript can drive structural edits.
    Script,
    /// Accompanying music, sound effects, B-roll, and other timed media.
    Layer,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MediaPlacement {
    clip: Clip,
    role: MediaRole,
    timeline_range: TimeRange,
}

impl MediaPlacement {
    pub fn clip(&self) -> &Clip {
        &self.clip
    }

    pub fn role(&self) -> MediaRole {
        self.role
    }

    pub fn timeline_range(&self) -> TimeRange {
        self.timeline_range
    }

    /// The same clip placed `offset` later on a containing timeline, such as a
    /// segment's media on a reel clock. The source range is unchanged.
    pub fn shifted(&self, offset: Duration) -> Self {
        Self {
            clip: self.clip.clone(),
            role: self.role,
            timeline_range: TimeRange::new(
                self.timeline_range.start().after(offset),
                self.timeline_range.end().after(offset),
            ),
        }
    }

    pub fn for_window(&self, window: TimeRange) -> Option<Self> {
        let overlap_start = self.timeline_range.start().max(window.start());
        let overlap_end = self.timeline_range.end().min(window.end());
        if overlap_start >= overlap_end {
            return None;
        }
        let source_offset = overlap_start.as_nanos() - self.timeline_range.start().as_nanos();
        let overlap_duration = overlap_end.as_nanos() - overlap_start.as_nanos();
        let source_start = self
            .clip
            .source_range()
            .start()
            .after(Duration::from_nanos(source_offset));
        let clip = Clip {
            asset: self.clip.asset.clone(),
            source_range: TimeRange::new(
                source_start,
                source_start.after(Duration::from_nanos(overlap_duration)),
            ),
            gain_db: self.clip.gain_db,
        };
        let rebased_start = Time::from_nanos(overlap_start.as_nanos() - window.start().as_nanos());
        Some(Self {
            clip,
            role: self.role,
            timeline_range: TimeRange::new(
                rebased_start,
                rebased_start.after(Duration::from_nanos(overlap_duration)),
            ),
        })
    }
}

impl std::fmt::Display for Time {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}.{:09}",
            self.0 / NANOS_PER_SECOND,
            self.0 % NANOS_PER_SECOND
        )
    }
}

impl std::fmt::Display for Duration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}.{:09}",
            self.0 / NANOS_PER_SECOND,
            self.0 % NANOS_PER_SECOND
        )
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CueId(String);

impl CueId {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        assert!(!value.is_empty(), "cue ID must not be empty");
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
    id: CueId,
    range: TimeRange,
}

impl Cue {
    pub fn new(id: impl Into<String>, range: TimeRange) -> Self {
        Self {
            id: CueId::new(id),
            range,
        }
    }

    pub fn id(&self) -> &CueId {
        &self.id
    }

    pub fn start(&self) -> Time {
        self.range.start
    }

    pub fn end(&self) -> Time {
        self.range.end
    }

    pub fn start_offset(&self) -> Duration {
        Duration(self.start().0)
    }

    /// Places a composition at this cue's start relative to the containing
    /// composition's origin.
    pub fn at(&self, composition: impl Into<Composition>) -> Composition {
        Composition::delay(self.start_offset(), composition)
    }
}

#[derive(Clone, Debug)]
pub enum Composition {
    Animate(Motion),
    Annotate(Annotation),
    Task(TaskChange),
    TaskPose(TaskPoseChange),
    Play {
        clip: Clip,
        role: MediaRole,
    },
    Sequence(Vec<Composition>),
    Parallel(Vec<Composition>),
    Delay {
        duration: Duration,
        composition: Box<Composition>,
    },
    Hold(Duration),
    Named {
        id: CueId,
        composition: Box<Composition>,
    },
}

impl Composition {
    pub fn animate(motion: Motion) -> Self {
        Self::Animate(motion)
    }

    pub fn annotate(annotation: Annotation) -> Self {
        Self::Annotate(annotation)
    }

    pub fn task(change: TaskChange) -> Self {
        Self::Task(change)
    }

    pub fn task_pose(change: TaskPoseChange) -> Self {
        Self::TaskPose(change)
    }

    pub fn script(clip: Clip) -> Self {
        Self::Play {
            clip,
            role: MediaRole::Script,
        }
    }

    pub fn layer(clip: Clip) -> Self {
        Self::Play {
            clip,
            role: MediaRole::Layer,
        }
    }

    pub fn sequence(compositions: impl IntoIterator<Item = Composition>) -> Self {
        Self::Sequence(compositions.into_iter().collect())
    }

    pub fn parallel(compositions: impl IntoIterator<Item = Composition>) -> Self {
        Self::Parallel(compositions.into_iter().collect())
    }

    pub fn delay(duration: Duration, composition: impl Into<Composition>) -> Self {
        Self::Delay {
            duration,
            composition: Box::new(composition.into()),
        }
    }

    pub fn hold(duration: Duration) -> Self {
        Self::Hold(duration)
    }

    pub fn named(id: impl Into<String>, composition: impl Into<Composition>) -> Self {
        Self::Named {
            id: CueId::new(id),
            composition: Box::new(composition.into()),
        }
    }

    pub fn duration(&self) -> Duration {
        match self {
            Self::Animate(motion) => Duration::seconds(f64::from(motion.duration())),
            Self::Annotate(annotation) => annotation.duration(),
            Self::Task(_) | Self::TaskPose(_) => Duration::ZERO,
            Self::Play { clip, .. } => clip.duration(),
            Self::Sequence(compositions) => Duration(
                compositions
                    .iter()
                    .map(|composition| composition.duration().0)
                    .try_fold(0_u64, u64::checked_add)
                    .expect("composition duration overflowed"),
            ),
            Self::Parallel(compositions) => Duration(
                compositions
                    .iter()
                    .map(|composition| composition.duration().0)
                    .max()
                    .unwrap_or(0),
            ),
            Self::Delay {
                duration,
                composition,
            } => Duration(
                duration
                    .0
                    .checked_add(composition.duration().0)
                    .expect("composition duration overflowed"),
            ),
            Self::Hold(duration) => *duration,
            Self::Named { composition, .. } => composition.duration(),
        }
    }

    pub(crate) fn lower(&self) -> Result<LoweredComposition> {
        let mut scheduled = Scheduled::default();
        self.schedule(Time::ZERO, &mut scheduled)?;
        let duration = self.duration();
        let mut motions = scheduled
            .motions
            .into_iter()
            .map(|(start, motion)| Motion::delay(start.as_seconds() as f32, motion))
            .collect::<Vec<_>>();
        motions.push(Motion::hold(duration.as_seconds() as f32));
        Ok(LoweredComposition {
            motion: Motion::parallel(motions),
            media: scheduled.media,
            annotations: scheduled.annotations,
            tasks: scheduled.tasks,
            task_poses: scheduled.task_poses,
            duration,
        })
    }

    fn schedule(&self, start: Time, scheduled: &mut Scheduled) -> Result<()> {
        match self {
            Self::Animate(motion) => scheduled.motions.push((start, motion.clone())),
            Self::Annotate(annotation) => {
                scheduled.annotations.push((start, annotation.clone()));
            }
            Self::Task(change) => scheduled.tasks.push((start, change.clone())),
            Self::TaskPose(change) => scheduled.task_poses.push((start, change.clone())),
            Self::Play { clip, role } => scheduled.media.push(MediaPlacement {
                clip: clip.clone(),
                role: *role,
                timeline_range: TimeRange::new(start, start.after(clip.duration())),
            }),
            Self::Sequence(compositions) => {
                let mut cursor = start;
                for composition in compositions {
                    composition.schedule(cursor, scheduled)?;
                    cursor = cursor.after(composition.duration());
                }
            }
            Self::Parallel(compositions) => {
                for composition in compositions {
                    composition.schedule(start, scheduled)?;
                }
            }
            Self::Delay {
                duration,
                composition,
            } => composition.schedule(start.after(*duration), scheduled)?,
            Self::Hold(_) => {}
            Self::Named { id, composition } => {
                if !scheduled.cues.insert(id.clone()) {
                    bail!("composition defines cue '{}' more than once", id.as_str());
                }
                composition.schedule(start, scheduled)?;
            }
        }
        Ok(())
    }
}

impl From<Motion> for Composition {
    fn from(value: Motion) -> Self {
        Self::animate(value)
    }
}

impl From<Annotation> for Composition {
    fn from(value: Annotation) -> Self {
        Self::annotate(value)
    }
}

impl From<TaskChange> for Composition {
    fn from(value: TaskChange) -> Self {
        Self::task(value)
    }
}

impl From<TaskPoseChange> for Composition {
    fn from(value: TaskPoseChange) -> Self {
        Self::task_pose(value)
    }
}

#[derive(Default)]
struct Scheduled {
    motions: Vec<(Time, Motion)>,
    media: Vec<MediaPlacement>,
    cues: HashSet<CueId>,
    annotations: Vec<(Time, Annotation)>,
    tasks: Vec<(Time, TaskChange)>,
    task_poses: Vec<(Time, TaskPoseChange)>,
}

pub(crate) struct LoweredComposition {
    pub motion: Motion,
    pub media: Vec<MediaPlacement>,
    pub annotations: Vec<(Time, Annotation)>,
    pub tasks: Vec<(Time, TaskChange)>,
    pub task_poses: Vec<(Time, TaskPoseChange)>,
    pub duration: Duration,
}

#[cfg(test)]
mod tests {
    use super::{Asset, Composition, Cue, Duration, MediaRole, Time, TimeRange};

    fn range(start: f64, end: f64) -> TimeRange {
        TimeRange::new(Time::seconds(start), Time::seconds(end))
    }

    #[test]
    fn script_edits_preserve_source_ranges_and_place_clips_sequentially() {
        let take = Asset::audio("take-3", "assets/take-3.wav");
        let opening = take.clip(range(4.2, 8.7));
        let explanation = take.clip(range(12.1, 18.4));
        let composition = Composition::sequence([
            Composition::script(opening),
            Composition::hold(Duration::milliseconds(150.0)),
            Composition::script(explanation),
        ]);

        let lowered = composition.lower().unwrap();

        assert_eq!(lowered.duration, Duration::seconds(10.95));
        assert_eq!(lowered.media.len(), 2);
        assert_eq!(lowered.media[0].role(), MediaRole::Script);
        assert_eq!(lowered.media[0].timeline_range(), range(0.0, 4.5));
        assert_eq!(lowered.media[0].clip().source_range(), range(4.2, 8.7));
        assert_eq!(lowered.media[1].timeline_range(), range(4.65, 10.95));
    }

    #[test]
    fn layers_can_be_synchronized_to_external_cue_ranges() {
        let cue = Cue::new("not-found", range(3.0, 3.8));
        let pop = Asset::audio("pop", "assets/pop.wav")
            .clip(range(0.0, 0.4))
            .gain_db(12.0);
        let composition = cue.at(Composition::layer(pop));

        let lowered = composition.lower().unwrap();

        assert_eq!(lowered.media[0].role(), MediaRole::Layer);
        assert_eq!(lowered.media[0].timeline_range(), range(3.0, 3.4));
        assert_eq!(lowered.media[0].clip().audio_gain_db(), 12.0);
    }

    #[test]
    fn named_compositions_reject_repeated_cue_names() {
        let clip = Asset::audio("take", "assets/take.wav").clip(range(0.0, 1.25));
        let composition = Composition::sequence([
            Composition::named("opening", Composition::script(clip.clone())),
            Composition::named("opening", Composition::script(clip)),
        ]);

        let error = composition.lower().err().unwrap();

        assert!(error.to_string().contains("cue 'opening' more than once"));
    }

    #[test]
    fn frame_count_is_exact_at_frame_aligned_decimal_durations() {
        assert_eq!(Duration::seconds(4.15).frame_count(60), 249);
        assert_eq!(Duration::seconds(5.0).frame_count(60), 300);
        assert_eq!(Duration::milliseconds(1.0).frame_count(60), 1);
    }

    #[test]
    fn media_placement_is_trimmed_and_rebased_to_render_window() {
        let clip = Asset::audio("take", "take.wav").clip(range(10.0, 20.0));
        let placement = super::MediaPlacement {
            clip,
            role: MediaRole::Script,
            timeline_range: range(3.0, 13.0),
        };

        let windowed = placement.for_window(range(5.0, 9.0)).unwrap();
        assert_eq!(windowed.clip().source_range(), range(12.0, 16.0));
        assert_eq!(windowed.timeline_range(), range(0.0, 4.0));
        assert_eq!(windowed.role(), MediaRole::Script);
    }

    #[test]
    fn disjoint_media_is_excluded_from_render_window() {
        let clip = Asset::audio("take", "take.wav").clip(range(10.0, 12.0));
        let placement = super::MediaPlacement {
            clip,
            role: MediaRole::Layer,
            timeline_range: range(3.0, 5.0),
        };

        assert!(placement.for_window(range(0.0, 3.0)).is_none());
        assert!(placement.for_window(range(5.0, 7.0)).is_none());
    }

    #[test]
    fn exact_times_format_with_nanosecond_precision() {
        assert_eq!(Time::from_nanos(1).to_string(), "0.000000001");
        assert_eq!(Duration::seconds(12.25).to_string(), "12.250000000");
    }
}
