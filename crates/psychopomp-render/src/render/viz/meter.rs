//! Meter pixels: a ring (gauge or countdown) or a bar. The track and ticks
//! draw on with `reveal`; the value arc, its head, the lit ticks, the tone,
//! and the Readout all follow the one sampled `value`; `flash` lifts the ink.
use psychopomp::{
    math::smoothstep,
    meter::{MeterKind, MeterPlan},
    tone::Tone,
};

use super::{
    super::{
        chart::{Anchor, TICK_SIZE, disclosure, dot, solid, stroke},
        theme::mix,
        ui::{Bounds, card::UiCanvas},
    },
    arc,
    readout::Readout,
};
use crate::render::HeadlessRenderer;

const WHITE: [u8; 3] = [255, 255, 255];

impl HeadlessRenderer {
    pub(crate) fn composite_meter(
        &mut self,
        pixels: &mut [u8],
        plan: &MeterPlan,
        sample: impl Fn(&str, f32) -> f32,
        velocity: impl Fn(&str) -> f32,
    ) {
        let opacity = sample("opacity", 1.0).clamp(0.0, 1.0);
        if opacity <= 0.001 {
            return;
        }
        let shift = [sample("x", 0.0), sample("y", 0.0)];
        let pose = MeterPose {
            reveal: sample("reveal", 1.0).clamp(0.0, 1.0),
            value: sample("value", plan.scale.range[0]),
            speed: velocity("value"),
            flash: sample("flash", 0.0).clamp(0.0, 1.0),
            opacity,
            shift,
        };
        match plan.kind {
            MeterKind::Ring => self.meter_ring(pixels, plan, pose),
            MeterKind::Bar => self.meter_bar(pixels, plan, pose),
        }
    }

    /// The value's stroke color and the readout's ink layers.
    fn meter_inks(&self, plan: &MeterPlan, pose: MeterPose) -> ([u8; 3], Vec<([u8; 3], f32)>) {
        let blend = plan.tone_at(pose.value);
        let stroke = mix(
            self.theme.tone(blend.from),
            self.theme.tone(blend.to),
            blend.weight,
        );
        let stroke = mix(stroke, WHITE, pose.flash * 0.45);
        // The number stays in text color until a warning or error takes it.
        let text = self.theme.palette().text;
        let number = |tone: Tone| match tone {
            Tone::Warning | Tone::Error => self.theme.tone(tone),
            _ => text,
        };
        let mut ink = vec![
            (number(blend.from), 1.0 - blend.weight),
            (number(blend.to), blend.weight),
        ];
        if ink[0].0 == ink[1].0 {
            ink = vec![(ink[0].0, 1.0)];
        }
        if pose.flash > 0.001 {
            ink.push((WHITE, pose.flash * 0.6));
        }
        (stroke, ink)
    }

    fn meter_ring(&mut self, pixels: &mut [u8], plan: &MeterPlan, pose: MeterPose) {
        let canvas = self.viz_canvas();
        let palette = self.theme.palette();
        let center = [
            plan.center[0] + pose.shift[0],
            plan.center[1] + pose.shift[1],
        ];
        let radius = plan.size;
        let thickness = plan.stroke();
        let fraction = plan.fraction(pose.value);
        let shown = fraction.min(pose.reveal);
        let (color, ink) = self.meter_inks(plan, pose);
        let alpha = pose.opacity;
        let start = plan.angle(0.0);
        if pose.reveal > 0.0 {
            let track = arc(center, radius, start, plan.angle(pose.reveal));
            stroke(pixels, canvas, &track, thickness, palette.raised, alpha);
        }
        let length = (plan.angle(shown) - start) * radius;
        if length > 0.25 {
            let value = arc(center, radius, start, plan.angle(shown));
            stroke(pixels, canvas, &value, thickness, color, alpha);
        }
        // The head marks the moving end; it grows in from nothing at zero.
        let head = smoothstep(length / thickness);
        if head > 0.001 {
            let at = plan.ring_point(shown, radius);
            let at = [at[0] + pose.shift[0], at[1] + pose.shift[1]];
            dot(
                pixels,
                canvas,
                at,
                thickness * 0.62,
                mix(color, WHITE, 0.4),
                alpha * head,
            );
        }
        let closed = plan.sweep >= 360.0;
        let inner = radius + thickness * 0.5 + 9.0;
        for &tick in &plan.scale.ticks {
            let f = plan.fraction(tick);
            if closed && f >= 1.0 - 1e-4 && plan.scale.ticks.len() > 1 {
                continue;
            }
            let present = alpha * disclosure(f, pose.reveal);
            if present <= 0.001 {
                continue;
            }
            let lit = f <= fraction + 1e-4 && fraction > 0.0;
            let from = plan.ring_point(f, inner);
            let to = plan.ring_point(f, inner + 10.0);
            let line = [
                [from[0] + pose.shift[0], from[1] + pose.shift[1]],
                [to[0] + pose.shift[0], to[1] + pose.shift[1]],
            ];
            if lit {
                stroke(pixels, canvas, &line, 2.2, color, present);
            } else {
                stroke(pixels, canvas, &line, 1.6, palette.muted, present * 0.55);
            }
            if plan.tick_labels {
                let at = plan.ring_point(f, inner + 30.0);
                self.viz_text(
                    pixels,
                    &plan.scale.tick_label(tick),
                    TICK_SIZE,
                    palette.muted,
                    Anchor::Center,
                    [at[0] + pose.shift[0], at[1] + pose.shift[1]],
                    present,
                );
            }
        }
        // The number and label settle in as the track draws.
        let alpha = alpha * smoothstep((pose.reveal - 0.25) / 0.5);
        let size = (radius * 0.46).round();
        let label_size = (radius * 0.13).max(18.0).round();
        let has_label = !plan.label.is_empty();
        let number_y = center[1] - if has_label { radius * 0.06 } else { 0.0 };
        if let Some(format) = &plan.readout {
            self.draw_readout(
                pixels,
                &Readout {
                    format,
                    value: pose.value,
                    velocity: pose.speed,
                    size,
                    affix_scale: 0.5,
                    ink: &ink,
                    affix: palette.muted,
                },
                Anchor::Center,
                [center[0], number_y],
                alpha,
            );
        }
        if has_label {
            self.viz_text(
                pixels,
                &plan.label,
                label_size,
                palette.muted,
                Anchor::Center,
                [center[0], number_y + size * 0.62 + label_size * 0.6],
                alpha,
            );
        }
    }

    fn meter_bar(&mut self, pixels: &mut [u8], plan: &MeterPlan, pose: MeterPose) {
        let canvas = self.viz_canvas();
        let palette = self.theme.palette();
        let thickness = plan.stroke();
        let length = plan.size;
        let left = plan.center[0] - length * 0.5 + pose.shift[0];
        let y = plan.center[1] + pose.shift[1];
        let fraction = plan.fraction(pose.value);
        let (color, ink) = self.meter_inks(plan, pose);
        let alpha = pose.opacity;
        let radius = thickness * 0.5;
        let bar = |width: f32| Bounds {
            origin: [left, y - radius],
            size: [width, thickness],
        };
        if pose.reveal > 0.0 {
            UiCanvas::new(pixels, canvas).fill(
                bar(length * pose.reveal),
                radius,
                solid(palette.raised),
                alpha,
            );
        }
        let filled = length * fraction.min(pose.reveal);
        if filled > 0.25 {
            // Below one stroke width the fill keeps its rounded height and
            // fades in rather than pinching.
            UiCanvas::new(pixels, canvas).fill(
                bar(filled.max(thickness)),
                radius,
                solid(color),
                alpha * (filled / thickness).min(1.0),
            );
        }
        for &tick in &plan.scale.ticks {
            let f = plan.fraction(tick);
            let present = alpha * disclosure(f, pose.reveal);
            if present <= 0.001 {
                continue;
            }
            let x = left + length * f;
            let top = y + radius + 8.0;
            let lit = f <= fraction + 1e-4 && fraction > 0.0;
            let (ink, width, strength) = if lit {
                (color, 2.0, 1.0)
            } else {
                (palette.muted, 1.5, 0.55)
            };
            stroke(
                pixels,
                canvas,
                &[[x, top], [x, top + 8.0]],
                width,
                ink,
                present * strength,
            );
            if plan.tick_labels {
                self.viz_text(
                    pixels,
                    &plan.scale.tick_label(tick),
                    TICK_SIZE,
                    palette.muted,
                    Anchor::Center,
                    [x, top + 8.0 + TICK_SIZE],
                    present,
                );
            }
        }
        let alpha = alpha * smoothstep((pose.reveal - 0.25) / 0.5);
        let size = (thickness * 2.2).max(30.0).round();
        let row = y - radius - size * 0.8;
        if let Some(format) = &plan.readout {
            self.draw_readout(
                pixels,
                &Readout {
                    format,
                    value: pose.value,
                    velocity: pose.speed,
                    size,
                    affix_scale: 0.6,
                    ink: &ink,
                    affix: palette.muted,
                },
                Anchor::Right,
                [left + length, row],
                alpha,
            );
        }
        self.viz_text(
            pixels,
            &plan.label,
            (size * 0.62).round(),
            palette.muted,
            Anchor::Left,
            [left, row + size * 0.1],
            alpha,
        );
    }
}

#[derive(Clone, Copy)]
struct MeterPose {
    reveal: f32,
    value: f32,
    speed: f32,
    flash: f32,
    opacity: f32,
    shift: [f32; 2],
}

#[cfg(test)]
mod gpu_tests {
    use psychopomp::{axis::AxisPlan, meter::MeterPlan};

    use crate::render::{HeadlessRenderer, RenderSpec};

    #[test]
    #[ignore = "requires a headless GPU; the value moves the arc and readout, and sampling is order-free"]
    fn meters_follow_their_value() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "meter-proof".into(),
        }))
        .unwrap();
        let background = renderer.render_title_card("", None, 0.);
        for plan in [
            MeterPlan::countdown([960.0, 540.0], 160.0, 10.0),
            MeterPlan::bar(
                [960.0, 540.0],
                800.0,
                AxisPlan::new([0.0, 100.0]).every(25.0),
            ),
        ] {
            let mut draw = |value: f32| {
                let mut pixels = background.clone();
                renderer.composite_meter(
                    &mut pixels,
                    &plan,
                    |property, default| if property == "value" { value } else { default },
                    |_| 0.0,
                );
                pixels
            };
            let low = draw(2.0);
            let high = draw(8.0);
            assert!(low != high && low != background);
            assert!(draw(2.0) == low, "sampling order cannot change a frame");
        }
    }
}
