use std::{env, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let deck = psychopomp_opencode_quality::build_deck()?;
    let output = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/opencode-quality/deck.json"));
    deck.write_with_slides(&output)?;
    eprintln!("Wrote {} slides to {}", deck.slides.len(), output.display());
    Ok(())
}
