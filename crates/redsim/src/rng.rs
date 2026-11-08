//! The game's `LegacyRandomSource` (Java's 48-bit LCG, `BitRandomSource`'s draws) and the drops the
//! bridge seeds with it (shulker study, docs/mechanics.md "Drops and the bridge's seeds").
//!
//! redsim does not mirror the LEVEL random (its draws depend on the sandbox's chunk count: 3
//! `nextInt(48)` per block-ticking chunk per tick, piston sound pitches, dispenser slots...). The
//! bridge instead gives each item entity that `Block.popResource` spawns during a tick its own
//! random, reseeded to `seed ^ (0x6d63746173 + n)` (n = the sandbox's n-th such drop), and draws
//! everything from it: the offsets in its cell (x, y, z), the yaw, the x and z drift. [`Drop`]
//! mirrors that: a drop is a function of the sandbox seed, its number and its cell.

use crate::pos::BlockPos;

/// `LegacyRandomSource`: `setSeed`, `next(bits)` and `BitRandomSource`'s `nextInt(bound)`,
/// `nextFloat` (float precision), `nextDouble`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LegacyRandom {
    seed: i64,
}

const MULTIPLIER: i64 = 25_214_903_917;
const MASK: i64 = (1 << 48) - 1;

impl LegacyRandom {
    pub fn new(seed: i64) -> LegacyRandom {
        let mut r = LegacyRandom { seed: 0 };
        r.set_seed(seed);
        r
    }

    /// `setSeed`: `(seed ^ 0x5DEECE66D) & (2^48 - 1)`.
    pub fn set_seed(&mut self, seed: i64) {
        self.seed = (seed ^ MULTIPLIER) & MASK;
    }

    /// `next(bits)`.
    pub fn next(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(11) & MASK;
        (self.seed >> (48 - bits)) as i32
    }

    /// `nextInt(bound)` (bound > 0): a power of two takes the high bits, anything else rejects the
    /// samples that would bias the modulo.
    pub fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "nextInt bound must be positive");
        if bound & (bound - 1) == 0 {
            return ((bound as i64 * self.next(31) as i64) >> 31) as i32;
        }
        loop {
            let sample = self.next(31);
            let modulo = sample % bound;
            if sample.wrapping_sub(modulo).wrapping_add(bound - 1) >= 0 {
                return modulo;
            }
        }
    }

    /// `nextFloat`: `next(24) * 5.9604645E-8F`.
    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 * 5.960_464_5e-8_f32
    }

    /// `nextBoolean`: `next(1) != 0`.
    pub fn next_bool(&mut self) -> bool {
        self.next(1) != 0
    }

    /// `nextDouble`: `((long) next(26) << 27) + next(27)`, times `DOUBLE_MULTIPLIER` (a double
    /// field initialized from the float literal `1.110223E-16F`, i.e. exactly 2^-53; decompilers
    /// print the float literal inline, but the product is a double one).
    pub fn next_double(&mut self) -> f64 {
        let upper = self.next(26) as i64;
        let lower = self.next(27) as i64;
        let combined = (upper << 27) + lower;
        combined as f64 * (1.110_223e-16_f32 as f64)
    }

    /// `Mth.nextDouble(random, min, max)`.
    pub fn next_double_between(&mut self, min: f64, max: f64) -> f64 {
        if min >= max { min } else { self.next_double() * (max - min) + min }
    }

    // ---- the level random's mirror (clk-misc) ----
    /// The internal 48-bit state (`LegacyRandomSource.seed`): what the bridge's `random_state`
    /// returns.
    pub fn state(&self) -> i64 {
        self.seed
    }

    /// `BitRandomSource.nextLong`: `((long) next(32) << 32) + next(32)`.
    pub fn next_long(&mut self) -> i64 {
        let upper = self.next(32) as i64;
        let lower = self.next(32) as i64;
        (upper << 32).wrapping_add(lower)
    }

    /// `RandomSource.triangle(mean, spread)` (doubles): `mean + spread * (nextDouble() - nextDouble())`.
    pub fn triangle(&mut self, mean: f64, spread: f64) -> f64 {
        let a = self.next_double();
        let b = self.next_double();
        mean + spread * (a - b)
    }
}

/// How many LCG steps (`next(bits)` calls) lead from internal state `a` to `b`, within `max`.
pub fn lcg_steps(a: i64, b: i64, max: u32) -> Option<u32> {
    let mut s = a;
    for k in 0..=max {
        if s == b {
            return Some(k);
        }
        s = s.wrapping_mul(MULTIPLIER).wrapping_add(0xB) & MASK;
    }
    None
}

/// A trace check of the level random (`rng` of an exact trace frame against the mirror's
/// state): `None` when equal, else the difference with the draw distance when it is small (which
/// side made extra `next()` calls).
pub fn state_diff(game: i64, ours: i64) -> Option<String> {
    if game == ours {
        return None;
    }
    let why = match (lcg_steps(ours, game, 4096), lcg_steps(game, ours, 4096)) {
        (Some(k), _) => format!("redsim made {k} fewer next() calls"),
        (_, Some(k)) => format!("redsim made {k} more next() calls"),
        _ => "not within 4096 next() calls".to_string(),
    };
    Some(format!("level random: game {game}, redsim {ours} ({why})"))
}

// ---- the stream registry (tickexact, 2026-11-08) ------------------------------------------------
//
// Every random a mob, spawn, weather or fire path draws from is seeded by the bridge from the
// sandbox seed (docs/mechanics.md "The bridge's seeds"; bridge `Exact.java`), and redsim draws the
// same values from the same streams:
//
// * the level random: `LegacyRandom::new(seed)` ([`World::mirror_level_random`], the chunk tick
//   order with it), `randValue` = `(seed ^ 0x72616E64) as i32`, the loot sequences' salt
//   (`stems::loot_salt`);
// * entity n's own random: [`entity_seed`] (n = its id: the n-th entity to enter, from 0), then
//   the constructor's draws in the game's order ([`replay_uuid`], the yaw, the sensors...); the
//   head (and yaw) a spawn path leaves unsnapped holds that replayed yaw, and a cube mob's move
//   control turns to it (180 * yaw / pi) until a goal picks a direction;
// * before it enters, from its construction: [`construct_seed`] (k = the k-th entity constructed
//   in the sandbox), for what `finalizeSpawn` draws from the entity's own random;
// * each brain's shuffling lists (`GateBehavior`/`RunOne`, `TriggerGate`): [`shuffle_seed`];
// * a raid's own random: [`raid_seed`]; the wandering trader spawner's: [`trader_seed`];
// * popResource drops ([`Drop`]) and dispensed items (`World::spawn_dispensed`) keep their own
//   seeding (the shulker and elevator studies).

/// The golden-ratio increment of SplitMix64 (`RandomSupport.GOLDEN_RATIO_64`).
pub const GOLDEN: i64 = 0x9E37_79B9_7F4A_7C15_u64 as i64;

/// `RandomSupport.mixStafford13` (SplitMix64's finalizer).
pub fn mix_stafford13(z: i64) -> i64 {
    let mut z = z as u64;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) as i64
}

/// The seed of entity `n`'s own random in a bridge sandbox seeded `seed` (`Exact.entitySeed`):
/// SplitMix64's (n + 1)-th step from `seed ^ "entity"`.
pub fn entity_seed(seed: i64, n: u32) -> i64 {
    mix_stafford13((seed ^ 0x656e_7469_7479).wrapping_add((n as i64 + 1).wrapping_mul(GOLDEN)))
}

/// Java's `String.hashCode` (over UTF-16 code units).
pub fn java_string_hash(s: &str) -> i32 {
    let mut h: i32 = 0;
    for u in s.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(u as i32);
    }
    h
}

/// The seed of the shuffling list at `key` of a brain whose entity's random was seeded
/// `entity_seed` (`Exact.shuffleSeed`): key = `"<generation>/<activity>/<priority>/<index>"`, then
/// `"/<k>"` per nested gate, `"#<j>"` for the j-th list in a closure (`TriggerGate`).
pub fn shuffle_seed(entity_seed: i64, key: &str) -> i64 {
    mix_stafford13(entity_seed.wrapping_add((java_string_hash(key) as i64).wrapping_mul(GOLDEN)))
}

/// The seed of raid `id`'s own random (`Exact.raidSeed`).
pub fn raid_seed(seed: i64, id: i32) -> i64 {
    mix_stafford13((seed ^ 0x7261_6964).wrapping_add((id as i64).wrapping_mul(GOLDEN)))
}

/// The seed of a world sandbox's wandering trader spawner random (`Exact.traderSeed`).
pub fn trader_seed(seed: i64) -> i64 {
    mix_stafford13((seed ^ 0x7472_6164_6572).wrapping_add(GOLDEN))
}

/// The seed the k-th entity constructed in a bridge sandbox gets right after its constructor
/// (`Exact.constructSeed`; k counts every `EntityType.create` in the sandbox: spawn attempts that
/// are then rejected, golem attempts, summons, loaded entities, bolts, reinforcements, offspring).
/// What the entity draws before it enters the level comes from it: a zombie's `finalizeSpawn` ->
/// `handleAttributes` (reinforcement chance, knockback resistance, follow range, the leader roll
/// and its bonuses), a salmon's variant. Entering then reseeds it with [`entity_seed`].
pub fn construct_seed(seed: i64, k: u64) -> i64 {
    mix_stafford13((seed ^ 0x636f_6e73_7472).wrapping_add((k as i64).wrapping_add(1).wrapping_mul(GOLDEN)))
}

/// `Mth.createInsecureUUID(random)`: the entity constructor's first draws (2 `nextLong`), as the
/// UUID's (most, least) significant bits.
pub fn replay_uuid(r: &mut LegacyRandom) -> (i64, i64) {
    let most = r.next_long() & -61441 | 16384;
    let least = r.next_long() & 4_611_686_018_427_387_903 | i64::MIN;
    (most, least)
}

/// Entity `n`'s own random in a sandbox seeded `seed`, past the `Entity` constructor's UUID draws
/// (the next draws are the subclasses' constructors': a living entity's yaw, its sensors...).
pub fn entity_random(seed: i64, n: u32) -> LegacyRandom {
    let mut r = LegacyRandom::new(entity_seed(seed, n));
    replay_uuid(&mut r);
    r
}

/// A drop as the bridge places it (`Sandbox.DropSeeder`): where, its yaw, its motion.
#[derive(Clone, Debug, PartialEq)]
pub struct Drop {
    pub pos: [f64; 3],
    pub y_rot: f32,
    pub motion: [f64; 3],
    /// The item's own random after those draws (`Entity.random`: `moveTowardsClosestSpace`...).
    pub random: LegacyRandom,
}

/// Half the item entity's height (`EntityTypes.ITEM`: 0.25).
const HALF_HEIGHT: f64 = 0.125;

impl Drop {
    /// The `n`-th drop (from 0) of a sandbox seeded `seed`, out of the block at `cell`.
    pub fn seeded(seed: i64, n: u64, cell: BlockPos) -> Drop {
        let mut r = LegacyRandom::new(seed ^ 0x6d_6374_6173_i64.wrapping_add(n as i64));
        let x = cell.x as f64 + 0.5 + r.next_double_between(-0.25, 0.25);
        let y = cell.y as f64 + 0.5 + r.next_double_between(-0.25, 0.25) - HALF_HEIGHT;
        let z = cell.z as f64 + 0.5 + r.next_double_between(-0.25, 0.25);
        let y_rot = r.next_float() * 360.0;
        let mx = r.next_double() * 0.2 - 0.1;
        let mz = r.next_double() * 0.2 - 0.1;
        Drop { pos: [x, y, z], y_rot, motion: [mx, 0.2, mz], random: r }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Java's `new Random(42)`-style values, from `LegacyRandomSource(42)`: the LCG itself.
    #[test]
    fn lcg_matches_java() {
        let mut r = LegacyRandom::new(42);
        // java.util.Random(42).nextInt() = -1170105035, nextInt(10) after that = 3 (the same LCG)
        assert_eq!(r.next(32), -1_170_105_035);
        assert_eq!(r.next_int(10), 3);
    }

    /// The first drop of the real-game exact trace of shulker-ref-loader-1 (seed -5180098856, the
    /// box cell (0, 1, 0)): recorded after its first tick (gravity 0.04, then the drag: 0.98F
    /// horizontally, a float widened, and 0.98 vertically), its yaw as drawn.
    #[test]
    fn drop_matches_the_bridge() {
        let d = Drop::seeded(-5_180_098_856, 0, BlockPos::new(0, 1, 0));
        let drag = 0.98_f32 as f64;
        assert_eq!(d.y_rot, 219.419_204_711_914_06_f32);
        assert_eq!(d.motion[0] * drag, -0.042_815_966_586_209_03, "{:?}", d);
        assert_eq!(d.motion[2] * drag, -0.006_477_272_257_639_967, "{:?}", d);
        assert_eq!(d.pos[1] + (0.2 - 0.04), 1.409_805_512_992_943_4, "{:?}", d);
    }
}
