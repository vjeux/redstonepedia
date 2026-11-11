//! Declared states (ironspec, Nov 11 01:04: make the silent qboost hazard impossible to repeat):
//! a design's generator states what each of its controlled or always-on cells must be (a booster
//! powered, a braked rail unpowered, a plain or detector rail present, a gate shut or open), and
//! every runner checks the world against it at tick 0 and, for the always-on cells (boosters),
//! again after a short warm-up. Any mismatch fails the run. A design file carries its states as a
//! header line `declared: X,Y,Z id[k=v,...]!; ...` (`!` marks an always-on cell, checked through
//! the warm-up too; `<X,Y,Z` after it names a braked rail's control block, which the runtime
//! check uses); loop designs derive theirs from their `queue:` options (`queue::declared_states`),
//! so an older file whose blocks no longer match its options fails too.
//!
//! The runtime check (Nov 11 03:17: P's power ran along the track through a powered R into slot 0,
//! which the tick-0 check could not see): every tick, a braked rail with a named control block may
//! be powered only while that block is a redstone block.

use redsim::pos::BlockPos;
use redsim::world::World;

/// One declared cell.
#[derive(Clone, Debug, PartialEq)]
pub struct Decl {
    pub cell: [i32; 3],
    /// The block id (`minecraft:powered_rail`).
    pub id: String,
    /// Properties that must hold (`powered=true`).
    pub props: Vec<(String, String)>,
    /// Checked after the warm-up too (an always-on cell).
    pub always: bool,
    /// A braked rail's control block: the rail may be powered only while it is a redstone block.
    pub ctl: Option<[i32; 3]>,
    /// A block the bench sets directly (no control line): no redstone may ever power it (a gate
    /// opened by a neighbour detector rail, say). Header suffix `~`.
    pub unpowered: bool,
}

impl Decl {
    pub fn new(cell: [i32; 3], id: &str, props: &[(&str, &str)], always: bool) -> Decl {
        let id = if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") };
        Decl { cell, id, props: props.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), always, ctl: None, unpowered: false }
    }

    /// This braked rail's control block.
    pub fn with_ctl(mut self, ctl: [i32; 3]) -> Decl {
        self.ctl = Some(ctl);
        self
    }

    /// A bench-set block no redstone may power.
    pub fn bench_set(mut self) -> Decl {
        self.unpowered = true;
        self
    }
}

/// The header line's value.
pub fn format(ds: &[Decl]) -> String {
    ds.iter()
        .map(|d| {
            format!(
                "{},{},{} {}{}{}{}{}",
                d.cell[0],
                d.cell[1],
                d.cell[2],
                d.id.trim_start_matches("minecraft:"),
                if d.props.is_empty() { String::new() } else { format!("[{}]", d.props.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(",")) },
                if d.always { "!" } else { "" },
                if d.unpowered { "~" } else { "" },
                d.ctl.map_or(String::new(), |c| format!("<{},{},{}", c[0], c[1], c[2]))
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Parses a `declared:` header value.
pub fn parse(s: &str) -> Result<Vec<Decl>, String> {
    let cell3 = |t: &str, item: &str| -> Result<[i32; 3], String> {
        let v: Vec<i32> = t.split(',').map(|x| x.trim().parse::<i32>().map_err(|e| format!("declared: {item}: {e}"))).collect::<Result<_, _>>()?;
        match v.as_slice() {
            [x, y, z] => Ok([*x, *y, *z]),
            _ => Err(format!("declared: {item}: give X,Y,Z")),
        }
    };
    let mut out = Vec::new();
    for item in s.split(';').map(str::trim).filter(|t| !t.is_empty()) {
        let (cell, rest) = item.split_once(' ').ok_or_else(|| format!("declared: {item}: give X,Y,Z id"))?;
        let cell = cell3(cell, item)?;
        let rest = rest.trim();
        let (rest, ctl) = match rest.split_once('<') {
            Some((r, c)) => (r.trim(), Some(cell3(c, item)?)),
            None => (rest, None),
        };
        let (rest, unpowered) = match rest.strip_suffix('~') {
            Some(r) => (r, true),
            None => (rest, false),
        };
        let (rest, always) = match rest.strip_suffix('!') {
            Some(r) => (r, true),
            None => (rest, false),
        };
        let (id, props) = match rest.split_once('[') {
            Some((id, p)) => (id, p.trim_end_matches(']').split(',').filter(|t| !t.is_empty()).map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())).ok_or_else(|| format!("declared: {item}: {kv}"))).collect::<Result<Vec<_>, _>>()?),
            None => (rest, Vec::new()),
        };
        let id = if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") };
        out.push(Decl { cell, id, props, always, ctl, unpowered });
    }
    Ok(out)
}

/// The mismatches between the world and the declared states (`warm`: only the always-on cells).
pub fn check(w: &World, ds: &[Decl], warm: bool) -> Vec<String> {
    let mut bad = Vec::new();
    for d in ds.iter().filter(|d| !warm || d.always) {
        let b = w.get(BlockPos::new(d.cell[0], d.cell[1], d.cell[2])).to_string();
        let id = b.split('[').next().unwrap_or("");
        let id = if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") };
        let props_ok = d.props.iter().all(|(k, v)| b.contains(&format!("{k}={v}")));
        if id != d.id || !props_ok {
            bad.push(format!("{},{},{}: declared {}{}, found {b}", d.cell[0], d.cell[1], d.cell[2], d.id, if d.props.is_empty() { String::new() } else { format!("[{}]", d.props.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(",")) }));
        }
    }
    bad
}

/// The runtime check: each braked rail with a control block that is powered while its control is
/// not a redstone block (power from somewhere else: along the track, a neighbour), and each
/// bench-set block that redstone powers.
pub fn check_runtime(w: &World, ds: &[Decl]) -> Vec<String> {
    let mut bad = Vec::new();
    for d in ds {
        if d.unpowered && w.get(BlockPos::new(d.cell[0], d.cell[1], d.cell[2])).to_string().contains("powered=true") {
            bad.push(format!("{},{},{}: powered by a neighbour (it has no control line)", d.cell[0], d.cell[1], d.cell[2]));
            continue;
        }
        let Some(c) = d.ctl else { continue };
        let b = w.get(BlockPos::new(d.cell[0], d.cell[1], d.cell[2])).to_string();
        if b.contains("powered=true") && !w.get(BlockPos::new(c[0], c[1], c[2])).to_string().contains("redstone_block") {
            bad.push(format!("{},{},{}: powered with its control {},{},{} off", d.cell[0], d.cell[1], d.cell[2], c[0], c[1], c[2]));
        }
    }
    bad
}

/// The rider space (day-1's release-gen rule, ironspec Nov 11 04:16): no suffocating block in the
/// two cells over a rail (it suffocates a passing rider; positive control: day-1's first B golem
/// route, support stones over the loop's return track, three riders lost), and no block with a
/// collision box in the cell over an ascending rail (the cart rides up into it), unless the design
/// declares the cell (a gate in `declared:`, the generator's own gates, or the header
/// `rider_space_ok: X,Y,Z; ...`). Glass, gates and other non-suffocating blocks over a flat rail
/// pass: a passenger has no collision of its own.
pub fn rider_space(w: &World, rails: &[[i32; 3]], allowed: &std::collections::HashSet<[i32; 3]>) -> Vec<String> {
    let mut bad = Vec::new();
    for r in rails {
        let ascending = w.get(BlockPos::new(r[0], r[1], r[2])).to_string().contains("ascending");
        for dy in 1..=2 {
            let c = [r[0], r[1] + dy, r[2]];
            if allowed.contains(&c) {
                continue;
            }
            let b = w.get(BlockPos::new(c[0], c[1], c[2]));
            let suffocates = redsim::entity::mob::blocks::is_suffocating(&b);
            let blocks_cart = dy == 1 && ascending && !redsim::behavior::collision_boxes(&b).is_empty();
            if suffocates || blocks_cart {
                bad.push(format!("{},{},{}: {b} over the rail at {},{},{} ({})", c[0], c[1], c[2], r[0], r[1], r[2], if suffocates { "suffocates the rider" } else { "in the cart's way up the slope" }));
            }
        }
    }
    bad
}

/// The sight lines (day-1's release-gen check, ironspec Nov 11 04:31): every rider over a rail of a
/// declared sight stretch against every declared zombie within 8 blocks feet to feet (the villagers'
/// hostile range), the rider's eye at 1.4/1.7/2.0/2.3 over the rail's cell (slopes and stands vary
/// it), the zombie's at 1.5/1.9 over its feet, through redsim's line of sight (collision shapes stop
/// it, glass included). Returns the lines checked and every stopped one with the rail, the zombie
/// and the block, unless that block is allowed (positive control: day-1's B1 golem-route stair cut
/// 3 lines to the station zombie, where a rider lost its hostile in the judge run).
pub fn sight_cuts(w: &World, rails: &[[i32; 3]], zombies: &[[f64; 3]], allowed: &std::collections::HashSet<[i32; 3]>) -> (usize, Vec<String>) {
    let mut n = 0;
    let mut bad = Vec::new();
    for r in rails {
        let feet = (r[0] as f64 + 0.5, r[1] as f64, r[2] as f64 + 0.5);
        for z in zombies {
            let d2 = (z[0] - feet.0).powi(2) + (z[1] - feet.1).powi(2) + (z[2] - feet.2).powi(2);
            if d2 > 64.0 {
                continue;
            }
            for re in [1.4, 1.7, 2.0, 2.3] {
                for ze in [1.5, 1.9] {
                    n += 1;
                    let from = redsim::entity::Vec3::new(feet.0, feet.1 + re, feet.2);
                    let to = redsim::entity::Vec3::new(z[0], z[1] + ze, z[2]);
                    if let Some(h) = redsim::entity::mob::los::first_hit(w, from, to)
                        && !allowed.contains(&[h.x, h.y, h.z])
                    {
                        bad.push(format!("{},{},{} (eye {re}) to the zombie at ({:.2},{:.2},{:.2}) (eye {ze}) cut by {},{},{} {}", r[0], r[1], r[2], z[0], z[1], z[2], h.x, h.y, h.z, w.get(h)));
                    }
                }
            }
        }
    }
    (n, bad)
}

/// A `sight:` header value: rail boxes `X0,Y0,Z0..X1,Y1,Z1; ...` (inclusive).
pub fn parse_boxes(s: &str) -> Result<Vec<([i32; 3], [i32; 3])>, String> {
    s.split(';')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| {
            let (a, b) = t.split_once("..").ok_or_else(|| format!("{t}: give X0,Y0,Z0..X1,Y1,Z1"))?;
            let c = parse_cells(a)?;
            let d = parse_cells(b)?;
            match (c.as_slice(), d.as_slice()) {
                ([p], [q]) => Ok(([p[0].min(q[0]), p[1].min(q[1]), p[2].min(q[2])], [p[0].max(q[0]), p[1].max(q[1]), p[2].max(q[2])])),
                _ => Err(format!("{t}: give X0,Y0,Z0..X1,Y1,Z1")),
            }
        })
        .collect()
}

/// A `rider_space_ok:` header value (X,Y,Z; ...).
pub fn parse_cells(s: &str) -> Result<Vec<[i32; 3]>, String> {
    s.split(';')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| {
            let v: Vec<i32> = t.split(',').map(|x| x.trim().parse::<i32>().map_err(|e| format!("{t}: {e}"))).collect::<Result<_, _>>()?;
            match v.as_slice() {
                [x, y, z] => Ok([*x, *y, *z]),
                _ => Err(format!("{t}: give X,Y,Z")),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_round_trip() {
        let ds = vec![Decl::new([0, 2, 31], "powered_rail", &[("powered", "true")], true), Decl::new([0, 3, 31], "oak_fence_gate", &[("open", "false")], false).bench_set(), Decl::new([0, 3, 30], "rail", &[], false), Decl::new([0, 0, 33], "powered_rail", &[("powered", "false")], false).with_ctl([0, -1, 33])];
        let s = format(&ds);
        assert_eq!(s, "0,2,31 powered_rail[powered=true]!; 0,3,31 oak_fence_gate[open=false]~; 0,3,30 rail; 0,0,33 powered_rail[powered=false]<0,-1,33");
        assert_eq!(parse(&s).unwrap(), ds);
    }
}
