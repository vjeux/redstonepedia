//! `mctas mobfarm trim <design>`: a local optimizer for any mob-farm design (vjeux 2026-11-08
//! 20:59: "optimize every single part of the system during the night"; brief farm-iron rev 9
//! iteration 20).
//!
//! At the design's own spot (its terrain site's offset, kept fixed while it trims), outer shell
//! first, it tries:
//! * removing each placed structural block (in terrain the natural block stays, in the void the
//!   cell is air) and refilling each dug cell (the natural rock stays);
//! * swapping each full block for a cheaper full block at the design's stage (prices.tsv:
//!   cobblestone);
//!
//! in two classes:
//! * A, the build's shape unchanged: a placed full block over natural full rock (no sand or
//!   gravel), or a full opaque block swapped: every entity moves and sees as before, so only the
//!   price is checked (never a block a golem spawns on: off the farm's own floors the site caps
//!   the cell);
//! * B, the shape changes, away from the golems' space (the open cells connected to the golem
//!   spawn cells, and their neighbours: the pad, the drop, the kill stay as designed) and from the
//!   fluids (no cell opened beside water or lava, no refill where water can flow): kept only when
//!   the static checks pass as on the design: the refusals (`refuse::static_checks_in`: each
//!   villager in its one cell on its bed's reach, the zombie in its vehicle out of reach and out of
//!   the sky, lava that cannot flow, no water on lava, gossip, the villagers' roofs), the calm's
//!   sight lines (each villager's view of its zombie, standing and asleep, the hide shut and open),
//!   each villager's golem spawn cells (from its cell and the cell above: none lost; the caps and
//!   seals recomputed at the spot), the player's way from the chest up (as high as before), the
//!   surface rule (no new block at or over the ground layer), the hostile flood's breaking
//!   channels (no new one), and the build's price (the stage's build seconds with travel:
//!   `circuit::mobfarm::site_bill`) going down.
//!
//! Then the trimmed design passes the full static checks with the golem exit flood (no escape, no
//! new stall) and short AI-on runs against the design's (the same seeds and days, many of them:
//! the mean rate within [`RATE_TOLERANCE`], no failed run; d1u-deep's first trim, its spiral's
//! slab steps gone, passed every static check yet lost 11% of its summons in the judge); on a miss
//! the edits are bisected and the culprits dropped. It prints blocks placed, dug and build minutes
//! before -> after and the edits kept; the card (iron per hour, T, the gate) comes from the judge
//! in `mctas site variation`.

use anyhow::{Result, anyhow};
use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{self as rfarm, FarmSpec, RunOpts, WalkMode};
use redsim::entity::mob::site::{self, Priced, Site};
use redsim::entity::mob::types::MobType;
use redsim::entity::{Aabb, EntityKind, Vec3};
use redsim::{BlockPos, World};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

type Pos = [i32; 3];

/// The short runs' mean rate a trimmed design may lose against the design's (192 one-day runs on
/// the same seeds: about 2% standard error on the difference).
pub const RATE_TOLERANCE: f64 = 0.03;

/// `mctas mobfarm trim`'s options.
pub struct TrimArgs {
    pub design: PathBuf,
    pub out: PathBuf,
    pub threads: usize,
    /// The short AI-on runs: game days each, and how many.
    pub days: f64,
    pub runs: u32,
}

/// Where a design is priced and checked: its terrain site's offset (fixed while it trims).
struct Place {
    sample: Option<&'static redsim::terrain::samples::Sample>,
    world: Option<Arc<RwLock<World>>>,
    offset: [i32; 3],
    ground: i32,
    afk: [f64; 3],
    clear_box: bool,
    stage: usize,
}

/// What an edit must keep, and what the build pays.
#[derive(Clone, Debug, Default)]
struct Facts {
    seconds: f64,
    placed: u32,
    dug: u32,
    caps: usize,
    seals: usize,
    surface: usize,
    breaks: BTreeSet<String>,
    sight: Vec<bool>,
    spawn: Vec<usize>,
    climb: f64,
    stalls: usize,
    worst_life: i64,
}

impl Facts {
    /// Every check as on the design (`base`), and cheaper than `now`.
    fn keeps(&self, base: &Facts, now: &Facts) -> Result<(), String> {
        if self.sight != base.sight {
            return Err("a sight line of the calm changed".into());
        }
        if self.spawn.len() != base.spawn.len() || self.spawn.iter().zip(&base.spawn).any(|(a, b)| a < b) {
            return Err("a villager's golem search lost spawn cells".into());
        }
        if let Some(b) = self.breaks.difference(&base.breaks).next() {
            return Err(format!("a new hostile break: {b}"));
        }
        if self.surface > base.surface {
            return Err("a block on the natural surface".into());
        }
        if self.climb < base.climb - 1e-6 {
            return Err("the way up from the chest is cut".into());
        }
        if self.stalls > base.stalls || self.worst_life > base.worst_life + 20 {
            return Err("a golem's walk stalls or lives longer".into());
        }
        if self.seconds >= now.seconds - 1e-6 {
            return Err("no cheaper".into());
        }
        Ok(())
    }

    fn line(&self) -> String {
        format!("{} placed ({} caps, {} seals), {} dug, {:.1} build minutes, the climb from the chest to y {:.1}", self.placed, self.caps, self.seals, self.dug, self.seconds / 60.0, self.climb)
    }
}

/// An edit of a design cell: unlisted (its block removed: the natural block or air stays; a dug
/// cell refilled: the natural rock stays), or swapped for a cheaper block.
#[derive(Clone, Debug, PartialEq)]
enum Edit {
    Unlist(Pos),
    Swap(Pos, String),
}

fn apply(d: &Design, edits: &[Edit]) -> Design {
    let mut d = d.clone();
    for e in edits {
        match e {
            Edit::Unlist(p) => {
                d.blocks.remove(p);
            }
            Edit::Swap(p, s) => {
                d.blocks.insert(*p, s.clone());
            }
        }
    }
    d
}

/// The blocks a trim may remove: structure only (the farm's working parts, fluids, light and the
/// calm's blocks are never touched).
fn removable(state: &str) -> bool {
    let id = state.split('[').next().unwrap_or("").trim_start_matches("minecraft:");
    matches!(id, "cobblestone" | "stone" | "dirt" | "coarse_dirt" | "grass_block" | "glass" | "cobblestone_slab" | "stone_slab" | "stone_bricks" | "andesite" | "diorite" | "granite" | "deepslate" | "cobbled_deepslate" | "tuff" | "netherrack" | "sandstone" | "smooth_stone") || id.ends_with("_planks")
}

fn is_air(state: &str) -> bool {
    matches!(state.split('[').next().unwrap_or("").trim_start_matches("minecraft:"), "air" | "cave_air" | "void_air")
}

fn fluid(state: &str) -> bool {
    let id = state.split('[').next().unwrap_or("").trim_start_matches("minecraft:");
    id == "water" || id == "lava" || state.contains("waterlogged=true")
}

fn full_opaque(state: &str) -> bool {
    removable(state) && !state.contains("_slab") && !state.contains("glass")
}

/// The design's site at the place's fixed offset (leaked: `RunOpts.site` is 'static; a site is
/// small, its world is shared).
fn site_at(pl: &Place, spec: &FarmSpec) -> Result<Option<&'static Site>, String> {
    let (Some(sample), Some(world)) = (pl.sample, &pl.world) else { return Ok(None) };
    let priced = {
        let w = world.read().unwrap_or_else(|e| e.into_inner());
        let fp = site::footprint(spec, pl.ground)?;
        let spot = ([pl.offset[0] + fp.lo.x, pl.offset[2] + fp.lo.z], pl.offset[1] + pl.ground);
        let price = site::price_spots(&w, spec, fp, pl.ground, pl.clear_box, &[spot]).pop().ok_or("no price")?;
        Priced::at(&w, spec, price, pl.clear_box)
    };
    Ok(Some(Box::leak(Box::new(Site::at_priced(world.clone(), sample, vec![priced], 0, pl.afk, pl.clear_box)))))
}

/// The farm's facts at its place (an error: a check refuses it). `exit`: with the golem exit flood
/// (`threads` for it); `hostile`: with the hostile flood's breaking channels.
fn facts(pl: &Place, d: &Design, exit: Option<usize>, hostile: bool) -> Result<Facts, String> {
    let c = circuit::Circuit::from_design(d.clone()).map_err(|e| format!("{e:#}"))?;
    let spec = circuit::mobfarm::spec_of(&c)?;
    let site = site_at(pl, &spec)?;
    let moved = match site {
        Some(s) => s.spec(&spec),
        None => spec.clone(),
    };
    let refused = redsim::entity::mob::refuse::static_checks_in(&moved, site);
    if !refused.is_empty() {
        return Err(format!("refused: {}", refused.join("; ")));
    }
    let mut f = Facts::default();
    if let Some(threads) = exit {
        let ex = redsim::entity::mob::exit::golem_exit_in(&moved, site, 1, threads)?;
        if !ex.failures.is_empty() {
            return Err(format!("golem exit: {}", ex.failures.join("; ")));
        }
        f.stalls = ex.walk_stalls.len();
        f.worst_life = ex.worst_life;
    }
    if hostile && let Some(s) = site {
        f.breaks = circuit::mobfarm::exposures_of(s, &moved).1.into_iter().collect();
    }
    let mut o = RunOpts::steady(1, 0, WalkMode::Random);
    o.site = site;
    let run = rfarm::Run::new(&moved, &o)?;
    let own = run.own.clone();
    let w = run.world;
    f.climb = climb(&w, moved.output);
    let (sight, spawn) = calm_and_spawns(w, &own, &moved);
    f.sight = sight;
    f.spawn = spawn;
    f.seconds = circuit::mobfarm::site_bill(&c, site, pl.stage)?.seconds;
    let top = pl.offset[1] + pl.ground;
    match site {
        Some(s) => {
            let e = s.effort(0, 0);
            f.surface = moved.blocks.iter().filter(|(p, b)| !b.is_air() && p.y >= top).count() + s.fills.iter().filter(|p| p.y >= top).count() + s.seals.iter().filter(|(p, _)| p.y >= top).count();
            (f.placed, f.dug, f.caps, f.seals) = (e.placed, e.dug, s.fills.len(), s.seals.len());
        }
        None => f.placed = moved.blocks.iter().filter(|(_, b)| !b.is_air()).count() as u32,
    }
    Ok(f)
}

/// How high the player can climb from beside the chest (a 0.6 x 1.8 box walking, jumping 1.25,
/// dropping up to 3; blocks without a collision box pass, a fence gate is a door it opens): the
/// highest floor reached.
fn climb(w: &World, chest: BlockPos) -> f64 {
    use redsim::entity::collide::block_collisions;
    use redsim::entity::mob::refuse::floor_height;
    let gate = |p: BlockPos| format!("{}", w.get(p)).contains("fence_gate");
    let fits = |c: BlockPos, h: f64| {
        let bb = Aabb::new(c.x as f64 + 0.2, h + 1.0e-3, c.z as f64 + 0.2, c.x as f64 + 0.8, h + 1.8, c.z as f64 + 0.8);
        block_collisions(w, bb).iter().all(|(p, bs)| gate(*p) || bs.iter().all(|b| !b.intersects(bb)))
    };
    let mut cells: Vec<BlockPos> = Vec::new();
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    for d in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
        let c = BlockPos::new(chest.x + d[0], chest.y, chest.z + d[1]);
        if fits(c, floor_height(w, c)) && seen.insert([c.x, c.y, c.z]) {
            cells.push(c);
        }
    }
    let mut best = f64::NEG_INFINITY;
    let mut k = 0;
    while k < cells.len() && cells.len() < 20000 {
        let c = cells[k];
        k += 1;
        let h = floor_height(w, c);
        best = best.max(h);
        for d in [[1, 0], [-1, 0], [0, 1], [0, -1], [0, 0]] {
            for dy in [1, 0, -1, -2, -3] {
                if d == [0, 0] && dy != 1 {
                    continue;
                }
                let m = BlockPos::new(c.x + d[0], c.y + dy, c.z + d[1]);
                let hm = floor_height(w, m);
                if hm - h > 1.25 || !seen.insert([m.x, m.y, m.z]) {
                    continue;
                }
                let top = h.max(hm);
                if fits(m, top) && fits(c, top) && fits(m, hm) {
                    cells.push(m);
                } else {
                    seen.remove(&[m.x, m.y, m.z]);
                }
            }
        }
    }
    if std::env::var("MCTAS_TRIM_DEBUG").is_ok() {
        eprintln!("trim climb: from the chest {chest}: {} cells reached, starts {:?}, best floor {best:.2}", cells.len(), cells.iter().take(4).collect::<Vec<_>>());
        let mut top: Vec<&BlockPos> = cells.iter().collect();
        top.sort_by_key(|c| -c.y);
        for c in top.iter().take(8) {
            eprintln!("trim climb:   reached {c} (floor {:.2})", floor_height(w, **c));
        }
    }
    best
}

/// The calm's sight lines (each farm villager's view of its nearest zombie, standing and asleep
/// on its bed, the hide's trapdoors shut then open) and each villager's golem spawn cells (its
/// search from its cell and the cell above: every farm villager can summon).
fn calm_and_spawns(mut w: World, own: &[u32], moved: &FarmSpec) -> (Vec<bool>, Vec<usize>) {
    use redsim::entity::mob::{los, spawn, villager};
    w.entities.list.retain(|e| own.contains(&e.id));
    let of = |w: &World, ty: MobType| -> Vec<usize> { w.entities.list.iter().enumerate().filter(|(_, e)| matches!(&e.kind, EntityKind::Mob(m) if m.ty == ty)).map(|(i, _)| i).collect() };
    let (zombies, villagers) = (of(&w, MobType::Zombie), of(&w, MobType::Villager));
    let trapdoors: Vec<BlockPos> = moved.blocks.iter().filter(|(_, b)| matches!(b, Block::Trapdoor { .. })).map(|(p, _)| *p).collect();
    let mut sight = Vec::new();
    for open in [false, true] {
        let mut v = w.clone();
        for p in &trapdoors {
            if let Block::Trapdoor { iron, facing, top, powered, .. } = v.get(*p) {
                v.set_block(*p, Block::Trapdoor { iron, facing, top, open, powered }, redsim::world::flags::RAW);
            }
        }
        for &i in &villagers {
            let e = &v.entities.list[i];
            let Some(&z) = zombies.iter().min_by(|a, b| v.entities.list[**a].pos.distance_to_sqr(e.pos).total_cmp(&v.entities.list[**b].pos.distance_to_sqr(e.pos))) else { continue };
            let ze = villager::eye_pos(&v, z);
            let mut eyes = vec![villager::eye_pos(&v, i)];
            if let EntityKind::Mob(m) = &e.kind
                && let Some(home) = m.brain.as_deref().and_then(|b| b.home)
            {
                eyes.push(Vec3::new(home.x as f64 + 0.5, home.y as f64 + 0.5625 + 0.2, home.z as f64 + 0.5));
            }
            for ve in eyes {
                sight.push(los::first_hit(&v, ve, ze).is_none());
            }
        }
    }
    let mut cells = Vec::new();
    for &i in &villagers {
        let c = w.entities.list[i].block_position();
        for start in [c, BlockPos::new(c.x, c.y + 1, c.z)] {
            cells.push(spawn::golem_spawn_cells_from(&w, start).iter().filter(|(_, r)| r.is_ok()).count());
        }
    }
    (sight, cells)
}

fn neighbours(p: Pos) -> [Pos; 6] {
    [[p[0] + 1, p[1], p[2]], [p[0] - 1, p[1], p[2]], [p[0], p[1] + 1, p[2]], [p[0], p[1] - 1, p[2]], [p[0], p[1], p[2] + 1], [p[0], p[1], p[2] - 1]]
}

/// What the edits must leave alone, in design coordinates: the golems' space (the open cells
/// connected to the golem spawn cells, and their neighbours), the cells water can reach (7
/// sideways, any distance down) and every cell beside a fluid, the floors golems spawn on, the
/// entities' cells and the chest's.
struct Keep {
    golem: BTreeSet<Pos>,
    wet: BTreeSet<Pos>,
    floors: BTreeSet<Pos>,
    cells: BTreeSet<Pos>,
}

fn keep_of(pl: &Place, d: &Design) -> Result<Keep, String> {
    let c = circuit::Circuit::from_design(d.clone()).map_err(|e| format!("{e:#}"))?;
    let spec = circuit::mobfarm::spec_of(&c)?;
    let site = site_at(pl, &spec)?;
    let moved = match site {
        Some(s) => s.spec(&spec),
        None => spec.clone(),
    };
    let mut o = RunOpts::steady(1, 0, WalkMode::Random);
    o.site = site;
    let run = rfarm::Run::new(&moved, &o)?;
    let own = run.own.clone();
    let w = run.world;
    let off = match site {
        Some(s) => s.offset,
        None => [0, 0, 0],
    };
    let to_d = |p: BlockPos| [p.x - off[0], p.y - off[1], p.z - off[2]];
    let spots: Vec<Pos> = redsim::entity::mob::exit::spawn_spots_of(&w, &own).into_iter().map(to_d).collect();
    let floors: BTreeSet<Pos> = spots.iter().map(|p| [p[0], p[1] - 1, p[2]]).collect();
    // open in the built world: no collision box, or a fluid
    let open = |p: &Pos| {
        let b = w.get(BlockPos::new(p[0] + off[0], p[1] + off[1], p[2] + off[2]));
        let s = format!("{b}");
        s.contains("water") || s.contains("lava") || redsim::entity::collide::boxes_at(&w, &b, BlockPos::new(p[0] + off[0], p[1] + off[1], p[2] + off[2])).is_empty()
    };
    let (lo, hi) = (spec.lo, spec.hi);
    let inside = |p: &Pos| p[0] >= lo.x - 2 && p[0] <= hi.x + 2 && p[1] >= lo.y - 2 && p[1] <= hi.y + 2 && p[2] >= lo.z - 2 && p[2] <= hi.z + 2;
    let mut golem: BTreeSet<Pos> = BTreeSet::new();
    let mut todo: Vec<Pos> = spots.clone();
    for p in &spots {
        golem.insert(*p);
    }
    while let Some(p) = todo.pop() {
        for q in neighbours(p) {
            if inside(&q) && open(&q) && golem.insert(q) {
                todo.push(q);
            }
        }
    }
    let ring: Vec<Pos> = golem.iter().flat_map(|p| neighbours(*p)).collect();
    golem.extend(ring);
    // water's reach and the fluids' neighbours
    let dopen = |p: &Pos| d.blocks.get(p).is_none_or(|s| is_air(s));
    let mut wet = BTreeSet::new();
    let mut frontier: Vec<(Pos, i32)> = Vec::new();
    for (p, s) in &d.blocks {
        if fluid(s) {
            wet.extend(neighbours(*p));
            if s.contains("water") {
                frontier.push((*p, 0));
            }
        }
    }
    let mut seen: BTreeSet<Pos> = frontier.iter().map(|(p, _)| *p).collect();
    while let Some((p, k)) = frontier.pop() {
        let below = [p[0], p[1] - 1, p[2]];
        let steps: Vec<(Pos, i32)> = if dopen(&below) { vec![(below, 0)] } else { [[1, 0], [-1, 0], [0, 1], [0, -1]].iter().map(|d| ([p[0] + d[0], p[1], p[2] + d[1]], k + 1)).collect() };
        for (q, kq) in steps {
            if kq > 7 || !dopen(&q) || !inside(&q) || !seen.insert(q) {
                continue;
            }
            wet.insert(q);
            frontier.push((q, kq));
        }
    }
    let mut cells = BTreeSet::new();
    for e in &d.entities {
        let c = e.cell();
        for dy in -1..=2 {
            cells.insert([c[0], c[1] + dy, c[2]]);
        }
    }
    for (name, p) in &d.ports {
        if name.starts_with("output") {
            cells.insert(*p);
            cells.insert([p[0], p[1] + 1, p[2]]);
        }
    }
    // a built-in access (its open cells, steps, torches and hatch, connected to the ground layer):
    // the player's way down to the chest is never trimmed (bed-plate-lava-out-of-sight's first
    // trim refilled a staircase the climb could not walk anyway)
    let passable = |s: &str| is_air(s) || s.contains("_slab") || s.contains("fence_gate") || s.contains("torch") || s.contains("ladder");
    let mut todo: Vec<Pos> = d.blocks.iter().filter(|(p, s)| p[1] >= pl.ground && passable(s)).map(|(p, _)| *p).collect();
    let mut access: BTreeSet<Pos> = todo.iter().copied().collect();
    while let Some(p) = todo.pop() {
        for q in neighbours(p) {
            if q[1] <= p[1] && inside(&q) && d.blocks.get(&q).is_some_and(|s| passable(s)) && access.insert(q) {
                todo.push(q);
            }
        }
    }
    cells.extend(access);
    Ok(Keep { golem, wet, floors, cells })
}

/// The natural block at a design cell is a full rock cube (no sand or gravel): a placed block
/// there can go with nothing changed.
fn natural_rock(pl: &Place, p: &Pos) -> bool {
    let Some(world) = &pl.world else { return false };
    if pl.clear_box {
        return false;
    }
    let w = world.read().unwrap_or_else(|e| e.into_inner());
    let at = BlockPos::new(p[0] + pl.offset[0], p[1] + pl.offset[1], p[2] + pl.offset[2]);
    let b = w.get(at);
    let name = format!("{b}");
    if redsim::block::FALLING.iter().any(|f| name.contains(f)) || name.contains("log") || name.contains("leaves") || name.contains("planks") {
        return false;
    }
    let bs = redsim::entity::collide::boxes_at(&w, &b, at);
    bs.len() == 1 && {
        let x = bs[0];
        (x.min_x - at.x as f64).abs() < 1e-9 && (x.max_x - at.x as f64 - 1.0).abs() < 1e-9 && (x.min_y - at.y as f64).abs() < 1e-9 && (x.max_y - at.y as f64 - 1.0).abs() < 1e-9 && (x.min_z - at.z as f64).abs() < 1e-9 && (x.max_z - at.z as f64 - 1.0).abs() < 1e-9
    }
}

fn natural_open(pl: &Place, p: &Pos) -> bool {
    let Some(world) = &pl.world else { return true };
    let w = world.read().unwrap_or_else(|e| e.into_inner());
    redsim::terrain::place::air_like(&w.get(BlockPos::new(p[0] + pl.offset[0], p[1] + pl.offset[1], p[2] + pl.offset[2])))
}

/// The edits worth trying on `d`: class A (the shape unchanged) and class B (away from the golems'
/// space and the fluids), each outer shell first (the farthest from the design's middle).
fn candidates(pl: &Place, d: &Design, keep: &Keep) -> (Vec<Edit>, Vec<Edit>) {
    let prices = redsim::entity::mob::prices::Prices::get().ok();
    let price = |state: &str| -> Option<f64> { prices.and_then(|pr| redsim::entity::mob::prices::item_of(state).and_then(|it| pr.price(&it, pl.stage).ok())) };
    let cobble = price("minecraft:cobblestone").unwrap_or(1.0);
    let n = d.blocks.len().max(1) as f64;
    let mid = d.blocks.keys().fold([0.0; 3], |a, p| [a[0] + p[0] as f64 / n, a[1] + p[1] as f64 / n, a[2] + p[2] as f64 / n]);
    let (mut a, mut b): (Vec<(f64, Edit)>, Vec<(f64, Edit)>) = (Vec::new(), Vec::new());
    let debug = std::env::var("MCTAS_TRIM_DEBUG").is_ok();
    for (p, s) in &d.blocks {
        if keep.cells.contains(p) {
            continue;
        }
        if debug && removable(s) {
            let nat = pl.world.as_ref().map(|w| format!("{}", w.read().unwrap_or_else(|e| e.into_inner()).get(BlockPos::new(p[0] + pl.offset[0], p[1] + pl.offset[1], p[2] + pl.offset[2])))).unwrap_or_default();
            eprintln!("trim: {p:?} {s}: natural {nat}, rock {}, golem {}, wet {}, floor {}", natural_rock(pl, p), keep.golem.contains(p), keep.wet.contains(p), keep.floors.contains(p));
        }
        let far = (p[0] as f64 - mid[0]).powi(2) + (p[1] as f64 - mid[1]).powi(2) + (p[2] as f64 - mid[2]).powi(2);
        if is_air(s) {
            // a dug cell refilled: natural rock there, away from the golems and the water
            if pl.world.is_some() && !pl.clear_box && !keep.golem.contains(p) && !keep.wet.contains(p) && natural_rock(pl, p) {
                b.push((far, Edit::Unlist(*p)));
            }
        } else if removable(s) {
            let cheaper = full_opaque(s) && !s.contains("cobblestone") && price(s).is_some_and(|x| x > cobble + 1e-9);
            if keep.floors.contains(p) {
                // a golem spawn floor: only a swap for an equal full block
                if cheaper {
                    a.push((far, Edit::Swap(*p, "minecraft:cobblestone".into())));
                }
                continue;
            }
            if full_opaque(s) && natural_rock(pl, p) {
                a.push((far, Edit::Unlist(*p)));
            } else if !keep.golem.contains(p) && !keep.wet.contains(p) && (natural_open(pl, p) || pl.world.is_none() || pl.clear_box || natural_rock(pl, p)) {
                b.push((far, Edit::Unlist(*p)));
            }
            if cheaper {
                a.push((far - 0.5, Edit::Swap(*p, "minecraft:cobblestone".into())));
            } else if s.contains("glass") && !keep.golem.contains(p) && price(s).is_some_and(|x| x > cobble + 1e-9) {
                b.push((far - 0.5, Edit::Swap(*p, "minecraft:cobblestone".into())));
            }
        }
    }
    a.sort_by(|x, y| y.0.total_cmp(&x.0));
    b.sort_by(|x, y| y.0.total_cmp(&x.0));
    (a.into_iter().map(|(_, e)| e).collect(), b.into_iter().map(|(_, e)| e).collect())
}

/// Each edit's class-B check on `d` (all in parallel): the ones that pass, in order.
fn screen(pl: &Place, d: &Design, edits: &[Edit], base: &Facts, now: &Facts, threads: usize) -> Vec<Edit> {
    let next = AtomicUsize::new(0);
    let ok: Mutex<Vec<(usize, Edit)>> = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..threads.max(1).min(edits.len().max(1)) {
            sc.spawn(|| {
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    if k >= edits.len() {
                        break;
                    }
                    let e = &edits[k];
                    let opens = matches!(e, Edit::Unlist(p) if !d.blocks.get(p).is_some_and(|s| is_air(s)));
                    if facts(pl, &apply(d, std::slice::from_ref(e)), None, opens).is_ok_and(|f| f.keeps(base, now).is_ok()) {
                        ok.lock().unwrap_or_else(|e| e.into_inner()).push((k, e.clone()));
                    }
                }
            });
        }
    });
    let mut v = ok.into_inner().unwrap_or_default();
    v.sort_by_key(|(k, _)| *k);
    v.into_iter().map(|(_, e)| e).collect()
}

/// The screened edits applied in order, in groups (a group that fails is split): each kept only if
/// the design with every edit kept so far still passes the class-B checks.
fn accept(pl: &Place, d: &Design, edits: Vec<Edit>, base: &Facts) -> (Design, Vec<Edit>) {
    let mut cur = d.clone();
    let Ok(mut now) = facts(pl, &cur, None, true) else { return (cur, Vec::new()) };
    let mut kept = Vec::new();
    let mut todo: Vec<Vec<Edit>> = edits.chunks(16).rev().map(|c| c.to_vec()).collect();
    while let Some(group) = todo.pop() {
        let tried = apply(&cur, &group);
        match facts(pl, &tried, None, true) {
            Ok(f) if f.keeps(base, &now).is_ok() => {
                cur = tried;
                now = f;
                kept.extend(group);
            }
            _ if group.len() > 1 => {
                let (x, y) = group.split_at(group.len() / 2);
                todo.push(y.to_vec());
                todo.push(x.to_vec());
            }
            _ => {}
        }
    }
    (cur, kept)
}

/// The short AI-on runs' mean golems per hour (the same seeds for every design), or the first
/// failure.
fn short_rate(pl: &Place, d: &Design, days: f64, runs: u32) -> Result<f64, String> {
    let c = circuit::Circuit::from_design(d.clone()).map_err(|e| format!("{e:#}"))?;
    let spec = circuit::mobfarm::spec_of(&c)?;
    let site = site_at(pl, &spec)?;
    let moved = match site {
        Some(s) => s.spec(&spec),
        None => spec,
    };
    let ticks = (days * 24000.0) as u32;
    let jobs: Vec<RunOpts> = (0..runs).map(|k| rfarm::judge_opts_in(rfarm::judge_seed(0, k), ticks, WalkMode::Random, site)).collect();
    let mut rates = Vec::new();
    for r in circuit::mobfarm::run_all(&moved, &jobs) {
        let r = r?;
        if let Some(f) = r.failures.first() {
            return Err(f.clone());
        }
        rates.push(r.golems_per_hour());
    }
    Ok(rates.iter().sum::<f64>() / rates.len().max(1) as f64)
}

/// `mctas mobfarm trim`.
pub fn trim_cmd(a: &TrimArgs) -> Result<()> {
    let t0 = std::time::Instant::now();
    let text = std::fs::read_to_string(&a.design)?;
    let d0 = Design::parse(&text)?;
    let c0 = circuit::Circuit::from_design(d0.clone())?;
    let spec0 = circuit::mobfarm::spec_of(&c0).map_err(|e| anyhow!(e))?;
    let stage = circuit::mobfarm::price_stage(&c0);
    let (ground, afk, clear_box) = site::params(&spec0, d0.meta.get("ground").map(String::as_str), d0.meta.get("afk").map(String::as_str), d0.meta.get("terrain").map(String::as_str)).map_err(|e| anyhow!(e))?;
    let sample = circuit::mobfarm::terrain_sample(&c0).map_err(|e| anyhow!(e))?;
    let (world, offset) = match sample {
        Some(s) => {
            let site0 = circuit::mobfarm::site_of(&c0, &spec0, s.name).map_err(|e| anyhow!(e))?;
            (Some(site0.world.clone()), site0.offset)
        }
        None => (None, [0, 0, 0]),
    };
    let pl = Place { sample, world, offset, ground, afk, clear_box, stage };
    let threads = if a.threads > 0 { a.threads } else { std::thread::available_parallelism().map_or(8, |n| n.get()).saturating_sub(16).max(4) };
    let base = facts(&pl, &d0, Some(threads), true).map_err(|e| anyhow!("the design itself: {e}"))?;
    println!("{}: {} at {} ({:.0} s)", a.design.display(), base.line(), sample.map_or("the void".to_string(), |s| format!("{} {:?}", s.name, offset)), t0.elapsed().as_secs_f64());
    let keep = keep_of(&pl, &d0).map_err(|e| anyhow!(e))?;
    let mut cur = d0.clone();
    let mut kept_all: Vec<Edit> = Vec::new();
    for round in 1..=4 {
        let (class_a, class_b) = candidates(&pl, &cur, &keep);
        // class A: the shape unchanged, so the price alone decides (every one cheaper alone is
        // cheaper together: applied at once, then checked whole)
        let now = facts(&pl, &cur, None, false).map_err(|e| anyhow!(e))?;
        let mut a_kept = Vec::new();
        if !class_a.is_empty() {
            let tried = apply(&cur, &class_a);
            match facts(&pl, &tried, None, false) {
                Ok(f) if f.keeps(&base, &now).is_ok() => {
                    cur = tried;
                    a_kept = class_a.clone();
                }
                _ => {
                    let (d, k) = accept(&pl, &cur, class_a.clone(), &base);
                    cur = d;
                    a_kept = k;
                }
            }
        }
        let now = facts(&pl, &cur, None, true).map_err(|e| anyhow!(e))?;
        let ok = screen(&pl, &cur, &class_b, &base, &now, threads);
        let (d, b_kept) = accept(&pl, &cur, ok.clone(), &base);
        cur = d;
        println!("round {round}: class A {} tried, {} kept; class B {} tried, {} pass alone, {} kept; {} ({:.0} s)", class_a.len(), a_kept.len(), class_b.len(), ok.len(), b_kept.len(), facts(&pl, &cur, None, false).map(|f| f.line()).unwrap_or_default(), t0.elapsed().as_secs_f64());
        let progress = a_kept.len() + b_kept.len();
        kept_all.extend(a_kept);
        kept_all.extend(b_kept);
        if progress == 0 {
            break;
        }
    }
    // the whole check (the golem exit flood too) and the short AI-on runs; a miss bisects the edits
    let r0 = short_rate(&pl, &d0, a.days, a.runs).map_err(|e| anyhow!("the design's own short runs: {e}"))?;
    let good = |d: &Design| facts(&pl, d, Some(threads), true).is_ok_and(|f| f.keeps(&base, &Facts { seconds: f64::INFINITY, ..base.clone() }).is_ok()) && short_rate(&pl, d, a.days, a.runs).is_ok_and(|r| r >= (1.0 - RATE_TOLERANCE) * r0);
    if !kept_all.is_empty() && !good(&cur) {
        let why = match facts(&pl, &cur, Some(threads), true) {
            Err(e) => e,
            Ok(f) => match f.keeps(&base, &Facts { seconds: f64::INFINITY, ..base.clone() }) {
                Err(e) => format!("{e} (golem stalls {} -> {}, longest life {} -> {} gt)", base.stalls, f.stalls, base.worst_life, f.worst_life),
                Ok(()) => format!("the short runs: {} golems/h against {r0:.1}", short_rate(&pl, &cur, a.days, a.runs).map_or_else(|e| e, |r| format!("{r:.1}"))),
            },
        };
        println!("the trimmed design misses the whole check or the short runs ({why}): bisecting {} edits", kept_all.len());
        let mut kept: Vec<Edit> = Vec::new();
        let mut todo: Vec<Vec<Edit>> = vec![kept_all.clone()];
        while let Some(group) = todo.pop() {
            let mut with = kept.clone();
            with.extend(group.iter().cloned());
            if good(&apply(&d0, &with)) {
                kept = with;
            } else if group.len() > 1 {
                let (x, y) = group.split_at(group.len() / 2);
                todo.push(y.to_vec());
                todo.push(x.to_vec());
            }
        }
        cur = apply(&d0, &kept);
        kept_all = kept;
    }
    let after = facts(&pl, &cur, Some(threads), true).map_err(|e| anyhow!("the trimmed design: {e}"))?;
    let r1 = short_rate(&pl, &cur, a.days, a.runs).map_err(|e| anyhow!("the trimmed design's short runs: {e}"))?;
    write_trimmed(&text, &cur, &a.out)?;
    let removed = kept_all.iter().filter(|e| matches!(e, Edit::Unlist(p) if !d0.blocks.get(p).is_some_and(|s| is_air(s)))).count();
    let refilled = kept_all.iter().filter(|e| matches!(e, Edit::Unlist(p) if d0.blocks.get(p).is_some_and(|s| is_air(s)))).count();
    let swapped = kept_all.iter().filter(|e| matches!(e, Edit::Swap(..))).count();
    for e in &kept_all {
        match e {
            Edit::Unlist(p) => println!("  {} {:?}: {}", if d0.blocks.get(p).is_some_and(|s| is_air(s)) { "refilled" } else { "removed" }, p, d0.blocks.get(p).map(String::as_str).unwrap_or("")),
            Edit::Swap(p, s) => println!("  swapped {:?}: {} -> {s}", p, d0.blocks.get(p).map(String::as_str).unwrap_or("")),
        }
    }
    println!(
        "{}: before {} -> after {}; {removed} blocks removed, {refilled} dug cells refilled, {swapped} swapped; short AI-on runs {r0:.1} -> {r1:.1} golems/h ({} x {} days); {:.0} s",
        a.out.display(),
        base.line(),
        after.line(),
        a.runs,
        a.days,
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}

/// `mctas mobfarm climb <design>`: the player's way up from the chest at the design's spot (the
/// trim's climb flood): the highest floor reached against the ground layer (farmopt: check a
/// built-in access before an upload).
pub fn climb_cmd(design: &Path) -> Result<()> {
    let d = Design::load(design)?;
    let c = circuit::Circuit::from_design(d.clone())?;
    let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow!(e))?;
    let (ground, _, _) = site::params(&spec, d.meta.get("ground").map(String::as_str), d.meta.get("afk").map(String::as_str), d.meta.get("terrain").map(String::as_str)).map_err(|e| anyhow!(e))?;
    let site: Option<&'static Site> = match circuit::mobfarm::terrain_sample(&c).map_err(|e| anyhow!(e))? {
        Some(s) => Some(Box::leak(Box::new(circuit::mobfarm::site_of(&c, &spec, s.name).map_err(|e| anyhow!(e))?))),
        None => None,
    };
    let moved = match site {
        Some(s) => s.spec(&spec),
        None => spec.clone(),
    };
    let mut o = RunOpts::steady(1, 0, WalkMode::Random);
    o.site = site;
    let run = rfarm::Run::new(&moved, &o).map_err(|e| anyhow!(e))?;
    let oy = site.map_or(0, |s| s.offset[1]);
    let floor = climb(&run.world, moved.output);
    // (the last step out, through the opened hatch onto the surface, is the player's: the flood
    // stands it right under the ground layer)
    let reached = floor >= (oy + ground - 1) as f64 - 1e-6;
    println!(
        "{}: from the chest (design y {}) the player climbs to a floor at design y {:.1}; the ground layer is y {ground}: {}",
        design.display(),
        moved.output.y - oy,
        floor - oy as f64,
        if reached { "the access reaches the surface" } else { "NO WAY UP: the access does not reach the surface" }
    );
    Ok(())
}

/// The trimmed design written with the original's caption (its leading comment lines, less the
/// judge's stale `# CIRCUIT:` verdict line).
fn write_trimmed(original: &str, d: &Design, out: &Path) -> Result<()> {
    let mut s = String::new();
    for line in original.lines() {
        if !line.starts_with('#') {
            break;
        }
        if line.starts_with("# CIRCUIT") || line.starts_with("# archived") {
            continue;
        }
        s.push_str(line);
        s.push('\n');
    }
    s.push_str(&d.to_text());
    std::fs::write(out, s)?;
    Ok(())
}
