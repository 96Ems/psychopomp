//! Authoring diagnostics, not automatic identity inference. No fonts or GPU.
use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use super::EditorRecipePlan;
use crate::{
    code::{RangeId, TransitionProgress},
    plan::{ScalarPlan, ScenePlan, TrackEventPlan},
    timeline::{PropertyId, SpringProfile, TimedEvent, Timeline},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepInspection {
    steps: Vec<StepDelta>,
    warnings: Vec<StabilityWarning>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StepDelta {
    id: String,
    title: String,
    editors: Vec<EditorDelta>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorDelta {
    actor_id: String,
    lines: Vec<LineDelta>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LineDelta {
    id: String,
    before: Option<String>,
    after: Option<String>,
    before_delta: Option<String>,
    delta: String,
    changed_part_ids: Vec<String>,
    moved: bool,
    before_y: Option<f32>,
    after_y: Option<f32>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StabilityWarning {
    code: &'static str,
    step_id: String,
    actor_id: String,
    line_ids: Vec<String>,
    common_text: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    channel_ids: Vec<String>,
    suggestion: &'static str,
}
struct Pose {
    row: f32,
    presence: f32,
    parts: Vec<f32>,
}

pub fn inspect_steps(plan: &ScenePlan) -> Result<StepInspection> {
    plan.validate()?;
    if plan.presentation_steps.is_empty() {
        bail!("code step inspection requires presentationSteps");
    }
    let mut result = StepInspection {
        steps: plan
            .presentation_steps
            .iter()
            .map(|step| StepDelta {
                id: step.id.clone(),
                title: step.title.clone(),
                editors: Vec::new(),
            })
            .collect(),
        warnings: Vec::new(),
    };
    for actor in plan
        .actors
        .iter()
        .filter(|actor| actor.recipe == super::EDITOR_RECIPE)
    {
        let recipe: EditorRecipePlan = serde_json::from_value(actor.data.clone())
            .context("parse editor for step inspection")?;
        let transition = recipe.transition()?;
        let mut channels = plan
            .continuous_channels
            .iter()
            .filter(|channel| channel.actor_id == actor.id)
            .cloned()
            .collect::<Vec<_>>();
        for channel in recipe.snapshot_channels(&actor.id, plan.duration_nanos)? {
            if channels
                .iter()
                .any(|existing| existing.property == channel.property)
            {
                bail!(
                    "authored channel collides with generated line channel '{}'",
                    channel.id
                );
            }
            channels.push(channel);
        }
        // Only geometry/reveal channels are relevant. Other channels may contain
        // renderer-resolved semantic scalars which do not change the code delta.
        let relevant = |property: &str| {
            property == "layout"
                || property == "content"
                || property.starts_with("line.")
                || std::iter::once(&recipe.inline_reveal)
                    .chain(&recipe.additional_inline_reveals)
                    .any(|reveal| reveal.channel() == property)
        };
        let literal = |value: &ScalarPlan| match value {
            ScalarPlan::Literal(value) => Ok(*value),
            _ => anyhow::bail!("code geometry channels must use literal values"),
        };
        let mut initials = Vec::new();
        let mut events = Vec::new();
        for channel in channels
            .iter()
            .filter(|channel| relevant(&channel.property))
        {
            let id = PropertyId::new(&channel.property);
            initials.push((id.clone(), literal(&channel.initial)?));
            for event in &channel.events {
                events.push(match event {
                    TrackEventPlan::Set { at_nanos, value } => {
                        TimedEvent::set(*at_nanos as f64 / 1e9, id.clone(), literal(value)?)
                    }
                    TrackEventPlan::Spring {
                        at_nanos,
                        target,
                        response_seconds,
                        damping_ratio,
                        position_threshold,
                        velocity_threshold,
                    } => TimedEvent::spring(
                        *at_nanos as f64 / 1e9,
                        id.clone(),
                        literal(target)?,
                        SpringProfile::new(
                            *response_seconds,
                            *damping_ratio,
                            *position_threshold,
                            *velocity_threshold,
                        ),
                    ),
                });
            }
        }
        let timeline =
            Timeline::compile_events(initials, events, plan.duration_nanos as f64 / 1e9)?;
        let mut previous: HashMap<String, Pose> = HashMap::new();
        for (step_index, step) in plan.presentation_steps.iter().enumerate() {
            let time = step.hold_nanos as f64 / 1e9;
            let unsettled = channels
                .iter()
                .filter(|channel| relevant(&channel.property))
                .filter(|channel| {
                    timeline
                        .sample_at(&PropertyId::new(&channel.property), time)
                        .is_some_and(|state| {
                            let presence = channel.property == "content"
                                || channel.property.ends_with(".opacity")
                                || std::iter::once(&recipe.inline_reveal)
                                    .chain(&recipe.additional_inline_reveals)
                                    .any(|reveal| reveal.channel() == channel.property);
                            state.velocity != 0.0
                                || (presence && state.position != 0.0 && state.position != 1.0)
                        })
                })
                .map(|channel| channel.id.clone())
                .collect::<Vec<_>>();
            if !unsettled.is_empty() {
                result.warnings.push(StabilityWarning { code: "unsettled-code-step", step_id: step.id.clone(), actor_id: actor.id.clone(), line_ids: Vec::new(), common_text: String::new(), channel_ids: unsettled,
                    suggestion: "This hold has moving or partially visible code. Delta text lists participating parts, not the exact clipped glyphs; choose a settled hold before comparing semantic replacements." });
            }
            let value = |property: &str, default| {
                timeline
                    .sample_at(&PropertyId::new(property), time)
                    .map_or(default, |state| state.position)
            };
            let placed = transition.sample(TransitionProgress {
                layout: value("layout", 0.),
                content: value("content", 0.),
            });
            let mut lines = Vec::new();
            let mut exits = Vec::new();
            let mut enters = Vec::new();
            for line in &recipe.lines {
                let code = line.code_line()?;
                let placement = placed
                    .iter()
                    .find(|placed| placed.line.id.as_str() == line.id);
                let mut pose = Pose {
                    row: placement.map_or(0., |line| line.y),
                    presence: placement.map_or(0., |line| line.opacity),
                    parts: vec![1.; line.parts.len()],
                };
                if !recipe.snapshots.is_empty() {
                    pose.row = value(&format!("line.{}.y", line.id), pose.row);
                    pose.presence =
                        value(&format!("line.{}.opacity", line.id), pose.presence).clamp(0., 1.);
                }
                for reveal in std::iter::once(&recipe.inline_reveal)
                    .chain(&recipe.additional_inline_reveals)
                    .filter(|reveal| reveal.line_id == line.id)
                {
                    let range = code
                        .semantic_range(&RangeId::new(&reveal.range_id))
                        .context("unknown inline reveal range")?;
                    let range = code.resolve_logical_range(range)?;
                    pose.parts[range.start_part..=range.end_part]
                        .fill(reveal.progress(value(reveal.channel(), 0.)).clamp(0., 1.));
                }
                let before = previous.get(&line.id);
                let text = |pose: &Pose| {
                    line.parts
                        .iter()
                        .zip(&pose.parts)
                        .filter(|(_, presence)| **presence > 0.001 && pose.presence > 0.001)
                        .flat_map(|(part, _)| &part.spans)
                        .map(|span| span.text.as_str())
                        .collect::<String>()
                };
                let before_text = before.filter(|pose| pose.presence > 0.001).map(text);
                let after_text = (pose.presence > 0.001).then(|| text(&pose));
                let changed = line
                    .parts
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| {
                        before.map_or(0., |old| old.parts[*index] * old.presence)
                            != pose.parts[*index] * pose.presence
                    })
                    .map(|(_, part)| part.id.clone())
                    .collect::<Vec<_>>();
                let mut removed = String::new();
                let mut added = String::new();
                if let Some(before) = before {
                    for (index, part) in line.parts.iter().enumerate() {
                        let old = before.parts[index] * before.presence;
                        let new = pose.parts[index] * pose.presence;
                        let text = part
                            .spans
                            .iter()
                            .map(|span| span.text.as_str())
                            .collect::<String>();
                        if old > 0.001 && new <= 0.001 {
                            removed.push_str(&text);
                        }
                        if new > 0.001 && old <= 0.001 {
                            added.push_str(&text);
                        }
                    }
                }
                if let Some(common) = common_text(&removed, &added) {
                    result.warnings.push(StabilityWarning { code: "common-text-in-replacements", step_id: step.id.clone(), actor_id: actor.id.clone(), line_ids: vec![line.id.clone()], common_text: common, channel_ids: Vec::new(), suggestion: "Keep this common text in one stable inline part outside the exchanged ranges; verify semantic identity manually." });
                }
                if before_text.is_some() && after_text.is_none() {
                    exits.push((line.id.clone(), before_text.clone().unwrap()));
                }
                if before_text.is_none() && after_text.is_some() {
                    enters.push((line.id.clone(), after_text.clone().unwrap()));
                }
                let delta = line
                    .parts
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| pose.presence > 0.001 && pose.parts[*index] > 0.001)
                    .map(|(_, part)| {
                        let text = part
                            .spans
                            .iter()
                            .map(|span| span.text.as_str())
                            .collect::<String>();
                        if step_index > 0 && changed.contains(&part.id) {
                            format!("«{text}»")
                        } else {
                            text
                        }
                    })
                    .collect();
                let before_delta = before.filter(|old| old.presence > 0.001).map(|old| {
                    line.parts
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| old.parts[*index] > 0.001)
                        .map(|(_, part)| {
                            let text = part
                                .spans
                                .iter()
                                .map(|span| span.text.as_str())
                                .collect::<String>();
                            if changed.contains(&part.id) {
                                format!("«{text}»")
                            } else {
                                text
                            }
                        })
                        .collect()
                });
                let moved = before.is_some_and(|old| {
                    old.row != pose.row && old.presence > 0.001 && pose.presence > 0.001
                });
                let before_y = before.filter(|old| old.presence > 0.001).map(|old| old.row);
                let after_y = (pose.presence > 0.001).then_some(pose.row);
                if before_text.is_some() || after_text.is_some() {
                    lines.push(LineDelta {
                        id: line.id.clone(),
                        before: before_text,
                        after: after_text,
                        before_delta,
                        delta,
                        changed_part_ids: changed,
                        moved,
                        before_y,
                        after_y,
                    });
                }
                previous.insert(line.id.clone(), pose);
            }
            for (old_id, old) in &exits {
                for (new_id, new) in &enters {
                    if let Some(common) = common_text(old, new) {
                        result.warnings.push(StabilityWarning { code: "common-text-in-replaced-lines", step_id: step.id.clone(), actor_id: actor.id.clone(), line_ids: vec![old_id.clone(), new_id.clone()], common_text: common, channel_ids: Vec::new(), suggestion: "Check whether these are the same logical line; preserve its ID and reveal only changed inline content." });
                    }
                }
            }
            lines.sort_by(|a, b| {
                a.after_y
                    .or(a.before_y)
                    .unwrap_or(0.)
                    .total_cmp(&b.after_y.or(b.before_y).unwrap_or(0.))
                    .then(a.id.cmp(&b.id))
            });
            result.steps[step_index].editors.push(EditorDelta {
                actor_id: actor.id.clone(),
                lines,
            });
        }
    }
    Ok(result)
}

/// A warning heuristic only: never assign identity based on coincident text.
fn common_text(a: &str, b: &str) -> Option<String> {
    let a = a.chars().collect::<Vec<_>>();
    let b = b.chars().collect::<Vec<_>>();
    let mut lengths = vec![0; b.len() + 1];
    let (mut longest, mut end) = (0, 0);
    for (i, left) in a.iter().enumerate() {
        for j in (0..b.len()).rev() {
            lengths[j + 1] = if *left == b[j] { lengths[j] + 1 } else { 0 };
            if lengths[j + 1] > longest {
                longest = lengths[j + 1];
                end = i + 1;
            }
        }
    }
    let common = a[end - longest..end].iter().collect::<String>();
    (common.trim().chars().count() >= 3).then_some(common)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example(stable_wrapper: bool) -> ScenePlan {
        let mut builder = crate::author::PlanBuilder::new("stability", 4_000_000_000);
        let recipe = serde_json::json!({
            "fileName": "example.ts", "initialLineIds": ["line"], "finalLineIds": ["line"],
            "lineHeight": 44, "enteringOffsetX": 0, "focusLineId": "line", "focusHeight": 44,
            "lines": [{"id": "line", "parts": [
                {"id": "prefix", "spans": [{"text": if stable_wrapper { "const read = () => " } else { "const read = " }, "style": "plain"}]},
                {"id": "old", "spans": [{"text": if stable_wrapper { "???" } else { "() => ???" }, "style": "plain"}]},
                {"id": "new", "spans": [{"text": if stable_wrapper { "Effect.succeed(1)" } else { "() => Effect.succeed(1)" }, "style": "plain"}]}
            ], "semanticRanges": [
                {"id": "old", "firstPartId": "old", "lastPartId": "old"},
                {"id": "new", "firstPartId": "new", "lastPartId": "new"}
            ]}],
            "inlineReveal": {"lineId": "line", "rangeId": "old", "channel": "change", "reversed": true},
            "additionalInlineReveals": [{"lineId": "line", "rangeId": "new", "channel": "change"}]
        });
        let editor = builder.actor("editor", "editor", recipe).unwrap();
        let change = builder.continuous(&editor, "change", 0.);
        builder.spring(&change, 1_000_000_000, 1., 0.4, 0.);
        builder.presentation_step("initial", "Initial", 0, 0);
        builder.presentation_step("changed", "Changed", 1_000_000_000, 3_000_000_000);
        builder.finish().unwrap()
    }

    #[test]
    fn delta_preserves_common_parts_and_warns_about_replaced_wrappers() {
        let stable = inspect_steps(&example(true)).unwrap();
        assert!(stable.warnings.is_empty());
        let line = &stable.steps[1].editors[0].lines[0];
        assert_eq!(line.delta, "const read = () => «Effect.succeed(1)»");
        assert_eq!(
            line.before_delta.as_deref(),
            Some("const read = () => «???»")
        );
        assert_eq!(line.changed_part_ids, ["old", "new"]);
        assert!(!line.moved);
        let unstable = inspect_steps(&example(false)).unwrap();
        assert_eq!(unstable.warnings.len(), 1);
        assert_eq!(unstable.warnings[0].common_text, "() => ");
    }

    #[test]
    fn partial_hold_is_diagnosed_instead_of_claiming_a_settled_delta() {
        let mut plan = example(true);
        plan.presentation_steps[1].hold_nanos = 1_150_000_000;
        let report = inspect_steps(&plan).unwrap();
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.code == "unsettled-code-step"
                    && warning.channel_ids == ["editor.change"])
        );
    }

    #[test]
    fn moved_lines_keep_identity_and_report_their_sampled_order() {
        let mut plan = example(true);
        let data = &mut plan.actors[0].data;
        let mut added = data["lines"][0].clone();
        added["id"] = "added".into();
        data["lines"].as_array_mut().unwrap().push(added);
        data["finalLineIds"] = serde_json::json!(["added", "line"]);
        data["snapshots"] =
            serde_json::json!([{ "atNanos": 1_000_000_000u64, "lineIds": ["added", "line"] }]);
        let report = inspect_steps(&plan).unwrap();
        let lines = &report.steps[1].editors[0].lines;
        assert_eq!(lines[0].id, "added");
        assert_eq!(lines[1].id, "line");
        assert!(lines[1].moved);
        assert_eq!(lines[1].before_y, Some(0.));
        assert_eq!(lines[1].after_y, Some(44.));
    }

    #[test]
    fn unknown_or_overlapping_ranges_are_not_silently_omitted() {
        let mut plan = example(true);
        plan.actors[0].data["inlineReveal"]["lineId"] = "missing".into();
        assert!(inspect_steps(&plan).is_err());
        let mut plan = example(true);
        plan.actors[0].data["additionalInlineReveals"][0]["rangeId"] = "old".into();
        assert!(inspect_steps(&plan).is_err());
    }

    #[test]
    fn replacing_a_line_warns_without_guessing_its_identity() {
        let mut plan = example(true);
        let data = &mut plan.actors[0].data;
        let mut added = data["lines"][0].clone();
        added["id"] = "replacement".into();
        data["lines"].as_array_mut().unwrap().push(added);
        data["finalLineIds"] = serde_json::json!(["replacement"]);
        data["snapshots"] =
            serde_json::json!([{ "atNanos": 1_000_000_000u64, "lineIds": ["replacement"] }]);
        let report = inspect_steps(&plan).unwrap();
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.code == "common-text-in-replaced-lines")
        );
    }
    #[test]
    fn common_text_is_unicode_safe_and_preserves_common_wrappers() {
        assert_eq!(
            common_text("() => ???", "() => Effect.succeed(1)"),
            Some("() => ".into())
        );
        assert_eq!(common_text("A)", "B)"), None);
        assert_eq!(
            common_text(
                "get(\"https://example.com\")",
                "fetch(\"https://example.com\")"
            ),
            Some("(\"https://example.com\")".into())
        );
        assert_eq!(common_text("a café!", "b café?"), Some(" café".into()));
    }
}
