//! Stage: a 2.5D motion-graphics surface for explainers. Elements (cards, particle
//! orbs, light beams, travelling packets, labels, rings) sit at world positions seen
//! through a perspective camera, and every change is an ordinary Continuous
//! Channel. Geometry that depends on time (orb spin, beam flow) is a pure function
//! of the sample time, so any frame renders identically in any order.
use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    author::{ActorHandle, ContinuousHandle, PlanBuilder},
    caption::{CaptionAlign, CaptionSpanPlan},
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

/// Look of the whole frame; the matching `post.*` channels animate these.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StagePost {
    pub bloom: f32,
    pub grain: f32,
    pub vignette: f32,
    /// A soft accent-tinted light behind the scene, 0 for none.
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
        #[serde(default, skip_serializing_if = "is_center")]
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

    /// Channel properties this element reads (after its `<id>.` prefix).
    pub fn properties(&self) -> &'static [&'static str] {
        match self {
            Self::Card { .. } => &[
                "opacity", "x", "y", "z", "scale", "glow", "flash", "alarm", "dim", "status",
            ],
            Self::Orb { .. } => &[
                "opacity", "x", "y", "z", "scale", "shatter", "pulse", "hurt", "spin",
            ],
            Self::Beam { .. } => &["opacity", "draw", "break", "flow", "emphasis"],
            Self::Packet { .. } => &["opacity", "travel", "impact"],
            Self::Label { .. } => &["opacity", "x", "y", "z", "scale", "typed"],
            Self::Ring { .. } => &["opacity", "x", "y", "z", "scale", "sweep", "expand"],
        }
    }
}

/// Channels that belong to the whole stage rather than an element.
pub const STAGE_PROPERTIES: [&str; 10] = [
    "camera.x",
    "camera.y",
    "camera.z",
    "camera.focus",
    "camera.dof",
    "camera.shake",
    "post.bloom",
    "post.chroma",
    "post.exposure",
    "post.vignette",
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
    pub x: f32,
    pub y: f32,
    /// Dolly toward the scene; positive values make the z = 0 plane larger.
    pub z: f32,
    pub width: f32,
    pub height: f32,
}

impl Camera {
    /// Screen position and scale of a world point, or `None` behind the camera.
    pub fn project(&self, [x, y, z]: [f32; 3]) -> Option<([f32; 2], f32)> {
        let depth = FOCAL + z - self.z;
        if depth <= 1.0 {
            return None;
        }
        let scale = FOCAL / depth;
        let [cx, cy] = [self.width * 0.5, self.height * 0.5];
        Some((
            [
                cx + (x - cx - self.x) * scale,
                cy + (y - cy - self.y) * scale,
            ],
            scale,
        ))
    }
}

/// Unit-sphere points spread evenly by the golden angle, with per-point seeds.
pub fn fibonacci_sphere(count: u32) -> Vec<([f32; 3], [f32; 3])> {
    let n = count.max(2);
    (0..n)
        .map(|i| {
            let y = 1.0 - (i as f32 / (n - 1) as f32) * 2.0;
            let r = (1.0 - y * y).max(0.0).sqrt();
            let theta = i as f32 * 2.399_963_1;
            (
                [theta.cos() * r, y, theta.sin() * r],
                [hash(i, 3), hash(i, 7), hash(i, 11)],
            )
        })
        .collect()
}

/// Deterministic 0..1 value for an integer and a salt.
pub fn hash(value: u32, salt: u32) -> f32 {
    let mut h = value.wrapping_mul(0x9E37_79B1) ^ salt.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1 << 24) as f32
}

/// One point of a shattering orb, relative to its center, before projection.
/// Points burst outward by different amounts, then fall; `shatter` runs 0..1.
pub fn shatter_offset(unit: [f32; 3], seed: [f32; 3], radius: f32, shatter: f32) -> [f32; 3] {
    let burst = 1.0 + shatter * (0.6 + 2.4 * seed[0]);
    let fall = shatter * shatter * (160.0 + 460.0 * seed[1]);
    let drift = shatter * (seed[2] - 0.5) * 120.0;
    [
        unit[0] * radius * burst + drift,
        unit[1] * radius * burst + fall,
        unit[2] * radius * burst,
    ]
}

/// A point on the quadratic curve from `a` to `b`, bowed sideways by `bend`.
pub fn beam_point(a: [f32; 3], b: [f32; 3], bend: f32, t: f32) -> [f32; 3] {
    let mid = [
        (a[0] + b[0]) * 0.5,
        (a[1] + b[1]) * 0.5,
        (a[2] + b[2]) * 0.5,
    ];
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = (dx * dx + dy * dy).sqrt().max(1e-3);
    let control = [
        mid[0] - dy / length * bend,
        mid[1] + dx / length * bend,
        mid[2],
    ];
    let u = 1.0 - t;
    std::array::from_fn(|i| u * u * a[i] + 2.0 * u * t * control[i] + t * t * b[i])
}

/// Authoring handle: declares each stage channel once, with the recipe default
/// as its initial value.
pub struct StageActor {
    actor: ActorHandle,
    channels: HashMap<String, ContinuousHandle>,
    /// Characters in each label, for typing.
    labels: HashMap<String, usize>,
}

impl StageActor {
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: &StagePlan,
    ) -> Result<Self> {
        plan.validate()?;
        let actor = scene.actor(id, STAGE_RECIPE, plan)?;
        let labels = plan
            .elements
            .iter()
            .filter_map(|element| match element {
                StageElement::Label { id, spans, .. } => Some((
                    id.clone(),
                    spans.iter().map(|span| span.text.chars().count()).sum(),
                )),
                _ => None,
            })
            .collect();
        Ok(Self {
            actor,
            channels: HashMap::new(),
            labels,
        })
    }

    pub fn actor(&self) -> &ActorHandle {
        &self.actor
    }

    /// The channel for `property`, declared on first use with `initial`.
    pub fn channel(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        self.channels
            .entry(property.to_owned())
            .or_insert_with(|| scene.continuous(&self.actor, property, initial))
            .clone()
    }

    /// Spring `property` to `target` at `at_nanos`. `initial` applies only when
    /// this is the channel's first use.
    pub fn to(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
        at_nanos: u64,
        target: f32,
        seconds: f32,
    ) {
        let channel = self.channel(scene, property, initial);
        scene.spring(&channel, at_nanos, target, seconds, 0.0);
    }

    /// Like `to`, with overshoot: `bounce` 0.2 reads as a lively landing.
    #[allow(clippy::too_many_arguments)]
    pub fn bounce(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
        at_nanos: u64,
        target: f32,
        seconds: f32,
        bounce: f32,
    ) {
        let channel = self.channel(scene, property, initial);
        scene.spring(&channel, at_nanos, target, seconds, bounce);
    }

    /// Jump `property` to `value` at `at_nanos`.
    pub fn set(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
        at_nanos: u64,
        value: f32,
    ) {
        let channel = self.channel(scene, property, initial);
        scene.set(&channel, at_nanos, value);
    }

    /// Rise to `peak` and settle back to `rest`: a flash, a hit, a pulse.
    pub fn hit(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        at_nanos: u64,
        peak: f32,
        rest: f32,
    ) {
        let channel = self.channel(scene, property, rest);
        scene.spring(&channel, at_nanos, peak, 0.12, 0.0);
        scene.spring(&channel, at_nanos + 140_000_000, rest, 0.7, 0.0);
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
        let chars = self.labels.get(label).copied().unwrap_or(0).max(1);
        let opacity = self.channel(scene, &format!("{label}.opacity"), 0.0);
        let typed = self.channel(scene, &format!("{label}.typed"), 0.0);
        scene.set(&opacity, at_nanos, 1.0);
        let per_char = (1e9 / f64::from(chars_per_second.max(1.0))) as u64;
        for index in 1..=chars {
            scene.set(
                &typed,
                at_nanos + per_char * index as u64,
                index as f32 / chars as f32,
            );
        }
        at_nanos + per_char * chars as u64
    }

    /// Send a packet along its beam over `seconds`, then ripple on arrival.
    /// Returns the arrival time.
    pub fn send(
        &mut self,
        scene: &mut PlanBuilder,
        packet: &str,
        at_nanos: u64,
        seconds: f32,
    ) -> u64 {
        let travel = self.channel(scene, &format!("{packet}.travel"), 0.0);
        scene.set(&travel, at_nanos, 0.0);
        let steps = 24;
        // Whole milliseconds: an f32 duration such as 0.8 is not exact in nanoseconds.
        let span = (f64::from(seconds) * 1000.0).round() as u64 * 1_000_000;
        // Ease in and out along the path with exact steps, so the packet
        // accelerates away and decelerates into the target.
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            let eased = t * t * (3.0 - 2.0 * t);
            scene.set(&travel, at_nanos + span * step / steps, eased);
        }
        let arrival = at_nanos + span;
        let impact = self.channel(scene, &format!("{packet}.impact"), 0.0);
        scene.set(&impact, arrival, 0.0);
        scene.spring(&impact, arrival, 1.0, 0.9, 0.0);
        let opacity = self.channel(scene, &format!("{packet}.opacity"), 0.0);
        scene.set(&opacity, at_nanos, 1.0);
        scene.spring(&opacity, arrival + 350_000_000, 0.0, 0.3, 0.0);
        arrival
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
                && plan.accepts("probe.travel")
        );
        assert!(
            !plan.accepts("service.travel")
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
        let camera = Camera {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        assert_eq!(
            camera.project([300.0, 200.0, 0.0]),
            Some(([300.0, 200.0], 1.0))
        );
        let (far, scale) = camera.project([300.0, 200.0, 700.0]).unwrap();
        assert!(
            scale < 1.0 && far[0] > 300.0,
            "farther points shrink toward the center"
        );
        let dolly = Camera { z: 700.0, ..camera };
        assert!(dolly.project([300.0, 200.0, 0.0]).unwrap().1 > 1.0);
        assert!(camera.project([0.0, 0.0, -FOCAL]).is_none());
    }

    #[test]
    fn orb_and_beam_geometry_is_deterministic() {
        let points = fibonacci_sphere(64);
        assert_eq!(points.len(), 64);
        assert!(
            points.iter().all(
                |(p, _)| ((p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt() - 1.0).abs() < 1e-4
            )
        );
        assert_eq!(points, fibonacci_sphere(64));
        let (unit, seed) = points[5];
        assert_eq!(
            shatter_offset(unit, seed, 100.0, 0.0),
            unit.map(|v| v * 100.0)
        );
        let burst = shatter_offset(unit, seed, 100.0, 1.0);
        assert!(burst[1] > unit[1] * 100.0 + 100.0, "shattered points fall");
        let (a, b) = ([0.0, 0.0, 0.0], [100.0, 0.0, 0.0]);
        assert_eq!(beam_point(a, b, 40.0, 0.0), a);
        assert_eq!(beam_point(a, b, 40.0, 1.0), b);
        assert!(
            beam_point(a, b, 40.0, 0.5)[1] > 0.0,
            "positive bend bows to the right of travel"
        );
    }

    #[test]
    fn packets_ease_along_their_beam_and_ripple_on_arrival() {
        let mut scene = PlanBuilder::new("stage-demo", 5_000_000_000);
        let mut stage = StageActor::declare(&mut scene, "stage", &plan()).unwrap();
        let arrival = stage.send(&mut scene, "probe", 1_000_000_000, 0.8);
        assert_eq!(arrival, 1_800_000_000);
        let plan = scene.finish().unwrap();
        let travel = plan
            .continuous_channels
            .iter()
            .find(|c| c.property == "probe.travel")
            .unwrap();
        assert_eq!(travel.events.len(), 25);
    }
}
