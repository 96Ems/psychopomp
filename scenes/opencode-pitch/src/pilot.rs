//! The pilot: two weeks, five candidate problems, three lasting proofs, one
//! owner and a budget, and a decision on day fourteen.
use anyhow::{Context, Result};
use psychopomp::{
    effects::spinner, math::easing::Ease, plan::ScenePlan, stage::StagePlan, tone::Tone,
};

use crate::{
    Film, MARK, Narration, REST_Z, SUCCESS, beam, begin, card, footer, header, label, packet, plug,
    post, ring, seconds, send, show, sound, status,
};

const CANDIDATES: [(&str, f32); 5] = [
    ("session migration", 300.0),
    ("desktop launch", 420.0),
    ("mcp handshake", 540.0),
    ("prompt delivery", 660.0),
    ("version upgrades", 780.0),
];
const PROOFS: [f32; 3] = [420.0, 540.0, 660.0];
const CLOCK: [f32; 3] = [960.0, 230.0, 0.0];

fn stage() -> StagePlan {
    let mut elements = vec![
        ring("days-track", CLOCK, 62.0, 1.5, Tone::Muted),
        ring("days", CLOCK, 62.0, 3.0, Tone::Accent),
        ring("days-pulse", CLOCK, 62.0, 2.0, Tone::Accent),
        label(
            "days-label",
            [CLOCK[0], 234.0, 0.0],
            20.0,
            &[("14 days", Tone::Plain)],
        ),
        label(
            "owner",
            [960.0, 870.0, 0.0],
            22.0,
            &[
                ("one owner", Tone::Plain),
                ("  ·  ", Tone::Muted),
                ("a spending limit", Tone::Plain),
            ],
        ),
    ];
    for (index, (title, y)) in CANDIDATES.into_iter().enumerate() {
        elements.push(card(
            &format!("candidate-{index}"),
            [480.0, y, 0.0],
            [300.0, 84.0],
            title,
            &[],
            Tone::Plain,
        ));
    }
    for (index, y) in PROOFS.into_iter().enumerate() {
        elements.extend([
            card(
                &format!("proof-{index}"),
                [1440.0, y, 0.0],
                [260.0, 84.0],
                "proof",
                &[("in progress", Tone::Muted), ("pays off", Tone::Success)],
                Tone::Success,
            ),
            beam(
                &format!("make-{index}"),
                &format!("candidate-{index}"),
                &format!("proof-{index}"),
                0.0,
                Tone::Request,
            ),
            packet(
                &format!("go-{index}"),
                &format!("make-{index}"),
                false,
                Tone::Request,
            ),
        ]);
    }
    StagePlan {
        post: post(),
        elements,
    }
}

pub fn film(narration: &Narration) -> Result<ScenePlan> {
    let Film {
        sc: mut scene,
        s: mut actor,
        v,
    } = begin(narration, "pilot", 0.7, 2.4, &stage())?;
    let (s, sc) = (&mut actor, &mut scene);
    header(sc, "04", "the pilot")?;

    // Two weeks: a clock that closes on day fourteen.
    let pilot = v.at("two week pilot");
    let day = v.at("on day 14");
    s.to(sc, "days-track.opacity", pilot, 0.45, 0.4);
    show(s, sc, "days", pilot);
    show(s, sc, "days-label", pilot + seconds(0.2));
    let span = day.saturating_sub(pilot) as f32 / 1e9;
    s.ease(sc, "days.sweep", pilot, 1.0, span, Ease::Linear);

    // Five candidates settle in a ripple.
    let five = v.at("five candidate");
    for index in 0..CANDIDATES.len() {
        s.settle_in(
            sc,
            &format!("candidate-{index}"),
            five + seconds(index as f64 * 0.12),
        );
    }

    // Three become proofs; the other two step back.
    let three = v.at("three lasting");
    for index in [3, 4] {
        s.to(sc, &format!("candidate-{index}.dim"), three, 0.6, 0.8);
    }
    let mut spinning = [0; PROOFS.len()];
    for (index, started) in spinning.iter_mut().enumerate() {
        let at = three + seconds(index as f64 * 0.14);
        let ready = s.settle_in(sc, &format!("proof-{index}"), at);
        let contact = plug(
            s,
            sc,
            &format!("make-{index}"),
            ready.saturating_sub(seconds(0.5)),
        );
        let landed = send(s, sc, &format!("go-{index}"), contact + seconds(0.05), 0.55);
        s.land(sc, &format!("proof-{index}"), landed);
        s.clock(sc, &format!("proof-{index}.spinner"), landed);
        *started = landed;
    }

    s.type_in(sc, "owner", v.at("one owner"), 30.0);

    // Day fourteen: the clock closes and the proofs must pay off.
    s.set(sc, "days-pulse.opacity", 0, 0.0);
    s.set(sc, "days-pulse.opacity", day, 1.0);
    s.ease(sc, "days-pulse.expand", day, 1.0, 0.9, Ease::CubicOut);
    s.ease(sc, "days-pulse.scale", day, 1.8, 0.9, Ease::CubicOut);
    sc.media(sound("day", MARK, day, -18.0));
    let paid = v.at("paid for themselves");
    for (index, started) in spinning.into_iter().enumerate() {
        let at = paid + seconds(index as f64 * 0.12);
        status(s, sc, &format!("proof-{index}"), at, 1);
        // Each spinner resolves into its check at its next top-right crossing.
        let waited = at.saturating_sub(started) as f32 / 1e9;
        s.clock(
            sc,
            &format!("proof-{index}.mark"),
            started + seconds(f64::from(spinner::handoff(waited))),
        );
        s.hit(sc, &format!("proof-{index}.flash"), at, 0.55, 0.0);
        s.to(sc, &format!("proof-{index}.glow"), at, 0.35, 0.6);
    }
    sc.media(sound("paid", SUCCESS, paid, -16.0));
    footer(
        sc,
        &[
            ("continue only if the proofs ", Tone::Plain),
            ("pay for themselves", Tone::Accent),
        ],
        day,
    )?;
    s.to(sc, "camera.z", paid, REST_Z + 40.0, 2.0);
    scene.finish().context("pilot")
}
