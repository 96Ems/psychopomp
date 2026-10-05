//! Changed Files: the recipe is decoded and its row channels checked during
//! preflight; preparation measures the columns and compiles the totals'
//! Rolling Numbers once. Rows are channel-only; rolling totals follow the
//! authored clock, so a list whose totals roll is export-only.
use anyhow::{Result, bail, ensure};
use psychopomp::{
    changed_files::{ChangedFilesPlan, RowChannel, accepts_property},
    plan::{ActorPlan, ContinuousChannelPlan},
};

use super::preflight::{decode, strict_channels};
use crate::render::{ChangedFilesLayout, HeadlessRenderer};

pub(super) struct ChangedFilesInput {
    id: String,
    plan: ChangedFilesPlan,
}

impl ChangedFilesInput {
    pub(super) fn new(
        actor: &ActorPlan,
        channels: &[ContinuousChannelPlan],
        duration_nanos: u64,
    ) -> Result<Self> {
        let plan = decode(actor, "changed files", ChangedFilesPlan::validate)?;
        ensure!(
            plan.totals
                .last()
                .is_none_or(|totals| totals.at_nanos <= duration_nanos),
            "changed files actor '{}' totals change after the scene ends",
            actor.id
        );
        strict_channels(&actor.id, channels, "changed files", |property| {
            accepts_property(property) || RowChannel::parse(property).is_some()
        })?;
        for channel in channels.iter().filter(|c| c.actor_id == actor.id) {
            if let Some((id, _)) = RowChannel::parse(&channel.property)
                && plan.find(id).is_none()
            {
                bail!(
                    "changed files actor '{}' channel '{}' names a file that is not listed",
                    actor.id,
                    channel.property
                );
            }
        }
        Ok(Self {
            id: actor.id.clone(),
            plan,
        })
    }

    /// Rolling totals follow the authored clock, not Playback destinations.
    pub(super) fn native(&self) -> bool {
        self.plan.totals.is_empty()
    }

    pub(super) fn prepare(self, renderer: &mut HeadlessRenderer) -> PreparedChangedFiles {
        let layout = renderer.prepare_changed_files(&self.plan);
        PreparedChangedFiles {
            id: self.id,
            plan: self.plan,
            layout,
        }
    }
}

pub(super) struct PreparedChangedFiles {
    id: String,
    plan: ChangedFilesPlan,
    layout: ChangedFilesLayout,
}

impl PreparedChangedFiles {
    /// Samples differ while a total settles, even with no channel moving.
    pub(super) fn moving(&self, time: f64) -> bool {
        self.layout.moving(time)
    }

    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &mut HeadlessRenderer,
        time: f64,
        sample: impl Fn(&str, &str, f32) -> f32,
    ) -> Result<()> {
        renderer.composite_changed_files(pixels, &self.plan, &self.layout, time, |property, d| {
            sample(&self.id, property, d)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::plan_runtime::{preflight, validate_renderer_plan};
    use psychopomp::{
        author::PlanBuilder,
        changed_files::{ChangedFilePlan, ChangedFilesActor, ChangedFilesPlan, FileStatus},
    };

    fn build(property: &str, reveal: bool) -> psychopomp::plan::ScenePlan {
        let mut scene = PlanBuilder::new("files", 4_000_000_000);
        let mut card = ChangedFilesActor::declare(
            &mut scene,
            "files",
            ChangedFilesPlan::new(
                [100.0, 100.0],
                1200.0,
                vec![
                    ChangedFilePlan::new("a", "src/a.ts", FileStatus::Added, 3, 0),
                    ChangedFilePlan::new("b", "src/b.ts", FileStatus::Modified, 4, 2),
                ],
            ),
        )
        .unwrap();
        if reveal {
            card.reveal(&mut scene, 0, 0.1).unwrap();
        }
        card.channel(&mut scene, property, 0.0);
        scene.finish().unwrap()
    }

    #[test]
    fn preflight_checks_row_channels_and_native_eligibility_without_a_gpu() {
        for valid in ["opacity", "scroll", "row.a.reveal", "row.b.dim"] {
            validate_renderer_plan(&build(valid, true)).unwrap();
        }
        for invalid in ["typed", "row.c.reveal", "row.a.wobble"] {
            assert!(
                validate_renderer_plan(&build(invalid, true)).is_err(),
                "{invalid}"
            );
        }
        assert!(
            preflight::Plan::new(build("row.a.highlight", false))
                .unwrap()
                .native()
        );
        assert!(
            !preflight::Plan::new(build("row.a.highlight", true))
                .unwrap()
                .native(),
            "rolling totals are export-only"
        );
    }
}
