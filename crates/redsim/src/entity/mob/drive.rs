//! A bench's drive (`--drive-out` of farmopt's `linear-bench` / `station-bench` and ironfarm's
//! queue loop, 2026-11-10): what a part's bench does to its run, written as data so the viewer
//! replays the bench's own run tick for tick (vjeux 06:58, "Why does it keep saying not simulated
//! in the viewer": the carousel station plays on its page) and a check holds it to the bench's
//! expectations.
//!
//! The run starts as every bench starts it: [`farm::Run::start`] with
//! `RunOpts { ai: true, ..RunOpts::steady(seed, days * 24000, WalkMode::Random) }`, the settle run
//! out, then the design's entity lines spawned in file order ([`spawn_lines`]); `t0` is the world's
//! time then. Its lines (one each; `#` header lines carry `seed`, `days`, `walk`, `ai`, `t0`):
//!
//! * `act <t> set x y z <state>`: a block written with every update (`flags::ALL`);
//! * `act <t> tp eN x y z`: line N's entity (the cart) moved there at rest, its riders placed;
//! * `act <t> home eN.pK x y z`, `act <t> last-slept eN.pK <t'>` (`t0 + t'`): a villager's memories;
//! * `act <t> poi-take x y z`: the POI there taken (the bed);
//! * `expect <t> ...`, `expect-end ...`: what the bench saw, for the check;
//! * `# caption <words>`: what the bench drives, in a player's words (vjeux 07:29, "write down
//!   these details when you show me things"): the view shows it on its badge while the run plays.
//!
//! An act at `t` is applied when the world's time is `t0 + t`, before the run's next tick (the
//! benches' order: their actions at the top of the loop); an expectation at `t` happened in the
//! tick that brought the time to `t0 + t`. `eN` counts the entity lines from 0, `.pK` its
//! passengers from 0.

use super::farm::{FarmSpec, Run, RunOpts, WalkMode};
use crate::block::Block;
use crate::entity::{EntityKind, Vec3};
use crate::pos::BlockPos;
use crate::world::World;

/// An entity of the drive: line N's entity, or its passenger K.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Who {
    Line(usize),
    Rider(usize, usize),
}

/// One action of a bench.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Set(BlockPos, Block),
    Tp(Who, Vec3),
    Home(Who, BlockPos),
    LastSlept(Who, i64),
    PoiTake(BlockPos),
}

/// A parsed drive.
#[derive(Clone, Debug, Default)]
pub struct Drive {
    pub seed: i64,
    pub days: u32,
    /// The run's length when the bench gives it in ticks (`# ticks N`: ironfarm's queue loop)
    /// rather than days.
    pub ticks: Option<u32>,
    /// What the bench drives, in a player's words (`# caption <words>`): the view's badge.
    pub caption: Option<String>,
    /// The world's time once the entities are spawned, as the bench saw it (None: not given).
    pub t0: Option<i64>,
    /// The acts, by t, in the bench's order within one t.
    pub acts: Vec<(i64, Act)>,
    /// `expect <t> <what>` (the rest of the line) and `expect-end <what>`.
    pub expects: Vec<(i64, String)>,
    pub ends: Vec<String>,
}

fn who(s: &str) -> Result<Who, String> {
    let bad = || format!("{s:?}: expected eN or eN.pK");
    let r = s.strip_prefix('e').ok_or_else(bad)?;
    match r.split_once(".p") {
        Some((n, k)) => Ok(Who::Rider(n.parse().map_err(|_| bad())?, k.parse().map_err(|_| bad())?)),
        None => Ok(Who::Line(r.parse().map_err(|_| bad())?)),
    }
}

fn int(s: Option<&&str>, line: &str) -> Result<i32, String> {
    s.and_then(|v| v.parse().ok()).ok_or_else(|| format!("{line:?}: expected a whole number"))
}

fn num(s: Option<&&str>, line: &str) -> Result<f64, String> {
    s.and_then(|v| v.parse().ok()).ok_or_else(|| format!("{line:?}: expected a number"))
}

impl Drive {
    pub fn parse(text: &str) -> Result<Drive, String> {
        let mut d = Drive { days: 1, ..Default::default() };
        let mut walk_ok = true;
        for line in text.lines() {
            let l = line.trim();
            if l.is_empty() {
                continue;
            }
            if let Some(h) = l.strip_prefix('#') {
                let w: Vec<&str> = h.split_whitespace().collect();
                match w.first().copied() {
                    Some("seed") => d.seed = w.get(1).and_then(|v| v.parse().ok()).ok_or_else(|| format!("{l:?}: a seed"))?,
                    Some("days") => d.days = w.get(1).and_then(|v| v.parse().ok()).ok_or_else(|| format!("{l:?}: a number of days"))?,
                    Some("ticks") => d.ticks = Some(w.get(1).and_then(|v| v.parse().ok()).ok_or_else(|| format!("{l:?}: a number of ticks"))?),
                    Some("t0") => d.t0 = Some(w.get(1).and_then(|v| v.parse().ok()).ok_or_else(|| format!("{l:?}: a time"))?),
                    Some("caption") => d.caption = Some(h.trim_start().trim_start_matches("caption").trim().to_string()).filter(|c| !c.is_empty()),
                    Some("walk") => walk_ok = w.get(1) == Some(&"random"),
                    Some("ai") if w.get(1) != Some(&"on") => return Err(format!("{l:?}: the benches run with the mob AI on")),
                    _ => {}
                }
                continue;
            }
            let w: Vec<&str> = l.split_whitespace().collect();
            // (a header line written bare, without its `#`: ironfarm's queue loop's first lines)
            if matches!(w.first().copied(), Some("seed" | "days" | "walk" | "ai" | "ticks" | "t0" | "caption")) {
                let h = format!("#{l}");
                let mut sub = Drive::parse(&h)?;
                if w[0] == "caption" {
                    d.caption = sub.caption.take();
                } else if w[0] == "seed" {
                    d.seed = sub.seed;
                } else if w[0] == "days" {
                    d.days = sub.days;
                } else if w[0] == "ticks" {
                    d.ticks = sub.ticks.take();
                } else if w[0] == "t0" {
                    d.t0 = sub.t0;
                } else if w[0] == "walk" {
                    walk_ok = w.get(1) == Some(&"random");
                }
                continue;
            }
            match w.first().copied() {
                Some("act") => {
                    let t: i64 = w.get(1).and_then(|v| v.parse().ok()).ok_or_else(|| format!("{l:?}: act <t> ..."))?;
                    let pos = |i: usize| -> Result<BlockPos, String> { Ok(BlockPos::new(int(w.get(i), l)?, int(w.get(i + 1), l)?, int(w.get(i + 2), l)?)) };
                    let a = match w.get(2).copied() {
                        Some("set") => {
                            let state = w.get(6).ok_or_else(|| format!("{l:?}: set x y z <state>"))?;
                            Act::Set(pos(3)?, Block::parse(state).map_err(|e| format!("{l:?}: {e}"))?)
                        }
                        Some("tp") => Act::Tp(who(w.get(3).copied().unwrap_or(""))?, Vec3::new(num(w.get(4), l)?, num(w.get(5), l)?, num(w.get(6), l)?)),
                        Some("home") => Act::Home(who(w.get(3).copied().unwrap_or(""))?, pos(4)?),
                        Some("last-slept") => Act::LastSlept(who(w.get(3).copied().unwrap_or(""))?, w.get(4).and_then(|v| v.parse().ok()).ok_or_else(|| format!("{l:?}: last-slept eN.pK <t>"))?),
                        Some("poi-take") => Act::PoiTake(pos(3)?),
                        _ => return Err(format!("{l:?}: an act this replay does not know")),
                    };
                    d.acts.push((t, a));
                }
                Some("expect") => {
                    let t: i64 = w.get(1).and_then(|v| v.parse().ok()).ok_or_else(|| format!("{l:?}: expect <t> ..."))?;
                    d.expects.push((t, w[2..].join(" ")));
                }
                Some("expect-end") => d.ends.push(w[1..].join(" ")),
                _ => return Err(format!("{l:?}: not a drive line")),
            }
        }
        if !walk_ok {
            return Err("a drive whose golems do not walk at random (`# walk random`): the benches' policy".into());
        }
        d.acts.sort_by_key(|(t, _)| *t);
        Ok(d)
    }

    /// The benches' run options.
    pub fn opts(&self) -> RunOpts {
        RunOpts { ai: true, ..RunOpts::steady(self.seed, self.ticks.unwrap_or(self.days * 24000), WalkMode::Random) }
    }

    /// The ticks the bench ran.
    pub fn ticks(&self) -> i64 {
        self.ticks.map(i64::from).unwrap_or(self.days as i64 * 24000)
    }
}

/// The design's entity lines spawned in file order after the settle, as the benches do: each
/// line's entity id (its vehicle's), and the run's own mobs set to all of them.
pub fn spawn_lines(run: &mut Run, spec: &FarmSpec) -> Result<Vec<u32>, String> {
    let first = run.world.entities.next_id;
    let mut lines = Vec::with_capacity(spec.entities.len());
    for (ty, pos, nbt) in &spec.entities {
        lines.push(crate::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?);
    }
    run.own = (first..run.world.entities.next_id).collect();
    Ok(lines)
}

/// The world index of a drive's entity (None: gone).
pub fn index_of(w: &World, lines: &[u32], who: Who) -> Option<usize> {
    let alive = |id: u32| w.entities.list.iter().position(|e| e.id == id && !e.removed);
    match who {
        Who::Line(n) => alive(*lines.get(n)?),
        Who::Rider(n, k) => {
            let v = alive(*lines.get(n)?)?;
            alive(*w.entities.list[v].passengers.get(k)?)
        }
    }
}

/// One act on the world (`t0`: the drive's start time, for a `last-slept`).
pub fn apply(w: &mut World, lines: &[u32], t0: i64, a: &Act) {
    match a {
        Act::Set(p, b) => {
            w.set_block(*p, b.clone(), crate::world::flags::ALL);
        }
        Act::Tp(who, p) => {
            // (the benches' `move_cart`: at rest there, its riders placed on it)
            let Some(i) = index_of(w, lines, *who) else { return };
            w.entities.set_pos(i, *p);
            w.entities.list[i].delta_movement = Vec3::new(0.0, 0.0, 0.0);
            let passengers = w.entities.list[i].passengers.clone();
            for pid in passengers {
                if let Some(j) = w.entities.list.iter().position(|e| e.id == pid && !e.removed) {
                    crate::entity::position_rider(w, i, j);
                }
            }
        }
        Act::Home(who, p) => {
            if let Some(i) = index_of(w, lines, *who).filter(|i| matches!(w.entities.list[*i].kind, EntityKind::Mob(_))) {
                super::ai::brain::brain_mut(w, i).home = Some(*p);
            }
        }
        Act::LastSlept(who, t) => {
            if let Some(i) = index_of(w, lines, *who).filter(|i| matches!(w.entities.list[*i].kind, EntityKind::Mob(_))) {
                super::ai::brain::brain_mut(w, i).last_slept = Some(t0 + t);
            }
        }
        Act::PoiTake(p) => super::ai::poi::take(w, *p),
    }
}

/// A drive's place in its run: the acts applied so far.
#[derive(Clone, Debug, Default)]
pub struct Cursor {
    pub next: usize,
}

impl Cursor {
    /// The acts due now (the world's time `t0 + t`: every act at `t` or before not applied yet),
    /// in the bench's order: call it before each tick.
    pub fn apply_due(&mut self, w: &mut World, lines: &[u32], d: &Drive, t0: i64) {
        let t = w.time - t0;
        while let Some((at, a)) = d.acts.get(self.next) {
            if *at > t {
                break;
            }
            apply(w, lines, t0, a);
            self.next += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drive_reads_its_lines() {
        let d = Drive::parse("# drive v1\n# seed 3\n# days 2\n# walk random\n# ai on\n# t0 200\nact 0 poi-take 3 0 66\nact 0 home e0.p0 3 0 66\nact 0 last-slept e0.p0 -100\nact 40 tp e4 1.5 0.0625 2.5\nact 41 set 1 0 2 minecraft:redstone_block\nexpect 100 golem g0 1 3 -1\nexpect-end golems 239\n").unwrap();
        assert_eq!((d.seed, d.days, d.t0, d.ticks()), (3, 2, Some(200), 48000));
        assert_eq!(d.acts.len(), 5);
        assert_eq!(d.acts[1].1, Act::Home(Who::Rider(0, 0), BlockPos::new(3, 0, 66)));
        assert_eq!(d.acts[3].1, Act::Tp(Who::Line(4), Vec3::new(1.5, 0.0625, 2.5)));
        assert_eq!(d.expects, vec![(100, "golem g0 1 3 -1".to_string())]);
        assert_eq!(d.ends, vec!["golems 239".to_string()]);
        assert!(Drive::parse("act 1 fly e0\n").is_err());
        let c = Drive::parse("# seed 1\n# caption arrivals and the release driven by the test bench, no redstone yet\nact 0 poi-take 3 0 66\n").unwrap();
        assert_eq!(c.caption.as_deref(), Some("arrivals and the release driven by the test bench, no redstone yet"));
        assert_eq!(Drive::parse("seed 1\ncaption the stand teleported\n").unwrap().caption.as_deref(), Some("the stand teleported"));
        assert_eq!(d.caption, None);
        assert!(Drive::parse("# walk fixed\n").is_err());
    }
}
