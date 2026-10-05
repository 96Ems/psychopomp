//! The README example: a card plugs into an orb and sends it a packet.
use psychopomp::{
    author::{PlanBuilder, SECOND},
    stage::{StageActor, StageElement, StagePlan},
};

fn main() -> anyhow::Result<()> {
    let plan = StagePlan {
        post: Default::default(),
        elements: vec![
            StageElement::card("client", [560.0, 540.0, 0.0], [300.0, 110.0], "client"),
            StageElement::orb("server", [1360.0, 540.0, 0.0], 140.0),
            StageElement::beam("link", "client", "server"),
            StageElement::packet("hello", "link").labeled("GET /hello"),
        ],
    };
    let mut scene = PlanBuilder::new("hello", 4 * SECOND);
    let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;
    let ready = stage.settle_in(&mut scene, "client", 0); // the card drifts into place
    let wired = stage.connect(&mut scene, "link", ready, 0.6); // the wire draws on
    let landed = stage.send(&mut scene, "hello", wired + SECOND / 2, 0.8); // a packet flies
    stage.land(&mut scene, "server", landed); // the orb lights up
    stage.jolt(&mut scene, landed, [1.0, 0.0], 0.6); // and the camera takes the hit
    std::fs::create_dir_all("target")?;
    std::fs::write(
        "target/hello.json",
        serde_json::to_string_pretty(&scene.finish()?)?,
    )?;
    Ok(())
}
