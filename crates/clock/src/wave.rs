//! Period, waveform and family from a run's states: shared by the exact redsim detector
//! ([`crate::sim`]) and the real-game judge ([`crate::verify`]), so both measure the same way.

use crate::Pos;
use std::collections::HashMap;

/// Is a component "on" in this state? (the waveform's on-ticks and rising edges)
///
/// Pistons: extended; a piston head or a moving block: present; sculk sensors: ACTIVE; hoppers:
/// locked (`enabled=false`); doors, trapdoors, fence gates: open; dispensers, droppers, crafters:
/// triggered; leaves: distance < 7; scaffolding: distance > 0; else `lit`, then `power > 0`, then
/// `powered`; a block with none of these: present (not air).
pub fn active(state: &str) -> bool {
    let name = mctas::blockstate::block_name(state);
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    if matches!(name, "air" | "cave_air" | "void_air") {
        return false;
    }
    let prop = |k: &str| -> Option<&str> {
        let inner = state.split_once('[')?.1.trim_end_matches(']');
        inner.split(',').find_map(|kv| kv.split_once('=').filter(|(kk, _)| kk.trim() == k).map(|(_, v)| v.trim()))
    };
    let is = |k: &str, v: &str| prop(k) == Some(v);
    match name {
        "piston" | "sticky_piston" => return is("extended", "true"),
        "piston_head" | "moving_piston" => return true,
        "sculk_sensor" | "calibrated_sculk_sensor" => return is("sculk_sensor_phase", "active"),
        "hopper" => return is("enabled", "false"),
        "dispenser" | "dropper" | "crafter" => return is("triggered", "true"),
        "oak_leaves" => return prop("distance").and_then(|d| d.parse::<u8>().ok()).is_some_and(|d| d < 7),
        "scaffolding" => return prop("distance").and_then(|d| d.parse::<u8>().ok()).is_some_and(|d| d > 0),
        _ => {}
    }
    if name.ends_with("_door") || name.ends_with("_trapdoor") || name.ends_with("_fence_gate") {
        return is("open", "true");
    }
    if let Some(v) = prop("lit") {
        return v == "true";
    }
    if let Some(v) = prop("power") {
        return v.parse::<u8>().is_ok_and(|p| p > 0);
    }
    if let Some(v) = prop("powered") {
        return v == "true";
    }
    true
}

/// A block state's name in a family signature, or `None` for blocks with no behavior of their
/// own (plain blocks, slabs, logs...: they only move or hold things up) and air. Piston heads and
/// moving blocks count as their piston (`type=sticky` -> `sticky_piston`), wall torches as
/// torches, every waxed copper bulb as `copper_bulb`, wall skulls as skulls.
pub fn family_name(state: &str) -> Option<String> {
    let name = mctas::blockstate::block_name(state);
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    match name {
        "air" | "cave_air" | "void_air" => return None,
        // a moving block says nothing about which piston moves it (`type=normal` is any moved
        // block): the piston's own base cell always changes too and names it
        "moving_piston" => return None,
        "piston_head" => {
            return Some(if state.contains("type=sticky") { "sticky_piston" } else { "piston" }.into());
        }
        "redstone_wall_torch" => return Some("redstone_torch".into()),
        "skeleton_wall_skull" => return Some("skeleton_skull".into()),
        _ => {}
    }
    if name.contains("copper_bulb") {
        return Some("copper_bulb".into());
    }
    match redsim::Block::parse(state) {
        Ok(redsim::Block::Simple(_) | redsim::Block::Glazed { .. } | redsim::Block::Slab { .. } | redsim::Block::Log { .. }) => None,
        // modeled behavior blocks and blocks redsim does not know: their own name
        _ => Some(name.to_string()),
    }
}

/// The block-entity part of a cell's state that the clock judge compares and a family counts,
/// from the game's saved data (`kind` = `minecraft:comparator`, SNBT with sorted keys): a
/// comparator's output strength `{out=N}`; a container's items `{items=slot:count:id,...}` (and a
/// hopper's `cd=TransferCooldown`). Other block-entity data is left out: moving blocks are implied
/// by the block states, and sculk sensors carry absolute game times.
pub fn be_extra_from_nbt(kind: &str, nbt: &str) -> Option<String> {
    let kind = kind.strip_prefix("minecraft:").unwrap_or(kind);
    let int_after = |key: &str| -> Option<i64> {
        let i = nbt.find(key)? + key.len();
        let digits: String = nbt[i..].chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
        digits.parse().ok()
    };
    match kind {
        "comparator" => Some(format!("{{out={}}}", int_after("OutputSignal:").unwrap_or(0))),
        "hopper" | "dropper" | "dispenser" | "crafter" => {
            let mut items = items_of(nbt);
            items.sort();
            let list: Vec<String> = items.iter().map(|(s, c, i)| format!("{s}:{c}:{i}")).collect();
            let cd = if kind == "hopper" { format!(",cd={}", int_after("TransferCooldown:").unwrap_or(-1)) } else { String::new() };
            Some(format!("{{items={}{cd}}}", list.join(",")))
        }
        // the smelter study (track sorter): a furnace's slots, burn time left and cooking timer (a
        // blast furnace's alike, tickexact 2026-11-10)
        "furnace" | "blast_furnace" => {
            let mut items = items_of(nbt);
            items.sort();
            let list: Vec<String> = items.iter().map(|(s, c, i)| format!("{s}:{c}:{i}")).collect();
            Some(format!("{{items={},lit={},cook={}}}", list.join(","), int_after("lit_time_remaining:").unwrap_or(0), int_after("cooking_time_spent:").unwrap_or(0)))
        }
        // the brewer study (track sorter): a brewing stand's slots, brew time and fuel uses left
        "brewing_stand" => {
            let mut items = items_of(nbt);
            items.sort();
            let list: Vec<String> = items.iter().map(|(s, c, i)| format!("{s}:{c}:{i}")).collect();
            Some(format!("{{items={},brew={},fuel={}}}", list.join(","), int_after("BrewTime:").unwrap_or(0), int_after("Fuel:").unwrap_or(0)))
        }
        _ => None,
    }
}

/// The items of an SNBT `Items:[...]` list (the game's saved container data, keys sorted):
/// (slot, count, id without `minecraft:`), each read from its compound's own top-level keys. A
/// nested part (an item's `components`, a list) is skipped and a quoted string read whole, so a
/// named item keeps its place (the sorter's "filter" papers: `{Slot:1b,components:{..},count:1,
/// id:"minecraft:paper"}` split at `},` lost them) and an id holding a `b` keeps its name
/// (`minecraft:bucket` was cut at the `b`, the byte suffix of `Slot:0b`).
fn items_of(nbt: &str) -> Vec<(i64, i64, String)> {
    let Some(start) = nbt.find("Items:[") else { return Vec::new() };
    let mut items = Vec::new();
    let (mut depth, mut in_str, mut escape) = (0i32, false, false);
    // `cur`: the item compound's own top-level text; `raw`: all of it (nested parts included)
    let mut cur = String::new();
    let mut raw = String::new();
    for ch in nbt[start + 7..].chars() {
        if in_str {
            if depth == 1 {
                cur.push(ch);
            }
            if depth >= 1 {
                raw.push(ch);
            }
            if ch == '"' && !escape {
                in_str = false;
            }
            escape = ch == '\\' && !escape;
            continue;
        }
        match ch {
            '"' => {
                in_str = true;
                escape = false;
                if depth == 1 {
                    cur.push(ch);
                }
                if depth >= 1 {
                    raw.push(ch);
                }
            }
            '{' | '[' => {
                depth += 1;
                if depth == 1 {
                    cur.clear();
                    raw.clear();
                } else {
                    raw.push(ch);
                }
            }
            '}' | ']' if depth == 0 => break,
            '}' | ']' => {
                if depth == 1 {
                    let mut slot = None;
                    let (mut count, mut id) = (1i64, None);
                    for kv in cur.split(',') {
                        let Some((k, v)) = kv.split_once(':') else { continue };
                        match k.trim() {
                            "Slot" => slot = v.trim().trim_end_matches(['b', 'B']).parse::<i64>().ok(),
                            "count" => count = v.trim().trim_end_matches(char::is_alphabetic).parse().unwrap_or(1),
                            "id" => {
                                let s = v.trim().trim_matches('"');
                                id = Some(s.strip_prefix("minecraft:").unwrap_or(s).to_string());
                            }
                            _ => {}
                        }
                    }
                    // the brewer study: a potion is named by its contents' potion (`potion[awkward]`)
                    if let Some(p) = potion_of(&raw) {
                        id = id.map(|i| format!("{i}[{p}]"));
                    }
                    if let (Some(s), Some(i)) = (slot, id) {
                        items.push((s, count, i));
                    }
                } else {
                    raw.push(ch);
                }
                depth -= 1;
            }
            _ => {
                if depth == 1 {
                    cur.push(ch);
                }
                if depth >= 1 {
                    raw.push(ch);
                }
            }
        }
    }
    items
}

/// The `potion` of an item's `minecraft:potion_contents` component in its SNBT text (without
/// `minecraft:`), if any.
pub fn potion_of(item_snbt: &str) -> Option<String> {
    let i = item_snbt.find("potion_contents")?;
    let rest = &item_snbt[i..];
    let j = rest.find("potion:\"")? + "potion:\"".len();
    let v: String = rest[j..].chars().take_while(|c| *c != '"').collect();
    Some(v.strip_prefix("minecraft:").unwrap_or(&v).to_string())
}

/// Whether [`be_extra_redsim`] reads anything at a block of this kind (a comparator's strength, a
/// container's contents): a block it does not read has no extras, so a run that sees the same
/// block again has nothing more to compare (`clock::sim`).
pub fn reads_extras(b: &redsim::Block) -> bool {
    matches!(b, redsim::Block::Comparator { .. } | redsim::Block::Hopper { .. } | redsim::Block::Dispenser { .. } | redsim::Block::Crafter { .. } | redsim::Block::Furnace { .. } | redsim::Block::BrewingStand { .. })
}

/// What [`be_extra_redsim`] reads at a comparator, its output strength (`None`: not a comparator):
/// its extras are this number alone, so a run that sees the same comparator with the same
/// strength again keeps the text it made instead of making it again (`clock::sim`).
pub fn comparator_read(w: &redsim::World, p: redsim::BlockPos, b: &redsim::Block) -> Option<u8> {
    matches!(b, redsim::Block::Comparator { .. }).then(|| w.comparator_out.get(&p).copied().unwrap_or(0))
}

/// [`be_extra_from_nbt`] from a redsim world: the same text for the same state. (Only the kinds
/// [`reads_extras`] lists have extras: a kind added below must be added there.)
pub fn be_extra_redsim(w: &redsim::World, p: redsim::BlockPos, b: &redsim::Block) -> Option<String> {
    if !reads_extras(b) {
        return None;
    }
    match b {
        redsim::Block::Comparator { .. } => Some(format!("{{out={}}}", comparator_read(w, p, b).unwrap_or(0))),
        redsim::Block::Hopper { .. } | redsim::Block::Dispenser { .. } | redsim::Block::Crafter { .. } => {
            let c = w.containers.map.get(&p);
            let mut list: Vec<String> = Vec::new();
            if let Some(c) = c {
                for (slot, st) in c.slots.iter().enumerate() {
                    if let Some(st) = st {
                        list.push(format!("{slot}:{}:{}", st.count, redsim::redstone::container::item_name(st.item).trim_start_matches("minecraft:")));
                    }
                }
            }
            let cd = if matches!(b, redsim::Block::Hopper { .. }) { format!(",cd={}", c.map(|c| c.cooldown).unwrap_or(-1)) } else { String::new() };
            Some(format!("{{items={}{cd}}}", list.join(",")))
        }
        redsim::Block::Furnace { .. } => {
            let c = w.containers.map.get(&p);
            let mut list: Vec<String> = Vec::new();
            if let Some(c) = c {
                for (slot, st) in c.slots.iter().enumerate() {
                    if let Some(st) = st {
                        list.push(format!("{slot}:{}:{}", st.count, redsim::redstone::container::item_name(st.item).trim_start_matches("minecraft:")));
                    }
                }
            }
            Some(format!("{{items={},lit={},cook={}}}", list.join(","), c.map(|c| c.lit_time).unwrap_or(0), c.map(|c| c.cook_timer).unwrap_or(0)))
        }
        redsim::Block::BrewingStand { .. } => {
            let c = w.containers.map.get(&p);
            let mut list: Vec<String> = Vec::new();
            if let Some(c) = c {
                for (slot, st) in c.slots.iter().enumerate() {
                    if let Some(st) = st {
                        list.push(format!("{slot}:{}:{}", st.count, redsim::redstone::container::item_name(st.item).trim_start_matches("minecraft:")));
                    }
                }
            }
            Some(format!("{{items={},brew={},fuel={}}}", list.join(","), c.map(|c| c.brew_time).unwrap_or(0), c.map(|c| c.fuel_uses).unwrap_or(0)))
        }
        _ => None,
    }
}

/// A block state with its compared block-entity part appended (`minecraft:comparator[...]{out=15}`).
pub fn with_extra(state: &str, extra: Option<&str>) -> String {
    match extra {
        Some(e) => format!("{state}{e}"),
        None => state.to_string(),
    }
}

/// The part of a block-entity extra that belongs to the clock's VISIBLE state (what decides the
/// period): a comparator's output strength. Container contents and hopper cooldowns are hidden
/// state (seen only through a comparator, whose strength is visible): they count for the family
/// and the waveform tables, not for the period.
pub fn visible_extra(extra: Option<&str>) -> Option<&str> {
    extra.filter(|e| e.starts_with("{out="))
}

/// The block state of a cell state that may carry a block-entity part.
pub fn block_part(s: &str) -> &str {
    match (s.find(']'), s.find('{')) {
        (Some(b), Some(e)) if e > b => &s[..e],
        (None, Some(e)) => &s[..e],
        _ => s,
    }
}

/// On-ticks, rising edges and pulse widths of a cyclic boolean sequence (one period). A
/// sequence that is always on has one pulse of the whole period and no rising edge.
pub fn bool_wave(seq: &[bool]) -> (u32, u32, Vec<u32>) {
    let n = seq.len();
    let on = seq.iter().filter(|b| **b).count() as u32;
    if n == 0 || on == 0 {
        return (0, 0, Vec::new());
    }
    if on as usize == n {
        return (on, 0, vec![on]);
    }
    let mut edges = 0;
    let mut widths = Vec::new();
    // start at a rising edge
    let start = (0..n).find(|&i| seq[i] && !seq[(i + n - 1) % n]).unwrap();
    let mut run = 0u32;
    for k in 0..n {
        let i = (start + k) % n;
        if seq[i] {
            if run == 0 {
                edges += 1;
            }
            run += 1;
        } else if run > 0 {
            widths.push(run);
            run = 0;
        }
    }
    if run > 0 {
        widths.push(run);
    }
    (on, edges, widths)
}

/// The ticks between consecutive rising edges of a cyclic boolean sequence (one period), from the
/// first rising edge; they sum to the period. Empty without a rising edge. A builder's period is
/// the output's: equal gaps = a regular output of period `gaps[0]`; unequal = irregular (a site
/// critique, 2026-10-20: a 476 gt state period whose output pulses every 40-60 gt).
pub fn rise_gaps(seq: &[bool]) -> Vec<u32> {
    let n = seq.len();
    let rises: Vec<usize> = (0..n).filter(|&i| seq[i] && !seq[(i + n - 1) % n]).collect();
    if rises.is_empty() {
        return Vec::new();
    }
    (0..rises.len()).map(|k| ((rises[(k + 1) % rises.len()] + n - rises[k] - 1) % n + 1) as u32).collect()
}

/// The waveform of one cell over one period.
#[derive(Clone, Debug)]
pub struct CellWave {
    pub pos: Pos,
    /// The states the cell takes in the period, in order of first appearance (shortened).
    pub states: Vec<String>,
    /// Family names of those states ([`family_name`]).
    pub kinds: Vec<String>,
    pub on: u32,
    pub edges: u32,
    pub widths: Vec<u32>,
    /// Ticks between consecutive rising edges ([`rise_gaps`]).
    pub gaps: Vec<u32>,
}

impl CellWave {
    /// `on 2/6, 1 edge, pulses 2`.
    pub fn describe(&self, period: u32) -> String {
        format!("on {}/{period}, {} edge{}, pulses {:?}", self.on, self.edges, if self.edges == 1 { "" } else { "s" }, self.widths)
    }

    /// Several rising edges per period at unequal spacing: (shortest gap, longest gap). Such an
    /// output has no single period a builder could time with.
    pub fn irregular(&self) -> Option<(u32, u32)> {
        let (lo, hi) = (*self.gaps.iter().min()?, *self.gaps.iter().max()?);
        (self.gaps.len() >= 2 && lo != hi).then_some((lo, hi))
    }

    /// The output's own period: the gap between rising edges when they are evenly spaced (the
    /// whole period with one edge), None when irregular or without edges.
    pub fn output_period(&self) -> Option<u32> {
        let g0 = *self.gaps.first()?;
        self.gaps.iter().all(|&g| g == g0).then_some(g0)
    }

    /// Whether a declared `port output` here reads what this wave shows: the cell always holds the
    /// same component, read by its own state (a plain block moving through, or a head, reads as
    /// the power the cell receives instead).
    pub fn tappable(&self) -> bool {
        let mut names = self.states.iter().map(|s| mctas::blockstate::block_name(block_part(s)).to_string());
        let Some(first) = names.next() else { return false };
        family_name(block_part(&self.states[0])).is_some() && !first.contains("moving_piston") && !first.contains("piston_head") && names.all(|n| n == first)
    }
}

/// The analysis of one period: every cell whose state changes, the family (sorted, deduplicated)
/// and the output's waveform.
#[derive(Clone, Debug, Default)]
pub struct PeriodAnalysis {
    pub cells: Vec<CellWave>,
    pub family: Vec<String>,
    /// Blocks that only move during the period (pushed or pulled, their own state unchanged):
    /// not in the family.
    pub moved: Vec<String>,
}

/// Analyze one period: `states[t][i]` = the state of `cells[i]` at tick `W + t` (t in 0..P).
pub fn analyze(cells: &[Pos], states: &[Vec<&str>]) -> PeriodAnalysis {
    analyze_with(cells, states.len(), |t, i| states[t][i], |i| states.iter().any(|s| s[i] != states[0][i]))
}

/// [`analyze`] over `n` ticks read through `get(tick, cell)`, with `changes(cell)` whether a cell's
/// state changes at all in them (the simulator keeps one period as interned states per changing
/// cell, not whole frames of Strings: the coordinator 2026-10-21, 82 GB site builds).
pub fn analyze_with<'a>(cells: &[Pos], n: usize, get: impl Fn(usize, usize) -> &'a str, changes: impl Fn(usize) -> bool) -> PeriodAnalysis {
    let mut out = PeriodAnalysis::default();
    let p = n;
    if p == 0 {
        return out;
    }
    let mut fam: Vec<String> = Vec::new();
    let mut moved: Vec<String> = Vec::new();
    let type_of = |s: &str| -> String { block_part(s).split('[').next().unwrap_or("").to_string() };
    for (i, pos) in cells.iter().enumerate() {
        if !changes(i) {
            continue;
        }
        let mut seen: Vec<String> = Vec::new();
        for t in 0..n {
            let st = crate::short(get(t, i)).to_string();
            if !seen.contains(&st) {
                seen.push(st);
            }
        }
        // A block counts in the family when its own state changes IN PLACE (the same block in the
        // cell before and after: lit, powered, power, open, extended, phase, a comparator's
        // strength, a container's contents), or when it is a redstone block that moves (a moving
        // power source). A block that only moves (a lamp a piston pushes, never lit) is listed
        // apart as moved.
        let mut kinds: Vec<String> = Vec::new();
        let mut cell_moved: Vec<String> = Vec::new();
        for t in 0..n {
            let (a, b) = (get(t, i), get((t + 1) % n, i));
            if a != b && type_of(a) == type_of(b) {
                if let Some(k) = family_name(block_part(a)) {
                    kinds.push(k);
                }
            }
        }
        for s in &seen {
            if let Some(k) = family_name(block_part(s)) {
                if k == "redstone_block" {
                    kinds.push(k);
                } else if !kinds.contains(&k) {
                    cell_moved.push(k);
                }
            }
        }
        kinds.sort();
        kinds.dedup();
        cell_moved.retain(|k| !kinds.contains(k));
        let seq: Vec<bool> = (0..n).map(|t| active(block_part(get(t, i)))).collect();
        let (on, edges, widths) = bool_wave(&seq);
        let gaps = rise_gaps(&seq);
        fam.extend(kinds.iter().cloned());
        moved.extend(cell_moved);
        out.cells.push(CellWave { pos: *pos, states: seen, kinds, on, edges, widths, gaps });
    }
    fam.sort();
    fam.dedup();
    moved.sort();
    moved.dedup();
    moved.retain(|k| !fam.contains(k));
    out.family = fam;
    out.moved = moved;
    out
}

/// How many groups the changing cells split into: two cells are linked when they are at most 2
/// apart (Chebyshev distance: blocks touching, a dust step, quasi-connectivity), and a sculk sensor
/// links every changing cell within its listening range (8, calibrated 16). Independent sub-clocks
/// side by side are 2 or more groups; one mechanism is one.
pub fn spatial_groups(positions: &[Pos], sensors: &[(Pos, i32)]) -> usize {
    let n = positions.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut Vec<usize>, i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        let mut j = i;
        while p[j] != r {
            let next = p[j];
            p[j] = r;
            j = next;
        }
        r
    }
    let cheb = |a: Pos, b: Pos| (0..3).map(|k| (a[k] - b[k]).abs()).max().unwrap_or(0);
    for i in 0..n {
        for j in i + 1..n {
            let near = cheb(positions[i], positions[j]) <= 2 || sensors.iter().any(|(s, r)| cheb(*s, positions[i]) <= *r && cheb(*s, positions[j]) <= *r);
            if near {
                let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                parent[a] = b;
            }
        }
    }
    (0..n).filter(|&i| find(&mut parent, i) == i).count()
}

/// The sculk sensors of a design with their listening ranges.
pub fn sensors_of(blocks: &std::collections::BTreeMap<Pos, String>) -> Vec<(Pos, i32)> {
    blocks
        .iter()
        .filter_map(|(p, s)| {
            if s.contains("calibrated_sculk_sensor") {
                Some((*p, 16))
            } else if s.contains("sculk_sensor") {
                Some((*p, 8))
            } else {
                None
            }
        })
        .collect()
}

/// The smallest d dividing `seq.len()` such that the cyclic sequence repeats every d.
pub fn cyclic_period<T: PartialEq>(seq: &[T]) -> usize {
    let n = seq.len();
    (1..=n.max(1)).filter(|d| n % d == 0).find(|&d| (0..n).all(|t| seq[t] == seq[(t + d) % n])).unwrap_or(n.max(1))
}

/// The family signature: `observer+sticky_piston`, or `-` for none.
pub fn signature(family: &[String]) -> String {
    if family.is_empty() { "-".into() } else { family.join("+") }
}

/// The smallest period of a sequence of frame ids that is periodic from some warmup on, seen in
/// `frames[start..]`: for every candidate p, `W_p` = the earliest tick from which
/// `frames[t] == frames[t + p]` holds to the end; candidates need at least `min_periods` full
/// periods after `W_p`; the answer is the candidate with the smallest `W_p`, then the smallest p
/// (a multiple of the true period never starts earlier, and a false period that only holds on a
/// quiet tail starts later). Returns (warmup, period).
pub fn detect_period(frames: &[u64], start: usize, min_periods: usize) -> Option<(usize, usize)> {
    let n = frames.len();
    if n <= start + 1 {
        return None;
    }
    let mut best: Option<(usize, usize)> = None;
    let maxp = (n - start) / min_periods.max(1);
    for p in 1..=maxp {
        // earliest W such that frames[t] == frames[t+p] for all t in W..n-p
        let mut w = n - p;
        while w > start && frames[w - 1] == frames[w - 1 + p] {
            w -= 1;
        }
        if n - w < min_periods * p {
            continue;
        }
        if best.is_none_or(|(bw, bp)| (w, p) < (bw, bp)) {
            best = Some((w, p));
        }
    }
    best
}

/// Interns frames (vectors of state ids) and states (strings) into dense ids.
#[derive(Default)]
pub struct Interner {
    pub states: Vec<String>,
    state_ids: HashMap<String, u32>,
    frame_ids: HashMap<Vec<u32>, u64>,
}

impl Interner {
    pub fn state(&mut self, s: &str) -> u32 {
        if let Some(i) = self.state_ids.get(s) {
            return *i;
        }
        let i = self.states.len() as u32;
        self.states.push(s.to_string());
        self.state_ids.insert(s.to_string(), i);
        i
    }
    pub fn frame(&mut self, f: &[u32]) -> u64 {
        let n = self.frame_ids.len() as u64;
        *self.frame_ids.entry(f.to_vec()).or_insert(n)
    }
    pub fn name(&self, id: u32) -> &str {
        &self.states[id as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waves() {
        assert_eq!(bool_wave(&[true, true, false, false, false, false]), (2, 1, vec![2]));
        assert_eq!(bool_wave(&[false, true, false, true]), (2, 2, vec![1, 1]));
        assert_eq!(bool_wave(&[true, false, false, true]), (2, 1, vec![2]));
        assert_eq!(bool_wave(&[true, true]), (2, 0, vec![2]));
        assert_eq!(bool_wave(&[false, false]), (0, 0, vec![]));
        assert_eq!(rise_gaps(&[true, false, false, true, false, false]), vec![3, 3]);
        assert_eq!(rise_gaps(&[false, true, false, true, true, false, false]), vec![2, 5]);
        assert_eq!(rise_gaps(&[true, true, false]), vec![3]);
        assert_eq!(rise_gaps(&[true, true]), Vec::<u32>::new());
        assert_eq!(rise_gaps(&[false, false]), Vec::<u32>::new());
    }

    #[test]
    fn periods() {
        // warmup 3, period 4
        let f: Vec<u64> = vec![9, 8, 7, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2];
        assert_eq!(detect_period(&f, 0, 3), Some((3, 4)));
        // a quiet tail does not fake period 1
        let mut g: Vec<u64> = Vec::new();
        for _ in 0..5 {
            g.extend([1, 2, 3]);
            g.extend([0; 20]);
        }
        assert_eq!(detect_period(&g, 0, 3), Some((0, 23)));
        assert_eq!(detect_period(&[5, 5, 5, 5], 0, 3), Some((0, 1)));
    }

    #[test]
    fn activity_and_family() {
        assert!(active("minecraft:observer[facing=up,powered=true]"));
        assert!(!active("minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]"));
        assert!(active("minecraft:sticky_piston[extended=true,facing=up]"));
        assert!(!active("minecraft:air"));
        assert!(active("minecraft:stone"));
        assert_eq!(family_name("minecraft:moving_piston[facing=up,type=sticky]"), None);
        assert_eq!(family_name("minecraft:piston_head[facing=up,short=false,type=sticky]").as_deref(), Some("sticky_piston"));
        assert_eq!(family_name("minecraft:stone"), None);
        assert_eq!(family_name("minecraft:redstone_wall_torch[facing=north,lit=true]").as_deref(), Some("redstone_torch"));
        assert_eq!(family_name("minecraft:waxed_copper_bulb[lit=true,powered=false]").as_deref(), Some("copper_bulb"));
        assert_eq!(be_extra_from_nbt("minecraft:comparator", "{OutputSignal:13}").as_deref(), Some("{out=13}"));
        assert_eq!(
            be_extra_from_nbt("minecraft:hopper", "{Items:[{Slot:0b,count:3,id:\"minecraft:stone\"},{Slot:4b,count:1,id:\"minecraft:redstone\"}],TransferCooldown:7}").as_deref(),
            Some("{items=0:3:stone,4:1:redstone,cd=7}")
        );
        assert_eq!(be_extra_from_nbt("minecraft:hopper", "{Items:[],TransferCooldown:-1}").as_deref(), Some("{items=,cd=-1}"));
        assert_eq!(block_part("minecraft:comparator[facing=west,mode=subtract,powered=true]{out=2}"), "minecraft:comparator[facing=west,mode=subtract,powered=true]");
        // the pick sweep (2026-10-21): the sorter's named filter papers (sorted keys put
        // `components` between `Slot` and `count`) and ids holding a `b` keep their place
        assert_eq!(
            be_extra_from_nbt("minecraft:hopper", r#"{Items:[{Slot:0b,count:64,id:"minecraft:iron_ingot"},{Slot:1b,components:{"minecraft:custom_name":"lock"},count:1,id:"minecraft:paper"}],TransferCooldown:0}"#).as_deref(),
            Some("{items=0:64:iron_ingot,1:1:paper,cd=0}")
        );
        assert_eq!(be_extra_from_nbt("minecraft:dropper", r#"{Items:[{Slot:3b,count:16,id:"minecraft:bucket"},{Slot:0b,count:2,id:"minecraft:cobblestone"}]}"#).as_deref(), Some("{items=0:2:cobblestone,3:16:bucket}"));
    }
}
