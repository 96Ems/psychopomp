use std::{env, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let output = env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/charts.json"), PathBuf::from);
    psychopomp_charts::build_plan()?.write_or_print(Some(&output))?;
    eprintln!("Wrote {}", output.display());
    Ok(())
}
