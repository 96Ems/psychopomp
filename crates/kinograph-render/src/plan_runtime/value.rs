use anyhow::{Context, Result};
use kinograph::{plan::ActorPlan, value::ValueTokenPlan};

use crate::render::HeadlessRenderer;

pub(super) struct PreparedValueToken {
    actor_id: String,
    recipe: ValueTokenPlan,
}

impl PreparedValueToken {
    pub(super) fn new(actor: &ActorPlan) -> Result<Self> {
        let recipe: ValueTokenPlan = serde_json::from_value(actor.data.clone())
            .with_context(|| format!("parse value token '{}'", actor.id))?;
        recipe.validate()?;
        Ok(Self {
            actor_id: actor.id.clone(),
            recipe,
        })
    }

    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &mut HeadlessRenderer,
        sample: impl Fn(&str, &str, f32) -> f32,
    ) {
        renderer.composite_value_token(
            pixels,
            &self.recipe,
            [
                sample(&self.actor_id, "x", self.recipe.center[0]),
                sample(&self.actor_id, "y", self.recipe.center[1]),
            ],
            sample(&self.actor_id, "opacity", 1.),
            sample(&self.actor_id, "emphasis", 0.),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::plan_runtime::{PreparedPlan, new_renderer, validate_renderer_plan};
    use kinograph::playback::PlaybackCommand;
    use std::{path::Path, time::Duration};

    #[test]
    fn validates_the_whole_deck_and_rejects_malformed_tokens_without_a_gpu() {
        let deck = kinograph_data_modeling::build_deck().unwrap();
        for slide in &deck.slides {
            validate_renderer_plan(&slide.plan).unwrap();
        }
        let mut plan = deck.slides[0].plan.clone();
        let i = plan
            .actors
            .iter()
            .position(|a| a.recipe == "value-token")
            .unwrap();
        plan.actors[i].data["size"] = serde_json::json!([0, 100]);
        assert!(validate_renderer_plan(&plan).is_err());
        plan.actors[i].data["size"] = serde_json::json!([200, 100]);
        plan.actors[i].data["fontSize"] = serde_json::json!(1e100);
        assert!(validate_renderer_plan(&plan).is_err());
    }

    #[test]
    #[ignore = "requires a headless GPU; every slide through forward/back/skip and interrupted A-B-A-C navigation"]
    fn data_modeling_pixels_and_velocity_survive_interrupted_navigation() {
        let mut renderer = pollster::block_on(new_renderer("data-modeling-navigation")).unwrap();
        for slide in kinograph_data_modeling::build_deck().unwrap().slides {
            let p = PreparedPlan::prepare(slide.plan, Path::new("."), &mut renderer).unwrap();
            let mut playback = p.playback(false).unwrap();
            let mut now = Duration::ZERO;
            for command in [
                PlaybackCommand::Next,
                PlaybackCommand::Previous,
                PlaybackCommand::Last,
                PlaybackCommand::Previous,
                PlaybackCommand::First,
                PlaybackCommand::Next,
            ] {
                let boundary = crate::plan_runtime::proof::interrupt(
                    &p,
                    &mut renderer,
                    &mut playback,
                    now,
                    command,
                )
                .unwrap();
                assert!(boundary.changed);
                boundary.assert_states_within(&p, 0.001);
                boundary.assert_pixels(&p, &mut renderer);
                let future = boundary.sample_later_and_repeat(&p, &mut renderer, 0.07);
                assert!(
                    boundary.pixels != future,
                    "{} should visibly move",
                    p.plan.id
                );
                now += Duration::from_millis(130);
            }
            crate::plan_runtime::proof::assert_reduced_motion_holds(&p, &mut renderer);
        }
    }

    #[test]
    #[ignore = "requires a headless GPU; authored grid scale changes geometry while preserving centering and the default"]
    fn product_framing_uses_the_authored_scale_channel() {
        let mut plan = kinograph_data_modeling::build_deck().unwrap().slides[5]
            .plan
            .clone();
        plan.actors.retain(|actor| actor.recipe == "keyed-grid");
        plan.continuous_channels
            .retain(|channel| channel.actor_id == "grid");
        let mut renderer = pollster::block_on(new_renderer("product-framing")).unwrap();
        let render = |renderer: &mut crate::render::HeadlessRenderer, scale: Option<f32>| {
            let mut plan = plan.clone();
            if let Some(scale) = scale {
                plan.continuous_channels[0].initial = scale.into();
            } else {
                plan.continuous_channels.clear();
            }
            PreparedPlan::prepare(plan, Path::new("."), renderer)
                .unwrap()
                .render_sample(renderer, 8.)
                .unwrap()
        };
        let default = render(&mut renderer, None);
        assert!(
            default == render(&mut renderer, Some(1.)),
            "omitted scale must keep the existing default"
        );
        let bounds = |pixels: &[u8]| {
            let mut min = [1920, 1080];
            let mut max = [0, 0];
            for (i, pixel) in pixels.chunks_exact(4).enumerate() {
                if pixel[0] > 120
                    && f32::from(pixel[0]) > f32::from(pixel[1]) * 1.3
                    && pixel[2] < 140
                {
                    let point = [i % 1920, i / 1920];
                    for axis in 0..2 {
                        min[axis] = min[axis].min(point[axis]);
                        max[axis] = max[axis].max(point[axis]);
                    }
                }
            }
            assert!(
                max[0] > min[0] && max[1] > min[1],
                "expected orange grid strokes"
            );
            (min, max)
        };
        let (a, b) = bounds(&render(&mut renderer, Some(0.9)));
        let (c, d) = bounds(&render(&mut renderer, Some(1.8)));
        for axis in 0..2 {
            assert!(
                ((d[axis] - c[axis]) as f32 - 2. * (b[axis] - a[axis]) as f32).abs() < 4.,
                "scale must reach visible geometry"
            );
            assert!(((a[axis] + b[axis]) as f32 / 2. - [960., 540.][axis]).abs() <= 0.75);
            assert!(((c[axis] + d[axis]) as f32 / 2. - [960., 540.][axis]).abs() <= 0.75);
        }
    }
}
