// Deterministic procedural combustion, not a frame-integrated fluid simulation.
// Domain-warped fBM shapes a rising volume; front-to-back Beer-Lambert
// absorption carries hot emission through cool smoke (GPU Gems 3, chapter 30).
fn burst_hash(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn burst_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(mix(burst_hash(i), burst_hash(i + vec3<f32>(1, 0, 0)), u.x),
            mix(burst_hash(i + vec3<f32>(0, 1, 0)), burst_hash(i + vec3<f32>(1, 1, 0)), u.x), u.y),
        mix(mix(burst_hash(i + vec3<f32>(0, 0, 1)), burst_hash(i + vec3<f32>(1, 0, 1)), u.x),
            mix(burst_hash(i + vec3<f32>(0, 1, 1)), burst_hash(i + vec3<f32>(1, 1, 1)), u.x), u.y), u.z);
}

fn burst_fbm(p: vec3<f32>) -> f32 {
    var q = p;
    var sum = 0.0;
    var amplitude = 0.57;
    for (var octave = 0; octave < 4; octave++) {
        sum += burst_noise(q) * amplitude;
        q = q.yzx * 2.03 + vec3<f32>(7.1, 3.7, 13.2);
        amplitude *= 0.47;
    }
    return sum;
}

fn burst_volume(pixel: vec2<f32>, radius: f32, age: f32) -> vec4<f32> {
    let t = max(age - 0.12, 0.0);
    let ignite = smoothstep(0.0, 0.065, t);
    let fade = 1.0 - smoothstep(2.6, 5.2, age);
    if ignite * fade < 0.001 { return vec4<f32>(0.0); }
    let growth = 0.32 + 1.48 * (1.0 - exp(-t * 3.5)) + 0.10 * t;
    let lift = vec2<f32>(0.06 * t, -0.28 * t);
    let xy = pixel / max(radius, 1.0) - lift;
    if length(xy / vec2<f32>(1.12, 1.0)) > growth * 1.3 { return vec4<f32>(0.0); }
    let step_size = growth * 2.5 / 28.0;
    var transmittance = 1.0;
    var light = vec3<f32>(0.0);
    let cooling = exp(-t * 1.7);
    // Fixed quadrature avoids temporal noise crawling through the smoke.
    for (var sample = 0; sample < 28; sample++) {
        let z = -growth * 1.25 + (f32(sample) + 0.5) * step_size;
        let p = vec3<f32>(xy, z) / growth;
        let advected = p * 3.2 + vec3<f32>(0.0, t * 0.75, t * 0.16);
        let warp = vec3<f32>(burst_noise(advected + 9.0), burst_noise(advected.yzx + 23.0), burst_noise(advected.zxy + 41.0)) - 0.5;
        let turbulence = burst_fbm(advected + warp * 1.8);
        let lobes = burst_noise(p * 2.6 + vec3<f32>(4.0, 8.0, 1.0));
        let boundary = 0.67 + 0.60 * lobes;
        let density = max(boundary - length(p * vec3<f32>(0.92, 1.0, 1.0)) + (turbulence - 0.48) * 1.05, 0.0);
        let absorb = (1.0 - exp(-density * step_size * 8.0)) * ignite * fade;
        // Cooler opaque folds interrupt the hot interior instead of making an
        // evenly emissive orange ball. Fine noise supplies glowing fissures.
        let heat = cooling * (0.15 + 1.7 * smoothstep(0.36, 0.68, turbulence))
            * (1.0 - smoothstep(0.25, 1.05, length(p)) * 0.72);
        let ember = mix(vec3<f32>(0.24, 0.008, 0.001), vec3<f32>(3.6, 0.38, 0.015), smoothstep(0.10, 0.52, heat));
        let fire = mix(ember, vec3<f32>(7.0, 3.2, 0.8), smoothstep(0.6, 1.1, heat));
        let smoke_key = 0.012 + 0.12 * pow(1.0 - turbulence, 3.0) + 0.026 * max(-p.y, 0.0);
        let smoke = vec3<f32>(0.65, 0.72, 0.83) * smoke_key;
        let emission = fire * smoothstep(0.035, 0.22, heat);
        light += transmittance * absorb * (smoke + emission);
        transmittance *= 1.0 - absorb;
        if transmittance < 0.015 { break; }
    }
    // A very brief ignition core; the turbulent surface remains visible after it.
    let flash = exp(-t * 19.0) * ignite * exp(-dot(xy, xy) * 5.0);
    light += vec3<f32>(7.0, 3.0, 0.9) * flash;
    return vec4<f32>(light, 1.0 - transmittance);
}
