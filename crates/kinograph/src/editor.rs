use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::code::{
    CodeDocument, CodeLayout, CodeLine, CodeSnapshot, CodeTransition, InlinePart, LogicalRange,
    SemanticRange, StyledSpan,
};

mod stability;
pub use stability::inspect_steps;

pub const EDITOR_RECIPE: &str = "editor";
pub const POINTER_RECIPE: &str = "pointer";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorRecipePlan {
    pub file_name: String,
    pub lines: Vec<EditorLinePlan>,
    pub initial_line_ids: Vec<String>,
    pub final_line_ids: Vec<String>,
    /// Optional multi-step line order. Empty preserves the legacy two-snapshot
    /// layout/content channels. Otherwise the last order must equal final_line_ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub snapshots: Vec<EditorSnapshotPlan>,
    pub line_height: f32,
    pub entering_offset_x: f32,
    pub focus_line_id: String,
    pub focus_height: f32,
    pub inline_reveal: EditorInlineRevealPlan,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_inline_reveals: Vec<EditorInlineRevealPlan>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSnapshotPlan {
    pub at_nanos: u64,
    pub line_ids: Vec<String>,
}

impl EditorRecipePlan {
    pub fn transition(&self) -> Result<CodeTransition> {
        if !self.line_height.is_finite()
            || self.line_height <= 0.0
            || !self.entering_offset_x.is_finite()
            || !self.focus_height.is_finite()
            || self.focus_height <= 0.0
        {
            bail!("editor line height must be positive and placement values finite");
        }
        let document = CodeDocument::new(
            self.lines
                .iter()
                .map(EditorLinePlan::code_line)
                .collect::<Result<Vec<_>>>()?,
        )?;
        let mut ranges: std::collections::HashMap<&str, Vec<std::ops::Range<usize>>> =
            std::collections::HashMap::new();
        for reveal in std::iter::once(&self.inline_reveal).chain(&self.additional_inline_reveals) {
            let line = self
                .lines
                .iter()
                .find(|line| line.id == reveal.line_id)
                .with_context(|| {
                    format!("inline reveal references unknown line '{}'", reveal.line_id)
                })?
                .code_line()?;
            let range = line.semantic_span_range(&crate::code::RangeId::new(&reveal.range_id))?;
            let previous = ranges.entry(&reveal.line_id).or_default();
            if previous
                .iter()
                .any(|old| old.start < range.end && range.start < old.end)
            {
                bail!("inline reveal ranges overlap on line '{}'", reveal.line_id);
            }
            previous.push(range);
        }
        for snapshot in &self.snapshots {
            CodeTransition::compile(
                &document,
                &CodeSnapshot::new(self.initial_line_ids.iter().cloned()),
                &CodeSnapshot::new(snapshot.line_ids.iter().cloned()),
                CodeLayout {
                    line_height: self.line_height,
                    entering_offset_x: 0.0,
                },
            )?;
        }
        if self
            .snapshots
            .last()
            .is_some_and(|snapshot| snapshot.line_ids != self.final_line_ids)
        {
            bail!("last editor snapshot must match finalLineIds");
        }
        let final_ids = if self.snapshots.is_empty() {
            self.final_line_ids.clone()
        } else {
            self.lines.iter().map(|line| line.id.clone()).collect()
        };
        if !self.initial_line_ids.contains(&self.focus_line_id)
            && !final_ids.contains(&self.focus_line_id)
        {
            bail!("editor focus references a line outside its snapshots");
        }
        CodeTransition::compile(
            &document,
            &CodeSnapshot::new(self.initial_line_ids.iter().cloned()),
            &CodeSnapshot::new(final_ids),
            CodeLayout {
                line_height: self.line_height,
                entering_offset_x: if self.snapshots.is_empty() {
                    self.entering_offset_x
                } else {
                    0.0
                },
            },
        )
    }

    /// Lower keyed snapshots into ordinary continuous channels so native
    /// navigation and video use the same per-line position/presence trajectories.
    pub fn snapshot_channels(
        &self,
        actor_id: &str,
        duration_nanos: u64,
    ) -> Result<Vec<crate::plan::ContinuousChannelPlan>> {
        use crate::plan::{ContinuousChannelPlan, TrackEventPlan};
        self.transition()?;
        if self.snapshots.is_empty() {
            return Ok(Vec::new());
        }
        let mut previous_time = 0;
        for snapshot in &self.snapshots {
            if snapshot.at_nanos < previous_time || snapshot.at_nanos > duration_nanos {
                bail!("editor snapshots must be ordered within scene duration");
            }
            previous_time = snapshot.at_nanos;
        }
        // Equal-time writes are one effective snapshot, not a transient layout.
        let snapshots = self
            .snapshots
            .iter()
            .enumerate()
            .filter_map(|(index, snapshot)| {
                self.snapshots
                    .get(index + 1)
                    .is_none_or(|next| next.at_nanos != snapshot.at_nanos)
                    .then_some(snapshot)
            })
            .collect::<Vec<_>>();
        let mut channels = Vec::new();
        for line in &self.lines {
            let initial_row = self.initial_line_ids.iter().position(|id| id == &line.id);
            let first_row = initial_row
                .or_else(|| {
                    snapshots
                        .iter()
                        .find_map(|snapshot| snapshot.line_ids.iter().position(|id| id == &line.id))
                })
                .unwrap_or(0);
            for (property, initial) in [
                (
                    format!("line.{}.y", line.id),
                    first_row as f32 * self.line_height,
                ),
                (
                    format!("line.{}.opacity", line.id),
                    if initial_row.is_some() { 1.0 } else { 0.0 },
                ),
            ] {
                let is_y = property.ends_with(".y");
                let mut current = initial;
                let mut events = Vec::new();
                for snapshot in &snapshots {
                    let row = snapshot.line_ids.iter().position(|id| id == &line.id);
                    let target = if is_y {
                        row.map_or(current, |row| row as f32 * self.line_height)
                    } else if row.is_some() {
                        1.0
                    } else {
                        0.0
                    };
                    if target == current {
                        continue;
                    }
                    events.push(TrackEventPlan::Spring {
                        at_nanos: snapshot.at_nanos,
                        target: target.into(),
                        response_seconds: 0.45 * 1.2,
                        damping_ratio: 1.0,
                        position_threshold: 0.001,
                        velocity_threshold: 0.001,
                    });
                    current = target;
                }
                channels.push(ContinuousChannelPlan {
                    id: format!("{actor_id}.{property}"),
                    actor_id: actor_id.into(),
                    property,
                    initial: initial.into(),
                    events,
                });
            }
        }
        Ok(channels)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorLinePlan {
    pub id: String,
    pub parts: Vec<EditorPartPlan>,
    #[serde(default)]
    pub semantic_ranges: Vec<EditorSemanticRangePlan>,
}

impl EditorLinePlan {
    fn code_line(&self) -> Result<CodeLine> {
        CodeLine::with_parts(
            self.id.clone(),
            self.parts.iter().map(EditorPartPlan::inline_part).collect(),
            self.semantic_ranges
                .iter()
                .map(EditorSemanticRangePlan::semantic_range),
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPartPlan {
    pub id: String,
    pub spans: Vec<StyledSpan>,
}

impl EditorPartPlan {
    fn inline_part(&self) -> InlinePart {
        InlinePart::new(self.id.clone(), self.spans.clone())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSemanticRangePlan {
    pub id: String,
    pub first_part_id: String,
    pub last_part_id: String,
}

impl EditorSemanticRangePlan {
    fn semantic_range(&self) -> SemanticRange {
        SemanticRange::new(
            self.id.clone(),
            LogicalRange::spanning(self.first_part_id.clone(), self.last_part_id.clone()),
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorInlineRevealPlan {
    pub line_id: String,
    pub range_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub reversed: bool,
}

impl EditorInlineRevealPlan {
    pub fn channel(&self) -> &str {
        self.channel.as_deref().unwrap_or("inline-reveal")
    }

    pub fn progress(&self, value: f32) -> f32 {
        if self.reversed { 1.0 - value } else { value }
    }
}

fn is_false(value: &bool) -> bool {
    !value
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorTargetSelector {
    pub line_id: String,
    pub range_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PointerRecipePlan {
    pub editor_id: String,
}

#[cfg(test)]
mod tests {
    use crate::code::{StyledSpan, SyntaxStyle, TransitionProgress};

    use super::{
        EditorInlineRevealPlan, EditorLinePlan, EditorPartPlan, EditorRecipePlan,
        EditorSemanticRangePlan,
    };

    #[test]
    fn editor_recipe_compiles_stable_code_transition() {
        let recipe = EditorRecipePlan {
            file_name: "demo.ts".to_owned(),
            lines: vec![
                EditorLinePlan {
                    id: "stable".to_owned(),
                    parts: vec![EditorPartPlan {
                        id: "content".to_owned(),
                        spans: vec![StyledSpan::new("const stable = true", SyntaxStyle::Plain)],
                    }],
                    semantic_ranges: vec![EditorSemanticRangePlan {
                        id: "content".to_owned(),
                        first_part_id: "content".to_owned(),
                        last_part_id: "content".to_owned(),
                    }],
                },
                EditorLinePlan {
                    id: "entering".to_owned(),
                    parts: vec![EditorPartPlan {
                        id: "content".to_owned(),
                        spans: vec![StyledSpan::new("const entering = true", SyntaxStyle::Plain)],
                    }],
                    semantic_ranges: vec![],
                },
            ],
            initial_line_ids: vec!["stable".to_owned()],
            final_line_ids: vec!["stable".to_owned(), "entering".to_owned()],
            snapshots: Vec::new(),
            line_height: 44.0,
            entering_offset_x: 96.0,
            focus_line_id: "stable".to_owned(),
            focus_height: 44.0,
            inline_reveal: EditorInlineRevealPlan {
                line_id: "stable".to_owned(),
                range_id: "content".to_owned(),
                channel: None,
                reversed: false,
            },
            additional_inline_reveals: Vec::new(),
        };

        let transition = recipe.transition().unwrap();
        assert_eq!(
            transition
                .sample(TransitionProgress {
                    layout: 1.0,
                    content: 1.0,
                })
                .len(),
            2
        );
    }

    #[test]
    fn inline_reveals_can_share_one_channel_in_opposing_directions() {
        let entering = EditorInlineRevealPlan {
            line_id: "line".to_owned(),
            range_id: "new".to_owned(),
            channel: Some("api-change".to_owned()),
            reversed: false,
        };
        let exiting = EditorInlineRevealPlan {
            line_id: "line".to_owned(),
            range_id: "old".to_owned(),
            channel: Some("api-change".to_owned()),
            reversed: true,
        };

        assert_eq!(entering.channel(), "api-change");
        assert_eq!(entering.progress(0.25), 0.25);
        assert_eq!(exiting.progress(0.25), 0.75);
    }

    #[test]
    fn keyed_snapshots_retain_intermediate_lines_and_unchanged_targets() {
        let line = |id: &str| EditorLinePlan {
            id: id.into(),
            parts: vec![EditorPartPlan {
                id: "body".into(),
                spans: vec![StyledSpan::new(id, SyntaxStyle::Plain)],
            }],
            semantic_ranges: vec![EditorSemanticRangePlan {
                id: "body".into(),
                first_part_id: "body".into(),
                last_part_id: "body".into(),
            }],
        };
        let mut recipe = EditorRecipePlan {
            file_name: "test.ts".into(),
            lines: vec![line("a"), line("insert"), line("b")],
            initial_line_ids: vec!["a".into(), "b".into()],
            final_line_ids: vec!["a".into(), "b".into()],
            snapshots: vec![
                super::EditorSnapshotPlan {
                    at_nanos: 1_000_000_000,
                    line_ids: vec!["a".into(), "insert".into(), "b".into()],
                },
                super::EditorSnapshotPlan {
                    at_nanos: 2_000_000_000,
                    line_ids: vec!["b".into(), "insert".into()],
                },
                super::EditorSnapshotPlan {
                    at_nanos: 3_000_000_000,
                    line_ids: vec!["a".into(), "b".into()],
                },
            ],
            line_height: 44.,
            entering_offset_x: 99.,
            focus_line_id: "a".into(),
            focus_height: 44.,
            inline_reveal: EditorInlineRevealPlan {
                line_id: "a".into(),
                range_id: "body".into(),
                channel: None,
                reversed: false,
            },
            additional_inline_reveals: Vec::new(),
        };
        let transition = recipe.transition().unwrap();
        assert_eq!(
            transition
                .sample(TransitionProgress {
                    layout: 0.,
                    content: 0.
                })
                .len(),
            3
        );
        let channels = recipe.snapshot_channels("editor", 4_000_000_000).unwrap();
        assert_eq!(channels.len(), 6);
        let insert_y = channels
            .iter()
            .find(|channel| channel.property == "line.insert.y")
            .unwrap();
        assert!(insert_y.events.is_empty()); // entering/exiting must not restart an unchanged row
        assert!(matches!(
            insert_y.initial,
            crate::plan::ScalarPlan::Literal(44.)
        ));
        let b = channels
            .iter()
            .find(|channel| channel.property == "line.b.y")
            .unwrap();
        assert_eq!(b.events.len(), 3);
        let canonical = recipe.snapshot_channels("editor", 4_000_000_000).unwrap();
        recipe.snapshots.insert(
            1,
            super::EditorSnapshotPlan {
                at_nanos: 2_000_000_000,
                line_ids: vec!["insert".into(), "a".into(), "b".into()],
            },
        );
        assert_eq!(
            serde_json::to_value(recipe.snapshot_channels("editor", 4_000_000_000).unwrap())
                .unwrap(),
            serde_json::to_value(canonical).unwrap()
        );
        recipe.snapshots.remove(1);
        recipe.snapshots[1].at_nanos = 5_000_000_000;
        assert!(recipe.snapshot_channels("editor", 4_000_000_000).is_err());
        recipe.snapshots[1].at_nanos = 2_000_000_000;
        recipe.snapshots[1].line_ids.push("missing".into());
        assert!(recipe.snapshot_channels("editor", 4_000_000_000).is_err());
    }
}
