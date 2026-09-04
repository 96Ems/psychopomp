use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufRead, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use kinograph::{
    composition::{Asset, Composition, Duration, Time, TimeRange},
    deployment::DEPLOYMENT_QUEUE_RECIPE,
    dsl::{Scalar, Scene, TargetGeometry},
    editor::{EDITOR_RECIPE, POINTER_RECIPE, PointerRecipePlan},
    plan::{
        MediaKindPlan, MediaRolePlan, ReadPlanError, ScalarPlan, ScenePlan, TargetComponentPlan,
        TrackEventPlan,
    },
    state::{StateTrack, TimedState},
    task::TASK_RECIPE,
    terminal::TERMINAL_RECORDING_RECIPE,
    timeline::{PropertyId, SpringProfile, TimedEvent, Timeline},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    render::{HeadlessRenderer, RenderSpec},
    scenes::{FONT_PATH, HEIGHT, WIDTH},
};

mod attachments;
mod delivery;
mod deployment_queue;
mod editor;
mod keyed_layout;
mod presentation;
#[cfg(test)]
mod stability_tests;
mod task;
mod terminal;

use deployment_queue::{DeploymentQueueVisualKey, PreparedDeploymentQueue};
use editor::PreparedEditor;
use terminal::PreparedTerminal;

const BUILTIN_HERO_PLAN: &str = include_str!("../../../scenes/hero/hero.plan.json");

pub(crate) async fn render_builtin_hero(output: &Path) -> Result<()> {
    let plan = ScenePlan::from_json(BUILTIN_HERO_PLAN)?;
    let window = TimeRange::new(Time::ZERO, Time::from_nanos(plan.duration_nanos));
    render_loaded_plan(plan, Path::new("."), output, window).await
}

pub(crate) fn command(arguments: &[String]) -> Result<()> {
    if arguments.first().is_some_and(|command| command == "render") {
        return render_command(&arguments[1..]);
    }
    if arguments.first().is_some_and(|command| command == "frame") {
        return frame_command(&arguments[1..]);
    }
    if arguments
        .first()
        .is_some_and(|command| command == "present")
    {
        let [path, flags @ ..] = &arguments[1..] else {
            bail!(
                "usage: kinograph plan present <plan.json> [--reduced-motion] [--full-quality] [--fps FPS] [--benchmark | --benchmark-gpu]"
            );
        };
        let options = presentation::Options::parse(flags)?;
        let json = fs::read_to_string(path)?;
        let value: Value = serde_json::from_str(&json)?;
        let slides = if value.get("slides").is_some() {
            let deck: kinograph::plan::DeckPlan = serde_json::from_value(value)?;
            deck.validate()?;
            deck.slides
        } else {
            let plan = read_plan(Path::new(path))?;
            vec![kinograph::plan::SlidePlan {
                title: plan.id.clone(),
                plan,
            }]
        };
        for slide in &slides {
            validate_renderer_plan(&slide.plan)?;
        }
        let base = Path::new(path).parent().unwrap_or_else(|| Path::new("."));
        return presentation::run(slides, base.to_owned(), options);
    }
    match arguments {
        [command] if command == "serve" => pollster::block_on(serve()),
        [command] if command == "schema" => {
            println!("{}", serde_json::to_string_pretty(&ScenePlan::schema())?);
            Ok(())
        }
        [command, path] if command == "validate" => {
            let plan = read_plan(Path::new(path))?;
            validate_renderer_plan(&plan)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "valid": true,
                    "path": path,
                }))?
            );
            Ok(())
        }
        [command, path] if command == "inspect" => {
            let plan = read_plan(Path::new(path))?;
            println!("{}", serde_json::to_string_pretty(&inspect_plan(&plan))?);
            Ok(())
        }
        [command, path] if command == "steps" => {
            let plan = read_plan(Path::new(path))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&kinograph::editor::inspect_steps(&plan)?)?
            );
            Ok(())
        }
        [command, before, after] if command == "diff" => {
            let before = read_plan(Path::new(before))?;
            let after = read_plan(Path::new(after))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({ "changes": before.diff(&after)? }))?
            );
            Ok(())
        }
        _ => bail!(
            "usage: kinograph plan serve | kinograph plan schema | kinograph plan validate <plan.json> | \
             kinograph plan inspect <plan.json> | kinograph plan steps <plan.json> | kinograph plan diff <before.json> <after.json> | \
             kinograph plan frame <plan.json> <seconds> [output.png] | \
              kinograph plan render <plan.json> [output] [--cue ID | --range START..END] | \
               kinograph plan present <plan.json> [--reduced-motion] [--full-quality] [--fps FPS] [--benchmark | --benchmark-gpu]"
        ),
    }
}

fn frame_command(arguments: &[String]) -> Result<()> {
    let [plan, seconds, rest @ ..] = arguments else {
        bail!("usage: kinograph plan frame <plan.json> <seconds> [output.png]");
    };
    if rest.len() > 1 {
        bail!("usage: kinograph plan frame <plan.json> <seconds> [output.png]");
    }
    let seconds = seconds
        .parse::<f64>()
        .context("parse frame time in seconds")?;
    let at = Time::try_seconds(seconds)
        .context("frame time must be finite, non-negative, and representable")?;
    let output = rest
        .first()
        .map_or_else(|| PathBuf::from("output/scene-plan.png"), PathBuf::from);
    render_frame(Path::new(plan), &output, at)
}

fn render_command(arguments: &[String]) -> Result<()> {
    let Some(path) = arguments.first() else {
        bail!("plan render requires a plan path");
    };
    let mut cursor = 1;
    let output = if arguments
        .get(cursor)
        .is_some_and(|argument| !argument.starts_with("--"))
    {
        let output = PathBuf::from(&arguments[cursor]);
        cursor += 1;
        output
    } else {
        PathBuf::from("output/scene-plan.mp4")
    };
    let selection = match arguments.get(cursor).map(String::as_str) {
        None => WindowSelection::Full,
        Some("--cue") => {
            let cue = arguments
                .get(cursor + 1)
                .context("--cue requires a cue ID")?
                .clone();
            cursor += 2;
            WindowSelection::Cue(cue)
        }
        Some("--range") => {
            let range = arguments
                .get(cursor + 1)
                .context("--range requires START..END seconds")?;
            cursor += 2;
            WindowSelection::Range(parse_range(range)?)
        }
        Some(argument) => bail!("unknown plan render option '{argument}'"),
    };
    if cursor != arguments.len() {
        bail!("unexpected plan render arguments");
    }
    render_plan(Path::new(path), &output, selection)
}

enum WindowSelection {
    Full,
    Cue(String),
    Range(TimeRange),
}

fn parse_range(value: &str) -> Result<TimeRange> {
    let (start, end) = value
        .split_once("..")
        .context("render range must use START..END seconds")?;
    let start = Time::try_seconds(start.parse::<f64>().context("parse render range start")?)
        .context("render range start must be finite, non-negative, and representable")?;
    let end = Time::try_seconds(end.parse::<f64>().context("parse render range end")?)
        .context("render range end must be finite, non-negative, and representable")?;
    if end <= start {
        bail!("render range must have positive duration");
    }
    Ok(TimeRange::new(start, end))
}

fn read_plan(path: &Path) -> Result<ScenePlan> {
    let json =
        fs::read_to_string(path).with_context(|| format!("read scene plan {}", path.display()))?;
    match ScenePlan::from_json(&json) {
        Ok(plan) => Ok(plan),
        Err(ReadPlanError::Validation(error)) => {
            eprintln!("{}", serde_json::to_string_pretty(error.diagnostics())?);
            Err(error.into())
        }
        Err(error) => Err(error.into()),
    }
}

fn render_plan(path: &Path, output: &Path, selection: WindowSelection) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output directory {}", parent.display()))?;
    }
    let plan = read_plan(path)?;
    let window = match selection {
        WindowSelection::Full => TimeRange::new(Time::ZERO, Time::from_nanos(plan.duration_nanos)),
        WindowSelection::Cue(id) => {
            let cue = plan
                .cues
                .iter()
                .find(|cue| cue.id == id)
                .with_context(|| format!("scene plan has no cue '{id}'"))?;
            TimeRange::new(
                Time::from_nanos(cue.start_nanos),
                Time::from_nanos(cue.end_nanos),
            )
        }
        WindowSelection::Range(range) => range,
    };
    let base = path.parent().unwrap_or_else(|| Path::new(".")).to_owned();
    pollster::block_on(render_loaded_plan(plan, &base, output, window))
}

fn render_frame(path: &Path, output: &Path, at: Time) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output directory {}", parent.display()))?;
    }
    let plan = read_plan(path)?;
    if at.as_nanos() > plan.duration_nanos {
        bail!(
            "frame time {} exceeds scene duration {}",
            at,
            Time::from_nanos(plan.duration_nanos)
        );
    }
    let base = path.parent().unwrap_or_else(|| Path::new(".")).to_owned();
    pollster::block_on(render_loaded_frame(plan, &base, output, at))
}

async fn render_loaded_plan(
    plan: ScenePlan,
    base: &Path,
    output: &Path,
    window: TimeRange,
) -> Result<()> {
    let mut renderer = new_renderer(&plan.id).await?;
    let prepared = PreparedPlan::prepare(plan, base, &mut renderer)?;
    delivery::render_video(&prepared, &mut renderer, output, window)
}

async fn render_loaded_frame(plan: ScenePlan, base: &Path, output: &Path, at: Time) -> Result<()> {
    let mut renderer = new_renderer(&plan.id).await?;
    let prepared = PreparedPlan::prepare(plan, base, &mut renderer)?;
    delivery::render_frame(&prepared, &mut renderer, output, at)
}

async fn new_renderer(file_name: &str) -> Result<HeadlessRenderer> {
    HeadlessRenderer::new(RenderSpec {
        width: WIDTH,
        height: HEIGHT,
        font_path: PathBuf::from(FONT_PATH),
        file_name: file_name.to_owned(),
    })
    .await
}

struct PreparedPlan {
    plan: ScenePlan,
    timeline: Timeline,
    properties: HashMap<String, PropertyId>,
    state_tracks: HashMap<String, StateTrack<serde_json::Value>>,
    scene: kinograph::dsl::CompiledScene,
    editors: Vec<PreparedEditor>,
    pointers: HashMap<String, String>,
    terminal: Option<PreparedTerminal>,
    deployment_queue: Option<PreparedDeploymentQueue>,
    attachments: Vec<attachments::Attachment>,
    tasks: Vec<task::PreparedTask>,
}

#[derive(Debug, PartialEq)]
struct VisualSampleKey {
    motion: Vec<[u32; 4]>,
    states: Vec<Value>,
    video_frame: Option<u64>,
    deployment_queue: Option<DeploymentQueueVisualKey>,
    ambient_time: Option<u64>,
}

impl PreparedPlan {
    fn playback(&self, reduced_motion: bool) -> Result<kinograph::playback::Playback> {
        let defaults = self
            .attachments
            .iter()
            .map(|attachment| (attachment.weight.clone(), attachment.default_profile))
            .collect();
        kinograph::playback::Playback::with_default_profiles(
            &self.plan,
            &self.timeline,
            reduced_motion,
            &defaults,
        )
    }
    #[cfg(test)]
    fn compile(plan: ScenePlan, base: &Path) -> Result<Self> {
        Self::compile_with_targets(plan, base, &HashMap::new(), &HashSet::new())
    }

    fn prepare(mut plan: ScenePlan, base: &Path, renderer: &mut HeadlessRenderer) -> Result<Self> {
        plan.validate()?;
        validate_root_recipes(&plan)?;
        let mut editors = plan
            .actors
            .iter()
            .filter(|actor| actor.recipe == EDITOR_RECIPE)
            .map(PreparedEditor::new)
            .collect::<Result<Vec<_>>>()?;
        compile_editor_channels(&mut plan, &editors)?;
        validate_task_channels(&plan)?;
        let tasks = plan
            .actors
            .iter()
            .filter(|actor| actor.recipe == TASK_RECIPE)
            .map(|actor| task::PreparedTask::new(actor, plan.duration_nanos, renderer))
            .collect::<Result<Vec<_>>>()?;
        for task in &tasks {
            for channel in task.channels() {
                if let Some(existing) = plan.continuous_channels.iter().find(|existing| {
                    existing.id == channel.id
                        || (existing.actor_id == channel.actor_id
                            && existing.property == channel.property)
                }) {
                    if existing.actor_id == channel.actor_id
                        && existing.property == channel.property
                        && (channel.property == "x" || channel.property == "y")
                    {
                        continue;
                    }
                    bail!(
                        "authored channel collides with generated task channel '{}'",
                        channel.id
                    );
                }
                plan.continuous_channels.push(channel);
            }
        }
        plan.validate()?;
        let terminal = plan
            .actors
            .iter()
            .find(|actor| actor.recipe == TERMINAL_RECORDING_RECIPE)
            .map(|actor| PreparedTerminal::new(actor, &plan.media, base))
            .transpose()?;
        let deployment_queue = plan
            .actors
            .iter()
            .find(|actor| actor.recipe == DEPLOYMENT_QUEUE_RECIPE)
            .map(|actor| {
                PreparedDeploymentQueue::new(actor, &plan.state_channels, plan.duration_nanos)
            })
            .transpose()?;
        let mut pointers = HashMap::new();
        for actor in plan
            .actors
            .iter()
            .filter(|actor| actor.recipe == POINTER_RECIPE)
        {
            let recipe = serde_json::from_value::<PointerRecipePlan>(actor.data.clone())
                .with_context(|| format!("parse pointer recipe for actor '{}'", actor.id))?;
            if !editors
                .iter()
                .any(|editor| editor.actor_id() == recipe.editor_id)
            {
                bail!(
                    "pointer actor '{}' references unknown editor '{}'",
                    actor.id,
                    recipe.editor_id
                );
            }
            if pointers
                .insert(recipe.editor_id.clone(), actor.id.clone())
                .is_some()
            {
                bail!(
                    "editor '{}' has more than one pointer actor",
                    recipe.editor_id
                );
            }
        }
        let mut targets = HashMap::new();
        for target in &plan.semantic_targets {
            let editor = editors
                .iter_mut()
                .find(|editor| editor.actor_id() == target.actor_id)
                .with_context(|| {
                    format!(
                        "actor '{}' recipe did not resolve semantic target '{}'",
                        target.actor_id, target.id
                    )
                })?;
            targets.insert(target.id.clone(), editor.resolve_target(renderer, target)?);
        }
        let visual_media = terminal
            .iter()
            .flat_map(PreparedTerminal::media_ids)
            .collect::<HashSet<_>>();
        let scales = plan
            .semantic_targets
            .iter()
            .map(|target| {
                let scale = editors
                    .iter()
                    .find_map(|editor| editor.target_scale(&target.id))
                    .expect("resolved target");
                (target.id.clone(), scale)
            })
            .collect();
        let attachments = attachments::compile(&mut plan, &targets, &scales)?;
        let mut prepared = Self::compile_with_targets(plan, base, &targets, &visual_media)?;
        prepared.editors = editors;
        prepared.pointers = pointers;
        prepared.terminal = terminal;
        prepared.deployment_queue = deployment_queue;
        prepared.attachments = attachments;
        prepared.tasks = tasks;
        Ok(prepared)
    }

    fn compile_with_targets(
        plan: ScenePlan,
        base: &Path,
        targets: &HashMap<String, TargetGeometry>,
        visual_media: &HashSet<&str>,
    ) -> Result<Self> {
        let mut initial_values = Vec::new();
        let mut events = Vec::new();
        let mut properties = HashMap::new();
        for channel in &plan.continuous_channels {
            let property = PropertyId::new(channel.id.clone());
            initial_values.push((property.clone(), resolve_scalar(&channel.initial, targets)?));
            properties.insert(channel.id.clone(), property.clone());
            for event in &channel.events {
                events.push(match event {
                    TrackEventPlan::Set { at_nanos, value } => TimedEvent::set(
                        seconds_f64(*at_nanos),
                        property.clone(),
                        resolve_scalar(value, targets)?,
                    ),
                    TrackEventPlan::Spring {
                        at_nanos,
                        target,
                        response_seconds,
                        damping_ratio,
                        position_threshold,
                        velocity_threshold,
                    } => TimedEvent::spring(
                        seconds_f64(*at_nanos),
                        property.clone(),
                        resolve_scalar(target, targets)?,
                        SpringProfile::new(
                            *response_seconds,
                            *damping_ratio,
                            *position_threshold,
                            *velocity_threshold,
                        ),
                    ),
                });
            }
        }
        let timeline =
            Timeline::compile_events(initial_values, events, seconds_f64(plan.duration_nanos))?;
        let state_tracks = plan
            .state_channels
            .iter()
            .map(|channel| {
                Ok((
                    channel.id.clone(),
                    StateTrack::compile(
                        channel.initial.clone(),
                        channel.events.iter().map(|event| {
                            TimedState::new(seconds_f64(event.at_nanos), event.value.clone())
                        }),
                        seconds_f64(plan.duration_nanos),
                    )?,
                ))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        let mut composition = vec![Composition::hold(Duration::from_nanos(plan.duration_nanos))];
        for media in &plan.media {
            if !matches!(media.kind, MediaKindPlan::Audio) {
                if matches!(media.kind, MediaKindPlan::Video)
                    && visual_media.contains(media.id.as_str())
                {
                    continue;
                }
                bail!(
                    "plan renderer has no actor consuming {:?} media '{}'",
                    media.kind,
                    media.id
                );
            }
            let path = resolve_media_path(base, media);
            let asset = Asset::audio(media.id.clone(), path);
            let clip = asset
                .clip(TimeRange::new(
                    Time::from_nanos(media.source_start_nanos),
                    Time::from_nanos(media.source_end_nanos),
                ))
                .gain_db(media.gain_db);
            let placement = match media.role {
                MediaRolePlan::Script => Composition::script(clip),
                MediaRolePlan::Layer => Composition::layer(clip),
            };
            composition.push(Composition::delay(
                Duration::from_nanos(media.timeline_start_nanos),
                placement,
            ));
        }
        let scene = Scene::new(
            Vec::<(PropertyId, Scalar)>::new(),
            Composition::parallel(composition),
        )
        .compile(&HashMap::new())?;
        Ok(Self {
            plan,
            timeline,
            properties,
            state_tracks,
            scene,
            editors: Vec::new(),
            pointers: HashMap::new(),
            terminal: None,
            deployment_queue: None,
            attachments: Vec::new(),
            tasks: Vec::new(),
        })
    }

    fn file_name(&self) -> &str {
        self.editors
            .first()
            .map(PreparedEditor::file_name)
            .or_else(|| self.terminal.as_ref().map(PreparedTerminal::file_name))
            .or_else(|| {
                self.deployment_queue
                    .as_ref()
                    .map(PreparedDeploymentQueue::file_name)
            })
            .unwrap_or(self.plan.id.as_str())
    }

    fn visual_sample_key(&self, time: f64) -> Result<VisualSampleKey> {
        self.visual_sample_key_using(time, &self.timeline)
    }

    fn visual_sample_key_using(&self, time: f64, timeline: &Timeline) -> Result<VisualSampleKey> {
        let previous_time = (time - 1.0 / 240.0).max(0.0);
        let motion = self
            .plan
            .continuous_channels
            .iter()
            .map(|channel| {
                let property = self
                    .properties
                    .get(&channel.id)
                    .with_context(|| format!("missing compiled property '{}'", channel.id))?;
                let current = timeline
                    .sample_at(property, time)
                    .with_context(|| format!("sample property '{}' at {time}", channel.id))?;
                let previous = timeline
                    .sample_at(property, previous_time)
                    .with_context(|| {
                        format!("sample property '{}' at {previous_time}", channel.id)
                    })?;
                Ok([
                    current.position.to_bits(),
                    current.velocity.to_bits(),
                    previous.position.to_bits(),
                    previous.velocity.to_bits(),
                ])
            })
            .collect::<Result<Vec<_>>>()?;
        let states = self
            .plan
            .state_channels
            .iter()
            .map(|channel| {
                self.state_tracks
                    .get(&channel.id)
                    .with_context(|| format!("missing compiled state track '{}'", channel.id))
                    .map(|track| track.sample_at(time).current.clone())
            })
            .collect::<Result<Vec<_>>>()?;
        let video_frame = self
            .terminal
            .as_ref()
            .map(|terminal| {
                let recording = self
                    .state_value(terminal.actor_id(), "recording", time)
                    .and_then(Value::as_str)
                    .context("terminal-recording actor requires string recording state")?;
                terminal.frame_index_at(recording, time)
            })
            .transpose()?;
        let deployment_queue = self
            .deployment_queue
            .as_ref()
            .map(|deployment| deployment.visual_key(time));
        Ok(VisualSampleKey {
            motion,
            states,
            video_frame,
            deployment_queue,
            ambient_time: self
                .running_properties()
                .iter()
                .any(|property| {
                    timeline
                        .sample_at(property, time)
                        .is_some_and(|state| state.position > 0.001)
                })
                .then_some(time.to_bits()),
        })
    }

    fn running_properties(&self) -> Vec<PropertyId> {
        self.tasks
            .iter()
            .filter_map(task::PreparedTask::running_property)
            .map(PropertyId::new)
            .collect()
    }

    fn render_sample(&self, renderer: &mut HeadlessRenderer, time: f64) -> Result<Vec<u8>> {
        self.render_sample_using(renderer, time, &self.timeline)
    }

    fn render_sample_using(
        &self,
        renderer: &mut HeadlessRenderer,
        time: f64,
        timeline: &Timeline,
    ) -> Result<Vec<u8>> {
        let title_card = self
            .plan
            .actors
            .iter()
            .find(|actor| actor.recipe == "title-card");
        let mut pixels = if let Some(editor) = self.editors.first() {
            editor.render(
                renderer,
                time,
                self.pointers.get(editor.actor_id()).map(String::as_str),
                |actor, property, at| self.motion_value(timeline, actor, property, at),
            )?
        } else if let Some(terminal) = &self.terminal {
            let recording = self
                .state_value(terminal.actor_id(), "recording", time)
                .and_then(Value::as_str)
                .context("terminal-recording actor requires string recording state")?;
            terminal.render(renderer, recording, time, |property, default| {
                self.property_value(timeline, terminal.actor_id(), property, time, default)
            })?
        } else if let Some(deployment) = &self.deployment_queue {
            deployment.render(renderer, time, |property, default| {
                self.property_value(timeline, deployment.actor_id(), property, time, default)
            })?
        } else if let Some(actor) = title_card {
            let title = actor
                .data
                .get("title")
                .and_then(serde_json::Value::as_str)
                .context("title-card actor requires string data.title")?;
            let subtitle = self
                .state_value(&actor.id, "subtitle", time)
                .or_else(|| actor.data.get("subtitle"))
                .and_then(serde_json::Value::as_str);
            renderer.render_title_card(
                title,
                subtitle,
                self.property_value(timeline, &actor.id, "opacity", time, 1.0)
                    .clamp(0.0, 1.0),
            )
        } else {
            renderer.render_title_card("", None, 0.0)
        };
        for actor in &self.plan.actors {
            match actor.recipe.as_str() {
                "title-card"
                | EDITOR_RECIPE
                | POINTER_RECIPE
                | TASK_RECIPE
                | TERMINAL_RECORDING_RECIPE
                | DEPLOYMENT_QUEUE_RECIPE => {}
                "text" => {
                    let text = self
                        .state_value(&actor.id, "content", time)
                        .or_else(|| actor.data.get("text"))
                        .and_then(serde_json::Value::as_str)
                        .context("text actor requires string data.text or content state")?;
                    let center = actor
                        .data
                        .get("center")
                        .and_then(serde_json::Value::as_array)
                        .filter(|values| values.len() == 2)
                        .context("text actor requires data.center [x, y]")?;
                    let x = self.property_value(
                        timeline,
                        &actor.id,
                        "x",
                        time,
                        center[0]
                            .as_f64()
                            .context("text center x must be numeric")?
                            as f32,
                    );
                    let y = self.property_value(
                        timeline,
                        &actor.id,
                        "y",
                        time,
                        center[1]
                            .as_f64()
                            .context("text center y must be numeric")?
                            as f32,
                    );
                    let font_size = actor
                        .data
                        .get("fontSize")
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(28.0) as f32;
                    let color = actor
                        .data
                        .get("color")
                        .and_then(serde_json::Value::as_array)
                        .filter(|values| values.len() == 3)
                        .map(|values| {
                            [
                                values[0].as_u64().unwrap_or(255) as u8,
                                values[1].as_u64().unwrap_or(255) as u8,
                                values[2].as_u64().unwrap_or(255) as u8,
                            ]
                        })
                        .unwrap_or([255, 255, 255]);
                    renderer.composite_centered_text(
                        &mut pixels,
                        text,
                        [x, y],
                        font_size,
                        color,
                        self.property_value(timeline, &actor.id, "opacity", time, 1.0)
                            .clamp(0.0, 1.0),
                    );
                }
                recipe => bail!("unsupported actor recipe '{recipe}'"),
            }
        }
        for task in &self.tasks {
            task.render(&mut pixels, renderer, time, |actor, property| {
                self.motion_value(timeline, actor, property, time)
            })?;
        }
        Ok(pixels)
    }

    fn property_value(
        &self,
        timeline: &Timeline,
        actor_id: &str,
        property_name: &str,
        time: f64,
        default: f32,
    ) -> f32 {
        self.motion_value(timeline, actor_id, property_name, time)
            .map_or(default, |state| state.position)
    }

    fn motion_value(
        &self,
        timeline: &Timeline,
        actor_id: &str,
        property_name: &str,
        time: f64,
    ) -> Option<kinograph::motion::MotionState> {
        let channel =
            self.plan.continuous_channels.iter().find(|channel| {
                channel.actor_id == actor_id && channel.property == property_name
            })?;
        let mut state = timeline.sample_at(self.properties.get(&channel.id)?, time)?;
        for attachment in self
            .attachments
            .iter()
            .filter(|attachment| attachment.channel == channel.id)
        {
            let weight = timeline.sample_at(self.properties.get(&attachment.weight)?, time)?;
            if weight.position == 0.0 && weight.velocity == 0.0 {
                continue;
            }
            let geometry = self.editors.iter().find_map(|editor| {
                editor.target_motion(&attachment.target, |actor, property| {
                    self.raw_motion_value(timeline, actor, property, time)
                })
            })?;
            let target = match attachment.component {
                TargetComponentPlan::X => geometry.x,
                TargetComponentPlan::Width => geometry.width,
                TargetComponentPlan::LineY => geometry.line_y,
                TargetComponentPlan::CenterX => kinograph::motion::MotionState {
                    position: geometry.x.position + geometry.width.position * 0.5,
                    velocity: geometry.x.velocity + geometry.width.velocity * 0.5,
                },
            };
            let offset = target.position - attachment.base;
            state.position += weight.position * offset;
            state.velocity += weight.velocity * offset + weight.position * target.velocity;
        }
        Some(state)
    }

    fn raw_motion_value(
        &self,
        timeline: &Timeline,
        actor_id: &str,
        property_name: &str,
        time: f64,
    ) -> Option<kinograph::motion::MotionState> {
        self.plan
            .continuous_channels
            .iter()
            .find(|channel| channel.actor_id == actor_id && channel.property == property_name)
            .and_then(|channel| self.properties.get(&channel.id))
            .and_then(|property| timeline.sample_at(property, time))
    }

    fn state_value(&self, actor_id: &str, state_name: &str, time: f64) -> Option<&Value> {
        self.plan
            .state_channels
            .iter()
            .find(|channel| channel.actor_id == actor_id && channel.state == state_name)
            .and_then(|channel| self.state_tracks.get(&channel.id))
            .map(|track| track.sample_at(time).current)
    }
}

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "kebab-case")]
enum ServerRequest {
    Schema,
    Validate {
        plan: PathBuf,
    },
    Inspect {
        plan: PathBuf,
    },
    Steps {
        plan: PathBuf,
    },
    Diff {
        before: PathBuf,
        after: PathBuf,
    },
    Frame {
        plan: PathBuf,
        output: PathBuf,
        at_nanos: u64,
    },
    Render {
        plan: PathBuf,
        output: PathBuf,
        #[serde(default)]
        cue: Option<String>,
        #[serde(default)]
        start_nanos: Option<u64>,
        #[serde(default)]
        end_nanos: Option<u64>,
    },
    Shutdown,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerResponse {
    id: Option<Value>,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

async fn serve() -> Result<()> {
    let mut renderer = HeadlessRenderer::new(RenderSpec {
        width: WIDTH,
        height: HEIGHT,
        font_path: PathBuf::from(FONT_PATH),
        file_name: "scene-plan".to_owned(),
    })
    .await?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let envelope = serde_json::from_str::<Value>(&line);
        let id = envelope
            .as_ref()
            .ok()
            .and_then(|value| value.get("id"))
            .cloned();
        let request = envelope.and_then(serde_json::from_value::<ServerRequest>);
        let shutdown = matches!(&request, Ok(ServerRequest::Shutdown));
        let response = match request {
            Ok(request) => match handle_server_request(request, &mut renderer) {
                Ok(result) => ServerResponse {
                    id,
                    ok: true,
                    result: Some(result),
                    error: None,
                },
                Err(error) => ServerResponse {
                    id,
                    ok: false,
                    result: None,
                    error: Some(format!("{error:#}")),
                },
            },
            Err(error) => ServerResponse {
                id,
                ok: false,
                result: None,
                error: Some(format!("parse request: {error}")),
            },
        };
        serde_json::to_writer(&mut stdout, &response)?;
        writeln!(stdout)?;
        stdout.flush()?;
        if shutdown {
            break;
        }
    }
    Ok(())
}

fn handle_server_request(request: ServerRequest, renderer: &mut HeadlessRenderer) -> Result<Value> {
    match request {
        ServerRequest::Schema => Ok(ScenePlan::schema()),
        ServerRequest::Validate { plan } => {
            let plan = read_plan(&plan)?;
            validate_renderer_plan(&plan)?;
            Ok(json!({ "valid": true, "id": plan.id }))
        }
        ServerRequest::Inspect { plan } => {
            let plan = read_plan(&plan)?;
            Ok(inspect_plan(&plan))
        }
        ServerRequest::Steps { plan } => Ok(serde_json::to_value(
            kinograph::editor::inspect_steps(&read_plan(&plan)?)?,
        )?),
        ServerRequest::Diff { before, after } => {
            let before = read_plan(&before)?;
            let after = read_plan(&after)?;
            Ok(json!({ "changes": before.diff(&after)? }))
        }
        ServerRequest::Frame {
            plan,
            output,
            at_nanos,
        } => {
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            let scene_plan = read_plan(&plan)?;
            if at_nanos > scene_plan.duration_nanos {
                bail!("frame time exceeds scene duration");
            }
            let base = plan.parent().unwrap_or_else(|| Path::new("."));
            let prepared = PreparedPlan::prepare(scene_plan, base, renderer)?;
            delivery::render_frame(&prepared, renderer, &output, Time::from_nanos(at_nanos))?;
            Ok(json!({ "output": output, "atNanos": at_nanos }))
        }
        ServerRequest::Render {
            plan,
            output,
            cue,
            start_nanos,
            end_nanos,
        } => {
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            let scene_plan = read_plan(&plan)?;
            let window = server_window(&scene_plan, cue.as_deref(), start_nanos, end_nanos)?;
            let base = plan.parent().unwrap_or_else(|| Path::new("."));
            let prepared = PreparedPlan::prepare(scene_plan, base, renderer)?;
            delivery::render_video(&prepared, renderer, &output, window)?;
            Ok(json!({
                "output": output,
                "startNanos": window.start().as_nanos(),
                "endNanos": window.end().as_nanos(),
            }))
        }
        ServerRequest::Shutdown => Ok(json!({ "shutdown": true })),
    }
}

fn resolve_media_path(base: &Path, media: &kinograph::plan::MediaPlan) -> PathBuf {
    if media.path.is_absolute() {
        media.path.clone()
    } else {
        base.join(&media.path)
    }
}

fn inspect_plan(plan: &ScenePlan) -> Value {
    let mut cues = plan.cues.iter().collect::<Vec<_>>();
    cues.sort_by_key(|cue| (cue.start_nanos, cue.end_nanos, cue.id.as_str()));
    json!({
        "id": plan.id,
        "version": plan.version,
        "durationNanos": plan.duration_nanos,
        "presentationSteps": plan.presentation_steps,
        "actors": plan.actors.iter().map(|actor| &actor.id).collect::<Vec<_>>(),
        "semanticTargets": plan.semantic_targets.iter().map(|target| &target.id).collect::<Vec<_>>(),
        "continuousChannels": plan.continuous_channels.iter().map(|channel| &channel.id).collect::<Vec<_>>(),
        "stateChannels": plan.state_channels.iter().map(|channel| &channel.id).collect::<Vec<_>>(),
        "cues": cues,
        "media": plan.media.iter().map(|media| &media.id).collect::<Vec<_>>(),
    })
}

fn resolve_scalar(scalar: &ScalarPlan, targets: &HashMap<String, TargetGeometry>) -> Result<f32> {
    match scalar {
        ScalarPlan::Literal(value) => Ok(*value),
        ScalarPlan::Target(target) => {
            let geometry = targets.get(&target.target_id).with_context(|| {
                format!(
                    "semantic target '{}' was not resolved by its renderer recipe",
                    target.target_id
                )
            })?;
            let value = match target.component {
                TargetComponentPlan::X => geometry.x,
                TargetComponentPlan::Width => geometry.width,
                TargetComponentPlan::CenterX => geometry.center_x(),
                TargetComponentPlan::LineY => geometry.line_y,
            };
            Ok(value + target.offset)
        }
    }
}

fn validate_text_recipe(actor: &kinograph::plan::ActorPlan) -> Result<()> {
    let center = actor
        .data
        .get("center")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 2)
        .with_context(|| format!("text actor '{}' requires data.center [x, y]", actor.id))?;
    for (axis, value) in ["x", "y"].into_iter().zip(center) {
        let value = value
            .as_f64()
            .with_context(|| format!("text actor '{}' center {axis} must be numeric", actor.id))?;
        if !value.is_finite() || value < f64::from(f32::MIN) || value > f64::from(f32::MAX) {
            bail!(
                "text actor '{}' center {axis} must fit a finite f32",
                actor.id
            );
        }
    }
    if let Some(font_size) = actor.data.get("fontSize") {
        let font_size = font_size
            .as_f64()
            .with_context(|| format!("text actor '{}' fontSize must be numeric", actor.id))?;
        if !font_size.is_finite() || font_size <= 0.0 || font_size > 512.0 {
            bail!(
                "text actor '{}' fontSize must be finite and in (0, 512]",
                actor.id
            );
        }
    }
    if let Some(color) = actor.data.get("color") {
        let color = color
            .as_array()
            .filter(|values| values.len() == 3)
            .with_context(|| format!("text actor '{}' color must be [r, g, b]", actor.id))?;
        if color
            .iter()
            .any(|value| value.as_u64().is_none_or(|value| value > 255))
        {
            bail!("text actor '{}' color channels must be bytes", actor.id);
        }
    }
    Ok(())
}

fn validate_root_recipes(plan: &ScenePlan) -> Result<()> {
    let editor_count = plan
        .actors
        .iter()
        .filter(|actor| actor.recipe == EDITOR_RECIPE)
        .count();
    let title_card_count = plan
        .actors
        .iter()
        .filter(|actor| actor.recipe == "title-card")
        .count();
    let terminal_count = plan
        .actors
        .iter()
        .filter(|actor| actor.recipe == TERMINAL_RECORDING_RECIPE)
        .count();
    let deployment_queue_count = plan
        .actors
        .iter()
        .filter(|actor| actor.recipe == DEPLOYMENT_QUEUE_RECIPE)
        .count();
    if editor_count > 1 {
        bail!("plan renderer currently supports at most one editor root actor");
    }
    if title_card_count > 1 {
        bail!("plan renderer currently supports at most one title-card root actor");
    }
    if terminal_count > 1 {
        bail!("plan renderer currently supports at most one terminal-recording root actor");
    }
    if deployment_queue_count > 1 {
        bail!("plan renderer currently supports at most one deployment-queue root actor");
    }
    if editor_count + title_card_count + terminal_count + deployment_queue_count > 1 {
        bail!(
            "editor, title-card, terminal-recording, and deployment-queue actors are exclusive root recipes"
        );
    }
    for actor in plan.actors.iter().filter(|actor| actor.recipe == "text") {
        validate_text_recipe(actor)?;
    }
    Ok(())
}

fn compile_editor_channels(plan: &mut ScenePlan, editors: &[PreparedEditor]) -> Result<()> {
    if plan
        .continuous_channels
        .iter()
        .any(|channel| channel.property.starts_with("__attachment-"))
    {
        bail!("authored channels cannot use the reserved __attachment- namespace");
    }
    for editor in editors {
        for channel in editor.snapshot_channels(plan.duration_nanos)? {
            if plan.continuous_channels.iter().any(|existing| {
                existing.id == channel.id
                    || (existing.actor_id == channel.actor_id
                        && existing.property == channel.property)
            }) {
                bail!(
                    "authored channel collides with generated line channel '{}'",
                    channel.id
                );
            }
            plan.continuous_channels.push(channel);
        }
        let drivers = editor.geometry_channels();
        for channel in plan.continuous_channels.iter().filter(|channel| {
            channel.actor_id == editor.actor_id() && drivers.contains(&channel.property)
        }) {
            if std::iter::once(&channel.initial)
                .chain(channel.events.iter().map(|event| match event {
                    TrackEventPlan::Set { value, .. } => value,
                    TrackEventPlan::Spring { target, .. } => target,
                }))
                .any(|value| matches!(value, ScalarPlan::Target(_)))
            {
                bail!(
                    "editor geometry channel '{}' must use literal values, not a cyclic semantic attachment",
                    channel.id
                );
            }
        }
    }
    plan.validate()?;
    Ok(())
}

fn validate_task_channels(plan: &ScenePlan) -> Result<()> {
    for actor in plan
        .actors
        .iter()
        .filter(|actor| actor.recipe == TASK_RECIPE)
    {
        let recipe: kinograph::task::TaskRecipePlan = serde_json::from_value(actor.data.clone())?;
        recipe.validate(plan.duration_nanos)?;
        for property in recipe.channel_properties() {
            let id = format!("{}.{property}", actor.id);
            for channel in &plan.continuous_channels {
                let matching_property =
                    channel.actor_id == actor.id && channel.property == property;
                if (channel.id == id && !matching_property)
                    || (matching_property && property != "x" && property != "y")
                {
                    bail!("authored channel collides with generated task channel '{id}'");
                }
            }
        }
    }
    Ok(())
}

fn validate_renderer_plan(plan: &ScenePlan) -> Result<()> {
    validate_root_recipes(plan)?;
    let editors = plan
        .actors
        .iter()
        .filter(|actor| actor.recipe == EDITOR_RECIPE)
        .map(PreparedEditor::new)
        .collect::<Result<Vec<_>>>()?;
    compile_editor_channels(&mut plan.clone(), &editors)?;
    validate_task_channels(plan)?;
    for actor in &plan.actors {
        match actor.recipe.as_str() {
            TASK_RECIPE => {}
            "title-card" | "text" | EDITOR_RECIPE | POINTER_RECIPE | TERMINAL_RECORDING_RECIPE => {}
            DEPLOYMENT_QUEUE_RECIPE => {
                PreparedDeploymentQueue::new(actor, &plan.state_channels, plan.duration_nanos)?;
            }
            recipe => bail!("unsupported actor recipe '{recipe}'"),
        }
    }
    Ok(())
}

fn server_window(
    plan: &ScenePlan,
    cue: Option<&str>,
    start_nanos: Option<u64>,
    end_nanos: Option<u64>,
) -> Result<TimeRange> {
    match (cue, start_nanos, end_nanos) {
        (Some(id), None, None) => {
            let cue = plan
                .cues
                .iter()
                .find(|cue| cue.id == id)
                .with_context(|| format!("scene plan has no cue '{id}'"))?;
            Ok(TimeRange::new(
                Time::from_nanos(cue.start_nanos),
                Time::from_nanos(cue.end_nanos),
            ))
        }
        (None, Some(start), Some(end)) if start < end => Ok(TimeRange::new(
            Time::from_nanos(start),
            Time::from_nanos(end),
        )),
        (None, None, None) => Ok(TimeRange::new(
            Time::ZERO,
            Time::from_nanos(plan.duration_nanos),
        )),
        _ => bail!("provide either cue or both startNanos and endNanos"),
    }
}

fn seconds_f64(nanos: u64) -> f64 {
    nanos as f64 / 1_000_000_000.0
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use kinograph::{
        dsl::TargetGeometry,
        plan::{
            ActorPlan, ContinuousChannelPlan, MediaKindPlan, MediaPlan, MediaRolePlan, ScalarPlan,
            ScenePlan, SemanticTargetPlan, TargetComponentPlan, TargetScalarPlan, TrackEventPlan,
        },
    };
    use serde_json::json;

    use super::{
        BUILTIN_HERO_PLAN, PreparedPlan, parse_range, validate_renderer_plan, validate_root_recipes,
    };

    #[test]
    fn plan_channels_compile_through_the_shared_timeline() {
        let mut plan = ScenePlan::new("demo", 2_000_000_000);
        plan.actors.push(ActorPlan {
            id: "title".to_owned(),
            recipe: "title-card".to_owned(),
            data: json!({ "title": "Hello" }),
        });
        plan.continuous_channels.push(ContinuousChannelPlan {
            id: "title.opacity".to_owned(),
            actor_id: "title".to_owned(),
            property: "opacity".to_owned(),
            initial: 0.0.into(),
            events: vec![TrackEventPlan::Set {
                at_nanos: 1_000_000_000,
                value: 1.0.into(),
            }],
        });
        plan.validate().unwrap();

        let prepared = PreparedPlan::compile(plan, std::path::Path::new(".")).unwrap();
        let property = &prepared.properties["title.opacity"];
        assert_eq!(
            prepared.timeline.sample(property, 0.5).unwrap().position,
            0.0
        );
        assert_eq!(
            prepared.timeline.sample(property, 1.0).unwrap().position,
            1.0
        );
    }

    #[test]
    fn visual_keys_merge_settled_samples_but_preserve_motion() {
        let mut plan = ScenePlan::new("demo", 2_000_000_000);
        plan.actors.push(ActorPlan {
            id: "title".to_owned(),
            recipe: "title-card".to_owned(),
            data: json!({ "title": "Hello" }),
        });
        plan.continuous_channels.push(ContinuousChannelPlan {
            id: "title.opacity".to_owned(),
            actor_id: "title".to_owned(),
            property: "opacity".to_owned(),
            initial: 0.0.into(),
            events: vec![TrackEventPlan::Spring {
                at_nanos: 0,
                target: 1.0.into(),
                response_seconds: 0.4,
                damping_ratio: 1.0,
                position_threshold: 0.001,
                velocity_threshold: 0.001,
            }],
        });
        plan.validate().unwrap();
        let prepared = PreparedPlan::compile(plan, std::path::Path::new(".")).unwrap();

        assert_ne!(
            prepared.visual_sample_key(0.1).unwrap(),
            prepared.visual_sample_key(0.11).unwrap()
        );
        assert_eq!(
            prepared.visual_sample_key(1.5).unwrap(),
            prepared.visual_sample_key(1.51).unwrap()
        );
    }

    #[test]
    fn semantic_target_scalars_resolve_before_timeline_compilation() {
        let mut plan = ScenePlan::new("demo", 1_000_000_000);
        plan.actors.push(ActorPlan {
            id: "editor".to_owned(),
            recipe: "editor".to_owned(),
            data: json!({}),
        });
        plan.semantic_targets.push(SemanticTargetPlan {
            id: "token".to_owned(),
            actor_id: "editor".to_owned(),
            selector: json!({}),
        });
        plan.continuous_channels.push(ContinuousChannelPlan {
            id: "editor.x".to_owned(),
            actor_id: "editor".to_owned(),
            property: "x".to_owned(),
            initial: ScalarPlan::Target(TargetScalarPlan {
                target_id: "token".to_owned(),
                component: TargetComponentPlan::CenterX,
                offset: 40.0,
            }),
            events: vec![],
        });
        plan.validate().unwrap();
        let targets = HashMap::from([(
            "token".to_owned(),
            TargetGeometry {
                x: 100.0,
                width: 20.0,
                line_y: 200.0,
            },
        )]);

        let prepared = PreparedPlan::compile_with_targets(
            plan,
            std::path::Path::new("."),
            &targets,
            &HashSet::new(),
        )
        .unwrap();
        assert_eq!(
            prepared
                .timeline
                .sample(&prepared.properties["editor.x"], 0.0)
                .unwrap()
                .position,
            150.0
        );
    }

    #[test]
    fn unsupported_timed_media_returns_an_error_instead_of_panicking() {
        let mut plan = ScenePlan::new("demo", 2_000_000_000);
        plan.media.push(MediaPlan {
            id: "image".to_owned(),
            path: "image.png".into(),
            kind: MediaKindPlan::Image,
            role: MediaRolePlan::Layer,
            source_start_nanos: 0,
            source_end_nanos: 1_000_000_000,
            timeline_start_nanos: 0,
            timeline_end_nanos: 1_000_000_000,
            gain_db: 0.0,
        });
        plan.validate().unwrap();

        let error = PreparedPlan::compile(plan, std::path::Path::new("."))
            .err()
            .unwrap();
        assert!(
            error
                .to_string()
                .contains("no actor consuming Image media 'image'")
        );
    }

    #[test]
    fn invalid_cli_ranges_return_errors() {
        assert!(parse_range("NaN..1").is_err());
        assert!(parse_range("-1..1").is_err());
        assert!(parse_range("2..1").is_err());
    }

    #[test]
    fn renderer_default_tests_guard_the_canonical_hero_plan() {
        assert_eq!(
            kinograph_hero::build_plan()
                .unwrap()
                .to_json_pretty()
                .unwrap(),
            BUILTIN_HERO_PLAN
        );
    }

    #[test]
    fn invalid_text_recipes_and_duplicate_roots_are_rejected_during_preparation() {
        let mut plan = ScenePlan::new("demo", 1_000_000_000);
        plan.actors.push(ActorPlan {
            id: "text".to_owned(),
            recipe: "text".to_owned(),
            data: json!({ "text": "Hello", "center": [960, 540], "fontSize": -1 }),
        });
        assert!(validate_root_recipes(&plan).is_err());

        plan.actors = vec![
            ActorPlan {
                id: "first".to_owned(),
                recipe: "title-card".to_owned(),
                data: json!({ "title": "First" }),
            },
            ActorPlan {
                id: "second".to_owned(),
                recipe: "title-card".to_owned(),
                data: json!({ "title": "Second" }),
            },
        ];
        assert!(validate_root_recipes(&plan).is_err());
    }

    #[test]
    fn concrete_validation_rejects_unpreparable_and_unknown_recipes() {
        let mut plan = ScenePlan::new("demo", 1_000_000_000);
        plan.actors.push(ActorPlan {
            id: "deployments".to_owned(),
            recipe: "deployment-queue".to_owned(),
            data: json!({
                "product": "NORTHSTAR",
                "title": "Release Control",
                "subtitle": "Live deployment telemetry",
                "environment": "PRODUCTION",
                "release": "release-2026.07.16",
                "items": []
            }),
        });
        assert!(validate_renderer_plan(&plan).is_err());

        plan.actors[0].recipe = "unknown".to_owned();
        assert!(validate_renderer_plan(&plan).is_err());
    }

    #[test]
    fn media_ranges_remain_exact_integer_nanoseconds_during_preparation() {
        let mut plan = ScenePlan::new("long", u64::MAX);
        plan.media.push(MediaPlan {
            id: "audio".to_owned(),
            path: "audio.wav".into(),
            kind: MediaKindPlan::Audio,
            role: MediaRolePlan::Layer,
            source_start_nanos: u64::MAX - 1,
            source_end_nanos: u64::MAX,
            timeline_start_nanos: 0,
            timeline_end_nanos: 1,
            gain_db: 0.0,
        });
        plan.validate().unwrap();

        let prepared = PreparedPlan::compile(plan, std::path::Path::new(".")).unwrap();
        let source = prepared.scene.media()[0].clip().source_range();
        assert_eq!(source.start().as_nanos(), u64::MAX - 1);
        assert_eq!(source.end().as_nanos(), u64::MAX);
    }
}
