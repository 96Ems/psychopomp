//! Stage: a 2.5D motion-graphics surface for explainers. Elements (cards, particle
//! orbs and forms, light beams, drawn paths, flat shapes, icons, travelling
//! packets, labels, rings) sit at world positions seen
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
        easing::{Ease, smootherstep},
        random::hash,
        shapes::{
            Box2, Circle, Polygon, Shape, box_points, cylinder_points, fibonacci_sphere,
            grid_points, match_points, torus_points,
        },
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
    /// A particle form: the orb's glowing points arranged on one or more
    /// `shapes` (a box, a dot-matrix plane, a lattice, a cylinder, a torus, or
    /// a sphere). It turns in 3D, and the `morph` channel carries every point
    /// from one shape to the next with stable identity.
    #[serde(rename_all = "camelCase")]
    Form {
        id: String,
        at: [f32; 3],
        shapes: Vec<FormShape>,
        #[serde(default = "default_points")]
        points: u32,
        #[serde(default = "accent")]
        tone: Tone,
        /// How far the form leans back toward the camera, in radians (the
        /// orb's view). 0 faces the camera squarely.
        #[serde(default = "default_tilt", skip_serializing_if = "is_default_tilt")]
        tilt: f32,
    },
    /// A flat figure with no card chrome: a rectangle, circle, arc, or
    /// polygon, filled and stroked. The stroke draws on along its outline.
    #[serde(rename_all = "camelCase")]
    Shape {
        id: String,
        at: [f32; 3],
        shape: Figure,
        /// Corner radius of a rectangle or polygon.
        #[serde(default, skip_serializing_if = "is_zero")]
        corner: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill: Option<Fill>,
        #[serde(default = "one", skip_serializing_if = "is_one")]
        fill_opacity: f32,
        /// `null` for no stroke.
        #[serde(default = "muted_stroke", skip_serializing_if = "is_muted_stroke")]
        stroke: Option<Tone>,
        #[serde(default = "default_width", skip_serializing_if = "is_default_width")]
        width: f32,
        /// Dash and gap lengths in pixels.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dash: Option<[f32; 2]>,
        /// Arrowheads at an arc's ends.
        #[serde(default, skip_serializing_if = "Arrow::is_none")]
        arrow: Arrow,
    },
    /// A drawn connection through world points and positioned elements, with
    /// optional arrowheads. Packets ride it like a beam; element waypoints
    /// between its ends are stops where a riding packet lands and relays.
    #[serde(rename_all = "camelCase")]
    Path {
        id: String,
        through: Vec<Waypoint>,
        #[serde(default, skip_serializing_if = "Curve::is_straight")]
        curve: Curve,
        /// Radius that rounds a straight path's corners at point waypoints.
        #[serde(default, skip_serializing_if = "is_zero")]
        corner: f32,
        /// Sideways bow of each hop that leaves or enters an element.
        #[serde(default, skip_serializing_if = "is_zero")]
        bend: f32,
        #[serde(default, skip_serializing_if = "Tone::is_default")]
        tone: Tone,
        #[serde(default = "default_width", skip_serializing_if = "is_default_width")]
        width: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dash: Option<[f32; 2]>,
        #[serde(default, skip_serializing_if = "Arrow::is_none")]
        arrow: Arrow,
    },
    /// A monochrome SVG icon `size` world pixels square, tinted by its tone:
    /// a bundled Phosphor `icon` by name, or SVG `path` data in a `view`-unit
    /// square.
    #[serde(rename_all = "camelCase")]
    Icon {
        id: String,
        at: [f32; 3],
        size: f32,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        icon: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        path: String,
        #[serde(default = "default_view", skip_serializing_if = "is_default_view")]
        view: f32,
        #[serde(default, skip_serializing_if = "Tone::is_default")]
        tone: Tone,
    },
}

/// One shape a form's points can take, in world pixels about its center.
/// Boxes carry a share of their points on their edges, so they read as
/// solids; planes and lattices are exact grids, so their point count must
/// factor into one.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "shape", rename_all = "kebab-case", deny_unknown_fields)]
pub enum FormShape {
    Sphere {
        radius: f32,
    },
    /// A box (a cube, a slab) of `size` width, height, and depth.
    Box {
        size: [f32; 3],
        /// Share of the points on its twelve edges, 0..1.
        #[serde(default = "default_edges")]
        edges: f32,
    },
    /// A flat dot matrix facing the camera (at tilt 0).
    Plane {
        size: [f32; 2],
    },
    /// A grid of points filling a box.
    Lattice {
        size: [f32; 3],
    },
    /// An open tube about the vertical axis, its rims drawn as rings.
    Cylinder {
        radius: f32,
        height: f32,
    },
    /// A ring lying flat: `radius` to the middle of its tube.
    Torus {
        radius: f32,
        tube: f32,
    },
}

impl FormShape {
    pub fn cube(size: f32) -> Self {
        Self::Box {
            size: [size; 3],
            edges: default_edges(),
        }
    }

    /// Distance from the center to the farthest point.
    pub fn radius(&self) -> f32 {
        match *self {
            Self::Sphere { radius } => radius,
            Self::Box { size, .. } | Self::Lattice { size } => Vec3::from(size).length() * 0.5,
            Self::Plane { size } => Vec2::from(size).length() * 0.5,
            Self::Cylinder { radius, height } => radius.hypot(height * 0.5),
            Self::Torus { radius, tube } => radius + tube,
        }
    }

    /// The silhouette's width and height at rest, unturned.
    fn extent(&self) -> Vec2 {
        match *self {
            Self::Sphere { radius } => Vec2::splat(radius * 2.0),
            Self::Box { size, .. } | Self::Lattice { size } => Vec2::new(size[0], size[1]),
            Self::Plane { size } => Vec2::from(size),
            Self::Cylinder { radius, height } => Vec2::new(radius * 2.0, height),
            Self::Torus { radius, tube } => Vec2::new((radius + tube) * 2.0, tube * 2.0),
        }
    }

    /// `count` points in this shape's natural, deterministic arrangement, or
    /// `None` for a grid that `count` cannot fill exactly.
    pub fn points(&self, count: u32) -> Option<Vec<Vec3>> {
        Some(match *self {
            Self::Sphere { radius } => fibonacci_sphere(count)
                .into_iter()
                .map(|p| p * radius)
                .collect(),
            Self::Box { size, edges } => box_points(count, Vec3::from(size) * 0.5, edges),
            Self::Plane { size } => grid_points(count, size)?,
            Self::Lattice { size } => grid_points(count, size)?,
            Self::Cylinder { radius, height } => cylinder_points(count, radius, height),
            Self::Torus { radius, tube } => torus_points(count, radius, tube),
        })
    }

    fn validate(&self, id: &str, count: u32) -> Result<()> {
        let sized = |v: &[f32], min: f32| {
            v.iter()
                .all(|v| v.is_finite() && (min..=2000.0).contains(v))
        };
        let ok = match *self {
            Self::Sphere { radius } => sized(&[radius], 10.0),
            Self::Box { size, edges } => sized(&size, 1.0) && (0.0..=1.0).contains(&edges),
            Self::Plane { size } => sized(&size, 10.0),
            Self::Lattice { size } => sized(&size, 10.0),
            Self::Cylinder { radius, height } => sized(&[radius, height], 4.0),
            Self::Torus { radius, tube } => sized(&[radius, tube], 4.0) && tube < radius,
        };
        ensure!(ok, "form '{id}' has a shape whose size is out of range");
        ensure!(
            self.points(count).is_some(),
            "form '{id}' has {count} points, which do not fill a grid of at least 2 per side; choose a count with convenient factors (720 = 36 × 20)"
        );
        Ok(())
    }
}

/// Every shape of a form as `count` points, each shape reordered to pair its
/// points with the previous shape's, so morphing moves each point a short
/// way. Shapes must have validated.
pub fn form_points(shapes: &[FormShape], count: u32) -> Vec<Vec<Vec3>> {
    let mut matched: Vec<Vec<Vec3>> = Vec::with_capacity(shapes.len());
    for shape in shapes {
        let points = shape
            .points(count)
            .unwrap_or_else(|| fibonacci_sphere(count));
        let points = match matched.last() {
            Some(previous) => match_points(previous, points),
            None => points,
        };
        matched.push(points);
    }
    matched
}

/// How much of a morph's span the per-point stagger takes.
pub const MORPH_STAGGER: f32 = 0.35;

/// Where point `index` of a form sits at `morph`, a fractional index into
/// its matched `shapes` (see [`form_points`]): each point leaves one shape
/// for the next at a moment staggered by its `seed` (0..1), bows slightly
/// outward on the way, and arrives exactly on the next whole `morph`.
pub fn morph_point(shapes: &[Vec<Vec3>], index: usize, seed: f32, morph: f32) -> Vec3 {
    let last = shapes.len().saturating_sub(1);
    let morph = morph.clamp(0.0, last as f32);
    let from = (morph.floor() as usize).min(last.saturating_sub(1));
    let a = shapes[from][index];
    let t = morph - from as f32;
    if last == 0 || t <= 0.0 {
        return a;
    }
    let b = shapes[from + 1][index];
    let local = smootherstep(((t - MORPH_STAGGER * seed) / (1.0 - MORPH_STAGGER)).clamp(0.0, 1.0));
    if local <= 0.0 {
        return a;
    }
    if local >= 1.0 {
        return b;
    }
    let bow =
        (a + b).normalize_or_zero() * (a.distance(b) * 0.18 * (std::f32::consts::PI * local).sin());
    a.lerp(b, local) + bow
}

/// A flat figure's geometry, in world pixels about the shape's `at`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Figure {
    /// Width and height.
    Rect([f32; 2]),
    /// Radius.
    Circle(f32),
    /// Part of a circle, clockwise from twelve o'clock: `start` and `sweep`
    /// in turns.
    #[serde(rename_all = "camelCase")]
    Arc {
        radius: f32,
        #[serde(default)]
        start: f32,
        sweep: f32,
    },
    /// Corners relative to `at`, in order.
    Polygon(Vec<[f32; 2]>),
}

impl Figure {
    /// The outline at rest, about `center`, at `scale`.
    pub fn outline(&self, center: Vec2, scale: f32) -> Shape {
        match self {
            Self::Rect(size) => {
                Shape::Box(Box2::from_center_size(center, Vec2::from(*size) * scale))
            }
            Self::Circle(radius) | Self::Arc { radius, .. } => Shape::Circle(Circle {
                center,
                radius: radius * scale,
            }),
            Self::Polygon(points) => Shape::Polygon(Polygon::hull(
                center,
                points.iter().map(|p| center + Vec2::from(*p) * scale),
            )),
        }
    }
}

/// What fills a shape: a tone, or the theme's card `surface` or `background`
/// (an opaque panel that hides what is behind it).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Fill {
    Tone(Tone),
    Material(Material),
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Material {
    Surface,
    Background,
}

/// Which ends of an open line carry an arrowhead.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Arrow {
    #[default]
    None,
    Start,
    End,
    Both,
}

impl Arrow {
    pub fn is_none(&self) -> bool {
        *self == Self::None
    }
    pub fn at_start(self) -> bool {
        matches!(self, Self::Start | Self::Both)
    }
    pub fn at_end(self) -> bool {
        matches!(self, Self::End | Self::Both)
    }
}

/// A path's waypoint: a positioned element's id, or a world point.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Waypoint {
    Element(String),
    Point([f32; 3]),
}

impl Waypoint {
    pub fn element(&self) -> Option<&str> {
        match self {
            Self::Element(id) => Some(id),
            Self::Point(_) => None,
        }
    }
}

/// How a path runs between its waypoints. `smooth` passes through every point
/// on a Catmull-Rom curve; with `bezier` the points are a cubic chain (start,
/// two controls, end, two controls, end, ...). Both take points only.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Curve {
    #[default]
    Straight,
    Smooth,
    Bezier,
}

impl Curve {
    pub fn is_straight(&self) -> bool {
        *self == Self::Straight
    }
}

/// Bundled Phosphor icons (MIT, `assets/icons`), by name.
pub const ICONS: [&str; 40] = [
    "arrows-clockwise",
    "bell",
    "brain",
    "broadcast",
    "chart-line-up",
    "check-circle",
    "clock",
    "cloud",
    "code",
    "cpu",
    "cube",
    "database",
    "desktop",
    "device-mobile",
    "envelope",
    "file",
    "fingerprint",
    "folder",
    "gear",
    "git-branch",
    "globe",
    "hard-drives",
    "hourglass",
    "key",
    "lightning",
    "lock",
    "lock-open",
    "magnifying-glass",
    "package",
    "plug",
    "queue",
    "robot",
    "shield-check",
    "sparkle",
    "stack",
    "terminal",
    "user",
    "users",
    "warning",
    "x-circle",
];

fn default_points() -> u32 {
    720
}
fn default_thickness() -> f32 {
    3.0
}
fn default_tilt() -> f32 {
    0.42
}
fn is_default_tilt(tilt: &f32) -> bool {
    *tilt == default_tilt()
}
fn default_edges() -> f32 {
    0.5
}
fn default_width() -> f32 {
    1.4
}
fn is_default_width(width: &f32) -> bool {
    *width == default_width()
}
fn default_view() -> f32 {
    256.0
}
fn is_default_view(view: &f32) -> bool {
    *view == default_view()
}
fn is_zero(value: &f32) -> bool {
    *value == 0.0
}
fn one() -> f32 {
    1.0
}
fn is_one(value: &f32) -> bool {
    *value == 1.0
}
fn muted_stroke() -> Option<Tone> {
    Some(Tone::Muted)
}
fn is_muted_stroke(stroke: &Option<Tone>) -> bool {
    *stroke == muted_stroke()
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

impl StageElement {
    pub fn id(&self) -> &str {
        match self {
            Self::Card { id, .. }
            | Self::Orb { id, .. }
            | Self::Beam { id, .. }
            | Self::Packet { id, .. }
            | Self::Label { id, .. }
            | Self::Ring { id, .. }
            | Self::Form { id, .. }
            | Self::Shape { id, .. }
            | Self::Path { id, .. }
            | Self::Icon { id, .. } => id,
        }
    }

    /// World position of elements that have one (beams, paths, and packets
    /// derive theirs).
    pub fn anchor(&self) -> Option<[f32; 3]> {
        match self {
            Self::Card { at, .. }
            | Self::Orb { at, .. }
            | Self::Label { at, .. }
            | Self::Ring { at, .. }
            | Self::Form { at, .. }
            | Self::Shape { at, .. }
            | Self::Icon { at, .. } => Some(*at),
            Self::Beam { .. } | Self::Packet { .. } | Self::Path { .. } => None,
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
            Self::Label { .. } | Self::Beam { .. } | Self::Packet { .. } | Self::Path { .. } => {
                Shape::Point(center)
            }
            // The resting silhouette of the first shape; the renderer attaches
            // to the sampled, turned silhouette instead.
            Self::Form { shapes, .. } => match shapes.first() {
                Some(FormShape::Sphere { radius }) | Some(FormShape::Torus { radius, .. }) => {
                    Shape::Circle(Circle {
                        center,
                        radius: radius * scale,
                    })
                }
                Some(shape) => Shape::Box(Box2::from_center_size(center, shape.extent() * scale)),
                None => Shape::Point(center),
            },
            Self::Shape { shape, .. } => shape.outline(center, scale),
            Self::Icon { size, .. } => {
                Shape::Box(Box2::from_center_size(center, Vec2::splat(size * scale)))
            }
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
            Self::Form { .. } => &[
                "opacity", "x", "y", "z", "scale", "blur", "rotation", "spin", "pitch", "roll",
                "morph", "burst", "shatter", "pulse", "hurt",
            ],
            Self::Shape { .. } => &[
                "opacity", "x", "y", "z", "scale", "rotation", "blur", "draw", "fill", "emphasis",
                "flash",
            ],
            Self::Path { .. } => &["opacity", "draw", "trim", "flow", "emphasis", "surge"],
            Self::Icon { .. } => &["opacity", "x", "y", "z", "scale", "blur", "flash"],
        }
    }

    /// True for the particle bodies (orbs and forms): wires end beneath their
    /// occluding shell, and arrivals are absorbed out of sight.
    pub fn is_body(&self) -> bool {
        matches!(self, Self::Orb { .. } | Self::Form { .. })
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

    /// The legs a packet flies, in order, by the element each one arrives
    /// at (`None` for a path's free end). A beam is one leg; a path has one
    /// more leg per stop, an element waypoint between its ends.
    pub fn legs(&self, packet: &str) -> Vec<Option<&str>> {
        let Some(StageElement::Packet { beam, reverse, .. }) = self.element(packet) else {
            return Vec::new();
        };
        match self.element(beam) {
            Some(StageElement::Beam { from, to, .. }) => {
                vec![Some(if *reverse { from.as_str() } else { to.as_str() })]
            }
            Some(StageElement::Path { through, .. }) => {
                let mut waypoints = through.iter().collect::<Vec<_>>();
                if *reverse {
                    waypoints.reverse();
                }
                let last = waypoints.len() - 1;
                waypoints
                    .iter()
                    .enumerate()
                    .skip(1)
                    .filter(|(index, waypoint)| *index == last || waypoint.element().is_some())
                    .map(|(_, waypoint)| waypoint.element())
                    .collect()
            }
            _ => Vec::new(),
        }
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
                        matches!(
                            self.element(beam),
                            Some(StageElement::Beam { .. } | StageElement::Path { .. })
                        ),
                        "packet '{id}' must travel an existing beam or path"
                    );
                    line(id, label, 40)?;
                }
                StageElement::Form {
                    shapes,
                    points,
                    tilt,
                    ..
                } => {
                    ensure!(
                        (1..=8).contains(&shapes.len()) && (8..=4000).contains(points),
                        "form '{id}' needs one to eight shapes and 8..4000 points"
                    );
                    ensure!(tilt.is_finite(), "form '{id}' tilt must be finite");
                    for shape in shapes {
                        shape.validate(id, *points)?;
                    }
                }
                StageElement::Shape {
                    shape,
                    corner,
                    fill,
                    fill_opacity,
                    stroke,
                    width,
                    dash,
                    arrow,
                    ..
                } => {
                    let finite = |v: &[f32]| v.iter().all(|v| v.is_finite());
                    let ok = match shape {
                        Figure::Rect(size) => size.iter().all(|v| (1.0..=4000.0).contains(v)),
                        Figure::Circle(radius) => (1.0..=2000.0).contains(radius),
                        Figure::Arc {
                            radius,
                            start,
                            sweep,
                        } => {
                            (1.0..=2000.0).contains(radius)
                                && start.is_finite()
                                && *sweep > 0.0
                                && *sweep <= 1.0
                        }
                        Figure::Polygon(points) => {
                            (3..=64).contains(&points.len())
                                && points.iter().all(|p| {
                                    finite(p) && p[0].abs() <= 4000.0 && p[1].abs() <= 4000.0
                                })
                        }
                    };
                    ensure!(ok, "shape '{id}' geometry is out of range");
                    ensure!(
                        fill.is_some() || stroke.is_some(),
                        "shape '{id}' needs a fill or a stroke"
                    );
                    ensure!(
                        (0.0..=400.0).contains(corner) && (0.0..=1.0).contains(fill_opacity),
                        "shape '{id}' corner or fill opacity is out of range"
                    );
                    stroke_style(id, *width, *dash)?;
                    ensure!(
                        arrow.is_none() || matches!(shape, Figure::Arc { .. }),
                        "shape '{id}' is closed; only an arc can carry arrowheads"
                    );
                }
                StageElement::Path {
                    through,
                    curve,
                    corner,
                    bend,
                    width,
                    dash,
                    ..
                } => {
                    ensure!(
                        (2..=32).contains(&through.len()),
                        "path '{id}' needs two to 32 waypoints"
                    );
                    for (index, waypoint) in through.iter().enumerate() {
                        match waypoint {
                            Waypoint::Element(end) => {
                                ensure!(
                                    end != id
                                        && self
                                            .element(end)
                                            .and_then(StageElement::anchor)
                                            .is_some(),
                                    "path '{id}' runs through '{end}', which is not a positioned element"
                                );
                                ensure!(
                                    index == 0 || through[index - 1] != *waypoint,
                                    "path '{id}' visits '{end}' twice in a row"
                                );
                            }
                            Waypoint::Point(at) => ensure!(
                                at.iter().all(|v| v.is_finite()) && at[2] > -FOCAL * 0.8,
                                "path '{id}' has a point behind the camera"
                            ),
                        }
                    }
                    let points = through.iter().all(|w| w.element().is_none());
                    match curve {
                        Curve::Straight => {}
                        Curve::Smooth => ensure!(points, "a smooth path '{id}' takes points only"),
                        Curve::Bezier => ensure!(
                            points && through.len() % 3 == 1,
                            "a bezier path '{id}' takes 3n + 1 points: start, then two controls and an end per curve"
                        ),
                    }
                    ensure!(
                        (0.0..=400.0).contains(corner) && bend.is_finite(),
                        "path '{id}' corner or bend is out of range"
                    );
                    stroke_style(id, *width, *dash)?;
                }
                StageElement::Icon {
                    size,
                    icon,
                    path,
                    view,
                    ..
                } => {
                    ensure!(
                        (8.0..=600.0).contains(size) && *view > 0.0 && view.is_finite(),
                        "icon '{id}' size is out of range"
                    );
                    ensure!(
                        icon.is_empty() != path.is_empty(),
                        "icon '{id}' needs either a bundled `icon` or SVG `path` data"
                    );
                    ensure!(
                        icon.is_empty() || ICONS.contains(&icon.as_str()),
                        "icon '{id}' names '{icon}', which is not bundled; one of: {}",
                        ICONS.join(", ")
                    );
                    ensure!(
                        path.len() <= 20_000 && !path.contains(['"', '<', '>', '&']),
                        "icon '{id}' path data must be at most 20000 characters of SVG path commands"
                    );
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

fn stroke_style(id: &str, width: f32, dash: Option<[f32; 2]>) -> Result<()> {
    ensure!(
        (0.25..=24.0).contains(&width)
            && dash.is_none_or(|[on, off]| (0.5..=400.0).contains(&on) && (0.5..=400.0).contains(&off)),
        "stage element '{id}' stroke width or dash is out of range"
    );
    Ok(())
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

    /// How long a relaying packet rests at a stop after arriving before it
    /// gathers at the stop's far side and flies on: the reply never prepares
    /// before the request lands.
    pub const RELAY: f32 = 0.12;

    /// Seconds after dispatch that leg `leg` of `legs` starts gathering when
    /// the whole route flies in `flight` seconds, shared equally by its legs.
    /// Each leg is a whole packet life on its own clock, `age - leg_start`.
    pub fn leg_start(leg: usize, legs: usize, flight: f32) -> f32 {
        leg as f32 * (GATHER + flight / legs.max(1) as f32 + RELAY)
    }

    /// Seconds after dispatch that leg `leg` arrives.
    pub fn leg_arrival(leg: usize, legs: usize, flight: f32) -> f32 {
        leg_start(leg, legs, flight) + GATHER + flight / legs.max(1) as f32
    }

    /// Every phase of every leg has finished by this age.
    pub fn lifetime(legs: usize, flight: f32) -> f32 {
        leg_start(legs.max(1) - 1, legs, flight) + LIFETIME
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
        let legs = self.plan.legs(packet).len().max(1);
        // The clock runs at real speed until every phase has finished. A
        // packet can be sent again once its previous life has ended: the new
        // dispatch restarts the same clock.
        self.clock_for(
            scene,
            &format!("{packet}.age"),
            dispatch,
            packet::lifetime(legs, seconds),
        );
        let flight = self.channel(scene, &format!("{packet}.flight"), seconds);
        scene.set(&flight, dispatch, seconds);
        leg_arrival(dispatch, legs - 1, legs, seconds)
    }

    /// Send `packet` along its whole route, landing on each element it
    /// reaches: a path's stop lights as the packet arrives, then gathers it at
    /// its far side and relays it on. `seconds` is the whole route's flight,
    /// shared equally by its legs. Returns each leg's arrival time.
    pub fn relay(
        &mut self,
        scene: &mut PlanBuilder,
        packet: &str,
        at_nanos: u64,
        seconds: f32,
    ) -> Vec<u64> {
        let ends = self
            .plan
            .legs(packet)
            .into_iter()
            .map(|end| end.map(str::to_owned))
            .collect::<Vec<_>>();
        let dispatch = at_nanos.saturating_sub(whole_millis(packet::GATHER));
        self.send(scene, packet, at_nanos, seconds);
        let legs = ends.len().max(1);
        ends.iter()
            .enumerate()
            .map(|(leg, end)| {
                let arrival = leg_arrival(dispatch, leg, legs, seconds);
                if let Some(end) = end {
                    self.land(scene, end, arrival);
                }
                arrival
            })
            .collect()
    }

    /// Carry `form`'s points to its shape `index` over `seconds` on a
    /// minimum-jerk curve; each point's own move is staggered inside it.
    /// Returns when every point has arrived.
    pub fn morph(
        &mut self,
        scene: &mut PlanBuilder,
        form: &str,
        at_nanos: u64,
        index: usize,
        seconds: f32,
    ) -> u64 {
        self.ease(
            scene,
            &format!("{form}.morph"),
            at_nanos,
            index as f32,
            seconds,
            Ease::Smootherstep,
        );
        at_nanos + whole_millis(seconds)
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
        // A path has no port channel: its sockets follow its own draw.
        let start = if matches!(self.plan.element(beam), Some(StageElement::Path { .. })) {
            at_nanos
        } else {
            let port = self.channel(scene, &format!("{beam}.port"), 0.0);
            scene.ease(&port, at_nanos, 1.0, PORT_POP_SECONDS, Ease::Smootherstep);
            at_nanos + whole_millis(PORT_POP_SECONDS)
        };
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
            Some(StageElement::Orb { .. } | StageElement::Form { .. }) => {
                self.hit(scene, &format!("{element}.pulse"), at_nanos, 0.6, 0.0);
            }
            Some(StageElement::Shape { .. } | StageElement::Icon { .. }) => {
                self.hit(scene, &format!("{element}.flash"), at_nanos, 0.6, 0.0);
            }
            _ => {}
        }
    }
}

/// When leg `leg` of `legs` arrives for a packet dispatched at `dispatch`,
/// in whole milliseconds like every authored clock.
fn leg_arrival(dispatch: u64, leg: usize, legs: usize, seconds: f32) -> u64 {
    dispatch
        + whole_millis(packet::leg_start(leg, legs, seconds))
        + whole_millis(packet::GATHER)
        + whole_millis(seconds / legs as f32)
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

    fn diagram() -> StagePlan {
        serde_json::from_value(serde_json::json!({
            "elements": [
                { "kind": "form", "id": "store", "at": [1300, 540, 0], "points": 360,
                  "shapes": [{ "shape": "box", "size": [200, 200, 200] }, { "shape": "sphere", "radius": 130 }] },
                { "kind": "card", "id": "client", "at": [400, 540, 0], "size": [260, 100], "title": "client" },
                { "kind": "shape", "id": "gate", "at": [850, 540, 0], "shape": { "rect": [120, 160] },
                  "corner": 12, "fill": "surface" },
                { "kind": "shape", "id": "loop", "at": [850, 300, 0],
                  "shape": { "arc": { "radius": 50, "sweep": 0.8 } }, "arrow": "end", "stroke": "accent" },
                { "kind": "icon", "id": "lock", "at": [850, 540, -2], "size": 48, "icon": "lock" },
                { "kind": "path", "id": "route", "through": ["client", "gate", [1100, 700, 0], "store"],
                  "corner": 24, "arrow": "end" },
                { "kind": "packet", "id": "write", "beam": "route", "label": "PUT" }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn new_elements_validate_round_trip_and_accept_their_channels() {
        let plan = diagram();
        plan.validate().unwrap();
        let json = serde_json::to_value(&plan).unwrap();
        assert_eq!(json["elements"][2]["fill"], "surface");
        assert_eq!(json["elements"][3]["stroke"], "accent");
        assert!(
            json["elements"][2].get("stroke").is_none(),
            "muted is the default"
        );
        assert_eq!(
            json["elements"][5]["through"][2],
            serde_json::json!([1100.0, 700.0, 0.0])
        );
        assert_eq!(serde_json::from_value::<StagePlan>(json).unwrap(), plan);
        for property in [
            "store.morph",
            "store.pitch",
            "gate.draw",
            "gate.rotation",
            "route.trim",
            "lock.flash",
        ] {
            assert!(plan.accepts(property), "{property}");
        }
        assert!(!plan.accepts("route.x") && !plan.accepts("lock.morph"));
        let no_stroke: StageElement = serde_json::from_value(serde_json::json!({
            "kind": "shape", "id": "s", "at": [0, 0, 0], "shape": { "circle": 10 },
            "fill": "accent", "stroke": null
        }))
        .unwrap();
        assert!(matches!(
            no_stroke,
            StageElement::Shape { stroke: None, .. }
        ));
    }

    #[test]
    fn new_elements_reject_invalid_payloads() {
        let broken = |edit: &dyn Fn(&mut serde_json::Value)| {
            let mut json = serde_json::to_value(diagram()).unwrap();
            edit(&mut json["elements"]);
            serde_json::from_value::<StagePlan>(json).map_or(true, |plan| plan.validate().is_err())
        };
        assert!(!broken(&|_| {}), "the diagram itself is valid");
        assert!(broken(&|e| e[0]["shapes"] = serde_json::json!([])));
        assert!(
            broken(&|e| {
                e[0]["shapes"] = serde_json::json!([{ "shape": "plane", "size": [300, 200] }]);
                e[0]["points"] = serde_json::json!(359);
            }),
            "a prime count cannot fill a dot matrix"
        );
        assert!(broken(
            &|e| e[0]["shapes"][0]["edges"] = serde_json::json!(2.0)
        ));
        assert!(broken(
            &|e| e[0]["shapes"][0]["shape"] = serde_json::json!("cone")
        ));
        assert!(
            broken(&|e| {
                e[2]["fill"] = serde_json::Value::Null;
                e[2]["stroke"] = serde_json::Value::Null;
            }),
            "a shape needs ink"
        );
        assert!(
            broken(&|e| e[2]["arrow"] = serde_json::json!("end")),
            "closed shapes have no ends"
        );
        assert!(broken(
            &|e| e[3]["shape"]["arc"]["sweep"] = serde_json::json!(0)
        ));
        assert!(broken(&|e| e[4]["icon"] = serde_json::json!("lok")));
        assert!(
            broken(&|e| e[4]["path"] = serde_json::json!("M0 0 L10 10 Z")),
            "one source"
        );
        assert!(broken(&|e| e[5]["through"] = serde_json::json!(["client"])));
        assert!(
            broken(&|e| e[5]["through"][1] = serde_json::json!("route")),
            "not itself"
        );
        assert!(
            broken(&|e| e[5]["through"][1] = serde_json::json!("client")),
            "no repeats"
        );
        assert!(
            broken(&|e| e[5]["curve"] = serde_json::json!("smooth")),
            "smooth takes points"
        );
        assert!(
            broken(&|e| {
                e[5]["through"] = serde_json::json!([[0, 0, 0], [1, 1, 0], [2, 2, 0]]);
                e[5]["curve"] = serde_json::json!("bezier");
            }),
            "a bezier chain has 3n + 1 points"
        );
        assert!(broken(&|e| e[5]["width"] = serde_json::json!(0)));
        assert!(broken(&|e| e[6]["beam"] = serde_json::json!("gate")));
    }

    #[test]
    fn a_path_relays_its_packet_through_each_stop() {
        let plan = diagram();
        assert_eq!(plan.legs("write"), vec![Some("gate"), Some("store")]);
        let mut scene = PlanBuilder::new("relay", 8_000_000_000);
        let mut stage = StageActor::declare(&mut scene, "stage", &plan).unwrap();
        let arrivals = stage.relay(&mut scene, "write", 1_000_000_000, 1.0);
        // Two legs of 0.5 s; the second gathers after the first lands and rests.
        assert_eq!(
            arrivals,
            vec![
                1_500_000_000,
                1_500_000_000 + 120_000_000 + 340_000_000 + 500_000_000
            ]
        );
        let plan = scene.finish().unwrap();
        let channel = |property: &str| {
            plan.continuous_channels
                .iter()
                .find(|c| c.property == property)
                .unwrap_or_else(|| panic!("missing {property}"))
        };
        assert!(
            matches!(
                channel("write.age").events[1],
                crate::plan::TrackEventPlan::Ease {
                    duration_nanos: 4_960_000_000,
                    ..
                }
            ),
            "the clock runs through the last leg's life"
        );
        channel("gate.flash");
        channel("store.pulse");
        assert_eq!(packet::leg_start(0, 2, 1.0), 0.0);
        assert!((packet::leg_arrival(1, 2, 1.0) - 1.8).abs() < 1e-6);
        assert_eq!(
            packet::lifetime(1, 0.8),
            packet::LIFETIME,
            "a beam is one leg"
        );
    }

    #[test]
    fn a_packet_can_be_sent_again_after_it_lands() {
        let mut scene = PlanBuilder::new("resend", 12_000_000_000);
        let mut stage = StageActor::declare(&mut scene, "stage", &plan()).unwrap();
        let first = stage.send(&mut scene, "probe", 1_000_000_000, 0.8);
        let second = stage.send(&mut scene, "probe", 6_000_000_000, 0.5);
        assert_eq!((first, second), (1_800_000_000, 6_500_000_000));
        let plan = scene.finish().unwrap();
        let timeline = crate::plan::compile_channels(
            plan.continuous_channels
                .iter()
                .map(|c| (c, crate::timeline::PropertyId::new(&c.property))),
            plan.duration_nanos,
            |scalar| match scalar {
                crate::plan::ScalarPlan::Literal(value) => Ok(*value),
                _ => unreachable!(),
            },
        )
        .unwrap();
        let sample = |property: &str, time: f64| {
            timeline
                .sample_at(&crate::timeline::PropertyId::new(property), time)
                .unwrap()
                .position
        };
        assert!((sample("probe.age", 1.66) - 1.0).abs() < 1e-3);
        assert!(
            (sample("probe.age", 5.0) - packet::LIFETIME).abs() < 1e-3,
            "spent between sends"
        );
        assert!(
            sample("probe.age", 5.66) < 0.01,
            "the second dispatch restarts the clock"
        );
        assert_eq!(sample("probe.flight", 5.7), 0.5);
    }

    #[test]
    fn forms_match_their_shapes_and_morph_exactly_onto_each() {
        let shapes = [FormShape::cube(200.0), FormShape::Sphere { radius: 130.0 }];
        let points = form_points(&shapes, 300);
        assert_eq!(points.len(), 2);
        assert!(points.iter().all(|shape| shape.len() == 300));
        assert_eq!(points, form_points(&shapes, 300), "deterministic");
        for index in [0, 17, 299] {
            let seed = hash(index as u32, 3);
            assert_eq!(morph_point(&points, index, seed, 0.0), points[0][index]);
            assert_eq!(morph_point(&points, index, seed, 1.0), points[1][index]);
            assert_eq!(
                morph_point(&points, index, seed, 7.0),
                points[1][index],
                "held at the last"
            );
            let middle = morph_point(&points, index, seed, 0.5);
            assert!(middle.is_finite() && middle != points[0][index]);
        }
        // Early points are still on the cube while late ones have left.
        let moved = (0..300)
            .filter(|&i| morph_point(&points, i, hash(i as u32, 3), 0.2) != points[0][i])
            .count();
        assert!(moved > 0 && moved < 300, "{moved}");
        assert_eq!(shapes[0].radius(), Vec3::splat(100.0).length());
    }

    #[test]
    fn new_elements_attach_to_their_resting_outlines() {
        use crate::math::vec2;
        let plan = diagram();
        let outline = |id: &str| plan.element(id).unwrap().outline(vec2(0.0, 0.0), 1.0);
        assert_eq!(
            outline("gate"),
            Shape::Box(Box2::from_center_size(Vec2::ZERO, vec2(120.0, 160.0)))
        );
        assert!(matches!(
            outline("loop"),
            Shape::Circle(Circle { radius: 50.0, .. })
        ));
        assert_eq!(outline("lock").distance(vec2(24.0, 0.0)), 0.0);
        assert!(matches!(outline("store"), Shape::Box(_)));
        let triangle: StageElement = serde_json::from_value(serde_json::json!({
            "kind": "shape", "id": "t", "at": [0, 0, 0],
            "shape": { "polygon": [[0, -40], [40, 30], [-40, 30]] }
        }))
        .unwrap();
        let Shape::Polygon(hull) = triangle.outline(vec2(100.0, 100.0), 2.0) else {
            panic!("a polygon attaches to its hull");
        };
        assert_eq!(hull.vertices().len(), 3);
        assert_eq!(
            hull.port_toward(vec2(100.0, 1000.0)).point,
            vec2(100.0, 160.0)
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
