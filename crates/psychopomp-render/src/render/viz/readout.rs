//! Readout pixels: each digit is a wheel drawn through a stationary window
//! with linear edge fades, its faces offset by the wheel's fractional
//! position; columns open their room by presence. A wheel turning faster
//! than the eye can read crossfades into a vertical smear, as a Rolling
//! Number's does, instead of strobing between ghosted faces.
use psychopomp::{
    math::remap_clamp,
    readout::{ReadoutFormat, ReadoutGlyph, wheel_rate},
};

use super::{super::chart::Anchor, text_spec};
use crate::render::{HeadlessRenderer, TextDraw, TextFilter, VerticalMask, composite_text};

/// Pitch between wheel faces and the window height, in ems.
const ROW_EM: f32 = 1.2;
/// Linear fade at the top and bottom of each window, in ems.
const EDGE_FADE_EM: f32 = 0.14;
/// Where a line's baseline sits below its vertical center, in ems, so a
/// smaller unit shares the digits' baseline.
const BASELINE_EM: f32 = 0.3;
/// Wheel speeds, in faces per second, from sharp to fully smeared.
const SMEAR_RATES: [f32; 2] = [6.0, 30.0];
/// The smear's deviation, in rows.
const SMEAR_ROWS: f32 = 0.16;

/// A readout to draw at its sampled value.
pub(in crate::render) struct Readout<'a> {
    pub format: &'a ReadoutFormat,
    pub value: f32,
    /// The value's rate of change, units per second, which smears wheels.
    pub velocity: f32,
    pub size: f32,
    /// The unit and prefix size relative to the digits.
    pub affix_scale: f32,
    /// Ink layers for the digits: colors and weights, painted in order.
    pub ink: &'a [([u8; 3], f32)],
    pub affix: [u8; 3],
}

impl HeadlessRenderer {
    /// The readout's width at its value, as `draw_readout` lays it out.
    pub(in crate::render) fn readout_width(&mut self, readout: &Readout) -> f32 {
        let affix = readout.size * readout.affix_scale;
        let prefix = self.text_advance(&readout.format.prefix, affix);
        let unit = self.text_advance(&readout.format.unit, affix);
        let digit = self.text_advance("0", readout.size);
        let cells = readout
            .format
            .cells(readout.value)
            .iter()
            .map(|cell| match cell.glyph {
                ReadoutGlyph::Digit(_) => digit * cell.presence,
                ReadoutGlyph::Symbol(symbol) => {
                    self.text_advance(&symbol.to_string(), readout.size) * cell.presence
                }
            })
            .sum::<f32>();
        prefix + cells + unit
    }

    /// Draw `readout` with its `anchor` edge at `at[0]` and its digits
    /// vertically centered on `at[1]`; returns its width.
    pub(in crate::render) fn draw_readout(
        &mut self,
        pixels: &mut [u8],
        readout: &Readout,
        anchor: Anchor,
        at: [f32; 2],
        opacity: f32,
    ) -> f32 {
        let width = self.readout_width(readout);
        if opacity <= 0.001 {
            return width;
        }
        let canvas = self.viz_canvas();
        let size = readout.size;
        let affix_size = size * readout.affix_scale;
        let affix_y = at[1] + BASELINE_EM * (size - affix_size);
        let mut x = match anchor {
            Anchor::Left => at[0],
            Anchor::Center => at[0] - width * 0.5,
            Anchor::Right => at[0] - width,
        };
        x += self.viz_text(
            pixels,
            &readout.format.prefix,
            affix_size,
            readout.affix,
            Anchor::Left,
            [x, affix_y],
            opacity,
        );
        let row = size * ROW_EM;
        let mask = VerticalMask {
            top: at[1] - row * 0.5,
            bottom: at[1] + row * 0.5,
            fade: size * EDGE_FADE_EM,
        };
        let digit_width = self.text_advance("0", size);
        for cell in readout.format.cells(readout.value) {
            let alpha = opacity * cell.presence;
            match cell.glyph {
                ReadoutGlyph::Digit(wheel) => {
                    let cell_width = digit_width * cell.presence;
                    let rate = wheel_rate(readout.format, cell.place, readout.velocity);
                    let filter = TextFilter::Smear {
                        sigma: row * SMEAR_ROWS,
                        amount: remap_clamp(rate, SMEAR_RATES, [0.0, 1.0]),
                    };
                    if alpha > 0.001 {
                        for &(color, weight) in readout.ink {
                            if weight <= 0.001 {
                                continue;
                            }
                            let spec = text_spec(size, color);
                            for face in (wheel.floor() as i64)..=(wheel.ceil() as i64) {
                                let offset = (face as f32 - wheel) * row;
                                let digit = char::from(b'0' + face.rem_euclid(10) as u8);
                                let sprite = self.plain_text_sprite(&digit.to_string(), spec);
                                let left = x + (cell_width - sprite.advance) * 0.5;
                                composite_text(
                                    pixels,
                                    canvas,
                                    TextDraw {
                                        filter,
                                        opacity: alpha * weight,
                                        mask: Some(mask),
                                        ..TextDraw::new(
                                            sprite,
                                            [left, at[1] + offset - sprite.height as f32 * 0.5],
                                        )
                                    },
                                );
                            }
                        }
                    }
                    x += cell_width;
                }
                ReadoutGlyph::Symbol(symbol) => {
                    let text = symbol.to_string();
                    let advance = self.text_advance(&text, size);
                    for &(color, weight) in readout.ink {
                        self.viz_text(
                            pixels,
                            &text,
                            size,
                            color,
                            Anchor::Left,
                            [x, at[1]],
                            alpha * weight,
                        );
                    }
                    x += advance * cell.presence;
                }
            }
        }
        self.viz_text(
            pixels,
            &readout.format.unit,
            affix_size,
            readout.affix,
            Anchor::Left,
            [x, affix_y],
            opacity,
        );
        width
    }
}
