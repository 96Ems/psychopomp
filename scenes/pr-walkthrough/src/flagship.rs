//! #50825 as a Stage film. The service is a particle orb and its clients float
//! around it. In the broken story a reconnecting client's SIGTERM shatters the
//! orb and snaps every connection; the fix replays the same moment and nothing
//! breaks. The camera then flies into the client card, which opens into the code.
use std::path::PathBuf;

use anyhow::{Context, Result};
use kinograph::{
    author::PlanBuilder,
    caption::{CaptionAlign, CaptionSpanPlan},
    plan::{
        MediaKindPlan, MediaPlan, MediaRolePlan, ReelPlan, ReelSegmentPlan, ReelTransitionStyle,
        ScenePlan,
    },
    stage::{Camera, StageActor, StageElement, StagePlan, StagePost, StatusText},
    tone::Tone,
};

use crate::{PRS, chip, code, diffs, footer, header, narration::Narration, ns, span};

const CLIENT: [f32; 3] = [430.0, 420.0, -60.0];
const CLIENT_SIZE: [f32; 2] = [340.0, 124.0];
const SERVICE: [f32; 3] = [1060.0, 480.0, 0.0];
/// Where the camera settles before flying into the client card.
const CLOSING_CAMERA: [f32; 3] = [-110.0, 0.0, 60.0];

pub fn build_flagship(narration_dir: &std::path::Path) -> Result<ReelPlan> {
    let narration = Narration::load(narration_dir)?;
    let pr = &PRS[1];
    let stage = stage_film(&narration)?;
    let code = code(pr, &narration, diffs(1), false)?;
    let reel = ReelPlan {
        version: ReelPlan::VERSION,
        id: "pr-50825".to_owned(),
        segments: vec![
            ReelSegmentPlan {
                transition_nanos: 0,
                transition_style: ReelTransitionStyle::Dip,
                transition_focus: None,
                plan: stage,
            },
            ReelSegmentPlan {
                transition_nanos: ns(1.15),
                transition_style: ReelTransitionStyle::Zoom,
                transition_focus: Some(client_rect()),
                plan: code,
            },
        ],
    };
    reel.validate()?;
    Ok(reel)
}

/// The client card's rectangle on screen once the closing camera settles.
fn client_rect() -> [f32; 4] {
    let camera = Camera {
        x: CLOSING_CAMERA[0],
        y: CLOSING_CAMERA[1],
        z: CLOSING_CAMERA[2],
        width: 1920.0,
        height: 1080.0,
    };
    let (center, scale) = camera
        .project(CLIENT)
        .expect("client is in front of the camera");
    let size = [CLIENT_SIZE[0] * scale, CLIENT_SIZE[1] * scale];
    [
        center[0] - size[0] * 0.5,
        center[1] - size[1] * 0.5,
        size[0],
        size[1],
    ]
}

fn status(text: &str, tone: Tone) -> StatusText {
    StatusText {
        text: text.to_owned(),
        tone,
    }
}

fn spans(parts: &[(&str, Tone)]) -> Vec<CaptionSpanPlan> {
    parts.iter().map(|(text, tone)| span(text, *tone)).collect()
}

fn card(
    id: &str,
    at: [f32; 3],
    size: [f32; 2],
    title: &str,
    status: Vec<StatusText>,
    tone: Tone,
) -> StageElement {
    StageElement::Card {
        id: id.into(),
        at,
        size,
        title: title.into(),
        status,
        tone,
    }
}

fn beam(id: &str, from: &str, to: &str, bend: f32, tone: Tone) -> StageElement {
    StageElement::Beam {
        id: id.into(),
        from: from.into(),
        to: to.into(),
        bend,
        tone,
    }
}

fn packet(id: &str, reverse: bool, label: &str, tone: Tone) -> StageElement {
    StageElement::Packet {
        id: id.into(),
        beam: "link".into(),
        reverse,
        label: label.into(),
        tone,
    }
}

fn label(id: &str, at: [f32; 3], size: f32, parts: &[(&str, Tone)]) -> StageElement {
    StageElement::Label {
        id: id.into(),
        at,
        size,
        align: CaptionAlign::Center,
        spans: spans(parts),
    }
}

fn ring(id: &str, radius: f32, thickness: f32, tone: Tone) -> StageElement {
    StageElement::Ring {
        id: id.into(),
        at: SERVICE,
        radius,
        thickness,
        tone,
    }
}

const OTHERS: [(&str, &str, [f32; 3], f32); 3] = [
    ("tui-1", "b1", [1640.0, 250.0, 80.0], 60.0),
    ("desktop", "b2", [1700.0, 600.0, 30.0], -30.0),
    ("tui-2", "b3", [1450.0, 900.0, 120.0], -60.0),
];

fn stage_plan() -> StagePlan {
    let connected = || {
        vec![
            status("connected", Tone::Success),
            status("disconnected", Tone::Error),
        ]
    };
    let mut elements = vec![
        StageElement::Orb {
            id: "service".into(),
            at: SERVICE,
            radius: 150.0,
            points: 900,
            tone: Tone::Accent,
        },
        label(
            "service-name",
            [SERVICE[0], 690.0, 0.0],
            24.0,
            &[("opencode service", Tone::Plain)],
        ),
        label(
            "service-healthy",
            [SERVICE[0], 728.0, 0.0],
            19.0,
            &[("● healthy", Tone::Success)],
        ),
        label(
            "service-stopped",
            [SERVICE[0], 728.0, 0.0],
            19.0,
            &[("● stopped", Tone::Error)],
        ),
        card(
            "client",
            CLIENT,
            CLIENT_SIZE,
            "client",
            vec![
                status("connected", Tone::Success),
                status("reconnecting", Tone::Plain),
                status("replacing the server", Tone::Error),
                status("stopped with an error", Tone::Warning),
            ],
            Tone::Request,
        ),
        beam("link", "client", "service", -70.0, Tone::Request),
    ];
    for (id, link, at, bend) in OTHERS {
        let title = if id == "desktop" {
            "desktop app"
        } else {
            "TUI"
        };
        elements.push(card(
            id,
            at,
            [290.0, 110.0],
            title,
            connected(),
            Tone::Plain,
        ));
        elements.push(beam(link, id, "service", bend, Tone::Success));
    }
    elements.extend([
        packet("probe", false, "GET /api/info", Tone::Request),
        packet("reply", true, "404", Tone::Error),
        packet("kill", false, "SIGTERM", Tone::Error),
        packet("probe-2", false, "GET /api/info", Tone::Request),
        packet("reply-2", true, "404", Tone::Warning),
        label(
            "thought-before",
            [CLIENT[0], 560.0, CLIENT[2]],
            21.0,
            &[
                ("404", Tone::Error),
                (" → outdated → ", Tone::Plain),
                ("replace it", Tone::Error),
            ],
        ),
        label(
            "thought-after",
            [CLIENT[0], 560.0, CLIENT[2]],
            21.0,
            &[
                ("version ok", Tone::Success),
                (" → ", Tone::Plain),
                ("protocol mismatch", Tone::Warning),
            ],
        ),
        label(
            "message",
            [CLIENT[0], 604.0, CLIENT[2]],
            19.0,
            &[
                ("error: ", Tone::Warning),
                ("update this client, or restart explicitly", Tone::Plain),
            ],
        ),
        ring("shock", 160.0, 3.0, Tone::Error),
        ring("safe", 205.0, 2.5, Tone::Success),
    ]);
    StagePlan {
        post: StagePost {
            bloom: 0.45,
            grain: 0.035,
            vignette: 0.42,
            backdrop: 0.4,
        },
        elements,
    }
}

fn sound(id: &str, file: &str, seconds: f64, at: u64, gain_db: f32) -> MediaPlan {
    let length = ns(seconds);
    MediaPlan {
        id: id.to_owned(),
        path: PathBuf::from(format!("../../assets/{file}")),
        kind: MediaKindPlan::Audio,
        role: MediaRolePlan::Layer,
        source_start_nanos: 0,
        source_end_nanos: length,
        timeline_start_nanos: at,
        timeline_end_nanos: at + length,
        gain_db,
    }
}

fn stage_film(narration: &Narration) -> Result<ScenePlan> {
    let pr = &PRS[1];
    let before_clip = narration.clip("mismatch-before")?;
    let after_clip = narration.clip("mismatch-after")?;
    let lead = ns(1.6);
    let rewind = ns(2.4);
    let duration = lead + before_clip.duration() + rewind + after_clip.duration() + ns(2.4);
    let mut scene = PlanBuilder::new("mismatch-stage", duration);
    let before = before_clip.place(&mut scene, lead);
    let after = after_clip.place(&mut scene, before.end() + rewind);
    let b = |phrase: &str| before.at(phrase);
    let a = |phrase: &str| after.at(phrase);
    let mut stage = StageActor::declare(&mut scene, "stage", &stage_plan())?;
    let s = &mut stage;
    let sc = &mut scene;

    // Entrance: dolly in; the orb swells into place; cards pop in; beams draw, then flow.
    s.to(sc, "camera.z", -320.0, 0, 0.0, 2.6);
    s.set(sc, "camera.dof", 0.9, 0, 0.9);
    s.bounce(sc, "service.scale", 0.55, ns(0.15), 1.0, 1.0, 0.25);
    s.to(sc, "service.opacity", 0.0, ns(0.15), 1.0, 0.6);
    for (index, name) in ["service-name", "service-healthy"].iter().enumerate() {
        s.to(
            sc,
            &format!("{name}.opacity"),
            0.0,
            ns(0.9 + index as f64 * 0.15),
            1.0,
            0.5,
        );
    }
    s.to(sc, "service-stopped.opacity", 0.0, 0, 0.0, 0.1);
    let cards = ["client", "tui-1", "desktop", "tui-2"];
    let links = ["link", "b1", "b2", "b3"];
    for (index, (card, link)) in cards.iter().zip(links).enumerate() {
        let at = ns(0.45 + index as f64 * 0.14);
        s.to(sc, &format!("{card}.opacity"), 0.0, at, 1.0, 0.45);
        s.bounce(sc, &format!("{card}.scale"), 0.9, at, 1.0, 0.7, 0.28);
        s.to(sc, &format!("{link}.draw"), 0.0, at + ns(0.2), 1.0, 0.8);
        s.to(sc, &format!("{link}.flow"), 0.0, at + ns(1.0), 1.0, 0.6);
    }
    for hidden in ["thought-before", "thought-after", "message"] {
        s.set(sc, &format!("{hidden}.opacity"), 0.0, 0, 0.0);
    }
    for hidden in ["shock", "safe"] {
        s.set(sc, &format!("{hidden}.opacity"), 0.0, 0, 0.0);
    }
    s.set(sc, "safe.sweep", 0.0, 0, 0.0);
    header(sc, pr, Some(ns(0.4)))?;
    let mut before_chip = chip(sc, "chip-before", Tone::Error, "before")?;
    before_chip.show(sc, ns(0.7));

    // A client reconnects and asks for the health endpoint.
    let reconnect = b("when a client reconnects");
    s.to(sc, "client.status", 0.0, reconnect, 1.0, 0.4);
    s.to(sc, "client.glow", 0.0, reconnect, 0.7, 0.5);
    s.to(sc, "link.flow", 0.0, reconnect, 0.0, 0.4);
    s.to(sc, "link.emphasis", 0.0, reconnect, 1.0, 0.6);
    s.to(sc, "camera.x", 0.0, reconnect, -110.0, 1.4);
    s.to(sc, "camera.focus", 0.0, reconnect, -60.0, 1.2);
    let probe_arrival = s.send(sc, "probe", b("health endpoint"), 0.95);
    s.hit(sc, "service.pulse", probe_arrival, 1.0, 0.0);
    sc.media(sound(
        "probe-send",
        "opencode-hot-reload/save.wav",
        0.15,
        b("health endpoint"),
        -8.0,
    ));

    // The server answers 404; the old rule reads that as "outdated".
    let reply_arrival = s.send(sc, "reply", b("returns a 404"), 0.8);
    s.hit(sc, "client.alarm", reply_arrival, 1.0, 0.0);
    s.hit(sc, "post.chroma", reply_arrival, 0.7, 0.0);
    sc.media(sound(
        "reply-land",
        "visual-effects/task-failure.wav",
        0.47,
        reply_arrival,
        -9.0,
    ));
    let outdated = b("assumed the server was outdated");
    s.type_in(sc, "thought-before", outdated, 42.0);
    s.to(sc, "client.status", 0.0, outdated + ns(1.0), 2.0, 0.4);

    // SIGTERM: the orb shatters, the shockwave spreads, every connection snaps.
    let kill_send = b("sig term");
    let kill_arrival = s.send(sc, "kill", kill_send, 0.55);
    sc.media(sound(
        "kill-send",
        "opencode-hot-reload/launch.wav",
        1.36,
        kill_send - ns(0.35),
        -15.0,
    ));
    sc.media(sound(
        "kill-impact",
        "opencode-hot-reload/impact.wav",
        0.51,
        kill_arrival,
        -5.0,
    ));
    sc.media(sound(
        "shatter",
        "visual-effects/task-death.wav",
        1.09,
        kill_arrival + ns(0.05),
        -7.0,
    ));
    s.to(sc, "service.shatter", 0.0, kill_arrival, 1.0, 1.7);
    s.to(sc, "service.hurt", 0.0, kill_arrival, 1.0, 0.2);
    s.set(sc, "shock.opacity", 0.0, kill_arrival, 1.0);
    s.set(sc, "shock.expand", 0.0, kill_arrival, 0.0);
    s.to(sc, "shock.expand", 0.0, kill_arrival, 1.0, 1.0);
    s.hit(sc, "post.chroma", kill_arrival, 2.0, 0.0);
    s.hit(sc, "camera.shake", kill_arrival, 14.0, 0.0);
    s.hit(sc, "post.bloom", kill_arrival, 1.4, 0.45);
    s.to(sc, "camera.focus", 0.0, kill_arrival, 0.0, 0.8);
    s.to(
        sc,
        "service-healthy.opacity",
        0.0,
        kill_arrival + ns(0.2),
        0.0,
        0.3,
    );
    s.to(
        sc,
        "service-stopped.opacity",
        0.0,
        kill_arrival + ns(0.3),
        1.0,
        0.3,
    );
    s.to(sc, "link.break", 0.0, kill_arrival + ns(0.15), 1.0, 0.9);
    for (index, (card, link, _, _)) in OTHERS.iter().enumerate() {
        let at = kill_arrival + ns(0.3 + index as f64 * 0.14);
        s.to(sc, &format!("{link}.flow"), 0.0, kill_arrival, 0.0, 0.25);
        s.to(sc, &format!("{link}.break"), 0.0, at, 1.0, 0.9);
        s.hit(sc, &format!("{card}.alarm"), at + ns(0.2), 1.0, 0.0);
        s.to(sc, &format!("{card}.status"), 0.0, at + ns(0.25), 1.0, 0.4);
        s.to(sc, &format!("{card}.dim"), 0.0, at + ns(0.6), 0.55, 0.8);
    }
    let cut = b("cut off every other client");
    s.to(sc, "camera.x", 0.0, cut - ns(0.4), 0.0, 1.6);
    s.to(sc, "camera.z", -320.0, cut - ns(0.4), -150.0, 1.8);
    let mut footer_before = footer(
        sc,
        "footer-before",
        vec![
            span("a 404 meant: ", Tone::Plain),
            span("kill the server", Tone::Error),
        ],
    )?;
    footer_before.type_in(sc, cut, 42.0, 0.8);

    // Rewind: everything returns to the moment before the probe.
    let switch = before.end() + ns(0.5);
    before_chip.hide(sc, switch);
    footer_before.hide(sc, switch);
    let mut rewind_chip = chip(sc, "chip-rewind", Tone::Accent, "◀◀ rewind")?;
    rewind_chip.show(sc, switch);
    rewind_chip.hide(sc, switch + ns(1.5));
    let mut after_chip = chip(sc, "chip-after", Tone::Success, "after the fix")?;
    after_chip.show(sc, switch + ns(1.65));
    sc.media(sound(
        "rewind",
        "opencode-hot-reload/launch.wav",
        1.36,
        switch - ns(0.1),
        -13.0,
    ));
    s.to(sc, "service.shatter", 0.0, switch + ns(0.1), 0.0, 1.3);
    s.to(sc, "service.hurt", 0.0, switch + ns(0.6), 0.0, 0.6);
    s.to(sc, "shock.opacity", 0.0, switch, 0.0, 0.3);
    s.set(sc, "post.chroma", 0.0, switch, 1.6);
    s.to(sc, "post.chroma", 0.0, switch + ns(0.2), 0.0, 1.2);
    s.to(sc, "camera.z", -320.0, switch, 0.0, 1.8);
    s.to(sc, "thought-before.opacity", 0.0, switch, 0.0, 0.4);
    s.to(
        sc,
        "service-stopped.opacity",
        0.0,
        switch + ns(0.9),
        0.0,
        0.3,
    );
    s.to(
        sc,
        "service-healthy.opacity",
        0.0,
        switch + ns(1.0),
        1.0,
        0.3,
    );
    s.to(sc, "client.status", 0.0, switch + ns(0.6), 1.0, 0.3);
    s.to(sc, "link.break", 0.0, switch + ns(0.5), 0.0, 0.9);
    for (index, (card, link, _, _)) in OTHERS.iter().enumerate() {
        let at = switch + ns(0.35 + index as f64 * 0.1);
        s.to(sc, &format!("{link}.break"), 0.0, at, 0.0, 0.9);
        s.to(sc, &format!("{link}.flow"), 0.0, at + ns(1.0), 1.0, 0.6);
        s.to(sc, &format!("{card}.status"), 0.0, at, 0.0, 0.4);
        s.to(sc, &format!("{card}.dim"), 0.0, at, 0.0, 0.6);
    }

    // The fix: the same 404, understood as a protocol mismatch.
    let now = a("now a 404");
    s.to(sc, "camera.x", 0.0, now - ns(0.4), -110.0, 1.2);
    s.to(sc, "camera.focus", 0.0, now - ns(0.4), -60.0, 1.0);
    let probe_2 = s.send(sc, "probe-2", now - ns(0.1), 0.75);
    sc.media(sound(
        "probe-send-2",
        "opencode-hot-reload/save.wav",
        0.15,
        now - ns(0.1),
        -8.0,
    ));
    s.hit(sc, "service.pulse", probe_2, 1.0, 0.0);
    let reply_2 = s.send(sc, "reply-2", probe_2 + ns(0.05), 0.7);
    s.hit(sc, "client.flash", reply_2, 0.8, 0.0);
    s.type_in(sc, "thought-after", a("health protocols"), 44.0);
    let message = a("clear message");
    s.type_in(sc, "message", message, 52.0);
    s.to(sc, "client.status", 0.0, message, 3.0, 0.4);
    sc.media(sound(
        "message",
        "visual-effects/task-reset.wav",
        0.33,
        message,
        -12.0,
    ));

    // Nothing gets killed: the camera finds the orb, whole and breathing.
    let safe = a("nothing gets killed");
    s.to(sc, "camera.x", 0.0, safe - ns(0.2), 40.0, 1.6);
    s.to(sc, "camera.z", 0.0, safe - ns(0.2), 170.0, 1.8);
    s.to(sc, "camera.focus", 0.0, safe - ns(0.2), 0.0, 1.0);
    s.hit(sc, "service.pulse", safe + ns(0.2), 1.4, 0.0);
    s.set(sc, "safe.opacity", 0.0, safe, 1.0);
    s.to(sc, "safe.sweep", 0.0, safe, 1.0, 1.3);
    for (_, link, _, _) in OTHERS {
        s.to(sc, &format!("{link}.flow"), 0.0, safe, 1.4, 0.8);
    }
    sc.media(sound(
        "safe",
        "effect-shows-errors/prismatic-bloom.wav",
        0.785,
        safe + ns(0.15),
        -9.0,
    ));
    let mut footer_after = footer(
        sc,
        "footer-after",
        vec![
            span("a mismatch is an ", Tone::Plain),
            span("error", Tone::Warning),
            span(", never a kill", Tone::Plain),
        ],
    )?;
    footer_after.type_in(sc, safe, 42.0, 0.8);

    // Return to the client card; the reel's zoom flies into its code.
    let close = after.end() + ns(0.3);
    s.to(sc, "camera.x", 0.0, close, CLOSING_CAMERA[0], 1.2);
    s.to(sc, "camera.y", 0.0, close, CLOSING_CAMERA[1], 1.2);
    s.to(sc, "camera.z", 0.0, close, CLOSING_CAMERA[2], 1.2);
    s.to(sc, "camera.focus", 0.0, close, -60.0, 1.0);
    s.to(sc, "client.glow", 0.0, close, 1.0, 0.8);
    after_chip.hide(sc, close);
    footer_after.hide(sc, close);

    scene.finish().context("mismatch-stage")
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_zoom_rectangle_matches_the_client_card() {
        let rect = super::client_rect();
        assert!(
            rect[2] > super::CLIENT_SIZE[0],
            "the closing camera magnifies the card"
        );
        assert!(rect[0] > 0.0 && rect[1] > 0.0 && rect[0] + rect[2] < 1920.0);
    }
}
