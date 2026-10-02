use std::{env, fs, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let output = env::args().nth(1).map_or_else(
        || PathBuf::from("target/rolling-number.json"),
        PathBuf::from,
    );
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &output,
        kinograph_rolling_number::build_plan()?.to_json_pretty()?,
    )?;
    eprintln!("Wrote {}", output.display());
    Ok(())
}
