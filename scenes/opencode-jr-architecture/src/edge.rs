//! The edge: Slack's event passes the Worker's checks, lands in the thread's
//! mailbox, and only then does Slack get its 200. A redelivery dedupes.
use anyhow::{Context, Result};
use psychopomp::{
    author::PlanTime,
    caption::CaptionAlign,
    math::{Vec2, Vec3, vec2},
    plan::ScenePlan,
    sfx,
    stage::{Camera, StageElement, StagePlan, StagePost, reply_after},
    tone::Tone,
};

use crate::{Film, Narration, arrive, begin, chip, footer, header, seconds, send, status};

const SLACK: [f32; 3] = [330.0, 440.0, 0.0];
const WORKER: [f32; 3] = [960.0, 440.0, 0.0];
const WORKER_SIZE: [f32; 2] = [320.0, 124.0];
const SESSION: [f32; 3] = [1590.0, 440.0, 0.0];
/// Where the camera rests before the reel flies into the Worker's code.
const CLOSING_CAMERA: [f32; 3] = [0.0, 0.0, 260.0];
const CHECKS_X: f32 = 830.0;

fn stage() -> StagePlan {
    let check = |id: &str, y: f32, mark: (&str, Tone), text: &str| {
        StageElement::label(id, [CHECKS_X, y, 0.0], 21.0, &[mark, (text, Tone::Muted)])
            .align(CaptionAlign::Left)
    };
    StagePlan {
        post: StagePost::RESTRAINED,
        elements: vec![
            StageElement::card("slack", SLACK, [280.0, 120.0], "slack").statuses(&[
                ("events api", Tone::Muted),
                ("waiting for 200", Tone::Plain),
                ("200 · done", Tone::Success),
                ("retrying", Tone::Warning),
            ]),
            StageElement::card("worker", WORKER, WORKER_SIZE, "worker")
                .statuses(&[
                    ("POST /slack/events", Tone::Muted),
                    ("checking", Tone::Plain),
                    ("enqueueing", Tone::Plain),
                    ("200 sent", Tone::Success),
                ])
                .tone(Tone::Request),
            StageElement::card("session", SESSION, [320.0, 124.0], "SessionDO").statuses(&[
                ("mailbox · empty", Tone::Muted),
                ("mailbox · 1 event", Tone::Plain),
                ("same ts · deduped", Tone::Warning),
            ]),
            StageElement::beam("in", "slack", "worker").tone(Tone::Request),
            StageElement::beam("enq", "worker", "session"),
            check(
                "signature",
                570.0,
                ("✓ ", Tone::Success),
                "signature verified",
            ),
            check("envelope", 610.0, ("✓ ", Tone::Success), "envelope decoded"),
            check(
                "team",
                650.0,
                ("✕ ", Tone::Error),
                "other workspaces dropped",
            ),
            check(
                "chatter",
                690.0,
                ("✕ ", Tone::Error),
                "top-level chatter ignored",
            ),
            check("actor", 730.0, ("✓ ", Tone::Success), "actor checked"),
            StageElement::label(
                "sqlite",
                [SESSION[0], 545.0, 0.0],
                20.0,
                &[("written to sqlite", Tone::Success)],
            ),
            StageElement::label(
                "dedupe",
                [SESSION[0], 585.0, 0.0],
                20.0,
                &[("keyed by message ts", Tone::Muted)],
            ),
            StageElement::packet("event", "in")
                .labeled("event")
                .tone(Tone::Request),
            StageElement::packet("enqueue", "enq")
                .labeled("enqueue")
                .tone(Tone::Request),
            StageElement::packet("commit", "enq")
                .reversed()
                .labeled("committed")
                .tone(Tone::Success),
            StageElement::packet("ok", "in")
                .reversed()
                .labeled("200")
                .tone(Tone::Success),
            StageElement::packet("again", "in")
                .labeled("same event")
                .tone(Tone::Warning),
            StageElement::packet("enqueue-2", "enq")
                .labeled("enqueue")
                .tone(Tone::Warning),
            StageElement::packet("ok-2", "in")
                .reversed()
                .labeled("200")
                .tone(Tone::Success),
        ],
    }
}

/// The Worker card on screen once the closing camera settles.
fn worker_rect() -> [f32; 4] {
    let camera = Camera {
        position: Vec3::from(CLOSING_CAMERA),
        size: vec2(1920.0, 1080.0),
    };
    let (center, scale) = camera
        .project(Vec3::from(WORKER))
        .expect("the worker is in front of the camera");
    let size = Vec2::from(WORKER_SIZE) * scale;
    let corner = center - size * 0.5;
    [corner.x, corner.y, size.x, size.y]
}

pub fn film(narration: &Narration) -> Result<(ScenePlan, [f32; 4])> {
    let Film {
        sc: mut scene,
        s: mut actor,
        v,
    } = begin(narration, "edge", 1.0, 1.9, &stage())?;
    let (s, sc) = (&mut actor, &mut scene);
    header(sc, "1", "the edge")?;
    let mut chip = chip(sc, "src/ingress.ts")?;

    s.settle_in(sc, "slack", seconds(0.4));
    arrive(s, sc, "worker", "in", seconds(0.56));
    arrive(s, sc, "session", "enq", seconds(0.72));

    // Slack posts the event; the Worker starts its checks.
    let landed = send(s, sc, "event", v.at("posts the event"), 0.85);
    s.land(sc, "worker", landed);
    status(s, sc, "slack", landed, 1);
    status(s, sc, "worker", landed, 1);
    s.clock(sc, "slack.spinner", landed);
    s.to(sc, "camera.x", landed, -60.0, 1.4);
    let checks = [
        ("signature", v.at("verifies the signature")),
        ("envelope", v.at("decodes the envelope")),
        ("team", v.at("drops other workspaces")),
        ("chatter", v.at("ambient chatter")),
        ("actor", v.at("who sent it")),
    ];
    for (index, (id, at)) in checks.into_iter().enumerate() {
        s.type_in(sc, id, at, 48.0);
        sfx::TICK.play(sc, &format!("check-{id}"), at, -24.0);
        if index == 0 {
            s.to(sc, "camera.focus", at, 0.0, 0.8);
        }
    }

    // Into the mailbox: the write commits before anything answers Slack.
    let mailbox = v.at("threads mailbox");
    status(s, sc, "worker", mailbox, 2);
    s.to(sc, "camera.x", mailbox, 120.0, 1.6);
    let stored = send(s, sc, "enqueue", mailbox, 0.8);
    s.land(sc, "session", stored);
    status(s, sc, "session", stored, 1);
    let commits = v.at("write commits");
    s.type_in(sc, "sqlite", commits, 40.0);
    s.hit(sc, "session.flash", commits, 0.5, 0.0);
    let back = send(s, sc, "commit", commits + seconds(0.5), 0.7);
    s.land(sc, "worker", back);
    let receipt_at = v.at("get its");
    status(s, sc, "worker", receipt_at, 3);
    s.to(sc, "camera.x", receipt_at, -40.0, 1.6);
    let receipt = send(s, sc, "ok", receipt_at.not_before(back + seconds(0.4)), 0.8);
    s.hit(sc, "slack.flash", receipt, 0.6, 0.0);
    s.clock(sc, "slack.mark", receipt);
    status(s, sc, "slack", receipt, 2);
    sfx::SUCCESS.play(sc, "receipt", receipt, -12.0);
    let mut receipt_footer = footer(
        sc,
        "footer-receipt",
        &[
            ("the 200 is a ", Tone::Plain),
            ("durability receipt", Tone::Accent),
        ],
        v.at("durability receipt"),
    )?;

    // A redelivery: the same Slack timestamp is already in the mailbox.
    let retry = v.at("slack retries");
    receipt_footer.hide(sc, retry);
    for clock in ["spinner", "mark"] {
        s.set(sc, &format!("slack.{clock}"), retry - seconds(0.4), -1.0);
    }
    status(s, sc, "slack", retry - seconds(0.3), 3);
    for id in ["signature", "envelope", "team", "chatter", "actor"] {
        s.to(
            sc,
            &format!("{id}.opacity"),
            retry - seconds(0.3),
            0.35,
            0.5,
        );
    }
    let again = send(s, sc, "again", retry, 0.75);
    s.land(sc, "worker", again);
    let duplicate = send(s, sc, "enqueue-2", reply_after(again), 0.75);
    status(s, sc, "session", duplicate, 2);
    s.hit(sc, "session.flash", duplicate, 0.4, 0.0);
    sfx::CONFIRM.play(sc, "dedupe", duplicate, -16.0);
    let dedupe = v.at("by message timestamp").not_before(duplicate);
    s.type_in(sc, "dedupe", dedupe, 40.0);
    let answered = send(s, sc, "ok-2", duplicate + seconds(0.5), 0.75);
    s.hit(sc, "slack.flash", answered, 0.5, 0.0);
    status(s, sc, "slack", answered, 2);
    let mut dedupe_footer = footer(
        sc,
        "footer-dedupe",
        &[
            ("a retry finds the same ts: ", Tone::Plain),
            ("no second run", Tone::Success),
        ],
        dedupe,
    )?;

    // Lean into the Worker; the reel zooms through it into its code.
    let close = v.end() + seconds(0.2);
    s.to(sc, "camera.x", close, CLOSING_CAMERA[0], 1.2);
    s.to(sc, "camera.y", close, CLOSING_CAMERA[1], 1.2);
    s.to(sc, "camera.z", close, CLOSING_CAMERA[2], 1.2);
    s.to(sc, "worker.glow", close, 0.6, 0.8);
    // Notes under the cards would loom huge through the zoom; they leave first.
    for id in [
        "signature",
        "envelope",
        "team",
        "chatter",
        "actor",
        "sqlite",
        "dedupe",
    ] {
        s.to(sc, &format!("{id}.opacity"), close, 0.0, 0.5);
    }
    dedupe_footer.hide(sc, close);
    chip.hide(sc, close);
    let plan = scene.finish().context("edge")?;
    Ok((plan, worker_rect()))
}
