use std::{env, path::PathBuf};

/// Writes the showroom plan to `target/diagnostics.json` (or the given path).
fn main() -> anyhow::Result<()> {
    let output = env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/diagnostics.json"), PathBuf::from);
    psychopomp_diagnostics::build()?.write_or_print(Some(output.clone()))?;
    eprintln!("Wrote {}", output.display());
    Ok(())
}
