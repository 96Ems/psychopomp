// Stage post-processing: a physically inspired bloom (13-tap downsample,
// tent upsample) and a composite with highlight rolloff, chromatic aberration,
// vignette, and frame-locked film grain.
//
// Live editing: set KINOGRAPH_SHADER_DIR to this directory and re-render.

struct Post {
    texel: vec4<f32>,  // 1 / source width, 1 / source height, 0, 0
    params: vec4<f32>, // bloom intensity, threshold, knee, exposure
    look: vec4<f32>,   // chroma, vignette, grain, frame seed
    shock: vec4<f32>,  // center pixels, burst age (-1 inactive), projected scale
};

@group(0) @binding(0) var<uniform> post: Post;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var linear_sampler: sampler;
@group(0) @binding(3) var bloom: texture_2d<f32>;

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vertex: u32) -> VOut {
    let xy = vec2<f32>(f32((vertex << 1u) & 2u), f32(vertex & 2u));
    var out: VOut;
    out.position = vec4<f32>(xy * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(xy.x, 1.0 - xy.y);
    return out;
}

fn tap(uv: vec2<f32>, offset: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(source, linear_sampler, uv + offset * post.texel.xy, 0.0).rgb;
}

// Jimenez 2014: 13 taps as overlapping 4-tap boxes, which avoids fireflies.
fn downsample13(uv: vec2<f32>) -> vec3<f32> {
    let a = tap(uv, vec2<f32>(-2.0, 2.0));
    let b = tap(uv, vec2<f32>(0.0, 2.0));
    let c = tap(uv, vec2<f32>(2.0, 2.0));
    let d = tap(uv, vec2<f32>(-2.0, 0.0));
    let e = tap(uv, vec2<f32>(0.0, 0.0));
    let f = tap(uv, vec2<f32>(2.0, 0.0));
    let g = tap(uv, vec2<f32>(-2.0, -2.0));
    let h = tap(uv, vec2<f32>(0.0, -2.0));
    let i = tap(uv, vec2<f32>(2.0, -2.0));
    let j = tap(uv, vec2<f32>(-1.0, 1.0));
    let k = tap(uv, vec2<f32>(1.0, 1.0));
    let l = tap(uv, vec2<f32>(-1.0, -1.0));
    let m = tap(uv, vec2<f32>(1.0, -1.0));
    return e * 0.125 + (a + c + g + i) * 0.03125 + (b + d + f + h) * 0.0625 + (j + k + l + m) * 0.125;
}

@fragment
fn prefilter(in: VOut) -> @location(0) vec4<f32> {
    let color = downsample13(in.uv);
    // Soft-knee threshold: only light brighter than the UI palette blooms.
    let brightness = max(color.r, max(color.g, color.b));
    let knee = post.params.z;
    var soft = clamp(brightness - post.params.y + knee, 0.0, 2.0 * knee);
    soft = soft * soft / (4.0 * knee + 1.0e-4);
    let contribution = max(soft, brightness - post.params.y) / max(brightness, 1.0e-4);
    return vec4<f32>(color * contribution, 1.0);
}

@fragment
fn down(in: VOut) -> @location(0) vec4<f32> {
    return vec4<f32>(downsample13(in.uv), 1.0);
}

@fragment
fn up(in: VOut) -> @location(0) vec4<f32> {
    // 3x3 tent, added onto the next larger level by the blend state.
    let sum = tap(in.uv, vec2<f32>(-1.0, -1.0)) + tap(in.uv, vec2<f32>(1.0, -1.0))
        + tap(in.uv, vec2<f32>(-1.0, 1.0)) + tap(in.uv, vec2<f32>(1.0, 1.0))
        + (tap(in.uv, vec2<f32>(0.0, -1.0)) + tap(in.uv, vec2<f32>(-1.0, 0.0))
            + tap(in.uv, vec2<f32>(1.0, 0.0)) + tap(in.uv, vec2<f32>(0.0, 1.0))) * 2.0
        + tap(in.uv, vec2<f32>(0.0, 0.0)) * 4.0;
    return vec4<f32>(sum / 16.0, 1.0);
}

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

// Identity below the knee so authored UI colors stay exact; bright light
// rolls off smoothly instead of clipping.
fn rolloff(c: vec3<f32>) -> vec3<f32> {
    let knee = 0.8;
    let over = max(c - vec3<f32>(knee), vec3<f32>(0.0));
    let compressed = vec3<f32>(knee) + (1.0 - knee) * (vec3<f32>(1.0) - exp(-over / (1.0 - knee)));
    return select(c, compressed, c > vec3<f32>(knee));
}

@fragment
fn composite(in: VOut) -> @location(0) vec4<f32> {
    var uv = in.uv;
    var pressure = 0.0;
    if post.shock.z >= 0.0 {
        let scale = max(post.shock.w, 0.01);
        let delta = in.position.xy - post.shock.xy;
        let wave = pressure_wave(delta, scale, post.shock.z);
        pressure = wave.z;
        uv += wave.xy / vec2<f32>(textureDimensions(source));
    }
    let offset = (in.uv - vec2<f32>(0.5)) * post.look.x * 0.006;
    let uv_r = uv + offset;
    let uv_b = uv - offset;
    let hdr = vec3<f32>(
        textureSampleLevel(source, linear_sampler, uv_r, 0.0).r,
        textureSampleLevel(source, linear_sampler, uv, 0.0).g,
        textureSampleLevel(source, linear_sampler, uv_b, 0.0).b,
    );
    let glow = vec3<f32>(
        textureSampleLevel(bloom, linear_sampler, uv_r, 0.0).r,
        textureSampleLevel(bloom, linear_sampler, uv, 0.0).g,
        textureSampleLevel(bloom, linear_sampler, uv_b, 0.0).b,
    );
    var color = (hdr + glow * post.params.x) * post.params.w;
    color += vec3<f32>(0.035, 0.028, 0.021) * pressure;
    color = rolloff(color);

    let centered = (in.uv - vec2<f32>(0.5)) * vec2<f32>(1.0, 0.82);
    let vignette = 1.0 - post.look.y * smoothstep(0.28, 0.95, length(centered) * 1.35);
    color = color * vignette;

    // Grain in display space, identical for every temporal sample of a frame.
    let pixel = in.position.xy;
    let noise = hash(pixel + vec2<f32>(post.look.w * 17.0, post.look.w * 29.0)) - 0.5;
    var display = pow(max(color, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
    display = display + vec3<f32>(noise * post.look.z);
    color = pow(max(display, vec3<f32>(0.0)), vec3<f32>(2.2));
    return vec4<f32>(color, 1.0);
}
