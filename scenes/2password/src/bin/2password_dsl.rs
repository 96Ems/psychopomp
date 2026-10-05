//! `scenes/2password` re-expressed with `psychopomp::score` and `psychopomp::layout`.
//!
//! `PlanBuilder` is the only mutable binding; `Stage`, `Caption` (headers,
//! chips, footers), and `sfx` are immutable values whose beats compose inside
//! `sc.at(...)` / `at!(sc, time => ...)` with zero escape hatches and emit
//! byte-identical `ScenePlan` JSON.
use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use psychopomp::{
    all, at,
    author::{PlanBuilder, millis, seconds},
    caption::CaptionAlign,
    layout::Placement,
    math::easing::Ease,
    narration::Narration,
    plan::{ReelPlan, ReelSegmentPlan, ReelTransitionStyle, ScenePlan},
    score::{Beat, Caption, CueTime, Stage, each, stagger, stagger_indexed},
    sfx,
    stage::{OrbEntrance, StageElement as El, StagePlan, StagePost},
    tone::Tone::{self, Accent, Error, Muted, Plain, Request, Success, Warning},
};

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
    let hdr = Caption::header(&mut sc, "2password", "1Password for coding agents")?;
    let raw_chip = Caption::chip(&mut sc, "chip-raw", Error, "raw op")?;
    let footer_problem = Caption::footer(
        &mut sc,
        "footer-problem",
        &[
            ("raw op: ", Plain),
            ("a prompt per call", Error),
            (", secrets in the chat", Plain),
        ],
    )?;
    let rewind_chip = Caption::chip(&mut sc, "chip-rewind", Accent, "◀◀ rewind")?;
    let fixed_chip = Caption::chip(&mut sc, "chip-fixed", Success, "with 2password")?;
    let footer_layer = Caption::footer(
        &mut sc,
        "footer-layer",
        &[
            ("2password: ", Plain),
            ("one approval", Success),
            (", references only", Plain),
        ],
    )?;
    let footer_features = Caption::footer(
        &mut sc,
        "footer-features",
        &[
            ("secrets go to processes, ", Plain),
            ("never to the agent", Accent),
        ],
    )?;

    // ── Establish: the vault, the command line, the agent, plugged together ──
    at!(sc, 0 =>
        s.channels([("camera.z", -160.0), ("camera.dof", 0.45)]),
        s.to("camera.z", 0.0, 2.2),
        s.orb_in("vault", OrbEntrance::HERO).after(seconds(0.15)),
        s.fade_in("vault-name", 1.0, 0.5).after(seconds(0.9)),
        s.settle_in("op")
            .after(seconds(0.45))
            .then_after(seconds(0.1), s.connect("vault-link", 0.5))
            .on_end(sfx::TICK.beat("connect-vault", -20.0)),
        s.settle_in("agent")
            .after(seconds(0.7))
            .then_after(seconds(0.15), s.connect("direct", 0.75))
            .on_end(sfx::TICK.beat("connect-direct", -20.0)),
        hdr.type_in(55.0, 0.6).after(seconds(0.4)),
        raw_chip.show().after(seconds(0.7)),
    );

    // ── The problem: prompt, prompt, PROMPT ──
    sc.at(p("needs"), s.to("agent.glow", 0.4, 0.5));
    at!(sc, p("asks") =>
        s.to("direct.emphasis", 1.0, 0.5),
        s.send("ask-1", 0.8).with(sfx::SEND.beat("ask-1", -10.0)).then(s.land("op")),
        s.to("camera.x", 70.0, 1.6),
        s.to("camera.focus", -60.0, 1.2),
    );

    let demands = [p("allow"), p("approve"), p("authorize"), p("sign in")];
    for (i, ((id, ..), at)) in PROMPTS.iter().zip(demands).enumerate() {
        at!(sc, at =>
            s.settle_in(*id),
            s.hit(format!("{id}.alarm"), 0.25 + 0.15 * i as f32, 0.0).after(seconds(0.05)),
            s.glitch(*id, [5.0 + i as f32, 8.0, 6.0]).after(seconds(0.06)),
            sfx::FAILURE.beat(format!("prompt-{i}"), -12.0 + 2.0 * i as f32),
        );
    }
    at!(sc, p("allow") =>
        s.to("agent.status", 1.0, 0.3),
        s.to("op.status", 1.0, 0.3),
        s.clock("agent.spinner"),
    );
    sc.at(
        p("approve").early(seconds(0.3)),
        s.send("ask-2", 0.55).then(s.land("op")),
    );
    at!(sc, p("authorize") =>
        s.send("ask-3", 0.45).early(seconds(0.25)).then(s.land("op")),
        s.jolt([1.0, 0.3], 0.6),
        s.hit("post.chroma", 0.1, 0.0),
        s.to("camera.z", 30.0, 1.2),
    );

    // The app locks: the vault cools to red, the command line locks up.
    at!(sc, p("unlock") =>
        s.to("op.status", 2.0, 0.25),
        s.to("vault.hurt", 0.55, 0.4),
        s.to("vault-link.break", 1.0, 0.9).after(seconds(0.1)),
        s.hit("op.alarm", 0.5, 0.15),
        sfx::GLITCH.beat("unlock", -14.0),
    );
    sc.at(p("sign in"), s.clock("op.spinner"));

    // STUCK: everything glitches at once and the frame takes the blow.
    let stuck_cards = [
        "op", "agent", "prompt-1", "prompt-2", "prompt-3", "prompt-4",
    ];
    at!(sc, p("stuck") =>
        s.to("agent.status", 2.0, 0.2),
        stagger(millis(35), stuck_cards, |c| {
            all![
                s.glitch(c, [7.0, 9.0, 8.0]),
                s.glitch(c, [9.0, 6.0, 7.0]).after(seconds(0.32)),
                s.set(format!("{c}.damage"), 1.0),
            ]
        }),
        s.jolt([-0.4, 1.0], 1.0),
        s.hit("post.chroma", 0.2, 0.0),
        s.hit("post.bloom", 0.35, 0.18),
        sfx::IMPACT.beat("stuck-impact", -7.0),
        sfx::GLITCH.beat("stuck-glitch", -10.0).after(seconds(0.05)),
    );

    // "When it finally works": the dialogs give way, one by one, into silence.
    at!(sc, p("finally") =>
        stagger_indexed(millis(90), PROMPTS, |_, (id, ..)| {
            s.fade_out(id, 0.35).also(s.set(format!("{id}.damage"), 0.0))
        }),
        s.set("op.damage", 0.0),
        s.set("agent.damage", 0.0),
        s.to("op.status", 3.0, 0.3),
        s.set("op.spinner", -1.0),
        s.to("vault.hurt", 0.0, 0.6),
        s.to("vault-link.break", 0.0, 0.9),
        s.to("camera.x", -60.0, 1.6),
        s.to("camera.z", 20.0, 1.6),
    );

    // ...and prints the secret RIGHT INTO THE CHAT.
    at!(sc, p("prints") =>
        s.to("camera.x", LEAK_CAMERA[0], 1.1),
        s.to("camera.z", LEAK_CAMERA[2], 1.1),
        s.send("leak", 0.6)
            .with(sfx::LAUNCH.beat("leak-send", -16.0).early(seconds(0.35)))
            .then(all![
                s.hit("agent.alarm", 0.6, 0.2),
                s.to("agent.status", 3.0, 0.2),
                s.set("agent.spinner", -1.0),
                s.type_in("leak-line", 80.0),
            ]),
    );
    at!(sc, p("chat") =>
        s.jolt([-1.0, 0.0], 1.0),
        s.hit("post.chroma", 0.28, 0.0),
        s.hit("post.bloom", 0.4, 0.18),
        s.hit("agent.alarm", 0.9, 0.3),
        sfx::IMPACT.beat("chat-impact", -5.0),
        sfx::DEATH.beat("chat-death", -9.0).after(seconds(0.05)),
        footer_problem.type_in(46.0, 0.8).after(seconds(0.3)),
    );

    // ── Rewind ──
    at!(sc, problem.end() + seconds(0.25) =>
        s.rewind(0.12),
        sfx::LAUNCH.beat("rewind", -14.0).early(seconds(0.1)),
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
    );

    // ── The layer: the direct wire unplugs, 2password settles in between ──
    let meet = l("meet");
    sc.at(meet.early(seconds(0.3)), s.disconnect("direct", 0.35));
    let layer_ready = sc.at(
        meet.after(seconds(0.1)),
        s.settle_in("layer").with(sfx::BLOOM.beat("meet", -12.0)),
    );
    let ask_contact = sc.at(layer_ready, s.connect("ask", 0.45));
    let batch_contact = sc.at(layer_ready.after(seconds(0.15)), s.connect("batch", 0.45));
    for (b, at) in [("ask", ask_contact), ("batch", batch_contact)] {
        at!(sc, at =>
            s.hit(format!("{b}.surge"), 0.45, 0.0),
            s.twang(b),
            sfx::TICK.beat(format!("connect-{b}"), -19.0),
        );
    }
    at!(sc, l("tiny layer") =>
        s.to("layer.glow", 0.55, 0.6),
        s.hit("layer.flash", 0.45, 0.0),
        s.to("camera.x", -60.0, 1.6),
        s.to("camera.z", 60.0, 1.8),
    );

    // The agent asks for everything at once; 2password asks op once.
    let find = at!(sc, l("everything") =>
        s.to("agent.status", 4.0, 0.3),
        s.send("find", 0.6)
            .with(sfx::SEND.beat("find", -11.0))
            .early(seconds(0.2))
            .then(s.land("layer").also(s.to("layer.status", 1.0, 0.3))),
    );
    let lookup = sc.at(
        l("just once").early(seconds(0.35)).not_before(find.reply()),
        s.send("lookup", 0.55)
            .with(sfx::SEND.beat("lookup", -11.0))
            .then(s.land("op")),
    );
    let fetch = sc.at(lookup.reply(), s.send("fetch", 0.45).then(s.land("vault")));

    // ONE approval.
    at!(sc, l("one approval") =>
        s.settle_in("prompt-ok"),
        s.hit("prompt-ok.flash", 0.6, 0.0).after(seconds(0.2)),
        s.to("op.status", 4.0, 0.3),
        sfx::SUCCESS.beat("approval", -8.0),
        s.to("camera.x", 60.0, 1.6),
    );

    // References come back; the secret itself never does.
    let refs_in = sc.at(
        l("references")
            .early(seconds(0.9))
            .not_before(fetch.reply()),
        s.send("refs-in", 0.5)
            .then(s.land("layer").also(s.to("prompt-ok.opacity", 0.0, 0.4))),
    );
    let refs = sc.at(
        refs_in.reply(),
        s.send("refs", 0.55).on_end(all![
            s.land("agent"),
            s.to("agent.status", 5.0, 0.3),
            s.to("layer.status", 2.0, 0.3),
            s.type_in("refs-line", 60.0),
            sfx::MARK.beat("refs", -14.0),
        ]),
    );
    sc.at(refs.early(seconds(0.4)), s.to("camera.x", -40.0, 1.6));
    sc.at(l("never"), footer_layer.type_in(46.0, 0.8));

    // ── Features: inject, save, and a service account ──
    let inject_contact = at!(sc, f("runs") =>
        footer_layer.hide(),
        s.to("camera.x", 0.0, 1.6),
        s.to("camera.z", -130.0, 1.8),
        s.settle_in("process")
            .then(s.connect("inject", 0.4))
            .on_end(sfx::TICK.beat("connect-inject", -19.0)),
    );
    sc.at(
        f("injected").not_before(inject_contact.after(seconds(0.34))),
        s.send("secrets", 0.5).then(all![
            s.land("process"),
            s.to("process.status", 1.0, 0.3),
            s.to("layer.status", 3.0, 0.3),
            sfx::CONFIRM.beat("secrets", -12.0),
        ]),
    );

    let paste_contact = sc.at(
        f("saves"),
        s.settle_in("clipboard")
            .then(s.connect("paste", 0.4))
            .on_end(sfx::TICK.beat("connect-paste", -19.0)),
    );
    let new_key = sc.at(
        f("clipboard").not_before(paste_contact.after(seconds(0.34))),
        s.send("new-key", 0.5).then(s.land("layer")),
    );
    let store = sc.at(
        f("checks").not_before(new_key.reply()),
        s.send("store", 0.5).then(s.land("op")),
    );
    sc.at(
        f("landed").not_before(store.reply()),
        s.send("verified", 0.5).then(all![
            s.land("layer"),
            s.to("layer.status", 4.0, 0.3),
            sfx::SUCCESS.beat("verified", -11.0),
        ]),
    );
    sc.at(f("checks"), footer_features.type_in(46.0, 0.8));

    sc.at(
        f("service account"),
        s.settle_in("keychain")
            .then(s.connect("token", 0.4))
            .on_end(sfx::TICK.beat("connect-token", -19.0))
            .then_after(seconds(0.34), s.send("unlock", 0.45))
            .then(s.land("layer")),
    );

    // No prompts. At all. Everything breathes.
    let active_beams = ["ask", "batch", "inject", "paste", "token", "vault-link"];
    let calm_cards = ["agent", "op", "process", "clipboard", "keychain"];
    at!(sc, f("no prompts") => s.to("layer.status", 5.0, 0.3), s.type_in("zero", 30.0));
    let all = f("at all");
    at!(sc, all =>
        s.hit("vault.pulse", 0.85, 0.0).after(seconds(0.15)),
        s.ease("vault.rotation", 1.6, 2.4, Ease::CubicOut),
        s.halo([("calm", 0.35), ("calm-outer", 0.5)], 0.22),
        s.hit("post.bloom", 0.3, 0.18),
        s.to("camera.z", -60.0, 2.0),
        each(active_beams, |b| s.to(format!("{b}.flow"), 0.5, 0.6)),
        each(calm_cards, |c| s.hit(format!("{c}.flash"), 0.35, 0.0)).after(seconds(0.1)),
        sfx::BLOOM.beat("all", -8.0).after(seconds(0.1)),
    );

    // ── Let your agent fly: the camera rises to the install lines ──
    at!(sc, all + seconds(1.6) =>
        each(["camera.x", "camera.z", "camera.focus"], |prop| s.to(prop, 0.0, 2.0)),
        s.to("camera.y", TITLE_CAMERA_Y, 2.4),
        s.to("ask.opacity", 0.0, 0.5),
        all![
            s.bounce("agent.x", AGENT_FLIGHT[0], 2.6, 0.12),
            s.bounce("agent.y", AGENT_FLIGHT[1], 2.6, 0.12),
        ]
        .after(seconds(0.3)),
        fixed_chip.hide(),
        footer_features.hide(),
    );
    at!(sc, f("install").early(seconds(0.6)) =>
        s.channel("title.scale", 0.86),
        s.bounce("title.scale", 1.0, 0.8, 0.2),
        s.fade_in("title", 1.0, 0.4),
        s.fade_in("subtitle", 1.0, 0.5).after(seconds(0.3)),
        sfx::CONFIRM.beat("title", -12.0),
    );
    sc.at(f("install"), s.type_in("install", 40.0));
    sc.at(f("skill").early(seconds(0.2)), s.type_in("skill", 48.0));
    at!(sc, f("fly") =>
        s.hit("post.bloom", 0.25, 0.18),
        sfx::BLOOM.beat("fly", -10.0),
        each(active_beams, |b| s.to(format!("{b}.flow"), 0.0, 1.2)),
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
