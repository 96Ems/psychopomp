use super::*;
use psychopomp::plan::{PresentationStepPlan, StateEventPlan};
use serde_json::json;

fn plan(recipe: &str, data: Value) -> ScenePlan {
    let mut p = ScenePlan::new("preflight", 2_000_000_000);
    p.actors.push(ActorPlan {
        id: "actor".into(),
        recipe: recipe.into(),
        data,
    });
    p.presentation_steps.push(PresentationStepPlan {
        id: "initial".into(),
        title: "Initial".into(),
        start_nanos: 0,
        hold_nanos: 0,
    });
    p
}
fn state(name: &str, initial: Value, value: Value) -> StateChannelPlan {
    StateChannelPlan {
        id: format!("actor.{name}"),
        actor_id: "actor".into(),
        state: name.into(),
        initial,
        events: vec![StateEventPlan {
            at_nanos: 500_000_000,
            value,
        }],
    }
}

#[test]
fn typed_current_states_preserve_unobservable_equal_time_writes_and_raw_keys() {
    let mut p = plan("text", json!({"center":[0,0]}));
    let mut content = state("content", Value::Null, json!("Ready"));
    content.events[0].at_nanos = 0;
    content.events.extend([
        StateEventPlan {
            at_nanos: 500_000_000,
            value: Value::Null,
        },
        StateEventPlan {
            at_nanos: 500_000_000,
            value: json!("After"),
        },
    ]);
    p.state_channels.push(content);
    let input = Plan::new(p).unwrap();
    assert_eq!(input.texts[0].content.sample_at(0.).current, "Ready");
    assert_eq!(input.texts[0].content.sample_at(0.5).current, "After");
    let mut p = plan("title-card", json!({"title":"Title","subtitle":"fallback"}));
    p.state_channels
        .push(state("subtitle", json!(42), json!(43)));
    let input = Plan::new(p.clone()).unwrap();
    let RootPlan::Title(title) = input.root else {
        panic!("title")
    };
    assert!(title.subtitle.sample_at(0.).current.is_none());
    assert!(title.subtitle.sample_at(1.).current.is_none());
    let compiled = super::super::CompiledPlan::compile(p, std::path::Path::new(".")).unwrap();
    assert_ne!(
        compiled.visual_sample_key(0.).unwrap(),
        compiled.visual_sample_key(1.).unwrap()
    );
}

#[test]
fn native_eligibility_rejects_valid_export_only_inputs_and_missing_steps() {
    let mut p = plan("title-card", json!({"title":"Title"}));
    p.state_channels
        .push(state("subtitle", json!("one"), json!("two")));
    assert!(Plan::new(p).unwrap().require_native().is_err());
    let mut p = plan("title-card", json!({"title":"Audio"}));
    p.media.push(psychopomp::plan::MediaPlan {
        id: "audio".into(),
        path: "not-opened.wav".into(),
        kind: MediaKindPlan::Audio,
        role: psychopomp::plan::MediaRolePlan::Layer,
        source_start_nanos: 0,
        source_end_nanos: 1_000_000_000,
        timeline_start_nanos: 0,
        timeline_end_nanos: 1_000_000_000,
        gain_db: 0.,
    });
    assert!(Plan::new(p).unwrap().require_native().is_err());
    let mut p = plan("title-card", json!({"title":"Title"}));
    p.presentation_steps.clear();
    assert!(Plan::new(p).unwrap().require_native().is_err());
    let p = psychopomp_deployment_queue::build_plan().unwrap();
    assert!(Plan::new(p).unwrap().require_native().is_err());
    let p = psychopomp_opencode_session_tool::build_plan().unwrap();
    assert!(Plan::new(p).unwrap().require_native().is_err());
}

#[test]
fn preflight_rejects_font_independent_rich_text_width_failure() {
    let p = plan(
        RICH_TEXT,
        json!({"origin":[0,0],"width":80,"fontSize":120,"markdown":"x"}),
    );
    assert!(
        Plan::new(p)
            .err()
            .unwrap()
            .to_string()
            .contains("too little room")
    );
}

#[test]
fn title_and_text_are_typed_before_resources_but_keep_distinct_state_rules() {
    for data in [json!({}), json!({"title":42}), json!({"title":null})] {
        assert_eq!(
            Plan::new(plan("title-card", data))
                .err()
                .unwrap()
                .to_string(),
            "title-card actor requires string data.title"
        );
    }
    let mut p = plan("title-card", json!({"title":"","subtitle":"static"}));
    p.state_channels
        .push(state("subtitle", json!("live"), Value::Null));
    let checked = Plan::new(p).unwrap();
    assert!(!checked.native());
    let RootPlan::Title(title) = checked.root else {
        panic!("title root")
    };
    assert_eq!(title.title, "");
    assert_eq!(
        title.subtitle.sample_at(0.).current.as_deref(),
        Some("live")
    );
    assert_eq!(title.subtitle.sample_at(1.).current.as_deref(), None);

    let mut p = plan("text", json!({"center":[0.25,1.25],"text":42}));
    p.state_channels
        .push(state("content", json!("initial"), json!("later")));
    let checked = Plan::new(p.clone()).unwrap();
    let text = &checked.texts[0];
    assert_eq!(text.center, [0.25, 1.25]);
    assert_eq!(text.font_size, 28.);
    assert_eq!(text.color, [255; 3]);
    assert_eq!(text.content.sample_at(0.).current, "initial");
    assert_eq!(text.content.sample_at(1.).current, "later");
    p.state_channels[0].events[0].value = Value::Null;
    assert!(
        Plan::new(p).is_err(),
        "invalid string state must not fall back to static text"
    );
    assert!(Plan::new(plan("text", json!({"center":[0,0]}))).is_err());
}

#[test]
fn explicit_invalid_text_fields_do_not_become_defaults() {
    for (field, value) in [
        ("center", json!([1])),
        ("center", json!([1e100, 0])),
        ("fontSize", Value::Null),
        ("fontSize", json!(0)),
        ("color", json!([256, 0, 0])),
        ("color", Value::Null),
        ("verticalMask", Value::Null),
    ] {
        let mut data = json!({"text":"text","center":[0,0]});
        data[field] = value;
        assert!(
            Plan::new(plan("text", data)).is_err(),
            "accepted bad {field}"
        );
    }
}

#[test]
fn pointers_bind_after_parsing_and_reject_dangling_or_duplicate_owners() {
    let mut p = psychopomp_hero::build_plan().unwrap();
    let index = p
        .actors
        .iter()
        .position(|a| a.recipe == POINTER_RECIPE)
        .unwrap();
    let pointer = p.actors.remove(index);
    p.actors.insert(0, pointer.clone());
    Plan::new(p.clone()).unwrap();
    let mut duplicate = pointer;
    duplicate.id = "other-pointer".into();
    p.actors.push(duplicate);
    assert!(
        Plan::new(p.clone())
            .err()
            .unwrap()
            .to_string()
            .contains("more than one pointer")
    );
    p.actors.pop();
    p.actors[0].data["editorId"] = json!("missing");
    assert!(
        Plan::new(p)
            .err()
            .unwrap()
            .to_string()
            .contains("unknown editor")
    );
}

#[test]
fn every_root_pair_is_exclusive_without_opening_media_or_a_gpu() {
    let mut terminal = psychopomp_opencode_session_tool::build_plan().unwrap();
    for media in &mut terminal.media {
        media.path = "preflight-does-not-open-this-file.mp4".into();
    }
    let sources = vec![
        plan("title-card", json!({"title":"Title"})),
        psychopomp_hero::build_plan().unwrap(),
        psychopomp_keyed_grid::build_deck()
            .unwrap()
            .slides
            .remove(0)
            .plan,
        psychopomp_opencode_architecture::build_scene().unwrap(),
        psychopomp_deployment_queue::build_plan().unwrap(),
        terminal,
    ];
    for left in &sources {
        for right in &sources {
            let mut a = Plan::new(left.clone()).unwrap().root;
            let b = Plan::new(right.clone()).unwrap().root;
            let same = a.recipe() == b.recipe();
            let error = put_root(&mut a, b).unwrap_err().to_string();
            assert!(error.contains(if same {
                "at most one"
            } else {
                "exclusive root"
            }));
        }
    }
}

#[test]
fn terminal_bindings_and_selections_fail_before_cache_opening() {
    let good = psychopomp_opencode_session_tool::build_plan().unwrap();
    for mutation in 0..5 {
        let mut p = good.clone();
        let actor = p
            .actors
            .iter_mut()
            .find(|a| a.recipe == TERMINAL_RECORDING_RECIPE)
            .unwrap();
        match mutation {
            0 => actor.data["recordings"] = json!([]),
            1 => actor.data["recordings"][0]["fps"] = json!(0),
            2 => actor.data["recordings"][0]["mediaId"] = json!("missing"),
            3 => p.state_channels.retain(|c| c.state != "recording"),
            _ => {
                p.state_channels
                    .iter_mut()
                    .find(|c| c.state == "recording")
                    .unwrap()
                    .initial = json!("missing")
            }
        }
        assert!(
            Plan::new(p).is_err(),
            "accepted invalid terminal mutation {mutation}"
        );
    }
}
