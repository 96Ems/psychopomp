use anyhow::Result;
use std::path::PathBuf;
fn main() -> Result<()> {
    let path = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "target/opencode-architecture/daemon-merge.json".into()),
    );
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&kinograph_opencode_architecture::build_scene()?)?,
    )?;
    if let Some(parent) = path.parent() {
        let deck = kinograph_opencode_architecture::build_deck()?;
        for slide in &deck.slides {
            std::fs::write(
                parent.join(format!("{}.json", slide.plan.id)),
                serde_json::to_vec_pretty(&slide.plan)?,
            )?;
        }
        std::fs::write(parent.join("deck.json"), serde_json::to_vec_pretty(&deck)?)?;
    }
    println!("{}", path.display());
    Ok(())
}
