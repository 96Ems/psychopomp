use std::{env, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);
    let first = args.next();
    let styles = first.as_deref() == Some("--styles");
    let deck = if styles {
        kinograph_keyed_grid::build_style_deck()?
    } else {
        kinograph_keyed_grid::build_deck()?
    };
    let output = if styles { args.next() } else { first }
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(if styles {
                "target/grid-styles/deck.json"
            } else {
                "target/keyed-grid/deck.json"
            })
        });
    deck.write_with_slides(&output)?;
    eprintln!("Wrote {} slides to {}", deck.slides.len(), output.display());
    Ok(())
}
