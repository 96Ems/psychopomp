//! Captions: short lines of styled CommitMono text in the terminal voice of an
//! explainer, with an optional typing reveal and block caret. Spans carry
//! semantic tones, so one keyword can take the accent while the rest stays plain.
use std::collections::HashMap;

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    author::{ActorHandle, ContinuousHandle, PlanBuilder},
    tone::Tone,
};

pub const CAPTION_RECIPE: &str = "caption";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptionPlan {
    /// Anchor point: `align` chooses which edge of each line sits on x, and y is
    /// the vertical center of the first line.
    pub origin: [f32; 2],
    #[serde(default, skip_serializing_if = "CaptionAlign::is_default")]
    pub align: CaptionAlign,
    pub size: f32,
    pub lines: Vec<Vec<CaptionSpanPlan>>,
    /// A rounded surface behind the text, as for a status chip.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub chip: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptionSpanPlan {
    pub text: String,
    #[serde(default, skip_serializing_if = "Tone::is_default")]
    pub tone: Tone,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptionAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl CaptionAlign {
    pub fn is_default(&self) -> bool {
        *self == Self::Left
    }
}

impl CaptionSpanPlan {
    pub fn new(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone,
        }
    }
}

impl CaptionPlan {
    /// One line of spans.
    pub fn line(origin: [f32; 2], size: f32, spans: Vec<CaptionSpanPlan>) -> Self {
        Self {
            origin,
            align: CaptionAlign::Left,
            size,
            lines: vec![spans],
            chip: false,
        }
    }

    pub fn aligned(mut self, align: CaptionAlign) -> Self {
        self.align = align;
        self
    }

    pub fn chip(mut self) -> Self {
        self.chip = true;
        self
    }

    pub fn line_height(&self) -> f32 {
        self.size * 1.45
    }

    /// Characters revealed by the `typed` channel, across all lines in order.
    pub fn char_count(&self) -> usize {
        self.lines
            .iter()
            .flatten()
            .map(|span| span.text.chars().count())
            .sum()
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.origin.iter().all(|v| v.is_finite()),
            "caption origin must be finite"
        );
        ensure!(
            (10.0..=120.0).contains(&self.size),
            "caption size must be between 10 and 120"
        );
        ensure!(
            (1..=6).contains(&self.lines.len()),
            "a caption has one to six lines"
        );
        for line in &self.lines {
            ensure!(
                line.iter().any(|span| !span.text.is_empty()),
                "caption lines cannot be empty"
            );
            for span in line {
                ensure!(
                    !span.text.contains('\n'),
                    "caption spans are single-line; use another line instead"
                );
            }
        }
        ensure!(
            self.char_count() <= 320,
            "captions are limited to 320 characters"
        );
        Ok(())
    }
}

/// Authoring handle for one caption actor. Channels are declared once, with the
/// recipe's defaults as initial values.
pub struct CaptionActor {
    actor: ActorHandle,
    chars: usize,
    channels: HashMap<String, ContinuousHandle>,
}

impl CaptionActor {
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: &CaptionPlan,
    ) -> Result<Self> {
        plan.validate()?;
        let actor = scene.actor(id, CAPTION_RECIPE, plan)?;
        Ok(Self {
            actor,
            chars: plan.char_count(),
            channels: HashMap::new(),
        })
    }

    pub fn actor(&self) -> &ActorHandle {
        &self.actor
    }

    pub fn channel(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        self.channels
            .entry(property.to_owned())
            .or_insert_with(|| scene.continuous(&self.actor, property, initial))
            .clone()
    }

    /// Fade and rise in. A caption with a `show` starts hidden.
    pub fn show(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        let opacity = self.channel(scene, "opacity", 0.0);
        let y = self.channel(scene, "y", 10.0);
        scene.spring(&opacity, at_nanos, 1.0, 0.35, 0.0);
        scene.spring(&y, at_nanos, 0.0, 0.45, 0.0);
    }

    /// Fade out in place.
    pub fn hide(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        let opacity = self.channel(scene, "opacity", 0.0);
        scene.spring(&opacity, at_nanos, 0.0, 0.3, 0.0);
    }

    /// Type the caption in at `chars_per_second`, showing the block caret while
    /// typing and for `caret_hold_seconds` afterward. The caption becomes
    /// visible when typing starts. Returns when typing finishes.
    pub fn type_in(
        &mut self,
        scene: &mut PlanBuilder,
        at_nanos: u64,
        chars_per_second: f32,
        caret_hold_seconds: f32,
    ) -> u64 {
        let opacity = self.channel(scene, "opacity", 0.0);
        let typed = self.channel(scene, "typed", 0.0);
        let caret = self.channel(scene, "caret", 0.0);
        scene.set(&opacity, at_nanos, 1.0);
        scene.set(&caret, at_nanos, 1.0);
        let per_char = (1e9 / f64::from(chars_per_second.max(1.0))) as u64;
        for index in 1..=self.chars {
            scene.set(
                &typed,
                at_nanos + per_char * index as u64,
                index as f32 / self.chars as f32,
            );
        }
        let done = at_nanos + per_char * self.chars as u64;
        let caret_off = done + (f64::from(caret_hold_seconds.max(0.0)) * 1e9) as u64;
        scene.spring(&caret, caret_off, 0.0, 0.2, 0.0);
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> CaptionPlan {
        CaptionPlan::line(
            [140.0, 960.0],
            30.0,
            vec![
                CaptionSpanPlan::new("one server", Tone::Accent),
                CaptionSpanPlan::new(". every client.", Tone::Plain),
            ],
        )
    }

    #[test]
    fn captions_round_trip_with_compact_defaults() {
        let plan = plan();
        plan.validate().unwrap();
        assert_eq!(plan.char_count(), 25);
        let json = serde_json::to_value(&plan).unwrap();
        assert!(json.get("align").is_none() && json.get("chip").is_none());
        assert_eq!(json["lines"][0][0]["tone"], "accent");
        assert!(json["lines"][0][1].get("tone").is_none());
        assert_eq!(serde_json::from_value::<CaptionPlan>(json).unwrap(), plan);
    }

    #[test]
    fn invalid_captions_are_rejected() {
        let mut empty = plan();
        empty.lines = vec![vec![CaptionSpanPlan::new("", Tone::Plain)]];
        assert!(empty.validate().is_err());
        let mut newline = plan();
        newline.lines[0][0].text = "a\nb".into();
        assert!(newline.validate().is_err());
        let mut huge = plan();
        huge.size = 400.0;
        assert!(huge.validate().is_err());
    }

    #[test]
    fn typing_writes_one_exact_step_per_character() {
        let mut scene = PlanBuilder::new("caption-demo", 10_000_000_000);
        let mut caption = CaptionActor::declare(&mut scene, "line", &plan()).unwrap();
        let done = caption.type_in(&mut scene, 1_000_000_000, 50.0, 0.5);
        assert_eq!(done, 1_000_000_000 + 25 * 20_000_000);
        let plan = scene.finish().unwrap();
        let typed = plan
            .continuous_channels
            .iter()
            .find(|channel| channel.property == "typed")
            .unwrap();
        assert_eq!(typed.events.len(), 25);
    }
}
