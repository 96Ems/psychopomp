use std::{env, fs, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let output = env::args().nth(1).map_or_else(
        || PathBuf::from("target/effects-showroom.json"),
        PathBuf::from,
    );
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let reel = psychopomp_effects_showroom::build_reel()?;
    fs::write(&output, serde_json::to_string_pretty(&reel)?)?;
    eprintln!("Wrote {}", output.display());
    Ok(())
}
