//! Trees: the recipe is decoded, flattened, and its per-path channels checked
//! against the tree during preflight. Sampling is channel-only, so trees run
//! in interruptible native playback as well as video.
use anyhow::{Result, bail};
use psychopomp::{
    plan::{ActorPlan, ContinuousChannelPlan},
    tree::{TreeChannel, TreeModel, TreePlan},
};

use super::preflight::{decode, strict_channels};
use crate::render::{HeadlessRenderer, TreeNames};

pub(super) struct PreparedTree {
    id: String,
    plan: TreePlan,
    model: TreeModel,
    names: TreeNames,
}

impl PreparedTree {
    pub(super) fn new(actor: &ActorPlan, channels: &[ContinuousChannelPlan]) -> Result<Self> {
        let plan = decode(actor, "tree", TreePlan::validate)?;
        let model = plan.model()?;
        strict_channels(&actor.id, channels, "tree", |property| {
            matches!(property, "opacity" | "x" | "y" | "scroll")
                || TreeChannel::parse(property).is_some()
        })?;
        for channel in channels.iter().filter(|c| c.actor_id == actor.id) {
            let Some((path, kind)) = TreeChannel::parse(&channel.property) else {
                continue;
            };
            let Some(node) = model.find(path) else {
                bail!(
                    "tree actor '{}' channel '{}' names a path that is not in the tree",
                    actor.id,
                    channel.property
                );
            };
            let node = &model.nodes[node];
            match kind {
                TreeChannel::Open if !node.foldable() => bail!(
                    "tree actor '{}' channel '{}' folds a path that is not a non-empty object or array",
                    actor.id,
                    channel.property
                ),
                TreeChannel::Value if node.variants() < 2 => bail!(
                    "tree actor '{}' channel '{}' has no later values at that path",
                    actor.id,
                    channel.property
                ),
                _ => {}
            }
        }
        let names = TreeNames::new(&plan, &model);
        Ok(Self {
            id: actor.id.clone(),
            plan,
            model,
            names,
        })
    }

    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &mut HeadlessRenderer,
        sample: impl Fn(&str, &str, f32) -> f32,
    ) {
        renderer.composite_tree(
            pixels,
            &self.plan,
            &self.model,
            &self.names,
            |property, d| sample(&self.id, property, d),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::plan_runtime::validate_renderer_plan;
    use psychopomp::{
        author::PlanBuilder,
        tree::{TreeActor, TreePlan},
    };
    use serde_json::json;

    #[test]
    fn preflight_checks_every_path_channel_without_a_gpu() {
        let build = |property: &str| {
            let mut scene = PlanBuilder::new("tree", 2_000_000_000);
            let mut tree = TreeActor::declare(
                &mut scene,
                "plan",
                TreePlan::new(
                    [100.0, 100.0],
                    800.0,
                    json!({"actors": [{"id": "a"}], "id": "demo"}),
                )
                .expanded(["$"]),
            )
            .unwrap();
            tree.channel(&mut scene, property, 0.0);
            scene.finish().unwrap()
        };
        for valid in [
            "opacity",
            "scroll",
            "node.$.actors.open",
            "node.$.actors[0].id.highlight",
        ] {
            validate_renderer_plan(&build(valid)).unwrap();
        }
        for invalid in [
            "typed",
            "node.$.missing.open",
            "node.$.id.open",
            "node.$.id.value",
            "node.$.actors.wobble",
        ] {
            assert!(
                validate_renderer_plan(&build(invalid)).is_err(),
                "{invalid}"
            );
        }
    }
}
