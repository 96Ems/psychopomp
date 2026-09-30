// Binding-free VHS rewind interference, after the OpenCode blog's tape rewind
// (`vhsRewind.wgsl` and its tracking map). Every function is a pure function of
// pixel and local age in seconds, so shutter samples and seeks agree exactly.

const REWIND_SECONDS: f32 = 1.4;

fn rewind_smooth(x: f32) -> f32 {
    let t = clamp(x, 0.0, 1.0);
    return t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
}

// Quintic edges: zero slope and acceleration at both clean endpoints.
fn rewind_envelope(age: f32) -> f32 {
    if age <= 0.0 || age >= REWIND_SECONDS { return 0.0; }
    let edge = min(0.3, REWIND_SECONDS * 0.4);
    return rewind_smooth(age / edge) * rewind_smooth((REWIND_SECONDS - age) / edge);
}

// One repeating tape coordinate drives both the tear and the snow.
fn rewind_tape(y: f32, age: f32) -> f32 {
    return fract(y + age * 1.7);
}

fn rewind_tracking(tape: f32) -> f32 {
    return exp(-pow((tape - 0.22) / 0.055, 2.0));
}

fn rewind_seam(tape: f32) -> f32 {
    return exp(-pow((tape - 0.66) / 0.018, 2.0));
}

fn rewind_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// Horizontal tear in pixels: the tracking band pulls one way with a per-row
// wobble on the tape's 512-row grid, and the thin seam snaps the other way.
fn rewind_tear(pixel: vec2<f32>, size: vec2<f32>, age: f32, amount: f32) -> f32 {
    let tape = rewind_tape(pixel.y / size.y, age);
    let row = floor(pixel.y * 512.0 / size.y);
    let shift = rewind_tracking(tape) * (0.62 + 0.25 * sin(row * 1.9)) - rewind_seam(tape) * 0.85;
    return shift * 9.0 * amount * size.y / 1080.0;
}

// Snow and shade: 2 px grain at 30 fps, dropout streaks in the tracking band,
// the seam's bright line, and 3 px scanlines. Returns (snow, shade), scaled.
fn rewind_snow(pixel: vec2<f32>, size: vec2<f32>, age: f32, amount: f32) -> vec2<f32> {
    let tape = rewind_tape(pixel.y / size.y, age);
    let tracking = rewind_tracking(tape);
    let seam = rewind_seam(tape);
    let unit = max(1.0, size.y / 1080.0);
    let tick = floor(age * 30.0);
    let row = floor(pixel.y / (2.0 * unit));
    let grain = rewind_hash(vec2<f32>(floor(pixel.x / (2.0 * unit)), row + tick * 97.0));
    let streak = rewind_hash(vec2<f32>(floor(pixel.x / (28.0 * unit)) + tick, row));
    let dropout = step(0.92, streak) * tracking;
    // The web rig's base grain is lighter here: large lit volumes (smoke)
    // would otherwise read as broadcast snow rather than tape.
    let snow = 0.55 * (grain * 0.05 + tracking * grain * 0.85 + seam * 0.55 + dropout * 0.5);
    let scan = (1.0 - step(unit, pixel.y % (3.0 * unit))) * 0.055;
    return vec2<f32>(snow, tracking * 0.16 + scan) * amount;
}
