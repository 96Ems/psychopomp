//! Chart ink shared by Plots and Lanes: CommitMono labels (monospaced, so
//! figures are tabular), axis rulers with ticks disclosed as the line reaches
//! them, and antialiased strokes, dashes, dots, and keyframe diamonds. Static
//! layout snaps text to whole pixels; motion offsets stay fractional.
use psychopomp::{
    axis::AxisPlan,
    math::{curve::Polyline, lerp, remap_clamp, smoothstep, vec2},
};

use super::{
    HeadlessRenderer, PlainTextSpec, TextDraw, blend_pixel, composite_text,
    ui::{
        Bounds,
        card::{Fill, UiCanvas, UiColor},
    },
};

pub(super) const TICK_SIZE: f32 = 16.0;
pub(super) const HAIRLINE: f32 = 1.25;
const TICK_LENGTH: f32 = 6.0;

#[derive(Clone, Copy)]
pub(super) enum Anchor {
    Left,
    Center,
    Right,
}

/// Which side of an axis line its tick marks and labels hang on.
#[derive(Clone, Copy)]
pub(super) enum Side {
    Below,
    Above,
    Left,
}

/// An axis line from `line[0]` (range start) to `line[1]` (range end) in
/// static layout, moved by `shift`. Tick labels within reach of the canvas x
/// in `avoid` fade by its strength, so a playhead readout never sits on one.
pub(super) struct AxisDraw<'a> {
    pub axis: &'a AxisPlan,
    pub line: [[f32; 2]; 2],
    pub side: Side,
    pub reveal: f32,
    pub opacity: f32,
    pub shift: [f32; 2],
    pub avoid: Option<(f32, f32)>,
}

/// One line of text vertically centered on `at[1]`, anchored at `at[0]`.
pub(super) struct Label<'a> {
    pub text: &'a str,
    pub size: f32,
    pub color: [u8; 3],
    pub anchor: Anchor,
    pub at: [f32; 2],
    pub opacity: f32,
}

impl HeadlessRenderer {
    fn canvas(&self) -> [u32; 2] {
        [self.spec.width, self.spec.height]
    }

    pub(super) fn chart_advance(&mut self, text: &str, size: f32) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        self.plain_text_sprite(text, label_spec(size, [255; 3]))
            .advance
    }

    /// Draws `label` with its layout snapped to whole pixels, then moved by
    /// the fractional `shift`; returns its width.
    pub(super) fn chart_label(&mut self, pixels: &mut [u8], label: Label, shift: [f32; 2]) -> f32 {
        if label.text.is_empty() || label.opacity <= 0.001 {
            return 0.0;
        }
        let canvas = self.canvas();
        let spec = label_spec(label.size, label.color);
        let sprite = self.plain_text_sprite(label.text, spec);
        let advance = sprite.advance;
        let left = match label.anchor {
            Anchor::Left => label.at[0],
            Anchor::Center => label.at[0] - advance * 0.5,
            Anchor::Right => label.at[0] - advance,
        };
        let top = label.at[1] - spec.size[1] as f32 * 0.5;
        composite_text(
            pixels,
            canvas,
            TextDraw {
                clip_width: advance.ceil() + 1.0,
                opacity: label.opacity,
                ..TextDraw::new(sprite, [left.round() + shift[0], top.round() + shift[1]])
            },
        );
        advance
    }

    /// An axis line drawn on up to `reveal`, with tick marks and labels that
    /// fade in as the line reaches them and make way for a playhead readout.
    pub(super) fn chart_axis(&mut self, pixels: &mut [u8], draw: AxisDraw) {
        let AxisDraw {
            axis,
            line: [from, to],
            side,
            reveal,
            opacity,
            shift,
            avoid,
        } = draw;
        if reveal <= 0.0 || opacity <= 0.001 {
            return;
        }
        let palette = self.theme.palette();
        let at = |f: f32| {
            [
                lerp(from[0], to[0], f) + shift[0],
                lerp(from[1], to[1], f) + shift[1],
            ]
        };
        stroke(
            pixels,
            self.canvas(),
            &[at(0.0), at(reveal.min(1.0))],
            HAIRLINE,
            palette.muted,
            opacity * 0.7,
        );
        let normal = match side {
            Side::Below => [0.0, 1.0],
            Side::Above => [0.0, -1.0],
            Side::Left => [-1.0, 0.0],
        };
        for &tick in &axis.ticks {
            let f = axis.fraction(tick);
            let alpha = opacity * disclosure(f, reveal);
            if alpha <= 0.001 {
                continue;
            }
            let base = at(f);
            stroke(
                pixels,
                self.canvas(),
                &[
                    base,
                    [
                        base[0] + normal[0] * TICK_LENGTH,
                        base[1] + normal[1] * TICK_LENGTH,
                    ],
                ],
                HAIRLINE,
                palette.muted,
                alpha * 0.7,
            );
            let gap = TICK_LENGTH + 6.0;
            let layout = [lerp(from[0], to[0], f), lerp(from[1], to[1], f)];
            let (anchor, position) = match side {
                Side::Below => (Anchor::Center, [layout[0], layout[1] + gap + 9.0]),
                Side::Above => (Anchor::Center, [layout[0], layout[1] - gap - 9.0]),
                Side::Left => (Anchor::Right, [layout[0] - gap, layout[1]]),
            };
            let clear = avoid.map_or(1.0, |(x, strength)| {
                1.0 - strength * (1.0 - smoothstep(((base[0] - x).abs() - 34.0) / 20.0))
            });
            self.chart_label(
                pixels,
                Label {
                    text: &axis.tick_label(tick),
                    size: TICK_SIZE,
                    color: palette.muted,
                    anchor,
                    at: position,
                    opacity: alpha * clear,
                },
                shift,
            );
        }
    }

    /// A small rounded tab with knocked-out text, as for a playhead readout.
    pub(super) fn chart_tab(
        &mut self,
        pixels: &mut [u8],
        text: &str,
        center: [f32; 2],
        fill: [u8; 3],
        ink: [u8; 3],
        opacity: f32,
    ) {
        if opacity <= 0.001 {
            return;
        }
        let width = self.chart_advance(text, TICK_SIZE) + 14.0;
        UiCanvas::new(pixels, self.canvas()).fill(
            Bounds::from_center(center, [width, 24.0]),
            5.0,
            solid(fill),
            opacity,
        );
        self.chart_label(
            pixels,
            Label {
                text,
                size: TICK_SIZE,
                color: ink,
                anchor: Anchor::Left,
                at: [0.0, 0.0],
                opacity,
            },
            [center[0] - (width - 14.0) * 0.5, center[1]],
        );
    }
}

/// Ink for a mark at fraction `f` of a line drawn up to `reveal`: absent
/// until the line reaches it, fully present a short way after, and present
/// everywhere (the far end included) once the line is complete.
pub(super) fn disclosure(f: f32, reveal: f32) -> f32 {
    const LAG: f32 = 0.06;
    smoothstep(remap_clamp(
        reveal * (1.0 + LAG) - f,
        [0.0, LAG],
        [0.0, 1.0],
    ))
}

fn label_spec(size: f32, color: [u8; 3]) -> PlainTextSpec {
    PlainTextSpec {
        font_size: size,
        color,
        size: [1200, (size * 1.5).ceil() as u32],
        semibold: false,
        crop_to_advance: true,
    }
}

pub(super) fn solid([r, g, b]: [u8; 3]) -> Fill {
    Fill::Solid(UiColor::srgb8(r, g, b, 255))
}

pub(super) fn stroke(
    pixels: &mut [u8],
    canvas: [u32; 2],
    points: &[[f32; 2]],
    width: f32,
    [r, g, b]: [u8; 3],
    opacity: f32,
) {
    UiCanvas::new(pixels, canvas).polyline(points, width, UiColor::srgb8(r, g, b, 255), opacity);
}

/// Dashes measured along the whole path, so corners never restart them.
pub(super) fn dashed(
    pixels: &mut [u8],
    canvas: [u32; 2],
    path: &Polyline,
    width: f32,
    color: [u8; 3],
    opacity: f32,
) {
    const DASH: f32 = 7.0;
    const GAP: f32 = 6.0;
    let length = path.length();
    if length <= 0.0 {
        return;
    }
    let mut cursor = 0.0;
    while cursor < length {
        let dash = path.slice(cursor / length, ((cursor + DASH) / length).min(1.0));
        let points = dash.points().iter().map(|p| [p.x, p.y]).collect::<Vec<_>>();
        stroke(pixels, canvas, &points, width, color, opacity);
        cursor += DASH + GAP;
    }
}

/// A path through canvas points, for arc-length draw-on and dashes.
pub(super) fn path(points: impl IntoIterator<Item = [f32; 2]>) -> Polyline {
    Polyline::new(points.into_iter().map(|[x, y]| vec2(x, y)).collect())
}

pub(super) fn points(path: &Polyline) -> Vec<[f32; 2]> {
    path.points().iter().map(|p| [p.x, p.y]).collect()
}

pub(super) fn dot(
    pixels: &mut [u8],
    canvas: [u32; 2],
    center: [f32; 2],
    radius: f32,
    color: [u8; 3],
    opacity: f32,
) {
    UiCanvas::new(pixels, canvas).fill(
        Bounds::from_center(center, [radius * 2.0, radius * 2.0]),
        radius,
        solid(color),
        opacity,
    );
}

/// A keyframe diamond: `fill` inside, an optional `outline` ring of `width`.
#[allow(clippy::too_many_arguments)]
pub(super) fn diamond(
    pixels: &mut [u8],
    [canvas_width, canvas_height]: [u32; 2],
    center: [f32; 2],
    radius: f32,
    fill: [u8; 3],
    outline: Option<([u8; 3], f32)>,
    opacity: f32,
) {
    if opacity <= 0.001 {
        return;
    }
    let reach = radius + 1.5;
    let x0 = (center[0] - reach).floor().max(0.0) as u32;
    let x1 = (center[0] + reach).ceil().min(canvas_width as f32) as u32;
    let y0 = (center[1] - reach).floor().max(0.0) as u32;
    let y1 = (center[1] + reach).ceil().min(canvas_height as f32) as u32;
    for y in y0..y1 {
        for x in x0..x1 {
            let d = [
                (x as f32 + 0.5 - center[0]).abs(),
                (y as f32 + 0.5 - center[1]).abs(),
            ];
            // Signed distance to the diamond's edge.
            let distance = (d[0] + d[1] - radius) * std::f32::consts::FRAC_1_SQRT_2;
            let inside = (0.5 - distance).clamp(0.0, 1.0);
            if inside <= 0.0 {
                continue;
            }
            let i = (y as usize * canvas_width as usize + x as usize) * 4;
            let [r, g, b] = match outline {
                Some((ring, width)) => {
                    let ring_coverage = (distance + width + 0.5).clamp(0.0, 1.0);
                    super::theme::mix(fill, ring, ring_coverage)
                }
                None => fill,
            };
            blend_pixel(&mut pixels[i..i + 4], [r, g, b, 255], inside * opacity);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::disclosure;

    #[test]
    fn marks_disclose_as_the_line_reaches_them_and_all_show_when_complete() {
        assert_eq!(disclosure(0.5, 0.4), 0.0);
        assert!(disclosure(0.5, 0.5) > 0.0);
        assert_eq!(disclosure(0.0, 1.0), 1.0);
        assert_eq!(disclosure(1.0, 1.0), 1.0);
        assert_eq!(disclosure(1.0, 0.0), 0.0);
    }
}
