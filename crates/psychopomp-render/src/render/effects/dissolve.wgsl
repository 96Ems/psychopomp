// Binding-free dissolve mask; no dependencies. Integer-hashed lattice value
// noise, bit-identical to psychopomp::math::random::{hash, lattice_noise2},
// and the field, front, and band widths of psychopomp::effects::dissolve,
// so CPU ash leaves exactly where these pixels burn.
const DISSOLVE_EDGE: f32 = 0.035;
const DISSOLVE_GRAIN: f32 = 0.025; // 1 / 40 world pixels
const DISSOLVE_BURN: f32 = 1.1;

fn fx_lattice_hash(value: u32, salt: u32) -> f32 {
    var h = (value * 0x9E3779B1u) ^ (salt * 0x85EBCA77u);
    h = h ^ (h >> 15u);
    h = h * 0x2C1B3C6Du;
    h = h ^ (h >> 12u);
    h = h * 0x297A2D39u;
    h = h ^ (h >> 15u);
    return f32(h >> 8u) / 16777216.0;
}

fn fx_lattice_at(cell: vec2<i32>, salt: u32) -> f32 {
    return fx_lattice_hash(bitcast<u32>(cell.x) + bitcast<u32>(cell.y) * 0x27D4EB2Fu, salt);
}

fn fx_lattice2(p: vec2<f32>, salt: u32) -> f32 {
    let c = floor(p);
    let f = p - c;
    let i = vec2<i32>(c);
    let u = f * f * (3.0 - 2.0 * f);
    let bottom = mix(fx_lattice_at(i, salt), fx_lattice_at(i + vec2<i32>(1, 0), salt), u.x);
    let top = mix(fx_lattice_at(i + vec2<i32>(0, 1), salt), fx_lattice_at(i + vec2<i32>(1, 1), salt), u.x);
    return mix(bottom, top, u.y);
}

fn fx_lattice_fbm2(p: vec2<f32>, salt: u32) -> f32 {
    var q = p;
    var sum = 0.0;
    var amplitude = 0.5;
    for (var octave = 0u; octave < 3u; octave++) {
        sum += fx_lattice2(q, salt + octave) * amplitude;
        q = q * 2.03 + vec2<f32>(17.0, 5.0);
        amplitude *= 0.5;
    }
    return sum / 0.875;
}

// The field a dissolve front sweeps, 0..1, at `local` world pixels from the
// center of a card with half size `half`: noise leaning from the top left.
fn dissolve_field(local: vec2<f32>, half: vec2<f32>, seed: u32) -> f32 {
    let unit = local / max(half, vec2<f32>(1.0));
    let sweep = clamp((unit.x * 0.75 + unit.y * 0.35) / 1.1 * 0.5 + 0.5, 0.0, 1.0);
    let noise = clamp((fx_lattice_fbm2(local * DISSOLVE_GRAIN, seed) - 0.2) / 0.6, 0.0, 1.0);
    return clamp(0.55 * sweep + 0.45 * noise, 0.0, 1.0);
}

fn dissolve_front(age: f32) -> f32 {
    return clamp(age / DISSOLVE_BURN, 0.0, 1.0) * (1.0 + 2.0 * DISSOLVE_EDGE) - DISSOLVE_EDGE;
}

// (remaining coverage, rim heat, scorch) of a field value at `age`; `aa` is
// the field's change across one pixel. The rim glows just behind the front;
// a scorched band darkens the material just ahead of it.
fn dissolve_burn(field: f32, age: f32, aa: f32) -> vec3<f32> {
    let lead = field - dissolve_front(age);
    let keep = smoothstep(-aa, aa, lead);
    let rim = keep * (1.0 - smoothstep(0.0, DISSOLVE_EDGE, lead));
    let scorch = keep * (1.0 - smoothstep(DISSOLVE_EDGE, DISSOLVE_EDGE * 3.0, lead));
    return vec3<f32>(keep, rim, scorch);
}

// The rim's light: the tone at its cool side, white-hot at the front.
fn dissolve_glow(rim: f32, tone: vec3<f32>) -> vec3<f32> {
    let hot = rim * rim;
    return mix(tone * 0.7, vec3<f32>(1.9, 1.75, 1.6), hot * hot) * rim;
}
