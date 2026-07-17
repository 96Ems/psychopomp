use std::{
    cell::RefCell,
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    path::Path,
};

use anyhow::{Context, Result, bail};
use kinograph::{
    plan::{ActorPlan, MediaKindPlan, MediaPlan},
    terminal::TerminalRecordingRecipePlan,
};

use crate::{
    render::{HeadlessRenderer, TerminalBackground, TerminalSceneFrame},
    video::VideoFrameCache,
};

pub(super) struct PreparedTerminal {
    actor_id: String,
    file_name: String,
    recordings: HashMap<String, PreparedRecording>,
}

struct PreparedRecording {
    media: MediaPlan,
    cache: RefCell<VideoFrameCache>,
}

impl PreparedTerminal {
    pub(super) fn new(actor: &ActorPlan, media: &[MediaPlan], base: &Path) -> Result<Self> {
        let recipe = serde_json::from_value::<TerminalRecordingRecipePlan>(actor.data.clone())
            .with_context(|| format!("parse terminal recording recipe for actor '{}'", actor.id))?;
        if recipe.recordings.is_empty() {
            bail!(
                "terminal recording actor '{}' requires at least one recording",
                actor.id
            );
        }
        let mut recordings = HashMap::new();
        for recording in recipe.recordings {
            if recordings.contains_key(&recording.media_id) {
                bail!(
                    "terminal actor '{}' declares recording '{}' more than once",
                    actor.id,
                    recording.media_id
                );
            }
            if recording.width == 0 || recording.height == 0 || recording.fps == 0 {
                bail!(
                    "terminal recording '{}' dimensions and fps must be non-zero",
                    recording.media_id
                );
            }
            let planned = media
                .iter()
                .find(|media| media.id == recording.media_id)
                .with_context(|| {
                    format!(
                        "terminal recording actor '{}' references unknown media '{}'",
                        actor.id, recording.media_id
                    )
                })?;
            if !matches!(planned.kind, MediaKindPlan::Video) {
                bail!(
                    "terminal recording '{}' must reference video media",
                    recording.media_id
                );
            }
            let source = super::resolve_media_path(base, planned);
            let cache = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("target")
                .join("kinograph-cache")
                .join(cache_directory_name(&actor.id, &recording.media_id));
            let cache = VideoFrameCache::open(
                source,
                cache,
                recording.width,
                recording.height,
                recording.fps,
            )?;
            recordings.insert(
                recording.media_id.clone(),
                PreparedRecording {
                    media: planned.clone(),
                    cache: RefCell::new(cache),
                },
            );
        }
        Ok(Self {
            actor_id: actor.id.clone(),
            file_name: recipe.file_name,
            recordings,
        })
    }

    pub(super) fn actor_id(&self) -> &str {
        &self.actor_id
    }

    pub(super) fn file_name(&self) -> &str {
        &self.file_name
    }

    pub(super) fn media_ids(&self) -> impl Iterator<Item = &str> {
        self.recordings.keys().map(String::as_str)
    }

    pub(super) fn render(
        &self,
        renderer: &mut HeadlessRenderer,
        recording_id: &str,
        time: f64,
        value: impl Fn(&str, f32) -> f32,
    ) -> Result<Vec<u8>> {
        let recording = self.recordings.get(recording_id).with_context(|| {
            format!(
                "terminal actor '{}' selected unknown recording '{}'",
                self.actor_id, recording_id
            )
        })?;
        let global_nanos = (time * 1_000_000_000.0).round() as u64;
        let local_nanos = source_nanos_at(&recording.media, global_nanos);
        let mut cache = recording.cache.borrow_mut();
        let source_size = cache.size();
        let source_pixels = cache.frame_at(local_nanos as f32 / 1_000_000_000.0)?;
        renderer.render_terminal_scene(&TerminalSceneFrame {
            source_pixels,
            source_size,
            background: TerminalBackground::Neutral,
            panel_center: [value("panel-x", 960.0), value("panel-y", 540.0)],
            panel_scale: value("panel-scale", 0.9),
            panel_rotation: value("panel-rotation", 0.0),
            panel_tilt_x: value("panel-tilt-x", 0.0),
            panel_tilt_y: value("panel-tilt-y", 0.0),
            panel_near_blur: value("panel-near-blur", 0.0),
            command_file: None,
            missile_age: None,
            tagline_opacity: 0.0,
        })
    }

    pub(super) fn frame_index_at(&self, recording_id: &str, time: f64) -> Result<u64> {
        let recording = self.recordings.get(recording_id).with_context(|| {
            format!(
                "terminal actor '{}' selected unknown recording '{}'",
                self.actor_id, recording_id
            )
        })?;
        let global_nanos = (time * 1_000_000_000.0).round() as u64;
        let local_nanos = source_nanos_at(&recording.media, global_nanos);
        Ok(recording
            .cache
            .borrow()
            .frame_index_at(local_nanos as f32 / 1_000_000_000.0))
    }
}

fn cache_directory_name(actor_id: &str, media_id: &str) -> String {
    let mut hasher = DefaultHasher::new();
    actor_id.hash(&mut hasher);
    media_id.hash(&mut hasher);
    format!("plan-{:016x}", hasher.finish())
}

fn source_nanos_at(media: &MediaPlan, global_nanos: u64) -> u64 {
    let offset = global_nanos
        .saturating_sub(media.timeline_start_nanos)
        .min(media.timeline_end_nanos - media.timeline_start_nanos);
    media.source_start_nanos + offset
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use kinograph::plan::{MediaKindPlan, MediaPlan, MediaRolePlan};

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
