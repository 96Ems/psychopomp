//! Benchmark Bars pixels: gridlines and an axis drawn on with `axes`, a
//! legend, rows placed by their `slot` channels, bars with counting Readouts
//! at their ends, and delta chips in a fixed column.
use psychopomp::{
    bars::{BarsPlan, paint_order},
    math::{remap_clamp, smoothstep},
    tone::Tone,
};

use super::{
    super::{
        chart::{Anchor, AxisDraw, Side, TITLE_SIZE, disclosure, solid, stroke},
        theme::mix,
        ui::{
            Bounds,
            card::{Fill, SurfaceStyle, UiCanvas, UiColor},
        },
    },
    readout::Readout,
};
use crate::render::HeadlessRenderer;

/// The legend row sits this far above the first row's top.
const LEGEND_Y: f32 = -38.0;

impl HeadlessRenderer {
    pub(crate) fn composite_bars(
        &mut self,
        pixels: &mut [u8],
        plan: &BarsPlan,
        sample: impl Fn(&str, f32) -> f32,
        velocity: impl Fn(&str) -> f32,
    ) {
        let opacity = sample("opacity", 1.0).clamp(0.0, 1.0);
        if opacity <= 0.001 {
            return;
        }
        let shift = [sample("x", 0.0), sample("y", 0.0)];
        let canvas = self.viz_canvas();
        let palette = self.theme.palette();
        let axes = sample("axes", 1.0).clamp(0.0, 1.0);
        let [left, top] = [plan.origin[0] + shift[0], plan.origin[1] + shift[1]];
        let axis_y = plan.axis_y();
        let grid = Grid {
            left,
            top: top - 6.0,
            bottom: axis_y + shift[1],
            axes,
            opacity,
        };
        bars_grid(
            pixels,
            canvas,
            plan,
            grid,
            [f32::MIN, f32::MAX],
            palette.muted,
        );
        self.chart_axis(
            pixels,
            AxisDraw {
                axis: &plan.axis,
                line: [
                    [plan.origin[0], axis_y],
                    [plan.origin[0] + plan.width, axis_y],
                ],
                side: Side::Below,
                reveal: axes,
                opacity,
                shift,
                avoid: None,
            },
        );
        let legend = opacity * smoothstep(remap_clamp(axes, [0.3, 0.8], [0.0, 1.0]));
        let mut legend_x = left;
        for series in &plan.series {
            if series.label.is_empty() {
                continue;
            }
            let y = top + LEGEND_Y;
            UiCanvas::new(pixels, canvas).fill(
                Bounds::from_center([legend_x + 9.0, y], [18.0, 10.0]),
                3.0,
                solid(self.series_ink(series.tone)),
                legend,
            );
            let width = self.viz_text(
                pixels,
                &series.label,
                TITLE_SIZE,
                mix(palette.muted, palette.text, 0.7),
                Anchor::Left,
                [legend_x + 28.0, y],
                legend,
            );
            legend_x += 28.0 + width + 36.0;
        }
        self.viz_text(
            pixels,
            &plan.axis.label,
            TITLE_SIZE,
            palette.muted,
            Anchor::Right,
            [left + plan.width, top + LEGEND_Y],
            legend,
        );
        // Chips align in one column past the widest possible readout.
        let widest = self.readout_width(&bar_readout(plan, plan.axis.range[1], &[], palette.muted));
        let chip_x = left + plan.width * 1.04 + 14.0 + widest + 28.0;
        let thickness = plan.bar_thickness();
        let speeds = plan
            .rows
            .iter()
            .map(|row| velocity(&format!("row.{}.slot", row.id)))
            .collect::<Vec<_>>();
        for index in paint_order(&speeds) {
            let row = &plan.rows[index];
            let slot = sample(&format!("row.{}.slot", row.id), index as f32);
            let alpha = opacity * sample(&format!("row.{}.opacity", row.id), 1.0).clamp(0.0, 1.0);
            if alpha <= 0.001 {
                continue;
            }
            let y = plan.row_center(slot) + shift[1];
            // A moving row carries an opaque band, so crossing rows occlude
            // each other instead of interleaving their labels and bars.
            let band = alpha * smoothstep(speeds[index].abs() / 0.8);
            if band > 0.001 {
                let height = plan.row_height * 0.92;
                UiCanvas::new(pixels, canvas).fill(
                    Bounds {
                        origin: [
                            left - 40.0 - self.text_advance(&row.label, plan.size),
                            y - height * 0.5,
                        ],
                        size: [
                            chip_x - left + 160.0 + self.text_advance(&row.label, plan.size),
                            height,
                        ],
                    },
                    10.0,
                    solid(palette.background),
                    band,
                );
                let span = [y - height * 0.5, y + height * 0.5];
                bars_grid(pixels, canvas, plan, grid, span, palette.muted);
            }
            self.viz_text(
                pixels,
                &row.label,
                plan.size,
                palette.text,
                Anchor::Right,
                [left - 24.0, y],
                alpha,
            );
            for (series_index, series) in plan.series.iter().enumerate() {
                let channel = format!("bar.{}.{}", row.id, series.id);
                let value = sample(&channel, 0.0);
                let end = plan.bar_end(value) + shift[0];
                let bar_y = y + plan.bar_offset(series_index);
                let length = end - left;
                let color = self.series_ink(series.tone);
                if length > 0.25 {
                    let radius = (thickness * 0.3).min(5.0);
                    UiCanvas::new(pixels, canvas).fill(
                        Bounds {
                            origin: [left, bar_y - thickness * 0.5],
                            size: [length.max(radius * 2.0), thickness],
                        },
                        radius,
                        solid(color),
                        alpha * (length / (radius * 2.0)).min(1.0),
                    );
                }
                let label = alpha * smoothstep(length / 24.0);
                let ink = [(
                    if series.tone == Tone::Muted {
                        palette.muted
                    } else {
                        mix(color, palette.text, 0.35)
                    },
                    1.0,
                )];
                self.draw_readout(
                    pixels,
                    &Readout {
                        velocity: velocity(&channel),
                        ..bar_readout(plan, value, &ink, palette.muted)
                    },
                    Anchor::Left,
                    [end.max(left) + 12.0, bar_y],
                    label,
                );
            }
            if let Some(delta) = &plan.delta {
                let chip = sample(&format!("delta.{}", row.id), 0.0).clamp(0.0, 1.2);
                let from = sample(&format!("bar.{}.{}", row.id, delta.from), 0.0);
                let to = sample(&format!("bar.{}.{}", row.id, delta.to), 0.0);
                if let Some((text, tone)) = delta.text(from, to)
                    && chip > 0.001
                {
                    self.bars_chip(pixels, &text, tone, [chip_x, y], plan.size, alpha, chip);
                }
            }
        }
    }

    fn series_ink(&self, tone: Tone) -> [u8; 3] {
        match tone {
            Tone::Muted => mix(
                self.theme.palette().muted,
                self.theme.palette().raised,
                0.35,
            ),
            tone => self.theme.tone(tone),
        }
    }

    /// A tinted pill with the delta in its tone. `pop` springs past 1 as it
    /// lands; the chip slides in from the left as it appears.
    #[allow(clippy::too_many_arguments)]
    fn bars_chip(
        &mut self,
        pixels: &mut [u8],
        text: &str,
        tone: Tone,
        [x, y]: [f32; 2],
        size: f32,
        alpha: f32,
        pop: f32,
    ) {
        let palette = self.theme.palette();
        let ink = match tone {
            Tone::Muted => palette.muted,
            tone => self.theme.tone(tone),
        };
        let presence = alpha * pop.min(1.0);
        let size = (size * 0.9).round();
        let width = self.text_advance(text, size) + size * 1.1;
        let height = (size * 1.45).round();
        let slide = (1.0 - pop) * 12.0;
        let fill = mix(palette.surface, ink, 0.16);
        let [fr, fg, fb] = fill;
        let [br, bg, bb] = ink;
        UiCanvas::new(pixels, self.viz_canvas()).surface(
            Bounds {
                origin: [x - slide, y - height * 0.5],
                size: [width, height],
            },
            SurfaceStyle::new(Fill::Solid(UiColor::srgb8(fr, fg, fb, 255)), height * 0.5).border(
                1.2,
                UiColor::srgb8(br, bg, bb, 255),
                0.45,
            ),
            presence,
        );
        self.viz_text(
            pixels,
            text,
            size,
            ink,
            Anchor::Center,
            [x - slide + width * 0.5, y],
            presence,
        );
    }
}

/// Where the grid sits and how far it has drawn on.
#[derive(Clone, Copy)]
struct Grid {
    left: f32,
    top: f32,
    bottom: f32,
    axes: f32,
    opacity: f32,
}

/// Faint gridlines at the ticks and the bars' baseline, between rows `span`
/// (the whole grid, or a moving row's band that covered it).
fn bars_grid(
    pixels: &mut [u8],
    canvas: [u32; 2],
    plan: &BarsPlan,
    grid: Grid,
    span: [f32; 2],
    color: [u8; 3],
) {
    let rows = |bottom: f32| [grid.top.max(span[0]), bottom.min(span[1])];
    let [from, to] = rows(grid.bottom);
    for &tick in &plan.axis.ticks {
        let f = plan.axis.fraction(tick);
        let alpha = grid.opacity * disclosure(f, grid.axes) * 0.14;
        if alpha > 0.001 && f > 0.0 && to > from {
            let x = grid.left + f * plan.width;
            stroke(pixels, canvas, &[[x, from], [x, to]], 1.0, color, alpha);
        }
    }
    // The value axis starts at the bars' baseline and draws down.
    let [from, to] = rows(grid.top + (grid.bottom - grid.top) * grid.axes);
    if to > from {
        let line = [[grid.left, from], [grid.left, to]];
        stroke(pixels, canvas, &line, 1.25, color, grid.opacity * 0.5);
    }
}

/// The Readout at a bar's end.
fn bar_readout<'a>(
    plan: &'a BarsPlan,
    value: f32,
    ink: &'a [([u8; 3], f32)],
    affix: [u8; 3],
) -> Readout<'a> {
    Readout {
        format: &plan.readout,
        value,
        velocity: 0.0,
        size: (plan.size * 0.85).round(),
        affix_scale: 0.8,
        ink,
        affix,
    }
}

#[cfg(test)]
mod gpu_tests {
    use psychopomp::{
        axis::AxisPlan,
        bars::{BarDeltaPlan, BarSeriesPlan, BarsPlan},
        tone::Tone,
    };

    use crate::render::{HeadlessRenderer, RenderSpec};

    #[test]
    #[ignore = "requires a headless GPU; bars, readouts, slots, and chips reach pixels"]
    fn bars_grow_sort_and_chip() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "bars-proof".into(),
        }))
        .unwrap();
        let plan = BarsPlan::new(
            [600.0, 300.0],
            800.0,
            AxisPlan::new([0.0, 2000.0]).every(500.0),
        )
        .series(BarSeriesPlan::new("before", "before", Tone::Muted))
        .series(BarSeriesPlan::new("after", "after", Tone::Accent))
        .row("a", "cold start")
        .row("b", "warm start")
        .delta(BarDeltaPlan::new("before", "after"));
        let background = renderer.render_title_card("", None, 0.);
        let mut draw = |grown: f32, slot: f32, chip: f32| {
            let mut pixels = background.clone();
            renderer.composite_bars(
                &mut pixels,
                &plan,
                |property, default| match property {
                    "bar.a.before" => 1800.0 * grown,
                    "bar.a.after" => 1200.0 * grown,
                    "row.a.slot" => slot,
                    "delta.a" => chip,
                    _ => default,
                },
                |_| 0.0,
            );
            pixels
        };
        let empty = draw(0.0, 0.0, 0.0);
        let full = draw(1.0, 0.0, 0.0);
        assert!(empty != full);
        assert!(draw(1.0, 1.0, 0.0) != full, "slots move rows");
        assert!(draw(1.0, 0.0, 1.0) != full, "chips reach pixels");
        assert!(draw(1.0, 0.0, 0.0) == full);
    }
}
