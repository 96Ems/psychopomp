//! The climax: every voice at once over a montage cut on drum hits, faster
//! and faster. A lightning storm between balls; a terminal that will not stop
//! printing BALLS; the atom; CUBES ARE BALLS, shouted word by word while
//! every cube rounds into a ball; then a chart, a meter, the chimp
//! electrified, basketball rain, a shield bursting, lasagna, the fork, the
//! loupe, a tunnel of rings, until every ball converges and one colossal hit
//! implodes them into a point. The captions give up on words.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, seconds},
    axis::AxisPlan,
    bars::{BarSeriesPlan, BarsActor, BarsPlan},
    callout::{CalloutAnchorPlan, CalloutSide},
    caption::{CaptionActor, CaptionAlign, CaptionPlan, CaptionSpanPlan},
    confetti::{ConfettiActor, ConfettiPlan},
    effects::combustion,
    face::Face,
    lens::{LensActor, LensPlan},
    math::{easing::Ease, lerp, random::hash},
    meter::{MeterActor, MeterPlan},
    readout::ReadoutFormat,
    stage::{Move, StageActor},
    subtitles::SubtitlesPlan,
    terminal::{TerminalActor, TerminalLinePlan, TerminalPlan},
    tone::Tone,
};
use serde_json::{Value, json};

use crate::{
    Shot,
    andy::{self, CHIMP, MARKERS},
    kit::{Angle, Later, Window, drop, form, orb, stage, typed},
    sound::{CUBES_WORDS, Film, HIT},
};

const TONES: [&str; 5] = ["accent", "plain", "request", "error", "warning"];

/// What the captions say instead of words, a little more unhinged each cut:
/// knocked off center, recolored, and seeing double.
fn noise(scene: &mut PlanBuilder, shot: u32, until: Option<u64>) -> Result<()> {
    let tones = [Tone::Error, Tone::Warning, Tone::Accent, Tone::Request];
    let text = "[EVERYONE SCREAMING ABOUT BALLS]";
    let x = 960.0 + (hash(shot, 1) - 0.5) * 120.0;
    let y = 975.0 + (hash(shot, 2) - 0.5) * 40.0;
    for (layer, (dx, dy, opacity)) in [(0.0, 0.0, 1.0), (9.0, -6.0, 0.45)].into_iter().enumerate() {
        let tone = tones[(shot as usize + layer) % tones.len()];
        let mut plan = CaptionPlan::line(
            [x + dx, y + dy],
            46.0,
            vec![CaptionSpanPlan::new(text, tone)],
        )
        .aligned(CaptionAlign::Center);
        if layer == 0 {
            plan = plan.chip();
        }
        let mut caption = CaptionActor::declare(scene, format!("noise-{layer}"), &plan)?;
        let alpha = caption.channel(scene, "opacity", opacity);
        scene.set(&alpha, 0, opacity);
        if let Some(until) = until {
            scene.ease(&alpha, until, 0.0, 0.08, Ease::Linear);
        }
    }
    Ok(())
}

/// A field of balls of every kind, scattered in depth.
fn swarm(prefix: &str, count: u32, salt: u32, spread: [f32; 2], depth: [f32; 2]) -> Vec<Value> {
    (0..count)
        .map(|index| {
            let h = |k: u32| hash(index, salt + k);
            let at = [
                960.0 + (h(1) - 0.5) * spread[0],
                540.0 + (h(2) - 0.5) * spread[1],
                lerp(depth[0], depth[1], h(3)),
            ];
            let tone = TONES[(index as usize + salt as usize) % TONES.len()];
            let id = format!("{prefix}-{index}");
            let size = lerp(50.0, 130.0, h(4));
            match index % 4 {
                0 | 2 => orb(&id, at, size, 160 + (h(5) * 260.0) as u32, tone),
                1 => form(
                    &id,
                    at,
                    300,
                    tone,
                    json!([
                    { "shape": "box", "size": [size * 1.4, size * 1.4, size * 1.4] },
                    { "shape": "sphere", "radius": size },
                    { "shape": "torus", "radius": size, "tube": size * 0.3 }]),
                ),
                _ => form(
                    &id,
                    at,
                    343,
                    tone,
                    json!([
                    { "shape": "torus", "radius": size, "tube": size * 0.3 },
                    { "shape": "lattice", "size": [size * 1.5, size * 1.5, size * 1.5] },
                    { "shape": "sphere", "radius": size }]),
                ),
            }
        })
        .collect()
}

fn post() -> Value {
    json!({ "bloom": 0.75, "grain": 0.05, "vignette": 0.45, "backdrop": 0.12 })
}

/// Everything in a swarm tumbles and morphs from the first frame.
fn tumble(s: &mut StageActor, sc: &mut PlanBuilder, prefix: &str, count: u32, length: f32) {
    for index in 0..count {
        let id = format!("{prefix}-{index}");
        let turn = if index % 2 == 0 { 3.0 } else { -3.0 };
        s.ease(sc, &format!("{id}.rotation"), 0, turn, length, Ease::Linear);
        if index % 4 == 1 || index % 4 == 3 {
            s.ease(
                sc,
                &format!("{id}.pitch"),
                0,
                turn * 0.7,
                length,
                Ease::Linear,
            );
            let shape = if index % 8 < 4 { 1.0 } else { 2.0 };
            s.ease(
                sc,
                &format!("{id}.morph"),
                0,
                shape,
                length * 0.8,
                Ease::Smootherstep,
            );
        }
    }
}

fn seconds_of(window: &Window) -> f32 {
    (window.duration as f64 / 1e9) as f32
}

/// A shouted word that slams in at `at` from `from` times its size.
fn slam_word(s: &mut StageActor, sc: &mut PlanBuilder, id: &str, at: u64, from: f32, to: f32) {
    s.channel(sc, &format!("{id}.scale"), from);
    s.set(sc, &format!("{id}.scale"), at, from);
    s.bounce(sc, &format!("{id}.scale"), at, to, 0.26, 0.35);
}

/// Opener 1: a lightning storm between balls, the camera lunging in.
pub fn storm(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-storm");
    let sc = &mut scene;
    let length = seconds_of(&window);
    let mut elements = swarm("ball", 16, 3, [1800.0, 950.0], [-300.0, 900.0]);
    let pairs = [
        (0, 2),
        (4, 6),
        (8, 10),
        (12, 14),
        (1, 5),
        (3, 9),
        (7, 11),
        (13, 15),
    ];
    for (index, (from, to)) in pairs.iter().enumerate() {
        elements.push(json!({ "kind": "bolt", "id": format!("bolt-{index}"),
            "from": format!("ball-{from}"), "to": format!("ball-{to}"),
            "strikes": 4, "branching": 1.0, "tone": if index % 2 == 0 { "request" } else { "error" } }));
    }
    elements.push(typed(
        "shout",
        [960.0, 520.0, -200.0],
        160.0,
        Face::Shout,
        &[("BALLS!!", "plain")],
    ));
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    s.channel(sc, "camera.z", -260.0);
    s.channel(sc, "camera.yaw", -0.25);
    s.ease(sc, "camera.z", 0, 160.0, length, Ease::Linear);
    s.ease(sc, "camera.yaw", 0, 0.25, length, Ease::Linear);
    s.set(sc, "camera.quake", 0, 1.4);
    s.set(sc, "post.chroma", 0, 0.35);
    tumble(s, sc, "ball", 16, length);
    for index in (0..16).filter(|index| index % 4 == 0 || index % 4 == 2) {
        s.charge(sc, &format!("ball-{index}"), 0, 1.2, 0.0);
    }
    for (index, _) in pairs.iter().enumerate() {
        let at = seconds(0.02 + 0.04 * index as f64);
        let contact = s.zap(sc, &format!("bolt-{index}"), at);
        later.jolt(contact, [if index % 2 == 0 { 1.0 } else { -1.0 }, 0.5], 0.6);
    }
    slam_word(s, sc, "shout", seconds(0.02), 2.4, 1.3);
    later.run(s, sc);
    noise(sc, 0, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// Opener 2: a terminal that will not stop.
pub fn terminal(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-terminal");
    let sc = &mut scene;
    let tones = [Tone::Error, Tone::Warning, Tone::Accent, Tone::Plain];
    let line = |index: usize| {
        let text = match index % 3 {
            0 => "BALLS WITH DOTS ARE THE FUTURE",
            1 => "BALLS BALLS BALLS BALLS BALLS BALLS",
            _ => "● ● ● ● ● ● ● ● ● ● ● ● ● ● ● ● ● ●",
        };
        vec![CaptionSpanPlan::new(text, tones[index % tones.len()])]
    };
    let mut plan = TerminalPlan::new([110.0, 60.0], 1700.0, 15)
        .titled("~/the-future — zsh")
        .size(40.0);
    plan.lines.push(TerminalLinePlan::Command {
        id: "run".into(),
        text: "balls --with-dots --forever".into(),
    });
    for index in 0..8 {
        plan.lines.push(TerminalLinePlan::Output {
            id: format!("early{index}"),
            spans: line(index),
        });
    }
    let mut term = TerminalActor::declare(sc, "term", plan)?;
    let mut at = 0;
    for index in 8..40 {
        term.print(sc, at, [line(index)])?;
        at += seconds(0.01);
    }
    noise(sc, 1, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// Opener 3: the atom again, its orbits whirling, the camera swinging around it.
pub fn atom(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-atom");
    let sc = &mut scene;
    let length = seconds_of(&window);
    let mut elements = vec![orb("nucleus", [960.0, 560.0, 0.0], 130.0, 1200, "accent")];
    let orbits = [(0.35_f32, 0.0_f32), (0.629, 0.951), (-0.629, -0.951)];
    for index in 0..orbits.len() {
        elements.push(json!({
            "kind": "form", "id": format!("orbit-{index}"), "at": [960, 560, 0],
            "tone": "request", "points": 480, "tilt": 0.0,
            "shapes": [{ "shape": "torus", "radius": 330, "tube": 8 }]
        }));
    }
    elements.extend(swarm("ball", 18, 41, [2600.0, 1500.0], [600.0, 1600.0]));
    elements.push(typed(
        "shout",
        [960.0, 170.0, 0.0],
        160.0,
        Face::Shout,
        &[("ATOMS!", "accent")],
    ));
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    for (index, (pitch, roll)) in orbits.iter().enumerate() {
        let id = format!("orbit-{index}");
        s.channel(sc, &format!("{id}.pitch"), *pitch);
        s.channel(sc, &format!("{id}.roll"), *roll);
        s.channel(sc, &format!("{id}.solid"), 0.0);
        s.ease(sc, &format!("{id}.rotation"), 0, 6.0, length, Ease::Linear);
    }
    tumble(s, sc, "ball", 18, length);
    s.channel(sc, "camera.yaw", -0.5);
    s.channel(sc, "camera.roll", 0.14);
    s.ease(sc, "camera.yaw", 0, 0.5, length, Ease::Linear);
    s.ease(sc, "camera.roll", 0, -0.12, length, Ease::Linear);
    s.ease(sc, "camera.z", 0, 240.0, length, Ease::Linear);
    s.set(sc, "camera.quake", 0, 1.2);
    s.charge(sc, "nucleus", 0, 1.3, 0.0);
    slam_word(s, sc, "shout", seconds(0.02), 1.9, 1.0);
    later.hit("nucleus.pulse", seconds(0.02), 1.2, 0.0);
    later.hit("post.chroma", seconds(0.02), 0.5, 0.0);
    later.run(s, sc);
    noise(sc, 2, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// CUBES! ARE! BALLS! A wall of tumbling cubes; each shouted word slams in
/// on its drum and the camera cuts; on "BALLS" every cube rounds into a ball.
pub fn cubes(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-cubes");
    let sc = &mut scene;
    let length = seconds_of(&window);
    let cube = |id: &str, at: [f32; 3], size: f32, tone: &str, points: u32| {
        form(
            id,
            at,
            points,
            tone,
            json!([
            { "shape": "box", "size": [size, size, size], "edges": 0.6 },
            { "shape": "sphere", "radius": size * 0.62 }]),
        )
    };
    let mut elements = vec![cube("hero", [960.0, 600.0, 0.0], 360.0, "accent", 1600)];
    const RING: u32 = 14;
    for index in 0..RING {
        let angle = index as f32 * std::f32::consts::TAU / RING as f32 + 0.2;
        let reach = if index % 2 == 0 { 700.0 } else { 1050.0 };
        let at = [
            960.0 + reach * angle.cos(),
            600.0 + reach * 0.55 * angle.sin(),
            250.0 + 500.0 * hash(index, 5),
        ];
        elements.push(cube(
            &format!("cube-{index}"),
            at,
            130.0 + 60.0 * hash(index, 6),
            TONES[index as usize % TONES.len()],
            360,
        ));
    }
    let words = [
        ("w-cubes", "CUBES!", "plain"),
        ("w-are", "ARE!", "warning"),
        ("w-balls", "BALLS!", "error"),
    ];
    for (id, text, tone) in words {
        elements.push(typed(
            id,
            [960.0, 560.0, -300.0],
            160.0,
            Face::Shout,
            &[(text, tone)],
        ));
    }
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    let [cubes_at, are_at, balls_at] = CUBES_WORDS.map(seconds);
    for (id, ..) in words {
        s.channel(sc, &format!("{id}.opacity"), 0.0);
    }
    // Everything tumbles; the hero spins hardest.
    s.channel(sc, "hero.pitch", 0.5);
    s.ease(sc, "hero.pitch", 0, 3.4, length, Ease::Linear);
    s.ease(sc, "hero.rotation", 0, 4.0, length, Ease::Linear);
    for index in 0..RING {
        let id = format!("cube-{index}");
        let turn = if index % 2 == 0 { 4.0 } else { -4.0 };
        s.ease(sc, &format!("{id}.pitch"), 0, turn, length, Ease::Linear);
        s.ease(
            sc,
            &format!("{id}.rotation"),
            0,
            -turn,
            length,
            Ease::Linear,
        );
    }
    // "CUBES!": a lunge into the wall of cubes.
    for (property, value) in [
        ("camera.z", -500.0),
        ("camera.yaw", -0.3),
        ("camera.roll", 0.08),
        ("camera.quake", 0.9),
    ] {
        s.channel(sc, property, value);
    }
    s.ease(sc, "camera.z", cubes_at, 60.0, 0.5, Ease::CubicOut);
    s.ease(sc, "camera.yaw", cubes_at, 0.1, 0.55, Ease::CubicOut);
    s.set(sc, "w-cubes.opacity", cubes_at, 1.0);
    slam_word(s, sc, "w-cubes", cubes_at, 3.0, 1.6);
    later.slam(cubes_at, [0.0, 1.0], 0.9);
    // "ARE!": cut to a Dutch angle from below.
    s.set(sc, "w-cubes.opacity", are_at, 0.0);
    s.set(sc, "w-are.opacity", are_at, 1.0);
    slam_word(s, sc, "w-are", are_at, 2.8, 1.8);
    later.cut(
        are_at,
        Angle {
            y: 60.0,
            z: 120.0,
            yaw: 0.45,
            pitch: -0.3,
            roll: -0.16,
            x: 0.0,
        },
    );
    later.glide(
        are_at,
        Angle {
            y: 40.0,
            z: 200.0,
            yaw: 0.3,
            pitch: -0.22,
            roll: -0.12,
            x: 0.0,
        },
        0.55,
    );
    later.slam(are_at, [1.0, 0.3], 0.8);
    // "BALLS!": every cube rounds into a ball, and the camera punches in.
    s.set(sc, "w-are.opacity", balls_at, 0.0);
    s.set(sc, "w-balls.opacity", balls_at, 1.0);
    slam_word(s, sc, "w-balls", balls_at, 3.2, 1.8);
    later.cut(
        balls_at,
        Angle {
            z: 260.0,
            ..Angle::default()
        },
    );
    later.glide(
        balls_at,
        Angle {
            z: 120.0,
            roll: 0.05,
            ..Angle::default()
        },
        length - CUBES_WORDS[2] as f32,
    );
    later.slam(balls_at, [0.0, 1.0], 1.0);
    s.morph(sc, "hero", balls_at, 1, 0.28);
    later.hit("hero.pulse", balls_at + seconds(0.2), 1.5, 0.0);
    for index in 0..RING {
        let id = format!("cube-{index}");
        let at = balls_at + seconds(0.015 * f64::from(index));
        s.morph(sc, &id, at, 1, 0.26);
        later.hit(&format!("{id}.pulse"), at + seconds(0.2), 1.2, 0.0);
    }
    s.set(sc, "camera.quake", balls_at, 1.5);
    later.run(s, sc);
    let mut confetti = ConfettiActor::declare(
        sc,
        "confetti",
        &ConfettiPlan::new([960.0, 1120.0])
            .seed(11)
            .count(260)
            .speed(2600.0),
    )?;
    confetti.burst(sc, balls_at + seconds(0.04));
    // The caption is the shout.
    let ends = [are_at, balls_at, seconds(f64::from(length))];
    let plan = [("CUBES!", cubes_at), ("ARE!", are_at), ("BALLS!", balls_at)]
        .into_iter()
        .zip(ends)
        .fold(
            SubtitlesPlan::new([960.0, 975.0], 1300.0)
                .size(64.0)
                .face(Face::Shout)
                .highlight(Tone::Error),
            |plan, ((text, at), end)| plan.word(text, at, end),
        );
    psychopomp::subtitles::SubtitlesActor::declare(sc, "shout", &plan)?;
    Ok(window.finish(scene, film)?.into())
}

/// The data: balls with dots are the future; cubes are not.
pub fn chart(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-chart");
    let sc = &mut scene;
    let plan = BarsPlan::new(
        [560.0, 330.0],
        1180.0,
        AxisPlan::new([0.0, 100.0]).every(25.0).unit("%"),
    )
    .row_height(170.0)
    .size(46.0)
    .series(BarSeriesPlan::new("future", "the future", Tone::Error))
    .row("balls", "balls")
    .row("dots", "dots")
    .row("cubes", "cubes")
    .readout(ReadoutFormat::new(0).unit("%"));
    let mut bars = BarsActor::declare(sc, "future", &plan)?;
    bars.channel(sc, "opacity", 1.0);
    bars.channel(sc, "axes", 1.0);
    for (row, from, to) in [
        ("balls", 30.0, 100.0),
        ("dots", 20.0, 100.0),
        ("cubes", 60.0, 100.0),
    ] {
        let bar = bars.channel(sc, &format!("bar.{row}.future"), from);
        sc.ease(&bar, 0, to, 0.14, Ease::CubicOut);
    }
    let mut confetti = ConfettiActor::declare(
        sc,
        "confetti",
        &ConfettiPlan::new([1740.0, 420.0])
            .seed(9)
            .count(260)
            .speed(2400.0),
    )?;
    confetti.burst(sc, seconds(0.08));
    noise(sc, 5, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// A gauge of ball levels, pinned past the red.
pub fn meter(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-meter");
    let sc = &mut scene;
    let plan = MeterPlan::ring(
        [960.0, 500.0],
        330.0,
        AxisPlan::new([0.0, 100.0]).every(10.0).unit("%"),
    )
    .label("BALL LEVELS")
    .threshold(60.0, Tone::Warning)
    .threshold(85.0, Tone::Error)
    .readout(ReadoutFormat::new(0).unit("%"));
    let mut gauge = MeterActor::declare(sc, "gauge", &plan, 10.0)?;
    gauge.channel(sc, "opacity", 1.0);
    gauge.channel(sc, "reveal", 1.0);
    let value = gauge.channel(sc, "value", 10.0);
    sc.ease(&value, 0, 100.0, 0.16, Ease::CubicOut);
    gauge.flash(sc, seconds(0.16), 1.0);
    noise(sc, 6, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// The chimp, electrified.
pub fn chimp(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-chimp");
    let sc = &mut scene;
    let length = seconds_of(&window);
    let mut elements = andy::figure();
    elements.extend(swarm("ball", 16, 23, [2600.0, 1400.0], [500.0, 1500.0]));
    for (index, (marker, ball)) in [("l-hand", 0), ("r-hand", 2), ("head", 4)]
        .into_iter()
        .enumerate()
    {
        elements.push(
            json!({ "kind": "bolt", "id": format!("bolt-{index}"), "from": marker,
            "to": format!("ball-{ball}"), "strikes": 4, "tone": "request" }),
        );
    }
    elements.push(typed(
        "shout",
        [960.0, 170.0, 0.0],
        160.0,
        Face::Shout,
        &[("A CHIMP!", "plain")],
    ));
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    andy::hold(s, sc, &CHIMP);
    s.channel(sc, "floor.opacity", 0.5);
    s.channel(sc, "floor.spin", 0.0);
    tumble(s, sc, "ball", 16, length);
    later.cut(
        0,
        Angle {
            yaw: -0.6,
            z: 60.0,
            roll: -0.1,
            ..Angle::default()
        },
    );
    later.glide(
        0,
        Angle {
            yaw: 0.2,
            z: 200.0,
            roll: 0.06,
            ..Angle::default()
        },
        length,
    );
    for index in 0..3 {
        s.zap(
            sc,
            &format!("bolt-{index}"),
            seconds(0.01 + 0.04 * index as f64),
        );
    }
    for id in MARKERS {
        later.hit(&format!("{id}.pulse"), seconds(0.04), 1.2, 0.0);
        s.set(sc, &format!("{id}.hurt"), 0, 0.6);
    }
    s.set(sc, "camera.quake", 0, 1.3);
    slam_word(s, sc, "shout", seconds(0.01), 1.9, 1.0);
    later.hit("post.chroma", 0, 0.5, 0.0);
    later.run(s, sc);
    noise(sc, 4, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// It is raining basketballs.
pub fn rain(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-rain");
    let sc = &mut scene;
    const COUNT: u32 = 22;
    let floor = 900.0;
    let mut elements = vec![
        json!({ "kind": "shape", "id": "court", "at": [960, floor + 2.0, 0],
        "shape": { "rect": [2000, 4] }, "fill": "warning", "stroke": null }),
    ];
    for index in 0..COUNT {
        let h = |k: u32| hash(index, 300 + k);
        let radius = lerp(90.0, 190.0, h(1));
        let at = [
            lerp(-300.0, 2220.0, h(2)),
            floor - radius,
            lerp(-200.0, 900.0, h(3)),
        ];
        elements.push(orb(&format!("ball-{index}"), at, radius, 500, "warning"));
    }
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let length = seconds_of(&window);
    for index in 0..COUNT {
        let id = format!("ball-{index}");
        let land = seconds(lerp(0.02, f64::from(length) as f32, hash(index, 330)) as f64);
        drop(
            s,
            sc,
            &id,
            land,
            lerp(700.0, 1500.0, hash(index, 331)),
            0.5,
            2,
        );
        s.channel(sc, &format!("{id}.opacity"), 1.0);
        s.ease(sc, &format!("{id}.rotation"), 0, 4.0, length, Ease::Linear);
    }
    s.channel(sc, "camera.pitch", -0.22);
    s.channel(sc, "camera.y", 140.0);
    s.ease(sc, "camera.z", 0, 140.0, length, Ease::Linear);
    s.set(sc, "camera.quake", 0, 1.1);
    noise(sc, 7, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// A ball bursts against the shields of its neighbours.
pub fn shield(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-shield");
    let sc = &mut scene;
    let mut elements = vec![orb("core", [960.0, 520.0, 0.0], 210.0, 2400, "accent")];
    let ring = [
        (380.0, 300.0, "request"),
        (1540.0, 300.0, "plain"),
        (380.0, 760.0, "error"),
        (1540.0, 760.0, "warning"),
    ];
    for (index, (x, y, tone)) in ring.iter().enumerate() {
        elements.push(orb(
            &format!("guard-{index}"),
            [*x, *y, 80.0],
            90.0,
            700,
            tone,
        ));
        elements.push(json!({ "kind": "shield", "id": format!("shield-{index}"),
            "around": format!("guard-{index}"), "radius": 160, "tone": tone }));
        elements.push(
            json!({ "kind": "bolt", "id": format!("arc-{index}"), "from": "core",
            "to": format!("shield-{index}"), "strikes": 3, "tone": "error" }),
        );
    }
    elements.extend(swarm("ball", 20, 77, [2800.0, 1600.0], [700.0, 1700.0]));
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    let length = seconds_of(&window);
    s.set(sc, "camera.quake", 0, 1.2);
    tumble(s, sc, "ball", 20, length);
    for index in 0..ring.len() {
        s.raise(sc, &format!("shield-{index}"), 0, 0.1);
        s.zap(sc, &format!("arc-{index}"), seconds(0.02 * index as f64));
    }
    s.charge(sc, "core", 0, 1.5, 0.0);
    let pop = seconds(0.06);
    s.clock_for(sc, "core.burst", pop, combustion::DURATION);
    later.jolt(pop, [0.0, 1.0], 1.0);
    later.hit("post.zoom", pop, 0.3, 0.0);
    later.hit("post.chroma", pop, 0.5, 0.0);
    later.run(s, sc);
    s.camera().dolly_zoom(
        sc,
        "core",
        pop,
        -500.0,
        Move::Glide((length - 0.07).max(0.05)),
    )?;
    noise(sc, 3, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// Lasagna sheets tumbling through the dark.
pub fn lasagna(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-lasagna");
    let sc = &mut scene;
    let length = seconds_of(&window);
    let mut elements = Vec::new();
    for index in 0..9_u32 {
        let h = |k: u32| hash(index, 500 + k);
        let at = [
            lerp(200.0, 1720.0, h(1)),
            lerp(150.0, 930.0, h(2)),
            lerp(-100.0, 900.0, h(3)),
        ];
        let tone = if index % 3 == 1 { "error" } else { "warning" };
        elements.push(form(
            &format!("slab-{index}"),
            at,
            420,
            tone,
            json!([{ "shape": "box", "size": [520, 22, 300], "edges": 0.4 }]),
        ));
    }
    elements.push(typed(
        "shout",
        [960.0, 540.0, -200.0],
        160.0,
        Face::Shout,
        &[("LASAGNA!", "warning")],
    ));
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    for index in 0..9_u32 {
        let id = format!("slab-{index}");
        let turn = if index % 2 == 0 { 2.5 } else { -2.5 };
        s.channel(sc, &format!("{id}.pitch"), hash(index, 9) * 3.0);
        s.ease(sc, &format!("{id}.pitch"), 0, turn, length, Ease::Linear);
        s.ease(
            sc,
            &format!("{id}.roll"),
            0,
            -turn * 0.6,
            length,
            Ease::Linear,
        );
        s.ease(sc, &format!("{id}.y"), 0, 160.0, length, Ease::Linear);
    }
    slam_word(s, sc, "shout", 0, 2.0, 1.2);
    s.set(sc, "camera.quake", 0, 1.3);
    s.set(sc, "camera.roll", 0, -0.12);
    noise(sc, 8, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// The fork comes down on a plate of spaghetti and meatballs.
pub fn fork(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-fork");
    let sc = &mut scene;
    let mut elements = andy::figure();
    for index in 0..5 {
        elements.push(andy::noodle(index));
    }
    elements.push(andy::fork());
    elements.push(typed(
        "shout",
        [800.0, 230.0, -100.0],
        160.0,
        Face::Shout,
        &[("WILL SMITH!", "error")],
    ));
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    let length = seconds_of(&window);
    andy::hold(s, sc, &andy::meatballs());
    for id in MARKERS {
        s.channel(sc, &format!("{id}.hurt"), 1.0);
    }
    for id in ["bone-spine", "bone-arms", "bone-legs"] {
        s.channel(sc, &format!("{id}.opacity"), 0.0);
    }
    s.channel(sc, "floor.opacity", 0.5);
    s.channel(sc, "fork.x", 560.0);
    s.channel(sc, "fork.y", -700.0);
    s.ease(
        sc,
        "fork.y",
        0,
        -60.0,
        0.08,
        Ease::CubicBezier([0.55, 0.0, 1.0, 0.6]),
    );
    slam_word(s, sc, "shout", seconds(0.01), 1.8, 0.9);
    later.jolt(seconds(0.08), [0.0, 1.0], 1.0);
    later.hit("post.chroma", seconds(0.08), 0.5, 0.0);
    s.set(sc, "camera.quake", 0, 1.4);
    s.channel(sc, "camera.pitch", 0.35);
    s.channel(sc, "camera.z", 100.0);
    s.ease(sc, "camera.roll", 0, 0.14, length, Ease::Linear);
    later.run(s, sc);
    noise(sc, 9, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// The loupe over a wall of balls: every one has dots.
pub fn loupe(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-loupe");
    let sc = &mut scene;
    let length = seconds_of(&window);
    let mut elements = Vec::new();
    for row in 0..4_u32 {
        for column in 0..7_u32 {
            let index = row * 7 + column;
            let at = [
                180.0 + 260.0 * column as f32 + if row % 2 == 0 { 0.0 } else { 130.0 },
                170.0 + 245.0 * row as f32,
                0.0,
            ];
            elements.push(orb(
                &format!("ball-{index}"),
                at,
                92.0,
                420,
                TONES[index as usize % TONES.len()],
            ));
        }
    }
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    s.set(sc, "camera.quake", 0, 0.8);
    s.ease(sc, "camera.z", 0, 120.0, length, Ease::Linear);
    let anchor = |id: &str| CalloutAnchorPlan::Stage {
        id: id.into(),
        element: id.into(),
        edge: CalloutSide::Center,
        side: None,
    };
    let mut lens = LensActor::declare(
        sc,
        "loupe",
        &LensPlan::circle(anchor("ball-10"), 420.0)
            .anchor(anchor("ball-12"))
            .magnification(3.2),
    )?;
    let presence = lens.channel(sc, "presence", 1.0);
    sc.set(&presence, 0, 1.0);
    lens.move_to(sc, "ball-12", 0)?;
    noise(sc, 10, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// One word, set enormous in a serif: "dots."
pub fn dots(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-dots");
    let sc = &mut scene;
    let length = seconds_of(&window);
    let elements = vec![
        orb("ball", [960.0, 540.0, 600.0], 520.0, 3000, "accent"),
        typed(
            "word",
            [960.0, 520.0, -300.0],
            160.0,
            Face::SerifItalic,
            &[("dots.", "plain")],
        ),
    ];
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    slam_word(s, sc, "word", 0, 2.6, 1.8);
    s.channel(sc, "ball.spin", 6.0);
    s.ease(sc, "camera.roll", 0, 0.2, length, Ease::Linear);
    s.set(sc, "camera.quake", 0, 1.0);
    s.hit(sc, "ball.pulse", 0, 1.4, 0.0);
    noise(sc, 11, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// A swarm of every form at the lens, arcs humming among them.
pub fn swarm_shot(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-swarm");
    let sc = &mut scene;
    let length = seconds_of(&window);
    let mut elements = swarm("ball", 44, 59, [3000.0, 1700.0], [-200.0, 1500.0]);
    for (index, (from, to)) in [(0, 8), (12, 20), (24, 32), (34, 42)].iter().enumerate() {
        elements.push(json!({ "kind": "bolt", "id": format!("hum-{index}"),
            "from": format!("ball-{from}"), "to": format!("ball-{to}"), "tone": "request" }));
    }
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    tumble(s, sc, "ball", 44, length);
    for index in 0..4 {
        s.hum(sc, &format!("hum-{index}"), 0, 1.3, 0.0);
    }
    for index in [6, 18, 30] {
        s.clock_for(sc, &format!("ball-{index}.burst"), 0, 5.2);
    }
    s.channel(sc, "camera.yaw", 0.4);
    s.ease(sc, "camera.yaw", 0, -0.4, length, Ease::Linear);
    s.ease(sc, "camera.z", 0, 400.0, length, Ease::Linear);
    s.set(sc, "camera.quake", 0, 1.5);
    s.set(sc, "post.chroma", 0, 0.4);
    noise(sc, 12, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// BALLS, in the shouting face, filling the frame.
pub fn word(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-word");
    let sc = &mut scene;
    let length = seconds_of(&window);
    let mut elements = swarm("ball", 20, 91, [2600.0, 1400.0], [300.0, 1500.0]);
    elements.push(typed(
        "word",
        [960.0, 540.0, -400.0],
        160.0,
        Face::Shout,
        &[("BALLS", "error")],
    ));
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    tumble(s, sc, "ball", 20, length);
    slam_word(s, sc, "word", 0, 3.4, 2.2);
    s.set(sc, "camera.quake", 0, 1.6);
    s.set(sc, "camera.roll", 0, 0.1);
    s.set(sc, "post.chroma", 0, 0.6);
    noise(sc, 13, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// A wall of balls receding to a vanishing point, seen almost edge on.
pub fn wall(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-wall");
    let sc = &mut scene;
    let length = seconds_of(&window);
    let mut elements = Vec::new();
    for row in 0..5_u32 {
        for column in 0..11_u32 {
            let index = row * 11 + column;
            let at = [
                -400.0 + 300.0 * column as f32,
                60.0 + 240.0 * row as f32,
                0.0,
            ];
            elements.push(orb(
                &format!("ball-{index}"),
                at,
                100.0,
                320,
                TONES[(index as usize * 3) % TONES.len()],
            ));
        }
    }
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    for (property, value) in [
        ("camera.yaw", -0.95),
        ("camera.pivot", 0.0),
        ("camera.z", 300.0),
        ("camera.roll", -0.12),
    ] {
        s.channel(sc, property, value);
    }
    s.ease(sc, "camera.yaw", 0, -0.7, length, Ease::Linear);
    s.ease(sc, "camera.x", 0, 500.0, length, Ease::Linear);
    for index in 0..55_u32 {
        s.hit(
            sc,
            &format!("ball-{index}.pulse"),
            seconds(0.006 * f64::from(index % 11)),
            1.4,
            0.0,
        );
    }
    s.set(sc, "camera.quake", 0, 1.0);
    noise(sc, 14, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// Every ball converges, then ONE colossal drum: they implode into a single
/// point at the center of the frame, which whites out.
pub fn converge(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-converge");
    let sc = &mut scene;
    const COUNT: u32 = 48;
    let elements = swarm("ball", COUNT, 17, [3000.0, 1700.0], [-300.0, 900.0]);
    let homes = elements
        .iter()
        .map(|ball| {
            let at = &ball["at"];
            [
                at[0].as_f64().unwrap() as f32,
                at[1].as_f64().unwrap() as f32,
                at[2].as_f64().unwrap() as f32,
            ]
        })
        .collect::<Vec<_>>();
    let mut elements = elements;
    for (index, (from, to)) in [(0, 4), (5, 9), (10, 14), (15, 19), (20, 24), (25, 29)]
        .iter()
        .enumerate()
    {
        elements.push(json!({ "kind": "bolt", "id": format!("hum-{index}"),
            "from": format!("ball-{from}"), "to": format!("ball-{to}"), "tone": if index % 2 == 0 { "request" } else { "error" } }));
    }
    elements.push(typed(
        "line-1",
        [960.0, 430.0, -250.0],
        150.0,
        Face::Shout,
        &[("BALLS WITH DOTS", "plain")],
    ));
    elements.push(typed(
        "line-2",
        [960.0, 610.0, -250.0],
        150.0,
        Face::Shout,
        &[("FOREVER", "error")],
    ));
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    let hit = seconds(HIT);
    let frenzy = HIT as f32;
    // The frenzy: everything swirls inward and shakes harder.
    s.set(sc, "camera.quake", 0, 1.6);
    s.ease(sc, "camera.quake", 0, 2.0, frenzy, Ease::Linear);
    s.ease(
        sc,
        "camera.roll",
        0,
        0.3,
        frenzy,
        Ease::CubicBezier([0.5, 0.0, 1.0, 1.0]),
    );
    s.ease(sc, "post.chroma", 0, 0.6, frenzy, Ease::Linear);
    tumble(s, sc, "ball", COUNT, seconds_of(&window));
    for (index, [x, y, z]) in homes.iter().enumerate() {
        let id = format!("ball-{index}");
        s.channel(sc, &format!("{id}.scale"), 1.0);
        let gather = Ease::CubicBezier([0.4, 0.0, 1.0, 1.0]);
        s.ease(
            sc,
            &format!("{id}.x"),
            0,
            (960.0 - x) * 0.35,
            frenzy,
            gather,
        );
        s.ease(
            sc,
            &format!("{id}.y"),
            0,
            (540.0 - y) * 0.35,
            frenzy,
            gather,
        );
        s.ease(sc, &format!("{id}.z"), 0, (200.0 - z) * 0.2, frenzy, gather);
    }
    for index in 0..6 {
        s.hum(sc, &format!("hum-{index}"), 0, 1.3, 0.0);
    }
    for (index, line) in ["line-1", "line-2"].into_iter().enumerate() {
        let at = seconds(0.02 + 0.1 * index as f64);
        s.channel(sc, &format!("{line}.opacity"), 0.0);
        s.set(sc, &format!("{line}.opacity"), at, 1.0);
        slam_word(s, sc, line, at, 2.2, 0.85);
        later.jolt(at, [0.0, 1.0], 1.0);
    }
    // BOOM. The shaking stops dead and everything falls into one point.
    let implode = (CONVERGE_REST as f32).max(0.2);
    let pull = Ease::CubicBezier([0.55, 0.0, 1.0, 0.45]);
    later.at(hit, move |s, sc| {
        s.set(sc, "camera.quake", hit, 0.0);
        s.ease(sc, "camera.roll", hit, 0.0, implode, Ease::Smoothstep);
        s.ease(sc, "post.zoom", hit, 0.45, implode, pull);
        s.ease(sc, "post.chroma", hit, 0.9, implode, pull);
    });
    later.jolt(hit, [0.0, 1.0], 1.0);
    for (index, [x, y, z]) in homes.iter().enumerate() {
        let id = format!("ball-{index}");
        s.ease(sc, &format!("{id}.x"), hit, 960.0 - x, implode, pull);
        s.ease(sc, &format!("{id}.y"), hit, 540.0 - y, implode, pull);
        s.ease(sc, &format!("{id}.z"), hit, 200.0 - z, implode, pull);
        s.ease(sc, &format!("{id}.scale"), hit, 0.02, implode, pull);
        later.hit(&format!("{id}.pulse"), hit, 1.4, 0.0);
    }
    for index in 0..6 {
        s.hum(sc, &format!("hum-{index}"), hit, 0.0, 0.0);
    }
    for line in ["line-1", "line-2"] {
        s.ease(sc, &format!("{line}.scale"), hit, 0.0, implode, pull);
        s.ease(
            sc,
            &format!("{line}.opacity"),
            hit + seconds(0.16),
            0.0,
            0.1,
            Ease::Linear,
        );
    }
    // The point flares white, and the film cuts to black.
    let flare = hit + seconds(f64::from(implode) - 0.05);
    s.ease(
        sc,
        "post.flash",
        flare,
        1.0,
        0.05,
        Ease::CubicBezier([0.5, 0.0, 1.0, 1.0]),
    );
    later.run(s, sc);
    let mut confetti = ConfettiActor::declare(
        sc,
        "confetti",
        &ConfettiPlan::new([960.0, 1100.0])
            .seed(4)
            .count(300)
            .speed(2800.0),
    )?;
    confetti.burst(sc, 0);
    let fade = confetti.channel(sc, "opacity", 1.0);
    sc.ease(&fade, hit, 0.0, 0.12, Ease::Linear);
    noise(sc, 15, Some(hit))?;
    Ok(window.finish(scene, film)?.into())
}

/// How long the implosion runs after the hit.
const CONVERGE_REST: f64 = crate::sound::CONVERGE - HIT;
