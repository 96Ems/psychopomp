//! Around the core: durable approval cards, the per-team scheduler, and the
//! settings and connectors objects behind the web dashboard.
use anyhow::{Context, Result};
use psychopomp::{
    caption::CaptionAlign, math::easing::Ease, plan::ScenePlan, stage::StagePlan, tone::Tone,
};

use crate::{
    BLOOM, CONFIRM, Film, MARK, Narration, SUCCESS, beam, begin, card, chip, footer, header, label,
    orb, orb_in, packet, plug, post, ring, seconds, send, show, sound, status,
};

const SESSION: [f32; 3] = [960.0, 470.0, 0.0];
const APPROVAL: [f32; 3] = [420.0, 300.0, 0.0];
const SCHEDULER: [f32; 3] = [420.0, 720.0, 0.0];
const FRESH: [f32; 3] = [960.0, 830.0, 0.0];
const SETTINGS: [f32; 3] = [1300.0, 280.0, 0.0];
const CONNECTORS: [f32; 3] = [1300.0, 690.0, 0.0];
const DASHBOARD: [f32; 3] = [1650.0, 485.0, 0.0];

fn stage() -> StagePlan {
    StagePlan {
        post: post(),
        elements: vec![
            orb("session", SESSION, 104.0, 700),
            label(
                "session-name",
                [SESSION[0], 610.0, 0.0],
                21.0,
                CaptionAlign::Center,
                &[("SessionDO", Tone::Plain)],
            ),
            card(
                "approval",
                APPROVAL,
                [340.0, 120.0],
                "approval card",
                &[
                    ("pending", Tone::Warning),
                    ("executing", Tone::Plain),
                    ("completed", Tone::Success),
                ],
                Tone::Plain,
            ),
            label(
                "args",
                [APPROVAL[0], 400.0, 0.0],
                19.0,
                CaptionAlign::Center,
                &[("full arguments recorded", Tone::Muted)],
            ),
            label(
                "claim",
                [APPROVAL[0], 435.0, 0.0],
                19.0,
                CaptionAlign::Center,
                &[
                    ("pending → executing", Tone::Plain),
                    (" · flushed first", Tone::Muted),
                ],
            ),
            label(
                "already",
                [APPROVAL[0], 470.0, 0.0],
                19.0,
                CaptionAlign::Center,
                &[("second click", Tone::Plain), (" → already", Tone::Muted)],
            ),
            card(
                "scheduler",
                SCHEDULER,
                [320.0, 110.0],
                "SchedulerDO",
                &[
                    ("one per team", Tone::Muted),
                    ("firing", Tone::Plain),
                    ("fired once · late", Tone::Warning),
                ],
                Tone::Plain,
            ),
            ring(
                "clock",
                [SCHEDULER[0] - 205.0, SCHEDULER[1], 0.0],
                18.0,
                2.0,
                Tone::Plain,
            ),
            label(
                "burst",
                [SCHEDULER[0], 815.0, 0.0],
                19.0,
                CaptionAlign::Center,
                &[
                    ("missed ticks: one late run, ", Tone::Muted),
                    ("never a burst", Tone::Plain),
                ],
            ),
            orb("fresh", FRESH, 46.0, 260),
            label(
                "fresh-name",
                [FRESH[0] + 70.0, FRESH[1], 0.0],
                19.0,
                CaptionAlign::Left,
                &[("an ordinary new session", Tone::Muted)],
            ),
            card(
                "settings",
                SETTINGS,
                [280.0, 100.0],
                "SettingsDO",
                &[("configuration", Tone::Muted)],
                Tone::Plain,
            ),
            card(
                "connectors",
                CONNECTORS,
                [280.0, 100.0],
                "ConnectorsDO",
                &[("connections", Tone::Muted)],
                Tone::Plain,
            ),
            card(
                "dashboard",
                DASHBOARD,
                [260.0, 100.0],
                "web dashboard",
                &[("operators", Tone::Muted)],
                Tone::Plain,
            ),
            beam("ap", "session", "approval", Tone::Request),
            beam("sf", "scheduler", "fresh", Tone::Plain),
            beam("ds", "dashboard", "settings", Tone::Plain),
            beam("dc", "dashboard", "connectors", Tone::Plain),
            beam("ss", "settings", "session", Tone::Plain),
            beam("cs", "connectors", "session", Tone::Plain),
            packet("call", "ap", false, "github comment", Tone::Request),
            packet("result", "ap", true, "result", Tone::Success),
            packet("fire", "sf", false, "fire", Tone::Plain),
        ],
    }
}

pub fn film(narration: &Narration) -> Result<ScenePlan> {
    let Film {
        sc: mut scene,
        s: mut actor,
        v,
    } = begin(narration, "around", 1.0, 2.2, &stage())?;
    let (s, sc) = (&mut actor, &mut scene);
    header(sc, "6", "around the core")?;
    chip(sc, "src/session/approval-lifecycle.ts")?;

    orb_in(s, sc, "session", seconds(0.3));
    show(s, sc, "session-name", seconds(0.9));
    // The shared objects take their places, dim, until each one is named.
    let shared = v.at("shared objects");
    for (index, card) in [
        "approval",
        "settings",
        "dashboard",
        "connectors",
        "scheduler",
    ]
    .into_iter()
    .enumerate()
    {
        s.channel(sc, &format!("{card}.dim"), 0.6);
        s.settle_in(sc, card, shared + seconds(index as f64 * 0.12));
    }

    // A gated tool records its call and posts a card; the turn ends.
    let approval = v.at("humans approval");
    s.to(sc, "approval.dim", approval - seconds(0.4), 0.0, 0.5);
    let contact = plug(s, sc, "ap", approval - seconds(0.4));
    s.to(sc, "camera.x", approval - seconds(0.4), -80.0, 1.6);
    s.to(sc, "camera.y", approval - seconds(0.4), -40.0, 1.6);
    let call = send(s, sc, "call", contact + seconds(0.3), 0.8);
    s.land(sc, "approval", call);
    s.type_in(sc, "args", v.at("full arguments").max(call), 44.0);
    s.hit(sc, "approval.flash", v.at("card is posted"), 0.5, 0.0);
    s.clock(sc, "approval.spinner", v.at("card is posted"));
    let ends = v.at("turn ends");
    s.to(sc, "session.opacity", ends, 0.45, 0.6);

    // Approve: the claim commits first, so a second click finds it taken.
    let click = v.at("clicks approve");
    s.hit(sc, "approval.flash", click, 0.8, 0.0);
    sc.media(sound("click", CONFIRM, click, -12.0));
    let claim = v.at("commits first");
    status(s, sc, "approval", claim, 1);
    s.type_in(sc, "claim", claim, 44.0);
    sc.media(sound("claim", MARK, claim, -18.0));
    let mut claim_footer = footer(
        sc,
        "footer-claim",
        &[
            ("the claim commits ", Tone::Plain),
            ("before", Tone::Accent),
            (" the action runs", Tone::Plain),
        ],
        claim,
    )?;
    let double = v.at("double click");
    s.hit(sc, "approval.flash", double, 0.35, 0.0);
    sc.media(sound("click-again", CONFIRM, double, -18.0));
    s.type_in(sc, "already", double + seconds(0.2), 44.0);
    let wakes = v.at("wakes the model");
    let returned = send(s, sc, "result", wakes - seconds(0.2), 0.8);
    s.set(sc, "approval.spinner", returned - seconds(0.8), -1.0);
    status(s, sc, "approval", returned - seconds(0.8), 2);
    s.to(sc, "session.opacity", returned, 1.0, 0.4);
    s.hit(sc, "session.pulse", returned, 0.75, 0.0);
    sc.media(sound("woke", SUCCESS, returned, -13.0));

    // The scheduler fires an ordinary new session; catch-up fires once.
    let scheduler = v.at("scheduler object");
    claim_footer.hide(sc, scheduler - seconds(0.3));
    for id in ["args", "claim", "already"] {
        s.to(
            sc,
            &format!("{id}.opacity"),
            scheduler - seconds(0.3),
            0.3,
            0.5,
        );
    }
    s.to(sc, "camera.y", scheduler - seconds(0.3), 80.0, 1.6);
    s.to(sc, "scheduler.dim", scheduler - seconds(0.2), 0.0, 0.5);
    s.fade_in(sc, "clock", scheduler + seconds(0.3), 1.0, 0.3);
    s.channel(sc, "clock.sweep", 0.0);
    s.ease(
        sc,
        "clock.sweep",
        scheduler + seconds(0.3),
        1.0,
        1.0,
        Ease::Smootherstep,
    );
    let fresh = v.at("new session");
    status(s, sc, "scheduler", fresh - seconds(0.6), 1);
    let contact = plug(s, sc, "sf", fresh - seconds(0.9));
    let fired = send(
        s,
        sc,
        "fire",
        (fresh - seconds(0.1)).max(contact + seconds(0.2)),
        0.6,
    );
    orb_in(s, sc, "fresh", fired - seconds(0.1));
    s.hit(sc, "fresh.pulse", fired + seconds(0.3), 0.6, 0.0);
    s.type_in(sc, "fresh-name", fired + seconds(0.2), 44.0);
    sc.media(sound("fired", BLOOM, fired, -16.0));
    let once = v.at("fires once");
    status(s, sc, "scheduler", once, 2);
    s.type_in(
        sc,
        "burst",
        v.at("never in a burst").max(once + seconds(0.3)),
        44.0,
    );
    let mut once_footer = footer(
        sc,
        "footer-once",
        &[
            ("after downtime: ", Tone::Plain),
            ("once, late", Tone::Accent),
        ],
        once,
    )?;

    // Behind the dashboard: settings and connectors.
    let dashboard = v.at("web dashboard");
    once_footer.hide(sc, dashboard - seconds(0.3));
    s.to(sc, "camera.x", dashboard - seconds(0.4), 90.0, 1.6);
    s.to(sc, "camera.y", dashboard - seconds(0.4), 0.0, 1.6);
    s.to(sc, "dashboard.dim", dashboard - seconds(0.2), 0.0, 0.5);
    let objects = v.at("settings and connectors");
    for card in ["settings", "connectors"] {
        s.to(sc, &format!("{card}.dim"), objects - seconds(0.3), 0.0, 0.5);
    }
    plug(s, sc, "ds", objects - seconds(0.3));
    plug(s, sc, "dc", objects - seconds(0.14));
    plug(s, sc, "ss", objects + seconds(0.5));
    plug(s, sc, "cs", objects + seconds(0.64));
    s.to(sc, "camera.x", v.end(), 0.0, 1.6);
    s.to(sc, "camera.z", v.end(), -40.0, 1.8);
    footer(
        sc,
        "footer-core",
        &[
            ("shared objects around ", Tone::Plain),
            ("one object per thread", Tone::Accent),
        ],
        objects + seconds(0.6),
    )?;
    scene.finish().context("around")
}
