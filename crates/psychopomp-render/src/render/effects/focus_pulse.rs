use psychopomp::math::easing::cubic_out;

use super::{Canvas, EffectTarget};

pub(super) fn composite(canvas: &mut Canvas<'_>, target: EffectTarget, phase: f32, color: [u8; 3]) {
    let phase = phase.clamp(0.0, 1.0);
    if !(0.0..1.0).contains(&phase) {
        return;
    }
    let travel = cubic_out(phase);
    let fade = (phase / 0.08).min(1.0) * (1.0 - phase).powf(1.35);
    let ring_radius = 22.0 + 112.0 * travel;
    let extent = 155_i32;

    for offset_y in -extent..=extent {
        for offset_x in -extent..=extent {
            let distance = ((offset_x * offset_x + offset_y * offset_y) as f32).sqrt();
            let glow = (1.0 - distance / 112.0).max(0.0).powi(3) * fade * 0.16;
            let ring = (1.0 - (distance - ring_radius).abs() / 3.0).max(0.0) * fade * 0.72;
            let alpha = glow + ring;
            if alpha > 0.001 {
                canvas.blend(
                    target.center[0].round() as i32 + offset_x,
                    target.center[1].round() as i32 + offset_y,
                    [color[0], color[1], color[2], 255],
                    alpha,
                );
            }
        }
    }
}
