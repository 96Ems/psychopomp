//! Effects showroom: each Stage effect in a small explainer beat, captioned
//! with the `StageActor` call that writes it. A build charges and zaps a
//! deploy; a shield blocks an attack but lets a request through; a stale
//! config burns away, its replacement materializes and is scanned; a live
//! link hums. Quiet between beats; every light has a source.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, seconds},
    math::easing::Ease,
    plan::{ReelPlan, ReelSegmentPlan, ReelTransitionStyle, ScenePlan},
    stage::{StageActor, StagePlan},
};
use serde_json::{Value, json};

const DIP: f64 = 0.6;

/// A caption in the terminal voice: `stage.<call>(<args>)`, typed in at the
/// bottom of the frame.
fn caption(id: &str, call: &str, args: &str, y: f32) -> Value {
    json!({ "kind": "label", "id": id, "at": [960, y, 0], "size": 26, "spans": [
        { "text": "stage.", "tone": "muted" },
        { "text": call, "tone": "accent" },
        { "text": format!("({args})") }
    ] })
}

fn stage(elements: Vec<Value>) -> Result<StagePlan> {
    Ok(serde_json::from_value(json!({ "elements": elements }))?)
}

/// The hero entrance for an orb: it grows, sharpens, and turns into place.
fn orb_in(stage: &mut StageActor, scene: &mut PlanBuilder, orb: &str, at: u64) {
    for (property, from) in [("scale", 0.58), ("blur", 11.0), ("rotation", -1.8)] {
        let channel = stage.channel(scene, &format!("{orb}.{property}"), from);
        scene.set(&channel, at, from);
    }
    stage.bounce(scene, &format!("{orb}.scale"), at, 1.0, 0.85, 0.2);
    stage.to(scene, &format!("{orb}.blur"), at, 0.0, 0.7);
    stage.ease(
        scene,
        &format!("{orb}.rotation"),
        at,
        0.0,
        1.25,
        Ease::CubicOut,
    );
    let opacity = stage.channel(scene, &format!("{orb}.opacity"), 0.0);
    scene.ease(&opacity, at, 1.0, 0.3, Ease::Smootherstep);
}

/// Type a caption in at `at`; fade it out at `out`, if given.
fn say(stage: &mut StageActor, scene: &mut PlanBuilder, label: &str, at: u64, out: Option<u64>) {
    stage.type_in(scene, label, at, 46.0);
    if let Some(out) = out {
        stage.ease(
            scene,
            &format!("{label}.opacity"),
            out,
            0.0,
            0.25,
            Ease::Smoothstep,
        );
    }
}

/// A build charges until it arcs over to the deploy it feeds.
fn strike() -> Result<ScenePlan> {
    let plan = stage(vec![
        json!({ "kind": "card", "id": "build", "at": [560, 500, 0], "size": [300, 120], "title": "build",
                "status": [{ "text": "idle" }, { "text": "charging", "tone": "warning" },
                           { "text": "shipped", "tone": "success" }] }),
        json!({ "kind": "orb", "id": "deploy", "at": [1380, 500, 0], "radius": 130, "points": 900 }),
        json!({ "kind": "bolt", "id": "strike", "from": "build", "to": "deploy" }),
        caption("charge", "charge", "\"build\", 1.0", 880.0),
        caption("zap", "zap", "\"strike\", at", 880.0),
    ])?;
    let mut scene = PlanBuilder::new("strike", seconds(6.4));
    let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;
    stage.settle_in(&mut scene, "build", 0);
    orb_in(&mut stage, &mut scene, "deploy", seconds(0.12));
    for label in ["charge", "zap"] {
        stage.channel(&mut scene, &format!("{label}.opacity"), 0.0);
    }
    stage.to(&mut scene, "camera.z", 0, 90.0, 2.2);
    let charge = seconds(1.5);
    say(
        &mut stage,
        &mut scene,
        "charge",
        charge - seconds(0.5),
        Some(seconds(2.55)),
    );
    stage.to(&mut scene, "build.status", charge, 1.0, 0.4);
    stage.charge(&mut scene, "build", charge, 1.0, 1.4);
    say(&mut stage, &mut scene, "zap", seconds(2.75), None);
    let contact = stage.zap(&mut scene, "strike", seconds(3.3));
    // The charge leaves with the bolt; the deploy takes the hit.
    stage.charge(&mut scene, "build", contact, 0.0, 0.0);
    stage.land(&mut scene, "deploy", contact);
    stage.jolt(&mut scene, contact, [1.0, 0.0], 0.45);
    stage.to(&mut scene, "build.status", contact + seconds(0.3), 2.0, 0.4);
    Ok(scene.finish()?)
}

/// A shield rises around a gateway; an attack breaks on it, a request
/// passes through.
fn shield() -> Result<ScenePlan> {
    let plan = stage(vec![
        json!({ "kind": "card", "id": "intruder", "at": [470, 300, 0], "size": [260, 104], "title": "intruder",
                "tone": "error" }),
        json!({ "kind": "card", "id": "client", "at": [470, 760, 0], "size": [260, 104], "title": "client" }),
        json!({ "kind": "orb", "id": "gateway", "at": [1300, 520, 0], "radius": 105, "points": 700 }),
        json!({ "kind": "shield", "id": "firewall", "around": "gateway", "radius": 215 }),
        json!({ "kind": "beam", "id": "link", "from": "client", "to": "gateway", "bend": -30 }),
        json!({ "kind": "packet", "id": "request", "beam": "link", "label": "GET /orders", "tone": "request" }),
        json!({ "kind": "bolt", "id": "attack", "from": "intruder", "to": "firewall", "strikes": 4,
                "tone": "error" }),
        caption("raise", "raise", "\"firewall\", at, 0.8", 960.0),
        caption("zap", "zap", "\"attack\", at", 960.0),
    ])?;
    let mut scene = PlanBuilder::new("shield", seconds(7.6));
    let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;
    stage.settle_in(&mut scene, "intruder", 0);
    stage.settle_in(&mut scene, "client", seconds(0.12));
    orb_in(&mut stage, &mut scene, "gateway", seconds(0.2));
    for label in ["raise", "zap"] {
        stage.channel(&mut scene, &format!("{label}.opacity"), 0.0);
    }
    stage.to(&mut scene, "camera.z", 0, 70.0, 2.2);
    stage.connect(&mut scene, "link", seconds(0.7), 0.6);
    say(
        &mut stage,
        &mut scene,
        "raise",
        seconds(1.0),
        Some(seconds(2.5)),
    );
    stage.raise(&mut scene, "firewall", seconds(1.5), 0.8);
    say(&mut stage, &mut scene, "zap", seconds(2.75), None);
    stage.charge(&mut scene, "intruder", seconds(2.7), 0.8, 0.6);
    let contact = stage.zap(&mut scene, "attack", seconds(3.4));
    stage.charge(&mut scene, "intruder", contact, 0.0, 0.0);
    stage.jolt(&mut scene, contact, [0.6, 0.8], 0.3);
    let arrival = stage.send(&mut scene, "request", seconds(5.0), 0.95);
    stage.land(&mut scene, "gateway", arrival);
    Ok(scene.finish()?)
}

/// A stale config burns away; its replacement forms in the same slot, then a
/// scan verifies it.
fn replace() -> Result<ScenePlan> {
    let card = |id: &str, title: &str, status: Value| {
        json!({ "kind": "card", "id": id, "at": [960, 470, 0], "size": [380, 140], "title": title,
                "status": status })
    };
    let plan = stage(vec![
        card(
            "old",
            "config v1",
            json!([{ "text": "stale", "tone": "warning" }]),
        ),
        card(
            "new",
            "config v2",
            json!([{ "text": "unverified" }, { "text": "verified", "tone": "success" }]),
        ),
        caption("dissolve", "dissolve", "\"old\", at", 800.0),
        caption("materialize", "materialize", "\"new\", at, 1.6", 800.0),
        caption("scan", "scan", "\"new\", at, 1.1", 800.0),
    ])?;
    let mut scene = PlanBuilder::new("replace", seconds(8.0));
    let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;
    stage.settle_in(&mut scene, "old", 0);
    stage.to(&mut scene, "camera.z", 0, 380.0, 2.2);
    for label in ["dissolve", "materialize", "scan"] {
        stage.channel(&mut scene, &format!("{label}.opacity"), 0.0);
    }
    say(
        &mut stage,
        &mut scene,
        "dissolve",
        seconds(0.8),
        Some(seconds(2.7)),
    );
    let gone = stage.dissolve(&mut scene, "old", seconds(1.4));
    // The new card waits as cold ash until it forms.
    stage.channel(
        &mut scene,
        "new.dissolve",
        psychopomp::effects::dissolve::DURATION,
    );
    let formed = gone + seconds(0.8);
    say(
        &mut stage,
        &mut scene,
        "materialize",
        formed - seconds(0.5),
        Some(formed + seconds(1.45)),
    );
    let whole = stage.materialize(&mut scene, "new", formed, 1.6);
    say(&mut stage, &mut scene, "scan", whole + seconds(0.25), None);
    let scanned = stage.scan(&mut scene, "new", whole + seconds(0.75), 1.1);
    stage.to(&mut scene, "new.status", scanned - seconds(0.15), 1.0, 0.4);
    stage.land(&mut scene, "new", scanned - seconds(0.1));
    Ok(scene.finish()?)
}

/// A live link hums between a client and the service it streams from.
fn hum() -> Result<ScenePlan> {
    let plan = stage(vec![
        json!({ "kind": "card", "id": "viewer", "at": [560, 520, 0], "size": [280, 112], "title": "viewer",
                "status": [{ "text": "connecting" }, { "text": "live", "tone": "success" }] }),
        json!({ "kind": "orb", "id": "stream", "at": [1380, 520, 0], "radius": 120, "points": 800 }),
        json!({ "kind": "bolt", "id": "live", "from": "viewer", "to": "stream", "branching": 0.5 }),
        caption("hum", "hum", "\"live\", at, 0.8", 880.0),
    ])?;
    let mut scene = PlanBuilder::new("hum", seconds(6.0));
    let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;
    stage.settle_in(&mut scene, "viewer", 0);
    orb_in(&mut stage, &mut scene, "stream", seconds(0.12));
    stage.channel(&mut scene, "hum.opacity", 0.0);
    stage.to(&mut scene, "camera.z", 0, 90.0, 2.2);
    say(&mut stage, &mut scene, "hum", seconds(1.0), None);
    let live = seconds(1.5);
    stage.hum(&mut scene, "live", live, 0.85, 0.35);
    stage.charge(&mut scene, "stream", live, 0.35, 0.6);
    stage.to(&mut scene, "viewer.status", live + seconds(0.2), 1.0, 0.4);
    // Disconnect: the arc dies away and the frame is quiet again.
    stage.hum(&mut scene, "live", seconds(4.4), 0.0, 0.3);
    stage.charge(&mut scene, "stream", seconds(4.4), 0.0, 0.5);
    stage.to(&mut scene, "viewer.status", seconds(4.5), 0.0, 0.4);
    Ok(scene.finish()?)
}

pub fn build_reel() -> Result<ReelPlan> {
    let dip = |plan| ReelSegmentPlan {
        transition_nanos: seconds(DIP),
        transition_style: ReelTransitionStyle::Dip,
        transition_focus: None,
        transition_wipe: None,
        plan,
    };
    let reel = ReelPlan {
        version: ReelPlan::VERSION,
        id: "effects-showroom".to_owned(),
        segments: vec![
            ReelSegmentPlan {
                transition_nanos: 0,
                ..dip(strike()?)
            },
            dip(shield()?),
            dip(replace()?),
            dip(hum()?),
        ],
    };
    reel.validate()?;
    Ok(reel)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_showroom_reel_validates() {
        let reel = super::build_reel().unwrap();
        assert_eq!(reel.segments.len(), 4);
    }
}
