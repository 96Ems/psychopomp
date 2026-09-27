//! A reel plays independently prepared Scene Plans on one clock. Each segment keeps
//! its own actors and local time; crossfades blend the outgoing and incoming frames,
//! and every segment's media is retimed onto the reel clock for one audio mix.
use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use kinograph::{
    composition::{Duration, MediaPlacement, Time, TimeRange},
    plan::ReelPlan,
};
use serde_json::{Value, json};

use super::{PreparedPlan, VisualSampleKey, delivery, inspect_plan, validate_renderer_plan};
use crate::render::{HeadlessRenderer, Theme};

/// Reels are recognized by their `segments` key, as decks are by `slides`.
pub(super) fn is_reel(path: &Path) -> Result<bool> {
    let json =
        fs::read_to_string(path).with_context(|| format!("read plan file {}", path.display()))?;
    let value: Value =
        serde_json::from_str(&json).with_context(|| format!("parse JSON {}", path.display()))?;
    Ok(value.get("segments").is_some())
}

pub(super) fn read(path: &Path) -> Result<ReelPlan> {
    let json = fs::read_to_string(path).with_context(|| format!("read reel {}", path.display()))?;
    let reel: ReelPlan =
        serde_json::from_str(&json).with_context(|| format!("parse reel {}", path.display()))?;
    reel.validate()?;
    Ok(reel)
}

pub(super) fn validate(reel: &ReelPlan) -> Result<()> {
    for segment in &reel.segments {
        validate_renderer_plan(&segment.plan)
            .with_context(|| format!("reel segment '{}'", segment.plan.id))?;
    }
    Ok(())
}

pub(super) fn inspect(reel: &ReelPlan) -> Value {
    let segments = reel
        .segments
        .iter()
        .zip(reel.spans())
        .map(|(segment, span)| {
            json!({
                "startNanos": span.start_nanos,
                "endNanos": span.end_nanos,
                "transitionNanos": span.transition_nanos,
                "plan": inspect_plan(&segment.plan),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "id": reel.id,
        "version": reel.version,
        "durationNanos": reel.duration_nanos(),
        "segments": segments,
    })
}

/// `--cue` on a reel selects one whole segment by its scene ID.
pub(super) fn segment_window(reel: &ReelPlan, id: &str) -> Result<TimeRange> {
    let (_, span) = reel
        .segments
        .iter()
        .zip(reel.spans())
        .find(|(segment, _)| segment.plan.id == id)
        .with_context(|| format!("reel has no segment '{id}'"))?;
    Ok(TimeRange::new(
        Time::from_nanos(span.start_nanos),
        Time::from_nanos(span.end_nanos),
    ))
}

pub(super) struct PreparedReel {
    reel: ReelPlan,
    segments: Vec<PreparedPlan>,
    media: Vec<MediaPlacement>,
}

#[derive(Debug, PartialEq)]
pub(super) struct ReelSampleKey(Vec<(usize, u32, VisualSampleKey)>);

impl PreparedReel {
    pub(super) fn prepare(
        reel: ReelPlan,
        base: &Path,
        renderer: &mut HeadlessRenderer,
    ) -> Result<Self> {
        let spans = reel.spans();
        let mut segments = Vec::with_capacity(reel.segments.len());
        let mut media = Vec::new();
        for (segment, span) in reel.segments.iter().zip(spans) {
            let prepared = PreparedPlan::prepare(segment.plan.clone(), base, renderer)
                .with_context(|| format!("prepare reel segment '{}'", segment.plan.id))?;
            let offset = Duration::from_nanos(span.start_nanos);
            media.extend(
                prepared
                    .scene
                    .media()
                    .iter()
                    .map(|placement| placement.shifted(offset)),
            );
            segments.push(prepared);
        }
        Ok(Self {
            reel,
            segments,
            media,
        })
    }

    pub(super) fn duration(&self) -> Duration {
        Duration::from_nanos(self.reel.duration_nanos())
    }

    pub(super) fn media(&self) -> &[MediaPlacement] {
        &self.media
    }

    pub(super) fn visual_sample_key(&self, time: f64) -> Result<ReelSampleKey> {
        self.reel
            .layers_at(time)
            .into_iter()
            .map(|layer| {
                Ok((
                    layer.segment,
                    layer.weight.to_bits(),
                    self.segments[layer.segment].visual_sample_key(layer.local_seconds)?,
                ))
            })
            .collect::<Result<Vec<_>>>()
            .map(ReelSampleKey)
    }

    pub(super) fn render_sample(
        &self,
        renderer: &mut HeadlessRenderer,
        time: f64,
    ) -> Result<Vec<u8>> {
        let layers = self.reel.layers_at(time);
        let mut blended = match layers.first() {
            Some(first) if first.weight >= 1.0 => None,
            // A dip, or an instant with nothing visible, starts from the theme's
            // empty background.
            _ => Some(renderer.render_title_card("", None, 0.0)),
        };
        for layer in layers {
            if layer.weight <= 0.0 {
                continue;
            }
            let prepared = &self.segments[layer.segment];
            renderer.set_file_name(prepared.file_name());
            let pixels = prepared.render_sample(renderer, layer.local_seconds)?;
            blended = Some(match blended {
                None => pixels,
                Some(mut below) => {
                    crossfade(&mut below, &pixels, layer.weight);
                    below
                }
            });
        }
        blended.context("reel has no visible segment at this time")
    }
}

/// Mix the opaque `above` frame over the opaque `below` frame by `weight`.
fn crossfade(below: &mut [u8], above: &[u8], weight: f32) {
    let weight = weight.clamp(0.0, 1.0);
    for (dst, src) in below.iter_mut().zip(above) {
        let mixed = f32::from(*dst) + (f32::from(*src) - f32::from(*dst)) * weight;
        *dst = mixed.round() as u8;
    }
}

pub(super) async fn render(
    reel: ReelPlan,
    base: &Path,
    output: &Path,
    window: Option<TimeRange>,
    theme: Theme,
) -> Result<()> {
    let mut renderer = super::new_renderer(&reel.id).await?;
    renderer.set_theme(theme);
    let prepared = PreparedReel::prepare(reel, base, &mut renderer)?;
    let window =
        window.unwrap_or_else(|| TimeRange::new(Time::ZERO, Time::ZERO.after(prepared.duration())));
    delivery::render_reel(&prepared, &mut renderer, output, window)
}

pub(super) async fn frame(
    reel: ReelPlan,
    base: &Path,
    output: &Path,
    at: Time,
    theme: Theme,
) -> Result<()> {
    if at.as_nanos() > reel.duration_nanos() {
        bail!(
            "frame time {} exceeds reel duration {}",
            at,
            Time::from_nanos(reel.duration_nanos())
        );
    }
    let mut renderer = super::new_renderer(&reel.id).await?;
    renderer.set_theme(theme);
    let prepared = PreparedReel::prepare(reel, base, &mut renderer)?;
    let pixels = prepared.render_sample(&mut renderer, at.as_seconds())?;
    delivery::write_png(output, &pixels)
}

#[cfg(test)]
mod tests {
    use super::crossfade;

    #[test]
    fn crossfade_mixes_linearly_between_opaque_frames() {
        let mut below = vec![0, 100, 200, 255];
        crossfade(&mut below, &[200, 100, 0, 255], 0.25);
        assert_eq!(below, vec![50, 100, 150, 255]);
        let mut unchanged = vec![10, 20, 30, 255];
        crossfade(&mut unchanged, &[250, 250, 250, 255], 0.0);
        assert_eq!(unchanged, vec![10, 20, 30, 255]);
    }
}
