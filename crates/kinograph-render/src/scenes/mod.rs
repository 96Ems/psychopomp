use std::{path::Path, time::Instant};

use anyhow::{Context, Result, bail};

use kinograph::{
    code::{CodeLine, PlacedLine, StyledSpan, SyntaxStyle},
    composition::{Time, TimeRange},
    dsl::{CompiledScene, Pointer, TargetGeometry},
};

use crate::{
    encode::{FfmpegEncoder, VideoSpec},
    render::{HeadlessRenderer, PointerFrame, TextRangeBounds},
};

pub(crate) mod effect_institute;
pub(crate) mod effect_is_a_description;
pub(crate) mod effect_shows_errors;
pub(crate) mod opencode_hot_reload;
pub(crate) mod promises_only_happy_path;
pub(crate) mod visual_effects;

pub(crate) const WIDTH: u32 = 1920;
pub(crate) const HEIGHT: u32 = 1080;
const FPS: u32 = 60;
const TEMPORAL_SAMPLES: u32 = 8;
const ENTRANCE_TEMPORAL_SAMPLES: u32 = 16;
const SHUTTER_ANGLE: f32 = 180.0;
pub(crate) const FONT_PATH: &str = "/Users/kit/Library/Fonts/CommitMono-400-Regular.otf";
const WORKSPACE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const _: () = assert!(TEMPORAL_SAMPLES > 0);

pub(crate) fn encode_video(
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

pub(crate) fn encode_video_with_samples(
    renderer: &mut HeadlessRenderer,
    output: &Path,
    scene: &CompiledScene,
    temporal_samples: u32,
    entrance_temporal_samples: u32,
    high_sample_ranges: &[std::ops::Range<f32>],
    mut render_sample: impl FnMut(&mut HeadlessRenderer, f32) -> Result<Vec<u8>>,
) -> Result<()> {
    let window = TimeRange::new(Time::ZERO, Time::ZERO.after(scene.duration()));
    encode_video_window_with_samples(
        renderer,
        output,
        scene,
        window,
        temporal_samples,
        entrance_temporal_samples,
        high_sample_ranges,
        |renderer, time| render_sample(renderer, time as f32),
    )
}

pub(crate) fn encode_video_window_by_key<K: PartialEq>(
    renderer: &mut HeadlessRenderer,
    output: &Path,
    scene: &CompiledScene,
    window: TimeRange,
    sample_key: impl FnMut(f64) -> Result<K>,
    render_sample: impl FnMut(&mut HeadlessRenderer, f64) -> Result<Vec<u8>>,
) -> Result<()> {
    encode_video_window_with_samples_by_key(
        renderer,
        output,
        scene,
        window,
        TEMPORAL_SAMPLES,
        ENTRANCE_TEMPORAL_SAMPLES,
        &[0.0..1.0],
        sample_key,
        render_sample,
    )
}

#[allow(clippy::too_many_arguments)]
fn encode_video_window_with_samples(
    renderer: &mut HeadlessRenderer,
    output: &Path,
    scene: &CompiledScene,
    window: TimeRange,
    temporal_samples: u32,
    entrance_temporal_samples: u32,
    high_sample_ranges: &[std::ops::Range<f32>],
    render_sample: impl FnMut(&mut HeadlessRenderer, f64) -> Result<Vec<u8>>,
) -> Result<()> {
    encode_video_window_with_samples_by_key(
        renderer,
        output,
        scene,
        window,
        temporal_samples,
        entrance_temporal_samples,
        high_sample_ranges,
        |time| Ok(time.to_bits()),
        render_sample,
    )
}

#[allow(clippy::too_many_arguments)]
fn encode_video_window_with_samples_by_key<K: PartialEq>(
    renderer: &mut HeadlessRenderer,
    output: &Path,
    scene: &CompiledScene,
    window: TimeRange,
    temporal_samples: u32,
    entrance_temporal_samples: u32,
    high_sample_ranges: &[std::ops::Range<f32>],
    mut sample_key: impl FnMut(f64) -> Result<K>,
    mut render_sample: impl FnMut(&mut HeadlessRenderer, f64) -> Result<Vec<u8>>,
) -> Result<()> {
    assert!(temporal_samples > 0 && entrance_temporal_samples > 0);
    if window.duration() == kinograph::composition::Duration::ZERO {
        bail!("render window must have positive duration");
    }
    let scene_end = Time::ZERO.after(scene.duration());
    if window.end() > scene_end {
        bail!(
            "render window {}..{} exceeds scene duration {}",
            window.start(),
            window.end(),
            scene.duration()
        );
    }
    let started = Instant::now();
    let frame_count = window.duration().frame_count(FPS);
    let media = scene
        .media()
        .iter()
        .filter_map(|placement| placement.for_window(window))
        .collect::<Vec<_>>();
    let mut encoder = FfmpegEncoder::start_with_media(
        output,
        VideoSpec {
            width: WIDTH,
            height: HEIGHT,
            fps: FPS,
        },
        &media,
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
        let frame_start = window.start().as_seconds() + frame as f64 / f64::from(FPS);
        let frame_end = (window.start().as_seconds() + (frame + 1) as f64 / f64::from(FPS))
            .min(window.end().as_seconds());
        let center_time = (frame_start + frame_end) * 0.5;
        let frame_temporal_samples = if high_sample_ranges
            .iter()
            .any(|range| range.contains(&(center_time as f32)))
        {
            entrance_temporal_samples
        } else {
            temporal_samples
        };
        let samples = unique_sample_times(
            temporal_sample_times(frame_start, frame_end, frame_temporal_samples),
            &mut sample_key,
        )?;
        let unique_samples = samples.len();
        for (time, multiplicity) in samples {
            let pixels = render_sample(renderer, time)?;
            if pixels.len() != frame_byte_count {
                bail!(
                    "renderer returned {} bytes for a {frame_byte_count}-byte RGBA frame",
                    pixels.len()
                );
            }

            let weight = multiplicity as f32;
            for (sum, pixel) in accumulation.chunks_exact_mut(4).zip(pixels.chunks_exact(4)) {
                sum[0] += srgb_to_linear[pixel[0] as usize] * weight;
                sum[1] += srgb_to_linear[pixel[1] as usize] * weight;
                sum[2] += srgb_to_linear[pixel[2] as usize] * weight;
                sum[3] += f32::from(pixel[3]) / 255.0 * weight;
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
            eprintln!(
                "Rendered {:>3}/{frame_count} frames ({:.1}s, {frame_temporal_samples} samples, {unique_samples} unique)",
                frame + 1,
                center_time,
            );
        }
    }

    encoder.finish()?;
    eprintln!(
        "Wrote {} in {:.1}s",
        output.display(),
        started.elapsed().as_secs_f32()
    );
    Ok(())
}

fn unique_sample_times<K: PartialEq>(
    times: impl IntoIterator<Item = f64>,
    mut sample_key: impl FnMut(f64) -> Result<K>,
) -> Result<Vec<(f64, u32)>> {
    let mut samples: Vec<(K, f64, u32)> = Vec::new();
    for time in times {
        let key = sample_key(time)?;
        if let Some((_, _, multiplicity)) = samples
            .iter_mut()
            .find(|(candidate, _, _)| candidate == &key)
        {
            *multiplicity += 1;
        } else {
            samples.push((key, time, 1));
        }
    }
    Ok(samples
        .into_iter()
        .map(|(_, time, multiplicity)| (time, multiplicity))
        .collect())
}

fn temporal_sample_times(frame_start: f64, frame_end: f64, samples: u32) -> Vec<f64> {
    let center = (frame_start + frame_end) * 0.5;
    let shutter = (f64::from(SHUTTER_ANGLE) / 360.0 / f64::from(FPS)).min(frame_end - frame_start);
    (0..samples)
        .map(|sample| {
            let phase = (f64::from(sample) + 0.5) / f64::from(samples) - 0.5;
            (center + phase * shutter).clamp(frame_start, frame_end)
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::{temporal_sample_times, unique_sample_times};

    #[test]
    fn partial_frame_samples_stay_inside_the_render_window() {
        let samples = temporal_sample_times(12.0, 12.001, 8);

        assert!(samples.iter().all(|time| (12.0..=12.001).contains(time)));
        assert!(samples[0] > 12.0);
        assert!(samples[7] < 12.001);
    }

    #[test]
    fn identical_temporal_states_are_weighted_once() {
        let samples = unique_sample_times([0.1, 0.2, 0.3, 0.4], |time| {
            Ok::<_, anyhow::Error>((time * 10.0_f64).round() as u32 % 2)
        })
        .unwrap();

        assert_eq!(samples, vec![(0.1, 2), (0.2, 2)]);
    }
}
