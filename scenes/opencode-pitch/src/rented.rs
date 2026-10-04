//! The problem: an agent learns why a bug happens, ships a fix, and the
//! session ends. When the bug returns, the same insight is paid for again.
use anyhow::{Context, Result};
use psychopomp::{math::easing::Ease, plan::ScenePlan, stage::StagePlan, tone::Tone};

use crate::{
    DEATH, FAILURE, Film, IMPACT, Narration, REST_Z, SUCCESS, arrive, beam, begin, card, footer,
    glitch, header, hide, label, orb, orb_in, packet, plug, post, ring, seconds, send, show, sound,
    status,
};

const AGENT: [f32; 3] = [960.0, 560.0, 0.0];

fn stage() -> StagePlan {
    StagePlan {
        post: post(),
        elements: vec![
            card(
                "issue",
                [420.0, 560.0, 0.0],
                [300.0, 110.0],
                "bug report",
                &[
                    ("a user is blocked", Tone::Error),
                    ("fixed", Tone::Success),
                    ("broken again", Tone::Error),
                ],
                Tone::Plain,
            ),
            orb("agent", AGENT, 100.0, 760),
            ring("spend", AGENT, 148.0, 2.0, Tone::Accent),
            ring("spend-again", AGENT, 148.0, 2.0, Tone::Error),
            label(
                "agent-name",
                [AGENT[0], 740.0, 0.0],
                20.0,
                &[("agent session", Tone::Muted)],
            ),
            card(
                "fix",
                [1500.0, 560.0, 0.0],
                [280.0, 110.0],
                "fix",
                &[("shipped", Tone::Success)],
                Tone::Success,
            ),
            beam("ask", "issue", "agent", 0.0, Tone::Request),
            beam("ship", "agent", "fix", 0.0, Tone::Success),
            beam("again", "fix", "issue", -420.0, Tone::Error),
            packet("report", "ask", false, Tone::Request),
            packet("probe", "ask", true, Tone::Request),
            packet("learn", "ask", false, Tone::Request),
            packet("release", "ship", false, Tone::Success),
            packet("regression", "again", false, Tone::Error),
        ],
    }
}

pub fn film(narration: &Narration) -> Result<ScenePlan> {
    let Film {
        sc: mut scene,
        s: mut actor,
        v,
    } = begin(narration, "rented", 0.8, 2.4, &stage())?;
    let (s, sc) = (&mut actor, &mut scene);
    header(sc, "01", "the problem")?;

    // A user reports a bug: the report settles in, already hurting.
    let reports = v.at("reports a bug");
    s.settle_in(sc, "issue", reports.saturating_sub(seconds(0.3)));
    s.hit(sc, "issue.alarm", reports + seconds(0.4), 0.3, 0.0);

    // An agent gathers and investigates; the ring is what it spends.
    let agent = v.at("an agent");
    orb_in(s, sc, "agent", agent.saturating_sub(seconds(0.3)));
    show(s, sc, "agent-name", agent + seconds(0.3));
    let contact = plug(s, sc, "ask", agent);
    let mut arrived = send(s, sc, "report", contact + seconds(0.1), 0.6);
    s.hit(sc, "agent.pulse", arrived, 0.5, 0.0);
    arrived = send(s, sc, "probe", arrived + seconds(0.3), 0.55);
    s.land(sc, "issue", arrived);
    arrived = send(s, sc, "learn", arrived + seconds(0.3), 0.55);
    s.hit(sc, "agent.pulse", arrived, 0.7, 0.0);
    let spends = v.at("spends real time");
    show(s, sc, "spend", spends);
    s.ease(sc, "spend.sweep", spends, 1.0, 3.4, Ease::Smootherstep);
    s.to(sc, "camera.x", spends, -40.0, 1.8);

    // It ships a fix.
    let ships = v.at("ships a fix");
    arrive(s, sc, "fix", "ship", ships.saturating_sub(seconds(0.7)));
    let shipped = send(s, sc, "release", ships + seconds(0.1), 0.6);
    s.land(sc, "fix", shipped);
    sc.media(sound("shipped", SUCCESS, shipped, -16.0));
    status(s, sc, "issue", shipped + seconds(0.2), 1);
    s.to(sc, "camera.x", ships, 60.0, 1.6);

    // The session ends: what it learned falls apart.
    let ends = v.at("session ends");
    s.ease(
        sc,
        "agent.shatter",
        ends,
        1.0,
        1.8,
        Ease::CubicBezier([0.55, 0.0, 0.9, 0.6]),
    );
    s.to(sc, "agent.opacity", ends + seconds(0.5), 0.0, 1.2);
    hide(s, sc, "spend", ends);
    hide(s, sc, "agent-name", ends);
    s.to(sc, "ask.break", ends + seconds(0.1), 1.0, 0.8);
    s.to(sc, "ship.break", ends + seconds(0.2), 1.0, 0.8);
    sc.media(sound("ends", DEATH, ends, -18.0));
    s.to(sc, "camera.x", ends, 0.0, 1.8);

    // The bug comes back, and the user is blocked again.
    let back = v.at("comes back");
    let contact = plug(s, sc, "again", back.saturating_sub(seconds(0.7)));
    let hit = send(s, sc, "regression", contact + seconds(0.1), 0.85);
    s.hit(sc, "issue.alarm", hit, 0.7, 0.25);
    glitch(s, sc, "issue", hit);
    status(s, sc, "issue", hit + seconds(0.1), 2);
    s.jolt(sc, hit, [-1.0, 0.0], 0.35);
    sc.media(sound("hit", IMPACT, hit, -12.0));
    sc.media(sound("again", FAILURE, hit + seconds(0.05), -14.0));
    s.to(sc, "fix.dim", hit, 0.5, 0.8);

    // And the same insight is bought again.
    let again = v.at("all over again");
    s.ease(
        sc,
        "agent.shatter",
        again - seconds(0.6),
        0.0,
        1.1,
        Ease::Smootherstep,
    );
    s.to(sc, "agent.opacity", again - seconds(0.6), 1.0, 0.6);
    s.to(sc, "ask.break", again - seconds(0.4), 0.0, 0.8);
    show(s, sc, "spend-again", again);
    s.ease(sc, "spend-again.sweep", again, 1.0, 2.2, Ease::Smootherstep);
    footer(
        sc,
        &[
            ("the insight was ", Tone::Plain),
            ("rented", Tone::Accent),
            (", not kept", Tone::Plain),
        ],
        v.at("we pay"),
    )?;
    s.to(sc, "camera.z", again, REST_Z + 50.0, 2.2);
    scene.finish().context("rented")
}
