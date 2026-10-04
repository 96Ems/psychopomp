//! Terminals: the recipe is decoded and every per-line channel checked
//! against its line's kind during preflight. Sampling is channel-only, so
//! terminals run in interruptible native playback as well as video.
use anyhow::{Result, bail};
use psychopomp::{
    plan::{ActorPlan, ContinuousChannelPlan},
    terminal::{TerminalChannel, TerminalPlan, accepts_property},
};

use super::preflight::{decode, strict_channels};
use crate::render::{HeadlessRenderer, TerminalNames};

pub(super) struct PreparedTerminal {
    id: String,
    plan: TerminalPlan,
    names: TerminalNames,
}

impl PreparedTerminal {
    pub(super) fn new(actor: &ActorPlan, channels: &[ContinuousChannelPlan]) -> Result<Self> {
        let plan = decode(actor, "terminal", TerminalPlan::validate)?;
        strict_channels(&actor.id, channels, "terminal", |property| {
            accepts_property(property) || TerminalChannel::parse(property).is_some()
        })?;
        for channel in channels.iter().filter(|c| c.actor_id == actor.id) {
            let Some((id, kind)) = TerminalChannel::parse(&channel.property) else {
                continue;
            };
            let Some(line) = plan.find(id).map(|index| &plan.lines[index]) else {
                bail!(
                    "terminal actor '{}' channel '{}' names a line that is not declared",
                    actor.id,
                    channel.property
                );
            };
            if !line.accepts(kind) {
                bail!(
                    "terminal actor '{}' channel '{}' does not apply to a {} line",
                    actor.id,
                    channel.property,
                    match line {
                        psychopomp::terminal::TerminalLinePlan::Command { .. } => "command",
                        psychopomp::terminal::TerminalLinePlan::Output { .. } => "output",
                        psychopomp::terminal::TerminalLinePlan::Task { .. } => "task",
                    }
                );
            }
        }
        let names = TerminalNames::new(&plan);
        Ok(Self {
            id: actor.id.clone(),
            plan,
            names,
        })
    }

    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &mut HeadlessRenderer,
        sample: impl Fn(&str, &str, f32) -> f32,
    ) -> Result<()> {
        renderer.composite_terminal(pixels, &self.plan, &self.names, |property, d| {
            sample(&self.id, property, d)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::plan_runtime::{preflight, validate_renderer_plan};
    use psychopomp::{
        author::PlanBuilder,
        caption::CaptionSpanPlan,
        terminal::{TerminalActor, TerminalPlan},
        tone::Tone,
    };

    #[test]
    fn preflight_checks_every_line_channel_without_a_gpu() {
        let build = |property: &str| {
            let mut scene = PlanBuilder::new("terminal", 4_000_000_000);
            let mut term = TerminalActor::declare(
                &mut scene,
                "term",
                TerminalPlan::new([100.0, 100.0], 1000.0, 8),
            )
            .unwrap();
            term.type_command(&mut scene, 0, "ls").unwrap();
            term.spin(
                &mut scene,
                1_000_000_000,
                vec![CaptionSpanPlan::new("build", Tone::Muted)],
            )
            .unwrap();
            term.channel(&mut scene, property, 0.0);
            scene.finish().unwrap()
        };
        for valid in [
            "opacity",
            "content",
            "caret",
            "scroll",
            "line.cmd0.typed",
            "line.task1.mark",
            "line.cmd0.highlight",
        ] {
            validate_renderer_plan(&build(valid)).unwrap();
        }
        assert!(
            preflight::Plan::new(build("caret")).unwrap().native(),
            "terminals are channel-only, so they present natively"
        );
        for invalid in [
            "typed",
            "line.missing.reveal",
            "line.cmd0.spin",
            "line.task1.typed",
            "line.cmd0.wobble",
        ] {
            assert!(
                validate_renderer_plan(&build(invalid)).is_err(),
                "{invalid}"
            );
        }
    }
}
