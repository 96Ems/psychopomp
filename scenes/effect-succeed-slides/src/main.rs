use std::{env, fs, path::Path};

fn main() -> anyhow::Result<()> {
    let plan = kinograph_effect_succeed_slides::build_plan()?;
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
