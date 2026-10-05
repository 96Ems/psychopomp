//! A success burst in closed form: seeded paper pieces launched in a cone
//! tumble under gravity and heavy drag (`math::dynamics::ballistic`),
//! fluttering side to side as they fall, while sparkles streak out, twinkle,
//! and burn out first. Every piece is a function of the burst's age and its
//! index, so any frame samples alone and a reversed age reassembles it.
use std::f32::consts::{PI, TAU};

use crate::math::{Vec2, dynamics::ballistic, random::hash, smoothstep, vec2, vec3};

/// Seconds until the last piece has faded.
pub const LIFETIME: f32 = 3.6;
/// Share of pieces that are sparkles rather than paper.
const SPARKLES: f32 = 0.24;

/// The launch: how many pieces, where they aim, and how hard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Burst {
    pub count: u32,
    pub seed: u32,
    /// Launch speed of the fastest pieces, px/s.
    pub speed: f32,
    /// Aim in degrees clockwise from straight up.
    pub angle: f32,
    /// Half-angle of the launch cone, in degrees.
    pub spread: f32,
    /// Downward acceleration, px/s².
    pub gravity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceKind {
    /// A tumbling paper rectangle.
    Paper,
    /// A four-point twinkle.
    Sparkle,
}

/// One sampled piece, relative to the burst's origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub kind: PieceKind,
    pub position: Vec2,
    /// In-plane rotation, radians.
    pub rotation: f32,
    /// The paper's tumble: its width scales by this, and a negative value
    /// shows the darker back.
    pub flip: f32,
    /// Full width and height (a sparkle's arm length in `x`).
    pub size: Vec2,
    /// Index into the burst's palette.
    pub color: u32,
    pub opacity: f32,
    /// A sparkle's twinkle brightness, 0 to 1.
    pub glow: f32,
}

impl Burst {
    /// Every live piece at `age` seconds after launch (none before it).
    pub fn sample(&self, age: f32, colors: u32) -> Vec<Piece> {
        if age < 0.0 {
            return Vec::new();
        }
        (0..self.count)
            .filter_map(|index| self.piece(index, age, colors.max(1)))
            .collect()
    }

    fn piece(&self, index: u32, age: f32, colors: u32) -> Option<Piece> {
        let salt = self.seed.wrapping_mul(0x9E37);
        let h = |k: u32| hash(index, salt.wrapping_add(k.wrapping_mul(7919)));
        let kind = if h(0) < SPARKLES {
            PieceKind::Sparkle
        } else {
            PieceKind::Paper
        };
        // A triangular spread clusters pieces toward the aim.
        let aim = (self.angle + self.spread * (h(1) + h(2) - 1.0)).to_radians();
        let direction = vec2(aim.sin(), -aim.cos());
        let (speed, drag, gravity, life) = match kind {
            PieceKind::Paper => (
                self.speed * (0.3 + 0.7 * h(3).powf(0.6)),
                1.3 + 1.2 * h(4),
                self.gravity,
                2.4 + 1.1 * h(5),
            ),
            PieceKind::Sparkle => (
                self.speed * (0.55 + 0.75 * h(3)),
                3.4 + 1.2 * h(4),
                self.gravity * 0.2,
                0.45 + 0.6 * h(5),
            ),
        };
        if age >= life {
            return None;
        }
        let travel = ballistic(
            vec3(direction.x, direction.y, 0.0) * speed,
            vec3(0.0, gravity, 0.0),
            drag,
            age,
        );
        let fade = 1.0 - smoothstep((age - (life - 0.5)) / 0.5);
        let pop = smoothstep(age / 0.05);
        Some(match kind {
            PieceKind::Paper => {
                // Flutter grows as the piece slows into its fall.
                let sway = (16.0 + 26.0 * h(6))
                    * (age * (2.6 + 2.4 * h(7)) + TAU * h(8)).sin()
                    * smoothstep(age / 0.8);
                let spin = (2.0 + 5.0 * h(9)) * if h(10) < 0.5 { -1.0 } else { 1.0 };
                Piece {
                    kind,
                    position: vec2(travel.x + sway, travel.y),
                    rotation: TAU * h(11) + spin * (0.35 * age + (1.0 - (-1.4 * age).exp())),
                    flip: (TAU * h(12) + (6.0 + 9.0 * h(13)) * age).cos(),
                    size: vec2(12.0 + 8.0 * h(14), 6.0 + 3.0 * h(15)) * pop,
                    color: ((h(16) * colors as f32) as u32).min(colors - 1),
                    opacity: fade,
                    glow: 0.0,
                }
            }
            PieceKind::Sparkle => {
                let burn = (1.0 - age / life).max(0.0).powf(1.4);
                let twinkle = 0.6 + 0.4 * (age * (24.0 + 14.0 * h(6)) + TAU * h(7)).sin();
                Piece {
                    kind,
                    position: vec2(travel.x, travel.y),
                    rotation: PI * 0.25 * h(11).round(),
                    flip: 1.0,
                    size: Vec2::splat((7.0 + 7.0 * h(14)) * pop),
                    color: ((h(16) * colors as f32) as u32).min(colors - 1),
                    opacity: fade,
                    glow: burn * twinkle,
                }
            }
        })
    }
}

/// The ignition at the origin: a bright core that swells and fades in a
/// quarter second, as radius and opacity, or `None` once it has gone.
pub fn ignition(age: f32) -> Option<(f32, f32)> {
    const SPAN: f32 = 0.2;
    if !(0.0..SPAN).contains(&age) {
        return None;
    }
    let t = age / SPAN;
    Some((6.0 + 18.0 * (1.0 - (1.0 - t).powi(3)), (1.0 - t).powi(2)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn burst() -> Burst {
        Burst {
            count: 160,
            seed: 7,
            speed: 1700.0,
            angle: 0.0,
            spread: 55.0,
            gravity: 1200.0,
        }
    }

    #[test]
    fn bursts_are_deterministic_and_start_at_the_origin() {
        let burst = burst();
        assert!(burst.sample(-0.1, 5).is_empty());
        let start = burst.sample(0.0, 5);
        assert_eq!(start.len(), 160);
        assert!(start.iter().all(|piece| piece.position.length() < 1e-3));
        assert_eq!(burst.sample(0.8, 5), burst.sample(0.8, 5));
        let other = Burst { seed: 8, ..burst };
        assert_ne!(
            burst.sample(0.8, 5),
            other.sample(0.8, 5),
            "the seed matters"
        );
        let sparkles = start
            .iter()
            .filter(|piece| piece.kind == PieceKind::Sparkle)
            .count();
        assert!((20..70).contains(&sparkles), "{sparkles} sparkles");
    }

    #[test]
    fn pieces_rise_then_fall_and_are_gone_by_the_lifetime() {
        let burst = burst();
        let height = |age: f32| {
            let pieces = burst.sample(age, 5);
            pieces
                .iter()
                .filter(|piece| piece.kind == PieceKind::Paper)
                .map(|piece| piece.position.y)
                .sum::<f32>()
                / pieces.len() as f32
        };
        assert!(height(0.3) < -50.0, "launched upward");
        assert!(height(2.2) > height(0.6), "then falling");
        assert!(burst.sample(LIFETIME, 5).is_empty());
        // Sparkles burn out long before paper does.
        assert!(
            burst
                .sample(1.2, 5)
                .iter()
                .all(|piece| piece.kind == PieceKind::Paper)
        );
        assert!(ignition(0.1).is_some() && ignition(0.5).is_none());
        // Colors index the palette.
        assert!(burst.sample(0.5, 3).iter().all(|piece| piece.color < 3));
    }
}
