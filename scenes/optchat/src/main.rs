//! optchat explainer reel: a narrated, seven-segment film about the optchat
//! plugin, oriented on **orchestrator mode** — one conversation holds a mission
//! and a ledger of finished runs while sub-agents fan out, and that chat is
//! append-only and never rewritten, so the prompt cache keeps hitting. Memory
//! mode is the single segment that shows the log kept word for word, the tree
//! of one-line summaries, and the fixed-size 95k view.
//!
//! Every visual beat is keyed to a phrase the recorded narration still says
//! (`scenes/optchat/narration`), so re-voicing the script re-times the film and
//! a stale anchor panics with its clip. Big numbers only ever describe the
//! session's accumulated log; the one number in a "context sent to the model"
//! position is the fixed 95k view. Run:
//!
//! ```sh
//! cargo run -p psychopomp-optchat
//! cargo run --release -- plan validate scenes/optchat/optchat.reel.json
//! ```
use std::{env, fs, path::PathBuf};

use anyhow::{Context, Result};
use psychopomp::{
    author::{PlanBuilder, seconds},
    axis::AxisPlan,
    bars::{BarDeltaPlan, BarSeriesPlan, BarsActor, BarsPlan},
    caption::{CaptionActor, CaptionAlign, CaptionPlan, CaptionSpanPlan},
    chrome::{chip, footer, header},
    editor::diff::{Diff, add, keep},
    meter::{MeterActor, MeterPlan},
    narration::Narration,
    plan::{ReelPlan, ScenePlan},
    readout::ReadoutFormat,
    rolling::{RollingNumberActor, RollingNumberPlan},
    sfx,
    stage::{Arrow, Curve, StageActor, StageElement, StagePlan, StagePost, Waypoint},
    terminal::{TerminalActor, TerminalPlan},
    tone::Tone,
};

/// Silence before each clip, and the tail after it.
const LEAD: u64 = 400_000_000;
const GAP: u64 = 300_000_000;
/// The dip between two reel segments.
const TRANSITION: u64 = 700_000_000;

/// Type-in speed for the one caption that sums up a beat.
const TYPE: f32 = 42.0;
/// A faster type-in for the longest captions, so they finish inside their beat.
const TYPE_LONG: f32 = 96.0;

/// The plugin's context budget: 128 kB at ~1.35 bytes per token is the 95k the
/// view is fixed at, every turn. It is the only figure a "sent to the model"
/// position is allowed to carry.
const VIEW_TOKENS: &str = "95";

fn span(text: &str, tone: Tone) -> CaptionSpanPlan {
    CaptionSpanPlan::new(text, tone)
}

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let narration = Narration::load(&root.join("narration"))?;
    let reel = ReelPlan::dipped(
        "optchat",
        vec![
            intro(&narration)?,
            memory(&narration)?,
            zoom(&narration)?,
            orchestrator(&narration)?,
            fanout(&narration)?,
            tui(&narration)?,
            outro(&narration)?,
        ],
        TRANSITION,
    )?;
    let output = root.join("optchat.reel.json");
    fs::write(&output, serde_json::to_string_pretty(&reel)? + "\n")?;
    eprintln!(
        "wrote {} ({:.2}s, {} segments)",
        output.display(),
        reel.duration_nanos() as f64 / 1e9,
        reel.segments.len()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 1. intro — two walls (what it remembers, and what it costs), then the answer
// ---------------------------------------------------------------------------

fn intro(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(LEAD, [("intro", GAP)])?;
    let mut scene = PlanBuilder::new("optchat-intro", reading.duration());
    let [said] = reading.place(&mut scene);

    let mut head = header(&mut scene, "optchat", "two walls · one answer")?;
    head.show(&mut scene, seconds(0.2));
    let mut status = chip(&mut scene, "chip", Tone::Muted, "two walls")?;
    status.show(&mut scene, seconds(0.4));

    // ---- wall one: what a session can remember --------------------------
    let mut term = TerminalActor::declare(
        &mut scene,
        "term",
        TerminalPlan::new([120.0, 240.0], 820.0, 9)
            .size(20.0)
            .titled("opencode — session")
            .prompt(vec![span("~/dev ❯ ", Tone::Accent)]),
    )?;
    let shown = term.show(&mut scene, seconds(0.5));
    let entered = term.type_command(&mut scene, shown + seconds(0.3), "opencode")?;

    let walls = said.at("two walls").max(entered + seconds(0.15));
    term.print(
        &mut scene,
        walls,
        [
            vec![span("● session 1 started", Tone::Plain)],
            vec![span("user  explain how the engine loop works", Tone::Plain)],
            vec![span(
                "tool  reading packages/opencode/src/session.ts",
                Tone::Muted,
            )],
        ],
    )?;

    // The log climbs past the window: the first wall is the hard limit.
    let mut meter = MeterActor::declare(
        &mut scene,
        "window",
        &MeterPlan::ring(
            [1520.0, 470.0],
            150.0,
            AxisPlan::new([0.0, 1_200_000.0]).nice(4),
        )
        .label("the window")
        .readout(ReadoutFormat::new(0))
        .threshold(128_000.0, Tone::Warning)
        .threshold(1_100_000.0, Tone::Error),
        0.0,
    )?;
    let window_at = said.at("a window");
    meter.show(&mut scene, window_at);
    meter.sweep(&mut scene, window_at, 1_180_000.0, 1.9);

    let mut f_limit = footer(
        &mut scene,
        "footer-limit",
        vec![
            span("one wall: ", Tone::Muted),
            span("what the session can remember — a window", Tone::Error),
        ],
    )?;
    f_limit.type_in(&mut scene, walls, TYPE, 0.7);

    // ...and a compaction that throws the detail away.
    let detail = said.at("the detail away");
    f_limit.hide(&mut scene, detail.saturating_sub(seconds(0.3)));
    meter.flash(&mut scene, detail, 0.7);
    term.print(
        &mut scene,
        detail,
        [
            vec![span(
                "✻ context full — compaction rewrites the past",
                Tone::Error,
            )],
            vec![span("  the detail is gone", Tone::Muted)],
        ],
    )?;
    let mut f_zero = footer(
        &mut scene,
        "footer-zero",
        vec![
            span("and a compaction ", Tone::Plain),
            span("that throws the detail away", Tone::Error),
        ],
    )?;
    f_zero.type_in(&mut scene, detail, TYPE, 0.7);

    let restarted = term.type_command(&mut scene, detail + seconds(0.9), "opencode")?;
    term.print(
        &mut scene,
        restarted + seconds(0.2),
        [vec![span("● session 2 started — from zero", Tone::Warning)]],
    )?;

    // ---- wall two: what it costs — every turn pays again ----------------
    let cost = said.at("what it costs");
    let clear_wall = cost.saturating_sub(seconds(0.4));
    f_zero.hide(&mut scene, clear_wall);
    term.hide(&mut scene, clear_wall);
    meter.hide(&mut scene, clear_wall);

    let plan = StagePlan {
        post: StagePost::RESTRAINED,
        elements: vec![
            StageElement::card(
                "session",
                [330.0, 560.0, 0.0],
                [330.0, 120.0],
                "the session",
            )
            .statuses(&[("1,000 messages · 1.5 MB log", Tone::Plain)])
            .tone(Tone::Request),
            StageElement::card("turn", [960.0, 560.0, 0.0], [430.0, 120.0], "every turn")
                .statuses(&[("the whole history attached", Tone::Warning)])
                .tone(Tone::Warning),
            StageElement::card("model", [1590.0, 560.0, 0.0], [220.0, 110.0], "the model")
                .tone(Tone::Plain),
            StageElement::beam("send", "session", "turn").tone(Tone::Request),
            StageElement::beam("ask", "turn", "model").tone(Tone::Warning),
            StageElement::packet("cost-1", "send")
                .labeled("the history")
                .tone(Tone::Warning),
            StageElement::packet("cost-2", "send")
                .labeled("the history")
                .tone(Tone::Warning),
            StageElement::packet("cost-3", "send")
                .labeled("the history")
                .tone(Tone::Warning),
        ],
    };
    let mut stage = StageActor::declare(&mut scene, "cost-stage", &plan)?;
    stage.settle_in(&mut scene, "session", cost);
    stage.settle_in(&mut scene, "turn", cost + seconds(0.2));
    stage.settle_in(&mut scene, "model", cost + seconds(0.4));
    stage.connect(&mut scene, "send", cost + seconds(0.5), 0.5);
    stage.connect(&mut scene, "ask", cost + seconds(0.7), 0.5);

    // Every turn re-sends the whole history.
    let send_at = cost + seconds(0.6);
    for (index, packet) in ["cost-1", "cost-2", "cost-3"].iter().enumerate() {
        let at = send_at + seconds(index as f64 * 0.75);
        let arrival = stage.send(&mut scene, packet, at, 0.6);
        stage.land(&mut scene, "turn", arrival);
        sfx::TICK.play(&mut scene, *packet, at, -19.0);
    }
    let again = said.at("the history again");
    stage.hit(&mut scene, "turn.flash", again, 0.5, 0.0);
    stage.jolt(&mut scene, again, [1.0, 0.0], 0.35);

    let mut f_cost = footer(
        &mut scene,
        "footer-cost",
        vec![
            span("and what it costs: ", Tone::Plain),
            span("every turn pays for the history again", Tone::Error),
        ],
    )?;
    f_cost.type_in(&mut scene, cost, TYPE, 0.7);

    // ---- the answer ------------------------------------------------------
    let both = said.at("answers both");
    f_cost.hide(&mut scene, both.saturating_sub(seconds(0.3)));
    status.hide(&mut scene, both);
    let mut promise = chip(&mut scene, "chip-promise", Tone::Success, "the answer")?;
    promise.show(&mut scene, both);

    let never = said.at("never compacts");
    let mut line_a = CaptionActor::declare(
        &mut scene,
        "answer-a",
        &CaptionPlan::line(
            [960.0, 300.0],
            30.0,
            vec![
                span("a chat ", Tone::Plain),
                span("that never compacts", Tone::Success),
            ],
        )
        .aligned(CaptionAlign::Center),
    )?;
    line_a.type_in(&mut scene, never.saturating_sub(seconds(0.7)), TYPE, 0.6);

    let team = said.at("one conversation into a team");
    let mut line_b = CaptionActor::declare(
        &mut scene,
        "answer-b",
        &CaptionPlan::line(
            [960.0, 385.0],
            30.0,
            vec![
                span("and a second mode: ", Tone::Plain),
                span("one conversation into a team", Tone::Accent),
            ],
        )
        .aligned(CaptionAlign::Center),
    )?;
    line_b.type_in(&mut scene, team.saturating_sub(seconds(0.9)), TYPE, 0.8);
    sfx::BLOOM.play(&mut scene, "team", team, -13.0);

    scene.finish().context("optchat-intro")
}

// ---------------------------------------------------------------------------
// 2. memory — the log kept word for word, the tree, and one fixed-size view
// ---------------------------------------------------------------------------

fn memory(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(LEAD, [("memory", GAP)])?;
    let mut scene = PlanBuilder::new("optchat-memory", reading.duration());
    let [said] = reading.place(&mut scene);

    let plan = StagePlan {
        post: StagePost::RESTRAINED,
        elements: vec![
            StageElement::card("log", [300.0, 430.0, 0.0], [300.0, 120.0], "the log")
                .statuses(&[("every line, kept word for word", Tone::Success)])
                .tone(Tone::Success),
            StageElement::card("m1", [560.0, 790.0, -10.0], [140.0, 64.0], "msg 1")
                .tone(Tone::Muted),
            StageElement::card("m2", [720.0, 790.0, -10.0], [140.0, 64.0], "msg 2")
                .tone(Tone::Muted),
            StageElement::card("m3", [880.0, 790.0, -10.0], [140.0, 64.0], "msg 3")
                .tone(Tone::Muted),
            StageElement::card("m4", [1040.0, 790.0, -10.0], [140.0, 64.0], "msg 4")
                .tone(Tone::Muted),
            StageElement::card("s12", [640.0, 560.0, 0.0], [180.0, 72.0], "sum 1-2")
                .tone(Tone::Accent),
            StageElement::card("s34", [960.0, 560.0, 0.0], [180.0, 72.0], "sum 3-4")
                .tone(Tone::Accent),
            StageElement::card("root", [800.0, 430.0, 0.0], [220.0, 78.0], "sum 1-4")
                .tone(Tone::Accent),
            StageElement::card("view", [800.0, 890.0, 0.0], [760.0, 90.0], "the view")
                .statuses(&[("recent whole · older summarized", Tone::Muted)])
                .tone(Tone::Success),
            StageElement::beam("stream", "log", "m1").tone(Tone::Success),
            StageElement::beam("p1", "m1", "s12"),
            StageElement::beam("p2", "m2", "s12"),
            StageElement::beam("p3", "m3", "s34"),
            StageElement::beam("p4", "m4", "s34"),
            StageElement::beam("up1", "s12", "root").tone(Tone::Accent),
            StageElement::beam("up2", "s34", "root").tone(Tone::Accent),
            StageElement::beam("render", "root", "view").tone(Tone::Success),
            StageElement::packet("l1", "stream")
                .labeled("msg")
                .tone(Tone::Plain),
            StageElement::packet("l2", "stream")
                .labeled("msg")
                .tone(Tone::Plain),
            StageElement::packet("l3", "stream")
                .labeled("msg")
                .tone(Tone::Plain),
            StageElement::packet("v1", "render")
                .labeled("view")
                .tone(Tone::Success),
        ],
    };
    let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;

    let mut head = header(
        &mut scene,
        "optchat",
        "memory mode · the log, a tree, a view",
    )?;
    head.show(&mut scene, seconds(0.18));
    let mut status = chip(&mut scene, "chip", Tone::Success, "memory mode")?;
    status.show(&mut scene, seconds(0.3));

    // ---- the log is kept word for word ----------------------------------
    let word_for_word = said.at("word for word");
    stage.settle_in(
        &mut scene,
        "log",
        word_for_word.saturating_sub(seconds(0.3)),
    );
    let streamed = stage.connect(
        &mut scene,
        "stream",
        word_for_word.saturating_sub(seconds(0.2)),
        0.5,
    );
    let l1 = stage.send(&mut scene, "l1", streamed, 0.5);
    stage.send(&mut scene, "l2", l1 + seconds(0.35), 0.5);
    stage.send(&mut scene, "l3", l1 + seconds(0.7), 0.5);
    sfx::TICK.play(&mut scene, "memory-stream", word_for_word, -20.0);

    let mut f1 = footer(
        &mut scene,
        "footer-log",
        vec![
            span("the log is kept ", Tone::Plain),
            span("word for word", Tone::Success),
        ],
    )?;
    f1.type_in(&mut scene, word_for_word, TYPE, 0.6);

    // ---- a cheap model writes one line per message ----------------------
    let cheap = said.at("a cheap model");
    stage.settle_in(&mut scene, "m1", cheap);
    stage.settle_in(&mut scene, "m2", cheap + seconds(0.12));
    stage.settle_in(&mut scene, "m3", cheap + seconds(0.24));
    stage.settle_in(&mut scene, "m4", cheap + seconds(0.36));
    f1.hide(&mut scene, cheap.saturating_sub(seconds(0.2)));
    let mut f2 = footer(
        &mut scene,
        "footer-lines",
        vec![
            span("a cheap model writes ", Tone::Plain),
            span("one line per message", Tone::Accent),
        ],
    )?;
    f2.type_in(&mut scene, cheap, TYPE, 0.6);

    // ---- the lines pair up into a tree ----------------------------------
    let merge = said.at("lines pair up into a tree");
    f2.hide(&mut scene, merge.saturating_sub(seconds(0.2)));
    stage.connect(&mut scene, "p1", merge.saturating_sub(seconds(0.5)), 0.4);
    stage.connect(&mut scene, "p2", merge.saturating_sub(seconds(0.5)), 0.4);
    stage.connect(&mut scene, "p3", merge.saturating_sub(seconds(0.35)), 0.4);
    stage.connect(&mut scene, "p4", merge.saturating_sub(seconds(0.35)), 0.4);
    let s12 = stage.settle_in(&mut scene, "s12", merge);
    stage.settle_in(&mut scene, "s34", merge + seconds(0.1));
    stage.land(&mut scene, "s12", s12);
    let b12 = stage.connect(&mut scene, "up1", merge + seconds(0.9), 0.45);
    stage.connect(&mut scene, "up2", merge + seconds(0.9), 0.45);
    let root = stage.settle_in(&mut scene, "root", merge + seconds(1.4));
    stage.land(&mut scene, "root", root);
    sfx::TICK.play(&mut scene, "memory-merge", b12, -18.0);
    sfx::BLOOM.play(&mut scene, "memory-root", root, -12.0);
    let mut f3 = footer(
        &mut scene,
        "footer-tree",
        vec![
            span("the lines pair up ", Tone::Plain),
            span("into a tree", Tone::Accent),
        ],
    )?;
    f3.type_in(&mut scene, merge, TYPE, 0.6);

    // ---- the log outgrows the window; the view never does ---------------
    let outgrows = said.at("the log outgrows the window");
    f3.hide(&mut scene, outgrows.saturating_sub(seconds(0.25)));
    let mut window = MeterActor::declare(
        &mut scene,
        "log",
        &MeterPlan::bar(
            [520.0, 205.0],
            700.0,
            AxisPlan::new([0.0, 1_200_000.0]).nice(4),
        )
        .label("the log")
        .readout(ReadoutFormat::new(0))
        .threshold(128_000.0, Tone::Warning)
        .threshold(1_100_000.0, Tone::Error),
        0.0,
    )?;
    window.show(&mut scene, outgrows.saturating_sub(seconds(0.3)));
    window.sweep(&mut scene, outgrows, 1_200_000.0, 1.5);
    let mut f4 = footer(
        &mut scene,
        "footer-window",
        vec![
            span("the log outgrows the window — ", Tone::Plain),
            span("that is what forces a compaction", Tone::Error),
        ],
    )?;
    f4.type_in(&mut scene, outgrows, TYPE, 0.6);

    let view_never = said.at("the view never does");
    window.hide(&mut scene, view_never.saturating_sub(seconds(0.2)));
    f4.hide(&mut scene, view_never.saturating_sub(seconds(0.3)));
    stage.settle_in(&mut scene, "view", view_never);
    stage.connect(
        &mut scene,
        "render",
        view_never.saturating_sub(seconds(0.3)),
        0.4,
    );
    let arrived = stage.send(&mut scene, "v1", view_never + seconds(0.5), 0.45);
    stage.land(&mut scene, "view", arrived);
    sfx::BLOOM.play(&mut scene, "memory-view", arrived, -14.0);
    let mut f5 = footer(
        &mut scene,
        "footer-view",
        vec![
            span("the view never does — ", Tone::Plain),
            span("it stays a fixed size", Tone::Success),
        ],
    )?;
    f5.type_in(&mut scene, view_never, TYPE, 0.6);

    // ---- every turn carries the view: the two pills ---------------------
    // The red pill is the whole session log; the green is the fixed view. No
    // number here describes what ONE request carries except the 95k view.
    let carries = said.at("every turn carries");
    let mut tokens = RollingNumberActor::declare(
        &mut scene,
        "tokens",
        RollingNumberPlan::new([960.0, 170.0], 52.0, "0")
            .aligned(CaptionAlign::Center)
            .tone(Tone::Error)
            .prefix(vec![span("the log so far ", Tone::Muted)])
            .suffix(vec![span("k tokens", Tone::Muted)])
            .chip(),
    )?;
    let mut smaller = RollingNumberActor::declare(
        &mut scene,
        "smaller",
        RollingNumberPlan::new([960.0, 262.0], 52.0, "0")
            .aligned(CaptionAlign::Center)
            .tone(Tone::Success)
            .prefix(vec![span("the view ", Tone::Muted)])
            .suffix(vec![span("k tokens · ×12 smaller", Tone::Muted)])
            .chip(),
    )?;
    f5.hide(&mut scene, carries.saturating_sub(seconds(0.25)));
    tokens.show(&mut scene, carries);
    tokens.roll(&mut scene, carries + seconds(0.2), "1200")?;
    smaller.show(&mut scene, carries + seconds(0.35));
    smaller.roll(&mut scene, carries + seconds(0.55), VIEW_TOKENS)?;
    sfx::MARK.play(
        &mut scene,
        "memory-log-count",
        carries + seconds(0.2),
        -16.0,
    );

    let ratio = said.at("tokens,12 times less");
    sfx::BLOOM.play(&mut scene, "memory-ratio", ratio, -13.0);
    let clean = said.at("starts clean");
    stage.hit(&mut scene, "view.flash", clean, 0.5, 0.0);
    let mut f6 = footer(
        &mut scene,
        "footer-clean",
        vec![
            span("every turn starts from ", Tone::Plain),
            span("a clean, fixed-size view", Tone::Success),
        ],
    )?;
    f6.type_in(&mut scene, clean.saturating_sub(seconds(0.9)), TYPE, 0.6);

    // ---- the honest boundary label --------------------------------------
    let no_rot = said.at("no context rot");
    f6.hide(&mut scene, no_rot.saturating_sub(seconds(0.2)));
    let mut bound = footer(
        &mut scene,
        "footer-bound",
        vec![
            span("~1,000-message run · ", Tone::Muted),
            span("the log is an upper bound", Tone::Warning),
            span(" — past the window OpenCode compacts", Tone::Muted),
        ],
    )?;
    bound.type_in(&mut scene, no_rot, TYPE_LONG, 0.4);
    sfx::CONFIRM.play(&mut scene, "memory-clean", said.at("no compaction"), -14.0);

    scene.finish().context("optchat-memory")
}

// ---------------------------------------------------------------------------
// 3. zoom — a vague line opens down to the original message, then the code
// ---------------------------------------------------------------------------

fn zoom(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(LEAD, [("zoom", GAP)])?;
    let mut scene = PlanBuilder::new("optchat-zoom", reading.duration());
    let [said] = reading.place(&mut scene);

    // A terminal walks the plugin's own zoom tool, call by call: the result of
    // each call is the pair printed under it.
    let mut zoomer = TerminalActor::declare(
        &mut scene,
        "zoom-term",
        TerminalPlan::new([150.0, 230.0], 1260.0, 11)
            .size(22.0)
            .titled("opencode · zoom tool")
            .prompt(vec![span("~/dev ❯ ", Tone::Accent)]),
    )?;

    let mut head = header(&mut scene, "optchat", "zoom · the whole log stays")?;
    head.show(&mut scene, seconds(0.18));
    let mut status = chip(&mut scene, "chip", Tone::Accent, "zoom")?;
    status.show(&mut scene, seconds(0.3));

    // A vague line opens into the lines it was made from, down to the message.
    let vague = said.at("when a line is too vague");
    let opens = said.at("the line opens");
    let any_fact = said.at("any fact in your history");
    let two_tools = said.at("two tools");
    let and_date = said.at("and date");

    zoomer.show(&mut scene, seconds(0.5));
    zoomer.type_command(
        &mut scene,
        vague.saturating_sub(seconds(0.5)),
        "zoom(64, 8)",
    )?;
    zoomer.print(
        &mut scene,
        opens,
        [
            vec![
                span("node 64    ", Tone::Warning),
                span("a vague line · covers 8 messages", Tone::Muted),
            ],
            vec![
                span("→ 8 lines under it: ", Tone::Accent),
                span("32 40 44 48 52 56 60 62", Tone::Plain),
            ],
        ],
    )?;

    let mut first = footer(
        &mut scene,
        "footer-a",
        vec![
            span("a vague line opens into ", Tone::Plain),
            span("the two it was made from", Tone::Accent),
        ],
    )?;
    first.type_in(&mut scene, opens, TYPE, 0.7);

    let original = said.at("original message");
    zoomer.type_command(
        &mut scene,
        original.saturating_sub(seconds(0.6)),
        "zoom(64, 1)",
    )?;
    zoomer.print(
        &mut scene,
        original,
        [vec![
            span("→ \"explain how the engine loop works\"", Tone::Success),
            span("   the original, word for word", Tone::Muted),
        ]],
    )?;
    sfx::BLOOM.play(&mut scene, "orig", original, -14.0);

    // The real code: the hook that rebuilds the context, and the two tools.
    let diff = Diff {
        file_name: "optchat/index.ts",
        lines: vec![
            keep("session.hook(\"context\", async (event) => {"),
            add(1, "  const chat = await getChat(event.sessionID)"),
            add(1, "  await ingest(chat, event.messages)    // log verbatim"),
            add(1, "  const view = C.renderView(chat.state, \"line\")"),
            add(1, "  event.messages = buildTurn(chat, event.messages)"),
            add(2, "editor.add({ name: \"zoom\", input: { id, n } })"),
            add(
                2,
                "  return C.zoomText(state, id, n)    // the message, whole",
            ),
            add(2, "editor.add({ name: \"date\", input: { id } })"),
            add(2, "  return state.messages[id].date    // when it was said"),
            keep("})"),
        ],
    };
    let editor = diff.declare(&mut scene, &[any_fact, two_tools], seconds(0.9), false)?;
    // The editor is held back until the code beat, then rises into place.
    let editor_opacity = scene.channel(editor.actor(), "panel-opacity", 0.0);
    scene.spring(&editor_opacity, any_fact, 1.0, 0.5, 0.0);

    // Hide the walk as the code comes in.
    let clear = any_fact.saturating_sub(seconds(0.5));
    zoomer.hide(&mut scene, clear);
    first.hide(&mut scene, any_fact.saturating_sub(seconds(0.2)));

    let mut caption = footer(
        &mut scene,
        "footer-code",
        vec![
            span("condensed for display · ", Tone::Muted),
            span("the hook that rebuilds the context", Tone::Plain),
        ],
    )?;
    caption.type_in(&mut scene, any_fact, TYPE, 0.7);

    let mut tools = footer(
        &mut scene,
        "footer-tools",
        vec![
            span("the plugin's own tools: ", Tone::Plain),
            span("zoom", Tone::Accent),
            span(" and ", Tone::Plain),
            span("date", Tone::Accent),
        ],
    )?;
    tools.type_in(&mut scene, two_tools, TYPE, 0.8);
    caption.hide(&mut scene, two_tools.saturating_sub(seconds(0.2)));

    // Any fact is a few zooms away.
    let mut closing = footer(
        &mut scene,
        "footer-away",
        vec![
            span("any fact in your history is ", Tone::Plain),
            span("a few zooms away", Tone::Success),
        ],
    )?;
    closing.type_in(&mut scene, and_date, TYPE_LONG, 0.4);
    tools.hide(&mut scene, and_date.saturating_sub(seconds(0.2)));

    scene.finish().context("optchat-zoom")
}

// ---------------------------------------------------------------------------
// 4. orchestrator — one chat: a mission and a ledger, sub-agents fanning out,
//    the chat never rewritten so the prompt cache keeps hitting
// ---------------------------------------------------------------------------

fn orchestrator(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(LEAD, [("orchestrator", GAP)])?;
    let mut scene = PlanBuilder::new("optchat-orchestrator", reading.duration());
    let [said] = reading.place(&mut scene);

    let submissions = ["api", "tests", "docs", "schema"];
    let mut elements = vec![
        StageElement::card(
            "chat",
            [560.0, 300.0, 0.0],
            [700.0, 190.0],
            "the orchestrator's chat",
        )
        .statuses(&[
            ("the mission · a ledger of finished runs", Tone::Plain),
            ("append-only · never rewritten", Tone::Success),
        ])
        .tone(Tone::Success),
        StageElement::card(
            "cache",
            [1500.0, 300.0, 0.0],
            [420.0, 130.0],
            "the prompt cache",
        )
        .statuses(&[("the head never changes → keeps hitting", Tone::Success)])
        .tone(Tone::Success),
        StageElement::label(
            "head",
            [960.0, 150.0, 0.0],
            22.0,
            &[
                ("head: the mission + the ledger ", Tone::Muted),
                ("— the same, turn after turn", Tone::Success),
            ],
        ),
        StageElement::label(
            "runs",
            [960.0, 500.0, 0.0],
            20.0,
            &[
                ("the ledger:  ", Tone::Muted),
                ("run 12 ✓   run 11 ✓   run 10 ✓", Tone::Plain),
            ],
        ),
        StageElement::beam("cache-line", "chat", "cache").tone(Tone::Success),
    ];
    let positions = [
        [330.0, 760.0, 0.0],
        [730.0, 760.0, 0.0],
        [1130.0, 760.0, 0.0],
        [1530.0, 760.0, 0.0],
    ];
    for (index, subject) in submissions.iter().enumerate() {
        elements.push(
            StageElement::card(
                &format!("sa{index}"),
                positions[index],
                [340.0, 170.0],
                &format!("sub-agent · {subject}"),
            )
            .statuses(&[
                ("its own brief · its own context", Tone::Plain),
                ("a diff, a log, a dead end", Tone::Muted),
            ])
            .tone(Tone::Accent),
        );
        elements.push(
            StageElement::beam(&format!("fan{index}"), "chat", &format!("sa{index}"))
                .tone(Tone::Accent),
        );
        elements.push(
            StageElement::packet(&format!("brief{index}"), &format!("fan{index}"))
                .labeled("brief")
                .tone(Tone::Accent),
        );
    }
    let plan = StagePlan {
        post: StagePost::RESTRAINED,
        elements,
    };
    let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;

    let mut head = header(
        &mut scene,
        "optchat",
        "orchestrator mode · one chat holds the mission and the ledger",
    )?;
    head.show(&mut scene, seconds(0.18));
    let mut status = chip(&mut scene, "chip", Tone::Accent, "orchestrator")?;
    status.show(&mut scene, seconds(0.3));

    // ---- one conversation holds the mission and the ledger --------------
    let holds = said.at("one conversation holds nothing");
    stage.settle_in(&mut scene, "chat", holds);
    stage.fade_in(&mut scene, "head", holds, 1.0, 0.4);
    let ledger = said.at("mission and a ledger");
    let mut f1 = footer(
        &mut scene,
        "footer-holds",
        vec![
            span("one conversation holds nothing but ", Tone::Plain),
            span("the mission and a ledger", Tone::Success),
        ],
    )?;
    f1.type_in(&mut scene, ledger, TYPE, 0.6);
    stage.fade_in(
        &mut scene,
        "runs",
        said.at("a ledger of finished runs"),
        1.0,
        0.4,
    );

    // ---- the noise fans out into sub-agent sessions ---------------------
    let noise = said.at("every piece of noise");
    f1.hide(&mut scene, noise.saturating_sub(seconds(0.2)));
    for index in 0..submissions.len() {
        let at = noise + seconds(index as f64 * 0.5);
        let beam = format!("fan{index}");
        stage.settle_in(&mut scene, &format!("sa{index}"), at);
        stage.connect(&mut scene, &beam, at + seconds(0.2), 0.5);
        let arrival = stage.send(
            &mut scene,
            &format!("brief{index}"),
            at + seconds(0.4),
            0.55,
        );
        stage.land(&mut scene, &format!("sa{index}"), arrival);
        stage.hit(&mut scene, &format!("sa{index}.flash"), arrival, 0.5, 0.0);
        sfx::TICK.play(&mut scene, &beam, at, -20.0);
    }
    let mut f2 = footer(
        &mut scene,
        "footer-noise",
        vec![
            span(
                "every piece of noise — a diff, a log, a dead end — ",
                Tone::Plain,
            ),
            span("stays in a sub-agent session", Tone::Accent),
        ],
    )?;
    f2.type_in(&mut scene, noise, TYPE, 0.6);
    // The noise is named inside the boxes as it starts happening.
    let diff = said.at("a diff");
    for index in 0..submissions.len() {
        stage.swap_status(
            &mut scene,
            &format!("sa{index}"),
            diff + seconds(index as f64 * 0.12),
            [0, 1],
            0.4,
        );
    }

    let session = said.at("sub-agent session");
    f2.hide(&mut scene, session.saturating_sub(seconds(0.2)));
    let mut f3 = footer(
        &mut scene,
        "footer-session",
        vec![
            span("a sub-agent session it ", Tone::Muted),
            span("starts, follows, and resumes", Tone::Accent),
            span(" — the noise never reaches the chat", Tone::Success),
        ],
    )?;
    f3.type_in(&mut scene, session, TYPE, 0.6);

    // ---- the chat stays short, append-only, never rewritten -------------
    let short = said.at("your chat stays short");
    f3.hide(&mut scene, short.saturating_sub(seconds(0.25)));
    stage.hit(&mut scene, "chat.flash", short, 0.5, 0.0);
    let mut f4 = footer(
        &mut scene,
        "footer-short",
        vec![
            span("your chat stays ", Tone::Plain),
            span("short, append-only", Tone::Success),
            span(", never rewritten", Tone::Plain),
        ],
    )?;
    f4.type_in(&mut scene, short, TYPE, 0.6);
    // The chat's second status line: append-only, never rewritten.
    stage.swap_status(&mut scene, "chat", said.at("append only"), [0, 1], 0.4);

    // ---- ...so the cache keeps hitting ----------------------------------
    let rewritten = said.at("never rewritten");
    stage.settle_in(&mut scene, "cache", rewritten);
    stage.connect(
        &mut scene,
        "cache-line",
        rewritten.saturating_sub(seconds(0.2)),
        0.5,
    );
    stage.land(&mut scene, "cache", rewritten + seconds(0.4));

    let hitting = said.at("the cache keeps hitting");
    f4.hide(&mut scene, hitting.saturating_sub(seconds(0.2)));
    stage.hit(&mut scene, "cache.flash", hitting, 0.6, 0.0);
    sfx::SUCCESS.play(&mut scene, "cache", hitting, -13.0);
    let mut f5 = footer(
        &mut scene,
        "footer-cache",
        vec![
            span(
                "the head never changes while the sub-agents churn — ",
                Tone::Plain,
            ),
            span("the prompt cache keeps hitting", Tone::Success),
        ],
    )?;
    f5.type_in(&mut scene, hitting, TYPE, 1.0);

    // ---- while N sub-agents work ----------------------------------------
    let work = said.at("sub-agents work");
    stage.hit(&mut scene, "chat.flash", work, 0.4, 0.0);
    stage.fade_in(
        &mut scene,
        "runs",
        work.saturating_sub(seconds(0.2)),
        1.0,
        0.4,
    );

    scene.finish().context("optchat-orchestrator")
}

// ---------------------------------------------------------------------------
// 5. fanout — the orchestration loop: one sub-agent per subject, logged,
//    status, collected, stopped, resumed — and the orchestrator's tools
// ---------------------------------------------------------------------------

fn fanout(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(LEAD, [("fanout", GAP)])?;
    let mut scene = PlanBuilder::new("optchat-fanout", reading.duration());
    let [said] = reading.place(&mut scene);

    let plan = StagePlan {
        post: StagePost::RESTRAINED,
        elements: vec![
            // the fan-out
            StageElement::card("fan", [870.0, 240.0, 0.0], [300.0, 110.0], "spawn")
                .statuses(&[("one sub-agent per subject", Tone::Plain)])
                .tone(Tone::Accent),
            StageElement::card(
                "parallel",
                [1260.0, 240.0, 0.0],
                [300.0, 110.0],
                "in parallel",
            )
            .statuses(&[("its own brief · its own context", Tone::Muted)])
            .tone(Tone::Accent),
            StageElement::card("noise", [1630.0, 240.0, 0.0], [280.0, 110.0], "the noise")
                .statuses(&[("never reaches you", Tone::Success)])
                .tone(Tone::Success),
            // the loop
            StageElement::card("log", [1630.0, 480.0, 0.0], [280.0, 100.0], "log")
                .statuses(&[("one line per run", Tone::Plain)])
                .tone(Tone::Plain),
            StageElement::card("ask", [1260.0, 480.0, 0.0], [300.0, 100.0], "status")
                .statuses(&[("asks, never blocks", Tone::Plain)])
                .tone(Tone::Plain),
            StageElement::card("collect", [870.0, 480.0, 0.0], [300.0, 100.0], "collect")
                .statuses(&[("what is done", Tone::Plain)])
                .tone(Tone::Plain),
            StageElement::card("stop", [870.0, 720.0, 0.0], [300.0, 100.0], "stop")
                .statuses(&[("what no longer matters", Tone::Muted)])
                .tone(Tone::Warning),
            StageElement::card("resume", [1260.0, 720.0, 0.0], [320.0, 100.0], "resume")
                .statuses(&[("the same sub-agent", Tone::Success)])
                .tone(Tone::Success),
            StageElement::card("evidence", [1630.0, 720.0, 0.0], [280.0, 100.0], "evidence")
                .statuses(&[("never a summary", Tone::Success)])
                .tone(Tone::Success),
            // the loop-back, routed clear of the middle card
            StageElement::Path {
                id: "back".to_owned(),
                through: vec![
                    Waypoint::Element("resume".to_owned()),
                    Waypoint::Point([1440.0, 600.0, 0.0]),
                    Waypoint::Element("parallel".to_owned()),
                ],
                curve: Curve::Straight,
                corner: 90.0,
                bend: 0.0,
                tone: Tone::Success,
                width: 4.0,
                dash: Some([14.0, 10.0]),
                arrow: Arrow::End,
            },
            // the wires
            StageElement::beam("b-fan", "fan", "parallel").tone(Tone::Accent),
            StageElement::beam("b-noise", "parallel", "noise").tone(Tone::Success),
            StageElement::beam("b-down", "noise", "log").tone(Tone::Plain),
            StageElement::beam("b-ask", "log", "ask").tone(Tone::Plain),
            StageElement::beam("b-collect", "ask", "collect").tone(Tone::Plain),
            StageElement::beam("b-stop", "collect", "stop").tone(Tone::Warning),
            StageElement::beam("b-resume", "stop", "resume").tone(Tone::Success),
            StageElement::beam("b-evidence", "resume", "evidence").tone(Tone::Success),
            // what travels on them
            StageElement::packet("p-fan", "b-fan")
                .labeled("subject")
                .tone(Tone::Accent),
            StageElement::packet("p-line", "b-down")
                .labeled("one line")
                .tone(Tone::Plain),
            StageElement::packet("p-ask", "b-ask")
                .labeled("status")
                .tone(Tone::Plain),
            StageElement::packet("p-collect", "b-collect")
                .labeled("what is done")
                .tone(Tone::Plain),
            StageElement::packet("p-stop", "b-stop")
                .labeled("stop")
                .tone(Tone::Warning),
            StageElement::packet("p-resume", "b-resume")
                .labeled("resume")
                .tone(Tone::Success),
            StageElement::packet("p-evidence", "b-evidence")
                .labeled("evidence")
                .tone(Tone::Success),
        ],
    };
    let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;

    let mut head = header(
        &mut scene,
        "optchat",
        "the fan-out · one sub-agent per subject",
    )?;
    head.show(&mut scene, seconds(0.18));
    let mut status = chip(&mut scene, "chip", Tone::Accent, "the loop")?;
    status.show(&mut scene, seconds(0.3));

    // The orchestrator's tools, exactly as it exposes them, as call → result.
    let mut tools = TerminalActor::declare(
        &mut scene,
        "tools",
        TerminalPlan::new([150.0, 190.0], 520.0, 22)
            .size(17.0)
            .titled("the orchestrator's tools")
            .prompt(vec![span("❯ ", Tone::Accent)]),
    )?;
    tools.show(&mut scene, seconds(0.5));

    let fans = said.at("fans out");
    let subjects = said.at("one sub-agent per subject");
    let parallel = said.at("running in parallel");
    let reaches = said.at("never reaches you");
    let line_per_run = said.at("logs one line per run");
    let asks = said.at("asks status");
    let collect = said.at("collects what is done");
    let stop = said.at("interrupts what matters");
    let resume = said.at("resumes the same sub-agent");
    let evidence = said.at("never trusts a summary");

    // ---- one sub-agent per subject, in parallel -------------------------
    tools.print(
        &mut scene,
        fans,
        [
            vec![span("spawn    start a sub-agent on a subject", Tone::Plain)],
            vec![span("collect  what a finished run returned", Tone::Plain)],
            vec![span("status   the ledger, without blocking", Tone::Plain)],
            vec![span(
                "stop     end a run that no longer matters",
                Tone::Plain,
            )],
            vec![span("note     one line into the ledger", Tone::Plain)],
            vec![span("find     search the log", Tone::Plain)],
            vec![span("zoom     open a line into its parts", Tone::Plain)],
            vec![span("date     when a line was said", Tone::Plain)],
        ],
    )?;
    stage.settle_in(&mut scene, "fan", subjects.saturating_sub(seconds(0.2)));
    stage.connect(&mut scene, "b-fan", fans, 0.5);
    let fanned = stage.send(&mut scene, "p-fan", fans + seconds(0.3), 0.55);
    stage.land(&mut scene, "parallel", fanned);
    stage.settle_in(
        &mut scene,
        "parallel",
        parallel.saturating_sub(seconds(0.3)),
    );
    stage.connect(&mut scene, "b-noise", parallel, 0.5);
    stage.settle_in(&mut scene, "noise", reaches.saturating_sub(seconds(0.3)));
    stage.hit(&mut scene, "noise.flash", reaches, 0.5, 0.0);

    let mut f1 = footer(
        &mut scene,
        "footer-fan",
        vec![
            span("say the word: ", Tone::Plain),
            span("one sub-agent per subject", Tone::Accent),
            span(", in parallel", Tone::Plain),
        ],
    )?;
    f1.type_in(&mut scene, fans, TYPE, 0.6);
    f1.hide(&mut scene, reaches.saturating_sub(seconds(0.2)));
    let mut f2 = footer(
        &mut scene,
        "footer-noise",
        vec![
            span("each with its own brief and context — ", Tone::Plain),
            span("their noise never reaches you", Tone::Success),
        ],
    )?;
    f2.type_in(&mut scene, reaches, TYPE, 0.6);

    // ---- the loop: log, status, collect, stop, resume -------------------
    f2.hide(&mut scene, line_per_run.saturating_sub(seconds(0.25)));
    stage.connect(
        &mut scene,
        "b-down",
        line_per_run.saturating_sub(seconds(0.4)),
        0.5,
    );
    stage.settle_in(&mut scene, "log", line_per_run.saturating_sub(seconds(0.3)));
    let logged = stage.send(
        &mut scene,
        "p-line",
        line_per_run.saturating_sub(seconds(0.2)),
        0.5,
    );
    stage.land(&mut scene, "log", logged);
    let mut f3 = footer(
        &mut scene,
        "footer-log",
        vec![
            span("the orchestrator logs ", Tone::Plain),
            span("one line per run", Tone::Plain),
        ],
    )?;
    f3.type_in(&mut scene, line_per_run, TYPE, 0.6);

    f3.hide(&mut scene, asks.saturating_sub(seconds(0.2)));
    stage.connect(&mut scene, "b-ask", asks.saturating_sub(seconds(0.4)), 0.4);
    stage.settle_in(&mut scene, "ask", asks.saturating_sub(seconds(0.3)));
    stage.send(&mut scene, "p-ask", asks.saturating_sub(seconds(0.1)), 0.45);
    tools.type_command(&mut scene, asks, "status")?;
    tools.print(
        &mut scene,
        asks + seconds(0.6),
        [
            vec![span("run 12  api audit      done", Tone::Plain)],
            vec![span("run 13  test sweep     running", Tone::Plain)],
            vec![span("run 14  docs pass      queued", Tone::Muted)],
        ],
    )?;
    let mut f4 = footer(
        &mut scene,
        "footer-status",
        vec![
            span("it asks ", Tone::Plain),
            span("status", Tone::Accent),
            span(" instead of blocking", Tone::Plain),
        ],
    )?;
    f4.type_in(&mut scene, asks, TYPE, 0.6);

    f4.hide(&mut scene, collect.saturating_sub(seconds(0.25)));
    stage.connect(
        &mut scene,
        "b-collect",
        collect.saturating_sub(seconds(0.5)),
        0.4,
    );
    stage.settle_in(&mut scene, "collect", collect.saturating_sub(seconds(0.3)));
    let done = stage.send(&mut scene, "p-collect", collect, 0.45);
    stage.land(&mut scene, "collect", done);
    sfx::TICK.play(
        &mut scene,
        "b-collect",
        collect.saturating_sub(seconds(0.5)),
        -18.0,
    );
    tools.type_command(&mut scene, collect + seconds(0.5), "collect 12")?;
    tools.print(
        &mut scene,
        collect + seconds(1.3),
        [vec![span(
            "→ 3 files touched · no schema change",
            Tone::Success,
        )]],
    )?;
    let mut f5 = footer(
        &mut scene,
        "footer-collect",
        vec![
            span("it collects ", Tone::Plain),
            span("what is done", Tone::Plain),
        ],
    )?;
    f5.type_in(&mut scene, collect, TYPE, 0.6);

    f5.hide(&mut scene, stop.saturating_sub(seconds(0.25)));
    stage.connect(&mut scene, "b-stop", stop.saturating_sub(seconds(0.5)), 0.4);
    stage.settle_in(&mut scene, "stop", stop.saturating_sub(seconds(0.3)));
    stage.send(&mut scene, "p-stop", stop, 0.45);
    let mut f6 = footer(
        &mut scene,
        "footer-stop",
        vec![
            span("it stops ", Tone::Warning),
            span("what no longer matters", Tone::Plain),
        ],
    )?;
    f6.type_in(&mut scene, stop, TYPE, 0.6);

    f6.hide(&mut scene, resume.saturating_sub(seconds(0.25)));
    stage.connect(
        &mut scene,
        "b-resume",
        resume.saturating_sub(seconds(0.5)),
        0.4,
    );
    stage.settle_in(&mut scene, "resume", resume.saturating_sub(seconds(0.3)));
    let resumed = stage.send(&mut scene, "p-resume", resume, 0.45);
    stage.land(&mut scene, "resume", resumed);
    stage.connect(&mut scene, "back", resume + seconds(0.6), 0.6);
    let mut f7 = footer(
        &mut scene,
        "footer-resume",
        vec![
            span("it resumes ", Tone::Plain),
            span("the same sub-agent", Tone::Success),
            span(" when a subject needs more", Tone::Plain),
        ],
    )?;
    f7.type_in(&mut scene, resume, TYPE, 0.6);

    // ---- evidence, not a summary ----------------------------------------
    f7.hide(&mut scene, evidence.saturating_sub(seconds(0.3)));
    stage.fade_in(&mut scene, "evidence", evidence, 1.0, 0.4);
    stage.connect(
        &mut scene,
        "b-evidence",
        evidence.saturating_sub(seconds(0.3)),
        0.4,
    );
    let landed = stage.send(&mut scene, "p-evidence", evidence, 0.45);
    stage.land(&mut scene, "evidence", landed);
    stage.hit(&mut scene, "evidence.flash", landed, 0.5, 0.0);
    sfx::BLOOM.play(&mut scene, "evidence", landed, -12.0);
    tools.type_command(&mut scene, evidence + seconds(0.4), "collect 14")?;
    tools.print(
        &mut scene,
        evidence + seconds(1.2),
        [vec![span(
            "→ the diff, re-read by a second sub-agent",
            Tone::Success,
        )]],
    )?;
    let mut f8 = footer(
        &mut scene,
        "footer-evidence",
        vec![
            span("it never trusts a summary — ", Tone::Plain),
            span("evidence", Tone::Success),
            span(", a second sub-agent checks the first", Tone::Plain),
        ],
    )?;
    f8.type_in(&mut scene, evidence, TYPE, 0.6);

    scene.finish().context("optchat-fanout")
}

// ---------------------------------------------------------------------------
// 6. tui — the pop-up: two switches, Stats, View, Summaries, Settings
// ---------------------------------------------------------------------------

fn tui(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(LEAD, [("tui", GAP)])?;
    let mut scene = PlanBuilder::new("optchat-tui", reading.duration());
    let [said] = reading.place(&mut scene);

    let mut head = header(&mut scene, "optchat", "the pop-up · two switches")?;
    head.show(&mut scene, seconds(0.18));
    let mut status = chip(&mut scene, "chip", Tone::Accent, "the pop-up")?;
    status.show(&mut scene, seconds(0.3));

    // ---- Stats: the two switches, then what a turn carries and costs -----
    let mut stats = TerminalActor::declare(
        &mut scene,
        "term-stats",
        TerminalPlan::new([150.0, 250.0], 920.0, 12)
            .size(22.0)
            .titled("opencode · /optchat")
            .prompt(vec![span("~/dev ❯ ", Tone::Accent)]),
    )?;
    stats.show(&mut scene, seconds(0.6));
    stats.type_command(&mut scene, seconds(1.0), "/optchat")?;

    let popup = said.at("the pop-up is there");
    stats.print(
        &mut scene,
        popup,
        [
            vec![span("optchat  ─  the pop-up", Tone::Accent)],
            vec![span(
                "Memory · Orchestrator · Stats · View · Summaries · Settings",
                Tone::Muted,
            )],
        ],
    )?;

    let switches = said.at("two switches");
    stats.print(
        &mut scene,
        switches,
        [
            vec![span("## Modes", Tone::Accent)],
            vec![span("memory        on", Tone::Success)],
            vec![span("orchestrator  on", Tone::Success)],
        ],
    )?;

    let statistics = said.at("then stats");
    stats.print(
        &mut scene,
        statistics,
        [
            vec![span("## Session", Tone::Accent)],
            vec![
                span("messages    1,000", Tone::Plain),
                span("   · every message kept", Tone::Muted),
            ],
            vec![
                span("transcript  1.5 MB", Tone::Plain),
                span("   · log, never rewritten", Tone::Muted),
            ],
            vec![span("compression ×12", Tone::Success)],
        ],
    )?;

    let carries = said.at("what every turn carries");
    stats.print(
        &mut scene,
        carries,
        [vec![span("## Context carried per turn", Tone::Accent)]],
    )?;

    let bars = BarsPlan::new(
        [1160.0, 440.0],
        660.0,
        AxisPlan::new([0.0, 1.0]).every(0.25),
    )
    .row("cost", "per request")
    .series(BarSeriesPlan::new("full", "full log", Tone::Error))
    .series(BarSeriesPlan::new("optchat", "optchat", Tone::Success))
    .size(24.0)
    .readout(ReadoutFormat::new(2).unit("¢"))
    .delta(BarDeltaPlan::new("full", "optchat").factor());
    let mut bars = BarsActor::declare(&mut scene, "cost", &bars)?;

    let mut opt = RollingNumberActor::declare(
        &mut scene,
        "cost-optchat",
        RollingNumberPlan::new([1080.0, 250.0], 48.0, "0.00")
            .tone(Tone::Success)
            .prefix(vec![span("optchat  ", Tone::Muted)])
            .suffix(vec![span("¢ a request", Tone::Muted)])
            .chip(),
    )?;
    let mut full = RollingNumberActor::declare(
        &mut scene,
        "cost-full",
        RollingNumberPlan::new([1080.0, 320.0], 48.0, "0.00")
            .tone(Tone::Error)
            .prefix(vec![span("full log ", Tone::Muted)])
            .suffix(vec![span("¢ a request", Tone::Muted)])
            .chip(),
    )?;

    bars.show(&mut scene, carries);
    bars.reveal_rows(&mut scene, carries + seconds(0.1));

    let since = said.at("a request");
    bars.grow(&mut scene, since, "optchat", &[("cost", 0.16)])?;
    opt.show(&mut scene, since);
    opt.roll(&mut scene, since, "0.16")?;
    sfx::SUCCESS.play(&mut scene, "cost-optchat", since, -14.0);

    let whole = said.at("the whole log");
    bars.grow(&mut scene, whole, "full", &[("cost", 0.86)])?;
    full.show(&mut scene, whole);
    full.roll(&mut scene, whole, "0.86")?;
    sfx::MARK.play(&mut scene, "cost-full", whole, -16.0);

    let mut bill = footer(
        &mut scene,
        "footer-bill",
        vec![
            span("0.16 ¢ a request against 0.86 ¢ · ", Tone::Plain),
            span("95k tokens against 1,200k (×12)", Tone::Success),
        ],
    )?;
    bill.type_in(&mut scene, since, TYPE, 0.8);

    // ---- View: the context line by line, originals bright ----------------
    let view = said.at("the view");
    let clear = view.saturating_sub(seconds(0.25));
    stats.hide(&mut scene, clear);
    bars.hide(&mut scene, clear);
    opt.hide(&mut scene, clear);
    full.hide(&mut scene, clear);
    bill.hide(&mut scene, clear);

    let mut show_view = TerminalActor::declare(
        &mut scene,
        "term-view",
        TerminalPlan::new([150.0, 210.0], 1420.0, 15)
            .size(20.0)
            .titled("optchat · view sent to the model")
            .prompt(vec![span("~/dev ❯ ", Tone::Accent)]),
    )?;
    show_view.show(&mut scene, view.saturating_sub(seconds(0.1)));

    let originals = said.at("originals bright");
    show_view.print(
        &mut scene,
        originals,
        [
            vec![span(
                "originals bright · summaries on a grey ramp",
                Tone::Muted,
            )],
            vec![span(
                "L0·0012  text     whole   explain how the engine loop works",
                Tone::Plain,
            )],
            vec![span(
                "L0·0013  tool     whole   read packages/opencode/src/session.ts",
                Tone::Plain,
            )],
            vec![span(
                "L0·0014  text     whole   why does the loop re-read everything?",
                Tone::Plain,
            )],
        ],
    )?;

    let fading = said.at("summaries fading");
    show_view.print(
        &mut scene,
        fading,
        [
            vec![span(
                "L1·0004  summary   8 msg  session: the engine loop",
                Tone::Muted,
            )],
            vec![span(
                "L2·0002  summary  32 msg  session: opencode internals",
                Tone::Muted,
            )],
            vec![span(
                "L3·0001  summary 128 msg  session: opencode engine work",
                Tone::Muted,
            )],
        ],
    )?;

    // ---- Summaries: the tree, levels and nodes ---------------------------
    let tree = said.at("the tree");
    let clear = tree.saturating_sub(seconds(0.25));
    show_view.hide(&mut scene, clear);

    let mut summaries = TerminalActor::declare(
        &mut scene,
        "term-summaries",
        TerminalPlan::new([150.0, 230.0], 1240.0, 13)
            .size(20.0)
            .titled("optchat · summaries")
            .prompt(vec![span("~/dev ❯ ", Tone::Accent)]),
    )?;
    summaries.show(&mut scene, tree.saturating_sub(seconds(0.15)));
    summaries.print(
        &mut scene,
        tree,
        [
            vec![span("the tree, level by level", Tone::Muted)],
            vec![span(
                "L3·0001  summary                          covers 1,000",
                Tone::Muted,
            )],
            vec![span(
                "  ├─ L2·0002  summary                     covers  500",
                Tone::Muted,
            )],
            vec![span(
                "  │    ├─ L1·0004  summary                covers  128",
                Tone::Plain,
            )],
            vec![span(
                "  │    └─ L1·0005  summary                covers  128",
                Tone::Plain,
            )],
            vec![span(
                "  └─ L2·0003  summary                     covers  500",
                Tone::Muted,
            )],
            vec![span(
                "       └─ L1·0006  summary                covers  128",
                Tone::Plain,
            )],
        ],
    )?;

    // ---- Settings: the two switches --------------------------------------
    let settings_at = said.at("the settings");
    let clear = settings_at.saturating_sub(seconds(0.25));
    summaries.hide(&mut scene, clear);

    let mut settings = TerminalActor::declare(
        &mut scene,
        "term-settings",
        TerminalPlan::new([150.0, 250.0], 1240.0, 12)
            .size(20.0)
            .titled("optchat · settings")
            .prompt(vec![span("~/dev ❯ ", Tone::Accent)]),
    )?;
    settings.show(&mut scene, settings_at.saturating_sub(seconds(0.15)));
    settings.print(
        &mut scene,
        settings_at,
        [
            vec![span("## Modes", Tone::Accent)],
            vec![span("memory             on", Tone::Success)],
            vec![span("orchestrator       on", Tone::Success)],
            vec![span(
                "compactor model    (the session's model)",
                Tone::Plain,
            )],
            vec![span("context budget     128 kB", Tone::Plain)],
            vec![span("tool result cap    30,000 B", Tone::Plain)],
            vec![span("bytes per token    1.3", Tone::Plain)],
        ],
    )?;

    scene.finish().context("optchat-tui")
}

// ---------------------------------------------------------------------------
// 7. outro — one line to install, the two modes, plain files, and the credits
// ---------------------------------------------------------------------------

fn outro(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(LEAD, [("outro", GAP)])?;
    let mut scene = PlanBuilder::new("optchat-outro", reading.duration());
    let [said] = reading.place(&mut scene);

    let mut head = header(&mut scene, "optchat", "install")?;
    head.show(&mut scene, seconds(0.18));
    let mut status = chip(&mut scene, "chip", Tone::Accent, "install")?;
    status.show(&mut scene, seconds(0.3));

    let mut term = TerminalActor::declare(
        &mut scene,
        "term",
        TerminalPlan::new([260.0, 150.0], 1380.0, 9)
            .size(20.0)
            .titled("install")
            .prompt(vec![span("~/dev ❯ ", Tone::Accent)]),
    )?;
    let shown = term.show(&mut scene, seconds(0.5));

    let install = said.at("one line to install").max(shown);
    let entered = term.type_command(
        &mut scene,
        install.saturating_sub(seconds(0.3)),
        "git clone https://github.com/96Ems/opencode2-plugin-optchat.git ~/.config/opencode/plugins/optchat",
    )?;
    sfx::CONFIRM.play(&mut scene, "install", entered, -14.0);

    let disk = said.at("playing files");
    term.print(
        &mut scene,
        entered + seconds(0.2),
        [vec![span("Cloning into 'optchat'... done.", Tone::Success)]],
    )?;
    term.print(
        &mut scene,
        disk,
        [
            vec![span(
                "main/2026-10-05.jsonl     the log, every message verbatim",
                Tone::Plain,
            )],
            vec![span(
                "tree/2026-10-05.jsonl     the tree, one summary per node",
                Tone::Plain,
            )],
            vec![span(
                "ledger/2026-10-05.jsonl   the ledger, one line per run",
                Tone::Plain,
            )],
        ],
    )?;

    // The two modes, one after the other.
    let memory_mode = said.at("memory mode");
    let mut mode_a = CaptionActor::declare(
        &mut scene,
        "mode-memory",
        &CaptionPlan::line(
            [960.0, 560.0],
            26.0,
            vec![
                span("memory mode ", Tone::Success),
                span("for the conversation you keep", Tone::Plain),
            ],
        )
        .aligned(CaptionAlign::Center),
    )?;
    mode_a.type_in(&mut scene, memory_mode, TYPE, 0.6);

    let orchestrator_mode = said.at("orchestrator mode");
    mode_a.hide(&mut scene, orchestrator_mode.saturating_sub(seconds(0.2)));
    let mut mode_b = CaptionActor::declare(
        &mut scene,
        "mode-orchestrator",
        &CaptionPlan::line(
            [960.0, 630.0],
            26.0,
            vec![
                span("orchestrator mode ", Tone::Accent),
                span("for the work you fan out", Tone::Plain),
            ],
        )
        .aligned(CaptionAlign::Center),
    )?;
    mode_b.type_in(&mut scene, orchestrator_mode, TYPE, 0.6);

    // The closing line: you never compact again.
    let memory = said.at("the chat is the memory");
    mode_b.hide(&mut scene, memory.saturating_sub(seconds(0.3)));
    let mut line_a = CaptionActor::declare(
        &mut scene,
        "closing-memory",
        &CaptionPlan::line(
            [960.0, 690.0],
            28.0,
            vec![
                span("the log is the chat, ", Tone::Plain),
                span("the chat is the memory", Tone::Success),
            ],
        )
        .aligned(CaptionAlign::Center),
    )?;
    line_a.type_in(&mut scene, memory, TYPE, 0.9);

    let compact_again = said.at("never compact again");
    let mut line_b = CaptionActor::declare(
        &mut scene,
        "closing",
        &CaptionPlan::line(
            [960.0, 760.0],
            28.0,
            vec![
                span("and you ", Tone::Plain),
                span("never compact again", Tone::Accent),
            ],
        )
        .aligned(CaptionAlign::Center),
    )?;
    line_b.type_in(&mut scene, compact_again, TYPE, 0.4);
    sfx::BLOOM.play(&mut scene, "closing", compact_again, -14.0);

    // The credits card: small type, bottom, legible.
    let mut credits = TerminalActor::declare(
        &mut scene,
        "credits",
        TerminalPlan::new([260.0, 820.0], 1380.0, 5)
            .size(16.0)
            .titled("credits")
            .prompt(vec![span("", Tone::Muted)]),
    )?;
    credits.show(&mut scene, seconds(0.7));
    credits.print(
        &mut scene,
        seconds(1.1),
        [
            vec![
                span("psychopomp · film   ", Tone::Muted),
                span("MIT · Kit Langton", Tone::Plain),
            ],
            vec![
                span("voice               ", Tone::Muted),
                span("Kokoro-82M (Apache-2.0)", Tone::Plain),
            ],
            vec![
                span("timings             ", Tone::Muted),
                span(
                    "Parakeet-TDT 0.6B (CC-BY-4.0) · onnx-asr (MIT)",
                    Tone::Plain,
                ),
            ],
            vec![
                span("type · render       ", Tone::Muted),
                span(
                    "CommitMono (SIL OFL) · Phosphor (MIT) · FFmpeg",
                    Tone::Plain,
                ),
            ],
            vec![
                span("optchat             ", Tone::Muted),
                span("MIT", Tone::Plain),
            ],
        ],
    )?;

    scene.finish().context("optchat-outro")
}
