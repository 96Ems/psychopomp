use std::{path::Path, time::Instant};

use anyhow::{Context, Result, bail};

use kinograph::{
    code::{CodeLine, PlacedLine, StyledSpan, SyntaxStyle},
    composition::{Duration, MediaPlacement, Time, TimeRange},
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

/// Legacy scenes: `temporal_samples` per frame, `entrance_temporal_samples`
/// inside `high_sample_ranges` (scene seconds), averaged on the CPU.
pub(crate) fn encode_video_with_samples(
    renderer: &mut HeadlessRenderer,
    output: &Path,
    scene: &CompiledScene,
    temporal_samples: u32,
    entrance_temporal_samples: u32,
    high_sample_ranges: &[std::ops::Range<f32>],
    mut render_sample: impl FnMut(&mut HeadlessRenderer, f32) -> Result<Vec<u8>>,
) -> Result<()> {
    assert!(temporal_samples > 0 && entrance_temporal_samples > 0);
    let window = TimeRange::new(Time::ZERO, Time::ZERO.after(scene.duration()));
    encode_exposures(
        renderer,
        output,
        scene.duration(),
        scene.media(),
        window,
        |center| {
            if high_sample_ranges
                .iter()
                .any(|range| range.contains(&(center as f32)))
            {
                entrance_temporal_samples
            } else {
                temporal_samples
            }
        },
        |time| Ok(time.to_bits()),
        |renderer, exposure| {
            accumulate(renderer, exposure, |renderer, time| {
                render_sample(renderer, time as f32)
            })
        },
    )
}

/// Samples per frame for Scene Plans: more in the first second, where
/// entrances move fastest.
pub(crate) fn plan_temporal_samples(center: f64) -> u32 {
    if center < 1.0 {
        ENTRANCE_TEMPORAL_SAMPLES
    } else {
        TEMPORAL_SAMPLES
    }
}

/// Encode a timeline one exposed frame at a time. `samples_at` chooses how
/// many shutter samples a frame centered at a time takes; samples with equal
/// `sample_key`s merge their weights; `render_exposure` turns one frame's
/// weighted samples into pixels.
#[allow(clippy::too_many_arguments)]
pub(crate) fn encode_exposures<K: PartialEq>(
    renderer: &mut HeadlessRenderer,
    output: &Path,
    duration: Duration,
    media: &[MediaPlacement],
    window: TimeRange,
    mut samples_at: impl FnMut(f64) -> u32,
    mut sample_key: impl FnMut(f64) -> Result<K>,
    mut render_exposure: impl FnMut(&mut HeadlessRenderer, &[(f64, f32)]) -> Result<Vec<u8>>,
) -> Result<()> {
    if window.duration() == kinograph::composition::Duration::ZERO {
        bail!("render window must have positive duration");
    }
    let scene_end = Time::ZERO.after(duration);
    if window.end() > scene_end {
        bail!(
            "render window {}..{} exceeds scene duration {}",
            window.start(),
            window.end(),
            duration
        );
    }
    let started = Instant::now();
    let frame_count = window.duration().frame_count(FPS);
    let media = media
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
    for frame in 0..frame_count {
        let frame_start = window.start().as_seconds() + frame as f64 / f64::from(FPS);
        let frame_end = (window.start().as_seconds() + (frame + 1) as f64 / f64::from(FPS))
            .min(window.end().as_seconds());
        let center = (frame_start + frame_end) * 0.5;
        let samples = samples_at(center).max(1);
        let exposure = merge_equal_samples(
            exposure(center, frame_end - frame_start, samples),
            &mut sample_key,
        )?;
        encoder.write_frame(&render_exposure(renderer, &exposure)?)?;
        if frame % u64::from(FPS) == 0 || frame + 1 == frame_count {
            eprintln!(
                "Rendered {:>3}/{frame_count} frames ({center:.1}s, {samples} samples, {} unique)",
                frame + 1,
                exposure.len(),
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

/// One frame's shutter: `samples` stratified times across a 180-degree
/// shutter centered on `center` (clamped to `span`), with weights summing
/// to 1. The weights ease off over the outer quarter at each end, so a fast
/// highlight's streak fades out instead of ending on a hard copy.
pub(crate) fn exposure(center: f64, span: f64, samples: u32) -> Vec<(f64, f32)> {
    let shutter = (f64::from(SHUTTER_ANGLE) / 360.0 / f64::from(FPS)).min(span);
    let (start, end) = (center - span * 0.5, center + span * 0.5);
    let mut weighted = (0..samples)
        .map(|sample| {
            let phase = (f64::from(sample) + 0.5) / f64::from(samples) - 0.5;
            let edge = ((0.5 - phase.abs()) / 0.25).clamp(0.0, 1.0);
            let weight = if samples < 4 {
                1.0
            } else {
                edge * edge * (3.0 - 2.0 * edge)
            };
            ((center + phase * shutter).clamp(start, end), weight as f32)
        })
        .collect::<Vec<_>>();
    let total: f32 = weighted.iter().map(|(_, weight)| weight).sum();
    for (_, weight) in &mut weighted {
        *weight /= total;
    }
    weighted
}

/// Merge samples whose visual state is identical, keeping the first time and
/// the summed weight, so a still frame renders once.
pub(crate) fn merge_equal_samples<K: PartialEq>(
    samples: impl IntoIterator<Item = (f64, f32)>,
    mut sample_key: impl FnMut(f64) -> Result<K>,
) -> Result<Vec<(f64, f32)>> {
    let mut merged: Vec<(K, f64, f32)> = Vec::new();
    for (time, weight) in samples {
        let key = sample_key(time)?;
        match merged.iter_mut().find(|(candidate, ..)| candidate == &key) {
            Some((_, _, total)) => *total += weight,
            None => merged.push((key, time, weight)),
        }
    }
    Ok(merged
        .into_iter()
        .map(|(_, time, weight)| (time, weight))
        .collect())
}

/// Average sRGB frames in linear light by weight: the CPU exposure every
/// root supports.
pub(crate) fn accumulate(
    renderer: &mut HeadlessRenderer,
    exposure: &[(f64, f32)],
    mut render_sample: impl FnMut(&mut HeadlessRenderer, f64) -> Result<Vec<u8>>,
) -> Result<Vec<u8>> {
    if let [(time, _)] = exposure {
        return render_sample(renderer, *time);
    }
    let frame_byte_count = WIDTH as usize * HEIGHT as usize * 4;
    let tables = linear_tables();
    let mut sum = vec![0.0_f32; frame_byte_count];
    for &(time, weight) in exposure {
        let pixels = render_sample(renderer, time)?;
        if pixels.len() != frame_byte_count {
            bail!(
                "renderer returned {} bytes for a {frame_byte_count}-byte RGBA frame",
                pixels.len()
            );
        }
        for (sum, pixel) in sum.chunks_exact_mut(4).zip(pixels.chunks_exact(4)) {
            for channel in 0..3 {
                sum[channel] += tables.to_linear[pixel[channel] as usize] * weight;
            }
            sum[3] += f32::from(pixel[3]) / 255.0 * weight;
        }
    }
    Ok(sum
        .chunks_exact(4)
        .flat_map(|sum| {
            let encode =
                |linear: f32| tables.to_srgb[(linear.clamp(0.0, 1.0) * 65535.0).round() as usize];
            [
                encode(sum[0]),
                encode(sum[1]),
                encode(sum[2]),
                (sum[3] * 255.0).round() as u8,
            ]
        })
        .collect())
}

struct LinearTables {
    to_linear: [f32; 256],
    to_srgb: Vec<u8>,
}

fn linear_tables() -> &'static LinearTables {
    static TABLES: std::sync::OnceLock<LinearTables> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| LinearTables {
        to_linear: std::array::from_fn(|value| {
            let encoded = value as f32 / 255.0;
            if encoded <= 0.04045 {
                encoded / 12.92
            } else {
                ((encoded + 0.055) / 1.055).powf(2.4)
            }
        }),
        to_srgb: (0..65536)
            .map(|value| {
                let linear = value as f32 / 65535.0;
                let encoded = if linear <= 0.003_130_8 {
                    linear * 12.92
                } else {
                    1.055 * linear.powf(1.0 / 2.4) - 0.055
                };
                (encoded * 255.0).round() as u8
            })
            .collect(),
    })
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
    use super::{exposure, merge_equal_samples};

    #[test]
    fn partial_frame_samples_stay_inside_the_render_window() {
        let samples = exposure(12.0005, 0.001, 8);
        assert!(
            samples
                .iter()
                .all(|(time, _)| (12.0..=12.001).contains(time))
        );
        let total: f32 = samples.iter().map(|(_, weight)| weight).sum();
        assert!((total - 1.0).abs() < 1e-5);
    }

    #[test]
    fn the_shutter_eases_off_at_both_ends() {
        let samples = exposure(1.0, 1.0 / 60.0, 16);
        assert!(samples[0].1 < samples[8].1 * 0.2);
        assert!((samples[0].1 - samples[15].1).abs() < 1e-6, "symmetric");
        assert!(samples.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn identical_temporal_states_are_weighted_once() {
        let samples = merge_equal_samples(
            [(0.1, 0.25), (0.2, 0.25), (0.3, 0.25), (0.4, 0.25)],
            |time| Ok::<_, anyhow::Error>((time * 10.0_f64).round() as u32 % 2),
        )
        .unwrap();
        assert_eq!(samples, vec![(0.1, 0.5), (0.2, 0.5)]);
    }
}
