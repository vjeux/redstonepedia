//! A farm's parts (page copy: designs/farms/iron/<stage>/<stem>.parts.tsv): every block of a
//! grammar farm by the part that placed it (cage, calm, pad, transport, kill, collection, storage,
//! access), each part's blocks placed, blocks dug and build seconds at the stage's prices. The
//! access and the storage are the farm's cells that change when the build leaves them out; the
//! calm is every redstone component; the cage is everything at the villagers' level and above on
//! the pods' side of the pad; the pad its layers over the pad's square; the kill its lava and what
//! holds it, and the chamber under a blade; the collection the kill's hoppers and chest; the
//! transport the drop and the side chute's arm. Each part's seconds: its blocks' items and their
//! placing, its digging; the card's other rows (the villagers moved and the zombie: the cage's;
//! the lava trips: the kill's; the travel and the height: by each part's share of the blocks
//! placed) so the parts sum to the card.

use crate::cand::Cand;
use crate::parts::{CalmKind, Kill, Place};
use anyhow::Result;
use std::collections::BTreeMap;

const PARTS: [&str; 8] = ["cage", "calm", "pad", "transport", "kill", "collection", "storage", "access"];

/// A farm's site in its terrain sample (the judge's spot: `site::Site::new`, where the farm's page
/// shows it), for a dug-only part of it shown in its ground; None for a void farm.
fn farm_site(d: &mctas::design::Design) -> Result<Option<[i32; 3]>> {
    use redsim::entity::mob::site;
    let meta = |k: &str| d.meta.get(k).map(String::as_str);
    let Some(sample) = site::sample_of(meta("terrain"), meta("sample")).map_err(|e| anyhow::anyhow!(e))? else { return Ok(None) };
    let c = circuit::Circuit::from_design(d.clone())?;
    let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow::anyhow!(e))?;
    let (ground, afk, clear_box) = site::params(&spec, meta("ground"), meta("afk"), meta("terrain")).map_err(|e| anyhow::anyhow!(e))?;
    let s = site::Site::new(sample, &spec, ground, afk, clear_box).map_err(|e| anyhow::anyhow!(e))?;
    Ok(Some(s.offset))
}

fn design_of(c: &Cand, calm_parts: &[crate::calm::Part], without: (bool, bool)) -> Result<mctas::design::Design> {
    crate::design::WITHOUT.with(|w| w.set(without));
    let built = crate::design::build_uncached(c, calm_parts);
    crate::design::WITHOUT.with(|w| w.set((false, false)));
    Ok(mctas::design::Design::parse(&built?.text)?)
}

fn id(s: &str) -> String {
    s.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string()
}

/// Each cell's part.
pub fn roles(c: &Cand, calm_parts: &[crate::calm::Part]) -> Result<(mctas::design::Design, BTreeMap<[i32; 3], &'static str>)> {
    roles_of(c, calm_parts, None)
}

/// Each cell's part in `actual` (a listed design that a later pass changed, `mobfarm trim`), or
/// in the key's own build: the access and the storage are the cells the key's build has only with
/// them, the rest by place and kind as below.
pub fn roles_of(c: &Cand, calm_parts: &[crate::calm::Part], actual: Option<&mctas::design::Design>) -> Result<(mctas::design::Design, BTreeMap<[i32; 3], &'static str>)> {
    let full = design_of(c, calm_parts, (false, false))?;
    let no_access = design_of(c, calm_parts, (true, false))?;
    let no_storage = design_of(c, calm_parts, (false, true))?;
    let access: std::collections::HashSet<[i32; 3]> = full.blocks.iter().filter(|(p, s)| no_access.blocks.get(*p) != Some(*s)).map(|(p, _)| *p).collect();
    let storage: std::collections::HashSet<[i32; 3]> = full.blocks.iter().filter(|(p, s)| no_storage.blocks.get(*p) != Some(*s)).map(|(p, _)| *p).collect();
    let full_build = full;
    let full = actual.cloned().unwrap_or_else(|| full_build.clone());
    // (a listed design changed by a later pass: its access found in it, the 2 x 2 spiral under its
    // fence-gate hatch in the ground layer down to its corridor, and the corridor to the chest)
    let access: std::collections::HashSet<[i32; 3]> = match actual {
        // (fewer than 20 cells: the staircase was refilled, only its hatch is left)
        Some(d) => access_cells(d).filter(|a| a.len() >= 20).unwrap_or_default(),
        None => access,
    };
    let m = &c.module;
    let n = if m.pad.n == 9 { 8 } else { m.pad.n as i32 };
    let fl = m.pad.floor as i32;
    let x0 = crate::parts::pad_x0(&m.cage, &m.pad);
    let (hx, hz) = (x0 + n / 2 - 2, 2 + n / 2 - 2);
    let k = m.kill.k();
    let mut out = BTreeMap::new();
    for (p, s) in &full.blocks {
        let i = id(s);
        let role = if access.contains(p) {
            "access"
        } else if storage.contains(p) {
            "storage"
        } else if matches!(i.as_str(), "observer" | "repeater" | "comparator" | "redstone_wire" | "redstone_torch" | "redstone_wall_torch" | "oak_pressure_plate" | "stone_pressure_plate" | "target" | "redstone_block") {
            "calm"
        } else if p[1] >= -1 && p[2] <= 1 {
            "cage"
        } else if (fl - 1..=fl + 2).contains(&p[1]) && (x0 - 1..=x0 + n).contains(&p[0]) && (1..=n + 2).contains(&p[2]) {
            // (the pad's open cells are the golems' fall path, the drop shaft's, from under the pad
            // to the kill; the pad's blocks (its fence gates, walls, water) stay the pad's:
            // ironspec 2026-11-10)
            if i == "air" { "transport" } else { "pad" }
        } else if matches!(i.as_str(), "hopper" | "chest") {
            "collection"
        } else if matches!(i.as_str(), "lava" | "oak_wall_sign" | "oak_button" | "oak_fence_gate") {
            "kill"
        } else {
            // below the pad: the kill's own cells (a pit's or blade's bottom 4 layers, the blade's
            // chamber north of the shaft, the side chute's last rows), else the transport
            let in_column = (hx..=hx + 3).contains(&p[0]) && (hz..=hz + 3).contains(&p[2]);
            let chamber = (hx..=hx + 3).contains(&p[0]) && (hz - 3..hz).contains(&p[2]);
            match m.kill {
                Kill::Side { .. } => {
                    if p[1] <= k + 2 && p[2] >= hz + 4 {
                        "kill"
                    } else {
                        "transport"
                    }
                }
                // (a pit's kill: its lava's reach, 5 layers up from its floor; the site's split
                // agrees)
                Kill::Pit { .. } if in_column && p[1] <= k + 4 => "kill",
                _ if (in_column || chamber) && p[1] <= k + 3 => "kill",
                _ if chamber => "kill",
                _ => "transport",
            }
        };
        out.insert(*p, role);
    }
    let _ = (CalmKind::Plate, Place::Void);
    Ok((full, out))
}

/// A dug design's built-in access, found in the design itself: the hatch's fence gates (and its
/// top step) in the ground layer give the spiral's 2 x 2 columns; every cell of those columns down
/// to the lowest one the spiral reaches, and the corridor (the cells at its two lowest levels on
/// the straight line from the spiral to the output chest). None: no hatch.
fn access_cells(d: &mctas::design::Design) -> Option<std::collections::HashSet<[i32; 3]>> {
    let ground = d.meta.get("ground").and_then(|g| g.trim().parse::<i32>().ok())?;
    let top: Vec<[i32; 3]> = d.blocks.iter().filter(|(p, s)| p[1] == ground && (s.contains("fence_gate") || s.contains("slab"))).map(|(p, _)| *p).collect();
    if top.is_empty() {
        return None;
    }
    let (x0, x1) = (top.iter().map(|p| p[0]).min()?, top.iter().map(|p| p[0]).max()?);
    let (z0, z1) = (top.iter().map(|p| p[2]).min()?, top.iter().map(|p| p[2]).max()?);
    let in_spiral = |p: &[i32; 3]| (x0..=x1).contains(&p[0]) && (z0..=z1).contains(&p[2]);
    // the spiral: its columns' cells the design lists (air, steps, torches, gates), down to the
    // lowest
    let col: Vec<[i32; 3]> = d.blocks.keys().filter(|p| in_spiral(p) && p[1] <= ground).copied().collect();
    let bottom = col.iter().map(|p| p[1]).min()?;
    let mut out: std::collections::HashSet<[i32; 3]> = col.into_iter().collect();
    // the corridor: toward the chest (the output port), at the spiral's two lowest levels
    if let Some(o) = d.ports.get("output") {
        let (ox, oz) = (o[0], o[2]);
        for (p, s) in &d.blocks {
            if !(bottom..=bottom + 1).contains(&p[1]) || in_spiral(p) || s.contains("chest") || s.contains("hopper") {
                continue;
            }
            let between_x = (x0.min(ox)..=x1.max(ox)).contains(&p[0]);
            let between_z = (z0.min(oz)..=z1.max(oz)).contains(&p[2]);
            let on_line = (between_x && (z0..=z1).contains(&p[2]) && (z0..=z1).contains(&oz)) || (between_z && (x0..=x1).contains(&p[0]) && (x0..=x1).contains(&ox)) || (between_x && p[2] == oz) || (between_z && p[0] == ox);
            if on_line && between_x && between_z {
                out.insert(*p);
            }
        }
    }
    Some(out)
}

/// The parts.tsv text: each part's member (its catalog stem, if any), blocks placed, blocks dug and
/// build seconds at the stage's prices, the card's other rows shared out (see the module's doc).
pub fn parts_tsv(c: &Cand, calm_parts: &[crate::calm::Part], stage: usize, card_seconds: Option<f64>) -> Result<String> {
    parts_tsv_of(c, calm_parts, stage, card_seconds, None)
}

/// `parts_tsv` of `actual` (the listed design as it is, after a trim) rather than the key's build.
pub fn parts_tsv_of(c: &Cand, calm_parts: &[crate::calm::Part], stage: usize, card_seconds: Option<f64>, actual: Option<&mctas::design::Design>) -> Result<String> {
    let (d, roles) = roles_of(c, calm_parts, actual)?;
    let circ = circuit::Circuit::from_design(d.clone())?;
    let bill = circuit::mobfarm::site_bill(&circ, None, stage).map_err(|e| anyhow::anyhow!(e))?;
    let prices = redsim::entity::mob::prices::Prices::get().map_err(|e| anyhow::anyhow!(e))?;
    let (place, dig) = (prices.price("place", stage).unwrap_or(1.0), prices.price("dig", stage).unwrap_or(1.0));
    let mut placed: BTreeMap<&str, u32> = BTreeMap::new();
    let mut dug: BTreeMap<&str, u32> = BTreeMap::new();
    let mut secs: BTreeMap<&str, f64> = BTreeMap::new();
    for (p, s) in &d.blocks {
        let r = roles.get(p).copied().unwrap_or("cage");
        if id(s) == "air" {
            *dug.entry(r).or_default() += u32::from(c.place == Place::Dug);
            *secs.entry(r).or_default() += if c.place == Place::Dug { dig } else { 0.0 };
            continue;
        }
        *placed.entry(r).or_default() += 1;
        let item = redsim::entity::mob::prices::item_of(s).and_then(|it| prices.price(&it, stage).ok()).unwrap_or(0.0);
        // (a bed's two cells are one bed)
        let item = if id(s).ends_with("_bed") && !s.contains("part=head") { 0.0 } else { item };
        // (the items a block entity holds: a filter's stock)
        let held: f64 = d.block_entities.get(p).map_or(0.0, |e| held_items(e).iter().map(|(it, n)| *n as f64 * prices.price(it, stage).unwrap_or(0.0)).sum());
        *secs.entry(r).or_default() += place + item + held;
    }
    // the card's rows no block carries: the villagers and the zombie (the cage), the lava trips
    // (the kill), the travel and the height (by the blocks placed)
    let row = |k: &str| bill.items.get(k).map_or(0.0, |e| e.1);
    let cage_rows: f64 = bill.items.iter().filter(|(k, _)| k.starts_with("villager") || k.as_str() == "zombie").map(|(_, e)| e.1).sum();
    *secs.entry("cage").or_default() += cage_rows;
    *secs.entry("kill").or_default() += row("lava_trip");
    let given: f64 = secs.values().sum();
    // (the card's own total when given: its terrain site's seals, caps and digging included)
    let rest = (card_seconds.unwrap_or(bill.seconds) - given).max(0.0);
    let total_placed: u32 = placed.values().sum();
    let mut out = String::new();
    out.push_str(&format!("# {}: every block by the part that placed it (farmopt split); build seconds at {} prices, the card's travel and height ({:.0} s) shared by the blocks placed\n", crate::cand::key(c), ["day-1", "middle", "late"][stage], rest));
    out.push_str("part\tmember\tblocks\tdug\tbuild_seconds\n");
    for part in PARTS {
        let (np, nd) = (placed.get(part).copied().unwrap_or(0), dug.get(part).copied().unwrap_or(0));
        if np == 0 && nd == 0 {
            continue;
        }
        let share = if total_placed > 0 { rest * np as f64 / total_placed as f64 } else { 0.0 };
        let s = secs.get(part).copied().unwrap_or(0.0) + share;
        out.push_str(&format!("{part}\t{}\t{np}\t{nd}\t{s:.0}\n", member_of(c, part)));
    }
    Ok(out)
}

/// The items a block entity's SNBT holds (`id:"minecraft:X",count:N`), by item id.
fn held_items(snbt: &str) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    let mut rest = snbt;
    while let Some(i) = rest.find("id:\"minecraft:") {
        rest = &rest[i + "id:\"minecraft:".len()..];
        let Some(end) = rest.find('"') else { break };
        let item = rest[..end].to_string();
        rest = &rest[end..];
        let n = rest.find("count:").and_then(|j| rest[j + 6..].split(|ch: char| !ch.is_ascii_digit()).next().and_then(|v| v.parse::<u32>().ok())).unwrap_or(1);
        out.push((item, n));
    }
    out
}

/// The part's member stem in designs/farms/iron/parts/<part>/members.tsv (farmopt's catalogs'
/// naming), when it is one.
pub fn member_of(c: &Cand, part: &str) -> String {
    let m = &c.module;
    match part {
        "cage" if m.cage.calm == CalmKind::Plate || m.cage.calm == CalmKind::Observer => {
            format!("{}-{}-watching{}{}", if m.cage.calm == CalmKind::Plate { "bed-plate" } else { "bed-observer" }, m.cage.scared, if m.cage.blind > 0 { format!("-{}-sleeping", m.cage.blind) } else { String::new() }, if m.cage.regular { "-regular" } else { "" })
        }
        "cage" if m.cage.calm == CalmKind::Pulse => format!("dust-pulse-{}-watching{}", m.cage.scared, if m.cage.regular { "-regular" } else { "" }),
        "pad" if m.pad.n == 9 => "quadrant-holes-8x8".into(),
        "pad" => format!("dry-floor-{0}x{0}", m.pad.n),
        "kill" => match m.kill {
            Kill::Pit { k } => format!("lava-corner-{}-drop", -7 - k as i32),
            Kill::Blade { k } => format!("lava-blade-{}-drop", -7 - k as i32),
            Kill::Side { len, .. } => format!("side-chute-{len}"),
            Kill::Shaft { .. } => String::new(),
        },
        "storage" if c.place == Place::Void && c.family_gen == 3 && c.modules >= 2 => "sorter-crafter".into(),
        "storage" if c.place == Place::Void && c.family_gen == 0 => match c.chain {
            0 => "chest-row".into(),
            _ => "sorter-crafter".into(),
        },
        "access" if c.place == Place::Dug => "spiral-staircase".into(),
        "transport" => match m.kill {
            Kill::Side { len, feet, .. } => format!("side-chute-arm-{len}{}", if feet == 0 { "-shallow".to_string() } else { format!("-d{}", -feet) }),
            k => format!("drop-shaft-{}", m.pad.floor as i32 - k.k() - 1),
        },
        _ => String::new(),
    }
}

/// The transport, storage and access catalogs (page copy's new component pages): each member the
/// blocks its part holds in a reference farm (the split above), its blocks placed, dug and build
/// seconds at its stage's prices (storage: its hours at 480 iron/h), the farms of `used` that use
/// it. Returns one line per part.
pub fn write_members(dir: &std::path::Path, refs: &[(Cand, &str)], used: &[(String, Cand, String)]) -> Result<Vec<String>> {
    let mut by_part: BTreeMap<&str, Vec<(String, String, String, String)>> = BTreeMap::new();
    for (c, stage) in refs {
        let stage_ix = crate::price::stage_ix(stage);
        let calm_parts = crate::calm::parts(stage);
        let (d, roles) = roles(c, &calm_parts)?;
        let prices = redsim::entity::mob::prices::Prices::get().map_err(|e| anyhow::anyhow!(e))?;
        for part in ["transport", "storage", "access", "kill", "cage"] {
            let stem = member_of(c, part);
            if stem.is_empty() || by_part.get(part).is_some_and(|v| v.iter().any(|m| m.0 == stem)) {
                continue;
            }
            let mut out = mctas::design::Design::default();
            out.name = stem.clone();
            let (mut placed, mut dug, mut secs) = (0u32, 0u32, 0.0f64);
            for (p, s) in &d.blocks {
                if roles.get(p).copied() != Some(part) {
                    continue;
                }
                if id(s) == "air" {
                    dug += 1;
                    secs += prices.price("dig", stage_ix).unwrap_or(1.0);
                    continue;
                }
                placed += 1;
                secs += prices.price("place", stage_ix).unwrap_or(1.0) + redsim::entity::mob::prices::item_of(s).and_then(|it| prices.price(&it, stage_ix).ok()).unwrap_or(0.0);
                out.blocks.insert(*p, s.clone());
                if let Some(e) = d.block_entities.get(p) {
                    out.block_entities.insert(*p, e.clone());
                    // the items it holds (a filter's stock): each priced
                    secs += held_items(e).iter().map(|(item, n)| *n as f64 * prices.price(item, stage_ix).unwrap_or(0.0)).sum::<f64>();
                }
            }
            if placed == 0 && dug == 0 {
                continue;
            }
            // a dug-only part (nothing placed: an open drop dug into the ground): its dug cells as
            // air and its reference farm's terrain lines and site (ironspec 2026-11-10: "its file
            // carrying the terrain sample and its dug cells as air, so the view shows the shaft dug
            // into the ground"; the view draws it there: `mcview::terrain::is_part_in_terrain`)
            if placed == 0 {
                for (p, s) in &d.blocks {
                    if roles.get(p).copied() == Some(part) && id(s) == "air" {
                        out.blocks.insert(*p, s.clone());
                    }
                }
                for k in ["terrain", "sample", "ground", "afk"] {
                    if let Some(v) = d.meta.get(k) {
                        out.meta.insert(k.to_string(), v.clone());
                    }
                }
                if let Some(o) = farm_site(&d)? {
                    out.meta.insert("site".into(), format!("{},{},{}", o[0], o[1], o[2]));
                }
            }
            let (name, extra) = match part {
                "storage" => {
                    let slots = out.blocks.values().map(|s| match id(s).as_str() { "chest" => 27, "hopper" => 5, _ => 0 }).sum::<u32>() + 27;
                    let crafter = out.blocks.values().any(|s| id(s) == "crafter");
                    let hours = if crafter { 15552.0 / 480.0 } else { slots as f64 * 64.0 / 600.0 };
                    let name = match c.chain { 1 => "sorter and iron-block crafter (named-paper filters)", 2 => "sorter and iron-block crafter (plain filters)", _ => "hopper into a double chest beside the chest" };
                    (name.to_string(), format!(", storage hours at 480 iron/h {hours:.1}"))
                }
                "access" => ("spiral staircase with a fence-gate hatch".to_string(), String::new()),
                _ => match c.module.kill {
                    Kill::Side { len, .. } => (format!("drop out of the villagers' range, then a water channel {len} long to the side"), String::new()),
                    k => (format!("open drop {} blocks down to the kill", c.module.pad.floor as i32 - k.k() - 1), String::new()),
                },
            };
            // (kill and cage: the catalogs' own names)
            let name = match part {
                "kill" => match c.module.kill {
                    Kill::Pit { k } => format!("lava corner after a {}-block drop (out of the villagers' range)", -7 - k as i32),
                    Kill::Blade { k } => format!("lava blade after a {}-block drop", -7 - k as i32),
                    Kill::Side { len, .. } => format!("lava corner at the end of a {len}-long side channel"),
                    Kill::Shaft { .. } => name,
                },
                "cage" => format!("{} cage, {} watching villagers{}", match c.module.cage.calm { CalmKind::Plate => "bed-plate", CalmKind::Pulse => "dust-pulse", CalmKind::Observer => "bed-observer" }, c.module.cage.scared, if c.module.cage.regular { " (regular)" } else { "" }),
                _ => name,
            };
            let users: Vec<&str> = used.iter().filter(|(_, u, text)| member_of(u, part) == stem && match part {
                // (the listed design as it is: its access built, its storage in it)
                "access" => text.contains("\naccess: built"),
                "storage" => match c.chain { 1 | 2 => text.contains("crafter["), _ => text.contains("type=right") && !text.contains("crafter[") },
                _ => true,
            }).map(|(f, _, _)| f.as_str()).collect();
            let metrics = format!("blocks {placed}, blocks dug {dug}, build seconds {secs:.0}{extra}");
            std::fs::create_dir_all(dir.join(part))?;
            let catalog = std::fs::read_to_string(dir.join(part).join("members.tsv")).unwrap_or_default();
            let mine = matches!(part, "storage" | "access" | "transport");
            if mine || !catalog.lines().any(|l| l.split('\t').next() == Some(format!("{stem}.txt").as_str())) {
                std::fs::write(dir.join(part).join(format!("{stem}.txt")), out.to_text())?;
            }
            by_part.entry(part).or_default().push((stem, name, stage.to_string(), format!("{metrics}\t{}", users.join(" "))));
        }
    }
    let mut lines = Vec::new();
    for (part, members) in by_part {
        // merged into the part's catalog: its members kept as they are (farmopt's catalogs, page
        // copy's), only the missing ones added
        let path = dir.join(part).join("members.tsv");
        let old = std::fs::read_to_string(&path).unwrap_or_default();
        let have: Vec<&str> = old.lines().skip(1).map(|l| l.split('\t').next().unwrap_or("")).collect();
        let members: Vec<_> = members.into_iter().filter(|m| !have.contains(&format!("{}.txt", m.0).as_str()) || part == "storage" || part == "access" || part == "transport").collect();
        let mut tsv = String::from("file\tname\tstage\tmetrics\tused_by\n");
        // (a member written again keeps its row's place: the catalog is best first)
        for l in old.lines().skip(1) {
            let file = l.split('\t').next().unwrap_or("");
            if l.trim().is_empty() {
                continue;
            }
            match members.iter().find(|m| format!("{}.txt", m.0) == file) {
                Some((stem, name, stage, rest)) => tsv.push_str(&format!("{stem}.txt\t{name}\t{stage}\t{rest}\n")),
                None => {
                    tsv.push_str(l);
                    tsv.push('\n');
                }
            }
        }
        for (stem, name, stage, rest) in &members {
            if !have.contains(&format!("{stem}.txt").as_str()) {
                tsv.push_str(&format!("{stem}.txt\t{name}\t{stage}\t{rest}\n"));
            }
        }
        std::fs::write(&path, tsv)?;
        lines.push(format!("{part}: {} members ({})", members.len(), members.iter().map(|m| m.0.as_str()).collect::<Vec<_>>().join(", ")));
    }
    Ok(lines)
}

/// The family's bound for a part at a stage (iteration 20, ironspec's numbers from the game's
/// rules), printed beside the measured front; None: no bound known.
fn family_bound(part: &str, stage: usize) -> Vec<String> {
    match part {
        "kill" => {
            let mut v = vec!["lava (simulated): 240-250 gt a golem (4 damage per 10 gt of hurt cooldown on 100 HP)".to_string()];
            if stage >= 1 {
                v.push("fire charges (not simulated: redsim has no small fireball): ~190 gt; drops burn; a dispenser's 9 stacks then a refill".into());
            }
            if stage >= 2 {
                v.push("splash Harming II (not simulated: no splash potions): ~80 gt, 9 potions a golem, unstackable: a dispenser lasts ONE golem (fails the AFK bar)".into());
                v.push("falling anvils (not simulated: falling blocks hurt nothing): ~20 gt, 3 anvils of 40 damage, 31 iron each once".into());
            }
            v
        }
        "transport" => {
            let mut v = vec!["gravity (simulated): ~20 gt out of the villagers' cubes".to_string()];
            if stage >= 1 {
                v.push("slime launch (not simulated: slime blocks launch no mob): ~11 gt".into());
            }
            v
        }
        "storage" => vec!["one hopper is enough for the rate (2.5 items a second); 8 h late".into()],
        "pad" => vec!["mark share 1 (every summon attempt finds a spot)".into()],
        _ => Vec::new(),
    }
}

/// A part front's table: every candidate of `all` judged in full in `j`, its part's own numbers
/// (the split), its effect in the reference farm, the part's own front marked (min the part's
/// build seconds, max the farm's iron per hour, and the stage's time objective), the family's
/// bound and the measured gap.
pub fn part_front(all: &[Cand], j: &farmopt::journal::Journal, part: &str, stage: usize, calm_parts: &[crate::calm::Part]) -> Result<String> {
    struct Row {
        key: String,
        member: String,
        blocks: u32,
        dug: u32,
        secs: f64,
        iron: f64,
        iron_lo: f64,
        iron_hi: f64,
        time: f64,
        kill: String,
        tail: String,
        burnt: String,
        misses: String,
        ceiling: String,
        per_golem: String,
        fire: String,
    }
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for c in all {
        let key = crate::cand::key(c);
        if !seen.insert(key.clone()) {
            continue;
        }
        let Some(rec) = j.get(&key) else { continue };
        let Some(e) = rec.by_level.get(&farmopt::Level::Full) else { continue };
        let stat = rec.by_level.get(&farmopt::Level::Static);
        let get = |m: &str| e.metrics.get(m).or_else(|| stat.and_then(|s| s.metrics.get(m))).map(|v| (v.v, v.lo, v.hi));
        let Some((iron, iron_lo, iron_hi)) = get("iron_per_hour") else { continue };
        let time = if stage == 0 { get("t_kit_minutes") } else { get("build_minutes_per_module") }.map_or(f64::NAN, |t| t.0);
        // the part's own blocks, dug and seconds (the split, before the card's shared rows)
        let (d, roles) = roles(c, calm_parts)?;
        let prices = redsim::entity::mob::prices::Prices::get().map_err(|e| anyhow::anyhow!(e))?;
        let (mut blocks, mut dug, mut secs) = (0u32, 0u32, 0.0f64);
        for (p, s) in &d.blocks {
            if roles.get(p).copied() != Some(part) {
                continue;
            }
            if id(s) == "air" {
                if c.place == Place::Dug {
                    dug += 1;
                    secs += prices.price("dig", stage).unwrap_or(1.0);
                }
                continue;
            }
            blocks += 1;
            secs += prices.price("place", stage).unwrap_or(1.0) + redsim::entity::mob::prices::item_of(s).and_then(|it| prices.price(&it, stage).ok()).unwrap_or(0.0);
            if let Some(be) = d.block_entities.get(p) {
                secs += held_items(be).iter().map(|(it, n)| *n as f64 * prices.price(it, stage).unwrap_or(0.0)).sum::<f64>();
            }
        }
        let line = |k: &str| e.lines.get(k).cloned().unwrap_or_default();
        let misses = line("misses");
        // "N of M tries found no spot": the share
        let share = {
            let w: Vec<&str> = misses.split_whitespace().collect();
            match (w.first().and_then(|x| x.parse::<f64>().ok()), w.get(2).and_then(|x| x.parse::<f64>().ok())) {
                (Some(a), Some(b)) if b > 0.0 => format!("{:.0}%", 100.0 * a / b),
                _ => "-".into(),
            }
        };
        let fire = line("fire");
        rows.push(Row {
            key,
            member: member_of(c, part),
            blocks,
            dug,
            secs,
            iron,
            iron_lo,
            iron_hi,
            time,
            kill: line("kill"),
            ceiling: line("share_of_ceiling"),
            per_golem: line("seconds_per_golem"),
            tail: line("kill_tail").split(':').next().unwrap_or("").trim_start_matches("the longest ").to_string(),
            burnt: line("iron_burnt_per_golem").split(' ').next().unwrap_or("").to_string(),
            misses: share,
            fire: if fire.is_empty() || fire.starts_with("never") || fire.starts_with("no ") { String::new() } else { "fire warning".into() },
        });
    }
    // the part's front: min its own seconds, max the farm's iron per hour, min the stage's time
    let pts: Vec<Vec<f64>> = rows.iter().map(|r| vec![-r.secs, r.iron, -r.time]).collect();
    let on: std::collections::HashSet<usize> = farmopt::pareto::front(&pts).into_iter().collect();
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|&a, &b| rows[b].iron.partial_cmp(&rows[a].iron).unwrap_or(std::cmp::Ordering::Equal));
    let time_head = if stage == 0 { "T (min, travel incl.)" } else { "build min / module" };
    let mut s = format!(
        "# The {part} front, {} (iteration 20)\n\nEvery member the part run judged in full (AI on), in the stage's reference farm with only the {part} changed. Its own numbers are the split's (the blocks the part places, digs, and their seconds at the stage's prices, before the card's shared travel rows). ● marks the part's front: least own seconds, most iron per hour, least {time_head}. The ceiling share is the golems an hour against one golem per 600 gt per summoner (what is left: the lockout's timing, the golem's dwell in the follow boxes).\n\n| | member | key | blocks | dug | own s | iron/h | {time_head} | ceiling share | s a golem | kill mean | kill longest | burnt / golem | no-spot tries | |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
        ["day 1", "middle", "late"][stage]
    );
    for &i in &order {
        let r = &rows[i];
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {:.0} | {:.0} ({:.0}-{:.0}) | {:.1} | {} | {} | {} | {} | {} | {} | {} |\n",
            if on.contains(&i) { "●" } else { "" },
            if r.member.is_empty() { "-" } else { &r.member },
            r.key,
            r.blocks,
            r.dug,
            r.secs,
            r.iron,
            r.iron_lo,
            r.iron_hi,
            r.time,
            r.ceiling,
            r.per_golem,
            r.kill,
            r.tail,
            r.burnt,
            r.misses,
            r.fire
        ));
    }
    let bounds = family_bound(part, stage);
    if !bounds.is_empty() {
        s.push_str("\nBounds (each family's best from the game's rules):\n\n");
        for b in &bounds {
            s.push_str(&format!("- {b}\n"));
        }
        if part == "kill" {
            let best = rows.iter().filter_map(|r| r.kill.split_whitespace().next().and_then(|x| x.parse::<f64>().ok())).fold(f64::INFINITY, f64::min);
            if best.is_finite() {
                s.push_str(&format!("\nMeasured: the fastest mean kill {best:.0} gt against lava's 240 gt: a gap of {:.0}% (the dwell before a golem meets the lava: its walk to the corner, the blade's reach)\n", 100.0 * (best - 240.0) / 240.0));
            }
        }
    }
    Ok(s)
}
