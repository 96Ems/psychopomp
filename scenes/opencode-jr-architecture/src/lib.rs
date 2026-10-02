//! A narrated architecture walkthrough of OpenCode Jr, the Slack bot in
//! `opencode-slack`: every Slack thread becomes one durable object that hosts
//! a coding agent, with a sandbox for its hands.
//!
//! Each chapter is a Stage film keyed to phrases in its narration clip, so
//! re-voicing the script re-times the reel. The edge chapter zooms from the
//! Worker card into a condensed view of `src/ingress.ts`.
mod around;
mod code;
mod edge;
mod intro;
mod outro;
mod publish;
mod resets;
mod session;
mod workspace;

use std::path::{Path, PathBuf};

use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, seconds},
    caption::{CaptionActor, CaptionAlign, CaptionPlan, CaptionSpanPlan},
    effects::spinner::Mark,
    narration::{Narration, Spoken},
    plan::{
        MediaKindPlan, MediaPlan, MediaRolePlan, ReelPlan, ReelSegmentPlan, ReelTransitionStyle,
        ScenePlan,
    },
    stage::{StageActor, StageElement, StagePlan, StagePost, StatusText},
    tone::Tone,
};

const TRANSITION: f64 = 0.7;
const LEFT: f32 = 140.0;
const RIGHT: f32 = 1780.0;
const HEADER_Y: f32 = 96.0;
const FOOTER_Y: f32 = 1004.0;

pub fn build_reel(narration_dir: &Path) -> Result<ReelPlan> {
    let narration = Narration::load(narration_dir)?;
    let (edge, worker_rect) = edge::film(&narration)?;
    let segments: Vec<(ScenePlan, ReelTransitionStyle, Option<[f32; 4]>)> = vec![
        (intro::film(&narration)?, ReelTransitionStyle::Dip, None),
        (edge, ReelTransitionStyle::Dip, None),
        (
            code::film(&narration)?,
            ReelTransitionStyle::Zoom,
            Some(worker_rect),
        ),
        (session::film(&narration)?, ReelTransitionStyle::Dip, None),
        (workspace::film(&narration)?, ReelTransitionStyle::Dip, None),
        (publish::film(&narration)?, ReelTransitionStyle::Dip, None),
        (resets::film(&narration)?, ReelTransitionStyle::Dip, None),
        (around::film(&narration)?, ReelTransitionStyle::Dip, None),
        (outro::film(&narration)?, ReelTransitionStyle::Dip, None),
    ];
    let reel = ReelPlan {
        version: ReelPlan::VERSION,
        id: "opencode-jr-architecture".to_owned(),
        segments: segments
            .into_iter()
            .enumerate()
            .map(|(index, (plan, style, focus))| ReelSegmentPlan {
                transition_nanos: match (index, style) {
                    (0, _) => 0,
                    (_, ReelTransitionStyle::Zoom) => seconds(1.15),
                    _ => seconds(TRANSITION),
                },
                transition_style: style,
                transition_focus: focus,
                plan,
            })
            .collect(),
    };
    reel.validate()?;
    Ok(reel)
}

// ---------------------------------------------------------------------------
// Shared pieces
// ---------------------------------------------------------------------------

fn span(text: &str, tone: Tone) -> CaptionSpanPlan {
    CaptionSpanPlan::new(text, tone)
}

fn spans(parts: &[(&str, Tone)]) -> Vec<CaptionSpanPlan> {
    parts.iter().map(|(text, tone)| span(text, *tone)).collect()
}

/// One chapter: its plan clock, its Stage, and its placed narration clip.
struct Film<'a> {
    sc: PlanBuilder,
    s: StageActor,
    v: Spoken<'a>,
}

/// A chapter whose clip starts after `lead` seconds and holds `tail` seconds
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
    s.channel(&mut sc, "camera.z", -140.0);
    s.channel(&mut sc, "camera.dof", 0.4);
    s.to(&mut sc, "camera.z", 0, 0.0, 2.2);
    Ok(Film { sc, s, v })
}

/// `3  the workspace`, typed top left.
fn header(sc: &mut PlanBuilder, number: &str, title: &str) -> Result<()> {
    header_at(sc, number, title, Some(seconds(0.3)))
}

/// The header, typed at `typed`, or already present when `None`.
fn header_at(sc: &mut PlanBuilder, number: &str, title: &str, typed: Option<u64>) -> Result<()> {
    let plan = CaptionPlan::line(
        [LEFT, HEADER_Y],
        30.0,
        vec![
            span(number, Tone::Accent),
            span("  ", Tone::Plain),
            span(title, Tone::Plain),
        ],
    );
    let mut caption = CaptionActor::declare(sc, "header", &plan)?;
    if let Some(at) = typed {
        caption.type_in(sc, at, 50.0, 0.6);
    }
    Ok(())
}

/// The source the chapter is read from, as a chip top right.
fn chip(sc: &mut PlanBuilder, text: &str) -> Result<CaptionActor> {
    chip_at(sc, text, Some(seconds(0.7)))
}

fn chip_at(sc: &mut PlanBuilder, text: &str, shown: Option<u64>) -> Result<CaptionActor> {
    let plan = CaptionPlan::line(
        [RIGHT, HEADER_Y],
        22.0,
        vec![span("● ", Tone::Muted), span(text, Tone::Plain)],
    )
    .aligned(CaptionAlign::Right)
    .chip();
    let mut chip = CaptionActor::declare(sc, "chip", &plan)?;
    if let Some(at) = shown {
        chip.show(sc, at);
    }
    Ok(chip)
}

/// The one caption that sums up a beat, typed bottom left.
fn footer(sc: &mut PlanBuilder, id: &str, parts: &[(&str, Tone)], at: u64) -> Result<CaptionActor> {
    let mut caption = CaptionActor::declare(
        sc,
        id,
        &CaptionPlan::line([LEFT, FOOTER_Y], 28.0, spans(parts)),
    )?;
    caption.type_in(sc, at, 42.0, 0.8);
    Ok(caption)
}

fn post() -> StagePost {
    StagePost {
        bloom: 0.18,
        grain: 0.012,
        vignette: 0.22,
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

fn beam(id: &str, from: &str, to: &str, tone: Tone) -> StageElement {
    bent(id, from, to, 0.0, tone)
}

fn bent(id: &str, from: &str, to: &str, bend: f32, tone: Tone) -> StageElement {
    StageElement::Beam {
        id: id.into(),
        from: from.into(),
        to: to.into(),
        bend,
        tone,
    }
}

fn packet(id: &str, beam: &str, reverse: bool, label: &str, tone: Tone) -> StageElement {
    StageElement::Packet {
        id: id.into(),
        beam: beam.into(),
        reverse,
        label: label.into(),
        tone,
    }
}

fn label(
    id: &str,
    at: [f32; 3],
    size: f32,
    align: CaptionAlign,
    parts: &[(&str, Tone)],
) -> StageElement {
    StageElement::Label {
        id: id.into(),
        at,
        size,
        align,
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
    s.ease(
        sc,
        &format!("{id}.rotation"),
        at,
        0.0,
        1.25,
        psychopomp::math::easing::Ease::CubicOut,
    );
    s.to(sc, &format!("{id}.opacity"), at, 1.0, 0.6);
}

/// Fade a label in place.
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

/// Settle a card in and plug `wire` into it a beat later, with a soft tick.
/// Returns the contact time. Flow rests again after its brief proof.
fn arrive(s: &mut StageActor, sc: &mut PlanBuilder, card: &str, wire: &str, at: u64) -> u64 {
    let ready = s.settle_in(sc, card, at);
    plug(s, sc, wire, ready.saturating_sub(seconds(0.3)))
}

/// Draw `wire` in, then let its flow rest.
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
fn send(s: &mut StageActor, sc: &mut PlanBuilder, packet: &str, at: u64, seconds: f32) -> u64 {
    sc.media(sound(&format!("send-{packet}"), SEND, at, -14.0));
    s.send(sc, packet, at, seconds)
}

/// Sound assets under `assets/` and their lengths in seconds.
struct Sfx(&'static str, f64);

const TICK: Sfx = Sfx("visual-effects/task-running.wav", 0.13);
const SEND: Sfx = Sfx("opencode-hot-reload/save.wav", 0.15);
const SUCCESS: Sfx = Sfx("visual-effects/task-success.wav", 0.41);
const LAUNCH: Sfx = Sfx("opencode-hot-reload/launch.wav", 1.36);
const IMPACT: Sfx = Sfx("opencode-hot-reload/impact.wav", 0.51);
const DEATH: Sfx = Sfx("visual-effects/task-death.wav", 1.09);
const GLITCH: Sfx = Sfx("pr-walkthrough/glitch.wav", 0.2);
const MARK: Sfx = Sfx("pr-walkthrough/mark.wav", 0.3);
const RESET: Sfx = Sfx("visual-effects/task-reset.wav", 0.33);
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
        let committed =
            std::fs::read_to_string(root.join("opencode-jr-architecture.reel.json")).unwrap();
        assert_eq!(
            serde_json::to_string_pretty(&reel).unwrap() + "\n",
            committed
        );
        assert_eq!(reel.segments.len(), 9);
    }
}
