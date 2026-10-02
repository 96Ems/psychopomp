//! The whole shape in one line: the edge, one object per thread, the sandbox,
//! and back to Slack.
use anyhow::{Context, Result};
use psychopomp::{caption::CaptionAlign, plan::ScenePlan, stage::StagePlan, tone::Tone};

use crate::{
    BLOOM, Film, Narration, arrive, beam, begin, card, footer, header, label, orb, orb_in, packet,
    plug, post, seconds, send, show, sound,
};

const Y: f32 = 500.0;

fn stage() -> StagePlan {
    let node = |id: &str, x: f32, title: &str, status: &str| {
        card(
            id,
            [x, Y, 0.0],
            [250.0, 100.0],
            title,
            &[(status, Tone::Muted)],
            Tone::Plain,
        )
    };
    StagePlan {
        post: post(),
        elements: vec![
            node("slack", 240.0, "slack", "a thread"),
            node("worker", 600.0, "worker", "the edge"),
            orb("session", [960.0, Y, 0.0], 88.0, 600),
            label(
                "session-name",
                [960.0, Y + 125.0, 0.0],
                21.0,
                CaptionAlign::Center,
                &[("one object per thread", Tone::Plain)],
            ),
            node("workspace", 1320.0, "WorkspaceDO", "its hands"),
            node("sandbox", 1680.0, "sandbox", "a real vm"),
            beam("a", "slack", "worker", Tone::Request),
            beam("b", "worker", "session", Tone::Request),
            beam("c", "session", "workspace", Tone::Plain),
            beam("d", "workspace", "sandbox", Tone::Plain),
            packet("a-out", "a", false, "", Tone::Request),
            packet("b-out", "b", false, "", Tone::Request),
            packet("c-out", "c", false, "", Tone::Request),
            packet("d-out", "d", false, "", Tone::Request),
            packet("d-back", "d", true, "", Tone::Success),
            packet("c-back", "c", true, "", Tone::Success),
            packet("answer", "b", true, "", Tone::Success),
            packet("a-back", "a", true, "answer", Tone::Success),
            label(
                "next",
                [960.0, 780.0, 0.0],
                24.0,
                CaptionAlign::Center,
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
    let at_sandbox = send(s, sc, "d-out", at_workspace + seconds(0.42), 0.55);
    s.land(sc, "sandbox", at_sandbox);
    // The answer runs the chain back to Slack.
    let back = v.at("flows back").max(at_sandbox + seconds(0.2));
    let mut at = send(s, sc, "d-back", back, 0.5);
    for packet in ["c-back", "answer", "a-back"] {
        at = send(s, sc, packet, at + seconds(0.42), 0.5);
    }
    s.hit(sc, "slack.flash", at, 0.7, 0.0);
    sc.media(sound("answered", BLOOM, at, -13.0));
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
