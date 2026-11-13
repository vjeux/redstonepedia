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

/// The ticks a fixed point needs at least: one daylight detector cycle (its 20-gt check), so a detector
/// stored at the wrong power shows.
pub const MIN_TICKS: u32 = 21;

/// Split on `sep` outside parentheses (a declaration's note can hold the separator).
fn split_outside(s: &str, sep: char) -> Vec<String> {
    let (mut out, mut cur, mut depth) = (Vec::new(), String::new(), 0i32);
    for ch in s.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if ch == sep && depth == 0 {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
    }
    out.push(cur);
    out.into_iter().map(|e| e.trim().to_string()).filter(|e| !e.is_empty()).collect()
}

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
    /// bench holds (`# held: x,y,z STATE (driver: …; phase …)` or `x,y,z=DRIVER, PHASE`, ironspec 01:07
    /// (d)), each with its note.
    pub standins: Vec<BlockPos>,
    pub held: Vec<(BlockPos, String)>,
    /// The declared stored shapes (`# shape: x,y,z=STORED (reason; evidence; build step)`, ironspec 01:49
    /// (1)): (cell, the stored state as declared, the note).
    pub shapes: Vec<(BlockPos, String, String)>,
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
    crate::declkeys::check(text)?;
    for line in text.lines() {
        let l = line.trim();
        // (G1 on 9625147af: a declaration a reader would skip is a hole, so a near-miss of a declaration key
        // (`#water:`, `# Water :`, …) is an error, never a silent comment: the shared check over every
        // tool's keys, `declkeys::check`, run once before this loop)
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
            for entry in split_outside(rest, ';') {
                let (c, why) = entry.split_once('=').ok_or_else(|| format!("{l:?}: {entry}: give `x,y,z=REASON`"))?;
                if why.trim().is_empty() {
                    return Err(format!("{l:?}: {entry}: a water cell needs its REASON"));
                }
                d.water.push((cell(l, c)?, why.trim().to_string()));
            }
        } else if let Some(rest) = l.strip_prefix("# held:") {
            for entry in split_outside(rest, ';') {
                let cut = entry.find(|c: char| c == '=' || c.is_whitespace()).ok_or_else(|| format!("{l:?}: {entry}: give `x,y,z STATE (driver: …; phase …)`"))?;
                let (c, why) = (&entry[..cut], entry[cut..].trim_start_matches('=').trim());
                if why.is_empty() {
                    return Err(format!("{l:?}: {entry}: a held stand-in names its real driver and controller phase"));
                }
                d.held.push((cell(l, c)?, why.to_string()));
            }
        } else if let Some(rest) = l.strip_prefix("# shape:") {
            for entry in split_outside(rest, ';') {
                let (c, stored) = entry.split_once('=').ok_or_else(|| format!("{l:?}: {entry}: give `x,y,z=STORED (reason; evidence; build step)`"))?;
                let (state, note) = match stored.find(" (") {
                    Some(i) => (stored[..i].trim(), stored[i..].trim().trim_start_matches('(').trim_end_matches(')').trim()),
                    None => (stored.trim(), ""),
                };
                if state.is_empty() || note.is_empty() {
                    return Err(format!("{l:?}: {entry}: a declared shape gives its stored state and its note (reason; evidence; build step)"));
                }
                d.shapes.push((cell(l, c)?, state.to_string(), note.to_string()));
            }
        } else if let Some(rest) = l.strip_prefix("# day-time:") {
            // (`N` or `N (reason)`: record7f's form names why the save's day time is what it is)
            let rest = rest.trim();
            let (n, why) = match rest.find(" (") {
                Some(i) => (rest[..i].trim(), rest[i..].trim()),
                None => (rest, ""),
            };
            if !why.is_empty() && !(why.starts_with('(') && why.ends_with(')')) {
                return Err(format!("{l:?}: give `# day-time: N` or `# day-time: N (reason)`"));
            }
            d.day_time = Some(n.parse::<i64>().map_err(|e| format!("{l:?}: {n}: {e}"))?);
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

/// One differing cell: its position, the stored state, the fixed point's (or the computed shape's), and
/// its class (redstone, shape, fluid, popped, fell, block).
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub pos: BlockPos,
    pub stored: String,
    pub settled: String,
    pub class: &'static str,
}

/// A difference's class label (G1's CHANGES 2 (1): every difference fails, labelled).
fn label(st: &Block, now: &Block) -> &'static str {
    let fluid = |b: &Block| !matches!(redsim::fluid::fluid_state_of(b).fluid, redsim::fluid::Fluid::Empty) || matches!(b, Block::BubbleColumn { .. });
    if class_of(st) == Class::Redstone || class_of(now) == Class::Redstone {
        "redstone"
    } else if fluid(st) || fluid(now) {
        "fluid"
    } else if !st.is_air() && now.is_air() {
        "popped"
    } else if st.is_air() && !now.is_air() {
        "fell or moved in"
    } else if class_of(st) == Class::Shape {
        "shape"
    } else {
        "block"
    }
}

/// The check's result.
#[derive(Clone, Debug, Default)]
pub struct Settle {
    /// The ticks run: to the fixed point, or the cap.
    pub ticks: u32,
    /// A fixed point reached (at least [`MIN_TICKS`] run, then a tick with no change and nothing pending).
    pub fixed: bool,
    /// With no fixed point at the cap: what `World::idle` still saw pending there (scheduled ticks, fluid
    /// ticks, block events, moving pistons, falling blocks), each with its count where it has one.
    pub pending_at_cap: Vec<String>,
    /// With no fixed point at the cap: the pending work no declared clock accounts for (ironspec 06:21 (a)):
    /// each falling block in flight, each block event at a cell no `# clock:` names, each moving block whose
    /// piston (back along its motion, within 13) is no declared clock cell. Failing even with clocks declared.
    pub pending_away: Vec<String>,
    /// With no fixed point at the cap (or after the observers' kick): the cells that changed in the run's
    /// second half or hold a pending tick at the cap, declared `# clock:` (with each one's period: the
    /// interval its state recurs at) and not (failing: "no fixed point within the cap").
    pub clock_declared: Vec<BlockPos>,
    pub clock_periods: Vec<(BlockPos, Option<u32>)>,
    pub clock_undeclared: Vec<BlockPos>,
    /// `# clock:` cells never seen recurring (ironspec 02:41 (i)): false declarations, failing; their
    /// differences judged as any cell's.
    pub clock_false: Vec<BlockPos>,
    /// Redstone cells whose fixed point differs from the stored state (the declared clocks' and held
    /// cells excluded): failing.
    pub redstone: Vec<Cell>,
    /// Shape-bearing cells whose computed shape differs from the stored state (a declared stable `# shape:`
    /// excepted): failing.
    pub shapes: Vec<Cell>,
    /// Every other cell whose fixed point differs from the stored state (fluids changing level or
    /// receding, bubble columns, popped or fallen blocks, plants, …): failing, labelled by class (G1's
    /// CHANGES 2 (1), ironspec 01:49 (2a)).
    pub other: Vec<Cell>,
    /// Block entities whose contents the settle changed (a hopper's item moving on, …): failing.
    pub block_entities: Vec<(BlockPos, String, String)>,
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
    /// Stored currents (flowing or falling water cells) no `# water:` names (ironspec 06:21 (b)): failing.
    pub currents_undeclared: Vec<BlockPos>,
    /// Walls and stairs: no shape update in redsim, unchecked.
    pub unchecked: Vec<Cell>,
    /// Time-dependent blocks (daylight detectors) in a file with no `# day-time:` (ironspec 02:37 (4)):
    /// failing.
    pub time_undeclared: Vec<BlockPos>,
    /// Cells that can't survive where they stand at the fixed point (an attachable on air): failing.
    pub unsupported: Vec<Cell>,
    /// Fluid that falls out of the compared box (the file's cells + 2): the design leaking there, failing,
    /// each `(x,y,z): state` as the world refused it.
    pub fluid_escapes: Vec<String>,
    /// Doors and trapdoors stored against their neighbour signal at the fixed point (ironspec 02:48's
    /// follow-up): any one's POWERED, and an iron one's OPEN, must equal the signal (an iron door can't be
    /// built open unpowered, and the first neighbour update moves either one): failing.
    pub update_states: Vec<Cell>,
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
    /// The held cells (`# held:`), out of the compare like the clocks: (cell, stored, at the fixed point,
    /// the declared driver and phase): both phase states printed.
    pub held_states: Vec<(BlockPos, String, String, String)>,
    /// The `# shape:` declarations (ironspec 01:46 / 01:49): stable in operation, so the stored shape
    /// stands (with the reason); and failing (with why: not a rail, not as stored, re-shaping, unsupported).
    pub shapes_declared: Vec<(BlockPos, String)>,
    pub shapes_declared_failing: Vec<(BlockPos, String)>,
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
        // (no fixed point at the cap with no cell to blame (G1 on 9625147af): pending work not tied to a cell,
        // a falling block in flight or a block event, still fails)
        if !self.fixed && self.ticks > 0 && self.clock_declared.is_empty() && self.clock_undeclared.is_empty() {
            v.push(format!("no fixed point within the cap: pending work not tied to a cell ({})", if self.pending_at_cap.is_empty() { "nothing idle() names".to_string() } else { self.pending_at_cap.join(", ") }));
        } else if !self.fixed && !self.pending_away.is_empty() {
            // (ironspec 06:21 (a): a declared clock no longer masks pending work away from it)
            v.push(format!("no fixed point within the cap: {} pending works away from every declared clock cell ({})", self.pending_away.len(), self.pending_away.iter().take(6).cloned().collect::<Vec<_>>().join("; ")));
        }
        if !self.clock_undeclared.is_empty() {
            v.push(format!(
                "no fixed point within the cap: {} undeclared cells changing in the run's second half or holding a pending tick at the cap ({})",
                self.clock_undeclared.len(),
                cells(&self.clock_undeclared)
            ));
        }
        if !self.clock_false.is_empty() {
            v.push(format!(
                "{} `# clock:` cells never seen recurring (false declarations: a `# clock:` excuses only a cell that changes, ironspec 02:41 (i)): {}",
                self.clock_false.len(),
                cells(&self.clock_false)
            ));
        }
        if !self.other.is_empty() {
            let mut by: Vec<(&str, usize)> = Vec::new();
            for c in &self.other {
                match by.iter_mut().find(|b| b.0 == c.class) {
                    Some(b) => b.1 += 1,
                    None => by.push((c.class, 1)),
                }
            }
            v.push(format!("{} other cells stored unsettled ({})", self.other.len(), by.iter().map(|(c, n)| format!("{c} {n}")).collect::<Vec<_>>().join(", ")));
        }
        if !self.block_entities.is_empty() {
            v.push(format!("{} block entities the settle changes ({})", self.block_entities.len(), cells(&self.block_entities.iter().map(|b| b.0).collect::<Vec<_>>())));
        }
        if !self.shapes_declared_failing.is_empty() {
            v.push(format!(
                "{} `# shape:` declarations failing ({})",
                self.shapes_declared_failing.len(),
                self.shapes_declared_failing.iter().take(6).map(|(p, why)| format!("({},{},{}) {why}", p.x, p.y, p.z)).collect::<Vec<_>>().join("; ")
            ));
        }
        if !self.water_undeclared.is_empty() || !self.water_declared.is_empty() {
            v.push(format!(
                "{} new fluid cells after the settle ({} named in `# water:`): a saved-settled file holds its water, so re-save it with --write (ironspec 01:07 (b))",
                self.water_undeclared.len() + self.water_declared.len(),
                self.water_declared.len()
            ));
        }
        if !self.currents_undeclared.is_empty() {
            v.push(format!(
                "{} stored currents (flowing or falling water) no `# water:` names ({}): a design's current is declared, so its boundedness is checked (ironspec 06:21 (b))",
                self.currents_undeclared.len(),
                cells(&self.currents_undeclared)
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
        if !self.time_undeclared.is_empty() {
            v.push(format!(
                "{} time-dependent blocks (daylight detectors) in a file with no `# day-time:` ({}): it declares its save's day time (ironspec 02:37 (4))",
                self.time_undeclared.len(),
                cells(&self.time_undeclared)
            ));
        }
        if !self.fluid_escapes.is_empty() {
            v.push(format!("{} fluid cells fall out of the compared box (the design leaks: {})", self.fluid_escapes.len(), self.fluid_escapes.iter().take(6).cloned().collect::<Vec<_>>().join("; ")));
        }
        if !self.update_states.is_empty() {
            v.push(format!(
                "{} doors / trapdoors stored against their neighbour signal (an iron one's OPEN, or any one's POWERED, isn't the signal at the fixed point: unmakeable, or moved by the first update): {}",
                self.update_states.len(),
                cells(&self.update_states.iter().map(|c| c.pos).collect::<Vec<_>>())
            ));
        }
        if !self.unsupported.is_empty() {
            v.push(format!("{} cells can't survive where they stand (they pop at their first update: {})", self.unsupported.len(), cells(&self.unsupported.iter().map(|c| c.pos).collect::<Vec<_>>())));
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
            Block::Rail { activator, powered, waterlogged, .. } => Block::Rail { activator, shape, powered, waterlogged },
            Block::PlainRail { waterlogged, .. } => Block::PlainRail { shape, waterlogged },
            Block::DetectorRail { powered, waterlogged, .. } => Block::DetectorRail { shape, powered, waterlogged },
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
    // (the mob layer's clock on at day offset 0, so the daylight detectors and the sky darkening read the
    // file's day time, not the frozen DAY_TIME of the judges' sandboxes; its entities stay held below. The
    // seed sets only the mob layer's random and day offset: natural spawning (`mob.natural`), random ticks
    // (`random_ticks`) and the weather (`weather.on`) stay off, asserted here (G1 on 9625147af))
    w.mob.seed(0, 0);
    assert!(w.mob.natural.is_none() && w.random_ticks.is_none() && !w.weather.on, "settle-check's world: the day clock alone");
    w.place_raw(blocks);
    redsim::redstone::container::apply_block_entities(&mut w, block_entities.iter().map(|(p, s)| (*p, s.as_str())))?;
    w.place_updates(blocks);
    // (the entities held: present for the blocks that sense them, never ticked: an empty entity-ticking
    // range)
    w.entity_chunks = Some([1, 1, 0, 0]);
    for (kind, pos, nbt) in entities {
        redsim::entity::spawn(&mut w, kind, *pos, nbt.as_deref())?;
    }
    // (the entities laid, their passengers from NBT included: the count the settle must keep)
    let laid_entities = w.entities.list.len();
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
        // (a bubble column's own tick too, as its shape update from the block below gives it at the build:
        // `BubbleColumnBlock.tick` sets its drag from what is under it, so a column stored with the wrong
        // drag shows)
        if matches!(b, Block::BubbleColumn { .. }) {
            w.schedule_tick(*p, redsim::block::Kind::BubbleColumn, 5, redsim::ticks::Priority::Normal);
        }
        // (a falling block's own tick too, as `FallingBlock.onPlace` gives it at the build: one stored with
        // nothing under it falls, which the file can't hold)
        if matches!(b, Block::Falling { .. }) {
            w.schedule_tick(*p, b.kind(), 2, redsim::ticks::Priority::Normal);
        }
    }
    // (the laid world, kept for the stand-ins' SURVIVAL and the block entities against the fixed point)
    let laid = w.clone();
    let stored: std::collections::HashMap<BlockPos, Block> = blocks.iter().cloned().collect();
    let all: Vec<BlockPos> = (lo.x..=hi.x).flat_map(|x| (lo.y..=hi.y).flat_map(move |y| (lo.z..=hi.z).map(move |z| BlockPos::new(x, y, z)))).collect();
    let snap = |w: &World| -> Vec<Block> { all.iter().map(|p| w.get(*p)).collect() };
    // (each cell's change ticks; a fixed point: at least MIN_TICKS run, then a tick with no change and
    // nothing pending: `World::idle`, no scheduled or fluid tick, no block event, no moving piston, no
    // falling block; a hopper's cooldown and a detector's next 20-gt check are not pending, the first
    // caught by the block-entity compare, the second by MIN_TICKS. A hopper's items moving change no block
    // state, so this loop can call a world fixed while hoppers still move: the block-entity compare below
    // then fails it, which is why that compare stays (G1 on 6cee60380))
    let run = |w: &mut World| -> (u32, bool, Vec<Vec<u32>>) {
        let mut prev = snap(w);
        let mut history: Vec<Vec<u32>> = vec![Vec::new(); all.len()];
        for t in 1..=cap {
            w.tick();
            let now = snap(w);
            let mut any = false;
            for i in 0..all.len() {
                if now[i] != prev[i] {
                    any = true;
                    history[i].push(t);
                }
            }
            prev = now;
            if !any && w.idle() && t >= MIN_TICKS {
                return (t, true, history);
            }
        }
        (cap, false, history)
    };
    let (ticks, fixed, history) = run(&mut w);
    // (the same three checked directly after the settle (G1 on the follow-up): no entity spawned (none
    // natural, none custom), no random tick could run (no random-tick settings, no chunk order), the weather
    // still off; so a later change to `seed` can't slip past)
    assert!(
        w.entities.list.len() == laid_entities && w.mob.natural.is_none() && w.mob.custom.is_none() && w.random_ticks.is_none() && w.chunk_order.is_empty() && !w.weather.on,
        "settle-check's world after the settle: the day clock alone ({} entities for the {} laid)",
        w.entities.list.len(),
        laid_entities
    );
    let declared_cells: std::collections::HashSet<BlockPos> = decl.clocks.iter().map(|(p, _)| *p).collect();
    let held_cells: std::collections::HashSet<BlockPos> = decl.held.iter().map(|(p, _)| *p).collect();
    // (a fluid falling out of the compared box is the design leaking, its own failure, not a model note
    // (G1 on 9625147af): the world's "setBlock outside the box" notes for water or lava split off)
    let (escapes, notes): (Vec<String>, Vec<String>) = w.errors.iter().cloned().partition(|e| e.starts_with("setBlock outside the box at ") && (e.contains("minecraft:water") || e.contains("minecraft:lava")));
    let mut s = Settle { ticks, fixed, model_notes: notes, fluid_cells, ..Default::default() };
    s.fluid_escapes = escapes.iter().map(|e| e.trim_start_matches("setBlock outside the box at ").to_string()).collect();
    if !fixed {
        // (what `World::idle` still sees pending at the cap)
        let mut p = Vec::new();
        if w.ticks.count() > 0 {
            p.push(format!("scheduled ticks {}", w.ticks.count()));
        }
        if w.fluid_ticks.count() > 0 {
            p.push(format!("fluid ticks {}", w.fluid_ticks.count()));
        }
        if !w.events.is_empty() {
            p.push(format!("block events {}", w.events.len()));
        }
        if !redsim::piston::is_idle(&w) {
            p.push("moving pistons".to_string());
        }
        if !redsim::gravity::is_idle(&w) {
            p.push("falling blocks".to_string());
        }
        s.pending_at_cap = p;
        // (the pending work no declared clock accounts for, ironspec 06:21 (a))
        let mut away = Vec::new();
        for e in w.gravity.entities.iter().filter(|e| !e.removed) {
            away.push(format!("a falling {} at ({:.2},{:.2},{:.2})", e.block, e.x, e.y, e.z));
        }
        for e in w.events.iter() {
            if !declared_cells.contains(&e.pos) {
                away.push(format!("a block event at ({},{},{})", e.pos.x, e.pos.y, e.pos.z));
            }
        }
        for (p, be) in w.piston.all() {
            let back = be.direction.opposite();
            let from_clock = (0..=13).any(|k| {
                let mut q = p;
                for _ in 0..k {
                    q = q.rel(back);
                }
                declared_cells.contains(&q)
            });
            if !from_clock {
                away.push(format!("a moving block at ({},{},{})", p.x, p.y, p.z));
            }
        }
        s.pending_away = away;
    }
    // (ironspec 02:37 (4): a block whose state follows the day time (a daylight detector) is saved at its
    // file's day time, which the file declares (`# day-time:`); with none it fails)
    if decl.day_time.is_none() {
        s.time_undeclared = blocks.iter().filter(|(_, b)| matches!(b, Block::DaylightDetector { .. })).map(|(p, _)| *p).collect();
    }
    // (no fixed point at the cap (G1's CHANGES 2 (2), ironspec 01:49 (2b)): every cell that changed in the
    // run's second half or holds a pending tick at the cap must be a declared clock, its period printed (the
    // interval its state recurs at); any other fails "no fixed point within the cap")
    let clocks_of = |w: &World, history: &[Vec<u32>], s: &mut Settle| {
        let pending: std::collections::HashSet<BlockPos> = w.ticks.all().into_iter().map(|t| t.pos).chain(w.fluid_ticks.all().into_iter().map(|t| t.pos)).collect();
        for (i, p) in all.iter().enumerate() {
            let late: Vec<u32> = history[i].iter().copied().filter(|t| *t > cap / 2).collect();
            if late.is_empty() && !pending.contains(p) {
                continue;
            }
            if declared_cells.contains(p) {
                // (a declared cell SEEN RECURRING (ironspec 02:41 (i)): 2 changes or more in the second half,
                // or a pending tick at the cap; one late change alone recurs not, and the cell stays a false
                // declaration below)
                if late.len() < 2 && !pending.contains(p) {
                    continue;
                }
                // (the period: the commonest interval between a change and the one two after it)
                let mut gaps: Vec<u32> = late.windows(3).map(|v| v[2] - v[0]).collect();
                gaps.sort();
                let period = gaps.chunk_by(|a, b| a == b).max_by_key(|g| g.len()).map(|g| g[0]);
                if !s.clock_declared.contains(p) {
                    s.clock_declared.push(*p);
                    s.clock_periods.push((*p, period));
                }
            } else if !s.clock_undeclared.contains(p) {
                s.clock_undeclared.push(*p);
            }
        }
    };
    if !fixed {
        clocks_of(&w, &history, &mut s);
    } else {
        // (the observers' kick: a build places each observer's target before it or after it, and the
        // judges' placement runs no observer's shape update; one pulse per unpowered observer, on a copy
        // of the fixed point, finds an observer clock that never starts from the placement (ironspec
        // 23:21: an undeclared observer clock fails); a kick that settles elsewhere is printed: the state
        // depends on the build order there)
        let observers: Vec<BlockPos> = all.iter().copied().filter(|p| matches!(w.get(*p), Block::Observer { powered: false, .. })).collect();
        s.observers_kicked = observers.len();
        if !observers.is_empty() {
            let mut k = w.clone();
            for p in &observers {
                let kind = k.get(*p).kind();
                k.schedule_tick(*p, kind, 2, redsim::ticks::Priority::Normal);
            }
            let (_, settled_again, kh) = run(&mut k);
            if !settled_again {
                clocks_of(&k, &kh, &mut s);
            } else {
                for p in &all {
                    if k.get(*p) != w.get(*p) {
                        s.kick_sensitive.push(Cell { pos: *p, stored: w.get(*p).to_string(), settled: k.get(*p).to_string(), class: label(&w.get(*p), &k.get(*p)) });
                    }
                }
            }
        }
    }
    // (a `# clock:` excuses only a cell seen recurring, in the main run or the kicked one (ironspec 02:41
    // (i)); a declared cell never seen recurring is a FALSE DECLARATION, failing, and its stored-vs-fixed-point
    // difference is judged as any cell's)
    let recurring: std::collections::HashSet<BlockPos> = s.clock_declared.iter().copied().collect();
    s.clock_false = decl.clocks.iter().map(|(p, _)| *p).filter(|p| !recurring.contains(p)).collect();
    // (the block entities: their contents laid against the fixed point's; a change fails, outside the
    // recurring clocks and held cells)
    let slots = |w: &World| -> std::collections::BTreeMap<(i32, i32, i32), String> { w.containers.map.iter().map(|(p, c)| ((p.x, p.y, p.z), format!("{:?}", c.slots))).collect() };
    let (before, after) = (slots(&laid), slots(&w));
    for (k, b) in &before {
        let p = BlockPos::new(k.0, k.1, k.2);
        let a = after.get(k).cloned().unwrap_or_else(|| "gone".into());
        if &a != b && !recurring.contains(&p) && !held_cells.contains(&p) {
            s.block_entities.push((p, b.clone(), a));
        }
    }
    // (the declared shapes, ironspec 01:46 / 01:49: a rail stored as declared and stable in operation, its
    // shape then standing)
    let mut shape_ok: std::collections::HashSet<BlockPos> = std::collections::HashSet::new();
    for (p, state, note) in &decl.shapes {
        let st = stored.get(p).copied();
        let verdict = (|| -> Result<(), String> {
            let st = st.ok_or("no block in the file there")?;
            let want = block_of(state).map_err(|e| format!("its declared state {state}: {e}"))?;
            if want != st {
                return Err(format!("stored {st}, declared {state}"));
            }
            if !redsim::rails::is_rail(&st) {
                return Err(format!("{st} re-shapes on any neighbour update: only rails can be declared (ironspec 01:49 (1))"));
            }
            if redsim::rails::would_pop(&laid, *p) {
                return Err("its support is missing: it pops".into());
            }
            if matches!(st, Block::PlainRail { .. }) && redsim::rails::RailState::new(*p, st).count_potential_connections(&laid) >= 3 {
                // (a signal source's toggle updates cells up to Manhattan 2 away (a torch's or lever's its
                // neighbours' neighbours, the lever through its support; a repeater's or observer's its front
                // cell and that cell's neighbours; a detector rail's own and its support's neighbours), so the
                // junction must have none that can change within 2 (ironspec 02:41 (ii); a redstone block is
                // constant)
                let near = (-2i32..=2).flat_map(|dx| (-2i32..=2).flat_map(move |dy| (-2i32..=2).map(move |dz| (dx, dy, dz)))).filter(|(dx, dy, dz)| (1..=2).contains(&(dx.abs() + dy.abs() + dz.abs())));
                let changing_source = near.map(|(dx, dy, dz)| BlockPos::new(p.x + dx, p.y + dy, p.z + dz)).find(|q| {
                    let b = laid.get(*q);
                    redsim::behavior::is_signal_source(&laid, &b) && !matches!(b, Block::RedstoneBlock)
                });
                if let Some(q) = changing_source {
                    return Err(format!("a junction within 2 of a signal source that can change ({},{},{} {}): it re-shapes when that source toggles", q.x, q.y, q.z, laid.get(q)));
                }
            }
            if w.get(*p) != st {
                return Err(format!("the settle changes it to {}", w.get(*p)));
            }
            Ok(())
        })();
        match verdict {
            Ok(()) => {
                shape_ok.insert(*p);
                s.shapes_declared.push((*p, note.clone()));
            }
            Err(why) => s.shapes_declared_failing.push((*p, why)),
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
        if recurring.contains(p) {
            out = st;
        } else if let Some((_, note)) = decl.held.iter().find(|(q, _)| q == p) {
            // (a held cell, out of the compare like a clock: both phase states printed)
            s.held_states.push((*p, st.to_string(), now.to_string(), note.clone()));
            out = st;
        } else if st != now {
            let cell = Cell { pos: *p, stored: st.to_string(), settled: now.to_string(), class: label(&st, &now) };
            // (a fluid cell laid by no build: named in a `# water:` line or not, it fails (ironspec 01:07 (b)))
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
        // (the shape rule: the fixed point's shape-bearing cells against their own shape update; a declared
        // stable `# shape:` stands)
        let unheld = !recurring.contains(p) && !held_cells.contains(p);
        if let Some((ok, want)) = shape_rule(&w, &mut scratch, *p) {
            s.shape_cells += 1;
            if !ok && unheld && !shape_ok.contains(p) && !s.redstone.iter().any(|x| x.pos == *p) {
                s.shapes.push(Cell { pos: *p, stored: st.to_string(), settled: want.to_string(), class: "shape" });
            }
            if unheld && !shape_ok.contains(p) {
                out = want;
            }
        } else if class == Class::Shape && !st.is_air() {
            s.unchecked.push(Cell { pos: *p, stored: st.to_string(), settled: now.to_string(), class: "shape" });
        }
        // (a cell that can't survive where it stands pops at its first update, which the judges'
        // placement may never give it (an attachable on air): it fails as popped, as the farm loader's
        // survival check refuses it)
        if !now.is_air() && unheld && (!redsim::behavior::can_survive(&w, &now, *p) || redsim::rails::would_pop(&w, *p)) {
            s.unsupported.push(Cell { pos: *p, stored: st.to_string(), settled: "minecraft:air (no support: it pops at its first update)".into(), class: "popped" });
        }
        // (a door or trapdoor stored against its neighbour signal (ironspec 02:48): its POWERED follows the
        // signal at the first neighbour update (a door reads both halves), and an iron one's OPEN with it (no
        // hand can open it): the STORED state against the fixed point's signal, so a door the placement never
        // updates is judged too (one it updated already differs, judged above)
        let gate = match st {
            Block::Door { iron, upper, open, powered, .. } => {
                let other = if upper { p.below() } else { p.above() };
                Some((iron, open, powered, w.has_neighbor_signal(*p) || w.has_neighbor_signal(other), "door"))
            }
            Block::Trapdoor { iron, open, powered, .. } => Some((iron, open, powered, w.has_neighbor_signal(*p), "trapdoor")),
            // (redsim's Door / Trapdoor are the iron and oak sets, `iron` the iron set only (block.rs's parse);
            // any other set, copper or another wood, is no block redsim models, so settle-check refuses its file
            // rather than leaving the rule unchecked (ironspec 05:13 real (1)))
            _ => None,
        };
        if let Some((iron, open, powered, signal, class)) = gate
            && unheld
            && (powered != signal || (iron && open != signal))
        {
            s.update_states.push(Cell { pos: *p, stored: st.to_string(), settled: format!("the neighbour signal {}", if signal { "on" } else { "off" }), class });
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
    // (and every stored current, a flowing or falling water cell, is declared (ironspec 06:21 (b)): a
    // mistyped key can't skip a current's boundedness check)
    let declared_water: std::collections::HashSet<BlockPos> = decl.water.iter().map(|(p, _)| *p).collect();
    let mut undeclared_currents: Vec<BlockPos> = stored.iter().filter(|(p, b)| matches!(b, Block::Water { level } if *level >= 1) && !declared_water.contains(*p)).map(|(p, _)| *p).collect();
    undeclared_currents.sort();
    s.currents_undeclared = undeclared_currents;
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
        assert!(s.fails(false).iter().any(|f| f.starts_with("no fixed point within the cap")), "{:?}", s.fails(false));
        let decl = declared("# clock: 0,0,0; 1,0,0; 1,0,1; 1,0,2; 0,0,2; -1,0,2; -1,0,1; -1,0,0 = the repeater ring's pulse\n").unwrap();
        let s = check(&ring, &decl);
        assert!(s.clock_undeclared.is_empty() && !s.clock_declared.is_empty() && s.fails(false).is_empty(), "{:?} {:?} {:?}", s.clock_undeclared, s.fails(false), s.redstone);
        // (ironspec 06:21 (a): a declared clock no longer masks pending work away from it: the same declared
        // ring with a sand block 60 up, still falling at a cap of 30, fails on it; the ring alone at that cap
        // passes)
        let s = settle_check(&ring, &[], &[], &decl, 6000, 30).unwrap();
        assert!(!s.fixed && s.pending_away.is_empty() && s.fails(false).is_empty(), "{:?} {:?}", s.pending_away, s.fails(false));
        let mut sand = ring.clone();
        sand.push(at(-2, 60, -2, "minecraft:sand"));
        let s = settle_check(&sand, &[], &[], &decl, 6000, 30).unwrap();
        assert!(s.pending_away.iter().any(|p| p.starts_with("a falling minecraft:sand")) && s.fails(false).iter().any(|f| f.contains("pending works away from every declared clock cell")), "{:?} {:?}", s.pending_away, s.fails(false));
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
        assert!(!s.clock_undeclared.is_empty() && s.fails(false).iter().any(|f| f.starts_with("no fixed point within the cap")), "{:?} {:?}", s.clock_undeclared, s.fails(false));
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
    /// between two water sources over a solid block fails: the infinite rule makes it a source
    /// (FlowingFluid.getNewLiquid; the waterlogged rails on main, b8949c96's e8f7048f0 / 993cef260), one new
    /// fluid cell with no model note; the same sources with stone between them pass, the water already at
    /// its fixed point.
    #[test]
    fn fluid_settled() {
        let mut leak = floor(3);
        leak.extend([at(0, 0, 0, "minecraft:water[level=0]"), at(1, 0, 0, "minecraft:rail[shape=north_south,waterlogged=false]"), at(2, 0, 0, "minecraft:water[level=0]")]);
        for (x, z) in [(0, -1), (0, 1), (1, -1), (1, 1), (2, -1), (2, 1), (-1, 0), (3, 0)] {
            leak.push(at(x, 0, z, "minecraft:stone"));
        }
        let s = check(&leak, &Declared::default());
        let f = s.fails(false);
        assert!(s.water_undeclared.iter().any(|c| c.pos == BlockPos::new(1, 0, 0) && c.settled.contains("waterlogged=true")) && s.model_notes.is_empty(), "{:?} {:?}", s.water_undeclared, s.model_notes);
        assert!(f.iter().any(|x| x.contains("new fluid cells after the settle")), "{f:?}");
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
        // (ironspec 06:21 (b): the saved current undeclared fails, and so does it under an unknown key no reader
        // takes (`# currents:`); a key one letter off a known one (`# watr:`) is refused by the shared check)
        let s = check(&saved, &Declared::default());
        assert!(s.currents_undeclared == vec![BlockPos::new(1, 0, 0)] && s.fails(false).iter().any(|f| f.contains("stored currents (flowing or falling water) no `# water:` names")), "{:?} {:?}", s.currents_undeclared, s.fails(false));
        let typo = declared("# currents: 1,0,0=the trough's current\n").unwrap();
        assert_eq!(check(&saved, &typo).currents_undeclared, vec![BlockPos::new(1, 0, 0)]);
        assert!(declared("# watr: 1,0,0=the trough's current\n").is_err());
        // (ironspec 05:40 real: a current is a water cell with level > 0 (falling included); sources,
        // waterlogged blocks and bubble columns aren't: a walled bubble-column lift and a waterlogged rail pass
        // undeclared)
        let mut lift = floor(3);
        lift.extend([at(0, 0, 0, "minecraft:soul_sand"), at(0, 1, 0, "minecraft:bubble_column[drag=false]"), at(0, 2, 0, "minecraft:bubble_column[drag=false]"), at(0, 3, 0, "minecraft:stone")]);
        for y in 0..=3 {
            for (x, z) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                lift.push(at(x, y, z, "minecraft:glass"));
            }
        }
        lift.extend([at(2, 0, 2, "minecraft:rail[shape=north_south,waterlogged=true]"), at(2, 0, 1, "minecraft:stone"), at(2, 0, 3, "minecraft:stone"), at(1, 0, 2, "minecraft:stone"), at(3, 0, 2, "minecraft:stone")]);
        let s = check(&lift, &Declared::default());
        assert!(s.currents_undeclared.is_empty() && !s.fails(false).iter().any(|f| f.contains("stored currents")), "{:?} {:?}", s.currents_undeclared, s.fails(false));
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

    /// G1's CHANGES 2 (1), ironspec 01:49 (2a): every difference fails, labelled by class. Controls: a
    /// receding current (a flowing cell with no source), a bubble column over soul sand stored with the
    /// wrong drag, a torch hung on air (it can't survive: popped), a hopper over a chest holding an item
    /// (the item moves on: a changed block entity).
    #[test]
    fn every_difference_fails() {
        let mut recede = floor(2);
        recede.push(at(0, 0, 0, "minecraft:water[level=1]"));
        let s = check(&recede, &Declared::default());
        assert!(s.other.iter().any(|c| c.pos == BlockPos::new(0, 0, 0) && c.class == "fluid"), "{:?}", s.other);
        assert!(s.fails(false).iter().any(|f| f.contains("other cells stored unsettled (fluid")), "{:?}", s.fails(false));
        let mut column = floor(2);
        column.extend([at(0, -1, 0, "minecraft:soul_sand"), at(0, 0, 0, "minecraft:bubble_column[drag=true]"), at(0, 1, 0, "minecraft:water[level=0]")]);
        for (x, z) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            column.push(at(x, 0, z, "minecraft:glass"));
            column.push(at(x, 1, z, "minecraft:glass"));
        }
        column.push(at(0, 2, 0, "minecraft:glass"));
        let s = check(&column, &Declared::default());
        assert!(s.other.iter().any(|c| c.pos == BlockPos::new(0, 0, 0) && c.settled.contains("drag=false")), "{:?} {:?}", s.other, s.model_notes);
        let mut torch = floor(2);
        torch.push(at(1, 1, 0, "minecraft:redstone_wall_torch[facing=east,lit=true]"));
        let s = check(&torch, &Declared::default());
        assert!(s.unsupported.iter().any(|c| c.pos == BlockPos::new(1, 1, 0)) || s.other.iter().chain(s.redstone.iter()).any(|c| c.pos == BlockPos::new(1, 1, 0) && c.settled.contains("air")), "{:?} {:?}", s.unsupported, s.redstone);
        assert!(!s.fails(false).is_empty());
        let mut hopper = floor(2);
        hopper.extend([at(0, 0, 0, "minecraft:chest[facing=north,type=single,waterlogged=false]"), at(0, 1, 0, "minecraft:hopper[enabled=true,facing=down]")]);
        let item = vec![(BlockPos::new(0, 1, 0), "{Items:[{Slot:0b,id:\"minecraft:iron_ingot\",count:1}]}".to_string())];
        let s = settle_check(&hopper, &item, &[], &Declared::default(), 6000, CAP).unwrap();
        assert!(s.block_entities.iter().any(|(p, _, _)| *p == BlockPos::new(0, 1, 0)), "{:?}", s.block_entities);
        assert!(s.fails(false).iter().any(|f| f.contains("block entities the settle changes")), "{:?}", s.fails(false));
    }

    /// G1's CHANGES 2 (2), ironspec 01:49 (2b): a slow clock with a quiet phase longer than 40 gt (a ring
    /// of eight delay-4 repeaters, one pulse: each cell quiet 56 of its 64 gt) has no fixed point at the cap:
    /// undeclared it fails, declared it passes with its period printed (64 gt).
    #[test]
    fn slow_clock() {
        let mut ring: Vec<(BlockPos, Block)> = (-1..=4).flat_map(|x| (-1..=4).map(move |z| at(x, -1, z, "minecraft:stone"))).collect();
        let dot = "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]";
        let rep = |x: i32, z: i32, facing: &str, on: bool| at(x, 0, z, &format!("minecraft:repeater[delay=4,facing={facing},locked=false,powered={on}]"));
        ring.extend([
            at(0, 0, 0, dot),
            rep(1, 0, "west", true),
            rep(2, 0, "west", false),
            at(3, 0, 0, dot),
            rep(3, 1, "north", false),
            rep(3, 2, "north", false),
            at(3, 0, 3, dot),
            rep(2, 3, "east", false),
            rep(1, 3, "east", false),
            at(0, 0, 3, dot),
            rep(0, 2, "south", false),
            rep(0, 1, "south", false),
        ]);
        let s = check(&ring, &Declared::default());
        assert!(!s.fixed && !s.clock_undeclared.is_empty() && s.fails(false).iter().any(|f| f.starts_with("no fixed point within the cap")), "{} {:?}", s.fixed, s.fails(false));
        let cells = ["0,0,0", "1,0,0", "2,0,0", "3,0,0", "3,0,1", "3,0,2", "3,0,3", "2,0,3", "1,0,3", "0,0,3", "0,0,2", "0,0,1"];
        let decl = declared(&format!("# clock: {} = the slow ring\n", cells.join("; "))).unwrap();
        let s = check(&ring, &decl);
        assert!(s.clock_undeclared.is_empty() && s.fails(false).is_empty(), "{:?} {:?}", s.clock_undeclared, s.fails(false));
        assert!(s.clock_periods.iter().any(|(p, period)| *p == BlockPos::new(2, 0, 0) && *period == Some(64)), "{:?}", s.clock_periods);
    }

    /// `# shape:` (ironspec 01:46 / 01:49): a declared stable rail stands (a ramp stored ascending with no
    /// rail above it, the shape rule's mismatch otherwise); a declared dust fails (only rails); a declared
    /// junction beside a lever fails (it re-shapes when its power changes).
    #[test]
    fn declared_shapes() {
        let mut r = floor(3);
        r.extend([at(0, 0, 0, "minecraft:rail[shape=north_south,waterlogged=false]"), at(0, 0, 1, "minecraft:rail[shape=ascending_south,waterlogged=false]"), at(0, 0, 2, "minecraft:stone")]);
        let s = check(&r, &Declared::default());
        assert!(s.shapes.iter().any(|c| c.pos == BlockPos::new(0, 0, 1)), "the ramp is a mismatch undeclared: {:?}", s.shapes);
        let decl = declared("# shape: 0,0,1=minecraft:rail[shape=ascending_south,waterlogged=false] (the lane's ramp; tick-exact's row; laid against a temporary rail)\n").unwrap();
        let s = check(&r, &decl);
        assert!(s.shapes.is_empty() && s.shapes_declared.len() == 1 && s.fails(false).is_empty(), "{:?} {:?} {:?}", s.shapes, s.shapes_declared_failing, s.fails(false));
        let dust = declared("# shape: 0,0,0=minecraft:rail[shape=east_west,waterlogged=false] (wrong state; x; y)\n").unwrap();
        let s = check(&r, &dust);
        assert!(s.shapes_declared_failing.iter().any(|(_, why)| why.contains("declared")), "{:?}", s.shapes_declared_failing);
        let mut d = floor(3);
        d.push(at(1, 0, 0, "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]"));
        let s = check(&d, &declared("# shape: 1,0,0=minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none] (x; y; z)\n").unwrap());
        assert!(s.shapes_declared_failing.iter().any(|(_, why)| why.contains("only rails")), "{:?}", s.shapes_declared_failing);
        let mut t = floor(3);
        t.extend((-1..=3).map(|x| at(x, -1, 2, "minecraft:stone")));
        t.extend([
            at(0, 0, 1, "minecraft:rail[shape=east_west,waterlogged=false]"),
            at(2, 0, 1, "minecraft:rail[shape=east_west,waterlogged=false]"),
            at(1, 0, 2, "minecraft:rail[shape=north_south,waterlogged=false]"),
            at(1, 0, 1, "minecraft:rail[shape=south_east,waterlogged=false]"),
            at(1, 0, 0, "minecraft:lever[face=floor,facing=east,powered=false]"),
        ]);
        let s = check(&t, &declared("# shape: 1,0,1=minecraft:rail[shape=south_east,waterlogged=false] (x; y; z)\n").unwrap());
        assert!(s.shapes_declared_failing.iter().any(|(p, why)| *p == BlockPos::new(1, 0, 1) && why.contains("signal source")), "{:?}", s.shapes_declared_failing);
        // (ironspec 02:41 (ii)'s control: a lever two cells away, through its support (a wall lever on a stone
        // beside the junction), fails; with no source within 2 the junction stands)
        let mut t2: Vec<(BlockPos, Block)> = t.iter().copied().filter(|(p, _)| *p != BlockPos::new(1, 0, 0)).collect();
        t2.extend([at(1, 0, 0, "minecraft:stone"), at(1, 0, -1, "minecraft:lever[face=wall,facing=north,powered=false]")]);
        let s = check(&t2, &declared("# shape: 1,0,1=minecraft:rail[shape=south_east,waterlogged=false] (x; y; z)\n").unwrap());
        assert!(s.shapes_declared_failing.iter().any(|(p, why)| *p == BlockPos::new(1, 0, 1) && why.contains("within 2")), "{:?}", s.shapes_declared_failing);
        let t3: Vec<(BlockPos, Block)> = t.iter().copied().filter(|(p, _)| *p != BlockPos::new(1, 0, 0)).collect();
        let s = check(&t3, &declared("# shape: 1,0,1=minecraft:rail[shape=south_east,waterlogged=false] (x; y; z)\n").unwrap());
        assert!(s.shapes_declared.iter().any(|(p, _)| *p == BlockPos::new(1, 0, 1)) && s.shapes_declared_failing.is_empty(), "{:?}", s.shapes_declared_failing);
        assert!(declared("# shape: 1,0,1=minecraft:rail[shape=south_east,waterlogged=false]").is_err(), "a declaration gives its note");
        assert_eq!(declared("# held: 2,12,13 powered=true (driver: day-1's repeater 2,12,12; phase REL + 96 .. REL + 113; bench frame)\n").unwrap().held.len(), 1, "dfb89499's held format");
    }

    /// ironspec 02:41 (i): a `# clock:` excuses only a cell seen recurring. Control: a repeater stored
    /// unpowered behind a lever that's on, declared a clock: the run reaches its fixed point, the cell never
    /// recurs, so it fails as a false declaration and its difference is judged (a redstone cell unsettled).
    #[test]
    fn false_clock_declaration() {
        let mut d = floor(3);
        d.extend([
            at(0, 0, 0, "minecraft:lever[face=floor,facing=east,powered=true]"),
            at(1, 0, 0, "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]"),
            at(2, 0, 0, "minecraft:redstone_lamp[lit=false]"),
        ]);
        let s = check(&d, &declared("# clock: 1,0,0 = not a clock\n").unwrap());
        assert!(s.fixed && s.clock_false == vec![BlockPos::new(1, 0, 0)] && s.fails(false).iter().any(|f| f.contains("false declarations")), "{} {:?} {:?}", s.fixed, s.clock_false, s.fails(false));
        assert!(s.redstone.iter().any(|c| c.pos == BlockPos::new(1, 0, 0)), "its difference judged as any cell's: {:?}", s.redstone);
    }

    /// ironspec 02:37 (4): a daylight detector is saved at its file's day time: with no `# day-time:` it
    /// fails; at a declared midnight one stored at 0 is settled; at a declared noon the same file fails (the
    /// detector reads 15 there: settle-check settles at the declared time, the mob layer's clock on).
    #[test]
    fn time_dependent_needs_day_time() {
        let mut d = floor(2);
        d.push(at(0, 0, 0, "minecraft:daylight_detector[inverted=false,power=0]"));
        let s = check(&d, &Declared::default());
        assert!(s.time_undeclared == vec![BlockPos::new(0, 0, 0)] && s.fails(false).iter().any(|f| f.contains("no `# day-time:`")), "{:?} {:?}", s.time_undeclared, s.fails(false));
        let night = declared("# day-time: 18000\n").unwrap();
        let s = settle_check(&d, &[], &[], &night, 18000, CAP).unwrap();
        assert!(s.fails(false).is_empty(), "a night save holds the detector at 0: {:?} {:?}", s.fails(false), s.redstone);
        let noon = declared("# day-time: 6000\n").unwrap();
        let s = settle_check(&d, &[], &[], &noon, 6000, CAP).unwrap();
        assert!(s.redstone.iter().any(|c| c.pos == BlockPos::new(0, 0, 0) && c.settled.contains("power=15")), "at noon it reads 15: {:?}", s.redstone);
    }

    /// G1 on 9625147af: no fixed point at the cap with no cell to blame fails. Control: a sand block 60
    /// above the floor with a cap of 30: still falling at the cap, nothing changing in the run's second half
    /// and no tick pending at a cell, it fails "pending work not tied to a cell (falling blocks)". And G1's
    /// note (a): a near-miss of a declaration key is an error, not a skipped comment.
    #[test]
    fn pending_work_not_tied_to_a_cell() {
        let mut d = floor(1);
        d.push(at(0, 60, 0, "minecraft:sand"));
        let s = settle_check(&d, &[], &[], &Declared::default(), 6000, 30).unwrap();
        assert!(!s.fixed && s.clock_undeclared.is_empty() && s.clock_declared.is_empty(), "{} {:?} {:?}", s.fixed, s.clock_undeclared, s.pending_at_cap);
        assert!(s.fails(false).iter().any(|f| f.starts_with("no fixed point within the cap: pending work not tied to a cell") && f.contains("falling blocks")), "{:?}", s.fails(false));
        for bad in ["#water: 1,2,3=x\n", "# Water: 1,2,3=x\n", "# water : 1,2,3=x\n", "#  clock: 1,2,3 = x\n"] {
            assert!(declared(bad).is_err(), "{bad:?} is refused, not skipped");
        }
        assert_eq!(declared("# water: 1,2,3=the lift's current (a; b)\n").unwrap().water.len(), 1, "a reason's own `;` inside parentheses is kept");
        assert!(declared("# The return (loop frame): a comment\n").is_ok(), "other comments stand");
    }

    /// G1 on the reader fix: entries split outside parentheses and each parses strictly as `x,y,z=…`. record7f's
    /// `# day-time:` with its parenthesised reason reads 18000; its `# water:` line, whose reasons hold `;`
    /// outside any parentheses, is REFUSED (the file wraps those reasons, not the reader guessing); a reason
    /// with an inner `;` outside parentheses and a malformed cell (`3,4`, `3,4,5,6`) both error; a reason's `;`
    /// inside parentheses reads whole; anything after N but a parenthesised reason errors.
    #[test]
    fn reads_record7f_declarations() {
        let d = declared("# day-time: 18000 (the save's day time, a night placement as the build guide places the daylight detector (-5,15,3): its stored power 0 is its reading at this time; ironspec 02:37 PT (4))\n").unwrap();
        assert_eq!(d.day_time, Some(18000));
        let water7f = "# water: -5,23,7=designed current, saved settled (level 1): the shared lift's top-drift current, fed by the source north of the exit cell, carrying each riser south onto the exit booster; bounded by the glass round it, the cap over it and the booster's rail (tick-exact's water-settle row on record7e: one cell, then still; the bench's traces: the riser's z speed 0.027 -> 0.104 in the cell, on the booster 8 gt after the top); 6,-3,4=designed current, saved settled (level 1): the napper climb's top-drift current, fed by the source (5,-3,4), carrying the napper east onto the exit detector (7,-3,4); bounded by the glass round it and the detector's rail (tick-exact's water-settle row on record7e)\n";
        assert!(declared(water7f).is_err(), "record7f's `;` outside parentheses refused");
        for bad in ["# water: 1,2,3=a current; fed by its source\n", "# water: 1,2,3=a current; 3,4,5 is where its spread ends\n", "# water: 3,4=a current\n", "# water: 3,4,5,6=a current\n", "# held: 3,4 minecraft:lever (driver: x; phase y)\n", "# shape: 1,2,3,4=minecraft:rail[shape=north_south] (r; e; b)\n", "# day-time: 18000 the save's\n", "# day-time: 18000 (a) b\n"] {
            assert!(declared(bad).is_err(), "{bad:?} refused");
        }
        let ok = declared("# water: 1,2,3=a current (fed by its source; bounded by glass); 4,5,6=another (b)\n").unwrap();
        assert_eq!(ok.water.iter().map(|(p, _)| *p).collect::<Vec<_>>(), vec![BlockPos::new(1, 2, 3), BlockPos::new(4, 5, 6)]);
        assert_eq!(declared("# day-time: 6000\n").unwrap().day_time, Some(6000));
    }

    /// A fluid falling out of the compared box is its own failure, not a model note (G1 on 9625147af): a
    /// source on the file's lowest layer with air under it falls past the box (the file + 2 below).
    #[test]
    fn fluid_escapes_the_box() {
        let d = vec![at(1, 0, 0, "minecraft:stone"), at(0, 0, 0, "minecraft:water[level=0]"), at(0, 1, 0, "minecraft:stone")];
        let s = check(&d, &Declared::default());
        assert!(!s.fluid_escapes.is_empty() && s.model_notes.is_empty(), "{:?} {:?}", s.fluid_escapes, s.model_notes);
        assert!(s.fails(false).iter().any(|f| f.contains("fall out of the compared box")) && !s.fails(false).iter().any(|f| f.starts_with("NOT MODELLED")), "{:?}", s.fails(false));
    }

    /// Doors and trapdoors against their neighbour signal (ironspec 02:48's follow-up): an iron trapdoor
    /// stored open with no signal fails (unmakeable), and one stored open and powered beside a lever that's on
    /// passes; an oak trapdoor open with no signal passes (a hand opens it), but one stored powered with no
    /// signal fails (the first update moves it). (The settle world's flags, the day clock alone, are asserted
    /// inside settle_check on every run, these included.)
    #[test]
    fn update_driven_states() {
        let mut d = floor(3);
        d.push(at(0, 0, 0, "minecraft:iron_trapdoor[facing=north,half=bottom,open=true,powered=false,waterlogged=false]"));
        let s = check(&d, &Declared::default());
        assert!(s.update_states.iter().any(|c| c.pos == BlockPos::new(0, 0, 0)) && s.fails(false).iter().any(|f| f.contains("against their neighbour signal")), "{:?}", s.update_states);
        let mut on = floor(3);
        on.extend([at(0, 0, 0, "minecraft:iron_trapdoor[facing=north,half=bottom,open=true,powered=true,waterlogged=false]"), at(1, 0, 0, "minecraft:lever[face=floor,facing=east,powered=true]")]);
        let s = check(&on, &Declared::default());
        assert!(s.update_states.is_empty(), "{:?} {:?}", s.update_states, s.fails(false));
        let mut oak = floor(3);
        oak.push(at(0, 0, 0, "minecraft:oak_trapdoor[facing=north,half=bottom,open=true,powered=false,waterlogged=false]"));
        let s = check(&oak, &Declared::default());
        assert!(s.update_states.is_empty(), "{:?}", s.update_states);
        let mut oakp = floor(3);
        oakp.push(at(0, 0, 0, "minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=true,waterlogged=false]"));
        let s = check(&oakp, &Declared::default());
        assert!(s.update_states.iter().any(|c| c.pos == BlockPos::new(0, 0, 0)), "{:?}", s.update_states);
        // (the copper set, like every set but iron and oak, is no block redsim models: settle-check's laying
        // refuses it, ironspec 05:13 real (1))
        for state in ["minecraft:copper_trapdoor[facing=north,half=bottom,open=false,powered=true,waterlogged=false]", "minecraft:copper_door[facing=north,half=lower,hinge=left,open=false,powered=true]"] {
            assert!(block_of(state).is_err(), "{state} refused, not judged");
        }
    }

    /// A file's entity carrying a passenger in its NBT lays as two entities, both held: the settle keeps that
    /// count (the after-settle assertion compares the laid count, not the file's lines).
    #[test]
    fn entities_with_passengers_held() {
        let d = floor(2);
        let ents = vec![("minecraft:minecart".to_string(), [0.5, 0.0, 0.5], Some("{Passengers:[{id:\"minecraft:villager\"}]}".to_string()))];
        let s = settle_check(&d, &[], &ents, &Declared::default(), 6000, CAP).unwrap();
        assert!(s.fails(false).is_empty(), "{:?}", s.fails(false));
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
