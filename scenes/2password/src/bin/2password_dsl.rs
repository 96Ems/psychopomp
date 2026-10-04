//! `scenes/2password` re-expressed with `psychopomp::score` and `psychopomp::layout`.
//!
//! Proves that a full multi-act narrated Stage film (speech cues, causal
//! `not_before` joins, pre-rolled packets, staggered glitches, and relative
//! envelope placement) compiles to byte-for-byte identical `ScenePlan` JSON.
use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use psychopomp::{
    all,
    author::{PlanBuilder, seconds},
    caption::{CaptionAlign, CaptionSpanPlan},
    effects::spinner::Mark,
    layout::Placement,
    math::easing::Ease,
    narration::Narration,
    plan::{ReelPlan, ReelSegmentPlan, ReelTransitionStyle, ScenePlan},
    score::{Beat, CueTime, StageCtx, stage, stagger, stagger_indexed},
    stage::{DRAW_CURVE, StageActor, StageElement, StagePlan, StagePost, StatusText},
    tone::Tone,
};
use psychopomp_pr_walkthrough::film::{Pr, chip, footer, header, span};

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
    (
        "prompt-1",
        [1210.0, 300.0, -80.0],
        "Allow access?",
        Tone::Warning,
    ),
    (
        "prompt-2",
        [1590.0, 320.0, -130.0],
        "Approve this request?",
        Tone::Warning,
    ),
    (
        "prompt-3",
        [1210.0, 790.0, -300.0],
        "AUTHORIZE?!",
        Tone::Error,
    ),
    (
        "prompt-4",
        [790.0, 310.0, -120.0],
        "Sign in to continue",
        Tone::Error,
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
    let output = PathBuf::from("target/2password-dsl.reel.json");
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, serde_json::to_string_pretty(&reel)? + "\n")?;
    Ok(())
}

fn status(text: &str, tone: Tone) -> StatusText {
    StatusText {
        text: text.to_owned(),
        tone,
    }
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
        mark: Mark::Check,
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
        spans: parts
            .iter()
            .map(|(text, tone)| CaptionSpanPlan::new(*text, *tone))
            .collect(),
    }
}

fn ring(id: &str, radius: f32) -> StageElement {
    StageElement::Ring {
        id: id.into(),
        at: VAULT,
        radius,
        thickness: 1.3,
        tone: Tone::Success,
    }
}

fn stage_plan() -> StagePlan {
    let vault_box = Placement::orb(VAULT, 118.0);
    let agent_box = Placement::card(AGENT, CARD);
    let chat_at = agent_box.align_left(4.0, -100.0);

    let mut elements = vec![
        StageElement::Orb {
            id: "vault".into(),
            at: VAULT,
            radius: 118.0,
            points: 900,
            tone: Tone::Plain,
        },
        label(
            "vault-name",
            vault_box.below(46.0),
            24.0,
            CaptionAlign::Center,
            &[("1Password", Tone::Plain)],
        ),
        card(
            "agent",
            AGENT,
            CARD,
            "coding agent",
            vec![
                status("needs one API key", Tone::Plain),
                status("waiting for approval…", Tone::Warning),
                status("stuck", Tone::Error),
                status("secret in the chat!", Tone::Error),
                status("asking 2password", Tone::Plain),
                status("references only ✓", Tone::Success),
            ],
            Tone::Request,
        ),
        card(
            "op",
            OP,
            [330.0, 124.0],
            "op · 1Password CLI",
            vec![
                status("ready", Tone::Muted),
                status("approval required", Tone::Warning),
                status("locked", Tone::Error),
                status("ready", Tone::Muted),
                status("approved once ✓", Tone::Success),
            ],
            Tone::Plain,
        ),
        card(
            "layer",
            LAYER,
            CARD,
            "2password",
            vec![
                status("a layer on top of op", Tone::Muted),
                status("batching every lookup", Tone::Plain),
                status("references only", Tone::Success),
                status("injecting secrets", Tone::Plain),
                status("verified ✓", Tone::Success),
                status("no prompts", Tone::Success),
            ],
            Tone::Accent,
        ),
        card(
            "process",
            PROCESS,
            SMALL,
            "bun dev",
            vec![
                status("starting", Tone::Muted),
                status("running with secrets ✓", Tone::Success),
            ],
            Tone::Plain,
        ),
        card(
            "clipboard",
            CLIPBOARD,
            SMALL,
            "clipboard",
            vec![status("a new API key", Tone::Muted)],
            Tone::Plain,
        ),
        card(
            "keychain",
            KEYCHAIN,
            SMALL,
            "macOS Keychain",
            vec![status("service-account token", Tone::Muted)],
            Tone::Plain,
        ),
        beam("direct", "agent", "op", Tone::Request),
        beam("vault-link", "op", "vault", Tone::Plain),
        beam("ask", "agent", "layer", Tone::Request),
        beam("batch", "layer", "op", Tone::Accent),
        beam("inject", "layer", "process", Tone::Success),
        beam("paste", "clipboard", "layer", Tone::Plain),
        beam("token", "keychain", "layer", Tone::Plain),
    ];
    for (id, at, text, tone) in PROMPTS {
        elements.push(card(
            id,
            at,
            PROMPT,
            "1Password",
            vec![status(text, tone)],
            tone,
        ));
    }
    elements.push(card(
        "prompt-ok",
        [1210.0, 320.0, -80.0],
        PROMPT,
        "1Password",
        vec![status("approved once ✓", Tone::Success)],
        Tone::Success,
    ));
    elements.extend([
        packet(
            "ask-1",
            "direct",
            false,
            "op item get \"OpenAI API Key\"",
            Tone::Request,
        ),
        packet(
            "ask-2",
            "direct",
            false,
            "op item get \"OpenAI API Key\"",
            Tone::Request,
        ),
        packet(
            "ask-3",
            "direct",
            false,
            "op item get --reveal",
            Tone::Request,
        ),
        packet("leak", "direct", true, "sk-proj-7Hq2Zx9mK4pL", Tone::Error),
        packet(
            "find",
            "ask",
            false,
            "find openai stripe github",
            Tone::Request,
        ),
        packet("lookup", "batch", false, "one batched lookup", Tone::Accent),
        packet("fetch", "vault-link", false, "", Tone::Plain),
        packet("refs-in", "batch", true, "op:// references", Tone::Success),
        packet("refs", "ask", true, "op:// references", Tone::Success),
        packet("secrets", "inject", false, "OPENAI_API_KEY", Tone::Success),
        packet("new-key", "paste", false, "new key", Tone::Plain),
        packet(
            "store",
            "batch",
            false,
            "create, then read back",
            Tone::Accent,
        ),
        packet("verified", "batch", true, "verified ✓", Tone::Success),
        packet("unlock", "token", false, "token", Tone::Plain),
        label(
            "leak-line",
            chat_at,
            22.0,
            CaptionAlign::Left,
            &[
                ("chat ▸ ", Tone::Muted),
                ("sk-proj-7Hq2Zx9mK4pLw3eR8vN…", Tone::Error),
            ],
        ),
        label(
            "refs-line",
            chat_at,
            22.0,
            CaptionAlign::Left,
            &[
                ("chat ▸ ", Tone::Muted),
                ("op://Personal/OpenAI API Key/credential", Tone::Success),
            ],
        ),
        label(
            "zero",
            [OP[0], 330.0, -80.0],
            44.0,
            CaptionAlign::Center,
            &[("prompts: ", Tone::Muted), ("0", Tone::Success)],
        ),
        label(
            "title",
            [960.0, -470.0, 0.0],
            120.0,
            CaptionAlign::Center,
            &[("2password", Tone::Accent)],
        ),
        label(
            "subtitle",
            [960.0, -360.0, 0.0],
            34.0,
            CaptionAlign::Center,
            &[("1Password for coding agents", Tone::Plain)],
        ),
        label(
            "install",
            [960.0, -262.0, 0.0],
            30.0,
            CaptionAlign::Center,
            &[("$ ", Tone::Muted), ("bun add -g 2password", Tone::Plain)],
        ),
        label(
            "skill",
            [960.0, -212.0, 0.0],
            30.0,
            CaptionAlign::Center,
            &[
                ("$ ", Tone::Muted),
                ("bunx skills add kitlangton/2password", Tone::Plain),
            ],
        ),
        ring("calm", 132.0),
        ring("calm-outer", 138.0),
    ]);
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

fn sfx<'a>(id: impl Into<String>, Sfx(file, length): Sfx, gain_db: f32) -> impl Beat<StageCtx<'a>> {
    stage::sound(
        id,
        PathBuf::from(format!("../../assets/{file}")),
        seconds(length),
        gain_db,
    )
}

fn film(narration: &Narration) -> Result<ScenePlan> {
    let problem_clip = narration.clip("problem")?;
    let layer_clip = narration.clip("layer")?;
    let features_clip = narration.clip("features")?;
    let lead = seconds(2.0);
    let rewind = seconds(1.6);
    let duration = lead
        + problem_clip.duration()
        + rewind
        + layer_clip.duration()
        + seconds(0.5)
        + features_clip.duration()
        + seconds(3.2);
    let mut scene = PlanBuilder::new("2password-stage", duration);
    let problem = problem_clip.place(&mut scene, lead);
    let layer = layer_clip.place(&mut scene, problem.end() + rewind);
    let features = features_clip.place(&mut scene, layer.end() + seconds(0.5));
    let p = |phrase: &str| problem.at(phrase);
    let l = |phrase: &str| layer.at(phrase);
    let f = |phrase: &str| features.at(phrase);
    let mut stage_actor = StageActor::declare(&mut scene, "stage", &stage_plan())?;

    // ── Establish: the vault, the command line, the agent, plugged together ──
    stage_actor.score(&mut scene).at(
        0,
        all![
            stage::channels([
                ("camera.z", -160.0),
                ("camera.dof", 0.45),
                ("vault.scale", 0.58),
                ("vault.blur", 11.0),
                ("vault.rotation", -1.8),
            ]),
            stage::to("camera.z", 0.0, 2.2),
            all![
                stage::bounce("vault.scale", 1.0, 0.85, 0.2),
                stage::to("vault.blur", 0.0, 0.7),
                stage::ease("vault.rotation", 0.0, 1.25, Ease::CubicOut),
                stage::to("vault.opacity", 1.0, 0.6),
            ]
            .after(seconds(0.15)),
            stage::to("vault-name.opacity", 1.0, 0.5).after(seconds(0.9)),
            stage::settle_in("op")
                .after(seconds(0.45))
                .then_after(seconds(0.1), stage::connect("vault-link", 0.5))
                .on_end(sfx("connect-vault", TICK, -20.0)),
            stage::settle_in("agent")
                .after(seconds(0.7))
                .then_after(seconds(0.15), stage::connect("direct", 0.75))
                .on_end(sfx("connect-direct", TICK, -20.0)),
        ],
    );
    header(&mut scene, &PR, Some(seconds(0.4)))?;
    let mut raw_chip = chip(&mut scene, "chip-raw", Tone::Error, "raw op")?;
    raw_chip.show(&mut scene, seconds(0.7));

    // ── The problem: prompt, prompt, PROMPT ──
    let mut sc = stage_actor.score(&mut scene);
    sc.at(p("needs"), stage::to("agent.glow", 0.4, 0.5));
    sc.at(
        p("asks"),
        all![
            stage::to("direct.emphasis", 1.0, 0.5),
            stage::send("ask-1", 0.8)
                .with(sfx("ask-1", SEND, -10.0))
                .then(stage::land("op")),
            stage::to("camera.x", 70.0, 1.6),
            stage::to("camera.focus", -60.0, 1.2),
        ],
    );

    let demands = [p("allow"), p("approve"), p("authorize"), p("sign in")];
    for (index, ((id, ..), at)) in PROMPTS.iter().zip(demands).enumerate() {
        sc.at(
            at,
            all![
                stage::settle_in(*id),
                stage::hit(format!("{id}.alarm"), 0.25 + 0.15 * index as f32, 0.0)
                    .after(seconds(0.05)),
                stage::glitch(*id, [5.0 + index as f32, 8.0, 6.0]).after(seconds(0.06)),
                sfx(format!("prompt-{index}"), ALARM, -12.0 + 2.0 * index as f32),
            ],
        );
    }
    sc.at(
        p("allow"),
        all![
            stage::to("agent.status", 1.0, 0.3),
            stage::to("op.status", 1.0, 0.3),
            stage::clock("agent.spinner"),
        ],
    );
    sc.at(
        p("approve").early(seconds(0.3)),
        stage::send("ask-2", 0.55).then(stage::land("op")),
    );
    sc.at(
        p("authorize"),
        all![
            stage::send("ask-3", 0.45)
                .early(seconds(0.25))
                .then(stage::land("op")),
            stage::jolt([1.0, 0.3], 0.6),
            stage::hit("post.chroma", 0.1, 0.0),
            stage::to("camera.z", 30.0, 1.2),
        ],
    );

    // The app locks: the vault cools to red, the command line locks up.
    sc.at(
        p("unlock"),
        all![
            stage::to("op.status", 2.0, 0.25),
            stage::to("vault.hurt", 0.55, 0.4),
            stage::to("vault-link.break", 1.0, 0.9).after(seconds(0.1)),
            stage::hit("op.alarm", 0.5, 0.15),
            sfx("unlock", GLITCH, -14.0),
        ],
    );
    sc.at(p("sign in"), stage::clock("op.spinner"));

    // STUCK: everything glitches at once and the frame takes the blow.
    sc.at(
        p("stuck"),
        all![
            stage::to("agent.status", 2.0, 0.2),
            stagger(
                seconds(0.035),
                [
                    "op", "agent", "prompt-1", "prompt-2", "prompt-3", "prompt-4"
                ],
                |card| {
                    all![
                        stage::glitch(card, [7.0, 9.0, 8.0]),
                        stage::glitch(card, [9.0, 6.0, 7.0]).after(seconds(0.32)),
                        stage::set(format!("{card}.damage"), 1.0),
                    ]
                },
            ),
            stage::jolt([-0.4, 1.0], 1.0),
            stage::hit("post.chroma", 0.2, 0.0),
            stage::hit("post.bloom", 0.35, 0.18),
            sfx("stuck-impact", IMPACT, -7.0),
            sfx("stuck-glitch", GLITCH, -10.0).after(seconds(0.05)),
        ],
    );

    // "When it finally works": the dialogs give way, one by one, into silence.
    sc.at(
        p("finally"),
        all![
            stagger_indexed(seconds(0.09), PROMPTS, |_, (id, ..)| {
                stage::to(format!("{id}.opacity"), 0.0, 0.35)
                    .also(stage::set(format!("{id}.damage"), 0.0))
            }),
            stage::set("op.damage", 0.0),
            stage::set("agent.damage", 0.0),
            stage::to("op.status", 3.0, 0.3),
            stage::set("op.spinner", -1.0),
            stage::to("vault.hurt", 0.0, 0.6),
            stage::to("vault-link.break", 0.0, 0.9),
            stage::to("camera.x", -60.0, 1.6),
            stage::to("camera.z", 20.0, 1.6),
        ],
    );

    // ...and prints the secret RIGHT INTO THE CHAT.
    sc.at(
        p("prints"),
        all![
            stage::to("camera.x", LEAK_CAMERA[0], 1.1),
            stage::to("camera.z", LEAK_CAMERA[2], 1.1),
            stage::send("leak", 0.6)
                .with(sfx("leak-send", LAUNCH, -16.0).early(seconds(0.35)))
                .then(all![
                    stage::hit("agent.alarm", 0.6, 0.2),
                    stage::to("agent.status", 3.0, 0.2),
                    stage::set("agent.spinner", -1.0),
                    stage::type_in("leak-line", 80.0),
                ]),
        ],
    );
    let chat = p("chat");
    sc.at(
        chat,
        all![
            stage::jolt([-1.0, 0.0], 1.0),
            stage::hit("post.chroma", 0.28, 0.0),
            stage::hit("post.bloom", 0.4, 0.18),
            stage::hit("agent.alarm", 0.9, 0.3),
            sfx("chat-impact", IMPACT, -5.0),
            sfx("chat-death", DEATH, -9.0).after(seconds(0.05)),
        ],
    );
    let mut footer_problem = footer(
        &mut scene,
        "footer-problem",
        vec![
            span("raw op: ", Tone::Plain),
            span("a prompt per call", Tone::Error),
            span(", secrets in the chat", Tone::Plain),
        ],
    )?;
    footer_problem.type_in(&mut scene, chat + seconds(0.3), 46.0, 0.8);

    // ── Rewind ──
    let switch = problem.end() + seconds(0.25);
    stage_actor.score(&mut scene).at(
        switch,
        all![
            stage::clock_for("post.rewind", 1.4),
            stage::hit("post.chroma", 0.12, 0.0),
            sfx("rewind", LAUNCH, -14.0).early(seconds(0.1)),
        ],
    );
    raw_chip.hide(&mut scene, switch);
    footer_problem.hide(&mut scene, switch);
    let mut rewind_chip = chip(&mut scene, "chip-rewind", Tone::Accent, "◀◀ rewind")?;
    rewind_chip.show(&mut scene, switch + seconds(0.25));
    rewind_chip.hide(&mut scene, switch + seconds(1.5));
    let mut fixed_chip = chip(&mut scene, "chip-fixed", Tone::Success, "with 2password")?;
    fixed_chip.show(&mut scene, switch + seconds(1.65));
    let mut sc = stage_actor.score(&mut scene);
    sc.at(
        switch,
        all![
            stage::to("leak-line.opacity", 0.0, 0.4),
            stage::to("agent.alarm", 0.0, 0.4),
            stage::to("agent.status", 0.0, 0.9).after(seconds(0.2)),
            stage::to("agent.glow", 0.0, 0.6),
            stage::to("op.status", 0.0, 0.6).after(seconds(0.2)),
            stage::to("direct.emphasis", 0.0, 0.6),
            stage::to("camera.x", 0.0, 1.6),
            stage::to("camera.z", 0.0, 1.8),
            stage::to("camera.focus", 0.0, 1.0),
        ],
    );

    // ── The layer: the direct wire unplugs, 2password settles in between ──
    let meet = l("meet");
    sc.at(
        meet.early(seconds(0.3)),
        all![
            stage::ease("direct.draw", 0.0, 0.35, DRAW_CURVE),
            stage::ease("direct.port", 0.0, 0.3, Ease::Smootherstep).after(seconds(0.25)),
        ],
    );
    let layer_ready = sc.at(
        meet.after(seconds(0.1)),
        stage::settle_in("layer").with(sfx("meet", BLOOM, -12.0)),
    );
    let ask_contact = sc.at(layer_ready, stage::connect("ask", 0.45));
    let batch_contact = sc.at(
        layer_ready.after(seconds(0.15)),
        stage::connect("batch", 0.45),
    );
    for (beam, at) in [("ask", ask_contact), ("batch", batch_contact)] {
        sc.at(
            at,
            all![
                stage::hit(format!("{beam}.surge"), 0.45, 0.0),
                stage::twang(beam),
                sfx(format!("connect-{beam}"), TICK, -19.0),
            ],
        );
    }
    sc.at(
        l("tiny layer"),
        all![
            stage::to("layer.glow", 0.55, 0.6),
            stage::hit("layer.flash", 0.45, 0.0),
            stage::to("camera.x", -60.0, 1.6),
            stage::to("camera.z", 60.0, 1.8),
        ],
    );

    // The agent asks for everything at once; 2password asks op once.
    let everything = l("everything");
    let find = sc.at(
        everything,
        stage::to("agent.status", 4.0, 0.3).also(
            stage::send("find", 0.6)
                .with(sfx("find", SEND, -11.0))
                .early(seconds(0.2))
                .then(stage::land("layer").also(stage::to("layer.status", 1.0, 0.3))),
        ),
    );
    let lookup = sc.at(
        l("just once")
            .early(seconds(0.35))
            .not_before(find.after(seconds(0.42))),
        stage::send("lookup", 0.55)
            .with(sfx("lookup", SEND, -11.0))
            .then(stage::land("op")),
    );
    let fetch = sc.at(
        lookup.after(seconds(0.42)),
        stage::send("fetch", 0.45).then(stage::land("vault")),
    );

    // ONE approval.
    sc.at(
        l("one approval"),
        all![
            stage::settle_in("prompt-ok"),
            stage::hit("prompt-ok.flash", 0.6, 0.0).after(seconds(0.2)),
            stage::to("op.status", 4.0, 0.3),
            sfx("approval", SUCCESS, -8.0),
            stage::to("camera.x", 60.0, 1.6),
        ],
    );

    // References come back; the secret itself never does.
    let refs_in = sc.at(
        l("references")
            .early(seconds(0.9))
            .not_before(fetch.after(seconds(0.42))),
        stage::send("refs-in", 0.5).then(stage::land("layer").also(stage::to(
            "prompt-ok.opacity",
            0.0,
            0.4,
        ))),
    );
    let refs = sc.at(
        refs_in.after(seconds(0.42)),
        stage::send("refs", 0.55).on_end(all![
            stage::land("agent"),
            stage::to("agent.status", 5.0, 0.3),
            stage::to("layer.status", 2.0, 0.3),
            stage::type_in("refs-line", 60.0),
            sfx("refs", MARK, -14.0),
        ]),
    );
    sc.at(refs.early(seconds(0.4)), stage::to("camera.x", -40.0, 1.6));
    let mut footer_layer = footer(
        &mut scene,
        "footer-layer",
        vec![
            span("2password: ", Tone::Plain),
            span("one approval", Tone::Success),
            span(", references only", Tone::Plain),
        ],
    )?;
    footer_layer.type_in(&mut scene, l("never"), 46.0, 0.8);

    // ── Features: inject, save, and a service account ──
    footer_layer.hide(&mut scene, f("runs"));
    let mut sc = stage_actor.score(&mut scene);
    let inject_contact = sc.at(
        f("runs"),
        all![
            stage::to("camera.x", 0.0, 1.6),
            stage::to("camera.z", -130.0, 1.8),
            stage::settle_in("process")
                .then(stage::connect("inject", 0.4))
                .on_end(sfx("connect-inject", TICK, -19.0)),
        ],
    );
    sc.at(
        f("injected").not_before(inject_contact.after(seconds(0.34))),
        stage::send("secrets", 0.5).then(all![
            stage::land("process"),
            stage::to("process.status", 1.0, 0.3),
            stage::to("layer.status", 3.0, 0.3),
            sfx("secrets", CONFIRM, -12.0),
        ]),
    );

    let paste_contact = sc.at(
        f("saves"),
        stage::settle_in("clipboard")
            .then(stage::connect("paste", 0.4))
            .on_end(sfx("connect-paste", TICK, -19.0)),
    );
    let new_key = sc.at(
        f("clipboard").not_before(paste_contact.after(seconds(0.34))),
        stage::send("new-key", 0.5).then(stage::land("layer")),
    );
    let store = sc.at(
        f("checks").not_before(new_key.after(seconds(0.42))),
        stage::send("store", 0.5).then(stage::land("op")),
    );
    sc.at(
        f("landed").not_before(store.after(seconds(0.42))),
        stage::send("verified", 0.5).then(all![
            stage::land("layer"),
            stage::to("layer.status", 4.0, 0.3),
            sfx("verified", SUCCESS, -11.0),
        ]),
    );
    let mut footer_features = footer(
        &mut scene,
        "footer-features",
        vec![
            span("secrets go to processes, ", Tone::Plain),
            span("never to the agent", Tone::Accent),
        ],
    )?;
    footer_features.type_in(&mut scene, f("checks"), 46.0, 0.8);

    let mut sc = stage_actor.score(&mut scene);
    sc.at(
        f("service account"),
        stage::settle_in("keychain")
            .then(stage::connect("token", 0.4))
            .on_end(sfx("connect-token", TICK, -19.0))
            .then_after(seconds(0.34), stage::send("unlock", 0.45))
            .then(stage::land("layer")),
    );

    // No prompts. At all. Everything breathes.
    sc.at(
        f("no prompts"),
        stage::to("layer.status", 5.0, 0.3).also(stage::type_in("zero", 30.0)),
    );
    let all = f("at all");
    sc.at(
        all,
        all![
            stage::hit("vault.pulse", 0.85, 0.0).after(seconds(0.15)),
            stage::ease("vault.rotation", 1.6, 2.4, Ease::CubicOut),
            stage::to("calm.opacity", 0.35, 0.22),
            stage::to("calm-outer.opacity", 0.5, 0.22).after(seconds(0.06)),
            stage::hit("post.bloom", 0.3, 0.18),
            stage::to("camera.z", -60.0, 2.0),
            stagger(
                0,
                ["ask", "batch", "inject", "paste", "token", "vault-link"],
                |beam| stage::to(format!("{beam}.flow"), 0.5, 0.6),
            ),
            stagger(
                0,
                ["agent", "op", "process", "clipboard", "keychain"],
                |card| stage::hit(format!("{card}.flash"), 0.35, 0.0),
            )
            .after(seconds(0.1)),
            sfx("all", BLOOM, -8.0).after(seconds(0.1)),
        ],
    );

    // ── Let your agent fly: the camera rises to the install lines ──
    let rise = all + seconds(1.6);
    sc.at(
        rise,
        all![
            stagger(0, ["camera.x", "camera.z", "camera.focus"], |prop| {
                stage::to(prop, 0.0, 2.0)
            }),
            stage::to("camera.y", TITLE_CAMERA_Y, 2.4),
            stage::to("ask.opacity", 0.0, 0.5),
            all![
                stage::bounce("agent.x", AGENT_FLIGHT[0], 2.6, 0.12),
                stage::bounce("agent.y", AGENT_FLIGHT[1], 2.6, 0.12),
            ]
            .after(seconds(0.3)),
        ],
    );
    fixed_chip.hide(&mut scene, rise);
    footer_features.hide(&mut scene, rise);

    let mut sc = stage_actor.score(&mut scene);
    sc.at(
        f("install").early(seconds(0.6)),
        all![
            stage::channel("title.scale", 0.86),
            stage::bounce("title.scale", 1.0, 0.8, 0.2),
            stage::to("title.opacity", 1.0, 0.4),
            stage::to("subtitle.opacity", 1.0, 0.5).after(seconds(0.3)),
            sfx("title", CONFIRM, -12.0),
        ],
    );
    sc.at(f("install"), stage::type_in("install", 40.0));
    sc.at(
        f("skill").early(seconds(0.2)),
        stage::type_in("skill", 48.0),
    );
    sc.at(
        f("fly"),
        all![
            stage::hit("post.bloom", 0.25, 0.18),
            sfx("fly", BLOOM, -10.0),
            stagger(
                0,
                ["ask", "batch", "inject", "paste", "token", "vault-link"],
                |beam| stage::to(format!("{beam}.flow"), 0.0, 1.2),
            ),
        ],
    );

    scene.finish().context("2password-stage")
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
