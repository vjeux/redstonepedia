//! The carousel's linear stand (vjeux 2026-11-10 05:58: "a single lane where the villagers are
//! riding on back to back with a mechanism to release them 3 by 3"): the station + portal pad +
//! Nether part re-fit to a stand of the release's front 3 carts parked back to back on one track
//! beside the pad (ironfarm owns the queue, the release and the return path).
//!
//! [`design`] writes the bench design: the pad, the Nether part, the stand track with the batch's
//! 3 carts (each a villager), the stand zombies (in boats, out of reach), and the other 18
//! villagers in carts in a far calm hold (out of every search, the stand's 10.6 box and the
//! golems' follow box: the release stubbed). [`check`] runs the judge's golem search of the 3
//! stand riders cell by cell; [`bench`] rotates the 7 batches through the stand by moving their
//! carts (the release stubbed: each batch's carts arrive at the stand 60 gt before its slot) and
//! measures every slot, golem, pulse and Nether death.

use crate::station::{self, CheckReport, PadOpts, RiderSearch};
use anyhow::Result;
use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::ai::brain::{self, Act};
use redsim::entity::mob::farm::{FarmSpec, RunOpts, WalkMode};
use redsim::entity::mob::types::MobType;
use redsim::entity::mob::MobEvent;
use redsim::entity::{Aabb, EntityKind, Vec3};
use redsim::pos::BlockPos;
use redsim::world::World;

/// The batches (7 of 3: vjeux 22:27).
pub const BATCHES: i32 = 7;
/// A batch's riders.
pub const RIDERS: i32 = 3;

/// The stand's shape.
#[derive(Clone, Debug)]
pub struct LinearOpts {
    /// The stand's first cart cell (its rail's block): the 3 carts at x0, x0+1, x0+2 (along x) or
    /// z, z+1, z+2 (along z).
    pub x0: i32,
    pub y: i32,
    pub z: i32,
    pub along_x: bool,
    /// The far hold's z (the 18 other villagers' carts, a track along x).
    pub hold_z: i32,
    /// The stand carts' spacing along the track (1: back to back) and each next cart's drop (a
    /// stand of gate pockets: 3 cells apart, a block lower each).
    pub pitch: i32,
    pub drop: i32,
}

impl Default for LinearOpts {
    fn default() -> LinearOpts {
        LinearOpts { x0: 2, y: 0, z: 0, along_x: true, hold_z: 60, pitch: 1, drop: 0 }
    }
}

impl LinearOpts {
    /// The `linear:` line.
    pub fn meta(&self) -> String {
        format!("x0={} y={} z={} along_x={} hold_z={} pitch={} drop={}", self.x0, self.y, self.z, self.along_x as i32, self.hold_z, self.pitch, self.drop)
    }

    pub fn of(meta: &std::collections::BTreeMap<String, String>) -> Result<LinearOpts> {
        let line = meta.get("linear").ok_or_else(|| anyhow::anyhow!("not a linear-stand design (no `linear:` line)"))?;
        let mut o = LinearOpts::default();
        for kv in line.split_whitespace() {
            let (k, v) = kv.split_once('=').ok_or_else(|| anyhow::anyhow!("bad `linear:` entry {kv:?}"))?;
            let n: i32 = v.parse()?;
            match k {
                "x0" => o.x0 = n,
                "y" => o.y = n,
                "z" => o.z = n,
                "along_x" => o.along_x = n != 0,
                "hold_z" => o.hold_z = n,
                "pitch" => o.pitch = n,
                "drop" => o.drop = n,
                _ => anyhow::bail!("unknown `linear:` entry {k:?}"),
            }
        }
        Ok(o)
    }

    /// Stand cart `i`'s rail cell (`pitch` apart along the track, each `drop` lower; the end
    /// stops at i = -1 and i = 3 one cell past the end carts).
    pub fn cell(&self, i: i32) -> BlockPos {
        let (k, dy) = match i {
            -1 => (-1, 0),
            i if i >= RIDERS => ((RIDERS - 1) * self.pitch + 1, -(RIDERS - 1) * self.drop),
            i => (i * self.pitch, -i * self.drop),
        };
        if self.along_x { BlockPos::new(self.x0 + k, self.y + dy, self.z) } else { BlockPos::new(self.x0, self.y + dy, self.z + k) }
    }

    /// A cart's position on a flat rail at `c` (its feet: the rail's y + 1/16).
    pub fn cart_at(c: BlockPos) -> Vec3 {
        Vec3::new(c.x as f64 + 0.5, c.y as f64 + 0.0625, c.z as f64 + 0.5)
    }

    /// Hold cart `j`'s rail cell (j = 0..18: batch 1 + j / 3's): groups of 3 along x, between
    /// glass stops, out of every search.
    pub fn hold_cell(&self, j: i32) -> BlockPos {
        BlockPos::new(-6 + (j / RIDERS) * (RIDERS + 2) + j % RIDERS, self.y, self.hold_z)
    }

    /// The shared bed (its head): past the far hold.
    pub fn bed(&self) -> BlockPos {
        BlockPos::new(3, self.y, self.hold_z + 6)
    }
}

/// The bench design: the pad and its Nether part ([`station::pad`], [`station::nether_part`]),
/// the stand track (plain rails on glass, a rail beyond each end), batch 0's 3 carts with their
/// villagers, two zombies in boats on glass two cells from the track on its far side from the pad
/// (level with the riders: each sees one within 8), the far hold's track with the other 18
/// villagers' carts, the shared bed.
pub fn design(pad: &PadOpts, o: &LinearOpts) -> Result<String> {
    let mut b = crate::layouts::Builder::new("carousel-linear-stand");
    let glass = "glass";
    for (p, s) in station::pad(pad)? {
        b.set(p[0], p[1], p[2], s.trim_start_matches("minecraft:"));
    }
    // the stand's rails: unpowered powered rails (they brake: the stub's carts stay parked; the
    // release's own stops are ironfarm's), a glass stop past each end at cart and rider height
    let rail = if o.along_x { "powered_rail[powered=false,shape=east_west,waterlogged=false]" } else { "powered_rail[powered=false,shape=north_south,waterlogged=false]" };
    for i in 0..RIDERS {
        let c = o.cell(i);
        anyhow::ensure!(b.d.blocks.get(&[c.x, c.y, c.z]).is_none(), "the stand's rail at {c} meets the pad");
        b.set(c.x, c.y, c.z, rail);
        b.set(c.x, c.y - 1, c.z, glass);
    }
    for i in [-1, RIDERS] {
        let c = o.cell(i);
        for dy in 0..=1 {
            b.set(c.x, c.y + dy, c.z, glass);
        }
    }
    for i in 0..RIDERS {
        b.entity("minecart", {
            let p = LinearOpts::cart_at(o.cell(i));
            [p.x, p.y, p.z]
        }, Some(if o.along_x { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[90f,0f],Passengers:[{id:\"minecraft:villager\"}]}" }));
    }
    // the stand zombies: on the track's far side from the pad, 2 cells off, beside the first and
    // the last cart (a boat on glass: the zombie sits 0.51 below the boat, its eye near the riders')
    let (c0, c2) = (o.cell(0), o.cell(RIDERS - 1));
    let spots: Vec<[i32; 3]> = if o.along_x { vec![[c0.x, o.y, o.z + 2], [c2.x, o.y, o.z + 2]] } else { vec![[o.x0 + 2, o.y, c0.z], [o.x0 + 2, o.y, c2.z]] };
    for s in &spots {
        b.set(s[0], s[1] - 1, s[2], glass);
        b.set(s[0], s[1] + 3, s[2], "oak_leaves[distance=1,persistent=true,waterlogged=false]");
        b.entity("oak_boat", [s[0] as f64 + 0.5, s[1] as f64, s[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    }
    // the far hold: the other 18 villagers in carts, calm (no zombie), each batch's 3 carts on
    // braking rails between glass stops
    for g in 0..BATCHES - 1 {
        let base = -6 + g * (RIDERS + 2);
        for dx in -1..=RIDERS {
            let c = BlockPos::new(base + dx, o.y, o.hold_z);
            if dx < 0 || dx == RIDERS {
                b.set(c.x, c.y, c.z, glass);
                b.set(c.x, c.y + 1, c.z, glass);
            } else {
                b.set(c.x, c.y, c.z, "powered_rail[powered=false,shape=east_west,waterlogged=false]");
            }
            b.set(c.x, c.y - 1, c.z, glass);
        }
    }
    for j in 0..RIDERS * (BATCHES - 1) {
        let p = LinearOpts::cart_at(o.hold_cell(j));
        b.entity("minecart", [p.x, p.y, p.z], Some("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}"));
    }
    let bed = o.bed();
    b.set(bed.x, bed.y, bed.z, "red_bed[facing=north,occupied=false,part=head]");
    b.set(bed.x, bed.y, bed.z + 1, "red_bed[facing=north,occupied=false,part=foot]");
    b.set(bed.x, bed.y - 1, bed.z, glass);
    b.set(bed.x, bed.y - 1, bed.z + 1, glass);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    // the build's listed steps (brief farm-iron-carousel, "built in order"): the sorter's stocks
    // after every block. Filled as their hoppers are placed (the default order), the lock hoppers
    // are not locked yet (their torches come with the redstone, later): the iron lock hopper passes
    // its 64 iron to the crafter and the filter hopper refills it, 38 of its 41 iron gone by the
    // end of the build (one iron block in the double chest); filled after the redstone, each lock
    // hopper is locked when its stock goes in, and the lock before its filter (bottom up).
    b.d.meta.insert("build".into(), crate::placing::BuildPlan { last: Vec::new(), fills_last: true }.meta());
    // (the declared states, `crate::declared`: the stand's 3 rails and the far hold's 18 are
    // unpowered powered rails, brakes that keep the stub's carts parked, never to be powered (`!`:
    // checked after the warm-up too); no flights: the bench moves the carts, none leaves a rail)
    let mut decl: Vec<crate::declared::Decl> = (0..RIDERS).map(|i| {
        let c = o.cell(i);
        crate::declared::Decl::new([c.x, c.y, c.z], "powered_rail", &[("powered", "false")], true).bench_set()
    }).collect();
    for g in 0..BATCHES - 1 {
        let base = -6 + g * (RIDERS + 2);
        for dx in 0..RIDERS {
            decl.push(crate::declared::Decl::new([base + dx, o.y, o.hold_z], "powered_rail", &[("powered", "false")], true).bench_set());
        }
    }
    b.d.meta.insert("declared".into(), crate::declared::format(&decl));
    b.d.meta.insert("flights".into(), String::new());
    b.d.ports.insert("output".into(), [bed.x + 2, bed.y, bed.z]);
    let text = b.text(&[
        &format!("The carousel's station + portal pad on the linear stand (bench): the release's front {RIDERS} carts back to back on one track {} at y {}, the pad's {} planes at z {:?},", if o.along_x { format!("along x from x {} at z {}", o.x0, o.z) } else { format!("along z from z {} at x {}", o.z, o.x0) }, o.y, pad.planes.len(), pad.planes),
        "the Nether part's kill and collection; the other 18 villagers in a far calm hold (the release stubbed: the bench moves each batch's carts to the stand).",
    ]);
    let mut d = Design::parse(&text)?;
    let (nether, nether_bes, out) = station::nether_part();
    for (p, s) in nether {
        d.nether_blocks.insert(p, s);
    }
    for (p, s) in nether_bes {
        d.nether_block_entities.insert(p, s);
    }
    d.nether_ports.insert("nether-output".into(), out);
    d.ports.insert("nether-anchor".into(), [0, 0, 0]);
    d.ports.insert("nether-axis".into(), [1, 0, 0]);
    d.meta.insert("station".into(), pad.meta());
    d.meta.insert("linear".into(), o.meta());
    d.ports.insert("output.pulse".into(), pad.pulse());
    d.ports.insert("home".into(), [bed.x, bed.y, bed.z]);
    let mut s = String::new();
    for l in text.lines().take_while(|l| l.starts_with('#')) {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str(&d.to_text());
    Ok(s)
}

/// The pad and its Nether part alone, from a written (settled) bench design: the cells the pad
/// owns ([`station::pad`]) as the file writes them, the whole Nether part, the ports another
/// design needs (`output.pulse`, the Nether's anchor, axis and output) and the build steps; no
/// stand track, zombies, hold, bed or entities. The part a loop pastes under its stand (the
/// pocket stand's riders at x -1, 3, 7: `mobfarm linear --stand=-1,0,0 --pitch 4 --drop 1`).
pub fn alone(text: &str, pad: &PadOpts) -> Result<String> {
    let full = Design::parse(text)?;
    let cells: std::collections::BTreeSet<[i32; 3]> = station::pad(pad)?.into_iter().map(|(p, _)| p).collect();
    let mut d = full.clone();
    d.name = "carousel-station-pad".into();
    d.blocks.retain(|p, _| cells.contains(p));
    d.block_entities.retain(|p, _| cells.contains(p));
    d.entities.clear();
    d.meta.remove("linear");
    for k in ["home", "output"] {
        d.ports.remove(k);
    }
    // (the pad alone has no rail, gate, booster or detector, and no flight: both lines empty; the
    // stand bench's brakes stay out of it)
    d.meta.insert("declared".into(), String::new());
    d.meta.insert("flights".into(), String::new());
    let pulse = pad.pulse();
    let mut s = String::new();
    s.push_str(&format!("# The carousel station's portal pad and its Nether part, alone: {} planes at z {:?} (frames x {}..{}, bottom row y {}, portals {} tall), a trench row on each side, a tripwire line on each frame's top row;\n", pad.planes.len(), pad.planes, pad.x0, pad.x1, pad.by, pad.h));
    s.push_str(&format!("# the golem pulse out at the dust ({},{},{}) (`port output.pulse`: on 1 gt after a golem's spawn, 10 gt long); the Nether part's lava kill, hopper floor, sorter, iron-block crafter and double chest. Paste it at the loop's origin under the pocket stand.\n", pulse[0], pulse[1], pulse[2]));
    s.push_str(&d.to_text());
    Ok(s)
}
fn stand_boxes(o: &LinearOpts) -> Vec<Aabb> {
    let mut out = Vec::new();
    for i in 0..RIDERS {
        let c = LinearOpts::cart_at(o.cell(i));
        out.push(redsim::entity::make_bounding_box(c, 0.98, 0.7));
        // (the rider sits about half a block up the cart)
        out.push(redsim::entity::make_bounding_box(Vec3::new(c.x, c.y + 0.5, c.z), 0.6, 1.95));
    }
    out
}

/// The static check: each stand rider's golem search (from its cart's cell, the judge's
/// function), every spot classed by portal contact and against the batch's own parked boxes;
/// the held riders' searches (any spot there fails); every box a cell clear of the portals.
pub fn check(spec: &FarmSpec, pad: &PadOpts, o: &LinearOpts) -> Result<CheckReport, String> {
    let run = station::world_of(spec, 1)?;
    let w = &run.world;
    let mut rep = CheckReport::default();
    let parked = stand_boxes(o);
    for i in 0..RIDERS {
        let start = o.cell(i);
        let mut r = RiderSearch { lane: 0, level: i, cell: [start.x, start.y, start.z], ..Default::default() };
        for (cell, res) in redsim::entity::mob::spawn::golem_spawn_cells_from(w, start) {
            if res.is_err() {
                continue;
            }
            let bb = station::golem_box(cell);
            let c = [cell.x, cell.y, cell.z];
            if station::portal_cells_met(w, bb).is_empty() {
                r.stray.push(c);
            } else {
                r.valid.push(c);
                if parked.iter().any(|p| p.intersects(bb)) {
                    r.obstructed.push(c);
                }
            }
        }
        rep.riders.push(r);
    }
    for j in 0..RIDERS * (BATCHES - 1) {
        for (cell, res) in redsim::entity::mob::spawn::golem_spawn_cells_from(w, o.hold_cell(j)) {
            if res.is_ok() && !rep.held_strays.contains(&[cell.x, cell.y, cell.z]) {
                rep.held_strays.push([cell.x, cell.y, cell.z]);
            }
        }
    }
    for e in &w.entities.list {
        if let Some(g) = station::portal_gap(w, e.bb)
            && g < 1.0
        {
            rep.near_portal.push((format!("{} at {:.2} {:.2} {:.2}", station::kind_name(&e.kind), e.pos.x, e.pos.y, e.pos.z), g));
        }
    }
    let blocks = station::pad(pad).map_err(|e| e.to_string())?;
    rep.pad_blocks = blocks.len();
    rep.portal_cells = blocks.iter().filter(|(_, s)| s.contains("nether_portal")).count();
    rep.obsidian = blocks.iter().filter(|(_, s)| s.ends_with("obsidian")).count();
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for (p, _) in &blocks {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    rep.pad_box = (lo, hi);
    Ok(rep)
}

/// The linear bench's options.
#[derive(Clone, Debug)]
pub struct BenchOpts {
    pub seed: i64,
    pub days: u32,
    /// Each batch's carts reach the stand this many gt before its slot (the release stubbed).
    pub lead: i64,
    /// Record the drive (`--drive-out`).
    pub drive: bool,
    /// How the design is put in the world (the brief's placement checks: mobfarm::placing).
    pub placement: crate::placing::Placement,
    /// The design's listed build steps (its `build:` line), for a build in order.
    pub plan: crate::placing::BuildPlan,
    /// The design's declared states (its `declared:` line, `crate::declared`): checked once the
    /// design is in the world and the always-on (`!`) ones again 20 gt later; a mismatch is a
    /// violation.
    pub declared: Vec<crate::declared::Decl>,
    /// The design's declared flights (its `flights:` line, `crate::flight`): every cart is watched
    /// every tick, and an off-rail stretch that is no declared flight is a violation.
    pub flights: Vec<crate::flight::Flight>,
}

/// What the linear bench measured.
#[derive(Clone, Debug, Default)]
pub struct BenchReport {
    pub drive: Option<String>,
    /// Built in order: the cells the build left unlike the file (none: the build ends as written),
    /// and the build's steps, cells and ticks.
    pub built_diff: Vec<String>,
    pub build: Option<(usize, usize, i64)>,
    pub slots: u32,
    pub summoned: u32,
    pub willing: Vec<u8>,
    pub panicking: Vec<u8>,
    pub tries: Vec<u8>,
    pub ok: Vec<bool>,
    pub dwell: Vec<i64>,
    pub pulse: Vec<(Option<i64>, i64)>,
    pub spurious_pulses: u32,
    pub stray: Vec<String>,
    pub nether_deaths: u32,
    pub nether_alive: u32,
    pub returned: u32,
    pub violations: Vec<String>,
    pub violation_count: u32,
    pub errors: Vec<String>,
    /// The declared states the world didn't match (at the start, or after the warm-up).
    pub declared_bad: Vec<String>,
    /// Off-rail stretches that were no declared flight, and the declared flights that landed.
    pub derails: Vec<String>,
    pub flights_landed: usize,
}

/// Move a cart (and its rider) to a stand or hold cell, at rest.
fn move_cart(w: &mut World, cart: u32, c: BlockPos) {
    let Some(i) = w.entities.list.iter().position(|e| e.id == cart && !e.removed) else { return };
    let p = LinearOpts::cart_at(c);
    w.entities.set_pos(i, p);
    w.entities.list[i].delta_movement = Vec3::new(0.0, 0.0, 0.0);
    let passengers = w.entities.list[i].passengers.clone();
    for pid in passengers {
        if let Some(j) = w.entities.list.iter().position(|e| e.id == pid && !e.removed) {
            redsim::entity::position_rider(w, i, j);
        }
    }
}

/// The bench: the 7 batches through the stand on the 100-gt slot grid (batch n % 7 at slot n),
/// each batch's carts moved to the stand `lead` gt before its slot and back to the hold 10 gt
/// after it, the sleep routine's ideal stand-in (LAST_SLEPT once a night); every slot's willing
/// and panicking riders, tries and golem; every golem's stay, contact and pulse; the Nether.
pub fn bench(spec: &FarmSpec, pad: &PadOpts, o: &LinearOpts, bo: &BenchOpts) -> Result<BenchReport, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(bo.seed, bo.days * 24000, WalkMode::Random) };
    // (the judge's placement and the settle; the raw paste; or built in order)
    let (mut run, built) = crate::placing::start(spec, &ro, bo.placement, &bo.plan)?;
    let built_diff = if built.is_some() { crate::placing::diff_from_file(&run.world, spec, &ro)? } else { Vec::new() };
    let first = run.world.entities.next_id;
    let mut spawned = Vec::new();
    for (ty, pos, nbt) in &spec.entities {
        spawned.push(redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?);
    }
    run.own = (first..run.world.entities.next_id).collect();
    let bed = o.bed();
    let t0 = run.world.time;
    // (the drive's times count from the spawn, or from the build's start when built in order)
    let origin = built.as_ref().map_or(t0, |b| b.start);
    let mut drive = crate::drive::Drive::new(bo.drive);
    drive.name_entities(&run.world, &spawned, origin);
    for l in built.iter().flat_map(|b| b.lines.iter()) {
        drive.push(l.clone());
    }
    let drive = drive;
    redsim::entity::mob::ai::poi::take(&mut run.world, bed);
    drive.act(t0, format!("poi-take {} {} {}", bed.x, bed.y, bed.z));
    // the carts, by their starting cell: batch 0 at the stand, batch 1 + j / 3 at hold cell j
    let mut batches: Vec<Vec<(u32, u32)>> = vec![Vec::new(); BATCHES as usize];
    for i in 0..run.world.entities.list.len() {
        let e = &run.world.entities.list[i];
        if !matches!(e.kind, EntityKind::Minecart(..)) || e.passengers.is_empty() {
            continue;
        }
        let (cart, rider) = (e.id, e.passengers[0]);
        let cell = BlockPos::new(e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32);
        let b = if (0..RIDERS).any(|k| o.cell(k) == cell) {
            0
        } else if let Some(j) = (0..RIDERS * (BATCHES - 1)).find(|&j| o.hold_cell(j) == cell) {
            1 + j / RIDERS
        } else {
            return Err(format!("a cart at {cell} is neither at the stand nor in the hold"));
        };
        batches[b as usize].push((cart, rider));
    }
    for (b, v) in batches.iter_mut().enumerate() {
        if v.len() != RIDERS as usize {
            return Err(format!("batch {b} has {} carts", v.len()));
        }
        v.sort_by_key(|(c, _)| *c);
    }
    let index_of = |w: &World, id: u32| w.entities.list.iter().position(|e| e.id == id && !e.removed);
    // (in the design's entity order, as the drive names them)
    let mut villagers: Vec<u32> = batches.iter().flatten().map(|(_, r)| *r).collect();
    villagers.sort();
    for r in &villagers {
        if let Some(i) = index_of(&run.world, *r) {
            let br = brain::brain_mut(&mut run.world, i);
            br.home = Some(bed);
            br.last_slept = Some(t0 - 100);
            drive.act(t0, format!("home {} {} {} {}", drive.name(*r), bed.x, bed.y, bed.z));
            drive.act(t0, format!("last-slept {} {}", drive.name(*r), t0 - 100 - origin));
        }
    }
    let mut rep = BenchReport::default();
    let violate = |rep: &mut BenchReport, t: i64, what: String| {
        rep.violation_count += 1;
        if rep.violations.len() < 12 {
            rep.violations.push(format!("gt {t}: {what}"));
        }
    };
    // (the declared states, once the design is in the world (as placed, settled or built), the
    // always-on ones again at t0 + 20; every cart watched for off-rail stretches against the
    // declared flights)
    for b in crate::declared::check(&run.world, &bo.declared, false) {
        rep.declared_bad.push(b.clone());
        violate(&mut rep, t0, format!("declared state: {b}"));
    }
    let mut rail_watch = crate::flight::RailWatch::new(bo.flights.clone());
    let carts_watched: Vec<(u32, u32)> = batches.iter().flatten().copied().collect();
    let end = t0 + bo.days as i64 * 24000;
    // (the first slot after the spawn: batch 0 starts at the stand, so it needs no lead; the slots
    // after it move their batch in `lead` gt before. A start no settle or build rounds to 200 can
    // fall within the lead of the next hundred, whose summons are batch 0's)
    let s0 = (t0 / 100 + 1) * 100;
    // the stand's batch, each held batch's hold slot (its 3 cells), and the slot each batch
    // visits next
    let mut at_stand: usize = 0;
    let mut hold_of: Vec<i32> = (0..BATCHES).map(|b| (b - 1).max(0)).collect();
    let mut slot_n: i64 = 0;
    let mut slept_night: Option<i64> = None;
    let mut ev_seen = run.world.mob.events.len();
    let mut crossed_seen = run.world.crossings.len();
    let pulse_at = pad.pulse();
    let mut golems: Vec<(u32, i64, Option<i64>, Option<i64>)> = Vec::new();
    let mut nether: Vec<(u32, Option<i64>)> = Vec::new();
    let mut returned: std::collections::BTreeSet<u32> = Default::default();
    let mut pulse_on: Option<i64> = None;
    let mut slot_tries: Option<(i64, Vec<u32>, u8, bool)> = None;
    while run.world.time < end {
        let t_next = run.world.time + 1;
        let night_of = run.world.day_time() / 24000;
        if run.world.day_time() % 24000 >= 13000 && slept_night != Some(night_of) {
            slept_night = Some(night_of);
            let now = run.world.time;
            for r in &villagers {
                if let Some(i) = index_of(&run.world, *r) {
                    brain::brain_mut(&mut run.world, i).last_slept = Some(now);
                    drive.act(now, format!("last-slept {} {}", drive.name(*r), now - origin));
                }
            }
        }
        // the release stubbed: the next slot's batch to the stand `lead` gt before it, the batch
        // there back to its hold cells
        let slot = s0 + slot_n * 100;
        if t_next == slot - bo.lead {
            let next = (slot_n % BATCHES as i64) as usize;
            if next != at_stand {
                // the leaving batch takes the arriving batch's hold cells
                let h = hold_of[next];
                let now = run.world.time;
                for (k, (cart, _)) in batches[at_stand].clone().iter().enumerate() {
                    let c = o.hold_cell(h * RIDERS + k as i32);
                    move_cart(&mut run.world, *cart, c);
                    let p = LinearOpts::cart_at(c);
                    drive.act(now, format!("tp {} {} {} {}", drive.name(*cart), p.x, p.y, p.z));
                }
                for (k, (cart, _)) in batches[next].clone().iter().enumerate() {
                    let c = o.cell(k as i32);
                    move_cart(&mut run.world, *cart, c);
                    let p = LinearOpts::cart_at(c);
                    drive.act(now, format!("tp {} {} {} {}", drive.name(*cart), p.x, p.y, p.z));
                }
                hold_of[at_stand] = h;
                at_stand = next;
            }
        }
        if t_next == slot {
            let w = &run.world;
            if std::env::var("MCTAS_LIN_DEBUG").is_ok() && slot_n < 12 {
                for (cart, r) in &batches[at_stand] {
                    let ci = index_of(w, *cart);
                    let ri = index_of(w, *r);
                    let cp = ci.map(|i| w.entities.list[i].pos);
                    let (rp, info) = match ri {
                        Some(i) => match &w.entities.list[i].kind {
                            EntityKind::Mob(m) => (Some(w.entities.list[i].pos), m.brain.as_deref().map(|b| format!("slept {:?} golem_detected {:?} scared {} vehicle {:?}", b.last_slept.map(|s| t_next - s), b.golem_detected, b.scared(), w.entities.list[i].vehicle))),
                            _ => (None, None),
                        },
                        None => (None, None),
                    };
                    eprintln!("slot {slot_n} batch {at_stand}: cart {cart} at {cp:?} rider {r} at {rp:?} {info:?}");
                }
            }
            let (mut want, mut panic) = (0u8, 0u8);
            let ids: Vec<u32> = batches[at_stand].iter().map(|(_, r)| *r).collect();
            for id in &ids {
                let Some(i) = index_of(w, *id) else { continue };
                if let EntityKind::Mob(m) = &w.entities.list[i].kind
                    && m.brain.as_deref().is_some_and(|b| b.wants_golem(t_next))
                {
                    want += 1;
                }
                if brain::has_exact(w, i) && brain::ex(w, i).active == Act::Panic {
                    panic += 1;
                }
            }
            rep.willing.push(want);
            rep.panicking.push(panic);
            slot_tries = Some((t_next, ids, 0, false));
            slot_n += 1;
        }
        run.tick();
        let t = run.world.time;
        if t == t0 + 20 {
            for b in crate::declared::check(&run.world, &bo.declared, true) {
                rep.declared_bad.push(b.clone());
                violate(&mut rep, t, format!("declared state after the warm-up: {b}"));
            }
        }
        for (k, (cart, rider)) in carts_watched.iter().enumerate() {
            rail_watch.observe(&run.world, k, *cart, Some(*rider), &format!("gt {t}"));
        }
        // (the runtime check, `crate::declared::check_runtime`, every tick: a bench-set brake (no
        // control line) powered by any neighbour, or a controlled rail powered while its control
        // is off, fails the run; each cell reported once)
        for b in crate::declared::check_runtime(&run.world, &bo.declared) {
            if !rep.declared_bad.contains(&b) {
                rep.declared_bad.push(b.clone());
                violate(&mut rep, t, format!("runtime: {b}"));
            }
        }
        let evs: Vec<MobEvent> = run.world.mob.events[ev_seen.min(run.world.mob.events.len())..].to_vec();
        ev_seen = run.world.mob.events.len();
        for ev in evs {
            if let MobEvent::GolemTry { t: et, villager, golem, at, .. } = ev {
                if let Some((st, ids, n, ok)) = slot_tries.as_mut()
                    && *st == et
                    && ids.contains(&villager)
                {
                    *n += 1;
                    *ok |= golem.is_some();
                }
                if let (Some(id), Some(c)) = (golem, at) {
                    let cell = BlockPos::new(c[0], c[1], c[2]);
                    if station::portal_cells_met(&run.world, station::golem_box(cell)).is_empty() {
                        rep.stray.push(format!("gt {et}: a golem at {c:?} touches no portal"));
                    }
                    if !slot_tries.as_ref().is_some_and(|(st, ids, _, _)| *st == et && ids.contains(&villager)) {
                        violate(&mut rep, et, format!("a golem summoned by villager {villager}, not the stand batch's"));
                    }
                    golems.push((id, et, None, None));
                    drive.expect(et, format!("golem g{} {} {} {}", drive.golem(id), c[0], c[1], c[2]));
                }
            }
        }
        // each golem's stay, the pulse's rise and fall
        let p = station::wire_power(&run.world, pulse_at);
        match (p > 0, pulse_on) {
            (true, None) => {
                pulse_on = Some(t);
                match golems.iter_mut().rev().find(|g| g.2.is_none() && t - g.1 <= 5) {
                    Some(g) => g.2 = Some(t),
                    None => rep.spurious_pulses += 1,
                }
            }
            (false, Some(on)) => {
                pulse_on = None;
                if let Some(g) = golems.iter_mut().rev().find(|g| g.2 == Some(on)) {
                    g.3 = Some(t - on);
                    drive.expect(on, format!("pulse g{} {}", drive.golem(g.0), t - on));
                }
            }
            _ => {}
        }
        for &(ct, from, to) in &run.world.crossings[crossed_seen.min(run.world.crossings.len())..] {
            if golems.iter().any(|g| g.0 == from) {
                nether.push((to, None));
                drive.expect(ct, format!("cross g{}", drive.golem(from)));
                drive.alias(to, from);
                if let Some(g) = golems.iter().find(|g| g.0 == from) {
                    rep.dwell.push(ct - g.1);
                }
            }
        }
        crossed_seen = run.world.crossings.len();
        if let Some(n) = run.world.nether.as_deref() {
            for g in nether.iter_mut().filter(|g| g.1.is_none()) {
                if !n.entities.list.iter().any(|e| e.id == g.0 && !e.removed) {
                    g.1 = Some(t);
                    drive.expect(t, format!("nether-death g{}", drive.golem(g.0)));
                }
            }
        }
        for e in &run.world.entities.list {
            if !e.removed
                && let EntityKind::Mob(m) = &e.kind
                && m.ty == MobType::IronGolem
                && !golems.iter().any(|g| g.0 == e.id)
                && returned.insert(e.id)
            {
                violate(&mut rep, t, format!("a golem back in the overworld at {:.2} {:.2} {:.2}", e.pos.x, e.pos.y, e.pos.z));
            }
        }
        // a golem still in the overworld 20 gt after its spawn
        for g in &golems {
            if t - g.1 == 20 && run.world.entities.list.iter().any(|e| e.id == g.0 && !e.removed) {
                violate(&mut rep, t, format!("golem {} still in the overworld 20 gt after its spawn", g.0));
            }
        }
        // the slot's result, 20 gt after it
        if let Some((st, _, n, ok)) = slot_tries.as_ref()
            && t == *st + 20
        {
            rep.tries.push(*n);
            rep.slots += 1;
            rep.ok.push(*ok);
            if *ok {
                rep.summoned += 1;
            }
            slot_tries = None;
        }
    }
    // (a slot or a pulse the run's end cut short is not counted)
    rep.willing.truncate(rep.tries.len());
    rep.panicking.truncate(rep.tries.len());
    for g in golems.iter().filter(|g| g.1 <= end - 15) {
        rep.pulse.push((g.2.map(|r| r - g.1), g.3.unwrap_or(0)));
    }
    rep.nether_deaths = nether.iter().filter(|g| g.1.is_some()).count() as u32;
    rep.nether_alive = nether.iter().filter(|g| g.1.is_none()).count() as u32;
    rep.returned = returned.len() as u32;
    // (off-rail stretches that are no declared flight: each a violation)
    for d in &rail_watch.derails {
        violate(&mut rep, end, format!("derail: {d}"));
    }
    rep.derails = rail_watch.derails.clone();
    rep.flights_landed = rail_watch.landed;
    rep.errors = run.world.errors.iter().filter(|e| !e.contains("zombified piglin")).take(6).cloned().collect();
    if bo.drive {
        // the iron the Nether part's containers hold beyond the design's own stock, and loose
        let (mut chest, mut stock) = (0u32, 0u32);
        if let Some(n) = run.world.nether.as_deref() {
            use redsim::redstone::container as rc;
            let (ingot, block) = (rc::item_id("iron_ingot"), rc::item_id("iron_block"));
            for c in n.containers.map.values() {
                for s in c.slots.iter().flatten() {
                    if Some(s.item) == ingot {
                        chest += s.count as u32;
                    } else if Some(s.item) == block {
                        chest += 9 * s.count as u32;
                    }
                }
            }
            stock = 41 + 64;
        }
        drive.expect_end(format!("golems {}", golems.len()));
        drive.expect_end(format!("nether-deaths {}", rep.nether_deaths));
        drive.expect_end(format!("iron-in-nether-containers {} (the sorter's stock of {stock} included)", chest));
        let flag = match bo.placement {
            crate::placing::Placement::Judge => String::new(),
            crate::placing::Placement::Raw => " --placed-raw".into(),
            crate::placing::Placement::Built { gap: None } => " --placed-in-order".into(),
            crate::placing::Placement::Built { gap: Some(g) } => format!(" --placed-in-order {g}"),
        };
        let start = match bo.placement {
            crate::placing::Placement::Judge => "start Run::start(spec, RunOpts { ai: true, ..RunOpts::steady(seed, days * 24000, WalkMode::Random) }); settle_tick until false; the design's entity lines spawned in order (eN: line N from 0, eN.pK: its passenger K from 0); t0 = the world's time then",
            crate::placing::Placement::Raw => "start Run::start(spec, RunOpts { ai: true, raw: true, ..RunOpts::steady(seed, days * 24000, WalkMode::Random) }) (the file pasted as written, the Nether part too, no settle); the design's entity lines spawned in order (eN: line N from 0, eN.pK: its passenger K from 0); t0 = the world's time then",
            crate::placing::Placement::Built { .. } => "start Run::start(spec with no blocks, RunOpts { ai: true, ..RunOpts::steady(seed, days * 24000, WalkMode::Random) }) and an empty Nether world, the build (the `build:` line), then the design's entity lines spawned in order (eN: line N from 0, eN.pK: its passenger K from 0); t0 = the world's time then",
        };
        let mut header = vec![
            format!("drive v1: mctas mobfarm linear-bench <design> --from {s} --to {s} --days {} --lead {}{flag} --drive-out F", bo.days, bo.lead, s = bo.seed),
            // (mobviewer's badge line, 2026-11-10: the view shows it first while the run plays)
            "caption batches teleported to the stand by the test bench (the release is stubbed)".into(),
            format!("seed {}", bo.seed),
            format!("days {}", bo.days),
            "walk random".into(),
            "ai on".into(),
            start.into(),
            if built.is_some() { "act t: applied when the world's time is the build's start + t, before the run's next tick (the bench's own order: at the top of its loop)".into() } else { "act t: applied when the world's time is t0 + t, before the run's next tick (the bench's own order: at the top of its loop)".into() },
            format!("t0 {}", t0),
        ];
        header.extend(crate::placing::header(bo.placement, built.as_ref(), t0, &bo.plan));
        rep.drive = Some(drive.text(&header));
    }
    rep.built_diff = built_diff;
    rep.build = built.as_ref().map(|b| (b.steps, b.cells, t0 - b.start));
    Ok(rep)
}
