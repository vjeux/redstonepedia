//! Predicting `PortalForcer.createPortal` (26.3): given the destination dimension's blocks and
//! the exit point, which frame the game would build for an X-axis portal trip. A faithful copy
//! of the search (pocket scan per column of the 33x33 window, `canHostFrame` at offsets -1/0/+1,
//! closest FULL site by squared distance, partial only when no full site exists), over block
//! states read from the saved region files — the video driver uses it to place exactly the
//! cobblestone blockers that make the wanted bedrock row the winner, and checks every trip with
//! RCON afterwards anyway.

use mctas::sim::Region;
use std::collections::HashMap;

pub type Pos = [i32; 3];

/// How a block state reads to `PortalForcer`: replaceable with no fluid
/// (`canPortalReplaceBlock`), `isSolid()` (a valid frame bottom), or neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Replaceable,
    Solid,
    Other,
}

pub fn kind(state: &str) -> Kind {
    let name = state.split('[').next().unwrap_or(state);
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    match name {
        "air" | "cave_air" | "void_air" | "fire" | "soul_fire" | "short_grass" | "fern" | "tall_grass"
        | "large_fern" | "dead_bush" | "vine" | "glow_lichen" | "crimson_roots" | "warped_roots"
        | "nether_sprouts" | "crimson_fungus" | "warped_fungus" | "weeping_vines" | "weeping_vines_plant"
        | "twisting_vines" | "twisting_vines_plant" | "structure_void" | "seagrass" | "tall_seagrass" => Kind::Replaceable,
        // Fluids (not replaceable for the forcer, not solid) and the portal itself.
        "water" | "lava" | "nether_portal" | "end_portal" => Kind::Other,
        _ => Kind::Solid,
    }
}

/// Blocks of a box, with a writable overlay (planned blockers / conversions).
pub struct Blocks {
    region: Region,
    overlay: HashMap<Pos, String>,
}

impl Blocks {
    pub fn new(region: Region) -> Blocks {
        Blocks { region, overlay: HashMap::new() }
    }

    pub fn get(&self, p: Pos) -> &str {
        if let Some(s) = self.overlay.get(&p) {
            return s;
        }
        self.region.get(p).unwrap_or("minecraft:air")
    }

    pub fn kind(&self, p: Pos) -> Kind {
        kind(self.get(p))
    }

    pub fn set(&mut self, p: Pos, state: &str) {
        self.overlay.insert(p, state.to_string());
    }
}

/// One candidate frame site: `origin` is the interior's bottom-west cell (the game's
/// `closestFullPosition`); the converted (obsidian-overwritten) bedrock row is
/// `[origin.x - 1 ..= origin.x + 2, origin.y - 1, origin.z]`.
#[derive(Clone, Copy, Debug)]
pub struct Site {
    pub origin: Pos,
    pub full: bool,
    pub dist_sq: i64,
}

fn can_host_frame(b: &Blocks, origin: Pos, offset: i32) -> bool {
    // X axis: direction EAST (width along x), clockwise SOUTH (rows along z).
    for width in -1..3 {
        for height in -1..4 {
            let p = [origin[0] + width, origin[1] + height, origin[2] + offset];
            match (height < 0, b.kind(p)) {
                (true, Kind::Solid) => {}
                (true, _) => return false,
                (false, Kind::Replaceable) => {}
                (false, _) => return false,
            }
        }
    }
    true
}

/// Every site the forcer would consider for an X-axis trip exiting at `exit` (the destination's
/// min Y is `min_y`, its placeable top `max_place_y`: 0 / 127 in the Nether), closest first.
pub fn sites(b: &Blocks, exit: Pos, min_y: i32, max_place_y: i32) -> Vec<Site> {
    let mut out = Vec::new();
    for cx in exit[0] - 16..=exit[0] + 16 {
        for cz in exit[2] - 16..=exit[2] + 16 {
            let mut y = max_place_y;
            while y >= min_y {
                if b.kind([cx, y, cz]) == Kind::Replaceable {
                    let first_empty = y;
                    while y > min_y && b.kind([cx, y - 1, cz]) == Kind::Replaceable {
                        y -= 1;
                    }
                    let delta = first_empty - y;
                    if y + 4 <= max_place_y && (delta <= 0 || delta >= 3) {
                        let origin = [cx, y, cz];
                        if can_host_frame(b, origin, 0) {
                            let full = can_host_frame(b, origin, -1) && can_host_frame(b, origin, 1);
                            let d = [cx - exit[0], y - exit[1], cz - exit[2]];
                            out.push(Site { origin, full, dist_sq: (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) as i64 });
                        }
                    }
                }
                y -= 1;
            }
        }
    }
    // Full sites first, then by distance: the game's winner is element 0 of the full group (or
    // of the partial group when there is no full site); ties beyond that follow the spiral,
    // which the driver avoids by demanding a unique winner.
    out.sort_by_key(|s| (!s.full, s.dist_sq));
    out
}

/// The frame the game will build: the closest full site, else the closest partial, else the
/// clamped fallback platform (not modelled: `None`).
pub fn winner(b: &Blocks, exit: Pos, min_y: i32, max_place_y: i32) -> Option<Site> {
    sites(b, exit, min_y, max_place_y).into_iter().next()
}

/// The 4 bottom cells a site overwrites with obsidian.
pub fn converted_cells(site: &Site) -> [Pos; 4] {
    let o = site.origin;
    [[o[0] - 1, o[1] - 1, o[2]], [o[0], o[1] - 1, o[2]], [o[0] + 1, o[1] - 1, o[2]], [o[0] + 2, o[1] - 1, o[2]]]
}

/// Cells whose air-ness a full site depends on (all three rows) — a blocker in one of these (and
/// in no needed cell of the target site) kills the site.
pub fn air_cells(site: &Site) -> Vec<Pos> {
    let o = site.origin;
    let mut v = Vec::new();
    for offset in -1..2 {
        for width in -1..3 {
            for height in 0..4 {
                v.push([o[0] + width, o[1] + height, o[2] + offset]);
            }
        }
    }
    v
}
