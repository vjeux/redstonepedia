//! (ironspec Nov 14 12:26p / 12:28p (3), part 80; G1 13:46, part 92) The load-order census: the load-order window touches
//! only hoppers and random ticks (scheduled ticks wait for entities, by tick-exact's quotes), so the statics list, for a
//! composed record, in its OVERWORLD section and its NETHER section (each labelled), every block whose RANDOM tick can
//! change its state (persistent leaves excepted), every HOPPER (those under a rail marked) and every LAVA cell (its
//! fluid random tick sets fire to flammables within its reach), each with its cell and the part that lays it (read from
//! the record's `# +` lines: the part whose file, moved as composed, holds that state at that cell; "the record's own"
//! when no part does). A state whose random tick can't be told is listed NOT JUDGED by name, never passed.

use crate::settle::composed_moves;
use std::collections::BTreeMap;

/// One census row: the cell, its state, its owner part (its `# +` NAME, or "the record's own"), and its section.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub cell: [i32; 3],
    pub state: String,
    pub owner: String,
    pub nether: bool,
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
    /// (G1 13:46, part 92) every lava cell: its fluid random tick (`LavaFluid.randomTick`) sets fire to flammables
    /// within its reach (tick-exact's quote: the Nether casing statics carry "no flammable within lava's fire reach").
    pub lava: Vec<Row>,
    /// The states whose random tick can't be told: NOT JUDGED.
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

/// (G1 13:46, part 92; tick-exact 13:55 quoted the 26.3 overrides of `isRandomlyTicking(BlockState)`) Blocks whose
/// random ticking depends on the STATE (`CropBlock` `!isMaxAge` (wheat, carrots, potatoes, beetroots, torchflower),
/// `PitcherCropBlock` (lower half, not max age), `NetherWartBlock` `AGE < 3`, `GrowingPlantHeadBlock` `AGE < 25` (kelp,
/// weeping / twisting / cave vines heads), `BambooStalkBlock` `STAGE == 0`, `CocoaBlock` `AGE < 2`, `SweetBerryBushBlock`
/// `AGE < 3`, `ChorusFlowerBlock` `AGE < 5`, `RedStoneOreBlock` `LIT`, `FlowerPotBlock` (the potted eyeblossoms), every
/// weathering copper block (`WeatheringCopper.getNext(..).isPresent()`, the lightning rods among them)): for a state the
/// table lacks, the block's first row can't stand in, so such a state is NOT JUDGED.
pub const STATE_DEPENDENT_26_3: &[&str] = &[
    "minecraft:wheat",
    "minecraft:carrots",
    "minecraft:potatoes",
    "minecraft:beetroots",
    "minecraft:torchflower_crop",
    "minecraft:pitcher_crop",
    "minecraft:nether_wart",
    "minecraft:kelp",
    "minecraft:twisting_vines",
    "minecraft:weeping_vines",
    "minecraft:cave_vines",
    "minecraft:bamboo",
    "minecraft:cocoa",
    "minecraft:sweet_berry_bush",
    "minecraft:chorus_flower",
    "minecraft:redstone_ore",
    "minecraft:deepslate_redstone_ore",
    "minecraft:potted_open_eyeblossom",
    "minecraft:potted_closed_eyeblossom",
];

/// Whether a state's 26.3 block ticks randomly: Some(the block table's `random_ticking` flag), or None (NOT JUDGED) when
/// the table holds neither the state nor any state of its block (and the block isn't in [`NO_RANDOM_TICK_26_3`]), or
/// lacks the state of a block whose flag depends on the state ([`STATE_DEPENDENT_26_3`], copper's weathering family).
/// The flag is per block for the rest (`BlockBehaviour.Properties.randomTicks()`), so a state the table lacks reads its
/// block's first row; leaves are read from the state itself (26.3 `LeavesBlock.isRandomlyTicking`: `distance == 7 &&
/// !persistent`).
pub fn random_ticking(state: &str) -> Option<bool> {
    let block = state.split('[').next().unwrap_or(state);
    if block.ends_with("_leaves") {
        let prop = |k: &str| state.split(['[', ',', ']']).find_map(|kv| kv.strip_prefix(k).and_then(|v| v.strip_prefix('=')));
        return Some(prop("distance") == Some("7") && prop("persistent") != Some("true"));
    }
    if let Some(id) = redsim::terrain::blocks::id_of(state) {
        return Some(redsim::terrain::blocks::props(id).random_ticking);
    }
    // (tick-exact 13:55: `LiquidBlock` ticks randomly as its fluid does: lava yes (`LavaFluid.randomTick`, fire), water no)
    if block == "minecraft:lava" {
        return Some(true);
    }
    if block == "minecraft:water" {
        return Some(false);
    }
    if STATE_DEPENDENT_26_3.contains(&block) || block.contains("copper") || block.contains("lightning_rod") {
        return None;
    }
    match redsim::terrain::blocks::first_of_block(block) {
        Some(id) => Some(redsim::terrain::blocks::props(id).random_ticking),
        None => NO_RANDOM_TICK_26_3.iter().any(|(b, _)| *b == block).then_some(false),
    }
}

/// The census of a composed record `text` (its parts' files read from the repository's top `top`), its overworld section
/// and its Nether section. A part file that can't be read or parsed is an error naming it (an owner can't be told); a
/// delta part's cells stay "the record's own".
pub fn census(text: &str, top: &std::path::Path) -> Result<Census, String> {
    let d = mctas::design::Design::parse(text).map_err(|e| format!("the record doesn't parse: {e}"))?;
    let moves = composed_moves(text)?;
    // (a DELTA part, `# + NAME (delta): PATH …`, is no design file: its cells stay "the record's own" here)
    let deltas: Vec<&str> = text.lines().filter_map(|l| l.strip_prefix("# + ")).filter_map(|r| r.split_once(": ")).filter(|(n, _)| n.trim().ends_with(" (delta)")).filter_map(|(_, rest)| rest.split_whitespace().next()).collect();
    // (each part's overworld cells moved as composed; its Nether cells in the Nether part's own coordinates)
    type Cells = BTreeMap<[i32; 3], String>;
    let mut parts: Vec<(String, Cells, Cells, [i32; 3])> = Vec::new();
    for (name, path, off) in &moves {
        if deltas.contains(&path.as_str()) {
            continue;
        }
        let pt = std::fs::read_to_string(top.join(path)).map_err(|e| format!("part {name}: {path}: {e}"))?;
        let pd = mctas::design::Design::parse(&pt).map_err(|e| format!("part {name}: {path} doesn't parse: {e}"))?;
        parts.push((name.clone(), pd.blocks, pd.nether_blocks, *off));
    }
    let owner = |c: &[i32; 3], s: &str, nether: bool| -> String {
        let o = parts
            .iter()
            .filter(|(_, ow, ne, o)| if nether { ne.get(c).is_some_and(|x| x == s) } else { ow.get(&[c[0] - o[0], c[1] - o[1], c[2] - o[2]]).is_some_and(|x| x == s) })
            .map(|(n, ..)| n.clone())
            .collect::<Vec<_>>()
            .join("+");
        if o.is_empty() { "the record's own".to_string() } else { o }
    };
    let mut out = Census::default();
    for (blocks, nether) in [(&d.blocks, false), (&d.nether_blocks, true)] {
        let row = |c: &[i32; 3], s: &str| Row { cell: *c, state: s.to_string(), owner: owner(c, s, nether), nether };
        for (c, s) in blocks {
            if s.starts_with("minecraft:lava") {
                out.lava.push(row(c, s));
            }
            match random_ticking(s) {
                Some(true) => {
                    out.random.push(row(c, s));
                    let watched_by = |q: &[i32; 3], st: &str| -> bool {
                        let f = st.split(['[', ',', ']']).find_map(|kv| kv.strip_prefix("facing="));
                        let dv = match f {
                            Some("north") => [0, 0, -1],
                            Some("south") => [0, 0, 1],
                            Some("east") => [1, 0, 0],
                            Some("west") => [-1, 0, 0],
                            Some("up") => [0, 1, 0],
                            Some("down") => [0, -1, 0],
                            _ => return false,
                        };
                        [q[0] + dv[0], q[1] + dv[1], q[2] + dv[2]] == *c
                    };
                    out.watchers.push(blocks.iter().filter(|(q, st)| st.starts_with("minecraft:observer") && watched_by(q, st)).map(|(q, _)| *q).collect());
                }
                Some(false) => {}
                None => out.unknown.push(row(c, s)),
            }
            if s.starts_with("minecraft:hopper") {
                let under_rail = blocks.get(&[c[0], c[1] + 1, c[2]]).is_some_and(|a| a.contains("rail"));
                out.hoppers.push((row(c, s), under_rail));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    /// (part 80) the census on a small composed record: a part laying a hopper (a rail on it) and wheat (random-ticking),
    /// a persistent leaf (excepted) and a decaying one (counted), stone (neither), a sapling the block table doesn't hold
    /// (NOT JUDGED); each row with its cell and owner; (part 92) the NETHER section too, labelled: a Nether hopper under a
    /// rail and a lava cell listed there; a state-dependent block's missing state NOT JUDGED
    #[test]
    fn census_lists_random_ticks_and_hoppers_by_owner() {
        let dir = std::env::temp_dir().join(format!("census-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("part.txt"), "name: p\norigin: 0 0 0\npalette:\n  h minecraft:hopper[enabled=true,facing=down]\n  w minecraft:wheat[age=0]\nlayer 0:\n  hw\n").unwrap();
        let record = "# + P: part.txt (md5 0) moved by 1,0,0 into the frame, then 0,0,0 into its tier\nname: r\norigin: 0 0 0\npalette:\n  s minecraft:stone\n  h minecraft:hopper[enabled=true,facing=down]\n  w minecraft:wheat[age=0]\n  L minecraft:oak_leaves[distance=7,persistent=true,waterlogged=false]\n  l minecraft:oak_leaves[distance=7,persistent=false,waterlogged=false]\n  o minecraft:oak_sapling[stage=0]\n  r minecraft:rail[shape=north_south,waterlogged=false]\n  b minecraft:observer[facing=down,powered=false]\n  v minecraft:lava[level=0]\nlayer 0:\n  shwLlo\nlayer 1:\n  .rb\nnether-origin: 0 0 0\nnether layer 0:\n  hv\nnether layer 1:\n  r\n";
        let c = super::census(record, &dir).unwrap();
        let cells = |v: &[super::Row]| v.iter().map(|r| (r.cell, r.owner.clone())).collect::<Vec<_>>();
        let ow: Vec<&(super::Row, bool)> = c.hoppers.iter().filter(|(r, _)| !r.nether).collect();
        assert!(ow.len() == 1 && ow[0].0.cell == [1, 0, 0] && ow[0].0.owner == "P" && ow[0].1, "the hopper, laid by P, a rail on it: {:?}", c.hoppers);
        assert!(c.random.iter().any(|r| r.cell == [2, 0, 0] && r.owner == "P" && !r.nether), "the wheat ticks randomly, laid by P: {:?}", cells(&c.random));
        assert!(c.random.iter().any(|r| r.cell == [4, 0, 0] && r.owner == "the record's own"), "the decaying leaf, the record's own: {:?}", cells(&c.random));
        assert!(!c.random.iter().any(|r| !r.nether && (r.cell == [3, 0, 0] || r.cell == [0, 0, 0])), "persistent leaves and stone excepted: {:?}", cells(&c.random));
        assert!(c.unknown.len() == 1 && c.unknown[0].cell == [5, 0, 0], "the sapling the block table doesn't hold: NOT JUDGED: {:?}", cells(&c.unknown));
        let watch = |cell: [i32; 3]| c.random.iter().position(|r| r.cell == cell && !r.nether).map(|i| c.watchers[i].clone());
        assert_eq!(watch([2, 0, 0]), Some(vec![[2, 1, 0]]), "the observer above the wheat, facing down, watches it");
        assert_eq!(watch([4, 0, 0]), Some(vec![]), "nothing watches the leaf");
        assert_eq!(super::random_ticking("minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]"), Some(false), "a block the table lacks, by its cited 26.3 class");
        // (part 92) the Nether section, labelled: its hopper under its rail, its lava listed
        assert!(c.hoppers.iter().any(|(r, under)| r.nether && r.cell == [0, 0, 0] && *under && r.owner == "the record's own"), "the Nether hopper under a rail: {:?}", c.hoppers);
        assert!(c.lava.len() == 1 && c.lava[0].nether && c.lava[0].cell == [1, 0, 0], "the Nether lava listed: {:?}", c.lava);
        assert_eq!(super::random_ticking("minecraft:wheat[age=99]"), None, "a state-dependent block's missing state: NOT JUDGED, never its first row");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
