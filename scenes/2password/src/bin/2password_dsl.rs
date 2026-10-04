//! `scenes/2password` re-expressed with `psychopomp::score` and `psychopomp::layout`.
//!
//! `PlanBuilder` is the only mutable binding; `Stage`, `Caption` (headers,
//! chips, footers), and `sfx` are immutable values whose beats compose inside
//! `sc.at(...)` with zero escape hatches and emit byte-identical `ScenePlan` JSON.
use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use psychopomp::{
    all,
    author::{PlanBuilder, seconds},
    caption::{CaptionAlign, CaptionPlan, CaptionSpanPlan},
    layout::Placement,
    math::easing::Ease,
    narration::Narration,
    plan::{ReelPlan, ReelSegmentPlan, ReelTransitionStyle, ScenePlan},
    score::{Beat, Caption, CueTime, Stage, each, sound, stagger, stagger_indexed},
    stage::{DRAW_CURVE, StageElement as El, StagePlan, StagePost},
    tone::Tone::{self, Accent, Error, Muted, Plain, Request, Success, Warning},
};
use psychopomp_pr_walkthrough::film::{FOOTER_Y, HEADER_Y, LEFT, Pr, chip, span};

const AGENT: [f32; 3] = [300.0, 540.0, 0.0];
const LAYER: [f32; 3] = [760.0, 540.0, -40.0];
const OP: [f32; 3] = [1210.0, 540.0, 0.0];
const VAULT: [f32; 3] = [1650.0, 540.0, 0.0];
const PROCESS: [f32; 3] = [760.0, 830.0, 0.0];
const CLIPBOARD: [f32; 3] = [300.0, 830.0, 0.0];
const KEYCHAIN: [f32; 3] = [760.0, 250.0, 0.0];
const CARD: [f32; 2] = [300.0, 124.0];
const SMALL: [f32; 2] = [300.0, 110.0];
const PROMPT: [f32; 2] = [340.0, 110.0];
const TITLE_CAMERA_Y: f32 = -900.0;
const AGENT_FLIGHT: [f32; 2] = [960.0 - AGENT[0], -570.0];
const LEAK_CAMERA: [f32; 3] = [-330.0, 0.0, 140.0];

const PROMPTS: [(&str, [f32; 3], &str, Tone); 4] = [
    ("prompt-1", [1210.0, 300.0, -80.0], "Allow access?", Warning),
    (
        "prompt-2",
        [1590.0, 320.0, -130.0],
        "Approve this request?",
        Warning,
    ),
    ("prompt-3", [1210.0, 790.0, -300.0], "AUTHORIZE?!", Error),
    (
        "prompt-4",
        [790.0, 310.0, -120.0],
        "Sign in to continue",
        Error,
    ),
];

const PR: Pr = Pr {
    number: "2password",
    title: "1Password for coding agents",
    slug: "2password",
};

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let narration = Narration::load(&root.join("narration"))?;
    let reel = ReelPlan {
        version: ReelPlan::VERSION,
        id: "2password".to_owned(),
        segments: vec![ReelSegmentPlan {
            transition_nanos: 0,
            transition_style: ReelTransitionStyle::Dip,
            transition_focus: None,
            transition_wipe: None,
            plan: film(&narration)?,
        }],
    };
    reel.validate()?;
    fs::create_dir_all("target")?;
    fs::write(
        "target/2password-dsl.reel.json",
        serde_json::to_string_pretty(&reel)? + "\n",
    )?;
    Ok(())
}

fn spans(parts: &[(&str, Tone)]) -> Vec<CaptionSpanPlan> {
    parts.iter().map(|&(t, tone)| span(t, tone)).collect()
}

fn stage_plan() -> StagePlan {
    let vault_box = Placement::orb(VAULT, 118.0);
    let chat_at = Placement::card(AGENT, CARD).align_left(4.0, -100.0);

    let mut elements = vec![
        El::orb("vault", VAULT, 118.0).points(900).tone(Plain),
        El::label(
            "vault-name",
            vault_box.below(46.0),
            24.0,
            &[("1Password", Plain)],
        ),
        El::card("agent", AGENT, CARD, "coding agent")
            .tone(Request)
            .statuses(&[
                ("needs one API key", Plain),
                ("waiting for approval…", Warning),
                ("stuck", Error),
                ("secret in the chat!", Error),
                ("asking 2password", Plain),
                ("references only ✓", Success),
            ]),
        El::card("op", OP, [330.0, 124.0], "op · 1Password CLI").statuses(&[
            ("ready", Muted),
            ("approval required", Warning),
            ("locked", Error),
            ("ready", Muted),
            ("approved once ✓", Success),
        ]),
        El::card("layer", LAYER, CARD, "2password")
            .tone(Accent)
            .statuses(&[
                ("a layer on top of op", Muted),
                ("batching every lookup", Plain),
                ("references only", Success),
                ("injecting secrets", Plain),
                ("verified ✓", Success),
                ("no prompts", Success),
            ]),
        El::card("process", PROCESS, SMALL, "bun dev")
            .statuses(&[("starting", Muted), ("running with secrets ✓", Success)]),
        El::card("clipboard", CLIPBOARD, SMALL, "clipboard").statuses(&[("a new API key", Muted)]),
        El::card("keychain", KEYCHAIN, SMALL, "macOS Keychain")
            .statuses(&[("service-account token", Muted)]),
        El::beam("direct", "agent", "op").tone(Request),
        El::beam("vault-link", "op", "vault"),
        El::beam("ask", "agent", "layer").tone(Request),
        El::beam("batch", "layer", "op").tone(Accent),
        El::beam("inject", "layer", "process").tone(Success),
        El::beam("paste", "clipboard", "layer"),
        El::beam("token", "keychain", "layer"),
    ];
    for (id, at, text, tone) in PROMPTS {
        elements.push(
            El::card(id, at, PROMPT, "1Password")
                .tone(tone)
                .statuses(&[(text, tone)]),
        );
    }
    elements.push(
        El::card("prompt-ok", [1210.0, 320.0, -80.0], PROMPT, "1Password")
            .tone(Success)
            .statuses(&[("approved once ✓", Success)]),
    );
    elements.extend([
        El::packet("ask-1", "direct")
            .labeled("op item get \"OpenAI API Key\"")
            .tone(Request),
        El::packet("ask-2", "direct")
            .labeled("op item get \"OpenAI API Key\"")
            .tone(Request),
        El::packet("ask-3", "direct")
            .labeled("op item get --reveal")
            .tone(Request),
        El::packet("leak", "direct")
            .reversed()
            .labeled("sk-proj-7Hq2Zx9mK4pL")
            .tone(Error),
        El::packet("find", "ask")
            .labeled("find openai stripe github")
            .tone(Request),
        El::packet("lookup", "batch")
            .labeled("one batched lookup")
            .tone(Accent),
        El::packet("fetch", "vault-link"),
        El::packet("refs-in", "batch")
            .reversed()
            .labeled("op:// references")
            .tone(Success),
        El::packet("refs", "ask")
            .reversed()
            .labeled("op:// references")
            .tone(Success),
        El::packet("secrets", "inject")
            .labeled("OPENAI_API_KEY")
            .tone(Success),
        El::packet("new-key", "paste").labeled("new key"),
        El::packet("store", "batch")
            .labeled("create, then read back")
            .tone(Accent),
        El::packet("verified", "batch")
            .reversed()
            .labeled("verified ✓")
            .tone(Success),
        El::packet("unlock", "token").labeled("token"),
        El::label(
            "leak-line",
            chat_at,
            22.0,
            &[("chat ▸ ", Muted), ("sk-proj-7Hq2Zx9mK4pLw3eR8vN…", Error)],
        )
        .align(CaptionAlign::Left),
        El::label(
            "refs-line",
            chat_at,
            22.0,
            &[
                ("chat ▸ ", Muted),
                ("op://Personal/OpenAI API Key/credential", Success),
            ],
        )
        .align(CaptionAlign::Left),
        El::label(
            "zero",
            [OP[0], 330.0, -80.0],
            44.0,
            &[("prompts: ", Muted), ("0", Success)],
        ),
        El::label(
            "title",
            [960.0, -470.0, 0.0],
            120.0,
            &[("2password", Accent)],
        ),
        El::label(
            "subtitle",
            [960.0, -360.0, 0.0],
            34.0,
            &[("1Password for coding agents", Plain)],
        ),
        El::label(
            "install",
            [960.0, -262.0, 0.0],
            30.0,
            &[("$ ", Muted), ("bun add -g 2password", Plain)],
        ),
        El::label(
            "skill",
            [960.0, -212.0, 0.0],
            30.0,
            &[
                ("$ ", Muted),
                ("bunx skills add kitlangton/2password", Plain),
            ],
        ),
        El::ring("calm", VAULT, 132.0).thickness(1.3).tone(Success),
        El::ring("calm-outer", VAULT, 138.0)
            .thickness(1.3)
            .tone(Success),
    ]);
    StagePlan {
        post: StagePost::RESTRAINED,
        elements,
    }
}

#[derive(Clone, Copy)]
struct Sfx(&'static str, f64);
const TICK: Sfx = Sfx("visual-effects/task-running.wav", 0.13);
const SEND: Sfx = Sfx("opencode-hot-reload/save.wav", 0.15);
const ALARM: Sfx = Sfx("visual-effects/task-failure.wav", 0.47);
const LAUNCH: Sfx = Sfx("opencode-hot-reload/launch.wav", 1.36);
const IMPACT: Sfx = Sfx("opencode-hot-reload/impact.wav", 0.51);
const DEATH: Sfx = Sfx("visual-effects/task-death.wav", 1.09);
const GLITCH: Sfx = Sfx("pr-walkthrough/glitch.wav", 0.2);
const MARK: Sfx = Sfx("pr-walkthrough/mark.wav", 0.3);
const SUCCESS: Sfx = Sfx("visual-effects/task-success.wav", 0.42);
const CONFIRM: Sfx = Sfx("opencode-hot-reload/confirm.wav", 0.34);
const BLOOM: Sfx = Sfx("effect-shows-errors/prismatic-bloom.wav", 0.785);

fn sfx(id: impl Into<String>, Sfx(file, length): Sfx, gain_db: f32) -> impl Beat {
    sound(
        id,
        PathBuf::from(format!("../../assets/{file}")),
        seconds(length),
        gain_db,
    )
}

fn foot(sc: &mut PlanBuilder, id: &str, parts: &[(&str, Tone)]) -> Result<Caption> {
    Caption::declare(
        sc,
        id,
        &CaptionPlan::line([LEFT, FOOTER_Y], 28.0, spans(parts)),
    )
}

fn film(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(
        seconds(2.0),
        [
            ("problem", seconds(1.6)),
            ("layer", seconds(0.5)),
            ("features", seconds(3.2)),
        ],
    )?;
    let mut sc = PlanBuilder::new("2password-stage", reading.duration());
    let [problem, layer, features] = reading.place(&mut sc);
    let (p, l, f) = (
        |w: &str| problem.at(w),
        |w: &str| layer.at(w),
        |w: &str| features.at(w),
    );

    // Cast of actors (all immutable values; `sc` is the sole mutable builder).
    let s = Stage::declare(&mut sc, "stage", &stage_plan())?;
    let hdr = Caption::declare(
        &mut sc,
        "header",
        &CaptionPlan::line(
            [LEFT, HEADER_Y],
            30.0,
            spans(&[(PR.number, Accent), ("  ", Plain), (PR.title, Plain)]),
        ),
    )?;
    let raw_chip = Caption::from(chip(&mut sc, "chip-raw", Error, "raw op")?);
    let footer_problem = foot(
        &mut sc,
        "footer-problem",
        &[
            ("raw op: ", Plain),
            ("a prompt per call", Error),
            (", secrets in the chat", Plain),
        ],
    )?;
    let rewind_chip = Caption::from(chip(&mut sc, "chip-rewind", Accent, "◀◀ rewind")?);
    let fixed_chip = Caption::from(chip(&mut sc, "chip-fixed", Success, "with 2password")?);
    let footer_layer = foot(
        &mut sc,
        "footer-layer",
        &[
            ("2password: ", Plain),
            ("one approval", Success),
            (", references only", Plain),
        ],
    )?;
    let footer_features = foot(
        &mut sc,
        "footer-features",
        &[
            ("secrets go to processes, ", Plain),
            ("never to the agent", Accent),
        ],
    )?;

    // ── Establish: the vault, the command line, the agent, plugged together ──
    sc.at(
        0,
        all![
            s.channels([
                ("camera.z", -160.0),
                ("camera.dof", 0.45),
                ("vault.scale", 0.58),
                ("vault.blur", 11.0),
                ("vault.rotation", -1.8)
            ]),
            s.to("camera.z", 0.0, 2.2),
            all![
                s.bounce("vault.scale", 1.0, 0.85, 0.2),
                s.to("vault.blur", 0.0, 0.7),
                s.ease("vault.rotation", 0.0, 1.25, Ease::CubicOut),
                s.to("vault.opacity", 1.0, 0.6),
            ]
            .after(seconds(0.15)),
            s.to("vault-name.opacity", 1.0, 0.5).after(seconds(0.9)),
            s.settle_in("op")
                .after(seconds(0.45))
                .then_after(seconds(0.1), s.connect("vault-link", 0.5))
                .on_end(sfx("connect-vault", TICK, -20.0)),
            s.settle_in("agent")
                .after(seconds(0.7))
                .then_after(seconds(0.15), s.connect("direct", 0.75))
                .on_end(sfx("connect-direct", TICK, -20.0)),
            hdr.type_in(55.0, 0.6).after(seconds(0.4)),
            raw_chip.show().after(seconds(0.7)),
        ],
    );

    // ── The problem: prompt, prompt, PROMPT ──
    sc.at(p("needs"), s.to("agent.glow", 0.4, 0.5));
    sc.at(
        p("asks"),
        all![
            s.to("direct.emphasis", 1.0, 0.5),
            s.send("ask-1", 0.8)
                .with(sfx("ask-1", SEND, -10.0))
                .then(s.land("op")),
            s.to("camera.x", 70.0, 1.6),
            s.to("camera.focus", -60.0, 1.2),
        ],
    );

    let demands = [p("allow"), p("approve"), p("authorize"), p("sign in")];
    for (i, ((id, ..), at)) in PROMPTS.iter().zip(demands).enumerate() {
        sc.at(
            at,
            all![
                s.settle_in(*id),
                s.hit(format!("{id}.alarm"), 0.25 + 0.15 * i as f32, 0.0)
                    .after(seconds(0.05)),
                s.glitch(*id, [5.0 + i as f32, 8.0, 6.0])
                    .after(seconds(0.06)),
                sfx(format!("prompt-{i}"), ALARM, -12.0 + 2.0 * i as f32),
            ],
        );
    }
    sc.at(
        p("allow"),
        all![
            s.to("agent.status", 1.0, 0.3),
            s.to("op.status", 1.0, 0.3),
            s.clock("agent.spinner")
        ],
    );
    sc.at(
        p("approve").early(seconds(0.3)),
        s.send("ask-2", 0.55).then(s.land("op")),
    );
    sc.at(
        p("authorize"),
        all![
            s.send("ask-3", 0.45)
                .early(seconds(0.25))
                .then(s.land("op")),
            s.jolt([1.0, 0.3], 0.6),
            s.hit("post.chroma", 0.1, 0.0),
            s.to("camera.z", 30.0, 1.2),
        ],
    );

    // The app locks: the vault cools to red, the command line locks up.
    sc.at(
        p("unlock"),
        all![
            s.to("op.status", 2.0, 0.25),
            s.to("vault.hurt", 0.55, 0.4),
            s.to("vault-link.break", 1.0, 0.9).after(seconds(0.1)),
            s.hit("op.alarm", 0.5, 0.15),
            sfx("unlock", GLITCH, -14.0),
        ],
    );
    sc.at(p("sign in"), s.clock("op.spinner"));

    // STUCK: everything glitches at once and the frame takes the blow.
    let stuck_cards = [
        "op", "agent", "prompt-1", "prompt-2", "prompt-3", "prompt-4",
    ];
    sc.at(
        p("stuck"),
        all![
            s.to("agent.status", 2.0, 0.2),
            stagger(seconds(0.035), stuck_cards, |c| {
                all![
                    s.glitch(c, [7.0, 9.0, 8.0]),
                    s.glitch(c, [9.0, 6.0, 7.0]).after(seconds(0.32)),
                    s.set(format!("{c}.damage"), 1.0),
                ]
            }),
            s.jolt([-0.4, 1.0], 1.0),
            s.hit("post.chroma", 0.2, 0.0),
            s.hit("post.bloom", 0.35, 0.18),
            sfx("stuck-impact", IMPACT, -7.0),
            sfx("stuck-glitch", GLITCH, -10.0).after(seconds(0.05)),
        ],
    );

    // "When it finally works": the dialogs give way, one by one, into silence.
    sc.at(
        p("finally"),
        all![
            stagger_indexed(seconds(0.09), PROMPTS, |_, (id, ..)| {
                s.to(format!("{id}.opacity"), 0.0, 0.35)
                    .also(s.set(format!("{id}.damage"), 0.0))
            }),
            s.set("op.damage", 0.0),
            s.set("agent.damage", 0.0),
            s.to("op.status", 3.0, 0.3),
            s.set("op.spinner", -1.0),
            s.to("vault.hurt", 0.0, 0.6),
            s.to("vault-link.break", 0.0, 0.9),
            s.to("camera.x", -60.0, 1.6),
            s.to("camera.z", 20.0, 1.6),
        ],
    );

    // ...and prints the secret RIGHT INTO THE CHAT.
    sc.at(
        p("prints"),
        all![
            s.to("camera.x", LEAK_CAMERA[0], 1.1),
            s.to("camera.z", LEAK_CAMERA[2], 1.1),
            s.send("leak", 0.6)
                .with(sfx("leak-send", LAUNCH, -16.0).early(seconds(0.35)))
                .then(all![
                    s.hit("agent.alarm", 0.6, 0.2),
                    s.to("agent.status", 3.0, 0.2),
                    s.set("agent.spinner", -1.0),
                    s.type_in("leak-line", 80.0),
                ]),
        ],
    );
    sc.at(
        p("chat"),
        all![
            s.jolt([-1.0, 0.0], 1.0),
            s.hit("post.chroma", 0.28, 0.0),
            s.hit("post.bloom", 0.4, 0.18),
            s.hit("agent.alarm", 0.9, 0.3),
            sfx("chat-impact", IMPACT, -5.0),
            sfx("chat-death", DEATH, -9.0).after(seconds(0.05)),
            footer_problem.type_in(46.0, 0.8).after(seconds(0.3)),
        ],
    );

    // ── Rewind ──
    sc.at(
        problem.end() + seconds(0.25),
        all![
            s.clock_for("post.rewind", 1.4),
            s.hit("post.chroma", 0.12, 0.0),
            sfx("rewind", LAUNCH, -14.0).early(seconds(0.1)),
            raw_chip.hide(),
            footer_problem.hide(),
            rewind_chip.show().after(seconds(0.25)),
            rewind_chip.hide().after(seconds(1.5)),
            fixed_chip.show().after(seconds(1.65)),
            s.to("leak-line.opacity", 0.0, 0.4),
            s.to("agent.alarm", 0.0, 0.4),
            s.to("agent.status", 0.0, 0.9).after(seconds(0.2)),
            s.to("agent.glow", 0.0, 0.6),
            s.to("op.status", 0.0, 0.6).after(seconds(0.2)),
            s.to("direct.emphasis", 0.0, 0.6),
            s.to("camera.x", 0.0, 1.6),
            s.to("camera.z", 0.0, 1.8),
            s.to("camera.focus", 0.0, 1.0),
        ],
    );

    // ── The layer: the direct wire unplugs, 2password settles in between ──
    let meet = l("meet");
    sc.at(
        meet.early(seconds(0.3)),
        s.ease("direct.draw", 0.0, 0.35, DRAW_CURVE).also(
            s.ease("direct.port", 0.0, 0.3, Ease::Smootherstep)
                .after(seconds(0.25)),
        ),
    );
    let layer_ready = sc.at(
        meet.after(seconds(0.1)),
        s.settle_in("layer").with(sfx("meet", BLOOM, -12.0)),
    );
    let ask_contact = sc.at(layer_ready, s.connect("ask", 0.45));
    let batch_contact = sc.at(layer_ready.after(seconds(0.15)), s.connect("batch", 0.45));
    for (b, at) in [("ask", ask_contact), ("batch", batch_contact)] {
        sc.at(
            at,
            all![
                s.hit(format!("{b}.surge"), 0.45, 0.0),
                s.twang(b),
                sfx(format!("connect-{b}"), TICK, -19.0)
            ],
        );
    }
    sc.at(
        l("tiny layer"),
        all![
            s.to("layer.glow", 0.55, 0.6),
            s.hit("layer.flash", 0.45, 0.0),
            s.to("camera.x", -60.0, 1.6),
            s.to("camera.z", 60.0, 1.8),
        ],
    );

    // The agent asks for everything at once; 2password asks op once.
    let find = sc.at(
        l("everything"),
        s.to("agent.status", 4.0, 0.3).also(
            s.send("find", 0.6)
                .with(sfx("find", SEND, -11.0))
                .early(seconds(0.2))
                .then(s.land("layer").also(s.to("layer.status", 1.0, 0.3))),
        ),
    );
    let lookup = sc.at(
        l("just once")
            .early(seconds(0.35))
            .not_before(find.after(seconds(0.42))),
        s.send("lookup", 0.55)
            .with(sfx("lookup", SEND, -11.0))
            .then(s.land("op")),
    );
    let fetch = sc.at(
        lookup.after(seconds(0.42)),
        s.send("fetch", 0.45).then(s.land("vault")),
    );

    // ONE approval.
    sc.at(
        l("one approval"),
        all![
            s.settle_in("prompt-ok"),
            s.hit("prompt-ok.flash", 0.6, 0.0).after(seconds(0.2)),
            s.to("op.status", 4.0, 0.3),
            sfx("approval", SUCCESS, -8.0),
            s.to("camera.x", 60.0, 1.6),
        ],
    );

    // References come back; the secret itself never does.
    let refs_in = sc.at(
        l("references")
            .early(seconds(0.9))
            .not_before(fetch.after(seconds(0.42))),
        s.send("refs-in", 0.5)
            .then(s.land("layer").also(s.to("prompt-ok.opacity", 0.0, 0.4))),
    );
    let refs = sc.at(
        refs_in.after(seconds(0.42)),
        s.send("refs", 0.55).on_end(all![
            s.land("agent"),
            s.to("agent.status", 5.0, 0.3),
            s.to("layer.status", 2.0, 0.3),
            s.type_in("refs-line", 60.0),
            sfx("refs", MARK, -14.0),
        ]),
    );
    sc.at(refs.early(seconds(0.4)), s.to("camera.x", -40.0, 1.6));
    sc.at(l("never"), footer_layer.type_in(46.0, 0.8));

    // ── Features: inject, save, and a service account ──
    let inject_contact = sc.at(
        f("runs"),
        all![
            footer_layer.hide(),
            s.to("camera.x", 0.0, 1.6),
            s.to("camera.z", -130.0, 1.8),
            s.settle_in("process")
                .then(s.connect("inject", 0.4))
                .on_end(sfx("connect-inject", TICK, -19.0)),
        ],
    );
    sc.at(
        f("injected").not_before(inject_contact.after(seconds(0.34))),
        s.send("secrets", 0.5).then(all![
            s.land("process"),
            s.to("process.status", 1.0, 0.3),
            s.to("layer.status", 3.0, 0.3),
            sfx("secrets", CONFIRM, -12.0),
        ]),
    );

    let paste_contact = sc.at(
        f("saves"),
        s.settle_in("clipboard")
            .then(s.connect("paste", 0.4))
            .on_end(sfx("connect-paste", TICK, -19.0)),
    );
    let new_key = sc.at(
        f("clipboard").not_before(paste_contact.after(seconds(0.34))),
        s.send("new-key", 0.5).then(s.land("layer")),
    );
    let store = sc.at(
        f("checks").not_before(new_key.after(seconds(0.42))),
        s.send("store", 0.5).then(s.land("op")),
    );
    sc.at(
        f("landed").not_before(store.after(seconds(0.42))),
        s.send("verified", 0.5).then(all![
            s.land("layer"),
            s.to("layer.status", 4.0, 0.3),
            sfx("verified", SUCCESS, -11.0),
        ]),
    );
    sc.at(f("checks"), footer_features.type_in(46.0, 0.8));

    sc.at(
        f("service account"),
        s.settle_in("keychain")
            .then(s.connect("token", 0.4))
            .on_end(sfx("connect-token", TICK, -19.0))
            .then_after(seconds(0.34), s.send("unlock", 0.45))
            .then(s.land("layer")),
    );

    // No prompts. At all. Everything breathes.
    let active_beams = ["ask", "batch", "inject", "paste", "token", "vault-link"];
    let calm_cards = ["agent", "op", "process", "clipboard", "keychain"];
    sc.at(
        f("no prompts"),
        s.to("layer.status", 5.0, 0.3).also(s.type_in("zero", 30.0)),
    );
    let all = f("at all");
    sc.at(
        all,
        all![
            s.hit("vault.pulse", 0.85, 0.0).after(seconds(0.15)),
            s.ease("vault.rotation", 1.6, 2.4, Ease::CubicOut),
            s.to("calm.opacity", 0.35, 0.22),
            s.to("calm-outer.opacity", 0.5, 0.22).after(seconds(0.06)),
            s.hit("post.bloom", 0.3, 0.18),
            s.to("camera.z", -60.0, 2.0),
            each(active_beams, |b| s.to(format!("{b}.flow"), 0.5, 0.6)),
            each(calm_cards, |c| s.hit(format!("{c}.flash"), 0.35, 0.0)).after(seconds(0.1)),
            sfx("all", BLOOM, -8.0).after(seconds(0.1)),
        ],
    );

    // ── Let your agent fly: the camera rises to the install lines ──
    sc.at(
        all + seconds(1.6),
        all![
            each(["camera.x", "camera.z", "camera.focus"], |prop| s
                .to(prop, 0.0, 2.0)),
            s.to("camera.y", TITLE_CAMERA_Y, 2.4),
            s.to("ask.opacity", 0.0, 0.5),
            all![
                s.bounce("agent.x", AGENT_FLIGHT[0], 2.6, 0.12),
                s.bounce("agent.y", AGENT_FLIGHT[1], 2.6, 0.12),
            ]
            .after(seconds(0.3)),
            fixed_chip.hide(),
            footer_features.hide(),
        ],
    );
    sc.at(
        f("install").early(seconds(0.6)),
        all![
            s.channel("title.scale", 0.86),
            s.bounce("title.scale", 1.0, 0.8, 0.2),
            s.to("title.opacity", 1.0, 0.4),
            s.to("subtitle.opacity", 1.0, 0.5).after(seconds(0.3)),
            sfx("title", CONFIRM, -12.0),
        ],
    );
    sc.at(f("install"), s.type_in("install", 40.0));
    sc.at(f("skill").early(seconds(0.2)), s.type_in("skill", 48.0));
    sc.at(
        f("fly"),
        all![
            s.hit("post.bloom", 0.25, 0.18),
            sfx("fly", BLOOM, -10.0),
            each(active_beams, |b| s.to(format!("{b}.flow"), 0.0, 1.2)),
        ],
    );

    sc.finish().context("2password-stage")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_password_dsl_emits_byte_identical_scene_plan_json() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let narration = Narration::load(&root.join("narration")).unwrap();
        let dsl_reel = ReelPlan {
            version: ReelPlan::VERSION,
            id: "2password".to_owned(),
            segments: vec![ReelSegmentPlan {
                transition_nanos: 0,
                transition_style: ReelTransitionStyle::Dip,
                transition_focus: None,
                transition_wipe: None,
                plan: film(&narration).unwrap(),
            }],
        };
        dsl_reel.validate().unwrap();

        let orig_json = fs::read_to_string(root.join("2password.reel.json")).unwrap();
        let dsl_json = serde_json::to_string_pretty(&dsl_reel).unwrap() + "\n";
        assert_eq!(dsl_json, orig_json);
    }
}
