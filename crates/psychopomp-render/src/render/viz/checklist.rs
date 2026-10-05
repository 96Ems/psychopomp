//! Checklist pixels: a pending ring, the closed-form status spinner and its
//! drawn mark, labels that brighten as items start, strikes for skips, a
//! progress rail, and right-aligned results. Every pose derives from the
//! item's channels through `checklist::ItemPose`.
use psychopomp::{
    checklist::{ChecklistPlan, ItemPose, Outcome},
    effects::spinner::{self, Mark},
    math::{smoothstep, vec2},
    readout::ReadoutFormat,
    tone::Tone,
};

use super::{
    super::{
        chart::{Anchor, solid, stroke},
        theme::mix,
        ui::{
            Bounds,
            card::{Fill, SurfaceStyle, UiCanvas, UiColor},
        },
    },
    arc,
    readout::Readout,
    weighted_stroke,
};
use crate::render::HeadlessRenderer;

/// The pending ring's radius, as a share of the icon box.
const RING: f32 = 0.34;

impl HeadlessRenderer {
    pub(crate) fn composite_checklist(
        &mut self,
        pixels: &mut [u8],
        plan: &ChecklistPlan,
        sample: impl Fn(&str, f32) -> f32,
    ) {
        let opacity = sample("opacity", 1.0).clamp(0.0, 1.0);
        if opacity <= 0.001 {
            return;
        }
        let shift = [sample("x", 0.0), sample("y", 0.0)];
        let canvas = self.viz_canvas();
        let palette = self.theme.palette();
        let poses = plan
            .items
            .iter()
            .map(|item| {
                let channel =
                    |name: &str, default: f32| sample(&format!("item.{}.{name}", item.id), default);
                (
                    ItemPose::new(
                        channel("reveal", 1.0),
                        channel("spinner", -1.0),
                        channel("mark", -1.0),
                        channel("outcome", 0.0),
                    ),
                    channel("spinner", -1.0),
                    channel("mark", -1.0),
                )
            })
            .collect::<Vec<_>>();
        let [left, top, right, bottom] = plan.extent();
        if plan.panel {
            let pad = [plan.size * 0.9, plan.size * 0.7];
            let [r, g, b] = palette.surface;
            let [br, bg, bb] = palette.raised;
            UiCanvas::new(pixels, canvas).surface(
                Bounds {
                    origin: [left - pad[0] + shift[0], top - pad[1] + shift[1]],
                    size: [right - left + pad[0] * 2.0, bottom - top + pad[1] * 2.0],
                },
                SurfaceStyle::new(Fill::Solid(UiColor::srgb8(r, g, b, 255)), 18.0).border(
                    1.2,
                    UiColor::srgb8(br, bg, bb, 255),
                    1.0,
                ),
                opacity,
            );
        }
        if !plan.title.is_empty() {
            self.checklist_title(pixels, plan, &poses, shift, opacity);
        }
        let icon = plan.icon_size();
        let unit = icon / 16.0;
        let x = plan.icon_x() + shift[0];
        // The rail runs from one ring's edge to the next.
        let ring = icon * RING;
        if plan.rail {
            for (index, pair) in poses.windows(2).enumerate() {
                let (pose, next) = (pair[0].0, pair[1].0);
                let from = plan.row_center(index) + ring + 5.0 + shift[1];
                let to = plan.row_center(index + 1) - ring - 5.0 + shift[1];
                let present = opacity * smoothstep(pose.reveal.min(next.reveal));
                if to <= from || present <= 0.001 {
                    continue;
                }
                stroke(
                    pixels,
                    canvas,
                    &[[x, from], [x, to]],
                    1.5,
                    palette.raised,
                    present,
                );
                if pose.rail > 0.001 {
                    let fill = self.outcome_ink(pose.outcome);
                    let end = from + (to - from) * pose.rail;
                    stroke(
                        pixels,
                        canvas,
                        &[[x, from], [x, end]],
                        2.0,
                        fill,
                        present * 0.9,
                    );
                }
            }
        }
        for (index, (item, &(pose, spinner_age, mark_age))) in
            plan.items.iter().zip(&poses).enumerate()
        {
            let alpha = opacity * smoothstep(pose.reveal);
            if alpha <= 0.001 {
                continue;
            }
            let y = plan.row_center(index) + shift[1] + (1.0 - pose.reveal) * 10.0;
            let center = [x, y];
            if pose.pending > 0.001 {
                let outline = arc(center, ring, 0.0, std::f32::consts::TAU);
                stroke(
                    pixels,
                    canvas,
                    &outline,
                    1.6,
                    palette.muted,
                    alpha * pose.pending * 0.7,
                );
            }
            self.checklist_icon(pixels, center, unit, pose, spinner_age, mark_age, alpha);
            // The label brightens as the item starts; a skip dims it again.
            let label_x = plan.label_x() + shift[0];
            let width = self.text_advance(&item.label, plan.size);
            for (color, weight) in [
                (palette.muted, 1.0 - pose.active),
                (palette.text, pose.active),
            ] {
                self.viz_text(
                    pixels,
                    &item.label,
                    plan.size,
                    color,
                    Anchor::Left,
                    [label_x, y],
                    alpha * weight,
                );
            }
            if pose.strike > 0.001 {
                let from = label_x - 4.0;
                let to = from + (width + 8.0) * pose.strike;
                let strike_y = y + plan.size * 0.04;
                stroke(
                    pixels,
                    canvas,
                    &[[from, strike_y], [to, strike_y]],
                    2.0,
                    palette.muted,
                    alpha,
                );
            }
            let result = item.result_for(pose.outcome);
            if pose.result > 0.001 && !result.is_empty() {
                let color = match pose.outcome {
                    Some(Outcome::Failed) => self.theme.tone(Tone::Error),
                    _ => palette.muted,
                };
                self.viz_text(
                    pixels,
                    result,
                    (plan.size * 0.85).round(),
                    color,
                    Anchor::Right,
                    [left + plan.width + shift[0] + (1.0 - pose.result) * 8.0, y],
                    alpha * pose.result,
                );
            }
        }
    }

    fn outcome_ink(&self, outcome: Option<Outcome>) -> [u8; 3] {
        match outcome {
            Some(Outcome::Done) => self.theme.tone(Tone::Success),
            Some(Outcome::Failed) => self.theme.tone(Tone::Error),
            _ => self.theme.palette().muted,
        }
    }

    /// The spinner while running, its mark once resolved, or a skip's dash.
    #[allow(clippy::too_many_arguments)]
    fn checklist_icon(
        &mut self,
        pixels: &mut [u8],
        center: [f32; 2],
        unit: f32,
        pose: ItemPose,
        spinner_age: f32,
        mark_age: f32,
        alpha: f32,
    ) {
        let canvas = self.viz_canvas();
        let palette = self.theme.palette();
        let accent = palette.accent;
        let (spinner, ink) = match pose.outcome {
            Some(Outcome::Skipped) => {
                // A running spinner coasts out under the dash.
                let dash = [
                    [center[0] - 4.5 * unit, center[1]],
                    [
                        center[0] - 4.5 * unit + 9.0 * unit * pose.resolved,
                        center[1],
                    ],
                ];
                if pose.resolved > 0.001 {
                    stroke(pixels, canvas, &dash, 1.7 * unit, palette.muted, alpha);
                }
                (
                    spinner::sample(spinner_age, mark_age, -1.0, Mark::Check),
                    accent,
                )
            }
            outcome => {
                let shape = outcome.and_then(Outcome::mark).unwrap_or_default();
                let landing = smoothstep(mark_age.max(0.0) / 0.2);
                let ink = if outcome.is_some() {
                    mix(accent, self.outcome_ink(outcome), landing)
                } else {
                    accent
                };
                (spinner::sample(spinner_age, -1.0, mark_age, shape), ink)
            }
        };
        if spinner.opacity <= 0.001 {
            return;
        }
        let color = mix(ink, [255, 255, 255], spinner.flash * 0.7);
        for points in &spinner.strokes {
            let points = points
                .iter()
                .map(|(point, weight)| {
                    let at = vec2(center[0], center[1]) + (*point - vec2(8.0, 8.0)) * unit;
                    ([at.x, at.y], *weight)
                })
                .collect::<Vec<_>>();
            weighted_stroke(
                pixels,
                canvas,
                &points,
                1.6 * unit,
                color,
                alpha * spinner.opacity,
            );
        }
    }

    /// The title row: the heading, a done count, and a hairline beneath.
    fn checklist_title(
        &mut self,
        pixels: &mut [u8],
        plan: &ChecklistPlan,
        poses: &[(ItemPose, f32, f32)],
        shift: [f32; 2],
        opacity: f32,
    ) {
        let palette = self.theme.palette();
        let canvas = self.viz_canvas();
        let height = plan.title_height();
        let size = (plan.size * 0.8).round();
        let y = plan.origin[1] + height * 0.42 + shift[1];
        let left = plan.origin[0] + shift[0];
        self.viz_text(
            pixels,
            &plan.title,
            size,
            palette.muted,
            Anchor::Left,
            [left, y],
            opacity,
        );
        let done = poses
            .iter()
            .filter(|(pose, ..)| pose.outcome == Some(Outcome::Done))
            .map(|(pose, ..)| pose.resolved)
            .sum::<f32>();
        // Every item passed or was skipped: the count turns to success.
        let all = poses.iter().all(|(pose, ..)| {
            matches!(pose.outcome, Some(Outcome::Done | Outcome::Skipped)) && pose.resolved >= 1.0
        });
        let format = ReadoutFormat::new(0).unit(format!("/{}", plan.items.len()));
        let ink = [(
            if all {
                self.theme.tone(Tone::Success)
            } else {
                palette.text
            },
            1.0,
        )];
        self.draw_readout(
            pixels,
            &Readout {
                format: &format,
                value: done,
                velocity: 0.0,
                size,
                affix_scale: 1.0,
                ink: &ink,
                affix: palette.muted,
            },
            Anchor::Right,
            [left + plan.width, y],
            opacity,
        );
        let rule_y = plan.origin[1] + height - 6.0 + shift[1];
        UiCanvas::new(pixels, canvas).fill(
            Bounds {
                origin: [left, rule_y.round()],
                size: [plan.width, 1.0],
            },
            0.0,
            solid(palette.raised),
            opacity,
        );
    }
}

#[cfg(test)]
mod gpu_tests {
    use psychopomp::checklist::{ChecklistItemPlan, ChecklistPlan};

    use crate::render::{HeadlessRenderer, RenderSpec};

    #[test]
    #[ignore = "requires a headless GPU; spinners, marks, and strikes reach pixels without residue"]
    fn checklists_resolve_without_residue() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "checklist-proof".into(),
        }))
        .unwrap();
        let plan = ChecklistPlan::new([600.0, 300.0], 700.0)
            .item(ChecklistItemPlan::new("a", "typecheck").result("4.2s"))
            .item(ChecklistItemPlan::new("b", "unit tests"))
            .title("checks")
            .rail();
        let background = renderer.render_title_card("", None, 0.);
        let draw = |renderer: &mut HeadlessRenderer, opacity: f32, spinner: f32, mark: f32| {
            let mut pixels = background.clone();
            renderer.composite_checklist(&mut pixels, &plan, |property, default| match property {
                "opacity" => opacity,
                "item.a.spinner" => spinner,
                "item.a.mark" => mark,
                "item.a.outcome" => 1.0,
                _ => default,
            });
            pixels
        };
        assert!(draw(&mut renderer, 0.0, 1.0, 0.5) == background);
        let pending = draw(&mut renderer, 1.0, -1.0, -1.0);
        let running = draw(&mut renderer, 1.0, 1.0, -1.0);
        let done = draw(&mut renderer, 1.0, 2.0, 1.0);
        assert!(pending != running && running != done);
        assert!(
            draw(&mut renderer, 1.0, 1.0, -1.0) == running,
            "deterministic"
        );
    }
}
