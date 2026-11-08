//! The natural-state table (`blocks.tsv`, written by `mctas terrain blockinfo`): every block state
//! of the terrain samples (and of the mob-farm designs pasted into them), described by the game
//! itself through the bridge's `blockinfo` op. `Block::Terrain(id)` indexes it; redsim reads a
//! natural state's collision boxes, light, solidity, spawn rules and tags from its row.

use std::collections::HashMap;
use std::sync::OnceLock;

/// The table as committed.
pub const TSV: &str = include_str!("blocks.tsv");

/// A fluid in a state (`getFluidState().getType()`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FluidKind {
    Empty,
    Water,
    Lava,
}

/// One state's row.
#[derive(Clone, Debug)]
pub struct Props {
    /// The game's string (`BlockStateParser.serialize`).
    pub name: String,
    /// The block's class chain (its own class first, up to `Block`).
    pub classes: Vec<String>,
    /// The reacting methods its classes declare (`updateShape`, `neighborChanged`, `tick`, ...).
    pub reactive: Vec<String>,
    /// Collision boxes in the block's coordinates (`getCollisionShape(...).toAabbs()`).
    pub boxes: Vec<[f64; 6]>,
    pub can_occlude: bool,
    pub shape_occlusion: bool,
    /// `getFaceOcclusionShape` per direction (down, up, north, south, west, east) on a 16 x 16
    /// grid, rows of bits as `daylight.rs` reads them; `None` unless it occludes by shape.
    pub faces: Option<[[u16; 16]; 6]>,
    pub dampening: u8,
    pub emission: u8,
    pub skylight_down: bool,
    pub solid: bool,
    pub solid_render: bool,
    pub conductor: bool,
    pub full_collision: bool,
    /// `isFaceSturdy` per support (FULL, CENTER, RIGID) per direction (down, up, north, south,
    /// west, east).
    pub sturdy: [[bool; 6]; 3],
    pub signal: bool,
    pub air: bool,
    pub liquid: bool,
    pub replaceable: bool,
    pub suffocating: bool,
    /// `isPathfindable` for LAND, WATER, AIR.
    pub path: [bool; 3],
    pub fluid: FluidKind,
    pub fluid_amount: u8,
    pub fluid_source: bool,
    /// The rule-relevant block tags (`minecraft:` names).
    pub tags: Vec<String>,
    pub leaves_block: bool,
    pub random_ticking: bool,
    pub friction: f32,
    pub speed_factor: f32,
    pub jump_factor: f32,
    pub push: String,
    pub block_entity: bool,
    /// `Heightmap.Types.isOpaque` for WORLD_SURFACE, MOTION_BLOCKING, MOTION_BLOCKING_NO_LEAVES,
    /// OCEAN_FLOOR.
    pub heightmaps: [bool; 4],
    /// Per entity type of [`Table::types`]: `isValidSpawn(level, pos, type)`.
    pub valid_spawn: Vec<bool>,
    /// Per entity type: `type.isBlockDangerous(state)`.
    pub dangerous: Vec<bool>,
    pub burn_odds: i32,
    pub ignite_odds: i32,
    /// `Block.getExplosionResistance()`.
    pub resistance: f32,
    /// Per direction (down, up, north, south, west, east): whether the neighbour there turning from
    /// dirt into air changes this state (`updateShape`: it pops off or reshapes); every direction
    /// when the state cannot stand among dirt at all (the bridge's `blockinfo`).
    pub support: [bool; 6],
}

impl Props {
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// The block's id without properties (`minecraft:oak_leaves`).
    pub fn block_id(&self) -> &str {
        self.name.split('[').next().unwrap_or(&self.name)
    }

    /// Its collision shape leaves its cell (`hasLargeCollisionShape`).
    pub fn large_shape(&self) -> bool {
        self.boxes.iter().any(|b| b.iter().take(3).any(|v| *v < 0.0) || b.iter().skip(3).any(|v| *v > 1.0))
    }
}

/// The parsed table.
pub struct Table {
    pub props: Vec<Props>,
    /// Canonical names and the raw palette strings that name the same state (a 26.x chunk writes a
    /// default state as the bare block id).
    pub by_name: HashMap<String, u16>,
    /// The entity types of the per-type columns (`minecraft:` ids).
    pub types: Vec<String>,
}

fn bits(s: &str) -> Vec<bool> {
    s.chars().map(|c| c == '1').collect()
}

fn parse_row(line: &str) -> Result<(Props, Vec<String>), String> {
    let f: Vec<&str> = line.split('\t').collect();
    if f.len() != 40 {
        return Err(format!("{} fields in {}", f.len(), line.chars().take(80).collect::<String>()));
    }
    let list = |s: &str, sep: char| -> Vec<String> { if s == "-" { Vec::new() } else { s.split(sep).map(|x| x.to_string()).collect() } };
    let num = |s: &str| -> Result<f64, String> { s.parse::<f64>().map_err(|_| format!("bad number {s:?}")) };
    let b = |s: &str| s == "1";
    let boxes = if f[4] == "-" {
        Vec::new()
    } else {
        f[4].split(';')
            .map(|bx| {
                let v: Vec<f64> = bx.split(' ').filter_map(|x| x.parse().ok()).collect();
                if v.len() == 6 { Ok([v[0], v[1], v[2], v[3], v[4], v[5]]) } else { Err(format!("bad box {bx:?}")) }
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    let faces = if f[7] == "-" {
        None
    } else {
        let mut out = [[0u16; 16]; 6];
        for (d, face) in f[7].split(' ').enumerate().take(6) {
            for r in 0..16 {
                out[d][r] = u16::from_str_radix(&face[r * 4..r * 4 + 4], 16).map_err(|_| format!("bad face {face:?}"))?;
            }
        }
        Some(out)
    };
    let st = bits(f[15]);
    let mut sturdy = [[false; 6]; 3];
    for (k, v) in st.iter().enumerate().take(18) {
        sturdy[k / 6][k % 6] = *v;
    }
    let path = bits(f[21]);
    let hm = bits(f[33]);
    let fluid = match f[22] {
        "minecraft:empty" => FluidKind::Empty,
        "minecraft:water" | "minecraft:flowing_water" => FluidKind::Water,
        "minecraft:lava" | "minecraft:flowing_lava" => FluidKind::Lava,
        other => return Err(format!("unknown fluid {other}")),
    };
    let p = Props {
        name: f[0].to_string(),
        classes: list(f[2], ','),
        reactive: list(f[3], ','),
        boxes,
        can_occlude: b(f[5]),
        shape_occlusion: b(f[6]),
        faces,
        dampening: num(f[8])? as u8,
        emission: num(f[9])? as u8,
        skylight_down: b(f[10]),
        solid: b(f[11]),
        solid_render: b(f[12]),
        conductor: b(f[13]),
        full_collision: b(f[14]),
        sturdy,
        signal: b(f[16]),
        air: b(f[17]),
        liquid: b(f[18]),
        replaceable: b(f[19]),
        suffocating: b(f[20]),
        path: [path.first() == Some(&true), path.get(1) == Some(&true), path.get(2) == Some(&true)],
        fluid,
        fluid_amount: num(f[23])? as u8,
        fluid_source: b(f[24]),
        tags: list(f[25], ','),
        leaves_block: b(f[26]),
        random_ticking: b(f[27]),
        friction: num(f[28])? as f32,
        speed_factor: num(f[29])? as f32,
        jump_factor: num(f[30])? as f32,
        push: f[31].to_string(),
        block_entity: b(f[32]),
        heightmaps: [hm.first() == Some(&true), hm.get(1) == Some(&true), hm.get(2) == Some(&true), hm.get(3) == Some(&true)],
        valid_spawn: bits(f[34]),
        dangerous: bits(f[35]),
        burn_odds: num(f[36])? as i32,
        ignite_odds: num(f[37])? as i32,
        resistance: num(f[38])? as f32,
        support: {
            let s: Vec<bool> = f[39].chars().map(|c| c == '1').collect();
            if s.len() != 6 {
                return Err(format!("support {} in {}", f[39], f[0]));
            }
            [s[0], s[1], s[2], s[3], s[4], s[5]]
        },
    };
    Ok((p, list(f[1], ' ')))
}

/// Parse a table text (the committed one: [`table`]).
pub fn parse(text: &str) -> Result<Table, String> {
    let mut t = Table { props: Vec::new(), by_name: HashMap::new(), types: Vec::new() };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# types ") {
            t.types = rest.split(',').map(|s| s.to_string()).collect();
            continue;
        }
        if line.starts_with('#') || line.starts_with("name\t") || line.is_empty() {
            continue;
        }
        let (p, aliases) = parse_row(line)?;
        let id = t.props.len() as u16;
        t.by_name.insert(p.name.clone(), id);
        for a in aliases {
            t.by_name.insert(a, id);
        }
        t.props.push(p);
    }
    Ok(t)
}

static TABLE: OnceLock<Table> = OnceLock::new();

/// The committed table.
pub fn table() -> &'static Table {
    TABLE.get_or_init(|| parse(TSV).unwrap_or_else(|e| panic!("terrain/blocks.tsv: {e}")))
}

/// A state's row id by its game string (canonical, or a raw palette alias).
pub fn id_of(state: &str) -> Option<u16> {
    let t = table();
    t.by_name.get(state).copied().or_else(|| if state.starts_with("minecraft:") { None } else { t.by_name.get(&format!("minecraft:{state}")).copied() })
}

/// A row.
pub fn props(id: u16) -> &'static Props {
    &table().props[id as usize]
}

/// The column of an entity type (`minecraft:zombie`) in the per-type bitsets.
pub fn type_index(type_id: &str) -> Option<usize> {
    table().types.iter().position(|t| t == type_id)
}

/// A direction's index in the table's per-direction columns (down, up, north, south, west, east).
pub fn dir_index(d: crate::pos::Dir) -> usize {
    use crate::pos::Dir;
    match d {
        Dir::Down => 0,
        Dir::Up => 1,
        Dir::North => 2,
        Dir::South => 3,
        Dir::West => 4,
        Dir::East => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_parses_and_reads_like_the_game() {
        let t = table();
        assert!(t.props.len() > 100, "{} states", t.props.len());
        let grass = props(id_of("minecraft:grass_block[snowy=false]").unwrap());
        assert_eq!(grass.dampening, 15);
        assert!(grass.solid && grass.conductor && grass.full_collision);
        assert_eq!(grass.boxes, vec![[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]]);
        let zombie = type_index("minecraft:zombie").unwrap();
        assert!(grass.valid_spawn[zombie]);
        let leaves = props(id_of("minecraft:oak_leaves[distance=1,persistent=false,waterlogged=false]").unwrap());
        assert_eq!(leaves.dampening, 1);
        assert!(!leaves.valid_spawn[zombie], "leaves: ocelots and parrots only");
        let short = props(id_of("minecraft:short_grass").unwrap());
        assert!(short.boxes.is_empty() && !short.solid);
        // a raw palette alias (a default state written as its bare id) finds its row
        assert_eq!(id_of("minecraft:short_grass"), id_of("short_grass"));
    }
}
