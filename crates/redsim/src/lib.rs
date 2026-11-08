//! `redsim`: an exact, tick-level Rust model of Minecraft 26.3's redstone and engine queues
//! (neighbor updates, shape updates, scheduled ticks, block events, the tick's phases), checked
//! cell by cell against the real game (the bridge sandbox) by differential fuzzing.
//!
//! Ownership: agent redmodel owns the engine and the redstone components; agent pistmodel owns
//! `piston/` (pistons, heads, moving blocks, the structure resolver) and `gravity.rs`.
//! The model, component by component with source references: `docs/mechanics.md`.

pub mod behavior;
pub mod bamboo;
pub mod block;
pub mod classes;
pub mod copper;
pub mod entity;
pub mod exact;
pub mod fluid;
pub mod gravity;
pub mod kelp;
pub mod links;
pub mod piston;
pub mod pos;
pub mod daylight;
pub mod explosion;
pub mod fence;
pub mod crops;
pub mod stems;
pub mod cactus;
pub mod loot;
pub mod loot_gen;
pub mod rails;
pub mod react;
pub mod redstone;
pub mod rng;
pub mod robust;
pub mod sculk;
pub mod slab;
pub mod snapshot;
pub mod terrain;
pub mod ticks;
pub mod tnt;
pub mod weather;
pub mod world;

pub use block::{Block, Kind};
pub use pos::{BlockPos, Dir};
pub use world::World;
