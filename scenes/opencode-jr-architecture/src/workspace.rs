//! The workspace: WorkspaceDO owns one Modal sandbox VM with a daemon; a
//! template registry keeps a prepared checkout; idle sandboxes are
//! snapshotted, shut down, and restored on demand.
use anyhow::{Context, Result};
use psychopomp::{
    author::PlanTime,
    math::easing::Ease,
    plan::ScenePlan,
    stage::{StageElement, StagePlan, StagePost, reply_after},
    tone::Tone,
};

use crate::{
    Film, MARK, Narration, RESET, SUCCESS, arrive, begin, chip, footer, header, hide, orb_in, plug,
    seconds, send, show, sound, status,
};

const SESSION: [f32; 3] = [330.0, 580.0, 0.0];
const WORKSPACE: [f32; 3] = [830.0, 580.0, 0.0];
const SANDBOX: [f32; 3] = [1430.0, 580.0, 0.0];
const TEMPLATE: [f32; 3] = [1430.0, 270.0, 0.0];

fn stage() -> StagePlan {
    StagePlan {
        post: StagePost::RESTRAINED,
        elements: vec![
            StageElement::orb("session", SESSION, 80.0)
                .points(500)
                .tone(Tone::Plain),
            StageElement::label(
                "session-name",
                [SESSION[0], 690.0, 0.0],
                21.0,
                &[("SessionDO", Tone::Plain)],
            ),
            StageElement::label(
                "no-wait",
                [SESSION[0], 440.0, 0.0],
                19.0,
                &[
                    ("text-only reply", Tone::Plain),
                    (" → no wait", Tone::Muted),
                ],
            ),
            StageElement::card("workspace", WORKSPACE, [300.0, 110.0], "WorkspaceDO").statuses(&[
                ("no sandbox yet", Tone::Muted),
                ("attached", Tone::Plain),
                ("snapshot saved", Tone::Plain),
                ("restoring", Tone::Plain),
                ("attached", Tone::Success),
            ]),
            StageElement::card("sandbox", SANDBOX, [400.0, 150.0], "modal sandbox").statuses(&[
                ("vm · daemon", Tone::Muted),
                ("running a command", Tone::Plain),
                ("repo mounted", Tone::Success),
                ("snapshotted", Tone::Plain),
                ("shut down", Tone::Muted),
                ("restored from snapshot", Tone::Success),
            ]),
            StageElement::card("template", TEMPLATE, [340.0, 100.0], "template registry").statuses(
                &[
                    ("opencode repo checkout", Tone::Muted),
                    ("refreshing", Tone::Plain),
                    ("dependencies installed", Tone::Success),
                ],
            ),
            StageElement::ring("hourly", [TEMPLATE[0] - 225.0, TEMPLATE[1], 0.0], 18.0)
                .thickness(2.0),
            StageElement::label(
                "hourly-name",
                [TEMPLATE[0] - 225.0, TEMPLATE[1] + 42.0, 0.0],
                18.0,
                &[("hourly", Tone::Muted)],
            ),
            StageElement::ring("idle", SANDBOX, 236.0)
                .thickness(2.0)
                .tone(Tone::Warning),
            StageElement::label(
                "idle-name",
                [SANDBOX[0], 850.0, 0.0],
                20.0,
                &[("20 idle minutes", Tone::Warning)],
            ),
            StageElement::label(
                "daemon",
                [SANDBOX[0], 690.0, 0.0],
                19.0,
                &[("process + file server", Tone::Muted)],
            ),
            StageElement::beam("ws", "session", "workspace"),
            StageElement::beam("sb", "workspace", "sandbox"),
            StageElement::beam("tp", "template", "sandbox"),
            StageElement::packet("cmd", "ws")
                .labeled("shell · file edit")
                .tone(Tone::Request),
            StageElement::packet("cmd-2", "sb").tone(Tone::Request),
            StageElement::packet("out-2", "sb")
                .reversed()
                .tone(Tone::Success),
            StageElement::packet("out", "ws")
                .reversed()
                .labeled("output")
                .tone(Tone::Success),
            StageElement::packet("mount", "tp")
                .labeled("mount")
                .tone(Tone::Success),
            StageElement::packet("snapshot", "sb")
                .reversed()
                .labeled("snapshot"),
            StageElement::packet("restore", "sb").labeled("restore"),
        ],
    }
}

pub fn film(narration: &Narration) -> Result<ScenePlan> {
    let Film {
        sc: mut scene,
        s: mut actor,
        v,
    } = begin(narration, "workspace", 1.0, 2.0, &stage())?;
    let (s, sc) = (&mut actor, &mut scene);
    header(sc, "3", "the workspace")?;
    chip(sc, "src/workspace/")?;

    for wire in ["sb", "tp"] {
        s.channel(sc, &format!("{wire}.opacity"), 1.0);
    }
    orb_in(s, sc, "session", seconds(0.3));
    show(s, sc, "session-name", seconds(0.9));
    let object = v.at("workspace object");
    arrive(s, sc, "workspace", "ws", object - seconds(0.2));
    let sandbox = v.at("modal sandbox");
    let contact = arrive(s, sc, "sandbox", "sb", sandbox - seconds(0.2));
    status(s, sc, "workspace", contact, 1);
    s.to(sc, "camera.x", sandbox, 140.0, 1.6);
    s.type_in(sc, "daemon", v.at("small daemon"), 40.0);

    // A command crosses into the sandbox; its output comes back.
    let shell = v.at("shell commands");
    s.to(sc, "camera.x", shell, 40.0, 1.6);
    let at_workspace = send(s, sc, "cmd", shell, 0.75);
    s.land(sc, "workspace", at_workspace);
    let at_sandbox = send(s, sc, "cmd-2", reply_after(at_workspace), 0.7);
    s.land(sc, "sandbox", at_sandbox);
    status(s, sc, "sandbox", at_sandbox, 1);
    s.clock(sc, "sandbox.spinner", at_sandbox);
    let back = send(s, sc, "out-2", at_sandbox + seconds(0.9), 0.7);
    s.set(sc, "sandbox.spinner", back, -1.0);
    status(s, sc, "sandbox", back, 0);
    let returned = send(s, sc, "out", reply_after(back), 0.75);
    s.hit(sc, "session.pulse", returned, 0.6, 0.0);
    s.type_in(sc, "no-wait", v.at("dont wait"), 40.0);

    // The template registry keeps a prepared checkout; cloning mounts it.
    let hour = v.at("every hour");
    s.to(sc, "camera.x", hour - seconds(0.2), 90.0, 1.6);
    s.to(sc, "camera.y", hour - seconds(0.2), -60.0, 1.6);
    s.settle_in(sc, "template", hour - seconds(0.3));
    s.fade_in(sc, "hourly", hour, 1.0, 0.3);
    s.channel(sc, "hourly.sweep", 0.0);
    s.ease(sc, "hourly.sweep", hour, 1.0, 1.2, Ease::Smootherstep);
    show(s, sc, "hourly-name", hour + seconds(0.2));
    status(s, sc, "template", hour + seconds(0.2), 1);
    s.clock(sc, "template.spinner", hour + seconds(0.2));
    let refreshed = hour + seconds(1.4);
    s.set(sc, "template.spinner", refreshed, -1.0);
    status(s, sc, "template", refreshed, 2);
    sc.media(sound("refreshed", MARK, refreshed, -18.0));
    let mount = v.at("not a download");
    let contact = plug(s, sc, "tp", mount - seconds(1.3));
    let mounted = send(
        s,
        sc,
        "mount",
        (mount - seconds(0.2)).not_before(contact + seconds(0.3)),
        0.7,
    );
    s.land(sc, "sandbox", mounted);
    status(s, sc, "sandbox", mounted, 2);
    sc.media(sound("mounted", SUCCESS, mounted, -14.0));
    let mut mount_footer = footer(
        sc,
        "footer-mount",
        &[
            ("cloning the prepared repo is a ", Tone::Plain),
            ("mount", Tone::Accent),
        ],
        mounted,
    )?;

    // Idle: a timer ring sweeps closed, then snapshot, then shut down.
    let idle = v.at("idle minutes");
    mount_footer.hide(sc, idle - seconds(0.6));
    s.to(sc, "camera.x", idle - seconds(0.6), 120.0, 1.6);
    s.to(sc, "camera.y", idle - seconds(0.6), 30.0, 1.6);
    s.to(sc, "tp.opacity", idle - seconds(0.6), 0.25, 0.6);
    s.to(sc, "template.dim", idle - seconds(0.6), 0.6, 0.8);
    s.fade_in(sc, "idle", idle - seconds(0.5), 0.7, 0.3);
    s.channel(sc, "idle.sweep", 0.0);
    s.ease(
        sc,
        "idle.sweep",
        idle - seconds(0.5),
        1.0,
        1.4,
        Ease::Smootherstep,
    );
    s.type_in(sc, "idle-name", idle - seconds(0.3), 40.0);
    let snapshotted = v.at("snapshotted").not_before(idle + seconds(1.0));
    hide(s, sc, "idle", snapshotted);
    hide(s, sc, "idle-name", snapshotted + seconds(0.2));
    status(s, sc, "sandbox", snapshotted, 3);
    s.hit(sc, "sandbox.flash", snapshotted, 0.5, 0.0);
    let saved = send(s, sc, "snapshot", snapshotted + seconds(0.2), 0.7);
    s.land(sc, "workspace", saved);
    status(s, sc, "workspace", saved, 2);
    let down = saved + seconds(0.5);
    status(s, sc, "sandbox", down, 4);
    s.to(sc, "sandbox.dim", down, 0.7, 0.6);
    s.to(sc, "sb.flow", down, 0.0, 0.3);
    s.to(sc, "sb.opacity", down, 0.3, 0.6);
    sc.media(sound("down", RESET, down, -16.0));

    // Restored from that snapshot when it is needed again.
    let restored = v.at("restored").not_before(down + seconds(0.8));
    status(s, sc, "workspace", restored - seconds(0.4), 3);
    let reached = send(s, sc, "restore", restored - seconds(0.2), 0.7);
    s.to(sc, "sb.opacity", restored - seconds(0.4), 1.0, 0.4);
    s.to(sc, "sandbox.dim", reached, 0.0, 0.5);
    s.land(sc, "sandbox", reached);
    s.twang(sc, "sb", reached);
    status(s, sc, "sandbox", reached, 5);
    status(s, sc, "workspace", reached, 4);
    sc.media(sound("restored", SUCCESS, reached, -13.0));
    footer(
        sc,
        "footer-restore",
        &[
            ("idle sandboxes are ", Tone::Plain),
            ("snapshotted", Tone::Accent),
            (", then restored", Tone::Plain),
        ],
        reached,
    )?;
    scene.finish().context("workspace")
}
