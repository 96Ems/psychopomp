//! The theory, as one evidence board the camera whips across: balls with dots
//! are ATOMS (the dots fly off into orbits, annotated like a figure), balls
//! with thoughts are HOPES & DREAMS (which float up), thoughts and dots are
//! the SAME THING (a spinning cycle, rewound), and dots are BASKETBALLS (a
//! dribble, a shot, a swish, confetti). Then the board: red string, pins,
//! and a quake building into the next shout. The camera bumps on every drum.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, seconds},
    confetti::{ConfettiActor, ConfettiPlan},
    face::Face,
    math::{easing::Ease, lerp, random::hash},
    stage::StageActor,
    subtitles::SubtitlesPlan,
    tone::Tone,
};
use serde_json::{Value, json};

use crate::{
    Shot,
    kit::{Angle, FALL, Later, RISE, Window, jitter, label, orb, stage, subtitles, typed, words},
    sound::Film,
};

/// A whip pan between exhibits takes this long.
pub const WHIP: f64 = 0.36;

/// Exhibit centers: atoms, hopes and dreams, the cycle, basketballs.
const A: [f32; 2] = [960.0, 540.0];
const B: [f32; 2] = [3260.0, 540.0];
const C: [f32; 2] = [3260.0, 2040.0];
const D: [f32; 2] = [960.0, 2040.0];
/// Where the camera starts, close on the nucleus and turning.
const OPEN: Angle = Angle {
    x: 0.0,
    y: -30.0,
    z: 520.0,
    yaw: -0.35,
    pitch: 0.1,
    roll: 0.06,
};
const NUCLEUS: f32 = 112.0;
const ORBITS: [(f32, f32); 3] = [(0.35, 0.0), (0.629, 0.951), (-0.629, -0.951)];
const PINS: u32 = 26;
/// The basketball's parts, which travel together.
const BASKETBALL: [&str; 5] = ["d-ball", "d-seam-v", "d-seam-h", "d-seam-l", "d-seam-r"];
/// Where the ball rests, and the rim it drops through.
const DRIBBLE: [f32; 2] = [-260.0, 150.0];
const RIM: [f32; 2] = [330.0, -190.0];

fn at(center: [f32; 2], dx: f32, dy: f32, z: f32) -> [f32; 3] {
    [center[0] + dx, center[1] + dy, z]
}

fn shape(id: &str, at: [f32; 3], figure: Value, extra: Value) -> Value {
    let mut value = json!({ "kind": "shape", "id": id, "at": at, "shape": figure });
    if let (Some(value), Some(extra)) = (value.as_object_mut(), extra.as_object()) {
        value.extend(extra.clone());
    }
    value
}

fn icon(id: &str, at: [f32; 3], size: f32, icon: &str, tone: &str) -> Value {
    json!({ "kind": "icon", "id": id, "at": at, "size": size, "icon": icon, "tone": tone })
}

fn left(mut value: Value) -> Value {
    value["align"] = json!("left");
    value
}

fn path(id: &str, through: Value, tone: &str, width: f32, arrow: &str) -> Value {
    json!({ "kind": "path", "id": id, "through": through, "tone": tone, "width": width, "arrow": arrow })
}

fn elements() -> Vec<Value> {
    let mut elements = vec![
        // Exhibit A: a ball whose dots fly out into orbits, as a figure.
        orb("a-ball", at(A, 0.0, -30.0, 0.0), NUCLEUS, 1100, "accent"),
        typed(
            "a-word",
            at(A, 0.0, -350.0, -80.0),
            150.0,
            Face::Shout,
            &[("ATOMS", "accent")],
        ),
        left(label(
            "a-fig",
            at(A, -560.0, -330.0, 0.0),
            34.0,
            &[("FIG. 1  ", "error"), ("THE BALL", "muted")],
        )),
        path(
            "a-lead-1",
            json!([
                [A[0] - 420.0, A[1] + 120.0, 0.0],
                [A[0] - 250.0, A[1] + 120.0, 0.0],
                [A[0] - 90.0, A[1] + 10.0, 0.0]
            ]),
            "plain",
            2.5,
            "end",
        ),
        typed(
            "a-note-1",
            at(A, -560.0, 120.0, 0.0),
            44.0,
            Face::SerifItalic,
            &[("ball = nucleus", "accent")],
        ),
        path(
            "a-lead-2",
            json!([
                [A[0] + 420.0, A[1] + 120.0, 0.0],
                [A[0] + 330.0, A[1] + 120.0, 0.0],
                [A[0] + 290.0, A[1] + 40.0, 0.0]
            ]),
            "plain",
            2.5,
            "end",
        ),
        typed(
            "a-note-2",
            at(A, 580.0, 120.0, 0.0),
            44.0,
            Face::SerifItalic,
            &[("dots = electrons", "request")],
        ),
        // Exhibit B: a ball, dreaming.
        orb("b-ball", at(B, -470.0, 170.0, 0.0), 120.0, 1000, "accent"),
        orb("b-bub-1", at(B, -320.0, 40.0, 0.0), 14.0, 40, "plain"),
        orb("b-bub-2", at(B, -250.0, -30.0, 0.0), 22.0, 70, "plain"),
        orb("b-bub-3", at(B, -160.0, -100.0, 0.0), 32.0, 110, "plain"),
        icon(
            "b-cloud",
            at(B, 210.0, -110.0, 0.0),
            600.0,
            "cloud",
            "plain",
        ),
        typed(
            "b-hopes",
            at(B, 240.0, -115.0, -1.0),
            88.0,
            Face::Shout,
            &[("HOPES", "warning")],
        ),
        typed(
            "b-dreams",
            at(B, 240.0, -35.0, -1.0),
            60.0,
            Face::SerifItalic,
            &[("& dreams", "warning")],
        ),
        icon(
            "b-star-1",
            at(B, 590.0, -400.0, -10.0),
            90.0,
            "sparkle",
            "warning",
        ),
        icon(
            "b-star-2",
            at(B, -120.0, -360.0, -10.0),
            60.0,
            "sparkle",
            "warning",
        ),
        icon(
            "b-star-3",
            at(B, 640.0, 160.0, -10.0),
            50.0,
            "sparkle",
            "accent",
        ),
        // Exhibit C: thoughts are dots are thoughts, around and around.
        icon(
            "c-thought",
            at(C, -330.0, -40.0, 0.0),
            260.0,
            "cloud",
            "plain",
        ),
        orb("c-dots", at(C, 330.0, -40.0, 0.0), 100.0, 900, "accent"),
        orb("c-ball", at(C, -330.0, -40.0, 0.0), 100.0, 900, "accent"),
        shape(
            "c-loop",
            at(C, 0.0, -40.0, -2.0),
            json!({ "arc": { "radius": 330, "start": 0.83, "sweep": 0.34 } }),
            json!({ "stroke": "error", "width": 7, "arrow": "end" }),
        ),
        shape(
            "c-loop-back",
            at(C, 0.0, -40.0, -2.0),
            json!({ "arc": { "radius": 330, "start": 0.33, "sweep": 0.34 } }),
            json!({ "stroke": "error", "width": 7, "arrow": "end" }),
        ),
        typed(
            "c-thoughts",
            at(C, -330.0, 120.0, 0.0),
            56.0,
            Face::Light,
            &[("THOUGHTS", "plain")],
        ),
        typed(
            "c-word",
            at(C, 330.0, 120.0, 0.0),
            56.0,
            Face::Shout,
            &[("DOTS", "accent")],
        ),
        typed(
            "c-same",
            at(C, 0.0, -380.0, -60.0),
            130.0,
            Face::Shout,
            &[("= SAME THING", "error")],
        ),
        // Exhibit D: a basketball and a hoop.
        shape(
            "d-board",
            at(D, RIM[0] + 20.0, RIM[1] - 150.0, 6.0),
            json!({ "rect": [420, 270] }),
            json!({ "stroke": "plain", "width": 4, "corner": 6, "fill": "surface", "fillOpacity": 0.6 }),
        ),
        shape(
            "d-square",
            at(D, RIM[0] + 20.0, RIM[1] - 110.0, 5.0),
            json!({ "rect": [150, 110] }),
            json!({ "stroke": "plain", "width": 3 }),
        ),
        path(
            "d-net",
            json!([
                [D[0] + RIM[0] - 95.0, D[1] + RIM[1], 0.0],
                [D[0] + RIM[0] - 60.0, D[1] + RIM[1] + 120.0, 0.0],
                [D[0] + RIM[0], D[1] + RIM[1] + 30.0, 0.0],
                [D[0] + RIM[0] + 60.0, D[1] + RIM[1] + 120.0, 0.0],
                [D[0] + RIM[0] + 95.0, D[1] + RIM[1], 0.0]
            ]),
            "plain",
            2.5,
            "none",
        ),
        orb(
            "d-ball",
            at(D, DRIBBLE[0], DRIBBLE[1], -10.0),
            92.0,
            1100,
            "warning",
        ),
        shape(
            "d-seam-v",
            at(D, DRIBBLE[0], DRIBBLE[1], -14.0),
            json!({ "rect": [5, 184] }),
            json!({ "fill": "plain", "stroke": null }),
        ),
        shape(
            "d-seam-h",
            at(D, DRIBBLE[0], DRIBBLE[1], -14.0),
            json!({ "rect": [184, 5] }),
            json!({ "fill": "plain", "stroke": null }),
        ),
        shape(
            "d-seam-l",
            at(D, DRIBBLE[0] - 130.0, DRIBBLE[1], -14.0),
            json!({ "arc": { "radius": 108, "start": 0.12, "sweep": 0.26 } }),
            json!({ "stroke": "plain", "width": 4 }),
        ),
        shape(
            "d-seam-r",
            at(D, DRIBBLE[0] + 130.0, DRIBBLE[1], -14.0),
            json!({ "arc": { "radius": 108, "start": 0.62, "sweep": 0.26 } }),
            json!({ "stroke": "plain", "width": 4 }),
        ),
        shape(
            "d-rim",
            at(D, RIM[0], RIM[1], -20.0),
            json!({ "rect": [200, 9] }),
            json!({ "fill": "error", "stroke": null, "corner": 4 }),
        ),
        shape(
            "d-floor",
            at(D, 0.0, DRIBBLE[1] + 96.0, 0.0),
            json!({ "rect": [1400, 3] }),
            json!({ "fill": "muted", "stroke": null }),
        ),
        typed(
            "d-word",
            at(D, -330.0, -400.0, -60.0),
            100.0,
            Face::Shout,
            &[("BASKETBALLS!", "warning")],
        ),
        // The red string that ties it all together.
        json!({ "kind": "path", "id": "string", "through": ["a-ball", "b-ball", "c-dots", "d-ball", "a-ball"],
                "tone": "error", "width": 4, "bend": 50 }),
        typed(
            "connected",
            [2110.0, 1290.0, -400.0],
            150.0,
            Face::Shout,
            &[("IT'S ALL CONNECTED", "error")],
        ),
    ];
    for index in 0..PINS {
        let h = |salt: u32| hash(index, salt);
        let tone = ["error", "accent", "plain", "request", "warning"][index as usize % 5];
        let point = [
            lerp(250.0, 4000.0, (index as f32 + h(1)) / PINS as f32),
            lerp(120.0, 2480.0, h(2)),
            lerp(-60.0, 160.0, h(3)),
        ];
        elements.push(orb(
            &format!("pin-{index}"),
            point,
            lerp(24.0, 62.0, h(4)),
            150,
            tone,
        ));
    }
    for index in 0..ORBITS.len() {
        elements.push(json!({
            "kind": "form", "id": format!("a-orbit-{index}"), "at": at(A, 0.0, -30.0, 0.0),
            "tone": "request", "points": 420, "tilt": 0.0,
            "shapes": [{ "shape": "sphere", "radius": NUCLEUS + 6.0 },
                       { "shape": "torus", "radius": 300, "tube": 7 }]
        }));
    }
    elements
}

/// The camera looking straight at `center` from `z`, turned a little.
fn facing(center: [f32; 2], z: f32, yaw: f32, roll: f32) -> Angle {
    Angle {
        x: center[0] - 960.0,
        y: center[1] - 540.0,
        z,
        yaw,
        roll,
        ..Angle::default()
    }
}

/// Something small appears with a pop: scale up past full and settle.
fn pop(s: &mut StageActor, sc: &mut PlanBuilder, id: &str, at: u64) {
    s.channel(sc, &format!("{id}.scale"), 0.3);
    s.ease(sc, &format!("{id}.opacity"), at, 1.0, 0.06, Ease::Linear);
    s.bounce(sc, &format!("{id}.scale"), at, 1.0, 0.35, 0.4);
}

pub fn segment(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("theory");
    let sc = &mut scene;
    let t = |at: u64| window.t(at);
    let theory = &film.theory;
    let plan = stage(
        elements(),
        json!({ "bloom": 0.6, "grain": 0.04, "vignette": 0.45, "backdrop": 0.06 }),
    )?;
    let mut stage = StageActor::declare(sc, "stage", &plan)?;
    let s = &mut stage;
    let camera = s.camera();
    let mut later = Later::default();

    let hidden = [
        "a-word",
        "a-fig",
        "a-lead-1",
        "a-note-1",
        "a-lead-2",
        "a-note-2",
        "b-bub-1",
        "b-bub-2",
        "b-bub-3",
        "b-cloud",
        "b-hopes",
        "b-dreams",
        "b-star-1",
        "b-star-2",
        "b-star-3",
        "c-loop",
        "c-loop-back",
        "c-same",
        "c-word",
        "c-ball",
        "string",
        "connected",
    ];
    for id in hidden {
        s.channel(sc, &format!("{id}.opacity"), 0.0);
    }
    for index in 0..PINS {
        s.channel(sc, &format!("pin-{index}.opacity"), 0.0);
    }
    for (index, (pitch, roll)) in ORBITS.iter().enumerate() {
        let id = format!("a-orbit-{index}");
        s.channel(sc, &format!("{id}.pitch"), *pitch);
        s.channel(sc, &format!("{id}.roll"), *roll);
        s.channel(sc, &format!("{id}.spin"), 0.0);
        s.channel(sc, &format!("{id}.solid"), 0.0);
    }
    s.channel(sc, "c-thought.opacity", 1.0);
    for id in ["a-lead-1", "a-lead-2", "string"] {
        s.channel(sc, &format!("{id}.draw"), 0.0);
    }
    for (property, value) in [
        ("camera.x", OPEN.x),
        ("camera.y", OPEN.y),
        ("camera.z", OPEN.z),
        ("camera.yaw", OPEN.yaw),
        ("camera.pitch", OPEN.pitch),
        ("camera.roll", OPEN.roll),
        ("camera.pivot", 0.0),
        ("camera.quake", 0.25),
    ] {
        s.channel(sc, property, value);
    }
    let entry = camera.screen_box(sc, "a-ball", &camera.pose(sc, 0), 0)?;

    // ── A: "Balls with dots represent ATOMS." ──
    // Swing around the nucleus as the dots leave it and fall into orbit.
    later.glide(
        0,
        Angle {
            y: -30.0,
            z: 260.0,
            yaw: 0.32,
            pitch: -0.06,
            roll: -0.03,
            ..Angle::default()
        },
        1.1,
    );
    let represent = t(theory.at("represent"));
    for index in 0..ORBITS.len() {
        let id = format!("a-orbit-{index}");
        s.morph(sc, &id, represent, 1, 0.42);
        s.ease(
            sc,
            &format!("{id}.rotation"),
            represent,
            2.4,
            2.0,
            Ease::CubicOut,
        );
    }
    let atoms = t(theory.at("atoms"));
    later.word("a-word", atoms, None, 0.8);
    later.crash(
        atoms,
        Angle {
            y: -150.0,
            z: 430.0,
            yaw: 0.36,
            roll: -0.08,
            ..Angle::default()
        },
        Angle {
            y: -40.0,
            z: 140.0,
            yaw: 0.2,
            roll: -0.02,
            ..Angle::default()
        },
        0.55,
    );
    later.hit("a-ball.pulse", atoms, 1.2, 0.0);
    s.charge(sc, "a-ball", atoms, 0.9, 0.1);
    s.charge(sc, "a-ball", atoms + seconds(0.45), 0.0, 0.25);
    for (index, id) in ["a-fig", "a-lead-1", "a-note-1", "a-lead-2", "a-note-2"]
        .into_iter()
        .enumerate()
    {
        let at = atoms + seconds(0.1 + 0.07 * index as f64);
        s.set(sc, &format!("{id}.opacity"), at, 1.0);
        if id.contains("lead") {
            s.ease(
                sc,
                &format!("{id}.draw"),
                at,
                1.0,
                0.3,
                Ease::CubicBezier([0.45, 0.0, 0.2, 1.0]),
            );
        }
    }

    // ── B: "Balls with THOUGHTS represent HOPES and DREAMS." ──
    let [to_b, to_c, to_d] = film.whips().map(t);
    later.whip(
        to_b,
        facing([B[0] - 40.0, B[1] - 20.0], 60.0, -0.12, 0.03),
        WHIP as f32,
    );
    let thoughts = t(theory.at("thoughts"));
    for (index, bubble) in ["b-bub-1", "b-bub-2", "b-bub-3"].into_iter().enumerate() {
        pop(s, sc, bubble, thoughts + seconds(0.05 * index as f64));
    }
    pop(s, sc, "b-cloud", thoughts + seconds(0.17));
    let hopes = t(theory.at("hopes"));
    later.word("b-hopes", hopes, None, 0.55);
    later.crash(
        hopes,
        facing([B[0] + 230.0, B[1] - 90.0], 520.0, 0.1, 0.07),
        facing([B[0] + 200.0, B[1] - 80.0], 360.0, 0.06, 0.03),
        0.4,
    );
    later.hit("b-ball.pulse", hopes, 0.9, 0.0);
    let dreams = t(theory.at("dreams"));
    later.word("b-dreams", dreams, None, 0.4);
    for (index, star) in ["b-star-1", "b-star-2", "b-star-3"].into_iter().enumerate() {
        pop(s, sc, star, dreams + seconds(0.04 * index as f64));
        later.hit(
            &format!("{star}.flash"),
            dreams + seconds(0.04 * index as f64),
            1.0,
            0.0,
        );
    }
    // Dreams rise, and the camera tilts up after them.
    for (id, lift) in [
        ("b-ball", -60.0),
        ("b-bub-1", -90.0),
        ("b-bub-2", -110.0),
        ("b-bub-3", -130.0),
        ("b-cloud", -150.0),
        ("b-hopes", -150.0),
        ("b-dreams", -150.0),
        ("b-star-1", -220.0),
        ("b-star-2", -200.0),
        ("b-star-3", -180.0),
    ] {
        s.ease(sc, &format!("{id}.y"), dreams, lift, 1.4, Ease::CubicOut);
    }
    later.glide(
        dreams + seconds(0.1),
        Angle {
            pitch: 0.2,
            roll: -0.04,
            ..facing([B[0], B[1] - 150.0], 140.0, -0.05, 0.0)
        },
        0.6,
    );
    later.hit("b-ball.pulse", dreams, 0.7, 0.0);

    // ── C: "Balls with thoughts represent balls with DOTS." ──
    later.whip(
        to_c,
        facing([C[0], C[1] - 30.0], 0.0, 0.15, -0.03),
        WHIP as f32,
    );
    let same = t(theory.at_after("thoughts", "dreams"));
    for loop_arc in ["c-loop", "c-loop-back"] {
        s.channel(sc, &format!("{loop_arc}.draw"), 0.0);
        s.set(sc, &format!("{loop_arc}.opacity"), same, 1.0);
        s.ease(
            sc,
            &format!("{loop_arc}.draw"),
            same,
            1.0,
            0.3,
            Ease::CubicBezier([0.45, 0.0, 0.2, 1.0]),
        );
        // The cycle turns, faster and faster.
        s.ease(
            sc,
            &format!("{loop_arc}.rotation"),
            same,
            9.0,
            3.0,
            Ease::CubicBezier([0.5, 0.0, 1.0, 1.0]),
        );
    }
    pop(s, sc, "c-word", same + seconds(0.2));
    later.hit("c-dots.pulse", same + seconds(0.3), 0.8, 0.0);
    let dots = t(theory.at_after("balls with dots", "dreams"));
    later.word("c-same", dots, None, 0.75);
    later.crash(
        dots,
        facing([C[0], C[1] - 170.0], 380.0, -0.1, 0.1),
        facing([C[0], C[1] - 90.0], 120.0, -0.04, 0.03),
        0.45,
    );
    s.clock_for(sc, "post.rewind", dots.saturating_sub(seconds(0.1)), 1.4);
    // The thought becomes a ball with dots.
    s.ease(sc, "c-thought.opacity", dots, 0.0, 0.12, Ease::Linear);
    pop(s, sc, "c-ball", dots);
    later.hit("c-ball.pulse", dots, 1.2, 0.0);
    later.hit("c-dots.pulse", dots, 1.2, 0.0);

    // ── D: "Balls with dots represent BASKETBALLS." ──
    later.whip(
        to_d,
        facing([D[0] + 40.0, D[1] - 60.0], 40.0, -0.1, 0.0),
        WHIP as f32,
    );
    let basketballs = t(theory.at("basketballs"));
    // Two dribbles, the second on the word, then the shot: it arcs over and
    // drops through the rim.
    for (index, bounce) in [
        basketballs.saturating_sub(seconds(0.3)),
        basketballs + seconds(0.02),
    ]
    .into_iter()
    .enumerate()
    {
        for part in BASKETBALL {
            let y = format!("{part}.y");
            let up = bounce.saturating_sub(seconds(0.15));
            if index == 0 {
                s.ease(sc, &y, up.saturating_sub(seconds(0.15)), -150.0, 0.15, RISE);
            } else {
                s.ease(sc, &y, up, -150.0, 0.15, RISE);
            }
            s.ease(
                sc,
                &y,
                up + seconds(if index == 0 { 0.0 } else { 0.15 }),
                0.0,
                0.15,
                FALL,
            );
        }
        later.hit("d-ball.pulse", bounce, 0.8, 0.0);
    }
    let shot = basketballs + seconds(0.06);
    let (dx, dy) = (RIM[0] - DRIBBLE[0], RIM[1] - DRIBBLE[1]);
    for part in BASKETBALL {
        s.ease(sc, &format!("{part}.x"), shot, dx, 0.42, Ease::Linear);
        s.ease(
            sc,
            &format!("{part}.y"),
            shot + seconds(0.15),
            dy - 260.0,
            0.2,
            RISE,
        );
        s.ease(
            sc,
            &format!("{part}.y"),
            shot + seconds(0.35),
            dy + 30.0,
            0.18,
            FALL,
        );
        s.ease(
            sc,
            &format!("{part}.y"),
            shot + seconds(0.53),
            dy + 420.0,
            0.3,
            FALL,
        );
    }
    later.word("d-word", basketballs, None, 0.9);
    later.crash(
        basketballs,
        facing([D[0] - 60.0, D[1] - 170.0], 300.0, 0.1, -0.08),
        facing([D[0] + 20.0, D[1] - 130.0], 150.0, 0.04, -0.02),
        0.5,
    );
    let swish = shot + seconds(0.38);
    later.hit("d-net.surge", swish, 1.0, 0.0);
    later.hit("d-rim.flash", swish, 1.0, 0.0);
    later.hit("post.flash", swish, 0.12, 0.0);

    // ── The board: pull back on the pins and the red string ──
    let board = swish + seconds(0.22);
    later.at(board, move |s, sc| {
        for (property, target) in [
            ("camera.x", 1150.0),
            ("camera.y", 760.0),
            ("camera.z", -2100.0),
            ("camera.yaw", 0.0),
            ("camera.pitch", 0.0),
            ("camera.roll", 0.05),
        ] {
            s.to(sc, property, board, target, 0.45);
        }
    });
    s.set(sc, "string.opacity", board, 1.0);
    s.ease(sc, "string.draw", board, 1.0, 0.35, Ease::Smoothstep);
    s.to(sc, "string.flow", board, 1.0, 0.3);
    later.hit("string.surge", board + seconds(0.1), 1.0, 0.0);
    later.word("connected", board + seconds(0.18), None, 0.6);
    let waves = [atoms, hopes, dreams, dots, basketballs, board];
    for index in 0..PINS {
        let wave = waves[(index as usize * 7) % waves.len()];
        let at = wave + seconds(0.025 * f64::from(index % 5));
        pop(s, sc, &format!("pin-{index}"), at);
        later.hit(&format!("pin-{index}.pulse"), at, 1.0, 0.0);
    }
    // The quake builds under the riser into the impact on "ANDY".
    let end = window.duration;
    let rest = ((end - board) as f64 / 1e9) as f32;
    s.ease(sc, "camera.quake", board, 1.7, rest, Ease::Linear);
    later.at(board, move |s, sc| {
        s.ease(sc, "post.chroma", board, 0.4, rest, Ease::Linear);
    });
    // The drums: a bump on every pulse.
    for &hit in film
        .pulse
        .iter()
        .filter(|&&hit| hit >= window.from && hit < window.until)
    {
        let at = t(hit);
        later.at(at, move |s, sc| {
            s.kick(sc, ["camera.kick-x", "camera.kick-y"], at, [0.0, 7.0]);
        });
    }
    later.run(s, sc);

    // Confetti from the hoop on the swish.
    let mut burst = ConfettiActor::declare(
        sc,
        "confetti",
        &ConfettiPlan::new([1290.0, 320.0])
            .count(160)
            .seed(7)
            .speed(1500.0)
            .aim(0.0, 70.0),
    )?;
    burst.burst(sc, swish);

    // ── Subtitles: shouted now, and starting to shake ──
    let said = words(theory, &window, str::to_uppercase);
    let plan = SubtitlesPlan::new([960.0, 975.0], 1500.0)
        .size(58.0)
        .face(Face::Shout)
        .highlight(Tone::Warning);
    let mut loud = subtitles(sc, "sub-theory", &said, plan)?;
    jitter(
        sc,
        &mut loud,
        &[
            (atoms, 0.4),
            (hopes, 0.3),
            (dreams, 0.3),
            (dots, 0.6),
            (basketballs, 0.8),
            (board, 0.5),
        ],
        1.0,
    );
    Ok(Shot {
        plan: window.finish(scene, film)?,
        entry: Some([
            entry.min.x,
            entry.min.y,
            entry.max.x - entry.min.x,
            entry.max.y - entry.min.y,
        ]),
        exit: None,
    })
}
