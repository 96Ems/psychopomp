use std::{collections::HashMap, path::Path};

use anyhow::Result;

use kinograph::{
    code::{
        CodeDocument, CodeLayout, CodeLine, CodeSnapshot, CodeTransition, SyntaxStyle,
        TransitionProgress,
    },
    composition::{Asset, Clip, Composition, Time, TimeRange},
    dsl::{Code, CodeEdit, CompiledScene, Motion, Pointer, Scalar, Scene, Task},
    timeline::{PropertyId, SpringProfile},
    transcript::Transcript,
};

use crate::render::{
    EditorFrame, HeadlessRenderer, InlineRevealFrame, RenderSpec, TaskSceneFrame, TokenHighlight,
};

use super::{
    CodeTarget, HEIGHT, WIDTH, WORKSPACE_ROOT, encode_video_with_samples, measure_target,
    measure_text_width, sample_pointer_frame, span,
};

const DESCRIPTION_AUDIO_DURATION: f64 = 30.366;

pub(crate) async fn render(output: &Path) -> Result<()> {
    let asset_directory = Path::new(WORKSPACE_ROOT)
        .join("assets")
        .join("effect-is-a-description");
    let transcript = Transcript::load(&asset_directory.join("timings.json"))?;
    let transitions = effect_is_a_description_transitions()?;
    let mut renderer = HeadlessRenderer::new(RenderSpec {
        width: WIDTH,
        height: HEIGHT,
        file_name: "effect.ts".to_owned(),
    })
    .await?;
    let initial = transitions.effect_run.sample(TransitionProgress {
        layout: 0.0,
        content: 0.0,
    });
    let effect_run = transitions.effect_run.sample(TransitionProgress {
        layout: 1.0,
        content: 1.0,
    });
    let function = transitions.function.sample(TransitionProgress {
        layout: 1.0,
        content: 1.0,
    });
    let function_comment = transitions.function_comment.sample(TransitionProgress {
        layout: 1.0,
        content: 1.0,
    });
    let hidden_definition_width = measure_text_width(
        &mut renderer,
        ": Effect.Effect<number>Clock.currentTimeMillis",
    )?;
    let hidden_run_prefix_width = measure_text_width(&mut renderer, "Effect.runSync(")?;
    let function_call_suffix_width = measure_text_width(&mut renderer, "()")?;
    let hidden_effect_subject_width = measure_text_width(&mut renderer, "Effects")?;
    let mut function_impl =
        measure_target(&mut renderer, &function, "definition", "() => Date.now()")?;
    function_impl.bounds.x -= hidden_definition_width;
    let mut function_call = measure_target(&mut renderer, &function, "run", "getTime")?;
    function_call.bounds.x -= hidden_run_prefix_width;
    function_call.bounds.width += function_call_suffix_width;
    let mut function_lazy = measure_target(&mut renderer, &function_comment, "comment", "LAZY")?;
    function_lazy.bounds.x -= hidden_effect_subject_width;
    let mut run_effect =
        measure_target(&mut renderer, &effect_run, "run", "Effect.runSync(getTime")?;
    run_effect.bounds.width += measure_text_width(&mut renderer, ")")?;
    let targets = DescriptionTargets {
        effect_type: measure_target(
            &mut renderer,
            &initial,
            "definition",
            "Effect.Effect<number>",
        )?,
        run_sync: measure_target(&mut renderer, &effect_run, "run", "Effect.runSync")?,
        run_effect,
        function_impl,
        function_call,
        function_lazy,
    };
    let narration = Asset::audio(
        "effect-is-a-description",
        asset_directory.join("narration.webm"),
    )
    .clip(TimeRange::new(
        Time::ZERO,
        Time::seconds(DESCRIPTION_AUDIO_DURATION),
    ));
    let task_asset_directory = Path::new(WORKSPACE_ROOT)
        .join("assets")
        .join("visual-effects");
    let running_sound = Asset::audio(
        "description-task-running",
        task_asset_directory.join("task-running.wav"),
    )
    .clip(TimeRange::new(Time::ZERO, Time::seconds(0.14)))
    .gain_db(18.0);
    let success_sound = Asset::audio(
        "description-task-success",
        task_asset_directory.join("task-success.wav"),
    )
    .clip(TimeRange::new(Time::ZERO, Time::seconds(0.42)))
    .gain_db(12.0);
    let reset_sound = Asset::audio(
        "description-task-reset",
        task_asset_directory.join("task-reset.wav"),
    )
    .clip(TimeRange::new(Time::ZERO, Time::seconds(0.34)))
    .gain_db(12.0);
    let timestamp_width = renderer.measure_task_result_width("1736078400000");
    let choreography = effect_is_a_description_choreography(
        &transcript,
        targets,
        timestamp_width,
        narration,
        running_sound,
        success_sound,
        reset_sound,
    )?;

    encode_video_with_samples(
        &mut renderer,
        output,
        &choreography.scene,
        4,
        8,
        &[
            0.0..2.2,
            5.8..9.5,
            13.2..14.9,
            17.8..20.1,
            21.6..23.3,
            27.3..29.1,
        ],
        |renderer, time| {
            render_effect_is_a_description_sample(renderer, &transitions, &choreography, time)
        },
    )
}

#[derive(Clone, Copy)]
struct DescriptionTargets {
    effect_type: CodeTarget,
    run_sync: CodeTarget,
    run_effect: CodeTarget,
    function_impl: CodeTarget,
    function_call: CodeTarget,
    function_lazy: CodeTarget,
}

struct EffectIsADescriptionChoreography {
    scene: CompiledScene,
    panel_y: PropertyId,
    panel_rotation: PropertyId,
    panel_tilt_x: PropertyId,
    panel_tilt_y: PropertyId,
    panel_scale: PropertyId,
    panel_near_blur: PropertyId,
    effect_run: CodeEdit,
    function: CodeEdit,
    function_comment: CodeEdit,
    type_annotation: PropertyId,
    effect_impl: PropertyId,
    function_impl: PropertyId,
    run_prefix: PropertyId,
    run_middle: PropertyId,
    function_subject: PropertyId,
    effect_subject: PropertyId,
    focus: PropertyId,
    focus_y: PropertyId,
    token_x: PropertyId,
    token_y: PropertyId,
    token_width: PropertyId,
    token_opacity: PropertyId,
    pointer: Pointer,
}

fn effect_is_a_description_choreography(
    transcript: &Transcript,
    measured: DescriptionTargets,
    timestamp_width: f32,
    narration: Clip,
    running_sound: Clip,
    success_sound: Clip,
    reset_sound: Clip,
) -> Result<EffectIsADescriptionChoreography> {
    let code = Code::new("description.code");
    let pointer = Pointer::new("description.pointer");
    let effect_run_edit = code.edit("effect-run");
    let function_edit = code.edit("function");
    let function_comment_edit = code.edit("function-comment");
    let type_annotation = PropertyId::new("description.code.type_annotation");
    let effect_impl = PropertyId::new("description.code.effect_impl");
    let function_impl_part = PropertyId::new("description.code.function_impl");
    let run_prefix = PropertyId::new("description.code.run_prefix");
    let run_middle = PropertyId::new("description.code.run_middle");
    let function_subject = PropertyId::new("description.code.function_subject");
    let effect_subject = PropertyId::new("description.code.effect_subject");
    let spotlight = SpringProfile::from_visual_duration(0.3, 0.0, 0.001, 0.001);
    let line_motion = SpringProfile::from_visual_duration(0.45, 0.0, 0.001, 0.001);
    let variable_part = SpringProfile::from_visual_duration(0.4, 0.0, 0.001, 0.001);
    let entrance = SpringProfile::from_visual_duration(0.72, 0.2, 0.001, 0.001);
    let pointer_motion = SpringProfile::from_visual_duration(0.5, 0.3, 0.001, 0.001);
    let pointer_scale = 24.0 / 36.0;

    let effect_type = code.text("definition", "Effect.Effect<number>");
    let run_sync = code.text("run", "Effect.runSync");
    let run_effect = code.text("run", "Effect.runSync(getTime)");
    let function_impl = code.text("definition", "() => Date.now()");
    let function_call = code.text("run", "getTime()");
    let function_lazy = code.text("comment", "LAZY");
    let targets = HashMap::from([
        (effect_type.clone(), measured.effect_type.into()),
        (run_sync.clone(), measured.run_sync.into()),
        (run_effect.clone(), measured.run_effect.into()),
        (function_impl.clone(), measured.function_impl.into()),
        (function_call.clone(), measured.function_call.into()),
        (function_lazy.clone(), measured.function_lazy.into()),
    ]);
    let get_time = Task::new("get-time", "getTime")
        .at(WIDTH as f32 * 0.5, 742.0)
        .with_result_width(timestamp_width);
    let focus_cursor_at = |target: kinograph::dsl::TextTarget, cue: kinograph::composition::Cue| {
        cue.at(Motion::parallel([
            code.focus(target.clone(), spotlight),
            code.highlight(target.clone(), spotlight),
            Motion::spring(pointer.opacity.clone(), 1.0, pointer_motion),
            pointer.move_to(target, 55.0, pointer_motion),
        ]))
    };
    let blueprint = transcript.word("blueprint,")?;
    let to = transcript.word("To")?;
    let run = transcript.word("run")?;
    let explicitly = transcript.word("explicitly.")?;
    let function = transcript.word("function.")?;
    let called = transcript.word("called.")?;
    let laziness = transcript.word("laziness")?;
    let r#in = transcript.word("In")?;
    let composition = Composition::parallel([
        Composition::script(narration),
        Composition::delay(
            kinograph::composition::Duration::milliseconds(80.0),
            Composition::animate(Motion::parallel([
                Motion::spring(code.panel_y.clone(), 0.0, entrance),
                Motion::spring(code.panel_rotation.clone(), 0.0, entrance),
                Motion::spring(code.panel_tilt_x.clone(), 0.0, entrance),
                Motion::spring(code.panel_tilt_y.clone(), 0.0, entrance),
                Motion::spring(code.panel_scale.clone(), 1.0, entrance),
                Motion::spring(code.panel_near_blur.clone(), 0.0, entrance),
            ])),
        ),
        Composition::delay(
            kinograph::composition::Duration::milliseconds(320.0),
            Composition::task(get_time.idle()),
        ),
        blueprint.at(Motion::parallel([
            Motion::spring(type_annotation.clone(), 1.0, variable_part),
            code.highlight(effect_type.clone(), spotlight),
            Motion::spring(pointer.opacity.clone(), 1.0, pointer_motion),
            pointer.move_to(effect_type, 55.0, pointer_motion),
        ])),
        to.at(Motion::parallel([
            effect_run_edit.enter(line_motion),
            code.focus(run_sync.clone(), spotlight),
            code.highlight(run_sync.clone(), spotlight),
            pointer.move_to(run_sync.clone(), 55.0, pointer_motion),
        ])),
        focus_cursor_at(run_effect, run.clone()),
        run.at(get_time.run()),
        run.at(Composition::layer(running_sound)),
        explicitly.at(get_time.succeed("1736078400000")),
        explicitly.at(Composition::layer(success_sound)),
        function.at(Motion::parallel([
            function_edit.enter(line_motion),
            Motion::spring(type_annotation.clone(), 0.0, variable_part),
            Motion::spring(effect_impl.clone(), 0.0, variable_part),
            Motion::spring(function_impl_part.clone(), 1.0, variable_part),
            Motion::spring(run_prefix.clone(), 0.0, variable_part),
            Motion::spring(run_middle.clone(), 1.0, variable_part),
            code.focus(function_impl.clone(), spotlight),
            code.highlight(function_impl.clone(), spotlight),
            pointer.move_to(function_impl, 55.0, pointer_motion),
        ])),
        function.at(get_time.idle()),
        function.at(Composition::layer(reset_sound)),
        focus_cursor_at(function_call, called),
        laziness.at(Motion::parallel([
            function_comment_edit.enter(line_motion),
            code.focus(function_lazy.clone(), spotlight),
            code.highlight(function_lazy.clone(), spotlight),
            pointer.move_to(function_lazy, 55.0, pointer_motion),
        ])),
        r#in.at(Motion::parallel([
            Motion::spring(type_annotation.clone(), 1.0, variable_part),
            Motion::spring(effect_impl.clone(), 1.0, variable_part),
            Motion::spring(function_impl_part.clone(), 0.0, variable_part),
            Motion::spring(run_prefix.clone(), 1.0, variable_part),
            Motion::spring(run_middle.clone(), 0.0, variable_part),
            Motion::spring(function_subject.clone(), 0.0, variable_part),
            Motion::spring(effect_subject.clone(), 1.0, variable_part),
            Motion::spring(code.focus.clone(), 0.0, spotlight),
            Motion::spring(code.highlight_opacity.clone(), 0.0, spotlight),
            Motion::spring(pointer.opacity.clone(), 0.0, pointer_motion),
        ])),
    ]);
    let initial_values = [
        (code.panel_y.clone(), Scalar::Literal(280.0)),
        (code.panel_rotation.clone(), Scalar::Literal(-0.12)),
        (code.panel_tilt_x.clone(), Scalar::Literal(-0.28)),
        (code.panel_tilt_y.clone(), Scalar::Literal(0.34)),
        (code.panel_scale.clone(), Scalar::Literal(1.46)),
        (code.panel_near_blur.clone(), Scalar::Literal(4.0)),
        (type_annotation.clone(), Scalar::Literal(0.0)),
        (effect_impl.clone(), Scalar::Literal(1.0)),
        (function_impl_part.clone(), Scalar::Literal(0.0)),
        (run_prefix.clone(), Scalar::Literal(1.0)),
        (run_middle.clone(), Scalar::Literal(0.0)),
        (function_subject.clone(), Scalar::Literal(1.0)),
        (effect_subject.clone(), Scalar::Literal(0.0)),
        (code.focus.clone(), Scalar::Literal(0.0)),
        (code.focus_y.clone(), Scalar::TargetLineY(run_sync.clone())),
        (code.highlight_x.clone(), Scalar::TargetX(run_sync.clone())),
        (
            code.highlight_y.clone(),
            Scalar::TargetLineY(run_sync.clone()),
        ),
        (
            code.highlight_width.clone(),
            Scalar::TargetWidth(run_sync.clone()),
        ),
        (code.highlight_opacity.clone(), Scalar::Literal(0.0)),
        (
            pointer.x.clone(),
            Scalar::TargetCenterX(run_sync.clone()).offset(40.0),
        ),
        (
            pointer.y.clone(),
            Scalar::TargetBelow {
                target: run_sync,
                offset: 85.0,
            },
        ),
        (pointer.opacity.clone(), Scalar::Literal(0.0)),
        (pointer.scale.clone(), Scalar::Literal(pointer_scale)),
        (pointer.blur.clone(), Scalar::Literal(0.0)),
    ]
    .into_iter()
    .chain(effect_run_edit.initial_values())
    .chain(function_edit.initial_values())
    .chain(function_comment_edit.initial_values());
    let scene = Scene::new(initial_values, composition).compile(&targets)?;

    Ok(EffectIsADescriptionChoreography {
        scene,
        panel_y: code.panel_y,
        panel_rotation: code.panel_rotation,
        panel_tilt_x: code.panel_tilt_x,
        panel_tilt_y: code.panel_tilt_y,
        panel_scale: code.panel_scale,
        panel_near_blur: code.panel_near_blur,
        effect_run: effect_run_edit,
        function: function_edit,
        function_comment: function_comment_edit,
        type_annotation,
        effect_impl,
        function_impl: function_impl_part,
        run_prefix,
        run_middle,
        function_subject,
        effect_subject,
        focus: code.focus,
        focus_y: code.focus_y,
        token_x: code.highlight_x,
        token_y: code.highlight_y,
        token_width: code.highlight_width,
        token_opacity: code.highlight_opacity,
        pointer,
    })
}

fn render_effect_is_a_description_sample(
    renderer: &mut HeadlessRenderer,
    transitions: &DescriptionTransitions,
    choreography: &EffectIsADescriptionChoreography,
    time: f32,
) -> Result<Vec<u8>> {
    let timeline = choreography.scene.timeline();
    let sample = |property| {
        timeline
            .sample(property, time)
            .expect("description choreography property has an initial value")
            .position
    };
    let progress = |edit: &CodeEdit| {
        edit.progress_at(&choreography.scene, time)
            .expect("code edit properties have initial values")
    };
    let effect_run = progress(&choreography.effect_run);
    let function = progress(&choreography.function);
    let function_comment = progress(&choreography.function_comment);
    let lines = if function_comment.layout > 0.001 || function_comment.content > 0.001 {
        transitions.function_comment.sample(function_comment)
    } else if function.layout > 0.001 || function.content > 0.001 {
        transitions.function.sample(function)
    } else {
        transitions.effect_run.sample(effect_run)
    };
    let reveals = [
        InlineRevealFrame {
            line_id: "definition",
            start_span: 2,
            end_span: 7,
            progress: sample(&choreography.type_annotation),
        },
        InlineRevealFrame {
            line_id: "definition",
            start_span: 8,
            end_span: 9,
            progress: sample(&choreography.effect_impl),
        },
        InlineRevealFrame {
            line_id: "definition",
            start_span: 9,
            end_span: 12,
            progress: sample(&choreography.function_impl),
        },
        InlineRevealFrame {
            line_id: "run",
            start_span: 0,
            end_span: 2,
            progress: sample(&choreography.run_prefix),
        },
        InlineRevealFrame {
            line_id: "run",
            start_span: 3,
            end_span: 4,
            progress: sample(&choreography.run_middle),
        },
        InlineRevealFrame {
            line_id: "comment",
            start_span: 1,
            end_span: 2,
            progress: sample(&choreography.function_subject),
        },
        InlineRevealFrame {
            line_id: "comment",
            start_span: 2,
            end_span: 3,
            progress: sample(&choreography.effect_subject),
        },
    ];
    let squiggles = [];
    let annotations = [];
    let frame = EditorFrame {
        panel_offset_x: 0.0,
        panel_offset_y: sample(&choreography.panel_y),
        panel_opacity: 1.0,
        line_marks: &[],
        panel_rotation: sample(&choreography.panel_rotation),
        panel_tilt_x: sample(&choreography.panel_tilt_x),
        panel_tilt_y: sample(&choreography.panel_tilt_y),
        panel_scale: sample(&choreography.panel_scale),
        panel_near_blur: sample(&choreography.panel_near_blur),
        focus_intensity: sample(&choreography.focus).clamp(0.0, 1.0),
        focus_line_y: sample(&choreography.focus_y),
        focus_height: 44.0,
        token_highlight: TokenHighlight {
            x: sample(&choreography.token_x),
            y: sample(&choreography.token_y),
            width: sample(&choreography.token_width),
            opacity: sample(&choreography.token_opacity).clamp(0.0, 1.0),
        },
        bright_text: &[],
        pointer: sample_pointer_frame(&choreography.scene, &choreography.pointer, time),
        inline_reveals: &reveals,
        squiggles: &squiggles,
        annotations: &annotations,
        lines: &lines,
    };
    let mut pixels = renderer.render_editor(&frame)?;
    let nodes = choreography.scene.task_frames_at(time);
    let links = [];
    renderer.composite_task_scene(
        &mut pixels,
        &TaskSceneFrame {
            quote: None,
            links: &links,
            nodes: &nodes,
        },
    )?;
    Ok(pixels)
}

struct DescriptionTransitions {
    effect_run: CodeTransition,
    function: CodeTransition,
    function_comment: CodeTransition,
}

fn effect_is_a_description_transitions() -> Result<DescriptionTransitions> {
    use SyntaxStyle::{Accent, Keyword, Plain, String as StringStyle, Type};

    let document = CodeDocument::new(vec![
        CodeLine::new(
            "import",
            vec![
                span("import", Keyword),
                span(" { ", Plain),
                span("Effect", Accent),
                span(", ", Plain),
                span("Clock", Accent),
                span(" } ", Plain),
                span("from", Keyword),
                span(" \"effect\"", StringStyle),
            ],
        ),
        CodeLine::new("import-blank", vec![]),
        CodeLine::new(
            "comment",
            vec![
                span("// ", Accent),
                span("Functions", Type),
                span("Effects", Type),
                span(" are ", Plain),
                span("LAZY", Accent),
            ],
        ),
        CodeLine::new(
            "definition",
            vec![
                span("const", Keyword),
                span(" getTime", Plain),
                span(": ", Plain),
                span("Effect.Effect", Accent),
                span("<", Plain),
                span("number", Type),
                span(">", Plain),
                span(" = ", Plain),
                span("Clock.currentTimeMillis", Accent),
                span("() => ", Keyword),
                span("Date.now", Accent),
                span("()", Plain),
            ],
        ),
        CodeLine::new("blank", vec![]),
        CodeLine::new(
            "run",
            vec![
                span("Effect.runSync", Accent),
                span("(", Plain),
                span("getTime", Plain),
                span("(", Plain),
                span(")", Plain),
            ],
        ),
    ])?;
    let effect_base = CodeSnapshot::new(["import", "import-blank", "definition"]);
    let effect_running =
        CodeSnapshot::new(["import", "import-blank", "definition", "blank", "run"]);
    let function = CodeSnapshot::new(["definition", "blank", "run"]);
    let function_comment = CodeSnapshot::new(["comment", "definition", "blank", "run"]);
    let layout = CodeLayout {
        line_height: 44.0,
        entering_offset_x: 0.0,
    };
    Ok(DescriptionTransitions {
        effect_run: CodeTransition::compile(&document, &effect_base, &effect_running, layout)?,
        function: CodeTransition::compile(&document, &effect_running, &function, layout)?,
        function_comment: CodeTransition::compile(&document, &function, &function_comment, layout)?,
    })
}

#[cfg(test)]
mod tests {
    use super::effect_is_a_description_transitions;
    use kinograph::code::TransitionProgress;

    #[test]
    fn function_transition_preserves_get_time_line_identity() {
        let transitions = effect_is_a_description_transitions().unwrap();
        let lines = transitions.function.sample(TransitionProgress {
            layout: 0.5,
            content: 0.5,
        });

        for id in ["definition", "blank", "run"] {
            let line = lines
                .iter()
                .find(|line| line.line.id.as_str() == id)
                .unwrap();
            assert_eq!(line.x, 0.0);
            assert_eq!(line.opacity, 1.0);
        }
        assert!({
            let import = lines
                .iter()
                .find(|line| line.line.id.as_str() == "import")
                .unwrap();
            import.x == 0.0 && import.opacity < 1.0
        });
    }
}
