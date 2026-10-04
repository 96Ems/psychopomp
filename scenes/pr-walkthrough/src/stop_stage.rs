//! #50042 as a narrated Stage + Code explainer reel (`pr-50042.reel.json`).
//!
//! Uses ElevenLabs v4 narration in Kit's voice, Stage choreography (orb, cards,
//! beams, packets, combustion, rewind, camera dolly), a Stage Callout that
//! glides from `service.json` to the lingering server orb, a RollingNumber
//! grace-period timer (`0.0 s` → `5.0 s` → `10.0 s`), and a Zoom transition
//! into the Stepped Diff with an Editor Callout pinned to `SIGKILL`.
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use psychopomp::{
    author::{PlanBuilder, seconds},
    callout::{CalloutActor, CalloutAnchorPlan, CalloutPlan, CalloutSide},
    caption::{CaptionAlign, CaptionSpanPlan},
    code::{StyledSpan, SyntaxStyle},
    editor::{
        EditorPartPlan, EditorRecipePlan, EditorSemanticRangePlan, EditorTargetSelector,
        LineMarkPlan,
    },
    effects::{
        combustion,
        spinner::{self, Mark},
    },
    highlight,
    math::{Vec2, Vec3, easing::Ease, vec2},
    narration::Narration,
    plan::{
        MediaKindPlan, MediaPlan, MediaRolePlan, ReelPlan, ReelSegmentPlan, ReelTransitionStyle,
        ScenePlan,
    },
    rolling::{RollingNumberActor, RollingNumberPlan},
    stage::{Camera, StageActor, StageElement, StagePlan, StagePost, StatusText},
    tone::Tone,
};

use crate::{
    PRS, diffs,
    film::{chip, code, footer, header, span},
};

const CLIENT: [f32; 3] = [390.0, 540.0, -40.0];
const CLIENT_SIZE: [f32; 2] = [330.0, 122.0];
const OLD_SERVICE: [f32; 3] = [960.0, 540.0, 0.0];
const REG_FILE: [f32; 3] = [960.0, 215.0, -20.0];
const REG_SIZE: [f32; 2] = [270.0, 96.0];
const NEW_SERVICE: [f32; 3] = [1530.0, 540.0, -40.0];
const NEW_SIZE: [f32; 2] = [320.0, 122.0];
const CLOSING_CAMERA: [f32; 3] = [-130.0, 15.0, 60.0];

struct Sfx(&'static str, f64);

const TASK_RUNNING: Sfx = Sfx("visual-effects/task-running.wav", 0.13);
const SAVE: Sfx = Sfx("opencode-hot-reload/save.wav", 0.15);
const TASK_FAILURE: Sfx = Sfx("visual-effects/task-failure.wav", 0.47);
const LAUNCH: Sfx = Sfx("opencode-hot-reload/launch.wav", 1.36);
const IMPACT: Sfx = Sfx("opencode-hot-reload/impact.wav", 0.51);
const TASK_DEATH: Sfx = Sfx("visual-effects/task-death.wav", 1.09);
const GLITCH: Sfx = Sfx("pr-walkthrough/glitch.wav", 0.2);
const MARK: Sfx = Sfx("pr-walkthrough/mark.wav", 0.3);
const PRISMATIC_BLOOM: Sfx = Sfx("effect-shows-errors/prismatic-bloom.wav", 0.785);

fn sound(id: &str, Sfx(file, length): Sfx, at: u64, gain_db: f32) -> MediaPlan {
    let length = seconds(length);
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

pub fn build_stop_reel(narration_dir: &Path) -> Result<ReelPlan> {
    let narration = Narration::load(narration_dir)?;
    let stage = stage_film(&narration)?;
    let code = stop_code(&narration)?;
    let reel = ReelPlan {
        version: ReelPlan::VERSION,
        id: "pr-50042".to_owned(),
        segments: vec![
            ReelSegmentPlan {
                transition_nanos: 0,
                transition_style: ReelTransitionStyle::Dip,
                transition_focus: None,
                transition_wipe: None,
                plan: stage,
            },
            ReelSegmentPlan {
                transition_nanos: seconds(1.15),
                transition_style: ReelTransitionStyle::Zoom,
                transition_focus: Some(client_rect()),
                transition_wipe: None,
                plan: code,
            },
        ],
    };
    reel.validate()?;
    Ok(reel)
}

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
    mark: Mark,
) -> StageElement {
    StageElement::Card {
        id: id.into(),
        at,
        size,
        title: title.into(),
        status,
        tone,
        mark,
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

fn packet(id: &str, beam: &str, reverse: bool, label: &str, tone: Tone) -> StageElement {
    StageElement::Packet {
        id: id.into(),
        beam: beam.into(),
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

fn glitch(s: &mut StageActor, sc: &mut PlanBuilder, card: &str, at: u64, seeds: [f32; 3]) -> u64 {
    let mut step = at;
    for seed in seeds.into_iter().chain([0.0]) {
        s.set(sc, &format!("{card}.glitch"), step, seed);
        step += seconds(0.027);
    }
    step - seconds(0.027)
}

fn stage_plan() -> StagePlan {
    let elements = vec![
        StageElement::Orb {
            id: "old".into(),
            at: OLD_SERVICE,
            radius: 126.0,
            points: 850,
            tone: Tone::Plain,
        },
        StageElement::Ring {
            id: "port-ring".into(),
            at: OLD_SERVICE,
            radius: 138.0,
            thickness: 1.5,
            tone: Tone::Success,
        },
        StageElement::Ring {
            id: "port-ring-outer".into(),
            at: OLD_SERVICE,
            radius: 146.0,
            thickness: 1.3,
            tone: Tone::Success,
        },
        label(
            "old-name",
            [OLD_SERVICE[0], 700.0, 0.0],
            23.0,
            CaptionAlign::Center,
            &[("old server · pid 4127", Tone::Plain)],
        ),
        label(
            "old-active",
            [OLD_SERVICE[0], 732.0, 0.0],
            19.0,
            CaptionAlign::Center,
            &[("●", Tone::Success), (" holds :49374", Tone::Muted)],
        ),
        label(
            "old-lingering",
            [OLD_SERVICE[0], 732.0, 0.0],
            19.0,
            CaptionAlign::Center,
            &[
                ("●", Tone::Warning),
                (" lingering · still holds :49374", Tone::Error),
            ],
        ),
        label(
            "old-exited",
            [OLD_SERVICE[0], 732.0, 0.0],
            19.0,
            CaptionAlign::Center,
            &[
                ("●", Tone::Success),
                (" pid 4127 exited · :49374 free", Tone::Muted),
            ],
        ),
        card(
            "file",
            REG_FILE,
            REG_SIZE,
            "service.json",
            vec![
                status("owner: pid 4127", Tone::Muted),
                status("unregistered", Tone::Warning),
            ],
            Tone::Plain,
            Mark::Cross,
        ),
        card(
            "client",
            CLIENT,
            CLIENT_SIZE,
            "Service.stop",
            vec![
                status("stopping pid 4127", Tone::Plain),
                status("file gone → return early", Tone::Warning),
                status("watching pid 4127", Tone::Plain),
                status("escalating → SIGKILL", Tone::Error),
                status("stopped cleanly", Tone::Success),
            ],
            Tone::Request,
            Mark::Check,
        ),
        card(
            "new",
            NEW_SERVICE,
            NEW_SIZE,
            "new server",
            vec![
                status("starting", Tone::Plain),
                status("EADDRINUSE :49374", Tone::Error),
                status("starting", Tone::Plain),
                status("listening :49374", Tone::Success),
            ],
            Tone::Accent,
            Mark::Check,
        ),
        beam("stop-link", "client", "old", 0.0, Tone::Error),
        beam("reg-link", "old", "file", 0.0, Tone::Plain),
        beam("check-link", "client", "file", -32.0, Tone::Request),
        beam("bind-link", "new", "old", 0.0, Tone::Accent),
        packet("sigterm", "stop-link", false, "SIGTERM", Tone::Error),
        packet("sigkill", "stop-link", false, "SIGKILL", Tone::Error),
        packet("unreg", "reg-link", false, "unlink", Tone::Warning),
        packet("check", "check-link", false, "read file", Tone::Request),
        packet("gone", "check-link", true, "missing", Tone::Warning),
        packet("bind-1", "bind-link", false, "bind :49374", Tone::Request),
        packet("clash", "bind-link", true, "EADDRINUSE", Tone::Error),
        packet("bind-2", "bind-link", false, "bind :49374", Tone::Success),
    ];
    StagePlan {
        post: StagePost {
            bloom: 0.18,
            grain: 0.012,
            vignette: 0.22,
            backdrop: 0.12,
        },
        elements,
    }
}

fn stage_film(narration: &Narration) -> Result<ScenePlan> {
    let pr = &PRS[3];
    let before_clip = narration.clip("stop-before")?;
    let after_clip = narration.clip("stop-after")?;
    let lead = seconds(1.4);
    let rewind = seconds(2.2);
    let duration = lead + before_clip.duration() + rewind + after_clip.duration() + seconds(2.2);
    let mut scene = PlanBuilder::new("stop-stage", duration);
    let before = before_clip.place(&mut scene, lead);
    let after = after_clip.place(&mut scene, before.end() + rewind);
    let b = |phrase: &str| before.at(phrase);
    let a = |phrase: &str| after.at(phrase);

    let mut stage = StageActor::declare(&mut scene, "stage", &stage_plan())?;
    let s = &mut stage;
    let sc = &mut scene;

    // -----------------------------------------------------------------------
    // Opening establish: orb, service.json, and Service.stop client card
    // -----------------------------------------------------------------------
    for (property, initial) in [
        ("camera.z", -140.0),
        ("camera.dof", 0.38),
        ("old.scale", 0.62),
        ("old.blur", 10.0),
    ] {
        s.channel(sc, property, initial);
    }
    s.to(sc, "camera.z", 0, 0.0, 2.0);
    s.bounce(sc, "old.scale", seconds(0.12), 1.0, 0.8, 0.2);
    s.to(sc, "old.blur", seconds(0.12), 0.0, 0.65);
    s.channel(sc, "old.rotation", -1.4);
    s.ease(sc, "old.rotation", seconds(0.12), 0.0, 1.15, Ease::CubicOut);
    s.fade_in(sc, "old", seconds(0.12), 1.0, 0.55);
    s.fade_in(sc, "old-name", seconds(0.65), 1.0, 0.45);
    s.fade_in(sc, "old-active", seconds(0.8), 1.0, 0.45);

    let file_land = s.settle_in(sc, "file", seconds(0.32));
    s.connect(sc, "reg-link", file_land + seconds(0.1), 0.48);

    let client_land = s.settle_in(sc, "client", seconds(0.45));
    let client_conn = s.connect(sc, "stop-link", client_land + seconds(0.12), 0.5);
    sc.media(sound("connect-0", TASK_RUNNING, client_conn, -20.0));

    header(sc, pr, Some(seconds(0.3)))?;
    let mut before_chip = chip(sc, "chip-before", Tone::Muted, "before")?;
    before_chip.show(sc, seconds(0.55));

    // RollingNumber odometer: tracks the 5.0 s grace period and 10.0 s timeout
    let muted = |text: &str| CaptionSpanPlan::new(text, Tone::Muted);
    let mut timer = RollingNumberActor::declare(
        sc,
        "grace-timer",
        RollingNumberPlan::new([390.0, 385.0], 26.0, "0.0")
            .aligned(CaptionAlign::Center)
            .tone(Tone::Accent)
            .prefix(vec![muted("grace  ")])
            .suffix(vec![muted(" s")])
            .chip(),
    )?;

    // Stage Callout: shows what `terminate()` watches—starts on `service.json`,
    // then glides down to `old` (pid 4127) when the fix is replayed.
    let mut target_note = CalloutActor::declare(
        sc,
        "target-note",
        &CalloutPlan::new(
            CalloutAnchorPlan::Stage {
                id: "file".into(),
                element: "file".into(),
                edge: CalloutSide::Right,
                side: Some(CalloutSide::TopRight),
            },
            vec![
                span("SIGKILL gate ", Tone::Accent),
                span("· what stop() checks", Tone::Muted),
            ],
        )
        .anchor(CalloutAnchorPlan::Stage {
            id: "old".into(),
            element: "old".into(),
            edge: CalloutSide::TopRight,
            side: Some(CalloutSide::TopRight),
        })
        .side(CalloutSide::TopRight)
        .elbow()
        .reach(68.0)
        .tone(Tone::Accent)
        .chip(),
    )?;

    // -----------------------------------------------------------------------
    // Act 1 (stop-before): SIGTERM -> grace timer -> file unlinked -> bail out -> EADDRINUSE
    // -----------------------------------------------------------------------
    let sends = b("the client sends");
    s.clock(sc, "client.spinner", sends);
    s.to(sc, "client.glow", sends, 0.4, 0.45);
    s.to(sc, "stop-link.emphasis", sends, 1.0, 0.45);
    s.to(sc, "camera.x", sends, -60.0, 1.4);

    let sigterm_at = before.at_any(&["sigterm", "sig term"]);
    let sigterm_hit = s.send(sc, "sigterm", sigterm_at, 0.58);
    sc.media(sound("sigterm-send", SAVE, sigterm_at, -9.0));
    sc.media(sound("sigterm-hit", IMPACT, sigterm_hit, -11.0));

    let blow = Vec3::from(OLD_SERVICE) - Vec3::from(CLIENT);
    s.jolt(sc, sigterm_hit, [blow.x * 0.45, blow.y * 0.45], 0.55);
    s.to(sc, "old.hurt", sigterm_hit, 0.55, 0.25);

    // "waits for the server to exit": show the rolling grace timer ticking up to 5.0 s
    let waits = b("waits for the server");
    timer.show(sc, waits);
    timer.roll(sc, b("still running"), "2.5")?;
    timer.roll(sc, b("grace period"), "5.0")?;

    // "still running": the orb lingers on :49374 instead of exiting
    let running = b("still running");
    s.to(sc, "old-active.opacity", running, 0.0, 0.15);
    s.fade_in(sc, "old-lingering", running + seconds(0.22), 1.0, 0.25);

    // "registration file had disappeared": old server unlinks service.json
    let reg_file = b("registration file");
    let unreg_hit = s.send(sc, "unreg", reg_file, 0.45);
    s.to(sc, "reg-link.break", unreg_hit, 1.0, 0.55);
    s.hit(sc, "file.flash", unreg_hit, 0.35, 0.0);
    s.to(sc, "file.status", unreg_hit, 1.0, 0.25);
    s.to(sc, "file.dim", unreg_hit + seconds(0.2), 0.5, 0.5);
    target_note.show(sc, b("disappeared or changed"));

    // Client checks service.json and gives up without escalating
    let gave_up = b("gave up without escalating");
    s.connect(sc, "check-link", gave_up - seconds(0.95), 0.35);
    let check_hit = s.send(sc, "check", gave_up - seconds(0.65), 0.42);
    let gone_hit = s.send(sc, "gone", check_hit + seconds(0.36), 0.42);
    s.hit(sc, "client.flash", gone_hit, 0.3, 0.0);
    s.to(sc, "client.status", gone_hit, 1.0, 0.3);
    s.clock(sc, "client.release", gone_hit);
    s.to(sc, "stop-link.emphasis", gone_hit, 0.0, 0.4);
    target_note.emphasize(sc, gave_up);

    // "still hold the port, and the restart that followed would collide with it"
    let hold_port = b("still hold the port");
    s.hit(sc, "old.pulse", hold_port, 0.55, 0.0);
    let restart_at = b("restart that followed");
    s.to(sc, "camera.x", restart_at - seconds(0.3), 75.0, 1.3);
    s.settle_in(sc, "new", restart_at - seconds(0.25));
    s.clock(sc, "new.spinner", restart_at);
    s.connect(sc, "bind-link", restart_at - seconds(0.05), 0.4);

    let collide = b("collide with it");
    let bind_1_send = collide.saturating_sub(seconds(0.85));
    let bind_1_hit = s.send(sc, "bind-1", bind_1_send, 0.42);
    sc.media(sound("bind-1", SAVE, bind_1_send, -10.0));
    s.hit(sc, "old.pulse", bind_1_hit, 0.65, 0.0);

    let clash_hit = s.send(sc, "clash", bind_1_hit + seconds(0.36), 0.42);
    sc.media(sound("clash-hit", TASK_FAILURE, clash_hit, -8.0));
    s.hit(sc, "new.alarm", clash_hit, 0.28, 0.0);
    s.set(sc, "new.damage", clash_hit, 1.0);
    glitch(s, sc, "new", clash_hit, [7.0, 9.0, 8.0]);
    sc.media(sound("clash-glitch", GLITCH, clash_hit, -18.0));
    s.to(sc, "new.status", clash_hit, 1.0, 0.2);
    s.clock(sc, "new.release", clash_hit);
    s.to(sc, "bind-link.break", clash_hit + seconds(0.08), 1.0, 0.55);

    let mut footer_before = footer(
        sc,
        "footer-before",
        vec![
            span("trusted the file, so the restart ", Tone::Plain),
            span("collided with pid 4127", Tone::Error),
        ],
    )?;
    footer_before.type_in(sc, collide, 46.0, 0.7);

    // -----------------------------------------------------------------------
    // Rewind between stop-before and stop-after
    // -----------------------------------------------------------------------
    let switch = before.end() + seconds(0.45);
    s.clock_for(sc, "post.rewind", switch, 1.35);
    before_chip.hide(sc, switch);
    footer_before.hide(sc, switch);
    let mut rewind_chip = chip(sc, "chip-rewind", Tone::Accent, "◀◀ rewind")?;
    rewind_chip.show(sc, switch + seconds(0.22));
    rewind_chip.hide(sc, switch + seconds(1.4));
    let mut after_chip = chip(sc, "chip-after", Tone::Success, "after the fix")?;
    after_chip.show(sc, switch + seconds(1.55));
    sc.media(sound("rewind", LAUNCH, switch - seconds(0.08), -13.0));

    s.hit(sc, "post.chroma", switch, 0.12, 0.0);
    s.to(sc, "camera.x", switch, -35.0, 1.4);
    s.to(sc, "check-link.opacity", switch, 0.0, 0.4);
    s.to(sc, "bind-link.break", switch + seconds(0.2), 0.0, 0.5);
    s.to(sc, "bind-link.opacity", switch, 0.0, 0.35);
    s.to(sc, "new.opacity", switch, 0.0, 0.35);
    s.set(sc, "new.damage", switch + seconds(0.45), 0.0);
    s.set(sc, "new.status", switch + seconds(0.45), 2.0);
    for clock in ["spinner", "release", "mark"] {
        s.set(sc, &format!("new.{clock}"), switch + seconds(0.45), -1.0);
        s.set(sc, &format!("client.{clock}"), switch + seconds(0.45), -1.0);
    }
    timer.roll(sc, switch + seconds(0.35), "0.0")?;

    let watch_at = switch + seconds(0.75);
    s.to(sc, "client.status", watch_at, 2.0, 0.3);
    s.clock(sc, "client.spinner", watch_at);
    s.to(sc, "stop-link.emphasis", watch_at, 1.0, 0.45);

    // -----------------------------------------------------------------------
    // Act 2 (stop-after): Callout glides to PID 4127 -> SIGKILL shatters orb -> clean restart
    // -----------------------------------------------------------------------
    let trusts_pid = a("only trusts the process");
    target_note.move_to(sc, "old", trusts_pid)?;
    target_note.emphasize(sc, a("signaled"));

    timer.roll(sc, a("still alive"), "3.0")?;
    timer.roll(sc, a("grace period"), "5.0")?;

    let sigkill_at = after.at_any(&["sigkill", "sig kill"]);
    let sigkill_send = sigkill_at.saturating_sub(seconds(0.35));
    s.to(sc, "client.status", sigkill_send, 3.0, 0.25);
    let sigkill_hit = s.send(sc, "sigkill", sigkill_send, 0.52);
    sc.media(sound(
        "sigkill-launch",
        LAUNCH,
        sigkill_send - seconds(0.3),
        -15.0,
    ));
    sc.media(sound("sigkill-impact", IMPACT, sigkill_hit, -5.0));
    sc.media(sound(
        "sigkill-shatter",
        TASK_DEATH,
        sigkill_hit + seconds(0.05),
        -7.0,
    ));

    // The lingering orb compresses and combusts on SIGKILL
    s.clock_for(sc, "old.burst", sigkill_hit, combustion::DURATION);
    s.to(sc, "old.hurt", sigkill_hit, 1.0, 0.18);
    s.hit(sc, "post.chroma", sigkill_hit, 0.15, 0.0);
    s.hit(sc, "post.bloom", sigkill_hit, 0.38, 0.18);
    s.jolt(sc, sigkill_hit, [blow.x, blow.y], 0.95);
    s.to(sc, "stop-link.break", sigkill_hit + seconds(0.12), 1.0, 0.7);
    s.to(
        sc,
        "old-name.opacity",
        sigkill_hit + seconds(0.25),
        0.42,
        0.35,
    );
    s.to(
        sc,
        "old-lingering.opacity",
        sigkill_hit + seconds(0.12),
        0.0,
        0.15,
    );
    s.fade_in(sc, "old-exited", sigkill_hit + seconds(0.38), 1.0, 0.25);
    target_note.hide(sc, sigkill_hit + seconds(0.1));

    // Service.stop spinner resolves into check mark
    let client_done = sigkill_hit + seconds(0.45);
    s.to(sc, "client.status", client_done, 4.0, 0.3);
    let client_waited = client_done.saturating_sub(watch_at) as f32 / 1e9;
    let client_handoff = watch_at + seconds(f64::from(spinner::handoff(client_waited)));
    s.clock(sc, "client.mark", client_handoff);

    // Port :49374 opens cleanly and new server takes it
    let port_free = sigkill_hit + seconds(0.55);
    s.fade_in(sc, "port-ring", port_free, 0.35, 0.25);
    s.fade_in(sc, "port-ring-outer", port_free + seconds(0.06), 0.5, 0.25);

    let new_ready_enter = a("if even that fails");
    s.to(sc, "camera.x", new_ready_enter - seconds(0.15), 55.0, 1.3);
    s.settle_in(sc, "new", new_ready_enter);
    let new_spin = new_ready_enter + seconds(0.2);
    s.clock(sc, "new.spinner", new_spin);
    s.to(
        sc,
        "bind-link.opacity",
        new_ready_enter + seconds(0.15),
        1.0,
        0.25,
    );
    s.connect(sc, "bind-link", new_ready_enter + seconds(0.15), 0.42);

    let bind_2_send = a("clear error");
    let bind_2_hit = s.send(sc, "bind-2", bind_2_send, 0.48);
    sc.media(sound("bind-2", SAVE, bind_2_send, -9.0));
    s.hit(sc, "new.flash", bind_2_hit, 0.35, 0.0);
    s.to(sc, "new.status", bind_2_hit, 3.0, 0.3);
    s.to(sc, "new.glow", bind_2_hit, 0.55, 0.5);
    let new_waited = bind_2_hit.saturating_sub(new_spin) as f32 / 1e9;
    let new_handoff = new_spin + seconds(f64::from(spinner::handoff(new_waited)));
    s.clock(sc, "new.mark", new_handoff);
    sc.media(sound(
        "new-mark",
        MARK,
        new_handoff + seconds(f64::from(spinner::DRAW)),
        -18.0,
    ));
    sc.media(sound(
        "clean-restart",
        PRISMATIC_BLOOM,
        bind_2_hit + seconds(0.1),
        -10.0,
    ));

    // Roll timer to 10.0 s on "10 seconds"
    timer.roll(sc, after.at_any(&["10 seconds", "ten seconds"]), "10.0")?;

    let mut footer_after = footer(
        sc,
        "footer-after",
        vec![
            span("SIGKILL if needed · ", Tone::Plain),
            span("a clear error", Tone::Warning),
            span(" if even that fails", Tone::Plain),
        ],
    )?;
    footer_after.type_in(sc, a("clear error"), 44.0, 0.75);

    // -----------------------------------------------------------------------
    // Closing camera flies into Service.stop card -> Zoom into stop-code
    // -----------------------------------------------------------------------
    let close = after.end() + seconds(0.3);
    s.to(sc, "camera.x", close, CLOSING_CAMERA[0], 1.2);
    s.to(sc, "camera.y", close, CLOSING_CAMERA[1], 1.2);
    s.to(sc, "camera.z", close, CLOSING_CAMERA[2], 1.2);
    s.to(sc, "camera.focus", close, -40.0, 1.0);
    s.to(sc, "client.glow", close, 0.65, 0.8);
    after_chip.hide(sc, close);
    footer_after.hide(sc, close);
    timer.hide(sc, close);
    s.to(sc, "port-ring-outer.opacity", close, 0.0, 0.6);
    s.to(sc, "port-ring.opacity", close + seconds(0.08), 0.0, 0.6);

    scene.finish().context("stop-stage")
}

/// Build the `stop-code` Stepped Diff segment and attach an Editor Callout
/// pinned to the `signal(info.pid, "SIGKILL")` line as the early return leaves.
fn stop_code(narration: &Narration) -> Result<ScenePlan> {
    let pr = &PRS[3];
    let clip = narration.clip("stop-code")?;
    let lead = seconds(0.9);
    let mut plan = code(pr, narration, diffs(3), false)?;
    let editor = plan
        .actors
        .iter_mut()
        .find(|actor| actor.id == "editor")
        .context("stop-code editor")?;
    let mut recipe: EditorRecipePlan = serde_json::from_value(editor.data.clone())?;
    if let Some(line) = recipe.lines.iter_mut().find(|line| line.id == "line-8") {
        line.parts = vec![
            EditorPartPlan {
                id: "indent".into(),
                spans: vec![StyledSpan::new("  ", SyntaxStyle::Plain)],
            },
            EditorPartPlan {
                id: "sigkill".into(),
                spans: highlight::typescript("yield* signal(info.pid, \"SIGKILL\")"),
            },
        ];
        line.semantic_ranges.push(EditorSemanticRangePlan {
            id: "sigkill".into(),
            first_part_id: "sigkill".into(),
            last_part_id: "sigkill".into(),
        });
        line.mark = Some(LineMarkPlan::Added);
    }
    recipe.compile()?;
    editor.data = serde_json::to_value(recipe)?;

    // Rebuild with an Editor Callout pinned to `sigkill`
    let duration = plan.duration_nanos;
    let mut scene = PlanBuilder::new("stop-code-callout", duration);
    let spoken = clip.place(&mut scene, lead);
    let dummy_editor = scene.actor(
        "editor",
        psychopomp::editor::EDITOR_RECIPE,
        &serde_json::from_value::<EditorRecipePlan>(editor.data.clone())?,
    )?;
    scene.semantic_target(
        "sigkill",
        &dummy_editor,
        EditorTargetSelector {
            line_id: "line-8".into(),
            range_id: "sigkill".into(),
        },
    )?;
    let mut callout = CalloutActor::declare(
        &mut scene,
        "sigkill-note",
        &CalloutPlan::new(
            CalloutAnchorPlan::Editor {
                id: "sigkill".into(),
                target: "sigkill".into(),
                edge: CalloutSide::Right,
                side: None,
            },
            vec![
                span("always reached ", Tone::Success),
                span("when pid is still alive", Tone::Plain),
            ],
        )
        .side(CalloutSide::Right)
        .reach(72.0)
        .tone(Tone::Success)
        .chip(),
    )?;
    callout.show(&mut scene, spoken.at("early return"));
    callout.emphasize(&mut scene, spoken.at("trusted the file"));
    let extra = scene.finish()?;
    plan.semantic_targets.extend(extra.semantic_targets);
    if let Some(actor) = extra.actors.into_iter().find(|a| a.id == "sigkill-note") {
        plan.actors.push(actor);
    }
    plan.continuous_channels.extend(
        extra
            .continuous_channels
            .into_iter()
            .filter(|c| c.actor_id == "sigkill-note"),
    );
    plan.validate()?;
    Ok(plan)
}
