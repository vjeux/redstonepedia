//! Natural mobs (the terrain track, 81eaf43b): the rows of the natural mob types
//! ([`super::types::MobType::Natural`]), by index; NaturalSpawner, the spawn rules and despawning
//! follow here. This file is the terrain track's: ironfarm created it empty so the type registry
//! compiles (`MobType::def` and `from_id` read [`DEFS`] after the modeled types).

use super::types::TypeDef;

/// The natural mob types' rows (`DEFS[i].ty == MobType::Natural(i)`).
pub static DEFS: &[TypeDef] = &[];
