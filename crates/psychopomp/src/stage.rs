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
    effects::{dissolve, lightning, shield, spinner::Mark},
    math::{
        Vec2, Vec3,
        easing::Ease,
        random::hash,
        shapes::{Box2, Circle, Shape, fibonacci_sphere},
        vec3,
    },
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatusText {
    pub text: String,
    #[serde(default, skip_serializing_if = "Tone::is_default")]
    pub tone: Tone,
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
    /// Lightning between two positioned elements (or shields) or world
    /// points. Its `age` clock runs a stepped leader, `strikes` strobing
    /// return strokes that re-roll the path, contact sparks, and afterglow;
    /// `hum` keeps a writhing arc alive.
    #[serde(rename_all = "camelCase")]
    Bolt {
        id: String,
        from: BoltEnd,
        to: BoltEnd,
        #[serde(default = "default_strikes")]
        strikes: u32,
        /// Forks per stroke, about six at 1.
        #[serde(default = "default_branching")]
        branching: f32,
        #[serde(default = "request")]
        tone: Tone,
    },
    /// A forcefield bubble around a positioned element: faint hexagonal
    /// cells that ripple wherever a packet crosses it or a bolt strikes it.
    #[serde(rename_all = "camelCase")]
    Shield {
        id: String,
        around: String,
        radius: f32,
        #[serde(default = "accent")]
        tone: Tone,
    },
}

/// One end of a bolt: a positioned element or shield by ID, or a world point.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum BoltEnd {
    Element(String),
    Point([f32; 3]),
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
fn request() -> Tone {
    Tone::Request
}
fn default_strikes() -> u32 {
    3
}
fn default_branching() -> f32 {
    0.6
}
fn is_center(align: &CaptionAlign) -> bool {
    *align == CaptionAlign::Center
}
fn center() -> CaptionAlign {
    CaptionAlign::Center
}

impl StageElement {
    pub fn id(&self) -> &str {
        match self {
            Self::Card { id, .. }
            | Self::Orb { id, .. }
            | Self::Beam { id, .. }
            | Self::Packet { id, .. }
            | Self::Label { id, .. }
            | Self::Ring { id, .. }
            | Self::Bolt { id, .. }
            | Self::Shield { id, .. } => id,
        }
    }

    /// World position of elements that have one (beams and packets derive theirs).
    pub fn anchor(&self) -> Option<[f32; 3]> {
        match self {
            Self::Card { at, .. }
            | Self::Orb { at, .. }
            | Self::Label { at, .. }
            | Self::Ring { at, .. } => Some(*at),
            Self::Beam { .. } | Self::Packet { .. } | Self::Bolt { .. } | Self::Shield { .. } => {
                None
            }
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
            Self::Ring { radius, .. } | Self::Shield { radius, .. } => Shape::Circle(Circle {
                center,
                radius: radius * scale,
            }),
            Self::Label { .. } | Self::Beam { .. } | Self::Packet { .. } | Self::Bolt { .. } => {
                Shape::Point(center)
            }
        }
    }

    /// Channel properties this element reads (after its `<id>.` prefix).
    pub fn properties(&self) -> &'static [&'static str] {
        match self {
            Self::Card { .. } => &[
                "opacity", "x", "y", "z", "scale", "blur", "glow", "flash", "alarm", "dim",
                "status", "content", "cool", "damage", "glitch", "cut", "ghost", "spinner",
                "release", "mark", "charge", "dissolve", "scan",
            ],
            Self::Orb { .. } => &[
                "opacity", "x", "y", "z", "scale", "blur", "rotation", "burst", "shatter", "pulse",
                "hurt", "spin", "charge",
            ],
            Self::Beam { .. } => &[
                "opacity", "sweep", "port", "draw", "break", "flow", "emphasis", "surge", "twang",
            ],
            Self::Packet { .. } => &["opacity", "age", "flight"],
            Self::Label { .. } => &["opacity", "x", "y", "z", "scale", "typed"],
            Self::Ring { .. } => &["opacity", "x", "y", "z", "scale", "sweep", "expand"],
            Self::Bolt { .. } => &["opacity", "age", "seed", "hum"],
            Self::Shield { .. } => &["opacity", "up", "scale"],
        }
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

impl StagePlan {
    pub fn element(&self, id: &str) -> Option<&StageElement> {
        self.elements.iter().find(|element| element.id() == id)
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
                StageElement::Bolt {
                    from,
                    to,
                    strikes,
                    branching,
                    ..
                } => {
                    for end in [from, to] {
                        match end {
                            BoltEnd::Element(end) => ensure!(
                                end != id
                                    && self.element(end).is_some_and(|element| {
                                        element.anchor().is_some()
                                            || matches!(element, StageElement::Shield { .. })
                                    }),
                                "bolt '{id}' must strike positioned elements or shields; '{end}' is not one"
                            ),
                            BoltEnd::Point(at) => ensure!(
                                at.iter().all(|v| v.is_finite()) && at[2] > -FOCAL * 0.8,
                                "bolt '{id}' points must be finite and in front of the camera"
                            ),
                        }
                    }
                    ensure!(from != to, "bolt '{id}' needs two different ends");
                    ensure!(
                        (1..=lightning::MAX_STRIKES).contains(strikes)
                            && (0.0..=1.5).contains(branching),
                        "bolt '{id}' needs 1..=8 strikes and branching in 0..1.5"
                    );
                }
                StageElement::Shield { around, radius, .. } => {
                    ensure!(
                        self.element(around)
                            .and_then(StageElement::anchor)
                            .is_some(),
                        "shield '{id}' must surround a positioned element"
                    );
                    ensure!(
                        (10.0..=900.0).contains(radius),
                        "shield '{id}' radius is out of range"
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

    /// Spring `property` to `target` at `at_nanos`. Channels not declared
    /// with [`Self::channel`] start at 0.
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
        let channel = self.channel(scene, property, 0.0);
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
        let channel = self.channel(scene, property, 0.0);
        scene.ease(&channel, at_nanos, target, seconds, curve);
    }

    /// Jump `property` to `value` at `at_nanos`.
    pub fn set(&mut self, scene: &mut PlanBuilder, property: &str, at_nanos: u64, value: f32) {
        let channel = self.channel(scene, property, 0.0);
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

/// Effect beats: lightning, charge, dissolve, scans, and shields. Each writes
/// one clock or amount channel; the renderer derives every phase from it.
impl StageActor {
    /// Lightning strikes along `bolt`: its stepped leader sets out at
    /// `at_nanos` and the first return stroke connects [`lightning::LEADER`]
    /// later; further strokes strobe a few frames apart, each re-rolling the
    /// path, then the channel cools. Every zap rolls a fresh seed. Returns the
    /// contact time; pair it with `land` or `jolt` for the receiver.
    pub fn zap(&mut self, scene: &mut PlanBuilder, bolt: &str, at_nanos: u64) -> u64 {
        let strikes = match self.plan.element(bolt) {
            Some(StageElement::Bolt { strikes, .. }) => *strikes,
            _ => default_strikes(),
        };
        let seed = lightning::seed_for(at_nanos);
        let discharge = lightning::Discharge::new(strikes, seed);
        // A receiver's reaction (an orb's surface wave, a shield's ripple)
        // outlasts the bolt's own glow.
        let last = discharge.strike_time(discharge.strikes - 1);
        let lifetime = discharge.lifetime().max(last + shield::RIPPLE);
        let channel = self.channel(scene, &format!("{bolt}.seed"), 0.0);
        scene.set(&channel, at_nanos, seed as f32);
        // Whole milliseconds, so the clock's slope is exactly one.
        let seconds = (lifetime * 1000.0).ceil() / 1000.0;
        self.clock_for(scene, &format!("{bolt}.age"), at_nanos, seconds);
        at_nanos + whole_millis(lightning::LEADER)
    }

    /// Ease `element`'s (a card's or orb's) charge to `intensity` (0..1,
    /// overdriven to 1.5) over `seconds`: short arcs crawl its outline and
    /// strobe, lighting its rim. Zero discharges it; zero seconds is instant.
    pub fn charge(
        &mut self,
        scene: &mut PlanBuilder,
        element: &str,
        at_nanos: u64,
        intensity: f32,
        seconds: f32,
    ) {
        self.amount(
            scene,
            &format!("{element}.charge"),
            at_nanos,
            intensity,
            seconds,
        );
    }

    /// Keep an arc alive along `bolt` at `intensity` (eased over `seconds`):
    /// it re-strikes 24 times a second while its channel writhes. Zero stops it.
    pub fn hum(
        &mut self,
        scene: &mut PlanBuilder,
        bolt: &str,
        at_nanos: u64,
        intensity: f32,
        seconds: f32,
    ) {
        self.amount(scene, &format!("{bolt}.hum"), at_nanos, intensity, seconds);
    }

    fn amount(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        at_nanos: u64,
        target: f32,
        seconds: f32,
    ) {
        if seconds > 0.0 {
            self.ease(scene, property, at_nanos, target, seconds, Ease::Smoothstep);
        } else {
            self.set(scene, property, at_nanos, target);
        }
    }

    /// Burn `card` away from `at_nanos`: a noisy front with a hot rim crosses
    /// it in [`dissolve::BURN`] seconds, shedding ash that drifts up and
    /// cools. Returns when the card is gone; the ash cools a little longer.
    pub fn dissolve(&mut self, scene: &mut PlanBuilder, card: &str, at_nanos: u64) -> u64 {
        self.clock_for(
            scene,
            &format!("{card}.dissolve"),
            at_nanos,
            dissolve::DURATION,
        );
        at_nanos + whole_millis(dissolve::BURN)
    }

    /// Form `card` out of ash: the dissolve played backwards over `seconds`
    /// (its full clock is [`dissolve::DURATION`]), so flakes fly home and the
    /// rim recedes. Returns when the card is whole.
    pub fn materialize(
        &mut self,
        scene: &mut PlanBuilder,
        card: &str,
        at_nanos: u64,
        seconds: f32,
    ) -> u64 {
        let channel = self.channel(scene, &format!("{card}.dissolve"), dissolve::DURATION);
        scene.set(&channel, at_nanos, dissolve::DURATION);
        scene.ease(&channel, at_nanos, 0.0, seconds, Ease::Linear);
        at_nanos + whole_millis(seconds)
    }

    /// Sweep a scan line down `card` over `seconds`: a bright line with a
    /// fading wake, lighting the rim where it crosses. Returns when it ends.
    pub fn scan(
        &mut self,
        scene: &mut PlanBuilder,
        card: &str,
        at_nanos: u64,
        seconds: f32,
    ) -> u64 {
        let channel = self.channel(scene, &format!("{card}.scan"), 0.0);
        scene.set(&channel, at_nanos, 0.0);
        scene.ease(&channel, at_nanos, 1.0, seconds, Ease::Linear);
        at_nanos + whole_millis(seconds)
    }

    /// Raise `shield` over `seconds`: its cells switch on in seeded order.
    /// A shield is up by default; raising one first declares it down.
    pub fn raise(&mut self, scene: &mut PlanBuilder, shield: &str, at_nanos: u64, seconds: f32) {
        let channel = self.channel(scene, &format!("{shield}.up"), 0.0);
        scene.ease(&channel, at_nanos, 1.0, seconds, Ease::Smoothstep);
    }

    /// Lower `shield` over `seconds`: its cells switch off in reverse order.
    pub fn lower(&mut self, scene: &mut PlanBuilder, shield: &str, at_nanos: u64, seconds: f32) {
        let channel = self.channel(scene, &format!("{shield}.up"), 1.0);
        scene.ease(&channel, at_nanos, 0.0, seconds, Ease::Smoothstep);
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

    fn effects_plan() -> StagePlan {
        serde_json::from_value(serde_json::json!({
            "elements": [
                { "kind": "card", "id": "build", "at": [420, 540, 0], "size": [300, 110], "title": "build" },
                { "kind": "orb", "id": "deploy", "at": [1400, 540, 0], "radius": 120 },
                { "kind": "shield", "id": "guard", "around": "deploy", "radius": 190 },
                { "kind": "bolt", "id": "zap", "from": "build", "to": "deploy" },
                { "kind": "bolt", "id": "strike", "from": [960, -40, 0], "to": "guard", "strikes": 4 }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn bolts_and_shields_validate_and_round_trip() {
        let plan = effects_plan();
        plan.validate().unwrap();
        let json = serde_json::to_value(&plan).unwrap();
        assert_eq!(
            json["elements"][3]["tone"], "request",
            "bolts default to the request tone"
        );
        assert_eq!(
            json["elements"][4]["from"],
            serde_json::json!([960.0, -40.0, 0.0])
        );
        assert_eq!(serde_json::from_value::<StagePlan>(json).unwrap(), plan);
        for property in [
            "zap.age",
            "zap.hum",
            "guard.up",
            "build.charge",
            "build.dissolve",
            "build.scan",
            "deploy.charge",
        ] {
            assert!(plan.accepts(property), "{property}");
        }
        assert!(!plan.accepts("guard.charge") && !plan.accepts("zap.x"));
        let mut dangling = effects_plan();
        if let StageElement::Bolt { to, .. } = &mut dangling.elements[3] {
            *to = BoltEnd::Element("zap".into());
        }
        assert!(dangling.validate().is_err(), "a bolt cannot strike a bolt");
        let mut storm = effects_plan();
        if let StageElement::Bolt { strikes, .. } = &mut storm.elements[3] {
            *strikes = 9;
        }
        assert!(storm.validate().is_err());
    }

    #[test]
    fn a_zap_starts_its_clock_with_a_fresh_seed_and_returns_contact() {
        let mut scene = PlanBuilder::new("zap", 5_000_000_000);
        let mut stage = StageActor::declare(&mut scene, "stage", &effects_plan()).unwrap();
        let contact = stage.zap(&mut scene, "strike", 1_000_000_000);
        assert_eq!(contact, 1_075_000_000, "the leader comes first");
        stage.zap(&mut scene, "strike", 3_000_000_000);
        let gone = stage.dissolve(&mut scene, "build", 2_000_000_000);
        assert_eq!(gone, 3_100_000_000);
        let plan = scene.finish().unwrap();
        let channel = |property: &str| {
            plan.continuous_channels
                .iter()
                .find(|c| c.property == property)
                .unwrap()
        };
        let seeds = channel("strike.seed")
            .events
            .iter()
            .filter_map(|event| match event {
                crate::plan::TrackEventPlan::Set {
                    value: crate::plan::ScalarPlan::Literal(v),
                    ..
                } => Some(*v),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(seeds.len(), 2);
        assert!(seeds[0] != seeds[1] && seeds.iter().all(|s| s.fract() == 0.0));
        assert!(
            matches!(channel("strike.age").initial, crate::plan::ScalarPlan::Literal(v) if v == -1.0)
        );
        assert!(
            matches!(channel("build.dissolve").initial, crate::plan::ScalarPlan::Literal(v) if v == -1.0)
        );
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
