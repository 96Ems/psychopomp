//! A forcefield bubble around an element: faint hexagonal cells that rise in
//! when it is raised, and a ripple that lights them wherever something
//! crosses or strikes it. The ripple is [`super::surface::impact`] on a
//! sphere; `effects/shield.wgsl` mirrors the pieces below per pixel.
use crate::math::smoothstep;

/// A contact's ripple has crossed the bubble and faded by this age.
pub const RIPPLE: f32 = 1.4;

/// Presence of a cell with stable `seed` (0..1) while the bubble is raised
/// to `up` (0..1): cells switch on in seeded order, each over a short ramp.
/// Mirrors `shield_cell` in shield.wgsl.
pub fn cell(up: f32, seed: f32) -> f32 {
    smoothstep((up.clamp(0.0, 1.0) * 1.3 - seed) / 0.3)
}

/// The light a contact casts on nearby rims: instant attack, fast decay,
/// exactly dark at [`RIPPLE`].
pub fn flare(age: f32) -> f32 {
    if !(0.0..RIPPLE).contains(&age) {
        return 0.0;
    }
    let attack = 1.0 - (-age / 0.02).exp();
    attack * (-age / 0.22).exp() * (1.0 - smoothstep((age - 0.9) / 0.5))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_rise_in_order_and_contacts_flare_then_rest() {
        assert_eq!(cell(0.0, 0.0), 0.0);
        assert_eq!(cell(1.0, 0.99), 1.0);
        assert!(cell(0.4, 0.1) > cell(0.4, 0.5));
        assert_eq!(flare(0.0), 0.0);
        assert!(flare(0.05) > 0.5 && flare(0.05) > flare(0.4));
        assert_eq!((flare(RIPPLE), flare(-0.1)), (0.0, 0.0));
    }
}
