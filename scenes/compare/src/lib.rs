//! Compare showroom: one gateway and its four regions, before and after a
//! fix. A wipe sweeps the fixed frame in, rests mid-frame so the slow regions
//! sit beside the fast ones, then sweeps on; a plain downward wipe closes on
//! a title.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, seconds},
    plan::{ReelPlan, ReelSegmentPlan, ReelWipePlan, ScenePlan, WipeDirection},
    stage::{StageActor, StageElement, StagePlan, StagePost},
    tone::Tone,
};
use serde_json::json;

const REGIONS: [(&str, &str); 4] = [
    ("eu", "eu-west"),
    ("us", "us-east"),
    ("ap", "ap-south"),
    ("sa", "sa-east"),
];

/// Each half of the held wipe's sweep, and the hold between them.
const SWEEP: f64 = 0.9;
const HOLD: f64 = 2.6;

/// The same topology with every region reporting `latency` in `tone`.
fn regions(id: &str, latency: [&str; 4], tone: Tone, duration: u64) -> Result<ScenePlan> {
    let mut elements = vec![StageElement::orb("gateway", [960.0, 330.0, 0.0], 74.0)];
    for (index, (key, title)) in REGIONS.into_iter().enumerate() {
        let at = [360.0 + index as f32 * 400.0, 700.0, 0.0];
        elements.push(
            StageElement::card(key, at, [300.0, 132.0], title)
                .statuses(&[(&format!("p99 {}", latency[index]), tone)])
                .tone(tone),
        );
        elements.push(StageElement::beam(&format!("to-{key}"), "gateway", key).tone(tone));
    }
    let mut scene = PlanBuilder::new(id, duration);
    StageActor::declare(
        &mut scene,
        "stage",
        &StagePlan {
            post: StagePost::default(),
            elements,
        },
    )?;
    Ok(scene.finish()?)
}

pub fn build_reel() -> Result<ReelPlan> {
    let compare = seconds(SWEEP * 2.0 + HOLD);
    let mut title = PlanBuilder::new("closing", seconds(2.4));
    title.actor("title", "title-card", json!({ "title": "before / after" }))?;
    let reel = ReelPlan {
        version: ReelPlan::VERSION,
        id: "compare".to_owned(),
        segments: vec![
            ReelSegmentPlan {
                transition_nanos: 0,
                transition_style: Default::default(),
                transition_focus: None,
                transition_wipe: None,
                plan: regions(
                    "before",
                    ["840 ms", "910 ms", "1.2 s", "760 ms"],
                    Tone::Error,
                    seconds(1.4) + compare,
                )?,
            },
            ReelSegmentPlan::wiped(
                regions(
                    "after",
                    ["120 ms", "96 ms", "140 ms", "110 ms"],
                    Tone::Success,
                    compare + seconds(1.6),
                )?,
                compare,
                ReelWipePlan::new(WipeDirection::Left)
                    .hold(0.5, seconds(HOLD))
                    .labeled("BEFORE", "AFTER"),
            ),
            ReelSegmentPlan::wiped(
                title.finish()?,
                seconds(0.8),
                ReelWipePlan::new(WipeDirection::Down),
            ),
        ],
    };
    reel.validate()?;
    Ok(reel)
}

#[cfg(test)]
mod tests {
    use super::build_reel;

    #[test]
    fn the_held_wipe_shows_both_frames_mid_frame() {
        let reel = build_reel().unwrap();
        // The compare starts 1.4 s in; its hold begins after the first sweep.
        let held = reel.layers_at(1.4 + 0.9 + 1.3);
        assert_eq!(held.len(), 2);
        assert_eq!(held[1].wipe.unwrap().position, 0.5);
    }
}
