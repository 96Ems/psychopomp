use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Result;

use kinograph::{
    code::{
        CodeDocument, CodeLayout, CodeLine, CodeSnapshot, CodeTransition, PlacedLine, SyntaxStyle,
        TransitionProgress,
    },
    dsl::{Code, CompiledScene, Motion, Pointer, Scalar, Scene},
    render::{
        EditorFrame, HeadlessRenderer, InlineRevealFrame, PointerFrame, RenderSpec, TokenHighlight,
    },
    timeline::{PropertyId, SpringProfile},
};

use super::{
    CodeTarget, FONT_PATH, HEIGHT, WIDTH, encode_video, measure_target, sample_pointer_frame, span,
};

const FOCUS_LINE_ID: &str = "find-effect";

pub(crate) async fn render(output: &Path) -> Result<()> {
    let transition = hero_code_transition()?;
    let mut renderer = HeadlessRenderer::new(RenderSpec {
        width: WIDTH,
        height: HEIGHT,
        font_path: PathBuf::from(FONT_PATH),
        file_name: "service.ts".to_owned(),
    })
    .await?;
    let settled_lines = transition.sample(TransitionProgress {
        layout: 1.0,
        content: 1.0,
    });
    let effect = measure_target(
        &mut renderer,
        &settled_lines,
        FOCUS_LINE_ID,
        "Effect.Effect",
    )?;
    let string = measure_target(&mut renderer, &settled_lines, "find-signature", "string")?;
    let not_found = measure_target(&mut renderer, &settled_lines, FOCUS_LINE_ID, "NotFound")?;
    let context = measure_target(&mut renderer, &settled_lines, "class-open", "Context.Tag")?;
    let choreography = hero_choreography(effect, string, not_found, context)?;

    encode_video(
        &mut renderer,
        output,
        &choreography.scene,
        |renderer, time| {
            let (
                panel_offset_y,
                focus_intensity,
                focus_line_y,
                token_highlight,
                pointer,
                inline_reveal,
                lines,
            ) = sample_editor(&transition, &choreography, time);
            let inline_reveals = [inline_reveal];
            let squiggles = [];
            let annotations = [];
            let frame = EditorFrame {
                panel_offset_y,
                panel_rotation: 0.0,
                panel_tilt_x: 0.0,
                panel_tilt_y: 0.0,
                panel_scale: 1.0,
                panel_near_blur: 0.0,
                focus_intensity,
                focus_line_y,
                focus_height: 44.0,
                token_highlight,
                pointer,
                inline_reveals: &inline_reveals,
                squiggles: &squiggles,
                annotations: &annotations,
                lines: &lines,
            };
            renderer.render_editor(&frame)
        },
    )
}

struct HeroChoreography {
    scene: CompiledScene,
    panel_y: PropertyId,
    code_layout: PropertyId,
    code_content: PropertyId,
    focus: PropertyId,
    token_x: PropertyId,
    token_y: PropertyId,
    token_width: PropertyId,
    token_opacity: PropertyId,
    pointer: Pointer,
    inline_reveal: PropertyId,
}

fn hero_choreography(
    effect: CodeTarget,
    string: CodeTarget,
    not_found: CodeTarget,
    context: CodeTarget,
) -> Result<HeroChoreography> {
    let code = Code::new("editor");
    let panel_y = code.panel_y.clone();
    let code_layout = code.layout.clone();
    let code_content = code.content.clone();
    let focus = code.focus.clone();
    let token_x = code.highlight_x.clone();
    let token_y = code.highlight_y.clone();
    let token_width = code.highlight_width.clone();
    let token_opacity = code.highlight_opacity.clone();
    let pointer = Pointer::new("pointer");
    let pointer_x = pointer.x.clone();
    let pointer_y = pointer.y.clone();
    let pointer_opacity = pointer.opacity.clone();
    let pointer_scale = pointer.scale.clone();
    let pointer_blur = pointer.blur.clone();
    let inline_reveal = code.inline_reveal.clone();
    let profile = SpringProfile::from_visual_duration(0.3, 0.0, 0.001, 0.001);
    let pointer_profile = SpringProfile::from_visual_duration(0.42, 0.18, 0.001, 0.001);
    let effect_target = code.text(FOCUS_LINE_ID, "Effect.Effect");
    let string_target = code.text("find-signature", "string");
    let not_found_target = code.text(FOCUS_LINE_ID, "NotFound");
    let context_target = code.text("class-open", "Context.Tag");
    let targets = HashMap::from([
        (effect_target.clone(), effect.into()),
        (string_target.clone(), string.into()),
        (not_found_target.clone(), not_found.into()),
        (context_target.clone(), context.into()),
    ]);
    let animation = Motion::parallel([
        Motion::hold(5.0),
        Motion::delay(0.08, Motion::spring(panel_y.clone(), 0.0, profile)),
        Motion::delay(0.88, Motion::spring(code_layout.clone(), 1.0, profile)),
        Motion::delay(1.12, Motion::spring(code_content.clone(), 1.0, profile)),
        Motion::delay(1.55, Motion::spring(focus.clone(), 1.0, profile)),
        Motion::delay(1.72, code.highlight(effect_target.clone(), profile)),
        Motion::delay(
            1.72,
            Motion::spring(pointer_opacity.clone(), 1.0, pointer_profile),
        ),
        Motion::delay(
            1.72,
            Motion::spring(pointer_scale.clone(), 1.0, pointer_profile),
        ),
        Motion::delay(
            1.72,
            Motion::spring(pointer_blur.clone(), 0.0, pointer_profile),
        ),
        Motion::delay(
            1.72,
            pointer.move_to(effect_target.clone(), 55.0, pointer_profile),
        ),
        Motion::delay(2.72, code.reveal_inline(profile)),
        Motion::delay(3.15, code.highlight(not_found_target.clone(), profile)),
        Motion::delay(2.35, pointer.move_to(string_target, 55.0, pointer_profile)),
        Motion::delay(
            3.15,
            pointer.move_to(not_found_target, 55.0, pointer_profile),
        ),
        Motion::delay(4.05, pointer.move_to(context_target, 55.0, pointer_profile)),
    ]);
    let scene = Scene::new(
        [
            (panel_y.clone(), Scalar::Literal(250.0)),
            (code_layout.clone(), Scalar::Literal(0.0)),
            (code_content.clone(), Scalar::Literal(0.0)),
            (focus.clone(), Scalar::Literal(0.0)),
            (token_x.clone(), Scalar::TargetX(effect_target.clone())),
            (token_y.clone(), Scalar::TargetLineY(effect_target.clone())),
            (
                token_width.clone(),
                Scalar::TargetWidth(effect_target.clone()),
            ),
            (token_opacity.clone(), Scalar::Literal(0.0)),
            (
                pointer_x.clone(),
                Scalar::TargetCenterX(effect_target.clone()).offset(40.0),
            ),
            (
                pointer_y.clone(),
                Scalar::TargetBelow {
                    target: effect_target,
                    offset: 85.0,
                },
            ),
            (pointer_opacity.clone(), Scalar::Literal(0.0)),
            (pointer_scale.clone(), Scalar::Literal(0.7)),
            (pointer_blur.clone(), Scalar::Literal(4.0)),
            (inline_reveal.clone(), Scalar::Literal(0.0)),
        ],
        animation,
    );
    let scene = scene.compile(&targets)?;

    Ok(HeroChoreography {
        scene,
        panel_y,
        code_layout,
        code_content,
        focus,
        token_x,
        token_y,
        token_width,
        token_opacity,
        pointer,
        inline_reveal,
    })
}

fn sample_editor<'a>(
    transition: &'a CodeTransition,
    choreography: &HeroChoreography,
    time: f32,
) -> (
    f32,
    f32,
    f32,
    TokenHighlight,
    PointerFrame,
    InlineRevealFrame,
    Vec<PlacedLine<'a>>,
) {
    let sample = |property| {
        choreography
            .scene
            .timeline()
            .sample(property, time)
            .expect("hero choreography property has an initial value")
            .position
    };
    let panel_offset_y = sample(&choreography.panel_y);
    let layout_progress = sample(&choreography.code_layout);
    let content_progress = sample(&choreography.code_content);
    let focus_intensity = sample(&choreography.focus).clamp(0.0, 1.0);
    let token_highlight = TokenHighlight {
        x: sample(&choreography.token_x),
        y: sample(&choreography.token_y),
        width: sample(&choreography.token_width),
        opacity: sample(&choreography.token_opacity).clamp(0.0, 1.0),
    };
    let pointer = sample_pointer_frame(&choreography.scene, &choreography.pointer, time);
    let inline_reveal = InlineRevealFrame {
        line_id: FOCUS_LINE_ID,
        start_span: 4,
        end_span: 6,
        progress: sample(&choreography.inline_reveal),
    };
    let lines = transition.sample(TransitionProgress {
        layout: layout_progress,
        content: content_progress,
    });
    let focus_line_y = lines
        .iter()
        .find(|placed| placed.line.id.as_str() == FOCUS_LINE_ID)
        .expect("focus line is part of the compiled code manifest")
        .y;
    (
        panel_offset_y,
        focus_intensity,
        focus_line_y,
        token_highlight,
        pointer,
        inline_reveal,
        lines,
    )
}

fn hero_code_transition() -> Result<CodeTransition> {
    use SyntaxStyle::{Accent, Keyword, Plain, String as StringStyle, Type};

    let document = CodeDocument::new(vec![
        CodeLine::new(
            "import",
            vec![
                span("import", Keyword),
                span(" { Context, Effect } ", Plain),
                span("from", Keyword),
                span(" \"effect\"", StringStyle),
            ],
        ),
        CodeLine::new("spacer", vec![]),
        CodeLine::new(
            "class-open",
            vec![
                span("class", Keyword),
                span(" UserService ", Type),
                span("extends", Keyword),
                span(" Context.Tag(", Plain),
                span("\"UserService\"", StringStyle),
                span(")<", Plain),
            ],
        ),
        CodeLine::new(
            "service-self",
            vec![
                span("  ", Plain),
                span("UserService", Type),
                span(",", Plain),
            ],
        ),
        CodeLine::new("shape-open", vec![span("  {", Plain)]),
        CodeLine::new(
            "find-signature",
            vec![
                span("    ", Plain),
                span("readonly", Keyword),
                span(" find: (id: ", Plain),
                span("string", Type),
                span(") =>", Plain),
            ],
        ),
        CodeLine::new(
            FOCUS_LINE_ID,
            vec![
                span("      ", Plain),
                span("Effect.Effect", Accent),
                span("<", Plain),
                span("User", Type),
                span(", ", Plain),
                span("NotFound", Accent),
                span(">", Plain),
            ],
        ),
        CodeLine::new("shape-close", vec![span("  }", Plain)]),
        CodeLine::new("close", vec![span(">() {}", Plain)]),
    ])?;

    CodeTransition::compile(
        &document,
        &CodeSnapshot::new([
            "import",
            "spacer",
            "class-open",
            "service-self",
            "shape-open",
            "shape-close",
            "close",
        ]),
        &CodeSnapshot::new([
            "import",
            "spacer",
            "class-open",
            "service-self",
            "shape-open",
            "find-signature",
            FOCUS_LINE_ID,
            "shape-close",
            "close",
        ]),
        CodeLayout {
            line_height: 44.0,
            entering_offset_x: 96.0,
        },
    )
}
