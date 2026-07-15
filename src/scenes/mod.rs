use std::{path::Path, time::Instant};

use anyhow::{Context, Result, bail};

use kinograph::{
    code::{CodeLine, PlacedLine, StyledSpan, SyntaxStyle},
    dsl::{CompiledScene, Pointer, TargetGeometry},
    encode::{FfmpegEncoder, VideoSpec},
    render::{HeadlessRenderer, PointerFrame, TextRangeBounds},
};

pub(crate) mod effect_is_a_description;
pub(crate) mod effect_shows_errors;
pub(crate) mod hero;
pub(crate) mod opencode_hot_reload;
pub(crate) mod promises_only_happy_path;
pub(crate) mod visual_effects;

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const FPS: u32 = 60;
const TEMPORAL_SAMPLES: u32 = 8;
const ENTRANCE_TEMPORAL_SAMPLES: u32 = 16;
const SHUTTER_ANGLE: f32 = 180.0;
const FONT_PATH: &str = "/Users/kit/Library/Fonts/CommitMono-400-Regular.otf";
const _: () = assert!(TEMPORAL_SAMPLES > 0);

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
            if pixels.len() != frame_byte_count {
                bail!(
                    "renderer returned {} bytes for a {frame_byte_count}-byte RGBA frame",
                    pixels.len()
                );
            }

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

fn span(text: &str, style: SyntaxStyle) -> StyledSpan {
    StyledSpan::new(text, style)
}
