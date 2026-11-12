//! (b8949c96 2026-11-12; ironspec's storage spec, farmopt 01:02, its 20 play-hour default 01:13
//! and the accepted plan 01:17) The carousel's storage at the shared Nether kill room, its design
//! and its redsim bench.
//!
//! The design (`design`), built from the repo's record holders: the kill floor's 2 x 2 hoppers
//! feed `lines` hopper lines (each fed from the side: a hopper pointing down onto a sorter's line
//! hopper would push and be pulled from, two items every 8 gt, and the slice lets half its kind
//! by); on each line a sorter-classic iron slice (study 13: recipe 41 iron + 4 named papers, so a
//! poppy never enters it) whose drain hopper B, turned east, is the input hopper of an
//! ironblock-v24 auto-crafter (study 20: it crafts only once its comparator reads 9, so a partial
//! grid never crafts; mirrored in z so its redstone keeps clear of the slice's); the crafter
//! crafts down into a chest (a hopper under the crafter would pull the grid out: hoppers take from
//! any container above them), a hopper under the chest takes the blocks, and they run down a storage line into a hall of double
//! chests on one level (storage-bulk's layout: a double chest across the line, pitch 1, a pull
//! hopper over its inner half under the line hopper). The poppies run on along the line, into a
//! dropper an observer clock fires into lava: no chest to fill, nothing to back the iron line up.
//! Once the hall is full the crafters' output can't go down: 26.3's `CrafterBlock.dispenseItem`
//! spawns what its front container won't take as an item entity (`DefaultDispenseItemBehavior.
//! spawnItem`, accuracy 6), where it lies and despawns, and the crafter goes on crafting, so
//! nothing backs up to the kill floor or the portal.
//!
//! The bench (`bench`): golem deaths over the kill floor at N modules' rate (N x 236 a game day,
//! Bernoulli each game tick), each the iron golem's loot (`entities/iron_golem`: 3 to 5 iron
//! ingots and 0 to 2 poppies, uniform), dropped where it dies with the game's drift, over the given
//! game days; it reports iron in against iron stored (ingots + 9 x blocks), poppies disposed,
//! poppies reaching a crafter, crafts, the intake's largest backlog, items despawned, and the
//! lines' load against 3,000 items a game day each.

use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::EntityKind;
use redsim::pos::BlockPos;
use std::collections::{BTreeMap, BTreeSet};

type P = [i32; 3];

const SORTER: &str = include_str!("../../../designs/circuits/sorter/archive/sorter-classic.txt");
const CRAFTER: &str = include_str!("../../../designs/circuits/crafter/archive/ironblock-v24.txt");

/// Golems a module a game day (the bill's).
pub const GOLEMS_A_DAY: f64 = 236.0;
/// Items a hopper line carries a game day (one per 8 gt).
pub const LINE_A_DAY: f64 = 3000.0;
/// Iron blocks a double chest holds (54 slots of 64).
pub const DOUBLE_CHEST: u32 = 3456;

/// The storage's shape: its hopper lines (one sorter and one crafter each) and its double chests.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub lines: usize,
    pub chests: usize,
    /// The ghast rule's enclosure (ironspec 2026-11-12): blast-resistant walls and roof round the
    /// hall and its walkway (cobblestone; hoppers count), the walkway's floor unspawnable (bottom
    /// slabs), a doorway at the hall's east end for the player's way in.
    pub enclose: bool,
}

/// The design and the places the bench reads.
pub struct Built {
    pub design: Design,
    /// The kill floor's hoppers (where the drops land, on top).
    pub floor: Vec<P>,
    /// The intake: the floor's hoppers, the collectors and each line's first hopper.
    pub intake: Vec<P>,
    /// Each line's filter hopper A and drain hopper B (their recipe stock: 41 and 64 iron).
    pub recipes: Vec<P>,
    /// Each line's crafter.
    pub crafters: Vec<P>,
    /// The observer to kick (the clock starts on a change it sees).
    pub kick: P,
    /// Every chest half.
    pub chests: Vec<P>,
    /// The bench's own blocks (the kill floor's glass ring), left out of a framed copy.
    pub bench_only: BTreeSet<P>,
    /// The enclosure's blocks (walls, roof and floor round the hall, the door and its buttons).
    pub enclosure: usize,
    /// The door's buttons (left on the outside: the opener).
    pub opener: BTreeSet<P>,
    /// The shield blocks (cobblestone on the faces of the blocks under the explosion bar).
    pub shield: usize,
}

/// A block's blast resistance (26.3's `explosionResistance`) for the storage's blocks; 0 for any
/// other (the conservative side).
pub fn resistance(state: &str) -> f32 {
    let n = state.trim_start_matches("minecraft:");
    let n = n.split('[').next().unwrap_or(n);
    match n {
        "obsidian" => 1200.0,
        "lava" | "water" => 100.0,
        "cobblestone" | "stone" | "cobblestone_slab" | "stone_slab" | "cobblestone_wall" => 6.0,
        "iron_door" | "iron_trapdoor" => 5.0,
        "hopper" => 4.8,
        "crafter" | "dropper" | "dispenser" => 3.5,
        "observer" => 3.0,
        "chest" | "trapped_chest" => 2.5,
        "stone_button" => 0.5,
        "glass" => 0.3,
        _ => 0.0,
    }
}

/// The explosion bar: a ghast fireball's power-1 blast stops at a block of 4.034 or more.
pub const BLAST_BAR: f32 = 4.034;
/// The breaking bar (G1 2026-11-12): a power-1 ray's second sample has at most 1.075 left, so a
/// block of 3.283 or more (crafters, droppers: 3.5) breaks only with the blast's centre in its own
/// cell; one under it can break from a reachable cell.
pub const BREAK_BAR: f32 = 3.283;

/// The 26 neighbours (faces, edges and corners: a power-1 ray samples every 0.3 blocks, so from a
/// centre near a cell's edge its next sample can land in a diagonal cell, past both face shields).
fn twenty_six() -> impl Iterator<Item = P> {
    (-1..=1).flat_map(|x| (-1..=1).flat_map(move |y| (-1..=1).map(move |z| [x, y, z]))).filter(|v| *v != [0, 0, 0])
}

/// The cells a blast from outside the design's box (one cell round it) reaches through cells
/// under the explosion bar (empty, or a block under 4.034), 26-connected (G1 2026-11-12).
fn reach26(d: &Design) -> BTreeSet<P> {
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for p in d.blocks.keys() {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i] - 1);
            hi[i] = hi[i].max(p[i] + 1);
        }
    }
    let inside = |p: P| (0..3).all(|i| p[i] >= lo[i] && p[i] <= hi[i]);
    let passable = |p: &P| d.blocks.get(p).is_none_or(|s| resistance(s) < BLAST_BAR);
    let mut seen: BTreeSet<P> = BTreeSet::new();
    let mut queue: std::collections::VecDeque<P> = std::collections::VecDeque::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let edge = x == lo[0] || x == hi[0] || y == lo[1] || y == hi[1] || z == lo[2] || z == hi[2];
                if edge && passable(&[x, y, z]) && seen.insert([x, y, z]) {
                    queue.push_back([x, y, z]);
                }
            }
        }
    }
    while let Some(p) = queue.pop_front() {
        for v in twenty_six() {
            let q = [p[0] + v[0], p[1] + v[1], p[2] + v[2]];
            if inside(q) && passable(&q) && seen.insert(q) {
                queue.push_back(q);
            }
        }
    }
    seen
}

/// Where shields go: every empty cell a blast reaches (`reach26`) that is a 26-neighbour of a
/// block under the explosion bar (the opener's aside), so the blast meets a shield before any
/// such block, edges and corners included.
fn shield_cells(d: &Design, opener: &BTreeSet<P>) -> BTreeSet<P> {
    let reach = reach26(d);
    let mut out = BTreeSet::new();
    for (p, s) in &d.blocks {
        if resistance(s) >= BLAST_BAR || opener.contains(p) {
            continue;
        }
        for v in twenty_six() {
            let q = [p[0] + v[0], p[1] + v[1], p[2] + v[2]];
            if !d.blocks.contains_key(&q) && reach.contains(&q) {
                out.insert(q);
            }
        }
    }
    out
}

/// Every block under the breaking bar a blast from outside reaches (`reach26`; the opener's
/// aside): `(the block, its state)`; 0 required (G1 2026-11-12).
pub fn exposed(d: &Design, opener: &BTreeSet<P>) -> Vec<(P, String)> {
    let reach = reach26(d);
    d.blocks.iter().filter(|(p, s)| resistance(s) < BREAK_BAR && !opener.contains(*p) && reach.contains(*p)).map(|(p, s)| (*p, s.clone())).collect()
}

/// The design moved by `off` without the bench's own blocks (onechunk's Nether frame), and its
/// opener moved likewise.
pub fn shifted(b: &Built, off: P) -> (Design, BTreeSet<P>) {
    let mv = |p: &P| [p[0] + off[0], p[1] + off[1], p[2] + off[2]];
    let mut d = Design { name: format!("{}-framed", b.design.name), meta: b.design.meta.clone(), ..Default::default() };
    for (p, s) in &b.design.blocks {
        if !b.bench_only.contains(p) {
            d.blocks.insert(mv(p), s.clone());
        }
    }
    for (p, nbt) in &b.design.block_entities {
        d.block_entities.insert(mv(p), nbt.clone());
    }
    for (k, p) in &b.design.ports {
        d.ports.insert(k.clone(), mv(p));
    }
    (d, b.opener.iter().map(mv).collect())
}

/// North and south swapped in a block state (the crafter mirrored in z).
fn swap_ns(s: &str) -> String {
    s.replace("north", "\u{1}").replace("south", "north").replace('\u{1}', "south")
}

fn hopper(facing: &str) -> String {
    format!("minecraft:hopper[enabled=true,facing={facing}]")
}

/// The storage design for `l` (1 or 2 lines, at least one double chest): the bench frame, the
/// kill floor at y 4, the lines' first hoppers at y 3, the hall's chests at y -3.
pub fn design(l: &Layout) -> Result<Built, String> {
    if !(1..=2).contains(&l.lines) || l.chests == 0 {
        return Err(format!("{} lines and {} double chests: 1 or 2 lines, at least one double chest", l.lines, l.chests));
    }
    let sorter = Design::parse(SORTER).map_err(|e| format!("sorter-classic: {e}"))?;
    let crafter = Design::parse(CRAFTER).map_err(|e| format!("ironblock-v24: {e}"))?;
    let mut d = Design { name: format!("storage-{}-lines-{}-double-chests", l.lines, l.chests), ..Default::default() };
    let mut put = |d: &mut Design, p: P, s: &str| -> Result<(), String> {
        if let Some(old) = d.blocks.get(&p) {
            return Err(format!("{s} lands on {old} at {p:?}"));
        }
        d.blocks.insert(p, s.to_string());
        Ok(())
    };
    let mut b = Built { design: Design::default(), floor: Vec::new(), intake: Vec::new(), recipes: Vec::new(), crafters: Vec::new(), kick: [7, 1, 0], chests: Vec::new(), bench_only: BTreeSet::new(), enclosure: 0, opener: BTreeSet::new(), shield: 0 };
    let zs: Vec<i32> = (0..l.lines as i32).map(|i| 7 * i).collect();
    for &z0 in &zs {
        // the sorter slice as archived (L at y 3, A, B, its redstone south of it at x 0), its
        // drain B turned east into the crafter
        for (p, s) in &sorter.blocks {
            let s = if *p == [0, 1, 0] { s.replace("facing=down", "facing=east") } else { s.clone() };
            put(&mut d, [p[0], p[1], p[2] + z0], &s)?;
        }
        for (p, nbt) in &sorter.block_entities {
            d.block_entities.insert([p[0], p[1], p[2] + z0], nbt.clone());
        }
        // the auto-crafter as archived, mirrored in z, its hopper H the sorter's B (M at x 1, y 1)
        for (p, s) in &crafter.blocks {
            if *p == [-1, 0, 0] {
                continue;
            }
            put(&mut d, [p[0] + 1, p[1] + 1, z0 - p[2]], &swap_ns(s))?;
        }
        for (p, nbt) in &crafter.block_entities {
            d.block_entities.insert([p[0] + 1, p[1] + 1, z0 - p[2]], nbt.clone());
        }
        b.recipes.push([0, 2, z0]);
        b.recipes.push([0, 1, z0]);
        b.crafters.push([1, 1, z0]);
        // the crafter's blocks: down into a chest (a hopper under the crafter would pull its grid
        // out, the bench's first run: hoppers take from any container above them), a hopper under
        // the chest, then north into the storage line
        put(&mut d, [1, 0, z0], "minecraft:chest[facing=north,type=single,waterlogged=false]")?;
        put(&mut d, [1, -1, z0], &hopper("north"))?;
        put(&mut d, [1, -1, z0 - 1], &hopper("north"))?;
        // the collector into L, from the west
        put(&mut d, [-1, 3, z0], &hopper("east"))?;
        b.intake.push([-1, 3, z0]);
        b.intake.push([0, 3, z0]);
    }
    // the kill floor (2 x 2 at y 4) and its way to the collectors
    let zk = if l.lines == 1 { 0 } else { 3 };
    if l.lines == 1 {
        for (p, f) in [([-2, 4, 1], "north"), ([-1, 4, 1], "north"), ([-2, 4, 0], "down"), ([-1, 4, 0], "down"), ([-2, 3, 0], "east")] {
            put(&mut d, p, &hopper(f))?;
            b.intake.push(p);
        }
    } else {
        for (p, f) in [
            ([-2, 4, 3], "east"),
            ([-1, 4, 3], "north"),
            ([-1, 4, 2], "north"),
            ([-1, 4, 1], "north"),
            ([-1, 4, 0], "down"),
            ([-2, 4, 4], "east"),
            ([-1, 4, 4], "south"),
            ([-1, 4, 5], "south"),
            ([-1, 4, 6], "south"),
            ([-1, 4, 7], "down"),
        ] {
            put(&mut d, p, &hopper(f))?;
            b.intake.push(p);
        }
    }
    b.floor = vec![[-2, 4, zk], [-1, 4, zk], [-2, 4, zk + 1], [-1, 4, zk + 1]];
    // its glass ring, two high, so a drop stays on the floor (the bench's: the kill room's own
    // walls hold the drops there)
    for y in 5..=6 {
        for z in zk - 1..=zk + 2 {
            for x in -3..=0 {
                if !((-2..=-1).contains(&x) && (zk..=zk + 1).contains(&z)) {
                    put(&mut d, [x, y, z], "minecraft:glass")?;
                    b.bench_only.insert([x, y, z]);
                }
            }
        }
    }
    // the poppies' line: east at y 3 to the dropper at x 7 (line 1 turning north at x 5)
    for x in 1..=6 {
        put(&mut d, [x, 3, 0], &hopper("east"))?;
    }
    if l.lines == 2 {
        for x in 1..=4 {
            put(&mut d, [x, 3, 7], &hopper("east"))?;
        }
        for z in 1..=7 {
            put(&mut d, [5, 3, z], &hopper("north"))?;
        }
    }
    // the disposal: a dropper into a lava source, fired by an observer pair (O1 under it
    // watching O2, O2 watching O1; O1's back powers the dropper)
    put(&mut d, [7, 3, 0], "minecraft:dropper[facing=east,triggered=false]")?;
    put(&mut d, [8, 3, 0], "minecraft:lava[level=0]")?;
    for p in [[9, 3, 0], [8, 2, 0], [8, 3, 1], [8, 3, -1], [8, 4, 0]] {
        put(&mut d, p, "minecraft:cobblestone")?;
    }
    put(&mut d, [7, 2, 0], "minecraft:observer[facing=down,powered=false]")?;
    put(&mut d, [7, 1, 0], "minecraft:observer[facing=up,powered=false]")?;
    // the iron blocks' line: east along z -2 at y -1 (line 1 merging at x 5), then the hall
    for x in 1..=5 {
        put(&mut d, [x, -1, -2], &hopper("east"))?;
    }
    if l.lines == 2 {
        for x in 1..=4 {
            put(&mut d, [x, -1, 5], &hopper("east"))?;
        }
        for z in -1..=5 {
            put(&mut d, [5, -1, z], &hopper("north"))?;
        }
    }
    // the hall: from x 6 east, each double chest across the line (its inner half at z -2 under
    // the pull hopper, its outer half at z -3 on the walkway at z -4)
    for k in 0..l.chests as i32 {
        let x = 6 + k;
        put(&mut d, [x, -1, -2], &hopper("east"))?;
        put(&mut d, [x, -2, -2], &hopper("down"))?;
        put(&mut d, [x, -3, -2], "minecraft:chest[facing=west,type=left,waterlogged=false]")?;
        put(&mut d, [x, -3, -3], "minecraft:chest[facing=west,type=right,waterlogged=false]")?;
        b.chests.push([x, -3, -2]);
        b.chests.push([x, -3, -3]);
    }
    if l.enclose {
        // the hall's room: x 5..6+D, z -5..-1, y -4 (floor) .. 0 (roof); the walkway (z -4, y -3
        // and -2) floored with bottom slabs, the chests' floor cobblestone; the doorway at the
        // east wall's walkway cells; a cell a hopper of the lines already holds stays a hopper
        let x1 = 6 + l.chests as i32;
        for y in -4..=0 {
            for z in -5..=-1 {
                for x in 5..=x1 {
                    let edge = x == 5 || x == x1 || z == -5 || z == -1 || y == 0 || y == -4;
                    if !edge || d.blocks.contains_key(&[x, y, z]) {
                        continue;
                    }
                    if x == x1 && z == -4 && (y == -3 || y == -2) {
                        continue;
                    }
                    let s = if y == -4 && z == -4 && x > 5 && x < x1 { "minecraft:cobblestone_slab[type=bottom,waterlogged=false]" } else { "minecraft:cobblestone" };
                    put(&mut d, [x, y, z], s)?;
                    b.enclosure += 1;
                }
            }
        }
        // the doorway closed by an iron door (blast resistance 5.0, G1), a stone button each side
        // to open it (the opener: the only blocks left on the outside, a button being 0.5)
        put(&mut d, [x1, -3, -4], "minecraft:iron_door[facing=west,half=lower,hinge=left,open=false,powered=false]")?;
        put(&mut d, [x1, -2, -4], "minecraft:iron_door[facing=west,half=upper,hinge=left,open=false,powered=false]")?;
        put(&mut d, [x1 - 1, -2, -4], "minecraft:stone_button[face=wall,facing=south,powered=false]")?;
        put(&mut d, [x1 + 1, -2, -5], "minecraft:stone_button[face=wall,facing=east,powered=false]")?;
        b.opener.insert([x1 - 1, -2, -4]);
        b.opener.insert([x1 + 1, -2, -5]);
        b.enclosure += 4;
        // every block under the explosion bar (blast resistance 4.034: a ghast fireball's power-1
        // blast) shielded: a cobblestone slab in every empty cell a blast from outside reaches
        // (26-connected through cells under the bar: a ray samples every 0.3 blocks and can cut a
        // corner) beside such a block, so no block under the 3.283 breaking bar is reachable (G1
        // 2026-11-12, the loader's strict rule); hoppers (4.8) and the walls count as shields;
        // reckoned on the design without the bench's own blocks (the framed copy's outside)
        let mut real = Design::default();
        for (p, s) in &d.blocks {
            if !b.bench_only.contains(p) {
                real.blocks.insert(*p, s.clone());
            }
        }
        for p in shield_cells(&real, &b.opener) {
            if !d.blocks.contains_key(&p) {
                // (a bottom slab, blast resistance 6.0 like cobblestone, but no redstone
                // conductor: a full block here cut a dust step and locked the sorter's drain for
                // good, the shield's first bench run; a slab neither cuts dust nor takes power)
                put(&mut d, p, "minecraft:cobblestone_slab[type=bottom,waterlogged=false]")?;
                b.shield += 1;
            }
        }
    }
    // (the bench's floor under everything at y -5, so a spilled or dropped item rests and despawns
    // instead of falling out of the bench world)
    for z in -6..=12 {
        for x in -4..=8 + l.chests as i32 {
            if !d.blocks.contains_key(&[x, -5, z]) {
                put(&mut d, [x, -5, z], "minecraft:cobblestone")?;
                b.bench_only.insert([x, -5, z]);
            }
        }
    }
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    // (a mob-farm design's output port must be air or its single chest: the judge's chest, out of
    // the way)
    d.ports.insert("output".into(), [-6, 8, -8]);
    b.design = d;
    Ok(b)
}

/// The design file's text, with its header.
pub fn design_text(l: &Layout, b: &Built) -> String {
    let mut t = format!(
        "# Storage at the shared Nether kill room (b8949c96 2026-11-12, ironspec's spec; bench frame, onechunk places it): \
         the kill floor's 2 x 2 hoppers (x -2..-1, y 4) feed {} hopper line(s), each fed from the side; on each a sorter-classic iron slice \
         (study 13, recipe 41 iron + 4 named papers) whose drain, turned east, feeds an ironblock-v24 auto-crafter (study 20, mirrored in z; it crafts \
         only at comparator 9); the crafters' iron blocks run down the storage line (y -1) into {} double chest(s) across the line at y -3 (a pull \
         hopper over each inner half; the walkway at z -4); the poppies run on to a dropper an observer pair fires into lava (x 7..8, y 3).\n",
        l.lines, l.chests
    );
    t.push_str(&b.design.to_text());
    t
}

#[derive(Clone, Debug)]
pub struct BenchOpts {
    /// Modules killing into the room (236 golems a game day each).
    pub modules: u32,
    /// Game days run.
    pub days: f64,
    pub seed: i64,
    /// Iron blocks put in each double chest before the run (3,456: the hall full).
    pub prefill: u32,
    /// Every line's sorter set to poppies instead of iron (a mis-set filter: the control).
    pub misset: bool,
}

/// The bench's counts.
#[derive(Clone, Debug, Default)]
pub struct Tally {
    pub golems: u64,
    pub iron_in: u64,
    pub poppies_in: u64,
    pub ingots_held: u64,
    pub blocks_held: u64,
    pub blocks_stored: u64,
    pub ingots_ground: u64,
    pub blocks_ground: u64,
    pub poppies_held: u64,
    pub poppies_ground: u64,
    pub poppies_in_crafters: u64,
    pub crafts: u64,
    pub backlog_max: u32,
    pub backlog_at: P,
    pub despawned_iron: u64,
    pub despawned_poppies: u64,
    pub oldest_on_floor: i32,
    pub spilled: u64,
    pub errors: Vec<String>,
    pub dump: Vec<String>,
}

fn item_counts(w: &redsim::world::World) -> BTreeMap<u32, (String, u32, i32, [f64; 3])> {
    w.entities
        .list
        .iter()
        .filter(|e| !e.removed)
        .filter_map(|e| match &e.kind {
            EntityKind::Item(s) => Some((e.id, (s.item.trim_start_matches("minecraft:").to_string(), s.count as u32, s.age, [e.pos.x, e.pos.y, e.pos.z]))),
            _ => None,
        })
        .collect()
}

pub fn bench(spec: &FarmSpec, b: &Built, l: &Layout, o: &BenchOpts) -> Result<Tally, String> {
    use redsim::redstone::container as rc;
    let ticks = (o.days * 24000.0).round() as i64;
    let ro = RunOpts { ai: false, ..RunOpts::steady(o.seed, (ticks + 200) as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    run.world.level_random = Some(redsim::rng::LegacyRandom::new(o.seed));
    run.world.drop_seed = Some(o.seed);
    let iron = rc::item_id("iron_ingot").ok_or("no iron_ingot item")?;
    let iron_block = rc::item_id("iron_block").ok_or("no iron_block item")?;
    let poppy = rc::item_id("poppy").ok_or("no poppy item")?;
    let bp = |p: P| BlockPos::new(p[0], p[1], p[2]);
    if o.misset {
        for p in &b.recipes {
            if let Some(c) = run.world.containers.map.get_mut(&bp(*p)) {
                for st in c.slots.iter_mut().flatten() {
                    if st.item == iron {
                        st.item = poppy;
                    }
                }
            }
        }
    }
    if o.prefill > 0 {
        let mut left_in = |n: u32| -> Vec<Option<rc::Stack>> {
            let mut v = Vec::new();
            let mut n = n;
            for _ in 0..27 {
                let c = n.min(64);
                n -= c;
                v.push(if c > 0 { Some(rc::Stack::new(iron_block, c as u8)) } else { None });
            }
            v
        };
        let per_half = (o.prefill.min(DOUBLE_CHEST) + 1) / 2;
        for p in &b.chests {
            if let Some(c) = run.world.containers.map.get_mut(&bp(*p)) {
                c.slots = left_in(per_half);
            }
        }
        if o.prefill >= DOUBLE_CHEST {
            // the full-storage row: every container past the crafters (their output chests, the
            // storage line, the hall's hoppers: y 0 and under in the bench frame) full of iron
            // blocks too, so the crafters' next blocks have nowhere to go
            for (q, c) in run.world.containers.map.iter_mut() {
                if q.y <= 0 && !b.chests.contains(&[q.x, q.y, q.z]) {
                    let n = c.slots.len();
                    c.slots = vec![Some(rc::Stack::new(iron_block, 64)); n];
                }
            }
        }
    }
    // the clock's start: the observer pair sees O2 change
    let k = bp(b.kick);
    let s = run.world.get(k);
    run.world.set_block(k, Block::parse("minecraft:stone")?, redsim::world::flags::ALL);
    run.world.set_block(k, s, redsim::world::flags::ALL);
    let stock: u64 = b
        .recipes
        .iter()
        .map(|p| run.world.containers.map.get(&bp(*p)).map_or(0, |c| c.slots.iter().flatten().filter(|s| s.item == iron || s.item == poppy).map(|s| s.count as u64).sum::<u64>()))
        .sum();
    let stored0: u64 = b.chests.iter().map(|p| run.world.containers.map.get(&bp(*p)).map_or(0, |c| c.slots.iter().flatten().filter(|s| s.item == iron_block).map(|s| s.count as u64).sum::<u64>())).sum();
    let held0: u64 = run.world.containers.map.values().map(|c| c.slots.iter().flatten().filter(|s| s.item == iron_block).map(|s| s.count as u64).sum::<u64>()).sum();
    let mut t = Tally::default();
    let p = o.modules as f64 * GOLEMS_A_DAY / 24000.0;
    let mut rng = redsim::rng::LegacyRandom::new(o.seed ^ 0x5707_a6e5);
    let (fx0, fz0) = (b.floor.iter().map(|q| q[0]).min().unwrap_or(0), b.floor.iter().map(|q| q[2]).min().unwrap_or(0));
    let fy = b.floor[0][1] as f64 + 1.25;
    let mut last: BTreeMap<u32, (String, u32, i32, [f64; 3])> = BTreeMap::new();
    let on_floor = |q: [f64; 3]| q[0] >= fx0 as f64 && q[0] < fx0 as f64 + 2.0 && q[2] >= fz0 as f64 && q[2] < fz0 as f64 + 2.0 && q[1] > fy - 1.5;
    let errors0 = run.world.errors.len();
    for _ in 0..ticks {
        if rng.next_double() < p {
            t.golems += 1;
            let n_iron = 3 + rng.next_int(3) as u32;
            let n_poppy = rng.next_int(3) as u32;
            let at = redsim::entity::Vec3::new(fx0 as f64 + 0.3 + rng.next_double() * 1.4, fy, fz0 as f64 + 0.3 + rng.next_double() * 1.4);
            run.world.spawn_item(at, "minecraft:iron_ingot", n_iron, 0);
            if n_poppy > 0 {
                run.world.spawn_item(at, "minecraft:poppy", n_poppy, 0);
            }
            t.iron_in += n_iron as u64;
            t.poppies_in += n_poppy as u64;
        }
        run.tick();
        // items gone at the despawn age (absorbed or burnt ones go younger)
        let now = item_counts(&run.world);
        for (id, (it, n, age, _)) in &last {
            if !now.contains_key(id) && *age >= 5998 {
                if it == "poppy" {
                    t.despawned_poppies += *n as u64;
                } else {
                    t.despawned_iron += if it == "iron_block" { 9 * *n as u64 } else { *n as u64 };
                }
            }
        }
        for (id, (it, n, age, q)) in &now {
            if on_floor(*q) && it != "iron_block" {
                t.oldest_on_floor = t.oldest_on_floor.max(*age);
            }
            // (an iron block entity can only be a crafter's output its front container wouldn't take)
            if it == "iron_block" && !last.contains_key(id) {
                t.spilled += *n as u64;
            }
        }
        last = now;
        for q in &b.intake {
            let n: u32 = run.world.containers.map.get(&bp(*q)).map_or(0, |c| c.slots.iter().flatten().map(|s| s.count as u32).sum());
            if n > t.backlog_max {
                t.backlog_max = n;
                t.backlog_at = *q;
            }
        }
    }
    // the end's counts: the containers (less the sorters' recipe stock) and the ground
    for c in run.world.containers.map.values() {
        for s in c.slots.iter().flatten() {
            if s.item == iron {
                t.ingots_held += s.count as u64;
            } else if s.item == iron_block {
                t.blocks_held += s.count as u64;
            } else if s.item == poppy {
                t.poppies_held += s.count as u64;
            }
        }
    }
    t.ingots_held = t.ingots_held.saturating_sub(stock);
    t.blocks_stored = b.chests.iter().map(|q| run.world.containers.map.get(&bp(*q)).map_or(0, |c| c.slots.iter().flatten().filter(|s| s.item == iron_block).map(|s| s.count as u64).sum::<u64>())).sum::<u64>() - stored0;
    t.blocks_held -= held0;
    for (it, n, _, _) in item_counts(&run.world).values() {
        match it.as_str() {
            "iron_ingot" => t.ingots_ground += *n as u64,
            "iron_block" => t.blocks_ground += *n as u64,
            "poppy" => t.poppies_ground += *n as u64,
            _ => {}
        }
    }
    t.poppies_in_crafters = b.crafters.iter().map(|q| run.world.containers.map.get(&bp(*q)).map_or(0, |c| c.slots.iter().flatten().filter(|s| s.item == poppy).map(|s| s.count as u64).sum::<u64>())).sum();
    if o.misset {
        // (the mis-set recipe's own poppies are its stock, not arrivals)
        t.poppies_held = t.poppies_held.saturating_sub(stock);
    }
    // (blocks made: those that went into a container, net, and those spilled into the world)
    t.crafts = t.blocks_held + t.spilled;
    t.errors = run.world.errors[errors0..].iter().take(5).cloned().collect();
    // (what each machine holds at the end: the sorters' A and B, the crafters, their output
    // hoppers, the poppy line and the dropper, the hall's first line hopper)
    let mut dump = Vec::new();
    let names = |w: &redsim::world::World, q: P| -> String {
        w.containers.map.get(&bp(q)).map_or("no container".to_string(), |c| {
            let v: Vec<String> = c.slots.iter().flatten().map(|s| format!("{} {}", rc::item_name(s.item), s.count)).collect();
            if v.is_empty() { "empty".to_string() } else { v.join(", ") }
        })
    };
    for (i, m) in b.crafters.iter().enumerate() {
        let z0 = m[2];
        for (what, q) in [("A", [0, 2, z0]), ("B", [0, 1, z0]), ("crafter", *m), ("its output chest", [1, 0, z0]), ("L", [0, 3, z0])] {
            dump.push(format!("line {i} {what} {q:?}: {} ({:?})", names(&run.world, q), run.world.get(bp(q))));
        }
    }
    for q in [[6, 3, 0], [7, 3, 0], [5, -1, -2], [6, -1, -2]] {
        dump.push(format!("{q:?}: {} ({:?})", names(&run.world, q), run.world.get(bp(q))));
    }
    t.dump = dump;
    let _ = l;
    Ok(t)
}

/// The report of one bench run.
pub fn report(l: &Layout, o: &BenchOpts, t: &Tally) -> String {
    let stored = t.ingots_held + 9 * t.blocks_held;
    let ground = t.ingots_ground + 9 * t.blocks_ground;
    let lost = t.iron_in as i64 - stored as i64 - ground as i64 - t.despawned_iron as i64;
    let disposed = t.poppies_in as i64 - t.poppies_held as i64 - t.poppies_ground as i64 - t.despawned_poppies as i64;
    let load = (t.iron_in + t.poppies_in) as f64 / o.days / l.lines as f64;
    let mut s = format!(
        "storage bench (redsim): {} module(s) ({} golems a game day), {} line(s), {} double chest(s){}{}, {} game days, seed {}\n",
        o.modules,
        o.modules as f64 * GOLEMS_A_DAY,
        l.lines,
        l.chests,
        if o.prefill >= DOUBLE_CHEST { ", every double chest and every container past the crafters prefilled FULL (the full-storage row)".to_string() } else if o.prefill > 0 { format!(", each prefilled with {} iron blocks", o.prefill.min(DOUBLE_CHEST)) } else { String::new() },
        if o.misset { ", the sorters MIS-SET to poppies (control)" } else { "" },
        o.days,
        o.seed
    );
    s.push_str(&format!("  golems {}: iron in {} ingots, poppies in {}\n", t.golems, t.iron_in, t.poppies_in));
    s.push_str(&format!(
        "  iron stored {} (ingots {} in hoppers and crafters + 9 x {} blocks, {} of them in the chests), on the ground {} ({} ingots + 9 x {} blocks), despawned {}, unaccounted {}\n",
        stored, t.ingots_held, t.blocks_held, t.blocks_stored, ground, t.ingots_ground, t.blocks_ground, t.despawned_iron, lost
    ));
    s.push_str(&format!("  poppies disposed {} (held {}, on the ground {}, despawned {}); poppies in a crafter {}\n", disposed, t.poppies_held, t.poppies_ground, t.despawned_poppies, t.poppies_in_crafters));
    s.push_str(&format!("  crafts {} (iron blocks made); iron blocks the crafters spilled into the world (their front container full) {}\n", t.crafts, t.spilled));
    s.push_str(&format!(
        "  intake: largest backlog {} items (at {:?}), the oldest drop on the kill floor {} gt (despawn at 6000); load {:.0} items a game day a line against {:.0} ({:.0}% margin)\n",
        t.backlog_max,
        t.backlog_at,
        t.oldest_on_floor,
        load,
        LINE_A_DAY,
        (LINE_A_DAY / load - 1.0) * 100.0
    ));
    for e in &t.errors {
        s.push_str(&format!("  model note: {e}\n"));
    }
    for d in &t.dump {
        s.push_str(&format!("  at the end: {d}\n"));
    }
    s
}
