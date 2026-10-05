use super::*;
use crate::{
    author::SECOND,
    plan::{ScalarPlan, TrackEventPlan},
};

fn output(id: &str, text: &str) -> TerminalLinePlan {
    TerminalLinePlan::Output {
        id: id.into(),
        spans: vec![CaptionSpanPlan::new(text, Tone::Plain)],
    }
}

fn plan(lines: usize, rows: u32) -> TerminalPlan {
    let mut plan = TerminalPlan::new([100.0, 100.0], 1200.0, rows);
    plan.lines = (0..lines)
        .map(|i| output(&format!("l{i}"), &format!("line {i}")))
        .collect();
    plan
}

fn ys(layout: &TerminalLayout) -> Vec<(usize, f32)> {
    layout.rows.iter().map(|row| (row.line, row.y)).collect()
}

#[test]
fn terminals_round_trip_with_compact_defaults() {
    let mut plan = plan(1, 10).titled("zsh");
    plan.lines.push(TerminalLinePlan::Command {
        id: "c".into(),
        text: "ls".into(),
    });
    plan.lines.push(TerminalLinePlan::Task {
        id: "t".into(),
        spans: vec![CaptionSpanPlan::new("building", Tone::Muted)],
        done: Vec::new(),
        mark: Mark::Cross,
    });
    plan.validate().unwrap();
    let json = serde_json::to_value(&plan).unwrap();
    assert!(json.get("size").is_none() && json.get("prompt").is_none());
    assert_eq!(json["lines"][1]["kind"], "command");
    assert_eq!(json["lines"][2]["mark"], "cross");
    assert!(json["lines"][2].get("done").is_none());
    assert_eq!(serde_json::from_value::<TerminalPlan>(json).unwrap(), plan);
}

#[test]
fn invalid_terminals_are_rejected() {
    let mut repeated = plan(2, 4);
    repeated.lines[1] = output("l0", "again");
    assert!(repeated.validate().is_err());
    let mut dotted = plan(1, 4);
    dotted.lines[0] = output("a.b", "x");
    assert!(dotted.validate().is_err());
    let mut newline = plan(1, 4);
    newline.lines[0] = output("l0", "a\nb");
    assert!(newline.validate().is_err());
    assert!(plan(1, 0).validate().is_err());
    assert!(
        serde_json::from_value::<TerminalPlan>(serde_json::json!({
            "origin": [0, 0], "width": 800, "rows": 4, "wobble": true
        }))
        .is_err()
    );
}

#[test]
fn lines_stack_by_their_reveals_and_appear_in_place() {
    let plan = plan(3, 10);
    let layout = plan.layout(&[1.0, 1.0, 0.0], 0.0);
    assert_eq!(ys(&layout), vec![(0, 0.0), (1, 1.0)]);
    assert_eq!(layout.height, 2.0);
    // A half-open line sits below the others; nothing above it moves.
    let opening = plan.layout(&[1.0, 1.0, 0.5], 0.0);
    assert_eq!(ys(&opening), vec![(0, 0.0), (1, 1.0), (2, 2.0)]);
    assert_eq!(opening.rows[2].reveal, 0.5);
}

#[test]
fn a_full_window_slides_older_lines_up_by_exactly_the_new_room() {
    let plan = plan(5, 3);
    let full = plan.layout(&[1.0, 1.0, 1.0, 0.0, 0.0], 0.0);
    assert_eq!(full.top, 0.0);
    let opening = plan.layout(&[1.0, 1.0, 1.0, 0.25, 0.0], 0.0);
    assert_eq!(opening.top, 0.25);
    for (before, after) in full.rows.iter().zip(&opening.rows) {
        assert_eq!(before.line, after.line);
        assert_eq!(after.y, before.y - 0.25, "every row moves by the room");
    }
    // The opening line enters from the bottom edge.
    assert_eq!(opening.rows.last().unwrap().y, 2.75);
    let settled = plan.layout(&[1.0; 5], 0.0);
    assert_eq!(ys(&settled), vec![(2, 0.0), (3, 1.0), (4, 2.0)]);
}

#[test]
fn clearing_lifts_everything_out_through_the_scroll_floor() {
    let plan = plan(4, 6);
    let partway = plan.layout(&[1.0, 1.0, 0.0, 0.0], 0.5);
    assert_eq!(ys(&partway), vec![(0, -0.5), (1, 0.5)]);
    let halfway = plan.layout(&[1.0, 1.0, 0.0, 0.0], 1.0);
    assert_eq!(ys(&halfway), vec![(1, 0.0)]);
    let cleared = plan.layout(&[1.0, 1.0, 0.0, 0.0], 2.0);
    assert!(cleared.rows.is_empty());
    // After a clear the next line starts at the top of the window.
    let next = plan.layout(&[1.0, 1.0, 1.0, 0.0], 2.0);
    assert_eq!(ys(&next), vec![(2, 0.0)]);
    // Sampling is a pure function of the channels.
    assert_eq!(
        plan.layout(&[1.0, 0.3, 0.7, 0.0], 0.4),
        plan.layout(&[1.0, 0.3, 0.7, 0.0], 0.4)
    );
}

#[test]
fn keystrokes_are_deterministic_monotonic_and_pause_between_words() {
    let text = "bun run test --filter session";
    let times = keystrokes(text, SECOND, 18.0, 7);
    assert_eq!(times, keystrokes(text, SECOND, 18.0, 7));
    assert_eq!(times.len(), text.chars().count());
    assert!(times.windows(2).all(|pair| pair[0] < pair[1]));
    let total = (times.last().unwrap() - SECOND) as f64 / 1e9;
    let nominal = text.len() as f64 / 18.0;
    assert!(
        (0.75..1.35).contains(&(total / nominal)),
        "{total} vs {nominal}"
    );
    let gap = |i: usize| times[i] - times[i - 1];
    let after_space = (1..times.len())
        .filter(|&i| text.as_bytes()[i - 1] == b' ')
        .map(gap)
        .sum::<u64>() as f64
        / 4.0;
    let inside = (1..times.len())
        .filter(|&i| text.as_bytes()[i - 1].is_ascii_lowercase())
        .map(gap)
        .sum::<u64>() as f64
        / (1..times.len())
            .filter(|&i| text.as_bytes()[i - 1].is_ascii_lowercase())
            .count() as f64;
    assert!(after_space > inside, "a beat before each new word");
}

#[test]
fn channels_parse_by_line_and_reject_the_wrong_kind() {
    assert_eq!(
        TerminalChannel::parse("line.cmd0.typed"),
        Some(("cmd0", TerminalChannel::Typed))
    );
    assert_eq!(TerminalChannel::parse("line.cmd0.wobble"), None);
    assert_eq!(TerminalChannel::parse("caret"), None);
    let task = TerminalLinePlan::Task {
        id: "t".into(),
        spans: Vec::new(),
        done: Vec::new(),
        mark: Mark::Check,
    };
    assert!(task.accepts(TerminalChannel::Spin) && !task.accepts(TerminalChannel::Typed));
    assert!(!output("o", "x").accepts(TerminalChannel::Mark));
    assert!(accepts_property("scroll") && accepts_property("content"));
    assert!(!accepts_property("typed"));
}

fn events<'a>(plan: &'a crate::plan::ScenePlan, property: &str) -> &'a [TrackEventPlan] {
    &plan
        .continuous_channels
        .iter()
        .find(|channel| channel.property == property)
        .unwrap_or_else(|| panic!("no channel {property}"))
        .events
}

#[test]
fn a_session_types_prints_spins_resolves_and_clears() {
    let mut scene = PlanBuilder::new("terminal", 20 * SECOND);
    let mut term =
        TerminalActor::declare(&mut scene, "term", TerminalPlan::new([0.0, 0.0], 1000.0, 8))
            .unwrap();
    let shown = term.show(&mut scene, 0);
    let entered = term.type_command(&mut scene, shown, "ls -la").unwrap();
    assert!(entered > shown + seconds(REACH_SECONDS));
    let printed = term
        .print_text(&mut scene, entered, "a\n\nb", Tone::Plain)
        .unwrap();
    assert_eq!(printed, entered + 2 * seconds(PRINT_GAP));
    let task = term
        .spin(
            &mut scene,
            printed,
            vec![CaptionSpanPlan::new("testing", Tone::Muted)],
        )
        .unwrap();
    let drawn = term
        .resolve(
            &mut scene,
            &task,
            printed + 2 * SECOND,
            Mark::Check,
            vec![CaptionSpanPlan::new("passed", Tone::Success)],
        )
        .unwrap();
    assert!(drawn >= printed + 2 * SECOND + seconds(f64::from(spinner::DRAW)));
    term.prompt(&mut scene, drawn).unwrap();
    let second = term
        .type_command(&mut scene, drawn + 2 * SECOND, "clear")
        .unwrap();
    term.clear(&mut scene, second);
    let lines = term.plan().lines.clone();
    let plan = scene.finish().unwrap();
    // One exact step per character, ending fully typed.
    let typed = events(&plan, "line.cmd0.typed");
    assert_eq!(typed.len(), 6);
    assert!(
        matches!(typed.last(), Some(TrackEventPlan::Set { value: ScalarPlan::Literal(v), .. }) if *v == 1.0)
    );
    // The caret blinks while the second prompt waits and is on when typing starts.
    let caret = events(&plan, "caret");
    assert!(caret.len() > 6, "{caret:?}");
    // Every line has a reveal; the scroll floor lands at the content height.
    assert_eq!(lines.len(), 6);
    assert!(lines.iter().all(|line| {
        plan.continuous_channels
            .iter()
            .any(|c| c.property == line_property(line.id(), TerminalChannel::Reveal))
    }));
    assert!(matches!(
        events(&plan, "scroll").last(),
        Some(TrackEventPlan::Spring { target: ScalarPlan::Literal(v), .. }) if *v == 6.0
    ));
    // The mark clock starts at a top-right crossing after the request.
    let Some(TrackEventPlan::Set { at_nanos, .. }) = events(&plan, "line.task4.mark").first()
    else {
        panic!("mark clock")
    };
    assert!(*at_nanos >= printed + 2 * SECOND);
}
