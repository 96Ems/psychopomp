use std::ops::Range;

use anyhow::{Context, Result};
use kinograph::{
    code::{CodeTransition, RangeId, TransitionProgress},
    dsl::TargetGeometry,
    editor::{EditorRecipePlan, EditorTargetSelector},
    motion::MotionState,
    plan::{ActorPlan, SemanticTargetPlan},
};

use crate::render::{
    EditorFrame, HeadlessRenderer, InlineRevealFrame, PointerFrame, TokenHighlight,
};

pub(super) struct PreparedEditor {
    actor_id: String,
    recipe: EditorRecipePlan,
    transition: CodeTransition,
    inline_reveal_spans: Range<usize>,
}

impl PreparedEditor {
    pub(super) fn new(actor: &ActorPlan) -> Result<Self> {
        let recipe = serde_json::from_value::<EditorRecipePlan>(actor.data.clone())
            .with_context(|| format!("parse editor recipe for actor '{}'", actor.id))?;
        let transition = recipe.transition()?;
        let settled = transition.sample(TransitionProgress {
            layout: 1.0,
            content: 1.0,
        });
        let reveal_line = settled
            .iter()
            .find(|line| line.line.id.as_str() == recipe.inline_reveal.line_id)
            .with_context(|| {
                format!(
                    "editor actor '{}' inline reveal references unknown line '{}'",
                    actor.id, recipe.inline_reveal.line_id
                )
            })?;
        let inline_reveal_spans = reveal_line
            .line
            .semantic_span_range(&RangeId::new(&recipe.inline_reveal.range_id))?;
        Ok(Self {
            actor_id: actor.id.clone(),
            recipe,
            transition,
            inline_reveal_spans,
        })
    }

    pub(super) fn actor_id(&self) -> &str {
        &self.actor_id
    }

    pub(super) fn file_name(&self) -> &str {
        &self.recipe.file_name
    }

    pub(super) fn resolve_target(
        &self,
        renderer: &mut HeadlessRenderer,
        target: &SemanticTargetPlan,
    ) -> Result<TargetGeometry> {
        let selector = serde_json::from_value::<EditorTargetSelector>(target.selector.clone())
            .with_context(|| format!("parse editor semantic target '{}'", target.id))?;
        let lines = self.transition.sample(TransitionProgress {
            layout: 1.0,
            content: 1.0,
        });
        let line = lines
            .iter()
            .find(|line| line.line.id.as_str() == selector.line_id)
            .with_context(|| {
                format!(
                    "editor actor '{}' target '{}' references unknown line '{}'",
                    self.actor_id, target.id, selector.line_id
                )
            })?;
        let bytes = line
            .line
            .semantic_byte_range(&RangeId::new(&selector.range_id))?;
        let bounds = renderer.measure_text_byte_range(line.line, bytes.start, bytes.end)?;
        Ok(TargetGeometry {
            x: bounds.x,
            width: bounds.width,
            line_y: line.y,
        })
    }

    pub(super) fn render(
        &self,
        renderer: &mut HeadlessRenderer,
        time: f64,
        pointer_actor_id: Option<&str>,
        sample: impl Fn(&str, &str, f64) -> Option<MotionState>,
    ) -> Result<Vec<u8>> {
        let value = |actor: &str, property: &str, default: f32| {
            sample(actor, property, time).map_or(default, |state| state.position)
        };
        let layout = value(&self.actor_id, "layout", 0.0);
        let content = value(&self.actor_id, "content", 0.0);
        let lines = self
            .transition
            .sample(TransitionProgress { layout, content });
        let focus_line_y = lines
            .iter()
            .find(|line| line.line.id.as_str() == self.recipe.focus_line_id)
            .with_context(|| {
                format!(
                    "editor actor '{}' focus references unknown line '{}'",
                    self.actor_id, self.recipe.focus_line_id
                )
            })?
            .y;
        let token_highlight = TokenHighlight {
            x: value(&self.actor_id, "highlight-x", 0.0),
            y: value(&self.actor_id, "highlight-y", 0.0),
            width: value(&self.actor_id, "highlight-width", 0.0),
            opacity: value(&self.actor_id, "highlight-opacity", 0.0).clamp(0.0, 1.0),
        };
        let pointer = if let Some(actor) = pointer_actor_id {
            pointer_frame(actor, time, &sample)?
        } else {
            PointerFrame {
                x: 0.0,
                y: 0.0,
                opacity: 0.0,
                rotation: 0.0,
                scale: 1.0,
                blur: 0.0,
            }
        };
        let inline_reveals = [InlineRevealFrame {
            line_id: &self.recipe.inline_reveal.line_id,
            start_span: self.inline_reveal_spans.start,
            end_span: self.inline_reveal_spans.end,
            progress: value(&self.actor_id, "inline-reveal", 0.0),
        }];
        renderer.render_editor(&EditorFrame {
            panel_offset_y: value(&self.actor_id, "panel-y", 0.0),
            panel_rotation: 0.0,
            panel_tilt_x: 0.0,
            panel_tilt_y: 0.0,
            panel_scale: 1.0,
            panel_near_blur: 0.0,
            focus_intensity: value(&self.actor_id, "focus", 0.0).clamp(0.0, 1.0),
            focus_line_y,
            focus_height: self.recipe.focus_height,
            token_highlight,
            bright_text: &[],
            pointer,
            inline_reveals: &inline_reveals,
            squiggles: &[],
            annotations: &[],
            lines: &lines,
        })
    }
}

fn pointer_frame(
    actor_id: &str,
    time: f64,
    sample: impl Fn(&str, &str, f64) -> Option<MotionState>,
) -> Result<PointerFrame> {
    let state = |property: &str, at: f64| {
        sample(actor_id, property, at)
            .with_context(|| format!("pointer actor '{actor_id}' has no '{property}' channel"))
    };
    let x = state("x", time)?;
    let y = state("y", time)?;
    let derivative_step = 1.0 / 240.0;
    let previous_time = (time - derivative_step).max(0.0);
    let previous_x = state("x", previous_time)?;
    let previous_y = state("y", previous_time)?;
    let acceleration_x = (x.velocity - previous_x.velocity) / derivative_step as f32;
    let acceleration_y = (y.velocity - previous_y.velocity) / derivative_step as f32;
    Ok(PointerFrame {
        x: x.position,
        y: y.position,
        opacity: state("opacity", time)?.position.clamp(0.0, 1.0),
        rotation: (x.velocity * 0.00012 + y.velocity * 0.00004
            - acceleration_x * 0.000012
            - acceleration_y * 0.000004)
            .clamp(-0.30, 0.30),
        scale: state("scale", time)?.position,
        blur: state("blur", time)?.position.max(0.0),
    })
}
