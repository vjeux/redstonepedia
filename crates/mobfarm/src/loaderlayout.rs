//! (loaderlay, Nov 13 2026: tick-exact's N = 1 / 2 / 4 loader rows, onechunk's linking rule, ironspec's
//! site record, G1's 22:01 / 22:10 conditions) `mctas mobfarm loader-layout`: a loader layout laid exactly
//! as `mctas mobai loader-long` lays one (mobai `freeze::recipe_part`: each part's overworld cells and
//! entities moved by its offset; each Nether cell at the Nether image of the part's own
//! `port nether-anchor` (anchor + offset, x and z floored by 8, y kept) plus the cell, its y less the
//! part's Nether y base), then:
//! - every portal of the layout (each part's 6-connected `nether_portal` cells, both dimensions) with its
//!   frame; a portal whose frame sides (the cells its cells face in its plane) are not all obsidian in the
//!   built layout is one the game breaks: its records are skipped by the search (their blocks lose
//!   HORIZONTAL_AXIS) and, for a part's own portal, the layout FAILS;
//! - the cells two parts both claim, either dimension (loader-long lays the parts in order, so the later
//!   part's block replaces the earlier's): FAIL;
//! - the loader's FORWARD leg (its overworld firing dropper's item into its Nether portal) and RETURN leg
//!   (its Nether firing dropper's item back), from every exit block the item can have when it crosses,
//!   and every copy's pads from every exit block a golem in them can have: each must land at its own
//!   partner (the loader's other portal; the copy's own Nether twin), with the least margin (the next
//!   portal's distance less the partner's); an exit block with no lit portal in its square FAILS by name
//!   (the game builds a new portal there); a tie at the least distance AND the same lowest y between two
//!   portals FAILS (the game's pick among equals is the POI stream's order, not modeled); the exit blocks
//!   decided by the lowest y are counted; each copy's twin back to the overworld is information;
//! - for each leg, the EXCLUSION ZONE (G1 22:10 (1): every lit portal's record counts, loaded or not, so
//!   the build guide must keep foreign portals out): the search square's bounds over all its exit blocks
//!   (any y), and the capture box (the exit blocks grown by the partner's largest distance: a foreign lit
//!   portal there can win);
//! - for the loader's legs, where the item comes out (G1 22:10 (3)): its crossing box mapped across the
//!   source portal onto the partner (`getDimensionTransitionFromExit`: `PortalShape.getRelativePosition`
//!   then the destination rectangle, redsim's `entity::portal::relative` / `arrival`, before the
//!   collision-free nudge), in world coordinates and as column / row / depth of the partner;
//! - the chunks: every overworld chunk a part's cells or entities use, every Nether chunk, the loader's
//!   PORTAL chunks (the chunk of the portal block each leg picks: `placePortalTicket` at the block
//!   `findClosestPortalPosition` found, ticketcheck.rs), each chunk's level from those two tickets alone
//!   (30 + the Chebyshev distance): a chunk holding a rail or an entity of the layout (and every Nether
//!   chunk) must be entity-ticking (<= 31), any other must tick its blocks (<= 32); the control chunk
//!   (loader-long's: the portal chunk + (2, 2)) must hold no farm cell; each part's cells outside the
//!   chunk holding most of them are listed;
//! - the loader's portal cells and obsidian frames, world and farm frame (world less the first copy's
//!   offset), and its farm-frame offset (ironspec 21:54 / 21:55: the loader goes into vcompose's site
//!   record as cells);
//! - loader-long's command line for the layout.
//!
//! Controls (ironspec's standing rule: every planted state read back, a no-op refuses by name): `--plant`
//! lays a framed portal (its obsidian ring in its plane, corners left out) and `--plant-unframed` its
//! portal cells alone; every planted cell is read back from the built layout; the layout is judged with
//! and without; a plant that moves no exit block's pick refuses, except an unframed one whose framed twin
//! (the same cells framed) does move picks: that is the broken-frame skip's control (G1 22:10). Zero
//! samples never pass (G1 00:46): a leg with no exit block fails, a copy with no pad FAILS NOT JUDGED,
//! a negative or non-finite golem margin, two plants of one name and a dropper firing up or down into a
//! portal are refused; with a loader leg failing, the chunk levels are NOT JUDGED and the tickets are
//! printed where the legs' picks went.
//!
//! The search (tick-exact's 26.3 pins, 22:07, adopted by G1 / ironspec 22:04): the target is
//! `NetherPortalBlock.getPortalDestination` (130-142): scale `DimensionType.getTeleportationScale`
//! (109-113), 1/8 into the Nether and 8 out of it, applied to the entity's own x and z when its portal
//! process completes, y not scaled, then `worldBorder.clampToBounds` and `BlockPos.containing` (WorldBorder
//! 84-86 / 92-94: each coordinate floored; the border never binds here);
//! `PortalForcer.findClosestPortalPosition` (41-50): radius 16 into the Nether, 128 into the overworld,
//! every `nether_portal` POI record in the square |dx| <= radius, |dz| <= radius at any y
//! (`PoiManager.getInSquare` 87-95; a portal in an unloaded chunk counts through its saved record,
//! `ensureLoadedAndValid` 281-290), its block still a portal; the least `distSqr` between block
//! coordinates, then the lowest y. Each lit cell is its own record (only lit portal blocks count:
//! PoiTypes 40 / 117). Nothing in range builds a new portal (`createPortal`). The entity comes out of the
//! picked cell's portal at its relative position in the source portal, so the pick decides the portal and
//! the ticket's chunk, not the spot. redsim's `entity::portal::closest_portal` models the same rule.
//!
//! Where a dispensed item crosses (`DefaultDispenseItemBehavior.spawnItem`, accuracy 6; l1items.rs's
//! bound): it spawns 0.7 in front of the dropper's centre, its feet 0.15625 under the centre's y for a
//! horizontal facing, with velocity forward `nextDouble() * 0.1 + 0.2` ± 0.103365, sideways 0 ± 0.103365
//! and up 0.2 ± 0.103365 (each a bounded triangle); the item's tick applies gravity (0.04) before its move
//! and the 0.98 drag after; it is inside the portal's cell from its spawn, so its first move marks the
//! portal and it crosses at its next tick's `handlePortal` (a non-player's transition time 0) with the
//! position after that one move. The verdict searches the union of the positions after one move and after
//! two (l1-item-bench's allowance), a superset of the code's; the one-move legs are printed beside.
//! A golem (width 1.4) is in a pad when its centre is anywhere over the pad's cells grown 0.7 on x and z,
//! any of the pad's rows (tick-exact 22:02 (5); portal-links' margin).

use std::collections::{BTreeMap, BTreeSet};

use mctas::design::Design;

pub type C = [i32; 3];

/// The dispenser's velocity spread at accuracy 6: 0.0172275 * 6.
pub const SPREAD: f64 = 0.0172275 * 6.0;
/// A golem's half width (its centre over a pad cell grown this much is in the pad).
pub const GOLEM_MARGIN: f64 = 0.7;
/// A dropped item's box (width, height).
const ITEM_DIMS: (f32, f32) = (0.25, 0.25);

/// One part laid as loader-long lays it.
#[derive(Clone, Debug, Default)]
pub struct Laid {
    pub name: String,
    pub file: String,
    pub offset: C,
    /// The Nether image of the anchor: floor((anchor + offset) / 8) in x and z, y kept.
    pub image: C,
    pub ybase: i32,
    pub blocks: BTreeMap<C, String>,
    pub nether: BTreeMap<C, String>,
    pub entities: Vec<(String, [f64; 3])>,
    /// Its block entities' data (`nbt:` lines), moved like its cells.
    pub block_entities: BTreeMap<C, String>,
    /// Its Nether block entities' data (`nether nbt:`), placed like its Nether cells.
    pub nether_block_entities: BTreeMap<C, String>,
    /// The world y of its ground layer (the offset's y plus the design's `ground:`, the design y
    /// lying at the ground layer, default -1 as the farm judge's site reads it): the site's surface.
    pub ground: i32,
    /// Its villager places beyond its rail cells and carts (feet cells, world coordinates): a
    /// positions file in the design's written frame, moved like its cells (`--positions`).
    pub positions: Vec<C>,
    /// Its entities' NBT (`entity:` lines' data: a cart's passenger), parallel to `entities`.
    pub entity_nbt: Vec<Option<String>>,
    /// Its `port output`, moved like its cells.
    pub output: Option<C>,
}

/// mobai `freeze::recipe_part`'s placement of `d` at `offset` (the Nether part at the anchor's image, its
/// y less `ybase`).
pub fn lay(name: &str, file: &str, d: &Design, offset: C, ybase: i32) -> Result<Laid, String> {
    let anchor = *d.ports.get("nether-anchor").ok_or_else(|| format!("{file}: no `port nether-anchor`"))?;
    let a = [anchor[0] + offset[0], anchor[1] + offset[1], anchor[2] + offset[2]];
    let image = [a[0].div_euclid(8), a[1], a[2].div_euclid(8)];
    let mv = |p: &C| [p[0] + offset[0], p[1] + offset[1], p[2] + offset[2]];
    Ok(Laid {
        name: name.to_string(),
        file: file.to_string(),
        offset,
        image,
        ybase,
        blocks: d.blocks.iter().map(|(p, s)| (mv(p), s.clone())).collect(),
        nether: d.nether_blocks.iter().map(|(p, s)| ([image[0] + p[0], image[1] + p[1] - ybase, image[2] + p[2]], s.clone())).collect(),
        entities: d.entities.iter().map(|e| (e.kind.clone(), [e.pos[0] + offset[0] as f64, e.pos[1] + offset[1] as f64, e.pos[2] + offset[2] as f64])).collect(),
        block_entities: d.block_entities.iter().map(|(p, s)| (mv(p), s.clone())).collect(),
        nether_block_entities: d.nether_block_entities.iter().map(|(p, s)| ([image[0] + p[0], image[1] + p[1] - ybase, image[2] + p[2]], s.clone())).collect(),
        ground: offset[1] + d.meta.get("ground").map(|v| v.trim().parse::<i32>().map_err(|e| format!("{file}: ground: {e}"))).transpose()?.unwrap_or(-1),
        positions: Vec::new(),
        entity_nbt: d.entities.iter().map(|e| e.nbt.clone()).collect(),
        output: d.ports.get("output").map(mv),
    })
}

/// (9fc3155f / ironspec Nov 14 05:27: the module lightning check run on the whole layout, all its
/// rods) The overworld layout as one design, world coordinates: every part's overworld cells as the
/// layout builds them (a later part's cell over an earlier one's), its block entities and entities
/// (with their NBT), the first copy's `port output`, `ground:` the copies' ground. The Nether parts are
/// left out (`--write-layout`).
pub fn compose(parts: &[Laid]) -> Result<Design, String> {
    let (ow, _) = built(parts, false, &[])?;
    let first = parts.iter().find(|p| p.name != "loader").ok_or("no copy")?;
    let mut d = Design { name: format!("loader-layout ({} parts, the overworld)", parts.len()), ..Default::default() };
    d.blocks = ow.iter().map(|(c, (s, _))| (*c, s.clone())).collect();
    for p in parts {
        d.block_entities.extend(p.block_entities.iter().map(|(c, s)| (*c, s.clone())));
        for (k, (kind, e)) in p.entities.iter().enumerate() {
            d.entities.push(mctas::design::EntitySpec { kind: kind.clone(), pos: *e, nbt: p.entity_nbt.get(k).cloned().flatten() });
        }
    }
    if let Some(o) = first.output {
        d.ports.insert("output".into(), o);
    }
    d.meta.insert("ground".into(), first.ground.to_string());
    Ok(d)
}

/// The binary rule's global flags as mctas's dispatch parsed them (`--cite <commit>` / `--cite=<commit>`,
/// HEAD by default; `--not-evidence`), read from the process arguments (G1 06:37 N2: the shared
/// provenance tool has no accessor yet; these are the same clap globals).
pub fn provenance_args() -> (String, bool) {
    let args: Vec<String> = std::env::args().collect();
    let mut cite = "HEAD".to_string();
    for (k, a) in args.iter().enumerate() {
        if let Some(v) = a.strip_prefix("--cite=") {
            cite = v.to_string();
        } else if a == "--cite"
            && let Some(v) = args.get(k + 1)
        {
            cite = v.clone();
        }
    }
    (cite, args.iter().any(|a| a == "--not-evidence"))
}

/// (G1 06:37 (6), ironspec 06:43) A design file a run reads is an INPUT: it must lie inside the worktree
/// (the current directory's) and be the cited commit's blob at that path. `Ok("path blob")`, or the
/// refusal naming why.
pub fn cited_input(p: &std::path::Path, cite: &str) -> Result<String, String> {
    cited_input_in(&std::env::current_dir().map_err(|e| e.to_string())?, p, cite)
}

/// [`cited_input`] for the worktree holding `dir`.
pub fn cited_input_in(dir: &std::path::Path, p: &std::path::Path, cite: &str) -> Result<String, String> {
    let git = |args: &[&str]| -> Result<String, String> {
        let o = std::process::Command::new("git").arg("-C").arg(dir).args(args).output().map_err(|e| format!("git {}: {e}", args.join(" ")))?;
        if !o.status.success() {
            return Err(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim()));
        }
        Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let top = std::fs::canonicalize(git(&["rev-parse", "--show-toplevel"])?).map_err(|e| format!("the worktree: {e}"))?;
    let abs = std::fs::canonicalize(if p.is_absolute() { p.to_path_buf() } else { dir.join(p) }).map_err(|e| format!("{}: {e}", p.display()))?;
    let rel = abs.strip_prefix(&top).map_err(|_| format!("{} lies outside the worktree {} (an evidence run reading a file outside the worktree is NOT EVIDENCE, ironspec 06:43)", p.display(), top.display()))?;
    let rel = rel.to_string_lossy().replace('\\', "/");
    let have = git(&["hash-object", &abs.to_string_lossy()])?;
    let want = git(&["rev-parse", &format!("{cite}:{rel}")]).map_err(|_| format!("{rel} is not in the cited commit {cite}"))?;
    if have != want {
        return Err(format!("{rel} is blob {have}, not the cited commit {cite}'s {want}"));
    }
    Ok(format!("{rel} {}", &have[..12.min(have.len())]))
}

/// A planted portal (a control): its cells, framed or not.
#[derive(Clone, Debug)]
pub struct Plant {
    pub name: String,
    pub nether: bool,
    pub cells: Vec<C>,
    pub framed: bool,
}

/// The box of a cell list.
fn lo_hi(cells: &[C]) -> (C, C) {
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for c in cells {
        for k in 0..3 {
            lo[k] = lo[k].min(c[k]);
            hi[k] = hi[k].max(c[k]);
        }
    }
    (lo, hi)
}

/// A planar box's portal axis: 'x' when it is one cell deep in z, 'z' when one deep in x.
fn plane_axis(cells: &[C]) -> Result<char, String> {
    let (lo, hi) = lo_hi(cells);
    if lo[2] == hi[2] {
        Ok('x')
    } else if lo[0] == hi[0] {
        Ok('z')
    } else {
        Err(format!("a portal is one cell thick in x or z: box {lo:?}..{hi:?}"))
    }
}

/// The ring around a portal's box in its plane: (cell, a corner).
fn ring(cells: &[C], axis: char) -> Vec<(C, bool)> {
    let (lo, hi) = lo_hi(cells);
    let (u0, u1) = if axis == 'x' { (lo[0], hi[0]) } else { (lo[2], hi[2]) };
    let mut v = Vec::new();
    for y in lo[1] - 1..=hi[1] + 1 {
        for u in u0 - 1..=u1 + 1 {
            let side_u = u == u0 - 1 || u == u1 + 1;
            let side_y = y == lo[1] - 1 || y == hi[1] + 1;
            if side_u || side_y {
                let c = if axis == 'x' { [u, y, lo[2]] } else { [lo[0], y, u] };
                v.push((c, side_u && side_y));
            }
        }
    }
    v
}

impl Plant {
    /// The cells it lays: its portal cells, and its frame (the ring less the corners) when framed.
    pub fn laid(&self) -> Result<Vec<(C, String)>, String> {
        let axis = plane_axis(&self.cells)?;
        let mut v: Vec<(C, String)> = self.cells.iter().map(|c| (*c, format!("minecraft:nether_portal[axis={axis}]"))).collect();
        if self.framed {
            v.extend(ring(&self.cells, axis).into_iter().filter(|(_, corner)| !corner).map(|(c, _)| (c, "minecraft:obsidian".to_string())));
        }
        Ok(v)
    }
}

/// A portal of the layout.
#[derive(Clone, Debug)]
pub struct Portal {
    pub name: String,
    /// Index of its part (`usize::MAX`: a planted portal).
    pub part: usize,
    pub nether: bool,
    pub cells: Vec<C>,
    /// 'x' or 'z' (the block state's axis).
    pub axis: char,
}

/// The built map of one dimension: (state, the part's index, `usize::MAX` for a plant).
pub type Built = BTreeMap<C, (String, usize)>;

impl Portal {
    pub fn lo_hi(&self) -> (C, C) {
        lo_hi(&self.cells)
    }

    /// The obsidian of its frame ring (corners included) in the built layout.
    pub fn frame(&self, built: &Built) -> Vec<C> {
        ring(&self.cells, self.axis).into_iter().filter(|(c, _)| built.get(c).is_some_and(|b| b.0 == "minecraft:obsidian")).map(|(c, _)| c).collect()
    }

    /// The cells its cells face in its plane (corners excluded) that are neither its own nor obsidian in
    /// the built layout: the game breaks the portal (`NetherPortalBlock.updateShape`).
    pub fn broken_sides(&self, built: &Built) -> Vec<C> {
        let set: BTreeSet<C> = self.cells.iter().copied().collect();
        let mut out = BTreeSet::new();
        let dirs: [C; 4] = if self.axis == 'x' { [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0]] } else { [[0, 0, 1], [0, 0, -1], [0, 1, 0], [0, -1, 0]] };
        for c in &self.cells {
            for d in dirs {
                let n = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
                if !set.contains(&n) && !built.get(&n).is_some_and(|b| b.0 == "minecraft:obsidian") {
                    out.insert(n);
                }
            }
        }
        out.into_iter().collect()
    }

    /// Its rectangle for redsim's portal mapping (a whole rectangle; `rect_at`'s shape).
    pub fn rect(&self) -> redsim::entity::portal::Rect {
        let (lo, hi) = self.lo_hi();
        let x_axis = self.axis == 'x';
        redsim::entity::portal::Rect { corner: redsim::pos::BlockPos::new(lo[0], lo[1], lo[2]), w: if x_axis { hi[0] - lo[0] + 1 } else { hi[2] - lo[2] + 1 }, h: hi[1] - lo[1] + 1, x_axis }
    }
}

/// The 6-connected `nether_portal` components of a cell map, each with its axis.
pub fn components(cells: &BTreeMap<C, String>) -> Vec<(Vec<C>, char)> {
    let portal: BTreeSet<C> = cells.iter().filter(|(_, s)| s.contains("nether_portal")).map(|(p, _)| *p).collect();
    let mut seen: BTreeSet<C> = BTreeSet::new();
    let mut out = Vec::new();
    for &p in &portal {
        if !seen.insert(p) {
            continue;
        }
        let mut comp = Vec::new();
        let mut stack = vec![p];
        while let Some(q) = stack.pop() {
            comp.push(q);
            for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                let n = [q[0] + d[0], q[1] + d[1], q[2] + d[2]];
                if portal.contains(&n) && seen.insert(n) {
                    stack.push(n);
                }
            }
        }
        comp.sort();
        let axis = if cells[&p].contains("axis=z") { 'z' } else { 'x' };
        out.push((comp, axis));
    }
    out
}

/// A horizontal facing's unit step.
fn step(facing: &str) -> Option<C> {
    match facing {
        "north" => Some([0, 0, -1]),
        "south" => Some([0, 0, 1]),
        "west" => Some([-1, 0, 0]),
        "east" => Some([1, 0, 0]),
        _ => None,
    }
}

/// A block state's property value.
fn prop<'a>(state: &'a str, key: &str) -> Option<&'a str> {
    let inner = state.split_once('[')?.1.strip_suffix(']')?;
    inner.split(',').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| v))
}

/// The droppers of `cells` that fire into one of `portal`'s cells (a horizontal facing onto it); one
/// facing up or down into it is refused (its crossing is not modeled here).
pub fn firing_droppers(cells: &BTreeMap<C, String>, portal: &[C]) -> Result<Vec<(C, C)>, String> {
    let set: BTreeSet<C> = portal.iter().copied().collect();
    let mut out = Vec::new();
    for (p, s) in cells {
        if !s.starts_with("minecraft:dropper") && !s.starts_with("minecraft:dispenser") {
            continue;
        }
        let f = prop(s, "facing");
        if let Some(d) = f.and_then(step) {
            let q = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
            if set.contains(&q) {
                out.push((*p, d));
            }
        } else if let Some(dy) = f.and_then(|f| match f {
            "up" => Some(1),
            "down" => Some(-1),
            _ => None,
        }) && set.contains(&[p[0], p[1] + dy, p[2]])
        {
            return Err(format!("the dropper at ({},{},{}) fires {} into a portal cell: a vertical crossing is not modeled, refused", p[0], p[1], p[2], f.unwrap_or("?")));
        }
    }
    Ok(out)
}

/// Where the item a dropper at `p` facing `d` (horizontal) can be when it crosses: its position (feet)
/// box, after one move (`moves` 1, the code's crossing) or over one and two moves (2).
pub fn crossing_box(p: C, d: C, moves: u32) -> ([f64; 3], [f64; 3]) {
    let (f0, f1) = (0.2 - SPREAD, 0.3 + SPREAD);
    let (u0, u1) = (0.2 - SPREAD - 0.04, 0.2 + SPREAD - 0.04);
    // (forward and sideways steps over the moves: each later move's step decayed by the 0.98 drag; the
    // second move's y step is the first's x 0.98 less the gravity)
    let k: f64 = if moves >= 2 { 1.98 } else { 1.0 };
    let (fwd_lo, fwd_hi) = (f0, f1 * k);
    let side = SPREAD * k;
    let (dy_lo, dy_hi) = (u0, if moves >= 2 { (u1 * 1.98 - 0.04).max(u1) } else { u1 });
    let centre = [p[0] as f64 + 0.5 + 0.7 * d[0] as f64, p[1] as f64 + 0.5 - 0.15625, p[2] as f64 + 0.5 + 0.7 * d[2] as f64];
    let mut lo = [0.0; 3];
    let mut hi = [0.0; 3];
    for a in [0usize, 2] {
        if d[a] != 0 {
            let s = d[a] as f64;
            let (x0, x1) = (centre[a] + s * fwd_lo, centre[a] + s * fwd_hi);
            lo[a] = x0.min(x1);
            hi[a] = x0.max(x1);
        } else {
            lo[a] = centre[a] - side;
            hi[a] = centre[a] + side;
        }
    }
    lo[1] = centre[1] + dy_lo;
    hi[1] = centre[1] + dy_hi;
    (lo, hi)
}

/// Every exit block of a position box scaled by `s` on x and z (y kept): `BlockPos.containing`.
pub fn exits_of_box(lo: [f64; 3], hi: [f64; 3], s: f64) -> Vec<C> {
    let r = |a: f64, b: f64, sc: f64| ((a * sc).floor() as i32, (b * sc).floor() as i32);
    let (x0, x1) = r(lo[0], hi[0], s);
    let (y0, y1) = r(lo[1], hi[1], 1.0);
    let (z0, z1) = r(lo[2], hi[2], s);
    let mut out = Vec::new();
    for x in x0..=x1 {
        for y in y0..=y1 {
            for z in z0..=z1 {
                out.push([x, y, z]);
            }
        }
    }
    out
}

/// Every exit block of an entity whose centre is anywhere over `cells` grown `margin` on x and z (the
/// cell's own y), scaled by `s`.
pub fn exits_of_cells(cells: &[C], margin: f64, s: f64) -> Vec<C> {
    let mut out: BTreeSet<C> = BTreeSet::new();
    for c in cells {
        let lo = [c[0] as f64 - margin, c[1] as f64, c[2] as f64 - margin];
        // (the centre strictly inside the grown cell: its far edge excluded)
        let hi = [c[0] as f64 + 1.0 + margin - 1e-9, c[1] as f64, c[2] as f64 + 1.0 + margin - 1e-9];
        out.extend(exits_of_box(lo, hi, s));
    }
    out.into_iter().collect()
}

/// The search from exit block `t`: for each portal in `dest` its least (squared distance, y, cell) inside
/// the square of `radius`, sorted (the game's pick first).
pub fn candidates(t: C, dest: &[&Portal], radius: i32) -> Vec<(i64, i32, usize, C)> {
    let mut out = Vec::new();
    for (i, q) in dest.iter().enumerate() {
        let best = q
            .cells
            .iter()
            .filter(|c| (c[0] - t[0]).abs() <= radius && (c[2] - t[2]).abs() <= radius)
            .map(|c| {
                let d = [(c[0] - t[0]) as i64, (c[1] - t[1]) as i64, (c[2] - t[2]) as i64];
                (d[0] * d[0] + d[1] * d[1] + d[2] * d[2], c[1], *c)
            })
            .min();
        if let Some((d, y, c)) = best {
            out.push((d, y, i, c));
        }
    }
    out.sort();
    out
}

/// One link check: a source's exit blocks against the destination portals.
#[derive(Clone, Debug, Default)]
pub struct Leg {
    pub label: String,
    pub expected: Option<String>,
    pub radius: i32,
    pub targets: Vec<C>,
    /// target -> the picked portal's name and the picked cell (None: nothing in the square; a tie: the
    /// tied names joined by " = ").
    pub picked: BTreeMap<C, (String, Option<C>)>,
    pub ties: usize,
    pub empty: usize,
    /// Exit blocks whose least distance two portals share, decided by the lower y.
    pub y_decided: usize,
    /// The least (next portal's distance - the partner's), over the targets.
    pub least_margin: f64,
    /// The partner's largest distance over the targets.
    pub worst_own: f64,
    pub dists: BTreeMap<String, (f64, f64)>,
}

impl Leg {
    /// (G1 00:46 (1): zero samples never pass) every exit block searched picks the partner, with no tie
    /// and no empty search, and at least one exit block was searched.
    pub fn pass(&self) -> bool {
        !self.targets.is_empty() && self.ties == 0 && self.empty == 0 && self.expected.as_ref().is_none_or(|w| self.picked.values().all(|(n, _)| n == w))
    }

    pub fn chosen(&self) -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        for (n, _) in self.picked.values() {
            *m.entry(n.clone()).or_insert(0) += 1;
        }
        m
    }

    /// The chunks of every picked cell, whichever portal (G1 00:46: a PORTAL ticket goes where the pick
    /// is, the partner or not).
    pub fn picked_chunks(&self) -> BTreeSet<[i32; 2]> {
        self.picked.values().filter_map(|(_, c)| *c).map(|c| [c[0].div_euclid(16), c[2].div_euclid(16)]).collect()
    }
}

pub fn run_leg(label: &str, targets: Vec<C>, dest: &[&Portal], radius: i32, expected: Option<&str>) -> Leg {
    let mut leg = Leg { label: label.to_string(), expected: expected.map(|s| s.to_string()), radius, least_margin: f64::INFINITY, ..Default::default() };
    for t in &targets {
        let cands = candidates(*t, dest, radius);
        for (d, _, i, _) in &cands {
            let r = (*d as f64).sqrt();
            let e = leg.dists.entry(dest[*i].name.clone()).or_insert((f64::INFINITY, 0.0));
            e.0 = e.0.min(r);
            e.1 = e.1.max(r);
        }
        let Some(&(d0, y0, i0, c0)) = cands.first() else {
            leg.empty += 1;
            leg.picked.insert(*t, ("NOTHING (no lit portal in the square: the game builds a new one there)".into(), None));
            continue;
        };
        let same_d: Vec<&(i64, i32, usize, C)> = cands.iter().filter(|c| c.0 == d0).collect();
        let tied: Vec<&str> = same_d.iter().filter(|c| c.1 == y0).map(|c| dest[c.2].name.as_str()).collect();
        if tied.len() > 1 {
            leg.ties += 1;
            leg.picked.insert(*t, (format!("TIE {}", tied.join(" = ")), Some(c0)));
        } else {
            if same_d.len() > 1 {
                leg.y_decided += 1;
            }
            leg.picked.insert(*t, (dest[i0].name.clone(), Some(c0)));
        }
        if let Some(w) = expected {
            let own = cands.iter().find(|c| dest[c.2].name == w).map(|c| (c.0 as f64).sqrt());
            let other = cands.iter().filter(|c| dest[c.2].name != w).map(|c| (c.0 as f64).sqrt()).fold(f64::INFINITY, f64::min);
            match own {
                Some(o) => {
                    leg.least_margin = leg.least_margin.min(other - o);
                    leg.worst_own = leg.worst_own.max(o);
                }
                None => leg.least_margin = f64::NEG_INFINITY,
            }
        }
    }
    leg.targets = targets;
    leg
}

/// The text of one leg, its exclusion zone included.
pub fn leg_text(leg: &Leg, dim: &str) -> String {
    let (lo, hi) = lo_hi(&leg.targets);
    let mut s = format!("  {}: {} exit blocks x {}..{} y {}..{} z {}..{} ({dim}, searched within {})\n", leg.label, leg.targets.len(), lo[0], hi[0], lo[1], hi[1], lo[2], hi[2], leg.radius);
    let mut dl: Vec<(&String, &(f64, f64))> = leg.dists.iter().collect();
    dl.sort_by(|a, b| a.1.0.partial_cmp(&b.1.0).unwrap_or(std::cmp::Ordering::Equal));
    for (n, (a, b)) in dl.iter().take(4) {
        s.push_str(&format!("    candidate {n}: {a:.2}..{b:.2}{}\n", if leg.expected.as_deref() == Some(n.as_str()) { " (the partner)" } else { "" }));
    }
    let chosen: Vec<String> = leg.chosen().iter().map(|(n, k)| format!("{n} x {k}")).collect();
    let verdict = match &leg.expected {
        Some(w) => {
            if leg.pass() {
                format!("PASS (every exit block picks {w}; the partner's distance at most {:.2}, least margin {:.2})", leg.worst_own, leg.least_margin)
            } else {
                let bad: Vec<String> = leg.picked.iter().filter(|(_, (n, _))| n != w).take(6).map(|(t, (n, _))| format!("({},{},{}) -> {n}", t[0], t[1], t[2])).collect();
                format!("FAIL (expected {w}; {} of {} exit blocks go elsewhere, e.g. {})", leg.picked.values().filter(|(n, _)| n != w).count(), leg.targets.len(), bad.join(", "))
            }
        }
        None => "information".into(),
    };
    s.push_str(&format!("    picked: {}; decided by the lowest y among equal distances: {}; {verdict}\n", chosen.join(", "), leg.y_decided));
    if leg.empty > 0 {
        s.push_str(&format!("    FAIL: {} exit blocks find no lit portal within {}: the game builds a new portal there (createPortal)\n", leg.empty, leg.radius));
    }
    if leg.ties > 0 {
        s.push_str(&format!("    FAIL: {} exit blocks see two portals at the same least distance and the same lowest y (the pick is the POI stream's order)\n", leg.ties));
    }
    if leg.expected.is_some() {
        let r = leg.radius;
        let g = leg.worst_own.ceil() as i32;
        s.push_str(&format!(
            "    exclusion zone ({dim}, every lit portal's record counts, loaded or not): the search square x {}..{} z {}..{} at every y holds no lit portal but the layout's own; a foreign lit portal within the partner's distance of an exit block wins: the capture box x {}..{} y {}..{} z {}..{}\n",
            lo[0] - r,
            hi[0] + r,
            lo[2] - r,
            hi[2] + r,
            lo[0] - g,
            hi[0] + g,
            lo[1] - g,
            hi[1] + g,
            lo[2] - g,
            hi[2] + g
        ));
    }
    s
}

/// The built map of one dimension: loader-long lays the parts in order, a later part's block replacing an
/// earlier one's; then the plants; the cells claimed twice listed.
pub fn built(parts: &[Laid], nether: bool, plants: &[Plant]) -> Result<(Built, Vec<String>), String> {
    let mut map: Built = BTreeMap::new();
    let mut clashes = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        for (c, s) in if nether { &p.nether } else { &p.blocks } {
            if let Some((old, j)) = map.get(c) {
                clashes.push(format!("({},{},{}) {} {} -> {} {}", c[0], c[1], c[2], parts[*j].name, old, p.name, s));
            }
            map.insert(*c, (s.clone(), i));
        }
    }
    for pl in plants.iter().filter(|p| p.nether == nether) {
        for (c, s) in pl.laid()? {
            if let Some((old, j)) = map.get(&c) {
                let who = if *j == usize::MAX { "a plant".to_string() } else { parts[*j].name.clone() };
                clashes.push(format!("({},{},{}) {who} {old} -> {} {s}", c[0], c[1], c[2], pl.name));
            }
            map.insert(c, (s, usize::MAX));
        }
    }
    Ok((map, clashes))
}

/// Every portal of the layout, named: `copyK.padJ` / `copyK.twin` for the copies, `loader.overworld` /
/// `loader.nether`, the plants by their own names.
pub fn portals(parts: &[Laid], plants: &[Plant]) -> Result<Vec<Portal>, String> {
    let mut out = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        for nether in [false, true] {
            let comps = components(if nether { &p.nether } else { &p.blocks });
            let n = comps.len();
            for (j, (cells, axis)) in comps.into_iter().enumerate() {
                let loader = p.name == "loader";
                let base = match (loader, nether) {
                    (true, false) => "overworld",
                    (true, true) => "nether",
                    (false, false) => "pad",
                    (false, true) => "twin",
                };
                let name = if n == 1 && (loader || nether) { format!("{}.{base}", p.name) } else { format!("{}.{base}{}", p.name, j + 1) };
                out.push(Portal { name, part: i, nether, cells, axis });
            }
        }
    }
    for pl in plants {
        let mut cells = pl.cells.clone();
        cells.sort();
        out.push(Portal { name: pl.name.clone(), part: usize::MAX, nether: pl.nether, cells: cells.clone(), axis: plane_axis(&cells)? });
    }
    Ok(out)
}

/// The result of one evaluation (with or without the plants).
pub struct Eval {
    pub legs: Vec<Leg>,
    pub info: Vec<Leg>,
    pub text: String,
    pub ok: bool,
    pub ow_portal_chunks: BTreeSet<[i32; 2]>,
    pub nether_portal_chunks: BTreeSet<[i32; 2]>,
}

/// Cells as x-runs: "(x0..x1,y,z)".
pub fn runs_text(cells: &[C]) -> String {
    let mut rows: BTreeMap<(i32, i32), Vec<i32>> = BTreeMap::new();
    for c in cells {
        rows.entry((c[1], c[2])).or_default().push(c[0]);
    }
    let mut parts = Vec::new();
    for ((y, z), mut xs) in rows {
        xs.sort();
        xs.dedup();
        let mut start = xs[0];
        let mut prev = xs[0];
        for &x in xs.iter().skip(1).chain(std::iter::once(&i32::MAX)) {
            if x != prev + 1 {
                parts.push(if start == prev { format!("({start},{y},{z})") } else { format!("({start}..{prev},{y},{z})") });
                start = x;
            }
            prev = x;
        }
    }
    parts.join(" ")
}

/// Where a loader leg's item comes out: its crossing box mapped across `src` onto `dst` (redsim's
/// `relative` / `arrival`, the item's box, not living), world and partner-local (column along the axis,
/// row up, depth across, from the partner's min corner).
fn arrival_text(src: &Portal, dst: &Portal, lo: [f64; 3], hi: [f64; 3]) -> String {
    use redsim::entity::Vec3;
    let (rs, rd) = (src.rect(), dst.rect());
    let map = |p: [f64; 3]| -> [f64; 3] {
        let rel = redsim::entity::portal::relative(&rs, Vec3::new(p[0], p[1], p[2]), ITEM_DIMS, false);
        let a = redsim::entity::portal::arrival(&rd, rel, ITEM_DIMS);
        [a.x, a.y, a.z]
    };
    let (a, b) = (map(lo), map(hi));
    let alo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
    let ahi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
    let c = [rd.corner.x as f64, rd.corner.y as f64, rd.corner.z as f64];
    let (col, dep) = if rd.x_axis { (0usize, 2usize) } else { (2, 0) };
    format!(
        "    it comes out of {} at x {:.3}..{:.3} y {:.3}..{:.3} z {:.3}..{:.3} (before the collision-free nudge): column {:.3}..{:.3} of {}, row {:.3}..{:.3} of {}, depth {:.3}..{:.3}\n",
        dst.name,
        alo[0],
        ahi[0],
        alo[1],
        ahi[1],
        alo[2],
        ahi[2],
        alo[col] - c[col],
        ahi[col] - c[col],
        rd.w,
        alo[1] - c[1],
        ahi[1] - c[1],
        rd.h,
        alo[dep] - c[dep],
        ahi[dep] - c[dep]
    )
}

/// The portals, the legs and their verdicts.
pub fn evaluate(parts: &[Laid], plants: &[Plant], golem_margin: f64) -> Result<Eval, String> {
    let (ow, ow_clash) = built(parts, false, plants)?;
    let (nw, n_clash) = built(parts, true, plants)?;
    let ps = portals(parts, plants)?;
    let mut s = String::new();
    let mut ok = true;
    s.push_str(&format!("portals: {} overworld, {} Nether\n", ps.iter().filter(|p| !p.nether).count(), ps.iter().filter(|p| p.nether).count()));
    let mut skipped: BTreeSet<String> = BTreeSet::new();
    for p in &ps {
        let built_map = if p.nether { &nw } else { &ow };
        let (lo, hi) = p.lo_hi();
        let frame = p.frame(built_map);
        let broken = p.broken_sides(built_map);
        let broken_text = broken.iter().map(|c| format!("({},{},{}) {}", c[0], c[1], c[2], built_map.get(c).map(|b| b.0.as_str()).unwrap_or("air"))).collect::<Vec<_>>().join(", ");
        let note = if broken.is_empty() {
            String::new()
        } else if p.part == usize::MAX {
            skipped.insert(p.name.clone());
            format!("; frame sides not obsidian: {broken_text}: the game breaks it, its records skipped (HORIZONTAL_AXIS gone)")
        } else {
            ok = false;
            skipped.insert(p.name.clone());
            format!("; FAIL: frame sides not obsidian in the built layout: {broken_text}: the game breaks this portal (its records skipped)")
        };
        s.push_str(&format!(
            "  {} ({}, axis {}): {} cells x {}..{} y {}..{} z {}..{}; {} obsidian in its frame ring{note}\n",
            p.name,
            if p.nether { "Nether" } else { "overworld" },
            p.axis,
            p.cells.len(),
            lo[0],
            hi[0],
            lo[1],
            hi[1],
            lo[2],
            hi[2],
            frame.len()
        ));
    }
    for (what, cl) in [("overworld", &ow_clash), ("Nether", &n_clash)] {
        s.push_str(&format!("cells two parts claim ({what}): {}{}\n", cl.len(), if cl.is_empty() { String::new() } else { format!(" FAIL: {}", cl.iter().take(12).cloned().collect::<Vec<_>>().join("; ")) }));
        if !cl.is_empty() {
            ok = false;
        }
    }
    // (the search sees only portals the game keeps)
    let ow_ps: Vec<&Portal> = ps.iter().filter(|p| !p.nether && !skipped.contains(&p.name)).collect();
    let n_ps: Vec<&Portal> = ps.iter().filter(|p| p.nether && !skipped.contains(&p.name)).collect();
    // (the loader's legs: its firing droppers' items)
    let li = parts.iter().position(|p| p.name == "loader").ok_or("no loader part")?;
    if ps.iter().filter(|p| p.part == li).count() != 2 {
        return Err("the loader must hold exactly one portal in each dimension".into());
    }
    let lo_ow = ps.iter().find(|p| p.part == li && !p.nether).ok_or("the loader has no overworld portal")?;
    let lo_n = ps.iter().find(|p| p.part == li && p.nether).ok_or("the loader has no Nether portal")?;
    let mut legs = Vec::new();
    let mut info = Vec::new();
    for (src, dst, cells, dest, scale, radius, label, dim) in [
        (lo_ow, lo_n, &parts[li].blocks, &n_ps, 0.125, 16, "the loader's FORWARD leg (overworld -> Nether)", "the Nether"),
        (lo_n, lo_ow, &parts[li].nether, &ow_ps, 8.0, 128, "the loader's RETURN leg (Nether -> overworld)", "the overworld"),
    ] {
        let fire = firing_droppers(cells, &src.cells)?;
        if fire.len() != 1 {
            return Err(format!("{}: {} droppers fire into it (one expected)", src.name, fire.len()));
        }
        let (p, d) = fire[0];
        let (l1, h1) = crossing_box(p, d, 1);
        let (l2, h2) = crossing_box(p, d, 2);
        s.push_str(&format!(
            "{label}: the firing dropper ({},{},{}) facing {:?} into {}; the item crosses at (feet) x {:.3}..{:.3} y {:.3}..{:.3} z {:.3}..{:.3} after one move (x {:.3}..{:.3} y {:.3}..{:.3} z {:.3}..{:.3} over two)\n",
            p[0], p[1], p[2], d, src.name, l1[0], h1[0], l1[1], h1[1], l1[2], h1[2], l2[0], h2[0], l2[1], h2[1], l2[2], h2[2]
        ));
        s.push_str(&arrival_text(src, dst, l1, h1));
        let lo = [l1[0].min(l2[0]), l1[1].min(l2[1]), l1[2].min(l2[2])];
        let hi = [h1[0].max(h2[0]), h1[1].max(h2[1]), h1[2].max(h2[2])];
        let leg = run_leg(&format!("{} -> {} (one or two moves: the verdict)", src.name, dst.name), exits_of_box(lo, hi, scale), dest, radius, Some(&dst.name));
        let leg1 = run_leg(&format!("{} -> {} (one move alone, the code's crossing; information)", src.name, dst.name), exits_of_box(l1, h1, scale), dest, radius, Some(&dst.name));
        s.push_str(&leg_text(&leg, dim));
        s.push_str(&leg_text(&leg1, dim));
        ok &= leg.pass();
        legs.push(leg);
        info.push(leg1);
    }
    // (every copy's pads: a golem in them)
    for (i, part) in parts.iter().enumerate() {
        if i == li {
            continue;
        }
        let twin: Vec<&Portal> = ps.iter().filter(|p| p.part == i && p.nether).collect();
        let twin_name = match twin.as_slice() {
            [t] => t.name.clone(),
            _ => return Err(format!("{}: {} Nether portals (its pads' twin: one expected)", part.name, twin.len())),
        };
        let pads: Vec<&Portal> = ps.iter().filter(|p| p.part == i && !p.nether).collect();
        if pads.is_empty() {
            // (G1 00:46 (1): a copy with no pad has no golem leg to judge: it fails, never passes)
            ok = false;
            s.push_str(&format!("  FAIL: {}: no pad (no overworld portal in the module: NOT JUDGED)\n", part.name));
        }
        for pad in pads {
            let leg = run_leg(&format!("{} (a golem's centre over its cells grown {golem_margin}) -> {twin_name}", pad.name), exits_of_cells(&pad.cells, golem_margin, 0.125), &n_ps, 16, Some(&twin_name));
            s.push_str(&leg_text(&leg, "the Nether"));
            ok &= leg.pass();
            legs.push(leg);
        }
        for t in twin {
            let leg = run_leg(&format!("{} back to the overworld (a centre over its cells grown {golem_margin}; information)", t.name), exits_of_cells(&t.cells, golem_margin, 8.0), &ow_ps, 128, None);
            s.push_str(&leg_text(&leg, "the overworld"));
            info.push(leg);
        }
    }
    let ow_portal_chunks = legs[1].picked_chunks();
    let nether_portal_chunks = legs[0].picked_chunks();
    Ok(Eval { legs, info, text: s, ok, ow_portal_chunks, nether_portal_chunks })
}

/// Chebyshev distance between chunks.
fn cheb(a: [i32; 2], b: [i32; 2]) -> i32 {
    (a[0] - b[0]).abs().max((a[1] - b[1]).abs())
}

/// What the chunk report hands loader-long: the overworld and Nether PORTAL chunks, the control chunk,
/// every overworld chunk and every Nether chunk.
pub struct Chunks {
    pub ow_portal: [i32; 2],
    pub nether_portal: [i32; 2],
    pub control: [i32; 2],
    pub ow: Vec<[i32; 2]>,
    pub nether: Vec<[i32; 2]>,
}

/// The chunks: each part's, the PORTAL chunks, each chunk's level from the loader's two tickets alone, the
/// control chunk.
pub fn chunks_text(parts: &[Laid], ev: &Eval) -> (String, bool, Option<Chunks>) {
    let ch = |x: i32, z: i32| [x.div_euclid(16), z.div_euclid(16)];
    let mut s = String::new();
    let mut ok = true;
    let mut ow_all: BTreeSet<[i32; 2]> = BTreeSet::new();
    let mut n_all: BTreeSet<[i32; 2]> = BTreeSet::new();
    for p in parts {
        let mut o: BTreeSet<[i32; 2]> = p.blocks.keys().map(|c| ch(c[0], c[2])).collect();
        o.extend(p.entities.iter().map(|(_, e)| ch(e[0].floor() as i32, e[2].floor() as i32)));
        let n: BTreeSet<[i32; 2]> = p.nether.keys().map(|c| ch(c[0], c[2])).collect();
        s.push_str(&format!("  {} at ({},{},{}) (Nether image ({},{},{}), y base {}): overworld chunks {:?}, Nether chunks {:?}\n", p.name, p.offset[0], p.offset[1], p.offset[2], p.image[0], p.image[1], p.image[2], p.ybase, o, n));
        // (the part's cells and entities outside the chunk holding most of its cells: what spills)
        if o.len() > 1 {
            let mut count: BTreeMap<[i32; 2], usize> = BTreeMap::new();
            for c in p.blocks.keys() {
                *count.entry(ch(c[0], c[2])).or_default() += 1;
            }
            let main = count.iter().max_by_key(|(_, k)| **k).map(|(c, _)| *c).unwrap_or([0, 0]);
            let spill: Vec<String> = p.blocks.iter().filter(|(c, _)| ch(c[0], c[2]) != main).map(|(c, st)| format!("({},{},{}) {}", c[0], c[1], c[2], st.trim_start_matches("minecraft:"))).collect();
            let espill: Vec<String> = p.entities.iter().filter(|(_, e)| ch(e[0].floor() as i32, e[2].floor() as i32) != main).map(|(k, e)| format!("{k} ({:.2},{:.2},{:.2})", e[0], e[1], e[2])).collect();
            s.push_str(&format!("    outside its chunk ({},{}): {} cells{}{}\n", main[0], main[1], spill.len(), if spill.is_empty() { String::new() } else { format!(": {}", spill.iter().take(10).cloned().collect::<Vec<_>>().join(", ")) }, if espill.is_empty() { String::new() } else { format!("; entities: {}", espill.join(", ")) }));
        }
        ow_all.extend(o);
        n_all.extend(n);
    }
    // (G1 00:46: the tickets go wherever the legs pick; with a loader leg failing, the levels from the
    // loader's own portal chunks would judge a layout that doesn't run: NOT JUDGED)
    if !ev.legs.iter().take(2).all(|l| l.pass()) {
        s.push_str(&format!(
            "  the PORTAL tickets go where the legs pick: overworld chunks {:?} (the return leg's), Nether chunks {:?} (the forward leg's); a loader leg FAILS: the chunk levels are NOT JUDGED\n",
            ev.ow_portal_chunks, ev.nether_portal_chunks
        ));
        return (s, false, None);
    }
    let (Some(&owp), Some(&np)) = (ev.ow_portal_chunks.iter().next(), ev.nether_portal_chunks.iter().next()) else {
        s.push_str("  FAIL: a loader leg picks no cell: no PORTAL chunk\n");
        return (s, false, None);
    };
    if ev.ow_portal_chunks.len() != 1 || ev.nether_portal_chunks.len() != 1 {
        ok = false;
        s.push_str(&format!("  FAIL: a leg's picked cells span chunks (overworld {:?}, Nether {:?}): the ticket moves with the item\n", ev.ow_portal_chunks, ev.nether_portal_chunks));
    }
    s.push_str(&format!("  the loader's PORTAL tickets: overworld chunk ({},{}) (the return leg's picked cells), Nether chunk ({},{}) (the forward leg's)\n", owp[0], owp[1], np[0], np[1]));
    // (a chunk holding a rail or an entity of the layout must be entity-ticking (<= 31): a cart or a
    // rider there would freeze; a chunk holding only other cells needs block ticking (<= 32: blocks,
    // scheduled ticks and block entities run, entities frozen); 33 and beyond nothing ticks)
    let mut moving: BTreeSet<[i32; 2]> = BTreeSet::new();
    for p in parts {
        moving.extend(p.blocks.iter().filter(|(_, st)| st.contains("rail")).map(|(c, _)| ch(c[0], c[2])));
        moving.extend(p.entities.iter().map(|(_, e)| ch(e[0].floor() as i32, e[2].floor() as i32)));
    }
    let mut route: Vec<[i32; 2]> = Vec::new();
    for (what, all, at, mv) in [("overworld", &ow_all, owp, &moving), ("Nether", &n_all, np, &BTreeSet::new())] {
        let mut line = Vec::new();
        for c in all {
            let l = 30 + cheb(*c, at);
            let carries = mv.contains(c) || (what == "Nether" && l > 31);
            let bad = l > 32 || (l > 31 && carries);
            if bad {
                ok = false;
            }
            if what == "overworld" && l <= 31 {
                route.push(*c);
            }
            let note = if bad {
                " FAIL"
            } else if l == 32 {
                " (block-ticking: holds no rail and no entity of the layout)"
            } else {
                ""
            };
            line.push(format!("({},{}) {l}{note}", c[0], c[1]));
        }
        s.push_str(&format!("  {what} chunks' levels from the {what} ticket alone (<= 31 entity-ticking, 32 block-ticking): {}\n", line.join(", ")));
    }
    // (the control: loader-long reads the portal chunk + (2, 2); it must hold no farm cell)
    let ctl = [owp[0] + 2, owp[1] + 2];
    if ow_all.contains(&ctl) {
        ok = false;
        s.push_str(&format!("  FAIL: loader-long's control chunk ({},{}) holds farm cells\n", ctl[0], ctl[1]));
    } else {
        s.push_str(&format!("  the control chunk ({},{}) (loader-long's: the portal chunk + (2, 2)): Chebyshev 2, no farm cell; it must unload\n", ctl[0], ctl[1]));
    }
    (s, ok, Some(Chunks { ow_portal: owp, nether_portal: np, control: ctl, ow: route, nether: n_all.into_iter().collect() }))
}

/// `--plant DIM:NAME=x0,y0,z0,x1,y1,z1[;...]` (DIM `overworld` or `nether`): a planted portal.
pub fn parse_plant(spec: &str, framed: bool) -> Result<Plant, String> {
    let (dim, rest) = spec.split_once(':').ok_or(format!("--plant {spec}: DIM:NAME=x0,y0,z0,x1,y1,z1"))?;
    let nether = match dim {
        "overworld" => false,
        "nether" => true,
        _ => return Err(format!("--plant {spec}: DIM is overworld or nether")),
    };
    let p = crate::portallinks::parse(rest)?;
    if p.cells.is_empty() {
        return Err(format!("--plant {spec}: no cell"));
    }
    plane_axis(&p.cells)?;
    Ok(Plant { name: format!("planted:{}", p.name), nether, cells: p.cells, framed })
}

/// The exit blocks whose pick moved between two evaluations (over the verdict legs).
fn moved(a: &Eval, b: &Eval) -> usize {
    a.legs.iter().zip(&b.legs).map(|(x, y)| x.picked.iter().filter(|(t, v)| y.picked.get(*t).is_none_or(|w| w.0 != v.0)).count()).sum()
}

/// The plants read back and judged: Err for a refused run (a no-op), Ok((text, verdict with the plants)).
fn plant_report(parts: &[Laid], plants: &[Plant], base: &Eval, golem_margin: f64) -> Result<(String, bool), String> {
    let mut s = String::new();
    let (ow, _) = built(parts, false, plants)?;
    let (nw, _) = built(parts, true, plants)?;
    for pl in plants {
        let m = if pl.nether { &nw } else { &ow };
        let laid = pl.laid()?;
        let held = laid.iter().filter(|(c, st)| m.get(c).is_some_and(|b| b.1 == usize::MAX && &b.0 == st)).count();
        s.push_str(&format!(
            "planted {} ({}, {}): read back {held} of {} cells in the built layout: portal {}\n",
            pl.name,
            if pl.nether { "Nether" } else { "overworld" },
            if pl.framed { "framed" } else { "UNFRAMED" },
            laid.len(),
            runs_text(&pl.cells)
        ));
        if held != laid.len() {
            return Err(format!("the planted portal {}: only {held} of {} cells read back", pl.name, laid.len()));
        }
    }
    let pv = evaluate(parts, plants, golem_margin)?;
    let k = moved(base, &pv);
    let names = plants.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ");
    if k == 0 {
        // (the broken-frame skip's control: the same cells framed must move picks, else it shows nothing)
        if plants.iter().all(|p| !p.framed) {
            let framed: Vec<Plant> = plants.iter().map(|p| Plant { framed: true, ..p.clone() }).collect();
            let fv = evaluate(parts, &framed, golem_margin)?;
            let kf = moved(base, &fv);
            if kf == 0 {
                return Err(format!("the unframed plant(s) {names} move no pick and neither do the same cells framed: a no-op, refused"));
            }
            s.push_str(&format!("control: the unframed plant(s) {names} move no pick: skipped as the game skips a record whose block lost HORIZONTAL_AXIS; the same cells framed move {kf} exit blocks' picks\n"));
            s.push_str(&format!("== WITH the unframed plant(s)\n{}", pv.text));
            s.push_str(&format!("== WITH the same cells framed (the control's twin)\n{}", fv.text));
            return Ok((s, pv.ok));
        }
        return Err(format!("the planted portal(s) {names} move no exit block's pick on any leg: a no-op, refused"));
    }
    s.push_str(&format!("== WITH the planted portal(s): {k} exit blocks' picks moved\n{}", pv.text));
    s.push_str(&format!("verdict WITH the planted portal(s): {}\n", if pv.ok { "PASS" } else { "FAIL" }));
    Ok((s, pv.ok))
}

/// The whole report; Err for a refused run (a no-op plant), Ok((text, pass)).
pub fn report(parts: &[Laid], plants: &[Plant], golem_margin: f64) -> Result<(String, bool), String> {
    // (G1 00:46 (1): a margin that shrinks or voids the golem's population is refused)
    if !golem_margin.is_finite() || golem_margin < 0.0 {
        return Err(format!("--golem-margin {golem_margin}: a finite margin >= 0 (a golem: 0.7)"));
    }
    // (G1 00:46: two plants of one name would read back as one)
    let mut names = BTreeSet::new();
    for p in plants {
        if !names.insert(p.name.clone()) {
            return Err(format!("two planted portals named {}: refused", p.name));
        }
    }
    let mut s = String::from(
        "loader-layout (loaderlay; bench, tool unreviewed): the parts laid as loader-long lays them (mobai freeze::recipe_part); the portal search as tick-exact pinned it from 26.3 (22:07: NetherPortalBlock.getPortalDestination 130-142, PortalForcer.findClosestPortalPosition 41-50, PoiManager.getInSquare 87-95): the exit block BlockPos.containing(x * s, y, z * s) of the entity's position when it crosses (s 1/8 into the Nether, 8 out), every lit portal cell within 16 (into the Nether) or 128 (into the overworld) in x and z at any y, the least squared distance between block coordinates, then the lowest y; nothing in range = a new portal (a failure)\n",
    );
    for p in parts {
        s.push_str(&format!("part {}: {} + ({},{},{}), Nether image ({},{},{}), Nether y base {}; {} overworld cells, {} Nether cells, {} entities\n", p.name, p.file, p.offset[0], p.offset[1], p.offset[2], p.image[0], p.image[1], p.image[2], p.ybase, p.blocks.len(), p.nether.len(), p.entities.len()));
    }
    let ev = evaluate(parts, &[], golem_margin)?;
    if !plants.is_empty() {
        s.push_str(&format!("== WITHOUT the planted portal(s) (the layout as written): {}\n", if ev.ok { "PASS" } else { "FAIL" }));
        let (t, ok) = plant_report(parts, plants, &ev, golem_margin)?;
        s.push_str(&t);
        return Ok((s, ok));
    }
    let mut ok = ev.ok;
    s.push_str(&ev.text);
    s.push_str("chunks:\n");
    let (ct, cok, got) = chunks_text(parts, &ev);
    s.push_str(&ct);
    ok &= cok;
    // (ironspec 01:17 (1b), G1 01:22: the lightning-rod coverage across the tiled copies; the rods'
    // load status from the loader's overworld PORTAL chunk, when its legs pass)
    let portal = if ev.legs.iter().take(2).all(|l| l.pass()) && ev.ow_portal_chunks.len() == 1 { ev.ow_portal_chunks.iter().next().copied() } else { None };
    let (rt, rok) = crate::loaderrods::rods_text(parts, portal)?;
    s.push_str(&rt);
    ok &= rok;
    // (ironspec 21:54 / 21:55: the loader's portal cells and full frames, world and farm frame, and its
    // farm-frame offset, for vcompose's site record)
    let first = parts.iter().find(|p| p.name != "loader").map(|p| p.offset).unwrap_or([0, 0, 0]);
    if let Some(l) = parts.iter().position(|p| p.name == "loader") {
        let lp = &parts[l];
        s.push_str(&format!(
            "the loader in the farm frame (world less the first copy's offset ({},{},{})): offset ({},{},{}); its Nether part at the image ({},{},{}), y base {}\n",
            first[0], first[1], first[2], lp.offset[0] - first[0], lp.offset[1] - first[1], lp.offset[2] - first[2], lp.image[0], lp.image[1], lp.image[2], lp.ybase
        ));
        let (ow, _) = built(parts, false, &[])?;
        let (nw, _) = built(parts, true, &[])?;
        for p in portals(parts, &[])?.iter().filter(|p| p.part == l) {
            let m = if p.nether { &nw } else { &ow };
            let frame = p.frame(m);
            let farm = |cs: &[C]| -> Vec<C> { cs.iter().map(|c| if p.nether { *c } else { [c[0] - first[0], c[1] - first[1], c[2] - first[2]] }).collect() };
            s.push_str(&format!(
                "  {} portal cells (world{}): {}\n    farm frame: {}\n    obsidian frame (world): {}\n    farm frame: {}\n",
                p.name,
                if p.nether { ", the Nether's own coordinates" } else { "" },
                runs_text(&p.cells),
                runs_text(&farm(&p.cells)),
                runs_text(&frame),
                runs_text(&farm(&frame))
            ));
        }
    }
    if let Some(c) = got {
        let copies: Vec<&Laid> = parts.iter().filter(|p| p.name != "loader").collect();
        let pairs = |v: &[[i32; 2]]| v.iter().map(|c| format!("{},{}", c[0], c[1])).collect::<Vec<_>>().join(";");
        if let (Some(c0), Some(lp)) = (copies.first(), parts.iter().find(|p| p.name == "loader")) {
            s.push_str(&format!(
                "loader-long: mctas mobai loader-long --module {} --module-offsets \"{}\" --module-nether-ybase {} --loader {} --loader-offset {},{},{} --loader-nether-ybase {} --chunks \"{}\" --nchunks \"{}\" --portal {},{} --nportal {},{} --farm-afk <x,y,z> --nether-afk <x,y,z> (control chunk ({},{}))\n",
                c0.file,
                copies.iter().map(|c| format!("{},{},{}", c.offset[0], c.offset[1], c.offset[2])).collect::<Vec<_>>().join(";"),
                c0.ybase,
                lp.file,
                lp.offset[0],
                lp.offset[1],
                lp.offset[2],
                lp.ybase,
                pairs(&c.ow),
                pairs(&c.nether),
                c.ow_portal[0],
                c.ow_portal[1],
                c.nether_portal[0],
                c.nether_portal[1],
                c.control[0],
                c.control[1]
            ));
        }
    }
    s.push_str(&format!("verdict: {}\n", if ok { "PASS (every link lands on its partner, no clash, no broken frame, every chunk holding a rail or an entity at <= 31 and every other at <= 32 from the loader's tickets, the control chunk free; lightning: every strike-area column goes to a credited rod, every place clear of every credited rod's bolt box, no rod's box on the loader, every credited rod's chunk loaded)" } else { "FAIL" }));
    Ok((s, ok))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic loader: an overworld 5 x 5 portal (axis x, cells x 0..4 y 10..14 z 0) with its firing
    /// dropper at (1,14,1) facing north, the same twin in its Nether part with its dropper at (3,14,-1)
    /// facing south (the loader of record's columns), anchored at `anchor_x`.
    fn loader(anchor_x: i32) -> Design {
        let mut d = Design::default();
        d.ports.insert("nether-anchor".into(), [anchor_x, 0, 0]);
        for x in 0..5 {
            for y in 10..15 {
                d.blocks.insert([x, y, 0], "minecraft:nether_portal[axis=x]".into());
                d.nether_blocks.insert([x, y, 0], "minecraft:nether_portal[axis=x]".into());
            }
            for y in [9, 15] {
                d.blocks.insert([x, y, 0], "minecraft:obsidian".into());
                d.nether_blocks.insert([x, y, 0], "minecraft:obsidian".into());
            }
        }
        for y in 10..15 {
            for x in [-1, 5] {
                d.blocks.insert([x, y, 0], "minecraft:obsidian".into());
                d.nether_blocks.insert([x, y, 0], "minecraft:obsidian".into());
            }
        }
        d.blocks.insert([1, 14, 1], "minecraft:dropper[facing=north,triggered=false]".into());
        d.nether_blocks.insert([3, 14, -1], "minecraft:dropper[facing=south,triggered=false]".into());
        d
    }

    /// A synthetic module: one pad (axis x, x 0..3 y 0..2 z 0) and its 2 x 3 twin (nether cells x 1..2 y
    /// 65..67 z 0, y base 64), anchored at (0,0,0), both framed.
    fn module() -> Design {
        let mut d = Design::default();
        d.ports.insert("nether-anchor".into(), [0, 0, 0]);
        for x in 0..4 {
            for y in 0..3 {
                d.blocks.insert([x, y, 0], "minecraft:nether_portal[axis=x]".into());
            }
            d.blocks.insert([x, -1, 0], "minecraft:obsidian".into());
            d.blocks.insert([x, 3, 0], "minecraft:obsidian".into());
        }
        for y in 0..3 {
            d.blocks.insert([-1, y, 0], "minecraft:obsidian".into());
            d.blocks.insert([4, y, 0], "minecraft:obsidian".into());
        }
        for y in 65..68 {
            for x in 1..3 {
                d.nether_blocks.insert([x, y, 0], "minecraft:nether_portal[axis=x]".into());
            }
            d.nether_blocks.insert([0, y, 0], "minecraft:obsidian".into());
            d.nether_blocks.insert([3, y, 0], "minecraft:obsidian".into());
        }
        for x in 1..3 {
            d.nether_blocks.insert([x, 64, 0], "minecraft:obsidian".into());
            d.nether_blocks.insert([x, 68, 0], "minecraft:obsidian".into());
        }
        d
    }

    fn layout(anchor_x: i32, loader_at: C, copies: &[C]) -> Vec<Laid> {
        let m = module();
        let mut v: Vec<Laid> = copies.iter().enumerate().map(|(k, o)| lay(&format!("copy{}", k + 1), "m", &m, *o, 64).unwrap()).collect();
        v.push(lay("loader", "l", &loader(anchor_x), loader_at, 0).unwrap());
        v
    }

    #[test]
    fn the_crossing_box_matches_the_code_bound() {
        // (a dropper at (3,14,-1) facing south: the item's centre after one move x 3.5 +- 0.103365, z
        // 0.2 + 0.0966..0.4034 into the portal's cell)
        let (lo, hi) = crossing_box([3, 14, -1], [0, 0, 1], 1);
        assert!((lo[0] - (3.5 - SPREAD)).abs() < 1e-9 && (hi[0] - (3.5 + SPREAD)).abs() < 1e-9);
        assert!((lo[2] - (0.2 + 0.2 - SPREAD)).abs() < 1e-9 && (hi[2] - (0.2 + 0.3 + SPREAD)).abs() < 1e-9);
        assert!(lo[1] > 14.4 && hi[1] < 14.61, "feet y {:.4}..{:.4}", lo[1], hi[1]);
        // (two moves widen it: the union searched)
        let (l2, h2) = crossing_box([3, 14, -1], [0, 0, 1], 2);
        assert!(l2[2] <= lo[2] && h2[2] > hi[2] && h2[0] > hi[0]);
    }

    #[test]
    fn the_unshifted_twin_returns_to_a_nearer_pad_and_fails_by_name() {
        // (the 16:41 geometry: the twin at the anchor's /8 image, its firing column 3, so the return lands
        // 24..28 east of the overworld portal's west end: the loader's portal x 17..21 y 90..94 z 19, its
        // twin x 2..6 at z 2, the return's exit blocks x 42..45 y 94 z 18..23; the copy's pad x 38..41 y
        // 92..94 z 24 takes it; its twin at z 3, clear of the loader's)
        let parts = layout(0, [17, 80, 19], &[[38, 92, 24]]);
        let (rep, ok) = report(&parts, &[], GOLEM_MARGIN).unwrap();
        assert!(!ok, "{rep}");
        assert!(rep.contains("FAIL (expected loader.overworld") && rep.contains("-> copy1.pad1"), "{rep}");
        assert!(rep.contains("cells two parts claim (Nether): 0\n"), "{rep}");
    }

    #[test]
    fn the_shifted_twin_lands_both_legs_on_the_loader() {
        // (the fix: the anchor 24 west, the portal's west x mod 8 = 1, its z mod 8 = 3)
        let parts = layout(-24, [25, 80, 19], &[[38, 92, 24]]);
        let ev = evaluate(&parts, &[], GOLEM_MARGIN).unwrap();
        assert!(ev.legs[0].pass() && ev.legs[1].pass(), "{}", ev.text);
        // (one move: the return's exit blocks z 18..20 against the portal's z 19; two: up to z 23)
        assert!(ev.info[1].worst_own <= 1.0 + 1e-9, "the return lands in the portal (dz <= 1): {}", ev.text);
        assert!(ev.legs[1].worst_own <= 4.0 + 1e-9, "{}", ev.text);
        assert!(ev.legs[0].worst_own < 1e-9 && ev.info[0].worst_own < 1e-9, "the forward lands in the twin: {}", ev.text);
        // (the arrival: the return's item comes out of the overworld portal at its column 3.5 +- 0.103365,
        // the Nether side's firing column, as on the bench)
        assert!(ev.text.contains("it comes out of loader.overworld at x 28.397..28.603") && ev.text.contains("column 3.397..3.603 of 5"), "{}", ev.text);
        // (the exclusion zone printed for the build guide)
        assert!(ev.text.contains("exclusion zone (the overworld"), "{}", ev.text);
    }

    #[test]
    fn a_planted_nearer_portal_flips_the_verdict_and_a_far_one_refuses() {
        let parts = layout(-24, [25, 80, 19], &[[38, 92, 24]]);
        // (the return's exit blocks are x 26..29 y 94 z 18..23: a planted overworld portal there wins)
        let near = vec![parse_plant("overworld:near=27,94,21,28,94,21", true).unwrap()];
        let (rep, ok) = report(&parts, &near, GOLEM_MARGIN).unwrap();
        assert!(!ok && rep.contains("read back 8 of 8") && rep.contains("planted:near"), "{rep}");
        let far = vec![parse_plant("overworld:far=500,10,500,501,12,500", true).unwrap()];
        let e = report(&parts, &far, GOLEM_MARGIN).unwrap_err();
        assert!(e.contains("planted:far") && e.contains("no-op"), "{e}");
    }

    #[test]
    fn an_unframed_plant_is_skipped_and_its_framed_twin_is_the_control() {
        // (G1 22:10: a record whose block lost HORIZONTAL_AXIS is skipped: the same cells unframed move
        // nothing, framed they take the return)
        let parts = layout(-24, [25, 80, 19], &[[38, 92, 24]]);
        let bare = vec![parse_plant("overworld:bare=27,94,21,28,94,21", false).unwrap()];
        let (rep, ok) = report(&parts, &bare, GOLEM_MARGIN).unwrap();
        assert!(rep.contains("UNFRAMED") && rep.contains("skipped as the game skips") && rep.contains("framed move"), "{rep}");
        assert_eq!(ok, ev_pass_legs(&parts), "the skipped plant leaves the verdict as written: {rep}");
        // (unframed and far: neither moves a pick: refused)
        let far = vec![parse_plant("overworld:barefar=500,10,500,501,12,500", false).unwrap()];
        assert!(report(&parts, &far, GOLEM_MARGIN).unwrap_err().contains("no-op"));
    }

    fn ev_pass_legs(parts: &[Laid]) -> bool {
        evaluate(parts, &[], GOLEM_MARGIN).unwrap().ok
    }

    #[test]
    fn equal_distances_go_to_the_lower_y_and_a_true_tie_fails() {
        // (G1 22:10's tie control: two portals at the same squared distance, different y: the lower wins;
        // the same y: a tie, a failure)
        let a = Portal { name: "a".into(), part: 0, nether: false, cells: vec![[0, 1, 0]], axis: 'x' };
        let b = Portal { name: "b".into(), part: 1, nether: false, cells: vec![[0, -1, 0]], axis: 'x' };
        let leg = run_leg("t", vec![[0, 0, 0]], &[&a, &b], 16, Some("b"));
        assert!(leg.pass() && leg.y_decided == 1 && leg.picked[&[0, 0, 0]].0 == "b", "{leg:?}");
        let c = Portal { name: "c".into(), part: 2, nether: false, cells: vec![[2, 0, 0]], axis: 'x' };
        let d = Portal { name: "d".into(), part: 3, nether: false, cells: vec![[-2, 0, 0]], axis: 'x' };
        let leg = run_leg("t", vec![[0, 0, 0]], &[&c, &d], 16, Some("c"));
        assert_eq!(leg.ties, 1);
        assert!(!leg.pass());
        // (beyond the square: nothing, a named failure)
        let leg = run_leg("t", vec![[0, 0, 100]], &[&c], 16, Some("c"));
        assert!(leg.empty == 1 && !leg.pass() && leg_text(&leg, "the overworld").contains("builds a new portal"));
    }

    #[test]
    fn a_clash_breaking_a_frame_fails() {
        // (a later part's block replacing a twin's frame cell: the game breaks that portal)
        let mut parts = layout(-24, [25, 80, 19], &[[0, 90, 0]]);
        parts[1].nether.insert([1, 94, 0], "minecraft:hopper[enabled=true,facing=south]".into());
        let ev = evaluate(&parts, &[], GOLEM_MARGIN).unwrap();
        assert!(!ev.ok && ev.text.contains("claim (Nether): 1 FAIL") && ev.text.contains("the game breaks this portal"), "{}", ev.text);
    }

    #[test]
    fn an_input_off_the_cited_commit_or_outside_the_worktree_is_refused() {
        // (G1 06:37 (6): a scratch repository outside the worktree; the tested file in it and one outside)
        let dir = std::env::temp_dir().join(format!("loaderlay-cited-input-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("d")).unwrap();
        let git = |args: &[&str]| assert!(std::process::Command::new("git").arg("-C").arg(&dir).args(args).output().unwrap().status.success(), "git {args:?}");
        git(&["init", "-q"]);
        std::fs::write(dir.join("d/m.txt"), "one\n").unwrap();
        git(&["add", "d/m.txt"]);
        git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "m"]);
        let ok = cited_input_in(&dir, std::path::Path::new("d/m.txt"), "HEAD").unwrap();
        assert!(ok.starts_with("d/m.txt "), "{ok}");
        // (an edit: not the cited commit's blob)
        std::fs::write(dir.join("d/m.txt"), "two\n").unwrap();
        assert!(cited_input_in(&dir, std::path::Path::new("d/m.txt"), "HEAD").unwrap_err().contains("not the cited commit HEAD's"));
        // (an untracked file: not in the commit)
        std::fs::write(dir.join("d/new.txt"), "x\n").unwrap();
        assert!(cited_input_in(&dir, std::path::Path::new("d/new.txt"), "HEAD").unwrap_err().contains("is not in the cited commit"));
        // (a file outside the worktree)
        let out = std::env::temp_dir().join(format!("loaderlay-cited-outside-{}.txt", std::process::id()));
        std::fs::write(&out, "x\n").unwrap();
        assert!(cited_input_in(&dir, &out, "HEAD").unwrap_err().contains("lies outside the worktree"));
        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_composed_layout_holds_every_parts_cells_and_reads_back() {
        let parts = layout(-24, [25, 80, 19], &[[38, 92, 24], [70, 92, 24]]);
        let d = compose(&parts).unwrap();
        let (ow, _) = built(&parts, false, &[]).unwrap();
        assert_eq!(d.blocks.len(), ow.len());
        assert!(parts.iter().all(|p| p.blocks.keys().all(|c| d.blocks.contains_key(c))));
        let back = Design::parse(&d.to_text()).unwrap();
        assert_eq!(back.blocks, d.blocks);
    }

    #[test]
    fn zero_samples_never_pass() {
        // (G1 00:46 (1): an empty target set fails; a copy with no pad fails NOT JUDGED; a margin that
        // voids the golem population is refused)
        let a = Portal { name: "a".into(), part: 0, nether: false, cells: vec![[0, 0, 0]], axis: 'x' };
        assert!(!run_leg("t", Vec::new(), &[&a], 16, Some("a")).pass());
        let mut parts = layout(-24, [25, 80, 19], &[[38, 92, 24]]);
        parts[0].blocks.retain(|_, s| !s.contains("nether_portal"));
        let ev = evaluate(&parts, &[], GOLEM_MARGIN).unwrap();
        assert!(!ev.ok && ev.text.contains("FAIL: copy1: no pad"), "{}", ev.text);
        let ok_parts = layout(-24, [25, 80, 19], &[[38, 92, 24]]);
        for m in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(report(&ok_parts, &[], m).unwrap_err().contains("--golem-margin"), "margin {m}");
        }
        // (a vertical dropper into a portal is refused, never ignored)
        let cells: BTreeMap<C, String> = [([0, 0, 0], "minecraft:dropper[facing=up,triggered=false]".to_string())].into_iter().collect();
        assert!(firing_droppers(&cells, &[[0, 1, 0]]).unwrap_err().contains("vertical"));
        // (two plants of one name are refused)
        let p = parse_plant("overworld:x=27,96,23,28,97,23", true).unwrap();
        assert!(report(&ok_parts, &[p.clone(), p], GOLEM_MARGIN).unwrap_err().contains("two planted portals"));
    }

    /// The synthetic layout whose loader legs pass, its chunk report.
    fn chunk_report(parts: &[Laid]) -> (String, bool) {
        let ev = evaluate(parts, &[], GOLEM_MARGIN).unwrap();
        assert!(ev.legs[0].pass() && ev.legs[1].pass(), "{}", ev.text);
        let (s, ok, _) = chunks_text(parts, &ev);
        (s, ok)
    }

    #[test]
    fn the_chunk_bars_fail_on_their_negatives() {
        // (G1 00:46 (2): the loader's tickets at overworld (1,1) and Nether (0,0); the layout as laid
        // passes its chunk bars, each planted cell below fails them)
        let base = layout(-24, [25, 80, 19], &[[38, 92, 24]]);
        let (s, ok) = chunk_report(&base);
        assert!(ok && s.contains("overworld chunk (1,1)") && s.contains("the control chunk (3,3)"), "{s}");
        // (a rail at level 32: chunk (3,1), Chebyshev 2)
        let mut p = base.clone();
        p[0].blocks.insert([60, 80, 20], "minecraft:rail[shape=north_south,waterlogged=false]".into());
        let (s, ok) = chunk_report(&p);
        assert!(!ok && s.contains("(3,1) 32 FAIL"), "{s}");
        // (an entity at level 32)
        let mut p = base.clone();
        p[0].entities.push(("minecraft:minecart".into(), [60.5, 80.0, 20.5]));
        let (s, ok) = chunk_report(&p);
        assert!(!ok && s.contains("(3,1) 32 FAIL"), "{s}");
        // (a static cell at level 32 holds; at 33 it fails)
        let mut p = base.clone();
        p[0].blocks.insert([60, 80, 20], "minecraft:glass".into());
        let (s, ok) = chunk_report(&p);
        assert!(ok && s.contains("(3,1) 32 (block-ticking"), "{s}");
        p[0].blocks.insert([64, 80, 20], "minecraft:glass".into());
        let (s, ok) = chunk_report(&p);
        assert!(!ok && s.contains("(4,1) 33 FAIL"), "{s}");
        // (a farm cell in the control chunk (3,3))
        let mut p = base.clone();
        p[0].blocks.insert([50, 80, 50], "minecraft:glass".into());
        let (s, ok) = chunk_report(&p);
        assert!(!ok && s.contains("control chunk (3,3) holds farm cells"), "{s}");
        // (picks spanning two chunks: the ticket moves with the item)
        let mut ev = evaluate(&base, &[], GOLEM_MARGIN).unwrap();
        ev.ow_portal_chunks.insert([2, 1]);
        let (s, ok, _) = chunks_text(&base, &ev);
        assert!(!ok && s.contains("span chunks"), "{s}");
        // (a loader leg failing: the levels are NOT JUDGED, and the tickets are where the picks went)
        let bad = layout(0, [17, 80, 19], &[[38, 92, 24]]);
        let ev = evaluate(&bad, &[], GOLEM_MARGIN).unwrap();
        let (s, ok, got) = chunks_text(&bad, &ev);
        assert!(!ok && got.is_none() && s.contains("NOT JUDGED") && s.contains("[2, 1]"), "{s}");
    }
}
