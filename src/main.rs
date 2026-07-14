//! PROTOTYPE: stable code choreography rendered headlessly with wgpu.

mod code;
mod encode;
mod motion;
mod render;
pub mod timeline;

use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result};

use crate::{
    code::{
        CodeDocument, CodeLayout, CodeLine, CodeSnapshot, CodeTransition, PlacedLine, StyledSpan,
        SyntaxStyle, TransitionProgress,
    },
    encode::{FfmpegEncoder, VideoSpec},
    render::{
        EditorFrame, HeadlessRenderer, PointerFrame, RenderSpec, TextRangeBounds, TokenHighlight,
    },
    timeline::{Animation, PropertyId, SpringProfile, Timeline},
};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const FPS: u32 = 60;
const DURATION_SECONDS: u32 = 3;
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
    let focus_line = settled_lines
        .iter()
        .find(|placed| placed.line.id.as_str() == FOCUS_LINE_ID)
        .expect("focus line is part of the compiled code manifest");
    let effect_bounds = renderer.measure_text_range(focus_line.line, "Effect.Effect")?;
    let not_found_bounds = renderer.measure_text_range(focus_line.line, "NotFound")?;
    let choreography = hero_choreography(effect_bounds, not_found_bounds, focus_line.y)?;
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
            let (panel_offset_y, focus_intensity, focus_line_y, token_highlight, pointer, lines) =
                sample_editor(&transition, &choreography, time);
            let sample_frame = EditorFrame {
                panel_offset_y,
                focus_intensity,
                focus_line_y,
                token_highlight,
                pointer,
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
}

fn hero_choreography(
    effect: TextRangeBounds,
    not_found: TextRangeBounds,
    focus_line_y: f32,
) -> Result<HeroChoreography> {
    let panel_y = PropertyId::new("editor.panel_y");
    let code_layout = PropertyId::new("editor.code_layout");
    let code_content = PropertyId::new("editor.code_content");
    let focus = PropertyId::new("editor.focus");
    let token_x = PropertyId::new("editor.token_highlight.x");
    let token_width = PropertyId::new("editor.token_highlight.width");
    let token_opacity = PropertyId::new("editor.token_highlight.opacity");
    let pointer_x = PropertyId::new("pointer.x");
    let pointer_y = PropertyId::new("pointer.y");
    let pointer_opacity = PropertyId::new("pointer.opacity");
    let profile = SpringProfile::from_visual_duration(0.3, 0.0, 0.001, 0.001);
    let effect_center = effect.x + effect.width * 0.5;
    let not_found_center = not_found.x + not_found.width * 0.5;
    let animation = Animation::parallel([
        Animation::delay(0.08, Animation::spring(panel_y.clone(), 0.0, profile)),
        Animation::delay(0.88, Animation::spring(code_layout.clone(), 1.0, profile)),
        Animation::delay(1.12, Animation::spring(code_content.clone(), 1.0, profile)),
        Animation::delay(1.55, Animation::spring(focus.clone(), 1.0, profile)),
        Animation::delay(1.68, Animation::spring(token_opacity.clone(), 1.0, profile)),
        Animation::delay(
            1.68,
            Animation::spring(pointer_opacity.clone(), 1.0, profile),
        ),
        Animation::delay(
            2.08,
            Animation::spring(token_x.clone(), not_found.x, profile),
        ),
        Animation::delay(
            2.08,
            Animation::spring(token_width.clone(), not_found.width, profile),
        ),
        Animation::delay(
            2.08,
            Animation::spring(pointer_x.clone(), not_found_center, profile),
        ),
    ]);
    let timeline = Timeline::compile(
        [
            (panel_y.clone(), 250.0),
            (code_layout.clone(), 0.0),
            (code_content.clone(), 0.0),
            (focus.clone(), 0.0),
            (token_x.clone(), effect.x),
            (token_width.clone(), effect.width),
            (token_opacity.clone(), 0.0),
            (pointer_x.clone(), effect_center),
            (pointer_y.clone(), focus_line_y + 55.0),
            (pointer_opacity.clone(), 0.0),
        ],
        &animation,
    )?;

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
    Vec<PlacedLine<'a>>,
) {
    let sample = |property| {
        choreography
            .timeline
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
        width: sample(&choreography.token_width),
        opacity: sample(&choreography.token_opacity).clamp(0.0, 1.0),
    };
    let pointer = PointerFrame {
        x: sample(&choreography.pointer_x),
        y: sample(&choreography.pointer_y),
        opacity: sample(&choreography.pointer_opacity).clamp(0.0, 1.0),
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
