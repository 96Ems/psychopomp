use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::code::{
    CodeDocument, CodeLayout, CodeLine, CodeSnapshot, CodeTransition, InlinePart, LogicalRange,
    SemanticRange, StyledSpan,
};

pub const EDITOR_RECIPE: &str = "editor";
pub const POINTER_RECIPE: &str = "pointer";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorRecipePlan {
    pub file_name: String,
    pub lines: Vec<EditorLinePlan>,
    pub initial_line_ids: Vec<String>,
    pub final_line_ids: Vec<String>,
    pub line_height: f32,
    pub entering_offset_x: f32,
    pub focus_line_id: String,
    pub focus_height: f32,
    pub inline_reveal: EditorInlineRevealPlan,
}

impl EditorRecipePlan {
    pub fn transition(&self) -> Result<CodeTransition> {
        let document = CodeDocument::new(
            self.lines
                .iter()
                .map(EditorLinePlan::code_line)
                .collect::<Result<Vec<_>>>()?,
        )?;
        CodeTransition::compile(
            &document,
            &CodeSnapshot::new(self.initial_line_ids.iter().cloned()),
            &CodeSnapshot::new(self.final_line_ids.iter().cloned()),
            CodeLayout {
                line_height: self.line_height,
                entering_offset_x: self.entering_offset_x,
            },
        )
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
            line_height: 44.0,
            entering_offset_x: 96.0,
            focus_line_id: "stable".to_owned(),
            focus_height: 44.0,
            inline_reveal: EditorInlineRevealPlan {
                line_id: "stable".to_owned(),
                range_id: "content".to_owned(),
            },
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
}
