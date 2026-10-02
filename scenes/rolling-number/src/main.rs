use std::{env, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let output = env::args().nth(1).map_or_else(
        || PathBuf::from("target/rolling-number.json"),
        PathBuf::from,
    );
    kinograph_rolling_number::build_plan()?.write_or_print(Some(&output))?;
    eprintln!("Wrote {}", output.display());
    Ok(())
}
