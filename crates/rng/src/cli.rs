//! `mctas rng ...` (docs/tools.md#mctas-rng): the enchanting table's calculator and the
//! librarian's trades from the command line, and the runs on the real server that check them.

use crate::crack::{Observation, crack_xp_seed, filter};
use crate::data::Data;
use crate::player::{Goal, crack_player, next_seed_candidates, plan};
use crate::table::{format_ench, offers};
use crate::villager::{Generations, ObservedTrades, Want, plan_rerolls, sync, trade_set};
use crate::xoroshiro::SequenceSettings;
use crate::{game, parse_enchant, parse_xp_seed};
use anyhow::{Result, anyhow, bail};
use clap::Subcommand;

#[derive(Subcommand)]
pub enum RngCmd {
    /// What the table offers for an enchanting seed, an item and a bookshelf count (all 16 counts
    /// without --shelves).
    Offers {
        /// The enchanting seed: 8 hex digits (5F94CCE0) or a number.
        #[arg(long)]
        seed: String,
        #[arg(long, default_value = "diamond_sword")]
        item: String,
        #[arg(long)]
        shelves: Option<i32>,
    },
    /// Find the enchanting seed from readings of the table (every 2^32 seed, on every core).
    /// A reading: "shelves=15 costs=7,13,30", optionally "item=diamond_sword clues=unbreaking:1,smite:2,looting:2".
    Crack {
        #[arg(long = "obs", required = true)]
        obs: Vec<String>,
        /// Search only the 65,536 seeds that can follow this one (the seed after one enchant).
        #[arg(long)]
        after: Option<String>,
        #[arg(long, default_value_t = 0)]
        threads: usize,
    },
    /// The player's own generator from two enchanting seeds in a row.
    Player {
        #[arg(long)]
        xp1: String,
        #[arg(long)]
        xp2: String,
        /// Allow this many other draws of the player's generator between the two enchants.
        #[arg(long, default_value_t = 0)]
        max_gap: u32,
    },
    /// How many items to drop (then one throwaway enchant) for the enchantments you want.
    Plan {
        /// The player's generator (12 hex digits, from `mctas rng player`).
        #[arg(long)]
        player: String,
        #[arg(long, default_value = "diamond_sword")]
        item: String,
        /// An enchantment with its lowest level, e.g. sharpness:4 (repeat it).
        #[arg(long = "want")]
        want: Vec<String>,
        /// An enchantment the item must not get (repeat it).
        #[arg(long = "not")]
        not: Vec<String>,
        #[arg(long, default_value_t = 15)]
        max_shelves: i32,
        #[arg(long, default_value_t = 2048)]
        max_drops: u32,
        /// The player's level (a slot needs its cost, and its cost + 1 after the throwaway).
        #[arg(long, default_value_t = 999)]
        level: i32,
    },
    /// The trades of a profession's level, from the world seed (26.3's world random sequences).
    Trades {
        #[command(subcommand)]
        command: TradesCmd,
    },
    /// Runs on the real server: the data, and every answer above checked against the game.
    Game {
        #[command(subcommand)]
        command: GameCmd,
    },
}

#[derive(Subcommand)]
pub enum TradesCmd {
    /// The generations from the world's first.
    List {
        /// The number /seed prints (often negative).
        #[arg(long, allow_hyphen_values = true)]
        world_seed: i64,
        #[arg(long, default_value = "librarian")]
        profession: String,
        #[arg(long, default_value_t = 1)]
        level: i32,
        #[arg(long, default_value_t = 0)]
        from: u64,
        #[arg(long, default_value_t = 20)]
        count: u64,
    },
    /// Where a world is in its sequence, from fresh trade screens in the order they were seen:
    /// "book=mending:1 price=23", or "nobook", and "item=bookshelf" for another trade's result.
    Sync {
        #[arg(long, allow_hyphen_values = true)]
        world_seed: i64,
        #[arg(long, default_value = "librarian")]
        profession: String,
        #[arg(long, default_value_t = 1)]
        level: i32,
        #[arg(long = "seen", required = true)]
        seen: Vec<String>,
        #[arg(long, default_value_t = 200_000)]
        max: u64,
    },
    /// The rerolls after generation N (the last one seen) until a book.
    Plan {
        #[arg(long, allow_hyphen_values = true)]
        world_seed: i64,
        #[arg(long, default_value = "librarian")]
        profession: String,
        #[arg(long, default_value_t = 1)]
        level: i32,
        #[arg(long)]
        after: u64,
        /// e.g. mending, or sharpness:5
        #[arg(long)]
        want: String,
        #[arg(long)]
        max_price: Option<i32>,
        #[arg(long, default_value_t = 100_000)]
        max_rerolls: u64,
        /// Print every reroll on the way (the first 5 otherwise).
        #[arg(long)]
        verbose: bool,
    },
}

#[derive(Subcommand)]
pub enum GameCmd {
    /// Dump the game's data from the real server into crates/rng/data/mc-<version>.json.
    Datagen,
    /// Every offer of the real enchanting table (costs, hints, full lists) for N seeds, every
    /// enchantable item and every bookshelf count against ours.
    Table {
        #[arg(long, default_value_t = 200)]
        seeds: usize,
        /// Write the recording the tests replay (crates/rng/data/game-<version>-table.tsv).
        #[arg(long)]
        record: bool,
    },
    /// The calculator's whole procedure on mock players of the real server, item in hand.
    Enchant {
        #[arg(long, default_value_t = 7)]
        trials: usize,
        /// Read the hover hints too (the seeds are found from the level costs alone otherwise).
        #[arg(long)]
        clues: bool,
    },
    /// A villager made a librarian again and again, then rerolled by its own AI, against our
    /// generations from the world seed.
    Librarian {
        #[arg(long, default_value_t = game::WORLD_SEED, allow_hyphen_values = true)]
        world_seed: i64,
        #[arg(long, default_value_t = 300)]
        generations: usize,
        #[arg(long, default_value_t = 3)]
        ai_rerolls: usize,
        /// Write the recording the tests replay (crates/rng/data/game-<version>-librarian-<seed>.tsv).
        #[arg(long)]
        record: bool,
    },
}

fn parse_obs(data: &Data, s: &str) -> Result<Observation> {
    let (mut shelves, mut costs, mut item, mut clues) = (None, None, None, [None; 3]);
    for part in s.split_whitespace() {
        let (k, v) = part.split_once('=').ok_or_else(|| anyhow!("expected key=value, got {part}"))?;
        match k {
            "shelves" => shelves = Some(v.parse::<i32>()?),
            "costs" => {
                let c: Vec<i32> = v.split(',').map(|x| x.parse::<i32>()).collect::<Result<_, _>>()?;
                let [a, b, c] = c[..] else { bail!("costs needs three numbers") };
                costs = Some([a, b, c]);
            }
            "item" => item = Some(data.item(v).ok_or_else(|| anyhow!("unknown item {v}"))?),
            "clues" => {
                for (i, c) in v.split(',').enumerate().take(3) {
                    if !matches!(c, "?" | "-" | "") {
                        let (e, l) = parse_enchant(data, c).map_err(|e| anyhow!(e))?;
                        clues[i] = Some((e, l as u8));
                    }
                }
            }
            _ => bail!("unknown key {k}"),
        }
    }
    Ok(Observation { shelves: shelves.ok_or_else(|| anyhow!("missing shelves="))?, costs: costs.ok_or_else(|| anyhow!("missing costs="))?, item, clues })
}

fn parse_seen(data: &Data, s: &str) -> Result<ObservedTrades> {
    let mut o = ObservedTrades::default();
    let (mut price, mut book) = (None, None);
    for part in s.split_whitespace() {
        if part == "nobook" {
            o.no_book = true;
            continue;
        }
        let (k, v) = part.split_once('=').ok_or_else(|| anyhow!("expected key=value, got {part}"))?;
        match k {
            "book" => book = Some(parse_enchant(data, v).map_err(|e| anyhow!(e))?),
            "price" => price = Some(v.parse::<i32>()?),
            "item" => o.items.push(if v.contains(':') { v.to_string() } else { format!("minecraft:{v}") }),
            _ => bail!("unknown key {k}"),
        }
    }
    if let Some((e, l)) = book {
        o.book = Some((e, l as u8, price));
    }
    Ok(o)
}

fn print_offers(data: &Data, seed: i32, item: usize, shelves: i32) {
    let o = offers(data, seed, &data.items[item], shelves);
    let mut line = format!("shelves {shelves:>2}:");
    for (i, s) in o.iter().enumerate() {
        if s.cost == 0 {
            line += &format!("  [{}] --", i + 1);
            continue;
        }
        let list: Vec<String> = s.enchants.iter().map(|&e| format_ench(data, e)).collect();
        let clue = s.clue.map(|c| format_ench(data, c)).unwrap_or_default();
        line += &format!("  [{}] {:>2} levels, hint {clue} => {}", i + 1, s.cost, list.join(", "));
    }
    println!("{line}");
}

pub fn run(version: &str, cmd: RngCmd) -> Result<()> {
    let data = Data::load();
    if version != data.version {
        bail!("the rng data is {}'s (crates/rng/data), not {version}'s", data.version);
    }
    match cmd {
        RngCmd::Offers { seed, item, shelves } => {
            let seed = parse_xp_seed(&seed).map_err(|e| anyhow!(e))?;
            let item = data.item(&item).ok_or_else(|| anyhow!("unknown item"))?;
            println!("enchanting seed {:08X} ({seed}), {}", seed as u32, data.items[item].id);
            match shelves {
                Some(b) => print_offers(data, seed, item, b),
                None => (0..=15).for_each(|b| print_offers(data, seed, item, b)),
            }
        }
        RngCmd::Crack { obs, after, threads } => {
            let observations = obs.iter().map(|o| parse_obs(data, o)).collect::<Result<Vec<_>>>()?;
            let t = std::time::Instant::now();
            let found = match after {
                Some(prev) => {
                    let mut c = next_seed_candidates(parse_xp_seed(&prev).map_err(|e| anyhow!(e))?);
                    c.sort_unstable();
                    c.dedup();
                    filter(data, &c, &observations)
                }
                None => crack_xp_seed(data, &observations, threads),
            };
            eprintln!("searched in {:.2?}", t.elapsed());
            println!("{} seed(s)", found.len());
            for s in found.iter().take(20) {
                println!("  {:08X} ({s})", *s as u32);
            }
        }
        RngCmd::Player { xp1, xp2, max_gap } => {
            let res = crack_player(parse_xp_seed(&xp1).map_err(|e| anyhow!(e))?, parse_xp_seed(&xp2).map_err(|e| anyhow!(e))?, max_gap);
            if res.is_empty() {
                println!("none: the two seeds are not from enchants in a row (something else drew from the player's generator between them?)");
            }
            for (state, gap) in res {
                println!("player seed {state:012X} (other draws between them: {gap})");
            }
        }
        RngCmd::Plan { player, item, want, not, max_shelves, max_drops, level } => {
            let state = u64::from_str_radix(player.trim_start_matches("0x"), 16)?;
            let item = data.item(&item).ok_or_else(|| anyhow!("unknown item"))?;
            let wanted = want.iter().map(|w| parse_enchant(data, w)).collect::<Result<Vec<_>, _>>().map_err(|e| anyhow!(e))?;
            let unwanted = not.iter().map(|w| parse_enchant(data, w).map(|(e, _)| e)).collect::<Result<Vec<_>, _>>().map_err(|e| anyhow!(e))?;
            let goal = Goal { item, wanted, unwanted, max_shelves, max_drops, player_level: level };
            match plan(data, state, &goal) {
                None => println!("impossible within {max_drops} drops"),
                Some(p) => {
                    match p.drops {
                        None => println!("No drops and no throwaway enchant: enchant now."),
                        Some(n) => println!("1. Drop {n} item(s) ({} stack(s) of 64 + {}).\n2. Enchant any cheap item once (throwaway).", n / 64, n % 64),
                    }
                    println!("{}. Set {} bookshelves, put in the {}, click slot {} ({} levels).", if p.drops.is_some() { 3 } else { 1 }, p.shelves, data.items[item].name, p.slot + 1, p.cost);
                    let list: Vec<String> = p.enchants.iter().map(|&e| format_ench(data, e)).collect();
                    println!("   You get: {}", list.join(", "));
                    println!("   Enchanting seed used: {:08X}; player seed afterwards: {:012X}", p.xp_seed as u32, p.state_after);
                }
            }
        }
        RngCmd::Trades { command } => match command {
            TradesCmd::List { world_seed, profession, level, from, count } => {
                let set = trade_set(data, &profession, level).map_err(|e| anyhow!(e))?;
                let mut g = Generations::new(data, set, world_seed, SequenceSettings::default()).map_err(|e| anyhow!(e))?;
                while g.index < from {
                    g.next_generation().map_err(|e| anyhow!(e))?;
                }
                for _ in 0..count {
                    let i = g.index;
                    let offers = g.next_generation().map_err(|e| anyhow!(e))?;
                    println!("{i:>6}: {}", offers.iter().map(|o| o.describe(data)).collect::<Vec<_>>().join("  |  "));
                }
            }
            TradesCmd::Sync { world_seed, profession, level, seen, max } => {
                let set = trade_set(data, &profession, level).map_err(|e| anyhow!(e))?;
                let obs = seen.iter().map(|s| parse_seen(data, s)).collect::<Result<Vec<_>>>()?;
                let hits = sync(data, set, world_seed, SequenceSettings::default(), &obs, max).map_err(|e| anyhow!(e))?;
                println!("{} match(es) in the first {max} generations of {}", hits.len(), set.random_sequence.as_deref().unwrap_or("?"));
                for h in hits.iter().take(20) {
                    println!("  the first screen was generation {h}; the last one listed is {}", h + seen.len() as u64 - 1);
                }
            }
            TradesCmd::Plan { world_seed, profession, level, after, want, max_price, max_rerolls, verbose } => {
                let set = trade_set(data, &profession, level).map_err(|e| anyhow!(e))?;
                let (e, l) = parse_enchant(data, &want).map_err(|e| anyhow!(e))?;
                let w = Want { enchant: e, min_level: l as u8, max_price };
                match plan_rerolls(data, set, world_seed, SequenceSettings::default(), after, &w, max_rerolls).map_err(|e| anyhow!(e))? {
                    None => println!("not within {max_rerolls} rerolls"),
                    Some((n, path)) => {
                        println!("Reroll {n} time(s). The trades on reroll {n}:");
                        for o in path.last().into_iter().flatten() {
                            println!("    {}", o.describe(data));
                        }
                        let show = if verbose { path.len() } else { path.len().min(5) };
                        println!("Check you stay in step (the first {show} rerolls):");
                        for (i, offers) in path.iter().take(show).enumerate() {
                            println!("  reroll {:>4}: {}", i + 1, offers.iter().map(|o| o.describe(data)).collect::<Vec<_>>().join("  |  "));
                        }
                    }
                }
            }
        },
        RngCmd::Game { command } => {
            let ok = match command {
                GameCmd::Datagen => {
                    game::datagen(version)?;
                    true
                }
                GameCmd::Table { seeds, record } => game::table(version, seeds, record)?,
                GameCmd::Enchant { trials, clues } => game::enchant(version, trials, clues)?,
                GameCmd::Librarian { world_seed, generations, ai_rerolls, record } => game::librarian(version, world_seed, generations, ai_rerolls, record)?,
            };
            if !ok {
                bail!("the game and ours differ (above)");
            }
            println!("PASS");
        }
    }
    Ok(())
}
