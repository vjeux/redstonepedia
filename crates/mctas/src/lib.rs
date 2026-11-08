//! `mctas`: run the real Minecraft game headless, many simulations per JVM, one tick at a time.
//!
//! * [`setup`] downloads a Java runtime, the Minecraft server and its libraries.
//! * [`source`] decompiles the game (it ships unobfuscated since 26.1) so mechanics can be read.
//! * [`bridge`] compiles the small Java program that drives the server (see `bridge/` in the repo).
//! * [`sim`] spawns bridge workers and talks to them: sandboxes, blocks, ticks, regions.
//! * [`design`] is the text format contraptions are stored in.

pub mod blockstate;
pub mod bridge;
pub mod design;
pub mod fabric;
pub mod mcfont;
pub mod net;
pub mod paths;
pub mod recorded;
pub mod setup;
pub mod sim;
pub mod source;
pub mod vanilla;
pub mod video;

/// The Minecraft version everything defaults to.
pub const DEFAULT_VERSION: &str = "26.3";
