//! The workspace: WorkspaceDO owns one Modal sandbox VM with a daemon; a
//! template registry keeps a prepared checkout; idle sandboxes are
//! snapshotted, shut down, and restored on demand.
use anyhow::{Context, Result};
use kinograph::{
    caption::CaptionAlign, math::easing::Ease, plan::ScenePlan, stage::StagePlan, tone::Tone,
};

use crate::{
    Film, MARK, RESET, SUCCESS, arrive, beam, begin, card, chip, footer, header, hide, label,
    narration::Narration, ns, orb, orb_in, packet, plug, post, ring, send, show, sound, status,
};

const SESSION: [f32; 3] = [330.0, 580.0, 0.0];
const WORKSPACE: [f32; 3] = [830.0, 580.0, 0.0];
const SANDBOX: [f32; 3] = [1430.0, 580.0, 0.0];
const TEMPLATE: [f32; 3] = [1430.0, 270.0, 0.0];

fn stage() -> StagePlan {
    StagePlan {
        post: post(),
        elements: vec![
            orb("session", SESSION, 80.0, 500),
            label(
                "session-name",
                [SESSION[0], 690.0, 0.0],
                21.0,
                CaptionAlign::Center,
                &[("SessionDO", Tone::Plain)],
            ),
            label(
                "no-wait",
                [SESSION[0], 440.0, 0.0],
                19.0,
                CaptionAlign::Center,
                &[
                    ("text-only reply", Tone::Plain),
                    (" → no wait", Tone::Muted),
                ],
            ),
            card(
                "workspace",
                WORKSPACE,
                [300.0, 110.0],
                "WorkspaceDO",
                &[
                    ("no sandbox yet", Tone::Muted),
                    ("attached", Tone::Plain),
                    ("snapshot saved", Tone::Plain),
                    ("restoring", Tone::Plain),
                    ("attached", Tone::Success),
                ],
                Tone::Plain,
            ),
            card(
                "sandbox",
                SANDBOX,
                [400.0, 150.0],
                "modal sandbox",
                &[
                    ("vm · daemon", Tone::Muted),
                    ("running a command", Tone::Plain),
                    ("repo mounted", Tone::Success),
                    ("snapshotted", Tone::Plain),
                    ("shut down", Tone::Muted),
                    ("restored from snapshot", Tone::Success),
                ],
                Tone::Plain,
            ),
            card(
                "template",
                TEMPLATE,
                [340.0, 100.0],
                "template registry",
                &[
                    ("opencode repo checkout", Tone::Muted),
                    ("refreshing", Tone::Plain),
                    ("dependencies installed", Tone::Success),
                ],
                Tone::Plain,
            ),
            ring(
                "hourly",
                [TEMPLATE[0] - 225.0, TEMPLATE[1], 0.0],
                18.0,
                2.0,
                Tone::Plain,
            ),
            label(
                "hourly-name",
                [TEMPLATE[0] - 225.0, TEMPLATE[1] + 42.0, 0.0],
                18.0,
                CaptionAlign::Center,
                &[("hourly", Tone::Muted)],
            ),
            ring("idle", SANDBOX, 236.0, 2.0, Tone::Warning),
            label(
                "idle-name",
                [SANDBOX[0], 850.0, 0.0],
                20.0,
                CaptionAlign::Center,
                &[("20 idle minutes", Tone::Warning)],
            ),
            label(
                "daemon",
                [SANDBOX[0], 690.0, 0.0],
                19.0,
                CaptionAlign::Center,
                &[("process + file server", Tone::Muted)],
            ),
            beam("ws", "session", "workspace", Tone::Plain),
            beam("sb", "workspace", "sandbox", Tone::Plain),
            beam("tp", "template", "sandbox", Tone::Plain),
            packet("cmd", "ws", false, "shell · file edit", Tone::Request),
            packet("cmd-2", "sb", false, "", Tone::Request),
            packet("out-2", "sb", true, "", Tone::Success),
            packet("out", "ws", true, "output", Tone::Success),
            packet("mount", "tp", false, "mount", Tone::Success),
            packet("snapshot", "sb", true, "snapshot", Tone::Plain),
            packet("restore", "sb", false, "restore", Tone::Plain),
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
    orb_in(s, sc, "session", ns(0.3));
    show(s, sc, "session-name", ns(0.9));
    let object = v.at("workspace object");
    arrive(s, sc, "workspace", "ws", object - ns(0.2));
    let sandbox = v.at("modal sandbox");
    let contact = arrive(s, sc, "sandbox", "sb", sandbox - ns(0.2));
    status(s, sc, "workspace", contact, 1);
    s.to(sc, "camera.x", sandbox, 140.0, 1.6);
    s.type_in(sc, "daemon", v.at("small daemon"), 40.0);

    // A command crosses into the sandbox; its output comes back.
    let shell = v.at("shell commands");
    s.to(sc, "camera.x", shell, 40.0, 1.6);
    let at_workspace = send(s, sc, "cmd", shell, 0.75);
    s.land(sc, "workspace", at_workspace);
    let at_sandbox = send(s, sc, "cmd-2", at_workspace + ns(0.42), 0.7);
    s.land(sc, "sandbox", at_sandbox);
    status(s, sc, "sandbox", at_sandbox, 1);
    s.clock(sc, "sandbox.spinner", at_sandbox);
    let back = send(s, sc, "out-2", at_sandbox + ns(0.9), 0.7);
    s.set(sc, "sandbox.spinner", back, -1.0);
    status(s, sc, "sandbox", back, 0);
    let returned = send(s, sc, "out", back + ns(0.42), 0.75);
    s.hit(sc, "session.pulse", returned, 0.6, 0.0);
    s.type_in(sc, "no-wait", v.at("dont wait"), 40.0);

    // The template registry keeps a prepared checkout; cloning mounts it.
    let hour = v.at("every hour");
    s.to(sc, "camera.x", hour - ns(0.2), 90.0, 1.6);
    s.to(sc, "camera.y", hour - ns(0.2), -60.0, 1.6);
    s.settle_in(sc, "template", hour - ns(0.3));
    s.to(sc, "hourly.opacity", hour, 1.0, 0.3);
    s.ease(sc, "hourly.sweep", hour, 1.0, 1.2, Ease::Smootherstep);
    show(s, sc, "hourly-name", hour + ns(0.2));
    status(s, sc, "template", hour + ns(0.2), 1);
    s.clock(sc, "template.spinner", hour + ns(0.2));
    let refreshed = hour + ns(1.4);
    s.set(sc, "template.spinner", refreshed, -1.0);
    status(s, sc, "template", refreshed, 2);
    sc.media(sound("refreshed", MARK, refreshed, -18.0));
    let mount = v.at("not a download");
    let contact = plug(s, sc, "tp", mount - ns(1.3));
    let mounted = send(
        s,
        sc,
        "mount",
        (mount - ns(0.2)).max(contact + ns(0.3)),
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
    mount_footer.hide(sc, idle - ns(0.6));
    s.to(sc, "camera.x", idle - ns(0.6), 120.0, 1.6);
    s.to(sc, "camera.y", idle - ns(0.6), 30.0, 1.6);
    s.to(sc, "tp.opacity", idle - ns(0.6), 0.25, 0.6);
    s.to(sc, "template.dim", idle - ns(0.6), 0.6, 0.8);
    s.to(sc, "idle.opacity", idle - ns(0.5), 0.7, 0.3);
    s.ease(
        sc,
        "idle.sweep",
        idle - ns(0.5),
        1.0,
        1.4,
        Ease::Smootherstep,
    );
    s.type_in(sc, "idle-name", idle - ns(0.3), 40.0);
    let snapshotted = v.at("snapshotted").max(idle + ns(1.0));
    hide(s, sc, "idle", snapshotted);
    hide(s, sc, "idle-name", snapshotted + ns(0.2));
    status(s, sc, "sandbox", snapshotted, 3);
    s.hit(sc, "sandbox.flash", snapshotted, 0.5, 0.0);
    let saved = send(s, sc, "snapshot", snapshotted + ns(0.2), 0.7);
    s.land(sc, "workspace", saved);
    status(s, sc, "workspace", saved, 2);
    let down = saved + ns(0.5);
    status(s, sc, "sandbox", down, 4);
    s.to(sc, "sandbox.dim", down, 0.7, 0.6);
    s.to(sc, "sb.flow", down, 0.0, 0.3);
    s.to(sc, "sb.opacity", down, 0.3, 0.6);
    sc.media(sound("down", RESET, down, -16.0));

    // Restored from that snapshot when it is needed again.
    let restored = v.at("restored").max(down + ns(0.8));
    status(s, sc, "workspace", restored - ns(0.4), 3);
    let reached = send(s, sc, "restore", restored - ns(0.2), 0.7);
    s.to(sc, "sb.opacity", restored - ns(0.4), 1.0, 0.4);
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
