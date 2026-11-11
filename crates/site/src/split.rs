//! A listed iron farm's parts when its designer wrote no parts file (the parent 2026-11-08 20:59:
//! every farm page lists its parts, each with its own numbers). The farm optimizer's builder knows
//! which part placed each block of its own farms (`<stem>.parts.tsv`, farmopt-iron's split); a
//! farm built by hand has only its design, so every block (and every cell its terrain must dig) is
//! given the part its place in the farm says, from the design's own landmarks:
//!
//! - the **sleep trigger**: every redstone component (an observer, a repeater, a comparator, dust,
//!   a redstone torch, a pressure plate, a target...), as farmopt's split counts it;
//! - the **storage**: an iron-block crafter or a dropper and the redstone beside it;
//! - the **villager cage**: the cells near a villager or the zombie (2 blocks across), from the
//!   villagers' floor up to 4 above the highest villager;
//! - the **golem spawn area**: the layer the golems spawn on (the level with the most of the farm's
//!   spawn spots below the cage: `redsim::entity::mob::exit::spawn_spots` on the void build, as the
//!   judge finds them), from the block under the floor to the golem's head, 1 block around a spot;
//! - the **collection**: the hoppers and the chests;
//! - the **kill chamber**: the lava and what holds it (a sign, a button, a fence gate), and the
//!   chamber around the lava (2 blocks across, up to 3 above the lava);
//! - the **golem drop**: what lies under the spawn area's columns (1 block around), above the kill;
//! - the **access**: in a farm dug into the ground, every other cell (the stairs, the room by the
//!   chest); in a farm built in the open, the rest is the drop's supports (golem drop).
//!
//! Each part's numbers are farmopt's: its blocks placed (their items at the stage's prices and
//! their placing), the cells its terrain digs (the design's `air` cells of a farm dug into the
//! ground), the villagers moved and the zombie (the cage's), the lava trips (the kill's), and the
//! card's travel and height shared by the blocks placed, so the parts sum to the card.
//! `mctas site parts <design>` prints a design's split, against its parts file when it has one.

use crate::farms::FarmPart;
use anyhow::{Result, anyhow};
use std::collections::{BTreeMap, BTreeSet, HashSet};

type P = [i32; 3];

/// The redstone components the sleep trigger is made of (farmopt's split's list, and a lever and
/// a daylight detector).
const REDSTONE: [&str; 15] = ["observer", "repeater", "comparator", "redstone_wire", "redstone_torch", "redstone_wall_torch", "oak_pressure_plate", "stone_pressure_plate", "target", "redstone_block", "lever", "daylight_detector", "heavy_weighted_pressure_plate", "tripwire_hook", "tripwire"];

fn id(s: &str) -> &str {
    s.trim_start_matches("minecraft:").split('[').next().unwrap_or("")
}

fn cell(p: [f64; 3]) -> P {
    [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32]
}

/// Each cell of the design (its blocks and the `air` cells a farm dug into the ground must dig) by
/// its part's folder name (`crate::farms::PARTS`), in farmopt's split's order: the access, the
/// storage, the sleep trigger, the cage, the spawn area, the collection, the kill, the drop.
pub fn roles(d: &mctas::design::Design) -> Result<BTreeMap<P, &'static str>> {
    // (a farm dug into the ground is built in the void with the natural rock around it, `ground:`'s
    // layer and below, so its golems find the spots they find in the ground, not on its bare cells)
    let c = circuit::Circuit::from_design(with_rock(d))?;
    let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow!(e))?;
    use redsim::entity::mob::farm::{RunOpts, WalkMode, build};
    let w = build(&spec, &RunOpts::steady(1, 0, WalkMode::Random)).map_err(|e| anyhow!(e))?;
    // (a farm dug into the ground: only the spots in its own dug cells; a spot on the natural
    // surface or in the rock around is a stray one the site caps, not its spawn area)
    let spots: Vec<P> = redsim::entity::mob::exit::spawn_spots(&w).into_iter().map(|p| [p.x, p.y, p.z]).filter(|p| !d.meta.contains_key("terrain") || d.blocks.contains_key(p)).collect();
    let villagers: Vec<P> = d.entities.iter().filter(|e| e.kind.trim_start_matches("minecraft:") == "villager").map(|e| cell(e.pos)).collect();
    if villagers.is_empty() {
        return Err(anyhow!("no villager in the design"));
    }
    let dug_design = d.meta.contains_key("terrain");
    let floor = villagers.iter().map(|v| v[1]).min().unwrap_or(0) - 1;
    let block = |p: &P| d.blocks.get(p).map(|s| id(s)).unwrap_or("");
    // the cage of each group of villagers (a stack's modules each have theirs): from the group's
    // floor up to 4 above its highest villager
    let groups: Vec<(i32, i32)> = clusters(&villagers, 4)
        .into_iter()
        .flat_map(|g| {
            // (the same columns at other heights: a stack's modules are separate cages)
            let mut ys: Vec<i32> = g.iter().map(|v| v[1]).collect();
            ys.sort();
            let mut bands: Vec<(i32, i32)> = Vec::new();
            for y in ys {
                match bands.last_mut() {
                    Some(b) if y - b.1 <= 3 => b.1 = y,
                    _ => bands.push((y, y)),
                }
            }
            bands.into_iter().map(|(lo, hi)| (lo - 1, hi + 4)).collect::<Vec<_>>()
        })
        .collect();
    // the spawn areas: every level with at least half as many spawn spots as the busiest one below
    // a cage's floor (else anywhere); each group of a level's spots (8-connected, gaps of one cell
    // bridged) gives a rectangle, 1 block around it, from the block under the floor to the golem's
    // head
    let mut levels: BTreeMap<i32, usize> = BTreeMap::new();
    for s in spots.iter().filter(|s| !groups.iter().any(|g| (g.0..=g.1).contains(&s[1]))) {
        *levels.entry(s[1]).or_default() += 1;
    }
    // (a farm whose golems spawn beside the villagers, in pits at their level: not this shape, no
    // split)
    if levels.is_empty() {
        return Err(anyhow!("its golems spawn at the villagers' level: no spawn area apart from the cage"));
    }
    let busiest = levels.values().copied().max().unwrap_or(0);
    let pad_levels: Vec<i32> = levels.iter().filter(|(_, n)| **n * 2 >= busiest && **n >= 4).map(|(y, _)| *y).collect();
    // (y, x0, x1, z0, z1) per spawn area: its spots' rectangle, grown over the floor the spots stop
    // short of (a golem's wide box finds no spot in the row against a wall: the floor goes on)
    let floor_at = |x: i32, y: i32, z: i32| {
        let below = block(&[x, y - 1, z]);
        !below.is_empty() && !matches!(below, "air" | "water" | "lava" | "cave_air") && matches!(block(&[x, y, z]), "" | "air" | "water" | "cave_air")
    };
    let grow = |y: i32, r: (i32, i32, i32, i32)| {
        let mut r = r;
        for _ in 0..2 {
            let row = |z: i32, r: &(i32, i32, i32, i32)| (r.0..=r.1).filter(|x| floor_at(*x, y, z)).count() * 2 > (r.1 - r.0 + 1) as usize;
            let col = |x: i32, r: &(i32, i32, i32, i32)| (r.2..=r.3).filter(|z| floor_at(x, y, *z)).count() * 2 > (r.3 - r.2 + 1) as usize;
            if row(r.2 - 1, &r) {
                r.2 -= 1;
            }
            if row(r.3 + 1, &r) {
                r.3 += 1;
            }
            if col(r.0 - 1, &r) {
                r.0 -= 1;
            }
            if col(r.1 + 1, &r) {
                r.1 += 1;
            }
        }
        r
    };
    let rects: Vec<(i32, (i32, i32, i32, i32))> = pad_levels
        .iter()
        .flat_map(|y| {
            let at: Vec<P> = spots.iter().filter(|s| s[1] == *y).copied().collect();
            clusters(&at, 2).into_iter().map(|g| (*y, grow(*y, bbox(&g)))).collect::<Vec<_>>()
        })
        .collect();
    let over_pad = |p: &P, border: i32| rects.iter().any(|(_, r)| (r.0 - border..=r.1 + border).contains(&p[0]) && (r.2 - border..=r.3 + border).contains(&p[2]));
    let in_pad = |p: &P| rects.iter().any(|(y, r)| (y - 2..=y + 1).contains(&p[1]) && (r.0 - 1..=r.1 + 1).contains(&p[0]) && (r.2 - 1..=r.3 + 1).contains(&p[2]));
    let in_cage = |p: &P| groups.iter().any(|g| (g.0..=g.1).contains(&p[1])) && !over_pad(p, 0);
    // the kill: the floor of the pit the lava burns in (the open cells at the lava's level around
    // it, 4 blocks at most), from the block under the lava to 3 above it
    let lava: Vec<P> = d.blocks.iter().filter(|(_, s)| id(s) == "lava").map(|(p, _)| *p).collect();
    // (a farm dug into the ground: a cell its design leaves out is natural rock)
    let open = |p: &P| match block(p) {
        "" => !dug_design,
        b => matches!(b, "air" | "water" | "lava" | "cave_air"),
    };
    let mut pit: HashSet<P> = HashSet::new();
    for l in &lava {
        // (the open cells beside the lava, at its level and the one under it: a source set in a
        // wall's niche burns golems standing next to it)
        let mut todo = vec![*l];
        for dy in [-1, 0] {
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                let n = [l[0] + dx, l[1] + dy, l[2] + dz];
                if open(&n) {
                    todo.push(n);
                }
            }
        }
        while let Some(c) = todo.pop() {
            if !pit.insert(c) {
                continue;
            }
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = [c[0] + dx, c[1], c[2] + dz];
                if open(&n) && (n[0] - l[0]).abs() <= 4 && (n[2] - l[2]).abs() <= 4 && !pit.contains(&n) {
                    todo.push(n);
                }
            }
        }
    }
    // (the pit's walls and floor too where the design builds them: a farm in the open; a farm dug
    // into the ground has natural rock there)
    let in_kill = |p: &P| (-1..=3).any(|dy| pit.contains(&[p[0], p[1] - dy, p[2]]) || (!open(p) && [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)].iter().any(|(dx, dz)| pit.contains(&[p[0] + dx, p[1] - dy, p[2] + dz]))));
    if pit.is_empty() {
        return Err(anyhow!("no lava: no kill to find"));
    }
    let pits: Vec<(i32, i32, i32, i32, i32, i32)> = clusters(&lava, 3)
        .into_iter()
        .map(|g| {
            let r = bbox(&g);
            (r.0, r.1, r.2, r.3, g.iter().map(|p| p[1]).min().unwrap_or(0), g.iter().map(|p| p[1]).max().unwrap_or(0))
        })
        .collect();
    // the collection: the hoppers by the kill and the chest at the farm's output (and the hoppers
    // and chests touching them); every other container, and the redstone beside one, is storage
    let output: Option<P> = d.ports.get("output").copied();
    let containers: HashSet<P> = d.blocks.iter().filter(|(_, s)| matches!(id(s), "hopper" | "chest" | "trapped_chest" | "barrel" | "crafter" | "dropper")).map(|(p, _)| *p).collect();
    let mut collection: HashSet<P> = containers.iter().filter(|p| block(p) == "hopper" && pits.iter().any(|k| (k.0 - 2..=k.1 + 2).contains(&p[0]) && (k.2 - 2..=k.3 + 2).contains(&p[2]) && (k.4 - 3..=k.5 + 3).contains(&p[1]))).copied().collect();
    if let Some(o) = output.filter(|o| containers.contains(o)) {
        collection.insert(o);
    }
    // (the hoppers and chests that touch the collection, once: a double chest's other half, a
    // second hopper)
    let touching: Vec<P> = containers.iter().filter(|p| !collection.contains(*p) && matches!(block(p), "hopper" | "chest" | "trapped_chest") && collection.iter().any(|q| adjacent(p, q))).copied().collect();
    collection.extend(touching);
    let storage: HashSet<P> = containers.iter().filter(|p| !collection.contains(*p)).copied().collect();
    let beside_storage = |p: &P| storage.iter().any(|q| adjacent(p, q));
    // the access of a farm dug into the ground: the staircase's shaft (the columns, off the spawn
    // area and the pit, dug open through at least half the layers from the chest's level to the
    // top: a spiral leaves one step of rock or a slab in each layer), from its lowest dug cell to
    // its highest, and the passage its foot opens at each level (the dug cells it reaches across,
    // off the spawn area, the pit and the villagers' cells, 6 blocks at most)
    let mut air_layers: BTreeMap<(i32, i32), BTreeSet<i32>> = BTreeMap::new();
    for (p, s) in &d.blocks {
        if id(s) == "air" {
            air_layers.entry((p[0], p[2])).or_default().insert(p[1]);
        }
    }
    let top_y = d.blocks.keys().map(|p| p[1]).max().unwrap_or(0);
    let low_y = output.map(|o| o[1]).unwrap_or_else(|| d.blocks.keys().map(|p| p[1]).min().unwrap_or(0));
    let in_pit_column = |x: i32, z: i32| pit.iter().any(|q| q[0] == x && q[2] == z);
    let stair: HashSet<(i32, i32)> = if dug_design {
        let need = ((top_y - low_y) / 2).max(6) as usize;
        air_layers.iter().filter(|((x, z), ys)| ys.len() >= need && !over_pad(&[*x, 0, *z], 1) && !in_pit_column(*x, *z)).map(|(k, _)| *k).collect()
    } else {
        HashSet::new()
    };
    let villager_cells: HashSet<P> = villagers.iter().flat_map(|v| [*v, [v[0], v[1] + 1, v[2]]]).collect();
    let mut access: HashSet<P> = HashSet::new();
    for (x, z) in &stair {
        let ys = &air_layers[&(*x, *z)];
        let (lo, hi) = (*ys.iter().next().unwrap_or(&0), *ys.iter().next_back().unwrap_or(&0));
        // (and the hatch over its top)
        for y in lo..=hi + 1 {
            access.insert([*x, y, *z]);
        }
    }
    // (from its foot only: its two lowest layers, where it opens onto the chest's room)
    let foot = access.iter().map(|p| p[1]).min().unwrap_or(0);
    let shaft: Vec<P> = access.iter().copied().filter(|p| p[1] <= foot + 1).collect();
    for s in &shaft {
        let mut todo = vec![*s];
        let mut seen: HashSet<P> = HashSet::new();
        while let Some(c) = todo.pop() {
            if !seen.insert(c) {
                continue;
            }
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = [c[0] + dx, c[1], c[2] + dz];
                if block(&n) == "air" && (n[0] - s[0]).abs() <= 6 && (n[2] - s[2]).abs() <= 6 && !over_pad(&n, 1) && !pit.contains(&n) && !villager_cells.contains(&n) && !access.contains(&n) {
                    todo.push(n);
                }
            }
        }
        access.extend(seen);
    }
    let mut out = BTreeMap::new();
    for (pos, s) in &d.blocks {
        let p = *pos;
        let i = id(s);
        let role = if access.contains(&p) && !in_pad(&p) && !in_kill(&p) {
            "access"
        } else if storage.contains(&p) || (REDSTONE.contains(&i) && beside_storage(&p)) {
            "storage"
        } else if REDSTONE.contains(&i) {
            "sleep-trigger"
        } else if in_cage(&p) {
            "cage"
        } else if in_pad(&p) {
            // (its open cells are the golems' fall path, the drop shaft's: farmopt's split agrees)
            if i == "air" { "transport" } else { "spawn-area" }
        } else if collection.contains(&p) {
            "collection"
        } else if i == "lava" || i.ends_with("_wall_sign") || i.ends_with("_button") || i.ends_with("_fence_gate") || in_kill(&p) {
            "kill"
        } else {
            "transport"
        };
        out.insert(p, role);
    }
    Ok(out)
}

/// Whether two cells share a face.
fn adjacent(p: &P, q: &P) -> bool {
    (p[0] - q[0]).abs() + (p[1] - q[1]).abs() + (p[2] - q[2]).abs() == 1
}

/// A farm dug into the ground (`terrain:`) with its natural rock: every cell the design leaves out
/// below its `ground:` layer (the player's feet on the surface: the top block one below), 9 blocks
/// around it (a golem's search reaches 8 from a villager), as stone. Any other design as it is.
fn with_rock(d: &mctas::design::Design) -> mctas::design::Design {
    let Some(ground) = d.meta.get("ground").and_then(|g| g.trim().parse::<i32>().ok()).filter(|_| d.meta.contains_key("terrain")) else {
        return d.clone();
    };
    let Some(lo) = d.blocks.keys().copied().reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])]) else {
        return d.clone();
    };
    let hi = d.blocks.keys().copied().fold(lo, |a, b| [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]);
    let mut out = d.clone();
    for x in lo[0] - 9..=hi[0] + 9 {
        for z in lo[2] - 9..=hi[2] + 9 {
            for y in lo[1] - 2..ground {
                out.blocks.entry([x, y, z]).or_insert_with(|| "minecraft:stone".to_string());
            }
        }
    }
    out
}

/// Cells grouped by nearness (each within `gap` of another in x and z, any height).
fn clusters(cells: &[P], gap: i32) -> Vec<Vec<P>> {
    let mut groups: Vec<Vec<P>> = Vec::new();
    for c in cells {
        let near: Vec<usize> = groups.iter().enumerate().filter(|(_, g)| g.iter().any(|q| (q[0] - c[0]).abs() <= gap && (q[2] - c[2]).abs() <= gap)).map(|(i, _)| i).collect();
        let mut merged = vec![*c];
        for i in near.iter().rev() {
            merged.extend(groups.remove(*i));
        }
        groups.push(merged);
    }
    groups
}

/// A group's x and z extent: (x0, x1, z0, z1).
fn bbox(g: &[P]) -> (i32, i32, i32, i32) {
    let xs = g.iter().map(|p| p[0]);
    let zs = g.iter().map(|p| p[2]);
    (xs.clone().min().unwrap_or(0), xs.max().unwrap_or(0), zs.clone().min().unwrap_or(0), zs.max().unwrap_or(0))
}

/// A design's parts with their numbers at `stage` (0 day-1, 1 middle, 2 late), the card's build
/// seconds (its terrain site's seals, caps and digging included) shared out when given.
pub fn split(text: &str, stage: usize, card_seconds: Option<f64>) -> Result<Vec<FarmPart>> {
    let d = mctas::design::Design::parse(text)?;
    let roles = roles(&d)?;
    let c = circuit::Circuit::from_design(d.clone())?;
    let bill = circuit::mobfarm::site_bill(&c, None, stage).map_err(|e| anyhow!(e))?;
    let prices = redsim::entity::mob::prices::Prices::get().map_err(|e| anyhow!(e))?;
    let (place, dig) = (prices.price("place", stage).unwrap_or(1.0), prices.price("dig", stage).unwrap_or(1.0));
    let dug_design = d.meta.contains_key("terrain");
    let mut placed: BTreeMap<&str, u32> = BTreeMap::new();
    let mut dug: BTreeMap<&str, u32> = BTreeMap::new();
    let mut secs: BTreeMap<&str, f64> = BTreeMap::new();
    for (pos, s) in &d.blocks {
        let r = roles.get(pos).copied().unwrap_or("transport");
        if id(s) == "air" {
            if dug_design {
                *dug.entry(r).or_default() += 1;
                *secs.entry(r).or_default() += dig;
            }
            continue;
        }
        *placed.entry(r).or_default() += 1;
        let item = redsim::entity::mob::prices::item_of(s).and_then(|it| prices.price(&it, stage).ok()).unwrap_or(0.0);
        *secs.entry(r).or_default() += place + item;
    }
    let row = |k: &str| bill.items.get(k).map_or(0.0, |e| e.1);
    let cage_rows: f64 = bill.items.iter().filter(|(k, _)| k.starts_with("villager") || k.as_str() == "zombie").map(|(_, e)| e.1).sum();
    *secs.entry("cage").or_default() += cage_rows;
    *secs.entry("kill").or_default() += row("lava_trip");
    let given: f64 = secs.values().sum();
    let rest = (card_seconds.unwrap_or(bill.seconds) - given).max(0.0);
    let total_placed: u32 = placed.values().sum();
    let mut v = Vec::new();
    for (folder, ..) in crate::farms::PARTS {
        let (np, nd) = (placed.get(folder).copied().unwrap_or(0), dug.get(folder).copied().unwrap_or(0));
        if np == 0 && nd == 0 {
            continue;
        }
        let share = if total_placed > 0 { rest * np as f64 / total_placed as f64 } else { 0.0 };
        v.push(FarmPart { part: folder.to_string(), member: String::new(), blocks: np, dug: nd, build_seconds: secs.get(folder).copied().unwrap_or(0.0) + share });
    }
    Ok(v)
}

/// `mctas site parts`: each design's split (part, blocks placed, dug, build seconds), beside its
/// parts file's rows when it has one (`<stem>.parts.tsv` next to it): the split's check against
/// the farm optimizer's exact parts. `map`: each layer, top first, a letter per cell by its part
/// (capital: a block placed, small: a cell dug, `.`: neither).
pub fn cmd_parts(designs: &[std::path::PathBuf], stage: Option<&str>, map: bool) -> Result<()> {
    for path in designs {
        let text = std::fs::read_to_string(path)?;
        if map {
            let d = mctas::design::Design::parse(&text)?;
            let roles = roles(&d)?;
            let letter = |r: &str| match r {
                "cage" => 'c',
                "sleep-trigger" => 't',
                "spawn-area" => 'p',
                "transport" => 'd',
                "kill" => 'k',
                "collection" => 'o',
                "storage" => 's',
                "access" => 'a',
                _ => '?',
            };
            let lo = d.blocks.keys().copied().reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])]).unwrap_or([0; 3]);
            let hi = d.blocks.keys().copied().fold(lo, |a, b| [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]);
            println!("{}: c cage, t sleep trigger, p spawn area, d drop, k kill, o collection, s storage, a access (x {}..{} across, z {}..{} down)", path.display(), lo[0], hi[0], lo[2], hi[2]);
            for y in (lo[1]..=hi[1]).rev() {
                println!("layer {y}:");
                for z in lo[2]..=hi[2] {
                    let row: String = (lo[0]..=hi[0])
                        .map(|x| match (d.blocks.get(&[x, y, z]), roles.get(&[x, y, z])) {
                            (Some(s), Some(r)) if id(s) == "air" => letter(r),
                            (Some(_), Some(r)) => letter(r).to_ascii_uppercase(),
                            _ => '.',
                        })
                        .collect();
                    println!("  {row}");
                }
            }
            continue;
        }
        let stage_name = stage.map(str::to_string).or_else(|| path.parent().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().to_string())).unwrap_or_else(|| "middle".into());
        let st = redsim::entity::mob::prices::stage_ix(&stage_name);
        let t0 = std::time::Instant::now();
        let parts = split(&text, st, None)?;
        let exact_path = path.with_extension("").with_extension("parts.tsv");
        let exact = std::fs::read_to_string(&exact_path).ok().map(|t| crate::farms::read_farm_parts(&t)).transpose()?;
        println!("{} ({stage_name}, split in {:.2} s){}", path.display(), t0.elapsed().as_secs_f64(), if exact.is_some() { ": split | its parts file" } else { "" });
        let names: BTreeSet<String> = parts.iter().map(|p| p.part.clone()).chain(exact.iter().flatten().map(|p| p.part.clone())).collect();
        for (folder, ..) in crate::farms::PARTS.iter().filter(|p| names.contains(p.0)) {
            let s = parts.iter().find(|p| p.part == *folder);
            let e = exact.as_ref().and_then(|x| x.iter().find(|p| p.part == *folder));
            let f = |p: Option<&FarmPart>| p.map(|p| format!("{:>4} placed {:>4} dug {:>5.0} s", p.blocks, p.dug, p.build_seconds)).unwrap_or_else(|| format!("{:>27}", "-"));
            println!("  {folder:<14} {}{}", f(s), if exact.is_some() { format!(" | {}", f(e)) } else { String::new() });
        }
        if let Some(e) = &exact {
            println!("  cells in the same part (by count): {:.0}%", 100.0 * agreement(&parts, e));
        }
    }
    Ok(())
}

/// A design's blocks placed (every cell but `air`: what farmopt's parts files sum to).
pub fn placed_blocks(text: &str) -> Option<u32> {
    let d = mctas::design::Design::parse(text).ok()?;
    Some(d.blocks.values().filter(|s| id(s) != "air").count() as u32)
}

/// The share of a design's cells (by count, part by part) its split puts in the same part as its
/// parts file (the farm optimizer's exact one).
pub fn agreement(split: &[FarmPart], exact: &[FarmPart]) -> f64 {
    let (mut agree, mut total) = (0u32, 0u32);
    for (folder, ..) in crate::farms::PARTS {
        let s = split.iter().find(|p| p.part == folder).map_or(0, |p| p.blocks + p.dug);
        let e = exact.iter().find(|p| p.part == folder).map_or(0, |p| p.blocks + p.dug);
        agree += s.min(e);
        total += s.max(e);
    }
    if total == 0 { 1.0 } else { agree as f64 / total as f64 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_split_agrees_with_the_optimizer_s_own_parts() {
        // (the farms whose parts the farm optimizer's builder wrote, block by block: the split from
        // their designs alone puts nine cells in ten in the same parts, the side chute's eight)
        let root = format!("{}/../../designs/farms/iron", env!("CARGO_MANIFEST_DIR"));
        for (stem, stage, least) in [
            ("day-1/dust-pulse-out-of-sight", 0, 0.9),
            ("day-1/dust-pulse-dug10", 0, 0.9),
            ("day-1/bed-plate-lava-out-of-sight", 0, 0.88),
            ("day-1/bed-plate-dug12", 0, 0.9),
            ("day-1/bed-plate-side-chute", 0, 0.8),
            ("middle/module-two-watchers", 1, 0.9),
        ] {
            let (Ok(text), Ok(exact)) = (std::fs::read_to_string(format!("{root}/{stem}.txt")), std::fs::read_to_string(format!("{root}/{stem}.parts.tsv"))) else {
                continue;
            };
            let exact = crate::farms::read_farm_parts(&exact).unwrap();
            // (a parts file its design no longer matches, the design changed since — trimmed, say —
            // and its file not yet written again: nothing to check against; the site splits it)
            if exact.iter().map(|p| p.blocks).sum::<u32>() != placed_blocks(&text).unwrap() {
                continue;
            }
            let split = split(&text, stage, None).unwrap();
            let a = agreement(&split, &exact);
            assert!(a >= least, "{stem}: {:.0}% of its cells in the same parts (at least {:.0}%)", a * 100.0, least * 100.0);
        }
    }
}
