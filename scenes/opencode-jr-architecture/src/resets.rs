//! Surviving resets: a Durable Object reset rolls back unflushed writes while
//! Slack posts stand, so passes are time-boxed, writes are flushed before
//! side effects, alarms are backstops, and storage is versioned.
use anyhow::{Context, Result};
use psychopomp::{
    author::PlanTime,
    caption::CaptionAlign,
    effects::combustion,
    math::Vec3,
    plan::ScenePlan,
    sfx,
    stage::{OrbEntrance, StageElement, StagePlan, StagePost},
    tone::Tone,
};

use crate::{
    Film, Narration, arrive, begin, chip, footer, header, hide, seconds, send, show, status,
};

const SESSION: [f32; 3] = [640.0, 460.0, 0.0];
const SLACK: [f32; 3] = [1470.0, 460.0, 0.0];
const SQLITE: [f32; 3] = [640.0, 850.0, 0.0];

fn stage() -> StagePlan {
    StagePlan {
        post: StagePost::RESTRAINED,
        elements: vec![
            StageElement::orb("session", SESSION, 128.0)
                .points(900)
                .tone(Tone::Plain),
            StageElement::label(
                "session-name",
                [SESSION[0], 628.0, 0.0],
                22.0,
                &[("SessionDO", Tone::Plain)],
            ),
            StageElement::label(
                "unflushed",
                [SESSION[0], 690.0, 0.0],
                20.0,
                &[("+ ", Tone::Warning), ("an unflushed write", Tone::Muted)],
            ),
            StageElement::label(
                "rolled-back",
                [SESSION[0], 690.0, 0.0],
                20.0,
                &[("✕ ", Tone::Error), ("rolled back", Tone::Muted)],
            ),
            StageElement::card("slack", SLACK, [320.0, 120.0], "slack thread")
                .statuses(&[("waiting", Tone::Muted), ("post · stands", Tone::Plain)]),
            StageElement::label(
                "stands",
                [SLACK[0], 575.0, 0.0],
                20.0,
                &[("still there after the reset", Tone::Warning)],
            ),
            StageElement::card("sqlite", SQLITE, [300.0, 90.0], "sqlite").statuses(&[
                ("committed rows", Tone::Muted),
                ("flushed", Tone::Success),
                ("schema version mismatch", Tone::Warning),
                ("tables wiped · recreated", Tone::Plain),
            ]),
            StageElement::ring("gate", SESSION, 176.0)
                .thickness(1.5)
                .tone(Tone::Muted),
            StageElement::ring("budget", SESSION, 176.0)
                .thickness(2.5)
                .tone(Tone::Accent),
            StageElement::label(
                "budget-name",
                [SESSION[0], 250.0, 0.0],
                20.0,
                &[
                    ("time-boxed pass", Tone::Accent),
                    (" · returns before the limit", Tone::Muted),
                ],
            ),
            StageElement::ring("alarm", [SESSION[0] + 400.0, 760.0, 0.0], 16.0)
                .thickness(2.0)
                .tone(Tone::Warning),
            StageElement::label(
                "alarm-name",
                [SESSION[0] + 428.0, 760.0, 0.0],
                20.0,
                &[
                    ("alarm", Tone::Warning),
                    (" · a backstop, not a poll loop", Tone::Muted),
                ],
            )
            .align(CaptionAlign::Left),
            StageElement::beam("out", "session", "slack").tone(Tone::Request),
            StageElement::beam("db", "session", "sqlite"),
            StageElement::packet("first", "out")
                .labeled("post")
                .tone(Tone::Request),
            StageElement::packet("flush", "db")
                .labeled("flush")
                .tone(Tone::Success),
            StageElement::packet("second", "out")
                .labeled("post")
                .tone(Tone::Request),
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

    s.orb_in(sc, "session", seconds(0.2), OrbEntrance::HERO);
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
    for card in ["slack", "sqlite"] {
        s.shock_kick(sc, "session", reset, card, 8.0, None);
    }
    sfx::IMPACT.play(sc, "reset-impact", reset, -8.0);
    sfx::DEATH.play(sc, "reset-burst", reset + seconds(0.05), -10.0);
    s.to(sc, "out.flow", reset, 0.0, 0.2);
    s.to(sc, "out.break", reset + seconds(0.15), 1.0, 0.8);
    s.to(sc, "db.break", reset + seconds(0.2), 1.0, 0.8);
    let roll = v.at("roll back").not_before(reset + seconds(0.3));
    hide(s, sc, "unflushed", roll);
    s.type_in(sc, "rolled-back", roll + seconds(0.15), 40.0);
    sfx::GLITCH.play(sc, "rolled-back", roll, -18.0);
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
    s.rewind(sc, rewind, 0.0);
    sfx::LAUNCH.play(sc, "rewind", rewind - seconds(0.1), -15.0);
    s.unburst(sc, "session", rewind + seconds(0.1), 1.2);
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
    s.ring_timer(sc, "budget", ring_at, 1.4, 0.67);
    s.type_in(sc, "budget-name", ring_at, 44.0);
    s.hit(sc, "session.pulse", ring_at + seconds(1.4), 0.5, 0.0);
    sfx::MARK.play(sc, "budget", ring_at + seconds(1.4), -18.0);

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
    s.ring_timer(sc, "alarm", backstop - seconds(0.3), 0.7, 1.0);
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
    s.glitch(sc, "sqlite", wiped, [8.0, 7.0, 9.0]);
    sfx::GLITCH.play(sc, "wiped", wiped, -18.0);
    status(s, sc, "sqlite", wiped + seconds(0.1), 3);
    s.hit(sc, "sqlite.flash", wiped + seconds(0.3), 0.6, 0.0);
    sfx::RESET.play(sc, "recreated", wiped + seconds(0.3), -16.0);
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
