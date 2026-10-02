//! Planned Effect Institute Task blocks. Semantic state changes lower to scalar
//! destinations in the renderer, keeping native interruption and video aligned.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

pub const TASK_RECIPE: &str = "effect-task";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum TaskState {
    Hidden,
    Idle,
    Running,
    Succeeded(Option<String>),
    Failed(String),
    Death(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecipePlan {
    pub name: String,
    pub center: [f32; 2],
    pub initial: TaskState,
    #[serde(default)]
    pub events: Vec<TaskEventPlan>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskEventPlan {
    pub at_nanos: u64,
    pub state: TaskState,
}

impl TaskRecipePlan {
    pub fn states(&self) -> Vec<TaskState> {
        let mut states = vec![self.initial.clone()];
        for event in crate::plan::effective_snapshots(&self.events, |e| e.at_nanos) {
            if !states.contains(&event.state) {
                states.push(event.state.clone());
            }
        }
        states
    }

    pub fn channel_properties(&self) -> Vec<String> {
        let mut properties = ["x", "y", "width", "height", "scale", "opacity", "activity"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        for (index, state) in self.states().iter().enumerate() {
            properties.push(format!("state.{index}"));
            if !matches!(state, TaskState::Hidden | TaskState::Running) {
                properties.extend(
                    ["opacity", "scale", "blur", "rotation"]
                        .map(|property| format!("content.{index}.{property}")),
                );
            }
            if matches!(state, TaskState::Failed(_) | TaskState::Death(_)) {
                properties.extend(
                    ["opacity", "scale", "blur", "y"]
                        .map(|property| format!("bubble.{index}.{property}")),
                );
            }
        }
        properties
    }

    pub fn validate(&self, duration_nanos: u64) -> Result<()> {
        if self.name.trim().is_empty() || self.center.iter().any(|value| !value.is_finite()) {
            bail!("task requires a name and finite center coordinates");
        }
        let mut previous = 0;
        for event in &self.events {
            if event.at_nanos < previous || event.at_nanos > duration_nanos {
                bail!("task events must be ordered within scene duration");
            }
            previous = event.at_nanos;
        }
        Ok(())
    }
}
