//! Pure, typed recipe preflight. Each payload is decoded once and retained for
//! resource preparation; sampling never interprets immutable actor JSON again.
use super::{
    caption::PreparedCaption,
    component_prototype::{self, ComponentInput},
    deployment_queue::PreparedDeploymentQueue,
    diagram,
    editor::{EditorSelection, PreparedEditor},
    generated,
    grid::PreparedGrid,
    header,
    lanes::PreparedLanes,
    plot::PreparedPlot,
    rolling::RollingNumberInput,
    sequence::PreparedSequence,
    terminal::TerminalInput,
    value::PreparedValueToken,
    venn::PreparedVenn,
};
use crate::render::{RichTextSource, VerticalMask};
use anyhow::{Context, Result, bail};
use psychopomp::{
    caption::CAPTION_RECIPE,
    component_prototype::{
        COLLECTION, CONNECTOR, DIAGRAM, DiagramPlan, HEADER, HeaderPlan, RICH_TEXT, TYPESET, VENN,
        WIDTH_TEXT,
    },
    deployment::DEPLOYMENT_QUEUE_RECIPE,
    editor::{EDITOR_RECIPE, EditorTargetSelector, POINTER_RECIPE, PointerRecipePlan},
    grid::GRID_RECIPE,
    lanes::LANES_RECIPE,
    plan::{ActorPlan, ContinuousChannelPlan, MediaKindPlan, ScenePlan, StateChannelPlan},
    plot::PLOT_RECIPE,
    rolling::ROLLING_NUMBER_RECIPE,
    sequence::SEQUENCE_RECIPE,
    stage::{STAGE_RECIPE, StagePlan},
    state::{StateTrack, TimedState},
    task::{TASK_RECIPE, TaskRecipePlan},
    terminal::TERMINAL_RECORDING_RECIPE,
    value::VALUE_TOKEN_RECIPE,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashSet;

pub(super) struct Plan {
    pub plan: ScenePlan,
    pub root: RootPlan,
    pub texts: Vec<PlainText>,
    pub tasks: Vec<(String, TaskRecipePlan)>,
    pub components: Vec<ComponentInput>,
    pub rich_text: Vec<(String, RichTextSource)>,
    pub headers: Vec<(String, HeaderPlan)>,
    pub value_tokens: Vec<PreparedValueToken>,
    pub venn: Vec<PreparedVenn>,
    pub sequences: Vec<PreparedSequence>,
    pub captions: Vec<PreparedCaption>,
    pub rolling: Vec<RollingNumberInput>,
    pub plots: Vec<PreparedPlot>,
    pub lanes: Vec<PreparedLanes>,
}
pub(super) enum RootPlan {
    Blank,
    Title(Title),
    Editor {
        editor: Box<PreparedEditor>,
        pointer: Option<String>,
        selectors: Vec<(String, EditorSelection)>,
    },
    Terminal(Box<TerminalInput>),
    Deployment(Box<PreparedDeploymentQueue>),
    Grid(Box<PreparedGrid>),
    Diagram {
        id: String,
        recipe: DiagramPlan,
    },
    Stage {
        id: String,
        recipe: Box<StagePlan>,
    },
}
pub(super) struct Title {
    pub id: String,
    pub title: String,
    pub subtitle: StateTrack<Option<String>>,
}
pub(super) struct PlainText {
    pub id: String,
    pub content: StateTrack<String>,
    pub center: [f32; 2],
    pub font_size: f32,
    pub color: [u8; 3],
    pub mask: Option<VerticalMask>,
}

pub(super) const NATIVE_UNSUPPORTED: &str = "interruptible native playback supports continuous-channel editor, pointer, text, effect-task, keyed-grid, value-token, and provisional component/diagram scenes; generic State Channels and recorded media still support video export";

/// A typed projection of observable CURRENT states. Raw JSON StateTracks retain
/// full equal-time history for cache identity and previous-snapshot semantics.
pub(super) fn current_state_track<T: Clone + PartialEq>(
    channels: &[StateChannelPlan],
    actor: &str,
    state: &str,
    fallback: Option<&Value>,
    duration: u64,
    decode: impl Fn(&Value) -> Result<T>,
) -> Result<StateTrack<T>> {
    if let Some(channel) = channels
        .iter()
        .find(|c| c.actor_id == actor && c.state == state)
    {
        // Use the same f64 event-selection clock as StateTrack, including large
        // integer timestamps that collapse to one sampled instant.
        let effective = channel
            .events
            .iter()
            .enumerate()
            .filter(|(index, event)| {
                channel.events.get(index + 1).is_none_or(|next| {
                    super::seconds_f64(next.at_nanos) != super::seconds_f64(event.at_nanos)
                })
            })
            .collect::<Vec<_>>();
        let at_zero = effective.first().filter(|(_, event)| event.at_nanos == 0);
        let initial = if let Some((index, event)) = at_zero {
            decode(&event.value)
                .with_context(|| format!("actor '{actor}' state '{state}' event {index}"))?
        } else {
            decode(&channel.initial)
                .with_context(|| format!("actor '{actor}' state '{state}' initial value"))?
        };
        let events = effective
            .into_iter()
            .filter(|(_, event)| event.at_nanos > 0)
            .map(|(i, event)| {
                Ok(TimedState::new(
                    super::seconds_f64(event.at_nanos),
                    decode(&event.value)
                        .with_context(|| format!("actor '{actor}' state '{state}' event {i}"))?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        StateTrack::compile(initial, events, super::seconds_f64(duration))
    } else {
        StateTrack::compile(
            decode(fallback.with_context(|| {
                format!("actor '{actor}' requires a '{state}' state or static value")
            })?)?,
            [],
            super::seconds_f64(duration),
        )
    }
}

impl Title {
    fn new(actor: &ActorPlan, plan: &ScenePlan) -> Result<Self> {
        let title = actor
            .data
            .get("title")
            .and_then(Value::as_str)
            .context("title-card actor requires string data.title")?
            .to_owned();
        let absent = Value::Null;
        let subtitle = current_state_track(
            &plan.state_channels,
            &actor.id,
            "subtitle",
            Some(actor.data.get("subtitle").unwrap_or(&absent)),
            plan.duration_nanos,
            |v| Ok(v.as_str().map(str::to_owned)),
        )?;
        Ok(Self {
            id: actor.id.clone(),
            title,
            subtitle,
        })
    }
}

/// Decode an actor's recipe and run its own validation, naming the actor in
/// either failure.
pub(super) fn decode<T: DeserializeOwned>(
    actor: &ActorPlan,
    kind: &str,
    validate: impl FnOnce(&T) -> Result<()>,
) -> Result<T> {
    let recipe: T = serde_json::from_value(actor.data.clone())
        .with_context(|| format!("parse {kind} recipe for actor '{}'", actor.id))?;
    validate(&recipe).with_context(|| format!("{kind} actor '{}'", actor.id))?;
    Ok(recipe)
}

/// Typos in channel names would silently do nothing, so they fail preflight.
pub(super) fn strict_channels(
    actor_id: &str,
    channels: &[ContinuousChannelPlan],
    kind: &str,
    accepts: impl Fn(&str) -> bool,
) -> Result<()> {
    if let Some(channel) = channels
        .iter()
        .find(|channel| channel.actor_id == actor_id && !accepts(&channel.property))
    {
        bail!(
            "{kind} actor '{actor_id}' has unknown property '{}'",
            channel.property
        );
    }
    Ok(())
}

pub(super) fn vertical_mask(actor: &ActorPlan) -> Result<Option<VerticalMask>> {
    let Some(value) = actor.data.get("verticalMask") else {
        return Ok(None);
    };
    let field = |name: &str| {
        value
            .get(name)
            .and_then(Value::as_f64)
            .map(|v| v as f32)
            .with_context(|| {
                format!(
                    "text actor '{}' verticalMask.{name} must be numeric",
                    actor.id
                )
            })
    };
    let mask = VerticalMask {
        top: field("top")?,
        bottom: field("bottom")?,
        fade: field("fade")?,
    };
    if !mask.is_valid() {
        bail!(
            "text actor '{}' verticalMask requires finite top < bottom and fade in [0, half the mask height]",
            actor.id
        );
    }
    Ok(Some(mask))
}
impl PlainText {
    fn new(actor: &ActorPlan, plan: &ScenePlan) -> Result<Self> {
        let mask = vertical_mask(actor)?;
        let center = actor
            .data
            .get("center")
            .and_then(Value::as_array)
            .filter(|v| v.len() == 2)
            .with_context(|| format!("text actor '{}' requires data.center [x, y]", actor.id))?;
        let coordinate = |i: usize, axis: &str| -> Result<f32> {
            let v = center[i].as_f64().with_context(|| {
                format!("text actor '{}' center {axis} must be numeric", actor.id)
            })?;
            if !v.is_finite() || v < f64::from(f32::MIN) || v > f64::from(f32::MAX) {
                bail!(
                    "text actor '{}' center {axis} must fit a finite f32",
                    actor.id
                );
            }
            Ok(v as f32)
        };
        let center = [coordinate(0, "x")?, coordinate(1, "y")?];
        let font_size = match actor.data.get("fontSize") {
            None => 28.,
            Some(value) => {
                let v = value.as_f64().with_context(|| {
                    format!("text actor '{}' fontSize must be numeric", actor.id)
                })?;
                if !v.is_finite() || v <= 0. || v > 512. {
                    bail!(
                        "text actor '{}' fontSize must be finite and in (0, 512]",
                        actor.id
                    );
                }
                v as f32
            }
        };
        let color = match actor.data.get("color") {
            None => [255; 3],
            Some(value) => {
                let values = value.as_array().filter(|v| v.len() == 3).with_context(|| {
                    format!("text actor '{}' color must be [r, g, b]", actor.id)
                })?;
                let mut color = [0; 3];
                for (out, v) in color.iter_mut().zip(values) {
                    *out = v.as_u64().filter(|v| *v <= 255).with_context(|| {
                        format!("text actor '{}' color channels must be bytes", actor.id)
                    })? as u8;
                }
                color
            }
        };
        let content = current_state_track(
            &plan.state_channels,
            &actor.id,
            "content",
            actor.data.get("text"),
            plan.duration_nanos,
            |v| {
                v.as_str()
                    .map(str::to_owned)
                    .context("text actor requires string data.text or content state")
            },
        )?;
        Ok(Self {
            id: actor.id.clone(),
            content,
            center,
            font_size,
            color,
            mask,
        })
    }
}

impl RootPlan {
    fn recipe(&self) -> Option<&'static str> {
        match self {
            Self::Blank => None,
            Self::Title(_) => Some("title-card"),
            Self::Editor { .. } => Some(EDITOR_RECIPE),
            Self::Terminal(_) => Some(TERMINAL_RECORDING_RECIPE),
            Self::Deployment(_) => Some(DEPLOYMENT_QUEUE_RECIPE),
            Self::Grid(_) => Some(GRID_RECIPE),
            Self::Diagram { .. } => Some(DIAGRAM),
            Self::Stage { .. } => Some(STAGE_RECIPE),
        }
    }
}
fn put_root(root: &mut RootPlan, next: RootPlan) -> Result<()> {
    if let Some(existing) = root.recipe() {
        if Some(existing) == next.recipe() {
            bail!("plan renderer currently supports at most one {existing} root actor");
        }
        bail!(
            "editor, title-card, terminal-recording, deployment-queue, keyed-grid, prototype-diagram, and stage actors are exclusive root recipes"
        );
    }
    *root = next;
    Ok(())
}

impl Plan {
    pub(super) fn new(plan: ScenePlan) -> Result<Self> {
        plan.validate()?;
        let mut root = RootPlan::Blank;
        let mut pointers = Vec::new();
        let mut texts = Vec::new();
        let mut tasks = Vec::new();
        let mut components = Vec::new();
        let mut rich_text = Vec::new();
        let mut headers = Vec::new();
        let mut value_tokens = Vec::new();
        let mut venn = Vec::new();
        let mut sequences = Vec::new();
        let mut captions = Vec::new();
        let mut rolling = Vec::new();
        let mut plots = Vec::new();
        let mut lanes = Vec::new();
        for actor in &plan.actors {
            match actor.recipe.as_str() {
                "title-card" => put_root(&mut root, RootPlan::Title(Title::new(actor, &plan)?))?,
                EDITOR_RECIPE => put_root(
                    &mut root,
                    RootPlan::Editor {
                        editor: Box::new(PreparedEditor::new(actor)?),
                        pointer: None,
                        selectors: Vec::new(),
                    },
                )?,
                POINTER_RECIPE => {
                    let recipe: PointerRecipePlan = serde_json::from_value(actor.data.clone())
                        .with_context(|| {
                            format!("parse pointer recipe for actor '{}'", actor.id)
                        })?;
                    pointers.push((actor.id.clone(), recipe.editor_id));
                }
                TERMINAL_RECORDING_RECIPE => put_root(
                    &mut root,
                    RootPlan::Terminal(Box::new(TerminalInput::new(
                        actor,
                        &plan.media,
                        &plan.state_channels,
                        plan.duration_nanos,
                    )?)),
                )?,
                DEPLOYMENT_QUEUE_RECIPE => put_root(
                    &mut root,
                    RootPlan::Deployment(Box::new(PreparedDeploymentQueue::new(
                        actor,
                        &plan.state_channels,
                        plan.duration_nanos,
                    )?)),
                )?,
                GRID_RECIPE => put_root(
                    &mut root,
                    RootPlan::Grid(Box::new(PreparedGrid::new(actor, plan.duration_nanos)?)),
                )?,
                DIAGRAM => {
                    let recipe: DiagramPlan = serde_json::from_value(actor.data.clone())?;
                    diagram::validate_recipe(&actor.id, &recipe, &plan.continuous_channels)?;
                    put_root(
                        &mut root,
                        RootPlan::Diagram {
                            id: actor.id.clone(),
                            recipe,
                        },
                    )?;
                }
                STAGE_RECIPE => put_root(
                    &mut root,
                    RootPlan::Stage {
                        id: actor.id.clone(),
                        recipe: Box::new(super::stage::validate_recipe(
                            actor,
                            &plan.continuous_channels,
                        )?),
                    },
                )?,
                "text" => texts.push(PlainText::new(actor, &plan)?),
                TASK_RECIPE => {
                    let recipe: TaskRecipePlan = serde_json::from_value(actor.data.clone())
                        .context("parse Effect task recipe")?;
                    recipe.validate(plan.duration_nanos)?;
                    tasks.push((actor.id.clone(), recipe));
                }
                TYPESET | WIDTH_TEXT | COLLECTION | CONNECTOR => {
                    components.push(ComponentInput::parse(actor)?)
                }
                RICH_TEXT => rich_text.push((
                    actor.id.clone(),
                    crate::render::parse_rich_text(serde_json::from_value(actor.data.clone())?)?,
                )),
                HEADER => headers.push((
                    actor.id.clone(),
                    serde_json::from_value(actor.data.clone())?,
                )),
                VALUE_TOKEN_RECIPE => value_tokens.push(PreparedValueToken::new(actor)?),
                VENN => venn.push(PreparedVenn::new(actor)?),
                SEQUENCE_RECIPE => {
                    sequences.push(PreparedSequence::new(actor, &plan.continuous_channels)?)
                }
                CAPTION_RECIPE => {
                    captions.push(PreparedCaption::new(actor, &plan.continuous_channels)?)
                }
                ROLLING_NUMBER_RECIPE => rolling.push(RollingNumberInput::new(
                    actor,
                    &plan.continuous_channels,
                    plan.duration_nanos,
                )?),
                PLOT_RECIPE => plots.push(PreparedPlot::new(actor, &plan.continuous_channels)?),
                LANES_RECIPE => lanes.push(PreparedLanes::new(actor, &plan.continuous_channels)?),
                recipe => bail!("unsupported actor recipe '{recipe}'"),
            }
        }
        for (pointer_id, editor_id) in pointers {
            let RootPlan::Editor {
                editor, pointer, ..
            } = &mut root
            else {
                bail!("pointer actor '{pointer_id}' references unknown editor '{editor_id}'");
            };
            if editor.actor_id() != editor_id {
                bail!("pointer actor '{pointer_id}' references unknown editor '{editor_id}'");
            }
            if pointer.replace(pointer_id).is_some() {
                bail!("editor '{editor_id}' has more than one pointer actor");
            }
        }
        for target in &plan.semantic_targets {
            let RootPlan::Editor {
                editor, selectors, ..
            } = &mut root
            else {
                bail!(
                    "actor '{}' recipe did not resolve semantic target '{}'",
                    target.actor_id,
                    target.id
                );
            };
            if editor.actor_id() != target.actor_id {
                bail!(
                    "actor '{}' recipe did not resolve semantic target '{}'",
                    target.actor_id,
                    target.id
                );
            }
            let selector: EditorTargetSelector = serde_json::from_value(target.selector.clone())
                .with_context(|| format!("parse editor semantic target '{}'", target.id))?;
            let selection = editor.select(&target.id, &selector)?;
            selectors.push((target.id.clone(), selection));
        }
        let consumed = match &root {
            RootPlan::Terminal(input) => input.media_ids().collect::<HashSet<_>>(),
            _ => HashSet::new(),
        };
        for media in &plan.media {
            if matches!(media.kind, MediaKindPlan::Audio)
                || (matches!(media.kind, MediaKindPlan::Video)
                    && consumed.contains(media.id.as_str()))
            {
                continue;
            }
            bail!(
                "plan renderer has no actor consuming {:?} media '{}'",
                media.kind,
                media.id
            );
        }
        component_prototype::validate_inputs(&plan, &components)?;
        let mut result = Self {
            plan,
            root,
            texts,
            tasks,
            components,
            rich_text,
            headers,
            value_tokens,
            venn,
            sequences,
            captions,
            rolling,
            plots,
            lanes,
        };
        match &result.root {
            RootPlan::Editor { editor, .. } => editor.compile_channels(&mut result.plan)?,
            RootPlan::Grid(grid) => {
                generated::extend(&mut result.plan, grid.channels(), generated::Owner::Grid)?
            }
            _ => {}
        }
        header::compile_inputs(&mut result.plan, &result.headers)?;
        let mut slots = generated::Reservations::new(&result.plan.continuous_channels);
        component_prototype::reserve_inputs(&result.components, &mut slots)?;
        for (id, recipe) in &result.tasks {
            for property in recipe.channel_properties() {
                slots.reserve(
                    &format!("{id}.{property}"),
                    id,
                    &property,
                    generated::Owner::Task,
                )?;
            }
        }
        super::attachments::reserve(&result.plan, &mut slots)?;
        result.plan.validate()?;
        Ok(result)
    }
    /// Whether interruptible native playback can drive this plan.
    pub(super) fn native(&self) -> bool {
        let root = match &self.root {
            RootPlan::Blank
            | RootPlan::Title(_)
            | RootPlan::Editor { .. }
            | RootPlan::Grid(_)
            | RootPlan::Diagram { .. }
            | RootPlan::Stage { .. } => true,
            RootPlan::Terminal(_) | RootPlan::Deployment(_) => false,
        };
        root && self.plan.state_channels.is_empty()
            && self.plan.media.is_empty()
            // Their changes follow the authored clock, not Playback destinations.
            && self.rolling.is_empty()
    }

    pub(super) fn require_native(&self) -> Result<()> {
        if !self.native() {
            bail!(NATIVE_UNSUPPORTED);
        }
        if self.plan.presentation_steps.is_empty() {
            bail!(
                "scene '{}' has no presentation steps; use PlanBuilder::presentation_step",
                self.plan.id
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
