//! The siting cost of a mob-farm module from its envelope alone (ironspec 2026-11-11, vjeux's
//! compact one-bed plan: each layout family's minimum envelope priced from the rules before any part
//! is drawn; farmopt's envelope tool reads its rows). A module is a box `w` x `d` x `h` holding
//! `blocks` blocks, its lowest ranged target (a villager position or a golem cell) `low` layers over
//! its floor, its own dark floors `floors`; placed in the sky or on the surface fully spawn-proofed,
//! priced at a stage's prices (designs/farms/prices.tsv). The rules it prices, one row each:
//! - lightning strikes every entity-ticking chunk, a pearl-loaded one included, with no player gate
//!   (tick-exact via ironspec 2026-11-11), so the roof rules hold with the owner away: an opaque
//!   roof over every target outside every strike's box (x and z ±3, y -3..+9 around the strike
//!   block): bottom slabs (no spawn floor, not flammable) over the footprint and 3 around it with
//!   no rod (a strike on the ground beside the module reaches 9 up), over the footprint alone with
//!   one lightning rod on it (every bolt within 128 to the rod, `ServerLevel.findLightningRod`,
//!   which also leaves no skeleton trap: `tickChunk` spawns none on a rod);
//! - sky: every target `range` over the highest natural block within `range` columns (8: the golem
//!   search's floors y+5 to y-7 sideways ±8 reach nothing natural; 16: the golem cells out of a
//!   ground skeleton's bow, 15, the villagers out of a pillager's sensing, 15), the box's bottom
//!   over every natural block under it; the
//!   lift is the air under the module, its scaffold;
//! - surface: the ground within `radius` of the module (16: out of a bow and a patrol's 15) lit to
//!   block light 9 or more (a patrol spawns at 8 or less, a monster at 0): a torch per 61 cells (a
//!   torch's light 14 keeps 9 within 5 blocks by taxicab, a 61-cell diamond; the diamonds tile the
//!   ground); a fence ring at `radius` (no walker from beyond) with one gate; the caves under the
//!   ring are the site's, not counted;
//! - the module's own dark floors spawn-proofed, a bottom slab each (its interior is dark under the
//!   roof, and the open sides let the night's sky light in);
//! - the time signal: a daylight detector on the roof (the sentinel and its room go) and the
//!   controller's night-skip jump detector (a subtract comparator: farmopt 2026-11-11);
//! - the chunk loader: one ender pearl in a stasis cell (a water source, a trapdoor, 2 blocks) per
//!   chunk the module covers (natural spawning needs a player within 128: none while the owner is
//!   away; lightning, random ticks and skeleton traps do not);
//! - height (iteration 16's travel model, `prices::Access::travel`): a trip up and down per 1,728
//!   blocks to the farthest level, by the cheapest way (a pillar broken on the way down, so nothing
//!   is left on the surface; elytra up at late prices); `place:edge` for a block more than 2 above
//!   every standable surface within 3: the envelope's `edge` share of its blocks (v4 C' in the sky:
//!   0.68), the roof all edge, and on the surface the lowest 3 layers never.

use redsim::entity::mob::prices::{Access, Prices};

/// A module's box and what the siting rules read of it.
#[derive(Clone, Debug)]
pub struct Envelope {
    pub w: i32,
    pub d: i32,
    pub h: i32,
    /// The blocks it places (its parts), for the trips and the edge placements.
    pub blocks: u32,
    /// Its own dark floors a monster stands on.
    pub floors: u32,
    /// Its lowest ranged target's level over its floor (0: the floor layer).
    pub low: i32,
    /// Its highest villager position's level over its floor: the roof goes 6 over it (its top no
    /// golem floor in any search band, which also keeps it 3 over every villager box), else one
    /// over the box.
    pub high: i32,
    /// The share of its blocks placed at an edge in the sky.
    pub edge: f64,
    /// The chunks it covers (default: its footprint aligned to the chunk grid).
    pub chunks: Option<u32>,
    /// Its loader: `None`, one ender pearl in a stasis cell per chunk; `Some(n)`, a shared loader's
    /// share of `n` blocks a module (farmopt 2026-11-11: the ring-free plan's L1''' item loop, ~18).
    pub loader_blocks: Option<u32>,
    /// Its other trips, each to its mid-height (a scarer placed, a lava trip; at late prices the
    /// villagers come by the breeder's delivery line, no trip of the player's).
    pub extra_trips: u32,
    /// Its own dark floors lit instead of slabbed: one torch per this many floors (a torch's block
    /// light reaches 13 by taxicab, and a monster spawns only at block light 0; the parts'
    /// geometry decides the share). `None`: a bottom slab on each.
    pub floor_torch: Option<u32>,
    /// The modules in an array sharing the rod (one per 9) and the surface's lit ground and fence
    /// ring (9fc3155f's tiling: mirrored-z pairs 16 apart, pairs 32 apart in x, pair rows 48 apart
    /// in z); the roof is per module. 1: alone.
    pub array: u32,
}

/// Where the module goes.
#[derive(Clone, Copy, Debug)]
pub enum Placement {
    /// Floating: `range` (8 or 16) over the highest natural block within `range` columns, that block
    /// `relief` over the highest natural block under the footprint.
    Sky { range: i32, relief: i32, rod: bool },
    /// On the surface, the ground within `radius` lit and fenced.
    Surface { radius: i32, rod: bool },
}

/// One siting row: what, how many, the seconds, the rule.
#[derive(Clone, Debug)]
pub struct Row {
    pub item: &'static str,
    pub n: u32,
    pub secs: f64,
    pub rule: String,
}

/// A module's siting at a placement: its rows, the air under it (the lift) and its volume.
#[derive(Clone, Debug)]
pub struct Siting {
    pub rows: Vec<Row>,
    pub lift: i32,
    pub volume: i64,
    /// One visit's trip up to the module's top and back down (the collecting trip), seconds.
    pub visit_s: f64,
}

impl Siting {
    pub fn seconds(&self) -> f64 {
        self.rows.iter().map(|r| r.secs).sum()
    }
}

/// The siting rows of envelope `e` at `at`, at `stage`'s prices (0 day-1, 1 middle, 2 late).
pub fn siting(e: &Envelope, at: &Placement, stage: usize) -> Result<Siting, String> {
    let p = Prices::get()?;
    let price = |item: &str| p.price(item, stage);
    let place = price("place")?;
    let edge_s = price("place:edge")?;
    let slab = price("cobblestone_slab")? + place;
    let (rod, sky) = match at {
        Placement::Sky { rod, .. } => (*rod, true),
        Placement::Surface { rod, .. } => (*rod, false),
    };
    let lift = match at {
        Placement::Sky { range, relief, .. } => (relief + range - e.low - 1).max(0),
        Placement::Surface { .. } => 0,
    };
    let mut rows = Vec::new();
    // the roof: its level over the module's floor, one over the box or 6 over the highest villager
    // position (its top no golem floor in any band; 3 over every villager box with it)
    let roof_level = e.h.max(e.high + 6);
    let m = if rod { 0 } else { 3 };
    let roof = ((e.w + 2 * m) * (e.d + 2 * m)) as u32;
    let roof_edge = lift + roof_level > 2;
    rows.push(Row {
        item: "roof",
        n: roof,
        secs: roof as f64 * (slab + if roof_edge { edge_s } else { 0.0 }),
        rule: format!("bottom slabs over {} ({} x {}) at level {roof_level} ({} air layers over the box), opaque, no spawn floor, not flammable, outside every strike box", if rod { "the footprint" } else { "the footprint and 3 around" }, e.w + 2 * m, e.d + 2 * m, roof_level - e.h),
    });
    let n_mod = e.array.max(1);
    if rod {
        let rods = n_mod.div_ceil(9);
        rows.push(Row { item: "lightning rod", n: 1, secs: (price("lightning_rod")? + place + edge_s) * rods as f64 / n_mod as f64, rule: format!("on the roof: every bolt within 128 to it, no skeleton trap ({rods} for the array's {n_mod}: one per 9 modules)") });
    }
    rows.push(Row { item: "time signal", n: 2, secs: price("daylight_detector")? + place + edge_s + price("comparator")? + place, rule: "a daylight detector on the roof and the night-skip jump detector (a subtract comparator)".into() });
    if e.floors > 0 {
        match e.floor_torch {
            Some(k) => {
                let t = e.floors.div_ceil(k.max(1));
                rows.push(Row { item: "own spawn-proofing", n: t, secs: t as f64 * (price("torch")? + place), rule: format!("a torch per {k} of the module's {} dark floors (block light 1 or more: no monster spawns)", e.floors) });
            }
            None => rows.push(Row { item: "own spawn-proofing", n: e.floors, secs: e.floors as f64 * slab, rule: "a bottom slab on each of the module's dark floors".into() }),
        }
    }
    let chunks = e.chunks.unwrap_or(((e.w + 15) / 16 * ((e.d + 15) / 16)) as u32);
    let loader_n = match e.loader_blocks {
        Some(n) => {
            rows.push(Row { item: "loader share", n, secs: n as f64 * (price("filler")? + place), rule: format!("a shared loader's {n} blocks a module (the ring-free plan's item loop)") });
            n
        }
        None => {
            let pearl = price("ender_pearl").unwrap_or(0.0);
            let cell = price("water")? + price("oak_trapdoor")? + place + 2.0 * (price("cobblestone")? + place) + pearl;
            rows.push(Row {
                item: "pearl loader",
                n: chunks,
                secs: chunks as f64 * cell,
                rule: format!("a stasis cell per chunk (a water source, a trapdoor, 2 blocks{})", if pearl > 0.0 { ", the pearl" } else { "; the pearl unpriced: no ender_pearl row in prices.tsv" }),
            });
            chunks * 4
        }
    };
    if let Placement::Surface { radius, .. } = at {
        let r = *radius;
        // the array's box (9fc3155f's tiling), the lit ground and the fence ring round it shared
        let (wb, db) = if n_mod == 1 {
            (e.w, e.d)
        } else {
            let pairs = n_mod.div_ceil(2) as i32;
            let cols = (pairs as f64).sqrt().ceil() as i32;
            let rws = (pairs + cols - 1) / cols;
            (cols * e.w + (cols - 1) * 16, rws * (2 * e.d + 16))
        };
        let ring = ((wb + 2 * r) * (db + 2 * r) - n_mod as i32 * e.w * e.d) as u32;
        let torches = ring.div_ceil(61);
        rows.push(Row { item: "ground light", n: torches, secs: torches as f64 * (price("torch")? + place) / n_mod as f64, rule: format!("{ring} ground cells within {r} of the array's {wb} x {db} lit to block light 9 or more, a torch per 61 (a module's share of {n_mod})") });
        let fence = (2 * (wb + 2 * r) + 2 * (db + 2 * r)) as u32;
        rows.push(Row { item: "fence ring", n: fence, secs: fence as f64 * (price("oak_fence")? + place) / n_mod as f64, rule: format!("a fence ring {r} out round the array, one of its cells a gate (a module's share of {n_mod})") });
    }
    // height: the trips to the farthest level (the roof) and the edge placements
    let access = Access { ladder: false, stairs: false, bubble: false, drop_landing: false };
    let levels = (lift + roof_level - 2).max(0) as u32;
    let ((ur, us), (dr, ds)) = access.travel(levels, stage)?;
    let total = e.blocks + roof + 2 + loader_n;
    let trips = total.div_ceil(1728).max(1);
    rows.push(Row { item: "trips", n: trips, secs: trips as f64 * (us + ds), rule: format!("{trips} up and down to {levels} levels over reach ({ur} {us:.1} s, {dr} {ds:.1} s: nothing left on the surface)") });
    if e.extra_trips > 0 {
        let mid = (lift + e.h / 2 - 2).max(0) as u32;
        let ((mur, mus), (mdr, mds)) = access.travel(mid, stage)?;
        rows.push(Row { item: "other trips", n: e.extra_trips, secs: e.extra_trips as f64 * (mus + mds), rule: format!("{} up and down to its mid-height, {mid} levels over reach ({mur} {mus:.1} s, {mdr} {mds:.1} s)", e.extra_trips) });
    }
    let edges = if sky { (e.blocks as f64 * e.edge).round() } else { (e.blocks as f64 * e.edge * ((e.h - 3).max(0) as f64 / e.h.max(1) as f64)).round() } as u32;
    rows.push(Row { item: "edge placements", n: edges, secs: edges as f64 * edge_s, rule: format!("the module's own blocks more than 2 over every standable surface ({} of {})", if sky { "its edge share" } else { "its edge share above the lowest 3 layers" }, e.blocks) });
    let volume = ((e.w + 2 * m) as i64) * ((e.d + 2 * m) as i64) * ((lift + roof_level + 1) as i64);
    Ok(Siting { rows, lift, volume, visit_s: us + ds })
}

/// The rows as text: one per line, then the total.
pub fn render(e: &Envelope, at: &Placement, s: &Siting) -> String {
    let mut out = format!(
        "envelope {} x {} x {} ({} blocks, {} own floors, lowest target {} over its floor), {}: lift {} (the air under it), volume {} with its roof; loader: {}\n",
        e.w,
        e.d,
        e.h,
        e.blocks,
        e.floors,
        e.low,
        match at {
            Placement::Sky { range, relief, rod } => format!("sky (range {range}, relief {relief}{})", if *rod { ", a rod" } else { ", no rod: the roof's top is a strike surface, a skeleton trap's start (site-check's strike cells take it)" }),
            Placement::Surface { radius, rod } => format!("surface (radius {radius}{})", if *rod { ", a rod" } else { ", no rod" }),
        },
        s.lift,
        s.volume,
        match e.loader_blocks {
            Some(n) => format!("the item loop's share, {n} blocks (the lead)"),
            None => "a pearl stasis cell per chunk (the alternative; --loader-blocks N for the item loop, the lead)".to_string(),
        }
    );
    for r in &s.rows {
        out.push_str(&format!("  {:<20} {:>6} {:>9.1} s  {}\n", r.item, r.n, r.secs, r.rule));
    }
    out.push_str(&format!("siting {:.1} minutes; a visit's trip {:.1} s\n", s.seconds() / 60.0, s.visit_s));
    out
}
