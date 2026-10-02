//! Video showroom: a screen recording flies in as a titled card, then the
//! card zooms into the OpenCode prompt and back out while the footage plays.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, SECOND, seconds},
    caption::{CaptionActor, CaptionAlign, CaptionPlan, CaptionSpanPlan},
    plan::ScenePlan,
    tone::Tone,
    video::{self, VideoActor, VideoPlan},
};

const DURATION: u64 = 9 * SECOND;
/// The OpenCode prompt and its hints, in source pixels.
const PROMPT: [f32; 4] = [920.0, 225.0, 900.0, 356.0];

pub fn build_plan() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("video", DURATION);
    let mut card = VideoActor::declare(
        &mut scene,
        "recording",
        VideoPlan::new("session", [1920, 760], 60)
            .at([960.0, 520.0], 1520.0)
            .titled("vim  /  opencode v2"),
        // Relative to target/, where the plan is written.
        video::media(
            "session",
            "../assets/opencode-v2-session-tool/max-hot-reload-split.mp4",
            (seconds(1.5), seconds(1.5) + DURATION),
            0,
        ),
    )?;
    card.fly_in(&mut scene, seconds(0.25));
    card.focus(&mut scene, seconds(3.0), PROMPT, 0.9);
    card.unfocus(&mut scene, seconds(6.6), 0.9);

    let mut caption = CaptionActor::declare(
        &mut scene,
        "caption",
        &CaptionPlan::line(
            [960.0, 1000.0],
            24.0,
            vec![
                CaptionSpanPlan::new("focus ", Tone::Accent),
                CaptionSpanPlan::new("the live prompt, mid-recording", Tone::Muted),
            ],
        )
        .aligned(CaptionAlign::Center),
    )?;
    caption.show(&mut scene, seconds(3.2));
    caption.hide(&mut scene, seconds(6.4));

    scene.cue("enter", 0, seconds(3.0));
    scene.cue("focus", seconds(3.0), seconds(6.6));
    scene.cue("release", seconds(6.6), DURATION);
    Ok(scene.finish()?)
}

#[cfg(test)]
mod tests {
    use super::build_plan;

    #[test]
    fn the_showroom_plays_one_placement_for_its_whole_duration() {
        let plan = build_plan().unwrap();
        assert_eq!(plan.media.len(), 1);
        assert_eq!(plan.media[0].timeline_end_nanos, plan.duration_nanos);
        assert_eq!(plan.cues.len(), 3);
    }
}
