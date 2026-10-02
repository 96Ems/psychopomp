//! The PR-explainer template, shared with `psychopomp-config-migration`: a header
//! naming the pull request, status chips, a behavior segment that plays the
//! broken story as a Sequence Diagram and replays the fix in the same slots, and
//! a code segment that animates the change as a Stepped Diff. Every moment is a
//! phrase in the narration, so re-voicing the script re-times the film.
use anyhow::{Context, Result};
use psychopomp::{
    author::{PlanBuilder, seconds},
    caption::{CaptionActor, CaptionAlign, CaptionPlan, CaptionSpanPlan},
    editor::diff::Diff,
    narration::Narration,
    plan::ScenePlan,
    sequence::{SequenceActor, SequencePlan},
    tone::Tone,
};

/// The dip between segments.
pub const TRANSITION: u64 = 700_000_000;
pub const LEFT: f32 = 140.0;
pub const RIGHT: f32 = 1780.0;
pub const HEADER_Y: f32 = 96.0;
pub const FOOTER_Y: f32 = 1004.0;

pub struct Pr {
    pub number: &'static str,
    pub title: &'static str,
    pub slug: &'static str,
}

pub fn span(text: &str, tone: Tone) -> CaptionSpanPlan {
    CaptionSpanPlan::new(text, tone)
}

/// `#50784  keep the real startup error`, top left.
pub fn header(scene: &mut PlanBuilder, pr: &Pr, type_at: Option<u64>) -> Result<()> {
    let plan = CaptionPlan::line(
        [LEFT, HEADER_Y],
        30.0,
        vec![
            span(pr.number, Tone::Accent),
            span("  ", Tone::Plain),
            span(pr.title, Tone::Plain),
        ],
    );
    let mut caption = CaptionActor::declare(scene, "header", &plan)?;
    if let Some(at) = type_at {
        caption.type_in(scene, at, 55.0, 0.6);
    }
    Ok(())
}

/// A status chip, top right: `● before`.
pub fn chip(scene: &mut PlanBuilder, id: &str, dot: Tone, text: &str) -> Result<CaptionActor> {
    let plan = CaptionPlan::line(
        [RIGHT, HEADER_Y],
        22.0,
        vec![span("● ", dot), span(text, Tone::Plain)],
    )
    .aligned(CaptionAlign::Right)
    .chip();
    CaptionActor::declare(scene, id, &plan)
}

pub fn footer(
    scene: &mut PlanBuilder,
    id: &str,
    spans: Vec<CaptionSpanPlan>,
) -> Result<CaptionActor> {
    CaptionActor::declare(scene, id, &CaptionPlan::line([LEFT, FOOTER_Y], 28.0, spans))
}

#[derive(Clone, Copy)]
enum When {
    Before(&'static str),
    After(&'static str),
    /// A phrase in the after clip, after another phrase has been said.
    AfterFollowing(&'static str, &'static str),
}

#[derive(Clone, Copy)]
pub struct At(When, f64);

pub fn before(phrase: &'static str) -> At {
    At(When::Before(phrase), 0.0)
}

pub fn after(phrase: &'static str) -> At {
    At(When::After(phrase), 0.0)
}

impl At {
    pub fn plus(self, seconds: f64) -> Self {
        At(self.0, self.1 + seconds)
    }
}

/// One PR's behavior story.
pub struct Flow {
    pub sequence: SequencePlan,
    /// Rows that only exist in the broken behavior; they fade when the fix replays.
    pub before_only: &'static [&'static str],
    pub reveals: Vec<(&'static str, At)>,
    pub strikes: Vec<(&'static str, At)>,
    /// Participant emphasis on and off.
    pub emphasis: Vec<(&'static str, At, At)>,
    /// Participants that start hidden: when they appear, and their opacity after the fix.
    pub late: Vec<(&'static str, At, f32)>,
    pub footer_before: (Vec<CaptionSpanPlan>, At),
    pub footer_after: (Vec<CaptionSpanPlan>, At),
}

// ---------------------------------------------------------------------------
// Segments
// ---------------------------------------------------------------------------

pub fn behavior(pr: &Pr, narration: &Narration, flow: Flow) -> Result<ScenePlan> {
    let before_clip = narration.clip(&format!("{}-before", pr.slug))?;
    let after_clip = narration.clip(&format!("{}-after", pr.slug))?;
    let lead = seconds(1.0);
    let gap = seconds(1.6);
    let duration = lead + before_clip.duration() + gap + after_clip.duration() + seconds(1.4);
    let mut scene = PlanBuilder::new(format!("{}-behavior", pr.slug), duration);
    let spoken_before = before_clip.place(&mut scene, lead);
    let spoken_after = after_clip.place(&mut scene, spoken_before.end() + gap);
    let switch = spoken_before.end() + seconds(0.4);
    let time = |at: At| -> u64 {
        let base = match at.0 {
            When::Before(phrase) => spoken_before.at(phrase),
            When::After(phrase) => spoken_after.at(phrase),
            When::AfterFollowing(phrase, earlier) => spoken_after.at_after(phrase, earlier),
        };
        (base as i64 + (at.1 * 1e9) as i64).max(0) as u64
    };

    header(&mut scene, pr, Some(seconds(0.25)))?;
    let mut before_chip = chip(&mut scene, "chip-before", Tone::Error, "before")?;
    before_chip.show(&mut scene, seconds(0.5));
    before_chip.hide(&mut scene, switch);
    let mut after_chip = chip(&mut scene, "chip-after", Tone::Success, "after the fix")?;
    after_chip.show(&mut scene, switch + seconds(0.25));

    let mut sequence = SequenceActor::declare(&mut scene, "flow", &flow.sequence)?;
    sequence.animate(&mut scene, "opacity", 0.0, seconds(0.2), 1.0, 0.5);
    sequence.animate(&mut scene, "lifelines", 0.0, seconds(0.35), 1.0, 0.9);
    for (row, at) in &flow.reveals {
        sequence.reveal(&mut scene, row, time(*at));
    }
    for (row, at) in &flow.strikes {
        sequence.strike(&mut scene, row, time(*at));
        // The fix replays the same story: shared rows return unstruck.
        let strike = sequence.row_channel(&mut scene, row, "strike", 0.0);
        scene.spring(&strike, switch, 0.0, 0.4, 0.0);
    }
    for row in flow.before_only {
        sequence.fade(&mut scene, row, switch, 0.0);
    }
    for (participant, on, off) in &flow.emphasis {
        let emphasis = sequence.participant_channel(&mut scene, participant, "emphasis", 0.0);
        let (on, off) = (time(*on), time(*off));
        scene.spring(&emphasis, on, 1.0, 0.35, 0.0);
        // Emphasis from the broken story never outlives it.
        let off = if on < switch { off.min(switch) } else { off };
        scene.spring(&emphasis, off.max(on), 0.0, 0.45, 0.0);
    }
    for (participant, appear, fixed_opacity) in &flow.late {
        let opacity = sequence.participant_channel(&mut scene, participant, "opacity", 0.0);
        let appear = time(*appear);
        if appear < switch {
            // Introduced by the broken story; the fixed story sets its own presence.
            scene.spring(&opacity, appear, 1.0, 0.5, 0.0);
            scene.spring(&opacity, switch, *fixed_opacity, 0.45, 0.0);
        } else {
            scene.spring(&opacity, appear, *fixed_opacity, 0.5, 0.0);
        }
    }

    let mut footer_before = footer(&mut scene, "footer-before", flow.footer_before.0)?;
    footer_before.type_in(&mut scene, time(flow.footer_before.1), 42.0, 0.8);
    footer_before.hide(&mut scene, switch);
    let mut footer_after = footer(&mut scene, "footer-after", flow.footer_after.0)?;
    footer_after.type_in(&mut scene, time(flow.footer_after.1), 42.0, 0.8);
    scene
        .finish()
        .with_context(|| format!("{}-behavior", pr.slug))
}

/// `entrance`: the editor rises into place. Skip it when a zoom opens into the code.
pub fn code(
    pr: &Pr,
    narration: &Narration,
    (diff, steps, note): (Diff, Vec<&'static str>, &'static str),
    entrance: bool,
) -> Result<ScenePlan> {
    let clip = narration.clip(&format!("{}-code", pr.slug))?;
    let lead = seconds(0.9);
    let duration = lead + clip.duration() + seconds(1.6);
    let mut scene = PlanBuilder::new(format!("{}-code", pr.slug), duration);
    let spoken = clip.place(&mut scene, lead);
    header(&mut scene, pr, None)?;
    let mut change = chip(&mut scene, "chip-change", Tone::Accent, "the change")?;
    change.show(&mut scene, seconds(0.2));
    let times = steps
        .iter()
        .map(|phrase| spoken.at(phrase))
        .collect::<Vec<_>>();
    diff.declare(&mut scene, &times, seconds(0.9), entrance)?;
    let mut caption = footer(&mut scene, "footer", vec![span(note, Tone::Muted)])?;
    caption.show(&mut scene, seconds(0.6));
    scene.finish().with_context(|| format!("{}-code", pr.slug))
}

/// A phrase in the after clip that is only searched once `earlier` has been said.
pub fn after_following(phrase: &'static str, earlier: &'static str) -> At {
    At(When::AfterFollowing(phrase, earlier), 0.0)
}
