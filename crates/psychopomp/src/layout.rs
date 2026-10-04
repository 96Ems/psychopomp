//! Composable 2.5D spatial placement for Stage elements and overlays.
//!
//! Inspired by Haskell's `diagrams` envelopes: every positioned element has a
//! bounding [`Placement`] in Stage canvas coordinates (`[x, y, z]` center and
//! `[width, height]` extent). Relative placements (`below`, `above`, `left_of`,
//! `right_of`, `align_left`) and distributions (`row`, `column`, `spread_x`)
//! compute exact target coordinates at plan-construction time, without adding a
//! runtime layout solver to the renderer.

use crate::stage::{StageElement, StagePlan};

/// A 2.5D bounding envelope on the Stage canvas (`z = 0` is pixel-exact with the
/// default camera).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub center: [f32; 3],
    pub size: [f32; 2],
}

impl Placement {
    pub const fn new(center: [f32; 3], size: [f32; 2]) -> Self {
        Self { center, size }
    }

    /// Envelope of a rectangular card centered at `center` with `size`.
    pub const fn card(center: [f32; 3], size: [f32; 2]) -> Self {
        Self { center, size }
    }

    /// Envelope of a spherical orb or ring centered at `center` with `radius`.
    pub const fn orb(center: [f32; 3], radius: f32) -> Self {
        let diameter = radius * 2.0;
        Self {
            center,
            size: [diameter, diameter],
        }
    }

    /// Zero-size point envelope at `center`.
    pub const fn point(center: [f32; 3]) -> Self {
        Self {
            center,
            size: [0.0, 0.0],
        }
    }

    /// Derive the bounding placement of a [`StageElement`], if it is a
    /// positioned element (`Card`, `Orb`, `Label`, or `Ring`).
    pub fn from_element(element: &StageElement) -> Option<Self> {
        match *element {
            StageElement::Card { at, size, .. } => Some(Self::card(at, size)),
            StageElement::Orb { at, radius, .. } | StageElement::Ring { at, radius, .. } => {
                Some(Self::orb(at, radius))
            }
            StageElement::Label { at, size, .. } => Some(Self::new(at, [0.0, size])),
            StageElement::Beam { .. } | StageElement::Packet { .. } => None,
        }
    }

    /// Look up the bounding placement of `id` in `plan`.
    pub fn of(plan: &StagePlan, id: &str) -> Option<Self> {
        plan.element(id).and_then(Self::from_element)
    }

    pub const fn x(self) -> f32 {
        self.center[0]
    }

    pub const fn y(self) -> f32 {
        self.center[1]
    }

    pub const fn z(self) -> f32 {
        self.center[2]
    }

    pub const fn width(self) -> f32 {
        self.size[0]
    }

    pub const fn height(self) -> f32 {
        self.size[1]
    }

    pub fn left(self) -> f32 {
        self.center[0] - self.size[0] * 0.5
    }

    pub fn right(self) -> f32 {
        self.center[0] + self.size[0] * 0.5
    }

    pub fn top(self) -> f32 {
        self.center[1] - self.size[1] * 0.5
    }

    pub fn bottom(self) -> f32 {
        self.center[1] + self.size[1] * 0.5
    }

    /// Translate by `[dx, dy, dz]`, preserving size.
    pub fn translate(self, delta: [f32; 3]) -> Self {
        Self {
            center: [
                self.center[0] + delta[0],
                self.center[1] + delta[1],
                self.center[2] + delta[2],
            ],
            size: self.size,
        }
    }

    /// Same x/y and size, placed on depth plane `z`.
    pub const fn at_z(self, z: f32) -> Self {
        Self {
            center: [self.center[0], self.center[1], z],
            size: self.size,
        }
    }

    /// Point centered horizontally below this envelope's bottom edge by `gap`
    /// pixels (for labels under cards or orbs).
    pub fn below(self, gap: f32) -> [f32; 3] {
        [self.center[0], self.bottom() + gap, self.center[2]]
    }

    /// Point centered horizontally above this envelope's top edge by `gap`
    /// pixels.
    pub fn above(self, gap: f32) -> [f32; 3] {
        [self.center[0], self.top() - gap, self.center[2]]
    }

    /// Place a sibling envelope of `size` below this envelope with `gap`
    /// between their facing edges.
    pub fn stack_below(self, gap: f32, size: [f32; 2]) -> Self {
        Self::new(
            [
                self.center[0],
                self.bottom() + gap + size[1] * 0.5,
                self.center[2],
            ],
            size,
        )
    }

    /// Place a sibling envelope of `size` above this envelope with `gap`
    /// between their facing edges.
    pub fn stack_above(self, gap: f32, size: [f32; 2]) -> Self {
        Self::new(
            [
                self.center[0],
                self.top() - gap - size[1] * 0.5,
                self.center[2],
            ],
            size,
        )
    }

    /// Place a sibling envelope of `size` to the right of this envelope with
    /// `gap` between their facing edges.
    pub fn beside_right(self, gap: f32, size: [f32; 2]) -> Self {
        Self::new(
            [
                self.right() + gap + size[0] * 0.5,
                self.center[1],
                self.center[2],
            ],
            size,
        )
    }

    /// Place a sibling envelope of `size` to the left of this envelope with
    /// `gap` between their facing edges.
    pub fn beside_left(self, gap: f32, size: [f32; 2]) -> Self {
        Self::new(
            [
                self.left() - gap - size[0] * 0.5,
                self.center[1],
                self.center[2],
            ],
            size,
        )
    }

    /// Left-aligned anchor `inset` pixels inside the left edge, at `y` offset
    /// from the center (such as a chat line above an agent card).
    pub fn align_left(self, inset: f32, y_offset: f32) -> [f32; 3] {
        [
            self.left() + inset,
            self.center[1] + y_offset,
            self.center[2],
        ]
    }
}

/// `count` centers distributed horizontally around `center` at `pitch` spacing.
pub fn row(center: [f32; 3], pitch: f32, count: usize) -> Vec<[f32; 3]> {
    if count == 0 {
        return Vec::new();
    }
    let span = pitch * (count - 1) as f32;
    let start_x = center[0] - span * 0.5;
    (0..count)
        .map(|i| [start_x + i as f32 * pitch, center[1], center[2]])
        .collect()
}

/// `count` centers distributed vertically around `center` at `pitch` spacing.
pub fn column(center: [f32; 3], pitch: f32, count: usize) -> Vec<[f32; 3]> {
    if count == 0 {
        return Vec::new();
    }
    let span = pitch * (count - 1) as f32;
    let start_y = center[1] - span * 0.5;
    (0..count)
        .map(|i| [center[0], start_y + i as f32 * pitch, center[2]])
        .collect()
}

/// `count` points spread evenly from `left_x` to `right_x` (both included when
/// `count >= 2`; `left_x` when `count == 1`).
pub fn spread_x(left_x: f32, right_x: f32, y: f32, z: f32, count: usize) -> Vec<[f32; 3]> {
    match count {
        0 => Vec::new(),
        1 => vec![[left_x, y, z]],
        n => {
            let step = (right_x - left_x) / (n - 1) as f32;
            (0..n).map(|i| [left_x + i as f32 * step, y, z]).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opposite_relative_placements_round_trip() {
        let a = Placement::card([560.0, 540.0, 0.0], [300.0, 120.0]);
        let b = a.beside_right(80.0, [240.0, 100.0]);
        let a_back = b.beside_left(80.0, a.size);
        assert_eq!(a_back, a);

        let below = a.stack_below(40.0, [200.0, 80.0]);
        let a_up = below.stack_above(40.0, a.size);
        assert_eq!(a_up, a);
    }

    #[test]
    fn translation_commutes_with_envelope_queries() {
        let orb = Placement::orb([1650.0, 540.0, 0.0], 118.0);
        assert_eq!(orb.below(46.0), [1650.0, 704.0, 0.0]);
        let delta = [-120.0, 35.0, -40.0];
        let shifted = orb.translate(delta).below(46.0);
        let direct = orb.below(46.0);
        assert_eq!(
            shifted,
            [
                direct[0] + delta[0],
                direct[1] + delta[1],
                direct[2] + delta[2]
            ]
        );
    }

    #[test]
    fn row_and_column_preserve_centroid() {
        let center = [960.0, 540.0, -20.0];
        for count in 1..=6 {
            let pts = row(center, 340.0, count);
            let mean_x = pts.iter().map(|p| p[0]).sum::<f32>() / count as f32;
            assert!((mean_x - center[0]).abs() < 1e-4);

            let col = column(center, 180.0, count);
            let mean_y = col.iter().map(|p| p[1]).sum::<f32>() / count as f32;
            assert!((mean_y - center[1]).abs() < 1e-4);
        }
    }
}
