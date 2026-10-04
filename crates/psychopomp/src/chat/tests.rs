use super::*;
use crate::{
    author::SECOND,
    plan::{ScalarPlan, TrackEventPlan},
};

fn people() -> Vec<ChatPersonPlan> {
    vec![
        ChatPersonPlan::new("dax", "Dax Raad", Tone::Accent),
        ChatPersonPlan::new("bot", "opencode", Tone::Success).badge("APP"),
    ]
}

fn message(id: &str, author: &str, text: &str) -> ChatMessagePlan {
    ChatMessagePlan {
        id: id.into(),
        author: author.into(),
        spans: vec![ChatSpanPlan::plain(text)],
        time: None,
        reactions: Vec::new(),
    }
}

fn plan() -> ChatPlan {
    let mut plan = ChatPlan::new([100.0, 100.0], [900.0, 700.0], people())
        .titled("# opencode", None)
        .composer("Message #opencode");
    plan.messages = vec![
        message("a", "dax", "is the session test flaky?"),
        message("b", "dax", "it failed twice on main"),
        message("c", "bot", "On it."),
    ];
    plan
}

fn metrics(plan: &ChatPlan) -> Vec<ChatMetrics> {
    (0..plan.messages.len())
        .map(|i| ChatMetrics {
            header: if plan.group_start(i) { 30.0 } else { 0.0 },
            body: 31.0,
            reactions: 40.0,
        })
        .collect()
}

fn pose(typing: f32, reveal: f32) -> ChatPose {
    ChatPose {
        typing,
        reveal,
        reactions: 0.0,
    }
}

#[test]
fn chats_round_trip_with_compact_defaults() {
    let mut plan = plan();
    plan.messages[2].spans.push(ChatSpanPlan::code("bun test"));
    plan.messages[2].reactions.push(ChatReactionPlan {
        id: "r0".into(),
        label: "🎉".into(),
        count: 1,
    });
    plan.validate().unwrap();
    let json = serde_json::to_value(&plan).unwrap();
    assert!(json.get("style").is_none() && json.get("textSize").is_none());
    assert_eq!(json["messages"][2]["spans"][1]["code"], true);
    assert!(json["messages"][2]["reactions"][0].get("count").is_none());
    assert_eq!(json["people"][1]["badge"], "APP");
    assert_eq!(serde_json::from_value::<ChatPlan>(json).unwrap(), plan);
    assert_eq!(people()[0].avatar_text(), "DR");
}

#[test]
fn invalid_chats_are_rejected() {
    let mut stranger = plan();
    stranger.messages[0].author = "nobody".into();
    assert!(stranger.validate().is_err());
    let mut repeated = plan();
    repeated.messages[1].id = "a".into();
    assert!(repeated.validate().is_err());
    let mut dotted = plan();
    dotted.messages[0].id = "a.b".into();
    assert!(dotted.validate().is_err());
    assert!(plan().me("nobody").validate().is_err());
    let mut empty = plan();
    empty.people.clear();
    assert!(empty.validate().is_err());
}

#[test]
fn a_run_by_one_author_shows_its_header_once() {
    let plan = plan();
    assert!(plan.group_start(0));
    assert!(!plan.group_start(1));
    assert!(plan.group_start(2));
    let bubbles = plan.clone().style(ChatStyle::Bubbles).me("dax");
    assert!(bubbles.mine(0) && !bubbles.mine(2));
    assert!(!plan.mine(0), "only bubbles sit on the right");
}

#[test]
fn messages_stack_up_from_the_composer_and_push_older_ones_up() {
    let plan = plan();
    let metrics = metrics(&plan);
    let geometry = plan.geometry();
    let bottom = 700.0;
    let before = plan.layout(
        &metrics,
        &[pose(0.0, 1.0), pose(0.0, 1.0), pose(0.0, 0.0)],
        bottom,
    );
    // The newest present message sits on the bottom edge.
    assert_eq!(before[1].top + before[1].room, bottom);
    assert_eq!(before[2].room, 0.0);
    let opening = plan.layout(
        &metrics,
        &[pose(0.0, 1.0), pose(0.0, 1.0), pose(0.0, 0.5)],
        bottom,
    );
    let opened = opening[2].room + geometry.gaps[0] * 0.5;
    for index in 0..2 {
        assert!(
            (before[index].top - opening[index].top - opened).abs() < 1e-3,
            "older messages move up by exactly the new room"
        );
        assert_eq!(before[index].room, opening[index].room);
    }
    let after = plan.layout(&metrics, &[pose(0.0, 1.0); 3], bottom);
    assert_eq!(after[2].room, 30.0 + 31.0);
    assert_eq!(after[2].top + after[2].room, bottom);
}

#[test]
fn a_typing_slot_grows_into_its_message_and_reactions_open_room() {
    let plan = plan();
    let metrics = metrics(&plan);
    let room = |typing, reveal, reactions| {
        plan.layout(
            &metrics,
            &[
                pose(0.0, 1.0),
                pose(0.0, 1.0),
                ChatPose {
                    typing,
                    reveal,
                    reactions,
                },
            ],
            600.0,
        )[2]
        .room
    };
    assert_eq!(room(0.0, 0.0, 0.0), 0.0);
    assert_eq!(room(1.0, 0.0, 0.0), 30.0 + plan.typing_body());
    assert_eq!(
        room(1.0, 1.0, 0.0),
        61.0,
        "typing no longer matters once said"
    );
    assert_eq!(room(0.0, 1.0, 0.0), 61.0);
    let halfway = room(1.0, 0.5, 0.0);
    assert!(halfway > 30.0 + plan.typing_body().min(31.0) && halfway < 61.0 + plan.typing_body());
    assert_eq!(room(0.0, 1.0, 1.0), 101.0);
    assert_eq!(room(0.0, 1.0, 0.5), 81.0);
    // Overshoot never folds a slot inside out.
    assert!(room(0.0, 1.08, 0.0) > 61.0);
}

#[test]
fn typing_dots_rise_in_turn_and_repeat() {
    for index in 0..3 {
        for step in 0..240 {
            let t = step as f32 * 0.01;
            let (lift, ink) = typing_dot(t, index);
            assert!((0.0..=1.0).contains(&lift) && (0.4..=1.0).contains(&ink));
            let (again, _) = typing_dot(t + 1.2, index);
            assert!((lift - again).abs() < 1e-3, "one wave every 1.2 s");
        }
    }
    let peak = |index| {
        (0..120)
            .max_by(|a, b| {
                let at = |s: i32| typing_dot(s as f32 * 0.01, index).0;
                at(*a).total_cmp(&at(*b))
            })
            .unwrap()
    };
    assert!(peak(0) < peak(1) && peak(1) < peak(2), "left to right");
    assert_eq!(typing_dot(-1.0, 0), (0.0, 0.4));
}

#[test]
fn channels_parse_by_message_and_reaction() {
    assert_eq!(
        ChatChannel::parse("message.m3.typing"),
        Some(("m3", ChatChannel::Typing))
    );
    assert_eq!(ChatChannel::parse("message.m3.wobble"), None);
    assert_eq!(parse_reaction("reaction.m3.r0"), Some(("m3", "r0")));
    assert_eq!(parse_reaction("reaction.m3"), None);
}

#[test]
fn typing_then_saying_fills_the_same_slot_and_stops_the_dots() {
    let mut scene = PlanBuilder::new("chat", 12 * SECOND);
    let mut chat = ChatActor::declare(
        &mut scene,
        "chat",
        ChatPlan::new([0.0, 0.0], [900.0, 700.0], people()),
    )
    .unwrap();
    chat.show(&mut scene, 0);
    chat.stamp("10:42 AM");
    let typing = chat.typing(&mut scene, SECOND, "dax").unwrap();
    assert!(
        chat.typing(&mut scene, SECOND, "dax").is_err(),
        "one slot per typist"
    );
    let said = chat
        .say_text(&mut scene, 3 * SECOND, "dax", "is it flaky?")
        .unwrap();
    assert_eq!(typing, said, "the typing slot becomes the message");
    let (reply, done) = chat
        .stream(
            &mut scene,
            4 * SECOND,
            "bot",
            vec![ChatSpanPlan::plain("On it.")],
            30.0,
        )
        .unwrap();
    assert!(done > 4 * SECOND);
    chat.react(&mut scene, 6 * SECOND, &said, "👀", 2).unwrap();
    chat.highlight(&mut scene, &reply, 7 * SECOND, 1.0).unwrap();
    let recipe = chat.plan().clone();
    assert_eq!(recipe.messages.len(), 2);
    assert_eq!(recipe.messages[0].time.as_deref(), Some("10:42 AM"));
    assert_eq!(recipe.messages[0].reactions[0].count, 2);
    let plan = scene.finish().unwrap();
    let events = |property: &str| {
        plan.continuous_channels
            .iter()
            .find(|channel| channel.property == property)
            .unwrap_or_else(|| panic!("{property}"))
            .events
            .clone()
    };
    let wait = events("message.m0.wait");
    assert!(
        matches!(wait.last(), Some(TrackEventPlan::Set { at_nanos, value: ScalarPlan::Literal(v) })
            if *at_nanos == 3 * SECOND + seconds(f64::from(SAY_SECONDS)) && (*v - 2.5).abs() < 1e-6),
        "{wait:?}"
    );
    assert_eq!(events("message.m1.typed").len(), 6);
    assert_eq!(events("reaction.m0.r0").len(), 1);
    // The bot never typed: its message has no dots clock.
    assert!(
        !plan
            .continuous_channels
            .iter()
            .any(|c| c.property == "message.m1.wait")
    );
}
