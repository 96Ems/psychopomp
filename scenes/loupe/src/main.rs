use std::{env, fs, path::PathBuf};

/// Writes the reel to `target/loupe.json` (or the given path) and each
/// segment beside it under `target/loupe/`.
fn main() -> anyhow::Result<()> {
    let output = env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/loupe.json"), PathBuf::from);
    let reel = psychopomp_loupe::build_reel()?;
    let segments = output.with_extension("");
    fs::create_dir_all(&segments)?;
    for segment in &reel.segments {
        segment
            .plan
            .write_or_print(Some(segments.join(format!("{}.json", segment.plan.id))))?;
    }
    fs::write(&output, serde_json::to_string_pretty(&reel)? + "\n")?;
    eprintln!("Wrote {} and {}/", output.display(), segments.display());
    Ok(())
}
