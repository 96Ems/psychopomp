use std::path::PathBuf;

use anyhow::Result;
use psychopomp::{
    author::PlanBuilder,
    plan::{MediaKindPlan, MediaPlan, MediaRolePlan, ScenePlan},
    terminal::{TERMINAL_RECORDING_RECIPE, TerminalRecordingPlan, TerminalRecordingRecipePlan},
};
use serde_json::json;

const DURATION: u64 = 20_500_000_000;
const RECORDING_START: u64 = 600_000_000;
const RECORDING_DURATION: u64 = 19_216_667_000;

#[derive(Clone, Copy)]
struct AudioBeat {
    id: &'static str,
    path: &'static str,
    duration: u64,
    gain_db: f32,
}

#[derive(Clone, Copy)]
struct Beat {
    id: &'static str,
    heading: &'static str,
    detail: &'static str,
    start: u64,
    end: u64,
    sound: AudioBeat,
}

const SAVE: AudioBeat = AudioBeat {
    id: "save",
    path: "../../assets/opencode-hot-reload/save.wav",
    duration: 160_000_000,
    gain_db: -2.0,
};

const BEATS: [Beat; 9] = [
    Beat {
        id: "command",
        heading: "COMMAND ADDED LIVE",
        detail: "opencode.jsonc saved  /  slash command appears  /  no restart",
        start: 0,
        end: 2_800_000_000,
        sound: SAVE,
    },
    Beat {
        id: "agent",
        heading: "AGENT REGISTRY RELOADED",
        detail: "release-sentinel enabled  /  mention menu refreshes in place",
        start: 2_800_000_000,
        end: 5_200_000_000,
        sound: SAVE,
    },
    Beat {
        id: "skill",
        heading: "PROJECT SKILL INVALIDATED",
        detail: "SKILL.md saved  /  release-protocol becomes slash-visible",
        start: 5_200_000_000,
        end: 7_400_000_000,
        sound: SAVE,
    },
    Beat {
        id: "reference",
        heading: "REFERENCE MATERIALIZED",
        detail: "runbook configured  /  @runbook appears in the live picker",
        start: 7_400_000_000,
        end: 9_600_000_000,
        sound: SAVE,
    },
    Beat {
        id: "model",
        heading: "MODEL CATALOG REFRESHED",
        detail: "Local Instant enabled  /  model selector updates without reconnecting",
        start: 9_600_000_000,
        end: 11_800_000_000,
        sound: SAVE,
    },
    Beat {
        id: "permission",
        heading: "PERMISSIONS APPLY NEXT STEP",
        detail: "read denied for build  /  the following provider request omits it",
        start: 11_800_000_000,
        end: 13_600_000_000,
        sound: SAVE,
    },
    Beat {
        id: "instructions",
        heading: "AMBIENT INSTRUCTIONS REPLACED",
        detail: "AGENTS.md changes RED to GREEN inside the same conversation",
        start: 13_600_000_000,
        end: 15_400_000_000,
        sound: SAVE,
    },
    Beat {
        id: "plugin-v1",
        heading: "PLUGIN GENERATION ONE",
        detail: "release_status appears and returns STAGING in the open session",
        start: 15_400_000_000,
        end: 17_300_000_000,
        sound: SAVE,
    },
    Beat {
        id: "plugin-v2",
        heading: "PLUGIN GENERATION TWO. SAME SESSION.",
        detail: "READY  /  deployment_url added  /  one service, one client, zero restarts",
        start: 17_300_000_000,
        end: DURATION,
        sound: AudioBeat {
            id: "confirm",
            path: "../../assets/opencode-hot-reload/confirm.wav",
            duration: 340_000_000,
            gain_db: 4.0,
        },
    },
];

pub fn build_plan() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("opencode-v2-session-tool", DURATION);
    let terminal = scene.actor(
        "terminal",
        TERMINAL_RECORDING_RECIPE,
        TerminalRecordingRecipePlan {
            file_name: "OpenCode v2 / Vim".to_owned(),
            recordings: vec![TerminalRecordingPlan {
                media_id: "live-hot-reload".to_owned(),
                width: 1920,
                height: 760,
                fps: 60,
            }],
        },
    )?;
    scene.state(&terminal, "recording", "live-hot-reload")?;

    let panel_y = scene.continuous(&terminal, "panel-y", 500.0);
    let panel_scale = scene.continuous(&terminal, "panel-scale", 1.35);
    let panel_rotation = scene.continuous(&terminal, "panel-rotation", -0.06);
    let panel_tilt_x = scene.continuous(&terminal, "panel-tilt-x", -0.24);
    let panel_tilt_y = scene.continuous(&terminal, "panel-tilt-y", 0.34);
    let panel_near_blur = scene.continuous(&terminal, "panel-near-blur", 11.0);
    scene.spring(&panel_y, 0, 540.0, 0.7, 0.0);
    scene.spring(&panel_scale, 0, 1.15, 0.7, 0.0);
    scene.spring(&panel_rotation, 0, 0.0, 0.7, 0.0);
    scene.spring(&panel_tilt_x, 0, 0.0, 0.7, 0.0);
    scene.spring(&panel_tilt_y, 0, 0.0, 0.7, 0.0);
    scene.spring(&panel_near_blur, 0, 0.0, 0.7, 0.0);

    for (index, beat) in BEATS.iter().enumerate() {
        let heading_actor = scene.actor(
            format!("heading-{}", beat.id),
            "text",
            json!({
                "text": beat.heading,
                "center": [960, 82],
                "fontSize": 34,
                "color": [235, 240, 246],
            }),
        )?;
        let heading_opacity = scene.continuous(
            &heading_actor,
            "opacity",
            if index == 0 { 1.0 } else { 0.0 },
        );
        let detail_actor = scene.actor(
            format!("detail-{}", beat.id),
            "text",
            json!({
                "text": beat.detail,
                "center": [960, 1020],
                "fontSize": 22,
                "color": [139, 166, 199],
            }),
        )?;
        let detail_opacity =
            scene.continuous(&detail_actor, "opacity", if index == 0 { 1.0 } else { 0.0 });
        if beat.start > 0 {
            scene.spring(&heading_opacity, beat.start, 1.0, 0.32, 0.0);
            scene.spring(&detail_opacity, beat.start, 1.0, 0.32, 0.0);
        }
        if beat.end < DURATION {
            scene.spring(&heading_opacity, beat.end, 0.0, 0.32, 0.0);
            scene.spring(&detail_opacity, beat.end, 0.0, 0.32, 0.0);
        }
        scene.cue(beat.id, beat.start, beat.end);
    }

    scene.media(video(
        "live-hot-reload",
        "../../assets/opencode-v2-session-tool/max-hot-reload-split.mp4",
        RECORDING_DURATION,
        RECORDING_START,
    ));
    for (index, beat) in BEATS.into_iter().enumerate() {
        scene.media(audio(
            &format!("{}-{index}", beat.sound.id),
            beat.sound.path,
            beat.sound.duration,
            beat.start,
            MediaRolePlan::Layer,
            beat.sound.gain_db,
        ));
    }

    Ok(scene.finish()?)
}

fn video(id: &str, path: &str, duration: u64, timeline_start: u64) -> MediaPlan {
    MediaPlan {
        id: id.to_owned(),
        path: PathBuf::from(path),
        kind: MediaKindPlan::Video,
        role: MediaRolePlan::Layer,
        source_start_nanos: 0,
        source_end_nanos: duration,
        timeline_start_nanos: timeline_start,
        timeline_end_nanos: timeline_start + duration,
        gain_db: 0.0,
    }
}

fn audio(
    id: &str,
    path: &str,
    duration: u64,
    timeline_start: u64,
    role: MediaRolePlan,
    gain_db: f32,
) -> MediaPlan {
    MediaPlan {
        id: id.to_owned(),
        path: PathBuf::from(path),
        kind: MediaKindPlan::Audio,
        role,
        source_start_nanos: 0,
        source_end_nanos: duration,
        timeline_start_nanos: timeline_start,
        timeline_end_nanos: timeline_start + duration,
        gain_db,
    }
}

#[cfg(test)]
mod tests {
    use psychopomp::plan::TrackEventPlan;

    use super::build_plan;

    const CANONICAL_PLAN: &str = include_str!("../opencode-session-tool.plan.json");

    #[test]
    fn scene_plan_owns_video_audio_state_and_cues() {
        let plan = build_plan().unwrap();
        assert_eq!(plan.duration_nanos, 20_500_000_000);
        assert_eq!(plan.media.len(), 10);
        assert_eq!(plan.state_channels.len(), 1);
        assert_eq!(plan.cues.len(), 9);
    }

    #[test]
    fn beats_cover_the_complete_scene_without_a_dead_interval() {
        let plan = build_plan().unwrap();
        let mut cues = plan.cues.iter().collect::<Vec<_>>();
        cues.sort_by_key(|cue| cue.start_nanos);

        assert_eq!(cues.first().unwrap().start_nanos, 0);
        assert_eq!(cues.last().unwrap().end_nanos, plan.duration_nanos);
        for pair in cues.windows(2) {
            assert_eq!(pair[0].end_nanos, pair[1].start_nanos);
        }
    }

    #[test]
    fn canonical_plan_matches_rust_scene_program() {
        assert_eq!(
            build_plan().unwrap().to_json_pretty().unwrap(),
            CANONICAL_PLAN
        );
    }

    #[test]
    fn panel_entrance_is_critically_damped() {
        let plan = build_plan().unwrap();
        for property in [
            "panel-y",
            "panel-scale",
            "panel-rotation",
            "panel-tilt-x",
            "panel-tilt-y",
            "panel-near-blur",
        ] {
            let channel = plan
                .continuous_channels
                .iter()
                .find(|channel| channel.actor_id == "terminal" && channel.property == property)
                .unwrap();
            let TrackEventPlan::Spring {
                at_nanos,
                response_seconds,
                damping_ratio,
                ..
            } = channel.events.first().unwrap()
            else {
                panic!("{property} entrance must be a spring");
            };
            assert_eq!(*at_nanos, 0);
            assert!((*response_seconds - 0.84).abs() < f32::EPSILON * 2.0);
            assert_eq!(*damping_ratio, 1.0);
        }
    }
}
