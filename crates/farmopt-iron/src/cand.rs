//! The iron farm's candidates (designs/briefs/farm-optimizer.md, "The iron grammar"): a module
//! (cage, pad, kill), how many modules and how they tile, what they share, the calm's catalog
//! part, the placement. Exact seeds (the ironfarm and day-1 lanes' hand designs, rebuilt by their
//! own generators) are candidates too, for calibration: they never enter a front.

use crate::parts::{CalmKind, Cage, Kill, Module, Pad, Place};
use farmopt::spec::Spec;

/// How the modules tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tiling {
    /// Side by side on x, `pitch` apart (module i at x = i * pitch).
    Row(u8),
    /// Stacked on y, `pitch` apart.
    Stack(u8),
}

impl Tiling {
    pub fn pitch(self) -> i32 {
        match self {
            Tiling::Row(p) | Tiling::Stack(p) => p as i32,
        }
    }
}

/// One iron farm build.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Cand {
    /// An exact seed (its generator's name): calibration only.
    pub seed: Option<&'static str>,
    pub module: Module,
    pub modules: u8,
    pub tiling: Tiling,
    /// The calm part (an index into the stage's catalog timing parts) for observer cages.
    pub calm_part: u16,
    /// One calm trigger drives every module's hide (only module 0 has U1 and its part).
    pub shared_calm: bool,
    pub place: Place,
    /// Dug in: the design y of the ground layer (the beds that far below it).
    pub ground: i8,
    /// Built on the surface of the stage's terrain: the void design (every wall placed), its
    /// whole box cleared to the sky first (`clear_box`), instead of dug cell by cell.
    pub surface: bool,
    /// The L3 family (kill in place, the golems' own tripwire calm: `layouts::l3`), its spawn
    /// pits; 0: the module grammar.
    pub l3_pits: u8,
    /// The ironfarm lane's generators (crates/mobfarm/src/day1.rs): 1 `stack` (M2-open-ring
    /// modules stacked upward `tiling` apart, each with its own chests), 2 `row` (side by side, one
    /// hopper line to a shared sorter, iron-block crafter and composter); the count is `modules`,
    /// the pad `module.pad.n`; 0: not a generator family.
    pub family_gen: u8,
    /// A shared-kill stack's chain length (family 3): every `chain` modules share one kill at the
    /// chain's bottom, the chains stacked 43 apart (no coupling between chains); 0: one chain of
    /// all the modules.
    pub chain: u8,
}

/// The exact seeds: (name, stage it calibrates, what it is).
pub const SEEDS: &[(&str, &str, &str)] = &[
    ("d1", "day-1", "D1: the bed plate over a 4 x 4 pit (gen l6p-pad 6)"),
    ("d4", "day-1", "D4: two scared villagers through one hide (gen d4 6)"),
    ("d1u", "day-1", "D1U: D4 dug into the ground (gen d1u 6)"),
    ("l3", "day-1", "L3: the archived day-1 farm, 8 lava pits (gen l3 8)"),
    ("m2open", "middle", "M2-open: the bed observer and its hand-built 48 gt hide (gen m2open 6)"),
    ("m3", "middle", "M3: the bed plate with 3 scared villagers (gen m3 6)"),
    ("m4", "middle", "M4: two M2 modules stacked 43 apart (gen stack --modules 2)"),
    ("lb8", "late", "LB: 8 M2 modules stacked 43 apart (gen stack --modules 8)"),
    ("ls8", "late", "LS-open: 8 M2-open modules sharing one chute and collection (gen shared --modules 8)"),
];

fn kill_key(k: Kill) -> String {
    match k {
        Kill::Pit { k } => format!("pit{}", -k),
        Kill::Blade { k } => format!("blade{}", -k),
        Kill::Shaft { k } => format!("shaft{}", -k),
        Kill::Side { len, feet, one } => format!("side{len}{}{}", if feet == 0 { "s".to_string() } else { format!("d{}", -feet) }, if one { "h1" } else { "" }),
    }
}

/// The canonical key.
pub fn key(c: &Cand) -> String {
    if let Some(s) = c.seed {
        return format!("seed-{s}");
    }
    let m = &c.module;
    let calm = match m.cage.calm {
        CalmKind::Plate => "plate".to_string(),
        CalmKind::Observer => format!("obs{}{}{}", c.calm_part, if m.cage.u1_west { "w" } else { "" }, if m.cage.regular { "r" } else { "" }),
        CalmKind::Pulse => format!("pulse{}{}", if m.cage.u1_west { "w" } else { "" }, if m.cage.regular { "r" } else { "" }),
    };
    let tiling = match (c.modules, c.tiling) {
        (1, _) => "n1".to_string(),
        (n, Tiling::Row(p)) => format!("n{n}row{p}"),
        (n, Tiling::Stack(p)) => format!("n{n}stack{p}"),
    };
    let place = match (c.place, c.surface) {
        (_, true) => "surface".to_string(),
        (Place::Void, _) => "void".to_string(),
        (Place::Dug, _) => format!("dug{}", c.ground),
    };
    if c.l3_pits > 0 {
        return format!("l3-pits{}-{place}", c.l3_pits);
    }
    if c.family_gen == 1 || c.family_gen == 2 || c.family_gen == 5 {
        let name = match c.family_gen {
            1 => "stack",
            5 => "rowsharedkill",
            _ => "rowshared",
        };
        return format!("{name}-pad{}-{tiling}-{place}", c.module.pad.n);
    }
    // the shared-kill stack of grammar modules: the module's key with "sk" before its tiling
    let tiling = if c.family_gen == 8 { format!("cross{}r{}", c.modules, c.tiling.pitch()) } else if c.family_gen == 7 { format!("ring{}d{}", c.modules, c.tiling.pitch()) } else if c.family_gen == 3 && c.chain > 0 { format!("sk{}c{tiling}", c.chain) } else if c.family_gen == 3 { format!("sk{tiling}") } else if c.family_gen == 0 && c.chain == 1 { format!("{tiling}-sorter") } else if c.family_gen == 0 && c.chain == 2 { format!("{tiling}-sorterc") } else { tiling };
    format!(
        "{calm}-s{}b{}-pad{}f{}x{}-{}-{tiling}{}-{place}",
        m.cage.scared,
        m.cage.blind,
        m.pad.n,
        -m.pad.floor,
        m.pad.dx,
        kill_key(m.kill),
        if c.shared_calm && c.modules > 1 { "-sc" } else { "" }
    )
}

/// A parameter's value in words (forks, reports).
pub fn param(c: &Cand, name: &str) -> Option<String> {
    let m = &c.module;
    Some(match name {
        "seed" => c.seed.unwrap_or("-").to_string(),
        "calm" => match (c.l3_pits > 0, m.cage.calm) {
            (true, _) => "tripwire".into(),
            (_, CalmKind::Plate) => "plate".into(),
            (_, CalmKind::Observer) => "observer".into(),
            (_, CalmKind::Pulse) => "pulse".into(),
        },
        // day 1's fork: no redstone dust (the bed plate) or dust (the plate's falling edge into
        // catalog row 189's pulse: torches, dust and repeaters)
        "redstone" => match m.cage.calm {
            CalmKind::Plate => "none".into(),
            CalmKind::Observer | CalmKind::Pulse => "dust".into(),
        },
        "scared" => m.cage.scared.to_string(),
        "villagers" => (m.cage.scared + m.cage.blind + 1).to_string(),
        "pad" => m.pad.n.to_string(),
        "floor" => m.pad.floor.to_string(),
        "kill" => match (c.l3_pits > 0, m.kill) {
            (true, _) => "in-place".into(),
            (_, Kill::Pit { .. }) => "pit".into(),
            (_, Kill::Blade { .. }) => (if c.family_gen == 3 { "shared-blade" } else { "blade" }).into(),
            (_, Kill::Shaft { .. }) => "shaft".into(),
            (_, Kill::Side { .. }) => "side".into(),
        },
        "depth" => m.kill.k().to_string(),
        "ground" => c.ground.to_string(),
        "regular" => m.cage.regular.to_string(),
        "u1" => (if m.cage.u1_west { "west" } else { "east" }).into(),
        "blind" => m.cage.blind.to_string(),
        "modules" => c.modules.to_string(),
        "tiling" => match c.tiling {
            Tiling::Row(_) => "row".into(),
            Tiling::Stack(_) => "stack".into(),
        },
        "pitch" => c.tiling.pitch().to_string(),
        "placement" => match c.place {
            Place::Void => "void".into(),
            Place::Dug => "dug".into(),
        },
        // in the terrain: dug cell by cell, or the void build cleared into it on the surface
        "build" => (if c.surface { "surface" } else if c.place == Place::Dug { "cells" } else { "void" }).into(),
        "family" => (if c.l3_pits > 0 { "l3" } else if c.family_gen == 1 { "stack" } else if c.family_gen == 2 { "row-shared-collection" } else if c.family_gen == 3 { "stack-shared-kill" } else if c.family_gen == 5 { "row-shared-kill" } else if c.family_gen == 7 { "ring-shared-pad" } else if c.family_gen == 8 { "cross" } else { "module" }).into(),
        "pits" => c.l3_pits.to_string(),
        "calm_part" => c.calm_part.to_string(),
        "shared_calm" => (c.shared_calm && c.modules > 1).to_string(),
        _ => return None,
    })
}

/// The grammar's parameters (`space:` lines narrow them, `fork:` lines split on them).
pub fn params() -> Vec<farmopt::Param> {
    let p = |name: &'static str, values: &[&str], doc: &'static str| farmopt::Param { name, values: values.iter().map(|s| s.to_string()).collect(), doc };
    vec![
        p("calm", &["plate", "observer", "pulse"], "the calm trigger: U1's bed plate, a bed observer and a catalog timing part, or (day 1's dust fork) U1's plate into the falling-edge pulse part"),
        p("redstone", &["none", "dust"], "day 1's fork: no dust (the bed plate) or dust (a day-1 catalog part)"),
        p("scared", &["1", "2", "3", "4"], "villagers that see the zombie"),
        p("villagers", &["3", "4", "5", "6"], "villagers per cluster (U1 included)"),
        p("pad", &["6", "8", "10", "9"], "the spawn pad's side (9: the 8 x 8 quadrant pad, a 2 x 2 hole in each quadrant over a collector)"),
        p("floor", &["-7", "-6", "-5"], "the pad's floor under the beds"),
        p("kill", &["pit", "blade", "side", "shaft", "shared-blade", "in-place"], "the pit corner (1 lava), the fire-safe blade chamber (4 lava), the chute to the side (a drop, an arm through the rock, a blade beside the farm), a shared kill's shaft"),
        p("depth", &["-13", "-15", "-17", "-19", "-21", "-23", "-25"], "golem feet at the kill"),
        p("modules", &["1", "2", "3", "4", "8"], "villager clusters"),
        p("tiling", &["row", "stack"], "how modules tile"),
        p("pitch", &["17", "19", "21", "23", "25", "27", "43"], "the modules' pitch"),
        p("placement", &["void", "dug"], "the void, or in the stage's terrain sample"),
        p("ground", &["-1", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16"], "dug: the beds this far under the ground layer (-1: on the surface or in the void)"),
        p("regular", &["true", "false"], "the regular cage (iteration 17): U1 in the watchers' row at their level"),
        p("u1", &["east", "west"], "U1's side of the row"),
        p("blind", &["0", "1", "2", "3"], "sleeping villagers besides U1"),
        p("build", &["void", "cells", "surface"], "in the terrain: dug cell by cell, or the void build cleared into it"),
        p("family", &["module", "l3", "stack", "row-shared-collection", "stack-shared-kill", "row-shared-kill", "ring-shared-pad", "cross"], "the module grammar (cage, pad, kill), L3's spawn pits, the ironfarm lane's stack / row / row_shared (form b: drop blades, one items channel) generators, grammar modules stacked over one shared kill, 4 cages around one pad and kill (the ring), or the ironfarm lane's + (four modules at radius r around one deep junction kill)"),
        p("pits", &["0", "1", "2", "3", "4", "5", "6", "7", "8"], "L3's spawn pits"),
        p("calm_part", &["0", "1", "2", "3"], "the observer calm's catalog part"),
        p("shared_calm", &["true", "false"], "one calm trigger for every module"),
        p("seed", &[], "an exact seed (calibration only)"),
    ]
}

fn allowed(spec: &Spec, c: &Cand) -> bool {
    params().iter().all(|p| param(c, p.name).is_none_or(|v| spec.allows(p.name, &v)))
}

/// The cages of a stage (day 1: the bed plate, and the dust fork's plate + falling-edge pulse; no
/// quartz).
fn cages(stage: &str) -> Vec<Cage> {
    let mut v = Vec::new();
    for (scared, blinds) in [(1u8, 1..=3u8), (2, 0..=3)] {
        for blind in blinds {
            v.push(Cage { scared, blind, calm: CalmKind::Plate, u1_west: false, regular: false });
        }
    }
    if stage == "day-1" {
        // the dust fork (catalog row 189's falling-edge pulse): M2's hide, U1 on its plate at the
        // row's end
        for scared in 2..=3u8 {
            for blind in 0..=0u8 {
                for u1_west in [false, true] {
                    v.push(Cage { scared, blind, calm: CalmKind::Pulse, u1_west, regular: false });
                    // the regular row (ironspec iteration 17, the parent): U1 walled in the row at
                    // the watchers' level, its plate feeding the dust
                    v.push(Cage { scared, blind, calm: CalmKind::Pulse, u1_west, regular: true });
                }
            }
        }
    } else {
        for scared in 2..=4u8 {
            for blind in 0..=2u8 {
                for u1_west in [false, true] {
                    v.push(Cage { scared, blind, calm: CalmKind::Observer, u1_west, regular: false });
                    // the regular cage (iteration 17): U1 in the row at the watchers' level, in a
                    // pod like theirs, out of the zombie's reach; no sleepers (pods all alike)
                    if blind == 0 {
                        v.push(Cage { scared, blind, calm: CalmKind::Observer, u1_west, regular: true });
                    }
                }
            }
        }
    }
    v
}

/// Every module of a stage's space (fire-safe pits: lava at k + 2 at least 4 under the ring's
/// gates at the pad floor, `LavaFluid.randomTick` climbs 2: k <= floor - 6).
pub fn modules(stage: &str, place: Place) -> Vec<Module> {
    let mut out = Vec::new();
    for cage in cages(stage) {
        // (the quadrant pad, n 9, measured and dropped: 353 iron/h against the 8 x 8's 426 on the
        // middle module, its collector's second flush; FARMOPT_QUAD_PAD=1 enumerates it again)
        let quads = std::env::var("FARMOPT_QUAD_PAD").is_ok();
        for n in [6u8, 8, 10, 9].into_iter().filter(|&n| n != 9 || quads) {
            for floor in [-7i8, -6, -5] {
                for dx in [-1i8, 0, 1] {
                    let mut kills: Vec<Kill> = (0..6).map(|i| Kill::Pit { k: -13 - 2 * i }).collect();
                    kills.extend([-19i8, -22, -25].map(|k| Kill::Blade { k }));
                    // the chute to the side (vjeux 18:02): the pit corner 19-23 blocks east of the
                    // hole, out of every villager's follow range, at the farm's level (dug only)
                    if stage == "day-1" {
                        // the arm from the cube's floor (-20), from -24, or shallow (3 under
                        // the pad, inside the cubes: the judge measures the dwell)
                        for feet in [-20i8, -24, 0] {
                            for one in [false, true] {
                                kills.extend([16u8, 23].map(|len| Kill::Side { len, feet, one }));
                            }
                        }
                    }
                    for kill in kills {
                        let k = kill.k();
                        let ok = match kill {
                            Kill::Pit { .. } => k <= floor as i32 - 6,
                            Kill::Blade { .. } => k <= floor as i32 - 12,
                            Kill::Shaft { .. } => false,
                            // the side chute: dug only (the rock is its walls)
                            Kill::Side { .. } => place == Place::Dug,
                        };
                        // a dug blade chamber: its walls, floor and roof are the terrain's (the
                        // ironspec's M3 family on day 1: the drop and the fire-safe blade)
                        if ok {
                            out.push(Module { cage, pad: Pad { n, floor, dx }, kill });
                        }
                    }
                }
            }
        }
    }
    out
}

/// Every candidate of the spec's stage.
pub fn enumerate(spec: &Spec, calm_parts: usize) -> Vec<Cand> {
    let stage = spec.stage.as_str();
    let mut out = Vec::new();
    let base = |module: Module| Cand { seed: None, module, modules: 1, tiling: Tiling::Row(0), calm_part: 0, shared_calm: false, place: Place::Void, ground: 0, surface: false, l3_pits: 0, family_gen: 0, chain: 0 };
    match stage {
        "day-1" => {
            for m in modules(stage, Place::Dug) {
                // dug in: the beds 3 (half-dug: the pods' roofs at the surface) to 16 below the
                // ground layer (main's judge caps every villager's golem search from its cell and
                // the cell above: a shallow farm pays hundreds of covers, so depth trades against
                // them, D1U at 11; vjeux 18:02 refuses covers outright: the bed-plate cages need
                // 13 or more, their plate villager sitting a level up by the zombie)
                for ground in [3i8, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16] {
                    out.push(Cand { place: Place::Dug, ground, ..base(m) });
                }
            }
            // on the surface: the void module, its box cleared into the terrain (D1's way)
            for m in modules(stage, Place::Void) {
                if m.pad.dx == 0 && matches!(m.kill, Kill::Pit { .. }) {
                    out.push(Cand { place: Place::Dug, ground: -1, surface: true, ..base(m) });
                }
            }
            // the L3 family: spawn pits that burn golems in place, the golems' own tripwire
            // hides the zombie (no U1: 3 scared villagers), on the surface
            for pits in 1..=8u8 {
                out.push(Cand { place: Place::Dug, ground: -1, surface: true, l3_pits: pits, ..base(modules(stage, Place::Dug)[0]) });
            }
            // two clusters within the village's 8 villagers (ironspec: the day-1 "farm for the
            // week"), side by side on x: from 12 apart (each cluster's villagers count their own,
            // > 10.6) to 30 (out of each other's follow cubes: no coupling), dug 7 to 11 deep:
            // the 3-villager cages over the 6 x 6 pads
            for m in modules(stage, Place::Dug) {
                if m.cage.scared + m.cage.blind + 1 == 3 && m.pad.n == 6 && m.pad.dx == 0 && m.pad.floor >= -7 {
                    for ground in [7i8, 9, 11] {
                        for pitch in [12u8, 16, 20, 24, 27, 30] {
                            out.push(Cand { place: Place::Dug, ground, modules: 2, tiling: Tiling::Row(pitch), ..base(m) });
                        }
                    }
                    // the chute's edge (vjeux 18:14): two groups facing each other across one
                    // pad's rim, one chute (the hole) down past both groups' cubes to one kill
                    if m.cage.blind == 0 && !matches!(m.kill, Kill::Side { .. }) && m.kill.k() <= -21 {
                        for ground in [9i8, 11] {
                            for d in [1u8, 2, 3] {
                                out.push(Cand { place: Place::Dug, ground, modules: 2, tiling: Tiling::Row(d), family_gen: 7, ..base(m) });
                            }
                        }
                    }
                }
            }
        }
        _ => {
            // N free (spec iteration 4): the farm's curve points 2, 4, 8, 16 (late from 1); the
            // spec's `space: modules` narrows them; the front holds each farm at its own N
            // (middle's single modules only when the spec's `space: modules` names 1: the run that
            // picks the stage's best single module, the module the copies bar copies)
            let middle_single = spec.space.get("modules").is_some_and(|v| v.iter().any(|x| x.trim() == "1"));
            let all_ns: Vec<u8> = if stage == "late" || middle_single { vec![1, 2, 4, 8, 16] } else { vec![2, 4, 8, 16] };
            let ns: Vec<u8> = all_ns.into_iter().filter(|n| spec_allows_n(spec, *n)).collect();
            for m in modules(stage, Place::Void) {
                // the calm catalog's front: its two smallest parts (crates/farmopt-iron/src/calm.rs)
                let parts: Vec<u16> = if m.cage.calm == CalmKind::Observer { (0..calm_parts.min(2) as u16).collect() } else { vec![0] };
                for &calm_part in &parts {
                    for &n in &ns {
                        for pitch in [17u8, 19, 21, 23, 25, 27] {
                            // (a shared calm is the observer's: the bed plate's sharing is not in v1)
                            let shares: &[bool] = if m.cage.calm == CalmKind::Observer { &[false, true] } else { &[false] };
                            for &shared in shares {
                                out.push(Cand { modules: n, tiling: Tiling::Row(pitch), calm_part, shared_calm: shared, ..base(m) });
                            }
                        }
                    }
                }
            }
            // the shared kill (vjeux 14:50: "why does each module have their own kill chamber?"):
            // grammar modules stacked with their pad holes aligned, every module's golems falling
            // down one shaft through the pad space below into ONE fire-safe blade and ONE
            // collection at the bottom; the falls' coupling with the lower modules' cubes is
            // measured by the runs (pitch 26-38); the 2- and 3-watcher modules (the bar's module is
            // the 2-watcher one), no sleepers
            for m in modules(stage, Place::Void) {
                if m.cage.calm != CalmKind::Observer || !(2..=3).contains(&m.cage.scared) || m.cage.blind != 0 || m.pad.dx != 0 || m.pad.floor != -7 || !matches!(m.kill, Kill::Blade { k } if k <= -22) {
                    continue;
                }
                for &n in ns.iter().filter(|&&n| n >= 2) {
                    // chains of 2 or 3 (ironspec's pair stack): every chain shares one kill, the
                    // chains 43 apart, so a chain's falls pass only its own lower modules
                    for chain in [2u8, 3] {
                        if n > chain {
                            for pitch in [26u8, 30] {
                                let chains = (n as i32 + chain as i32 - 1) / chain as i32;
                                if chains * ((chain as i32 - 1) * pitch as i32 + 43) + 30 <= 380 {
                                    out.push(Cand { modules: n, tiling: Tiling::Stack(pitch), family_gen: 3, chain, ..base(m) });
                                }
                            }
                        }
                    }
                    for pitch in [26u8, 30, 34, 38] {
                        if (n as i32) * (pitch as i32) + 30 <= 380 {
                            out.push(Cand { modules: n, tiling: Tiling::Stack(pitch), family_gen: 3, ..base(m) });
                        }
                        // dug into flat ground (plains): not in this grammar yet. The plains sample
                        // holds 48 layers under its ground before its caves at y 22 refuse a spot
                        // (8 + (N - 1) pitch + 25 <= 48: N = 2 at pitch 15 at most, coupled), and an
                        // observer cage dug into rock needs its stray cavities capped in the
                        // terrain's world (the void caps cannot see the rock): the pitch-15 test
                        // failed the exit check on a cavity by the calm part
                    }
                }
            }
            // the ring (vjeux 17:54: "Why do we not build the villager chambers around the kill
            // chamber?"): 4 cages facing inward around one pad, hole, kill and collection, each
            // cage pulled d from the pad: the clusters are separate (every two villagers of
            // neighbouring cages more than 10.6 apart on x or z: d >= 7 for the 2-watcher cage)
            // and the pad must still lie within 8 of each cage (a 10 x 10 pad from d = 7);
            // every cluster sees every golem on the shared pad (coupled)
            for m in modules(stage, Place::Void) {
                if m.cage.calm != CalmKind::Observer || !(2..=3).contains(&m.cage.scared) || m.cage.blind != 0 || m.cage.u1_west || m.pad.dx != 0 || m.pad.floor != -7 || !matches!(m.kill, Kill::Blade { k } if k == -22) {
                    continue;
                }
                if ns.contains(&4) || ns.contains(&2) {
                    for d in [5u8, 6, 7, 8, 9] {
                        out.push(Cand { modules: 4, tiling: Tiling::Row(d), family_gen: 7, ..base(m) });
                    }
                    // N = 2 (vjeux 18:14: "build at the edge of the chute so only the group can
                    // see it and then drop"): two cages facing each other across one pad whose
                    // rim is each group's only spawn cells, the chute (the hole) between them, 11+
                    // apart (separate clusters), both reaching the whole rim
                    for d in [1u8, 2, 3] {
                        out.push(Cand { modules: 2, tiling: Tiling::Row(d), family_gen: 7, ..base(m) });
                    }
                }
            }
            // the ironfarm lane's generators: M2-open-ring modules stacked upward with their own
            // chests (the world's height y -64..319 holds 8 at pitch 43 from y -14), or side by
            // side with one shared collection (sorter, iron-block crafter, composter)
            let gm = modules(stage, Place::Void)[0];
            for pad in [6u8, 8, 10] {
                for &n in &ns {
                    for pitch in [30u8, 36, 43, 48] {
                        if n as i32 * pitch as i32 <= 330 {
                            out.push(Cand { modules: n, tiling: Tiling::Stack(pitch), family_gen: 1, module: Module { pad: Pad { n: pad, ..gm.pad }, ..gm }, ..base(gm) });
                        }
                    }
                    if n >= 2 {
                        for pitch in [17u8, 20, 23, 27] {
                            out.push(Cand { modules: n, tiling: Tiling::Row(pitch), family_gen: 2, module: Module { pad: Pad { n: pad, ..gm.pad }, ..gm }, ..base(gm) });
                        }
                        // the shared kill's form (b): drop blades + one deep items channel + one
                        // collection (ironfarm's `row_shared`; drops at least 8 apart, modules out
                        // of each other's cubes from 24)
                        for pitch in [24u8, 27] {
                            out.push(Cand { modules: n, tiling: Tiling::Row(pitch), family_gen: 5, module: Module { pad: Pad { n: pad, ..gm.pad }, ..gm }, ..base(gm) });
                        }
                    }
                }
                // the + (vjeux 18:16): four modules at radius r around one deep junction kill, or
                // two facing each other across it (the trench), with fenced channels (the ironfarm
                // lane's cross_channel_arms, r >= 12: its module's wedge), from the tightest r to
                // uncoupled (r = 21 passes the real game: 453.8 golems/h vs redsim's 445.2)
                for arms in [2u8, 4] {
                    if ns.contains(&arms) || ns.contains(&4) {
                        for r in [12u8, 13, 14, 16, 18, 21, 24] {
                            out.push(Cand { modules: arms, tiling: Tiling::Row(r), family_gen: 8, module: Module { pad: Pad { n: pad, ..gm.pad }, ..gm }, ..base(gm) });
                        }
                    }
                }
            }
        }
    }
    out.retain(|c| allowed(spec, c));
    out
}

/// The seeds of a stage.
/// Whether the spec lets a farm have `n` modules: its `space: modules` line when it has one, and
/// every `modules` constraint (as a number).
fn spec_allows_n(spec: &Spec, n: u8) -> bool {
    if let Some(vs) = spec.space.get("modules")
        && !vs.iter().any(|v| v == &n.to_string())
    {
        return false;
    }
    spec.constraints.iter().filter(|c| c.metric == "modules").all(|c| c.may_hold(n as f64, n as f64))
}

/// The single module (N = 1, the void, its own calm) whose key is `key`, among the stage's
/// modules: the spec's `bar: <key>` names the module the copies bar copies (ironspec: the bar is
/// copies of the best single module of the space).
pub fn single_from_key(stage: &str, key: &str) -> Option<Cand> {
    // (the sorter and crafter storage: a module's `chain` 1, the key's `-sorter`; the cheap one:
    // `chain` 2, `-sorterc`)
    if let Some(base) = key.strip_suffix("-sorterc-void").map(|k| format!("{k}-void")) {
        return single_from_key(stage, &base).map(|c| Cand { chain: 2, ..c });
    }
    if let Some(base) = key.strip_suffix("-sorter-void").map(|k| format!("{k}-void")) {
        return single_from_key(stage, &base).map(|c| Cand { chain: 1, ..c });
    }
    for m in modules(stage, Place::Void) {
        // (any blade or pit depth: the local search's neighbours)
        let mut kills = vec![m.kill];
        match m.kill {
            Kill::Blade { .. } => kills = (15..=28).map(|k| Kill::Blade { k: -k }).collect(),
            Kill::Pit { .. } => kills = (12..=26).map(|k| Kill::Pit { k: -k }).collect(),
            _ => {}
        }
        for kill in kills {
            for dx in -2i8..=2 {
            let m = Module { kill, pad: crate::parts::Pad { dx, ..m.pad }, ..m };
            for calm_part in 0..2u16 {
                let c = Cand { seed: None, module: m, modules: 1, tiling: Tiling::Row(0), calm_part, shared_calm: false, place: Place::Void, ground: 0, surface: false, l3_pits: 0, family_gen: 0, chain: 0 };
                if self::key(&c) == key {
                    return Some(c);
                }
            }
            }
        }
    }
    // a dug single module at any depth, its pit at any depth (the local search's neighbours: a
    // pit one deeper than the grammar's odd depths)
    if key.ends_with(&format!("-dug{}", key.rsplit("-dug").next().unwrap_or(""))) {
        for m in modules(stage, Place::Dug) {
            for ground in 3..=16i8 {
                let mut kills = vec![m.kill];
                if let Kill::Pit { .. } = m.kill {
                    kills = (12..=26).map(|k| Kill::Pit { k: -k }).collect();
                }
                for kill in kills {
                    let c = Cand { seed: None, module: Module { kill, ..m }, modules: 1, tiling: Tiling::Row(0), calm_part: 0, shared_calm: false, place: Place::Dug, ground, surface: false, l3_pits: 0, family_gen: 0, chain: 0 };
                    if self::key(&c) == key {
                        return Some(c);
                    }
                }
            }
        }
    }
    None
}

pub fn seeds(stage: &str) -> Vec<Cand> {
    let m = Module { cage: Cage { scared: 1, blind: 1, calm: CalmKind::Plate, u1_west: false, regular: false }, pad: Pad { n: 6, floor: -7, dx: 0 }, kill: Kill::Pit { k: -13 } };
    SEEDS.iter().filter(|s| s.1 == stage).map(|s| Cand { seed: Some(s.0), module: m, modules: 1, tiling: Tiling::Row(0), calm_part: 0, shared_calm: false, place: Place::Void, ground: 0, surface: false, l3_pits: 0, family_gen: 0, chain: 0 }).collect()
}

/// Geometric neighbours (local search): the pad moved one on x, its floor one up or down, the
/// kill one deeper or shallower, the pitch two closer or farther, the dug depth one either way.
pub fn neighbors(c: &Cand) -> Vec<Cand> {
    if c.seed.is_some() {
        return Vec::new();
    }
    let mut v = Vec::new();
    let mut push = |f: &dyn Fn(&mut Cand)| {
        let mut n = c.clone();
        f(&mut n);
        if n != *c {
            v.push(n);
        }
    };
    push(&|n| n.module.pad.dx = (n.module.pad.dx - 1).max(-3));
    push(&|n| n.module.pad.dx = (n.module.pad.dx + 1).min(3));
    push(&|n| n.module.kill = match n.module.kill {
        Kill::Pit { k } => Kill::Pit { k: k - 1 },
        Kill::Blade { k } => Kill::Blade { k: k - 1 },
        Kill::Shaft { k } => Kill::Shaft { k },
        Kill::Side { len, feet, one } => Kill::Side { len: len + 7, feet, one },
    });
    push(&|n| n.module.kill = match n.module.kill {
        Kill::Pit { k } if (k as i32) < n.module.pad.floor as i32 - 6 => Kill::Pit { k: k + 1 },
        Kill::Blade { k } if (k as i32) < n.module.pad.floor as i32 - 12 => Kill::Blade { k: k + 1 },
        other => other,
    });
    if c.modules > 1 {
        push(&|n| n.tiling = match n.tiling {
            Tiling::Row(p) => Tiling::Row(p.saturating_sub(1).max(12)),
            Tiling::Stack(p) => Tiling::Stack(p.saturating_sub(1).max(12)),
        });
        push(&|n| n.tiling = match n.tiling {
            Tiling::Row(p) => Tiling::Row(p + 1),
            Tiling::Stack(p) => Tiling::Stack(p + 1),
        });
        // the farm's curve: twice and half its count (the brief's N = 1, 2, 4, 8, 16), so a front
        // member's curve gets measured around it
        if c.modules < 16 {
            push(&|n| n.modules = (n.modules * 2).min(16));
        }
        if c.modules > 2 {
            push(&|n| n.modules /= 2);
        }
    }
    if c.place == Place::Dug {
        push(&|n| n.ground = (n.ground - 1).max(5));
        push(&|n| n.ground = (n.ground + 1).min(10));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_canonical_and_distinct() {
        let spec = Spec::parse("farm: iron\nstage: day-1\nobjective: min t_kit_minutes\n").unwrap();
        let all = enumerate(&spec, 0);
        let keys: std::collections::HashSet<String> = all.iter().map(key).collect();
        assert_eq!(keys.len(), all.len());
        assert!(all.iter().all(|c| c.place == Place::Dug && c.module.cage.calm != CalmKind::Observer));
        let narrowed = Spec::parse("farm: iron\nstage: day-1\nobjective: min t_kit_minutes\nspace: pad 6\nspace: kill pit\n").unwrap();
        let few = enumerate(&narrowed, 0);
        assert!(few.len() < all.len() && few.iter().all(|c| c.module.pad.n == 6));
    }
}
