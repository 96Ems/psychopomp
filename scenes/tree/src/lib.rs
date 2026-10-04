//! Tree showroom: the Scene Plan that `agent-demo` emits, opened one node at
//! a time. `actors` and its first title card open, then `continuousChannels`,
//! its first channel, and that channel's first spring (scrolling just enough
//! to keep the channel in view); its `property` lights up and its `initial`
//! value rolls; then everything folds and scrolls back.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, SECOND, millis},
    caption::{CaptionActor, CaptionPlan, CaptionSpanPlan},
    plan::ScenePlan,
    tone::Tone,
    tree::{TreeActor, TreePlan},
};
use serde_json::{Value, json};

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
    caption.show(&mut scene, millis(150));

    let mut tree = TreeActor::declare(
        &mut scene,
        "plan",
        TreePlan::new([560.0, 150.0], 900.0, emitted)
            .max_rows(22)
            .expanded(["$"]),
    )?;
    tree.show(&mut scene, millis(250));

    tree.open(&mut scene, "$.actors", millis(1200))?;
    tree.open(&mut scene, "$.actors[0]", millis(2100))?;
    tree.open(&mut scene, "$.continuousChannels", millis(3300))?;
    tree.open(&mut scene, "$.continuousChannels[0]", millis(4200))?;
    tree.open(&mut scene, "$.continuousChannels[0].events", millis(5100))?;
    // The first spring no longer fits: scroll just enough to show its channel.
    tree.open(
        &mut scene,
        "$.continuousChannels[0].events[0]",
        millis(5900),
    )?;
    tree.reveal(&mut scene, "$.continuousChannels[0]", millis(5900))?;
    tree.highlight(
        &mut scene,
        "$.continuousChannels[0].property",
        millis(6900),
        2.0,
    )?;
    tree.set(
        &mut scene,
        "$.continuousChannels[0].initial",
        json!(1.0),
        millis(7500),
    )?;
    // Folding the channel lets the whole plan fit again: scroll back with it.
    tree.close(&mut scene, "$.continuousChannels[0]", millis(9300))?;
    tree.reveal(&mut scene, "$", millis(9300))?;
    tree.close(&mut scene, "$.actors", millis(9900))?;
    tree.close(&mut scene, "$.continuousChannels", millis(10700))?;

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
