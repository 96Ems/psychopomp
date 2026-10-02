use anyhow::Result;
use std::path::PathBuf;
fn main() -> Result<()> {
    let path = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "target/opencode-architecture/daemon-merge.json".into()),
    );
    psychopomp_opencode_architecture::build_scene()?.write_or_print(Some(&path))?;
    if let Some(parent) = path.parent() {
        psychopomp_opencode_architecture::build_deck()?
            .write_with_slides(&parent.join("deck.json"))?;
    }
    println!("{}", path.display());
    Ok(())
}
