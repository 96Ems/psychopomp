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
    caption::CaptionSpanPlan,
    chrome::{self, chip, footer},
    narration::Narration,
    plan::{ReelPlan, ReelSegmentPlan, ReelTransitionStyle},
    sfx::Sfx,
    tone::Tone,
};

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
    let mut caption = chrome::header(scene, "#50231", title)?;
    match type_at {
        Some(at) => {
            caption.type_in(scene, at, 60.0, 0.5);
        }
        None => caption.show(scene, 0),
    }
    Ok(())
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
