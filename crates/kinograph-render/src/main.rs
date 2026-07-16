//! PROTOTYPE: deterministic motion-graphics scenes rendered headlessly with wgpu.

mod encode;
mod plan_runtime;
mod render;
mod scenes;
mod video;

use std::{fs, path::PathBuf};

use anyhow::{Context, Result, bail};

enum RenderScene {
    Hero,
    EffectShowsErrors,
    PromisesOnlyHappyPath,
    EffectIsADescription,
    IntroChapter,
    BasicsChapter,
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
            Self::IntroChapter => "output/chapters/intro.mp4",
            Self::BasicsChapter => "output/chapters/basics.mp4",
            Self::VisualEffects => "output/visual-effects.mp4",
            Self::OpencodeCommandHotReload => "output/opencode-command-hot-reload.mp4",
        }
    }
}

fn main() -> Result<()> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if let [command, rest @ ..] = arguments.as_slice()
        && command == "plan"
    {
        return plan_runtime::command(rest);
    }
    if let [command, kind, chapter, section, rest @ ..] = arguments.as_slice()
        && command == "render"
        && kind == "section"
        && rest.len() <= 1
    {
        let output = PathBuf::from(
            rest.first()
                .map(String::as_str)
                .unwrap_or("output/effect-institute-section.mp4"),
        );
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create output directory {}", parent.display()))?;
        }
        return pollster::block_on(scenes::effect_institute::render_section(
            chapter, section, &output,
        ));
    }
    let (scene, explicit_output) = match arguments.as_slice() {
        [] => (RenderScene::Hero, None),
        [output] if output != "render" => (RenderScene::Hero, Some(output.as_str())),
        [command, scene] if command == "render" => (
            RenderScene::parse(scene).with_context(|| format!("unknown scene '{scene}'"))?,
            None,
        ),
        [command, kind, chapter] if command == "render" && kind == "chapter" => (
            match chapter.as_str() {
                "intro" => RenderScene::IntroChapter,
                "basics" => RenderScene::BasicsChapter,
                _ => bail!("unknown chapter '{chapter}'"),
            },
            None,
        ),
        [command, kind, chapter, output] if command == "render" && kind == "chapter" => (
            match chapter.as_str() {
                "intro" => RenderScene::IntroChapter,
                "basics" => RenderScene::BasicsChapter,
                _ => bail!("unknown chapter '{chapter}'"),
            },
            Some(output.as_str()),
        ),
        [command, scene, output] if command == "render" => (
            RenderScene::parse(scene).with_context(|| format!("unknown scene '{scene}'"))?,
            Some(output.as_str()),
        ),
        _ => bail!(
            "usage: kinograph [output] | kinograph render \
             <hero|effect-shows-errors|promises-only-happy-path|effect-is-a-description|visual-effects|opencode-command-hot-reload> [output] | \
             kinograph render chapter <intro|basics> [output] | \
             kinograph render section <intro|basics> <section> [output]"
        ),
    };
    let output = PathBuf::from(explicit_output.unwrap_or_else(|| scene.default_output()));

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output directory {}", parent.display()))?;
    }

    match scene {
        RenderScene::Hero => pollster::block_on(plan_runtime::render_builtin_hero(&output)),
        RenderScene::EffectShowsErrors => {
            pollster::block_on(scenes::effect_shows_errors::render(&output))
        }
        RenderScene::PromisesOnlyHappyPath => {
            pollster::block_on(scenes::promises_only_happy_path::render(&output))
        }
        RenderScene::EffectIsADescription => {
            pollster::block_on(scenes::effect_is_a_description::render(&output))
        }
        RenderScene::IntroChapter => {
            pollster::block_on(scenes::effect_institute::render("intro", &output))
        }
        RenderScene::BasicsChapter => {
            pollster::block_on(scenes::effect_institute::render("basics", &output))
        }
        RenderScene::VisualEffects => pollster::block_on(scenes::visual_effects::render(&output)),
        RenderScene::OpencodeCommandHotReload => {
            pollster::block_on(scenes::opencode_hot_reload::render(&output))
        }
    }
}
