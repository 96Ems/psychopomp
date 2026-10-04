//! Prepared images: the recipe, its channels, and its image placement are
//! checked without reading the file; preparation decodes it once.
use std::path::Path;

use anyhow::{Context, Result, bail};
use psychopomp::{
    anchor::AnchorPlan,
    image::ImagePlan,
    math::Vec2,
    plan::{ActorPlan, ContinuousChannelPlan, MediaKindPlan, MediaPlan},
};

use super::preflight::{decode, strict_channels};
use crate::render::{DecodedImage, HeadlessRenderer, ImagePose, decode_image};

pub(super) struct ImageInput {
    id: String,
    plan: ImagePlan,
    media: MediaPlan,
}

pub(super) struct PreparedImage {
    id: String,
    plan: ImagePlan,
    image: DecodedImage,
}

impl ImageInput {
    pub(super) fn new(
        actor: &ActorPlan,
        media: &[MediaPlan],
        channels: &[ContinuousChannelPlan],
    ) -> Result<Self> {
        let plan = decode(actor, "image", ImagePlan::validate)?;
        strict_channels(&actor.id, channels, "image", |property| {
            plan.accepts(property)
        })?;
        let placement = media
            .iter()
            .find(|media| media.id == plan.media_id)
            .with_context(|| {
                format!(
                    "image actor '{}' references unknown media '{}'",
                    actor.id, plan.media_id
                )
            })?;
        if !matches!(placement.kind, MediaKindPlan::Image) {
            bail!("image actor '{}' must reference image media", actor.id);
        }
        Ok(Self {
            id: actor.id.clone(),
            media: placement.clone(),
            plan,
        })
    }

    pub(super) fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn media_id(&self) -> &str {
        &self.media.id
    }

    pub(super) fn anchors(&self) -> &[AnchorPlan] {
        &self.plan.anchors
    }

    pub(super) fn open(self, base: &Path) -> Result<PreparedImage> {
        let path = super::resolve_media_path(base, &self.media);
        let bytes =
            std::fs::read(&path).with_context(|| format!("read image {}", path.display()))?;
        let image = decode_image(&bytes)
            .with_context(|| format!("decode image {}", path.display()))?
            .reduced_for(self.plan.width);
        Ok(PreparedImage {
            id: self.id,
            plan: self.plan,
            image,
        })
    }
}

impl PreparedImage {
    pub(super) fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn anchors(&self) -> &[AnchorPlan] {
        &self.plan.anchors
    }

    /// The literal center, used while the image has no anchors.
    pub(super) fn center(&self) -> Vec2 {
        Vec2::from(self.plan.center)
    }

    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &mut HeadlessRenderer,
        center: Vec2,
        sample: impl Fn(&str, &str, f32) -> f32,
    ) -> Result<()> {
        let value = |property: &str, default: f32| sample(&self.id, property, default);
        let pose = ImagePose {
            center: [center.x + value("x", 0.0), center.y + value("y", 0.0)],
            scale: value("scale", 1.0),
            opacity: value("opacity", 1.0).clamp(0.0, 1.0),
            rotation: value("rotation", 0.0),
            tilt: [value("tilt-x", 0.0), value("tilt-y", 0.0)],
            blur: value("blur", 0.0),
        };
        renderer.composite_image(pixels, &self.plan, &self.image, pose)
    }
}

#[cfg(test)]
mod tests {
    use psychopomp::{
        anchor::{AnchorPlan, Edge},
        author::PlanBuilder,
        image::{self, ImageActor, ImagePlan},
    };

    use crate::plan_runtime::validate_renderer_plan;

    fn plan(
        anchor: Option<AnchorPlan>,
        property: &str,
        kind: psychopomp::plan::MediaKindPlan,
    ) -> psychopomp::plan::ScenePlan {
        let mut scene = PlanBuilder::new("image", 2_000_000_000);
        let mut recipe = ImagePlan::new("shot", [960.0, 540.0], 400.0).framed();
        if let Some(anchor) = anchor {
            recipe = recipe.anchor(anchor);
        }
        let media = image::media("shot", "never-opened.png", 0, 2_000_000_000);
        let mut actor = ImageActor::declare(&mut scene, "shot", &recipe, media).unwrap();
        let channel = actor.channel(&mut scene, property, 0.0);
        scene.set(&channel, 100_000_000, 1.0);
        let mut plan = scene.finish().unwrap();
        plan.media[0].kind = kind;
        plan
    }

    #[test]
    fn image_preflight_consumes_its_media_and_checks_channels_without_reading_it() {
        use psychopomp::plan::MediaKindPlan::{Image, Video};
        validate_renderer_plan(&plan(None, "tilt-x", Image)).unwrap();
        validate_renderer_plan(&plan(
            Some(AnchorPlan::point("here", [100.0, 100.0])),
            "anchor.here",
            Image,
        ))
        .unwrap();
        for (anchor, property, kind, expected) in [
            (None, "focus-x", Image, "unknown property"),
            (None, "opacity", Video, "must reference image media"),
            (
                Some(AnchorPlan::stage("card", "card", Edge::Top)),
                "opacity",
                Image,
                "needs a stage root",
            ),
        ] {
            let error = validate_renderer_plan(&plan(anchor, property, kind)).unwrap_err();
            assert!(format!("{error:#}").contains(expected), "{error:#}");
        }
    }
}
