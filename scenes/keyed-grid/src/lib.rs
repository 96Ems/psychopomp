//! The first procedural 3D diagram: finite products and reassociation.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, SECOND},
    grid::{
        GRID_RECIPE, GridArrangement, GridAxisPlan, GridCellLabelPlan, GridEventPlan,
        GridRecipePlan, GridSnapshotPlan,
    },
    plan::{DeckPlan, ScenePlan, SlidePlan},
};
use serde_json::json;

mod styles;
pub use styles::{build_plain_table_slide, build_style_deck};

struct Step {
    title: &'static str,
    equation: &'static str,
    snapshot: GridSnapshotPlan,
}

fn step(
    title: &'static str,
    equation: &'static str,
    visible: [usize; 3],
    arrangement: GridArrangement,
    focus_slice: Option<usize>,
) -> Step {
    Step {
        title,
        equation,
        snapshot: GridSnapshotPlan {
            visible,
            arrangement,
            focus_slice,
        },
    }
}

fn build_growth() -> Result<ScenePlan> {
    use GridArrangement::*;
    slide(
        "growing-grid",
        "Counting possibilities",
        &[
            step("One choice: a white rook", "1", [1, 1, 1], Table, None),
            step(
                "Choose a piece: rook, knight, or bishop",
                "3 pieces",
                [3, 1, 1],
                Table,
                None,
            ),
            step(
                "Each piece can belong to either side",
                "3 × 2 = 6",
                [3, 2, 1],
                Table,
                None,
            ),
            step(
                "Rotate the same grid: its depth was there all along",
                "3 × 2 = 6",
                [3, 2, 1],
                Layers,
                None,
            ),
            step(
                "Add another board behind the first",
                "3 × 2 × 2 = 12",
                [3, 2, 2],
                Layers,
                None,
            ),
            step(
                "Three pieces, two sides, four boards",
                "3 × 2 × 4 = 24",
                [3, 2, 4],
                Layers,
                None,
            ),
            step(
                "Straight-on again: depth is hidden, not removed",
                "3 × 2 × 4 = 24",
                [3, 2, 4],
                Table,
                None,
            ),
            step(
                "Isolate board II: the same six piece-and-side choices",
                "3 × 2 × 4 = 24",
                [3, 2, 4],
                Layers,
                Some(1),
            ),
            step(
                "Every cell is one piece, one side, and one board",
                "3 × 2 × 4 = 24",
                [3, 2, 4],
                Layers,
                None,
            ),
        ],
    )
}

pub fn build_deck() -> Result<DeckPlan> {
    use GridArrangement::*;
    let growth = build_growth()?;
    let isomorphism = slide(
        "grid-isomorphism",
        "Same values. Different grouping.",
        &[
            step(
                "One connected grid; a different way to group its values",
                "3 × 2 × 4 = 24",
                [3, 2, 4],
                Layers,
                None,
            ),
            step(
                "Group by board: four tables of piece × side",
                "(A × B) × C",
                [3, 2, 4],
                LeftAssociated,
                None,
            ),
            step(
                "Group by piece: three tables of side × board",
                "A × (B × C)",
                [3, 2, 4],
                RightAssociated,
                None,
            ),
            step(
                "Go back: every tuple returns to its original group",
                "(A × B) × C",
                [3, 2, 4],
                LeftAssociated,
                None,
            ),
            step(
                "Nothing created. Nothing lost. 24 values throughout.",
                "((a, b), c)  ↔  (a, (b, c))",
                [3, 2, 4],
                Layers,
                None,
            ),
        ],
    )?;
    let deck = DeckPlan {
        version: 1,
        id: "keyed-grid".into(),
        slides: vec![
            SlidePlan {
                title: "Growing a product".into(),
                plan: growth,
            },
            SlidePlan {
                title: "An isomorphism".into(),
                plan: isomorphism,
            },
        ],
    };
    deck.validate()?;
    Ok(deck)
}

fn slide(id: &str, title: &str, steps: &[Step]) -> Result<ScenePlan> {
    let duration = steps.len() as u64 * 3 * SECOND;
    let mut scene = PlanBuilder::new(id, duration);
    let recipe = GridRecipePlan {
        style: None,
        axes: std::array::from_fn(|i| GridAxisPlan {
            name: ["Piece", "Side", "Board"][i].into(),
            values: [
                vec!["Rook", "Knight", "Bishop"],
                vec!["White", "Black"],
                vec!["I", "II", "III", "IV"],
            ][i]
                .iter()
                .map(|v| v.to_string())
                .collect(),
        }),
        labels: (0..4)
            .flat_map(|c| {
                (0..2).flat_map(move |b| {
                    (0..3).map(move |a| GridCellLabelPlan {
                        indices: [a, b, c],
                        primary: [["♖", "♘", "♗"], ["♜", "♞", "♝"]][b][a].into(),
                        secondary: format!(
                            "{} · {}",
                            ["White", "Black"][b],
                            ["I", "II", "III", "IV"][c]
                        ),
                    })
                })
            })
            .collect(),
        initial: steps[0].snapshot.clone(),
        events: steps
            .iter()
            .enumerate()
            .skip(1)
            .map(|(index, step)| GridEventPlan {
                at_nanos: index as u64 * 3 * SECOND,
                snapshot: step.snapshot.clone(),
            })
            .collect(),
    };
    recipe.validate(duration)?;
    scene.actor("grid", GRID_RECIPE, recipe)?;
    scene.actor("eyebrow", "text", json!({"text":"FUNCTIONAL DATA MODELING", "center":[960,54], "fontSize":18, "color":[133,148,173]}))?;
    scene.actor(
        "title",
        "text",
        json!({"text":title, "center":[960,112], "fontSize":44, "color":[238,239,244]}),
    )?;
    scene.actor("legend", "text", json!({"text":"A: piece     B: side     C: board", "center":[960,168], "fontSize":22, "color":[157,168,189]}))?;
    // Each repeated equation is one retained actor, not a fresh copy per step.
    let mut equations = Vec::new();
    for step in steps {
        if !equations.contains(&step.equation) {
            equations.push(step.equation);
        }
    }
    for (index, equation) in equations.iter().enumerate() {
        caption(
            &mut scene,
            &format!("equation-{index}"),
            equation,
            224.,
            32.,
            steps,
            |step| step.equation == *equation,
        )?;
    }
    for (index, step) in steps.iter().enumerate() {
        caption(
            &mut scene,
            &format!("caption-{index}"),
            step.title,
            942.,
            26.,
            steps,
            |s| s.title == step.title,
        )?;
        let start = index as u64 * 3 * SECOND;
        scene.presentation_step(
            format!("step-{index}"),
            step.title,
            start,
            if index == 0 { 0 } else { start + 2 * SECOND },
        );
        scene.cue(format!("step-{index}"), start, start + 3 * SECOND);
    }
    scene.actor("hint", "text", json!({"text":"← / →  step     ⌘← / ⌘→  slide     R  replay     C  line color     M  reduced motion", "center":[960,1020], "fontSize":16, "color":[108,120,144]}))?;
    Ok(scene.finish()?)
}

fn caption(
    scene: &mut PlanBuilder,
    id: &str,
    text: &str,
    y: f32,
    size: f32,
    steps: &[Step],
    active: impl Fn(&Step) -> bool,
) -> Result<()> {
    let actor = scene.actor(
        id,
        "text",
        json!({"text":text, "center":[960,y], "fontSize":size, "color":[220,227,240],
        "verticalMask":{"top":y-30.,"bottom":y+30.,"fade":10.}}),
    )?;
    let opacity = scene.continuous(&actor, "opacity", if active(&steps[0]) { 1. } else { 0. });
    let position = scene.continuous(&actor, "y", if active(&steps[0]) { y } else { y + 45. });
    let mut current = active(&steps[0]);
    for (index, step) in steps.iter().enumerate().skip(1) {
        let next = active(step);
        if next != current {
            let at = index as u64 * 3 * SECOND;
            scene.spring(&opacity, at, if next { 1. } else { 0. }, 0.3, 0.);
            scene.spring(&position, at, if next { y } else { y - 45. }, 0.4, 0.);
        }
        current = next;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_scenes_match_their_deck_entries() {
        let deck = build_deck().unwrap();
        let styles = build_style_deck().unwrap();
        let growth = build_growth().unwrap().to_json_pretty().unwrap();
        assert_eq!(growth, deck.slides[0].plan.to_json_pretty().unwrap());
        assert_eq!(growth, styles.slides[3].plan.to_json_pretty().unwrap());
        assert_eq!(
            serde_json::to_value(build_plain_table_slide().unwrap()).unwrap(),
            serde_json::to_value(&styles.slides[0]).unwrap()
        );
    }

    #[test]
    fn deterministic_deck_and_constant_isomorphism_cardinality() {
        let deck = build_deck().unwrap();
        assert_eq!(
            serde_json::to_string(&deck).unwrap(),
            serde_json::to_string(&build_deck().unwrap()).unwrap()
        );
        let recipe: GridRecipePlan =
            serde_json::from_value(deck.slides[1].plan.actors[0].data.clone()).unwrap();
        assert_eq!(recipe.cells().unwrap().len(), 24);
        for snapshot in
            std::iter::once(&recipe.initial).chain(recipe.events.iter().map(|e| &e.snapshot))
        {
            assert_eq!(snapshot.visible.into_iter().product::<usize>(), 24);
        }
    }
}
