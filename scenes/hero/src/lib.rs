use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, SECOND, seconds},
    code::{StyledSpan, SyntaxStyle},
    editor::{
        EDITOR_RECIPE, EditorInlineRevealPlan, EditorRecipePlan, EditorTargetSelector,
        POINTER_RECIPE, PointerRecipePlan, line, part, semantic_line, semantic_range,
    },
    plan::{ScenePlan, TargetComponentPlan},
};

const FOCUS_LINE_ID: &str = "find-effect";

pub fn build_plan() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("hero", 5 * SECOND);
    let editor = scene.actor("editor", EDITOR_RECIPE, editor_recipe())?;
    let pointer = scene.actor(
        "pointer",
        POINTER_RECIPE,
        PointerRecipePlan {
            editor_id: editor.id().to_owned(),
        },
    )?;

    let effect = scene.semantic_target(
        "effect-type",
        &editor,
        EditorTargetSelector {
            line_id: FOCUS_LINE_ID.to_owned(),
            range_id: "effect-type".to_owned(),
        },
    )?;
    let string = scene.semantic_target(
        "string-type",
        &editor,
        EditorTargetSelector {
            line_id: "find-signature".to_owned(),
            range_id: "string-type".to_owned(),
        },
    )?;
    let not_found = scene.semantic_target(
        "not-found-type",
        &editor,
        EditorTargetSelector {
            line_id: FOCUS_LINE_ID.to_owned(),
            range_id: "not-found-type".to_owned(),
        },
    )?;
    let context = scene.semantic_target(
        "context-tag",
        &editor,
        EditorTargetSelector {
            line_id: "class-open".to_owned(),
            range_id: "context-tag".to_owned(),
        },
    )?;

    let panel_y = scene.continuous(&editor, "panel-y", 250.0);
    let layout = scene.continuous(&editor, "layout", 0.0);
    let content = scene.continuous(&editor, "content", 0.0);
    let focus = scene.continuous(&editor, "focus", 0.0);
    let highlight_x = scene.continuous(&editor, "highlight-x", effect.x());
    let highlight_y = scene.continuous(&editor, "highlight-y", effect.line_y());
    let highlight_width = scene.continuous(&editor, "highlight-width", effect.width());
    let highlight_opacity = scene.continuous(&editor, "highlight-opacity", 0.0);
    let inline_reveal = scene.continuous(&editor, "inline-reveal", 0.0);

    let pointer_x = scene.continuous(
        &pointer,
        "x",
        effect.offset(TargetComponentPlan::CenterX, 40.0),
    );
    let pointer_y = scene.continuous(
        &pointer,
        "y",
        effect.offset(TargetComponentPlan::LineY, 85.0),
    );
    let pointer_opacity = scene.continuous(&pointer, "opacity", 0.0);
    let pointer_scale = scene.continuous(&pointer, "scale", 0.7);
    let pointer_blur = scene.continuous(&pointer, "blur", 4.0);

    scene.spring(&panel_y, seconds(0.08), 0.0, 0.3, 0.0);
    scene.spring(&layout, seconds(0.88), 1.0, 0.3, 0.0);
    scene.spring(&content, seconds(1.12), 1.0, 0.3, 0.0);
    scene.spring(&focus, seconds(1.55), 1.0, 0.3, 0.0);
    scene.spring_to(&highlight_x, seconds(1.72), effect.x(), 0.3, 0.0);
    scene.spring_to(&highlight_y, seconds(1.72), effect.line_y(), 0.3, 0.0);
    scene.spring_to(&highlight_width, seconds(1.72), effect.width(), 0.3, 0.0);
    scene.spring(&highlight_opacity, seconds(1.72), 1.0, 0.3, 0.0);

    scene.spring(&pointer_opacity, seconds(1.72), 1.0, 0.42, 0.18);
    scene.spring(&pointer_scale, seconds(1.72), 1.0, 0.42, 0.18);
    scene.spring(&pointer_blur, seconds(1.72), 0.0, 0.42, 0.18);
    move_pointer(&mut scene, &pointer_x, &pointer_y, 1.72, &effect);
    move_pointer(&mut scene, &pointer_x, &pointer_y, 2.35, &string);

    scene.spring(&inline_reveal, seconds(2.72), 1.0, 0.3, 0.0);
    scene.spring_to(&highlight_x, seconds(3.15), not_found.x(), 0.3, 0.0);
    scene.spring_to(&highlight_y, seconds(3.15), not_found.line_y(), 0.3, 0.0);
    scene.spring_to(&highlight_width, seconds(3.15), not_found.width(), 0.3, 0.0);
    scene.spring(&highlight_opacity, seconds(3.15), 1.0, 0.3, 0.0);
    move_pointer(&mut scene, &pointer_x, &pointer_y, 3.15, &not_found);
    move_pointer(&mut scene, &pointer_x, &pointer_y, 4.05, &context);

    scene.cue("code-entry", 880_000_000, 1_550_000_000);
    scene.cue("semantic-tour", 1_720_000_000, 5_000_000_000);
    Ok(scene.finish()?)
}

fn move_pointer(
    scene: &mut PlanBuilder,
    x: &psychopomp::author::ContinuousHandle,
    y: &psychopomp::author::ContinuousHandle,
    at: f64,
    target: &psychopomp::author::SemanticTargetHandle,
) {
    scene.spring_to(x, seconds(at), target.center_x(), 0.42, 0.18);
    scene.spring_to(
        y,
        seconds(at),
        target.offset(TargetComponentPlan::LineY, 55.0),
        0.42,
        0.18,
    );
}

fn editor_recipe() -> EditorRecipePlan {
    use SyntaxStyle::{Accent, Keyword, Plain, String as StringStyle, Type};

    EditorRecipePlan {
        file_name: "service.ts".to_owned(),
        snapshots: Vec::new(),
        lines: vec![
            line(
                "import",
                vec![
                    span("import", Keyword),
                    span(" { Context, Effect } ", Plain),
                    span("from", Keyword),
                    span(" \"effect\"", StringStyle),
                ],
            ),
            line("spacer", vec![]),
            semantic_line(
                "class-open",
                vec![
                    part("keyword", vec![span("class", Keyword)]),
                    part("service", vec![span(" UserService ", Type)]),
                    part("extends", vec![span("extends", Keyword)]),
                    part("context-space", vec![span(" ", Plain)]),
                    part("context-tag", vec![span("Context.Tag", Plain)]),
                    part("context-open", vec![span("(", Plain)]),
                    part("tag-name", vec![span("\"UserService\"", StringStyle)]),
                    part("suffix", vec![span(")<", Plain)]),
                ],
                vec![semantic_range("context-tag", "context-tag", "context-tag")],
            ),
            line(
                "service-self",
                vec![
                    span("  ", Plain),
                    span("UserService", Type),
                    span(",", Plain),
                ],
            ),
            line("shape-open", vec![span("  {", Plain)]),
            semantic_line(
                "find-signature",
                vec![
                    part("indent", vec![span("    ", Plain)]),
                    part("readonly", vec![span("readonly", Keyword)]),
                    part("signature", vec![span(" find: (id: ", Plain)]),
                    part("string-type", vec![span("string", Type)]),
                    part("suffix", vec![span(") =>", Plain)]),
                ],
                vec![semantic_range("string-type", "string-type", "string-type")],
            ),
            semantic_line(
                FOCUS_LINE_ID,
                vec![
                    part("indent", vec![span("      ", Plain)]),
                    part("effect-type", vec![span("Effect.Effect", Accent)]),
                    part("open", vec![span("<", Plain)]),
                    part("success-type", vec![span("User", Type)]),
                    part("error-separator", vec![span(", ", Plain)]),
                    part("not-found-type", vec![span("NotFound", Accent)]),
                    part("close", vec![span(">", Plain)]),
                ],
                vec![
                    semantic_range("effect-type", "effect-type", "effect-type"),
                    semantic_range("not-found-type", "not-found-type", "not-found-type"),
                    semantic_range("error-slot", "error-separator", "not-found-type"),
                ],
            ),
            line("shape-close", vec![span("  }", Plain)]),
            line("close", vec![span(">() {}", Plain)]),
        ],
        initial_line_ids: vec![
            "import".to_owned(),
            "spacer".to_owned(),
            "class-open".to_owned(),
            "service-self".to_owned(),
            "shape-open".to_owned(),
            "shape-close".to_owned(),
            "close".to_owned(),
        ],
        final_line_ids: vec![
            "import".to_owned(),
            "spacer".to_owned(),
            "class-open".to_owned(),
            "service-self".to_owned(),
            "shape-open".to_owned(),
            "find-signature".to_owned(),
            FOCUS_LINE_ID.to_owned(),
            "shape-close".to_owned(),
            "close".to_owned(),
        ],
        line_height: 44.0,
        entering_offset_x: 96.0,
        focus_line_id: FOCUS_LINE_ID.to_owned(),
        focus_height: 44.0,
        inline_reveal: Some(EditorInlineRevealPlan {
            line_id: FOCUS_LINE_ID.to_owned(),
            range_id: "error-slot".to_owned(),
            channel: None,
            reversed: false,
        }),
        additional_inline_reveals: Vec::new(),
    }
}

fn span(text: &str, style: SyntaxStyle) -> StyledSpan {
    StyledSpan::new(text, style)
}

#[cfg(test)]
mod tests {
    use super::build_plan;

    const CANONICAL_PLAN: &str = include_str!("../hero.plan.json");

    #[test]
    fn hero_is_a_valid_scene_plan_with_semantic_targets() {
        let plan = build_plan().unwrap();

        assert_eq!(plan.actors.len(), 2);
        assert_eq!(plan.semantic_targets.len(), 4);
        assert_eq!(plan.duration_nanos, 5_000_000_000);
        assert!(plan.to_json_pretty().unwrap().contains("effect-type"));
    }

    #[test]
    fn canonical_plan_matches_rust_scene_program() {
        assert_eq!(
            build_plan().unwrap().to_json_pretty().unwrap(),
            CANONICAL_PLAN
        );
    }
}
