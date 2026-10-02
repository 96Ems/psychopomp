//! PROTOTYPE: deterministic motion-graphics scenes rendered headlessly with wgpu.

mod encode;
mod plan_runtime;
mod render;
mod scenes;
mod video;

use std::{fs, path::PathBuf};

use anyhow::{Context, Result, bail};

enum RenderScene<'a> {
    Hero,
    EffectShowsErrors,
    PromisesOnlyHappyPath,
    EffectIsADescription,
    VisualEffects,
    OpencodeCommandHotReload,
    /// A published Effect Institute chapter, or one of its sections.
    Chapter {
        chapter: &'a str,
        section: Option<&'a str>,
    },
}

impl<'a> RenderScene<'a> {
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

    fn default_output(&self) -> String {
        match self {
            Self::Hero => "output/kinograph-prototype.mp4".into(),
            Self::EffectShowsErrors => "output/effect-shows-errors.mp4".into(),
            Self::PromisesOnlyHappyPath => "output/promises-only-happy-path.mp4".into(),
            Self::EffectIsADescription => "output/effect-is-a-description.mp4".into(),
            Self::VisualEffects => "output/visual-effects.mp4".into(),
            Self::OpencodeCommandHotReload => "output/opencode-command-hot-reload.mp4".into(),
            Self::Chapter {
                section: Some(_), ..
            } => "output/effect-institute-section.mp4".into(),
            Self::Chapter {
                chapter,
                section: None,
            } => format!("output/chapters/{chapter}.mp4"),
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
    let (scene, explicit_output) = match arguments.as_slice() {
        [] => (RenderScene::Hero, None),
        [output] if output != "render" => (RenderScene::Hero, Some(output)),
        [command, kind, chapter, rest @ ..]
            if command == "render" && kind == "chapter" && rest.len() <= 1 =>
        {
            if !matches!(chapter.as_str(), "intro" | "basics") {
                bail!("unknown chapter '{chapter}'");
            }
            (
                RenderScene::Chapter {
                    chapter,
                    section: None,
                },
                rest.first(),
            )
        }
        [command, kind, chapter, section, rest @ ..]
            if command == "render" && kind == "section" && rest.len() <= 1 =>
        {
            (
                RenderScene::Chapter {
                    chapter,
                    section: Some(section),
                },
                rest.first(),
            )
        }
        [command, scene, rest @ ..] if command == "render" && rest.len() <= 1 => (
            RenderScene::parse(scene).with_context(|| format!("unknown scene '{scene}'"))?,
            rest.first(),
        ),
        _ => bail!(
            "usage: kinograph [output] | kinograph render \
             <hero|effect-shows-errors|promises-only-happy-path|effect-is-a-description|visual-effects|opencode-command-hot-reload> [output] | \
             kinograph render chapter <intro|basics> [output] | \
             kinograph render section <intro|basics> <section> [output]"
        ),
    };
    let output = PathBuf::from(
        explicit_output
            .cloned()
            .unwrap_or_else(|| scene.default_output()),
    );

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
        RenderScene::Chapter { chapter, section } => {
            pollster::block_on(scenes::effect_institute::render(chapter, section, &output))
        }
        RenderScene::VisualEffects => pollster::block_on(scenes::visual_effects::render(&output)),
        RenderScene::OpencodeCommandHotReload => {
            pollster::block_on(scenes::opencode_hot_reload::render(&output))
        }
    }
}
