//! One SessionDO per conversation: an alarm drains the mailbox, admission
//! decides wake, steer, or quiet context, and the embedded OpenCode runtime
//! keeps its state in the object's own SQLite.
use anyhow::{Context, Result};
use psychopomp::{
    author::PlanTime,
    caption::CaptionAlign,
    math::easing::Ease,
    plan::ScenePlan,
    stage::{StagePlan, reply_after},
    tone::Tone,
};

use crate::{
    BLOOM, Film, MARK, Narration, arrive, beam, begin, card, chip, footer, header, label, orb,
    orb_in, packet, plug, post, ring, seconds, send, show, sound, status,
};

const MAILBOX: [f32; 3] = [330.0, 510.0, 0.0];
const ADMISSION: [f32; 3] = [850.0, 510.0, 0.0];
const RUNTIME: [f32; 3] = [1400.0, 510.0, 0.0];
const SQLITE: [f32; 3] = [1400.0, 850.0, 0.0];
const OUTCOMES_X: f32 = 720.0;

fn stage() -> StagePlan {
    let outcome = |id: &str, y: f32, cause: &str, result: (&str, Tone)| {
        label(
            id,
            [OUTCOMES_X, y, 0.0],
            20.0,
            CaptionAlign::Left,
            &[(cause, Tone::Muted), (" → ", Tone::Muted), result],
        )
    };
    StagePlan {
        post: post(),
        elements: vec![
            label(
                "name",
                [960.0, 255.0, 0.0],
                26.0,
                CaptionAlign::Center,
                &[
                    ("SessionDO", Tone::Plain),
                    ("   team : channel : thread", Tone::Muted),
                ],
            ),
            card(
                "mailbox",
                MAILBOX,
                [280.0, 110.0],
                "mailbox",
                &[
                    ("3 pending", Tone::Plain),
                    ("2 pending", Tone::Plain),
                    ("1 pending", Tone::Plain),
                    ("empty", Tone::Muted),
                ],
                Tone::Plain,
            ),
            ring(
                "alarm",
                [MAILBOX[0] - 110.0, 385.0, 0.0],
                16.0,
                2.0,
                Tone::Warning,
            ),
            label(
                "alarm-name",
                [MAILBOX[0] - 82.0, 385.0, 0.0],
                20.0,
                CaptionAlign::Left,
                &[("alarm", Tone::Warning), (" · drains it", Tone::Muted)],
            ),
            card(
                "admission",
                ADMISSION,
                [300.0, 110.0],
                "admission",
                &[
                    ("deciding", Tone::Muted),
                    ("wake", Tone::Request),
                    ("steer", Tone::Request),
                    ("classifier…", Tone::Plain),
                    ("quiet context", Tone::Muted),
                    ("prompting", Tone::Request),
                ],
                Tone::Plain,
            ),
            orb("runtime", RUNTIME, 105.0, 700),
            label(
                "runtime-name",
                [RUNTIME[0], 652.0, 0.0],
                23.0,
                CaptionAlign::Center,
                &[("opencode runtime", Tone::Plain)],
            ),
            label(
                "runtime-embedded",
                [RUNTIME[0], 684.0, 0.0],
                19.0,
                CaptionAlign::Center,
                &[
                    ("embedded", Tone::Accent),
                    (" · no network hop", Tone::Muted),
                ],
            ),
            card(
                "sqlite",
                SQLITE,
                [300.0, 96.0],
                "sqlite",
                &[("slack + opencode state", Tone::Muted)],
                Tone::Plain,
            ),
            beam("mb", "mailbox", "admission", Tone::Plain),
            beam("ad", "admission", "runtime", Tone::Request),
            beam("db", "runtime", "sqlite", Tone::Plain),
            outcome("o-wake", 640.0, "a mention", ("wake", Tone::Request)),
            outcome(
                "o-steer",
                676.0,
                "a reply mid-turn",
                ("steer", Tone::Request),
            ),
            outcome(
                "o-classify",
                712.0,
                "several people",
                ("classifier", Tone::Plain),
            ),
            outcome(
                "o-context",
                748.0,
                "not for jr",
                ("quiet context", Tone::Plain),
            ),
            label(
                "msg-id",
                [MAILBOX[0], 605.0, 0.0],
                20.0,
                CaptionAlign::Center,
                &[("msg_… ", Tone::Accent), ("saved at enqueue", Tone::Muted)],
            ),
            label(
                "once",
                [(ADMISSION[0] + RUNTIME[0]) * 0.5, 425.0, 0.0],
                20.0,
                CaptionAlign::Center,
                &[("same id", Tone::Plain), (" → accepted once", Tone::Muted)],
            ),
            packet("m-wake", "mb", false, "@jr", Tone::Request),
            packet("p-wake", "ad", false, "wake", Tone::Request),
            packet("m-steer", "mb", false, "reply", Tone::Request),
            packet("p-steer", "ad", false, "steer", Tone::Request),
            packet("m-context", "mb", false, "chatter", Tone::Plain),
            packet("p-context", "ad", false, "context", Tone::Muted),
            packet("state", "db", false, "", Tone::Plain),
            packet("prompt", "ad", false, "prompt · msg_…", Tone::Request),
            packet("retry", "ad", false, "retry · msg_…", Tone::Warning),
        ],
    }
}

pub fn film(narration: &Narration) -> Result<ScenePlan> {
    let Film {
        sc: mut scene,
        s: mut actor,
        v,
    } = begin(narration, "session", 1.0, 2.0, &stage())?;
    let (s, sc) = (&mut actor, &mut scene);
    header(sc, "2", "one object per thread")?;
    chip(sc, "src/session/")?;

    // Exactly one object, named by its conversation root.
    let one = v.at("exactly one session object");
    s.settle_in(sc, "mailbox", seconds(0.5));
    arrive(s, sc, "admission", "mb", seconds(0.66));
    orb_in(s, sc, "runtime", one - seconds(0.2));
    show(s, sc, "runtime-name", one + seconds(0.5));
    plug(s, sc, "ad", one + seconds(0.4));
    s.type_in(sc, "name", v.at("named by its team"), 30.0);

    // The alarm: a ring sweeps closed, and the mailbox takes its energy.
    let alarm = v.at("an alarm wakes");
    s.fade_in(sc, "alarm", alarm - seconds(0.2), 1.0, 0.3);
    s.channel(sc, "alarm.sweep", 0.0);
    s.ease(
        sc,
        "alarm.sweep",
        alarm - seconds(0.2),
        1.0,
        0.7,
        Ease::Smootherstep,
    );
    s.type_in(sc, "alarm-name", alarm, 40.0);
    s.hit(sc, "mailbox.flash", alarm + seconds(0.5), 0.6, 0.0);
    sc.media(sound("alarm", MARK, alarm + seconds(0.4), -18.0));
    let decides = v.at("admission decides");
    s.hit(sc, "admission.flash", decides, 0.5, 0.0);
    let mut decides_footer = footer(
        sc,
        "footer-admission",
        &[
            ("admission: ", Tone::Plain),
            ("wake", Tone::Accent),
            (", steer, or quiet context", Tone::Plain),
        ],
        decides,
    )?;

    // Each message travels in, and admission sends on what it means.
    let rows: [(&str, &str, &str, usize, usize, u64); 3] = [
        ("m-wake", "p-wake", "o-wake", 1, 1, v.at("always wakes")),
        (
            "m-steer",
            "p-steer",
            "o-steer",
            2,
            2,
            v.at("steers the running turn"),
        ),
        (
            "m-context",
            "p-context",
            "o-classify",
            3,
            3,
            v.at("classifier decides"),
        ),
    ];
    for (message, decision, row, left, verdict, at) in rows {
        let arrived = send(s, sc, message, at - seconds(0.2), 0.75);
        status(s, sc, "mailbox", at, left);
        s.land(sc, "admission", arrived);
        status(s, sc, "admission", arrived, verdict);
        s.type_in(sc, row, arrived, 45.0);
        if decision != "p-context" {
            let landed = send(s, sc, decision, reply_after(arrived), 0.75);
            s.hit(sc, "runtime.pulse", landed, 0.6, 0.0);
        }
    }
    let quiet = v.at("quiet context");
    status(s, sc, "admission", quiet, 4);
    s.type_in(sc, "o-context", quiet, 45.0);
    let landed = send(s, sc, "p-context", quiet, 0.85);
    s.hit(sc, "runtime.pulse", landed, 0.25, 0.0);

    // Inside: the runtime and its SQLite are part of the same object.
    let embedded = v.at("embedded");
    decides_footer.hide(sc, embedded);
    for row in ["o-wake", "o-steer", "o-classify", "o-context"] {
        s.to(sc, &format!("{row}.opacity"), embedded, 0.35, 0.6);
    }
    s.to(sc, "camera.x", embedded - seconds(0.2), 110.0, 1.6);
    s.to(sc, "camera.y", embedded - seconds(0.2), 30.0, 1.6);
    s.to(sc, "camera.z", embedded - seconds(0.2), 40.0, 1.8);
    s.to(sc, "camera.focus", embedded, 0.0, 0.8);
    s.type_in(sc, "runtime-embedded", embedded + seconds(0.2), 40.0);
    let sqlite = v.at("own sqlite");
    let contact = arrive(s, sc, "sqlite", "db", sqlite - seconds(0.3));
    let stored = send(s, sc, "state", contact + seconds(0.5), 0.6);
    s.land(sc, "sqlite", stored);

    // One message ID, saved at enqueue: a retry is the same prompt.
    let upfront = v.at("generated up front");
    s.to(sc, "camera.x", upfront - seconds(0.4), 0.0, 1.6);
    s.to(sc, "camera.y", upfront - seconds(0.4), 0.0, 1.6);
    s.to(sc, "camera.z", upfront - seconds(0.4), 20.0, 1.6);
    s.type_in(sc, "msg-id", upfront, 40.0);
    status(s, sc, "admission", upfront, 5);
    let prompted = send(s, sc, "prompt", upfront + seconds(0.5), 0.8);
    s.hit(sc, "runtime.pulse", prompted, 0.6, 0.0);
    let twice = v.at("never submits twice");
    let retried = send(
        s,
        sc,
        "retry",
        twice.not_before(prompted + seconds(0.4)),
        0.8,
    );
    s.hit(sc, "runtime.pulse", retried, 0.2, 0.0);
    s.type_in(sc, "once", retried, 40.0);
    sc.media(sound("once", BLOOM, retried, -15.0));
    footer(
        sc,
        "footer-once",
        &[
            ("one message id, ", Tone::Plain),
            ("one submission", Tone::Accent),
        ],
        retried,
    )?;
    scene.finish().context("session")
}
