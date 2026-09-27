//! Small test primitives, not a universal scene-correctness runner. Each recipe
//! keeps its command sequence, offsets, tolerances and semantic assertions.
use super::PreparedPlan;
use crate::render::HeadlessRenderer;
use anyhow::Result;
use kinograph::{
    playback::{Playback, PlaybackCommand},
    timeline::{PropertyId, Timeline},
};
use std::{sync::Arc, time::Duration};

pub(super) struct Interruption {
    pub at: f64,
    pub before: Arc<Timeline>,
    pub after: Arc<Timeline>,
    pub pixels: Vec<u8>,
    pub changed: bool,
}
pub(super) fn interrupt(
    plan: &PreparedPlan,
    renderer: &mut HeadlessRenderer,
    playback: &mut Playback,
    now: Duration,
    command: PlaybackCommand,
) -> Result<Interruption> {
    let at = playback.sample(now).at_nanos as f64 / 1e9;
    let before = playback.timeline();
    let pixels = plan.render_sample_using(renderer, at, &before)?;
    let changed = playback.command(command, now);
    Ok(Interruption {
        at,
        before,
        after: playback.timeline(),
        pixels,
        changed,
    })
}
impl Interruption {
    pub(super) fn assert_states_within(&self, plan: &PreparedPlan, tolerance: f32) {
        for channel in &plan.plan.continuous_channels {
            let id = PropertyId::new(&channel.id);
            let a = self.before.sample_at(&id, self.at).unwrap();
            let b = self.after.sample_at(&id, self.at).unwrap();
            assert!(
                (a.position - b.position).abs() < tolerance
                    && (a.velocity - b.velocity).abs() < tolerance,
                "{}: {} at {}",
                plan.plan.id,
                channel.id,
                self.at
            );
        }
    }
    pub(super) fn assert_pixels(&self, plan: &PreparedPlan, renderer: &mut HeadlessRenderer) {
        assert!(
            self.pixels
                == plan
                    .render_sample_using(renderer, self.at, &self.after)
                    .unwrap(),
            "{} boundary pixels at {}",
            plan.plan.id,
            self.at
        );
    }
    pub(super) fn sample_later_and_repeat(
        &self,
        plan: &PreparedPlan,
        renderer: &mut HeadlessRenderer,
        offset: f64,
    ) -> Vec<u8> {
        let later = plan
            .render_sample_using(renderer, self.at + offset, &self.after)
            .unwrap();
        assert!(
            self.pixels
                == plan
                    .render_sample_using(renderer, self.at, &self.after)
                    .unwrap(),
            "{} out-of-order sample at {}",
            plan.plan.id,
            self.at
        );
        later
    }
}
