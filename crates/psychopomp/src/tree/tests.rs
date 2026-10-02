use serde_json::json;

use super::*;
use crate::author::SECOND;

fn value() -> Value {
    json!({
        "id": "demo",
        "actors": [
            {"id": "title", "recipe": "title-card"},
            {"id": "footer", "recipe": "text"}
        ],
        "durationNanos": 3000000000_u64,
        "media": [],
        "odd key": true
    })
}

fn opens(model: &TreeModel, open: &[(&str, f32)]) -> Vec<f32> {
    let mut opens = vec![0.0; model.nodes.len()];
    for (path, value) in open {
        opens[model.find(path).unwrap()] = *value;
    }
    opens
}

fn row(layout: &TreeLayout, model: &TreeModel, path: &str) -> TreeLine {
    let node = model.find(path).unwrap();
    *layout
        .lines
        .iter()
        .find(|line| line.node == node && !line.closing)
        .unwrap()
}

#[test]
fn paths_are_jsonpaths_that_can_name_channels() {
    assert_eq!(key_path("$", "actors"), "$.actors");
    assert_eq!(index_path("$.actors", 0), "$.actors[0]");
    assert_eq!(key_path("$", "odd key"), "$[\"odd\\u0020key\"]");
    assert_eq!(key_path("$", "9lives"), "$[\"9lives\"]");
    let property = node_property("$.actors[0].id", TreeChannel::Highlight);
    assert_eq!(property, "node.$.actors[0].id.highlight");
    assert_eq!(
        TreeChannel::parse(&property),
        Some(("$.actors[0].id", TreeChannel::Highlight))
    );
    assert_eq!(TreeChannel::parse("node.$.actors.wobble"), None);
    assert_eq!(TreeChannel::parse("scroll"), None);
    let model = TreeModel::new(&value(), &[]).unwrap();
    assert!(
        model
            .nodes
            .iter()
            .all(|node| !node.path.chars().any(char::is_whitespace))
    );
}

#[test]
fn the_model_flattens_in_preorder_with_summaries() {
    let model = TreeModel::new(&value(), &[]).unwrap();
    let actors = &model.nodes[model.find("$.actors").unwrap()];
    assert_eq!(actors.label, TreeLabel::Key("actors".into()));
    assert_eq!(actors.summary(), Some(("…]".into(), "2 items".into())));
    assert!(actors.foldable());
    let first = &model.nodes[model.find("$.actors[0]").unwrap()];
    assert_eq!((first.depth, &first.label), (2, &TreeLabel::Index(0)));
    assert_eq!(first.summary().unwrap().1, "2 keys");
    let media = &model.nodes[model.find("$.media").unwrap()];
    assert!(!media.foldable() && media.summary().is_none());
    assert_eq!(scalar_text(&json!("a\"b")), "\"a\\\"b\"");
}

#[test]
fn opening_a_node_opens_room_without_moving_rows_above() {
    let model = TreeModel::new(&value(), &[]).unwrap();
    let closed = model.layout(&opens(&model, &[("$", 1.0)]));
    let rows = ["$", "$.actors", "$.durationNanos", "$.id", "$.media"];
    for open in [0.0, 0.25, 0.5, 0.9, 1.0] {
        let layout = model.layout(&opens(&model, &[("$", 1.0), ("$.actors", open)]));
        let actors = row(&layout, &model, "$.actors");
        assert_eq!(
            actors.y,
            row(&closed, &model, "$.actors").y,
            "rows above stay"
        );
        assert_eq!(row(&layout, &model, "$").y, 0.0);
        // Two children and a closing row: three rows of room at full open.
        let room = open * 3.0;
        for path in &rows[2..] {
            assert_eq!(
                row(&layout, &model, path).y,
                row(&closed, &model, path).y + room,
                "{path} slides by exactly the opened room"
            );
        }
        assert_eq!(layout.height, closed.height + room);
        if open > 0.0 {
            // Children sit at full pitch inside the opened window, and the
            // closing bracket rides its bottom edge.
            let node = model.find("$.actors").unwrap();
            let closing = layout
                .lines
                .iter()
                .find(|line| line.node == node && line.closing)
                .unwrap();
            assert_eq!(closing.y, actors.y + room);
            assert_eq!(closing.clip, [actors.y + 1.0, actors.y + 1.0 + room]);
            if room > 1.0 {
                let child = row(&layout, &model, "$.actors[0]");
                assert_eq!(child.y, actors.y + 1.0);
                assert_eq!(child.clip, [actors.y + 1.0, closing.y]);
                assert_eq!(child.reveal, open);
            }
        }
    }
}

#[test]
fn folding_a_parent_hides_open_descendants_and_reopening_restores_them() {
    let model = TreeModel::new(&value(), &[]).unwrap();
    let open = [("$", 1.0), ("$.actors", 1.0), ("$.actors[0]", 1.0)];
    let full = model.layout(&opens(&model, &open));
    assert_eq!(row(&full, &model, "$.actors[0].recipe").y, 4.0);
    let folded = model.layout(&opens(
        &model,
        &[("$", 1.0), ("$.actors", 0.0), ("$.actors[0]", 1.0)],
    ));
    let hidden = model.find("$.actors[0].id").unwrap();
    assert!(folded.lines.iter().all(|line| line.node != hidden));
    // Halfway through folding, nested rows clip to the outer window.
    let half = model.layout(&opens(
        &model,
        &[("$", 1.0), ("$.actors", 0.5), ("$.actors[0]", 1.0)],
    ));
    let actors = row(&half, &model, "$.actors");
    for line in half.lines.iter().filter(|line| line.y > actors.y) {
        if line.reveal < 1.0 {
            // [0] is open (four rows) and [1] folded, plus the closing row.
            assert!(line.clip[1] <= actors.y + 1.0 + 0.5 * 6.0);
            assert!(line.clip[0] >= actors.y + 1.0);
        }
    }
    assert_eq!(model.layout(&opens(&model, &open)), full);
}

#[test]
fn layout_is_continuous_in_every_fold() {
    let model = TreeModel::new(&value(), &[]).unwrap();
    let y = |open: f32| {
        let layout = model.layout(&opens(&model, &[("$", 1.0), ("$.actors", open)]));
        row(&layout, &model, "$.media").y
    };
    for step in 0..100 {
        let a = step as f32 / 100.0;
        assert!((y(a + 0.01) - y(a)).abs() <= 0.0301);
    }
}

#[test]
fn changes_append_scalar_variants_and_reject_structure() {
    let changes = [TreeChangePlan {
        path: "$.durationNanos".into(),
        value: json!(4000000000_u64),
    }];
    let model = TreeModel::new(&value(), &changes).unwrap();
    assert_eq!(
        model.nodes[model.find("$.durationNanos").unwrap()].variants(),
        2
    );
    for (path, replacement) in [
        ("$.actors", json!(1)),
        ("$.id", json!({"a": 1})),
        ("$.missing", json!(1)),
    ] {
        let change = [TreeChangePlan {
            path: path.into(),
            value: replacement,
        }];
        assert!(TreeModel::new(&value(), &change).is_err(), "{path}");
    }
}

#[test]
fn plans_round_trip_and_validate_their_paths() {
    let plan = TreePlan::new([120.0, 80.0], 900.0, value()).expanded(["$", "$.actors"]);
    plan.validate().unwrap();
    let json = serde_json::to_value(&plan).unwrap();
    assert!(json.get("size").is_none() && json.get("indent").is_none());
    assert_eq!(serde_json::from_value::<TreePlan>(json).unwrap(), plan);
    assert!(plan.clone().expanded(["$.nope"]).validate().is_err());
    assert!(plan.clone().expanded(["$.media"]).validate().is_err());
    assert!(plan.clone().size(200.0).validate().is_err());
}

#[test]
fn the_actor_writes_fold_highlight_value_and_scroll_channels() {
    let mut scene = PlanBuilder::new("tree", 10 * SECOND);
    let plan = TreePlan::new([120.0, 80.0], 900.0, value())
        .expanded(["$"])
        .max_rows(4);
    let mut tree = TreeActor::declare(&mut scene, "plan", plan).unwrap();
    tree.open(&mut scene, "$.actors", SECOND).unwrap();
    tree.open(&mut scene, "$.actors[0]", 2 * SECOND).unwrap();
    // Rows: $, actors, [0], id, recipe, }, [1], ], ... so [0]'s block ends at row 5.
    tree.reveal(&mut scene, "$.actors[0]", 2 * SECOND).unwrap();
    tree.highlight(&mut scene, "$.actors[0].id", 3 * SECOND, 1.0)
        .unwrap();
    tree.set(
        &mut scene,
        "$.durationNanos",
        json!(4000000000_u64),
        4 * SECOND,
    )
    .unwrap();
    tree.close(&mut scene, "$.actors", 5 * SECOND).unwrap();
    tree.reveal(&mut scene, "$", 5 * SECOND).unwrap();
    assert!(tree.open(&mut scene, "$.media", SECOND).is_err());
    assert!(tree.set(&mut scene, "$.actors", json!(1), SECOND).is_err());
    let plan = scene.finish().unwrap();
    let channel = |property: &str| {
        plan.continuous_channels
            .iter()
            .find(|channel| channel.property == property)
            .unwrap_or_else(|| panic!("{property}"))
    };
    assert_eq!(channel("node.$.actors.open").events.len(), 2);
    assert_eq!(channel("node.$.actors[0].id.highlight").events.len(), 2);
    assert_eq!(channel("node.$.durationNanos.value").events.len(), 1);
    // Scrolled to fit [0]'s block, then back to the top once folded.
    assert_eq!(channel("scroll").events.len(), 2);
    let data: TreePlan = serde_json::from_value(plan.actors[0].data.clone()).unwrap();
    assert_eq!(data.changes.len(), 1);
}
