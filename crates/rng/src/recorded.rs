//! The game answers recorded from the real 26.3 server (`mctas rng game table --record`,
//! `mctas rng game librarian --record`) and the canonical text both sides write them in, so this
//! crate's tests replay them without a server: `data/game-<version>-table.tsv` (every offer of the
//! enchanting table for 200 seeds, 77 items and 16 bookshelf counts, as a digest plus every
//! 1,000th row in full) and `data/game-<version>-librarian-<world seed>.tsv` (the first
//! generations of the librarian's level-1 trades in a fresh world, and the sequence's state after
//! them).

use crate::data::Data;
use crate::java_random::JavaRandom;
use crate::table::{Ench, SlotOffer};
use crate::villager::Offer;

/// The seeds the table's recording covers: the edge values, then a fixed stream.
pub fn table_seeds(n: usize) -> Vec<i32> {
    let mut v = vec![0, -1, 1, i32::MIN, i32::MAX, i32::MAX - 1, i32::MAX - 2, i32::MIN + 1];
    let mut r = JavaRandom::new(0x5EED);
    while v.len() < n {
        v.push(r.next_int());
    }
    v.truncate(n);
    v
}

/// The items the table's recording covers: every item the table enchants, in the data's order.
pub fn table_items(data: &Data) -> Vec<usize> {
    (0..data.items.len()).filter(|&i| data.items[i].is_enchantable).collect()
}

fn ench_text(data: &Data, e: Ench) -> String {
    format!("{}:{}", data.enchants[e.0 as usize].name, e.1)
}

/// One row of the table's recording: `seed item shelves costs hints lists` (tab-separated; the
/// slots of hints and lists split by `|`, an enchantment `name:level`, `-` for none).
pub fn table_row(data: &Data, seed: i32, item: usize, shelves: i32, slots: &[SlotOffer; 3]) -> String {
    let costs: Vec<String> = slots.iter().map(|s| s.cost.to_string()).collect();
    let clues: Vec<String> = slots.iter().map(|s| s.clue.map(|c| ench_text(data, c)).unwrap_or_else(|| "-".into())).collect();
    let lists: Vec<String> = slots
        .iter()
        .map(|s| if s.enchants.is_empty() { "-".to_string() } else { s.enchants.iter().map(|&e| ench_text(data, e)).collect::<Vec<_>>().join(",") })
        .collect();
    format!("{seed}\t{}\t{shelves}\t{}\t{}\t{}", data.items[item].name, costs.join(","), clues.join("|"), lists.join("|"))
}

/// One generation of trades as one line: each offer `cost_a [+ cost_b] -> result [book] xUSES
/// xpXP`, joined by ` | `.
pub fn offers_line(data: &Data, offers: &[Offer]) -> String {
    offers
        .iter()
        .map(|o| {
            let item = |(id, n): &(String, i32)| format!("{n} {}", id.strip_prefix("minecraft:").unwrap_or(id));
            let mut s = item(&o.cost_a);
            if let Some(b) = &o.cost_b {
                s += &format!(" + {}", item(b));
            }
            s += &format!(" -> {}", item(&o.result));
            if let Some(b) = o.book {
                s += &format!(" {}", ench_text(data, b));
            }
            s += &format!(" x{} xp{}", o.max_uses, o.xp);
            s
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// A recording's header fields (`key<TAB>value` lines before the rows; `#` lines are notes).
pub fn header<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('\t')))
}

/// A recording's rows: the lines that are neither notes nor header fields (`fields` names them).
pub fn rows<'a>(text: &'a str, fields: &[&str]) -> Vec<&'a str> {
    text.lines().filter(|l| !l.starts_with('#') && !l.is_empty() && !fields.iter().any(|f| l.split('\t').next() == Some(f))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::offers;
    use crate::villager::{Generations, trade_set};
    use crate::xoroshiro::SequenceSettings;
    use sha2::{Digest, Sha256};

    const TABLE: &str = include_str!("../data/game-26.3-table.tsv");
    const LIBRARIANS: [&str; 2] = [include_str!("../data/game-26.3-librarian-8675309.tsv"), include_str!("../data/game-26.3-librarian--5584399522669591931.tsv")];

    #[test]
    fn the_tables_offers_replay_the_game() {
        let data = Data::load();
        let n: usize = header(TABLE, "seeds").and_then(|v| v.parse().ok()).expect("seeds");
        let want_rows: usize = header(TABLE, "rows").and_then(|v| v.parse().ok()).expect("rows");
        let want_sha = header(TABLE, "sha256").expect("sha256");
        let samples: std::collections::BTreeMap<(String, String, String), &str> = rows(TABLE, &["seeds", "rows", "sha256"])
            .into_iter()
            .map(|r| {
                let mut c = r.split('\t');
                ((c.next().unwrap_or("").into(), c.next().unwrap_or("").into(), c.next().unwrap_or("").into()), r)
            })
            .collect();
        let mut sha = Sha256::new();
        let mut count = 0;
        let mut first_bad: Option<(String, String)> = None;
        for seed in table_seeds(n) {
            for item in table_items(data) {
                for shelves in 0..16 {
                    let row = table_row(data, seed, item, shelves, &offers(data, seed, &data.items[item], shelves));
                    sha.update(row.as_bytes());
                    sha.update(b"\n");
                    count += 1;
                    let key = (seed.to_string(), data.items[item].name.clone(), shelves.to_string());
                    if let Some(want) = samples.get(&key)
                        && *want != row
                        && first_bad.is_none()
                    {
                        first_bad = Some((want.to_string(), row.clone()));
                    }
                }
            }
        }
        assert_eq!(first_bad, None, "a recorded row differs (game, then ours)");
        assert_eq!(count, want_rows);
        let got: String = sha.finalize().iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(got, want_sha, "the offers of {want_rows} rows differ from the game's somewhere (its sample rows all agree)");
        assert!(samples.len() >= 200, "the recording keeps every 1,000th row");
    }

    #[test]
    fn the_librarians_trades_replay_the_game() {
        let data = Data::load();
        for text in LIBRARIANS {
            let seed: i64 = header(text, "world_seed").and_then(|v| v.parse().ok()).expect("world_seed");
            let set = trade_set(data, "librarian", 1).expect("librarian level 1");
            assert_eq!(header(text, "sequence"), set.random_sequence.as_deref());
            let mut g = Generations::new(data, set, seed, SequenceSettings::default()).expect("sequence");
            let lines = rows(text, &["world_seed", "sequence", "generations", "state_after"]);
            assert_eq!(header(text, "generations").and_then(|v| v.parse::<usize>().ok()), Some(lines.len()));
            for line in lines {
                let (i, want) = line.split_once('\t').expect("index<TAB>offers");
                assert_eq!(i.parse::<u64>().ok(), Some(g.index));
                let got = offers_line(data, &g.next_generation().expect("generation"));
                assert_eq!(got, want, "world {seed}, generation {i}");
            }
            let s = g.state();
            assert_eq!(header(text, "state_after"), Some(format!("{} {}", s.lo as i64, s.hi as i64).as_str()), "world {seed}: the sequence's state after the recorded generations");
        }
    }
}
