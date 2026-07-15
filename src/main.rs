//! PROTOTYPE: stable code choreography rendered headlessly with wgpu.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result, bail};

use kinograph::{
    code::{
        CodeDocument, CodeLayout, CodeLine, CodeSnapshot, CodeTransition, PlacedLine, StyledSpan,
        SyntaxStyle, TransitionProgress,
    },
    composition::{Asset, Composition, Time, TimeRange},
    dsl::{Annotation, Code, CompiledScene, Motion, Pointer, Scalar, Scene, TargetGeometry},
    encode::{FfmpegEncoder, VideoSpec},
    render::{
        EditorFrame, HeadlessRenderer, InlineRevealFrame, PointerFrame, RenderSpec, SquiggleFrame,
        TextRangeBounds, TokenHighlight,
    },
    timeline::{PropertyId, SpringProfile},
    transcript::Transcript,
};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const FPS: u32 = 60;
const TEMPORAL_SAMPLES: u32 = 8;
const SHUTTER_ANGLE: f32 = 180.0;
const FONT_PATH: &str = "/Users/kit/Library/Fonts/CommitMono-400-Regular.otf";
const FOCUS_LINE_ID: &str = "find-effect";
const LESSON_AUDIO_DURATION: f64 = 31.708;
const _: () = assert!(TEMPORAL_SAMPLES > 0);

fn main() -> Result<()> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let (lesson, output) = match arguments.as_slice() {
        [] => (false, PathBuf::from("output/kinograph-prototype.mp4")),
        [output] if output != "render" => (false, PathBuf::from(output)),
        [command, scene] if command == "render" && scene == "hero" => {
            (false, PathBuf::from("output/kinograph-prototype.mp4"))
        }
        [command, scene, output] if command == "render" && scene == "hero" => {
            (false, PathBuf::from(output))
        }
        [command, scene] if command == "render" && scene == "effect-shows-errors" => {
            (true, PathBuf::from("output/effect-shows-errors.mp4"))
        }
        [command, scene, output] if command == "render" && scene == "effect-shows-errors" => {
            (true, PathBuf::from(output))
        }
        _ => bail!(
            "usage: kinograph [output] | kinograph render <hero|effect-shows-errors> [output]"
        ),
    };

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output directory {}", parent.display()))?;
    }

    if lesson {
        pollster::block_on(render_effect_shows_errors(&output))
    } else {
        pollster::block_on(render_hero(&output))
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

    encode_editor_video(
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

fn encode_editor_video(
    renderer: &mut HeadlessRenderer,
    output: &Path,
    scene: &CompiledScene,
    mut render_sample: impl FnMut(&mut HeadlessRenderer, f32) -> Result<Vec<u8>>,
) -> Result<()> {
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
    let mut accumulation = vec![0_u32; frame_byte_count];
    let mut blended_frame = vec![0_u8; frame_byte_count];

    for frame in 0..frame_count {
        accumulation.fill(0);
        let center_time = (frame as f32 + 0.5) / FPS as f32;
        let shutter_duration = SHUTTER_ANGLE / 360.0 / FPS as f32;

        for sample in 0..TEMPORAL_SAMPLES {
            let sample_phase = (sample as f32 + 0.5) / TEMPORAL_SAMPLES as f32 - 0.5;
            let time = (center_time + sample_phase * shutter_duration).max(0.0);
            let pixels = render_sample(renderer, time)?;

            for (sum, byte) in accumulation.iter_mut().zip(pixels) {
                *sum += u32::from(byte);
            }
        }

        for (output, sum) in blended_frame.iter_mut().zip(&accumulation) {
            *output = ((sum + TEMPORAL_SAMPLES / 2) / TEMPORAL_SAMPLES) as u8;
        }

        encoder.write_frame(&blended_frame)?;

        if frame % u64::from(FPS) == 0 || frame + 1 == frame_count {
            println!(
                "Rendered {:>3}/{frame_count} frames ({:.1}s, {TEMPORAL_SAMPLES} samples)",
                frame + 1,
                center_time
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
    let success = Asset::audio("success", asset_directory.join("success.wav"))
        .clip(TimeRange::new(Time::ZERO, Time::seconds(0.43)))
        .gain_db(18.0);
    let choreography = effect_shows_errors_choreography(&transcript, targets, narration, success)?;

    encode_editor_video(
        &mut renderer,
        output,
        &choreography.scene,
        |renderer, time| {
            render_effect_shows_errors_sample(renderer, &transitions, &choreography, time)
        },
    )
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
    pointer_x: PropertyId,
    pointer_y: PropertyId,
    pointer_opacity: PropertyId,
    pointer_scale: PropertyId,
    pointer_blur: PropertyId,
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
        pointer_x: pointer.x,
        pointer_y: pointer.y,
        pointer_opacity: pointer.opacity,
        pointer_scale: pointer.scale,
        pointer_blur: pointer.blur,
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
    let sample_state = |property, sample_time| {
        timeline
            .sample(property, sample_time)
            .expect("lesson choreography property has an initial value")
    };
    let pointer_x = sample_state(&choreography.pointer_x, time);
    let pointer_y = sample_state(&choreography.pointer_y, time);
    let derivative_step = 1.0 / 240.0;
    let previous_time = (time - derivative_step).max(0.0);
    let previous_x = sample_state(&choreography.pointer_x, previous_time);
    let previous_y = sample_state(&choreography.pointer_y, previous_time);
    let acceleration_x = (pointer_x.velocity - previous_x.velocity) / derivative_step;
    let acceleration_y = (pointer_y.velocity - previous_y.velocity) / derivative_step;
    let pointer = PointerFrame {
        x: pointer_x.position,
        y: pointer_y.position,
        opacity: sample(&choreography.pointer_opacity).clamp(0.0, 1.0),
        rotation: (pointer_x.velocity * 0.00012 + pointer_y.velocity * 0.00004
            - acceleration_x * 0.000012
            - acceleration_y * 0.000004)
            .clamp(-0.30, 0.30),
        scale: sample(&choreography.pointer_scale),
        blur: sample(&choreography.pointer_blur).max(0.0),
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
        focus_intensity: sample(&choreography.focus).clamp(0.0, 1.0),
        focus_line_y: sample(&choreography.focus_y),
        focus_height: sample(&choreography.focus_height),
        token_highlight: TokenHighlight {
            x: sample(&choreography.token_x),
            y: sample(&choreography.token_y),
            width: sample(&choreography.token_width),
            opacity: sample(&choreography.token_opacity).clamp(0.0, 1.0),
        },
        pointer,
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
    pointer_x: PropertyId,
    pointer_y: PropertyId,
    pointer_opacity: PropertyId,
    pointer_scale: PropertyId,
    pointer_blur: PropertyId,
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
        pointer_x,
        pointer_y,
        pointer_opacity,
        pointer_scale,
        pointer_blur,
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
    let sample_state_at = |property, sample_time| {
        choreography
            .scene
            .timeline()
            .sample(property, sample_time)
            .expect("hero choreography property has an initial value")
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
    let pointer_x = sample_state_at(&choreography.pointer_x, time);
    let pointer_y = sample_state_at(&choreography.pointer_y, time);
    let derivative_step = 1.0 / 240.0;
    let previous_time = (time - derivative_step).max(0.0);
    let previous_x = sample_state_at(&choreography.pointer_x, previous_time);
    let previous_y = sample_state_at(&choreography.pointer_y, previous_time);
    let acceleration_x = (pointer_x.velocity - previous_x.velocity) / derivative_step;
    let acceleration_y = (pointer_y.velocity - previous_y.velocity) / derivative_step;
    let travel_tilt = pointer_x.velocity * 0.00012 + pointer_y.velocity * 0.00004;
    let inertial_tilt = -acceleration_x * 0.000012 - acceleration_y * 0.000004;
    let pointer = PointerFrame {
        x: pointer_x.position,
        y: pointer_y.position,
        opacity: sample(&choreography.pointer_opacity).clamp(0.0, 1.0),
        rotation: (travel_tilt + inertial_tilt).clamp(-0.30, 0.30),
        scale: sample(&choreography.pointer_scale),
        blur: sample(&choreography.pointer_blur).max(0.0),
    };
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
