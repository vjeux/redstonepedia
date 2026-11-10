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
        CalmKind::Pulse => "U1's plate into the falling-edge pulse (catalog row 189: torches, dust, repeaters; a 22 gt hide when U1 lies down at dusk)".to_string(),
    };
    let kill = match m.kill {
        crate::parts::Kill::Pit { k } => format!("a pit corner {} below the beds under one lava source", -k),
        crate::parts::Kill::Blade { k } => format!("a fire-safe lava blade {} below the beds", -k),
        crate::parts::Kill::Shaft { k } => format!("a shaft {} deep to the shared kill below", -k),
        crate::parts::Kill::Side { len, feet, .. } => if feet == 0 { format!("a water channel {len} blocks to the side just under the pad, to a lava blade beside the farm") } else { format!("a drop to {} below the beds, then a water channel {len} blocks to the side through the rock, to a lava blade beside the farm", -feet) },
    };
    vec![
        format!("farmopt candidate {} (designs/briefs/farm-optimizer.md).", crate::cand::key(c)),
        format!("{} module(s); each: {} scared and {} blind villagers in 1-cell pods, a zombie in a boat, {calm};", c.modules, m.cage.scared, m.cage.blind),
        if m.pad.n == 9 { format!("golems spawn on an 8x8 quadrant pad {} below the beds, each quadrant's spawn cells around its own 2x2 hole, and fall through a collector into {kill}.", -m.pad.floor) } else { format!("golems spawn on a {}x{} pad {} below the beds and fall through its dry hole into {kill}.", m.pad.n, m.pad.n, -m.pad.floor) },
    ]
}

/// The candidate's design (built once per process and key: the calm's placement and the glass caps
/// build the world several times).
/// The stage this process builds for (set once by `IronFarm::new`): its kind's tier word.
pub static STAGE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

thread_local! {
    /// Leave the built-in access or the storage out of this thread's builds (the parts split diffs
    /// a farm with and without them).
    pub static WITHOUT: std::cell::Cell<(bool, bool)> = const { std::cell::Cell::new((false, false)) };
}

/// The judge's tier word of the process's stage (circuit::mobfarm::TIERS): day-1, middle,
/// fastest.
pub fn tier_word() -> &'static str {
    match STAGE.get().copied().unwrap_or(1) {
        0 => "day-1",
        2 => "fastest",
        _ => "middle",
    }
}

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

/// `build` without its cache (the parts split builds the same key with and without parts).
pub fn build_uncached(c: &Cand, calm_parts: &[crate::calm::Part]) -> Result<Built> {
    build_fresh(c, calm_parts)
}

fn build_fresh(c: &Cand, calm_parts: &[crate::calm::Part]) -> Result<Built> {
    if c.family_gen == 1 || c.family_gen == 2 || c.family_gen == 5 || c.family_gen == 8 {
        // the ironfarm lane's generators, as they write them (their timer: the smallest catalog
        // part legal in the Nether, which the End's materials include; L_min 20 gt)
        let stage = mobfarm::parts::Stage::Nether;
        let _ = calm_parts;
        let text = match c.family_gen {
            1 => mobfarm::day1::stack(c.modules as i32, c.tiling.pitch(), c.module.pad.n as i32, crate::calm::L_MIN, stage)?,
            // the shared kill's form (b) (ironspec's default): a blade under each module's drop,
            // one deep items channel and one collection for the row
            5 => mobfarm::day1::row_shared(c.modules as i32, c.tiling.pitch(), c.module.pad.n as i32, crate::calm::L_MIN, stage)?,
            // the + (vjeux 18:16: "a + layout with the central shaft in the middle"): four
            // modules at radius r around one deep junction kill (ironfarm's cross_channel), r free
            8 => mobfarm::day1::cross_channel_arms(c.tiling.pitch(), c.modules, c.module.pad.n as i32, crate::calm::L_MIN, stage)?,
            _ => mobfarm::day1::row(c.modules as i32, c.tiling.pitch(), c.module.pad.n as i32, crate::calm::L_MIN, stage)?,
        };
        let mut d = mctas::design::Design::parse(&text)?;
        d.name = crate::cand::key(c);
        let mut b = Builder { d };
        add_rod(&mut b);
        let gen_name = match c.family_gen {
            1 => "stack",
            5 => "row_shared (form b)",
            8 => "cross_channel (the +)",
            _ => "row",
        };
        let text = b.text(&[&format!("farmopt candidate {} (designs/briefs/farm-optimizer.md): the ironfarm lane's `{gen_name}` generator.", crate::cand::key(c))]);
        return Ok(Built { text, ports: Vec::new(), ground: None });
    }
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
    // a shared-kill stack's chains: every `chain` modules share the kill at the chain's bottom,
    // the chains 43 apart (the stack of copies' no-coupling pitch between a chain's top and the
    // next chain's bottom)
    let chain_len = if c.family_gen == 3 && c.chain > 0 { c.chain as i32 } else { c.modules as i32 };
    let chain_of = |i: i32| (i / chain_len, i % chain_len);
    // the ring: 4 cages around one pad, hole, kill and collection (its cage offset in the
    // tiling's pitch); one module's ports (the shared pad's and kill's)
    let ring = c.family_gen == 7;
    if ring {
        let part = if c.module.cage.calm == CalmKind::Observer { Some(calm_parts.get(c.calm_part as usize).ok_or_else(|| anyhow!("no calm part {}", c.calm_part))?) } else { None };
        let (alone, p) = module_alone(&c.module, c.place, true, part, true)?;
        let d = ring_shared_pad(&alone, &p, &c.module, c.tiling.pitch(), c.modules)?;
        merge(&mut b, &d, [0, 0, 0])?;
        ports.push(p);
        ats.push([0, 0, 0]);
    }
    for i in 0..(if ring { 0 } else { c.modules as i32 }) {
        let at = match c.tiling {
            Tiling::Row(p) => [i * p as i32, 0, 0],
            // a dug stack goes down into the ground from its top module (y 0, the one the
            // `ground` layer is measured from); a void stack goes up
            Tiling::Stack(p) if c.place == Place::Dug => [0, (i - (c.modules as i32 - 1)) * p as i32, 0],
            Tiling::Stack(p) if c.family_gen == 3 => {
                let (q, j) = chain_of(i);
                [0, q * ((chain_len - 1) * p as i32 + 43) + j * p as i32, 0]
            }
            Tiling::Stack(p) => [0, i * p as i32, 0],
        };
        let with_u1 = !(c.shared_calm && i > 0);
        let own_calm = c.module.cage.calm == CalmKind::Observer && (!c.shared_calm || i == 0);
        let pulse = if c.module.cage.calm == CalmKind::Pulse { Some(crate::calm::pulse_part().ok_or_else(|| anyhow!("no day-1 falling-edge pulse part in the catalog"))?) } else { None };
        let part = if let Some(p) = pulse.as_ref() {
            Some(p)
        } else if own_calm {
            Some(calm_parts.get(c.calm_part as usize).ok_or_else(|| anyhow!("no calm part {}", c.calm_part))?)
        } else {
            None
        };
        let gen_place = if c.surface { Place::Void } else { c.place };
        // a shared-kill stack: every module above its chain's bottom one drops its golems down a
        // shaft through the pad space of the module below (its hole right under), into the one
        // kill and collection at the chain's bottom
        let module = if c.family_gen == 3 && chain_of(i).1 > 0 {
            let p = c.tiling.pitch();
            // the shaft's rings reach down to 7 above the lower module's beds (over its villagers'
            // spawn scan, y - 6 .. y + 6); the golem falls the last 13 layers free into its hole
            crate::parts::Module { kill: crate::parts::Kill::Shaft { k: (7 - p).clamp(-120, -1) as i8 }, ..c.module }
        } else {
            c.module
        };
        let (alone, p) = module_alone(&module, gen_place, with_u1, part, !c.shared_calm)?;
        merge(&mut b, &alone, at).map_err(|e| anyhow!("module {i} at pitch {}: {e}", c.tiling.pitch()))?;
        ports.push(p.moved(at));
        ats.push(at);
    }
    // the chest of module 0 is the output; the others' chests are blocks (before the calm: the
    // glass caps build the farm, which needs its output); a shared-kill stack's upper modules
    // have none
    let mut out = ports[0].output;
    // late (an 8-hour absence, ironspec: iron the storage can't hold is lost): every collection
    // holds 8 hours at the ceiling (480 iron + 120 poppies a module an hour: 3 chests a module),
    // chained down from its chest (each hopper under a chest pulls from it into the next)
    let late = tier_word() == "fastest";
    let chain_down = |b: &mut Builder, o: [i32; 3], extra: i32| {
        for j in 1..=extra {
            b.set(o[0], o[1] - 2 * j + 1, o[2], "hopper[enabled=true,facing=down]");
            b.set(o[0], o[1] - 2 * j, o[2], parts::CHEST);
        }
    };
    // the storage (vjeux 21:06: "You also need a proper storage system. I don't know what this
    // chest + hopper chain is supposed to be but that's not it"; ironspec iteration 20: a HORIZONTAL
    // row of double chests at the player's level, never a vertical chain): under the judge's chest
    // (the output, unchanged) one hopper pushes east into a double chest beside it (the judge's
    // chest and the double chest: 81 slots, ~10 h of one module's iron and poppies), more double
    // chests on east along a hopper line each feeding the next only when its chest is full.
    let storage_row = |b: &mut Builder, o: [i32; 3]| {
        let y = o[1] - 1;
        b.set(o[0], y, o[2], "hopper[enabled=true,facing=east]");
        b.set(o[0] + 1, y, o[2], "chest[facing=south,type=right,waterlogged=false]");
        b.set(o[0] + 2, y, o[2], "chest[facing=south,type=left,waterlogged=false]");
    };
    // the ironfarm lane's own sorter and iron-block crafter (mobfarm::day1::collect_bottom_at1)
    // placed where a module's chest was, as their row-2x27 places it, its stock rewritten to the
    // classic one below (row-2x27's, whose real-game gate passes); returns its output chest. With
    // the cheap stock (filters 41 + 4 sticks, empty locks) late-shared-kill-pair lost a quarter of
    // its iron in the game (2026-11-09: 2.6-3.8 iron per golem; classic 4.00), the same frame
    let ironfarm_sorter = |b: &mut Builder, o: [i32; 3]| -> [i32; 3] {
        let mut t = Builder::new("collect");
        mobfarm::day1::collect_bottom_at1(&mut t, o[1] + 1);
        let (dx, dz) = (o[0] - 1, o[2] + 1);
        for (q, s) in &t.d.blocks {
            b.d.blocks.insert([q[0] + dx, q[1], q[2] + dz], s.clone());
        }
        for (q, s) in &t.d.block_entities {
            b.d.block_entities.insert([q[0] + dx, q[1], q[2] + dz], s.clone());
        }
        // the classic stock (row-2x27's, which passes in the game): each filter 41 of its item and
        // four papers named "<item> filter", its lock hopper under it 64 of the item and four
        // papers named "<item> lock". The cheap stock (filters 41 + 4 sticks, empty locks) loses a
        // quarter of the iron over the gate's 3 days in the game though redsim keeps it all
        let papers = |name: &str| (1..=4).map(|k| format!("{{Slot:{k}b,id:\"minecraft:paper\",count:1,components:{{\"minecraft:custom_name\":\"{name}\"}}}}")).collect::<Vec<_>>().join(",");
        for (q, s) in &t.d.block_entities {
            if !s.contains("minecraft:stick") {
                continue;
            }
            let (item, word) = if s.contains("minecraft:poppy") { ("poppy", "poppy") } else { ("iron_ingot", "iron") };
            let at = [q[0] + dx, q[1], q[2] + dz];
            b.d.block_entities.insert(at, format!("{{Items:[{{Slot:0b,id:\"minecraft:{item}\",count:41}},{}]}}", papers(&format!("{word} filter"))));
            b.d.block_entities.insert([at[0], at[1] - 1, at[2]], format!("{{Items:[{{Slot:0b,id:\"minecraft:{item}\",count:64}},{}]}}", papers(&format!("{word} lock"))));
        }
        let to = t.d.ports["output"];
        [to[0] + dx, to[1], to[2] + dz]
    };
    if ring {
        // the ring's one collection holds the judge's run of its 4 groups (late: 8 hours)
        let extra = if late { 3 * c.modules as i32 - 1 } else { (c.modules as i32 + 1) / 2 - 1 };
        chain_down(&mut b, out, extra);
    } else if c.family_gen != 3 {
        for p in &ports[1..] {
            b.set(p.output[0], p.output[1], p.output[2], parts::CHEST);
        }
        // middle (a session: one double chest a module, ~7 h of one module's iron and poppies) and
        // late (8 hours: two double chests a module) in the void: the storage row under each
        // module's chest; or (a grammar module's `chain` 1) the sorter and the iron-block crafter
        // (the ironfarm lane's collect_bottom_at1: the line hopper where the module's chest was,
        // the iron slice's filter under it, the crafter packing 9 ingots a block into one chest:
        // 27 stacks of blocks, 15,552 iron; the poppies to their own chests)
        let mut new_out = None;
        if c.place == Place::Void && c.family_gen == 0 && !WITHOUT.with(|w| w.get().1) {
            for (i, p) in ports.iter().enumerate() {
                if c.chain >= 1 {
                    // (chain 1 and 2 alike now: the ironfarm lane's own sorter and crafter with
                    // the classic stock; the cheap stock lost iron in the game)
                    let port = ironfarm_sorter(&mut b, p.output);
                    if i == 0 {
                        new_out = Some(port);
                    } else {
                        b.set(port[0], port[1], port[2], parts::CHEST);
                    }
                } else {
                    storage_row(&mut b, p.output);
                }
            }
        }
        if let Some(port) = new_out {
            out = port;
            b.d.blocks.remove(&port);
        }
    } else if c.place == Place::Void {
        // every chain's bottom: its collection holds the judge's run of its modules' iron (one
        // chest per two modules, chained down from its chest: each hopper under a chest pulls
        // from it into the next, 2.5 items a second); module 0's chest is the farm's output, the
        // other chains' chests are blocks. Its blade shaft gets a ring every third layer up to its
        // pad: golems falling from the modules above arrive fast and still walking, and an open
        // shaft lets them drift out (dug: the rock is the wall)
        let m = &c.module;
        let (n, fl, k) = (m.pad.n as i32, m.pad.floor as i32, m.kill.k());
        let x0 = parts::pad_x0(&m.cage, &m.pad);
        let (hx, hz) = (x0 + n / 2 - 2, 2 + n / 2 - 2);
        for i in (0..c.modules as i32).filter(|&i| chain_of(i).1 == 0) {
            let (q, _) = chain_of(i);
            let in_chain = (c.modules as i32 - q * chain_len).min(chain_len);
            let o = ports[i as usize].output;
            if i > 0 {
                b.set(o[0], o[1], o[2], parts::CHEST);
            }
            // (vjeux 21:06, iteration 20: no vertical chest chain): a chain of 2 or more modules
            // stores in the cheap sorter and iron-block crafter (8 hours of 2 modules: ~850 iron
            // blocks in one chest, the poppies in two), one module in the storage row
            if in_chain >= 2 && !WITHOUT.with(|w| w.get().1) {
                b.d.blocks.remove(&o);
                let port = ironfarm_sorter(&mut b, o);
                if i == 0 {
                    out = port;
                    b.d.blocks.remove(&port);
                } else {
                    b.set(port[0], port[1], port[2], parts::CHEST);
                }
            } else if !WITHOUT.with(|w| w.get().1) {
                storage_row(&mut b, o);
            }
            let _ = (late, &chain_down);
            if in_chain < 2 {
                continue;
            }
            let base = ats[i as usize][1];
            for y in (k + 4)..fl {
                // a ring every third layer (a golem, 2.7 tall, cannot leave through a 2-high
                // gap; under the pad, out of the villagers' spawn scan)
                if (fl - 1 - y) % 3 != 0 {
                    continue;
                }
                for x in (hx - 1)..=(hx + 4) {
                    for z in (hz - 1)..=(hz + 4) {
                        let inside = (hx..=hx + 3).contains(&x) && (hz..=hz + 3).contains(&z);
                        if !inside && !b.d.blocks.contains_key(&[x, base + y, z]) {
                            b.set(x, base + y, z, parts::STONE);
                        }
                    }
                }
            }
        }
    }
    b.d.ports.insert("output".into(), out);
    // the kind's tier: the judge's card prices the build at that stage (circuit::mobfarm's
    // price_stage; an untiered design pays middle prices once day 1 can't get every item)
    b.d.meta.insert("circuit".into(), format!("mob-farm iron_ingot {}", tier_word()));
    b.d.meta.insert("restart".into(), "yes".into());
    if c.module.cage.calm == CalmKind::Observer || c.module.cage.calm == CalmKind::Pulse {
        if c.shared_calm && c.modules > 1 {
            crate::calm::share(&mut b, &ports, &ats)?;
        }
        // a part's top or a support must not become a spawn base: a torch on every stray spot
        // (`cap_spawns`: ironspec iteration 3's cheapest cap), the pads' spots kept (each module
        // was capped alone; this catches the shared routes' supports and anything the tiling
        // exposes); in the void only (dug: the site's covers cap the terrain's stray cells, and a
        // void simulation of a dug design sees floating cells: its torches pop off in the terrain)
        let pads: std::collections::HashSet<[i32; 3]> = ports.iter().flat_map(|p| p.pad_cells.iter().map(|q| [q[0], q[1] + 1, q[2]])).collect();
        if c.place == Place::Void {
            cap_spawns(&mut b, &|q| pads.contains(&q))?;
        }
    }
    // the blocks a player actually places (vjeux 13:09: stone needs silk touch or a smelt, glass a
    // smelt, leaves shears): every wall is cobblestone, every slab a cobblestone slab, and what was
    // glass for spawn-proofing becomes cobblestone with the cheapest cap where a golem could
    // spawn (the caps below, or the site's covers in terrain); the judge checks each build
    if c.place == Place::Void && !c.surface {
        close_bed_gaps(&mut b);
    }
    truthful_materials(&mut b);
    if c.place == Place::Void {
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
        // a middle or late build dug into flat ground (the shared-kill stack: its rock walls the
        // shaft for free)
        if c.family_gen == 3 {
            b.d.meta.insert("sample".into(), "plains".into());
        }
        // day 1: the access built in (ironspec iteration 16, the parent: the day-1 lane's spiral
        // staircase from the chest up to the ground layer, its fence-gate hatch the one block on
        // the surface), where it fits
        if c.family_gen == 0 && c.modules == 1
            && let Some(out) = ports.first().map(|p| p.output)
        {
            if !WITHOUT.with(|w| w.get().0)
                && let Err(e) = dug_access(&mut b, out, c.ground as i32)
                && std::env::var("FARMOPT_DEBUG").is_ok()
            {
                eprintln!("{}: {e}", crate::cand::key(c));
            }
        }
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
        crate::parts::Kill::Blade { .. } if c.family_gen == 3 => "every module's golems fall down one shaft to one lava blade",
        crate::parts::Kill::Blade { .. } => "golems fall out of sight to a lava blade",
        crate::parts::Kill::Shaft { .. } => "golems fall down a shaft to a shared kill",
        crate::parts::Kill::Side { .. } => "golems ride a water channel to the side into a lava corner",
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
fn module_alone(m: &crate::parts::Module, place: Place, with_u1: bool, part: Option<&crate::calm::Part>, direct: bool) -> Result<(mctas::design::Design, Ports)> {
    type Cache = std::sync::Mutex<std::collections::HashMap<String, (mctas::design::Design, Ports)>>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let key = format!("{m:?}|{place:?}|{with_u1}|{}|{direct}", part.map(|p| p.name.as_str()).unwrap_or("-"));
    let cache = CACHE.get_or_init(Default::default);
    if let Some(v) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return Ok(v);
    }
    let build = |joint: bool| -> Result<(mctas::design::Design, Ports)> {
        let mut b = Builder::new("module");
        let p = parts::module(&mut b, [0, 0, 0], place, m, with_u1 && !joint);
        if let Some(part) = part {
            // the farm's output for the caps' farm build (a chest at the module's output)
            b.d.ports.insert("output".into(), p.output);
            b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
            if m.cage.calm == crate::parts::CalmKind::Pulse {
                crate::calm::wire_plate(&mut b, part, &p, [0, 0, 0])?;
            } else if joint {
                crate::calm::wire_direct(&mut b, part, &m.cage, [0, 0, 0], place)?;
            } else {
                crate::calm::wire(&mut b, part, &p, [0, 0, 0])?;
            }
            // (dug: the site's covers cap the terrain's stray cells, its rock caps the rest)
            if place == Place::Void {
                let pads: std::collections::HashSet<[i32; 3]> = p.pad_cells.iter().map(|q| [q[0], q[1] + 1, q[2]]).collect();
                cap_spawns(&mut b, &|q| pads.contains(&q))?;
            }
            b.d.ports.clear();
            b.d.meta.clear();
        }
        Ok((b.d, p))
    };
    // the calm placed jointly with the cage when the module drives its own hide (a shared calm
    // feeds the other modules from the routed feed), kept when it costs fewer blocks (the dust
    // fork's plate pod is always the cage's own: routed only)
    let routed = build(false);
    // (the regular cage keeps U1 in the row: routed only, never raised for a joint placement)
    let direct = direct && m.cage.calm == crate::parts::CalmKind::Observer && !m.cage.regular;
    let v = match (direct && part.is_some(), routed) {
        (true, Ok(r)) => match build(true) {
            Ok(j) if j.0.blocks.len() < r.0.blocks.len() => j,
            _ => r,
        },
        (true, Err(_)) => build(true)?,
        (false, r) => r?,
    };
    if let Ok(mut c) = cache.lock() {
        c.insert(key, v.clone());
    }
    Ok(v)
}

/// Every block by the cheapest one that fills its role at any stage (designs/farms/prices.tsv):
/// stone and glass walls become cobblestone (a full block: same collision, same sight blocking,
/// both spawnable, neither burns), stone slabs cobblestone slabs. Spawn-proofing then comes from
/// the caps (`cap_spawns`), which the judge's spawn scan checks exactly.
pub fn truthful_materials(b: &mut Builder) {
    let keys: Vec<[i32; 3]> = b.d.blocks.keys().copied().collect();
    for q in keys {
        let s = b.d.blocks[&q].clone();
        let id = s.trim_start_matches("minecraft:");
        let (name, props) = match id.find('[') {
            Some(i) => (&id[..i], &id[i..]),
            None => (id, ""),
        };
        let new = match name {
            "stone" | "glass" => Some("cobblestone".to_string()),
            // a cobblestone slab is the same half block (redsim reads it from the game's state
            // table) for 3 filler instead of a smelt
            "stone_slab" => Some(format!("cobblestone_slab{props}")),
            _ => None,
        };
        if let Some(n) = new {
            b.set(q[0], q[1], q[2], &n);
        }
    }
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

/// The ring (vjeux 17:54: "Why do we not build the villager chambers around the kill chamber?"):
/// 4 cages facing inward around ONE pad, hole, kill and collection: module A built whole, its
/// cage (everything at the villagers' level and above, north of the pad) pulled `d` away from the
/// pad, and 3 copies of that cage turned a quarter, a half and three quarters around the pad's
/// centre. Each cluster's villagers stand ~d + 4.5 from the centre: separate clusters (adjacent
/// ones more than 10.6 apart from d = 3) that all see every golem on the shared pad (coupled).
fn ring_shared_pad(alone: &mctas::design::Design, p: &Ports, m: &crate::parts::Module, d: i32, n_groups: u8) -> Result<mctas::design::Design> {
    let x0 = parts::pad_x0(&m.cage, &m.pad);
    let n = if m.pad.n == 9 { 8 } else { m.pad.n as i32 };
    // the pad's centre (cell corners): x0 + n/2, z 2 + n/2
    let (cx2, cz2) = (2 * x0 + n, 2 * 2 + n);
    let is_cage = |q: &[i32; 3]| q[1] >= -1 && q[2] <= 1;
    let mut out = mctas::design::Design { name: alone.name.clone(), meta: alone.meta.clone(), ports: alone.ports.clone(), blocks: Default::default(), entities: Vec::new(), block_entities: Default::default(), ..Default::default() };
    // the cell `q` turned `t` quarters clockwise around the pad's centre (doubled coordinates:
    // a cell's centre is 2q + 1)
    let turn = |q: [i32; 3], t: u8| -> [i32; 3] {
        let (mut dx, mut dz) = (2 * q[0] + 1 - cx2, 2 * q[2] + 1 - cz2);
        for _ in 0..t {
            (dx, dz) = (-dz, dx);
        }
        [(dx + cx2 - 1) / 2, q[1], (dz + cz2 - 1) / 2]
    };
    let turn_f = |e: [f64; 3], t: u8| -> [f64; 3] {
        let (mut dx, mut dz) = (2.0 * e[0] - cx2 as f64, 2.0 * e[2] - cz2 as f64);
        for _ in 0..t {
            (dx, dz) = (-dz, dx);
        }
        [(dx + cx2 as f64) / 2.0, e[1], (dz + cz2 as f64) / 2.0]
    };
    let mut put = |out: &mut mctas::design::Design, q: [i32; 3], s: String| -> Result<()> {
        if let Some(old) = out.blocks.get(&q)
            && *old != s
        {
            bail!("ring: cages overlap at {q:?} ({old} and {s})");
        }
        out.blocks.insert(q, s);
        Ok(())
    };
    for (q, s) in &alone.blocks {
        if !is_cage(q) {
            put(&mut out, *q, s.clone())?;
        }
    }
    // N = 2: the two cages face each other across the pad (turns 0 and 2); N = 4: all four sides
    let turns: &[u8] = if n_groups == 2 { &[0, 2] } else { &[0, 1, 2, 3] };
    for &t in turns {
        for (q, s) in &alone.blocks {
            if is_cage(q) {
                let moved = [q[0], q[1], q[2] - d];
                put(&mut out, turn(moved, t), clock::canon::rotate_state(s, t))?;
            }
        }
        for (q, s) in &alone.block_entities {
            if is_cage(q) {
                out.block_entities.insert(turn([q[0], q[1], q[2] - d], t), s.clone());
            }
        }
        for e in &alone.entities {
            if e.pos[2] < 2.0 && e.pos[1] >= -1.0 {
                let moved = [e.pos[0], e.pos[1], e.pos[2] - d as f64];
                out.entities.push(mctas::design::EntitySpec { kind: e.kind.clone(), pos: turn_f(moved, t), nbt: e.nbt.clone() });
            }
        }
    }
    // the pad's and the kill's entities (none today) stay
    for e in &alone.entities {
        if !(e.pos[2] < 2.0 && e.pos[1] >= -1.0) {
            out.entities.push(e.clone());
        }
    }
    let _ = p;
    Ok(out)
}

/// The cage's regularity (ironspec iteration 17; vjeux 18:26: "please optimize the villager room,
/// the current layout is super weird"), read from the module built alone, the cage being every cell
/// at the villagers' level and above on the pods' side of the pad: the irregularities counted
/// (each pod whose front differs from the first watcher's, each villager outside the watchers'
/// row, each villager level beyond the first, the zombie's window not in front of the watchers, the
/// plate's villager outside their row, its plate not beside the window's trapdoor), the cage's
/// bounding box and its blocks.
pub struct CageShape {
    pub irregular: u32,
    pub why: Vec<String>,
    pub volume: u32,
    pub blocks: u32,
}

pub fn cage_shape(m: &crate::parts::Module, place: Place, calm_parts: &[crate::calm::Part], calm_part: u16) -> Result<CageShape> {
    let pulse;
    let part = match m.cage.calm {
        CalmKind::Observer => Some(calm_parts.get(calm_part as usize).ok_or_else(|| anyhow!("no calm part {calm_part}"))?),
        CalmKind::Pulse => {
            pulse = crate::calm::pulse_part().ok_or_else(|| anyhow!("no pulse part"))?;
            Some(&pulse)
        }
        CalmKind::Plate => None,
    };
    let (d, _) = module_alone(m, place, true, part, true)?;
    let in_cage = |q: &[i32; 3]| q[1] >= -1 && q[2] <= 1;
    let cells: Vec<([i32; 3], &String)> = d.blocks.iter().filter(|(q, _)| in_cage(q)).map(|(q, s)| (*q, s)).collect();
    let lo = |i: usize| cells.iter().map(|(q, _)| q[i]).min().unwrap_or(0);
    let hi = |i: usize| cells.iter().map(|(q, _)| q[i]).max().unwrap_or(0);
    let volume = ((hi(0) - lo(0) + 1) * (hi(1) - lo(1) + 1) * (hi(2) - lo(2) + 1)) as u32;
    let blocks = cells.iter().filter(|(_, s)| !s.trim_start_matches("minecraft:").starts_with("air")).count() as u32;
    let mut why = Vec::new();
    // the villagers: the watchers' row is the commonest (level, z) among them
    // (each villager by its bed's head: the dust fork's U1 starts on its plate in front of its bed)
    let vs: Vec<[i32; 3]> = d.blocks.iter().filter(|(q, s)| in_cage(q) && s.contains("_bed[") && s.contains("part=head")).map(|(q, _)| *q).collect();
    let mut rows: std::collections::BTreeMap<(i32, i32), u32> = Default::default();
    for v in &vs {
        *rows.entry((v[1], v[2])).or_default() += 1;
    }
    let row = rows.iter().max_by_key(|(_, n)| **n).map(|(k, _)| *k).unwrap_or((0, 0));
    let outside = vs.iter().filter(|v| (v[1], v[2]) != row).count();
    if outside > 0 {
        why.push(format!("{outside} villager(s) outside the watchers' row"));
    }
    let levels: std::collections::BTreeSet<i32> = vs.iter().map(|v| v[1]).collect();
    if levels.len() > 1 {
        why.push(format!("{} villager level(s) beyond the first", levels.len() - 1));
    }
    // the pods: each bed's front (the cells in front of its head, floor to headroom) against the
    // first watcher's
    let beds: Vec<[i32; 3]> = d.blocks.iter().filter(|(q, s)| in_cage(q) && s.contains("_bed[") && s.contains("part=head")).map(|(q, _)| *q).collect();
    let front = |h: &[i32; 3]| -> Vec<String> { (0..=2).map(|dy| d.blocks.get(&[h[0], h[1] + dy, h[2] - 1]).map(|s| s.split('[').next().unwrap_or("").to_string()).unwrap_or_else(|| "-".into())).collect() };
    let watchers: Vec<i32> = vs.iter().filter(|v| (v[1], v[2]) == row).map(|v| v[0]).collect();
    // (the first watcher's pod: the commonest front among the pods, the watchers outnumbering U1)
    let mut fronts: std::collections::BTreeMap<Vec<String>, usize> = Default::default();
    for b in &beds {
        *fronts.entry(front(b)).or_default() += 1;
    }
    let common = fronts.iter().max_by_key(|(_, n)| **n).map(|(f, _)| f.clone());
    let _ = &watchers;
    let differing = common.map(|f| beds.iter().filter(|b| front(b) != f).count()).unwrap_or(0);
    if differing > 0 {
        why.push(format!("{differing} pod(s) unlike the first"));
    }
    // the zombie's window: its hide trapdoor (north of the row, at the zombie) between the
    // watchers' columns
    let window = d.blocks.iter().find(|(q, s)| q[2] <= -5 && q[1] >= 0 && s.contains("trapdoor")).map(|(q, _)| *q);
    let (wx0, wx1) = (watchers.iter().min().copied().unwrap_or(0), watchers.iter().max().copied().unwrap_or(0));
    if let Some(w) = window
        && !(wx0..=wx1).contains(&w[0])
    {
        why.push("the zombie's window not in front of the watchers".into());
    }
    // the plate's villager and its plate (day 1's bed plate, the dust fork)
    // (the dust fork: its dust is the plate's link to the window, ironspec: met by a direct run)
    if matches!(m.cage.calm, CalmKind::Plate) {
        let plate = d.blocks.iter().find(|(q, s)| in_cage(q) && s.contains("pressure_plate")).map(|(q, _)| *q);
        if let (Some(p), Some(w)) = (plate, window)
            && [p, [p[0], p[1] - 1, p[2]]].iter().all(|q| (q[0] - w[0]).abs() + (q[1] - w[1]).abs() + (q[2] - w[2]).abs() > 1)
        {
            why.push("the plate not beside the window's trapdoor".into());
        }
    }
    let irregular = outside as u32 + (levels.len().max(1) - 1) as u32 + differing as u32 + why.iter().filter(|w| w.starts_with("the ")).count() as u32;
    Ok(CageShape { irregular, why, volume, blocks })
}

/// The access built into a dug day-1 farm: the day-1 lane's spiral staircase (mobfarm::day1's
/// dug_access: a 2 x 2 spiral dug in the rock from the chest's level up to the ground layer, its
/// first cell 3 west (or east, or south) of the chest, the corridor between them 2 high and lit, a
/// bottom slab as each layer's step (no golem or monster spawns on a half top), a torch every 8
/// layers, closed fence gates over its open cells in the ground layer: its hatch), the first
/// placement whose cells are all free and 2 or more from the farm's lava and water. Err: none fits.
fn dug_access(b: &mut Builder, chest: [i32; 3], ground: i32) -> Result<()> {
    let (cx, ay, cz) = (chest[0], chest[1], chest[2]);
    let id = |s: &str| s.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
    let liquid = |q: &[i32; 3]| b.d.blocks.get(q).is_some_and(|s| matches!(id(s).as_str(), "lava" | "water"));
    let near_liquid = |q: [i32; 3]| (-2..=2).any(|dx| (-2..=2).any(|dy| (-2..=2).any(|dz| liquid(&[q[0] + dx, q[1] + dy, q[2] + dz]))));
    // (dir: the corridor's direction from the chest; the spiral's 4 cells beyond its end, turning
    // either way; the corridor at the chest's level or one lower, its upper cell beside the chest)
    let tries: Vec<((i32, i32), i32, i32, i32)> = [(-1, 0), (1, 0), (0, 1), (0, -1)].into_iter().flat_map(|d| (0..=1).flat_map(move |low| [1, -1].into_iter().flat_map(move |side| (3..=7).map(move |reach| (d, reach, side, low))))).collect();
    for ((dx, dz), reach, side, low) in tries {
        {
            let ay = ay - low;
            let a = [cx + dx * reach, cz + dz * reach];
            // (the corridor runs into the spiral's first cell at its level: the day-1 lane's
            // dug_access opens it as the corridor's end; left as rock the player cannot leave the
            // 2-high corridor)
            let corridor: Vec<[i32; 3]> = (1..=reach).flat_map(|i| [[cx + dx * i, ay, cz + dz * i], [cx + dx * i, ay + 1, cz + dz * i]]).collect();
            let (px, pz) = if dx != 0 { (0, side) } else { (side, 0) };
            let turn = [(a[0], a[1]), (a[0] + dx, a[1] + dz), (a[0] + dx + px, a[1] + dz + pz), (a[0] + px, a[1] + pz)];
            let mut cells = corridor.clone();
            for y in ay..=ground {
                for (x, z) in turn {
                    cells.push([x, y, z]);
                }
            }
            // every cell free, 2 or more from the farm's lava and water, and walled from the
            // farm's open cells (its dug air, its water) by rock or the farm's solid blocks
            let set: std::collections::HashSet<[i32; 3]> = cells.iter().copied().collect();
            let open_cell = |n: &[i32; 3]| b.d.blocks.get(n).is_some_and(|s| matches!(id(s).as_str(), "air" | "water" | "lava" | "oak_fence_gate" | "oak_trapdoor"));
            let sealed = |q: &[i32; 3]| {
                [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]].iter().all(|d| {
                    let n = [q[0] + d[0], q[1] + d[1], q[2] + d[2]];
                    set.contains(&n) || n == chest || !open_cell(&n)
                })
            };
            let free = cells.iter().all(|q| !b.d.blocks.contains_key(q) && !near_liquid(*q) && sealed(q));
            if !free && std::env::var("FARMOPT_DEBUG").is_ok() {
                let bad = cells.iter().find(|q| b.d.blocks.contains_key(*q) || near_liquid(**q) || !sealed(q));
                if let Some(q) = bad {
                    eprintln!("access dir ({dx},{dz}) reach {reach}: {q:?} taken {} near liquid {} unsealed {}", b.d.blocks.get(q).cloned().unwrap_or_default(), near_liquid(*q), !sealed(q));
                }
            }
            if !free {
                continue;
            }
            for q in &corridor {
                b.set(q[0], q[1], q[2], "air");
            }
            for y in ay..=ground {
                let step = ((y - ay + 1).rem_euclid(4)) as usize;
                for (j, &(x, z)) in turn.iter().enumerate() {
                    if j == step {
                        b.set(x, y, z, "cobblestone_slab[type=bottom,waterlogged=false]");
                    } else if y == ground {
                        b.set(x, y, z, "oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]");
                    } else if !(y == ay && j == 0) {
                        b.set(x, y, z, "air");
                    }
                }
            }
            // the light: a torch on the corridor's floor and a wall torch every 8 layers in the
            // spiral (no monster spawns at block light 1 or more), on the rock toward the chest
            if let Some(q) = corridor.first() {
                b.set(q[0], q[1], q[2], "torch");
            }
            let facing = match (dx, dz) { (-1, _) => "west", (1, _) => "east", (_, 1) => "south", _ => "north" };
            for y in (ay + 4..ground).step_by(8) {
                let (x, z) = turn[0];
                if b.d.blocks.get(&[x, y, z]).is_some_and(|s| s.ends_with("air")) {
                    b.set(x, y, z, &format!("wall_torch[facing={facing}]"));
                }
            }
            b.d.meta.insert("access".into(), "built".into());
            return Ok(());
        }
    }
    bail!("no access fits beside the chest")
}

/// A sight blocker between every two beds side by side (ironspec iteration 12): two sleeping
/// villagers' eyes (bed + 0.2) see each other across the open cell between their pods, and five
/// villagers in a box that see each other are refused (gossip, not modeled): the cell between two
/// villagers 2 apart on x at bed height gets a wall where nothing stands.
fn close_bed_gaps(b: &mut Builder) {
    let beds: Vec<[i32; 3]> = b.d.entities.iter().filter(|e| e.kind.trim_start_matches("minecraft:") == "villager").map(|e| [e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32]).collect();
    for p in &beds {
        for q in &beds {
            // side by side on x, or on z (a turned cage: the ring)
            let mid = if q[0] == p[0] + 2 && q[1] == p[1] && q[2] == p[2] {
                [p[0] + 1, p[1], p[2]]
            } else if q[2] == p[2] + 2 && q[1] == p[1] && q[0] == p[0] {
                [p[0], p[1], p[2] + 1]
            } else {
                continue;
            };
            if !b.d.blocks.contains_key(&mid) {
                b.set(mid[0], mid[1], mid[2], parts::GLASS);
            }
        }
    }
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
    b.set(under[0], under[1], under[2], "cobblestone");
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
