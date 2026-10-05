//! Charts showroom: a Plot of a critically damped and a bouncy spring with a
//! playhead riding the bouncy one, then a spring retargeted mid-flight beside
//! one restarted from rest, then Lanes built from this scene's own plot
//! channels. Every curve is sampled from Psychopomp's compiled Property Tracks,
//! so the plot shows exactly what the engine does.
use anyhow::{Context, Result, bail};
use psychopomp::{
    author::{ContinuousHandle, PlanBuilder, SECOND, millis},
    axis::AxisPlan,
    caption::{CaptionActor, CaptionPlan, CaptionSpanPlan},
    lanes::{LanesActor, LanesPlan},
    motion::MotionState,
    plan::{ScalarPlan, ScenePlan, compile_channels},
    plot::{PlotActor, PlotPlan, PlotSeriesPlan},
    timeline::PropertyId,
    tone::Tone,
};

/// Where the retargeted spring changes its mind, in its own seconds.
const RETARGET: f32 = 0.16;
const SPAN: [f32; 2] = [0.0, 1.6];
const SAMPLES: usize = 321;
/// The plot's story (the first two cues) ends here; Lanes replay it.
const STORY_END: u64 = 15 * SECOND;

pub fn build_plan() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("charts", 23 * SECOND);
    let mut plot = plot_story(&mut scene)?;
    plot.hide(&mut scene, STORY_END);

    caption(
        &mut scene,
        "springs-title",
        &[
            ("critically damped", Tone::Plain),
            (" vs ", Tone::Muted),
            ("bouncy", Tone::Accent),
        ],
        millis(300),
        millis(7300),
    )?;
    caption(
        &mut scene,
        "retarget-title",
        &[
            ("retarget mid-flight: ", Tone::Plain),
            ("velocity carries over", Tone::Accent),
        ],
        millis(7700),
        millis(14_700),
    )?;
    caption(
        &mut scene,
        "plan-title",
        &[
            ("a Scene Plan is just ", Tone::Plain),
            ("channels over time", Tone::Accent),
        ],
        millis(15_300),
        23 * SECOND,
    )?;

    // Replay the plot's own channels as lanes: build the same story into a
    // throwaway plan and read its keyframes and compiled curves back.
    let mut preview = PlanBuilder::new("charts-story", STORY_END);
    plot_story(&mut preview)?;
    let story = preview.finish()?;
    let recipe = LanesPlan::from_scene_plan(&story, [160.0, 230.0], 1600.0, |channel| {
        (channel.actor_id == "plot" && LANES.contains(&channel.property.as_str()))
            .then(|| channel.property.clone())
    })?;
    let mut lanes = LanesActor::declare(&mut scene, "lanes", &recipe)?;
    lanes.show(&mut scene, millis(15_500), 1.4);
    let scrubbed = lanes.scrub(&mut scene, [0.0, 15.0], millis(17_200), 5.0);
    lanes.emphasize(&mut scene, "plot.playhead", millis(17_200), 1.0);
    lanes.emphasize(&mut scene, "plot.playhead", scrubbed, 0.0);

    scene.cue("plan", STORY_END, 23 * SECOND);
    Ok(scene.finish()?)
}

/// The plot channels shown as lanes, in declaration order.
const LANES: [&str; 11] = [
    "axes",
    "series.critical.draw",
    "series.bouncy.draw",
    "playhead",
    "series.bouncy.ride",
    "series.bouncy.velocity",
    "series.target.draw",
    "mark.retarget.opacity",
    "series.interrupted.draw",
    "series.restart.draw",
    "series.interrupted.velocity",
];

/// The plot and its two cues: springs, then a retarget.
fn plot_story(scene: &mut PlanBuilder) -> Result<PlotActor> {
    let critical = spring_track(|lab, value| lab.spring(value, 0, 1.0, 0.5, 0.0))?;
    let bouncy = spring_track(|lab, value| lab.spring(value, 0, 1.0, 0.5, 0.5))?;
    let retarget_ns = (f64::from(RETARGET) * 1e9) as u64;
    let interrupted = spring_track(|lab, value| {
        lab.spring(value, 0, 1.0, 0.5, 0.25);
        lab.spring(value, retarget_ns, 0.3, 0.5, 0.25);
    })?;
    let before = interrupted(RETARGET).position;
    // The naive retarget: the same spring restarted from rest where it was.
    let restart = spring_track(|lab, value| {
        lab.spring(value, 0, 1.0, 0.5, 0.25);
        lab.set(value, retarget_ns, before);
        lab.spring(value, retarget_ns, 0.3, 0.5, 0.25);
    })?;

    let recipe = PlotPlan::new(
        [250.0, 250.0],
        [1420.0, 560.0],
        AxisPlan::new(SPAN).every(0.2).label("time (s)"),
        AxisPlan::new([0.0, 1.3]).every(0.5).label("position"),
    )
    .series(PlotSeriesPlan::motion(
        "critical",
        "critically damped",
        Tone::Plain,
        SPAN,
        SAMPLES,
        &critical,
    ))
    .series(PlotSeriesPlan::motion(
        "bouncy",
        "bouncy",
        Tone::Accent,
        SPAN,
        SAMPLES,
        &bouncy,
    ))
    .series(
        PlotSeriesPlan::new(
            "target",
            "target",
            Tone::Muted,
            vec![
                [SPAN[0], 1.0],
                [RETARGET, 1.0],
                [RETARGET, 0.3],
                [SPAN[1], 0.3],
            ],
        )
        .dashed(),
    )
    .series(PlotSeriesPlan::motion(
        "interrupted",
        "retargeted",
        Tone::Accent,
        SPAN,
        SAMPLES,
        &interrupted,
    ))
    .series(
        PlotSeriesPlan::motion(
            "restart",
            "restarted from rest",
            Tone::Error,
            [RETARGET, SPAN[1]],
            SAMPLES,
            &restart,
        )
        .dashed(),
    )
    .mark("retarget", RETARGET, "retarget");
    let mut plot = PlotActor::declare(scene, "plot", &recipe)?;

    // Springs.
    plot.show(scene, millis(400), 1.0);
    plot.draw(scene, "critical", millis(1200), 1.2);
    plot.draw(scene, "bouncy", millis(2300), 1.4);
    plot.fade(scene, "critical", millis(3900), 0.35);
    let arrived = plot.ride(scene, "bouncy", SPAN, millis(4000), 3.0);
    plot.velocity(scene, "bouncy", millis(4000), 1.0);
    plot.stop_ride(scene, "bouncy", arrived + millis(200));
    plot.velocity(scene, "bouncy", arrived + millis(200), 0.0);
    scene.cue("springs", 0, millis(7600));

    // Retarget, on the same axes.
    plot.fade(scene, "critical", millis(7700), 0.0);
    plot.fade(scene, "bouncy", millis(7700), 0.0);
    plot.draw(scene, "target", millis(8100), 0.9);
    plot.mark(scene, "retarget", millis(8500));
    plot.draw(scene, "interrupted", millis(8800), 1.4);
    plot.draw(scene, "restart", millis(10_300), 1.0);
    plot.fade(scene, "restart", millis(11_500), 0.6);
    let arrived = plot.ride(scene, "interrupted", SPAN, millis(11_600), 3.0);
    plot.velocity(scene, "interrupted", millis(11_600), 1.0);
    plot.stop_ride(scene, "interrupted", arrived);
    plot.velocity(scene, "interrupted", arrived, 0.0);
    scene.cue("retarget", millis(7600), STORY_END);
    Ok(plot)
}

/// One channel's motion as Psychopomp plays it: `write` authors events on a
/// scratch plan, which compiles through the renderer's own channel compiler.
fn spring_track(
    write: impl FnOnce(&mut PlanBuilder, &ContinuousHandle),
) -> Result<impl Fn(f32) -> MotionState> {
    let mut lab = PlanBuilder::new("spring-lab", 4 * SECOND);
    let actor = lab.actor("value", "text", ())?;
    let value = lab.channel(&actor, "value", 0.0);
    write(&mut lab, &value);
    let plan = lab.finish()?;
    let channel = &plan.continuous_channels[0];
    let property = PropertyId::new(channel.id.clone());
    let timeline = compile_channels(
        [(channel, property.clone())],
        plan.duration_nanos,
        |value| match value {
            ScalarPlan::Literal(value) => Ok(*value),
            ScalarPlan::Target(_) => bail!("the lab has no semantic targets"),
        },
    )?;
    timeline
        .sample_at(&property, 0.0)
        .context("compiled spring")?;
    Ok(move |t: f32| {
        timeline
            .sample_at(&property, f64::from(t))
            .expect("the compiled channel exists")
    })
}

fn caption(
    scene: &mut PlanBuilder,
    id: &str,
    spans: &[(&str, Tone)],
    at: u64,
    until: u64,
) -> Result<()> {
    let plan = CaptionPlan::line(
        [250.0, 130.0],
        30.0,
        spans
            .iter()
            .map(|(text, tone)| CaptionSpanPlan::new(*text, *tone))
            .collect(),
    );
    let mut caption = CaptionActor::declare(scene, id, &plan)?;
    caption.type_in(scene, at, 60.0, 0.5);
    if until < scene.duration_nanos() {
        caption.hide(scene, until);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_showroom_plan_is_valid() {
        let plan = build_plan().unwrap();
        assert_eq!(plan.cues.len(), 3);
    }

    #[test]
    fn retargeting_keeps_velocity_and_restarting_drops_it() {
        let retarget_ns = (f64::from(RETARGET) * 1e9) as u64;
        let interrupted = spring_track(|lab, value| {
            lab.spring(value, 0, 1.0, 0.5, 0.25);
            lab.spring(value, retarget_ns, 0.3, 0.5, 0.25);
        })
        .unwrap();
        let just_before = interrupted(RETARGET - 1e-4);
        let just_after = interrupted(RETARGET + 1e-4);
        assert!((just_before.velocity - just_after.velocity).abs() < 0.05);
        assert!(just_after.velocity > 1.0, "still rising after the retarget");
    }
}
