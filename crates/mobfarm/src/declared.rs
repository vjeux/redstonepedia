//! Declared states (ironspec, Nov 11 01:04: make the silent qboost hazard impossible to repeat):
//! a design's generator states what each of its controlled or always-on cells must be (a booster
//! powered, a braked rail unpowered, a plain or detector rail present, a gate shut or open), and
//! every runner checks the world against it at tick 0 and, for the always-on cells (boosters),
//! again after a short warm-up. Any mismatch fails the run. A design file carries its states as a
//! header line `declared: X,Y,Z id[k=v,...]!; ...` (`!` marks an always-on cell, checked through
//! the warm-up too); loop designs derive theirs from their `queue:` options (`queue::declared_states`),
//! so an older file whose blocks no longer match its options fails too.

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
}

impl Decl {
    pub fn new(cell: [i32; 3], id: &str, props: &[(&str, &str)], always: bool) -> Decl {
        let id = if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") };
        Decl { cell, id, props: props.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), always }
    }
}

/// The header line's value.
pub fn format(ds: &[Decl]) -> String {
    ds.iter()
        .map(|d| format!("{},{},{} {}{}{}", d.cell[0], d.cell[1], d.cell[2], d.id.trim_start_matches("minecraft:"), if d.props.is_empty() { String::new() } else { format!("[{}]", d.props.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(",")) }, if d.always { "!" } else { "" }))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Parses a `declared:` header value.
pub fn parse(s: &str) -> Result<Vec<Decl>, String> {
    let mut out = Vec::new();
    for item in s.split(';').map(str::trim).filter(|t| !t.is_empty()) {
        let (cell, rest) = item.split_once(' ').ok_or_else(|| format!("declared: {item}: give X,Y,Z id"))?;
        let v: Vec<i32> = cell.split(',').map(|x| x.parse::<i32>().map_err(|e| format!("declared: {item}: {e}"))).collect::<Result<_, _>>()?;
        let [x, y, z] = v.as_slice() else { return Err(format!("declared: {item}: give X,Y,Z")) };
        let rest = rest.trim();
        let (rest, always) = match rest.strip_suffix('!') {
            Some(r) => (r, true),
            None => (rest, false),
        };
        let (id, props) = match rest.split_once('[') {
            Some((id, p)) => (id, p.trim_end_matches(']').split(',').filter(|t| !t.is_empty()).map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())).ok_or_else(|| format!("declared: {item}: {kv}"))).collect::<Result<Vec<_>, _>>()?),
            None => (rest, Vec::new()),
        };
        let id = if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") };
        out.push(Decl { cell: [*x, *y, *z], id, props, always });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_round_trip() {
        let ds = vec![Decl::new([0, 2, 31], "powered_rail", &[("powered", "true")], true), Decl::new([0, 3, 31], "oak_fence_gate", &[("open", "false")], false), Decl::new([0, 3, 30], "rail", &[], false)];
        let s = format(&ds);
        assert_eq!(s, "0,2,31 powered_rail[powered=true]!; 0,3,31 oak_fence_gate[open=false]; 0,3,30 rail");
        assert_eq!(parse(&s).unwrap(), ds);
    }
}
