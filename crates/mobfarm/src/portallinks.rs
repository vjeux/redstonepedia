//! (onechunk, Nov 12; ironspec 00:02 / 00:16, tick-exact's linking traps) `mctas mobfarm
//! portal-links`: every portal's search as the game runs it (`PortalForcer.findClosestPortalPosition`,
//! redsim's `entity::portal::closest_portal`): from an entity in an overworld portal, the Nether
//! portal cells within 16 (x and z, every y) of its position x 1/8 (y not scaled), the closest by
//! squared distance between block positions, ties to the lower y; from the Nether, the overworld
//! cells within 128 of its position x 8. Each portal is a set of cell boxes (absolute coordinates);
//! the entity's centre is taken anywhere in a cell grown by `margin` on x and z (a golem's centre
//! is up to 0.7 off the cell it touches). For each source portal: every distinct target block, the
//! candidates' least and largest distances over those targets, the chosen portal per target, the
//! verdict against the expected partner and the least margin (the next candidate's distance less
//! the expected one's); an empty search (a portal created there) fails.

use std::collections::BTreeMap;

type C = [i32; 3];

#[derive(Clone, Debug)]
pub struct Portal {
    pub name: String,
    pub cells: Vec<C>,
}

/// `NAME=x0,y0,z0,x1,y1,z1[;x0,...]`: the portal's cells (inclusive boxes).
pub fn parse(spec: &str) -> Result<Portal, String> {
    let (name, rest) = spec.split_once('=').ok_or(format!("{spec}: NAME=x0,y0,z0,x1,y1,z1[;...]"))?;
    let mut cells = Vec::new();
    for b in rest.split(';') {
        let n: Vec<i32> = b.split(',').map(|s| s.trim().parse::<i32>().map_err(|e| format!("{spec}: {e}"))).collect::<Result<_, _>>()?;
        if n.len() != 6 {
            return Err(format!("{spec}: a box is x0,y0,z0,x1,y1,z1"));
        }
        for x in n[0].min(n[3])..=n[0].max(n[3]) {
            for y in n[1].min(n[4])..=n[1].max(n[4]) {
                for z in n[2].min(n[5])..=n[2].max(n[5]) {
                    cells.push([x, y, z]);
                }
            }
        }
    }
    Ok(Portal { name: name.trim().to_string(), cells })
}

/// The targets of an entity in portal `p`: its centre anywhere over a cell grown by `margin` on x
/// and z, scaled by `scale` on x and z (y as is), floored.
fn targets(p: &Portal, scale: f64, margin: f64) -> Vec<C> {
    let mut out: Vec<C> = Vec::new();
    for c in &p.cells {
        let xs = [c[0] as f64 - margin, c[0] as f64 + 0.5, c[0] as f64 + 0.999 + margin];
        let zs = [c[2] as f64 - margin, c[2] as f64 + 0.5, c[2] as f64 + 0.999 + margin];
        for x in xs {
            for z in zs {
                let t = [(x * scale).floor() as i32, c[1], (z * scale).floor() as i32];
                if !out.contains(&t) {
                    out.push(t);
                }
            }
        }
    }
    out
}

/// The least squared distance from `t` to `p`'s cells inside the search square (and that cell's y).
fn best_in(p: &Portal, t: C, radius: i32) -> Option<(i64, i32)> {
    p.cells
        .iter()
        .filter(|c| (c[0] - t[0]).abs() <= radius && (c[2] - t[2]).abs() <= radius)
        .map(|c| {
            let d = [(c[0] - t[0]) as i64, (c[1] - t[1]) as i64, (c[2] - t[2]) as i64];
            (d[0] * d[0] + d[1] * d[1] + d[2] * d[2], c[1])
        })
        .min()
}

fn search(from: &[Portal], to: &[Portal], scale: f64, radius: i32, margin: f64, expect: &BTreeMap<String, String>, label: &str, out: &mut String) -> usize {
    let mut fails = 0;
    for p in from {
        let ts = targets(p, scale, margin);
        let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
        for t in &ts {
            for k in 0..3 {
                lo[k] = lo[k].min(t[k]);
                hi[k] = hi[k].max(t[k]);
            }
        }
        out.push_str(&format!("{label} {} ({} cells): {} targets x {}..{} y {}..{} z {}..{} (x {scale}, within {radius})\n", p.name, p.cells.len(), ts.len(), lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]));
        let mut chosen: BTreeMap<String, usize> = BTreeMap::new();
        let mut empty = 0usize;
        let mut least_margin = f64::INFINITY;
        let mut dists: BTreeMap<String, (f64, f64, usize)> = BTreeMap::new();
        let want = expect.get(&p.name);
        for t in &ts {
            let mut cands: Vec<(i64, i32, &str)> = to.iter().filter_map(|q| best_in(q, *t, radius).map(|(d, y)| (d, y, q.name.as_str()))).collect();
            cands.sort();
            for (d, _, n) in &cands {
                let e = dists.entry(n.to_string()).or_insert((f64::INFINITY, 0.0, 0));
                let r = (*d as f64).sqrt();
                e.0 = e.0.min(r);
                e.1 = e.1.max(r);
                e.2 += 1;
            }
            match cands.first() {
                None => empty += 1,
                Some((d0, _, n0)) => {
                    *chosen.entry(n0.to_string()).or_default() += 1;
                    if let Some(w) = want {
                        let dw = cands.iter().find(|c| c.2 == w).map(|c| (c.0 as f64).sqrt());
                        let dother = cands.iter().filter(|c| c.2 != w).map(|c| (c.0 as f64).sqrt()).fold(f64::INFINITY, f64::min);
                        if let Some(dw) = dw {
                            least_margin = least_margin.min(dother - dw);
                        } else {
                            least_margin = least_margin.min(-(*d0 as f64).sqrt());
                        }
                    }
                }
            }
        }
        for (n, (a, b, k)) in &dists {
            out.push_str(&format!("  candidate {n}: distance {a:.2}..{b:.2} over {k} of the targets in its square{}\n", if want.is_some_and(|w| w == n) { " [the expected partner]" } else { "" }));
        }
        let picked: Vec<String> = chosen.iter().map(|(n, k)| format!("{n} x {k}")).collect();
        let ok = empty == 0 && want.is_none_or(|w| chosen.len() == 1 && chosen.contains_key(w));
        if !ok && want.is_some() {
            fails += 1;
        }
        out.push_str(&format!(
            "  chosen: {}{}{}{}\n",
            if picked.is_empty() { "nothing".to_string() } else { picked.join(", ") },
            if empty > 0 { format!("; {empty} targets find NOTHING (a new portal would be made there)") } else { String::new() },
            match want {
                Some(w) => format!("; expected {w}: {}", if ok { "PASS" } else { "FAIL" }),
                None => String::new(),
            },
            if want.is_some() && least_margin.is_finite() { format!(" (least margin {least_margin:.2}: the next candidate's distance less the expected one's)") } else { String::new() }
        ));
    }
    fails
}

pub fn report(ow: &[Portal], nether: &[Portal], expect: &BTreeMap<String, String>, margin: f64) -> String {
    let mut s = format!("portal-links: {} overworld and {} Nether portals; an entity's centre anywhere over a cell grown {margin} on x and z; the game's search (16 in the Nether from x 1/8, 128 in the overworld from x 8; y not scaled; the closest by squared distance, ties to the lower y)\n", ow.len(), nether.len());
    let f1 = search(ow, nether, 0.125, 16, margin, expect, "overworld", &mut s);
    let f2 = search(nether, ow, 8.0, 128, margin, expect, "Nether", &mut s);
    let n_exp = expect.len();
    s.push_str(&format!("verdict: {} of {n_exp} expected links fail{}\n", f1 + f2, if f1 + f2 == 0 { " (PASS)" } else { " (FAIL)" }));
    s
}
