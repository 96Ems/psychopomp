//! The deck's retained actors and motion, retimed to spoken phrases for export.
use std::{fs, path::Path};

use anyhow::{Context, Result, ensure};
use psychopomp::{
    author::{PlanBuilder, SECOND},
    narration::Narration,
    plan::{ReelPlan, ReelSegmentPlan, ReelTransitionStyle, ScenePlan, TrackEventPlan},
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Script {
    clips: Vec<Clip>,
}

#[derive(Deserialize)]
struct Clip {
    id: String,
    text: String,
    beats: Vec<Option<String>>,
}

pub fn build_reel(narration_dir: &Path) -> Result<ReelPlan> {
    let script: Script = serde_json::from_str(include_str!("../narration/script.json"))?;
    let deck = crate::build_deck()?;
    ensure!(
        script.clips.len() == deck.slides.len(),
        "one narration clip per slide"
    );
    let narration = Narration::load(narration_dir)?;
    let mut segments = Vec::new();
    for (index, (slide, script_clip)) in deck.slides.into_iter().zip(script.clips).enumerate() {
        let mut plan = slide.plan;
        ensure!(
            script_clip.id == plan.id,
            "narration and slide IDs must match"
        );
        ensure!(
            !script_clip.text.trim().is_empty(),
            "narration text must not be empty"
        );
        ensure!(
            script_clip.beats.len() == plan.presentation_steps.len(),
            "one phrase per reveal"
        );
        let clip = narration.clip(&script_clip.id)?;
        let duration = clip.duration() + 2 * SECOND;
        let mut audio = PlanBuilder::new(format!("voice-{}", plan.id), duration);
        // One second lets the short dip finish before the next voice begins.
        let spoken = clip.place(&mut audio, SECOND);
        let starts: Vec<_> = script_clip
            .beats
            .iter()
            .map(|phrase| phrase.as_ref().map_or(0, |phrase| spoken.at(phrase)))
            .collect();
        retime(&mut plan, &starts, duration)?;
        plan.media = audio.finish()?.media;
        for media in &mut plan.media {
            let file = media
                .path
                .file_name()
                .context("narration path needs a file name")?;
            media.path = fs::canonicalize(narration_dir.join(file))?;
        }
        plan.validate()?;
        segments.push(ReelSegmentPlan {
            transition_nanos: if index == 0 { 0 } else { 350_000_000 },
            transition_style: ReelTransitionStyle::Dip,
            transition_focus: None,
            transition_wipe: None,
            plan,
        });
    }
    let reel = ReelPlan {
        version: 1,
        id: "opencode-quality-film".into(),
        segments,
    };
    reel.validate()?;
    Ok(reel)
}

fn retime(plan: &mut ScenePlan, starts: &[u64], duration: u64) -> Result<()> {
    ensure!(
        plan.media.is_empty() && plan.state_channels.is_empty(),
        "retiming accepts only this continuous deck"
    );
    ensure!(
        starts.len() == plan.presentation_steps.len() && starts.first() == Some(&0),
        "retiming requires the first beat at zero and one destination per step"
    );
    ensure!(
        starts.windows(2).all(|pair| pair[0] < pair[1]),
        "spoken beats must remain ordered"
    );
    ensure!(
        starts
            .last()
            .is_some_and(|start| start + 2 * SECOND <= duration),
        "the final reveal needs a settling hold"
    );
    let previous: Vec<_> = plan
        .presentation_steps
        .iter()
        .map(|step| step.start_nanos)
        .collect();
    for channel in &mut plan.continuous_channels {
        for event in &mut channel.events {
            let index = previous
                .iter()
                .position(|at| *at == event.at_nanos())
                .context("only authored step-boundary events can be retimed")?;
            // Move the event, not its spring profile. Text identity and kinetics stay intact.
            match event {
                TrackEventPlan::Set { at_nanos, .. }
                | TrackEventPlan::Spring { at_nanos, .. }
                | TrackEventPlan::Ease { at_nanos, .. } => *at_nanos = starts[index],
            }
        }
    }
    for (index, step) in plan.presentation_steps.iter_mut().enumerate() {
        step.start_nanos = starts[index];
        step.hold_nanos = if index == 0 {
            0
        } else {
            (starts[index] + 2 * SECOND).min(starts.get(index + 1).copied().unwrap_or(duration))
        };
    }
    ensure!(
        plan.cues.is_empty(),
        "unexpected authored cues need explicit retiming"
    );
    plan.duration_nanos = duration;
    plan.validate()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_has_one_ordered_clip_and_anchor_per_slide() {
        let script: Script =
            serde_json::from_str(include_str!("../narration/script.json")).unwrap();
        let deck = crate::build_deck().unwrap();
        assert_eq!(script.clips.len(), deck.slides.len());
        for (clip, slide) in script.clips.iter().zip(&deck.slides) {
            assert_eq!(clip.id, slide.plan.id);
            assert_eq!(clip.beats.len(), slide.plan.presentation_steps.len());
            assert_eq!(clip.beats[0], None);
            let normalized: String = clip
                .text
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c.is_whitespace() {
                        c.to_ascii_lowercase()
                    } else {
                        ' '
                    }
                })
                .collect();
            let normalized = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
            for phrase in clip.beats.iter().skip(1).flatten() {
                assert!(
                    normalized.contains(&phrase.to_lowercase()),
                    "{} missing {phrase}",
                    clip.id
                );
            }
        }
    }

    #[test]
    fn retiming_keeps_actors_and_spring_profiles() {
        let mut plan = crate::build_deck().unwrap().slides.remove(3).plan;
        let actors = serde_json::to_string(&plan.actors).unwrap();
        let old_events: Vec<_> = plan
            .continuous_channels
            .iter()
            .flat_map(|channel| &channel.events)
            .cloned()
            .collect();
        retime(
            &mut plan,
            &[0, 8 * SECOND, 15 * SECOND, 23 * SECOND],
            30 * SECOND,
        )
        .unwrap();
        assert_eq!(actors, serde_json::to_string(&plan.actors).unwrap());
        let mut events: Vec<_> = plan
            .continuous_channels
            .iter()
            .flat_map(|channel| &channel.events)
            .cloned()
            .collect();
        for (event, old) in events.iter_mut().zip(&old_events) {
            match event {
                TrackEventPlan::Set { at_nanos, .. }
                | TrackEventPlan::Spring { at_nanos, .. }
                | TrackEventPlan::Ease { at_nanos, .. } => *at_nanos = old.at_nanos(),
            }
        }
        assert_eq!(
            serde_json::to_string(&events).unwrap(),
            serde_json::to_string(&old_events).unwrap()
        );
    }

    #[test]
    fn retiming_rejects_missing_reversed_or_unsettled_beats() {
        let plan = crate::build_deck().unwrap().slides.remove(0).plan;
        assert!(retime(&mut plan.clone(), &[0, SECOND], 10 * SECOND).is_err());
        assert!(retime(&mut plan.clone(), &[0, 4 * SECOND, 3 * SECOND], 10 * SECOND).is_err());
        assert!(retime(&mut plan.clone(), &[0, 4 * SECOND, 9 * SECOND], 10 * SECOND).is_err());
    }
}
