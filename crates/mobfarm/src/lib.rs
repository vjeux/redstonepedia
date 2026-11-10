//! Mob farms (the ironfarm track; brief `designs/briefs/farm-iron.md`, notes
//! `designs/circuits/mobfarm/tracks/ironfarm/NOTES.md`): the layout generators behind
//! `mctas mobfarm gen`, and the redsim judge's tools (`exit`, `spots`). The judge itself is the
//! circuit study `mobfarm` (crates/circuit/src/studies/mobfarm.rs).

pub mod bury;
pub mod carousel;
pub mod cli;
pub mod day1;
pub mod exact;
pub mod gate;
pub mod hold;
pub mod layouts;
pub mod parts;
pub mod spawnrules;
pub mod station;
pub mod assembly;
pub mod timeline;
pub mod trim;
pub mod validate;
pub mod weathercheck;
