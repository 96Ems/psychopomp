//! Concrete finite-value tiles. Shared card coverage and fractional glyph
//! placement keep them consistent with the editor and simulated UI recipes.
use kinograph::value::ValueTokenPlan;

use super::{
    HeadlessRenderer,
    ui::{
        Bounds,
        card::{Fill, SurfaceStyle, UiCanvas, UiColor},
    },
};

impl HeadlessRenderer {
    pub(crate) fn composite_value_token(
        &mut self,
        pixels: &mut [u8],
        token: &ValueTokenPlan,
        center: [f32; 2],
        opacity: f32,
        emphasis: f32,
    ) {
        let opacity = opacity.clamp(0., 1.);
        let emphasis = emphasis.clamp(0., 1.);
        let accent = self.theme.ink(token.accent);
        let color = std::array::from_fn::<_, 3, _>(|i| {
            (f32::from([66_u8, 75, 91][i]) * (1. - emphasis) + f32::from(accent[i]) * emphasis)
                .round() as u8
        });
        let bounds = Bounds::from_center(center, token.size);
        if opacity > 0. {
            let mut canvas = UiCanvas::new(pixels, [self.spec.width, self.spec.height]);
            canvas.surface(
                bounds,
                SurfaceStyle::new(
                    Fill::Solid({
                        let [r, g, b] = self.theme.surface([13, 18, 27]);
                        UiColor::srgb8(r, g, b, 255)
                    }),
                    12.,
                )
                .border(1.7, UiColor::srgb8(color[0], color[1], color[2], 255), 1.),
                opacity,
            );
        }
        // Warm hidden immutable glyphs too, before native presentation begins.
        // Emphasis changes only the perimeter, never fades stable value ink.
        let y = center[1] - if token.detail.is_empty() { 0. } else { 13. };
        self.composite_centered_text(
            pixels,
            &token.label,
            [center[0], y],
            token.font_size,
            [234, 239, 247],
            opacity,
        );
        if !token.detail.is_empty() {
            self.composite_centered_text(
                pixels,
                &token.detail,
                [center[0], center[1] + 28.],
                18.,
                [146, 162, 184],
                opacity,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a headless GPU; warm hidden ink, fractional tile motion, and transparent endpoints"]
    fn value_tokens_keep_fractional_coverage_and_no_hidden_residue() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(crate::render::RenderSpec {
            width: 1920,
            height: 1080,
            font_path: std::path::PathBuf::from(crate::scenes::FONT_PATH),
            file_name: "value-token-proof".into(),
        }))
        .unwrap();
        let token = ValueTokenPlan {
            label: "true".into(),
            detail: "one Boolean value".into(),
            center: [960., 540.],
            size: [300., 120.],
            font_size: 36.,
            accent: [250, 145, 80],
        };
        let background = renderer.render_title_card("", None, 0.);
        let draw = |renderer: &mut HeadlessRenderer, x, opacity| {
            let mut pixels = background.clone();
            renderer.composite_value_token(&mut pixels, &token, [x, 540.], opacity, 1.);
            pixels
        };
        let before_warming = renderer.plain_text_sprites.len();
        assert!(draw(&mut renderer, 960., 0.) == background);
        let warmed = renderer.plain_text_sprites.len();
        assert!(
            warmed > before_warming,
            "hidden token text must populate the plain-text cache"
        );
        let held = draw(&mut renderer, 960., 1.);
        assert_eq!(
            renderer.plain_text_sprites.len(),
            warmed,
            "hidden glyphs must warm before their first visible step"
        );
        let a = draw(&mut renderer, 960.15, 1.);
        let b = draw(&mut renderer, 960.35, 1.);
        assert!(
            a != b && held != a,
            "fractional motion must reach pixels before whole-pixel boundaries"
        );
        for opacity in [0.01, 0.5, 0., 1., 0.25] {
            let pixels = draw(&mut renderer, 960.15, opacity);
            draw(&mut renderer, 1020., 1.);
            assert!(
                pixels == draw(&mut renderer, 960.15, opacity),
                "arbitrary sampling cannot retain prior ink"
            );
        }
        assert!(draw(&mut renderer, 960., 0.) == background);
    }
}
