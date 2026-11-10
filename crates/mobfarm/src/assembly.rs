//! Assembly v0 of the iron carousel (parent 2026-11-10 02:51): the three finished parts in one
//! design: day-1's counter v7 (designs/farms/iron/parts/carousel-counter), ironfarm's release +
//! hold (`hold::design`) and farmopt's station + portal pad + Nether part (`station::design`),
//! with the sleep routine stood in by the benches' once-a-night LAST_SLEPT.
//!
//! Step a (this file now): the counter placed under the stand, out of every rider's golem search
//! (the stand riders' and the held riders' searches cover x -8..14, z -8..28 and floors y -7..11:
//! the counter's top layer at y -9 or lower), a glass roof over its ring (its 7 open hopper tops
//! under the loop's dust: no drop or mob reaches them), and the pad's pulse wired down to the
//! counter's input: a dust staircase in the x = 10 plane (clear of the hold zombies' boats at x 8,
//! which reach x 9.19), every step's stone under a dust (no spawn floor), one repeater halfway.
//! The counter's lane lines drive the slope controls ideally in the bench (`station::bench` reads
//! each lane's comparator); the physical line wiring (ironfarm's torch chains at the stand, the
//! hold ends' repeaters) is step b, on ironfarm's shifted stack.

use anyhow::{Result, anyhow};
use mctas::design::Design;
use redsim::pos::BlockPos;

/// Counter v7 (day-1, ec34ea415), as checked in.
pub const COUNTER_V7: &str = include_str!("../../../designs/farms/iron/parts/carousel-counter/carousel-counter-v7.txt");

/// Where the counter's frame goes: its blocks shifted by this (lane b's line at x = b: the file's
/// b0 is at x -3).
#[derive(Clone, Copy, Debug)]
pub struct AsmOpts {
    pub counter_at: [i32; 3],
    /// The lines drive the slope controls through the design's own wiring (step b): the bench
    /// then only reads the lines (the batch at each slot, the margins), it switches nothing.
    pub wired: bool,
}

impl Default for AsmOpts {
    fn default() -> AsmOpts {
        AsmOpts { counter_at: [3, -10, 15], wired: false }
    }
}

impl AsmOpts {
    /// The `assembly:` line.
    pub fn meta(&self) -> String {
        format!("counter=v7 at={},{},{}{}", self.counter_at[0], self.counter_at[1], self.counter_at[2], if self.wired { " wired=1" } else { "" })
    }

    pub fn of(meta: &std::collections::BTreeMap<String, String>) -> Result<Option<AsmOpts>> {
        let Some(line) = meta.get("assembly") else { return Ok(None) };
        let mut o = AsmOpts::default();
        for kv in line.split_whitespace() {
            let (k, v) = kv.split_once('=').ok_or_else(|| anyhow!("bad `assembly:` entry {kv:?}"))?;
            match k {
                "counter" => anyhow::ensure!(v == "v7", "only counter v7"),
                "at" => {
                    let p: Vec<i32> = v.split(',').map(|s| s.parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(p.len() == 3, "at=x,y,z");
                    o.counter_at = [p[0], p[1], p[2]];
                }
                "wired" => o.wired = v == "1",
                _ => anyhow::bail!("unknown `assembly:` entry {k:?}"),
            }
        }
        Ok(Some(o))
    }
}

fn counter() -> Result<Design> {
    Design::parse(COUNTER_V7)
}

/// Lane b's tap: the comparator whose output is the lane's line (beside the file's `output.bN`
/// cell), in the assembly frame.
pub fn counter_taps(o: &AsmOpts) -> Result<Vec<BlockPos>> {
    let c = counter()?;
    let a = o.counter_at;
    let mut taps = Vec::new();
    for b in 0..crate::hold::BATCHES {
        let out = *c.ports.get(&format!("output.b{b}")).ok_or_else(|| anyhow!("counter v7 has no output.b{b}"))?;
        let tap = [[0, 0, 1], [0, 0, -1], [1, 0, 0], [-1, 0, 0]]
            .iter()
            .map(|d| [out[0] + d[0], out[1] + d[1], out[2] + d[2]])
            .find(|p| c.blocks.get(p).is_some_and(|s| s.contains("comparator")))
            .ok_or_else(|| anyhow!("no comparator beside counter v7's output.b{b}"))?;
        taps.push(BlockPos::new(tap[0] + a[0], tap[1] + a[1], tap[2] + a[2]));
    }
    Ok(taps)
}

/// The counter's input cell in the assembly frame.
pub fn counter_input(o: &AsmOpts) -> Result<[i32; 3]> {
    let c = counter()?;
    let i = *c.ports.get("input").ok_or_else(|| anyhow!("counter v7 has no input port"))?;
    let a = o.counter_at;
    Ok([i[0] + a[0], i[1] + a[1], i[2] + a[2]])
}

fn put(d: &mut Design, p: [i32; 3], s: &str) -> Result<()> {
    let s = if s.starts_with("minecraft:") { s.to_string() } else { format!("minecraft:{s}") };
    if let Some(old) = d.blocks.get(&p)
        && *old != s
    {
        anyhow::bail!("the assembly's {s} at {p:?} meets {old}");
    }
    d.blocks.insert(p, s);
    Ok(())
}

/// Step a: the station design (release + hold, pad, Nether part) with the counter, its roof and
/// the pulse wire.
pub fn v0a(pad: &crate::station::PadOpts, lanes: &crate::hold::LaneOpts, o: &AsmOpts) -> Result<String> {
    let text = crate::station::design(pad, lanes)?;
    let mut d = Design::parse(&text)?;
    let c = counter()?;
    let a = o.counter_at;
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for (p, s) in &c.blocks {
        let q = [p[0] + a[0], p[1] + a[1], p[2] + a[2]];
        for i in 0..3 {
            lo[i] = lo[i].min(q[i]);
            hi[i] = hi[i].max(q[i]);
        }
        put(&mut d, q, s)?;
    }
    for (p, s) in &c.block_entities {
        d.block_entities.insert([p[0] + a[0], p[1] + a[1], p[2] + a[2]], s.clone());
    }
    anyhow::ensure!(hi[1] <= -9, "the counter's top layer at y {} (a golem search reaches floors at y -7: keep it at -9 or lower)", hi[1]);
    // the roof: glass over the ring's droppers and hoppers (two rows of 7, the loop's dust on them)
    for (p, s) in &c.blocks {
        if s.contains("hopper") || s.contains("dropper") {
            let q = [p[0] + a[0], hi[1] + 1, p[2] + a[2]];
            put(&mut d, q, "glass")?;
        }
    }
    // the pulse: the pad's output dust (x1+1, wire_y+1, the last plane) east one onto x 10, then a
    // dust staircase down in the x = 10 plane (+z, one down a step, each on stone), a repeater on
    // a flat step halfway (the staircase's 15 steps would spend the signal), and west into the
    // counter's input at its height
    let from = pad.pulse();
    let to = counter_input(o)?;
    let x = from[0] + 1;
    put(&mut d, [x, from[1], from[2]], "redstone_wire[east=side,north=side,power=0,south=side,west=side]")?;
    put(&mut d, [x, from[1] - 1, from[2]], "stone")?;
    let dust = "redstone_wire[east=side,north=side,power=0,south=side,west=side]";
    let (mut y, mut z) = (from[1], from[2]);
    let drop = y - to[1];
    anyhow::ensure!(drop >= 4, "the pulse wire needs the counter's input below the pad");
    let half = drop / 2;
    let mut steps = 0;
    while y > to[1] {
        if steps == half {
            // a flat step: the repeater (facing +z reads the dust behind it)
            z += 1;
            put(&mut d, [x, y, z], "repeater[delay=1,facing=north,locked=false,powered=false]")?;
            put(&mut d, [x, y - 1, z], "stone")?;
            z += 1;
            put(&mut d, [x, y, z], dust)?;
            put(&mut d, [x, y - 1, z], "stone")?;
        }
        y -= 1;
        z += 1;
        put(&mut d, [x, y, z], dust)?;
        put(&mut d, [x, y - 1, z], "stone")?;
        steps += 1;
    }
    // flat along z at the input's height to the input's z, then along x into the input cell
    anyhow::ensure!(z <= to[2], "the pulse wire reaches z {z} past the counter's input at z {}: move the counter", to[2]);
    while z < to[2] {
        z += 1;
        put(&mut d, [x, y, z], dust)?;
        put(&mut d, [x, y - 1, z], "stone")?;
    }
    let mut xx = x;
    while xx != to[0] {
        xx += if to[0] < xx { -1 } else { 1 };
        put(&mut d, [xx, y, z], dust)?;
        put(&mut d, [xx, y - 1, z], "stone")?;
    }
    d.name = "carousel-v0".into();
    d.meta.insert("assembly".into(), o.meta());
    let mut s = String::new();
    for l in [
        "Assembly v0 (step a) of the iron carousel: ironfarm's release + hold, farmopt's station + pad + Nether part, day-1's counter v7 under the stand",
        "(glass over its ring), the pad's pulse wired down to the counter's input; the counter's lines drive the slope controls ideally (the bench reads each lane's comparator).",
    ] {
        s.push_str("# ");
        s.push_str(l);
        s.push('\n');
    }
    s.push_str(&d.to_text());
    Ok(s)
}

/// What the wiring check measured.
#[derive(Clone, Debug, Default)]
pub struct WiringReport {
    pub slots: u32,
    pub pulses: u32,
    pub advances: u32,
    /// Per end and level ("stand L0", "hold L2"...): the delays from a line's change to its rail's
    /// (min, max, count).
    pub delays: std::collections::BTreeMap<String, (i64, i64, u32)>,
    /// Rail changes away from what their line asks (crosstalk, glitches), and mismatches lasting
    /// over the tolerance.
    pub crosstalk: Vec<String>,
    pub crosstalk_count: u32,
    pub stuck: Vec<String>,
    pub stuck_count: u32,
    /// Ticks with two or more lines high.
    pub two_lines: u32,
}

/// The physical wiring's check (`mobfarm assembly-check`): the design's redstone alone on redsim
/// (no entities), a golem pulse (a redstone block over the pad's output dust for 10 gt, from the
/// tick after each slot) at every slot but a random `miss` share (the counter's fallback then
/// runs), for `slots` slots; every tick each lane's line (its tap) against its slope rails: the
/// stand slopes powered while the line is low, the hold slopes while it is high. Each rail change
/// toward its line's ask is a delay from the line's last change; each change away from it is
/// crosstalk; each mismatch lasting over `tolerance` gt is stuck.
pub fn wiring_check(spec: &redsim::entity::mob::farm::FarmSpec, lanes: &crate::hold::LaneOpts, pad: &crate::station::PadOpts, asm: &AsmOpts, slots: u32, miss: f64, seed: i64, tolerance: i64) -> Result<WiringReport, String> {
    use redsim::block::Block;
    use redsim::entity::mob::farm::{Run, RunOpts, WalkMode};
    let ro = RunOpts { ai: false, ..RunOpts::steady(seed, slots * 100 + 400, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let taps = counter_taps(asm).map_err(|e| e.to_string())?;
    let n = crate::hold::BATCHES as usize;
    // each lane's rails: (end, level, cell)
    let mut rails: Vec<Vec<(&'static str, i32, BlockPos)>> = Vec::new();
    for b in 0..n as i32 {
        let x = lanes.x(b);
        let mut v = Vec::new();
        for k in 0..lanes.levels {
            v.push(("stand", k, BlockPos::new(x, lanes.level_gap * k, lanes.stand_z0 + k)));
            v.push(("hold", k, BlockPos::new(x, lanes.level_gap * k, lanes.run - k)));
        }
        rails.push(v);
    }
    let powered = |w: &redsim::world::World, p: BlockPos| matches!(w.get(p), Block::Rail { activator: false, powered: true, .. });
    let line = |w: &redsim::world::World, p: BlockPos| matches!(w.get(p), Block::Comparator { powered: true, .. });
    let mut rep = WiringReport::default();
    let mut rng = redsim::rng::LegacyRandom::new(seed ^ 0x7769_7265);
    let pulse = pad.pulse();
    let src = BlockPos::new(pulse[0], pulse[1] + 1, pulse[2]);
    let rblock = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let fl = redsim::world::flags::ALL;
    let t0 = run.world.time;
    let first = (t0 / 100 + 2) * 100;
    let end = first + slots as i64 * 100;
    let mut lines: Vec<bool> = taps.iter().map(|p| line(&run.world, *p)).collect();
    let mut last_change: Vec<i64> = vec![t0; n];
    let mut rail_state: Vec<Vec<bool>> = rails.iter().map(|v| v.iter().map(|(_, _, p)| powered(&run.world, *p)).collect()).collect();
    let mut wrong_since: Vec<Vec<Option<i64>>> = rails.iter().map(|v| vec![None; v.len()]).collect();
    let mut off_at: Option<i64> = None;
    // the counter's power-on settles before the check counts anything
    let warm = first + 300;
    while run.world.time < end {
        let t = run.world.time + 1;
        if t >= first && (t - first) % 100 == 1 {
            rep.slots += 1;
            if (rng.next_float() as f64) >= miss {
                run.world.set_block(src, rblock.clone(), fl);
                off_at = Some(t + 10);
                rep.pulses += 1;
            }
        }
        if off_at == Some(t) {
            run.world.set_block(src, air.clone(), fl);
            off_at = None;
        }
        run.tick();
        let w = &run.world;
        let now: Vec<bool> = taps.iter().map(|p| line(w, *p)).collect();
        for b in 0..n {
            if now[b] != lines[b] {
                lines[b] = now[b];
                last_change[b] = t;
                if now[b] && t >= warm {
                    rep.advances += 1;
                }
            }
        }
        if t >= warm && now.iter().filter(|&&h| h).count() > 1 {
            rep.two_lines += 1;
        }
        for b in 0..n {
            for (j, (end_, k, p)) in rails[b].iter().enumerate() {
                let want = if *end_ == "stand" { !lines[b] } else { lines[b] };
                let got = powered(w, *p);
                if got != rail_state[b][j] {
                    rail_state[b][j] = got;
                    if t >= warm {
                        if got == want {
                            let d = t - last_change[b];
                            let e = rep.delays.entry(format!("{end_} L{k}")).or_insert((i64::MAX, i64::MIN, 0));
                            e.0 = e.0.min(d);
                            e.1 = e.1.max(d);
                            e.2 += 1;
                        } else {
                            rep.crosstalk_count += 1;
                            if rep.crosstalk.len() < 8 {
                                rep.crosstalk.push(format!("gt {t}: lane {b}'s {end_} rail L{k} at {p} turned {} while its line is {}", if got { "on" } else { "off" }, if lines[b] { "high" } else { "low" }));
                            }
                        }
                    }
                }
                if got != want {
                    let since = *wrong_since[b][j].get_or_insert(t);
                    if t >= warm && t - since == tolerance {
                        rep.stuck_count += 1;
                        if rep.stuck.len() < 8 {
                            rep.stuck.push(format!("gt {t}: lane {b}'s {end_} rail L{k} at {p} {} for {tolerance} gt while its line is {}", if got { "on" } else { "off" }, if lines[b] { "high" } else { "low" }));
                        }
                    }
                } else {
                    wrong_since[b][j] = None;
                }
            }
        }
    }
    Ok(rep)
}
