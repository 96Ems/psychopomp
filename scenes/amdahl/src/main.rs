//! Amdahl's Law for I/O-bound coding agents.
//!
//! Act 1 (`microbenchmark`): A 0–1,000 ms benchmark chart where launch (23×),
//! shell spawn (8.6×), and exit (44×) look like massive breakthroughs.
//!
//! Act 2 (`turn` & `scale` on the Stage): The cross-country road-trip door-slam
//! metaphor and a live 60-second agent turn. The local harness spends 2 ms
//! formatting a request and then waits in `epoll_wait` while a remote LLM
//! thinks and streams tokens for 51.5 s and local tests run for 8.2 s. Whether
//! that idle loop is written in Zig, Rust, or TypeScript, every language waits
//! on a socket at 1.0 second per second. Pulling back to true wall-clock scale
//! reveals the 60-second bar: 85.8% remote model I/O, 13.7% tools, and a 0.5%
//! harness sliver at the tip—where a 44× speedup turns 60.0 s into 59.7 s.
use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use psychopomp::{
    author::{PlanBuilder, millis, seconds, stagger},
    axis::AxisPlan,
    bars::{BarDeltaPlan, BarSeriesPlan, BarsActor, BarsPlan},
    caption::{CaptionAlign, CaptionSpanPlan},
    chrome::{chip, footer, header},
    confetti::{ConfettiActor, ConfettiPlan},
    math::easing::Ease,
    narration::Reading,
    plan::{ReelPlan, ReelSegmentPlan, ReelTransitionStyle, ScenePlan},
    readout::ReadoutFormat,
    rolling::{RollingNumberActor, RollingNumberPlan},
    sfx,
    stage::{
        Arrow, Curve, Figure, Fill, Material, OrbEntrance, StageActor, StageElement, StagePlan,
        StagePost, Waypoint, reply_after,
    },
    tone::Tone,
};
use psychopomp_media::{Audio, Media, Voice};

/// Kit's ElevenLabs Professional Voice Clone.
const KIT: &str = "8olojUk4IXpvKgaOCHXj";

const MICRO: &str = "[confident, upbeat, tech-keynote energy] Every week, another coding agent benchmark drops. Launch: twenty-three times faster! Shell calls: eight point six times faster! Exiting: forty-four times faster! [impressed, playful] Look at those bars shrink. Look at those multipliers. [dry, conversational, leaning in] Until you remember... a coding agent is mostly waiting.";

const TURN: &str = "[conversational, clear, amused] Imagine driving across the country. Forty hours on the highway, and you brag that you oiled your car door hinges so closing the door at the gas station is forty-four times faster. [matter-of-fact, steady] Watch one real sixty-second agent turn. The local harness prepares a request in two milliseconds, and sends it across the network. [quiet, patient, deadpan] And now... the harness waits. Fifty-one seconds waiting on a remote model to think and stream tokens. Eight seconds waiting on the test suite. Whether your idle loop is written in Zig, Rust, or TypeScript, every language waits on a socket at the exact same speed: one second per second.";

const SCALE: &str = "[engaged, building] Now pull the camera back to true wall-clock scale. Out of sixty seconds, the remote model takes eighty-six percent. Local tools take fourteen percent. And that entire harness bar at the very end? Half of one percent. [amused, wry] Make that tiny sliver forty-four times faster, and your sixty-second turn finishes in... fifty-nine point seven seconds. [warm, grounded, concluding] Fixing a slow shell startup or a stuck exit is great hygiene. Just don't confuse fixing a local bug with speeding up the highway. Measure the whole turn.";

const HARNESS: [f32; 3] = [380.0, 380.0, 0.0];
const HARNESS_SIZE: [f32; 2] = [340.0, 124.0];
const TOOLS: [f32; 3] = [940.0, 500.0, -20.0];
const TOOLS_SIZE: [f32; 2] = [330.0, 114.0];
const MODEL: [f32; 3] = [1520.0, 380.0, 0.0];
const MODEL_RADIUS: f32 = 128.0;

/// True-scale 60-second bar geometry (1,400 px total width from x = 260 to x = 1660).
const BAR_Y: f32 = 760.0;
const BAR_H: f32 = 52.0;
const BAR_LLM_X: f32 = 860.5;
const BAR_LLM_W: f32 = 1201.0;
const BAR_TOOLS_X: f32 = 1556.5;
const BAR_TOOLS_W: f32 = 191.0;
const BAR_HARNESS_X: f32 = 1656.5;
const BAR_HARNESS_W: f32 = 7.0;

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let media = Media::open(&root)?;
    let kit = Voice::eleven(KIT)
        .v4()
        .stability(0.2)
        .similarity(0.65)
        .whisper();
    let micro = media.dialogue("micro", [(&kit, MICRO)])?;
    let turn = media.dialogue("turn", [(&kit, TURN)])?;
    let scale = media.dialogue("scale", [(&kit, SCALE)])?;
    media.finish()?;

    let reel = ReelPlan {
        version: ReelPlan::VERSION,
        id: "amdahl".to_owned(),
        segments: vec![
            ReelSegmentPlan {
                transition_nanos: 0,
                transition_style: ReelTransitionStyle::Dip,
                transition_focus: None,
                transition_wipe: None,
                plan: microbenchmark(&micro)?,
            },
            ReelSegmentPlan {
                transition_nanos: seconds(0.65),
                transition_style: ReelTransitionStyle::Dip,
                transition_focus: None,
                transition_wipe: None,
                plan: stage_film(&turn, &scale)?,
            },
        ],
    };
    reel.validate()?;
    let output = root.join("amdahl.reel.json");
    fs::write(&output, serde_json::to_string_pretty(&reel)? + "\n")?;
    eprintln!(
        "wrote {} ({:.1}s)",
        output.display(),
        reel.duration_nanos() as f64 / 1e9
    );
    Ok(())
}

fn span(text: &str, tone: Tone) -> CaptionSpanPlan {
    CaptionSpanPlan::new(text, tone)
}

/// Segment 1: The zoomed-in microbenchmark bar chart (`launch`, `shell call`, `exit`).
fn microbenchmark(micro_clip: &Audio) -> Result<ScenePlan> {
    let reading = Reading::new(seconds(0.9), [(micro_clip.clip(), seconds(1.6))]);
    let mut scene = PlanBuilder::new("microbenchmark", reading.duration());
    let [spoken] = reading.place(&mut scene);
    let m = |phrase: &str| spoken.at(phrase);

    let mut title = header(&mut scene, "benchmarks", "the multiplier illusion")?;
    title.type_in(&mut scene, seconds(0.2), 48.0, 0.7);
    let mut micro_chip = chip(&mut scene, "chip-micro", Tone::Accent, "microbenchmark")?;
    micro_chip.show(&mut scene, seconds(0.5));

    let rows = [
        ("launch", "launch", 184.0, 8.0),
        ("shell", "shell call", 238.8, 27.9),
        ("exit", "exit", 1003.0, 23.0),
    ];
    let plan = rows.iter().fold(
        BarsPlan::new(
            [420.0, 310.0],
            1020.0,
            AxisPlan::new([0.0, 1050.0])
                .every(250.0)
                .label("milliseconds (0 – 1,000 ms)"),
        )
        .row_height(132.0)
        .size(28.0)
        .series(BarSeriesPlan::new("before", "before", Tone::Muted))
        .series(BarSeriesPlan::new("after", "after", Tone::Accent))
        .readout(ReadoutFormat::new(0).grouped().unit("ms"))
        .delta(BarDeltaPlan::new("before", "after").factor()),
        |plan, (id, label, ..)| plan.row(*id, *label),
    );
    let mut bench = BarsActor::declare(&mut scene, "bench", &plan)?;
    bench.show(&mut scene, seconds(0.35));

    let before = rows
        .iter()
        .map(|(id, _, before, _)| (*id, *before))
        .collect::<Vec<_>>();
    let after = rows
        .iter()
        .map(|(id, _, _, after)| (*id, *after))
        .collect::<Vec<_>>();

    let drops = m("benchmark");
    bench.grow(&mut scene, drops, "before", &before)?;
    sfx::SEND.play(&mut scene, "before-grow", drops, -14.0);

    let faster = m("launch");
    bench.grow(&mut scene, faster, "after", &after)?;
    sfx::TICK.play(&mut scene, "after-grow", faster, -12.0);

    let mult = m("multipliers");
    bench.reveal_deltas(&mut scene, mult)?;
    sfx::SUCCESS.play(&mut scene, "deltas", mult, -9.0);
    let mut confetti = ConfettiActor::declare(
        &mut scene,
        "confetti",
        &ConfettiPlan::new([1420.0, 480.0]).seed(7).count(110),
    )?;
    confetti.burst(&mut scene, mult + seconds(0.1));

    let remember = m("remember");
    let mut foot = footer(
        &mut scene,
        "footer-micro",
        vec![
            span("huge multipliers on ", Tone::Plain),
            span("milliseconds", Tone::Accent),
            span("  ·  inside a turn measured in ", Tone::Plain),
            span("minutes", Tone::Warning),
        ],
    )?;
    foot.type_in(&mut scene, remember, 46.0, 0.8);

    scene.cue("microbenchmark", 0, reading.duration());
    scene.finish().context("microbenchmark")
}

fn rect_shape(id: &str, at: [f32; 3], size: [f32; 2], tone: Tone, opacity: f32) -> StageElement {
    StageElement::Shape {
        id: id.into(),
        at,
        shape: Figure::Rect(size),
        corner: 8.0,
        fill: Some(Fill::Tone(tone)),
        fill_opacity: opacity,
        stroke: Some(tone),
        width: 1.8,
        dash: None,
        arrow: Arrow::None,
    }
}

fn stage_plan() -> StagePlan {
    let note_x = HARNESS[0] - HARNESS_SIZE[0] * 0.5 + 4.0;
    let elements = vec![
        StageElement::orb("model", MODEL, MODEL_RADIUS)
            .points(900)
            .tone(Tone::Request),
        StageElement::label(
            "model-name",
            [MODEL[0], MODEL[1] + MODEL_RADIUS + 34.0, 0.0],
            24.0,
            &[("remote LLM", Tone::Plain)],
        ),
        StageElement::label(
            "model-sub",
            [MODEL[0], MODEL[1] + MODEL_RADIUS + 66.0, 0.0],
            19.0,
            &[
                ("● ", Tone::Request),
                ("51.5 s  network + token stream", Tone::Muted),
            ],
        ),
        StageElement::card("harness", HARNESS, HARNESS_SIZE, "agent harness")
            .statuses(&[
                ("format JSON · 2 ms", Tone::Plain),
                ("epoll_wait · 0% CPU", Tone::Muted),
                ("spawn shell · 8 ms", Tone::Plain),
                ("44× faster exit!", Tone::Accent),
            ])
            .tone(Tone::Accent),
        StageElement::card("tools", TOOLS, TOOLS_SIZE, "local tools")
            .statuses(&[
                ("idle", Tone::Muted),
                ("cargo test · 8.2 s", Tone::Warning),
                ("passed · 8.2 s", Tone::Success),
            ])
            .tone(Tone::Warning),
        StageElement::beam("net", "harness", "model")
            .bend(-42.0)
            .tone(Tone::Request),
        StageElement::beam("stream", "model", "harness")
            .bend(-48.0)
            .tone(Tone::Request),
        StageElement::beam("exec", "harness", "tools")
            .bend(28.0)
            .tone(Tone::Warning),
        StageElement::packet("req", "net")
            .labeled("POST /v1/responses (2 ms)")
            .tone(Tone::Request),
        StageElement::packet("tok-1", "stream")
            .labeled("thinking…")
            .tone(Tone::Request),
        StageElement::packet("tok-2", "stream")
            .labeled("tool_call: cargo test")
            .tone(Tone::Request),
        StageElement::packet("tok-3", "stream")
            .labeled("final answer")
            .tone(Tone::Success),
        StageElement::packet("run-test", "exec")
            .labeled("cargo test")
            .tone(Tone::Warning),
        StageElement::packet("test-ok", "exec")
            .reversed()
            .labeled("ok (8.2 s)")
            .tone(Tone::Success),
        StageElement::label(
            "door-metaphor",
            [960.0, 248.0, 0.0],
            23.0,
            &[
                ("40-hour road trip", Tone::Plain),
                ("  vs.  ", Tone::Muted),
                ("44× faster gas-station door slam (9 ms)", Tone::Accent),
            ],
        ),
        StageElement::label(
            "wait-note",
            [note_x, HARNESS[1] + 92.0, 0.0],
            20.0,
            &[
                ("waiting speed: ", Tone::Muted),
                ("Zig = Rust = TS = 1.0 s/s", Tone::Accent),
            ],
        )
        .align(CaptionAlign::Left),
        StageElement::ring("model-ring", MODEL, MODEL_RADIUS + 14.0)
            .thickness(1.4)
            .tone(Tone::Request),
        StageElement::ring("model-ring-outer", MODEL, MODEL_RADIUS + 22.0)
            .thickness(1.3)
            .tone(Tone::Request),
        // True-scale 60-second timeline bar along the lower stage
        StageElement::Shape {
            id: "bar-track".into(),
            at: [960.0, BAR_Y, 0.0],
            shape: Figure::Rect([1412.0, BAR_H + 10.0]),
            corner: 10.0,
            fill: Some(Fill::Material(Material::Surface)),
            fill_opacity: 0.9,
            stroke: Some(Tone::Muted),
            width: 1.4,
            dash: None,
            arrow: Arrow::None,
        },
        rect_shape(
            "bar-llm",
            [BAR_LLM_X, BAR_Y, 0.0],
            [BAR_LLM_W, BAR_H],
            Tone::Request,
            0.32,
        ),
        rect_shape(
            "bar-tools",
            [BAR_TOOLS_X, BAR_Y, 0.0],
            [BAR_TOOLS_W, BAR_H],
            Tone::Warning,
            0.34,
        ),
        rect_shape(
            "bar-harness",
            [BAR_HARNESS_X, BAR_Y, 0.0],
            [BAR_HARNESS_W, BAR_H],
            Tone::Accent,
            0.85,
        ),
        StageElement::label(
            "bar-title",
            [260.0, BAR_Y - 52.0, 0.0],
            21.0,
            &[
                ("true wall-clock scale  ", Tone::Plain),
                ("60,000 ms (100%)", Tone::Muted),
            ],
        )
        .align(CaptionAlign::Left),
        StageElement::label(
            "label-llm",
            [BAR_LLM_X, BAR_Y, 0.0],
            22.0,
            &[
                ("remote LLM I/O  ", Tone::Plain),
                ("51.5 s  (85.8%)", Tone::Request),
            ],
        ),
        StageElement::label(
            "label-tools",
            [BAR_TOOLS_X, BAR_Y + 52.0, 0.0],
            20.0,
            &[("tools  ", Tone::Plain), ("8.2 s (13.7%)", Tone::Warning)],
        ),
        StageElement::label(
            "label-harness",
            [1660.0, BAR_Y - 52.0, 0.0],
            21.0,
            &[("harness  ", Tone::Plain), ("0.3 s (0.5%) ▾", Tone::Accent)],
        )
        .align(CaptionAlign::Right),
        StageElement::label(
            "label-speedup",
            [960.0, BAR_Y + 96.0, 0.0],
            25.0,
            &[
                ("44× faster on 0.5%  →  ", Tone::Muted),
                ("60.0 s becomes 59.7 s", Tone::Plain),
                ("  (1.005× total)", Tone::Accent),
            ],
        ),
        StageElement::Path {
            id: "sliver-pointer".into(),
            through: vec![
                Waypoint::Point([1656.5, BAR_Y - 34.0, 0.0]),
                Waypoint::Point([1656.5, BAR_Y - 28.0, 0.0]),
            ],
            curve: Curve::Straight,
            corner: 0.0,
            bend: 0.0,
            tone: Tone::Accent,
            width: 2.0,
            dash: None,
            arrow: Arrow::End,
        },
    ];
    StagePlan {
        post: StagePost::RESTRAINED,
        elements,
    }
}

fn stage_film(turn_clip: &Audio, scale_clip: &Audio) -> Result<ScenePlan> {
    let reading = Reading::new(
        seconds(1.2),
        [
            (turn_clip.clip(), seconds(0.8)),
            (scale_clip.clip(), seconds(2.4)),
        ],
    );
    let mut scene = PlanBuilder::new("turn-and-scale", reading.duration());
    let [turn, scale] = reading.place(&mut scene);
    let t = |phrase: &str| turn.at(phrase);
    let a = |phrase: &str| scale.at(phrase);

    let mut stage = StageActor::declare(&mut scene, "stage", &stage_plan())?;
    let s = &mut stage;
    let sc = &mut scene;

    // Start with the true-scale timeline bars hidden until Act 3.
    for bar in ["bar-track", "bar-llm", "bar-tools", "bar-harness"] {
        s.channel(sc, &format!("{bar}.opacity"), 0.0);
        s.channel(sc, &format!("{bar}.draw"), 0.0);
        s.channel(sc, &format!("{bar}.fill"), 0.0);
    }
    s.channel(sc, "sliver-pointer.opacity", 0.0);
    s.channel(sc, "camera.z", -140.0);
    s.channel(sc, "camera.y", -40.0);
    s.channel(sc, "camera.dof", 0.35);
    s.to(sc, "camera.z", 0, 0.0, 2.0);

    // ── Establish Stage: Harness, Tools, Remote Model Orb ──
    let mut hdr = header(sc, "amdahl's law", "where an agent turn actually goes")?;
    hdr.show(sc, seconds(0.2));
    let mut turn_chip = chip(sc, "chip-turn", Tone::Request, "I/O-bound loop")?;
    turn_chip.show(sc, seconds(0.4));

    let harness_in = s.settle_in(sc, "harness", seconds(0.3));
    let tools_in = s.settle_in(sc, "tools", seconds(0.48));
    s.orb_in(sc, "model", seconds(0.2), OrbEntrance::HERO);
    stagger(
        ["model-name", "model-sub"],
        seconds(0.85),
        millis(140),
        |name, at| {
            s.fade_in(sc, name, at, 1.0, 0.45);
            at
        },
    );
    let net_contact = s.connect(sc, "net", harness_in + seconds(0.18), 0.65);
    let stream_contact = s.connect(sc, "stream", harness_in + seconds(0.32), 0.65);
    let exec_contact = s.connect(sc, "exec", tools_in + seconds(0.18), 0.55);
    for (i, (beam, at)) in [
        ("net", net_contact),
        ("stream", stream_contact),
        ("exec", exec_contact),
    ]
    .into_iter()
    .enumerate()
    {
        s.to(sc, &format!("{beam}.flow"), at + seconds(0.75), 0.0, 0.4);
        sfx::TICK.play(sc, format!("wire-{i}"), at, -20.0);
    }

    // ── Cross-country road trip metaphor ──
    let highway = t("driving");
    s.type_in(sc, "door-metaphor", highway, 72.0);
    let door = t("gas station");
    s.hit(sc, "harness.flash", door, 0.45, 0.0);

    // ── Watch one real 60-second agent turn ──
    let watch = t("agent turn");
    s.to(sc, "door-metaphor.opacity", watch, 0.0, 0.4);
    let mut clock = RollingNumberActor::declare(
        sc,
        "turn-clock",
        RollingNumberPlan::new([960.0, 175.0], 36.0, "0.0")
            .aligned(CaptionAlign::Center)
            .tone(Tone::Plain)
            .prefix(vec![span("turn elapsed  ", Tone::Muted)])
            .suffix(vec![span(" s", Tone::Muted)])
            .chip(),
    )?;
    clock.show(sc, watch);

    // Harness formats JSON in 2 ms and sends request across the wire.
    let prep = t("prepares");
    s.hit(sc, "harness.flash", prep, 0.5, 0.0);
    s.to(sc, "harness.glow", prep, 0.45, 0.35);
    let send_at = t("sends");
    let req_land = s.send(sc, "req", send_at, 0.75);
    sfx::SEND.play(sc, "req-send", send_at, -9.0);
    clock.roll(sc, req_land, "0.2")?;

    // "And now... the harness waits."
    let waits = t("waits");
    s.to(sc, "harness.status", waits, 1.0, 0.35);
    s.to(sc, "harness.glow", waits, 0.0, 0.4);
    s.to(sc, "harness.dim", waits, 0.38, 0.5);

    // Remote model thinks & streams tokens (51.5 s).
    let fifty = t("remote model");
    s.to(sc, "camera.x", fifty - seconds(0.3), 110.0, 1.5);
    s.to(sc, "model.charge", fifty, 0.75, 0.6);
    s.to(sc, "stream.flow", fifty, 0.55, 0.5);
    s.halo(
        sc,
        [("model-ring", 0.35), ("model-ring-outer", 0.5)],
        fifty + seconds(0.15),
        0.25,
    );
    let tok1 = s.send(sc, "tok-1", fifty + seconds(0.25), 0.75);
    s.land(sc, "harness", tok1);
    clock.roll(sc, fifty + seconds(0.4), "24.8")?;
    let tok2 = s.send(sc, "tok-2", fifty + seconds(1.2), 0.75);
    s.land(sc, "harness", tok2);
    clock.roll(sc, tok2, "51.5")?;
    s.to(sc, "model.charge", tok2, 0.15, 0.5);
    s.to(sc, "stream.flow", tok2, 0.0, 0.45);
    s.halo_out(sc, ["model-ring", "model-ring-outer"], tok2, 0.5);

    // 8 seconds waiting on the test suite.
    let eight = t("test suite");
    s.to(sc, "camera.x", eight - seconds(0.2), 0.0, 1.4);
    s.to(sc, "harness.dim", eight, 0.0, 0.25);
    s.swap_status(sc, "harness", eight, [1, 2], 0.25);
    let run_test = s.send(sc, "run-test", eight, 0.55);
    sfx::SEND.play(sc, "test-send", eight, -11.0);
    s.land(sc, "tools", run_test);
    s.to(sc, "tools.status", run_test, 1.0, 0.3);
    s.clock(sc, "tools.spinner", run_test);
    s.to(sc, "harness.dim", run_test + seconds(0.1), 0.38, 0.4);
    s.swap_status(sc, "harness", run_test + seconds(0.1), [2, 1], 0.3);

    let test_done = reply_after(run_test) + seconds(0.55);
    let test_ok = s.send(sc, "test-ok", test_done, 0.55);
    s.land(sc, "harness", test_ok);
    let mark_at = s.resolve_spinner(sc, "tools", run_test, test_done);
    s.swap_status(sc, "tools", test_done, [1, 2], 0.3);
    sfx::MARK.play(sc, "test-mark", mark_at, -16.0);
    let tok3 = s.send(sc, "tok-3", reply_after(test_ok), 0.65);
    s.land(sc, "harness", tok3);
    clock.roll(sc, tok3, "60.0")?;

    // "Whether your idle loop is written in Zig, Rust, or TypeScript..."
    let lang = t("every language");
    s.to(sc, "camera.x", lang - seconds(0.3), -90.0, 1.4);
    s.type_in(sc, "wait-note", lang, 46.0);

    // ── Act 3: Pull back to True Wall-Clock Scale (Amdahl's Law) ──
    let pull = a("pull the camera");
    turn_chip.hide(sc, pull);
    let mut law_chip = chip(sc, "chip-law", Tone::Accent, "amdahl's law")?;
    law_chip.show(sc, pull + seconds(0.25));
    s.to(sc, "camera.x", pull, 0.0, 1.5);
    s.to(sc, "camera.y", pull, 0.0, 1.5);
    s.to(sc, "camera.z", pull, -50.0, 1.6);
    s.to(sc, "harness.dim", pull, 0.0, 0.5);

    // Reveal the 60,000 ms track
    s.to(sc, "bar-track.opacity", pull + seconds(0.2), 1.0, 0.45);
    s.ease(
        sc,
        "bar-track.draw",
        pull + seconds(0.2),
        1.0,
        0.65,
        Ease::CubicOut,
    );
    s.to(sc, "bar-track.fill", pull + seconds(0.35), 1.0, 0.5);
    s.fade_in(sc, "bar-title", pull + seconds(0.35), 1.0, 0.45);

    // 1. Remote model: 85.8% (51.5 s)
    let llm_at = a("remote model");
    s.to(sc, "bar-llm.opacity", llm_at, 1.0, 0.35);
    s.ease(sc, "bar-llm.draw", llm_at, 1.0, 0.6, Ease::CubicOut);
    s.to(sc, "bar-llm.fill", llm_at + seconds(0.15), 1.0, 0.5);
    s.fade_in(sc, "label-llm", llm_at + seconds(0.2), 1.0, 0.45);
    sfx::SEND.play(sc, "bar-llm-in", llm_at, -14.0);

    // 2. Local tools: 13.7% (8.2 s)
    let tools_at = a("local tools");
    s.to(sc, "bar-tools.opacity", tools_at, 1.0, 0.35);
    s.ease(sc, "bar-tools.draw", tools_at, 1.0, 0.5, Ease::CubicOut);
    s.to(sc, "bar-tools.fill", tools_at + seconds(0.12), 1.0, 0.45);
    s.fade_in(sc, "label-tools", tools_at + seconds(0.18), 1.0, 0.45);
    sfx::TICK.play(sc, "bar-tools-in", tools_at, -14.0);

    // 3. Harness sliver at the tip: 0.5% (0.3 s)
    let sliver_at = a("entire harness");
    s.to(sc, "bar-harness.opacity", sliver_at, 1.0, 0.25);
    s.to(sc, "bar-harness.draw", sliver_at, 1.0, 0.3);
    s.to(sc, "bar-harness.fill", sliver_at, 1.0, 0.3);
    s.hit(sc, "bar-harness.flash", sliver_at, 0.85, 0.0);
    s.to(sc, "sliver-pointer.opacity", sliver_at, 1.0, 0.3);
    s.fade_in(sc, "label-harness", sliver_at + seconds(0.1), 1.0, 0.4);
    sfx::TICK.play(sc, "sliver-in", sliver_at, -11.0);

    // "Make that tiny sliver 44× faster..."
    let make_fast = a("sliver");
    s.swap_status(sc, "harness", make_fast, [1, 3], 0.3);
    s.hit(sc, "harness.flash", make_fast, 0.7, 0.0);
    s.hit(sc, "bar-harness.flash", make_fast, 1.0, 0.0);
    sfx::SPARKLE.play(sc, "fast-sparkle", make_fast, -12.0);

    // "...and your 60-second turn finishes in... 59.7 seconds."
    let fifty_nine = a("finishes");
    clock.roll(sc, fifty_nine, "59.7")?;
    s.type_in(sc, "label-speedup", fifty_nine, 48.0);
    sfx::RESET.play(sc, "tiny-roll", fifty_nine, -13.0);

    // Closing takeaway footer
    let whole_turn = a("whole turn");
    let mut foot = footer(
        sc,
        "footer-scale",
        vec![
            span("fix the local bugs  ·  ", Tone::Plain),
            span("benchmark the whole turn", Tone::Accent),
        ],
    )?;
    foot.type_in(sc, whole_turn - seconds(0.3), 44.0, 0.8);

    scene.cue("turn-and-scale", 0, reading.duration());
    scene.finish().context("turn-and-scale")
}

#[cfg(test)]
mod tests {
    #[test]
    fn stage_cards_fit_camera_compositions() {
        use psychopomp::math::{Vec3, vec2};
        use psychopomp::stage::{Camera, StageElement};

        for position in [
            [0.0, -40.0, -140.0],
            [0.0, -40.0, 0.0],
            [110.0, -40.0, 0.0],
            [-90.0, -40.0, 0.0],
            [0.0, 55.0, -70.0],
        ] {
            let camera = Camera::at(Vec3::from(position), vec2(1920.0, 1080.0));
            for element in super::stage_plan().elements {
                if let StageElement::Card { id, at, size, .. } = element {
                    let (center, scale) = camera.project(Vec3::from(at)).unwrap();
                    let half = vec2(size[0], size[1]) * (0.5 * scale);
                    assert!(
                        center.x - half.x >= 40.0
                            && center.x + half.x <= 1880.0
                            && center.y - half.y >= 40.0
                            && center.y + half.y <= 1040.0,
                        "card '{id}' out of bounds at camera {position:?}: center={center:?}, half={half:?}"
                    );
                }
            }
        }
    }
}
