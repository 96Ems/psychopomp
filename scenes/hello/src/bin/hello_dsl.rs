//! `scenes/hello` re-expressed with `psychopomp::score` composable beats.
//!
//! Emits a `ScenePlan` JSON byte-for-byte identical to `scenes/hello/src/main.rs`.
use psychopomp::{
    author::{PlanBuilder, SECOND},
    plan::ScenePlan,
    score::{Beat, stage},
    stage::{StageActor, StagePlan},
};

fn build_plan() -> anyhow::Result<ScenePlan> {
    let plan: StagePlan = serde_json::from_value(serde_json::json!({
        "elements": [
            { "kind": "card", "id": "client", "at": [560, 540, 0], "size": [300, 110], "title": "client" },
            { "kind": "orb", "id": "server", "at": [1360, 540, 0], "radius": 140 },
            { "kind": "beam", "id": "link", "from": "client", "to": "server" },
            { "kind": "packet", "id": "hello", "beam": "link", "label": "GET /hello" }
        ]
    }))?;
    let mut scene = PlanBuilder::new("hello", 4 * SECOND);
    let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;
    stage.score(&mut scene).at(
        0,
        stage::settle_in("client")
            .then(stage::connect("link", 0.6))
            .then_after(SECOND / 2, stage::send("hello", 0.8))
            .then(stage::land("server").also(stage::jolt([1.0, 0.0], 0.6))),
    );
    Ok(scene.finish()?)
}

fn main() -> anyhow::Result<()> {
    std::fs::create_dir_all("target")?;
    std::fs::write(
        "target/hello-dsl.json",
        serde_json::to_string_pretty(&build_plan()?)?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn original_plan() -> anyhow::Result<ScenePlan> {
        let plan: StagePlan = serde_json::from_value(serde_json::json!({
            "elements": [
                { "kind": "card", "id": "client", "at": [560, 540, 0], "size": [300, 110], "title": "client" },
                { "kind": "orb", "id": "server", "at": [1360, 540, 0], "radius": 140 },
                { "kind": "beam", "id": "link", "from": "client", "to": "server" },
                { "kind": "packet", "id": "hello", "beam": "link", "label": "GET /hello" }
            ]
        }))?;
        let mut scene = PlanBuilder::new("hello", 4 * SECOND);
        let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;
        let ready = stage.settle_in(&mut scene, "client", 0);
        let wired = stage.connect(&mut scene, "link", ready, 0.6);
        let landed = stage.send(&mut scene, "hello", wired + SECOND / 2, 0.8);
        stage.land(&mut scene, "server", landed);
        stage.jolt(&mut scene, landed, [1.0, 0.0], 0.6);
        Ok(scene.finish()?)
    }

    #[test]
    fn hello_dsl_emits_byte_identical_scene_plan_json() {
        let original = serde_json::to_string_pretty(&original_plan().unwrap()).unwrap();
        let dsl = serde_json::to_string_pretty(&build_plan().unwrap()).unwrap();
        assert_eq!(dsl, original);
    }
}
