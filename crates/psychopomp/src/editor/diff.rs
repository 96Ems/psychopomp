//! Stepped Diffs: a unified-diff view of one change, told in steps. Each line
//! says in which step it appears and, optionally, in which step it is removed.
//! Lines keep identity across steps, so unchanged code only moves; removed lines
//! turn red before they go, and added lines arrive green.
use anyhow::Result;

use super::{
    EDITOR_RECIPE, EditorLinePlan, EditorPartPlan, EditorRecipePlan, EditorSnapshotPlan,
    LineMarkPlan,
};
use crate::{
    author::PlanBuilder,
    code::{StyledSpan, SyntaxStyle},
    highlight,
};

/// The widest line that fits the editor card at 28 px CommitMono.
pub const MAX_COLUMNS: usize = 76;
/// Rows visible in the editor card.
pub const MAX_ROWS: usize = 14;
/// How long a pure insertion or removal holds blank rows, so moving code never
/// crosses entering or leaving code.
const ROOM: u64 = 300_000_000;

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

    /// Declare the `editor` actor. `step_times[k]` is when step `k + 1` happens; removed
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
        let mut snapshots = Vec::new();
        let mut gaps = 0;
        for (step, &at) in step_times.iter().enumerate() {
            let next = self.rows(step + 1);
            let mut settle = at;
            if let Some(room) = make_room(&self.rows(step), &next) {
                gaps = gaps.max(room.iter().filter(|id| id.starts_with("gap-")).count());
                snapshots.push(EditorSnapshotPlan {
                    at_nanos: at,
                    line_ids: room,
                });
                settle += ROOM;
            }
            snapshots.push(EditorSnapshotPlan {
                at_nanos: settle,
                line_ids: next,
            });
        }
        let lines = self
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let mark = if line.until.is_some() {
                    Some(LineMarkPlan::Removed)
                } else if line.from > 0 {
                    Some(LineMarkPlan::Added)
                } else {
                    None
                };
                editor_line(id(index), line.text, mark)
            })
            .chain((0..gaps).map(|gap| editor_line(format!("gap-{gap}"), "", None)))
            .collect::<Vec<_>>();
        let initial = self.rows(0);
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

/// A pure insertion first opens blank rows where the new lines go; a pure
/// removal first leaves blank rows where the old lines were. Mixed steps and
/// steps where no retained line moves need no room.
fn make_room(previous: &[String], next: &[String]) -> Option<Vec<String>> {
    let entering = next.iter().filter(|id| !previous.contains(id)).count();
    let leaving = previous.iter().filter(|id| !next.contains(id)).count();
    let (base, other) = match (entering, leaving) {
        (0, 0) => return None,
        (_, 0) => (next, previous),
        (0, _) => (previous, next),
        _ => return None,
    };
    let row = |ids: &[String], id: &String| ids.iter().position(|candidate| candidate == id);
    if !next
        .iter()
        .any(|id| row(previous, id).is_some_and(|from| Some(from) != row(next, id)))
    {
        return None;
    }
    let mut gap = 0;
    Some(
        base.iter()
            .map(|id| {
                if other.contains(id) {
                    id.clone()
                } else {
                    gap += 1;
                    format!("gap-{}", gap - 1)
                }
            })
            .collect(),
    )
}

fn editor_line(id: String, text: &str, mark: Option<LineMarkPlan>) -> EditorLinePlan {
    let spans = if text.trim().is_empty() {
        vec![StyledSpan::new(" ", SyntaxStyle::Plain)]
    } else {
        highlight::typescript(text)
    };
    EditorLinePlan {
        id,
        parts: vec![EditorPartPlan {
            id: "code".to_owned(),
            spans,
        }],
        semantic_ranges: Vec::new(),
        mark,
    }
}

fn id(index: usize) -> String {
    format!("line-{index}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pure_insertion_opens_room_before_the_new_lines_arrive() {
        let diff = Diff {
            file_name: "a.ts",
            lines: vec![keep("a"), add(1, "b"), keep("c")],
        };
        let mut scene = PlanBuilder::new("diff", 2_000_000_000);
        diff.declare(&mut scene, &[1_000_000_000], 0, false)
            .unwrap();
        let plan = scene.finish().unwrap();
        let recipe: EditorRecipePlan = serde_json::from_value(plan.actors[0].data.clone()).unwrap();
        let snapshots = recipe
            .snapshots
            .iter()
            .map(|snapshot| (snapshot.at_nanos, snapshot.line_ids.join(" ")))
            .collect::<Vec<_>>();
        assert_eq!(
            snapshots,
            vec![
                (1_000_000_000, "line-0 gap-0 line-2".to_owned()),
                (1_000_000_000 + ROOM, "line-0 line-1 line-2".to_owned()),
            ]
        );
        assert_eq!(recipe.lines[1].mark, Some(LineMarkPlan::Added));
    }

    #[test]
    fn a_step_count_mismatch_is_rejected() {
        let diff = Diff {
            file_name: "a.ts",
            lines: vec![keep("a"), remove(2, "b")],
        };
        let mut scene = PlanBuilder::new("diff", 2_000_000_000);
        assert!(diff.declare(&mut scene, &[1], 0, false).is_err());
    }
}
