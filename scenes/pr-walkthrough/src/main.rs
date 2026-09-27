use std::{env, fs, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let reel = kinograph_pr_walkthrough::build_reel(&root.join("narration"))?;
    let output = env::args()
        .nth(1)
        .map_or_else(|| root.join("pr-walkthrough.reel.json"), PathBuf::from);
    fs::write(&output, serde_json::to_string_pretty(&reel)? + "\n")?;
    eprintln!(
        "wrote {} ({:.1}s, {} segments)",
        output.display(),
        reel.duration_nanos() as f64 / 1e9,
        reel.segments.len()
    );
    Ok(())
}
