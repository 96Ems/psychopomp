use super::*;
use kinograph::{
    editor::inspect_steps,
    plan::{ScalarPlan, TrackEventPlan},
    value::{VALUE_TOKEN_RECIPE, ValueTokenPlan},
};
use serde_json::json;

#[test]
fn deck_is_deterministic_native_and_has_valid_concrete_recipes() {
    let deck = build_deck().unwrap();
    assert_eq!(deck.slides.len(), 7);
    let json = serde_json::to_string(&deck).unwrap();
    assert_eq!(json, serde_json::to_string(&build_deck().unwrap()).unwrap());
    serde_json::from_str::<DeckPlan>(&json)
        .unwrap()
        .validate()
        .unwrap();
    for slide in &deck.slides {
        assert!(slide.plan.state_channels.is_empty());
        assert!(slide.plan.media.is_empty());
        for actor in &slide.plan.actors {
            match actor.recipe.as_str() {
                VALUE_TOKEN_RECIPE => serde_json::from_value::<ValueTokenPlan>(actor.data.clone())
                    .unwrap()
                    .validate()
                    .unwrap(),
                GRID_RECIPE => serde_json::from_value::<GridRecipePlan>(actor.data.clone())
                    .unwrap()
                    .validate(slide.plan.duration_nanos)
                    .unwrap(),
                "editor" => {
                    serde_json::from_value::<kinograph::editor::EditorRecipePlan>(
                        actor.data.clone(),
                    )
                    .unwrap()
                    .transition()
                    .unwrap();
                }
                "text" => {}
                other => panic!("unexpected recipe {other}"),
            }
        }
        // Unchanged destinations must never restart a track.
        for channel in &slide.plan.continuous_channels {
            let ScalarPlan::Literal(mut previous) = channel.initial else {
                panic!("unexpected semantic scalar")
            };
            for event in &channel.events {
                let TrackEventPlan::Spring {
                    target: ScalarPlan::Literal(next),
                    ..
                } = event
                else {
                    panic!("expected continuous spring")
                };
                assert_ne!(previous, *next, "redundant event: {}", channel.id);
                previous = *next;
            }
        }
    }
}

#[test]
fn type_change_retains_the_signature_prefix_suffix_and_row() {
    let p = illegal_states::build().unwrap();
    let report = serde_json::to_value(inspect_steps(&p).unwrap()).unwrap();
    let lines = report["steps"][2]["editors"][0]["lines"]
        .as_array()
        .unwrap();
    let signature = lines.iter().find(|line| line["id"] == "signature").unwrap();
    assert_eq!(
        signature["before"],
        "def getUser(id: Int): (User, Error) = ???"
    );
    assert_eq!(
        signature["after"],
        "def getUser(id: Int): UserOrError = ???"
    );
    assert_eq!(
        signature["changedPartIds"],
        json!(["nullable-pair", "sum-type"])
    );
    assert_eq!(signature["moved"], false);
    assert!(
        report["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|warning| warning["code"] != "unsettled-code-step")
    );
}

#[test]
fn unsettled_code_warning_check_detects_a_deliberately_early_hold() {
    let mut plan = illegal_states::build().unwrap();
    let step = &mut plan.presentation_steps[2];
    step.hold_nanos = step.start_nanos + 100_000_000;
    let report = serde_json::to_value(inspect_steps(&plan).unwrap()).unwrap();
    assert!(
        report["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning["code"] == "unsettled-code-step")
    );
}

#[test]
fn finite_examples_count_values_without_conflating_case_shapes_with_payloads() {
    let count = |p: ScenePlan, prefix: &str| {
        p.actors
            .iter()
            .filter(|a| a.recipe == VALUE_TOKEN_RECIPE && a.id.starts_with(prefix))
            .count()
    };
    assert_eq!(count(cardinality().unwrap(), "value-"), 2);
    assert_eq!(count(joystick().unwrap(), "domain-"), 5);
    assert_eq!(count(alternatives().unwrap(), "toggle-"), 2);
    assert_eq!(count(alternatives().unwrap(), "joystick-"), 5);
    let p = product().unwrap();
    let grid: GridRecipePlan = serde_json::from_value(
        p.actors
            .iter()
            .find(|a| a.recipe == GRID_RECIPE)
            .unwrap()
            .data
            .clone(),
    )
    .unwrap();
    assert_eq!(grid.cells().unwrap().len(), 10);
    let p = illegal_states::build().unwrap();
    for id in ["success", "failure"] {
        assert!(
            !p.continuous_channels
                .iter()
                .any(|c| c.actor_id == id && c.property == "opacity"),
            "valid payload identity must never disappear"
        );
    }
}
