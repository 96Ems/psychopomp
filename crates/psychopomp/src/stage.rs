//! Stage: a 2.5D motion-graphics surface for explainers. Elements (cards, particle
//! orbs, light beams, travelling packets, labels, rings) sit at world positions seen
//! through a perspective camera, and every change is an ordinary Continuous
//! Channel. Geometry that depends on time (orb spin, beam flow) is a pure function
//! of the sample time, so any frame renders identically in any order.
use std::collections::HashSet;

use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    author::{ActorHandle, ContinuousHandle, PlanBuilder, whole_millis},
    caption::{self, CaptionAlign, CaptionSpanPlan},
    effects::spinner::Mark,
    math::{
        Vec2, Vec3,
        easing::Ease,
        random::hash,
        shapes::{Box2, Circle, Shape, fibonacci_sphere},
        vec3,
    },
    plan::SpringPlan,
    tone::Tone,
};

pub const STAGE_RECIPE: &str = "stage";

/// Distance from the camera to the z = 0 plane. World x/y are canvas pixels at
/// z = 0, so an element with default camera channels lands exactly where authored.
pub const FOCAL: f32 = 1400.0;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StagePlan {
    #[serde(default)]
    pub post: StagePost,
    pub elements: Vec<StageElement>,
}

/// Look of the whole frame. The `post.bloom` and `post.vignette` channels
/// override `bloom` and `vignette`; `grain` and `backdrop` stay fixed.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StagePost {
    pub bloom: f32,
    pub grain: f32,
    pub vignette: f32,
    /// A soft neutral light behind the scene, 0 for none.
    pub backdrop: f32,
}

impl Default for StagePost {
    fn default() -> Self {
        Self {
            bloom: 0.55,
            grain: 0.035,
            vignette: 0.4,
            backdrop: 0.35,
        }
    }
}

impl StagePost {
    /// The explainer films' look: restrained bloom on a quiet, nearly flat
    /// frame, so only what is alive glows.
    pub const RESTRAINED: Self = Self {
        bloom: 0.18,
        grain: 0.012,
        vignette: 0.22,
        backdrop: 0.12,
    };
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatusText {
    pub text: String,
    #[serde(default, skip_serializing_if = "Tone::is_default")]
    pub tone: Tone,
}

impl StatusText {
    pub fn new(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum StageElement {
    /// A floating panel with a title and an optional status line. `status`
    /// entries cross-fade by the fractional `status` channel.
    #[serde(rename_all = "camelCase")]
    Card {
        id: String,
        at: [f32; 3],
        size: [f32; 2],
        title: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        status: Vec<StatusText>,
        #[serde(default, skip_serializing_if = "Tone::is_default")]
        tone: Tone,
        /// What the card's status spinner resolves into (`mark` channel).
        #[serde(default, skip_serializing_if = "Mark::is_check")]
        mark: Mark,
    },
    /// A sphere of glowing points that spins, breathes, and can shatter.
    #[serde(rename_all = "camelCase")]
    Orb {
        id: String,
        at: [f32; 3],
        radius: f32,
        #[serde(default = "default_points")]
        points: u32,
        #[serde(default = "accent")]
        tone: Tone,
    },
    /// A curved light connection between two positioned elements.
    #[serde(rename_all = "camelCase")]
    Beam {
        id: String,
        from: String,
        to: String,
        /// Sideways bow of the curve, in pixels.
        #[serde(default)]
        bend: f32,
        #[serde(default, skip_serializing_if = "Tone::is_default")]
        tone: Tone,
    },
    /// A glowing message that travels along a beam, with an optional label.
    #[serde(rename_all = "camelCase")]
    Packet {
        id: String,
        beam: String,
        /// Travel from the beam's `to` back to its `from`.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        reverse: bool,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        label: String,
        #[serde(default, skip_serializing_if = "Tone::is_default")]
        tone: Tone,
    },
    /// Styled text in the scene, seen through the camera.
    #[serde(rename_all = "camelCase")]
    Label {
        id: String,
        at: [f32; 3],
        size: f32,
        #[serde(default = "center", skip_serializing_if = "is_center")]
        align: CaptionAlign,
        spans: Vec<CaptionSpanPlan>,
    },
    /// A circle or arc: a timer when its sweep grows, a ripple when it expands.
    #[serde(rename_all = "camelCase")]
    Ring {
        id: String,
        at: [f32; 3],
        radius: f32,
        #[serde(default = "default_thickness")]
        thickness: f32,
        #[serde(default, skip_serializing_if = "Tone::is_default")]
        tone: Tone,
    },
}

fn default_points() -> u32 {
    720
}
fn default_thickness() -> f32 {
    3.0
}
fn accent() -> Tone {
    Tone::Accent
}
fn is_center(align: &CaptionAlign) -> bool {
    *align == CaptionAlign::Center
}
fn center() -> CaptionAlign {
    CaptionAlign::Center
}

/// Constructors with each kind's serialized defaults, and builder-style
/// options: `StageElement::card("client", at, size, "client").tone(Tone::Request)`.
/// An option on a kind that lacks it is a programming error and panics.
impl StageElement {
    /// A card with a title, no statuses, the plain tone, and a check mark.
    pub fn card(id: &str, at: [f32; 3], size: [f32; 2], title: &str) -> Self {
        Self::Card {
            id: id.into(),
            at,
            size,
            title: title.into(),
            status: Vec::new(),
            tone: Tone::default(),
            mark: Mark::default(),
        }
    }

    /// An orb of 720 points in the accent tone.
    pub fn orb(id: &str, at: [f32; 3], radius: f32) -> Self {
        Self::Orb {
            id: id.into(),
            at,
            radius,
            points: default_points(),
            tone: accent(),
        }
    }

    /// A straight, plain beam from one positioned element to another.
    pub fn beam(id: &str, from: &str, to: &str) -> Self {
        Self::Beam {
            id: id.into(),
            from: from.into(),
            to: to.into(),
            bend: 0.0,
            tone: Tone::default(),
        }
    }

    /// An unlabeled, plain packet that travels `beam` from its `from` end.
    pub fn packet(id: &str, beam: &str) -> Self {
        Self::Packet {
            id: id.into(),
            beam: beam.into(),
            reverse: false,
            label: String::new(),
            tone: Tone::default(),
        }
    }

    /// Centered text of `(text, tone)` spans.
    pub fn label(id: &str, at: [f32; 3], size: f32, spans: &[(&str, Tone)]) -> Self {
        Self::Label {
            id: id.into(),
            at,
            size,
            align: center(),
            spans: spans
                .iter()
                .map(|&(text, tone)| CaptionSpanPlan::new(text, tone))
                .collect(),
        }
    }

    /// A plain 3 px ring.
    pub fn ring(id: &str, at: [f32; 3], radius: f32) -> Self {
        Self::Ring {
            id: id.into(),
            at,
            radius,
            thickness: default_thickness(),
            tone: Tone::default(),
        }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        match &mut self {
            Self::Card { tone: own, .. }
            | Self::Orb { tone: own, .. }
            | Self::Beam { tone: own, .. }
            | Self::Packet { tone: own, .. }
            | Self::Ring { tone: own, .. } => *own = tone,
            other => panic!("stage element '{}' has no tone", other.id()),
        }
        self
    }

    /// A card's status lines, cross-faded by its `status` channel.
    pub fn statuses(mut self, statuses: &[(&str, Tone)]) -> Self {
        let Self::Card { status, .. } = &mut self else {
            panic!("only cards have statuses, not '{}'", self.id());
        };
        *status = statuses
            .iter()
            .map(|&(text, tone)| StatusText::new(text, tone))
            .collect();
        self
    }

    /// What a card's status spinner resolves into.
    pub fn mark(mut self, mark: Mark) -> Self {
        let Self::Card { mark: own, .. } = &mut self else {
            panic!("only cards have marks, not '{}'", self.id());
        };
        *own = mark;
        self
    }

    pub fn points(mut self, points: u32) -> Self {
        let Self::Orb { points: own, .. } = &mut self else {
            panic!("only orbs have points, not '{}'", self.id());
        };
        *own = points;
        self
    }

    /// A beam's sideways bow, in pixels.
    pub fn bend(mut self, bend: f32) -> Self {
        let Self::Beam { bend: own, .. } = &mut self else {
            panic!("only beams bend, not '{}'", self.id());
        };
        *own = bend;
        self
    }

    /// The packet travels from its beam's `to` end back to its `from`.
    pub fn reversed(mut self) -> Self {
        let Self::Packet { reverse, .. } = &mut self else {
            panic!("only packets reverse, not '{}'", self.id());
        };
        *reverse = true;
        self
    }

    /// The text a packet carries.
    pub fn labeled(mut self, text: &str) -> Self {
        let Self::Packet { label, .. } = &mut self else {
            panic!("only packets carry labels, not '{}'", self.id());
        };
        *label = text.into();
        self
    }

    pub fn align(mut self, align: CaptionAlign) -> Self {
        let Self::Label { align: own, .. } = &mut self else {
            panic!("only labels align, not '{}'", self.id());
        };
        *own = align;
        self
    }

    /// A ring's stroke, in pixels.
    pub fn thickness(mut self, thickness: f32) -> Self {
        let Self::Ring { thickness: own, .. } = &mut self else {
            panic!("only rings have a thickness, not '{}'", self.id());
        };
        *own = thickness;
        self
    }
}

impl StageElement {
    pub fn id(&self) -> &str {
        match self {
            Self::Card { id, .. }
            | Self::Orb { id, .. }
            | Self::Beam { id, .. }
            | Self::Packet { id, .. }
            | Self::Label { id, .. }
            | Self::Ring { id, .. } => id,
        }
    }

    /// World position of elements that have one (beams and packets derive theirs).
    pub fn anchor(&self) -> Option<[f32; 3]> {
        match self {
            Self::Card { at, .. }
            | Self::Orb { at, .. }
            | Self::Label { at, .. }
            | Self::Ring { at, .. } => Some(*at),
            Self::Beam { .. } | Self::Packet { .. } => None,
        }
    }

    /// The outline beams attach to, around the element's projected `center`, at
    /// its total on-screen `scale`.
    pub fn outline(&self, center: Vec2, scale: f32) -> Shape {
        match self {
            Self::Card { size, .. } => {
                Shape::Box(Box2::from_center_size(center, Vec2::from(*size) * scale))
            }
            Self::Orb { radius, .. } => Shape::Circle(Circle {
                center,
                radius: radius * scale,
            }),
            Self::Ring { radius, .. } => Shape::Circle(Circle {
                center,
                radius: radius * scale,
            }),
            Self::Label { .. } | Self::Beam { .. } | Self::Packet { .. } => Shape::Point(center),
        }
    }

    /// Channel properties this element reads (after its `<id>.` prefix).
    pub fn properties(&self) -> &'static [&'static str] {
        match self {
            Self::Card { .. } => &[
                "opacity", "x", "y", "z", "scale", "blur", "glow", "flash", "alarm", "dim",
                "status", "content", "cool", "damage", "glitch", "cut", "ghost", "spinner",
                "release", "mark",
            ],
            Self::Orb { .. } => &[
                "opacity", "x", "y", "z", "scale", "blur", "rotation", "burst", "shatter", "pulse",
                "hurt", "spin",
            ],
            Self::Beam { .. } => &[
                "opacity", "sweep", "port", "draw", "break", "flow", "emphasis", "surge", "twang",
            ],
            Self::Packet { .. } => &["opacity", "age", "flight"],
            Self::Label { .. } => &["opacity", "x", "y", "z", "scale", "typed"],
            Self::Ring { .. } => &["opacity", "x", "y", "z", "scale", "sweep", "expand"],
        }
    }

    /// The Stage's channel defaults: what each of [`Self::properties`] reads
    /// before anything writes it. `StageActor` declares a new channel at this
    /// value and the renderer falls back to it, so the two cannot disagree.
    /// A property added to `properties` needs its default here (a test checks).
    pub fn channel_defaults(&self) -> &'static [(&'static str, f32)] {
        match self {
            Self::Card { .. } => &[
                ("opacity", 1.0),
                ("x", 0.0),
                ("y", 0.0),
                ("z", 0.0),
                ("scale", 1.0),
                ("blur", 0.0),
                ("glow", 0.0),
                ("flash", 0.0),
                ("alarm", 0.0),
                ("dim", 0.0),
                ("status", 0.0),
                ("content", 1.0),
                ("cool", 0.0),
                ("damage", 0.0),
                ("glitch", 0.0),
                ("cut", 0.0),
                ("ghost", 0.0),
                ("spinner", -1.0),
                ("release", -1.0),
                ("mark", -1.0),
            ],
            Self::Orb { .. } => &[
                ("opacity", 1.0),
                ("x", 0.0),
                ("y", 0.0),
                ("z", 0.0),
                ("scale", 1.0),
                ("blur", 0.0),
                ("rotation", 0.0),
                ("burst", -1.0),
                ("shatter", 0.0),
                ("pulse", 0.0),
                ("hurt", 0.0),
                ("spin", 1.0),
            ],
            Self::Beam { .. } => &[
                ("opacity", 1.0),
                ("sweep", 0.0),
                ("port", 0.0),
                ("draw", 1.0),
                ("break", 0.0),
                ("flow", 0.0),
                ("emphasis", 0.0),
                ("surge", 0.0),
                ("twang", 0.0),
            ],
            Self::Packet { .. } => &[("opacity", 1.0), ("age", -1.0), ("flight", 0.8)],
            Self::Label { .. } => &[
                ("opacity", 1.0),
                ("x", 0.0),
                ("y", 0.0),
                ("z", 0.0),
                ("scale", 1.0),
                ("typed", 1.0),
            ],
            Self::Ring { .. } => &[
                ("opacity", 1.0),
                ("x", 0.0),
                ("y", 0.0),
                ("z", 0.0),
                ("scale", 1.0),
                ("sweep", 1.0),
                ("expand", 0.0),
            ],
        }
    }

    /// What `property` reads before anything writes it, if this element reads it.
    pub fn channel_default(&self, property: &str) -> Option<f32> {
        self.channel_defaults()
            .iter()
            .find(|(name, _)| *name == property)
            .map(|&(_, value)| value)
    }
}

/// Channels that belong to the whole stage rather than an element.
/// `camera.shake` is the trauma jolts write; `camera.quake` is sustained
/// trauma a scene ramps itself (up to 2 for overdrive). They add.
/// `post.zoom` streaks the developed frame toward its center (a radial blur);
/// `post.flash` washes it toward white (0..1), for impacts that blind.
pub const STAGE_PROPERTIES: [&str; 17] = [
    "camera.x",
    "camera.y",
    "camera.z",
    "camera.focus",
    "camera.dof",
    "camera.shake",
    "camera.quake",
    "camera.kick-x",
    "camera.kick-y",
    "camera.punch",
    "post.bloom",
    "post.chroma",
    "post.exposure",
    "post.vignette",
    "post.rewind",
    "post.zoom",
    "post.flash",
];

impl StagePost {
    /// The defaults of the [`STAGE_PROPERTIES`]: what each reads before
    /// anything writes it. `post.bloom` and `post.vignette` rest at this look.
    /// A stage property added above needs its default here (a test checks).
    pub fn channel_default(&self, property: &str) -> Option<f32> {
        Some(match property {
            "camera.x" | "camera.y" | "camera.z" | "camera.focus" | "camera.dof" => 0.0,
            "camera.shake" | "camera.quake" | "camera.kick-x" | "camera.kick-y" => 0.0,
            "camera.punch" => 0.0,
            "post.bloom" => self.bloom,
            "post.vignette" => self.vignette,
            "post.exposure" => 1.0,
            "post.rewind" => -1.0,
            "post.chroma" | "post.zoom" | "post.flash" => 0.0,
            _ => return None,
        })
    }
}

impl StagePlan {
    pub fn element(&self, id: &str) -> Option<&StageElement> {
        self.elements.iter().find(|element| element.id() == id)
    }

    /// What a stage channel (`camera.z`) or an element's (`client.opacity`)
    /// reads before anything writes it, from the one table of Stage channel
    /// defaults ([`StagePost::channel_default`], [`StageElement::channel_defaults`]).
    /// `None` for a property the Stage does not read.
    pub fn channel_default(&self, property: &str) -> Option<f32> {
        self.post.channel_default(property).or_else(|| {
            let (id, rest) = property.split_once('.')?;
            self.element(id)?.channel_default(rest)
        })
    }

    /// True when `property` names a stage channel or a property of an element.
    pub fn accepts(&self, property: &str) -> bool {
        if STAGE_PROPERTIES.contains(&property) {
            return true;
        }
        property.split_once('.').is_some_and(|(id, rest)| {
            self.element(id)
                .is_some_and(|element| element.properties().contains(&rest))
        })
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            (0.0..=4.0).contains(&self.post.bloom)
                && (0.0..=0.3).contains(&self.post.grain)
                && (0.0..=1.0).contains(&self.post.vignette)
                && (0.0..=1.0).contains(&self.post.backdrop),
            "stage post values are out of range"
        );
        ensure!(
            !self.elements.is_empty() && self.elements.len() <= 64,
            "a stage has one to 64 elements"
        );
        let mut ids = HashSet::new();
        for element in &self.elements {
            let id = element.id();
            if id.is_empty()
                || id.chars().any(|c| c.is_whitespace() || c == '.')
                || matches!(id, "camera" | "post")
            {
                bail!(
                    "stage element ID '{id}' must be non-empty, without whitespace or dots, and not camera/post"
                );
            }
            ensure!(ids.insert(id), "stage element '{id}' is declared twice");
            if let Some(at) = element.anchor() {
                ensure!(
                    at.iter().all(|v| v.is_finite()) && at[2] > -FOCAL * 0.8,
                    "stage element '{id}' must be in front of the camera"
                );
            }
            match element {
                StageElement::Card {
                    size,
                    title,
                    status,
                    ..
                } => {
                    ensure!(
                        (40.0..=1400.0).contains(&size[0]) && (30.0..=800.0).contains(&size[1]),
                        "card '{id}' size is out of range"
                    );
                    line(id, title, 48)?;
                    ensure!(status.len() <= 6, "card '{id}' has more than six statuses");
                    for entry in status {
                        line(id, &entry.text, 48)?;
                    }
                }
                StageElement::Orb { radius, points, .. } => {
                    ensure!(
                        (10.0..=600.0).contains(radius) && (8..=4000).contains(points),
                        "orb '{id}' needs a radius of 10..600 and 8..4000 points"
                    );
                }
                StageElement::Beam { from, to, bend, .. } => {
                    for end in [from, to] {
                        ensure!(
                            self.element(end).and_then(StageElement::anchor).is_some(),
                            "beam '{id}' must connect positioned elements; '{end}' is not one"
                        );
                    }
                    ensure!(
                        from != to && bend.is_finite(),
                        "beam '{id}' needs two different ends"
                    );
                }
                StageElement::Packet { beam, label, .. } => {
                    ensure!(
                        matches!(self.element(beam), Some(StageElement::Beam { .. })),
                        "packet '{id}' must travel an existing beam"
                    );
                    line(id, label, 40)?;
                }
                StageElement::Label { size, spans, .. } => {
                    ensure!(
                        (10.0..=160.0).contains(size),
                        "label '{id}' size is out of range"
                    );
                    ensure!(
                        spans.iter().any(|span| !span.text.is_empty()),
                        "label '{id}' needs text"
                    );
                    for span in spans {
                        line(id, &span.text, 120)?;
                    }
                }
                StageElement::Ring {
                    radius, thickness, ..
                } => {
                    ensure!(
                        (2.0..=900.0).contains(radius) && (0.5..=80.0).contains(thickness),
                        "ring '{id}' radius or thickness is out of range"
                    );
                }
            }
        }
        Ok(())
    }
}

fn line(id: &str, text: &str, max: usize) -> Result<()> {
    ensure!(
        !text.contains(['\n', '\r']) && text.chars().count() <= max,
        "stage element '{id}' text must be one line of at most {max} characters"
    );
    Ok(())
}

/// A sampled camera: where it looks and how far it has moved toward the scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Pan in x/y; z dollies toward the scene, making the z = 0 plane larger.
    pub position: Vec3,
    /// The frame size in pixels.
    pub size: Vec2,
}

impl Camera {
    /// Screen position and scale of a world point, or `None` behind the camera.
    pub fn project(&self, point: Vec3) -> Option<(Vec2, f32)> {
        let depth = FOCAL + point.z - self.position.z;
        if depth <= 1.0 {
            return None;
        }
        let scale = FOCAL / depth;
        let center = self.size * 0.5;
        Some((
            center + (point.truncate() - center - self.position.truncate()) * scale,
            scale,
        ))
    }
}

/// One point of an orb's shell and the seeds that shape its shatter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbPoint {
    pub unit: Vec3,
    pub seed: Vec3,
}

pub fn orb_points(count: u32) -> Vec<OrbPoint> {
    fibonacci_sphere(count)
        .into_iter()
        .zip(0..)
        .map(|(unit, i)| OrbPoint {
            unit,
            seed: vec3(hash(i, 3), hash(i, 7), hash(i, 11)),
        })
        .collect()
}

/// Where a shell point sits relative to the orb's center, before projection.
/// Points burst outward by different amounts, then fall; `shatter` runs 0..1.
pub fn shatter_offset(point: OrbPoint, radius: f32, shatter: f32) -> Vec3 {
    let seed = point.seed;
    let burst = 1.0 + shatter * (0.6 + 2.4 * seed.x);
    let fall = shatter * shatter * (160.0 + 460.0 * seed.y);
    let drift = shatter * (seed.z - 0.5) * 120.0;
    point.unit * (radius * burst) + vec3(drift, fall, 0.0)
}

/// A packet's life, derived from one dispatch clock (`age`, in seconds) and its
/// flight time, so every phase is exact at any sample time. Light gathers at the
/// start port, the packet flies on an acceleration-continuous quintic ease,
/// then it is absorbed as a small ring while its trail cools.
pub mod packet {
    use crate::math::easing::{smootherstep, smootherstep_inverse};

    pub const GATHER: f32 = 0.34;
    pub const LANDING: f32 = 0.72;
    /// How long the trail takes to cool behind the packet.
    pub const COOLING: f32 = 0.45;
    /// How long the glow left at the start port lasts.
    pub const EMBER: f32 = 2.2;
    /// How long light floods in from the end port.
    pub const FLOOD: f32 = 1.2;
    /// Every phase has finished by this age.
    pub const LIFETIME: f32 = 4.0;

    /// Progress (0..1) through the gather at `age`, if it is gathering.
    pub fn gather(age: f32) -> Option<f32> {
        window(age, 0.0, GATHER)
    }

    /// Progress (0..1) through the flight, before easing, if it is flying.
    pub fn flight(age: f32, flight: f32) -> Option<f32> {
        window(age, GATHER, flight)
    }

    /// Progress (0..1) through the landing, if it is landing.
    pub fn landing(age: f32, flight: f32) -> Option<f32> {
        window(age, GATHER + flight, LANDING)
    }

    /// Seconds since the packet arrived, if it has.
    pub fn since_arrival(age: f32, flight: f32) -> Option<f32> {
        (age >= GATHER + flight).then_some(age - GATHER - flight)
    }

    /// Where the packet is along its beam, as a fraction of the length.
    pub fn travel(age: f32, flight: f32) -> f32 {
        smootherstep(((age - GATHER) / flight).clamp(0.0, 1.0))
    }

    /// Seconds since the packet crossed the point at `fraction` of its beam, if
    /// it has reached it.
    pub fn since_crossing(age: f32, flight: f32, fraction: f32) -> Option<f32> {
        let crossed = GATHER + flight * smootherstep_inverse(fraction);
        (age >= crossed).then_some(age - crossed)
    }

    /// Heat of the trail at a point crossed `since` seconds ago.
    pub fn heat(since: f32) -> f32 {
        0.7 * (1.0 - (since / COOLING).clamp(0.0, 1.0)).powf(1.7)
    }

    fn window(age: f32, start: f32, length: f32) -> Option<f32> {
        (age >= start && age < start + length).then(|| (age - start) / length)
    }
}

/// Wire draw-on, after the blog diagrams: the port pops, then the wire draws
/// with a gentle start and stop. (A frame `sweep` before it is opt-in.)
pub const PORT_POP_SECONDS: f32 = 0.3;
pub const DRAW_CURVE: Ease = Ease::CubicBezier([0.45, 0.0, 0.2, 1.0]);

/// How long a receiver takes to react once a request has landed, before its
/// reply starts to gather.
pub const REACT_SECONDS: f32 = 0.08;

/// The earliest launch of a reply to a packet that arrives at `arrival`:
/// reaction follows contact, so even the reply's gather waits for the arrival
/// (340 ms gather plus an 80 ms beat). Pass it to [`StageActor::send`].
pub fn reply_after(arrival: u64) -> u64 {
    arrival + whole_millis(packet::GATHER) + whole_millis(REACT_SECONDS)
}

/// Authoring handle: declares each stage channel once, on first use.
pub struct StageActor {
    actor: ActorHandle,
    /// The declared recipe, for helpers that follow a beam to its ends.
    plan: StagePlan,
}

impl StageActor {
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: &StagePlan,
    ) -> Result<Self> {
        plan.validate()?;
        let actor = scene.actor(id, STAGE_RECIPE, plan)?;
        Ok(Self {
            actor,
            plan: plan.clone(),
        })
    }

    /// The channel for `property`, declared on first use with `initial`.
    pub fn channel(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        scene.channel(&self.actor, property, initial)
    }

    /// The channel for `property`, declared on first use at the value the
    /// renderer reads when nothing writes it ([`StagePlan::channel_default`]).
    /// An unknown property starts at 0; the renderer's preflight rejects it.
    fn resting(&mut self, scene: &mut PlanBuilder, property: &str) -> ContinuousHandle {
        let initial = self.plan.channel_default(property).unwrap_or(0.0);
        self.channel(scene, property, initial)
    }

    /// Spring `property` to `target` at `at_nanos`. Channels not declared
    /// with [`Self::channel`] start at their resting value, the Stage channel
    /// default (opacity 1, burst -1, most others 0): declare another starting
    /// pose, such as an opacity of 0 to fade in, with `channel`.
    pub fn to(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        at_nanos: u64,
        target: f32,
        seconds: f32,
    ) {
        self.bounce(scene, property, at_nanos, target, seconds, 0.0);
    }

    /// Spring `property` to `target` with a named feel, such as
    /// [`SpringPlan::CAMERA`] or [`SpringPlan::PANEL`].
    pub fn spring(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        at_nanos: u64,
        target: f32,
        feel: SpringPlan,
    ) {
        let channel = self.resting(scene, property);
        scene.spring_with(&channel, at_nanos, target, feel);
    }

    /// Like `to`, with overshoot: `bounce` 0.2 reads as a lively landing.
    pub fn bounce(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        at_nanos: u64,
        target: f32,
        seconds: f32,
        bounce: f32,
    ) {
        let channel = self.resting(scene, property);
        scene.spring(&channel, at_nanos, target, seconds, bounce);
    }

    /// Ease `property` to `target` over `seconds` along `curve`.
    pub fn ease(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        at_nanos: u64,
        target: f32,
        seconds: f32,
        curve: Ease,
    ) {
        let channel = self.resting(scene, property);
        scene.ease(&channel, at_nanos, target, seconds, curve);
    }

    /// Fade `element` in to `opacity` on a `seconds` spring. It starts hidden:
    /// the first write declares its opacity at 0, though the Stage rests visible.
    pub fn fade_in(
        &mut self,
        scene: &mut PlanBuilder,
        element: &str,
        at_nanos: u64,
        opacity: f32,
        seconds: f32,
    ) {
        let channel = self.channel(scene, &format!("{element}.opacity"), 0.0);
        scene.spring(&channel, at_nanos, opacity, seconds, 0.0);
    }

    /// Fade `element` out on a `seconds` spring.
    pub fn fade_out(
        &mut self,
        scene: &mut PlanBuilder,
        element: &str,
        at_nanos: u64,
        seconds: f32,
    ) {
        self.to(scene, &format!("{element}.opacity"), at_nanos, 0.0, seconds);
    }

    /// Jump `property` to `value` at `at_nanos`.
    pub fn set(&mut self, scene: &mut PlanBuilder, property: &str, at_nanos: u64, value: f32) {
        let channel = self.resting(scene, property);
        scene.set(&channel, at_nanos, value);
    }

    /// Start a clock of elapsed seconds at `at_nanos` that runs to the end of
    /// the scene; before it starts the channel reads -1 ("not yet"). Effect
    /// rigs sample their own poses from it.
    pub fn clock(&mut self, scene: &mut PlanBuilder, property: &str, at_nanos: u64) {
        // Whole milliseconds, as `ease` durations are, so the slope is exactly 1.
        let seconds = (scene.duration_nanos().saturating_sub(at_nanos) / 1_000_000) as f32 / 1000.0;
        self.clock_for(scene, property, at_nanos, seconds);
    }

    /// Like [`Self::clock`], but the clock stops at `seconds`, as for an effect
    /// with a fixed lifetime (a packet, a burst, a rewind).
    pub fn clock_for(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        at_nanos: u64,
        seconds: f32,
    ) {
        let channel = self.channel(scene, property, -1.0);
        scene.set(&channel, at_nanos, 0.0);
        if seconds > 0.0 {
            scene.ease(&channel, at_nanos, seconds, seconds, Ease::Linear);
        }
    }

    /// Light `property` to `peak` at once, then let it decay to `rest`, fast
    /// and then with a long tail: how a flash, a hit, or a pulse behaves.
    pub fn hit(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        at_nanos: u64,
        peak: f32,
        rest: f32,
    ) {
        let channel = self.channel(scene, property, rest);
        scene.set(&channel, at_nanos, peak);
        scene.ease(&channel, at_nanos, rest, 0.8, Ease::CubicOut);
    }

    /// Shove the `x`/`y` channel pair by `offset` in about two frames, then let
    /// it spring back past rest and settle: a hit with weight.
    pub fn kick(
        &mut self,
        scene: &mut PlanBuilder,
        [x, y]: [&str; 2],
        at_nanos: u64,
        offset: [f32; 2],
    ) {
        for (property, amount) in [(x, offset[0]), (y, offset[1])] {
            self.to(scene, property, at_nanos, amount, 0.04);
            self.bounce(scene, property, at_nanos + 40_000_000, 0.0, 0.6, 0.35);
        }
    }

    /// An impact jolts the camera: the frame is shoved along `direction` (the
    /// way the blow pushes the scene) and rebounds, a trauma rumble decays, and
    /// the frame punches in slightly. `strength` 1 is a full hit.
    pub fn jolt(
        &mut self,
        scene: &mut PlanBuilder,
        at_nanos: u64,
        direction: [f32; 2],
        strength: f32,
    ) {
        let length = direction[0].hypot(direction[1]).max(1e-6);
        // The camera moves against the push, so the scene moves with it.
        let shove = -16.0 * strength / length;
        self.kick(
            scene,
            ["camera.kick-x", "camera.kick-y"],
            at_nanos,
            [direction[0] * shove, direction[1] * shove],
        );
        self.hit(scene, "camera.shake", at_nanos, strength.min(1.0), 0.0);
        self.hit(scene, "camera.punch", at_nanos, 0.022 * strength, 0.0);
    }

    /// A rigid panel settles with a small vertical drift and restrained scale.
    /// Its content follows 65 ms later, so the body leads and the ink settles.
    /// Returns the time the panel is ready to connect.
    pub fn settle_in(&mut self, scene: &mut PlanBuilder, card: &str, at_nanos: u64) -> u64 {
        let scale = self.channel(scene, &format!("{card}.scale"), 1.035);
        scene.set(&scale, at_nanos, 1.035);
        scene.spring(&scale, at_nanos, 1.0, 0.6, 0.12);
        let y = self.channel(scene, &format!("{card}.y"), 16.0);
        scene.set(&y, at_nanos, 16.0);
        scene.spring(&y, at_nanos, 0.0, 0.55, 0.16);
        let content = self.channel(scene, &format!("{card}.content"), 0.0);
        scene.set(&content, at_nanos, 0.0);
        scene.spring(&content, at_nanos + 65_000_000, 1.0, 0.36, 0.0);
        for (property, from, to, seconds) in [("opacity", 0.0, 1.0, 0.18), ("blur", 3.0, 0.0, 0.3)]
        {
            let channel = self.channel(scene, &format!("{card}.{property}"), from);
            scene.set(&channel, at_nanos, from);
            scene.ease(&channel, at_nanos, to, seconds, Ease::Smootherstep);
        }
        at_nanos + 500_000_000
    }

    /// Type a label in at `chars_per_second`, one exact step per character.
    /// The label becomes visible when typing starts. Returns when it finishes.
    pub fn type_in(
        &mut self,
        scene: &mut PlanBuilder,
        label: &str,
        at_nanos: u64,
        chars_per_second: f32,
    ) -> u64 {
        let chars = match self.plan.element(label) {
            Some(StageElement::Label { spans, .. }) => {
                spans.iter().map(|span| span.text.chars().count()).sum()
            }
            _ => 0,
        }
        .max(1);
        let opacity = self.channel(scene, &format!("{label}.opacity"), 0.0);
        let typed = self.channel(scene, &format!("{label}.typed"), 0.0);
        scene.set(&opacity, at_nanos, 1.0);
        caption::type_steps(scene, &typed, at_nanos, chars, chars_per_second)
    }

    /// Send a packet so that it launches at `at_nanos` and flies for `seconds`.
    /// Light gathers at its port just before, and after arriving it lands as a
    /// small ring while its trail cools. Returns the arrival time.
    pub fn send(
        &mut self,
        scene: &mut PlanBuilder,
        packet: &str,
        at_nanos: u64,
        seconds: f32,
    ) -> u64 {
        let dispatch = at_nanos.saturating_sub(whole_millis(packet::GATHER));
        // The clock runs at real speed until every phase has finished.
        self.clock_for(scene, &format!("{packet}.age"), dispatch, packet::LIFETIME);
        let flight = self.channel(scene, &format!("{packet}.flight"), seconds);
        scene.set(&flight, dispatch, seconds);
        dispatch + whole_millis(packet::GATHER) + whole_millis(seconds)
    }

    /// Send `packet` so that it arrives at `arrival` after flying for
    /// `seconds`, as when a hit must land on a spoken word: it launches one
    /// flight earlier and gathers before that. Returns the arrival time.
    pub fn send_arriving(
        &mut self,
        scene: &mut PlanBuilder,
        packet: &str,
        arrival: u64,
        seconds: f32,
    ) -> u64 {
        self.send(
            scene,
            packet,
            arrival.saturating_sub(whole_millis(seconds)),
            seconds,
        )
    }

    /// Plug `beam` in so that the wire reaches its far end at `contact`,
    /// drawing for `seconds` after the port resolves. Returns the contact time.
    pub fn connect_contacting(
        &mut self,
        scene: &mut PlanBuilder,
        beam: &str,
        contact: u64,
        seconds: f32,
    ) -> u64 {
        let lead = whole_millis(PORT_POP_SECONDS) + whole_millis(seconds);
        self.connect(scene, beam, contact.saturating_sub(lead), seconds)
    }

    /// Plug `beam` in, starting at `at_nanos`: the port resolves softly and
    /// the wire draws over `seconds`, then stays still. Returns the contact
    /// time. Packets, impacts, twangs, and flow are separate authored actions.
    pub fn connect(
        &mut self,
        scene: &mut PlanBuilder,
        beam: &str,
        at_nanos: u64,
        seconds: f32,
    ) -> u64 {
        let port = self.channel(scene, &format!("{beam}.port"), 0.0);
        scene.ease(&port, at_nanos, 1.0, PORT_POP_SECONDS, Ease::Smootherstep);
        let start = at_nanos + whole_millis(PORT_POP_SECONDS);
        let draw = self.channel(scene, &format!("{beam}.draw"), 0.0);
        scene.set(&draw, start, 0.0);
        scene.ease(&draw, start, 1.0, seconds, DRAW_CURVE);
        start + whole_millis(seconds)
    }

    /// `beam` is struck like a cable: it bows out over a few frames, then its
    /// momentum carries into an underdamped spring that vibrates back to rest.
    pub fn twang(&mut self, scene: &mut PlanBuilder, beam: &str, at_nanos: u64) {
        let twang = self.channel(scene, &format!("{beam}.twang"), 0.0);
        scene.spring(&twang, at_nanos, 1.0, 0.09, 0.0);
        scene.spring(&twang, at_nanos + 90_000_000, 0.0, 0.42, 0.28);
    }

    /// Something arrives at `element`: a card's ink flashes, an orb lights up.
    /// Nothing scales on a hit; the arrival's own light floods in from its port.
    pub fn land(&mut self, scene: &mut PlanBuilder, element: &str, at_nanos: u64) {
        match self.plan.element(element) {
            Some(StageElement::Card { .. }) => {
                self.hit(scene, &format!("{element}.flash"), at_nanos, 0.6, 0.0);
            }
            Some(StageElement::Orb { .. }) => {
                self.hit(scene, &format!("{element}.pulse"), at_nanos, 0.6, 0.0);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> StagePlan {
        serde_json::from_value(serde_json::json!({
            "elements": [
                { "kind": "orb", "id": "service", "at": [960, 460, 0], "radius": 150 },
                { "kind": "card", "id": "client", "at": [420, 300, -40], "size": [300, 120], "title": "client",
                  "status": [{ "text": "reconnecting" }, { "text": "disconnected", "tone": "error" }] },
                { "kind": "beam", "id": "link", "from": "client", "to": "service", "bend": 60 },
                { "kind": "packet", "id": "probe", "beam": "link", "label": "GET /api/info", "tone": "request" },
                { "kind": "label", "id": "caption", "at": [960, 700, 0], "size": 28,
                  "spans": [{ "text": "healthy", "tone": "success" }] },
                { "kind": "ring", "id": "timer", "at": [960, 460, 0], "radius": 190 }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn stage_plans_validate_and_round_trip() {
        let plan = plan();
        plan.validate().unwrap();
        let json = serde_json::to_value(&plan).unwrap();
        assert_eq!(
            json["elements"][0]["tone"], "accent",
            "orbs default to the accent"
        );
        assert_eq!(serde_json::from_value::<StagePlan>(json).unwrap(), plan);
        assert!(
            plan.accepts("camera.z")
                && plan.accepts("service.shatter")
                && plan.accepts("probe.age")
        );
        assert!(
            !plan.accepts("probe.travel")
                && !plan.accepts("missing.opacity")
                && !plan.accepts("camera.roll")
        );
    }

    #[test]
    fn constructors_carry_the_serialized_defaults() {
        let built = vec![
            StageElement::orb("service", [960.0, 460.0, 0.0], 150.0),
            StageElement::card("client", [420.0, 300.0, -40.0], [300.0, 120.0], "client")
                .statuses(&[("reconnecting", Tone::Plain), ("disconnected", Tone::Error)]),
            StageElement::beam("link", "client", "service").bend(60.0),
            StageElement::packet("probe", "link")
                .labeled("GET /api/info")
                .tone(Tone::Request),
            StageElement::label(
                "caption",
                [960.0, 700.0, 0.0],
                28.0,
                &[("healthy", Tone::Success)],
            ),
            StageElement::ring("timer", [960.0, 460.0, 0.0], 190.0),
        ];
        assert_eq!(built, plan().elements);
        let options = StageElement::card("c", [0.0; 3], [100.0, 50.0], "c")
            .mark(Mark::Cross)
            .tone(Tone::Accent);
        assert!(matches!(
            options,
            StageElement::Card {
                mark: Mark::Cross,
                tone: Tone::Accent,
                ..
            }
        ));
        assert!(matches!(
            StageElement::packet("p", "link").reversed(),
            StageElement::Packet { reverse: true, .. }
        ));
    }

    #[test]
    #[should_panic(expected = "only orbs have points")]
    fn an_option_on_the_wrong_kind_panics() {
        let _ = StageElement::ring("timer", [0.0; 3], 10.0).points(9);
    }

    /// One element of every kind, so each kind's table is checked.
    fn every_kind() -> Vec<StageElement> {
        let plan = plan();
        let kinds = ["card", "orb", "beam", "packet", "label", "ring"];
        let elements = plan.elements;
        for kind in kinds {
            assert!(
                elements
                    .iter()
                    .any(|e| serde_json::to_value(e).unwrap()["kind"] == kind),
                "the test plan lacks a {kind}"
            );
        }
        elements
    }

    #[test]
    fn every_stage_property_has_exactly_one_default() {
        let plan = plan();
        for property in STAGE_PROPERTIES {
            assert!(
                plan.channel_default(property).is_some(),
                "stage property '{property}' has no default"
            );
        }
        for element in every_kind() {
            let defaults = element.channel_defaults();
            let names = defaults.iter().map(|(name, _)| *name).collect::<Vec<_>>();
            let properties = element.properties();
            assert_eq!(
                names.iter().collect::<HashSet<_>>(),
                properties.iter().collect::<HashSet<_>>(),
                "'{}' defaults must name exactly its properties",
                element.id()
            );
            assert_eq!(names.len(), properties.len(), "a property is listed twice");
            for property in properties {
                let id = format!("{}.{property}", element.id());
                assert_eq!(plan.channel_default(&id), element.channel_default(property));
            }
        }
        assert_eq!(plan.channel_default("camera.roll"), None);
        assert_eq!(
            plan.channel_default("client.age"),
            None,
            "cards have no age"
        );
        assert_eq!(plan.channel_default("missing.opacity"), None);
    }

    #[test]
    fn defaults_are_resting_poses() {
        let plan = plan();
        for (property, rest) in [
            ("client.opacity", 1.0),
            ("client.content", 1.0),
            ("client.mark", -1.0),
            ("service.burst", -1.0),
            ("service.spin", 1.0),
            ("link.draw", 1.0),
            ("probe.age", -1.0),
            ("probe.flight", 0.8),
            ("caption.typed", 1.0),
            ("timer.sweep", 1.0),
            ("camera.z", 0.0),
            ("post.exposure", 1.0),
            ("post.rewind", -1.0),
            ("post.bloom", StagePost::default().bloom),
        ] {
            assert_eq!(plan.channel_default(property), Some(rest), "{property}");
        }
        let custom = StagePlan {
            post: StagePost {
                vignette: 0.22,
                ..StagePost::default()
            },
            ..plan
        };
        assert_eq!(custom.channel_default("post.vignette"), Some(0.22));
    }

    #[test]
    fn undeclared_channels_start_at_their_default() {
        let mut scene = PlanBuilder::new("stage-demo", 5_000_000_000);
        let mut stage = StageActor::declare(&mut scene, "stage", &plan()).unwrap();
        stage.to(&mut scene, "link.opacity", 1_000_000_000, 0.0, 0.5);
        stage.set(&mut scene, "client.mark", 1_000_000_000, -1.0);
        stage.ease(&mut scene, "camera.x", 0, 40.0, 1.0, Ease::Smootherstep);
        stage.channel(&mut scene, "service.opacity", 0.0);
        stage.to(&mut scene, "service.opacity", 0, 1.0, 0.5);
        let plan = scene.finish().unwrap();
        let initial = |property: &str| match plan
            .continuous_channels
            .iter()
            .find(|c| c.property == property)
            .unwrap()
            .initial
        {
            crate::plan::ScalarPlan::Literal(value) => value,
            _ => unreachable!(),
        };
        assert_eq!(
            initial("link.opacity"),
            1.0,
            "a wire fading out was visible"
        );
        assert_eq!(initial("client.mark"), -1.0);
        assert_eq!(initial("camera.x"), 0.0);
        assert_eq!(initial("service.opacity"), 0.0, "a declared pose wins");
    }

    #[test]
    fn invalid_references_are_rejected() {
        let mut dangling = plan();
        if let StageElement::Beam { to, .. } = &mut dangling.elements[2] {
            *to = "nowhere".into();
        }
        assert!(dangling.validate().is_err());
        let mut packet_on_card = plan();
        if let StageElement::Packet { beam, .. } = &mut packet_on_card.elements[3] {
            *beam = "client".into();
        }
        assert!(packet_on_card.validate().is_err());
        let mut reserved = plan();
        if let StageElement::Ring { id, .. } = &mut reserved.elements[5] {
            *id = "camera".into();
        }
        assert!(reserved.validate().is_err());
    }

    #[test]
    fn the_default_camera_is_pixel_exact_at_depth_zero() {
        use crate::math::vec2;
        let camera = Camera {
            position: Vec3::ZERO,
            size: vec2(1920.0, 1080.0),
        };
        assert_eq!(
            camera.project(vec3(300.0, 200.0, 0.0)),
            Some((vec2(300.0, 200.0), 1.0))
        );
        let (far, scale) = camera.project(vec3(300.0, 200.0, 700.0)).unwrap();
        assert!(
            scale < 1.0 && far.x > 300.0,
            "farther points shrink toward the center"
        );
        let dolly = Camera {
            position: vec3(0.0, 0.0, 700.0),
            ..camera
        };
        assert!(dolly.project(vec3(300.0, 200.0, 0.0)).unwrap().1 > 1.0);
        assert!(camera.project(vec3(0.0, 0.0, -FOCAL)).is_none());
    }

    #[test]
    fn label_alignment_survives_plan_serialization() {
        for align in [
            CaptionAlign::Left,
            CaptionAlign::Center,
            CaptionAlign::Right,
        ] {
            let label = StageElement::Label {
                id: "name".into(),
                at: [960.0, 640.0, 0.0],
                size: 24.0,
                align,
                spans: vec![CaptionSpanPlan::new("service", Tone::Plain)],
            };
            let json = serde_json::to_value(&label).unwrap();
            assert_eq!(serde_json::from_value::<StageElement>(json).unwrap(), label);
        }
    }

    #[test]
    fn orbs_shatter_deterministically() {
        let points = orb_points(64);
        assert_eq!(points, orb_points(64));
        let point = points[5];
        assert_eq!(shatter_offset(point, 100.0, 0.0), point.unit * 100.0);
        let burst = shatter_offset(point, 100.0, 1.0);
        assert!(
            burst.y > point.unit.y * 100.0 + 100.0,
            "shattered points fall"
        );
    }

    #[test]
    fn beams_attach_to_the_facing_side_of_a_card_and_an_orb_outline() {
        use crate::math::{shapes::connect, vec2};
        let plan = plan();
        let card = plan
            .element("client")
            .unwrap()
            .outline(vec2(420.0, 300.0), 1.0);
        let orb = plan
            .element("service")
            .unwrap()
            .outline(vec2(960.0, 460.0), 1.0);
        let curve = connect(card, orb, 0.0);
        assert_eq!(curve.start, vec2(570.0, 300.0), "the card's right side");
        assert!((curve.end.distance(vec2(960.0, 460.0)) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn packets_gather_fly_and_land_on_one_clock() {
        let mut scene = PlanBuilder::new("stage-demo", 5_000_000_000);
        let mut stage = StageActor::declare(&mut scene, "stage", &plan()).unwrap();
        let arrival = stage.send(&mut scene, "probe", 1_000_000_000, 0.8);
        assert_eq!(
            arrival, 1_800_000_000,
            "it launches on time; the gather comes first"
        );
        let plan = scene.finish().unwrap();
        let age = plan
            .continuous_channels
            .iter()
            .find(|c| c.property == "probe.age")
            .unwrap();
        assert!(matches!(age.initial, crate::plan::ScalarPlan::Literal(value) if value == -1.0));
        assert_eq!(
            age.events[0].at_nanos(),
            660_000_000,
            "dispatched one gather early"
        );
        use packet::*;
        assert_eq!(gather(0.17), Some(0.5));
        assert_eq!((flight(0.34, 0.8), travel(0.34, 0.8)), (Some(0.0), 0.0));
        assert!(
            (travel(0.74, 0.8) - 0.5).abs() < 1e-6,
            "halfway in time is halfway along"
        );
        assert_eq!(landing(1.14, 0.8), Some(0.0));
        assert_eq!(since_crossing(0.74, 0.8, 0.9), None, "not there yet");
        let since = since_crossing(0.74, 0.8, 0.5).unwrap();
        assert!(
            since.abs() < 1e-5 && (heat(since) - 0.7).abs() < 1e-4,
            "hottest at the head"
        );
        assert_eq!(heat(COOLING), 0.0);
    }

    #[test]
    fn beats_can_be_timed_by_where_they_land() {
        let mut scene = PlanBuilder::new("stage-demo", 5_000_000_000);
        let mut stage = StageActor::declare(&mut scene, "stage", &plan()).unwrap();
        let word = 2_000_000_000;
        assert_eq!(stage.send_arriving(&mut scene, "probe", word, 0.8), word);
        let contact = stage.connect_contacting(&mut scene, "link", word, 0.6);
        assert_eq!(contact, word, "port 0.3 s plus draw 0.6 s before the word");
        assert_eq!(
            reply_after(word),
            word + 420_000_000,
            "a 340 ms gather and an 80 ms reaction"
        );
        let plan = scene.finish().unwrap();
        let first = |property: &str| {
            plan.continuous_channels
                .iter()
                .find(|c| c.property == property)
                .unwrap()
                .events[0]
                .at_nanos()
        };
        assert_eq!(first("probe.age"), word - 800_000_000 - 340_000_000);
        assert_eq!(first("link.port"), word - 900_000_000);
    }

    #[test]
    fn connecting_draws_once_then_rests_without_implicit_impacts_or_flow() {
        let mut scene = PlanBuilder::new("stage-demo", 5_000_000_000);
        let mut stage = StageActor::declare(&mut scene, "stage", &plan()).unwrap();
        let contact = stage.connect(&mut scene, "link", 1_000_000_000, 0.6);
        assert_eq!(contact, 1_900_000_000, "port 0.3 s, draw 0.6 s");
        let plan = scene.finish().unwrap();
        let channel = |property: &str| {
            plan.continuous_channels
                .iter()
                .find(|c| c.property == property)
                .unwrap_or_else(|| panic!("missing {property}"))
        };
        assert!(matches!(
            channel("link.draw").events[1],
            crate::plan::TrackEventPlan::Ease {
                at_nanos: 1_300_000_000,
                duration_nanos: 600_000_000,
                curve: DRAW_CURVE,
                ..
            }
        ));
        channel("link.port");
        for property in ["link.surge", "link.twang", "link.flow", "service.pulse"] {
            assert!(
                !plan
                    .continuous_channels
                    .iter()
                    .any(|c| c.property == property)
            );
        }
    }

    #[test]
    fn hits_strike_at_once_and_decay() {
        let mut scene = PlanBuilder::new("stage-demo", 5_000_000_000);
        let mut stage = StageActor::declare(&mut scene, "stage", &plan()).unwrap();
        stage.hit(&mut scene, "client.flash", 1_000_000_000, 1.0, 0.0);
        let landing = stage.settle_in(&mut scene, "client", 2_000_000_000);
        assert_eq!(landing, 2_500_000_000);
        let plan = scene.finish().unwrap();
        let flash = plan
            .continuous_channels
            .iter()
            .find(|c| c.property == "client.flash")
            .unwrap();
        assert!(matches!(
            flash.events[0],
            crate::plan::TrackEventPlan::Set {
                at_nanos: 1_000_000_000,
                ..
            }
        ));
        assert!(matches!(
            flash.events[1],
            crate::plan::TrackEventPlan::Ease {
                curve: Ease::CubicOut,
                ..
            }
        ));
    }
}
