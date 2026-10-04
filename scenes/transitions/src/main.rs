use std::{env, fs, path::PathBuf};

/// Writes the reel to `target/transitions.json` (or the given path).
fn main() -> anyhow::Result<()> {
    let output = env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/transitions.json"), PathBuf::from);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let reel = psychopomp_transitions::build_reel()?;
    fs::write(&output, serde_json::to_string_pretty(&reel)? + "\n")?;
    eprintln!(
        "Wrote {} ({:.1}s)",
        output.display(),
        reel.duration_nanos() as f64 / 1e9
    );
    Ok(())
}
