//! Mob farms (the ironfarm track; brief `designs/briefs/farm-iron.md`, notes
//! `designs/circuits/mobfarm/tracks/ironfarm/NOTES.md`): the layout generators behind
//! `mctas mobfarm gen`, and the redsim judge's tools (`exit`, `spots`). The judge itself is the
//! circuit study `mobfarm` (crates/circuit/src/studies/mobfarm.rs).

pub mod cli;
pub mod day1;
pub mod layouts;
pub mod validate;
