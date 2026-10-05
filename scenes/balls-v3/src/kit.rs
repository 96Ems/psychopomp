//! Small authoring helpers shared by the segments: a segment's window on the
//! film clock, stage elements as JSON, word-timed subtitles that come apart,
//! and a queue for the hits, slams, and camera cuts that many beats write to
//! the same channels.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, seconds},
    face::Face,
    math::easing::Ease,
    plan::ScenePlan,
    stage::{StageActor, StagePlan},
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
        scene.sort_events();
        scene.drop_events_after_end();
        Ok(scene.finish()?)
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

/// A label set in `face`.
pub fn typed(id: &str, at: [f32; 3], size: f32, face: Face, spans: &[(&str, &str)]) -> Value {
    let mut value = label(id, at, size, spans);
    value["face"] = serde_json::to_value(face).expect("a face serializes");
    value
}

pub fn form(id: &str, at: [f32; 3], points: u32, tone: &str, shapes: Value) -> Value {
    json!({ "kind": "form", "id": id, "at": at, "points": points, "tone": tone, "shapes": shapes })
}

/// Quadratic ease out and in, exactly: a thrown ball's rise and fall.
pub const RISE: Ease = Ease::CubicBezier([1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 1.0]);
pub const FALL: Ease = Ease::CubicBezier([1.0 / 3.0, 0.0, 2.0 / 3.0, 1.0 / 3.0]);

/// Drop `id` from `height` pixels above its rest so it lands exactly at
/// `land`, then bounce it `hops` times, each hop `restitution` as high (and
/// its square root as long) as the last. Returns every contact time.
pub fn drop(
    s: &mut StageActor,
    sc: &mut PlanBuilder,
    id: &str,
    land: u64,
    height: f32,
    restitution: f32,
    hops: u32,
) -> Vec<u64> {
    let y = format!("{id}.y");
    // A fall from `height` under a gravity that takes `fall` seconds.
    let fall = 0.26_f32 * (height / 700.0).sqrt();
    let start = land.saturating_sub(seconds(fall as f64));
    s.channel(sc, &y, -height);
    s.ease(sc, &y, start, 0.0, fall, FALL);
    let mut contacts = vec![land];
    let (mut at, mut rise, mut time) = (land, height, fall);
    for _ in 0..hops {
        rise *= restitution;
        time *= restitution.sqrt();
        s.ease(sc, &y, at, -rise, time, RISE);
        at += seconds(time as f64);
        s.ease(sc, &y, at, 0.0, time, FALL);
        at += seconds(time as f64);
        contacts.push(at);
    }
    contacts
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

type Write = Box<dyn FnOnce(&mut StageActor, &mut PlanBuilder)>;

/// Deferred writes, run in time order: flashes, pulses, jolts, and camera
/// moves land on shared channels from many beats, and a channel's events
/// must be ordered.
#[derive(Default)]
pub struct Later {
    queue: Vec<(u64, usize, Write)>,
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

    pub fn jolt(&mut self, at: u64, direction: [f32; 2], strength: f32) {
        self.at(at, move |s, sc| s.jolt(sc, at, direction, strength));
    }

    /// The hype register's slam: a jolt, a zoom streak, chroma, and a flash.
    pub fn slam(&mut self, at: u64, direction: [f32; 2], strength: f32) {
        self.jolt(at, direction, strength);
        self.hit("post.zoom", at, 0.14 * strength, 0.0);
        self.hit("post.chroma", at, 0.45 * strength, 0.0);
        self.flash(at, 0.07 * strength, 0.14);
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

    /// Glide the camera to `angle` over `seconds`, from a frame boundary
    /// so it follows a cut at the same moment.
    pub fn glide(&mut self, at: u64, angle: Angle, seconds: f32) {
        let at = snap(at);
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

    /// Whip the camera to `angle`: a minimum-jerk move the shutter streaks,
    /// with a radial zoom streak through its middle.
    pub fn whip(&mut self, at: u64, angle: Angle, seconds: f32) {
        self.glide(at, angle, seconds);
        self.at(at, move |s, sc| {
            let half = seconds * 0.5;
            s.ease(sc, "post.zoom", at, 0.18, half, Ease::Smoothstep);
            let middle = at + ms((half * 1000.0).round() as u64);
            s.ease(sc, "post.zoom", middle, 0.0, half, Ease::Smoothstep);
        });
    }

    pub fn run(mut self, stage: &mut StageActor, scene: &mut PlanBuilder) {
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
    from: u64,
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
    jitter(scene, &mut double, hits, -1.6);
    Ok(())
}
