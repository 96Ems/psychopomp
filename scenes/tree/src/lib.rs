//! Tree showroom: the Scene Plan that `agent-demo` emits, opened one node at
//! a time. `actors` and its first title card open, then `continuousChannels`,
//! its first channel, and that channel's first spring (scrolling just enough
//! to keep the channel in view); its `property` lights up and its `initial`
//! value rolls; then everything folds and scrolls back.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, SECOND},
    caption::{CaptionActor, CaptionPlan, CaptionSpanPlan},
    plan::ScenePlan,
    tone::Tone,
    tree::{TreeActor, TreePlan},
};
use serde_json::{Value, json};

const MS: u64 = 1_000_000;

pub fn build_plan() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("tree", 12 * SECOND);
    // The real emitted plan, read back from its JSON text so numbers keep
    // their shortest form (`0.48000002`, not the f64 widening of an f32).
    let emitted: Value = serde_json::from_str(&serde_json::to_string(&agent_demo::build_plan())?)?;

    let mut caption = CaptionActor::declare(
        &mut scene,
        "caption",
        &CaptionPlan::line(
            [560.0, 96.0],
            26.0,
            vec![
                CaptionSpanPlan::new("agent-demo", Tone::Accent),
                CaptionSpanPlan::new(" emits this Scene Plan", Tone::Muted),
            ],
        ),
    )?;
    caption.show(&mut scene, 150 * MS);

    let mut tree = TreeActor::declare(
        &mut scene,
        "plan",
        TreePlan::new([560.0, 150.0], 900.0, emitted)
            .max_rows(22)
            .expanded(["$"]),
    )?;
    tree.show(&mut scene, 250 * MS);

    tree.open(&mut scene, "$.actors", 1200 * MS)?;
    tree.open(&mut scene, "$.actors[0]", 2100 * MS)?;
    tree.open(&mut scene, "$.continuousChannels", 3300 * MS)?;
    tree.open(&mut scene, "$.continuousChannels[0]", 4200 * MS)?;
    tree.open(&mut scene, "$.continuousChannels[0].events", 5100 * MS)?;
    // The first spring no longer fits: scroll just enough to show its channel.
    tree.open(&mut scene, "$.continuousChannels[0].events[0]", 5900 * MS)?;
    tree.reveal(&mut scene, "$.continuousChannels[0]", 5900 * MS)?;
    tree.highlight(
        &mut scene,
        "$.continuousChannels[0].property",
        6900 * MS,
        2.0,
    )?;
    tree.set(
        &mut scene,
        "$.continuousChannels[0].initial",
        json!(1.0),
        7500 * MS,
    )?;
    // Folding the channel lets the whole plan fit again: scroll back with it.
    tree.close(&mut scene, "$.continuousChannels[0]", 9300 * MS)?;
    tree.reveal(&mut scene, "$", 9300 * MS)?;
    tree.close(&mut scene, "$.actors", 9900 * MS)?;
    tree.close(&mut scene, "$.continuousChannels", 10700 * MS)?;

    scene.cue("tree", 0, 12 * SECOND);
    Ok(scene.finish()?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_showroom_plan_is_valid() {
        let plan = super::build_plan().unwrap();
        assert_eq!(plan.actors.len(), 2);
    }
}
