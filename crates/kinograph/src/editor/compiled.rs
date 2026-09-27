//! One validated editor identity/placement model, shared by inspection and paint
//! preparation. Glyph metrics and attachment derivatives remain renderer-owned.
use super::{EditorInlineRevealPlan, EditorRecipePlan, EditorSnapshotPlan};
use crate::{
    code::{
        CodeDocument, CodeLayout, CodeLine, CodeSnapshot, CodeTransition, LineId, PlacedLine,
        TransitionProgress,
    },
    plan::{ContinuousChannelPlan, SpringPlan, destination_channel, effective_snapshots},
};
use anyhow::{Result, bail};
use std::ops::Range;

pub struct CompiledEditor {
    document: CodeDocument,
    catalog: Vec<LineId>,
    placement: Placement,
    reveals: Vec<CompiledInlineReveal>,
    file_name: String,
    line_height: f32,
    focus_line_id: String,
    focus_height: f32,
}

enum Placement {
    Legacy(CodeTransition),
    Keyed {
        initial: CodeSnapshot,
        snapshots: Vec<EditorSnapshotPlan>,
        lines: Vec<KeyedLine>,
    },
}
struct KeyedLine {
    initial_row: Option<usize>,
    first_row: usize,
    y: String,
    opacity: String,
}

pub struct CompiledInlineReveal {
    pub plan: EditorInlineRevealPlan,
    pub spans: Range<usize>,
    pub parts: Range<usize>,
}

impl CompiledEditor {
    pub(super) fn new(
        recipe: &EditorRecipePlan,
        document: CodeDocument,
        reveals: Vec<CompiledInlineReveal>,
    ) -> Result<Self> {
        let catalog = recipe
            .lines
            .iter()
            .map(|line| LineId::new(&line.id))
            .collect::<Vec<_>>();
        let initial = CodeSnapshot::new(recipe.initial_line_ids.iter().cloned());
        let placement = if recipe.snapshots.is_empty() {
            Placement::Legacy(CodeTransition::compile(
                &document,
                &initial,
                &CodeSnapshot::new(recipe.final_line_ids.iter().cloned()),
                CodeLayout {
                    line_height: recipe.line_height,
                    entering_offset_x: recipe.entering_offset_x,
                },
            )?)
        } else {
            let snapshots =
                effective_snapshots(&recipe.snapshots, |s| s.at_nanos).collect::<Vec<_>>();
            let lines = catalog
                .iter()
                .map(|id| {
                    let initial_row = recipe
                        .initial_line_ids
                        .iter()
                        .position(|candidate| candidate == id.as_str());
                    let first_row = initial_row
                        .or_else(|| {
                            snapshots.iter().find_map(|snapshot| {
                                snapshot
                                    .line_ids
                                    .iter()
                                    .position(|candidate| candidate == id.as_str())
                            })
                        })
                        .unwrap_or(0);
                    KeyedLine {
                        initial_row,
                        first_row,
                        y: format!("line.{}.y", id.as_str()),
                        opacity: format!("line.{}.opacity", id.as_str()),
                    }
                })
                .collect();
            Placement::Keyed {
                initial,
                snapshots: recipe.snapshots.clone(),
                lines,
            }
        };
        Ok(Self {
            document,
            catalog,
            placement,
            reveals,
            file_name: recipe.file_name.clone(),
            line_height: recipe.line_height,
            focus_line_id: recipe.focus_line_id.clone(),
            focus_height: recipe.focus_height,
        })
    }

    pub fn file_name(&self) -> &str {
        &self.file_name
    }
    pub fn line_height(&self) -> f32 {
        self.line_height
    }
    pub fn focus_line_id(&self) -> &str {
        &self.focus_line_id
    }
    pub fn focus_height(&self) -> f32 {
        self.focus_height
    }
    pub fn is_keyed(&self) -> bool {
        matches!(&self.placement, Placement::Keyed { .. })
    }
    pub fn lines(&self) -> impl ExactSizeIterator<Item = &CodeLine> {
        self.catalog
            .iter()
            .map(|id| self.document.line(id).expect("compiled catalog line"))
    }
    pub fn line(&self, id: &str) -> Option<&CodeLine> {
        self.document.line(&LineId::new(id))
    }
    pub fn inline_reveals(&self) -> &[CompiledInlineReveal] {
        &self.reveals
    }

    /// Preserve the expanded two-endpoint attachment baseline. Keyed "after"
    /// means catalog order, NOT the final snapshot or first-visible row.
    pub fn line_endpoints(&self, id: &str) -> Option<[[f32; 2]; 2]> {
        match &self.placement {
            Placement::Legacy(transition) => {
                let id = LineId::new(id);
                let before = transition.sample_line(
                    &id,
                    TransitionProgress {
                        layout: 0.,
                        content: 0.,
                    },
                )?;
                let after = transition.sample_line(
                    &id,
                    TransitionProgress {
                        layout: 1.,
                        content: 1.,
                    },
                )?;
                Some([[before.x, before.y], [after.x, after.y]])
            }
            Placement::Keyed { lines, .. } => {
                let row = self
                    .catalog
                    .iter()
                    .position(|candidate| candidate.as_str() == id)?;
                Some([
                    [
                        0.,
                        lines[row].initial_row.unwrap_or(row) as f32 * self.line_height,
                    ],
                    [0., row as f32 * self.line_height],
                ])
            }
        }
    }

    pub fn sample_lines(&self, sample: impl Fn(&str, f32) -> f32) -> Vec<PlacedLine<'_>> {
        let progress = TransitionProgress {
            layout: sample("layout", 0.),
            content: sample("content", 0.),
        };
        match &self.placement {
            Placement::Legacy(transition) => transition.sample(progress),
            Placement::Keyed { lines, .. } => self
                .catalog
                .iter()
                .zip(lines)
                .enumerate()
                .map(|(row, (id, keyed))| {
                    let fallback = keyed.initial_row.map_or(row as f32, |from| {
                        from as f32 + (row as f32 - from as f32) * progress.layout
                    });
                    let presence = if keyed.initial_row.is_some() {
                        1.
                    } else {
                        progress.content.clamp(0., 1.)
                    };
                    let opacity = sample(&keyed.opacity, presence).clamp(0., 1.);
                    PlacedLine {
                        line: self.document.line(id).expect("compiled catalog line"),
                        x: 0.,
                        y: sample(&keyed.y, fallback * self.line_height),
                        opacity,
                        blur: (1. - opacity) * 4.,
                    }
                })
                .collect(),
        }
    }

    /// Diagnostic participation, not exact clipped-glyph visibility.
    pub fn part_presence(
        &self,
        id: &LineId,
        sample: impl Fn(&str, f32) -> f32,
    ) -> Option<Vec<f32>> {
        let mut parts = vec![1.; self.document.line(id)?.parts().len()];
        for reveal in self
            .reveals
            .iter()
            .filter(|reveal| reveal.plan.line_id == id.as_str())
        {
            parts[reveal.parts.clone()].fill(
                reveal
                    .plan
                    .progress(sample(reveal.plan.channel(), 0.))
                    .clamp(0., 1.),
            );
        }
        Some(parts)
    }

    pub fn snapshot_channels(
        &self,
        actor_id: &str,
        duration_nanos: u64,
    ) -> Result<Vec<ContinuousChannelPlan>> {
        let Placement::Keyed {
            snapshots, lines, ..
        } = &self.placement
        else {
            return Ok(Vec::new());
        };
        let mut previous = 0;
        for snapshot in snapshots {
            if snapshot.at_nanos < previous || snapshot.at_nanos > duration_nanos {
                bail!("editor snapshots must be ordered within scene duration");
            }
            previous = snapshot.at_nanos;
        }
        let snapshots = effective_snapshots(snapshots, |s| s.at_nanos).collect::<Vec<_>>();
        let mut channels = Vec::new();
        for (id, line) in self.catalog.iter().zip(lines) {
            for (property, is_y, initial) in [
                (
                    line.y.clone(),
                    true,
                    line.first_row as f32 * self.line_height,
                ),
                (
                    line.opacity.clone(),
                    false,
                    if line.initial_row.is_some() { 1. } else { 0. },
                ),
            ] {
                let mut current = initial;
                let targets = snapshots.iter().map(|s| {
                    let row = s
                        .line_ids
                        .iter()
                        .position(|candidate| candidate == id.as_str());
                    current = if is_y {
                        row.map_or(current, |row| row as f32 * self.line_height)
                    } else {
                        f32::from(row.is_some())
                    };
                    (s.at_nanos, current)
                });
                channels.push(destination_channel(
                    actor_id,
                    property,
                    initial,
                    targets,
                    |_, _| SpringPlan::visual(0.45, 0.),
                ));
            }
        }
        Ok(channels)
    }

    /// Existing public compatibility entrypoint only. Real keyed consumers use
    /// sample_lines rather than a fabricated initial-to-catalog transition.
    pub(super) fn into_transition(self) -> Result<CodeTransition> {
        match self.placement {
            Placement::Legacy(transition) => Ok(transition),
            Placement::Keyed { initial, .. } => CodeTransition::compile(
                &self.document,
                &initial,
                &CodeSnapshot::new(self.catalog.iter().map(|id| id.as_str().to_owned())),
                CodeLayout {
                    line_height: self.line_height,
                    entering_offset_x: 0.,
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests;
