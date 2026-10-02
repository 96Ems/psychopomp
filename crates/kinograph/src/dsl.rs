use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::code::{CodeTransition, PlacedLine, TransitionProgress};
use crate::composition::{Composition, Duration, MediaPlacement, Time, TimeRange};
use crate::state::{StateTrack, TimedState};
use crate::timeline::{Animation, PropertyId, SpringProfile, Timeline};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TextTarget {
    pub line_id: String,
    pub text: String,
}

impl TextTarget {
    pub fn new(line_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            line_id: line_id.into(),
            text: text.into(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TargetGeometry {
    pub x: f32,
    pub width: f32,
    pub line_y: f32,
}

impl TargetGeometry {
    pub fn center_x(self) -> f32 {
        self.x + self.width * 0.5
    }

    pub fn below(self, offset: f32) -> f32 {
        self.line_y + offset
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AnnotationEffect {
    #[default]
    PrismaticBloom,
    FocusPulse,
    DangerPulse,
    SadPulse,
}

#[derive(Clone, Debug)]
pub struct Annotation {
    target: TextTarget,
    effect: AnnotationEffect,
    duration: Duration,
}

impl Annotation {
    pub fn on(target: TextTarget) -> Self {
        Self {
            target,
            effect: AnnotationEffect::default(),
            duration: Duration::milliseconds(900.0),
        }
    }

    pub fn effect(mut self, effect: AnnotationEffect) -> Self {
        self.effect = effect;
        self
    }

    pub(crate) fn target(&self) -> &TextTarget {
        &self.target
    }

    pub(crate) fn selected_effect(&self) -> AnnotationEffect {
        self.effect
    }

    pub(crate) fn duration(&self) -> Duration {
        self.duration
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AnnotationFrame {
    pub target: TargetGeometry,
    pub effect: AnnotationEffect,
    pub phase: f32,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TaskId(String);

impl TaskId {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        assert!(!value.is_empty(), "task ID must not be empty");
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct Task {
    id: TaskId,
    name: String,
    x: f32,
    y: f32,
    result_width: Option<f32>,
}

impl Task {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        let id = id.into();
        assert!(!id.is_empty(), "task ID must not be empty");
        Self {
            id: TaskId::new(id),
            name: name.into(),
            x: 0.0,
            y: 0.0,
            result_width: None,
        }
    }

    pub fn at(mut self, x: f32, y: f32) -> Self {
        assert!(
            x.is_finite() && y.is_finite(),
            "task position must be finite"
        );
        self.x = x;
        self.y = y;
        self
    }

    pub fn move_to(&self, x: f32, y: f32) -> TaskPoseChange {
        assert!(
            x.is_finite() && y.is_finite(),
            "task position must be finite"
        );
        TaskPoseChange {
            id: self.id.clone(),
            x,
            y,
        }
    }

    pub fn with_result_width(mut self, width: f32) -> Self {
        assert!(
            width.is_finite() && width > 0.0,
            "task result width must be positive"
        );
        self.result_width = Some(width);
        self
    }

    pub fn idle(&self) -> TaskChange {
        self.change(TaskState::Idle)
    }

    pub fn run(&self) -> TaskChange {
        self.change(TaskState::Running)
    }

    pub fn succeed(&self, result: impl Into<String>) -> TaskChange {
        self.change(TaskState::Succeeded(Some(result.into())))
    }

    pub fn complete(&self) -> TaskChange {
        self.change(TaskState::Succeeded(None))
    }

    pub fn fail(&self, error: impl Into<String>) -> TaskChange {
        self.change(TaskState::Failed(error.into()))
    }

    pub fn die(&self, defect: impl Into<String>) -> TaskChange {
        self.change(TaskState::Death(defect.into()))
    }

    pub fn hide(&self) -> TaskChange {
        self.change(TaskState::Hidden)
    }

    fn change(&self, state: TaskState) -> TaskChange {
        TaskChange {
            task: self.clone(),
            state,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum TaskState {
    Hidden,
    Idle,
    Running,
    Succeeded(Option<String>),
    Failed(String),
    Death(String),
}

#[derive(Clone, Debug)]
pub struct TaskChange {
    task: Task,
    state: TaskState,
}

#[derive(Clone, Debug)]
pub struct TaskPoseChange {
    id: TaskId,
    x: f32,
    y: f32,
}

impl TaskPoseChange {
    pub(crate) fn id(&self) -> &TaskId {
        &self.id
    }

    pub(crate) fn position(&self) -> [f32; 2] {
        [self.x, self.y]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TaskFrame<'a> {
    pub id: &'a TaskId,
    pub x: f32,
    pub y: f32,
    pub name: &'a str,
    pub result_width: Option<f32>,
    pub previous_state: &'a TaskState,
    pub previous_state_duration: f32,
    pub state: &'a TaskState,
    pub state_age: f32,
    pub visible_age: f32,
}

#[derive(Clone, Debug)]
pub enum Scalar {
    Literal(f32),
    TargetX(TextTarget),
    TargetWidth(TextTarget),
    TargetCenterX(TextTarget),
    TargetLineY(TextTarget),
    TargetBelow { target: TextTarget, offset: f32 },
    Offset { value: Box<Scalar>, amount: f32 },
}

impl From<f32> for Scalar {
    fn from(value: f32) -> Self {
        Self::Literal(value)
    }
}

impl Scalar {
    pub fn offset(self, amount: f32) -> Self {
        Self::Offset {
            value: Box::new(self),
            amount,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Motion {
    Spring {
        property: PropertyId,
        target: Scalar,
        profile: SpringProfile,
    },
    Parallel(Vec<Motion>),
    Delay {
        seconds: f32,
        motion: Box<Motion>,
    },
    Hold(f32),
}

impl Motion {
    pub fn spring(property: PropertyId, target: impl Into<Scalar>, profile: SpringProfile) -> Self {
        Self::Spring {
            property,
            target: target.into(),
            profile,
        }
    }

    pub fn parallel(motions: impl IntoIterator<Item = Motion>) -> Self {
        Self::Parallel(motions.into_iter().collect())
    }

    pub fn delay(seconds: f32, motion: Motion) -> Self {
        Self::Delay {
            seconds,
            motion: Box::new(motion),
        }
    }

    pub fn hold(seconds: f32) -> Self {
        Self::Hold(seconds)
    }

    pub fn duration(&self) -> f32 {
        match self {
            Self::Spring { profile, .. } => profile.advance_time(),
            Self::Parallel(motions) => motions.iter().map(Self::duration).fold(0.0, f32::max),
            Self::Delay { seconds, motion } => seconds + motion.duration(),
            Self::Hold(seconds) => *seconds,
        }
    }

    fn resolve(&self, targets: &HashMap<TextTarget, TargetGeometry>) -> Result<Animation> {
        Ok(match self {
            Self::Spring {
                property,
                target,
                profile,
            } => Animation::spring(property.clone(), resolve_scalar(target, targets)?, *profile),
            Self::Parallel(motions) => Animation::parallel(
                motions
                    .iter()
                    .map(|motion| motion.resolve(targets))
                    .collect::<Result<Vec<_>>>()?,
            ),
            Self::Delay { seconds, motion } => Animation::delay(*seconds, motion.resolve(targets)?),
            Self::Hold(seconds) => Animation::hold(*seconds),
        })
    }
}

pub struct Scene {
    initial_values: Vec<(PropertyId, Scalar)>,
    composition: Composition,
}

impl Scene {
    pub fn new(
        initial_values: impl IntoIterator<Item = (PropertyId, Scalar)>,
        composition: impl Into<Composition>,
    ) -> Self {
        Self {
            initial_values: initial_values.into_iter().collect(),
            composition: composition.into(),
        }
    }

    pub fn compile(&self, targets: &HashMap<TextTarget, TargetGeometry>) -> Result<CompiledScene> {
        let initial_values = self
            .initial_values
            .iter()
            .map(|(property, value)| Ok((property.clone(), resolve_scalar(value, targets)?)))
            .collect::<Result<Vec<_>>>()?;
        let lowered = self.composition.lower()?;
        let (tasks, task_pose_timeline) =
            compile_task_actors(lowered.tasks, lowered.task_poses, lowered.duration)?;
        let timeline = Timeline::compile(initial_values, &lowered.motion.resolve(targets)?)?;
        Ok(CompiledScene {
            timeline,
            media: lowered.media,
            duration: lowered.duration,
            tasks,
            task_pose_timeline,
            annotations: lowered
                .annotations
                .into_iter()
                .map(|(start, annotation)| {
                    Ok(CompiledAnnotation {
                        target: resolve_target(annotation.target(), targets)?,
                        effect: annotation.selected_effect(),
                        range: TimeRange::new(start, start.after(annotation.duration())),
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        })
    }
}

fn compile_task_actors(
    changes: Vec<(Time, TaskChange)>,
    pose_changes: Vec<(Time, TaskPoseChange)>,
    duration: Duration,
) -> Result<(Vec<CompiledTaskActor>, Timeline)> {
    let mut changes = changes.into_iter().enumerate().collect::<Vec<_>>();
    changes.sort_by(|(left_index, (left, _)), (right_index, (right, _))| {
        left.cmp(right).then(left_index.cmp(right_index))
    });
    let mut ids = Vec::<TaskId>::new();
    for (_, (_, change)) in &changes {
        if !ids.contains(&change.task.id) {
            ids.push(change.task.id.clone());
        }
    }

    let duration_seconds = duration.as_seconds() as f32;
    let mut actors = Vec::with_capacity(ids.len());
    let mut pose_properties = HashMap::new();
    let mut pose_initial_values = Vec::new();
    for id in ids {
        let actor_changes = changes
            .iter()
            .filter(|(_, (_, change))| change.task.id == id)
            .map(|(_, (at, change))| (*at, change))
            .collect::<Vec<_>>();
        let (first_at, first) = actor_changes[0];
        for (_, change) in &actor_changes[1..] {
            if change.task.name != first.task.name
                || change.task.x != first.task.x
                || change.task.y != first.task.y
                || change.task.result_width != first.task.result_width
            {
                anyhow::bail!(
                    "task '{}' is scheduled with conflicting actor metadata",
                    id.as_str()
                );
            }
        }
        let starts_at = first_at.as_seconds() as f32;
        let states = StateTrack::compile_at(
            f64::from(starts_at),
            first.state.clone(),
            actor_changes
                .iter()
                .skip(1)
                .map(|(at, change)| TimedState::new(at.as_seconds(), change.state.clone())),
            f64::from(duration_seconds),
        )?;
        let x_property = PropertyId::new(format!("task.{}.x", id.as_str()));
        let y_property = PropertyId::new(format!("task.{}.y", id.as_str()));
        pose_initial_values.extend([
            (x_property.clone(), first.task.x),
            (y_property.clone(), first.task.y),
        ]);
        pose_properties.insert(id.clone(), (x_property.clone(), y_property.clone()));
        actors.push(CompiledTaskActor {
            task: first.task.clone(),
            starts_at,
            states,
            x_property,
            y_property,
        });
    }

    let pose_profile = SpringProfile::new(0.379, 0.904, 0.01, 0.01);
    let pose_animation = Animation::parallel(
        pose_changes
            .into_iter()
            .map(|(at, pose)| {
                let (x, y) = pose_properties
                    .get(pose.id())
                    .with_context(|| {
                        format!("task pose references unknown task '{}'", pose.id().as_str())
                    })?
                    .clone();
                let [target_x, target_y] = pose.position();
                Ok(Animation::delay(
                    at.as_seconds() as f32,
                    Animation::parallel([
                        Animation::spring(x, target_x, pose_profile),
                        Animation::spring(y, target_y, pose_profile),
                    ]),
                ))
            })
            .chain(std::iter::once(Ok(Animation::hold(duration_seconds))))
            .collect::<Result<Vec<_>>>()?,
    );
    let pose_timeline = Timeline::compile(pose_initial_values, &pose_animation)?;
    Ok((actors, pose_timeline))
}

pub struct CompiledScene {
    timeline: Timeline,
    media: Vec<MediaPlacement>,
    duration: Duration,
    tasks: Vec<CompiledTaskActor>,
    task_pose_timeline: Timeline,
    annotations: Vec<CompiledAnnotation>,
}

struct CompiledTaskActor {
    task: Task,
    starts_at: f32,
    states: StateTrack<TaskState>,
    x_property: PropertyId,
    y_property: PropertyId,
}

struct CompiledAnnotation {
    target: TargetGeometry,
    effect: AnnotationEffect,
    range: TimeRange,
}

impl CompiledScene {
    /// A scene with no initial property values or semantic targets.
    pub fn from_composition(composition: impl Into<Composition>) -> Result<Self> {
        Scene::new([], composition).compile(&HashMap::new())
    }

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    pub fn media(&self) -> &[MediaPlacement] {
        &self.media
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn task_frames_at(&self, seconds: f32) -> Vec<TaskFrame<'_>> {
        let seconds = seconds.max(0.0);
        self.tasks
            .iter()
            .filter_map(|actor| {
                if seconds < actor.starts_at {
                    return None;
                }
                let state = actor.states.sample(seconds);
                if state.current == &TaskState::Hidden && state.age >= 0.25 {
                    return None;
                }
                let visible_at = actor
                    .states
                    .last_interval_start(seconds, |state| state != &TaskState::Hidden)
                    .unwrap_or(actor.starts_at);
                let x = self.task_pose_timeline.sample(&actor.x_property, seconds)?;
                let y = self.task_pose_timeline.sample(&actor.y_property, seconds)?;
                Some(TaskFrame {
                    id: &actor.task.id,
                    x: x.position,
                    y: y.position,
                    name: &actor.task.name,
                    result_width: actor.task.result_width,
                    previous_state: state.previous,
                    previous_state_duration: state.previous_duration,
                    state: state.current,
                    state_age: state.age,
                    visible_age: seconds - visible_at,
                })
            })
            .collect()
    }

    pub fn annotations_at(&self, seconds: f32) -> impl Iterator<Item = AnnotationFrame> + '_ {
        let seconds = f64::from(seconds.max(0.0));
        self.annotations.iter().filter_map(move |annotation| {
            let start = annotation.range.start().as_seconds();
            let duration = annotation.range.duration().as_seconds();
            let phase = (seconds - start) / duration;
            (0.0..1.0).contains(&phase).then_some(AnnotationFrame {
                target: annotation.target,
                effect: annotation.effect,
                phase: phase as f32,
            })
        })
    }
}

#[derive(Clone)]
pub struct Pointer {
    pub x: PropertyId,
    pub y: PropertyId,
    pub opacity: PropertyId,
    pub scale: PropertyId,
    pub blur: PropertyId,
}

#[derive(Clone)]
pub struct Code {
    pub panel_y: PropertyId,
    pub panel_rotation: PropertyId,
    pub panel_tilt_x: PropertyId,
    pub panel_tilt_y: PropertyId,
    pub panel_scale: PropertyId,
    pub panel_near_blur: PropertyId,
    pub focus: PropertyId,
    pub focus_y: PropertyId,
    pub highlight_x: PropertyId,
    pub highlight_y: PropertyId,
    pub highlight_width: PropertyId,
    pub highlight_opacity: PropertyId,
}

#[derive(Clone)]
pub struct CodeEdit {
    pub layout: PropertyId,
    pub content: PropertyId,
}

impl CodeEdit {
    fn new(id: &str) -> Self {
        Self {
            layout: PropertyId::new(format!("{id}.layout")),
            content: PropertyId::new(format!("{id}.content")),
        }
    }

    pub fn initial_values(&self) -> [(PropertyId, Scalar); 2] {
        [
            (self.layout.clone(), Scalar::Literal(0.0)),
            (self.content.clone(), Scalar::Literal(0.0)),
        ]
    }

    pub fn enter(&self, profile: SpringProfile) -> Motion {
        self.animate_to(1.0, profile)
    }

    pub fn exit(&self, profile: SpringProfile) -> Motion {
        self.animate_to(0.0, profile)
    }

    pub fn progress_at(&self, scene: &CompiledScene, seconds: f32) -> Option<TransitionProgress> {
        Some(TransitionProgress {
            layout: scene.timeline().sample(&self.layout, seconds)?.position,
            content: scene.timeline().sample(&self.content, seconds)?.position,
        })
    }

    pub fn sample_at<'a>(
        &self,
        scene: &CompiledScene,
        transition: &'a CodeTransition,
        seconds: f32,
    ) -> Option<Vec<PlacedLine<'a>>> {
        self.progress_at(scene, seconds)
            .map(|progress| transition.sample(progress))
    }

    fn animate_to(&self, progress: f32, profile: SpringProfile) -> Motion {
        Motion::parallel([
            Motion::spring(self.layout.clone(), progress, profile),
            Motion::spring(self.content.clone(), progress, profile),
        ])
    }
}

impl Code {
    pub fn new(id: &str) -> Self {
        Self {
            panel_y: PropertyId::new(format!("{id}.panel_y")),
            panel_rotation: PropertyId::new(format!("{id}.panel_rotation")),
            panel_tilt_x: PropertyId::new(format!("{id}.panel_tilt_x")),
            panel_tilt_y: PropertyId::new(format!("{id}.panel_tilt_y")),
            panel_scale: PropertyId::new(format!("{id}.panel_scale")),
            panel_near_blur: PropertyId::new(format!("{id}.panel_near_blur")),
            focus: PropertyId::new(format!("{id}.focus")),
            focus_y: PropertyId::new(format!("{id}.focus_y")),
            highlight_x: PropertyId::new(format!("{id}.highlight.x")),
            highlight_y: PropertyId::new(format!("{id}.highlight.y")),
            highlight_width: PropertyId::new(format!("{id}.highlight.width")),
            highlight_opacity: PropertyId::new(format!("{id}.highlight.opacity")),
        }
    }

    pub fn text(&self, line_id: impl Into<String>, text: impl Into<String>) -> TextTarget {
        TextTarget::new(line_id, text)
    }

    pub fn edit(&self, id: &str) -> CodeEdit {
        let code_id = self
            .panel_y
            .as_str()
            .strip_suffix(".panel_y")
            .expect("Code panel property retains its generated suffix");
        CodeEdit::new(&format!("{code_id}.edit.{id}"))
    }

    pub fn highlight(&self, target: TextTarget, profile: SpringProfile) -> Motion {
        Motion::parallel([
            Motion::spring(
                self.highlight_x.clone(),
                Scalar::TargetX(target.clone()),
                profile,
            ),
            Motion::spring(
                self.highlight_y.clone(),
                Scalar::TargetLineY(target.clone()),
                profile,
            ),
            Motion::spring(
                self.highlight_width.clone(),
                Scalar::TargetWidth(target),
                profile,
            ),
            Motion::spring(self.highlight_opacity.clone(), 1.0, profile),
        ])
    }

    pub fn focus(&self, target: TextTarget, profile: SpringProfile) -> Motion {
        Motion::parallel([
            Motion::spring(self.focus.clone(), 1.0, profile),
            Motion::spring(self.focus_y.clone(), Scalar::TargetLineY(target), profile),
        ])
    }
}

impl Pointer {
    pub fn new(id: &str) -> Self {
        Self {
            x: PropertyId::new(format!("{id}.x")),
            y: PropertyId::new(format!("{id}.y")),
            opacity: PropertyId::new(format!("{id}.opacity")),
            scale: PropertyId::new(format!("{id}.scale")),
            blur: PropertyId::new(format!("{id}.blur")),
        }
    }

    pub fn move_to(&self, target: TextTarget, offset_y: f32, profile: SpringProfile) -> Motion {
        Motion::parallel([
            Motion::spring(
                self.x.clone(),
                Scalar::TargetCenterX(target.clone()),
                profile,
            ),
            Motion::spring(
                self.y.clone(),
                Scalar::TargetBelow {
                    target,
                    offset: offset_y,
                },
                profile,
            ),
        ])
    }
}

fn resolve_scalar(scalar: &Scalar, targets: &HashMap<TextTarget, TargetGeometry>) -> Result<f32> {
    Ok(match scalar {
        Scalar::Literal(value) => *value,
        Scalar::TargetX(target) => resolve_target(target, targets)?.x,
        Scalar::TargetWidth(target) => resolve_target(target, targets)?.width,
        Scalar::TargetCenterX(target) => resolve_target(target, targets)?.center_x(),
        Scalar::TargetLineY(target) => resolve_target(target, targets)?.line_y,
        Scalar::TargetBelow { target, offset } => resolve_target(target, targets)?.below(*offset),
        Scalar::Offset { value, amount } => resolve_scalar(value, targets)? + amount,
    })
}

fn resolve_target(
    target: &TextTarget,
    targets: &HashMap<TextTarget, TargetGeometry>,
) -> Result<TargetGeometry> {
    targets.get(target).copied().with_context(|| {
        format!(
            "semantic target '{}:{}' was not measured",
            target.line_id, target.text
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{
        Annotation, AnnotationEffect, Code, Motion, Pointer, Scalar, Scene, TargetGeometry, Task,
        TaskState, TextTarget,
    };
    use crate::code::{
        CodeDocument, CodeLayout, CodeLine, CodeSnapshot, CodeTransition, StyledSpan, SyntaxStyle,
    };
    use crate::composition::{Asset, Composition, Duration, MediaRole, Time, TimeRange};
    use crate::timeline::SpringProfile;
    use std::collections::HashMap;

    #[test]
    fn code_edit_coordinates_transition_tracks_and_sampling() {
        let document = CodeDocument::new(vec![
            CodeLine::new("a", vec![StyledSpan::new("a", SyntaxStyle::Plain)]),
            CodeLine::new(
                "inserted",
                vec![StyledSpan::new("inserted", SyntaxStyle::Plain)],
            ),
        ])
        .unwrap();
        let transition = CodeTransition::compile(
            &document,
            &CodeSnapshot::new(["a"]),
            &CodeSnapshot::new(["a", "inserted"]),
            CodeLayout {
                line_height: 44.0,
                entering_offset_x: 96.0,
            },
        )
        .unwrap();
        let edit = Code::new("lesson.code").edit("insert");
        let profile = SpringProfile::from_visual_duration(0.3, 0.0, 0.001, 0.001);
        let scene = Scene::new(edit.initial_values(), edit.enter(profile));
        let compiled = scene.compile(&HashMap::new()).unwrap();

        let before = edit.sample_at(&compiled, &transition, 0.0).unwrap();
        let after = edit.sample_at(&compiled, &transition, 1.0).unwrap();
        let inserted_before = before
            .iter()
            .find(|line| line.line.id.as_str() == "inserted")
            .unwrap();
        let inserted_after = after
            .iter()
            .find(|line| line.line.id.as_str() == "inserted")
            .unwrap();

        assert_eq!(inserted_before.x, 96.0);
        assert_eq!(inserted_before.opacity, 0.0);
        assert!(inserted_after.x.abs() < 0.01);
        assert!(inserted_after.opacity > 0.999);

        let reversed = Scene::new(
            edit.initial_values(),
            Motion::parallel([
                edit.enter(profile),
                Motion::delay(edit.enter(profile).duration(), edit.exit(profile)),
            ]),
        )
        .compile(&HashMap::new())
        .unwrap();
        let after_exit = edit.sample_at(&reversed, &transition, 10.0).unwrap();
        let inserted_after_exit = after_exit
            .iter()
            .find(|line| line.line.id.as_str() == "inserted")
            .unwrap();
        assert_eq!(inserted_after_exit.x, 96.0);
        assert_eq!(inserted_after_exit.opacity, 0.0);
    }

    #[test]
    fn task_state_changes_compile_as_composable_timeline_events() {
        let task = Task::new("request", "request").at(120.0, 80.0);
        let scene = Scene::new(
            [],
            Composition::parallel([
                task.idle().into(),
                Composition::delay(Duration::seconds(0.5), task.run()),
                Composition::delay(Duration::seconds(1.0), task.succeed("OK")),
                Composition::delay(Duration::seconds(1.0), task.move_to(240.0, 80.0)),
                Composition::hold(Duration::seconds(2.0)),
            ]),
        );
        let compiled = scene.compile(&HashMap::new()).unwrap();

        assert_eq!(compiled.task_frames_at(0.25)[0].state, &TaskState::Idle);
        assert_eq!(compiled.task_frames_at(0.75)[0].state, &TaskState::Running);
        let completed = &compiled.task_frames_at(1.9)[0];
        assert_eq!(
            completed.state,
            &TaskState::Succeeded(Some("OK".to_owned()))
        );
        assert!((completed.previous_state_duration - 0.5).abs() < 0.001);
        assert!((completed.x - 240.0).abs() < 0.1);
    }

    #[test]
    fn showing_a_hidden_task_starts_a_new_visibility_interval() {
        let task = Task::new("request", "request").at(120.0, 80.0);
        let scene = Scene::new(
            [],
            Composition::parallel([
                task.idle().into(),
                Composition::delay(Duration::seconds(0.5), task.hide()),
                Composition::delay(Duration::seconds(1.0), task.run()),
                Composition::hold(Duration::seconds(2.0)),
            ]),
        );
        let compiled = scene.compile(&HashMap::new()).unwrap();

        let exiting = &compiled.task_frames_at(0.55)[0];
        assert_eq!(exiting.state, &TaskState::Hidden);
        assert!((exiting.visible_age - 0.55).abs() < 0.001);
        assert!(compiled.task_frames_at(0.75).is_empty());
        let shown = &compiled.task_frames_at(1.1)[0];
        assert_eq!(shown.state, &TaskState::Running);
        assert!((shown.visible_age - 0.1).abs() < 0.001);
    }

    #[test]
    fn moving_a_task_does_not_restart_its_state_transition() {
        let task = Task::new("request", "request").at(120.0, 80.0);
        let scene = Scene::new(
            [],
            Composition::parallel([
                task.run().into(),
                Composition::delay(Duration::seconds(0.5), task.succeed("OK")),
                Composition::delay(Duration::seconds(1.0), task.move_to(240.0, 80.0)),
                Composition::hold(Duration::seconds(2.0)),
            ]),
        );
        let compiled = scene.compile(&HashMap::new()).unwrap();

        let frame = &compiled.task_frames_at(1.1)[0];
        assert_eq!(frame.state, &TaskState::Succeeded(Some("OK".to_owned())));
        assert!((frame.state_age - 0.6).abs() < 0.001);
        assert!(frame.x > 120.0 && frame.x < 240.0);
    }

    #[test]
    fn interrupted_task_pose_motion_preserves_velocity() {
        let task = Task::new("request", "request").at(120.0, 80.0);
        let scene = Scene::new(
            [],
            Composition::parallel([
                task.idle().into(),
                Composition::delay(Duration::seconds(0.2), task.move_to(240.0, 80.0)),
                Composition::delay(Duration::seconds(0.3), task.move_to(0.0, 80.0)),
                Composition::hold(Duration::seconds(1.0)),
            ]),
        );
        let compiled = scene.compile(&HashMap::new()).unwrap();

        let x = &compiled.tasks[0].x_property;
        let before = compiled.task_pose_timeline.sample(x, 0.2999).unwrap();
        let redirected = compiled.task_pose_timeline.sample(x, 0.3).unwrap();
        assert_eq!(compiled.task_frames_at(0.3)[0].x, redirected.position);
        assert!((redirected.position - before.position).abs() < 0.1);
        assert!(redirected.velocity > 0.0);
        assert!((redirected.velocity - before.velocity).abs() < 1.0);
    }

    #[test]
    fn semantic_pointer_motion_compiles_to_scalar_tracks() {
        let pointer = Pointer::new("pointer");
        let target = TextTarget::new("line", "Effect");
        let profile = SpringProfile::from_visual_duration(0.3, 0.0, 0.001, 0.001);
        let scene = Scene::new(
            [
                (pointer.x.clone(), Scalar::Literal(0.0)),
                (pointer.y.clone(), Scalar::Literal(0.0)),
            ],
            Motion::delay(0.2, pointer.move_to(target.clone(), 55.0, profile)),
        );
        let targets = HashMap::from([(
            target,
            TargetGeometry {
                x: 100.0,
                width: 40.0,
                line_y: 80.0,
            },
        )]);
        let compiled = scene.compile(&targets).unwrap();
        let timeline = compiled.timeline();

        assert_eq!(timeline.sample(&pointer.x, 0.0).unwrap().position, 0.0);
        assert!((timeline.sample(&pointer.x, 2.0).unwrap().position - 120.0).abs() < 0.01);
        assert!((timeline.sample(&pointer.y, 2.0).unwrap().position - 135.0).abs() < 0.01);
    }

    #[test]
    fn scene_compilation_preserves_scheduled_media_with_visual_motion() {
        let opacity = crate::timeline::PropertyId::new("title.opacity");
        let profile = SpringProfile::from_visual_duration(0.3, 0.0, 0.001, 0.001);
        let narration = Asset::audio("narration", "assets/narration.wav")
            .clip(TimeRange::new(Time::seconds(1.0), Time::seconds(3.0)));
        let composition = Composition::parallel([
            Composition::script(narration),
            Composition::named("title", Motion::spring(opacity.clone(), 1.0, profile)),
        ]);
        let scene = Scene::new([(opacity.clone(), Scalar::Literal(0.0))], composition);

        let compiled = scene.compile(&HashMap::new()).unwrap();

        assert_eq!(compiled.media().len(), 1);
        assert_eq!(compiled.media()[0].role(), MediaRole::Script);
        assert_eq!(compiled.duration().as_seconds(), 2.0);
        assert!(compiled.timeline().sample(&opacity, 1.0).unwrap().position > 0.99);
    }

    #[test]
    fn annotations_resolve_semantic_targets_and_sample_at_arbitrary_times() {
        let target = TextTarget::new("signature", "VeryBadRoll");
        assert_eq!(
            Annotation::on(target.clone()).selected_effect(),
            AnnotationEffect::PrismaticBloom
        );
        let geometry = TargetGeometry {
            x: 120.0,
            width: 84.0,
            line_y: 44.0,
        };
        let scene = Scene::new(
            [],
            Composition::delay(
                Duration::seconds(0.5),
                Annotation::on(target.clone()).effect(AnnotationEffect::FocusPulse),
            ),
        );
        let compiled = scene.compile(&HashMap::from([(target, geometry)])).unwrap();

        assert!(compiled.annotations_at(0.49).next().is_none());
        assert!(compiled.annotations_at(1.41).next().is_none());

        let later = compiled.annotations_at(0.95).next().unwrap();
        let earlier = compiled.annotations_at(0.5).next().unwrap();
        assert_eq!(earlier.target.x, geometry.x);
        assert_eq!(earlier.effect, AnnotationEffect::FocusPulse);
        assert_eq!(earlier.phase, 0.0);
        assert!((later.phase - 0.5).abs() < 0.0001);
    }
}
