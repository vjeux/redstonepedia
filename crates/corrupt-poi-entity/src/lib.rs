//! POI & entity-store desynchronization audit, lane 4 of 5 (designs/corrupt-poi-entity/NOTES.md).
//! Deterministic scenarios on the UNMODIFIED official 26.3 server: commands only ever set the
//! scene and read markers back; the desync step itself is a survival action (the bot breaks a
//! block, a piston pushes a workstation, a villager walks and claims on its own brain).

pub mod cli;
pub mod scenarios;
