use std::{
    env, fs,
    path::{Path, PathBuf},
};

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
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    fs::write(&output, serde_json::to_string_pretty(&deck)?)?;
    for slide in &deck.slides {
        fs::write(
            parent.join(format!("{}.json", slide.plan.id)),
            slide.plan.to_json_pretty()?,
        )?;
    }
    eprintln!("Wrote {} slides to {}", deck.slides.len(), output.display());
    Ok(())
}
