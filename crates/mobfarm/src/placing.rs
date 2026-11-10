//! The brief's two placement checks on a part's bench (designs/briefs/farm-iron-carousel.md:
//! "settled as placed" and "built in order", every part), run the way ironfarm's queue loop runs
//! them (explore/ironfarm, crates/mobfarm/src/queue.rs: `queue-loop-run --placed-raw`,
//! `--placed-in-order [GAP]`, `--placed-all`, `--build-gap-sweep`). Its build order and place:edge
//! cells ([`build_order_of`], [`edge_cells_of`], explore/ironfarm 88ba42478) are brought over
//! unchanged for the parts on main (the bed siding's and the linear stand's benches);
//! [`build_steps_of`] adds the two steps a player takes that are not one block's placement.
//!
//! - [`Placement::Judge`]: `Run::start` (raw writes, the block entities merged, a shape update of
//!   every wire, a self update of every block), then the settle (200 gt): the benches' way so far.
//! - [`Placement::Raw`], settled as placed: the file as written, a raw paste (blocks raw, its
//!   `nbt:` lines merged, no update, no settle), the Nether part too; the file must hold every
//!   cell in its settled state ([`settle_design`] writes it so).
//! - [`Placement::Built`], built in order: the world starts with none of the design's blocks; each
//!   build step is placed with the game's updates (`flags::ALL`), a container filled as it is
//!   placed (`data merge` of its `nbt:` line), then the step's gap passes: each cell's priced gap
//!   (prices.tsv's `place`, 1 s = 20 gt; 40 gt for a cell the bill charges `place:edge`) or a flat
//!   gap. A Nether part is built first (its world ticking while it is built: the player is there),
//!   then the overworld's (the Nether unticked again until a crossing's ticket, as redsim runs it).

use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;
use redsim::world::World;
use std::collections::{HashMap, HashSet};

/// How a bench puts its design in the world.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Placement {
    /// The judge's placement, then the settle.
    #[default]
    Judge,
    /// Settled as placed: the raw paste.
    Raw,
    /// Built in order, `gap` gt between steps (`None`: each step's priced gap).
    Built { gap: Option<i64> },
}

impl Placement {
    pub fn label(&self) -> String {
        match self {
            Placement::Judge => "the judge's placement".into(),
            Placement::Raw => "settled as placed (raw paste)".into(),
            Placement::Built { gap: None } => "built in order, each cell's priced gap (20 gt, 40 gt at a place:edge cell)".into(),
            Placement::Built { gap: Some(g) } => format!("built in order, {g} gt between placements"),
        }
    }

    /// The benches' flags (ironfarm's): `--placed-raw`; `--placed-in-order [GAP]` (no GAP or 0:
    /// each cell's priced gap).
    pub fn of_flags(raw: bool, in_order: Option<i64>) -> Placement {
        match in_order {
            Some(g) if g > 0 => Placement::Built { gap: Some(g) },
            Some(_) => Placement::Built { gap: None },
            None if raw => Placement::Raw,
            None => Placement::Judge,
        }
    }

    /// The placements a run of the bench covers: `--placed-all` (the judge's, the raw paste, built
    /// in order at the priced gaps or GAP), `--build-gap-sweep` (built in order at every gap from 1
    /// gt to 40 or GAP), else the one the flags name.
    pub fn modes(raw: bool, in_order: Option<i64>, all: bool, sweep: bool) -> Vec<Placement> {
        let gap = in_order.filter(|g| *g > 0);
        if sweep {
            (1..=gap.unwrap_or(40)).map(|g| Placement::Built { gap: Some(g) }).collect()
        } else if all {
            vec![Placement::Judge, Placement::Raw, Placement::Built { gap }]
        } else {
            vec![Placement::of_flags(raw, in_order)]
        }
    }
}

/// The default build order (the brief's "built in order", for a design that lists none): supports
/// first, bottom up; then containers, each filled as it is placed (before power can reach it);
/// then rails, bottom up; then redstone from its sources outward (breadth-first over cells that
/// touch, wire's up and down steps included; a cell no source reaches starts a new search, the
/// lowest first). Each entry: the cell, its block, the container's fill (SNBT). (explore/ironfarm
/// 88ba42478, crates/mobfarm/src/queue.rs, unchanged.)
pub fn build_order_of(blocks: &[(BlockPos, Block)], block_entities: &[(BlockPos, String)]) -> Vec<(BlockPos, Block, Option<String>)> {
    fn class(b: &Block) -> u8 {
        match b {
            Block::Air => 9,
            Block::Rail { .. } | Block::PlainRail { .. } | Block::DetectorRail { .. } => 2,
            Block::Chest { .. } | Block::Hopper { .. } | Block::Dispenser { .. } | Block::Furnace { .. } | Block::ShulkerBox { .. } | Block::BrewingStand { .. } | Block::Crafter { .. } | Block::Jukebox { .. } | Block::Composter { .. } | Block::ChiseledBookshelf { .. } => 1,
            Block::Wire { .. } | Block::RedstoneBlock | Block::Torch { .. } | Block::WallTorch { .. } | Block::Lever { .. } | Block::Button { .. } | Block::Repeater { .. } | Block::Comparator { .. } | Block::Observer { .. } | Block::Lamp { .. } | Block::Bulb { .. } | Block::Note { .. } | Block::Target { .. } | Block::Piston { .. } | Block::PistonHead { .. } | Block::Plate { .. } | Block::Tripwire { .. } | Block::TripwireHook { .. } | Block::DaylightDetector { .. } | Block::Tnt { .. } | Block::Trapdoor { .. } | Block::Door { .. } | Block::FenceGate { .. } | Block::Sculk { .. } => 3,
            _ => 0,
        }
    }
    let source = |b: &Block| matches!(b, Block::RedstoneBlock | Block::Torch { .. } | Block::WallTorch { .. } | Block::Lever { .. } | Block::Button { .. } | Block::DaylightDetector { .. } | Block::Plate { .. } | Block::TripwireHook { .. } | Block::Observer { .. } | Block::Target { .. });
    // (a block that holds up a rail, a wire, a repeater, a comparator, a torch or a plate is a
    // support whatever it is, a redstone block under a booster included: placed first, or what it
    // holds up would pop off)
    let needs_floor = |b: &Block| matches!(b, Block::Rail { .. } | Block::PlainRail { .. } | Block::DetectorRail { .. } | Block::Wire { .. } | Block::Repeater { .. } | Block::Comparator { .. } | Block::Torch { .. } | Block::Plate { .. });
    let holds: HashSet<BlockPos> = blocks.iter().filter(|(_, b)| needs_floor(b)).map(|(p, _)| BlockPos::new(p.x, p.y - 1, p.z)).chain(blocks.iter().filter_map(|(p, b)| match b { Block::WallTorch { facing, .. } => Some(p.rel(facing.opposite())), _ => None })).collect();
    let class_at = |p: &BlockPos, b: &Block| if holds.contains(p) && !needs_floor(b) && class(b) == 3 { 0 } else { class(b) };
    let fills: HashMap<BlockPos, String> = block_entities.iter().cloned().collect();
    let key = |p: &BlockPos| (p.y, p.x, p.z);
    let mut out = Vec::new();
    for c in 0..3u8 {
        let mut v: Vec<(BlockPos, Block)> = blocks.iter().filter(|(p, b)| class_at(p, b) == c).cloned().collect();
        v.sort_by_key(|(p, _)| key(p));
        out.extend(v.into_iter().map(|(p, b)| (p, b, fills.get(&p).cloned())));
    }
    let reds: Vec<(BlockPos, Block)> = blocks.iter().filter(|(p, b)| class_at(p, b) == 3).cloned().collect();
    let idx: HashMap<BlockPos, usize> = reds.iter().enumerate().map(|(i, (p, _))| (*p, i)).collect();
    let mut seen = vec![false; reds.len()];
    let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    let mut starts: Vec<usize> = (0..reds.len()).filter(|&i| source(&reds[i].1)).collect();
    starts.sort_by_key(|&i| key(&reds[i].0));
    for i in starts {
        seen[i] = true;
        queue.push_back(i);
    }
    let mut order = Vec::new();
    loop {
        while let Some(i) = queue.pop_front() {
            order.push(i);
            let p = reds[i].0;
            for dx in -1..=1i32 {
                for dy in -1..=1i32 {
                    for dz in -1..=1i32 {
                        let m = dx.abs() + dy.abs() + dz.abs();
                        // (face neighbours, and the up and down steps of a wire)
                        if m == 0 || m > 2 || (m == 2 && dy == 0) {
                            continue;
                        }
                        if let Some(&j) = idx.get(&BlockPos::new(p.x + dx, p.y + dy, p.z + dz)) {
                            if !seen[j] {
                                seen[j] = true;
                                queue.push_back(j);
                            }
                        }
                    }
                }
            }
        }
        match (0..reds.len()).filter(|&i| !seen[i]).min_by_key(|&i| key(&reds[i].0)) {
            Some(i) => {
                seen[i] = true;
                queue.push_back(i);
            }
            None => break,
        }
    }
    out.extend(order.into_iter().map(|i| (reds[i].0, reds[i].1, fills.get(&reds[i].0).cloned())));
    out
}

/// The cells the build bill prices as `place:edge` (prices.rs's rule, approximated on the design's
/// own blocks): more than 2 above the design's lowest level, with no standable surface (a block a
/// player can stand on, two air above it) within 3 horizontally and 1 or 2 below. A player takes
/// an extra second to place each (sneaking to an edge), so the in-order build gives it a longer gap.
/// (explore/ironfarm 88ba42478, unchanged.)
pub fn edge_cells_of(blocks: &[(BlockPos, Block)]) -> HashSet<BlockPos> {
    let cells: HashSet<BlockPos> = blocks.iter().filter(|(_, b)| !matches!(b, Block::Air)).map(|(p, _)| *p).collect();
    let standable = |b: &Block| !matches!(b, Block::Air | Block::Wire { .. } | Block::Torch { .. } | Block::WallTorch { .. } | Block::Rail { .. } | Block::PlainRail { .. } | Block::DetectorRail { .. } | Block::Button { .. } | Block::Plate { .. } | Block::Tripwire { .. } | Block::TripwireHook { .. } | Block::Water { .. } | Block::Lava { .. } | Block::Fire { .. } | Block::BubbleColumn { .. });
    let up = |p: &BlockPos, d: i32| BlockPos::new(p.x, p.y + d, p.z);
    let surfaces: HashSet<BlockPos> = blocks.iter().filter(|(p, b)| standable(b) && !cells.contains(&up(p, 1)) && !cells.contains(&up(p, 2))).map(|(p, _)| *p).collect();
    let low = blocks.iter().map(|(p, _)| p.y).min().unwrap_or(0);
    cells
        .iter()
        .filter(|c| c.y - low > 2 && !(-3..=3).any(|dx| (-3..=3).any(|dz| (1..=2).any(|dy| surfaces.contains(&BlockPos::new(c.x + dx, c.y - dy, c.z + dz))))))
        .copied()
        .collect()
}

/// One cell of a build step: the cell, its block (none: a fill alone), its container's fill.
pub type Cell = (BlockPos, Option<Block>, Option<String>);

/// A design's listed build steps (its `build:` line; the brief: "a part whose build needs a
/// temporary block or a 'place this last' step says so in the list"): `last x,y,z ...`, the
/// overworld cells placed after every other one, in the listed order (each its own step);
/// `fills-last`, every container's contents a step of its own after every block's placement (in the
/// containers' order: bottom up), not as the container is placed (a sorter's stocked hopper whose
/// lock is not built yet would pass its stock on), and no comparator's stored output merged (the
/// comparators read the stocks as they go in).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildPlan {
    pub last: Vec<BlockPos>,
    pub fills_last: bool,
}

impl BuildPlan {
    /// The plan of a design's meta (none: the default order alone).
    pub fn of(meta: &std::collections::BTreeMap<String, String>) -> anyhow::Result<BuildPlan> {
        let mut plan = BuildPlan::default();
        let Some(line) = meta.get("build") else { return Ok(plan) };
        for w in line.split_whitespace() {
            match w {
                "last" => {}
                "fills-last" => plan.fills_last = true,
                c => {
                    let v: Vec<i32> = c.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>().map_err(|e| anyhow::anyhow!("`build:` word {c:?} (fills-last, last x,y,z ...): {e}"))?;
                    anyhow::ensure!(v.len() == 3, "`build: last x,y,z ...`: {c:?} is not a cell");
                    plan.last.push(BlockPos::new(v[0], v[1], v[2]));
                }
            }
        }
        Ok(plan)
    }

    /// The `build:` line's value.
    pub fn meta(&self) -> String {
        let mut words: Vec<String> = Vec::new();
        if self.fills_last {
            words.push("fills-last".into());
        }
        if !self.last.is_empty() {
            words.push("last".into());
            words.extend(self.last.iter().map(|p| format!("{},{},{}", p.x, p.y, p.z)));
        }
        words.join(" ")
    }
}

/// A nether portal block (a terrain block in redsim).
pub fn is_portal(b: &Block) -> bool {
    matches!(b, Block::Terrain(id) if redsim::terrain::blocks::props(*id).block_id() == "minecraft:nether_portal")
}

/// A container (the default order's second class: its fill is an item stock).
pub fn is_container(b: &Block) -> bool {
    matches!(b, Block::Chest { .. } | Block::Hopper { .. } | Block::Dispenser { .. } | Block::Furnace { .. } | Block::ShulkerBox { .. } | Block::BrewingStand { .. } | Block::Crafter { .. } | Block::Jukebox { .. } | Block::Composter { .. } | Block::ChiseledBookshelf { .. })
}

/// [`build_order_of`] as build steps (each step placed in one tick, its gap after it), with the two
/// steps a player takes that are not one block's placement (farmopt 2026-11-10, the station's
/// build): a nether portal is lit, not placed: each set of joined portal cells is one step (the
/// game's flint and steel fills a lit frame at once), after every other block (a portal block whose
/// frame is not complete when a neighbour along its plane changes is removed by the game's shape
/// update, which redsim does not model: placed in the default order, each row's portal cells
/// would be removed as its east pillar went in); a fluid source is poured from a bucket, last (an
/// earlier pour flows out of a basin whose walls are not up yet); a flowing fluid cell is not
/// placed: the flow makes it. The design's listed steps ([`BuildPlan`]): with `fills_last` the
/// fills after every placement, before the portals; the `last` cells at the very end, in order.
pub fn build_steps_of(blocks: &[(BlockPos, Block)], block_entities: &[(BlockPos, String)], plan: &BuildPlan) -> Vec<Vec<Cell>> {
    let order: Vec<(BlockPos, Block, Option<String>)> = build_order_of(blocks, block_entities).into_iter().filter(|c| !plan.last.contains(&c.0)).collect();
    let fluid = |b: &Block| matches!(b, Block::Water { .. } | Block::Lava { .. });
    // (`fills_last` defers a container's contents, and leaves a comparator's stored output to the
    // game: a player cannot set it, and merged at its placement, before the stock it reads is in,
    // it would match the stock's output already when the stock goes in, so the comparator would
    // send its dust no update; without the merge it reads the stock and updates its line)
    let deferred = |c: &(BlockPos, Block, Option<String>)| plan.fills_last && c.2.is_some() && is_container(&c.1);
    let merged = |c: &(BlockPos, Block, Option<String>)| if plan.fills_last { None } else { c.2.clone() };
    let mut steps: Vec<Vec<Cell>> = order.iter().filter(|c| !is_portal(&c.1) && !fluid(&c.1)).map(|c| vec![(c.0, Some(c.1), merged(c))]).collect();
    steps.extend(order.iter().filter(|c| !is_portal(&c.1) && !fluid(&c.1) && deferred(c)).filter_map(|c| c.2.clone().map(|f| vec![(c.0, None, Some(f))])));
    let portals: Vec<&(BlockPos, Block, Option<String>)> = order.iter().filter(|c| is_portal(&c.1)).collect();
    let at: HashMap<BlockPos, usize> = portals.iter().enumerate().map(|(i, c)| (c.0, i)).collect();
    let key = |i: &usize| (portals[*i].0.y, portals[*i].0.x, portals[*i].0.z);
    let mut first: Vec<usize> = (0..portals.len()).collect();
    first.sort_by_key(key);
    let mut seen = vec![false; portals.len()];
    for s in first {
        if seen[s] {
            continue;
        }
        seen[s] = true;
        let mut group = vec![s];
        let mut k = 0;
        while k < group.len() {
            let p = portals[group[k]].0;
            for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                if let Some(&j) = at.get(&BlockPos::new(p.x + dx, p.y + dy, p.z + dz))
                    && !seen[j]
                {
                    seen[j] = true;
                    group.push(j);
                }
            }
            k += 1;
        }
        group.sort_by_key(key);
        steps.push(group.into_iter().map(|i| (portals[i].0, Some(portals[i].1), portals[i].2.clone())).collect());
    }
    steps.extend(order.iter().filter(|c| matches!(c.1, Block::Water { level: 0 } | Block::Lava { level: 0 })).map(|c| vec![(c.0, Some(c.1), c.2.clone())]));
    let fills: HashMap<BlockPos, String> = block_entities.iter().cloned().collect();
    for p in &plan.last {
        if let Some((_, b)) = blocks.iter().find(|(q, _)| q == p) {
            steps.push(vec![(*p, Some(*b), fills.get(p).cloned())]);
        }
    }
    steps
}

/// What a build in order did.
#[derive(Clone, Debug, Default)]
pub struct BuildLog {
    /// The world's time at the build's start (`Run::start`'s).
    pub start: i64,
    /// The build's drive lines, their times from the build's start: `act t set x y z <state>` (a
    /// placement with every update), `act t nbt x y z <snbt>` (a container's fill), and for a
    /// Nether part `act t nether-set x y z <state>` / `act t nether-nbt x y z <snbt>` (its cells in
    /// the Nether's coordinates).
    pub lines: Vec<String>,
    pub steps: usize,
    pub cells: usize,
    /// The ticks the Nether part's build took (its world ticked through them), 0 without one.
    pub nether_ticks: i64,
}

/// The ticks a survival player stands in a nether portal to cross it (prices.tsv's
/// `portal_crossing`: 4 s = 80 gt): after building a Nether part, the way back to the overworld.
pub const PORTAL_WAIT: i64 = 80;

/// The Nether world's tick while the player is there (its chunks loaded without a portal ticket:
/// its build), after the overworld's as the game ticks its levels (`portal::tick_nether` ticks it
/// only under a crossing's ticket).
fn tick_nether_loaded(w: &mut World) {
    let now = w.time;
    let Some(mut n) = w.nether.take() else { return };
    n.time = now - 1;
    n.entities.next_id = w.entities.next_id;
    n.tick();
    w.entities.next_id = n.entities.next_id;
    n.time = now;
    let errs = std::mem::take(&mut n.errors);
    w.nether = Some(n);
    for e in errs {
        w.errors.push(format!("the Nether: {e}"));
    }
}

/// Places one build step in `w`, its drive lines at `t` (`set`/`nbt`, or for the Nether part
/// `nether-set`/`nether-nbt`).
fn place_step(w: &mut World, step: &[Cell], t: i64, nether: bool, lines: &mut Vec<String>) -> Result<(), String> {
    let (set, nbt) = if nether { ("nether-set", "nether-nbt") } else { ("set", "nbt") };
    for (p, b, fill) in step {
        if let Some(b) = b {
            w.set_block(*p, *b, redsim::world::flags::ALL);
            lines.push(format!("act {t} {set} {} {} {} {b}", p.x, p.y, p.z));
        }
        if let Some(f) = fill {
            redsim::redstone::container::apply_block_entities(w, [(*p, f.as_str())])?;
            lines.push(format!("act {t} {nbt} {} {} {} {f}", p.x, p.y, p.z));
        }
    }
    Ok(())
}

/// (a diagnostic) `MCTAS_BUILD_WATCH="x,y,z n1,59,1 ..."`: the watched cells (`n`: the Nether's)
/// printed on stderr whenever one changes during a build: state, comparator output, contents.
fn watched() -> Vec<(bool, BlockPos)> {
    std::env::var("MCTAS_BUILD_WATCH")
        .unwrap_or_default()
        .split_whitespace()
        .filter_map(|w| {
            let (n, c) = match w.strip_prefix('n') { Some(c) => (true, c), None => (false, w) };
            let v: Vec<i32> = c.split(',').filter_map(|x| x.parse().ok()).collect();
            (v.len() == 3).then(|| (n, BlockPos::new(v[0], v[1], v[2])))
        })
        .collect()
}

fn watch_print(w: &World, cells: &[(bool, BlockPos)], last: &mut Vec<String>, t: i64) {
    if cells.is_empty() {
        return;
    }
    let now: Vec<String> = cells
        .iter()
        .map(|(n, p)| {
            let ww = if *n { w.nether.as_deref().unwrap_or(w) } else { w };
            let items: u32 = ww.containers.map.get(p).map(|c| c.slots.iter().flatten().map(|s| s.count as u32).sum()).unwrap_or(0);
            format!("{}{} {} {}={} out {} items {items}", if *n { "n" } else { "" }, p.x, p.y, p.z, ww.get(*p).to_string().trim_start_matches("minecraft:"), ww.comparator_out.get(p).copied().unwrap_or(0))
        })
        .collect();
    if now != *last {
        eprintln!("build t {t}: {}", now.join(" | "));
        *last = now;
    }
}

/// Builds `spec` in order into `run` (started with none of its blocks, its Nether world empty).
fn build(run: &mut Run, spec: &FarmSpec, gap: Option<i64>, plan: &BuildPlan) -> Result<BuildLog, String> {
    let t_b = run.world.time;
    let mut log = BuildLog { start: t_b, ..Default::default() };
    let gap_of = |step: &[Cell], edges: &HashSet<BlockPos>| gap.unwrap_or(if step.iter().any(|c| edges.contains(&c.0)) { 40 } else { 20 }).max(1);
    let wcells = watched();
    let mut wlast: Vec<String> = Vec::new();
    if let Some(ns) = &spec.nether {
        let placed = ns.placed();
        let edges = edge_cells_of(&placed);
        // (the listed `last` cells are the overworld's; `fills_last` holds for both parts)
        for step in build_steps_of(&placed, &ns.block_entities_at(), &BuildPlan { last: Vec::new(), ..plan.clone() }) {
            let t = run.world.time - t_b;
            let n = run.world.nether.as_deref_mut().ok_or("the run has no Nether world to build in")?;
            place_step(n, &step, t, true, &mut log.lines)?;
            log.cells += step.len();
            log.steps += 1;
            watch_print(&run.world, &wcells, &mut wlast, t);
            for _ in 0..gap_of(&step, &edges) {
                run.tick();
                tick_nether_loaded(&mut run.world);
                watch_print(&run.world, &wcells, &mut wlast, run.world.time - t_b);
            }
        }
        // (the player's way back: standing in the Nether portal for a survival player's wait,
        // prices.tsv's portal_crossing, the Nether still loaded: a source poured last has flowed
        // by the time it unloads, whatever the gap)
        for _ in 0..PORTAL_WAIT {
            run.tick();
            tick_nether_loaded(&mut run.world);
        }
        log.nether_ticks = run.world.time - t_b;
    }
    let edges = edge_cells_of(&spec.blocks);
    for step in build_steps_of(&spec.blocks, &spec.block_entities, plan) {
        let t = run.world.time - t_b;
        place_step(&mut run.world, &step, t, false, &mut log.lines)?;
        log.cells += step.len();
        log.steps += 1;
        watch_print(&run.world, &wcells, &mut wlast, t);
        for _ in 0..gap_of(&step, &edges) {
            run.tick();
            watch_print(&run.world, &wcells, &mut wlast, run.world.time - t_b);
        }
    }
    Ok(log)
}

/// A bench's run started the given way, ready for its entities: the judge's (`Run::start`, the
/// settle), the raw paste (`Run::start` raw, no settle), or built in order (`Run::start` with none
/// of the design's blocks, then the build, the design's listed steps in `plan`). Returns the run
/// and the build's log (built only).
pub fn start(spec: &FarmSpec, ro: &RunOpts, how: Placement, plan: &BuildPlan) -> Result<(Run, Option<BuildLog>), String> {
    match how {
        Placement::Judge => {
            let mut run = Run::start(spec, ro)?;
            while run.settle_tick() {}
            Ok((run, None))
        }
        Placement::Raw => Ok((Run::start(spec, &RunOpts { raw: true, ..*ro })?, None)),
        Placement::Built { gap } => {
            let empty = FarmSpec { blocks: Vec::new(), block_entities: Vec::new(), nether: None, ..spec.clone() };
            let mut run = Run::start(&empty, ro)?;
            if let Some(ns) = &spec.nether {
                let mut n = ns.world(ro)?;
                n.time = run.world.time;
                run.world.nether = Some(Box::new(n));
            }
            let log = build(&mut run, spec, gap, plan)?;
            Ok((run, Some(log)))
        }
    }
}

/// The drive header lines (`# ` prefixed by the drive's writer) that say how a run was placed:
/// none for the judge's; for a build, ironfarm's `build:` line and the spawn tick (`spawn S`, from
/// the build's start).
pub fn header(how: Placement, log: Option<&BuildLog>, t_spawn: i64, plan: &BuildPlan) -> Vec<String> {
    match (how, log) {
        (Placement::Raw, _) => vec!["placed: settled as placed: Run::start(spec, RunOpts { raw: true, .. }): the design's blocks written raw, its nbt: lines merged (the game's data merge), no shape or neighbour update and no settle (the Nether part too); the entities spawned at once (t0 = the world's time then)".into()],
        (Placement::Built { gap }, Some(l)) => {
            let gaps = match gap {
                None => "each cell's priced gap between placements (20 gt; 40 gt at a cell the bill charges place:edge)".to_string(),
                Some(g) => format!("{g} gt between placements"),
            };
            let nether = if l.nether_ticks > 0 {
                format!(" The Nether part is built first, as `act t nether-set x y z <state>` / `act t nether-nbt x y z <snbt>` (the Nether's coordinates), its world ticked every tick of its build and of the player's {PORTAL_WAIT} gt in the portal back (t 0..{}: the player is there); then the overworld's, the Nether not ticked again until a golem's crossing (its PORTAL ticket).", l.nether_ticks)
            } else {
                String::new()
            };
            let listed = if *plan == BuildPlan::default() { String::new() } else { format!(", then the design's listed steps (`build: {}`: fills-last, each container's contents a step of its own after every placement and no comparator output merged; last, these cells at the very end)", plan.meta()) };
            vec![
                format!("build: built in order instead: Run::start with the design's blocks and block entities removed (no settle), then each `act t set x y z <state>` a placement with the game's updates (flags 3) and each `act t nbt x y z <snbt>` a container's fill, in the build order (mobfarm::placing::build_steps_of: ironfarm's build_order_of, each lit portal's cells at one tick after every other block, fluid sources poured last{listed}), {gaps}; every time here counts from the build's start (time {}), not from t0.{nether}", l.start),
                format!("spawn {}", t_spawn - l.start),
            ]
        }
        _ => Vec::new(),
    }
}

/// Every cell of `w`'s box, its comparator output and container, where `w` differs from `r`.
fn cmp_worlds(w: &World, r: &World, tag: &str, skip: &[BlockPos], out: &mut Vec<String>) {
    for x in w.min.x..w.min.x + w.size[0] {
        for y in w.min.y..w.min.y + w.size[1] {
            for z in w.min.z..w.min.z + w.size[2] {
                let p = BlockPos::new(x, y, z);
                if skip.contains(&p) {
                    continue;
                }
                let (a, b) = (w.get(p), r.get(p));
                if a != b {
                    out.push(format!("{tag}{x} {y} {z}: {a} (file: {b})"));
                }
                let (ca, cb) = (w.comparator_out.get(&p).copied().unwrap_or(0), r.comparator_out.get(&p).copied().unwrap_or(0));
                if ca != cb {
                    out.push(format!("{tag}{x} {y} {z}: comparator output {ca} (file: {cb})"));
                }
                let items = |w: &World| -> String {
                    w.containers.map.get(&p).map(|c| c.slots.iter().flatten().map(|s| format!("{} {}", s.count, redsim::redstone::container::item_name(s.item))).collect::<Vec<_>>().join(", ")).unwrap_or_default()
                };
                let (ia, ib) = (items(w), items(r));
                if ia != ib {
                    out.push(format!("{tag}{x} {y} {z}: holds [{ia}] (file: [{ib}])"));
                }
            }
        }
    }
}

/// The cells (states, comparator outputs, container contents) where a run's world differs from
/// the design as written (its raw paste: `Run::start` raw), the Nether part's too: after a build
/// in order, what the order left unlike the file (a design that needs a listed order, or a "place
/// this last" step, shows up here).
pub fn diff_from_file(w: &World, spec: &FarmSpec, ro: &RunOpts) -> Result<Vec<String>, String> {
    let r = Run::start(spec, &RunOpts { raw: true, ..*ro })?;
    let mut out = Vec::new();
    cmp_worlds(w, &r.world, "", &[], &mut out);
    if let (Some(n), Some(rn)) = (w.nether.as_deref(), r.world.nether.as_deref()) {
        cmp_worlds(n, rn, "the Nether's ", &[], &mut out);
    }
    Ok(out)
}

/// Writes `d` (the design `spec` was made from) in its settled state, as a raw paste must find it
/// ("settled as placed"; day-1's loop-gen does the same for the loop): every cell as it stands at
/// the benches' start (the judge's placement and the settle, no entity yet), a cell the settle
/// filled added (a fluid's flow), a cell it emptied removed, each comparator's output as its block
/// entity (`OutputSignal`); the Nether part's too (as written: it must need no turn). A container
/// whose contents the settle moved is an error (its stock would not be the file's). Returns what
/// changed.
pub fn settle_design(d: &mut Design, spec: &FarmSpec) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let (run, _) = start(spec, &ro, Placement::Judge, &BuildPlan::default())?;
    let raw = Run::start(spec, &RunOpts { raw: true, ..ro })?;
    let mut changes = Vec::new();
    write_settled(&run.world, &raw.world, &mut d.blocks, &mut d.block_entities, BlockPos::new(0, 0, 0), "", &mut changes)?;
    if let (Some(ns), Some(n), Some(rn)) = (&spec.nether, run.world.nether.as_deref(), raw.world.nether.as_deref()) {
        if d.nether_turns() != Some(0) {
            return Err("settle_design: a Nether part turned from its file (nether-axis not +x)".into());
        }
        write_settled(n, rn, &mut d.nether_blocks, &mut d.nether_block_entities, ns.base(), "the Nether's ", &mut changes)?;
    }
    Ok(changes)
}

/// [`settle_design`] on a design file, written back in place (its leading `#` lines kept).
pub fn settle_file(path: &std::path::Path) -> anyhow::Result<Vec<String>> {
    let text = std::fs::read_to_string(path)?;
    let c = circuit::Circuit::load(path)?;
    let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow::anyhow!(e))?;
    let mut d = Design::parse(&text)?;
    let changes = settle_design(&mut d, &spec).map_err(|e| anyhow::anyhow!(e))?;
    let mut s: String = text.lines().take_while(|l| l.starts_with('#')).map(|l| format!("{l}\n")).collect();
    s.push_str(&d.to_text());
    std::fs::write(path, s)?;
    Ok(changes)
}

fn write_settled(w: &World, r: &World, blocks: &mut std::collections::BTreeMap<[i32; 3], String>, bes: &mut std::collections::BTreeMap<[i32; 3], String>, base: BlockPos, tag: &str, changes: &mut Vec<String>) -> Result<(), String> {
    let mut diffs = Vec::new();
    cmp_worlds(w, r, "", &[], &mut diffs);
    if let Some(c) = diffs.iter().find(|l| l.contains(": holds [")) {
        return Err(format!("settle_design: the settle moved a container's items ({tag}{c})"));
    }
    for x in w.min.x..w.min.x + w.size[0] {
        for y in w.min.y..w.min.y + w.size[1] {
            for z in w.min.z..w.min.z + w.size[2] {
                let p = BlockPos::new(x, y, z);
                let q = [x - base.x, y - base.y, z - base.z];
                let (a, b) = (w.get(p), r.get(p));
                if a != b {
                    if matches!(a, Block::Air) {
                        blocks.remove(&q);
                    } else {
                        blocks.insert(q, a.to_string());
                    }
                    changes.push(format!("{tag}{} {} {}: {b} -> {a}", q[0], q[1], q[2]));
                }
                let (ca, cb) = (w.comparator_out.get(&p).copied().unwrap_or(0), r.comparator_out.get(&p).copied().unwrap_or(0));
                if ca != cb {
                    if ca > 0 {
                        bes.insert(q, format!("{{OutputSignal:{ca}}}"));
                    } else {
                        bes.remove(&q);
                    }
                    changes.push(format!("{tag}{} {} {}: comparator output {cb} -> {ca}", q[0], q[1], q[2]));
                }
            }
        }
    }
    Ok(())
}
