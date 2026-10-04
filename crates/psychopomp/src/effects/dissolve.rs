//! Dissolve: a card burns away along a noisy front with a hot rim and a
//! scorched band ahead of it, shedding ash that drifts up and cools. One
//! clock owns it: age in seconds, negative or zero intact, the card gone at
//! [`BURN`] and the last ash cold at [`DURATION`]. Easing the clock back
//! rematerializes the card while the ash flies home. `effects/dissolve.wgsl`
//! evaluates the same field per pixel, so flakes leave exactly at the rim.
use crate::math::{
    Vec2,
    dynamics::ballistic,
    random::{hash, lattice_fbm2},
    smoothstep, vec2, vec3,
};

/// Width of the glowing band behind the front, in field units.
pub const EDGE: f32 = 0.035;
/// Noise cells per world pixel: blobs about 40 px across.
pub const GRAIN: f32 = 1.0 / 40.0;
/// The front crosses the whole card in this many seconds.
pub const BURN: f32 = 1.1;
/// The longest-lived flake, released last, has cooled by this age.
pub const DURATION: f32 = BURN + 0.95;
/// Ash particles shed by one card.
pub const ASH: u32 = 160;

/// The field the front sweeps, 0..1, at `local` world pixels from a card's
/// center with half size `half`: noise, leaning from the top left corner.
pub fn field(local: Vec2, half: Vec2, seed: u32) -> f32 {
    let unit = local / half.max(Vec2::ONE);
    let sweep = ((unit.x * 0.75 + unit.y * 0.35) / 1.1 * 0.5 + 0.5).clamp(0.0, 1.0);
    let noise = lattice_fbm2(local.x * GRAIN, local.y * GRAIN, seed);
    let noise = ((noise - 0.2) / 0.6).clamp(0.0, 1.0);
    (0.55 * sweep + 0.45 * noise).clamp(0.0, 1.0)
}

/// Where the front stands at `age`: field values below it are gone. At
/// zero the band lies below every field value; at [`BURN`] above all of them.
pub fn front(age: f32) -> f32 {
    (age / BURN).clamp(0.0, 1.0) * (1.0 + 2.0 * EDGE) - EDGE
}

/// The age at which the front passes a field value.
pub fn passes(value: f32) -> f32 {
    BURN * (value + EDGE) / (1.0 + 2.0 * EDGE)
}

/// One ash flake, relative to the card's center in world pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ash {
    pub local: Vec2,
    /// 1 glowing as it leaves the rim, cooling to 0.
    pub heat: f32,
    pub opacity: f32,
    pub radius: f32,
}

/// Where flake `index` of a card with half size `half` sits in the card.
pub fn home(index: u32, seed: u32, half: Vec2) -> Vec2 {
    let salt = seed ^ 0xA5A5_0F0F;
    vec2(
        hash(index, salt) * 2.0 - 1.0,
        hash(index, salt ^ 1) * 2.0 - 1.0,
    ) * half
}

/// Flake `index` at `age`, once the front has passed its home.
pub fn ash(index: u32, seed: u32, half: Vec2, age: f32) -> Option<Ash> {
    let salt = seed ^ 0xA5A5_0F0F;
    let home = home(index, seed, half);
    let since = age - passes(field(home, half, seed));
    let life = 0.35 + 0.6 * hash(index, salt ^ 2);
    if !(0.0..life).contains(&since) {
        return None;
    }
    let velocity = vec3(
        (hash(index, salt ^ 3) - 0.35) * 70.0,
        -(25.0 + 80.0 * hash(index, salt ^ 4)),
        0.0,
    );
    // Buoyant ash: lifted and blown a little right, slowed by the air.
    let lift = vec3(24.0, -110.0, 0.0);
    let t = since / life;
    Some(Ash {
        local: home + ballistic(velocity, lift, 1.4, since).truncate(),
        heat: (1.0 - t).powf(1.6),
        opacity: smoothstep(since / 0.04) * (1.0 - smoothstep((t - 0.55) / 0.45)),
        radius: 0.7 + 0.9 * hash(index, salt ^ 5),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_front_clears_every_field_value_between_its_ends() {
        let half = vec2(160.0, 60.0);
        for i in 0..400 {
            let local = vec2(hash(i, 1) * 2.0 - 1.0, hash(i, 2) * 2.0 - 1.0) * half;
            let value = field(local, half, 7);
            assert!((0.0..=1.0).contains(&value));
            assert!(front(0.0) + EDGE <= value, "intact at 0");
            assert!(front(BURN) > value, "gone once burnt");
            assert!((front(passes(value)) - value).abs() < 1e-5);
        }
        assert_eq!(
            field(vec2(10.0, 4.0), half, 7),
            field(vec2(10.0, 4.0), half, 7)
        );
    }

    #[test]
    fn ash_leaves_the_rim_and_returns_when_reversed() {
        let half = vec2(160.0, 60.0);
        assert!(
            (0..ASH).all(|i| ash(i, 7, half, 0.0).is_none()),
            "no ash at rest"
        );
        assert!(
            (0..ASH).all(|i| ash(i, 7, half, DURATION).is_none()),
            "all cold"
        );
        let shed = (0..ASH).filter_map(|i| ash(i, 7, half, 0.6)).count();
        assert!(shed > 20, "{shed}");
        // A flake starts at its home exactly as the front passes it.
        for i in 0..ASH {
            let home = home(i, 7, half);
            let flake = ash(i, 7, half, passes(field(home, half, 7)) + 1e-4).unwrap();
            assert!(flake.local.distance(home) < 0.1);
        }
        assert_eq!(ash(3, 7, half, 0.6), ash(3, 7, half, 0.6));
    }
}
