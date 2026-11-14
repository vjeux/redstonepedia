//! This crate's commands, one module per family: each command's clap arguments (`<Name>Args`,
//! its `DoorCmd` variant holds them) and its body (`cmd_<name>`, which `crate::run` calls).

pub mod boards;
pub mod checks;
pub mod design;
pub mod properties;
pub mod verify;
pub mod review;
