//! Visualization components showroom: a CI checklist that runs, fails,
//! retries, and celebrates; a countdown ring, a gauge, and a progress bar;
//! a before/after benchmark that grows, compares, and re-sorts; and
//! word-timed subtitles over a narrated Stage clip.
use std::path::{Path, PathBuf};

use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, seconds},
    axis::AxisPlan,
    bars::{BarDeltaPlan, BarSeriesPlan, BarsActor, BarsPlan, SortOrder},
    caption::{CaptionActor, CaptionAlign, CaptionPlan, CaptionSpanPlan},
    checklist::{ChecklistActor, ChecklistItemPlan, ChecklistPlan, Outcome},
    confetti::{ConfettiActor, ConfettiPlan},
    meter::{MeterActor, MeterPlan},
    narration::Narration,
    plan::{ReelPlan, ScenePlan},
    readout::ReadoutFormat,
    stage::{StageActor, StagePlan},
    subtitles::{SubtitlesActor, SubtitlesPlan},
    tone::Tone,
};

/// The narration files the subtitles segment plays, copied beside the reel.
pub const NARRATION_FILES: [&str; 1] = ["layer.mp3"];

/// The 2password narration, voiced with Whisper word timings.
pub fn narration_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../2password/narration")
}

pub fn build_reel(narration: &Path) -> Result<ReelPlan> {
    ReelPlan::dipped(
        "viz-components",
        vec![checklist()?, meters()?, bars()?, subtitles(narration)?],
        seconds(0.6),
    )
}

fn span(text: &str, tone: Tone) -> CaptionSpanPlan {
    CaptionSpanPlan::new(text, tone)
}

fn header(scene: &mut PlanBuilder, spans: Vec<CaptionSpanPlan>, at: u64) -> Result<CaptionActor> {
    let mut caption = CaptionActor::declare(
        scene,
        "header",
        &CaptionPlan::line([960.0, 150.0], 30.0, spans).aligned(CaptionAlign::Center),
    )?;
    caption.show(scene, at);
    Ok(caption)
}

/// CI checks: install, then typecheck and unit tests in parallel while lint
/// is skipped; unit tests fail, retry, and pass; end-to-end passes; confetti.
pub fn checklist() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("checklist", seconds(13.5));
    header(
        &mut scene,
        vec![
            span("CI", Tone::Accent),
            span("  ·  pull request #50231", Tone::Muted),
        ],
        seconds(0.2),
    )?;
    let plan = ChecklistPlan::new([580.0, 300.0], 760.0)
        .size(30.0)
        .title("checks")
        .rail()
        .panel()
        .item(ChecklistItemPlan::new("install", "install dependencies").result("1.8s"))
        .item(ChecklistItemPlan::new("typecheck", "typecheck").result("4.2s"))
        .item(ChecklistItemPlan::new("lint", "lint").result("no changes"))
        .item(
            ChecklistItemPlan::new("unit", "unit tests")
                .result("812 passed")
                .failure("2 failed"),
        )
        .item(ChecklistItemPlan::new("e2e", "end-to-end").result("38s"));
    let mut checks = ChecklistActor::declare(&mut scene, "checks", &plan)?;
    checks.show(&mut scene, seconds(0.3));
    checks.reveal(&mut scene, seconds(0.5))?;
    checks.start(&mut scene, "install", seconds(1.3))?;
    let installed = checks.resolve(&mut scene, "install", seconds(2.4), Outcome::Done)?;
    checks.start(&mut scene, "typecheck", installed)?;
    checks.start(&mut scene, "unit", installed + seconds(0.12))?;
    checks.skip(&mut scene, "lint", installed + seconds(0.4))?;
    checks.resolve(
        &mut scene,
        "typecheck",
        installed + seconds(1.6),
        Outcome::Done,
    )?;
    let failed = checks.resolve(
        &mut scene,
        "unit",
        installed + seconds(2.1),
        Outcome::Failed,
    )?;
    let retry = failed + seconds(0.7);
    checks.start(&mut scene, "unit", retry)?;
    checks.start(&mut scene, "e2e", retry + seconds(0.2))?;
    let passed = checks.resolve(&mut scene, "unit", retry + seconds(1.5), Outcome::Done)?;
    let green = checks.resolve(&mut scene, "e2e", passed + seconds(0.5), Outcome::Done)?;
    let mut confetti = ConfettiActor::declare(
        &mut scene,
        "confetti",
        &ConfettiPlan::new([960.0, 690.0]).seed(5).count(160),
    )?;
    confetti.burst(&mut scene, green);
    scene.cue("checklist", 0, seconds(13.5));
    Ok(scene.finish()?)
}

/// A countdown ring sweeping closed, a gauge springing between readings
/// through its thresholds, and an upload bar.
pub fn meters() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("meters", seconds(11.0));
    header(&mut scene, vec![span("meters", Tone::Accent)], seconds(0.2))?;
    let countdown = MeterPlan::countdown([640.0, 500.0], 170.0, 8.0).label("approval expires");
    let mut timer = MeterActor::declare(&mut scene, "timer", &countdown, 8.0)?;
    timer.show(&mut scene, seconds(0.3));
    timer.countdown(&mut scene, seconds(1.4), 8.0);

    let gauge = MeterPlan::ring(
        [1280.0, 500.0],
        170.0,
        AxisPlan::new([0.0, 100.0]).every(10.0),
    )
    .label("cpu")
    .readout(ReadoutFormat::new(0).unit("%"))
    .threshold(70.0, Tone::Warning)
    .threshold(90.0, Tone::Error);
    let mut cpu = MeterActor::declare(&mut scene, "cpu", &gauge, 0.0)?;
    cpu.show(&mut scene, seconds(0.45));
    cpu.set(&mut scene, seconds(1.2), 38.0);
    cpu.set(&mut scene, seconds(3.2), 74.0);
    cpu.set(&mut scene, seconds(5.0), 96.0);
    cpu.set(&mut scene, seconds(7.4), 41.0);

    let bar = MeterPlan::bar(
        [960.0, 900.0],
        1240.0,
        AxisPlan::new([0.0, 100.0]).every(25.0).unit("%"),
    )
    .label("upload")
    .readout(ReadoutFormat::new(1).unit("%"))
    .tone(Tone::Request)
    .threshold(100.0, Tone::Success)
    .tick_labels();
    let mut upload = MeterActor::declare(&mut scene, "upload", &bar, 0.0)?;
    upload.show(&mut scene, seconds(0.6));
    let stalled = upload.sweep(&mut scene, seconds(1.6), 36.0, 1.8);
    let resumed = stalled + seconds(1.1);
    upload.sweep(&mut scene, resumed, 100.0, 4.2);
    scene.cue("meters", 0, seconds(11.0));
    Ok(scene.finish()?)
}

/// Startup timings before and after a change: bars grow, deltas pop in,
/// and the rows race into order.
pub fn bars() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("bars", seconds(11.0));
    header(
        &mut scene,
        vec![
            span("benchmark", Tone::Accent),
            span("  ·  before and after the cache", Tone::Muted),
        ],
        seconds(0.2),
    )?;
    let rows = [
        ("cold", "cold start", 1840.0, 1214.0),
        ("paint", "first paint", 960.0, 420.0),
        ("build", "build", 1420.0, 1530.0),
        ("tests", "test suite", 1680.0, 610.0),
    ];
    let plan = rows.iter().fold(
        BarsPlan::new(
            [520.0, 330.0],
            900.0,
            AxisPlan::new([0.0, 2000.0])
                .every(500.0)
                .label("milliseconds"),
        )
        .row_height(104.0)
        .size(26.0)
        .series(BarSeriesPlan::new("before", "before", Tone::Muted))
        .series(BarSeriesPlan::new("after", "after", Tone::Accent))
        .readout(ReadoutFormat::new(0).grouped().unit("ms"))
        .delta(BarDeltaPlan::new("before", "after")),
        |plan, (id, label, ..)| plan.row(*id, *label),
    );
    let mut bench = BarsActor::declare(&mut scene, "bench", &plan)?;
    bench.show(&mut scene, seconds(0.3));
    let before = rows
        .iter()
        .map(|(id, _, before, _)| (*id, *before))
        .collect::<Vec<_>>();
    let after = rows
        .iter()
        .map(|(id, _, _, after)| (*id, *after))
        .collect::<Vec<_>>();
    bench.grow(&mut scene, seconds(1.0), "before", &before)?;
    bench.grow(&mut scene, seconds(2.8), "after", &after)?;
    bench.reveal_deltas(&mut scene, seconds(4.6))?;
    bench.sort(&mut scene, seconds(6.6), "after", SortOrder::Ascending)?;
    scene.cue("bars", 0, seconds(11.0));
    Ok(scene.finish()?)
}

/// The 2password "layer" clip over a small Stage, with word-timed subtitles
/// and a success burst on "One approval!".
pub fn subtitles(narration: &Path) -> Result<ScenePlan> {
    let narration = Narration::load(narration)?;
    let clip = narration.clip("layer")?;
    let start = seconds(0.4);
    let mut scene = PlanBuilder::new("subtitles", start + clip.duration() + seconds(1.0));
    let spoken = clip.place(&mut scene, start);
    let stage: StagePlan = serde_json::from_value(serde_json::json!({
        "elements": [
            { "kind": "card", "id": "agent", "at": [420, 470, 0], "size": [300, 110], "title": "agent" },
            { "kind": "orb", "id": "layer", "at": [960, 470, 0], "radius": 120 },
            { "kind": "card", "id": "vault", "at": [1500, 470, 0], "size": [300, 110], "title": "1Password" },
            { "kind": "beam", "id": "ask", "from": "agent", "to": "layer" },
            { "kind": "beam", "id": "once", "from": "layer", "to": "vault" },
            { "kind": "packet", "id": "request", "beam": "ask", "label": "op://…" },
            { "kind": "packet", "id": "unlock", "beam": "once", "label": "once" }
        ]
    }))?;
    let mut film = StageActor::declare(&mut scene, "stage", &stage)?;
    film.settle_in(&mut scene, "agent", spoken.at("tiny layer"));
    film.settle_in(&mut scene, "vault", spoken.at("1Password command"));
    let wired = film.connect(&mut scene, "ask", spoken.at("your agent"), 0.5);
    let asked = film.send(
        &mut scene,
        "request",
        wired.max(spoken.at("asks 2Password")),
        0.8,
    );
    film.land(&mut scene, "layer", asked);
    let linked = film.connect(&mut scene, "once", spoken.at("and 2Password"), 0.5);
    let unlocked = film.send(
        &mut scene,
        "unlock",
        linked.max(spoken.at("asks 1Password")),
        0.8,
    );
    film.land(&mut scene, "vault", unlocked);
    let mut confetti = ConfettiActor::declare(
        &mut scene,
        "confetti",
        &ConfettiPlan::new([1500.0, 420.0])
            .seed(11)
            .count(120)
            .speed(1300.0),
    )?;
    confetti.burst(&mut scene, spoken.at("one approval"));
    SubtitlesActor::declare(
        &mut scene,
        "subtitles",
        &SubtitlesPlan::from_spoken(&spoken, [960.0, 900.0], 1300.0).size(44.0),
    )?;
    scene.cue("subtitles", 0, spoken.end());
    Ok(scene.finish()?)
}
