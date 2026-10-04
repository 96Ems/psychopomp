//! Eight CI checks spin and resolve into marks while the count rolls to 8/8;
//! then the title card and the promise that nothing you depend on changes.
use anyhow::{Context, Result};
use psychopomp::{
    author::{PlanBuilder, seconds},
    caption::CaptionAlign,
    effects::spinner,
    math::easing::Ease,
    narration::Narration,
    plan::ScenePlan,
    rolling::{RollingNumberActor, RollingNumberPlan},
    stage::{StageActor, StageElement, StagePlan, StagePost},
    tone::Tone,
};

use crate::{MARK, RESOLUTION, sound, span};

const CHECKS: [&str; 8] = [
    "typecheck",
    "unit (linux)",
    "unit (windows)",
    "e2e (linux)",
    "e2e (windows)",
    "affected packages",
    "check-standards",
    "check-compliance",
];
fn stage_plan() -> StagePlan {
    let mut elements = vec![
        StageElement::label(
            "title",
            [960.0, 740.0, 0.0],
            46.0,
            &[
                ("#50231", Tone::Accent),
                (" · chore: upgrade Effect to rc.117", Tone::Plain),
            ],
        ),
        StageElement::label(
            "tagline",
            [960.0, 822.0, 0.0],
            32.0,
            &[("nothing you depend on changes.", Tone::Plain)],
        ),
    ];
    for (index, name) in CHECKS.iter().enumerate() {
        elements.push(
            StageElement::card(
                &format!("check-{index}"),
                [
                    345.0 + (index % 4) as f32 * 420.0,
                    460.0 + (index / 4) as f32 * 112.0,
                    0.0,
                ],
                [380.0, 88.0],
                name,
            )
            .statuses(&[("running", Tone::Muted), ("passed", Tone::Success)]),
        );
    }
    StagePlan {
        post: StagePost::RESTRAINED,
        elements,
    }
}

pub fn build(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(seconds(0.5), [("outro", seconds(1.6))])?;
    let mut scene = PlanBuilder::new("outro", reading.duration());
    let [spoken] = reading.place(&mut scene);
    let w = |phrase: &str| spoken.at(phrase);
    let mut stage = StageActor::declare(&mut scene, "stage", &stage_plan())?;
    let s = &mut stage;
    let sc = &mut scene;

    s.channel(sc, "camera.z", -120.0);
    s.to(sc, "camera.z", 0, 0.0, 2.0);
    // The count advances as each check's mark is drawn.
    let mut count = RollingNumberActor::declare(
        sc,
        "count",
        RollingNumberPlan::new([960.0, 300.0], 64.0, "0/8")
            .aligned(CaptionAlign::Center)
            .tone(Tone::Success)
            .suffix(vec![span(" checks pass", Tone::Plain)]),
    )?;
    count.show(sc, seconds(0.2));

    // Every check starts spinning, then resolves into its mark in turn.
    let start = spoken.start();
    let pass = w("pass");
    let mut drawn_at = Vec::new();
    for index in 0..CHECKS.len() {
        let card = format!("check-{index}");
        let settle = start + seconds(0.07 * index as f64);
        s.settle_in(sc, &card, settle);
        let spin = settle + seconds(0.2);
        s.clock(sc, &format!("{card}.spinner"), spin);
        let target = pass + seconds(0.17 * index as f64);
        let waited = target.saturating_sub(spin) as f32 / 1e9;
        let mark = spin + seconds(f64::from(spinner::handoff(waited)));
        s.clock(sc, &format!("{card}.mark"), mark);
        let drawn = mark + seconds(f64::from(spinner::DRAW));
        s.to(sc, &format!("{card}.status"), drawn, 1.0, 0.3);
        sc.media(sound(&format!("mark-{index}"), MARK, drawn, -25.0));
        drawn_at.push(drawn);
    }
    drawn_at.sort_unstable();
    for (passed, at) in drawn_at.into_iter().enumerate() {
        count.roll(sc, at, format!("{}/8", passed + 1))?;
    }

    // The title card.
    let title = w("that's pull request");
    for index in 0..CHECKS.len() {
        s.to(
            sc,
            &format!("check-{index}.dim"),
            title + seconds(0.03 * index as f64),
            0.45,
            0.8,
        );
    }
    s.to(sc, "camera.y", title, 40.0, 1.6);
    s.type_in(sc, "title", title + seconds(0.2), 60.0);
    let nothing = w("nothing you depend");
    let done = s.type_in(sc, "tagline", nothing, 40.0);
    sc.media(sound("resolve", RESOLUTION, done, -20.0));

    // Fade out together.
    let out = reading.duration() - seconds(0.75);
    count.hide(sc, out);
    let names = ["title", "tagline"]
        .into_iter()
        .map(str::to_owned)
        .chain((0..CHECKS.len()).map(|index| format!("check-{index}")));
    for name in names {
        s.ease(
            sc,
            &format!("{name}.opacity"),
            out,
            0.0,
            0.6,
            Ease::Smootherstep,
        );
    }
    scene.finish().context("outro")
}
