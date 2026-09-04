use std::{collections::HashMap, ops::Range};

use anyhow::{Context, Result};
use kinograph::{
    code::{CodeTransition, RangeId, TransitionProgress},
    dsl::TargetGeometry,
    editor::{EditorRecipePlan, EditorTargetSelector},
    motion::MotionState,
    plan::{ActorPlan, SemanticTargetPlan},
};

use crate::render::{
    EditorFrame, HeadlessRenderer, InlineRangeMetrics, InlineRevealFrame, PointerFrame,
    TokenHighlight,
};

pub(super) struct PreparedEditor {
    actor_id: String,
    recipe: EditorRecipePlan,
    transition: CodeTransition,
    inline_reveals: Vec<PreparedInlineReveal>,
    targets: HashMap<String, MeasuredTarget>,
}

struct MeasuredTarget {
    line_id: String,
    keyed_lines: bool,
    segments: Vec<(InlineRangeMetrics, Option<(String, bool)>)>,
    before: [f32; 2],
    after: [f32; 2],
}

pub(super) struct TargetMotion {
    pub x: MotionState,
    pub width: MotionState,
    pub line_y: MotionState,
}

struct PreparedInlineReveal {
    line_id: String,
    spans: Range<usize>,
    channel: String,
    reversed: bool,
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
        let inline_reveals = std::iter::once(&recipe.inline_reveal)
            .chain(&recipe.additional_inline_reveals)
            .map(|reveal| {
                let line = settled
                    .iter()
                    .find(|line| line.line.id.as_str() == reveal.line_id)
                    .with_context(|| {
                        format!(
                            "editor actor '{}' inline reveal references unknown line '{}'",
                            actor.id, reveal.line_id
                        )
                    })?;
                Ok(PreparedInlineReveal {
                    line_id: reveal.line_id.clone(),
                    spans: line
                        .line
                        .semantic_span_range(&RangeId::new(&reveal.range_id))?,
                    channel: reveal.channel().to_owned(),
                    reversed: reveal.reversed,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            actor_id: actor.id.clone(),
            recipe,
            transition,
            inline_reveals,
            targets: HashMap::new(),
        })
    }

    pub(super) fn actor_id(&self) -> &str {
        &self.actor_id
    }

    pub(super) fn file_name(&self) -> &str {
        &self.recipe.file_name
    }

    pub(super) fn resolve_target(
        &mut self,
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
        let selected = line
            .line
            .semantic_span_range(&RangeId::new(&selector.range_id))?;
        let reveals = self
            .inline_reveals
            .iter()
            .filter(|reveal| reveal.line_id == selector.line_id)
            .collect::<Vec<_>>();
        let frames = reveals
            .iter()
            .map(|reveal| InlineRevealFrame {
                line_id: &reveal.line_id,
                start_span: reveal.spans.start,
                end_span: reveal.spans.end,
                progress: 1.0,
            })
            .collect::<Vec<_>>();
        let segments = renderer
            .measure_inline_target(line.line, &frames, selected)?
            .into_iter()
            .map(|metrics| {
                let driver = reveals
                    .iter()
                    .find(|reveal| reveal.spans == metrics.spans)
                    .map(|reveal| (reveal.channel.clone(), reveal.reversed));
                (metrics, driver)
            })
            .collect();
        let before = self.transition.sample(TransitionProgress {
            layout: 0.,
            content: 0.,
        });
        let before = before
            .iter()
            .find(|line| line.line.id.as_str() == selector.line_id)
            .expect("validated line");
        let measured = MeasuredTarget {
            line_id: selector.line_id,
            keyed_lines: !self.recipe.snapshots.is_empty(),
            segments,
            before: [before.x, before.y],
            after: [line.x, line.y],
        };
        let motion = measured.sample(|_, default| MotionState::at(default), true);
        self.targets.insert(target.id.clone(), measured);
        Ok(TargetGeometry {
            x: motion.x.position,
            width: motion.width.position,
            line_y: motion.line_y.position,
        })
    }

    pub(super) fn target_motion(
        &self,
        id: &str,
        sample: impl Fn(&str, &str) -> Option<MotionState>,
    ) -> Option<TargetMotion> {
        self.targets.get(id).map(|target| {
            target.sample(
                |property, default| {
                    sample(&self.actor_id, property).unwrap_or(MotionState::at(default))
                },
                false,
            )
        })
    }

    pub(super) fn target_scale(&self, id: &str) -> Option<f32> {
        self.targets.get(id).map(|target| {
            let width = target
                .segments
                .iter()
                .map(|(metrics, _)| metrics.advance)
                .sum::<f32>();
            // Conservative local text/line extent, in pixels. Companion weights
            // must not inherit pixel tolerances as dimensionless tolerances.
            (width
                + target.before[0].abs()
                + target.after[0].abs()
                + self.recipe.lines.len() as f32 * self.recipe.line_height)
                .max(1.0)
        })
    }

    pub(super) fn geometry_channels(&self) -> Vec<String> {
        std::iter::once("layout".into())
            .chain(std::iter::once("content".into()))
            .chain(
                self.inline_reveals
                    .iter()
                    .map(|reveal| reveal.channel.clone()),
            )
            .chain(
                self.recipe
                    .lines
                    .iter()
                    .map(|line| format!("line.{}.y", line.id)),
            )
            .collect()
    }

    pub(super) fn snapshot_channels(
        &self,
        duration_nanos: u64,
    ) -> Result<Vec<kinograph::plan::ContinuousChannelPlan>> {
        self.recipe
            .snapshot_channels(&self.actor_id, duration_nanos)
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
        let mut lines = self
            .transition
            .sample(TransitionProgress { layout, content });
        if !self.recipe.snapshots.is_empty() {
            for line in &mut lines {
                line.x = 0.0;
                line.y = value(
                    &self.actor_id,
                    &format!("line.{}.y", line.line.id.as_str()),
                    line.y,
                );
                line.opacity = value(
                    &self.actor_id,
                    &format!("line.{}.opacity", line.line.id.as_str()),
                    line.opacity,
                )
                .clamp(0.0, 1.0);
                line.blur = (1.0 - line.opacity) * 4.0;
            }
        }
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
        let inline_reveals = self
            .inline_reveals
            .iter()
            .map(|reveal| {
                let progress = value(&self.actor_id, &reveal.channel, 0.0);
                InlineRevealFrame {
                    line_id: &reveal.line_id,
                    start_span: reveal.spans.start,
                    end_span: reveal.spans.end,
                    progress: if reveal.reversed {
                        1.0 - progress
                    } else {
                        progress
                    },
                }
            })
            .collect::<Vec<_>>();
        renderer.render_editor(&EditorFrame {
            panel_offset_y: value(&self.actor_id, "panel-y", 0.0),
            panel_rotation: value(&self.actor_id, "panel-rotation", 0.0),
            panel_tilt_x: value(&self.actor_id, "panel-tilt-x", 0.0),
            panel_tilt_y: value(&self.actor_id, "panel-tilt-y", 0.0),
            panel_scale: value(&self.actor_id, "panel-scale", 1.0),
            panel_near_blur: value(&self.actor_id, "panel-near-blur", 0.0),
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

fn add(a: MotionState, b: MotionState) -> MotionState {
    MotionState {
        position: a.position + b.position,
        velocity: a.velocity + b.velocity,
    }
}
fn scale(a: MotionState, factor: f32) -> MotionState {
    MotionState {
        position: a.position * factor,
        velocity: a.velocity * factor,
    }
}

impl MeasuredTarget {
    fn sample(&self, sample: impl Fn(&str, f32) -> MotionState, expanded: bool) -> TargetMotion {
        let mut cursor = MotionState::at(0.0);
        let mut left = None;
        let mut right = MotionState::at(0.0);
        for (metrics, driver) in &self.segments {
            let progress = if let Some((channel, reversed)) = driver.as_ref().filter(|_| !expanded)
            {
                let state = sample(channel, 0.0);
                let state = if *reversed {
                    add(MotionState::at(1.0), scale(state, -1.0))
                } else {
                    state
                };
                MotionState {
                    position: state.position.clamp(0.0, 1.0),
                    velocity: if (0.0..=1.0).contains(&state.position) {
                        state.velocity
                    } else {
                        0.0
                    },
                }
            } else {
                MotionState::at(1.0)
            };
            let width = scale(progress, metrics.advance);
            if let Some([start, end]) = metrics.selection {
                let clipped = |edge| {
                    if width.position < edge {
                        width
                    } else {
                        MotionState::at(edge)
                    }
                };
                left.get_or_insert(add(cursor, clipped(start)));
                right = add(cursor, clipped(end));
            }
            cursor = add(cursor, width);
        }
        let left = left.expect("nonempty validated semantic selection");
        let layout = if expanded {
            MotionState::at(1.0)
        } else {
            sample("layout", 0.0)
        };
        let content = if expanded {
            MotionState::at(1.0)
        } else {
            sample("content", 0.0)
        };
        let x = add(
            MotionState::at(self.before[0]),
            scale(content, self.after[0] - self.before[0]),
        );
        let mut y = add(
            MotionState::at(self.before[1]),
            scale(layout, self.after[1] - self.before[1]),
        );
        if !expanded && self.keyed_lines {
            y = sample(&format!("line.{}.y", self.line_id), y.position);
        }
        TargetMotion {
            x: add(x, left),
            width: add(right, scale(left, -1.0)),
            line_y: y,
        }
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
