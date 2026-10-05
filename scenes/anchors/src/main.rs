use std::{env, fs, path::PathBuf};

/// Writes the reel to `target/anchors.json` (or the given path) and each
/// segment beside it, so the image's relative media path holds for both.
fn main() -> anyhow::Result<()> {
    let output = env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/anchors.json"), PathBuf::from);
    let reel = psychopomp_anchors::build_reel()?;
    let directory = output.parent().unwrap_or_else(|| "".as_ref());
    fs::create_dir_all(directory)?;
    for segment in &reel.segments {
        let path = directory.join(format!("{}.json", segment.plan.id));
        segment.plan.write_or_print(Some(&path))?;
        eprintln!("Wrote {}", path.display());
    }
    fs::write(&output, serde_json::to_string_pretty(&reel)? + "\n")?;
    eprintln!("Wrote {}", output.display());
    Ok(())
}
