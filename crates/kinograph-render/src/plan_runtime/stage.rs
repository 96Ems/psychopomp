//! Prepared Stage root: decoded and validated once, with strict channel names,
//! and GPU resources (text atlas, bloom chain, pipelines) built at preparation.
use anyhow::Result;
use kinograph::{
    plan::{ActorPlan, ContinuousChannelPlan},
    stage::StagePlan,
};

use super::preflight::{decode, strict_channels};
use crate::render::{HeadlessRenderer, StageGpu};

pub(super) fn validate_recipe(
    actor: &ActorPlan,
    channels: &[ContinuousChannelPlan],
) -> Result<StagePlan> {
    let plan = decode(actor, "stage", StagePlan::validate)?;
    strict_channels(&actor.id, channels, "stage", |property| {
        plan.accepts(property)
    })?;
    Ok(plan)
}

pub(super) struct PreparedStage {
    id: String,
    plan: StagePlan,
    gpu: StageGpu,
}

impl PreparedStage {
    pub(super) fn from_recipe(
        id: String,
        plan: StagePlan,
        renderer: &mut HeadlessRenderer,
    ) -> Result<Self> {
        let gpu = renderer.prepare_stage(&plan)?;
        Ok(Self { id, plan, gpu })
    }

    pub(super) fn id(&self) -> &str {
        &self.id
    }

    /// One frame exposed through weighted shutter samples, accumulated on the GPU.
    pub(super) fn render_exposure(
        &self,
        renderer: &mut HeadlessRenderer,
        exposure: &[(f64, f32)],
        value: impl Fn(&str, &str, f64, f32) -> f32,
    ) -> Result<Vec<u8>> {
        renderer.render_stage_exposure(
            &self.plan,
            &self.gpu,
            exposure,
            |time, property, default| value(&self.id, property, time, default),
        )
    }

    pub(super) fn render(
        &self,
        renderer: &mut HeadlessRenderer,
        time: f64,
        value: impl Fn(&str, &str, f32) -> f32,
    ) -> Result<Vec<u8>> {
        renderer.render_stage(&self.plan, &self.gpu, time, |property, default| {
            value(&self.id, property, default)
        })
    }
}

#[cfg(test)]
mod tests {
    use kinograph::{author::PlanBuilder, stage::StageActor};

    use super::super::validate_renderer_plan;

    fn plan(extra: Option<&str>) -> kinograph::plan::ScenePlan {
        let recipe = serde_json::from_value(serde_json::json!({
            "elements": [
                { "kind": "orb", "id": "service", "at": [960, 460, 0], "radius": 150 },
                { "kind": "card", "id": "client", "at": [420, 300, 0], "size": [300, 110], "title": "client" },
                { "kind": "beam", "id": "link", "from": "client", "to": "service" },
                { "kind": "packet", "id": "probe", "beam": "link", "label": "GET" }
            ]
        }))
        .unwrap();
        let mut scene = PlanBuilder::new("stage-preflight", 3_000_000_000);
        let mut stage = StageActor::declare(&mut scene, "stage", &recipe).unwrap();
        stage.send(&mut scene, "probe", 500_000_000, 0.6);
        stage.to(&mut scene, "camera.z", 0, 120.0, 1.0);
        if let Some(property) = extra {
            stage.to(&mut scene, property, 0, 1.0, 0.3);
        }
        scene.finish().unwrap()
    }

    #[test]
    fn stage_preflight_accepts_element_channels_and_rejects_typos() {
        validate_renderer_plan(&plan(None)).unwrap();
        validate_renderer_plan(&plan(Some("service.shatter"))).unwrap();
        for typo in [
            "service.shater",
            "client.travel",
            "camera.roll",
            "ghost.opacity",
        ] {
            let error = validate_renderer_plan(&plan(Some(typo))).unwrap_err();
            assert!(
                format!("{error:#}").contains("unknown property"),
                "{typo}: {error:#}"
            );
        }
    }
}
