use crate::render::HeadlessRenderer;
use anyhow::Result;
use kinograph::component_prototype::VennPlan;

pub(super) struct PreparedVenn {
    id: String,
    plan: VennPlan,
}
impl PreparedVenn {
    pub(super) fn new(actor: &kinograph::plan::ActorPlan) -> Result<Self> {
        let plan: VennPlan = serde_json::from_value(actor.data.clone())?;
        crate::render::validate_venn(&plan)?;
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
        renderer.composite_venn(pixels, &self.plan, |p, d| sample(&self.id, p, d));
    }
}
