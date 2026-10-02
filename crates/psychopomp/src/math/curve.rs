//! Paths in the plane, measured by arc length so motion along them keeps a
//! steady speed however a curve bunches its parameter.
use super::{Vec2, inverse_lerp};

/// A cubic Bézier: leaves `start` toward `control_a` and arrives at `end`
/// from the direction of `control_b`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezier {
    pub start: Vec2,
    pub control_a: Vec2,
    pub control_b: Vec2,
    pub end: Vec2,
}

impl CubicBezier {
    pub fn point(&self, t: f32) -> Vec2 {
        let u = 1.0 - t;
        self.start * (u * u * u)
            + self.control_a * (3.0 * u * u * t)
            + self.control_b * (3.0 * u * t * t)
            + self.end * (t * t * t)
    }

    /// The curve as `segments` straight pieces through evenly spaced parameters.
    pub fn flatten(&self, segments: usize) -> Polyline {
        let segments = segments.max(1);
        Polyline::new(
            (0..=segments)
                .map(|i| self.point(i as f32 / segments as f32))
                .collect(),
        )
    }
}

/// Points joined by straight segments, with the running length at each point.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Polyline {
    points: Vec<Vec2>,
    lengths: Vec<f32>,
}

impl Polyline {
    pub fn new(points: Vec<Vec2>) -> Self {
        let mut total = 0.0;
        let lengths = points
            .iter()
            .enumerate()
            .map(|(index, point)| {
                if index > 0 {
                    total += point.distance(points[index - 1]);
                }
                total
            })
            .collect();
        Self { points, lengths }
    }

    pub fn points(&self) -> &[Vec2] {
        &self.points
    }

    /// Running length at each point, starting at zero.
    pub fn lengths(&self) -> &[f32] {
        &self.lengths
    }

    pub fn length(&self) -> f32 {
        self.lengths.last().copied().unwrap_or(0.0)
    }

    /// The point `distance` along the path, held at its ends.
    pub fn at_length(&self, distance: f32) -> Vec2 {
        match self.points.len() {
            0 => Vec2::ZERO,
            1 => self.points[0],
            count => {
                let distance = distance.clamp(0.0, self.length());
                let index = self
                    .lengths
                    .partition_point(|&length| length < distance)
                    .clamp(1, count - 1);
                let (before, after) = (self.lengths[index - 1], self.lengths[index]);
                self.points[index - 1]
                    .lerp(self.points[index], inverse_lerp(before, after, distance))
            }
        }
    }

    /// The point at `fraction` of the path's length.
    pub fn at(&self, fraction: f32) -> Vec2 {
        self.at_length(fraction * self.length())
    }

    /// The stretch between two fractions of the length, with exact end points.
    /// An empty stretch is a single point.
    pub fn slice(&self, from: f32, to: f32) -> Self {
        let length = self.length();
        let start = from.clamp(0.0, 1.0) * length;
        let end = to.clamp(0.0, 1.0) * length;
        if end <= start {
            return Self::new(vec![self.at_length(start)]);
        }
        let inner = self
            .points
            .iter()
            .zip(&self.lengths)
            .filter(|&(_, &at)| at > start && at < end)
            .map(|(point, _)| *point);
        Self::new(
            std::iter::once(self.at_length(start))
                .chain(inner)
                .chain(std::iter::once(self.at_length(end)))
                .collect(),
        )
    }

    pub fn reversed(&self) -> Self {
        Self::new(self.points.iter().rev().copied().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::vec2;

    fn corner() -> Polyline {
        Polyline::new(vec![vec2(0.0, 0.0), vec2(100.0, 0.0), vec2(100.0, 100.0)])
    }

    #[test]
    fn polylines_are_measured_and_sampled_by_arc_length() {
        let path = corner();
        assert_eq!(path.length(), 200.0);
        assert_eq!(path.lengths(), &[0.0, 100.0, 200.0]);
        assert_eq!(path.at(0.25), vec2(50.0, 0.0));
        assert_eq!(path.at(0.75), vec2(100.0, 50.0));
        assert_eq!(
            (path.at(-1.0), path.at(2.0)),
            (vec2(0.0, 0.0), vec2(100.0, 100.0))
        );
        assert_eq!(path.reversed().at(0.25), vec2(100.0, 50.0));
    }

    #[test]
    fn slices_keep_exact_ends_and_inner_corners() {
        let slice = corner().slice(0.25, 0.75);
        assert_eq!(
            slice.points(),
            &[vec2(50.0, 0.0), vec2(100.0, 0.0), vec2(100.0, 50.0)]
        );
        assert_eq!(slice.length(), 100.0);
        assert_eq!(corner().slice(0.6, 0.6).points().len(), 1);
    }

    #[test]
    fn arc_length_sampling_ignores_how_the_parameter_bunches() {
        // A straight curve whose handles crowd its start: the parameter midpoint
        // is near the start, but half the length is the true midpoint.
        let curve = CubicBezier {
            start: vec2(0.0, 0.0),
            control_a: vec2(1.0, 0.0),
            control_b: vec2(2.0, 0.0),
            end: vec2(100.0, 0.0),
        };
        assert_eq!(
            (curve.point(0.0), curve.point(1.0)),
            (curve.start, curve.end)
        );
        assert!(curve.point(0.5).x < 20.0);
        let path = curve.flatten(64);
        assert!((path.length() - 100.0).abs() < 1e-3);
        assert!((path.at(0.5).x - 50.0).abs() < 1e-3);
    }
}
