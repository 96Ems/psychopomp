use serde::Serialize;

use crate::math::easing::Ease;
use crate::plan::{
    ActorPlan, ContinuousChannelPlan, CuePlan, MediaPlan, PresentationStepPlan, ScalarPlan,
    ScenePlan, SemanticTargetPlan, StateChannelPlan, StateEventPlan, TargetComponentPlan,
    TargetScalarPlan, TrackEventPlan,
};

pub struct PlanBuilder {
    plan: ScenePlan,
}

/// One second on the plan clock, in nanoseconds.
pub const SECOND: u64 = 1_000_000_000;

/// `seconds` on the plan clock, rounded to the nearest nanosecond.
pub fn seconds(seconds: f64) -> u64 {
    (seconds * 1e9).round() as u64
}

/// Whole milliseconds in nanoseconds: an f32 duration such as 0.8 is not exact
/// in nanoseconds, so eases and helpers that chain them round alike.
pub(crate) fn whole_millis(seconds: f32) -> u64 {
    (f64::from(seconds) * 1000.0).round() as u64 * 1_000_000
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ActorHandle {
    id: String,
}

impl ActorHandle {
    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ContinuousHandle {
    id: String,
}

impl ContinuousHandle {
    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct StateHandle {
    id: String,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SemanticTargetHandle {
    id: String,
}

impl SemanticTargetHandle {
    pub fn x(&self) -> ScalarPlan {
        self.component(TargetComponentPlan::X)
    }

    pub fn width(&self) -> ScalarPlan {
        self.component(TargetComponentPlan::Width)
    }

    pub fn center_x(&self) -> ScalarPlan {
        self.component(TargetComponentPlan::CenterX)
    }

    pub fn line_y(&self) -> ScalarPlan {
        self.component(TargetComponentPlan::LineY)
    }

    pub fn offset(&self, component: TargetComponentPlan, offset: f32) -> ScalarPlan {
        ScalarPlan::Target(TargetScalarPlan {
            target_id: self.id.clone(),
            component,
            offset,
        })
    }

    fn component(&self, component: TargetComponentPlan) -> ScalarPlan {
        self.offset(component, 0.0)
    }
}

impl PlanBuilder {
    pub fn new(id: impl Into<String>, duration_nanos: u64) -> Self {
        Self {
            plan: ScenePlan::new(id, duration_nanos),
        }
    }

    pub fn actor(
        &mut self,
        id: impl Into<String>,
        recipe: impl Into<String>,
        data: impl Serialize,
    ) -> Result<ActorHandle, serde_json::Error> {
        let id = id.into();
        self.plan.actors.push(ActorPlan {
            id: id.clone(),
            recipe: recipe.into(),
            data: serde_json::to_value(data)?,
        });
        Ok(ActorHandle { id })
    }

    /// Replace a declared actor's recipe data, for handles whose recipe grows
    /// as the scene is authored (such as a Rolling Number's later values).
    pub fn replace_actor_data(
        &mut self,
        actor: &ActorHandle,
        data: impl Serialize,
    ) -> Result<(), serde_json::Error> {
        let data = serde_json::to_value(data)?;
        self.plan
            .actors
            .iter_mut()
            .find(|candidate| candidate.id == actor.id)
            .expect("actor handle belongs to this plan builder")
            .data = data;
        Ok(())
    }

    /// The `actor.property` channel: the existing one, or a new one starting at
    /// `initial`. An already declared channel keeps its initial value.
    pub fn channel(
        &mut self,
        actor: &ActorHandle,
        property: &str,
        initial: impl Into<ScalarPlan>,
    ) -> ContinuousHandle {
        let id = format!("{}.{}", actor.id, property);
        if self
            .plan
            .continuous_channels
            .iter()
            .any(|channel| channel.id == id)
        {
            return ContinuousHandle { id };
        }
        self.continuous(actor, property, initial)
    }

    pub fn continuous(
        &mut self,
        actor: &ActorHandle,
        property: impl Into<String>,
        initial: impl Into<ScalarPlan>,
    ) -> ContinuousHandle {
        let property = property.into();
        let id = format!("{}.{}", actor.id, property);
        self.plan.continuous_channels.push(ContinuousChannelPlan {
            id: id.clone(),
            actor_id: actor.id.clone(),
            property,
            initial: initial.into(),
            events: Vec::new(),
        });
        ContinuousHandle { id }
    }

    /// One destination per already declared Presentation Step. The first value
    /// is the initial pose; only changed later destinations emit spring events.
    /// Ordinary `spring` remains an explicit write and never suppresses events.
    pub fn step_track(
        &mut self,
        actor: &ActorHandle,
        property: impl Into<String>,
        values: &[f32],
        motion: crate::plan::SpringPlan,
    ) -> ContinuousHandle {
        let property = property.into();
        assert!(
            !values.is_empty(),
            "a destination track needs an initial value"
        );
        assert_eq!(
            values.len(),
            self.plan.presentation_steps.len(),
            "one destination per step: {}.{property}",
            actor.id
        );
        let channel = crate::plan::destination_channel(
            &actor.id,
            property,
            values[0],
            self.plan
                .presentation_steps
                .iter()
                .zip(values)
                .skip(1)
                .map(|(step, value)| (step.start_nanos, *value)),
            |_, _| motion,
        );
        let handle = ContinuousHandle {
            id: channel.id.clone(),
        };
        self.plan.continuous_channels.push(channel);
        handle
    }

    pub fn duration_nanos(&self) -> u64 {
        self.plan.duration_nanos
    }

    pub fn set(&mut self, channel: &ContinuousHandle, at_nanos: u64, value: f32) {
        self.set_to(channel, at_nanos, value.into());
    }

    pub fn set_to(&mut self, channel: &ContinuousHandle, at_nanos: u64, value: ScalarPlan) {
        self.continuous_channel_mut(channel)
            .events
            .push(TrackEventPlan::Set { at_nanos, value });
    }

    pub fn spring(
        &mut self,
        channel: &ContinuousHandle,
        at_nanos: u64,
        target: f32,
        visual_duration: f32,
        bounce: f32,
    ) {
        self.spring_to(channel, at_nanos, target.into(), visual_duration, bounce);
    }

    pub fn spring_to(
        &mut self,
        channel: &ContinuousHandle,
        at_nanos: u64,
        target: ScalarPlan,
        visual_duration: f32,
        bounce: f32,
    ) {
        let event =
            crate::plan::SpringPlan::visual(visual_duration, bounce).event(at_nanos, target);
        self.continuous_channel_mut(channel).events.push(event);
    }

    /// Move `channel` from its current value to `target` along `curve` over
    /// `seconds` (rounded to whole milliseconds, so f32 durations stay exact).
    pub fn ease(
        &mut self,
        channel: &ContinuousHandle,
        at_nanos: u64,
        target: f32,
        seconds: f32,
        curve: Ease,
    ) {
        let duration_nanos = whole_millis(seconds);
        self.continuous_channel_mut(channel)
            .events
            .push(TrackEventPlan::Ease {
                at_nanos,
                target: target.into(),
                duration_nanos,
                curve,
            });
    }

    pub fn semantic_target(
        &mut self,
        id: impl Into<String>,
        actor: &ActorHandle,
        selector: impl Serialize,
    ) -> Result<SemanticTargetHandle, serde_json::Error> {
        let id = id.into();
        self.plan.semantic_targets.push(SemanticTargetPlan {
            id: id.clone(),
            actor_id: actor.id.clone(),
            selector: serde_json::to_value(selector)?,
        });
        Ok(SemanticTargetHandle { id })
    }

    pub fn state(
        &mut self,
        actor: &ActorHandle,
        name: impl Into<String>,
        initial: impl Serialize,
    ) -> Result<StateHandle, serde_json::Error> {
        let name = name.into();
        let id = format!("{}.{}", actor.id, name);
        self.plan.state_channels.push(StateChannelPlan {
            id: id.clone(),
            actor_id: actor.id.clone(),
            state: name,
            initial: serde_json::to_value(initial)?,
            events: Vec::new(),
        });
        Ok(StateHandle { id })
    }

    pub fn change(
        &mut self,
        channel: &StateHandle,
        at_nanos: u64,
        value: impl Serialize,
    ) -> Result<(), serde_json::Error> {
        let value = serde_json::to_value(value)?;
        self.state_channel_mut(channel)
            .events
            .push(StateEventPlan { at_nanos, value });
        Ok(())
    }

    pub fn cue(&mut self, id: impl Into<String>, start_nanos: u64, end_nanos: u64) {
        self.plan.cues.push(CuePlan {
            id: id.into(),
            start_nanos,
            end_nanos,
        });
    }

    pub fn media(&mut self, media: MediaPlan) {
        self.plan.media.push(media);
    }

    /// Declare a presentation stop without changing the automatic video timing.
    pub fn presentation_step(
        &mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        start_nanos: u64,
        hold_nanos: u64,
    ) {
        self.plan.presentation_steps.push(PresentationStepPlan {
            id: id.into(),
            title: title.into(),
            start_nanos,
            hold_nanos,
        });
    }

    pub fn finish(self) -> Result<ScenePlan, crate::plan::PlanValidationError> {
        self.plan.validate()?;
        Ok(self.plan)
    }

    fn continuous_channel_mut(&mut self, handle: &ContinuousHandle) -> &mut ContinuousChannelPlan {
        self.plan
            .continuous_channels
            .iter_mut()
            .find(|channel| channel.id == handle.id)
            .expect("continuous handle belongs to this plan builder")
    }

    fn state_channel_mut(&mut self, handle: &StateHandle) -> &mut StateChannelPlan {
        self.plan
            .state_channels
            .iter_mut()
            .find(|channel| channel.id == handle.id)
            .expect("state handle belongs to this plan builder")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::PlanBuilder;

    #[test]
    fn step_destinations_use_authored_times_and_do_not_restart_constants() {
        let mut p = PlanBuilder::new("steps", 3_000_000_000);
        for (index, at) in [0, 800_000_000, 2_000_000_000].into_iter().enumerate() {
            p.presentation_step(format!("s{index}"), "step", at, at);
        }
        let actor = p
            .actor("a", "title-card", json!({"title":"Example"}))
            .unwrap();
        p.step_track(
            &actor,
            "opacity",
            &[0., 0., 1.],
            crate::plan::SpringPlan::visual(0.16, 0.),
        );
        let plan = p.finish().unwrap();
        assert_eq!(plan.continuous_channels[0].events.len(), 1);
        assert_eq!(
            plan.continuous_channels[0].events[0].at_nanos(),
            2_000_000_000
        );
    }

    #[test]
    fn channel_declares_once_in_first_use_order() {
        let mut scene = PlanBuilder::new("channels", 1_000_000_000);
        let actor = scene.actor("a", "title-card", json!({})).unwrap();
        let y = scene.channel(&actor, "y", 10.0);
        scene.channel(&actor, "opacity", 0.0);
        assert_eq!(scene.channel(&actor, "y", 99.0), y);
        let plan = scene.finish().unwrap();
        let ids = plan
            .continuous_channels
            .iter()
            .map(|channel| match channel.initial {
                crate::plan::ScalarPlan::Literal(value) => (channel.id.as_str(), value),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![("a.y", 10.0), ("a.opacity", 0.0)],
            "a repeated channel keeps its first initial value"
        );
    }

    #[test]
    fn handles_construct_valid_deterministic_channels() {
        let mut scene = PlanBuilder::new("demo", 2_000_000_000);
        let title = scene
            .actor("title", "title-card", json!({ "title": "Hello" }))
            .unwrap();
        let opacity = scene.continuous(&title, "opacity", 0.0);
        let subtitle = scene.state(&title, "subtitle", "First").unwrap();
        let target = scene
            .semantic_target("title-text", &title, json!({ "rangeId": "title" }))
            .unwrap();
        let x = scene.continuous(&title, "x", target.center_x());
        scene.spring(&opacity, 0, 1.0, 0.4, 0.0);
        scene.spring_to(
            &x,
            0,
            target.offset(crate::plan::TargetComponentPlan::X, 20.0),
            0.4,
            0.0,
        );
        scene.change(&subtitle, 1_000_000_000, "Second").unwrap();
        scene.cue("change", 1_000_000_000, 2_000_000_000);
        scene.presentation_step("change", "Change the title", 1_000_000_000, 2_000_000_000);
        let plan = scene.finish().unwrap();

        assert_eq!(opacity.id(), "title.opacity");
        assert_eq!(plan.state_channels[0].id, "title.subtitle");
        assert_eq!(plan.continuous_channels[0].actor_id, title.id());
        assert_eq!(plan.semantic_targets[0].id, "title-text");
        assert_eq!(plan.cues[0].id, "change");
        assert_eq!(plan.presentation_steps[0].title, "Change the title");
    }
}
