//! Prepared Sequence Diagrams: the recipe is decoded and validated once, and
//! every channel on the actor must name a real participant, row, or property.
use anyhow::Result;
use kinograph::{
    plan::{ActorPlan, ContinuousChannelPlan},
    sequence::SequencePlan,
};

use super::preflight::{decode, strict_channels};
use crate::render::HeadlessRenderer;

pub(super) struct PreparedSequence {
    id: String,
    plan: SequencePlan,
}

impl PreparedSequence {
    pub(super) fn new(actor: &ActorPlan, channels: &[ContinuousChannelPlan]) -> Result<Self> {
        let plan = decode(actor, "sequence", SequencePlan::validate)?;
        strict_channels(&actor.id, channels, "sequence", |property| {
            accepts(&plan, property)
        })?;
        Ok(Self {
            id: actor.id.clone(),
            plan,
        })
    }

    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &mut HeadlessRenderer,
        sample: impl Fn(&str, &str, f32) -> f32,
    ) {
        renderer.composite_sequence(pixels, &self.plan, |property, default| {
            sample(&self.id, property, default)
        });
    }
}

fn accepts(plan: &SequencePlan, property: &str) -> bool {
    if matches!(property, "opacity" | "x" | "y" | "lifelines") {
        return true;
    }
    let parts = property.split('.').collect::<Vec<_>>();
    match parts.as_slice() {
        ["participant", id, "opacity" | "emphasis"] => plan.participant_index(id).is_some(),
        ["row", id, "reveal" | "opacity" | "strike"] => plan.rows.iter().any(|row| row.id() == *id),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use kinograph::{author::PlanBuilder, sequence::SequenceActor};

    use super::super::validate_renderer_plan;

    fn plan(extra: Option<&str>) -> kinograph::plan::ScenePlan {
        let recipe = serde_json::from_value(serde_json::json!({
            "origin": [200, 160], "width": 1400, "rowHeight": 80,
            "participants": [{ "id": "client", "label": "client" }, { "id": "server", "label": "server" }],
            "rows": [{ "kind": "message", "id": "probe", "from": "client", "to": "server", "label": "GET" }]
        }))
        .unwrap();
        let mut scene = PlanBuilder::new("sequence-preflight", 2_000_000_000);
        let mut sequence = SequenceActor::declare(&mut scene, "flow", &recipe).unwrap();
        sequence.reveal(&mut scene, "probe", 500_000_000);
        sequence.participant(&mut scene, "server", "emphasis", 0.0, 1_000_000_000, 1.0);
        if let Some(property) = extra {
            sequence.animate(&mut scene, property, 0.0, 0, 1.0, 0.3);
        }
        scene.finish().unwrap()
    }

    #[test]
    fn sequence_preflight_accepts_known_channels_and_rejects_typos() {
        validate_renderer_plan(&plan(None)).unwrap();
        validate_renderer_plan(&plan(Some("lifelines"))).unwrap();
        for typo in [
            "row.probe.reveel",
            "row.missing.reveal",
            "participant.daemon.opacity",
            "scale",
        ] {
            let error = validate_renderer_plan(&plan(Some(typo))).unwrap_err();
            assert!(
                format!("{error:#}").contains("unknown property"),
                "{typo}: {error:#}"
            );
        }
    }
}
