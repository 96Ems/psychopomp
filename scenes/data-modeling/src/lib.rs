//! A compact native adaptation of the original Scala functional-data-modeling
//! talk. Finite tokens, explicit pairings, and authored code identity—not a
//! generic set enumerator, graph engine, or live joystick simulation.
mod illegal_states;
mod stage;

use anyhow::Result;
use psychopomp::{
    grid::{
        GRID_RECIPE, GridArrangement, GridAxisPlan, GridCellLabelPlan, GridEventPlan,
        GridRecipePlan, GridSnapshotPlan,
    },
    plan::{DeckPlan, ScenePlan, SlidePlan},
};
use stage::*;

const DIRECTIONS: [&str; 5] = ["Up", "Down", "Left", "Right", "Center"];
const SYMBOLS: [&str; 5] = ["↑", "↓", "←", "→", "●"];

pub fn build_deck() -> Result<DeckPlan> {
    let deck = DeckPlan {
        version: 1,
        id: "data-modeling".into(),
        slides: [
            ("Types and cardinality", cardinality()?),
            ("Small, huge, unbounded", sizes()?),
            ("Same information, different names", correspondence()?),
            ("Does the type fit?", joystick()?),
            ("OR adds", alternatives()?),
            ("AND multiplies", product()?),
            (
                "Make illegal states unrepresentable",
                illegal_states::build()?,
            ),
        ]
        .into_iter()
        .map(|(title, plan)| SlidePlan {
            title: title.into(),
            plan,
        })
        .collect(),
    };
    deck.validate()?;
    Ok(deck)
}

fn cardinality() -> Result<ScenePlan> {
    let mut s = Stage::new(
        "types-cardinality",
        "A type is a set of possible values",
        &[
            "Start with a type you already know.",
            "true is one possible value.",
            "false is the other. That is the complete set.",
            "Boolean has exactly two values.",
            "Cardinality is the size of that set.",
        ],
    )?;
    s.text("type-name", "Boolean", [960., 300.], 68., ORANGE)?;
    for (i, label) in ["true", "false"].iter().enumerate() {
        let actor = s.tile(
            &format!("value-{label}"),
            label,
            "",
            [780. + i as f32 * 360., 480.],
            [280., 124.],
        )?;
        s.show(&actor, i + 1, 5);
    }
    let count = s.text("cardinality", "|Boolean| = 2", [960., 680.], 56., INK)?;
    s.show(&count, 3, 5);
    let definition = s.text(
        "definition",
        "cardinality  =  number of possible values",
        [960., 795.],
        30.,
        MUTED,
    )?;
    s.show(&definition, 4, 5);
    s.finish()
}

fn sizes() -> Result<ScenePlan> {
    let mut s = Stage::new(
        "type-sizes",
        "Small. Huge. Unbounded.",
        &[
            "A Boolean is small enough to enumerate completely.",
            "Scala Int is signed 32-bit: huge, but still finite.",
            "The abstract String model has no fixed maximum length.",
        ],
    )?;
    for (i, (name, examples, count)) in [
        ("Boolean", "true   false", "2"),
        ("Int", "−2,147,483,648  …  0  …  2,147,483,647", "2³²"),
        ("String", "\"\"   \"A\"   \"AA\"   \"AAA\"   …", "∞"),
    ]
    .iter()
    .enumerate()
    {
        let y = 340. + i as f32 * 180.;
        for (part, text, x, font, color) in [
            ("name", *name, 345., 40., ORANGE),
            ("examples", *examples, 995., 28., INK),
            ("count", *count, 1620., 58., INK),
        ] {
            let actor = s.text(&format!("{name}-{part}"), text, [x, y], font, color)?;
            s.show(&actor, i, 3);
        }
    }
    let note = s.text(
        "model-limit",
        "∞ describes arbitrary finite strings; real runtimes impose size limits.",
        [960., 835.],
        22.,
        MUTED,
    )?;
    s.show(&note, 2, 3);
    s.finish()
}

fn correspondence() -> Result<ScenePlan> {
    let mut s = Stage::new(
        "boolean-toggle",
        "Different names. Same information.",
        &[
            "Boolean has two values.",
            "Toggle also has two values.",
            "Pair every value with exactly one partner.",
            "Convert true to On.",
            "Convert back: you recover the original value.",
        ],
    )?;
    for (side, x, name) in [(0, 590., "Boolean"), (1, 1330., "Toggle")] {
        let title = s.text(&format!("type-{side}"), name, [x, 285.], 48., ORANGE)?;
        s.show(&title, side, 5);
        let count = s.text(&format!("count-{side}"), "2 values", [x, 755.], 28., MUTED)?;
        s.show(&count, side, 5);
        for (row, label) in if side == 0 {
            ["true", "false"]
        } else {
            ["On", "Off"]
        }
        .iter()
        .enumerate()
        {
            let tile = s.tile(
                &format!("{name}-{label}"),
                label,
                "",
                [x, 435. + row as f32 * 180.],
                [280., 110.],
            )?;
            s.show(&tile, side, 5);
            s.track(
                &tile,
                "emphasis",
                if row == 0 {
                    &[0., 0., 0., 1., 1.]
                } else {
                    &[0.; 5]
                },
            );
        }
    }
    for row in 0..2 {
        let link = s.text(
            &format!("pair-{row}"),
            "←──────────────────────→",
            [960., 435. + row as f32 * 180.],
            30.,
            MUTED,
        )?;
        s.show(&link, 2, 5);
    }
    let marker = s.text("round-trip", "●", [775., 435.], 28., ORANGE)?;
    s.show(&marker, 2, 5);
    s.track(&marker, "x", &[775., 775., 775., 1145., 775.]);
    let law = s.text(
        "round-trip-law",
        "fromToggle(toToggle(value)) == value",
        [960., 845.],
        28.,
        INK,
    )?;
    s.show(&law, 4, 5);
    s.finish()
}

fn joystick() -> Result<ScenePlan> {
    let mut s = Stage::new(
        "joystick-fit",
        "Does your type fit the domain?",
        &[
            "The joystick has five meaningful states.",
            "Boolean cannot distinguish all five states.",
            "String names all five, but also admits unrelated values.",
            "Centaur and Banana Pudding are not joystick states.",
            "A five-case Joystick type fits exactly.",
        ],
    )?;
    s.text(
        "domain-label",
        "THE DOMAIN · 5 STATES",
        [960., 225.],
        22.,
        MUTED,
    )?;
    for (i, direction) in DIRECTIONS.iter().enumerate() {
        let x = 360. + i as f32 * 300.;
        s.tile(
            &format!("domain-{direction}"),
            SYMBOLS[i],
            direction,
            [x, 310.],
            [210., 116.],
        )?;
        let y = 485. + i as f32 * 57.;
        // Each candidate has its own semantic role. Retain earlier candidates
        // for comparison instead of crossfading competing labels in one slot.
        let string = s.tile(
            &format!("string-{direction}"),
            &format!("\"{direction}\""),
            "",
            [960., y],
            [330., 46.],
        )?;
        s.show(&string, 2, 5);
        let value = s.tile(
            &format!("representation-{direction}"),
            direction,
            "",
            [1460., y],
            [330., 46.],
        )?;
        s.show(&value, 4, 5);
        s.track(&value, "emphasis", &[0., 0., 0., 0., 1.]);
        if i < 2 {
            let boolean = s.tile(
                &format!("boolean-{i}"),
                ["true", "false"][i],
                "",
                [460., y],
                [330., 46.],
            )?;
            s.show(&boolean, 1, 5);
        } else {
            let missing = s.text(&format!("missing-{i}"), "?", [460., y], 32., RED)?;
            s.show(&missing, 1, 5);
        }
    }
    for (id, text, first, x) in [
        ("bool", "Boolean · 2", 1, 460.),
        ("str", "String · too many", 2, 960.),
        ("enum", "Joystick · 5", 4, 1460.),
    ] {
        let label = s.text(
            &format!("representation-{id}"),
            text,
            [x, 415.],
            26.,
            ORANGE,
        )?;
        s.show(&label, first, 5);
    }
    for (i, text) in ["Centaur", "Banana Pudding"].iter().enumerate() {
        let tile = s.tile(
            &format!("extra-{i}"),
            &format!("\"{text}\""),
            "",
            [960., 770. + i as f32 * 57.],
            [330., 46.],
        )?;
        s.show(&tile, 2, 5);
        let cross = s.text(
            &format!("extra-invalid-{i}"),
            "×",
            [1160., 770. + i as f32 * 57.],
            36.,
            RED,
        )?;
        s.show(&cross, 3, 5);
    }
    let code = s.text(
        "joystick-type",
        "enum Joystick { case Up, Down, Left, Right, Center }",
        [960., 885.],
        25.,
        INK,
    )?;
    s.show(&code, 4, 5);
    s.finish()
}

fn alternatives() -> Result<ScenePlan> {
    let mut s = Stage::new(
        "sum-types",
        "OR adds alternatives",
        &[
            "A Toggle has two alternatives.",
            "A Joystick has five alternatives.",
            "Choose a Toggle OR a Joystick: two plus five.",
            "The constructor tag tells us which alternative we received.",
        ],
    )?;
    s.text("toggle-heading", "Toggle", [390., 300.], 40., ORANGE)?;
    let heading = s.text("joystick-heading", "Joystick", [1335., 300.], 40., ORANGE)?;
    s.show(&heading, 1, 4);
    for (i, label) in ["On", "Off"].iter().enumerate() {
        let t = s.tile(
            &format!("toggle-{label}"),
            label,
            "",
            [285. + i as f32 * 210., 455.],
            [180., 110.],
        )?;
        s.track(&t, "emphasis", &[0., 0., 0., 1.]);
    }
    for (i, label) in DIRECTIONS.iter().enumerate() {
        let t = s.tile(
            &format!("joystick-{label}"),
            label,
            "",
            [975. + i as f32 * 180., 455.],
            [160., 110.],
        )?;
        s.show(&t, 1, 4);
        s.track(&t, "emphasis", &[0., 0., 0., 1.]);
    }
    let plus = s.text("plus", "+", [730., 455.], 64., ORANGE)?;
    s.show(&plus, 2, 4);
    let equation = s.text("sum", "2 + 5 = 7", [960., 650.], 60., INK)?;
    s.show(&equation, 2, 4);
    for (id, text, x) in [
        ("toggle-tag", "IsToggle(toggle)", 390.),
        ("joystick-tag", "IsJoystick(joystick)", 1335.),
    ] {
        let a = s.text(id, text, [x, 560.], 24., MUTED)?;
        s.show(&a, 3, 4);
    }
    let code = s.text(
        "sum-code",
        "ToggleOrJoystick = IsToggle(Toggle) | IsJoystick(Joystick)",
        [960., 805.],
        27.,
        INK,
    )?;
    s.show(&code, 3, 4);
    s.finish()
}

fn product() -> Result<ScenePlan> {
    let mut s = Stage::new(
        "product-types",
        "AND multiplies choices",
        &[
            "Choose On AND one of the five joystick states.",
            "Off can accompany those same five joystick states.",
            "Each cell is one complete pair: two times five.",
            "Ten pairs, not the seven alternatives from OR.",
        ],
    )?;
    let snapshot = |rows| GridSnapshotPlan {
        visible: [5, rows, 1],
        arrangement: GridArrangement::Table,
        focus_slice: None,
    };
    let grid = s.scene.actor(
        "grid",
        GRID_RECIPE,
        GridRecipePlan {
            style: None,
            axes: [
                GridAxisPlan {
                    name: "Joystick".into(),
                    values: DIRECTIONS.map(str::to_owned).to_vec(),
                },
                GridAxisPlan {
                    name: "Toggle".into(),
                    values: vec!["On".into(), "Off".into()],
                },
                GridAxisPlan {
                    name: "Pair".into(),
                    values: vec!["Control".into()],
                },
            ],
            labels: (0..2)
                .flat_map(|row| {
                    (0..5).map(move |col| GridCellLabelPlan {
                        indices: [col, row, 0],
                        primary: SYMBOLS[col].into(),
                        secondary: ["On", "Off"][row].into(),
                    })
                })
                .collect(),
            initial: snapshot(1),
            events: vec![GridEventPlan {
                at_nanos: BEAT,
                snapshot: snapshot(2),
            }],
        },
    )?;
    // This slide never regroups. Frame the table for reading rather than the
    // renderer's conservative fit for every possible reassociated layout.
    s.scene.continuous(&grid, "scale", 1.8);
    let eq = s.text("product-count", "2 × 5 = 10", [960., 240.], 48., INK)?;
    s.show(&eq, 2, 4);
    let code = s.text(
        "pair-code",
        "case class Control(toggle: Toggle, joystick: Joystick)",
        [960., 845.],
        27.,
        INK,
    )?;
    s.show(&code, 3, 4);
    s.finish()
}

#[cfg(test)]
mod tests;
