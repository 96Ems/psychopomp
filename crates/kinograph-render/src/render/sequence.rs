//! Sequence Diagram pixels: participant headers, dashed lifelines, travelling
//! messages, notes and termination marks. All geometry comes from the recipe and
//! sampled channels; nothing is carried between frames.
use kinograph::math::easing::cubic_out;
use kinograph::{
    sequence::{SequencePlan, SequenceRowPlan},
    tone::Tone,
};

use super::{
    HeadlessRenderer, PlainTextSpec, composite_text_region,
    ui::{
        Bounds,
        card::{Fill, SurfaceStyle, UiCanvas, UiColor},
    },
};

const LABEL_SIZE: f32 = 24.0;
const DETAIL_SIZE: f32 = 17.0;
const MESSAGE_SIZE: f32 = 21.0;
const NOTE_SIZE: f32 = 20.0;
const ASIDE_SIZE: f32 = 17.0;
const STROKE: f32 = 2.4;
const DASH: f32 = 7.0;
const DASH_GAP: f32 = 7.0;

#[derive(Clone, Copy)]
enum Anchor {
    Left,
    Center,
    Right,
}

impl HeadlessRenderer {
    pub(crate) fn composite_sequence(
        &mut self,
        pixels: &mut [u8],
        plan: &SequencePlan,
        sample: impl Fn(&str, f32) -> f32,
    ) {
        let opacity = sample("opacity", 1.0).clamp(0.0, 1.0);
        if opacity <= 0.001 {
            return;
        }
        let offset = [sample("x", 0.0), sample("y", 0.0)];
        let palette = self.theme.palette();
        let participant_alpha = plan
            .participants
            .iter()
            .map(|p| {
                opacity * sample(&format!("participant.{}.opacity", p.id), 1.0).clamp(0.0, 1.0)
            })
            .collect::<Vec<_>>();
        let x_of = |index: usize| plan.participant_x(index) + offset[0];
        let slots = plan.row_slots();
        let y_of = |row: usize| plan.slot_y(slots[row]) + offset[1];
        let reveal_of = |id: &str| sample(&format!("row.{id}.reveal"), 0.0).clamp(0.0, 1.0);
        let row_opacity_of = |id: &str| sample(&format!("row.{id}.opacity"), 1.0).clamp(0.0, 1.0);

        // Lifelines first: everything else sits on top of them.
        let header_height = plan.header_height();
        let top = plan.origin[1] + offset[1] + header_height + 6.0;
        let bottom = plan.lifeline_bottom() + offset[1];
        let extent = sample("lifelines", 1.0).clamp(0.0, 1.0);
        let lifeline_end = top + (bottom - top) * extent;
        for (index, participant) in plan.participants.iter().enumerate() {
            let alpha = participant_alpha[index] * 0.5;
            if alpha <= 0.001 || lifeline_end <= top {
                continue;
            }
            // A revealed End row dims the lifeline below its mark.
            let ended = plan
                .rows
                .iter()
                .enumerate()
                .find_map(|(row_index, row)| match row {
                    SequenceRowPlan::End {
                        id,
                        participant: who,
                        ..
                    } if *who == participant.id => {
                        Some((y_of(row_index), reveal_of(id) * row_opacity_of(id)))
                    }
                    _ => None,
                });
            let x = x_of(index);
            let mut canvas = UiCanvas::new(pixels, self.canvas_size());
            let mut y = top;
            while y < lifeline_end {
                let end = (y + DASH).min(lifeline_end);
                let dim = match ended {
                    Some((end_y, reveal)) if y > end_y + 12.0 => 1.0 - 0.82 * reveal,
                    _ => 1.0,
                };
                canvas.fill(
                    Bounds {
                        origin: [x - 0.9, y],
                        size: [1.8, end - y],
                    },
                    0.9,
                    solid(palette.muted, 255),
                    alpha * dim,
                );
                y += DASH + DASH_GAP;
            }
        }

        // Participant headers.
        for (index, participant) in plan.participants.iter().enumerate() {
            let alpha = participant_alpha[index];
            if alpha <= 0.001 {
                continue;
            }
            let emphasis =
                sample(&format!("participant.{}.emphasis", participant.id), 0.0).clamp(0.0, 1.0);
            let label_width = self.sequence_advance(&participant.label, LABEL_SIZE, palette.text);
            let detail_width = if participant.detail.is_empty() {
                0.0
            } else {
                self.sequence_advance(&participant.detail, DETAIL_SIZE, palette.muted)
            };
            let column = plan.width / plan.participants.len() as f32;
            let width = (label_width.max(detail_width) + 60.0).clamp(150.0, column * 0.92);
            let center = [
                x_of(index),
                plan.origin[1] + offset[1] + header_height * 0.5,
            ];
            let bounds = Bounds::from_center(center, [width, header_height]);
            let border = mix(
                mix(palette.raised, palette.muted, 0.35),
                palette.accent,
                emphasis,
            );
            {
                let mut canvas = UiCanvas::new(pixels, self.canvas_size());
                if emphasis > 0.001 {
                    canvas.fill(
                        bounds.expand(7.0),
                        19.0,
                        solid(palette.accent, 255),
                        alpha * emphasis * 0.18,
                    );
                }
                canvas.surface(
                    bounds,
                    SurfaceStyle::new(solid(palette.surface, 255), 12.0).border(
                        1.5,
                        rgba(border, 255),
                        1.0,
                    ),
                    alpha,
                );
            }
            let label_y = if participant.detail.is_empty() {
                center[1]
            } else {
                center[1] - 11.0
            };
            let label_color = mix(palette.text, palette.accent, emphasis * 0.6);
            self.sequence_text(
                pixels,
                &participant.label,
                LABEL_SIZE,
                label_color,
                Anchor::Center,
                [center[0], label_y],
                alpha,
            );
            if !participant.detail.is_empty() {
                self.sequence_text(
                    pixels,
                    &participant.detail,
                    DETAIL_SIZE,
                    palette.muted,
                    Anchor::Center,
                    [center[0], center[1] + 17.0],
                    alpha,
                );
            }
        }

        // Rows, in declaration order.
        for (row_index, row) in plan.rows.iter().enumerate() {
            let reveal = reveal_of(row.id());
            let alpha = opacity * row_opacity_of(row.id());
            if reveal <= 0.001 || alpha <= 0.001 {
                continue;
            }
            let y = y_of(row_index);
            let strike = sample(&format!("row.{}.strike", row.id()), 0.0).clamp(0.0, 1.0);
            let color = mix(self.tone_color(row.tone()), palette.muted, strike * 0.7);
            let dim = 1.0 - 0.45 * strike;
            if !row.aside().is_empty() {
                self.sequence_text(
                    pixels,
                    row.aside(),
                    ASIDE_SIZE,
                    palette.muted,
                    Anchor::Right,
                    // Clear of notes, which extend 75 px past the outer lifelines.
                    [x_of(0) - 96.0, y],
                    alpha * cubic_out((reveal / 0.4).clamp(0.0, 1.0)),
                );
            }
            match row {
                SequenceRowPlan::Message {
                    from,
                    to,
                    label,
                    reply,
                    ..
                } => {
                    let from = plan.participant_index(from).expect("validated participant");
                    let to = plan.participant_index(to).expect("validated participant");
                    self.sequence_message(
                        pixels,
                        [x_of(from), x_of(to)],
                        y,
                        label,
                        *reply,
                        color,
                        reveal,
                        strike,
                        alpha * dim,
                    );
                }
                SequenceRowPlan::Note { over, text, .. } => {
                    let xs = over
                        .iter()
                        .map(|id| x_of(plan.participant_index(id).expect("validated participant")))
                        .collect::<Vec<_>>();
                    let min = xs.iter().copied().fold(f32::INFINITY, f32::min);
                    let max = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                    self.sequence_note(
                        pixels,
                        [min, max],
                        y,
                        text,
                        color,
                        row.tone(),
                        reveal,
                        alpha * dim,
                    );
                }
                SequenceRowPlan::End {
                    participant, label, ..
                } => {
                    let index = plan
                        .participant_index(participant)
                        .expect("validated participant");
                    self.sequence_end(pixels, [x_of(index), y], label, color, reveal, alpha * dim);
                }
            }
        }
    }

    fn canvas_size(&self) -> [u32; 2] {
        [self.spec.width, self.spec.height]
    }

    fn tone_color(&self, tone: Tone) -> [u8; 3] {
        self.theme.tone(tone)
    }

    #[allow(clippy::too_many_arguments)]
    fn sequence_message(
        &mut self,
        pixels: &mut [u8],
        [from, to]: [f32; 2],
        y: f32,
        label: &str,
        reply: bool,
        color: [u8; 3],
        reveal: f32,
        strike: f32,
        alpha: f32,
    ) {
        let label_alpha = alpha * cubic_out(((reveal - 0.2) / 0.5).clamp(0.0, 1.0));
        let travel = cubic_out((reveal / 0.8).clamp(0.0, 1.0));
        let head = cubic_out(((reveal - 0.7) / 0.3).clamp(0.0, 1.0));
        if (to - from).abs() < 1.0 {
            // A message to itself: a small loop to the right of the lifeline.
            let path = [
                [from + 6.0, y - 15.0],
                [from + 50.0, y - 15.0],
                [from + 50.0, y + 15.0],
                [from + 10.0, y + 15.0],
            ];
            let drawn = path_prefix(&path, travel);
            self.stroke_path(pixels, &drawn, reply, color, alpha);
            if head > 0.0 {
                let tip = [from + 8.0, y + 15.0];
                self.composite_prototype_path(
                    pixels,
                    &[
                        [tip[0] + 11.0 * head, tip[1] - 6.5 * head],
                        tip,
                        [tip[0] + 11.0 * head, tip[1] + 6.5 * head],
                    ],
                    STROKE,
                    color,
                    alpha,
                );
            }
            let width = self.sequence_text(
                pixels,
                label,
                MESSAGE_SIZE,
                color,
                Anchor::Left,
                [from + 66.0, y],
                label_alpha,
            );
            self.strike_label(pixels, [from + 66.0, y], width, strike, color, label_alpha);
            return;
        }
        let direction = (to - from).signum();
        let start = from + direction * 7.0;
        let end = to - direction * 5.0;
        let tip = start + (end - start) * travel;
        self.stroke_path(pixels, &[[start, y], [tip, y]], reply, color, alpha);
        if head > 0.0 {
            self.composite_prototype_path(
                pixels,
                &[
                    [end - direction * 12.0 * head, y - 7.0 * head],
                    [end, y],
                    [end - direction * 12.0 * head, y + 7.0 * head],
                ],
                STROKE,
                color,
                alpha,
            );
        }
        // The message travels as a glowing packet until it lands.
        let packet = alpha * (1.0 - ((reveal - 0.72) / 0.2).clamp(0.0, 1.0));
        if packet > 0.001 && travel < 0.999 {
            let mut canvas = UiCanvas::new(pixels, self.canvas_size());
            canvas.fill(
                Bounds::from_center([tip, y], [26.0, 26.0]),
                13.0,
                solid(color, 255),
                packet * 0.18,
            );
            canvas.fill(
                Bounds::from_center([tip, y], [10.0, 10.0]),
                5.0,
                solid(color, 255),
                packet,
            );
        }
        let center = [
            (from + to) * 0.5,
            y - 22.0 + (1.0 - label_alpha / alpha.max(1e-6)) * 6.0,
        ];
        let width = self.sequence_advance(label, MESSAGE_SIZE, color);
        self.sequence_text(
            pixels,
            label,
            MESSAGE_SIZE,
            color,
            Anchor::Center,
            center,
            label_alpha,
        );
        self.strike_label(
            pixels,
            [center[0] - width * 0.5, center[1]],
            width,
            strike,
            color,
            label_alpha,
        );
    }

    fn strike_label(
        &self,
        pixels: &mut [u8],
        [left, y]: [f32; 2],
        width: f32,
        strike: f32,
        color: [u8; 3],
        alpha: f32,
    ) {
        if strike <= 0.001 || width <= 0.0 {
            return;
        }
        self.composite_prototype_path(
            pixels,
            &[
                [left - 4.0, y + 1.0],
                [left - 4.0 + (width + 8.0) * strike, y + 1.0],
            ],
            2.0,
            color,
            alpha,
        );
    }

    fn stroke_path(
        &self,
        pixels: &mut [u8],
        points: &[[f32; 2]],
        dashed: bool,
        color: [u8; 3],
        alpha: f32,
    ) {
        if points.len() < 2 {
            return;
        }
        if !dashed {
            self.composite_prototype_path(pixels, points, STROKE, color, alpha);
            return;
        }
        // Dashes are measured along the whole path so corners do not restart them.
        let total = path_length(points);
        let mut cursor = 0.0;
        while cursor < total {
            let end = (cursor + 10.0).min(total);
            let dash = path_between(points, cursor, end);
            self.composite_prototype_path(pixels, &dash, STROKE, color, alpha);
            cursor += 17.0;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn sequence_note(
        &mut self,
        pixels: &mut [u8],
        [min, max]: [f32; 2],
        y: f32,
        text: &str,
        color: [u8; 3],
        tone: Tone,
        reveal: f32,
        alpha: f32,
    ) {
        let palette = self.theme.palette();
        let appear = cubic_out((reveal / 0.7).clamp(0.0, 1.0));
        let text_width = self.sequence_advance(text, NOTE_SIZE, color);
        let natural = text_width + 56.0;
        let span = if (max - min).abs() < 1.0 {
            natural
        } else {
            (max - min + 150.0).max(natural)
        };
        let bounds = Bounds::from_center(
            [(min + max) * 0.5, y],
            [span - (1.0 - appear) * 16.0, 46.0 - (1.0 - appear) * 6.0],
        );
        let tint = if tone == Tone::Plain {
            palette.raised
        } else {
            color
        };
        {
            let mut canvas = UiCanvas::new(pixels, self.canvas_size());
            canvas.surface(
                bounds,
                SurfaceStyle::new(solid(mix(palette.surface, tint, 0.16), 255), 10.0).border(
                    1.4,
                    rgba(tint, 255),
                    0.55,
                ),
                alpha * appear,
            );
        }
        self.sequence_text(
            pixels,
            text,
            NOTE_SIZE,
            color,
            Anchor::Center,
            [(min + max) * 0.5, y],
            alpha * appear,
        );
    }

    fn sequence_end(
        &mut self,
        pixels: &mut [u8],
        [x, y]: [f32; 2],
        label: &str,
        color: [u8; 3],
        reveal: f32,
        alpha: f32,
    ) {
        let first = cubic_out((reveal / 0.45).clamp(0.0, 1.0));
        let second = cubic_out(((reveal - 0.3) / 0.45).clamp(0.0, 1.0));
        let size = 11.0;
        if first > 0.0 {
            self.composite_prototype_path(
                pixels,
                &[
                    [x - size, y - size],
                    [x - size + 2.0 * size * first, y - size + 2.0 * size * first],
                ],
                3.4,
                color,
                alpha,
            );
        }
        if second > 0.0 {
            self.composite_prototype_path(
                pixels,
                &[
                    [x + size, y - size],
                    [
                        x + size - 2.0 * size * second,
                        y - size + 2.0 * size * second,
                    ],
                ],
                3.4,
                color,
                alpha,
            );
        }
        if !label.is_empty() {
            let label_alpha = alpha * cubic_out(((reveal - 0.35) / 0.45).clamp(0.0, 1.0));
            self.sequence_text(
                pixels,
                label,
                MESSAGE_SIZE,
                color,
                Anchor::Left,
                [x + 24.0, y],
                label_alpha,
            );
        }
    }

    /// Width of `text` as drawn, from the same cached sprite used to draw it.
    fn sequence_advance(&mut self, text: &str, size: f32, color: [u8; 3]) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        self.plain_text_sprite(text, text_spec(size, color)).advance
    }

    /// Draw one line of CommitMono vertically centered on `y`; returns its width.
    #[allow(clippy::too_many_arguments)]
    fn sequence_text(
        &mut self,
        pixels: &mut [u8],
        text: &str,
        size: f32,
        color: [u8; 3],
        anchor: Anchor,
        [x, y]: [f32; 2],
        opacity: f32,
    ) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        let canvas = self.canvas_size();
        let spec = text_spec(size, color);
        let sprite = self.plain_text_sprite(text, spec);
        let advance = sprite.advance;
        let left = match anchor {
            Anchor::Left => x,
            Anchor::Center => x - advance * 0.5,
            Anchor::Right => x - advance,
        };
        composite_text_region(
            pixels,
            canvas,
            sprite,
            [left, y - spec.size[1] as f32 * 0.5],
            0.0,
            advance.ceil() + 1.0,
            0.0,
            opacity,
            [0.0, canvas[1] as f32],
            None,
        );
        advance
    }
}

fn text_spec(size: f32, color: [u8; 3]) -> PlainTextSpec {
    PlainTextSpec {
        font_size: size,
        color,
        size: [1400, (size * 1.5).ceil() as u32],
        semibold: false,
        crop_to_advance: true,
    }
}

fn solid(color: [u8; 3], alpha: u8) -> Fill {
    Fill::Solid(rgba(color, alpha))
}

fn rgba([r, g, b]: [u8; 3], alpha: u8) -> UiColor {
    UiColor::srgb8(r, g, b, alpha)
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    std::array::from_fn(|i| {
        (f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * t).round() as u8
    })
}

fn path_length(points: &[[f32; 2]]) -> f32 {
    points
        .windows(2)
        .map(|pair| distance(pair[0], pair[1]))
        .sum()
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

/// The leading `fraction` of a polyline, by length.
fn path_prefix(points: &[[f32; 2]], fraction: f32) -> Vec<[f32; 2]> {
    path_between(points, 0.0, path_length(points) * fraction.clamp(0.0, 1.0))
}

/// The part of a polyline between two distances along it.
fn path_between(points: &[[f32; 2]], from: f32, to: f32) -> Vec<[f32; 2]> {
    let mut result = Vec::new();
    let mut walked = 0.0;
    for pair in points.windows(2) {
        let length = distance(pair[0], pair[1]);
        let segment_end = walked + length;
        if segment_end >= from && walked <= to && length > 0.0 {
            let at = |d: f32| {
                let t = ((d - walked) / length).clamp(0.0, 1.0);
                [
                    pair[0][0] + (pair[1][0] - pair[0][0]) * t,
                    pair[0][1] + (pair[1][1] - pair[0][1]) * t,
                ]
            };
            if result.is_empty() {
                result.push(at(from.max(walked)));
            }
            result.push(at(to.min(segment_end)));
        }
        walked = segment_end;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{path_between, path_length, path_prefix};

    #[test]
    fn path_prefix_follows_corners_by_length() {
        let path = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]];
        assert_eq!(path_length(&path), 20.0);
        assert_eq!(path_prefix(&path, 0.25), vec![[0.0, 0.0], [5.0, 0.0]]);
        assert_eq!(
            path_prefix(&path, 0.75),
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 5.0]]
        );
        assert_eq!(
            path_between(&path, 8.0, 12.0),
            vec![[8.0, 0.0], [10.0, 0.0], [10.0, 2.0]]
        );
        assert!(path_prefix(&path, 0.0).len() <= 2);
    }
}

#[cfg(test)]
mod gpu_tests {
    use kinograph::sequence::SequencePlan;

    use crate::render::HeadlessRenderer;

    #[test]
    #[ignore = "requires a headless GPU; hidden diagrams leave no ink and sampling is order-independent"]
    fn sequence_reveals_reach_pixels_without_residue() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(crate::render::RenderSpec {
            width: 1920,
            height: 1080,
            font_path: std::path::PathBuf::from(crate::scenes::FONT_PATH),
            file_name: "sequence-proof".into(),
        }))
        .unwrap();
        let plan: SequencePlan = serde_json::from_value(serde_json::json!({
            "origin": [300, 170], "width": 1320, "rowHeight": 80,
            "participants": [{ "id": "client", "label": "client" }, { "id": "server", "label": "server" }],
            "rows": [
                { "kind": "message", "id": "probe", "from": "client", "to": "server", "label": "GET" },
                { "kind": "end", "id": "stopped", "participant": "server", "label": "stopped" }
            ]
        }))
        .unwrap();
        let background = renderer.render_title_card("", None, 0.);
        let draw = |renderer: &mut HeadlessRenderer, opacity: f32, reveal: f32| {
            let mut pixels = background.clone();
            renderer.composite_sequence(&mut pixels, &plan, |property, default| match property {
                "opacity" => opacity,
                "row.probe.reveal" | "row.stopped.reveal" => reveal,
                _ => default,
            });
            pixels
        };
        assert!(
            draw(&mut renderer, 0.0, 1.0) == background,
            "hidden diagrams leave no ink"
        );
        let headers = draw(&mut renderer, 1.0, 0.0);
        assert!(
            headers != background,
            "participants draw before any row is revealed"
        );
        let half = draw(&mut renderer, 1.0, 0.5);
        let full = draw(&mut renderer, 1.0, 1.0);
        assert!(
            half != headers && half != full,
            "reveal progress reaches pixels"
        );
        draw(&mut renderer, 1.0, 0.2);
        assert!(
            draw(&mut renderer, 1.0, 0.5) == half,
            "sampling order cannot change a frame"
        );
    }
}
