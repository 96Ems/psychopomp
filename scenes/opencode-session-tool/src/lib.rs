use std::path::PathBuf;

use anyhow::Result;
use kinograph::{
    author::PlanBuilder,
    plan::{MediaKindPlan, MediaPlan, MediaRolePlan, ScenePlan},
    terminal::{TERMINAL_RECORDING_RECIPE, TerminalRecordingPlan, TerminalRecordingRecipePlan},
};
use serde_json::json;

const DURATION: u64 = 15_500_000_000;
const RECORDING_START: u64 = 600_000_000;
const RECORDING_DURATION: u64 = 12_280_000_000;

pub fn build_plan() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("opencode-v2-session-tool", DURATION);
    let terminal = scene.actor(
        "terminal",
        TERMINAL_RECORDING_RECIPE,
        TerminalRecordingRecipePlan {
            file_name: "OpenCode v2".to_owned(),
            recordings: vec![TerminalRecordingPlan {
                media_id: "live-hot-reload".to_owned(),
                width: 1120,
                height: 640,
                fps: 25,
            }],
        },
    )?;
    scene.state(&terminal, "recording", "live-hot-reload")?;

    let panel_y = scene.continuous(&terminal, "panel-y", 500.0);
    let panel_scale = scene.continuous(&terminal, "panel-scale", 1.18);
    let panel_rotation = scene.continuous(&terminal, "panel-rotation", -0.06);
    let panel_tilt_x = scene.continuous(&terminal, "panel-tilt-x", -0.24);
    let panel_tilt_y = scene.continuous(&terminal, "panel-tilt-y", 0.34);
    let panel_near_blur = scene.continuous(&terminal, "panel-near-blur", 11.0);
    scene.spring(&panel_y, 0, 560.0, 0.7, 0.0);
    scene.spring(&panel_scale, 0, 0.86, 0.7, 0.0);
    scene.spring(&panel_rotation, 0, 0.0, 0.7, 0.0);
    scene.spring(&panel_tilt_x, 0, 0.0, 0.7, 0.0);
    scene.spring(&panel_tilt_y, 0, 0.0, 0.7, 0.0);
    scene.spring(&panel_near_blur, 0, 0.0, 0.7, 0.0);

    let beats = [
        (
            "before-tool",
            "THE TOOL DOES NOT EXIST YET",
            "provider tools before patch  /  session_greeting absent",
            0,
            3_400_000_000,
        ),
        (
            "create-plugin",
            "THE SESSION CREATES A V2 PLUGIN",
            "patch creates .opencode/plugins/session-tool.ts inside the live conversation",
            3_400_000_000,
            7_000_000_000,
        ),
        (
            "hot-reload",
            "THE WATCHER LOADS IT. NO RESTART.",
            "config.updated  /  plugin loaded  /  the TUI and session stay alive",
            7_000_000_000,
            10_400_000_000,
        ),
        (
            "same-session-call",
            "THE SAME SESSION CALLS session_greeting",
            "same session ID  /  session.tool.called  /  session.tool.success",
            10_400_000_000,
            12_400_000_000,
        ),
        (
            "proof",
            "HOT RELOAD WORKED. HELLO, ADA.",
            "one interactive session  /  one live reload  /  real tool result",
            12_400_000_000,
            DURATION,
        ),
    ];
    for (index, (id, heading, detail, start, end)) in beats.into_iter().enumerate() {
        let heading_actor = scene.actor(
            format!("heading-{id}"),
            "text",
            json!({
                "text": heading,
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
            format!("detail-{id}"),
            "text",
            json!({
                "text": detail,
                "center": [960, 1020],
                "fontSize": 22,
                "color": [139, 166, 199],
            }),
        )?;
        let detail_opacity =
            scene.continuous(&detail_actor, "opacity", if index == 0 { 1.0 } else { 0.0 });
        if start > 0 {
            scene.spring(&heading_opacity, start, 1.0, 0.32, 0.0);
            scene.spring(&detail_opacity, start, 1.0, 0.32, 0.0);
        }
        if end < DURATION {
            scene.spring(&heading_opacity, end, 0.0, 0.32, 0.0);
            scene.spring(&detail_opacity, end, 0.0, 0.32, 0.0);
        }
        scene.cue(id, start, end);
    }

    scene.media(video(
        "live-hot-reload",
        "../../assets/opencode-v2-session-tool/live-hot-reload.mp4",
        RECORDING_DURATION,
        RECORDING_START,
    ));
    for (id, file, duration, start) in [
        (
            "voice-setup",
            "voice-01-setup.mp3",
            2_229_116_000,
            500_000_000,
        ),
        (
            "voice-create",
            "voice-02-create.mp3",
            3_018_594_000,
            3_400_000_000,
        ),
        (
            "voice-reload",
            "voice-03-reload.mp3",
            3_390_113_000,
            7_000_000_000,
        ),
        (
            "voice-call",
            "voice-04-call.mp3",
            1_950_476_000,
            10_400_000_000,
        ),
        (
            "voice-proof",
            "voice-05-proof.mp3",
            2_507_755_000,
            12_400_000_000,
        ),
    ] {
        scene.media(audio(
            id,
            &format!("../../assets/opencode-v2-session-tool/{file}"),
            duration,
            start,
            MediaRolePlan::Script,
            6.0,
        ));
    }
    scene.media(audio(
        "plugin-loaded",
        "../../assets/opencode-hot-reload/save.wav",
        160_000_000,
        7_000_000_000,
        MediaRolePlan::Layer,
        2.0,
    ));
    scene.media(audio(
        "tool-success",
        "../../assets/opencode-hot-reload/confirm.wav",
        340_000_000,
        12_400_000_000,
        MediaRolePlan::Layer,
        4.0,
    ));

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
    use kinograph::plan::{MediaRolePlan, TrackEventPlan};

    use super::build_plan;

    const CANONICAL_PLAN: &str = include_str!("../opencode-session-tool.plan.json");

    #[test]
    fn scene_plan_owns_video_audio_state_and_cues() {
        let plan = build_plan().unwrap();
        assert_eq!(plan.duration_nanos, 15_500_000_000);
        assert_eq!(plan.media.len(), 8);
        assert_eq!(plan.state_channels.len(), 1);
        assert_eq!(plan.cues.len(), 5);
    }

    #[test]
    fn narration_beats_do_not_leave_a_dead_interval() {
        let plan = build_plan().unwrap();
        let mut script = plan
            .media
            .iter()
            .filter(|media| matches!(media.role, MediaRolePlan::Script))
            .collect::<Vec<_>>();
        script.sort_by_key(|media| media.timeline_start_nanos);

        for pair in script.windows(2) {
            let gap = pair[1]
                .timeline_start_nanos
                .saturating_sub(pair[0].timeline_end_nanos);
            assert!(
                gap <= 1_000_000_000,
                "narration gap between {} and {} is {:.3}s",
                pair[0].id,
                pair[1].id,
                gap as f64 / 1_000_000_000.0,
            );
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
