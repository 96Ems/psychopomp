use std::{
    collections::{BTreeSet, HashSet},
    fmt,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::math::{easing::cubic_in_out, smoothstep, vec2};

mod channels;
pub use channels::{SpringPlan, compile_channels, destination_channel, effective_snapshots};

pub const SCENE_PLAN_VERSION: u32 = 2;

/// One native presentation containing independently authored Scene Plans.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckPlan {
    pub version: u32,
    pub id: String,
    pub slides: Vec<SlidePlan>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlidePlan {
    pub title: String,
    pub plan: ScenePlan,
}

impl DeckPlan {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.version != 1 || self.id.trim().is_empty() || self.slides.is_empty() {
            anyhow::bail!("deck requires version 1, an ID, and at least one slide");
        }
        let mut ids = HashSet::new();
        for slide in &self.slides {
            slide.plan.validate()?;
            if slide.title.trim().is_empty()
                || slide.plan.presentation_steps.is_empty()
                || !ids.insert(&slide.plan.id)
            {
                anyhow::bail!(
                    "deck slides need a title, presentation steps, and distinct scene IDs"
                );
            }
        }
        Ok(())
    }
}

/// One encoded video that plays independently authored Scene Plans in order on a
/// single clock. Each segment keeps its own actors and local time; a transition
/// crossfades from the previous segment, so at most two segments overlap.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReelPlan {
    pub version: u32,
    pub id: String,
    pub segments: Vec<ReelSegmentPlan>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReelSegmentPlan {
    /// Transition from the previous segment. The first segment must use zero.
    #[serde(default)]
    pub transition_nanos: u64,
    #[serde(default)]
    pub transition_style: ReelTransitionStyle,
    /// For `zoom`: the rectangle (x, y, width, height) in the outgoing frame that
    /// becomes this segment, such as a card that opens into its code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_focus: Option<[f32; 4]>,
    pub plan: ScenePlan,
}

/// How a segment replaces its predecessor during `transition_nanos`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReelTransitionStyle {
    /// Both segments are visible while the incoming one fades in over the other.
    #[default]
    Crossfade,
    /// The outgoing segment fades to the empty background, then the incoming one
    /// fades in. Dense frames never overlap.
    Dip,
    /// The camera flies into `transition_focus`: the outgoing frame zooms past
    /// while the incoming segment grows out of that rectangle.
    Zoom,
}

/// Screen transform of one layer during a zoom: `output = source * scale + offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReelZoom {
    pub scale: f32,
    pub offset: [f32; 2],
    /// Corner radius of the incoming frame while it is still a card.
    pub radius: f32,
}

impl ReelZoom {
    /// Where the layer sits at `progress` of a zoom into `focus` on a
    /// `width` x `height` frame. The outgoing frame magnifies until `focus`
    /// fills the width; the incoming frame starts inside `focus`.
    pub fn at(focus: [f32; 4], width: f32, height: f32, progress: f32, incoming: bool) -> Self {
        let eased = cubic_in_out(progress.clamp(0.0, 1.0));
        let focus_center = vec2(focus[0] + focus[2] * 0.5, focus[1] + focus[3] * 0.5);
        let center = vec2(width, height) * 0.5;
        let fill = width / focus[2].max(1.0);
        // The focus center travels to the screen center while the scale changes
        // geometrically, so the zoom speed feels constant.
        let anchor = focus_center.lerp(center, eased);
        let (scale, pivot) = if incoming {
            (fill.powf(eased - 1.0), center)
        } else {
            (fill.powf(eased), focus_center)
        };
        Self {
            scale,
            offset: (anchor - pivot * scale).to_array(),
            radius: if incoming { 28.0 * (1.0 - eased) } else { 0.0 },
        }
    }
}

/// Where one segment sits on the reel clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReelSpan {
    pub start_nanos: u64,
    pub end_nanos: u64,
    pub transition_nanos: u64,
    pub transition_style: ReelTransitionStyle,
    pub transition_focus: Option<[f32; 4]>,
}

/// One segment visible at a reel time, sampled at its own local time. Layers are
/// mixed in order over what is below them by `weight`; when the first layer's
/// weight is below one it is mixed over the empty background.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReelLayer {
    pub segment: usize,
    pub local_seconds: f64,
    pub weight: f32,
    /// Set during a zoom; the renderer resolves it with `ReelZoom::at`.
    pub zoom: Option<ZoomPhase>,
}

/// One layer's part in a zoom transition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoomPhase {
    pub focus: [f32; 4],
    pub progress: f32,
    pub incoming: bool,
}

impl ReelPlan {
    pub const VERSION: u32 = 1;

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.version != Self::VERSION {
            anyhow::bail!("reel requires version {}", Self::VERSION);
        }
        if self.id.trim().is_empty() || self.id.chars().any(char::is_whitespace) {
            anyhow::bail!("reel requires an ID without whitespace");
        }
        if self.segments.is_empty() {
            anyhow::bail!("reel requires at least one segment");
        }
        let mut ids = HashSet::new();
        for (index, segment) in self.segments.iter().enumerate() {
            segment.plan.validate()?;
            if !ids.insert(&segment.plan.id) {
                anyhow::bail!(
                    "reel segment {index} repeats scene ID '{}'",
                    segment.plan.id
                );
            }
            if index == 0 {
                if segment.transition_nanos != 0 {
                    anyhow::bail!("the first reel segment cannot transition from nothing");
                }
                continue;
            }
            if segment.transition_style == ReelTransitionStyle::Zoom {
                let focus = segment.transition_focus.unwrap_or_default();
                if focus.iter().any(|v| !v.is_finite()) || focus[2] < 8.0 || focus[3] < 8.0 {
                    anyhow::bail!(
                        "reel segment '{}' zooms without a transitionFocus rectangle",
                        segment.plan.id
                    );
                }
            }
            let previous = &self.segments[index - 1];
            // The previous segment must be alone on screen before this one starts,
            // so no instant ever blends three segments.
            let available = previous
                .plan
                .duration_nanos
                .saturating_sub(previous.transition_nanos);
            if segment.transition_nanos > available
                || segment.transition_nanos > segment.plan.duration_nanos
            {
                anyhow::bail!(
                    "reel segment '{}' transition is longer than the time either neighbor is alone on screen",
                    segment.plan.id
                );
            }
        }
        Ok(())
    }

    pub fn spans(&self) -> Vec<ReelSpan> {
        let mut spans = Vec::with_capacity(self.segments.len());
        let mut end = 0_u64;
        for segment in &self.segments {
            let start = end.saturating_sub(segment.transition_nanos);
            end = start + segment.plan.duration_nanos;
            spans.push(ReelSpan {
                start_nanos: start,
                end_nanos: end,
                transition_nanos: segment.transition_nanos,
                transition_style: segment.transition_style,
                transition_focus: segment.transition_focus,
            });
        }
        spans
    }

    pub fn duration_nanos(&self) -> u64 {
        self.spans().last().map_or(0, |span| span.end_nanos)
    }

    /// The segments visible at `seconds`, in draw order. Outside transitions this
    /// is one fully weighted segment. A crossfade mixes the incoming segment over
    /// the outgoing one; a dip shows one segment faded toward the background.
    pub fn layers_at(&self, seconds: f64) -> Vec<ReelLayer> {
        let spans = self.spans();
        let at = seconds.max(0.0);
        let local = |index: usize| {
            let span = spans[index];
            let start = span.start_nanos as f64 / 1e9;
            (at - start).clamp(0.0, (span.end_nanos - span.start_nanos) as f64 / 1e9)
        };
        // The latest segment that has started is the one being entered or shown.
        let Some(current) = spans
            .iter()
            .rposition(|span| span.start_nanos as f64 / 1e9 <= at)
        else {
            return Vec::new();
        };
        let span = spans[current];
        let progress = if span.transition_nanos == 0 {
            1.0
        } else {
            ((at - span.start_nanos as f64 / 1e9) / (span.transition_nanos as f64 / 1e9))
                .clamp(0.0, 1.0)
        };
        let layer = |segment: usize, weight: f64| ReelLayer {
            segment,
            local_seconds: local(segment),
            weight: smoothstep(weight as f32),
            zoom: None,
        };
        if progress >= 1.0 || current == 0 {
            return vec![layer(current, 1.0)];
        }
        match span.transition_style {
            ReelTransitionStyle::Crossfade if progress <= 0.0 => vec![layer(current - 1, 1.0)],
            ReelTransitionStyle::Crossfade => {
                vec![layer(current - 1, 1.0), layer(current, progress)]
            }
            ReelTransitionStyle::Dip if progress < 0.5 => {
                vec![layer(current - 1, 1.0 - progress * 2.0)]
            }
            ReelTransitionStyle::Dip => vec![layer(current, progress * 2.0 - 1.0)],
            ReelTransitionStyle::Zoom => {
                let focus = span.transition_focus.unwrap_or([0.0, 0.0, 1.0, 1.0]);
                let phase = |incoming| {
                    Some(ZoomPhase {
                        focus,
                        progress: progress as f32,
                        incoming,
                    })
                };
                // The incoming card fades in while it is still small.
                vec![
                    ReelLayer {
                        zoom: phase(false),
                        ..layer(current - 1, 1.0)
                    },
                    ReelLayer {
                        zoom: phase(true),
                        ..layer(current, (progress - 0.08) / 0.4)
                    },
                ]
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenePlan {
    pub version: u32,
    pub id: String,
    pub duration_nanos: u64,
    #[serde(default)]
    pub actors: Vec<ActorPlan>,
    #[serde(default)]
    pub semantic_targets: Vec<SemanticTargetPlan>,
    #[serde(default)]
    pub continuous_channels: Vec<ContinuousChannelPlan>,
    #[serde(default)]
    pub state_channels: Vec<StateChannelPlan>,
    #[serde(default)]
    pub cues: Vec<CuePlan>,
    #[serde(default)]
    pub media: Vec<MediaPlan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presentation_steps: Vec<PresentationStepPlan>,
}

impl ScenePlan {
    pub fn new(id: impl Into<String>, duration_nanos: u64) -> Self {
        Self {
            version: SCENE_PLAN_VERSION,
            id: id.into(),
            duration_nanos,
            actors: Vec::new(),
            semantic_targets: Vec::new(),
            continuous_channels: Vec::new(),
            state_channels: Vec::new(),
            cues: Vec::new(),
            media: Vec::new(),
            presentation_steps: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), PlanValidationError> {
        let mut diagnostics = Vec::new();
        if self.version != SCENE_PLAN_VERSION {
            diagnostics.push(PlanDiagnostic::new(
                "unsupported-plan-version",
                "version",
                format!(
                    "scene plan version {} is not supported; expected {SCENE_PLAN_VERSION}",
                    self.version
                ),
            ));
        }
        validate_id(&self.id, "id", "scene", &mut diagnostics);
        if self.duration_nanos == 0 {
            diagnostics.push(PlanDiagnostic::new(
                "invalid-duration",
                "durationNanos",
                "scene duration must be positive",
            ));
        }

        let mut actor_ids = HashSet::new();
        for (index, actor) in self.actors.iter().enumerate() {
            let path = format!("actors[{index}]");
            validate_id(&actor.id, &format!("{path}.id"), "actor", &mut diagnostics);
            validate_id(
                &actor.recipe,
                &format!("{path}.recipe"),
                "actor recipe",
                &mut diagnostics,
            );
            if !actor_ids.insert(actor.id.as_str()) {
                diagnostics.push(PlanDiagnostic::new(
                    "duplicate-actor-id",
                    format!("{path}.id"),
                    format!("actor '{}' is declared more than once", actor.id),
                ));
            }
        }

        let mut channel_ids = HashSet::new();
        let mut target_ids = HashSet::new();
        for (index, target) in self.semantic_targets.iter().enumerate() {
            let path = format!("semanticTargets[{index}]");
            validate_id(
                &target.id,
                &format!("{path}.id"),
                "semantic target",
                &mut diagnostics,
            );
            if !target_ids.insert(target.id.as_str()) {
                diagnostics.push(PlanDiagnostic::new(
                    "duplicate-semantic-target-id",
                    format!("{path}.id"),
                    format!("semantic target '{}' is declared more than once", target.id),
                ));
            }
            if !actor_ids.contains(target.actor_id.as_str()) {
                diagnostics.push(PlanDiagnostic::new(
                    "unknown-actor",
                    format!("{path}.actorId"),
                    format!(
                        "semantic target '{}' references unknown actor '{}'",
                        target.id, target.actor_id
                    ),
                ));
            }
        }

        let mut continuous_properties = HashSet::new();
        for (index, channel) in self.continuous_channels.iter().enumerate() {
            let path = format!("continuousChannels[{index}]");
            validate_channel(
                &channel.id,
                &channel.actor_id,
                &path,
                &actor_ids,
                &mut channel_ids,
                &mut diagnostics,
            );
            validate_id(
                &channel.property,
                &format!("{path}.property"),
                "continuous property",
                &mut diagnostics,
            );
            if !continuous_properties.insert((channel.actor_id.as_str(), channel.property.as_str()))
            {
                diagnostics.push(PlanDiagnostic::new(
                    "duplicate-actor-property",
                    format!("{path}.property"),
                    format!(
                        "actor '{}' has more than one continuous '{}' channel",
                        channel.actor_id, channel.property
                    ),
                ));
            }
            validate_scalar(
                &channel.initial,
                &target_ids,
                &format!("{path}.initial"),
                &mut diagnostics,
            );
            validate_track_events(
                &channel.events,
                self.duration_nanos,
                &target_ids,
                &path,
                &mut diagnostics,
            );
        }

        let mut state_properties = HashSet::new();
        for (index, channel) in self.state_channels.iter().enumerate() {
            let path = format!("stateChannels[{index}]");
            validate_channel(
                &channel.id,
                &channel.actor_id,
                &path,
                &actor_ids,
                &mut channel_ids,
                &mut diagnostics,
            );
            validate_id(
                &channel.state,
                &format!("{path}.state"),
                "state property",
                &mut diagnostics,
            );
            if !state_properties.insert((channel.actor_id.as_str(), channel.state.as_str())) {
                diagnostics.push(PlanDiagnostic::new(
                    "duplicate-actor-state",
                    format!("{path}.state"),
                    format!(
                        "actor '{}' has more than one '{}' state channel",
                        channel.actor_id, channel.state
                    ),
                ));
            }
            validate_ordered_times(
                channel.events.iter().map(|event| event.at_nanos),
                self.duration_nanos,
                &format!("{path}.events"),
                &mut diagnostics,
            );
        }

        let mut cue_ids = HashSet::new();
        for (index, cue) in self.cues.iter().enumerate() {
            let path = format!("cues[{index}]");
            validate_id(&cue.id, &format!("{path}.id"), "cue", &mut diagnostics);
            if !cue_ids.insert(cue.id.as_str()) {
                diagnostics.push(PlanDiagnostic::new(
                    "duplicate-cue-id",
                    format!("{path}.id"),
                    format!("cue '{}' is declared more than once", cue.id),
                ));
            }
            validate_range(
                cue.start_nanos,
                cue.end_nanos,
                self.duration_nanos,
                &path,
                &mut diagnostics,
            );
        }

        let mut step_ids = HashSet::new();
        let mut previous_hold = None;
        for (index, step) in self.presentation_steps.iter().enumerate() {
            let path = format!("presentationSteps[{index}]");
            validate_id(
                &step.id,
                &format!("{path}.id"),
                "presentation step",
                &mut diagnostics,
            );
            if !step_ids.insert(step.id.as_str()) {
                diagnostics.push(PlanDiagnostic::new(
                    "duplicate-presentation-step-id",
                    format!("{path}.id"),
                    format!("presentation step '{}' is declared more than once", step.id),
                ));
            }
            if step.title.trim().is_empty() {
                diagnostics.push(PlanDiagnostic::new(
                    "empty-step-title",
                    format!("{path}.title"),
                    "presentation step title must not be empty",
                ));
            }
            if step.start_nanos > step.hold_nanos || step.hold_nanos > self.duration_nanos {
                diagnostics.push(PlanDiagnostic::new(
                    "invalid-step-range",
                    &path,
                    "presentation step must satisfy 0 <= startNanos <= holdNanos <= scene duration",
                ));
            }
            if previous_hold.is_some_and(|hold| step.start_nanos < hold) {
                diagnostics.push(PlanDiagnostic::new(
                    "overlapping-presentation-steps",
                    &path,
                    "presentation steps must be in playback order and start at or after the preceding hold",
                ));
            }
            previous_hold = Some(step.hold_nanos);
        }

        let mut media_ids = HashSet::new();
        for (index, media) in self.media.iter().enumerate() {
            let path = format!("media[{index}]");
            validate_id(&media.id, &format!("{path}.id"), "media", &mut diagnostics);
            if !media_ids.insert(media.id.as_str()) {
                diagnostics.push(PlanDiagnostic::new(
                    "duplicate-media-id",
                    format!("{path}.id"),
                    format!("media '{}' is declared more than once", media.id),
                ));
            }
            if media.path.as_os_str().is_empty() {
                diagnostics.push(PlanDiagnostic::new(
                    "empty-media-path",
                    format!("{path}.path"),
                    "media path must not be empty",
                ));
            }
            if !media.gain_db.is_finite() {
                diagnostics.push(PlanDiagnostic::new(
                    "non-finite-value",
                    format!("{path}.gainDb"),
                    "media gain must be finite",
                ));
            }
            validate_range(
                media.source_start_nanos,
                media.source_end_nanos,
                u64::MAX,
                &format!("{path}.source"),
                &mut diagnostics,
            );
            validate_range(
                media.timeline_start_nanos,
                media.timeline_end_nanos,
                self.duration_nanos,
                &format!("{path}.timeline"),
                &mut diagnostics,
            );
            if media
                .source_end_nanos
                .saturating_sub(media.source_start_nanos)
                != media
                    .timeline_end_nanos
                    .saturating_sub(media.timeline_start_nanos)
            {
                diagnostics.push(PlanDiagnostic::new(
                    "media-duration-mismatch",
                    path,
                    format!("media '{}' source and timeline durations differ", media.id),
                ));
            }
        }

        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(PlanValidationError { diagnostics })
        }
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, ReadPlanError> {
        let plan = serde_json::from_str::<Self>(json).map_err(ReadPlanError::Json)?;
        plan.validate().map_err(ReadPlanError::Validation)?;
        Ok(plan)
    }

    pub fn schema() -> Value {
        serde_json::json!({
            "version": SCENE_PLAN_VERSION,
            "clock": "integer nanoseconds",
            "actor": {
                "required": ["id", "recipe"],
                "data": "recipe-owned JSON value"
            },
            "semanticTarget": {
                "required": ["id", "actorId", "selector"],
                "selector": "renderer-recipe-owned JSON value"
            },
            "continuousChannel": {
                "required": ["id", "actorId", "property", "initial"],
                "operations": ["set", "spring"],
                "scalar": "literal number or semantic target component"
            },
            "stateChannel": {
                "required": ["id", "actorId", "state", "initial"],
                "events": "ordered values at exact times"
            },
            "cue": {
                "required": ["id", "startNanos", "endNanos"]
            },
            "presentationStep": {
                "required": ["id", "title", "startNanos", "holdNanos"],
                "timing": "ordered, non-overlapping entry ranges; equal start and hold means a still step",
                "playback": "open at the first hold; Next plays the following entry then holds; Previous restores the preceding hold"
            },
            "media": {
                "required": [
                    "id", "path", "kind", "role", "sourceStartNanos", "sourceEndNanos",
                    "timelineStartNanos", "timelineEndNanos"
                ]
            }
        })
    }

    pub fn diff(&self, other: &Self) -> Result<Vec<PlanChange>, serde_json::Error> {
        let before = serde_json::to_value(self)?;
        let after = serde_json::to_value(other)?;
        let mut changes = Vec::new();
        diff_values("$", Some(&before), Some(&after), &mut changes);
        Ok(changes)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanChange {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActorPlan {
    pub id: String,
    pub recipe: String,
    #[serde(default)]
    pub data: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTargetPlan {
    pub id: String,
    pub actor_id: String,
    pub selector: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ScalarPlan {
    Literal(f32),
    Target(TargetScalarPlan),
}

impl From<f32> for ScalarPlan {
    fn from(value: f32) -> Self {
        Self::Literal(value)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetScalarPlan {
    pub target_id: String,
    pub component: TargetComponentPlan,
    #[serde(default)]
    pub offset: f32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TargetComponentPlan {
    X,
    Width,
    CenterX,
    LineY,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuousChannelPlan {
    pub id: String,
    pub actor_id: String,
    pub property: String,
    pub initial: ScalarPlan,
    #[serde(default)]
    pub events: Vec<TrackEventPlan>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "operation"
)]
pub enum TrackEventPlan {
    Set {
        at_nanos: u64,
        value: ScalarPlan,
    },
    Spring {
        at_nanos: u64,
        target: ScalarPlan,
        response_seconds: f32,
        damping_ratio: f32,
        position_threshold: f32,
        velocity_threshold: f32,
    },
}

impl TrackEventPlan {
    pub fn at_nanos(&self) -> u64 {
        match self {
            Self::Set { at_nanos, .. } | Self::Spring { at_nanos, .. } => *at_nanos,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateChannelPlan {
    pub id: String,
    pub actor_id: String,
    pub state: String,
    pub initial: Value,
    #[serde(default)]
    pub events: Vec<StateEventPlan>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateEventPlan {
    pub at_nanos: u64,
    pub value: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CuePlan {
    pub id: String,
    pub start_nanos: u64,
    pub end_nanos: u64,
}

/// An authored entry range and exact held endpoint on the original scene clock.
/// Presentation waits do not alter scene time or the automatic video schedule.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentationStepPlan {
    pub id: String,
    pub title: String,
    pub start_nanos: u64,
    pub hold_nanos: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MediaRolePlan {
    Script,
    Layer,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MediaKindPlan {
    Audio,
    Video,
    Image,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaPlan {
    pub id: String,
    pub path: std::path::PathBuf,
    pub kind: MediaKindPlan,
    pub role: MediaRolePlan,
    pub source_start_nanos: u64,
    pub source_end_nanos: u64,
    pub timeline_start_nanos: u64,
    pub timeline_end_nanos: u64,
    #[serde(default)]
    pub gain_db: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlanDiagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
}

impl PlanDiagnostic {
    fn new(code: impl Into<String>, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            path: path.into(),
            message: message.into(),
            suggestions: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct PlanValidationError {
    diagnostics: Vec<PlanDiagnostic>,
}

impl PlanValidationError {
    pub fn diagnostics(&self) -> &[PlanDiagnostic] {
        &self.diagnostics
    }
}

impl fmt::Display for PlanValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "scene plan has {} validation error(s)",
            self.diagnostics.len()
        )?;
        // Name each problem: a bare count sends authors digging through JSON.
        for diagnostic in self.diagnostics.iter().take(5) {
            write!(
                formatter,
                "\n  {} at {}: {}",
                diagnostic.code, diagnostic.path, diagnostic.message
            )?;
        }
        if self.diagnostics.len() > 5 {
            write!(formatter, "\n  …and {} more", self.diagnostics.len() - 5)?;
        }
        Ok(())
    }
}

impl std::error::Error for PlanValidationError {}

#[derive(Debug)]
pub enum ReadPlanError {
    Json(serde_json::Error),
    Validation(PlanValidationError),
}

impl fmt::Display for ReadPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "parse scene plan: {error}"),
            Self::Validation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ReadPlanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Validation(error) => Some(error),
        }
    }
}

fn validate_channel<'a>(
    id: &'a str,
    actor_id: &str,
    path: &str,
    actor_ids: &HashSet<&str>,
    channel_ids: &mut HashSet<&'a str>,
    diagnostics: &mut Vec<PlanDiagnostic>,
) {
    validate_id(id, &format!("{path}.id"), "channel", diagnostics);
    if !channel_ids.insert(id) {
        diagnostics.push(PlanDiagnostic::new(
            "duplicate-channel-id",
            format!("{path}.id"),
            format!("channel '{id}' is declared more than once"),
        ));
    }
    if !actor_ids.contains(actor_id) {
        let mut diagnostic = PlanDiagnostic::new(
            "unknown-actor",
            format!("{path}.actorId"),
            format!("channel '{id}' references unknown actor '{actor_id}'"),
        );
        diagnostic.suggestions = actor_ids.iter().map(|value| (*value).to_owned()).collect();
        diagnostic.suggestions.sort();
        diagnostics.push(diagnostic);
    }
}

fn validate_track_events(
    events: &[TrackEventPlan],
    duration_nanos: u64,
    target_ids: &HashSet<&str>,
    path: &str,
    diagnostics: &mut Vec<PlanDiagnostic>,
) {
    validate_ordered_times(
        events.iter().map(TrackEventPlan::at_nanos),
        duration_nanos,
        &format!("{path}.events"),
        diagnostics,
    );
    for (index, event) in events.iter().enumerate() {
        let event_path = format!("{path}.events[{index}]");
        match event {
            TrackEventPlan::Set { value, .. } => {
                validate_scalar(
                    value,
                    target_ids,
                    &format!("{event_path}.value"),
                    diagnostics,
                );
            }
            TrackEventPlan::Spring {
                target,
                response_seconds,
                damping_ratio,
                position_threshold,
                velocity_threshold,
                ..
            } => {
                validate_scalar(
                    target,
                    target_ids,
                    &format!("{event_path}.target"),
                    diagnostics,
                );
                if !response_seconds.is_finite()
                    || *response_seconds <= 0.0
                    || !(std::f32::consts::TAU / response_seconds).is_finite()
                {
                    diagnostics.push(PlanDiagnostic::new(
                        "invalid-spring",
                        format!("{event_path}.responseSeconds"),
                        "spring response must be positive, finite, and have a representable angular frequency",
                    ));
                }
                if !damping_ratio.is_finite() || *damping_ratio <= 0.0 || *damping_ratio > 1.0 {
                    diagnostics.push(PlanDiagnostic::new(
                        "invalid-spring",
                        format!("{event_path}.dampingRatio"),
                        "spring damping ratio must be finite and in (0, 1]",
                    ));
                }
                if !position_threshold.is_finite()
                    || *position_threshold <= 0.0
                    || !velocity_threshold.is_finite()
                    || *velocity_threshold <= 0.0
                {
                    diagnostics.push(PlanDiagnostic::new(
                        "invalid-spring",
                        event_path,
                        "spring thresholds must be positive and finite",
                    ));
                }
            }
        }
    }
}

fn validate_scalar(
    scalar: &ScalarPlan,
    target_ids: &HashSet<&str>,
    path: &str,
    diagnostics: &mut Vec<PlanDiagnostic>,
) {
    match scalar {
        ScalarPlan::Literal(value) if !value.is_finite() => diagnostics.push(PlanDiagnostic::new(
            "non-finite-value",
            path,
            "scalar value must be finite",
        )),
        ScalarPlan::Target(target) => {
            if !target_ids.contains(target.target_id.as_str()) {
                diagnostics.push(PlanDiagnostic::new(
                    "unknown-semantic-target",
                    format!("{path}.targetId"),
                    format!(
                        "scalar references unknown semantic target '{}'",
                        target.target_id
                    ),
                ));
            }
            if !target.offset.is_finite() {
                diagnostics.push(PlanDiagnostic::new(
                    "non-finite-value",
                    format!("{path}.offset"),
                    "semantic target offset must be finite",
                ));
            }
        }
        ScalarPlan::Literal(_) => {}
    }
}

fn validate_ordered_times(
    times: impl IntoIterator<Item = u64>,
    duration_nanos: u64,
    path: &str,
    diagnostics: &mut Vec<PlanDiagnostic>,
) {
    let mut previous = None;
    for (index, at) in times.into_iter().enumerate() {
        if at > duration_nanos {
            diagnostics.push(PlanDiagnostic::new(
                "event-after-scene",
                format!("{path}[{index}].atNanos"),
                format!("event at {at}ns exceeds scene duration {duration_nanos}ns"),
            ));
        }
        if previous.is_some_and(|value| at < value) {
            diagnostics.push(PlanDiagnostic::new(
                "unordered-events",
                format!("{path}[{index}].atNanos"),
                "events must be ordered by time; equal timestamps preserve source order",
            ));
        }
        previous = Some(at);
    }
}

fn validate_range(
    start: u64,
    end: u64,
    maximum_end: u64,
    path: &str,
    diagnostics: &mut Vec<PlanDiagnostic>,
) {
    if start >= end {
        diagnostics.push(PlanDiagnostic::new(
            "invalid-range",
            path,
            format!("range {start}ns..{end}ns must have positive duration"),
        ));
    }
    if end > maximum_end {
        diagnostics.push(PlanDiagnostic::new(
            "range-after-scene",
            path,
            format!("range end {end}ns exceeds scene duration {maximum_end}ns"),
        ));
    }
}

fn validate_id(id: &str, path: &str, kind: &str, diagnostics: &mut Vec<PlanDiagnostic>) {
    if id.is_empty() {
        diagnostics.push(PlanDiagnostic::new(
            "empty-id",
            path,
            format!("{kind} ID must not be empty"),
        ));
    } else if id.chars().any(char::is_whitespace) {
        diagnostics.push(PlanDiagnostic::new(
            "invalid-id",
            path,
            format!("{kind} ID '{id}' must not contain whitespace"),
        ));
    }
}

fn diff_values(
    path: &str,
    before: Option<&Value>,
    after: Option<&Value>,
    changes: &mut Vec<PlanChange>,
) {
    match (before, after) {
        (Some(Value::Object(before)), Some(Value::Object(after))) => {
            let keys = before
                .keys()
                .chain(after.keys())
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            for key in keys {
                diff_values(
                    &format!("{path}.{key}"),
                    before.get(key),
                    after.get(key),
                    changes,
                );
            }
        }
        (Some(Value::Array(before)), Some(Value::Array(after))) => {
            for index in 0..before.len().max(after.len()) {
                diff_values(
                    &format!("{path}[{index}]"),
                    before.get(index),
                    after.get(index),
                    changes,
                );
            }
        }
        (Some(before), Some(after)) if before == after => {}
        (before, after) => changes.push(PlanChange {
            path: path.to_owned(),
            before: before.cloned(),
            after: after.cloned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        ActorPlan, ContinuousChannelPlan, CuePlan, ScalarPlan, ScenePlan, SemanticTargetPlan,
        StateChannelPlan, StateEventPlan, TargetComponentPlan, TargetScalarPlan, TrackEventPlan,
    };

    fn plan() -> ScenePlan {
        let mut plan = ScenePlan::new("demo", 2_000_000_000);
        plan.actors.push(ActorPlan {
            id: "title".to_owned(),
            recipe: "text".to_owned(),
            data: json!({ "text": "Hello" }),
        });
        plan.continuous_channels.push(ContinuousChannelPlan {
            id: "title.opacity".to_owned(),
            actor_id: "title".to_owned(),
            property: "opacity".to_owned(),
            initial: 0.0.into(),
            events: vec![TrackEventPlan::Spring {
                at_nanos: 500_000_000,
                target: 1.0.into(),
                response_seconds: 0.4,
                damping_ratio: 1.0,
                position_threshold: 0.01,
                velocity_threshold: 0.01,
            }],
        });
        plan.state_channels.push(StateChannelPlan {
            id: "title.content".to_owned(),
            actor_id: "title".to_owned(),
            state: "content".to_owned(),
            initial: json!("Hello"),
            events: vec![StateEventPlan {
                at_nanos: 1_000_000_000,
                value: json!("World"),
            }],
        });
        plan.cues.push(CuePlan {
            id: "reveal".to_owned(),
            start_nanos: 500_000_000,
            end_nanos: 1_500_000_000,
        });
        plan
    }

    #[test]
    fn valid_plan_round_trips_as_deterministic_json() {
        let plan = plan();
        plan.validate().unwrap();
        let json = plan.to_json_pretty().unwrap();
        let decoded = ScenePlan::from_json(&json).unwrap();

        assert!(json.contains("\"responseSeconds\""));
        assert!(!json.contains("response_seconds"));
        assert_eq!(decoded.to_json_pretty().unwrap(), json);
    }

    #[test]
    fn unrepresentable_spring_frequency_returns_a_structured_diagnostic() {
        let mut plan = plan();
        let TrackEventPlan::Spring {
            response_seconds, ..
        } = &mut plan.continuous_channels[0].events[0]
        else {
            unreachable!()
        };
        *response_seconds = 1e-38;
        let error = plan.validate().unwrap_err();
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == "invalid-spring"
                    && diagnostic.path.ends_with("responseSeconds"))
        );
    }

    #[test]
    fn validation_returns_structured_paths_and_suggestions() {
        let mut plan = plan();
        plan.continuous_channels[0].actor_id = "missing".to_owned();
        let error = plan.validate().unwrap_err();
        let diagnostic = error
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == "unknown-actor")
            .unwrap();

        assert_eq!(diagnostic.path, "continuousChannels[0].actorId");
        assert_eq!(diagnostic.suggestions, ["title"]);
    }

    #[test]
    fn equal_event_times_are_ordered_and_preserved() {
        let mut plan = plan();
        plan.continuous_channels[0].events = vec![
            TrackEventPlan::Set {
                at_nanos: 1_000_000_000,
                value: 0.5.into(),
            },
            TrackEventPlan::Set {
                at_nanos: 1_000_000_000,
                value: 1.0.into(),
            },
        ];

        plan.validate().unwrap();
        assert_eq!(plan.continuous_channels[0].events.len(), 2);
    }

    #[test]
    fn unordered_events_are_rejected() {
        let mut plan = plan();
        plan.continuous_channels[0].events = vec![
            TrackEventPlan::Set {
                at_nanos: 1_500_000_000,
                value: 1.0.into(),
            },
            TrackEventPlan::Set {
                at_nanos: 500_000_000,
                value: 0.0.into(),
            },
        ];

        let error = plan.validate().unwrap_err();
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == "unordered-events")
        );
    }

    #[test]
    fn duplicate_actor_properties_are_rejected_even_with_unique_channel_ids() {
        let mut plan = plan();
        let mut duplicate = plan.continuous_channels[0].clone();
        duplicate.id = "other-opacity-channel".to_owned();
        plan.continuous_channels.push(duplicate);

        let error = plan.validate().unwrap_err();
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == "duplicate-actor-property")
        );
    }

    #[test]
    fn unsupported_overdamped_springs_are_rejected_before_compilation() {
        let mut plan = plan();
        let TrackEventPlan::Spring { damping_ratio, .. } =
            &mut plan.continuous_channels[0].events[0]
        else {
            unreachable!();
        };
        *damping_ratio = 1.1;

        let error = plan.validate().unwrap_err();
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.path.ends_with(".dampingRatio"))
        );

        let TrackEventPlan::Spring { damping_ratio, .. } =
            &mut plan.continuous_channels[0].events[0]
        else {
            unreachable!();
        };
        *damping_ratio = 0.0;
        assert!(plan.validate().is_err());
    }

    #[test]
    fn semantic_target_scalars_round_trip_and_validate_references() {
        let mut plan = plan();
        plan.semantic_targets.push(SemanticTargetPlan {
            id: "title-text".to_owned(),
            actor_id: "title".to_owned(),
            selector: json!({ "rangeId": "title" }),
        });
        plan.continuous_channels[0].initial = ScalarPlan::Target(TargetScalarPlan {
            target_id: "title-text".to_owned(),
            component: TargetComponentPlan::CenterX,
            offset: 40.0,
        });
        plan.validate().unwrap();
        let json = plan.to_json_pretty().unwrap();
        let decoded = ScenePlan::from_json(&json).unwrap();
        assert!(json.contains("\"component\": \"center-x\""));
        assert_eq!(decoded.to_json_pretty().unwrap(), json);

        plan.semantic_targets.clear();
        let error = plan.validate().unwrap_err();
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == "unknown-semantic-target")
        );
    }

    #[test]
    fn diff_is_stable_and_distinguishes_missing_values_from_null() {
        let before = plan();
        let mut after = plan();
        after.actors[0].data = json!({ "text": null, "color": "white" });
        after.cues[0].end_nanos = 1_750_000_000;

        let changes = before.diff(&after).unwrap();
        assert_eq!(
            changes
                .iter()
                .map(|change| change.path.as_str())
                .collect::<Vec<_>>(),
            [
                "$.actors[0].data.color",
                "$.actors[0].data.text",
                "$.cues[0].endNanos"
            ]
        );
        assert_eq!(changes[0].before, None);
        assert_eq!(changes[0].after, Some(json!("white")));
        assert_eq!(changes[1].after, Some(json!(null)));
    }

    #[test]
    fn presentation_metadata_is_optional_and_does_not_change_existing_json() {
        let original = plan().to_json_pretty().unwrap();
        assert!(!original.contains("presentationSteps"));
        let mut plan = ScenePlan::from_json(&original).unwrap();
        plan.presentation_steps = vec![
            super::PresentationStepPlan {
                id: "initial".into(),
                title: "Initial state".into(),
                start_nanos: 0,
                hold_nanos: 0,
            },
            super::PresentationStepPlan {
                id: "reveal".into(),
                title: "Reveal".into(),
                start_nanos: 500_000_000,
                hold_nanos: 2_000_000_000,
            },
        ];
        let json = plan.to_json_pretty().unwrap();
        assert_eq!(
            ScenePlan::from_json(&json)
                .unwrap()
                .to_json_pretty()
                .unwrap(),
            json
        );
        plan.presentation_steps.clear();
        assert_eq!(plan.to_json_pretty().unwrap(), original);
    }

    #[test]
    fn presentation_steps_validate_identity_titles_ranges_and_order() {
        let mut plan = plan();
        plan.presentation_steps = vec![
            super::PresentationStepPlan {
                id: "first".into(),
                title: "First".into(),
                start_nanos: 0,
                hold_nanos: 1_000_000_000,
            },
            super::PresentationStepPlan {
                id: "first".into(),
                title: " ".into(),
                start_nanos: 500_000_000,
                hold_nanos: 3_000_000_000,
            },
        ];
        let error = plan.validate().unwrap_err();
        for code in [
            "duplicate-presentation-step-id",
            "empty-step-title",
            "invalid-step-range",
            "overlapping-presentation-steps",
        ] {
            assert!(
                error
                    .diagnostics()
                    .iter()
                    .any(|diagnostic| diagnostic.code == code),
                "{code}"
            );
        }
        plan.presentation_steps[0].start_nanos = 1_500_000_000;
        assert!(
            plan.validate()
                .unwrap_err()
                .diagnostics()
                .iter()
                .any(|d| d.path == "presentationSteps[0]" && d.code == "invalid-step-range")
        );
    }
}

#[cfg(test)]
mod reel_tests {
    use super::{
        ReelLayer, ReelPlan, ReelSegmentPlan, ReelSpan,
        ReelTransitionStyle::{self, Crossfade, Dip},
        ScenePlan,
    };

    const SECOND: u64 = 1_000_000_000;

    fn reel(segments: &[(&str, u64, u64)]) -> ReelPlan {
        ReelPlan {
            version: ReelPlan::VERSION,
            id: "walkthrough".to_owned(),
            segments: segments
                .iter()
                .map(|&(id, duration, transition)| ReelSegmentPlan {
                    transition_nanos: transition,
                    transition_style: ReelTransitionStyle::default(),
                    transition_focus: None,
                    plan: ScenePlan::new(id, duration),
                })
                .collect(),
        }
    }

    #[test]
    fn transitions_overlap_neighbors_on_one_clock() {
        let reel = reel(&[
            ("intro", 4 * SECOND, 0),
            ("a", 6 * SECOND, SECOND),
            ("b", 3 * SECOND, 0),
        ]);
        reel.validate().unwrap();
        assert_eq!(
            reel.spans(),
            vec![
                ReelSpan {
                    start_nanos: 0,
                    end_nanos: 4 * SECOND,
                    transition_nanos: 0,
                    transition_style: Crossfade,
                    transition_focus: None,
                },
                ReelSpan {
                    start_nanos: 3 * SECOND,
                    end_nanos: 9 * SECOND,
                    transition_nanos: SECOND,
                    transition_style: Crossfade,
                    transition_focus: None,
                },
                ReelSpan {
                    start_nanos: 9 * SECOND,
                    end_nanos: 12 * SECOND,
                    transition_nanos: 0,
                    transition_style: Crossfade,
                    transition_focus: None,
                },
            ]
        );
        assert_eq!(reel.duration_nanos(), 12 * SECOND);
    }

    #[test]
    fn crossfade_mixes_the_incoming_segment_over_the_outgoing_one() {
        let reel = reel(&[("intro", 4 * SECOND, 0), ("a", 6 * SECOND, SECOND)]);
        let only = |segment, local_seconds| {
            vec![ReelLayer {
                segment,
                local_seconds,
                weight: 1.0,
                zoom: None,
            }]
        };
        assert_eq!(reel.layers_at(1.0), only(0, 1.0));
        let middle = reel.layers_at(3.5);
        assert_eq!(middle.len(), 2);
        assert_eq!((middle[0].segment, middle[0].weight), (0, 1.0));
        assert_eq!(middle[1].segment, 1);
        assert!((middle[1].weight - 0.5).abs() < 1e-6);
        assert!((middle[1].local_seconds - 0.5).abs() < 1e-9);
        assert!(
            reel.layers_at(3.1)[1].weight < 0.1,
            "smoothstep starts gently"
        );
        assert_eq!(
            reel.layers_at(3.0),
            only(0, 3.0),
            "the first instant is still the outgoing segment"
        );
        assert_eq!(reel.layers_at(4.0), only(1, 1.0));
        assert_eq!(
            reel.layers_at(9.0),
            only(1, 6.0),
            "the final frame samples the end"
        );
    }

    #[test]
    fn dip_passes_through_the_background_without_overlap() {
        let mut reel = reel(&[("intro", 4 * SECOND, 0), ("a", 6 * SECOND, SECOND)]);
        reel.segments[1].transition_style = Dip;
        let early = reel.layers_at(3.25);
        assert_eq!(early.len(), 1);
        assert_eq!(early[0].segment, 0);
        assert!((early[0].weight - 0.5).abs() < 1e-6);
        let midpoint = reel.layers_at(3.5);
        assert_eq!(midpoint.len(), 1);
        assert!(
            midpoint[0].weight.abs() < 1e-6,
            "the midpoint is the empty background"
        );
        let late = reel.layers_at(3.75);
        assert_eq!((late[0].segment, late.len()), (1, 1));
        assert!((late[0].weight - 0.5).abs() < 1e-6);
        assert!((late[0].local_seconds - 0.75).abs() < 1e-9);
    }

    #[test]
    fn zoom_opens_the_incoming_segment_out_of_the_focus_rectangle() {
        use super::ReelZoom;
        let focus = [240.0, 360.0, 480.0, 270.0];
        let start = ReelZoom::at(focus, 1920.0, 1080.0, 0.0, true);
        // The incoming frame begins exactly inside the focus rectangle.
        assert!((start.scale - 0.25).abs() < 1e-6);
        assert!((start.offset[0] - 240.0).abs() < 1e-3 && (start.offset[1] - 360.0).abs() < 1e-3);
        let end = ReelZoom::at(focus, 1920.0, 1080.0, 1.0, true);
        assert!((end.scale - 1.0).abs() < 1e-6 && end.offset[0].abs() < 1e-3 && end.radius == 0.0);
        let out = ReelZoom::at(focus, 1920.0, 1080.0, 1.0, false);
        // The outgoing frame magnifies the focus rectangle to fill the screen.
        assert!((out.scale - 4.0).abs() < 1e-5);
        assert!((240.0 * out.scale + out.offset[0]).abs() < 1e-2);
        let mut zoom = reel(&[("stage", 4 * SECOND, 0), ("code", 4 * SECOND, SECOND)]);
        zoom.segments[1].transition_style = super::ReelTransitionStyle::Zoom;
        assert!(zoom.validate().is_err(), "a zoom needs a focus rectangle");
        zoom.segments[1].transition_focus = Some(focus);
        zoom.validate().unwrap();
        let layers = zoom.layers_at(3.5);
        assert_eq!(layers.len(), 2);
        assert!(layers[0].zoom.is_some_and(|phase| !phase.incoming));
        assert!(layers[1].zoom.is_some_and(|phase| phase.incoming));
    }

    #[test]
    fn invalid_reels_are_rejected() {
        assert!(reel(&[]).validate().is_err());
        assert!(
            reel(&[("a", SECOND, 1)]).validate().is_err(),
            "first segment cannot fade in"
        );
        assert!(
            reel(&[("a", SECOND, 0), ("a", SECOND, 0)])
                .validate()
                .is_err(),
            "IDs repeat"
        );
        assert!(
            reel(&[("a", SECOND, 0), ("b", 3 * SECOND, 2 * SECOND)])
                .validate()
                .is_err(),
            "transition longer than the previous segment"
        );
        assert!(
            reel(&[
                ("a", 3 * SECOND, 0),
                ("b", 2 * SECOND, 2 * SECOND),
                ("c", 4 * SECOND, SECOND)
            ])
            .validate()
            .is_err(),
            "three segments would overlap"
        );
        let mut versioned = reel(&[("a", SECOND, 0)]);
        versioned.version = 2;
        assert!(versioned.validate().is_err());
    }

    #[test]
    fn reel_json_is_camel_case_and_strict() {
        let reel = reel(&[("intro", SECOND, 0), ("a", SECOND, 250_000_000)]);
        let json = serde_json::to_string(&reel).unwrap();
        assert!(json.contains("\"transitionNanos\":250000000"));
        let decoded: ReelPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.spans(), reel.spans());
        let unknown = json.replacen("\"segments\"", "\"extra\":1,\"segments\"", 1);
        assert!(serde_json::from_str::<ReelPlan>(&unknown).is_err());
    }
}
