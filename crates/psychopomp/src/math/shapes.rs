//! Shapes that connectors attach to, the connector itself, and points on a
//! sphere.
use std::f32::consts::{FRAC_PI_2, PI};

use super::{
    Vec2, Vec3,
    curve::{CubicBezier, Polyline},
    lerp,
};

/// An axis-aligned box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box2 {
    pub min: Vec2,
    pub max: Vec2,
}

impl Box2 {
    pub fn from_center_size(center: Vec2, size: Vec2) -> Self {
        Self {
            min: center - size * 0.5,
            max: center + size * 0.5,
        }
    }

    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    /// Half the size.
    pub fn extents(&self) -> Vec2 {
        (self.max - self.min) * 0.5
    }

    /// The frame as a closed path from `start`, a point on its edge, back to
    /// `start`, turning counter-clockwise on screen (up a right side first),
    /// with corners rounded at `corner`.
    pub fn perimeter_from(&self, start: Vec2, corner: f32) -> Polyline {
        let (min, max) = (self.min, self.max);
        let r = corner.clamp(0.0, self.extents().min_element());
        // Corner arcs in screen angles (y down), counter-clockwise from top right.
        let corners = [
            (Vec2::new(max.x - r, min.y + r), 0.0),
            (Vec2::new(min.x + r, min.y + r), -FRAC_PI_2),
            (Vec2::new(min.x + r, max.y - r), -PI),
            (Vec2::new(max.x - r, max.y - r), -PI - FRAC_PI_2),
        ];
        // Right, top, left, bottom: the side `start` is on picks the first corner.
        let first = [
            (start.x - max.x).abs(),
            (start.y - min.y).abs(),
            (start.x - min.x).abs(),
            (start.y - max.y).abs(),
        ]
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.total_cmp(b.1))
        .map_or(0, |(side, _)| side);
        let arcs = (0..4).flat_map(|k| {
            let (center, from) = corners[(first + k) % 4];
            (0..=6).map(move |i| {
                let angle = lerp(from, from - FRAC_PI_2, i as f32 / 6.0);
                center + Vec2::new(angle.cos(), angle.sin()) * r
            })
        });
        Polyline::new(
            std::iter::once(start)
                .chain(arcs)
                .chain(std::iter::once(start))
                .collect(),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle {
    pub center: Vec2,
    pub radius: f32,
}

impl Circle {
    /// First intersection while following an arc-length path, as a 0..1 fraction.
    /// A path starting inside is already in contact; a miss returns None.
    pub fn entry_fraction(self, path: &Polyline) -> Option<f32> {
        if path.points().first()?.distance(self.center) <= self.radius {
            return Some(0.0);
        }
        for (index, pair) in path.points().windows(2).enumerate() {
            let offset = pair[0] - self.center;
            let delta = pair[1] - pair[0];
            let a = delta.length_squared();
            if a <= f32::EPSILON {
                continue;
            }
            let b = offset.dot(delta);
            let c = offset.length_squared() - self.radius * self.radius;
            let discriminant = b * b - a * c;
            if discriminant < 0.0 {
                continue;
            }
            let t = (-b - discriminant.sqrt()) / a;
            if (0.0..=1.0).contains(&t) {
                return Some(
                    (path.lengths()[index] + delta.length() * t) / path.length().max(f32::EPSILON),
                );
            }
        }
        None
    }
}

/// A closed small circle on the unit sphere, `angle` radians from a unit `axis`.
pub fn sphere_ring(axis: Vec3, angle: f32, segments: usize) -> Vec<Vec3> {
    let u = axis.any_orthonormal_vector();
    let v = axis.cross(u);
    (0..=segments.max(3))
        .map(|i| {
            let phase = std::f32::consts::TAU * i as f32 / segments.max(3) as f32;
            axis * angle.cos() + (u * phase.cos() + v * phase.sin()) * angle.sin()
        })
        .collect()
}

/// Where a connector meets a shape, and the outward direction it leaves along.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Port {
    pub point: Vec2,
    pub normal: Vec2,
}

/// Move a measured box the least distance needed to keep it outside both
/// ports' tangent planes, with `gap` of clearance. Normals point into the
/// space between the connected bodies. Returns `None` when it cannot fit.
pub fn fit_between_ports(bounds: Box2, ports: [Port; 2], gap: f32) -> Option<Box2> {
    let center = bounds.center();
    let extents = bounds.extents();
    let [a, b] = ports;
    let required =
        |port: Port| gap + port.normal.abs().dot(extents) - (center - port.point).dot(port.normal);
    let [ra, rb] = [required(a), required(b)];
    let mut candidates = vec![Vec2::ZERO, a.normal * ra.max(0.0), b.normal * rb.max(0.0)];
    let det = a.normal.perp_dot(b.normal);
    if det.abs() > 1e-6 {
        candidates.push(
            Vec2::new(
                ra * b.normal.y - rb * a.normal.y,
                rb * a.normal.x - ra * b.normal.x,
            ) / det,
        );
    }
    let shift = candidates
        .into_iter()
        .filter(|shift| a.normal.dot(*shift) >= ra - 1e-4 && b.normal.dot(*shift) >= rb - 1e-4)
        .min_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))?;
    Some(Box2 {
        min: bounds.min + shift,
        max: bounds.max + shift,
    })
}

/// An outline that connectors attach to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Box(Box2),
    Circle(Circle),
    Point(Vec2),
}

impl Shape {
    pub fn center(&self) -> Vec2 {
        match self {
            Self::Box(bounds) => bounds.center(),
            Self::Circle(circle) => circle.center,
            Self::Point(point) => *point,
        }
    }

    /// Signed distance from `point` to the outline: negative inside.
    pub fn distance(&self, point: Vec2) -> f32 {
        match self {
            Self::Box(bounds) => {
                let q = (point - bounds.center()).abs() - bounds.extents();
                q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0)
            }
            Self::Circle(circle) => point.distance(circle.center) - circle.radius,
            Self::Point(center) => point.distance(*center),
        }
    }

    /// The port facing `target`: the middle of the box side whose outward normal
    /// points most toward it, the circle's surface in its direction, or the point.
    pub fn port_toward(&self, target: Vec2) -> Port {
        let center = self.center();
        let direction = (target - center).normalize_or(Vec2::X);
        match self {
            Self::Box(bounds) => {
                let normal = if direction.x.abs() >= direction.y.abs() {
                    Vec2::new(direction.x.signum(), 0.0)
                } else {
                    Vec2::new(0.0, direction.y.signum())
                };
                Port {
                    point: center + normal * bounds.extents(),
                    normal,
                }
            }
            Self::Circle(circle) => Port {
                point: center + direction * circle.radius,
                normal: direction,
            },
            Self::Point(point) => Port {
                point: *point,
                normal: direction,
            },
        }
    }
}

/// A connector between two outlines. Each end attaches to the side facing the
/// other and leaves perpendicular to it; the handles grow with the distance, so
/// near and far connections curve alike. `bend` bows the curve to the right of
/// travel (screen coordinates, y down).
pub fn connect(from: Shape, to: Shape, bend: f32) -> CubicBezier {
    let start = from.port_toward(to.center());
    let end = to.port_toward(start.point);
    // Aim the start at the end's actual port, not its center.
    let start = from.port_toward(end.point);
    let chord = end.point - start.point;
    let reach = (chord.length() * 0.4).clamp(16.0, 320.0);
    let side = chord.normalize_or_zero().perp() * bend;
    CubicBezier {
        start: start.point,
        control_a: start.point + start.normal * reach + side,
        control_b: end.point + end.normal * reach + side,
        end: end.point,
    }
}

/// `count` points spread evenly over the unit sphere by the golden angle,
/// from the top (y = 1) to the bottom.
pub fn fibonacci_sphere(count: u32) -> Vec<Vec3> {
    let n = count.max(2);
    (0..n)
        .map(|i| {
            let y = 1.0 - (i as f32 / (n - 1) as f32) * 2.0;
            let radius = (1.0 - y * y).max(0.0).sqrt();
            let theta = i as f32 * 2.399_963_1;
            Vec3::new(theta.cos() * radius, y, theta.sin() * radius)
        })
        .collect()
}

/// Distance from `point` to the segment from `a` to `b`: the field of a
/// stroked line, such as a chevron, whose coverage is `width / 2 - distance`.
pub fn segment_distance(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let along = b - a;
    let t = ((point - a).dot(along) / along.length_squared().max(1e-6)).clamp(0.0, 1.0);
    point.distance(a + along * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::vec2;

    #[test]
    fn measured_boxes_stop_before_either_port_without_changing_size() {
        let ports = [
            Port {
                point: vec2(100.0, 0.0),
                normal: Vec2::X,
            },
            Port {
                point: vec2(400.0, 0.0),
                normal: -Vec2::X,
            },
        ];
        for x in [80.0, 160.0, 250.0, 380.0, 420.0] {
            let bounds = Box2::from_center_size(vec2(x, -26.0), vec2(120.0, 24.0));
            let fitted = fit_between_ports(bounds, ports, 8.0).unwrap();
            assert!(fitted.min.x >= 108.0 && fitted.max.x <= 392.0);
            assert_eq!(fitted.extents(), bounds.extents());
            assert_eq!(fitted.center().y, -26.0);
        }
        let too_wide = Box2::from_center_size(Vec2::ZERO, vec2(300.0, 24.0));
        assert_eq!(fit_between_ports(too_wide, ports, 8.0), None);
    }

    #[test]
    fn port_clearance_supports_vertical_and_diagonal_connections() {
        for normal in [Vec2::Y, vec2(1.0, 1.0).normalize()] {
            let ports = [
                Port {
                    point: Vec2::ZERO,
                    normal,
                },
                Port {
                    point: normal * 200.0,
                    normal: -normal,
                },
            ];
            let bounds = Box2::from_center_size(normal * 190.0, vec2(80.0, 30.0));
            let fitted = fit_between_ports(bounds, ports, 8.0).unwrap();
            for port in ports {
                let clearance = (fitted.center() - port.point).dot(port.normal)
                    - fitted.extents().dot(port.normal.abs());
                assert!(clearance >= 8.0 - 1e-4);
            }
        }
    }

    #[test]
    fn nonparallel_port_planes_fit_the_box_at_their_intersection() {
        let ports = [
            Port {
                point: Vec2::ZERO,
                normal: Vec2::X,
            },
            Port {
                point: Vec2::ZERO,
                normal: Vec2::Y,
            },
        ];
        let bounds = Box2::from_center_size(vec2(-20.0, -30.0), vec2(40.0, 20.0));
        let fitted = fit_between_ports(bounds, ports, 8.0).unwrap();
        assert_eq!(fitted.min, Vec2::splat(8.0));
    }

    #[test]
    fn segment_distance_measures_to_the_nearest_point_or_end() {
        let (a, b) = (vec2(0.0, 0.0), vec2(10.0, 0.0));
        assert_eq!(segment_distance(vec2(5.0, 3.0), a, b), 3.0);
        assert_eq!(segment_distance(vec2(13.0, 4.0), a, b), 5.0);
        assert_eq!(segment_distance(vec2(-3.0, 0.0), a, a), 3.0);
    }

    fn card() -> Shape {
        Shape::Box(Box2::from_center_size(
            vec2(300.0, 400.0),
            vec2(200.0, 100.0),
        ))
    }

    fn orb() -> Shape {
        Shape::Circle(Circle {
            center: vec2(900.0, 460.0),
            radius: 120.0,
        })
    }

    #[test]
    fn circle_contact_uses_arc_length_and_finds_crossings_between_samples() {
        let circle = Circle {
            center: Vec2::ZERO,
            radius: 1.0,
        };
        let through = Polyline::new(vec![vec2(-2.0, 0.0), vec2(2.0, 0.0)]);
        assert_eq!(circle.entry_fraction(&through), Some(0.25));
        let miss = Polyline::new(vec![vec2(-2.0, 2.0), vec2(2.0, 2.0)]);
        assert_eq!(circle.entry_fraction(&miss), None);
        let uneven = Polyline::new(vec![vec2(-3.0, 0.0), vec2(-2.0, 0.0), Vec2::ZERO]);
        assert!((circle.entry_fraction(&uneven).unwrap() - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn sphere_ring_stays_on_the_surface_and_at_the_requested_angle() {
        let axis = Vec3::new(-1.0, 0.2, -0.34).normalize();
        let ring = sphere_ring(axis, 0.7, 72);
        for point in &ring {
            assert!((point.length() - 1.0).abs() < 1e-6);
            assert!((point.dot(axis) - 0.7_f32.cos()).abs() < 1e-6);
        }
        assert!(ring[0].distance(*ring.last().unwrap()) < 1e-6);
    }

    #[test]
    fn ports_face_the_target() {
        let Shape::Box(bounds) = card() else {
            unreachable!()
        };
        assert_eq!(bounds.extents(), vec2(100.0, 50.0));
        let right = card().port_toward(vec2(900.0, 420.0));
        assert_eq!((right.point, right.normal), (vec2(400.0, 400.0), Vec2::X));
        let top = card().port_toward(vec2(320.0, 0.0));
        assert_eq!((top.point, top.normal), (vec2(300.0, 350.0), Vec2::NEG_Y));
        let surface = orb().port_toward(vec2(900.0, 0.0));
        assert_eq!(surface.point, vec2(900.0, 340.0));
        assert!(surface.normal.abs_diff_eq(Vec2::NEG_Y, 1e-6));
    }

    #[test]
    fn perimeters_start_at_the_port_and_turn_up_first() {
        let Shape::Box(bounds) = card() else {
            unreachable!()
        };
        let port = vec2(400.0, 400.0);
        let frame = bounds.perimeter_from(port, 0.0);
        assert_eq!((frame.at(0.0), frame.at(1.0)), (port, port));
        assert!(
            (frame.length() - 600.0).abs() < 1e-3,
            "a sharp frame is its perimeter"
        );
        assert!(frame.at_length(10.0).y < port.y, "up the right side first");
        assert!(
            frame.at_length(100.0).x < 400.0 - 40.0,
            "then left along the top"
        );
        let rounded = bounds.perimeter_from(port, 14.0);
        let expected = 600.0 - (8.0 - 2.0 * std::f32::consts::PI) * 14.0;
        assert!(
            (rounded.length() - expected).abs() < 1.0,
            "{}",
            rounded.length()
        );
    }

    #[test]
    fn distances_are_signed_from_the_outline() {
        assert_eq!(card().distance(vec2(450.0, 400.0)), 50.0, "beside a side");
        assert_eq!(card().distance(vec2(403.0, 454.0)), 5.0, "past a corner");
        assert_eq!(card().distance(vec2(300.0, 390.0)), -40.0, "inside");
        assert_eq!(orb().distance(vec2(900.0, 300.0)), 40.0);
        assert_eq!(Shape::Point(Vec2::ZERO).distance(vec2(3.0, 4.0)), 5.0);
    }

    #[test]
    fn connectors_leave_perpendicular_and_never_cross_their_ends() {
        let curve = connect(card(), orb(), 0.0);
        assert_eq!(
            curve.start,
            vec2(400.0, 400.0),
            "the middle of the facing side"
        );
        assert!(
            (curve.end.distance(vec2(900.0, 460.0)) - 120.0).abs() < 1e-3,
            "on the orb's surface"
        );
        assert_eq!(
            (curve.control_a - curve.start).normalize().y,
            0.0,
            "leaves the side head-on"
        );
        let toward_center = (vec2(900.0, 460.0) - curve.end).normalize();
        let arriving = (curve.end - curve.control_b).normalize();
        assert!(
            arriving.dot(toward_center) > 0.999,
            "enters the orb radially"
        );
        let path = curve.flatten(48);
        assert!(path.points().iter().all(|p| p.x >= 400.0 - 1e-3));
        assert!(
            path.points()
                .iter()
                .all(|p| p.distance(vec2(900.0, 460.0)) >= 120.0 - 1e-3)
        );
    }

    #[test]
    fn bend_bows_to_the_right_of_travel() {
        let straight = connect(card(), orb(), 0.0).flatten(32).at(0.5);
        let bent = connect(card(), orb(), 60.0).flatten(32).at(0.5);
        assert!(
            bent.y > straight.y + 20.0,
            "moving right, right is down on screen"
        );
    }

    #[test]
    fn sphere_points_are_unit_and_deterministic() {
        let points = fibonacci_sphere(64);
        assert_eq!(points.len(), 64);
        assert!(points.iter().all(|p| (p.length() - 1.0).abs() < 1e-4));
        assert_eq!(points[0].y, 1.0);
        assert_eq!(points, fibonacci_sphere(64));
    }
}
