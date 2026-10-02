use std::{collections::HashMap, path::Path};

use anyhow::Result;
use serde::Deserialize;

use kinograph::{
    composition::{Asset, Clip, Composition, Time, TimeRange},
    dsl::{Scalar, Scene, Task},
    timeline::PropertyId,
    transcript::Transcript,
};

use crate::{
    plan_runtime::new_renderer,
    render::{QuoteFrame, TaskLinkFrame, TaskSceneFrame},
};

use super::{HEIGHT, WIDTH, WORKSPACE_ROOT, encode_scene, plan_temporal_samples};

pub(crate) async fn render(output: &Path) -> Result<()> {
    let asset_directory = Path::new(WORKSPACE_ROOT)
        .join("assets")
        .join("visual-effects");
    let transcript = Transcript::load(&asset_directory.join("timings.json"))?;
    let narration = Asset::audio("visual-effects", asset_directory.join("narration.webm"))
        .clip(TimeRange::new(Time::ZERO, Time::seconds(48.627)));
    let running_sound = Asset::audio("task-running", asset_directory.join("task-running.wav"))
        .clip(TimeRange::new(Time::ZERO, Time::seconds(0.14)))
        .gain_db(18.0);
    let success_sound = Asset::audio("task-success", asset_directory.join("task-success.wav"))
        .clip(TimeRange::new(Time::ZERO, Time::seconds(0.42)))
        .gain_db(12.0);
    let failure_sound = Asset::audio("task-failure", asset_directory.join("task-failure.wav"))
        .clip(TimeRange::new(Time::ZERO, Time::seconds(0.48)))
        .gain_db(16.0);
    let death_sound = Asset::audio("task-death", asset_directory.join("task-death.wav"))
        .clip(TimeRange::new(Time::ZERO, Time::seconds(1.1)))
        .gain_db(16.0);
    let cues = VisualEffectsCues::from_transcript(&transcript)?;
    let center = WIDTH as f32 * 0.5;
    let y = HEIGHT as f32 * 0.5;
    let mut renderer = new_renderer("effect-simulacra").await?;
    let typescript_width = renderer.measure_task_result_width("TypeScript");
    let homicide_width = renderer.measure_task_result_width("HOMICIDE");
    let detective_width = renderer.measure_task_result_width("JR. DETECTIVE");
    let two_nodes = centered_task_row(center, &[typescript_width, 128.0], 24.0);
    let three_nodes = centered_task_row(center, &[typescript_width, 128.0, 128.0], 24.0);
    let idle_row = centered_task_row(center, &[128.0, 128.0, 128.0], 24.0);
    let classify_done = centered_task_row(center, &[homicide_width, 128.0, 128.0], 24.0);
    let assign_done = centered_task_row(center, &[homicide_width, detective_width, 128.0], 24.0);
    let lang = Task::new("lang", "lang")
        .at(center, y)
        .with_result_width(typescript_width);
    let launch = Task::new("launch", "launch").at(two_nodes[1], y);
    let pact = Task::new("pact", "pact").at(three_nodes[2], y);
    let classify = Task::new("classify", "classify")
        .at(center, y)
        .with_result_width(homicide_width);
    let assign = Task::new("assign", "assign")
        .at(idle_row[1], y)
        .with_result_width(detective_width);
    let notify = Task::new("notify", "notify").at(idle_row[2], y);
    let at = |seconds: f32, change| {
        Composition::delay(
            kinograph::composition::Duration::seconds(f64::from(seconds)),
            change,
        )
    };
    let pose_at = |seconds: f32, change| {
        Composition::delay(
            kinograph::composition::Duration::seconds(f64::from(seconds)),
            Composition::task_pose(change),
        )
    };
    let sound_at = |seconds: f32, clip: Clip| {
        Composition::delay(
            kinograph::composition::Duration::seconds(f64::from(seconds)),
            Composition::layer(clip),
        )
    };
    let composition = Composition::parallel([
        Composition::script(narration),
        at(cues.demo, lang.idle()),
        at(cues.lang_running, lang.run()),
        at(cues.lang_completed, lang.succeed("TypeScript")),
        pose_at(cues.launch_visible, lang.move_to(two_nodes[0], y)),
        at(cues.launch_visible, launch.idle()),
        at(cues.launch_running, launch.run()),
        at(cues.launch_failed, launch.fail("NoFuel")),
        pose_at(cues.pact_visible, lang.move_to(three_nodes[0], y)),
        pose_at(cues.pact_visible, launch.move_to(three_nodes[1], y)),
        at(cues.pact_visible, pact.idle()),
        at(cues.pact_running, pact.run()),
        at(cues.pact_death, pact.die("")),
        at(cues.classify_visible, lang.hide()),
        at(cues.classify_visible, launch.hide()),
        at(cues.classify_visible, pact.hide()),
        at(cues.classify_visible + 0.18, classify.idle()),
        pose_at(cues.combined, classify.move_to(idle_row[0], y)),
        at(cues.combined, assign.idle()),
        at(cues.combined, notify.idle()),
        at(cues.classify_running, classify.run()),
        at(cues.assign_running, classify.succeed("HOMICIDE")),
        pose_at(cues.assign_running, assign.move_to(classify_done[1], y)),
        at(cues.assign_running, assign.run()),
        pose_at(cues.assign_running, notify.move_to(classify_done[2], y)),
        pose_at(cues.notify_running, classify.move_to(assign_done[0], y)),
        at(cues.notify_running, assign.succeed("JR. DETECTIVE")),
        pose_at(cues.notify_running, notify.move_to(assign_done[2], y)),
        at(cues.notify_running, notify.run()),
        at(cues.notify_failed, notify.fail("RateLimitError")),
        at(cues.notify_retry, notify.run()),
        at(cues.notify_completed, notify.complete()),
        sound_at(cues.lang_running, running_sound.clone()),
        sound_at(cues.launch_running, running_sound.clone()),
        sound_at(cues.pact_running, running_sound.clone()),
        sound_at(cues.classify_running, running_sound.clone()),
        sound_at(cues.assign_running, running_sound.clone()),
        sound_at(cues.notify_running, running_sound.clone()),
        sound_at(cues.notify_retry, running_sound),
        sound_at(cues.lang_completed, success_sound.clone()),
        sound_at(cues.assign_running, success_sound.clone()),
        sound_at(cues.notify_running, success_sound.clone()),
        sound_at(cues.notify_completed, success_sound),
        sound_at(cues.launch_failed, failure_sound.clone()),
        sound_at(cues.notify_failed, failure_sound),
        sound_at(cues.pact_death, death_sound),
    ]);
    let scene =
        Scene::new(Vec::<(PropertyId, Scalar)>::new(), composition).compile(&HashMap::new())?;
    encode_scene(
        &mut renderer,
        output,
        &scene,
        plan_temporal_samples,
        |renderer, time| {
            let quote = visual_effects_quote(time, &cues);
            let nodes = scene.task_frames_at(time);
            let links = visual_effects_task_links(time, &cues, &nodes);
            renderer.render_task_scene(&TaskSceneFrame {
                quote,
                links: &links,
                nodes: &nodes,
            })
        },
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VisualEffectsCues {
    quote_words: [f32; 8],
    hide_quote: f32,
    subliminal: f32,
    demo: f32,
    lang_running: f32,
    lang_completed: f32,
    launch_visible: f32,
    launch_running: f32,
    launch_failed: f32,
    pact_visible: f32,
    pact_running: f32,
    pact_death: f32,
    classify_visible: f32,
    combined: f32,
    classify_running: f32,
    assign_running: f32,
    notify_running: f32,
    notify_failed: f32,
    notify_retry: f32,
    notify_completed: f32,
}

impl VisualEffectsCues {
    fn from_transcript(transcript: &Transcript) -> Result<Self> {
        let start = |word: &str, occurrence| {
            Ok::<_, anyhow::Error>(
                transcript
                    .word_occurrence(word, occurrence)?
                    .start()
                    .as_seconds() as f32
                    - 0.15,
            )
        };
        Ok(Self {
            quote_words: [
                start("\"why", 0)?,
                start("would", 0)?,
                start("I", 1)?,
                start("ever", 0)?,
                start("want", 0)?,
                start("to", 1)?,
                start("use", 0)?,
                start("Effect?\"", 0)?,
            ],
            hide_quote: start("I'd", 0)?,
            subliminal: start("tastefully", 0)?,
            demo: start("This", 0)?,
            lang_running: start("Effect,", 0)?,
            lang_completed: start("succeed,", 0)?,
            launch_visible: start("or", 0)?,
            launch_running: start("they", 0)?,
            launch_failed: start("fail,", 0)?,
            pact_visible: start("They'll", 0)?,
            pact_running: start("even", 0)?,
            pact_death: start("die", 0)?,
            classify_visible: start("You", 0)?,
            combined: start("combine", 0)?,
            classify_running: start("running", 0)?,
            assign_running: start("after", 0)?,
            notify_running: start("another.", 0)?,
            notify_failed: start("fails,", 0)?,
            notify_retry: start("retry", 0)?,
            notify_completed: start("schedule.", 0)?,
        })
    }
}

fn visual_effects_quote(time: f32, cues: &VisualEffectsCues) -> Option<QuoteFrame<'static>> {
    const SUBLIMINAL: [&str; 8] = [
        "EFFECT", "IS", "THE", "GREATEST", "LIBRARY", "OF", "ALL", "TIME",
    ];
    if time >= cues.demo {
        return None;
    }
    let highlight = cues.quote_words.iter().rposition(|cue| time >= *cue);
    let subliminal_index = ((time - cues.subliminal) / 0.1).floor() as isize;
    let subliminal = (0..SUBLIMINAL.len() as isize)
        .contains(&subliminal_index)
        .then(|| SUBLIMINAL[subliminal_index as usize]);
    Some(QuoteFrame {
        time,
        highlight,
        show_hypnotic: time >= cues.quote_words[0],
        hide_quote: time >= cues.hide_quote,
        subliminal,
    })
}

fn centered_task_row(center: f32, widths: &[f32], gap: f32) -> Vec<f32> {
    let total_width = widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32;
    let mut cursor = center - total_width * 0.5;
    widths
        .iter()
        .map(|width| {
            let position = cursor + width * 0.5;
            cursor += width + gap;
            position
        })
        .collect()
}

fn visual_effects_task_links(
    time: f32,
    cues: &VisualEffectsCues,
    nodes: &[kinograph::dsl::TaskFrame<'_>],
) -> Vec<TaskLinkFrame> {
    if time < cues.combined {
        return Vec::new();
    }
    let node = |id: &str| {
        nodes
            .iter()
            .find(|node| node.id.as_str() == id)
            .map(|node| [node.x, node.y])
    };
    let (Some(classify), Some(assign), Some(notify)) =
        (node("classify"), node("assign"), node("notify"))
    else {
        return Vec::new();
    };
    let handoff = |arrival: f32| {
        let progress = (time - (arrival - 0.38)) / 0.38;
        (0.0..=1.0).contains(&progress).then_some(progress)
    };
    vec![
        TaskLinkFrame {
            from: classify,
            to: assign,
            pulse: handoff(cues.assign_running),
        },
        TaskLinkFrame {
            from: assign,
            to: notify,
            pulse: handoff(cues.notify_running),
        },
    ]
}
