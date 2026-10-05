// Requires noise.wgsl. A forcefield bubble seen as a unit sphere: `q` is a
// pixel's offset from its center over the projected radius `radius_px`.
// Hexagonal cells are laid out stereographically, so they crowd toward the
// rim as on a real sphere; a Fresnel rim; a faint flowing shimmer; cells that
// switch on in seeded order while it is raised (`up`, 0..1); and ripples from
// up to four contacts, each (direction xyz scaled by strength, with the
// viewer toward -z, age in seconds; a negative age for none). Returns
// additive HDR light.
// psychopomp::effects::shield owns the matching cell and flare curves.

fn shield_cell(up: f32, seed: f32) -> f32 {
    return smoothstep(0.0, 1.0, (clamp(up, 0.0, 1.0) * 1.3 - seed) / 0.3);
}

// psychopomp::effects::surface::impact's light: a contact flash and a
// damped wave travelling 3.6 radians a second over the sphere.
fn shield_ripple(age: f32, angle: f32) -> f32 {
    if age < 0.0 || age >= 1.4 {
        return 0.0;
    }
    let attack = 1.0 - exp(-age / 0.025);
    let decay = exp(-age / 0.48) * (1.0 - smoothstep(0.0, 1.0, (age - 1.0) / 0.4));
    let band = (angle - 3.6 * age) / 0.22;
    let near = angle / 0.36;
    let wave = exp(-band * band) * attack * decay;
    let contact = exp(-near * near) * attack * exp(-age / 0.12);
    return 2.2 * wave + 1.2 * contact;
}

// The nearest cell of a unit hexagonal grid: (offset from its center, its
// center in grid units).
fn shield_hex(p: vec2<f32>) -> vec4<f32> {
    let s = vec2<f32>(1.0, 1.7320508);
    let a = floor(p / s) + 0.5;
    let b = floor((p - vec2<f32>(0.5, 1.0)) / s) + 0.5;
    let ha = p - a * s;
    let hb = p - (b + 0.5) * s;
    if dot(ha, ha) < dot(hb, hb) {
        return vec4<f32>(ha, a * s);
    }
    return vec4<f32>(hb, (b + 0.5) * s);
}

// A contact's direction is scaled by its strength.
fn shield_contact(normal: vec3<f32>, contact: vec4<f32>) -> f32 {
    let strength = length(contact.xyz);
    if contact.w < 0.0 || strength <= 0.0 {
        return 0.0;
    }
    let toward = contact.xyz / strength;
    return shield_ripple(contact.w, acos(clamp(dot(normal, toward), -1.0, 1.0))) * strength;
}

fn shield_bubble(
    q: vec2<f32>,
    radius_px: f32,
    up: f32,
    time: f32,
    blur: f32,
    tone: vec3<f32>,
    c0: vec4<f32>,
    c1: vec4<f32>,
    c2: vec4<f32>,
    c3: vec4<f32>,
) -> vec3<f32> {
    let r2 = dot(q, q);
    let r = sqrt(r2);
    let pixel = 1.0 / max(radius_px, 1.0);
    let inside = clamp((1.0 - r) / (pixel * (1.0 + blur)) + 0.5, 0.0, 1.0);
    let raised = clamp(up, 0.0, 1.0);
    // A thin outer halo where the sphere's limb glows.
    let limb = exp(-max(r - 1.0, 0.0) * radius_px / 2.5) * (1.0 - inside) * raised;
    if inside <= 0.0 {
        return tone * (0.3 * limb);
    }
    let z = sqrt(max(1.0 - r2, 0.0));
    let normal = vec3<f32>(q, -z);
    let fresnel = pow(1.0 - z, 3.0);
    // Cells about 22 px across at the center, foreshortened toward the limb.
    let cells = radius_px / 11.0;
    let p = q / (1.0 + z) * cells;
    let hex = shield_hex(p);
    let k = abs(hex.xy);
    let edge = 0.5 - max(dot(k, vec2<f32>(0.5, 0.8660254)), k.x);
    let present = shield_cell(up, fx_hash2(hex.zw * 1.37 + vec2<f32>(0.71, 3.3)));
    let line = 1.0 - smoothstep(0.0, 0.05 + 0.12 * (1.0 - z), edge);
    // The cell's own point on the sphere: inverse stereographic projection.
    let theta = 2.0 * atan(length(hex.zw) / cells);
    let toward = hex.zw / max(length(hex.zw), 1.0e-5);
    let center = vec3<f32>(toward * sin(theta), -cos(theta));
    let ripple_cell = shield_contact(center, c0) + shield_contact(center, c1)
        + shield_contact(center, c2) + shield_contact(center, c3);
    let ripple = shield_contact(normal, c0) + shield_contact(normal, c1)
        + shield_contact(normal, c2) + shield_contact(normal, c3);
    let shimmer = 0.55 + 0.45 * fx_noise3(vec3<f32>(p * 0.35, time * 0.3));
    // A cell flashes as it switches on.
    let flash = present * (1.0 - present) * 4.0;
    let lines = line * present * (0.022 * shimmer + 0.5 * ripple + 0.5 * flash);
    let fill = present * (0.14 * ripple_cell + 0.08 * flash);
    let light = tone * (lines + fill + fresnel * 0.16 * raised + 0.3 * limb)
        + vec3<f32>(0.9, 0.95, 1.0) * (line * present * 0.35 * ripple * ripple);
    return light * inside + tone * (0.2 * limb);
}
