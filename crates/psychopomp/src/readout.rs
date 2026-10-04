//! Readouts: a number whose digit wheels follow a continuous value, like an
//! odometer. Unlike a Rolling Number, which plays authored changes on its own
//! schedule, a Readout is a pure function of one channel's sampled value, so a
//! bar's label counts up as the bar grows and a timer counts down with its
//! ring, in video and in native playback alike.
//!
//! Each displayed unit holds still for most of its interval and rolls to the
//! next across a short window around the rounding boundary; a place turns only
//! while every place below it rolls over from 9, and a new leading place rolls
//! in (and opens its room) as the value reaches it.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::math::smoothstep;

/// Share of each displayed unit spent rolling to the next one.
pub const ROLL: f64 = 0.35;
/// The most integer places a readout shows.
const MAX_PLACES: usize = 12;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadoutFormat {
    /// Digits after the decimal point, 0 to 3.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub decimals: u8,
    /// Thousands separators between integer digits.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub grouping: bool,
    #[serde(default, skip_serializing_if = "Rounding::is_default")]
    pub rounding: Rounding,
    /// Static text before the digits, such as `$`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prefix: String,
    /// Static text after the digits, such as `ms` or `%`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unit: String,
}

/// Which displayed value a fractional value shows. A countdown rounds up, so
/// `3` stays on screen until three seconds have fully elapsed.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Rounding {
    #[default]
    Nearest,
    Up,
    Down,
}

impl Rounding {
    fn is_default(&self) -> bool {
        *self == Self::Nearest
    }
}

fn is_zero(value: &u8) -> bool {
    *value == 0
}

/// One sampled column of a readout, left to right.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReadoutCell {
    pub glyph: ReadoutGlyph,
    /// 0 to 1: how much of its room the column takes and how opaque it is.
    pub presence: f32,
    /// A digit's place in display units: 0 is the last digit shown, so it
    /// turns `10^place` times slower than that one. Symbols share their
    /// neighbor's place.
    pub place: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReadoutGlyph {
    /// A wheel: face `n` shows `n mod 10`, offset by `(n - wheel)` rows.
    Digit(f32),
    Symbol(char),
}

impl ReadoutFormat {
    pub fn new(decimals: u8) -> Self {
        Self {
            decimals,
            ..Self::default()
        }
    }

    pub fn grouped(mut self) -> Self {
        self.grouping = true;
        self
    }

    pub fn rounding(mut self, rounding: Rounding) -> Self {
        self.rounding = rounding;
        self
    }

    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = unit.into();
        self
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(self.decimals <= 3, "readouts show at most 3 decimals");
        ensure!(
            self.prefix.chars().count() <= 8 && self.unit.chars().count() <= 12,
            "readout prefix is at most 8 characters and unit at most 12"
        );
        ensure!(
            !self.prefix.contains('\n') && !self.unit.contains('\n'),
            "readout prefix and unit are single-line"
        );
        Ok(())
    }

    /// The text the readout settles on for `value`, as a test and a label
    /// would print it.
    pub fn text(&self, value: f32) -> String {
        let cells = self.cells(value);
        let digits = cells
            .iter()
            .filter(|cell| cell.presence > 0.5)
            .map(|cell| match cell.glyph {
                ReadoutGlyph::Digit(wheel) => {
                    char::from(b'0' + (wheel.round() as i64).rem_euclid(10) as u8)
                }
                ReadoutGlyph::Symbol(symbol) => symbol,
            })
            .collect::<String>();
        format!("{}{digits}{}", self.prefix, self.unit)
    }

    /// The columns showing `value`: an optional sign, integer places with
    /// grouping separators, then the decimal point and fraction places.
    pub fn cells(&self, value: f32) -> Vec<ReadoutCell> {
        let scale = 10_f64.powi(i32::from(self.decimals));
        let scaled = f64::from(value) * scale;
        let shift = match self.rounding {
            Rounding::Nearest => 0.0,
            Rounding::Up => 0.5 - ROLL * 0.5,
            Rounding::Down => -0.5 + ROLL * 0.5,
        };
        let wheel = odometer(scaled.abs() + shift).max(0.0);
        let whole = wheel.floor();
        let fraction = wheel - whole;
        let decimals = usize::from(self.decimals);
        // Places that exist once the wheel has rolled into them, at least
        // the ones place and every fraction place.
        let needed = (whole + 1.0).log10().ceil().max(1.0) as usize + 1;
        let places = needed.clamp(decimals + 1, decimals + MAX_PLACES);
        let place_wheel = |place: usize| {
            let unit = 10_f64.powi(place as i32);
            let below = whole.rem_euclid(unit);
            let carry = if below >= unit - 1.0 { fraction } else { 0.0 };
            (whole / unit).floor() + carry
        };
        let mut cells = Vec::with_capacity(places + 4);
        let negative = scaled < 0.0;
        if negative {
            let presence = smoothstep((wheel / 0.5) as f32);
            cells.push(ReadoutCell {
                glyph: ReadoutGlyph::Symbol('−'),
                presence,
                place: places as u8,
            });
        }
        for place in (0..places).rev() {
            let turns = place_wheel(place);
            // Leading integer places roll in as the value reaches them.
            let presence = if place > decimals {
                turns.clamp(0.0, 1.0) as f32
            } else {
                1.0
            };
            if presence <= 0.0 {
                continue;
            }
            cells.push(ReadoutCell {
                glyph: ReadoutGlyph::Digit(turns.rem_euclid(1000.0) as f32),
                presence,
                place: place as u8,
            });
            let integer_place = place.checked_sub(decimals);
            if let Some(integer_place) = integer_place
                && self.grouping
                && integer_place > 0
                && integer_place % 3 == 0
            {
                cells.push(ReadoutCell {
                    glyph: ReadoutGlyph::Symbol(','),
                    presence,
                    place: place as u8,
                });
            }
            if decimals > 0 && place == decimals {
                cells.push(ReadoutCell {
                    glyph: ReadoutGlyph::Symbol('.'),
                    presence: 1.0,
                    place: place as u8,
                });
            }
        }
        cells
    }
}

/// How fast place `place` turns, in faces per second, while the value
/// moves at `velocity` units per second: the speed that smears a wheel.
pub fn wheel_rate(format: &ReadoutFormat, place: u8, velocity: f32) -> f32 {
    velocity.abs() * 10_f32.powi(i32::from(format.decimals) - i32::from(place))
}

/// The odometer position for a continuous `value` in display units: it rests
/// on `round(value)` and rolls to the next unit across a [`ROLL`]-wide window
/// centered on the rounding boundary, so settled values read crisply.
pub fn odometer(value: f64) -> f64 {
    let shifted = value - 0.5 + ROLL * 0.5;
    let base = shifted.floor();
    base + f64::from(smoothstep(((shifted - base) / ROLL) as f32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_odometer_rests_on_rounded_values_and_rolls_at_the_boundary() {
        assert_eq!(odometer(3.0), 3.0);
        assert_eq!(odometer(3.3), 3.0);
        assert_eq!(odometer(3.7), 4.0);
        assert_eq!(odometer(0.0), 0.0);
        let mid = odometer(3.5);
        assert!((mid - 3.5).abs() < 1e-6, "half way at the boundary: {mid}");
        // Monotone through the roll.
        let samples = (0..200).map(|i| odometer(3.0 + i as f64 * 0.005));
        let values = samples.collect::<Vec<_>>();
        assert!(values.windows(2).all(|pair| pair[1] >= pair[0]));
    }

    #[test]
    fn settled_values_print_like_formatted_numbers() {
        let format = ReadoutFormat::new(0).grouped().unit("ms");
        assert_eq!(format.text(1383.0), "1,383ms");
        assert_eq!(format.text(0.0), "0ms");
        assert_eq!(format.text(999.2), "999ms");
        let fraction = ReadoutFormat::new(1).unit("s");
        assert_eq!(fraction.text(0.27), "0.3s");
        assert_eq!(fraction.text(12.04), "12.0s");
        assert_eq!(ReadoutFormat::new(0).text(-12.0), "−12");
    }

    #[test]
    fn countdowns_round_up_and_floors_round_down() {
        // Rounding up rolls across the start of each unit, so a countdown
        // lands on 2 exactly as it reaches 2.0.
        let up = ReadoutFormat::new(0).rounding(Rounding::Up);
        assert_eq!(up.text(2.5), "3");
        assert_eq!(up.text(3.0), "3");
        assert_eq!(up.text(2.0), "2");
        assert_eq!(up.text(0.0), "0");
        let down = ReadoutFormat::new(0).rounding(Rounding::Down);
        assert_eq!(down.text(2.5), "2");
        assert_eq!(down.text(3.0), "3");
    }

    #[test]
    fn higher_places_turn_only_while_the_lower_ones_roll_over() {
        let format = ReadoutFormat::new(0);
        let wheels = |value: f32| {
            format
                .cells(value)
                .iter()
                .map(|cell| match cell.glyph {
                    ReadoutGlyph::Digit(wheel) => wheel,
                    ReadoutGlyph::Symbol(_) => f32::NAN,
                })
                .collect::<Vec<_>>()
        };
        // 1,2 settled: tens wheel at 1 and ones at 12 (shows 2).
        assert_eq!(wheels(12.0), vec![1.0, 12.0]);
        // Mid roll from 12 to 13: the tens wheel stays put.
        let rolling = wheels(12.5);
        assert_eq!(rolling[0], 1.0);
        assert!((rolling[1] - 12.5).abs() < 1e-4);
        // Mid roll from 19 to 20: the tens wheel turns with the ones.
        let carry = wheels(19.5);
        assert!((carry[0] - 1.5).abs() < 1e-4 && (carry[1] - 19.5).abs() < 1e-4);
    }

    #[test]
    fn a_new_leading_place_rolls_in_and_opens_its_room() {
        let format = ReadoutFormat::new(0).grouped();
        let presence = |value: f32| {
            format
                .cells(value)
                .iter()
                .map(|cell| cell.presence)
                .collect::<Vec<_>>()
        };
        assert_eq!(presence(9.0), vec![1.0]);
        let entering = presence(9.5);
        assert_eq!(entering.len(), 2);
        assert!((entering[0] - 0.5).abs() < 1e-4, "{entering:?}");
        assert_eq!(presence(10.0), vec![1.0, 1.0]);
        // A thousands separator shares its digit's presence.
        let cells = format.cells(999.5);
        assert_eq!(cells[1].glyph, ReadoutGlyph::Symbol(','));
        assert!((cells[0].presence - cells[1].presence).abs() < 1e-6);
        // Fraction places and the ones place always show.
        assert_eq!(ReadoutFormat::new(2).text(0.0), "0.00");
    }

    #[test]
    fn readouts_are_pure_functions_of_their_value() {
        let format = ReadoutFormat::new(1).grouped();
        let forward = (0..50)
            .map(|i| format.cells(i as f32 * 37.3))
            .collect::<Vec<_>>();
        let backward = (0..50)
            .rev()
            .map(|i| format.cells(i as f32 * 37.3))
            .collect::<Vec<_>>();
        assert!(forward.iter().eq(backward.iter().rev()));
        assert!(ReadoutFormat::new(4).validate().is_err());
    }

    #[test]
    fn lower_places_turn_ten_times_faster() {
        let format = ReadoutFormat::new(1);
        let places = format
            .cells(12.3)
            .iter()
            .map(|cell| cell.place)
            .collect::<Vec<_>>();
        assert_eq!(places, vec![2, 1, 1, 0], "tens, ones, point, tenths");
        assert_eq!(wheel_rate(&format, 0, 20.0), 200.0);
        assert_eq!(wheel_rate(&format, 2, -20.0), 2.0);
    }
}
