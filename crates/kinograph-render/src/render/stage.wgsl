// Stage primitives: instanced quads whose fragments evaluate a signed distance.
// Output is premultiplied HDR color; glow adds light without occluding (alpha 0),
// so bloom later turns bright emissive color into soft halos.
//
// Live editing: set KINOGRAPH_SHADER_DIR to this directory and re-render; the
// renderer reads the file at startup instead of the compiled-in copy.

struct Globals {
    viewport: vec4<f32>, // width, height, atlas width, atlas height
};

struct Prim {
    bbox: vec4<f32>,   // screen rectangle to rasterize: x0, y0, x1, y1
    a: vec4<f32>,      // kind, then kind-specific
    b: vec4<f32>,      // kind-specific
    fill: vec4<f32>,   // straight linear RGBA
    stroke: vec4<f32>, // straight linear RGBA
    glow: vec4<f32>,   // linear RGB intensity, radius in pixels
    uv: vec4<f32>,     // text atlas rectangle, or polyline point range
    light: vec4<f32>,  // rounded rects: a reflection's x, y, radius, strength
    light_color: vec4<f32>,
    pool: vec4<f32>,   // rounded rects: a pool of light in the glass, same form
    pool_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var<storage, read> prims: array<Prim>;
@group(0) @binding(2) var<storage, read> points: array<vec4<f32>>;
@group(0) @binding(3) var atlas: texture_2d<f32>;
@group(0) @binding(4) var atlas_sampler: sampler;

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) px: vec2<f32>,
    @location(1) @interpolate(flat) index: u32,
};

@vertex
fn vs(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> VOut {
    let prim = prims[instance];
    let corner = vec2<f32>(f32(vertex & 1u), f32((vertex >> 1u) & 1u));
    let px = mix(prim.bbox.xy, prim.bbox.zw, corner);
    var out: VOut;
    out.position = vec4<f32>(
        px.x / globals.viewport.x * 2.0 - 1.0,
        1.0 - px.y / globals.viewport.y * 2.0,
        0.0,
        1.0,
    );
    out.px = px;
    out.index = instance;
    return out;
}

fn sd_round_box(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let r = min(radius, min(half_size.x, half_size.y));
    let q = abs(p) - half_size + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

// Coverage of the inside of a distance field, softened by blur for defocus.
fn coverage(d: f32, blur: f32) -> f32 {
    let width = 0.85 + blur;
    return clamp(0.5 - d / width, 0.0, 1.0);
}

// Light that falls off with distance outside a shape.
fn halo(d: f32, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    let outside = max(d, 0.0);
    return exp(-outside / radius) * exp(-outside / (radius * 3.0)) * 0.9;
}

// The diagrams' reflection falloff: full at the light, 0.65 at 0.3 of its
// radius, 0.16 at 0.7, gone at the radius (REFLECTION in stage.rs).
fn reflection(r: f32) -> f32 {
    if r < 0.3 {
        return mix(1.0, 0.65, r / 0.3);
    }
    if r < 0.7 {
        return mix(0.65, 0.16, (r - 0.3) / 0.4);
    }
    return max(mix(0.16, 0.0, (r - 0.7) / 0.3), 0.0);
}

// Light from things that move. A reflection lights only the border near it;
// a pool also enters the glass, and the border catches some of it.
fn card_light(prim: Prim, px: vec2<f32>, band: f32, inside: f32) -> vec3<f32> {
    var light = vec3<f32>(0.0);
    if prim.light.w > 0.0 {
        let r = length(px - prim.light.xy) / max(prim.light.z, 1.0);
        light += prim.light_color.rgb * prim.light.w * reflection(r) * band * 0.6;
    }
    if prim.pool.w > 0.0 {
        let r = length(px - prim.pool.xy) / max(prim.pool.z, 1.0);
        let field = prim.pool.w * exp(-2.0 * r * r);
        light += prim.pool_color.rgb * field * (inside * 0.12 + band * 0.4);
    }
    return light;
}

fn direction(angle: f32) -> vec2<f32> {
    return vec2<f32>(cos(angle), sin(angle));
}

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    let prim = prims[in.index];
    let kind = u32(prim.a.x + 0.5);
    let px = in.px;
    var color = vec3<f32>(0.0);
    var alpha = 0.0;

    switch kind {
        // Rounded rectangle: a = (kind, cx, cy, corner), b = (half w, half h, border, blur)
        case 0u: {
            let d = sd_round_box(px - prim.a.yz, prim.b.xy, prim.a.w);
            let outer = coverage(d, prim.b.w);
            let inner = coverage(d + prim.b.z, prim.b.w);
            let fill_a = prim.fill.a * inner;
            let stroke_a = prim.stroke.a * max(outer - inner, 0.0);
            alpha = fill_a + stroke_a * (1.0 - fill_a);
            // A restrained overhead key gives the panel thickness without an
            // emissive outline. Socket light is added separately below.
            let height = clamp((px.y - prim.a.z) / max(prim.b.y, 1.0), -1.0, 1.0);
            let key = mix(1.08, 0.92, height * 0.5 + 0.5);
            let rim = mix(1.2, 0.7, height * 0.5 + 0.5);
            color = prim.fill.rgb * fill_a * key + prim.stroke.rgb * stroke_a * (1.0 - fill_a) * rim;
            color += prim.glow.rgb * halo(d, prim.glow.w) * smoothstep(-1.0, 1.5, d);
            color += card_light(prim, px, max(outer - inner, 0.0), inner);
        }
        // Circle: a = (kind, cx, cy, radius), b = (border, 0, 0, blur)
        case 1u: {
            let d = length(px - prim.a.yz) - prim.a.w;
            let outer = coverage(d, prim.b.w);
            let inner = coverage(d + prim.b.x, prim.b.w);
            let fill_a = prim.fill.a * inner;
            let stroke_a = prim.stroke.a * max(outer - inner, 0.0);
            alpha = fill_a + stroke_a * (1.0 - fill_a);
            color = prim.fill.rgb * fill_a + prim.stroke.rgb * stroke_a * (1.0 - fill_a);
            color += prim.glow.rgb * halo(d, prim.glow.w);
        }
        // Arc: a = (kind, cx, cy, radius), b = (thickness, start angle, sweep, blur)
        case 2u: {
            let q = px - prim.a.yz;
            let radius = prim.a.w;
            let half_thickness = prim.b.x * 0.5;
            let sweep = prim.b.z;
            var d = abs(length(q) - radius) - half_thickness;
            if sweep < 6.2831 {
                var angle = atan2(q.y, q.x) - prim.b.y;
                angle = angle - floor(angle / 6.2831853) * 6.2831853;
                if angle > sweep {
                    let start_cap = length(q - direction(prim.b.y) * radius);
                    let end_cap = length(q - direction(prim.b.y + sweep) * radius);
                    d = min(start_cap, end_cap) - half_thickness;
                }
            }
            alpha = prim.stroke.a * coverage(d, prim.b.w);
            color = prim.stroke.rgb * alpha + prim.glow.rgb * halo(d, prim.glow.w);
        }
        // Polyline: a = (kind, width, drawn length, blur), b = (dash, gap, phase, fade)
        // uv = (first point, point count). Points are (x, y, length so far, heat).
        case 3u: {
            let first = u32(prim.uv.x + 0.5);
            let count = u32(prim.uv.y + 0.5);
            let drawn = prim.a.z;
            var best = 1.0e9;
            var along = 0.0;
            var heat = 1.0;
            for (var k = 0u; k + 1u < count; k = k + 1u) {
                let a = points[first + k];
                let b = points[first + k + 1u];
                if a.z >= drawn {
                    break;
                }
                var end = b.xy;
                var end_length = b.z;
                if b.z > drawn {
                    let t = (drawn - a.z) / max(b.z - a.z, 1.0e-4);
                    end = mix(a.xy, b.xy, t);
                    end_length = drawn;
                }
                let pa = px - a.xy;
                let ba = end - a.xy;
                let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1.0e-4), 0.0, 1.0);
                let distance = length(pa - ba * h);
                if distance < best {
                    best = distance;
                    along = mix(a.z, end_length, h);
                    // Points carry a heat (1 for ordinary lines): a cooling trail.
                    heat = mix(a.w, b.w, h * (end_length - a.z) / max(b.z - a.z, 1.0e-4));
                }
            }
            var d = best - prim.a.y * 0.5;
            let dash = prim.b.x;
            if dash > 0.0 {
                // Each dash is a capsule: measure along the path to the nearest dash
                // too, so its glow is round rather than a bar across the line.
                let period = dash + prim.b.y;
                let m = (along + prim.b.z) - floor((along + prim.b.z) / period) * period;
                let gap = select(0.0, min(m - dash, period - m), m > dash);
                d = length(vec2<f32>(best, gap)) - prim.a.y * 0.5;
            }
            // Fade toward the start: a comet trail brightest at its head.
            let strength = mix(1.0, clamp(along / max(drawn, 1.0), 0.0, 1.0), prim.b.w) * heat;
            alpha = prim.stroke.a * coverage(d, prim.a.w) * strength;
            color = prim.stroke.rgb * alpha + prim.glow.rgb * halo(d, prim.glow.w) * strength;
        }
        // Text: a = (kind, left, top, blur), b = (width, height, revealed width, 0)
        // uv = atlas rectangle in texels.
        case 4u: {
            let local = (px - prim.a.yz) / prim.b.xy;
            if all(local >= vec2<f32>(0.0)) && all(local <= vec2<f32>(1.0)) && px.x <= prim.a.y + prim.b.z {
                let uv = mix(prim.uv.xy, prim.uv.zw, local) / globals.viewport.zw;
                var cov = textureSampleLevel(atlas, atlas_sampler, uv, 0.0).r;
                let blur = prim.a.w;
                if blur > 0.25 {
                    let step = vec2<f32>(blur, blur) / globals.viewport.zw * 1.5;
                    cov = (cov * 2.0
                        + textureSampleLevel(atlas, atlas_sampler, uv + vec2<f32>(step.x, 0.0), 0.0).r
                        + textureSampleLevel(atlas, atlas_sampler, uv - vec2<f32>(step.x, 0.0), 0.0).r
                        + textureSampleLevel(atlas, atlas_sampler, uv + vec2<f32>(0.0, step.y), 0.0).r
                        + textureSampleLevel(atlas, atlas_sampler, uv - vec2<f32>(0.0, step.y), 0.0).r) / 6.0;
                }
                alpha = prim.fill.a * cov;
                color = prim.fill.rgb * alpha;
            }
        }
        // Backdrop: a radial gradient from fill (center) to stroke (edge).
        // a = (kind, cx, cy, radius)
        case 5u: {
            let t = clamp(length(px - prim.a.yz) / prim.a.w, 0.0, 1.0);
            color = mix(prim.fill.rgb, prim.stroke.rgb, smoothstep(0.0, 1.0, t));
            alpha = 1.0;
        }
        default: {}
    }
    return vec4<f32>(color, alpha);
}
