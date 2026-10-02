//! Two bounded rounded sets, with hatch coverage derived from their sampled
//! intersection. A circle-to-square morph uses the same signed-distance shape.
use super::*;
use psychopomp::component_prototype::VennPlan;

pub(crate) fn validate(plan: &VennPlan) -> Result<()> {
    anyhow::ensure!(
        plan.center.iter().all(|x| x.is_finite())
            && plan.radius.is_finite()
            && (20.0..=400.).contains(&plan.radius),
        "Venn requires finite center and radius in [20,400]"
    );
    for label in [&plan.left, &plan.right] {
        anyhow::ensure!(
            !label.is_empty() && label.len() <= 120 && !label.contains(['\n', '\r']),
            "Venn labels must be nonempty single-line text up to 120 bytes"
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Shape {
    center: [f32; 2],
    radius: f32,
    roundness: f32,
}
impl Shape {
    fn distance(self, p: [f32; 2]) -> f32 {
        let corner = self.radius * self.roundness;
        let q = [
            (p[0] - self.center[0]).abs() - self.radius + corner,
            (p[1] - self.center[1]).abs() - self.radius + corner,
        ];
        q[0].max(q[1]).min(0.) + q[0].max(0.).hypot(q[1].max(0.)) - corner
    }
}

impl HeadlessRenderer {
    pub(crate) fn composite_venn(
        &mut self,
        pixels: &mut [u8],
        plan: &VennPlan,
        sample: impl Fn(&str, f32) -> f32,
    ) {
        let origin = [sample("x", plan.center[0]), sample("y", plan.center[1])];
        let shapes = [("left", -0.6), ("right", 0.6)].map(|(id, side)| Shape {
            center: [
                origin[0] + sample(&format!("{id}.x"), side * plan.radius),
                origin[1] + sample(&format!("{id}.y"), 0.),
            ],
            radius: sample(&format!("{id}.radius"), plan.radius).clamp(0., 500.),
            roundness: sample(&format!("{id}.roundness"), 1.).clamp(0., 1.),
        });
        let opacity = sample("opacity", 1.).clamp(0., 1.);
        let hatch = sample("hatch", 1.).clamp(0., 1.);
        let color = self.theme.palette().accent;
        let min = std::array::from_fn::<_, 2, _>(|i| {
            shapes
                .iter()
                .map(|s| s.center[i] - s.radius - 2.)
                .fold(f32::INFINITY, f32::min)
                .floor()
                .max(0.) as u32
        });
        let max = std::array::from_fn::<_, 2, _>(|i| {
            shapes
                .iter()
                .map(|s| s.center[i] + s.radius + 2.)
                .fold(f32::NEG_INFINITY, f32::max)
                .ceil()
                .clamp(0., [self.spec.width, self.spec.height][i] as f32) as u32
        });
        for y in min[1]..max[1] {
            for x in min[0]..max[0] {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let d = shapes.map(|s| {
                    if s.radius < 0.001 {
                        f32::INFINITY
                    } else {
                        s.distance(p)
                    }
                });
                let coverage = d.map(|d| (0.5 - d).clamp(0., 1.));
                let outline = d
                    .map(|d| (1.5 - d.abs()).clamp(0., 1.))
                    .into_iter()
                    .fold(0., f32::max);
                let intersection = coverage[0].min(coverage[1]);
                // Eight-pixel diagonal hatch spacing, 1.5px strokes as in visual-types.
                let phase = ((p[0] + p[1]) / std::f32::consts::SQRT_2).rem_euclid(8.);
                let line = (1.25 - phase.min(8. - phase)).clamp(0., 1.);
                let alpha = (0.2 * coverage[0].max(coverage[1]))
                    .max(0.6 * outline)
                    .max(0.4 * intersection * line * hatch)
                    * opacity;
                if alpha > 0. {
                    let i = ((y * self.spec.width + x) * 4) as usize;
                    blend_pixel(
                        &mut pixels[i..i + 4],
                        [color[0], color[1], color[2], 255],
                        alpha,
                    );
                }
            }
        }
        for (index, (shape, label)) in shapes.iter().zip([&plan.left, &plan.right]).enumerate() {
            let sign = if index == 0 { -1. } else { 1. };
            let edge = shape.center[0] + sign * shape.radius;
            // Keep callouts outside both sampled sets, even when this set is
            // nested inside the other. Endpoints still follow the named boundary.
            let envelope = shapes
                .iter()
                .map(|s| s.center[0] + sign * s.radius)
                .reduce(|a, b| if sign < 0. { a.min(b) } else { a.max(b) })
                .unwrap();
            let end = envelope + sign * 64.;
            self.composite_prototype_path(
                pixels,
                &[[edge, shape.center[1]], [end, shape.center[1]]],
                1.,
                color,
                opacity * 0.5,
            );
            self.composite_centered_text(
                pixels,
                label,
                [end + sign * 70., shape.center[1]],
                26.,
                self.theme.palette().text,
                opacity,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intersection_uses_sampled_geometry_including_square_corners() {
        let circle = Shape {
            center: [0., 0.],
            radius: 100.,
            roundness: 1.,
        };
        let square = Shape {
            roundness: 0.,
            ..circle
        };
        assert!(circle.distance([90., 90.]) > 0.);
        assert!(square.distance([90., 90.]) < 0.);
        assert_eq!(circle.distance([100., 0.]), 0.);
        let disjoint = Shape {
            center: [250., 0.],
            ..circle
        };
        assert!(disjoint.distance([0., 0.]) > 0.);
    }
}
