//! Subtitles: the recipe is decoded and its channels checked during
//! preflight; preparation measures the words and chunks them into timed
//! pages once.
use anyhow::Result;
use psychopomp::{
    plan::{ActorPlan, ContinuousChannelPlan},
    subtitles::{SubtitleLayout, SubtitlesPlan},
};

use super::super::preflight::{decode, strict_channels};
use crate::render::HeadlessRenderer;

pub(super) struct SubtitlesInput {
    id: String,
    plan: SubtitlesPlan,
}

impl SubtitlesInput {
    pub(super) fn new(actor: &ActorPlan, channels: &[ContinuousChannelPlan]) -> Result<Self> {
        let plan = decode(actor, "subtitles", SubtitlesPlan::validate)?;
        strict_channels(&actor.id, channels, "subtitles", |property| {
            matches!(property, "opacity" | "x" | "y" | "tilt")
        })?;
        Ok(Self {
            id: actor.id.clone(),
            plan,
        })
    }

    pub(super) fn prepare(self, renderer: &mut HeadlessRenderer) -> PreparedSubtitles {
        let layout = renderer.compile_subtitles(&self.plan);
        PreparedSubtitles {
            id: self.id,
            plan: self.plan,
            layout,
        }
    }
}

pub(super) struct PreparedSubtitles {
    id: String,
    plan: SubtitlesPlan,
    layout: SubtitleLayout,
}

impl PreparedSubtitles {
    pub(super) fn moving(&self, time: f64) -> bool {
        self.layout.moving(time)
    }

    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &mut HeadlessRenderer,
        time: f64,
        sample: impl Fn(&str, &str, f32) -> f32,
    ) {
        renderer.composite_subtitles(pixels, &self.plan, &self.layout, time, |property, d| {
            sample(&self.id, property, d)
        });
    }
}
