use std::{env, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);
    let first = args.next();
    let slideshow = first.as_deref() == Some("--slideshow");
    let deck = if slideshow {
        kinograph_component_prototypes::build_slideshow_deck()?
    } else {
        kinograph_component_prototypes::build_deck()?
    };
    let output = if slideshow { args.next() } else { first }
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(if slideshow {
                "target/slideshow-components/deck.json"
            } else {
                "target/component-prototypes/deck.json"
            })
        });
    deck.write_with_slides(&output)?;
    eprintln!(
        "Wrote {} component trials to {}",
        deck.slides.len(),
        output.display()
    );
    Ok(())
}
