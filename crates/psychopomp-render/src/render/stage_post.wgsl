// Stage post-processing: a physically inspired bloom (13-tap downsample,
// tent upsample) and a composite with the burst's pressure wave, VHS rewind
// interference, highlight rolloff, chromatic aberration, vignette, and
// frame-locked film grain. Concatenated after effects/noise, pressure, rewind.
//
// Live editing: set PSYCHOPOMP_SHADER_DIR to this directory and re-render.

struct Post {
    texel: vec4<f32>,  // 1 / source width, 1 / source height, 0, 0
    params: vec4<f32>, // bloom intensity, threshold, knee, exposure
    look: vec4<f32>,   // chroma, vignette, grain, frame seed
    shock: vec4<f32>,  // center pixels, burst age (-1 inactive), projected scale
    rewind: vec4<f32>, // VHS rewind age (-1 inactive), background luminance, camera roll (radians), punch-in
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

// One shutter sample, weighted by `params.x`, added into the exposure.
@fragment
fn accumulate(in: VOut) -> @location(0) vec4<f32> {
    let light = textureLoad(source, vec2<i32>(in.position.xy), 0).rgb;
    return vec4<f32>(light * post.params.x, 1.0);
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
    let size = vec2<f32>(textureDimensions(source));
    // Camera roll and punch-in, about the frame center in square pixels. The
    // punch also covers the corners a roll would otherwise expose.
    let roll = post.rewind.z;
    let zoom = 1.0 + post.rewind.w + abs(roll) * 0.6;
    let from_center = (in.uv - vec2<f32>(0.5)) * size;
    let turned = vec2<f32>(
        from_center.x * cos(roll) + from_center.y * sin(roll),
        -from_center.x * sin(roll) + from_center.y * cos(roll),
    );
    var uv = turned / zoom / size + vec2<f32>(0.5);
    var pressure = 0.0;
    if post.shock.z >= 0.0 {
        let scale = max(post.shock.w, 0.01);
        let delta = in.position.xy - post.shock.xy;
        let wave = pressure_wave(delta, scale, post.shock.z);
        pressure = wave.z;
        uv += wave.xy / size;
    }
    let tape = rewind_envelope(post.rewind.x);
    uv.x += rewind_tear(in.position.xy, size, post.rewind.x, tape) / size.x;
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
    if tape > 0.0 {
        // Snow sits only on ink, so the empty canvas stays clean; shade darkens
        // the tracking band and scanlines everywhere (invisible on black).
        let vhs = rewind_snow(in.position.xy, size, post.rewind.x, tape);
        let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
        let ink = smoothstep(0.004, 0.03, luma - post.rewind.y);
        let alpha = clamp(vhs.x * ink + vhs.y, 0.0, 0.7);
        color = color * (1.0 - alpha) + vec3<f32>(0.71, 0.75, 0.81) * (vhs.x * ink);
    }
    color += vec3<f32>(0.035, 0.028, 0.021) * pressure;
    color = rolloff(color);

    let centered = (in.uv - vec2<f32>(0.5)) * vec2<f32>(1.0, 0.82);
    let vignette = 1.0 - post.look.y * smoothstep(0.28, 0.95, length(centered) * 1.35);
    color = color * vignette;

    // Grain in display space, identical for every temporal sample of a frame.
    let pixel = in.position.xy;
    let noise = fx_hash2(pixel + vec2<f32>(post.look.w * 17.0, post.look.w * 29.0)) - 0.5;
    var display = pow(max(color, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
    display = display + vec3<f32>(noise * post.look.z);
    color = pow(max(display, vec3<f32>(0.0)), vec3<f32>(2.2));
    return vec4<f32>(color, 1.0);
}
