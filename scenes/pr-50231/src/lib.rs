//! #50231, "chore: upgrade Effect to rc.117", as a narrated Stage film.
//!
//! The compiler caught every rename; the film is about the three behaviors it
//! could not catch. Saved settings lose unknown fields at a schema gate, tool
//! schemas arrive in shapes their consumers reject, and permission rules are
//! silently reordered so a last-match-wins scan lands on the wrong card. Each
//! story is keyed to phrases in the Eleven v4 narration.
mod diff;
mod intro;
mod outro;
mod permissions;
mod settings;
mod tools;

use std::path::Path;

use anyhow::Result;
use psychopomp::{
    author::PlanBuilder,
    caption::{CaptionActor, CaptionAlign, CaptionPlan, CaptionSpanPlan},
    narration::Narration,
    plan::{ReelPlan, ReelSegmentPlan, ReelTransitionStyle},
    sfx::Sfx,
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
        transition_wipe: None,
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

// Eleven Sound Effects v2 stems from `sfx/generate.ts`, with their exact
// lengths (48 kHz sample counts). Paths resolve against the reel file.
const REWIND: Sfx = Sfx::new("sfx/rewind.wav", 1_085_833_333);
const SHUFFLE: Sfx = Sfx::new("sfx/shuffle.wav", 870_375_000);
const DROP: Sfx = Sfx::new("sfx/drop.wav", 880_000_000);
const RESOLUTION: Sfx = Sfx::new("sfx/resolution.wav", 880_000_000);

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
