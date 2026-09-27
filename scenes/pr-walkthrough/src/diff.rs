//! A unified-diff view of one change, told in steps. Each line says in which step
//! it appears and, optionally, in which step it is removed. Lines keep identity
//! across steps, so unchanged code only moves; removed lines turn red before they
//! go, and added lines arrive green.
use anyhow::Result;
use kinograph::{
    author::PlanBuilder,
    code::StyledSpan,
    editor::{
        EDITOR_RECIPE, EditorLinePlan, EditorPartPlan, EditorRecipePlan, EditorSnapshotPlan,
        LineMarkPlan,
    },
    highlight,
};

/// The widest line that fits the editor card at 28 px CommitMono.
pub const MAX_COLUMNS: usize = 76;
/// Rows visible in the editor card.
pub const MAX_ROWS: usize = 14;

pub struct Line {
    pub text: &'static str,
    pub from: usize,
    pub until: Option<usize>,
}

/// Present from the start and never removed.
pub const fn keep(text: &'static str) -> Line {
    Line {
        text,
        from: 0,
        until: None,
    }
}

/// Added in `step`.
pub const fn add(step: usize, text: &'static str) -> Line {
    Line {
        text,
        from: step,
        until: None,
    }
}

/// Present from the start and removed in `step`.
pub const fn remove(step: usize, text: &'static str) -> Line {
    Line {
        text,
        from: 0,
        until: Some(step),
    }
}

pub struct Diff {
    pub file_name: &'static str,
    pub lines: Vec<Line>,
}

impl Diff {
    fn steps(&self) -> usize {
        self.lines
            .iter()
            .flat_map(|line| [line.from, line.until.unwrap_or(0)])
            .max()
            .unwrap_or(0)
    }

    fn rows(&self, step: usize) -> Vec<String> {
        self.lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.from <= step && line.until.is_none_or(|until| step < until))
            .map(|(index, _)| id(index))
            .collect()
    }

    /// Declare the editor. `step_times[k]` is when step `k + 1` happens; removed
    /// lines turn red `warning` nanoseconds before their step.
    pub fn declare(
        &self,
        scene: &mut PlanBuilder,
        step_times: &[u64],
        warning: u64,
        entrance: bool,
    ) -> Result<()> {
        let steps = self.steps();
        anyhow::ensure!(
            step_times.len() == steps,
            "{} has {steps} steps but {} step times",
            self.file_name,
            step_times.len()
        );
        for (index, line) in self.lines.iter().enumerate() {
            anyhow::ensure!(
                line.text.chars().count() <= MAX_COLUMNS,
                "{} line {index} is wider than the editor: {}",
                self.file_name,
                line.text
            );
        }
        for step in 0..=steps {
            let rows = self.rows(step).len();
            anyhow::ensure!(
                rows <= MAX_ROWS,
                "{} step {step} has {rows} rows",
                self.file_name
            );
        }
        let lines = self
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let spans = if line.text.trim().is_empty() {
                    vec![StyledSpan::new(" ", kinograph::code::SyntaxStyle::Plain)]
                } else {
                    highlight::typescript(line.text)
                };
                EditorLinePlan {
                    id: id(index),
                    parts: vec![EditorPartPlan {
                        id: "code".to_owned(),
                        spans,
                    }],
                    semantic_ranges: Vec::new(),
                    mark: if line.until.is_some() {
                        Some(LineMarkPlan::Removed)
                    } else if line.from > 0 {
                        Some(LineMarkPlan::Added)
                    } else {
                        None
                    },
                }
            })
            .collect::<Vec<_>>();
        let initial = self.rows(0);
        let snapshots = step_times
            .iter()
            .enumerate()
            .map(|(step, at)| EditorSnapshotPlan {
                at_nanos: *at,
                line_ids: self.rows(step + 1),
            })
            .collect::<Vec<_>>();
        let final_ids = self.rows(steps);
        let recipe = EditorRecipePlan {
            file_name: self.file_name.to_owned(),
            focus_line_id: initial.first().cloned().unwrap_or_else(|| id(0)),
            lines,
            initial_line_ids: initial,
            final_line_ids: final_ids,
            snapshots,
            line_height: 44.0,
            entering_offset_x: 0.0,
            focus_height: 44.0,
            inline_reveal: None,
            additional_inline_reveals: Vec::new(),
        };
        let editor = scene.actor("editor", EDITOR_RECIPE, &recipe)?;
        // Removed lines start unmarked and turn red just before they leave.
        for (index, line) in self.lines.iter().enumerate() {
            if let Some(until) = line.until {
                let mark = scene.continuous(&editor, format!("mark.{}", id(index)), 0.0);
                let at = step_times[until - 1];
                scene.spring(&mark, at.saturating_sub(warning), 1.0, 0.35, 0.0);
            }
        }
        if entrance {
            // The card rises into place.
            let y = scene.continuous(&editor, "panel-y", 70.0);
            let opacity = scene.continuous(&editor, "panel-opacity", 0.0);
            scene.spring(&y, 0, 0.0, 0.7, 0.0);
            scene.spring(&opacity, 0, 1.0, 0.5, 0.0);
        }
        Ok(())
    }
}

fn id(index: usize) -> String {
    format!("line-{index}")
}
