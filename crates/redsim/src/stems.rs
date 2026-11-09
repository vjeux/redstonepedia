//! Stem crops (clk-misc, 2026-10-21: w86's pumpkin and melon harvesters, study 18): farmland
//! (`FarmlandBlock`), pumpkin and melon stems (`StemBlock`, `AttachedStemBlock`) and their fruits,
//! 26.3, with the loot random sequence a melon's slices draw from.
//!
//! A stem stands on farmland. Its random tick, with a raw brightness of at least 9, draws
//! `nextInt(int(25 / speed) + 1)` from the level random (`CropBlock.getGrowthSpeed`: 1, plus each
//! farmland of the 3x3 below: 1 under the stem and 1/4 around it, 3 and 3/4 when moist; halved by a
//! stem of the same kind on two sides or a diagonal); on a 0 it ages, and at age 7 it picks a side
//! (`nextInt(4)`: north, east, south, west): when that cell is air on #supports_stem_fruit (dirt,
//! mud, grass, moss, farmland), the fruit grows there (`setBlockAndUpdate`) and the stem turns into
//! the attached stem facing it, which turns back into a stem of age 7 when the fruit goes.
//!
//! Farmland's random tick keeps it moist near water (any water within 4 blocks across and 1 above)
//! and dries it otherwise, down to dirt unless a crop stands on it; it turns into dirt a tick after
//! a solid block lands on it. Anything landing on farmland draws `nextFloat` from the level random
//! (`fallOn`, the trampling roll, drawn before it asks whether the entity is a living one).
//!
//! A melon's loot (3-7 slices in one stack) draws from the server's random sequence of its loot
//! table ([`LootSequence`]), which the bridge reseeds from the sandbox seed (`Sandbox.lootSaltOf`).

use crate::block::{Block, Kind, Simple};
use crate::pos::{BlockPos, Dir};
use crate::ticks::Priority;
use crate::world::{World, flags};

// ------------------------------------------------------------------ the loot random sequences

/// MD5 (RFC 1321), for `RandomSupport.seedFromHashOf` (Guava's `Hashing.md5()` of the id).
fn md5(input: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16,
        23, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let k: Vec<u32> = (0..64).map(|i| ((i as f64 + 1.0).sin().abs() * 4_294_967_296.0) as u32).collect();
    let mut msg = input.to_vec();
    let bits = (input.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_le_bytes());
    let (mut a0, mut b0, mut c0, mut d0) = (0x67452301u32, 0xefcdab89u32, 0x98badcfeu32, 0x10325476u32);
    for chunk in msg.chunks(64) {
        let m: Vec<u32> = chunk.chunks(4).map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]])).collect();
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f = f.wrapping_add(a).wrapping_add(k[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    let mut out = [0u8; 16];
    for (i, v) in [a0, b0, c0, d0].iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    out
}

/// `RandomSupport.mixStafford13`.
fn mix_stafford13(z: i64) -> i64 {
    let z = (z ^ ((z as u64) >> 30) as i64).wrapping_mul(-4_658_895_280_553_007_687);
    let z = (z ^ ((z as u64) >> 27) as i64).wrapping_mul(-7_723_592_293_110_705_685);
    z ^ ((z as u64) >> 31) as i64
}

/// `XoroshiroRandomSource` (`Xoroshiro128PlusPlus`): what a loot random sequence draws from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LootSequence {
    lo: i64,
    hi: i64,
}

impl LootSequence {
    /// `RandomSequence(seed, key)` as the bridge's sandboxes make them
    /// (`RandomSequences.setSeedDefaults(salt, false, true)`: seed = `(long) salt`, the id mixed in).
    pub fn new(salt: i32, key: &str) -> LootSequence {
        let seed = salt as i64;
        // upgradeSeedTo128bitUnmixed, xor seedFromHashOf(key), mixed
        let lo = seed ^ 7_640_891_576_956_012_809;
        let hi = lo.wrapping_add(-7_046_029_254_386_353_131);
        let h = md5(key.as_bytes());
        let hlo = i64::from_be_bytes([h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]]);
        let hhi = i64::from_be_bytes([h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]]);
        let (mut lo, mut hi) = (mix_stafford13(lo ^ hlo), mix_stafford13(hi ^ hhi));
        if (lo | hi) == 0 {
            lo = -7_046_029_254_386_353_131;
            hi = 7_640_891_576_956_012_809;
        }
        LootSequence { lo, hi }
    }

    fn next_long(&mut self) -> i64 {
        let (s0, mut s1) = (self.lo, self.hi);
        let result = s0.wrapping_add(s1).rotate_left(17).wrapping_add(s0);
        s1 ^= s0;
        self.lo = s0.rotate_left(49) ^ s1 ^ (s1 << 21);
        self.hi = s1.rotate_left(28);
        result
    }

    /// `XoroshiroRandomSource.nextFloat()`: `nextBits(24) * 5.9604645E-8F` (the long's top 24 bits).
    pub fn next_float(&mut self) -> f32 {
        ((self.next_long() as u64) >> 40) as i32 as f32 * 5.960_464_5e-8_f32
    }

    /// `XoroshiroRandomSource.nextInt(bound)` (Lemire's unbiased multiply).
    pub fn next_int(&mut self, bound: i32) -> i32 {
        let bound64 = bound as u64;
        let mut product = (self.next_long() as i32 as u32 as u64) * bound64;
        let mut low = product & 0xffff_ffff;
        if low < bound64 {
            let threshold = ((!(bound as u32)).wrapping_add(1) % bound as u32) as u64;
            while low < threshold {
                product = (self.next_long() as i32 as u32 as u64) * bound64;
                low = product & 0xffff_ffff;
            }
        }
        (product >> 32) as i32
    }
}

/// `Sandbox.lootSaltOf(seed)`: the salt of a sandbox's loot random sequences.
pub fn loot_salt(seed: i64) -> i32 {
    (seed ^ 0x6C6F_6F74) as i32
}

/// The slices a broken melon drops (`blocks/melon`: `set_count` uniform 3-7, `apply_bonus`
/// fortune with no enchantment: `nextInt(1)`, `limit_count` 9), from its random sequence.
pub fn melon_slices(w: &mut World) -> Result<u32, String> {
    let Some(salt) = w.loot_salt else {
        return Err("a melon's slices draw from its loot table's random sequence (World::loot_salt: the sandbox seed)".to_string());
    };
    let r = w.loot_sequences.entry("minecraft:blocks/melon".to_string()).or_insert_with(|| LootSequence::new(salt, "minecraft:blocks/melon"));
    let n = 3 + r.next_int(5);
    r.next_int(1);
    Ok(n.min(9) as u32)
}

// ------------------------------------------------------------------ farmland

/// #maintains_farmland among redsim's blocks: the stems, moving pistons, fence gates.
fn maintains_farmland(b: &Block) -> bool {
    matches!(b, Block::Stem { .. } | Block::AttachedStem { .. } | Block::MovingPiston { .. } | Block::FenceGate { .. })
}

/// `FarmlandBlock.canSurvive`: the block above is not solid, or keeps the farmland.
pub fn farmland_can_survive(w: &World, p: BlockPos) -> bool {
    let above = w.get(p.above());
    !crate::behavior::is_solid(&above) || maintains_farmland(&above)
}

/// `FarmlandBlock.updateShape`: a tick later it turns into dirt when it cannot stay.
pub fn farmland_update_shape(w: &mut World, s: Block, p: BlockPos, d: Dir) -> Block {
    if d == Dir::Up && !farmland_can_survive(w, p) {
        w.schedule_tick(p, Kind::Farmland, 1, Priority::Normal);
    }
    s
}

/// `FarmlandBlock.tick`.
pub fn farmland_tick(w: &mut World, p: BlockPos) {
    if !farmland_can_survive(w, p) {
        turn_to_dirt(w, p);
    }
}

/// `FarmlandBlock.turnToBaseBlock`: dirt (`pushEntitiesUp` lifts what stands on it: refused), the
/// BLOCK_CHANGE game event.
fn turn_to_dirt(w: &mut World, p: BlockPos) {
    let top = p.y as f64 + 1.0;
    let lifted = w.entities.list.iter().any(|e| {
        let b = e.bb;
        b.min_x < p.x as f64 + 1.0 && b.max_x > p.x as f64 && b.min_z < p.z as f64 + 1.0 && b.max_z > p.z as f64 && b.min_y < top && b.max_y > p.y as f64 + 15.0 / 16.0 - 1e-7
    });
    if lifted {
        let why = format!("farmland at {p} turning into dirt under an entity (pushEntitiesUp) is not modeled");
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
    }
    let dirt = Block::Simple(Simple::Dirt);
    w.set_block_and_update(p, dirt);
    w.game_event_state("block_change", p, dirt);
}

/// `FarmlandBlock.isNearWater`: water in the box 4 across and 1 above.
fn near_water(w: &World, p: BlockPos) -> bool {
    for dy in 0..=1 {
        for dz in -4..=4 {
            for dx in -4..=4 {
                if crate::fluid::fluid_state(w, p.offset(dx, dy, dz)).fluid.is_water() {
                    return true;
                }
            }
        }
    }
    false
}

fn farmland_random_tick(w: &mut World, s: Block, p: BlockPos) {
    let Block::Farmland { moisture } = s else { return };
    if !near_water(w, p) {
        if moisture > 0 {
            w.set_block(p, Block::Farmland { moisture: moisture - 1 }, flags::CLIENTS);
        } else if !maintains_farmland(&w.get(p.above())) {
            turn_to_dirt(w, p);
        }
    } else if moisture < 7 {
        w.set_block(p, Block::Farmland { moisture: 7 }, flags::CLIENTS);
    }
}

/// `FarmlandBlock.fallOn` for an entity landing on the farmland at `p` (`fallDistance > 0`): the
/// trampling roll draws `nextFloat` from the level random whatever the entity; only a living one
/// (none modeled) could trample.
pub fn fall_on(w: &mut World) {
    match w.level_random.as_mut() {
        Some(r) => {
            r.next_float();
        }
        None => {
            let why = "an entity landing on farmland draws from the level random (World::mirror_level_random)".to_string();
            if !w.errors.contains(&why) {
                w.errors.push(why);
            }
        }
    }
}

// ------------------------------------------------------------------ stems

/// #supports_stem_fruit among redsim's blocks: dirt, grass, mud, farmland.
fn supports_fruit(b: &Block) -> bool {
    matches!(b, Block::Simple(Simple::Dirt | Simple::GrassBlock | Simple::Mud) | Block::Farmland { .. })
}

/// `VegetationBlock.canSurvive` of a stem or an attached stem: on farmland.
pub fn stem_can_survive(w: &World, p: BlockPos) -> bool {
    matches!(w.get(p.below()), Block::Farmland { .. })
}

/// The fruit block of a stem.
pub fn fruit(melon: bool) -> Block {
    Block::Simple(if melon { Simple::Melon } else { Simple::Pumpkin })
}

/// `StemBlock.updateShape` (`VegetationBlock`): air when it cannot stand.
pub fn stem_update_shape(w: &World, s: Block, p: BlockPos) -> Block {
    if !stem_can_survive(w, p) { Block::Air } else { s }
}

/// `AttachedStemBlock.updateShape`: a stem of age 7 again when the cell it faces holds no fruit.
pub fn attached_update_shape(w: &World, s: Block, p: BlockPos, d: Dir, ns: Block) -> Block {
    if let Block::AttachedStem { melon, facing } = s
        && d == facing
        && ns != fruit(melon)
    {
        return Block::Stem { melon, age: 7 };
    }
    if !stem_can_survive(w, p) { Block::Air } else { s }
}

/// `CropBlock.getGrowthSpeed(type, level, pos)`.
fn growth_speed(w: &World, s: &Block, p: BlockPos) -> f32 {
    let Block::Stem { melon, .. } = *s else { return 1.0 };
    let mut speed = 1.0f32;
    let below = p.below();
    for dx in -1..=1 {
        for dz in -1..=1 {
            let mut b = 0.0f32;
            if let Block::Farmland { moisture } = w.get(below.offset(dx, 0, dz)) {
                b = if moisture > 0 { 3.0 } else { 1.0 };
            }
            if dx != 0 || dz != 0 {
                b /= 4.0;
            }
            speed += b;
        }
    }
    let same = |q: BlockPos| matches!(w.get(q), Block::Stem { melon: m, .. } if m == melon);
    let horizontal = same(p.offset(-1, 0, 0)) || same(p.offset(1, 0, 0));
    let vertical = same(p.offset(0, 0, -1)) || same(p.offset(0, 0, 1));
    if horizontal && vertical {
        speed /= 2.0;
    } else if same(p.offset(-1, 0, -1)) || same(p.offset(1, 0, -1)) || same(p.offset(1, 0, 1)) || same(p.offset(-1, 0, 1)) {
        speed /= 2.0;
    }
    speed
}

/// `Direction.Plane.HORIZONTAL`'s faces, in order.
const HORIZONTAL: [Dir; 4] = [Dir::North, Dir::East, Dir::South, Dir::West];

fn stem_random_tick(w: &mut World, s: Block, p: BlockPos) {
    let Block::Stem { melon, age } = s else { return };
    match crate::bamboo::bright_enough(w, p) {
        Some(true) => {}
        Some(false) => return,
        None => {
            let near = crate::daylight::unclassified_near(w, p).map(|(q, s)| format!(": {s} at {q}")).unwrap_or_default();
            let why = format!("the light at the stem at {p} (redsim's light model does not classify a block near it{near})");
            if !w.errors.contains(&why) {
                w.errors.push(why);
            }
            return;
        }
    }
    let speed = growth_speed(w, &s, p);
    let bound = (25.0f32 / speed) as i32 + 1;
    let Some(r) = w.level_random.as_mut() else {
        let why = "a stem's random tick draws from the level random (World::mirror_level_random)".to_string();
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
        return;
    };
    if r.next_int(bound) != 0 {
        return;
    }
    if age < 7 {
        w.set_block(p, Block::Stem { melon, age: age + 1 }, flags::CLIENTS);
        return;
    }
    let d = HORIZONTAL[r.next_int(4) as usize];
    let q = p.rel(d);
    if w.get(q).is_air() && supports_fruit(&w.get(q.below())) {
        w.set_block_and_update(q, fruit(melon));
        w.set_block_and_update(p, Block::AttachedStem { melon, facing: d });
    }
}

/// The random ticks of this module's blocks.
pub fn random_tick(w: &mut World, s: Block, p: BlockPos) {
    match s {
        Block::Farmland { .. } => farmland_random_tick(w, s, p),
        Block::Stem { .. } => stem_random_tick(w, s, p),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: [u8; 16]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn md5_vectors() {
        assert_eq!(hex(md5(b"")), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(hex(md5(b"abc")), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(hex(md5(b"The quick brown fox jumps over the lazy dog")), "9e107d9d372bb6826bd81d3542a419d6");
        assert_eq!(hex(md5(&[b'a'; 100])), "36a92cc94a9e0fa21f625f8bfb007adf");
    }

    #[test]
    fn xoroshiro_bounded() {
        // a sequence draws in range and repeats from the same salt and id
        let mut a = LootSequence::new(12345, "minecraft:blocks/melon");
        let mut b = LootSequence::new(12345, "minecraft:blocks/melon");
        for _ in 0..1000 {
            let (x, y) = (a.next_int(5), b.next_int(5));
            assert_eq!(x, y);
            assert!((0..5).contains(&x));
            assert_eq!(a.next_int(1), 0);
            b.next_int(1);
        }
    }
}
