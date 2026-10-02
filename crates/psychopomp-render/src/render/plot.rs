//! Plot pixels: gridlines and axes, curves drawn on along their length, marks,
//! a legend, and a playhead whose dots ride curves with tangent arrows. Every
//! curve is the Scene Program's sampled points; nothing is carried between
//! frames.
use psychopomp::{
    math::{remap_clamp, smoothstep},
    plot::{PlotPlan, PlotSeriesPlan},
};

use super::{
    HeadlessRenderer,
    chart::{
        Anchor, AxisDraw, HAIRLINE, Label, Side, TICK_SIZE, dashed, disclosure, dot, path, points,
        stroke,
    },
    theme::mix,
};

const CURVE: f32 = 2.5;
const LABEL_SIZE: f32 = 17.0;
/// The velocity arrow spans this fraction of the x range ahead of its dot.
const VELOCITY_LEAD: f32 = 0.08;
const MAX_ARROW: f32 = 240.0;
/// The legend row and y label, above the frame's top edge.
const LEGEND_Y: f32 = -46.0;

impl HeadlessRenderer {
    pub(crate) fn composite_plot(
        &mut self,
        pixels: &mut [u8],
        plan: &PlotPlan,
        sample: impl Fn(&str, f32) -> f32,
    ) {
        let opacity = sample("opacity", 1.0).clamp(0.0, 1.0);
        if opacity <= 0.001 {
            return;
        }
        let shift = [sample("x", 0.0), sample("y", 0.0)];
        let canvas = [self.spec.width, self.spec.height];
        let palette = self.theme.palette();
        let axes = sample("axes", 1.0).clamp(0.0, 1.0);
        let [left, top] = plan.origin;
        let [right, bottom] = [left + plan.size[0], top + plan.size[1]];
        let at = |[x, y]: [f32; 2]| {
            let [cx, cy] = plan.to_canvas([x, y]);
            [cx + shift[0], cy + shift[1]]
        };
        let playhead = sample("playhead", f32::NAN);
        let head = if playhead.is_finite() {
            opacity * sample("playhead.opacity", 1.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let playhead_x = plan.to_canvas([playhead, plan.y.range[0]])[0] + shift[0];

        // Gridlines at the y ticks draw across with the axes.
        for &tick in &plan.y.ticks {
            let y = plan.to_canvas([plan.x.range[0], tick])[1] + shift[1];
            let alpha = opacity * disclosure(plan.y.fraction(tick), axes);
            if alpha > 0.001 && (y - bottom - shift[1]).abs() > 0.5 {
                stroke(
                    pixels,
                    canvas,
                    &[
                        [left + shift[0], y],
                        [left + shift[0] + plan.size[0] * axes, y],
                    ],
                    1.0,
                    palette.muted,
                    alpha * 0.16,
                );
            }
        }
        let axis = |axis, line, side, avoid| AxisDraw {
            axis,
            line,
            side,
            reveal: axes,
            opacity,
            shift,
            avoid,
        };
        self.chart_axis(
            pixels,
            axis(&plan.y, [[left, bottom], [left, top]], Side::Left, None),
        );
        self.chart_axis(
            pixels,
            axis(
                &plan.x,
                [[left, bottom], [right, bottom]],
                Side::Below,
                (head > 0.0).then_some((playhead_x, head)),
            ),
        );
        let axis_label = |text, anchor, at| Label {
            text,
            size: TICK_SIZE,
            color: palette.muted,
            anchor,
            at,
            opacity: opacity * smoothstep(remap_clamp(axes, [0.6, 1.0], [0.0, 1.0])),
        };
        self.chart_label(
            pixels,
            axis_label(&plan.y.label, Anchor::Left, [left - 8.0, LEGEND_Y + top]),
            shift,
        );
        self.chart_label(
            pixels,
            axis_label(&plan.x.label, Anchor::Right, [right, bottom + 58.0]),
            shift,
        );

        for mark in &plan.marks {
            let alpha = opacity * sample(&format!("mark.{}.opacity", mark.id), 1.0).clamp(0.0, 1.0);
            if alpha <= 0.001 {
                continue;
            }
            let x = plan.to_canvas([mark.x, plan.y.range[0]])[0];
            stroke(
                pixels,
                canvas,
                &[
                    [x + shift[0], top + shift[1]],
                    [x + shift[0], bottom + shift[1]],
                ],
                HAIRLINE,
                palette.muted,
                alpha * 0.45,
            );
            self.chart_label(
                pixels,
                Label {
                    text: &mark.label,
                    size: TICK_SIZE,
                    color: palette.muted,
                    anchor: Anchor::Left,
                    at: [x + 10.0, top + 14.0],
                    opacity: alpha,
                },
                shift,
            );
        }

        let draw_of = |series: &PlotSeriesPlan| {
            sample(&format!("series.{}.draw", series.id), 1.0).clamp(0.0, 1.0)
        };
        // The legend flows after the y label. An entry claims its room as its
        // curve appears or fades, so later entries glide rather than jump.
        let mut legend_left = left + self.chart_advance(&plan.y.label, TICK_SIZE).ceil() + 64.0;
        for series in plan.series.iter().filter(|series| !series.label.is_empty()) {
            let series_opacity = self.series_opacity(series, &sample);
            let drawn = smoothstep(draw_of(series) / 0.25);
            let width = self.plot_legend(
                pixels,
                series,
                [legend_left, LEGEND_Y + top],
                shift,
                opacity * series_opacity * drawn,
            );
            // A dimmed entry keeps its room; only a vanishing one gives it up.
            legend_left += width * drawn * smoothstep(series_opacity / 0.3);
        }
        for series in &plan.series {
            let draw = draw_of(series);
            let alpha = opacity * self.series_opacity(series, &sample);
            if draw <= 0.0 || alpha <= 0.001 {
                continue;
            }
            let color = self.theme.tone(series.tone);
            let curve = path(series.points.iter().map(|&p| at(p)));
            let drawn = curve.slice(0.0, draw);
            if series.dashed {
                dashed(pixels, canvas, &drawn, CURVE - 0.5, color, alpha);
            } else {
                stroke(pixels, canvas, &points(&drawn), CURVE, color, alpha);
            }
            // The pen: a small head that exists only while the curve draws.
            let pen = smoothstep(draw / 0.04) * smoothstep((1.0 - draw) / 0.04);
            if pen > 0.001 {
                let head = curve.at(draw);
                dot(pixels, canvas, [head.x, head.y], 3.6, color, alpha * pen);
            }
        }

        if !playhead.is_finite() {
            return;
        }
        let x = playhead_x;
        if head > 0.001 {
            stroke(
                pixels,
                canvas,
                &[[x, top + shift[1]], [x, bottom + shift[1]]],
                HAIRLINE,
                palette.accent,
                head * 0.5,
            );
            let decimals = (plan.x.decimals() + 1).min(3);
            let readout = format!("{playhead:.decimals$}{}", plan.x.unit);
            self.chart_tab(
                pixels,
                &readout,
                [x, bottom + 27.0 + shift[1]],
                palette.accent,
                palette.background,
                head,
            );
        }
        for series in &plan.series {
            let ride = sample(&format!("series.{}.ride", series.id), 0.0).clamp(0.0, 1.0);
            let alpha = opacity * ride * self.series_opacity(series, &sample);
            if alpha <= 0.001 {
                continue;
            }
            let color = self.theme.tone(series.tone);
            let point = at([playhead, series.y_at(playhead)]);
            let velocity = sample(&format!("series.{}.velocity", series.id), 0.0).clamp(0.0, 1.0);
            if velocity > 0.001 {
                self.plot_velocity(
                    pixels,
                    plan,
                    series,
                    playhead,
                    shift,
                    color,
                    alpha * velocity,
                );
            }
            dot(pixels, canvas, point, 8.5, palette.background, alpha);
            dot(pixels, canvas, point, 5.5, color, alpha);
        }
    }

    fn series_opacity(&self, series: &PlotSeriesPlan, sample: &impl Fn(&str, f32) -> f32) -> f32 {
        sample(&format!("series.{}.opacity", series.id), 1.0).clamp(0.0, 1.0)
    }

    /// One legend entry (a swatch, then the label) starting at `left`;
    /// returns the room it takes, including the gap before the next entry.
    fn plot_legend(
        &mut self,
        pixels: &mut [u8],
        series: &PlotSeriesPlan,
        [left, y]: [f32; 2],
        shift: [f32; 2],
        alpha: f32,
    ) -> f32 {
        let color = self.theme.tone(series.tone);
        let width = self.chart_advance(&series.label, LABEL_SIZE);
        if alpha > 0.001 {
            let ends = [
                [left + shift[0], y.round() + shift[1]],
                [left + 24.0 + shift[0], y.round() + shift[1]],
            ];
            let canvas = [self.spec.width, self.spec.height];
            if series.dashed {
                dashed(pixels, canvas, &path(ends), CURVE - 0.5, color, alpha);
            } else {
                stroke(pixels, canvas, &ends, CURVE, color, alpha);
            }
            let text = mix(self.theme.palette().muted, self.theme.palette().text, 0.7);
            self.chart_label(
                pixels,
                Label {
                    text: &series.label,
                    size: LABEL_SIZE,
                    color: text,
                    anchor: Anchor::Left,
                    at: [36.0, y],
                    opacity: alpha,
                },
                [left + shift[0], shift[1]],
            );
        }
        width.ceil() + 36.0 + 40.0
    }

    /// The tangent at the riding dot: where the dot will be a short time
    /// ahead, so its length grows with speed, with the signed slope beside it.
    #[allow(clippy::too_many_arguments)]
    fn plot_velocity(
        &mut self,
        pixels: &mut [u8],
        plan: &PlotPlan,
        series: &PlotSeriesPlan,
        x: f32,
        shift: [f32; 2],
        color: [u8; 3],
        alpha: f32,
    ) {
        let [px, py] = plan.to_canvas([x, series.y_at(x)]);
        let point = [px + shift[0], py + shift[1]];
        let slope = series.slope_at(x);
        let lead = VELOCITY_LEAD * (plan.x.range[1] - plan.x.range[0]);
        let mut vector = [
            lead / (plan.x.range[1] - plan.x.range[0]) * plan.size[0],
            -slope * lead / (plan.y.range[1] - plan.y.range[0]) * plan.size[1],
        ];
        let length = vector[0].hypot(vector[1]);
        if length > MAX_ARROW {
            vector = vector.map(|v| v * MAX_ARROW / length);
        }
        let length = length.min(MAX_ARROW);
        let direction = vector.map(|v| v / length);
        let start = [
            point[0] + direction[0] * 11.0,
            point[1] + direction[1] * 11.0,
        ];
        let tip = [point[0] + vector[0], point[1] + vector[1]];
        let canvas = [self.spec.width, self.spec.height];
        if length > 16.0 {
            stroke(pixels, canvas, &[start, tip], 2.0, color, alpha);
        }
        let barb = |angle: f32| {
            let (sin, cos) = angle.sin_cos();
            let back = [-direction[0], -direction[1]];
            [
                tip[0] + (back[0] * cos - back[1] * sin) * 11.0,
                tip[1] + (back[0] * sin + back[1] * cos) * 11.0,
            ]
        };
        stroke(
            pixels,
            canvas,
            &[barb(0.5), tip, barb(-0.5)],
            2.0,
            color,
            alpha,
        );
        // The signed slope reads out above the frame, on the playhead's line.
        self.chart_label(
            pixels,
            Label {
                text: &format!("v {slope:+.2}"),
                size: TICK_SIZE,
                color,
                anchor: Anchor::Center,
                at: [0.0, 0.0],
                opacity: alpha,
            },
            [point[0], plan.origin[1] + shift[1] - 16.0],
        );
    }
}

#[cfg(test)]
mod gpu_tests {
    use psychopomp::{
        axis::AxisPlan,
        plot::{PlotPlan, PlotSeriesPlan},
        tone::Tone,
    };

    use crate::render::HeadlessRenderer;

    #[test]
    #[ignore = "requires a headless GPU; draw progress and the playhead reach pixels without residue"]
    fn plots_draw_on_and_ride_without_residue() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(crate::render::RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "plot-proof".into(),
        }))
        .unwrap();
        let plan = PlotPlan::new(
            [300.0, 250.0],
            [1300.0, 560.0],
            AxisPlan::new([0.0, 1.0]).every(0.25),
            AxisPlan::new([0.0, 1.0]).every(0.5),
        )
        .series(PlotSeriesPlan::sampled(
            "ease",
            "ease",
            Tone::Accent,
            [0.0, 1.0],
            120,
            |x| x * x * (3.0 - 2.0 * x),
        ));
        let background = renderer.render_title_card("", None, 0.);
        let draw = |renderer: &mut HeadlessRenderer, opacity: f32, drawn: f32, playhead: f32| {
            let mut pixels = background.clone();
            renderer.composite_plot(&mut pixels, &plan, |property, default| match property {
                "opacity" => opacity,
                "series.ease.draw" => drawn,
                "playhead" => playhead,
                "series.ease.ride" | "series.ease.velocity" => 1.0,
                _ => default,
            });
            pixels
        };
        assert!(
            draw(&mut renderer, 0.0, 1.0, 0.5) == background,
            "hidden plots leave no ink"
        );
        let half = draw(&mut renderer, 1.0, 0.5, f32::NAN);
        let full = draw(&mut renderer, 1.0, 1.0, f32::NAN);
        assert!(half != full, "draw progress reaches pixels");
        let early = draw(&mut renderer, 1.0, 1.0, 0.25);
        assert!(early != full, "the playhead reaches pixels");
        draw(&mut renderer, 1.0, 1.0, 0.75);
        assert!(
            draw(&mut renderer, 1.0, 1.0, 0.25) == early,
            "sampling order cannot change a frame"
        );
    }
}
