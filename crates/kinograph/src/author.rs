use serde::Serialize;

use crate::plan::{
    ActorPlan, ContinuousChannelPlan, CuePlan, MediaPlan, ScalarPlan, ScenePlan,
    SemanticTargetPlan, StateChannelPlan, StateEventPlan, TargetComponentPlan, TargetScalarPlan,
    TrackEventPlan,
};

pub struct PlanBuilder {
    plan: ScenePlan,
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
    pub fn id(&self) -> &str {
        &self.id
    }

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

impl StateHandle {
    pub fn id(&self) -> &str {
        &self.id
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
        assert!(visual_duration.is_finite() && visual_duration > 0.0);
        assert!((0.0..1.0).contains(&bounce));
        self.continuous_channel_mut(channel)
            .events
            .push(TrackEventPlan::Spring {
                at_nanos,
                target,
                response_seconds: visual_duration * 1.2,
                damping_ratio: 1.0 - bounce,
                position_threshold: 0.001,
                velocity_threshold: 0.001,
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
        let plan = scene.finish().unwrap();

        assert_eq!(opacity.id(), "title.opacity");
        assert_eq!(subtitle.id(), "title.subtitle");
        assert_eq!(plan.continuous_channels[0].actor_id, title.id());
        assert_eq!(target.id(), "title-text");
        assert_eq!(plan.cues[0].id, "change");
    }
}
