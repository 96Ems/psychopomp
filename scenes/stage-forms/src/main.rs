//! Stage forms showroom: a write crosses a small diagram drawn from plain
//! shapes, icons, and arrowed paths, relaying through the gateway on one
//! packet. The store assembles from a flat dot matrix into a cube, tumbles,
//! and becomes a sphere above a slab of points; the cache it bypasses bursts.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, SECOND, seconds},
    math::easing::Ease,
    stage::{StageActor, StagePlan},
};

fn stage() -> Result<StagePlan> {
    let node = |id: &str, x: f32| {
        serde_json::json!({ "kind": "shape", "id": id, "at": [x, 500, 0],
            "shape": { "rect": [232, 132] }, "corner": 22, "fill": "surface" })
    };
    let icon = |id: &str, x: f32, icon: &str| serde_json::json!({ "kind": "icon", "id": id, "at": [x, 482, -1], "size": 46, "icon": icon });
    let label = |id: &str, at: [f32; 2], text: &str| {
        serde_json::json!({ "kind": "label", "id": id, "at": [at[0], at[1], -1], "size": 19,
            "spans": [{ "text": text, "tone": "muted" }] })
    };
    Ok(serde_json::from_value(serde_json::json!({
        "elements": [
            node("client", 330.0),
            icon("client-icon", 330.0, "desktop"),
            label("client-name", [330.0, 534.0], "client"),
            node("gateway", 900.0),
            icon("gateway-icon", 900.0, "shield-check"),
            label("gateway-name", [900.0, 534.0], "gateway"),
            { "kind": "shape", "id": "retry", "at": [900, 392, 0],
              "shape": { "arc": { "radius": 30, "start": 0.6, "sweep": 0.8 } },
              "arrow": "end", "width": 1.6 },
            { "kind": "form", "id": "store", "at": [1500, 470, 0], "points": 720,
              "shapes": [
                  { "shape": "plane", "size": [340, 184] },
                  { "shape": "box", "size": [210, 210, 210] },
                  { "shape": "sphere", "radius": 142 }
              ] },
            { "kind": "form", "id": "slab", "at": [1500, 690, 0], "points": 480, "tone": "muted",
              "shapes": [{ "shape": "box", "size": [420, 14, 250], "edges": 0.7 }] },
            label("store-name", [1500.0, 880.0], "store"),
            { "kind": "form", "id": "cache", "at": [900, 790, 0], "points": 360, "tone": "warning",
              "shapes": [{ "shape": "box", "size": [104, 104, 104], "edges": 0.6 }] },
            label("cache-name", [900.0, 905.0], "cache"),
            { "kind": "path", "id": "write", "through": ["client", "gateway", "store"],
              "arrow": "end" },
            { "kind": "path", "id": "fill", "through": ["gateway", "cache"],
              "arrow": "end", "dash": [6, 7] },
            { "kind": "packet", "id": "put", "beam": "write", "label": "PUT /doc", "tone": "request" }
        ]
    }))?)
}

fn main() -> Result<()> {
    let plan = stage()?;
    let mut scene = PlanBuilder::new("stage-forms", 16 * SECOND);
    let mut s = StageActor::declare(&mut scene, "stage", &plan)?;
    let sc = &mut scene;

    s.channel(sc, "camera.z", -90.0);
    s.to(sc, "camera.z", 0, 0.0, 2.2);

    // Nodes draw their outlines, then their glass and ink settle in.
    for (index, id) in ["client", "gateway"].into_iter().enumerate() {
        let at = seconds(0.2 + 0.12 * index as f64);
        s.channel(sc, &format!("{id}.draw"), 0.0);
        s.channel(sc, &format!("{id}.fill"), 0.0);
        s.ease(
            sc,
            &format!("{id}.draw"),
            at,
            1.0,
            0.7,
            Ease::CubicBezier([0.45, 0.0, 0.2, 1.0]),
        );
        s.ease(
            sc,
            &format!("{id}.fill"),
            at + seconds(0.35),
            1.0,
            0.5,
            Ease::Smootherstep,
        );
        for ink in ["icon", "name"] {
            s.channel(sc, &format!("{id}-{ink}.opacity"), 0.0);
            s.ease(
                sc,
                &format!("{id}-{ink}.opacity"),
                at + seconds(0.5),
                1.0,
                0.4,
                Ease::Smootherstep,
            );
        }
    }
    // The store arrives as a flat dot matrix over its slab.
    for (id, at) in [("slab", 0.5), ("store", 0.7), ("cache", 0.9)] {
        s.channel(sc, &format!("{id}.opacity"), 0.0);
        s.ease(
            sc,
            &format!("{id}.opacity"),
            seconds(at),
            1.0,
            0.8,
            Ease::Smootherstep,
        );
    }
    for (id, at) in [("store-name", 1.1), ("cache-name", 1.2)] {
        s.channel(sc, &format!("{id}.opacity"), 0.0);
        s.ease(
            sc,
            &format!("{id}.opacity"),
            seconds(at),
            1.0,
            0.4,
            Ease::Smootherstep,
        );
    }

    // Arrows draw on, their heads riding the tip.
    s.channel(sc, "write.draw", 0.0);
    let wired = s.connect(sc, "write", seconds(1.4), 1.1);
    s.channel(sc, "fill.draw", 0.0);
    s.connect(sc, "fill", seconds(1.9), 0.6);
    s.channel(sc, "retry.draw", 0.0);
    s.ease(
        sc,
        "retry.draw",
        seconds(2.2),
        1.0,
        0.6,
        Ease::CubicBezier([0.45, 0.0, 0.2, 1.0]),
    );

    // One packet relays through the gateway into the flat store.
    let hops = s.relay(sc, "put", wired + seconds(0.5), 1.3);
    let landed = hops[hops.len() - 1];

    // The dot matrix assembles into a cube, which tumbles a quarter turn.
    let cube = s.morph(sc, "store", landed + seconds(0.3), 1, 1.7);
    s.ease(
        sc,
        "store.pitch",
        cube + seconds(0.2),
        std::f32::consts::FRAC_PI_2,
        1.4,
        Ease::Smootherstep,
    );

    // A second write lands in the cube, which rounds into a sphere.
    let hops = s.relay(sc, "put", cube + seconds(1.4), 1.1);
    let landed = hops[hops.len() - 1];
    let sphere = s.morph(sc, "store", landed + seconds(0.25), 2, 1.8);

    // The bypassed cache bursts; the scene takes the blow.
    let burst = sphere + seconds(0.6);
    s.clock_for(sc, "cache.burst", burst, 5.2);
    s.jolt(sc, burst + seconds(0.12), [0.0, 1.0], 0.5);
    s.channel(sc, "fill.opacity", 1.0);
    s.to(sc, "fill.opacity", burst + seconds(0.3), 0.0, 0.8);
    s.to(sc, "cache-name.opacity", burst + seconds(0.3), 0.0, 0.8);
    s.to(sc, "camera.z", burst + seconds(1.4), 110.0, 1.8);
    s.to(sc, "camera.x", burst + seconds(1.4), 220.0, 1.8);

    scene.finish()?.write_or_print(Some(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "target/stage-forms.json".into()),
    ))?;
    Ok(())
}
