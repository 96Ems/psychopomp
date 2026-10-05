//! Silence. A shaky exhale, one small ball in the dark: "...balls with dots."
//! Then the camera pulls back: the ball is one dot on a bigger ball, which
//! is one dot on a bigger ball, which is one dot on a planet of dots.
//! "In the end, there were balls."
//!
//! There is no camera move. Each bigger ball starts scaled up so one of its
//! front dots sits exactly where the smaller ball is, and shrinks while the
//! smaller ball shrinks by the same factor, so the smaller ball becomes that
//! dot. Orb dots keep their pixel size at any scale, so a ball shrunk small
//! enough reads as one dot.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, seconds},
    math::{Quat, Vec3, easing::Ease},
    stage::{FOCAL, StageActor, orb_points},
    subtitles::SubtitlesPlan,
    tone::Tone,
};
use serde_json::{Value, json};

use crate::{
    Shot,
    kit::{Later, Window, orb, stage, subtitles},
    sound::{CHAIN, ENDING, Film},
};

/// The small ball, and each bigger ball: radius, points, tone.
const SMALL: (f32, u32) = (46.0, 260);
const LEVELS: [(f32, u32, &str); 3] = [
    (300.0, 1500, "plain"),
    (360.0, 2400, "accent"),
    (600.0, 4000, "accent"),
];
/// How big one dot looks: a ball shrunk to this radius becomes one.
const DOT: f32 = 2.4;
/// Where the closing title's measured line starts, to sit centered.
const TITLE_LEFT: f32 = 470.0;
/// The planet's radius on screen when it settles.
const PLANET: f32 = 520.0;
const ZOOM: Ease = Ease::CubicBezier([0.45, 0.0, 0.2, 1.0]);

/// An orb's resting turn (no spin): the dot of `points` that faces the lens.
fn front_dot(points: u32) -> Vec3 {
    let turn = Quat::from_rotation_x(0.42);
    orb_points(points)
        .into_iter()
        .map(|point| turn * point.unit)
        .min_by(|a, b| {
            let toward = Vec3::new(0.0, 0.0, -1.0);
            a.distance(toward).total_cmp(&b.distance(toward))
        })
        .expect("an orb has points")
}

/// How large something of world `radius` at world depth `z` looks.
fn apparent(radius: f32, z: f32) -> f32 {
    radius * FOCAL / (z + FOCAL)
}

/// One pull back: the bigger ball `id` starts at scale `from` and settles at
/// `to`; its front dot stays on `center`, where the smaller ball shrinks.
struct Level {
    id: String,
    at: Vec3,
    dot: Vec3,
    radius: f32,
    from: f32,
    to: f32,
}

impl Level {
    fn offset(&self, scale: f32) -> Vec3 {
        -(scale - 1.0) * self.radius * self.dot
    }

    /// Where its center rests once settled.
    fn center(&self) -> Vec3 {
        self.at + self.offset(self.to)
    }

    fn element(&self, points: u32, tone: &str) -> Value {
        orb(&self.id, self.at.to_array(), self.radius, points, tone)
    }

    fn prepare(&self, s: &mut StageActor, sc: &mut PlanBuilder) {
        let start = self.offset(self.from);
        for (axis, value) in ["x", "y", "z"].into_iter().zip(start.to_array()) {
            s.channel(sc, &format!("{}.{axis}", self.id), value);
        }
        s.channel(sc, &format!("{}.scale", self.id), self.from);
        s.channel(sc, &format!("{}.opacity", self.id), 0.0);
        s.channel(sc, &format!("{}.spin", self.id), 0.0);
    }

    fn pull_back(&self, s: &mut StageActor, sc: &mut PlanBuilder, at: u64, length: f32) {
        let end = self.offset(self.to);
        for (axis, value) in ["x", "y", "z"].into_iter().zip(end.to_array()) {
            s.ease(sc, &format!("{}.{axis}", self.id), at, value, length, ZOOM);
        }
        s.ease(sc, &format!("{}.scale", self.id), at, self.to, length, ZOOM);
        s.ease(
            sc,
            &format!("{}.opacity", self.id),
            at,
            1.0,
            0.25,
            Ease::Smoothstep,
        );
    }
}

pub fn segment(film: &Film, window: Window) -> Result<Shot> {
    let mut scene = window.scene("after");
    let sc = &mut scene;
    let t = |at: u64| window.t(at);

    // Lay out the chain from the small ball outward.
    let small = Vec3::new(960.0, 520.0, 0.0);
    let mut center = small;
    let mut size = apparent(SMALL.0, small.z);
    let mut levels = Vec::new();
    for (index, (radius, points, _)) in LEVELS.into_iter().enumerate() {
        let dot = front_dot(points);
        let last = index + 1 == LEVELS.len();
        // The planet settles large; the others at their own size.
        let to = if last {
            // radius·to·FOCAL / (center.z + radius·to + FOCAL) = PLANET
            PLANET * (center.z + FOCAL) / (radius * (FOCAL - PLANET))
        } else {
            1.0
        };
        let from = to * size / DOT;
        let level = Level {
            id: format!("level-{index}"),
            at: center - radius * dot,
            dot,
            radius,
            from,
            to,
        };
        center = level.center();
        size = apparent(radius * to, center.z);
        levels.push(level);
    }

    let mut elements = vec![orb("ball", small.to_array(), SMALL.0, SMALL.1, "accent")];
    for (level, (_, points, tone)) in levels.iter().zip(LEVELS) {
        elements.push(level.element(points, tone));
    }
    let plan = stage(
        elements,
        json!({ "bloom": 0.5, "grain": 0.03, "vignette": 0.62, "backdrop": 0.0 }),
    )?;
    let mut stage = StageActor::declare(sc, "stage", &plan)?;
    let s = &mut stage;
    let mut later = Later::default();
    s.channel(sc, "ball.opacity", 0.0);
    s.channel(sc, "ball.blur", 6.0);
    s.channel(sc, "ball.spin", 0.0);
    s.channel(sc, "ball.scale", 1.0);
    for level in &levels {
        level.prepare(s, sc);
    }

    // After a beat of dead silence, it glows up out of the dark with the breath.
    let appear = t(film.exhale) + seconds(0.35);
    s.ease(sc, "ball.opacity", appear, 0.9, 1.3, Ease::Smoothstep);
    s.ease(sc, "ball.blur", appear, 0.0, 1.5, Ease::Smoothstep);
    later.hit("ball.pulse", t(film.after.at("balls")), 0.35, 0.0);
    later.hit("ball.pulse", t(film.after.at("dots")), 0.6, 0.0);

    // Pull back: each ball becomes one dot of the next, faster each time.
    let mut at = t(film.chain);
    let mut smaller = ("ball".to_owned(), 1.0_f32);
    for (level, length) in levels.iter().zip(CHAIN) {
        let length = length as f32;
        level.pull_back(s, sc, at, length);
        let (id, scale) = &smaller;
        let shrunk = scale * level.to / level.from;
        s.ease(sc, &format!("{id}.scale"), at, shrunk, length, ZOOM);
        // Once it is one dot among the rest, the real dot takes over.
        let gone = at + seconds(f64::from(length) * 0.5);
        s.ease(
            sc,
            &format!("{id}.opacity"),
            gone,
            0.0,
            length * 0.4,
            Ease::Smoothstep,
        );
        later.hit(
            &format!("{}.pulse", level.id),
            at + seconds(f64::from(length) * 0.6),
            0.5,
            0.0,
        );
        smaller = (level.id.clone(), level.to);
        at += seconds(f64::from(length));
    }

    // "In the end, there were balls." The planet blooms and turns slowly
    // behind it, then everything goes dark.
    let planet = levels[levels.len() - 1].id.clone();
    let title_at = t(film.title);
    let out = title_at + seconds(ENDING.0 + ENDING.1);
    let rest = ((window.duration - title_at) as f64 / 1e9) as f32;
    s.ease(
        sc,
        &format!("{planet}.opacity"),
        title_at,
        0.6,
        0.8,
        Ease::Smoothstep,
    );
    s.ease(
        sc,
        &format!("{planet}.rotation"),
        title_at,
        0.6,
        rest,
        Ease::Linear,
    );
    later.hit(&format!("{planet}.pulse"), title_at, 0.8, 0.15);
    later.hit("post.bloom", title_at, 0.9, 0.55);
    let fade = planet.clone();
    later.at(out, move |s, sc| {
        s.ease(
            sc,
            &format!("{fade}.opacity"),
            out,
            0.0,
            ENDING.2 as f32,
            Ease::Smoothstep,
        );
    });
    later.run(s, sc);
    crate::ball::title(
        sc,
        "title",
        "In the end, there were balls.",
        TITLE_LEFT,
        470.0,
        title_at,
        out,
    )?;

    let words = film
        .after
        .words()
        .into_iter()
        .enumerate()
        .map(|(index, (text, start, end))| {
            let text = match index {
                0 => format!("…{}", text.to_lowercase()),
                _ => text.to_lowercase(),
            };
            (text, t(start), t(end))
        })
        .collect::<Vec<_>>();
    let mut whisper = subtitles(
        sc,
        "sub-after",
        &words,
        SubtitlesPlan::new([960.0, 680.0], 1200.0)
            .size(50.0)
            .highlight(Tone::Accent)
            .without_backing()
            .word_by_word(),
    )?;
    let opacity = whisper.channel(sc, "opacity", 1.0);
    sc.ease(&opacity, t(film.chain), 0.0, 0.3, Ease::Smoothstep);
    Ok(window.finish(scene, film)?.into())
}
