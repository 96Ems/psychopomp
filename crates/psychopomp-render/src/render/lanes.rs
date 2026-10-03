//! Lanes pixels: cue brackets, a seconds ruler, named lanes with sparklines and
//! keyframe diamonds, and a scrubbing playhead. Keys light as the playhead
//! crosses them and cool over a fixed span of playhead time, so scrubbing in
//! either direction samples deterministically.
use psychopomp::{
    lanes::{LANES_HEADER, LanePlan, LanesPlan},
    math::{easing::cubic_out, lerp, remap_clamp, smoothstep},
};

use super::{
    HeadlessRenderer,
    chart::{
        Anchor, Avoid, AxisDraw, HAIRLINE, Label, Side, TICK_LABEL_OFFSET, TICK_SIZE, TITLE_SIZE,
        diamond, disclosure, path, points, stroke,
    },
    theme::mix,
};

const KEY_RADIUS: f32 = 6.5;
/// Playhead seconds over which a crossed keyframe cools from the accent.
const KEY_FLASH: f32 = 0.45;

impl HeadlessRenderer {
    pub(crate) fn composite_lanes(
        &mut self,
        pixels: &mut [u8],
        plan: &LanesPlan,
        sample: impl Fn(&str, f32) -> f32,
    ) {
        let opacity = sample("opacity", 1.0).clamp(0.0, 1.0);
        if opacity <= 0.001 {
            return;
        }
        let shift = [sample("x", 0.0), sample("y", 0.0)];
        let reveal = sample("reveal", 1.0).clamp(0.0, 1.0);
        let palette = self.theme.palette();
        let canvas = [self.spec.width, self.spec.height];
        let [left, top] = plan.origin;
        let track = [left + plan.label_width, left + plan.width];
        let time_x = |t: f32| lerp(track[0], track[1], plan.time.fraction(t));
        let ruler_y = top + LANES_HEADER - 8.0;
        let lanes_top = top + LANES_HEADER;
        let bottom = lanes_top + plan.lane_height * plan.lanes.len() as f32;
        let playhead = sample("playhead", f32::NAN);
        let head = if playhead.is_finite() {
            opacity * sample("playhead.opacity", 1.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let played = |t: f32| playhead.is_finite() && t <= playhead;

        let playhead_x = time_x(playhead.clamp(plan.time.range[0], plan.time.range[1])) + shift[0];
        let readout = format!(
            "{playhead:.decimals$}{}",
            plan.time.unit,
            decimals = (plan.time.decimals() + 2).min(3)
        );
        let tab = (head > 0.0).then(|| Avoid {
            x: playhead_x,
            width: self.chart_tab_width(&readout),
            strength: head,
        });
        self.chart_axis(
            pixels,
            AxisDraw {
                axis: &plan.time,
                line: [[track[0], ruler_y], [track[1], ruler_y]],
                side: Side::Above,
                reveal: remap_clamp(reveal, [0.0, 0.45], [0.0, 1.0]),
                opacity,
                shift,
                avoid: tab,
            },
        );

        let cues = opacity * smoothstep(remap_clamp(reveal, [0.6, 1.0], [0.0, 1.0]));
        for cue in &plan.cues {
            if cues <= 0.001 {
                break;
            }
            let [start, end] = [cue.start, cue.end]
                .map(|t| time_x(t.clamp(plan.time.range[0], plan.time.range[1])));
            if end - start < 1.0 {
                continue;
            }
            let active = playhead.is_finite() && (cue.start..cue.end).contains(&playhead);
            let color = if active { palette.text } else { palette.muted };
            let y = top + 36.0;
            let bracket = [
                [start + 1.5, y + 8.0],
                [start + 1.5, y],
                [end - 1.5, y],
                [end - 1.5, y + 8.0],
            ]
            .map(|[x, y]| [x + shift[0], y + shift[1]]);
            stroke(
                pixels,
                canvas,
                &bracket,
                HAIRLINE,
                color,
                cues * if active { 0.9 } else { 0.6 },
            );
            self.chart_label(
                pixels,
                Label {
                    text: &cue.label,
                    size: TICK_SIZE,
                    color,
                    anchor: Anchor::Center,
                    at: [(start + end) * 0.5, top + 16.0],
                    opacity: cues,
                },
                shift,
            );
        }

        let count = plan.lanes.len() as f32;
        for (index, lane) in plan.lanes.iter().enumerate() {
            // Lanes arrive top to bottom while the ruler draws.
            let start = 0.12 + 0.5 * index as f32 / count;
            let arrive = cubic_out(remap_clamp(reveal, [start, start + 0.3], [0.0, 1.0]));
            let alpha = opacity
                * arrive
                * sample(&format!("lane.{}.opacity", lane.id), 1.0).clamp(0.0, 1.0);
            if alpha <= 0.001 {
                continue;
            }
            let emphasis = sample(&format!("lane.{}.emphasis", lane.id), 0.0).clamp(0.0, 1.0);
            let row_top = lanes_top + plan.lane_height * index as f32;
            let lift = [shift[0], shift[1] + (1.0 - arrive) * 8.0];
            stroke(
                pixels,
                canvas,
                &[
                    [left + lift[0], row_top + plan.lane_height + lift[1]],
                    [track[1] + lift[0], row_top + plan.lane_height + lift[1]],
                ],
                1.0,
                palette.muted,
                alpha * 0.18,
            );
            self.chart_label(
                pixels,
                Label {
                    text: &lane.label,
                    size: TITLE_SIZE,
                    color: mix(palette.muted, palette.text, 0.35 + 0.65 * emphasis),
                    anchor: Anchor::Left,
                    at: [left, row_top + plan.lane_height * 0.5],
                    opacity: alpha,
                },
                lift,
            );
            let band = [row_top + 10.0, row_top + plan.lane_height - 10.0];
            let value_y = lane_value_y(lane, band);
            let at = |t: f32| [time_x(t) + lift[0], value_y(t) + lift[1]];
            // The curve and keys draw on left to right as the lane arrives.
            let drawn_until = lerp(plan.time.range[0], plan.time.range[1], arrive);
            if lane.curve.len() >= 2 {
                let lit = mix(palette.text, palette.accent, emphasis);
                let in_range = |t: f32| t >= plan.time.range[0] && t <= drawn_until;
                let split = if playhead.is_finite() {
                    playhead.min(drawn_until)
                } else {
                    plan.time.range[0]
                };
                let behind = clip_curve(&lane.curve, plan.time.range[0], split);
                let ahead = clip_curve(&lane.curve, split, drawn_until);
                for (part, color, weight) in [
                    (ahead, palette.muted, 0.5),
                    (behind, lit, 0.4 + 0.5 * emphasis.max(head)),
                ] {
                    let part = part
                        .into_iter()
                        .filter(|p| in_range(p[0]))
                        .map(|p| at(p[0]));
                    stroke(
                        pixels,
                        canvas,
                        &points(&path(part)),
                        1.6,
                        color,
                        alpha * weight,
                    );
                }
            }
            for &key in &lane.keys {
                if !plan.time.contains(key) {
                    continue;
                }
                let key_alpha = alpha * disclosure(plan.time.fraction(key), arrive);
                if key_alpha <= 0.001 {
                    continue;
                }
                let center = at(key);
                if played(key) {
                    // Instant attack, convex decay: light from the playhead.
                    let age = (playhead - key) / KEY_FLASH;
                    let flash = (1.0 - age).clamp(0.0, 1.0).powi(2) * head;
                    if flash > 0.001 {
                        diamond(
                            pixels,
                            canvas,
                            center,
                            KEY_RADIUS + 3.0 + 7.0 * age.min(1.0),
                            palette.accent,
                            None,
                            key_alpha * flash * 0.25,
                        );
                    }
                    let fill = mix(
                        mix(palette.text, palette.accent, emphasis),
                        palette.accent,
                        flash,
                    );
                    diamond(pixels, canvas, center, KEY_RADIUS, fill, None, key_alpha);
                } else {
                    diamond(
                        pixels,
                        canvas,
                        center,
                        KEY_RADIUS,
                        palette.background,
                        Some((mix(palette.muted, palette.text, emphasis), 1.5)),
                        key_alpha,
                    );
                }
            }
        }

        if head > 0.001 {
            let x = playhead_x;
            stroke(
                pixels,
                canvas,
                &[
                    [x, ruler_y - TICK_LABEL_OFFSET + shift[1]],
                    [x, bottom + shift[1]],
                ],
                1.5,
                palette.accent,
                head * 0.85,
            );
            self.chart_tab(
                pixels,
                &readout,
                [x, ruler_y - TICK_LABEL_OFFSET + shift[1]],
                palette.accent,
                palette.background,
                head,
            );
        }
    }
}

/// Canvas y for a lane's value at time `t` within `band`: the curve scaled to
/// its own range (flat curves sit mid-band), or the band's middle without one.
fn lane_value_y(lane: &LanePlan, [high, low]: [f32; 2]) -> impl Fn(f32) -> f32 + '_ {
    let range = lane.value_range();
    move |t| match range {
        Some([lo, hi]) if hi - lo > 1e-6 => {
            let value = curve_at(&lane.curve, t);
            lerp(low, high, (value - lo) / (hi - lo))
        }
        _ => (high + low) * 0.5,
    }
}

fn curve_at(curve: &[[f32; 2]], t: f32) -> f32 {
    let index = curve
        .partition_point(|p| p[0] <= t)
        .clamp(1, curve.len() - 1);
    let (a, b) = (curve[index - 1], curve[index]);
    if b[0] > a[0] {
        lerp(a[1], b[1], ((t - a[0]) / (b[0] - a[0])).clamp(0.0, 1.0))
    } else {
        b[1]
    }
}

/// The curve between two times, with interpolated end points.
fn clip_curve(curve: &[[f32; 2]], from: f32, to: f32) -> Vec<[f32; 2]> {
    if to <= from {
        return Vec::new();
    }
    std::iter::once([from, curve_at(curve, from)])
        .chain(curve.iter().copied().filter(|p| p[0] > from && p[0] < to))
        .chain(std::iter::once([to, curve_at(curve, to)]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::clip_curve;

    #[test]
    fn clipped_curves_keep_interpolated_ends() {
        let curve = [[0.0, 0.0], [1.0, 10.0], [2.0, 10.0]];
        assert_eq!(
            clip_curve(&curve, 0.5, 1.5),
            vec![[0.5, 5.0], [1.0, 10.0], [1.5, 10.0]]
        );
        assert!(clip_curve(&curve, 1.0, 1.0).is_empty());
    }
}

#[cfg(test)]
mod gpu_tests {
    use psychopomp::{
        axis::AxisPlan,
        lanes::{LanePlan, LanesPlan},
    };

    use crate::render::HeadlessRenderer;

    #[test]
    #[ignore = "requires a headless GPU; reveal and scrubbing reach pixels and sample in any order"]
    fn lanes_reveal_and_scrub_without_residue() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(crate::render::RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "lanes-proof".into(),
        }))
        .unwrap();
        let plan = LanesPlan::new(
            [160.0, 200.0],
            1600.0,
            AxisPlan::new([0.0, 4.0]).every(1.0).unit("s"),
        )
        .lane(
            LanePlan::new("card.x", "card.x")
                .keys([0.5, 2.0])
                .curve(vec![[0.0, 0.0], [0.5, 0.0], [1.5, 1.0], [4.0, 1.0]]),
        )
        .cue("intro", [0.0, 2.0], "intro");
        let background = renderer.render_title_card("", None, 0.);
        let draw = |renderer: &mut HeadlessRenderer, opacity: f32, reveal: f32, playhead: f32| {
            let mut pixels = background.clone();
            renderer.composite_lanes(&mut pixels, &plan, |property, default| match property {
                "opacity" => opacity,
                "reveal" => reveal,
                "playhead" => playhead,
                _ => default,
            });
            pixels
        };
        assert!(
            draw(&mut renderer, 0.0, 1.0, 1.0) == background,
            "hidden lanes leave no ink"
        );
        let half = draw(&mut renderer, 1.0, 0.5, f32::NAN);
        let full = draw(&mut renderer, 1.0, 1.0, f32::NAN);
        assert!(half != full, "reveal reaches pixels");
        let early = draw(&mut renderer, 1.0, 1.0, 0.6);
        assert!(early != full, "the playhead reaches pixels");
        draw(&mut renderer, 1.0, 1.0, 3.0);
        assert!(
            draw(&mut renderer, 1.0, 1.0, 0.6) == early,
            "sampling order cannot change a frame"
        );
    }
}
