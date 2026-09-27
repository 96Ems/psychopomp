use std::{env, fs, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let narration = root.join("narration");
    let only = env::args().nth(1);
    let reels = [
        (
            "pr-walkthrough.reel.json",
            kinograph_pr_walkthrough::build_reel(&narration)?,
        ),
        (
            "pr-50825.reel.json",
            kinograph_pr_walkthrough::build_flagship(&narration)?,
        ),
    ];
    for (file, reel) in reels {
        if only.as_deref().is_some_and(|name| !file.starts_with(name)) {
            continue;
        }
        let output = root.join(file);
        fs::write(&output, serde_json::to_string_pretty(&reel)? + "\n")?;
        eprintln!(
            "wrote {} ({:.1}s, {} segments)",
            output.display(),
            reel.duration_nanos() as f64 / 1e9,
            reel.segments.len()
        );
    }
    Ok(())
}
