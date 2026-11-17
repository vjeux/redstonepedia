//! The game's random number generators a player can steer, as Minecraft 26.3 runs them
//! (docs/mechanics.md, "The enchanting table and the player's random generator" and "Villager
//! trades: the world's random sequences"):
//!
//! - the enchanting table ([`table`]): its offers from the player's enchanting seed, the item and
//!   the bookshelves; the seed found from what the table shows ([`crack`]); the player's own
//!   generator found from two seeds in a row and steered by dropping items ([`player`]);
//! - the librarian's trades ([`villager`]): drawn from one world-wide random sequence per trade
//!   set ([`xoroshiro`]), seeded only by the world seed, so the n-th set of trades ever made in a
//!   world is fixed.
//!
//! The game's data ([`data`]) is dumped from the real server (`mctas rng game datagen`), and the
//! recorded game answers in `data/` are replayed by this crate's tests.

pub mod crack;
pub mod data;
pub mod java_random;
pub mod player;
pub mod recorded;
pub mod table;
pub mod villager;
pub mod xoroshiro;

#[cfg(feature = "cli")]
pub mod cli;
#[cfg(feature = "cli")]
mod game;

pub use data::Data;

/// Parse "sharpness", "sharpness:4", "sharpness 4", "sharpness iv" into (enchantment, level).
pub fn parse_enchant(data: &Data, s: &str) -> Result<(data::EnchIdx, i32), String> {
    let s = s.trim().to_ascii_lowercase();
    let (name, level) = match s.rsplit_once([':', ' ', '=']) {
        Some((n, l)) if parse_level(l).is_some() => (n.trim().to_string(), parse_level(l).unwrap_or(1)),
        _ => (s.clone(), 1),
    };
    let name = name.replace(' ', "_");
    let e = data.enchant(&name).ok_or_else(|| format!("unknown enchantment '{name}'"))?;
    Ok((e, level))
}

fn parse_level(s: &str) -> Option<i32> {
    let s = s.trim();
    if let Ok(n) = s.parse::<i32>() {
        return Some(n);
    }
    match s {
        "i" => Some(1),
        "ii" => Some(2),
        "iii" => Some(3),
        "iv" => Some(4),
        "v" => Some(5),
        _ => None,
    }
}

/// Parse an enchanting seed written as 8 hex digits (`5F94CCE0`), `0x...`, or a decimal int.
pub fn parse_xp_seed(s: &str) -> Result<i32, String> {
    let t = s.trim();
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u32::from_str_radix(h, 16).map(|v| v as i32).map_err(|e| e.to_string());
    }
    if t.len() == 8 && t.chars().all(|c| c.is_ascii_hexdigit()) && t.chars().any(|c| c.is_ascii_alphabetic()) {
        return u32::from_str_radix(t, 16).map(|v| v as i32).map_err(|e| e.to_string());
    }
    t.parse::<i64>().map(|v| v as i32).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_and_seeds() {
        let d = Data::load();
        assert_eq!(parse_enchant(d, "sharpness:4").map(|(_, l)| l), Ok(4));
        assert_eq!(parse_enchant(d, "fire aspect ii").map(|(e, l)| (d.enchants[e as usize].name.clone(), l)), Ok(("fire_aspect".to_string(), 2)));
        assert!(parse_enchant(d, "sharpnes").is_err());
        assert_eq!(parse_xp_seed("5F94CCE0"), Ok(0x5F94CCE0u32 as i32));
        assert_eq!(parse_xp_seed("-1"), Ok(-1));
        assert_eq!(parse_xp_seed("0xBE8A2899"), Ok(0xBE8A2899u32 as i32));
    }
}
