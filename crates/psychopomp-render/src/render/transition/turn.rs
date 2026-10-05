//! Transitions in space: a matched zoom carries one element onto another,
//! a card flips over, and a cube turns to its next face. Frames are sampled
//! through their inverse transforms with mip levels, so a shrunken or oblique
//! frame averages its pixels instead of shimmering.
use psychopomp::{
    math::{Vec2, Vec3, smoothstep, vec2, vec3},
    plan::{
        ReelZoom, TransitionPhase, WipeDirection,
        transition::{Turn, cube_turn, flip_turn, match_element, match_mix},
    },
};

use super::{Frames, Mips, Paint, add, mix, paint_rows, put, scale};

/// How softly the matched element's edge blends, in screen pixels.
const ELEMENT_FEATHER: f32 = 28.0;
/// How far in from its border a shrunken frame starts to fade, against how
/// much it has shrunk.
const FRAME_FEATHER: f32 = 3.0;

pub(super) fn matched(
    frames: Frames,
    phase: TransitionPhase,
    to: [f32; 4],
    round: bool,
    paint: Paint,
) -> Vec<u8> {
    let Frames {
        outgoing,
        incoming,
        width,
        height,
    } = frames;
    let size = vec2(width as f32, height as f32);
    let from = phase.focus.unwrap_or([0.0, 0.0, size.x, size.y]);
    let progress = phase.progress;
    let out_zoom = ReelZoom::matched(from, to, progress, false);
    let in_zoom = ReelZoom::matched(from, to, progress, true);
    let element = match_element(from, to, progress);
    let (inside, around) = match_mix(progress);
    let levels = |zoom: ReelZoom| (1.0 / zoom.scale).max(1.0).log2().ceil() as usize + 1;
    let outgoing = Mips::new(outgoing, width, height, levels(out_zoom).min(6));
    let incoming = Mips::new(incoming, width, height, levels(in_zoom).min(6));
    let center = vec2(element[0] + element[2] * 0.5, element[1] + element[3] * 0.5);
    let half = vec2(element[2], element[3]) * 0.5;
    paint_rows(width, height, |y, row| {
        for (x, pixel) in row.iter_mut().enumerate() {
            let point = vec2(x as f32 + 0.5, y as f32 + 0.5);
            let (below, below_cover) = sample_zoomed(&outgoing, out_zoom, point, size);
            let rgb = mix(paint.background, below, below_cover);
            // The element turns into its counterpart first; the scene around
            // it follows, its shrunken border dissolving into the outgoing one.
            let edge = if round {
                ellipse(point - center, half)
            } else {
                rounded_box(point - center, half, 12.0)
            };
            let element = smoothstep(0.5 - edge / ELEMENT_FEATHER);
            let weight = (inside * element).max(around);
            if weight <= 0.0 {
                put(pixel, rgb);
                continue;
            }
            let (above, above_cover) = sample_zoomed(&incoming, in_zoom, point, size);
            let border = above_cover.max(element);
            put(pixel, mix(rgb, above, weight * border));
        }
    })
}

/// A frame placed by `zoom`, sampled at a screen point: its color, and how
/// much it shows there. A frame at rest reaches every pixel; a shrunken one
/// fades toward its border, so its vignette never draws a rectangle.
fn sample_zoomed(mips: &Mips, zoom: ReelZoom, point: Vec2, size: Vec2) -> ([f32; 3], f32) {
    let source = (point - Vec2::from(zoom.offset)) / zoom.scale;
    let band = ((1.0 - zoom.scale) * FRAME_FEATHER).clamp(0.0, 0.9);
    let cover = if band <= 0.0 {
        let inside = (source
            .x
            .min(size.x - source.x)
            .min(source.y)
            .min(size.y - source.y))
            * zoom.scale;
        (inside + 0.5).clamp(0.0, 1.0)
    } else {
        // Fade across the outer `band` of each half-axis.
        let centered = (source / size - Vec2::splat(0.5)).abs() * 2.0;
        let fade = |reach: f32| 1.0 - smoothstep((reach - (1.0 - band)) / band);
        fade(centered.x) * fade(centered.y)
    };
    (mips.sample(source.x, source.y, 1.0 / zoom.scale), cover)
}

/// The approximate signed distance from `point` to the ellipse of `half`
/// axes, in pixels: exact on a circle, and smooth enough for a feather.
fn ellipse(point: Vec2, half: Vec2) -> f32 {
    let scaled = point / half.max(Vec2::splat(1.0));
    (scaled.length() - 1.0) * half.min_element()
}

/// Signed distance from a rounded box centered at the origin; negative inside.
fn rounded_box(point: Vec2, half: Vec2, radius: f32) -> f32 {
    let radius = radius.min(half.x).min(half.y).max(0.0);
    let q = point.abs() - half + Vec2::splat(radius);
    q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - radius
}

/// The camera's focal length, against the larger side of the frame.
const FOCAL: f32 = 1.25;
/// How far a flipping card and a turning cube back away at most, against the
/// focal length.
const FLIP_DEPTH: f32 = 0.42;
const CUBE_DEPTH: f32 = 0.5;
/// The flipping card's corners round as it lifts off the frame.
const FLIP_RADIUS: f32 = 26.0;
/// Shading: the light comes from above and in front; the rest is ambient.
const AMBIENT: f32 = 0.42;
/// The brightest a flipping card's sheen gets, in linear light.
const GLEAM: f32 = 0.008;
/// How strongly a turning face's edges catch the light.
const RIM: f32 = 0.5;
/// How much darker than the frames the space behind them falls while they turn.
const VOID_DARKEN: f32 = 0.7;

pub(super) fn flip(
    frames: Frames,
    phase: TransitionPhase,
    direction: WipeDirection,
    paint: Paint,
) -> Vec<u8> {
    let space = Space::new(frames, direction);
    let turn = flip_turn(phase.progress);
    let angle = space.sign * turn.angle;
    let depth = turn.lift * FLIP_DEPTH * space.focal;
    let card = Face::turned(
        space.center(depth),
        angle,
        Vec2::ZERO,
        vec2(1.0, 0.0),
        space.half(),
    );
    let radius = FLIP_RADIUS * turn.lift;
    let outgoing = Mips::new(frames.outgoing, frames.width, frames.height, 4);
    let incoming = Mips::new(frames.incoming, frames.width, frames.height, 4);
    let sheen = turn.lift;
    let void = scale(paint.background, 1.0 - VOID_DARKEN * turn.lift);
    space.paint(|ray| {
        let Some(hit) = card.hit(ray, &space) else {
            return void;
        };
        // The card's back shows the incoming frame, the right way round.
        let mirror = |v: Vec2| vec2(-v.x, v.y);
        let (mips, hit) = if hit.front {
            (&outgoing, hit)
        } else {
            let steps = hit.steps.map(mirror);
            let local = mirror(hit.local);
            (
                &incoming,
                Hit {
                    local,
                    steps,
                    ..hit
                },
            )
        };
        let local = hit.local;
        let edge = rounded_box(local, space.half(), radius) / hit.footprint;
        let cover = (0.5 - edge).clamp(0.0, 1.0);
        let shade = space.shade(if hit.front { card.normal } else { -card.normal });
        let mut rgb = scale(space.sample(mips, &hit), shade);
        // A soft sheen crosses the card as it turns, and its rim catches light.
        let across = local.x / (space.half().x * 2.0) + 0.5;
        let sweep = 1.0 - turn.angle / std::f32::consts::PI;
        let gleam = (-((across - sweep) / 0.16).powi(2)).exp() * GLEAM * sheen;
        let rim = (-(edge / 1.2).powi(2)).exp() * RIM * sheen;
        rgb = add(rgb, scale(paint.ink, gleam));
        rgb = mix(rgb, mix(paint.raised, paint.ink, 0.25), rim);
        mix(void, rgb, cover)
    })
}

pub(super) fn cube(
    frames: Frames,
    phase: TransitionPhase,
    direction: WipeDirection,
    paint: Paint,
) -> Vec<u8> {
    let space = Space::new(frames, direction);
    let Turn { angle, lift } = cube_turn(phase.progress);
    let angle = space.sign * angle;
    let half = space.half();
    let depth = lift * CUBE_DEPTH * space.focal;
    // The cube's center sits half its depth behind the front face.
    let center = space.center(depth) + vec3(0.0, 0.0, half.x);
    let side = -space.sign;
    let front = Face::turned(center, angle, vec2(0.0, -half.x), vec2(1.0, 0.0), half);
    let next = Face::turned(
        center,
        angle,
        vec2(side * half.x, 0.0),
        vec2(0.0, side),
        half,
    );
    let outgoing = Mips::new(frames.outgoing, frames.width, frames.height, 4);
    let incoming = Mips::new(frames.incoming, frames.width, frames.height, 4);
    let void = scale(paint.background, 1.0 - VOID_DARKEN * lift);
    let rim_color = mix(paint.raised, paint.ink, 0.25);
    space.paint(|ray| {
        let hits = [
            front
                .hit(ray, &space)
                .filter(|hit| hit.front)
                .map(|hit| (hit, &front, &outgoing)),
            next.hit(ray, &space)
                .filter(|hit| hit.front)
                .map(|hit| (hit, &next, &incoming)),
        ];
        let mut ordered = hits.into_iter().flatten().collect::<Vec<_>>();
        ordered.sort_by(|a, b| a.0.distance.total_cmp(&b.0.distance));
        // Composite back to front so a shared edge blends both faces.
        let mut rgb = void;
        for (hit, face, mips) in ordered.into_iter().rev() {
            let edge = rounded_box(hit.local, half, 0.0) / hit.footprint;
            let cover = (0.5 - edge).clamp(0.0, 1.0);
            let shade = space.shade(face.normal);
            let mut color = scale(space.sample(mips, &hit), shade);
            // The cube's edges catch a little light, so its faces read as a solid.
            let rim = (-(edge / 1.2).powi(2)).exp() * RIM * lift;
            color = mix(color, rim_color, rim);
            rgb = mix(rgb, color, cover);
        }
        rgb
    })
}

/// Screen space turned so the motion runs along x: a vertical flip or cube is
/// a horizontal one with x and y exchanged. The camera looks down +z from
/// `focal` in front of the frame, so the frame at depth zero is pixel exact.
struct Space {
    horizontal: bool,
    /// Frame size along and across the motion.
    along: f32,
    across: f32,
    width: usize,
    height: usize,
    focal: f32,
    /// -1 toward left or up, 1 toward right or down.
    sign: f32,
}

impl Space {
    fn new(frames: Frames, direction: WipeDirection) -> Self {
        let horizontal = matches!(direction, WipeDirection::Left | WipeDirection::Right);
        let (width, height) = (frames.width as f32, frames.height as f32);
        let (along, across) = if horizontal {
            (width, height)
        } else {
            (height, width)
        };
        Self {
            horizontal,
            along,
            across,
            width: frames.width,
            height: frames.height,
            focal: FOCAL * width.max(height),
            sign: if matches!(direction, WipeDirection::Left | WipeDirection::Up) {
                -1.0
            } else {
                1.0
            },
        }
    }

    fn half(&self) -> Vec2 {
        vec2(self.along, self.across) * 0.5
    }

    /// The frame's center pushed `depth` away from the camera.
    fn center(&self, depth: f32) -> Vec3 {
        vec3(self.along * 0.5, self.across * 0.5, depth)
    }

    fn origin(&self) -> Vec3 {
        vec3(self.along * 0.5, self.across * 0.5, -self.focal)
    }

    /// The ray through a turned-space screen point.
    fn ray(&self, point: Vec2) -> Vec3 {
        vec3(
            point.x - self.along * 0.5,
            point.y - self.across * 0.5,
            self.focal,
        )
    }

    /// Brightness of a face with outward `normal`, one when square to the camera.
    fn shade(&self, normal: Vec3) -> f32 {
        let light = vec3(0.0, -0.45, -1.0).normalize();
        let lit = |normal: Vec3| AMBIENT + (1.0 - AMBIENT) * normal.dot(light).max(0.0);
        (lit(normal) / lit(vec3(0.0, 0.0, -1.0))).min(1.0)
    }

    /// A frame's color around a face-local point (centered, along/across),
    /// averaged over the pixel's footprint: taps along its long axis, each
    /// at the mip level of its short axis, so an oblique face stays sharp
    /// across and smooth along.
    fn sample(&self, mips: &Mips, hit: &Hit) -> [f32; 3] {
        let [a, b] = hit.steps;
        let (major, minor) = if a.length() >= b.length() {
            (a, b)
        } else {
            (b, a)
        };
        let taps = (major.length() / minor.length().max(1.0))
            .ceil()
            .clamp(1.0, 8.0) as usize;
        let mut sum = [0.0; 3];
        for tap in 0..taps {
            let offset = major * ((tap as f32 + 0.5) / taps as f32 - 0.5);
            let point = hit.local + offset + self.half();
            let (x, y) = if self.horizontal {
                (point.x, point.y)
            } else {
                (point.y, point.x)
            };
            sum = add(
                sum,
                mips.sample(x, y, minor.length().max(major.length() / 8.0)),
            );
        }
        scale(sum, 1.0 / taps as f32)
    }

    /// Paint every pixel from the turned-space ray through it.
    fn paint(&self, shade: impl Fn(Vec2) -> [f32; 3] + Sync) -> Vec<u8> {
        paint_rows(self.width, self.height, |y, row| {
            for (x, pixel) in row.iter_mut().enumerate() {
                let (along, across) = if self.horizontal { (x, y) } else { (y, x) };
                put(pixel, shade(vec2(along as f32 + 0.5, across as f32 + 0.5)));
            }
        })
    }
}

/// A flat rectangle in turned space, rotated about the across axis.
struct Face {
    center: Vec3,
    /// Unit vector along the face's own along axis.
    axis: Vec3,
    /// Outward normal; it faces the camera when square to it.
    normal: Vec3,
    half: Vec2,
}

struct Hit {
    /// Face-local point, centered.
    local: Vec2,
    /// Face pixels per screen pixel here, along the steeper screen axis.
    footprint: f32,
    /// How far one screen pixel right and one down move across the face.
    steps: [Vec2; 2],
    distance: f32,
    front: bool,
}

impl Face {
    /// A face whose center sits at `offset` (x, z) from `pivot` and whose
    /// along axis is `axis` (x, z) before the whole is turned by `angle`.
    fn turned(pivot: Vec3, angle: f32, offset: Vec2, axis: Vec2, half: Vec2) -> Self {
        let turn = |v: Vec2| {
            let (sin, cos) = angle.sin_cos();
            vec3(v.x * cos - v.y * sin, 0.0, v.x * sin + v.y * cos)
        };
        let axis = turn(axis);
        Self {
            center: pivot + turn(offset),
            axis,
            // Perpendicular to the axis and to y, toward the camera at rest.
            normal: vec3(axis.z, 0.0, -axis.x),
            half,
        }
    }

    fn local(&self, space: &Space, point: Vec2) -> Option<(Vec2, f32, bool)> {
        let origin = space.origin();
        let ray = space.ray(point);
        let facing = self.normal.dot(ray);
        if facing.abs() < 1e-6 {
            return None;
        }
        let distance = self.normal.dot(self.center - origin) / facing;
        if distance <= 0.0 {
            return None;
        }
        let offset = origin + ray * distance - self.center;
        Some((
            vec2(offset.dot(self.axis), offset.y),
            distance,
            facing < 0.0,
        ))
    }

    fn hit(&self, point: Vec2, space: &Space) -> Option<Hit> {
        let (local, distance, front) = self.local(space, point)?;
        if local.x.abs() > self.half.x + 2.0 || local.y.abs() > self.half.y + 2.0 {
            return None;
        }
        let step = |delta: Vec2| {
            self.local(space, point + delta)
                .map_or(Vec2::splat(64.0), |(next, ..)| next - local)
        };
        let steps = [step(vec2(1.0, 0.0)), step(vec2(0.0, 1.0))];
        let footprint = steps[0].length().max(steps[1].length()).min(64.0);
        Some(Hit {
            local,
            footprint: footprint.max(1e-3),
            steps,
            distance,
            front,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space(direction: WipeDirection) -> Space {
        Space::new(
            Frames {
                outgoing: &[],
                incoming: &[],
                width: 1920,
                height: 1080,
            },
            direction,
        )
    }

    #[test]
    fn a_face_at_rest_is_pixel_exact() {
        for direction in [WipeDirection::Left, WipeDirection::Down] {
            let space = space(direction);
            let face = Face::turned(
                space.center(0.0),
                0.0,
                Vec2::ZERO,
                vec2(1.0, 0.0),
                space.half(),
            );
            let hit = face.hit(vec2(100.5, 200.5), &space).unwrap();
            assert!(hit.front);
            let pixel = hit.local + space.half();
            assert!((pixel - vec2(100.5, 200.5)).length() < 1e-2, "{pixel}");
            assert!((hit.footprint - 1.0).abs() < 1e-3);
            assert_eq!(space.shade(face.normal), 1.0);
        }
    }

    #[test]
    fn a_turned_cube_brings_its_next_face_square_to_the_camera() {
        for direction in [WipeDirection::Left, WipeDirection::Right] {
            let space = space(direction);
            let half = space.half();
            let center = space.center(0.0) + vec3(0.0, 0.0, half.x);
            let side = -space.sign;
            let angle = space.sign * std::f32::consts::FRAC_PI_2;
            let next = Face::turned(
                center,
                angle,
                vec2(side * half.x, 0.0),
                vec2(0.0, side),
                half,
            );
            let hit = next.hit(vec2(300.5, 540.5), &space).unwrap();
            assert!(hit.front, "{direction:?}");
            assert!(
                (hit.local + half - vec2(300.5, 540.5)).length() < 0.05,
                "{direction:?}"
            );
        }
    }

    #[test]
    fn ellipses_measure_signed_distance_exactly_on_a_circle() {
        let half = vec2(20.0, 20.0);
        assert_eq!(ellipse(Vec2::ZERO, half), -20.0);
        assert_eq!(ellipse(vec2(0.0, 20.0), half), 0.0);
        assert!((ellipse(vec2(30.0, 0.0), half) - 10.0).abs() < 1e-5);
        assert!(
            ellipse(vec2(19.0, 19.0), half) > 0.0,
            "a circle cuts the box corner"
        );
    }

    #[test]
    fn rounded_boxes_measure_signed_distance() {
        let half = vec2(10.0, 5.0);
        assert_eq!(rounded_box(Vec2::ZERO, half, 0.0), -5.0);
        assert_eq!(rounded_box(vec2(12.0, 0.0), half, 0.0), 2.0);
        assert!(
            (rounded_box(vec2(10.0, 5.0), half, 2.0) - (2.0_f32.sqrt() * 2.0 - 2.0)).abs() < 1e-5
        );
    }
}
