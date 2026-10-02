use std::collections::HashSet;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::author::{ActorHandle, ContinuousHandle, PlanBuilder, StateHandle};

pub const DEPLOYMENT_QUEUE_RECIPE: &str = "deployment-queue";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentQueueRecipePlan {
    pub product: String,
    pub title: String,
    pub subtitle: String,
    pub environment: String,
    pub release: String,
    pub items: Vec<DeploymentItemPlan>,
}

impl DeploymentQueueRecipePlan {
    pub fn new(
        product: impl Into<String>,
        title: impl Into<String>,
        subtitle: impl Into<String>,
        environment: impl Into<String>,
        release: impl Into<String>,
        items: impl IntoIterator<Item = DeploymentItemPlan>,
    ) -> Self {
        Self {
            product: product.into(),
            title: title.into(),
            subtitle: subtitle.into(),
            environment: environment.into(),
            release: release.into(),
            items: items.into_iter().collect(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        validate_recipe(self)?;
        validate_catalog(&self.items)?;
        Ok(())
    }

    pub fn validate_snapshot(&self, snapshot: &DeploymentQueueSnapshotPlan) -> Result<()> {
        validate_snapshot(snapshot, &validate_catalog(&self.items)?)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentItemPlan {
    pub id: String,
    pub label: String,
}

impl DeploymentItemPlan {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }

    pub fn queued(&self) -> DeploymentItemSnapshotPlan {
        self.snapshot(DeploymentPhasePlan::Queued)
    }

    pub fn building(&self, progress: f32) -> DeploymentItemSnapshotPlan {
        self.snapshot(DeploymentPhasePlan::Building { progress })
    }

    pub fn deploying(&self, progress: f32) -> DeploymentItemSnapshotPlan {
        self.snapshot(DeploymentPhasePlan::Deploying { progress })
    }

    pub fn verifying(&self, progress: f32) -> DeploymentItemSnapshotPlan {
        self.snapshot(DeploymentPhasePlan::Verifying { progress })
    }

    pub fn succeeded(&self) -> DeploymentItemSnapshotPlan {
        self.snapshot(DeploymentPhasePlan::Succeeded)
    }

    pub fn failed(&self) -> DeploymentItemSnapshotPlan {
        self.snapshot(DeploymentPhasePlan::Failed)
    }

    pub fn target(&self) -> DeploymentAttentionTargetPlan {
        DeploymentAttentionTargetPlan::Item {
            item_id: self.id.clone(),
        }
    }

    fn snapshot(&self, phase: DeploymentPhasePlan) -> DeploymentItemSnapshotPlan {
        DeploymentItemSnapshotPlan {
            item_id: self.id.clone(),
            phase,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentQueueSnapshotPlan {
    pub items: Vec<DeploymentItemSnapshotPlan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attention: Option<DeploymentAttentionTargetPlan>,
}

impl DeploymentQueueSnapshotPlan {
    pub fn new(items: impl IntoIterator<Item = DeploymentItemSnapshotPlan>) -> Self {
        Self {
            items: items.into_iter().collect(),
            attention: None,
        }
    }

    pub fn attend(mut self, target: DeploymentAttentionTargetPlan) -> Self {
        self.attention = Some(target);
        self
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentItemSnapshotPlan {
    pub item_id: String,
    #[serde(flatten)]
    pub phase: DeploymentPhasePlan,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "phase"
)]
pub enum DeploymentPhasePlan {
    Queued,
    Building { progress: f32 },
    Deploying { progress: f32 },
    Verifying { progress: f32 },
    Succeeded,
    Failed,
}

impl DeploymentPhasePlan {
    fn progress(self) -> Option<f32> {
        match self {
            Self::Building { progress }
            | Self::Deploying { progress }
            | Self::Verifying { progress } => Some(progress),
            Self::Queued | Self::Succeeded | Self::Failed => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "target"
)]
pub enum DeploymentAttentionTargetPlan {
    Queue,
    Item { item_id: String },
}

#[derive(Clone, Debug)]
pub struct DeploymentQueueHandle {
    actor: ActorHandle,
    snapshot: StateHandle,
    x: ContinuousHandle,
    y: ContinuousHandle,
    scale: ContinuousHandle,
    rotation: ContinuousHandle,
    tilt_x: ContinuousHandle,
    tilt_y: ContinuousHandle,
    near_edge_blur: ContinuousHandle,
    opacity: ContinuousHandle,
    catalog_ids: HashSet<String>,
}

impl DeploymentQueueHandle {
    pub fn add(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        recipe: DeploymentQueueRecipePlan,
        initial: DeploymentQueueSnapshotPlan,
    ) -> Result<Self> {
        validate_recipe(&recipe)?;
        let catalog_ids = validate_catalog(&recipe.items)?;
        validate_snapshot(&initial, &catalog_ids)?;

        let actor = scene.actor(id, DEPLOYMENT_QUEUE_RECIPE, recipe)?;
        let snapshot = scene.state(&actor, "snapshot", initial)?;
        let x = scene.continuous(&actor, "x", 960.0);
        let y = scene.continuous(&actor, "y", 540.0);
        let scale = scene.continuous(&actor, "scale", 1.0);
        let rotation = scene.continuous(&actor, "rotation", 0.0);
        let tilt_x = scene.continuous(&actor, "tilt-x", 0.0);
        let tilt_y = scene.continuous(&actor, "tilt-y", 0.0);
        let near_edge_blur = scene.continuous(&actor, "near-edge-blur", 0.0);
        let opacity = scene.continuous(&actor, "opacity", 1.0);

        Ok(Self {
            actor,
            snapshot,
            x,
            y,
            scale,
            rotation,
            tilt_x,
            tilt_y,
            near_edge_blur,
            opacity,
            catalog_ids,
        })
    }

    pub fn change(
        &self,
        scene: &mut PlanBuilder,
        at_nanos: u64,
        snapshot: DeploymentQueueSnapshotPlan,
    ) -> Result<()> {
        validate_snapshot(&snapshot, &self.catalog_ids)?;
        scene.change(&self.snapshot, at_nanos, snapshot)?;
        Ok(())
    }

    pub fn actor(&self) -> &ActorHandle {
        &self.actor
    }

    pub fn x(&self) -> &ContinuousHandle {
        &self.x
    }

    pub fn y(&self) -> &ContinuousHandle {
        &self.y
    }

    pub fn scale(&self) -> &ContinuousHandle {
        &self.scale
    }

    pub fn rotation(&self) -> &ContinuousHandle {
        &self.rotation
    }

    pub fn tilt_x(&self) -> &ContinuousHandle {
        &self.tilt_x
    }

    pub fn tilt_y(&self) -> &ContinuousHandle {
        &self.tilt_y
    }

    pub fn near_edge_blur(&self) -> &ContinuousHandle {
        &self.near_edge_blur
    }

    pub fn opacity(&self) -> &ContinuousHandle {
        &self.opacity
    }
}

fn validate_recipe(recipe: &DeploymentQueueRecipePlan) -> Result<()> {
    for (name, value) in [
        ("product", recipe.product.as_str()),
        ("title", recipe.title.as_str()),
        ("subtitle", recipe.subtitle.as_str()),
        ("environment", recipe.environment.as_str()),
        ("release", recipe.release.as_str()),
    ] {
        if value.trim().is_empty() {
            bail!("deployment queue recipe {name} must not be empty");
        }
    }
    Ok(())
}

fn validate_catalog(items: &[DeploymentItemPlan]) -> Result<HashSet<String>> {
    let mut ids = HashSet::new();
    for item in items {
        if item.id.trim().is_empty() {
            bail!("deployment item catalog ID must not be empty");
        }
        if item.label.trim().is_empty() {
            bail!("deployment item catalog label must not be empty");
        }
        if !ids.insert(item.id.clone()) {
            bail!(
                "deployment item catalog ID '{}' is declared more than once",
                item.id
            );
        }
    }
    Ok(ids)
}

fn validate_snapshot(
    snapshot: &DeploymentQueueSnapshotPlan,
    catalog_ids: &HashSet<String>,
) -> Result<()> {
    let mut visible_ids = HashSet::new();
    for item in &snapshot.items {
        if !visible_ids.insert(item.item_id.as_str()) {
            bail!(
                "deployment snapshot item '{}' is declared more than once",
                item.item_id
            );
        }
        if !catalog_ids.contains(&item.item_id) {
            bail!(
                "deployment snapshot references unknown item '{}'",
                item.item_id
            );
        }
        if let Some(progress) = item.phase.progress()
            && (!progress.is_finite() || !(0.0..=1.0).contains(&progress))
        {
            bail!(
                "deployment snapshot item '{}' progress must be finite and in [0, 1]",
                item.item_id
            );
        }
    }

    if let Some(DeploymentAttentionTargetPlan::Item { item_id }) = &snapshot.attention
        && !visible_ids.contains(item_id.as_str())
    {
        bail!(
            "deployment attention target '{}' must be visible in the snapshot",
            item_id
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        DeploymentItemPlan, DeploymentQueueHandle, DeploymentQueueRecipePlan,
        DeploymentQueueSnapshotPlan,
    };
    use crate::{author::PlanBuilder, plan::ScalarPlan};

    #[test]
    fn deployment_queue_builds_a_deterministic_plan_shape() {
        let api = DeploymentItemPlan::new("api", "API");
        let web = DeploymentItemPlan::new("web", "Web");
        let mut scene = PlanBuilder::new("deploy", 2_000_000_000);
        let queue = DeploymentQueueHandle::add(
            &mut scene,
            "deployments",
            DeploymentQueueRecipePlan::new(
                "NORTHSTAR",
                "Release Control",
                "Live deployment telemetry",
                "PRODUCTION",
                "release-2026.07.16",
                [api.clone(), web.clone()],
            ),
            DeploymentQueueSnapshotPlan::new([api.queued(), web.building(0.25)])
                .attend(web.target()),
        )
        .unwrap();
        queue
            .change(
                &mut scene,
                1_000_000_000,
                DeploymentQueueSnapshotPlan::new([api.deploying(0.5), web.succeeded()])
                    .attend(api.target()),
            )
            .unwrap();
        let plan = scene.finish().unwrap();

        assert_eq!(plan.actors[0].recipe, "deployment-queue");
        assert_eq!(
            plan.actors[0].data,
            json!({
                "items": [
                    { "id": "api", "label": "API" },
                    { "id": "web", "label": "Web" }
                ],
                "product": "NORTHSTAR",
                "title": "Release Control",
                "subtitle": "Live deployment telemetry",
                "environment": "PRODUCTION",
                "release": "release-2026.07.16"
            })
        );
        assert_eq!(
            plan.state_channels[0].initial,
            json!({
                "items": [
                    { "itemId": "api", "phase": "queued" },
                    { "itemId": "web", "phase": "building", "progress": 0.25 }
                ],
                "attention": { "target": "item", "itemId": "web" }
            })
        );
        assert_eq!(
            plan.continuous_channels
                .iter()
                .map(|channel| {
                    let ScalarPlan::Literal(initial) = &channel.initial else {
                        panic!("deployment queue channel defaults must be literals");
                    };
                    (channel.property.as_str(), *initial)
                })
                .collect::<Vec<_>>(),
            [
                ("x", 960.0),
                ("y", 540.0),
                ("scale", 1.0),
                ("rotation", 0.0),
                ("tilt-x", 0.0),
                ("tilt-y", 0.0),
                ("near-edge-blur", 0.0),
                ("opacity", 1.0),
            ]
        );
        assert_eq!(queue.actor().id(), "deployments");
        assert_eq!(queue.near_edge_blur().id(), "deployments.near-edge-blur");
        assert_eq!(plan.state_channels[0].events.len(), 1);
    }

    #[test]
    fn invalid_snapshots_are_rejected_before_the_plan_changes() {
        let api = DeploymentItemPlan::new("api", "API");
        let hidden = DeploymentItemPlan::new("hidden", "Hidden");
        let mut scene = PlanBuilder::new("deploy", 1_000_000_000);
        let queue = DeploymentQueueHandle::add(
            &mut scene,
            "deployments",
            DeploymentQueueRecipePlan::new(
                "NORTHSTAR",
                "Release Control",
                "Live deployment telemetry",
                "PRODUCTION",
                "release-2026.07.16",
                [api.clone(), hidden.clone()],
            ),
            DeploymentQueueSnapshotPlan::new([api.queued()]),
        )
        .unwrap();

        assert!(
            queue
                .change(
                    &mut scene,
                    500_000_000,
                    DeploymentQueueSnapshotPlan::new([api.building(f32::NAN)]),
                )
                .is_err()
        );
        assert!(
            queue
                .change(
                    &mut scene,
                    500_000_000,
                    DeploymentQueueSnapshotPlan::new([api.queued()]).attend(hidden.target()),
                )
                .is_err()
        );
        let unknown = DeploymentItemPlan::new("unknown", "Unknown");
        assert!(
            queue
                .change(
                    &mut scene,
                    500_000_000,
                    DeploymentQueueSnapshotPlan::new([unknown.failed()]),
                )
                .is_err()
        );

        let plan = scene.finish().unwrap();
        assert!(plan.state_channels[0].events.is_empty());
    }

    #[test]
    fn invalid_catalogs_are_rejected_before_adding_an_actor() {
        let mut scene = PlanBuilder::new("deploy", 1_000_000_000);
        let duplicate = DeploymentItemPlan::new("api", "Duplicate");
        let result = DeploymentQueueHandle::add(
            &mut scene,
            "deployments",
            DeploymentQueueRecipePlan::new(
                "NORTHSTAR",
                "Release Control",
                "Live deployment telemetry",
                "PRODUCTION",
                "release-2026.07.16",
                [DeploymentItemPlan::new("api", "API"), duplicate],
            ),
            DeploymentQueueSnapshotPlan::new([]),
        );

        assert!(result.is_err());
        assert!(scene.finish().unwrap().actors.is_empty());
    }

    #[test]
    fn add_preserves_validation_precedence_and_the_entire_builder_on_error() {
        let api = DeploymentItemPlan::new("api", "API");
        let unknown = DeploymentItemPlan::new("unknown", "Unknown");
        let invalid_snapshot = DeploymentQueueSnapshotPlan::new([unknown.building(f32::NAN)]);
        let builder = |populated| {
            let mut scene = PlanBuilder::new("deploy", 1_000_000_000);
            if populated {
                let actor = scene
                    .actor("retained", "title-card", json!({"title": "Keep"}))
                    .unwrap();
                let opacity = scene.continuous(&actor, "opacity", 0.0);
                scene.spring(&opacity, 100_000_000, 1.0, 0.4, 0.0);
                scene.state(&actor, "subtitle", "Unchanged").unwrap();
            }
            scene
        };
        for (product, items, snapshot, expected) in [
            (
                " ",
                vec![api.clone(), api.clone()],
                invalid_snapshot.clone(),
                "deployment queue recipe product must not be empty",
            ),
            (
                "APP",
                vec![api.clone(), api.clone()],
                invalid_snapshot.clone(),
                "deployment item catalog ID 'api' is declared more than once",
            ),
            (
                "APP",
                vec![api.clone()],
                invalid_snapshot,
                "deployment snapshot references unknown item 'unknown'",
            ),
            (
                "APP",
                vec![api.clone()],
                DeploymentQueueSnapshotPlan::new([api.building(f32::NAN)]).attend(unknown.target()),
                "deployment snapshot item 'api' progress must be finite and in [0, 1]",
            ),
            (
                "APP",
                vec![api.clone()],
                DeploymentQueueSnapshotPlan::new([api.queued()]).attend(unknown.target()),
                "deployment attention target 'unknown' must be visible in the snapshot",
            ),
        ] {
            let recipe = DeploymentQueueRecipePlan::new(
                product,
                "Release",
                "Telemetry",
                "PRODUCTION",
                "release-1",
                items,
            );
            for populated in [false, true] {
                let mut scene = builder(populated);
                let error = DeploymentQueueHandle::add(
                    &mut scene,
                    "deployments",
                    recipe.clone(),
                    snapshot.clone(),
                )
                .unwrap_err();
                assert_eq!(error.to_string(), expected);
                assert_eq!(
                    scene.finish().unwrap().to_json_pretty().unwrap(),
                    builder(populated)
                        .finish()
                        .unwrap()
                        .to_json_pretty()
                        .unwrap()
                );
            }
        }
    }
}
