// Binding-free scan line over a rounded rectangle; no dependencies. `p` is a
// pixel's offset from the rectangle's center, `line` the scan line's offset
// along y, and `heading` +1 sweeping down or -1 up. Returns weights for the
// line (which overhangs the sides a little), its wake fading behind it inside
// the panel, and the rim it lights where it crosses the border.
fn scan_box(p: vec2<f32>, half: vec2<f32>, corner: f32) -> f32 {
    let r = min(corner, min(half.x, half.y));
    let q = abs(p) - half + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

fn scan_light(
    p: vec2<f32>,
    half: vec2<f32>,
    corner: f32,
    line: f32,
    heading: f32,
    wake: f32,
    width: f32,
    blur: f32,
) -> vec3<f32> {
    let d = scan_box(p, half, corner);
    let inside = clamp(0.5 - d / (0.9 + blur), 0.0, 1.0);
    let ahead = (p.y - line) * heading;
    let soft = width + blur;
    let beam = exp(-ahead * ahead / (2.0 * soft * soft));
    let overhang = exp(-max(d, 0.0) / 9.0) * select(0.0, 1.0, abs(p.y) <= half.y);
    let line_light = beam * max(inside, overhang);
    let trail = select(0.0, exp(ahead / max(wake, 1.0)), ahead <= 0.0) * inside;
    let along = ahead / (soft * 5.0);
    let rim = exp(-d * d / 3.0) * exp(-along * along);
    return vec3<f32>(line_light, trail, rim);
}
