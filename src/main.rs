//! PROTOTYPE: deterministic motion-graphics scenes rendered headlessly with wgpu.

mod scenes;

use std::{fs, path::PathBuf};

use anyhow::{Context, Result, bail};

enum RenderScene {
    Hero,
    EffectShowsErrors,
    PromisesOnlyHappyPath,
    EffectIsADescription,
    VisualEffects,
    OpencodeCommandHotReload,
}

impl RenderScene {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "hero" => Some(Self::Hero),
            "effect-shows-errors" => Some(Self::EffectShowsErrors),
            "promises-only-happy-path" => Some(Self::PromisesOnlyHappyPath),
            "effect-is-a-description" => Some(Self::EffectIsADescription),
            "visual-effects" => Some(Self::VisualEffects),
            "opencode-command-hot-reload" => Some(Self::OpencodeCommandHotReload),
            _ => None,
        }
    }

    fn default_output(&self) -> &'static str {
        match self {
            Self::Hero => "output/kinograph-prototype.mp4",
            Self::EffectShowsErrors => "output/effect-shows-errors.mp4",
            Self::PromisesOnlyHappyPath => "output/promises-only-happy-path.mp4",
            Self::EffectIsADescription => "output/effect-is-a-description.mp4",
            Self::VisualEffects => "output/visual-effects.mp4",
            Self::OpencodeCommandHotReload => "output/opencode-command-hot-reload.mp4",
        }
    }
}

fn main() -> Result<()> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let (scene, explicit_output) = match arguments.as_slice() {
        [] => (RenderScene::Hero, None),
        [output] if output != "render" => (RenderScene::Hero, Some(output.as_str())),
        [command, scene] if command == "render" => (
            RenderScene::parse(scene).with_context(|| format!("unknown scene '{scene}'"))?,
            None,
        ),
        [command, scene, output] if command == "render" => (
            RenderScene::parse(scene).with_context(|| format!("unknown scene '{scene}'"))?,
            Some(output.as_str()),
        ),
        _ => bail!(
            "usage: kinograph [output] | kinograph render \
             <hero|effect-shows-errors|promises-only-happy-path|effect-is-a-description|visual-effects|opencode-command-hot-reload> [output]"
        ),
    };
    let output = PathBuf::from(explicit_output.unwrap_or_else(|| scene.default_output()));

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output directory {}", parent.display()))?;
    }

    match scene {
        RenderScene::Hero => pollster::block_on(scenes::hero::render(&output)),
        RenderScene::EffectShowsErrors => {
            pollster::block_on(scenes::effect_shows_errors::render(&output))
        }
        RenderScene::PromisesOnlyHappyPath => {
            pollster::block_on(scenes::promises_only_happy_path::render(&output))
        }
        RenderScene::EffectIsADescription => {
            pollster::block_on(scenes::effect_is_a_description::render(&output))
        }
        RenderScene::VisualEffects => pollster::block_on(scenes::visual_effects::render(&output)),
        RenderScene::OpencodeCommandHotReload => {
            pollster::block_on(scenes::opencode_hot_reload::render(&output))
        }
    }
}
