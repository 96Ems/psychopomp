//! Lightning: a discharge clock of strobing return strokes, seeded jagged
//! bolts with branches, surface crackle, and contact sparks.
//!
//! Real lightning is not a line that slides: a dim stepped leader feels its way
//! out, the return stroke flashes the whole channel white for a frame or two,
//! and further strokes re-light it a few frames apart before the ionized
//! channel cools. Every pose here is a pure function of a clock and integer
//! seeds, so any frame renders identically in any order and a reversed clock
//! replays the strike backwards. Positions are screen pixels; callers project.
use crate::math::{
    Vec2,
    dynamics::ballistic,
    lerp,
    random::{hash, smooth_noise},
    smoothstep, vec3,
};

/// The stepped leader feels its way out from the source for this long; the
/// first return stroke (contact) follows.
pub const LEADER: f32 = 0.075;
/// The leader advances in discrete steps this long: one 60 fps frame.
pub const LEADER_STEP: f32 = 1.0 / 60.0;
/// A discharge has one to this many return strokes.
pub const MAX_STRIKES: u32 = 8;
/// A stroke's channel holds white-hot this long, then decays.
const HOLD: f32 = 0.014;
const CORE_DECAY: f32 = 0.03;
const AFTERGLOW_DECAY: f32 = 0.065;
/// A stroke's channel and sparks are dark this long after it struck.
pub const AFTERGLOW: f32 = 0.6;
/// Sparks thrown at a contact live at most this long.
pub const SPARK_LIFE: f32 = 0.55;
/// Surface crackle and sustained arcs re-strike this many times a second.
pub const CRACKLE_RATE: f32 = 30.0;
pub const HUM_RATE: f32 = 24.0;
/// The most crackle arcs alive on one outline at full charge.
pub const LANES: u32 = 9;

/// One discharge: a leader, then `strikes` return strokes at seeded intervals
/// of 45 to 110 ms, all on one clock (seconds since the leader set out).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Discharge {
    pub strikes: u32,
    pub seed: u32,
}

impl Discharge {
    pub fn new(strikes: u32, seed: u32) -> Self {
        Self {
            strikes: strikes.clamp(1, MAX_STRIKES),
            seed,
        }
    }

    /// When return stroke `k` connects, in seconds of discharge age.
    pub fn strike_time(self, k: u32) -> f32 {
        LEADER
            + (0..k)
                .map(|i| 0.045 + 0.065 * hash(self.seed, 101 + i))
                .sum::<f32>()
    }

    /// Every stroke, spark, and glow has finished by this age.
    pub fn lifetime(self) -> f32 {
        self.strike_time(self.strikes - 1) + AFTERGLOW
    }

    /// Progress of the leader's tip from source (0) toward contact, in
    /// whole frame steps, while it is still feeling its way out.
    pub fn leader(self, age: f32) -> Option<f32> {
        (0.0..LEADER).contains(&age).then(|| {
            let steps = (LEADER / LEADER_STEP).ceil();
            (((age / LEADER_STEP).floor() + 1.0) / (steps + 1.0)).min(0.95)
        })
    }

    /// The latest return stroke at `age`, and seconds since it struck. The
    /// index is a step function of age: it never slides between strokes.
    pub fn stroke(self, age: f32) -> Option<(u32, f32)> {
        if !(LEADER..self.lifetime()).contains(&age) {
            return None;
        }
        let k = (0..self.strikes)
            .rev()
            .find(|&k| self.strike_time(k) <= age)
            .unwrap_or(0);
        Some((k, age - self.strike_time(k)))
    }

    /// Every stroke still glowing at `age`, oldest first, with seconds since it struck.
    pub fn glowing(self, age: f32) -> impl Iterator<Item = (u32, f32)> {
        (0..self.strikes).filter_map(move |k| {
            let since = age - self.strike_time(k);
            (0.0..AFTERGLOW).contains(&since).then_some((k, since))
        })
    }

    /// The first stroke is the brightest; re-strikes carry 55 to 95 % of it.
    pub fn strength(self, k: u32) -> f32 {
        if k == 0 {
            1.0
        } else {
            0.55 + 0.4 * hash(self.seed, 211 + k)
        }
    }

    /// White-hot channel brightness of stroke `k`, `since` seconds after it
    /// struck: held for a frame, then a fast exponential decay to exact rest.
    pub fn core(self, k: u32, since: f32) -> f32 {
        if !(0.0..AFTERGLOW).contains(&since) {
            return 0.0;
        }
        let decay = if since < HOLD {
            1.0
        } else {
            (-(since - HOLD) / CORE_DECAY).exp()
        };
        self.strength(k) * decay * rest(since)
    }

    /// The cooling, tone-colored glow the channel keeps after a stroke.
    pub fn afterglow(self, k: u32, since: f32) -> f32 {
        if !(0.0..AFTERGLOW).contains(&since) {
            return 0.0;
        }
        0.45 * self.strength(k) * (-since / AFTERGLOW_DECAY).exp() * rest(since)
    }

    /// How brightly the whole discharge lights its surroundings at `age`.
    pub fn light(self, age: f32) -> f32 {
        if let Some(progress) = self.leader(age) {
            return 0.25 + 0.2 * progress;
        }
        self.glowing(age)
            .map(|(k, since)| self.core(k, since) + 0.6 * self.afterglow(k, since))
            .fold(0.0, f32::max)
    }

    /// The path roll of stroke `k`: the discharge's coarse channel persists,
    /// each stroke re-rolls the fine detail and branches.
    pub fn roll(self, k: u32) -> Roll {
        Roll {
            shape: self.seed,
            strike: hash(self.seed, 401 + k).to_bits(),
            writhe: 0.0,
        }
    }
}

/// Fades a glow to exact rest at [`AFTERGLOW`].
fn rest(since: f32) -> f32 {
    1.0 - smoothstep((since - AFTERGLOW * 0.5) / (AFTERGLOW * 0.5))
}

/// The seeds of one bolt path. `shape` fixes its coarse channel; `strike`
/// re-rolls the detail; `writhe` slides the coarse channel continuously (a
/// phase, for sustained arcs; zero for a strike).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Roll {
    pub shape: u32,
    pub strike: u32,
    pub writhe: f32,
}

/// How much each coarse level of a path follows its stroke rather than its
/// discharge: the overall channel persists while the kinks re-roll.
const SWAY: [f32; 3] = [0.25, 0.4, 0.6];
/// Displacement per subdivision level, relative to the jag amplitude: broad
/// bends stay gentle while fine kinks grow sharper, as in real channels.
const ROUGHNESS: [f32; 6] = [0.55, 0.8, 1.0, 1.1, 1.2, 1.25];

impl Roll {
    /// A deviate in -1..1 for subdivision `level`, segment `index`, pushed
    /// toward its extremes so kinks read as sharp turns.
    fn deviate(self, level: u32, index: u32, salt: u32) -> f32 {
        let key = index.wrapping_mul(0x9E37_79B1) ^ level.wrapping_mul(0x85EB_CA77) ^ salt;
        let uniform = hash(self.strike ^ key, 0x51ED) * 2.0 - 1.0;
        let fine = uniform.signum() * uniform.abs().powf(0.7);
        match SWAY.get(level as usize) {
            Some(&sway) => {
                let coarse = smooth_noise(self.writhe + hash(key, 0x2C1B) * 7.0, self.shape ^ key);
                lerp(coarse, fine, sway)
            }
            None => fine,
        }
    }
}

/// Midpoint-displace the path `base(0..1)` into `2^levels + 1` jagged points.
/// Offsets are perpendicular to the base (plus a little along it), scaled
/// by `amplitude` times each subdivided segment's length, so the jaggedness
/// is self-similar and independent of on-screen size. The ends stay exact.
pub fn jag(base: impl Fn(f32) -> Vec2, levels: u32, roll: Roll, amplitude: f32) -> Vec<Vec2> {
    let n = 1_usize << levels.min(10);
    let points = (0..=n)
        .map(|i| base(i as f32 / n as f32))
        .collect::<Vec<_>>();
    let tangent =
        |i: usize| (points[(i + 1).min(n)] - points[i.saturating_sub(1)]).normalize_or(Vec2::X);
    let mut across = vec![0.0_f32; n + 1];
    let mut along = vec![0.0_f32; n + 1];
    for level in 0..levels.min(10) {
        let stride = n >> level;
        let half = stride / 2;
        for segment in 0..(n / stride) {
            let (start, end) = (segment * stride, segment * stride + stride);
            let mid = start + half;
            let length = points[start].distance(points[end]);
            let reach = amplitude * length * ROUGHNESS[(level as usize).min(5)];
            let index = segment as u32;
            across[mid] =
                (across[start] + across[end]) * 0.5 + roll.deviate(level, index, 1) * reach;
            along[mid] =
                (along[start] + along[end]) * 0.5 + roll.deviate(level, index, 2) * reach * 0.22;
        }
    }
    (0..=n)
        .map(|i| {
            let t = tangent(i);
            points[i] + t.perp() * across[i] + t * along[i]
        })
        .collect()
}

/// A side channel forking off a bolt.
#[derive(Clone, Debug, PartialEq)]
pub struct Branch {
    pub points: Vec<Vec2>,
    /// Where it forks, as a fraction of the main channel's length (a parent
    /// branch's, for a sub-branch), so a growing leader reveals it in order.
    pub fork: f32,
    /// Brightness relative to the main channel.
    pub energy: f32,
    /// 1 for a branch off the main channel, 2 for one off a branch.
    pub depth: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bolt {
    pub main: Vec<Vec2>,
    pub branches: Vec<Branch>,
}

/// A short jagged twig from `start` along `direction` (a unit vector).
pub fn twig(start: Vec2, direction: Vec2, length: f32, levels: u32, roll: Roll) -> Vec<Vec2> {
    jag(|t| start + direction * (length * t), levels, roll, 0.34)
}

/// A jagged bolt from `from` to `to` with about `branching * 6.5` forks,
/// mostly early along the channel, leaning toward the receiver and
/// shortening as they go.
pub fn bolt(from: Vec2, to: Vec2, roll: Roll, branching: f32) -> Bolt {
    let main = jag(|t| from.lerp(to, t), 6, roll, 0.3);
    let path = crate::math::curve::Polyline::new(main.clone());
    let total = path.length().max(1.0);
    let heading = (to - from).normalize_or(Vec2::X);
    let count = (branching.clamp(0.0, 1.5) * (4.0 + 5.0 * hash(roll.strike, 301))).round() as u32;
    let mut branches = Vec::new();
    for b in 0..count {
        let salt = 310 + b * 13;
        let fork = 0.06 + 0.74 * hash(roll.strike, salt).powf(1.3);
        let local = (path.at(fork + 0.02) - path.at(fork - 0.02)).normalize_or(heading);
        let side = if hash(roll.strike, salt + 1) < 0.5 {
            -1.0
        } else {
            1.0
        };
        let turn = side * (0.35 + 0.5 * hash(roll.strike, salt + 2));
        let direction = Vec2::from_angle(turn).rotate(local.lerp(heading, 0.4).normalize_or(local));
        let length = total * (0.12 + 0.25 * hash(roll.strike, salt + 3)) * (1.0 - 0.5 * fork);
        let start = path.at(fork);
        let fork_roll = Roll {
            shape: hash(roll.shape, salt).to_bits(),
            strike: hash(roll.strike, salt + 4).to_bits(),
            writhe: roll.writhe,
        };
        let points = twig(start, direction, length, 4, fork_roll);
        let energy = 0.4 + 0.25 * hash(roll.strike, salt + 5);
        if hash(roll.strike, salt + 6) < 0.45 {
            let along = crate::math::curve::Polyline::new(points.clone());
            let sub_fork = 0.3 + 0.4 * hash(roll.strike, salt + 7);
            let sub_start = along.at(sub_fork);
            let sub_direction = Vec2::from_angle(-side * (0.4 + 0.4 * hash(roll.strike, salt + 8)))
                .rotate(direction);
            let sub_length = length * (0.35 + 0.25 * hash(roll.strike, salt + 9));
            let sub = Roll {
                strike: hash(roll.strike, salt + 10).to_bits(),
                ..fork_roll
            };
            branches.push(Branch {
                points: twig(sub_start, sub_direction, sub_length, 3, sub),
                fork: fork + sub_fork * length / total,
                energy: energy * 0.55,
                depth: 2,
            });
        }
        branches.push(Branch {
            points,
            fork,
            energy,
            depth: 1,
        });
    }
    Bolt { main, branches }
}

/// One short arc crawling along an outline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Creep {
    /// Center and length along the closed outline, as fractions of its perimeter.
    pub center: f32,
    pub span: f32,
    pub roll: Roll,
    pub energy: f32,
}

/// The crackle on a charged outline at scene `time`: short arcs that crawl
/// along it, re-striking [`CRACKLE_RATE`] times a second with fresh detail,
/// some slots dark so it strobes. More lanes join as `intensity` rises (0..1,
/// overdriven to 1.5); zero is exactly quiet.
pub fn crackle(time: f32, intensity: f32, seed: u32) -> Vec<Creep> {
    let intensity = intensity.clamp(0.0, 1.5);
    if intensity <= 0.0 {
        return Vec::new();
    }
    let slot = (time * CRACKLE_RATE).floor() as i64 as u32;
    (0..LANES)
        .filter_map(|lane| {
            let salt = seed ^ lane.wrapping_mul(0x632B_E5AB);
            // Lanes join in a seeded order as the charge builds.
            if hash(salt, 1) * 0.9 + 0.05 > intensity {
                return None;
            }
            let life = 0.22 + 0.35 * hash(salt, 2);
            let phase = (time + hash(salt, 3) * life) / life;
            let generation = phase.floor() as i64 as u32;
            let age = phase - phase.floor();
            let generation_salt = salt ^ generation.wrapping_mul(0x2545_F491);
            let flicker = hash(slot.wrapping_mul(LANES).wrapping_add(lane), seed ^ 0x5BD1);
            if flicker > 0.42 + 0.45 * intensity.min(1.0) {
                return None;
            }
            let direction = if hash(generation_salt, 4) < 0.5 {
                -1.0
            } else {
                1.0
            };
            let speed = direction * (0.05 + 0.12 * hash(generation_salt, 5));
            let envelope = smoothstep(age / 0.12) * (1.0 - smoothstep((age - 0.8) / 0.2));
            let energy = intensity * envelope * (0.55 + 0.45 * hash(slot ^ salt, 6));
            (energy > 0.01).then(|| Creep {
                center: (hash(generation_salt, 7) + speed * age * life).rem_euclid(1.0),
                span: (0.035 + 0.05 * hash(generation_salt, 8))
                    * (0.75 + 0.25 * intensity.min(1.0)),
                roll: Roll {
                    shape: hash(generation_salt, 9).to_bits(),
                    strike: hash(slot ^ salt, 10).to_bits(),
                    writhe: 0.0,
                },
                energy,
            })
        })
        .collect()
}

/// A sustained arc at scene `time`: it re-strikes [`HUM_RATE`] times a
/// second with fresh detail while its coarse channel writhes continuously,
/// and now and then a slot drops out. `None` when quiet.
pub fn hum(time: f32, intensity: f32, seed: u32) -> Option<(Roll, f32)> {
    let intensity = intensity.clamp(0.0, 1.5);
    if intensity <= 0.0 {
        return None;
    }
    let slot = (time * HUM_RATE).floor() as i64 as u32;
    let flicker = hash(slot, seed ^ 0x7F4A);
    if flicker < 0.12 * (1.2 - intensity.min(1.0)) {
        return None;
    }
    Some((
        Roll {
            shape: seed,
            strike: hash(slot, seed ^ 0x1B87).to_bits(),
            writhe: time * 1.6,
        },
        intensity * (0.6 + 0.4 * flicker),
    ))
}

/// One hot fragment thrown from a contact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spark {
    /// Screen offsets from the contact at unit scale, at the head and a
    /// moment earlier (for a short streak).
    pub offset: Vec2,
    pub tail: Vec2,
    /// 1 white-hot at birth, cooling to 0.
    pub heat: f32,
    pub opacity: f32,
}

/// Spark `index` of stroke `k`, `since` seconds after it struck, thrown into
/// the half-space around `normal` (a unit vector out of the struck surface):
/// fast, then dragged and pulled down by gravity, cooling as it falls.
pub fn spark(seed: u32, k: u32, index: u32, normal: Vec2, since: f32) -> Option<Spark> {
    let salt = seed ^ k.wrapping_mul(0x27D4_EB2F) ^ index.wrapping_mul(0x1656_67B1);
    let life = 0.2 + (SPARK_LIFE - 0.2) * hash(salt, 1);
    if !(0.0..life).contains(&since) {
        return None;
    }
    let angle = (hash(salt, 2) * 2.0 - 1.0) * 1.25;
    let direction = Vec2::from_angle(angle).rotate(normal.normalize_or(Vec2::NEG_Y));
    let velocity = vec3(direction.x, direction.y, 0.0) * (260.0 + 560.0 * hash(salt, 3));
    let gravity = vec3(0.0, 1300.0, 0.0);
    let drag = 2.5 + 2.0 * hash(salt, 4);
    let at = |t: f32| ballistic(velocity, gravity, drag, t).truncate();
    let t = since / life;
    Some(Spark {
        offset: at(since),
        tail: at((since - 0.018).max(0.0)),
        heat: (1.0 - t).powi(2),
        opacity: 1.0 - smoothstep((t - 0.6) / 0.4),
    })
}

/// How many sparks stroke `k` throws: a shower for the first, a few after.
pub fn spark_count(k: u32) -> u32 {
    if k == 0 { 9 } else { 4 }
}

/// A point on a unit circle's arc, for crackle around round outlines.
pub fn on_circle(center: Vec2, radius: f32, fraction: f32) -> Vec2 {
    center + Vec2::from_angle(fraction * std::f32::consts::TAU) * radius
}

/// The deterministic seed a strike at `at_nanos` rolls, so repeated zaps of
/// one bolt never repeat a path. Exactly representable as an f32 channel.
pub fn seed_for(at_nanos: u64) -> u32 {
    1 + (hash((at_nanos / 1_000_000) as u32, 0xB017) * 65_536.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::vec2;

    fn discharge() -> Discharge {
        Discharge::new(3, 17)
    }

    #[test]
    fn bolts_are_deterministic_for_a_seed_and_keep_exact_ends() {
        let roll = discharge().roll(0);
        let (from, to) = (vec2(100.0, 200.0), vec2(900.0, 420.0));
        let a = bolt(from, to, roll, 0.8);
        assert_eq!(a, bolt(from, to, roll, 0.8));
        assert_eq!(a.main.len(), 65);
        assert_eq!((a.main[0], a.main[64]), (from, to));
        assert!(!a.branches.is_empty());
        for branch in &a.branches {
            assert!(branch.points.iter().all(|p| p.is_finite()));
            assert!((0.0..1.2).contains(&branch.fork));
        }
        let detour = a
            .main
            .iter()
            .map(|p| crate::math::shapes::segment_distance(*p, from, to));
        let widest = detour.fold(0.0, f32::max);
        assert!(widest > 8.0 && widest < 0.3 * from.distance(to), "{widest}");
    }

    #[test]
    fn each_stroke_rerolls_detail_around_a_persisting_channel() {
        let discharge = discharge();
        let (from, to) = (vec2(0.0, 0.0), vec2(800.0, 0.0));
        let paths = (0..3)
            .map(|k| bolt(from, to, discharge.roll(k), 0.0).main)
            .collect::<Vec<_>>();
        assert_ne!(paths[0], paths[1]);
        assert_ne!(paths[1], paths[2]);
        // The coarse channel persists: strokes sit nearer each other than a
        // bolt of another discharge.
        let other = bolt(from, to, Discharge::new(3, 9001).roll(0), 0.0).main;
        let spread = |a: &[Vec2], b: &[Vec2]| {
            a.iter().zip(b).map(|(p, q)| p.distance(*q)).sum::<f32>() / a.len() as f32
        };
        assert!(spread(&paths[0], &paths[1]) < spread(&paths[0], &other));
    }

    #[test]
    fn strikes_are_quantized_steps_on_one_clock() {
        let discharge = discharge();
        assert_eq!(discharge.stroke(LEADER - 1e-4), None);
        assert_eq!(discharge.stroke(LEADER), Some((0, 0.0)));
        let times = (0..3).map(|k| discharge.strike_time(k)).collect::<Vec<_>>();
        assert!(
            times
                .windows(2)
                .all(|t| t[1] - t[0] >= 0.045 && t[1] - t[0] <= 0.11)
        );
        // Constant between strikes, and it steps exactly at each strike time.
        for k in 0..3 {
            let next = times.get(k + 1).copied().unwrap_or(discharge.lifetime());
            for f in [0.0, 0.3, 0.7, 0.999] {
                let age = lerp(times[k], next, f);
                assert_eq!(discharge.stroke(age).map(|s| s.0), Some(k as u32), "{age}");
            }
        }
        // The leader steps a whole frame at a time.
        let a = discharge.leader(0.001).unwrap();
        assert_eq!(discharge.leader(LEADER_STEP * 0.9), Some(a));
        assert!(discharge.leader(LEADER_STEP * 1.1).unwrap() > a);
        assert_eq!(discharge.leader(LEADER), None);
    }

    #[test]
    fn envelopes_flash_then_reach_exact_rest() {
        let d = discharge();
        assert_eq!(d.core(0, 0.0), 1.0);
        assert_eq!(d.core(0, HOLD * 0.5), 1.0);
        assert!(d.core(0, 0.05) < 0.4);
        for k in 0..3 {
            assert_eq!(d.core(k, AFTERGLOW), 0.0);
            assert_eq!(d.afterglow(k, AFTERGLOW), 0.0);
            assert_eq!(d.core(k, -0.01), 0.0);
        }
        assert_eq!(d.light(d.lifetime()), 0.0);
        assert_eq!(d.light(-1.0), 0.0);
        assert!(d.light(LEADER) > d.light(LEADER - 0.01));
        for index in 0..spark_count(0) {
            assert!(spark(17, 0, index, Vec2::NEG_Y, SPARK_LIFE).is_none());
        }
    }

    #[test]
    fn every_pose_samples_out_of_order() {
        let d = discharge();
        let ages = [0.31, 0.02, 0.5, 0.09, 0.31, 0.2, 0.02];
        let first = ages.map(|age| (d.stroke(age), d.leader(age), d.light(age)));
        let again = ages.map(|age| (d.stroke(age), d.leader(age), d.light(age)));
        assert_eq!(first, again);
        let s = spark(5, 1, 2, Vec2::X, 0.1);
        assert_eq!(
            spark(5, 1, 2, Vec2::X, 0.3).is_some(),
            spark(5, 1, 2, Vec2::X, 0.3).is_some()
        );
        assert_eq!(spark(5, 1, 2, Vec2::X, 0.1), s);
        let falling = spark(5, 1, 2, Vec2::X, 0.19).unwrap();
        assert!(falling.offset.x > 0.0, "thrown along the normal");
    }

    #[test]
    fn crackle_is_quiet_at_rest_strobes_in_slots_and_grows_with_charge() {
        assert!(crackle(1.0, 0.0, 3).is_empty());
        assert_eq!(hum(1.0, 0.0, 3), None);
        let within = |t: f32| {
            crackle(t, 1.0, 3)
                .iter()
                .map(|c| c.roll.strike)
                .collect::<Vec<_>>()
        };
        let slot = 31.0 / CRACKLE_RATE;
        assert_eq!(
            within(slot + 0.001),
            within(slot + 0.03),
            "one slot, one detail"
        );
        assert_ne!(
            within(slot + 0.001),
            within(slot + 1.0 / CRACKLE_RATE + 0.001)
        );
        let count = |intensity| {
            (0..120)
                .map(|i| crackle(i as f32 * 0.05, intensity, 3).len())
                .sum::<usize>()
        };
        assert!(count(1.0) > count(0.3) && count(0.3) > 0);
        for creep in crackle(2.0, 1.0, 3) {
            assert!((0.0..1.0).contains(&creep.center) && creep.span < 0.1);
        }
    }

    #[test]
    fn strike_seeds_are_whole_and_vary() {
        let a = seed_for(1_000_000_000);
        assert_eq!(a as f32 as u32, a);
        assert_ne!(a, seed_for(2_500_000_000));
        assert!(a >= 1);
    }
}
