//! `mctas terrain spawn-check`: natural spawning, the official server vs redsim, on the same saved
//! world around the same AFK player (brief, Validation V1):
//!
//! * the game: a copy of the world with a one-line data pack that sets every mob's movement speed
//!   to 0 each tick (mobs stand where they spawned; gravity, fluids and despawning still act), the
//!   bot (creative) teleported to the AFK spot, clear weather, Normal, spawning on, `tick step` for
//!   the night; then saved and the entities read back from the region files;
//! * redsim: `terrain::build_world` of the same area, the saved cats and bees as counted mobs, the
//!   player at the AFK spot, the same time of day, `World::tick` for the same ticks
//!   (`MobWorld.natural`: `natural::tick_chunks` and `check_despawn`);
//! * per mob type, the per-night counts compared with Welch's t-test (Holm-corrected over the
//!   types); spawn heights and distances pooled and compared by their means.

use crate::worldgen;
use anyhow::{Context, Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use redsim::entity::mob::natural::{self, Difficulty, Natural};
use redsim::terrain::Terrain;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One night's outcome: the natural mobs alive at its end (type id, position).
#[derive(Clone, Debug, Default)]
pub struct Night {
    pub mobs: Vec<(String, [f64; 3])>,
}

impl Night {
    pub fn counts(&self) -> BTreeMap<String, f64> {
        let mut m = BTreeMap::new();
        for (t, _) in &self.mobs {
            *m.entry(t.clone()).or_insert(0.0) += 1.0;
        }
        m
    }
}

/// The settings both sides share.
#[derive(Clone, Debug)]
pub struct Opts {
    pub version: String,
    pub afk: [f64; 3],
    pub ticks: u32,
    pub day_time: i64,
    /// Columns read back around the AFK spot.
    pub radius: i32,
    /// Print redsim's counts every 100 ticks.
    pub trace: bool,
    /// Turn spawning off after this many ticks (0: never): the despawn rate alone.
    pub fill: u32,
}

/// The types natural spawning makes in these biomes (the ones compared).
fn counted(id: &str) -> bool {
    natural::kind_of(id).is_some_and(|k| !matches!(k.id, "minecraft:cat" | "minecraft:bee"))
}

const PACK_META: &str = "{\"pack\":{\"description\":\"mctas terrain spawn-check: mobs stand still\",\"min_format\":121,\"max_format\":121}}\n";
const TICK_TAG: &str = "{\"values\":[\"mctas:freeze\"]}\n";
const FREEZE: &str = "execute as @e[type=!player] run attribute @s minecraft:movement_speed base set 0\n";

/// One night in the official server.
pub fn game_night(o: &Opts, world: &Path, scratch: &Path) -> Result<(Night, i64)> {
    if scratch.exists() {
        std::fs::remove_dir_all(scratch)?;
    }
    worldgen::copy_world(world, scratch)?;
    let pack = scratch.join("datapacks/mctas_freeze");
    std::fs::create_dir_all(pack.join("data/mctas/function"))?;
    std::fs::create_dir_all(pack.join("data/minecraft/tags/function"))?;
    std::fs::write(pack.join("pack.mcmeta"), PACK_META)?;
    std::fs::write(pack.join("data/mctas/function/freeze.mcfunction"), FREEZE)?;
    std::fs::write(pack.join("data/minecraft/tags/function/tick.json"), TICK_TAG)?;
    let mut s = VanillaServer::start(&VanillaOptions {
        version: o.version.clone(),
        heap_mb: 4096,
        world: Some(scratch.to_path_buf()),
        default_world: true,
        frozen: true,
        with_bot: true,
        view_distance: 10,
        simulation_distance: 10,
        difficulty: "normal".into(),
        ..VanillaOptions::default()
    })?;
    let packs = s.cmd("datapack list enabled")?;
    if !packs.contains("mctas_freeze") {
        bail!("the freeze data pack is not enabled: {packs}");
    }
    for c in ["weather clear", "difficulty normal", "gamerule advance_weather false"] {
        s.cmd_ok(c)?;
    }
    s.cmd_ok(&format!("tp mctas_bot {} {} {}", o.afk[0], o.afk[1], o.afk[2]))?;
    // the chunks around the player load and start ticking (spawning still off)
    s.step(100)?;
    s.cmd_ok(&format!("time set {}", o.day_time))?;
    s.cmd_ok("gamerule advance_time true")?;
    s.cmd_ok("gamerule spawn_mobs true")?;
    s.cmd_ok("gamerule spawn_monsters true")?;
    let t0 = s.game_time()?;
    if o.trace {
        // the monsters alive every 100 ticks, at the surface (y >= 60) and below
        let sel = "type=!player,type=!bat,type=!villager,type=!iron_golem,type=!cat,type=!bee,type=!chest_minecart,type=!squid,type=!glow_squid,type=!cow,type=!pig,type=!sheep,type=!chicken,type=!horse,type=!donkey,type=!wolf,type=!rabbit,type=!item,type=!salmon,type=!cod";
        for k in 0..o.ticks / 100 {
            if o.fill > 0 && k * 100 == o.fill {
                s.cmd_ok("gamerule spawn_mobs false")?;
            }
            s.step(100)?;
            let (x0, z0) = (o.afk[0] - 160.0, o.afk[2] - 160.0);
            let up = s.cmd(&format!("execute if entity @e[{sel},x={x0},y=60,z={z0},dx=320,dy=200,dz=320]"))?;
            let down = s.cmd(&format!("execute if entity @e[{sel},x={x0},y=-64,z={z0},dx=320,dy=123.99,dz=320]"))?;
            let num = |t: &str| if t.contains("Test failed") { 0 } else { t.rsplit(' ').next().unwrap_or("0").trim().parse::<i32>().unwrap_or(-1) };
            let day = s.cmd("time query time")?;
            eprintln!("game tick {}: monsters at the surface {}, below {} ({day})", (k + 1) * 100, num(&up), num(&down));
        }
        s.step(o.ticks % 100)?;
    } else {
        s.step(o.ticks)?;
    }
    let probe = s.cmd("execute if entity @e[type=!player]")?;
    let bot = s.cmd("data get entity mctas_bot Pos")?;
    let mode = s.cmd("data get entity mctas_bot playerGameType")?;
    eprintln!("game: entities {probe}; bot {bot}; {mode}; {}", s.cmd("list")?);
    let out = s.cmd("save-all flush")?;
    if !out.to_lowercase().contains("saved") {
        bail!("save-all flush: {out}");
    }
    let root = s.dir.join(&s.level_name);
    let modern = root.join("dimensions").join("minecraft").join("overworld");
    let dim = if modern.exists() { modern } else { root };
    let mut cache = std::collections::HashMap::new();
    let mut night = Night::default();
    let (cx0, cz0) = (((o.afk[0] as i32) - o.radius).div_euclid(16), ((o.afk[2] as i32) - o.radius).div_euclid(16));
    let (cx1, cz1) = (((o.afk[0] as i32) + o.radius).div_euclid(16), ((o.afk[2] as i32) + o.radius).div_euclid(16));
    let mut all = 0;
    for cz in cz0..=cz1 {
        for cx in cx0..=cx1 {
            for e in crate::region::read_entities(&dim, cx, cz, &mut cache)? {
                all += 1;
                if counted(&e.id) {
                    night.mobs.push((e.id, e.pos));
                }
            }
        }
    }
    eprintln!("game: {all} entities read back from {}", dim.join("entities").display());
    Ok((night, t0))
}

/// One night in redsim (a clone of `base` per seed).
pub fn redsim_night(o: &Opts, base: &redsim::World, t: &Terrain, seed: i64, t0: i64) -> Result<Night> {
    let mut w = base.clone();
    w.time = t0;
    w.mob.seed(seed, o.day_time - t0);
    // the village's cats count toward the creature cap (its bees sleep in their hives at night:
    // inside a block entity, not counted)
    for e in &t.entities {
        let ty = match e.id.as_str() {
            "minecraft:cat" => redsim::entity::mob::types::MobType::from_id("cat"),
            _ => None,
        };
        if let Some(ty) = ty {
            let id = redsim::entity::mob::add(&mut w, ty, redsim::entity::Vec3::new(e.pos[0], e.pos[1], e.pos[2]), false);
            // they stand still (their walking is not modeled)
            if let Some(i) = redsim::entity::index_of(&w, id) {
                w.entities.list[i].no_physics = true;
            }
        }
    }
    w.mob.natural = Some(Natural::new(vec![redsim::entity::Vec3::new(o.afk[0], o.afk[1], o.afk[2])], Difficulty::Normal));
    for tick in 0..o.ticks {
        if o.fill > 0 && tick == o.fill
            && let Some(n) = w.mob.natural.as_mut()
        {
            n.spawning = false;
        }
        w.tick();
        if o.trace && (tick + 1) % 100 == 0 {
            let n = w.mob.natural.as_ref().unwrap();
            let spawns = n.log.iter().filter(|e| matches!(e, natural::NaturalEvent::Spawn { .. })).count();
            let despawns = n.log.iter().filter(|e| matches!(e, natural::NaturalEvent::Despawn { .. })).count();
            let alive = w.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, redsim::entity::EntityKind::Mob(m) if natural::category_of(m.ty) == natural::Category::Monster)).count();
            let surface = w.entities.list.iter().filter(|e| !e.removed && e.pos.y >= 60.0 && matches!(&e.kind, redsim::entity::EntityKind::Mob(m) if natural::category_of(m.ty) == natural::Category::Monster)).count();
            eprintln!("redsim seed {seed} tick {}: day {} darken {} monsters alive {alive} (surface {surface}); spawns {spawns}, despawns {despawns}", tick + 1, w.day_time(), redsim::daylight::sky_darken(w.day_time()));
        }
    }
    if let Some(e) = w.errors.iter().find(|e| !e.starts_with("natural spawning: minecraft:")) {
        bail!("redsim night (seed {seed}): {e}");
    }
    let mut night = Night::default();
    for e in &w.entities.list {
        if e.removed {
            continue;
        }
        if let redsim::entity::EntityKind::Mob(m) = &e.kind {
            let id = m.ty.def().game_id;
            if counted(id) {
                night.mobs.push((id.to_string(), [e.pos.x, e.pos.y, e.pos.z]));
            }
        }
    }
    Ok(night)
}

// ------------------------------------------------------------------------------ statistics

fn ln_gamma(x: f64) -> f64 {
    // Lanczos (g = 7, n = 9)
    const C: [f64; 9] = [0.999_999_999_999_809_9, 676.520_368_121_885_1, -1_259.139_216_722_402_8, 771.323_428_777_653_1, -176.615_029_162_140_6, 12.507_343_278_686_905, -0.138_571_095_265_720_12, 9.984_369_578_019_572e-6, 1.505_632_735_149_311_6e-7];
    if x < 0.5 {
        return (std::f64::consts::PI / (std::f64::consts::PI * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = C[0];
    let t = x + 7.5;
    for (i, c) in C.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// The regularized incomplete beta function (continued fraction).
fn betainc(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let bt = (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    let cf = |a: f64, b: f64, x: f64| {
        let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
        let (mut c, mut d) = (1.0, 1.0 - qab * x / qap);
        if d.abs() < 1e-300 {
            d = 1e-300;
        }
        d = 1.0 / d;
        let mut h = d;
        for m in 1..300 {
            let m = m as f64;
            let m2 = 2.0 * m;
            let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
            d = 1.0 + aa * d;
            if d.abs() < 1e-300 {
                d = 1e-300;
            }
            c = 1.0 + aa / c;
            if c.abs() < 1e-300 {
                c = 1e-300;
            }
            d = 1.0 / d;
            h *= d * c;
            let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
            d = 1.0 + aa * d;
            if d.abs() < 1e-300 {
                d = 1e-300;
            }
            c = 1.0 + aa / c;
            if c.abs() < 1e-300 {
                c = 1e-300;
            }
            d = 1.0 / d;
            let del = d * c;
            h *= del;
            if (del - 1.0).abs() < 1e-12 {
                break;
            }
        }
        h
    };
    if x < (a + 1.0) / (a + b + 2.0) { bt * cf(a, b, x) / a } else { 1.0 - bt * cf(b, a, 1.0 - x) / b }
}

/// Welch's two-sample t-test: (t, degrees of freedom, two-sided p).
pub fn welch(a: &[f64], b: &[f64]) -> (f64, f64, f64) {
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    let var = |v: &[f64], m: f64| v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() as f64 - 1.0).max(1.0);
    let (ma, mb) = (mean(a), mean(b));
    let (va, vb) = (var(a, ma) / a.len() as f64, var(b, mb) / b.len() as f64);
    let se = (va + vb).sqrt();
    if se == 0.0 {
        return (0.0, 0.0, if ma == mb { 1.0 } else { 0.0 });
    }
    let t = (ma - mb) / se;
    let df = (va + vb) * (va + vb) / (va * va / (a.len() as f64 - 1.0).max(1.0) + vb * vb / (b.len() as f64 - 1.0).max(1.0));
    let p = betainc(df / 2.0, 0.5, df / (df + t * t));
    (t, df, p)
}

/// Holm's step-down adjustment of p-values (same order as given).
pub fn holm(p: &[f64]) -> Vec<f64> {
    let mut idx: Vec<usize> = (0..p.len()).collect();
    idx.sort_by(|&i, &j| p[i].total_cmp(&p[j]));
    let m = p.len();
    let mut out = vec![0.0; m];
    let mut running: f64 = 0.0;
    for (rank, &i) in idx.iter().enumerate() {
        let adj = ((m - rank) as f64 * p[i]).min(1.0);
        running = running.max(adj);
        out[i] = running;
    }
    out
}

/// The report: per type the game and redsim means per night, Welch's p and Holm's.
pub fn report(game: &[Night], ours: &[Night], afk: [f64; 3]) -> String {
    let mut types: Vec<String> = game.iter().chain(ours.iter()).flat_map(|n| n.mobs.iter().map(|m| m.0.clone())).collect();
    types.sort();
    types.dedup();
    types.insert(0, "(all monsters)".into());
    let monster = |id: &str| natural::kind_of(id).is_some_and(|k| k.category == natural::Category::Monster);
    let per = |nights: &[Night], ty: &str| -> Vec<f64> {
        nights
            .iter()
            .map(|n| n.mobs.iter().filter(|m| if ty == "(all monsters)" { monster(&m.0) } else { m.0 == ty }).count() as f64)
            .collect()
    };
    let mut rows = Vec::new();
    let mut ps = Vec::new();
    for ty in &types {
        let (g, r) = (per(game, ty), per(ours, ty));
        let (t, df, p) = welch(&g, &r);
        ps.push(p);
        rows.push((ty.clone(), g.iter().sum::<f64>() / g.len() as f64, r.iter().sum::<f64>() / r.len() as f64, t, df, p));
    }
    let adj = holm(&ps);
    let mut out = format!("{} game nights, {} redsim nights; mean mobs alive per night at the end:\n", game.len(), ours.len());
    out.push_str("type                         game    redsim      t      df   p      p(Holm)\n");
    for (i, (ty, mg, mr, t, df, p)) in rows.iter().enumerate() {
        out.push_str(&format!("{ty:<28} {mg:6.2}  {mr:6.2}  {t:6.2}  {df:6.1}  {p:.3}  {:.3}\n", adj[i]));
    }
    // where: heights and distances of the monsters, pooled
    let pos = |nights: &[Night]| -> (Vec<f64>, Vec<f64>) {
        let mut ys = Vec::new();
        let mut ds = Vec::new();
        for n in nights {
            for (id, p) in &n.mobs {
                if monster(id) {
                    ys.push(p[1]);
                    ds.push(((p[0] - afk[0]).powi(2) + (p[2] - afk[2]).powi(2)).sqrt());
                }
            }
        }
        (ys, ds)
    };
    let ((gy, gd), (ry, rd)) = (pos(game), pos(ours));
    if gy.len() > 1 && ry.len() > 1 {
        // the height histogram (bands of 16), per night
        out.push_str("monster heights per night (bands of 16 blocks): band  game  redsim\n");
        for band in (-64..128).step_by(16) {
            let g = gy.iter().filter(|y| (**y as i32) >= band && (**y as i32) < band + 16).count() as f64 / game.len() as f64;
            let r = ry.iter().filter(|y| (**y as i32) >= band && (**y as i32) < band + 16).count() as f64 / ours.len() as f64;
            if g > 0.0 || r > 0.0 {
                out.push_str(&format!("  {band:4}..{:<4} {g:6.2} {r:6.2}\n", band + 15));
            }
        }
        // per type: at the surface (y >= 60) and below
        out.push_str("per type, surface (y >= 60) / below, per night:       game          redsim\n");
        let mut tys: Vec<String> = game.iter().chain(ours.iter()).flat_map(|n| n.mobs.iter().filter(|m| monster(&m.0)).map(|m| m.0.clone())).collect();
        tys.sort();
        tys.dedup();
        for ty in tys {
            let split = |nights: &[Night]| {
                let s = nights.iter().flat_map(|n| n.mobs.iter()).filter(|m| m.0 == ty && m.1[1] >= 60.0).count() as f64 / nights.len() as f64;
                let b = nights.iter().flat_map(|n| n.mobs.iter()).filter(|m| m.0 == ty && m.1[1] < 60.0).count() as f64 / nights.len() as f64;
                (s, b)
            };
            let ((gs, gb), (rs, rb)) = (split(game), split(ours));
            out.push_str(&format!("  {ty:<28} {gs:6.2} / {gb:6.2}   {rs:6.2} / {rb:6.2}\n"));
        }
        let (_, _, py) = welch(&gy, &ry);
        let (_, _, pd) = welch(&gd, &rd);
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        out.push_str(&format!("monster height: game mean y {:.1} ({} mobs), redsim {:.1} ({} mobs), p {:.3} (clustered: indicative)\n", mean(&gy), gy.len(), mean(&ry), ry.len(), py));
        out.push_str(&format!("monster distance from the AFK spot: game {:.1}, redsim {:.1}, p {:.3}\n", mean(&gd), mean(&rd), pd));
    }
    out
}

/// Read the game nights back from a dump `run` wrote (`game\t<night>\t<id>\tx\ty\tz` lines).
pub fn load_game(dump: &Path) -> Result<Vec<Night>> {
    let text = std::fs::read_to_string(dump)?;
    let mut nights: BTreeMap<usize, Night> = BTreeMap::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() == 6 && f[0] == "game" {
            let n: usize = f[1].parse()?;
            let p = [f[3].parse()?, f[4].parse()?, f[5].parse()?];
            nights.entry(n).or_default().mobs.push((f[2].to_string(), p));
        }
    }
    Ok(nights.into_values().collect())
}

/// The whole check (`game_dump`: the game's nights from an earlier run's dump).
pub fn run(o: &Opts, area: &Path, world: &Path, nights: u32, seeds: u32, out: Option<&Path>, game_dump: Option<&Path>) -> Result<String> {
    let t = Terrain::decode(&std::fs::read(area).with_context(|| format!("reading {}", area.display()))?).map_err(anyhow::Error::msg)?;
    let scratch: PathBuf = crate::cache_dir().join("spawncheck");
    let mut game = Vec::new();
    let mut t0 = 24135;
    if let Some(d) = game_dump {
        game = load_game(d)?;
    }
    for n in 0..if game_dump.is_some() { 0 } else { nights } {
        let (night, start) = game_night(o, world, &scratch.join(format!("night{n}")))?;
        eprintln!("game night {n}: {} mobs (start gametime {start})", night.mobs.len());
        t0 = start;
        game.push(night);
    }
    let base = redsim::terrain::build_world(&t).map_err(anyhow::Error::msg)?;
    let mut ours = Vec::new();
    for s in 0..seeds {
        let night = redsim_night(o, &base, &t, 1000 + s as i64, t0)?;
        eprintln!("redsim seed {s}: {} mobs", night.mobs.len());
        ours.push(night);
    }
    let text = report(&game, &ours, o.afk);
    if let Some(p) = out {
        let mut dump = text.clone();
        for (who, nights) in [("game", &game), ("redsim", &ours)] {
            for (i, n) in nights.iter().enumerate() {
                for (id, p) in &n.mobs {
                    dump.push_str(&format!("{who}\t{i}\t{id}\t{:.2}\t{:.2}\t{:.2}\n", p[0], p[1], p[2]));
                }
            }
        }
        std::fs::write(p, dump)?;
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn welch_matches_known_values() {
        // equal samples: p = 1; clearly different: p small
        let (_, _, p) = welch(&[1.0, 2.0, 3.0, 4.0], &[1.0, 2.0, 3.0, 4.0]);
        assert!((p - 1.0).abs() < 1e-9);
        let (t, _, p) = welch(&[10.0, 11.0, 12.0, 13.0], &[1.0, 2.0, 3.0, 4.0]);
        assert!(t > 9.0 && p < 1e-3, "{t} {p}");
        // t = 2.0 with 10 degrees of freedom: two-sided p = 0.0734
        let p = betainc(5.0, 0.5, 10.0 / 14.0);
        assert!((p - 0.0734).abs() < 1e-3, "{p}");
    }

    #[test]
    fn holm_is_monotone() {
        let a = holm(&[0.01, 0.04, 0.03]);
        assert_eq!(a, vec![0.03, 0.06, 0.06]);
    }
}
