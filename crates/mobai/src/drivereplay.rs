//! (tickexact 2026-11-13, ironspec 06:08 / 06:24 / 06:31 / 06:36 real msg-ids, onechunk's drive v1)
//! `mctas mobai drive-replay`: the composed night on the official 26.3 server, driven OPEN LOOP by
//! onechunk's f1q2-run act list (`--drive-out`) over its scene (`--scene-out`: the design with every
//! rider's cart as an entity line). The scene's blocks are pasted as bed-trap pastes them (+80 y,
//! fluids placed fresh), its entity lines summoned in file order, tagged e<N> (the vehicle) and
//! e<N>p<K> (its K-th passenger) so the drive's `eN.pK` names it. The day time at t = 0 is
//! --day-start, then let run (advance_time on), weather clear. Each act is applied before the tick
//! it names (t from the drive's t0): `home eN.pK x y z` (Brain HOME, memory only: no ticket),
//! `last-slept eN.pK d` (LAST_SLEPT = now + d), `set x y z STATE` (a setblock that must change its
//! cell, the override rule; before -> after printed the first time per cell). The brain agent (+poi)
//! names every line's villager.
//!
//! Bars, each with its numbers: (1) REST ticks at night (day time 12000..23999) of any villager whose
//! latest HOME path is not complete (partial, or none computed yet) -- a villager on a complete HOME
//! path is a napper's trip, excluded and counted; (2) no CANT_REACH stamp ever reaching 1,000 gt (the
//! oldest per villager printed, and each villager's stamp at every REST entry and at dawn); (3) no
//! villager in REST within 16 of the bed's centre (ValidateNearbyPoi's own measure) while the bed is
//! occupied and it isn't the sleeper; (4) no HOME lost (each loss printed with its mechanism and
//! position); (5) every set act changed its cell. OPEN LOOP: the drive carries no recorded state per
//! act, so G1's divergence check cannot run; the controller's coverage is NOT JUDGED by this row.

use anyhow::{bail, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const DY: i32 = 80;
/// Mth.equal's bound (`Math.abs(b - a) < 1.0E-5F`, Mth 162-164) as a double: the game's own test for
/// a clipped axis, used for every contact tolerance here (G1 on M #2)
const MTH_EQUAL: f64 = 1.0E-5f32 as f64;
const MEMS: &str = "minecraft:path,minecraft:walk_target,minecraft:nearest_hostile,minecraft:cant_reach_walk_target_since,minecraft:home,minecraft:last_slept,+poi,+sight,+hurt";

struct Act {
    t: u32,
    verb: String,
    args: Vec<String>,
    /// (onechunk's closed-loop drive) redsim's state just before this write: `carts …; holds …;
    /// pockets …`, names as eN / eN.pK
    expect: Option<String>,
}

/// The drive file: its t0 and its acts, sorted by t (stable: file order within a tick).
fn parse_drive(path: &Path) -> Result<(u32, Vec<Act>)> {
    let text = std::fs::read_to_string(path)?;
    let mut t0 = None;
    let mut acts = Vec::new();
    for (i, l) in text.lines().enumerate() {
        let l = l.trim();
        if let Some(v) = l.strip_prefix("# t0 ") {
            t0 = Some(v.trim().parse::<u32>()?);
        } else if let Some(rest) = l.strip_prefix("act ") {
            let mut w = rest.split_whitespace();
            let t: u32 = w.next().ok_or_else(|| anyhow::anyhow!("line {}: act with no t", i + 1))?.parse()?;
            let verb = w.next().ok_or_else(|| anyhow::anyhow!("line {}: act with no verb", i + 1))?.to_string();
            let args: Vec<String> = w.map(|s| s.to_string()).collect();
            anyhow::ensure!(["home", "last-slept", "set"].contains(&verb.as_str()), "line {}: unknown act verb {verb}", i + 1);
            acts.push(Act { t, verb, args, expect: None });
        } else if let Some(rest) = l.strip_prefix("# expect ") {
            // `# expect t x y z: …` follows the set act it describes
            let (head, text) = rest.split_once(": ").ok_or_else(|| anyhow::anyhow!("line {}: an expect line with no ': '", i + 1))?;
            let h: Vec<&str> = head.split_whitespace().collect();
            let last = acts.last_mut().ok_or_else(|| anyhow::anyhow!("line {}: an expect line before any act", i + 1))?;
            anyhow::ensure!(
                h.len() == 4 && last.verb == "set" && h[0] == last.t.to_string() && h[1..] == last.args[..3],
                "line {}: the expect line ({head}) doesn't follow its set act (t{} {} {:?})",
                i + 1,
                last.t,
                last.verb,
                last.args
            );
            last.expect = Some(text.trim().to_string());
        } else if !l.is_empty() && !l.starts_with('#') {
            bail!("line {}: neither a comment nor an act: {l}", i + 1);
        }
    }
    let t0 = t0.ok_or_else(|| anyhow::anyhow!("{}: no `# t0` line", path.display()))?;
    acts.sort_by_key(|a| a.t);
    anyhow::ensure!(!acts.is_empty(), "{}: no acts", path.display());
    Ok((t0, acts))
}

struct Log {
    at: u64,
}

impl Log {
    fn drain(&mut self, srv: &VanillaServer) -> Result<Vec<String>> {
        let mut f = std::fs::File::open(&srv.log)?;
        f.seek(SeekFrom::Start(self.at))?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        let whole = s.rfind('\n').map(|i| i + 1).unwrap_or(0);
        self.at += whole as u64;
        Ok(s[..whole].lines().filter_map(|l| l.split("mctas brain agent: ").nth(1).map(|r| r.trim().to_string())).collect())
    }
}

/// An entity's UUID as the agent prints it (hyphenated hex).
fn uuid_hex(srv: &mut VanillaServer, sel: &str) -> Result<String> {
    let u = srv.cmd_long(&format!("data get entity {sel} UUID"))?;
    let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
    anyhow::ensure!(ints.len() == 4, "the UUID of {sel}: {u}");
    let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
    Ok(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

/// The entity line's NBT with the replay's tags: the vehicle e<n>, each passenger compound (`{id:"`)
/// e<n>p<k> in order. Returns the NBT and the passengers' kinds.
fn tagged(nbt: &str, n: usize) -> (String, Vec<String>) {
    let s = if nbt.trim().is_empty() { "{}".to_string() } else { nbt.trim().to_string() };
    let body = s.strip_prefix('{').unwrap_or(&s);
    let out = if body.trim_start().starts_with('}') { format!("{{Tags:[\"e{n}\"]{body}") } else { format!("{{Tags:[\"e{n}\"],{body}") };
    let mut kinds = Vec::new();
    let mut k = 0;
    let mut res = String::new();
    let mut rest = out.as_str();
    while let Some(i) = rest.find("{id:\"") {
        res.push_str(&rest[..i]);
        let after = &rest[i + 5..];
        let kind: String = after.chars().take_while(|c| *c != '"').collect();
        kinds.push(kind);
        res.push_str(&format!("{{Tags:[\"e{n}p{k}\"],id:\""));
        k += 1;
        rest = after;
    }
    res.push_str(rest);
    (res, kinds)
}

/// One `+move` line: "move cart req (x, y, z) got (x, y, z) hcoll B vcoll B box AABB[x, y, z] -> [x, y, z] @…"
struct MoveRow {
    req: [f64; 3],
    got: [f64; 3],
    hcoll: bool,
    vcoll: bool,
    bmin: [f64; 3],
    bmax: [f64; 3],
}

fn parse_move(l: &str) -> Option<MoveRow> {
    let triple = |s: &str| -> Option<[f64; 3]> {
        let v: Vec<f64> = s.split(',').map(|n| n.trim().parse().ok()).collect::<Option<_>>()?;
        (v.len() == 3).then(|| [v[0], v[1], v[2]])
    };
    let req = triple(l.split("req (").nth(1)?.split(')').next()?)?;
    let got = triple(l.split("got (").nth(1)?.split(')').next()?)?;
    let hcoll = l.split("hcoll ").nth(1)?.starts_with("true");
    let vcoll = l.split("vcoll ").nth(1)?.starts_with("true");
    let bx = l.split("box AABB[").nth(1)?;
    let bmin = triple(bx.split(']').next()?)?;
    let bmax = triple(bx.split("-> [").nth(1)?.split(']').next()?)?;
    Some(MoveRow { req, got, hcoll, vcoll, bmin, bmax })
}

/// (G1 10:36) The drop-entry envelope from `+move` rows: per lane (x0 = -1 / 3 / 7), each cart pass
/// through the drop region (a run of a cart's rows with no gap over 5 gt) with its off-rail ticks on
/// C's support's top (the box's floor at y 24.000: resting on (x0,23,7)'s glass, not on a rail) and its
/// largest lateral displacement of the box centre from the lane's centre x0 + 0.5 while at or above
/// y 23.5; then N, min / median / max of both per lane, and the passes at each maximum.
pub fn drop_envelope(files: &[std::path::PathBuf]) -> Result<()> {
    struct Pass {
        lane: i32,
        cart: String,
        start: u32,
        last: u32,
        off: u32,
        lat: f64,
        /// (ticks since the pass started, box centre x) at or above y 23.5, for the corridor-relative lateral
        path: Vec<(f64, f64)>,
    }
    let mut done: Vec<(String, Pass)> = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(f)?;
        let mut open: BTreeMap<String, Pass> = BTreeMap::new();
        let mut rows = 0usize;
        for l in text.lines() {
            let Some(rest) = l.trim_start().strip_prefix('t') else { continue };
            let Some((ts, body)) = rest.split_once(": move cart req ") else { continue };
            let Ok(t) = ts.parse::<u32>() else { continue };
            let line = format!("move cart req {body}");
            let Some(m) = parse_move(&line) else { continue };
            let cart = body.rsplit(" @").next().unwrap_or("?").split_whitespace().next().unwrap_or("?").to_string();
            rows += 1;
            let cx = (m.bmin[0] + m.bmax[0]) / 2.0;
            let y0 = m.bmin[1] - DY as f64;
            let Some(lane) = [-1i32, 3, 7].into_iter().find(|&x0| (cx - (x0 as f64 + 0.5)).abs() < 1.5) else { continue };
            // (G1 12:09) off-rail only: a cart on a rail moves with y 0 inside moveAlongTrack (199-203) and
            // its in-move box on a slope reads the slope block's top, so the box floor alone can't tell
            // C's glass from a slope; an off-rail move (comeOffTrack) carries gravity in its y
            let off = u32::from((y0 - 24.0).abs() < MTH_EQUAL && m.req[1].abs() >= MTH_EQUAL);
            let lat = if y0 >= 23.5 { (cx - (lane as f64 + 0.5)).abs() } else { 0.0 };
            match open.get_mut(&cart) {
                Some(p) if p.lane == lane && t <= p.last + 5 => {
                    p.last = t;
                    p.off += off;
                    p.lat = p.lat.max(lat);
                    if y0 >= 23.5 {
                        p.path.push(((t - p.start) as f64, cx));
                    }
                }
                _ => {
                    if let Some(p) = open.remove(&cart) {
                        done.push((f.display().to_string(), p));
                    }
                    let path = if y0 >= 23.5 { vec![(0.0, cx)] } else { Vec::new() };
                    open.insert(cart.clone(), Pass { lane, cart, start: t, last: t, off, lat, path });
                }
            }
        }
        anyhow::ensure!(rows > 0, "{}: no +move rows (run drive-replay with MCTAS_REPLAY_MOVE over the drop region, unclipped)", f.display());
        done.extend(open.into_values().map(|p| (f.display().to_string(), p)));
    }
    println!("drop-entry envelope over {} sample(s): {} pass(es)", files.len(), done.len());
    for lane in [-1i32, 3, 7] {
        let ps: Vec<&(String, Pass)> = done.iter().filter(|(_, p)| p.lane == lane).collect();
        if ps.is_empty() {
            println!("  lane {lane}: no passes (NOT JUDGED)");
            continue;
        }
        let mut offs: Vec<u32> = ps.iter().map(|(_, p)| p.off).collect();
        offs.sort_unstable();
        let mut lats: Vec<f64> = ps.iter().map(|(_, p)| p.lat).collect();
        lats.sort_by(|a, b| a.total_cmp(b));
        let n = ps.len();
        println!(
            "  lane {lane}: N {n} passes; off-rail ticks on C's top min {} / median {} / max {}; lateral displacement min {:.4} / median {:.4} / max {:.4}",
            offs[0],
            offs[n / 2],
            offs[n - 1],
            lats[0],
            lats[n / 2],
            lats[n - 1]
        );
        for (f, p) in ps.iter().filter(|(_, p)| p.off == offs[n - 1] || (p.lat - lats[n - 1]).abs() < 1e-12) {
            println!("    at a maximum: {} t{}..t{} off-rail {} lateral {:.4} ({f})", p.cart, p.start, p.last, p.off, p.lat);
        }
        // (ironspec 10:58 (2)) the lateral CORRIDOR-RELATIVE: passes grouped by their entry (the first
        // point at y >= 23.5 west of the lane's centre by more than 0.2 = from the S-bend's c3 arm, else
        // from D); per entry, the corridor is the median box-centre x at each tick since the pass's
        // start over that entry's passes (a path can cross one z twice, so ticks, not z, index it); a
        // pass's lateral is its largest |x - corridor(tick)|
        for (entry, west) in [("D", false), ("c3", true)] {
            let group: Vec<&&(String, Pass)> = ps.iter().filter(|(_, p)| p.path.first().is_some_and(|&(_, x)| (x < lane as f64 + 0.3) == west)).collect();
            if group.is_empty() {
                println!("    entry {entry}: no passes");
                continue;
            }
            let bin = |k: f64| k as i64;
            let mut bins: BTreeMap<i64, Vec<f64>> = BTreeMap::new();
            for (_, p) in &group {
                for &(z, x) in &p.path {
                    bins.entry(bin(z)).or_default().push(x);
                }
            }
            let corridor: BTreeMap<i64, f64> = bins
                .into_iter()
                .map(|(k, mut v)| {
                    v.sort_by(|a, b| a.total_cmp(b));
                    (k, v[v.len() / 2])
                })
                .collect();
            let mut rel: Vec<(f64, &Pass, &String)> = group.iter().map(|(f, p)| (p.path.iter().map(|&(z, x)| (x - corridor[&bin(z)]).abs()).fold(0.0, f64::max), p, f)).collect();
            rel.sort_by(|a, b| a.0.total_cmp(&b.0));
            let m = rel.len();
            println!(
                "    entry {entry}: N {m} passes; corridor-relative lateral min {:.4} / median {:.4} / max {:.4} (the max: {} t{}..t{} in {})",
                rel[0].0,
                rel[m / 2].0,
                rel[m - 1].0,
                rel[m - 1].1.cart,
                rel[m - 1].1.start,
                rel[m - 1].1.last,
                rel[m - 1].2
            );
        }
    }
    Ok(())
}

/// (ironspec 10:38 (9)) The scene and the drive translated by `by` (the built chunk phase is +6,0,+6):
/// every block, block entity, port and entity of the scene, and the drive's `home` / `set` act cells;
/// `# expect` lines are dropped (their states are in the old frame), so the shifted drive is open loop.
/// (G1 on U) Translate every coordinate triplet (`x,y,z`, three integers) at parenthesis depth 0 of a
/// record's metadata value by `by`; text in parentheses (notes, often in another frame: an offset, a
/// module's local cells) is kept as written. A run of 4 or more comma-joined integers at depth 0 is
/// ambiguous and refuses. Returns the new value and how many triplets moved.
pub(crate) fn shift_meta_value(s: &str, by: [i32; 3]) -> Result<(String, usize)> {
    let mut out = String::new();
    let mut word = String::new();
    let mut depth = 0i32;
    let mut moved = 0usize;
    let flush = |word: &mut String, out: &mut String, depth: i32, moved: &mut usize| -> Result<()> {
        // a list's separating commas around the word are kept as they were
        let lead = word.len() - word.trim_start_matches(',').len();
        let core = word.trim_matches(',');
        let trail = word.len() - lead - core.len();
        let parts: Vec<&str> = core.split(',').collect();
        let ints = parts.iter().filter(|x| x.parse::<i32>().is_ok()).count();
        if depth == 0 && parts.len() == 3 && ints == 3 {
            let v: Vec<i32> = parts.iter().map(|x| x.parse::<i32>().unwrap_or(0)).collect();
            out.push_str(&word[..lead]);
            out.push_str(&format!("{},{},{}", v[0] + by[0], v[1] + by[1], v[2] + by[2]));
            out.push_str(&word[word.len() - trail..]);
            *moved += 1;
        } else {
            anyhow::ensure!(depth != 0 || ints < 4 || ints != parts.len(), "an ambiguous run of {ints} integers `{core}` (not one x,y,z)");
            out.push_str(word);
        }
        word.clear();
        Ok(())
    };
    for c in s.chars() {
        if c.is_ascii_digit() || c == '-' || c == ',' {
            word.push(c);
        } else {
            flush(&mut word, &mut out, depth, &mut moved)?;
            if c == '(' {
                depth += 1;
            } else if c == ')' {
                depth -= 1;
            }
            out.push(c);
        }
    }
    flush(&mut word, &mut out, depth, &mut moved)?;
    Ok((out, moved))
}

pub fn shift_scene(scene: &Path, drive: &Path, by: [i32; 3], scene_out: &Path, drive_out: &Path) -> Result<()> {
    let mut d = mctas::design::Design::load(scene)?;
    let sh = |p: &[i32; 3]| [p[0] + by[0], p[1] + by[1], p[2] + by[2]];
    d.blocks = d.blocks.iter().map(|(p, s)| (sh(p), s.clone())).collect();
    // (ironspec 13:43 (b)) coordinates inside block-entity data can't be translated: refused by cell
    let be_bad: Vec<String> = d.block_entities.iter().filter(|(_, s)| s.contains("[I;") || s.contains("Command")).map(|(p, _)| format!("({},{},{})", p[0], p[1], p[2])).collect();
    anyhow::ensure!(be_bad.is_empty(), "shift-scene refuses: block entities at {be_bad:?} carry data with coordinates it doesn't translate");
    d.block_entities = d.block_entities.iter().map(|(p, s)| (sh(p), s.clone())).collect();
    for p in d.ports.values_mut() {
        *p = sh(p);
    }
    // (ironspec 13:43 (b)) an entity's NBT positions: each `pos:[I;x,y,z]` (a brain memory's GlobalPos,
    // e.g. HOME) is translated; any other int array is refused by name
    let mut nbt_moved = 0usize;
    for (ei, e) in d.entities.iter_mut().enumerate() {
        for (i, v) in e.pos.iter_mut().enumerate() {
            *v += by[i] as f64;
        }
        if let Some(n) = e.nbt.as_mut() {
            let mut out = String::new();
            let mut rest = n.as_str();
            while let Some(i) = rest.find("pos:[I;") {
                out.push_str(&rest[..i + 7]);
                let after = &rest[i + 7..];
                let end = after.find(']').ok_or_else(|| anyhow::anyhow!("shift-scene: entity {ei}: an unclosed pos:[I; array"))?;
                let v: Vec<i32> = after[..end].split(',').map(|s| s.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "shift-scene: entity {ei}: pos:[I;…] with {} values", v.len());
                out.push_str(&format!("{},{},{}", v[0] + by[0], v[1] + by[1], v[2] + by[2]));
                nbt_moved += 1;
                rest = &after[end..];
            }
            out.push_str(rest);
            let stray = out.matches("[I;").count() - out.matches("pos:[I;").count();
            anyhow::ensure!(stray == 0, "shift-scene refuses: entity {ei} ({}) carries {stray} int array(s) other than pos:[I;…] it doesn't translate", e.kind);
            *n = out;
        }
    }
    // the Nether part (the bench's placeholder) moves with the rest, keeping its own layout
    d.nether_blocks = d.nether_blocks.iter().map(|(p, s)| (sh(p), s.clone())).collect();
    d.nether_block_entities = d.nether_block_entities.iter().map(|(p, s)| (sh(p), s.clone())).collect();
    for p in d.nether_ports.values_mut() {
        *p = sh(p);
    }
    // (G1 on U) the record's metadata: every coordinate triplet at the top level of every value is
    // translated (the keys are cell lists: positions, run-cells, keep-*, placement-order-ts, update-only-ts,
    // held-ts, …), text in parentheses kept as written (notes, often in another frame); an ambiguous run of
    // integers refuses by key; `shifted-by` records the cumulative shift
    let mut meta_moved: Vec<String> = Vec::new();
    for (k, v) in d.meta.iter_mut() {
        let (nv, n) = shift_meta_value(v, by).map_err(|e| anyhow::anyhow!("shift-scene refuses: metadata key `{k}`: {e}"))?;
        if n > 0 {
            meta_moved.push(format!("{k} ({n})"));
            *v = nv;
        }
    }
    if !d.meta.contains_key("shifted-by") {
        d.meta.insert("shifted-by".into(), format!("{},{},{} (the cells at each value's top level translated; text in parentheses as written, in its own frame)", by[0], by[1], by[2]));
    }
    d.save(scene_out)?;
    let text = std::fs::read_to_string(drive)?;
    let (mut out, mut moved, mut dropped) = (String::new(), 0usize, 0usize);
    for (li, l) in text.lines().enumerate() {
        if l.starts_with("# expect") {
            dropped += 1;
            continue;
        }
        let w: Vec<&str> = l.split_whitespace().collect();
        let shifted = match w.as_slice() {
            ["act", t, "home", who, x, y, z, rest @ ..] => Some(format!("act {t} home {who} {} {} {}{}", x.parse::<i32>()? + by[0], y.parse::<i32>()? + by[1], z.parse::<i32>()? + by[2], rest.iter().map(|r| format!(" {r}")).collect::<String>())),
            ["act", _, "set", _, _, _, rest @ ..] if rest.iter().any(|r| r.contains('{')) => {
                anyhow::bail!("shift-scene refuses: drive line {}: a set act with block-entity data ({}), whose coordinates it doesn't translate", li + 1, l.trim())
            }
            ["act", t, "set", x, y, z, rest @ ..] => Some(format!("act {t} set {} {} {}{}", x.parse::<i32>()? + by[0], y.parse::<i32>()? + by[1], z.parse::<i32>()? + by[2], rest.iter().map(|r| format!(" {r}")).collect::<String>())),
            ["act", _, "last-slept", ..] => None,
            ["act", _, verb, ..] => anyhow::bail!("shift-scene refuses: drive line {}: an act verb `{verb}` it doesn't translate", li + 1),
            _ => None,
        };
        match shifted {
            Some(s) => {
                moved += 1;
                out.push_str(&s);
            }
            None => out.push_str(l),
        }
        out.push('\n');
    }
    std::fs::write(drive_out, out)?;
    println!(
        "shift-scene by {by:?}: {} blocks, {} entities, {} ports moved -> {} (md5 {}); drive acts moved {moved}, expect lines dropped {dropped} (the shifted drive runs open loop) -> {} (md5 {})",
        d.blocks.len(),
        d.entities.len(),
        d.ports.len(),
        scene_out.display(),
        crate::md5::md5_hex(&std::fs::read(scene_out)?),
        drive_out.display(),
        crate::md5::md5_hex(&std::fs::read(drive_out)?)
    );
    println!(
        "  entity NBT positions (pos:[I;…]) moved {nbt_moved}; the Nether part moved with it ({} blocks, {} ports); the record's metadata: every coordinate triplet translated in the keys {}",
        d.nether_blocks.len(),
        d.nether_ports.len(),
        if meta_moved.is_empty() { "(none)".to_string() } else { meta_moved.join(", ") }
    );
    Ok(())
}

fn cell_state(srv: &mut VanillaServer, p: [i32; 3]) -> Result<String> {
    let q = [p[0], p[1] + DY, p[2]];
    let snap = srv.snapshot(q, q)?;
    Ok(snap.region.get(q).unwrap_or("?").to_string())
}

fn pos_of(srv: &mut VanillaServer, sel: &str) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long(&format!("data get entity {sel} Pos"))?;
    let v: Vec<f64> = out.split('[').nth(1).unwrap_or("").split(',').filter_map(|s| s.trim().trim_end_matches(|c: char| c == ']' || c == 'd' || c.is_whitespace()).parse().ok()).collect();
    Ok(if v.len() == 3 { Some([v[0], v[1] - DY as f64, v[2]]) } else { None })
}

fn fmt_pos(p: Option<[f64; 3]>) -> String {
    p.map(|p| format!("({:.2},{:.2},{:.2})", p[0], p[1], p[2])).unwrap_or_else(|| "?".into())
}

/// (G1's divergence check) every scene cart's block (frame y) and rider, from the game.
fn cart_states(srv: &mut VanillaServer, carts: &[String]) -> Result<Vec<(String, [i32; 3], Option<String>)>> {
    let mut out = Vec::new();
    for c in carts {
        let Some(p) = pos_of(srv, &format!("@e[tag={c},limit=1]"))? else { continue };
        let r = srv.cmd_long(&format!("execute as @e[tag={c},limit=1] on passengers run data get entity @s Tags"))?;
        let rider = r.split('"').find(|s| s.starts_with('e') && s.contains('p') && s[1..].chars().all(|ch| ch.is_ascii_digit() || ch == 'p')).map(|s| s.to_string());
        out.push((c.clone(), [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32], rider));
    }
    Ok(out)
}

/// The game's state in the drive's `# expect` form for a write at `cell` (names without the dot),
/// each scene index renamed through `names` (the drive's own numbering, where it differs).
fn expect_form(states: &[(String, [i32; 3], Option<String>)], cell: [i32; 3], names: &BTreeMap<usize, usize>) -> String {
    let rn = |s: &str| -> String {
        // eN or eNpK
        let body = &s[1..];
        let (n, rest) = match body.find('p') {
            Some(i) => (&body[..i], &body[i..]),
            None => (body, ""),
        };
        match n.parse::<usize>() {
            Ok(k) => format!("e{}{rest}", names.get(&k).copied().unwrap_or(k)),
            Err(_) => s.to_string(),
        }
    };
    let who = |c: &str, r: &Option<String>| match r {
        Some(r) => format!("{} with {}", rn(c), rn(r)),
        None => rn(c),
    };
    let col: Vec<String> = states
        .iter()
        .filter(|(_, b, _)| b[0] == cell[0] && b[2] == cell[2] && (cell[1] - 1..=cell[1] + 1).contains(&b[1]))
        .map(|(c, b, r)| format!("{} at y{:+}", who(c, r), b[1] - cell[1]))
        .collect();
    let at = |q: [i32; 3]| states.iter().find(|(_, b, _)| *b == q).map(|(c, _, r)| who(c, r)).unwrap_or_else(|| "none".into());
    format!(
        "carts {}; holds -1:{} 3:{} 7:{}; pockets -1:{} 3:{} 7:{}",
        if col.is_empty() { "none".to_string() } else { col.join(", ") },
        at([-1, 16, 0]),
        at([3, 16, 0]),
        at([7, 16, 0]),
        at([-1, 0, 0]),
        at([3, 0, 0]),
        at([7, 0, 0])
    )
}

#[derive(Default)]
struct Spell {
    start: u32,
    at: String,
    hostile: bool,
    wt_at_entry: Option<String>,
    sink_stop: Option<u32>,
    home_set: Option<(u32, String)>,
    home_path: Option<(bool, String)>,
    armed: Option<u32>,
    released: Option<(u32, String)>,
    sight: String,
}

impl Spell {
    fn describe(&self, end: Option<u32>) -> String {
        format!(
            "REST t{}..{} ({} gt) at {} hostile {}; WALK_TARGET at entry {}; the sink's stop {}; HOME targeted {}; its path {}; stamp armed {}; released {}{}",
            self.start,
            end.map(|e| format!("t{e}")).unwrap_or_else(|| "(open)".into()),
            end.map(|e| (e - self.start).to_string()).unwrap_or_else(|| "?".into()),
            self.at,
            self.hostile,
            self.wt_at_entry.clone().unwrap_or_else(|| "absent".into()),
            self.sink_stop.map(|t| format!("t{t}")).unwrap_or_else(|| "-".into()),
            self.home_set.as_ref().map(|(t, a)| format!("t{t} (stamp age then {a})")).unwrap_or_else(|| "never in the spell".into()),
            self.home_path.as_ref().map(|(c, s)| format!("{} start {s}", if *c { "complete" } else { "PARTIAL" })).unwrap_or_else(|| "-".into()),
            self.armed.map(|t| format!("t{t}")).unwrap_or_else(|| "-".into()),
            self.released.as_ref().map(|(t, a)| format!("t{t} (stamp age then {a})")).unwrap_or_else(|| "-".into()),
            if self.sight.is_empty() { String::new() } else { format!("; sight: {}", self.sight) }
        )
    }
}

#[derive(Default)]
struct V {
    tag: String,
    gone: Option<String>,
    hurts: Vec<String>,
    /// WALK_TARGET held (its source) or absent
    wt: Option<String>,
    spell: Option<Spell>,
    spells: Vec<String>,
    /// at-entry judgements: (tick, stamp age or none) at each first SetWalkTargetFromBlockMemory run per spell
    entry_judged: Vec<(u32, Option<i64>)>,
    /// (ironspec 09:42 (R1)) the release branch's runs, HOME held now, the lie-downs (LAST_SLEPT
    /// sets), and the current stop visit (since, HOME held on arrival)
    releases: u32,
    has_home: bool,
    slept: Vec<u32>,
    at_stop: Option<(u32, bool)>,
    stop_least: f64,
    near_stop: bool,
    stop_done: bool,
    /// the least air supply read (every 20 gt): (air, tick, position)
    min_air: Option<(i32, u32, String)>,
    activity: String,
    hostile: bool,
    /// (farmopt 11:22) the NEAREST_HOSTILE memory's value as last set (its PANIC trigger), and the
    /// latest damage taken (the HURT_BY route)
    hostile_what: String,
    last_hurt: Option<String>,
    home_path: Option<(bool, String)>,
    stamp: Option<i64>,
    oldest: Option<(i64, u32)>,
    rest_partial: u32,
    rest_complete: u32,
    rest_entries: Vec<String>,
    stamps_armed: Vec<String>,
    home_lost: Vec<String>,
    home_set: Vec<String>,
    dawn: Vec<String>,
    least_occupied: Option<(f64, u32)>,
}

pub fn drive_replay(version: &str, scene: &Path, drive: &Path, ticks: u32, day_start: i64, bed: [i32; 3], loads: &[u32]) -> Result<()> {
    let (t0, acts) = parse_drive(drive)?;
    println!(
        "== drive-replay on the official {version} server: the scene {} (+{DY} y), the drive {} ({} acts, t0 {t0}) OPEN LOOP, the day time {day_start} at t 0, {ticks} gt (past the drive's last act the controller is idle), weather clear; the bed head ({},{},{}); world opens (save, stop, start) before t {loads:?}",
        scene.display(),
        drive.display(),
        acts.len(),
        bed[0],
        bed[1],
        bed[2]
    );
    if !acts.iter().any(|a| a.expect.is_some()) {
        println!("  OPEN LOOP: the drive carries no recorded state per act, so the divergence check (the rider / cart each act assumes) is UNAVAILABLE: the controller's coverage is NOT JUDGED by this row; its bars judge the mechanics it reaches");
    } else {
        println!("  CLOSED-LOOP CHECKED: {} write(s) carry redsim's recorded state (`# expect`), compared with the game's before each write (exact: the carts in the act's column with their riders and block-y offsets, each hold's and pocket's cart and rider); acts are applied between ticks with the server frozen (`tick step`), as f1q2-run applies them after its tick", acts.iter().filter(|a| a.expect.is_some()).count());
    }
    let jar = mctas::bridge::brain_agent(version)?;
    // (cc767b78's t166 brake) MCTAS_REPLAY_PUSH=<from>-<to>: every minecart push (the cart's delta
    // before / after, both boxes, the pusher and its vehicle) and every seated villager's push list in
    // that window, entities named by their scene tags
    let push_window: Option<(u32, u32)> = match std::env::var("MCTAS_REPLAY_PUSH") {
        Ok(s) => {
            let (a, b) = s.split_once('-').ok_or_else(|| anyhow::anyhow!("MCTAS_REPLAY_PUSH=from-to, got {s}"))?;
            Some((a.trim().parse()?, b.trim().parse()?))
        }
        Err(_) => None,
    };
    // (ironspec 09:47 / 09:53) MCTAS_REPLAY_MOVE=<from>-<to>: every minecart's Entity.move in the window
    // (requested delta, the movement it got, the collision flags), named by scene tag
    let move_window: Option<(u32, u32)> = match std::env::var("MCTAS_REPLAY_MOVE") {
        Ok(s) => {
            let (a, b) = s.split_once('-').ok_or_else(|| anyhow::anyhow!("MCTAS_REPLAY_MOVE=from-to, got {s}"))?;
            Some((a.trim().parse()?, b.trim().parse()?))
        }
        Err(_) => None,
    };
    let opts = |world: Option<std::path::PathBuf>| VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        keep_dir: true,
        world,
        java_agent: Some(std::path::PathBuf::from(format!("{}={MEMS}{}{}", jar.display(), if push_window.is_some() { ",+push" } else { "" }, if move_window.is_some() { ",+move" } else { "" }))),
        ..Default::default()
    };
    let mut srv = VanillaServer::start(&opts(None))?;
    // (G1 12:10: the far position) the forceload area follows the scene's own bounds (a shifted scene,
    // e.g. x / z + 16,384, is loaded where it is)
    let forceload = match mctas::design::Design::load(scene)?.bounds() {
        Some((lo, hi)) => format!("forceload add {} {} {} {}", lo[0].min(-32), lo[2].min(-32), hi[0].max(31), hi[2].max(31)),
        None => "forceload add -32 -32 31 31".to_string(),
    };
    let fl_span = |c: &str| -> i64 {
        let n: Vec<i64> = c.split_whitespace().skip(2).filter_map(|v| v.parse().ok()).collect();
        if n.len() == 4 { ((n[2] >> 4) - (n[0] >> 4) + 1) * ((n[3] >> 4) - (n[1] >> 4) + 1) } else { 0 }
    };
    // a far scene keeps the origin's 4 x 4 chunks too only when they fit the command's 256-chunk cap
    let forceload = if fl_span(&forceload) > 256 {
        let (lo, hi) = mctas::design::Design::load(scene)?.bounds().expect("bounds above");
        format!("forceload add {} {} {} {}", lo[0] - 16, lo[2] - 16, hi[0] + 16, hi[2] + 16)
    } else {
        forceload
    };
    srv.cmd_ok(&forceload)?;
    // (G1 on G #9 / H #2) the scene is summoned under the run's OWN day time: a brain takes its
    // schedule activity when it is built (registerBrainGoals) and then updates it at most once every
    // > 20 gt (Brain.updateActivityFromSchedule 327-335), so a night-built brain would sit in REST for
    // the run's first ~21 gt
    srv.cmd_ok(&format!("time set {day_start}"))?;
    crate::seatclear::place_blocks(&mut srv, scene)?;
    let d = mctas::design::Design::load(scene)?;
    // (ironspec 10:38 (7)) EVERY T in the scene asserted at t0 and after the last tick against the
    // record's declared shape: a plain rail with rails on 3 horizontal sides (RailState's raw count:
    // the side, one up or one down, any rail block, 90-92 / 121-129)
    let mut switched_ts: Vec<String> = Vec::new();
    let mut switched_cells: Vec<([i32; 3], Vec<String>)> = Vec::new();
    let t_asserts: Vec<([i32; 3], String)> = {
        let is_rail = |p: [i32; 3]| d.blocks.get(&p).is_some_and(|s| s.split('[').next().unwrap_or("").ends_with("rail"));
        let mut v = Vec::new();
        for (p, s) in &d.blocks {
            let name = s.split('[').next().unwrap_or("");
            if name != "minecraft:rail" && name != "rail" {
                continue;
            }
            let sides = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]]
                .iter()
                .filter(|o| (-1..=1).any(|dy| is_rail([p[0] + o[0], p[1] + dy, p[2] + o[2]])))
                .count();
            if sides == 3 {
                let shape = s.split("shape=").nth(1).and_then(|r| r.split([',', ']']).next()).ok_or_else(|| anyhow::anyhow!("the T at {p:?} declares no shape: {s}"))?;
                // a T with a signal source within Manhattan distance 2 (every component's update
                // reach: RailBlock.updateState re-shapes on a source's update) is SWITCHED by design:
                // its shape follows its control's power, so it's printed, not asserted, until the
                // record declares it (`switches:`)
                const SOURCES: [&str; 20] = [
                    "lever", "redstone_torch", "redstone_wall_torch", "detector_rail", "redstone_wire", "repeater", "comparator", "observer", "_button", "_pressure_plate", "redstone_block", "daylight_detector", "target", "tripwire_hook", "lectern", "trapped_chest", "sculk_sensor", "lightning_rod", "jukebox", "calibrated_sculk_sensor",
                ];
                let mut src: Option<String> = None;
                for dx in -2i32..=2 {
                    for dy in -2i32..=2 {
                        for dz in -2i32..=2 {
                            if dx.abs() + dy.abs() + dz.abs() > 2 || (dx, dy, dz) == (0, 0, 0) {
                                continue;
                            }
                            if let Some(b) = d.blocks.get(&[p[0] + dx, p[1] + dy, p[2] + dz]) {
                                let n = b.split('[').next().unwrap_or("");
                                if SOURCES.iter().any(|k| n.ends_with(k)) && src.is_none() {
                                    src = Some(format!("{n} at ({},{},{})", p[0] + dx, p[1] + dy, p[2] + dz));
                                }
                            }
                        }
                    }
                }
                match src {
                    Some(sname) => {
                        // (G1 on N #1) a switched T must still sit in one of its LEGAL shapes: one joining
                        // two of its raw arms (a curve for a perpendicular pair, straight or ascending
                        // for the opposite pair), asserted at t0 and the end
                        let arm = |o: [i32; 3]| (-1..=1).any(|dy| is_rail([p[0] + o[0], p[1] + dy, p[2] + o[2]]));
                        let up = |o: [i32; 3]| is_rail([p[0] + o[0], p[1] + 1, p[2] + o[2]]);
                        let (n, s_, w, e) = (arm([0, 0, -1]), arm([0, 0, 1]), arm([-1, 0, 0]), arm([1, 0, 0]));
                        let mut legal: Vec<String> = Vec::new();
                        for (a, b, name) in [(n, e, "north_east"), (n, w, "north_west"), (s_, e, "south_east"), (s_, w, "south_west")] {
                            if a && b {
                                legal.push(name.to_string());
                            }
                        }
                        if n && s_ {
                            legal.push("north_south".into());
                            if up([0, 0, -1]) {
                                legal.push("ascending_north".into());
                            }
                            if up([0, 0, 1]) {
                                legal.push("ascending_south".into());
                            }
                        }
                        if e && w {
                            legal.push("east_west".into());
                            if up([1, 0, 0]) {
                                legal.push("ascending_east".into());
                            }
                            if up([-1, 0, 0]) {
                                legal.push("ascending_west".into());
                            }
                        }
                        switched_ts.push(format!("({},{},{}) declared {shape}, legal {legal:?}, a source within Manhattan distance 2 (a heuristic: the record's `switches:` lines replace it): {sname}", p[0], p[1], p[2]));
                        switched_cells.push((*p, legal));
                    }
                    None => v.push((*p, format!("minecraft:rail[shape={shape}]"))),
                }
            }
        }
        v
    };
    println!("  the scene's Ts (plain rails with rails on 3 sides): {} asserted at t0 and after the last tick; {} SWITCHED (a signal source within distance 2: printed, not asserted, until the record declares them)", t_asserts.len(), switched_ts.len());
    for s in &switched_ts {
        println!("    switched T {s}");
    }
    let mut by_uuid: BTreeMap<String, usize> = BTreeMap::new();
    let mut vs: Vec<V> = Vec::new();
    let mut cart_tags: Vec<String> = Vec::new();
    let mut uuid_names: Vec<(String, String)> = Vec::new();
    let mut push_rows: Vec<String> = Vec::new();
    // (G1 on H #2) each villager's activity as its brain is built at the summon: the agent prints it
    // before the brain knows its owner ("@none"), so the lines between two summons are that entity's
    let mut slog = Log { at: std::fs::metadata(&srv.log)?.len() };
    let mut built: BTreeMap<String, String> = BTreeMap::new();
    for (n, e) in d.entities.iter().enumerate() {
        let (nbt, kinds) = tagged(e.nbt.as_deref().unwrap_or(""), n);
        let out = srv.cmd(&format!("summon {} {} {} {} {nbt}", e.kind, e.pos[0], e.pos[1] + DY as f64, e.pos[2]))?;
        anyhow::ensure!(out.contains("Summoned new"), "entity line {n} ({} at {:?}): {}", e.kind, e.pos, out.trim());
        let acts_built: Vec<String> = slog.drain(&srv)?.into_iter().filter_map(|l| l.strip_prefix("activity ").map(|r| r.split_whitespace().next().unwrap_or("").trim_start_matches("minecraft:").to_string())).collect();
        let villagers = kinds.iter().filter(|k| *k == "minecraft:villager").count();
        if villagers == 1
            && let Some(last) = acts_built.last()
        {
            built.insert(format!("e{n}p{}", kinds.iter().position(|k| k == "minecraft:villager").unwrap_or(0)), last.clone());
        }
        if e.kind.contains("minecart") {
            cart_tags.push(format!("e{n}"));
            if push_window.is_some() || move_window.is_some() {
                uuid_names.push((uuid_hex(&mut srv, &format!("@e[tag=e{n},limit=1]"))?, format!("e{n}")));
            }
        }
        for (k, kind) in kinds.iter().enumerate() {
            if kind == "minecraft:villager" {
                let tag = format!("e{n}p{k}");
                let u = uuid_hex(&mut srv, &format!("@e[tag={tag},limit=1]"))?;
                if push_window.is_some() {
                    uuid_names.push((u.clone(), tag.clone()));
                }
                by_uuid.insert(u, vs.len());
                vs.push(V { tag, ..Default::default() });
            }
        }
    }
    println!("  the scene's entity lines summoned: {}; villagers named: {} ({})", d.entities.len(), vs.len(), vs.iter().map(|v| v.tag.as_str()).collect::<Vec<_>>().join(" "));
    // (ironspec 09:21 (2)(a)) MCTAS_REPLAY_NOAI=1: the pure-cart control: every rider NoAI (no brain,
    // no travel: Mob.isEffectiveAi 1311-1312, LivingEntity.aiStep 3087 / 3135), still seated and still
    // pushing from its previous seat; cart timing only (no REST, no PANIC)
    let noai = std::env::var_os("MCTAS_REPLAY_NOAI").is_some();
    if noai {
        for v in &vs {
            srv.cmd_ok(&format!("data merge entity @e[tag={},limit=1] {{NoAI:1b}}", v.tag))?;
            anyhow::ensure!(crate::strict::classify_test("noai", &srv.cmd_long(&format!("execute if entity @e[tag={},nbt={{NoAI:1b}},limit=1]", v.tag))?)?, "{}: NoAI not set", v.tag);
        }
        println!("  PURE-CART CONTROL: every rider NoAI:1b (no brain, no travel); a fidelity-only row (cart timing)");
    }
    // (G1 / ironspec 07:57 (2)) the scene's own `# eN` names asserted at t0: each tagged line's name is
    // its spawn index (the replay's tag), and the game's entity has the line's type and position
    {
        let text = std::fs::read_to_string(scene)?;
        let mut pending: Option<String> = None;
        let mut named: Vec<(String, String, [f64; 3])> = Vec::new();
        for l in text.lines() {
            let l = l.trim();
            if let Some(n) = l.strip_prefix("# e")
                && !n.is_empty()
                && n.chars().all(|c| c.is_ascii_digit())
            {
                pending = Some(format!("e{n}"));
            } else if let Some(rest) = l.strip_prefix("entity:") {
                let w: Vec<&str> = rest.split_whitespace().collect();
                anyhow::ensure!(w.len() >= 4, "a short entity line: {l}");
                let pos = [w[1].parse::<f64>()?, w[2].parse::<f64>()?, w[3].parse::<f64>()?];
                named.push((pending.take().unwrap_or_default(), w[0].to_string(), pos));
            }
        }
        let tagged_lines = named.iter().filter(|(n, _, _)| !n.is_empty()).count();
        if tagged_lines > 0 {
            anyhow::ensure!(tagged_lines == named.len(), "{} of {} entity lines carry a `# eN` name: all or none", tagged_lines, named.len());
            for (k, (name, kind, pos)) in named.iter().enumerate() {
                anyhow::ensure!(*name == format!("e{k}"), "entity line {k} is named {name}: the drive's names must be the spawn order");
                let p = pos_of(&mut srv, &format!("@e[tag=e{k},limit=1]"))?.ok_or_else(|| anyhow::anyhow!("{name}: not in the world at t0"))?;
                anyhow::ensure!((p[0] - pos[0]).abs() < 1e-6 && (p[1] - pos[1]).abs() < 1e-6 && (p[2] - pos[2]).abs() < 1e-6, "{name}: at ({:.6},{:.6},{:.6}) at t0, its line says {pos:?}", p[0], p[1], p[2]);
                let ty = srv.cmd_long(&format!("execute if entity @e[tag=e{k},type={kind},limit=1]"))?;
                anyhow::ensure!(crate::strict::classify_test("type", &ty)?, "{name}: not a {kind} at t0");
            }
            // 26.3 spawns a summoned vehicle before its passengers (ServerLevel.addFreshEntityWithPassengers:
            // getSelfAndPassengers in order), so the entity tick list runs e0, e0.p0, e1, … in file order
            println!("  the scene's `# eN` names: {} lines, each its spawn index, type and t0 position asserted; the game's tick order is the spawn order (each cart before its rider)", named.len());
        }
    }
    srv.cmd_ok(&format!("time set {day_start}"))?;
    srv.cmd_ok("weather clear 1000000")?;
    srv.cmd_ok("gamerule advance_time true")?;
    let gt0 = srv.game_time()?;
    let mut log = Log { at: 0 };
    let mut setup_lines: Vec<String>;
    let (mut home_set_t0, mut slept_set_t0) = (0usize, 0usize);
    let mut declared_home: BTreeSet<String> = BTreeSet::new();
    let mut farm_stop: Vec<String> = Vec::new();
    let mut stop_visits: Vec<String> = Vec::new();
    // (G1 on G #6) the agent hooks this replay reads must have installed (N > 0), and a positive:
    // a scratch villager given 1 drowning damage must print its `hurt drown` line
    {
        // (G1 on G #9) the setup's own agent lines kept: each villager's activity before t0 comes from
        // them (the summon-time brain build, then the switch after the run's day is set)
        setup_lines = Log { at: 0 }.drain(&srv)?;
        let text = std::fs::read_to_string(&srv.log)?;
        for (what, needle, want) in [("+hurt", "LivingEntity reports hurtServer (", 1u32), ("+poi acquire", "AcquirePoi reports findPathToPois's path (", 1), ("+poi tickets", "PoiRecord reports acquireTicket and releaseTicket (", 2)] {
            let n: Option<u32> = text.lines().find_map(|l| l.split(needle).nth(1)).and_then(|r| r.split_whitespace().next()).and_then(|s| s.parse().ok());
            println!("  agent hook {what}: {}", n.map(|n| format!("{n} installed")).unwrap_or_else(|| "NOT installed".into()));
            // (G1 on K #1, ironspec 09:43: every agent hook) the expected count exactly
            anyhow::ensure!(n == Some(want), "the agent hook {what} hooked {n:?}, not exactly {want}: its lines can't be trusted");
        }
        // (ironspec 09:28 (a)) with the push window on, each push hook hooks EXACTLY one method (matched
        // by its descriptor), so a renamed or doubled method can't read as "no pushes"
        if push_window.is_some() || move_window.is_some() {
            let mut hooks: Vec<(&str, &str)> = Vec::new();
            if push_window.is_some() {
                hooks.push(("+push AbstractMinecart.push(Entity)", "AbstractMinecart.push(Entity) reports its entry and returns ("));
                hooks.push(("+push LivingEntity.pushEntities", "LivingEntity.pushEntities reports its entry ("));
            }
            if move_window.is_some() {
                hooks.push(("+move Entity.move(MoverType, Vec3)", "Entity.move(MoverType, Vec3) reports its entry and returns ("));
            }
            for (what, needle) in hooks {
                let n: Option<u32> = text.lines().find_map(|l| l.split(needle).nth(1)).and_then(|r| r.split_whitespace().next()).and_then(|s| s.parse().ok());
                println!("  agent hook {what}: {}", n.map(|n| format!("{n} method(s) hooked")).unwrap_or_else(|| "NOT installed".into()));
                anyhow::ensure!(n == Some(1), "the agent hook {what} hooked {n:?} method(s), not exactly 1: the run fails");
            }
        }
        srv.cmd_ok(&format!("setblock 28 {} 28 minecraft:glass", 100 + DY))?;
        srv.cmd_ok(&format!("summon minecraft:villager 28.5 {} 28.5 {{NoAI:1b,Tags:[\"scratchv\"]}}", 101 + DY))?;
        let su = uuid_hex(&mut srv, "@e[tag=scratchv,limit=1]")?;
        log.at = std::fs::metadata(&srv.log)?.len();
        let out = srv.cmd("damage @e[tag=scratchv,limit=1] 1 minecraft:drown")?;
        // the damage command calls hurtServer at once: the line is read from the log without stepping
        // the world (a setup tick would put the scene one tick ahead of the drive's clock)
        let mut lines = Vec::new();
        let t_wait = std::time::Instant::now();
        while t_wait.elapsed() < std::time::Duration::from_secs(5) {
            lines.extend(log.drain(&srv)?);
            if lines.iter().any(|l: &String| l.starts_with("hurt drown")) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        setup_lines.extend(lines.iter().cloned());
        let hit = lines.iter().any(|l| l.starts_with("hurt drown") && l.ends_with(&format!("@{su}")));
        srv.cmd("kill @e[tag=scratchv]")?;
        srv.cmd(&format!("setblock 28 {} 28 minecraft:air", 100 + DY))?;
        println!("  the +hurt positive (a scratch villager, `damage … 1 minecraft:drown`: {}): {}", out.trim(), if hit { "its `hurt drown` line printed" } else { "NO line" });
        anyhow::ensure!(hit, "the +hurt positive printed no `hurt drown` line: a drowning would go unseen");
    }
    setup_lines.extend(log.drain(&srv)?);
    for v in vs.iter_mut() {
        if let Some(a) = built.get(&v.tag) {
            v.activity = a.clone();
        }
    }
    for l in &setup_lines {
        let Some(&vi) = l.rsplit(" @").next().and_then(|u| by_uuid.get(u.trim())) else { continue };
        if let Some(rest) = l.strip_prefix("activity ") {
            vs[vi].activity = rest.split_whitespace().next().unwrap_or("").trim_start_matches("minecraft:").to_string();
        } else if l.starts_with("set minecraft:nearest_hostile") {
            vs[vi].hostile = true;
        } else if l.starts_with("erase minecraft:nearest_hostile") {
            vs[vi].hostile = false;
        }
    }
    println!(
        "  the brains before t0 (from the setup's own lines): {}",
        vs.iter().map(|v| format!("{}={}", v.tag, if v.activity.is_empty() { "?" } else { v.activity.as_str() })).collect::<Vec<_>>().join(" ")
    );
    let centre = [bed[0] as f64 + 0.5, bed[1] as f64 + 0.5, bed[2] as f64 + 0.5];
    let home_target = format!("BlockPos{{x={}, y={}, z={}}}", bed[0], bed[1] + DY, bed[2]);
    let mut noop: Vec<String> = Vec::new();
    let mut first_seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut events: Vec<String> = Vec::new();
    let mut collisions: Vec<String> = Vec::new();
    let (mut ai, mut applied, mut night_ticks) = (0usize, 0usize, 0u32);
    // (ironspec 07:30 / G1) after each world open, every line of every villager for 60 gt (the blip)
    let mut detail_until: Option<u32> = None;
    let mut detail: Vec<String> = Vec::new();
    // (ironfarm / ironspec 07:30 (3)(a)) each lane's cart count (y 16..26, z 0..8 of its column) and
    // the slope-top V (y >= 21.5, z >= 5.5) every 20 gt
    let lanes = [-1i32, 3, 7];
    let mut lane_max = [0u32; 3];
    let mut v_ticks = [0u32; 3];
    let mut lane_hist: Vec<String> = Vec::new();
    let mut activity_rows: Vec<String> = Vec::new();
    let mut reloaded = false;
    let air_watch = std::env::var("MCTAS_REPLAY_AIR").ok();
    let mut air_rows: Vec<String> = Vec::new();
    let mut state_at: Option<(u32, Vec<(String, [i32; 3], Option<String>)>)> = None;
    let (mut expects_checked, mut first_div, mut divergences) = (0usize, None::<u32>, Vec::<String>::new());
    let plant_at: Option<u32> = std::env::var("MCTAS_REPLAY_PLANT_EXPECT").ok().map(|s| s.parse()).transpose()?;
    let mut planted: Option<u32> = None;
    // (tick of a diverged write, redsim's holds / pockets then, the tick the game first matched them)
    let mut pending: Vec<(u32, String, Option<u32>)> = Vec::new();
    let trace_spec: Option<(String, u32, u32)> = match std::env::var("MCTAS_REPLAY_TRACE") {
        Ok(s) => {
            let (who, w) = s.split_once(':').ok_or_else(|| anyhow::anyhow!("MCTAS_REPLAY_TRACE=eN:from-to, got {s}"))?;
            let (a, b) = w.split_once('-').ok_or_else(|| anyhow::anyhow!("MCTAS_REPLAY_TRACE=eN:from-to, got {s}"))?;
            Some((who.to_string(), a.parse()?, b.parse()?))
        }
        Err(_) => None,
    };
    let mut trace_rows: Vec<String> = Vec::new();
    let cell3 = |s: &str| -> Result<[i32; 3]> {
        let v: Vec<i32> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
        anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
        Ok([v[0], v[1], v[2]])
    };
    let swaps: Vec<([i32; 3], String, u32, u32)> = match std::env::var("MCTAS_REPLAY_SWAP") {
        // several swaps separated by '/', each x,y,z;STATE;t0;t1
        Ok(s) => s
            .split('/')
            .filter(|e| !e.trim().is_empty())
            .map(|e| -> Result<([i32; 3], String, u32, u32)> {
                let f: Vec<&str> = e.split(';').collect();
                anyhow::ensure!(f.len() == 4, "MCTAS_REPLAY_SWAP=x,y,z;STATE;t0;t1[/…], got {e}");
                Ok((cell3(f[0])?, f[1].trim().to_string(), f[2].trim().parse()?, f[3].trim().parse()?))
            })
            .collect::<Result<_>>()?,
        Err(_) => Vec::new(),
    };
    let kills: Vec<(u32, String)> = match std::env::var("MCTAS_REPLAY_KILL") {
        Ok(s) => s
            .split('/')
            .filter(|e| !e.trim().is_empty())
            .map(|e| -> Result<(u32, String)> {
                let (t, sel) = e.split_once(';').ok_or_else(|| anyhow::anyhow!("MCTAS_REPLAY_KILL=t;SELECTOR[/…], got {e}"))?;
                Ok((t.trim().parse()?, sel.trim().to_string()))
            })
            .collect::<Result<_>>()?,
        Err(_) => Vec::new(),
    };
    let injects: Vec<(u32, [f64; 3], [f64; 3], bool)> = match std::env::var("MCTAS_REPLAY_INJECT") {
        Ok(s) => s
            .split('/')
            .filter(|e| !e.trim().is_empty())
            .map(|e| -> Result<(u32, [f64; 3], [f64; 3], bool)> {
                let f: Vec<&str> = e.split(';').collect();
                anyhow::ensure!(f.len() == 3 || f.len() == 4, "MCTAS_REPLAY_INJECT=t;x,y,z;vx,vy,vz[;empty][/…], got {e}");
                let tri = |s: &str| -> Result<[f64; 3]> {
                    let v: Vec<f64> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "three numbers, got {s}");
                    Ok([v[0], v[1], v[2]])
                };
                Ok((f[0].trim().parse()?, tri(f[1])?, tri(f[2])?, f.get(3).is_none_or(|r| r.trim() != "empty")))
            })
            .collect::<Result<_>>()?,
        Err(_) => Vec::new(),
    };
    let hold: Option<([i32; 3], u32, u32)> = match std::env::var("MCTAS_REPLAY_HOLDCART") {
        Ok(s) => {
            let f: Vec<&str> = s.split(';').collect();
            anyhow::ensure!(f.len() == 3, "MCTAS_REPLAY_HOLDCART=x,y,z;t0;t1, got {s}");
            Some((cell3(f[0])?, f[1].trim().parse()?, f[2].trim().parse()?))
        }
        Err(_) => None,
    };
    let mut swap_old: Vec<Option<String>> = vec![None; swaps.len()];
    let mut move_rows: Vec<String> = Vec::new();
    let move_box: Option<[f64; 6]> = match std::env::var("MCTAS_REPLAY_MOVE_BOX") {
        Ok(s) => {
            let v: Vec<f64> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(v.len() == 6, "MCTAS_REPLAY_MOVE_BOX=x0,y0,z0,x1,y1,z1 (frame), got {s}");
            Some([v[0], v[1], v[2], v[3], v[4], v[5]])
        }
        Err(_) => None,
    };
    let move_clipped_only = std::env::var_os("MCTAS_REPLAY_MOVE_CLIPPED").is_some();
    // (a shifted scene, `shift-scene`) MCTAS_REPLAY_DROP_SHIFT=dx,dz moves the drop flags' lanes and
    // their C cells by the scene's translation
    let drop_shift: (f64, f64) = match std::env::var("MCTAS_REPLAY_DROP_SHIFT") {
        Ok(s) => {
            let v: Vec<f64> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(v.len() == 2, "MCTAS_REPLAY_DROP_SHIFT=dx,dz, got {s}");
            (v[0], v[1])
        }
        Err(_) => (0.0, 0.0),
    };
    let (mut drop_clips, mut lip_rests) = (0usize, 0usize);
    let (probes, probe_window): (Vec<([i32; 3], String)>, Option<(u32, u32)>) = match std::env::var("MCTAS_REPLAY_PROBE") {
        Ok(s) => {
            let (cells, win) = s.rsplit_once(':').ok_or_else(|| anyhow::anyhow!("MCTAS_REPLAY_PROBE=x,y,z|PRED;…:from-to, got {s}"))?;
            let (a, b) = win.split_once('-').ok_or_else(|| anyhow::anyhow!("MCTAS_REPLAY_PROBE window from-to, got {win}"))?;
            let v: Vec<([i32; 3], String)> = cells
                .split(';')
                .filter(|e| !e.trim().is_empty())
                .map(|e| -> Result<([i32; 3], String)> {
                    let (c, p) = e.split_once('|').ok_or_else(|| anyhow::anyhow!("MCTAS_REPLAY_PROBE entry x,y,z|PRED, got {e}"))?;
                    Ok((cell3(c)?, p.trim().to_string()))
                })
                .collect::<Result<_>>()?;
            (v, Some((a.trim().parse()?, b.trim().parse()?)))
        }
        Err(_) => (Vec::new(), None),
    };
    let mut probe_last: Vec<Option<bool>> = vec![None; probes.len()];
    let mut probe_yes: Vec<u32> = vec![0; probes.len()];
    let mut probe_rows: Vec<String> = Vec::new();
    let mut block_asserts: Vec<([i32; 3], String)> = match std::env::var("MCTAS_REPLAY_ASSERT") {
        Ok(s) => s
            .split(';')
            .filter(|e| !e.trim().is_empty())
            .map(|e| -> Result<([i32; 3], String)> {
                let (c, p) = e.split_once('|').ok_or_else(|| anyhow::anyhow!("MCTAS_REPLAY_ASSERT=x,y,z|PREDICATE;…, got {e}"))?;
                Ok((cell3(c)?, p.trim().to_string()))
            })
            .collect::<Result<_>>()?,
        Err(_) => Vec::new(),
    };
    // (ironspec 13:43 (a)) the record's HELD Ts (`held-ts: x,y,z=SHAPE[ (note)]; …`): a T held in one shape
    // must sit in THAT shape at t0 and after the last tick (the switched-T check accepts any legal shape)
    if let Some(v) = d.meta.get("held-ts") {
        for entry in v.split(';').filter(|e| !e.trim().is_empty()) {
            let (c, rest) = entry.trim().split_once('=').ok_or_else(|| anyhow::anyhow!("held-ts: x,y,z=SHAPE, got {entry}"))?;
            let shape = rest.split_whitespace().next().unwrap_or("");
            anyhow::ensure!(!shape.is_empty() && shape.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_'), "held-ts: {entry}: no shape");
            block_asserts.push((cell3(c)?, format!("minecraft:rail[shape={shape}]")));
        }
        println!("  the record's held Ts: {} (each asserted at its held shape at t0 and after the last tick)", v.trim());
    }
    let mut unread: Vec<String> = Vec::new();
    let mut day_base: i64 = day_start;
    // MCTAS_REPLAY_NAMES="7=28,8-11=7-10,12-28=11-27": scene index -> the drive's eN, where the drive
    // numbers its entities in another order (its spawn order)
    let mut names: BTreeMap<usize, usize> = BTreeMap::new();
    if let Ok(spec) = std::env::var("MCTAS_REPLAY_NAMES") {
        for part in spec.split(',').filter(|p| !p.trim().is_empty()) {
            let (a, b) = part.split_once('=').ok_or_else(|| anyhow::anyhow!("MCTAS_REPLAY_NAMES: a=b or a-b=c-d, got {part}"))?;
            let range = |s: &str| -> Result<(usize, usize)> {
                Ok(match s.split_once('-') {
                    Some((x, y)) => (x.trim().parse()?, y.trim().parse()?),
                    None => (s.trim().parse()?, s.trim().parse()?),
                })
            };
            let ((a0, a1), (b0, b1)) = (range(a)?, range(b)?);
            anyhow::ensure!(a1 >= a0 && a1 - a0 == b1.wrapping_sub(b0), "MCTAS_REPLAY_NAMES: {part}: ranges of different lengths");
            for k in 0..=a1 - a0 {
                names.insert(a0 + k, b0 + k);
            }
        }
        println!("  names: scene index -> the drive's eN: {}", names.iter().map(|(a, b)| format!("{a}->{b}")).collect::<Vec<_>>().join(" "));
    }
    for t in 0..=ticks {
        // (ironspec 07:05 / 08:02: the lift-top fault rows) MCTAS_REPLAY_SWAP="x,y,z;STATE;t0;t1": the cell
        // set to STATE before tick t0 and its own state restored before t1 (each write must change it);
        // MCTAS_REPLAY_HOLDCART="x,y,z;t0;t1": an EMPTY minecart summoned on the cell before t0 and held
        // there (position and zero motion re-set before every tick) until t1, then removed
        for (k, (c, st, a, b)) in swaps.iter().enumerate() {
            let cmd_at = |s: &str| format!("setblock {} {} {} {s}", c[0], c[1] + DY, c[2]);
            if t == *a {
                // (G1 on T) after a world open the old state can't be read (the fresh server's snapshot
                // markers), so the restore could never complete: refuse the swap at its start, by name
                anyhow::ensure!(
                    !reloaded,
                    "MCTAS_REPLAY_SWAP ({},{},{}) starts at t{t}, after a world open: its old state can't be read there, so its restore could never run; start it before the open",
                    c[0],
                    c[1],
                    c[2]
                );
                swap_old[k] = Some(cell_state(&mut srv, *c)?);
                let out = srv.cmd(&cmd_at(st))?;
                anyhow::ensure!(out.contains("Changed the block"), "MCTAS_REPLAY_SWAP at t{t}: {}", out.trim());
                events.push(format!("t{t}: SWAP ({},{},{}) {} -> {st}", c[0], c[1], c[2], swap_old[k].clone().unwrap_or_default()));
            } else if t == *b {
                let old = swap_old[k].clone().ok_or_else(|| anyhow::anyhow!("MCTAS_REPLAY_SWAP: no state saved"))?;
                let out = srv.cmd(&cmd_at(&old))?;
                anyhow::ensure!(out.contains("Changed the block"), "MCTAS_REPLAY_SWAP restore at t{t}: {}", out.trim());
                events.push(format!("t{t}: SWAP ({},{},{}) restored to {old}", c[0], c[1], c[2]));
            }
        }
        // (ironspec 11:03 (2)(c): the forced-configuration row) MCTAS_REPLAY_INJECT="t;x,y,z;vx,vy,vz[;rider]"
        // (several separated by '/'): a minecart summoned before tick t at that frame position with that
        // motion, ridden by a fresh villager unless `rider` is `empty`; tagged inject<k> / inject<k>p0
        // (G1 12:10 (2)(a): the leader's rider calm) MCTAS_REPLAY_KILL="t;SELECTOR[/…]": the entities
        // matching SELECTOR removed before tick t (e.g. the scene's pillager scarer: no hostile in view)
        for (kt, sel) in &kills {
            if t == *kt {
                let out = srv.cmd(&format!("kill {sel}"))?;
                anyhow::ensure!(out.contains("Killed"), "MCTAS_REPLAY_KILL at t{t} ({sel}): {}", out.trim());
                events.push(format!("t{t}: KILL {sel}: {}", out.trim()));
            }
        }
        for (k, (it, ip, iv, ridden)) in injects.iter().enumerate() {
            if t == *it {
                let pass = if *ridden { format!(",Passengers:[{{id:\"minecraft:villager\",Tags:[\"inject{k}p0\"]}}]") } else { String::new() };
                let out = srv.cmd(&format!(
                    "summon minecraft:minecart {} {} {} {{Tags:[\"inject{k}\"],Motion:[{:?}d,{:?}d,{:?}d]{pass}}}",
                    ip[0],
                    ip[1] + DY as f64,
                    ip[2],
                    iv[0],
                    iv[1],
                    iv[2]
                ))?;
                anyhow::ensure!(out.contains("Summoned new"), "MCTAS_REPLAY_INJECT at t{t}: {}", out.trim());
                let u = uuid_hex(&mut srv, &format!("@e[tag=inject{k},limit=1]"))?;
                uuid_names.push((u, format!("inject{k}")));
                if *ridden {
                    let ru = uuid_hex(&mut srv, &format!("@e[tag=inject{k}p0,limit=1]"))?;
                    uuid_names.push((ru, format!("inject{k}p0")));
                }
                // (G1 on O #1) the stored motion and the per-tick move it gives: a ridden cart moves 0.75 x its
                // delta per tick on a rail (OldMinecartBehavior 199), an empty one 1.0 x
                let scale = if *ridden { 0.75 } else { 1.0 };
                events.push(format!(
                    "t{t}: INJECT inject{k} at ({:.4},{:.4},{:.4}) STORED motion ({:?},{:?},{:?}) = a per-tick move of ({:?},{:?},{:?}) ({scale} x){}",
                    ip[0],
                    ip[1],
                    ip[2],
                    iv[0],
                    iv[1],
                    iv[2],
                    iv[0] * scale,
                    iv[1] * scale,
                    iv[2] * scale,
                    if *ridden { ", ridden" } else { ", empty" }
                ));
            }
        }
        if let Some((c, a, b)) = hold.as_ref() {
            let (x, y, z) = (c[0] as f64 + 0.5, (c[1] + DY) as f64 + 0.0625, c[2] as f64 + 0.5);
            if t == *a {
                let out = srv.cmd(&format!("summon minecraft:minecart {x} {y} {z} {{Tags:[\"holdcart\"],Motion:[0.0d,0.0d,0.0d]}}"))?;
                anyhow::ensure!(out.contains("Summoned new"), "MCTAS_REPLAY_HOLDCART at t{t}: {}", out.trim());
                events.push(format!("t{t}: HOLDCART an empty minecart held at ({},{},{})", c[0], c[1], c[2]));
            } else if t > *a && t < *b {
                srv.cmd_ok(&format!("tp @e[tag=holdcart,limit=1] {x} {y} {z}"))?;
                srv.cmd_ok("data merge entity @e[tag=holdcart,limit=1] {Motion:[0.0d,0.0d,0.0d]}")?;
            } else if t == *b {
                srv.cmd_ok("kill @e[tag=holdcart]")?;
                events.push(format!("t{t}: HOLDCART removed"));
            }
        }
        if loads.contains(&t) {
            reloaded = true;
            let (s, rl) = crate::restartcheck::reload(srv, |w| opts(Some(w)), None)?;
            srv = s;
            log.at = 0;
            srv.cmd_ok(&forceload)?;
            srv.cmd_ok("gamerule advance_time true")?;
            srv.cmd_ok("weather clear 1000000")?;
            // (G1 on G #2) the clock re-based from the game after the load: `time query time` is the
            // overworld clock (26.3: "Clock minecraft:overworld is at N tick(s)"); the tick about to
            // run reads N + 1
            let clock = srv.cmd_long("time query time")?;
            let n: i64 = clock.split(" is at ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|s| s.parse().ok()).ok_or_else(|| anyhow::anyhow!("time query time: unreadable: {}", clock.trim()))?;
            day_base = n - t as i64;
            events.push(format!("t{t}: WORLD OPEN ({}); the clock {n} (day {})", rl.describe(), n.rem_euclid(24000)));
            detail_until = Some(t + 60);
            // a world open wipes every codec-less memory: the tracked activity, hostile, stamp, path
            for v in vs.iter_mut() {
                v.stamp = None;
                v.hostile = false;
                v.home_path = None;
            }
        }
        while ai < acts.len() && acts[ai].t == t {
            let a = &acts[ai];
            match a.verb.as_str() {
                "home" | "last-slept" => {
                    let who = a.args.first().ok_or_else(|| anyhow::anyhow!("t{t}: {} with no entity", a.verb))?.replace('.', "");
                    // (G1 on G #1, ironspec 08:21) each memory act read back: the game holds what the
                    // drive wrote
                    if a.verb == "home" {
                        anyhow::ensure!(a.args.len() == 4, "t{t}: home eN.pK x y z");
                        let p: Vec<i32> = a.args[1..].iter().map(|s| s.parse()).collect::<Result<_, _>>()?;
                        srv.cmd_ok(&format!("data merge entity @e[tag={who},limit=1] {{Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}", p[0], p[1] + DY, p[2]))?;
                        let back = crate::strict::data_value(&mut srv, &format!("data get entity @e[tag={who},limit=1] Brain.memories.\"minecraft:home\".value.pos"))?;
                        let want = format!("[I; {}, {}, {}]", p[0], p[1] + DY, p[2]);
                        anyhow::ensure!(back.as_deref() == Some(want.as_str()), "t{t}: {who}'s HOME read back {back:?}, the act wrote {want}");
                        home_set_t0 += usize::from(t == 0);
                        if t == 0 {
                            declared_home.insert(who.clone());
                        }
                        if let Some(v) = vs.iter_mut().find(|v| v.tag == who) {
                            v.has_home = true;
                        }
                    } else {
                        anyhow::ensure!(a.args.len() == 2, "t{t}: last-slept eN.pK d");
                        let dt: i64 = a.args[1].parse()?;
                        let v = gt0 + t as i64 + dt;
                        srv.cmd_ok(&format!("data merge entity @e[tag={who},limit=1] {{Brain:{{memories:{{\"minecraft:last_slept\":{{value:{v}L}}}}}}}}"))?;
                        let back = crate::strict::data_value(&mut srv, &format!("data get entity @e[tag={who},limit=1] Brain.memories.\"minecraft:last_slept\".value"))?;
                        anyhow::ensure!(back.as_deref() == Some(format!("{v}L").as_str()), "t{t}: {who}'s LAST_SLEPT read back {back:?}, the act wrote {v}");
                        slept_set_t0 += usize::from(t == 0);
                    }
                }
                "set" => {
                    anyhow::ensure!(a.args.len() == 4, "t{t}: set x y z STATE");
                    let p: Vec<i32> = a.args[..3].iter().map(|s| s.parse()).collect::<Result<_, _>>()?;
                    let cell = [p[0], p[1], p[2]];
                    // (G1's divergence check) redsim's recorded state before the write against the
                    // game's: carts in the act's column, each hold and pocket
                    if let Some(exp) = &a.expect {
                        if state_at.as_ref().is_none_or(|(st, _)| *st != t) {
                            state_at = Some((t, cart_states(&mut srv, &cart_tags)?));
                        }
                        let game = expect_form(&state_at.as_ref().expect("set above").1, cell, &names);
                        let mut want = exp.replace(".p", "p");
                        // (G1's control) MCTAS_REPLAY_PLANT_EXPECT=t: the first expected state at or
                        // after t is planted wrong and must be counted as a divergence
                        if plant_at.is_some_and(|p| t >= p) && planted.is_none() {
                            want = format!("{want} (planted)");
                            planted = Some(t);
                        }
                        expects_checked += 1;
                        // (G1) the divergence's size: when the game's holds and pockets first reach
                        // the state redsim had at each diverged write
                        let game_hp = game.split_once("; ").map(|(_, r)| r.to_string()).unwrap_or_default();
                        for p in pending.iter_mut() {
                            // (a state matched within 100 gt; later the lap repeats the same names)
                            if p.2.is_none() && t <= p.0 + 100 && p.1 == game_hp {
                                p.2 = Some(t);
                            }
                        }
                        if game != want {
                            first_div.get_or_insert(t);
                            divergences.push(format!("t{t} ({},{},{}): redsim {want} | game {game}", cell[0], cell[1], cell[2]));
                            let want_hp = want.split_once("; ").map(|(_, r)| r.to_string()).unwrap_or_default();
                            if want_hp != game_hp && pending.len() < 400 {
                                pending.push((t, want_hp, None));
                            }
                        }
                    }
                    // (the harness's snapshot toggles a save marker per chunk, tracked per server
                    // instance: after a world open a new instance can't read it; not read then)
                    let before = if first_seen.insert(cell) { Some(if reloaded { "(not read after a world open)".to_string() } else { cell_state(&mut srv, cell)? }) } else { None };
                    let out = srv.cmd(&format!("setblock {} {} {} {}", p[0], p[1] + DY, p[2], a.args[3]))?;
                    if !out.contains("Changed the block") {
                        // (G1) a no-op act is a DIVERGENCE signal: the game's cell already held the
                        // act's state, so (as redsim's write changed it) the two worlds differ here
                        noop.push(format!("t{t} DIVERGENCE at ({},{},{}): the game's cell already held {} before the act ({})", p[0], p[1], p[2], a.args[3], out.trim()));
                    }
                    if let Some(b) = before {
                        println!("  first act at ({},{},{}), t{t}: {b} -> {}", p[0], p[1], p[2], a.args[3]);
                    }
                }
                _ => unreachable!(),
            }
            applied += 1;
            ai += 1;
        }
        // (ironspec 08:21 / 08:23) the t0 state, before the first tick: every villager's HOME and
        // LAST_SLEPT as the drive's t0 acts wrote them (each read back), the bed's ticket free (no
        // ticket taken during the setup)
        if t == 0 {
            let taken = setup_lines.iter().filter(|l| l.starts_with("poi acquire")).count();
            // (ironspec 09:48) MCTAS_REPLAY_ASSERT="x,y,z|PREDICATE;…": block predicates asserted at t0
            // (and again after the last tick), e.g. the pocket-exit Ts' placement-order shapes
            for (p, legal) in &switched_cells {
                let st = cell_state(&mut srv, *p)?;
                let shape = st.split("shape=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("");
                let ok = legal.iter().any(|l| l == shape);
                println!("  t0 switched T ({},{},{}): {st} ({} of its legal shapes {legal:?})", p[0], p[1], p[2], if ok { "one" } else { "NONE" });
                anyhow::ensure!(ok, "t0: the switched T ({},{},{}) is {st}, none of its legal shapes {legal:?}", p[0], p[1], p[2]);
            }
            for (c, pred) in block_asserts.iter().chain(t_asserts.iter()) {
                let ok = crate::strict::test(&mut srv, &format!("execute if block {} {} {} {pred}", c[0], c[1] + DY, c[2]))?;
                println!("  t0 assert ({},{},{}) {pred}: {}", c[0], c[1], c[2], if ok { "HELD" } else { "FAILED" });
                anyhow::ensure!(ok, "t0: ({},{},{}) is not {pred}", c[0], c[1], c[2]);
            }
            println!(
                "  t0: the run's start day {day_start} (scene {} md5 {}, drive {} md5 {}{}); HOME written and read back for {home_set_t0} villager(s), LAST_SLEPT for {slept_set_t0}, of {}; tickets taken during the setup: {taken} (start state (i): the ticket free){}",
                scene.display(),
                crate::md5::md5_hex(&std::fs::read(scene)?),
                drive.display(),
                crate::md5::md5_hex(&std::fs::read(drive)?),
                std::env::var("MCTAS_REPLAY_MODULE_MD5").map(|m| format!(", built from module md5 {m}")).unwrap_or_default(),
                vs.len(),
                if home_set_t0 == vs.len() { "" } else { " (NOT every villager holds HOME)" }
            );
            anyhow::ensure!(taken == 0, "t0: {taken} ticket(s) taken during the setup: not start state (i)");
            // (ironspec 09:42 (R3)) the drive DECLARES its HOME holders (its t0 `home` acts): each read
            // back above, and every other villager HOME-less at t0
            for v in vs.iter() {
                if !declared_home.contains(&v.tag) {
                    let h = crate::strict::data_value(&mut srv, &format!("data get entity @e[tag={},limit=1] Brain.memories.\"minecraft:home\"", v.tag))?;
                    anyhow::ensure!(h.is_none(), "t0: {} holds HOME {h:?} but the drive declares it HOME-less", v.tag);
                }
            }
            println!("  t0: the drive's declared HOME holders: {} of {} ({}); every other villager HOME-less, read back", declared_home.len(), vs.len(), if declared_home.len() == vs.len() { "all".to_string() } else { declared_home.iter().cloned().collect::<Vec<_>>().join(" ") });
        }
        srv.step(1)?;
        let now = gt0 + t as i64 + 1;
        let day = (day_base + t as i64 + 1).rem_euclid(24000);
        for l in log.drain(&srv)? {
            // (G1 on G #6) a hook that couldn't read its values fails the run, never reads as "no event"
            if l.contains("(unread)") {
                unread.push(format!("t{t}: {l}"));
                continue;
            }
            if l.starts_with("move cart req ") {
                if move_window.is_some_and(|(a, b)| (a..=b).contains(&t)) {
                    // (G1 10:04, ironspec 10:06: the drop-entry row) MCTAS_REPLAY_MOVE_BOX limits the
                    // rows to carts whose box meets that frame box; MCTAS_REPLAY_MOVE_CLIPPED keeps only
                    // moves that collided (a flag set) or got less than they asked
                    let mv = parse_move(&l);
                    let keep_box = match (&move_box, &mv) {
                        (Some(r), Some(m)) => {
                            let dy = DY as f64;
                            m.bmin[0] < r[3] && m.bmax[0] > r[0] && (m.bmin[1] - dy) < r[4] && (m.bmax[1] - dy) > r[1] && m.bmin[2] < r[5] && m.bmax[2] > r[2]
                        }
                        (Some(_), None) => true,
                        (None, _) => true,
                    };
                    // (G1 on M #2) every tolerance is Mth.equal's (abs(b - a) < 1.0E-5F, Mth 162-164: the game's
                    // own test for a clipped axis in Entity.move): a collision snaps a box face exactly onto
                    // the blocking surface, so the face and the floor tests use the same bound
                    let clipped = mv.as_ref().is_none_or(|m| m.hcoll || m.vcoll || (0..3).any(|i| (m.req[i] - m.got[i]).abs() >= MTH_EQUAL));
                    if keep_box && (!move_clipped_only || clipped) {
                        let mut s = l.clone();
                        for (u, name) in &uuid_names {
                            s = s.replace(u.as_str(), name);
                        }
                        if let Some(m) = &mv {
                            // the drop's two failure forms (ironfarm 10:04), at each lane's drop (lanes
                            // x0 = -1, 3, 7): a move clipped into C's support (x0,23,7) at its north face,
                            // and an off-rail rest at the lip
                            let dy = DY as f64;
                            let cx = (m.bmin[0] + m.bmax[0]) / 2.0;
                            let cz = (m.bmin[2] + m.bmax[2]) / 2.0;
                            for x0 in [-1.0f64 + drop_shift.0, 3.0 + drop_shift.0, 7.0 + drop_shift.0] {
                                let zc = 7.0 + drop_shift.1;
                                let into_c = m.hcoll && m.req[2] > 0.0 && m.bmin[0] < x0 + 1.0 && m.bmax[0] > x0 && (m.bmin[1] - dy) < 24.0 && (m.bmax[1] - dy) > 23.0 && (m.bmax[2] - zc).abs() < MTH_EQUAL;
                                let lip_rest = m.got.iter().all(|g| g.abs() < MTH_EQUAL) && m.req[1].abs() >= MTH_EQUAL && (m.bmin[1] - dy - 24.0).abs() < MTH_EQUAL && (x0..x0 + 1.5).contains(&cx) && (zc - 1.0..zc + 0.5).contains(&cz);
                                if into_c {
                                    drop_clips += 1;
                                    s.push_str(&format!(" [CLIP INTO lane {x0}'s C support ({x0},23,7)]"));
                                }
                                if lip_rest {
                                    lip_rests += 1;
                                    s.push_str(&format!(" [AT REST at lane {x0}'s drop lip]"));
                                }
                            }
                        }
                        move_rows.push(format!("t{t}: {s}"));
                    }
                }
                continue;
            }
            if l.starts_with("push cart by ") || l.starts_with("pushlist ") {
                if push_window.is_some_and(|(a, b)| (a..=b).contains(&t)) {
                    let mut s = l.clone();
                    // (ironspec 10:09) per push: the pusher's activity and walk target
                    if let Some(pu) = l.strip_prefix("push cart by Villager ").and_then(|r| r.split_whitespace().next())
                        && let Some(&pi) = by_uuid.get(pu)
                    {
                        let p = &vs[pi];
                        s.push_str(&format!(
                            " [pusher activity {}, walk target by {}, nearest hostile {}, last hurt {}]",
                            if p.activity.is_empty() { "?" } else { p.activity.as_str() },
                            p.wt.as_deref().unwrap_or("none"),
                            if p.hostile { p.hostile_what.as_str() } else { "none" },
                            p.last_hurt.as_deref().unwrap_or("none")
                        ));
                    }
                    for (u, name) in &uuid_names {
                        s = s.replace(u.as_str(), name);
                    }
                    push_rows.push(format!("t{t}: {s}"));
                }
                continue;
            }
            let Some(&vi) = l.rsplit(" @").next().and_then(|u| by_uuid.get(u.trim())) else {
                if l.starts_with("poi ") {
                    events.push(format!("t{t}: (no named villager's brain) {l}"));
                }
                continue;
            };
            let tag = vs[vi].tag.clone();
            let body = l.split(" @").next().unwrap_or(&l).to_string();
            if detail_until.is_some_and(|u| t <= u) {
                detail.push(format!("t{t}: {tag} {body}"));
            }
            if let Some(rest) = l.strip_prefix("activity ") {
                // Activity's toString is its bare name ("rest", "panic", "idle")
                let act = rest.split_whitespace().next().unwrap_or("").trim_start_matches("minecraft:").to_string();
                // a REST spell closes when the activity leaves REST
                if vs[vi].activity == "rest" && act != "rest" {
                    if let Some(sp) = vs[vi].spell.take() {
                        let d = sp.describe(Some(t));
                        vs[vi].spells.push(d);
                    }
                }
                if act == "rest" {
                    let at = pos_of(&mut srv, &format!("@e[tag={tag},limit=1]"))?;
                    let v = &mut vs[vi];
                    let age = v.stamp.map(|s| (now - s).to_string()).unwrap_or_else(|| "none".into());
                    v.rest_entries.push(format!("t{t} at {} hostile {} stamp age {age}", fmt_pos(at), v.hostile));
                    v.spell = Some(Spell { start: t, at: fmt_pos(at), hostile: v.hostile, wt_at_entry: v.wt.clone(), ..Default::default() });
                }
                events.push(format!("t{t}: {tag} activity {act}"));
                vs[vi].activity = act;
            } else if l.starts_with("set minecraft:walk_target") {
                let by = body.split(" by ").nth(1).unwrap_or("?").to_string();
                let v = &mut vs[vi];
                v.wt = Some(by.clone());
                if by.starts_with("SetWalkTargetFromBlockMemory") {
                    // (ironspec 09:42 (R2)) EVERY run of SetWalkTargetFromBlockMemory is judged on the
                    // age it saw, not only a spell's first (a later run after a stale sink stops)
                    let age = v.stamp.map(|s| now - s);
                    v.entry_judged.push((t, age));
                    if let Some(sp) = v.spell.as_mut()
                        && sp.home_set.is_none()
                        && sp.released.is_none()
                    {
                        sp.home_set = Some((t, age.map(|a| a.to_string()).unwrap_or_else(|| "none".into())));
                    }
                }
            } else if l.starts_with("erase minecraft:walk_target") {
                let by = body.split(" by ").nth(1).unwrap_or("?").to_string();
                let v = &mut vs[vi];
                v.wt = None;
                v.home_path = None;
                if by.starts_with("MoveToTargetSink.stop")
                    && let Some(sp) = v.spell.as_mut()
                    && sp.sink_stop.is_none()
                {
                    sp.sink_stop = Some(t);
                }
            } else if l.starts_with("set minecraft:nearest_hostile") {
                vs[vi].hostile = true;
                vs[vi].hostile_what = body.split(" = ").nth(1).unwrap_or(&body).split(" by ").next().unwrap_or("").to_string();
            } else if l.starts_with("erase minecraft:nearest_hostile") {
                vs[vi].hostile = false;
                vs[vi].hostile_what.clear();
            } else if l.starts_with("set minecraft:path = path to") {
                if l.contains(&home_target) {
                    let complete = l.contains("reach true");
                    let start = l.split(" start ").nth(1).and_then(|s| s.split(" end").next()).unwrap_or("?").to_string();
                    if !complete && vs[vi].activity == "rest" {
                        events.push(format!("t{t}: {tag} PARTIAL HOME path in REST, start {start}"));
                    }
                    if let Some(sp) = vs[vi].spell.as_mut() {
                        sp.home_path = Some((complete, start.clone()));
                    }
                    vs[vi].home_path = Some((complete, start));
                } else {
                    // another path replaces the HOME one
                    vs[vi].home_path = None;
                }
            } else if l.starts_with("erase minecraft:path") {
                // (G1 on G #2) a finished, stopped or erased path is no longer a complete HOME path
                vs[vi].home_path = None;
            } else if l.contains("minecraft:cant_reach_walk_target_since") {
                if l.starts_with("set ") {
                    let s: Option<i64> = l.split(" = ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok());
                    anyhow::ensure!(s.is_some(), "t{t}: unparsable stamp line: {l}");
                    let at = pos_of(&mut srv, &format!("@e[tag={tag},limit=1]"))?;
                    let v = &mut vs[vi];
                    v.stamp = s;
                    let by = body.split(" by ").nth(1).unwrap_or("?").to_string();
                    if by.starts_with("MoveToTargetSink")
                        && let Some(sp) = v.spell.as_mut()
                        && sp.armed.is_none()
                    {
                        sp.armed = Some(t);
                    }
                    v.stamps_armed.push(format!("t{t} day {day} {} at {} by {by}", v.activity.trim_start_matches("minecraft:"), fmt_pos(at)));
                } else if l.starts_with("erase ") && l.contains("(had it)") {
                    vs[vi].stamp = None;
                }
            } else if l.starts_with("erase minecraft:home") {
                let at = pos_of(&mut srv, &format!("@e[tag={tag},limit=1]"))?;
                let by = body.split(" by ").nth(1).unwrap_or("?").to_string();
                // the bed's state and who sleeps in it at the erase (ValidateNearbyPoi's case)
                let occ = crate::strict::test(&mut srv, &format!("execute if block {} {} {} #minecraft:beds[occupied=true]", bed[0], bed[1] + DY, bed[2]))?;
                let mut sleepers = Vec::new();
                for w in vs.iter() {
                    if w.gone.is_none() && srv.count_entities(&format!("@e[tag={}]", w.tag))? > 0 && crate::strict::has_data(&mut srv, &format!("data get entity @e[tag={},limit=1] sleeping_pos", w.tag))? {
                        sleepers.push(w.tag.clone());
                    }
                }
                let dist = at.map(|p| ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2) + (p[2] - centre[2]).powi(2)).sqrt());
                let what = format!("t{t} by {by} at {} (distance to the bed's centre {}; the bed occupied {occ}; asleep: {})", fmt_pos(at), dist.map(|d| format!("{d:.2}")).unwrap_or_else(|| "?".into()), if sleepers.is_empty() { "nobody".to_string() } else { sleepers.join(" ") });
                // a release by SetWalkTargetFromBlockMemory is that spell's first run of it
                if by.starts_with("SetWalkTargetFromBlockMemory") {
                    let v = &mut vs[vi];
                    let age = v.stamp.map(|s| now - s);
                    // (R2) a release is a run of the behaviour too: judged on the age it saw (the
                    // erase-HOME line precedes its stamp set)
                    v.entry_judged.push((t, age));
                    v.releases += 1;
                    if let Some(sp) = v.spell.as_mut()
                        && sp.released.is_none()
                    {
                        sp.released = Some((t, age.map(|a| a.to_string()).unwrap_or_else(|| "none".into())));
                    }
                }
                vs[vi].has_home = false;
                vs[vi].home_lost.push(what.clone());
                events.push(format!("t{t}: {tag} HOME ERASED {}", what.trim_start_matches(&format!("t{t} "))));
            } else if l.starts_with("set minecraft:home") {
                let by = body.split(" by ").nth(1).unwrap_or("?").to_string();
                vs[vi].has_home = true;
                if !by.contains("makeBrain") && !by.contains("refreshBrain") {
                    vs[vi].home_set.push(format!("t{t} by {by}"));
                    events.push(format!("t{t}: {tag} HOME set by {by}"));
                }
            } else if l.starts_with("set minecraft:last_slept") {
                // SleepInBed.start's LAST_SLEPT (a lie-down): the FARM-STOP bar's evidence
                let v = &mut vs[vi];
                v.slept.push(t);
                if let Some((s, _)) = v.at_stop.take() {
                    v.stop_done = true;
                    events.push(format!("t{t}: {tag} LIE-DOWN (at the stop since t{s}, least distance {:.3})", v.stop_least));
                    stop_visits.push(format!("{tag} t{s}: least {:.3}, lay down at t{t} (+{} gt)", v.stop_least, t - s));
                }
            } else if l.starts_with("poi ") {
                events.push(format!("t{t}: {tag} {body}"));
            } else if body.starts_with("hurt ") {
                // (ironspec 06:22 platform) every damage a villager takes, drowning above all
                vs[vi].hurts.push(format!("t{t}: {body}"));
                vs[vi].last_hurt = Some(format!("t{t}: {body}"));
                events.push(format!("t{t}: {tag} {body}"));
            } else if let Some(s) = body.strip_prefix("sight at rest ") {
                // (ironspec 06:50 real) the REST entrant's Monsters within 16: distance, its own
                // hasLineOfSight, the clip's blocking cell
                let what = s.split_once("): ").map(|(_, r)| r).unwrap_or(s).to_string();
                if let Some(last) = vs[vi].rest_entries.last_mut() {
                    last.push_str(&format!("; sight: {what}"));
                }
                if let Some(sp) = vs[vi].spell.as_mut()
                    && sp.sight.is_empty()
                {
                    sp.sight = what.clone();
                }
                events.push(format!("t{t}: {tag} sight at its REST entry: {what}"));
            }
        }
        // (G1 on G #9, on H #2) the brains' activity at t1 asserted: by day the run's schedule activity
        // (IDLE, or PANIC on a hostile); by night REST, or PANIC with a hostile sensed
        if t == 0 && !noai {
            // (G1 on I) night as the schedule's keyframes give it (Timelines 186-198: REST from 12,000
            // to the IDLE keyframe at 10 of the next day), the day time taken mod 24,000
            let d0 = day.rem_euclid(24_000);
            let night0 = d0 >= 12_000 || d0 < 10;
            // (ironspec 09:28 (d)) within 20 gt of the REST / IDLE keyframes (12,000 and 10) the
            // brain's > 20-gt update cadence allows either side's activity
            let near_edge = (d0 - 12_000).abs() < 20 || d0 < 30 || d0 > 23_990;
            let wrong: Vec<String> = vs
                .iter()
                .filter(|v| v.gone.is_none())
                .filter(|v| match v.activity.as_str() {
                    "idle" => night0 && !near_edge,
                    "rest" => !night0 && !near_edge,
                    "panic" => !v.hostile && night0,
                    _ => true,
                })
                .map(|v| format!("{} {}", v.tag, if v.activity.is_empty() { "(no activity line)" } else { v.activity.as_str() }))
                .collect();
            println!(
                "  the brains at t1 (day {day}, {}): {}",
                if night0 { "night: REST, or PANIC with a hostile sensed" } else { "day: IDLE or PANIC" },
                if wrong.is_empty() { "every villager on the schedule".to_string() } else { format!("NOT on the schedule: {}", wrong.join(", ")) }
            );
            anyhow::ensure!(wrong.is_empty(), "at t1 {} villager(s) not on the run's schedule activity: {}", wrong.len(), wrong.join(", "));
        }
        // (G1's air-model fidelity row) MCTAS_REPLAY_AIR=<tag>: that villager's air supply every tick
        // while it is inside the main lift's box (x -6..-3, y 0..25, z 5..9): its position, eye
        // height, whether its eye's block is a bubble column or other water, its vehicle
        if let Some(who) = air_watch.as_deref()
            && vs.iter().any(|v| v.tag == who && v.gone.is_none())
        {
            let at = pos_of(&mut srv, &format!("@e[tag={who},limit=1]"))?;
            if let Some(p) = at
                && (-6.0..-3.0).contains(&p[0])
                && (0.0..25.0).contains(&p[1])
                && (5.0..9.0).contains(&p[2])
            {
                let air = srv.cmd_long(&format!("data get entity @e[tag={who},limit=1] Air"))?;
                let a = air.split("data: ").nth(1).map(|s| s.trim().to_string()).unwrap_or_else(|| air.trim().to_string());
                // the eye's cell: a marker summoned at the eyes, read back, then tested by absolute
                // block coordinates (a forked `execute as … if block` prints no test reply)
                srv.cmd(&format!("execute as @e[tag={who},limit=1] at @s anchored eyes positioned ^ ^ ^ run summon minecraft:marker ~ ~ ~ {{Tags:[\"airm\"]}}"))?;
                let eye = pos_of(&mut srv, "@e[type=minecraft:marker,tag=airm,limit=1]")?;
                srv.cmd("kill @e[type=minecraft:marker,tag=airm]")?;
                let e = eye.ok_or_else(|| anyhow::anyhow!("t{t}: {who}'s eye marker unreadable"))?;
                let (bx, by, bz) = (e[0].floor() as i32, e[1].floor() as i32 + DY, e[2].floor() as i32);
                let bubble = crate::strict::test(&mut srv, &format!("execute if block {bx} {by} {bz} minecraft:bubble_column"))?;
                let water = crate::strict::test(&mut srv, &format!("execute if block {bx} {by} {bz} minecraft:water"))?;
                let riding = crate::strict::test_on(&mut srv, &format!("execute as @e[tag={who},limit=1] on vehicle if entity @s"))?;
                air_rows.push(format!(
                    "t{t}: air {a} at {} eye y {:.3} (cell {bx},{},{bz}) eye in bubble_column {bubble} / water {water}; seated {riding}",
                    fmt_pos(at),
                    e[1],
                    by - DY
                ));
            }
        }
        // (cc767b78's divergence trace) MCTAS_REPLAY_TRACE=<eN>:<from>-<to>: that cart's position,
        // motion, rider and the rail block under it every tick in the window
        if let Some((whos, from, to)) = trace_spec.as_ref()
            && (*from..=*to).contains(&t)
        {
          for who in whos.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            let p = pos_of(&mut srv, &format!("@e[tag={who},limit=1]"))?;
            let m = srv.cmd_long(&format!("data get entity @e[tag={who},limit=1] Motion"))?;
            let rider = srv.cmd_long(&format!("execute as @e[tag={who},limit=1] on passengers run data get entity @s Tags"))?;
            let mut rail = String::from("?");
            if let Some(q) = p {
                let (bx, by, bz) = (q[0].floor() as i32, q[1].floor() as i32 + DY, q[2].floor() as i32);
                for kind in ["minecraft:powered_rail[powered=true]", "minecraft:powered_rail[powered=false]", "minecraft:detector_rail", "minecraft:activator_rail", "minecraft:rail", "minecraft:water", "minecraft:bubble_column", "minecraft:air"] {
                    if crate::strict::test(&mut srv, &format!("execute if block {bx} {by} {bz} {kind}"))? {
                        rail = format!("{kind} at ({bx},{},{bz})", by - DY);
                        break;
                    }
                }
            }
            trace_rows.push(format!(
                "t{t}: {who} pos {} motion {} rider {} block {rail}",
                p.map(|q| format!("({:.6},{:.6},{:.6})", q[0], q[1], q[2])).unwrap_or_else(|| "gone".into()),
                m.split("data: ").nth(1).unwrap_or("?").trim(),
                rider.split('"').find(|s| s.starts_with('e') && s.contains('p')).unwrap_or("none")
            ));
          }
        }
        // per tick: the REST counters, the stamps' ages, the dawn reads (G1 on G #2: the night from
        // 11,990 in numerator and denominator alike)
        let night = day >= 11990;
        if night {
            night_ticks += 1;
        }
        // (ironspec 06:13 platform (3)) every villager's activity per tick across day 11,990..12,040, and
        // the REST set at every 20-gt schedule tick through the night
        if (11990..=12040).contains(&day) || (night && day % 20 == 0) {
            let rest: Vec<&str> = vs.iter().filter(|v| v.activity == "rest" && v.gone.is_none()).map(|v| v.tag.as_str()).collect();
            if (11990..=12040).contains(&day) {
                activity_rows.push(format!("day {day}: {}", vs.iter().map(|v| format!("{}={}", v.tag, if v.activity.is_empty() { "?" } else { v.activity.as_str() })).collect::<Vec<_>>().join(" ")));
            } else if !rest.is_empty() {
                activity_rows.push(format!("day {day}: in REST {}", rest.join(" ")));
            }
        }
        for v in vs.iter_mut() {
            if let Some(s) = v.stamp {
                let age = now - s;
                if v.oldest.is_none_or(|(a, _)| age > a) {
                    v.oldest = Some((age, t));
                }
            }
            if night && v.activity == "rest" {
                match &v.home_path {
                    Some((true, _)) => v.rest_complete += 1,
                    _ => v.rest_partial += 1,
                }
            }
            if day == 0 {
                v.dawn.push(format!("t{t}: stamp {}", v.stamp.map(|s| format!("age {}", now - s)).unwrap_or_else(|| "none".into())));
            }
        }
        // (ironspec 10:52: the presence string in the scene) MCTAS_REPLAY_PROBE="x,y,z|PREDICATE;…:from-to":
        // each block predicate tested after every tick in the window; its transitions printed
        if let Some((pa, pb)) = probe_window
            && (pa..=pb).contains(&t)
        {
            for (k, (c, pred)) in probes.iter().enumerate() {
                let ok = crate::strict::test(&mut srv, &format!("execute if block {} {} {} {pred}", c[0], c[1] + DY, c[2]))?;
                if probe_last[k] != Some(ok) {
                    probe_rows.push(format!("t{t}: ({},{},{}) {pred}: {}", c[0], c[1], c[2], if ok { "YES" } else { "no" }));
                    probe_last[k] = Some(ok);
                }
                probe_yes[k] += u32::from(ok);
            }
        }
        // (ironspec 09:42 (R1)) FARM-STOP: at night a villager (not gone) that comes within 3.0 of the bed
        // head's centre starts a stop visit; it must hold HOME on arrival and lie down (its LAST_SLEPT
        // set) within the 900-gt follow window, or before it leaves past 4.0. (G1 on L) each visit
        // prints its least distance (so a napper standing at ~2.05, outside SleepInBed's strict 2.0,
        // fails visibly) and its arrival -> lie-down time; villagers within 4.0 are sampled every tick,
        // the rest every 5 gt
        if night {
            for v in vs.iter_mut() {
                if v.gone.is_some() || !(t % 5 == 0 || v.near_stop) {
                    continue;
                }
                let Some(p) = pos_of(&mut srv, &format!("@e[tag={},limit=1]", v.tag))? else { continue };
                let dist = ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2) + (p[2] - centre[2]).powi(2)).sqrt();
                v.near_stop = dist < 4.0;
                // a villager that lay down starts no new visit until it has left past 4.0 (its sleep
                // and the release's wake-up are the same visit)
                if v.stop_done && dist > 4.0 {
                    v.stop_done = false;
                }
                match v.at_stop {
                    None if dist < 3.0 && !v.stop_done => {
                        v.at_stop = Some((t, v.has_home));
                        v.stop_least = dist;
                        if !v.has_home {
                            farm_stop.push(format!("t{t}: {} at the stop ({dist:.2} from the head's centre) WITHOUT HOME", v.tag));
                        }
                    }
                    Some((s, _)) => {
                        v.stop_least = v.stop_least.min(dist);
                        if t > s + 900 {
                            farm_stop.push(format!("t{t}: {} at the stop since t{s} (least distance {:.3}), no lie-down within 900 gt", v.tag, v.stop_least));
                            stop_visits.push(format!("{} t{s}: least {:.3}, NO lie-down within 900 gt", v.tag, v.stop_least));
                            v.at_stop = None;
                            // (ironspec 13:31 on L) the visit is over: it re-arms only after the villager has
                            // left past 4.0, as after a lie-down (no second failure for the same stay)
                            v.stop_done = true;
                        } else if dist > 4.0 {
                            farm_stop.push(format!("t{t}: {} left the stop ({dist:.2}) after t{s} (least distance {:.3}) without lying down", v.tag, v.stop_least));
                            stop_visits.push(format!("{} t{s}: least {:.3}, left at t{t} without lying down", v.tag, v.stop_least));
                            v.at_stop = None;
                        }
                    }
                    _ => {}
                }
            }
        }
        if t % 20 == 0 {
            // (a rider lost) every villager's presence and air supply; a disappearance with the
            // server log's death line
            for v in vs.iter_mut() {
                if v.gone.is_some() {
                    continue;
                }
                let air = srv.cmd_long(&format!("data get entity @e[tag={},limit=1] Air", v.tag))?;
                if air.contains("No entity was found") {
                    let log_text = std::fs::read_to_string(&srv.log)?;
                    let deaths: Vec<&str> = log_text.lines().filter(|l| l.contains("died, message")).collect();
                    let why = deaths.last().map(|l| l.split("]: ").last().unwrap_or(l).to_string()).unwrap_or_else(|| "no death line in the log".into());
                    v.gone = Some(format!("t{t}: {why}"));
                    events.push(format!("t{t}: {} GONE ({why})", v.tag));
                    continue;
                }
                let a: i32 = air.split("data: ").nth(1).and_then(|s| s.trim().trim_end_matches('s').parse().ok()).ok_or_else(|| anyhow::anyhow!("{} Air unreadable: {}", v.tag, air.trim()))?;
                if a < 300 && v.min_air.as_ref().is_none_or(|m| a < m.0) {
                    let at = pos_of(&mut srv, &format!("@e[tag={},limit=1]", v.tag))?;
                    v.min_air = Some((a, t, fmt_pos(at)));
                }
                // (ironspec 06:28 platform: the drowned-rider trace) a rider losing air: where it
                // stands, its cart's position and motion, the lift-top exit booster (-5,23,8), every
                // cart within 3 blocks
                if a < 250 {
                    let at = pos_of(&mut srv, &format!("@e[tag={},limit=1]", v.tag))?;
                    let cart = srv.cmd_long(&format!("execute as @e[tag={},limit=1] on vehicle run data get entity @s Pos", v.tag))?;
                    let motion = srv.cmd_long(&format!("execute as @e[tag={},limit=1] on vehicle run data get entity @s Motion", v.tag))?;
                    let booster = crate::strict::test(&mut srv, &format!("execute if block -5 {} 8 minecraft:powered_rail[powered=true]", 23 + DY))?;
                    let near = srv.cmd_long(&format!("execute as @e[tag={},limit=1] at @s as @e[type=#minecraft:minecart,distance=..3] run data get entity @s Pos", v.tag))?;
                    let carts: Vec<String> = near.split("entity data: ").skip(1).map(|r| r.split(']').next().unwrap_or("").trim_start_matches('[').to_string()).collect();
                    events.push(format!(
                        "t{t}: {} AIR LOW {a} at {}; cart {} motion {}; the exit booster (-5,23,8) powered {booster}; carts within 3: {}",
                        v.tag,
                        fmt_pos(at),
                        cart.split("entity data: ").nth(1).unwrap_or("none").trim(),
                        motion.split("entity data: ").nth(1).unwrap_or("none").trim(),
                        carts.join(" | ")
                    ));
                }
            }
            let mut row = Vec::new();
            for (i, &x) in lanes.iter().enumerate() {
                let n = srv.count_entities(&format!("@e[type=minecraft:minecart,x={x},y={},z=0,dx=0.99,dy=10,dz=8]", 16 + DY))?;
                let v = srv.count_entities(&format!("@e[type=minecraft:minecart,x={x},y={},z=5.5,dx=0.99,dy=4,dz=2.5]", 21.5 + DY as f64))?;
                lane_max[i] = lane_max[i].max(n);
                if v > 0 {
                    v_ticks[i] += 20;
                }
                row.push(format!("{n}{}", if v > 0 { "V" } else { "" }));
            }
            if t % 200 == 0 {
                lane_hist.push(format!("t{t}:{}", row.join("/")));
            }
        }
        // (G1 on G #2) the 16-block exposure sampled every tick for the villagers in REST
        if vs.iter().any(|v| v.activity == "rest" && v.gone.is_none()) {
            let occ = crate::strict::test(&mut srv, &format!("execute if block {} {} {} #minecraft:beds[occupied=true]", bed[0], bed[1] + DY, bed[2]))?;
            if occ {
                for v in vs.iter_mut() {
                    if v.gone.is_some() || v.activity != "rest" {
                        continue;
                    }
                    let Some(p) = pos_of(&mut srv, &format!("@e[tag={},limit=1]", v.tag))? else { continue };
                    let dist = ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2) + (p[2] - centre[2]).powi(2)).sqrt();
                    if v.least_occupied.is_none_or(|(d, _)| dist < d) {
                        v.least_occupied = Some((dist, t));
                    }
                    if dist < 16.0 && v.activity == "rest" && !crate::strict::has_data(&mut srv, &format!("data get entity @e[tag={},limit=1] sleeping_pos", v.tag))? {
                        collisions.push(format!("t{t}: {} in REST at {dist:.2} from the occupied bed", v.tag));
                    }
                }
            }
        }
    }
    // the block predicates again after the last tick (the Ts must still hold their shapes)
    let mut end_asserts_bad: Vec<String> = Vec::new();
    for (p, legal) in &switched_cells {
        // after a world open the fresh server's snapshot markers start over while the world on disk keeps
        // the old ones (the marker setblock then reports no change): read the shape by block tests there
        let st = if reloaded {
            let mut found = None;
            for l in legal {
                if crate::strict::test(&mut srv, &format!("execute if block {} {} {} minecraft:rail[shape={l}]", p[0], p[1] + DY, p[2]))? {
                    found = Some(format!("minecraft:rail[shape={l}] (block test after a world open)"));
                    break;
                }
            }
            found.unwrap_or_else(|| "none of its legal rail shapes (block test after a world open)".to_string())
        } else {
            cell_state(&mut srv, *p)?
        };
        let shape = st.split("shape=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("").to_string();
        let ok = legal.iter().any(|l| *l == shape);
        println!("  end switched T ({},{},{}): {st} ({} of its legal shapes)", p[0], p[1], p[2], if ok { "one" } else { "NONE" });
        if !ok {
            end_asserts_bad.push(format!("switched T ({},{},{}) {st} not in {legal:?}", p[0], p[1], p[2]));
        }
    }
    for (c, pred) in block_asserts.iter().chain(t_asserts.iter()) {
        let ok = crate::strict::test(&mut srv, &format!("execute if block {} {} {} {pred}", c[0], c[1] + DY, c[2]))?;
        println!("  end assert ({},{},{}) {pred}: {}", c[0], c[1], c[2], if ok { "HELD" } else { "FAILED" });
        if !ok {
            end_asserts_bad.push(format!("({},{},{}) {pred}", c[0], c[1], c[2]));
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    anyhow::ensure!(end_asserts_bad.is_empty(), "after the run: {} block assert(s) failed: {}", end_asserts_bad.len(), end_asserts_bad.join("; "));
    println!("  acts applied: {applied} of {}; night ticks (day time 12000..23999) in the run: {night_ticks}", acts.len());
    for e in &events {
        println!("  {e}");
    }
    for d in &detail {
        println!("  after a world open: {d}");
    }
    for a in &activity_rows {
        println!("  activity {a}");
    }
    println!("  lanes (-1 / 3 / 7): the most carts in a lane at once {lane_max:?}; ticks with a cart at the slope top V {v_ticks:?}; every 200 gt (count, V marked): {}", lane_hist.join(" "));
    let (mut partial, mut complete, mut lost, mut old) = (0u32, 0u32, 0usize, 0usize);
    for v in &vs {
        partial += v.rest_partial;
        complete += v.rest_complete;
        lost += v.home_lost.len();
        if v.oldest.is_some_and(|(a, _)| a >= 1000) {
            old += 1;
        }
        println!(
            "  {}: night REST ticks without a complete HOME path {} (with one, a trip: {}); REST entries {:?}; stamps armed {:?}; oldest stamp {}; at dawn {:?}; HOME erased {:?}; HOME set {:?}; least distance to the occupied bed {}",
            v.tag,
            v.rest_partial,
            v.rest_complete,
            v.rest_entries,
            v.stamps_armed,
            v.oldest.map(|(a, t)| format!("{a} gt (t{t})")).unwrap_or_else(|| "none".into()),
            v.dawn,
            v.home_lost,
            v.home_set,
            v.least_occupied.map(|(d, t)| format!("{d:.2} (t{t})")).unwrap_or_else(|| "- (never occupied at a read)".into())
        );
    }
    if let Some((a, b)) = probe_window {
        println!("  block probes t{a}..t{b} (each predicate after every tick; transitions):");
        for (k, (c, pred)) in probes.iter().enumerate() {
            println!("    ({},{},{}) {pred}: held on {} tick(s)", c[0], c[1], c[2], probe_yes[k]);
        }
        for r in &probe_rows {
            println!("    {r}");
        }
    }
    if let Some((a, b)) = move_window {
        println!(
            "  minecart moves t{a}..t{b} (+move: each Entity.move's requested delta, the movement it got, the collision flags after it, the box{}{}): {} line(s); clipped into a lane's C support ((x0,23,7), x0 = -1 / 3 / 7): {drop_clips}; at rest at a drop lip: {lip_rests}",
            move_box.map(|r| format!("; carts meeting the frame box {r:?}")).unwrap_or_default(),
            if move_clipped_only { "; clipped moves only" } else { "" },
            move_rows.len()
        );
        for r in &move_rows {
            println!("    {r}");
        }
    }
    if let Some((a, b)) = push_window {
        println!("  pushes t{a}..t{b} (+push: every AbstractMinecart.push with the cart's delta before -> after and both boxes; every seated villager's pushEntities list; in the game's own order within each tick): {} line(s)", push_rows.len());
        for r in &push_rows {
            println!("    {r}");
        }
    }
    let judged = night_ticks as u64 * vs.len() as u64;
    if let Some((who, from, to)) = trace_spec.as_ref() {
        println!("  {who}'s trace t{from}..t{to} (game side; position after each tick, the block at its feet):");
        for r in &trace_rows {
            println!("    {r}");
        }
        anyhow::ensure!(!trace_rows.is_empty(), "MCTAS_REPLAY_TRACE: no tick traced");
    }
    // (G1's divergence check) the replay counts only while the game matches redsim's recorded state
    println!(
        "  divergence check (redsim's `# expect` state before each write vs the game's: carts in the act's column, each hold and pocket): {} of {expects_checked} write(s) diverged{}",
        divergences.len(),
        match first_div {
            Some(f) => format!("; the FIRST at t{f}: every result after it is the open-loop drive's, not the design's"),
            None if expects_checked == 0 => " (the drive carries no `# expect` lines: OPEN LOOP, NOT JUDGED as a design run)".to_string(),
            None => " (faithful through the run)".to_string(),
        }
    );
    for d in divergences.iter().take(40) {
        println!("    {d}");
    }
    // the size of each hold / pocket divergence: the game reaching redsim's state that many gt later
    // (or never within the run)
    let lags: Vec<String> = pending.iter().take(60).map(|(t, _, m)| format!("t{t}: {}", m.map(|x| format!("+{} gt", x - t)).unwrap_or_else(|| "not within 100 gt".into()))).collect();
    if !lags.is_empty() {
        println!("  hold / pocket divergences, the game reaching redsim's state (first 60): {}", lags.join("; "));
    }
    if let Some(p) = plant_at {
        let caught = planted.is_some_and(|pt| divergences.iter().any(|d| d.starts_with(&format!("t{pt} ")) && d.contains("(planted)")));
        println!("  the planted control (a wrong expected state at the first write at or after t{p}): {}", match planted {
            Some(pt) if caught => format!("planted at t{pt}, CAUGHT"),
            Some(pt) => format!("planted at t{pt}, NOT caught"),
            None => "never planted (no expect line at or after it)".into(),
        });
        anyhow::ensure!(caught, "the planted divergence control failed: the comparison can't be trusted");
    }
    if let Some(who) = air_watch.as_deref() {
        println!("  {who}'s air supply per tick inside the main lift's box ({} tick(s)):", air_rows.len());
        for r in &air_rows {
            println!("    {r}");
        }
        anyhow::ensure!(!air_rows.is_empty(), "MCTAS_REPLAY_AIR={who}: no tick inside the lift's box: NOT JUDGED");
    }
    // (ironspec 06:35 / 06:37 platform) every REST spell: its length, WALK_TARGET at entry and its
    // source, the sink's stop, the HOME-target tick or "never in the spell", the path, any arming or
    // release; the at-entry stamp bar judged at each spell's first SetWalkTargetFromBlockMemory run
    let mut at_entry_bad: Vec<String> = Vec::new();
    let mut at_entry_n = 0usize;
    let mut at_entry_after = 0usize;
    let mut h_spells = 0usize;
    for v in vs.iter_mut() {
        if let Some(sp) = v.spell.take() {
            let d = sp.describe(None);
            v.spells.push(d);
        }
        println!("  {}: {} REST spell(s)", v.tag, v.spells.len());
        for s in &v.spells {
            println!("    {s}");
            // H = the holds (x -0.5 / 3.5 / 7.5, y 15.9..17.2, z 0..1)
            if let Some(p) = s.split(" at (").nth(1).and_then(|r| r.split(')').next()) {
                let c: Vec<f64> = p.split(',').filter_map(|n| n.trim().parse().ok()).collect();
                if c.len() == 3 && [-0.5, 3.5, 7.5].iter().any(|x| (c[0] - x).abs() < 0.3) && (15.9..17.2).contains(&c[1]) && (0.0..1.0).contains(&c[2]) {
                    h_spells += 1;
                }
            }
        }
        for (t, age) in &v.entry_judged {
            // (ironspec 07:02 (1)) after the first divergence a result is the drift's, NOT JUDGED
            if first_div.is_some_and(|f| *t >= f) {
                at_entry_after += 1;
                continue;
            }
            at_entry_n += 1;
            if age.is_some_and(|a| a >= 1000) {
                at_entry_bad.push(format!("{} t{t} stamp age {}", v.tag, age.unwrap_or(0)));
            }
        }
    }
    println!(
        "  the at-entry stamp bar (< 1,000 gt at each spell's first SetWalkTargetFromBlockMemory run): {} of {at_entry_n} judged spell(s) at or over 1,000{}; spells at H: {h_spells}{}",
        at_entry_bad.len(),
        if at_entry_n == 0 { " (NOT JUDGED: 0 samples)".to_string() } else if at_entry_bad.is_empty() { " (HELD)".to_string() } else { format!(" (FAILED: {})", at_entry_bad.join("; ")) },
        first_div.map(|f| format!("; {at_entry_after} spell(s) from t{f} on NOT JUDGED (after the first divergence)")).unwrap_or_default()
    );
    let gone: Vec<String> = vs.iter().filter_map(|v| v.gone.as_ref().map(|g| format!("{} {g}", v.tag))).collect();
    println!("  villagers lost: {} {:?}", gone.len(), gone);
    let drowning: Vec<String> = vs.iter().flat_map(|v| v.hurts.iter().filter(|h| h.contains("hurt drown")).map(move |h| format!("{} {h}", v.tag))).collect();
    println!(
        "  air: the least air supply read per villager (every 20 gt, < 300 only): {}; drowning damage taken: {} {:?}",
        vs.iter().filter_map(|v| v.min_air.as_ref().map(|m| format!("{} {} at t{} {}", v.tag, m.0, m.1, m.2))).collect::<Vec<_>>().join("; "),
        drowning.len(),
        drowning.iter().take(12).collect::<Vec<_>>()
    );
    // (ironspec 09:42 (R1)) the ruled night bars fail the run; the stricter old counts print only
    let releases: u32 = vs.iter().map(|v| v.releases).sum();
    let mut lost_by: BTreeMap<String, usize> = BTreeMap::new();
    for v in &vs {
        for h in &v.home_lost {
            let mech = h.split(" by ").nth(1).and_then(|r| r.split('.').next()).unwrap_or("?").to_string();
            *lost_by.entry(mech).or_insert(0) += 1;
        }
    }
    for v in vs.iter() {
        if let Some((s, _)) = v.at_stop {
            farm_stop.push(format!("{} at the stop since t{s} at the run's end (least distance {:.3}), no lie-down", v.tag, v.stop_least));
            stop_visits.push(format!("{} t{s}: least {:.3}, still at the stop at the run's end", v.tag, v.stop_least));
        }
    }
    println!("  stop visits (within 3.0 of the bed head's centre at night; per visit: the least distance, the arrival -> lie-down time): {}", stop_visits.len());
    for sv in &stop_visits {
        println!("    visit {sv}");
    }
    println!(
        "  printed only (ironspec 09:42 (R1)): night REST ticks without a complete HOME path {partial} of {judged} villager-ticks (trips excluded: {complete}); villagers whose stamp ever reached 1,000 gt: {old}"
    );
    println!(
        "  the night bars: (a) at-entry stamp (above); (b) release-branch runs: {releases} ({}); (c) HOME LOST by mechanism: {lost} {:?} ({}); (d) EXPOSURE (per tick): {} ({}); (e) FARM-STOP: {} ({}); and villagers lost {}, drowning hits {}, no-op acts {} ({})",
        if releases == 0 { "HELD" } else { "FAILED" },
        lost_by,
        if lost == 0 { "HELD" } else { "FAILED" },
        collisions.len(),
        if collisions.is_empty() { "HELD" } else { "FAILED" },
        farm_stop.len(),
        if farm_stop.is_empty() { "HELD" } else { "FAILED" },
        gone.len(),
        drowning.len(),
        noop.len(),
        if gone.is_empty() && drowning.is_empty() && noop.is_empty() { "HELD" } else { "FAILED" }
    );
    for c in &collisions {
        println!("    exposure {c}");
    }
    for f in &farm_stop {
        println!("    farm-stop {f}");
    }
    for n in &noop {
        println!("    no-op act {n}");
    }
    anyhow::ensure!(unread.is_empty(), "{} agent line(s) unread (first: {}): the run fails", unread.len(), unread.first().map(|s| s.as_str()).unwrap_or(""));
    // (G1 on H #1) every `# expect` line must be checked (one that never ran, or slipped a name map,
    // isn't a pass), and an open-loop drive is no design run: it exits non-zero unless declared
    // fidelity-only (MCTAS_REPLAY_FIDELITY_ONLY=1, printed)
    let expect_lines = acts.iter().filter(|a| a.expect.is_some()).count();
    // a run shorter than the drive checks the expect lines inside its own ticks
    let expect_in_run = acts.iter().filter(|a| a.expect.is_some() && a.t <= ticks).count();
    let fidelity_only = std::env::var_os("MCTAS_REPLAY_FIDELITY_ONLY").is_some();
    println!(
        "  run kind: {}; expect lines {expect_lines} ({expect_in_run} inside the run), checked {expects_checked}",
        if expect_lines > 0 { "closed-loop checked (a design run)" } else if fidelity_only { "OPEN LOOP, declared fidelity-only (MCTAS_REPLAY_FIDELITY_ONLY): mechanics only, no design verdict" } else { "OPEN LOOP (no `# expect` lines): not a design run" }
    );
    anyhow::ensure!(expects_checked == expect_in_run, "{expects_checked} of the {expect_in_run} `# expect` line(s) inside the run's {ticks} gt checked: the run fails");
    anyhow::ensure!(expect_lines > 0 || fidelity_only, "an OPEN-LOOP drive (no `# expect` lines) is no design run: declare it with MCTAS_REPLAY_FIDELITY_ONLY=1 for a mechanics-only row; the run fails");
    anyhow::ensure!(judged > 0, "no night tick judged: NOT JUDGED, the run fails");
    // (G1 on K #2) the acts inside the run (the loop runs t = 0..=ticks, so acts at t <= ticks apply)
    let acts_in_run = acts.iter().filter(|a| a.t <= ticks).count();
    anyhow::ensure!(applied == acts_in_run, "only {applied} of the {acts_in_run} acts inside the run applied within {ticks} gt");
    anyhow::ensure!(first_div.is_none() || expects_checked == 0, "the replay diverged from redsim's recorded state at t{}: every design result after it is NOT JUDGED (drift): the run fails as a design run", first_div.unwrap_or(0));
    anyhow::ensure!(
        at_entry_bad.is_empty() && releases == 0 && lost == 0 && collisions.is_empty() && farm_stop.is_empty() && noop.is_empty() && gone.is_empty() && drowning.is_empty(),
        "a night bar failed (at-entry {}, release-branch runs {releases}, HOME lost {lost}, exposure {}, farm-stop {}, no-op acts {}, villagers lost {}, drowning hits {})",
        at_entry_bad.len(),
        collisions.len(),
        farm_stop.len(),
        noop.len(),
        gone.len(),
        drowning.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{shift_meta_value, shift_scene, tagged};

    #[test]
    fn tags_vehicle_and_passengers() {
        let (s, k) = tagged("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}", 7);
        assert_eq!(s, "{Tags:[\"e7\"],Rotation:[0f,0f],Passengers:[{Tags:[\"e7p0\"],id:\"minecraft:villager\"}]}");
        assert_eq!(k, vec!["minecraft:villager".to_string()]);
        let (s, k) = tagged("", 3);
        assert_eq!(s, "{Tags:[\"e3\"]}");
        assert!(k.is_empty());
    }

    /// (G1 on U, the control) every coordinate-carrying metadata key comes out translated at its top
    /// level, its notes as written; held-ts too; the acts and the NBT HOME move; refusals by name
    #[test]
    fn shift_scene_translates_metadata_and_refuses_by_name() {
        let by = [6, 0, 6];
        let cases = [
            ("positions", "-1,0,0 -1,16,0 3,0,0", "5,0,6 5,16,6 9,0,6"),
            ("run-cells", "P0 -1,0,0; D0 -1,0,2", "P0 5,0,6; D0 5,0,8"),
            ("keep-clear", "-3,-26,3 -3,-25,3 (A6's header, + (0,-27,-1): the bed's column)", "3,-26,9 3,-25,9 (A6's header, + (0,-27,-1): the bed's column)"),
            ("keep-block", "-5,-24,3=oak_leaves[distance=7,persistent=true] (A6's local (-5,3,4))", "1,-24,9=oak_leaves[distance=7,persistent=true] (A6's local (-5,3,4))"),
            ("keep-out", "10,-2,3 10,-2,4", "16,-2,9 16,-2,10"),
            ("riderless", "1,-26,-5+0,-27,-5=the stop's", "7,-26,1+6,-27,1=the stop's"),
            ("placement-order-ts", "-1,0,7=the merge T at lane -1 (placed after (-1,0,6))", "5,0,13=the merge T at lane -1 (placed after (-1,0,6))"),
            ("update-only-ts", "1,25,9+5,25,9=the split's K junctions", "7,25,15+11,25,15=the split's K junctions"),
            ("held-ts", "1,25,9=north_west (lever (1,26,9)); 5,25,9=north_east", "7,25,15=north_west (lever (1,26,9)); 11,25,15=north_east"),
            ("emulated", "7,-2,8=glass (a lever at (7,-1,8))", "13,-2,14=glass (a lever at (7,-1,8))"),
            ("list", "-1,0,0, 3,0,0", "5,0,6, 9,0,6"),
            ("dwell-cap", "15 (PROVISIONAL, line 3)", "15 (PROVISIONAL, line 3)"),
        ];
        for (k, v, want) in cases {
            let (got, _) = shift_meta_value(v, by).unwrap();
            assert_eq!(got, want, "key {k}");
        }
        assert!(shift_meta_value("1,2,3,4,5,6", by).is_err(), "an ambiguous run refuses");
        let dir = std::env::temp_dir().join(format!("shift-scene-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut d = mctas::design::Design { name: "t".into(), ..Default::default() };
        for (k, v, _) in cases {
            d.meta.insert(k.into(), v.into());
        }
        d.blocks.insert([0, 0, 0], "minecraft:stone".into());
        d.ports.insert("home".into(), [-3, -27, 3]);
        d.entities.push(mctas::design::EntitySpec {
            kind: "minecraft:minecart".into(),
            pos: [-0.5, 0.0625, 0.5],
            nbt: Some("{Passengers:[{id:\"minecraft:villager\",Brain:{memories:{\"minecraft:home\":{value:{dimension:\"minecraft:overworld\",pos:[I;-3,-27,3]}}}}}]}".into()),
        });
        let (scene, drive, so, dout) = (dir.join("s.txt"), dir.join("d.txt"), dir.join("so.txt"), dir.join("do.txt"));
        d.save(&scene).unwrap();
        std::fs::write(&drive, "act 0 home e0.p0 -3 -27 3\nact 0 last-slept e0.p0 -100\nact 5 set 1 2 3 minecraft:stone\n# expect t5 x\n").unwrap();
        shift_scene(&scene, &drive, by, &so, &dout).unwrap();
        let o = mctas::design::Design::load(&so).unwrap();
        for (k, _, want) in cases {
            assert_eq!(o.meta.get(k).map(|s| s.as_str()), Some(want), "key {k} in the shifted file");
        }
        assert!(o.meta.get("shifted-by").is_some_and(|v| v.starts_with("6,0,6 ")));
        assert_eq!(o.ports.get("home"), Some(&[3, -27, 9]));
        assert!(o.entities[0].nbt.as_deref().unwrap_or("").contains("pos:[I;3,-27,9]"));
        assert_eq!(std::fs::read_to_string(&dout).unwrap(), "act 0 home e0.p0 3 -27 9\nact 0 last-slept e0.p0 -100\nact 5 set 7 2 9 minecraft:stone\n");
        // refusals, by name
        std::fs::write(&drive, "act 3 frob 1 2 3\n").unwrap();
        let e = shift_scene(&scene, &drive, by, &so, &dout).unwrap_err().to_string();
        assert!(e.contains("act verb `frob`"), "{e}");
        let mut bad = d.clone();
        bad.meta.insert("cells".into(), "1,2,3,4".into());
        bad.save(&scene).unwrap();
        std::fs::write(&drive, "").unwrap();
        let e = shift_scene(&scene, &drive, by, &so, &dout).unwrap_err().to_string();
        assert!(e.contains("metadata key `cells`"), "{e}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
