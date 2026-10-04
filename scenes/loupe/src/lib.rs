//! Loupe showroom: a lens of thick glass condenses over a code range, glides
//! to another as if it were a puck on the page, stretches into a capsule to
//! read along a line, then follows a Stage card's status through a camera move
//! and glides to the next card.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, SECOND},
    callout::{CalloutAnchorPlan, CalloutSide},
    code::StyledSpan,
    editor::{
        EDITOR_RECIPE, EditorLinePlan, EditorPartPlan, EditorRecipePlan, EditorSemanticRangePlan,
        EditorTargetSelector,
    },
    highlight,
    lens::{LensActor, LensPlan},
    plan::{ReelPlan, ScenePlan},
    stage::{StageActor, StagePlan},
};

const MS: u64 = 1_000_000;

pub fn build_reel() -> Result<ReelPlan> {
    ReelPlan::dipped("loupe", vec![build_editor()?, build_stage()?], 600 * MS)
}

fn code_line(id: &str, text: &str) -> EditorLinePlan {
    EditorLinePlan {
        id: id.into(),
        parts: vec![EditorPartPlan {
            id: "code".into(),
            spans: highlight::typescript(text),
        }],
        semantic_ranges: Vec::new(),
        mark: None,
    }
}

/// A line whose `range` part is a Semantic Target the lens can pin to.
fn targeted_line(id: &str, lead: &str, range: (&str, &str), tail: &str) -> EditorLinePlan {
    let part = |id: &str, spans: Vec<StyledSpan>| EditorPartPlan {
        id: id.into(),
        spans,
    };
    let mut parts = vec![
        part("lead", highlight::typescript(lead)),
        part(range.0, highlight::typescript(range.1)),
    ];
    if !tail.is_empty() {
        parts.push(part("tail", highlight::typescript(tail)));
    }
    EditorLinePlan {
        id: id.into(),
        parts,
        semantic_ranges: vec![EditorSemanticRangePlan {
            id: range.0.into(),
            first_part_id: range.0.into(),
            last_part_id: range.0.into(),
        }],
        mark: None,
    }
}

fn editor_anchor(target: &str) -> CalloutAnchorPlan {
    CalloutAnchorPlan::Editor {
        id: target.into(),
        target: target.into(),
        edge: CalloutSide::Center,
        side: None,
    }
}

/// The loupe reads code: it condenses over a call, glides to the retry
/// schedule, stretches into a capsule to read along that line, then settles
/// on the timeout and leans in closer.
pub fn build_editor() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("loupe-editor", 9 * SECOND);
    let lines = vec![
        code_line("import", "import { Effect, Schedule } from \"effect\""),
        code_line("blank", " "),
        code_line("fetch", "const fetchUser = (id: UserId) =>"),
        code_line("get", "  Http.get(`/users/${id}`).pipe("),
        targeted_line(
            "retry",
            "    Effect.retry(",
            ("schedule", "Schedule.exponential(\"100 millis\")"),
            "),",
        ),
        targeted_line(
            "timeout",
            "    Effect.timeout(",
            ("limit", "\"2 seconds\""),
            "),",
        ),
        code_line("close", "  )"),
        code_line("gap", " "),
        code_line("open", "const program = Effect.gen(function* () {"),
        targeted_line(
            "user",
            "  const user = yield* ",
            ("call", "fetchUser(id)"),
            "",
        ),
        code_line("return", "  return user.name"),
        code_line("end", "})"),
    ];
    let ids = lines.iter().map(|line| line.id.clone()).collect::<Vec<_>>();
    let recipe = EditorRecipePlan {
        file_name: "user.ts".into(),
        focus_line_id: "user".into(),
        lines,
        initial_line_ids: ids.clone(),
        final_line_ids: ids,
        snapshots: Vec::new(),
        line_height: 44.0,
        entering_offset_x: 0.0,
        focus_height: 44.0,
        inline_reveal: None,
        additional_inline_reveals: Vec::new(),
    };
    let editor = scene.actor("editor", EDITOR_RECIPE, &recipe)?;
    for (target, line) in [
        ("call", "user"),
        ("schedule", "retry"),
        ("limit", "timeout"),
    ] {
        scene.semantic_target(
            target,
            &editor,
            EditorTargetSelector {
                line_id: line.into(),
                range_id: target.into(),
            },
        )?;
    }
    let panel_y = scene.continuous(&editor, "panel-y", 70.0);
    let panel_opacity = scene.continuous(&editor, "panel-opacity", 0.0);
    scene.spring(&panel_y, 0, 0.0, 0.7, 0.0);
    scene.spring(&panel_opacity, 0, 1.0, 0.5, 0.0);

    let mut loupe = LensActor::declare(
        &mut scene,
        "loupe",
        &LensPlan::circle(editor_anchor("call"), 250.0)
            .anchor(editor_anchor("schedule"))
            .anchor(editor_anchor("limit"))
            .magnification(1.7),
    )?;
    loupe.show(&mut scene, 900 * MS);
    loupe.move_to(&mut scene, "schedule", 2300 * MS)?;
    // Stretch into a capsule at the start of the schedule and read along it.
    loupe.resize(&mut scene, [560.0, 96.0], 3400 * MS);
    loupe.magnify(&mut scene, 1.5, 3400 * MS);
    loupe.slide(&mut scene, [-150.0, 0.0], 3400 * MS);
    loupe.slide(&mut scene, [150.0, 0.0], 4300 * MS);
    // Round again on the timeout, then float above the line while still
    // showing it, as a text loupe lifts clear of what it reads.
    loupe.move_to(&mut scene, "limit", 5600 * MS)?;
    loupe.slide(&mut scene, [0.0, 0.0], 5600 * MS);
    loupe.resize(&mut scene, [300.0, 300.0], 5600 * MS);
    loupe.magnify(&mut scene, 1.6, 5600 * MS);
    loupe.slide(&mut scene, [0.0, -150.0], 6700 * MS);
    loupe.focus(&mut scene, [0.0, 150.0], 6700 * MS);
    loupe.hide(&mut scene, 8000 * MS);

    scene.cue("reading", 0, 9 * SECOND);
    Ok(scene.finish()?)
}

/// The loupe magnifies a card's status while it changes, rides the card
/// through a dolly, then glides to the next card.
pub fn build_stage() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("loupe-stage", 8 * SECOND);
    let recipe: StagePlan = serde_json::from_value(serde_json::json!({
        "post": { "bloom": 0.18, "grain": 0.012, "vignette": 0.22, "backdrop": 0.12 },
        "elements": [
            { "kind": "card", "id": "client", "at": [420, 540, 0], "size": [300, 110], "title": "client",
              "status": [{ "text": "retrying 2/3" }] },
            { "kind": "card", "id": "api", "at": [960, 540, 0], "size": [300, 110], "title": "api",
              "status": [{ "text": "503 unavailable", "tone": "error" }, { "text": "200 ok", "tone": "success" }] },
            { "kind": "card", "id": "cache", "at": [1500, 540, 0], "size": [300, 110], "title": "cache",
              "status": [{ "text": "hit · 2 ms", "tone": "success" }] },
            { "kind": "beam", "id": "request", "from": "client", "to": "api" },
            { "kind": "beam", "id": "lookup", "from": "api", "to": "cache" },
            { "kind": "packet", "id": "retry", "beam": "request" }
        ]
    }))?;
    let mut stage = StageActor::declare(&mut scene, "stage", &recipe)?;
    stage.settle_in(&mut scene, "client", 100 * MS);
    stage.settle_in(&mut scene, "api", 220 * MS);
    stage.settle_in(&mut scene, "cache", 340 * MS);
    stage.connect(&mut scene, "request", 700 * MS, 0.5);
    stage.connect(&mut scene, "lookup", 820 * MS, 0.5);

    let status = |card: &str| CalloutAnchorPlan::Stage {
        id: card.into(),
        element: card.into(),
        edge: CalloutSide::Center,
        side: None,
    };
    let mut loupe = LensActor::declare(
        &mut scene,
        "loupe",
        &LensPlan::capsule(status("api"), [300.0, 96.0])
            .anchor(status("cache"))
            .magnification(1.8),
    )?;
    // The status line sits 19 px below the card's center.
    loupe.channel(&mut scene, "y", 19.0);
    loupe.show(&mut scene, 1400 * MS);

    // A retry lands and the status changes under the glass.
    let landed = stage.send(&mut scene, "retry", 2400 * MS, 0.7);
    stage.land(&mut scene, "api", landed);
    stage.ease(
        &mut scene,
        "api.status",
        landed,
        1.0,
        0.5,
        psychopomp::math::easing::Ease::Smootherstep,
    );

    // A gentle dolly; the glass rides the card.
    stage.to(&mut scene, "camera.z", 3900 * MS, 160.0, 1.4);
    stage.to(&mut scene, "camera.x", 3900 * MS, 80.0, 1.4);

    loupe.move_to(&mut scene, "cache", 5200 * MS)?;
    loupe.hide(&mut scene, 7000 * MS);

    scene.cue("following", 0, 8 * SECOND);
    Ok(scene.finish()?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_showroom_builds() {
        let reel = super::build_reel().unwrap();
        assert_eq!(reel.segments.len(), 2);
    }
}
