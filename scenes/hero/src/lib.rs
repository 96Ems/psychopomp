use anyhow::Result;
use kinograph::{
    author::PlanBuilder,
    code::{StyledSpan, SyntaxStyle},
    editor::{
        EDITOR_RECIPE, EditorInlineRevealPlan, EditorLinePlan, EditorPartPlan, EditorRecipePlan,
        EditorSemanticRangePlan, EditorTargetSelector, POINTER_RECIPE, PointerRecipePlan,
    },
    plan::{ScenePlan, TargetComponentPlan},
};

const SECOND: u64 = 1_000_000_000;
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

    spring(&mut scene, &panel_y, 0.08, 0.0, 0.3, 0.0);
    spring(&mut scene, &layout, 0.88, 1.0, 0.3, 0.0);
    spring(&mut scene, &content, 1.12, 1.0, 0.3, 0.0);
    spring(&mut scene, &focus, 1.55, 1.0, 0.3, 0.0);
    spring_to(&mut scene, &highlight_x, 1.72, effect.x(), 0.3, 0.0);
    spring_to(&mut scene, &highlight_y, 1.72, effect.line_y(), 0.3, 0.0);
    spring_to(&mut scene, &highlight_width, 1.72, effect.width(), 0.3, 0.0);
    spring(&mut scene, &highlight_opacity, 1.72, 1.0, 0.3, 0.0);

    spring(&mut scene, &pointer_opacity, 1.72, 1.0, 0.42, 0.18);
    spring(&mut scene, &pointer_scale, 1.72, 1.0, 0.42, 0.18);
    spring(&mut scene, &pointer_blur, 1.72, 0.0, 0.42, 0.18);
    move_pointer(&mut scene, &pointer_x, &pointer_y, 1.72, &effect);
    move_pointer(&mut scene, &pointer_x, &pointer_y, 2.35, &string);

    spring(&mut scene, &inline_reveal, 2.72, 1.0, 0.3, 0.0);
    spring_to(&mut scene, &highlight_x, 3.15, not_found.x(), 0.3, 0.0);
    spring_to(&mut scene, &highlight_y, 3.15, not_found.line_y(), 0.3, 0.0);
    spring_to(
        &mut scene,
        &highlight_width,
        3.15,
        not_found.width(),
        0.3,
        0.0,
    );
    spring(&mut scene, &highlight_opacity, 3.15, 1.0, 0.3, 0.0);
    move_pointer(&mut scene, &pointer_x, &pointer_y, 3.15, &not_found);
    move_pointer(&mut scene, &pointer_x, &pointer_y, 4.05, &context);

    scene.cue("code-entry", 880_000_000, 1_550_000_000);
    scene.cue("semantic-tour", 1_720_000_000, 5_000_000_000);
    Ok(scene.finish()?)
}

fn move_pointer(
    scene: &mut PlanBuilder,
    x: &kinograph::author::ContinuousHandle,
    y: &kinograph::author::ContinuousHandle,
    at: f64,
    target: &kinograph::author::SemanticTargetHandle,
) {
    spring_to(scene, x, at, target.center_x(), 0.42, 0.18);
    spring_to(
        scene,
        y,
        at,
        target.offset(TargetComponentPlan::LineY, 55.0),
        0.42,
        0.18,
    );
}

fn spring(
    scene: &mut PlanBuilder,
    channel: &kinograph::author::ContinuousHandle,
    at: f64,
    target: f32,
    visual_duration: f32,
    bounce: f32,
) {
    scene.spring(channel, nanos(at), target, visual_duration, bounce);
}

fn spring_to(
    scene: &mut PlanBuilder,
    channel: &kinograph::author::ContinuousHandle,
    at: f64,
    target: kinograph::plan::ScalarPlan,
    visual_duration: f32,
    bounce: f32,
) {
    scene.spring_to(channel, nanos(at), target, visual_duration, bounce);
}

fn nanos(seconds: f64) -> u64 {
    (seconds * SECOND as f64).round() as u64
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
                    part("keyword", span("class", Keyword)),
                    part("service", span(" UserService ", Type)),
                    part("extends", span("extends", Keyword)),
                    part("context-space", span(" ", Plain)),
                    part("context-tag", span("Context.Tag", Plain)),
                    part("context-open", span("(", Plain)),
                    part("tag-name", span("\"UserService\"", StringStyle)),
                    part("suffix", span(")<", Plain)),
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
                    part("indent", span("    ", Plain)),
                    part("readonly", span("readonly", Keyword)),
                    part("signature", span(" find: (id: ", Plain)),
                    part("string-type", span("string", Type)),
                    part("suffix", span(") =>", Plain)),
                ],
                vec![semantic_range("string-type", "string-type", "string-type")],
            ),
            semantic_line(
                FOCUS_LINE_ID,
                vec![
                    part("indent", span("      ", Plain)),
                    part("effect-type", span("Effect.Effect", Accent)),
                    part("open", span("<", Plain)),
                    part("success-type", span("User", Type)),
                    part("error-separator", span(", ", Plain)),
                    part("not-found-type", span("NotFound", Accent)),
                    part("close", span(">", Plain)),
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

fn line(id: &str, spans: Vec<StyledSpan>) -> EditorLinePlan {
    EditorLinePlan {
        id: id.to_owned(),
        parts: spans
            .into_iter()
            .enumerate()
            .map(|(index, span)| part(&format!("span-{index}"), span))
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

fn part(id: &str, span: StyledSpan) -> EditorPartPlan {
    EditorPartPlan {
        id: id.to_owned(),
        spans: vec![span],
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
