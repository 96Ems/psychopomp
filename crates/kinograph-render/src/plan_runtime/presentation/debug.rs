//! Native-only diagnostics. The snapshot travels with the exact rendered sample,
//! never a later UI clock reading. It is painted over (not into) cached scene pixels.
use super::super::PreparedPlan;
use crate::render::HeadlessRenderer;
use kinograph::{
    playback::{PendingStarts, Playback, PlaybackSample, PlaybackSpeed},
    timeline::Timeline,
};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DebugState {
    speed: PlaybackSpeed,
    navigation_start: Duration,
    pending: PendingStarts,
}

impl DebugState {
    pub(super) fn capture(playback: &Playback, sample: PlaybackSample) -> Self {
        Self {
            speed: playback.speed(),
            navigation_start: playback.navigation_start(),
            pending: playback.pending_starts(Duration::from_nanos(sample.at_nanos)),
        }
    }
    pub(super) fn paint(
        self,
        prepared: &PreparedPlan,
        renderer: &mut HeadlessRenderer,
        pixels: &mut [u8],
        sample: PlaybackSample,
        timeline: &Timeline,
    ) {
        let at = Duration::from_nanos(sample.at_nanos);
        let elapsed = at.saturating_sub(self.navigation_start);
        let mut lines = vec![
            format!(
                "DEBUG  {}  | local {:.3}s  | nav +{:.1}ms  | {:?}  | step {}/{}",
                self.speed.label(),
                at.as_secs_f64(),
                elapsed.as_secs_f64() * 1000.,
                sample.phase,
                sample.step_index + 1,
                prepared.plan.presentation_steps.len()
            ),
            format!(
                "Pending starts: {}  | next: {} (scene time)",
                self.pending.count,
                self.pending.until_next.map_or_else(
                    || "none".into(),
                    |d| format!("+{:.1}ms", d.as_secs_f64() * 1000.)
                )
            ),
        ];
        for header in prepared.headers.iter().take(4) {
            lines.push(header.debug_line(|a, p, d| {
                prepared.property_value(timeline, a, p, at.as_secs_f64(), d)
            }));
        }
        lines.push(
            "S / Shift+S speed   P pause   , / . frame-step   Shift+R replay paused   D hide"
                .into(),
        );
        renderer.composite_debug_hud(pixels, &lines);
    }
}
