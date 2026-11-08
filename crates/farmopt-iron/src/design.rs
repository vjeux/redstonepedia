//! A candidate as a design file (the judge's text format, preview-ready): its modules placed by
//! the tiling, the calm wired, the chest at `port output`, the meta the judge and the site read.

use crate::cand::{Cand, Tiling};
use crate::parts::{self, CalmKind, Place, Ports};
use anyhow::{Result, anyhow, bail};
use mobfarm::layouts::Builder;

/// A built candidate: its text and what the evaluation needs.
pub struct Built {
    pub text: String,
    pub ports: Vec<Ports>,
    /// The design y of the ground layer (dug designs).
    pub ground: Option<i32>,
}

/// An exact seed's text, by its generator (crates/mobfarm), its chest put in.
fn seed_text(name: &str) -> Result<String> {
    use mobfarm::day1;
    let (text, how) = match name {
        "d1" => (day1::l6p_pad(6), "golems burn in a corner of their pad"),
        "d4" => (day1::d4_two_scared(6), "golems burn in a corner of their pit"),
        "d1u" => (day1::d1u(6), "golems burn in a corner of their pit"),
        "l3" => (mobfarm::layouts::l3(8), "golems burn in lava pits"),
        "m2open" => (day1::m2_open(6), "golems fall out of sight to a lava blade"),
        "m3" => (day1::m3_bedplate(6), "golems fall out of sight to a lava blade"),
        "m4" => (day1::m2_stack(2, [0, 43, 0], 6), "2 middle modules"),
        "lb8" => (day1::m2_stack(8, [0, 43, 0], 6), "8 middle modules"),
        "ls8" => (day1::stack_collect(8, 43, 6), "8 modules share one collection"),
        other => bail!("no seed {other:?}"),
    };
    mobfarm::layouts::edit(&text, true, Some(how))
}

/// The caption: what the build is, in a few words.
fn caption(c: &Cand) -> Vec<String> {
    let m = &c.module;
    let calm = match m.cage.calm {
        CalmKind::Plate => "the bed plate (U1's wooden plate holds the hide open by day; at nightfall it lies down and the hide shuts)".to_string(),
        CalmKind::Observer => format!("a bed observer and catalog timing part {} (a hide of 20 gt or more when U1 lies down)", c.calm_part),
    };
    let kill = match m.kill {
        crate::parts::Kill::Pit { k } => format!("a pit corner {} below the beds under one lava source", -k),
        crate::parts::Kill::Blade { k } => format!("a fire-safe lava blade {} below the beds", -k),
    };
    vec![
        format!("farmopt candidate {} (designs/briefs/farm-optimizer.md).", crate::cand::key(c)),
        format!("{} module(s); each: {} scared and {} blind villagers in 1-cell pods, a zombie in a boat, {calm};", c.modules, m.cage.scared, m.cage.blind),
        format!("golems spawn on a {}x{} pad {} below the beds and fall through its dry hole into {kill}.", m.pad.n, m.pad.n, -m.pad.floor),
    ]
}

/// The candidate's design (built once per process and key: the calm's placement and the glass caps
/// build the world several times).
pub fn build(c: &Cand, calm_parts: &[crate::calm::Part]) -> Result<Built> {
    if let Some(s) = c.seed {
        return Ok(Built { text: seed_text(s)?, ports: Vec::new(), ground: None });
    }
    type Cache = std::sync::Mutex<std::collections::HashMap<String, (String, Vec<Ports>, Option<i32>)>>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let key = crate::cand::key(c);
    let cache = CACHE.get_or_init(Default::default);
    if let Some((text, ports, ground)) = cache.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return Ok(Built { text, ports, ground });
    }
    let built = build_fresh(c, calm_parts)?;
    if let Ok(mut m) = cache.lock() {
        if m.len() > 20000 {
            m.clear();
        }
        m.insert(key, (built.text.clone(), built.ports.clone(), built.ground));
    }
    Ok(built)
}

fn build_fresh(c: &Cand, calm_parts: &[crate::calm::Part]) -> Result<Built> {
    if c.l3_pits > 0 {
        // the L3 family, by its own generator (spawn pits that burn golems in place)
        let mut d = mctas::design::Design::parse(&mobfarm::layouts::l3(c.l3_pits as i32))?;
        d.meta.insert("ground".into(), "-1".into());
        d.name = crate::cand::key(c);
        let mut text = format!("# farmopt candidate {} (designs/briefs/farm-optimizer.md): L3's spawn pits, {} of them, on the surface.\n", crate::cand::key(c), c.l3_pits);
        text.push_str(&d.to_text());
        return Ok(Built { text: mobfarm::layouts::edit(&text, true, Some("golems burn in lava pits"))?, ports: Vec::new(), ground: Some(-1) });
    }
    let mut b = Builder::new(&crate::cand::key(c));
    let mut ports = Vec::new();
    let mut ats = Vec::new();
    for i in 0..c.modules as i32 {
        let at = match c.tiling {
            Tiling::Row(p) => [i * p as i32, 0, 0],
            Tiling::Stack(p) => [0, i * p as i32, 0],
        };
        let with_u1 = !(c.shared_calm && i > 0);
        let own_calm = c.module.cage.calm == CalmKind::Observer && (!c.shared_calm || i == 0);
        let part = if own_calm { Some(calm_parts.get(c.calm_part as usize).ok_or_else(|| anyhow!("no calm part {}", c.calm_part))?) } else { None };
        let gen_place = if c.surface { Place::Void } else { c.place };
        let (alone, p) = module_alone(&c.module, gen_place, with_u1, part)?;
        merge(&mut b, &alone, at).map_err(|e| anyhow!("module {i} at pitch {}: {e}", c.tiling.pitch()))?;
        ports.push(p.moved(at));
        ats.push(at);
    }
    // the chest of module 0 is the output; the others' chests are blocks (before the calm: the
    // glass caps build the farm, which needs its output)
    let out = ports[0].output;
    for p in &ports[1..] {
        b.set(p.output[0], p.output[1], p.output[2], parts::CHEST);
    }
    b.d.ports.insert("output".into(), out);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    if c.module.cage.calm == CalmKind::Observer {
        if c.shared_calm && c.modules > 1 {
            crate::calm::share(&mut b, &ports, &ats)?;
        }
        // a part's top or a support must not become a spawn base: a torch on every stray spot
        // (`cap_spawns`: ironspec iteration 3's cheapest cap), the pads' spots kept (each module
        // was capped alone; this catches the shared routes' supports and anything the tiling
        // exposes)
        let pads: std::collections::HashSet<[i32; 3]> = ports.iter().flat_map(|p| p.pad_cells.iter().map(|q| [q[0], q[1] + 1, q[2]])).collect();
        cap_spawns(&mut b, &|q| pads.contains(&q))?;
    }
    // middle and late: a lightning rod on the farm's highest block takes every bolt within 128
    // blocks (ServerLevel.findLightningRod), 3 copper for no villager turned and no drop burnt
    if c.place == Place::Void {
        add_rod(&mut b);
    }
    let ground = if c.place == Place::Dug && !c.surface {
        b.d.meta.insert("terrain".into(), "cells".into());
        b.d.meta.insert("ground".into(), c.ground.to_string());
        b.d.meta.insert("afk".into(), format!("2.5 {} 4.5", c.ground as i32 + 1));
        Some(c.ground as i32)
    } else if c.surface {
        // a void build on the terrain's surface: its pods' floor (y -1) on the ground layer, its
        // box cleared to the sky (the site's `clear_box`)
        b.d.meta.insert("ground".into(), "-1".into());
        Some(-1)
    } else {
        None
    };
    let how = match c.module.kill {
        crate::parts::Kill::Pit { k } if k >= -16 => "golems burn in a corner of their pit",
        crate::parts::Kill::Pit { .. } => "golems fall out of sight into a lava pit",
        crate::parts::Kill::Blade { .. } => "golems fall out of sight to a lava blade",
    };
    let cap = caption(c);
    let text = mobfarm::layouts::edit(&b.text(&cap.iter().map(|s| s.as_str()).collect::<Vec<_>>()), true, Some(how))?;
    Ok(Built { text, ports, ground })
}

/// The design of a candidate, parsed back (what the judge reads).
pub fn design(c: &Cand, calm_parts: &[crate::calm::Part]) -> Result<mctas::design::Design> {
    mctas::design::Design::parse(&build(c, calm_parts)?.text)
}

/// One module built alone at the origin, its own calm wired and its stray spawn spots capped:
/// cached per (module, placement, U1, part), since every tiling reuses it.
fn module_alone(m: &crate::parts::Module, place: Place, with_u1: bool, part: Option<&crate::calm::Part>) -> Result<(mctas::design::Design, Ports)> {
    type Cache = std::sync::Mutex<std::collections::HashMap<String, (mctas::design::Design, Ports)>>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let key = format!("{m:?}|{place:?}|{with_u1}|{}", part.map(|p| p.name.as_str()).unwrap_or("-"));
    let cache = CACHE.get_or_init(Default::default);
    if let Some(v) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return Ok(v);
    }
    let mut b = Builder::new("module");
    let p = parts::module(&mut b, [0, 0, 0], place, m, with_u1);
    if let Some(part) = part {
        // the farm's output for the caps' farm build (a chest at the module's output)
        b.d.ports.insert("output".into(), p.output);
        b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
        crate::calm::wire(&mut b, part, &p, [0, 0, 0])?;
        let pads: std::collections::HashSet<[i32; 3]> = p.pad_cells.iter().map(|q| [q[0], q[1] + 1, q[2]]).collect();
        cap_spawns(&mut b, &|q| pads.contains(&q))?;
        b.d.ports.clear();
        b.d.meta.clear();
    }
    let v = (b.d, p);
    if let Ok(mut c) = cache.lock() {
        c.insert(key, v.clone());
    }
    Ok(v)
}

/// Cover every golem spawn spot of the farm in `b` that `keep` does not name with a torch in its
/// feet cell (LEGACY_IRON_GOLEM needs air or liquid above its base: a torch, button, plate or
/// carpet there stops the scan as glass does, and a torch is the cheapest at every stage:
/// designs/farms/prices.tsv). The ironfarm lane's `parts::cap_spawns` with the cheaper cap.
pub fn cap_spawns(b: &mut Builder, keep: &dyn Fn([i32; 3]) -> bool) -> Result<u32> {
    use redsim::entity::mob::farm::{RunOpts, WalkMode, build};
    let mut placed = 0;
    for _ in 0..3 {
        let c = circuit::Circuit::from_design(b.d.clone())?;
        let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow!(e))?;
        let w = build(&spec, &RunOpts::steady(1, 0, WalkMode::Random)).map_err(|e| anyhow!(e))?;
        let stray: Vec<[i32; 3]> = redsim::entity::mob::exit::spawn_spots(&w).into_iter().map(|p| [p.x, p.y, p.z]).filter(|p| !keep(*p)).collect();
        if stray.is_empty() {
            break;
        }
        for p in stray {
            b.set(p[0], p[1], p[2], "torch");
            placed += 1;
        }
    }
    Ok(placed)
}

/// A lightning rod topping its own column away from every entity, inside the farm's box (no extra
/// volume): on a stone at one of the box's top corners, the one farthest from the villagers and
/// the zombie (a bolt strikes the rod and hurts what stands within a few blocks of it); north of
/// the zombies' row when no corner is free.
fn add_rod(b: &mut Builder) {
    if b.d.blocks.is_empty() {
        return;
    }
    let (lo, hi) = b.d.blocks.keys().fold(([i32::MAX; 3], [i32::MIN; 3]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])], [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])]));
    let ents: Vec<[f64; 3]> = b.d.entities.iter().map(|e| e.pos).collect();
    let free = |p: [i32; 3]| b.d.blocks.get(&p).is_none_or(|s| s.ends_with(":air"));
    let far = |p: [i32; 3]| ents.iter().map(|e| ((e[0] - p[0] as f64 - 0.5).powi(2) + (e[1] - p[1] as f64).powi(2) + (e[2] - p[2] as f64 - 0.5).powi(2)).sqrt()).fold(f64::INFINITY, f64::min);
    let corners = [[lo[0], hi[1], lo[2]], [hi[0], hi[1], lo[2]], [lo[0], hi[1], hi[2]], [hi[0], hi[1], hi[2]]];
    let best = corners.iter().filter(|p| free(**p) && free([p[0], p[1] - 1, p[2]]) && far(**p) >= 6.0).max_by(|a, b2| far(**a).total_cmp(&far(**b2))).copied();
    let (at, under) = match best {
        Some(p) => (p, [p[0], p[1] - 1, p[2]]),
        None => {
            let cx = (lo[0] + hi[0]) / 2;
            let z = lo[2].min(-7) - 10;
            ([cx, hi[1] + 1, z], [cx, hi[1], z])
        }
    };
    b.set(under[0], under[1], under[2], crate::parts::STONE);
    b.set(at[0], at[1], at[2], "lightning_rod[facing=up,powered=false,waterlogged=false]");
}

/// Merge a module built alone into the farm at `at`: a cell both write must hold the same block.
fn merge(b: &mut Builder, m: &mctas::design::Design, at: [i32; 3]) -> Result<()> {
    for (p, s) in &m.blocks {
        let q = [p[0] + at[0], p[1] + at[1], p[2] + at[2]];
        if let Some(old) = b.d.blocks.get(&q)
            && old != s
        {
            bail!("modules overlap at {q:?} ({old} and {s})");
        }
        b.d.blocks.insert(q, s.clone());
    }
    for (p, s) in &m.block_entities {
        b.d.block_entities.insert([p[0] + at[0], p[1] + at[1], p[2] + at[2]], s.clone());
    }
    for e in &m.entities {
        b.d.entities.push(mctas::design::EntitySpec { kind: e.kind.clone(), pos: [e.pos[0] + at[0] as f64, e.pos[1] + at[1] as f64, e.pos[2] + at[2] as f64], nbt: e.nbt.clone() });
    }
    Ok(())
}
