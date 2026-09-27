use anyhow::{Context, Result, bail};
use kinograph::{
    grid::GRID_RECIPE,
    plan::{ScalarPlan, ScenePlan},
    playback::Playback,
    timeline::{PropertyId, Timeline},
};

// Use the actual native grid preparation, including disclosure and table layout.
include!(concat!(env!("OUT_DIR"), "/plan_runtime.rs"));

pub struct Scene {
    pub plan: ScenePlan,
    pub timeline: Timeline,
    pub playback: Playback,
    grid: Option<grid::PreparedGrid>,
    diagram: Option<diagram::PreparedDiagram>,
}

impl Scene {
    pub fn new(
        mut plan: ScenePlan,
        renderer: &mut crate::render::HeadlessRenderer,
    ) -> Result<Self> {
        // Only a grid or bare diagram root; no general overlay/scene graph port.
        let count = plan
            .actors
            .iter()
            .filter(|a| {
                a.recipe == GRID_RECIPE || a.recipe == kinograph::component_prototype::DIAGRAM
            })
            .count();
        if count != 1 {
            bail!("expected one grid or diagram root");
        }
        let actor = plan
            .actors
            .iter()
            .find(|a| {
                a.recipe == GRID_RECIPE || a.recipe == kinograph::component_prototype::DIAGRAM
            })
            .context("expected one grid or diagram actor")?
            .id
            .clone();
        plan.actors.retain(|a| a.id == actor);
        plan.continuous_channels.retain(|c| c.actor_id == actor);
        plan.validate()?;
        let grid = grid::compile(&mut plan)?;
        let diagram = diagram::PreparedDiagram::prepare(&plan, renderer)?;
        let literal = |v: &ScalarPlan| -> Result<f32> {
            match v {
                ScalarPlan::Literal(v) => Ok(*v),
                _ => bail!("browser spike accepts literal channels only"),
            }
        };
        let timeline = kinograph::plan::compile_channels(
            plan.continuous_channels.iter().map(|channel| (channel, PropertyId::new(&channel.id))),
            plan.duration_nanos,
            literal,
        )?;
        let mut delays = std::collections::HashMap::new();
        if let Some(diagram) = &diagram {
            diagram.delays(&mut delays);
        }
        let playback = Playback::with_start_delays(
            &plan,
            &timeline,
            false,
            &std::collections::HashMap::new(),
            &delays,
        )?;
        Ok(Self {
            plan,
            timeline,
            playback,
            grid,
            diagram,
        })
    }

    pub fn render(
        &self,
        renderer: &mut crate::render::HeadlessRenderer,
        time: f64,
        timeline: &Timeline,
    ) -> Result<Vec<u8>> {
        #[cfg(target_arch = "wasm32")]
        renderer.begin_frame()?;
        if let Some(diagram) = &self.diagram {
            return diagram.render(renderer, |actor, property, default| {
                self.plan
                    .continuous_channels
                    .iter()
                    .find(|c| c.actor_id == actor && c.property == property)
                    .and_then(|c| timeline.sample_at(&PropertyId::new(&c.id), time))
                    .map_or(default, |s| s.position)
            });
        }
        let grid = self.grid.as_ref().context("prepared browser root")?;
        let scale = self
            .plan
            .continuous_channels
            .iter()
            .find(|c| c.actor_id == grid.actor_id() && c.property == "scale")
            .and_then(|c| timeline.sample_at(&PropertyId::new(&c.id), time))
            .map_or(1., |s| s.position);
        grid.render(renderer, timeline, time, scale)
    }
}
