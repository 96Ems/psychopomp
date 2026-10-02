//! Embedded video: one planned video media placement played inside a framed
//! card. The plan clock maps through the placement into source time, so a cue
//! or range render never restarts the footage. A focus window crops into a
//! region of the frame, as when zooming into part of a screen recording.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    author::{ActorHandle, ContinuousHandle, PlanBuilder},
    plan::{MediaKindPlan, MediaPlan, MediaRolePlan},
};

pub const VIDEO_RECIPE: &str = "video";

/// Every channel a video actor accepts. `x`/`y` offset the card from its
/// center; `rotation`, `tilt-x`, and `tilt-y` are radians; `blur` softens the
/// near edge of a tilted card. `focus-x`/`focus-y` are the visible window's
/// center and `focus-size` its size, as fractions of the frame.
pub const VIDEO_CHANNELS: [&str; 11] = [
    "x",
    "y",
    "scale",
    "opacity",
    "rotation",
    "tilt-x",
    "tilt-y",
    "blur",
    "focus-x",
    "focus-y",
    "focus-size",
];

/// Height of the optional title bar, in card pixels at scale 1.
pub const TITLE_BAR: f32 = 44.0;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoPlan {
    /// The planned video media this card plays.
    pub media_id: String,
    /// Decoded frame size in pixels; the footage keeps this aspect.
    pub size: [u32; 2],
    pub fps: u32,
    /// Card center on the canvas at rest.
    pub center: [f32; 2],
    /// Card width at scale 1; its height follows the footage and title bar.
    pub width: f32,
    /// A title bar above the footage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl VideoPlan {
    /// A card that fills most of the canvas width, centered.
    pub fn new(media_id: impl Into<String>, size: [u32; 2], fps: u32) -> Self {
        Self {
            media_id: media_id.into(),
            size,
            fps,
            center: [960.0, 540.0],
            width: 1400.0,
            title: None,
        }
    }

    pub fn at(mut self, center: [f32; 2], width: f32) -> Self {
        self.center = center;
        self.width = width;
        self
    }

    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    fn title_bar(&self) -> f32 {
        if self.title.is_some() { TITLE_BAR } else { 0.0 }
    }

    /// Card size at scale 1.
    pub fn card_size(&self) -> [f32; 2] {
        let footage = self.width * self.size[1] as f32 / self.size[0] as f32;
        [self.width, footage + self.title_bar()]
    }

    /// The footage rectangle inside the card, `[x, y, width, height]` from
    /// its top-left corner.
    pub fn footage_rect(&self) -> [f32; 4] {
        let [width, height] = self.card_size();
        let bar = self.title_bar();
        [0.0, bar, width, height - bar]
    }

    /// The visible source window `[x, y, width, height]` in source pixels for
    /// a focus center (fractions of the frame) and size (fraction of the frame
    /// visible). The window keeps the footage aspect and stays inside it.
    pub fn view(&self, center: [f32; 2], size: f32) -> [f32; 4] {
        let size = size.clamp(0.02, 1.0);
        let source = [self.size[0] as f32, self.size[1] as f32];
        let half = size * 0.5;
        let center = center.map(|c| c.clamp(half, 1.0 - half));
        [
            (center[0] - half) * source[0],
            (center[1] - half) * source[1],
            size * source[0],
            size * source[1],
        ]
    }

    /// The focus center and size whose view fits `region` (source pixels,
    /// `[x, y, width, height]`) inside it.
    pub fn focus_on(&self, region: [f32; 4]) -> ([f32; 2], f32) {
        let source = [self.size[0] as f32, self.size[1] as f32];
        let size = (region[2] / source[0]).max(region[3] / source[1]);
        (
            [
                (region[0] + region[2] * 0.5) / source[0],
                (region[1] + region[3] * 0.5) / source[1],
            ],
            size.clamp(0.02, 1.0),
        )
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.size[0] > 0 && self.size[1] > 0 && self.fps > 0,
            "video size and fps must be non-zero"
        );
        ensure!(
            self.center.iter().all(|v| v.is_finite()),
            "video center must be finite"
        );
        ensure!(
            self.width.is_finite() && (16.0..=4096.0).contains(&self.width),
            "video width must be between 16 and 4096"
        );
        if let Some(title) = &self.title {
            ensure!(
                !title.trim().is_empty() && title.chars().count() <= 80 && !title.contains('\n'),
                "video titles are one line of 1 to 80 characters"
            );
        }
        Ok(())
    }
}

/// A video placement: `source` nanoseconds of the file starting at `at` on
/// the plan clock.
pub fn media(
    id: impl Into<String>,
    path: impl Into<std::path::PathBuf>,
    source: (u64, u64),
    at: u64,
) -> MediaPlan {
    MediaPlan {
        id: id.into(),
        path: path.into(),
        kind: MediaKindPlan::Video,
        role: MediaRolePlan::Layer,
        source_start_nanos: source.0,
        source_end_nanos: source.1,
        timeline_start_nanos: at,
        timeline_end_nanos: at + (source.1 - source.0),
        gain_db: 0.0,
    }
}

/// Authoring handle for one video actor.
pub struct VideoActor {
    actor: ActorHandle,
    plan: VideoPlan,
}

impl VideoActor {
    /// Declare the card and the placement it plays.
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: VideoPlan,
        media: MediaPlan,
    ) -> Result<Self> {
        plan.validate()?;
        ensure!(
            matches!(media.kind, MediaKindPlan::Video) && media.id == plan.media_id,
            "a video card plays the video media '{}'",
            plan.media_id
        );
        let actor = scene.actor(id, VIDEO_RECIPE, &plan)?;
        scene.media(media);
        Ok(Self { actor, plan })
    }

    pub fn actor(&self) -> &ActorHandle {
        &self.actor
    }

    pub fn plan(&self) -> &VideoPlan {
        &self.plan
    }

    pub fn channel(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        scene.channel(&self.actor, property, initial)
    }

    /// Fly the card up into place: it starts low, small, tipped back, and
    /// soft, and lands on critically damped springs as it sharpens.
    pub fn fly_in(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        for (property, from, response) in [
            ("y", 220.0, 0.8),
            ("scale", 0.82, 0.8),
            ("tilt-x", 0.42, 0.85),
            ("blur", 9.0, 0.7),
            ("opacity", 0.0, 0.35),
        ] {
            let rest = if property == "scale" || property == "opacity" {
                1.0
            } else {
                0.0
            };
            let channel = self.channel(scene, property, from);
            scene.spring(&channel, at_nanos, rest, response, 0.0);
        }
    }

    /// Zoom the footage so `region` (source pixels) fills the card.
    pub fn focus(
        &mut self,
        scene: &mut PlanBuilder,
        at_nanos: u64,
        region: [f32; 4],
        seconds: f32,
    ) {
        let (center, size) = self.plan.focus_on(region);
        self.focus_to(scene, at_nanos, center, size, seconds);
    }

    /// Return to the whole frame.
    pub fn unfocus(&mut self, scene: &mut PlanBuilder, at_nanos: u64, seconds: f32) {
        self.focus_to(scene, at_nanos, [0.5, 0.5], 1.0, seconds);
    }

    fn focus_to(
        &mut self,
        scene: &mut PlanBuilder,
        at_nanos: u64,
        center: [f32; 2],
        size: f32,
        seconds: f32,
    ) {
        // The window's center and size move together on one spring, so every
        // point of the region travels monotonically to where it lands.
        for (property, target) in [
            ("focus-x", center[0]),
            ("focus-y", center[1]),
            ("focus-size", size),
        ] {
            let initial = if property == "focus-size" { 1.0 } else { 0.5 };
            let channel = self.channel(scene, property, initial);
            scene.spring(&channel, at_nanos, target, seconds, 0.0);
        }
    }

    /// Fade out in place.
    pub fn hide(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        let opacity = self.channel(scene, "opacity", 1.0);
        scene.spring(&opacity, at_nanos, 0.0, 0.3, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> VideoPlan {
        VideoPlan::new("clip", [1920, 760], 60).titled("OpenCode")
    }

    #[test]
    fn the_card_keeps_the_footage_aspect_below_its_title_bar() {
        let plan = plan();
        let [width, height] = plan.card_size();
        assert_eq!(width, 1400.0);
        assert!((height - (1400.0 * 760.0 / 1920.0 + TITLE_BAR)).abs() < 1e-3);
        assert_eq!(plan.footage_rect()[1], TITLE_BAR);
        assert_eq!(VideoPlan::new("clip", [100, 50], 30).footage_rect()[1], 0.0);
    }

    #[test]
    fn focus_windows_fit_the_region_and_stay_inside_the_frame() {
        let plan = plan();
        assert_eq!(plan.view([0.5, 0.5], 1.0), [0.0, 0.0, 1920.0, 760.0]);
        // A region wider than the footage aspect fits by width.
        let (center, size) = plan.focus_on([960.0, 0.0, 960.0, 190.0]);
        assert_eq!(size, 0.5);
        let view = plan.view(center, size);
        assert_eq!(view, [960.0, 0.0, 960.0, 380.0], "clamped into the frame");
        // A centered window is not shifted.
        assert_eq!(plan.view([0.5, 0.5], 0.5), [480.0, 190.0, 960.0, 380.0]);
    }

    #[test]
    fn declare_adds_the_card_and_its_video_placement() {
        let mut scene = PlanBuilder::new("video", 5_000_000_000);
        let mut card = VideoActor::declare(
            &mut scene,
            "card",
            plan().at([960.0, 520.0], 1200.0),
            media(
                "clip",
                "clip.mp4",
                (1_000_000_000, 4_000_000_000),
                500_000_000,
            ),
        )
        .unwrap();
        card.fly_in(&mut scene, 0);
        card.focus(&mut scene, 2_000_000_000, [0.0, 0.0, 960.0, 380.0], 0.8);
        let plan = scene.finish().unwrap();
        assert_eq!(plan.media[0].timeline_end_nanos, 3_500_000_000);
        let properties = plan
            .continuous_channels
            .iter()
            .map(|channel| channel.property.as_str())
            .collect::<Vec<_>>();
        assert!(properties.iter().all(|p| VIDEO_CHANNELS.contains(p)));
        assert!(properties.contains(&"focus-size"));

        let mut scene = PlanBuilder::new("video", 5_000_000_000);
        let mut audio = media("clip", "clip.wav", (0, 1), 0);
        audio.kind = MediaKindPlan::Audio;
        assert!(
            VideoActor::declare(
                &mut scene,
                "card",
                super::VideoPlan::new("clip", [2, 2], 1),
                audio
            )
            .is_err()
        );
    }

    #[test]
    fn invalid_videos_are_rejected() {
        let mut zero = plan();
        zero.fps = 0;
        assert!(zero.validate().is_err());
        assert!(plan().titled(" ").validate().is_err());
        assert!(plan().at([0.0, 0.0], 4.0).validate().is_err());
    }
}
