//! The OpenCode blog's radial activity spinner (`spinnerPhysics.ts` and
//! `spinnerCompletion.ts`), sampled in closed form. A critically damped motor
//! builds to cruise and coasts to rest; its wake follows actual speed. Handed
//! off at a top-right crossing, one tip draws a fixed mark route: the ring's
//! wake drains while a tangent turn hooks into the mark's first arm.
//!
//! Poses live in a 16-unit box centered on (8, 8), the web rig's coordinates.
use std::f32::consts::PI;

use crate::math::{
    Vec2,
    curve::{CubicBezier, Polyline},
    smoothstep, vec2,
};

/// Cruise speed in degrees per second.
pub const CRUISE: f32 = 900.0;
/// Ring radius in box units.
pub const RADIUS: f32 = 6.5;
/// Duration of the mark's stroke in seconds.
pub const DRAW: f32 = 0.46;
/// The drawn mark's flash cools back to resting ink over this many seconds.
pub const COOL: f32 = 1.3;
/// Where the mark route begins, in degrees clockwise from twelve o'clock.
pub const HANDOFF_ANGLE: f32 = 45.0;

fn omega() -> f32 {
    9.0 / 1.2_f32.sqrt()
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Motor {
    angle: f32,
    velocity: f32,
    acceleration: f32,
}

/// From rest toward cruise: a critically damped response of velocity.
fn loading(t: f32) -> Motor {
    let w = omega();
    let t = t.max(0.0);
    let e = (-w * t).exp();
    Motor {
        angle: CRUISE * (t - 2.0 / w + (t + 2.0 / w) * e),
        velocity: CRUISE * (1.0 - (1.0 + w * t) * e),
        acceleration: CRUISE * w * w * t * e,
    }
}

/// Coasting to rest `s` seconds after releasing at motor age `at`, keeping the
/// released velocity and acceleration so the force never jumps.
fn released(at: f32, s: f32) -> Motor {
    let w = omega();
    let start = loading(at);
    let s = s.max(0.0);
    let e = (-w * s).exp();
    let a = start.velocity;
    let b = start.acceleration + w * start.velocity;
    Motor {
        angle: start.angle + a * (1.0 - e) / w + b * (1.0 - (1.0 + w * s) * e) / (w * w),
        velocity: (a + b * s) * e,
        acceleration: (b - w * (a + b * s)) * e,
    }
}

fn wake(velocity: f32) -> f32 {
    (170.0 * (1.0 - (-velocity.max(0.0) / 400.0).exp())).min(250.0)
}

fn presence(velocity: f32) -> f32 {
    smoothstep(velocity / 360.0)
}

fn circle(angle: f32) -> Vec2 {
    let radians = (angle - 90.0).to_radians();
    vec2(8.0, 8.0) + vec2(radians.cos(), radians.sin()) * RADIUS
}

/// The motor age at the first handoff crossing at or after `after`.
pub fn handoff(after: f32) -> f32 {
    let angle = loading(after).angle;
    let target = HANDOFF_ANGLE + 360.0 * ((angle - HANDOFF_ANGLE) / 360.0).ceil();
    let (mut low, mut high) = (after, after + 1.0);
    while loading(high).angle < target {
        high += 1.0;
    }
    for _ in 0..48 {
        let middle = 0.5 * (low + high);
        if loading(middle).angle < target {
            low = middle;
        } else {
            high = middle;
        }
    }
    high
}

/// The shape a completed spinner resolves into.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mark {
    #[default]
    Check,
    Cross,
}

struct Route {
    /// Tangent turn then first arm, one continuous stroke.
    first: Polyline,
    /// A second, separately lifted arm (the cross's).
    second: Polyline,
    /// Length of the temporary turn at the start of `first`.
    turn: f32,
}

impl Mark {
    pub fn is_check(&self) -> bool {
        *self == Self::Check
    }

    fn route(self) -> Route {
        let start = circle(HANDOFF_ANGLE);
        let (tip, arm, second) = match self {
            Self::Check => (
                vec2(13.0, 4.5),
                vec![vec2(6.5, 11.5), vec2(3.0, 8.0)],
                vec![],
            ),
            Self::Cross => (
                vec2(11.6, 4.4),
                vec![vec2(4.4, 11.6)],
                vec![vec2(4.4, 4.4), vec2(11.6, 11.6)],
            ),
        };
        // The turn leaves along the circle's clockwise tangent at the handoff.
        let turn = CubicBezier {
            start,
            control_a: start + vec2(0.6, 0.6),
            control_b: tip + vec2(0.39, -0.42),
            end: tip,
        }
        .flatten(24);
        Route {
            turn: turn.length(),
            first: Polyline::new(turn.points().iter().copied().chain(arm).collect()),
            second: Polyline::new(second),
        }
    }
}

/// One sampled spinner. Strokes are polylines with an ink weight per point.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpinnerPose {
    pub strokes: Vec<Vec<(Vec2, f32)>>,
    pub opacity: f32,
    /// Toward white, 0..1.
    pub flash: f32,
}

/// Sample at motor age `age` (negative: hidden). `release` is seconds since
/// the motor was released to coast; `mark` is seconds since the handoff to
/// `shape`. Negative values mean the event has not happened.
pub fn sample(age: f32, release: f32, mark: f32, shape: Mark) -> SpinnerPose {
    if age < 0.0 {
        return SpinnerPose::default();
    }
    let ring = |angle: f32, trail: f32, solid: f32| {
        (0..=36)
            .map(|index| {
                let t = index as f32 / 36.0;
                (circle(angle - trail * (1.0 - t)), t + (1.0 - t) * solid)
            })
            .collect::<Vec<_>>()
    };
    if mark >= 0.0 {
        let arrival = loading(age - mark);
        let route = shape.route();
        let first = route.first.length();
        let total = first + route.second.length();
        // Hermite spacing carries the motor's speed into the stroke and
        // arrives at rest; the slope cap keeps long strokes monotone.
        let speed = (arrival.velocity * PI / 180.0 * RADIUS * DRAW / total).min(2.8);
        let t = (mark / DRAW).clamp(0.0, 1.0);
        let progress = t * (speed + t * (3.0 - 2.0 * speed + t * (speed - 2.0)));
        let remaining = 1.0 - (progress / 0.45).clamp(0.0, 1.0);
        let trim = route.turn * ((progress - 0.45) / 0.25).clamp(0.0, 1.0);
        let distance = total * progress;
        let inked = |path: &Polyline, from: f32, to: f32| {
            let length = path.length().max(1e-6);
            path.slice(from / length, to / length)
                .points()
                .iter()
                .map(|point| (*point, 1.0))
                .collect::<Vec<_>>()
        };
        // The ring's wake drains into the tip; the slice starts at the ring's
        // own head, so the shared point is dropped from the arc.
        let mut head = if remaining > 0.0 {
            let mut arc = ring(
                HANDOFF_ANGLE,
                wake(arrival.velocity) * remaining,
                1.0 - remaining,
            );
            arc.pop();
            arc
        } else {
            Vec::new()
        };
        head.extend(inked(&route.first, trim, distance.min(first)));
        let mut strokes = vec![head];
        if distance > first {
            strokes.push(inked(&route.second, 0.0, distance - first));
        }
        let since = mark - DRAW;
        let flash = if since <= 0.0 {
            0.0
        } else {
            (1.0 - (-since / 0.04).exp()) * (1.0 - (since / COOL).clamp(0.0, 1.0)).powi(2)
        };
        return SpinnerPose {
            strokes,
            opacity: presence(arrival.velocity) + (1.0 - presence(arrival.velocity)) * progress,
            flash,
        };
    }
    let motor = if release >= 0.0 {
        released(age - release, release)
    } else {
        loading(age)
    };
    let opacity = presence(motor.velocity);
    if opacity <= 0.001 {
        return SpinnerPose::default();
    }
    SpinnerPose {
        strokes: vec![ring(motor.angle, wake(motor.velocity), 0.0)],
        opacity,
        flash: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_motor_builds_to_cruise_and_coasts_without_a_jump() {
        assert_eq!(loading(0.0).velocity, 0.0);
        assert!((loading(3.0).velocity - CRUISE).abs() < 1.0);
        let before = loading(1.2);
        let after = released(1.2, 0.0);
        assert!((before.angle - after.angle).abs() < 1e-3);
        assert!((before.velocity - after.velocity).abs() < 1e-3);
        assert!((before.acceleration - after.acceleration).abs() < 1e-2);
        assert!(released(1.2, 3.0).velocity.abs() < 1.0, "coasts to rest");
        assert!(released(1.2, 3.0).angle > before.angle, "never reverses");
    }

    #[test]
    fn the_handoff_is_at_a_top_right_crossing_after_the_request() {
        let at = handoff(1.6);
        assert!(at >= 1.6);
        let turns = (loading(at).angle - HANDOFF_ANGLE) / 360.0;
        assert!((turns - turns.round()).abs() < 1e-4);
    }

    #[test]
    fn the_mark_starts_where_the_ring_head_is_and_finishes_whole() {
        let at = handoff(1.6);
        let spinning = sample(at, -1.0, -1.0, Mark::Cross);
        let starting = sample(at, -1.0, 0.0, Mark::Cross);
        let head = spinning.strokes[0].last().unwrap().0;
        assert!(head.distance(starting.strokes[0].last().unwrap().0) < 1e-3);
        let done = sample(at + 1.0, -1.0, 1.0, Mark::Cross);
        assert_eq!(done.strokes.len(), 2, "both arms of the cross");
        assert!(
            done.strokes[0][0].0.distance(vec2(11.6, 4.4)) < 1e-3,
            "the temporary turn has drained to the cross's tip"
        );
        let before = sample(at + 0.001, -1.0, -1.0, Mark::Cross).opacity;
        assert!(
            (before - starting.opacity).abs() < 0.01,
            "no opacity jump at handoff"
        );
        assert!(done.opacity > 0.999);
        assert_eq!(sample(at + 3.0, -1.0, 3.0, Mark::Cross).flash, 0.0);
    }

    #[test]
    fn a_released_spinner_fades_out() {
        assert!(sample(4.0, 3.0, -1.0, Mark::Check).strokes.is_empty());
        assert!(sample(1.3, 0.2, -1.0, Mark::Check).opacity > 0.0);
        assert!(sample(-0.1, -1.0, -1.0, Mark::Check).strokes.is_empty());
    }
}
