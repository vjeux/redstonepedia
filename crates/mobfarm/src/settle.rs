//! `mctas mobfarm settle-check` (ironspec 2026-11-12 23:19 (3) / 23:21 / 23:38 / 23:48): a design file
//! must be saved settled, its stored states the fixed point of the game's own updates. The file is laid
//! as the judges lay a design (raw, its block entities, then the judge's update pass), its entities held
//! (present, so a cart on a detector rail or a plate keeps its power, but never ticked), daylight at the
//! file's declared time (`# day-time: N`, else `--day-time`, else noon); then block ticks until no cell
//! changes and nothing is pending, capped at [`CAP`] ticks. A component still changing at the cap is a
//! clock and must be declared (`# clock: x,y,z; x,y,z = REASON`), or the check fails. The fixed point is
//! compared with the stored states: a changed redstone cell fails, listed; so does a shape state that
//! differs from what the block's shape update computes from its neighbours (fence, pane and bars arms;
//! rail shapes, by the game's own placement rule `RailState.place`; dust connections, by the judge's
//! update pass), since a wrong stored shape makes redsim collide differently from the game. Walls and
//! stairs have no shape update in redsim: their cells are UNCHECKED and fail unless allowed. Every bar
//! prints its sample count; 0 samples is NOT JUDGED and fails (ironspec 23:56).

use redsim::block::Block;
use redsim::pos::{BlockPos, Dir};
use redsim::world::World;

/// The settle loop's cap (ironspec 23:21).
pub const CAP: u32 = 200;

/// The ticks before the cap whose changes mark a cell as still changing: a clock.
pub const CLOCK_WINDOW: u32 = 40;

/// The file's declarations: its clocks (`# clock: x,y,z; x,y,z = REASON`, each cell with its reason)
/// and its declared day time (`# day-time: N`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Declared {
    pub clocks: Vec<(BlockPos, String)>,
    pub day_time: Option<i64>,
    /// The fluid cells laid by no build that the design means (`# water: x,y,z=REASON; x,y,z=REASON`:
    /// a lift's top-drift current, 7b90c3f1's key, farmopt 01:06), each with its reason.
    pub water: Vec<(BlockPos, String)>,
    /// The file's stand-ins (`# stand-in:` lines, read as record-match reads them) and the ones its
    /// bench holds (`# held: x,y,z=REAL DRIVER, CONTROLLER PHASE; …`, ironspec 01:07 (d)).
    pub standins: Vec<BlockPos>,
    pub held: Vec<(BlockPos, String)>,
}

/// The declarations read from the file's comment lines.
pub fn declared(text: &str) -> Result<Declared, String> {
    let mut d = Declared::default();
    let cell = |l: &str, c: &str| -> Result<BlockPos, String> {
        let n: Vec<i32> = c.split(',').map(|x| x.trim().replace('\u{2212}', "-").parse::<i32>()).collect::<Result<_, _>>().map_err(|e| format!("{l:?}: {c}: {e}"))?;
        if n.len() != 3 {
            return Err(format!("{l:?}: {c}: give x,y,z"));
        }
        Ok(BlockPos::new(n[0], n[1], n[2]))
    };
    for line in text.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("# clock:") {
            let (cells, why) = rest.split_once('=').ok_or_else(|| format!("{l:?}: give `# clock: x,y,z; x,y,z = REASON`"))?;
            let why = why.trim();
            if why.is_empty() {
                return Err(format!("{l:?}: a clock needs its REASON"));
            }
            for c in cells.split(';').map(str::trim).filter(|c| !c.is_empty()) {
                d.clocks.push((cell(l, c)?, why.to_string()));
            }
        } else if let Some(rest) = l.strip_prefix("# water:") {
            for entry in rest.split(';').map(str::trim).filter(|e| !e.is_empty()) {
                let (c, why) = entry.split_once('=').ok_or_else(|| format!("{l:?}: {entry}: give `x,y,z=REASON`"))?;
                if why.trim().is_empty() {
                    return Err(format!("{l:?}: {entry}: a water cell needs its REASON"));
                }
                d.water.push((cell(l, c)?, why.trim().to_string()));
            }
        } else if let Some(rest) = l.strip_prefix("# held:") {
            for entry in rest.split(';').map(str::trim).filter(|e| !e.is_empty()) {
                let (c, why) = entry.split_once('=').ok_or_else(|| format!("{l:?}: {entry}: give `x,y,z=REAL DRIVER, CONTROLLER PHASE`"))?;
                if why.trim().is_empty() {
                    return Err(format!("{l:?}: {entry}: a held stand-in names its real driver and controller phase"));
                }
                d.held.push((cell(l, c)?, why.trim().to_string()));
            }
        } else if let Some(rest) = l.strip_prefix("# day-time:") {
            d.day_time = Some(rest.trim().parse::<i64>().map_err(|e| format!("{l:?}: {e}"))?);
        }
    }
    d.standins = crate::recordmatch::declared_from_header(text)?.standins.keys().map(|c| BlockPos::new(c[0], c[1], c[2])).collect();
    Ok(d)
}

/// What a cell's change counts as: a redstone component (its change fails), a shape-bearing block (fence,
/// pane, bars, wall, stairs: its arms are checked by the shape rule), or anything else (printed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Redstone,
    Shape,
    Other,
}

/// The block's id (`minecraft:...`, no properties).
fn id_of(b: &Block) -> String {
    let s = b.to_string();
    s.split('[').next().unwrap_or(&s).to_string()
}

/// [`Class`] of a block, by its id: rails and every component that carries or reacts to power.
pub fn class_of(b: &Block) -> Class {
    let id = id_of(b);
    let id = id.trim_start_matches("minecraft:");
    const REDSTONE: [&str; 27] = [
        "redstone", "repeater", "comparator", "observer", "piston", "lever", "button", "rail", "lamp", "daylight_detector", "target", "note_block", "dispenser", "dropper", "hopper",
        "trapdoor", "_door", "fence_gate", "pressure_plate", "tripwire", "sculk_sensor", "bell", "crafter", "copper_bulb", "lightning_rod", "tnt", "moving_piston",
    ];
    if REDSTONE.iter().any(|k| id.contains(k)) || id == "iron_door" {
        return Class::Redstone;
    }
    if id.ends_with("_fence") || id.ends_with("_pane") || id == "glass_pane" || id == "iron_bars" || id.ends_with("_wall") || id.ends_with("_stairs") {
        return Class::Shape;
    }
    Class::Other
}

/// One differing cell: its position, the stored state, the fixed point's (or the computed shape's).
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub pos: BlockPos,
    pub stored: String,
    pub settled: String,
}

/// The check's result.
#[derive(Clone, Debug, Default)]
pub struct Settle {
    /// The ticks run: to the fixed point, or the cap.
    pub ticks: u32,
    /// A fixed point reached (a tick with no change and nothing pending).
    pub fixed: bool,
    /// Cells still changing in the last [`CLOCK_WINDOW`] ticks before the cap, declared and not.
    pub clock_declared: Vec<BlockPos>,
    pub clock_undeclared: Vec<BlockPos>,
    /// Redstone cells whose fixed point differs from the stored state (the declared clocks' cells
    /// excluded).
    pub redstone: Vec<Cell>,
    /// Shape-bearing cells whose computed shape differs from the stored state.
    pub shapes: Vec<Cell>,
    /// Other cells that changed (printed, not failing: fluids, plants).
    pub other: Vec<Cell>,
    /// Fluid cells the settle made where the file holds none (a source spreading, the infinite rule):
    /// named in a `# water:` line (with its purpose) or not; either way the saved file must hold them
    /// (ironspec 01:07 (b)): nonzero fails.
    pub water_declared: Vec<(Cell, String)>,
    pub water_undeclared: Vec<Cell>,
    /// The `# water:` cells the file holds as fluid: designed currents (ironspec 01:07 (c)), each with
    /// its purpose; those holding no water; those unbounded in the saved design (a neighbour, below or
    /// beside, that is air, washed away by fluids or a dry waterloggable block: its spread not blocked).
    pub currents: Vec<(BlockPos, String)>,
    pub currents_missing: Vec<BlockPos>,
    pub currents_unbounded: Vec<(BlockPos, BlockPos, String)>,
    /// Walls and stairs: no shape update in redsim, unchecked.
    pub unchecked: Vec<Cell>,
    /// The model notes redsim recorded laying and settling the file (`World::errors`: water reaching a
    /// waterloggable block redsim keeps dry, …): the fixed point isn't modelled there, so the check fails.
    pub model_notes: Vec<String>,
    /// The observers kicked once each on a copy of the fixed point (their pulse finds an observer clock
    /// the placement never starts), and the cells where the kicked copy settled elsewhere (printed: the
    /// state depends on the build order there).
    pub observers_kicked: usize,
    pub kick_sensitive: Vec<Cell>,
    /// The stand-ins' SURVIVAL verdicts (`recordmatch::guard_survival`, ironspec 01:07 (d)): passing, and
    /// failing with why.
    pub standins_ok: usize,
    pub standins_failing: Vec<(BlockPos, String)>,
    /// The samples: the cells compared, the redstone cells among them, the shape cells checked, the fluid
    /// cells given their tick (ironspec 00:27 (B): printed).
    pub compared: usize,
    pub redstone_cells: usize,
    pub shape_cells: usize,
    pub fluid_cells: usize,
    /// The settled states of the file's cells (the fixed point, computed shapes applied, the declared
    /// clocks as stored), for `--write`.
    pub settled: Vec<(BlockPos, Block)>,
}

impl Settle {
    /// The failures (empty: it passes); `allow_unchecked` lets wall and stair cells stand unchecked.
    pub fn fails(&self, allow_unchecked: bool) -> Vec<String> {
        let mut v = Vec::new();
        if self.compared == 0 {
            v.push("NOT JUDGED (0 samples: no cell compared)".to_string());
        }
        if !self.clock_undeclared.is_empty() {
            v.push(format!("an undeclared clock: {} cells still changing at the cap ({})", self.clock_undeclared.len(), cells(&self.clock_undeclared)));
        }
        if !self.water_undeclared.is_empty() || !self.water_declared.is_empty() {
            v.push(format!(
                "{} new fluid cells after the settle ({} named in `# water:`): a saved-settled file holds its water, so re-save it with --write (ironspec 01:07 (b))",
                self.water_undeclared.len() + self.water_declared.len(),
                self.water_declared.len()
            ));
        }
        if !self.currents_missing.is_empty() {
            v.push(format!("{} `# water:` cells hold no water in the file or at the fixed point ({})", self.currents_missing.len(), cells(&self.currents_missing)));
        }
        if !self.currents_unbounded.is_empty() {
            v.push(format!(
                "{} designed currents unbounded (an air, washed-away or dry waterloggable neighbour: {})",
                self.currents_unbounded.len(),
                self.currents_unbounded.iter().take(6).map(|(p, q, b)| format!("({},{},{}) -> ({},{},{}) {b}", p.x, p.y, p.z, q.x, q.y, q.z)).collect::<Vec<_>>().join("; ")
            ));
        }
        if !self.standins_failing.is_empty() {
            v.push(format!(
                "{} stand-ins fail SURVIVAL ({})",
                self.standins_failing.len(),
                self.standins_failing.iter().take(6).map(|(p, why)| format!("({},{},{}) {why}", p.x, p.y, p.z)).collect::<Vec<_>>().join("; ")
            ));
        }
        if !self.redstone.is_empty() {
            v.push(format!("{} redstone cells stored unsettled", self.redstone.len()));
        }
        if !self.shapes.is_empty() {
            v.push(format!("{} shape states differing from their shape update", self.shapes.len()));
        }
        if !self.unchecked.is_empty() && !allow_unchecked {
            v.push(format!("{} wall / stair cells UNCHECKED (redsim models no wall or stair shape update)", self.unchecked.len()));
        }
        if !self.model_notes.is_empty() {
            v.push(format!("NOT MODELLED: {} model notes laying and settling the file ({})", self.model_notes.len(), self.model_notes.iter().take(4).cloned().collect::<Vec<_>>().join("; ")));
        }
        v
    }
}

fn cells(v: &[BlockPos]) -> String {
    let mut s: Vec<String> = v.iter().take(12).map(|p| format!("{},{},{}", p.x, p.y, p.z)).collect();
    if v.len() > 12 {
        s.push("...".into());
    }
    s.join("; ")
}

/// The shape rule for the cell at `p` of `w` (the fixed point): None for a block it doesn't cover;
/// else (ok, the expected state). A fence's, a pane's or iron bars' arms must equal their own shape update
/// from the neighbours (`updateShape`, run on `scratch`, a copy of `w`: a shape update can schedule, an
/// observer's, and nothing of it may reach the checked world; the copy's blocks never change). A rail with
/// at most two rails to join must store the shape the game's placement rule gives it (`RailState.place`:
/// every build order agrees there; a plain rail's signal its own power); a junction (three or four) may
/// store any build order's shape (the junction check's ruling, ironspec Nov 12 02:46: placement-order
/// shapes accepted), which joins two of its arms: a stored exit toward no rail fails.
pub fn shape_rule(w: &World, scratch: &mut World, p: BlockPos) -> Option<(bool, Block)> {
    let b = w.get(p);
    if redsim::rails::is_rail(&b) {
        let st = redsim::rails::RailState::new(p, b);
        let stored = redsim::rails::shape_of(&b);
        if st.count_potential_connections(w) >= 3 {
            let (a, c) = redsim::rails::exits(stored);
            let arm = |e: [i32; 3]| {
                let q = BlockPos::new(p.x + e[0], p.y, p.z + e[2]);
                [q, q.above(), q.below()].iter().any(|r| redsim::rails::is_rail(&w.get(*r)))
            };
            return Some((arm(a) && arm(c), b));
        }
        let plain = matches!(b, Block::PlainRail { .. });
        let shape = st.shape_for(w, plain && w.has_neighbor_signal(p), stored);
        let want = match b {
            Block::Rail { activator, powered, .. } => Block::Rail { activator, shape, powered },
            Block::PlainRail { .. } => Block::PlainRail { shape },
            Block::DetectorRail { powered, .. } => Block::DetectorRail { shape, powered },
            x => x,
        };
        return Some((want == b, want));
    }
    let fenceish = matches!(b, Block::Fence { .. } | Block::Pane { .. }) || matches!(b, Block::Terrain(id) if redsim::terrain::plants::connects(id));
    if !fenceish {
        return None;
    }
    let mut st = b;
    for d in Dir::HORIZONTAL {
        let np = p.rel(d);
        let ns = scratch.get(np);
        st = redsim::behavior::update_shape(scratch, st, p, d, np, ns);
    }
    Some((st == b, st))
}

/// The stand-ins' SURVIVAL for a bench (ironspec 01:07 (d): sight-diff, sensor-at, f1q2-run, the A6 bench
/// call it on their laid world): a copy of `w` settled with its entities held (block ticks until nothing
/// changes and nothing is pending, capped at `cap`), then `recordmatch::guard_survival` on each stand-in,
/// `held` naming the ones the bench holds (their real driver and controller phase).
pub fn stand_in_survival(w: &World, standins: &[BlockPos], held: &[(BlockPos, String)], cap: u32) -> Vec<(BlockPos, crate::recordmatch::Verdict)> {
    let mut settled = w.clone();
    settled.entity_chunks = Some([1, 1, 0, 0]);
    for _ in 0..cap {
        let before: Vec<Block> = standins.iter().map(|p| settled.get(*p)).collect();
        settled.tick();
        if settled.idle() && standins.iter().map(|p| settled.get(*p)).collect::<Vec<_>>() == before {
            break;
        }
    }
    standins.iter().map(|p| (*p, crate::recordmatch::guard_survival(w, &settled, *p, held.iter().find(|(q, _)| q == p).map(|(_, h)| h.as_str())))).collect()
}

/// A design cell's state as redsim's block, as the farm loader reads it (`circuit::studies::mobfarm`'s
/// `farm_block`: a Nether portal through the terrain table, everything else `Block::parse`).
pub fn block_of(s: &str) -> Result<Block, String> {
    if s.starts_with("minecraft:nether_portal[") {
        return redsim::terrain::land::block_of(s);
    }
    Block::parse(s).map_err(|e| e.to_string())
}

/// The file re-emitted settled (`--write`): its leading `#` header block verbatim (every line up to the
/// first that is neither blank nor a comment), then the design with its blocks replaced by `settled`'s
/// non-air states (`Design::to_text`; comment lines after the header are not kept).
pub fn settled_text(text: &str, d: &mctas::design::Design, settled: &[(BlockPos, Block)]) -> String {
    let mut d2 = d.clone();
    d2.blocks = settled.iter().filter(|(_, b)| !b.is_air()).map(|(p, b)| ([p.x, p.y, p.z], b.to_string())).collect();
    let lead: String = text.lines().take_while(|l| l.trim().is_empty() || l.trim_start().starts_with('#')).map(|l| format!("{l}\n")).collect();
    format!("{lead}{}", d2.to_text())
}

/// The leading `#` header block's lines of a design file (blank lines skipped).
pub fn header_lines(text: &str) -> Vec<&str> {
    text.lines().take_while(|l| l.trim().is_empty() || l.trim_start().starts_with('#')).filter(|l| !l.trim().is_empty()).collect()
}

/// Lay the design as the judges do, hold its entities, settle it and compare (see the module's doc).
/// `blocks`: the file's non-air cells; `block_entities`: its `nbt:` lines; `entities`: (kind, position,
/// NBT). `day_time`: the daylight's time.
pub fn settle_check(blocks: &[(BlockPos, Block)], block_entities: &[(BlockPos, String)], entities: &[(String, [f64; 3], Option<String>)], decl: &Declared, day_time: i64, cap: u32) -> Result<Settle, String> {
    if blocks.is_empty() {
        return Ok(Settle::default());
    }
    let (mut lo, mut hi) = (blocks[0].0, blocks[0].0);
    for (p, _) in blocks {
        lo = BlockPos::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = BlockPos::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    // (a margin of 2 around the file's cells: a piston's head or a flow lands inside the compare)
    let lo = BlockPos::new(lo.x - 2, lo.y - 2, lo.z - 2);
    let hi = BlockPos::new(hi.x + 2, hi.y + 2, hi.z + 2);
    let mut w = World::new(lo, [hi.x - lo.x + 1, hi.y - lo.y + 1, hi.z - lo.z + 1]);
    w.time = day_time;
    w.place_raw(blocks);
    redsim::redstone::container::apply_block_entities(&mut w, block_entities.iter().map(|(p, s)| (*p, s.as_str())))?;
    w.place_updates(blocks);
    // (the entities held: present for the blocks that sense them, never ticked: an empty entity-ticking
    // range)
    w.entity_chunks = Some([1, 1, 0, 0]);
    for (kind, pos, nbt) in entities {
        redsim::entity::spawn(&mut w, kind, *pos, nbt.as_deref())?;
    }
    // (every water, lava and waterlogged cell gets its fluid tick, so a file that isn't fluid-settled
    // shows it (farmopt 00:27, C7's leak: a dry rail between two sources over a solid block becomes a
    // source, FlowingFluid.getNewLiquid's infinite rule); a model note on the way fails the check)
    let mut fluid_cells = 0usize;
    for (p, b) in blocks {
        let f = redsim::fluid::fluid_state_of(b);
        if !matches!(f.fluid, redsim::fluid::Fluid::Empty) {
            let d = redsim::fluid::tick_delay(&w, f.fluid);
            w.schedule_fluid_tick(*p, f.fluid, d);
            fluid_cells += 1;
        }
    }
    // (the laid world, kept for the stand-ins' SURVIVAL against the fixed point)
    let laid = w.clone();
    let stored: std::collections::HashMap<BlockPos, Block> = blocks.iter().cloned().collect();
    let all: Vec<BlockPos> = (lo.x..=hi.x).flat_map(|x| (lo.y..=hi.y).flat_map(move |y| (lo.z..=hi.z).map(move |z| BlockPos::new(x, y, z)))).collect();
    let snap = |w: &World| -> Vec<Block> { all.iter().map(|p| w.get(*p)).collect() };
    let mut prev = snap(&w);
    let mut last_change: Vec<u32> = vec![0; all.len()];
    let mut changed_ever: Vec<bool> = vec![false; all.len()];
    let (mut ticks, mut fixed) = (0u32, false);
    for t in 1..=cap {
        w.tick();
        ticks = t;
        let now = snap(&w);
        let mut any = false;
        for i in 0..all.len() {
            if now[i] != prev[i] {
                any = true;
                last_change[i] = t;
                changed_ever[i] = true;
            }
        }
        prev = now;
        if !any && w.idle() {
            fixed = true;
            break;
        }
    }
    let declared_cells: std::collections::HashSet<BlockPos> = decl.clocks.iter().map(|(p, _)| *p).collect();
    let mut s = Settle { ticks, fixed, model_notes: w.errors.clone(), fluid_cells, ..Default::default() };
    if !fixed {
        for (i, p) in all.iter().enumerate() {
            if changed_ever[i] && last_change[i] + CLOCK_WINDOW > ticks {
                if declared_cells.contains(p) {
                    s.clock_declared.push(*p);
                } else {
                    s.clock_undeclared.push(*p);
                }
            }
        }
    } else {
        // (the observers' kick: a build places each observer's target before it or after it, and the
        // judges' placement runs no observer's shape update; one pulse per unpowered observer, on a copy
        // of the fixed point, finds an observer clock that never starts from the placement (ironspec
        // 23:21: an undeclared observer clock fails). A kick that settles elsewhere is printed: the state
        // depends on the build order there.)
        let observers: Vec<BlockPos> = all.iter().copied().filter(|p| matches!(w.get(*p), Block::Observer { powered: false, .. })).collect();
        s.observers_kicked = observers.len();
        if !observers.is_empty() {
            let mut k = w.clone();
            for p in &observers {
                let kind = k.get(*p).kind();
                k.schedule_tick(*p, kind, 2, redsim::ticks::Priority::Normal);
            }
            let mut prev = snap(&k);
            let mut last: Vec<u32> = vec![0; all.len()];
            let mut ever: Vec<bool> = vec![false; all.len()];
            let mut settled_again = false;
            for t in 1..=cap {
                k.tick();
                let now = snap(&k);
                let mut any = false;
                for i in 0..all.len() {
                    if now[i] != prev[i] {
                        any = true;
                        last[i] = t;
                        ever[i] = true;
                    }
                }
                prev = now;
                if !any && k.idle() {
                    settled_again = true;
                    break;
                }
            }
            for (i, p) in all.iter().enumerate() {
                if !settled_again && ever[i] && last[i] + CLOCK_WINDOW > cap {
                    if declared_cells.contains(p) {
                        s.clock_declared.push(*p);
                    } else {
                        s.clock_undeclared.push(*p);
                    }
                } else if settled_again && k.get(*p) != w.get(*p) {
                    s.kick_sensitive.push(Cell { pos: *p, stored: w.get(*p).to_string(), settled: k.get(*p).to_string() });
                }
            }
        }
    }
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    // (one scratch copy of the fixed point for every shape update)
    let mut scratch = w.clone();
    for p in &all {
        let st = stored.get(p).copied().unwrap_or(air);
        let now = w.get(*p);
        if st.is_air() && now.is_air() {
            continue;
        }
        s.compared += 1;
        let class = if st.is_air() { class_of(&now) } else { class_of(&st) };
        if class == Class::Redstone {
            s.redstone_cells += 1;
        }
        let mut out = now;
        if declared_cells.contains(p) {
            out = st;
        } else if st != now {
            let cell = Cell { pos: *p, stored: st.to_string(), settled: now.to_string() };
            // (a fluid cell laid by no build: declared in a `# water:` line, or UNDECLARED, failing)
            let new_fluid = matches!(redsim::fluid::fluid_state_of(&st).fluid, redsim::fluid::Fluid::Empty) && !matches!(redsim::fluid::fluid_state_of(&now).fluid, redsim::fluid::Fluid::Empty);
            if new_fluid {
                match decl.water.iter().find(|(q, _)| q == p) {
                    Some((_, why)) => s.water_declared.push((cell, why.clone())),
                    None => s.water_undeclared.push(cell),
                }
            } else {
                match class {
                    Class::Redstone => s.redstone.push(cell),
                    Class::Shape | Class::Other => s.other.push(cell),
                }
            }
        }
        // (the shape rule: the fixed point's shape-bearing cells against their own shape update)
        if let Some((ok, want)) = shape_rule(&w, &mut scratch, *p) {
            s.shape_cells += 1;
            let clocked = declared_cells.contains(p);
            if !ok && !clocked && !s.redstone.iter().any(|x| x.pos == *p) {
                s.shapes.push(Cell { pos: *p, stored: st.to_string(), settled: want.to_string() });
            }
            if !clocked {
                out = want;
            }
        } else if class == Class::Shape && !st.is_air() {
            s.unchecked.push(Cell { pos: *p, stored: st.to_string(), settled: now.to_string() });
        }
        if !out.is_air() || !st.is_air() {
            s.settled.push((*p, out));
        }
    }
    // (the stand-ins' SURVIVAL, ironspec 01:07 (d): each survives where it stands and stays as laid
    // through the settle, unless held)
    for p in &decl.standins {
        let held = decl.held.iter().find(|(q, _)| q == p).map(|(_, h)| h.as_str());
        match crate::recordmatch::guard_survival(&laid, &w, *p, held) {
            Ok(_) => s.standins_ok += 1,
            Err(why) => s.standins_failing.push((*p, why)),
        }
    }
    // (the designed currents, ironspec 01:07 (c): each `# water:` cell the file holds as fluid, bounded
    // in the saved design: no neighbour below or beside it that is air, washed away by fluids or a dry
    // waterloggable block, so its spread stays blocked)
    for (p, why) in &decl.water {
        let st = stored.get(p).copied().unwrap_or(air);
        let is_fluid = |b: &Block| !matches!(redsim::fluid::fluid_state_of(b).fluid, redsim::fluid::Fluid::Empty);
        if !is_fluid(&st) {
            if !is_fluid(&w.get(*p)) {
                s.currents_missing.push(*p);
            }
            continue;
        }
        s.currents.push((*p, why.clone()));
        for d in [Dir::Down, Dir::North, Dir::East, Dir::South, Dir::West] {
            let q = p.rel(d);
            let nb = stored.get(&q).copied().unwrap_or(air);
            let open = !is_fluid(&nb) && (nb.is_air() || redsim::fluid::natural_washed_away(&nb.to_string()) || redsim::fluid::waterlogged(&nb) == Some(false));
            if open {
                s.currents_unbounded.push((*p, q, nb.to_string()));
            }
        }
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(s: &str) -> Block {
        Block::parse(s).unwrap()
    }

    fn at(x: i32, y: i32, z: i32, s: &str) -> (BlockPos, Block) {
        (BlockPos::new(x, y, z), b(s))
    }

    fn check(blocks: &[(BlockPos, Block)], decl: &Declared) -> Settle {
        settle_check(blocks, &[], &[], decl, 6000, CAP).unwrap()
    }

    /// The floor under a layout: stone at y = -1 over x 0..n, z -1..1.
    fn floor(n: i32) -> Vec<(BlockPos, Block)> {
        (-1..=n).flat_map(|x| (-1..=1).map(move |z| at(x, -1, z, "minecraft:stone"))).collect()
    }

    /// A lever on, two delay-4 repeaters and a lamp: stored at its fixed point (repeaters on, lamp lit)
    /// it passes, though it takes 16 gt to settle from an unpowered start; stored with the repeaters and
    /// the lamp off (the positive control) it fails, listing the three cells.
    #[test]
    fn repeater_chain() {
        let mut settled = floor(4);
        settled.extend([
            at(0, 0, 0, "minecraft:lever[face=floor,facing=east,powered=true]"),
            at(1, 0, 0, "minecraft:repeater[delay=4,facing=west,locked=false,powered=true]"),
            at(2, 0, 0, "minecraft:repeater[delay=4,facing=west,locked=false,powered=true]"),
            at(3, 0, 0, "minecraft:redstone_lamp[lit=true]"),
        ]);
        let s = check(&settled, &Declared::default());
        assert!(s.fails(false).is_empty() && s.fixed, "{:?} {:?} {:?}", s.fails(false), s.redstone, s.shapes);
        assert!(s.compared > 0 && s.redstone_cells == 4, "{} {}", s.compared, s.redstone_cells);
        let mut raw = floor(4);
        raw.extend([
            at(0, 0, 0, "minecraft:lever[face=floor,facing=east,powered=true]"),
            at(1, 0, 0, "minecraft:repeater[delay=4,facing=west,locked=false,powered=false]"),
            at(2, 0, 0, "minecraft:repeater[delay=4,facing=west,locked=false,powered=false]"),
            at(3, 0, 0, "minecraft:redstone_lamp[lit=false]"),
        ]);
        let s = check(&raw, &Declared::default());
        assert!(s.fixed && s.ticks >= 16, "{} ticks", s.ticks);
        let cells: Vec<BlockPos> = s.redstone.iter().map(|c| c.pos).collect();
        assert_eq!(cells, vec![BlockPos::new(1, 0, 0), BlockPos::new(2, 0, 0), BlockPos::new(3, 0, 0)]);
        assert_eq!(s.fails(false), vec!["3 redstone cells stored unsettled".to_string()]);
    }

    /// The positive controls: dust beside a lit torch stored unpowered, a repeater stored off with its
    /// input on: each fails.
    #[test]
    fn unsettled_components() {
        let mut dust = floor(3);
        dust.extend([at(0, 0, 0, "minecraft:redstone_torch[lit=true]"), at(1, 0, 0, "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]"), at(2, 0, 0, "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=side]")]);
        let s = check(&dust, &Declared::default());
        assert!(s.redstone.iter().any(|c| c.pos == BlockPos::new(1, 0, 0) && c.settled.contains("power=15")), "{:?}", s.redstone);
        assert!(!s.fails(false).is_empty());
        let mut rep = floor(2);
        rep.extend([at(0, 0, 0, "minecraft:redstone_block"), at(1, 0, 0, "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]")]);
        let s = check(&rep, &Declared::default());
        assert_eq!(s.redstone.iter().map(|c| c.pos).collect::<Vec<_>>(), vec![BlockPos::new(1, 0, 0)]);
    }

    /// A ring of four delay-1 repeaters joined by dust at its corners, one repeater stored powered: the
    /// pulse runs round it for ever. Undeclared it fails at the cap; declared (`# clock:`) it passes, its
    /// cells left out of the compare.
    #[test]
    fn repeater_ring_clock() {
        let mut ring = floor(2);
        ring.extend((-1..=2).flat_map(|x| (2..=3).map(move |z| at(x, -1, z, "minecraft:stone"))));
        let dot = "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]";
        ring.extend([
            at(0, 0, 0, "minecraft:repeater[delay=1,facing=west,locked=false,powered=true]"),
            at(1, 0, 0, dot),
            at(1, 0, 1, "minecraft:repeater[delay=1,facing=north,locked=false,powered=false]"),
            at(1, 0, 2, dot),
            at(0, 0, 2, "minecraft:repeater[delay=1,facing=east,locked=false,powered=false]"),
            at(-1, 0, 2, dot),
            at(-1, 0, 1, "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]"),
            at(-1, 0, 0, dot),
        ]);
        let s = check(&ring, &Declared::default());
        assert!(!s.fixed && s.ticks == CAP && !s.clock_undeclared.is_empty(), "{:?} {:?}", s.fixed, s.clock_undeclared);
        assert!(s.fails(false).iter().any(|f| f.starts_with("an undeclared clock")), "{:?}", s.fails(false));
        let decl = declared("# clock: 0,0,0; 1,0,0; 1,0,1; 1,0,2; 0,0,2; -1,0,2; -1,0,1; -1,0,0 = the repeater ring's pulse\n").unwrap();
        let s = check(&ring, &decl);
        assert!(s.clock_undeclared.is_empty() && !s.clock_declared.is_empty() && s.fails(false).is_empty(), "{:?} {:?} {:?}", s.clock_undeclared, s.fails(false), s.redstone);
    }

    /// Two observers facing each other: a clock the judges' placement never starts (it runs no observer's
    /// shape update), found by the observers' kick: undeclared it fails, declared it passes; a lone observer
    /// watching a lamp is kicked too and settles back (no clock, nothing flagged).
    #[test]
    fn observer_clock() {
        let mut pair = floor(2);
        pair.extend([at(0, 0, 0, "minecraft:observer[facing=east,powered=false]"), at(1, 0, 0, "minecraft:observer[facing=west,powered=false]")]);
        let s = check(&pair, &Declared::default());
        assert!(s.fixed && s.observers_kicked == 2, "{} {}", s.fixed, s.observers_kicked);
        assert!(!s.clock_undeclared.is_empty() && s.fails(false).iter().any(|f| f.starts_with("an undeclared clock")), "{:?} {:?}", s.clock_undeclared, s.fails(false));
        let decl = declared("# clock: 0,0,0; 1,0,0 = the observer pair\n").unwrap();
        let s = check(&pair, &decl);
        assert!(s.clock_undeclared.is_empty() && s.clock_declared.len() == 2 && s.fails(false).is_empty(), "{:?} {:?}", s.clock_undeclared, s.fails(false));
        let mut lone = floor(2);
        lone.extend([at(0, 0, 0, "minecraft:observer[facing=east,powered=false]"), at(1, 0, 0, "minecraft:redstone_lamp[lit=false]")]);
        let s = check(&lone, &Declared::default());
        assert!(s.observers_kicked == 1 && s.clock_undeclared.is_empty() && s.kick_sensitive.is_empty() && s.fails(false).is_empty(), "{:?} {:?} {:?}", s.clock_undeclared, s.kick_sensitive, s.fails(false));
    }

    /// The shape rule: a fence stored with an arm toward air fails; stored with its computed arms it
    /// passes; a rail joined by straights from the west and the north must store the north-west curve
    /// (ironspec 23:48's record7 cell); a junction stored with an exit toward no rail fails, a build order's
    /// shape joining two of its arms stands.
    #[test]
    fn shapes() {
        let mut f = floor(3);
        f.extend([at(0, 0, 0, "minecraft:oak_fence[east=true,north=true,south=false,waterlogged=false,west=false]"), at(1, 0, 0, "minecraft:oak_fence[east=false,north=false,south=false,waterlogged=false,west=true]")]);
        let s = check(&f, &Declared::default());
        assert_eq!(s.shapes.iter().map(|c| c.pos).collect::<Vec<_>>(), vec![BlockPos::new(0, 0, 0)], "the arm toward air at north");
        assert!(s.fails(false).iter().any(|x| x.contains("shape states")));
        let k = f.len() - 2;
        f[k] = at(0, 0, 0, "minecraft:oak_fence[east=true,north=false,south=false,waterlogged=false,west=false]");
        let s = check(&f, &Declared::default());
        assert!(s.shapes.is_empty() && s.shape_cells == 2, "{:?}", s.shapes);
        // (a curve: rails at west (x-1) and north (z-1) of (1,0,1), straight toward it)
        let mut r = floor(3);
        r.extend((-1..=3).map(|x| at(x, -1, 2, "minecraft:stone")));
        r.extend([at(0, 0, 1, "minecraft:rail[shape=east_west,waterlogged=false]"), at(1, 0, 0, "minecraft:rail[shape=north_south,waterlogged=false]"), at(1, 0, 1, "minecraft:rail[shape=north_south,waterlogged=false]")]);
        let s = check(&r, &Declared::default());
        let c = s.shapes.iter().find(|c| c.pos == BlockPos::new(1, 0, 1)).expect("the curve cell flagged");
        assert!(c.settled.contains("shape=north_west"), "{c:?}");
        let mut t = floor(3);
        t.extend((-1..=3).map(|x| at(x, -1, 2, "minecraft:stone")));
        t.extend([
            at(0, 0, 1, "minecraft:rail[shape=east_west,waterlogged=false]"),
            at(2, 0, 1, "minecraft:rail[shape=east_west,waterlogged=false]"),
            at(1, 0, 2, "minecraft:rail[shape=north_south,waterlogged=false]"),
            at(1, 0, 1, "minecraft:rail[shape=north_south,waterlogged=false]"),
        ]);
        let s = check(&t, &Declared::default());
        assert!(s.shapes.iter().any(|c| c.pos == BlockPos::new(1, 0, 1)), "a junction (arms west, east, south) stored north_south, an exit toward no rail, fails: {:?}", s.shapes);
        // (a build order's shape joining two of its arms stands: the junction check's ruling)
        let n = t.len();
        t[n - 1] = at(1, 0, 1, "minecraft:rail[shape=south_west,waterlogged=false]");
        let s = check(&t, &Declared::default());
        assert!(!s.shapes.iter().any(|c| c.pos == BlockPos::new(1, 0, 1)), "{:?}", s.shapes);
    }

    /// Fluid-settled (farmopt 00:27, C7's leak): every water cell gets its fluid tick, so a dry rail
    /// between two water sources over a solid block (the infinite rule makes it a source; redsim keeps a
    /// rail dry and records the note) fails; the same sources with stone between them pass, the water
    /// already at its fixed point.
    #[test]
    fn fluid_settled() {
        let mut leak = floor(3);
        leak.extend([at(0, 0, 0, "minecraft:water[level=0]"), at(1, 0, 0, "minecraft:rail[shape=north_south,waterlogged=false]"), at(2, 0, 0, "minecraft:water[level=0]")]);
        for (x, z) in [(0, -1), (0, 1), (1, -1), (1, 1), (2, -1), (2, 1), (-1, 0), (3, 0)] {
            leak.push(at(x, 0, z, "minecraft:stone"));
        }
        let s = check(&leak, &Declared::default());
        let f = s.fails(false);
        assert!(f.iter().any(|x| x.starts_with("NOT MODELLED") && x.contains("waterlog")), "{f:?} {:?}", s.redstone);
        let n = leak.len();
        let mut walled = leak.clone();
        walled[n - 10] = at(1, 0, 0, "minecraft:stone");
        let s = check(&walled, &Declared::default());
        assert!(s.fails(false).is_empty(), "{:?} {:?}", s.fails(false), s.other);
    }

    /// Fluid cells laid by no build (farmopt 01:06, ironspec 01:07 (b) / (c)): a source in a stone trough
    /// open one cell to the east spreads into it, so the file isn't saved settled and fails, named in a
    /// `# water:` line or not; re-saved with `--write` the flowing cell is in the file: 0 new water, and the
    /// named cell a designed current, bounded (stone below and beside, the source behind): it passes. With
    /// the trough's end wall gone the current is unbounded (air east of it) and fails.
    #[test]
    fn new_fluid_cells_declared() {
        let mut t = floor(3);
        t.push(at(0, 0, 0, "minecraft:water[level=0]"));
        for (x, z) in [(-1, 0), (0, -1), (0, 1), (1, -1), (1, 1), (2, 0)] {
            t.push(at(x, 0, z, "minecraft:stone"));
        }
        let s = check(&t, &Declared::default());
        assert_eq!(s.water_undeclared.iter().map(|c| c.pos).collect::<Vec<_>>(), vec![BlockPos::new(1, 0, 0)], "{:?}", s.other);
        assert!(s.fluid_cells == 1 && s.fails(false).iter().any(|f| f.contains("new fluid cells after the settle (0 named")), "{:?}", s.fails(false));
        let decl = declared("# water: 1,0,0=the trough's current\n").unwrap();
        let s = check(&t, &decl);
        assert!(s.water_undeclared.is_empty() && s.water_declared.len() == 1 && s.water_declared[0].1 == "the trough's current", "{:?}", s.water_declared);
        assert!(s.fails(false).iter().any(|f| f.contains("new fluid cells after the settle (1 named")), "named or not, new water fails: {:?}", s.fails(false));
        // (re-saved settled: the flowing cell in the file)
        let saved: Vec<(BlockPos, Block)> = s.settled.iter().filter(|(_, b)| !b.is_air()).copied().collect();
        assert!(saved.iter().any(|(p, b)| *p == BlockPos::new(1, 0, 0) && !matches!(redsim::fluid::fluid_state_of(b).fluid, redsim::fluid::Fluid::Empty)));
        let s = check(&saved, &decl);
        assert!(s.water_undeclared.is_empty() && s.water_declared.is_empty() && s.currents.len() == 1 && s.currents_unbounded.is_empty() && s.fails(false).is_empty(), "{:?} {:?} {:?}", s.currents, s.currents_unbounded, s.fails(false));
        let open: Vec<(BlockPos, Block)> = saved.iter().filter(|(p, _)| *p != BlockPos::new(2, 0, 0)).copied().collect();
        let s = check(&open, &decl);
        assert!(s.currents_unbounded.iter().any(|(p, q, _)| *p == BlockPos::new(1, 0, 0) && *q == BlockPos::new(2, 0, 0)), "{:?}", s.currents_unbounded);
        assert!(s.fails(false).iter().any(|f| f.contains("designed currents unbounded")), "{:?}", s.fails(false));
        let missing = declared("# water: 3,0,0=nothing there\n").unwrap();
        assert_eq!(check(&saved, &missing).currents_missing, vec![BlockPos::new(3, 0, 0)]);
        assert!(declared("# water: 1,0,0").is_err() && declared("# water: 1,0,0=").is_err());
        assert_eq!(declared("# water: \u{2212}5,23,7=a; 6,\u{2212}3,4=b\n").unwrap().water.iter().map(|(p, _)| *p).collect::<Vec<_>>(), vec![BlockPos::new(-5, 23, 7), BlockPos::new(6, -3, 4)]);
    }

    /// `--write` keeps the file's header verbatim (farmopt 01:08's rule for `mobfarm edit`, here too): a
    /// file with three header lines and a repeater stored off with its input on is written settled, its
    /// header lines equal to the original's, the repeater on in the written copy.
    #[test]
    fn written_header_kept() {
        let text = "# stand-in: 1,0,0\n# contacts: 2,0,0=a stop\n# clock: 0,0,0 = x\nname: t\npalette:\n  S minecraft:stone\n  R minecraft:repeater[delay=1,facing=west,locked=false,powered=false]\n  B minecraft:redstone_block\nlayer -1:\n  SSS\nlayer 0:\n  BR.\n";
        let d = mctas::design::Design::parse(text).unwrap();
        let blocks: Vec<(BlockPos, Block)> = d.blocks.iter().map(|(p, s)| (BlockPos::new(p[0], p[1], p[2]), b(s))).collect();
        let s = check(&blocks, &Declared::default());
        assert_eq!(s.redstone.len(), 1, "the repeater stored off with its input on: {:?}", s.redstone);
        let written = settled_text(text, &d, &s.settled);
        assert_eq!(header_lines(&written), header_lines(text), "the header kept verbatim");
        assert_eq!(header_lines(text).len(), 3);
        let again = mctas::design::Design::parse(&written).unwrap();
        assert!(again.blocks.values().any(|v| v.contains("repeater") && v.contains("powered=true")), "{:?}", again.blocks);
    }

    /// SURVIVAL (ironspec 01:07 (d)): a stand-in lever on glass, unchanged by the settle, passes; a stand-in
    /// repeater whose input is on (the settle powers it) fails unless a `# held:` line names its real driver
    /// and controller phase; a stand-in torch hung on air can't survive and fails; the bench-facing helper
    /// gives the same verdicts on a laid world.
    #[test]
    fn stand_in_survival_rule() {
        let mut f = floor(4);
        f.extend([
            at(0, 0, 0, "minecraft:lever[face=floor,facing=east,powered=false]"),
            at(2, 0, 0, "minecraft:redstone_block"),
            at(3, 0, 0, "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]"),
        ]);
        let decl = declared("# stand-in: 0,0,0 (the pocket drive's lever); 3,0,0 (day-1's repeater)\n").unwrap();
        assert_eq!(decl.standins.len(), 2);
        let s = check(&f, &decl);
        assert_eq!(s.standins_ok, 1, "the lever survives, unchanged");
        assert_eq!(s.standins_failing.iter().map(|(p, _)| *p).collect::<Vec<_>>(), vec![BlockPos::new(3, 0, 0)], "{:?}", s.standins_failing);
        assert!(s.standins_failing[0].1.contains("the settle changes it"), "{:?}", s.standins_failing);
        let held = declared("# stand-in: 0,0,0 (lever); 3,0,0 (repeater)\n# held: 3,0,0=the controller's block (x+2,17,1), on from REL + 3\n").unwrap();
        let s = check(&f, &held);
        assert!(s.standins_failing.is_empty() && s.standins_ok == 2, "{:?}", s.standins_failing);
        // (a wall torch facing east hung on the air west of it can't survive)
        let mut t = floor(2);
        t.push(at(1, 0, 0, "minecraft:redstone_wall_torch[facing=east,lit=true]"));
        let laid = {
            let mut w = World::new(BlockPos::new(-4, -3, -4), [10, 6, 10]);
            w.place_raw(&t);
            w
        };
        let v = stand_in_survival(&laid, &[BlockPos::new(1, 0, 0)], &[], CAP);
        assert!(matches!(&v[0].1, Err(e) if e.contains("can't survive")), "{v:?}");
        assert!(declared("# held: 3,0,0").is_err() && declared("# held: 3,0,0=").is_err());
    }

    /// No vacuous pass (ironspec 23:56): an empty file is NOT JUDGED and fails.
    #[test]
    fn empty_is_not_judged() {
        let s = check(&[], &Declared::default());
        assert_eq!(s.fails(false), vec!["NOT JUDGED (0 samples: no cell compared)".to_string()]);
        assert!(declared("# clock: 1,2 = x").is_err() && declared("# clock: 1,2,3 =").is_err());
        assert_eq!(declared("# day-time: 13000\n").unwrap().day_time, Some(13000));
    }
}
