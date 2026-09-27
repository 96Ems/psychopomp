use std::{
    env, fs,
    path::{Path, PathBuf},
};

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
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    fs::write(&output, serde_json::to_string_pretty(&deck)?)?;
    for slide in &deck.slides {
        fs::write(
            parent.join(format!("{}.json", slide.plan.id)),
            slide.plan.to_json_pretty()?,
        )?;
    }
    eprintln!(
        "Wrote {} component trials to {}",
        deck.slides.len(),
        output.display()
    );
    Ok(())
}
