//! Balls with dots: "In the beginning, there were balls." A whispered ASMR
//! reply about one ball with dots slides into a chant, a theory, Andy Serkis
//! in a suit of balls, spaghetti, Will Smith, and a montage of every voice
//! and effect at once, then a hard cut to silence and one small ball.
//!
//! The film is a reel. The first half is one long, calm take; the second
//! half cuts faster and faster. One clock (`sound`) places every clip and
//! sound effect by phrase; each segment takes the sound inside its window
//! and keys its beats to the same words.
use std::path::Path;

use anyhow::{Context, Result};
use psychopomp::{
    author::seconds,
    plan::{ReelPlan, ReelSegmentPlan, ReelWipePlan, ScenePlan, WipeDirection},
};

mod after;
mod andy;
mod ball;
mod chaos;
mod kit;
pub mod sound;
mod theory;

use kit::{Window, snap};
use sound::{Accent, Film};

/// What a segment hands the reel: its plan, and the rectangles a match cut
/// carries into it and out of it.
pub struct Shot {
    pub plan: ScenePlan,
    pub entry: Option<[f32; 4]>,
    pub exit: Option<[f32; 4]>,
}

impl From<ScenePlan> for Shot {
    fn from(plan: ScenePlan) -> Self {
        Self {
            plan,
            entry: None,
            exit: None,
        }
    }
}

type Segment = fn(&Film, Window) -> Result<Shot>;

/// How a segment replaces the one before it, over how many seconds.
#[derive(Clone, Copy)]
enum Enter {
    Cut,
    Match(f64),
    Flash(f64),
    Glitch(f64),
    Cube(f64, WipeDirection),
    Whip(f64, WipeDirection),
    Push(f64, WipeDirection),
    Iris(f64),
    Ink(f64),
    /// A split screen: the divider rests mid-frame for the hold.
    Wipe(f64, f64),
}

impl Enter {
    fn seconds(self) -> f64 {
        match self {
            Self::Cut => 0.0,
            Self::Match(s) | Self::Flash(s) | Self::Glitch(s) | Self::Iris(s) | Self::Ink(s) => s,
            Self::Wipe(s, _) => s,
            Self::Cube(s, _) | Self::Whip(s, _) | Self::Push(s, _) => s,
        }
    }
}

pub fn build(root: &Path) -> Result<ReelPlan> {
    use WipeDirection::*;
    let mut film = Film::load(root)?;
    let theory = &film.theory;
    let at = |time: u64, lead: f64| time.saturating_sub(seconds(lead));
    let atom = theory.voice_start() + seconds(0.08);
    let dreams = at(theory.at_after("balls", "atoms"), 0.05);
    let thoughts = at(theory.at_after("balls", "dreams"), 0.05);
    let split = at(theory.at_after("represent", "dreams"), 0.12);
    let closed = at(theory.at_after("dots", "dreams"), 0.12);
    let basketballs = at(theory.at("dots represent basketballs"), 0.45);
    let andy = at(film.andy.voice_start(), 0.12);
    let board = at(andy, 0.5);
    let spaghetti = at(film.spaghetti.voice_start(), 0.12);
    let wipe = (closed - split) as f64 / 1e9;
    let mut cuts: Vec<(u64, Segment, Enter)> = vec![
        (0, ball::segment, Enter::Cut),
        (atom, theory::atom, Enter::Match(0.4)),
        (dreams, theory::dreams, Enter::Match(0.35)),
        (thoughts, theory::thoughts, Enter::Glitch(0.2)),
        (split, theory::dots, Enter::Wipe(wipe, wipe - 0.34)),
        (basketballs, theory::basketballs, Enter::Push(0.18, Up)),
        (board, theory::board, Enter::Whip(0.16, Left)),
        (andy, andy::andy, Enter::Flash(0.24)),
        (spaghetti, andy::spaghetti, Enter::Cube(0.3, Up)),
    ];
    let montage: [(Segment, Enter); 13] = [
        (chaos::storm, Enter::Match(0.3)),
        (chaos::terminal, Enter::Whip(0.12, Left)),
        (chaos::atom, Enter::Iris(0.12)),
        (chaos::dissolve, Enter::Cut),
        (chaos::burst, Enter::Glitch(0.1)),
        (chaos::chimp, Enter::Push(0.1, Up)),
        (chaos::chart, Enter::Whip(0.1, Down)),
        (chaos::cubes, Enter::Cube(0.14, Up)),
        (chaos::loupe, Enter::Cut),
        (chaos::swarm_shot, Enter::Whip(0.1, Right)),
        (chaos::future, Enter::Glitch(0.1)),
        (chaos::fork, Enter::Ink(0.1)),
        (chaos::finale, Enter::Cut),
    ];
    for (at, (segment, enter)) in film.montage().into_iter().zip(montage) {
        cuts.push((at, segment, enter));
    }
    cuts.push((film.cut, after::segment, Enter::Cut));
    // Every cut is heard.
    let first = film.chaos;
    let accents = cuts
        .iter()
        .skip(1)
        .enumerate()
        .filter_map(|(index, (at, _, enter))| {
            let accent = if *at > first && *at < film.cut {
                Accent::Montage(index)
            } else {
                match enter {
                    Enter::Match(_) => Accent::Dive,
                    Enter::Whip(..) | Enter::Push(..) | Enter::Cube(..) | Enter::Wipe(..) => {
                        Accent::Whip
                    }
                    Enter::Glitch(_) | Enter::Ink(_) | Enter::Iris(_) => Accent::Glitch,
                    Enter::Flash(_) => Accent::Flash,
                    Enter::Cut => return None,
                }
            };
            Some((*at, accent))
        })
        .collect::<Vec<_>>();
    film.accent(&accents);

    let starts = cuts.iter().map(|(at, ..)| snap(*at)).collect::<Vec<_>>();
    let mut segments: Vec<ReelSegmentPlan> = Vec::new();
    let mut exit = None;
    for (index, (_, segment, enter)) in cuts.iter().enumerate() {
        let from = starts[index];
        let (until, overlap) = match cuts.get(index + 1) {
            Some((_, _, next)) => (starts[index + 1], snap(seconds(next.seconds()))),
            None => (snap(film.end), 0),
        };
        let shot = segment(
            &film,
            Window {
                from,
                until,
                duration: until + overlap - from,
            },
        )
        .with_context(|| format!("segment {index} at {:.2}s", from as f64 / 1e9))?;
        let length = snap(seconds(enter.seconds()));
        let plan = shot.plan;
        segments.push(match *enter {
            Enter::Cut => ReelSegmentPlan::cut(plan),
            Enter::Match(_) => ReelSegmentPlan::matched_round(
                plan,
                length,
                exit.expect("a match cut needs the previous shot's exit"),
                shot.entry.expect("a match cut needs this shot's entry"),
            ),
            Enter::Flash(_) => ReelSegmentPlan::flashed(plan, length),
            Enter::Glitch(_) => ReelSegmentPlan::glitched(plan, length),
            Enter::Cube(_, direction) => ReelSegmentPlan::cubed(plan, length, direction),
            Enter::Whip(_, direction) => ReelSegmentPlan::whipped(plan, length, direction),
            Enter::Push(_, direction) => ReelSegmentPlan::pushed(plan, length, direction),
            Enter::Iris(_) => ReelSegmentPlan::irised(plan, length, true),
            Enter::Ink(_) => ReelSegmentPlan::inked(plan, length),
            Enter::Wipe(_, hold) => ReelSegmentPlan::wiped(
                plan,
                length,
                ReelWipePlan::new(WipeDirection::Left).hold(0.5, snap(seconds(hold))),
            ),
        });
        exit = shot.exit;
    }
    ReelPlan::new("balls-with-dots", segments)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use psychopomp::author::seconds;

    use crate::sound::Film;

    fn film() -> Film {
        Film::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn the_reel_validates_and_runs_under_a_minute() {
        let reel = super::build(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let length = reel.duration_nanos() as f64 / 1e9;
        assert!((44.0..=54.0).contains(&length), "{length:.2}s");
    }

    #[test]
    fn every_chanted_balls_lands_inside_the_chant() {
        let film = film();
        let beats = film.chant.beats();
        assert_eq!(beats.len(), 13);
        assert!(beats.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!(beats[0].0 >= film.chant.start && beats[12].1 <= film.chant.end());
    }

    #[test]
    fn the_climax_starts_after_will_smith_and_stops_dead_into_silence() {
        let film = film();
        assert!(film.chaos > film.spaghetti.end_of("will smith"));
        for sound in &film.sounds {
            let end = sound.at + (sound.to - sound.from);
            if sound.at < film.cut && !sound.script {
                assert!(end <= film.cut, "{} rings past the cut", sound.id);
            }
        }
        // A beat of dead silence after the cut.
        assert!(
            film.sounds
                .iter()
                .all(|sound| sound.at + (sound.to - sound.from) <= film.cut
                    || sound.at >= film.cut + seconds(0.7))
        );
    }
}
