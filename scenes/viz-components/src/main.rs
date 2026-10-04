use std::{env, fs, path::PathBuf};

/// Writes the reel to `target/viz-components/reel.json` (or the given path),
/// each segment beside it, and a copy of the narration the subtitles play.
fn main() -> anyhow::Result<()> {
    let output = env::args().nth(1).map_or_else(
        || PathBuf::from("target/viz-components/reel.json"),
        PathBuf::from,
    );
    let directory = output.parent().map(PathBuf::from).unwrap_or_default();
    let narration = psychopomp_viz_components::narration_dir();
    let reel = psychopomp_viz_components::build_reel(&narration)?;
    fs::create_dir_all(directory.join("narration"))?;
    for file in psychopomp_viz_components::NARRATION_FILES {
        fs::copy(narration.join(file), directory.join("narration").join(file))?;
    }
    for segment in &reel.segments {
        segment
            .plan
            .write_or_print(Some(directory.join(format!("{}.json", segment.plan.id))))?;
    }
    fs::write(&output, serde_json::to_string_pretty(&reel)? + "\n")?;
    eprintln!("Wrote {} and its segments", output.display());
    Ok(())
}
