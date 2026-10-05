//! Images: one planned image media placement (PNG, JPEG, or WebP) drawn bare
//! or inside a framed card, through the same projected card compositor as the
//! Video Card, so it can move, scale, rotate, tilt, defocus, and pin to an
//! anchor. The renderer decodes the file once at preparation.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    anchor::{self, AnchorPlan},
    author::{ActorHandle, ContinuousHandle, PlanBuilder},
    plan::{MediaKindPlan, MediaPlan, MediaRolePlan},
    video::TITLE_BAR,
};

pub const IMAGE_RECIPE: &str = "image";

/// Every channel an image actor accepts, besides `anchor.<id>` weights. `x`/`y`
/// offset it from its center (or anchor); `rotation`, `tilt-x`, and `tilt-y`
/// are radians; `blur` softens the near edge of a tilted image.
pub const IMAGE_CHANNELS: [&str; 8] = [
    "x", "y", "scale", "opacity", "rotation", "tilt-x", "tilt-y", "blur",
];

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImagePlan {
    /// The planned image media this actor draws.
    pub media_id: String,
    /// Center on the canvas at rest; an anchor replaces it.
    pub center: [f32; 2],
    /// Width at scale 1; the height follows the image's aspect.
    pub width: f32,
    /// Draw inside a card: theme material, border, shadow, rounded corners.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub framed: bool,
    /// A title bar above the image; framed images only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Corner radius of a bare image, in pixels at scale 1.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub radius: f32,
    /// Places the image can pin to; while it has any, the blended anchor (plus
    /// that anchor's offset) replaces `center`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<AnchorPlan>,
}

fn is_zero(value: &f32) -> bool {
    *value == 0.0
}

impl ImagePlan {
    /// A bare image `width` pixels wide, centered at `center`.
    pub fn new(media_id: impl Into<String>, center: [f32; 2], width: f32) -> Self {
        Self {
            media_id: media_id.into(),
            center,
            width,
            framed: false,
            title: None,
            radius: 0.0,
            anchors: Vec::new(),
        }
    }

    /// Draw inside a card.
    pub fn framed(mut self) -> Self {
        self.framed = true;
        self
    }

    /// A framed card with a title bar.
    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.framed = true;
        self.title = Some(title.into());
        self
    }

    pub fn rounded(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Pin the image's center to `anchor`; the first anchor is where it starts.
    pub fn anchor(mut self, anchor: AnchorPlan) -> Self {
        self.anchors.push(anchor);
        self
    }

    pub fn title_bar(&self) -> f32 {
        if self.title.is_some() { TITLE_BAR } else { 0.0 }
    }

    /// Size at scale 1 for an image of `pixels` (width, height).
    pub fn card_size(&self, pixels: [u32; 2]) -> [f32; 2] {
        let height = self.width * pixels[1] as f32 / pixels[0].max(1) as f32;
        [self.width, height + self.title_bar()]
    }

    /// True when `property` names one of this image's channels.
    pub fn accepts(&self, property: &str) -> bool {
        IMAGE_CHANNELS.contains(&property) || anchor::accepts(property, &self.anchors)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(!self.media_id.is_empty(), "an image needs a media ID");
        ensure!(
            self.center.iter().all(|v| v.is_finite()),
            "image center must be finite"
        );
        ensure!(
            self.width.is_finite() && (8.0..=4096.0).contains(&self.width),
            "image width must be between 8 and 4096"
        );
        ensure!(
            self.radius.is_finite() && self.radius >= 0.0,
            "image radius must be finite and non-negative"
        );
        if let Some(title) = &self.title {
            ensure!(self.framed, "only framed images have a title bar");
            ensure!(
                !title.trim().is_empty() && title.chars().count() <= 80 && !title.contains('\n'),
                "image titles are one line of 1 to 80 characters"
            );
        }
        anchor::validate("image", &self.anchors)
    }
}

/// An image placement available from `from` to `until` on the plan clock.
/// Images hold one frame, so the placement only names the file and its span.
pub fn media(
    id: impl Into<String>,
    path: impl Into<std::path::PathBuf>,
    from: u64,
    until: u64,
) -> MediaPlan {
    MediaPlan {
        id: id.into(),
        path: path.into(),
        kind: MediaKindPlan::Image,
        role: MediaRolePlan::Layer,
        source_start_nanos: 0,
        source_end_nanos: until.saturating_sub(from),
        timeline_start_nanos: from,
        timeline_end_nanos: until,
        gain_db: 0.0,
    }
}

/// Authoring handle for one image actor.
#[derive(Clone, Debug)]
pub struct ImageActor {
    actor: ActorHandle,
    anchors: Vec<String>,
}

impl ImageActor {
    /// Declare the image and the placement it draws.
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: &ImagePlan,
        media: MediaPlan,
    ) -> Result<Self> {
        plan.validate()?;
        ensure!(
            matches!(media.kind, MediaKindPlan::Image) && media.id == plan.media_id,
            "an image draws the image media '{}'",
            plan.media_id
        );
        let actor = scene.actor(id, IMAGE_RECIPE, plan)?;
        scene.media(media);
        Ok(Self {
            actor,
            anchors: anchor::ids(&plan.anchors),
        })
    }

    pub fn actor(&self) -> &ActorHandle {
        &self.actor
    }

    pub fn id(&self) -> &str {
        self.actor.id()
    }

    pub fn channel(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        scene.channel(&self.actor, property, initial)
    }

    /// Fly the image up into place, as a Video Card does: it starts low,
    /// small, tipped back, and soft, and lands on critically damped springs.
    pub fn fly_in(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        for (property, from, rest, response) in [
            ("y", 160.0, 0.0, 0.8),
            ("scale", 0.86, 1.0, 0.8),
            ("tilt-x", 0.42, 0.0, 0.85),
            ("blur", 9.0, 0.0, 0.7),
            ("opacity", 0.0, 1.0, 0.35),
        ] {
            let channel = self.channel(scene, property, from);
            scene.spring(&channel, at_nanos, rest, response, 0.0);
        }
    }

    /// Fade out in place.
    pub fn hide(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        let opacity = self.channel(scene, "opacity", 1.0);
        scene.spring(&opacity, at_nanos, 0.0, 0.3, 0.0);
    }

    /// Glide to the anchor `to`, carrying velocity through interruptions.
    pub fn move_to(&mut self, scene: &mut PlanBuilder, to: &str, at_nanos: u64) -> Result<()> {
        anchor::move_to(scene, &self.actor, &self.anchors, to, at_nanos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::Edge;

    #[test]
    fn images_follow_their_aspect_and_round_trip_compactly() {
        let plan = ImagePlan::new("shot", [960.0, 540.0], 600.0);
        assert_eq!(plan.card_size([1200, 800]), [600.0, 400.0]);
        assert_eq!(
            serde_json::to_value(&plan).unwrap(),
            serde_json::json!({"mediaId": "shot", "center": [960.0, 540.0], "width": 600.0})
        );
        let titled = plan.titled("diagram.png");
        assert_eq!(titled.card_size([1200, 800]), [600.0, 400.0 + TITLE_BAR]);
        let json = serde_json::to_value(&titled).unwrap();
        assert_eq!(json["framed"], true);
        assert_eq!(serde_json::from_value::<ImagePlan>(json).unwrap(), titled);
    }

    #[test]
    fn invalid_images_are_rejected() {
        let plan = ImagePlan::new("shot", [960.0, 540.0], 600.0);
        assert!(
            ImagePlan {
                width: 2.0,
                ..plan.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            ImagePlan {
                title: Some("bare".into()),
                ..plan.clone()
            }
            .validate()
            .is_err()
        );
        assert!(plan.clone().rounded(-1.0).validate().is_err());
        assert!(!plan.accepts("focus-x") && plan.accepts("tilt-x"));
    }

    #[test]
    fn declare_adds_the_image_and_its_placement() {
        let mut scene = PlanBuilder::new("image", 4_000_000_000);
        let plan = ImagePlan::new("shot", [960.0, 540.0], 600.0)
            .framed()
            .anchor(AnchorPlan::stage("card", "card", Edge::Top).with_offset([0.0, -200.0]));
        let mut image = ImageActor::declare(
            &mut scene,
            "image",
            &plan,
            media("shot", "shot.png", 0, 4_000_000_000),
        )
        .unwrap();
        image.fly_in(&mut scene, 500_000_000);
        image.hide(&mut scene, 3_000_000_000);
        image.move_to(&mut scene, "card", 1_000_000_000).unwrap();
        let built = scene.finish().unwrap();
        assert!(matches!(built.media[0].kind, MediaKindPlan::Image));
        assert!(
            built
                .continuous_channels
                .iter()
                .all(|channel| plan.accepts(&channel.property))
        );
        let mut scene = PlanBuilder::new("image", 4_000_000_000);
        let mut video = media("shot", "shot.mp4", 0, 1);
        video.kind = MediaKindPlan::Video;
        assert!(ImageActor::declare(&mut scene, "image", &plan, video).is_err());
    }
}
