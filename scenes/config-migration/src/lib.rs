//! Narrated explainers for two OpenCode config pull requests.
//!
//! #51889 removes the duplicate V1 config migration; #51901 renames legacy
//! provider IDs in the top-level model. Each reel plays the broken behavior as a
//! sequence diagram, replays the fix in the same slots, then animates the diff.
//! Both use the `kinograph_pr_walkthrough::film` template, so every visual moment
//! is keyed to a phrase in the narration.
use std::path::Path;

use anyhow::Result;
use kinograph::{
    editor::diff::{Diff, add, keep, remove},
    narration::Narration,
    plan::ReelPlan,
    sequence::{SequenceParticipantPlan as Participant, SequencePlan, SequenceRowPlan as Row},
    tone::Tone,
};
use kinograph_pr_walkthrough::film::{Flow, Pr, TRANSITION, after, before, behavior, code, span};

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
    ReelPlan::dipped(
        "pr-51889",
        vec![
            behavior(&DUPLICATE, &narration, duplicate_flow())?,
            code(&DUPLICATE, &narration, duplicate_diff(), true)?,
        ],
        TRANSITION,
    )
}

pub fn build_rename(narration_dir: &Path) -> Result<ReelPlan> {
    let narration = Narration::load(narration_dir)?;
    ReelPlan::dipped(
        "pr-51901",
        vec![
            behavior(&RENAME, &narration, rename_flow())?,
            code(&RENAME, &narration, rename_diff(), true)?,
        ],
        TRANSITION,
    )
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
            Participant::new("config", "v1 config", "opencode.json"),
            Participant::new("normalize", "normalize", "runs in production"),
            Participant::new("migrate", "migrate", "whole-config copy"),
            Participant::new("tests", "tests", "migration suite"),
        ],
        rows: vec![
            Row::message(
                "load",
                "config",
                "normalize",
                "legacy fields",
                Tone::Request,
            ),
            Row::note(
                "whole",
                &["migrate"],
                "converts a whole config",
                Tone::Muted,
            ),
            Row::message(
                "borrow",
                "normalize",
                "migrate",
                "attachment · autoupdate · small_model",
                Tone::Warning,
            )
            .with_aside("3 calls"),
            Row::note(
                "copy",
                &["normalize", "migrate"],
                "the rest duplicates normalize",
                Tone::Warning,
            ),
            Row::note(
                "drift",
                &["normalize", "migrate"],
                "compaction.prune: unsupported  ≠  passed through",
                Tone::Error,
            ),
            Row::message(
                "tests-old",
                "tests",
                "migrate",
                "most migration tests",
                Tone::Warning,
            ),
            Row::message(
                "direct",
                "normalize",
                "normalize",
                "three fields, handled directly",
                Tone::Success,
            )
            .in_slot(1),
            Row::end("gone", "migrate", "deleted", Tone::Muted).in_slot(2),
            Row::note(
                "fixture",
                &["tests"],
                "v1 schema → random configs",
                Tone::Success,
            )
            .in_slot(3),
            Row::message(
                "tests-new",
                "tests",
                "normalize",
                "every migration test",
                Tone::Success,
            )
            .in_slot(5),
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
            Participant::new("config", "old config", "v1 provider names"),
            Participant::new("normalize", "normalize", "config loading"),
            Participant::new("resolver", "model resolver", "at run time"),
        ],
        rows: vec![
            Row::message(
                "agent",
                "config",
                "normalize",
                "agent model: google-vertex-anthropic/claude",
                Tone::Request,
            ),
            Row::message(
                "agent-ok",
                "normalize",
                "resolver",
                "google-vertex/claude",
                Tone::Success,
            ),
            Row::message(
                "top",
                "config",
                "normalize",
                "model: azure-cognitive-services/deployment",
                Tone::Request,
            ),
            Row::message(
                "pass",
                "normalize",
                "resolver",
                "azure-cognitive-services/deployment",
                Tone::Warning,
            )
            .with_aside("not renamed"),
            Row::reply(
                "fail",
                "resolver",
                "config",
                "this provider has been deprecated",
                Tone::Error,
            ),
            Row::message(
                "renamed",
                "normalize",
                "resolver",
                "azure/deployment",
                Tone::Success,
            )
            .in_slot(3),
            Row::reply(
                "loads",
                "resolver",
                "config",
                "default model loads",
                Tone::Success,
            )
            .in_slot(4),
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
