//! Slack, three threads, and for each thread its own agent and sandbox.
use anyhow::{Context, Result};
use psychopomp::{
    author::{PlanBuilder, millis, stagger},
    caption::CaptionAlign,
    plan::ScenePlan,
    stage::{StagePlan, reply_after},
    tone::Tone,
};

use crate::{
    BLOOM, Film, arrive, beam, begin, card, footer, label, orb, orb_in, packet, plug, post,
    seconds, send, sound,
};

const ROWS: [f32; 3] = [330.0, 540.0, 750.0];

fn stage() -> StagePlan {
    let mut elements = vec![card(
        "slack",
        [300.0, 540.0, 0.0],
        [240.0, 110.0],
        "slack",
        &[("a workspace", Tone::Muted)],
        Tone::Plain,
    )];
    for (index, y) in ROWS.into_iter().enumerate() {
        elements.extend([
            card(
                &format!("thread-{index}"),
                [700.0, y, 0.0],
                [250.0, 100.0],
                "thread",
                &[("@jr mentioned", Tone::Muted)],
                Tone::Plain,
            ),
            orb(&format!("agent-{index}"), [1140.0, y, 0.0], 62.0, 420),
            card(
                &format!("sandbox-{index}"),
                [1580.0, y, 0.0],
                [250.0, 100.0],
                "sandbox",
                &[("a real vm", Tone::Muted)],
                Tone::Plain,
            ),
            beam(
                &format!("in-{index}"),
                "slack",
                &format!("thread-{index}"),
                Tone::Plain,
            ),
            beam(
                &format!("own-{index}"),
                &format!("thread-{index}"),
                &format!("agent-{index}"),
                Tone::Plain,
            ),
            beam(
                &format!("hands-{index}"),
                &format!("agent-{index}"),
                &format!("sandbox-{index}"),
                Tone::Plain,
            ),
        ]);
    }
    elements.extend([
        label(
            "agents-name",
            [1140.0, 870.0, 0.0],
            20.0,
            CaptionAlign::Center,
            &[("one durable agent each", Tone::Muted)],
        ),
        packet("mention", "in-1", false, "@jr", Tone::Request),
        packet("wake", "own-1", false, "", Tone::Request),
    ]);
    StagePlan {
        post: post(),
        elements,
    }
}

pub fn film(narration: &crate::Narration) -> Result<ScenePlan> {
    let Film {
        sc: mut scene,
        s: mut actor,
        v,
    } = begin(narration, "intro", 0.9, 2.2, &stage())?;
    let (s, sc) = (&mut actor, &mut scene);
    let title = psychopomp::caption::CaptionPlan::line(
        [crate::LEFT, crate::HEADER_Y],
        30.0,
        vec![
            crate::span("opencode jr", Tone::Accent),
            crate::span("  how it works", Tone::Plain),
        ],
    );
    psychopomp::caption::CaptionActor::declare(sc, "header", &title)?.type_in(
        sc,
        seconds(0.3),
        50.0,
        0.6,
    );

    s.settle_in(sc, "slack", seconds(0.45));
    // One thread per mention: each settles and plugs into slack.
    stagger(0..3, v.at("mention it in"), millis(140), |index, at| {
        arrive(
            s,
            sc,
            &format!("thread-{index}"),
            &format!("in-{index}"),
            at,
        )
    });
    // Each thread gathers its own agent out of a blur.
    stagger(
        0..3,
        v.at("its own coding agent"),
        millis(140),
        |index, at| {
            orb_in(s, sc, &format!("agent-{index}"), at);
            plug(s, sc, &format!("own-{index}"), at + seconds(0.35))
        },
    );
    let durable = v.at("durable state");
    s.type_in(sc, "agents-name", durable, 40.0);
    let sandbox = v.at("sandboxed computer");
    for index in 0..3 {
        let card = format!("sandbox-{index}");
        let ready = s.settle_in(sc, &card, sandbox + seconds(index as f64 * 0.14));
        plug(
            s,
            sc,
            &format!("hands-{index}"),
            ready.saturating_sub(seconds(0.6)),
        );
    }
    s.to(sc, "camera.x", sandbox, 60.0, 1.6);

    // Follow one message: the camera leans into the middle row.
    let follow = v.at("follow one message");
    s.to(sc, "camera.x", follow - seconds(0.3), -120.0, 1.6);
    s.to(sc, "camera.z", follow - seconds(0.3), 90.0, 1.8);
    hide_others(s, sc, follow);
    let landed = send(s, sc, "mention", follow + seconds(0.3), 0.8);
    s.land(sc, "thread-1", landed);
    let woke = send(s, sc, "wake", reply_after(landed), 0.7);
    s.hit(sc, "agent-1.pulse", woke, 0.75, 0.0);
    sc.media(sound("wake", BLOOM, woke, -14.0));
    footer(
        sc,
        "footer",
        &[
            ("every thread is its own ", Tone::Plain),
            ("durable agent", Tone::Accent),
        ],
        durable + seconds(0.3),
    )?;
    scene.finish().context("intro")
}

/// The other rows step back so the followed message reads alone.
fn hide_others(s: &mut psychopomp::stage::StageActor, sc: &mut PlanBuilder, at: u64) {
    for index in [0, 2] {
        for card in [format!("thread-{index}"), format!("sandbox-{index}")] {
            s.to(sc, &format!("{card}.dim"), at, 0.55, 0.8);
        }
    }
}
