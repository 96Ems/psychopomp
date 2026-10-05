//! 2password as a Stage film. An agent talks to the 1Password command line
//! directly: every lookup pops another approval prompt, the app locks, sticks,
//! and finally prints the secret into the chat. Rewind. A small layer settles
//! between them: one batched approval, references instead of values, secrets
//! injected into processes, verified saves, and a service account with no
//! prompts at all. The camera then flies up to the install lines.
use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use psychopomp::{
    author::{PlanBuilder, PlanTime, millis, seconds, stagger},
    caption::CaptionAlign,
    math::easing::Ease,
    narration::Narration,
    plan::{ReelPlan, ReelSegmentPlan, ReelTransitionStyle, ScenePlan},
    sfx,
    stage::{OrbEntrance, StageActor, StageElement, StagePlan, StagePost, reply_after},
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
/// Left edge of the agent's chat line, above its card.
const CHAT_X: f32 = AGENT[0] - CARD[0] * 0.5 + 4.0;
/// Where the camera rests over the title, above the diagram.
const TITLE_CAMERA_Y: f32 = -900.0;
/// The agent card flies up to rest, centered, under the install lines.
const AGENT_FLIGHT: [f32; 2] = [960.0 - AGENT[0], -570.0];
/// The camera lunges at the agent as the secret lands in its chat.
const LEAK_CAMERA: [f32; 3] = [-330.0, 0.0, 140.0];

/// Approval prompts that pile up over the command line: id, position, text, tone.
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
    let output = root.join("2password.reel.json");
    fs::write(&output, serde_json::to_string_pretty(&reel)? + "\n")?;
    eprintln!(
        "wrote {} ({:.1}s)",
        output.display(),
        reel.duration_nanos() as f64 / 1e9
    );
    Ok(())
}

fn stage_plan() -> StagePlan {
    let mut elements = vec![
        StageElement::orb("vault", VAULT, 118.0)
            .points(900)
            .tone(Tone::Plain),
        StageElement::label(
            "vault-name",
            [VAULT[0], 704.0, 0.0],
            24.0,
            &[("1Password", Tone::Plain)],
        ),
        StageElement::card("agent", AGENT, CARD, "coding agent")
            .statuses(&[
                ("needs one API key", Tone::Plain),
                ("waiting for approval…", Tone::Warning),
                ("stuck", Tone::Error),
                ("secret in the chat!", Tone::Error),
                ("asking 2password", Tone::Plain),
                ("references only ✓", Tone::Success),
            ])
            .tone(Tone::Request),
        StageElement::card("op", OP, [330.0, 124.0], "op · 1Password CLI").statuses(&[
            ("ready", Tone::Muted),
            ("approval required", Tone::Warning),
            ("locked", Tone::Error),
            ("ready", Tone::Muted),
            ("approved once ✓", Tone::Success),
        ]),
        StageElement::card("layer", LAYER, CARD, "2password")
            .statuses(&[
                ("a layer on top of op", Tone::Muted),
                ("batching every lookup", Tone::Plain),
                ("references only", Tone::Success),
                ("injecting secrets", Tone::Plain),
                ("verified ✓", Tone::Success),
                ("no prompts", Tone::Success),
            ])
            .tone(Tone::Accent),
        StageElement::card("process", PROCESS, SMALL, "bun dev").statuses(&[
            ("starting", Tone::Muted),
            ("running with secrets ✓", Tone::Success),
        ]),
        StageElement::card("clipboard", CLIPBOARD, SMALL, "clipboard")
            .statuses(&[("a new API key", Tone::Muted)]),
        StageElement::card("keychain", KEYCHAIN, SMALL, "macOS Keychain")
            .statuses(&[("service-account token", Tone::Muted)]),
        StageElement::beam("direct", "agent", "op").tone(Tone::Request),
        StageElement::beam("vault-link", "op", "vault"),
        StageElement::beam("ask", "agent", "layer").tone(Tone::Request),
        StageElement::beam("batch", "layer", "op").tone(Tone::Accent),
        StageElement::beam("inject", "layer", "process").tone(Tone::Success),
        StageElement::beam("paste", "clipboard", "layer"),
        StageElement::beam("token", "keychain", "layer"),
    ];
    for (id, at, text, tone) in PROMPTS {
        elements.push(
            StageElement::card(id, at, PROMPT, "1Password")
                .statuses(&[(text, tone)])
                .tone(tone),
        );
    }
    elements.push(
        StageElement::card("prompt-ok", [1210.0, 320.0, -80.0], PROMPT, "1Password")
            .statuses(&[("approved once ✓", Tone::Success)])
            .tone(Tone::Success),
    );
    elements.extend([
        StageElement::packet("ask-1", "direct")
            .labeled("op item get \"OpenAI API Key\"")
            .tone(Tone::Request),
        StageElement::packet("ask-2", "direct")
            .labeled("op item get \"OpenAI API Key\"")
            .tone(Tone::Request),
        StageElement::packet("ask-3", "direct")
            .labeled("op item get --reveal")
            .tone(Tone::Request),
        StageElement::packet("leak", "direct")
            .reversed()
            .labeled("sk-proj-7Hq2Zx9mK4pL")
            .tone(Tone::Error),
        StageElement::packet("find", "ask")
            .labeled("find openai stripe github")
            .tone(Tone::Request),
        StageElement::packet("lookup", "batch")
            .labeled("one batched lookup")
            .tone(Tone::Accent),
        StageElement::packet("fetch", "vault-link"),
        StageElement::packet("refs-in", "batch")
            .reversed()
            .labeled("op:// references")
            .tone(Tone::Success),
        StageElement::packet("refs", "ask")
            .reversed()
            .labeled("op:// references")
            .tone(Tone::Success),
        StageElement::packet("secrets", "inject")
            .labeled("OPENAI_API_KEY")
            .tone(Tone::Success),
        StageElement::packet("new-key", "paste").labeled("new key"),
        StageElement::packet("store", "batch")
            .labeled("create, then read back")
            .tone(Tone::Accent),
        StageElement::packet("verified", "batch")
            .reversed()
            .labeled("verified ✓")
            .tone(Tone::Success),
        StageElement::packet("unlock", "token").labeled("token"),
        StageElement::label(
            "leak-line",
            [CHAT_X, 440.0, 0.0],
            22.0,
            &[
                ("chat ▸ ", Tone::Muted),
                ("sk-proj-7Hq2Zx9mK4pLw3eR8vN…", Tone::Error),
            ],
        )
        .align(CaptionAlign::Left),
        StageElement::label(
            "refs-line",
            [CHAT_X, 440.0, 0.0],
            22.0,
            &[
                ("chat ▸ ", Tone::Muted),
                ("op://Personal/OpenAI API Key/credential", Tone::Success),
            ],
        )
        .align(CaptionAlign::Left),
        StageElement::label(
            "zero",
            [OP[0], 330.0, -80.0],
            44.0,
            &[("prompts: ", Tone::Muted), ("0", Tone::Success)],
        ),
        StageElement::label(
            "title",
            [960.0, -470.0, 0.0],
            120.0,
            &[("2password", Tone::Accent)],
        ),
        StageElement::label(
            "subtitle",
            [960.0, -360.0, 0.0],
            34.0,
            &[("1Password for coding agents", Tone::Plain)],
        ),
        StageElement::label(
            "install",
            [960.0, -262.0, 0.0],
            30.0,
            &[("$ ", Tone::Muted), ("bun add -g 2password", Tone::Plain)],
        ),
        StageElement::label(
            "skill",
            [960.0, -212.0, 0.0],
            30.0,
            &[
                ("$ ", Tone::Muted),
                ("bunx skills add kitlangton/2password", Tone::Plain),
            ],
        ),
        StageElement::ring("calm", VAULT, 132.0)
            .thickness(1.3)
            .tone(Tone::Success),
        StageElement::ring("calm-outer", VAULT, 138.0)
            .thickness(1.3)
            .tone(Tone::Success),
    ]);
    StagePlan {
        post: StagePost::RESTRAINED,
        elements,
    }
}

fn film(narration: &Narration) -> Result<ScenePlan> {
    // The problem, a rewind, the layer, a breath, the features, and a tail.
    let reading = narration.reading(
        seconds(2.0),
        [
            ("problem", seconds(1.6)),
            ("layer", seconds(0.5)),
            ("features", seconds(3.2)),
        ],
    )?;
    let mut scene = PlanBuilder::new("2password-stage", reading.duration());
    let [problem, layer, features] = reading.place(&mut scene);
    let p = |phrase: &str| problem.at(phrase);
    let l = |phrase: &str| layer.at(phrase);
    let f = |phrase: &str| features.at(phrase);
    let mut stage = StageActor::declare(&mut scene, "stage", &stage_plan())?;
    let s = &mut stage;
    let sc = &mut scene;

    // ── Establish: the vault, the command line, the agent, plugged together ──
    s.channel(sc, "camera.z", -160.0);
    s.channel(sc, "camera.dof", 0.45);
    s.to(sc, "camera.z", 0, 0.0, 2.2);
    s.orb_in(sc, "vault", seconds(0.15), OrbEntrance::HERO);
    s.fade_in(sc, "vault-name", seconds(0.9), 1.0, 0.5);
    let op_ready = s.settle_in(sc, "op", seconds(0.45));
    let vault_contact = s.connect(sc, "vault-link", op_ready + seconds(0.1), 0.5);
    sfx::TICK.play(sc, "connect-vault", vault_contact, -20.0);
    let agent_ready = s.settle_in(sc, "agent", seconds(0.7));
    let direct_contact = s.connect(sc, "direct", agent_ready + seconds(0.15), 0.75);
    sfx::TICK.play(sc, "connect-direct", direct_contact, -20.0);
    header(sc, &PR, Some(seconds(0.4)))?;
    let mut raw_chip = chip(sc, "chip-raw", Tone::Error, "raw op")?;
    raw_chip.show(sc, seconds(0.7));

    // ── The problem: prompt, prompt, PROMPT ──
    s.to(sc, "agent.glow", p("needs"), 0.4, 0.5);
    s.to(sc, "direct.emphasis", p("asks"), 1.0, 0.5);
    let ask_1 = s.send(sc, "ask-1", p("asks"), 0.8);
    sfx::SEND.play(sc, "ask-1", p("asks"), -10.0);
    s.land(sc, "op", ask_1);
    s.to(sc, "camera.x", p("asks"), 70.0, 1.6);
    s.to(sc, "camera.focus", p("asks"), -60.0, 1.2);

    // Each demand pops another dialog nearer the lens, louder than the last.
    let demands = [p("allow"), p("approve"), p("authorize"), p("sign in")];
    for (index, ((id, ..), at)) in PROMPTS.iter().zip(demands).enumerate() {
        s.settle_in(sc, id, at);
        s.hit(
            sc,
            &format!("{id}.alarm"),
            at + seconds(0.05),
            0.25 + 0.15 * index as f32,
            0.0,
        );
        s.glitch(sc, id, at + seconds(0.06), [5.0 + index as f32, 8.0, 6.0]);
        sfx::FAILURE.play(
            sc,
            format!("prompt-{index}"),
            at,
            -12.0 + 2.0 * index as f32,
        );
    }
    s.to(sc, "agent.status", p("allow"), 1.0, 0.3);
    s.to(sc, "op.status", p("allow"), 1.0, 0.3);
    s.clock(sc, "agent.spinner", p("allow"));
    // More asks keep arriving underneath the dialogs.
    let ask_2 = s.send(sc, "ask-2", p("approve") - seconds(0.3), 0.55);
    s.land(sc, "op", ask_2);
    let ask_3 = s.send(sc, "ask-3", p("authorize") - seconds(0.25), 0.45);
    s.land(sc, "op", ask_3);
    s.jolt(sc, p("authorize"), [1.0, 0.3], 0.6);
    s.hit(sc, "post.chroma", p("authorize"), 0.1, 0.0);
    s.to(sc, "camera.z", p("authorize"), 30.0, 1.2);

    // The app locks: the vault cools to red, the command line locks up.
    let unlock = p("unlock");
    s.to(sc, "op.status", unlock, 2.0, 0.25);
    s.to(sc, "vault.hurt", unlock, 0.55, 0.4);
    s.to(sc, "vault-link.break", unlock + seconds(0.1), 1.0, 0.9);
    s.hit(sc, "op.alarm", unlock, 0.5, 0.15);
    sfx::GLITCH.play(sc, "unlock", unlock, -14.0);
    s.clock(sc, "op.spinner", p("sign in"));

    // STUCK: everything glitches at once and the frame takes the blow.
    let stuck = p("stuck");
    s.to(sc, "agent.status", stuck, 2.0, 0.2);
    let cards = [
        "op", "agent", "prompt-1", "prompt-2", "prompt-3", "prompt-4",
    ];
    stagger(cards, stuck, millis(35), |card, at| {
        s.glitch(sc, card, at, [7.0, 9.0, 8.0]);
        s.glitch(sc, card, at + seconds(0.32), [9.0, 6.0, 7.0]);
        s.set(sc, &format!("{card}.damage"), at, 1.0);
        at
    });
    s.jolt(sc, stuck, [-0.4, 1.0], 1.0);
    s.hit(sc, "post.chroma", stuck, 0.2, 0.0);
    s.hit(sc, "post.bloom", stuck, 0.35, 0.18);
    sfx::IMPACT.play(sc, "stuck-impact", stuck, -7.0);
    sfx::GLITCH.play(sc, "stuck-glitch", stuck + seconds(0.05), -10.0);

    // "When it finally works": the dialogs give way, one by one, into silence.
    let finally = p("finally");
    stagger(PROMPTS, finally, millis(90), |(id, ..), at| {
        s.fade_out(sc, id, at, 0.35);
        s.set(sc, &format!("{id}.damage"), at, 0.0);
        at
    });
    for card in ["op", "agent"] {
        s.set(sc, &format!("{card}.damage"), finally, 0.0);
    }
    s.to(sc, "op.status", finally, 3.0, 0.3);
    s.set(sc, "op.spinner", finally, -1.0);
    s.to(sc, "vault.hurt", finally, 0.0, 0.6);
    s.to(sc, "vault-link.break", finally, 0.0, 0.9);
    s.to(sc, "camera.x", finally, -60.0, 1.6);
    s.to(sc, "camera.z", finally, 20.0, 1.6);

    // ...and prints the secret RIGHT INTO THE CHAT.
    s.to(sc, "camera.x", p("prints"), LEAK_CAMERA[0], 1.1);
    s.to(sc, "camera.z", p("prints"), LEAK_CAMERA[2], 1.1);
    let leak = s.send(sc, "leak", p("prints"), 0.6);
    sfx::LAUNCH.play(sc, "leak-send", p("prints") - seconds(0.35), -16.0);
    s.hit(sc, "agent.alarm", leak, 0.6, 0.2);
    s.to(sc, "agent.status", leak, 3.0, 0.2);
    s.set(sc, "agent.spinner", leak, -1.0);
    s.type_in(sc, "leak-line", leak, 80.0);
    let chat = p("chat");
    s.jolt(sc, chat, [-1.0, 0.0], 1.0);
    s.hit(sc, "post.chroma", chat, 0.28, 0.0);
    s.hit(sc, "post.bloom", chat, 0.4, 0.18);
    s.hit(sc, "agent.alarm", chat, 0.9, 0.3);
    sfx::IMPACT.play(sc, "chat-impact", chat, -5.0);
    sfx::DEATH.play(sc, "chat-death", chat + seconds(0.05), -9.0);
    let mut footer_problem = footer(
        sc,
        "footer-problem",
        vec![
            span("raw op: ", Tone::Plain),
            span("a prompt per call", Tone::Error),
            span(", secrets in the chat", Tone::Plain),
        ],
    )?;
    footer_problem.type_in(sc, chat + seconds(0.3), 46.0, 0.8);

    // ── Rewind ──
    let switch = problem.end() + seconds(0.25);
    s.rewind(sc, switch, 0.12);
    sfx::LAUNCH.play(sc, "rewind", switch - seconds(0.1), -14.0);
    raw_chip.hide(sc, switch);
    footer_problem.hide(sc, switch);
    let mut rewind_chip = chip(sc, "chip-rewind", Tone::Accent, "◀◀ rewind")?;
    rewind_chip.show(sc, switch + seconds(0.25));
    rewind_chip.hide(sc, switch + seconds(1.5));
    let mut fixed_chip = chip(sc, "chip-fixed", Tone::Success, "with 2password")?;
    fixed_chip.show(sc, switch + seconds(1.65));
    s.to(sc, "leak-line.opacity", switch, 0.0, 0.4);
    s.to(sc, "agent.alarm", switch, 0.0, 0.4);
    s.to(sc, "agent.status", switch + seconds(0.2), 0.0, 0.9);
    s.to(sc, "agent.glow", switch, 0.0, 0.6);
    s.to(sc, "op.status", switch + seconds(0.2), 0.0, 0.6);
    s.to(sc, "direct.emphasis", switch, 0.0, 0.6);
    s.to(sc, "camera.x", switch, 0.0, 1.6);
    s.to(sc, "camera.z", switch, 0.0, 1.8);
    s.to(sc, "camera.focus", switch, 0.0, 1.0);

    // ── The layer: the direct wire unplugs, 2password settles in between ──
    let meet = l("meet");
    let unplug = meet.saturating_sub(seconds(0.3));
    s.disconnect(sc, "direct", unplug, 0.35);
    let layer_ready = s.settle_in(sc, "layer", meet + seconds(0.1));
    sfx::BLOOM.play(sc, "meet", meet + seconds(0.1), -12.0);
    let ask_contact = s.connect(sc, "ask", layer_ready, 0.45);
    let batch_contact = s.connect(sc, "batch", layer_ready + seconds(0.15), 0.45);
    for (beam, at) in [("ask", ask_contact), ("batch", batch_contact)] {
        s.hit(sc, &format!("{beam}.surge"), at, 0.45, 0.0);
        s.twang(sc, beam, at);
        sfx::TICK.play(sc, format!("connect-{beam}"), at, -19.0);
    }
    s.to(sc, "layer.glow", l("tiny layer"), 0.55, 0.6);
    s.hit(sc, "layer.flash", l("tiny layer"), 0.45, 0.0);
    s.to(sc, "camera.x", l("tiny layer"), -60.0, 1.6);
    s.to(sc, "camera.z", l("tiny layer"), 60.0, 1.8);

    // The agent asks for everything at once; 2password asks op once.
    let everything = l("everything");
    s.to(sc, "agent.status", everything, 4.0, 0.3);
    let find = s.send(sc, "find", everything - seconds(0.2), 0.6);
    sfx::SEND.play(sc, "find", everything - seconds(0.2), -11.0);
    s.land(sc, "layer", find);
    s.to(sc, "layer.status", find, 1.0, 0.3);
    let lookup = s.send(
        sc,
        "lookup",
        l("just once")
            .saturating_sub(seconds(0.35))
            .not_before(reply_after(find)),
        0.55,
    );
    sfx::SEND.play(sc, "lookup", lookup - seconds(0.55), -11.0);
    s.land(sc, "op", lookup);
    let fetch = s.send(sc, "fetch", reply_after(lookup), 0.45);
    s.land(sc, "vault", fetch);

    // ONE approval.
    let approval = l("one approval");
    s.settle_in(sc, "prompt-ok", approval);
    s.hit(sc, "prompt-ok.flash", approval + seconds(0.2), 0.6, 0.0);
    s.to(sc, "op.status", approval, 4.0, 0.3);
    sfx::SUCCESS.play(sc, "approval", approval, -8.0);
    s.to(sc, "camera.x", approval, 60.0, 1.6);

    // References come back; the secret itself never does.
    let refs_in = s.send(
        sc,
        "refs-in",
        l("references")
            .saturating_sub(seconds(0.9))
            .not_before(reply_after(fetch)),
        0.5,
    );
    s.land(sc, "layer", refs_in);
    s.to(sc, "prompt-ok.opacity", refs_in, 0.0, 0.4);
    let refs = s.send(sc, "refs", reply_after(refs_in), 0.55);
    s.land(sc, "agent", refs);
    s.to(sc, "agent.status", refs, 5.0, 0.3);
    s.to(sc, "layer.status", refs, 2.0, 0.3);
    s.type_in(sc, "refs-line", refs, 60.0);
    sfx::MARK.play(sc, "refs", refs, -14.0);
    s.to(sc, "camera.x", refs - seconds(0.4), -40.0, 1.6);
    let mut footer_layer = footer(
        sc,
        "footer-layer",
        vec![
            span("2password: ", Tone::Plain),
            span("one approval", Tone::Success),
            span(", references only", Tone::Plain),
        ],
    )?;
    footer_layer.type_in(sc, l("never"), 46.0, 0.8);

    // ── Features: inject, save, and a service account ──
    footer_layer.hide(sc, f("runs"));
    s.to(sc, "camera.x", f("runs"), 0.0, 1.6);
    s.to(sc, "camera.z", f("runs"), -130.0, 1.8);
    let process_ready = s.settle_in(sc, "process", f("runs"));
    let inject_contact = s.connect(sc, "inject", process_ready, 0.4);
    sfx::TICK.play(sc, "connect-inject", inject_contact, -19.0);
    let secrets = s.send(
        sc,
        "secrets",
        f("injected").not_before(inject_contact + seconds(0.34)),
        0.5,
    );
    s.land(sc, "process", secrets);
    s.to(sc, "process.status", secrets, 1.0, 0.3);
    s.to(sc, "layer.status", secrets, 3.0, 0.3);
    sfx::CONFIRM.play(sc, "secrets", secrets, -12.0);

    let clipboard_ready = s.settle_in(sc, "clipboard", f("saves"));
    let paste_contact = s.connect(sc, "paste", clipboard_ready, 0.4);
    sfx::TICK.play(sc, "connect-paste", paste_contact, -19.0);
    let new_key = s.send(
        sc,
        "new-key",
        f("clipboard").not_before(paste_contact + seconds(0.34)),
        0.5,
    );
    s.land(sc, "layer", new_key);
    let store = s.send(
        sc,
        "store",
        f("checks").not_before(reply_after(new_key)),
        0.5,
    );
    s.land(sc, "op", store);
    let verified = s.send(
        sc,
        "verified",
        f("landed").not_before(reply_after(store)),
        0.5,
    );
    s.land(sc, "layer", verified);
    s.to(sc, "layer.status", verified, 4.0, 0.3);
    sfx::SUCCESS.play(sc, "verified", verified, -11.0);
    let mut footer_features = footer(
        sc,
        "footer-features",
        vec![
            span("secrets go to processes, ", Tone::Plain),
            span("never to the agent", Tone::Accent),
        ],
    )?;
    footer_features.type_in(sc, f("checks"), 46.0, 0.8);

    let keychain_ready = s.settle_in(sc, "keychain", f("service account"));
    let token_contact = s.connect(sc, "token", keychain_ready, 0.4);
    sfx::TICK.play(sc, "connect-token", token_contact, -19.0);
    let unlock = s.send(sc, "unlock", token_contact + seconds(0.34), 0.45);
    s.land(sc, "layer", unlock);

    // No prompts. At all. Everything breathes.
    let none = f("no prompts");
    s.to(sc, "layer.status", none, 5.0, 0.3);
    s.type_in(sc, "zero", none, 30.0);
    let all = f("at all");
    s.hit(sc, "vault.pulse", all + seconds(0.15), 0.85, 0.0);
    s.ease(sc, "vault.rotation", all, 1.6, 2.4, Ease::CubicOut);
    s.halo(sc, [("calm", 0.35), ("calm-outer", 0.5)], all, 0.22);
    s.hit(sc, "post.bloom", all, 0.3, 0.18);
    s.to(sc, "camera.z", all, -60.0, 2.0);
    for beam in ["ask", "batch", "inject", "paste", "token", "vault-link"] {
        s.to(sc, &format!("{beam}.flow"), all, 0.5, 0.6);
    }
    // The token's arrival lights the layer itself.
    for card in ["agent", "op", "process", "clipboard", "keychain"] {
        s.hit(sc, &format!("{card}.flash"), all + seconds(0.1), 0.35, 0.0);
    }
    sfx::BLOOM.play(sc, "all", all + seconds(0.1), -8.0);

    // ── Let your agent fly: the camera rises to the install lines ──
    let rise = all + seconds(1.6);
    for property in ["camera.x", "camera.z", "camera.focus"] {
        s.to(sc, property, rise, 0.0, 2.0);
    }
    s.to(sc, "camera.y", rise, TITLE_CAMERA_Y, 2.4);
    // Let your agent fly: it rises with the camera, untethered.
    s.to(sc, "ask.opacity", rise, 0.0, 0.5);
    s.bounce(
        sc,
        "agent.x",
        rise + seconds(0.3),
        AGENT_FLIGHT[0],
        2.6,
        0.12,
    );
    s.bounce(
        sc,
        "agent.y",
        rise + seconds(0.3),
        AGENT_FLIGHT[1],
        2.6,
        0.12,
    );
    fixed_chip.hide(sc, rise);
    footer_features.hide(sc, rise);
    let title = f("install").saturating_sub(seconds(0.6));
    s.channel(sc, "title.scale", 0.86);
    s.bounce(sc, "title.scale", title, 1.0, 0.8, 0.2);
    s.fade_in(sc, "title", title, 1.0, 0.4);
    s.fade_in(sc, "subtitle", title + seconds(0.3), 1.0, 0.5);
    sfx::CONFIRM.play(sc, "title", title, -12.0);
    s.type_in(sc, "install", f("install"), 40.0);
    s.type_in(sc, "skill", f("skill").saturating_sub(seconds(0.2)), 48.0);
    s.hit(sc, "post.bloom", f("fly"), 0.25, 0.18);
    sfx::BLOOM.play(sc, "fly", f("fly"), -10.0);
    for beam in ["ask", "batch", "inject", "paste", "token", "vault-link"] {
        s.to(sc, &format!("{beam}.flow"), f("fly"), 0.0, 1.2);
    }

    scene.finish().context("2password-stage")
}

#[cfg(test)]
mod tests {
    use psychopomp::{
        math::{Vec3, vec2},
        stage::{Camera, StageElement},
    };

    #[test]
    fn diagram_cards_fit_every_working_camera() {
        let all = |_: &str| true;
        let leak = |id: &str| id == "agent" || id == "op";
        // The prompts are dismissed as this camera move begins.
        let dismissed = |id: &str| !id.starts_with("prompt");
        type Framed = fn(&str) -> bool;
        let cameras: [([f32; 3], Framed); 8] = [
            ([0.0, 0.0, -160.0], all),
            ([0.0, 0.0, 0.0], all),
            ([70.0, 0.0, 30.0], all),
            ([-60.0, 0.0, 20.0], dismissed),
            (super::LEAK_CAMERA, leak),
            ([60.0, 0.0, 60.0], all),
            ([0.0, 0.0, -130.0], all),
            ([0.0, 0.0, -60.0], all),
        ];
        for (position, framed) in cameras {
            let camera = Camera::at(Vec3::from(position), vec2(1920.0, 1080.0));
            for element in super::stage_plan().elements {
                if let StageElement::Card { id, at, size, .. } = element
                    && framed(&id)
                {
                    let (center, scale) = camera.project(Vec3::from(at)).unwrap();
                    let half = vec2(size[0], size[1]) * (0.5 * scale);
                    let (low, high) = (center - half, center + half);
                    assert!(
                        low.x > 40.0 && high.x < 1880.0 && low.y > 130.0 && high.y < 975.0,
                        "{id} clips at camera {position:?}: {low:?}..{high:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_title_frame_shows_only_the_title() {
        let camera = Camera::at(
            Vec3::new(0.0, super::TITLE_CAMERA_Y, 0.0),
            vec2(1920.0, 1080.0),
        );
        let (title, _) = camera.project(Vec3::new(960.0, -470.0, 0.0)).unwrap();
        assert!((200.0..700.0).contains(&title.y), "title at {title:?}");
        let (keychain, scale) = camera.project(Vec3::from(super::KEYCHAIN)).unwrap();
        assert!(
            keychain.y - 55.0 * scale > 1080.0,
            "the diagram leaves the frame"
        );
        let (agent, scale) = camera
            .project(Vec3::new(
                super::AGENT[0] + super::AGENT_FLIGHT[0],
                super::AGENT[1] + super::AGENT_FLIGHT[1],
                0.0,
            ))
            .unwrap();
        assert!(
            agent.y - 62.0 * scale > 720.0 && agent.y + 62.0 * scale < 1040.0,
            "the agent lands under the install lines at {agent:?}"
        );
        assert!(
            (agent.x - 960.0).abs() < 1.0,
            "the agent lands centered at {agent:?}"
        );
    }
}
