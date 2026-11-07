//! Block-entity desynchronization audit (lane 3 of the corruption audits,
//! `designs/briefs/corrupt-block-entity.md`): scripted scenarios on the UNMODIFIED official
//! server — RCON commands, a bot player clicking with real packets, frozen ticks stepped one
//! at a time, block states read back from the saved region files, block-entity tags read with
//! `data get block`, and an optional full server restart on the same world to prove what
//! survives a save/load.

pub mod cli;
pub mod vanilla;
