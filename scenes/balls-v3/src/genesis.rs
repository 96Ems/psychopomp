//! Genesis, one continuous take from the dark to the chant. A title racks
//! into focus among drifting motes; the focus pulls past it to a point of
//! light; the camera flies through the title toward it, and on "balls" it
//! ignites into a ball, soft until "dots" racks it sharp. A macro orbit; a
//! second ball with more dots; then every chanted "balls" drops another ball
//! onto a floor of dots while the camera cuts on each beat, and on the last
//! the camera dives through the first ball into one of its dots.
use anyhow::Result;
use psychopomp::{
    author::seconds,
    face::Face,
    math::{Quat, Vec3, easing::Ease, lerp, random::hash},
    stage::{FOCAL, Move, StageActor, orb_points},
    subtitles::SubtitlesPlan,
    tone::Tone,
};
use serde_json::{Value, json};

use crate::{
    Shot,
    kit::{Later, Window, drop, form, orb, stage, subtitles, typed, words},
    sound::Film,
};

const BALL1: [f32; 3] = [960.0, 600.0, 1800.0];
const RADIUS1: f32 = 300.0;
const POINTS1: u32 = 2600;
const BALL2: [f32; 3] = [2000.0, 560.0, 2050.0];
const RADIUS2: f32 = 340.0;
const FLOOR: f32 = 900.0;
/// The first ball turns this far by the dive and holds still for it.
const TURN: f32 = 1.3;
/// How close the lens comes to the dot it dives into.
const DIVE: f32 = 46.0;
const MOTES: u32 = 44;

/// What each chanted "balls" drops: x and z on the floor, radius, tone, points.
const SPAWNS: [(f32, f32, f32, &str, u32); 13] = [
    (-150.0, 2150.0, 150.0, "plain", 900),
    (1450.0, 2750.0, 170.0, "error", 1000),
    (380.0, 1450.0, 105.0, "warning", 600),
    (2850.0, 2250.0, 160.0, "accent", 900),
    (1150.0, 3100.0, 190.0, "success", 1000),
    (-550.0, 2850.0, 140.0, "plain", 700),
    (3250.0, 2950.0, 150.0, "error", 700),
    (2450.0, 1500.0, 95.0, "accent", 500),
    (80.0, 3350.0, 180.0, "warning", 800),
    (1650.0, 1600.0, 85.0, "plain", 450),
    (3500.0, 1900.0, 120.0, "success", 600),
    (-750.0, 1800.0, 120.0, "error", 600),
    (850.0, 1250.0, 70.0, "accent", 400),
];

/// The chant's words: text, face, size, tone, and where they hang.
const CHANT: [(&str, Face, f32, &str, [f32; 3]); 13] = [
    (
        "balls.",
        Face::SerifItalic,
        120.0,
        "plain",
        [430.0, 300.0, 1500.0],
    ),
    (
        "balls.",
        Face::SerifItalic,
        130.0,
        "plain",
        [1650.0, 260.0, 1700.0],
    ),
    (
        "Balls.",
        Face::Serif,
        140.0,
        "accent",
        [300.0, 520.0, 1300.0],
    ),
    (
        "Balls.",
        Face::Serif,
        150.0,
        "plain",
        [1700.0, 700.0, 1350.0],
    ),
    (
        "BALLS",
        Face::Light,
        150.0,
        "warning",
        [960.0, 330.0, 1250.0],
    ),
    ("BALLS", Face::Shout, 150.0, "error", [450.0, 760.0, 1200.0]),
    (
        "BALLS",
        Face::Shout,
        150.0,
        "accent",
        [1500.0, 280.0, 1150.0],
    ),
    (
        "BALLS",
        Face::Shout,
        150.0,
        "plain",
        [1350.0, 850.0, 1100.0],
    ),
    (
        "BALLS",
        Face::Shout,
        150.0,
        "warning",
        [350.0, 260.0, 1050.0],
    ),
    (
        "BALLS",
        Face::Shout,
        150.0,
        "error",
        [1750.0, 500.0, 1000.0],
    ),
    ("BALLS", Face::Shout, 150.0, "accent", [600.0, 560.0, 950.0]),
    ("BALLS", Face::Shout, 150.0, "plain", [1150.0, 640.0, 900.0]),
    (
        "BALLS!",
        Face::Shout,
        160.0,
        "error",
        [960.0, 560.0, 1150.0],
    ),
];

/// Camera angles for the chant's cuts from the third beat: yaw, pitch, roll,
/// and how tight (padding around the balls framed).
const ANGLES: [(f32, f32, f32, f32); 11] = [
    (0.42, 0.12, -0.06, 120.0),
    (-0.38, -0.08, 0.07, 120.0),
    (0.15, 0.32, 0.0, 140.0),
    (-0.55, 0.05, -0.1, 60.0),
    (0.6, -0.1, 0.12, 60.0),
    (-0.2, 0.4, -0.14, 60.0),
    (0.3, -0.18, 0.16, 50.0),
    (-0.65, 0.15, -0.18, 50.0),
    (0.7, 0.25, 0.2, 40.0),
    (-0.4, -0.2, -0.22, 40.0),
    (0.0, 0.0, 0.0, 90.0),
];

fn mote(index: u32) -> Value {
    let h = |salt: u32| hash(index, salt + 400);
    let at = [
        lerp(-700.0, 2620.0, h(1)),
        lerp(-350.0, 1430.0, h(2)),
        lerp(-500.0, 4400.0, h(3)),
    ];
    let tone = ["accent", "plain", "accent", "warning"][index as usize % 4];
    json!({ "kind": "shape", "id": format!("mote-{index}"), "at": at,
            "shape": { "circle": lerp(10.0, 34.0, h(4)) }, "fill": tone, "fillOpacity": 0.3,
            "stroke": tone, "width": 2.5 })
}

/// The dot of the first ball nearest the lens once it has turned `TURN`
/// (the renderer's orb rotation, with no ambient spin), in world space.
fn front_dot() -> Vec3 {
    let turn = Quat::from_rotation_x(0.42) * Quat::from_rotation_y(TURN);
    let unit = orb_points(POINTS1)
        .into_iter()
        .map(|point| turn * point.unit)
        .min_by(|a, b| a.z.total_cmp(&b.z))
        .expect("an orb has points");
    Vec3::from(BALL1) + unit * RADIUS1
}

pub fn segment(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("genesis");
    let sc = &mut scene;
    let t = |at: u64| window.t(at);
    let mut elements = vec![
        typed(
            "title",
            [960.0, 450.0, 0.0],
            76.0,
            Face::SerifItalic,
            &[("In the beginning, there were balls.", "plain")],
        ),
        json!({ "kind": "ring", "id": "shock", "at": BALL1, "radius": 330, "thickness": 3, "tone": "accent" }),
        orb("ball-1", BALL1, RADIUS1, POINTS1, "accent"),
        orb("ball-2-few", BALL2, RADIUS2, 160, "plain"),
        orb("ball-2", BALL2, RADIUS2, 2600, "plain"),
        form(
            "floor",
            [1300.0, FLOOR + 10.0, 2300.0],
            1000,
            "muted",
            json!([{ "shape": "plane", "size": [2000, 800] }]),
        ),
    ];
    if let Some(floor) = elements.last_mut() {
        floor["tilt"] = json!(1.48);
    }
    for index in 0..MOTES {
        elements.push(mote(index));
    }
    for (index, (x, z, radius, tone, points)) in SPAWNS.iter().enumerate() {
        elements.push(orb(
            &format!("spawn-{index}"),
            [*x, FLOOR - radius, *z],
            *radius,
            *points,
            tone,
        ));
    }
    for (index, (text, face, size, tone, at)) in CHANT.iter().enumerate() {
        elements.push(typed(
            &format!("word-{index}"),
            *at,
            *size,
            *face,
            &[(text, tone)],
        ));
    }
    let plan = stage(
        elements,
        json!({ "bloom": 0.55, "grain": 0.04, "vignette": 0.62, "backdrop": 0.0 }),
    )?;
    let mut stage = StageActor::declare(sc, "stage", &plan)?;
    let s = &mut stage;
    let camera = s.camera();
    let mut later = Later::default();

    for (property, initial) in [
        ("camera.z", -140.0),
        ("camera.dof", 1.5),
        ("camera.focus", 700.0),
        ("camera.zoom", 1.0),
        ("post.vignette", 0.75),
        ("post.bloom", 0.55),
        ("title.opacity", 0.0),
        ("shock.opacity", 0.0),
        ("ball-1.opacity", 0.0),
        ("ball-1.scale", 0.022),
        ("ball-1.spin", 0.0),
        ("ball-1.blur", 0.0),
        ("ball-2-few.opacity", 0.0),
        ("ball-2-few.scale", 0.3),
        ("ball-2.opacity", 0.0),
        ("ball-2.scale", 0.9),
        ("floor.opacity", 0.0),
        ("floor.scale", 2.5),
        ("floor.spin", 0.0),
    ] {
        s.channel(sc, property, initial);
    }
    for index in 0..SPAWNS.len() {
        s.channel(sc, &format!("spawn-{index}.opacity"), 0.0);
        s.channel(sc, &format!("word-{index}.opacity"), 0.0);
    }
    // The motes drift on their own, slowly, the whole time.
    let length = (window.duration as f64 / 1e9) as f32;
    for index in 0..MOTES {
        let h = |salt: u32| hash(index, salt + 900);
        let id = format!("mote-{index}");
        s.channel(sc, &format!("{id}.opacity"), lerp(0.3, 0.75, h(1)));
        s.channel(sc, &format!("{id}.blur"), lerp(3.0, 12.0, h(5)));
        s.ease(
            sc,
            &format!("{id}.x"),
            0,
            lerp(-90.0, 90.0, h(2)),
            length,
            Ease::Linear,
        );
        s.ease(
            sc,
            &format!("{id}.y"),
            0,
            lerp(-140.0, -20.0, h(3)),
            length,
            Ease::Linear,
        );
    }

    // ── 0 · In the beginning ──
    // The title racks into focus; then the focus pulls past it to a point
    // of light far behind, and the camera sets off toward it.
    let whisper = t(film.hush.voice_start());
    s.ease(
        sc,
        "title.opacity",
        seconds(0.05),
        1.0,
        0.6,
        Ease::Smoothstep,
    );
    s.ease(
        sc,
        "camera.focus",
        seconds(0.05),
        0.0,
        1.0,
        Ease::Smootherstep,
    );
    s.ease(sc, "camera.z", 0, 330.0, 1.6, Ease::Smoothstep);
    s.ease(sc, "camera.y", 0, 60.0, 1.6, Ease::Smoothstep);
    s.ease(
        sc,
        "ball-1.opacity",
        seconds(0.7),
        1.0,
        0.8,
        Ease::Smoothstep,
    );
    later.hit("ball-1.pulse", seconds(0.9), 1.4, 0.4);
    let rack = whisper.saturating_sub(seconds(0.15));
    s.ease(sc, "camera.focus", rack, BALL1[2], 0.9, Ease::Smootherstep);
    s.ease(
        sc,
        "title.opacity",
        rack + seconds(0.5),
        0.0,
        1.2,
        Ease::Smoothstep,
    );
    s.ease(
        sc,
        "camera.z",
        seconds(1.6),
        2120.0,
        4.2,
        Ease::Smootherstep,
    );

    // ── "...balls." It ignites, soft and out of focus. ──
    let balls = t(film.hush.at("balls"));
    s.set(sc, "ball-1.blur", balls, 34.0);
    s.bounce(sc, "ball-1.scale", balls, 1.0, 0.85, 0.16);
    s.set(sc, "ball-1.rotation", balls, -1.6);
    s.ease(sc, "ball-1.rotation", balls, 0.0, 1.3, Ease::CubicOut);
    later.hit("ball-1.pulse", balls, 1.6, 0.25);
    later.hit("post.bloom", balls, 0.95, 0.55);
    s.set(sc, "shock.opacity", balls, 0.9);
    s.ease(sc, "shock.expand", balls, 1.0, 1.1, Ease::CubicOut);
    s.channel(sc, "shock.scale", 0.0);
    s.ease(sc, "shock.scale", balls, 2.6, 1.1, Ease::CubicOut);

    // ── "...with dots." The focus snaps onto its front face. ──
    let dots = t(film.hush.at("dots"));
    s.ease(
        sc,
        "ball-1.blur",
        dots.saturating_sub(seconds(0.12)),
        0.0,
        0.42,
        Ease::CubicOut,
    );
    s.ease(
        sc,
        "camera.focus",
        dots.saturating_sub(seconds(0.12)),
        BALL1[2] - RADIUS1 * 0.6,
        0.42,
        Ease::CubicOut,
    );
    s.ease(sc, "camera.dof", dots, 2.2, 0.6, Ease::Smoothstep);
    later.hit("ball-1.pulse", dots, 1.0, 0.0);
    s.ease(sc, "post.vignette", dots, 0.55, 1.5, Ease::Smoothstep);
    // It turns on its own from here, slowly, and stops for the dive.
    let dive = t(film.dive().0);
    let turn_from = dots + seconds(0.6);
    s.ease(
        sc,
        "ball-1.rotation",
        turn_from,
        TURN,
        ((dive - turn_from) as f64 / 1e9) as f32 - 0.2,
        Ease::Smoothstep,
    );

    // "Just one ball with dots on it." A slow macro orbit, hand-held.
    let just = t(film.hush.at("just"));
    s.channel(sc, "camera.pivot", 0.0);
    s.set(sc, "camera.pivot", just, BALL1[2]);
    s.ease(sc, "camera.yaw", just, 0.42, 4.6, Ease::Smootherstep);
    s.ease(sc, "camera.pitch", just, -0.12, 4.6, Ease::Smootherstep);
    camera.handheld(sc, just, 0.7, 1.5);
    later.hit(
        "ball-1.pulse",
        t(film.hush.at_after("dots", "just")),
        0.7,
        0.0,
    );

    // ── "But what if... another ball with more dots?" ──
    let what = t(film.another.at("but what if"));
    s.ease(sc, "camera.z", what, 1560.0, 2.4, Ease::Smootherstep);
    s.ease(sc, "camera.roll", what, -0.05, 2.6, Ease::Smootherstep);
    s.ease(
        sc,
        "camera.yaw",
        what + seconds(0.4),
        0.12,
        2.4,
        Ease::Smootherstep,
    );
    s.ease(
        sc,
        "camera.pitch",
        what + seconds(0.4),
        0.0,
        2.4,
        Ease::Smootherstep,
    );
    s.ease(sc, "camera.dof", what, 1.2, 2.0, Ease::Smoothstep);
    camera.handheld(sc, t(film.another.at_after("but what if", "if")), 1.2, 0.8);
    let another = t(film.another.at("another"));
    s.ease(
        sc,
        "ball-2-few.opacity",
        another,
        1.0,
        0.15,
        Ease::Smootherstep,
    );
    s.bounce(sc, "ball-2-few.scale", another, 1.0, 0.55, 0.32);
    s.ease(sc, "ball-2-few.rotation", another, 1.6, 2.0, Ease::CubicOut);
    later.hit("ball-2-few.pulse", another, 1.0, 0.0);
    later.jolt(another, [1.0, 0.2], 0.25);
    // The camera swings to take them both in.
    s.ease(
        sc,
        "camera.focus",
        another,
        BALL2[2] - 100.0,
        0.5,
        Ease::Smoothstep,
    );
    camera.frame(sc, &["ball-1", "ball-2"], 130.0, another, Move::Spring(0.6))?;
    let more = t(film.another.at("more dots"));
    s.ease(sc, "ball-2-few.opacity", more, 0.0, 0.3, Ease::Smoothstep);
    s.ease(sc, "ball-2.opacity", more, 1.0, 0.18, Ease::Smootherstep);
    s.set(sc, "ball-2.scale", more, 0.86);
    s.bounce(sc, "ball-2.scale", more, 1.0, 0.6, 0.35);
    s.ease(sc, "ball-2.rotation", more, 1.6, 2.0, Ease::CubicOut);
    later.hit("ball-2.pulse", more + seconds(0.08), 1.2, 0.0);
    later.hit("post.chroma", more, 0.25, 0.0);
    camera.dolly_zoom(sc, "ball-2", more, 520.0, Move::Glide(1.6))?;

    // The heart: every thump lights the balls on screen.
    for &beat in &film.heartbeats {
        let at = t(beat);
        later.hit("ball-1.pulse", at, 0.35, 0.0);
        if at > another {
            later.hit("ball-2.pulse", at, 0.35, 0.0);
        }
    }

    // ── The chant: one ball per "balls", on a floor of dots ──
    let beats = film.chant.beats();
    let first = t(beats[0].0);
    s.ease(
        sc,
        "floor.opacity",
        first.saturating_sub(seconds(0.3)),
        0.8,
        0.6,
        Ease::Smoothstep,
    );
    s.ease(sc, "camera.dof", first, 0.35, 1.0, Ease::Smoothstep);
    s.ease(sc, "camera.focus", first, 2200.0, 1.0, Ease::Smoothstep);
    s.ease(sc, "post.vignette", first, 0.45, 1.5, Ease::Smoothstep);
    camera.handheld(sc, first, 0.4, 0.5);
    s.set(sc, "camera.pivot", first, 2200.0);
    let mut visible = vec!["ball-1".to_owned(), "ball-2".to_owned()];
    let mut angle = ANGLES.iter();
    let last = beats.len() - 1;
    for (index, (beat, _)) in beats.iter().enumerate() {
        let at = t(*beat);
        let id = format!("spawn-{index}");
        let late = index as f32 / last as f32;
        // The first five drop and bounce; the rest pop in as they are shouted.
        if index < 5 {
            s.set(
                sc,
                &format!("{id}.opacity"),
                at.saturating_sub(seconds(0.3)),
                1.0,
            );
            let contacts = drop(s, sc, &id, at, 760.0, 0.42, 2);
            for contact in contacts.into_iter().skip(1) {
                later.hit(&format!("{id}.pulse"), contact, 0.5, 0.0);
            }
        } else {
            s.set(sc, &format!("{id}.opacity"), at, 1.0);
            s.channel(sc, &format!("{id}.scale"), 0.15);
            s.bounce(sc, &format!("{id}.scale"), at, 1.0, 0.32, 0.4);
            drop(s, sc, &id, at + seconds(0.05), 140.0, 0.3, 1);
        }
        let side = if index % 2 == 0 { 2.2 } else { -2.2 };
        s.ease(sc, &format!("{id}.rotation"), at, side, 2.4, Ease::CubicOut);
        later.hit(&format!("{id}.pulse"), at, 1.2, 0.0);
        visible.push(id);
        // The word slams in where it hangs.
        let word = format!("word-{index}");
        let hard = 1.0 + 0.9 * late;
        s.set(sc, &format!("{word}.opacity"), at, 1.0);
        s.channel(sc, &format!("{word}.scale"), hard * 1.25);
        if index < last {
            s.bounce(sc, &format!("{word}.scale"), at, 1.0, 0.3, 0.3);
        }
        if index < 5 {
            s.ease(
                sc,
                &format!("{word}.opacity"),
                at + seconds(0.5),
                0.0,
                0.35,
                Ease::Smoothstep,
            );
        }
        // The camera: two reframes, then a cut on every beat.
        let targets = visible.iter().map(String::as_str).collect::<Vec<_>>();
        if index < 2 {
            camera.frame(sc, &targets, 170.0, at, Move::Spring(0.9))?;
            later.jolt(at, [0.0, 1.0], 0.12);
            continue;
        }
        let (yaw, pitch, roll, padding) = *angle.next().expect("an angle per beat");
        let at = crate::kit::snap(at);
        for (property, value) in [
            ("camera.yaw", yaw),
            ("camera.pitch", pitch),
            ("camera.roll", roll),
            ("camera.zoom", 1.0),
        ] {
            s.set(sc, property, at, value);
        }
        let framed: Vec<&str> = if padding < 100.0 {
            // Close: the new ball and its neighbors.
            targets[targets.len().saturating_sub(3)..].to_vec()
        } else {
            targets.clone()
        };
        camera.frame(sc, &framed, padding, at, Move::Cut)?;
        camera.push_in(sc, at, 90.0 + 60.0 * late, Move::Glide(0.5));
        let direction = if index % 2 == 0 {
            [1.0, 0.5]
        } else {
            [-1.0, 0.5]
        };
        later.jolt(at, direction, 0.15 + 0.35 * late);
        later.hit("post.chroma", at, 0.08 + 0.3 * late, 0.0);
        later.hit("post.zoom", at, 0.04 + 0.08 * late, 0.0);
    }
    let rapid = t(beats[5].0);
    let shout = t(beats[last].0);
    s.ease(
        sc,
        "camera.quake",
        rapid,
        1.0,
        ((shout - rapid) as f64 / 1e9) as f32,
        Ease::Linear,
    );
    s.set(sc, "camera.quake", shout, 1.3);
    // "BALLS!": as wide as the frame, in front of everything; everything
    // pulses; then the quake stops for the dive.
    let word = Vec3::from(CHANT[last].4);
    let near = camera
        .pose(sc, shout)
        .project(word)
        .map_or(1.0, |(_, scale)| scale);
    let wide = 1500.0 / (CHANT[last].0.len() as f32 * CHANT[last].2 * 0.5 * near);
    s.set(sc, &format!("word-{last}.scale"), shout, wide * 1.5);
    s.bounce(sc, &format!("word-{last}.scale"), shout, wide, 0.3, 0.3);
    later.slam(shout, [0.0, 1.0], 1.0);
    for other in &visible {
        later.hit(&format!("{other}.pulse"), shout, 1.4, 0.0);
    }

    // ── The dive: straight through the first ball into one of its dots ──
    let end = window.until - window.from;
    let lunge = shout + seconds(0.16);
    s.set(sc, "camera.quake", lunge, 0.0);
    s.set(sc, &format!("word-{last}.opacity"), lunge, 1.0);
    s.ease(
        sc,
        &format!("word-{last}.opacity"),
        lunge,
        0.0,
        0.14,
        Ease::Linear,
    );
    let dot = front_dot();
    let reach = ((end - lunge) as f64 / 1e9) as f32;
    let target = [
        ("camera.x", dot.x - 960.0),
        ("camera.y", dot.y - 540.0),
        ("camera.z", FOCAL + dot.z - DIVE),
        ("camera.yaw", 0.0),
        ("camera.pitch", 0.0),
        ("camera.roll", 0.0),
        ("camera.zoom", 1.0),
    ];
    for (property, value) in target {
        let curve = if property == "camera.z" {
            Ease::CubicBezier([0.55, 0.0, 0.95, 0.75])
        } else {
            Ease::CubicBezier([0.3, 0.0, 0.3, 1.0])
        };
        s.ease(sc, property, lunge, value, reach, curve);
    }
    camera.handheld(sc, lunge, 0.0, 0.1);
    // Thirty times magnified, any leftover shake would smear every dot.
    for property in [
        "camera.shake",
        "camera.kick-x",
        "camera.kick-y",
        "camera.punch",
        "post.chroma",
    ] {
        later.at(lunge, move |s, sc| {
            s.ease(sc, property, lunge, 0.0, 0.08, Ease::Linear)
        });
    }
    later.at(lunge, move |s, sc| {
        s.ease(sc, "post.bloom", lunge, 0.35, 0.2, Ease::Linear)
    });
    s.ease(sc, "ball-1.pulse", lunge, 0.0, 0.1, Ease::Linear);
    s.ease(sc, "camera.focus", lunge, dot.z, reach, Ease::Linear);
    s.ease(sc, "camera.dof", lunge, 0.0, 0.2, Ease::Linear);
    later.hit("post.zoom", lunge, 0.22, 0.0);
    for index in 0..CHANT.len() - 1 {
        s.ease(
            sc,
            &format!("word-{index}.opacity"),
            lunge,
            0.0,
            0.12,
            Ease::Linear,
        );
    }
    later.run(s, sc);

    // ── Subtitles: the whisper, in italic ──
    let whisper = [
        words(&film.hush, &window, str::to_lowercase),
        words(&film.another, &window, str::to_lowercase),
    ]
    .concat();
    let plan = SubtitlesPlan::new([960.0, 968.0], 1300.0)
        .size(48.0)
        .face(Face::SerifItalic)
        .highlight(Tone::Accent)
        .without_backing();
    let mut said = subtitles(sc, "whisper", &whisper, plan)?;
    let opacity = said.channel(sc, "opacity", 1.0);
    sc.ease(&opacity, first, 0.0, 0.2, Ease::Linear);

    // The dot the dive ends in, as the match cut's starting rectangle.
    let radius = 2.0 * FOCAL / DIVE;
    Ok(Shot {
        plan: window.finish(scene, film)?,
        entry: None,
        exit: Some([960.0 - radius, 540.0 - radius, radius * 2.0, radius * 2.0]),
    })
}
