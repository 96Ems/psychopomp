use std::{env, fs, path::PathBuf};

/// Writes the reel to `target/footage.json` (or the given path) and each
/// segment beside it, so the clips' relative media paths hold for both.
/// `--bench` writes the twenty-clip measuring wall to `target/footage-bench.json`.
fn main() -> anyhow::Result<()> {
    if env::args().nth(1).as_deref() == Some("--bench") {
        let output = PathBuf::from("target/footage-bench.json");
        psychopomp_footage::build_bench()?.write_or_print(Some(&output))?;
        eprintln!("Wrote {}", output.display());
        return Ok(());
    }
    let output = env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/footage.json"), PathBuf::from);
    let reel = psychopomp_footage::build_reel()?;
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
