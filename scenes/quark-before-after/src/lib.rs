use anyhow::Result;
use kinograph::{
    author::PlanBuilder,
    code::{StyledSpan, SyntaxStyle},
    editor::{
        EDITOR_RECIPE, EditorInlineRevealPlan, EditorLinePlan, EditorPartPlan, EditorRecipePlan,
        EditorSemanticRangePlan,
    },
    plan::{MediaKindPlan, MediaPlan, MediaRolePlan, ScenePlan},
};
use serde_json::json;
use std::path::PathBuf;

const SECOND: u64 = 1_000_000_000;
const DURATION: u64 = 23 * SECOND;
const NARRATION_DURATION: u64 = 22_151_837_000;

pub fn build_plan() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("quark-before-after", DURATION);
    let editor = scene.actor("editor", EDITOR_RECIPE, editor_recipe())?;
    let before = scene.actor(
        "before-label",
        "text",
        json!({
            "text": "BEFORE  /  SOLID STORE",
            "center": [960, 118],
            "fontSize": 25,
            "color": [148, 163, 184],
        }),
    )?;
    let after = scene.actor(
        "after-label",
        "text",
        json!({
            "text": "AFTER  /  QUARK",
            "center": [960, 118],
            "fontSize": 25,
            "color": [96, 165, 250],
        }),
    )?;
    let reconcile_detail = scene.actor(
        "reconcile-detail",
        "text",
        json!({
            "text": "RECONCILE THE ARRAY",
            "center": [960, 1010],
            "fontSize": 21,
            "color": [148, 163, 184],
        }),
    )?;
    let slot_detail = scene.actor(
        "slot-detail",
        "text",
        json!({
            "text": "ONE SLOT  /  NO STRUCTURAL PUBLICATION",
            "center": [960, 1010],
            "fontSize": 21,
            "color": [96, 165, 250],
        }),
    )?;

    let panel_y = scene.continuous(&editor, "panel-y", 260.0);
    let panel_scale = scene.continuous(&editor, "panel-scale", 0.86);
    let panel_rotation = scene.continuous(&editor, "panel-rotation", -0.045);
    let panel_tilt_x = scene.continuous(&editor, "panel-tilt-x", -0.18);
    let panel_tilt_y = scene.continuous(&editor, "panel-tilt-y", 0.24);
    let panel_near_blur = scene.continuous(&editor, "panel-near-blur", 10.0);
    let layout = scene.continuous(&editor, "layout", 0.0);
    let content = scene.continuous(&editor, "content", 0.0);
    let api_change = scene.continuous(&editor, "api-change", 0.0);
    let update_change = scene.continuous(&editor, "update-change", 0.0);
    let before_opacity = scene.continuous(&before, "opacity", 0.0);
    let after_opacity = scene.continuous(&after, "opacity", 0.0);
    let reconcile_opacity = scene.continuous(&reconcile_detail, "opacity", 0.0);
    let slot_opacity = scene.continuous(&slot_detail, "opacity", 0.0);

    for (channel, target) in [
        (&panel_y, 0.0),
        (&panel_scale, 1.0),
        (&panel_rotation, 0.0),
        (&panel_tilt_x, 0.0),
        (&panel_tilt_y, 0.0),
        (&panel_near_blur, 0.0),
    ] {
        scene.spring(channel, 0, target, 0.7, 0.0);
    }
    scene.spring(&before_opacity, 350_000_000, 1.0, 0.4, 0.0);
    scene.spring(&reconcile_opacity, 2_379_000_000, 1.0, 0.4, 0.0);

    // Open the new container before exchanging inline content.
    scene.spring(&layout, 8_619_000_000, 1.0, 0.45, 0.0);
    scene.spring(&content, 8_749_000_000, 1.0, 0.45, 0.0);
    scene.spring(&before_opacity, 8_419_000_000, 0.0, 0.35, 0.0);
    scene.spring(&after_opacity, 8_819_000_000, 1.0, 0.35, 0.0);
    scene.spring(&api_change, 9_319_000_000, 1.0, 0.4, 0.0);

    scene.spring(&update_change, 13_239_000_000, 1.0, 0.4, 0.0);
    scene.spring(&reconcile_opacity, 13_039_000_000, 0.0, 0.35, 0.0);
    scene.spring(&slot_opacity, 13_439_000_000, 1.0, 0.35, 0.0);

    scene.media(MediaPlan {
        id: "narration".to_owned(),
        path: PathBuf::from("assets/narration.mp3"),
        kind: MediaKindPlan::Audio,
        role: MediaRolePlan::Script,
        source_start_nanos: 0,
        source_end_nanos: NARRATION_DURATION,
        timeline_start_nanos: 0,
        timeline_end_nanos: NARRATION_DURATION,
        gain_db: 0.0,
    });
    scene.cue("solid", 0, 8_619_000_000);
    scene.cue("identity", 8_619_000_000, 13_239_000_000);
    scene.cue("direct-update", 13_239_000_000, 19_399_000_000);
    scene.cue("less-work", 19_399_000_000, DURATION);
    Ok(scene.finish()?)
}

fn editor_recipe() -> EditorRecipePlan {
    use SyntaxStyle::{Accent, Keyword, Plain, Type};

    EditorRecipePlan {
        file_name: "state.ts".to_owned(),
        snapshots: Vec::new(),
        lines: vec![
            semantic_line(
                "comment",
                vec![
                    part("comment-prefix", vec![span("// ", Accent)]),
                    part("solid-comment", vec![span("Values and structure", Accent)]),
                    part("quark-comment", vec![span("Stable keyed slots", Accent)]),
                ],
                vec![
                    semantic_range("solid-comment", "solid-comment", "solid-comment"),
                    semantic_range("quark-comment", "quark-comment", "quark-comment"),
                ],
            ),
            semantic_line(
                "declaration",
                vec![
                    part("const", vec![span("const", Keyword)]),
                    part("space", vec![span(" ", Plain)]),
                    part("solid-binding-open", vec![span("[", Plain)]),
                    part("rows", vec![span("rows", Plain)]),
                    part("solid-binding-rest", vec![span(", setRows]", Plain)]),
                    part("equals", vec![span(" = ", Plain)]),
                    part(
                        "solid-initializer",
                        vec![span("createStore", Type), span("(initialRows)", Plain)],
                    ),
                    part(
                        "quark-initializer",
                        vec![
                            span("Keyed", Type),
                            span(".make({ ", Plain),
                            span("key", Accent),
                            span(": (row) => row.id })", Plain),
                        ],
                    ),
                ],
                vec![
                    semantic_range(
                        "solid-binding-open",
                        "solid-binding-open",
                        "solid-binding-open",
                    ),
                    semantic_range(
                        "solid-binding-rest",
                        "solid-binding-rest",
                        "solid-binding-rest",
                    ),
                    semantic_range(
                        "solid-initializer",
                        "solid-initializer",
                        "solid-initializer",
                    ),
                    semantic_range(
                        "quark-initializer",
                        "quark-initializer",
                        "quark-initializer",
                    ),
                ],
            ),
            line("gap", vec![]),
            semantic_line(
                "update",
                vec![
                    part(
                        "solid-update-prefix",
                        vec![span("setRows(reconcile(", Type)],
                    ),
                    part("quark-update-prefix", vec![span("rows.update(", Accent)]),
                    part("next-row", vec![span("nextRow", Plain)]),
                    part("solid-update-plural", vec![span("s", Plain)]),
                    part("solid-update-suffix", vec![span("))", Plain)]),
                    part("quark-update-suffix", vec![span(")", Plain)]),
                ],
                vec![
                    semantic_range(
                        "solid-update-prefix",
                        "solid-update-prefix",
                        "solid-update-prefix",
                    ),
                    semantic_range(
                        "quark-update-prefix",
                        "quark-update-prefix",
                        "quark-update-prefix",
                    ),
                    semantic_range(
                        "solid-update-plural",
                        "solid-update-plural",
                        "solid-update-plural",
                    ),
                    semantic_range(
                        "solid-update-suffix",
                        "solid-update-suffix",
                        "solid-update-suffix",
                    ),
                    semantic_range(
                        "quark-update-suffix",
                        "quark-update-suffix",
                        "quark-update-suffix",
                    ),
                ],
            ),
            line("render-gap", vec![]),
            line(
                "slots",
                vec![
                    span("const", Keyword),
                    span(" slots = ", Plain),
                    span("useValue", Type),
                    span("(rows.slots)", Plain),
                ],
            ),
            semantic_line(
                "for-open",
                vec![
                    part("open", vec![span("<", Plain)]),
                    part("keyed-prefix", vec![span("Keyed", Accent)]),
                    part("for", vec![span("For", Type)]),
                    part("each-open", vec![span(" each={", Plain)]),
                    part("solid-source", vec![span("rows", Plain)]),
                    part("quark-source", vec![span("slots", Plain)]),
                    part("open-close", vec![span("}>", Plain)]),
                ],
                vec![
                    semantic_range("keyed-prefix", "keyed-prefix", "keyed-prefix"),
                    semantic_range("solid-source", "solid-source", "solid-source"),
                    semantic_range("quark-source", "quark-source", "quark-source"),
                ],
            ),
            semantic_line(
                "row",
                vec![
                    part(
                        "row-prefix",
                        vec![span("  {(row) => <RowView row={row", Plain)],
                    ),
                    part("row-call", vec![span("()", Accent)]),
                    part("row-suffix", vec![span("} />}", Plain)]),
                ],
                vec![semantic_range("row-call", "row-call", "row-call")],
            ),
            semantic_line(
                "for-close",
                vec![
                    part("close-open", vec![span("</", Plain)]),
                    part("close-keyed-prefix", vec![span("Keyed", Accent)]),
                    part("close-for", vec![span("For", Type)]),
                    part("close", vec![span(">", Plain)]),
                ],
                vec![semantic_range(
                    "close-keyed-prefix",
                    "close-keyed-prefix",
                    "close-keyed-prefix",
                )],
            ),
        ],
        initial_line_ids: ids([
            "comment",
            "declaration",
            "gap",
            "update",
            "render-gap",
            "for-open",
            "row",
            "for-close",
        ]),
        final_line_ids: ids([
            "comment",
            "declaration",
            "gap",
            "update",
            "render-gap",
            "slots",
            "for-open",
            "row",
            "for-close",
        ]),
        line_height: 46.0,
        entering_offset_x: 0.0,
        focus_line_id: "for-open".to_owned(),
        focus_height: 46.0,
        inline_reveal: Some(reveal("comment", "solid-comment", true)),
        additional_inline_reveals: vec![
            reveal("comment", "quark-comment", false),
            reveal("declaration", "solid-binding-open", true),
            reveal("declaration", "solid-binding-rest", true),
            reveal("declaration", "solid-initializer", true),
            reveal("declaration", "quark-initializer", false),
            reveal("for-open", "keyed-prefix", false),
            reveal("for-open", "solid-source", true),
            reveal("for-open", "quark-source", false),
            reveal("row", "row-call", false),
            reveal("for-close", "close-keyed-prefix", false),
            reveal_on("update", "solid-update-prefix", "update-change", true),
            reveal_on("update", "quark-update-prefix", "update-change", false),
            reveal_on("update", "solid-update-plural", "update-change", true),
            reveal_on("update", "solid-update-suffix", "update-change", true),
            reveal_on("update", "quark-update-suffix", "update-change", false),
        ],
    }
}

fn reveal(line_id: &str, range_id: &str, reversed: bool) -> EditorInlineRevealPlan {
    reveal_on(line_id, range_id, "api-change", reversed)
}

fn reveal_on(
    line_id: &str,
    range_id: &str,
    channel: &str,
    reversed: bool,
) -> EditorInlineRevealPlan {
    EditorInlineRevealPlan {
        line_id: line_id.to_owned(),
        range_id: range_id.to_owned(),
        channel: Some(channel.to_owned()),
        reversed,
    }
}

fn line(id: &str, spans: Vec<StyledSpan>) -> EditorLinePlan {
    EditorLinePlan {
        id: id.to_owned(),
        parts: spans
            .into_iter()
            .enumerate()
            .map(|(index, span)| part(&format!("span-{index}"), vec![span]))
            .collect(),
        semantic_ranges: Vec::new(),
        mark: None,
    }
}

fn semantic_line(
    id: &str,
    parts: Vec<EditorPartPlan>,
    semantic_ranges: Vec<EditorSemanticRangePlan>,
) -> EditorLinePlan {
    EditorLinePlan {
        id: id.to_owned(),
        parts,
        semantic_ranges,
        mark: None,
    }
}

fn part(id: &str, spans: Vec<StyledSpan>) -> EditorPartPlan {
    EditorPartPlan {
        id: id.to_owned(),
        spans,
    }
}

fn semantic_range(id: &str, first: &str, last: &str) -> EditorSemanticRangePlan {
    EditorSemanticRangePlan {
        id: id.to_owned(),
        first_part_id: first.to_owned(),
        last_part_id: last.to_owned(),
    }
}

fn span(text: &str, style: SyntaxStyle) -> StyledSpan {
    StyledSpan::new(text, style)
}

fn ids<const N: usize>(values: [&str; N]) -> Vec<String> {
    values.into_iter().map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::build_plan;

    const CANONICAL_PLAN: &str = include_str!("../quark-before-after.plan.json");

    #[test]
    fn scene_has_one_before_after_transition() {
        let plan = build_plan().unwrap();
        let editor = &plan.actors[0].data;
        let initial = editor["initialLineIds"].as_array().unwrap();
        let final_lines = editor["finalLineIds"].as_array().unwrap();

        assert_eq!(plan.id, "quark-before-after");
        assert_eq!(plan.duration_nanos, 23_000_000_000);
        assert_eq!(plan.actors.len(), 5);
        assert_eq!(plan.cues.len(), 4);
        assert_eq!(plan.media.len(), 1);
        assert_eq!(initial.len(), 8);
        assert_eq!(final_lines.len(), 9);
        assert!(
            initial
                .iter()
                .all(|line| final_lines.iter().any(|candidate| candidate == line))
        );
        assert_eq!(
            editor["additionalInlineReveals"].as_array().unwrap().len(),
            15
        );
    }

    #[test]
    fn canonical_plan_matches_rust_scene_program() {
        assert_eq!(
            build_plan().unwrap().to_json_pretty().unwrap(),
            CANONICAL_PLAN
        );
    }
}
