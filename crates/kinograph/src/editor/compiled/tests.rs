use super::*;

fn fixture() -> EditorRecipePlan {
    serde_json::from_value(serde_json::json!({
        "fileName":"compiled.ts","lines":[
            {"id":"a","parts":[{"id":"prefix","spans":[{"text":"const","style":"keyword"},{"text":" ","style":"plain"}]},{"id":"old","spans":[{"text":"é","style":"plain"}]},{"id":"empty","spans":[]},{"id":"new","spans":[{"text":"Effect","style":"type"},{"text":".succeed(1)","style":"plain"}]}],"semanticRanges":[{"id":"old","firstPartId":"old","lastPartId":"empty"},{"id":"new","firstPartId":"new","lastPartId":"new"}]},
            {"id":"insert","parts":[{"id":"body","spans":[{"text":"insert","style":"plain"}]}]},
            {"id":"b","parts":[{"id":"body","spans":[{"text":"b","style":"plain"}]}]},
            {"id":"never","parts":[{"id":"body","spans":[{"text":"never","style":"plain"}]}]},
            {"id":"exit","parts":[{"id":"body","spans":[{"text":"exit","style":"plain"}]}]}
        ],"initialLineIds":["a","b","exit"],"finalLineIds":["b","insert","a"],"lineHeight":44.25,"enteringOffsetX":93.5,"focusLineId":"a","focusHeight":44.25,
        "inlineReveal":{"lineId":"a","rangeId":"old","channel":"swap","reversed":true},"additionalInlineReveals":[{"lineId":"a","rangeId":"new","channel":"swap"}]
    })).unwrap()
}
fn reference(recipe: &EditorRecipePlan) -> CodeTransition {
    let document = CodeDocument::new(
        recipe
            .lines
            .iter()
            .map(|line| line.code_line().unwrap())
            .collect(),
    )
    .unwrap();
    let final_ids = if recipe.snapshots.is_empty() {
        recipe.final_line_ids.clone()
    } else {
        recipe.lines.iter().map(|l| l.id.clone()).collect()
    };
    CodeTransition::compile(
        &document,
        &CodeSnapshot::new(recipe.initial_line_ids.iter().cloned()),
        &CodeSnapshot::new(final_ids),
        CodeLayout {
            line_height: recipe.line_height,
            entering_offset_x: if recipe.snapshots.is_empty() {
                recipe.entering_offset_x
            } else {
                0.
            },
        },
    )
    .unwrap()
}
fn signature(lines: &[PlacedLine<'_>]) -> Vec<(String, [u32; 4])> {
    lines
        .iter()
        .map(|line| {
            (
                line.line.id.as_str().to_owned(),
                [line.x, line.y, line.opacity, line.blur].map(f32::to_bits),
            )
        })
        .collect()
}
#[test]
fn compiled_legacy_preserves_paint_order_catalog_and_empty_part_identity() {
    let recipe = fixture();
    let editor = recipe.compile().unwrap();
    let reference = reference(&recipe);
    assert_eq!(
        editor.lines().map(|l| l.id.as_str()).collect::<Vec<_>>(),
        ["a", "insert", "b", "never", "exit"]
    );
    assert!(editor.line_endpoints("never").is_none());
    for (layout, content) in [(0., 0.), (0.37, 0.81), (-0.2, 1.2), (1., 1.)] {
        let actual = editor.sample_lines(|p, d| match p {
            "layout" => layout,
            "content" => content,
            _ => d,
        });
        assert_eq!(
            signature(&actual),
            signature(&reference.sample(TransitionProgress { layout, content }))
        );
        let exit = actual
            .iter()
            .find(|l| l.line.id.as_str() == "exit")
            .unwrap();
        assert_eq!(exit.x, -93.5 * content);
        assert_eq!(exit.y, 88.5);
    }
    assert_eq!(editor.inline_reveals()[0].spans, 2..3);
    assert_eq!(editor.inline_reveals()[0].parts, 1..3);
    assert_eq!(editor.inline_reveals()[1].spans, 3..5);
    assert_eq!(
        editor.part_presence(&LineId::new("a"), |_, _| 0.25),
        Some(vec![1., 0.75, 0.75, 0.25])
    );
}
#[test]
fn keyed_sampling_matches_compatibility_overrides_without_fabricating_a_transition() {
    let mut recipe = fixture();
    recipe.final_line_ids = recipe.initial_line_ids.clone();
    recipe.snapshots = vec![
        EditorSnapshotPlan {
            at_nanos: 1_000_000_000,
            line_ids: vec!["never".into(), "a".into(), "b".into()],
        },
        EditorSnapshotPlan {
            at_nanos: 1_000_000_000,
            line_ids: vec!["a".into(), "insert".into(), "b".into()],
        },
        EditorSnapshotPlan {
            at_nanos: 2_000_000_000,
            line_ids: recipe.final_line_ids.clone(),
        },
    ];
    let editor = recipe.compile().unwrap();
    let reference = reference(&recipe);
    let channels = editor.snapshot_channels("editor", 4_000_000_000).unwrap();
    let timeline = crate::plan::compile_channels(
        channels
            .iter()
            .map(|c| (c, crate::timeline::PropertyId::new(&c.property))),
        4_000_000_000,
        |s| match s {
            crate::plan::ScalarPlan::Literal(v) => Ok(*v),
            _ => unreachable!(),
        },
    )
    .unwrap();
    assert_eq!(editor.line_endpoints("b"), Some([[0., 44.25], [0., 88.5]]));
    assert_eq!(
        editor.line_endpoints("never"),
        Some([[0., 132.75], [0., 132.75]])
    );
    for at in [2.05, 0., 1.12, 4., 1.12] {
        let sample = |p: &str, d| match p {
            "layout" => 0.37,
            "content" => 0.81,
            _ => timeline
                .sample_at(&crate::timeline::PropertyId::new(p), at)
                .map_or(d, |s| s.position),
        };
        let actual = editor.sample_lines(sample);
        let mut expected = reference.sample(TransitionProgress {
            layout: 0.37,
            content: 0.81,
        });
        for line in &mut expected {
            line.x = 0.;
            line.y = sample(&format!("line.{}.y", line.line.id.as_str()), line.y);
            line.opacity = sample(
                &format!("line.{}.opacity", line.line.id.as_str()),
                line.opacity,
            )
            .clamp(0., 1.);
            line.blur = (1. - line.opacity) * 4.;
        }
        assert_eq!(signature(&actual), signature(&expected));
    }
    recipe.snapshots[0].at_nanos = 3_000_000_000;
    assert!(recipe.compile().is_ok());
    assert!(recipe.transition().is_ok());
    assert!(recipe.snapshot_channels("editor", 4_000_000_000).is_err());
}
