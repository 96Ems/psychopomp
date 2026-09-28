use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use kinograph::math::smoothstep;
use serde::Deserialize;

use kinograph::{
    composition::{Asset, Clip, Composition, Time, TimeRange},
    dsl::{Scalar, Scene},
    motion::{MotionState, Spring},
    timeline::PropertyId,
};

use crate::{
    render::{
        CommandFileFrame, HeadlessRenderer, RenderSpec, TerminalBackground, TerminalSceneFrame,
    },
    video::VideoFrameCache,
};

use super::{FONT_PATH, HEIGHT, WIDTH, WORKSPACE_ROOT, encode_video_with_samples};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenCodeCapture {
    viewport: OpenCodeCaptureViewport,
    fps: u32,
    cues_seconds: OpenCodeCaptureCues,
}

#[derive(Deserialize)]
struct OpenCodeCaptureViewport {
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenCodeCaptureCues {
    typing_start: f64,
    command_created: f64,
    command_live: f64,
    command_completed: f64,
    command_submitted: f64,
    response_complete: f64,
}

pub(crate) async fn render(output: &Path) -> Result<()> {
    const DURATION: f64 = 10.0;

    let asset_directory = Path::new(WORKSPACE_ROOT)
        .join("assets")
        .join("opencode-hot-reload");
    let capture: OpenCodeCapture = serde_json::from_slice(
        &fs::read(asset_directory.join("capture.json"))
            .context("read OpenCode capture metadata")?,
    )
    .context("parse OpenCode capture metadata")?;
    let audio = |id: &str, file: &str, duration: f64, gain_db: f32| {
        Asset::audio(id, asset_directory.join(file))
            .clip(TimeRange::new(Time::ZERO, Time::seconds(duration)))
            .gain_db(gain_db)
    };
    let voiceover = audio("opencode-voiceover", "voiceover.wav", 8.359_188, -2.0);
    let typing = audio("opencode-typing", "typing.wav", 0.56, 0.0);
    let save = audio("opencode-command-save", "save.wav", 0.16, 4.0);
    let launch = audio("opencode-missile-launch", "launch.wav", 1.25, 8.0);
    let impact = audio("opencode-missile-impact", "impact.wav", 0.52, 4.0);
    let confirm = audio("opencode-command-confirm", "confirm.wav", 0.34, 6.0);
    let sound_at = |seconds: f64, clip: Clip| {
        Composition::delay(
            kinograph::composition::Duration::seconds(seconds),
            Composition::layer(clip),
        )
    };
    let composition = Composition::parallel([
        Composition::hold(kinograph::composition::Duration::seconds(DURATION)),
        Composition::script(voiceover),
        sound_at(capture.cues_seconds.typing_start, typing),
        sound_at(capture.cues_seconds.command_created, save),
        sound_at(capture.cues_seconds.command_submitted, launch),
        sound_at(
            capture.cues_seconds.command_submitted + 0.92,
            impact.clone(),
        ),
        sound_at(capture.cues_seconds.command_submitted + 1.04, impact),
        sound_at(capture.cues_seconds.response_complete, confirm),
    ]);
    let scene =
        Scene::new(Vec::<(PropertyId, Scalar)>::new(), composition).compile(&HashMap::new())?;
    let mut recording = VideoFrameCache::open(
        asset_directory.join("fire-the-missiles.mp4"),
        Path::new(WORKSPACE_ROOT)
            .join("target")
            .join("kinograph-cache")
            .join("fire-the-missiles"),
        capture.viewport.width,
        capture.viewport.height,
        capture.fps,
    )?;
    let mut renderer = HeadlessRenderer::new(RenderSpec {
        width: WIDTH,
        height: HEIGHT,
        font_path: PathBuf::from(FONT_PATH),
        file_name: "opencode-v2".to_owned(),
    })
    .await?;
    let source_size = recording.size();

    encode_video_with_samples(
        &mut renderer,
        output,
        &scene,
        4,
        8,
        &[0.0..1.0, 2.2..4.55, 4.75..6.25],
        |renderer, time| {
            let source_pixels = recording.frame_at(time)?;
            renderer.render_terminal_scene(&opencode_hot_reload_frame(
                time,
                source_pixels,
                source_size,
                &capture.cues_seconds,
            ))
        },
    )
}

fn opencode_hot_reload_frame<'a>(
    time: f32,
    source_pixels: &'a [u8],
    source_size: [u32; 2],
    cues: &OpenCodeCaptureCues,
) -> TerminalSceneFrame<'a> {
    let command_created = cues.command_created as f32;
    let command_live = cues.command_live as f32;
    let command_completed = cues.command_completed as f32;
    let command_submitted = cues.command_submitted as f32;
    let response_complete = cues.response_complete as f32;

    let enter = spring_progress(time, 0.55, 0.88);
    let zoom_in = spring_progress(time - 0.72, 0.62, 0.9).clamp(0.0, 1.0);
    let zoom_out = spring_progress(time - command_submitted, 0.66, 0.9).clamp(0.0, 1.0);
    let focus = zoom_in * (1.0 - zoom_out);
    let split_in = spring_progress(time - (command_created - 0.74), 0.48, 0.88).clamp(0.0, 1.0);
    let split_exit_at = command_live + (command_completed - command_live) * 0.5;
    let split_out = spring_progress(time - split_exit_at, 0.48, 0.9).clamp(0.0, 1.0);
    let split = split_in * (1.0 - split_out);
    let write_progress = smoothstep((time - (command_created - 0.68)) / 0.62);
    let submit_scale = if time < command_submitted {
        1.0
    } else if time < command_submitted + 0.11 {
        1.0 - 0.018 * smoothstep((time - command_submitted) / 0.11)
    } else {
        0.982 + 0.018 * spring_progress(time - (command_submitted + 0.11), 0.28, 0.72)
    };
    let tagline_opacity = smoothstep((time - (response_complete + 0.45)) / 0.5);

    TerminalSceneFrame {
        source_pixels,
        source_size,
        background: TerminalBackground::Aurora,
        panel_center: [
            960.0 + (570.0 - 960.0) * split,
            520.0 + (1.0 - enter) * 96.0 + (500.0 - 520.0) * focus + 40.0 * split,
        ],
        panel_scale: (0.9 + 0.1 * enter)
            * ((1.0 + 0.22 * focus) + (0.74 - (1.0 + 0.22 * focus)) * split)
            * submit_scale,
        panel_rotation: (1.0 - enter) * -0.035 + focus * 0.014 + (-0.018 - focus * 0.014) * split,
        panel_tilt_x: 0.0,
        panel_tilt_y: 0.0,
        panel_near_blur: 0.0,
        command_file: (split > 0.001).then_some(CommandFileFrame {
            enter: split_in,
            opacity: split,
            write_progress,
            saved: time >= command_created,
        }),
        missile_age: (command_submitted..command_submitted + 2.0)
            .contains(&time)
            .then_some(time - command_submitted),
        tagline_opacity: tagline_opacity * smoothstep((time - 7.05) / 0.45),
    }
}

fn spring_progress(age: f32, response: f32, damping: f32) -> f32 {
    if age <= 0.0 {
        return 0.0;
    }
    Spring::new(response, damping)
        .sample(MotionState::at(0.0), 1.0, age)
        .position
}
