//! Experimental component payloads for the native showroom. These are not a
//! settled public authoring API; keep or remove them after visual review.
use serde::{Deserialize, Serialize};

pub const TYPESET: &str = "prototype-typeset";
pub const COLLECTION: &str = "prototype-collection";
pub const CONNECTOR: &str = "prototype-connector";
pub const RICH_TEXT: &str = "prototype-rich-text";
pub const WIDTH_TEXT: &str = "prototype-width-text";
pub const VENN: &str = "prototype-venn";
pub const HEADER: &str = "prototype-header";
pub const DIAGRAM: &str = "prototype-diagram";

/// Provisional finite box-and-wire surface. Positions/timing are authored scalar
/// channels, not an automatic graph layout or an OpenCode-specific state machine.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramPlan {
    #[serde(default)]
    pub view: DiagramView,
    /// Reference-card metadata retained in the provisional payload. Bare views
    /// do not paint the frame, title, captions, dots or step indicators.
    pub bounds: [f32; 4],
    pub title: String,
    pub captions: Vec<String>,
    pub nodes: Vec<DiagramNode>,
    pub links: Vec<DiagramLink>,
    #[serde(default)]
    pub delays: Vec<DiagramDelay>,
}

/// A presentation of the same authored boxes/ports, not a different graph.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DiagramView {
    #[default]
    Flat,
    Isometric,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramNode {
    pub id: String,
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub label: String,
    pub alternate_label: Option<String>,
    pub muted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiagramAnchor {
    pub node: String,
    pub side: Side,
    /// Tangential offset in authored pixels, clamped to the sampled footprint
    /// edge. Isometric ports are halfway down the current side-face depth.
    pub offset: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiagramLink {
    pub id: String,
    pub from: DiagramAnchor,
    pub to: DiagramAnchor,
}

/// Native opt-in resting-pose waits. The Scene Program must also place authored
/// video events at these offsets; Playback cancels superseded unstarted writes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiagramDelay {
    pub property: String,
    pub from: f32,
    pub to: f32,
    pub millis: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderPlan {
    pub origin: [f32; 2],
    pub text: String,
    pub font_size: f32,
    pub width: f32,
    pub split: HeaderSplit,
    pub stagger_millis: u64,
    pub duration_seconds: f32,
    #[serde(default)]
    pub reflection: Option<HeaderReflection>,
    pub visible: bool,
    #[serde(default)]
    pub events: Vec<HeaderEvent>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum HeaderSplit {
    Line,
    Words,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct HeaderReflection {
    pub opacity: f32,
    pub depth: f32,
    pub gap: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderEvent {
    pub at_nanos: u64,
    pub visible: bool,
}

/// Immutable Markdown; separate actors/blocks may animate without replacing
/// retained prose. No HTML, image fetching, or browser layout is implied.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RichTextPlan {
    pub origin: [f32; 2],
    pub width: f32,
    pub font_size: f32,
    pub markdown: String,
    /// Stationary canvas-space top/bottom/linear-feather aperture for header rises.
    #[serde(default)]
    pub vertical_mask: Option<[f32; 3]>,
    /// Optional optical softness during fades. Prose is sharp by default;
    /// title treatments may opt in without changing their animation profile.
    #[serde(default)]
    pub fade_blur: f32,
}

/// Two authored sets. Geometry is explicit: no type evaluator or inferred
/// mathematical relationship based on the spelling of the labels.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VennPlan {
    pub center: [f32; 2],
    pub left: String,
    pub right: String,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Font {
    #[default]
    Sans,
    Mono,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPart {
    pub id: String,
    pub text: String,
    pub color: [u8; 3],
    /// Optional syntax runs within this one animated part. Their concatenation
    /// must equal `text`; `color` remains the legacy unstyled fallback.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spans: Vec<crate::code::StyledSpan>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypesetPlan {
    pub origin: [f32; 2],
    pub font: Font,
    pub font_size: f32,
    pub parts: Vec<TextPart>,
    pub visible: Vec<String>,
    #[serde(default)]
    pub events: Vec<TextEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEvent {
    pub at_nanos: u64,
    pub visible: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Arrangement {
    Row,
    Column,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSnapshot {
    pub order: Vec<String>,
    pub arrangement: Arrangement,
    pub focus: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPlan {
    pub origin: [f32; 2],
    pub font: Font,
    pub font_size: f32,
    pub gap: f32,
    pub items: Vec<TextPart>,
    pub initial: CollectionSnapshot,
    #[serde(default)]
    pub events: Vec<CollectionEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionEvent {
    pub at_nanos: u64,
    pub snapshot: CollectionSnapshot,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Anchor {
    pub actor: String,
    pub item: String,
    pub side: Side,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorPlan {
    pub from: Anchor,
    pub to: Anchor,
    pub color: [u8; 3],
    pub accent: [u8; 3],
}
