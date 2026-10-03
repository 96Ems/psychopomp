//! Frame exposure: the output format, how many shutter samples a frame takes,
//! their weights across a 180-degree shutter, and encoding a timeline one
//! exposed frame at a time. Every root and the legacy scenes share it.
use std::{ops::Range, path::Path, time::Instant};

use anyhow::{Result, bail};
use psychopomp::{
    composition::{Duration, MediaPlacement, Time, TimeRange},
    math::{Vec2, shapes::Box2, vec2},
};

use crate::{
    encode::{FfmpegEncoder, VideoSpec},
    render::HeadlessRenderer,
};

pub(crate) const WIDTH: u32 = 1920;
pub(crate) const HEIGHT: u32 = 1080;
const FPS: u32 = 60;
const TEMPORAL_SAMPLES: u32 = 8;
const ENTRANCE_TEMPORAL_SAMPLES: u32 = 16;
const SHUTTER_ANGLE: f32 = 180.0;

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
    if window.duration() == psychopomp::composition::Duration::ZERO {
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
    let tables = linear_tables();
    let mut sum = vec![0.0_f32; FRAME_BYTES];
    for &(time, weight) in exposure {
        let pixels = render_sample(renderer, time)?;
        check_frame(&pixels)?;
        for (sum, pixel) in sum.chunks_exact_mut(4).zip(pixels.chunks_exact(4)) {
            add_linear(tables, sum, pixel, weight);
        }
    }
    Ok(sum
        .chunks_exact(4)
        .flat_map(|sum| encode_linear(tables, sum))
        .collect())
}

/// `accumulate` for samples that differ only inside `region`. `first` is the
/// first sample's frame; `render_sample` repaints each later sample into a
/// frame that holds an earlier one, and only its `region` is read back.
/// Outside `region` every pixel takes the same weighted average through a
/// per-value table, so the result is bit-identical to `accumulate`.
pub(crate) fn accumulate_region(
    exposure: &[(f64, f32)],
    region: &Region,
    first: Vec<u8>,
    mut render_sample: impl FnMut(&mut [u8], f64) -> Result<()>,
) -> Result<Vec<u8>> {
    check_frame(&first)?;
    if exposure.len() == 1 {
        return Ok(first);
    }
    let tables = linear_tables();
    let mut frame = first.clone();
    let mut sum = vec![0.0_f32; region.pixel_count() * 4];
    for (index, &(time, weight)) in exposure.iter().enumerate() {
        if index > 0 {
            render_sample(&mut frame, time)?;
        }
        let pixels = region.rows().flat_map(|row| frame[row].chunks_exact(4));
        for (sum, pixel) in sum.chunks_exact_mut(4).zip(pixels) {
            add_linear(tables, sum, pixel, weight);
        }
    }
    let constant: [[u8; 4]; 256] = std::array::from_fn(|value| {
        let mut sum = [0.0_f32; 4];
        for &(_, weight) in exposure {
            add_linear(tables, &mut sum, &[value as u8; 4], weight);
        }
        encode_linear(tables, &sum)
    });
    let mut exposed = first;
    for pixel in exposed.chunks_exact_mut(4) {
        for (channel, value) in pixel.iter_mut().enumerate() {
            *value = constant[*value as usize][channel];
        }
    }
    let mut sums = sum.chunks_exact(4);
    for row in region.rows() {
        for (pixel, sum) in exposed[row].chunks_exact_mut(4).zip(&mut sums) {
            pixel.copy_from_slice(&encode_linear(tables, sum));
        }
    }
    Ok(exposed)
}

/// The frame pixels some set of boxes touches, as disjoint row spans.
pub(crate) struct Region {
    rows: Vec<Range<usize>>,
}

impl Region {
    /// The frame pixels any of `bounds` touches; empty for none.
    pub(crate) fn covering(bounds: impl IntoIterator<Item = Box2>) -> Self {
        let frame = vec2(WIDTH as f32, HEIGHT as f32);
        let boxes = bounds
            .into_iter()
            .map(|bounds| {
                let min = bounds.min.floor().clamp(Vec2::ZERO, frame);
                let max = bounds.max.ceil().clamp(min, frame);
                (
                    min.x as usize..max.x as usize,
                    min.y as usize..max.y as usize,
                )
            })
            .collect::<Vec<_>>();
        let mut rows = Vec::new();
        for y in 0..HEIGHT as usize {
            let mut spans = boxes
                .iter()
                .filter(|(x, rows)| rows.contains(&y) && !x.is_empty())
                .map(|(x, _)| x.clone())
                .collect::<Vec<_>>();
            spans.sort_by_key(|span| span.start);
            let row = y * WIDTH as usize;
            let mut spans = spans.into_iter();
            let Some(mut merged) = spans.next() else {
                continue;
            };
            for span in spans {
                if span.start <= merged.end {
                    merged.end = merged.end.max(span.end);
                } else {
                    rows.push((row + merged.start) * 4..(row + merged.end) * 4);
                    merged = span;
                }
            }
            rows.push((row + merged.start) * 4..(row + merged.end) * 4);
        }
        Self { rows }
    }

    fn pixel_count(&self) -> usize {
        self.rows.iter().map(|row| row.len() / 4).sum()
    }

    /// Each span's byte range in an RGBA frame.
    fn rows(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        self.rows.iter().cloned()
    }

    /// Copy this region of `from` into `to`.
    pub(crate) fn copy(&self, from: &[u8], to: &mut [u8]) {
        for row in self.rows() {
            to[row.clone()].copy_from_slice(&from[row]);
        }
    }
}

const FRAME_BYTES: usize = WIDTH as usize * HEIGHT as usize * 4;

fn check_frame(pixels: &[u8]) -> Result<()> {
    if pixels.len() != FRAME_BYTES {
        bail!(
            "renderer returned {} bytes for a {FRAME_BYTES}-byte RGBA frame",
            pixels.len()
        );
    }
    Ok(())
}

#[inline]
fn add_linear(tables: &LinearTables, sum: &mut [f32], pixel: &[u8], weight: f32) {
    for channel in 0..3 {
        sum[channel] += tables.to_linear[pixel[channel] as usize] * weight;
    }
    sum[3] += f32::from(pixel[3]) / 255.0 * weight;
}

#[inline]
fn encode_linear(tables: &LinearTables, sum: &[f32]) -> [u8; 4] {
    let encode = |linear: f32| tables.to_srgb[(linear.clamp(0.0, 1.0) * 65535.0).round() as usize];
    [
        encode(sum[0]),
        encode(sum[1]),
        encode(sum[2]),
        (sum[3] * 255.0).round() as u8,
    ]
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

#[cfg(test)]
mod tests {
    use psychopomp::math::{shapes::Box2, vec2};

    use super::{
        FRAME_BYTES, Region, accumulate_region, add_linear, encode_linear, exposure, linear_tables,
        merge_equal_samples,
    };

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
    fn region_exposure_matches_the_whole_frame_bit_for_bit() {
        let boxes = [
            Box2 {
                min: vec2(100.4, 50.0),
                max: vec2(300.0, 90.6),
            },
            Box2 {
                min: vec2(250.0, 80.0),
                max: vec2(420.2, 140.0),
            },
            Box2 {
                min: vec2(1800.0, 1000.0),
                max: vec2(2100.0, 1200.0),
            },
        ];
        let region = Region::covering(boxes);
        let base = (0..FRAME_BYTES)
            .map(|index| (index * 7 % 251) as u8)
            .collect::<Vec<_>>();
        let sample = |time: f64| {
            let mut frame = base.clone();
            for row in region.rows() {
                for (offset, value) in frame[row.clone()].iter_mut().enumerate() {
                    *value = value.wrapping_add((time * 97.0) as u8 ^ (row.start + offset) as u8);
                }
            }
            frame
        };
        let samples = exposure(2.0, 1.0 / 60.0, 7);

        let tables = linear_tables();
        let mut sum = vec![0.0_f32; FRAME_BYTES];
        for &(time, weight) in &samples {
            for (sum, pixel) in sum.chunks_exact_mut(4).zip(sample(time).chunks_exact(4)) {
                add_linear(tables, sum, pixel, weight);
            }
        }
        let whole = sum
            .chunks_exact(4)
            .flat_map(|sum| encode_linear(tables, sum))
            .collect::<Vec<_>>();

        let exposed = accumulate_region(&samples, &region, sample(samples[0].0), |frame, time| {
            region.copy(&sample(time), frame);
            Ok(())
        })
        .unwrap();
        assert!(exposed == whole);
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
