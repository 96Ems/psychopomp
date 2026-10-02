//! Wipe transitions: a divider sweeps across the frame, the incoming segment
//! behind it and the outgoing one ahead. Holds rest the divider mid-frame so a
//! before/after comparison shows both segments at once, then the sweep goes on.
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::math::easing::smootherstep;

/// The divider's travel and optional side labels for a `wipe` transition.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReelWipePlan {
    #[serde(default, skip_serializing_if = "WipeDirection::is_default")]
    pub direction: WipeDirection,
    /// Rests on the way across: the divider eases to each `position` (a
    /// fraction of its travel) and holds there, then sweeps on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holds: Vec<WipeHoldPlan>,
    /// Labels for the outgoing and incoming sides, riding beside the divider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<[String; 2]>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WipeHoldPlan {
    pub position: f32,
    pub hold_nanos: u64,
}

/// Which way the divider travels. The incoming segment is behind it: a
/// `left` wipe leaves the outgoing frame on the left and the incoming on the
/// right, the usual before/after order.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WipeDirection {
    #[default]
    Right,
    Left,
    Down,
    Up,
}

impl WipeDirection {
    pub fn is_default(&self) -> bool {
        *self == Self::Right
    }
}

/// One layer's part in a wipe: the incoming segment is shown behind the
/// divider at `position` (0..1 of its travel).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WipePhase {
    pub position: f32,
    pub direction: WipeDirection,
}

impl ReelWipePlan {
    /// A wipe with no holds and no labels.
    pub fn new(direction: WipeDirection) -> Self {
        Self {
            direction,
            ..Self::default()
        }
    }

    /// Rest the divider at `position` for `hold_nanos` on its way across.
    pub fn hold(mut self, position: f32, hold_nanos: u64) -> Self {
        self.holds.push(WipeHoldPlan {
            position,
            hold_nanos,
        });
        self
    }

    pub fn labeled(mut self, outgoing: impl Into<String>, incoming: impl Into<String>) -> Self {
        self.labels = Some([outgoing.into(), incoming.into()]);
        self
    }

    pub fn validate(&self, transition_nanos: u64) -> Result<()> {
        let mut held = 0_u64;
        for hold in &self.holds {
            ensure!(
                hold.position.is_finite() && hold.position > 0.0 && hold.position < 1.0,
                "wipe holds rest strictly inside the frame (0 < position < 1)"
            );
            held = held.saturating_add(hold.hold_nanos);
        }
        if held >= transition_nanos {
            bail!("wipe holds must leave time in the transition to sweep");
        }
        for label in self.labels.iter().flatten() {
            ensure!(
                !label.trim().is_empty() && label.chars().count() <= 24 && !label.contains('\n'),
                "wipe labels are single short lines (1 to 24 characters)"
            );
        }
        Ok(())
    }

    /// Divider position `elapsed` seconds into a `duration`-second wipe. Each
    /// sweep between rests is minimum-jerk and takes time in proportion to its
    /// distance, so the divider keeps one pace and settles into every hold.
    pub fn position(&self, elapsed: f64, duration: f64) -> f32 {
        let held: f64 = self
            .holds
            .iter()
            .map(|hold| hold.hold_nanos as f64 / 1e9)
            .sum();
        let stops = std::iter::once(0.0)
            .chain(self.holds.iter().map(|hold| hold.position))
            .chain(std::iter::once(1.0))
            .collect::<Vec<_>>();
        let distance: f32 = stops.windows(2).map(|leg| (leg[1] - leg[0]).abs()).sum();
        let sweep = (duration - held).max(1e-9);
        let mut clock = elapsed.max(0.0);
        for (index, leg) in stops.windows(2).enumerate() {
            let length = sweep * f64::from((leg[1] - leg[0]).abs() / distance.max(1e-6));
            if clock < length {
                let t = (clock / length.max(1e-9)) as f32;
                return leg[0] + (leg[1] - leg[0]) * smootherstep(t);
            }
            clock -= length;
            let Some(hold) = self.holds.get(index) else {
                break;
            };
            let rest = hold.hold_nanos as f64 / 1e9;
            if clock < rest {
                return leg[1];
            }
            clock -= rest;
        }
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: u64 = 1_000_000_000;

    #[test]
    fn a_plain_wipe_sweeps_from_rest_to_rest() {
        let wipe = ReelWipePlan::default();
        assert_eq!(wipe.position(0.0, 1.0), 0.0);
        assert!((wipe.position(0.5, 1.0) - 0.5).abs() < 1e-6);
        assert_eq!(wipe.position(1.0, 1.0), 1.0);
        assert!(
            wipe.position(0.02, 1.0) < 0.001,
            "it leaves the edge gently"
        );
    }

    #[test]
    fn a_held_wipe_rests_mid_frame_then_sweeps_on() {
        // Two half-frame sweeps of 0.5 s around a 2 s hold.
        let wipe = ReelWipePlan::default().hold(0.5, 2 * SECOND);
        let duration = 3.0;
        assert!((wipe.position(0.25, duration) - 0.25).abs() < 1e-6);
        for t in [0.5, 1.0, 2.0, 2.5] {
            assert_eq!(wipe.position(t, duration), 0.5, "held at {t}");
        }
        assert!((wipe.position(2.75, duration) - 0.75).abs() < 1e-6);
        assert_eq!(wipe.position(3.0, duration), 1.0);
        // Sweep time follows distance: a hold at 0.25 spends a quarter of it first.
        let early = ReelWipePlan::default().hold(0.25, SECOND);
        assert_eq!(early.position(0.25, 2.0), 0.25);
        assert_eq!(early.position(1.25, 2.0), 0.25);
    }

    #[test]
    fn invalid_wipes_are_rejected() {
        assert!(ReelWipePlan::default().validate(SECOND).is_ok());
        assert!(
            ReelWipePlan::default()
                .hold(1.0, 1)
                .validate(SECOND)
                .is_err()
        );
        assert!(
            ReelWipePlan::default()
                .hold(0.5, SECOND)
                .validate(SECOND)
                .is_err(),
            "no time left to sweep"
        );
        assert!(
            ReelWipePlan::default()
                .labeled("", "after")
                .validate(SECOND)
                .is_err()
        );
    }

    #[test]
    fn wipe_json_is_compact_and_strict() {
        let wipe = ReelWipePlan::new(WipeDirection::Left)
            .hold(0.5, SECOND)
            .labeled("before", "after");
        let json = serde_json::to_value(&wipe).unwrap();
        assert_eq!(json["direction"], "left");
        assert_eq!(json["holds"][0]["holdNanos"], SECOND);
        assert_eq!(serde_json::from_value::<ReelWipePlan>(json).unwrap(), wipe);
        assert_eq!(
            serde_json::to_string(&ReelWipePlan::default()).unwrap(),
            "{}"
        );
    }
}
