//! (tickexact 2026-11-12, ironspec: the queue post's sight lines) `mctas mobai sight-check`: on the
//! official 26.3 server, the composition (the module and its parts, their entities summoned, extra
//! `--summon`s); a villager at each `--points` position, seated in a hovering cart (NoGravity, held
//! at its point every tick: a rider there, its eye where a falling or arriving rider's is) or, with
//! `--free`, alone (moved by whatever moves it there: a bubble lift). `--repeat N --every K`: each
//! point's villager is summoned N times, K gt apart, each removed after `--life` gt (a ride shorter
//! than the sensors' 20-gt scan can fall between two scans: repeated rides land at every phase).
//! Per ride: whether its NEAREST_HOSTILE was ever set (the VillagerHostilesSensor: a hostile within
//! its distance, zombies 8, and in sight) and by which entity, its first tick, its flee walk
//! targets (SetWalkTargetAwayFrom), its closest distance to the nearest zombie (feet to feet) and
//! the distance at the end; `--trace` prints every tick each live villager's position, its distance
//! to the nearest zombie and those lines.

use crate::seatclear::{module_mismatches, place_part, start_world_part};
use anyhow::Result;
use mctas::vanilla::VanillaServer;

const DY: i32 = 80;

fn pos(srv: &mut VanillaServer, sel: &str) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long(&format!("data get entity {sel} Pos"))?;
    let Some(rest) = out.split("entity data: ").nth(1) else { return Ok(None) };
    let v: Vec<f64> = rest.trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|s| s.trim().trim_end_matches('d').parse().ok()).collect();
    Ok((v.len() == 3).then(|| [v[0], v[1] - DY as f64, v[2]]))
}

fn uuid_of(srv: &mut VanillaServer, sel: &str) -> Result<String> {
    let u = srv.cmd_long(&format!("data get entity {sel} UUID"))?;
    let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
    anyhow::ensure!(ints.len() == 4, "the UUID of {sel}: {u}");
    let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
    Ok(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

/// The villager's eye height and a zombie's (both adults, standing; the seated villager's eye is its
/// position plus its eye height, the same as standing).
const VILLAGER_EYE: f64 = 1.62;
const ZOMBIE_EYE: f64 = 1.74;

/// The nearest zombie to a villager: (its position, feet-to-feet distance, eye-to-eye distance).
fn nearest_zombie(srv: &mut VanillaServer, r: [f64; 3]) -> Result<Option<([f64; 3], f64, f64)>> {
    let Some(z) = pos(srv, &format!("@e[type=minecraft:zombie,sort=nearest,limit=1,x={},y={},z={}]", r[0], r[1] + DY as f64, r[2]))? else { return Ok(None) };
    let feet = ((r[0] - z[0]).powi(2) + (r[1] - z[1]).powi(2) + (r[2] - z[2]).powi(2)).sqrt();
    let eye = ((r[0] - z[0]).powi(2) + (r[1] + VILLAGER_EYE - z[1] - ZOMBIE_EYE).powi(2) + (r[2] - z[2]).powi(2)).sqrt();
    Ok(Some((z, feet, eye)))
}

/// One villager's stay: a point, from `start` to `end` gt.
struct Ride {
    name: String,
    p: [f64; 3],
    start: u32,
    end: u32,
    uuid: String,
    seen: Option<(u32, String)>,
    flee: u32,
    closest: f64,
    last: Option<[f64; 3]>,
}

/// The options of a check beyond its composition.
pub struct SightOpts {
    pub ticks: u32,
    pub free: bool,
    pub trace: bool,
    pub repeat: u32,
    pub every: u32,
    pub life: u32,
    /// (tickexact 2026-11-12, (C')'s hatch) cells laid after the parts, "x,y,z" and the state
    pub set: Vec<([i32; 3], String)>,
    /// the day time set before the rides (None: as the world starts)
    pub day_time: Option<i64>,
}

pub fn sight_check(version: &str, module: &std::path::Path, parts: &[std::path::PathBuf], summon: &[String], points: &[(String, [f64; 3])], o: &SightOpts) -> Result<()> {
    let repeat = o.repeat.max(1);
    let ticks = if repeat > 1 { o.ticks.max((repeat - 1) * o.every + o.life + 1) } else { o.ticks };
    println!(
        "== sight-check on the official {version} server: {} with {}; {} villagers {}{}; {ticks} gt of the sensors",
        module.display(),
        parts.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "),
        points.len(),
        if o.free { "alone (free)" } else { "seated in hovering carts" },
        if repeat > 1 { format!(", each {repeat} times {} gt apart, {} gt each", o.every, o.life) } else { String::new() }
    );
    let (mut srv, mut agent, _bed) = start_world_part(version, module, false, &[], None)?;
    for p in parts {
        place_part(&mut srv, p)?;
    }
    for (p, s) in &o.set {
        srv.cmd(&format!("setblock {} {} {} {s}", p[0], p[1] + DY, p[2]))?;
        println!("  set: ({},{},{}) {s}", p[0], p[1], p[2]);
    }
    if let Some(d) = o.day_time {
        srv.cmd_ok(&format!("time set {d}"))?;
        println!("  the day time: {d}");
    }
    for s in summon {
        let w: Vec<&str> = s.trim().splitn(5, ' ').collect();
        anyhow::ensure!(w.len() >= 4, "--summon \"kind x y z [nbt]\", got {s}");
        let (x, y, z): (f64, f64, f64) = (w[1].parse()?, w[2].parse()?, w[3].parse()?);
        srv.cmd_ok(&format!("summon {} {x} {} {z} {}", w[0], y + DY as f64, w.get(4).copied().unwrap_or("")))?;
        println!("  summoned: {s}");
    }
    srv.step(10)?;
    for f in std::iter::once(module).chain(parts.iter().map(|p| p.as_path())) {
        let m = module_mismatches(&mut srv, f)?;
        println!("  module-check {}: {} cells differ{}", f.display(), m.len(), if m.is_empty() { String::new() } else { format!(" (first: {})", m.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) });
    }
    let mut rides: Vec<Ride> = Vec::new();
    for (name, p) in points {
        for k in 0..repeat {
            let start = k * o.every;
            let end = if repeat > 1 { start + o.life } else { ticks + 1 };
            rides.push(Ride { name: if repeat > 1 { format!("{name}#{k}") } else { name.clone() }, p: *p, start, end, uuid: String::new(), seen: None, flee: 0, closest: f64::MAX, last: None });
        }
    }
    for t in 0..=ticks {
        // the rides starting now, then those ending now
        for (i, r) in rides.iter_mut().enumerate() {
            if r.start == t {
                if o.free {
                    srv.cmd_ok(&format!("summon minecraft:villager {} {} {} {{PersistenceRequired:1b,Tags:[\"scv{i}\"]}}", r.p[0], r.p[1] + DY as f64, r.p[2]))?;
                } else {
                    srv.cmd_ok(&format!(
                        "summon minecraft:minecart {} {} {} {{NoGravity:1b,Tags:[\"sc{i}\"],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"scv{i}\"]}}]}}",
                        r.p[0],
                        r.p[1] + DY as f64,
                        r.p[2]
                    ))?;
                }
                r.uuid = uuid_of(&mut srv, &format!("@e[tag=scv{i},limit=1]"))?;
            }
            if r.end == t {
                r.last = pos(&mut srv, &format!("@e[tag=scv{i},limit=1]"))?;
                srv.cmd(&format!("kill @e[tag=scv{i}]"))?;
                srv.cmd(&format!("kill @e[tag=sc{i}]"))?;
            }
        }
        if t == ticks {
            break;
        }
        srv.step(1)?;
        let live: Vec<usize> = (0..rides.len()).filter(|&i| rides[i].start <= t && t < rides[i].end).collect();
        // the hovering carts are held at their points (stacked carts push each other apart, and a
        // cart on a rail rolls): back to the point, at rest, every tick
        if !o.free {
            for &i in &live {
                let p = rides[i].p;
                srv.cmd(&format!("tp @e[tag=sc{i},limit=1] {} {} {}", p[0], p[1] + DY as f64, p[2]))?;
                srv.cmd(&format!("data merge entity @e[tag=sc{i},limit=1] {{Motion:[0.0d,0.0d,0.0d]}}"))?;
            }
        }
        let mut lines: Vec<Vec<String>> = vec![Vec::new(); rides.len()];
        for l in agent.drain(&srv)? {
            let who = l.rsplit(" @").next().map(|u| u.trim()).unwrap_or("");
            let Some(i) = rides.iter().position(|r| !r.uuid.is_empty() && r.uuid == who) else { continue };
            let by = l.rsplit(" by ").next().unwrap_or("");
            let text = l.split(" @").next().unwrap_or("").to_string();
            if l.starts_with("set minecraft:nearest_hostile") {
                if rides[i].seen.is_none() {
                    rides[i].seen = Some((t + 1 - rides[i].start, l.split(" = ").nth(1).unwrap_or("").split(" by ").next().unwrap_or("").to_string()));
                }
                lines[i].push(text);
            } else if l.starts_with("set minecraft:walk_target") && by.starts_with("SetWalkTargetAwayFrom") {
                rides[i].flee += 1;
                lines[i].push(text);
            } else if l.starts_with("activity panic") {
                lines[i].push(text);
            }
        }
        for &i in &live {
            let r = pos(&mut srv, &format!("@e[tag=scv{i},limit=1]"))?;
            let nz = match r {
                Some(r) => nearest_zombie(&mut srv, r)?,
                None => None,
            };
            if let Some((_, feet, _)) = nz {
                rides[i].closest = rides[i].closest.min(feet);
            }
            if o.trace {
                println!(
                    "  t{} {} (+{}): {} | {}{}",
                    t + 1,
                    rides[i].name,
                    t + 1 - rides[i].start,
                    r.map(|r| format!("({:.4},{:.4},{:.4})", r[0], r[1], r[2])).unwrap_or_else(|| "gone".into()),
                    nz.map(|(_, f, e)| format!("zombie {f:.3} feet / {e:.3} eye")).unwrap_or_else(|| "no zombie".into()),
                    if lines[i].is_empty() { String::new() } else { format!(" | {}", lines[i].join("; ")) }
                );
            }
        }
    }
    let mut n_seen = 0;
    for (i, r) in rides.iter().enumerate() {
        let at = match r.last {
            Some(p) => Some(p),
            None => pos(&mut srv, &format!("@e[tag=scv{i},limit=1]"))?,
        };
        if r.seen.is_some() {
            n_seen += 1;
        }
        println!(
            "  {}: the villager at {} at its end; closest to a zombie {:.3} (feet to feet) | NEAREST_HOSTILE {} | flee walk targets {}",
            r.name,
            at.map(|p| format!("({:.4},{:.4},{:.4})", p[0], p[1], p[2])).unwrap_or_else(|| "gone".into()),
            r.closest,
            match &r.seen {
                Some((t, v)) => format!("SET at +{t} gt: {v}"),
                None => "never set".into(),
            },
            r.flee
        );
    }
    println!("  {n_seen} of {} rides saw a hostile", rides.len());
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
