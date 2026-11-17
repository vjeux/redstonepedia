//! The runs on the real 26.3 server (`mctas rng game ...`): the unmodified dedicated server
//! started in one JVM by the rng bridge (bridge/rng: mock players ticked like network clients,
//! villagers on their own AI), driven from here. The bridge only acts and reads; every decision
//! and every comparison is made here.

use crate::crack::{Observation, crack_xp_seed, filter};
use crate::data::Data;
use crate::java_random::JavaRandom;
use crate::player::{Goal, crack_player, next_seed_candidates, plan};
use crate::recorded::{offers_line, table_items, table_row, table_seeds};
use crate::table::{SlotOffer, format_ench, offers};
use crate::villager::{Generations, ObservedTrades, Offer, observation_matches, sync, trade_set};
use crate::xoroshiro::SequenceSettings;
use anyhow::{Context, Result, bail};
use mctas::setup::{self, ServerInstall};
use mctas::{net, paths};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const BRIDGE_SOURCE: (&str, &str) = ("dev/vjeux/mctas/rng/RngBridge.java", include_str!("../../../bridge/rng/dev/vjeux/mctas/rng/RngBridge.java"));
const MAIN_CLASS: &str = "dev.vjeux.mctas.rng.RngBridge";
/// The world seed of the bridge's world unless a run names another.
pub const WORLD_SEED: i64 = 8675309;

/// The bridge compiled against `version`'s server (cached by a hash of its source).
fn compile(version: &str) -> Result<(ServerInstall, PathBuf)> {
    let install = setup::install_server(version)?;
    let (name, text) = BRIDGE_SOURCE;
    let hash = &net::sha256_hex(format!("{version}{name}{text}").as_bytes())[..12];
    let dir = paths::version_dir(version).join(format!("rng-bridge-{hash}"));
    let classes = dir.join("classes");
    if dir.join(".built").exists() {
        return Ok((install, classes));
    }
    let tmp = paths::version_dir(version).join(format!("rng-bridge-{hash}.tmp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let src = tmp.join("src").join(name);
    std::fs::create_dir_all(src.parent().unwrap_or(&tmp))?;
    std::fs::write(&src, text)?;
    std::fs::create_dir_all(tmp.join("classes"))?;
    let sep = if cfg!(windows) { ";" } else { ":" };
    let cp = format!("{}{sep}{}", install.server_jar.display(), install.libs_dir.join("*").display());
    let javac = setup::javac_for(&install.java);
    let major = setup::required_java(version)?;
    let out = Command::new(&javac)
        .args(["--release", &major.to_string(), "-nowarn", "-encoding", "UTF-8", "-cp", &cp, "-d"])
        .arg(tmp.join("classes"))
        .arg(&src)
        .output()
        .with_context(|| format!("running {}", javac.display()))?;
    if !out.status.success() {
        let _ = std::fs::remove_dir_all(&tmp);
        bail!("javac (the rng bridge) failed:\n{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    }
    std::fs::write(tmp.join(".built"), b"ok")?;
    if std::fs::rename(&tmp, &dir).is_err() {
        let _ = std::fs::remove_dir_all(&tmp);
        if !dir.join(".built").exists() {
            bail!("could not move the rng bridge into {}", dir.display());
        }
    }
    Ok((install, classes))
}

/// The server's settings for a bridge run: a fresh superflat world with no mobs of its own, never
/// paused (the bridge's players join only after the start), on a free local port (runs side by
/// side), nothing else listening.
fn server_properties(world_seed: i64) -> Result<String> {
    let port = std::net::TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
    Ok(format!(
        "level-type=minecraft\\:flat\nlevel-seed={world_seed}\nlevel-name=world\nserver-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nspawn-protection=0\ngenerate-structures=false\nspawn-monsters=false\nspawn-animals=false\nenable-rcon=false\nenable-query=false\nsync-chunk-writes=false\nview-distance=4\nsimulation-distance=6\nmax-players=10\ndifficulty=normal\ngamemode=survival\npause-when-empty-seconds=0\n"
    ))
}

pub struct Bridge {
    child: Child,
    stdin: ChildStdin,
    out: BufReader<ChildStdout>,
    log: std::fs::File,
    pub ready: Value,
    pub dir: PathBuf,
}

impl Bridge {
    pub fn start(version: &str, name: &str, world_seed: i64) -> Result<Bridge> {
        let (install, classes) = compile(version)?;
        let dir = paths::run_dir().join(format!("rng-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("server.properties"), server_properties(world_seed)?)?;
        std::fs::write(dir.join("eula.txt"), "eula=true\n")?;
        let sep = if cfg!(windows) { ";" } else { ":" };
        let cp = [classes.display().to_string(), install.server_jar.display().to_string(), install.libs_dir.join("*").display().to_string()].join(sep);
        let mut child = Command::new(&install.java)
            .args(["-Xmx6g", "-XX:+UseParallelGC", "--enable-native-access=ALL-UNNAMED", "-cp", &cp, MAIN_CLASS])
            .current_dir(&dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(std::fs::File::create(dir.join("stderr.log"))?))
            .spawn()
            .with_context(|| format!("starting {}", install.java.display()))?;
        let stdin = child.stdin.take().context("the bridge's stdin")?;
        let out = BufReader::new(child.stdout.take().context("the bridge's stdout")?);
        let log = std::fs::File::create(dir.join("server.log"))?;
        let mut b = Bridge { child, stdin, out, log, ready: Value::Null, dir };
        b.ready = b.read_reply()?;
        eprintln!("server ready: {} ({})", b.ready, b.dir.display());
        Ok(b)
    }

    fn read_reply(&mut self) -> Result<Value> {
        let mut line = String::new();
        loop {
            line.clear();
            if self.out.read_line(&mut line)? == 0 {
                bail!("the bridge exited (its log: {})", self.dir.join("server.log").display());
            }
            if let Some(j) = line.strip_prefix("@@BRIDGE ") {
                let v: Value = serde_json::from_str(j)?;
                if let Some(err) = v.get("error") {
                    bail!("bridge error: {err}\n{}", v["trace"].as_str().unwrap_or(""));
                }
                return Ok(v);
            }
            self.log.write_all(line.as_bytes())?;
        }
    }

    pub fn call(&mut self, cmd: Value) -> Result<Value> {
        writeln!(self.stdin, "{cmd}")?;
        self.stdin.flush()?;
        self.read_reply()
    }

    pub fn quit(mut self) -> Result<()> {
        self.call(json!({"cmd": "quit"}))?;
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
        Ok(())
    }
}

fn ench_pairs(data: &Data, v: &Value) -> Vec<(u8, u8)> {
    v.as_array().map(|a| a.iter().filter_map(|p| Some((data.enchant(p[0].as_str()?)?, p[1].as_i64()? as u8))).collect()).unwrap_or_default()
}

fn data_dir() -> Result<PathBuf> {
    let d = PathBuf::from("crates/rng/data");
    if !d.is_dir() {
        bail!("run from the repository root (no crates/rng/data here)");
    }
    Ok(d)
}

/// `datagen`: every enchantment, enchantable item, enchantment tag and trade set from the real
/// server, and the trade sets' and villager trades' files from its jar, into
/// crates/rng/data/mc-<version>.json.
pub fn datagen(version: &str) -> Result<()> {
    let dir = data_dir()?;
    let mut b = Bridge::start(version, "datagen", WORLD_SEED)?;
    let mut dump = b.call(json!({"cmd": "dump"}))?;
    b.quit()?;
    let install = setup::install_server(version)?;
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&install.server_jar)?)?;
    let mut trade_sets = serde_json::Map::new();
    let mut trades = serde_json::Map::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        let name = f.name().to_string();
        let (map, rest) = if let Some(rest) = name.strip_prefix("data/minecraft/trade_set/") {
            (&mut trade_sets, rest)
        } else if let Some(rest) = name.strip_prefix("data/minecraft/villager_trade/") {
            (&mut trades, rest)
        } else {
            continue;
        };
        if let Some(id) = rest.strip_suffix(".json") {
            let mut s = String::new();
            f.read_to_string(&mut s)?;
            map.insert(format!("minecraft:{id}"), serde_json::from_str(&s)?);
        }
    }
    eprintln!("{} trade sets, {} villager trades from the jar", trade_sets.len(), trades.len());
    dump["trade_set_files"] = Value::Object(trade_sets);
    dump["villager_trade_files"] = Value::Object(trades);
    let text = serde_json::to_string(&dump)?;
    let data = Data::parse(&text).map_err(|e| anyhow::anyhow!("the dumped data does not parse: {e}"))?;
    let path = dir.join(format!("mc-{version}.json"));
    std::fs::write(&path, &text)?;
    eprintln!(
        "wrote {}: {} enchantments, {} enchantable items, table pool {}, tradeable {}, {} trade sets, {} trades read, {} not read (their modifiers are not modelled)",
        path.display(),
        data.enchants.len(),
        data.items.len(),
        data.table_order.len(),
        data.tags.get("minecraft:tradeable").map(|t| t.len()).unwrap_or(0),
        data.trade_sets.len(),
        data.trades.len(),
        data.unsupported_trades.len()
    );
    for level in 1..=5 {
        let set = trade_set(&data, "librarian", level).map_err(anyhow::Error::msg)?;
        let missing: Vec<&String> = set.trades.iter().filter(|t| !data.trades.contains_key(*t)).collect();
        if !missing.is_empty() {
            bail!("{}: trades not read: {missing:?}", set.id);
        }
    }
    Ok(())
}

/// `table`: every offer (costs, hover hints, full lists) of the real EnchantmentMenu for `seeds`
/// seeds, every enchantable item and every bookshelf count against ours; `record` writes the
/// recording the tests replay (data/game-<version>-table.tsv).
pub fn table(version: &str, seeds: usize, record: bool) -> Result<bool> {
    use sha2::{Digest, Sha256};
    let data = Data::load();
    let mut b = Bridge::start(version, "table", WORLD_SEED)?;
    let items = table_items(data);
    let ids: Vec<&str> = items.iter().map(|&i| data.items[i].id.as_str()).collect();
    let shelves: Vec<i32> = (0..16).collect();
    let (mut count, mut bad) = (0usize, 0usize);
    let mut sha = Sha256::new();
    let mut samples: Vec<String> = Vec::new();
    for chunk in table_seeds(seeds).chunks(10) {
        let reply = b.call(json!({"cmd": "offers", "seeds": chunk, "items": ids, "shelves": shelves}))?;
        for row in reply["rows"].as_array().context("rows")? {
            let seed = row["seed"].as_i64().context("seed")? as i32;
            let item = data.item(row["item"].as_str().context("item")?).context("an item we do not know")?;
            let sh = row["shelves"].as_i64().context("shelves")? as i32;
            let game: [SlotOffer; 3] = std::array::from_fn(|s| SlotOffer {
                cost: row["costs"][s].as_i64().unwrap_or(-1) as i32,
                clue: if row["clues"][s].is_null() { None } else { ench_pairs(data, &json!([row["clues"][s]])).first().copied() },
                enchants: ench_pairs(data, &row["lists"][s]),
            });
            let line = table_row(data, seed, item, sh, &game);
            sha.update(line.as_bytes());
            sha.update(b"\n");
            if count % 1000 == 0 {
                samples.push(line.clone());
            }
            count += 1;
            let ours = table_row(data, seed, item, sh, &offers(data, seed, &data.items[item], sh));
            if ours != line {
                bad += 1;
                if bad <= 5 {
                    eprintln!("MISMATCH\n  game: {line}\n  ours: {ours}");
                }
            }
        }
    }
    b.quit()?;
    let digest: String = sha.finalize().iter().map(|x| format!("{x:02x}")).collect();
    println!("table: {count} (seed, item, bookshelves) offers from the real EnchantmentMenu ({seeds} seeds x {} items x 16), {bad} differ from ours", items.len());
    if record {
        let path = data_dir()?.join(format!("game-{version}-table.tsv"));
        let mut text = format!(
            "# The enchanting table's offers in the real {version} game (EnchantmentMenu: the costs, the hover hints and every slot's full\n# list), recorded by `mctas rng game table --record`; replayed by crates/rng's tests (src/recorded.rs).\n# A row: seed, item, bookshelves, costs, hints, lists (`name:level`, `-` for none). The digest covers every row in order.\nseeds\t{seeds}\nrows\t{count}\nsha256\t{digest}\n# every 1,000th row in full:\n"
        );
        for s in &samples {
            text.push_str(s);
            text.push('\n');
        }
        std::fs::write(&path, text)?;
        println!("wrote {}", path.display());
    }
    Ok(count > 0 && bad == 0)
}

/// Read the table at a bookshelf count as a player does.
fn look(b: &mut Bridge, data: &Data, name: &str, shelves: i32, item: &str, clues: bool) -> Result<Observation> {
    let r = b.call(json!({"cmd": "observe", "name": name, "shelves": shelves, "item": item}))?;
    if !r["rng_untouched"].as_bool().unwrap_or(false) {
        bail!("looking at the table moved the player's random generator");
    }
    let mut obs = Observation::costs_only(shelves, std::array::from_fn(|i| r["costs"][i].as_i64().unwrap_or(-1) as i32));
    if clues {
        obs.item = data.item(item);
        for i in 0..3 {
            if !r["clues"][i].is_null() {
                obs.clues[i] = ench_pairs(data, &json!([r["clues"][i]])).first().copied();
            }
        }
    }
    Ok(obs)
}

/// Find the current enchanting seed the way a player would: read the table at more bookshelf
/// counts until one seed is left (`after`: the previous seed, for the 65,536-seed search).
fn crack_current_seed(b: &mut Bridge, data: &Data, name: &str, after: Option<i32>, clues: bool) -> Result<Option<(i32, usize)>> {
    const ORDER: [i32; 16] = [15, 8, 3, 12, 1, 6, 10, 0, 14, 5, 2, 9, 4, 13, 7, 11];
    let mut observations = Vec::new();
    let mut candidates: Option<Vec<i32>> = after.map(|s| {
        let mut c = next_seed_candidates(s);
        c.sort_unstable();
        c.dedup();
        c
    });
    for &sh in &ORDER {
        observations.push(look(b, data, name, sh, "minecraft:diamond_sword", clues)?);
        candidates = Some(match candidates {
            None if observations.len() >= 2 || clues => crack_xp_seed(data, &observations, 0),
            None => continue,
            Some(c) => filter(data, &c, &observations),
        });
        match candidates.as_deref() {
            Some([one]) => return Ok(Some((*one, observations.len()))),
            Some([]) => return Ok(None),
            _ => {}
        }
    }
    Ok(None)
}

/// The goals the e2e trials ask for, one per trial in turn.
const GOALS: [(&str, &[&str]); 7] = [
    ("diamond_sword", &["sharpness:4", "looting:3", "fire_aspect:2"]),
    ("diamond_pickaxe", &["efficiency:4", "fortune:3", "unbreaking:3"]),
    ("diamond_chestplate", &["protection:4", "unbreaking:3"]),
    ("book", &["sharpness:4"]),
    ("diamond_boots", &["feather_falling:4", "unbreaking:3"]),
    ("bow", &["power:4", "infinity"]),
    ("iron_pickaxe", &["silk_touch", "efficiency:4", "unbreaking:3"]),
];

/// `enchant`: the calculator's whole procedure on mock players of the real server: enchant once,
/// read the table until one seed is left, enchant something cheap, read it again, find the
/// player's generator, plan, press Q that many times, enchant a book, set the bookshelves,
/// enchant; the item must carry exactly the planned enchantments and the player's generator be
/// where we said.
pub fn enchant(version: &str, trials: usize, clues: bool) -> Result<bool> {
    let data = Data::load();
    let mut b = Bridge::start(version, "enchant", WORLD_SEED)?;
    let mut passed = 0;
    for t in 0..trials {
        let name = format!("p{t}");
        let p0 = b.call(json!({"cmd": "player_new", "name": name}))?;
        let first = p0["rng_state"].as_u64().unwrap_or(0);
        b.call(json!({"cmd": "tick", "n": 20}))?;
        b.call(json!({"cmd": "enchant", "name": name, "shelves": 0, "item": "minecraft:wooden_sword", "slot": 0}))?;
        b.call(json!({"cmd": "tick", "n": 10}))?;
        let Some((xp1, looks1)) = crack_current_seed(&mut b, data, &name, None, clues)? else {
            println!("trial {t}: could not find the first seed");
            continue;
        };
        let real1 = b.call(json!({"cmd": "player_state", "name": name}))?["xp_seed"].as_i64().unwrap_or(0) as i32;
        b.call(json!({"cmd": "enchant", "name": name, "shelves": 0, "item": "minecraft:wooden_sword", "slot": 0}))?;
        b.call(json!({"cmd": "tick", "n": 10}))?;
        let Some((xp2, looks2)) = crack_current_seed(&mut b, data, &name, Some(xp1), clues)? else {
            println!("trial {t}: could not find the second seed");
            continue;
        };
        let found = crack_player(xp1, xp2, 0);
        let st = b.call(json!({"cmd": "player_state", "name": name}))?;
        let truth = st["rng_state"].as_u64().unwrap_or(0);
        let real2 = st["xp_seed"].as_i64().unwrap_or(0) as i32;
        if found.len() != 1 || found[0].0 != truth || xp1 != real1 || xp2 != real2 {
            println!("trial {t}: seeds {xp1:08X} (game {real1:08X}) {xp2:08X} (game {real2:08X}), player {found:?} (game {truth:012X})");
            continue;
        }
        let (item, wants) = GOALS[t % GOALS.len()];
        let goal = Goal {
            item: data.item(item).context("goal item")?,
            wanted: wants.iter().map(|w| crate::parse_enchant(data, w)).collect::<Result<Vec<_>, _>>().map_err(anyhow::Error::msg)?,
            unwanted: vec![],
            max_shelves: 15,
            max_drops: 20_000,
            player_level: 999,
        };
        let Some(p) = plan(data, found[0].0, &goal) else {
            println!("trial {t}: nothing within 20,000 drops for {item} {wants:?}");
            continue;
        };
        if let Some(n) = p.drops {
            let d = b.call(json!({"cmd": "drop", "name": name, "count": n}))?;
            let mut expect = JavaRandom::from_state(d["rng_before"].as_u64().unwrap_or(0));
            expect.advance(4 * n as u64);
            if expect.state != d["rng_after"].as_u64().unwrap_or(0) {
                bail!("trial {t}: {n} presses of Q were not {} calls of the player's generator", 4 * n);
            }
            b.call(json!({"cmd": "tick", "n": 5}))?;
            b.call(json!({"cmd": "enchant", "name": name, "shelves": 0, "item": "minecraft:book", "slot": 0}))?;
        }
        b.call(json!({"cmd": "tick", "n": 5}))?;
        let r = b.call(json!({"cmd": "enchant", "name": name, "shelves": p.shelves, "item": format!("minecraft:{item}"), "slot": p.slot}))?;
        let mut got = ench_pairs(data, &r["result"]);
        let after = r["player"]["rng_state"].as_u64().unwrap_or(0);
        let mut want = p.enchants.clone();
        let words = |l: &[(u8, u8)]| l.iter().map(|&e| format_ench(data, e)).collect::<Vec<_>>().join(", ");
        let (gw, ww) = (words(&got), words(&want));
        got.sort_unstable();
        want.sort_unstable();
        let same = got == want && after == p.state_after;
        println!(
            "trial {t}: player {first:012X}; seeds found after {looks1} + {looks2} readings; drop {:?}, {} bookshelves, slot {} -> planned [{ww}], the game gave [{gw}]{}",
            p.drops,
            p.shelves,
            p.slot + 1,
            if same { "" } else { "  <-- DIFFERS" }
        );
        if same {
            passed += 1;
        }
    }
    b.quit()?;
    println!("enchant: {passed}/{trials} trials got exactly the planned enchantments");
    Ok(passed == trials)
}

fn game_offers(data: &Data, v: &Value) -> Vec<Offer> {
    v["offers"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|o| {
                    let it = |x: &Value| (x["id"].as_str().unwrap_or("").to_string(), x["count"].as_i64().unwrap_or(0) as i32);
                    Offer {
                        trade: String::new(),
                        cost_a: it(&o["cost_a"]),
                        cost_b: if o["cost_b"].is_null() { None } else { Some(it(&o["cost_b"])) },
                        result: it(&o["result"]),
                        book: ench_pairs(data, &o["result"]["enchantments"]).first().copied(),
                        max_uses: o["max_uses"].as_i64().unwrap_or(0) as i32,
                        xp: o["xp"].as_i64().unwrap_or(0) as i32,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn wait_for(b: &mut Bridge, uuid: &str, max_ticks: i32, pred: impl Fn(&Value) -> bool) -> Result<Option<Value>> {
    let mut waited = 0;
    loop {
        let info = b.call(json!({"cmd": "villager_info", "uuid": uuid}))?;
        if pred(&info) {
            return Ok(Some(info));
        }
        if waited >= max_ticks {
            return Ok(None);
        }
        b.call(json!({"cmd": "tick", "n": 20}))?;
        waited += 20;
    }
}

/// `librarian`: a villager made a librarian again and again (each time its profession is
/// dropped and taken back, then its trade screen opened) against our generations from the world
/// seed alone, the sequence's state after them, a sync from two screens, then another villager
/// rerolled by its own AI (a lectern placed, removed, placed again); `record` writes the recording
/// the tests replay (data/game-<version>-librarian-<world seed>.tsv).
pub fn librarian(version: &str, world_seed: i64, generations: usize, ai_rerolls: usize, record: bool) -> Result<bool> {
    let data = Data::load();
    let mut b = Bridge::start(version, "librarian", world_seed)?;
    let seed = b.ready["seed"].as_i64().context("the world's seed")?;
    if seed != world_seed {
        bail!("the server's world seed is {seed}, not {world_seed}");
    }
    let set = trade_set(data, "librarian", 1).map_err(anyhow::Error::msg)?;
    let key = set.random_sequence.clone().context("the librarian's random sequence")?;
    let mut gens = Generations::new(data, set, seed, SequenceSettings::default()).map_err(anyhow::Error::msg)?;
    let mut ok = true;
    if b.call(json!({"cmd": "seq_state", "key": key}))?["exists"].as_bool() != Some(false) {
        bail!("{key} exists before any trade");
    }
    let v = b.call(json!({"cmd": "villager_spawn", "x": 40.5, "y": -60, "z": 40.5}))?["uuid"].as_str().unwrap_or("").to_string();
    let mut seen: Vec<Vec<Offer>> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let mut differ = 0;
    for i in 0..generations {
        let game = game_offers(data, &b.call(json!({"cmd": "villager_reroll", "uuid": v, "profession": "minecraft:librarian"}))?);
        let ours = gens.next_generation().map_err(anyhow::Error::msg)?;
        let (gl, ol) = (offers_line(data, &game), offers_line(data, &ours));
        if gl != ol {
            differ += 1;
            if differ <= 5 {
                eprintln!("generation {i} DIFFERS\n  game: {gl}\n  ours: {ol}");
            }
        }
        if i < 3 {
            println!("generation {i}: {gl}");
        }
        lines.push(format!("{i}\t{gl}"));
        seen.push(game);
    }
    let st = b.call(json!({"cmd": "seq_state", "key": key}))?;
    let ours = gens.state();
    let game_state = (st["lo"].as_i64().unwrap_or(0), st["hi"].as_i64().unwrap_or(0));
    let state_ok = game_state == (ours.lo as i64, ours.hi as i64);
    println!("librarian (world {seed}): {generations} generations, {differ} differ; the sequence's state after them {}", if state_ok { "is ours" } else { "DIFFERS" });
    ok &= differ == 0 && state_ok;
    if generations >= 4 {
        let mid = generations / 2;
        let obs: Vec<ObservedTrades> = [mid, mid + 1]
            .iter()
            .map(|&i| {
                let book = seen[i].iter().find_map(|o| o.book.map(|(e, l)| (e, l, Some(o.cost_a.1))));
                ObservedTrades { book, no_book: book.is_none(), items: seen[i].iter().filter(|o| o.book.is_none()).map(|o| o.result.0.clone()).collect() }
            })
            .collect();
        let hits = sync(data, set, seed, SequenceSettings::default(), &obs, 100_000).map_err(anyhow::Error::msg)?;
        let matched = observation_matches(&seen[mid], &obs[0]) && hits.first() == Some(&(mid as u64));
        println!("sync from screens {mid} and {}: first match {:?} of {}", mid + 1, hits.first(), hits.len());
        ok &= matched;
    }
    // the AI's own rerolls: the first villager leaves (it would compete for the lectern), the
    // server runs fast
    b.call(json!({"cmd": "villager_remove", "uuid": v}))?;
    b.call(json!({"cmd": "command", "command": "tick rate 400"}))?;
    let lectern = (60, -60, 40);
    let v2 = b.call(json!({"cmd": "villager_spawn", "x": 60.5, "y": -60, "z": 42.5}))?["uuid"].as_str().unwrap_or("").to_string();
    for r in 0..ai_rerolls {
        b.call(json!({"cmd": "set_block", "x": lectern.0, "y": lectern.1, "z": lectern.2, "block": "minecraft:lectern"}))?;
        if wait_for(&mut b, &v2, 6000, |i| i["profession"] == "minecraft:librarian")?.is_none() {
            println!("AI reroll {r}: the villager never took the lectern");
            ok = false;
            break;
        }
        let game = game_offers(data, &b.call(json!({"cmd": "villager_offers", "uuid": v2}))?);
        let ours = gens.next_generation().map_err(anyhow::Error::msg)?;
        let same = offers_line(data, &game) == offers_line(data, &ours);
        println!("AI reroll {r}: generation {}: {}{}", gens.index - 1, offers_line(data, &game), if same { "  = ours" } else { "  <-- DIFFERS" });
        ok &= same;
        b.call(json!({"cmd": "set_block", "x": lectern.0, "y": lectern.1, "z": lectern.2, "block": "minecraft:air"}))?;
        if wait_for(&mut b, &v2, 6000, |i| i["profession"] == "minecraft:none")?.is_none() {
            println!("AI reroll {r}: the villager kept its profession without the lectern");
            ok = false;
            break;
        }
    }
    b.quit()?;
    if record {
        let path = data_dir()?.join(format!("game-{version}-librarian-{seed}.tsv"));
        let mut text = format!(
            "# The librarian's level-1 trades in a fresh {version} world, one generation per line (a villager made a librarian again\n# and again, its trade screen opened each time), recorded by `mctas rng game librarian --record`; replayed by\n# crates/rng's tests (src/recorded.rs). state_after: the random sequence's two longs after these generations.\nworld_seed\t{seed}\nsequence\t{key}\ngenerations\t{generations}\nstate_after\t{} {}\n",
            game_state.0, game_state.1
        );
        for l in &lines {
            text.push_str(l);
            text.push('\n');
        }
        std::fs::write(&path, text)?;
        println!("wrote {}", path.display());
    }
    Ok(ok)
}
