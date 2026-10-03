use std::{env, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let arguments: Vec<_> = env::args().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|argument| argument == "--video")
    {
        let narration = arguments
            .get(1)
            .ok_or_else(|| anyhow::anyhow!("--video needs a narration directory"))?;
        let output = arguments
            .get(2)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("output/opencode-quality/reel.json"));
        let reel = psychopomp_opencode_quality::video::build_reel(std::path::Path::new(narration))?;
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&output, serde_json::to_string_pretty(&reel)?)?;
        eprintln!(
            "Wrote {} narrated segments to {}",
            reel.segments.len(),
            output.display()
        );
        return Ok(());
    }
    let deck = psychopomp_opencode_quality::build_deck()?;
    let output = arguments
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/opencode-quality/deck.json"));
    deck.write_with_slides(&output)?;
    eprintln!("Wrote {} slides to {}", deck.slides.len(), output.display());
    Ok(())
}
