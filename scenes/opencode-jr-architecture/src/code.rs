//! The Worker's pipeline in `src/ingress.ts`, condensed for display: each
//! stage of the handler arrives as the narration names it.
use anyhow::{Context, Result};
use kinograph::{
    author::PlanBuilder,
    editor::{EDITOR_RECIPE, EditorLinePlan, EditorPartPlan, EditorRecipePlan, EditorSnapshotPlan},
    highlight,
    plan::ScenePlan,
    tone::Tone,
};

use crate::{Narration, chip_at, footer, header_at, seconds};

/// The widest line that fits the editor card at 28 px CommitMono.
const MAX_COLUMNS: usize = 76;
/// Rows visible in the editor card.
const MAX_ROWS: usize = 14;

/// `(step, text)`: the step whose phrase brings the line in.
const LINES: [(usize, &str); 14] = [
    (
        0,
        "const body = yield* verifyRequest(request, workerEnv.signingSecret)",
    ),
    (0, "if (body === undefined)"),
    (
        0,
        "  return new Response(\"invalid signature\", { status: 401 })",
    ),
    (1, "const decoded = decodeEnvelope(body)"),
    (1, "if (Result.isFailure(decoded))"),
    (
        1,
        "  return new Response(\"malformed envelope\", { status: 400 })",
    ),
    (2, "if (yield* isForeignTeam(envelope)) return ignored()"),
    (
        2,
        "if (message.sourceType !== \"app_mention\" && !followUp &&",
    ),
    (2, "    !isDirectMessageWork(contextual)) return ignored()"),
    (
        2,
        "const trust = yield* admittedTrust(contextual, authorizations, followUp)",
    ),
    (2, "if (trust === undefined) return ignored()"),
    (
        3,
        "// resolves only after the event is in the DO's SQLite mailbox",
    ),
    (
        3,
        "yield* sessions.enqueue(envelope.event_id, event, { deliberate })",
    ),
    (3, "return ok()"),
];

fn ids(step: usize) -> Vec<String> {
    LINES
        .iter()
        .enumerate()
        .filter(|(_, (from, _))| *from <= step)
        .map(|(index, _)| format!("line-{index}"))
        .collect()
}

fn editor(scene: &mut PlanBuilder, step_times: [u64; 3]) -> Result<()> {
    anyhow::ensure!(LINES.len() <= MAX_ROWS, "too many rows for the editor");
    let lines = LINES
        .iter()
        .enumerate()
        .map(|(index, (_, text))| {
            anyhow::ensure!(
                text.chars().count() <= MAX_COLUMNS,
                "line {index} is wider than the editor: {text}"
            );
            Ok(EditorLinePlan {
                id: format!("line-{index}"),
                parts: vec![EditorPartPlan {
                    id: "code".to_owned(),
                    spans: highlight::typescript(text),
                }],
                semantic_ranges: Vec::new(),
                mark: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let recipe = EditorRecipePlan {
        file_name: "src/ingress.ts".to_owned(),
        focus_line_id: "line-0".to_owned(),
        lines,
        initial_line_ids: ids(0),
        final_line_ids: ids(3),
        snapshots: step_times
            .iter()
            .enumerate()
            .map(|(step, &at)| EditorSnapshotPlan {
                at_nanos: at,
                line_ids: ids(step + 1),
            })
            .collect(),
        line_height: 44.0,
        entering_offset_x: 0.0,
        focus_height: 44.0,
        inline_reveal: None,
        additional_inline_reveals: Vec::new(),
    };
    recipe.compile()?;
    scene.actor("editor", EDITOR_RECIPE, &recipe)?;
    Ok(())
}

pub fn film(narration: &Narration) -> Result<ScenePlan> {
    let clip = narration.clip("edge-code")?;
    let lead = seconds(0.9);
    let mut sc = PlanBuilder::new("edge-code", lead + clip.duration() + seconds(1.8));
    let v = clip.place(&mut sc, lead);
    header_at(&mut sc, "1", "the edge", None)?;
    chip_at(&mut sc, "src/ingress.ts", None)?;
    editor(
        &mut sc,
        [
            v.at("malformed envelope"),
            v.at("not for us"),
            v.at("mailbox write"),
        ],
    )?;
    footer(
        &mut sc,
        "footer",
        &[
            ("condensed for display · ", Tone::Muted),
            ("200 only after the write", Tone::Accent),
        ],
        v.at("mailbox write") + seconds(0.6),
    )?;
    sc.finish().context("edge-code")
}
