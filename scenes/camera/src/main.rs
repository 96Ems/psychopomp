//! Camera showroom: a Stage diagram shot like a film. The camera establishes,
//! frames two cards, follows a packet across, racks focus to a far plane,
//! orbits a particle orb, dolly-zooms on an impact, whips to another cluster,
//! sways handheld on a quiet beat, and pushes in on the resolution. Every
//! move is a `CameraRig` shot; the plan is ordinary `camera.*` channels.
//!
//! Writes `target/camera.json` (or the given path).
use psychopomp::{
    author::{PlanBuilder, SECOND, seconds},
    caption::{CaptionActor, CaptionPlan, CaptionSpanPlan},
    math::{lerp, random::hash},
    stage::{Camera, Move, StageActor, StageElement, StagePlan},
    tone::Tone,
};
use serde_json::json;

/// Rings far behind the core and along the whip, and a little dust in front
/// of it, so depth reads when the camera turns, stretches, and streaks.
const FIELD: u32 = 12;
const DUST: [[f32; 3]; 3] = [
    [700.0, 1080.0, -40.0],
    [1420.0, 1600.0, -80.0],
    [1380.0, 1020.0, 20.0],
];

fn stage_plan() -> anyhow::Result<StagePlan> {
    let mut plan: StagePlan = serde_json::from_value(json!({
        "elements": [
            // The request: two cards on one wire, and a far archive behind them.
            { "kind": "card", "id": "client", "at": [560, 560, 0], "size": [300, 110], "title": "client",
              "status": [{ "text": "ready" }, { "text": "sent", "tone": "request" }] },
            { "kind": "card", "id": "api", "at": [1380, 520, 0], "size": [300, 110], "title": "api",
              "status": [{ "text": "listening" }, { "text": "200 ok", "tone": "success" }] },
            { "kind": "card", "id": "archive", "at": [900, 20, 900], "size": [420, 140], "title": "archive",
              "status": [{ "text": "cold storage", "tone": "muted" }] },
            { "kind": "beam", "id": "wire", "from": "client", "to": "api", "bend": 30 },
            { "kind": "packet", "id": "request", "beam": "wire", "label": "POST /orders", "tone": "request" },
            // The core: an orb with workers in front of and behind it.
            { "kind": "orb", "id": "core", "at": [1060, 1320, 400], "radius": 150 },
            { "kind": "ring", "id": "halo", "at": [1060, 1320, 400], "radius": 205, "thickness": 1.5, "tone": "muted" },
            { "kind": "card", "id": "worker-a", "at": [620, 1240, 120], "size": [240, 96], "title": "worker 1" },
            { "kind": "card", "id": "worker-b", "at": [1560, 1440, 760], "size": [240, 96], "title": "worker 2" },
            { "kind": "beam", "id": "feed-a", "from": "worker-a", "to": "core" },
            { "kind": "beam", "id": "feed-b", "from": "worker-b", "to": "core" },
            { "kind": "packet", "id": "strike", "beam": "feed-a", "tone": "error" },
            // The queue, far to the right.
            { "kind": "card", "id": "queue", "at": [3300, 520, 0], "size": [300, 110], "title": "queue",
              "status": [{ "text": "3 jobs" }, { "text": "draining", "tone": "warning" }, { "text": "empty", "tone": "success" }] },
            { "kind": "card", "id": "drain", "at": [3980, 600, 160], "size": [300, 110], "title": "drain",
              "status": [{ "text": "idle" }, { "text": "working", "tone": "request" }, { "text": "resolved", "tone": "success" }] },
            { "kind": "ring", "id": "clock", "at": [3700, 220, 700], "radius": 90, "thickness": 3, "tone": "muted" },
            { "kind": "beam", "id": "lane", "from": "queue", "to": "drain", "bend": -24 },
            { "kind": "packet", "id": "job", "beam": "lane", "label": "job 3", "tone": "request" }
        ]
    }))?;
    for i in 0..FIELD {
        let h = |salt| hash(i, salt);
        plan.elements.push(StageElement::Ring {
            id: format!("far-{i}"),
            at: [
                lerp(-500.0, 2700.0, (i as f32 + h(1)) / FIELD as f32),
                lerp(500.0, 2100.0, h(2)),
                lerp(1500.0, 2600.0, h(3)),
            ],
            radius: lerp(50.0, 130.0, h(4)),
            thickness: 2.5,
            tone: Tone::Muted,
        });
    }
    for (i, at) in DUST.into_iter().enumerate() {
        plan.elements.push(StageElement::Ring {
            id: format!("dust-{i}"),
            at,
            radius: 18.0 + 8.0 * i as f32,
            thickness: 2.0,
            tone: Tone::Muted,
        });
    }
    Ok(plan)
}

/// Cross-fade a card's status line to entry `index`.
fn status(stage: &mut StageActor, scene: &mut PlanBuilder, card: &str, at: u64, index: f32) {
    stage.glide(scene, &format!("{card}.status"), at, index, 0.4);
}

/// A shot name over the frame, like a slate: the method in the accent.
fn slate(
    scene: &mut PlanBuilder,
    id: &str,
    method: &str,
    note: &str,
    from: u64,
    to: u64,
) -> anyhow::Result<()> {
    let mut caption = CaptionActor::declare(
        scene,
        id,
        &CaptionPlan::line(
            [96.0, 990.0],
            26.0,
            vec![
                CaptionSpanPlan::new(format!("camera.{method}"), Tone::Accent),
                CaptionSpanPlan::new(format!("  {note}"), Tone::Muted),
            ],
        )
        .chip(),
    )?;
    caption.show(scene, from);
    caption.hide(scene, to);
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let plan = stage_plan()?;
    let mut scene = PlanBuilder::new("camera", seconds(32.4));
    let sc = &mut scene;
    let mut stage = StageActor::declare(sc, "stage", &plan)?;
    let s = &mut stage;
    let camera = s.camera();
    s.channel(sc, "camera.dof", 0.4);
    let depth_cues = (0..FIELD)
        .map(|i| format!("far-{i}"))
        .chain((0..DUST.len()).map(|i| format!("dust-{i}")))
        .chain(["core".into(), "halo".into()])
        .collect::<Vec<_>>();
    for element in &depth_cues {
        s.channel(sc, &format!("{element}.opacity"), 0.0);
    }

    // Establish: start on the request's whole composition, pulled back, and
    // dolly in while the cards settle.
    let wide = camera.framing(sc, &["client", "api", "archive"], 230.0, 0)?;
    camera.move_to(sc, &wide, 0, Move::Cut);
    let established = camera.establish(sc, 0, 380.0, 2.6);
    for (index, card) in ["archive", "client", "api"].into_iter().enumerate() {
        s.settle_in(sc, card, seconds(0.3 + 0.16 * index as f64));
    }
    s.connect(sc, "wire", seconds(1.4), 0.7);
    slate(
        sc,
        "slate-establish",
        "establish",
        "dolly in to open",
        seconds(0.4),
        seconds(3.0),
    )?;

    // Frame two cards, then lean in close on the client.
    let framed = camera.frame(
        sc,
        &["client", "api"],
        150.0,
        established - seconds(0.4),
        Move::Spring(1.6),
    )?;
    slate(
        sc,
        "slate-frame",
        "frame",
        "fit two cards with padding",
        seconds(3.3),
        seconds(5.4),
    )?;
    let close = camera.frame(
        sc,
        &["client"],
        440.0,
        framed + seconds(0.4),
        Move::Spring(1.4),
    )?;

    // Follow the request across: the renderer keeps it centered every sample.
    let launch = close + seconds(0.3);
    status(s, sc, "client", launch, 1.0);
    camera.follow(sc, "request", launch - seconds(0.2), Move::Spring(0.7))?;
    let landed = s.send(sc, "request", launch, 1.9);
    s.land(sc, "api", landed);
    status(s, sc, "api", landed, 1.0);
    slate(
        sc,
        "slate-follow",
        "follow",
        "track a packet in flight",
        launch - seconds(0.2),
        landed + seconds(0.6),
    )?;
    let held = camera.release(sc, landed + seconds(0.2), Move::Spring(1.0))?;

    // Rack focus: pull back to see the archive, then move the focal plane to it.
    let pulled = camera.frame(
        sc,
        &["client", "api", "archive"],
        170.0,
        held,
        Move::Spring(1.6),
    )?;
    camera.aperture(sc, held, 1.0, Move::Spring(1.0));
    let racked = camera.focus_on(sc, "archive", pulled - seconds(0.4), Move::Spring(1.3))?;
    slate(
        sc,
        "slate-focus",
        "focus_on",
        "rack focus to the far plane",
        pulled - seconds(0.5),
        racked + seconds(1.0),
    )?;

    // Down to the core; orbit it so the workers parallax around its depth.
    let descend = racked + seconds(1.2);
    for (index, card) in ["worker-a", "worker-b"].into_iter().enumerate() {
        s.settle_in(sc, card, descend + seconds(0.5 + 0.16 * index as f64));
    }
    for element in &depth_cues {
        s.to(
            sc,
            &format!("{element}.opacity"),
            descend + seconds(0.3),
            1.0,
            1.2,
        );
    }
    s.connect(sc, "feed-a", descend + seconds(1.0), 0.6);
    s.connect(sc, "feed-b", descend + seconds(1.15), 0.6);
    let arrived = camera.frame(
        sc,
        &["core", "worker-a", "worker-b"],
        140.0,
        descend,
        Move::Spring(1.8),
    )?;
    camera.focus_on(sc, "core", descend, Move::Spring(1.4))?;
    camera.aperture(sc, descend, 0.45, Move::Spring(1.4));
    let orbited = camera.orbit(
        sc,
        "core",
        arrived - seconds(0.2),
        [0.48, -0.16],
        Move::Glide(3.4),
    )?;
    slate(
        sc,
        "slate-orbit",
        "orbit",
        "swing around the core",
        arrived - seconds(0.2),
        orbited,
    )?;

    // The impact: a strike lands, the camera takes the hit, and the world
    // stretches away behind the core while it holds still.
    let strike = s.send(sc, "strike", orbited + seconds(0.3), 0.75);
    s.land(sc, "core", strike);
    s.hit(sc, "core.hurt", strike, 0.8, 0.0);
    s.jolt(sc, strike, [1.0, -0.4], 0.8);
    let vertigo = camera.dolly_zoom(sc, "core", strike, 560.0, Move::Glide(2.4))?;
    camera.roll(sc, strike + seconds(0.1), 0.04, Move::Glide(1.6));
    slate(
        sc,
        "slate-vertigo",
        "dolly_zoom",
        "hold the subject, stretch the depth",
        strike,
        vertigo + seconds(0.4),
    )?;

    // Whip to the queue: level the lens and streak across in 0.6 s.
    let whip = vertigo + seconds(0.7);
    let level = Camera {
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        zoom: 1.0,
        pivot: 0.0,
        ..camera.pose(sc, whip)
    };
    camera.move_to(sc, &level, whip, Move::Glide(0.6));
    camera.focus_on(sc, "queue", whip, Move::Glide(0.6))?;
    let landed_b = camera.whip(sc, &["queue", "drain"], 170.0, whip, 0.6)?;
    for (index, card) in ["queue", "drain"].into_iter().enumerate() {
        s.settle_in(sc, card, whip + seconds(0.25 + 0.14 * index as f64));
    }
    slate(
        sc,
        "slate-whip",
        "whip",
        "streak to another cluster",
        whip,
        landed_b + seconds(0.8),
    )?;

    // A quiet beat: the operator breathes and drifts toward the drain.
    let quiet = landed_b + seconds(0.6);
    camera.handheld(sc, quiet, 1.0, 1.0);
    camera.drift(sc, "drain", quiet + seconds(0.3), 4.2)?;
    let lane = s.connect(sc, "lane", quiet + seconds(0.4), 0.7);
    status(s, sc, "queue", lane, 1.0);
    let job = s.send(sc, "job", lane + seconds(0.5), 1.2);
    s.land(sc, "drain", job);
    status(s, sc, "drain", job, 1.0);
    slate(
        sc,
        "slate-handheld",
        "handheld",
        "sway on a quiet beat",
        quiet,
        job + seconds(0.8),
    )?;

    // The resolution: steady the lens and push in on it.
    let resolved = job + seconds(1.0);
    status(s, sc, "drain", resolved, 2.0);
    status(s, sc, "queue", resolved, 2.0);
    s.land(sc, "drain", resolved);
    camera.handheld(sc, resolved, 0.0, 1.4);
    let push = camera.frame(
        sc,
        &["drain"],
        425.0,
        resolved + seconds(0.2),
        Move::Glide(3.6),
    )?;
    slate(
        sc,
        "slate-push",
        "frame",
        "push in on the resolution",
        resolved + seconds(0.2),
        push + seconds(0.8),
    )?;
    assert!(
        push + SECOND <= sc.duration_nanos(),
        "the push settles at {:.2}s, too close to the end",
        push as f64 / 1e9
    );

    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/camera.json".into());
    std::fs::create_dir_all("target")?;
    scene.finish()?.write_or_print(Some(&output))?;
    eprintln!("Wrote {output}");
    Ok(())
}
