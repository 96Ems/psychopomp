use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LineId(String);

impl LineId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Hash)]
pub enum SyntaxStyle {
    Plain,
    Keyword,
    Type,
    String,
    Accent,
}

#[derive(Clone, Debug)]
pub struct StyledSpan {
    pub text: String,
    pub style: SyntaxStyle,
}

impl StyledSpan {
    pub fn new(text: impl Into<String>, style: SyntaxStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CodeLine {
    pub id: LineId,
    pub spans: Vec<StyledSpan>,
}

impl CodeLine {
    pub fn new(id: impl Into<String>, spans: Vec<StyledSpan>) -> Self {
        Self {
            id: LineId::new(id),
            spans,
        }
    }
}

pub struct CodeDocument {
    lines: HashMap<LineId, CodeLine>,
}

impl CodeDocument {
    pub fn new(lines: Vec<CodeLine>) -> Result<Self> {
        let mut by_id = HashMap::with_capacity(lines.len());
        for line in lines {
            let id = line.id.clone();
            if by_id.insert(id.clone(), line).is_some() {
                bail!("duplicate code line id '{}'", id.as_str());
            }
        }
        Ok(Self { lines: by_id })
    }
}

pub struct CodeSnapshot {
    order: Vec<LineId>,
}

impl CodeSnapshot {
    pub fn new(order: impl IntoIterator<Item = &'static str>) -> Self {
        Self {
            order: order.into_iter().map(LineId::new).collect(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct CodeLayout {
    pub line_height: f32,
    pub entering_offset_x: f32,
}

struct LineTrack {
    line: CodeLine,
    from_row: Option<usize>,
    to_row: Option<usize>,
}

pub struct CodeTransition {
    tracks: Vec<LineTrack>,
    layout: CodeLayout,
}

#[derive(Clone, Copy)]
pub struct TransitionProgress {
    pub layout: f32,
    pub content: f32,
}

pub struct PlacedLine<'a> {
    pub line: &'a CodeLine,
    pub x: f32,
    pub y: f32,
    pub opacity: f32,
}

impl CodeTransition {
    pub fn compile(
        document: &CodeDocument,
        before: &CodeSnapshot,
        after: &CodeSnapshot,
        layout: CodeLayout,
    ) -> Result<Self> {
        validate_snapshot(document, before)?;
        validate_snapshot(document, after)?;

        let before_rows: HashMap<_, _> = before
            .order
            .iter()
            .enumerate()
            .map(|(row, id)| (id.clone(), row))
            .collect();
        let after_rows: HashMap<_, _> = after
            .order
            .iter()
            .enumerate()
            .map(|(row, id)| (id.clone(), row))
            .collect();

        let mut manifest = after.order.clone();
        manifest.extend(
            before
                .order
                .iter()
                .filter(|id| !after_rows.contains_key(*id))
                .cloned(),
        );

        let tracks = manifest
            .into_iter()
            .map(|id| LineTrack {
                line: document.lines[&id].clone(),
                from_row: before_rows.get(&id).copied(),
                to_row: after_rows.get(&id).copied(),
            })
            .collect();

        Ok(Self { tracks, layout })
    }

    pub fn sample(&self, progress: TransitionProgress) -> Vec<PlacedLine<'_>> {
        self.tracks
            .iter()
            .map(|track| {
                let (x, row, opacity) = match (track.from_row, track.to_row) {
                    (Some(from), Some(to)) => {
                        (0.0, lerp(from as f32, to as f32, progress.layout), 1.0)
                    }
                    (None, Some(to)) => (
                        self.layout.entering_offset_x * (1.0 - progress.content),
                        to as f32,
                        progress.content.clamp(0.0, 1.0),
                    ),
                    (Some(from), None) => (
                        -self.layout.entering_offset_x * progress.content,
                        from as f32,
                        (1.0 - progress.content).clamp(0.0, 1.0),
                    ),
                    (None, None) => unreachable!("line track must appear in at least one snapshot"),
                };
                PlacedLine {
                    line: &track.line,
                    x,
                    y: row * self.layout.line_height,
                    opacity,
                }
            })
            .collect()
    }
}

fn validate_snapshot(document: &CodeDocument, snapshot: &CodeSnapshot) -> Result<()> {
    let mut seen = HashSet::with_capacity(snapshot.order.len());
    for id in &snapshot.order {
        if !document.lines.contains_key(id) {
            bail!("snapshot references unknown code line '{}'", id.as_str());
        }
        if !seen.insert(id) {
            bail!("snapshot repeats code line '{}'", id.as_str());
        }
    }
    Ok(())
}

fn lerp(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * progress
}

#[cfg(test)]
mod tests {
    use super::{
        CodeDocument, CodeLayout, CodeLine, CodeSnapshot, CodeTransition, StyledSpan, SyntaxStyle,
        TransitionProgress,
    };

    fn line(id: &str) -> CodeLine {
        CodeLine::new(id, vec![StyledSpan::new(id, SyntaxStyle::Plain)])
    }

    #[test]
    fn inserted_lines_move_in_without_replacing_stable_lines() {
        let document = CodeDocument::new(vec![line("a"), line("insert"), line("b")]).unwrap();
        let transition = CodeTransition::compile(
            &document,
            &CodeSnapshot::new(["a", "b"]),
            &CodeSnapshot::new(["a", "insert", "b"]),
            CodeLayout {
                line_height: 44.0,
                entering_offset_x: 100.0,
            },
        )
        .unwrap();

        let before = transition.sample(TransitionProgress {
            layout: 0.0,
            content: 0.0,
        });
        let after = transition.sample(TransitionProgress {
            layout: 1.0,
            content: 1.0,
        });
        let before_b = before
            .iter()
            .find(|line| line.line.id.as_str() == "b")
            .unwrap();
        let after_b = after
            .iter()
            .find(|line| line.line.id.as_str() == "b")
            .unwrap();
        let inserted_before = before
            .iter()
            .find(|line| line.line.id.as_str() == "insert")
            .unwrap();

        assert_eq!(before_b.y, 44.0);
        assert_eq!(after_b.y, 88.0);
        assert_eq!(inserted_before.x, 100.0);
        assert_eq!(inserted_before.opacity, 0.0);
    }
}
