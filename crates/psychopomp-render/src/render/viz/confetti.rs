//! Confetti pixels: an ignition core, tumbling paper rectangles whose
//! backs show darker as they flip, and four-point sparkles that twinkle and
//! burn out. Poses come from `effects::confetti` at the burst's age.
use psychopomp::{
    confetti::ConfettiPlan,
    effects::confetti::{PieceKind, ignition},
    tone::Tone,
};

use super::{
    super::{
        chart::{dot, stroke},
        theme::mix,
        ui::{
            Bounds,
            card::{Fill, UiCanvas, UiColor},
        },
    },
    rotated_rect,
};
use crate::render::HeadlessRenderer;

const WHITE: [u8; 3] = [255, 255, 255];

impl HeadlessRenderer {
    pub(crate) fn composite_confetti(
        &mut self,
        pixels: &mut [u8],
        plan: &ConfettiPlan,
        sample: impl Fn(&str, f32) -> f32,
    ) {
        let age = sample("burst", -1.0);
        let opacity = sample("opacity", 1.0).clamp(0.0, 1.0);
        if age < 0.0 || opacity <= 0.001 {
            return;
        }
        let canvas = self.viz_canvas();
        let palette = self.theme.palette();
        let origin = [
            plan.origin[0] + sample("x", 0.0),
            plan.origin[1] + sample("y", 0.0),
        ];
        let colors = plan
            .tones
            .iter()
            .map(|&tone| match tone {
                Tone::Plain => palette.text,
                tone => self.theme.tone(tone),
            })
            .collect::<Vec<_>>();
        if let Some((radius, strength)) = ignition(age) {
            UiCanvas::new(pixels, canvas).fill(
                Bounds::from_center(origin, [radius * 2.0, radius * 2.0]),
                radius,
                Fill::Radial {
                    center: [radius, radius],
                    radius,
                    inner: UiColor::srgb8(255, 255, 255, 255),
                    outer: UiColor::srgb8(255, 255, 255, 0),
                },
                opacity * strength * 0.9,
            );
        }
        for piece in plan.burst().sample(age, colors.len() as u32) {
            let alpha = opacity * piece.opacity;
            if alpha <= 0.001 {
                continue;
            }
            let color = colors[piece.color as usize];
            let at = [origin[0] + piece.position.x, origin[1] + piece.position.y];
            match piece.kind {
                PieceKind::Paper => {
                    let back = piece.flip < 0.0;
                    let shade = if back {
                        mix(color, palette.background, 0.38)
                    } else {
                        color
                    };
                    rotated_rect(
                        pixels,
                        canvas,
                        at,
                        [piece.size.x * piece.flip.abs().max(0.12), piece.size.y],
                        piece.rotation,
                        shade,
                        alpha,
                    );
                }
                PieceKind::Sparkle => {
                    let glow = alpha * piece.glow;
                    if glow <= 0.001 {
                        continue;
                    }
                    let hot = mix(color, WHITE, 0.65);
                    let arm = piece.size.x * (0.55 + 0.45 * piece.glow);
                    let (sin, cos) = piece.rotation.sin_cos();
                    for (dx, dy) in [(cos, sin), (-sin, cos)] {
                        stroke(
                            pixels,
                            canvas,
                            &[
                                [at[0] - dx * arm, at[1] - dy * arm],
                                [at[0] + dx * arm, at[1] + dy * arm],
                            ],
                            1.5,
                            hot,
                            glow,
                        );
                    }
                    dot(pixels, canvas, at, arm * 0.55, color, glow * 0.22);
                    dot(pixels, canvas, at, 1.8, WHITE, glow);
                }
            }
        }
    }
}

#[cfg(test)]
mod gpu_tests {
    use psychopomp::confetti::ConfettiPlan;

    use crate::render::{HeadlessRenderer, RenderSpec};

    #[test]
    #[ignore = "requires a headless GPU; the burst reaches pixels only while its clock runs"]
    fn confetti_bursts_on_its_clock() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "confetti-proof".into(),
        }))
        .unwrap();
        let plan = ConfettiPlan::new([960.0, 700.0]);
        let background = renderer.render_title_card("", None, 0.);
        let mut draw = |age: f32| {
            let mut pixels = background.clone();
            renderer.composite_confetti(&mut pixels, &plan, |property, default| {
                if property == "burst" { age } else { default }
            });
            pixels
        };
        assert!(draw(-1.0) == background);
        let mid = draw(0.6);
        assert!(mid != background);
        assert!(draw(10.0) == background, "every piece is gone");
        assert!(draw(0.6) == mid);
    }
}
