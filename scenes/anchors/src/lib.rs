//! Anchors showroom: overlays that pin to things without a hierarchy. On a
//! Stage, a caption, a Rolling Number, text labels, and a framed image ride
//! two cards through a dolly, a jolt, and glides between anchors (one of them
//! redirected mid-flight). In an editor, a caption and a Rolling Number stay
//! on their code ranges while lines are inserted above and the panel zooms.
//! In a Sequence Diagram, a callout, a counter, and a cursor ride its rows and
//! participant headers as the diagram slides.
use anyhow::Result;
use psychopomp::{
    anchor::{AnchorPlan, Edge},
    author::{PlanBuilder, SECOND},
    callout::{CalloutActor, CalloutAnchorPlan, CalloutPlan},
    caption::{CaptionActor, CaptionAlign, CaptionPlan, CaptionSpanPlan},
    code::{StyledSpan, SyntaxStyle},
    editor::{
        EDITOR_RECIPE, EditorLinePlan, EditorPartPlan, EditorRecipePlan, EditorSemanticRangePlan,
        EditorTargetSelector, diff,
    },
    highlight,
    image::{self, ImageActor, ImagePlan},
    math::easing::Ease,
    plan::{ReelPlan, ScenePlan},
    rolling::{RollingNumberActor, RollingNumberPlan},
    sequence::{SequenceActor, SequenceParticipantPlan, SequencePlan, SequenceRowPlan},
    stage::{StageActor, StagePlan},
    text::{TextActor, TextPlan},
    tone::Tone,
};

const MS: u64 = 1_000_000;
const MUTED_INK: [u8; 3] = [150, 160, 178];

pub fn build_reel() -> Result<ReelPlan> {
    ReelPlan::dipped(
        "anchors",
        vec![build_stage()?, build_editor()?, build_sequence()?],
        600 * MS,
    )
}

fn span(text: &str, tone: Tone) -> CaptionSpanPlan {
    CaptionSpanPlan::new(text, tone)
}

/// Two cards and a wire. Every overlay names the card it rides; the camera
/// dollies in, a request lands with a jolt, and the request's caption glides
/// from the client to the API alongside the packet.
pub fn build_stage() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("anchors-stage", 10 * SECOND);
    let recipe: StagePlan = serde_json::from_value(serde_json::json!({
        "post": { "bloom": 0.18, "grain": 0.012, "vignette": 0.22, "backdrop": 0.12 },
        "elements": [
            { "kind": "card", "id": "client", "at": [520, 600, 0], "size": [320, 120], "title": "client" },
            { "kind": "card", "id": "api", "at": [1320, 600, 0], "size": [320, 120], "title": "api", "tone": "accent" },
            { "kind": "beam", "id": "link", "from": "client", "to": "api" },
            { "kind": "packet", "id": "request", "beam": "link" }
        ]
    }))?;
    let mut stage = StageActor::declare(&mut scene, "stage", &recipe)?;
    stage.settle_in(&mut scene, "client", 100 * MS);
    stage.settle_in(&mut scene, "api", 220 * MS);
    stage.connect(&mut scene, "link", 700 * MS, 0.5);

    // Text under the API card that changes its words in place.
    let below = |id: &str| AnchorPlan::stage(id, "api", Edge::Bottom).with_offset([0.0, 34.0]);
    let region = TextPlan::new("us-east-1", [0.0, 0.0])
        .size(22.0)
        .color(MUTED_INK)
        .anchor(below("api"));
    let mut cold = TextActor::declare(&mut scene, "region", &region)?;
    let mut warm = TextActor::declare(
        &mut scene,
        "region-warm",
        &TextPlan {
            text: "us-east-1 · warm".into(),
            ..region.clone()
        },
    )?;
    cold.show(&mut scene, 900 * MS);

    // A latency readout above the API card.
    let mut latency = RollingNumberActor::declare(
        &mut scene,
        "latency",
        RollingNumberPlan::new([0.0, 0.0], 28.0, "0")
            .aligned(CaptionAlign::Center)
            .tone(Tone::Accent)
            .prefix(vec![span("p95 ", Tone::Muted)])
            .suffix(vec![span(" ms", Tone::Muted)])
            .chip()
            .anchor(AnchorPlan::stage("api", "api", Edge::Top).with_offset([0.0, -44.0]))
            .roll(4800 * MS, "142")
            .roll(6500 * MS, "38"),
    )?;
    latency.show(&mut scene, 1100 * MS);

    // The request's caption sits under whichever card holds the request.
    let mut request = CaptionActor::declare(
        &mut scene,
        "request",
        &CaptionPlan::line(
            [0.0, 0.0],
            24.0,
            vec![span("GET ", Tone::Accent), span("/user", Tone::Plain)],
        )
        .aligned(CaptionAlign::Center)
        .chip()
        .anchor(AnchorPlan::stage("client", "client", Edge::Bottom).with_offset([0.0, 40.0]))
        .anchor(AnchorPlan::stage("api", "api", Edge::Bottom).with_offset([0.0, 84.0])),
    )?;
    request.type_in(&mut scene, 1300 * MS, 30.0, 0.6);

    // A callout rides along too, through the same shared anchors.
    let mut owner = CalloutActor::declare(
        &mut scene,
        "owner",
        &CalloutPlan::new(
            CalloutAnchorPlan::Stage {
                id: "client".into(),
                element: "client".into(),
                edge: Edge::Top,
                side: None,
            },
            vec![span("browser tab", Tone::Muted)],
        )
        .side(Edge::TopRight)
        .elbow()
        .reach(48.0)
        .tone(Tone::Muted),
    )?;
    owner.show(&mut scene, 1600 * MS);

    // Dolly toward the API; every pinned overlay follows its card.
    stage.to(&mut scene, "camera.z", 2400 * MS, 300.0, 1.4);
    stage.to(&mut scene, "camera.x", 2400 * MS, 140.0, 1.4);
    stage.to(&mut scene, "camera.y", 2400 * MS, -20.0, 1.4);

    // The request flies; its caption glides along beside the packet.
    let launch = 4000 * MS;
    let landed = stage.send(&mut scene, "request", launch, 0.7);
    request.move_to(&mut scene, "api", launch)?;
    stage.land(&mut scene, "api", landed);
    stage.jolt(&mut scene, landed, [1.0, 0.0], 0.9);
    cold.swap(&mut scene, &mut warm, landed);

    // A trace of the request hovers over the API card.
    let mut trace = ImageActor::declare(
        &mut scene,
        "trace",
        &ImagePlan::new("trace", [0.0, 0.0], 340.0)
            .titled("trace.png")
            .anchor(AnchorPlan::stage("api", "api", Edge::Top).with_offset([0.0, -236.0])),
        // Relative to the plan file, which `main` writes under `target/`.
        image::media("trace", "../assets/anchors/trace.png", 0, 10 * SECOND),
    )?;
    trace.fly_in(&mut scene, 5200 * MS);

    // Pull back out, then send the caption home and change its mind halfway:
    // the redirect carries the glide's velocity.
    stage.to(&mut scene, "camera.z", 7000 * MS, 0.0, 1.4);
    stage.to(&mut scene, "camera.x", 7000 * MS, 0.0, 1.4);
    stage.to(&mut scene, "camera.y", 7000 * MS, 0.0, 1.4);
    request.move_to(&mut scene, "client", 7900 * MS)?;
    request.move_to(&mut scene, "api", 8150 * MS)?;

    trace.hide(&mut scene, 9000 * MS);
    owner.hide(&mut scene, 9000 * MS);
    request.hide(&mut scene, 9100 * MS);
    latency.hide(&mut scene, 9100 * MS);
    warm.hide(&mut scene, 9100 * MS);

    scene.cue("pinned", 0, 10 * SECOND);
    Ok(scene.finish()?)
}

const FILE: [(&str, &str); 9] = [
    ("import", "import { Effect, Schedule } from \"effect\""),
    ("blank", " "),
    ("open", "const program = Effect.gen(function* () {"),
    ("log", "  yield* Effect.log(\"loading user\")"),
    ("id", "  const id = yield* UserId"),
    ("user", "  const user = yield* fetchUser(id).pipe("),
    ("retry", "    Effect.retry(Schedule.recurs(3))"),
    ("close", "  )"),
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

/// A line whose `target` part is its own Semantic Target range.
fn targeted_line(id: &str, lead: &str, target: Vec<StyledSpan>, tail: &str) -> EditorLinePlan {
    let part = |id: &str, spans: Vec<StyledSpan>| EditorPartPlan {
        id: id.into(),
        spans,
    };
    let mut parts = vec![
        part("lead", highlight::typescript(lead)),
        part("target", target),
    ];
    if !tail.is_empty() {
        parts.push(part("tail", highlight::typescript(tail)));
    }
    EditorLinePlan {
        id: id.into(),
        parts,
        semantic_ranges: vec![EditorSemanticRangePlan {
            id: id.into(),
            first_part_id: "target".into(),
            last_part_id: "target".into(),
        }],
        mark: None,
    }
}

/// Two lines are inserted above the targeted call; a caption and a Rolling
/// Number ride their code ranges down, then follow the panel's zoom.
pub fn build_editor() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("anchors-editor", 8 * SECOND);
    let ids = |ids: &[&str]| ids.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();
    let before = ids(&["import", "blank", "open", "user", "retry", "close", "end"]);
    let after = ids(&[
        "import", "blank", "open", "log", "id", "user", "retry", "close", "end",
    ]);
    let snapshots = diff::step_snapshots(&before, &after, 2400 * MS);
    let lines = FILE
        .iter()
        .map(|(id, text)| match *id {
            "user" => targeted_line(
                "user",
                "  const user = yield* ",
                vec![
                    StyledSpan::new("fetchUser", SyntaxStyle::Type),
                    StyledSpan::new("(id).pipe(", SyntaxStyle::Plain),
                ],
                "",
            ),
            "retry" => targeted_line(
                "retry",
                "    Effect.retry(",
                highlight::typescript("Schedule.recurs(3)"),
                ")",
            ),
            _ => code_line(id, text),
        })
        .chain(diff::gap_lines(&snapshots))
        .collect::<Vec<_>>();
    let recipe = EditorRecipePlan {
        file_name: "program.ts".into(),
        focus_line_id: "user".into(),
        lines,
        initial_line_ids: before,
        final_line_ids: after,
        snapshots,
        line_height: 44.0,
        entering_offset_x: 0.0,
        focus_height: 44.0,
        inline_reveal: None,
        additional_inline_reveals: Vec::new(),
    };
    let editor = scene.actor("editor", EDITOR_RECIPE, &recipe)?;
    for (id, line) in [("fetch", "user"), ("policy", "retry")] {
        scene.semantic_target(
            id,
            &editor,
            EditorTargetSelector {
                line_id: line.into(),
                range_id: line.into(),
            },
        )?;
    }
    let panel_y = scene.continuous(&editor, "panel-y", 70.0);
    let panel_opacity = scene.continuous(&editor, "panel-opacity", 0.0);
    scene.spring(&panel_y, 0, 0.0, 0.7, 0.0);
    scene.spring(&panel_opacity, 0, 1.0, 0.5, 0.0);

    let mut fails = CaptionActor::declare(
        &mut scene,
        "fails",
        &CaptionPlan::line(
            [0.0, 0.0],
            22.0,
            vec![
                span("can fail: ", Tone::Muted),
                span("UserNotFound", Tone::Error),
            ],
        )
        .anchor(AnchorPlan::editor("fetch", "fetch", Edge::Right).with_offset([32.0, 0.0])),
    )?;
    fails.show(&mut scene, 1000 * MS);

    let mut attempt = RollingNumberActor::declare(
        &mut scene,
        "attempt",
        RollingNumberPlan::new([0.0, 0.0], 24.0, "1")
            .tone(Tone::Warning)
            .prefix(vec![span("attempt ", Tone::Muted)])
            .suffix(vec![span(" of 3", Tone::Muted)])
            .chip()
            .anchor(AnchorPlan::editor("policy", "policy", Edge::Right).with_offset([64.0, 0.0]))
            .roll(4000 * MS, "2")
            .roll(4700 * MS, "3"),
    )?;
    attempt.show(&mut scene, 1400 * MS);

    // Zoom the panel toward the call; both labels follow the projection.
    let scale = scene.continuous(&editor, "panel-scale", 1.0);
    let panel_x = scene.continuous(&editor, "panel-x", 0.0);
    scene.ease(&scale, 5400 * MS, 1.12, 1.2, Ease::Smootherstep);
    scene.ease(&panel_x, 5400 * MS, -90.0, 1.2, Ease::Smootherstep);
    scene.spring(&panel_y, 5400 * MS, -40.0, 1.0, 0.0);
    fails.hide(&mut scene, 7200 * MS);
    attempt.hide(&mut scene, 7200 * MS);

    scene.cue("pinned", 0, 8 * SECOND);
    Ok(scene.finish()?)
}

/// A request travels through three participants; a cursor caption steps
/// from row to row as each arrow lands, a callout marks the query, and a
/// counter sits under the database header. Then the whole diagram slides and
/// everything pinned to it follows.
pub fn build_sequence() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("anchors-sequence", 7 * SECOND);
    let recipe = SequencePlan {
        origin: [260.0, 190.0],
        width: 1400.0,
        row_height: 96.0,
        slots: None,
        participants: vec![
            SequenceParticipantPlan::new("client", "client", "browser"),
            SequenceParticipantPlan::new("api", "api", "us-east-1"),
            SequenceParticipantPlan::new("db", "postgres", "primary"),
        ],
        rows: vec![
            SequenceRowPlan::message("request", "client", "api", "GET /user", Tone::Request),
            SequenceRowPlan::message("query", "api", "db", "SELECT user", Tone::Request),
            SequenceRowPlan::reply("rows", "db", "api", "1 row", Tone::Success),
            SequenceRowPlan::reply("done", "api", "client", "200 OK", Tone::Success),
        ],
    };
    let mut flow = SequenceActor::declare(&mut scene, "flow", &recipe)?;
    flow.animate(&mut scene, "opacity", 0.0, 0, 1.0, 0.5);

    let rows = ["request", "query", "rows", "done"];
    let mut cursor = CaptionActor::declare(
        &mut scene,
        "cursor",
        &rows.iter().fold(
            CaptionPlan::line(
                [0.0, 0.0],
                20.0,
                vec![span("← ", Tone::Accent), span("now", Tone::Muted)],
            ),
            |plan, row| {
                plan.anchor(
                    AnchorPlan::row(*row, "flow", *row, Edge::Right).with_offset([18.0, 0.0]),
                )
            },
        ),
    )?;
    let mut queries = RollingNumberActor::declare(
        &mut scene,
        "queries",
        RollingNumberPlan::new([0.0, 0.0], 22.0, "0")
            .aligned(CaptionAlign::Center)
            .tone(Tone::Accent)
            .prefix(vec![span("queries ", Tone::Muted)])
            .chip()
            .anchor(
                AnchorPlan::participant("db", "flow", "db", Edge::Bottom).with_offset([0.0, 34.0]),
            )
            .roll(1900 * MS, "1"),
    )?;
    queries.show(&mut scene, 700 * MS);
    let mut index = CalloutActor::declare(
        &mut scene,
        "index",
        &CalloutPlan::new(
            CalloutAnchorPlan::Row {
                id: "query".into(),
                sequence: "flow".into(),
                row: "query".into(),
                edge: Edge::Left,
                side: None,
            },
            vec![
                span("index scan ", Tone::Plain),
                span("2 ms", Tone::Success),
            ],
        )
        .side(Edge::TopLeft)
        .elbow()
        .reach(44.0)
        .tone(Tone::Success),
    )?;

    for (step, row) in rows.iter().enumerate() {
        let at = (800 + 800 * step as u64) * MS;
        flow.reveal(&mut scene, row, at);
        if step == 0 {
            cursor.show(&mut scene, at + 300 * MS);
        } else {
            cursor.move_to(&mut scene, row, at + 300 * MS)?;
        }
    }
    index.show(&mut scene, 2000 * MS);

    // The diagram slides; the cursor, the counter, and the callout follow.
    flow.animate(&mut scene, "x", 0.0, 4600 * MS, -110.0, 0.8);
    flow.animate(&mut scene, "y", 0.0, 4600 * MS, 60.0, 0.8);
    cursor.hide(&mut scene, 6200 * MS);
    queries.hide(&mut scene, 6200 * MS);
    index.hide(&mut scene, 6200 * MS);

    scene.cue("pinned", 0, 7 * SECOND);
    Ok(scene.finish()?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_showroom_builds() {
        let reel = super::build_reel().unwrap();
        assert_eq!(reel.segments.len(), 3);
    }
}
