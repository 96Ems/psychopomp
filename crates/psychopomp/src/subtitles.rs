//! Subtitles: burned-in captions driven by narration word timings. Words are
//! chunked into pages of at most `max_lines` balanced lines no wider than
//! `max_width`, breaking at sentence ends and pauses; each page replaces the
//! last with a short fade and rise. The spoken word takes the highlight tone,
//! karaoke style, with a soft pill that glides from word to word.
//!
//! Layout needs glyph widths, so the renderer measures and this module
//! chunks; everything after that is a pure function of time and the word
//! list, so any frame samples alone.
use std::ops::Range;

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    author::{ActorHandle, ContinuousHandle, PlanBuilder},
    math::{easing::smootherstep, lerp, smoothstep},
    narration::Spoken,
    tone::Tone,
};

pub const SUBTITLES_RECIPE: &str = "subtitles";

/// A page appears this long before its first word is spoken.
const LEAD: f64 = 0.15;
/// After its last word, a page with nothing following stays this long.
const HOLD: f64 = 0.7;
/// A silence at least this long ends a page.
const PAUSE: f64 = 0.55;
/// A page is cut at the next word once it has run this long.
const MAX_PAGE: f64 = 6.0;
const FADE_IN: f64 = 0.16;
const FADE_OUT: f64 = 0.12;
/// Faster sequential text handoff during a direct page swap: outgoing text
/// clears quickly without colliding with the incoming sentence.
const SWAP_OUT: f64 = 0.08;
const SWAP_IN: f64 = 0.11;
/// A word becomes spoken over this long from its start.
const SPEAK: f64 = 0.08;
/// The highlight leaves a final word this long after it ends, over `RELEASE`.
const LINGER: f64 = 0.25;
const RELEASE: f64 = 0.15;
/// Vertical travel of an entering and a leaving page, in line heights.
const RISE_IN: f32 = 0.22;
const RISE_OUT: f32 = 0.16;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubtitlesPlan {
    /// Every line's center x, and the vertical center of the bottom line;
    /// pages with more lines grow upward.
    pub origin: [f32; 2],
    pub max_width: f32,
    #[serde(default = "default_size", skip_serializing_if = "is_default_size")]
    pub size: f32,
    #[serde(default = "default_lines", skip_serializing_if = "is_default_lines")]
    pub max_lines: u8,
    /// The spoken word's color.
    #[serde(default = "accent", skip_serializing_if = "is_accent")]
    pub highlight: Tone,
    /// A dark rounded surface behind each page, for legibility over footage.
    #[serde(default = "yes", skip_serializing_if = "is_yes")]
    pub backing: bool,
    pub words: Vec<SubtitleWordPlan>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubtitleWordPlan {
    pub text: String,
    pub start_nanos: u64,
    pub end_nanos: u64,
}

fn default_size() -> f32 {
    40.0
}

fn is_default_size(size: &f32) -> bool {
    *size == default_size()
}

fn default_lines() -> u8 {
    2
}

fn is_default_lines(lines: &u8) -> bool {
    *lines == default_lines()
}

fn accent() -> Tone {
    Tone::Accent
}

fn is_accent(tone: &Tone) -> bool {
    *tone == Tone::Accent
}

fn yes() -> bool {
    true
}

fn is_yes(value: &bool) -> bool {
    *value
}

impl SubtitlesPlan {
    pub fn new(origin: [f32; 2], max_width: f32) -> Self {
        Self {
            origin,
            max_width,
            size: default_size(),
            max_lines: default_lines(),
            highlight: Tone::Accent,
            backing: true,
            words: Vec::new(),
        }
    }

    /// Subtitles for a placed narration clip, on the plan clock.
    pub fn from_spoken(spoken: &Spoken<'_>, origin: [f32; 2], max_width: f32) -> Self {
        Self::new(origin, max_width).spoken(spoken)
    }

    /// Append a placed clip's words, as when several clips play in turn.
    pub fn spoken(mut self, spoken: &Spoken<'_>) -> Self {
        self.words
            .extend(spoken.words().map(|(text, start, end)| SubtitleWordPlan {
                text: text.trim().to_owned(),
                start_nanos: start,
                end_nanos: end,
            }));
        self
    }

    pub fn word(mut self, text: impl Into<String>, start_nanos: u64, end_nanos: u64) -> Self {
        self.words.push(SubtitleWordPlan {
            text: text.into(),
            start_nanos,
            end_nanos,
        });
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn max_lines(mut self, lines: u8) -> Self {
        self.max_lines = lines;
        self
    }

    pub fn highlight(mut self, tone: Tone) -> Self {
        self.highlight = tone;
        self
    }

    pub fn without_backing(mut self) -> Self {
        self.backing = false;
        self
    }

    pub fn line_height(&self) -> f32 {
        (self.size * 1.35).round()
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.origin.iter().all(|v| v.is_finite()),
            "subtitles origin must be finite"
        );
        ensure!(
            (16.0..=96.0).contains(&self.size),
            "subtitles size must be between 16 and 96"
        );
        ensure!(
            self.max_width.is_finite() && self.max_width >= self.size * 6.0,
            "subtitles max width must be at least six times their size"
        );
        ensure!(
            (1..=3).contains(&self.max_lines),
            "subtitles show one to three lines"
        );
        ensure!(
            (1..=4000).contains(&self.words.len()),
            "subtitles need 1 to 4000 words"
        );
        let mut previous_start = 0;
        for word in &self.words {
            ensure!(
                !word.text.trim().is_empty()
                    && word.text.chars().count() <= 40
                    && !word.text.contains(['\n', '\r']),
                "subtitle word '{}' must be 1 to 40 characters on one line",
                word.text
            );
            ensure!(
                word.start_nanos <= word.end_nanos && word.start_nanos >= previous_start,
                "subtitle word '{}' must not end before it starts or start before the previous word",
                word.text
            );
            previous_start = word.start_nanos;
        }
        Ok(())
    }

    /// Chunk the words into timed pages of balanced lines. `advance`
    /// measures a word's width in pixels (and `" "` a space).
    pub fn layout(&self, mut advance: impl FnMut(&str) -> f32) -> SubtitleLayout {
        let space = advance(" ");
        let widths = self
            .words
            .iter()
            .map(|word| advance(&word.text))
            .collect::<Vec<_>>();
        let seconds = |nanos: u64| nanos as f64 * 1e-9;
        let mut ranges = Vec::new();
        let mut start = 0;
        for index in 1..=self.words.len() {
            let page = start..index;
            let ends = index == self.words.len() || {
                let previous = &self.words[index - 1];
                let next = &self.words[index];
                let pause = seconds(next.start_nanos) - seconds(previous.end_nanos) >= PAUSE;
                let sentence = previous.text.ends_with(['.', '!', '?', '…']);
                let long =
                    seconds(next.end_nanos) - seconds(self.words[start].start_nanos) > MAX_PAGE;
                let full = wrap(&widths[start..=index], space, self.max_width).len()
                    > usize::from(self.max_lines);
                pause || sentence || long || full
            };
            if ends {
                ranges.push(page);
                start = index;
            }
        }
        let mut pages = ranges
            .into_iter()
            .map(|words| {
                let lines = balance(&widths[words.clone()], space, self.max_width)
                    .into_iter()
                    .map(|line| {
                        let mut x = 0.0;
                        let placed = line
                            .map(|offset| {
                                let index = words.start + offset;
                                if x > 0.0 {
                                    x += space;
                                }
                                let word = PlacedWord {
                                    index,
                                    x,
                                    width: widths[index],
                                };
                                x += widths[index];
                                word
                            })
                            .collect::<Vec<_>>();
                        SubtitleLine {
                            width: x,
                            words: placed,
                        }
                    })
                    .collect();
                SubtitlePage {
                    show: 0.0,
                    hide: 0.0,
                    swaps: false,
                    words,
                    lines,
                }
            })
            .collect::<Vec<_>>();
        let first = |page: &SubtitlePage| seconds(self.words[page.words.start].start_nanos);
        let last = |page: &SubtitlePage| seconds(self.words[page.words.end - 1].end_nanos);
        for index in 0..pages.len() {
            let begin = first(&pages[index]);
            let floor = if index == 0 {
                0.0
            } else {
                pages[index - 1].hide.min(begin)
            };
            pages[index].show = (begin - LEAD).max(floor);
            let end = last(&pages[index]);
            let next = pages.get(index + 1).map(first);
            // Speech continues: the next page replaces this one directly.
            let swaps = next.is_some_and(|next| next - LEAD - end < HOLD);
            pages[index].swaps = swaps;
            pages[index].hide = match next {
                Some(next) if swaps => (next - LEAD).max(end.min(next)),
                _ => end + HOLD,
            };
        }
        let mut busy = Vec::new();
        for page in &pages {
            busy.push((page.show, page.show + FADE_IN));
            busy.push((page.hide - FADE_OUT, page.hide));
            for index in page.words.clone() {
                let word = &self.words[index];
                let (start, end) = (seconds(word.start_nanos), seconds(word.end_nanos));
                busy.push((start, start + SPEAK));
                let held = index + 1 < page.words.end
                    && seconds(self.words[index + 1].start_nanos) - end < LINGER;
                if !held {
                    busy.push((end + LINGER, end + LINGER + RELEASE));
                }
            }
        }
        busy.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for (start, end) in busy {
            match merged.last_mut() {
                Some(last) if start <= last.1 => last.1 = last.1.max(end),
                _ => merged.push((start, end)),
            }
        }
        SubtitleLayout {
            pages,
            busy: merged,
            starts: self.words.iter().map(|w| seconds(w.start_nanos)).collect(),
            ends: self.words.iter().map(|w| seconds(w.end_nanos)).collect(),
        }
    }
}

/// Greedy lines of word offsets no wider than `limit` (a longer word sits
/// alone on its line).
fn wrap(widths: &[f32], space: f32, limit: f32) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut width = 0.0;
    for (index, &word) in widths.iter().enumerate() {
        let next = if index == start {
            word
        } else {
            width + space + word
        };
        if index > start && next > limit {
            lines.push(start..index);
            start = index;
            width = word;
        } else {
            width = next;
        }
    }
    if start < widths.len() {
        lines.push(start..widths.len());
    }
    lines
}

/// The narrowest greedy wrap with as few lines as `limit` allows, so a page
/// never leaves one orphaned word on its last line.
fn balance(widths: &[f32], space: f32, limit: f32) -> Vec<Range<usize>> {
    let lines = wrap(widths, space, limit).len();
    let widest = widths.iter().copied().fold(0.0, f32::max);
    let (mut low, mut high) = (widest, limit.max(widest));
    for _ in 0..24 {
        let middle = (low + high) * 0.5;
        if wrap(widths, space, middle).len() <= lines {
            high = middle;
        } else {
            low = middle;
        }
    }
    wrap(widths, space, high)
}

/// Every page with its measured lines and its on-screen window.
#[derive(Clone, Debug, PartialEq)]
pub struct SubtitleLayout {
    pub pages: Vec<SubtitlePage>,
    busy: Vec<(f64, f64)>,
    starts: Vec<f64>,
    ends: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SubtitlePage {
    /// Plan seconds the page starts entering and finishes leaving.
    pub show: f64,
    pub hide: f64,
    /// Whether the next page replaces this one directly.
    swaps: bool,
    pub words: Range<usize>,
    pub lines: Vec<SubtitleLine>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SubtitleLine {
    pub width: f32,
    pub words: Vec<PlacedWord>,
}

/// A word's index and its left edge and width within its line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedWord {
    pub index: usize,
    pub x: f32,
    pub width: f32,
}

/// One page as it looks at a sampled time.
#[derive(Clone, Debug, PartialEq)]
pub struct PageFrame<'a> {
    pub page: &'a SubtitlePage,
    pub opacity: f32,
    /// Vertical offset in line heights (positive is down).
    pub rise: f32,
    /// Per placed word, in line order: (upcoming, past, current) weights.
    pub words: Vec<WordInk>,
    /// Highlight pills: line index, left, width, opacity.
    pub pills: Vec<Pill>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WordInk {
    pub line: usize,
    pub word: PlacedWord,
    /// Not yet spoken.
    pub upcoming: f32,
    /// Spoken, and no longer the current word.
    pub past: f32,
    /// The word being spoken, in the highlight tone.
    pub current: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pill {
    pub line: usize,
    pub x: f32,
    pub width: f32,
    pub opacity: f32,
}

/// The backing surface: page size in pixels and opacity, morphing between
/// pages during a direct swap rather than blinking.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Backing {
    pub width: f32,
    pub lines: f32,
    pub opacity: f32,
    pub rise: f32,
}

impl SubtitleLayout {
    /// Whether subtitle pixels may change around `seconds`; elsewhere every
    /// sample is identical until the next transition.
    pub fn moving(&self, seconds: f64) -> bool {
        let index = self.busy.partition_point(|&(start, _)| start <= seconds);
        index > 0 && seconds < self.busy[index - 1].1
    }

    /// The pages visible at `seconds` (at most two, during a swap).
    pub fn sample(&self, seconds: f64) -> Vec<PageFrame<'_>> {
        self.pages
            .iter()
            .enumerate()
            .filter(|&(_, page)| seconds >= page.show && seconds < page.hide)
            .map(|(index, page)| self.page_frame(index, page, seconds))
            .collect()
    }

    fn spoken(&self, index: usize, seconds: f64) -> f32 {
        smoothstep(((seconds - self.starts[index]) / SPEAK) as f32)
    }

    fn page_frame<'a>(
        &'a self,
        page_index: usize,
        page: &'a SubtitlePage,
        seconds: f64,
    ) -> PageFrame<'a> {
        let inherited = page_index > 0
            && self.pages[page_index - 1].swaps
            && (self.pages[page_index - 1].hide - page.show).abs() < 1e-9;
        let fade_in = if inherited { SWAP_IN } else { FADE_IN };
        let fade_out = if page.swaps { SWAP_OUT } else { FADE_OUT };
        let entering = smoothstep(((seconds - page.show) / fade_in) as f32);
        let leaving = smoothstep(((seconds - (page.hide - fade_out)) / fade_out) as f32);
        let current = |index: usize| {
            let spoken = self.spoken(index, seconds);
            let next = if index + 1 < page.words.end {
                self.spoken(index + 1, seconds)
            } else {
                0.0
            };
            // Leaves a word when the next begins, or after it ends when
            // nothing follows (a pause or the page's last word).
            let release = if index + 1 < page.words.end
                && self.starts[index + 1] - self.ends[index] < LINGER
            {
                1.0
            } else {
                1.0 - smoothstep(((seconds - self.ends[index] - LINGER) / RELEASE) as f32)
            };
            (spoken - next).max(0.0) * release
        };
        let mut words = Vec::new();
        for (line_index, line) in page.lines.iter().enumerate() {
            for word in &line.words {
                let spoken = self.spoken(word.index, seconds);
                let current = current(word.index);
                words.push(WordInk {
                    line: line_index,
                    word: *word,
                    upcoming: 1.0 - spoken,
                    past: (spoken - current).max(0.0),
                    current,
                });
            }
        }
        // The pill rides the latest spoken word, gliding from the one before
        // on the same line, or crossfading across a line break.
        let mut pills = Vec::new();
        if let Some(latest) = words.iter().rposition(|ink| ink.upcoming < 1.0) {
            let ink = words[latest];
            let glide = 1.0 - ink.upcoming;
            let raw_glide =
                ((seconds - self.starts[ink.word.index]) / SPEAK).clamp(0.0, 1.0) as f32;
            let previous = latest.checked_sub(1).map(|i| words[i]);
            match previous {
                Some(before) if glide < 1.0 && before.current > 0.0 => {
                    if before.line == ink.line {
                        let k = smootherstep(raw_glide);
                        pills.push(Pill {
                            line: ink.line,
                            x: lerp(before.word.x, ink.word.x, k),
                            width: lerp(before.word.width, ink.word.width, k),
                            opacity: (before.current + ink.current).min(1.0),
                        });
                    } else {
                        for side in [before, ink] {
                            pills.push(Pill {
                                line: side.line,
                                x: side.word.x,
                                width: side.word.width,
                                opacity: side.current,
                            });
                        }
                    }
                }
                _ => pills.push(Pill {
                    line: ink.line,
                    x: ink.word.x,
                    width: ink.word.width,
                    opacity: ink.current,
                }),
            }
        }
        pills.retain(|pill| pill.opacity > 0.001);
        PageFrame {
            page,
            opacity: entering * (1.0 - leaving),
            rise: (1.0 - entering) * RISE_IN - leaving * RISE_OUT,
            words,
            pills,
        }
    }

    /// The backing surface at `seconds`.
    pub fn backing(&self, seconds: f64) -> Option<Backing> {
        let index = self
            .pages
            .iter()
            .position(|page| seconds >= page.show && seconds < page.hide)?;
        let page = &self.pages[index];
        let size = |page: &SubtitlePage| {
            (
                page.lines.iter().map(|line| line.width).fold(0.0, f32::max),
                page.lines.len() as f32,
            )
        };
        let (width, lines) = size(page);
        let inherited = index > 0
            && self.pages[index - 1].swaps
            && (self.pages[index - 1].hide - page.show).abs() < 1e-9;
        let fade_in = if inherited { SWAP_IN } else { FADE_IN };
        let fade_out = if page.swaps { SWAP_OUT } else { FADE_OUT };
        let in_t = ((seconds - page.show) / fade_in).clamp(0.0, 1.0) as f32;
        let out_t = ((seconds - (page.hide - fade_out)) / fade_out).clamp(0.0, 1.0) as f32;
        let entering = smoothstep(in_t);
        let leaving = smoothstep(out_t);
        // A page replacing its predecessor keeps the surface and morphs it
        // on one unbroken minimum-jerk curve without stalling at the midpoint.
        if page.swaps && out_t > 0.0 {
            let next = &self.pages[index + 1];
            let (next_width, next_lines) = size(next);
            let k = smootherstep(out_t * 0.5);
            return Some(Backing {
                width: lerp(width, next_width, k),
                lines: lerp(lines, next_lines, k),
                opacity: 1.0,
                rise: 0.0,
            });
        }
        if inherited && in_t < 1.0 {
            let (before_width, before_lines) = size(&self.pages[index - 1]);
            let k = smootherstep(0.5 + in_t * 0.5);
            return Some(Backing {
                width: lerp(before_width, width, k),
                lines: lerp(before_lines, lines, k),
                opacity: 1.0,
                rise: 0.0,
            });
        }
        Some(Backing {
            width,
            lines,
            opacity: entering * (1.0 - leaving),
            rise: (1.0 - entering) * RISE_IN - leaving * RISE_OUT,
        })
    }
}

/// Authoring handle for one subtitles actor.
#[derive(Clone, Debug)]
pub struct SubtitlesActor {
    actor: ActorHandle,
}

impl SubtitlesActor {
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: &SubtitlesPlan,
    ) -> Result<Self> {
        plan.validate()?;
        let actor = scene.actor(id, SUBTITLES_RECIPE, plan)?;
        Ok(Self { actor })
    }

    pub fn actor(&self) -> &ActorHandle {
        &self.actor
    }

    pub fn id(&self) -> &str {
        self.actor.id()
    }

    pub fn channel(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        scene.channel(&self.actor, property, initial)
    }

    /// Fade every subtitle in from `at_nanos`.
    pub fn show(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        crate::caption::show(scene, &self.actor, at_nanos);
    }

    /// Fade every subtitle out from `at_nanos`, as before a scene change.
    pub fn hide(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        crate::caption::hide(scene, &self.actor, at_nanos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;

    /// "One server, every client. Then a pause and a very long tail..."
    fn plan() -> SubtitlesPlan {
        let mut plan = SubtitlesPlan::new([960.0, 960.0], 400.0);
        let mut at = 0;
        for (word, gap) in [
            ("One", 0),
            ("server,", 0),
            ("every", 0),
            ("client.", 0),
            ("Then", 100),
            ("a", 0),
            ("pause", 0),
            ("and", 900),
            ("a", 0),
            ("very", 0),
            ("long", 0),
            ("tail", 0),
            ("that", 0),
            ("keeps", 0),
            ("on", 0),
            ("going", 0),
            ("and", 0),
            ("going", 0),
            ("on.", 0),
        ] {
            at += gap * MS;
            plan = plan.word(word, at, at + 280 * MS);
            at += 300 * MS;
        }
        plan
    }

    /// Ten pixels per character, monospace.
    fn layout(plan: &SubtitlesPlan) -> SubtitleLayout {
        plan.layout(|text| text.chars().count() as f32 * 10.0)
    }

    fn text(plan: &SubtitlesPlan, page: &SubtitlePage) -> Vec<String> {
        page.lines
            .iter()
            .map(|line| {
                line.words
                    .iter()
                    .map(|word| plan.words[word.index].text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect()
    }

    #[test]
    fn pages_break_at_sentences_pauses_and_width() {
        let plan = plan();
        plan.validate().unwrap();
        let layout = layout(&plan);
        let pages = layout
            .pages
            .iter()
            .map(|page| text(&plan, page))
            .collect::<Vec<_>>();
        assert_eq!(pages[0], vec!["One server, every client."]);
        assert_eq!(pages[1], vec!["Then a pause"], "the 0.9 s pause ends it");
        assert_eq!(pages[2].len(), 2, "the long tail wraps: {:?}", pages[2]);
        for page in &layout.pages {
            assert!(page.lines.len() <= 2);
            for line in &page.lines {
                assert!(line.width <= 400.0, "{line:?}");
            }
        }
        // Every word appears exactly once, in order.
        let order = layout
            .pages
            .iter()
            .flat_map(|page| page.lines.iter().flat_map(|line| &line.words))
            .map(|word| word.index)
            .collect::<Vec<_>>();
        assert_eq!(order, (0..plan.words.len()).collect::<Vec<_>>());
    }

    #[test]
    fn lines_balance_instead_of_orphaning_a_word() {
        // Greedy wrapping leaves one word alone on the second line.
        let widths = [50.0; 6];
        assert_eq!(wrap(&widths, 10.0, 300.0), vec![0..5, 5..6]);
        assert_eq!(balance(&widths, 10.0, 300.0), vec![0..3, 3..6]);
        assert_eq!(wrap(&[500.0, 20.0], 10.0, 100.0), vec![0..1, 1..2]);
    }

    #[test]
    fn pages_swap_directly_while_speech_continues_and_hold_after_a_pause() {
        let plan = plan();
        let layout = layout(&plan);
        let [first, second, third] = [&layout.pages[0], &layout.pages[1], &layout.pages[2]];
        assert_eq!(first.show, 0.0);
        assert!(first.swaps && (first.hide - second.show).abs() < 1e-9);
        // The pause: the second page holds after its last word, then the
        // third appears ahead of its first word.
        assert!(!second.swaps);
        let last = plan.words[second.words.end - 1].end_nanos as f64 * 1e-9;
        assert!((second.hide - (last + HOLD)).abs() < 1e-9);
        let begin = plan.words[third.words.start].start_nanos as f64 * 1e-9;
        assert!((third.show - (begin - LEAD)).abs() < 1e-9);
        // At most two pages at any time, never two fully opaque, and the
        // backing morphs continuously through the midpoint without stalling.
        for step in 0..1200 {
            let frames = layout.sample(step as f64 * 0.01);
            assert!(frames.len() <= 2);
            assert!(frames.iter().filter(|f| f.opacity > 0.999).count() <= 1);
        }
        let w_before = layout.backing(first.hide - 0.01).unwrap().width;
        let w_at = layout.backing(first.hide).unwrap().width;
        let w_after = layout.backing(first.hide + 0.01).unwrap().width;
        assert!(
            (w_at - w_before).abs() > 1.0 && (w_after - w_at).abs() > 1.0,
            "backing morphs through the midpoint without stalling"
        );
    }

    #[test]
    fn the_spoken_word_highlights_at_any_time_in_any_order() {
        let plan = plan();
        let layout = layout(&plan);
        let at = |seconds: f64| {
            let frames = layout.sample(seconds);
            frames
                .iter()
                .flat_map(|frame| frame.words.iter())
                .filter(|ink| ink.current > 0.5)
                .map(|ink| plan.words[ink.word.index].text.clone())
                .collect::<Vec<_>>()
        };
        // Mid-word: "server," (0.30–0.58 s) is current; "One" is past.
        assert_eq!(at(0.45), vec!["server,"]);
        let frames = layout.sample(0.45);
        let one = frames[0].words[0];
        assert!(one.past > 0.99 && one.current < 0.01);
        let every = frames[0].words[2];
        assert!(every.upcoming > 0.99);
        // During the pause the highlight releases.
        let pause_end = plan.words[6].end_nanos as f64 * 1e-9;
        assert!(at(pause_end + 0.6).is_empty());
        // Sampling is order-independent.
        let forward = (0..300)
            .map(|i| layout.sample(i as f64 * 0.02))
            .collect::<Vec<_>>();
        let backward = (0..300)
            .rev()
            .map(|i| layout.sample(i as f64 * 0.02))
            .collect::<Vec<_>>();
        assert!(forward.iter().eq(backward.iter().rev()));
    }

    #[test]
    fn the_pill_glides_between_words_on_a_line() {
        let plan = plan();
        let layout = layout(&plan);
        // "every" starts at 0.6 s; half way through its 80 ms ramp the pill
        // sits between "server," and "every".
        let frames = layout.sample(0.64);
        let pills = &frames[0].pills;
        assert_eq!(pills.len(), 1);
        let words = &frames[0].page.lines[0].words;
        assert!(pills[0].x > words[1].x && pills[0].x < words[2].x);
        assert!(pills[0].opacity > 0.99);
    }

    #[test]
    fn transitions_mark_their_samples_as_moving() {
        let plan = plan();
        let layout = layout(&plan);
        assert!(layout.moving(0.01), "the first page enters");
        assert!(layout.moving(0.62), "a word starts");
        // Between word starts in a run of speech, nothing changes.
        assert!(!layout.moving(0.5));
        assert!(layout.backing(0.1).is_some());
        assert!(layout.backing(30.0).is_none());
    }

    #[test]
    fn invalid_subtitles_are_rejected() {
        let mut plan = plan();
        plan.words[3].start_nanos = 0;
        assert!(plan.validate().is_err());
        let mut empty = SubtitlesPlan::new([0.0, 0.0], 600.0);
        assert!(empty.validate().is_err());
        empty = empty.word("  ", 0, 10);
        assert!(empty.validate().is_err());
    }
}
