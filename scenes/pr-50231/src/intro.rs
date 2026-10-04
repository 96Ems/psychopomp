//! The version jumps five release candidates; the compiler wires into every
//! renamed call and checks it off; then the three things it could not see
//! arrive beyond its reach.
use anyhow::{Context, Result};
use psychopomp::{
    author::{PlanBuilder, seconds},
    caption::CaptionAlign,
    narration::Narration,
    plan::ScenePlan,
    rolling::{RollingNumberActor, RollingNumberPlan},
    sfx,
    stage::{StageActor, StageElement, StagePlan, StagePost},
    tone::Tone,
};

use crate::{footer, header, span};

const COMPILER: [f32; 3] = [300.0, 716.0, 0.0];
const RENAMES: [&str; 4] = [
    "Flag.string → Flag.String",
    "Argument.string → Argument.String",
    "Config.redacted → Config.Redacted",
    "callback runner → socket.reader",
];
const UNCAUGHT: [&str; 3] = ["saved settings", "tool schemas", "permission order"];

fn stage_plan() -> StagePlan {
    let mut elements = vec![
        StageElement::card("compiler", COMPILER, [260.0, 110.0], "compiler")
            .statuses(&[("typecheck", Tone::Muted)])
            .tone(Tone::Request),
    ];
    for (index, title) in RENAMES.iter().enumerate() {
        elements.push(
            StageElement::card(
                &format!("rename-{index}"),
                [820.0, 560.0 + index as f32 * 104.0, 0.0],
                [620.0, 88.0],
                title,
            )
            .statuses(&[("renamed", Tone::Muted), ("caught", Tone::Success)]),
        );
        elements.push(
            StageElement::beam(
                &format!("check-{index}"),
                "compiler",
                &format!("rename-{index}"),
            )
            .tone(Tone::Request),
        );
    }
    for (index, title) in UNCAUGHT.iter().enumerate() {
        elements.push(
            StageElement::card(
                &format!("uncaught-{index}"),
                [1530.0, 612.0 + index as f32 * 104.0, 0.0],
                [400.0, 88.0],
                title,
            )
            .statuses(&[("runtime behavior", Tone::Warning)])
            .tone(Tone::Warning),
        );
    }
    StagePlan {
        post: StagePost::RESTRAINED,
        elements,
    }
}

pub fn build(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(seconds(0.5), [("intro", seconds(0.6))])?;
    let mut scene = PlanBuilder::new("intro", reading.duration());
    let [spoken] = reading.place(&mut scene);
    let w = |phrase: &str| spoken.at(phrase);
    let mut stage = StageActor::declare(&mut scene, "stage", &stage_plan())?;
    let s = &mut stage;
    let sc = &mut scene;

    s.channel(sc, "camera.z", -160.0);
    s.to(sc, "camera.z", 0, 0.0, 2.2);
    header(sc, "chore: upgrade Effect to rc.117", Some(seconds(0.25)))?;
    // The version rolls five release candidates in one jump.
    let mut version = RollingNumberActor::declare(
        sc,
        "version",
        RollingNumberPlan::new([960.0, 300.0], 64.0, "rc.112")
            .aligned(CaptionAlign::Center)
            .tone(Tone::Accent)
            .prefix(vec![
                span("effect ", Tone::Plain),
                span("4.0.0-", Tone::Muted),
            ])
            .duration_nanos(seconds(0.9)),
    )?;
    version.show(sc, seconds(0.3));
    let roll = w("five release candidates");
    version.roll(sc, roll, "rc.117")?;
    sfx::TICK.play(sc, "roll", roll + seconds(0.75), -20.0);

    // The compiler plugs into every renamed call and checks it off.
    let compiler = w("the compiler");
    s.settle_in(sc, "compiler", compiler.saturating_sub(seconds(0.2)));
    for index in 0..RENAMES.len() {
        s.settle_in(
            sc,
            &format!("rename-{index}"),
            compiler + seconds(0.1 * index as f64),
        );
    }
    let caught = w("caught every");
    for index in 0..RENAMES.len() {
        let card = format!("rename-{index}");
        let link = format!("check-{index}");
        let contact = s.connect(sc, &link, caught + seconds(0.12 * index as f64), 0.42);
        s.to(
            sc,
            &format!("{link}.flow"),
            contact + seconds(0.7),
            0.0,
            0.45,
        );
        s.to(sc, &format!("{card}.status"), contact, 1.0, 0.3);
        s.clock(sc, &format!("{card}.spinner"), contact);
        let drawn = s.resolve_spinner(sc, &card, contact, contact + seconds(0.25));
        sfx::TICK.play(sc, format!("check-{index}"), contact, -24.0);
        sfx::MARK.play(sc, format!("mark-{index}"), drawn, -23.0 - index as f32);
    }

    // What it couldn't catch: the checked work cools, the camera leans right.
    let hard = w("the hard part");
    s.to(sc, "camera.x", hard, 50.0, 1.6);
    s.to(sc, "camera.z", hard, 30.0, 1.6);
    for name in ["compiler"]
        .into_iter()
        .map(str::to_owned)
        .chain((0..RENAMES.len()).map(|index| format!("rename-{index}")))
    {
        s.to(sc, &format!("{name}.dim"), hard + seconds(0.2), 0.5, 0.8);
    }
    let everything = w("everything");
    for index in 0..UNCAUGHT.len() {
        s.settle_in(
            sc,
            &format!("uncaught-{index}"),
            everything + seconds(0.12 * index as f64),
        );
    }
    let catch = w("couldn't catch");
    for index in 0..UNCAUGHT.len() {
        s.hit(
            sc,
            &format!("uncaught-{index}.glow"),
            catch + seconds(0.1 * index as f64),
            0.7,
            0.2,
        );
    }
    let mut note = footer(
        sc,
        "footer",
        vec![
            span("the hard part: ", Tone::Plain),
            span("what the compiler couldn't catch", Tone::Warning),
        ],
    )?;
    note.type_in(sc, everything, 48.0, 0.6);
    scene.finish().context("intro")
}
