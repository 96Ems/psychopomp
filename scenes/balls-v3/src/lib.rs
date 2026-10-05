//! Balls with dots, the third cut: "In the beginning, there were balls." A
//! whispered ASMR reply about one ball with dots builds through a chant, a
//! theory, Andy Serkis in a suit of balls, spaghetti, and Will Smith into a
//! climax cut on drum hits, then implodes into silence and one small ball.
//!
//! The film is a reel of Stage segments on one clock (`sound`): each segment
//! takes the sound inside its window and keys its beats to the same words.
use std::path::Path;

use anyhow::{Context, Result};
use psychopomp::plan::{ReelPlan, ReelSegmentPlan, ScenePlan, WipeDirection};

mod after;
mod andy;
mod chaos;
mod genesis;
mod kit;
pub mod sound;
pub mod theory;

use kit::{Window, snap};
use sound::Film;

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

type Segment = Box<dyn Fn(&Film, Window) -> Result<Shot>>;

/// How a segment replaces the one before it, over how many seconds.
#[derive(Clone, Copy)]
#[allow(dead_code)]
enum Enter {
    Cut,
    Match(f64),
    Flash(f64),
    Glitch(f64),
    Cube(f64, WipeDirection),
    Whip(f64, WipeDirection),
    Push(f64, WipeDirection),
    Flip(f64, WipeDirection),
    Iris(f64),
    Ink(f64),
}

impl Enter {
    fn seconds(self) -> f64 {
        match self {
            Self::Cut => 0.0,
            Self::Match(s) | Self::Flash(s) | Self::Glitch(s) | Self::Iris(s) | Self::Ink(s) => s,
            Self::Cube(s, _) | Self::Whip(s, _) | Self::Push(s, _) | Self::Flip(s, _) => s,
        }
    }
}

fn segment(f: impl Fn(&Film, Window) -> Result<Shot> + 'static) -> Segment {
    Box::new(f)
}

pub fn build(root: &Path) -> Result<ReelPlan> {
    use WipeDirection::*;
    let film = Film::load(root)?;
    let theory = film.dive().1;
    let andy = film
        .andy
        .voice_start()
        .saturating_sub(psychopomp::author::seconds(0.06));
    let spaghetti = film
        .spaghetti
        .voice_start()
        .saturating_sub(psychopomp::author::seconds(0.1));
    let mut cuts: Vec<(u64, Segment, Enter)> = vec![
        (0, segment(genesis::segment), Enter::Cut),
        (theory, segment(theory::segment), Enter::Match(0.3)),
        (andy, segment(andy::andy), Enter::Cut),
        (spaghetti, segment(andy::spaghetti), Enter::Cube(0.24, Down)),
    ];
    // The climax: a cut on every drum, and the convergence.
    type Climax = fn(&Film, Window) -> Result<Shot>;
    let shots: [(Climax, Enter); 19] = [
        (chaos::storm, Enter::Glitch(0.12)),
        (chaos::terminal, Enter::Whip(0.1, Left)),
        (chaos::atom, Enter::Push(0.1, Up)),
        (chaos::cubes, Enter::Cube(0.12, Up)),
        (chaos::chart, Enter::Flip(0.1, Left)),
        (chaos::meter, Enter::Cut),
        (chaos::chimp, Enter::Whip(0.08, Right)),
        (chaos::rain, Enter::Cut),
        (chaos::shield, Enter::Glitch(0.08)),
        (chaos::lasagna, Enter::Cut),
        (chaos::fork, Enter::Push(0.07, Down)),
        (chaos::loupe, Enter::Cut),
        (chaos::dots, Enter::Cut),
        (chaos::swarm_shot, Enter::Whip(0.06, Left)),
        (chaos::word, Enter::Cut),
        (chaos::wall, Enter::Cut),
        (chaos::storm, Enter::Cut),
        (chaos::word, Enter::Cut),
        (chaos::converge, Enter::Cut),
    ];
    for (at, (shot, enter)) in film.shots.iter().zip(shots) {
        cuts.push((*at, segment(shot), enter));
    }
    cuts.push((film.cut, segment(after::segment), Enter::Cut));
    assemble(&film, cuts)
}

fn assemble(film: &Film, cuts: Vec<(u64, Segment, Enter)>) -> Result<ReelPlan> {
    let starts = cuts.iter().map(|(at, ..)| snap(*at)).collect::<Vec<_>>();
    let mut segments: Vec<ReelSegmentPlan> = Vec::new();
    let mut exit = None;
    for (index, (_, segment, enter)) in cuts.iter().enumerate() {
        let from = starts[index];
        let (until, overlap) = match cuts.get(index + 1) {
            Some((_, _, next)) => (
                starts[index + 1],
                snap(psychopomp::author::seconds(next.seconds())),
            ),
            None => (snap(film.end), 0),
        };
        let shot = segment(
            film,
            Window {
                from,
                until,
                duration: until + overlap - from,
            },
        )
        .with_context(|| format!("segment {index} at {:.2}s", from as f64 / 1e9))?;
        let length = snap(psychopomp::author::seconds(enter.seconds()));
        let mut plan = shot.plan;
        // A shot the montage repeats needs its own scene ID.
        if segments.iter().any(|other| other.plan.id == plan.id) {
            plan.id = format!("{}-{index}", plan.id);
        }
        segments.push(match *enter {
            Enter::Cut => ReelSegmentPlan::cut(plan),
            Enter::Match(_) => ReelSegmentPlan::matched_round(
                plan,
                length,
                exit.context("a match cut needs the previous shot's exit")?,
                shot.entry.context("a match cut needs this shot's entry")?,
            ),
            Enter::Flash(_) => ReelSegmentPlan::flashed(plan, length),
            Enter::Glitch(_) => ReelSegmentPlan::glitched(plan, length),
            Enter::Cube(_, direction) => ReelSegmentPlan::cubed(plan, length, direction),
            Enter::Whip(_, direction) => ReelSegmentPlan::whipped(plan, length, direction),
            Enter::Push(_, direction) => ReelSegmentPlan::pushed(plan, length, direction),
            Enter::Flip(_, direction) => ReelSegmentPlan::flipped(plan, length, direction),
            Enter::Iris(_) => ReelSegmentPlan::irised(plan, length, true),
            Enter::Ink(_) => ReelSegmentPlan::inked(plan, length),
        });
        exit = shot.exit;
    }
    ReelPlan::new("balls-v3", segments)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use psychopomp::author::seconds;

    use crate::sound::{CUBES_WORDS, Film};

    fn film() -> Film {
        Film::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn the_reel_validates_and_runs_45_to_52_seconds() {
        let reel = super::build(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let length = reel.duration_nanos() as f64 / 1e9;
        assert!((45.0..=52.5).contains(&length), "{length:.2}s");
    }

    #[test]
    fn the_climax_cuts_faster_and_faster_after_the_cubes() {
        let film = film();
        let gaps = film
            .shots
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .collect::<Vec<_>>();
        let montage = &gaps[4..];
        assert!(montage.windows(2).all(|pair| pair[1] <= pair[0]));
        assert!(*montage.last().unwrap() <= seconds(0.13));
        // Each shouted word lands inside the CUBES ARE BALLS shot.
        let shot = film.shots[3];
        for (at, offset) in film.cubes.iter().zip(CUBES_WORDS) {
            assert_eq!(*at, shot + seconds(offset));
            assert!(*at < film.shots[4]);
        }
    }

    #[test]
    fn everything_stops_dead_at_the_implosion_then_silence() {
        let film = film();
        assert!(film.chaos > film.spaghetti.end_of("will smith"));
        for sound in &film.sounds {
            let end = sound.at + (sound.to - sound.from);
            let ringing = sound.at < film.implosion && end > film.implosion + seconds(0.002);
            assert!(
                !ringing || sound.id == "colossal",
                "{} rings past the hit",
                sound.id
            );
        }
        // Nothing but a faint ring plays between the cut and the exhale.
        assert!(film.sounds.iter().all(|sound| {
            let end = sound.at + (sound.to - sound.from);
            end <= film.cut + seconds(0.01) || sound.at >= film.exhale || sound.id == "tinnitus"
        }));
    }
}
