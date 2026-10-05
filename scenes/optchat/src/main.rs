//! optchat explainer reel: a narrated, seven-segment film about the optchat
//! plugin. Its point is not memory but the absence of a chore: **you never
//! compact again** — every turn starts from a clean, fixed-size view, and the
//! harness's own compaction request is answered from that view.
//!
//! Every visual beat is keyed to a phrase the recorded narration still says
//! (`scenes/optchat/narration`), so re-voicing the script re-times the film and
//! a stale anchor panics with its clip. Run:
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
    stage::{OrbEntrance, StageActor, StageElement, StagePlan, StagePost},
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
            before(&narration)?,
            after(&narration)?,
            zoom(&narration)?,
            plugin(&narration)?,
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
// 1. intro — the hard limit, and the promise: you never compact again
// ---------------------------------------------------------------------------

fn intro(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(LEAD, [("intro", GAP)])?;
    let mut scene = PlanBuilder::new("optchat-intro", reading.duration());
    let [said] = reading.place(&mut scene);

    let mut head = header(&mut scene, "optchat", "never compact again")?;
    head.show(&mut scene, seconds(0.2));
    let mut status = chip(&mut scene, "chip", Tone::Muted, "before")?;
    status.show(&mut scene, seconds(0.4));

    // A session fills its window; the next one starts from zero.
    let mut term = TerminalActor::declare(
        &mut scene,
        "term",
        TerminalPlan::new([120.0, 200.0], 900.0, 11)
            .size(20.0)
            .titled("opencode — session")
            .prompt(vec![span("~/dev ❯ ", Tone::Accent)]),
    )?;
    let shown = term.show(&mut scene, seconds(0.55));
    let entered = term.type_command(&mut scene, shown + seconds(0.30), "opencode")?;
    term.print(
        &mut scene,
        entered + seconds(0.25),
        [
            vec![span("● session 1 started", Tone::Plain)],
            vec![span("user  explain how the engine loop works", Tone::Plain)],
            vec![span(
                "tool  reading packages/opencode/src/session.ts",
                Tone::Muted,
            )],
        ],
    )?;

    // The window meter climbs to its ceiling at the hard limit.
    let mut meter = MeterActor::declare(
        &mut scene,
        "window",
        &MeterPlan::ring(
            [1540.0, 430.0],
            150.0,
            AxisPlan::new([0.0, 1_200_000.0]).nice(4),
        )
        .label("context carried")
        .readout(ReadoutFormat::new(0))
        .threshold(1_100_000.0, Tone::Error),
        0.0,
    )?;
    let limit = said.at("hard limit");
    meter.show(&mut scene, limit);
    meter.sweep(&mut scene, limit, 1_180_000.0, 2.2);
    meter.flash(&mut scene, limit + seconds(2.2), 0.7);

    let mut f_limit = footer(
        &mut scene,
        "footer-limit",
        vec![
            span("one hard limit: ", Tone::Muted),
            span("the window", Tone::Error),
        ],
    )?;
    f_limit.type_in(&mut scene, limit, TYPE, 0.7);

    // "and the next one starts from zero": compaction rewrites the past away.
    let next = said.at("the next one");
    f_limit.hide(&mut scene, next.saturating_sub(seconds(0.2)));
    term.print(
        &mut scene,
        next,
        [
            vec![span(
                "✻ context full — compaction rewrites the past",
                Tone::Error,
            )],
            vec![span("  the detail is gone", Tone::Muted)],
        ],
    )?;
    meter.set(&mut scene, next + seconds(0.4), 0.0);
    let restarted = term.type_command(&mut scene, next + seconds(0.9), "opencode")?;
    term.print(
        &mut scene,
        restarted + seconds(0.2),
        [vec![span("● session 2 started — from zero", Tone::Warning)]],
    )?;
    let mut f_zero = footer(
        &mut scene,
        "footer-zero",
        vec![
            span("compacted or thrown away — ", Tone::Plain),
            span("the next one starts from zero", Tone::Warning),
        ],
    )?;
    f_zero.type_in(&mut scene, next + seconds(0.2), TYPE, 0.7);

    // "the longer the context, the worse it works": it climbs again, and decays.
    let longer = said.at("the longer the context");
    f_zero.hide(&mut scene, longer.saturating_sub(seconds(0.2)));
    meter.show(&mut scene, longer);
    meter.sweep(&mut scene, longer, 820_000.0, 3.6);
    let mut f_decay = footer(
        &mut scene,
        "footer-decay",
        vec![
            span("the longer the context, ", Tone::Plain),
            span("the worse it works", Tone::Error),
        ],
    )?;
    f_decay.type_in(&mut scene, longer, TYPE, 0.7);

    // "optchat removes the limit": the chip flips to the promise.
    let removes = said.at("removes the limit");
    f_decay.hide(&mut scene, removes.saturating_sub(seconds(0.25)));
    status.hide(&mut scene, removes);
    let mut promise = chip(&mut scene, "chip-promise", Tone::Success, "the promise")?;
    promise.show(&mut scene, removes);

    // "one chat that never ends".
    let endless = said.at("one chat that never ends");
    let mut line_a = CaptionActor::declare(
        &mut scene,
        "endless",
        &CaptionPlan::line(
            [960.0, 720.0],
            28.0,
            vec![
                span("one chat ", Tone::Plain),
                span("that never ends", Tone::Success),
            ],
        )
        .aligned(CaptionAlign::Center),
    )?;
    line_a.type_in(&mut scene, endless, TYPE, 0.9);

    // "and no compaction ever again": the closing promise.
    let no_compaction = said.at("no compaction");
    meter.hide(&mut scene, no_compaction.saturating_sub(seconds(0.3)));
    let mut line_b = CaptionActor::declare(
        &mut scene,
        "no-compaction",
        &CaptionPlan::line(
            [960.0, 830.0],
            28.0,
            vec![
                span("no compaction ", Tone::Plain),
                span("ever again", Tone::Accent),
            ],
        )
        .aligned(CaptionAlign::Center),
    )?;
    line_b.type_in(&mut scene, no_compaction, TYPE, 1.0);

    scene.finish().context("optchat-intro")
}

// ---------------------------------------------------------------------------
// 2. before — one card, one request, the whole history each turn
// ---------------------------------------------------------------------------

fn before(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(LEAD, [("before", GAP)])?;
    let mut scene = PlanBuilder::new("optchat-before", reading.duration());
    let [said] = reading.place(&mut scene);

    let plan = StagePlan {
        post: StagePost::RESTRAINED,
        elements: vec![
            StageElement::card("session", [360.0, 470.0, 0.0], [330.0, 130.0], "session")
                .statuses(&[("1,000 messages", Tone::Plain), ("1.5 MB log", Tone::Muted)])
                .tone(Tone::Request),
            StageElement::card(
                "request",
                [960.0, 720.0, 0.0],
                [440.0, 110.0],
                "one request",
            )
            .statuses(&[("the whole history attached", Tone::Warning)])
            .tone(Tone::Warning),
            StageElement::orb("model", [1620.0, 470.0, 0.0], 120.0).tone(Tone::Plain),
            StageElement::card(
                "cache",
                [1360.0, 720.0, 0.0],
                [320.0, 100.0],
                "prompt cache",
            )
            .statuses(&[("re-read every turn", Tone::Muted)]),
            StageElement::beam("ask", "request", "model").tone(Tone::Request),
            StageElement::beam("read", "cache", "model").tone(Tone::Warning),
            StageElement::packet("turn", "ask")
                .labeled("1.2M tokens")
                .tone(Tone::Request),
            StageElement::packet("reread", "read")
                .reversed()
                .labeled("re-read")
                .tone(Tone::Warning),
        ],
    };
    let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;

    let mut head = header(&mut scene, "optchat", "one card, one request")?;
    head.show(&mut scene, seconds(0.18));
    let mut status = chip(&mut scene, "chip", Tone::Error, "before")?;
    status.show(&mut scene, seconds(0.35));

    stage.settle_in(&mut scene, "session", seconds(0.15));
    stage.orb_in(&mut scene, "model", seconds(0.2), OrbEntrance::HERO);

    let session_at = said.at("what you have today");
    stage.settle_in(
        &mut scene,
        "request",
        session_at.saturating_sub(seconds(0.25)),
    );
    stage.connect(&mut scene, "ask", session_at, 0.5);
    sfx::TICK.play(&mut scene, "connect-ask", session_at, -20.0);

    let mut first = footer(
        &mut scene,
        "footer-a",
        vec![
            span("one card = one request · ", Tone::Muted),
            span("the whole history, every turn", Tone::Warning),
        ],
    )?;
    first.type_in(&mut scene, session_at, TYPE, 0.8);

    // The context carried climbs to 1.2M tokens.
    let mut tokens = RollingNumberActor::declare(
        &mut scene,
        "tokens",
        RollingNumberPlan::new([960.0, 180.0], 54.0, "0.0")
            .aligned(CaptionAlign::Center)
            .tone(Tone::Accent)
            .prefix(vec![span("context ", Tone::Muted)])
            .suffix(vec![span("M tokens", Tone::Muted)])
            .chip(),
    )?;
    let thousand = said.at("a thousand messages");
    tokens.show(&mut scene, thousand.saturating_sub(seconds(0.4)));
    tokens.roll(&mut scene, thousand, "0.3")?;
    tokens.roll(&mut scene, said.at("every turn sends"), "0.7")?;
    tokens.roll(&mut scene, said.at("whole history again"), "1.2")?;

    let again = said.at("whole history again");
    let hit = stage.send(&mut scene, "turn", again, 0.6);
    sfx::SEND.play(&mut scene, "turn-send", again, -9.0);
    sfx::IMPACT.play(&mut scene, "turn-hit", hit, -11.0);
    stage.jolt(&mut scene, hit, [1100.0 * 0.35, 0.0], 0.55);
    stage.land(&mut scene, "model", hit);
    stage.hit(&mut scene, "model.pulse", hit, 0.5, 0.0);

    // A meter fills; the window is reached.
    let mut meter = MeterActor::declare(
        &mut scene,
        "context",
        &MeterPlan::bar(
            [960.0, 930.0],
            1200.0,
            AxisPlan::new([0.0, 1_200_000.0]).nice(4),
        )
        .label("context")
        .readout(ReadoutFormat::new(0))
        .threshold(900_000.0, Tone::Warning)
        .threshold(1_100_000.0, Tone::Error),
        0.0,
    )?;
    let fills = said.at("the window fills");
    meter.show(&mut scene, fills.saturating_sub(seconds(0.4)));
    meter.sweep(&mut scene, fills, 950_000.0, 1.8);
    first.hide(&mut scene, fills);

    // The cache re-reads everything; the answer is worse.
    let rereads = said.at("the provider re-reads");
    stage.connect(
        &mut scene,
        "read",
        rereads.saturating_sub(seconds(0.4)),
        0.4,
    );
    let reread = stage.send(
        &mut scene,
        "reread",
        rereads.saturating_sub(seconds(0.3)),
        0.45,
    );
    stage.hit(&mut scene, "cache.flash", reread, 0.45, 0.0);
    stage.land(&mut scene, "model", reread);
    meter.set(&mut scene, rereads, 1_200_000.0);
    meter.flash(&mut scene, rereads, 0.8);
    sfx::FAILURE.play(&mut scene, "cache-reread", reread, -9.0);

    let mut second = footer(
        &mut scene,
        "footer-b",
        vec![
            span("the cache re-reads the whole history — ", Tone::Plain),
            span("the answer is worse", Tone::Error),
        ],
    )?;
    second.type_in(&mut scene, rereads, TYPE, 0.8);

    // The honest label: the log it would have carried is an upper bound.
    let decay = said.at("answers decay");
    second.hide(&mut scene, decay.saturating_sub(seconds(0.2)));
    let mut bound = footer(
        &mut scene,
        "footer-bound",
        vec![
            span("real ~1,000-message run · ", Tone::Muted),
            span("full log is an upper bound", Tone::Warning),
            span(" — past the window OpenCode compacts", Tone::Muted),
        ],
    )?;
    bound.type_in(&mut scene, decay, TYPE_LONG, 0.4);

    scene.finish().context("optchat-before")
}

// ---------------------------------------------------------------------------
// 3. after — the same space, replayed: a log that merges into a tree
// ---------------------------------------------------------------------------

fn after(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(LEAD, [("after", GAP)])?;
    let mut scene = PlanBuilder::new("optchat-after", reading.duration());
    let [said] = reading.place(&mut scene);

    let plan = StagePlan {
        post: StagePost::RESTRAINED,
        elements: vec![
            StageElement::card("log", [300.0, 470.0, 0.0], [280.0, 120.0], "log")
                .statuses(&[
                    ("every line, verbatim", Tone::Plain),
                    ("never edited", Tone::Success),
                ])
                .tone(Tone::Success),
            StageElement::card("m1", [640.0, 780.0, -10.0], [120.0, 64.0], "msg 1")
                .tone(Tone::Muted),
            StageElement::card("m2", [790.0, 780.0, -10.0], [120.0, 64.0], "msg 2")
                .tone(Tone::Muted),
            StageElement::card("m3", [940.0, 780.0, -10.0], [120.0, 64.0], "msg 3")
                .tone(Tone::Muted),
            StageElement::card("m4", [1090.0, 780.0, -10.0], [120.0, 64.0], "msg 4")
                .tone(Tone::Muted),
            StageElement::card("s12", [715.0, 600.0, 0.0], [160.0, 72.0], "sum 1-2")
                .tone(Tone::Accent),
            StageElement::card("s34", [1015.0, 600.0, 0.0], [160.0, 72.0], "sum 3-4")
                .tone(Tone::Accent),
            StageElement::card("root", [865.0, 420.0, 0.0], [200.0, 78.0], "sum 1-4")
                .tone(Tone::Accent),
            StageElement::card(
                "view",
                [900.0, 940.0, 0.0],
                [1080.0, 86.0],
                "view · fixed size",
            )
            .statuses(&[("recent whole · older summarized", Tone::Muted)])
            .tone(Tone::Success),
            StageElement::beam("stream", "log", "m1").tone(Tone::Success),
            StageElement::beam("p1", "m1", "s12"),
            StageElement::beam("p2", "m2", "s12"),
            StageElement::beam("p3", "m3", "s34"),
            StageElement::beam("p4", "m4", "s34"),
            StageElement::beam("up1", "s12", "root").tone(Tone::Accent),
            StageElement::beam("up2", "s34", "root").tone(Tone::Accent),
            StageElement::packet("l1", "stream")
                .labeled("msg")
                .tone(Tone::Plain),
            StageElement::packet("l2", "stream")
                .labeled("msg")
                .tone(Tone::Plain),
            StageElement::packet("l3", "stream")
                .labeled("msg")
                .tone(Tone::Plain),
        ],
    };
    let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;

    let mut head = header(&mut scene, "optchat", "the replay, same space")?;
    head.show(&mut scene, seconds(0.18));
    let mut status = chip(&mut scene, "chip", Tone::Success, "after the plugin")?;
    status.show(&mut scene, seconds(0.35));

    // The log is kept: a beam of packets streams in.
    let keeps = said.at("keeps the log");
    stage.settle_in(&mut scene, "log", keeps.saturating_sub(seconds(0.3)));
    let streamed = stage.connect(&mut scene, "stream", keeps, 0.5);
    sfx::TICK.play(&mut scene, "connect-stream", keeps, -20.0);
    let l1 = stage.send(&mut scene, "l1", streamed, 0.5);
    stage.send(&mut scene, "l2", l1 + seconds(0.35), 0.5);
    stage.send(&mut scene, "l3", l1 + seconds(0.7), 0.5);

    let mut first = footer(
        &mut scene,
        "footer-a",
        vec![
            span("the log is kept ", Tone::Plain),
            span("exactly as written", Tone::Success),
        ],
    )?;
    first.type_in(&mut scene, keeps, TYPE, 0.7);

    // The messages land, then pairs merge into a tree.
    let cheap = said.at("a cheap model writes");
    stage.settle_in(&mut scene, "m1", cheap);
    stage.settle_in(&mut scene, "m2", cheap + seconds(0.12));
    stage.settle_in(&mut scene, "m3", cheap + seconds(0.24));
    stage.settle_in(&mut scene, "m4", cheap + seconds(0.36));

    let merge = said.at("merge into a tree");
    first.hide(&mut scene, merge);
    let b12 = stage.connect(&mut scene, "p1", merge.saturating_sub(seconds(0.5)), 0.4);
    stage.connect(&mut scene, "p2", merge.saturating_sub(seconds(0.5)), 0.4);
    stage.connect(&mut scene, "p3", merge.saturating_sub(seconds(0.35)), 0.4);
    stage.connect(&mut scene, "p4", merge.saturating_sub(seconds(0.35)), 0.4);
    let s12 = stage.settle_in(&mut scene, "s12", merge);
    stage.settle_in(&mut scene, "s34", merge + seconds(0.1));
    stage.land(&mut scene, "s12", s12);
    sfx::TICK.play(&mut scene, "connect-merge", b12, -18.0);
    stage.connect(&mut scene, "up1", merge + seconds(1.0), 0.45);
    stage.connect(&mut scene, "up2", merge + seconds(1.0), 0.45);
    let root = stage.settle_in(&mut scene, "root", merge + seconds(1.5));
    stage.land(&mut scene, "root", root);
    stage.hit(&mut scene, "root.flash", root, 0.5, 0.0);
    sfx::BLOOM.play(&mut scene, "merge-bloom", root, -12.0);

    // A fixed-size view at the bottom that stops growing.
    let view = said.at("view of a fixed size");
    stage.settle_in(&mut scene, "view", view);
    stage.connect(
        &mut scene,
        "stream",
        view.saturating_sub(seconds(0.4)),
        0.35,
    );
    let mut second = footer(
        &mut scene,
        "footer-b",
        vec![
            span("every turn sends ", Tone::Plain),
            span("a view of a fixed size", Tone::Success),
        ],
    )?;
    second.type_in(&mut scene, view, TYPE, 0.8);

    // The whole log against the view: 1,200k to 107k is ×11 smaller.
    let mut tokens = RollingNumberActor::declare(
        &mut scene,
        "tokens",
        RollingNumberPlan::new([250.0, 170.0], 52.0, "1200")
            .tone(Tone::Error)
            .prefix(vec![span("whole log ", Tone::Muted)])
            .suffix(vec![span("k tokens", Tone::Muted)])
            .chip(),
    )?;
    let older = said.at("older ones many per line");
    tokens.show(&mut scene, view);
    tokens.roll(&mut scene, older, "1200")?;

    let constant = said.at("constant size");
    let mut smaller = RollingNumberActor::declare(
        &mut scene,
        "smaller",
        RollingNumberPlan::new([1180.0, 170.0], 52.0, "11")
            .tone(Tone::Success)
            .prefix(vec![span("×", Tone::Muted)])
            .suffix(vec![span(" smaller", Tone::Muted)])
            .chip(),
    )?;
    smaller.show(&mut scene, constant);
    sfx::BLOOM.play(&mut scene, "view-bloom", constant, -14.0);

    // The honest label again: the full log is what it would have carried.
    let infinite = said.at("infinite context");
    second.hide(&mut scene, infinite.saturating_sub(seconds(0.2)));
    let mut bound = footer(
        &mut scene,
        "footer-bound",
        vec![
            span("real ~1,000-message run · ", Tone::Muted),
            span("full log is an upper bound", Tone::Warning),
            span(" — past the window OpenCode compacts", Tone::Muted),
        ],
    )?;
    bound.type_in(&mut scene, infinite, TYPE_LONG, 0.4);

    scene.finish().context("optchat-after")
}

// ---------------------------------------------------------------------------
// 4. zoom — a vague line opens down to the original message, then the code
// ---------------------------------------------------------------------------

fn zoom(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(LEAD, [("zoom", GAP)])?;
    let mut scene = PlanBuilder::new("optchat-zoom", reading.duration());
    let [said] = reading.place(&mut scene);

    // A terminal walks the zoom: a vague line opens into the two it was made
    // from, down to the original message.
    let mut zoomer = TerminalActor::declare(
        &mut scene,
        "zoom-term",
        TerminalPlan::new([150.0, 230.0], 1260.0, 11)
            .size(22.0)
            .titled("opencode · zoom")
            .prompt(vec![span("~/dev ❯ ", Tone::Accent)]),
    )?;

    let mut head = header(&mut scene, "optchat", "zoom · the whole log stays")?;
    head.show(&mut scene, seconds(0.18));
    let mut status = chip(&mut scene, "chip", Tone::Accent, "zoom")?;
    status.show(&mut scene, seconds(0.3));

    // A vague line opens into the two lines it was made from, down to the message.
    let vague = said.at("when a line is too vague");
    let opens = said.at("the line opens");
    let any_fact = said.at("any fact in your history");
    let two_tools = said.at("two tools");
    let and_date = said.at("and date");

    zoomer.show(&mut scene, seconds(0.5));
    zoomer.type_command(&mut scene, vague.saturating_sub(seconds(0.5)), "zoom 64 8")?;
    zoomer.print(
        &mut scene,
        opens,
        [
            vec![span(
                "line 64       a vague summary · covers 8 messages",
                Tone::Warning,
            )],
            vec![
                span("  ├─ line 32   ", Tone::Accent),
                span("covers 4   · the two it was made from", Tone::Muted),
            ],
            vec![span("  └─ line 48   covers 4", Tone::Accent)],
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
        "zoom 64 1",
    )?;
    zoomer.print(
        &mut scene,
        original,
        [vec![
            span("\"explain how the engine loop works\"", Tone::Success),
            span("   the original, word for word", Tone::Muted),
        ]],
    )?;
    sfx::BLOOM.play(&mut scene, "orig", original, -14.0);

    // The real code: the hook that rebuilds the context, and the zoom tool.
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
            span("the hook that rebuilds the context, and zoom", Tone::Plain),
        ],
    )?;
    caption.type_in(&mut scene, any_fact, TYPE, 0.7);

    let mut tools = footer(
        &mut scene,
        "footer-tools",
        vec![
            span("two tools are enough: ", Tone::Plain),
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
// 5. plugin — what it is: one directory, two sides, no configuration
// ---------------------------------------------------------------------------

fn plugin(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(LEAD, [("plugin", GAP)])?;
    let mut scene = PlanBuilder::new("optchat-plugin", reading.duration());
    let [said] = reading.place(&mut scene);

    let plan = StagePlan {
        post: StagePost::RESTRAINED,
        elements: vec![
            StageElement::card("dir", [960.0, 250.0, 0.0], [520.0, 116.0], "one directory")
                .statuses(&[
                    ("opencode v2 plugin", Tone::Muted),
                    ("works on 2.0.22 · 2.0.23", Tone::Muted),
                ])
                .tone(Tone::Accent),
            StageElement::card("server", [620.0, 620.0, 0.0], [500.0, 176.0], "server side")
                .statuses(&[
                    ("rebuilds the context of every turn", Tone::Plain),
                    ("answers compaction with the view", Tone::Success),
                ])
                .tone(Tone::Success),
            StageElement::card("cli", [1320.0, 620.0, 0.0], [440.0, 150.0], "cli side")
                .statuses(&[("the /optchat pop-up", Tone::Plain)])
                .tone(Tone::Request),
            StageElement::beam("to-server", "dir", "server").tone(Tone::Success),
            StageElement::beam("to-cli", "dir", "cli").tone(Tone::Request),
            StageElement::packet("turn", "to-server")
                .labeled("turn")
                .tone(Tone::Success),
        ],
    };
    let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;

    let mut head = header(&mut scene, "optchat", "the plugin")?;
    head.show(&mut scene, seconds(0.18));
    let mut status = chip(&mut scene, "chip", Tone::Accent, "the plugin")?;
    status.show(&mut scene, seconds(0.3));

    let directory = said.at("one directory");
    stage.settle_in(&mut scene, "dir", directory);
    let mut one = footer(
        &mut scene,
        "footer-one",
        vec![
            span("the plugin is ", Tone::Plain),
            span("one directory", Tone::Accent),
            span(" — an opencode v2 plugin", Tone::Muted),
        ],
    )?;
    one.type_in(&mut scene, directory, TYPE, 0.7);

    // Server side: rebuilds the context, answers the harness's compaction request.
    let server = said.at("the server side");
    stage.connect(&mut scene, "to-server", server, 0.5);
    stage.settle_in(&mut scene, "server", server + seconds(0.3));
    sfx::TICK.play(&mut scene, "to-server", server, -18.0);
    let mut side = footer(
        &mut scene,
        "footer-server",
        vec![
            span("server side: ", Tone::Muted),
            span("it rebuilds the context of every turn", Tone::Plain),
        ],
    )?;
    side.type_in(&mut scene, server, TYPE, 0.7);
    one.hide(&mut scene, server.saturating_sub(seconds(0.2)));

    let every = said.at("every turn");
    let turn = stage.send(&mut scene, "turn", every, 0.6);
    stage.land(&mut scene, "server", turn);

    // "so compaction never runs".
    let never = said.at("compaction never runs");
    stage.hit(&mut scene, "server.flash", never, 0.5, 0.0);
    side.hide(&mut scene, never.saturating_sub(seconds(0.2)));
    let mut answer = footer(
        &mut scene,
        "footer-answer",
        vec![
            span(
                "it answers the harness's compaction request with the view — ",
                Tone::Plain,
            ),
            span("compaction never runs", Tone::Success),
        ],
    )?;
    answer.type_in(&mut scene, never, TYPE, 0.7);

    // CLI side: the pop-up.
    let cli = said.at("the command line");
    stage.connect(&mut scene, "to-cli", cli, 0.5);
    stage.settle_in(&mut scene, "cli", cli + seconds(0.3));
    sfx::TICK.play(&mut scene, "to-cli", cli, -18.0);

    let popup = said.at("adds a pop-up");
    answer.hide(&mut scene, popup.saturating_sub(seconds(0.2)));
    let mut cli_note = footer(
        &mut scene,
        "footer-cli",
        vec![
            span("cli side: ", Tone::Muted),
            span("a pop-up in the terminal", Tone::Request),
        ],
    )?;
    cli_note.type_in(&mut scene, cli, TYPE, 0.7);

    // "No configuration, no dependency, MIT."
    let config = said.at("no configuration");
    cli_note.hide(&mut scene, config.saturating_sub(seconds(0.2)));
    let mut free = footer(
        &mut scene,
        "footer-mit",
        vec![
            span("no configuration · no dependency · ", Tone::Muted),
            span("MIT", Tone::Success),
        ],
    )?;
    free.type_in(&mut scene, config, TYPE, 0.8);
    sfx::CONFIRM.play(&mut scene, "mit", said.at("MIT"), -14.0);

    scene.finish().context("optchat-plugin")
}

// ---------------------------------------------------------------------------
// 6. tui — the pop-up: Stats, View, Summaries, Settings
// ---------------------------------------------------------------------------

fn tui(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(LEAD, [("tui", GAP)])?;
    let mut scene = PlanBuilder::new("optchat-tui", reading.duration());
    let [said] = reading.place(&mut scene);

    let mut head = header(&mut scene, "optchat", "the pop-up")?;
    head.show(&mut scene, seconds(0.18));
    let mut status = chip(&mut scene, "chip", Tone::Accent, "the pop-up")?;
    status.show(&mut scene, seconds(0.3));

    // ---- Stats: what a turn carries, and what it costs --------------------
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

    let screens = said.at("the screens");
    stats.print(
        &mut scene,
        screens,
        [vec![
            span("optchat  ", Tone::Accent),
            span("─  Stats · View · Summaries · Settings", Tone::Muted),
        ]],
    )?;
    let statistics = said.at("stats");
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
            vec![span("compression ×11.4", Tone::Success)],
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

    let since = said.at("since a request");
    bars.grow(&mut scene, since, "optchat", &[("cost", 0.16)])?;
    opt.show(&mut scene, since);
    opt.roll(&mut scene, since, "0.16")?;
    sfx::SUCCESS.play(&mut scene, "cost-optchat", since, -14.0);

    let whole = said.at("for the whole log");
    bars.grow(&mut scene, whole, "full", &[("cost", 0.86)])?;
    full.show(&mut scene, whole);
    full.roll(&mut scene, whole, "0.86")?;
    sfx::MARK.play(&mut scene, "cost-full", whole, -16.0);

    let mut bill = footer(
        &mut scene,
        "footer-bill",
        vec![
            span("0.16 ¢ a request against 0.86 ¢ ", Tone::Plain),
            span("carrying the whole log", Tone::Muted),
        ],
    )?;
    bill.type_in(&mut scene, since, TYPE, 0.8);

    // ---- View: the context line by line, originals bright ----------------
    let view = said.at("view");
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

    let lines = said.at("the context line by line");
    show_view.print(
        &mut scene,
        lines,
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

    // ---- Settings: five knobs --------------------------------------------
    let knobs = said.at("knobs");
    let clear = knobs.saturating_sub(seconds(0.25));
    summaries.hide(&mut scene, clear);

    let mut settings = TerminalActor::declare(
        &mut scene,
        "term-settings",
        TerminalPlan::new([150.0, 250.0], 1240.0, 12)
            .size(20.0)
            .titled("optchat · settings")
            .prompt(vec![span("~/dev ❯ ", Tone::Accent)]),
    )?;
    settings.show(&mut scene, knobs.saturating_sub(seconds(0.15)));
    settings.print(
        &mut scene,
        knobs,
        [
            vec![span("five knobs", Tone::Muted)],
            vec![span("memory             on", Tone::Success)],
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
// 7. outro — one line to install, plain files, and the credits
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

    let files = said.at("plain files");
    term.print(
        &mut scene,
        entered + seconds(0.2),
        [vec![span("Cloning into 'optchat'... done.", Tone::Success)]],
    )?;
    term.print(
        &mut scene,
        files,
        [
            vec![span(
                "main/2026-10-05.jsonl   every message, verbatim",
                Tone::Plain,
            )],
            vec![span(
                "tree/2026-10-05.jsonl   one summary per node",
                Tone::Plain,
            )],
            vec![span(
                "zoom(id, n)             reads any line back",
                Tone::Accent,
            )],
        ],
    )?;

    // The closing line: you never compact again.
    let memory = said.at("the chat is the memory");
    let mut line_a = CaptionActor::declare(
        &mut scene,
        "closing-memory",
        &CaptionPlan::line(
            [960.0, 660.0],
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
            [960.0, 740.0],
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
