//! Circuits (`circuit:` designs, docs/circuits.md): their inputs driven exactly like the judge's
//! (`circuit::Circuit::actions`: an input cell written with updates, or its lever / button used),
//! the judge's own stimulus (`circuit::stimulus`, carried by the site as `data-stimulus`) replayed
//! as the auto-demo, and the checker's phase labels.

use crate::design::{Design, Pos};
use redsim::Block;

/// How an input cell is driven (`circuit::InputHow`).
#[derive(Clone, Debug, PartialEq)]
pub enum InputHow {
    /// The cell is written with updates: `on` while the input is on, `off` (default air) while off.
    Block { on: Block, off: Block },
    /// The design's lever in this cell, used at every input edge.
    Lever,
    /// The design's button in this cell, used at every rising edge.
    Button,
    /// A value 0..15 as a signal strength (`port input.a: x y z strength`, the judge's hex kinds):
    /// the design's chest in this cell, whose contents the judge rewrites to the value's fill
    /// ([`strength_items`]): placed again (its state unchanged), then its contents set. A tap
    /// flips it between 0 and 15.
    Strength { chest: Block },
    /// A plain input driven like a real build drives it (the judge's R7, `circuit::InputHow::
    /// Driven`): the judge's stone (or the design's own receiving block) in the input cell, its
    /// delay-1 repeater in the cell `dir` from it with a stone under it, switched by a redstone
    /// block placed `2 dir` from the input cell 2 gt before each input edge.
    Driven { dir: Pos, through: bool },
    /// An item frame in this cell (`port input.d0: x y z frame`, study 14's combination locks;
    /// `circuit::InputHow::Frame`): each press turns its item one step of 8, like the judge's
    /// right-click (`Action::UseEntity`); a level step of the stimulus is one turn.
    Frame,
}

/// A plain input's on-state, written into its cell before R7 (`circuit::INPUT_ON`).
pub const INPUT_ON: &str = "minecraft:redstone_block";
/// The judge's stone in a driven input's cell and under its repeater (`circuit::DRIVEN_STONE`).
pub const DRIVEN_STONE: &str = "minecraft:smooth_stone";
/// The directions a driven input's repeater may stand in, in the judge's order
/// (`circuit::DRIVE_DIRS`).
pub const DRIVE_DIRS: [Pos; 4] = [[0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// A driven input's judge cells: its repeater, the repeater's support and the switch
/// (`circuit::driver_cells`).
pub fn driver_cells(pos: Pos, dir: Pos) -> (Pos, Pos, Pos) {
    let r = add(pos, dir);
    (r, add(r, [0, -1, 0]), add(r, dir))
}

/// The judge's repeater of a driven input, facing its direction (`circuit::Circuit::with_drivers`).
pub fn driver_repeater(dir: Pos) -> String {
    let facing = match dir {
        [0, 0, -1] => "north",
        [0, 0, 1] => "south",
        [-1, 0, 0] => "west",
        _ => "east",
    };
    format!("minecraft:repeater[delay=1,facing={facing},locked=false,powered=false]")
}

/// The box a circuit occupies during its run (`circuit::Circuit::swept_box`): its blocks' box,
/// swept along its travel by a piston extender's reach or a flying machine's distance.
fn swept_box(d: &Design) -> Option<(Pos, Pos)> {
    let mut it = d.blocks.keys();
    let first = *it.next()?;
    let (mut lo, mut hi) = (first, first);
    for p in it {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    let kind = d.meta.get("circuit").map(|s| s.as_str()).unwrap_or("");
    let first_word = kind.split_whitespace().next().unwrap_or("");
    let reach = match first_word {
        "piston-extender" => kind.split_whitespace().nth(1).and_then(|w| w.parse::<i32>().ok()).unwrap_or(1) + 1,
        "flying-machine" => d.meta.get("distance").and_then(|s| s.trim().parse::<i32>().ok()).unwrap_or(0) + 2,
        _ => 0,
    };
    let travel: Option<Pos> = d.meta.get("travel").and_then(|t| {
        let v: Vec<i32> = t.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        (v.len() == 3).then(|| [v[0], v[1], v[2]])
    });
    if let (Some(t), true) = (travel, reach > 0) {
        for i in 0..3 {
            lo[i] = lo[i].min(lo[i] + t[i] * reach);
            hi[i] = hi[i].max(hi[i] + t[i] * reach);
        }
    }
    Some((lo, hi))
}

/// Turn each plain input into a driven one like the judge (`circuit::Circuit::assign_drivers`):
/// the first direction of [`DRIVE_DIRS`] whose repeater, support and switch cells are outside the
/// box (a tiled run: free) and free of design blocks, ports and other drivers, touching no
/// observer or diode beside the repeater and no design cell beside the switch (a diode in the
/// input cell: at its back); else one through the build's air cells, its switch outside the box
/// (static designs only). An input with no way in keeps its block (the judge refuses the design).
fn assign_drivers(d: &Design, inputs: &mut [CircuitInput]) {
    let Some((lo, hi)) = swept_box(d) else { return };
    let tiled = d.meta.contains_key("tiled");
    let outside = |p: Pos| (0..3).any(|i| p[i] < lo[i] || p[i] > hi[i]);
    let mut taken: std::collections::BTreeSet<Pos> = d.ports.values().map(|(p, _)| *p).collect();
    taken.extend(d.blocks.keys().copied());
    let design_cells = taken.clone();
    let on_block = Block::parse(INPUT_ON).ok();
    let moving = d.meta.get("circuit").is_some_and(|k| k.starts_with("piston-extender") || k.starts_with("flying-machine"));
    for inp in inputs.iter_mut() {
        let InputHow::Block { on, .. } = &inp.how else { continue };
        if Some(*on) != on_block {
            continue;
        }
        let p = inp.pos;
        let here = d.blocks.get(&p).map(|s| s.as_str()).unwrap_or("");
        let back: Option<Pos> = if here.starts_with("minecraft:repeater") || here.starts_with("minecraft:comparator") {
            match here.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()) {
                Some("north") => Some([0, 0, -1]),
                Some("south") => Some([0, 0, 1]),
                Some("west") => Some([-1, 0, 0]),
                Some("east") => Some([1, 0, 0]),
                _ => None,
            }
        } else {
            None
        };
        let free = |q: Pos, taken: &std::collections::BTreeSet<Pos>| !taken.contains(&q);
        let alone = |dir: &Pos| {
            let (r, sup, sw) = driver_cells(p, *dir);
            let quiet = DRIVE_DIRS.iter().chain(&[[0, 1, 0], [0, -1, 0]]).map(|n| add(r, *n)).filter(|q| *q != p && *q != sup && *q != sw).all(|q| d.blocks.get(&q).is_none_or(|s| !(s.starts_with("minecraft:observer") || s.starts_with("minecraft:repeater") || s.starts_with("minecraft:comparator"))));
            let isolated = DRIVE_DIRS.iter().chain(&[[0, 1, 0], [0, -1, 0]]).map(|n| add(sw, *n)).filter(|q| *q != r).all(|q| !design_cells.contains(&q));
            quiet && isolated && back.is_none_or(|b| b == *dir)
        };
        let straight = DRIVE_DIRS.iter().copied().find(|dir| {
            let (r, sup, sw) = driver_cells(p, *dir);
            [r, sup, sw].iter().all(|q| free(*q, &taken) && (tiled || outside(*q))) && alone(dir)
        });
        let found = straight.map(|dir| (dir, false)).or_else(|| {
            DRIVE_DIRS
                .iter()
                .copied()
                .find(|dir| {
                    let (r, sup, sw) = driver_cells(p, *dir);
                    [r, sup, sw].iter().all(|q| free(*q, &taken) && (!moving || outside(*q))) && outside(sw) && alone(dir)
                })
                .map(|dir| (dir, true))
        });
        if let Some((dir, through)) = found {
            let (r, sup, sw) = driver_cells(p, dir);
            taken.extend([r, sup, sw]);
            inp.how = InputHow::Driven { dir, through };
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CircuitInput {
    /// The port's name after `input.` (`in` for a bare `port input:`).
    pub name: String,
    pub pos: Pos,
    pub how: InputHow,
}

/// An input's tag: its name's first three letters in capitals ("A", "CLK"); a combination
/// lock's frames by what they are (critique: "Tag the 3 dials and the 3 code frames", so the
/// page's "code frames on the right... dials on the left" can be matched): `d0` is "DIAL 1",
/// `k0` "CODE 1".
pub fn input_label(d: &Design, ci: &CircuitInput) -> String {
    let lock = d.meta.get("circuit").is_some_and(|k| k.split_whitespace().next() == Some("lock"));
    if lock && ci.how == InputHow::Frame {
        let what = match ci.name.chars().next() {
            Some('d') => Some("DIAL"),
            Some('k') => Some("CODE"),
            _ => None,
        };
        if let (Some(w), Ok(n)) = (what, ci.name[1..].parse::<u32>()) {
            return format!("{w} {}", n + 1);
        }
    }
    ci.name.to_uppercase().chars().take(3).collect()
}

/// The inputs of a circuit design, in name order like the judge's; empty for other designs.
pub fn inputs(d: &Design) -> Vec<CircuitInput> {
    if !d.meta.contains_key("circuit") {
        return Vec::new();
    }
    let mut v = Vec::new();
    for (name, (pos, args)) in &d.ports {
        let short = match name.as_str() {
            "input" => "in".to_string(),
            n => match n.strip_prefix("input.") {
                Some(s) => s.to_string(),
                None => continue,
            },
        };
        let words: Vec<&str> = args.split_whitespace().collect();
        let state = |s: &str| crate::design::normalize_state(s).ok().and_then(|n| Block::parse(&n).ok());
        let how = match words.as_slice() {
            ["lever"] => InputHow::Lever,
            ["button"] => InputHow::Button,
            ["frame"] => InputHow::Frame,
            ["strength"] => InputHow::Strength { chest: d.blocks.get(pos).and_then(|s| state(s)).unwrap_or(Block::Chest { facing: redsim::Dir::North, kind: 0, waterlogged: false }) },
            ws => {
                let on = ws.first().and_then(|s| state(s)).unwrap_or(Block::parse("minecraft:redstone_block").unwrap_or(Block::Air));
                let off = ws.get(1).and_then(|s| state(s)).unwrap_or(Block::Air);
                InputHow::Block { on, off }
            }
        };
        v.push(CircuitInput { name: short, pos: *pos, how });
    }
    v.sort_by(|a, b| a.name.cmp(&b.name));
    // R7: plain inputs driven by the judge's repeater from outside
    assign_drivers(d, &mut v);
    v
}

/// The judge's consumer (`circuit::CONSUMER`): a lamp in every air output cell.
pub const CONSUMER: &str = "minecraft:redstone_lamp[lit=false]";

/// The output cells that get the judge's consumer (`circuit::Circuit::consumer_cells`): air in
/// the design and not an input cell, for every kind read through its outputs (not piston
/// extenders and flying machines), and not where a piston may move a block in: a piston's head
/// cell, or the cell after an unbroken row of blocks in front of a piston (up to 12); with slime
/// or honey in the design, any cell within 13 in front of a piston.
pub fn consumer_cells(d: &Design) -> Vec<Pos> {
    let Some(kind) = d.meta.get("circuit") else { return Vec::new() };
    let kind = kind.split_whitespace().next().unwrap_or("");
    // (an item sorter's, an auto-smelter's, an auto-brewer's, an auto-composter's, a harvester's
    // ports and those of the kinds shown with static judge chests (`STATIC_CHEST_KINDS`) are the
    // judge's chests: no lamp there, the coordinator and site 2026-10-21)
    if matches!(kind, "piston-extender" | "flying-machine" | "shulker-loader" | "shulker-unloader" | "sorter" | "smelter" | "brewer" | "composter" | "harvester" | "mob-farm") || STATIC_CHEST_KINDS.contains(&kind) {
        return Vec::new();
    }
    let ins: Vec<Pos> = inputs(d).iter().map(|i| i.pos).collect();
    let blocks = &d.blocks;
    let sticky_blocks = blocks.values().any(|s| s.starts_with("minecraft:slime_block") || s.starts_with("minecraft:honey_block"));
    let mut lines: std::collections::BTreeSet<Pos> = std::collections::BTreeSet::new();
    for (p, s) in blocks {
        if !(s.starts_with("minecraft:piston[") || s.starts_with("minecraft:sticky_piston[")) {
            continue;
        }
        let f = match s.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()) {
            Some("north") => [0, 0, -1],
            Some("south") => [0, 0, 1],
            Some("east") => [1, 0, 0],
            Some("west") => [-1, 0, 0],
            Some("up") => [0, 1, 0],
            Some("down") => [0, -1, 0],
            _ => continue,
        };
        let at = |k: i32| [p[0] + k * f[0], p[1] + k * f[1], p[2] + k * f[2]];
        if sticky_blocks {
            lines.extend((1..=13).map(at));
            continue;
        }
        lines.insert(at(1));
        let mut k = 1;
        while k <= 12 && blocks.contains_key(&at(k)) {
            lines.insert(at(k + 1));
            k += 1;
        }
    }
    let mut v: Vec<Pos> = outputs(d).into_iter().map(|(_, p)| p).filter(|p| !ins.contains(p) && !lines.contains(p) && !blocks.contains_key(p)).collect();
    v.sort();
    v.dedup();
    v
}

/// The outputs of a circuit (`port output[.<name>]`): (label, cell), `OUT` for a bare one.
pub fn outputs(d: &Design) -> Vec<(String, Pos)> {
    let mut v: Vec<(String, Pos)> = d
        .ports
        .iter()
        .filter_map(|(n, (p, _))| match n.as_str() {
            "output" => Some(("OUT".to_string(), *p)),
            // `OUT A`: a bare letter would read like an input's tag
            n => n.strip_prefix("output.").map(|s| (format!("OUT {}", s.to_uppercase()), *p)),
        })
        .collect();
    v.sort();
    v
}

/// A shulker loader or unloader (`circuit: shulker-loader|shulker-unloader`): the viewer runs it
/// with the judge's chests on its ports ([`shulker_chests`]).
pub fn is_shulker(d: &Design) -> bool {
    d.meta.get("circuit").is_some_and(|k| matches!(k.split_whitespace().next(), Some("shulker-loader" | "shulker-unloader")))
}

/// The judge's chest on each `items`, `boxes`, `output` and `reject` port of a shulker design
/// (`circuit::shulker::CHEST`; air cells, not counted).
pub const CHEST: &str = "minecraft:chest[facing=north,type=single,waterlogged=false]";

/// The cells of a shulker design's judge chests (its `items[.k]`, `boxes[.k]`, `output[.k]`,
/// `reject[.k]` ports on air cells).
pub fn chest_cells(d: &Design) -> Vec<Pos> {
    if !is_shulker(d) {
        return Vec::new();
    }
    let mut v: Vec<Pos> = d
        .ports
        .iter()
        .filter(|(n, _)| ["items", "boxes", "output", "reject"].iter().any(|k| is_named(n, k)))
        .map(|(_, (p, _))| *p)
        .filter(|p| !d.blocks.contains_key(p))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The judge's chests of a shulker design as the viewer runs them (redsim's
/// `redstone::shulker::Chests`, the driver `mctas circuit shulker compare` checks against the real
/// game): endless (a loader's sources and every box chest kept full, the outputs emptied every
/// tick), with unstackables (27 per box: a box every few seconds; every archived design but the
/// filter also passes with them) unless the design sorts (a `reject` port: its kind's own item).
pub fn shulker_chests(d: &Design) -> Option<redsim::redstone::shulker::Chests> {
    if !is_shulker(d) {
        return None;
    }
    let mut words = d.meta.get("circuit")?.split_whitespace();
    let loader = words.next() == Some("shulker-loader");
    let stack: u32 = words.next().and_then(|n| n.parse().ok()).unwrap_or(64);
    let cells = |name: &str| -> Vec<redsim::pos::BlockPos> {
        let mut v: Vec<Pos> = d.ports.iter().filter(|(n, _)| is_named(n, name)).map(|(_, (p, _))| *p).collect();
        v.sort();
        v.into_iter().map(crate::session::bp).collect()
    };
    let rejects = cells("reject");
    let item = match (rejects.is_empty(), stack) {
        (true, _) | (false, 1) => "iron_shovel",
        (false, 16) => "snowball",
        _ => "cobblestone",
    };
    let item = redsim::redstone::container::item_id(item)?;
    Some(redsim::redstone::shulker::Chests::endless(loader, item, cells("items"), cells("boxes"), cells("output"), rejects))
}

/// An item sorter (study 13, `circuit: sorter <item>`: a slice sends every item of its kind into
/// its chest and lets every other item go on along the line).
pub fn is_sorter(d: &Design) -> bool {
    d.meta.get("circuit").is_some_and(|k| k.split_whitespace().next() == Some("sorter"))
}

/// The sorter judge's visits: every 20 gt (`circuit::shulker::SEG`, `circuit::sorter`'s `drive`).
pub const SORTER_SEG: u32 = 20;
/// How many times the viewer's sorter run repeats its judge's stream (an endless run, as far as
/// anyone watches: a pass takes minutes at a hopper's pace).
pub const SORTER_PASSES: usize = 8;

/// The kinds of the sorter judge's stream besides the sorted ones, by max stack
/// (`circuit::sorter::JUNK64`, `JUNK16`, `JUNK1`).
const SORTER_JUNK64: [&str; 5] = ["cobblestone", "stone", "iron_block", "slime_block", "heavy_weighted_pressure_plate"];
const SORTER_JUNK16: [&str; 3] = ["snowball", "ender_pearl", "bucket"];
const SORTER_JUNK1: [&str; 3] = ["iron_shovel", "wooden_shovel", "diamond_sword"];

/// The item kinds a design's own containers hold (its `nbt:` lines' `id:"..."`, without
/// `minecraft:`): a sorter's recipe and fillers, which the judge's stream leaves out.
pub fn design_kinds(d: &Design) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for nbt in d.block_entities.values() {
        let mut rest = nbt.as_str();
        while let Some(i) = rest.find("id:\"") {
            rest = &rest[i + 4..];
            let Some(j) = rest.find('"') else { break };
            let id = rest[..j].strip_prefix("minecraft:").unwrap_or(&rest[..j]).to_string();
            rest = &rest[j..];
            if !v.contains(&id) {
                v.push(id);
            }
        }
    }
    v
}

/// The sorter judge's base stream for a design sorting `kind` (`circuit::sorter::Scenario::base`:
/// its kind mixed with 64-stack, 16-stack and unstackable kinds, full and partial stacks, runs of
/// its kind back to back), the kinds the design holds left out; None when too many are.
pub fn sorter_stream(kind: &str, avoid: &[String]) -> Option<Vec<redsim::redstone::container::Stack>> {
    use redsim::redstone::container as rc;
    let k = kind.strip_prefix("minecraft:").unwrap_or(kind).to_string();
    let mut av = avoid.to_vec();
    av.push(k.clone());
    let pick = |from: &[&'static str], n: usize| -> Vec<&'static str> { from.iter().filter(|x| !av.iter().any(|a| a == *x)).take(n).copied().collect() };
    let (j, s, u) = (pick(&SORTER_JUNK64, 3), pick(&SORTER_JUNK16, 2), pick(&SORTER_JUNK1, 1));
    if j.len() < 3 || s.len() < 2 || u.is_empty() {
        return None;
    }
    let st = |id: &str, n: u32| -> Option<rc::Stack> {
        let item = rc::item_id(id)?;
        Some(rc::Stack::new(item, n.min(rc::max_stack_of(id) as u32) as u8))
    };
    [st(&k, 64), st(j[0], 64), st(&k, 20), st(s[0], 16), st(j[1], 37), st(&k, 64), st(&k, 64), st(u[0], 1), st(s[1], 5), st(&k, 33), st(j[2], 64), st(u[0], 1), st(&k, 1), st(j[0], 10), st(&k, 64)].into_iter().collect()
}

/// The sorter judge's chests on a sorter's ports (`circuit::sorter`: one on each `items`, `pass`
/// and `output` port, air cells): its source topped up from the judge's base stream
/// ([`sorter_stream`], [`SORTER_PASSES`] times over) and its sinks emptied at every visit, every
/// [`SORTER_SEG`] gt (redsim's chest driver, `redstone::shulker::Chests`, as a loader's: the
/// sources topped up from a queue, the outputs emptied).
pub fn sorter_chests(d: &Design) -> Option<redsim::redstone::shulker::Chests> {
    if !is_sorter(d) {
        return None;
    }
    let kind = d.meta.get("circuit")?.split_whitespace().nth(1)?;
    let stream = sorter_stream(kind, &design_kinds(d))?;
    let cells = |name: &str| -> Vec<redsim::pos::BlockPos> {
        let mut v: Vec<Pos> = d.ports.iter().filter(|(n, _)| is_named(n, name)).map(|(_, (p, _))| *p).collect();
        v.sort();
        v.into_iter().map(crate::session::bp).collect()
    };
    let queue: Vec<_> = (0..SORTER_PASSES).flat_map(|_| stream.iter().copied()).collect();
    let sinks: Vec<_> = cells("pass").into_iter().chain(cells("output")).collect();
    Some(redsim::redstone::shulker::Chests::judged(true, cells("items"), Vec::new(), sinks, Vec::new(), Vec::new(), queue))
}

/// An auto-smelter (study 16, `circuit: smelter N`: N furnaces fed from an input chest and a fuel
/// chest, every result into the output chest).
pub fn is_smelter(d: &Design) -> bool {
    d.meta.get("circuit").is_some_and(|k| k.split_whitespace().next() == Some("smelter"))
}

/// The kinds the viewer shows as built with their judge's chests standing (their judge's item
/// runs are not replayed yet): clk-obs's minecart stations (`cart-unloader`: the chest under its
/// unloading hopper at `port output`; `cart-loader`: its supply chest at `port items`),
/// clk-item's auto-crafters (`crafter <result>`: its ingredient chests `items`, `items.<x>` and
/// its `output`), clk-misc's item elevators (`item-elevator N`: its input chest `items` and
/// its output chest) and the site's whole minecart lines (`cart-line`: the site's view of a
/// crate::cartsys line, sorter 2026-10-22: its supply double chest `items` + `items.b` and its
/// storage double chest `output` + `output.b`).
pub const STATIC_CHEST_KINDS: [&str; 5] = ["cart-unloader", "cart-loader", "crafter", "item-elevator", "cart-line"];

/// Such a design's judge chests: its empty `items`/`items.<x>`/`output` port cells (the judge puts
/// a chest there). Empty for every other design.
pub fn static_judge_chests(d: &Design) -> Vec<Pos> {
    let kind = d.meta.get("circuit").and_then(|k| k.split_whitespace().next()).unwrap_or("");
    if !STATIC_CHEST_KINDS.contains(&kind) {
        return Vec::new();
    }
    d.ports.iter().filter(|(n, _)| is_named(n, "output") || is_named(n, "items")).map(|(_, (p, _))| *p).filter(|p| !d.blocks.contains_key(p)).collect()
}

/// [`static_judge_chests`] as the blocks the judge places: a single chest on each cell, the two
/// joined halves of a double chest where a port's `.b` partner stands beside it (a minecart line's
/// supply and storage chests, `crate::cartsys`).
pub fn static_judge_chest_blocks(d: &Design) -> Vec<(redsim::BlockPos, Block)> {
    let cells = static_judge_chests(d);
    let bp = crate::session::bp;
    let mut out: Vec<(redsim::BlockPos, Block)> = Vec::new();
    let mut joined: Vec<Pos> = Vec::new();
    for name in d.ports.keys().filter(|n| !n.ends_with(".b")) {
        if let Some((a, b)) = double_chest(d, name).filter(|(a, b)| cells.contains(a) && cells.contains(b)) {
            let (ha, hb) = double_chest_blocks(a, b);
            out.push((bp(a), ha));
            out.push((bp(b), hb));
            joined.extend([a, b]);
        }
    }
    let chest = Block::Chest { facing: redsim::Dir::North, kind: 0, waterlogged: false };
    out.extend(cells.iter().filter(|p| !joined.contains(p)).map(|p| (bp(*p), chest)));
    out
}

/// The judge's blocks a still shows standing (`Session::still`: nothing ticks): the chests of an
/// item sorter, an auto-smelter, an auto-brewer, a harvester (and its crop as planted) and the
/// kinds shown with static judge chests. (A shulker loader's chests come with its blocks.)
pub fn still_judge_blocks(d: &Design) -> Vec<(redsim::BlockPos, Block)> {
    let chest = Block::Chest { facing: redsim::Dir::North, kind: 0, waterlogged: false };
    let bp = crate::session::bp;
    let mut v: Vec<(redsim::BlockPos, Block)> = Vec::new();
    if let Some(ch) = sorter_chests(d).or_else(|| smelter_chests(d)).or_else(|| brewer_chests(d)) {
        v.extend(ch.sources.iter().chain(&ch.supplies).chain(&ch.outputs).chain(&ch.rejects).map(|p| (*p, chest)));
    }
    if let Some((chests, crops)) = harvester_ports(d) {
        v.extend(chests.iter().map(|p| (bp(*p), chest)));
        v.extend(crops.iter().filter_map(|(p, st)| Block::parse(st).ok().map(|b| (bp(*p), b))));
    } else if is_harvester(d) {
        v.extend(d.ports.iter().filter(|(n, _)| is_named(n, "output")).map(|(_, (p, _))| (bp(*p), chest)));
    }
    v.extend(static_judge_chest_blocks(d));
    // a mob farm: the judge's chest on its `port output` (an air cell; `farm::build` places it)
    if crate::mobfarm::is_mob_farm(d)
        && let Some((p, _)) = d.ports.get("output")
        && !d.blocks.contains_key(p)
    {
        v.push((bp(*p), chest));
    }
    v
}

/// A crop harvester (study 16 farms, `circuit: harvester <crop>`: it breaks every grown block of
/// its crop and sends the items into a chest, by itself).
pub fn is_harvester(d: &Design) -> bool {
    d.meta.get("circuit").is_some_and(|k| k.split_whitespace().next() == Some("harvester"))
}

/// What a view shown as built on purpose says ([`not_replayed_yet`]); `web check` counts such a
/// design as shown as built (a pass), every other unsimulated view as before.
pub const NOT_REPLAYED: &str = "this view does not replay our test exactly yet";

/// The kinds whose view does not replay their judge's run frame for frame yet: shown as built,
/// saying so, until it does (vjeux's standing rule: the 3D view shows only what redsim simulates
/// exactly, anything else as built; the coordinator 2026-10-22). An auto-composter: its judge
/// runs a row of its slices (`circuit::composter::scenario_for`), the view one. A big crop farm
/// is shown as built without its judge's speed (`Session::build`, `Options::farm_rts`).
pub fn not_replayed_yet(d: &Design) -> Option<String> {
    let kind = d.meta.get("circuit")?.split_whitespace().next()?;
    let what = match kind {
        // (a slice its judge tiles into a row, `tile:`: the page's view runs the judge's row,
        // `site::circuits`' view text, `tiled:`; a fixed composter is judged as built)
        "composter" if d.meta.contains_key("tile") => "an auto-composter",
        _ => return None,
    };
    Some(format!("{NOT_REPLAYED} ({what})"))
}

/// The random tick speed the farm judge grows its crops at (`circuit::farm::RTS`: 100 times
/// vanilla's 3, a cane growing a block about every 11 s): the view shows the judge's run.
pub const FARM_RTS: u32 = 300;
/// The farm judge's run (`circuit::farm::RUN`: 20 minutes of game time; growth stops then) and
/// the batches it steps in (`farm::BATCH`: its intruder drops fall on batch boundaries).
pub const FARM_RUN: u32 = 24000;
pub const FARM_BATCH: u32 = 500;
/// The farm judge's intruders (`circuit::farm::INTRUDERS`, the repo's ITEM INTRUSION drops): the
/// k-th dropped at k quarters of the run on every open hopper intake and above every crop column.
pub const FARM_INTRUDERS: [(&str, u8); 3] = [("minecraft:cobblestone", 1), ("minecraft:iron_ingot", 64), ("minecraft:iron_shovel", 1)];
/// At most this many intakes and this many crop columns get the intruders, evenly spread
/// (`circuit::farm::DROP_SAMPLE`).
pub const FARM_DROP_SAMPLE: usize = 8;
/// A farm of up to FARM_RTS_PLANTS plants grows at [`FARM_RTS`]; a bigger one at FARM_RTS x
/// FARM_RTS_PLANTS / plants, at least FARM_RTS_MIN (`circuit::farm::RTS_PLANTS`, `RTS_MIN`,
/// `rts_for`: its items arrive about as fast as an 8-plant farm's).
pub const FARM_RTS_PLANTS: u32 = 8;
pub const FARM_RTS_MIN: u32 = 30;
/// The farm judge empties its chests every FARM_EMPTY gt of its run and rest, when they hold a
/// crop item or an intruder (`circuit::farm::EMPTY`: it counts what it takes).
pub const FARM_EMPTY: u32 = 500;
/// After its run and rest, growth resumes while a crop item lies on a crop, in batches of
/// [`FARM_BATCH`], at most FARM_REGROW gt at [`FARM_RTS`] (scaled by FARM_RTS / rts:
/// `circuit::farm::REGROW`, `sim_finish`).
pub const FARM_REGROW: u32 = 1600;
/// The rest after a farm's run, twice (`circuit::shulker::REST`: the twin runs 2 x REST after it).
pub const FARM_REST: u32 = 400;

/// The farm judge's random tick speed for a harvester by its plant count (`circuit::farm::rts_for`).
pub fn farm_rts_for(d: &Design) -> u32 {
    let plants = d.ports.keys().filter(|n| is_named(n, "crop")).count().max(1) as u32;
    if plants <= FARM_RTS_PLANTS { FARM_RTS } else { (FARM_RTS * FARM_RTS_PLANTS / plants).max(FARM_RTS_MIN) }
}

/// The farm judge's run length at `rts` (`circuit::farm::game_ticks`: [`FARM_RUN`], up to 4 x
/// longer when its collection slowed the run below [`farm_rts_for`]).
pub fn farm_ticks(d: &Design, rts: u32) -> u32 {
    FARM_RUN * (farm_rts_for(d) / rts.max(1)).clamp(1, 4)
}

/// A harvester's crop as its full id (`minecraft:sugar_cane`; `circuit::farm::crop_id`).
pub fn farm_crop_id(d: &Design) -> Option<String> {
    let name = d.meta.get("circuit")?.split_whitespace().nth(1)?;
    Some(format!("minecraft:{}", name.trim_start_matches("minecraft:")))
}

/// The item a harvest gives (`circuit::farm::crop_item`: melon slices for a melon).
pub fn farm_crop_item(crop: &str) -> &str {
    if crop == "minecraft:melon" { "minecraft:melon_slice" } else { crop }
}

/// The judge counts every drop stack of these crops, stepping its run one gt at a time
/// (`circuit::farm::counts_drops`: the stem crops and bamboo).
pub fn farm_counts_drops(crop: &str) -> bool {
    matches!(crop, "minecraft:pumpkin" | "minecraft:melon" | "minecraft:bamboo")
}

/// A state that is the crop's block (`circuit::farm::is_crop_block`: kelp's plant too).
pub fn farm_is_crop_block(s: &str, crop: &str) -> bool {
    let is = |id: &str| s == id || s.strip_prefix(id).is_some_and(|r| r.starts_with('['));
    is(crop) || (crop.ends_with("kelp") && is("minecraft:kelp_plant"))
}

/// The box the farm judge builds a harvester in (`circuit::farm::game_run`: the watch box,
/// [`judge_box`], raised for the crop to grow: three blocks above its highest `port crop:`, and at
/// least three tall), from the design as written: its sandbox's seed and forced chunks come from
/// it (`web check` compares it with the circuit crate's box).
pub fn farm_box(d: &Design) -> Option<(Pos, Pos)> {
    let (from, mut to) = judge_box(d)?;
    let mut from = from;
    let top = d.ports.iter().filter(|(n, _)| is_named(n, "crop")).map(|(_, (p, _))| p[1] + 3).max();
    to[1] = to[1].max(top.unwrap_or(to[1]));
    from[1] = from[1].min(to[1] - 2);
    Some((from, to))
}

/// A harvester's world is the farm judge's sandbox (`circuit::farm::game_run`, its twin
/// `farm::sim_run`): the level random mirrored and the drops seeded from the box's seed
/// (`redstone::shulker::judge_sandbox`), its forced chunks ticked in the game's order, and the
/// random ticks' position generator (`Level.randValue`) where the bridge starts it for that seed
/// (`crops::RandomTicks::new`), at the sandbox's default speed (3) until the judge sets it (0 while
/// it plants and settles, [`FARM_RTS`] for its run). Set before the design is placed.
pub fn farm_sandbox(w: &mut redsim::World, from: Pos, to: Pos) {
    redsim::redstone::shulker::judge_sandbox(w, from, to);
    w.random_ticks = Some(redsim::crops::RandomTicks::new(3, redsim::redstone::shulker::build_seed(from, to, 0)));
}

/// The intakes of a design (`clock::intrusion::intakes`, which `web check` compares): the cell
/// above each hopper whose top no block closes (a full collision cube, an unextended piston, a
/// container) and that a dropped item can reach from outside the design's box through cells
/// holding no design block. In position order.
pub fn intakes(d: &Design) -> Vec<Pos> {
    let Some((lo, hi)) = d.bounds() else { return Vec::new() };
    let outside = |p: Pos| (0..3).any(|i| p[i] < lo[i] || p[i] > hi[i]);
    let closes = |state: &str| -> bool {
        let id = state.split('[').next().unwrap_or(state).trim_start_matches("minecraft:");
        if ["hopper", "chest", "trapped_chest", "barrel", "dropper", "dispenser", "crafter", "composter"].contains(&id) || id.ends_with("shulker_box") {
            return true;
        }
        match Block::parse(state) {
            Ok(b) => redsim::behavior::full_cube(&b) || matches!(b, Block::Piston { extended: false, .. }),
            Err(_) => true,
        }
    };
    let mut out = std::collections::BTreeSet::new();
    for (p, st) in &d.blocks {
        if st.split('[').next().unwrap_or(st).trim_start_matches("minecraft:") != "hopper" {
            continue;
        }
        let up = [p[0], p[1] + 1, p[2]];
        if d.blocks.get(&up).is_some_and(|s| closes(s)) {
            continue;
        }
        let mut seen: std::collections::HashSet<Pos> = std::collections::HashSet::from([up]);
        let mut queue = std::collections::VecDeque::from([up]);
        let mut reach = outside(up);
        while let Some(q) = queue.pop_front() {
            if reach {
                break;
            }
            for dd in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                let n = [q[0] + dd[0], q[1] + dd[1], q[2] + dd[2]];
                if d.blocks.contains_key(&n) || !seen.insert(n) {
                    continue;
                }
                if outside(n) {
                    reach = true;
                    break;
                }
                queue.push_back(n);
            }
        }
        if reach {
            out.insert(up);
        }
    }
    out.into_iter().collect()
}

/// The farm judge's intruder drops (`circuit::farm`'s run): (run tick, where, its summon data): at
/// k quarters of the run (on a batch boundary) the k-th of [`FARM_INTRUDERS`] on every intake
/// ([`intakes`], a quarter block up in its cell) and above every crop column (a block above the
/// crop's full height), summoned as the bridge summons them (`redsim::entity::spawn`: each item's
/// own random seeded by its summon number).
pub fn farm_drops(d: &Design, ticks: u32) -> Vec<(u32, [f64; 3], String)> {
    // the crop columns in the ports' name order, as the judge lists its stores (`crop.a`,
    // `crop.b`, ...): the summon numbers (each item's own random) follow it
    let crops: Vec<Pos> = d.ports.iter().filter(|(n, _)| is_named(n, "crop")).map(|(_, (p, _))| *p).collect();
    let full = harvester_crop(d).map_or(3, |c| c.1);
    // at most FARM_DROP_SAMPLE intakes and as many crop columns, evenly spread (`farm::drop_cells_of`)
    let pick = |v: &[Pos]| -> Vec<Pos> { if v.len() <= FARM_DROP_SAMPLE { v.to_vec() } else { (0..FARM_DROP_SAMPLE).map(|i| v[i * v.len() / FARM_DROP_SAMPLE]).collect() } };
    let cells: Vec<[f64; 3]> = pick(&intakes(d)).iter().map(|p| [p[0] as f64 + 0.5, p[1] as f64 + 0.25, p[2] as f64 + 0.5]).chain(pick(&crops).iter().map(|p| [p[0] as f64 + 0.5, (p[1] + full + 1) as f64, p[2] as f64 + 0.5])).collect();
    // (a crop whose drops the judge counts steps one gt at a time: its quarters are exact)
    let batch = if farm_crop_id(d).is_some_and(|c| farm_counts_drops(&c) || farm_crop_item(&c) != c) { 1 } else { FARM_BATCH };
    let mut v = Vec::new();
    for (k, (item, n)) in FARM_INTRUDERS.iter().enumerate() {
        let t = (ticks * (k as u32 + 1) / 4 / batch) * batch;
        for c in &cells {
            v.push((t, *c, format!("{{Item:{{id:\"{item}\",count:{n}}}}}")));
        }
    }
    v
}

/// The crop a harvester's judge plants (`circuit::farm::CROPS`): its state at age 0 and the
/// tallest it grows by itself (`farm::full_height`: sugar cane 3, bamboo 16, kelp 26, a pumpkin or
/// melon stem 2: attached to its fruit).
pub fn harvester_crop(d: &Design) -> Option<(&'static str, i32)> {
    match d.meta.get("circuit")?.split_whitespace().nth(1)?.trim_start_matches("minecraft:") {
        "sugar_cane" => Some(("minecraft:sugar_cane[age=0]", 3)),
        "bamboo" => Some(("minecraft:bamboo[age=0,leaves=none,stage=0]", 16)),
        "kelp" => Some(("minecraft:kelp[age=0]", 26)),
        // a stem crop: the stem, then the stem attached to its fruit (`farm::full_height` 2)
        "pumpkin" => Some(("minecraft:pumpkin_stem[age=0]", 2)),
        "melon" => Some(("minecraft:melon_stem[age=0]", 2)),
        "cactus" => Some(("minecraft:cactus[age=0]", 3)),
        _ => None,
    }
}

/// A harvester's judge things (`circuit::farm`): its chests (every `port output:`, an air cell)
/// and its crop planted (every `port crop:`, the crop's state at age 0). None when the design is no
/// harvester or names a crop the judge does not plant.
pub fn harvester_ports(d: &Design) -> Option<(Vec<Pos>, Vec<(Pos, &'static str)>)> {
    if !is_harvester(d) {
        return None;
    }
    let planted = harvester_crop(d)?.0;
    // in the ports' name order, as the judge lists its outputs and stores (it plants in that order)
    let cells = |name: &str| -> Vec<Pos> { d.ports.iter().filter(|(n, _)| is_named(n, name)).map(|(_, (p, _))| *p).collect() };
    Some((cells("output"), cells("crop").into_iter().map(|p| (p, planted)).collect()))
}

/// The smelter judge's base load for `n` furnaces (`circuit::smelter::Scenario::new`): 16 items a
/// furnace of six smeltables and two iron shovels, in the judge's order and stacks; and its coal,
/// one per 8 items and two a furnace, at least a stack.
pub fn smelter_load(n: u32) -> (Vec<redsim::redstone::container::Stack>, Vec<redsim::redstone::container::Stack>) {
    use redsim::redstone::container as rc;
    let n = n.max(1);
    let stacks = |id: &str, count: u32| -> Vec<rc::Stack> {
        let item = rc::item_id(id).expect("the smelter judge's items are redsim items");
        let max = rc::max_stack_of(id) as u32;
        let mut v = Vec::new();
        let mut left = count;
        while left > 0 {
            let k = left.min(max);
            v.push(rc::Stack::new(item, k as u8));
            left -= k;
        }
        v
    };
    let mut load = Vec::new();
    for (id, k) in [("raw_gold", 4 * n), ("cobblestone", 3 * n), ("iron_shovel", 1), ("sand", 3 * n), ("oak_log", 2 * n), ("kelp", 2 * n), ("iron_shovel", 1), ("raw_copper", 2 * n)] {
        load.extend(stacks(id, k));
    }
    let total: u32 = load.iter().map(|s| s.count as u32).sum();
    let coal = (total.div_ceil(SMELTER_PER_COAL) + 2 * n).max(64);
    (load, stacks("coal", coal))
}

/// A super smelter's run (`circuit::smelter::Scenario::chest`: a double input chest): a double
/// chest of raw iron (54 stacks, SMELTER_CHEST_LOAD), and the coal the load needs and a stack
/// more, never refilled. In the order of its chests: the input halves, the fuel.
pub const SMELTER_CHEST_LOAD: u32 = 54 * 64;
pub fn smelter_chest_load() -> (Vec<redsim::redstone::container::Stack>, Vec<redsim::redstone::container::Stack>) {
    use redsim::redstone::container as rc;
    let stacks = |id: &str, count: u32| -> Vec<rc::Stack> {
        let item = rc::item_id(id).expect("the smelter judge's items are redsim items");
        let max = rc::max_stack_of(id) as u32;
        (0..count.div_ceil(max)).map(|k| rc::Stack::new(item, (count - k * max).min(max) as u8)).collect()
    };
    (stacks("raw_iron", SMELTER_CHEST_LOAD), stacks("coal", SMELTER_CHEST_LOAD / SMELTER_PER_COAL + 64))
}

/// Items a coal smelts in a plain furnace (`circuit::smelter::PER_COAL`: 1600 / 200 gt).
pub const SMELTER_PER_COAL: u32 = 8;

/// The smelter judge's chests on a smelter's ports (`circuit::smelter`): the input chest on
/// `items` topped up from its load ([`smelter_load`]), the fuel chest on `fuel` with its coal, the
/// output chest on `output` emptied at every visit, every [`SORTER_SEG`] gt (redsim's chest
/// driver as a loader's: the load its queue, the coal its supply, all in one chest's 27 slots).
pub fn smelter_chests(d: &Design) -> Option<redsim::redstone::shulker::Chests> {
    if !is_smelter(d) {
        return None;
    }
    let n: u32 = d.meta.get("circuit")?.split_whitespace().nth(1)?.parse().ok()?;
    let cell = |name: &str| d.ports.iter().find(|(k, _)| is_named(k, name)).map(|(_, (p, _))| crate::session::bp(*p));
    let (items, fuel, output) = (cell("items")?, cell("fuel")?, cell("output")?);
    // a super smelter: its chest run, both input halves topped up from the one load
    if let Some((a, b)) = double_chest(d, "items") {
        let (raw, coal) = smelter_chest_load();
        let fuels: Vec<redsim::BlockPos> = match double_chest(d, "fuel") {
            Some((f, g)) => vec![crate::session::bp(f), crate::session::bp(g)],
            None => vec![fuel],
        };
        if coal.len() > 27 * fuels.len() {
            return None;
        }
        return Some(redsim::redstone::shulker::Chests::judged(true, vec![crate::session::bp(a), crate::session::bp(b)], fuels, vec![output], Vec::new(), coal, raw));
    }
    let (load, coal) = smelter_load(n);
    if coal.len() > 27 {
        return None;
    }
    Some(redsim::redstone::shulker::Chests::judged(true, vec![items], vec![fuel], vec![output], Vec::new(), coal, load))
}

/// An auto-brewer (study 21, `circuit: brewer N`: N brewing stands fed from a bottles chest, a fuel
/// chest and one chest per ingredient, every potion into the output chest).
pub fn is_brewer(d: &Design) -> bool {
    d.meta.get("circuit").is_some_and(|k| k.split_whitespace().next() == Some("brewer"))
}

/// The brewer judge's base load for `n` stands (`circuit::brewer::Scenario::new`): 4 batches a
/// stand, each 3 water bottles, 1 nether wart and 1 sugar; blaze powder: two a stand and one per
/// 20 brews (two brews a batch), at least a stack. In the order of its chests: bottles, fuel,
/// wart, sugar.
pub fn brewer_load(n: u32) -> [Vec<redsim::redstone::container::Stack>; 4] {
    use redsim::redstone::container as rc;
    let n = n.max(1);
    let stacks = |id: &str, count: u32| -> Vec<rc::Stack> {
        let item = rc::item_id(id).expect("the brewer judge's items are redsim items");
        let max = rc::max_stack_of(id) as u32;
        let mut v = Vec::new();
        let mut left = count;
        while left > 0 {
            let k = left.min(max);
            v.push(rc::Stack::new(item, k as u8));
            left -= k;
        }
        v
    };
    let batches = BREWER_BATCHES * n;
    // (`Scenario::new`: two a stand and one per 20 brews, at least a stack)
    let fuel = (2 * n + 2 * batches / BREWER_USES).max(64);
    [stacks("potion[water]", 3 * batches), stacks("blaze_powder", fuel), stacks("nether_wart", batches), stacks("sugar", batches)]
}

/// Batches a stand in the brewer judge's base run (`circuit::brewer::BATCHES`).
pub const BREWER_BATCHES: u32 = 4;

/// An auto-composter (study 22, `circuit: composter N`: a hopper line from an input chest past N
/// composters, the bone meal into chests, the rest into an overflow chest).
pub fn is_composter(d: &Design) -> bool {
    d.meta.get("circuit").is_some_and(|k| k.split_whitespace().next() == Some("composter"))
}

/// The composter judge's load for `n` composters (`circuit::composter::Scenario::new`): two rounds a
/// composter of wheat seeds 16, cobblestone 10, wheat 10, kelp 10, iron ingots 5, sugar cane 10 and
/// an iron shovel, in the judge's order.
pub fn composter_load(n: u32) -> Vec<redsim::redstone::container::Stack> {
    use redsim::redstone::container as rc;
    let st = |id: &str, k: u8| rc::Stack::new(rc::item_id(id).expect("the composter judge's items are redsim items"), k);
    let mut v = Vec::new();
    for _ in 0..2 * n.max(1) {
        v.extend([st("wheat_seeds", 16), st("cobblestone", 10), st("wheat", 10), st("kelp", 10), st("iron_ingot", 5), st("sugar_cane", 10), st("iron_shovel", 1)]);
    }
    v
}

/// The composter judge's chests on a composter's ports (`circuit::composter::drive`): the input
/// chest on `items` topped up from the load ([`composter_load`]), the overflow chest on `pass` and
/// the bone meal chests on `output` emptied at every visit, every [`SORTER_SEG`] gt. A slice shows
/// as built (one crop; the judge builds its row of four).
pub fn composter_chests(d: &Design) -> Option<redsim::redstone::shulker::Chests> {
    if !is_composter(d) {
        return None;
    }
    let n: u32 = d.meta.get("circuit")?.split_whitespace().nth(1)?.parse().ok()?;
    // (the judge's row of a tiled slice, `tiled: <copies>`: n composters a copy, its load for
    // all of them, `circuit::Circuit`'s count)
    let n = n * d.meta.get("tiled").and_then(|t| t.trim().parse::<u32>().ok()).unwrap_or(1);
    let cell = |name: &str| d.ports.iter().find(|(k, _)| is_named(k, name)).map(|(_, (p, _))| crate::session::bp(*p));
    let (items, pass) = (cell("items")?, cell("pass")?);
    let mut outs = vec![pass];
    outs.extend(d.ports.iter().filter(|(k, _)| is_named(k, "output")).map(|(_, (p, _))| crate::session::bp(*p)));
    Some(redsim::redstone::shulker::Chests::judged(true, vec![items], Vec::new(), outs, Vec::new(), Vec::new(), composter_load(n)))
}
/// Brews a blaze powder lasts (`circuit::brewer::USES`).
pub const BREWER_USES: u32 = 20;

/// The brewer judge's chests on a brewer's ports (`circuit::brewer::drive`): the bottles, fuel,
/// wart and sugar chests filled after the settle with the base load ([`brewer_load`]), the output
/// chest emptied at every visit, every [`SORTER_SEG`] gt.
pub fn brewer_chests(d: &Design) -> Option<redsim::redstone::shulker::Chests> {
    if !is_brewer(d) {
        return None;
    }
    let n: u32 = d.meta.get("circuit")?.split_whitespace().nth(1)?.parse().ok()?;
    let cell = |name: &str| d.ports.iter().find(|(k, _)| is_named(k, name)).map(|(_, (p, _))| crate::session::bp(*p));
    let cells = [cell("bottles")?, cell("fuel")?, cell("wart")?, cell("sugar")?];
    let output = cell("output")?;
    let load = brewer_load(n);
    if load.iter().any(|l| l.len() > 27) {
        return None;
    }
    Some(redsim::redstone::shulker::Chests::filled(cells.into_iter().zip(load).collect(), vec![output]))
}

/// The watch box the shulker judge builds a design in (`circuit::run::watch_box(c, 3)`: its blocks
/// and ports, plus the judge's margin): the sandbox's seed and forced chunks come from it
/// (`redsim::redstone::shulker::judge_sandbox`), so the viewer draws the judge's level random
/// (`web check` compares it with the circuit crate's box for every shulker design).
pub fn judge_box(d: &Design) -> Option<(Pos, Pos)> {
    let m = redsim::redstone::shulker::JUDGE_MARGIN;
    let mut pts = d.blocks.keys().copied().chain(d.ports.values().map(|(p, _)| *p));
    let first = pts.next()?;
    let (mut lo, mut hi) = (first, first);
    for p in pts {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    Some(([lo[0] - m, lo[1] - m, lo[2] - m], [hi[0] + m, hi[1] + m, hi[2] + m]))
}

/// Where a shulker design's box stands while it is filled or emptied: the cell in front of its
/// dispenser (the "box cell X" under the dispenser of the reference designs), when it is air: a
/// still shulker box there makes the machine readable (it is not simulated).
pub fn box_cells(d: &Design) -> Vec<Pos> {
    if !is_shulker(d) {
        return Vec::new();
    }
    let mut v = Vec::new();
    for (p, s) in &d.blocks {
        if !s.starts_with("minecraft:dispenser[") {
            continue;
        }
        let f = match s.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()) {
            Some("north") => [0, 0, -1],
            Some("south") => [0, 0, 1],
            Some("east") => [1, 0, 0],
            Some("west") => [-1, 0, 0],
            Some("up") => [0, 1, 0],
            Some("down") => [0, -1, 0],
            _ => continue,
        };
        let q = [p[0] + f[0], p[1] + f[1], p[2] + f[2]];
        if !d.blocks.contains_key(&q) && !d.ports.values().any(|(c, _)| *c == q) {
            v.push(q);
        }
    }
    v.sort();
    v.dedup();
    v
}

/// `name` or `name.<k>`.
pub(crate) fn is_named(port: &str, name: &str) -> bool {
    port == name || port.strip_prefix(name).is_some_and(|r| r.starts_with('.'))
}

/// A display's face (clk-obs's 0-9 display: 13 judge lamps in one plane in front of the
/// decoder): the way it faces, when a design has at least 7 output ports in one plane and every
/// block on one side of it (the face is read from the other side). A display's per-lamp OUT tags
/// are hidden by default and its camera reads the face from the front (the coordinator
/// 2026-10-22: "OUT AF, OUT A, … on every lamp"; the view seen obliquely).
pub fn display_face(d: &Design) -> Option<[f32; 3]> {
    let outs: Vec<Pos> = d.ports.iter().filter(|(n, _)| is_named(n, "output")).map(|(_, (p, _))| *p).collect();
    if outs.len() < 7 {
        return None;
    }
    // (a standing face: its plane across x or z, its lamps spread up and sideways in it, not a
    // row of outputs)
    let k = [0usize, 2].into_iter().find(|&k| outs.iter().all(|p| p[k] == outs[0][k]))?;
    let spread = |a: usize| outs.iter().map(|p| p[a]).max().unwrap_or(0) - outs.iter().map(|p| p[a]).min().unwrap_or(0);
    if spread(1) < 2 || spread(2 - k) < 2 {
        return None;
    }
    let plane = outs[0][k];
    // (the face's own walls stand in its plane: every block in it or behind it)
    let way = if d.blocks.keys().all(|p| p[k] >= plane) && d.blocks.keys().any(|p| p[k] > plane) {
        -1.0
    } else if d.blocks.keys().all(|p| p[k] <= plane) && d.blocks.keys().any(|p| p[k] < plane) {
        1.0
    } else {
        return None;
    };
    let mut v = [0.0f32; 3];
    v[k] = way;
    Some(v)
}

/// The double chest a judge places joined on a port and its `.b` port beside it (`port items:` +
/// `port items.b:`, sorter's super smelters, `circuit::smelter`): (the first half's cell, the
/// second's), when the two cells stand side by side.
pub fn double_chest(d: &Design, name: &str) -> Option<(Pos, Pos)> {
    let a = d.ports.get(name)?.0;
    let b = d.ports.get(&format!("{name}.b"))?.0;
    (a[1] == b[1] && (a[0] - b[0]).abs() + (a[2] - b[2]).abs() == 1).then_some((a, b))
}

/// The two halves of a double chest on cells `a` and `b`, as the judge places them
/// (`circuit::smelter`): facing north when they stand along x, east along z; the LEFT half is the
/// one whose partner lies on its facing's clockwise side (`ChestBlock.getConnectedDirection`).
pub fn double_chest_blocks(a: Pos, b: Pos) -> (redsim::Block, redsim::Block) {
    use redsim::{Block, Dir};
    let (facing, a_left) = match [b[0] - a[0], b[1] - a[1], b[2] - a[2]] {
        [1, 0, 0] => (Dir::North, true),
        [-1, 0, 0] => (Dir::North, false),
        [0, 0, 1] => (Dir::East, true),
        _ => (Dir::East, false),
    };
    let half = |left: bool| Block::Chest { facing, kind: if left { 1 } else { 2 }, waterlogged: false };
    (half(a_left), half(!a_left))
}

/// The tags a circuit's viewer shows on its ports besides the inputs: (label, cell, colour). The
/// outputs (OUT, orange), a shulker design's ITEMS and BOXES chests, a smelter's FUEL chest, a
/// brewer's BOTTLES, WART and SUGAR chests, a BUD switch's TARGET cells.
pub fn port_tags(d: &Design) -> Vec<(String, Pos, &'static str)> {
    if !d.meta.contains_key("circuit") {
        return Vec::new();
    }
    // (a static-chest kind's output is the judge's chest: the second half of a double one, a
    // minecart line's storage chest, is the same chest, no tag of its own)
    let kind = d.meta.get("circuit").and_then(|k| k.split_whitespace().next()).unwrap_or("");
    let joined_out = STATIC_CHEST_KINDS.contains(&kind).then(|| double_chest(d, "output").map(|(_, b)| b)).flatten();
    let mut v: Vec<(String, Pos, &'static str)> = outputs(d).into_iter().filter(|(_, p)| joined_out != Some(*p)).map(|(l, p)| (l, p, "#ffb04a")).collect();
    for (name, label, colour) in [("items", "ITEMS", "#ffd27a"), ("fuel", "FUEL", "#c9c9c9"), ("pass", "PASS", "#9ecbff"), ("boxes", "BOXES", "#d9a8ff"), ("reject", "REJECT", "#ff8f8f"), ("target", "TARGET", "#5ad1f0"), ("bottles", "BOTTLES", "#9ecbff"), ("wart", "WART", "#ff8f8f"), ("sugar", "SUGAR", "#f2f2f2"), ("swap", "PRESS", "#7cf09a"), ("bedrock", "BEDROCK", "#b4b4ff")] {
        let mut ports: Vec<(&String, Pos)> = d.ports.iter().filter(|(n, _)| is_named(n, name)).map(|(n, (p, _))| (n, *p)).collect();
        ports.sort();
        // (the second half of a double chest the judge places joined is the same chest: no tag
        // and no outline of its own; the coordinator 2026-10-22, "ITEMS B" sat on an empty frame)
        let joined = double_chest(d, name).map(|(_, b)| b);
        for (n, p) in ports {
            if joined == Some(p) && n.as_str() == format!("{name}.b") {
                continue;
            }
            let tag = match n.strip_prefix(name).and_then(|r| r.strip_prefix('.')) {
                Some(k) => format!("{label} {}", k.to_uppercase()),
                None => label.to_string(),
            };
            v.push((tag, p, colour));
        }
    }
    v
}

/// One tag over a group of a circuit's ports: its label, its cells, its colour.
#[derive(Clone, Debug, PartialEq)]
pub struct PortGroup {
    pub label: String,
    pub cells: Vec<Pos>,
    pub colour: &'static str,
}

/// The tags of a circuit's ports (besides the inputs): one per port ([`port_tags`]), except in a
/// shulker design, where the same kind's chests that stand together (within 2 cells of one
/// another) share ONE tag without letters (`ITEMS A`..`ITEMS F` in two rows of three: two
/// `ITEMS` tags; the two output chests far apart: two `OUT` tags), and in a bedrock breaker,
/// whose repeated swap pistons and target bedrock share one PRESS tag and one BEDROCK tag (each
/// cell keeps its coloured outline, so a row stays readable at phone size).
pub fn port_groups(d: &Design) -> Vec<PortGroup> {
    let tags = port_tags(d);
    let bedrock_breaker = d.meta.get("circuit").is_some_and(|k| k.split_whitespace().next() == Some("bedrock-breaker"));
    if bedrock_breaker {
        let mut groups: Vec<PortGroup> = Vec::new();
        for (label, colour) in [("PRESS", "#7cf09a"), ("BEDROCK", "#b4b4ff")] {
            let mut cells: Vec<Pos> = tags.iter().filter(|(l, _, c)| *c == colour && l.split(' ').next() == Some(label)).map(|(_, p, _)| *p).collect();
            cells.sort();
            cells.dedup();
            if !cells.is_empty() {
                groups.push(PortGroup { label: label.to_string(), cells, colour });
            }
        }
        groups.extend(tags.into_iter().filter(|(_, _, c)| *c != "#7cf09a" && *c != "#b4b4ff").map(|(label, p, colour)| PortGroup { label, cells: vec![p], colour }));
        return groups;
    }
    if !is_shulker(d) {
        return tags.into_iter().map(|(label, p, colour)| PortGroup { label, cells: vec![p], colour }).collect();
    }
    let mut groups: Vec<PortGroup> = Vec::new();
    for (base, label, colour) in [("output", "OUT", "#ffb04a"), ("items", "ITEMS", "#ffd27a"), ("boxes", "BOXES", "#d9a8ff"), ("reject", "REJECT", "#ff8f8f")] {
        // each port's cell and letter (`items.a` -> A; a bare `items` port has none)
        let mut ports: Vec<(Pos, Option<String>)> = d.ports.iter().filter(|(n, _)| is_named(n, base)).map(|(n, (p, _))| (*p, n.strip_prefix(base).and_then(|r| r.strip_prefix('.')).map(|k| k.to_uppercase()))).collect();
        ports.sort();
        ports.dedup_by(|a, b| a.0 == b.0);
        // clusters: cells within 2 (Chebyshev) of a cell of the cluster
        let mut left = ports;
        while let Some(first) = left.pop() {
            let mut cluster = vec![first];
            loop {
                let near: Vec<usize> = (0..left.len()).filter(|&i| cluster.iter().any(|c| (0..3).all(|k| (c.0[k] - left[i].0[k]).abs() <= 2))).collect();
                if near.is_empty() {
                    break;
                }
                for i in near.into_iter().rev() {
                    cluster.push(left.remove(i));
                }
            }
            cluster.sort();
            // its letters: `OUT A`, `ITEMS A–C` (a run), `ITEMS A,C`
            let mut letters: Vec<String> = cluster.iter().filter_map(|(_, k)| k.clone()).collect();
            letters.sort();
            let run = letters.len() > 2 && letters.iter().all(|l| l.len() == 1) && letters.windows(2).all(|w| w[1].as_bytes()[0] == w[0].as_bytes()[0] + 1);
            let name = if letters.len() != cluster.len() || letters.is_empty() {
                label.to_string()
            } else if run {
                format!("{label} {}–{}", letters[0], letters[letters.len() - 1])
            } else {
                format!("{label} {}", letters.join(","))
            };
            groups.push(PortGroup { label: name, cells: cluster.into_iter().map(|(p, _)| p).collect(), colour });
        }
    }
    groups.sort_by(|a, b| (a.label.as_str(), a.cells[0]).cmp(&(b.label.as_str(), b.cells[0])));
    groups
}

/// `1` -> `1st`.
fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// A tick count in the site's words: `6 ticks (0.3 s)`, `1 tick (0.05 s)`, `1,300 ticks (1 min
/// 5 s)` (critique: the viewer never says "gt"; the site's `real_time` rule: seconds with up to 2
/// decimals, minutes past 60 s).
pub fn ticks_words(n: u32) -> String {
    let num = |x: f64| -> String {
        let s = format!("{x:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let secs = n as f64 / 20.0;
    let time = if secs < 60.0 {
        format!("{} s", num(secs))
    } else {
        let mins = (secs / 60.0).floor();
        let rest = secs - mins * 60.0;
        if rest > 0.0 { format!("{mins} min {} s", num(rest)) } else { format!("{mins} min") }
    };
    format!("{} tick{} ({time})", crate::labels::thousands(n), if n == 1 { "" } else { "s" })
}

/// A demo phase label (the checker's: `10`, `pulse 3`, `cycle 2`, `place 1 t`...) in words for
/// a visitor (`A on, B off`, `an input pulse of 3 ticks (0.15 s)`, `2nd cycle`, `a block placed
/// on TARGET`). `kind`: the design's circuit kind; `inputs`: its input names. Unknown labels stay
/// as they are.
pub fn phase_words(kind: &str, label: &str, inputs: &[String]) -> String {
    let kind = kind.split_whitespace().next().unwrap_or("");
    let w: Vec<&str> = label.split_whitespace().collect();
    let num = |i: usize| w.get(i).and_then(|s| s.parse::<u32>().ok());
    let name = |i: usize| inputs.get(i).map(|n| n.to_uppercase()).unwrap_or_else(|| format!("input {}", i + 1));
    let bits = |s: &str| !inputs.is_empty() && s.len() == inputs.len() && s.chars().all(|c| c == '0' || c == '1');
    // a gate's input states
    if w.len() == 1 && bits(w[0]) {
        return w[0].chars().enumerate().map(|(i, c)| format!("{} {}", name(i), if c == '1' { "on" } else { "off" })).collect::<Vec<_>>().join(", ");
    }
    if w.len() == 2 && bits(w[0]) && w[1] == "(rest)" {
        return "all inputs off".into();
    }
    match (w.as_slice(), num(1)) {
        (["press", _], Some(n)) => return format!("{} button press", ordinal(n)),
        (["pulse", _], Some(n)) if matches!(kind, "counter" | "tnt-duper-triggered") => return format!("{} input pulse", ordinal(n)),
        (["pulse", _], Some(n)) => return format!("an input pulse of {}", ticks_words(n)),
        (["gap", _], Some(g)) => return format!("two pulses {} apart", ticks_words(g)),
        (["pair", _], Some(g)) => return format!("two pulses {} apart", ticks_words(g)),
        (["cycle", _], Some(n)) => return format!("{} cycle: out and back", ordinal(n)),
        (["train", _], Some(r)) => return format!("a pulse train: {}", ticks_words(r)),
        (["clock", _], Some(p)) => return format!("a clock, every {}", ticks_words(p)),
        (["on", _], Some(_)) | (["on"], _) => return "input on".into(),
        (["off", _], Some(_)) | (["off"], _) => return "input off".into(),
        (["idle"], _) => return "waiting".into(),
        (["flight"], _) => return "flying".into(),
        (["run"], _) => return "running".into(),
        _ => {}
    }
    // T flip-flops, RS latches: `A 1 pulse 4`, `B pulse 4`, `C period 8`, `S pulse 2`
    let n = |s: &str| s.parse::<u32>().ok();
    match w.as_slice() {
        [inp, k, "pulse", len] if n(k).is_some() && n(len).is_some() => return format!("{inp}: {} pulse, {}", ordinal(n(k).unwrap_or(1)), ticks_words(n(len).unwrap_or(0))),
        [inp, "pulse", len] if n(len).is_some() => return format!("{inp}: a pulse of {}", ticks_words(n(len).unwrap_or(0))),
        [inp, "period", p] if n(p).is_some() => return format!("{inp}: a clock, every {}", ticks_words(n(p).unwrap_or(0))),
        _ => {}
    }
    // a BUD switch's judge actions
    if kind == "bud" {
        let what = match w.first().copied() {
            Some("place") => Some("a block placed on TARGET"),
            Some("remove") => Some("the block on TARGET removed"),
            Some("state") => Some("TARGET's block changes state"),
            Some("shape") => Some("a shape update at TARGET"),
            Some("update") => Some("a block update at TARGET"),
            Some("setup") => Some("setting up"),
            Some("relatch") => Some("re-arming"),
            Some("rearm") => Some("re-arm test"),
            Some("probe") => match w.get(2).copied() {
                Some("place") => Some("a block placed next to the build"),
                Some("remove") => Some("a block removed next to the build"),
                _ => Some("a block update next to the build"),
            },
            _ => None,
        };
        if let Some(s) = what {
            return s.into();
        }
    }
    // `d=1 e=0`, `M 2 d=1 clk=0`: the levels in words
    if w.iter().any(|t| t.contains('=')) {
        let parts: Vec<String> = w
            .iter()
            .filter_map(|t| {
                let (k, v) = t.split_once('=')?;
                let k = if k == "clk" { "clock".to_string() } else { k.to_uppercase() };
                Some(format!("{k} {}", if v == "1" { "on" } else { "off" }))
            })
            .collect();
        if !parts.is_empty() {
            return parts.join(", ");
        }
    }
    label.to_string()
}
/// target and footprint actions), from a page's `data-events`.
#[derive(Clone, Debug, PartialEq)]
pub enum EventOp {
    /// Place this state with updates (flags 3, like a player).
    Place(Block),
    /// Remove the block with updates.
    Clear,
    /// Change to this state with shape updates only (flags 2).
    Shape(Block),
    /// A pure block update around the cell (`updateNeighborsAt`), nothing changes.
    Update,
}

/// Parse `data-events`: `<frame> <op> <x> <y> <z> [<state>]; ...` (design coordinates, frames
/// counted like the stimulus: after placement), op = place | clear | shape | update.
pub fn parse_events(text: &str) -> Result<Vec<(u32, Pos, EventOp)>, String> {
    let mut v = Vec::new();
    for part in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let w: Vec<&str> = part.split_whitespace().collect();
        let bad = || format!("event {part:?}: want `<frame> place|clear|shape|update <x> <y> <z> [<state>]`");
        if w.len() < 5 {
            return Err(bad());
        }
        let t: u32 = w[0].parse().map_err(|_| bad())?;
        let n = |k: usize| w[k].parse::<i32>().map_err(|_| bad());
        let p = [n(2)?, n(3)?, n(4)?];
        let state = |k: usize| -> Result<Block, String> {
            let s = w.get(k).ok_or_else(bad)?;
            let s = crate::design::normalize_state(s).map_err(|e| format!("event {part:?}: {e}"))?;
            Block::parse(&s).map_err(|e| format!("event {part:?}: {e}"))
        };
        let op = match w[1] {
            "place" => EventOp::Place(state(5)?),
            "clear" => EventOp::Clear,
            "shape" => EventOp::Shape(state(5)?),
            "update" => EventOp::Update,
            _ => return Err(bad()),
        };
        v.push((t, p, op));
    }
    v.sort_by_key(|(t, _, _)| *t);
    Ok(v)
}

/// The inputs' schedule (`circuit::stimulus::Schedule`): per input (in [`inputs`] order) its
/// edges (frame, on), alternating from on; frame t = after t ticks from placement, an edge applied
/// between ticks before frame t is shown. `ticks`: the last frame of the run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Schedule {
    pub edges: Vec<Vec<(u32, bool)>>,
    pub ticks: u32,
    /// The judge's actions at cells between ticks (`data-events`: a BUD switch's target and
    /// footprint probes): frame, cell, what.
    pub events: Vec<(u32, Pos, EventOp)>,
    /// Strength inputs' values (`data-levels`, `circuit::stimulus::Schedule::levels`): frame,
    /// input index, value 0..15.
    pub levels: Vec<(u32, usize, u8)>,
}

/// The item a strength input's chest is filled with (`circuit::compute::STRENGTH_ITEM`).
pub const STRENGTH_ITEM: &str = "minecraft:cobblestone";

/// A strength input's chest contents for a value (`circuit::compute::strength_items`): the fewest
/// items of [`STRENGTH_ITEM`] whose comparator signal is the value, as SNBT.
pub fn strength_items(v: u8) -> String {
    let v = v.min(15) as u64;
    let n = if v == 0 { 0 } else { ((v - 1) * 1728).div_ceil(14).max(1) };
    let mut items = Vec::new();
    let mut left = n;
    let mut slot = 0;
    while left > 0 {
        let k = left.min(64);
        items.push(format!("{{Slot:{slot}b,id:\"{STRENGTH_ITEM}\",count:{k}}}"));
        left -= k;
        slot += 1;
    }
    format!("{{Items:[{}]}}", items.join(","))
}

/// Parse `data-levels`: `<frame> <input> <value>; ...` (an input by its name, as in
/// `data-stimulus`).
pub fn parse_levels(text: &str, names: &[String]) -> Result<Vec<(u32, usize, u8)>, String> {
    let mut v = Vec::new();
    for part in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let w: Vec<&str> = part.split_whitespace().collect();
        let [t, name, value] = w.as_slice() else { return Err(format!("level `{part}`: want <frame> <input> <value>")) };
        let t: u32 = t.parse().map_err(|_| format!("level `{part}`: bad frame"))?;
        let i = names.iter().position(|n| n == name).ok_or_else(|| format!("level `{part}`: no input `{name}`"))?;
        let value: u8 = value.parse().ok().filter(|x| *x <= 15).ok_or_else(|| format!("level `{part}`: the value is 0..15"))?;
        v.push((t, i, value));
    }
    Ok(v)
}

/// Frames the circuit stands at rest before the first input (`circuit::stimulus::SETTLE`).
pub const SETTLE: u32 = 40;

/// Parse a stimulus (`circuit::stimulus::parse`): `;`-separated parts, each `[<input>] <edges>`
/// with edges `10+ 14-` or `pulse L every T x K [from S]`; the input may be left out when there
/// is one. `ticks` = the last edge + `tail`.
pub fn parse_stimulus(text: &str, names: &[String], tail: u32) -> Result<Schedule, String> {
    let mut s = Schedule { edges: vec![Vec::new(); names.len()], ticks: 0, events: Vec::new(), levels: Vec::new() };
    for part in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let words: Vec<&str> = part.split_whitespace().collect();
        let (i, rest) = match names.iter().position(|n| n == words[0]) {
            Some(i) => (i, &words[1..]),
            None if names.len() == 1 => (0, &words[..]),
            None => return Err(format!("stimulus {part:?}: name the input ({})", names.join(", "))),
        };
        if rest.first() == Some(&"pulse") {
            let num = |k: usize| -> Result<u32, String> { rest.get(k).and_then(|w| w.parse::<u32>().ok()).ok_or_else(|| format!("stimulus {part:?}: want `pulse L every T x K [from S]`")) };
            if rest.get(2) != Some(&"every") || rest.get(4) != Some(&"x") {
                return Err(format!("stimulus {part:?}: want `pulse L every T x K [from S]`"));
            }
            let (l, every, k) = (num(1)?, num(3)?, num(5)?);
            let from = if rest.get(6) == Some(&"from") { num(7)? } else { SETTLE };
            if l == 0 || every <= l {
                return Err(format!("stimulus {part:?}: want 0 < L < T"));
            }
            for j in 0..k {
                s.edges[i].push((from + j * every, true));
                s.edges[i].push((from + j * every + l, false));
            }
        } else {
            for w in rest {
                let (t, on) = if let Some(t) = w.strip_suffix('+') {
                    (t, true)
                } else if let Some(t) = w.strip_suffix('-') {
                    (t, false)
                } else {
                    return Err(format!("stimulus {part:?}: an edge is <frame>+ or <frame>-, got {w:?}"));
                };
                s.edges[i].push((t.parse().map_err(|_| format!("stimulus {part:?}: bad frame {t:?}"))?, on));
            }
        }
    }
    for e in &mut s.edges {
        e.sort_by_key(|(t, _)| *t);
    }
    s.ticks = s.edges.iter().flatten().map(|(t, _)| *t).max().unwrap_or(0) + tail;
    Ok(s)
}

/// `from-to label; ...` (the checker's phases, `data-phases`).
pub fn parse_phases(text: &str) -> Vec<(u32, u32, String)> {
    text.split(';')
        .filter_map(|p| {
            let p = p.trim();
            let (range, label) = p.split_once(' ')?;
            let (a, b) = range.split_once('-')?;
            Some((a.trim().parse().ok()?, b.trim().parse().ok()?, label.trim().to_string()))
        })
        .collect()
}

/// Kinds whose inputs are pulses (a tap = a short pulse); the others (gates, the D latch, piston
/// extenders) toggle (a tap flips it).
pub fn pulse_kind(d: &Design) -> bool {
    let k = d.meta.get("circuit").map(|s| s.as_str()).unwrap_or("");
    let k = k.split_whitespace().next().unwrap_or("");
    matches!(k, "rising-edge" | "falling-edge" | "dual-edge" | "pulse-limiter" | "pulse-extender" | "tff" | "delay" | "rs-latch" | "counter")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shulker loader gets the judge's chests on its items / boxes / output ports, a box under
    /// its dispenser, its port tags, and no consumer lamp; a BUD switch's target is tagged; the
    /// events parse like the site writes them.
    /// A strength input (the hex kinds): the design's chest, filled like the judge
    /// (`compute::strength_items`: 0 empty, 15 a full chest of 1728), its values parsed from
    /// `data-levels`.
    #[test]
    fn strength_inputs() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/calibration/hex-gt.txt")).unwrap();
        let d = Design::parse(&text).unwrap();
        let v = inputs(&d);
        assert_eq!(v.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert!(v.iter().all(|i| matches!(i.how, InputHow::Strength { chest: Block::Chest { .. } })), "{v:?}");
        assert_eq!(strength_items(0), "{Items:[]}");
        assert_eq!(strength_items(1), "{Items:[{Slot:0b,id:\"minecraft:cobblestone\",count:1}]}");
        assert_eq!(strength_items(15).matches("count:64").count(), 27);
        let names = vec!["a".to_string(), "b".to_string()];
        assert_eq!(parse_levels("40 a 5; 40 b 3; 80 a 0", &names).unwrap(), vec![(40, 0, 5), (40, 1, 3), (80, 0, 0)]);
        assert!(parse_levels("40 c 5", &names).is_err() && parse_levels("40 a 16", &names).is_err());
    }

    #[test]
    fn shulker_and_bud_ports() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/shulker/traces/exact/shulker-ref-loader-1.txt")).unwrap();
        let d = Design::parse(&text).unwrap();
        assert!(is_shulker(&d));
        assert_eq!(chest_cells(&d), vec![[0, 2, 1], [0, 3, -1], [2, 0, 0]]);
        assert!(consumer_cells(&d).is_empty());
        let boxes = box_cells(&d);
        assert_eq!(boxes.len(), 1, "{boxes:?}");
        let disp = d.blocks.iter().find(|(_, s)| s.starts_with("minecraft:dispenser[facing=down")).map(|(p, _)| *p).unwrap();
        assert_eq!(boxes[0], [disp[0], disp[1] - 1, disp[2]]);
        let tags: Vec<String> = port_tags(&d).into_iter().map(|(l, _, _)| l).collect();
        assert_eq!(tags, vec!["OUT", "ITEMS", "BOXES"]);
        // rail2: two rows of three ITEMS chests 6 apart, two OUT chests: 2 ITEMS + 2 OUT + BOXES
        let rail2 = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/shulker/archive/shulker-speed-unloader-rail2.txt")).unwrap();
        let g = port_groups(&Design::parse(&rail2).unwrap());
        let labels: Vec<(&str, usize)> = g.iter().map(|g| (g.label.as_str(), g.cells.len())).collect();
        assert_eq!(labels, vec![("BOXES", 1), ("ITEMS A–C", 3), ("ITEMS D–F", 3), ("OUT A", 1), ("OUT B", 1)]);
        assert_eq!(phase_words("gate and", "10", &["a".into(), "b".into()]), "A on, B off");
        assert_eq!(phase_words("gate and", "00 (rest)", &["a".into(), "b".into()]), "all inputs off");
        assert_eq!(phase_words("pulse-limiter", "pulse 3", &["in".into()]), "an input pulse of 3 ticks (0.15 s)");
        assert_eq!(phase_words("tff", "A 2 pulse 4", &["a".into()]), "A: 2nd pulse, 4 ticks (0.2 s)");
        assert_eq!(phase_words("pulsemult", "clock 1300", &["in".into()]), "a clock, every 1,300 ticks (1 min 5 s)");
        assert_eq!(ticks_words(1), "1 tick (0.05 s)");
        assert_eq!(ticks_words(6), "6 ticks (0.3 s)");
        assert_eq!(phase_words("counter 2", "pulse 2", &["in".into()]), "2nd input pulse");
        assert_eq!(phase_words("piston-extender 2", "cycle 2", &["in".into()]), "2nd cycle: out and back");
        assert_eq!(phase_words("bud", "place 1 t", &[]), "a block placed on TARGET");
        assert_eq!(phase_words("d-latch", "d=1 e=0", &["d".into(), "e".into()]), "D on, E off");
        assert_eq!(phase_words("x", "something new", &[]), "something new");
        let bud = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/bud/archive/bud-slime-qc.txt")).unwrap();
        let tags: Vec<(String, Pos)> = port_tags(&Design::parse(&bud).unwrap()).into_iter().map(|(l, p, _)| (l, p)).collect();
        assert_eq!(tags, vec![("OUT".to_string(), [1, 3, 0]), ("TARGET".to_string(), [1, 0, 0])]);
        let ev = parse_events("60 update 1 0 0; 41 place 1 0 0 minecraft:stone; 50 shape 1 0 0 oak_trapdoor[open=true]; 45 clear 1 0 0").unwrap();
        assert_eq!(ev.iter().map(|e| e.0).collect::<Vec<_>>(), vec![41, 45, 50, 60]);
        assert!(matches!(ev[0].2, EventOp::Place(_)) && ev[1].2 == EventOp::Clear && matches!(ev[2].2, EventOp::Shape(_)) && ev[3].2 == EventOp::Update);
        assert!(parse_events("41 smash 1 0 0").is_err());
    }

    #[test]
    fn bedrock_breaker_port_groups_share_tags() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/bedrock/archive/bedrock-breaker-row8.txt")).unwrap();
        let d = Design::parse(&text).unwrap();
        assert_eq!(port_tags(&d).len(), 16, "each target and piston still gets its own outline");
        let groups = port_groups(&d);
        assert_eq!(groups.iter().map(|g| (g.label.as_str(), g.cells.len())).collect::<Vec<_>>(), vec![("PRESS", 8), ("BEDROCK", 8)]);
    }

    #[test]
    fn stimulus_like_the_judge() {
        let names = vec!["a".to_string(), "b".to_string()];
        let s = parse_stimulus("a 40+ 44-; b pulse 2 every 10 x 2 from 50", &names, 20).unwrap();
        assert_eq!(s.edges[0], vec![(40, true), (44, false)]);
        assert_eq!(s.edges[1], vec![(50, true), (52, false), (60, true), (62, false)]);
        assert_eq!(s.ticks, 82);
        assert_eq!(parse_phases("40-60 pulse 4 gt; 60-90 ab 01"), vec![(40, 60, "pulse 4 gt".to_string()), (60, 90, "ab 01".to_string())]);
    }

    #[test]
    fn inputs_in_name_order() {
        let d = Design::parse("name: g\ncircuit: gate and\nport input.b: 2 0 0\nport input.a: 0 0 0\nport output: 1 1 0\npalette:\n  S minecraft:stone\nlayer 0:\n  .S.\n").unwrap();
        let v = inputs(&d);
        assert_eq!(v.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
        // R7: driven by the judge's repeater from outside, the first direction out of the build
        assert!(v.iter().all(|i| i.how == InputHow::Driven { dir: [0, 0, -1], through: false }), "{v:?}");
        assert_eq!(driver_cells(v[0].pos, [0, 0, -1]), ([0, 0, -1], [0, -1, -1], [0, 0, -2]));
        assert_eq!(outputs(&d), vec![("OUT".to_string(), [1, 1, 0])]);
    }
}
