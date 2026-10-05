//! Shapes that connectors attach to, the connector itself, and deterministic
//! points on 3D forms (sphere, box, grid, cylinder, torus) with a matching
//! that pairs the points of two forms for a morph.
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

/// Most vertices a [`Polygon`] keeps; a longer hull is simplified.
pub const POLYGON_VERTICES: usize = 32;

/// A convex outline around a center inside it, such as the silhouette of a
/// projected particle form or a rotated rectangle. Vertices wind with positive
/// signed area, so each edge's outward normal is `(e.y, -e.x)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Polygon {
    pub center: Vec2,
    count: usize,
    vertices: [Vec2; POLYGON_VERTICES],
}

impl Polygon {
    /// The convex hull of `points`, simplified to at most
    /// [`POLYGON_VERTICES`] by dropping the corners that matter least.
    /// Fewer than three distinct points make a degenerate outline that acts
    /// like its center.
    pub fn hull(center: Vec2, points: impl IntoIterator<Item = Vec2>) -> Self {
        let mut points = points.into_iter().collect::<Vec<_>>();
        points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
        points.dedup();
        // Andrew's monotone chain; collinear points are dropped.
        let mut hull: Vec<Vec2> = Vec::with_capacity(points.len() + 1);
        for pass in 0..2 {
            let start = hull.len();
            let ordered: Box<dyn Iterator<Item = &Vec2>> = if pass == 0 {
                Box::new(points.iter())
            } else {
                Box::new(points.iter().rev())
            };
            for &point in ordered {
                while hull.len() >= start + 2
                    && (hull[hull.len() - 1] - hull[hull.len() - 2])
                        .perp_dot(point - hull[hull.len() - 2])
                        <= 1e-4
                {
                    hull.pop();
                }
                hull.push(point);
            }
            hull.pop();
        }
        // Points along a straight side (projected grid rows, box edges) only
        // add rounding noise: drop any vertex within a hair of the chord
        // between its neighbors, so a box's silhouette is its corners.
        let mut index = 0;
        while hull.len() > 3 && index < hull.len() {
            let n = hull.len();
            let (prev, next) = (hull[(index + n - 1) % n], hull[(index + 1) % n]);
            let chord = next - prev;
            let off = chord.perp_dot(hull[index] - prev).abs() / chord.length().max(1e-6);
            if off < 0.35 {
                hull.remove(index);
                index = index.saturating_sub(1);
            } else {
                index += 1;
            }
        }
        // Visvalingam: drop the vertex spanning the smallest triangle.
        while hull.len() > POLYGON_VERTICES {
            let n = hull.len();
            let smallest = (0..n)
                .min_by(|&a, &b| {
                    let area = |i: usize| {
                        let (prev, next) = (hull[(i + n - 1) % n], hull[(i + 1) % n]);
                        (hull[i] - prev).perp_dot(next - prev).abs()
                    };
                    area(a).total_cmp(&area(b))
                })
                .unwrap_or(0);
            hull.remove(smallest);
        }
        let mut vertices = [Vec2::ZERO; POLYGON_VERTICES];
        let count = if hull.len() >= 3 { hull.len() } else { 0 };
        vertices[..count].copy_from_slice(&hull[..count]);
        Self {
            center,
            count,
            vertices,
        }
    }

    pub fn vertices(&self) -> &[Vec2] {
        &self.vertices[..self.count]
    }

    /// The same outline grown or shrunk about its center.
    pub fn scaled(&self, factor: f32) -> Self {
        let mut scaled = *self;
        for vertex in &mut scaled.vertices[..self.count] {
            *vertex = self.center + (*vertex - self.center) * factor;
        }
        scaled
    }

    pub fn bounds(&self) -> Box2 {
        self.vertices()
            .iter()
            .fold(Box2::from_center_size(self.center, Vec2::ZERO), |b, v| {
                Box2 {
                    min: b.min.min(*v),
                    max: b.max.max(*v),
                }
            })
    }

    fn edge(&self, index: usize) -> (Vec2, Vec2) {
        (
            self.vertices[index % self.count],
            self.vertices[(index + 1) % self.count],
        )
    }

    fn normal(&self, index: usize) -> Vec2 {
        let (a, b) = self.edge(index);
        let e = b - a;
        Vec2::new(e.y, -e.x).normalize_or(Vec2::X)
    }

    /// Signed distance from `point` to the outline: negative inside.
    pub fn distance(&self, point: Vec2) -> f32 {
        if self.count < 3 {
            return point.distance(self.center);
        }
        let mut inside = true;
        let mut nearest = f32::MAX;
        for index in 0..self.count {
            let (a, b) = self.edge(index);
            inside &= (point - a).dot(self.normal(index)) <= 0.0;
            nearest = nearest.min(segment_distance(point, a, b));
        }
        if inside { -nearest } else { nearest }
    }

    /// Where a ray from the center along `direction` leaves the outline: the
    /// edge index and the fraction along that edge.
    fn exit(&self, direction: Vec2) -> Option<(usize, f32)> {
        if self.count < 3 {
            return None;
        }
        // The ray leaves through the edge whose own segment it crosses; of
        // those (two only at a vertex, within rounding), the nearest.
        let mut best: Option<(f32, f32, usize, f32)> = None;
        for index in 0..self.count {
            let (a, b) = self.edge(index);
            let e = b - a;
            let denominator = direction.perp_dot(e);
            if denominator.abs() < 1e-9 {
                continue;
            }
            let offset = a - self.center;
            let t = offset.perp_dot(e) / denominator;
            let s = offset.perp_dot(direction) / denominator;
            // How far outside the segment the crossing falls, in pixels.
            let miss = (-s).max(s - 1.0).max(0.0) * e.length();
            let better = best.is_none_or(|(least, nearest, ..)| {
                miss < least - 1e-3 || (miss <= least + 1e-3 && t < nearest)
            });
            if t > 0.0 && better {
                best = Some((miss, t, index, s.clamp(0.0, 1.0)));
            }
        }
        best.map(|(_, _, index, s)| (index, s))
    }

    /// The outline's point in `direction` from the center.
    pub fn along(&self, direction: Vec2) -> Vec2 {
        match self.exit(direction.normalize_or(Vec2::X)) {
            Some((index, s)) => {
                let (a, b) = self.edge(index);
                a.lerp(b, s)
            }
            None => self.center,
        }
    }

    /// The port facing `target`: where the ray toward it leaves the outline,
    /// leaving along the edge's normal, rounded over the last few pixels
    /// before each corner, so a port slides continuously while the outline
    /// turns.
    pub fn port_toward(&self, target: Vec2) -> Port {
        let direction = (target - self.center).normalize_or(Vec2::X);
        let Some((index, s)) = self.exit(direction) else {
            return Port {
                point: self.center,
                normal: direction,
            };
        };
        let (a, b) = self.edge(index);
        let length = a.distance(b);
        let zone = (length * 0.5).clamp(1e-3, 10.0);
        let n = self.normal(index);
        let previous = self.normal(index + self.count - 1);
        let next = self.normal(index + 1);
        let from_a = s * length;
        let from_b = (1.0 - s) * length;
        let normal = if from_a < zone {
            ((previous + n) * 0.5)
                .lerp(n, super::smoothstep(from_a / zone))
                .normalize_or(n)
        } else if from_b < zone {
            ((next + n) * 0.5)
                .lerp(n, super::smoothstep(from_b / zone))
                .normalize_or(n)
        } else {
            n
        };
        Port {
            point: a.lerp(b, s),
            normal,
        }
    }
}

/// An outline that connectors attach to. A polygon keeps its few vertices
/// inline so outlines stay `Copy` values; the larger variant is deliberate.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Shape {
    Box(Box2),
    Circle(Circle),
    Point(Vec2),
    Polygon(Polygon),
}

impl Shape {
    pub fn center(&self) -> Vec2 {
        match self {
            Self::Box(bounds) => bounds.center(),
            Self::Circle(circle) => circle.center,
            Self::Point(point) => *point,
            Self::Polygon(polygon) => polygon.center,
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
            Self::Polygon(polygon) => polygon.distance(point),
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
            Self::Polygon(polygon) => polygon.port_toward(target),
        }
    }

    /// Where the ray from the center toward `target` leaves the outline, with
    /// the outward normal there: the straight-line contact for something that
    /// strikes the shape rather than plugging into a side's middle.
    pub fn boundary_toward(&self, target: Vec2) -> Port {
        let center = self.center();
        let direction = (target - center).normalize_or(Vec2::X);
        match self {
            Self::Box(bounds) => {
                let extents = bounds.extents();
                let reach = |e: f32, d: f32| {
                    if d.abs() > 1e-6 {
                        e / d.abs()
                    } else {
                        f32::MAX
                    }
                };
                let (tx, ty) = (reach(extents.x, direction.x), reach(extents.y, direction.y));
                Port {
                    point: center + direction * tx.min(ty),
                    normal: if tx <= ty {
                        Vec2::new(direction.x.signum(), 0.0)
                    } else {
                        Vec2::new(0.0, direction.y.signum())
                    },
                }
            }
            _ => self.port_toward(target),
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

/// The `index`th point of the R2 low-discrepancy sequence in the unit square:
/// evenly spread for any count, with no lattice to alias against.
pub fn r2(index: u32) -> Vec2 {
    // 1 / g and 1 / g² for the plastic number g.
    const A: [f64; 2] = [0.754_877_666_246_692_7, 0.569_840_290_998_053_3];
    let n = f64::from(index);
    Vec2::new(
        (0.5 + A[0] * n).fract() as f32,
        (0.5 + A[1] * n).fract() as f32,
    )
}

/// `total` split in proportion to `weights` by largest remainder: the parts
/// always sum to `total`.
fn apportion(total: usize, weights: &[f32]) -> Vec<usize> {
    let sum = weights.iter().sum::<f32>().max(f32::MIN_POSITIVE);
    let exact = weights
        .iter()
        .map(|w| w / sum * total as f32)
        .collect::<Vec<_>>();
    let mut parts = exact.iter().map(|e| e.floor() as usize).collect::<Vec<_>>();
    let mut order = (0..weights.len()).collect::<Vec<_>>();
    order.sort_by(|&a, &b| {
        (exact[b] - exact[b].floor())
            .total_cmp(&(exact[a] - exact[a].floor()))
            .then(a.cmp(&b))
    });
    let short = total.saturating_sub(parts.iter().sum());
    for &index in order.iter().cycle().take(short) {
        parts[index] += 1;
    }
    parts
}

/// `count` points on the surface of a box with half-size `half`: a share
/// `edges` (0..1) of them on its twelve edges, corners first and the rest
/// evenly spaced by length, and the remainder spread over its faces by area,
/// just inside the edges so those read as lines.
pub fn box_points(count: u32, half: Vec3, edges: f32) -> Vec<Vec3> {
    let count = count as usize;
    let on_edges = if edges <= 0.0 {
        0
    } else {
        ((count as f32 * edges.min(1.0)).round() as usize).clamp(8.min(count), count)
    };
    let corner = |x: f32, y: f32, z: f32| Vec3::new(x * half.x, y * half.y, z * half.z);
    let mut points = Vec::with_capacity(count);
    let signs = [-1.0, 1.0];
    for x in signs {
        for y in signs {
            for z in signs {
                if points.len() < on_edges {
                    points.push(corner(x, y, z));
                }
            }
        }
    }
    let mut segments = Vec::with_capacity(12);
    for a in signs {
        for b in signs {
            segments.push((corner(-1.0, a, b), corner(1.0, a, b)));
            segments.push((corner(a, -1.0, b), corner(a, 1.0, b)));
            segments.push((corner(a, b, -1.0), corner(a, b, 1.0)));
        }
    }
    let lengths = segments
        .iter()
        .map(|(a, b)| a.distance(*b))
        .collect::<Vec<_>>();
    for ((a, b), n) in segments
        .iter()
        .zip(apportion(on_edges - points.len(), &lengths))
    {
        points.extend((1..=n).map(|k| a.lerp(*b, k as f32 / (n + 1) as f32)));
    }
    // Faces: center, and the two half-axes spanning it.
    let faces = [
        (Vec3::X * half.x, Vec3::Y * half.y, Vec3::Z * half.z),
        (-Vec3::X * half.x, Vec3::Z * half.z, Vec3::Y * half.y),
        (Vec3::Y * half.y, Vec3::Z * half.z, Vec3::X * half.x),
        (-Vec3::Y * half.y, Vec3::X * half.x, Vec3::Z * half.z),
        (Vec3::Z * half.z, Vec3::X * half.x, Vec3::Y * half.y),
        (-Vec3::Z * half.z, Vec3::Y * half.y, Vec3::X * half.x),
    ];
    let areas = faces
        .iter()
        .map(|(_, u, v)| u.length() * v.length())
        .collect::<Vec<_>>();
    let inset = if on_edges > 0 { 0.88 } else { 1.0 };
    for (face, ((center, u, v), n)) in faces
        .iter()
        .zip(apportion(count - points.len(), &areas))
        .enumerate()
    {
        points.extend((0..n).map(|k| {
            let s = r2(k as u32 + face as u32 * 131) * 2.0 - Vec2::ONE;
            *center + *u * (s.x * inset) + *v * (s.y * inset)
        }));
    }
    points
}

/// The most even grid of exactly `count` cells over a box of `size`: one
/// count per axis, at least two each, whose cells are closest to cubes.
/// `None` when `count` has no such factorization (a prime, say).
pub fn grid_dims<const N: usize>(count: u32, size: [f32; N]) -> Option<[u32; N]> {
    fn search<const N: usize>(
        axis: usize,
        remaining: u32,
        dims: &mut [u32; N],
        size: &[f32; N],
        best: &mut Option<(f32, [u32; N])>,
    ) {
        if axis == N - 1 {
            if remaining < 2 {
                return;
            }
            dims[axis] = remaining;
            // Spacing per axis; a perfect grid has them all equal.
            let spacing = (0..N)
                .map(|a| (size[a].max(1e-3) / (dims[a] - 1) as f32).ln())
                .collect::<Vec<_>>();
            let mean = spacing.iter().sum::<f32>() / N as f32;
            let error = spacing.iter().map(|s| (s - mean).powi(2)).sum::<f32>();
            if best.is_none_or(|(e, _)| error < e - 1e-6) {
                *best = Some((error, *dims));
            }
            return;
        }
        for d in 2..=remaining / 2 {
            if remaining.is_multiple_of(d) {
                dims[axis] = d;
                search(axis + 1, remaining / d, dims, size, best);
            }
        }
    }
    let mut best = None;
    search(0, count, &mut [0; N], &size, &mut best);
    best.map(|(_, dims)| dims)
}

/// A grid of exactly `count` points filling a box of `size` (any axis may be
/// zero for a flat plane), row-major from the top left. `None` when `count`
/// does not factor into a grid with at least two points per axis.
pub fn grid_points<const N: usize>(count: u32, size: [f32; N]) -> Option<Vec<Vec3>> {
    let dims = grid_dims(count, size)?;
    let mut points = Vec::with_capacity(count as usize);
    for index in 0..count {
        let mut rest = index;
        let mut point = Vec3::ZERO;
        for axis in 0..N {
            let cell = rest % dims[axis];
            rest /= dims[axis];
            point[axis] = size[axis] * (cell as f32 / (dims[axis] - 1) as f32 - 0.5);
        }
        points.push(point);
    }
    Some(points)
}

/// `count` points on a capped-open cylinder about the y axis: a share on its
/// two rims, the rest on its side in a golden-angle spiral.
pub fn cylinder_points(count: u32, radius: f32, height: f32) -> Vec<Vec3> {
    let count = count as usize;
    let rim = (count / 6).min(count / 2);
    let side = count - 2 * rim;
    let mut points = Vec::with_capacity(count);
    for y in [-0.5, 0.5] {
        points.extend((0..rim).map(|k| {
            let angle = std::f32::consts::TAU * (k as f32 + 0.25 * y) / rim as f32;
            Vec3::new(angle.cos() * radius, y * height, angle.sin() * radius)
        }));
    }
    points.extend((0..side).map(|i| {
        let angle = i as f32 * 2.399_963_1;
        let y = height * (0.5 - (i as f32 + 0.5) / side as f32);
        Vec3::new(angle.cos() * radius, y, angle.sin() * radius)
    }));
    points
}

/// `count` points spread evenly by area over a torus lying in the x-z plane:
/// `radius` to the middle of its tube, `tube` the tube's radius.
pub fn torus_points(count: u32, radius: f32, tube: f32) -> Vec<Vec3> {
    let tube = tube.min(radius * 0.95);
    (0..count)
        .map(|i| {
            // Around the tube, by the inverse of the area's distribution
            // (R + r cos v) dv, so the outside is not sparser than the inside.
            let target = std::f32::consts::TAU * radius * (i as f32 + 0.5) / count as f32;
            let mut v = target / radius;
            for _ in 0..8 {
                v -= (radius * v + tube * v.sin() - target) / (radius + tube * v.cos());
            }
            let u = std::f32::consts::TAU * (i as f32 * 0.618_034).fract();
            let ring = radius + tube * v.cos();
            Vec3::new(ring * u.cos(), tube * v.sin(), ring * u.sin())
        })
        .collect()
}

/// `points` reordered so that point `i` lies near `reference[i]`: each
/// reference point, outermost first, claims its nearest unclaimed point, then
/// pairwise swaps shorten the total squared travel. Morphing point `i` from
/// the reference to the result then moves every point a short way.
pub fn match_points(reference: &[Vec3], mut points: Vec<Vec3>) -> Vec<Vec3> {
    let n = reference.len().min(points.len());
    points.truncate(n);
    let centroid = reference[..n].iter().sum::<Vec3>() / n.max(1) as f32;
    let mut order = (0..n).collect::<Vec<_>>();
    order.sort_by(|&a, &b| {
        reference[b]
            .distance_squared(centroid)
            .total_cmp(&reference[a].distance_squared(centroid))
            .then(a.cmp(&b))
    });
    let mut claimed = vec![false; n];
    let mut assigned = vec![0; n];
    for &i in &order {
        let mut nearest = (f32::MAX, 0);
        for (j, point) in points.iter().enumerate() {
            let distance = reference[i].distance_squared(*point);
            if !claimed[j] && distance < nearest.0 {
                nearest = (distance, j);
            }
        }
        claimed[nearest.1] = true;
        assigned[i] = nearest.1;
    }
    for _ in 0..6 {
        let mut improved = false;
        for i in 0..n {
            for j in i + 1..n {
                let (a, b) = (points[assigned[i]], points[assigned[j]]);
                let now = reference[i].distance_squared(a) + reference[j].distance_squared(b);
                let swapped = reference[i].distance_squared(b) + reference[j].distance_squared(a);
                if swapped < now - 1e-3 {
                    assigned.swap(i, j);
                    improved = true;
                }
            }
        }
        if !improved {
            break;
        }
    }
    assigned.into_iter().map(|j| points[j]).collect()
}

/// Distance from `point` to the segment from `a` to `b`: the field of a
/// stroked line, such as a chevron, whose coverage is `width / 2 - distance`.
pub fn segment_distance(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let along = b - a;
    let t = ((point - a).dot(along) / along.length_squared().max(1e-6)).clamp(0.0, 1.0);
    point.distance(a + along * t)
}

/// Signed distance from `point` to any simple polygon through `points` (in
/// order, not repeating the first): negative inside, by counting edge
/// crossings (Quilez's sdPolygon, as the Stage shader evaluates it).
pub fn polygon_distance(point: Vec2, points: &[Vec2]) -> f32 {
    let Some(&last) = points.last() else {
        return f32::MAX;
    };
    let mut nearest = f32::MAX;
    let mut sign = 1.0;
    let mut previous = last;
    for &vertex in points {
        nearest = nearest.min(segment_distance(point, vertex, previous));
        let edge = previous - vertex;
        let from = point - vertex;
        let crossing = [
            point.y >= vertex.y,
            point.y < previous.y,
            edge.x * from.y > edge.y * from.x,
        ];
        if crossing.iter().all(|c| *c) || crossing.iter().all(|c| !*c) {
            sign = -sign;
        }
        previous = vertex;
    }
    sign * nearest
}

/// A box with rounded corners, as a signed-distance field: a circle when it is
/// square with `corner` at half its side, a capsule when `corner` is half its
/// shorter side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundedBox {
    pub center: Vec2,
    /// Half the size.
    pub half: Vec2,
    pub corner: f32,
}

impl RoundedBox {
    /// `corner` held between square and fully round.
    pub fn new(center: Vec2, half: Vec2, corner: f32) -> Self {
        let half = half.max(Vec2::ZERO);
        Self {
            center,
            half,
            corner: corner.clamp(0.0, half.min_element()),
        }
    }

    /// Signed distance from `point` to the outline: negative inside.
    pub fn distance(&self, point: Vec2) -> f32 {
        let q = (point - self.center).abs() - self.half + self.corner;
        q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - self.corner
    }

    /// The outward unit normal of the nearest outline point: the distance
    /// field's gradient, radial around a rounded corner and straight out of a
    /// side. Inside, the deepest side wins, so it is constant along each side.
    pub fn normal(&self, point: Vec2) -> Vec2 {
        let offset = point - self.center;
        let q = offset.abs() - self.half + self.corner;
        let sign = Vec2::new(
            if offset.x < 0.0 { -1.0 } else { 1.0 },
            if offset.y < 0.0 { -1.0 } else { 1.0 },
        );
        let local = if q.x > 0.0 && q.y > 0.0 {
            q.normalize()
        } else if q.x > q.y {
            Vec2::X
        } else {
            Vec2::Y
        };
        local * sign
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::vec2;

    #[test]
    fn boundary_rays_leave_a_box_where_the_line_to_the_target_does() {
        let card = Shape::Box(Box2::from_center_size(vec2(0.0, 0.0), vec2(200.0, 100.0)));
        let side = card.boundary_toward(vec2(500.0, 100.0));
        assert_eq!((side.point, side.normal), (vec2(100.0, 20.0), Vec2::X));
        let top = card.boundary_toward(vec2(30.0, -400.0));
        assert!(top.point.abs_diff_eq(vec2(3.75, -50.0), 1e-4) && top.normal == Vec2::NEG_Y);
        let orb = Shape::Circle(Circle {
            center: Vec2::ZERO,
            radius: 50.0,
        });
        assert_eq!(
            orb.boundary_toward(vec2(0.0, 90.0)),
            orb.port_toward(vec2(0.0, 90.0))
        );
    }

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
    fn box_points_put_corners_and_edges_first_and_stay_on_the_surface() {
        let half = Vec3::new(120.0, 60.0, 90.0);
        let points = box_points(500, half, 0.5);
        assert_eq!(points.len(), 500);
        assert_eq!(points, box_points(500, half, 0.5), "deterministic");
        let on_surface = |p: &Vec3| {
            let q = p.abs() / half;
            (q.max_element() - 1.0).abs() < 1e-4 && q.cmple(Vec3::splat(1.0 + 1e-4)).all()
        };
        assert!(points.iter().all(on_surface));
        // The first eight are the corners; edge points touch two faces.
        assert!(points[..8].iter().all(|p| (p.abs() - half).length() < 1e-4));
        let on_edge = |p: &&Vec3| {
            let q = p.abs() / half;
            [q.x, q.y, q.z]
                .iter()
                .filter(|v| (**v - 1.0).abs() < 1e-4)
                .count()
                >= 2
        };
        assert_eq!(points.iter().filter(on_edge).count(), 250);
        assert_eq!(box_points(200, half, 0.0).iter().filter(on_edge).count(), 0);
        assert_eq!(box_points(5, half, 1.0).len(), 5);
    }

    #[test]
    fn grids_factor_the_count_exactly_and_prefer_square_cells() {
        assert_eq!(grid_dims(720, [480.0, 300.0]), Some([36, 20]));
        assert_eq!(grid_dims(1000, [200.0, 200.0, 200.0]), Some([10, 10, 10]));
        assert_eq!(grid_dims(13, [100.0, 100.0]), None, "a prime has no grid");
        let plane = grid_points(12, [300.0, 200.0]).unwrap();
        assert_eq!(plane.len(), 12);
        assert_eq!(
            plane[0],
            Vec3::new(-150.0, -100.0, 0.0),
            "from the top left"
        );
        assert_eq!(plane[1].y, plane[0].y, "rows first");
        assert_eq!(plane[11], Vec3::new(150.0, 100.0, 0.0));
        let lattice = grid_points(27, [90.0, 90.0, 90.0]).unwrap();
        assert!(lattice.iter().all(|p| p.abs().max_element() <= 45.0));
        assert!(lattice.contains(&Vec3::ZERO));
    }

    #[test]
    fn cylinders_and_tori_keep_their_counts_and_bounds() {
        let can = cylinder_points(300, 80.0, 200.0);
        assert_eq!(can.len(), 300);
        assert!(
            can.iter()
                .all(|p| { (p.x.hypot(p.z) - 80.0).abs() < 1e-3 && p.y.abs() <= 100.0 + 1e-3 })
        );
        let ring = torus_points(400, 150.0, 40.0);
        assert_eq!(ring.len(), 400);
        assert_eq!(ring, torus_points(400, 150.0, 40.0));
        for p in &ring {
            let along = p.x.hypot(p.z) - 150.0;
            assert!((along.hypot(p.y) - 40.0).abs() < 1e-2, "on the tube");
        }
        // Area-even: the outer half of the tube holds more points than the inner.
        let outer = ring.iter().filter(|p| p.x.hypot(p.z) > 150.0).count();
        assert!(outer > 200 && outer < 260, "{outer}");
    }

    #[test]
    fn matching_pairs_nearby_points_and_keeps_every_point() {
        let sphere = fibonacci_sphere(160)
            .into_iter()
            .map(|p| p * 100.0)
            .collect::<Vec<_>>();
        let cube = box_points(160, Vec3::splat(80.0), 0.5);
        let matched = match_points(&sphere, cube.clone());
        let sorted = |mut v: Vec<Vec3>| {
            v.sort_by(|a, b| {
                a.x.total_cmp(&b.x)
                    .then(a.y.total_cmp(&b.y))
                    .then(a.z.total_cmp(&b.z))
            });
            v
        };
        assert_eq!(
            sorted(matched.clone()),
            sorted(cube.clone()),
            "a permutation"
        );
        let travel = |pairs: &[Vec3]| {
            sphere
                .iter()
                .zip(pairs)
                .map(|(a, b)| a.distance(*b))
                .sum::<f32>()
                / 160.0
        };
        assert!(
            travel(&matched) < 0.5 * travel(&cube),
            "{} vs {}",
            travel(&matched),
            travel(&cube)
        );
        assert_eq!(
            match_points(&sphere, sphere.clone()),
            sphere,
            "identity stays"
        );
    }

    #[test]
    fn hulls_simplify_and_attach_ports_continuously() {
        let square = Polygon::hull(
            Vec2::ZERO,
            (0..=10).flat_map(|i| {
                let t = i as f32 * 20.0 - 100.0;
                [
                    vec2(t, -50.0),
                    vec2(t, 50.0),
                    vec2(-100.0, t * 0.5),
                    vec2(100.0, t * 0.5),
                ]
            }),
        );
        assert_eq!(square.vertices().len(), 4, "collinear points merge");
        assert_eq!(
            square.bounds(),
            Box2::from_center_size(Vec2::ZERO, vec2(200.0, 100.0))
        );
        let right = square.port_toward(vec2(900.0, 0.0));
        assert_eq!((right.point, right.normal), (vec2(100.0, 0.0), Vec2::X));
        assert_eq!(square.distance(vec2(150.0, 0.0)), 50.0);
        assert_eq!(square.distance(Vec2::ZERO), -50.0);
        assert_eq!(Shape::Polygon(square).center(), Vec2::ZERO);
        // Sweeping the target around never makes the port jump.
        let mut last = square.port_toward(vec2(500.0, 0.0));
        for step in 1..=720 {
            let angle = step as f32 / 720.0 * std::f32::consts::TAU;
            let port = square.port_toward(vec2(angle.cos(), angle.sin()) * 500.0);
            assert!(port.point.distance(last.point) < 3.0);
            assert!(port.normal.dot(last.normal) > 0.9);
            last = port;
        }
        let circle = Polygon::hull(
            vec2(10.0, 10.0),
            fibonacci_sphere(400)
                .into_iter()
                .map(|p| vec2(10.0, 10.0) + vec2(p.x, p.y) * 80.0),
        );
        assert_eq!(circle.vertices().len(), POLYGON_VERTICES);
        assert!((circle.along(Vec2::X).x - 90.0).abs() < 1.0);
        assert!(circle.scaled(0.5).along(Vec2::X).x < 51.0);
        let degenerate = Polygon::hull(Vec2::ZERO, [vec2(-5.0, 0.0), vec2(5.0, 0.0)]);
        assert_eq!(degenerate.port_toward(vec2(9.0, 0.0)).point, Vec2::ZERO);
    }

    #[test]
    fn sphere_points_are_unit_and_deterministic() {
        let points = fibonacci_sphere(64);
        assert_eq!(points.len(), 64);
        assert!(points.iter().all(|p| (p.length() - 1.0).abs() < 1e-4));
        assert_eq!(points[0].y, 1.0);
        assert_eq!(points, fibonacci_sphere(64));
    }

    #[test]
    fn polygon_distance_is_signed_for_concave_outlines() {
        // An L: a concave hexagon.
        let l = [
            vec2(0.0, 0.0),
            vec2(10.0, 0.0),
            vec2(10.0, 4.0),
            vec2(4.0, 4.0),
            vec2(4.0, 10.0),
            vec2(0.0, 10.0),
        ];
        assert!((polygon_distance(vec2(2.0, 2.0), &l) + 2.0).abs() < 1e-5);
        assert!(
            (polygon_distance(vec2(7.0, 7.0), &l) - 3.0).abs() < 1e-5,
            "outside the notch"
        );
        assert!((polygon_distance(vec2(-3.0, 5.0), &l) - 3.0).abs() < 1e-5);
        assert_eq!(polygon_distance(vec2(10.0, 2.0), &l), 0.0);
        assert_eq!(polygon_distance(Vec2::ZERO, &[]), f32::MAX);
    }

    #[test]
    fn rounded_boxes_span_circles_capsules_and_boxes() {
        let circle = RoundedBox::new(vec2(100.0, 100.0), vec2(50.0, 50.0), 80.0);
        assert_eq!(circle.corner, 50.0, "the corner never exceeds round");
        for angle in [0.0_f32, 0.7, 2.0, 4.0] {
            let at = vec2(100.0, 100.0) + Vec2::from_angle(angle) * 60.0;
            assert!((circle.distance(at) - 10.0).abs() < 1e-3);
            assert!(circle.normal(at).abs_diff_eq(Vec2::from_angle(angle), 1e-4));
        }
        let capsule = RoundedBox::new(Vec2::ZERO, vec2(200.0, 40.0), 40.0);
        assert_eq!(capsule.distance(vec2(0.0, -40.0)), 0.0);
        assert_eq!(capsule.distance(vec2(0.0, -30.0)), -10.0);
        assert_eq!(capsule.normal(vec2(30.0, -30.0)), -Vec2::Y);
        assert!(
            capsule
                .normal(vec2(-170.0, 5.0))
                .abs_diff_eq(vec2(-10.0, 5.0).normalize(), 1e-5),
            "radial around the cap"
        );
        assert!((capsule.distance(vec2(240.0, 0.0)) - 40.0).abs() < 1e-4);
        let card = RoundedBox::new(Vec2::ZERO, vec2(100.0, 40.0), 10.0);
        assert_eq!(card.normal(vec2(-95.0, 0.0)), -Vec2::X);
        let square = RoundedBox::new(Vec2::ZERO, vec2(10.0, 10.0), 0.0);
        assert_eq!(square.distance(vec2(13.0, 14.0)), 5.0);
    }
}
