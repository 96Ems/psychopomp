// Binding-free deterministic 3D value noise and four-octave fBM.
// Coordinates are caller-owned: scale, advect, and domain-warp before sampling.
fn fx_hash2(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

fn fx_hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn fx_noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(mix(fx_hash3(i), fx_hash3(i + vec3<f32>(1, 0, 0)), u.x),
            mix(fx_hash3(i + vec3<f32>(0, 1, 0)), fx_hash3(i + vec3<f32>(1, 1, 0)), u.x), u.y),
        mix(mix(fx_hash3(i + vec3<f32>(0, 0, 1)), fx_hash3(i + vec3<f32>(1, 0, 1)), u.x),
            mix(fx_hash3(i + vec3<f32>(0, 1, 1)), fx_hash3(i + vec3<f32>(1, 1, 1)), u.x), u.y), u.z);
}

fn fx_fbm3(p: vec3<f32>) -> f32 {
    var q = p;
    var sum = 0.0;
    var amplitude = 0.57;
    for (var octave = 0; octave < 4; octave++) {
        sum += fx_noise3(q) * amplitude;
        q = q.yzx * 2.03 + vec3<f32>(7.1, 3.7, 13.2);
        amplitude *= 0.47;
    }
    return sum;
}
