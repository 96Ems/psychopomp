//! #50825 as a Stage film. The service is a particle orb and its clients float
//! around it. In the broken story a reconnecting client's SIGTERM shatters the
//! orb and snaps every connection; the fix replays the same moment and nothing
//! breaks. The camera then flies into the client card, which opens into the code.
use std::path::PathBuf;

use anyhow::{Context, Result};
use kinograph::{
    author::PlanBuilder,
    caption::{CaptionAlign, CaptionSpanPlan},
    math::{Vec2, Vec3, easing::Ease, vec2},
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
/// Left edge of the notes under the client card, so they grow away from the orb.
const NOTE_X: f32 = CLIENT[0] - CLIENT_SIZE[0] * 0.5 + 4.0;
/// Where the camera settles before flying into the client card.
const CLOSING_CAMERA: [f32; 3] = [-110.0, 0.0, 60.0];

pub fn build_flagship(narration_dir: &std::path::Path) -> Result<ReelPlan> {
    let narration = Narration::load(narration_dir)?;
    let pr = &PRS[1];
    let stage = stage_film(&narration)?;
    let code = stable_code(code(pr, &narration, diffs(1), false)?)?;
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

/// Keep the compatible declaration and version-check expression alive while
/// splitting the computation. Their lines trade places; only edited slots fade.
fn stable_code(mut plan: ScenePlan) -> Result<ScenePlan> {
    use kinograph::{
        editor::{
            EditorInlineRevealPlan, EditorPartPlan, EditorRecipePlan, EditorSemanticRangePlan,
            LineMarkPlan,
        },
        highlight,
        plan::{SpringPlan, destination_channel},
    };
    let actor = plan
        .actors
        .iter_mut()
        .find(|actor| actor.id == "editor")
        .context("flagship editor")?;
    let mut recipe: EditorRecipePlan = serde_json::from_value(actor.data.clone())?;
    let at = recipe
        .snapshots
        .first()
        .context("version split snapshot")?
        .at_nanos;
    let part = |id: &str, text: &str| EditorPartPlan {
        id: id.into(),
        spans: highlight::typescript(text),
    };
    recipe
        .lines
        .retain(|line| line.id != "line-3" && line.id != "line-4");
    for line in &mut recipe.lines {
        if line.id == "line-1" {
            line.parts = vec![
                part("declaration", "  const compatible ="),
                part("rhs", " service.compatible && versionMatches"),
            ];
            line.mark = Some(LineMarkPlan::Added);
        }
        if line.id == "line-2" {
            line.parts = vec![
                part("indent", "  "),
                part("old-prefix", "  service.compatible && "),
                part("new-prefix", "const versionMatches = "),
                part("expression", "matchesVersion(service.version, options)"),
            ];
            line.mark = Some(LineMarkPlan::Added);
        }
    }
    for (line_id, part_id, reversed) in [
        ("line-1", "rhs", false),
        ("line-2", "old-prefix", true),
        ("line-2", "new-prefix", false),
    ] {
        let line = recipe
            .lines
            .iter_mut()
            .find(|line| line.id == line_id)
            .context("stable split line")?;
        line.semantic_ranges.push(EditorSemanticRangePlan {
            id: part_id.into(),
            first_part_id: part_id.into(),
            last_part_id: part_id.into(),
        });
        recipe
            .additional_inline_reveals
            .push(EditorInlineRevealPlan {
                line_id: line_id.into(),
                range_id: part_id.into(),
                channel: Some("version-split".into()),
                reversed,
            });
    }
    for order in recipe
        .snapshots
        .iter_mut()
        .map(|snapshot| &mut snapshot.line_ids)
        .chain(std::iter::once(&mut recipe.final_line_ids))
    {
        for id in order {
            if id == "line-3" {
                *id = "line-2".into();
            } else if id == "line-4" {
                *id = "line-1".into();
            }
        }
    }
    recipe.compile()?;
    actor.data = serde_json::to_value(recipe)?;
    plan.continuous_channels
        .retain(|channel| channel.id != "editor.mark.line-1" && channel.id != "editor.mark.line-2");
    for property in ["version-split", "mark.line-1", "mark.line-2"] {
        plan.continuous_channels.push(destination_channel(
            "editor",
            property,
            0.0,
            [(at, 1.0)],
            |_, _| SpringPlan::visual(0.4, 0.0),
        ));
    }
    Ok(plan)
}

/// The client card's rectangle on screen once the closing camera settles.
fn client_rect() -> [f32; 4] {
    let camera = Camera {
        position: Vec3::from(CLOSING_CAMERA),
        size: vec2(1920.0, 1080.0),
    };
    let (center, scale) = camera
        .project(Vec3::from(CLIENT))
        .expect("client is in front of the camera");
    let size = Vec2::from(CLIENT_SIZE) * scale;
    let corner = center - size * 0.5;
    [corner.x, corner.y, size.x, size.y]
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

fn beam(id: &str, from: &str, to: &str, tone: Tone) -> StageElement {
    StageElement::Beam {
        id: id.into(),
        from: from.into(),
        to: to.into(),
        bend: 0.0,
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

fn label(
    id: &str,
    at: [f32; 3],
    size: f32,
    align: CaptionAlign,
    parts: &[(&str, Tone)],
) -> StageElement {
    StageElement::Label {
        id: id.into(),
        at,
        size,
        align,
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

const OTHERS: [(&str, &str, [f32; 3]); 3] = [
    ("tui-1", "b1", [1490.0, 260.0, 80.0]),
    ("desktop", "b2", [1520.0, 570.0, 30.0]),
    ("tui-2", "b3", [1400.0, 830.0, 120.0]),
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
            [SERVICE[0], 659.0, 0.0],
            24.0,
            CaptionAlign::Center,
            &[("opencode service", Tone::Plain)],
        ),
        label(
            "service-healthy",
            [SERVICE[0], 691.0, 0.0],
            19.0,
            CaptionAlign::Center,
            &[("● healthy", Tone::Success)],
        ),
        label(
            "service-stopped",
            [SERVICE[0], 691.0, 0.0],
            19.0,
            CaptionAlign::Center,
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
        beam("link", "client", "service", Tone::Request),
    ];
    for (id, link, at) in OTHERS {
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
        elements.push(beam(link, id, "service", Tone::Success));
    }
    elements.extend([
        packet("probe", false, "GET /api/info", Tone::Request),
        packet("reply", true, "404", Tone::Error),
        packet("kill", false, "SIGTERM", Tone::Error),
        packet("probe-2", false, "GET /api/info", Tone::Request),
        packet("reply-2", true, "404", Tone::Warning),
        label(
            "thought-before",
            [NOTE_X, 560.0, CLIENT[2]],
            21.0,
            CaptionAlign::Left,
            &[
                ("404", Tone::Error),
                (" → outdated → ", Tone::Plain),
                ("replace it", Tone::Error),
            ],
        ),
        label(
            "thought-after",
            [NOTE_X, 560.0, CLIENT[2]],
            21.0,
            CaptionAlign::Left,
            &[
                ("version ok", Tone::Success),
                (" → ", Tone::Plain),
                ("protocol mismatch", Tone::Warning),
            ],
        ),
        label(
            "message",
            [NOTE_X, 604.0, CLIENT[2]],
            19.0,
            CaptionAlign::Left,
            &[
                ("error: ", Tone::Warning),
                ("update this client, or restart explicitly", Tone::Plain),
            ],
        ),
        ring("shock", 160.0, 1.8, Tone::Error),
        ring("safe", 162.0, 1.4, Tone::Success),
    ]);
    StagePlan {
        post: StagePost {
            bloom: 0.18,
            grain: 0.012,
            vignette: 0.22,
            backdrop: 0.24,
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

    // Establish the service, then its clients. Rigid panels drift into place;
    // their ink follows. The camera carries the composition without bouncing.
    s.to(sc, "camera.z", -160.0, 0, 0.0, 2.2);
    s.set(sc, "camera.dof", 0.45, 0, 0.45);
    s.bounce(sc, "service.scale", 0.58, ns(0.15), 1.0, 0.85, 0.2);
    s.to(sc, "service.blur", 11.0, ns(0.15), 0.0, 0.7);
    let rotation = s.channel(sc, "service.rotation", -1.8);
    sc.ease(&rotation, ns(0.15), 0.0, 1.25, Ease::CubicOut);
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
        // Each client settles in, then plugs into the service with a soft tick.
        let landing = s.settle_in(sc, card, ns(0.48 + index as f64 * 0.16));
        let contact = s.connect(sc, link, landing + ns(0.16), 0.68);
        // One brief proof of connection, then stillness gives the narrator room.
        s.to(
            sc,
            &format!("{link}.flow"),
            0.0,
            contact + ns(0.85),
            0.0,
            0.45,
        );
        sc.media(sound(
            &format!("connect-{index}"),
            "visual-effects/task-running.wav",
            0.13,
            contact,
            -20.0,
        ));
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
    s.to(sc, "client.glow", 0.0, reconnect, 0.45, 0.5);
    s.to(sc, "link.flow", 0.0, reconnect, 0.0, 0.4);
    s.to(sc, "link.emphasis", 0.0, reconnect, 1.0, 0.6);
    s.to(sc, "camera.x", 0.0, reconnect, -110.0, 1.6);
    s.to(sc, "camera.focus", 0.0, reconnect, -60.0, 1.2);
    s.send(sc, "probe", b("health endpoint"), 0.95);
    sc.media(sound(
        "probe-send",
        "opencode-hot-reload/save.wav",
        0.15,
        b("health endpoint"),
        -8.0,
    ));

    // The server answers 404; the old rule reads that as "outdated".
    let reply_arrival = s.send(sc, "reply", b("returns a 404"), 0.8);
    // The socket takes the red; the whole frame stays steady and legible.
    s.hit(sc, "client.alarm", reply_arrival, 0.18, 0.0);
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
    let kill_send = before.at_any(&["sig term", "sigterm"]);
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
    // Decisive impact followed by a long release. A spring from rest made the
    // explosion accelerate late, as though the service chose to fall apart.
    let shatter = s.channel(sc, "service.shatter", 0.0);
    sc.ease(&shatter, kill_arrival, 1.0, 1.65, Ease::CubicOut);
    let burst = s.channel(sc, "service.burst", -1.0);
    sc.set(&burst, kill_arrival, 0.0);
    sc.ease(&burst, kill_arrival, 5.2, 5.2, Ease::Linear);
    s.to(sc, "service.hurt", 0.0, kill_arrival, 1.0, 0.2);
    s.set(sc, "shock.opacity", 0.0, kill_arrival, 0.0);
    s.set(sc, "shock.expand", 0.0, kill_arrival, 0.0);
    s.to(sc, "shock.expand", 0.0, kill_arrival, 1.0, 1.0);
    s.hit(sc, "post.chroma", kill_arrival, 0.16, 0.0);
    s.hit(sc, "camera.shake", kill_arrival, 3.0, 0.0);
    s.hit(sc, "post.bloom", kill_arrival, 0.4, 0.18);
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
    for (index, (card, link, _)) in OTHERS.iter().enumerate() {
        let at = kill_arrival + ns(0.3 + index as f64 * 0.14);
        s.to(sc, &format!("{link}.flow"), 0.0, kill_arrival, 0.0, 0.25);
        s.to(sc, &format!("{link}.break"), 0.0, at, 1.0, 0.9);
        s.hit(sc, &format!("{card}.alarm"), at + ns(0.2), 0.35, 0.0);
        s.to(sc, &format!("{card}.status"), 0.0, at + ns(0.25), 1.0, 0.4);
        s.to(sc, &format!("{card}.dim"), 0.0, at + ns(0.6), 0.55, 0.8);
    }
    let cut = b("cut off every other client");
    s.to(sc, "camera.x", 0.0, cut - ns(0.4), 0.0, 1.6);
    s.to(sc, "camera.z", -160.0, cut - ns(0.4), -100.0, 1.8);
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
    let scan = s.channel(sc, "post.rewind", -1.0);
    sc.set(&scan, switch, 0.0);
    sc.ease(&scan, switch, 1.4, 1.4, Ease::Linear);
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
    s.glide(sc, "service.shatter", 0.0, switch + ns(0.1), 0.0, 1.3);
    sc.ease(&burst, switch + ns(0.1), 0.0, 1.3, Ease::Smootherstep);
    sc.set(&burst, switch + ns(1.4), -1.0);
    s.to(sc, "service.hurt", 0.0, switch + ns(0.6), 0.0, 0.6);
    s.to(sc, "shock.opacity", 0.0, switch, 0.0, 0.3);
    s.hit(sc, "post.chroma", switch, 0.12, 0.0);
    s.to(sc, "camera.z", -160.0, switch, 0.0, 1.8);
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
    for (index, (card, link, _)) in OTHERS.iter().enumerate() {
        let at = switch + ns(0.35 + index as f64 * 0.1);
        s.to(sc, &format!("{link}.break"), 0.0, at, 0.0, 0.9);
        // The halves meet: the line surges and snaps taut again.
        s.hit(sc, &format!("{link}.surge"), at + ns(0.7), 0.35, 0.0);
        s.twang(sc, link, at + ns(0.7));
        s.to(sc, &format!("{link}.flow"), 0.0, at + ns(1.0), 0.3, 0.4);
        s.to(sc, &format!("{link}.flow"), 0.0, at + ns(1.7), 0.0, 0.4);
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
    // `send` includes pre-launch gathering: even that preparation must wait
    // until the request has arrived (340 ms gather plus an 80 ms response beat).
    let reply_2 = s.send(sc, "reply-2", probe_2 + ns(0.42), 0.7);
    s.hit(sc, "client.flash", reply_2, 0.3, 0.0);
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
    s.to(sc, "camera.z", 0.0, safe - ns(0.2), 90.0, 1.8);
    s.to(sc, "camera.focus", 0.0, safe - ns(0.2), 0.0, 1.0);
    s.hit(sc, "service.pulse", safe + ns(0.2), 0.75, 0.0);
    s.to(sc, "safe.opacity", 0.0, safe, 0.7, 0.22);
    s.to(sc, "safe.sweep", 0.0, safe, 1.0, 1.3);
    for (_, link, _) in OTHERS {
        s.to(sc, &format!("{link}.flow"), 0.0, safe, 0.45, 0.5);
        s.to(sc, &format!("{link}.flow"), 0.0, safe + ns(1.3), 0.0, 0.5);
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
    s.to(sc, "client.glow", 0.0, close, 0.65, 0.8);
    after_chip.hide(sc, close);
    footer_after.hide(sc, close);

    scene.finish().context("mismatch-stage")
}

#[cfg(test)]
mod tests {
    #[test]
    fn client_cards_fit_the_camera_compositions() {
        use kinograph::math::{Vec3, vec2};
        use kinograph::stage::{Camera, StageElement};

        for position in [
            [0.0, 0.0, -160.0],
            [0.0, 0.0, 0.0],
            [-110.0, 0.0, 0.0],
            [40.0, 0.0, 90.0],
            super::CLOSING_CAMERA,
        ] {
            let camera = Camera {
                position: Vec3::from(position),
                size: vec2(1920.0, 1080.0),
            };
            for element in super::stage_plan().elements {
                if let StageElement::Card { id, at, size, .. } = element {
                    let (center, scale) = camera.project(Vec3::from(at)).unwrap();
                    let half = vec2(size[0], size[1]) * (0.5 * scale);
                    let low = center - half;
                    let high = center + half;
                    assert!(
                        low.x > 80.0 && high.x < 1840.0 && low.y > 160.0 && high.y < 970.0,
                        "{id} clips the composition at camera {position:?}: {low:?}..{high:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_version_split_preserves_common_parts_and_has_no_stability_warnings() {
        use kinograph::{author::PlanBuilder, editor::inspect_steps, plan::PresentationStepPlan};
        let mut builder = PlanBuilder::new("stable-split", super::ns(8.0));
        crate::diffs(1)
            .0
            .declare(
                &mut builder,
                &[super::ns(2.0), super::ns(5.0)],
                super::ns(0.8),
                false,
            )
            .unwrap();
        let mut plan = super::stable_code(builder.finish().unwrap()).unwrap();
        plan.presentation_steps = [(0.0, 1.0), (2.0, 4.0), (5.0, 7.0)]
            .into_iter()
            .enumerate()
            .map(|(i, (start, hold))| PresentationStepPlan {
                id: format!("step-{i}"),
                title: format!("Step {i}"),
                start_nanos: super::ns(start),
                hold_nanos: super::ns(hold),
            })
            .collect();
        let inspection = serde_json::to_value(inspect_steps(&plan).unwrap()).unwrap();
        assert_eq!(inspection["warnings"], serde_json::json!([]));
        for line in inspection["steps"][1]["editors"][0]["lines"]
            .as_array()
            .unwrap()
        {
            let changed = line["changedPartIds"].as_array().unwrap();
            assert!(
                !changed
                    .iter()
                    .any(|part| part == "declaration" || part == "expression" || part == "indent"),
                "common text must retain identity"
            );
        }
    }

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
