//! Chat Threads: the recipe is decoded and every message and reaction
//! channel checked during preflight; preparation shapes each message's text
//! once for its wrapped height. Sampling is channel-only, so chats run in
//! interruptible native playback as well as video.
use anyhow::{Result, bail};
use psychopomp::{
    chat::{ChatChannel, ChatPlan, accepts_property, parse_reaction},
    plan::{ActorPlan, ContinuousChannelPlan},
};

use super::preflight::{decode, strict_channels};
use crate::render::{ChatGlyphs, HeadlessRenderer};

pub(super) struct ChatInput {
    id: String,
    plan: ChatPlan,
}

impl ChatInput {
    pub(super) fn new(actor: &ActorPlan, channels: &[ContinuousChannelPlan]) -> Result<Self> {
        let plan = decode(actor, "chat", ChatPlan::validate)?;
        strict_channels(&actor.id, channels, "chat", |property| {
            accepts_property(property)
                || ChatChannel::parse(property).is_some()
                || parse_reaction(property).is_some()
        })?;
        for channel in channels.iter().filter(|c| c.actor_id == actor.id) {
            let property = channel.property.as_str();
            if let Some((id, _)) = ChatChannel::parse(property) {
                if plan.find(id).is_none() {
                    bail!(
                        "chat actor '{}' channel '{property}' names a message that is not declared",
                        actor.id
                    );
                }
            } else if let Some((message, reaction)) = parse_reaction(property) {
                let declared = plan.find(message).is_some_and(|index| {
                    plan.messages[index]
                        .reactions
                        .iter()
                        .any(|candidate| candidate.id == reaction)
                });
                if !declared {
                    bail!(
                        "chat actor '{}' channel '{property}' names a reaction that is not declared",
                        actor.id
                    );
                }
            }
        }
        Ok(Self {
            id: actor.id.clone(),
            plan,
        })
    }

    pub(super) fn prepare(self, renderer: &mut HeadlessRenderer) -> PreparedChat {
        let glyphs = renderer.prepare_chat(&self.plan);
        PreparedChat {
            id: self.id,
            plan: self.plan,
            glyphs,
        }
    }
}

pub(super) struct PreparedChat {
    id: String,
    plan: ChatPlan,
    glyphs: ChatGlyphs,
}

impl PreparedChat {
    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &mut HeadlessRenderer,
        sample: impl Fn(&str, &str, f32) -> f32,
    ) -> Result<()> {
        renderer.composite_chat(pixels, &self.plan, &self.glyphs, |property, d| {
            sample(&self.id, property, d)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::plan_runtime::{preflight, validate_renderer_plan};
    use psychopomp::{
        author::PlanBuilder,
        chat::{ChatActor, ChatPersonPlan, ChatPlan},
        tone::Tone,
    };

    #[test]
    fn preflight_checks_message_and_reaction_channels_without_a_gpu() {
        let build = |property: &str| {
            let mut scene = PlanBuilder::new("chat", 4_000_000_000);
            let mut chat = ChatActor::declare(
                &mut scene,
                "chat",
                ChatPlan::new(
                    [100.0, 100.0],
                    [900.0, 700.0],
                    vec![ChatPersonPlan::new("dax", "Dax", Tone::Accent)],
                ),
            )
            .unwrap();
            let said = chat.say_text(&mut scene, 0, "dax", "hi").unwrap();
            chat.react(&mut scene, 1_000_000_000, &said, "+1", 1)
                .unwrap();
            chat.channel(&mut scene, property, 0.0);
            scene.finish().unwrap()
        };
        for valid in ["opacity", "scale", "message.m0.typed", "reaction.m0.r0"] {
            validate_renderer_plan(&build(valid)).unwrap();
        }
        assert!(
            preflight::Plan::new(build("opacity")).unwrap().native(),
            "chats are channel-only, so they present natively"
        );
        for invalid in [
            "caret",
            "message.m9.reveal",
            "message.m0.wobble",
            "reaction.m0.r9",
            "reaction.m9.r0",
        ] {
            assert!(
                validate_renderer_plan(&build(invalid)).is_err(),
                "{invalid}"
            );
        }
    }
}
