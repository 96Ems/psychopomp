use std::{
    collections::{BTreeSet, HashSet},
    fmt,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
        )
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
