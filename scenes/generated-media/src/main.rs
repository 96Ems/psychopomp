//! Generated media showroom. A whispered ElevenLabs line names the balls; a
//! Fish Audio chant spawns one, with a generated pop, on every "balls" it
//! says; the whisper comes back six semitones down. `media.lock.json` records
//! all four resources: the first run generates them, later runs call nothing,
//! and editing one line regenerates only that line and what derives from it.
use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use psychopomp::{
    author::{PlanBuilder, seconds},
    plan::ScenePlan,
    stage::{StageActor, StageElement, StagePlan, StagePost},
    tone::Tone,
};
use psychopomp_media::{Audio, Effect, FISH_KIT, Media, Voice};

/// Kit's ElevenLabs Professional Voice Clone.
const KIT: &str = "8olojUk4IXpvKgaOCHXj";
const BALLS: [(&str, f32); 3] = [("left", 560.0), ("middle", 960.0), ("right", 1360.0)];

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let media = Media::open(&root)?;
    let kit = Voice::eleven(KIT).v4().stability(0.2).similarity(0.65);
    let hush = media.say(
        "hush",
        &kit,
        "[extremely soft ASMR whisper] Oh... so I hear you like... balls.",
    )?;
    let pop = media.sfx(
        "pop",
        "A single soft glassy pop, tiny and dry. No voice, no music.",
        seconds(0.5),
    )?;
    let demon = hush.derive(Effect::pitch(-6.0))?;
    let chant = media.say(
        "chant",
        &Voice::fish(FISH_KIT),
        "[chanting, louder each time] Balls! Balls! BALLS!",
    )?;
    media.finish()?;

    let plan = film(&hush, &chant, &pop, &demon)?;
    let output = root.join("generated-media.plan.json");
    fs::write(&output, serde_json::to_string_pretty(&plan)? + "\n")?;
    eprintln!(
        "wrote {} ({:.1}s)",
        output.display(),
        plan.duration_nanos as f64 / 1e9
    );
    Ok(())
}

fn stage() -> StagePlan {
    StagePlan {
        post: StagePost {
            bloom: 0.18,
            grain: 0.012,
            vignette: 0.22,
            backdrop: 0.12,
        },
        elements: BALLS
            .iter()
            .map(|&(id, x)| StageElement::Orb {
                id: id.into(),
                at: [x, 540.0, 0.0],
                radius: 90.0,
                points: 500,
                tone: Tone::Plain,
            })
            .collect(),
    }
}

fn film(hush: &Audio, chant: &Audio, pop: &Audio, demon: &Audio) -> Result<ScenePlan> {
    let (lead, gap) = (seconds(0.6), seconds(0.5));
    let duration =
        lead + hush.duration() + gap + chant.duration() + gap + demon.duration() + seconds(1.2);
    let mut scene = PlanBuilder::new("generated-media", duration);
    let whispered = hush.place(&mut scene, lead);
    let chanted = chant.place(&mut scene, whispered.end() + gap);
    let echoed = demon.place(&mut scene, chanted.end() + gap);
    let mut stage = StageActor::declare(&mut scene, "stage", &stage())?;
    let (s, sc) = (&mut stage, &mut scene);
    for (ball, _) in BALLS {
        s.channel(sc, &format!("{ball}.opacity"), 0.0);
        s.channel(sc, &format!("{ball}.scale"), 0.4);
    }

    // The whisper names it, and the middle ball half appears.
    let named = whispered.at("balls");
    s.to(sc, "middle.opacity", named, 0.35, 0.8);
    s.to(sc, "middle.scale", named, 0.6, 1.2);

    // Every chanted "balls" spawns one, with a pop that grows each time.
    for (index, (at, (ball, _))) in chanted.words("balls").into_iter().zip(BALLS).enumerate() {
        s.set(sc, &format!("{ball}.opacity"), at, 1.0);
        s.bounce(sc, &format!("{ball}.scale"), at, 1.0, 0.5, 0.35);
        s.hit(sc, &format!("{ball}.pulse"), at, 0.8, 0.0);
        pop.play(sc, at, -16.0 + 4.0 * index as f32);
    }

    // The echo, six semitones down: every ball throbs on its "balls".
    let low = echoed.at("balls");
    for (ball, _) in BALLS {
        s.hit(sc, &format!("{ball}.pulse"), low, 1.2, 0.0);
    }
    s.jolt(sc, low, [0.0, 1.0], 0.6);
    scene.finish().context("generated-media")
}
