//! Prepared lenses: decoded and validated once with strict channel names.
//! Like callouts, a lens resolves its anchors against the prepared root at
//! every sample, so the glass stays on its card or code range through camera
//! moves, line motion, and every shutter sample.
use anyhow::{Result, bail};
use psychopomp::{
    lens::{Glass, LensAnchorPlan, LensPlan},
    math::Vec2,
    plan::{ActorPlan, ContinuousChannelPlan, SemanticTargetPlan},
    stage::StageElement,
};

use super::preflight::{RootPlan, decode, strict_channels};

pub(super) struct PreparedLens {
    id: String,
    plan: LensPlan,
}

impl PreparedLens {
    pub(super) fn new(actor: &ActorPlan, channels: &[ContinuousChannelPlan]) -> Result<Self> {
        let plan = decode(actor, "lens", LensPlan::validate)?;
        strict_channels(&actor.id, channels, "lens", |property| {
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
                (LensAnchorPlan::Point { .. }, _) => {}
                (LensAnchorPlan::Stage { element, .. }, RootPlan::Stage { recipe, .. }) => {
                    if recipe
                        .element(element)
                        .and_then(StageElement::anchor)
                        .is_none()
                    {
                        bail!(
                            "lens '{}' anchor '{}' needs a positioned stage element; '{element}' is not one",
                            self.id,
                            anchor.id()
                        );
                    }
                }
                (LensAnchorPlan::Editor { target, .. }, RootPlan::Editor { editor, .. }) => {
                    if !targets
                        .iter()
                        .any(|t| &t.id == target && t.actor_id == editor.actor_id())
                    {
                        bail!(
                            "lens '{}' anchor '{}' references unknown editor semantic target '{target}'",
                            self.id,
                            anchor.id()
                        );
                    }
                }
                (LensAnchorPlan::Stage { .. }, _) => bail!(
                    "lens '{}' anchor '{}' needs a stage root",
                    self.id,
                    anchor.id()
                ),
                (LensAnchorPlan::Editor { .. }, _) => bail!(
                    "lens '{}' anchor '{}' needs an editor root",
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
            .any(|anchor| matches!(anchor, LensAnchorPlan::Stage { .. }))
    }

    /// The glass at one sample: anchors resolved and blended by their weights.
    /// `None` while absent or when no weighted anchor can be placed.
    pub(super) fn glass(
        &self,
        value: impl Fn(&str, &str, f32) -> f32,
        resolve: impl Fn(&LensAnchorPlan) -> Option<Vec2>,
    ) -> Option<Glass> {
        if value(&self.id, "presence", 1.0) <= 1e-3 {
            return None;
        }
        let mut center = Vec2::ZERO;
        let mut total = 0.0;
        for (index, anchor) in self.plan.anchors.iter().enumerate() {
            let weight = value(
                &self.id,
                &LensPlan::weight_property(anchor.id()),
                if index == 0 { 1.0 } else { 0.0 },
            );
            if weight.abs() < 1e-6 {
                continue;
            }
            let Some(point) = resolve(anchor) else {
                continue;
            };
            center += point * weight;
            total += weight;
        }
        if total.abs() <= 1e-6 {
            return None;
        }
        self.plan.glass(center / total, |property, default| {
            value(&self.id, property, default)
        })
    }
}

#[cfg(test)]
mod tests {
    use psychopomp::{
        author::PlanBuilder,
        callout::CalloutSide,
        lens::{LensActor, LensAnchorPlan, LensPlan},
        stage::StageActor,
    };

    use super::super::validate_renderer_plan;

    fn plan(anchor: LensAnchorPlan, extra: Option<&str>) -> psychopomp::plan::ScenePlan {
        let recipe = serde_json::from_value(serde_json::json!({
            "elements": [
                { "kind": "card", "id": "api", "at": [960, 540, 0], "size": [300, 110], "title": "api" },
                { "kind": "card", "id": "client", "at": [420, 540, 0], "size": [300, 110], "title": "client" },
                { "kind": "beam", "id": "link", "from": "client", "to": "api" }
            ]
        }))
        .unwrap();
        let mut scene = PlanBuilder::new("lens-preflight", 2_000_000_000);
        StageActor::declare(&mut scene, "stage", &recipe).unwrap();
        let mut lens =
            LensActor::declare(&mut scene, "loupe", &LensPlan::circle(anchor, 240.0)).unwrap();
        lens.show(&mut scene, 0);
        if let Some(property) = extra {
            let channel = lens.channel(&mut scene, property, 0.0);
            scene.set(&channel, 100_000_000, 1.0);
        }
        scene.finish().unwrap()
    }

    fn stage(element: &str) -> LensAnchorPlan {
        LensAnchorPlan::Stage {
            id: "pin".into(),
            element: element.into(),
            edge: CalloutSide::Center,
            side: None,
        }
    }

    #[test]
    #[ignore = "requires a headless GPU; a Stage lens rides the camera and touches only its own bounds"]
    fn stage_lenses_ride_the_camera_and_change_only_their_bounds() {
        use std::path::Path;

        use super::super::{PreparedPlan, PreparedRoot, new_renderer};
        use crate::render::composite_lens;

        let mut plan = plan(stage("client"), None);
        let camera = psychopomp::plan::ContinuousChannelPlan {
            id: "stage.camera.z".into(),
            actor_id: "stage".into(),
            property: "camera.z".into(),
            initial: 0.0.into(),
            events: vec![psychopomp::plan::TrackEventPlan::Ease {
                at_nanos: 1_000_000_000,
                target: 300.0.into(),
                duration_nanos: 1_000_000_000,
                curve: psychopomp::math::easing::Ease::Smootherstep,
            }],
        };
        plan.continuous_channels.push(camera);
        let mut renderer = pollster::block_on(new_renderer(&plan.id)).unwrap();
        let prepared = PreparedPlan::prepare(plan, Path::new("."), &mut renderer).unwrap();
        let PreparedRoot::Stage(stage) = &prepared.root else {
            panic!("a stage root");
        };
        let size = renderer.size();
        // The lens's own channels rest while the camera dollies: only its
        // resolved anchor tells the shutter samples apart.
        let key = |time| prepared.overlay_key(time, stage.id(), size).unwrap();
        assert_ne!(key(1.5), key(1.51));
        assert_eq!(key(2.5), key(2.51), "a resting lens merges its samples");
        assert!(
            !prepared
                .only_callouts_differ(&[(1.5, 0.5), (1.51, 0.5)], stage.id())
                .unwrap(),
            "a visible lens composes whole samples"
        );
        let glass = prepared
            .lens_glass(&prepared.lenses[0], 0.9, &prepared.timeline, size)
            .unwrap();
        let center = glass.outline.center;
        assert!((center.x - 420.0).abs() < 1.0 && (center.y - 540.0).abs() < 1.0);
        let base = renderer.render_title_card("", None, 0.0);
        let mut lensed = base.clone();
        composite_lens(&mut lensed, size, &glass);
        let bounds = glass.bounds();
        let outside = |x: f32, y: f32| {
            x + 1.0 < bounds.min.x || x > bounds.max.x || y + 1.0 < bounds.min.y || y > bounds.max.y
        };
        for (index, (a, b)) in base
            .as_chunks::<4>()
            .0
            .iter()
            .zip(lensed.as_chunks::<4>().0)
            .enumerate()
        {
            let (x, y) = (
                (index % size[0] as usize) as f32,
                (index / size[0] as usize) as f32,
            );
            if outside(x, y) {
                assert_eq!(a, b, "{x},{y}");
            }
        }
        assert!(base != lensed);
    }

    #[test]
    #[ignore = "requires a headless GPU; a lens over a still page reuses the page per sample, exactly"]
    fn lensed_exposures_match_whole_samples() {
        use std::path::Path;

        use psychopomp::callout::CalloutAnchorPlan;

        use super::super::{PreparedPlan, new_renderer};

        let mut scene = PlanBuilder::new("lens-exposure", 2_000_000_000);
        scene
            .actor(
                "title",
                "title-card",
                serde_json::json!({ "title": "magnify me" }),
            )
            .unwrap();
        let mut lens = LensActor::declare(
            &mut scene,
            "loupe",
            &LensPlan::circle(
                CalloutAnchorPlan::Point {
                    id: "here".into(),
                    at: [860.0, 540.0],
                    side: None,
                },
                220.0,
            ),
        )
        .unwrap();
        lens.slide(&mut scene, [200.0, 0.0], 500_000_000);
        let plan = scene.finish().unwrap();
        let mut renderer = pollster::block_on(new_renderer(&plan.id)).unwrap();
        let prepared = PreparedPlan::prepare(plan, Path::new("."), &mut renderer).unwrap();
        for center in [0.25, 0.8, 1.9] {
            let exposure = crate::exposure::exposure(center, 1.0 / 60.0, 8);
            let fast = prepared.render_exposure(&mut renderer, &exposure).unwrap();
            let whole = crate::exposure::accumulate(&mut renderer, &exposure, |renderer, time| {
                prepared.render_sample(renderer, time)
            })
            .unwrap();
            assert!(fast == whole, "exposure at {center}");
        }
    }

    #[test]
    fn lens_preflight_checks_anchors_against_the_root_and_channels_strictly() {
        validate_renderer_plan(&plan(stage("api"), Some("anchor.pin"))).unwrap();
        validate_renderer_plan(&plan(stage("api"), Some("focus-y"))).unwrap();
        for (anchor, extra, expected) in [
            (stage("api"), Some("anchor.nowhere"), "unknown property"),
            (stage("api"), Some("radius"), "unknown property"),
            (stage("link"), None, "positioned stage element"),
            (stage("ghost"), None, "positioned stage element"),
            (
                LensAnchorPlan::Editor {
                    id: "pin".into(),
                    target: "range".into(),
                    edge: CalloutSide::Center,
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
