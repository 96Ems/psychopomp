//! The climax: every voice at once over a montage of everything so far, a
//! cut every third of a second. A lightning storm between balls; a terminal
//! that will not stop printing BALLS; the atom; a ball bursting against
//! shields; the chimp, electrified; a chart of the future; CUBES ARE BALLS;
//! a swarm of every form; the fork; then everything flies at the lens until
//! the frame whites out. The captions give up on words.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, seconds},
    axis::AxisPlan,
    bars::{BarSeriesPlan, BarsActor, BarsPlan},
    callout::CalloutAnchorPlan,
    caption::{CaptionActor, CaptionAlign, CaptionPlan, CaptionSpanPlan},
    confetti::{ConfettiActor, ConfettiPlan},
    effects::combustion,
    lens::{LensActor, LensPlan},
    math::{Vec3, easing::Ease, lerp, random::hash, vec2},
    readout::ReadoutFormat,
    stage::{Camera, Move, StageActor},
    terminal::{TerminalActor, TerminalLinePlan, TerminalPlan},
    text::{TextActor, TextPlan},
    tone::Tone,
};
use serde_json::{Value, json};

use crate::{
    Shot,
    andy::{self, CHIMP, MARKERS},
    kit::{Angle, Later, Window, ball_rect, label, orb, stage},
    sound::{Film, HIT},
};

const STORM_CORE: [f32; 3] = [960.0, 520.0, 0.0];
const STORM_Z: f32 = -60.0;
const STORM_YAW: f32 = -0.2;

const TONES: [&str; 5] = ["accent", "plain", "request", "error", "warning"];

/// What the captions say instead of words, a little more unhinged each cut:
/// knocked off center, recolored, and seeing double.
fn noise(scene: &mut PlanBuilder, shot: u32, until: Option<u64>) -> Result<()> {
    let tones = [Tone::Error, Tone::Warning, Tone::Accent, Tone::Request];
    let text = "[EVERYONE SCREAMING ABOUT BALLS]";
    let x = 960.0 + (hash(shot, 1) - 0.5) * 90.0;
    let y = 960.0 + (hash(shot, 2) - 0.5) * 40.0;
    for (layer, (dx, dy, opacity)) in [(0.0, 0.0, 1.0), (9.0, -6.0, 0.45)].into_iter().enumerate() {
        let tone = tones[(shot as usize + layer) % tones.len()];
        let mut plan = CaptionPlan::line(
            [x + dx, y + dy],
            52.0,
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
            scene.ease(&alpha, until, 0.0, 0.1, Ease::Linear);
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
                1 => json!({ "kind": "form", "id": id, "at": at, "tone": tone, "points": 300,
                    "shapes": [{ "shape": "box", "size": [size * 1.4, size * 1.4, size * 1.4] },
                               { "shape": "sphere", "radius": size },
                               { "shape": "torus", "radius": size, "tube": size * 0.3 }] }),
                _ => json!({ "kind": "form", "id": id, "at": at, "tone": tone, "points": 343,
                    "shapes": [{ "shape": "torus", "radius": size, "tube": size * 0.3 },
                               { "shape": "lattice", "size": [size * 1.5, size * 1.5, size * 1.5] },
                               { "shape": "sphere", "radius": size }] }),
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

/// A lightning storm: bolts leap between balls in quick succession.
pub fn storm(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-storm");
    let sc = &mut scene;
    let length = seconds_of(&window);
    // The ball the film dives out of, charged and struck from every side.
    let mut elements = vec![orb("core", STORM_CORE, 130.0, 1200, "accent")];
    elements.extend(swarm("ball", 16, 3, [1800.0, 950.0], [-300.0, 900.0]));
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
    for (index, to) in [0, 6, 12].into_iter().enumerate() {
        elements.push(json!({ "kind": "bolt", "id": format!("core-bolt-{index}"),
            "from": "core", "to": format!("ball-{to}"), "strikes": 3, "tone": "plain" }));
    }
    elements.push(label(
        "shout",
        [960.0, 170.0, -100.0],
        150.0,
        &[("BALLS!!", "plain")],
    ));
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    s.channel(sc, "camera.z", STORM_Z);
    s.channel(sc, "camera.yaw", STORM_YAW);
    s.channel(sc, "shout.scale", 2.2);
    s.channel(sc, "core.spin", 3.0);
    s.charge(sc, "core", 0, 1.3, 0.0);
    for index in 0..3 {
        s.zap(
            sc,
            &format!("core-bolt-{index}"),
            seconds(0.05 + 0.07 * index as f64),
        );
    }
    s.ease(sc, "camera.z", 0, 140.0, length, Ease::Linear);
    s.ease(sc, "camera.yaw", 0, 0.25, length, Ease::Linear);
    // Still while the match lands, then shaking.
    s.ease(sc, "camera.quake", seconds(0.2), 1.4, 0.15, Ease::Linear);
    s.ease(sc, "post.chroma", seconds(0.2), 0.35, 0.15, Ease::Linear);
    tumble(s, sc, "ball", 16, length);
    for index in (0..16).filter(|index| index % 4 == 0 || index % 4 == 2) {
        s.charge(sc, &format!("ball-{index}"), 0, 1.2, 0.0);
    }
    for (index, _) in pairs.iter().enumerate() {
        let at = seconds(0.02 + 0.04 * index as f64);
        let contact = s.zap(sc, &format!("bolt-{index}"), at);
        later.jolt(contact, [if index % 2 == 0 { 1.0 } else { -1.0 }, 0.5], 0.6);
    }
    s.channel(sc, "shout.opacity", 0.0);
    s.set(sc, "shout.opacity", seconds(0.22), 1.0);
    s.bounce(sc, "shout.scale", seconds(0.22), 1.0, 0.22, 0.35);
    later.run(s, sc);
    noise(sc, 0, None)?;
    let camera = Camera {
        position: Vec3::new(0.0, 0.0, STORM_Z),
        yaw: STORM_YAW,
        ..Camera::new(vec2(1920.0, 1080.0))
    };
    Ok(Shot {
        plan: window.finish(scene, film)?,
        entry: Some(ball_rect(camera, STORM_CORE, 130.0)),
        exit: None,
    })
}

/// A terminal that will not stop.
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
    // The command has already run; the screen is full and still filling.
    let mut plan = TerminalPlan::new([110.0, 60.0], 1700.0, 15)
        .titled("~/the-future — zsh")
        .size(40.0);
    plan.lines.push(TerminalLinePlan::Command {
        id: "run".into(),
        text: "balls --with-dots --future".into(),
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

/// The atom again, its orbits whirling, the camera swinging around it.
pub fn atom(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-atom");
    let sc = &mut scene;
    let length = seconds_of(&window);
    let mut elements = vec![orb("nucleus", [960.0, 560.0, 0.0], 130.0, 1200, "accent")];
    let orbits = [(0.35_f32, 0.0_f32), (0.629, 0.951), (-0.629, -0.951)];
    for (index, _) in orbits.iter().enumerate() {
        elements.push(json!({
            "kind": "form", "id": format!("orbit-{index}"), "at": [960, 560, 0],
            "tone": "request", "points": 480, "tilt": 0.0,
            "shapes": [{ "shape": "torus", "radius": 330, "tube": 8 }]
        }));
    }
    elements.extend(swarm("ball", 18, 41, [2600.0, 1500.0], [600.0, 1600.0]));
    elements.push(label(
        "shout",
        [960.0, 170.0, 0.0],
        150.0,
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
    s.channel(sc, "camera.yaw", -0.45);
    s.channel(sc, "camera.roll", 0.12);
    s.channel(sc, "shout.scale", 1.8);
    s.ease(sc, "camera.yaw", 0, 0.45, length, Ease::Linear);
    s.ease(sc, "camera.roll", 0, -0.1, length, Ease::Linear);
    s.ease(sc, "camera.z", 0, 220.0, length, Ease::Linear);
    s.set(sc, "camera.quake", 0, 1.2);
    s.charge(sc, "nucleus", 0, 1.3, 0.0);
    s.bounce(sc, "shout.scale", seconds(0.02), 1.0, 0.22, 0.35);
    later.hit("nucleus.pulse", seconds(0.02), 1.2, 0.0);
    later.hit("post.chroma", seconds(0.02), 0.5, 0.0);
    later.run(s, sc);
    noise(sc, 2, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// A ball bursts against the shields of its neighbours.
pub fn burst(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-burst");
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
    elements.push(label(
        "shout",
        [960.0, 160.0, -100.0],
        130.0,
        &[("THE FUTURE", "error")],
    ));
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    let length = seconds_of(&window);
    s.channel(sc, "shout.scale", 1.8);
    s.set(sc, "camera.quake", 0, 1.2);
    tumble(s, sc, "ball", 20, length);
    for index in 0..ring.len() {
        s.raise(sc, &format!("shield-{index}"), 0, 0.15);
        s.zap(sc, &format!("arc-{index}"), seconds(0.03 * index as f64));
    }
    s.charge(sc, "core", 0, 1.5, 0.0);
    let pop = seconds(0.1);
    s.clock_for(sc, "core.burst", pop, combustion::DURATION);
    later.jolt(pop, [0.0, 1.0], 1.0);
    later.hit("post.zoom", pop, 0.3, 0.0);
    later.hit("post.chroma", pop, 0.5, 0.0);
    s.bounce(sc, "shout.scale", pop, 1.0, 0.22, 0.35);
    later.run(s, sc);
    s.camera()
        .dolly_zoom(sc, "core", pop, -500.0, Move::Glide(length - 0.1))?;
    noise(sc, 3, None)?;
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
    elements.push(label(
        "shout",
        [960.0, 160.0, 0.0],
        150.0,
        &[("A CHIMP!", "plain")],
    ));
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    andy::hold(s, sc, &CHIMP);
    s.channel(sc, "floor.opacity", 0.5);
    s.channel(sc, "floor.spin", 0.0);
    s.channel(sc, "shout.scale", 1.8);
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
            seconds(0.02 + 0.05 * index as f64),
        );
    }
    for id in MARKERS {
        later.hit(&format!("{id}.pulse"), seconds(0.05), 1.2, 0.0);
        s.set(sc, &format!("{id}.hurt"), 0, 0.6);
    }
    s.set(sc, "camera.quake", 0, 1.3);
    s.bounce(sc, "shout.scale", seconds(0.02), 1.0, 0.22, 0.35);
    later.hit("post.chroma", 0, 0.5, 0.0);
    later.run(s, sc);
    noise(sc, 4, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// The data: balls with dots are the future.
pub fn chart(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-chart");
    let sc = &mut scene;
    let mut title = TextActor::declare(
        sc,
        "title",
        &TextPlan::new("THE FUTURE", [960.0, 190.0])
            .size(96.0)
            .color([237, 129, 126]),
    )?;
    title.channel(sc, "opacity", 1.0);
    let plan = BarsPlan::new(
        [560.0, 400.0],
        1180.0,
        AxisPlan::new([0.0, 100.0]).every(25.0).unit("%"),
    )
    .row_height(170.0)
    .size(46.0)
    .series(BarSeriesPlan::new("future", "the future", Tone::Error))
    .row("balls", "balls with dots")
    .row("rest", "everything else")
    .readout(ReadoutFormat::new(0).unit("%"));
    let mut bars = BarsActor::declare(sc, "future", &plan)?;
    bars.channel(sc, "opacity", 1.0);
    bars.channel(sc, "axes", 1.0);
    let bar = bars.channel(sc, "bar.balls.future", 20.0);
    sc.ease(&bar, 0, 100.0, 0.16, Ease::CubicOut);
    let rest = bars.channel(sc, "bar.rest.future", 0.4);
    sc.set(&rest, 0, 0.4);
    let mut confetti = ConfettiActor::declare(
        sc,
        "confetti",
        &ConfettiPlan::new([1740.0, 470.0])
            .seed(9)
            .count(260)
            .speed(2400.0),
    )?;
    confetti.burst(sc, seconds(0.12));
    noise(sc, 5, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// Seventh: "CUBES ARE BALLS". A dotted cube rounds into a sphere under
/// the words, and a ring of smaller cubes follows. Held long enough to read.
pub fn cubes(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-cubes");
    let sc = &mut scene;
    let length = seconds_of(&window);
    let cube = |id: &str, at: [f32; 3], size: f32, tone: &str, points: u32| {
        json!({ "kind": "form", "id": id, "at": at, "tone": tone, "points": points,
            "shapes": [{ "shape": "box", "size": [size, size, size], "edges": 0.6 },
                       { "shape": "sphere", "radius": size * 0.62 }] })
    };
    let mut elements = vec![cube("hero", [960.0, 610.0, 0.0], 300.0, "accent", 1400)];
    for index in 0..8_u32 {
        let angle = index as f32 * std::f32::consts::TAU / 8.0 + 0.3;
        let at = [
            960.0 + 620.0 * angle.cos(),
            610.0 + 300.0 * angle.sin(),
            300.0 + 200.0 * hash(index, 5),
        ];
        let id = format!("cube-{index}");
        elements.push(cube(
            &id,
            at,
            110.0,
            TONES[index as usize % TONES.len()],
            300,
        ));
    }
    elements.push(label(
        "shout",
        [960.0, 175.0, -60.0],
        130.0,
        &[("CUBES ", "plain"), ("ARE ", "muted"), ("BALLS", "accent")],
    ));
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    s.channel(sc, "shout.scale", 1.8);
    s.channel(sc, "shout.opacity", 0.0);
    s.set(sc, "shout.opacity", seconds(0.03), 1.0);
    s.bounce(sc, "shout.scale", seconds(0.03), 1.0, 0.25, 0.35);
    later.jolt(seconds(0.03), [0.0, 1.0], 0.9);
    later.hit("post.chroma", seconds(0.03), 0.45, 0.0);
    // The cube tumbles, then rounds off into a ball on "BALLS".
    s.channel(sc, "hero.pitch", 0.5);
    s.ease(sc, "hero.pitch", 0, 2.2, length, Ease::Linear);
    s.ease(sc, "hero.rotation", 0, 2.6, length, Ease::Linear);
    let round = seconds(0.2);
    s.morph(sc, "hero", round, 1, 0.4);
    later.hit("hero.pulse", round + seconds(0.4), 1.2, 0.0);
    for index in 0..8_u32 {
        let id = format!("cube-{index}");
        s.ease(sc, &format!("{id}.pitch"), 0, 3.0, length, Ease::Linear);
        s.ease(sc, &format!("{id}.rotation"), 0, -3.0, length, Ease::Linear);
        s.morph(sc, &id, round + seconds(0.03 * f64::from(index)), 1, 0.35);
    }
    s.set(sc, "camera.quake", 0, 1.1);
    s.channel(sc, "camera.yaw", -0.18);
    s.ease(sc, "camera.yaw", 0, 0.18, length, Ease::Linear);
    s.ease(sc, "camera.z", 0, 120.0, length, Ease::Linear);
    later.run(s, sc);
    noise(sc, 6, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// A swarm of every form, tumbling and morphing, arcs humming among them.
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
    s.ease(sc, "camera.z", 0, 300.0, length, Ease::Linear);
    s.set(sc, "camera.quake", 0, 1.5);
    s.set(sc, "post.chroma", 0, 0.4);
    noise(sc, 7, None)?;
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
    // The name stays clear of the fork, which comes down to its right.
    elements.push(label(
        "shout",
        [820.0, 250.0, -100.0],
        120.0,
        &[("WILL SMITH!", "error")],
    ));
    elements.extend(swarm("ball", 16, 13, [2600.0, 1400.0], [600.0, 1500.0]));
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
        0.1,
        Ease::CubicBezier([0.55, 0.0, 1.0, 0.6]),
    );
    s.channel(sc, "shout.scale", 1.8);
    s.bounce(sc, "shout.scale", seconds(0.02), 1.0, 0.22, 0.35);
    tumble(s, sc, "ball", 16, length);
    later.jolt(seconds(0.1), [0.0, 1.0], 1.0);
    later.hit("post.chroma", seconds(0.1), 0.5, 0.0);
    s.set(sc, "camera.quake", 0, 1.4);
    s.channel(sc, "camera.pitch", 0.35);
    s.channel(sc, "camera.z", 100.0);
    s.ease(sc, "camera.roll", 0, 0.12, length, Ease::Linear);
    later.run(s, sc);
    noise(sc, 8, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// Three of the theory's claims burn away; the truth forms out of the ash.
pub fn dissolve(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-dissolve");
    let sc = &mut scene;
    let card = |id: &str, at: [f32; 3], title: &str, tone: &str| json!({ "kind": "card", "id": id, "at": at, "size": [520, 150], "title": title, "tone": tone });
    let mut elements = vec![
        card("atoms", [520.0, 300.0, 0.0], "ATOMS", "request"),
        card("dreams", [1400.0, 300.0, 0.0], "HOPES & DREAMS", "warning"),
        card("basketballs", [520.0, 780.0, 0.0], "BASKETBALLS", "warning"),
        card("truth", [1400.0, 780.0, 0.0], "BALLS WITH DOTS", "accent"),
    ];
    elements.extend(swarm("ball", 16, 31, [2600.0, 1400.0], [600.0, 1500.0]));
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let length = seconds_of(&window);
    for (index, id) in ["atoms", "dreams", "basketballs"].into_iter().enumerate() {
        s.dissolve(sc, id, seconds(0.01 + 0.04 * index as f64));
        s.charge(sc, id, 0, 1.2, 0.0);
    }
    s.materialize(sc, "truth", 0, length.max(0.2));
    s.scan(sc, "truth", 0, length.max(0.2));
    tumble(s, sc, "ball", 16, length);
    s.set(sc, "camera.quake", 0, 1.3);
    s.channel(sc, "camera.roll", -0.08);
    s.ease(sc, "camera.roll", 0, 0.08, length, Ease::Linear);
    s.ease(sc, "camera.z", 0, 160.0, length, Ease::Linear);
    noise(sc, 3, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// A loupe sweeps across one enormous ball: dots, dots, dots.
pub fn loupe(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-loupe");
    let sc = &mut scene;
    let mut elements = vec![orb("giant", [960.0, 540.0, -200.0], 420.0, 2600, "accent")];
    elements.extend(swarm("ball", 12, 71, [2800.0, 1500.0], [700.0, 1600.0]));
    elements.push(label(
        "shout",
        [960.0, 120.0, -100.0],
        120.0,
        &[("DOTS!!", "accent")],
    ));
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let length = seconds_of(&window);
    s.channel(sc, "giant.spin", 6.0);
    s.channel(sc, "shout.scale", 1.8);
    s.bounce(sc, "shout.scale", seconds(0.02), 1.0, 0.22, 0.35);
    s.set(sc, "camera.quake", 0, 1.1);
    tumble(s, sc, "ball", 12, length);
    s.hit(sc, "giant.pulse", 0, 1.0, 0.2);
    let mut lens = LensActor::declare(
        sc,
        "lens",
        &LensPlan::circle(
            CalloutAnchorPlan::Point {
                id: "center".into(),
                at: [960.0, 560.0],
                side: None,
            },
            420.0,
        )
        .magnification(3.0),
    )?;
    let presence = lens.channel(sc, "presence", 1.0);
    sc.set(&presence, 0, 1.0);
    let x = lens.channel(sc, "x", -420.0);
    sc.ease(&x, 0, 420.0, length, Ease::Linear);
    let y = lens.channel(sc, "y", 120.0);
    sc.ease(&y, 0, -120.0, length, Ease::Linear);
    noise(sc, 8, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// BALLS WITH DOTS ARE THE FUTURE, over a wall of dots rushing at the lens.
pub fn future(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-future");
    let sc = &mut scene;
    let mut elements = vec![
        json!({ "kind": "form", "id": "wall", "at": [960, 540, 600], "tone": "accent",
        "points": 1200, "tilt": 0.0, "shapes": [{ "shape": "plane", "size": [2000, 1100] }] }),
    ];
    elements.extend(swarm("ball", 20, 91, [2600.0, 1500.0], [-200.0, 500.0]));
    elements.push(label(
        "line-1",
        [960.0, 420.0, -250.0],
        120.0,
        &[("BALLS WITH DOTS", "plain")],
    ));
    elements.push(label(
        "line-2",
        [960.0, 620.0, -250.0],
        120.0,
        &[("ARE THE FUTURE", "error")],
    ));
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    let length = seconds_of(&window);
    s.channel(sc, "wall.spin", 0.0);
    s.channel(sc, "wall.opacity", 0.5);
    s.ease(sc, "wall.roll", 0, 0.5, length, Ease::Linear);
    tumble(s, sc, "ball", 20, length);
    s.set(sc, "camera.quake", 0, 1.6);
    s.ease(sc, "camera.z", 0, 300.0, length, Ease::Linear);
    for (index, line) in ["line-1", "line-2"].into_iter().enumerate() {
        let at = seconds(0.01 + 0.06 * index as f64);
        s.channel(sc, &format!("{line}.opacity"), 0.0);
        s.channel(sc, &format!("{line}.scale"), 2.0);
        s.set(sc, &format!("{line}.opacity"), at, 1.0);
        s.bounce(sc, &format!("{line}.scale"), at, 1.0, 0.2, 0.35);
        later.jolt(at, [0.0, 1.0], 1.0);
    }
    later.hit("post.chroma", 0, 0.6, 0.2);
    later.run(s, sc);
    noise(sc, 10, None)?;
    Ok(window.finish(scene, film)?.into())
}

/// Everything at once, then ONE colossal drum hit: every ball implodes
/// into a single point at the center of the frame, which whites out.
pub fn finale(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("chaos-finale");
    let sc = &mut scene;
    const COUNT: u32 = 44;
    let elements = swarm("ball", COUNT, 17, [2600.0, 1500.0], [-300.0, 900.0]);
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
    for (index, (from, to)) in [(0, 4), (5, 9), (10, 14), (15, 19)].iter().enumerate() {
        elements.push(json!({ "kind": "bolt", "id": format!("hum-{index}"),
            "from": format!("ball-{from}"), "to": format!("ball-{to}"), "tone": "request" }));
    }
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    let hit = seconds(HIT);
    let frenzy = HIT as f32;
    for index in 0..COUNT {
        s.channel(sc, &format!("ball-{index}.scale"), 1.0);
    }
    // The frenzy: everything tumbles and shakes.
    s.set(sc, "camera.quake", 0, 1.6);
    s.ease(sc, "camera.quake", 0, 2.0, frenzy, Ease::Linear);
    s.ease(sc, "camera.roll", 0, 0.12, frenzy, Ease::Smoothstep);
    s.ease(sc, "post.chroma", 0, 0.5, frenzy, Ease::Linear);
    tumble(s, sc, "ball", COUNT, seconds_of(&window));
    for index in 0..4 {
        s.hum(sc, &format!("hum-{index}"), 0, 1.2, 0.0);
    }
    s.ease(
        sc,
        "camera.z",
        0,
        260.0,
        frenzy,
        Ease::CubicBezier([0.5, 0.0, 1.0, 1.0]),
    );
    s.ease(sc, "post.zoom", 0, 0.2, frenzy, Ease::Linear);
    // BOOM. The shaking stops dead and everything falls into one point.
    let implode = 0.34;
    let pull = Ease::CubicBezier([0.55, 0.0, 1.0, 0.45]);
    later.at(hit, move |s, sc| {
        s.set(sc, "camera.quake", hit, 0.0);
        s.ease(sc, "camera.roll", hit, 0.0, implode, Ease::Smoothstep);
        s.ease(sc, "post.zoom", hit, 0.45, implode, pull);
        s.ease(sc, "camera.z", hit, 0.0, implode, Ease::CubicOut);
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
    for index in 0..4 {
        s.hum(sc, &format!("hum-{index}"), hit, 0.0, 0.0);
    }
    // The point flares white, and the film cuts to black.
    let flare = hit + seconds(f64::from(implode) - 0.04);
    s.ease(
        sc,
        "post.flash",
        flare,
        1.0,
        0.1,
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
    sc.ease(&fade, hit, 0.0, 0.15, Ease::Linear);
    noise(sc, 9, Some(hit))?;
    Ok(window.finish(scene, film)?.into())
}
