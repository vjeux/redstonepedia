//! Terrain (the terrain lane, `designs/briefs/terrain-and-spawning.md`): ground the official 26.3
//! server generated once, offline (`mctas terrain gen`), as redsim worlds.
//!
//! * `format.rs`: the `.mcterrain` file (blocks, biomes, heightmaps, saved light, structures,
//!   entities), pure Rust so the viewer decodes it in wasm.

pub mod biome;
pub mod blocks;
pub mod format;
pub mod land;
pub mod light;
pub mod place;

pub use format::{Heightmap, Terrain};
pub use land::{Land, build_world};
