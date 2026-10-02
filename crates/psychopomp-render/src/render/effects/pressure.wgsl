// Binding-free pressure displacement. Delta is in screen pixels; scale is the
// projection factor; age is seconds. Returns (pixel displacement x/y, intensity).
// The caller chooses textures, coordinate scaling, and the emitted ring color.
fn pressure_wave(delta: vec2<f32>, scale: f32, age: f32) -> vec3<f32> {
    let distance = length(delta) / scale;
    let radial = delta / max(length(delta), 1.0);
    let t = max(age - 0.12, 0.0);
    let pinch = sin(clamp(age / 0.12, 0.0, 1.0) * 3.14159265) * exp(-distance * distance / 70000.0);
    let front = 55.0 + 760.0 * pow(t, 0.75);
    let band = (distance - front) / (20.0 + 24.0 * t);
    let pressure = exp(-band * band) * exp(-t * 1.8) * smoothstep(0.0, 0.05, t);
    let displacement = 15.0 * pinch + 25.0 * band * pressure;
    return vec3<f32>(radial * displacement * scale, pressure);
}
