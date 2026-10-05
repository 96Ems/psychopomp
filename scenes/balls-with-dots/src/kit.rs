//! Small authoring helpers shared by the segments: a segment's window on the
//! film clock, stage elements as JSON, word-timed subtitles that come apart,
//! and a queue for the hits, slams, and camera cuts that many beats write to
//! the same channels.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, seconds},
    math::{Quat, Vec3, easing::Ease},
    plan::ScenePlan,
    stage::{Camera, FOCAL, StageActor, StagePlan, orb_points},
    subtitles::{SubtitlesActor, SubtitlesPlan},
    tone::Tone,
};
use serde_json::{Value, json};

use crate::sound::{Film, Voice};

/// A segment's place on the film clock: it starts at `from`, plays its own
/// sound until `until` (where the next segment starts), and runs `duration`.
#[derive(Clone, Copy, Debug)]
pub struct Window {
    pub from: u64,
    pub until: u64,
    pub duration: u64,
}

impl Window {
    /// A film time on this segment's clock (clamped to its start).
    pub fn t(&self, film: u64) -> u64 {
        film.saturating_sub(self.from).min(self.duration)
    }

    /// The segment's plan builder.
    pub fn scene(&self, id: &str) -> PlanBuilder {
        PlanBuilder::new(id, self.duration)
    }

    /// Add the segment's sound and validate it.
    pub fn finish(&self, mut scene: PlanBuilder, film: &Film) -> Result<ScenePlan> {
        for media in film.media(self.from, self.until) {
            scene.media(media);
        }
        let named = scene.channel_ids();
        scene.finish().map_err(|error| {
            let lines = error
                .diagnostics()
                .iter()
                .take(6)
                .map(|d| {
                    let channel = d
                        .path
                        .strip_prefix("continuousChannels[")
                        .and_then(|rest| rest.split(']').next())
                        .and_then(|index| index.parse::<usize>().ok())
                        .and_then(|index| named.get(index).cloned())
                        .unwrap_or_default();
                    format!("{channel} {}: {}", d.path, d.message)
                })
                .collect::<Vec<_>>();
            anyhow::anyhow!("{}", lines.join("\n"))
        })
    }
}

/// The nearest frame boundary at 60 fps. A camera cut there falls between
/// two frames' shutters, so neither frame blurs across it.
pub fn snap(at: u64) -> u64 {
    ((at as f64 * 60.0 / 1e9).round() * 1e9 / 60.0).round() as u64
}

pub fn ms(millis: u64) -> u64 {
    seconds(millis as f64 / 1000.0)
}

pub fn stage(elements: Vec<Value>, post: Value) -> Result<StagePlan> {
    Ok(serde_json::from_value(
        json!({ "elements": elements, "post": post }),
    )?)
}

pub fn orb(id: &str, at: [f32; 3], radius: f32, points: u32, tone: &str) -> Value {
    json!({ "kind": "orb", "id": id, "at": at, "radius": radius, "points": points, "tone": tone })
}

pub fn label(id: &str, at: [f32; 3], size: f32, spans: &[(&str, &str)]) -> Value {
    let spans = spans
        .iter()
        .map(|(text, tone)| json!({ "text": text, "tone": tone }))
        .collect::<Vec<_>>();
    json!({ "kind": "label", "id": id, "at": at, "size": size, "spans": spans })
}

/// A camera position for a hard cut: pan, dolly, and turn.
#[derive(Clone, Copy, Debug, Default)]
pub struct Angle {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
}

impl Angle {
    fn channels(self) -> [(&'static str, f32); 6] {
        [
            ("camera.x", self.x),
            ("camera.y", self.y),
            ("camera.z", self.z),
            ("camera.yaw", self.yaw),
            ("camera.pitch", self.pitch),
            ("camera.roll", self.roll),
        ]
    }
}

/// A sphere of dots on the Stage (an orb, or a form whose shape is a
/// sphere) at rest, so the camera can fly into one of its dots.
#[derive(Clone, Copy, Debug)]
pub struct Sphere {
    pub at: [f32; 3],
    pub radius: f32,
    pub points: u32,
    /// Its `rotation` channel, held still during the dive.
    pub rotation: f32,
    /// Its `spin` channel: ambient turn of 0.14 rad/s per unit.
    pub spin: f32,
}

impl Sphere {
    /// The world position at local time `at` of the dot that faces the
    /// lens most squarely, as the renderer places it (tilt, spin, breath).
    pub fn front(&self, at: u64) -> Vec3 {
        let time = at as f32 / 1e9;
        let turn = Quat::from_rotation_x(0.42)
            * Quat::from_rotation_y(time * 0.14 * self.spin + self.rotation);
        let breath = 1.0 + 0.006 * (time * 1.4).sin();
        let toward = Vec3::new(0.0, 0.0, -1.0);
        let unit = orb_points(self.points)
            .into_iter()
            .map(|point| turn * point.unit)
            .min_by(|a, b| a.distance(toward).total_cmp(&b.distance(toward)))
            .expect("a sphere has dots");
        Vec3::from(self.at) + unit * self.radius * breath
    }
}

/// Where a ball of `radius` at `at` sits on screen under `camera`, as a
/// match cut's target rectangle.
pub fn ball_rect(camera: Camera, at: [f32; 3], radius: f32) -> [f32; 4] {
    let (center, scale) = camera
        .project(Vec3::from(at))
        .expect("the ball is in front of the camera");
    let radius = radius * scale;
    [
        center.x - radius,
        center.y - radius,
        radius * 2.0,
        radius * 2.0,
    ]
}

type Write = Box<dyn FnOnce(&mut StageActor, &mut PlanBuilder)>;

/// Deferred writes, run in time order: flashes, pulses, jolts, and camera
/// moves land on shared channels from many beats, and a channel's events
/// must be ordered.
#[derive(Default)]
pub struct Later {
    queue: Vec<(u64, usize, Write)>,
    jolts: Vec<(u64, [f32; 2], f32)>,
}

impl Later {
    pub fn at(&mut self, at: u64, write: impl FnOnce(&mut StageActor, &mut PlanBuilder) + 'static) {
        let order = self.queue.len();
        self.queue.push((at, order, Box::new(write)));
    }

    /// `property` lights to `peak` and decays to `rest`.
    pub fn hit(&mut self, property: &str, at: u64, peak: f32, rest: f32) {
        let property = property.to_owned();
        self.at(at, move |s, sc| s.hit(sc, &property, at, peak, rest));
    }

    /// A white flash that decays over `seconds`.
    pub fn flash(&mut self, at: u64, peak: f32, seconds: f32) {
        self.at(at, move |s, sc| {
            s.set(sc, "post.flash", at, peak);
            s.ease(sc, "post.flash", at, 0.0, seconds, Ease::CubicOut);
        });
    }

    /// Jolts closer together than a kick's shove merge into the strongest.
    pub fn jolt(&mut self, at: u64, direction: [f32; 2], strength: f32) {
        self.jolts.push((at, direction, strength));
    }

    /// The hype register's slam: a jolt, a zoom streak, chroma, and a flash.
    pub fn slam(&mut self, at: u64, direction: [f32; 2], strength: f32) {
        self.jolt(at, direction, strength);
        self.hit("post.zoom", at, 0.14 * strength, 0.0);
        self.hit("post.chroma", at, 0.45 * strength, 0.0);
        self.flash(at, 0.16 * strength, 0.15);
    }

    /// A shouted word: label `id` slams in from 1.7× on a stiff spring with
    /// the slam's jolt, streak, chroma, and flash, and leaves at `until`.
    pub fn word(&mut self, id: &str, at: u64, until: Option<u64>, strength: f32) {
        let id = id.to_owned();
        self.at(at, move |s, sc| {
            // Unseen until it slams in.
            s.channel(sc, &format!("{id}.opacity"), 0.0);
            s.channel(sc, &format!("{id}.scale"), 0.0);
            s.set(sc, &format!("{id}.opacity"), at, 1.0);
            s.set(sc, &format!("{id}.scale"), at, 1.7);
            s.bounce(sc, &format!("{id}.scale"), at, 1.0, 0.3, 0.3);
            if let Some(until) = until {
                s.ease(sc, &format!("{id}.opacity"), until, 0.0, 0.1, Ease::Linear);
            }
        });
        self.slam(at, [0.0, 1.0], strength);
    }

    /// Cut the camera to `angle` between two frames.
    pub fn cut(&mut self, at: u64, angle: Angle) {
        let at = snap(at);
        self.at(at, move |s, sc| {
            for (property, value) in angle.channels() {
                s.set(sc, property, at, value);
            }
        });
    }

    /// Glide the camera to `angle` over `seconds`.
    pub fn glide(&mut self, at: u64, angle: Angle, seconds: f32) {
        self.at(at, move |s, sc| {
            for (property, value) in angle.channels() {
                s.glide(sc, property, at, value, seconds);
            }
        });
    }

    /// A crash zoom: cut to `close`, then ease back toward `rest` over
    /// `seconds`, as if the operator lunged at the word and recovered.
    pub fn crash(&mut self, at: u64, close: Angle, rest: Angle, seconds: f32) {
        self.cut(at, close);
        let at = snap(at);
        self.at(at, move |s, sc| {
            for (property, value) in rest.channels() {
                s.ease(sc, property, at, value, seconds, Ease::Decelerate(1.6));
            }
        });
    }

    /// Dive into `sphere`: the camera levels out and flies at its front dot
    /// from `from`, faster and faster, settling at `end` with the dot a
    /// glowing disc `radius` pixels across the middle of the frame, and
    /// holding it steady through the match cut that follows. Returns that
    /// disc's rectangle, which the match turns into the next ball.
    pub fn dive(&mut self, sphere: Sphere, from: u64, end: u64, radius: f32) -> [f32; 4] {
        let dot = sphere.front(end);
        let depth = 2.0 * FOCAL / radius;
        assert!(end > from, "a dive needs time");
        let length = (end - from) as f32 / 1e9;
        self.at(from, move |s, sc| {
            // Undeclared camera channels start at 0; the zoom must start at 1.
            s.channel(sc, "camera.zoom", 1.0);
            let settle = Ease::CubicBezier([0.4, 0.0, 0.2, 1.0]);
            for (property, target) in [
                ("camera.x", dot.x - 960.0),
                ("camera.y", dot.y - 540.0),
                ("camera.yaw", 0.0),
                ("camera.pitch", 0.0),
                ("camera.roll", 0.0),
                ("camera.zoom", 1.0),
            ] {
                s.ease(sc, property, from, target, length * 0.85, settle);
            }
            s.ease(
                sc,
                "camera.z",
                from,
                dot.z + FOCAL - depth,
                length,
                Ease::CubicBezier([0.5, 0.0, 0.3, 1.0]),
            );
            // Close to a dot, any shake is magnified: the camera goes still.
            for property in [
                "camera.quake",
                "camera.handheld",
                "camera.dof",
                "camera.shake",
                "camera.kick-x",
                "camera.kick-y",
            ] {
                s.ease(sc, property, from, 0.0, length * 0.4, Ease::Smoothstep);
            }
            s.ease(sc, "post.zoom", from, 0.14, length * 0.7, Ease::Smoothstep);
            s.ease(
                sc,
                "post.zoom",
                from + ms((length * 700.0) as u64),
                0.0,
                length * 0.3,
                Ease::Smoothstep,
            );
        });
        [960.0 - radius, 540.0 - radius, radius * 2.0, radius * 2.0]
    }

    pub fn run(mut self, stage: &mut StageActor, scene: &mut PlanBuilder) {
        // Jolts closer together than a kick's shove merge into the strongest,
        // then run in time order with everything else.
        self.jolts.sort_by_key(|(at, ..)| *at);
        let mut merged: Vec<(u64, [f32; 2], f32)> = Vec::new();
        for jolt in std::mem::take(&mut self.jolts) {
            match merged.last_mut() {
                Some(last) if jolt.0 < last.0 + ms(60) => {
                    if jolt.2 > last.2 {
                        *last = (last.0, jolt.1, jolt.2);
                    }
                }
                _ => merged.push(jolt),
            }
        }
        let limit = scene.duration_nanos();
        for (at, direction, strength) in merged {
            if at + ms(40) < limit {
                self.at(at, move |s, sc| s.jolt(sc, at, direction, strength));
            }
        }
        self.queue.sort_by_key(|(at, order, _)| (*at, *order));
        for (_, _, write) in self.queue {
            write(stage, scene);
        }
    }
}

/// Words for subtitles on a segment's clock, each rewritten by `style`.
pub fn words(voice: &Voice, window: &Window, style: fn(&str) -> String) -> Vec<(String, u64, u64)> {
    voice
        .words()
        .into_iter()
        .filter(|(_, start, end)| *end > window.from && *start < window.from + window.duration)
        .map(|(text, start, end)| (style(&text), window.t(start), window.t(end)))
        .collect()
}

pub fn lower(text: &str) -> String {
    text.to_lowercase()
}

pub fn upper(text: &str) -> String {
    text.to_uppercase()
}

/// Subtitles of `words`, declared with `id`.
pub fn subtitles(
    scene: &mut PlanBuilder,
    id: &str,
    words: &[(String, u64, u64)],
    plan: SubtitlesPlan,
) -> Result<SubtitlesActor> {
    let plan = words.iter().fold(plan, |plan, (text, start, end)| {
        plan.word(text.clone(), *start, *end)
    });
    SubtitlesActor::declare(scene, id, &plan)
}

/// Knock the subtitles sideways and askew at each of `hits` and let them
/// spring back, a little crooked: sanity coming loose.
pub fn jitter(scene: &mut PlanBuilder, said: &mut SubtitlesActor, hits: &[(u64, f32)], phase: f32) {
    let x = said.channel(scene, "x", 0.0);
    let y = said.channel(scene, "y", 0.0);
    let tilt = said.channel(scene, "tilt", 0.0);
    let mut hits = hits.to_vec();
    hits.sort_by_key(|(at, _)| *at);
    for (index, (at, strength)) in hits.into_iter().enumerate() {
        let side = if index % 2 == 0 { phase } else { -phase };
        scene.spring(&x, at, 22.0 * side * strength, 0.04, 0.0);
        scene.spring(&x, at + ms(40), 0.0, 0.45, 0.45);
        scene.spring(&y, at, -12.0 * strength, 0.04, 0.0);
        scene.spring(&y, at + ms(40), 0.0, 0.45, 0.45);
        scene.spring(&tilt, at, 0.07 * side * strength, 0.05, 0.0);
        scene.spring(&tilt, at + ms(50), 0.025 * side * strength, 0.5, 0.5);
    }
}

/// A second, see-through copy of `words` a few pixels off and in another
/// color: the subtitles start seeing double. It shakes on its own beat.
pub fn ghost(
    scene: &mut PlanBuilder,
    id: &str,
    words: &[(String, u64, u64)],
    plan: &SubtitlesPlan,
    tone: Tone,
    hits: &[(u64, f32)],
    (from, until): (u64, Option<u64>),
) -> Result<()> {
    let plan = SubtitlesPlan {
        origin: [plan.origin[0] + 7.0, plan.origin[1] - 5.0],
        highlight: tone,
        backing: false,
        ..plan.clone()
    };
    let mut double = subtitles(scene, id, words, plan)?;
    let opacity = double.channel(scene, "opacity", 0.0);
    scene.ease(&opacity, from, 0.45, 0.3, Ease::Smoothstep);
    if let Some(until) = until {
        scene.ease(&opacity, until, 0.0, 0.15, Ease::Linear);
    }
    jitter(scene, &mut double, hits, -1.6);
    Ok(())
}

/// A bottom-of-frame subtitle plan at `size` with its highlight `tone`.
pub fn caption(size: f32, tone: Tone) -> SubtitlesPlan {
    SubtitlesPlan::new([960.0, 985.0 - (size - 40.0) * 0.6], 1500.0)
        .size(size)
        .highlight(tone)
}
