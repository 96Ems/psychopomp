//! Shapes that connectors attach to, the connector itself, and points on a
//! sphere.
use super::{Vec2, Vec3, curve::CubicBezier};

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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle {
    pub center: Vec2,
    pub radius: f32,
}

/// Where a connector meets a shape, and the outward direction it leaves along.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Port {
    pub point: Vec2,
    pub normal: Vec2,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::vec2;

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
