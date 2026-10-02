//! #50231, "chore: upgrade Effect to rc.117", as a narrated Stage film.
//!
//! The compiler caught every rename; the film is about the three behaviors it
//! could not catch. Saved settings lose unknown fields at a schema gate, tool
//! schemas arrive in shapes their consumers reject, and permission rules are
//! silently reordered so a last-match-wins scan lands on the wrong card. Each
//! story is keyed to phrases in the Eleven v4 narration.
mod diff;
mod intro;
mod narration;
mod outro;
mod permissions;
mod settings;
mod tools;

use std::path::{Path, PathBuf};

use anyhow::Result;
use narration::Narration;
use psychopomp::{
    author::PlanBuilder,
    caption::{CaptionActor, CaptionAlign, CaptionPlan, CaptionSpanPlan},
    effects::spinner::Mark,
    plan::{
        MediaKindPlan, MediaPlan, MediaRolePlan, ReelPlan, ReelSegmentPlan, ReelTransitionStyle,
    },
    stage::{StageElement, StagePost, StatusText},
    tone::Tone,
};

const LEFT: f32 = 140.0;
const RIGHT: f32 = 1780.0;
const HEADER_Y: f32 = 96.0;
const FOOTER_Y: f32 = 1004.0;
const DIP: u64 = 600_000_000;
const ZOOM: u64 = 1_000_000_000;

pub fn build_reel(narration_dir: &Path) -> Result<ReelPlan> {
    let narration = Narration::load(narration_dir)?;
    let (stage, code, focus) = permissions::build(&narration)?;
    let segment = |transition_nanos, transition_style, plan| ReelSegmentPlan {
        transition_nanos,
        transition_style,
        transition_focus: None,
        plan,
    };
    let reel = ReelPlan {
        version: ReelPlan::VERSION,
        id: "pr-50231".to_owned(),
        segments: vec![
            segment(0, ReelTransitionStyle::Dip, intro::build(&narration)?),
            segment(DIP, ReelTransitionStyle::Dip, settings::build(&narration)?),
            segment(DIP, ReelTransitionStyle::Dip, tools::build(&narration)?),
            segment(DIP, ReelTransitionStyle::Dip, stage),
            ReelSegmentPlan {
                transition_focus: Some(focus),
                ..segment(ZOOM, ReelTransitionStyle::Zoom, code)
            },
            segment(DIP, ReelTransitionStyle::Dip, outro::build(&narration)?),
        ],
    };
    reel.validate()?;
    Ok(reel)
}

fn ns(seconds: f64) -> u64 {
    (seconds * 1e9).round() as u64
}

fn span(text: &str, tone: Tone) -> CaptionSpanPlan {
    CaptionSpanPlan::new(text, tone)
}

/// `#50231  saved settings`, top left.
fn header(scene: &mut PlanBuilder, title: &str, type_at: Option<u64>) -> Result<()> {
    let plan = CaptionPlan::line(
        [LEFT, HEADER_Y],
        30.0,
        vec![
            span("#50231", Tone::Accent),
            span("  ", Tone::Plain),
            span(title, Tone::Plain),
        ],
    );
    let mut caption = CaptionActor::declare(scene, "header", &plan)?;
    match type_at {
        Some(at) => {
            caption.type_in(scene, at, 60.0, 0.5);
        }
        None => caption.show(scene, 0),
    }
    Ok(())
}

/// A status chip, top right: `● before`.
fn chip(scene: &mut PlanBuilder, id: &str, dot: Tone, text: &str) -> Result<CaptionActor> {
    let plan = CaptionPlan::line(
        [RIGHT, HEADER_Y],
        22.0,
        vec![span("● ", dot), span(text, Tone::Plain)],
    )
    .aligned(CaptionAlign::Right)
    .chip();
    CaptionActor::declare(scene, id, &plan)
}

fn footer(scene: &mut PlanBuilder, id: &str, spans: Vec<CaptionSpanPlan>) -> Result<CaptionActor> {
    CaptionActor::declare(scene, id, &CaptionPlan::line([LEFT, FOOTER_Y], 28.0, spans))
}

/// The look shared by every Stage segment: restrained bloom, a quiet frame.
const POST: StagePost = StagePost {
    bloom: 0.18,
    grain: 0.012,
    vignette: 0.22,
    backdrop: 0.12,
};

fn status(text: &str, tone: Tone) -> StatusText {
    StatusText {
        text: text.to_owned(),
        tone,
    }
}

fn card(
    id: &str,
    at: [f32; 3],
    size: [f32; 2],
    title: &str,
    status: Vec<StatusText>,
    tone: Tone,
) -> StageElement {
    StageElement::Card {
        id: id.into(),
        at,
        size,
        title: title.into(),
        status,
        tone,
        mark: Mark::Check,
    }
}

fn beam(id: &str, from: &str, to: &str, tone: Tone) -> StageElement {
    StageElement::Beam {
        id: id.into(),
        from: from.into(),
        to: to.into(),
        bend: 0.0,
        tone,
    }
}

fn packet(id: &str, beam: &str, tone: Tone) -> StageElement {
    StageElement::Packet {
        id: id.into(),
        beam: beam.into(),
        reverse: false,
        label: String::new(),
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
        spans: parts.iter().map(|(text, tone)| span(text, *tone)).collect(),
    }
}

/// A sound file and its length in seconds. Paths resolve against the reel file.
#[derive(Clone, Copy)]
struct Sfx(&'static str, f64);

const TICK: Sfx = Sfx("../../assets/visual-effects/task-running.wav", 0.13);
const SEND: Sfx = Sfx("../../assets/opencode-hot-reload/save.wav", 0.15);
const FAILURE: Sfx = Sfx("../../assets/visual-effects/task-failure.wav", 0.47);
const MARK: Sfx = Sfx("../../assets/pr-walkthrough/mark.wav", 0.3);
const GLITCH: Sfx = Sfx("../../assets/pr-walkthrough/glitch.wav", 0.2);
// Eleven Sound Effects v2 stems from `sfx/generate.ts`.
const REWIND: Sfx = Sfx("sfx/rewind.wav", 1.084);
const SHUFFLE: Sfx = Sfx("sfx/shuffle.wav", 0.868);
const DROP: Sfx = Sfx("sfx/drop.wav", 0.878);
const RESOLUTION: Sfx = Sfx("sfx/resolution.wav", 0.878);

fn sound(id: &str, Sfx(file, seconds): Sfx, at: u64, gain_db: f32) -> MediaPlan {
    let length = ns(seconds);
    MediaPlan {
        id: id.to_owned(),
        path: PathBuf::from(file),
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
        let committed = std::fs::read_to_string(root.join("pr-50231.reel.json")).unwrap();
        assert_eq!(
            serde_json::to_string_pretty(&reel).unwrap() + "\n",
            committed
        );
    }
}
