//! Scene-local layout and step helpers. No additional playback state machine.
use anyhow::Result;
use psychopomp::{
    author::{ActorHandle, PlanBuilder, SECOND},
    plan::ScenePlan,
    value::{VALUE_TOKEN_RECIPE, ValueTokenPlan},
};
use serde_json::json;

pub const BEAT: u64 = 3 * SECOND;
pub const INK: [u8; 3] = [234, 239, 247];
pub const MUTED: [u8; 3] = [143, 159, 184];
pub const ORANGE: [u8; 3] = [250, 145, 80];
pub const RED: [u8; 3] = [248, 116, 130];

pub struct Stage {
    pub scene: PlanBuilder,
    pub steps: usize,
}

impl Stage {
    pub fn new(id: &str, title: &str, captions: &[&str]) -> Result<Self> {
        let mut stage = Self {
            scene: PlanBuilder::new(id, captions.len() as u64 * BEAT),
            steps: captions.len(),
        };
        stage.text(
            "eyebrow",
            "FUNCTIONAL DATA MODELING",
            [960., 54.],
            18.,
            MUTED,
        )?;
        stage.text("heading", title, [960., 118.], 46., INK)?;
        stage.text("controls", "← / →  step     ⌘← / ⌘→  slide     1–7  choose slide     R  replay     M  reduced motion", [960., 1020.], 17., MUTED)?;
        for (i, caption) in captions.iter().enumerate() {
            let at = i as u64 * BEAT;
            stage.scene.presentation_step(
                format!("step-{i}"),
                *caption,
                at,
                if i == 0 { 0 } else { at + 2 * SECOND },
            );
            stage.scene.cue(format!("step-{i}"), at, at + BEAT);
        }
        for (i, caption) in captions.iter().enumerate() {
            let actor = stage.scene.actor(
                format!("caption-{i}"),
                "text",
                json!({
                    "text": caption, "center": [960, 946], "fontSize": 27, "color": INK,
                    "verticalMask": {"top": 915, "bottom": 977, "fade": 10}
                }),
            )?;
            stage.show(&actor, i, i + 1);
            let y = (0..stage.steps)
                .map(|step| {
                    if step < i {
                        991.
                    } else if step == i {
                        946.
                    } else {
                        901.
                    }
                })
                .collect::<Vec<_>>();
            stage.track(&actor, "y", &y);
        }
        Ok(stage)
    }

    pub fn text(
        &mut self,
        id: &str,
        text: &str,
        center: [f32; 2],
        size: f32,
        color: [u8; 3],
    ) -> Result<ActorHandle> {
        Ok(self.scene.actor(
            id,
            "text",
            json!({"text":text,"center":center,"fontSize":size,"color":color}),
        )?)
    }

    pub fn tile(
        &mut self,
        id: &str,
        label: &str,
        detail: &str,
        center: [f32; 2],
        size: [f32; 2],
    ) -> Result<ActorHandle> {
        let token = ValueTokenPlan {
            label: label.into(),
            detail: detail.into(),
            center,
            size,
            font_size: 36_f32
                .min((size[0] - 36.) / (label.chars().count() as f32 * 0.65))
                .min((size[1] - 16.) / 1.2),
            accent: ORANGE,
        };
        token.validate()?;
        Ok(self.scene.actor(id, VALUE_TOKEN_RECIPE, token)?)
    }

    pub fn track(&mut self, actor: &ActorHandle, property: &str, values: &[f32]) {
        assert_eq!(
            values.len(),
            self.steps,
            "one destination per step: {}.{property}",
            actor.id()
        );
        self.scene.step_track(
            actor,
            property,
            values,
            psychopomp::plan::SpringPlan::visual(
                if property == "opacity" { 0.25 } else { 0.4 },
                0.,
            ),
        );
    }

    pub fn show(&mut self, actor: &ActorHandle, first: usize, end: usize) {
        self.track(
            actor,
            "opacity",
            &(0..self.steps)
                .map(|i| if (first..end).contains(&i) { 1. } else { 0. })
                .collect::<Vec<_>>(),
        );
    }

    pub fn finish(self) -> Result<ScenePlan> {
        Ok(self.scene.finish()?)
    }
}
