//! Lip sync for sprite portraits: a talking head drawn as an image sequence
//! (a **Sprite Sheet**) whose frames are expressions crossed with mouth
//! shapes, swapped on the narration clock by cutting a footage playhead.
//!
//! Word timings become **Visemes** on a fixed sprite tick (about nine a
//! second, the choppy cadence of a 90s CD-ROM), closed at pauses and at rest
//! after the last word. Blinks and expressions layer on top, and
//! [`Sprite::perform`] writes the result as `Set` events on the clip's `time`
//! channel: every frame is a pure function of plan time.
use anyhow::{Result, ensure};

use crate::{
    author::{MILLISECOND, PlanBuilder},
    footage::FootageActor,
    math::random::hash,
};

/// A mouth shape. `Rest` is the expression's own mouth (not speaking);
/// `Closed` is lips together mid-sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mouth {
    Rest,
    Closed,
    /// M, B, P: lips pressed.
    Mbp,
    /// A, E: jaw dropped.
    Open,
    /// I, Y and most consonants: teeth showing.
    Wide,
    /// O, U, W: lips rounded.
    Round,
    /// F, V: lip on teeth.
    Fv,
}

impl Mouth {
    /// Slot order within one expression of a Sprite Sheet, before `Blink`.
    pub const ALL: [Mouth; 7] = [
        Mouth::Rest,
        Mouth::Closed,
        Mouth::Mbp,
        Mouth::Open,
        Mouth::Wide,
        Mouth::Round,
        Mouth::Fv,
    ];

    fn slot(self) -> u32 {
        Self::ALL.iter().position(|mouth| *mouth == self).unwrap() as u32
    }

    /// How strongly a shape should win a tick it shares with others: lip
    /// closures and rounded vowels read most clearly at a low frame rate.
    fn priority(self) -> u8 {
        match self {
            Mouth::Mbp => 5,
            Mouth::Round => 4,
            Mouth::Open => 3,
            Mouth::Fv => 2,
            Mouth::Wide => 1,
            Mouth::Closed | Mouth::Rest => 0,
        }
    }
}

/// The viseme for one written letter, or `None` for a letter's silence
/// (punctuation, apostrophes).
pub fn viseme(letter: char) -> Option<Mouth> {
    Some(match letter.to_ascii_lowercase() {
        'm' | 'b' | 'p' => Mouth::Mbp,
        'f' | 'v' => Mouth::Fv,
        'o' | 'u' | 'w' | 'q' => Mouth::Round,
        'a' | 'e' | 'h' => Mouth::Open,
        c if c.is_ascii_alphanumeric() => Mouth::Wide,
        _ => return None,
    })
}

/// A gap between words at least this long closes the mouth.
pub const PAUSE: u64 = 90 * MILLISECOND;

/// The default sprite tick: 110 ms, about nine frames a second.
pub const TICK: u64 = 110 * MILLISECOND;

/// Mouth cues for `words` (`(text, start, end)` in plan nanoseconds) on a
/// `tick` grid anchored at zero: one cue per tick while speaking, `Closed`
/// in pauses of at least [`PAUSE`], and `Rest` after the last word. Letters
/// share their word's span evenly; each tick shows the clearest shape among
/// the letters it covers. Consecutive repeats are dropped.
pub fn visemes<'a>(
    words: impl IntoIterator<Item = (&'a str, u64, u64)>,
    tick: u64,
) -> Vec<(u64, Mouth)> {
    assert!(tick > 0, "the sprite tick must be positive");
    // Every letter's (start, end, mouth), and every pause.
    let mut letters = Vec::new();
    let mut spans: Vec<(u64, u64)> = Vec::new();
    for (word, start, end) in words {
        let shapes: Vec<Mouth> = word.chars().filter_map(viseme).collect();
        if shapes.is_empty() || end <= start {
            continue;
        }
        let step = (end - start) / shapes.len() as u64;
        for (index, mouth) in shapes.iter().enumerate() {
            let from = start + step * index as u64;
            let to = if index + 1 == shapes.len() {
                end
            } else {
                from + step
            };
            letters.push((from, to, *mouth));
        }
        match spans.last_mut() {
            Some(last) if start.saturating_sub(last.1) < PAUSE => last.1 = last.1.max(end),
            _ => spans.push((start, end)),
        }
    }
    let mut cues: Vec<(u64, Mouth)> = Vec::new();
    let mut push = |at: u64, mouth: Mouth| {
        if cues.last().is_none_or(|(_, last)| *last != mouth) {
            cues.push((at, mouth));
        }
    };
    let Some(&(_, last_end)) = spans.last() else {
        return cues;
    };
    let mut cursor = spans[0].0 / tick * tick;
    let mut letter = 0;
    while cursor < last_end {
        let next = cursor + tick;
        let speaking = spans.iter().any(|&(from, to)| from < next && to > cursor);
        while letter < letters.len() && letters[letter].1 <= cursor {
            letter += 1;
        }
        let shape = letters[letter..]
            .iter()
            .take_while(|(from, _, _)| *from < next)
            .filter(|(_, to, _)| *to > cursor)
            .map(|(_, _, mouth)| *mouth)
            .max_by_key(|mouth| mouth.priority());
        push(
            cursor,
            match shape {
                Some(mouth) if speaking => mouth,
                _ => Mouth::Closed,
            },
        );
        cursor = next;
    }
    push(cursor, Mouth::Rest);
    cues
}

/// Blink start times in `[from, until)`: one every 2.4 to 5.4 seconds,
/// deterministic in `salt` so portraits blink out of step.
pub fn blinks(from: u64, until: u64, salt: u32) -> Vec<u64> {
    let mut times = Vec::new();
    let mut at = from + (hash(0, salt) * 2_000.0) as u64 * MILLISECOND;
    let mut index = 1;
    while at < until {
        times.push(at);
        at += (2_400.0 + hash(index, salt) * 3_000.0) as u64 * MILLISECOND;
        index += 1;
    }
    times
}

/// The layout of a Sprite Sheet image sequence: for each expression in
/// order, one frame per [`Mouth`] in [`Mouth::ALL`] order, then a blink.
#[derive(Clone, Debug, PartialEq)]
pub struct SpriteSheet {
    expressions: Vec<String>,
}

impl SpriteSheet {
    /// Frames per expression: every mouth, then the blink.
    pub const SLOTS: u32 = Mouth::ALL.len() as u32 + 1;

    pub fn new<S: Into<String>>(expressions: impl IntoIterator<Item = S>) -> Result<Self> {
        let expressions: Vec<String> = expressions.into_iter().map(Into::into).collect();
        ensure!(
            !expressions.is_empty(),
            "a Sprite Sheet needs an expression"
        );
        for (index, name) in expressions.iter().enumerate() {
            ensure!(
                !expressions[..index].contains(name),
                "expression '{name}' appears twice"
            );
        }
        Ok(Self { expressions })
    }

    pub fn expressions(&self) -> &[String] {
        &self.expressions
    }

    pub fn len(&self) -> u32 {
        self.expressions.len() as u32 * Self::SLOTS
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn expression(&self, name: &str) -> Result<u32> {
        self.expressions
            .iter()
            .position(|expression| expression == name)
            .map(|index| index as u32)
            .ok_or_else(|| anyhow::anyhow!("the Sprite Sheet has no expression '{name}'"))
    }

    /// The frame showing `expression` (an index) with `mouth`.
    pub fn frame(&self, expression: u32, mouth: Mouth) -> u32 {
        expression * Self::SLOTS + mouth.slot()
    }

    /// The frame showing `expression` (an index) blinking at rest.
    pub fn blink(&self, expression: u32) -> u32 {
        expression * Self::SLOTS + Self::SLOTS - 1
    }
}

/// How long a blink holds.
pub const BLINK: u64 = 120 * MILLISECOND;

/// One portrait's whole performance as frame cuts: `expressions` (time,
/// sheet index) change the face, `mouths` the mouth, and `blinks` close the
/// eyes for [`BLINK`] whenever the mouth is resting or closed. Before the
/// first expression the sheet's first is shown. Repeats are dropped.
pub fn frames(
    sheet: &SpriteSheet,
    expressions: &[(u64, u32)],
    mouths: &[(u64, Mouth)],
    blinks: &[u64],
) -> Vec<(u64, u32)> {
    let mut times: Vec<u64> = std::iter::once(0)
        .chain(expressions.iter().map(|(at, _)| *at))
        .chain(mouths.iter().map(|(at, _)| *at))
        .chain(blinks.iter().flat_map(|at| [*at, at + BLINK]))
        .collect();
    times.sort_unstable();
    times.dedup();
    let latest = |cues: &[(u64, u32)], at: u64| {
        cues.iter()
            .take_while(|(time, _)| *time <= at)
            .last()
            .map(|(_, value)| *value)
    };
    let mut cuts: Vec<(u64, u32)> = Vec::new();
    for at in times {
        let expression = latest(expressions, at).unwrap_or(0);
        let mouth = mouths
            .iter()
            .take_while(|(time, _)| *time <= at)
            .last()
            .map_or(Mouth::Rest, |(_, mouth)| *mouth);
        let blinking = blinks.iter().any(|&b| b <= at && at < b + BLINK);
        let frame = if blinking && matches!(mouth, Mouth::Rest | Mouth::Closed) {
            sheet.blink(expression)
        } else {
            sheet.frame(expression, mouth)
        };
        if cuts.last().is_none_or(|(_, last)| *last != frame) {
            cuts.push((at, frame));
        }
    }
    cuts
}

/// A footage overlay playing a Sprite Sheet: an image sequence decoded at
/// `fps` frames a second, whose playhead is cut to one frame at a time.
#[derive(Clone, Debug)]
pub struct Sprite {
    footage: FootageActor,
    sheet: SpriteSheet,
    fps: u32,
}

impl Sprite {
    /// `footage` must play the sheet's image sequence frozen or held, with
    /// its `fps` set to `fps` so frame `n` sits at `n / fps` seconds.
    pub fn new(footage: FootageActor, sheet: SpriteSheet, fps: u32) -> Result<Self> {
        ensure!(fps > 0, "a Sprite needs a positive frame rate");
        ensure!(
            footage.plan().clip.fps == Some(fps),
            "sprite '{}' must decode its sequence at {fps} fps",
            footage.id()
        );
        Ok(Self {
            footage,
            sheet,
            fps,
        })
    }

    pub fn footage(&self) -> &FootageActor {
        &self.footage
    }

    pub fn sheet(&self) -> &SpriteSheet {
        &self.sheet
    }

    /// The playhead time that shows `frame`: the middle of its slot, so
    /// rounding never lands on a neighbour.
    pub fn time_of(&self, frame: u32) -> f32 {
        (frame as f32 + 0.5) / self.fps as f32
    }

    /// Cut to `frame` at `at`.
    pub fn show(&self, scene: &mut PlanBuilder, at: u64, frame: u32) {
        let channel = self.footage.channel(scene, "time");
        scene.set(&channel, at, self.time_of(frame));
    }

    /// Write every cut of a performance (see [`frames`]).
    pub fn perform(&self, scene: &mut PlanBuilder, cuts: &[(u64, u32)]) {
        for &(at, frame) in cuts {
            self.show(scene, at, frame);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::author::SECOND;

    const MS: u64 = MILLISECOND;

    #[test]
    fn letters_map_to_mouths() {
        assert_eq!(viseme('B'), Some(Mouth::Mbp));
        assert_eq!(viseme('v'), Some(Mouth::Fv));
        assert_eq!(viseme('o'), Some(Mouth::Round));
        assert_eq!(viseme('a'), Some(Mouth::Open));
        assert_eq!(viseme('t'), Some(Mouth::Wide));
        assert_eq!(viseme('!'), None);
    }

    #[test]
    fn visemes_sit_on_the_tick_grid_and_rest_after_speech() {
        let cues = visemes([("mama", 50 * MS, 490 * MS)], TICK);
        assert!(cues.iter().all(|(at, _)| at % TICK == 0));
        assert_eq!(cues.first().unwrap().0, 0);
        assert_eq!(cues.last().unwrap(), &(550 * MS, Mouth::Rest));
        assert!(cues.iter().any(|(_, mouth)| *mouth == Mouth::Mbp));
        assert!(cues.iter().any(|(_, mouth)| *mouth == Mouth::Open));
    }

    #[test]
    fn pauses_close_the_mouth_and_short_gaps_do_not() {
        let paused = visemes([("go", 0, 220 * MS), ("on", 660 * MS, 880 * MS)], TICK);
        assert!(paused.contains(&(220 * MS, Mouth::Closed)));
        let joined = visemes([("go", 0, 220 * MS), ("on", 260 * MS, 480 * MS)], TICK);
        assert!(!joined.iter().any(|(_, mouth)| *mouth == Mouth::Closed));
    }

    #[test]
    fn repeats_are_dropped_and_silence_is_empty() {
        let cues = visemes([("ooo", 0, 660 * MS)], TICK);
        assert_eq!(cues, vec![(0, Mouth::Round), (660 * MS, Mouth::Rest)]);
        assert!(visemes(std::iter::empty(), TICK).is_empty());
        assert!(visemes([("...", 0, SECOND)], TICK).is_empty());
    }

    #[test]
    fn blinks_are_deterministic_and_spaced() {
        let a = blinks(0, 60 * SECOND, 7);
        assert_eq!(a, blinks(0, 60 * SECOND, 7));
        assert_ne!(a, blinks(0, 60 * SECOND, 8));
        assert!(a.windows(2).all(|w| w[1] - w[0] >= 2_400 * MS));
        assert!(a.len() >= 10);
    }

    #[test]
    fn sheet_frames_follow_the_slot_layout() {
        let sheet = SpriteSheet::new(["neutral", "smug"]).unwrap();
        assert_eq!(sheet.len(), 16);
        assert_eq!(sheet.frame(0, Mouth::Rest), 0);
        assert_eq!(sheet.frame(1, Mouth::Open), 11);
        assert_eq!(sheet.blink(1), 15);
        assert_eq!(sheet.expression("smug").unwrap(), 1);
        assert!(sheet.expression("sad").is_err());
        assert!(SpriteSheet::new(["a", "a"]).is_err());
    }

    #[test]
    fn performances_layer_expressions_mouths_and_blinks() {
        let sheet = SpriteSheet::new(["neutral", "smug"]).unwrap();
        let cuts = frames(
            &sheet,
            &[(SECOND, 1)],
            &[(SECOND, Mouth::Open), (2 * SECOND, Mouth::Rest)],
            &[500 * MS, 1_100 * MS, 3 * SECOND],
        );
        assert_eq!(
            cuts,
            vec![
                (0, sheet.frame(0, Mouth::Rest)),
                (500 * MS, sheet.blink(0)),
                (620 * MS, sheet.frame(0, Mouth::Rest)),
                // Speaking suppresses the blink at 1.1 s.
                (SECOND, sheet.frame(1, Mouth::Open)),
                (2 * SECOND, sheet.frame(1, Mouth::Rest)),
                (3 * SECOND, sheet.blink(1)),
                (3 * SECOND + BLINK, sheet.frame(1, Mouth::Rest)),
            ]
        );
    }
}
