// Binding-free reverse-scan field. Local age in seconds; UV and target size
// supplied by the compositor. Returns pixel displacement xy, RGB split, energy.
// Smooth age and spatial windows preserve arbitrary-time shutter sampling.
fn rewind_field(uv: vec2<f32>, size: vec2<f32>, age: f32) -> vec4<f32> {
    if age <= 0.0 || age >= 1.4 { return vec4<f32>(0.0); }
    let envelope = smoothstep(0.0, 0.12, age) * (1.0 - smoothstep(0.95, 1.4, age));
    // Protect the header and footer; the body is pulled backwards in scan bands.
    let body = smoothstep(0.12, 0.23, uv.y) * (1.0 - smoothstep(0.79, 0.91, uv.y));
    let phase = (uv.y + age * 0.85) * 12.5663706;
    let band = pow(0.5 + 0.5 * cos(phase), 24.0);
    let energy = envelope * body;
    let weave = sin(uv.y * 95.0 + age * 32.0);
    let shift = vec2<f32>((-10.0 + 8.0 * weave) * band, -1.5 * band) * energy;
    return vec4<f32>(shift * size / vec2<f32>(1920.0, 1080.0),
        (0.7 + 3.0 * band) * energy * size.x / 1920.0, band * energy);
}
