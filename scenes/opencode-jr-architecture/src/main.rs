use std::{env, fs, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut narration = root.join("narration");
    let mut output = root.join("opencode-jr-architecture.reel.json");
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--narration" => {
                narration = PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--narration requires a directory"))?,
                )
                .canonicalize()?
            }
            "--output" => {
                output = PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--output requires a file"))?,
                )
            }
            _ => anyhow::bail!("unknown argument: {arg}"),
        }
    }
    let mut reel = kinograph_opencode_jr_architecture::build_reel(&narration)?;
    // Alternate exports can live outside the scene directory. Resolve media
    // from the selected narration and the original scene, not the output folder.
    if output.parent() != Some(root.as_path()) || narration != root.join("narration") {
        for segment in &mut reel.segments {
            for media in &mut segment.plan.media {
                media.path = if let Ok(relative) = media.path.strip_prefix("narration") {
                    narration.join(relative)
                } else {
                    root.join(&media.path)
                };
            }
        }
    }
    fs::write(&output, serde_json::to_string_pretty(&reel)? + "\n")?;
    eprintln!(
        "wrote {} ({:.1}s, {} segments)",
        output.display(),
        reel.duration_nanos() as f64 / 1e9,
        reel.segments.len()
    );
    Ok(())
}
