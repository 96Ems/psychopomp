use kinograph::math::easing::cubic_out;

use super::{Canvas, EffectTarget};

const PARTICLES: [(f32, f32, f32, [u8; 3]); 14] = [
    (-2.88, 104.0, 3.0, [110, 231, 183]),
    (-2.48, 82.0, 2.0, [125, 211, 252]),
    (-2.08, 118.0, 3.5, [196, 181, 253]),
    (-1.67, 91.0, 2.5, [250, 204, 21]),
    (-1.21, 109.0, 2.0, [244, 114, 182]),
    (-0.72, 78.0, 3.0, [110, 231, 183]),
    (-0.28, 124.0, 2.5, [125, 211, 252]),
    (0.24, 94.0, 3.5, [250, 204, 21]),
    (0.68, 115.0, 2.0, [196, 181, 253]),
    (1.12, 85.0, 3.0, [110, 231, 183]),
    (1.54, 121.0, 2.5, [244, 114, 182]),
    (1.96, 88.0, 3.5, [125, 211, 252]),
    (2.39, 112.0, 2.0, [250, 204, 21]),
    (2.79, 96.0, 3.0, [110, 231, 183]),
];

pub(super) fn composite(canvas: &mut Canvas<'_>, target: EffectTarget, phase: f32) {
    let phase = phase.clamp(0.0, 1.0);
    if !(0.0..1.0).contains(&phase) {
        return;
    }
    let travel = cubic_out(phase);
    let fade = (phase / 0.10).min(1.0) * (1.0 - phase).powf(1.5);

    for (index, (angle, distance, size, color)) in PARTICLES.into_iter().enumerate() {
        let stagger = index as f32 % 3.0 * 0.025;
        let local = ((phase - stagger) / (1.0 - stagger)).clamp(0.0, 1.0);
        let local_travel = cubic_out(local);
        let radius = 12.0 + distance * local_travel;
        let x = target.center[0] + angle.cos() * radius;
        let y = target.center[1] + angle.sin() * radius + 38.0 * local * local;
        let previous_radius = 12.0 + distance * (local_travel - 0.045).max(0.0);
        let previous_x = target.center[0] + angle.cos() * previous_radius;
        let previous_y = target.center[1] + angle.sin() * previous_radius + 38.0 * local * local;

        for step in 0..4 {
            let mix = step as f32 / 4.0;
            canvas.soft_disc(
                [
                    previous_x + (x - previous_x) * mix,
                    previous_y + (y - previous_y) * mix,
                ],
                size * (0.5 + mix * 0.5),
                color,
                fade * (0.12 + mix * 0.14),
            );
        }
        canvas.soft_disc([x, y], size * (0.85 + 0.15 * travel), color, fade);
    }
}
