use super::*;

const SECOND: u64 = 1_000_000_000;

/// A monospaced measure: ten pixels per character.
fn mono(text: &str) -> f32 {
    text.chars().count() as f32 * 10.0
}

fn identities(plan: &RollingNumberPlan, value: &str) -> Vec<(String, String)> {
    plan.model(value)
        .tokens
        .into_iter()
        .map(|t| (t.identity, t.text))
        .collect()
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

fn wheel(roll: &CompiledRoll, seconds: f64, identity: &str) -> MotionState {
    let glyph = roll
        .sample(seconds)
        .into_iter()
        .find(|g| g.token.identity == identity)
        .unwrap_or_else(|| panic!("{identity} is visible at {seconds}"));
    MotionState::at(glyph.wheel.expect("a digit"))
}

fn glyph<'a>(roll: &'a CompiledRoll, seconds: f64, identity: &str) -> Option<RollGlyph<'a>> {
    roll.sample(seconds)
        .into_iter()
        .find(|g| g.token.identity == identity)
}

#[test]
fn values_split_into_digit_places_separators_and_literals() {
    let plan = RollingNumberPlan::new([0.0, 0.0], 40.0, "rc.112")
        .prefix(vec![CaptionSpanPlan::new("v ", Tone::Muted)])
        .suffix(vec![CaptionSpanPlan::new(" tests", Tone::Muted)]);
    assert_eq!(
        identities(&plan, "rc.112"),
        pairs(&[
            ("prefix:0", "v "),
            ("lit:0", "rc."),
            ("digit:0:2", "1"),
            ("digit:0:1", "1"),
            ("digit:0:0", "2"),
            ("suffix:0", " tests"),
        ])
    );
    let plain = RollingNumberPlan::new([0.0, 0.0], 40.0, "0");
    assert_eq!(
        identities(&plain, "0/8"),
        pairs(&[("digit:0:0", "0"), ("lit:0", "/"), ("digit:1:0", "8")])
    );
    assert_eq!(
        identities(&plain, "$1,234.56"),
        pairs(&[
            ("lit:0", "$"),
            ("digit:0:3", "1"),
            ("group:0:3", ","),
            ("digit:0:2", "2"),
            ("digit:0:1", "3"),
            ("digit:0:0", "4"),
            ("decimal:0", "."),
            ("digit:0:-1", "5"),
            ("digit:0:-2", "6"),
        ])
    );
    // A trailing comma or period is punctuation, not grouping or a fraction.
    assert_eq!(
        identities(&plain, "8, done."),
        pairs(&[("digit:0:0", "8"), ("lit:0", ", done.")])
    );
}

#[test]
fn auto_direction_follows_each_runs_magnitude() {
    let plan = RollingNumberPlan::new([0.0, 0.0], 40.0, "0");
    let magnitude = |value: &str| plan.model(value).magnitudes;
    let up = |a: &str, b: &str| trend(&magnitude(a)[0], &magnitude(b)[0]);
    assert_eq!(up("999", "1,000"), 1);
    assert_eq!(up("1.5", "1.25"), -1);
    assert_eq!(up("007", "7"), 0);
    let (before, after) = (magnitude("0/8"), magnitude("8/8"));
    assert_eq!(trend(&before[0], &after[0]), 1);
    assert_eq!(trend(&before[1], &after[1]), 0);
}

#[test]
fn wheel_targets_honor_the_trend() {
    assert_eq!(roll_target(2.0, 7, 1), 7.0);
    assert_eq!(roll_target(7.0, 2, 1), 12.0);
    assert_eq!(roll_target(7.0, 2, -1), 2.0);
    assert_eq!(roll_target(2.0, 7, -1), -3.0);
    assert_eq!(roll_target(9.0, 0, 1), 10.0);
    assert_eq!(roll_target(8.0, 1, 0), 11.0);
    // Mid-roll, the nearest face in the direction of travel.
    assert_eq!(roll_target(5.3, 2, -1), 2.0);
    assert_eq!(roll_target(5.3, 2, 1), 12.0);
}

#[test]
fn rolling_up_turns_only_the_changed_wheel_forward() {
    let plan = RollingNumberPlan::new([0.0, 0.0], 40.0, "rc.112").roll(SECOND, "rc.117");
    plan.validate().unwrap();
    let roll = plan.compile(mono);
    let mut previous = wheel(&roll, 1.0, "digit:0:0").position;
    assert_eq!(previous, 2.0);
    for frame in 1..=40 {
        let t = 1.0 + f64::from(frame) / 60.0;
        let position = wheel(&roll, t, "digit:0:0").position;
        assert!(position >= previous, "the wheel only advances");
        previous = position;
        assert_eq!(wheel(&roll, t, "digit:0:1").position, 1.0);
        assert_eq!(wheel(&roll, t, "digit:0:2").position, 1.0);
        for still in ["lit:0", "digit:0:1", "digit:0:2"] {
            assert_eq!(
                glyph(&roll, t, still).unwrap().x,
                glyph(&roll, 0.0, still).unwrap().x
            );
        }
    }
    assert_eq!(wheel(&roll, 1.5, "digit:0:0").position, 7.0);
    let mid = glyph(&roll, 1.08, "digit:0:0").unwrap();
    assert!(mid.smear > 0.5, "a fast reel smears: {}", mid.smear);
    assert_eq!(glyph(&roll, 1.0, "digit:0:0").unwrap().smear, 0.0);
    assert_eq!(glyph(&roll, 1.5, "digit:0:0").unwrap().smear, 0.0);
    assert_eq!(roll.settled_text(2.0), "rc.117");
    assert!(roll.moving(1.2) && !roll.moving(0.9) && !roll.moving(1.6));
}

#[test]
fn rolling_down_turns_backward() {
    let roll = RollingNumberPlan::new([0.0, 0.0], 40.0, "117")
        .roll(SECOND, "112")
        .compile(mono);
    let positions = (0..=30)
        .map(|frame| wheel(&roll, 1.0 + f64::from(frame) / 60.0, "digit:0:0").position)
        .collect::<Vec<_>>();
    assert!(positions.windows(2).all(|w| w[1] <= w[0]));
    assert_eq!(positions[0], 7.0);
    assert_eq!(wheel(&roll, 2.0, "digit:0:0").position, 2.0);
}

#[test]
fn a_carry_rolls_every_nine_and_opens_room_for_new_places() {
    let roll = RollingNumberPlan::new([0.0, 0.0], 40.0, "999")
        .roll(SECOND, "1,000")
        .compile(mono);
    for place in ["digit:0:0", "digit:0:1", "digit:0:2"] {
        assert_eq!(wheel(&roll, 1.0, place).position, 9.0);
        assert_eq!(
            wheel(&roll, 2.0, place).position,
            10.0,
            "{place} rolls one face"
        );
    }
    // Left-aligned: the new "1," enters at the left edge of the old number,
    // and the retained digits glide right by its width.
    assert!(glyph(&roll, 0.99, "digit:0:3").is_none());
    let entering = glyph(&roll, 1.0, "digit:0:3").unwrap();
    assert_eq!((entering.x, entering.opacity), (0.0, 0.0));
    assert_eq!(entering.rise, plan_row());
    let comma = glyph(&roll, 1.04, "group:0:3").unwrap();
    assert_eq!(comma.opacity, 0.0, "a new separator waits for its digit");
    assert_eq!(glyph(&roll, 0.5, "digit:0:2").unwrap().x, 0.0);
    assert_eq!(glyph(&roll, 2.0, "digit:0:2").unwrap().x, 20.0);
    let rising = glyph(&roll, 1.2, "digit:0:3").unwrap();
    assert!(rising.rise > 0.0 && rising.rise < plan_row() && rising.opacity > 0.0);
    let settled = glyph(&roll, 2.0, "digit:0:3").unwrap();
    assert_eq!((settled.rise, settled.opacity), (0.0, 1.0));
    assert_eq!(roll.settled_text(2.0), "1,000");
}

fn plan_row() -> f32 {
    RollingNumberPlan::new([0.0, 0.0], 40.0, "0").row_height()
}

#[test]
fn shrinking_fades_old_places_out_and_removes_them() {
    let roll = RollingNumberPlan::new([0.0, 0.0], 40.0, "1,000")
        .roll(SECOND, "999")
        .compile(mono);
    let fading = glyph(&roll, 1.1, "digit:0:3").unwrap();
    assert!(fading.opacity < 1.0 && fading.opacity > 0.0);
    assert!(glyph(&roll, 1.0 + 0.65 * 0.5 + 1e-6, "digit:0:3").is_none());
    assert!(
        glyph(&roll, 1.2, "group:0:3").is_none(),
        "symbols fade quickly"
    );
    assert_eq!(wheel(&roll, 2.0, "digit:0:0").position, -1.0);
    assert_eq!(roll.settled_text(2.0), "999");
}

#[test]
fn a_mid_roll_retarget_keeps_position_and_velocity() {
    let roll = RollingNumberPlan::new([0.0, 0.0], 40.0, "112")
        .roll(SECOND, "117")
        .roll(SECOND + 150_000_000, "113")
        .compile(mono);
    let at = 1.15;
    let before = roll_state(&roll, at - 1e-4);
    let after = roll_state(&roll, at + 1e-4);
    assert!((after.position - before.position).abs() < 0.01);
    assert!((after.velocity - before.velocity).abs() < 0.05 * before.velocity.abs());
    assert!(before.velocity > 1.0, "it was still rolling up");
    // Rolling back down to 3 from about 5 passes no extra revolution.
    let positions = (0..60)
        .map(|frame| roll_state(&roll, at + f64::from(frame) / 60.0).position)
        .collect::<Vec<_>>();
    assert!(positions.iter().all(|p| (2.9..7.1).contains(p)));
    assert_eq!(roll.settled_text(3.0), "113");
}

fn roll_state(roll: &CompiledRoll, seconds: f64) -> MotionState {
    let column = roll
        .columns
        .iter()
        .find(|c| c.token(seconds).identity == "digit:0:0")
        .unwrap();
    column.wheel.sample(seconds)
}

#[test]
fn sampling_is_a_pure_function_of_time() {
    let plan = RollingNumberPlan::new([960.0, 540.0], 64.0, "0/8")
        .aligned(CaptionAlign::Center)
        .roll(SECOND, "3/8")
        .roll(SECOND + 200_000_000, "8/8")
        .roll(3 * SECOND, "10/18");
    let a = plan.compile(mono);
    let b = plan.compile(mono);
    let times = [2.4, 0.3, 1.1, 3.05, 1.21, 0.0, 3.3, 1.1];
    for t in times {
        assert_eq!(a.sample(t), b.sample(t));
    }
    let forward = times.map(|t| format!("{:?}", a.sample(t)));
    let mut reversed = times;
    reversed.reverse();
    let backward = reversed.map(|t| format!("{:?}", a.sample(t)));
    assert!(forward.iter().eq(backward.iter().rev()));
    assert_eq!(a.settled_text(5.0), "10/18");
}

#[test]
fn centered_text_stays_centered_as_it_widens() {
    let roll = RollingNumberPlan::new([0.0, 0.0], 40.0, "rc.99")
        .aligned(CaptionAlign::Center)
        .roll(SECOND, "rc.100")
        .compile(mono);
    let extent = |t: f64| {
        let glyphs = roll.sample(t);
        let left = glyphs.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
        let right = glyphs
            .iter()
            .map(|g| g.x + g.width)
            .fold(f32::NEG_INFINITY, f32::max);
        (left, right)
    };
    assert_eq!(extent(0.5), (-25.0, 25.0));
    assert_eq!(extent(2.0), (-30.0, 30.0));
    let (left, _) = extent(1.1);
    assert!(
        left < -25.0 && left > -30.0,
        "the prefix glides left: {left}"
    );
}

#[test]
fn start_stagger_sweeps_changing_digits_left_to_right() {
    let roll = RollingNumberPlan::new([0.0, 0.0], 40.0, "111")
        .stagger(RollStagger::Start)
        .roll(SECOND, "222")
        .compile(mono);
    let at = 1.03;
    let hundreds = wheel(&roll, at, "digit:0:2").position;
    let ones = wheel(&roll, at, "digit:0:0").position;
    assert!(hundreds > ones && ones == 1.0, "{hundreds} leads {ones}");
    assert_eq!(roll.settled_text(2.0), "222");
}

#[test]
fn entry_ranks_spread_outward_from_retained_digits() {
    assert_eq!(
        entry_ranks(&[false, false, true, true], RollStagger::Outward),
        vec![2, 1, 0, 0]
    );
    assert_eq!(
        entry_ranks(&[false, true, false, false], RollStagger::Outward),
        vec![1, 0, 1, 2]
    );
    assert_eq!(
        entry_ranks(&[false, false, false], RollStagger::Outward),
        vec![1, 2, 3]
    );
    assert_eq!(
        entry_ranks(&[false, true, false], RollStagger::End),
        vec![2, 0, 1]
    );
    assert_eq!(
        entry_ranks(&[false, true, false], RollStagger::None),
        vec![1, 0, 1]
    );
}

#[test]
fn plans_round_trip_with_compact_defaults_and_reject_bad_input() {
    let plan = RollingNumberPlan::new([100.0, 200.0], 48.0, "rc.112")
        .tone(Tone::Accent)
        .roll(SECOND, "rc.117");
    plan.validate().unwrap();
    let json = serde_json::to_value(&plan).unwrap();
    for absent in [
        "align",
        "durationNanos",
        "stagger",
        "direction",
        "blur",
        "chip",
    ] {
        assert!(json.get(absent).is_none(), "{absent} is omitted");
    }
    assert_eq!(json["rolls"][0]["atNanos"], SECOND);
    assert_eq!(
        serde_json::from_value::<RollingNumberPlan>(json).unwrap(),
        plan
    );
    for bad in [
        plan.clone().roll(SECOND, "rc.118"),
        plan.clone().roll(2 * SECOND, ""),
        plan.clone().roll(2 * SECOND, "a\nb"),
        plan.clone().duration_nanos(0),
        plan.clone().blur(f32::NAN),
        RollingNumberPlan::new([f32::NAN, 0.0], 48.0, "1"),
        RollingNumberPlan::new([0.0, 0.0], 400.0, "1"),
    ] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
}

#[test]
fn the_actor_records_changes_as_they_are_authored() {
    let mut scene = PlanBuilder::new("rolling", 4 * SECOND);
    let mut ci = RollingNumberActor::declare(
        &mut scene,
        "ci",
        RollingNumberPlan::new([960.0, 540.0], 40.0, "0/8"),
    )
    .unwrap();
    ci.show(&mut scene, 0);
    ci.roll(&mut scene, SECOND, "8/8").unwrap();
    assert!(ci.roll(&mut scene, SECOND, "7/8").is_err());
    let plan = scene.finish().unwrap();
    let data: RollingNumberPlan = serde_json::from_value(plan.actors[0].data.clone()).unwrap();
    assert_eq!(data.rolls.len(), 1);
    assert_eq!(data.rolls[0].value, "8/8");
}
