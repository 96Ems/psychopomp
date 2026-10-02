//! Narrated explainers for two OpenCode config pull requests.
//!
//! #51889 removes the duplicate V1 config migration; #51901 renames legacy
//! provider IDs in the top-level model. Each reel plays the broken behavior as a
//! sequence diagram, replays the fix in the same slots, then animates the diff.
//! Every visual moment is keyed to a phrase in the narration.
mod diff;
mod narration;

use std::path::Path;

use anyhow::{Context, Result};
use diff::{Diff, add, keep, remove};
use kinograph::{
    author::{PlanBuilder, seconds},
    caption::{CaptionActor, CaptionAlign, CaptionPlan, CaptionSpanPlan},
    plan::{ReelPlan, ReelSegmentPlan, ReelTransitionStyle, ScenePlan},
    sequence::{SequenceActor, SequenceParticipantPlan, SequencePlan, SequenceRowPlan},
    tone::Tone,
};
use narration::Narration;

const TRANSITION: u64 = 700_000_000;
const LEFT: f32 = 140.0;
const RIGHT: f32 = 1780.0;
const HEADER_Y: f32 = 96.0;
const FOOTER_Y: f32 = 1004.0;

struct Pr {
    number: &'static str,
    title: &'static str,
    slug: &'static str,
}

// ---------------------------------------------------------------------------
// Shared pieces
// ---------------------------------------------------------------------------

fn span(text: &str, tone: Tone) -> CaptionSpanPlan {
    CaptionSpanPlan::new(text, tone)
}

/// `#50784  keep the real startup error`, top left.
fn header(scene: &mut PlanBuilder, pr: &Pr, type_at: Option<u64>) -> Result<()> {
    let plan = CaptionPlan::line(
        [LEFT, HEADER_Y],
        30.0,
        vec![
            span(pr.number, Tone::Accent),
            span("  ", Tone::Plain),
            span(pr.title, Tone::Plain),
        ],
    );
    let mut caption = CaptionActor::declare(scene, "header", &plan)?;
    if let Some(at) = type_at {
        caption.type_in(scene, at, 55.0, 0.6);
    }
    Ok(())
}

/// A status chip, top right: `● before`.
fn chip(scene: &mut PlanBuilder, id: &str, dot: Tone, text: &str) -> Result<CaptionActor> {
    let plan = CaptionPlan::line(
        [RIGHT, HEADER_Y],
        22.0,
        vec![span("● ", dot), span(text, Tone::Plain)],
    )
    .aligned(CaptionAlign::Right)
    .chip();
    CaptionActor::declare(scene, id, &plan)
}

fn footer(scene: &mut PlanBuilder, id: &str, spans: Vec<CaptionSpanPlan>) -> Result<CaptionActor> {
    CaptionActor::declare(scene, id, &CaptionPlan::line([LEFT, FOOTER_Y], 28.0, spans))
}

fn participant(id: &str, label: &str, detail: &str) -> SequenceParticipantPlan {
    SequenceParticipantPlan {
        id: id.to_owned(),
        label: label.to_owned(),
        detail: detail.to_owned(),
    }
}

fn message(id: &str, from: &str, to: &str, label: &str, tone: Tone) -> SequenceRowPlan {
    SequenceRowPlan::Message {
        id: id.to_owned(),
        slot: None,
        from: from.to_owned(),
        to: to.to_owned(),
        label: label.to_owned(),
        tone,
        reply: false,
        aside: String::new(),
    }
}

fn reply(id: &str, from: &str, to: &str, label: &str, tone: Tone) -> SequenceRowPlan {
    match message(id, from, to, label, tone) {
        SequenceRowPlan::Message {
            id,
            slot,
            from,
            to,
            label,
            tone,
            aside,
            ..
        } => SequenceRowPlan::Message {
            id,
            slot,
            from,
            to,
            label,
            tone,
            reply: true,
            aside,
        },
        row => row,
    }
}

fn note(id: &str, over: &[&str], text: &str, tone: Tone) -> SequenceRowPlan {
    SequenceRowPlan::Note {
        id: id.to_owned(),
        slot: None,
        over: over.iter().map(|p| (*p).to_owned()).collect(),
        text: text.to_owned(),
        tone,
        aside: String::new(),
    }
}

fn end(id: &str, participant: &str, label: &str, tone: Tone) -> SequenceRowPlan {
    SequenceRowPlan::End {
        id: id.to_owned(),
        slot: None,
        participant: participant.to_owned(),
        label: label.to_owned(),
        tone,
        aside: String::new(),
    }
}

trait RowExt {
    fn slot(self, slot: u32) -> Self;
    fn aside(self, text: &str) -> Self;
}

impl RowExt for SequenceRowPlan {
    fn slot(mut self, value: u32) -> Self {
        match &mut self {
            SequenceRowPlan::Message { slot, .. }
            | SequenceRowPlan::Note { slot, .. }
            | SequenceRowPlan::End { slot, .. } => *slot = Some(value),
        }
        self
    }

    fn aside(mut self, text: &str) -> Self {
        match &mut self {
            SequenceRowPlan::Message { aside, .. }
            | SequenceRowPlan::Note { aside, .. }
            | SequenceRowPlan::End { aside, .. } => *aside = text.to_owned(),
        }
        self
    }
}

/// A moment in a behavior segment: a phrase in the before or after clip.
#[derive(Clone, Copy)]
enum When {
    Before(&'static str),
    After(&'static str),
}

#[derive(Clone, Copy)]
struct At(When, f64);

fn before(phrase: &'static str) -> At {
    At(When::Before(phrase), 0.0)
}

fn after(phrase: &'static str) -> At {
    At(When::After(phrase), 0.0)
}

impl At {
    fn plus(self, seconds: f64) -> Self {
        At(self.0, self.1 + seconds)
    }
}

/// One PR's behavior story.
struct Flow {
    sequence: SequencePlan,
    /// Rows that only exist in the broken behavior; they fade when the fix replays.
    before_only: &'static [&'static str],
    reveals: Vec<(&'static str, At)>,
    strikes: Vec<(&'static str, At)>,
    /// Participant emphasis on and off.
    emphasis: Vec<(&'static str, At, At)>,
    /// Participants that start hidden: when they appear, and their opacity after the fix.
    late: Vec<(&'static str, At, f32)>,
    footer_before: (Vec<CaptionSpanPlan>, At),
    footer_after: (Vec<CaptionSpanPlan>, At),
}

// ---------------------------------------------------------------------------
// Segments
// ---------------------------------------------------------------------------

fn behavior(pr: &Pr, narration: &Narration, flow: Flow) -> Result<ScenePlan> {
    let before_clip = narration.clip(&format!("{}-before", pr.slug))?;
    let after_clip = narration.clip(&format!("{}-after", pr.slug))?;
    let lead = seconds(1.0);
    let gap = seconds(1.6);
    let duration = lead + before_clip.duration() + gap + after_clip.duration() + seconds(1.4);
    let mut scene = PlanBuilder::new(format!("{}-behavior", pr.slug), duration);
    let spoken_before = before_clip.place(&mut scene, lead);
    let spoken_after = after_clip.place(&mut scene, spoken_before.end() + gap);
    let switch = spoken_before.end() + seconds(0.4);
    let time = |at: At| -> u64 {
        let base = match at.0 {
            When::Before(phrase) => spoken_before.at(phrase),
            When::After(phrase) => spoken_after.at(phrase),
        };
        (base as i64 + (at.1 * 1e9) as i64).max(0) as u64
    };

    header(&mut scene, pr, Some(seconds(0.25)))?;
    let mut before_chip = chip(&mut scene, "chip-before", Tone::Error, "before")?;
    before_chip.show(&mut scene, seconds(0.5));
    before_chip.hide(&mut scene, switch);
    let mut after_chip = chip(&mut scene, "chip-after", Tone::Success, "after the fix")?;
    after_chip.show(&mut scene, switch + seconds(0.25));

    let mut sequence = SequenceActor::declare(&mut scene, "flow", &flow.sequence)?;
    sequence.animate(&mut scene, "opacity", 0.0, seconds(0.2), 1.0, 0.5);
    sequence.animate(&mut scene, "lifelines", 0.0, seconds(0.35), 1.0, 0.9);
    for (row, at) in &flow.reveals {
        sequence.reveal(&mut scene, row, time(*at));
    }
    for (row, at) in &flow.strikes {
        sequence.strike(&mut scene, row, time(*at));
        // The fix replays the same story: shared rows return unstruck.
        let strike = sequence.channel(&mut scene, &format!("row.{row}.strike"), 0.0);
        scene.spring(&strike, switch, 0.0, 0.4, 0.0);
    }
    for row in flow.before_only {
        sequence.fade(&mut scene, row, switch, 0.0);
    }
    for (participant, on, off) in &flow.emphasis {
        let emphasis = sequence.channel(
            &mut scene,
            &format!("participant.{participant}.emphasis"),
            0.0,
        );
        let (on, off) = (time(*on), time(*off));
        scene.spring(&emphasis, on, 1.0, 0.35, 0.0);
        // Emphasis from the broken story never outlives it.
        let off = if on < switch { off.min(switch) } else { off };
        scene.spring(&emphasis, off.max(on), 0.0, 0.45, 0.0);
    }
    for (participant, appear, fixed_opacity) in &flow.late {
        let opacity = sequence.channel(
            &mut scene,
            &format!("participant.{participant}.opacity"),
            0.0,
        );
        let appear = time(*appear);
        if appear < switch {
            // Introduced by the broken story; the fixed story sets its own presence.
            scene.spring(&opacity, appear, 1.0, 0.5, 0.0);
            scene.spring(&opacity, switch, *fixed_opacity, 0.45, 0.0);
        } else {
            scene.spring(&opacity, appear, *fixed_opacity, 0.5, 0.0);
        }
    }

    let mut footer_before = footer(&mut scene, "footer-before", flow.footer_before.0)?;
    footer_before.type_in(&mut scene, time(flow.footer_before.1), 42.0, 0.8);
    footer_before.hide(&mut scene, switch);
    let mut footer_after = footer(&mut scene, "footer-after", flow.footer_after.0)?;
    footer_after.type_in(&mut scene, time(flow.footer_after.1), 42.0, 0.8);
    scene
        .finish()
        .with_context(|| format!("{}-behavior", pr.slug))
}

/// `entrance`: the editor rises into place. Skip it when a zoom opens into the code.
fn code(
    pr: &Pr,
    narration: &Narration,
    (diff, steps, note): (Diff, Vec<&'static str>, &'static str),
    entrance: bool,
) -> Result<ScenePlan> {
    let clip = narration.clip(&format!("{}-code", pr.slug))?;
    let lead = seconds(0.9);
    let duration = lead + clip.duration() + seconds(1.6);
    let mut scene = PlanBuilder::new(format!("{}-code", pr.slug), duration);
    let spoken = clip.place(&mut scene, lead);
    header(&mut scene, pr, None)?;
    let mut change = chip(&mut scene, "chip-change", Tone::Accent, "the change")?;
    change.show(&mut scene, seconds(0.2));
    let times = steps
        .iter()
        .map(|phrase| spoken.at(phrase))
        .collect::<Vec<_>>();
    diff.declare(&mut scene, &times, seconds(0.9), entrance)?;
    let mut caption = footer(&mut scene, "footer", vec![span(note, Tone::Muted)])?;
    caption.show(&mut scene, seconds(0.6));
    scene.finish().with_context(|| format!("{}-code", pr.slug))
}

// ---------------------------------------------------------------------------
// Reels
// ---------------------------------------------------------------------------

const DUPLICATE: Pr = Pr {
    number: "#51889",
    title: "remove the duplicate v1 config migration",
    slug: "dup",
};

const RENAME: Pr = Pr {
    number: "#51901",
    title: "rename legacy providers in the top-level model",
    slug: "rename",
};

pub fn build_duplicate(narration_dir: &Path) -> Result<ReelPlan> {
    let narration = Narration::load(narration_dir)?;
    reel(
        "pr-51889",
        vec![
            behavior(&DUPLICATE, &narration, duplicate_flow())?,
            code(&DUPLICATE, &narration, duplicate_diff(), true)?,
        ],
    )
}

pub fn build_rename(narration_dir: &Path) -> Result<ReelPlan> {
    let narration = Narration::load(narration_dir)?;
    reel(
        "pr-51901",
        vec![
            behavior(&RENAME, &narration, rename_flow())?,
            code(&RENAME, &narration, rename_diff(), true)?,
        ],
    )
}

fn reel(id: &str, plans: Vec<ScenePlan>) -> Result<ReelPlan> {
    let reel = ReelPlan {
        version: ReelPlan::VERSION,
        id: id.to_owned(),
        segments: plans
            .into_iter()
            .enumerate()
            .map(|(index, plan)| ReelSegmentPlan {
                transition_nanos: if index == 0 { 0 } else { TRANSITION },
                transition_style: ReelTransitionStyle::Dip,
                transition_focus: None,
                plan,
            })
            .collect(),
    };
    reel.validate()?;
    Ok(reel)
}

// ---------------------------------------------------------------------------
// #51889: one converter instead of two
// ---------------------------------------------------------------------------

fn duplicate_flow() -> Flow {
    let sequence = SequencePlan {
        origin: [250.0, 168.0],
        width: 1480.0,
        row_height: 88.0,
        slots: Some(7),
        participants: vec![
            participant("config", "v1 config", "opencode.json"),
            participant("normalize", "normalize", "runs in production"),
            participant("migrate", "migrate", "whole-config copy"),
            participant("tests", "tests", "migration suite"),
        ],
        rows: vec![
            message(
                "load",
                "config",
                "normalize",
                "legacy fields",
                Tone::Request,
            ),
            note(
                "whole",
                &["migrate"],
                "converts a whole config",
                Tone::Muted,
            ),
            message(
                "borrow",
                "normalize",
                "migrate",
                "attachment · autoupdate · small_model",
                Tone::Warning,
            )
            .aside("3 calls"),
            note(
                "copy",
                &["normalize", "migrate"],
                "the rest duplicates normalize",
                Tone::Warning,
            ),
            note(
                "drift",
                &["normalize", "migrate"],
                "compaction.prune: unsupported  ≠  passed through",
                Tone::Error,
            ),
            message(
                "tests-old",
                "tests",
                "migrate",
                "most migration tests",
                Tone::Warning,
            ),
            message(
                "direct",
                "normalize",
                "normalize",
                "three fields, handled directly",
                Tone::Success,
            )
            .slot(1),
            end("gone", "migrate", "deleted", Tone::Muted).slot(2),
            note(
                "fixture",
                &["tests"],
                "v1 schema → random configs",
                Tone::Success,
            )
            .slot(3),
            message(
                "tests-new",
                "tests",
                "normalize",
                "every migration test",
                Tone::Success,
            )
            .slot(5),
        ],
    };
    Flow {
        sequence,
        before_only: &["whole", "borrow", "copy", "drift", "tests-old"],
        reveals: vec![
            ("load", before("field by field")),
            ("whole", before("whole config at once")),
            ("borrow", before("three single fields")),
            ("copy", before("second copy")),
            ("drift", before("drifted apart")),
            ("tests-old", before("nobody runs")),
            ("direct", after("those three fields directly")),
            ("gone", after("migrate is deleted")),
            ("fixture", after("random configs")),
            ("tests-new", after("every migration test")),
        ],
        strikes: vec![],
        emphasis: vec![(
            "migrate",
            before("whole config at once"),
            before("drifted apart").plus(1.2),
        )],
        late: vec![("tests", before("migration tests").plus(-0.3), 1.0)],
        footer_before: (
            vec![
                span("two converters, and the tests checked the ", Tone::Plain),
                span("unused one", Tone::Error),
            ],
            before("most of the migration tests"),
        ),
        footer_after: (
            vec![
                span("one converter · every test runs the ", Tone::Plain),
                span("real path", Tone::Success),
            ],
            after("every migration test"),
        ),
    }
}

fn duplicate_diff() -> (Diff, Vec<&'static str>, &'static str) {
    (
        Diff {
            file_name: "config/normalize.ts",
            lines: vec![
                keep("const legacyMedia = own(input, \"attachment\")"),
                remove(
                    1,
                    "  ? decodeValue(ConfigAttachmentV1.Info, input.attachment, …)",
                ),
                add(
                    1,
                    "  ? decodeEncoded(ConfigMedia.Info, input.attachment, …)",
                ),
                remove(
                    1,
                    "encoded.media = migrate({ attachment: legacyMedia }).media",
                ),
                add(1, "encoded.media = legacyMedia"),
                keep("const migratedUpdate ="),
                remove(
                    2,
                    "  ConfigMigrateV1.migrate({ autoupdate: legacyUpdate }).update",
                ),
                add(2, "  legacyUpdate === false ? \"disable\""),
                add(2, "    : legacyUpdate === \"notify\" ? \"notify\""),
                add(2, "    : legacyUpdate === true ? \"auto\" : undefined"),
                keep("const migratedSmallModel ="),
                remove(
                    3,
                    "  migrate({ small_model: legacySmallModel }).agents?.title?.model",
                ),
                add(3, "  ConfigMigrateV1.modelSelection(legacySmallModel)"),
            ],
        },
        vec![
            "attachment decodes straight",
            "maps to an update policy",
            "same selection helper",
        ],
        "condensed for display · migrate() and its helpers are deleted",
    )
}

// ---------------------------------------------------------------------------
// #51901: the top-level model gets the same rename
// ---------------------------------------------------------------------------

fn rename_flow() -> Flow {
    let sequence = SequencePlan {
        origin: [300.0, 180.0],
        width: 1380.0,
        row_height: 96.0,
        slots: Some(6),
        participants: vec![
            participant("config", "old config", "v1 provider names"),
            participant("normalize", "normalize", "config loading"),
            participant("resolver", "model resolver", "at run time"),
        ],
        rows: vec![
            message(
                "agent",
                "config",
                "normalize",
                "agent model: google-vertex-anthropic/claude",
                Tone::Request,
            ),
            message(
                "agent-ok",
                "normalize",
                "resolver",
                "google-vertex/claude",
                Tone::Success,
            ),
            message(
                "top",
                "config",
                "normalize",
                "model: azure-cognitive-services/deployment",
                Tone::Request,
            ),
            message(
                "pass",
                "normalize",
                "resolver",
                "azure-cognitive-services/deployment",
                Tone::Warning,
            )
            .aside("not renamed"),
            reply(
                "fail",
                "resolver",
                "config",
                "this provider has been deprecated",
                Tone::Error,
            ),
            message(
                "renamed",
                "normalize",
                "resolver",
                "azure/deployment",
                Tone::Success,
            )
            .slot(3),
            reply(
                "loads",
                "resolver",
                "config",
                "default model loads",
                Tone::Success,
            )
            .slot(4),
        ],
    };
    Flow {
        sequence,
        before_only: &["pass", "fail"],
        reveals: vec![
            ("agent", before("renames them for agents")),
            ("agent-ok", before("provider settings")),
            ("top", before("top level model")),
            ("pass", before("keep working")),
            ("fail", before("has been deprecated")),
            ("renamed", after("becomes plain azure")),
            ("loads", after("loads like everything")),
        ],
        strikes: vec![],
        emphasis: vec![(
            "normalize",
            before("but not for"),
            before("keep working").plus(0.8),
        )],
        late: vec![],
        footer_before: (
            vec![
                span("agents are renamed, the ", Tone::Plain),
                span("top-level model", Tone::Error),
                span(" is not", Tone::Plain),
            ],
            before("default model fails"),
        ),
        footer_after: (
            vec![
                span("every model gets the ", Tone::Plain),
                span("same rename", Tone::Success),
            ],
            after("becomes plain azure"),
        ),
    }
}

fn rename_diff() -> (Diff, Vec<&'static str>, &'static str) {
    (
        Diff {
            file_name: "config/normalize.ts",
            lines: vec![
                keep("normalizeLsp(input, encoded, diagnostics)"),
                keep(""),
                add(2, "const model = own(input, \"model\")"),
                add(2, "  ? decodeValue(ConfigModel.Selection, input.model, …)"),
                add(2, "  : undefined"),
                add(3, "if (model)"),
                add(3, "  encoded.model = {"),
                add(3, "    ...model,"),
                add(
                    3,
                    "    providerID: ConfigMigrateV1.providerID(model.providerID),",
                ),
                add(3, "  }"),
                keep("const nativeAtomic = {"),
                keep("  shell: Info.fields.shell,"),
                remove(1, "  model: Info.fields.model,"),
                keep("  default_agent: Info.fields.default_agent,"),
            ],
        },
        vec![
            "leaves the shared field loop",
            "decodes as a model selection",
            "through the same rename",
        ],
        "condensed for display",
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    #[test]
    fn reels_match_the_committed_plans() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let narration = root.join("narration");
        for (file, reel) in [
            (
                "pr-51889.reel.json",
                super::build_duplicate(&narration).unwrap(),
            ),
            (
                "pr-51901.reel.json",
                super::build_rename(&narration).unwrap(),
            ),
        ] {
            let committed = std::fs::read_to_string(root.join(file)).unwrap();
            assert_eq!(
                serde_json::to_string_pretty(&reel).unwrap() + "\n",
                committed
            );
        }
    }
}
