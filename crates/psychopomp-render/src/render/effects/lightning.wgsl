// Binding-free plasma optics for lightning; no dependencies. `d` is the
// distance in pixels from a channel's centerline and `half` its core half
// width. A white-hot core, a hot sheath tinting toward the corona color,
// and atmospheric scatter falling off as 1 / (1 + r^2) (Lorentzian), cut
// smoothly to zero at `reach` so the caller's quad bounds it. Returns
// additive premultiplied HDR light: plasma emits and never occludes.
// Paths, strobing, and seeds live in psychopomp::effects::lightning.
fn lightning_channel(
    d: f32,
    half: f32,
    blur: f32,
    core: vec3<f32>,
    corona: vec3<f32>,
    radius: f32,
    reach: f32,
) -> vec3<f32> {
    let edge = d - half;
    let body = clamp(0.5 - edge / (0.9 + blur), 0.0, 1.0);
    let outside = max(edge, 0.0);
    let sheath_width = half * 1.4 + 0.6 + blur;
    let sheath = exp(-outside * outside / (2.0 * sheath_width * sheath_width));
    let r = d / max(radius, 0.5);
    let scatter = (1.0 / (1.0 + r * r)) * (1.0 - smoothstep(reach * 0.5, reach, d));
    return core * body + mix(core, corona, 0.75) * (sheath * 0.2) + corona * (scatter * 0.45);
}
