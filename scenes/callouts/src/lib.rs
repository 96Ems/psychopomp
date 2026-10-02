//! Callout showroom: callouts pinned to two Stage cards through a camera
//! dolly, a jolt, and a glide between anchors; then a callout pinned to a code
//! range that moves when lines are inserted above it and the panel zooms.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, SECOND},
    callout::{CalloutActor, CalloutAnchorPlan, CalloutPlan, CalloutSide},
    caption::CaptionSpanPlan,
    code::{StyledSpan, SyntaxStyle},
    editor::{
        EDITOR_RECIPE, EditorLinePlan, EditorPartPlan, EditorRecipePlan, EditorSemanticRangePlan,
        EditorSnapshotPlan, EditorTargetSelector,
    },
    highlight,
    math::easing::Ease,
    plan::{ReelPlan, ScenePlan},
    stage::{StageActor, StagePlan},
    tone::Tone,
};

const MS: u64 = 1_000_000;

pub fn build_reel() -> Result<ReelPlan> {
    ReelPlan::dipped("callouts", vec![build_stage()?, build_editor()?], 600 * MS)
}

fn span(text: &str, tone: Tone) -> CaptionSpanPlan {
    CaptionSpanPlan::new(text, tone)
}

fn stage_anchor(id: &str, element: &str, edge: CalloutSide) -> CalloutAnchorPlan {
    CalloutAnchorPlan::Stage {
        id: id.into(),
        element: element.into(),
        edge,
        side: None,
    }
}

/// Two cards and a wire. The camera dollies toward the API, a request lands
/// with a jolt, and a callout glides from one card to the other; every
/// callout stays pinned to its card edge throughout.
pub fn build_stage() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("callouts-stage", 9 * SECOND);
    let recipe: StagePlan = serde_json::from_value(serde_json::json!({
        "post": { "bloom": 0.18, "grain": 0.012, "vignette": 0.22, "backdrop": 0.12 },
        "elements": [
            { "kind": "card", "id": "client", "at": [560, 560, 0], "size": [320, 120], "title": "client" },
            { "kind": "card", "id": "api", "at": [1360, 560, 0], "size": [320, 120], "title": "api", "tone": "accent" },
            { "kind": "beam", "id": "link", "from": "client", "to": "api" },
            { "kind": "packet", "id": "request", "beam": "link", "label": "GET /user" }
        ]
    }))?;
    let mut stage = StageActor::declare(&mut scene, "stage", &recipe)?;
    stage.settle_in(&mut scene, "client", 100 * MS);
    stage.settle_in(&mut scene, "api", 220 * MS);
    let contact = stage.connect(&mut scene, "link", 700 * MS, 0.5);
    stage.to(&mut scene, "link.flow", contact + 1200 * MS, 0.0, 0.6);

    let mut retries = CalloutActor::declare(
        &mut scene,
        "retries",
        &CalloutPlan::new(
            stage_anchor("client", "client", CalloutSide::Top),
            vec![span("retries ", Tone::Plain), span("3×", Tone::Accent)],
        )
        .anchor(CalloutAnchorPlan::Stage {
            id: "api".into(),
            element: "api".into(),
            edge: CalloutSide::Top,
            side: Some(CalloutSide::TopLeft),
        })
        .side(CalloutSide::TopRight)
        .elbow()
        .reach(72.0),
    )?;
    let mut cache = CalloutActor::declare(
        &mut scene,
        "cache",
        &CalloutPlan::new(
            stage_anchor("api", "api", CalloutSide::BottomRight),
            vec![
                span("cache hit", Tone::Success),
                span(" · 2 ms", Tone::Muted),
            ],
        )
        .side(CalloutSide::BottomRight)
        .elbow()
        .reach(56.0)
        .tone(Tone::Success)
        .chip(),
    )?;
    let mut fixed = CalloutActor::declare(
        &mut scene,
        "wire",
        &CalloutPlan::new(
            CalloutAnchorPlan::Point {
                id: "here".into(),
                at: [960.0, 900.0],
                side: None,
            },
            vec![span("a fixed point stays put", Tone::Muted)],
        )
        .side(CalloutSide::Right)
        .reach(40.0)
        .tone(Tone::Muted),
    )?;

    retries.show(&mut scene, 1300 * MS);
    cache.show(&mut scene, 1700 * MS);
    fixed.show(&mut scene, 2000 * MS);

    // Dolly in toward the API; the callouts ride with their cards, and the
    // cache label slides back inside the frame rather than leave it.
    stage.to(&mut scene, "camera.z", 2800 * MS, 340.0, 1.4);
    stage.to(&mut scene, "camera.x", 2800 * MS, 100.0, 1.4);
    stage.to(&mut scene, "camera.y", 2800 * MS, -30.0, 1.4);

    // A request lands on the API with a jolt.
    let landed = stage.send(&mut scene, "request", 4300 * MS, 0.7);
    stage.land(&mut scene, "api", landed);
    stage.jolt(&mut scene, landed, [1.0, 0.0], 0.8);
    cache.emphasize(&mut scene, landed);

    // The retry note moves to the card that actually retries.
    retries.move_to(&mut scene, "api", 5600 * MS)?;

    // Pull back out; then everything retracts.
    stage.to(&mut scene, "camera.z", 6600 * MS, 0.0, 1.4);
    stage.to(&mut scene, "camera.x", 6600 * MS, 0.0, 1.4);
    stage.to(&mut scene, "camera.y", 6600 * MS, 0.0, 1.4);
    fixed.hide(&mut scene, 7900 * MS);
    cache.hide(&mut scene, 8000 * MS);
    retries.hide(&mut scene, 8100 * MS);

    scene.cue("pinned", 0, 9 * SECOND);
    Ok(scene.finish()?)
}

const FILE: [(&str, &str); 8] = [
    ("import", "import { Effect } from \"effect\""),
    ("blank", " "),
    ("open", "const program = Effect.gen(function* () {"),
    ("log", "  yield* Effect.log(\"loading user\")"),
    ("id", "  const id = yield* UserId"),
    ("user", "  const user = yield* fetchUser(id)"),
    ("close", "  return user.name"),
    ("end", "})"),
];

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

/// The `user` line splits its call into its own part so it can be targeted.
fn user_line() -> EditorLinePlan {
    let part = |id: &str, spans: Vec<StyledSpan>| EditorPartPlan {
        id: id.into(),
        spans,
    };
    EditorLinePlan {
        id: "user".into(),
        parts: vec![
            part("lead", highlight::typescript("  const user = yield* ")),
            part(
                "call",
                vec![
                    StyledSpan::new("fetchUser", SyntaxStyle::Type),
                    StyledSpan::new("(id)", SyntaxStyle::Plain),
                ],
            ),
        ],
        semantic_ranges: vec![EditorSemanticRangePlan {
            id: "fetch".into(),
            first_part_id: "call".into(),
            last_part_id: "call".into(),
        }],
        mark: None,
    }
}

/// Two lines are inserted above a targeted call; the callout rides down with
/// it, then follows the panel as it zooms and shifts.
pub fn build_editor() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("callouts-editor", 7 * SECOND);
    let lines = FILE
        .iter()
        .map(|(id, text)| match *id {
            "user" => user_line(),
            _ => code_line(id, text),
        })
        .chain(["gap-0", "gap-1"].map(|id| code_line(id, " ")))
        .collect::<Vec<_>>();
    let ids = |ids: &[&str]| ids.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();
    let before = ids(&["import", "blank", "open", "user", "close", "end"]);
    // Blank rows open first, so the moving call never crosses entering code.
    let room = ids(&[
        "import", "blank", "open", "gap-0", "gap-1", "user", "close", "end",
    ]);
    let after = ids(&[
        "import", "blank", "open", "log", "id", "user", "close", "end",
    ]);
    let recipe = EditorRecipePlan {
        file_name: "program.ts".into(),
        focus_line_id: "user".into(),
        lines,
        initial_line_ids: before,
        final_line_ids: after.clone(),
        snapshots: vec![
            EditorSnapshotPlan {
                at_nanos: 2600 * MS,
                line_ids: room,
            },
            EditorSnapshotPlan {
                at_nanos: 2900 * MS,
                line_ids: after,
            },
        ],
        line_height: 44.0,
        entering_offset_x: 0.0,
        focus_height: 44.0,
        inline_reveal: None,
        additional_inline_reveals: Vec::new(),
    };
    let editor = scene.actor("editor", EDITOR_RECIPE, &recipe)?;
    scene.semantic_target(
        "fetch",
        &editor,
        EditorTargetSelector {
            line_id: "user".into(),
            range_id: "fetch".into(),
        },
    )?;
    let panel_y = scene.continuous(&editor, "panel-y", 70.0);
    let panel_opacity = scene.continuous(&editor, "panel-opacity", 0.0);
    scene.spring(&panel_y, 0, 0.0, 0.7, 0.0);
    scene.spring(&panel_opacity, 0, 1.0, 0.5, 0.0);

    let mut fails = CalloutActor::declare(
        &mut scene,
        "fails",
        &CalloutPlan::new(
            CalloutAnchorPlan::Editor {
                id: "fetch".into(),
                target: "fetch".into(),
                edge: CalloutSide::Bottom,
                side: None,
            },
            vec![
                span("can fail: ", Tone::Muted),
                span("UserNotFound", Tone::Error),
            ],
        )
        .side(CalloutSide::BottomRight)
        .elbow()
        .reach(70.0)
        .tone(Tone::Error),
    )?;
    fails.show(&mut scene, 1000 * MS);
    fails.emphasize(&mut scene, 2600 * MS);

    // Zoom the panel toward the call; the callout follows the projection.
    let scale = scene.continuous(&editor, "panel-scale", 1.0);
    let panel_x = scene.continuous(&editor, "panel-x", 0.0);
    scene.ease(&scale, 4200 * MS, 1.12, 1.2, Ease::Smootherstep);
    scene.ease(&panel_x, 4200 * MS, -90.0, 1.2, Ease::Smootherstep);
    scene.spring(&panel_y, 4200 * MS, -40.0, 1.0, 0.0);
    fails.hide(&mut scene, 6200 * MS);

    scene.cue("pinned", 0, 7 * SECOND);
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
