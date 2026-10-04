//! The loop: the report becomes a failing test, the repair makes the same
//! test pass, and the test is kept. A later regression strikes the test.
use anyhow::{Context, Result};
use psychopomp::{
    effects::spinner, math::easing::Ease, plan::ScenePlan, stage::StagePlan, tone::Tone,
};

use crate::{
    FAILURE, Film, IMPACT, MARK, Narration, REST_Z, SUCCESS, arrive, beam, begin, card, footer,
    header, hide, label, orb, orb_in, packet, plug, post, seconds, send, sound, status,
};

const PROOF: [f32; 3] = [1240.0, 580.0, 0.0];

fn stage() -> StagePlan {
    StagePlan {
        post: post(),
        elements: vec![
            card(
                "issue",
                [320.0, 580.0, 0.0],
                [260.0, 110.0],
                "bug report",
                &[("a user is blocked", Tone::Error)],
                Tone::Plain,
            ),
            orb("agent", [770.0, 580.0, 0.0], 88.0, 680),
            card(
                "proof",
                PROOF,
                [330.0, 120.0],
                "regression test",
                &[
                    ("fails · right reason", Tone::Error),
                    ("passes", Tone::Success),
                    ("caught it", Tone::Warning),
                ],
                Tone::Success,
            ),
            label(
                "kept",
                [PROOF[0], 700.0, 0.0],
                20.0,
                &[("kept in the repo", Tone::Muted)],
            ),
            card(
                "users",
                [1690.0, 580.0, 0.0],
                [230.0, 110.0],
                "users",
                &[("still working", Tone::Muted)],
                Tone::Success,
            ),
            card(
                "change",
                [PROOF[0], 270.0, 0.0],
                [300.0, 100.0],
                "a later change",
                &[("brings the bug back", Tone::Error)],
                Tone::Plain,
            ),
            beam("ask", "issue", "agent", 0.0, Tone::Request),
            beam("write", "agent", "proof", 0.0, Tone::Plain),
            beam("guard", "proof", "users", 0.0, Tone::Success),
            beam("again", "change", "proof", 0.0, Tone::Error),
            packet("report", "ask", false, Tone::Request),
            packet("spec", "write", false, Tone::Request),
            packet("repair", "write", false, Tone::Success),
            packet("regression", "again", false, Tone::Error),
        ],
    }
}

pub fn film(narration: &Narration) -> Result<ScenePlan> {
    let Film {
        sc: mut scene,
        s: mut actor,
        v,
    } = begin(narration, "kept", 0.8, 2.4, &stage())?;
    let (s, sc) = (&mut actor, &mut scene);
    header(sc, "02", "the loop")?;

    // The same report, the same agent: this time it leaves something behind.
    for wire in ["ask", "write"] {
        s.channel(sc, &format!("{wire}.opacity"), 1.0);
    }
    s.settle_in(sc, "issue", seconds(0.3));
    orb_in(s, sc, "agent", seconds(0.5));
    let contact = plug(s, sc, "ask", seconds(0.9));
    let read = send(s, sc, "report", contact + seconds(0.2), 0.55);
    s.hit(sc, "agent.pulse", read, 0.5, 0.0);
    let behind = v.at("leave something behind");
    s.hit(sc, "agent.pulse", behind, 0.75, 0.0);

    // The report becomes a test that fails for the right reason.
    let test = v.at("into a test");
    arrive(s, sc, "proof", "write", test.saturating_sub(seconds(0.5)));
    let written = send(s, sc, "spec", test + seconds(0.2), 0.55);
    let fails = v.at("that fails").max(written);
    s.channel(sc, "proof.alarm", 0.0);
    s.hit(sc, "proof.alarm", fails, 0.45, 0.12);
    sc.media(sound("fails", FAILURE, fails, -16.0));
    s.to(sc, "camera.x", test, 40.0, 1.8);

    // The repair runs until the same test passes.
    let fix = v.at("fix the code");
    s.clock(sc, "proof.spinner", fix);
    s.hit(sc, "agent.pulse", fix, 0.6, 0.0);
    let repaired = send(s, sc, "repair", fix + seconds(0.3), 0.6);
    let passes = v.at("test passes");
    s.to(sc, "proof.alarm", repaired, 0.0, 0.5);
    status(s, sc, "proof", passes, 1);
    let waited = passes.saturating_sub(fix) as f32 / 1e9;
    let handoff = fix + seconds(f64::from(spinner::handoff(waited)));
    s.clock(sc, "proof.mark", handoff);
    s.hit(sc, "proof.flash", passes, 0.6, 0.0);
    s.to(sc, "proof.glow", passes, 0.45, 0.6);
    sc.media(sound(
        "mark",
        MARK,
        handoff + seconds(f64::from(spinner::DRAW)),
        -18.0,
    ));
    sc.media(sound("passes", SUCCESS, passes, -16.0));

    // Keep it: the session can end; the test stays and guards users.
    let keep = v.at("keep it");
    s.to(sc, "agent.opacity", keep, 0.0, 0.8);
    s.to(sc, "agent.scale", keep, 0.82, 0.9);
    for id in ["ask", "write", "issue"] {
        s.to(sc, &format!("{id}.opacity"), keep, 0.0, 0.7);
    }
    s.type_in(sc, "kept", keep + seconds(0.2), 36.0);
    arrive(s, sc, "users", "guard", keep.saturating_sub(seconds(0.3)));
    s.to(sc, "camera.x", keep, 360.0, 1.8);
    s.to(sc, "camera.z", keep, REST_Z + 50.0, 1.8);

    // The bug comes back, and strikes the test instead of the user.
    let back = v.at("comes back");
    arrive(s, sc, "change", "again", back.saturating_sub(seconds(0.6)));
    let hit = send(s, sc, "regression", back + seconds(0.25), 0.7);
    let strikes = v.at("hits the test").max(hit);
    s.hit(sc, "proof.alarm", hit, 0.8, 0.0);
    status(s, sc, "proof", hit + seconds(0.05), 2);
    s.kick(sc, ["proof.x", "proof.y"], hit, [0.0, 7.0]);
    s.twang(sc, "again", hit);
    s.jolt(sc, hit, [0.0, 1.0], 0.3);
    sc.media(sound("caught", IMPACT, hit, -12.0));
    hide(s, sc, "kept", strikes);
    let user = v.at("not the user");
    s.hit(sc, "users.flash", user, 0.35, 0.0);
    s.to(sc, "users.glow", user, 0.3, 0.6);
    s.ease(sc, "camera.z", user, REST_Z + 30.0, 1.6, Ease::Smootherstep);
    footer(
        sc,
        &[
            ("the bug hits the ", Tone::Plain),
            ("test", Tone::Accent),
            (", not the user", Tone::Plain),
        ],
        strikes,
    )?;
    scene.finish().context("kept")
}
