//! Sound effects for Scene Programs: the repository's `assets/` catalog with
//! exact lengths (sample counts at 48 kHz, read with `ffprobe`), placed as
//! Layer Clips. Media paths resolve against the plan or reel file; the catalog
//! assumes it is written beside its Scene Program in `scenes/<name>/`, two
//! levels below `assets/`. A scene's own sounds use [`Sfx::new`].
use std::path::PathBuf;

use crate::{
    author::PlanBuilder,
    plan::{MediaKindPlan, MediaPlan, MediaRolePlan},
};

/// One sound file and its exact length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sfx {
    path: &'static str,
    length: u64,
}

impl Sfx {
    /// `path` relative to the plan file, `length` in nanoseconds.
    pub const fn new(path: &'static str, length: u64) -> Self {
        Self { path, length }
    }

    pub const fn path(self) -> &'static str {
        self.path
    }

    /// The whole file's length on the plan clock.
    pub const fn length(self) -> u64 {
        self.length
    }

    /// The whole sound as a Layer Clip starting at `at`.
    pub fn media(self, id: impl Into<String>, at: u64, gain_db: f32) -> MediaPlan {
        MediaPlan {
            id: id.into(),
            path: PathBuf::from(self.path),
            kind: MediaKindPlan::Audio,
            role: MediaRolePlan::Layer,
            source_start_nanos: 0,
            source_end_nanos: self.length,
            timeline_start_nanos: at,
            timeline_end_nanos: at + self.length,
            gain_db,
        }
    }

    /// Play the whole sound at `at`, `gain_db` relative to the file.
    pub fn play(self, scene: &mut PlanBuilder, id: impl Into<String>, at: u64, gain_db: f32) {
        scene.media(self.media(id, at, gain_db));
    }

    /// Return a composable [`crate::score::Beat`] that plays this sound at its cue time.
    pub fn beat(self, id: impl Into<String>, gain_db: f32) -> impl crate::score::Beat {
        let id = id.into();
        crate::score::impulse(move |scene, at| {
            self.play(scene, id, at, gain_db);
        })
    }
}

/// The length of `samples` at 48 kHz, to the nearest nanosecond.
const fn samples(samples: u64) -> u64 {
    (samples * 1_000_000_000 + 24_000) / 48_000
}

macro_rules! asset {
    ($file:literal, $samples:literal) => {
        Sfx::new(concat!("../../assets/", $file), samples($samples))
    };
}

/// A soft tick: a wire plugs in, a panel settles.
pub const TICK: Sfx = asset!("visual-effects/task-running.wav", 6_720);
/// A light send: a request leaves.
pub const SEND: Sfx = asset!("opencode-hot-reload/save.wav", 7_680);
/// A small alarm: an error reply lands, a prompt pops up.
pub const FAILURE: Sfx = asset!("visual-effects/task-failure.wav", 23_040);
/// A rising launch, used under rewinds and big sends.
pub const LAUNCH: Sfx = asset!("opencode-hot-reload/launch.wav", 65_760);
/// A hit with weight.
pub const IMPACT: Sfx = asset!("opencode-hot-reload/impact.wav", 24_960);
/// Something dies: an orb shatters.
pub const DEATH: Sfx = asset!("visual-effects/task-death.wav", 52_800);
/// Digital tearing under a card glitch.
pub const GLITCH: Sfx = asset!("pr-walkthrough/glitch.wav", 9_600);
/// A spinner resolving into its mark.
pub const MARK: Sfx = asset!("pr-walkthrough/mark.wav", 14_400);
/// A prismatic bloom: a clean resolution, everything breathes.
pub const BLOOM: Sfx = asset!("effect-shows-errors/prismatic-bloom.wav", 38_160);
/// A slice: a card is cut off.
pub const SEVER: Sfx = asset!("pr-walkthrough/sever.wav", 27_671);
/// A reset chime.
pub const RESET: Sfx = asset!("visual-effects/task-reset.wav", 16_320);
/// A success chime.
pub const SUCCESS: Sfx = asset!("visual-effects/task-success.wav", 20_160);
/// A confirmation.
pub const CONFIRM: Sfx = asset!("opencode-hot-reload/confirm.wav", 16_320);
/// A long riser that builds into a peak.
pub const RISER: Sfx = asset!("psychopomp-intro/riser.wav", 272_158);
/// A cinematic boom.
pub const BOOM: Sfx = asset!("psychopomp-intro/boom.wav", 144_000);
/// A whoosh past the lens.
pub const WHOOSH: Sfx = asset!("psychopomp-intro/whoosh.wav", 29_386);
/// A sparkle: something wakes.
pub const SPARKLE: Sfx = asset!("psychopomp-intro/sparkle.wav", 71_040);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_are_exact_sample_counts() {
        assert_eq!(TICK.length(), 140_000_000);
        assert_eq!(BLOOM.length(), 795_000_000);
        assert_eq!(SEVER.length(), 576_479_167, "27 671 samples, rounded");
        assert_eq!(WHOOSH.length(), 612_208_333);
        assert_eq!(TICK.path(), "../../assets/visual-effects/task-running.wav");
    }

    #[test]
    fn a_sound_plays_whole_as_a_layer_clip() {
        let mut scene = PlanBuilder::new("sfx", 2_000_000_000);
        MARK.play(&mut scene, "mark", 500_000_000, -18.0);
        let plan = scene.finish().unwrap();
        let media = &plan.media[0];
        assert_eq!(
            (
                media.source_end_nanos,
                media.timeline_start_nanos,
                media.timeline_end_nanos
            ),
            (300_000_000, 500_000_000, 800_000_000)
        );
        assert!(matches!(media.role, MediaRolePlan::Layer));
        assert_eq!(media.gain_db, -18.0);
    }
}
