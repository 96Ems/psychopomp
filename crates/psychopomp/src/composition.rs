use std::path::{Path, PathBuf};

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
    /// `clip` placed at `start` on the containing timeline.
    pub fn new(clip: Clip, role: MediaRole, start: Time) -> Self {
        let timeline_range = TimeRange::new(start, start.after(clip.duration()));
        Self {
            clip,
            role,
            timeline_range,
        }
    }

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
}

#[cfg(test)]
mod tests {
    use super::{Asset, Duration, MediaRole, Time, TimeRange};

    fn range(start: f64, end: f64) -> TimeRange {
        TimeRange::new(Time::seconds(start), Time::seconds(end))
    }

    #[test]
    fn frame_count_is_exact_at_frame_aligned_decimal_durations() {
        assert_eq!(Duration::from_nanos(4_150_000_000).frame_count(60), 249);
        assert_eq!(Duration::from_nanos(5_000_000_000).frame_count(60), 300);
        assert_eq!(Duration::from_nanos(1_000_000).frame_count(60), 1);
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
        assert_eq!(
            Duration::from_nanos(12_250_000_000).to_string(),
            "12.250000000"
        );
    }
}
