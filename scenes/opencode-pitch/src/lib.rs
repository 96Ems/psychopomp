//! A short motion-graphics pitch for OpenCode quality loops: an agent's
//! insight is rented and lost, then kept as a regression test; three loops
//! and a two-week pilot follow. Each segment is a Stage film keyed to phrases
//! in its narration clip, so re-voicing re-times the reel.
mod close;
mod kept;
mod loops;
mod pilot;
mod rented;

use std::path::{Path, PathBuf};

use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, seconds},
    caption::{CaptionActor, CaptionAlign, CaptionPlan, CaptionSpanPlan},
    effects::spinner::Mark,
    math::easing::Ease,
    narration::{Narration, Spoken},
    plan::{
        MediaKindPlan, MediaPlan, MediaRolePlan, ReelPlan, ReelSegmentPlan, ReelTransitionStyle,
    },
    stage::{StageActor, StageElement, StagePlan, StagePost, StatusText},
    tone::Tone,
};

const LEFT: f32 = 140.0;
const HEADER_Y: f32 = 96.0;
const FOOTER_Y: f32 = 1004.0;
/// The camera rests a little closer than the authored plane, so the stage fills the frame.
const REST_Z: f32 = 150.0;

pub fn build_reel(narration_dir: &Path) -> Result<ReelPlan> {
    let narration = Narration::load(narration_dir)?;
    let plans = [
        rented::film(&narration)?,
        kept::film(&narration)?,
        loops::film(&narration)?,
        pilot::film(&narration)?,
        close::film(&narration)?,
    ];
    let reel = ReelPlan {
        version: ReelPlan::VERSION,
        id: "opencode-pitch".to_owned(),
        segments: plans
            .into_iter()
            .enumerate()
            .map(|(index, plan)| ReelSegmentPlan {
                transition_nanos: if index == 0 { 0 } else { seconds(0.6) },
                transition_style: ReelTransitionStyle::Dip,
                transition_focus: None,
                transition_wipe: None,
                plan,
            })
            .collect(),
    };
    reel.validate()?;
    Ok(reel)
}

fn spans(parts: &[(&str, Tone)]) -> Vec<CaptionSpanPlan> {
    parts
        .iter()
        .map(|(text, tone)| CaptionSpanPlan::new(*text, *tone))
        .collect()
}

/// One segment: its plan clock, its Stage, and its placed narration clip.
struct Film<'a> {
    sc: PlanBuilder,
    s: StageActor,
    v: Spoken<'a>,
}

/// A segment whose clip starts after `lead` seconds and holds `tail` seconds
/// after it ends. The camera dollies in from slightly behind its rest.
fn begin<'a>(
    narration: &'a Narration,
    id: &str,
    lead: f64,
    tail: f64,
    stage: &StagePlan,
) -> Result<Film<'a>> {
    let clip = narration.clip(id)?;
    let mut sc = PlanBuilder::new(id, seconds(lead) + clip.duration() + seconds(tail));
    let v = clip.place(&mut sc, seconds(lead));
    let mut s = StageActor::declare(&mut sc, "stage", stage)?;
    s.channel(&mut sc, "camera.z", REST_Z - 140.0);
    s.channel(&mut sc, "camera.dof", 0.4);
    s.to(&mut sc, "camera.z", 0, REST_Z, 2.2);
    Ok(Film { sc, s, v })
}

/// `01  the problem`, typed top left.
fn header(sc: &mut PlanBuilder, number: &str, title: &str) -> Result<()> {
    let plan = CaptionPlan::line(
        [LEFT, HEADER_Y],
        28.0,
        spans(&[
            (number, Tone::Accent),
            ("  ", Tone::Plain),
            (title, Tone::Plain),
        ]),
    );
    CaptionActor::declare(sc, "header", &plan)?.type_in(sc, seconds(0.3), 50.0, 0.6);
    Ok(())
}

/// The one caption that sums up a segment, typed bottom left.
fn footer(sc: &mut PlanBuilder, parts: &[(&str, Tone)], at: u64) -> Result<CaptionActor> {
    let mut caption = CaptionActor::declare(
        sc,
        "footer",
        &CaptionPlan::line([LEFT, FOOTER_Y], 28.0, spans(parts)),
    )?;
    caption.type_in(sc, at, 42.0, 0.8);
    Ok(caption)
}

fn post() -> StagePost {
    StagePost {
        bloom: 0.2,
        grain: 0.012,
        vignette: 0.24,
        backdrop: 0.12,
    }
}

fn card(
    id: &str,
    at: [f32; 3],
    size: [f32; 2],
    title: &str,
    status: &[(&str, Tone)],
    tone: Tone,
) -> StageElement {
    StageElement::Card {
        id: id.into(),
        at,
        size,
        title: title.into(),
        status: status
            .iter()
            .map(|(text, tone)| StatusText {
                text: (*text).to_owned(),
                tone: *tone,
            })
            .collect(),
        tone,
        mark: Mark::Check,
    }
}

fn orb(id: &str, at: [f32; 3], radius: f32, points: u32) -> StageElement {
    StageElement::Orb {
        id: id.into(),
        at,
        radius,
        points,
        tone: Tone::Plain,
    }
}

fn beam(id: &str, from: &str, to: &str, bend: f32, tone: Tone) -> StageElement {
    StageElement::Beam {
        id: id.into(),
        from: from.into(),
        to: to.into(),
        bend,
        tone,
    }
}

fn packet(id: &str, beam: &str, reverse: bool, tone: Tone) -> StageElement {
    StageElement::Packet {
        id: id.into(),
        beam: beam.into(),
        reverse,
        label: String::new(),
        tone,
    }
}

fn label(id: &str, at: [f32; 3], size: f32, parts: &[(&str, Tone)]) -> StageElement {
    StageElement::Label {
        id: id.into(),
        at,
        size,
        align: CaptionAlign::Center,
        spans: spans(parts),
    }
}

fn ring(id: &str, at: [f32; 3], radius: f32, thickness: f32, tone: Tone) -> StageElement {
    StageElement::Ring {
        id: id.into(),
        at,
        radius,
        thickness,
        tone,
    }
}

/// The hero entrance for an orb: it gathers out of a blur while turning
/// into place. Call before any other channel of the orb is written.
fn orb_in(s: &mut StageActor, sc: &mut PlanBuilder, id: &str, at: u64) {
    for (property, initial) in [("scale", 0.58), ("blur", 11.0), ("rotation", -1.8)] {
        s.channel(sc, &format!("{id}.{property}"), initial);
    }
    s.bounce(sc, &format!("{id}.scale"), at, 1.0, 0.85, 0.2);
    s.to(sc, &format!("{id}.blur"), at, 0.0, 0.7);
    s.ease(sc, &format!("{id}.rotation"), at, 0.0, 1.25, Ease::CubicOut);
    s.to(sc, &format!("{id}.opacity"), at, 1.0, 0.6);
}

fn show(s: &mut StageActor, sc: &mut PlanBuilder, id: &str, at: u64) {
    s.to(sc, &format!("{id}.opacity"), at, 1.0, 0.4);
}

fn hide(s: &mut StageActor, sc: &mut PlanBuilder, id: &str, at: u64) {
    s.to(sc, &format!("{id}.opacity"), at, 0.0, 0.35);
}

/// Cross-fade a card's status line to entry `index`.
fn status(s: &mut StageActor, sc: &mut PlanBuilder, card: &str, at: u64, index: usize) {
    s.to(sc, &format!("{card}.status"), at, index as f32, 0.35);
}

/// Settle a card in and plug `wire` into it a beat later. Returns contact.
fn arrive(s: &mut StageActor, sc: &mut PlanBuilder, card: &str, wire: &str, at: u64) -> u64 {
    let ready = s.settle_in(sc, card, at);
    plug(s, sc, wire, ready.saturating_sub(seconds(0.3)))
}

/// Draw `wire` in with a soft tick, then let its brief flow rest.
fn plug(s: &mut StageActor, sc: &mut PlanBuilder, wire: &str, at: u64) -> u64 {
    let contact = s.connect(sc, wire, at, 0.6);
    s.to(
        sc,
        &format!("{wire}.flow"),
        contact + seconds(0.85),
        0.0,
        0.45,
    );
    sc.media(sound(&format!("plug-{wire}"), TICK, contact, -22.0));
    contact
}

/// Send `packet` with its quiet launch sound; returns its arrival time.
fn send(s: &mut StageActor, sc: &mut PlanBuilder, packet: &str, at: u64, flight: f32) -> u64 {
    sc.media(sound(&format!("send-{packet}"), SEND, at, -15.0));
    s.send(sc, packet, at, flight)
}

/// A few frames of band displacement on a card, then still.
fn glitch(s: &mut StageActor, sc: &mut PlanBuilder, card: &str, at: u64) {
    for (step, seed) in [7.0, 9.0, 8.0, 0.0].into_iter().enumerate() {
        s.set(
            sc,
            &format!("{card}.glitch"),
            at + seconds(step as f64 * 0.027),
            seed,
        );
    }
}

/// Sound assets under `assets/` and their lengths in seconds.
struct Sfx(&'static str, f64);

const TICK: Sfx = Sfx("visual-effects/task-running.wav", 0.13);
const SEND: Sfx = Sfx("opencode-hot-reload/save.wav", 0.15);
const SUCCESS: Sfx = Sfx("visual-effects/task-success.wav", 0.41);
const FAILURE: Sfx = Sfx("visual-effects/task-failure.wav", 0.47);
const IMPACT: Sfx = Sfx("opencode-hot-reload/impact.wav", 0.51);
const DEATH: Sfx = Sfx("visual-effects/task-death.wav", 1.09);
const GLITCH: Sfx = Sfx("pr-walkthrough/glitch.wav", 0.2);
const MARK: Sfx = Sfx("pr-walkthrough/mark.wav", 0.3);
const BLOOM: Sfx = Sfx("effect-shows-errors/prismatic-bloom.wav", 0.785);
const CONFIRM: Sfx = Sfx("opencode-hot-reload/confirm.wav", 0.33);

fn sound(id: &str, Sfx(file, length): Sfx, at: u64, gain_db: f32) -> MediaPlan {
    let length = seconds(length);
    MediaPlan {
        id: id.to_owned(),
        path: PathBuf::from(format!("../../assets/{file}")),
        kind: MediaKindPlan::Audio,
        role: MediaRolePlan::Layer,
        source_start_nanos: 0,
        source_end_nanos: length,
        timeline_start_nanos: at,
        timeline_end_nanos: at + length,
        gain_db,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    #[test]
    fn reel_matches_the_committed_plan() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let reel = super::build_reel(&root.join("narration")).unwrap();
        let committed = std::fs::read_to_string(root.join("opencode-pitch.reel.json")).unwrap();
        assert_eq!(
            serde_json::to_string_pretty(&reel).unwrap() + "\n",
            committed
        );
        assert_eq!(reel.segments.len(), 5);
        assert!(
            reel.duration_nanos() < psychopomp::author::seconds(120.0),
            "a pitch, not a lecture"
        );
    }
}
