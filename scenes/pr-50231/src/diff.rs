//! The change as an editor card, condensed for display. New lines are marked
//! added from the start, since the zoom opens straight onto them; slot-sized
//! edits are inline reveals applied afterward (see `permissions::swap_call_site`).
use anyhow::Result;
use psychopomp::{
    author::PlanBuilder,
    code::StyledSpan,
    editor::{EDITOR_RECIPE, EditorLinePlan, EditorRecipePlan, LineMarkPlan, part},
    highlight,
};

/// The widest line that fits the editor card at 28 px CommitMono.
pub const MAX_COLUMNS: usize = 76;
/// Rows visible in the editor card.
pub const MAX_ROWS: usize = 14;

pub struct Line {
    pub text: &'static str,
    pub fresh: bool,
}

/// Unchanged context.
pub const fn keep(text: &'static str) -> Line {
    Line { text, fresh: false }
}

/// Code the change introduces, marked added.
pub const fn fresh(text: &'static str) -> Line {
    Line { text, fresh: true }
}

pub struct Diff {
    pub file_name: &'static str,
    pub lines: Vec<Line>,
}

impl Diff {
    pub fn declare(&self, scene: &mut PlanBuilder) -> Result<()> {
        anyhow::ensure!(
            self.lines.len() <= MAX_ROWS,
            "{} has {} rows",
            self.file_name,
            self.lines.len()
        );
        for (index, line) in self.lines.iter().enumerate() {
            anyhow::ensure!(
                line.text.chars().count() <= MAX_COLUMNS,
                "{} line {index} is wider than the editor: {}",
                self.file_name,
                line.text
            );
        }
        let ids = (0..self.lines.len()).map(id).collect::<Vec<_>>();
        let recipe = EditorRecipePlan {
            file_name: self.file_name.to_owned(),
            focus_line_id: id(0),
            lines: self
                .lines
                .iter()
                .enumerate()
                .map(|(index, line)| editor_line(id(index), line))
                .collect(),
            initial_line_ids: ids.clone(),
            final_line_ids: ids,
            snapshots: Vec::new(),
            line_height: 44.0,
            entering_offset_x: 0.0,
            focus_height: 44.0,
            inline_reveal: None,
            additional_inline_reveals: Vec::new(),
        };
        scene.actor("editor", EDITOR_RECIPE, &recipe)?;
        Ok(())
    }
}

fn editor_line(id: String, line: &Line) -> EditorLinePlan {
    let spans = if line.text.trim().is_empty() {
        vec![StyledSpan::new(" ", psychopomp::code::SyntaxStyle::Plain)]
    } else {
        highlight::typescript(line.text)
    };
    EditorLinePlan {
        id,
        parts: vec![part("code", spans)],
        semantic_ranges: Vec::new(),
        mark: line.fresh.then_some(LineMarkPlan::Added),
    }
}

fn id(index: usize) -> String {
    format!("line-{index}")
}
