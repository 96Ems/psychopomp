//! Surviving resets: a Durable Object reset rolls back unflushed writes while
//! Slack posts stand, so passes are time-boxed, writes are flushed before
//! side effects, alarms are backstops, and storage is versioned.
use anyhow::{Context, Result};
use psychopomp::{
    caption::CaptionAlign,
    effects::combustion,
    math::{Vec3, easing::Ease},
    plan::ScenePlan,
    stage::StagePlan,
    tone::Tone,
};

use crate::{
    DEATH, Film, GLITCH, IMPACT, LAUNCH, MARK, Narration, RESET, arrive, beam, begin, card, chip,
    footer, header, hide, label, orb, orb_in, packet, post, ring, seconds, send, show, sound,
    status,
};

const SESSION: [f32; 3] = [640.0, 460.0, 0.0];
const SLACK: [f32; 3] = [1470.0, 460.0, 0.0];
const SQLITE: [f32; 3] = [640.0, 850.0, 0.0];

fn stage() -> StagePlan {
    StagePlan {
        post: post(),
        elements: vec![
            orb("session", SESSION, 128.0, 900),
            label(
                "session-name",
                [SESSION[0], 628.0, 0.0],
                22.0,
                CaptionAlign::Center,
                &[("SessionDO", Tone::Plain)],
            ),
            label(
                "unflushed",
                [SESSION[0], 690.0, 0.0],
                20.0,
                CaptionAlign::Center,
                &[("+ ", Tone::Warning), ("an unflushed write", Tone::Muted)],
            ),
            label(
                "rolled-back",
                [SESSION[0], 690.0, 0.0],
                20.0,
                CaptionAlign::Center,
                &[("✕ ", Tone::Error), ("rolled back", Tone::Muted)],
            ),
            card(
                "slack",
                SLACK,
                [320.0, 120.0],
                "slack thread",
                &[("waiting", Tone::Muted), ("post · stands", Tone::Plain)],
                Tone::Plain,
            ),
            label(
                "stands",
                [SLACK[0], 575.0, 0.0],
                20.0,
                CaptionAlign::Center,
                &[("still there after the reset", Tone::Warning)],
            ),
            card(
                "sqlite",
                SQLITE,
                [300.0, 90.0],
                "sqlite",
                &[
                    ("committed rows", Tone::Muted),
                    ("flushed", Tone::Success),
                    ("schema version mismatch", Tone::Warning),
                    ("tables wiped · recreated", Tone::Plain),
                ],
                Tone::Plain,
            ),
            ring("gate", SESSION, 176.0, 1.5, Tone::Muted),
            ring("budget", SESSION, 176.0, 2.5, Tone::Accent),
            label(
                "budget-name",
                [SESSION[0], 250.0, 0.0],
                20.0,
                CaptionAlign::Center,
                &[
                    ("time-boxed pass", Tone::Accent),
                    (" · returns before the limit", Tone::Muted),
                ],
            ),
            ring(
                "alarm",
                [SESSION[0] + 400.0, 760.0, 0.0],
                16.0,
                2.0,
                Tone::Warning,
            ),
            label(
                "alarm-name",
                [SESSION[0] + 428.0, 760.0, 0.0],
                20.0,
                CaptionAlign::Left,
                &[
                    ("alarm", Tone::Warning),
                    (" · a backstop, not a poll loop", Tone::Muted),
                ],
            ),
            beam("out", "session", "slack", Tone::Request),
            beam("db", "session", "sqlite", Tone::Plain),
            packet("first", "out", false, "post", Tone::Request),
            packet("flush", "db", false, "flush", Tone::Success),
            packet("second", "out", false, "post", Tone::Request),
        ],
    }
}

pub fn film(narration: &Narration) -> Result<ScenePlan> {
    let Film {
        sc: mut scene,
        s: mut actor,
        v,
    } = begin(narration, "resets", 1.0, 2.0, &stage())?;
    let (s, sc) = (&mut actor, &mut scene);
    header(sc, "5", "surviving resets")?;
    chip(sc, "src/session/alarm-timing.ts")?;

    orb_in(s, sc, "session", seconds(0.2));
    show(s, sc, "session-name", seconds(0.8));
    arrive(s, sc, "slack", "out", seconds(0.6));
    arrive(s, sc, "sqlite", "db", seconds(0.8));

    // A pass writes, then posts: the post escapes before the write is flushed.
    let rule = v.at("one rule");
    s.type_in(sc, "unflushed", rule, 40.0);
    let posted = send(s, sc, "first", rule + seconds(0.9), 0.8);
    s.land(sc, "slack", posted);
    status(s, sc, "slack", posted, 1);

    // The reset: the object bursts and its unflushed write is gone.
    let reset = v.at("object resets");
    s.clock_for(sc, "session.burst", reset, combustion::DURATION);
    s.to(sc, "session.hurt", reset, 1.0, 0.2);
    s.hit(sc, "post.chroma", reset, 0.14, 0.0);
    s.hit(sc, "post.bloom", reset, 0.35, 0.18);
    let blow = Vec3::from(SESSION) - Vec3::from(SLACK);
    s.jolt(sc, reset, [blow.x, blow.y], 0.8);
    for (card, at) in [("slack", SLACK), ("sqlite", SQLITE)] {
        let away = (Vec3::from(at) - Vec3::from(SESSION)).truncate();
        let passes = reset + seconds(f64::from(combustion::shock_arrival(away.length())));
        let push = away.normalize() * 8.0;
        s.kick(
            sc,
            [&format!("{card}.x"), &format!("{card}.y")],
            passes,
            push.into(),
        );
    }
    sc.media(sound("reset-impact", IMPACT, reset, -8.0));
    sc.media(sound("reset-burst", DEATH, reset + seconds(0.05), -10.0));
    s.to(sc, "out.flow", reset, 0.0, 0.2);
    s.to(sc, "out.break", reset + seconds(0.15), 1.0, 0.8);
    s.to(sc, "db.break", reset + seconds(0.2), 1.0, 0.8);
    let roll = v.at("roll back").max(reset + seconds(0.3));
    hide(s, sc, "unflushed", roll);
    s.type_in(sc, "rolled-back", roll + seconds(0.15), 40.0);
    sc.media(sound("rolled-back", GLITCH, roll, -18.0));
    let stand = v.at("still stand");
    s.hit(sc, "slack.flash", stand, 0.6, 0.0);
    s.to(sc, "slack.glow", stand, 0.4, 0.5);
    s.type_in(sc, "stands", stand, 40.0);
    let mut stand_footer = footer(
        sc,
        "footer-stand",
        &[
            ("writes roll back, ", Tone::Plain),
            ("posts stand", Tone::Accent),
        ],
        stand + seconds(0.3),
    )?;

    // Rewind: the object reassembles, and the pass is time-boxed.
    let boxed = v.at("coordinator");
    let rewind = boxed - seconds(0.4);
    s.clock_for(sc, "post.rewind", rewind, 1.4);
    sc.media(sound("rewind", LAUNCH, rewind - seconds(0.1), -15.0));
    s.ease(
        sc,
        "session.burst",
        rewind + seconds(0.1),
        0.0,
        1.2,
        Ease::Smootherstep,
    );
    s.set(sc, "session.burst", rewind + seconds(1.3), -1.0);
    s.to(sc, "session.hurt", rewind + seconds(0.6), 0.0, 0.6);
    s.to(sc, "out.break", rewind + seconds(0.4), 0.0, 0.9);
    s.to(sc, "db.break", rewind + seconds(0.5), 0.0, 0.9);
    s.twang(sc, "out", rewind + seconds(1.2));
    s.twang(sc, "db", rewind + seconds(1.3));
    hide(s, sc, "rolled-back", rewind);
    hide(s, sc, "stands", rewind);
    s.to(sc, "slack.glow", rewind, 0.0, 0.5);
    status(s, sc, "slack", rewind + seconds(0.6), 0);
    stand_footer.hide(sc, rewind);
    let ring_at = rewind + seconds(1.3);
    s.fade_in(sc, "gate", ring_at, 0.3, 0.4);
    s.fade_in(sc, "budget", ring_at, 1.0, 0.3);
    s.channel(sc, "budget.sweep", 0.0);
    s.ease(sc, "budget.sweep", ring_at, 0.67, 1.4, Ease::Smootherstep);
    s.type_in(sc, "budget-name", ring_at, 44.0);
    s.hit(sc, "session.pulse", ring_at + seconds(1.4), 0.5, 0.0);
    sc.media(sound("budget", MARK, ring_at + seconds(1.4), -18.0));

    // Flush, then the side effect.
    let flush = v.at("before the side effect");
    hide(s, sc, "gate", flush - seconds(0.6));
    hide(s, sc, "budget", flush - seconds(0.6));
    s.to(sc, "budget-name.opacity", flush - seconds(0.6), 0.35, 0.4);
    let flushed = send(s, sc, "flush", flush - seconds(0.4), 0.6);
    s.land(sc, "sqlite", flushed);
    status(s, sc, "sqlite", flushed, 1);
    let posted = send(s, sc, "second", flushed + seconds(0.45), 0.8);
    s.land(sc, "slack", posted);
    status(s, sc, "slack", posted, 1);
    let mut flush_footer = footer(
        sc,
        "footer-flush",
        &[
            ("flush first, ", Tone::Accent),
            ("then the post", Tone::Plain),
        ],
        flushed,
    )?;

    // Alarms are backstops.
    let backstop = v.at("backstops");
    s.fade_in(sc, "alarm", backstop - seconds(0.3), 1.0, 0.3);
    s.channel(sc, "alarm.sweep", 0.0);
    s.ease(
        sc,
        "alarm.sweep",
        backstop - seconds(0.3),
        1.0,
        0.7,
        Ease::Smootherstep,
    );
    s.type_in(sc, "alarm-name", backstop, 44.0);

    // Never migrated: a version mismatch wipes and recreates the tables.
    let migrated = v.at("never migrated");
    flush_footer.hide(sc, migrated);
    s.to(sc, "camera.y", migrated - seconds(0.3), 70.0, 1.6);
    s.to(sc, "camera.z", migrated - seconds(0.3), 60.0, 1.6);
    hide(s, sc, "budget-name", migrated - seconds(0.3));
    status(s, sc, "sqlite", migrated, 2);
    s.hit(sc, "sqlite.alarm", migrated, 0.3, 0.0);
    let wiped = v.at("wipes the tables");
    for (step, seed) in [8.0, 7.0, 9.0, 0.0].into_iter().enumerate() {
        s.set(
            sc,
            "sqlite.glitch",
            wiped + seconds(step as f64 * 0.027),
            seed,
        );
    }
    sc.media(sound("wiped", GLITCH, wiped, -18.0));
    status(s, sc, "sqlite", wiped + seconds(0.1), 3);
    s.hit(sc, "sqlite.flash", wiped + seconds(0.3), 0.6, 0.0);
    sc.media(sound("recreated", RESET, wiped + seconds(0.3), -16.0));
    footer(
        sc,
        "footer-schema",
        &[
            ("storage is versioned: ", Tone::Plain),
            ("wiped, never migrated", Tone::Accent),
        ],
        wiped,
    )?;
    s.to(sc, "camera.y", v.end(), 0.0, 1.6);
    s.to(sc, "camera.z", v.end(), 0.0, 1.6);
    scene.finish().context("resets")
}
