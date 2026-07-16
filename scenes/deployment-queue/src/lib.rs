use anyhow::Result;
use kinograph::{
    author::PlanBuilder,
    deployment::{
        DeploymentAttentionTargetPlan, DeploymentItemPlan, DeploymentQueueHandle,
        DeploymentQueueSnapshotPlan,
    },
    plan::ScenePlan,
};

const SECOND: u64 = 1_000_000_000;
const DURATION: u64 = 9 * SECOND;

pub fn build_plan() -> Result<ScenePlan> {
    let web = DeploymentItemPlan::new("web", "Web Gateway");
    let api = DeploymentItemPlan::new("api", "Public API");
    let worker = DeploymentItemPlan::new("worker", "Queue Worker");

    let mut scene = PlanBuilder::new("northstar-release-control", DURATION);
    let queue = DeploymentQueueHandle::add(
        &mut scene,
        "deployment-queue",
        [web.clone(), api.clone(), worker.clone()],
        DeploymentQueueSnapshotPlan::new([web.queued(), api.queued(), worker.queued()]),
    )?;
    perspective_entrance(&mut scene, &queue);

    queue.change(
        &mut scene,
        1_400_000_000,
        DeploymentQueueSnapshotPlan::new([
            web.deploying(0.375),
            api.building(0.25),
            worker.queued(),
        ]),
    )?;
    queue.change(
        &mut scene,
        2_700_000_000,
        DeploymentQueueSnapshotPlan::new([
            web.verifying(0.75),
            api.deploying(0.5),
            worker.building(0.25),
        ]),
    )?;
    queue.change(
        &mut scene,
        3_800_000_000,
        DeploymentQueueSnapshotPlan::new([worker.deploying(0.5), api.failed(), web.succeeded()])
            .attend(api.target()),
    )?;
    queue.change(
        &mut scene,
        5_200_000_000,
        DeploymentQueueSnapshotPlan::new([
            api.building(0.125),
            worker.deploying(0.875),
            web.succeeded(),
        ]),
    )?;
    queue.change(
        &mut scene,
        6_600_000_000,
        DeploymentQueueSnapshotPlan::new([
            api.deploying(0.625),
            worker.succeeded(),
            web.succeeded(),
        ]),
    )?;
    queue.change(
        &mut scene,
        8_000_000_000,
        DeploymentQueueSnapshotPlan::new([web.succeeded(), api.succeeded(), worker.succeeded()])
            .attend(DeploymentAttentionTargetPlan::Queue),
    )?;

    scene.cue("rollout", 0, 3_800_000_000);
    scene.cue("blocked", 3_800_000_000, 5_200_000_000);
    scene.cue("retry", 5_200_000_000, 8_000_000_000);
    scene.cue("healthy", 8_000_000_000, DURATION);
    Ok(scene.finish()?)
}

fn perspective_entrance(scene: &mut PlanBuilder, queue: &DeploymentQueueHandle) {
    let channels = [
        (queue.x(), 1_035.0, 960.0),
        (queue.y(), 710.0, 540.0),
        (queue.scale(), 0.84, 1.0),
        (queue.rotation(), -0.045, 0.0),
        (queue.tilt_x(), -0.24, 0.0),
        (queue.tilt_y(), 0.32, 0.0),
        (queue.near_edge_blur(), 12.0, 0.0),
        (queue.opacity(), 0.0, 1.0),
    ];
    for (channel, initial, target) in channels {
        scene.set(channel, 0, initial);
        scene.spring(channel, 0, target, 0.72, 0.08);
    }
}

#[cfg(test)]
mod tests {
    use super::build_plan;

    const CANONICAL_PLAN: &str = include_str!("../deployment-queue.plan.json");

    #[test]
    fn deployment_phases_cues_and_state_count_are_complete() {
        let plan = build_plan().unwrap();
        let state = &plan.state_channels[0];
        let snapshots = std::iter::once(&state.initial)
            .chain(state.events.iter().map(|event| &event.value))
            .collect::<Vec<_>>();

        assert_eq!(plan.id, "northstar-release-control");
        assert_eq!(plan.duration_nanos, 9_000_000_000);
        assert_eq!(plan.actors.len(), 1);
        assert_eq!(plan.state_channels.len(), 1);
        assert_eq!(state.events.len(), 6);
        assert_eq!(snapshots.len(), 7);
        assert_eq!(
            state
                .events
                .iter()
                .map(|event| event.at_nanos)
                .collect::<Vec<_>>(),
            [
                1_400_000_000,
                2_700_000_000,
                3_800_000_000,
                5_200_000_000,
                6_600_000_000,
                8_000_000_000,
            ]
        );
        assert_eq!(
            snapshots
                .iter()
                .map(|snapshot| {
                    snapshot["items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|item| item["phase"].as_str().unwrap())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
            [
                vec!["queued", "queued", "queued"],
                vec!["deploying", "building", "queued"],
                vec!["verifying", "deploying", "building"],
                vec!["deploying", "failed", "succeeded"],
                vec!["building", "deploying", "succeeded"],
                vec!["deploying", "succeeded", "succeeded"],
                vec!["succeeded", "succeeded", "succeeded"],
            ]
        );
        assert_eq!(
            plan.cues
                .iter()
                .map(|cue| (cue.id.as_str(), cue.start_nanos, cue.end_nanos))
                .collect::<Vec<_>>(),
            [
                ("rollout", 0, 3_800_000_000),
                ("blocked", 3_800_000_000, 5_200_000_000),
                ("retry", 5_200_000_000, 8_000_000_000),
                ("healthy", 8_000_000_000, 9_000_000_000),
            ]
        );
    }

    #[test]
    fn retry_interrupts_the_previous_reorder_and_healthy_attends_the_queue() {
        let plan = build_plan().unwrap();
        let events = &plan.state_channels[0].events;
        let blocked = &events[2];
        let retry = &events[3];
        let healthy = &events[5];

        assert_eq!(blocked.at_nanos, 3_800_000_000);
        assert_eq!(blocked.value["items"][0]["itemId"], "worker");
        assert_eq!(blocked.value["attention"]["itemId"], "api");
        assert_eq!(retry.at_nanos, 5_200_000_000);
        assert_eq!(retry.value["items"][0]["itemId"], "api");
        assert_eq!(retry.value["items"][0]["phase"], "building");
        assert_eq!(healthy.value["attention"]["target"], "queue");
    }

    #[test]
    fn canonical_plan_matches_rust_scene_program() {
        assert_eq!(
            build_plan().unwrap().to_json_pretty().unwrap(),
            CANONICAL_PLAN
        );
    }
}
