//! Stage existing modules under mod.rs paths so Rust resolves their child modules
//! without modifying the native source or checking in a divergent renderer copy.
use std::{env, fs, path::PathBuf};
fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../../crates/kinograph-render/src");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    for (kind, files) in [
        (
            "render",
            vec![
                ("grid.rs", "mod.rs"),
                ("grid.wgsl", "grid.wgsl"),
                ("grid/edges.rs", "edges.rs"),
                ("grid/edges.wgsl", "edges.wgsl"),
                ("grid/edges_composite.wgsl", "edges_composite.wgsl"),
                ("grid/palette.rs", "palette.rs"),
                ("text/raster.rs", "text_raster.rs"),
            ],
        ),
        (
            "plan_runtime",
            vec![
                ("grid.rs", "mod.rs"),
                ("grid/disclosure.rs", "disclosure.rs"),
                ("grid/table.rs", "table.rs"),
            ],
        ),
    ] {
        let dir = out.join(kind);
        fs::create_dir_all(&dir).unwrap();
        for (source, dest) in files {
            let source = root.join(kind).join(source);
            println!("cargo:rerun-if-changed={}", source.display());
            fs::copy(source, dir.join(dest)).unwrap();
        }
        for name in if kind == "render" {
            vec!["diagram.rs", "diagram.wgsl"]
        } else {
            vec!["diagram.rs", "generated.rs"]
        } {
            let source = root.join(kind).join(name);
            println!("cargo:rerun-if-changed={}", source.display());
            fs::copy(source, dir.join(name)).unwrap();
        }
        let generated = if kind == "plan_runtime" {
            format!("\n#[allow(dead_code)] #[path = {:?}] mod generated;",dir.join("generated.rs"))
        } else {
            format!("\n#[cfg(not(target_arch = \"wasm32\"))] #[path = {:?}] mod text_raster;",dir.join("text_raster.rs"))
        };
        fs::write(
            out.join(format!("{kind}.rs")),
            format!(
                "#[path = {:?}] mod grid;\n#[path = {:?}] mod diagram;{generated}",
                dir.join("mod.rs"),
                dir.join("diagram.rs")
            ),
        )
        .unwrap();
    }
}
