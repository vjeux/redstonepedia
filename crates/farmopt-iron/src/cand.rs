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
        CalmKind::Observer => format!("obs{}{}", c.calm_part, if m.cage.u1_west { "w" } else { "" }),
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
        },
        // day 1's fork: no redstone dust (the bed plate) or dust (a day-1 catalog part); no
        // day-1 part exists yet, so every day-1 candidate is `none`
        "redstone" => match m.cage.calm {
            CalmKind::Plate => "none".into(),
            CalmKind::Observer => "dust".into(),
        },
        "scared" => m.cage.scared.to_string(),
        "villagers" => (m.cage.scared + m.cage.blind + 1).to_string(),
        "pad" => m.pad.n.to_string(),
        "floor" => m.pad.floor.to_string(),
        "kill" => match (c.l3_pits > 0, m.kill) {
            (true, _) => "in-place".into(),
            (_, Kill::Pit { .. }) => "pit".into(),
            (_, Kill::Blade { .. }) => "blade".into(),
        },
        "depth" => m.kill.k().to_string(),
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
        "family" => (if c.l3_pits > 0 { "l3" } else { "module" }).into(),
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
        p("calm", &["plate", "observer"], "the calm trigger: U1's bed plate, or a bed observer and a catalog timing part"),
        p("redstone", &["none", "dust"], "day 1's fork: no dust (the bed plate) or dust (a day-1 catalog part)"),
        p("scared", &["1", "2", "3", "4"], "villagers that see the zombie"),
        p("villagers", &["3", "4", "5", "6"], "villagers per cluster (U1 included)"),
        p("pad", &["6", "8", "10"], "the spawn pad's side"),
        p("floor", &["-7", "-6", "-5"], "the pad's floor under the beds"),
        p("kill", &["pit", "blade"], "the pit corner (1 lava) or the fire-safe blade chamber (4 lava)"),
        p("depth", &["-13", "-15", "-17", "-19", "-21", "-23", "-25"], "golem feet at the kill"),
        p("modules", &["1", "2", "3", "4", "8"], "villager clusters"),
        p("tiling", &["row", "stack"], "how modules tile"),
        p("pitch", &["17", "19", "21", "23", "25", "27", "43"], "the modules' pitch"),
        p("placement", &["void", "dug"], "the void, or in the stage's terrain sample"),
        p("build", &["void", "cells", "surface"], "in the terrain: dug cell by cell, or the void build cleared into it"),
        p("family", &["module", "l3"], "the module grammar (cage, pad, kill), or L3's spawn pits"),
        p("pits", &["0", "1", "2", "3", "4", "5", "6", "7", "8"], "L3's spawn pits"),
        p("calm_part", &["0", "1", "2", "3"], "the observer calm's catalog part"),
        p("shared_calm", &["true", "false"], "one calm trigger for every module"),
        p("seed", &[], "an exact seed (calibration only)"),
    ]
}

fn allowed(spec: &Spec, c: &Cand) -> bool {
    params().iter().all(|p| param(c, p.name).is_none_or(|v| spec.allows(p.name, &v)))
}

/// The cages of a stage (day 1: the bed plate only, no quartz).
fn cages(stage: &str) -> Vec<Cage> {
    let mut v = Vec::new();
    for (scared, blinds) in [(1u8, 1..=3u8), (2, 0..=3)] {
        for blind in blinds {
            v.push(Cage { scared, blind, calm: CalmKind::Plate, u1_west: false });
        }
    }
    if stage != "day-1" {
        for scared in 2..=4u8 {
            for blind in 0..=2u8 {
                for u1_west in [false, true] {
                    v.push(Cage { scared, blind, calm: CalmKind::Observer, u1_west });
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
        for n in [6u8, 8, 10] {
            for floor in [-7i8, -6, -5] {
                for dx in [-1i8, 0, 1] {
                    let mut kills: Vec<Kill> = (0..6).map(|i| Kill::Pit { k: -13 - 2 * i }).collect();
                    kills.extend([-19i8, -22, -25].map(|k| Kill::Blade { k }));
                    for kill in kills {
                        let k = kill.k();
                        let ok = match kill {
                            Kill::Pit { .. } => k <= floor as i32 - 6,
                            Kill::Blade { .. } => k <= floor as i32 - 12,
                        };
                        // a dug blade chamber is not in v1 (its walls are the terrain's: the
                        // pit's 4 x 4 shaft is)
                        if ok && !(place == Place::Dug && matches!(kill, Kill::Blade { .. })) {
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
    let base = |module: Module| Cand { seed: None, module, modules: 1, tiling: Tiling::Row(0), calm_part: 0, shared_calm: false, place: Place::Void, ground: 0, surface: false, l3_pits: 0 };
    match stage {
        "day-1" => {
            for m in modules(stage, Place::Dug) {
                // dug in: the beds 3 (half-dug: the pods' roofs at the surface) to 8 below the
                // ground layer
                for ground in [3i8, 4, 5, 6, 7, 8] {
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
        }
        _ => {
            // N free (spec iteration 4): the farm's curve points 2, 4, 8, 16 (late from 1); the
            // spec's `space: modules` narrows them; the front holds each farm at its own N
            let all_ns: Vec<u8> = if stage == "late" { vec![1, 2, 4, 8, 16] } else { vec![2, 4, 8, 16] };
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

pub fn seeds(stage: &str) -> Vec<Cand> {
    let m = Module { cage: Cage { scared: 1, blind: 1, calm: CalmKind::Plate, u1_west: false }, pad: Pad { n: 6, floor: -7, dx: 0 }, kill: Kill::Pit { k: -13 } };
    SEEDS.iter().filter(|s| s.1 == stage).map(|s| Cand { seed: Some(s.0), module: m, modules: 1, tiling: Tiling::Row(0), calm_part: 0, shared_calm: false, place: Place::Void, ground: 0, surface: false, l3_pits: 0 }).collect()
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
        assert!(all.iter().all(|c| c.place == Place::Dug && c.module.cage.calm == CalmKind::Plate));
        let narrowed = Spec::parse("farm: iron\nstage: day-1\nobjective: min t_kit_minutes\nspace: pad 6\nspace: kill pit\n").unwrap();
        let few = enumerate(&narrowed, 0);
        assert!(few.len() < all.len() && few.iter().all(|c| c.module.pad.n == 6));
    }
}
