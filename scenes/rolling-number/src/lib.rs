//! Rolling Number showroom: a version chip rolling up then back down, a CI
//! counter redirected mid-roll, and a test count that carries into a new
//! place and later shrinks. Every change is one `roll` call at a time.
use anyhow::Result;
use kinograph::{
    author::{PlanBuilder, SECOND},
    caption::{CaptionAlign, CaptionSpanPlan},
    plan::ScenePlan,
    rolling::{RollingNumberActor, RollingNumberPlan},
    tone::Tone,
};

const MS: u64 = 1_000_000;

pub fn build_plan() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("rolling-number", 8 * SECOND);
    let muted = |text: &str| CaptionSpanPlan::new(text, Tone::Muted);

    let mut version = RollingNumberActor::declare(
        &mut scene,
        "version",
        RollingNumberPlan::new([960.0, 300.0], 72.0, "rc.112")
            .aligned(CaptionAlign::Center)
            .tone(Tone::Accent)
            .prefix(vec![muted("opencode ")])
            .chip(),
    )?;
    version.show(&mut scene, 200 * MS);
    version.roll(&mut scene, 1000 * MS, "rc.117")?;
    version.roll(&mut scene, 4500 * MS, "rc.113")?;

    let mut checks = RollingNumberActor::declare(
        &mut scene,
        "checks",
        RollingNumberPlan::new([960.0, 520.0], 72.0, "0/8")
            .aligned(CaptionAlign::Center)
            .tone(Tone::Success)
            .prefix(vec![muted("CI ")])
            .suffix(vec![muted(" checks")]),
    )?;
    checks.show(&mut scene, 350 * MS);
    checks.roll(&mut scene, 1800 * MS, "3/8")?;
    // Redirected while the first roll is still fast.
    checks.roll(&mut scene, 1950 * MS, "8/8")?;
    checks.roll(&mut scene, 5200 * MS, "7/8")?;

    let mut tests = RollingNumberActor::declare(
        &mut scene,
        "tests",
        RollingNumberPlan::new([960.0, 740.0], 72.0, "999")
            .aligned(CaptionAlign::Center)
            .suffix(vec![muted(" tests")]),
    )?;
    tests.show(&mut scene, 500 * MS);
    tests.roll(&mut scene, 2800 * MS, "1,000")?;
    tests.roll(&mut scene, 3800 * MS, "1,383")?;
    tests.roll(&mut scene, 6000 * MS, "998")?;

    scene.cue("rolls", 0, 8 * SECOND);
    Ok(scene.finish()?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_showroom_plan_is_valid() {
        let plan = super::build_plan().unwrap();
        assert_eq!(plan.actors.len(), 3);
    }
}
