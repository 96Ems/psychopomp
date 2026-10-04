use super::*;
use crate::{
    author::SECOND,
    plan::{ScalarPlan, TrackEventPlan},
};

fn files() -> Vec<ChangedFilePlan> {
    vec![
        ChangedFilePlan::new(
            "compaction",
            "src/session/compaction.ts",
            FileStatus::Modified,
            48,
            12,
        ),
        ChangedFilePlan::new("test", "test/session.test.ts", FileStatus::Added, 1200, 0),
        ChangedFilePlan::new("old", "src/sleep.ts", FileStatus::Deleted, 0, 31),
        ChangedFilePlan::new("moved", "src/util/wait.ts", FileStatus::Modified, 2, 1)
            .renamed_from("src/wait.ts"),
    ]
}

#[test]
fn diffstats_follow_github() {
    use Block::*;
    assert_eq!(diffstat(0, 0), [Neutral; 5]);
    assert_eq!(diffstat(2, 1), [Added, Added, Removed, Neutral, Neutral]);
    assert_eq!(diffstat(48, 12), [Added, Added, Added, Added, Removed]);
    assert_eq!(diffstat(1200, 0), [Added; 5]);
    assert_eq!(diffstat(0, 31), [Removed; 5]);
    assert_eq!(
        diffstat(1, 400),
        [Added, Removed, Removed, Removed, Removed],
        "a nonzero side keeps a block"
    );
    assert_eq!(diffstat(400, 1)[4], Removed);
}

#[test]
fn counts_group_and_paths_split() {
    assert_eq!(grouped(0), "0");
    assert_eq!(grouped(999), "999");
    assert_eq!(grouped(1383), "1,383");
    assert_eq!(grouped(1_234_567), "1,234,567");
    assert_eq!(split_path("src/session/a.ts"), ("src/session/", "a.ts"));
    assert_eq!(split_path("README.md"), ("", "README.md"));
}

#[test]
fn changed_files_round_trip_and_reject_bad_lists() {
    let plan = ChangedFilesPlan::new([100.0, 100.0], 1200.0, files()).titled("#50231");
    plan.validate().unwrap();
    let json = serde_json::to_value(&plan).unwrap();
    assert!(
        json["files"][0].get("status").is_none(),
        "modified is the default"
    );
    assert_eq!(json["files"][3]["status"], "renamed");
    assert_eq!(
        serde_json::from_value::<ChangedFilesPlan>(json).unwrap(),
        plan
    );
    let mut repeated = plan.clone();
    repeated.files[1].id = "compaction".into();
    assert!(repeated.validate().is_err());
    let mut moved = plan.clone();
    moved.files[0].from = Some("x".into());
    assert!(
        moved.validate().is_err(),
        "only renames have a previous path"
    );
    let mut empty = plan.clone();
    empty.files.clear();
    assert!(empty.validate().is_err());
}

#[test]
fn rows_hold_fixed_slots_and_scroll_through_the_window() {
    let plan = ChangedFilesPlan::new([100.0, 100.0], 1200.0, files()).max_rows(3);
    assert_eq!(plan.visible_rows(), 3);
    let row = plan.row_height();
    assert_eq!(plan.row_top(2, 0.0) - plan.row_top(1, 0.0), row);
    assert_eq!(plan.row_top(1, 1.0), plan.rows_top());
    assert_eq!(
        plan.card_size()[1],
        plan.header_height() + 3.0 * row + PADDING * 0.5
    );
    let titled = plan.clone().titled("pr");
    assert_eq!(titled.rows_top() - plan.rows_top(), window::TITLE_BAR);
}

#[test]
fn static_totals_sum_every_file() {
    let plan = ChangedFilesPlan::new([0.0, 0.0], 1200.0, files());
    let (initial, later) = plan.totals_schedule();
    assert!(later.is_empty());
    assert_eq!(
        (initial.files, initial.added, initial.removed),
        (4, 1250, 44)
    );
}

#[test]
fn revealing_rows_staggers_them_and_rolls_the_totals() {
    let mut scene = PlanBuilder::new("changes", 10 * SECOND);
    let mut card = ChangedFilesActor::declare(
        &mut scene,
        "files",
        ChangedFilesPlan::new([0.0, 0.0], 1200.0, files()),
    )
    .unwrap();
    card.show(&mut scene, 0);
    let landed = card.reveal(&mut scene, SECOND, 0.08).unwrap();
    let gap = seconds(f64::from(0.08_f32));
    assert_eq!(
        landed,
        SECOND + 3 * gap + seconds(f64::from(REVEAL_SECONDS))
    );
    card.focus(&mut scene, "test", 3 * SECOND).unwrap();
    card.unfocus(&mut scene, 5 * SECOND);
    let recipe = card.plan().clone();
    let (initial, rolls) = recipe.totals_schedule();
    assert_eq!(
        initial,
        ChangedTotalsPlan::default(),
        "totals start at zero"
    );
    assert_eq!(rolls.len(), 4);
    assert_eq!(
        (rolls[1].files, rolls[1].added, rolls[1].removed),
        (2, 1248, 12)
    );
    assert_eq!(
        (rolls[3].files, rolls[3].added, rolls[3].removed),
        (4, 1250, 44)
    );
    assert!(
        rolls
            .windows(2)
            .all(|pair| pair[0].at_nanos < pair[1].at_nanos)
    );
    assert!(card.reveal_row(&mut scene, "test", 6 * SECOND).is_err());
    let plan = scene.finish().unwrap();
    let channel = |property: &str| {
        plan.continuous_channels
            .iter()
            .find(|channel| channel.property == property)
            .unwrap_or_else(|| panic!("{property}"))
    };
    assert!(matches!(channel("row.old.reveal").initial, ScalarPlan::Literal(v) if v == 0.0));
    assert!(matches!(
        channel("row.old.reveal").events[0],
        TrackEventPlan::Spring { at_nanos, .. } if at_nanos == SECOND + 2 * gap
    ));
    // Focus recedes every other row and lights the focused one.
    assert!(matches!(
        channel("row.compaction.dim").events[0],
        TrackEventPlan::Spring { target: ScalarPlan::Literal(v), .. } if v == 1.0
    ));
    assert!(matches!(
        channel("row.test.dim").events[0],
        TrackEventPlan::Spring { target: ScalarPlan::Literal(v), .. } if v == 0.0
    ));
}

#[test]
fn row_channels_parse() {
    assert_eq!(
        RowChannel::parse("row.a-1.dim"),
        Some(("a-1", RowChannel::Dim))
    );
    assert_eq!(RowChannel::parse("row.a.wobble"), None);
    assert!(accepts_property("scroll") && !accepts_property("caret"));
}
