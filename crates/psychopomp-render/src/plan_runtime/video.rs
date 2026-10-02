//! Prepared video cards: the recipe and its placement are checked without a
//! GPU or decoder, then each card opens its own seekable RGBA frame cache.
use std::{
    cell::RefCell,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::Path,
};

use anyhow::{Context, Result, bail};
use psychopomp::{
    plan::{ActorPlan, ContinuousChannelPlan, MediaKindPlan, MediaPlan},
    video::{VIDEO_CHANNELS, VideoPlan},
};

use super::preflight::{decode, strict_channels};
use crate::{
    render::{HeadlessRenderer, VideoPose},
    video::VideoFrameCache,
};

pub(super) struct VideoInput {
    id: String,
    plan: VideoPlan,
    media: MediaPlan,
}

pub(super) struct PreparedVideo {
    id: String,
    plan: VideoPlan,
    media: MediaPlan,
    cache: RefCell<VideoFrameCache>,
}

impl VideoInput {
    pub(super) fn new(
        actor: &ActorPlan,
        media: &[MediaPlan],
        channels: &[ContinuousChannelPlan],
    ) -> Result<Self> {
        let plan = decode(actor, "video", VideoPlan::validate)?;
        strict_channels(&actor.id, channels, "video", |property| {
            VIDEO_CHANNELS.contains(&property)
        })?;
        let placement = media
            .iter()
            .find(|media| media.id == plan.media_id)
            .with_context(|| {
                format!(
                    "video actor '{}' references unknown media '{}'",
                    actor.id, plan.media_id
                )
            })?;
        if !matches!(placement.kind, MediaKindPlan::Video) {
            bail!("video actor '{}' must reference video media", actor.id);
        }
        Ok(Self {
            id: actor.id.clone(),
            media: placement.clone(),
            plan,
        })
    }

    pub(super) fn media_id(&self) -> &str {
        &self.media.id
    }

    pub(super) fn open(self, base: &Path) -> Result<PreparedVideo> {
        let source = super::resolve_media_path(base, &self.media);
        let cache = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("target")
            .join("psychopomp-cache")
            .join(cache_directory_name(&self.id, &self.media.id));
        let cache = VideoFrameCache::open(
            source,
            cache,
            self.plan.size[0],
            self.plan.size[1],
            self.plan.fps,
        )?;
        Ok(PreparedVideo {
            id: self.id,
            plan: self.plan,
            media: self.media,
            cache: RefCell::new(cache),
        })
    }
}

impl PreparedVideo {
    /// The decoded source frame shown at plan time `time`.
    pub(super) fn frame_index_at(&self, time: f64) -> u64 {
        let global_nanos = (time * 1_000_000_000.0).round() as u64;
        let local_nanos = source_nanos_at(&self.media, global_nanos);
        self.cache
            .borrow()
            .frame_index_at(local_nanos as f32 / 1_000_000_000.0)
    }

    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &mut HeadlessRenderer,
        time: f64,
        sample: impl Fn(&str, &str, f32) -> f32,
    ) -> Result<()> {
        let value = |property: &str, default: f32| sample(&self.id, property, default);
        let pose = VideoPose {
            offset: [value("x", 0.0), value("y", 0.0)],
            scale: value("scale", 1.0),
            opacity: value("opacity", 1.0).clamp(0.0, 1.0),
            rotation: value("rotation", 0.0),
            tilt: [value("tilt-x", 0.0), value("tilt-y", 0.0)],
            blur: value("blur", 0.0).max(0.0),
            view: self.plan.view(
                [value("focus-x", 0.5), value("focus-y", 0.5)],
                value("focus-size", 1.0),
            ),
        };
        if pose.opacity <= 0.001 {
            return Ok(());
        }
        let index = self.frame_index_at(time);
        let mut cache = self.cache.borrow_mut();
        let size = cache.size();
        let frame = cache.frame_at_index(index)?;
        renderer.composite_video(pixels, &self.plan, frame, size, pose)
    }
}

fn cache_directory_name(actor_id: &str, media_id: &str) -> String {
    let mut hasher = DefaultHasher::new();
    actor_id.hash(&mut hasher);
    media_id.hash(&mut hasher);
    format!("plan-{:016x}", hasher.finish())
}

/// Source time for a global plan time: footage holds its first frame before
/// the placement starts and its last after it ends.
fn source_nanos_at(media: &MediaPlan, global_nanos: u64) -> u64 {
    let offset = global_nanos
        .saturating_sub(media.timeline_start_nanos)
        .min(media.timeline_end_nanos - media.timeline_start_nanos);
    media.source_start_nanos + offset
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use psychopomp::plan::{MediaKindPlan, MediaPlan, MediaRolePlan};

    use super::{cache_directory_name, source_nanos_at};

    #[test]
    fn cache_directory_does_not_expose_plan_ids_as_path_components() {
        let directory = cache_directory_name("../../actor", "../recording/video");

        assert!(directory.starts_with("plan-"));
        assert!(!directory.contains('/'));
        assert!(!directory.contains(".."));
    }

    #[test]
    fn global_time_maps_to_trimmed_video_source_time() {
        let media = MediaPlan {
            id: "recording".to_owned(),
            path: PathBuf::from("recording.mp4"),
            kind: MediaKindPlan::Video,
            role: MediaRolePlan::Layer,
            source_start_nanos: 2_000_000_000,
            source_end_nanos: 5_000_000_000,
            timeline_start_nanos: 10_000_000_000,
            timeline_end_nanos: 13_000_000_000,
            gain_db: 0.0,
        };

        assert_eq!(source_nanos_at(&media, 9_000_000_000), 2_000_000_000);
        assert_eq!(source_nanos_at(&media, 10_000_000_000), 2_000_000_000);
        assert_eq!(source_nanos_at(&media, 11_500_000_000), 3_500_000_000);
        assert_eq!(source_nanos_at(&media, 13_000_000_000), 5_000_000_000);
        assert_eq!(source_nanos_at(&media, 20_000_000_000), 5_000_000_000);
    }
}
