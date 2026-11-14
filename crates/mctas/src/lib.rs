//! `mctas`: run the real Minecraft game headless, many simulations per JVM, one tick at a time.
//!
//! * [`setup`] downloads a Java runtime, the Minecraft server and its libraries.
//! * [`source`] decompiles the game (it ships unobfuscated since 26.1) so mechanics can be read.
//! * [`bridge`] compiles the small Java program that drives the server (see `bridge/` in the repo).
//! * [`sim`] spawns bridge workers and talks to them: sandboxes, blocks, ticks, regions.
//! * [`design`] is the text format contraptions are stored in.

pub mod blockstate;
pub mod bridge;
pub mod buildinputs;
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

/// A block cell's centre coordinate, `v + 0.5`, as a number: the one helper every crate prints a
/// cell's centre with (ironspec's ruling, Nov 12). Never write the integer followed by a literal
/// dot-5 suffix: for a negative `v` that text reads `v − 0.5`, the centre of the next cell down
/// (cell −3's centre is −2.5; the text "-3.5" is cell −4's). Found by tick-exact (a2819a2ab) in
/// summon and entity lines.
pub fn cell_centre(v: i32) -> f64 {
    v as f64 + 0.5
}

#[cfg(test)]
mod tests {
    use super::cell_centre;

    #[test]
    fn a_cells_centre_is_half_a_block_up_from_its_corner_for_negative_cells_too() {
        assert_eq!(cell_centre(3), 3.5);
        assert_eq!(cell_centre(0), 0.5);
        assert_eq!(cell_centre(-1), -0.5);
        assert_eq!(cell_centre(-3), -2.5);
        // as printed in a summon or entity line
        assert_eq!(format!("{} 64 {}", cell_centre(-5), cell_centre(-2)), "-4.5 64 -1.5");
        assert_eq!(format!("{}", cell_centre(12)), "12.5");
    }
}
