use std::{collections::HashMap, path::Path};

use anyhow::Result;

use kinograph::{
    code::{
        CodeDocument, CodeLayout, CodeLine, CodeSnapshot, CodeTransition, SyntaxStyle,
        TransitionProgress,
    },
    composition::{Asset, Composition, Time, TimeRange},
    dsl::{
        Annotation, Code, CodeEdit, CompiledScene, Motion, Pointer, Scalar, Scene, TargetGeometry,
        TextTarget,
    },
    timeline::{PropertyId, SpringProfile},
    transcript::Transcript,
};

use crate::{
    plan_runtime::new_renderer,
    render::{EditorFrame, HeadlessRenderer, InlineRevealFrame, SquiggleFrame, TokenHighlight},
};

use super::{
    WORKSPACE_ROOT, encode_scene, measure_target, measure_text_width, plan_temporal_samples,
    sample_pointer_frame, span,
};

const LESSON_AUDIO_DURATION: f64 = 31.708;

pub(crate) async fn render(output: &Path) -> Result<()> {
    let asset_directory = Path::new(WORKSPACE_ROOT)
        .join("assets")
        .join("effect-shows-errors");
    let transcript = Transcript::load(&asset_directory.join("timings.json"))?;
    let transitions = effect_shows_errors_transitions()?;
    let mut renderer = new_renderer("slow-die.ts").await?;
    let initial_lines = transitions.split.sample(TransitionProgress {
        layout: 0.0,
        content: 0.0,
    });
    let settled_lines = transitions.fail.sample(TransitionProgress {
        layout: 1.0,
        content: 1.0,
    });
    let hidden_indent_width = measure_text_width(&mut renderer, "  ")?;
    let hidden_ellipsis_width = measure_text_width(&mut renderer, "...")?;
    let closing_angle_width = measure_text_width(&mut renderer, ">")?;
    let mut effect = measure_target(&mut renderer, &initial_lines, "sig", "Effect.Effect<number")?;
    effect.1.width += closing_angle_width;
    let mut random = measure_target(
        &mut renderer,
        &initial_lines,
        "random",
        "Random.nextIntBetween",
    )?;
    random.1.x -= hidden_indent_width;
    let mut sleep = measure_target(&mut renderer, &initial_lines, "sleep", "Effect.sleep")?;
    sleep.1.x -= hidden_indent_width;
    let mut bad_roll_fail = measure_target(&mut renderer, &settled_lines, "fail", "VeryBadRoll")?;
    bad_roll_fail.1.x -= hidden_ellipsis_width;
    let mut fail_call = measure_target(&mut renderer, &settled_lines, "fail", "Effect.fail")?;
    fail_call.1.x -= hidden_ellipsis_width;
    let targets = HashMap::from([
        effect,
        measure_target(&mut renderer, &initial_lines, "sig", "slowDie")?,
        random,
        sleep,
        measure_target(&mut renderer, &settled_lines, "fail", "(n === 4)")?,
        bad_roll_fail,
        fail_call,
        measure_target(
            &mut renderer,
            &settled_lines,
            "comment",
            "VeryBadRoll is not assignable to never",
        )?,
        measure_target(&mut renderer, &settled_lines, "sig", "VeryBadRoll")?,
    ]);
    let narration = Asset::audio(
        "effect-shows-errors",
        asset_directory.join("narration.webm"),
    )
    .clip(TimeRange::new(
        Time::ZERO,
        Time::seconds(LESSON_AUDIO_DURATION),
    ));
    let success = Asset::audio(
        "prismatic-bloom",
        asset_directory.join("prismatic-bloom.wav"),
    )
    .clip(TimeRange::new(Time::ZERO, Time::seconds(0.795)))
    .gain_db(-10.0);
    let choreography = effect_shows_errors_choreography(&transcript, &targets, narration, success)?;

    encode_scene(
        &mut renderer,
        output,
        &choreography.scene,
        plan_temporal_samples,
        |renderer, time| {
            render_effect_shows_errors_sample(renderer, &transitions, &choreography, time)
        },
    )
}

struct EffectShowsErrorsChoreography {
    scene: CompiledScene,
    code: Code,
    split_edit: CodeEdit,
    fail_edit: CodeEdit,
    focus_height: PropertyId,
    pointer: Pointer,
    fail_action: PropertyId,
    error_comment: PropertyId,
    error_type: PropertyId,
    inline_gen: PropertyId,
    indent: PropertyId,
    ellipsis: PropertyId,
    squiggle: PropertyId,
    squiggle_target: TargetGeometry,
}

fn effect_shows_errors_choreography(
    transcript: &Transcript,
    targets: &HashMap<TextTarget, TargetGeometry>,
    narration: kinograph::composition::Clip,
    success: kinograph::composition::Clip,
) -> Result<EffectShowsErrorsChoreography> {
    let code = Code::new("lesson.code");
    let pointer = Pointer::new("lesson.pointer");
    let split_edit = code.edit("split");
    let fail_edit = code.edit("fail");
    let fail_action = PropertyId::new("lesson.code.fail_action");
    let error_comment = PropertyId::new("lesson.code.error_comment");
    let error_type = PropertyId::new("lesson.code.error_type");
    let inline_gen = PropertyId::new("lesson.code.inline_gen");
    let indent = PropertyId::new("lesson.code.indent");
    let ellipsis = PropertyId::new("lesson.code.ellipsis");
    let squiggle = PropertyId::new("lesson.code.squiggle");
    let focus_height = PropertyId::new("lesson.code.focus_height");
    let spotlight = SpringProfile::from_visual_duration(0.3, 0.0, 0.001, 0.001);
    let variable_part = SpringProfile::from_visual_duration(0.4, 0.0, 0.001, 0.001);
    let line_motion = SpringProfile::from_visual_duration(0.45, 0.0, 0.001, 0.001);
    let pointer_motion = SpringProfile::from_visual_duration(0.5, 0.35, 0.001, 0.001);
    let pointer_scale = 24.0 / 36.0;

    let effect = code.text("sig", "Effect.Effect<number");
    let squiggle_target = targets[&effect];
    let slow_die = code.text("sig", "slowDie");
    let random = code.text("random", "Random.nextIntBetween");
    let sleep = code.text("sleep", "Effect.sleep");
    let roll = code.text("fail", "(n === 4)");
    let bad_roll_fail = code.text("fail", "VeryBadRoll");
    let fail_call = code.text("fail", "Effect.fail");
    let bad_roll_type = code.text("sig", "VeryBadRoll");
    let reckon = transcript.word("reckon")?;
    let success_annotation = Annotation::on(bad_roll_type.clone());

    let at = |cue: kinograph::composition::Cue, motion| cue.at(motion);
    let cursor_at = |target: kinograph::dsl::TextTarget, cue: kinograph::composition::Cue| {
        at(
            cue,
            Motion::parallel([
                code.highlight(target.clone(), spotlight),
                Motion::spring(pointer.opacity.clone(), 1.0, pointer_motion),
                pointer.move_to(target, 55.0, pointer_motion),
            ]),
        )
    };
    let focus_cursor_at = |target: kinograph::dsl::TextTarget, cue: kinograph::composition::Cue| {
        at(
            cue,
            Motion::parallel([
                code.focus(target.clone(), spotlight),
                code.highlight(target.clone(), spotlight),
                Motion::spring(pointer.opacity.clone(), 1.0, pointer_motion),
                pointer.move_to(target, 55.0, pointer_motion),
            ]),
        )
    };
    let composition = Composition::parallel([
        Composition::script(narration),
        reckon.at(Composition::parallel([
            Composition::layer(success),
            Composition::annotate(success_annotation),
        ])),
        Composition::delay(
            kinograph::composition::Duration::milliseconds(80.0),
            Motion::spring(code.panel_y.clone(), 0.0, spotlight),
        ),
        at(
            transcript.word("Effect")?,
            Motion::parallel([
                code.highlight(effect.clone(), spotlight),
                Motion::spring(pointer.opacity.clone(), 1.0, pointer_motion),
                Motion::spring(pointer.scale.clone(), pointer_scale, pointer_motion),
                Motion::spring(pointer.blur.clone(), 0.0, pointer_motion),
                pointer.move_to(effect.clone(), 55.0, pointer_motion),
            ]),
        ),
        cursor_at(slow_die, transcript.word("slowDie.")?),
        focus_cursor_at(random, transcript.word("random")?),
        focus_cursor_at(sleep, transcript.word("non-blocking,")?),
        at(
            transcript.word("result.")?,
            Motion::parallel([
                Motion::spring(code.focus.clone(), 0.0, spotlight),
                Motion::spring(code.highlight_opacity.clone(), 0.0, spotlight),
                Motion::spring(pointer.opacity.clone(), 0.0, pointer_motion),
            ]),
        ),
        at(
            transcript.word("lethal.")?,
            Motion::parallel([
                split_edit.enter(line_motion),
                Motion::spring(inline_gen.clone(), 0.0, variable_part),
                Motion::spring(indent.clone(), 1.0, variable_part),
            ]),
        ),
        at(
            transcript.word("roll")?,
            Motion::parallel([
                fail_edit.enter(line_motion),
                Motion::spring(pointer.opacity.clone(), 1.0, pointer_motion),
                code.focus(roll.clone(), spotlight),
                code.highlight(roll.clone(), spotlight),
                pointer.move_to(roll, 55.0, pointer_motion),
            ]),
        ),
        at(
            transcript.word("fail")?,
            Motion::parallel([
                Motion::spring(fail_action.clone(), 1.0, variable_part),
                Motion::spring(ellipsis.clone(), 0.0, variable_part),
                code.highlight(bad_roll_fail.clone(), spotlight),
                pointer.move_to(bad_roll_fail, 55.0, pointer_motion),
            ]),
        ),
        at(
            transcript.word("finally,")?,
            Motion::parallel([
                Motion::spring(error_comment.clone(), 1.0, variable_part),
                Motion::spring(squiggle.clone(), 1.0, spotlight),
                Motion::spring(code.focus.clone(), 1.0, spotlight),
                Motion::spring(
                    code.focus_y.clone(),
                    Scalar::TargetLineY(effect.clone()).offset(-22.0),
                    spotlight,
                ),
                Motion::spring(focus_height.clone(), 88.0, spotlight),
                Motion::spring(code.highlight_opacity.clone(), 0.0, spotlight),
                Motion::spring(pointer.opacity.clone(), 0.0, pointer_motion),
            ]),
        ),
        Composition::parallel([
            cursor_at(fail_call, transcript.word("propagates")?),
            at(
                transcript.word("propagates")?,
                Motion::parallel([
                    Motion::spring(code.focus.clone(), 0.0, spotlight),
                    Motion::spring(focus_height.clone(), 44.0, spotlight),
                ]),
            ),
        ]),
        at(
            reckon.clone(),
            Motion::parallel([
                Motion::spring(error_comment.clone(), 0.0, variable_part),
                Motion::spring(error_type.clone(), 1.0, variable_part),
                Motion::spring(squiggle.clone(), 0.0, spotlight),
                Motion::spring(code.focus.clone(), 0.0, spotlight),
                code.highlight(bad_roll_type.clone(), spotlight),
                pointer.move_to(bad_roll_type, 55.0, pointer_motion),
            ]),
        ),
        at(
            transcript.word("TypeScript.")?,
            Motion::parallel([
                Motion::spring(code.focus.clone(), 0.0, spotlight),
                Motion::spring(code.highlight_opacity.clone(), 0.0, spotlight),
                Motion::spring(pointer.opacity.clone(), 0.0, spotlight),
            ]),
        ),
    ]);
    let initial_values = [
        (code.panel_y.clone(), Scalar::Literal(250.0)),
        (code.focus.clone(), Scalar::Literal(0.0)),
        (code.focus_y.clone(), Scalar::TargetLineY(effect.clone())),
        (focus_height.clone(), Scalar::Literal(44.0)),
        (code.highlight_x.clone(), Scalar::TargetX(effect.clone())),
        (
            code.highlight_y.clone(),
            Scalar::TargetLineY(effect.clone()),
        ),
        (
            code.highlight_width.clone(),
            Scalar::TargetWidth(effect.clone()),
        ),
        (code.highlight_opacity.clone(), Scalar::Literal(0.0)),
        (
            pointer.x.clone(),
            Scalar::TargetCenterX(effect.clone()).offset(40.0),
        ),
        (pointer.y.clone(), Scalar::TargetLineY(effect).offset(85.0)),
        (pointer.opacity.clone(), Scalar::Literal(0.0)),
        (pointer.scale.clone(), Scalar::Literal(pointer_scale * 0.7)),
        (pointer.blur.clone(), Scalar::Literal(4.0)),
        (fail_action.clone(), Scalar::Literal(0.0)),
        (error_comment.clone(), Scalar::Literal(0.0)),
        (error_type.clone(), Scalar::Literal(0.0)),
        (inline_gen.clone(), Scalar::Literal(1.0)),
        (indent.clone(), Scalar::Literal(0.0)),
        (ellipsis.clone(), Scalar::Literal(1.0)),
        (squiggle.clone(), Scalar::Literal(0.0)),
    ]
    .into_iter()
    .chain(split_edit.initial_values())
    .chain(fail_edit.initial_values());
    let scene = Scene::new(initial_values, composition).compile(targets)?;

    Ok(EffectShowsErrorsChoreography {
        scene,
        code,
        split_edit,
        fail_edit,
        focus_height,
        pointer,
        fail_action,
        error_comment,
        error_type,
        inline_gen,
        indent,
        ellipsis,
        squiggle,
        squiggle_target,
    })
}

fn render_effect_shows_errors_sample(
    renderer: &mut HeadlessRenderer,
    transitions: &LessonTransitions,
    choreography: &EffectShowsErrorsChoreography,
    time: f32,
) -> Result<Vec<u8>> {
    let timeline = choreography.scene.timeline();
    let sample = |property| {
        timeline
            .sample(property, time)
            .expect("lesson choreography property has an initial value")
            .position
    };
    let split_progress = choreography
        .split_edit
        .progress_at(&choreography.scene, time)
        .expect("split edit properties have initial values");
    let lines = if split_progress.content < 0.999 {
        transitions.split.sample(split_progress)
    } else {
        choreography
            .fail_edit
            .sample_at(&choreography.scene, &transitions.fail, time)
            .expect("fail edit properties have initial values")
    };
    let indent = sample(&choreography.indent);
    let reveals = vec![
        InlineRevealFrame {
            line_id: "comment",
            start_span: 0,
            end_span: 1,
            progress: sample(&choreography.error_comment),
        },
        InlineRevealFrame {
            line_id: "sig",
            start_span: 6,
            end_span: 8,
            progress: sample(&choreography.error_type),
        },
        InlineRevealFrame {
            line_id: "sig",
            start_span: 9,
            end_span: 14,
            progress: sample(&choreography.inline_gen),
        },
        InlineRevealFrame {
            line_id: "fail",
            start_span: 3,
            end_span: 4,
            progress: sample(&choreography.ellipsis),
        },
        InlineRevealFrame {
            line_id: "fail",
            start_span: 4,
            end_span: 9,
            progress: sample(&choreography.fail_action),
        },
        InlineRevealFrame {
            line_id: "random",
            start_span: 0,
            end_span: 1,
            progress: indent,
        },
        InlineRevealFrame {
            line_id: "sleep",
            start_span: 0,
            end_span: 1,
            progress: indent,
        },
        InlineRevealFrame {
            line_id: "return",
            start_span: 0,
            end_span: 1,
            progress: indent,
        },
        InlineRevealFrame {
            line_id: "close",
            start_span: 0,
            end_span: 1,
            progress: indent,
        },
    ];
    let squiggles = [SquiggleFrame {
        x: choreography.squiggle_target.x,
        y: choreography.squiggle_target.line_y,
        width: choreography.squiggle_target.width,
        opacity: sample(&choreography.squiggle),
    }];
    let annotations = choreography.scene.annotations_at(time).collect::<Vec<_>>();
    let frame = EditorFrame {
        panel_offset_x: 0.0,
        panel_offset_y: sample(&choreography.code.panel_y),
        panel_opacity: 1.0,
        line_marks: &[],
        panel_rotation: 0.0,
        panel_tilt_x: 0.0,
        panel_tilt_y: 0.0,
        panel_scale: 1.0,
        panel_near_blur: 0.0,
        focus_intensity: sample(&choreography.code.focus).clamp(0.0, 1.0),
        focus_line_y: sample(&choreography.code.focus_y),
        focus_height: sample(&choreography.focus_height),
        token_highlight: TokenHighlight {
            x: sample(&choreography.code.highlight_x),
            y: sample(&choreography.code.highlight_y),
            width: sample(&choreography.code.highlight_width),
            opacity: sample(&choreography.code.highlight_opacity).clamp(0.0, 1.0),
        },
        bright_text: &[],
        pointer: sample_pointer_frame(&choreography.scene, &choreography.pointer, time),
        inline_reveals: &reveals,
        squiggles: &squiggles,
        annotations: &annotations,
        lines: &lines,
    };
    renderer.render_editor(&frame)
}

struct LessonTransitions {
    split: CodeTransition,
    fail: CodeTransition,
}

fn effect_shows_errors_transitions() -> Result<LessonTransitions> {
    use SyntaxStyle::{Accent, Keyword, Plain, String as StringStyle, Type};

    let document = CodeDocument::new(vec![
        CodeLine::new(
            "comment",
            vec![span("// > VeryBadRoll is not assignable to never", Accent)],
        ),
        CodeLine::new(
            "sig",
            vec![
                span("const", Keyword),
                span(" slowDie", Plain),
                span(": ", Plain),
                span("Effect.Effect", Accent),
                span("<", Plain),
                span("number", Type),
                span(", ", Plain),
                span("VeryBadRoll", Accent),
                span("> =", Plain),
                span(" ", Plain),
                span("Effect.gen", Accent),
                span("(", Plain),
                span("function*", Keyword),
                span(" () {", Plain),
            ],
        ),
        CodeLine::new(
            "gen",
            vec![
                span("  Effect.gen", Accent),
                span("(", Plain),
                span("function*", Keyword),
                span(" () {", Plain),
            ],
        ),
        CodeLine::new(
            "random",
            vec![
                span("  ", Plain),
                span("  ", Plain),
                span("const", Keyword),
                span(" n = ", Plain),
                span("yield*", Keyword),
                span(" Random.nextIntBetween", Accent),
                span("(1, 7)", Plain),
            ],
        ),
        CodeLine::new(
            "sleep",
            vec![
                span("  ", Plain),
                span("  ", Plain),
                span("yield*", Keyword),
                span(" Effect.sleep", Accent),
                span("(", Plain),
                span("\"1 second\"", StringStyle),
                span(")", Plain),
            ],
        ),
        CodeLine::new(
            "fail",
            vec![
                span("    ", Plain),
                span("if", Keyword),
                span(" (n === 4) ", Plain),
                span("...", Plain),
                span("yield*", Keyword),
                span(" Effect.fail", Accent),
                span("(new ", Plain),
                span("VeryBadRoll", Type),
                span("())", Plain),
            ],
        ),
        CodeLine::new(
            "return",
            vec![
                span("  ", Plain),
                span("  ", Plain),
                span("return", Keyword),
                span(" n", Plain),
            ],
        ),
        CodeLine::new("close", vec![span("  ", Plain), span("})", Plain)]),
    ])?;
    let initial = CodeSnapshot::new(["comment", "sig", "random", "sleep", "return", "close"]);
    let split = CodeSnapshot::new([
        "comment", "sig", "gen", "random", "sleep", "return", "close",
    ]);
    let final_state = CodeSnapshot::new([
        "comment", "sig", "gen", "random", "sleep", "fail", "return", "close",
    ]);
    let layout = CodeLayout {
        line_height: 44.0,
        entering_offset_x: 0.0,
    };

    Ok(LessonTransitions {
        split: CodeTransition::compile(&document, &initial, &split, layout)?,
        fail: CodeTransition::compile(&document, &split, &final_state, layout)?,
    })
}
