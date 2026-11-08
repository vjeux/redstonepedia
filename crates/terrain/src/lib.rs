//! Real terrain for the judges (the terrain lane; `designs/briefs/terrain-and-spawning.md`): the
//! official 26.3 server generates an area of a seed's world once, offline (`gen`), and the saved
//! chunks are read back (`region`) into an `.mcterrain` file (`redsim::terrain::format`), cached
//! under `$MCTAS_HOME/terrain/`; crops of it are committed under `designs/terrain/`.

pub mod blockinfo;
pub mod cli;
pub mod map;
pub mod pastecheck;
pub mod region;
pub mod worldgen;

/// Where generated areas are cached (`$MCTAS_HOME/terrain`).
pub fn cache_dir() -> std::path::PathBuf {
    mctas::paths::home().join("terrain")
}
