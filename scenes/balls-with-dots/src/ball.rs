//! In the beginning, there were balls: drifting bokeh in the dark, a title,
//! and a point of light that ignites into one ball of dots, seen close and
//! shallow while focus finds its dots. A second ball was behind it all
//! along; its dots multiply. Then a chant in which every spoken "balls" is
//! a new shot and a new ball, until the last one swallows the camera.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, seconds},
    component_prototype::{HEADER, HeaderEvent, HeaderPlan, HeaderReflection, HeaderSplit},
    math::{easing::Ease, lerp, random::hash},
    stage::{Move, StageActor},
    subtitles::SubtitlesPlan,
    tone::Tone,
};
use serde_json::{Value, json};

use crate::{
    Shot,
    kit::{Later, Sphere, Window, caption, lower, orb, stage, subtitles},
    sound::Film,
};

const ONE: [f32; 3] = [960.0, 540.0, 0.0];
/// The second ball waits behind the first, out of focus.
const TWO: [f32; 3] = [1720.0, 400.0, 900.0];
const BOKEH: u32 = 12;

/// A title in large type that rises word by word over its own reflection.
/// `left` is where its measured line starts so that it sits centered.
pub fn title(
    scene: &mut PlanBuilder,
    id: &str,
    text: &str,
    left: f32,
    top: f32,
    show: u64,
    hide: u64,
) -> Result<()> {
    let plan = HeaderPlan {
        origin: [left, top],
        text: text.into(),
        font_size: 76.0,
        width: 1500.0,
        split: HeaderSplit::Words,
        stagger_millis: 95,
        duration_seconds: 0.8,
        reflection: Some(HeaderReflection {
            opacity: 0.22,
            depth: 46.0,
            gap: 4.0,
        }),
        visible: false,
        events: vec![HeaderEvent {
            at_nanos: show,
            visible: true,
        }],
    };
    let actor = scene.actor(id, HEADER, &plan)?;
    let opacity = scene.channel(&actor, "opacity", 1.0);
    scene.ease(&opacity, hide, 0.0, 0.7, Ease::Smoothstep);
    let y = scene.channel(&actor, "y", top);
    // A slow drift upward while it holds.
    scene.ease(
        &y,
        show,
        top - 22.0,
        ((hide - show) as f64 / 1e9) as f32 + 0.7,
        Ease::Linear,
    );
    Ok(())
}

/// What each chanted "balls" spawns: a shape, its tone, where, and how big.
enum Kind {
    Orb(u32),
    Cube,
    Torus,
    Lattice,
    Cylinder,
}

struct Spawn {
    kind: Kind,
    tone: &'static str,
    at: [f32; 3],
    size: f32,
}

/// Thirteen balls for thirteen "balls": spheres first, then cubes, rings,
/// and grids, spiralling out from the first two.
fn spawns() -> Vec<Spawn> {
    use Kind::*;
    let plan: [(Kind, &str, f32, f32); 13] = [
        (Orb(700), "plain", 95.0, -120.0),
        (Orb(500), "accent", 80.0, 160.0),
        (Orb(900), "request", 110.0, -320.0),
        (Cube, "plain", 70.0, 60.0),
        (Orb(600), "accent", 85.0, -200.0),
        (Cube, "accent", 80.0, 220.0),
        (Torus, "plain", 95.0, -380.0),
        (Orb(400), "error", 65.0, 120.0),
        (Torus, "accent", 80.0, -60.0),
        (Lattice, "plain", 85.0, -460.0),
        (Cylinder, "request", 70.0, 200.0),
        (Orb(1200), "accent", 120.0, -250.0),
        (Orb(1600), "accent", 150.0, 40.0),
    ];
    let center = [1175.0, 525.0];
    plan.into_iter()
        .enumerate()
        .map(|(index, (kind, tone, size, z))| {
            let angle = -1.9 + index as f32 * 2.399_963;
            let radius = 420.0 + 46.0 * index as f32;
            Spawn {
                kind,
                tone,
                at: [
                    center[0] + radius * angle.cos(),
                    center[1] + radius * 0.62 * angle.sin(),
                    z,
                ],
                size,
            }
        })
        .collect()
}

fn element(id: &str, spawn: &Spawn) -> Value {
    let form = |shapes: Value, points: u32| {
        json!({ "kind": "form", "id": id, "at": spawn.at, "tone": spawn.tone, "points": points,
                "shapes": shapes })
    };
    let s = spawn.size;
    match spawn.kind {
        Kind::Orb(points) => orb(id, spawn.at, s, points, spawn.tone),
        Kind::Cube => form(
            json!([{ "shape": "box", "size": [s * 1.5, s * 1.5, s * 1.5], "edges": 0.55 }]),
            520,
        ),
        Kind::Torus => form(
            json!([{ "shape": "torus", "radius": s, "tube": s * 0.32 }]),
            640,
        ),
        Kind::Lattice => form(
            json!([{ "shape": "lattice", "size": [s * 1.6, s * 1.6, s * 1.6] }]),
            729,
        ),
        Kind::Cylinder => form(
            json!([{ "shape": "cylinder", "radius": s * 0.7, "height": s * 1.6 }]),
            560,
        ),
    }
}

/// The first ball: a form that starts as a point of light and blooms into
/// a sphere of dots.
const SEED: Sphere = Sphere {
    at: ONE,
    radius: 150.0,
    points: 1100,
    rotation: 0.0,
    spin: 1.0,
};
/// The chant's last ball, which the camera dives into.
pub fn last_ball() -> Sphere {
    let spawn = &spawns()[12];
    Sphere {
        at: spawn.at,
        radius: spawn.size,
        points: 1600,
        rotation: 0.0,
        spin: 0.0,
    }
}

pub fn segment(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("one-ball");
    let sc = &mut scene;
    let t = |at: u64| window.t(at);
    let spawns = spawns();
    let mut elements = vec![
        json!({ "kind": "form", "id": "ball-1", "at": ONE, "tone": "accent", "points": SEED.points,
                "shapes": [{ "shape": "sphere", "radius": 10 }, { "shape": "sphere", "radius": SEED.radius }] }),
        json!({ "kind": "ring", "id": "shock", "at": ONE, "radius": 150, "thickness": 2.5, "tone": "accent" }),
        orb("ball-2-few", TWO, 260.0, 40, "plain"),
        orb("ball-2", TWO, 260.0, 3200, "plain"),
    ];
    // Bokeh: far balls around the edges of the frame, never behind the
    // title or the first ball.
    for index in 0..BOKEH {
        let h = |k: u32| hash(index, 300 + k);
        let angle = (index as f32 + h(1) * 0.6) / BOKEH as f32 * std::f32::consts::TAU;
        let reach = lerp(1150.0, 1700.0, h(2));
        let at = [
            960.0 + reach * angle.cos() * 1.65,
            540.0 + reach * angle.sin() * 0.9,
            lerp(1500.0, 3200.0, h(3)),
        ];
        let tone = ["accent", "warning", "accent", "plain"][index as usize % 4];
        elements.push(orb(
            &format!("bokeh-{index}"),
            at,
            lerp(60.0, 150.0, h(4)),
            260,
            tone,
        ));
    }
    for (index, spawn) in spawns.iter().enumerate() {
        elements.push(element(&format!("spawn-{index}"), spawn));
    }
    let plan = stage(
        elements,
        json!({ "bloom": 0.6, "grain": 0.03, "vignette": 0.62, "backdrop": 0.05 }),
    )?;
    let mut stage = StageActor::declare(sc, "stage", &plan)?;
    let s = &mut stage;
    let camera = s.camera();
    let mut later = Later::default();

    for (property, initial) in [
        ("camera.z", -150.0),
        ("camera.dof", 1.2),
        ("camera.focus", 300.0),
        ("post.vignette", 0.72),
        ("ball-1.opacity", 0.0),
        ("ball-1.morph", 0.0),
        ("shock.opacity", 0.0),
        ("ball-2-few.opacity", 0.0),
        ("ball-2.opacity", 0.0),
        ("ball-2.scale", 0.85),
        ("ball-2.rotation", 1.2),
    ] {
        s.channel(sc, property, initial);
    }
    for index in 0..spawns.len() {
        let id = format!("spawn-{index}");
        s.channel(sc, &format!("{id}.opacity"), 0.0);
        s.channel(sc, &format!("{id}.scale"), 0.2);
    }
    // Bokeh: far balls, out of focus, drifting slowly through the dark.
    for index in 0..BOKEH {
        let id = format!("bokeh-{index}");
        let h = |k: u32| hash(index, 400 + k);
        s.channel(sc, &format!("{id}.opacity"), 0.0);
        s.ease(
            sc,
            &format!("{id}.opacity"),
            seconds(0.2 + 0.1 * f64::from(index % 5)),
            0.35,
            2.0,
            Ease::Smoothstep,
        );
        let length = (window.duration as f64 / 1e9) as f32;
        s.ease(
            sc,
            &format!("{id}.x"),
            0,
            (h(1) - 0.5) * 900.0,
            length,
            Ease::Linear,
        );
        s.ease(
            sc,
            &format!("{id}.y"),
            0,
            (h(2) - 0.6) * 500.0,
            length,
            Ease::Linear,
        );
    }

    // ── In the beginning, there were balls. ──
    let whisper = t(film.hush.start);
    title(
        sc,
        "title",
        "In the beginning, there were balls.",
        TITLE_LEFT,
        470.0,
        seconds(0.35),
        whisper,
    )?;
    s.ease(sc, "camera.z", 0, 120.0, 4.2, Ease::Smoothstep);

    // "Oh, I hear you like..." a point of light, then on "balls" it ignites.
    s.ease(
        sc,
        "ball-1.opacity",
        whisper + seconds(0.3),
        1.0,
        1.0,
        Ease::Smoothstep,
    );
    let balls = t(film.hush.at("balls"));
    s.morph(sc, "ball-1", balls, 1, 1.3);
    later.hit("ball-1.pulse", balls, 1.4, 0.0);
    s.set(sc, "shock.opacity", balls, 0.8);
    s.ease(sc, "shock.expand", balls, 1.6, 1.4, Ease::CubicOut);
    s.ease(
        sc,
        "shock.opacity",
        balls + seconds(0.2),
        0.0,
        1.0,
        Ease::Smoothstep,
    );
    // Macro: close and shallow, so its dots are glowing bokeh, swinging slowly.
    s.ease(sc, "camera.z", balls, 720.0, 2.8, Ease::Smootherstep);
    s.ease(sc, "camera.yaw", balls, 0.3, 6.0, Ease::Smoothstep);
    s.ease(sc, "camera.pitch", balls, -0.12, 6.0, Ease::Smoothstep);
    s.ease(sc, "camera.focus", balls, 1200.0, 0.5, Ease::Smoothstep);
    s.ease(sc, "camera.dof", balls, 2.4, 0.5, Ease::Smoothstep);

    // "...with dots." Focus racks onto its dots.
    let dots = t(film.hush.at("dots"));
    let near = camera.pose(sc, dots + seconds(1.0)).depth(SEED.front(dots));
    s.ease(
        sc,
        "camera.focus",
        dots.saturating_sub(seconds(0.15)),
        near,
        0.7,
        Ease::Smootherstep,
    );
    later.hit("ball-1.pulse", dots, 0.7, 0.0);
    s.ease(sc, "post.vignette", dots, 0.6, 2.0, Ease::Smoothstep);

    // "Just one ball with dots on it." Closer, breathing.
    let just = t(film.hush.at("just one ball"));
    s.ease(sc, "camera.z", just, 860.0, 3.0, Ease::Smoothstep);
    camera.handheld(sc, just, 0.5, 2.0);
    later.hit(
        "ball-1.pulse",
        t(film.hush.at_after("dots", "just")),
        0.5,
        0.0,
    );

    // "But what if... there was another ball?" It was behind it all along:
    // the camera eases aside and the blur behind the first ball sharpens.
    let what = t(film.another.at("but what if"));
    s.ease(sc, "camera.z", what, 380.0, 2.6, Ease::Smootherstep);
    s.ease(sc, "camera.x", what, 150.0, 2.6, Ease::Smootherstep);
    s.ease(sc, "camera.yaw", what, 0.0, 2.6, Ease::Smootherstep);
    s.ease(sc, "camera.pitch", what, 0.0, 2.6, Ease::Smootherstep);
    s.ease(sc, "ball-2-few.opacity", what, 1.0, 1.0, Ease::Smoothstep);
    let another = t(film.another.at("another"));
    let far = camera.pose(sc, another + seconds(1.0)).depth(TWO.into());
    s.ease(
        sc,
        "camera.focus",
        another.saturating_sub(seconds(0.1)),
        far,
        0.6,
        Ease::Smootherstep,
    );
    s.ease(sc, "camera.x", another, 330.0, 1.8, Ease::Smootherstep);
    s.ease(sc, "camera.y", another, -50.0, 1.8, Ease::Smootherstep);
    later.hit("ball-2-few.pulse", another, 1.0, 0.0);
    camera.handheld(sc, another, 0.9, 1.0);

    // "...with MORE dots?" The few dots multiply.
    let more = t(film.another.at("more dots"));
    s.ease(sc, "ball-2-few.opacity", more, 0.0, 0.4, Ease::Smoothstep);
    s.ease(sc, "ball-2.opacity", more, 1.0, 0.3, Ease::Smootherstep);
    s.bounce(sc, "ball-2.scale", more, 1.0, 0.5, 0.3);
    s.ease(sc, "camera.z", more, 470.0, 1.4, Ease::Smootherstep);
    later.hit("ball-2.pulse", more + seconds(0.08), 1.2, 0.0);

    // The heart: every thump lights the balls on screen.
    for &beat in &film.heartbeats {
        let at = t(beat);
        later.hit("ball-1.pulse", at, 0.35, 0.0);
        if at > more {
            later.hit("ball-2.pulse", at, 0.35, 0.0);
        }
    }

    // ── The chant: every "balls" is a new shot and a new ball ──
    let beats = film.chant.beats();
    let first = t(beats[0].0);
    s.ease(sc, "camera.dof", first, 0.35, 0.3, Ease::Smoothstep);
    s.ease(sc, "post.vignette", first, 0.4, 1.0, Ease::Smoothstep);
    let mut visible = vec!["ball-1".to_owned(), "ball-2".to_owned()];
    for (index, ((beat, _), spawn)) in beats.iter().zip(&spawns).enumerate() {
        let at = t(*beat);
        let id = format!("spawn-{index}");
        let late = index as f32 / (spawns.len() - 1) as f32;
        s.set(sc, &format!("{id}.opacity"), at, 1.0);
        s.bounce(sc, &format!("{id}.scale"), at, 1.0, 0.36 - 0.12 * late, 0.4);
        if index + 1 < spawns.len() {
            let turn = if index % 2 == 0 { 2.0 } else { -2.0 };
            s.ease(sc, &format!("{id}.rotation"), at, turn, 1.6, Ease::CubicOut);
        } else {
            s.channel(sc, &format!("{id}.spin"), 0.0);
        }
        if matches!(spawn.kind, Kind::Cube | Kind::Torus | Kind::Lattice) {
            s.ease(
                sc,
                &format!("{id}.pitch"),
                at,
                0.6 + 0.3 * index as f32,
                1.4,
                Ease::CubicOut,
            );
        }
        later.hit(&format!("{id}.pulse"), at, 1.2, 0.0);
        visible.push(id.clone());
        // A new composition on every word.
        let shot = SHOTS[index];
        for (property, value) in [
            ("camera.yaw", shot.yaw),
            ("camera.pitch", shot.pitch),
            ("camera.roll", shot.roll),
            ("camera.pivot", spawn.at[2]),
            ("camera.focus", 0.0),
        ] {
            s.set(sc, property, at, value);
        }
        let targets = if shot.close {
            vec![id.as_str()]
        } else {
            visible.iter().map(String::as_str).collect()
        };
        // Framed as the new ball will stand once it has popped to full size.
        let pose = camera.framing(sc, &targets, shot.padding, at + seconds(0.6))?;
        camera.move_to(sc, &pose, at, Move::Cut);
        let next = beats
            .get(index + 1)
            .map_or(at + seconds(0.5), |(b, _)| t(*b));
        let hold = ((next - at) as f64 / 1e9) as f32;
        s.ease(
            sc,
            "camera.z",
            at,
            pose.position.z + shot.push,
            hold,
            Ease::Linear,
        );
        let side = if index % 2 == 0 { 1.0 } else { -1.0 };
        if index + 1 == spawns.len() {
            later.slam(at, [0.0, 1.0], 1.0);
            for other in &visible {
                later.hit(&format!("{other}.pulse"), at, 1.3, 0.0);
            }
        } else {
            later.jolt(at, [side, 0.5], 0.25 + 0.5 * late);
            later.hit("post.chroma", at, 0.08 + 0.2 * late, 0.0);
        }
    }
    let rapid = t(beats[5].0);
    let last = t(beats[beats.len() - 1].0);
    s.ease(
        sc,
        "camera.handheld",
        first,
        1.8,
        ((last - first) as f64 / 1e9) as f32,
        Ease::Linear,
    );
    s.ease(
        sc,
        "camera.quake",
        rapid,
        0.6,
        ((last - rapid) as f64 / 1e9) as f32,
        Ease::Linear,
    );
    s.set(sc, "camera.quake", last, 1.1);

    // ...and the last ball swallows the camera: into one of its dots.
    let end = window.until - window.from;
    let exit = later.dive(last_ball(), last + seconds(0.14), end, DIVE);
    later.run(s, sc);

    // ── Subtitles: a whisper, word by word; then a chant that grows ──
    let whisper = [film.hush.words(), film.another.words()]
        .concat()
        .into_iter()
        .map(|(text, start, end)| (lower(&text), t(start), t(end)))
        .collect::<Vec<_>>();
    let chant = beats
        .iter()
        .enumerate()
        .map(|(index, (start, end))| {
            let text = match index {
                0 | 1 => "balls…",
                2..=4 => "balls,",
                12 => "BALLS!",
                _ => "balls",
            };
            (text.to_owned(), t(*start), t(*end))
        })
        .collect::<Vec<_>>();
    let groups = [
        (
            whisper,
            SubtitlesPlan::new([960.0, 905.0], 1400.0)
                .size(50.0)
                .highlight(Tone::Accent)
                .without_backing()
                .word_by_word(),
        ),
        (
            chant[0..2].to_vec(),
            caption(48.0, Tone::Accent).word_by_word(),
        ),
        (
            chant[2..5].to_vec(),
            caption(60.0, Tone::Warning).word_by_word(),
        ),
        (
            chant[5..].to_vec(),
            caption(84.0, Tone::Error).word_by_word(),
        ),
    ];
    for (index, (words, plan)) in groups.iter().enumerate() {
        let mut group = subtitles(sc, &format!("sub-{index}"), words, plan.clone())?;
        let opacity = group.channel(sc, "opacity", 1.0);
        match groups.get(index + 1) {
            Some((next, _)) => sc.set(&opacity, next[0].1.saturating_sub(seconds(0.16)), 0.0),
            None => sc.ease(&opacity, last + seconds(0.3), 0.0, 0.1, Ease::Linear),
        }
    }
    Ok(Shot {
        plan: window.finish(scene, film)?,
        entry: None,
        exit: Some(exit),
    })
}

/// Where the opening title's measured line starts, to sit centered.
const TITLE_LEFT: f32 = 355.0;
/// How large the dot the camera dives into is when the cut takes it.
pub const DIVE: f32 = 70.0;

/// One chant composition: turned, rolled, and how tight.
#[derive(Clone, Copy)]
struct Composition {
    yaw: f32,
    pitch: f32,
    roll: f32,
    close: bool,
    padding: f32,
    push: f32,
}

const fn shot(
    yaw: f32,
    pitch: f32,
    roll: f32,
    close: bool,
    padding: f32,
    push: f32,
) -> Composition {
    Composition {
        yaw,
        pitch,
        roll,
        close,
        padding,
        push,
    }
}

/// Thirteen words, thirteen shots: wide, close, low, Dutch, top-down...
const SHOTS: [Composition; 13] = [
    shot(0.0, 0.0, 0.0, false, 220.0, 60.0),
    shot(0.3, 0.0, 0.08, true, 380.0, 120.0),
    shot(-0.25, -0.3, -0.06, false, 160.0, 80.0),
    shot(0.15, 0.0, -0.2, true, 300.0, 120.0),
    shot(0.0, 0.75, 0.0, false, 140.0, 90.0),
    shot(-0.4, 0.1, 0.14, true, 420.0, 160.0),
    shot(0.35, -0.2, -0.12, false, 150.0, 60.0),
    shot(0.0, 0.0, 0.22, true, 340.0, 120.0),
    shot(-0.3, 0.45, -0.1, false, 130.0, 60.0),
    shot(0.45, 0.0, 0.16, true, 380.0, 120.0),
    shot(-0.2, -0.35, -0.2, false, 140.0, 60.0),
    shot(0.25, 0.3, 0.12, true, 320.0, 100.0),
    shot(0.0, 0.0, 0.0, false, 110.0, 30.0),
];
