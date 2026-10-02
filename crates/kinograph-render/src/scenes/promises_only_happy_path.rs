use std::{collections::HashMap, path::Path};

use anyhow::Result;

use kinograph::{
    code::{
        CodeDocument, CodeLayout, CodeLine, CodeSnapshot, CodeTransition, SyntaxStyle,
        TransitionProgress,
    },
    composition::{Asset, Composition, Time, TimeRange},
    dsl::{
        Annotation, AnnotationEffect, Code, CodeEdit, CompiledScene, Motion, Pointer, Scalar,
        Scene, TargetGeometry, TextTarget,
    },
    timeline::{PropertyId, SpringProfile},
    transcript::Transcript,
};

use crate::{
    plan_runtime::new_renderer,
    render::{HeadlessRenderer, InlineRevealFrame},
};

use super::{
    WORKSPACE_ROOT, editor_frame, encode_scene, measure_target, measure_text_width,
    plan_temporal_samples, span,
};

const PROMISES_AUDIO_DURATION: f64 = 31.107;

pub(crate) async fn render(output: &Path) -> Result<()> {
    let asset_directory = Path::new(WORKSPACE_ROOT)
        .join("assets")
        .join("promises-only-happy-path");
    let transcript = Transcript::load(&asset_directory.join("timings.json"))?;
    let transition = promises_only_happy_path_transition()?;
    let mut renderer = new_renderer("checkout.ts").await?;
    let settled_lines = transition.sample(TransitionProgress {
        layout: 1.0,
        content: 1.0,
    });
    let question_width = measure_text_width(&mut renderer, " // ???")?;
    let (error_target, mut error) =
        measure_target(&mut renderer, &settled_lines, "call", "SomeError")?;
    error.x -= question_width;
    let targets = HashMap::from([
        measure_target(&mut renderer, &settled_lines, "sig", "checkout")?,
        measure_target(&mut renderer, &settled_lines, "sig", "Promise<Order>")?,
        measure_target(&mut renderer, &settled_lines, "cart", "getCart")?,
        measure_target(&mut renderer, &settled_lines, "payment", "charge")?,
        measure_target(&mut renderer, &settled_lines, "shipment", "ship(payment)")?,
        measure_target(&mut renderer, &settled_lines, "call", "cart-123")?,
        measure_target(&mut renderer, &settled_lines, "sig", "Order")?,
        measure_target(&mut renderer, &settled_lines, "call", "???")?,
        (error_target, error),
    ]);
    let narration = Asset::audio(
        "promises-only-happy-path",
        asset_directory.join("narration.webm"),
    )
    .clip(TimeRange::new(
        Time::ZERO,
        Time::seconds(PROMISES_AUDIO_DURATION),
    ));
    let sad = Asset::audio("sad", asset_directory.join("sad.wav"))
        .clip(TimeRange::new(Time::ZERO, Time::seconds(0.67)));
    let choreography =
        promises_only_happy_path_choreography(&transcript, &targets, narration, sad)?;

    encode_scene(
        &mut renderer,
        output,
        &choreography.scene,
        plan_temporal_samples,
        |renderer, time| {
            render_promises_only_happy_path_sample(renderer, &transition, &choreography, time)
        },
    )
}

struct PromisesOnlyHappyPathChoreography {
    scene: CompiledScene,
    code: Code,
    call_edit: CodeEdit,
    pointer: Pointer,
    question: PropertyId,
    error: PropertyId,
}

fn promises_only_happy_path_choreography(
    transcript: &Transcript,
    targets: &HashMap<TextTarget, TargetGeometry>,
    narration: kinograph::composition::Clip,
    sad: kinograph::composition::Clip,
) -> Result<PromisesOnlyHappyPathChoreography> {
    let code = Code::new("promise.code");
    let pointer = Pointer::new("promise.pointer");
    let call_edit = code.edit("call");
    let question = PropertyId::new("promise.code.question");
    let error = PropertyId::new("promise.code.error");
    let spotlight = SpringProfile::from_visual_duration(0.3, 0.0, 0.001, 0.001);
    let entrance = SpringProfile::from_visual_duration(0.7, 0.22, 0.001, 0.001);
    let line_motion = SpringProfile::from_visual_duration(0.42, 0.0, 0.001, 0.001);
    let pointer_motion = SpringProfile::from_visual_duration(0.5, 0.35, 0.001, 0.001);
    let pointer_scale = 24.0 / 36.0;

    let checkout = code.text("sig", "checkout");
    let promise = code.text("sig", "Promise<Order>");
    let get_cart = code.text("cart", "getCart");
    let charge = code.text("payment", "charge");
    let ship = code.text("shipment", "ship(payment)");
    let call = code.text("call", "cart-123");
    let order = code.text("sig", "Order");
    let question_target = code.text("call", "???");
    let error_target = code.text("call", "SomeError");
    let forthcoming = transcript.word("forthcoming")?;
    let at = |cue: kinograph::composition::Cue, motion| cue.at(motion);
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
        Composition::delay(
            kinograph::composition::Duration::milliseconds(80.0),
            Motion::parallel([
                Motion::spring(code.panel_y.clone(), 0.0, entrance),
                Motion::spring(code.panel_rotation.clone(), 0.0, entrance),
                Motion::spring(code.panel_tilt_x.clone(), 0.0, entrance),
                Motion::spring(code.panel_tilt_y.clone(), 0.0, entrance),
                Motion::spring(code.panel_scale.clone(), 1.0, entrance),
                Motion::spring(code.panel_near_blur.clone(), 0.0, entrance),
            ]),
        ),
        focus_cursor_at(checkout.clone(), transcript.word("checkout")?),
        focus_cursor_at(promise.clone(), transcript.word("Promise<Order>.")?),
        at(
            transcript.word("Internally,")?,
            Motion::parallel([
                Motion::spring(code.focus.clone(), 0.0, spotlight),
                Motion::spring(code.highlight_opacity.clone(), 0.0, spotlight),
            ]),
        ),
        focus_cursor_at(get_cart, transcript.word("three")?),
        focus_cursor_at(charge, transcript.word("more")?),
        focus_cursor_at(ship, transcript.word("async")?),
        at(
            transcript.word("What")?,
            Motion::parallel([
                Motion::spring(code.focus.clone(), 0.0, spotlight),
                Motion::spring(code.highlight_opacity.clone(), 0.0, spotlight),
                Motion::spring(pointer.opacity.clone(), 0.0, pointer_motion),
            ]),
        ),
        at(
            transcript.word("call")?,
            Motion::parallel([
                call_edit.enter(line_motion),
                code.focus(call.clone(), spotlight),
                code.highlight(call.clone(), spotlight),
                Motion::spring(pointer.opacity.clone(), 1.0, pointer_motion),
                pointer.move_to(call, 55.0, pointer_motion),
            ]),
        ),
        focus_cursor_at(promise.clone(), transcript.word("type")?),
        focus_cursor_at(order, transcript.word("Order")?),
        at(
            transcript.word("fail?")?,
            Motion::parallel([
                Motion::spring(question.clone(), 1.0, line_motion),
                code.focus(question_target.clone(), spotlight),
                code.highlight(question_target.clone(), spotlight),
                pointer.move_to(question_target, 55.0, pointer_motion),
            ]),
        ),
        at(
            transcript.word("how")?,
            Motion::parallel([
                Motion::spring(question.clone(), 0.0, line_motion),
                Motion::spring(error.clone(), 1.0, line_motion),
                code.focus(error_target.clone(), spotlight),
                code.highlight(error_target.clone(), spotlight),
                pointer.move_to(error_target, 55.0, pointer_motion),
            ]),
        ),
        forthcoming.at(Composition::parallel([
            Composition::layer(sad),
            Composition::animate(Motion::parallel([
                call_edit.exit(line_motion),
                Motion::spring(question.clone(), 0.0, line_motion),
                Motion::spring(error.clone(), 0.0, line_motion),
                code.focus(promise.clone(), spotlight),
                code.highlight(promise.clone(), spotlight),
                Motion::spring(pointer.opacity.clone(), 0.0, pointer_motion),
            ])),
            Composition::annotate(Annotation::on(promise).effect(AnnotationEffect::FocusPulse)),
        ])),
        at(
            transcript.word("regard.")?,
            Motion::parallel([
                Motion::spring(code.focus.clone(), 0.0, spotlight),
                Motion::spring(code.highlight_opacity.clone(), 0.0, spotlight),
            ]),
        ),
    ]);
    let initial_values = [
        (code.panel_y.clone(), Scalar::Literal(280.0)),
        (code.panel_rotation.clone(), Scalar::Literal(-0.14)),
        (code.panel_tilt_x.clone(), Scalar::Literal(-0.31)),
        (code.panel_tilt_y.clone(), Scalar::Literal(0.38)),
        (code.panel_scale.clone(), Scalar::Literal(1.5)),
        (code.panel_near_blur.clone(), Scalar::Literal(4.0)),
        (code.focus.clone(), Scalar::Literal(0.0)),
        (code.focus_y.clone(), Scalar::TargetLineY(checkout.clone())),
        (code.highlight_x.clone(), Scalar::TargetX(checkout.clone())),
        (
            code.highlight_y.clone(),
            Scalar::TargetLineY(checkout.clone()),
        ),
        (
            code.highlight_width.clone(),
            Scalar::TargetWidth(checkout.clone()),
        ),
        (code.highlight_opacity.clone(), Scalar::Literal(0.0)),
        (
            pointer.x.clone(),
            Scalar::TargetCenterX(checkout.clone()).offset(40.0),
        ),
        (
            pointer.y.clone(),
            Scalar::TargetBelow {
                target: checkout,
                offset: 85.0,
            },
        ),
        (pointer.opacity.clone(), Scalar::Literal(0.0)),
        (pointer.scale.clone(), Scalar::Literal(pointer_scale)),
        (pointer.blur.clone(), Scalar::Literal(0.0)),
        (question.clone(), Scalar::Literal(0.0)),
        (error.clone(), Scalar::Literal(0.0)),
    ]
    .into_iter()
    .chain(call_edit.initial_values());
    let scene = Scene::new(initial_values, composition).compile(targets)?;

    Ok(PromisesOnlyHappyPathChoreography {
        scene,
        code,
        call_edit,
        pointer,
        question,
        error,
    })
}

fn render_promises_only_happy_path_sample(
    renderer: &mut HeadlessRenderer,
    transition: &CodeTransition,
    choreography: &PromisesOnlyHappyPathChoreography,
    time: f32,
) -> Result<Vec<u8>> {
    let timeline = choreography.scene.timeline();
    let sample = |property| {
        timeline
            .sample(property, time)
            .expect("promise choreography property has an initial value")
            .position
    };
    let lines = choreography
        .call_edit
        .sample_at(&choreography.scene, transition, time)
        .expect("call edit properties have initial values");
    let reveals = [
        InlineRevealFrame {
            line_id: "call",
            start_span: 3,
            end_span: 4,
            progress: sample(&choreography.question),
        },
        InlineRevealFrame {
            line_id: "call",
            start_span: 4,
            end_span: 6,
            progress: sample(&choreography.error),
        },
    ];
    let annotations = choreography.scene.annotations_at(time).collect::<Vec<_>>();
    renderer.render_editor(&editor_frame(
        &choreography.scene,
        &choreography.code,
        &choreography.pointer,
        time,
        &lines,
        &reveals,
        &annotations,
    ))
}

fn promises_only_happy_path_transition() -> Result<CodeTransition> {
    use SyntaxStyle::{Accent, Keyword, Plain, String as StringStyle, Type};

    let document = CodeDocument::new(vec![
        CodeLine::new(
            "sig",
            vec![
                span("async", Keyword),
                span(" function ", Keyword),
                span("checkout", Plain),
                span("(cartId: ", Plain),
                span("string", Type),
                span("): ", Plain),
                span("Promise", Accent),
                span("<", Plain),
                span("Order", Type),
                span("> {", Plain),
            ],
        ),
        CodeLine::new(
            "cart",
            vec![
                span("  const", Keyword),
                span(" cart = ", Plain),
                span("await", Keyword),
                span(" getCart", Accent),
                span("(cartId)", Plain),
            ],
        ),
        CodeLine::new(
            "payment",
            vec![
                span("  const", Keyword),
                span(" payment = ", Plain),
                span("await", Keyword),
                span(" charge", Accent),
                span("(cart)", Plain),
            ],
        ),
        CodeLine::new(
            "shipment",
            vec![
                span("  const", Keyword),
                span(" shipment = ", Plain),
                span("await", Keyword),
                span(" ship", Accent),
                span("(payment)", Plain),
            ],
        ),
        CodeLine::new(
            "return",
            vec![span("  return", Keyword), span(" shipment", Plain)],
        ),
        CodeLine::new("close", vec![span("}", Plain)]),
        CodeLine::new("spacer", vec![]),
        CodeLine::new(
            "call",
            vec![
                span("checkout(", Plain),
                span("\"cart-123\"", StringStyle),
                span(")", Plain),
                span(" // ???", Accent),
                span(" // throws ", Plain),
                span("SomeError", Type),
            ],
        ),
    ])?;
    let initial = CodeSnapshot::new(["sig", "cart", "payment", "shipment", "return", "close"]);
    let called = CodeSnapshot::new([
        "sig", "cart", "payment", "shipment", "return", "close", "spacer", "call",
    ]);
    CodeTransition::compile(
        &document,
        &initial,
        &called,
        CodeLayout {
            line_height: 44.0,
            entering_offset_x: 0.0,
        },
    )
}
