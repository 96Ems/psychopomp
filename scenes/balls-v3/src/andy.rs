//! ANDY SERKIS: the balls slam onto a motion-capture suit under a vertigo
//! dolly zoom, the camera circles the calibration pose, a loupe shows the
//! markers are balls with dots, and they become anything: a chimp, a little
//! shriveled guy. Then lasagna dropped from above, spaghetti, a barrel roll
//! through lightning on "WOO", and a spaghetti kid, speared by a giant fork
//! and carried off to be eaten by WILL SMITH. Names are type only.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, seconds},
    callout::{CalloutAnchorPlan, CalloutSide},
    face::Face,
    lens::{LensActor, LensPlan},
    math::{easing::Ease, lerp, random::hash},
    stage::{Move, StageActor},
    subtitles::SubtitlesPlan,
    tone::Tone,
};
use serde_json::{Value, json};

use crate::{
    Shot,
    kit::{Angle, Later, Window, drop, ghost, jitter, label, orb, stage, subtitles, typed, words},
    sound::Film,
};

pub const MARKERS: [&str; 15] = [
    "head",
    "neck",
    "pelvis",
    "l-shoulder",
    "l-elbow",
    "l-hand",
    "r-shoulder",
    "r-elbow",
    "r-hand",
    "l-hip",
    "l-knee",
    "l-foot",
    "r-hip",
    "r-knee",
    "r-foot",
];

/// Every marker's position in a pose, in the order of `MARKERS`.
pub type Pose = [[f32; 2]; 15];

/// The suit, standing: the home every pose is an offset from.
pub const SUIT: Pose = [
    [960.0, 285.0],
    [960.0, 375.0],
    [960.0, 600.0],
    [875.0, 395.0],
    [790.0, 485.0],
    [730.0, 575.0],
    [1045.0, 395.0],
    [1130.0, 485.0],
    [1190.0, 575.0],
    [905.0, 612.0],
    [890.0, 740.0],
    [880.0, 865.0],
    [1015.0, 612.0],
    [1030.0, 740.0],
    [1040.0, 865.0],
];

/// Arms out: the calibration pose.
const T_POSE: Pose = [
    [960.0, 285.0],
    [960.0, 375.0],
    [960.0, 600.0],
    [875.0, 390.0],
    [760.0, 385.0],
    [645.0, 380.0],
    [1045.0, 390.0],
    [1160.0, 385.0],
    [1275.0, 380.0],
    [905.0, 612.0],
    [880.0, 740.0],
    [860.0, 865.0],
    [1015.0, 612.0],
    [1040.0, 740.0],
    [1060.0, 865.0],
];

/// On all fours, knuckles down, facing right: shoulders above the hips.
pub const CHIMP: Pose = [
    [1190.0, 545.0],
    [1110.0, 515.0],
    [860.0, 590.0],
    [1085.0, 528.0],
    [1085.0, 690.0],
    [1072.0, 858.0],
    [1132.0, 522.0],
    [1142.0, 684.0],
    [1150.0, 852.0],
    [838.0, 600.0],
    [925.0, 722.0],
    [855.0, 860.0],
    [880.0, 596.0],
    [962.0, 712.0],
    [895.0, 860.0],
];

/// Small, hunched, squatting, clutching something to his chest.
pub const SHRIVELED: Pose = [
    [1040.0, 602.0],
    [990.0, 578.0],
    [925.0, 715.0],
    [972.0, 592.0],
    [992.0, 672.0],
    [1036.0, 640.0],
    [1004.0, 590.0],
    [1030.0, 670.0],
    [1060.0, 642.0],
    [908.0, 724.0],
    [990.0, 758.0],
    [932.0, 860.0],
    [946.0, 722.0],
    [1022.0, 752.0],
    [966.0, 860.0],
];

/// A child cheering, arms up.
pub const KID: Pose = [
    [960.0, 470.0],
    [960.0, 555.0],
    [960.0, 690.0],
    [905.0, 570.0],
    [850.0, 520.0],
    [820.0, 455.0],
    [1015.0, 570.0],
    [1070.0, 520.0],
    [1100.0, 455.0],
    [925.0, 700.0],
    [910.0, 780.0],
    [895.0, 860.0],
    [995.0, 700.0],
    [1010.0, 780.0],
    [1025.0, 860.0],
];

/// Three layers of five: the filling between the pasta.
fn lasagna() -> Pose {
    std::array::from_fn(|index| {
        let (row, column) = (index / 5, index % 5);
        [700.0 + 130.0 * column as f32, 505.0 + 105.0 * row as f32]
    })
}

/// A heap of meatballs on the noodles.
pub fn meatballs() -> Pose {
    std::array::from_fn(|index| {
        let angle = index as f32 * 2.399_963;
        let radius = 38.0 * (index as f32).sqrt();
        [
            960.0 + radius * angle.cos() * 1.6,
            600.0 + radius * angle.sin() * 0.7,
        ]
    })
}

/// Thrown everywhere.
pub fn scattered(salt: u32, reach: f32) -> Pose {
    std::array::from_fn(|index| {
        let angle = hash(index as u32, salt) * std::f32::consts::TAU;
        let radius = reach * (0.55 + 0.45 * hash(index as u32, salt + 1));
        [
            960.0 + radius * angle.cos(),
            560.0 + radius * angle.sin() * 0.62,
        ]
    })
}

const NOODLES: usize = 5;
const SLABS: [(&str, f32); 4] = [
    ("warning", 455.0),
    ("error", 560.0),
    ("warning", 665.0),
    ("warning", 770.0),
];
/// Balls in the dark behind the studio: they arrive in waves that double.
const CROWD: usize = 24;

/// Which half of the rant a stage is built for.
#[derive(Clone, Copy, PartialEq)]
enum Part {
    Andy,
    Spaghetti,
}

impl Part {
    fn words(self) -> &'static [(&'static str, &'static str, f32, &'static str, f32, Face)] {
        // id, text, size, tone, height, face
        match self {
            Self::Andy => &[
                ("w-andy", "ANDY SERKIS!", 160.0, "plain", 300.0, Face::Shout),
                (
                    "w-anything",
                    "ANYTHING!",
                    160.0,
                    "accent",
                    170.0,
                    Face::Shout,
                ),
                ("w-chimp", "A CHIMP!", 160.0, "plain", 170.0, Face::Shout),
                (
                    "w-guy",
                    "a little shriveled guy",
                    30.0,
                    "plain",
                    545.0,
                    Face::SerifItalic,
                ),
            ],
            Self::Spaghetti => &[
                (
                    "w-lasagna",
                    "LASAGNA!",
                    160.0,
                    "warning",
                    170.0,
                    Face::Shout,
                ),
                (
                    "w-spaghetti",
                    "SPAGHETTI!",
                    160.0,
                    "warning",
                    170.0,
                    Face::Shout,
                ),
                ("w-woo", "WOO!", 160.0, "error", 330.0, Face::Shout),
                (
                    "w-kid",
                    "SPAGHETTI KID!",
                    150.0,
                    "warning",
                    170.0,
                    Face::Shout,
                ),
                ("w-smith", "WILL SMITH!", 160.0, "error", 330.0, Face::Shout),
            ],
        }
    }

    fn crowd(self) -> usize {
        match self {
            Self::Andy => 20,
            Self::Spaghetti => CROWD,
        }
    }
}

fn bones(prefix: &str, tone: &str, width: f32, bend: f32) -> Vec<Value> {
    let path = |id: &str, through: &[&str]| {
        json!({ "kind": "path", "id": format!("{prefix}-{id}"), "through": through,
                "tone": tone, "width": width, "bend": bend })
    };
    vec![
        path("spine", &["head", "neck", "pelvis"]),
        path(
            "arms",
            &[
                "l-hand",
                "l-elbow",
                "l-shoulder",
                "neck",
                "r-shoulder",
                "r-elbow",
                "r-hand",
            ],
        ),
        path(
            "legs",
            &[
                "l-foot", "l-knee", "l-hip", "pelvis", "r-hip", "r-knee", "r-foot",
            ],
        ),
    ]
}

/// A tangle of noodle `index`: a smooth loop-de-loop across the plate.
pub fn noodle(index: usize) -> Value {
    let points = (0..14)
        .map(|k| {
            let u = k as f32 / 13.0;
            let phase = index as f32 * 1.3;
            let x = 960.0 + (u - 0.5) * 760.0 + 90.0 * (u * 17.0 + phase).sin();
            let y = 640.0 + 70.0 * (u * 11.0 + phase * 2.0).cos() + 18.0 * index as f32 - 36.0;
            json!([x, y, -20.0 - 4.0 * index as f32])
        })
        .collect::<Vec<_>>();
    json!({ "kind": "path", "id": format!("noodle-{index}"), "through": points, "curve": "smooth",
            "tone": "warning", "width": 5 })
}

pub fn fork() -> Value {
    let points = [
        [-30, -900],
        [30, -900],
        [30, -230],
        [125, -120],
        [125, 190],
        [88, 190],
        [88, -30],
        [58, -30],
        [58, 190],
        [20, 190],
        [20, -30],
        [-20, -30],
        [-20, 190],
        [-58, 190],
        [-58, -30],
        [-88, -30],
        [-88, 190],
        [-125, 190],
        [-125, -120],
        [-30, -230],
    ];
    json!({ "kind": "shape", "id": "fork", "at": [960, 300, 40], "shape": { "polygon": points },
            "corner": 6, "fill": "muted", "fillOpacity": 1, "stroke": "plain", "width": 2.5 })
}

/// One of the balls in the dark, far behind the figure.
pub fn crowd(index: usize) -> Value {
    let h = |salt: u32| hash(index as u32, salt);
    // Either side of the figure, never in front of it.
    let x = if index.is_multiple_of(2) {
        lerp(-600.0, 520.0, h(1))
    } else {
        lerp(1400.0, 2520.0, h(1))
    };
    let at = [x, lerp(-200.0, 1100.0, h(2)), lerp(500.0, 1500.0, h(3))];
    let tone = ["accent", "plain", "request", "error", "warning"][index % 5];
    let size = lerp(60.0, 150.0, h(4));
    let id = format!("crowd-{index}");
    match index % 4 {
        1 => json!({ "kind": "form", "id": id, "at": at, "tone": tone, "points": 400,
            "shapes": [{ "shape": "box", "size": [size * 1.4, size * 1.4, size * 1.4] }] }),
        3 => json!({ "kind": "form", "id": id, "at": at, "tone": tone, "points": 400,
            "shapes": [{ "shape": "torus", "radius": size, "tube": size * 0.3 }] }),
        _ => orb(&id, at, size, 400, tone),
    }
}

/// The marker set as JSON stage elements, with the floor and the crowd.
pub fn figure() -> Vec<Value> {
    let mut elements = vec![json!({
        "kind": "form", "id": "floor", "at": [960, 905, 120], "tone": "muted", "points": 800,
        "tilt": 1.35, "shapes": [{ "shape": "plane", "size": [2000, 900] }] })];
    for (id, [x, y]) in MARKERS.iter().zip(SUIT) {
        elements.push(orb(id, [x, y, 0.0], 21.0, 90, "plain"));
    }
    elements.extend(bones("bone", "muted", 2.5, 0.0));
    elements
}

fn elements(part: Part) -> Vec<Value> {
    let mut elements = figure();
    if part == Part::Spaghetti {
        for (index, (tone, y)) in SLABS.iter().enumerate() {
            elements.push(json!({
                "kind": "form", "id": format!("slab-{index}"), "at": [960, y, 0], "tone": tone,
                "points": 520,
                "shapes": [{ "shape": "box", "size": [780, 26, 380], "edges": 0.4 },
                           { "shape": "torus", "radius": 330.0 + 32.0 * index as f32, "tube": 6 }]
            }));
        }
        for index in 0..NOODLES {
            elements.push(noodle(index));
        }
        for (id, dx) in [("eye-l", -19.0), ("eye-r", 19.0)] {
            elements.push(orb(id, [960.0 + dx, 466.0, -40.0], 14.0, 60, "request"));
        }
        elements.extend(bones("noodle-bone", "warning", 7.0, 26.0));
        elements.push(fork());
        for (index, (from, to)) in [
            ("head", 20),
            ("l-hand", 21),
            ("r-hand", 22),
            ("pelvis", 23),
            ("l-foot", 1),
            ("r-foot", 3),
        ]
        .into_iter()
        .enumerate()
        {
            elements.push(
                json!({ "kind": "bolt", "id": format!("woo-{index}"), "from": from,
                "to": format!("crowd-{to}"), "strikes": 4, "branching": 1.2,
                "tone": if index % 2 == 0 { "request" } else { "error" } }),
            );
        }
    }
    for (id, text, size, tone, y, face) in part.words() {
        // The little shriveled guy's caption sits small beside his head.
        let x = if *id == "w-guy" { 1200.0 } else { 960.0 };
        elements.push(typed(id, [x, *y, -60.0], *size, *face, &[(text, tone)]));
    }
    if part == Part::Andy {
        let mut rec = label(
            "rec",
            [70.0, 70.0, 0.0],
            28.0,
            &[
                ("● REC  ", "error"),
                ("MOCAP · TAKE 1 · 15 MARKERS", "muted"),
            ],
        );
        rec["align"] = json!("left");
        elements.push(rec);
        // The markers arrive from everywhere along these.
        for (index, id) in ["l-hand", "r-hand", "head"].into_iter().enumerate() {
            elements.push(
                json!({ "kind": "bolt", "id": format!("spark-{index}"), "from": id,
                "to": format!("crowd-{}", index * 2), "strikes": 3, "tone": "request" }),
            );
        }
    }
    for index in 0..part.crowd() {
        elements.push(crowd(index));
    }
    elements
}

/// Spring every marker to `pose` over `seconds`, `stagger` apart.
fn pose(
    s: &mut StageActor,
    sc: &mut PlanBuilder,
    at: u64,
    pose: &Pose,
    seconds_: f32,
    stagger: f64,
) {
    for (index, (id, [x, y])) in MARKERS.iter().zip(pose).enumerate() {
        let home = SUIT[index];
        let at = at + seconds(stagger * index as f64);
        s.bounce(sc, &format!("{id}.x"), at, x - home[0], seconds_, 0.22);
        s.bounce(sc, &format!("{id}.y"), at, y - home[1], seconds_, 0.22);
    }
}

/// Start every marker in `pose`.
pub fn hold(s: &mut StageActor, sc: &mut PlanBuilder, pose: &Pose) {
    for (index, (id, [x, y])) in MARKERS.iter().zip(pose).enumerate() {
        let home = SUIT[index];
        s.channel(sc, &format!("{id}.x"), x - home[0]);
        s.channel(sc, &format!("{id}.y"), y - home[1]);
    }
}

fn fade(s: &mut StageActor, sc: &mut PlanBuilder, ids: &[String], at: u64, to: f32, seconds_: f32) {
    for id in ids {
        s.ease(
            sc,
            &format!("{id}.opacity"),
            at,
            to,
            seconds_,
            Ease::Smoothstep,
        );
    }
}

/// Bring crowd balls `range` out of the dark at `at`: they fly in from
/// farther back and pulse as they land.
fn arrive(
    s: &mut StageActor,
    later: &mut Later,
    sc: &mut PlanBuilder,
    range: std::ops::Range<usize>,
    at: u64,
) {
    for (order, index) in range.enumerate() {
        let id = format!("crowd-{index}");
        let at = at + seconds(0.03 * order as f64);
        s.ease(sc, &format!("{id}.opacity"), at, 1.0, 0.1, Ease::Linear);
        s.ease(sc, &format!("{id}.z"), at, 0.0, 0.35, Ease::CubicOut);
        s.ease(sc, &format!("{id}.rotation"), at, 2.5, 1.2, Ease::CubicOut);
        later.hit(&format!("{id}.pulse"), at + seconds(0.3), 0.8, 0.0);
    }
}

fn hide_crowd(s: &mut StageActor, sc: &mut PlanBuilder, from: usize, count: usize) {
    for index in from..count {
        s.channel(sc, &format!("crowd-{index}.opacity"), 0.0);
        s.channel(sc, &format!("crowd-{index}.z"), 1500.0);
    }
}

fn post() -> Value {
    json!({ "bloom": 0.6, "grain": 0.04, "vignette": 0.5, "backdrop": 0.1 })
}

/// A crash zoom onto a shouted word at `word_y`: the camera lunges in and
/// toward it, then settles back to `rest`.
fn punch(later: &mut Later, at: u64, word_y: f32, rest: Angle, lunge: f32, roll: f32) {
    let close = Angle {
        y: rest.y + (word_y - 540.0 - rest.y) * 0.6,
        z: rest.z + lunge,
        roll: rest.roll + roll,
        ..rest
    };
    later.crash(at, close, rest, 0.45);
}

fn shout_caption(size: f32, tone: Tone) -> SubtitlesPlan {
    SubtitlesPlan::new([960.0, 985.0 - (size - 40.0) * 0.6], 1500.0)
        .size(size)
        .face(Face::Shout)
        .highlight(tone)
}

fn front(z: f32) -> Angle {
    Angle {
        z,
        ..Angle::default()
    }
}

/// "ANDY SERKIS! If you cover Andy Serkis in balls with dots, you can make
/// anything! A chimp! A little shriveled guy!"
pub fn andy(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("andy-serkis");
    let sc = &mut scene;
    let t = |at: u64| window.t(at);
    let said = &film.andy;
    let part = Part::Andy;
    let mut stage = StageActor::declare(sc, "stage", &stage(elements(part), post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    let bones = ["spine", "arms", "legs"]
        .map(|id| format!("bone-{id}"))
        .to_vec();
    for (id, ..) in part.words() {
        s.channel(sc, &format!("{id}.opacity"), 0.0);
    }
    for index in 0..3 {
        s.channel(sc, &format!("spark-{index}.opacity"), 1.0);
    }
    for id in ["rec", "floor"] {
        s.channel(sc, &format!("{id}.opacity"), 0.0);
    }
    for id in &bones {
        s.channel(sc, &format!("{id}.draw"), 0.0);
        s.channel(sc, &format!("{id}.opacity"), 1.0);
    }
    s.channel(sc, "floor.spin", 0.0);
    hide_crowd(s, sc, 0, part.crowd());
    // The markers start thrown wide, out of frame and close to the lens.
    hold(s, sc, &scattered(5, 1500.0));
    for id in MARKERS {
        s.channel(sc, &format!("{id}.z"), -500.0);
    }
    s.channel(sc, "camera.z", 120.0);

    // ── ANDY SERKIS! The balls slam onto the suit. ──
    let slam = t(said.voice_start());
    pose(s, sc, slam, &SUIT, 0.3, 0.01);
    for id in MARKERS {
        s.bounce(sc, &format!("{id}.z"), slam, 0.0, 0.3, 0.2);
        later.hit(&format!("{id}.pulse"), slam + seconds(0.22), 1.0, 0.0);
    }
    for index in 0..3 {
        s.zap(
            sc,
            &format!("spark-{index}"),
            slam + seconds(0.18 + 0.06 * index as f64),
        );
    }
    later.word("w-andy", slam, Some(t(said.at("if you cover"))), 1.0);
    s.set(sc, "camera.quake", slam, 1.3);
    s.ease(sc, "camera.quake", slam, 0.35, 1.2, Ease::CubicOut);
    s.channel(sc, "camera.zoom", 1.0);
    // Vertigo: the room stretches away while the suit holds still.
    s.camera()
        .dolly_zoom(sc, "pelvis", slam, -700.0, Move::Ease(1.0, Ease::CubicOut))?;
    for id in &bones {
        s.ease(
            sc,
            &format!("{id}.draw"),
            slam + seconds(0.25),
            1.0,
            0.35,
            Ease::Smoothstep,
        );
    }
    s.ease(sc, "floor.opacity", slam, 0.55, 0.4, Ease::Smoothstep);
    s.set(sc, "rec.opacity", slam + seconds(0.3), 1.0);

    // "If you cover Andy Serkis": the calibration pose, and the camera
    // circles it from one side to the other.
    let cover = t(said.at("if you cover"));
    pose(s, sc, cover, &T_POSE, 0.45, 0.0);
    later.cut(
        cover,
        Angle {
            yaw: 0.7,
            z: 80.0,
            pitch: 0.12,
            ..Angle::default()
        },
    );
    later.at(cover, move |s, sc| s.set(sc, "camera.zoom", cover, 1.0));
    let serkis = t(said.at_after("serkis", "cover"));
    let dots = t(said.at("balls with dots"));
    later.glide(
        cover,
        Angle {
            yaw: -0.55,
            z: 160.0,
            pitch: -0.2,
            roll: 0.05,
            ..Angle::default()
        },
        ((dots - cover) as f64 / 1e9) as f32,
    );
    // "...in balls with dots": a loupe finds the dots on one ball.
    later.cut(dots, front(160.0));
    for (index, id) in MARKERS.iter().enumerate() {
        later.hit(
            &format!("{id}.pulse"),
            dots + seconds(0.02 * index as f64),
            1.1,
            0.0,
        );
    }
    let anything = t(said.at("anything"));
    let mut loupe = LensActor::declare(
        sc,
        "loupe",
        &LensPlan::circle(
            CalloutAnchorPlan::Stage {
                id: "hand".into(),
                element: "l-hand".into(),
                edge: CalloutSide::Center,
                side: None,
            },
            380.0,
        )
        .magnification(3.4),
    )?;
    loupe.show(sc, dots);
    loupe.hide(sc, anything.saturating_sub(seconds(0.2)));

    // "...you can make ANYTHING!" The markers fly apart; the dark fills.
    pose(s, sc, anything, &scattered(11, 760.0), 0.4, 0.0);
    fade(s, sc, &bones, anything, 0.0, 0.12);
    let chimp = t(said.at("a chimp"));
    later.word("w-anything", anything, Some(chimp), 0.9);
    later.crash(anything, front(-260.0), front(-80.0), 0.5);
    arrive(s, &mut later, sc, 0..2, anything);

    // "A CHIMP!"
    pose(s, sc, chimp, &CHIMP, 0.28, 0.006);
    fade(s, sc, &bones, chimp + seconds(0.12), 1.0, 0.12);
    let guy = t(said.at("a little shriveled guy"));
    later.word("w-chimp", chimp, Some(guy), 0.8);
    later.cut(
        chimp,
        Angle {
            yaw: -0.5,
            z: 80.0,
            roll: -0.05,
            ..Angle::default()
        },
    );
    arrive(s, &mut later, sc, 2..8, chimp);

    // "A little shriveled guy!"
    pose(s, sc, guy, &SHRIVELED, 0.32, 0.006);
    // Said small: the camera leans in on a tiny caption beside him.
    later.at(guy, move |s, sc| {
        s.set(sc, "w-guy.opacity", guy, 1.0);
    });
    later.glide(
        guy,
        Angle {
            x: 30.0,
            y: 160.0,
            z: 700.0,
            roll: -0.03,
            ..Angle::default()
        },
        0.55,
    );
    arrive(s, &mut later, sc, 8..20, guy);
    s.ease(sc, "camera.quake", guy, 0.7, 0.4, Ease::Linear);
    later.run(s, sc);

    let said_words = words(said, &window, str::to_uppercase);
    let plan = shout_caption(62.0, Tone::Error);
    let mut shout = subtitles(sc, "sub-andy", &said_words, plan.clone())?;
    let hits = [
        (slam + seconds(0.2), 1.0),
        (cover, 0.4),
        (serkis, 0.4),
        (anything, 0.8),
        (chimp, 0.7),
        (guy, 0.6),
    ];
    jitter(sc, &mut shout, &hits, 1.0);
    ghost(
        sc,
        "sub-andy-ghost",
        &said_words,
        &plan,
        Tone::Request,
        &hits,
        anything,
    )?;
    Ok(window.finish(scene, film)?.into())
}

/// "He could become lasagna! He could become spaghetti! Woo! Andy Serkis
/// is a spaghetti kid! And he's going to be eaten by Will Smith!"
pub fn spaghetti(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("spaghetti");
    let sc = &mut scene;
    let t = |at: u64| window.t(at);
    let said = &film.spaghetti;
    let part = Part::Spaghetti;
    let mut stage = StageActor::declare(sc, "stage", &stage(elements(part), post())?)?;
    let s = &mut stage;
    let mut later = Later::default();
    let bones = ["spine", "arms", "legs"]
        .map(|id| format!("bone-{id}"))
        .to_vec();
    let noodle_bones = ["spine", "arms", "legs"]
        .map(|id| format!("noodle-bone-{id}"))
        .to_vec();
    let slabs = (0..SLABS.len())
        .map(|i| format!("slab-{i}"))
        .collect::<Vec<_>>();
    let noodles = (0..NOODLES)
        .map(|i| format!("noodle-{i}"))
        .collect::<Vec<_>>();
    let hidden = [
        &noodle_bones[..],
        &slabs[..],
        &noodles[..],
        &["eye-l".into(), "eye-r".into()],
        &part
            .words()
            .iter()
            .map(|(id, ..)| id.to_string())
            .collect::<Vec<_>>()[..],
    ]
    .concat();
    for id in hidden {
        s.channel(sc, &format!("{id}.opacity"), 0.0);
    }
    for slab in &slabs {
        s.channel(sc, &format!("{slab}.scale"), 1.0);
    }
    for id in &noodles {
        s.channel(sc, &format!("{id}.draw"), 0.0);
    }
    s.channel(sc, "floor.opacity", 0.55);
    s.channel(sc, "floor.spin", 0.0);
    s.channel(sc, "fork.y", -1300.0);
    s.channel(sc, "camera.quake", 0.5);
    // The little shriveled guy is where we left him, and the dark is full.
    hold(s, sc, &SHRIVELED);
    hide_crowd(s, sc, 20, part.crowd());

    // "He could become LASAGNA!" Sheets of pasta drop from above, one on
    // another, onto layers of filling.
    let lasagna_at = t(said.at("lasagna"));
    pose(s, sc, lasagna_at, &lasagna(), 0.3, 0.008);
    for id in &bones {
        s.channel(sc, &format!("{id}.opacity"), 0.0);
    }
    fade(s, sc, &bones, lasagna_at, 0.0, 0.1);
    for (index, slab) in slabs.iter().enumerate().rev() {
        let land = lasagna_at + seconds(0.06 * (SLABS.len() - 1 - index) as f64);
        s.set(
            sc,
            &format!("{slab}.opacity"),
            land.saturating_sub(seconds(0.2)),
            1.0,
        );
        for contact in drop(s, sc, slab, land, 700.0, 0.25, 1) {
            later.hit(&format!("{slab}.pulse"), contact, 0.8, 0.0);
        }
    }
    let again = t(said.at_after("he could become", "lasagna"));
    later.word("w-lasagna", lasagna_at, Some(again), 0.8);
    later.cut(
        lasagna_at,
        Angle {
            y: -40.0,
            z: 120.0,
            pitch: 0.55,
            yaw: 0.2,
            ..Angle::default()
        },
    );
    later.glide(
        lasagna_at,
        Angle {
            y: -40.0,
            z: 220.0,
            pitch: 0.4,
            yaw: -0.15,
            ..Angle::default()
        },
        0.7,
    );
    arrive(s, &mut later, sc, 20..22, lasagna_at);
    later.cut(again, front(-60.0));

    // "He could become SPAGHETTI!" The pasta curls into noodles and the
    // filling rolls into meatballs.
    let spaghetti_at = t(said.at("spaghetti"));
    for (index, slab) in slabs.iter().enumerate() {
        s.morph(
            sc,
            slab,
            spaghetti_at + seconds(0.03 * index as f64),
            1,
            0.4,
        );
        let (_, y) = SLABS[index];
        s.ease(
            sc,
            &format!("{slab}.y"),
            spaghetti_at,
            640.0 - y,
            0.4,
            Ease::Smootherstep,
        );
    }
    for (index, noodle) in noodles.iter().enumerate() {
        let at = spaghetti_at + seconds(0.05 * index as f64);
        s.set(sc, &format!("{noodle}.opacity"), at, 1.0);
        s.ease(
            sc,
            &format!("{noodle}.draw"),
            at,
            1.0,
            0.4,
            Ease::Smoothstep,
        );
    }
    pose(s, sc, spaghetti_at, &meatballs(), 0.35, 0.008);
    for id in MARKERS {
        s.ease(
            sc,
            &format!("{id}.hurt"),
            spaghetti_at,
            1.0,
            0.25,
            Ease::Smoothstep,
        );
    }
    let woo = t(said.at("woo"));
    later.word("w-spaghetti", spaghetti_at, Some(woo), 0.8);
    punch(&mut later, spaghetti_at, 150.0, front(60.0), 260.0, -0.06);
    arrive(s, &mut later, sc, 22..24, spaghetti_at);

    // "WOO!" A barrel roll through lightning; the dark bursts.
    let kid_at = t(said.at_after("andy serkis", "woo"));
    later.word("w-woo", woo, Some(kid_at), 1.0);
    later.cut(
        woo,
        Angle {
            yaw: 0.35,
            z: -80.0,
            roll: 0.0,
            ..Angle::default()
        },
    );
    later.at(woo, move |s, sc| {
        s.ease(
            sc,
            "camera.roll",
            woo,
            std::f32::consts::TAU,
            0.62,
            Ease::Smootherstep,
        );
        s.ease(sc, "camera.z", woo, 120.0, 0.62, Ease::Smootherstep);
        s.ease(sc, "camera.yaw", woo, -0.2, 0.62, Ease::Smootherstep);
    });
    for index in 0..6 {
        let contact = s.zap(
            sc,
            &format!("woo-{index}"),
            woo + seconds(0.05 * index as f64),
        );
        later.hit("post.chroma", contact, 0.4, 0.0);
    }
    for noodle in &noodles {
        later.hit(&format!("{noodle}.surge"), woo, 1.0, 0.0);
        s.to(sc, &format!("{noodle}.flow"), woo, 1.0, 0.3);
    }
    for index in [0, 5, 7] {
        s.clock_for(sc, &format!("crowd-{index}.burst"), woo, 5.2);
    }
    s.ease(sc, "camera.quake", woo, 1.4, 0.1, Ease::Linear);
    s.ease(
        sc,
        "camera.quake",
        woo + seconds(0.1),
        0.6,
        0.6,
        Ease::CubicOut,
    );

    // "Andy Serkis is a SPAGHETTI KID!" The noodles become the kid.
    pose(s, sc, kid_at, &KID, 0.35, 0.008);
    later.cut(
        kid_at,
        Angle {
            roll: std::f32::consts::TAU,
            ..front(0.0)
        },
    );
    for id in MARKERS {
        s.ease(
            sc,
            &format!("{id}.hurt"),
            kid_at,
            0.0,
            0.3,
            Ease::Smoothstep,
        );
    }
    for slab in &slabs {
        s.ease(
            sc,
            &format!("{slab}.opacity"),
            kid_at,
            0.0,
            0.25,
            Ease::Smoothstep,
        );
        s.ease(
            sc,
            &format!("{slab}.scale"),
            kid_at,
            0.2,
            0.3,
            Ease::Smoothstep,
        );
    }
    for noodle in &noodles {
        s.ease(
            sc,
            &format!("{noodle}.trim"),
            kid_at,
            1.0,
            0.4,
            Ease::Smoothstep,
        );
    }
    for id in &noodle_bones {
        s.channel(sc, &format!("{id}.draw"), 0.0);
        s.set(sc, &format!("{id}.opacity"), kid_at + seconds(0.15), 1.0);
        s.ease(
            sc,
            &format!("{id}.draw"),
            kid_at + seconds(0.15),
            1.0,
            0.4,
            Ease::Smoothstep,
        );
    }
    let kid_word = t(said.at("spaghetti kid"));
    for (index, eye) in ["eye-l", "eye-r"].into_iter().enumerate() {
        let at = kid_word + seconds(0.06 * index as f64);
        s.channel(sc, &format!("{eye}.scale"), 0.2);
        s.set(sc, &format!("{eye}.opacity"), at, 1.0);
        s.bounce(sc, &format!("{eye}.scale"), at, 1.0, 0.3, 0.4);
    }
    let and = t(said.at("and he's going"));
    later.word("w-kid", kid_word, Some(and), 0.8);
    punch(
        &mut later,
        kid_word,
        170.0,
        Angle {
            roll: std::f32::consts::TAU,
            ..front(80.0)
        },
        140.0,
        0.05,
    );

    // "...going to be EATEN by WILL SMITH!" A fork comes down and spears
    // the kid, then yanks him away before the name lands.
    later.cut(
        and,
        Angle {
            roll: std::f32::consts::TAU,
            ..front(-60.0)
        },
    );
    let eaten = t(said.at("eaten"));
    let down = t(said.at("going"));
    s.ease(
        sc,
        "fork.y",
        down,
        0.0,
        ((eaten - down) as f64 / 1e9) as f32,
        Ease::CubicBezier([0.55, 0.0, 1.0, 0.6]),
    );
    later.jolt(eaten, [0.0, 1.0], 0.9);
    for id in MARKERS.iter().chain(&["eye-l", "eye-r"]) {
        later.hit(&format!("{id}.pulse"), eaten, 1.0, 0.0);
    }
    let smith = t(said.at("will smith"));
    let lift = t(said.at_after("by", "eaten"));
    let rise = ((smith - lift) as f64 / 1e9).min(0.22) as f32;
    let yank = Ease::CubicBezier([0.6, 0.0, 1.0, 1.0]);
    s.ease(sc, "fork.y", lift, -1100.0, rise, yank);
    for (index, id) in MARKERS.iter().enumerate() {
        let kid_y = KID[index][1] - SUIT[index][1];
        s.ease(sc, &format!("{id}.y"), lift, kid_y - 1100.0, rise, yank);
    }
    for id in ["eye-l", "eye-r"] {
        s.ease(sc, &format!("{id}.y"), lift, -1100.0, rise, yank);
    }
    for id in &noodle_bones {
        s.ease(
            sc,
            &format!("{id}.opacity"),
            lift + seconds(f64::from(rise)),
            0.0,
            0.05,
            Ease::Linear,
        );
    }
    later.word("w-smith", smith, None, 1.0);
    punch(
        &mut later,
        smith,
        330.0,
        Angle {
            roll: std::f32::consts::TAU,
            ..front(60.0)
        },
        240.0,
        -0.04,
    );
    later.flash(smith, 0.3, 0.25);
    s.set(sc, "camera.quake", smith, 1.8);
    for index in 0..CROWD {
        later.hit(&format!("crowd-{index}.pulse"), smith, 1.0, 0.0);
    }
    later.run(s, sc);

    let said_words = words(said, &window, str::to_uppercase);
    let plan = shout_caption(66.0, Tone::Error);
    let mut shout = subtitles(sc, "sub-spaghetti", &said_words, plan.clone())?;
    let hits = [
        (lasagna_at, 0.8),
        (again, 0.4),
        (spaghetti_at, 0.8),
        (woo, 1.2),
        (kid_at, 0.5),
        (kid_word, 0.8),
        (and, 0.4),
        (eaten, 0.8),
        (smith, 1.2),
    ];
    jitter(sc, &mut shout, &hits, -1.0);
    ghost(
        sc,
        "sub-spaghetti-ghost",
        &said_words,
        &plan,
        Tone::Request,
        &hits,
        0,
    )?;
    Ok(window.finish(scene, film)?.into())
}
