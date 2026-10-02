use std::{env, fs, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut narration = root.join("narration");
    let mut only = None;
    let mut output = None;
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
                output =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        anyhow::anyhow!("--output requires a file")
                    })?))
            }
            _ if !arg.starts_with('-') && only.is_none() => only = Some(arg),
            _ => anyhow::bail!("unknown argument: {arg}"),
        }
    }
    let files = ["pr-walkthrough.reel.json", "pr-50825.reel.json"];
    let count = files
        .iter()
        .filter(|file| only.as_deref().is_none_or(|name| file.starts_with(name)))
        .count();
    anyhow::ensure!(count > 0, "no reel matches the selection");
    anyhow::ensure!(
        output.is_none() || count == 1,
        "--output requires selecting one reel"
    );
    for file in files {
        if only.as_deref().is_some_and(|name| !file.starts_with(name)) {
            continue;
        }
        let mut reel = if file == "pr-50825.reel.json" {
            psychopomp_pr_walkthrough::build_flagship(&narration)?
        } else {
            psychopomp_pr_walkthrough::build_reel(&narration)?
        };
        // Alternate exports can live outside the scene directory. Resolve media
        // from the selected narration and original scene, not the output folder.
        if output.is_some() || narration != root.join("narration") {
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
        let output = output.clone().unwrap_or_else(|| root.join(file));
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
