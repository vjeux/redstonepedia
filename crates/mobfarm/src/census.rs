//! (ironspec Nov 14 12:26p / 12:28p (3), part 80) The load-order census: the load-order window touches only hoppers and
//! random ticks (scheduled ticks wait for entities, by tick-exact's quotes), so the statics list, for a composed record,
//! every block whose RANDOM tick can change its state (persistent leaves excepted) and every HOPPER (those under a rail
//! marked), each with its cell and the part that lays it (read from the record's `# +` lines: the part whose file, moved
//! as composed, holds that state at that cell; "the record's own" when no part does). A state the game's block table
//! (redsim's natural-state table) doesn't hold can't be told: it is listed NOT JUDGED by name, never passed.

use crate::settle::composed_moves;
use std::collections::BTreeMap;

/// One census row: the cell, its state, its owner part (its `# +` NAME, or "the record's own").
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub cell: [i32; 3],
    pub state: String,
    pub owner: String,
}

/// The census of a record.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Census {
    /// The states whose 26.3 block ticks randomly (the block table's flag; persistent leaves excepted).
    pub random: Vec<Row>,
    /// Per random-ticking row (same order): the observers watching its cell (an observer at Q facing F watches Q + F),
    /// the one block a random state change would pulse.
    pub watchers: Vec<Vec<[i32; 3]>>,
    /// Every hopper, with whether a rail sits on it (the cell above holds a rail).
    pub hoppers: Vec<(Row, bool)>,
    /// The states the block table doesn't hold: NOT JUDGED (their random tick can't be told).
    pub unknown: Vec<Row>,
}

/// (part 80) The blocks of the farm designs the natural-state table holds no state of, each with its 26.3 class: none
/// sets `BlockBehaviour.Properties.randomTicks()` or overrides `isRandomlyTicking` (their own ticks are scheduled ones,
/// where they have any). Read only for a block the table lacks; every other block the census can't tell is NOT JUDGED.
pub const NO_RANDOM_TICK_26_3: &[(&str, &str)] = &[
    ("minecraft:activator_rail", "26.3 PoweredRailBlock (BaseRailBlock): no randomTicks()"),
    ("minecraft:detector_rail", "26.3 DetectorRailBlock (BaseRailBlock): scheduled ticks only (checkPressed)"),
    ("minecraft:powered_rail", "26.3 PoweredRailBlock (BaseRailBlock): no randomTicks()"),
    ("minecraft:iron_trapdoor", "26.3 TrapDoorBlock: no randomTicks()"),
    ("minecraft:lever", "26.3 LeverBlock: no randomTicks()"),
    ("minecraft:redstone_block", "26.3 PoweredBlock: no randomTicks()"),
    ("minecraft:slime_block", "26.3 SlimeBlock (HalfTransparentBlock): no randomTicks()"),
    ("minecraft:soul_sand", "26.3 SoulSandBlock: its tick() is the scheduled bubble-column tick; no randomTicks()"),
];

/// Whether a state's 26.3 block ticks randomly: Some(the block table's `random_ticking` flag), or None when the table
/// holds neither the state nor any state of its block (and the block isn't in [`NO_RANDOM_TICK_26_3`]). The flag is per
/// block in 26.3 (`BlockBehaviour.Properties.randomTicks()`), so a state the table lacks reads its block's first row;
/// leaves are the state-dependent case (26.3 `LeavesBlock.isRandomlyTicking`: `distance == 7 && !persistent`), read
/// from the state itself.
pub fn random_ticking(state: &str) -> Option<bool> {
    let block = state.split('[').next().unwrap_or(state);
    if block.ends_with("_leaves") {
        let prop = |k: &str| state.split(['[', ',', ']']).find_map(|kv| kv.strip_prefix(k).and_then(|v| v.strip_prefix('=')));
        return Some(prop("distance") == Some("7") && prop("persistent") != Some("true"));
    }
    match redsim::terrain::blocks::id_of(state).or_else(|| redsim::terrain::blocks::first_of_block(block)) {
        Some(id) => Some(redsim::terrain::blocks::props(id).random_ticking),
        None => NO_RANDOM_TICK_26_3.iter().any(|(b, _)| *b == block).then_some(false),
    }
}

/// The census of a composed record `text` (its parts' files read from the repository's top `top`). A part file that
/// can't be read or parsed is an error naming it (an owner can't be told); a delta part's cells stay "the record's own".
pub fn census(text: &str, top: &std::path::Path) -> Result<Census, String> {
    let d = mctas::design::Design::parse(text).map_err(|e| format!("the record doesn't parse: {e}"))?;
    let moves = composed_moves(text)?;
    // (a DELTA part, `# + NAME (delta): PATH …`, is no design file: its cells stay "the record's own" here)
    let deltas: Vec<&str> = text.lines().filter_map(|l| l.strip_prefix("# + ")).filter_map(|r| r.split_once(": ")).filter(|(n, _)| n.trim().ends_with(" (delta)")).filter_map(|(_, rest)| rest.split_whitespace().next()).collect();
    let mut parts: Vec<(String, BTreeMap<[i32; 3], String>, [i32; 3])> = Vec::new();
    for (name, path, off) in &moves {
        if deltas.contains(&path.as_str()) {
            continue;
        }
        let pt = std::fs::read_to_string(top.join(path)).map_err(|e| format!("part {name}: {path}: {e}"))?;
        let pd = mctas::design::Design::parse(&pt).map_err(|e| format!("part {name}: {path} doesn't parse: {e}"))?;
        parts.push((name.clone(), pd.blocks, *off));
    }
    let owner = |c: &[i32; 3], s: &str| -> String {
        let o = parts.iter().filter(|(_, b, o)| b.get(&[c[0] - o[0], c[1] - o[1], c[2] - o[2]]).is_some_and(|x| x == s)).map(|(n, _, _)| n.clone()).collect::<Vec<_>>().join("+");
        if o.is_empty() { "the record's own".to_string() } else { o }
    };
    let row = |c: &[i32; 3], s: &str| Row { cell: *c, state: s.to_string(), owner: owner(c, s) };
    let mut out = Census::default();
    for (c, s) in &d.blocks {
        match random_ticking(s) {
            Some(true) => {
                out.random.push(row(c, s));
                let watched_by = |q: &[i32; 3], st: &str| -> bool {
                    let f = st.split(['[', ',', ']']).find_map(|kv| kv.strip_prefix("facing="));
                    let d = match f {
                        Some("north") => [0, 0, -1],
                        Some("south") => [0, 0, 1],
                        Some("east") => [1, 0, 0],
                        Some("west") => [-1, 0, 0],
                        Some("up") => [0, 1, 0],
                        Some("down") => [0, -1, 0],
                        _ => return false,
                    };
                    [q[0] + d[0], q[1] + d[1], q[2] + d[2]] == *c
                };
                out.watchers.push(d.blocks.iter().filter(|(q, st)| st.starts_with("minecraft:observer") && watched_by(q, st)).map(|(q, _)| *q).collect());
            }
            Some(false) => {}
            None => out.unknown.push(row(c, s)),
        }
        if s.starts_with("minecraft:hopper") {
            let under_rail = d.blocks.get(&[c[0], c[1] + 1, c[2]]).is_some_and(|a| a.contains("rail"));
            out.hoppers.push((row(c, s), under_rail));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    /// (part 80) the census on a small composed record: a part laying a hopper (a rail on it) and wheat (random-ticking),
    /// a persistent leaf (excepted) and a decaying one (counted), stone (neither), a sapling the block table doesn't hold
    /// (NOT JUDGED); each row with its cell and owner
    #[test]
    fn census_lists_random_ticks_and_hoppers_by_owner() {
        let dir = std::env::temp_dir().join(format!("census-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("part.txt"), "name: p\norigin: 0 0 0\npalette:\n  h minecraft:hopper[enabled=true,facing=down]\n  w minecraft:wheat[age=0]\nlayer 0:\n  hw\n").unwrap();
        let record = "# + P: part.txt (md5 0) moved by 1,0,0 into the frame, then 0,0,0 into its tier\nname: r\norigin: 0 0 0\npalette:\n  s minecraft:stone\n  h minecraft:hopper[enabled=true,facing=down]\n  w minecraft:wheat[age=0]\n  L minecraft:oak_leaves[distance=7,persistent=true,waterlogged=false]\n  l minecraft:oak_leaves[distance=7,persistent=false,waterlogged=false]\n  o minecraft:oak_sapling[stage=0]\n  r minecraft:rail[shape=north_south,waterlogged=false]\n  b minecraft:observer[facing=down,powered=false]\nlayer 0:\n  shwLlo\nlayer 1:\n  .rb\n";
        let c = super::census(record, &dir).unwrap();
        let cells = |v: &[super::Row]| v.iter().map(|r| (r.cell, r.owner.clone())).collect::<Vec<_>>();
        assert!(c.hoppers.len() == 1 && c.hoppers[0].0.cell == [1, 0, 0] && c.hoppers[0].0.owner == "P" && c.hoppers[0].1, "the hopper, laid by P, a rail on it: {:?}", c.hoppers);
        assert!(c.random.iter().any(|r| r.cell == [2, 0, 0] && r.owner == "P"), "the wheat ticks randomly, laid by P: {:?}", cells(&c.random));
        assert!(c.random.iter().any(|r| r.cell == [4, 0, 0] && r.owner == "the record's own"), "the decaying leaf, the record's own: {:?}", cells(&c.random));
        assert!(!c.random.iter().any(|r| r.cell == [3, 0, 0] || r.cell == [0, 0, 0]), "persistent leaves and stone excepted: {:?}", cells(&c.random));
        assert!(c.unknown.len() == 1 && c.unknown[0].cell == [5, 0, 0], "the sapling the block table doesn't hold: NOT JUDGED: {:?}", cells(&c.unknown));
        let watch = |cell: [i32; 3]| c.random.iter().position(|r| r.cell == cell).map(|i| c.watchers[i].clone());
        assert_eq!(watch([2, 0, 0]), Some(vec![[2, 1, 0]]), "the observer above the wheat, facing down, watches it");
        assert_eq!(watch([4, 0, 0]), Some(vec![]), "nothing watches the leaf");
        assert_eq!(super::random_ticking("minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]"), Some(false), "a block the table lacks, by its cited 26.3 class");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
