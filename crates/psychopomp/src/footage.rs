//! Footage: any image, video, or image sequence as scene material. A **Clip**
//! names one planned `video` or `image` media placement and says how it plays:
//! the source range it trims to, a `rate`, whether it holds, loops, or
//! bounces at the trim's ends, whether it runs in reverse, or one frame it
//! freezes on. The placement names the file and the span of the plan it is
//! available in; its timeline start is where the playhead starts.
//!
//! The playhead is the `time` channel (seconds into the trim). Without one
//! the clip plays naturally at its rate; writing it (through a [`Playhead`])
//! freezes, ramps, stutters, or scrubs the clip, each a pure function of plan
//! time, so any frame samples the same source frame in any order.
//!
//! The same clip is drawn two ways: as a screen-space overlay
//! ([`FootagePlan`], through the projected card the Video Card uses, with
//! anchors, tilt, and card chrome) or as a Stage element seen through the
//! camera ([`crate::stage::StageElement::Footage`], with depth, parallax, and
//! depth of field). [`layout`] arranges collages without a GPU.
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    anchor::{self, AnchorPlan},
    author::{ActorHandle, ContinuousHandle, PlanBuilder, whole_millis},
    math::easing::Ease,
    plan::{MediaKindPlan, MediaPlan, MediaRolePlan},
    tone::Tone,
    video::TITLE_BAR,
};

pub mod layout;

pub const FOOTAGE_RECIPE: &str = "footage";

/// Every channel a footage overlay accepts, besides `anchor.<id>` weights.
/// `x`/`y` offset it from its center (or anchor) and `rotation` from its
/// rest angle; `blur` softens the near edge of a tilted card and `defocus`
/// the whole surface. `focus-*` is the Ken Burns window, `time` the
/// playhead, and `saturation`, `tint`, and `dim` its color treatment.
pub const FOOTAGE_CHANNELS: [&str; 16] = [
    "x",
    "y",
    "scale",
    "opacity",
    "rotation",
    "tilt-x",
    "tilt-y",
    "blur",
    "defocus",
    "focus-x",
    "focus-y",
    "focus-size",
    "time",
    "saturation",
    "tint",
    "dim",
];

/// Every channel a Stage footage element reads (after its `<id>.` prefix).
pub const STAGE_FOOTAGE_CHANNELS: [&str; 14] = [
    "opacity",
    "x",
    "y",
    "z",
    "scale",
    "blur",
    "rotation",
    "focus-x",
    "focus-y",
    "focus-size",
    "time",
    "saturation",
    "tint",
    "dim",
];

/// What a footage channel reads before anything writes it. `time` has no
/// constant rest: unwritten, it is the clip's natural playhead.
pub fn channel_default(property: &str) -> Option<f32> {
    Some(match property {
        "scale" | "opacity" | "saturation" | "focus-size" => 1.0,
        "focus-x" | "focus-y" => 0.5,
        "x" | "y" | "z" | "rotation" | "tilt-x" | "tilt-y" | "blur" | "defocus" | "tint"
        | "dim" => 0.0,
        _ => return None,
    })
}

/// What a clip does at the ends of its trim.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Repeat {
    /// Hold the first frame before the trim and the last after it.
    #[default]
    Hold,
    /// Wrap around to the start.
    Loop,
    /// Play forward, then backward, and again.
    Bounce,
}

impl Repeat {
    pub fn is_hold(&self) -> bool {
        *self == Self::Hold
    }
}

/// How one source plays: which media, the part of it, and its playhead.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Clip {
    /// The planned `video` or `image` media placement this clip plays.
    pub media: String,
    /// Seconds into the source `[from, to]` the clip plays; the whole file
    /// by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim: Option<[f64; 2]>,
    /// Source seconds per plan second.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub rate: f32,
    #[serde(default, skip_serializing_if = "Repeat::is_hold")]
    pub repeat: Repeat,
    /// Play the trim from its end to its start.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reverse: bool,
    /// Show this many seconds into the trim throughout, as a still.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze: Option<f32>,
    /// Frames per second to decode: a video's own rate (at most 60) by
    /// default, and an image sequence's playback rate (24 by default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<u32>,
    /// Decoded width in pixels, overriding the renderer's choice (about
    /// twice the widest the clip is shown).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<u32>,
}

fn one() -> f32 {
    1.0
}
fn is_one(value: &f32) -> bool {
    *value == 1.0
}
fn is_zero(value: &f32) -> bool {
    *value == 0.0
}
fn accent() -> Tone {
    Tone::Accent
}
fn is_accent(tone: &Tone) -> bool {
    *tone == Tone::Accent
}

fn nanos(seconds: f64) -> i64 {
    (seconds * 1e9).round() as i64
}

impl Clip {
    /// The whole of `media`, playing once at its own rate.
    pub fn new(media: impl Into<String>) -> Self {
        Self {
            media: media.into(),
            trim: None,
            rate: 1.0,
            repeat: Repeat::Hold,
            reverse: false,
            freeze: None,
            fps: None,
            resolution: None,
        }
    }

    pub fn trimmed(mut self, from: f64, to: f64) -> Self {
        self.trim = Some([from, to]);
        self
    }

    pub fn at_rate(mut self, rate: f32) -> Self {
        self.rate = rate;
        self
    }

    pub fn looping(mut self) -> Self {
        self.repeat = Repeat::Loop;
        self
    }

    pub fn bouncing(mut self) -> Self {
        self.repeat = Repeat::Bounce;
        self
    }

    pub fn reversed(mut self) -> Self {
        self.reverse = true;
        self
    }

    /// Show one frame, `seconds` into the trim, throughout.
    pub fn frozen(mut self, seconds: f32) -> Self {
        self.freeze = Some(seconds);
        self
    }

    pub fn decoded_at(mut self, fps: u32) -> Self {
        self.fps = Some(fps);
        self
    }

    pub fn resolution(mut self, width: u32) -> Self {
        self.resolution = Some(width);
        self
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(!self.media.is_empty(), "a clip needs a media ID");
        if let Some([from, to]) = self.trim {
            ensure!(
                from.is_finite() && to.is_finite() && from >= 0.0 && to > from,
                "clip '{}' trim must run forward from a non-negative start",
                self.media
            );
        }
        ensure!(
            self.rate.is_finite() && self.rate > 0.0 && self.rate <= 16.0,
            "clip '{}' rate must be in (0, 16]; reverse it with `reverse`",
            self.media
        );
        if let Some(freeze) = self.freeze {
            ensure!(
                freeze.is_finite() && freeze >= 0.0,
                "clip '{}' freeze must be a non-negative time",
                self.media
            );
        }
        if let Some(fps) = self.fps {
            ensure!(
                (1..=120).contains(&fps),
                "clip '{}' fps must be 1 to 120",
                self.media
            );
        }
        if let Some(width) = self.resolution {
            ensure!(
                (16..=4096).contains(&width),
                "clip '{}' resolution must be 16 to 4096 pixels",
                self.media
            );
        }
        Ok(())
    }

    /// The trim in source nanoseconds, or `None` for the whole file.
    pub fn trim_nanos(&self) -> Option<(u64, u64)> {
        self.trim
            .map(|[from, to]| (nanos(from) as u64, nanos(to) as u64))
    }

    /// The playhead (nanoseconds into the trim) at plan time `at` with no
    /// `time` channel: it starts at the placement's timeline start and runs
    /// at `rate`, or holds its freeze.
    pub fn natural_nanos(&self, placement: &MediaPlan, at: u64) -> i64 {
        if let Some(freeze) = self.freeze {
            return nanos(f64::from(freeze));
        }
        let elapsed = at.saturating_sub(placement.timeline_start_nanos);
        if self.rate == 1.0 {
            elapsed as i64
        } else {
            (elapsed as f64 * f64::from(self.rate)).round() as i64
        }
    }

    /// The source time (nanoseconds into the file) a playhead shows within
    /// `trim`: held at its ends, wrapped, or bounced, then mirrored when the
    /// clip runs in reverse.
    pub fn source_nanos(&self, trim: (u64, u64), playhead: i64) -> u64 {
        let length = trim.1.saturating_sub(trim.0) as i64;
        if length == 0 {
            return trim.0;
        }
        let into = match self.repeat {
            Repeat::Hold => playhead.clamp(0, length),
            Repeat::Loop => playhead.rem_euclid(length),
            Repeat::Bounce => {
                let phase = playhead.rem_euclid(2 * length);
                if phase <= length {
                    phase
                } else {
                    2 * length - phase
                }
            }
        };
        let into = if self.reverse { length - into } else { into };
        trim.0 + into as u64
    }
}

/// How a source fills its box.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fit {
    /// Fill the box, cropping the source's overflow evenly.
    #[default]
    Cover,
    /// Show all of the source inside the box, letterboxed.
    Contain,
    /// Stretch the whole source over the box.
    Fill,
}

impl Fit {
    pub fn is_cover(&self) -> bool {
        *self == Self::Cover
    }

    /// For a source `source` pixels in size shown in a box of `size`: the
    /// source window `[x, y, width, height]` it shows, and the rectangle of
    /// the box it fills (from the box's top-left).
    pub fn frame(self, source: [f32; 2], size: [f32; 2]) -> ([f32; 4], [f32; 4]) {
        let whole = [0.0, 0.0, source[0], source[1]];
        let filled = [0.0, 0.0, size[0], size[1]];
        let source_aspect = source[0] / source[1].max(1e-6);
        let box_aspect = size[0] / size[1].max(1e-6);
        match self {
            Self::Fill => (whole, filled),
            Self::Cover if source_aspect > box_aspect => {
                let width = source[1] * box_aspect;
                ([(source[0] - width) * 0.5, 0.0, width, source[1]], filled)
            }
            Self::Cover => {
                let height = source[0] / box_aspect;
                ([0.0, (source[1] - height) * 0.5, source[0], height], filled)
            }
            Self::Contain if source_aspect > box_aspect => {
                let height = size[0] / source_aspect;
                (whole, [0.0, (size[1] - height) * 0.5, size[0], height])
            }
            Self::Contain => {
                let width = size[1] * source_aspect;
                (whole, [(size[0] - width) * 0.5, 0.0, width, size[1]])
            }
        }
    }
}

/// The part of `window` a focus shows: centered at `center` (fractions of
/// the window) and `size` of it, clamped inside it. A focus that eases from
/// one region to another is a Ken Burns move.
pub fn focus_window(window: [f32; 4], center: [f32; 2], size: f32) -> [f32; 4] {
    let size = size.clamp(0.02, 1.0);
    let half = size * 0.5;
    let center = center.map(|c| c.clamp(half, 1.0 - half));
    [
        window[0] + (center[0] - half) * window[2],
        window[1] + (center[1] - half) * window[3],
        size * window[2],
        size * window[3],
    ]
}

/// The focus center and size whose window fits `region` (`[x, y, width,
/// height]` as fractions of the visible frame) inside it.
pub fn focus_on(region: [f32; 4]) -> ([f32; 2], f32) {
    (
        [region[0] + region[2] * 0.5, region[1] + region[3] * 0.5],
        region[2].max(region[3]).clamp(0.02, 1.0),
    )
}

/// The outline footage is cut to.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "shape", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Mask {
    /// The box, with rounded corners.
    Rect {
        #[serde(default, skip_serializing_if = "is_zero")]
        radius: f32,
    },
    /// The largest circle inside the box.
    Circle,
    /// Corners as fractions of the box from its top-left, in order.
    Polygon { points: Vec<[f32; 2]> },
}

impl Default for Mask {
    fn default() -> Self {
        Self::Rect { radius: 0.0 }
    }
}

impl Mask {
    pub fn is_square(&self) -> bool {
        matches!(self, Self::Rect { radius } if *radius == 0.0)
    }

    /// A polygon's corners relative to the box center, for a box of `size`.
    pub fn corners(&self, size: [f32; 2]) -> Vec<[f32; 2]> {
        match self {
            Self::Polygon { points } => points
                .iter()
                .map(|[x, y]| [(x - 0.5) * size[0], (y - 0.5) * size[1]])
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Rect { radius } => ensure!(
                radius.is_finite() && *radius >= 0.0,
                "a footage mask radius must be finite and non-negative"
            ),
            Self::Circle => {}
            Self::Polygon { points } => ensure!(
                (3..=32).contains(&points.len())
                    && points
                        .iter()
                        .flatten()
                        .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
                "a footage polygon mask has 3 to 32 corners, as fractions of the box"
            ),
        }
        Ok(())
    }
}

/// A color treatment, as the `saturation`, `tint`, and `dim` channels: 1, 0,
/// and 0 leave the source untouched. `tint` maps the source's luminance onto
/// the footage's tint tone, so desaturated and tinted footage reads as
/// reference material.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Treatment {
    pub saturation: f32,
    pub tint: f32,
    pub dim: f32,
}

impl Treatment {
    pub const NONE: Self = Self {
        saturation: 1.0,
        tint: 0.0,
        dim: 0.0,
    };

    /// Gray, tinted, and a little dark: footage that is only a reference.
    pub const REFERENCE: Self = Self {
        saturation: 0.0,
        tint: 0.55,
        dim: 0.25,
    };

    pub fn is_none(&self) -> bool {
        *self == Self::NONE
    }

    /// `rgb` (0..1) treated, with `tint` the tint tone's color. Luminance is
    /// Rec. 709's weights over the encoded values, cheaply.
    pub fn apply(&self, rgb: [f32; 3], tint: [f32; 3]) -> [f32; 3] {
        let luma = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
        let saturation = self.saturation.max(0.0);
        let amount = self.tint.clamp(0.0, 1.0);
        let keep = 1.0 - self.dim.clamp(0.0, 1.0);
        std::array::from_fn(|i| {
            let gray = luma + (rgb[i] - luma) * saturation;
            let toned = gray + (tint[i] * luma * 1.6 - gray) * amount;
            (toned * keep).clamp(0.0, 1.0)
        })
    }
}

/// A screen-space footage overlay: a clip in a box `size` pixels at `center`
/// (or its anchor), turned `rotation` radians at rest, cut to its mask, and
/// optionally framed as a card with a title bar. Drawn through the shared
/// projected card, so it moves, scales, tilts, defocuses, and motion-blurs
/// like a Video Card.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FootagePlan {
    pub clip: Clip,
    /// Center on the canvas at rest; an anchor replaces it.
    pub center: [f32; 2],
    /// The footage's box at scale 1, below any title bar.
    pub size: [f32; 2],
    /// Rest angle in radians; the `rotation` channel turns from it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotation: f32,
    #[serde(default, skip_serializing_if = "Fit::is_cover")]
    pub fit: Fit,
    #[serde(default, skip_serializing_if = "Mask::is_square")]
    pub mask: Mask,
    /// A card behind it: the theme's material, border, and shadow.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub framed: bool,
    /// A title bar above it; framed rectangles only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The tone the `tint` channel moves toward.
    #[serde(default = "accent", skip_serializing_if = "is_accent")]
    pub tint: Tone,
    /// Places it can pin to; while it has any, the blended anchor (plus that
    /// anchor's offset) replaces `center`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<AnchorPlan>,
}

impl FootagePlan {
    /// `clip` bare in a box of `size` at `center`.
    pub fn new(clip: Clip, center: [f32; 2], size: [f32; 2]) -> Self {
        Self {
            clip,
            center,
            size,
            rotation: 0.0,
            fit: Fit::Cover,
            mask: Mask::default(),
            framed: false,
            title: None,
            tint: Tone::Accent,
            anchors: Vec::new(),
        }
    }

    /// In a layout tile: its center, size, and rest angle.
    pub fn in_tile(clip: Clip, tile: layout::Tile) -> Self {
        Self::new(clip, tile.center, tile.size).turned(tile.rotation)
    }

    pub fn turned(mut self, radians: f32) -> Self {
        self.rotation = radians;
        self
    }

    pub fn fit(mut self, fit: Fit) -> Self {
        self.fit = fit;
        self
    }

    /// On a card with rounded corners, the Video Card's chrome.
    pub fn framed(mut self) -> Self {
        self.framed = true;
        if self.mask.is_square() {
            self.mask = Mask::Rect { radius: 18.0 };
        }
        self
    }

    /// A framed card with a title bar.
    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self.framed()
    }

    pub fn rounded(mut self, radius: f32) -> Self {
        self.mask = Mask::Rect { radius };
        self
    }

    pub fn circle(mut self) -> Self {
        self.mask = Mask::Circle;
        self
    }

    /// Cut to a polygon whose corners are fractions of the box.
    pub fn polygon(mut self, points: impl IntoIterator<Item = [f32; 2]>) -> Self {
        self.mask = Mask::Polygon {
            points: points.into_iter().collect(),
        };
        self
    }

    pub fn tinted(mut self, tone: Tone) -> Self {
        self.tint = tone;
        self
    }

    /// Pin its center to `anchor`; the first anchor is where it starts.
    pub fn anchor(mut self, anchor: AnchorPlan) -> Self {
        self.anchors.push(anchor);
        self
    }

    pub fn title_bar(&self) -> f32 {
        if self.title.is_some() { TITLE_BAR } else { 0.0 }
    }

    /// The card at scale 1: the box and any title bar above it.
    pub fn card_size(&self) -> [f32; 2] {
        [self.size[0], self.size[1] + self.title_bar()]
    }

    /// True when `property` names one of this overlay's channels.
    pub fn accepts(&self, property: &str) -> bool {
        FOOTAGE_CHANNELS.contains(&property) || anchor::accepts(property, &self.anchors)
    }

    pub fn validate(&self) -> Result<()> {
        self.clip.validate()?;
        ensure!(
            self.center.iter().all(|v| v.is_finite()) && self.rotation.is_finite(),
            "footage center and rotation must be finite"
        );
        ensure!(
            self.size
                .iter()
                .all(|v| v.is_finite() && (8.0..=4096.0).contains(v)),
            "footage size must be 8 to 4096 pixels on each side"
        );
        self.mask.validate()?;
        if let Some(title) = &self.title {
            ensure!(
                self.framed && matches!(self.mask, Mask::Rect { .. }),
                "only a framed rectangle has a title bar"
            );
            ensure!(
                !title.trim().is_empty() && title.chars().count() <= 80 && !title.contains('\n'),
                "footage titles are one line of 1 to 80 characters"
            );
        }
        anchor::validate("footage", &self.anchors)
    }
}

/// A video or image-sequence placement: `path` available from `from` until
/// `until` on the plan clock, its playhead starting at `from`. An image
/// sequence is a printf pattern such as `frames/%04d.png`.
pub fn media(id: impl Into<String>, path: impl Into<PathBuf>, from: u64, until: u64) -> MediaPlan {
    placement(id, path, MediaKindPlan::Video, from, until)
}

/// A still image placement (PNG, JPEG, or WebP), available from `from`
/// until `until`.
pub fn still(id: impl Into<String>, path: impl Into<PathBuf>, from: u64, until: u64) -> MediaPlan {
    placement(id, path, MediaKindPlan::Image, from, until)
}

fn placement(
    id: impl Into<String>,
    path: impl Into<PathBuf>,
    kind: MediaKindPlan,
    from: u64,
    until: u64,
) -> MediaPlan {
    MediaPlan {
        id: id.into(),
        path: path.into(),
        kind,
        role: MediaRolePlan::Layer,
        source_start_nanos: 0,
        source_end_nanos: until.saturating_sub(from),
        timeline_start_nanos: from,
        timeline_end_nanos: until,
        gain_db: 0.0,
    }
}

/// True when `path` is an image-sequence pattern (`%d`, `%04d`, ...).
pub fn is_sequence(path: &Path) -> bool {
    let text = path.to_string_lossy();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            while chars.peek().is_some_and(char::is_ascii_digit) {
                chars.next();
            }
            if chars.peek() == Some(&'d') {
                return true;
            }
        }
    }
    false
}

/// The audio track of a clip's video, as layer placements that follow its
/// trim and timeline start (one per pass when it loops), each ending by
/// `until`. Audio does not follow a rate, reverse, bounce, freeze, or a
/// written `time` channel.
pub fn audio(
    id: &str,
    clip: &Clip,
    video: &MediaPlan,
    until: u64,
    gain_db: f32,
) -> Result<Vec<MediaPlan>> {
    ensure!(
        matches!(video.kind, MediaKindPlan::Video),
        "only a video clip has audio"
    );
    ensure!(
        clip.rate == 1.0 && !clip.reverse && clip.freeze.is_none() && clip.repeat != Repeat::Bounce,
        "clip '{}' audio follows its trim and start only: it must play forward at rate 1",
        clip.media
    );
    let (from, to) = clip
        .trim_nanos()
        .with_context(|| format!("clip '{}' needs a trim to place its audio", clip.media))?;
    let length = to - from;
    let mut placements = Vec::new();
    let mut at = video.timeline_start_nanos;
    while at < until {
        let span = length.min(until - at);
        placements.push(MediaPlan {
            id: if placements.is_empty() {
                id.to_owned()
            } else {
                format!("{id}#{}", placements.len())
            },
            path: video.path.clone(),
            kind: MediaKindPlan::Audio,
            role: MediaRolePlan::Layer,
            source_start_nanos: from,
            source_end_nanos: from + span,
            timeline_start_nanos: at,
            timeline_end_nanos: at + span,
            gain_db,
        });
        if clip.repeat != Repeat::Loop {
            break;
        }
        at += length;
    }
    Ok(placements)
}

/// A clip's playhead as a channel: seconds into its trim. Until something
/// writes it the clip plays naturally; each write continues from the
/// playhead authored so far, so retimes compose in time order.
#[derive(Clone, Debug)]
pub struct Playhead {
    actor: ActorHandle,
    property: String,
    start_nanos: u64,
    rate: f32,
    freeze: Option<f32>,
}

impl Playhead {
    /// The playhead of `clip` placed by `media`, on `actor`'s `property`
    /// (`time` for an overlay, `<element>.time` on a Stage).
    pub fn new(
        actor: ActorHandle,
        property: impl Into<String>,
        clip: &Clip,
        media: &MediaPlan,
    ) -> Self {
        Self {
            actor,
            property: property.into(),
            start_nanos: media.timeline_start_nanos,
            rate: clip.rate,
            freeze: clip.freeze,
        }
    }

    pub fn property(&self) -> &str {
        &self.property
    }

    fn natural(&self, at: u64) -> (f32, f32) {
        if let Some(freeze) = self.freeze {
            return (freeze, 0.0);
        }
        if at < self.start_nanos {
            return (0.0, 0.0);
        }
        let elapsed = (at - self.start_nanos) as f64 / 1e9;
        ((elapsed * f64::from(self.rate)) as f32, self.rate)
    }

    /// Where the playhead is (seconds into the trim) and how fast it runs at
    /// `at`, as written so far.
    pub fn at(&self, scene: &PlanBuilder, at: u64) -> (f32, f32) {
        scene.sample(&self.actor, &self.property, at).map_or_else(
            || self.natural(at),
            |state| (state.position, state.velocity),
        )
    }

    /// The channel, declared on first use playing naturally up to `at`.
    fn channel(&self, scene: &mut PlanBuilder, at: u64) -> ContinuousHandle {
        if scene.sample(&self.actor, &self.property, 0).is_some() {
            return scene.channel(&self.actor, &self.property, 0.0);
        }
        let (initial, _) = self.natural(0);
        let channel = scene.channel(&self.actor, &self.property, initial);
        if self.freeze.is_none() && at > self.start_nanos {
            let (position, _) = self.natural(at);
            scene.ease(
                &channel,
                self.start_nanos,
                position,
                seconds_between(self.start_nanos, at),
                Ease::Linear,
            );
        }
        channel
    }

    /// Hold the frame showing at `at`.
    pub fn freeze(&self, scene: &mut PlanBuilder, at: u64) -> u64 {
        let (position, _) = self.at(scene, at);
        let channel = self.channel(scene, at);
        scene.set(&channel, at, position);
        at
    }

    /// Run at `rate` from wherever the playhead is at `at`, to the end of
    /// the scene (or until a later write).
    pub fn play(&self, scene: &mut PlanBuilder, at: u64, rate: f32) -> u64 {
        let (position, _) = self.at(scene, at);
        let channel = self.channel(scene, at);
        let rest = seconds_between(at, scene.duration_nanos());
        if rest > 0.0 {
            scene.ease(&channel, at, position + rate * rest, rest, Ease::Linear);
        } else {
            scene.set(&channel, at, position);
        }
        at
    }

    /// Change speed from the current rate to `rate` over `seconds`, the
    /// rate changing linearly (a freeze ramping up to speed, or a slowdown
    /// into slow motion), then keep running at `rate`.
    pub fn ramp(&self, scene: &mut PlanBuilder, at: u64, seconds: f32, rate: f32) -> u64 {
        let (position, current) = self.at(scene, at);
        let channel = self.channel(scene, at);
        let end = at + whole_millis(seconds);
        let span = seconds_between(at, end);
        let average = (current + rate) * 0.5;
        if span > 0.0 && average > 1e-6 {
            // A quadratic playhead leaves at `current` and arrives at `rate`.
            let arrival = (rate / average).clamp(0.0, 2.0);
            scene.ease(
                &channel,
                at,
                position + average * span,
                span,
                Ease::Decelerate(arrival),
            );
        } else {
            scene.set(&channel, at, position);
        }
        self.play(scene, end, rate);
        end
    }

    /// Move the playhead to `target` seconds over `seconds` along `curve`,
    /// then hold there: a scrub, a rewind, or a slow push through a moment.
    pub fn retime(
        &self,
        scene: &mut PlanBuilder,
        at: u64,
        target: f32,
        seconds: f32,
        curve: Ease,
    ) -> u64 {
        let channel = self.channel(scene, at);
        scene.ease(&channel, at, target, seconds, curve);
        at + whole_millis(seconds)
    }

    /// Cut to `target` seconds at `at` and keep running at the rate it had.
    pub fn seek(&self, scene: &mut PlanBuilder, at: u64, target: f32) -> u64 {
        let (_, rate) = self.at(scene, at);
        let channel = self.channel(scene, at);
        scene.set(&channel, at, target);
        self.play(scene, at, rate);
        at
    }

    /// Repeat the `seconds` after `at` `times` times, then play on: a stutter.
    pub fn stutter(&self, scene: &mut PlanBuilder, at: u64, seconds: f32, times: u32) -> u64 {
        let (position, rate) = self.at(scene, at);
        let rate = if rate > 1e-6 { rate } else { 1.0 };
        let channel = self.channel(scene, at);
        let step = whole_millis(seconds);
        let span = seconds_between(0, step);
        for index in 0..u64::from(times) {
            let start = at + index * step;
            scene.set(&channel, start, position);
            scene.ease(&channel, start, position + rate * span, span, Ease::Linear);
        }
        let end = at + u64::from(times) * step;
        self.play(scene, end, rate);
        end
    }
}

fn seconds_between(from: u64, to: u64) -> f32 {
    (to.saturating_sub(from) / 1_000_000) as f32 / 1000.0
}

/// Authoring handle for one footage overlay. Every beat returns the time it
/// settles, so beats chain.
#[derive(Clone, Debug)]
pub struct FootageActor {
    actor: ActorHandle,
    plan: FootagePlan,
    media: MediaPlan,
    anchors: Vec<String>,
    playhead: Playhead,
}

impl FootageActor {
    /// Declare the overlay and the placement its clip plays.
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: &FootagePlan,
        media: MediaPlan,
    ) -> Result<Self> {
        plan.validate()?;
        ensure!(
            matches!(media.kind, MediaKindPlan::Video | MediaKindPlan::Image)
                && media.id == plan.clip.media,
            "footage plays the video or image media '{}'",
            plan.clip.media
        );
        let actor = scene.actor(id, FOOTAGE_RECIPE, plan)?;
        scene.media(media.clone());
        let playhead = Playhead::new(actor.clone(), "time", &plan.clip, &media);
        Ok(Self {
            actor,
            plan: plan.clone(),
            media,
            anchors: anchor::ids(&plan.anchors),
            playhead,
        })
    }

    pub fn actor(&self) -> &ActorHandle {
        &self.actor
    }

    pub fn id(&self) -> &str {
        self.actor.id()
    }

    pub fn plan(&self) -> &FootagePlan {
        &self.plan
    }

    pub fn media(&self) -> &MediaPlan {
        &self.media
    }

    pub fn playhead(&self) -> &Playhead {
        &self.playhead
    }

    /// The channel for `property`, declared on first use at its rest value.
    pub fn channel(&self, scene: &mut PlanBuilder, property: &str) -> ContinuousHandle {
        scene.channel(
            &self.actor,
            property,
            channel_default(property).unwrap_or(0.0),
        )
    }

    /// Spring each `(property, from, to, seconds, bounce)` from `from`, a
    /// channel's starting pose when this declares it.
    fn springs(&self, scene: &mut PlanBuilder, at: u64, moves: &[(&str, f32, f32, f32, f32)]) {
        for &(property, from, to, seconds, bounce) in moves {
            let channel = scene.channel(&self.actor, property, from);
            scene.spring(&channel, at, to, seconds, bounce);
        }
    }

    /// Fly up into place, as a Video Card does: it starts low, small, tipped
    /// back, and soft, and lands on critically damped springs.
    pub fn fly_in(&self, scene: &mut PlanBuilder, at: u64) -> u64 {
        self.springs(
            scene,
            at,
            &[
                ("y", 160.0, 0.0, 0.8, 0.0),
                ("scale", 0.86, 1.0, 0.8, 0.0),
                ("tilt-x", 0.42, 0.0, 0.85, 0.0),
                ("blur", 9.0, 0.0, 0.7, 0.0),
                ("opacity", 0.0, 1.0, 0.35, 0.0),
            ],
        );
        at + whole_millis(0.85)
    }

    /// Thrown onto a collage from `from` (pixels from its place), turning
    /// `spin` radians on the way and dropping from just above the table, so
    /// it lands with a little settle. Fast, so it motion-blurs.
    pub fn toss_in(&self, scene: &mut PlanBuilder, at: u64, from: [f32; 2], spin: f32) -> u64 {
        self.springs(
            scene,
            at,
            &[
                ("x", from[0], 0.0, 0.62, 0.12),
                ("y", from[1], 0.0, 0.62, 0.12),
                ("rotation", spin, 0.0, 0.7, 0.18),
                ("scale", 1.22, 1.0, 0.55, 0.1),
                ("opacity", 0.0, 1.0, 0.12, 0.0),
            ],
        );
        at + whole_millis(0.7)
    }

    /// Fade out in place.
    pub fn hide(&self, scene: &mut PlanBuilder, at: u64) -> u64 {
        let opacity = self.channel(scene, "opacity");
        scene.spring(&opacity, at, 0.0, 0.3, 0.0);
        at + whole_millis(0.3)
    }

    /// Spring any channel to `target`.
    pub fn to(
        &self,
        scene: &mut PlanBuilder,
        property: &str,
        at: u64,
        target: f32,
        seconds: f32,
    ) -> u64 {
        let channel = self.channel(scene, property);
        scene.spring(&channel, at, target, seconds, 0.0);
        at + whole_millis(seconds)
    }

    /// Glide to `offset` pixels from its place in exactly `seconds`, on the
    /// minimum-jerk curve.
    pub fn glide(&self, scene: &mut PlanBuilder, at: u64, offset: [f32; 2], seconds: f32) -> u64 {
        for (property, target) in [("x", offset[0]), ("y", offset[1])] {
            let channel = self.channel(scene, property);
            scene.ease(&channel, at, target, seconds, Ease::Smootherstep);
        }
        at + whole_millis(seconds)
    }

    /// Glide to its anchor `to`, carrying velocity through interruptions.
    pub fn move_to(&self, scene: &mut PlanBuilder, to: &str, at: u64) -> Result<u64> {
        anchor::move_to(scene, &self.actor, &self.anchors, to, at)?;
        Ok(at + whole_millis(0.6))
    }

    /// Zoom so `region` (`[x, y, width, height]` as fractions of the visible
    /// frame) fills the box.
    pub fn focus(&self, scene: &mut PlanBuilder, at: u64, region: [f32; 4], seconds: f32) -> u64 {
        let (center, size) = focus_on(region);
        self.focus_to(scene, at, center, size, seconds, None)
    }

    /// Return to the whole frame.
    pub fn unfocus(&self, scene: &mut PlanBuilder, at: u64, seconds: f32) -> u64 {
        self.focus_to(scene, at, [0.5, 0.5], 1.0, seconds, None)
    }

    /// A Ken Burns move: from `from` to `to` (regions as fractions of the
    /// visible frame) between `at` and `until`, slowly in and out.
    pub fn drift(
        &self,
        scene: &mut PlanBuilder,
        at: u64,
        until: u64,
        from: [f32; 4],
        to: [f32; 4],
    ) -> u64 {
        let (center, size) = focus_on(from);
        self.focus_to(scene, at, center, size, 0.0, None);
        let (center, size) = focus_on(to);
        self.focus_to(
            scene,
            at,
            center,
            size,
            seconds_between(at, until),
            Some(Ease::CubicInOut),
        )
    }

    fn focus_to(
        &self,
        scene: &mut PlanBuilder,
        at: u64,
        center: [f32; 2],
        size: f32,
        seconds: f32,
        curve: Option<Ease>,
    ) -> u64 {
        // The window's center and size move together, so every point of the
        // region travels monotonically to where it lands.
        for (property, target) in [
            ("focus-x", center[0]),
            ("focus-y", center[1]),
            ("focus-size", size),
        ] {
            let channel = self.channel(scene, property);
            match curve {
                _ if seconds <= 0.0 => scene.set(&channel, at, target),
                Some(curve) => scene.ease(&channel, at, target, seconds, curve),
                None => scene.spring(&channel, at, target, seconds, 0.0),
            }
        }
        at + whole_millis(seconds)
    }

    /// Ease the color treatment to `treatment` over `seconds`.
    pub fn treat(
        &self,
        scene: &mut PlanBuilder,
        at: u64,
        treatment: Treatment,
        seconds: f32,
    ) -> u64 {
        for (property, target) in [
            ("saturation", treatment.saturation),
            ("tint", treatment.tint),
            ("dim", treatment.dim),
        ] {
            let channel = self.channel(scene, property);
            scene.ease(&channel, at, target, seconds, Ease::CubicInOut);
        }
        at + whole_millis(seconds)
    }

    /// Hold the frame showing at `at`.
    pub fn freeze(&self, scene: &mut PlanBuilder, at: u64) -> u64 {
        self.playhead.freeze(scene, at)
    }

    /// Run at `rate` from `at`.
    pub fn play(&self, scene: &mut PlanBuilder, at: u64, rate: f32) -> u64 {
        self.playhead.play(scene, at, rate)
    }

    /// Ramp to `rate` over `seconds`, then keep it.
    pub fn ramp(&self, scene: &mut PlanBuilder, at: u64, seconds: f32, rate: f32) -> u64 {
        self.playhead.ramp(scene, at, seconds, rate)
    }

    /// Move the playhead to `target` seconds over `seconds` along `curve`.
    pub fn retime(
        &self,
        scene: &mut PlanBuilder,
        at: u64,
        target: f32,
        seconds: f32,
        curve: Ease,
    ) -> u64 {
        self.playhead.retime(scene, at, target, seconds, curve)
    }

    /// Repeat the `seconds` after `at` `times` times, then play on.
    pub fn stutter(&self, scene: &mut PlanBuilder, at: u64, seconds: f32, times: u32) -> u64 {
        self.playhead.stutter(scene, at, seconds, times)
    }

    /// Place the video's own audio under it, following its trim and start
    /// (and its loops) until the placement ends.
    pub fn audio(&self, scene: &mut PlanBuilder, gain_db: f32) -> Result<()> {
        let id = format!("{}-audio", self.media.id);
        for placement in audio(
            &id,
            &self.plan.clip,
            &self.media,
            self.media.timeline_end_nanos,
            gain_db,
        )? {
            scene.media(placement);
        }
        Ok(())
    }
}

/// What `ffprobe` reports about a source: its pixel size, frame rate,
/// duration, and whether it carries alpha or audio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Probe {
    pub size: [u32; 2],
    pub fps: f64,
    pub seconds: f64,
    pub alpha: bool,
    pub audio: bool,
    /// A WebM whose VP8 or VP9 stream keeps its alpha in a side channel only
    /// libvpx decodes.
    pub vpx_alpha: Option<&'static str>,
}

impl Probe {
    pub fn aspect(&self) -> f32 {
        self.size[0] as f32 / self.size[1].max(1) as f32
    }
}

/// Probe a video, an image, or an image sequence (a printf pattern, read at
/// `sequence_fps`) with `ffprobe`. Scene Programs use it to lay out footage
/// by its aspect; the renderer, to decode it.
pub fn probe(path: &Path, sequence_fps: Option<u32>) -> Result<Probe> {
    let mut command = std::process::Command::new("ffprobe");
    command.stdin(std::process::Stdio::null()).args([
        "-v",
        "error",
        "-show_entries",
        "stream=codec_type,codec_name,width,height,r_frame_rate,pix_fmt:stream_tags=alpha_mode:format=duration",
        "-of",
        "json",
    ]);
    if is_sequence(path) {
        command.args(["-framerate", &sequence_fps.unwrap_or(24).to_string()]);
    }
    let output = command
        .arg(path)
        .output()
        .with_context(|| format!("run ffprobe on {}", path.display()))?;
    if !output.status.success() {
        bail!(
            "ffprobe could not read {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)
        .with_context(|| format!("parse ffprobe output for {}", path.display()))?;
    parse_probe(&report).with_context(|| format!("probe {}", path.display()))
}

fn parse_probe(report: &serde_json::Value) -> Result<Probe> {
    let streams = report["streams"].as_array().context("no streams")?;
    let video = streams
        .iter()
        .find(|stream| stream["codec_type"] == "video")
        .context("no video stream")?;
    let dimension = |key: &str| {
        video[key]
            .as_u64()
            .filter(|v| *v > 0)
            .map(|v| v as u32)
            .with_context(|| format!("no {key}"))
    };
    let fps = video["r_frame_rate"]
        .as_str()
        .and_then(|rate| {
            let (numerator, denominator) = rate.split_once('/')?;
            let value = numerator.parse::<f64>().ok()? / denominator.parse::<f64>().ok()?;
            value.is_finite().then_some(value)
        })
        .filter(|fps| *fps > 0.0)
        .unwrap_or(25.0);
    let pix_fmt = video["pix_fmt"].as_str().unwrap_or("");
    let codec = video["codec_name"].as_str().unwrap_or("");
    let side_alpha = video["tags"]["alpha_mode"] == "1" || video["tags"]["ALPHA_MODE"] == "1";
    let vpx_alpha = match codec {
        "vp8" if side_alpha => Some("libvpx"),
        "vp9" if side_alpha => Some("libvpx-vp9"),
        _ => None,
    };
    let alpha = side_alpha
        || pix_fmt.starts_with("yuva")
        || pix_fmt.starts_with("gbrap")
        || pix_fmt.contains("rgba")
        || pix_fmt.contains("argb")
        || pix_fmt.contains("bgra")
        || pix_fmt.contains("abgr")
        || pix_fmt == "ya8"
        || pix_fmt == "ya16be"
        || pix_fmt == "ya16le";
    Ok(Probe {
        size: [dimension("width")?, dimension("height")?],
        fps,
        seconds: report["format"]["duration"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0),
        alpha,
        audio: streams.iter().any(|stream| stream["codec_type"] == "audio"),
        vpx_alpha,
    })
}

#[cfg(test)]
mod tests;
