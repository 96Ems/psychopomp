//! Plain text: one centered line of CommitMono at a canvas point, or pinned to
//! an anchor. `x` and `y` channels move it (absolute canvas coordinates that
//! default to `center`; while pinned, their displacement from `center` moves
//! it from the anchor), and `opacity` fades it. Its content may also follow a
//! `content` State Channel. The JSON shape is the one hand-built `text` actors
//! always used, so older plans decode unchanged.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    anchor::{self, AnchorPlan},
    author::{ActorHandle, ContinuousHandle, PlanBuilder},
};

pub const TEXT_RECIPE: &str = "text";

/// Every scalar channel a text actor accepts, besides `anchor.<id>` weights.
pub const TEXT_CHANNELS: [&str; 3] = ["opacity", "x", "y"];

const DEFAULT_SIZE: f32 = 28.0;
const WHITE: [u8; 3] = [255, 255, 255];

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextPlan {
    pub text: String,
    /// Where the line's center sits at rest.
    pub center: [f32; 2],
    #[serde(default = "default_size", skip_serializing_if = "is_default_size")]
    pub font_size: f32,
    #[serde(default = "white", skip_serializing_if = "is_white")]
    pub color: [u8; 3],
    /// A stationary canvas-space aperture the text moves through.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical_mask: Option<TextMaskPlan>,
    /// Places the text can pin to; while it has any, the blended anchor (plus
    /// that anchor's offset) replaces `center`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<AnchorPlan>,
}

/// Coverage ramps from zero at `top` to full at `top + fade`, and back to zero
/// at `bottom`; everything outside is clipped.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TextMaskPlan {
    pub top: f32,
    pub bottom: f32,
    pub fade: f32,
}

fn default_size() -> f32 {
    DEFAULT_SIZE
}
fn is_default_size(size: &f32) -> bool {
    *size == DEFAULT_SIZE
}
fn white() -> [u8; 3] {
    WHITE
}
fn is_white(color: &[u8; 3]) -> bool {
    *color == WHITE
}

impl TextPlan {
    pub fn new(text: impl Into<String>, center: [f32; 2]) -> Self {
        Self {
            text: text.into(),
            center,
            font_size: DEFAULT_SIZE,
            color: WHITE,
            vertical_mask: None,
            anchors: Vec::new(),
        }
    }

    pub fn size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }

    pub fn color(mut self, color: [u8; 3]) -> Self {
        self.color = color;
        self
    }

    pub fn masked(mut self, mask: TextMaskPlan) -> Self {
        self.vertical_mask = Some(mask);
        self
    }

    /// Pin the text's center to `anchor`; the first anchor is where it starts.
    pub fn anchor(mut self, anchor: AnchorPlan) -> Self {
        self.anchors.push(anchor);
        self
    }

    /// True when `property` names one of this text's channels.
    pub fn accepts(&self, property: &str) -> bool {
        TEXT_CHANNELS.contains(&property) || anchor::accepts(property, &self.anchors)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.center.iter().all(|v| v.is_finite()),
            "text center must be finite"
        );
        ensure!(
            self.font_size.is_finite() && self.font_size > 0.0 && self.font_size <= 512.0,
            "text fontSize must be finite and in (0, 512]"
        );
        if let Some(mask) = self.vertical_mask {
            ensure!(
                mask.top.is_finite()
                    && mask.bottom.is_finite()
                    && mask.fade.is_finite()
                    && mask.top < mask.bottom
                    && (0.0..=(mask.bottom - mask.top) * 0.5).contains(&mask.fade),
                "text verticalMask requires finite top < bottom and fade in [0, half the mask height]"
            );
        }
        anchor::validate("text", &self.anchors)
    }
}

/// Authoring handle for one text actor.
#[derive(Clone, Debug)]
pub struct TextActor {
    actor: ActorHandle,
    center: [f32; 2],
    anchors: Vec<String>,
}

/// How far below its rest a text rises from as it appears.
const RISE: f32 = 10.0;
/// A swap's outgoing words lift this far as the incoming ones rise into place.
const SWAP_DRIFT: f32 = 6.0;
/// The incoming words wait this long, so a swap reads as one label changing.
const SWAP_LAG: u64 = 80_000_000;

impl TextActor {
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: &TextPlan,
    ) -> Result<Self> {
        plan.validate()?;
        let actor = scene.actor(id, TEXT_RECIPE, plan)?;
        Ok(Self {
            actor,
            center: plan.center,
            anchors: anchor::ids(&plan.anchors),
        })
    }

    pub fn id(&self) -> &str {
        self.actor.id()
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
        scene.channel(&self.actor, property, initial)
    }

    /// Fade and rise into place. A text whose first write is `show` starts
    /// hidden and slightly low; otherwise text is visible from the start.
    pub fn show(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        let opacity = self.channel(scene, "opacity", 0.0);
        let y = self.channel(scene, "y", self.center[1] + RISE);
        scene.spring(&opacity, at_nanos, 1.0, 0.35, 0.0);
        scene.spring_with(&y, at_nanos, self.center[1], crate::plan::SpringPlan::ENTER);
    }

    /// Fade out in place.
    pub fn hide(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        let opacity = self.channel(scene, "opacity", 1.0);
        scene.spring_with(&opacity, at_nanos, 0.0, crate::plan::SpringPlan::EXIT);
    }

    /// Show from `from` and hide at `until`.
    pub fn show_during(&mut self, scene: &mut PlanBuilder, from: u64, until: u64) {
        self.show(scene, from);
        self.hide(scene, until);
    }

    /// Crossfade into `next` where this text stands, so one label reads as
    /// changing its words: this one lifts a few pixels as it fades, and
    /// `next` rises into the same place a beat later, so the two lines barely
    /// overlap. `next` starts hidden unless it was written earlier.
    pub fn swap(&mut self, scene: &mut PlanBuilder, next: &mut TextActor, at_nanos: u64) {
        let outgoing = self.channel(scene, "opacity", 1.0);
        let lift = self.channel(scene, "y", self.center[1]);
        let incoming = next.channel(scene, "opacity", 0.0);
        let rise = next.channel(scene, "y", next.center[1] + SWAP_DRIFT);
        scene.spring(&outgoing, at_nanos, 0.0, 0.2, 0.0);
        scene.spring(&lift, at_nanos, self.center[1] - SWAP_DRIFT, 0.3, 0.0);
        scene.spring(&incoming, at_nanos + SWAP_LAG, 1.0, 0.3, 0.0);
        scene.spring(&rise, at_nanos + SWAP_LAG, next.center[1], 0.4, 0.0);
    }

    /// Glide to the anchor `to`, carrying velocity through interruptions.
    pub fn move_to(&mut self, scene: &mut PlanBuilder, to: &str, at_nanos: u64) -> Result<()> {
        anchor::move_to(scene, &self.actor, &self.anchors, to, at_nanos)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{anchor::Edge, plan::ScalarPlan};

    #[test]
    fn text_plans_keep_the_hand_built_json_shape() {
        let plan = TextPlan::new("hello", [960.0, 780.0])
            .size(30.0)
            .color([170, 182, 200])
            .masked(TextMaskPlan {
                top: 750.0,
                bottom: 810.0,
                fade: 12.0,
            });
        plan.validate().unwrap();
        let json = serde_json::to_value(&plan).unwrap();
        assert_eq!(
            json,
            json!({"text": "hello", "center": [960.0, 780.0], "fontSize": 30.0,
                "color": [170, 182, 200], "verticalMask": {"top": 750.0, "bottom": 810.0, "fade": 12.0}})
        );
        // Hand-built JSON from older Scene Programs decodes to the same plan.
        let old = json!({"text": "hello", "center": [960, 780], "fontSize": 30,
            "color": [170, 182, 200], "verticalMask": {"top": 750, "bottom": 810, "fade": 12}});
        assert_eq!(serde_json::from_value::<TextPlan>(old).unwrap(), plan);
        let minimal = TextPlan::new("hi", [0.0, 0.0]);
        assert_eq!(
            serde_json::to_value(&minimal).unwrap(),
            json!({"text": "hi", "center": [0.0, 0.0]})
        );
    }

    #[test]
    fn invalid_text_is_rejected() {
        assert!(TextPlan::new("x", [f32::NAN, 0.0]).validate().is_err());
        assert!(TextPlan::new("x", [0.0; 2]).size(0.0).validate().is_err());
        let mask = TextMaskPlan {
            top: 10.0,
            bottom: 20.0,
            fade: 6.0,
        };
        assert!(
            TextPlan::new("x", [0.0; 2])
                .masked(mask)
                .validate()
                .is_err()
        );
        let twice = TextPlan::new("x", [0.0; 2])
            .anchor(AnchorPlan::stage("a", "card", Edge::Top))
            .anchor(AnchorPlan::stage("a", "card", Edge::Bottom));
        assert!(twice.validate().is_err());
    }

    #[test]
    fn swapping_crossfades_in_place_and_moves_between_anchors() {
        let mut scene = PlanBuilder::new("text", 5_000_000_000);
        let pinned = TextPlan::new("waiting", [960.0, 540.0])
            .anchor(AnchorPlan::stage("client", "client", Edge::Bottom).with_offset([0.0, 30.0]))
            .anchor(AnchorPlan::stage("api", "api", Edge::Bottom).with_offset([0.0, 30.0]));
        let mut before = TextActor::declare(&mut scene, "before", &pinned).unwrap();
        let mut after = TextActor::declare(
            &mut scene,
            "after",
            &TextPlan {
                text: "done".into(),
                ..pinned.clone()
            },
        )
        .unwrap();
        before.show(&mut scene, 500_000_000);
        before.swap(&mut scene, &mut after, 2_000_000_000);
        after.move_to(&mut scene, "api", 3_000_000_000).unwrap();
        assert!(after.move_to(&mut scene, "nowhere", 3_000_000_000).is_err());
        let plan = scene.finish().unwrap();
        let channel = |actor: &str, property: &str| {
            plan.continuous_channels
                .iter()
                .find(|c| c.actor_id == actor && c.property == property)
                .unwrap()
        };
        assert!(matches!(
            channel("before", "y").initial,
            ScalarPlan::Literal(550.0)
        ));
        assert!(matches!(
            channel("after", "opacity").initial,
            ScalarPlan::Literal(0.0)
        ));
        assert_eq!(channel("before", "opacity").events.len(), 2);
        assert!(
            plan.continuous_channels
                .iter()
                .filter(|c| c.actor_id == "after")
                .all(|c| pinned.accepts(&c.property))
        );
    }
}
