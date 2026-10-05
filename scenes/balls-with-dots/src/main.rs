use std::{fs, path::PathBuf};

/// Writes `balls-with-dots.reel.json` beside the narration and audio it plays.
fn main() -> anyhow::Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let reel = psychopomp_balls_with_dots::build(&root)?;
    let output = root.join("balls-with-dots.reel.json");
    fs::write(&output, serde_json::to_string_pretty(&reel)? + "\n")?;
    eprintln!(
        "wrote {} ({:.2}s)",
        output.display(),
        reel.duration_nanos() as f64 / 1e9
    );
    Ok(())
}
