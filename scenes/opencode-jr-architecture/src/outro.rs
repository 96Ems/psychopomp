//! The whole shape in one line: the edge, one object per thread, the sandbox,
//! and back to Slack.
use anyhow::{Context, Result};
use psychopomp::{
    author::PlanTime,
    plan::ScenePlan,
    sfx,
    stage::{StageElement, StagePlan, StagePost, reply_after},
    tone::Tone,
};

use crate::{Film, Narration, arrive, begin, footer, header, orb_in, plug, seconds, send, show};

const Y: f32 = 500.0;

fn stage() -> StagePlan {
    let node = |id: &str, x: f32, title: &str, status: &str| {
        StageElement::card(id, [x, Y, 0.0], [250.0, 100.0], title)
            .statuses(&[(status, Tone::Muted)])
    };
    StagePlan {
        post: StagePost::RESTRAINED,
        elements: vec![
            node("slack", 240.0, "slack", "a thread"),
            node("worker", 600.0, "worker", "the edge"),
            StageElement::orb("session", [960.0, Y, 0.0], 88.0)
                .points(600)
                .tone(Tone::Plain),
            StageElement::label(
                "session-name",
                [960.0, Y + 125.0, 0.0],
                21.0,
                &[("one object per thread", Tone::Plain)],
            ),
            node("workspace", 1320.0, "WorkspaceDO", "its hands"),
            node("sandbox", 1680.0, "sandbox", "a real vm"),
            StageElement::beam("a", "slack", "worker").tone(Tone::Request),
            StageElement::beam("b", "worker", "session").tone(Tone::Request),
            StageElement::beam("c", "session", "workspace"),
            StageElement::beam("d", "workspace", "sandbox"),
            StageElement::packet("a-out", "a").tone(Tone::Request),
            StageElement::packet("b-out", "b").tone(Tone::Request),
            StageElement::packet("c-out", "c").tone(Tone::Request),
            StageElement::packet("d-out", "d").tone(Tone::Request),
            StageElement::packet("d-back", "d")
                .reversed()
                .tone(Tone::Success),
            StageElement::packet("c-back", "c")
                .reversed()
                .tone(Tone::Success),
            StageElement::packet("answer", "b")
                .reversed()
                .tone(Tone::Success),
            StageElement::packet("a-back", "a")
                .reversed()
                .labeled("answer")
                .tone(Tone::Success),
            StageElement::label(
                "next",
                [960.0, 780.0, 0.0],
                24.0,
                &[
                    ("next: ", Tone::Muted),
                    ("where it's heading", Tone::Accent),
                ],
            ),
        ],
    }
}

pub fn film(narration: &Narration) -> Result<ScenePlan> {
    let Film {
        sc: mut scene,
        s: mut actor,
        v,
    } = begin(narration, "outro", 0.9, 3.0, &stage())?;
    let (s, sc) = (&mut actor, &mut scene);
    header(sc, "→", "the shape")?;

    s.settle_in(sc, "slack", seconds(0.4));
    let edge = v.at("hits the edge");
    arrive(s, sc, "worker", "a", edge - seconds(0.5));
    let at_worker = send(s, sc, "a-out", edge + seconds(0.3), 0.6);
    s.land(sc, "worker", at_worker);
    let object = v.at("one durable object");
    orb_in(s, sc, "session", object - seconds(0.4));
    show(s, sc, "session-name", object + seconds(0.3));
    let contact = plug(s, sc, "b", object - seconds(0.1));
    let at_session = send(s, sc, "b-out", contact + seconds(0.3), 0.6);
    s.hit(sc, "session.pulse", at_session, 0.6, 0.0);
    let sandbox = v.at("in a sandbox");
    arrive(s, sc, "workspace", "c", sandbox - seconds(0.6));
    arrive(s, sc, "sandbox", "d", sandbox - seconds(0.4));
    let at_workspace = send(s, sc, "c-out", sandbox + seconds(0.3), 0.55);
    let at_sandbox = send(s, sc, "d-out", reply_after(at_workspace), 0.55);
    s.land(sc, "sandbox", at_sandbox);
    // The answer runs the chain back to Slack.
    let back = v.at("flows back").not_before(at_sandbox + seconds(0.2));
    let mut at = send(s, sc, "d-back", back, 0.5);
    for packet in ["c-back", "answer", "a-back"] {
        at = send(s, sc, packet, reply_after(at), 0.5);
    }
    s.hit(sc, "slack.flash", at, 0.7, 0.0);
    sfx::BLOOM.play(sc, "answered", at, -13.0);
    footer(
        sc,
        "footer",
        &[
            ("edge → ", Tone::Plain),
            ("one object per thread", Tone::Accent),
            (" → sandbox → slack", Tone::Plain),
        ],
        back,
    )?;
    let next = v.at("next time");
    s.type_in(sc, "next", next, 30.0);
    s.to(sc, "camera.z", next, 60.0, 2.4);
    scene.finish().context("outro")
}
