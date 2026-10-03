//! Prepared callouts: decoded and validated once with strict channel names.
//! Anchors are resolved against the prepared root at every sample (the Stage
//! camera, the editor's measured code ranges), so a callout is as attached as
//! the pixels it points at, including across shutter samples.
use anyhow::{Result, bail};
use psychopomp::{
    callout::{CalloutAnchorPlan, CalloutLeg, CalloutPlan},
    math::{Vec2, shapes::Box2, vec2},
    motion::MotionState,
    plan::{ActorPlan, ContinuousChannelPlan, SemanticTargetPlan},
    stage::StageElement,
};

use super::{
    PreparedRoot,
    preflight::{RootPlan, decode, strict_channels},
};
use crate::render::{CalloutPose, HeadlessRenderer, stage_anchor};

pub(super) struct PreparedCallout {
    id: String,
    plan: CalloutPlan,
}

impl PreparedCallout {
    pub(super) fn new(actor: &ActorPlan, channels: &[ContinuousChannelPlan]) -> Result<Self> {
        let plan = decode(actor, "callout", CalloutPlan::validate)?;
        strict_channels(&actor.id, channels, "callout", |property| {
            plan.accepts(property)
        })?;
        Ok(Self {
            id: actor.id.clone(),
            plan,
        })
    }

    /// Every anchor must name something the plan's root can place.
    pub(super) fn validate_anchors(
        &self,
        root: &RootPlan,
        targets: &[SemanticTargetPlan],
    ) -> Result<()> {
        for anchor in &self.plan.anchors {
            match (anchor, root) {
                (CalloutAnchorPlan::Point { .. }, _) => {}
                (CalloutAnchorPlan::Stage { element, .. }, RootPlan::Stage { recipe, .. }) => {
                    if recipe
                        .element(element)
                        .and_then(StageElement::anchor)
                        .is_none()
                    {
                        bail!(
                            "callout '{}' anchor '{}' needs a positioned stage element; '{element}' is not one",
                            self.id,
                            anchor.id()
                        );
                    }
                }
                (CalloutAnchorPlan::Editor { target, .. }, RootPlan::Editor { editor, .. }) => {
                    if !targets
                        .iter()
                        .any(|t| &t.id == target && t.actor_id == editor.actor_id())
                    {
                        bail!(
                            "callout '{}' anchor '{}' references unknown editor semantic target '{target}'",
                            self.id,
                            anchor.id()
                        );
                    }
                }
                (CalloutAnchorPlan::Stage { .. }, _) => bail!(
                    "callout '{}' anchor '{}' needs a stage root",
                    self.id,
                    anchor.id()
                ),
                (CalloutAnchorPlan::Editor { .. }, _) => bail!(
                    "callout '{}' anchor '{}' needs an editor root",
                    self.id,
                    anchor.id()
                ),
            }
        }
        Ok(())
    }

    pub(super) fn id(&self) -> &str {
        &self.id
    }

    /// Whether any anchor follows the Stage camera.
    pub(super) fn on_stage(&self) -> bool {
        self.plan
            .anchors
            .iter()
            .any(|anchor| matches!(anchor, CalloutAnchorPlan::Stage { .. }))
    }

    /// The callout at one sample: anchors resolved and blended by their weights.
    /// `None` when hidden or when no weighted anchor can be placed.
    pub(super) fn pose(
        &self,
        value: impl Fn(&str, &str, f32) -> f32,
        resolve: impl Fn(&CalloutAnchorPlan) -> Option<Vec2>,
    ) -> Option<CalloutPose> {
        let opacity = value(&self.id, "opacity", 1.0);
        let draw = value(&self.id, "draw", 1.0);
        let label = value(&self.id, "label", 1.0);
        if opacity <= 0.001 || (draw <= 0.001 && label <= 0.001) {
            return None;
        }
        let mut anchor = Vec2::ZERO;
        let mut legs = Vec::new();
        for (index, plan) in self.plan.anchors.iter().enumerate() {
            let weight = value(
                &self.id,
                &CalloutPlan::weight_property(plan.id()),
                if index == 0 { 1.0 } else { 0.0 },
            );
            if weight.abs() < 1e-6 {
                continue;
            }
            let Some(point) = resolve(plan) else {
                continue;
            };
            anchor += point * weight;
            legs.push((self.plan.leg(plan), weight));
        }
        let total = legs.iter().map(|(_, weight)| weight).sum::<f32>();
        let leg = CalloutLeg::blend(legs)?;
        Some(CalloutPose {
            anchor: anchor / total,
            leg,
            opacity,
            draw,
            label,
            emphasis: value(&self.id, "emphasis", 0.0),
        })
    }

    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &mut HeadlessRenderer,
        pose: CalloutPose,
    ) {
        renderer.composite_callout(pixels, &self.plan, pose);
    }

    /// Everything `render` may ink for `pose`.
    pub(super) fn bounds(
        &self,
        renderer: &mut HeadlessRenderer,
        pose: CalloutPose,
    ) -> Option<Box2> {
        renderer.callout_bounds(&self.plan, pose)
    }
}

/// Where `anchor` is on the delivered frame at `time`, from the prepared root:
/// the root recipe owns the layout, the callout only asks for a point.
pub(super) fn resolve(
    root: &PreparedRoot,
    anchor: &CalloutAnchorPlan,
    size: [u32; 2],
    value: impl Fn(&str, &str, f32) -> f32,
    motion: impl Fn(&str, &str) -> Option<MotionState>,
    time: f64,
) -> Option<Vec2> {
    match (anchor, root) {
        (CalloutAnchorPlan::Point { at, .. }, _) => Some(Vec2::from(*at)),
        (CalloutAnchorPlan::Stage { element, edge, .. }, PreparedRoot::Stage(stage)) => {
            stage_anchor(
                stage.plan(),
                &|property, default| value(stage.id(), property, default),
                time,
                vec2(size[0] as f32, size[1] as f32),
                element,
                *edge,
            )
        }
        (CalloutAnchorPlan::Editor { target, edge, .. }, PreparedRoot::Editor { editor, .. }) => {
            editor.anchor(target, *edge, size, motion)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use psychopomp::{
        author::PlanBuilder,
        callout::{CalloutActor, CalloutAnchorPlan, CalloutPlan, CalloutSide},
        caption::CaptionSpanPlan,
        stage::StageActor,
        tone::Tone,
    };

    use super::super::validate_renderer_plan;

    fn plan(anchor: CalloutAnchorPlan, extra: Option<&str>) -> psychopomp::plan::ScenePlan {
        let recipe = serde_json::from_value(serde_json::json!({
            "elements": [
                { "kind": "card", "id": "api", "at": [960, 540, 0], "size": [300, 110], "title": "api" },
                { "kind": "card", "id": "client", "at": [420, 540, 0], "size": [300, 110], "title": "client" },
                { "kind": "beam", "id": "link", "from": "client", "to": "api" }
            ]
        }))
        .unwrap();
        let mut scene = PlanBuilder::new("callout-preflight", 2_000_000_000);
        StageActor::declare(&mut scene, "stage", &recipe).unwrap();
        let mut callout = CalloutActor::declare(
            &mut scene,
            "note",
            &CalloutPlan::new(anchor, vec![CaptionSpanPlan::new("here", Tone::Plain)]),
        )
        .unwrap();
        callout.show(&mut scene, 0);
        if let Some(property) = extra {
            let channel = callout.channel(&mut scene, property, 0.0);
            scene.set(&channel, 100_000_000, 1.0);
        }
        scene.finish().unwrap()
    }

    fn stage(element: &str) -> CalloutAnchorPlan {
        CalloutAnchorPlan::Stage {
            id: "pin".into(),
            element: element.into(),
            edge: CalloutSide::Top,
            side: None,
        }
    }

    #[test]
    fn callout_preflight_checks_anchors_against_the_root_and_channels_strictly() {
        validate_renderer_plan(&plan(stage("api"), Some("anchor.pin"))).unwrap();
        for (anchor, extra, expected) in [
            (stage("api"), Some("anchor.nowhere"), "unknown property"),
            (stage("api"), Some("emphasys"), "unknown property"),
            (stage("link"), None, "positioned stage element"),
            (stage("ghost"), None, "positioned stage element"),
            (
                CalloutAnchorPlan::Editor {
                    id: "pin".into(),
                    target: "range".into(),
                    edge: CalloutSide::Bottom,
                    side: None,
                },
                None,
                "needs an editor root",
            ),
        ] {
            let error = validate_renderer_plan(&plan(anchor, extra)).unwrap_err();
            assert!(format!("{error:#}").contains(expected), "{error:#}");
        }
    }
}
