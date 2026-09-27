//! Native build step only. No native fonts or rendering server needed at runtime.
use anyhow::{Context, Result};
use kinograph::playback::PlaybackCommand;
use kinograph_browser_grid_prototype::{
    plan_runtime::Scene,
    render::{HEIGHT, HeadlessRenderer, Theme, WIDTH},
};
use std::{collections::HashMap, path::PathBuf, time::Duration};
mod proof;

fn main() -> Result<()> {
    let output = PathBuf::from(std::env::args().nth(1).expect("bake <output-directory>"));
    std::fs::create_dir_all(&output)?;
    let mut renderer = pollster::block_on(HeadlessRenderer::new())?;
    let mut scenes = HashMap::new();
    for slide in kinograph_keyed_grid::build_deck()?.slides.into_iter()
        .chain(kinograph_opencode_architecture::build_deck()?.slides) {
        let id = slide.plan.id.clone();
        std::fs::write(output.join(format!("{id}.json")),serde_json::to_vec(&slide.plan)?)?;
        scenes.insert(id,Scene::new(slide.plan,&mut renderer)?);
    }
    let cases = proof::cases();
    for case in &cases {
        let scene = scenes.get_mut(case.scene).context("declared proof scene")?;
        renderer.set_theme(Theme::parse(case.theme)?);
        let pixels = match case.sample {
            proof::Sample::Authored {seconds} => scene.render(&mut renderer,seconds,&scene.timeline)?,
            proof::Sample::Interactive {millis,action} => {
                let now = Duration::from_millis(millis);
                let command = match action {
                    "next"=>Some(PlaybackCommand::Next),"previous"=>Some(PlaybackCommand::Previous),
                    "first"=>Some(PlaybackCommand::First),"last"=>Some(PlaybackCommand::Last),
                    ""=>None,_=>anyhow::bail!("unknown proof command {action}"),
                };
                if let Some(command)=command {scene.playback.command(command,now);}
                let sample=scene.playback.sample(now);
                scene.render(&mut renderer,sample.at_nanos as f64/1e9,&scene.playback.timeline())?
            }
        };
        let file=std::fs::File::create(output.join(&case.native))?;
        let mut encoder=png::Encoder::new(std::io::BufWriter::new(file),WIDTH,HEIGHT);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
    }
    renderer.bake_glyphs(&output.join("labels.glyphs"))?;
    std::fs::write(output.join("interruptions.json"),serde_json::to_vec(&proof::INTERRUPTIONS)?)?;
    std::fs::write(output.join("proof-cases.json"),serde_json::to_vec_pretty(&serde_json::json!({"version":1,"size":[WIDTH,HEIGHT],"cases":cases}))?)?;
    println!("Baked glyphs, case inventory and native references into {}",output.display());
    Ok(())
}
