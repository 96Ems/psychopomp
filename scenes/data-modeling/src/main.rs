use std::{env, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let deck = psychopomp_data_modeling::build_deck()?;
    let output = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/data-modeling/deck.json"));
    deck.write_with_slides(&output)?;
    eprintln!("Wrote {} slides to {}", deck.slides.len(), output.display());
    Ok(())
}
