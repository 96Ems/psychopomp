//! Native presentation examples sharing one Scene Plan model with video export.
use anyhow::Result;
use kinograph::{
    author::PlanBuilder,
    code::{StyledSpan, SyntaxStyle},
    dsl::TaskState,
    editor::{
        EditorInlineRevealPlan, EditorLinePlan, EditorPartPlan, EditorRecipePlan,
        EditorSemanticRangePlan, EditorSnapshotPlan, EditorTargetSelector,
    },
    plan::{DeckPlan, ScenePlan, SlidePlan},
    task::{TASK_RECIPE, TaskEventPlan, TaskRecipePlan},
};
use serde_json::json;

const SECOND: u64 = 1_000_000_000;

pub fn build_deck() -> Result<DeckPlan> {
    use TaskState::*;
    let lifecycle = task_slide(
        "task-lifecycle",
        "An Effect is a description",
        &["loadAnswer"],
        &[
            ("Idle: nothing has run yet", vec![Idle]),
            ("Run it: the computation is now executing", vec![Running]),
            (
                "Success produces a value",
                vec![Succeeded(Some("42".into()))],
            ),
            ("The same description can be run again", vec![Idle]),
            ("Start another run", vec![Running]),
            (
                "Failure is a value we can handle",
                vec![Failed("NetworkError".into())],
            ),
            ("Retry the failed computation", vec![Running]),
            ("The retry succeeds", vec![Succeeded(Some("42".into()))]),
        ],
    )?;
    let parallel = task_slide(
        "parallel-effects",
        "Independent computations, one result",
        &["loadUser", "loadSettings", "renderPage"],
        &[
            ("Three stable computations", vec![Idle, Idle, Idle]),
            (
                "Run the independent requests together",
                vec![Running, Running, Idle],
            ),
            (
                "The user finishes; settings are still running",
                vec![Succeeded(Some("Ada".into())), Running, Idle],
            ),
            (
                "Only the settings request fails",
                vec![
                    Succeeded(Some("Ada".into())),
                    Failed("Timeout".into()),
                    Idle,
                ],
            ),
            (
                "Retry just the failed branch",
                vec![Succeeded(Some("Ada".into())), Running, Idle],
            ),
            (
                "Both inputs are ready",
                vec![
                    Succeeded(Some("Ada".into())),
                    Succeeded(Some("dark".into())),
                    Idle,
                ],
            ),
            (
                "Use the successful values",
                vec![
                    Succeeded(Some("Ada".into())),
                    Succeeded(Some("dark".into())),
                    Running,
                ],
            ),
            (
                "The page is ready",
                vec![
                    Succeeded(Some("Ada".into())),
                    Succeeded(Some("dark".into())),
                    Succeeded(Some("Ready".into())),
                ],
            ),
        ],
    )?;
    let deck = DeckPlan {
        version: 1,
        id: "interactive-showcase".into(),
        slides: vec![
            SlidePlan {
                title: "Effect.succeed · stable inline reveals".into(),
                plan: kinograph_effect_succeed_slides::build_plan()?,
            },
            SlidePlan {
                title: "Effect blocks · lifecycle and retry".into(),
                plan: lifecycle,
            },
            SlidePlan {
                title: "Effect blocks · parallel work".into(),
                plan: parallel,
            },
            SlidePlan {
                title: "Code edits · keyed lines and attached focus".into(),
                plan: code_slide()?,
            },
        ],
    };
    deck.validate()?;
    Ok(deck)
}

fn task_slide(
    id: &str,
    title: &str,
    names: &[&str],
    steps: &[(&str, Vec<TaskState>)],
) -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new(id, steps.len() as u64 * 3 * SECOND);
    label(
        &mut scene,
        "heading",
        title,
        [960., 210.],
        46.,
        [234, 238, 244],
    )?;
    label(
        &mut scene,
        "controls",
        "← → steps     ' / Shift+' slides     Space pause     R replay",
        [960., 944.],
        21.,
        [115, 125, 140],
    )?;
    for (index, name) in names.iter().enumerate() {
        let x = 960. + (index as f32 - (names.len() - 1) as f32 * 0.5) * 420.;
        scene.actor(
            *name,
            TASK_RECIPE,
            TaskRecipePlan {
                name: name.to_string(),
                center: [x, 510.],
                initial: steps[0].1[index].clone(),
                events: steps
                    .iter()
                    .enumerate()
                    .skip(1)
                    .map(|(step, (_, states))| TaskEventPlan {
                        at_nanos: step as u64 * 3 * SECOND,
                        state: states[index].clone(),
                    })
                    .collect(),
            },
        )?;
    }
    for (index, (caption, _)) in steps.iter().enumerate() {
        let at = index as u64 * 3 * SECOND;
        scene.presentation_step(
            format!("step-{index}"),
            *caption,
            at,
            if index == 0 { 0 } else { at + 2 * SECOND },
        );
        // Like visual-types' CyclingSection/FadeOverlays: the aperture stays
        // fixed while rows roll through its 12-pixel top and bottom fades.
        let text = scene.actor(format!("caption-{index}"), "text", json!({"text": caption, "center": [960, 780], "fontSize": 30, "color": [170, 182, 200], "verticalMask": {"top": 750, "bottom": 810, "fade": 12}}))?;
        let opacity = scene.continuous(&text, "opacity", if index == 0 { 1. } else { 0. });
        let y = scene.continuous(&text, "y", if index == 0 { 780. } else { 824. });
        if index > 0 {
            scene.spring(&opacity, at, 1., 0.35, 0.);
            scene.spring(&y, at, 780., 0.35, 0.);
        }
        if index + 1 < steps.len() {
            scene.spring(&opacity, at + 3 * SECOND, 0., 0.35, 0.);
            scene.spring(&y, at + 3 * SECOND, 736., 0.35, 0.);
        }
    }
    Ok(scene.finish()?)
}

fn label(
    scene: &mut PlanBuilder,
    id: &str,
    text: &str,
    center: [f32; 2],
    size: f32,
    color: [u8; 3],
) -> Result<()> {
    scene.actor(
        id,
        "text",
        json!({"text": text, "center": center, "fontSize": size, "color": color}),
    )?;
    Ok(())
}

fn code_slide() -> Result<ScenePlan> {
    use SyntaxStyle::{Keyword, Plain, Rgb, String as StringStyle};
    let part = |id: &str, text: &str, style| EditorPartPlan {
        id: id.into(),
        spans: vec![StyledSpan::new(text, style)],
    };
    let line = |id: &str, parts: Vec<EditorPartPlan>| EditorLinePlan {
        id: id.into(),
        semantic_ranges: parts
            .iter()
            .map(|part| EditorSemanticRangePlan {
                id: part.id.clone(),
                first_part_id: part.id.clone(),
                last_part_id: part.id.clone(),
            })
            .collect(),
        parts,
    };
    let mut returns = line(
        "return",
        vec![
            part("prefix", "  return ", Keyword),
            part("id", "id", Plain),
            part("name", "user.name", Plain),
        ],
    );
    returns.semantic_ranges.push(EditorSemanticRangePlan {
        id: "value".into(),
        first_part_id: "id".into(),
        last_part_id: "name".into(),
    });
    let initial = vec!["open".into(), "id".into(), "return".into(), "close".into()];
    let order = |ids: &[&str]| ids.iter().map(|id| id.to_string()).collect::<Vec<_>>();
    let recipe = EditorRecipePlan {
        file_name: "load-name.ts".into(),
        lines: vec![
            line(
                "open",
                vec![
                    part("keywords", "async function ", Keyword),
                    part("name", "loadName", Rgb(250, 204, 130)),
                    part("parameters", "() {", Plain),
                ],
            ),
            line(
                "id",
                vec![
                    part("keyword", "  const ", Keyword),
                    part("binding", "id = ", Plain),
                    part("value", "\"user-1\"", StringStyle),
                ],
            ),
            line(
                "fetch",
                vec![
                    part("keyword", "  const ", Keyword),
                    part("binding", "user = ", Plain),
                    part("await", "await ", Keyword),
                    part("call", "fetchUser", Rgb(250, 204, 130)),
                    part("argument", "(id)", Plain),
                ],
            ),
            line(
                "log",
                vec![
                    part("console", "  console.", Plain),
                    part("call", "log", Rgb(250, 204, 130)),
                    part("argument", "(user)", Plain),
                ],
            ),
            returns,
            line("close", vec![part("body", "}", Plain)]),
        ],
        initial_line_ids: initial.clone(),
        final_line_ids: initial,
        snapshots: vec![
            EditorSnapshotPlan {
                at_nanos: 3 * SECOND,
                line_ids: order(&["open", "id", "fetch", "return", "close"]),
            },
            EditorSnapshotPlan {
                at_nanos: 6 * SECOND,
                line_ids: order(&["open", "id", "fetch", "log", "return", "close"]),
            },
            EditorSnapshotPlan {
                at_nanos: 9 * SECOND,
                line_ids: order(&["open", "id", "fetch", "return", "close"]),
            },
            EditorSnapshotPlan {
                at_nanos: 12 * SECOND,
                line_ids: order(&["open", "id", "return", "close"]),
            },
        ],
        line_height: 44.,
        entering_offset_x: 0.,
        focus_line_id: "return".into(),
        focus_height: 44.,
        inline_reveal: EditorInlineRevealPlan {
            line_id: "return".into(),
            range_id: "id".into(),
            channel: Some("load".into()),
            reversed: true,
        },
        additional_inline_reveals: vec![EditorInlineRevealPlan {
            line_id: "return".into(),
            range_id: "name".into(),
            channel: Some("load".into()),
            reversed: false,
        }],
    };
    let mut scene = PlanBuilder::new("stable-code-edits", 15 * SECOND);
    let editor = scene.actor("editor", "editor", recipe)?;
    let load = scene.continuous(&editor, "load", 0.);
    scene.spring(&load, 3 * SECOND, 1., 0.4, 0.);
    scene.spring(&load, 12 * SECOND, 0., 0.4, 0.);
    let value = scene.semantic_target(
        "return-value",
        &editor,
        EditorTargetSelector {
            line_id: "return".into(),
            range_id: "value".into(),
        },
    )?;
    scene.continuous(&editor, "highlight-x", value.x());
    scene.continuous(&editor, "highlight-y", value.line_y());
    scene.continuous(&editor, "highlight-width", value.width());
    scene.continuous(&editor, "highlight-opacity", 0.8);
    label(
        &mut scene,
        "controls",
        "← → change code     ' / Shift+' slides     Interrupt at any time",
        [960., 1000.],
        21.,
        [115, 125, 140],
    )?;
    for (index, title) in [
        "Return the ID",
        "Fetch the user and return its name",
        "Add logging without replacing the return",
        "Remove only the logging line",
        "Return to the original code",
    ]
    .into_iter()
    .enumerate()
    {
        let at = index as u64 * 3 * SECOND;
        scene.presentation_step(
            format!("edit-{index}"),
            title,
            at,
            if index == 0 { 0 } else { at + 2 * SECOND },
        );
    }
    Ok(scene.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn showcase_has_four_valid_independent_slides_and_stable_code_deltas() {
        let deck = build_deck().unwrap();
        assert_eq!(deck.slides.len(), 4);
        let json = serde_json::to_string(&deck).unwrap();
        serde_json::from_str::<DeckPlan>(&json)
            .unwrap()
            .validate()
            .unwrap();
        let mut duplicate = deck.clone();
        duplicate.slides.push(duplicate.slides[0].clone());
        assert!(duplicate.validate().is_err());
        for slide in &deck.slides {
            if slide
                .plan
                .actors
                .iter()
                .any(|actor| actor.recipe == "editor")
            {
                let report =
                    serde_json::to_value(kinograph::editor::inspect_steps(&slide.plan).unwrap())
                        .unwrap();
                assert_eq!(report["warnings"], json!([]));
            }
        }
    }
}
