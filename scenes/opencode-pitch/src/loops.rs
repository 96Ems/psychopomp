//! Three loops fed by agents: replay real failures, stress transitions,
//! and check the shipped experience.
use anyhow::{Context, Result};
use psychopomp::{plan::ScenePlan, stage::StagePlan, tone::Tone};

use crate::{
    CONFIRM, Film, GLITCH, Narration, REST_Z, arrive, beam, begin, card, footer, glitch, header,
    orb, orb_in, packet, post, seconds, send, sound, status,
};

const LANES_X: f32 = 1240.0;

fn stage() -> StagePlan {
    let lane = |id: &str, y: f32, title: &str, states: &[(&str, Tone)]| {
        card(
            id,
            [LANES_X, y, 0.0],
            [380.0, 120.0],
            title,
            states,
            Tone::Success,
        )
    };
    StagePlan {
        post: post(),
        elements: vec![
            orb("agents", [640.0, 540.0, 0.0], 96.0, 700),
            lane(
                "replay",
                330.0,
                "replay failures",
                &[("from user reports", Tone::Muted)],
            ),
            lane(
                "stress",
                540.0,
                "stress transitions",
                &[
                    ("restart · retry · upgrade", Tone::Muted),
                    ("recovers", Tone::Success),
                ],
            ),
            lane(
                "experience",
                750.0,
                "check the experience",
                &[("the installed build", Tone::Muted)],
            ),
            beam("to-replay", "agents", "replay", 0.0, Tone::Request),
            beam("to-stress", "agents", "stress", 0.0, Tone::Request),
            beam("to-experience", "agents", "experience", 0.0, Tone::Request),
            packet("replay-go", "to-replay", false, Tone::Request),
            packet("stress-go", "to-stress", false, Tone::Request),
            packet("experience-go", "to-experience", false, Tone::Request),
        ],
    }
}

pub fn film(narration: &Narration) -> Result<ScenePlan> {
    let Film {
        sc: mut scene,
        s: mut actor,
        v,
    } = begin(narration, "loops", 0.7, 2.4, &stage())?;
    let (s, sc) = (&mut actor, &mut scene);
    header(sc, "03", "three loops")?;
    orb_in(s, sc, "agents", seconds(0.4));

    // Each loop lands as its own lane, fed by the same agents.
    let lane = |s: &mut psychopomp::stage::StageActor,
                sc: &mut psychopomp::author::PlanBuilder,
                card: &str,
                at: u64| {
        let contact = arrive(
            s,
            sc,
            card,
            &format!("to-{card}"),
            at.saturating_sub(seconds(0.4)),
        );
        let landed = send(s, sc, &format!("{card}-go"), contact + seconds(0.05), 0.5);
        s.land(sc, card, landed);
        landed
    };
    let replay = v.at("replay real failures");
    lane(s, sc, "replay", replay);
    s.to(sc, "camera.y", replay, -60.0, 1.6);

    let stress = v.at("stress the moments");
    lane(s, sc, "stress", stress);
    s.to(sc, "camera.y", stress, 0.0, 1.6);
    // Restarts and upgrades shake it; it recovers.
    let restarts = v.at("restarts");
    for (index, at) in [restarts, v.at("and upgrades")].into_iter().enumerate() {
        glitch(s, sc, "stress", at);
        s.hit(sc, "stress.alarm", at, 0.5, 0.0);
        s.kick(
            sc,
            ["stress.x", "stress.y"],
            at,
            [if index == 0 { 8.0 } else { -8.0 }, 0.0],
        );
        sc.media(sound(&format!("shake-{index}"), GLITCH, at, -18.0));
    }
    let recovers = v.at("and upgrades") + seconds(0.6);
    status(s, sc, "stress", recovers, 1);
    s.hit(sc, "stress.flash", recovers, 0.5, 0.0);
    sc.media(sound("recovers", CONFIRM, recovers, -18.0));

    let check = v.at("check the experience");
    lane(s, sc, "experience", check);
    s.to(sc, "camera.y", check, 60.0, 1.6);

    let end = v.at("people actually install");
    for (index, card) in ["replay", "stress", "experience"].into_iter().enumerate() {
        s.to(
            sc,
            &format!("{card}.glow"),
            end + seconds(index as f64 * 0.12),
            0.35,
            0.6,
        );
    }
    s.to(sc, "camera.y", end, 0.0, 1.8);
    s.to(sc, "camera.z", end, REST_Z - 30.0, 1.8);
    footer(
        sc,
        &[
            ("every loop leaves a ", Tone::Plain),
            ("test", Tone::Accent),
            (" behind", Tone::Plain),
        ],
        end,
    )?;
    scene.finish().context("loops")
}
