//! Prepared Plots: the recipe is decoded and validated once, and every channel
//! on the actor must name a real series, mark, or plot property.
use anyhow::Result;
use psychopomp::{
    plan::{ActorPlan, ContinuousChannelPlan},
    plot::PlotPlan,
};

use super::preflight::{decode, strict_channels};
use crate::render::HeadlessRenderer;

pub(super) struct PreparedPlot {
    id: String,
    plan: PlotPlan,
}

impl PreparedPlot {
    pub(super) fn new(actor: &ActorPlan, channels: &[ContinuousChannelPlan]) -> Result<Self> {
        let plan = decode(actor, "plot", PlotPlan::validate)?;
        strict_channels(&actor.id, channels, "plot", |property| {
            plan.accepts(property)
        })?;
        Ok(Self {
            id: actor.id.clone(),
            plan,
        })
    }

    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &mut HeadlessRenderer,
        sample: impl Fn(&str, &str, f32) -> f32,
    ) {
        renderer.composite_plot(pixels, &self.plan, |property, default| {
            sample(&self.id, property, default)
        });
    }
}

#[cfg(test)]
mod tests {
    use psychopomp::{
        author::PlanBuilder,
        axis::AxisPlan,
        plot::{PlotActor, PlotPlan, PlotSeriesPlan},
        tone::Tone,
    };

    use super::super::validate_renderer_plan;

    #[test]
    fn plot_preflight_accepts_known_channels_and_rejects_typos() {
        let build = |property: Option<&str>| {
            let recipe = PlotPlan::new(
                [200.0, 200.0],
                [1000.0, 500.0],
                AxisPlan::new([0.0, 1.0]),
                AxisPlan::new([0.0, 1.0]),
            )
            .series(PlotSeriesPlan::sampled(
                "line",
                "",
                Tone::Plain,
                [0.0, 1.0],
                8,
                |x| x,
            ));
            let mut scene = PlanBuilder::new("plot-preflight", 2_000_000_000);
            let mut plot = PlotActor::declare(&mut scene, "plot", &recipe).unwrap();
            plot.draw(&mut scene, "line", 0, 1.0);
            plot.ride(&mut scene, "line", [0.0, 1.0], 0, 1.0);
            if let Some(property) = property {
                plot.channel(&mut scene, property, 1.0);
            }
            scene.finish().unwrap()
        };
        validate_renderer_plan(&build(None)).unwrap();
        for typo in [
            "series.line.drew",
            "series.other.draw",
            "mark.none.opacity",
            "scale",
        ] {
            let error = validate_renderer_plan(&build(Some(typo))).unwrap_err();
            assert!(
                format!("{error:#}").contains("unknown property"),
                "{typo}: {error:#}"
            );
        }
    }
}
