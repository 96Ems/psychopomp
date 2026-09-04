struct SceneUniforms {
    resolution: vec4<f32>,
    focus: vec4<f32>,
    token_highlight: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> scene: SceneUniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );

    let position = positions[vertex_index];
    var output: VertexOutput;
    output.position = vec4<f32>(position, 0.0, 1.0);
    output.uv = position * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    return output;
}

fn sd_rounded_box(point: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(point) - half_size + vec2<f32>(radius);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - radius;
}

fn over(background: vec4<f32>, foreground: vec3<f32>, alpha: f32) -> vec4<f32> {
    let source_alpha = clamp(alpha, 0.0, 1.0);
    let output_alpha = source_alpha + background.a * (1.0 - source_alpha);
    if output_alpha <= 0.0 {
        return vec4<f32>(0.0);
    }
    let output_color = (
        foreground * source_alpha
        + background.rgb * background.a * (1.0 - source_alpha)
    ) / output_alpha;
    return vec4<f32>(output_color, output_alpha);
}

fn fill_box(
    color: vec4<f32>,
    pixel: vec2<f32>,
    center: vec2<f32>,
    size: vec2<f32>,
    radius: f32,
    fill: vec3<f32>,
    opacity: f32
) -> vec4<f32> {
    let distance = sd_rounded_box(pixel - center, size * 0.5, radius);
    let alpha = (1.0 - smoothstep(-0.75, 0.75, distance)) * opacity;
    return over(color, fill, alpha);
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = input.uv * scene.resolution.xy;
    var color = vec4<f32>(0.0);

    let panel_center = vec2<f32>(scene.resolution.x * 0.5, scene.resolution.y * 0.52);
    let panel_size = vec2<f32>(scene.resolution.x * 0.78, scene.resolution.y * 0.70);

    if scene.focus.w > 0.5 {
    let titlebar_center = panel_center - vec2<f32>(0.0, panel_size.y * 0.5 - 32.0);
    color = fill_box(
        color,
        pixel,
        titlebar_center,
        vec2<f32>(panel_size.x - 2.0, 62.0),
        15.0,
        vec3<f32>(0.011, 0.012, 0.014),
        1.0
    );

    // Keep the panel's top corners rounded while giving the title bar a flat bottom edge.
    color = fill_box(
        color,
        pixel,
        titlebar_center + vec2<f32>(0.0, 15.5),
        vec2<f32>(panel_size.x - 2.0, 31.0),
        0.0,
        vec3<f32>(0.011, 0.012, 0.014),
        1.0
    );

    let status_center = vec2<f32>(panel_center.x - panel_size.x * 0.5 + 28.0, titlebar_center.y);
    color = fill_box(
        color,
        pixel,
        status_center,
        vec2<f32>(7.0),
        3.5,
        vec3<f32>(0.13, 0.50, 0.30),
        1.0
    );

    }

    let focus_center = panel_center + vec2<f32>(20.0, scene.focus.y);
    let focus_size = vec2<f32>(panel_size.x - 64.0, scene.focus.z);
    color = fill_box(
        color,
        pixel,
        focus_center,
        focus_size,
        4.0,
        vec3<f32>(0.014, 0.020, 0.018),
        scene.focus.x
    );

    let panel_top = panel_center.y - panel_size.y * 0.5;
    let code_origin = vec2<f32>(scene.resolution.x * 0.145, panel_top + 104.0);
    let token_center = code_origin + vec2<f32>(
        scene.token_highlight.x + scene.token_highlight.z * 0.5,
        scene.token_highlight.y + 22.0
    );
    let token_opacity = scene.token_highlight.w;
    let token_distance = sd_rounded_box(
        pixel - token_center,
        vec2<f32>(scene.token_highlight.z + 12.0, 36.0) * 0.5,
        5.0
    );
    let token_softness = 0.75 + (1.0 - clamp(token_opacity / 0.8, 0.0, 1.0)) * 7.0;
    let token_alpha = (1.0 - smoothstep(-token_softness, token_softness, token_distance)) * token_opacity * 0.72;
    color = over(color, vec3<f32>(0.055, 0.12, 0.095), token_alpha);

    return color;
}
