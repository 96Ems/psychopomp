//! PROTOTYPE: stable code choreography rendered headlessly with wgpu.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result};

use kinograph::{
    code::{
        CodeDocument, CodeLayout, CodeLine, CodeSnapshot, CodeTransition, PlacedLine, StyledSpan,
        SyntaxStyle, TransitionProgress,
    },
    dsl::{Code, Motion, Pointer, Scalar, Scene, TargetGeometry},
    encode::{FfmpegEncoder, VideoSpec},
    render::{
        EditorFrame, HeadlessRenderer, InlineRevealFrame, PointerFrame, RenderSpec,
        TextRangeBounds, TokenHighlight,
    },
    timeline::{PropertyId, SpringProfile, Timeline},
};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const FPS: u32 = 60;
const DURATION_SECONDS: u32 = 5;
const FRAME_COUNT: u32 = FPS * DURATION_SECONDS;
const TEMPORAL_SAMPLES: u32 = 8;
const SHUTTER_ANGLE: f32 = 180.0;
const FONT_PATH: &str = "/Users/kit/Library/Fonts/CommitMono-400-Regular.otf";
const FOCUS_LINE_ID: &str = "find-effect";
const _: () = assert!(TEMPORAL_SAMPLES > 0);

fn main() -> Result<()> {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("output/kinograph-prototype.mp4"));

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output directory {}", parent.display()))?;
    }

    pollster::block_on(render_video(&output))
}

async fn render_video(output: &Path) -> Result<()> {
    let started = Instant::now();
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
    let mut encoder = FfmpegEncoder::start(
        output,
        VideoSpec {
            width: WIDTH,
            height: HEIGHT,
            fps: FPS,
        },
    )?;

    let frame_byte_count = WIDTH as usize * HEIGHT as usize * 4;
    let mut accumulation = vec![0_u32; frame_byte_count];
    let mut blended_frame = vec![0_u8; frame_byte_count];

    for frame in 0..FRAME_COUNT {
        accumulation.fill(0);
        let center_time = (frame as f32 + 0.5) / FPS as f32;
        let shutter_duration = SHUTTER_ANGLE / 360.0 / FPS as f32;

        for sample in 0..TEMPORAL_SAMPLES {
            let sample_phase = (sample as f32 + 0.5) / TEMPORAL_SAMPLES as f32 - 0.5;
            let time = (center_time + sample_phase * shutter_duration).max(0.0);
            let (
                panel_offset_y,
                focus_intensity,
                focus_line_y,
                token_highlight,
                pointer,
                inline_reveal,
                lines,
            ) = sample_editor(&transition, &choreography, time);
            let sample_frame = EditorFrame {
                panel_offset_y,
                focus_intensity,
                focus_line_y,
                token_highlight,
                pointer,
                inline_reveal,
                lines: &lines,
            };
            let mut pixels = renderer.render_shapes(&sample_frame)?;
            renderer.composite_text(&mut pixels, &sample_frame)?;

            for (sum, byte) in accumulation.iter_mut().zip(pixels) {
                *sum += u32::from(byte);
            }
        }

        for (output, sum) in blended_frame.iter_mut().zip(&accumulation) {
            *output = ((sum + TEMPORAL_SAMPLES / 2) / TEMPORAL_SAMPLES) as u8;
        }

        encoder.write_frame(&blended_frame)?;

        if frame % FPS == 0 || frame + 1 == FRAME_COUNT {
            println!(
                "Rendered {:>3}/{FRAME_COUNT} frames ({:.1}s, {TEMPORAL_SAMPLES} samples)",
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

struct HeroChoreography {
    timeline: Timeline,
    panel_y: PropertyId,
    code_layout: PropertyId,
    code_content: PropertyId,
    focus: PropertyId,
    token_x: PropertyId,
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
    let timeline = scene.compile(&targets)?;

    Ok(HeroChoreography {
        timeline,
        panel_y,
        code_layout,
        code_content,
        focus,
        token_x,
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
            .timeline
            .sample(property, time)
            .expect("hero choreography property has an initial value")
            .position
    };
    let sample_state_at = |property, sample_time| {
        choreography
            .timeline
            .sample(property, sample_time)
            .expect("hero choreography property has an initial value")
    };
    let panel_offset_y = sample(&choreography.panel_y);
    let layout_progress = sample(&choreography.code_layout);
    let content_progress = sample(&choreography.code_content);
    let focus_intensity = sample(&choreography.focus).clamp(0.0, 1.0);
    let token_highlight = TokenHighlight {
        x: sample(&choreography.token_x),
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
