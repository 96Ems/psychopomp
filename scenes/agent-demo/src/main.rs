use psychopomp::{
    author::{PlanBuilder, SECOND},
    plan::ScenePlan,
};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plan = build_plan();
    plan.validate()?;
    plan.write_or_print(std::env::args().nth(1))?;
    Ok(())
}

fn build_plan() -> ScenePlan {
    let phrases = ["Rust computes the scene", "The renderer stays hot"];
    let mut scene = PlanBuilder::new("agent-demo", 3 * SECOND);
    let title = scene
        .actor(
            "title",
            "title-card",
            json!({
            "title": "Psychopomp Scene Plan",
            "subtitle": phrases[0],
            }),
        )
        .unwrap();
    let opacity = scene.continuous(&title, "opacity", 0.0);
    let subtitle = scene.state(&title, "subtitle", phrases[0]).unwrap();
    scene.spring(&opacity, 200_000_000, 1.0, 0.4, 0.0);
    scene.spring(&opacity, 2_500_000_000, 0.0, 0.35, 0.0);
    for (index, phrase) in phrases.iter().enumerate().skip(1) {
        scene
            .change(&subtitle, (index as u64 + 1) * SECOND, phrase)
            .unwrap();
    }
    scene.cue("intro", 0, 2 * SECOND);
    scene.cue("outro", 2 * SECOND, 3 * SECOND);
    let footer = scene
        .actor(
            "footer",
            "text",
            json!({
                "text": "A second renderer adapter, same timeline core",
                "center": [960, 820],
                "fontSize": 24,
                "color": [148, 163, 184],
            }),
        )
        .unwrap();
    let footer_opacity = scene.continuous(&footer, "opacity", 0.0);
    scene.spring(&footer_opacity, SECOND, 1.0, 0.35, 0.0);
    scene.finish().unwrap()
}
