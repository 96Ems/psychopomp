//! Presentation-sized adaptation of Effect Institute's basics/effect-succeed.
//! The two stable declaration lines are a display reflow, not a changing slot.
use anyhow::Result;
use kinograph::{
    author::PlanBuilder,
    code::{StyledSpan, SyntaxStyle},
    editor::{
        EDITOR_RECIPE, EditorInlineRevealPlan, EditorLinePlan, EditorPartPlan, EditorRecipePlan,
        EditorSemanticRangePlan,
    },
    plan::ScenePlan,
};

const SECOND: u64 = 1_000_000_000;

pub fn build_plan() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("effect-succeed", 14 * SECOND);
    let editor = scene.actor("editor", EDITOR_RECIPE, editor_recipe())?;
    let constructor = scene.continuous(&editor, "constructor", 0.0);
    let value = scene.continuous(&editor, "value", 0.0);
    let annotation = scene.continuous(&editor, "annotation", 0.0);
    let error = scene.continuous(&editor, "error", 0.0);
    let requirements = scene.continuous(&editor, "requirements", 0.0);

    scene.presentation_step("initial", "Start with a value", 0, 0);
    for (id, title, at, channels, target) in [
        (
            "constructor",
            "Describe a successful computation",
            1,
            vec![&constructor],
            1.0,
        ),
        (
            "value",
            "Give it the string \"pismire\"",
            3,
            vec![&value],
            1.0,
        ),
        (
            "annotation",
            "It succeeds with a string",
            5,
            vec![&annotation],
            1.0,
        ),
        ("error", "It can never fail", 7, vec![&error], 1.0),
        (
            "requirements",
            "It has no requirements",
            9,
            vec![&requirements],
            1.0,
        ),
        (
            "elide",
            "Leave out the trailing nevers",
            11,
            vec![&error, &requirements],
            0.0,
        ),
    ] {
        let start = at * SECOND;
        let hold = start + 1_500_000_000;
        for channel in channels {
            // Published inline motion: 0.4-second visual duration, no bounce.
            scene.spring(channel, start, target, 0.4, 0.0);
        }
        scene.cue(id, start, hold);
        scene.presentation_step(id, title, start, hold);
    }
    Ok(scene.finish()?)
}

fn editor_recipe() -> EditorRecipePlan {
    use SyntaxStyle::{Keyword, Plain, Rgb, String as StringStyle, Type};

    let lines = vec![
        line(
            "import",
            vec![
                part("keyword", "import", Keyword),
                part("effect", " { Effect } ", Plain),
                part("from", "from", Keyword),
                part("module", " \"effect\"", StringStyle),
            ],
        ),
        line("blank", vec![]),
        line(
            "magicWord",
            vec![
                part("const", "const", Keyword),
                part("name", " magicWord", Plain),
                tokens(
                    "type-open",
                    &[
                        (": ", Plain),
                        ("Effect", Plain),
                        (".", Plain),
                        ("Effect", Type),
                        ("<", Plain),
                        ("string", Keyword),
                    ],
                ),
                tokens("error", &[(", ", Plain), ("never", Keyword)]),
                tokens("requirements", &[(", ", Plain), ("never", Keyword)]),
                part("type-close", ">", Plain),
                part("equals", " =", Plain),
            ],
        ),
        line(
            "value",
            vec![
                part("indent", "  ", Plain),
                tokens(
                    "constructor",
                    &[
                        ("Effect", Plain),
                        (".", Plain),
                        ("succeed", Rgb(250, 204, 130)),
                        ("(", Plain),
                    ],
                ),
                part("placeholder", "...", Plain),
                part("pismire", "\"pismire\"", StringStyle),
                part("close", ")", Plain),
            ],
        ),
    ];
    let ids = lines.iter().map(|line| line.id.clone()).collect::<Vec<_>>();
    EditorRecipePlan {
        file_name: "effect-succeed.ts".into(),
        lines,
        initial_line_ids: ids.clone(),
        final_line_ids: ids,
        snapshots: Vec::new(),
        line_height: 44.0,
        entering_offset_x: 0.0,
        focus_line_id: "magicWord".into(),
        focus_height: 88.0,
        inline_reveal: reveal("value", "constructor", "constructor", false),
        additional_inline_reveals: vec![
            reveal("value", "close", "constructor", false),
            reveal("value", "placeholder", "value", true),
            reveal("value", "pismire", "value", false),
            reveal("magicWord", "type-open", "annotation", false),
            reveal("magicWord", "type-close", "annotation", false),
            reveal("magicWord", "error", "error", false),
            reveal("magicWord", "requirements", "requirements", false),
        ],
    }
}

fn part(id: &str, text: &str, style: SyntaxStyle) -> EditorPartPlan {
    tokens(id, &[(text, style)])
}

fn tokens(id: &str, spans: &[(&str, SyntaxStyle)]) -> EditorPartPlan {
    EditorPartPlan {
        id: id.into(),
        spans: spans
            .iter()
            .map(|(text, style)| StyledSpan::new(*text, *style))
            .collect(),
    }
}

fn line(id: &str, parts: Vec<EditorPartPlan>) -> EditorLinePlan {
    let semantic_ranges = parts
        .iter()
        .map(|part| EditorSemanticRangePlan {
            id: part.id.clone(),
            first_part_id: part.id.clone(),
            last_part_id: part.id.clone(),
        })
        .collect();
    EditorLinePlan {
        id: id.into(),
        parts,
        semantic_ranges,
    }
}

fn reveal(line: &str, range: &str, channel: &str, reversed: bool) -> EditorInlineRevealPlan {
    EditorInlineRevealPlan {
        line_id: line.into(),
        range_id: range.into(),
        channel: Some(channel.into()),
        reversed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kinograph::{
        code::TransitionProgress,
        timeline::{PropertyId, SpringProfile, TimedEvent, Timeline},
    };

    #[test]
    fn demo_preserves_lines_and_keeps_common_text_outside_changing_ranges() {
        let recipe = editor_recipe();
        assert_eq!(recipe.initial_line_ids, recipe.final_line_ids);
        let transition = recipe.transition().unwrap();
        let lines = transition.sample(TransitionProgress {
            layout: 1.0,
            content: 1.0,
        });
        assert_eq!(lines.len(), 4);
        for reveal in
            std::iter::once(&recipe.inline_reveal).chain(&recipe.additional_inline_reveals)
        {
            assert!(!["const", "name", "equals", "indent"].contains(&reveal.range_id.as_str()));
        }
    }

    #[test]
    fn punctuation_and_namespace_are_not_colored_as_a_function_or_type() {
        let recipe = editor_recipe();
        let constructor = &recipe.lines[3].parts[1];
        assert_eq!(constructor.id, "constructor");
        assert_eq!(
            constructor
                .spans
                .iter()
                .map(|span| span.text.as_str())
                .collect::<Vec<_>>(),
            ["Effect", ".", "succeed", "("]
        );
        assert!(matches!(constructor.spans[0].style, SyntaxStyle::Plain));
        assert!(matches!(constructor.spans[2].style, SyntaxStyle::Rgb(..)));
        assert!(matches!(constructor.spans[3].style, SyntaxStyle::Plain));
    }

    #[test]
    fn steps_match_the_published_slot_reveals_in_any_sampling_order() {
        let plan = build_plan().unwrap();
        assert_eq!(plan.presentation_steps.len(), 7);
        let channels = [
            "constructor",
            "value",
            "annotation",
            "error",
            "requirements",
        ];
        let expected = [
            [0., 0., 0., 0., 0.],
            [1., 0., 0., 0., 0.],
            [1., 1., 0., 0., 0.],
            [1., 1., 1., 0., 0.],
            [1., 1., 1., 1., 0.],
            [1., 1., 1., 1., 1.],
            [1., 1., 1., 0., 0.],
        ];
        for (index, channel) in plan.continuous_channels.iter().enumerate() {
            assert_eq!(channel.property, channels[index]);
            let id = PropertyId::new(&channel.id);
            let events = channel.events.iter().map(|event| {
                let kinograph::plan::TrackEventPlan::Spring {
                    at_nanos,
                    target: kinograph::plan::ScalarPlan::Literal(target),
                    response_seconds,
                    damping_ratio,
                    position_threshold,
                    velocity_threshold,
                } = event
                else {
                    panic!("expected literal spring");
                };
                assert_eq!(*response_seconds, 0.4 * 1.2);
                assert_eq!(*damping_ratio, 1.0);
                TimedEvent::spring(
                    *at_nanos as f64 / SECOND as f64,
                    id.clone(),
                    *target,
                    SpringProfile::new(
                        *response_seconds,
                        *damping_ratio,
                        *position_threshold,
                        *velocity_threshold,
                    ),
                )
            });
            let timeline = Timeline::compile_events([(id.clone(), 0.0)], events, 14.0).unwrap();
            for step_index in [6, 0, 4, 1, 5, 3, 2, 6] {
                let state = timeline
                    .sample_at(
                        &id,
                        plan.presentation_steps[step_index].hold_nanos as f64 / SECOND as f64,
                    )
                    .unwrap();
                assert_eq!(state.position, expected[step_index][index]);
                assert_eq!(state.velocity, 0.0);
            }
        }
    }

    #[test]
    fn held_text_matches_the_source_slots_after_display_reflow() {
        let recipe = editor_recipe();
        let mut visible = std::collections::HashMap::from([
            ("constructor", false),
            ("value", false),
            ("annotation", false),
            ("error", false),
            ("requirements", false),
        ]);
        let cases = [
            (None, "const magicWord =\n  ..."),
            (
                Some(("constructor", true)),
                "const magicWord =\n  Effect.succeed(...)",
            ),
            (
                Some(("value", true)),
                "const magicWord =\n  Effect.succeed(\"pismire\")",
            ),
            (
                Some(("annotation", true)),
                "const magicWord: Effect.Effect<string> =\n  Effect.succeed(\"pismire\")",
            ),
            (
                Some(("error", true)),
                "const magicWord: Effect.Effect<string, never> =\n  Effect.succeed(\"pismire\")",
            ),
            (
                Some(("requirements", true)),
                "const magicWord: Effect.Effect<string, never, never> =\n  Effect.succeed(\"pismire\")",
            ),
        ];
        for (change, expected) in cases {
            if let Some((channel, value)) = change {
                visible.insert(channel, value);
            }
            let actual = recipe
                .lines
                .iter()
                .skip(2)
                .map(|line| {
                    line.parts
                        .iter()
                        .filter(|part| {
                            std::iter::once(&recipe.inline_reveal)
                                .chain(&recipe.additional_inline_reveals)
                                .find(|reveal| {
                                    reveal.line_id == line.id && reveal.range_id == part.id
                                })
                                .is_none_or(|reveal| visible[reveal.channel()] != reveal.reversed)
                        })
                        .flat_map(|part| &part.spans)
                        .map(|span| span.text.as_str())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(actual, expected);
        }
    }
}
