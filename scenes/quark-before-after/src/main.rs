use std::{env, fs, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plan = kinograph_quark_before_after::build_plan()?;
    let json = plan.to_json_pretty()?;
    if let Some(path) = env::args().nth(1) {
        if let Some(parent) = Path::new(&path).parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, json)?;
    } else {
        println!("{json}");
    }
    Ok(())
}
