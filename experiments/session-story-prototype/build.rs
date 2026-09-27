use std::{env, fs, path::PathBuf};

fn main() {
    // Preserve the private module's child-path resolution without a source fork.
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/kinograph-render/src/render");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let ui = out.join("ui");
    fs::create_dir_all(&ui).unwrap();
    for (source, destination) in [("ui.rs", "mod.rs"), ("ui/card.rs", "card.rs")] {
        let source = root.join(source);
        println!("cargo:rerun-if-changed={}", source.display());
        fs::copy(source, ui.join(destination)).unwrap();
    }
    fs::write(
        out.join("ui-module.rs"),
        format!(
            "#[allow(dead_code)] #[path = {:?}] pub(crate) mod ui;",
            ui.join("mod.rs")
        ),
    )
    .unwrap();
}
