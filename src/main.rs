//! PROTOTYPE: stable code choreography rendered headlessly with wgpu.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use kinograph::{
    code::{
        CodeDocument, CodeLayout, CodeLine, CodeSnapshot, CodeTransition, PlacedLine, StyledSpan,
        SyntaxStyle, TransitionProgress,
    },
    composition::{Asset, Clip, Composition, Time, TimeRange},
    dsl::{
        Annotation, AnnotationEffect, Code, CompiledScene, Motion, Pointer, Scalar, Scene,
        TargetGeometry, Task,
    },
    encode::{FfmpegEncoder, VideoSpec},
    motion::{MotionState, Spring},
    render::{
        CommandFileFrame, EditorFrame, HeadlessRenderer, InlineRevealFrame, PointerFrame,
        QuoteFrame, RenderSpec, SquiggleFrame, TaskLinkFrame, TaskSceneFrame, TerminalSceneFrame,
        TextRangeBounds, TokenHighlight,
    },
    timeline::{PropertyId, SpringProfile},
    transcript::Transcript,
    video::VideoFrameCache,
};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const FPS: u32 = 60;
const TEMPORAL_SAMPLES: u32 = 8;
const ENTRANCE_TEMPORAL_SAMPLES: u32 = 16;
const SHUTTER_ANGLE: f32 = 180.0;
const FONT_PATH: &str = "/Users/kit/Library/Fonts/CommitMono-400-Regular.otf";
const FOCUS_LINE_ID: &str = "find-effect";
const LESSON_AUDIO_DURATION: f64 = 31.708;
const PROMISES_AUDIO_DURATION: f64 = 31.107;
const _: () = assert!(TEMPORAL_SAMPLES > 0);

enum RenderScene {
    Hero,
    EffectShowsErrors,
    PromisesOnlyHappyPath,
    VisualEffects,
    OpencodeCommandHotReload,
}

impl RenderScene {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "hero" => Some(Self::Hero),
            "effect-shows-errors" => Some(Self::EffectShowsErrors),
            "promises-only-happy-path" => Some(Self::PromisesOnlyHappyPath),
            "visual-effects" => Some(Self::VisualEffects),
            "opencode-command-hot-reload" => Some(Self::OpencodeCommandHotReload),
            _ => None,
        }
    }

    fn default_output(&self) -> &'static str {
        match self {
            Self::Hero => "output/kinograph-prototype.mp4",
            Self::EffectShowsErrors => "output/effect-shows-errors.mp4",
            Self::PromisesOnlyHappyPath => "output/promises-only-happy-path.mp4",
            Self::VisualEffects => "output/visual-effects.mp4",
            Self::OpencodeCommandHotReload => "output/opencode-command-hot-reload.mp4",
        }
    }
}

fn main() -> Result<()> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let (scene, explicit_output) = match arguments.as_slice() {
        [] => (RenderScene::Hero, None),
        [output] if output != "render" => (RenderScene::Hero, Some(output.as_str())),
        [command, scene] if command == "render" => (
            RenderScene::parse(scene).with_context(|| format!("unknown scene '{scene}'"))?,
            None,
        ),
        [command, scene, output] if command == "render" => (
            RenderScene::parse(scene).with_context(|| format!("unknown scene '{scene}'"))?,
            Some(output.as_str()),
        ),
        _ => bail!(
            "usage: kinograph [output] | kinograph render \
             <hero|effect-shows-errors|promises-only-happy-path|visual-effects|opencode-command-hot-reload> [output]"
        ),
    };
    let output = PathBuf::from(explicit_output.unwrap_or_else(|| scene.default_output()));

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output directory {}", parent.display()))?;
    }

    match scene {
        RenderScene::Hero => pollster::block_on(render_hero(&output)),
        RenderScene::EffectShowsErrors => pollster::block_on(render_effect_shows_errors(&output)),
        RenderScene::PromisesOnlyHappyPath => {
            pollster::block_on(render_promises_only_happy_path(&output))
        }
        RenderScene::VisualEffects => pollster::block_on(render_visual_effects(&output)),
        RenderScene::OpencodeCommandHotReload => {
            pollster::block_on(render_opencode_command_hot_reload(&output))
        }
    }
}

async fn render_hero(output: &Path) -> Result<()> {
    let transition = hero_code_transition()?;
    let mut renderer = HeadlessRenderer::new(RenderSpec {
        width: WIDTH,
        height: HEIGHT,
        font_path: PathBuf::from(FONT_PATH),
        file_name: "service.ts".to_owned(),
    })
    .await?;
    let settled_lines = transition.sample(TransitionProgress {
        layout: 1.0,
        content: 1.0,
    });
    let effect = measure_target(
        &mut renderer,
        &settled_lines,
        FOCUS_LINE_ID,
        "Effect.Effect",
    )?;
    let string = measure_target(&mut renderer, &settled_lines, "find-signature", "string")?;
    let not_found = measure_target(&mut renderer, &settled_lines, FOCUS_LINE_ID, "NotFound")?;
    let context = measure_target(&mut renderer, &settled_lines, "class-open", "Context.Tag")?;
    let choreography = hero_choreography(effect, string, not_found, context)?;

    encode_video(
        &mut renderer,
        output,
        &choreography.scene,
        |renderer, time| {
            let (
                panel_offset_y,
                focus_intensity,
                focus_line_y,
                token_highlight,
                pointer,
                inline_reveal,
                lines,
            ) = sample_editor(&transition, &choreography, time);
            let inline_reveals = [inline_reveal];
            let squiggles = [];
            let annotations = [];
            let frame = EditorFrame {
                panel_offset_y,
                panel_rotation: 0.0,
                panel_tilt_x: 0.0,
                panel_tilt_y: 0.0,
                panel_scale: 1.0,
                panel_near_blur: 0.0,
                focus_intensity,
                focus_line_y,
                focus_height: 44.0,
                token_highlight,
                pointer,
                inline_reveals: &inline_reveals,
                squiggles: &squiggles,
                annotations: &annotations,
                lines: &lines,
            };
            renderer.render_editor(&frame)
        },
    )
}

fn encode_video(
    renderer: &mut HeadlessRenderer,
    output: &Path,
    scene: &CompiledScene,
    render_sample: impl FnMut(&mut HeadlessRenderer, f32) -> Result<Vec<u8>>,
) -> Result<()> {
    encode_video_with_samples(
        renderer,
        output,
        scene,
        TEMPORAL_SAMPLES,
        ENTRANCE_TEMPORAL_SAMPLES,
        &[0.0..1.0],
        render_sample,
    )
}

fn encode_video_with_samples(
    renderer: &mut HeadlessRenderer,
    output: &Path,
    scene: &CompiledScene,
    temporal_samples: u32,
    entrance_temporal_samples: u32,
    high_sample_ranges: &[std::ops::Range<f32>],
    mut render_sample: impl FnMut(&mut HeadlessRenderer, f32) -> Result<Vec<u8>>,
) -> Result<()> {
    assert!(temporal_samples > 0 && entrance_temporal_samples > 0);
    let started = Instant::now();
    let frame_count = scene.duration().frame_count(FPS);
    let mut encoder = FfmpegEncoder::start_with_media(
        output,
        VideoSpec {
            width: WIDTH,
            height: HEIGHT,
            fps: FPS,
        },
        scene.media(),
    )?;

    let frame_byte_count = WIDTH as usize * HEIGHT as usize * 4;
    let srgb_to_linear = std::array::from_fn::<_, 256, _>(|value| {
        let encoded = value as f32 / 255.0;
        if encoded <= 0.04045 {
            encoded / 12.92
        } else {
            ((encoded + 0.055) / 1.055).powf(2.4)
        }
    });
    let linear_to_srgb = std::array::from_fn::<_, 65536, _>(|value| {
        let linear = value as f32 / 65535.0;
        let encoded = if linear <= 0.003_130_8 {
            linear * 12.92
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        };
        (encoded * 255.0).round() as u8
    });
    let mut accumulation = vec![0.0_f32; frame_byte_count];
    let mut blended_frame = vec![0_u8; frame_byte_count];

    for frame in 0..frame_count {
        accumulation.fill(0.0);
        let center_time = (frame as f32 + 0.5) / FPS as f32;
        let shutter_duration = SHUTTER_ANGLE / 360.0 / FPS as f32;

        let frame_temporal_samples = if high_sample_ranges
            .iter()
            .any(|range| range.contains(&center_time))
        {
            entrance_temporal_samples
        } else {
            temporal_samples
        };
        for sample in 0..frame_temporal_samples {
            let sample_phase = (sample as f32 + 0.5) / frame_temporal_samples as f32 - 0.5;
            let time = (center_time + sample_phase * shutter_duration).max(0.0);
            let pixels = render_sample(renderer, time)?;

            for (sum, pixel) in accumulation.chunks_exact_mut(4).zip(pixels.chunks_exact(4)) {
                sum[0] += srgb_to_linear[pixel[0] as usize];
                sum[1] += srgb_to_linear[pixel[1] as usize];
                sum[2] += srgb_to_linear[pixel[2] as usize];
                sum[3] += f32::from(pixel[3]) / 255.0;
            }
        }

        let inverse_samples = 1.0 / frame_temporal_samples as f32;
        for (output, sum) in blended_frame
            .chunks_exact_mut(4)
            .zip(accumulation.chunks_exact(4))
        {
            for channel in 0..3 {
                let linear = (sum[channel] * inverse_samples).clamp(0.0, 1.0);
                output[channel] = linear_to_srgb[(linear * 65535.0).round() as usize];
            }
            output[3] = (sum[3] * inverse_samples * 255.0).round() as u8;
        }

        encoder.write_frame(&blended_frame)?;

        if frame % u64::from(FPS) == 0 || frame + 1 == frame_count {
            println!(
                "Rendered {:>3}/{frame_count} frames ({:.1}s, {frame_temporal_samples} samples)",
                frame + 1,
                center_time,
            );
        }
    }

    encoder.finish()?;
    println!(
        "Wrote {} in {:.1}s",
        output.display(),
        started.elapsed().as_secs_f32()
    );
    Ok(())
}

async fn render_effect_shows_errors(output: &Path) -> Result<()> {
    let asset_directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("effect-shows-errors");
    let transcript = Transcript::load(&asset_directory.join("timings.json"))?;
    let transitions = effect_shows_errors_transitions()?;
    let mut renderer = HeadlessRenderer::new(RenderSpec {
        width: WIDTH,
        height: HEIGHT,
        font_path: PathBuf::from(FONT_PATH),
        file_name: "slow-die.ts".to_owned(),
    })
    .await?;
    let initial_lines = transitions.split.sample(TransitionProgress {
        layout: 0.0,
        content: 0.0,
    });
    let settled_lines = transitions.fail.sample(TransitionProgress {
        layout: 1.0,
        content: 1.0,
    });
    let hidden_indent_width = measure_text_width(&mut renderer, "  ")?;
    let hidden_ellipsis_width = measure_text_width(&mut renderer, "...")?;
    let closing_angle_width = measure_text_width(&mut renderer, ">")?;
    let mut effect_target =
        measure_target(&mut renderer, &initial_lines, "sig", "Effect.Effect<number")?;
    effect_target.bounds.width += closing_angle_width;
    let mut random_target = measure_target(
        &mut renderer,
        &initial_lines,
        "random",
        "Random.nextIntBetween",
    )?;
    random_target.bounds.x -= hidden_indent_width;
    let mut sleep_target = measure_target(&mut renderer, &initial_lines, "sleep", "Effect.sleep")?;
    sleep_target.bounds.x -= hidden_indent_width;
    let mut bad_roll_fail = measure_target(&mut renderer, &settled_lines, "fail", "VeryBadRoll")?;
    bad_roll_fail.bounds.x -= hidden_ellipsis_width;
    let mut fail_call = measure_target(&mut renderer, &settled_lines, "fail", "Effect.fail")?;
    fail_call.bounds.x -= hidden_ellipsis_width;
    let targets = LessonTargets {
        effect: effect_target,
        slow_die: measure_target(&mut renderer, &initial_lines, "sig", "slowDie")?,
        random: random_target,
        sleep: sleep_target,
        roll: measure_target(&mut renderer, &settled_lines, "fail", "(n === 4)")?,
        bad_roll_fail,
        fail_call,
        error: measure_target(
            &mut renderer,
            &settled_lines,
            "comment",
            "VeryBadRoll is not assignable to never",
        )?,
        bad_roll_type: measure_target(&mut renderer, &settled_lines, "sig", "VeryBadRoll")?,
    };
    let narration = Asset::audio(
        "effect-shows-errors",
        asset_directory.join("narration.webm"),
    )
    .clip(TimeRange::new(
        Time::ZERO,
        Time::seconds(LESSON_AUDIO_DURATION),
    ));
    let success = Asset::audio(
        "prismatic-bloom",
        asset_directory.join("prismatic-bloom.wav"),
    )
    .clip(TimeRange::new(Time::ZERO, Time::seconds(0.795)))
    .gain_db(-10.0);
    let choreography = effect_shows_errors_choreography(&transcript, targets, narration, success)?;

    encode_video(
        &mut renderer,
        output,
        &choreography.scene,
        |renderer, time| {
            render_effect_shows_errors_sample(renderer, &transitions, &choreography, time)
        },
    )
}

async fn render_promises_only_happy_path(output: &Path) -> Result<()> {
    let asset_directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("promises-only-happy-path");
    let transcript = Transcript::load(&asset_directory.join("timings.json"))?;
    let transition = promises_only_happy_path_transition()?;
    let mut renderer = HeadlessRenderer::new(RenderSpec {
        width: WIDTH,
        height: HEIGHT,
        font_path: PathBuf::from(FONT_PATH),
        file_name: "checkout.ts".to_owned(),
    })
    .await?;
    let settled_lines = transition.sample(TransitionProgress {
        layout: 1.0,
        content: 1.0,
    });
    let question_width = measure_text_width(&mut renderer, " // ???")?;
    let mut error = measure_target(&mut renderer, &settled_lines, "call", "SomeError")?;
    error.bounds.x -= question_width;
    let targets = PromiseTargets {
        checkout: measure_target(&mut renderer, &settled_lines, "sig", "checkout")?,
        promise: measure_target(&mut renderer, &settled_lines, "sig", "Promise<Order>")?,
        get_cart: measure_target(&mut renderer, &settled_lines, "cart", "getCart")?,
        charge: measure_target(&mut renderer, &settled_lines, "payment", "charge")?,
        ship: measure_target(&mut renderer, &settled_lines, "shipment", "ship(payment)")?,
        call: measure_target(&mut renderer, &settled_lines, "call", "cart-123")?,
        order: measure_target(&mut renderer, &settled_lines, "sig", "Order")?,
        question: measure_target(&mut renderer, &settled_lines, "call", "???")?,
        error,
    };
    let narration = Asset::audio(
        "promises-only-happy-path",
        asset_directory.join("narration.webm"),
    )
    .clip(TimeRange::new(
        Time::ZERO,
        Time::seconds(PROMISES_AUDIO_DURATION),
    ));
    let sad = Asset::audio("sad", asset_directory.join("sad.wav"))
        .clip(TimeRange::new(Time::ZERO, Time::seconds(0.67)));
    let choreography = promises_only_happy_path_choreography(&transcript, targets, narration, sad)?;

    encode_video(
        &mut renderer,
        output,
        &choreography.scene,
        |renderer, time| {
            render_promises_only_happy_path_sample(renderer, &transition, &choreography, time)
        },
    )
}

async fn render_visual_effects(output: &Path) -> Result<()> {
    let asset_directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("visual-effects");
    let transcript = Transcript::load(&asset_directory.join("timings.json"))?;
    let narration = Asset::audio("visual-effects", asset_directory.join("narration.webm"))
        .clip(TimeRange::new(Time::ZERO, Time::seconds(48.627)));
    let running_sound = Asset::audio("task-running", asset_directory.join("task-running.wav"))
        .clip(TimeRange::new(Time::ZERO, Time::seconds(0.14)))
        .gain_db(18.0);
    let success_sound = Asset::audio("task-success", asset_directory.join("task-success.wav"))
        .clip(TimeRange::new(Time::ZERO, Time::seconds(0.42)))
        .gain_db(12.0);
    let failure_sound = Asset::audio("task-failure", asset_directory.join("task-failure.wav"))
        .clip(TimeRange::new(Time::ZERO, Time::seconds(0.48)))
        .gain_db(16.0);
    let death_sound = Asset::audio("task-death", asset_directory.join("task-death.wav"))
        .clip(TimeRange::new(Time::ZERO, Time::seconds(1.1)))
        .gain_db(16.0);
    let cues = VisualEffectsCues::from_transcript(&transcript)?;
    let center = WIDTH as f32 * 0.5;
    let y = HEIGHT as f32 * 0.5;
    let mut renderer = HeadlessRenderer::new(RenderSpec {
        width: WIDTH,
        height: HEIGHT,
        font_path: PathBuf::from(FONT_PATH),
        file_name: "effect-simulacra".to_owned(),
    })
    .await?;
    let typescript_width = renderer.measure_task_result_width("TypeScript");
    let homicide_width = renderer.measure_task_result_width("HOMICIDE");
    let detective_width = renderer.measure_task_result_width("JR. DETECTIVE");
    let two_nodes = centered_task_row(center, &[typescript_width, 128.0], 24.0);
    let three_nodes = centered_task_row(center, &[typescript_width, 128.0, 128.0], 24.0);
    let idle_row = centered_task_row(center, &[128.0, 128.0, 128.0], 24.0);
    let classify_done = centered_task_row(center, &[homicide_width, 128.0, 128.0], 24.0);
    let assign_done = centered_task_row(center, &[homicide_width, detective_width, 128.0], 24.0);
    let lang = Task::new("lang", "lang")
        .at(center, y)
        .with_result_width(typescript_width);
    let launch = Task::new("launch", "launch").at(two_nodes[1], y);
    let pact = Task::new("pact", "pact").at(three_nodes[2], y);
    let classify = Task::new("classify", "classify")
        .at(center, y)
        .with_result_width(homicide_width);
    let assign = Task::new("assign", "assign")
        .at(idle_row[1], y)
        .with_result_width(detective_width);
    let notify = Task::new("notify", "notify").at(idle_row[2], y);
    let at = |seconds: f32, change| {
        Composition::delay(
            kinograph::composition::Duration::seconds(f64::from(seconds)),
            change,
        )
    };
    let pose_at = |seconds: f32, change| {
        Composition::delay(
            kinograph::composition::Duration::seconds(f64::from(seconds)),
            Composition::task_pose(change),
        )
    };
    let sound_at = |seconds: f32, clip: Clip| {
        Composition::delay(
            kinograph::composition::Duration::seconds(f64::from(seconds)),
            Composition::layer(clip),
        )
    };
    let composition = Composition::parallel([
        Composition::script(narration),
        at(cues.demo, lang.idle()),
        at(cues.lang_running, lang.run()),
        at(cues.lang_completed, lang.succeed("TypeScript")),
        pose_at(cues.launch_visible, lang.move_to(two_nodes[0], y)),
        at(cues.launch_visible, launch.idle()),
        at(cues.launch_running, launch.run()),
        at(cues.launch_failed, launch.fail("NoFuel")),
        pose_at(cues.pact_visible, lang.move_to(three_nodes[0], y)),
        pose_at(cues.pact_visible, launch.move_to(three_nodes[1], y)),
        at(cues.pact_visible, pact.idle()),
        at(cues.pact_running, pact.run()),
        at(cues.pact_death, pact.die("")),
        at(cues.classify_visible, lang.hide()),
        at(cues.classify_visible, launch.hide()),
        at(cues.classify_visible, pact.hide()),
        at(cues.classify_visible + 0.18, classify.idle()),
        pose_at(cues.combined, classify.move_to(idle_row[0], y)),
        at(cues.combined, assign.idle()),
        at(cues.combined, notify.idle()),
        at(cues.classify_running, classify.run()),
        at(cues.assign_running, classify.succeed("HOMICIDE")),
        pose_at(cues.assign_running, assign.move_to(classify_done[1], y)),
        at(cues.assign_running, assign.run()),
        pose_at(cues.assign_running, notify.move_to(classify_done[2], y)),
        pose_at(cues.notify_running, classify.move_to(assign_done[0], y)),
        at(cues.notify_running, assign.succeed("JR. DETECTIVE")),
        pose_at(cues.notify_running, notify.move_to(assign_done[2], y)),
        at(cues.notify_running, notify.run()),
        at(cues.notify_failed, notify.fail("RateLimitError")),
        at(cues.notify_retry, notify.run()),
        at(cues.notify_completed, notify.complete()),
        sound_at(cues.lang_running, running_sound.clone()),
        sound_at(cues.launch_running, running_sound.clone()),
        sound_at(cues.pact_running, running_sound.clone()),
        sound_at(cues.classify_running, running_sound.clone()),
        sound_at(cues.assign_running, running_sound.clone()),
        sound_at(cues.notify_running, running_sound.clone()),
        sound_at(cues.notify_retry, running_sound),
        sound_at(cues.lang_completed, success_sound.clone()),
        sound_at(cues.assign_running, success_sound.clone()),
        sound_at(cues.notify_running, success_sound.clone()),
        sound_at(cues.notify_completed, success_sound),
        sound_at(cues.launch_failed, failure_sound.clone()),
        sound_at(cues.notify_failed, failure_sound),
        sound_at(cues.pact_death, death_sound),
    ]);
    let scene =
        Scene::new(Vec::<(PropertyId, Scalar)>::new(), composition).compile(&HashMap::new())?;
    encode_video(&mut renderer, output, &scene, |renderer, time| {
        let quote = visual_effects_quote(time, &cues);
        let nodes = scene.task_frames_at(time);
        let links = visual_effects_task_links(time, &cues, &nodes);
        renderer.render_task_scene(&TaskSceneFrame {
            quote,
            links: &links,
            nodes: &nodes,
        })
    })
}

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

async fn render_opencode_command_hot_reload(output: &Path) -> Result<()> {
    const DURATION: f64 = 10.0;

    let asset_directory = Path::new(env!("CARGO_MANIFEST_DIR"))
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
        Path::new(env!("CARGO_MANIFEST_DIR"))
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
        panel_center: [
            960.0 + (570.0 - 960.0) * split,
            520.0 + (1.0 - enter) * 96.0 + (500.0 - 520.0) * focus + 40.0 * split,
        ],
        panel_scale: (0.9 + 0.1 * enter)
            * ((1.0 + 0.22 * focus) + (0.74 - (1.0 + 0.22 * focus)) * split)
            * submit_scale,
        panel_rotation: (1.0 - enter) * -0.035 + focus * 0.014 + (-0.018 - focus * 0.014) * split,
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

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

struct VisualEffectsCues {
    quote_words: [f32; 8],
    hide_quote: f32,
    subliminal: f32,
    demo: f32,
    lang_running: f32,
    lang_completed: f32,
    launch_visible: f32,
    launch_running: f32,
    launch_failed: f32,
    pact_visible: f32,
    pact_running: f32,
    pact_death: f32,
    classify_visible: f32,
    combined: f32,
    classify_running: f32,
    assign_running: f32,
    notify_running: f32,
    notify_failed: f32,
    notify_retry: f32,
    notify_completed: f32,
}

impl VisualEffectsCues {
    fn from_transcript(transcript: &Transcript) -> Result<Self> {
        let start = |word: &str, occurrence| {
            Ok::<_, anyhow::Error>(
                transcript
                    .word_occurrence(word, occurrence)?
                    .start()
                    .as_seconds() as f32
                    - 0.15,
            )
        };
        Ok(Self {
            quote_words: [
                start("\"why", 0)?,
                start("would", 0)?,
                start("I", 1)?,
                start("ever", 0)?,
                start("want", 0)?,
                start("to", 1)?,
                start("use", 0)?,
                start("Effect?\"", 0)?,
            ],
            hide_quote: start("I'd", 0)?,
            subliminal: start("tastefully", 0)?,
            demo: start("This", 0)?,
            lang_running: start("Effect,", 0)?,
            lang_completed: start("succeed,", 0)?,
            launch_visible: start("or", 0)?,
            launch_running: start("they", 0)?,
            launch_failed: start("fail,", 0)?,
            pact_visible: start("They'll", 0)?,
            pact_running: start("even", 0)?,
            pact_death: start("die", 0)?,
            classify_visible: start("You", 0)?,
            combined: start("combine", 0)?,
            classify_running: start("running", 0)?,
            assign_running: start("after", 0)?,
            notify_running: start("another.", 0)?,
            notify_failed: start("fails,", 0)?,
            notify_retry: start("retry", 0)?,
            notify_completed: start("schedule.", 0)?,
        })
    }
}

fn visual_effects_quote(time: f32, cues: &VisualEffectsCues) -> Option<QuoteFrame<'static>> {
    const SUBLIMINAL: [&str; 8] = [
        "EFFECT", "IS", "THE", "GREATEST", "LIBRARY", "OF", "ALL", "TIME",
    ];
    if time >= cues.demo {
        return None;
    }
    let highlight = cues.quote_words.iter().rposition(|cue| time >= *cue);
    let subliminal_index = ((time - cues.subliminal) / 0.1).floor() as isize;
    let subliminal = (0..SUBLIMINAL.len() as isize)
        .contains(&subliminal_index)
        .then(|| SUBLIMINAL[subliminal_index as usize]);
    Some(QuoteFrame {
        time,
        highlight,
        show_hypnotic: time >= cues.quote_words[0],
        hide_quote: time >= cues.hide_quote,
        subliminal,
    })
}

fn centered_task_row(center: f32, widths: &[f32], gap: f32) -> Vec<f32> {
    let total_width = widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32;
    let mut cursor = center - total_width * 0.5;
    widths
        .iter()
        .map(|width| {
            let position = cursor + width * 0.5;
            cursor += width + gap;
            position
        })
        .collect()
}

fn visual_effects_task_links(
    time: f32,
    cues: &VisualEffectsCues,
    nodes: &[kinograph::dsl::TaskFrame<'_>],
) -> Vec<TaskLinkFrame> {
    if time < cues.combined {
        return Vec::new();
    }
    let node = |id: &str| {
        nodes
            .iter()
            .find(|node| node.id.as_str() == id)
            .map(|node| [node.x, node.y])
    };
    let (Some(classify), Some(assign), Some(notify)) =
        (node("classify"), node("assign"), node("notify"))
    else {
        return Vec::new();
    };
    let handoff = |arrival: f32| {
        let progress = (time - (arrival - 0.38)) / 0.38;
        (0.0..=1.0).contains(&progress).then_some(progress)
    };
    vec![
        TaskLinkFrame {
            from: classify,
            to: assign,
            pulse: handoff(cues.assign_running),
        },
        TaskLinkFrame {
            from: assign,
            to: notify,
            pulse: handoff(cues.notify_running),
        },
    ]
}

#[cfg(any())]
fn old_visual_effects_frame(time: f32, cues: &VisualEffectsCues) {
    let y = HEIGHT as f32 * 0.5;
    if time < cues.launch_visible {
        let (state, state_age) = task_state(
            time,
            cues.demo,
            &[
                (cues.lang_running, TaskVisualState::Running),
                (cues.lang_completed, TaskVisualState::Completed),
            ],
        );
        return (
            None,
            vec![TaskNodeFrame {
                x: WIDTH as f32 * 0.5,
                y,
                name: "lang",
                state,
                state_age,
                visible_age: time - cues.demo,
                result: Some("TypeScript"),
                error: None,
            }],
        );
    }
    if time < cues.pact_visible {
        let (launch_state, launch_age) = task_state(
            time,
            cues.launch_visible,
            &[
                (cues.launch_running, TaskVisualState::Running),
                (cues.launch_failed, TaskVisualState::Failed),
            ],
        );
        return (
            None,
            vec![
                TaskNodeFrame {
                    x: WIDTH as f32 * 0.5 - 100.0,
                    y,
                    name: "lang",
                    state: TaskVisualState::Completed,
                    state_age: time - cues.lang_completed,
                    visible_age: time - cues.demo,
                    result: Some("TypeScript"),
                    error: None,
                },
                TaskNodeFrame {
                    x: WIDTH as f32 * 0.5 + 100.0,
                    y,
                    name: "launch",
                    state: launch_state,
                    state_age: launch_age,
                    visible_age: time - cues.launch_visible,
                    result: None,
                    error: Some("NoFuel"),
                },
            ],
        );
    }
    if time < cues.classify_visible {
        let (pact_state, pact_age) = task_state(
            time,
            cues.pact_visible,
            &[
                (cues.pact_running, TaskVisualState::Running),
                (cues.pact_death, TaskVisualState::Death),
            ],
        );
        return (
            None,
            vec![
                TaskNodeFrame {
                    x: WIDTH as f32 * 0.5 - 210.0,
                    y,
                    name: "lang",
                    state: TaskVisualState::Completed,
                    state_age: time - cues.lang_completed,
                    visible_age: time - cues.demo,
                    result: Some("TypeScript"),
                    error: None,
                },
                TaskNodeFrame {
                    x: WIDTH as f32 * 0.5,
                    y,
                    name: "launch",
                    state: TaskVisualState::Failed,
                    state_age: time - cues.launch_failed,
                    visible_age: time - cues.launch_visible,
                    result: None,
                    error: Some("NoFuel"),
                },
                TaskNodeFrame {
                    x: WIDTH as f32 * 0.5 + 210.0,
                    y,
                    name: "pact",
                    state: pact_state,
                    state_age: pact_age,
                    visible_age: time - cues.pact_visible,
                    result: None,
                    error: Some("wat"),
                },
            ],
        );
    }
    if time < cues.combined {
        return (
            None,
            vec![TaskNodeFrame {
                x: WIDTH as f32 * 0.5,
                y,
                name: "classify",
                state: TaskVisualState::Idle,
                state_age: time - cues.classify_visible,
                visible_age: time - cues.classify_visible,
                result: Some("HOMICIDE"),
                error: None,
            }],
        );
    }
    let (classify_state, classify_age) = task_state(
        time,
        cues.combined,
        &[
            (cues.classify_running, TaskVisualState::Running),
            (cues.assign_running, TaskVisualState::Completed),
        ],
    );
    let (assign_state, assign_age) = task_state(
        time,
        cues.combined,
        &[
            (cues.assign_running, TaskVisualState::Running),
            (cues.notify_running, TaskVisualState::Completed),
        ],
    );
    let (notify_state, notify_age) = task_state(
        time,
        cues.combined,
        &[
            (cues.notify_running, TaskVisualState::Running),
            (cues.notify_failed, TaskVisualState::Failed),
            (cues.notify_retry, TaskVisualState::Running),
            (cues.notify_completed, TaskVisualState::Completed),
        ],
    );
    (
        None,
        vec![
            TaskNodeFrame {
                x: WIDTH as f32 * 0.5 - 240.0,
                y,
                name: "classify",
                state: classify_state,
                state_age: classify_age,
                visible_age: time - cues.combined,
                result: Some("HOMICIDE"),
                error: None,
            },
            TaskNodeFrame {
                x: WIDTH as f32 * 0.5,
                y,
                name: "assign",
                state: assign_state,
                state_age: assign_age,
                visible_age: time - cues.combined,
                result: Some("JR. DETECTIVE"),
                error: None,
            },
            TaskNodeFrame {
                x: WIDTH as f32 * 0.5 + 240.0,
                y,
                name: "notify",
                state: notify_state,
                state_age: notify_age,
                visible_age: time - cues.combined,
                result: Some("DONE"),
                error: Some("RateLimitError"),
            },
        ],
    )
}

#[cfg(any())]
fn task_state(
    time: f32,
    visible_at: f32,
    changes: &[(f32, TaskVisualState)],
) -> (TaskVisualState, f32) {
    let mut state = TaskVisualState::Idle;
    let mut changed_at = visible_at;
    for (at, next) in changes {
        if time < *at {
            break;
        }
        state = *next;
        changed_at = *at;
    }
    (state, time - changed_at)
}

#[derive(Clone, Copy)]
struct PromiseTargets {
    checkout: CodeTarget,
    promise: CodeTarget,
    get_cart: CodeTarget,
    charge: CodeTarget,
    ship: CodeTarget,
    call: CodeTarget,
    order: CodeTarget,
    question: CodeTarget,
    error: CodeTarget,
}

struct PromisesOnlyHappyPathChoreography {
    scene: CompiledScene,
    panel_y: PropertyId,
    panel_rotation: PropertyId,
    panel_tilt_x: PropertyId,
    panel_tilt_y: PropertyId,
    panel_scale: PropertyId,
    panel_near_blur: PropertyId,
    code_layout: PropertyId,
    code_content: PropertyId,
    focus: PropertyId,
    focus_y: PropertyId,
    token_x: PropertyId,
    token_y: PropertyId,
    token_width: PropertyId,
    token_opacity: PropertyId,
    pointer: Pointer,
    question: PropertyId,
    error: PropertyId,
}

fn promises_only_happy_path_choreography(
    transcript: &Transcript,
    measured: PromiseTargets,
    narration: kinograph::composition::Clip,
    sad: kinograph::composition::Clip,
) -> Result<PromisesOnlyHappyPathChoreography> {
    let code = Code::new("promise.code");
    let pointer = Pointer::new("promise.pointer");
    let question = PropertyId::new("promise.code.question");
    let error = PropertyId::new("promise.code.error");
    let spotlight = SpringProfile::from_visual_duration(0.3, 0.0, 0.001, 0.001);
    let entrance = SpringProfile::from_visual_duration(0.7, 0.22, 0.001, 0.001);
    let line_motion = SpringProfile::from_visual_duration(0.42, 0.0, 0.001, 0.001);
    let pointer_motion = SpringProfile::from_visual_duration(0.5, 0.35, 0.001, 0.001);
    let pointer_scale = 24.0 / 36.0;

    let checkout = code.text("sig", "checkout");
    let promise = code.text("sig", "Promise<Order>");
    let get_cart = code.text("cart", "getCart");
    let charge = code.text("payment", "charge");
    let ship = code.text("shipment", "ship(payment)");
    let call = code.text("call", "cart-123");
    let order = code.text("sig", "Order");
    let question_target = code.text("call", "???");
    let error_target = code.text("call", "SomeError");
    let targets = HashMap::from([
        (checkout.clone(), measured.checkout.into()),
        (promise.clone(), measured.promise.into()),
        (get_cart.clone(), measured.get_cart.into()),
        (charge.clone(), measured.charge.into()),
        (ship.clone(), measured.ship.into()),
        (call.clone(), measured.call.into()),
        (order.clone(), measured.order.into()),
        (question_target.clone(), measured.question.into()),
        (error_target.clone(), measured.error.into()),
    ]);
    let forthcoming = transcript.word("forthcoming")?;
    let at = |cue: kinograph::composition::Cue, motion| {
        Composition::delay(cue.start_offset(), Composition::animate(motion))
    };
    let focus_cursor_at = |target: kinograph::dsl::TextTarget, cue: kinograph::composition::Cue| {
        at(
            cue,
            Motion::parallel([
                code.focus(target.clone(), spotlight),
                code.highlight(target.clone(), spotlight),
                Motion::spring(pointer.opacity.clone(), 1.0, pointer_motion),
                pointer.move_to(target, 55.0, pointer_motion),
            ]),
        )
    };
    let composition = Composition::parallel([
        Composition::script(narration),
        Composition::delay(
            kinograph::composition::Duration::milliseconds(80.0),
            Motion::parallel([
                Motion::spring(code.panel_y.clone(), 0.0, entrance),
                Motion::spring(code.panel_rotation.clone(), 0.0, entrance),
                Motion::spring(code.panel_tilt_x.clone(), 0.0, entrance),
                Motion::spring(code.panel_tilt_y.clone(), 0.0, entrance),
                Motion::spring(code.panel_scale.clone(), 1.0, entrance),
                Motion::spring(code.panel_near_blur.clone(), 0.0, entrance),
            ]),
        ),
        focus_cursor_at(checkout.clone(), transcript.word("checkout")?),
        focus_cursor_at(promise.clone(), transcript.word("Promise<Order>.")?),
        at(
            transcript.word("Internally,")?,
            Motion::parallel([
                Motion::spring(code.focus.clone(), 0.0, spotlight),
                Motion::spring(code.highlight_opacity.clone(), 0.0, spotlight),
            ]),
        ),
        focus_cursor_at(get_cart, transcript.word("three")?),
        focus_cursor_at(charge, transcript.word("more")?),
        focus_cursor_at(ship, transcript.word("async")?),
        at(
            transcript.word("What")?,
            Motion::parallel([
                Motion::spring(code.focus.clone(), 0.0, spotlight),
                Motion::spring(code.highlight_opacity.clone(), 0.0, spotlight),
                Motion::spring(pointer.opacity.clone(), 0.0, pointer_motion),
            ]),
        ),
        at(
            transcript.word("call")?,
            Motion::parallel([
                Motion::spring(code.layout.clone(), 1.0, line_motion),
                Motion::spring(code.content.clone(), 1.0, line_motion),
                code.focus(call.clone(), spotlight),
                code.highlight(call.clone(), spotlight),
                Motion::spring(pointer.opacity.clone(), 1.0, pointer_motion),
                pointer.move_to(call, 55.0, pointer_motion),
            ]),
        ),
        focus_cursor_at(promise.clone(), transcript.word("type")?),
        focus_cursor_at(order, transcript.word("Order")?),
        at(
            transcript.word("fail?")?,
            Motion::parallel([
                Motion::spring(question.clone(), 1.0, line_motion),
                code.focus(question_target.clone(), spotlight),
                code.highlight(question_target.clone(), spotlight),
                pointer.move_to(question_target, 55.0, pointer_motion),
            ]),
        ),
        at(
            transcript.word("how")?,
            Motion::parallel([
                Motion::spring(question.clone(), 0.0, line_motion),
                Motion::spring(error.clone(), 1.0, line_motion),
                code.focus(error_target.clone(), spotlight),
                code.highlight(error_target.clone(), spotlight),
                pointer.move_to(error_target, 55.0, pointer_motion),
            ]),
        ),
        Composition::delay(
            forthcoming.start_offset(),
            Composition::parallel([
                Composition::layer(sad),
                Composition::animate(Motion::parallel([
                    Motion::spring(code.layout.clone(), 0.0, line_motion),
                    Motion::spring(code.content.clone(), 0.0, line_motion),
                    Motion::spring(question.clone(), 0.0, line_motion),
                    Motion::spring(error.clone(), 0.0, line_motion),
                    code.focus(promise.clone(), spotlight),
                    code.highlight(promise.clone(), spotlight),
                    Motion::spring(pointer.opacity.clone(), 0.0, pointer_motion),
                ])),
                Composition::annotate(Annotation::on(promise).effect(AnnotationEffect::FocusPulse)),
            ]),
        ),
        at(
            transcript.word("regard.")?,
            Motion::parallel([
                Motion::spring(code.focus.clone(), 0.0, spotlight),
                Motion::spring(code.highlight_opacity.clone(), 0.0, spotlight),
            ]),
        ),
    ]);
    let scene = Scene::new(
        [
            (code.panel_y.clone(), Scalar::Literal(280.0)),
            (code.panel_rotation.clone(), Scalar::Literal(-0.14)),
            (code.panel_tilt_x.clone(), Scalar::Literal(-0.31)),
            (code.panel_tilt_y.clone(), Scalar::Literal(0.38)),
            (code.panel_scale.clone(), Scalar::Literal(1.5)),
            (code.panel_near_blur.clone(), Scalar::Literal(4.0)),
            (code.layout.clone(), Scalar::Literal(0.0)),
            (code.content.clone(), Scalar::Literal(0.0)),
            (code.focus.clone(), Scalar::Literal(0.0)),
            (code.focus_y.clone(), Scalar::TargetLineY(checkout.clone())),
            (code.highlight_x.clone(), Scalar::TargetX(checkout.clone())),
            (
                code.highlight_y.clone(),
                Scalar::TargetLineY(checkout.clone()),
            ),
            (
                code.highlight_width.clone(),
                Scalar::TargetWidth(checkout.clone()),
            ),
            (code.highlight_opacity.clone(), Scalar::Literal(0.0)),
            (
                pointer.x.clone(),
                Scalar::TargetCenterX(checkout.clone()).offset(40.0),
            ),
            (
                pointer.y.clone(),
                Scalar::TargetBelow {
                    target: checkout,
                    offset: 85.0,
                },
            ),
            (pointer.opacity.clone(), Scalar::Literal(0.0)),
            (pointer.scale.clone(), Scalar::Literal(pointer_scale)),
            (pointer.blur.clone(), Scalar::Literal(0.0)),
            (question.clone(), Scalar::Literal(0.0)),
            (error.clone(), Scalar::Literal(0.0)),
        ],
        composition,
    )
    .compile(&targets)?;

    Ok(PromisesOnlyHappyPathChoreography {
        scene,
        panel_y: code.panel_y,
        panel_rotation: code.panel_rotation,
        panel_tilt_x: code.panel_tilt_x,
        panel_tilt_y: code.panel_tilt_y,
        panel_scale: code.panel_scale,
        panel_near_blur: code.panel_near_blur,
        code_layout: code.layout,
        code_content: code.content,
        focus: code.focus,
        focus_y: code.focus_y,
        token_x: code.highlight_x,
        token_y: code.highlight_y,
        token_width: code.highlight_width,
        token_opacity: code.highlight_opacity,
        pointer,
        question,
        error,
    })
}

fn render_promises_only_happy_path_sample(
    renderer: &mut HeadlessRenderer,
    transition: &CodeTransition,
    choreography: &PromisesOnlyHappyPathChoreography,
    time: f32,
) -> Result<Vec<u8>> {
    let timeline = choreography.scene.timeline();
    let sample = |property| {
        timeline
            .sample(property, time)
            .expect("promise choreography property has an initial value")
            .position
    };
    let lines = transition.sample(TransitionProgress {
        layout: sample(&choreography.code_layout),
        content: sample(&choreography.code_content),
    });
    let reveals = [
        InlineRevealFrame {
            line_id: "call",
            start_span: 3,
            end_span: 4,
            progress: sample(&choreography.question),
        },
        InlineRevealFrame {
            line_id: "call",
            start_span: 4,
            end_span: 6,
            progress: sample(&choreography.error),
        },
    ];
    let squiggles = [];
    let annotations = choreography.scene.annotations_at(time).collect::<Vec<_>>();
    let frame = EditorFrame {
        panel_offset_y: sample(&choreography.panel_y),
        panel_rotation: sample(&choreography.panel_rotation),
        panel_tilt_x: sample(&choreography.panel_tilt_x),
        panel_tilt_y: sample(&choreography.panel_tilt_y),
        panel_scale: sample(&choreography.panel_scale),
        panel_near_blur: sample(&choreography.panel_near_blur),
        focus_intensity: sample(&choreography.focus).clamp(0.0, 1.0),
        focus_line_y: sample(&choreography.focus_y),
        focus_height: 44.0,
        token_highlight: TokenHighlight {
            x: sample(&choreography.token_x),
            y: sample(&choreography.token_y),
            width: sample(&choreography.token_width),
            opacity: sample(&choreography.token_opacity).clamp(0.0, 1.0),
        },
        pointer: sample_pointer_frame(&choreography.scene, &choreography.pointer, time),
        inline_reveals: &reveals,
        squiggles: &squiggles,
        annotations: &annotations,
        lines: &lines,
    };
    renderer.render_editor(&frame)
}

#[derive(Clone, Copy)]
struct LessonTargets {
    effect: CodeTarget,
    slow_die: CodeTarget,
    random: CodeTarget,
    sleep: CodeTarget,
    roll: CodeTarget,
    bad_roll_fail: CodeTarget,
    fail_call: CodeTarget,
    error: CodeTarget,
    bad_roll_type: CodeTarget,
}

struct EffectShowsErrorsChoreography {
    scene: CompiledScene,
    panel_y: PropertyId,
    code_layout: PropertyId,
    code_content: PropertyId,
    fail_layout: PropertyId,
    fail_content: PropertyId,
    focus: PropertyId,
    focus_y: PropertyId,
    focus_height: PropertyId,
    token_x: PropertyId,
    token_y: PropertyId,
    token_width: PropertyId,
    token_opacity: PropertyId,
    pointer: Pointer,
    fail_action: PropertyId,
    error_comment: PropertyId,
    error_type: PropertyId,
    inline_gen: PropertyId,
    indent: PropertyId,
    ellipsis: PropertyId,
    squiggle: PropertyId,
    squiggle_target: CodeTarget,
}

fn effect_shows_errors_choreography(
    transcript: &Transcript,
    measured: LessonTargets,
    narration: kinograph::composition::Clip,
    success: kinograph::composition::Clip,
) -> Result<EffectShowsErrorsChoreography> {
    let code = Code::new("lesson.code");
    let pointer = Pointer::new("lesson.pointer");
    let fail_action = PropertyId::new("lesson.code.fail_action");
    let error_comment = PropertyId::new("lesson.code.error_comment");
    let error_type = PropertyId::new("lesson.code.error_type");
    let inline_gen = PropertyId::new("lesson.code.inline_gen");
    let indent = PropertyId::new("lesson.code.indent");
    let fail_layout = PropertyId::new("lesson.code.fail_layout");
    let fail_content = PropertyId::new("lesson.code.fail_content");
    let ellipsis = PropertyId::new("lesson.code.ellipsis");
    let squiggle = PropertyId::new("lesson.code.squiggle");
    let focus_height = PropertyId::new("lesson.code.focus_height");
    let spotlight = SpringProfile::from_visual_duration(0.3, 0.0, 0.001, 0.001);
    let variable_part = SpringProfile::from_visual_duration(0.4, 0.0, 0.001, 0.001);
    let line_motion = SpringProfile::from_visual_duration(0.45, 0.0, 0.001, 0.001);
    let pointer_motion = SpringProfile::from_visual_duration(0.5, 0.35, 0.001, 0.001);
    let pointer_scale = 24.0 / 36.0;

    let effect = code.text("sig", "Effect.Effect<number");
    let slow_die = code.text("sig", "slowDie");
    let random = code.text("random", "Random.nextIntBetween");
    let sleep = code.text("sleep", "Effect.sleep");
    let roll = code.text("fail", "(n === 4)");
    let bad_roll_fail = code.text("fail", "VeryBadRoll");
    let fail_call = code.text("fail", "Effect.fail");
    let error = code.text("comment", "VeryBadRoll is not assignable to never");
    let bad_roll_type = code.text("sig", "VeryBadRoll");
    let targets = HashMap::from([
        (effect.clone(), measured.effect.into()),
        (slow_die.clone(), measured.slow_die.into()),
        (random.clone(), measured.random.into()),
        (sleep.clone(), measured.sleep.into()),
        (roll.clone(), measured.roll.into()),
        (bad_roll_fail.clone(), measured.bad_roll_fail.into()),
        (fail_call.clone(), measured.fail_call.into()),
        (error.clone(), measured.error.into()),
        (bad_roll_type.clone(), measured.bad_roll_type.into()),
    ]);
    let reckon = transcript.word("reckon")?;
    let success_annotation = Annotation::on(bad_roll_type.clone());

    let at = |cue: kinograph::composition::Cue, motion| {
        Composition::delay(cue.start_offset(), Composition::animate(motion))
    };
    let cursor_at = |target: kinograph::dsl::TextTarget, cue: kinograph::composition::Cue| {
        at(
            cue,
            Motion::parallel([
                code.highlight(target.clone(), spotlight),
                Motion::spring(pointer.opacity.clone(), 1.0, pointer_motion),
                pointer.move_to(target, 55.0, pointer_motion),
            ]),
        )
    };
    let focus_cursor_at = |target: kinograph::dsl::TextTarget, cue: kinograph::composition::Cue| {
        at(
            cue,
            Motion::parallel([
                code.focus(target.clone(), spotlight),
                code.highlight(target.clone(), spotlight),
                Motion::spring(pointer.opacity.clone(), 1.0, pointer_motion),
                pointer.move_to(target, 55.0, pointer_motion),
            ]),
        )
    };
    let composition = Composition::parallel([
        Composition::script(narration),
        Composition::delay(
            reckon.start_offset(),
            Composition::parallel([
                Composition::layer(success),
                Composition::annotate(success_annotation),
            ]),
        ),
        Composition::delay(
            kinograph::composition::Duration::milliseconds(80.0),
            Motion::spring(code.panel_y.clone(), 0.0, spotlight),
        ),
        at(
            transcript.word("Effect")?,
            Motion::parallel([
                code.highlight(effect.clone(), spotlight),
                Motion::spring(pointer.opacity.clone(), 1.0, pointer_motion),
                Motion::spring(pointer.scale.clone(), pointer_scale, pointer_motion),
                Motion::spring(pointer.blur.clone(), 0.0, pointer_motion),
                pointer.move_to(effect.clone(), 55.0, pointer_motion),
            ]),
        ),
        cursor_at(slow_die, transcript.word("slowDie.")?),
        focus_cursor_at(random, transcript.word("random")?),
        focus_cursor_at(sleep, transcript.word("non-blocking,")?),
        at(
            transcript.word("result.")?,
            Motion::parallel([
                Motion::spring(code.focus.clone(), 0.0, spotlight),
                Motion::spring(code.highlight_opacity.clone(), 0.0, spotlight),
                Motion::spring(pointer.opacity.clone(), 0.0, pointer_motion),
            ]),
        ),
        at(
            transcript.word("lethal.")?,
            Motion::parallel([
                Motion::spring(code.layout.clone(), 1.0, line_motion),
                Motion::spring(code.content.clone(), 1.0, line_motion),
                Motion::spring(inline_gen.clone(), 0.0, variable_part),
                Motion::spring(indent.clone(), 1.0, variable_part),
            ]),
        ),
        at(
            transcript.word("roll")?,
            Motion::parallel([
                Motion::spring(fail_layout.clone(), 1.0, line_motion),
                Motion::spring(fail_content.clone(), 1.0, line_motion),
                Motion::spring(pointer.opacity.clone(), 1.0, pointer_motion),
                code.focus(roll.clone(), spotlight),
                code.highlight(roll.clone(), spotlight),
                pointer.move_to(roll, 55.0, pointer_motion),
            ]),
        ),
        at(
            transcript.word("fail")?,
            Motion::parallel([
                Motion::spring(fail_action.clone(), 1.0, variable_part),
                Motion::spring(ellipsis.clone(), 0.0, variable_part),
                code.highlight(bad_roll_fail.clone(), spotlight),
                pointer.move_to(bad_roll_fail, 55.0, pointer_motion),
            ]),
        ),
        at(
            transcript.word("finally,")?,
            Motion::parallel([
                Motion::spring(error_comment.clone(), 1.0, variable_part),
                Motion::spring(squiggle.clone(), 1.0, spotlight),
                Motion::spring(code.focus.clone(), 1.0, spotlight),
                Motion::spring(
                    code.focus_y.clone(),
                    Scalar::TargetLineY(effect.clone()).offset(-22.0),
                    spotlight,
                ),
                Motion::spring(focus_height.clone(), 88.0, spotlight),
                Motion::spring(code.highlight_opacity.clone(), 0.0, spotlight),
                Motion::spring(pointer.opacity.clone(), 0.0, pointer_motion),
            ]),
        ),
        Composition::parallel([
            cursor_at(fail_call, transcript.word("propagates")?),
            at(
                transcript.word("propagates")?,
                Motion::parallel([
                    Motion::spring(code.focus.clone(), 0.0, spotlight),
                    Motion::spring(focus_height.clone(), 44.0, spotlight),
                ]),
            ),
        ]),
        at(
            reckon.clone(),
            Motion::parallel([
                Motion::spring(error_comment.clone(), 0.0, variable_part),
                Motion::spring(error_type.clone(), 1.0, variable_part),
                Motion::spring(squiggle.clone(), 0.0, spotlight),
                Motion::spring(code.focus.clone(), 0.0, spotlight),
                code.highlight(bad_roll_type.clone(), spotlight),
                pointer.move_to(bad_roll_type, 55.0, pointer_motion),
            ]),
        ),
        at(
            transcript.word("TypeScript.")?,
            Motion::parallel([
                Motion::spring(code.focus.clone(), 0.0, spotlight),
                Motion::spring(code.highlight_opacity.clone(), 0.0, spotlight),
                Motion::spring(pointer.opacity.clone(), 0.0, spotlight),
            ]),
        ),
    ]);
    let scene = Scene::new(
        [
            (code.panel_y.clone(), Scalar::Literal(250.0)),
            (code.layout.clone(), Scalar::Literal(0.0)),
            (code.content.clone(), Scalar::Literal(0.0)),
            (fail_layout.clone(), Scalar::Literal(0.0)),
            (fail_content.clone(), Scalar::Literal(0.0)),
            (code.focus.clone(), Scalar::Literal(0.0)),
            (code.focus_y.clone(), Scalar::TargetLineY(effect.clone())),
            (focus_height.clone(), Scalar::Literal(44.0)),
            (code.highlight_x.clone(), Scalar::TargetX(effect.clone())),
            (
                code.highlight_y.clone(),
                Scalar::TargetLineY(effect.clone()),
            ),
            (
                code.highlight_width.clone(),
                Scalar::TargetWidth(effect.clone()),
            ),
            (code.highlight_opacity.clone(), Scalar::Literal(0.0)),
            (
                pointer.x.clone(),
                Scalar::TargetCenterX(effect.clone()).offset(40.0),
            ),
            (
                pointer.y.clone(),
                Scalar::TargetBelow {
                    target: effect,
                    offset: 85.0,
                },
            ),
            (pointer.opacity.clone(), Scalar::Literal(0.0)),
            (pointer.scale.clone(), Scalar::Literal(pointer_scale * 0.7)),
            (pointer.blur.clone(), Scalar::Literal(4.0)),
            (fail_action.clone(), Scalar::Literal(0.0)),
            (error_comment.clone(), Scalar::Literal(0.0)),
            (error_type.clone(), Scalar::Literal(0.0)),
            (inline_gen.clone(), Scalar::Literal(1.0)),
            (indent.clone(), Scalar::Literal(0.0)),
            (ellipsis.clone(), Scalar::Literal(1.0)),
            (squiggle.clone(), Scalar::Literal(0.0)),
        ],
        composition,
    )
    .compile(&targets)?;

    Ok(EffectShowsErrorsChoreography {
        scene,
        panel_y: code.panel_y,
        code_layout: code.layout,
        code_content: code.content,
        fail_layout,
        fail_content,
        focus: code.focus,
        focus_y: code.focus_y,
        focus_height,
        token_x: code.highlight_x,
        token_y: code.highlight_y,
        token_width: code.highlight_width,
        token_opacity: code.highlight_opacity,
        pointer,
        fail_action,
        error_comment,
        error_type,
        inline_gen,
        indent,
        ellipsis,
        squiggle,
        squiggle_target: measured.effect,
    })
}

fn render_effect_shows_errors_sample(
    renderer: &mut HeadlessRenderer,
    transitions: &LessonTransitions,
    choreography: &EffectShowsErrorsChoreography,
    time: f32,
) -> Result<Vec<u8>> {
    let timeline = choreography.scene.timeline();
    let sample = |property| {
        timeline
            .sample(property, time)
            .expect("lesson choreography property has an initial value")
            .position
    };
    let split_progress = TransitionProgress {
        layout: sample(&choreography.code_layout),
        content: sample(&choreography.code_content),
    };
    let lines = if split_progress.content < 0.999 {
        transitions.split.sample(split_progress)
    } else {
        transitions.fail.sample(TransitionProgress {
            layout: sample(&choreography.fail_layout),
            content: sample(&choreography.fail_content),
        })
    };
    let indent = sample(&choreography.indent);
    let reveals = vec![
        InlineRevealFrame {
            line_id: "comment",
            start_span: 0,
            end_span: 1,
            progress: sample(&choreography.error_comment),
        },
        InlineRevealFrame {
            line_id: "sig",
            start_span: 6,
            end_span: 8,
            progress: sample(&choreography.error_type),
        },
        InlineRevealFrame {
            line_id: "sig",
            start_span: 9,
            end_span: 14,
            progress: sample(&choreography.inline_gen),
        },
        InlineRevealFrame {
            line_id: "fail",
            start_span: 3,
            end_span: 4,
            progress: sample(&choreography.ellipsis),
        },
        InlineRevealFrame {
            line_id: "fail",
            start_span: 4,
            end_span: 9,
            progress: sample(&choreography.fail_action),
        },
        InlineRevealFrame {
            line_id: "random",
            start_span: 0,
            end_span: 1,
            progress: indent,
        },
        InlineRevealFrame {
            line_id: "sleep",
            start_span: 0,
            end_span: 1,
            progress: indent,
        },
        InlineRevealFrame {
            line_id: "return",
            start_span: 0,
            end_span: 1,
            progress: indent,
        },
        InlineRevealFrame {
            line_id: "close",
            start_span: 0,
            end_span: 1,
            progress: indent,
        },
    ];
    let squiggles = [SquiggleFrame {
        x: choreography.squiggle_target.bounds.x,
        y: choreography.squiggle_target.line_y,
        width: choreography.squiggle_target.bounds.width,
        opacity: sample(&choreography.squiggle),
    }];
    let annotations = choreography.scene.annotations_at(time).collect::<Vec<_>>();
    let frame = EditorFrame {
        panel_offset_y: sample(&choreography.panel_y),
        panel_rotation: 0.0,
        panel_tilt_x: 0.0,
        panel_tilt_y: 0.0,
        panel_scale: 1.0,
        panel_near_blur: 0.0,
        focus_intensity: sample(&choreography.focus).clamp(0.0, 1.0),
        focus_line_y: sample(&choreography.focus_y),
        focus_height: sample(&choreography.focus_height),
        token_highlight: TokenHighlight {
            x: sample(&choreography.token_x),
            y: sample(&choreography.token_y),
            width: sample(&choreography.token_width),
            opacity: sample(&choreography.token_opacity).clamp(0.0, 1.0),
        },
        pointer: sample_pointer_frame(&choreography.scene, &choreography.pointer, time),
        inline_reveals: &reveals,
        squiggles: &squiggles,
        annotations: &annotations,
        lines: &lines,
    };
    renderer.render_editor(&frame)
}

#[derive(Clone, Copy)]
struct CodeTarget {
    bounds: TextRangeBounds,
    line_y: f32,
}

impl From<CodeTarget> for TargetGeometry {
    fn from(target: CodeTarget) -> Self {
        Self {
            x: target.bounds.x,
            width: target.bounds.width,
            line_y: target.line_y,
        }
    }
}

fn measure_target(
    renderer: &mut HeadlessRenderer,
    lines: &[PlacedLine<'_>],
    line_id: &str,
    text: &str,
) -> Result<CodeTarget> {
    let placed = lines
        .iter()
        .find(|placed| placed.line.id.as_str() == line_id)
        .with_context(|| format!("code target line '{line_id}' is not in the settled scene"))?;
    Ok(CodeTarget {
        bounds: renderer.measure_text_range(placed.line, text)?,
        line_y: placed.y,
    })
}

fn measure_text_width(renderer: &mut HeadlessRenderer, text: &str) -> Result<f32> {
    let line = CodeLine::new("measurement", vec![span(text, SyntaxStyle::Plain)]);
    Ok(renderer.measure_text_range(&line, text)?.width)
}

fn sample_pointer_frame(scene: &CompiledScene, pointer: &Pointer, time: f32) -> PointerFrame {
    let sample_state = |property, sample_time| {
        scene
            .timeline()
            .sample(property, sample_time)
            .expect("pointer property has an initial value")
    };
    let sample = |property| sample_state(property, time).position;
    let x = sample_state(&pointer.x, time);
    let y = sample_state(&pointer.y, time);
    let derivative_step = 1.0 / 240.0;
    let previous_time = (time - derivative_step).max(0.0);
    let previous_x = sample_state(&pointer.x, previous_time);
    let previous_y = sample_state(&pointer.y, previous_time);
    let acceleration_x = (x.velocity - previous_x.velocity) / derivative_step;
    let acceleration_y = (y.velocity - previous_y.velocity) / derivative_step;

    PointerFrame {
        x: x.position,
        y: y.position,
        opacity: sample(&pointer.opacity).clamp(0.0, 1.0),
        rotation: (x.velocity * 0.00012 + y.velocity * 0.00004
            - acceleration_x * 0.000012
            - acceleration_y * 0.000004)
            .clamp(-0.30, 0.30),
        scale: sample(&pointer.scale),
        blur: sample(&pointer.blur).max(0.0),
    }
}

struct HeroChoreography {
    scene: CompiledScene,
    panel_y: PropertyId,
    code_layout: PropertyId,
    code_content: PropertyId,
    focus: PropertyId,
    token_x: PropertyId,
    token_y: PropertyId,
    token_width: PropertyId,
    token_opacity: PropertyId,
    pointer: Pointer,
    inline_reveal: PropertyId,
}

fn hero_choreography(
    effect: CodeTarget,
    string: CodeTarget,
    not_found: CodeTarget,
    context: CodeTarget,
) -> Result<HeroChoreography> {
    let code = Code::new("editor");
    let panel_y = code.panel_y.clone();
    let code_layout = code.layout.clone();
    let code_content = code.content.clone();
    let focus = code.focus.clone();
    let token_x = code.highlight_x.clone();
    let token_y = code.highlight_y.clone();
    let token_width = code.highlight_width.clone();
    let token_opacity = code.highlight_opacity.clone();
    let pointer = Pointer::new("pointer");
    let pointer_x = pointer.x.clone();
    let pointer_y = pointer.y.clone();
    let pointer_opacity = pointer.opacity.clone();
    let pointer_scale = pointer.scale.clone();
    let pointer_blur = pointer.blur.clone();
    let inline_reveal = code.inline_reveal.clone();
    let profile = SpringProfile::from_visual_duration(0.3, 0.0, 0.001, 0.001);
    let pointer_profile = SpringProfile::from_visual_duration(0.42, 0.18, 0.001, 0.001);
    let effect_target = code.text(FOCUS_LINE_ID, "Effect.Effect");
    let string_target = code.text("find-signature", "string");
    let not_found_target = code.text(FOCUS_LINE_ID, "NotFound");
    let context_target = code.text("class-open", "Context.Tag");
    let targets = HashMap::from([
        (effect_target.clone(), effect.into()),
        (string_target.clone(), string.into()),
        (not_found_target.clone(), not_found.into()),
        (context_target.clone(), context.into()),
    ]);
    let animation = Motion::parallel([
        Motion::hold(5.0),
        Motion::delay(0.08, Motion::spring(panel_y.clone(), 0.0, profile)),
        Motion::delay(0.88, Motion::spring(code_layout.clone(), 1.0, profile)),
        Motion::delay(1.12, Motion::spring(code_content.clone(), 1.0, profile)),
        Motion::delay(1.55, Motion::spring(focus.clone(), 1.0, profile)),
        Motion::delay(1.72, code.highlight(effect_target.clone(), profile)),
        Motion::delay(
            1.72,
            Motion::spring(pointer_opacity.clone(), 1.0, pointer_profile),
        ),
        Motion::delay(
            1.72,
            Motion::spring(pointer_scale.clone(), 1.0, pointer_profile),
        ),
        Motion::delay(
            1.72,
            Motion::spring(pointer_blur.clone(), 0.0, pointer_profile),
        ),
        Motion::delay(
            1.72,
            pointer.move_to(effect_target.clone(), 55.0, pointer_profile),
        ),
        Motion::delay(2.72, code.reveal_inline(profile)),
        Motion::delay(3.15, code.highlight(not_found_target.clone(), profile)),
        Motion::delay(2.35, pointer.move_to(string_target, 55.0, pointer_profile)),
        Motion::delay(
            3.15,
            pointer.move_to(not_found_target, 55.0, pointer_profile),
        ),
        Motion::delay(4.05, pointer.move_to(context_target, 55.0, pointer_profile)),
    ]);
    let scene = Scene::new(
        [
            (panel_y.clone(), Scalar::Literal(250.0)),
            (code_layout.clone(), Scalar::Literal(0.0)),
            (code_content.clone(), Scalar::Literal(0.0)),
            (focus.clone(), Scalar::Literal(0.0)),
            (token_x.clone(), Scalar::TargetX(effect_target.clone())),
            (token_y.clone(), Scalar::TargetLineY(effect_target.clone())),
            (
                token_width.clone(),
                Scalar::TargetWidth(effect_target.clone()),
            ),
            (token_opacity.clone(), Scalar::Literal(0.0)),
            (
                pointer_x.clone(),
                Scalar::TargetCenterX(effect_target.clone()).offset(40.0),
            ),
            (
                pointer_y.clone(),
                Scalar::TargetBelow {
                    target: effect_target,
                    offset: 85.0,
                },
            ),
            (pointer_opacity.clone(), Scalar::Literal(0.0)),
            (pointer_scale.clone(), Scalar::Literal(0.7)),
            (pointer_blur.clone(), Scalar::Literal(4.0)),
            (inline_reveal.clone(), Scalar::Literal(0.0)),
        ],
        animation,
    );
    let scene = scene.compile(&targets)?;

    Ok(HeroChoreography {
        scene,
        panel_y,
        code_layout,
        code_content,
        focus,
        token_x,
        token_y,
        token_width,
        token_opacity,
        pointer,
        inline_reveal,
    })
}

fn sample_editor<'a>(
    transition: &'a CodeTransition,
    choreography: &HeroChoreography,
    time: f32,
) -> (
    f32,
    f32,
    f32,
    TokenHighlight,
    PointerFrame,
    InlineRevealFrame,
    Vec<PlacedLine<'a>>,
) {
    let sample = |property| {
        choreography
            .scene
            .timeline()
            .sample(property, time)
            .expect("hero choreography property has an initial value")
            .position
    };
    let panel_offset_y = sample(&choreography.panel_y);
    let layout_progress = sample(&choreography.code_layout);
    let content_progress = sample(&choreography.code_content);
    let focus_intensity = sample(&choreography.focus).clamp(0.0, 1.0);
    let token_highlight = TokenHighlight {
        x: sample(&choreography.token_x),
        y: sample(&choreography.token_y),
        width: sample(&choreography.token_width),
        opacity: sample(&choreography.token_opacity).clamp(0.0, 1.0),
    };
    let pointer = sample_pointer_frame(&choreography.scene, &choreography.pointer, time);
    let inline_reveal = InlineRevealFrame {
        line_id: FOCUS_LINE_ID,
        start_span: 4,
        end_span: 6,
        progress: sample(&choreography.inline_reveal),
    };
    let lines = transition.sample(TransitionProgress {
        layout: layout_progress,
        content: content_progress,
    });
    let focus_line_y = lines
        .iter()
        .find(|placed| placed.line.id.as_str() == FOCUS_LINE_ID)
        .expect("focus line is part of the compiled code manifest")
        .y;
    (
        panel_offset_y,
        focus_intensity,
        focus_line_y,
        token_highlight,
        pointer,
        inline_reveal,
        lines,
    )
}

struct LessonTransitions {
    split: CodeTransition,
    fail: CodeTransition,
}

fn promises_only_happy_path_transition() -> Result<CodeTransition> {
    use SyntaxStyle::{Accent, Keyword, Plain, String as StringStyle, Type};

    let document = CodeDocument::new(vec![
        CodeLine::new(
            "sig",
            vec![
                span("async", Keyword),
                span(" function ", Keyword),
                span("checkout", Plain),
                span("(cartId: ", Plain),
                span("string", Type),
                span("): ", Plain),
                span("Promise", Accent),
                span("<", Plain),
                span("Order", Type),
                span("> {", Plain),
            ],
        ),
        CodeLine::new(
            "cart",
            vec![
                span("  const", Keyword),
                span(" cart = ", Plain),
                span("await", Keyword),
                span(" getCart", Accent),
                span("(cartId)", Plain),
            ],
        ),
        CodeLine::new(
            "payment",
            vec![
                span("  const", Keyword),
                span(" payment = ", Plain),
                span("await", Keyword),
                span(" charge", Accent),
                span("(cart)", Plain),
            ],
        ),
        CodeLine::new(
            "shipment",
            vec![
                span("  const", Keyword),
                span(" shipment = ", Plain),
                span("await", Keyword),
                span(" ship", Accent),
                span("(payment)", Plain),
            ],
        ),
        CodeLine::new(
            "return",
            vec![span("  return", Keyword), span(" shipment", Plain)],
        ),
        CodeLine::new("close", vec![span("}", Plain)]),
        CodeLine::new("spacer", vec![]),
        CodeLine::new(
            "call",
            vec![
                span("checkout(", Plain),
                span("\"cart-123\"", StringStyle),
                span(")", Plain),
                span(" // ???", Accent),
                span(" // throws ", Plain),
                span("SomeError", Type),
            ],
        ),
    ])?;
    let initial = CodeSnapshot::new(["sig", "cart", "payment", "shipment", "return", "close"]);
    let called = CodeSnapshot::new([
        "sig", "cart", "payment", "shipment", "return", "close", "spacer", "call",
    ]);
    CodeTransition::compile(
        &document,
        &initial,
        &called,
        CodeLayout {
            line_height: 44.0,
            entering_offset_x: 96.0,
        },
    )
}

fn effect_shows_errors_transitions() -> Result<LessonTransitions> {
    use SyntaxStyle::{Accent, Keyword, Plain, String as StringStyle, Type};

    let document = CodeDocument::new(vec![
        CodeLine::new(
            "comment",
            vec![span("// > VeryBadRoll is not assignable to never", Accent)],
        ),
        CodeLine::new(
            "sig",
            vec![
                span("const", Keyword),
                span(" slowDie", Plain),
                span(": ", Plain),
                span("Effect.Effect", Accent),
                span("<", Plain),
                span("number", Type),
                span(", ", Plain),
                span("VeryBadRoll", Accent),
                span("> =", Plain),
                span(" ", Plain),
                span("Effect.gen", Accent),
                span("(", Plain),
                span("function*", Keyword),
                span(" () {", Plain),
            ],
        ),
        CodeLine::new(
            "gen",
            vec![
                span("  Effect.gen", Accent),
                span("(", Plain),
                span("function*", Keyword),
                span(" () {", Plain),
            ],
        ),
        CodeLine::new(
            "random",
            vec![
                span("  ", Plain),
                span("  ", Plain),
                span("const", Keyword),
                span(" n = ", Plain),
                span("yield*", Keyword),
                span(" Random.nextIntBetween", Accent),
                span("(1, 7)", Plain),
            ],
        ),
        CodeLine::new(
            "sleep",
            vec![
                span("  ", Plain),
                span("  ", Plain),
                span("yield*", Keyword),
                span(" Effect.sleep", Accent),
                span("(", Plain),
                span("\"1 second\"", StringStyle),
                span(")", Plain),
            ],
        ),
        CodeLine::new(
            "fail",
            vec![
                span("    ", Plain),
                span("if", Keyword),
                span(" (n === 4) ", Plain),
                span("...", Plain),
                span("yield*", Keyword),
                span(" Effect.fail", Accent),
                span("(new ", Plain),
                span("VeryBadRoll", Type),
                span("())", Plain),
            ],
        ),
        CodeLine::new(
            "return",
            vec![
                span("  ", Plain),
                span("  ", Plain),
                span("return", Keyword),
                span(" n", Plain),
            ],
        ),
        CodeLine::new("close", vec![span("  ", Plain), span("})", Plain)]),
    ])?;
    let initial = CodeSnapshot::new(["comment", "sig", "random", "sleep", "return", "close"]);
    let split = CodeSnapshot::new([
        "comment", "sig", "gen", "random", "sleep", "return", "close",
    ]);
    let final_state = CodeSnapshot::new([
        "comment", "sig", "gen", "random", "sleep", "fail", "return", "close",
    ]);
    let layout = CodeLayout {
        line_height: 44.0,
        entering_offset_x: 96.0,
    };

    Ok(LessonTransitions {
        split: CodeTransition::compile(&document, &initial, &split, layout)?,
        fail: CodeTransition::compile(&document, &split, &final_state, layout)?,
    })
}

fn hero_code_transition() -> Result<CodeTransition> {
    use SyntaxStyle::{Accent, Keyword, Plain, String as StringStyle, Type};

    let document = CodeDocument::new(vec![
        CodeLine::new(
            "import",
            vec![
                span("import", Keyword),
                span(" { Context, Effect } ", Plain),
                span("from", Keyword),
                span(" \"effect\"", StringStyle),
            ],
        ),
        CodeLine::new("spacer", vec![]),
        CodeLine::new(
            "class-open",
            vec![
                span("class", Keyword),
                span(" UserService ", Type),
                span("extends", Keyword),
                span(" Context.Tag(", Plain),
                span("\"UserService\"", StringStyle),
                span(")<", Plain),
            ],
        ),
        CodeLine::new(
            "service-self",
            vec![
                span("  ", Plain),
                span("UserService", Type),
                span(",", Plain),
            ],
        ),
        CodeLine::new("shape-open", vec![span("  {", Plain)]),
        CodeLine::new(
            "find-signature",
            vec![
                span("    ", Plain),
                span("readonly", Keyword),
                span(" find: (id: ", Plain),
                span("string", Type),
                span(") =>", Plain),
            ],
        ),
        CodeLine::new(
            FOCUS_LINE_ID,
            vec![
                span("      ", Plain),
                span("Effect.Effect", Accent),
                span("<", Plain),
                span("User", Type),
                span(", ", Plain),
                span("NotFound", Accent),
                span(">", Plain),
            ],
        ),
        CodeLine::new("shape-close", vec![span("  }", Plain)]),
        CodeLine::new("close", vec![span(">() {}", Plain)]),
    ])?;

    CodeTransition::compile(
        &document,
        &CodeSnapshot::new([
            "import",
            "spacer",
            "class-open",
            "service-self",
            "shape-open",
            "shape-close",
            "close",
        ]),
        &CodeSnapshot::new([
            "import",
            "spacer",
            "class-open",
            "service-self",
            "shape-open",
            "find-signature",
            FOCUS_LINE_ID,
            "shape-close",
            "close",
        ]),
        CodeLayout {
            line_height: 44.0,
            entering_offset_x: 96.0,
        },
    )
}

fn span(text: &str, style: SyntaxStyle) -> StyledSpan {
    StyledSpan::new(text, style)
}
