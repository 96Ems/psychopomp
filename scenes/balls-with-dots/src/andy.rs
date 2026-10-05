//! ANDY SERKIS: the balls slam onto a motion-capture suit, a loupe shows
//! they are balls with dots, and the markers become anything: a chimp, a
//! little shriveled guy. Then lasagna, spaghetti, and a spaghetti kid, who is
//! speared by a giant fork and carried off to be eaten by WILL SMITH. The
//! camera cuts on every shouted word, and the balls in the dark behind him
//! multiply.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, seconds},
    callout::{CalloutAnchorPlan, CalloutSide},
    lens::{LensActor, LensPlan},
    math::{easing::Ease, lerp, random::hash},
    stage::StageActor,
    tone::Tone,
};
use serde_json::{Value, json};

use crate::{
    Shot,
    kit::{
        Angle, Later, Sphere, Window, caption, ghost, jitter, label, orb, stage, subtitles, upper,
        words,
    },
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

/// Dancing: arms thrown up, one knee raised, leaning left.
const DANCE_A: Pose = [
    [930.0, 285.0],
    [940.0, 375.0],
    [960.0, 600.0],
    [860.0, 380.0],
    [800.0, 290.0],
    [760.0, 200.0],
    [1025.0, 385.0],
    [1100.0, 300.0],
    [1150.0, 210.0],
    [905.0, 612.0],
    [860.0, 735.0],
    [880.0, 865.0],
    [1015.0, 612.0],
    [1090.0, 700.0],
    [1040.0, 790.0],
];

/// Dancing: arms out low, the other knee up, leaning right.
const DANCE_B: Pose = [
    [995.0, 290.0],
    [985.0, 378.0],
    [960.0, 600.0],
    [900.0, 400.0],
    [800.0, 455.0],
    [700.0, 430.0],
    [1070.0, 395.0],
    [1150.0, 470.0],
    [1240.0, 440.0],
    [905.0, 612.0],
    [830.0, 700.0],
    [880.0, 790.0],
    [1015.0, 612.0],
    [1060.0, 735.0],
    [1040.0, 865.0],
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
    fn words(self) -> &'static [(&'static str, &'static str, f32, &'static str, f32)] {
        // id, text, size, tone, height
        match self {
            Self::Andy => &[
                ("w-andy", "ANDY SERKIS!", 150.0, "plain", 150.0),
                ("w-anything", "ANYTHING!", 160.0, "accent", 150.0),
                ("w-chimp", "A CHIMP!", 150.0, "plain", 150.0),
                ("w-guy", "A LITTLE SHRIVELED GUY!", 96.0, "plain", 150.0),
            ],
            Self::Spaghetti => &[
                ("w-lasagna", "LASAGNA!", 150.0, "warning", 150.0),
                ("w-spaghetti", "SPAGHETTI!", 150.0, "warning", 150.0),
                ("w-woo", "WOO!", 160.0, "error", 380.0),
                ("w-kid", "SPAGHETTI KID!", 130.0, "warning", 150.0),
                ("w-smith", "WILL SMITH!", 160.0, "error", 540.0),
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

/// Crowd ball `index` as a sphere to dive into (an orb: an even index not a
/// multiple of four's neighbours).
fn crowd_sphere(index: usize) -> Sphere {
    let ball = crowd(index);
    let at = &ball["at"];
    let at = [0, 1, 2].map(|k| at[k].as_f64().expect("a position") as f32);
    Sphere {
        at,
        radius: ball["radius"].as_f64().expect("an orb") as f32,
        points: ball["points"].as_u64().expect("an orb") as u32,
        rotation: 0.0,
        spin: 1.0,
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
    }
    for (id, text, size, tone, y) in part.words() {
        // Will Smith is announced right up against the lens, filling the frame.
        let z = if *id == "w-smith" { -380.0 } else { 0.0 };
        elements.push(label(id, [960.0, *y, z], *size, &[(text, tone)]));
    }
    if part == Part::Andy {
        let mut rec = label(
            "rec",
            [70.0, 70.0, 0.0],
            28.0,
            &[("● REC  ", "error"), ("MOCAP · TAKE 1", "muted")],
        );
        rec["align"] = json!("left");
        elements.push(rec);
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
    later.word("w-andy", slam, Some(t(said.at("if you cover"))), 1.0);
    s.set(sc, "camera.quake", slam, 1.2);
    s.ease(sc, "camera.quake", slam, 0.35, 1.4, Ease::CubicOut);
    later.at(slam, move |s, sc| {
        s.ease(sc, "camera.z", slam, 0.0, 1.0, Ease::CubicOut);
    });
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

    // "If you cover Andy Serkis": he dances, and the camera circles him.
    let cover = t(said.at("if you cover"));
    let serkis = t(said.at_after("serkis", "cover"));
    let dots = t(said.at("balls with dots"));
    let steps = [
        (cover, DANCE_A),
        (t(said.at_after("andy", "cover")), DANCE_B),
        (serkis, DANCE_A),
        (t(said.at_after("in", "cover")), DANCE_B),
    ];
    for (at, step) in steps {
        pose(s, sc, at, &step, 0.22, 0.0);
    }
    pose(s, sc, dots, &T_POSE, 0.3, 0.0);
    let swing = ((dots - cover) as f64 / 1e9) as f32;
    later.at(cover, move |s, sc| {
        s.set(sc, "camera.yaw", cover, -0.75);
        s.set(sc, "camera.pitch", cover, 0.12);
        s.set(sc, "camera.z", cover, 120.0);
        s.ease(sc, "camera.yaw", cover, 0.7, swing, Ease::Linear);
        s.ease(sc, "camera.pitch", cover, -0.18, swing, Ease::Smoothstep);
    });
    later.hit("post.zoom", cover, 0.12, 0.0);
    // "...in balls with dots": a loupe finds the dots on one ball.
    later.cut(dots, front(120.0));
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
    later.word("w-guy", guy, Some(window.duration), 0.6);
    punch(
        &mut later,
        t(said.at("shriveled")),
        150.0,
        front(60.0),
        120.0,
        0.05,
    );
    arrive(s, &mut later, sc, 8..20, guy);
    s.ease(sc, "camera.quake", guy, 0.7, 0.4, Ease::Linear);
    later.run(s, sc);

    let said_words = words(said, &window, upper);
    let plan = caption(60.0, Tone::Error);
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
        (anything, None),
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

    // "He could become LASAGNA!" Layers of filling between sheets of pasta.
    let lasagna_at = t(said.at("lasagna"));
    pose(s, sc, lasagna_at, &lasagna(), 0.3, 0.008);
    for id in &bones {
        s.channel(sc, &format!("{id}.opacity"), 0.0);
    }
    fade(s, sc, &bones, lasagna_at, 0.0, 0.1);
    // The sheets slam down one by one, from the bottom up.
    for (index, slab) in slabs.iter().enumerate().rev() {
        let order = (SLABS.len() - 1 - index) as f64;
        let land = lasagna_at + seconds(0.09 * order);
        let fall = 0.12;
        let start = land.saturating_sub(seconds(fall));
        s.channel(sc, &format!("{slab}.y"), -900.0);
        s.channel(sc, &format!("{slab}.scale"), 1.0);
        s.set(sc, &format!("{slab}.opacity"), start, 1.0);
        s.ease(
            sc,
            &format!("{slab}.y"),
            start,
            0.0,
            fall as f32,
            Ease::CubicBezier([0.5, 0.0, 1.0, 1.0]),
        );
        later.jolt(land, [0.0, 1.0], 0.45);
        later.hit(&format!("{slab}.pulse"), land, 1.0, 0.0);
    }
    let again = t(said.at_after("he could become", "lasagna"));
    later.word("w-lasagna", lasagna_at + seconds(0.27), Some(again), 0.8);
    later.cut(
        lasagna_at,
        Angle {
            y: -40.0,
            z: 120.0,
            pitch: 0.5,
            ..Angle::default()
        },
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

    // "WOO!" The plate spins, the noodles writhe, the dark bursts.
    let kid_at = t(said.at_after("andy serkis", "woo"));
    later.word("w-woo", woo, Some(kid_at), 1.0);
    later.cut(
        woo,
        Angle {
            yaw: 0.35,
            z: 0.0,
            roll: 0.2,
            ..Angle::default()
        },
    );
    later.at(woo, move |s, sc| {
        s.bounce(sc, "camera.roll", woo + seconds(0.05), -0.08, 0.6, 0.4);
    });
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
    later.cut(kid_at, front(0.0));
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
    punch(&mut later, kid_word, 150.0, front(80.0), 240.0, 0.05);

    // "...going to be EATEN by WILL SMITH!" A fork comes down and spears
    // the kid, then yanks him away before the name lands.
    later.cut(and, front(-60.0));
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
    let dive = smith + seconds(0.45);
    later.word("w-smith", smith, Some(dive), 1.0);
    later.crash(
        smith,
        Angle {
            roll: -0.05,
            ..front(160.0)
        },
        front(40.0),
        0.5,
    );
    later.hit("post.chroma", smith, 0.7, 0.1);
    s.set(sc, "camera.quake", smith, 1.8);
    for index in 0..CROWD {
        later.hit(&format!("crowd-{index}.pulse"), smith, 1.0, 0.0);
    }
    // ...and into one of the balls in the dark, where the chaos is.
    let end = window.until - window.from;
    let exit = later.dive(crowd_sphere(2), dive, end, crate::ball::DIVE);
    later.run(s, sc);

    let said_words = words(said, &window, upper);
    let plan = caption(64.0, Tone::Error);
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
    // The captions leave as the camera dives.
    let gone = shout.channel(sc, "opacity", 1.0);
    sc.ease(&gone, dive + seconds(0.1), 0.0, 0.15, Ease::Linear);
    ghost(
        sc,
        "sub-spaghetti-ghost",
        &said_words,
        &plan,
        Tone::Request,
        &hits,
        (0, Some(dive + seconds(0.1))),
    )?;
    Ok(Shot {
        plan: window.finish(scene, film)?,
        entry: None,
        exit: Some(exit),
    })
}
