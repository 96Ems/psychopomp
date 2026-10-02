use crate::render::{HeadlessRenderer, RichTextGlyphs};
use anyhow::Result;

pub(super) struct PreparedRichText {
    id: String,
    origin: [f32; 2],
    glyphs: RichTextGlyphs,
}
impl PreparedRichText {
    pub(super) fn from_source(
        id: String,
        source: crate::render::RichTextSource,
        renderer: &mut HeadlessRenderer,
    ) -> Result<Self> {
        let origin = source.plan.origin;
        let glyphs = renderer.prepare_rich_text_source(source)?;
        Ok(Self { id, origin, glyphs })
    }
    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &mut HeadlessRenderer,
        sample: impl Fn(&str, &str, f32) -> f32,
    ) {
        let origin = [
            sample(&self.id, "x", self.origin[0]),
            sample(&self.id, "y", self.origin[1]),
        ];
        renderer.composite_rich_text(pixels, &self.glyphs, origin, |p, d| sample(&self.id, p, d));
    }
}

#[cfg(test)]
mod tests {
    use crate::plan_runtime::{PreparedPlan, new_renderer};
    use kinograph::playback::PlaybackCommand;
    use std::{path::Path, time::Duration};
    #[test]
    #[ignore = "requires a headless GPU; rich text, headers, width text, and Venn under interrupted navigation"]
    fn new_components_preserve_pixels_and_velocity_on_reverse_skip_and_reduced_motion() {
        let mut renderer = pollster::block_on(new_renderer("slideshow-component-proof")).unwrap();
        for slide in kinograph_component_prototypes::build_slideshow_deck()
            .unwrap()
            .slides
            .into_iter()
            .enumerate()
            .filter(|(index, _)| *index < 5 || *index == 7)
            .map(|(_, slide)| slide)
        {
            let p = PreparedPlan::prepare(slide.plan, Path::new("."), &mut renderer).unwrap();
            let mut playback = p.playback(false).unwrap();
            let mut now = Duration::ZERO;
            for command in [
                PlaybackCommand::Next,
                PlaybackCommand::Previous,
                PlaybackCommand::Next,
                PlaybackCommand::Last,
                PlaybackCommand::Previous,
                PlaybackCommand::First,
            ] {
                let boundary = crate::plan_runtime::proof::interrupt(
                    &p,
                    &mut renderer,
                    &mut playback,
                    now,
                    command,
                )
                .unwrap();
                boundary.assert_pixels(&p, &mut renderer);
                boundary.assert_states_within(&p, 0.001);
                boundary.sample_later_and_repeat(&p, &mut renderer, 0.12);
                now += Duration::from_millis(130);
            }
            crate::plan_runtime::proof::assert_reduced_motion_holds(&p, &mut renderer);
        }
    }
}
