//! The biome table (`biomes.tsv`, written by `mctas terrain biomes` from the game's own biome data
//! and tags): climate, colours, the tags the spawn rules read, and the natural spawns
//! (`minecraft:gameplay/natural_mob_spawns`).

// (the tables are looked up by name at every spawn check: a fast hash, and no code walks them in
// order; tickexact 2026-11-08)
use crate::fxhash::FxHashMap as HashMap;
use std::sync::OnceLock;

pub const TSV: &str = include_str!("biomes.tsv");

/// One `MobSpawnSettings.SpawnerData`: the type, its weight and count (an `IntProvider`:
/// constant or uniform).
#[derive(Clone, Debug, PartialEq)]
pub struct Spawner {
    pub category: String,
    pub ty: String,
    pub weight: i32,
    pub min: i32,
    pub max: i32,
}

/// One biome's row.
#[derive(Clone, Debug)]
pub struct Biome {
    pub name: String,
    pub temperature: f32,
    pub downfall: f32,
    pub has_precipitation: bool,
    pub temperature_modifier: String,
    /// Colours as 0xRRGGBB (`None`: no override).
    pub water_color: Option<u32>,
    pub grass_color: Option<u32>,
    pub foliage_color: Option<u32>,
    pub dry_foliage_color: Option<u32>,
    pub grass_modifier: String,
    pub tags: Vec<String>,
    pub surface_slime_chance: f32,
    pub spawners: Vec<Spawner>,
    /// `MobSpawnCost`: (type, charge, energy budget).
    pub costs: Vec<(String, f64, f64)>,
    /// The colours the client tints with before the grass modifier (`Biome.getGrassColor` /
    /// `getFoliageColor` / `getDryFoliageColor`: the override, else the colormap at the clamped
    /// climate), 0xRRGGBB.
    pub grass: u32,
    pub foliage: u32,
    pub dry_foliage: u32,
}

impl Biome {
    /// `Biome.getGrassColor(x, z)`: the resolved grass colour through the biome's modifier
    /// (`dark_forest`: `(c & 0xfefefe) + 0x28340a >> 1`; `swamp` reads `BIOME_INFO_NOISE`, not
    /// ported: an error).
    pub fn grass_color(&self, _x: f64, _z: f64) -> Result<u32, String> {
        match self.grass_modifier.as_str() {
            "none" => Ok(self.grass),
            "dark_forest" => Ok(((self.grass & 0xfefefe) + 0x28340a) >> 1),
            m => Err(format!("{}: grass colour modifier {m} is not modeled", self.name)),
        }
    }

    /// The water colour (the biome's `water_color`).
    pub fn water(&self) -> u32 {
        self.water_color.unwrap_or(0x3f76e4)
    }
}

impl Biome {
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// The weighted list of a category (`MobSpawnSettings.getMobs`).
    pub fn mobs(&self, category: &str) -> impl Iterator<Item = &Spawner> {
        self.spawners.iter().filter(move |s| s.category == category)
    }

    pub fn cost(&self, ty: &str) -> Option<(f64, f64)> {
        self.costs.iter().find(|c| c.0 == ty).map(|c| (c.1, c.2))
    }
}

fn hex(s: &str) -> Option<u32> {
    if s == "-" { None } else { u32::from_str_radix(s, 16).ok() }
}

fn parse(text: &str) -> Result<HashMap<String, Biome>, String> {
    let mut out = HashMap::default();
    for line in text.lines() {
        if line.starts_with('#') || line.starts_with("name\t") || line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 14 && f.len() != 17 {
            return Err(format!("{} fields in {}", f.len(), line.chars().take(60).collect::<String>()));
        }
        let resolved = |i: usize| f.get(i).and_then(|s| hex(s)).unwrap_or(0xff00ff);
        let mut spawners = Vec::new();
        if f[12] != "-" {
            for s in f[12].split(';') {
                let p: Vec<&str> = s.split(':').collect();
                // category:minecraft:type:weight:min:max
                if p.len() != 6 {
                    return Err(format!("bad spawner {s}"));
                }
                spawners.push(Spawner {
                    category: p[0].to_string(),
                    ty: format!("{}:{}", p[1], p[2]),
                    weight: p[3].parse().map_err(|_| format!("bad weight in {s}"))?,
                    min: p[4].parse().map_err(|_| format!("bad count in {s}"))?,
                    max: p[5].parse().map_err(|_| format!("bad count in {s}"))?,
                });
            }
        }
        let mut costs = Vec::new();
        if f[13] != "-" {
            for c in f[13].split(';') {
                let p: Vec<&str> = c.split(':').collect();
                if p.len() == 4 {
                    costs.push((format!("{}:{}", p[0], p[1]), p[2].parse().unwrap_or(0.0), p[3].parse().unwrap_or(0.0)));
                }
            }
        }
        let b = Biome {
            name: f[0].to_string(),
            temperature: f[1].parse().unwrap_or(0.5),
            downfall: f[2].parse().unwrap_or(0.5),
            has_precipitation: f[3] == "1",
            temperature_modifier: f[4].to_string(),
            water_color: hex(f[5]),
            grass_color: hex(f[6]),
            foliage_color: hex(f[7]),
            dry_foliage_color: hex(f[8]),
            grass_modifier: f[9].to_string(),
            tags: if f[10] == "-" { Vec::new() } else { f[10].split(',').map(|t| format!("minecraft:{t}")).collect() },
            surface_slime_chance: f[11].parse().unwrap_or(0.0),
            spawners,
            costs,
            grass: resolved(14),
            foliage: resolved(15),
            dry_foliage: resolved(16),
        };
        out.insert(b.name.clone(), b);
    }
    Ok(out)
}

static TABLE: OnceLock<HashMap<String, Biome>> = OnceLock::new();

pub const STRUCTURES_TSV: &str = include_str!("structures.tsv");

/// A structure's `StructureSpawnOverride` for one category.
#[derive(Clone, Debug)]
pub struct Override {
    pub category: String,
    /// `BoundingBoxType.PIECE` (else the whole start's box).
    pub piece: bool,
    pub spawns: Vec<Spawner>,
}

static STRUCTURES: OnceLock<HashMap<String, Vec<Override>>> = OnceLock::new();

fn parse_structures(text: &str) -> HashMap<String, Vec<Override>> {
    let mut out = HashMap::default();
    for line in text.lines() {
        if line.starts_with('#') || line.starts_with("name\t") || line.is_empty() {
            continue;
        }
        let Some((id, rest)) = line.split_once('\t') else { continue };
        if rest == "-" {
            continue;
        }
        let mut v = Vec::new();
        for cat in rest.split(';') {
            let mut parts = cat.splitn(3, ':');
            let (Some(c), Some(mode), Some(spawns)) = (parts.next(), parts.next(), parts.next()) else { continue };
            let spawns = spawns
                .split(',')
                .filter(|s| !s.is_empty())
                .filter_map(|s| {
                    let f: Vec<&str> = s.split('/').collect();
                    (f.len() == 4).then(|| Spawner { category: c.to_string(), ty: f[0].to_string(), weight: f[1].parse().unwrap_or(0), min: f[2].parse().unwrap_or(1), max: f[3].parse().unwrap_or(1) })
                })
                .collect();
            v.push(Override { category: c.to_string(), piece: mode == "piece", spawns });
        }
        out.insert(id.to_string(), v);
    }
    out
}

/// A structure's spawn overrides by id (`minecraft:trial_chambers`).
pub fn overrides(structure: &str) -> &'static [Override] {
    STRUCTURES.get_or_init(|| parse_structures(STRUCTURES_TSV)).get(structure).map(|v| v.as_slice()).unwrap_or(&[])
}

/// A biome's row by id (`minecraft:plains`).
pub fn biome(name: &str) -> Option<&'static Biome> {
    TABLE.get_or_init(|| parse(TSV).unwrap_or_else(|e| panic!("terrain/biomes.tsv: {e}"))).get(name)
}

#[cfg(test)]
mod tests {
    #[test]
    fn plains_lists_its_monsters() {
        let p = super::biome("minecraft:plains").unwrap();
        let zombie = p.mobs("monster").find(|s| s.ty == "minecraft:zombie").unwrap();
        assert_eq!((zombie.weight, zombie.min, zombie.max), (90, 4, 4));
        assert!(super::biome("minecraft:river").unwrap().has_tag("minecraft:more_frequent_drowned_spawns"));
    }
}
