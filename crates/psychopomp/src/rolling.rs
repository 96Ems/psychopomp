//! Rolling Numbers: a value such as `rc.112`, `0/8`, or `1,383` whose digits
//! roll in place when it changes, ported from `@kitlangton/rolling-number`.
//! Each digit place is a wheel that rolls the way the number moved; unchanged
//! digits stay still; new places rise in and old ones fade out while every
//! glyph glides to its new position. The changes are timed in the recipe and
//! compiled into closed-form tracks, so any frame samples in any order.
use std::collections::HashMap;

use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    anchor::{self, AnchorPlan},
    author::{ActorHandle, ContinuousHandle, PlanBuilder},
    caption::{self, CaptionAlign, CaptionSpanPlan},
    math::{dynamics::settle, remap_clamp},
    motion::MotionState,
    tone::Tone,
};

pub const ROLLING_NUMBER_RECIPE: &str = "rolling-number";

const DEFAULT_DURATION_NANOS: u64 = 500_000_000;
/// The library's slot is one line box tall; faces stack at this pitch.
const ROW_EM: f32 = 1.2;
/// Linear alpha fade at the top and bottom of each digit's window (`--rn-edge-fade`).
const EDGE_FADE_EM: f32 = 0.12;
/// Share of the duration a new digit waits, masked, while space opens for it.
const ENTRY_HOLD: f64 = 0.14;
/// Symbols (separators, signs, literals) fade quickly instead of rolling.
const SYMBOL_FADE_SECONDS: f64 = 0.18;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RollingNumberPlan {
    /// `align` chooses which edge of the whole text sits on x; y is its
    /// vertical center.
    pub origin: [f32; 2],
    #[serde(default, skip_serializing_if = "CaptionAlign::is_default")]
    pub align: CaptionAlign,
    pub size: f32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    /// Color role of the value; prefix and suffix spans carry their own.
    #[serde(default, skip_serializing_if = "Tone::is_default")]
    pub tone: Tone,
    /// Static spans before the value. They slide when the value's width changes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prefix: Vec<CaptionSpanPlan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suffix: Vec<CaptionSpanPlan>,
    /// The value shown from time zero.
    pub value: String,
    /// Later values, in strictly increasing time order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rolls: Vec<RollPlan>,
    /// Settling time of every roll, slide, and digit fade.
    #[serde(
        default = "default_duration",
        skip_serializing_if = "is_default_duration"
    )]
    pub duration_nanos: u64,
    #[serde(default, skip_serializing_if = "RollStagger::is_default")]
    pub stagger: RollStagger,
    #[serde(default, skip_serializing_if = "RollDirection::is_default")]
    pub direction: RollDirection,
    /// Strength of the vertical smear on fast wheels (`--rn-blur`); 0 disables it.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub blur: f32,
    /// A rounded surface behind the text, as for a status chip.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub chip: bool,
    /// Places the number can pin to; while it has any, the blended anchor
    /// (plus that anchor's offset) replaces `origin`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<AnchorPlan>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RollPlan {
    pub at_nanos: u64,
    pub value: String,
}

/// The order in which new places cascade in.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RollStagger {
    /// Spread from the digits already on screen.
    #[default]
    Outward,
    /// Sweep from the left edge, including digits that change in place.
    Start,
    /// Sweep from the right edge, including digits that change in place.
    End,
    None,
}

/// Which way wheels turn. Auto follows each number's displayed magnitude.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RollDirection {
    #[default]
    Auto,
    Up,
    Down,
}

impl RollStagger {
    fn is_default(&self) -> bool {
        *self == Self::Outward
    }
}

impl RollDirection {
    fn is_default(&self) -> bool {
        *self == Self::Auto
    }
}

fn default_duration() -> u64 {
    DEFAULT_DURATION_NANOS
}

fn is_default_duration(nanos: &u64) -> bool {
    *nanos == DEFAULT_DURATION_NANOS
}

fn one() -> f32 {
    1.0
}

fn is_one(value: &f32) -> bool {
    *value == 1.0
}

/// One glyph run with a stable identity: a digit place, a separator, a
/// literal segment, or a static prefix/suffix span.
#[derive(Clone, Debug, PartialEq)]
pub struct RollToken {
    /// Identity plus glyph for symbols; a changed symbol is a new column.
    pub key: String,
    /// The role that survives value changes (`digit:0:2`, `group:0:3`, `lit:0`).
    pub identity: String,
    pub text: String,
    pub tone: Tone,
    pub digit: Option<u8>,
    run: Option<usize>,
}

/// A value's tokens and, per numeric run, its displayed magnitude.
#[derive(Clone, Debug, PartialEq)]
pub struct RollModel {
    pub tokens: Vec<RollToken>,
    magnitudes: Vec<(String, String)>,
}

impl RollingNumberPlan {
    pub fn new(origin: [f32; 2], size: f32, value: impl Into<String>) -> Self {
        Self {
            origin,
            align: CaptionAlign::Left,
            size,
            bold: false,
            tone: Tone::Plain,
            prefix: Vec::new(),
            suffix: Vec::new(),
            value: value.into(),
            rolls: Vec::new(),
            duration_nanos: DEFAULT_DURATION_NANOS,
            stagger: RollStagger::Outward,
            direction: RollDirection::Auto,
            blur: 1.0,
            chip: false,
            anchors: Vec::new(),
        }
    }

    pub fn aligned(mut self, align: CaptionAlign) -> Self {
        self.align = align;
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn prefix(mut self, spans: Vec<CaptionSpanPlan>) -> Self {
        self.prefix = spans;
        self
    }

    pub fn suffix(mut self, spans: Vec<CaptionSpanPlan>) -> Self {
        self.suffix = spans;
        self
    }

    pub fn chip(mut self) -> Self {
        self.chip = true;
        self
    }

    /// Pin the number's origin to `anchor`; the first anchor is where it starts.
    pub fn anchor(mut self, anchor: AnchorPlan) -> Self {
        self.anchors.push(anchor);
        self
    }

    /// True when `property` names one of this number's channels.
    pub fn accepts(&self, property: &str) -> bool {
        matches!(property, "opacity" | "x" | "y") || anchor::accepts(property, &self.anchors)
    }

    pub fn duration_nanos(mut self, nanos: u64) -> Self {
        self.duration_nanos = nanos;
        self
    }

    pub fn stagger(mut self, stagger: RollStagger) -> Self {
        self.stagger = stagger;
        self
    }

    pub fn direction(mut self, direction: RollDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn blur(mut self, strength: f32) -> Self {
        self.blur = strength;
        self
    }

    /// Change to `value` at `at_nanos`.
    pub fn roll(mut self, at_nanos: u64, value: impl Into<String>) -> Self {
        self.rolls.push(RollPlan {
            at_nanos,
            value: value.into(),
        });
        self
    }

    /// Pitch between wheel faces and the height of each digit's window.
    pub fn row_height(&self) -> f32 {
        self.size * ROW_EM
    }

    pub fn edge_fade(&self) -> f32 {
        self.size * EDGE_FADE_EM
    }

    pub fn duration_seconds(&self) -> f64 {
        self.duration_nanos as f64 * 1e-9
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.origin.iter().all(|v| v.is_finite()),
            "rolling number origin must be finite"
        );
        ensure!(
            (10.0..=240.0).contains(&self.size),
            "rolling number size must be between 10 and 240"
        );
        ensure!(
            (1_000_000..=10_000_000_000).contains(&self.duration_nanos),
            "rolling number duration must be between 1 ms and 10 s"
        );
        ensure!(
            self.blur.is_finite() && (0.0..=4.0).contains(&self.blur),
            "rolling number blur must be between 0 and 4"
        );
        ensure!(
            self.prefix.len() + self.suffix.len() <= 8,
            "a rolling number has at most eight prefix and suffix spans"
        );
        for span in self.prefix.iter().chain(&self.suffix) {
            ensure!(
                !span.text.is_empty() && !span.text.contains(['\n', '\r']),
                "rolling number spans must be nonempty single-line text"
            );
        }
        for value in std::iter::once(&self.value).chain(self.rolls.iter().map(|r| &r.value)) {
            ensure!(
                !value.is_empty() && !value.contains(['\n', '\r']) && value.chars().count() <= 48,
                "rolling number values must be single-line text of 1 to 48 characters"
            );
        }
        if self
            .rolls
            .windows(2)
            .any(|w| w[1].at_nanos <= w[0].at_nanos)
        {
            bail!("rolling number changes must be in strictly increasing time order");
        }
        anchor::validate("rolling number", &self.anchors)
    }

    /// Tokens for the full text shown with `value`: prefix spans, the value's
    /// numeric runs and literal segments, then suffix spans. A numeric run is
    /// ASCII digits with `,` grouping between digits and an optional `.`
    /// fraction; digit identity is its run and place, so `999` → `1,000`
    /// keeps the ones, tens, and hundreds wheels.
    pub fn model(&self, value: &str) -> RollModel {
        let mut tokens = self
            .prefix
            .iter()
            .enumerate()
            .map(|(index, span)| static_token("prefix", index, span))
            .collect::<Vec<_>>();
        let mut magnitudes = Vec::new();
        let chars = value.chars().collect::<Vec<_>>();
        let digit = |index: usize| chars.get(index).is_some_and(char::is_ascii_digit);
        let mut literal = String::new();
        let mut index = 0;
        while index < chars.len() {
            if !digit(index) {
                literal.push(chars[index]);
                index += 1;
                continue;
            }
            if !literal.is_empty() {
                let segment = tokens
                    .iter()
                    .filter(|t| t.identity.starts_with("lit:"))
                    .count();
                tokens.push(self.symbol(format!("lit:{segment}"), std::mem::take(&mut literal)));
            }
            // Integer digits and grouping separators between them.
            let start = index;
            while index < chars.len()
                && (digit(index) || (chars[index] == ',' && index > start && digit(index + 1)))
            {
                index += 1;
            }
            let integer = &chars[start..index];
            let fraction = if index + 1 < chars.len() && chars[index] == '.' && digit(index + 1) {
                let from = index + 1;
                index = from;
                while digit(index) {
                    index += 1;
                }
                &chars[from..index]
            } else {
                &[][..]
            };
            let run = magnitudes.len();
            let mut place = integer.iter().filter(|c| c.is_ascii_digit()).count();
            for &c in integer {
                if c == ',' {
                    tokens.push(self.symbol(format!("group:{run}:{place}"), c.into()));
                } else {
                    place -= 1;
                    tokens.push(self.digit(format!("digit:{run}:{place}"), c, run));
                }
            }
            if !fraction.is_empty() {
                tokens.push(self.symbol(format!("decimal:{run}"), ".".into()));
            }
            for (offset, &c) in fraction.iter().enumerate() {
                tokens.push(self.digit(format!("digit:{run}:-{}", offset + 1), c, run));
            }
            let whole = integer
                .iter()
                .filter(|c| c.is_ascii_digit())
                .collect::<String>();
            let trimmed = whole.trim_start_matches('0');
            magnitudes.push((
                if trimmed.is_empty() { "0" } else { trimmed }.to_owned(),
                fraction.iter().collect(),
            ));
        }
        if !literal.is_empty() {
            let segment = tokens
                .iter()
                .filter(|t| t.identity.starts_with("lit:"))
                .count();
            tokens.push(self.symbol(format!("lit:{segment}"), literal));
        }
        tokens.extend(
            self.suffix
                .iter()
                .enumerate()
                .map(|(index, span)| static_token("suffix", index, span)),
        );
        RollModel { tokens, magnitudes }
    }

    fn symbol(&self, identity: String, text: String) -> RollToken {
        RollToken {
            key: format!("{identity}:{text}"),
            identity,
            text,
            tone: self.tone,
            digit: None,
            run: None,
        }
    }

    fn digit(&self, identity: String, c: char, run: usize) -> RollToken {
        RollToken {
            key: identity.clone(),
            identity,
            text: c.into(),
            tone: self.tone,
            digit: Some(c as u8 - b'0'),
            run: Some(run),
        }
    }

    /// Compile every change into closed-form tracks. `advance` measures one
    /// token's horizontal advance in pixels; layout is the renderer's, motion
    /// is this module's.
    pub fn compile(&self, mut advance: impl FnMut(&str) -> f32) -> CompiledRoll {
        let mut roll = Compiler {
            plan: self,
            duration: self.duration_seconds(),
            columns: Vec::new(),
            live: HashMap::new(),
            displayed: self.model(&self.value),
            busy: Vec::new(),
        };
        let (geometry, _) = roll.layout(&roll.displayed.tokens.clone(), &mut advance);
        for (token, (x, width)) in roll.displayed.tokens.clone().into_iter().zip(geometry) {
            let column = Column::at_rest(token, x, width, 0.0);
            roll.live
                .insert(column.token(0.0).key.clone(), roll.columns.len());
            roll.columns.push(column);
        }
        for change in &self.rolls {
            roll.change(change.at_nanos as f64 * 1e-9, &change.value, &mut advance);
        }
        CompiledRoll {
            columns: roll.columns,
            busy: roll.busy,
            row: self.row_height(),
        }
    }
}

fn static_token(side: &str, index: usize, span: &CaptionSpanPlan) -> RollToken {
    RollToken {
        key: format!("{side}:{index}:{}", span.text),
        identity: format!("{side}:{index}"),
        text: span.text.clone(),
        tone: span.tone,
        digit: None,
        run: None,
    }
}

/// Every column of a Rolling Number over its whole schedule.
#[derive(Clone, Debug)]
pub struct CompiledRoll {
    columns: Vec<Column>,
    busy: Vec<(f64, f64)>,
    row: f32,
}

/// One column sampled at a time. `x` is its left edge relative to the
/// origin's x, after alignment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RollGlyph<'a> {
    pub token: &'a RollToken,
    pub x: f32,
    pub width: f32,
    pub opacity: f32,
    /// Downward offset of a digit still rising into its window, in pixels.
    pub rise: f32,
    /// Wheel position of a digit: face `n` shows `n mod 10`, offset by
    /// `(n - wheel) · row` from the center. `None` for symbols.
    pub wheel: Option<f32>,
    /// Vertical smear amount, 0 (sharp) to 1, from the wheel's speed.
    pub smear: f32,
}

impl CompiledRoll {
    /// Columns visible at `seconds`, in order of creation.
    pub fn sample(&self, seconds: f64) -> Vec<RollGlyph<'_>> {
        self.columns
            .iter()
            .filter(|column| column.alive(seconds))
            .map(|column| {
                let token = column.token(seconds);
                let opacity = column.opacity.sample(seconds).position.clamp(0.0, 1.0);
                let rise = column.rise.sample(seconds);
                let wheel = token.digit.map(|_| column.wheel.sample(seconds));
                let roll_speed = wheel.map_or(0.0, |w| w.velocity.abs());
                let rise_speed = rise.velocity.abs() / self.row;
                RollGlyph {
                    token,
                    x: column.x.sample(seconds).position,
                    width: column.width(seconds),
                    opacity,
                    rise: rise.position,
                    wheel: wheel.map(|w| w.position),
                    // Fast reels crossfade into a smear (full at 24 rows/s);
                    // entrances smear sooner (full at 6 rows/s).
                    smear: remap_clamp(roll_speed, [4.0, 24.0], [0.0, 1.0]).max(remap_clamp(
                        rise_speed,
                        [1.0, 6.0],
                        [0.0, 1.0],
                    )),
                }
            })
            .collect()
    }

    /// Whether anything may still be moving at `seconds`. Outside these
    /// windows every sample is identical until the next change.
    pub fn moving(&self, seconds: f64) -> bool {
        self.busy
            .iter()
            .any(|&(start, end)| (start..end).contains(&seconds))
    }

    /// The text shown once every change up to `seconds` has settled.
    pub fn settled_text(&self, seconds: f64) -> String {
        let mut glyphs = self
            .sample(seconds)
            .into_iter()
            .filter(|glyph| glyph.opacity > 0.5)
            .collect::<Vec<_>>();
        glyphs.sort_by(|a, b| a.x.total_cmp(&b.x));
        glyphs
            .iter()
            .map(|glyph| match glyph.wheel {
                Some(wheel) => {
                    char::from(b'0' + (wheel.round() as i64).rem_euclid(10) as u8).to_string()
                }
                None => glyph.token.text.clone(),
            })
            .collect()
    }
}

/// One scalar as a list of settling moves; each takes over from the state of
/// the one before it at its start, so a redirect keeps position and velocity.
#[derive(Clone, Debug)]
struct Track {
    initial: f32,
    moves: Vec<Move>,
}

#[derive(Clone, Copy, Debug)]
struct Move {
    at: f64,
    /// Held at `from`, still, for this long before settling begins.
    delay: f64,
    duration: f64,
    from: f32,
    target: f32,
    velocity: f32,
}

impl Track {
    fn new(initial: f32) -> Self {
        Self {
            initial,
            moves: Vec::new(),
        }
    }

    fn sample(&self, seconds: f64) -> MotionState {
        let Some(m) = self.moves.iter().rev().find(|m| m.at <= seconds) else {
            return MotionState::at(self.initial);
        };
        let local = seconds - m.at - m.delay;
        if local <= 0.0 {
            return MotionState::at(m.from);
        }
        let (position, velocity) = settle(
            m.from,
            m.target,
            m.velocity,
            m.duration as f32,
            local as f32,
        );
        MotionState { position, velocity }
    }

    fn set(&mut self, at: f64, value: f32) {
        self.moves.push(Move {
            at,
            delay: 0.0,
            duration: 0.0,
            from: value,
            target: value,
            velocity: 0.0,
        });
    }

    /// Settle toward `target` from the current state; returns when it rests.
    fn to(&mut self, at: f64, target: f32, duration: f64, delay: f64) -> f64 {
        let state = self.sample(at);
        self.moves.push(Move {
            at,
            delay,
            duration,
            from: state.position,
            target,
            velocity: state.velocity,
        });
        at + delay + duration
    }
}

#[derive(Clone, Debug)]
struct Column {
    /// Token and advance width from each time they were assigned.
    tokens: Vec<(f64, RollToken, f32)>,
    born: f64,
    /// When an exit's fade reaches zero; cleared by re-entry.
    gone: Option<f64>,
    exiting: bool,
    x: Track,
    wheel: Track,
    opacity: Track,
    rise: Track,
}

impl Column {
    fn at_rest(token: RollToken, x: f32, width: f32, at: f64) -> Self {
        Self {
            wheel: Track::new(token.digit.map_or(0.0, f32::from)),
            tokens: vec![(at, token, width)],
            born: at,
            gone: None,
            exiting: false,
            x: Track::new(x),
            opacity: Track::new(1.0),
            rise: Track::new(0.0),
        }
    }

    fn alive(&self, seconds: f64) -> bool {
        seconds >= self.born && self.gone.is_none_or(|gone| seconds < gone)
    }

    fn entry(&self, seconds: f64) -> &(f64, RollToken, f32) {
        self.tokens
            .iter()
            .rev()
            .find(|(at, ..)| *at <= seconds)
            .unwrap_or(&self.tokens[0])
    }

    fn token(&self, seconds: f64) -> &RollToken {
        &self.entry(seconds).1
    }

    fn width(&self, seconds: f64) -> f32 {
        self.entry(seconds).2
    }
}

struct Compiler<'a> {
    plan: &'a RollingNumberPlan,
    duration: f64,
    columns: Vec<Column>,
    /// Key → column index for columns that have not finished exiting.
    live: HashMap<String, usize>,
    displayed: RollModel,
    busy: Vec<(f64, f64)>,
}

impl Compiler<'_> {
    /// Left edges and widths relative to the origin, after alignment.
    fn layout(
        &self,
        tokens: &[RollToken],
        advance: &mut impl FnMut(&str) -> f32,
    ) -> (Vec<(f32, f32)>, f32) {
        let widths = tokens.iter().map(|t| advance(&t.text)).collect::<Vec<_>>();
        let total: f32 = widths.iter().sum();
        let mut x = match self.plan.align {
            CaptionAlign::Left => 0.0,
            CaptionAlign::Center => -total * 0.5,
            CaptionAlign::Right => -total,
        };
        let geometry = widths
            .into_iter()
            .map(|width| {
                let left = x;
                x += width;
                (left, width)
            })
            .collect();
        (geometry, total)
    }

    /// The library's commit: retarget every column from its sampled state.
    fn change(&mut self, at: f64, value: &str, advance: &mut impl FnMut(&str) -> f32) {
        let target = self.plan.model(value);
        if target == self.displayed {
            return;
        }
        let duration = self.duration;
        self.live
            .retain(|_, index| self.columns[*index].gone.is_none_or(|gone| gone > at));
        let (geometry, _) = self.layout(&target.tokens, advance);
        let positions = target
            .tokens
            .iter()
            .zip(&geometry)
            .map(|(token, &g)| (token.key.clone(), g))
            .collect::<HashMap<_, _>>();
        let previous = self
            .live
            .iter()
            .map(|(key, &index)| {
                let column = &self.columns[index];
                (key.clone(), (column.x.sample(at), column.width(at)))
            })
            .collect::<HashMap<_, _>>();
        let previous_x = previous
            .iter()
            .map(|(key, (x, width))| (key.clone(), (x.position, *width)))
            .collect::<HashMap<_, _>>();
        let target_keys = target
            .tokens
            .iter()
            .map(|t| t.key.clone())
            .collect::<Vec<_>>();
        let starts = collapse_positions(&target_keys, &previous_x);
        // Ties keep creation order, so compilation is deterministic.
        let mut old_order = self.live.iter().collect::<Vec<_>>();
        old_order.sort_by(|a, b| {
            previous_x[a.0]
                .0
                .total_cmp(&previous_x[b.0].0)
                .then(a.1.cmp(b.1))
        });
        let old_order = old_order
            .into_iter()
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        let exits = collapse_positions(&old_order, &positions);
        let symbols = |model: &RollModel| {
            model
                .tokens
                .iter()
                .filter(|t| t.digit.is_none())
                .map(|t| (t.identity.clone(), t.key.clone()))
                .collect::<HashMap<_, _>>()
        };
        let old_symbols = symbols(&self.displayed);
        let new_symbols = symbols(&target);
        let trends = (0..target.magnitudes.len())
            .map(|run| match self.plan.direction {
                RollDirection::Up => 1,
                RollDirection::Down => -1,
                RollDirection::Auto => self
                    .displayed
                    .magnitudes
                    .get(run)
                    .map_or(0, |old| trend(old, &target.magnitudes[run])),
            })
            .collect::<Vec<_>>();
        // New places cascade outward from the digits already on screen within
        // a third of the duration, so the change reads as one update rather
        // than typing. "start"/"end" also sweep digits that change in place.
        let sweep = matches!(self.plan.stagger, RollStagger::Start | RollStagger::End);
        let moving = target
            .tokens
            .iter()
            .map(|token| match self.live.get(&token.key) {
                None => true,
                Some(&index) => {
                    let column = &self.columns[index];
                    column.exiting
                        || (sweep && token.digit.is_some() && column.token(at).text != token.text)
                }
            })
            .collect::<Vec<_>>();
        let retained = target
            .tokens
            .iter()
            .zip(&moving)
            .map(|(token, moving)| token.digit.is_some() && !moving)
            .collect::<Vec<_>>();
        let ranks = entry_ranks(&retained, self.plan.stagger);
        let span = ranks
            .iter()
            .zip(&moving)
            .map(|(rank, moving)| if *moving { rank.saturating_sub(1) } else { 0 })
            .max()
            .unwrap_or(0);
        let step = (duration * 0.045).min(duration * 0.3 / span.max(1) as f64);
        let mut settled = at;
        for (index, token) in target.tokens.iter().enumerate() {
            let (x, width) = geometry[index];
            let delay = ranks[index].saturating_sub(1) as f64 * step;
            let replacement = old_symbols
                .get(&token.identity)
                .filter(|old| **old != token.key)
                .and_then(|old| previous.get(old));
            let existing = self.live.get(&token.key).copied();
            let fresh = existing.is_none();
            let column_index = existing.unwrap_or_else(|| {
                let start = replacement
                    .map(|(x, _)| x.position)
                    .or_else(|| starts.get(&token.key).copied())
                    .unwrap_or(x);
                let mut column = Column::at_rest(token.clone(), start, width, at);
                column.opacity.set(at, 0.0);
                self.live.insert(token.key.clone(), self.columns.len());
                self.columns.push(column);
                self.columns.len() - 1
            });
            let column = &mut self.columns[column_index];
            let old = column.token(at).clone();
            let old_width = column.width(at);
            let reentered = column.exiting;
            column.exiting = false;
            column.gone = None;
            settled = settled.max(column.x.to(at, x, duration, 0.0));
            if fresh || reentered {
                let fade = if token.digit.is_some() {
                    duration
                } else {
                    duration.min(SYMBOL_FADE_SECONDS)
                };
                // A new grouping separator shares its digit's masked hold, so
                // a comma does not appear before the digit beside it.
                let hold = if token.identity.starts_with("group:") && replacement.is_none() {
                    duration * ENTRY_HOLD
                } else {
                    0.0
                };
                let wait = if fresh { delay + hold } else { 0.0 };
                settled = settled.max(column.opacity.to(at, 1.0, fade, wait));
            }
            if fresh {
                // Already at rest on its own face.
            } else if let (Some(digit), Some(_)) = (token.digit, old.digit)
                && old.text != token.text
            {
                let run = token.run.expect("digits belong to a run");
                let current = column.wheel.sample(at).position;
                let goal = roll_target(current, digit, trends[run]);
                let wait = if sweep { delay } else { 0.0 };
                settled = settled.max(column.wheel.to(at, goal, duration, wait));
            }
            if old != *token || old_width != width {
                column.tokens.push((at, token.clone(), width));
            }
            if fresh && token.digit.is_some() {
                // Rise from just below the window after space starts opening.
                let row = self.plan.row_height();
                column.rise.set(at, row);
                settled = settled.max(column.rise.to(
                    at,
                    0.0,
                    duration * (1.0 - ENTRY_HOLD),
                    delay + duration * ENTRY_HOLD,
                ));
            }
        }
        for key in &old_order {
            if positions.contains_key(key) {
                continue;
            }
            let index = self.live[key];
            let column = &mut self.columns[index];
            let replacement = new_symbols
                .get(&column.token(at).identity)
                .and_then(|key| positions.get(key));
            let current = previous[key].0.position;
            let goal = replacement
                .map(|g| g.0)
                .or_else(|| exits.get(key).copied())
                .unwrap_or(current);
            settled = settled.max(column.x.to(at, goal, duration, 0.0));
            if column.exiting {
                continue;
            }
            column.exiting = true;
            let fade = if column.token(at).digit.is_some() {
                duration * 0.65
            } else {
                duration.min(SYMBOL_FADE_SECONDS)
            };
            let gone = column.opacity.to(at, 0.0, fade, 0.0);
            column.gone = Some(gone);
            settled = settled.max(gone);
        }
        self.busy.push((at, settled));
        self.displayed = target;
    }
}

/// Compare displayed magnitudes as strings, without parsing or precision loss.
fn trend(old: &(String, String), new: &(String, String)) -> i8 {
    use std::cmp::Ordering;
    let integer = new
        .0
        .len()
        .cmp(&old.0.len())
        .then_with(|| new.0.cmp(&old.0));
    let order = integer.then_with(|| {
        let length = old.1.len().max(new.1.len());
        let pad = |s: &str| format!("{s:0<length$}");
        pad(&new.1).cmp(&pad(&old.1))
    });
    match order {
        Ordering::Greater => 1,
        Ordering::Less => -1,
        Ordering::Equal => 0,
    }
}

/// The nearest wheel position showing `digit`, honoring the trend: up only
/// ever advances, down only ever retreats, and no trend takes the shortest way.
pub fn roll_target(position: f32, digit: u8, trend: i8) -> f32 {
    let mut target = (position / 10.0).floor() * 10.0 + f32::from(digit);
    if trend > 0 && target < position - 0.001 {
        target += 10.0;
    } else if trend < 0 && target > position + 0.001 {
        target -= 10.0;
    } else if trend == 0 {
        target += ((position - target) / 10.0).round() * 10.0;
    }
    target
}

/// Stagger order for new tokens: 0 for retained tokens, then 1, 2, ...
fn entry_ranks(retained: &[bool], stagger: RollStagger) -> Vec<usize> {
    if stagger != RollStagger::Outward {
        let mut order = (0..retained.len())
            .filter(|&i| !retained[i])
            .collect::<Vec<_>>();
        if stagger == RollStagger::End {
            order.reverse();
        }
        let mut ranks = retained
            .iter()
            .map(|&kept| usize::from(!kept))
            .collect::<Vec<_>>();
        if stagger != RollStagger::None {
            for (rank, index) in order.into_iter().enumerate() {
                ranks[index] = rank + 1;
            }
        }
        return ranks;
    }
    let mut ranks = (0..retained.len())
        .map(|i| if retained[i] { 0 } else { i + 1 })
        .collect::<Vec<_>>();
    if !retained.contains(&true) {
        return ranks;
    }
    let mut last = None;
    for index in 0..retained.len() {
        if retained[index] {
            last = Some(index);
        } else {
            ranks[index] = last.map_or(usize::MAX, |last| index - last);
        }
    }
    let mut last = None;
    for index in (0..retained.len()).rev() {
        if retained[index] {
            last = Some(index);
        } else if let Some(last) = last {
            ranks[index] = ranks[index].min(last - index);
        }
    }
    ranks
}

/// Missing keys share the next present key's left edge; trailing ones share
/// the last present key's right edge.
fn collapse_positions(
    keys: &[String],
    positions: &HashMap<String, (f32, f32)>,
) -> HashMap<String, f32> {
    let mut result = HashMap::new();
    let mut pending = Vec::new();
    let mut end = 0.0;
    for key in keys {
        let Some(&(x, width)) = positions.get(key) else {
            pending.push(key.clone());
            continue;
        };
        for missing in pending.drain(..) {
            result.insert(missing, x);
        }
        end = x + width;
    }
    for missing in pending {
        result.insert(missing, end);
    }
    result
}

/// Authoring handle for one Rolling Number actor. Changes may be added after
/// declaration, as narration phrases are found; the current schedule is stored
/// inside the [`PlanBuilder`].
#[derive(Clone, Debug)]
pub struct RollingNumberActor {
    actor: ActorHandle,
    anchors: Vec<String>,
}

impl RollingNumberActor {
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: RollingNumberPlan,
    ) -> Result<Self> {
        plan.validate()?;
        let actor = scene.actor(id, ROLLING_NUMBER_RECIPE, &plan)?;
        let anchors = anchor::ids(&plan.anchors);
        Ok(Self { actor, anchors })
    }

    pub fn actor(&self) -> &ActorHandle {
        &self.actor
    }

    pub fn id(&self) -> &str {
        self.actor.id()
    }

    /// Roll to `value` at `at_nanos`, after every earlier change.
    pub fn roll(
        &mut self,
        scene: &mut PlanBuilder,
        at_nanos: u64,
        value: impl Into<String>,
    ) -> Result<()> {
        self.roll_at(scene, at_nanos, value).map(|_| ())
    }

    pub(crate) fn roll_at(
        &self,
        scene: &mut PlanBuilder,
        at_nanos: u64,
        value: impl Into<String>,
    ) -> Result<u64> {
        let current: RollingNumberPlan = scene.actor_data(&self.actor)?;
        let duration = current.duration_nanos;
        let next = current.roll(at_nanos, value);
        next.validate()?;
        scene.replace_actor_data(&self.actor, &next)?;
        Ok(at_nanos + duration)
    }

    pub fn channel(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        scene.channel(&self.actor, property, initial)
    }

    /// Fade and rise in, like a caption. A number with a `show` starts hidden.
    pub fn show(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        caption::show(scene, &self.actor, at_nanos);
    }

    /// Fade out in place.
    pub fn hide(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        caption::hide(scene, &self.actor, at_nanos);
    }

    /// Glide to the anchor `to`, carrying velocity through interruptions.
    pub fn move_to(&mut self, scene: &mut PlanBuilder, to: &str, at_nanos: u64) -> Result<()> {
        anchor::move_to(scene, &self.actor, &self.anchors, to, at_nanos)
    }
}

#[cfg(test)]
mod tests;
