use std::{ops::Range, path::Path};

use anyhow::{Context, Result};

use kinograph::{
    code::{CodeLine, PlacedLine, StyledSpan, SyntaxStyle},
    composition::{Time, TimeRange},
    dsl::{AnnotationFrame, Code, CompiledScene, Pointer, TargetGeometry, TextTarget},
};

use crate::{
    exposure::{accumulate, encode_exposures},
    render::{EditorFrame, HeadlessRenderer, InlineRevealFrame, PointerFrame, TokenHighlight},
};

pub(crate) mod effect_institute;
pub(crate) mod effect_is_a_description;
pub(crate) mod effect_shows_errors;
pub(crate) mod opencode_hot_reload;
pub(crate) mod promises_only_happy_path;
pub(crate) mod visual_effects;

const WORKSPACE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// Encode a legacy scene: `samples_at` chooses each frame's shutter samples
/// from its center time; samples are averaged on the CPU.
pub(crate) fn encode_scene(
    renderer: &mut HeadlessRenderer,
    output: &Path,
    scene: &CompiledScene,
    samples_at: impl FnMut(f64) -> u32,
    mut render_sample: impl FnMut(&mut HeadlessRenderer, f32) -> Result<Vec<u8>>,
) -> Result<()> {
    let window = TimeRange::new(Time::ZERO, Time::ZERO.after(scene.duration()));
    encode_exposures(
        renderer,
        output,
        scene.duration(),
        scene.media(),
        window,
        samples_at,
        |time| Ok(time.to_bits()),
        |renderer, exposure| {
            accumulate(renderer, exposure, |renderer, time| {
                render_sample(renderer, time as f32)
            })
        },
    )
}

/// Eight samples per frame inside `ranges` (scene seconds), four elsewhere.
pub(crate) fn boosted_samples(ranges: &[Range<f32>]) -> impl Fn(f64) -> u32 + '_ {
    move |center| {
        if ranges.iter().any(|range| range.contains(&(center as f32))) {
            8
        } else {
            4
        }
    }
}

/// Measure `text` on a placed line, keyed by that same target.
fn measure_target(
    renderer: &mut HeadlessRenderer,
    lines: &[PlacedLine<'_>],
    line_id: &str,
    text: &str,
) -> Result<(TextTarget, TargetGeometry)> {
    let placed = lines
        .iter()
        .find(|placed| placed.line.id.as_str() == line_id)
        .with_context(|| format!("code target line '{line_id}' is not in the settled scene"))?;
    let bounds = renderer.measure_text_range(placed.line, text)?;
    Ok((
        TextTarget::new(line_id, text),
        TargetGeometry {
            x: bounds.x,
            width: bounds.width,
            line_y: placed.y,
        },
    ))
}

/// The perspective-card editor frame whose panel, focus, and highlight are all
/// sampled from `code`.
fn editor_frame<'a>(
    scene: &CompiledScene,
    code: &Code,
    pointer: &Pointer,
    time: f32,
    lines: &'a [PlacedLine<'a>],
    inline_reveals: &'a [InlineRevealFrame<'a>],
    annotations: &'a [AnnotationFrame],
) -> EditorFrame<'a> {
    let sample = |property| {
        scene
            .timeline()
            .sample(property, time)
            .expect("code property has an initial value")
            .position
    };
    EditorFrame {
        panel_offset_x: 0.0,
        panel_offset_y: sample(&code.panel_y),
        panel_opacity: 1.0,
        line_marks: &[],
        panel_rotation: sample(&code.panel_rotation),
        panel_tilt_x: sample(&code.panel_tilt_x),
        panel_tilt_y: sample(&code.panel_tilt_y),
        panel_scale: sample(&code.panel_scale),
        panel_near_blur: sample(&code.panel_near_blur),
        focus_intensity: sample(&code.focus).clamp(0.0, 1.0),
        focus_line_y: sample(&code.focus_y),
        focus_height: 44.0,
        token_highlight: TokenHighlight {
            x: sample(&code.highlight_x),
            y: sample(&code.highlight_y),
            width: sample(&code.highlight_width),
            opacity: sample(&code.highlight_opacity).clamp(0.0, 1.0),
        },
        bright_text: &[],
        pointer: sample_pointer_frame(scene, pointer, time),
        inline_reveals,
        squiggles: &[],
        annotations,
        lines,
    }
}

fn measure_text_width(renderer: &mut HeadlessRenderer, text: &str) -> Result<f32> {
    let line = CodeLine::new("measurement", vec![span(text, SyntaxStyle::Plain)]);
    Ok(renderer.measure_text_range(&line, text)?.width)
}

fn sample_pointer_frame(scene: &CompiledScene, pointer: &Pointer, time: f32) -> PointerFrame {
    let sample_state = |property, sample_time| {
        scene
            .timeline()
            .sample(property, sample_time)
            .expect("pointer property has an initial value")
    };
    let sample = |property| sample_state(property, time).position;
    let x = sample_state(&pointer.x, time);
    let y = sample_state(&pointer.y, time);
    let derivative_step = 1.0 / 240.0;
    let previous_time = (time - derivative_step).max(0.0);
    let previous_x = sample_state(&pointer.x, previous_time);
    let previous_y = sample_state(&pointer.y, previous_time);
    let acceleration_x = (x.velocity - previous_x.velocity) / derivative_step;
    let acceleration_y = (y.velocity - previous_y.velocity) / derivative_step;

    PointerFrame {
        x: x.position,
        y: y.position,
        opacity: sample(&pointer.opacity).clamp(0.0, 1.0),
        rotation: (x.velocity * 0.00012 + y.velocity * 0.00004
            - acceleration_x * 0.000012
            - acceleration_y * 0.000004)
            .clamp(-0.30, 0.30),
        scale: sample(&pointer.scale),
        blur: sample(&pointer.blur).max(0.0),
    }
}

fn span(text: &str, style: SyntaxStyle) -> StyledSpan {
    StyledSpan::new(text, style)
}
