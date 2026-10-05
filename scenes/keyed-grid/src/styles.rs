//! Same grid recipe, two layouts and configurable paint. Ordinary records are
//! authored here as a fixed row/column catalog; this is not a sorting/editing API.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, SECOND},
    component_prototype::{Font, TYPESET, TextPart, TypesetPlan},
    grid::{
        GRID_RECIPE, GridAlignment, GridArrangement, GridAxisPlan, GridCellLabelPlan,
        GridEventPlan, GridFillPlan, GridRecipePlan, GridRules, GridSnapshotPlan, GridStylePlan,
    },
    plan::{DeckPlan, ScenePlan, SlidePlan},
};

/// The plain-table slide shared by the grid and slideshow showrooms.
pub fn build_plain_table_slide() -> Result<SlidePlan> {
    Ok(SlidePlan {
        title: "Plain table · no fill, full dividers".into(),
        plan: table(
            "plain-table",
            "Just a table.",
            GridFillPlan::None,
            GridRules::Grid,
        )?,
    })
}

pub fn build_style_deck() -> Result<DeckPlan> {
    let original = super::build_growth()?;
    let mut unfilled = original.clone();
    unfilled.id = "unfilled-volume".into();
    let actor = unfilled
        .actors
        .iter_mut()
        .find(|a| a.recipe == GRID_RECIPE)
        .unwrap();
    let mut recipe: GridRecipePlan = serde_json::from_value(actor.data.clone())?;
    recipe.style = Some(GridStylePlan {
        fill: GridFillPlan::None,
        ..Default::default()
    });
    actor.data = serde_json::to_value(recipe)?;
    let deck = DeckPlan {
        version: 1,
        id: "grid-styles".into(),
        slides: vec![
            build_plain_table_slide()?,
            SlidePlan {
                title: "Banded table · optional row fill".into(),
                plan: table(
                    "banded-table",
                    "A little structure.",
                    GridFillPlan::Banded {
                        color: [31, 36, 47],
                    },
                    GridRules::Rows,
                )?,
            },
            SlidePlan {
                title: "Unfilled volume · still opaque".into(),
                plan: unfilled,
            },
            SlidePlan {
                title: "Original volume · unchanged".into(),
                plan: original,
            },
        ],
    };
    deck.validate()?;
    Ok(deck)
}

fn text(
    scene: &mut PlanBuilder,
    id: &str,
    text: &str,
    origin: [f32; 2],
    font_size: f32,
    color: [u8; 3],
) -> Result<()> {
    scene.actor(
        id,
        TYPESET,
        TypesetPlan {
            origin,
            font: Font::Sans,
            font_size,
            parts: vec![TextPart {
                id: "text".into(),
                text: text.into(),
                color,
                spans: vec![],
            }],
            visible: vec!["text".into()],
            events: vec![],
        },
    )?;
    Ok(())
}

fn table(id: &str, title: &str, fill: GridFillPlan, rules: GridRules) -> Result<ScenePlan> {
    let steps = [
        ("One column and one row", [1, 1, 1], GridArrangement::Table),
        (
            "Reveal the other columns",
            [3, 1, 1],
            GridArrangement::Table,
        ),
        ("Add the next row", [3, 2, 1], GridArrangement::Table),
        (
            "Complete the table; retained rows do not move",
            [3, 4, 1],
            GridArrangement::Table,
        ),
        (
            "The same table viewed in 3D",
            [3, 4, 1],
            GridArrangement::Layers,
        ),
        (
            "Return to the plain table",
            [3, 4, 1],
            GridArrangement::Table,
        ),
    ];
    let mut scene = PlanBuilder::new(id, steps.len() as u64 * 3 * SECOND);
    let snapshot = |visible, arrangement| GridSnapshotPlan {
        visible,
        arrangement,
        focus_slice: None,
    };
    let mut style = GridStylePlan::plain_table(vec![400., 360., 260.]);
    style.fill = fill;
    style.rules = rules;
    let layout = style.table.as_mut().unwrap();
    layout.headers = vec!["Stage".into(), "Status".into(), "Duration".into()];
    layout.alignments = vec![
        GridAlignment::Left,
        GridAlignment::Left,
        GridAlignment::Right,
    ];
    let rows = [
        ["parse", "complete", "12 ms"],
        ["validate", "running", "—"],
        ["persist", "waiting", "—"],
        ["render", "waiting", "—"],
    ];
    let recipe = GridRecipePlan {
        axes: [
            GridAxisPlan {
                name: "Column".into(),
                values: vec!["stage".into(), "state".into(), "time".into()],
            },
            GridAxisPlan {
                name: "Row".into(),
                values: vec![
                    "parse".into(),
                    "validate".into(),
                    "persist".into(),
                    "render".into(),
                ],
            },
            GridAxisPlan {
                name: "Layer".into(),
                values: vec!["only".into()],
            },
        ],
        labels: rows
            .iter()
            .enumerate()
            .flat_map(|(row, values)| {
                values
                    .iter()
                    .enumerate()
                    .map(move |(col, text)| GridCellLabelPlan {
                        indices: [col, row, 0],
                        primary: (*text).into(),
                        secondary: String::new(),
                    })
            })
            .collect(),
        initial: snapshot(steps[0].1, steps[0].2),
        events: steps
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, (_, visible, arrangement))| GridEventPlan {
                at_nanos: i as u64 * 3 * SECOND,
                snapshot: snapshot(*visible, *arrangement),
            })
            .collect(),
        style: Some(style),
    };
    recipe.validate(steps.len() as u64 * 3 * SECOND)?;
    scene.actor("grid", GRID_RECIPE, recipe)?;
    text(
        &mut scene,
        "title",
        title,
        [450., 145.],
        56.,
        [235, 233, 227],
    )?;
    text(
        &mut scene,
        "description",
        "Fixed headers. Rows reveal into place.",
        [450., 248.],
        23.,
        [143, 150, 165],
    )?;
    text(
        &mut scene,
        "footer",
        "Grid / table layout / configurable fill and rules",
        [450., 910.],
        18.,
        [143, 150, 165],
    )?;
    scene.steps(
        "step",
        steps.iter().map(|step| step.0),
        3 * SECOND,
        2 * SECOND,
    );
    scene.cue_steps(3 * SECOND);
    Ok(scene.finish()?)
}
