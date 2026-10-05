//! The theory, at speed: balls with dots are atoms (electrons whirl around
//! a nucleus, and the camera dives into one of its dots); balls with
//! thoughts are hopes and dreams (glowing balls rise into the stars);
//! thoughts and dots side by side, then one and the same; basketballs that
//! bounce like basketballs; and the whole conspiracy board in a flash.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, seconds},
    math::{Vec3, easing::Ease, lerp, random::hash},
    stage::StageActor,
    subtitles::SubtitlesPlan,
    tone::Tone,
};
use serde_json::{Value, json};

use crate::{
    Shot,
    kit::{
        Angle, Later, Sphere, Window, caption, jitter, label, orb, stage, subtitles, upper, words,
    },
    sound::Film,
};

/// Exhibit centers on the board: atoms, hopes and dreams, thoughts are
/// dots, basketballs.
const A: [f32; 2] = [960.0, 540.0];
const B: [f32; 2] = [3160.0, 540.0];
const C: [f32; 2] = [3160.0, 1940.0];
const D: [f32; 2] = [960.0, 1940.0];

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

fn exhibit(id: &str, center: [f32; 2], letter: &str) -> Value {
    let mut value = label(
        id,
        at(center, -790.0, -470.0, 0.0),
        34.0,
        &[("EXHIBIT ", "muted"), (letter, "error")],
    );
    value["align"] = json!("left");
    value
}

fn icon(id: &str, at: [f32; 3], size: f32, icon: &str, tone: &str) -> Value {
    json!({ "kind": "icon", "id": id, "at": at, "size": size, "icon": icon, "tone": tone })
}

/// Three orbits as (pitch, roll): flat ellipses whose long axes sit 60° apart.
const ORBITS: [(f32, f32); 3] = [(0.35, 0.0), (0.629, 0.951), (-0.629, -0.951)];
/// Balls pinned to the board.
const PINS: u32 = 24;
fn board_elements() -> Vec<Value> {
    let mut elements = vec![
        // Exhibit A: a ball whose dots fly out into orbits.
        orb("a-ball", at(A, 0.0, -30.0, -60.0), 112.0, 1000, "accent"),
        label(
            "a-word",
            at(A, 0.0, -360.0, 0.0),
            130.0,
            &[("ATOMS", "accent")],
        ),
        exhibit("a-exhibit", A, "A"),
        // Exhibit B: a ball dreaming.
        orb("b-ball", at(B, -470.0, 150.0, 0.0), 115.0, 1000, "accent"),
        orb("b-bub-1", at(B, -310.0, 30.0, 0.0), 14.0, 40, "plain"),
        orb("b-bub-2", at(B, -245.0, -40.0, 0.0), 22.0, 70, "plain"),
        orb("b-bub-3", at(B, -160.0, -110.0, 0.0), 32.0, 110, "plain"),
        icon(
            "b-cloud",
            at(B, 200.0, -110.0, 0.0),
            600.0,
            "cloud",
            "plain",
        ),
        label(
            "b-hopes",
            at(B, 255.0, -112.0, -1.0),
            84.0,
            &[("HOPES", "warning")],
        ),
        label(
            "b-dreams",
            at(B, 255.0, -22.0, -1.0),
            56.0,
            &[("& DREAMS", "warning")],
        ),
        icon(
            "b-sparkle-1",
            at(B, 560.0, -390.0, 0.0),
            80.0,
            "sparkle",
            "warning",
        ),
        icon(
            "b-sparkle-2",
            at(B, -120.0, -330.0, 0.0),
            56.0,
            "sparkle",
            "warning",
        ),
        exhibit("b-exhibit", B, "B"),
        // Exhibit C: thoughts are dots are thoughts, around and around.
        icon(
            "c-thought",
            at(C, -300.0, -60.0, 0.0),
            220.0,
            "cloud",
            "plain",
        ),
        orb("c-dots", at(C, 300.0, -60.0, 0.0), 92.0, 900, "accent"),
        shape(
            "c-loop-1",
            at(C, 0.0, -60.0, 0.0),
            json!({ "arc": { "radius": 300, "start": 0.82, "sweep": 0.36 } }),
            json!({ "stroke": "error", "width": 6, "arrow": "end" }),
        ),
        shape(
            "c-loop-2",
            at(C, 0.0, -60.0, 0.0),
            json!({ "arc": { "radius": 300, "start": 0.32, "sweep": 0.36 } }),
            json!({ "stroke": "error", "width": 6, "arrow": "end" }),
        ),
        label(
            "c-thoughts",
            at(C, -380.0, 190.0, 0.0),
            48.0,
            &[("THOUGHTS", "plain")],
        ),
        label(
            "c-word",
            at(C, 380.0, 190.0, 0.0),
            48.0,
            &[("DOTS", "accent")],
        ),
        label(
            "c-equals",
            at(C, 0.0, -400.0, 0.0),
            110.0,
            &[("= ", "muted"), ("SAME THING", "error")],
        ),
        exhibit("c-exhibit", C, "C"),
        // Exhibit D: a basketball.
        orb("d-ball", at(D, 0.0, 20.0, 0.0), 205.0, 1600, "warning"),
        shape(
            "d-rim",
            at(D, 0.0, 20.0, -2.0),
            json!({ "circle": 207 }),
            json!({ "stroke": "plain", "width": 5 }),
        ),
        shape(
            "d-seam-v",
            at(D, 0.0, 20.0, -2.0),
            json!({ "rect": [7, 414] }),
            json!({ "fill": "plain", "stroke": null }),
        ),
        shape(
            "d-seam-h",
            at(D, 0.0, 20.0, -2.0),
            json!({ "rect": [414, 7] }),
            json!({ "fill": "plain", "stroke": null }),
        ),
        shape(
            "d-seam-l",
            at(D, -300.0, 20.0, -2.0),
            json!({ "arc": { "radius": 240, "start": 0.1, "sweep": 0.3 } }),
            json!({ "stroke": "plain", "width": 5 }),
        ),
        shape(
            "d-seam-r",
            at(D, 300.0, 20.0, -2.0),
            json!({ "arc": { "radius": 240, "start": 0.6, "sweep": 0.3 } }),
            json!({ "stroke": "plain", "width": 5 }),
        ),
        label(
            "d-word",
            at(D, 0.0, -360.0, 0.0),
            120.0,
            &[("BASKETBALLS", "warning")],
        ),
        exhibit("d-exhibit", D, "D"),
        // The red string that ties it all together.
        json!({ "kind": "path", "id": "string", "through": ["a-ball", "b-ball", "c-dots", "d-ball", "a-ball"],
                "tone": "error", "width": 3, "bend": 40 }),
    ];
    // Pins: the board is stuck all over with little balls.
    for index in 0..PINS {
        let h = |salt: u32| hash(index, salt);
        let tone = ["error", "accent", "plain", "request", "warning"][index as usize % 5];
        let at = [
            lerp(300.0, 3800.0, (index as f32 + h(1)) / PINS as f32),
            lerp(150.0, 2350.0, h(2)),
            lerp(-40.0, 120.0, h(3)),
        ];
        elements.push(orb(
            &format!("pin-{index}"),
            at,
            lerp(26.0, 60.0, h(4)),
            160,
            tone,
        ));
    }
    for (index, _) in ORBITS.iter().enumerate() {
        elements.push(json!({
            "kind": "form", "id": format!("a-orbit-{index}"), "at": at(A, 0.0, -30.0, 0.0),
            "tone": "request", "points": 420, "tilt": 0.0,
            "shapes": [{ "shape": "sphere", "radius": 120 },
                       { "shape": "torus", "radius": 300, "tube": 7 }]
        }));
    }
    elements
}

fn post() -> Value {
    json!({ "bloom": 0.65, "grain": 0.04, "vignette": 0.45, "backdrop": 0.1 })
}

/// The rectangle of a ball of `radius` at the center of the frame under the
/// resting camera: where a match cut lands the dot it came from.
fn centered(radius: f32) -> [f32; 4] {
    [960.0 - radius, 540.0 - radius, radius * 2.0, radius * 2.0]
}

/// This shot's spoken words, shouted, shaking on its `hits`.
fn shout(sc: &mut PlanBuilder, film: &Film, window: &Window, hits: &[(u64, f32)]) -> Result<()> {
    shout_in(sc, film, window, hits, caption(56.0, Tone::Warning), None)?;
    Ok(())
}

/// The shouted words on `plan`, for a shot that places its own captions.
fn shout_in(
    sc: &mut PlanBuilder,
    film: &Film,
    window: &Window,
    hits: &[(u64, f32)],
    plan: SubtitlesPlan,
    slide: Option<(u64, f32)>,
) -> Result<()> {
    let said = words(&film.theory, window, upper);
    if said.is_empty() {
        return Ok(());
    }
    let mut loud = subtitles(sc, "sub-theory", &said, plan)?;
    if let Some((at, dx)) = slide {
        let x = loud.channel(sc, "x", 0.0);
        sc.ease(&x, at, dx, 0.3, Ease::Smootherstep);
    }
    // Jitter rides on top of the slide.
    let hits = hits
        .iter()
        .copied()
        .filter(|(at, _)| slide.is_none_or(|(slid, _)| *at > slid + seconds(0.3)))
        .collect::<Vec<_>>();
    jitter(sc, &mut loud, &hits, 1.0);
    Ok(())
}

/// Captions for one half of a split screen.
fn half(x: f32) -> SubtitlesPlan {
    SubtitlesPlan::new([x, 990.0], 860.0)
        .size(44.0)
        .highlight(Tone::Warning)
}

/// A circle of `radius` about `center`, tilted `tilt` out of the frame and
/// turned `turn` in it: an orbit seen from just above its plane.
fn orbit(center: Vec3, radius: f32, turn: f32, tilt: f32, angle: f32) -> Vec3 {
    let u = Vec3::new(turn.cos(), turn.sin(), 0.0);
    let v = Vec3::new(-turn.sin(), turn.cos(), 0.0) * tilt.sin() + Vec3::Z * tilt.cos();
    center + (u * angle.cos() + v * angle.sin()) * radius
}

const NUCLEUS: f32 = 130.0;
const ORBIT: f32 = 340.0;
const TURNS: [f32; 3] = [0.0, 1.047, 2.094];
const TILT: f32 = 0.28;

/// "Balls with dots represent ATOMS." Electrons whirl around a nucleus;
/// then the camera dives into one of the nucleus's dots.
pub fn atom(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("theory-atom");
    let sc = &mut scene;
    let t = |at: u64| window.t(at);
    let center = Vec3::new(960.0, 540.0, 0.0);
    let mut elements = vec![orb("nucleus", center.to_array(), NUCLEUS, 1300, "accent")];
    for (index, turn) in TURNS.iter().enumerate() {
        let points = (0..=24)
            .map(|k| {
                orbit(
                    center,
                    ORBIT,
                    *turn,
                    TILT,
                    k as f32 / 24.0 * std::f32::consts::TAU,
                )
                .to_array()
            })
            .collect::<Vec<_>>();
        elements.push(
            json!({ "kind": "path", "id": format!("orbit-{index}"), "through": points,
            "curve": "smooth", "tone": "request", "width": 2.5 }),
        );
        let start = orbit(center, ORBIT, *turn, TILT, 0.0);
        elements.push(orb(
            &format!("electron-{index}"),
            start.to_array(),
            17.0,
            90,
            "plain",
        ));
    }
    elements.push(label(
        "word",
        [960.0, 150.0, 0.0],
        150.0,
        &[("ATOMS!", "accent")],
    ));
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    s.channel(sc, "nucleus.spin", 0.0);
    s.channel(sc, "word.opacity", 0.0);
    s.channel(sc, "camera.quake", 0.35);
    for index in 0..3 {
        s.channel(sc, &format!("orbit-{index}.draw"), 0.0);
        s.channel(sc, &format!("electron-{index}.opacity"), 0.0);
    }
    // The orbits draw on as it is said, and the electrons start to whirl.
    let represent = t(film.theory.at("represent"));
    let end = window.until - window.from;
    for (index, turn) in TURNS.iter().enumerate() {
        let at = represent + seconds(0.06 * index as f64);
        s.ease(
            sc,
            &format!("orbit-{index}.draw"),
            at,
            1.0,
            0.35,
            Ease::CubicBezier([0.45, 0.0, 0.2, 1.0]),
        );
        let id = format!("electron-{index}");
        s.ease(sc, &format!("{id}.opacity"), at, 1.0, 0.1, Ease::Linear);
        // 1.7 turns a second, in 32 straight steps a turn; the shutter
        // smooths the corners away.
        let start = orbit(center, ORBIT, *turn, TILT, 0.0);
        let step = 1.0 / (1.7 * 32.0);
        let mut time = at;
        let mut k = 0;
        while time < window.duration {
            k += 1;
            let angle = k as f32 / 32.0 * std::f32::consts::TAU + index as f32 * 2.1;
            let point = orbit(center, ORBIT, *turn, TILT, angle) - start;
            for (axis, value) in ["x", "y", "z"].into_iter().zip(point.to_array()) {
                s.ease(sc, &format!("{id}.{axis}"), time, value, step, Ease::Linear);
            }
            time += seconds(f64::from(step));
        }
        later.hit(&format!("orbit-{index}.surge"), at + seconds(0.3), 1.0, 0.0);
    }
    s.ease(sc, "camera.yaw", represent, 0.4, 1.2, Ease::Smoothstep);
    s.ease(
        sc,
        "camera.z",
        0,
        140.0,
        (end as f64 / 1e9) as f32,
        Ease::Smoothstep,
    );
    let atoms = t(film.theory.at("atoms"));
    later.word("word", atoms, None, 0.9);
    later.hit("nucleus.pulse", atoms, 1.2, 0.0);
    s.charge(sc, "nucleus", atoms, 0.8, 0.1);
    s.charge(sc, "nucleus", atoms + seconds(0.3), 0.0, 0.2);
    // Into the nucleus.
    let dive = atoms + seconds(0.3);
    later.at(dive, move |s, sc| {
        s.ease(sc, "word.opacity", dive, 0.0, 0.15, Ease::Linear)
    });
    let sphere = Sphere {
        at: center.to_array(),
        radius: NUCLEUS,
        points: 1300,
        rotation: 0.0,
        spin: 0.0,
    };
    let exit = later.dive(sphere, dive, end, crate::ball::DIVE);
    later.run(s, sc);
    shout(sc, film, &window, &[(atoms, 0.6)])?;
    Ok(Shot {
        plan: window.finish(scene, film)?,
        entry: Some(centered(NUCLEUS)),
        exit: Some(exit),
    })
}

/// "Balls with THOUGHTS represent HOPES and DREAMS." A ball dreams; glowing
/// balls rise past it into the stars.
pub fn dreams(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("theory-dreams");
    let sc = &mut scene;
    let t = |at: u64| window.t(at);
    let ball = [700.0, 700.0, 0.0];
    let icon = |id: &str, at: [f32; 3], size: f32, icon: &str, tone: &str| json!({ "kind": "icon", "id": id, "at": at, "size": size, "icon": icon, "tone": tone });
    let mut elements = vec![
        json!({ "kind": "form", "id": "stars", "at": [960, 300, 1800], "tone": "plain", "points": 1200,
                "tilt": 0.0, "shapes": [{ "shape": "plane", "size": [2000, 1500] }] }),
        orb("ball", ball, NUCLEUS, 1300, "warning"),
        orb("bubble-1", [880.0, 540.0, 0.0], 16.0, 50, "plain"),
        orb("bubble-2", [960.0, 460.0, 0.0], 25.0, 80, "plain"),
        orb("bubble-3", [1060.0, 380.0, 0.0], 36.0, 120, "plain"),
        icon("cloud", [1380.0, 300.0, 0.0], 560.0, "cloud", "plain"),
        label(
            "hopes",
            [1430.0, 290.0, -1.0],
            84.0,
            &[("HOPES", "warning")],
        ),
        label(
            "dreams",
            [1430.0, 370.0, -1.0],
            60.0,
            &[("& DREAMS", "warning")],
        ),
    ];
    for index in 0..10_u32 {
        let h = |k: u32| hash(index, 600 + k);
        let at = [
            lerp(150.0, 1770.0, h(1)),
            lerp(1250.0, 1700.0, h(2)),
            lerp(-200.0, 500.0, h(3)),
        ];
        let tone = ["warning", "accent"][index as usize % 2];
        elements.push(orb(
            &format!("rise-{index}"),
            at,
            lerp(30.0, 70.0, h(4)),
            220,
            tone,
        ));
    }
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    for id in [
        "bubble-1", "bubble-2", "bubble-3", "cloud", "hopes", "dreams",
    ] {
        s.channel(sc, &format!("{id}.opacity"), 0.0);
        s.channel(sc, &format!("{id}.scale"), 0.3);
    }
    s.channel(sc, "stars.opacity", 0.25);
    s.channel(sc, "stars.spin", 0.0);
    s.channel(sc, "ball.spin", 0.4);
    // The match lands the dot on the ball, centered; then the camera opens up.
    s.channel(sc, "camera.x", ball[0] - 960.0);
    s.channel(sc, "camera.y", ball[1] - 540.0);
    let thoughts = t(film.theory.at("thoughts"));
    s.ease(
        sc,
        "camera.x",
        thoughts.saturating_sub(seconds(0.1)),
        0.0,
        0.45,
        Ease::Smootherstep,
    );
    s.ease(
        sc,
        "camera.y",
        thoughts.saturating_sub(seconds(0.1)),
        0.0,
        0.45,
        Ease::Smootherstep,
    );
    s.ease(
        sc,
        "camera.z",
        thoughts.saturating_sub(seconds(0.1)),
        -120.0,
        0.45,
        Ease::Smootherstep,
    );
    for (index, id) in ["bubble-1", "bubble-2", "bubble-3", "cloud"]
        .into_iter()
        .enumerate()
    {
        let at = thoughts + seconds(0.06 * index as f64);
        s.ease(sc, &format!("{id}.opacity"), at, 1.0, 0.08, Ease::Linear);
        s.bounce(sc, &format!("{id}.scale"), at, 1.0, 0.35, 0.4);
    }
    let hopes = t(film.theory.at("hopes"));
    let dreams = t(film.theory.at("dreams"));
    later.word("hopes", hopes, None, 0.5);
    later.word("dreams", dreams, None, 0.6);
    later.hit("ball.pulse", hopes, 1.0, 0.0);
    // Dreams rise: glowing balls float up into the stars, and the camera
    // tilts up to follow them.
    let end = window.duration;
    let rest = ((end - hopes) as f64 / 1e9) as f32;
    for index in 0..10_u32 {
        let id = format!("rise-{index}");
        let at = hopes + seconds(0.05 * f64::from(index));
        s.ease(
            sc,
            &format!("{id}.y"),
            at,
            -1900.0,
            rest + 0.6,
            Ease::CubicBezier([0.4, 0.0, 0.7, 1.0]),
        );
        later.hit(&format!("{id}.pulse"), at, 1.0, 0.3);
    }
    for id in [
        "ball", "bubble-1", "bubble-2", "bubble-3", "cloud", "hopes", "dreams",
    ] {
        s.ease(sc, &format!("{id}.y"), dreams, -160.0, rest, Ease::CubicOut);
    }
    s.ease(sc, "camera.pitch", hopes, -0.32, rest, Ease::Smoothstep);
    s.ease(sc, "camera.y", hopes, -240.0, rest, Ease::Smoothstep);
    s.ease(sc, "stars.opacity", dreams, 0.9, 0.4, Ease::Smoothstep);
    later.hit("post.bloom", dreams, 1.4, 0.65);
    later.run(s, sc);
    shout(sc, film, &window, &[(hopes, 0.4), (dreams, 0.4)])?;
    Ok(Shot {
        plan: window.finish(scene, film)?,
        entry: Some(centered(NUCLEUS)),
        exit: None,
    })
}

/// "Balls with THOUGHTS..." on the left of a split screen.
pub fn thoughts(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("theory-thoughts");
    let sc = &mut scene;
    let icon = |id: &str, at: [f32; 3], size: f32, icon: &str, tone: &str| json!({ "kind": "icon", "id": id, "at": at, "size": size, "icon": icon, "tone": tone });
    let elements = vec![
        orb("ball", [430.0, 560.0, 0.0], 110.0, 1000, "accent"),
        orb("bubble-1", [560.0, 420.0, 0.0], 14.0, 40, "plain"),
        orb("bubble-2", [620.0, 350.0, 0.0], 22.0, 70, "plain"),
        icon("cloud", [560.0, 220.0, 0.0], 330.0, "cloud", "plain"),
        icon("brain", [575.0, 235.0, -2.0], 120.0, "brain", "error"),
        label("word", [480.0, 780.0, 0.0], 76.0, &[("THOUGHTS", "plain")]),
    ];
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    let length = (window.duration as f64 / 1e9) as f32;
    // Centered at first; the frame slides left as the split opens.
    let split = window.until - window.from;
    s.channel(sc, "camera.x", -480.0);
    s.ease(
        sc,
        "camera.x",
        split.saturating_sub(seconds(0.25)),
        0.0,
        0.3,
        Ease::Smootherstep,
    );
    s.ease(sc, "camera.z", 0, 120.0, length, Ease::Linear);
    s.channel(sc, "word.scale", 1.7);
    s.bounce(sc, "word.scale", seconds(0.05), 1.0, 0.3, 0.3);
    later.jolt(seconds(0.05), [1.0, 0.0], 0.6);
    later.hit("brain.flash", seconds(0.1), 1.0, 0.0);
    later.hit("ball.pulse", seconds(0.05), 1.0, 0.0);
    s.set(sc, "camera.quake", 0, 0.4);
    later.run(s, sc);
    // Captions sit in the left half once the split opens.
    let slide = (split.saturating_sub(seconds(0.25)), -480.0);
    shout_in(sc, film, &window, &[], half(960.0), Some(slide))?;
    Ok(window.finish(scene, film)?.into())
}

/// "...represent balls with DOTS." on the right of the split; then the
/// split closes and the two are one.
pub fn dots(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("theory-dots");
    let sc = &mut scene;
    let t = |at: u64| window.t(at);
    let elements = vec![
        orb("ball", [1440.0, 500.0, 0.0], 170.0, 1800, "accent"),
        label("word", [1440.0, 780.0, 0.0], 76.0, &[("DOTS", "accent")]),
        label(
            "equals",
            [1440.0, 170.0, 0.0],
            110.0,
            &[("THOUGHTS ", "plain"), ("= ", "error"), ("DOTS", "accent")],
        ),
    ];
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    s.channel(sc, "equals.opacity", 0.0);
    s.set(sc, "camera.quake", 0, 0.4);
    let dots = t(film.theory.at_after("dots", "dreams"));
    // The split closes on "dots": the ball takes the whole frame.
    s.ease(
        sc,
        "camera.x",
        dots.saturating_sub(seconds(0.12)),
        480.0,
        0.3,
        Ease::Smootherstep,
    );
    s.ease(
        sc,
        "camera.z",
        dots.saturating_sub(seconds(0.12)),
        200.0,
        0.4,
        Ease::Smootherstep,
    );
    s.channel(sc, "word.opacity", 0.0);
    s.ease(
        sc,
        "word.opacity",
        dots.saturating_sub(seconds(0.12)),
        0.0,
        0.15,
        Ease::Linear,
    );
    later.word("equals", dots, None, 0.8);
    later.hit("ball.pulse", dots, 1.3, 0.0);
    s.clock_for(sc, "post.rewind", dots.saturating_sub(seconds(0.05)), 1.4);
    later.hit("post.chroma", dots, 0.6, 0.0);
    s.charge(sc, "ball", dots, 1.0, 0.0);
    s.charge(sc, "ball", dots + seconds(0.4), 0.0, 0.3);
    later.run(s, sc);
    // Captions sit in the right half until the split closes.
    let slide = (dots.saturating_sub(seconds(0.12)), -480.0);
    shout_in(
        sc,
        film,
        &window,
        &[(dots + seconds(0.35), 0.8)],
        half(1440.0),
        Some(slide),
    )?;
    Ok(window.finish(scene, film)?.into())
}

/// Ballistic bounces on a `y` offset: a drop from `height` above the floor
/// landing at `land`, then rebounds of `bounce` each time. Returns the
/// landing times. Quadratic eases are exact free fall.
fn bounce(
    s: &mut StageActor,
    sc: &mut PlanBuilder,
    ids: &[String],
    land: u64,
    height: f32,
    bounce: f32,
) -> Vec<u64> {
    let limit = sc.duration_nanos();
    const GRAVITY: f32 = 5200.0;
    let fall = Ease::CubicBezier([0.11, 0.0, 0.5, 0.0]);
    let rise = Ease::CubicBezier([0.5, 1.0, 0.89, 1.0]);
    let mut drop = (2.0 * height / GRAVITY).sqrt();
    let mut landings = vec![land];
    let start = land.saturating_sub(seconds(f64::from(drop)));
    for id in ids {
        s.channel(sc, &format!("{id}.y"), -height);
        s.set(sc, &format!("{id}.y"), start, -height);
        s.ease(sc, &format!("{id}.y"), start, 0.0, drop, fall);
    }
    let mut at = land;
    let mut apex = height;
    for _ in 0..8 {
        if at + seconds(f64::from(drop * bounce)) >= limit {
            break;
        }
        apex *= bounce * bounce;
        drop *= bounce;
        for id in ids {
            s.ease(sc, &format!("{id}.y"), at, -apex, drop, rise);
            s.ease(
                sc,
                &format!("{id}.y"),
                at + seconds(f64::from(drop)),
                0.0,
                drop,
                fall,
            );
        }
        at += seconds(f64::from(2.0 * drop));
        landings.push(at);
    }
    landings.retain(|landing| *landing < limit);
    landings
}

/// "Balls with dots represent BASKETBALLS." They drop in and bounce.
pub fn basketballs(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("theory-basketballs");
    let sc = &mut scene;
    let t = |at: u64| window.t(at);
    let floor = 900.0;
    let hero = [960.0, floor - 190.0, 0.0];
    let shape = |id: &str, at: [f32; 3], figure: Value, extra: Value| {
        let mut value = json!({ "kind": "shape", "id": id, "at": at, "shape": figure });
        if let (Some(value), Some(extra)) = (value.as_object_mut(), extra.as_object()) {
            value.extend(extra.clone());
        }
        value
    };
    let over = |dx: f32| [hero[0] + dx, hero[1], -2.0];
    let mut elements = vec![
        json!({ "kind": "form", "id": "floor", "at": [960, floor + 10.0, 200], "tone": "muted", "points": 800,
                "tilt": 1.45, "shapes": [{ "shape": "plane", "size": [2000, 900] }] }),
        orb("hero", hero, 190.0, 1600, "warning"),
        shape(
            "rim",
            over(0.0),
            json!({ "circle": 192 }),
            json!({ "stroke": "plain", "width": 5 }),
        ),
        shape(
            "seam-v",
            over(0.0),
            json!({ "rect": [7, 384] }),
            json!({ "fill": "plain", "stroke": null }),
        ),
        shape(
            "seam-h",
            over(0.0),
            json!({ "rect": [384, 7] }),
            json!({ "fill": "plain", "stroke": null }),
        ),
        shape(
            "seam-l",
            over(-280.0),
            json!({ "arc": { "radius": 225, "start": 0.1, "sweep": 0.3 } }),
            json!({ "stroke": "plain", "width": 5 }),
        ),
        shape(
            "seam-r",
            over(280.0),
            json!({ "arc": { "radius": 225, "start": 0.6, "sweep": 0.3 } }),
            json!({ "stroke": "plain", "width": 5 }),
        ),
        label(
            "word",
            [960.0, 170.0, 0.0],
            140.0,
            &[("BASKETBALLS", "warning")],
        ),
    ];
    let others = [
        (330.0, 85.0, 300.0),
        (1600.0, 100.0, 200.0),
        (640.0, 60.0, 700.0),
        (1330.0, 70.0, 600.0),
    ];
    for (index, (x, radius, z)) in others.iter().enumerate() {
        elements.push(orb(
            &format!("ball-{index}"),
            [*x, floor - radius, *z],
            *radius,
            500,
            "warning",
        ));
    }
    let mut stage = StageActor::declare(sc, "stage", &stage(elements, post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    s.channel(sc, "word.opacity", 0.0);
    s.channel(sc, "floor.opacity", 0.5);
    s.channel(sc, "floor.spin", 0.0);
    s.channel(sc, "camera.pitch", -0.12);
    s.channel(sc, "camera.y", 90.0);
    s.ease(
        sc,
        "camera.z",
        0,
        160.0,
        (window.duration as f64 / 1e9) as f32,
        Ease::Linear,
    );
    let word = t(film.theory.at("basketballs"));
    let parts = ["hero", "rim", "seam-v", "seam-h", "seam-l", "seam-r"].map(String::from);
    for (index, landing) in bounce(s, sc, &parts, word, 900.0, 0.55)
        .into_iter()
        .enumerate()
    {
        later.hit("hero.pulse", landing, 1.0 - 0.2 * index as f32, 0.0);
        // The first landing is the word's slam; the next one jolts too.
        if index == 1 {
            later.jolt(landing, [0.0, 1.0], 0.4);
        }
    }
    // The others are already dribbling when the shot opens.
    for (index, land) in [0.12, 0.3, 0.2, 0.42].into_iter().enumerate() {
        let id = vec![format!("ball-{index}")];
        for landing in bounce(s, sc, &id, seconds(land), 520.0, 0.72) {
            later.hit(&format!("ball-{index}.pulse"), landing, 0.8, 0.0);
        }
    }
    later.word("word", word, None, 1.0);
    later.run(s, sc);
    shout(sc, film, &window, &[(word, 0.9)])?;
    Ok(window.finish(scene, film)?.into())
}

/// The conspiracy board, all of it, in a flash: pins, exhibits, and the red
/// string drawing on as the camera rips back.
pub fn board(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("theory-board");
    let sc = &mut scene;
    let mut stage = StageActor::declare(sc, "stage", &stage(board_elements(), post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    let length = (window.duration as f64 / 1e9) as f32;
    for (index, (pitch, roll)) in ORBITS.iter().enumerate() {
        let id = format!("a-orbit-{index}");
        s.channel(sc, &format!("{id}.pitch"), *pitch);
        s.channel(sc, &format!("{id}.roll"), *roll);
        s.channel(sc, &format!("{id}.spin"), 0.0);
        s.channel(sc, &format!("{id}.solid"), 0.0);
        s.channel(sc, &format!("{id}.morph"), 1.0);
    }
    s.channel(sc, "string.draw", 0.0);
    s.ease(
        sc,
        "string.draw",
        0,
        1.0,
        (length * 0.8).max(0.1),
        Ease::Smoothstep,
    );
    s.set(sc, "string.flow", 0, 1.0);
    s.set(sc, "string.emphasis", 0, 1.0);
    later.cut(
        0,
        Angle {
            x: D[0] - 960.0,
            y: D[1] - 540.0,
            z: 300.0,
            roll: -0.05,
            ..Angle::default()
        },
    );
    later.at(0, move |s, sc| {
        for (property, target) in [
            ("camera.x", 1100.0),
            ("camera.y", 700.0),
            ("camera.z", -1950.0),
            ("camera.roll", 0.05),
        ] {
            s.ease(sc, property, 0, target, length.min(0.4), Ease::CubicOut);
        }
        s.ease(sc, "post.zoom", 0, 0.2, 0.1, Ease::Linear);
        s.ease(sc, "post.zoom", seconds(0.1), 0.0, 0.25, Ease::Linear);
    });
    for index in 0..PINS {
        later.hit(
            &format!("pin-{index}.pulse"),
            seconds(0.015 * f64::from(index)),
            1.0,
            0.0,
        );
    }
    s.set(sc, "camera.quake", 0, 1.2);
    s.ease(sc, "camera.quake", 0, 1.7, length, Ease::Linear);
    later.run(s, sc);
    shout(sc, film, &window, &[])?;
    Ok(window.finish(scene, film)?.into())
}
