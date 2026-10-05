//! `psychopomp-media adopt <narration-dir> [<scene-dir>]` records existing
//! `scripts/narrate.ts` narration in the scene's Media Lock without
//! regenerating it; `psychopomp-media show <scene-dir>` lists a lock.
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["adopt", narration, ..] => {
            let narration = PathBuf::from(narration);
            let root = match args.get(2) {
                Some(root) => PathBuf::from(root),
                None => narration
                    .canonicalize()?
                    .parent()
                    .map(Path::to_owned)
                    .context("the narration directory has no parent")?,
            };
            let adopted = psychopomp_media::adopt(&narration, &root)?;
            println!(
                "adopted {} clip(s) into {}: {}",
                adopted.len(),
                root.join("media.lock.json").display(),
                adopted.join(", ")
            );
        }
        ["show", root] => print!("{}", psychopomp_media::show(Path::new(root))?),
        _ => bail!(
            "usage: psychopomp-media adopt <narration-dir> [<scene-dir>]\n       psychopomp-media show <scene-dir>"
        ),
    }
    Ok(())
}
