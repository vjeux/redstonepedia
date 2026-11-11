//! R1's queue-contact scenes for tick-exact's merge bridge (ironspec Nov 11 09:55: the bar is the
//! arriving cart's stored speed the tick before first contact at or under 0.6, or the contact
//! geometry bridged merge-free to 1.15 times the highest arrival; tick-exact's ask the same hour).
//!
//! The escapement column of a queue-loop design (its x, from 5 cells before G0 to 3 after, 4 below
//! to 5 above: the descent's last slopes, G0 over P, P on its block, G_P over R, slot 0 on its
//! control) is copied into one lane per rider pairing, lane i at x + 2i: RR, RE, ER, EE (the waiting
//! cart's rider, then the arrival's: R ridden, E empty). P's block is a redstone block (P always on,
//! as R1 runs). One scene per arrival speed:
//! - case g0: G0 shut; the waiting cart pressed at G0's rest (its centre 0.115 short of G0's cell);
//!   the arrival on the slope 3 cells before G0 (R1: (0,5,28)), at the cell's middle, moving toward
//!   G0 at the stored speed, down that slope and the next into the waiting cart;
//! - case p: G0 open; P's cart at rest on P, pressed on G_P (shut); the arrival on the slope 2
//!   cells before G0 (R1: (0,4,29)), moving toward G0 at the stored speed, through G0 onto P.
//!
//! Each scene also runs on redsim here, scored as `mctas mobai rec-carts` scores the game's
//! recording: the first contact (horizontal centres under 1.18 and heights under 0.7 apart), the
//! arriving cart's stored speed (|motion| in x and z) the tick before, the closest centres over the
//! run, and a merge (centres under 0.01 apart).

use mctas::design::{Design, EntitySpec};
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::EntityKind;

/// The rider pairings, one lane each (lane i at x + 2i): (name, the waiting cart ridden, the
/// arrival ridden).
pub const PAIRS: [(&str, bool, bool); 4] = [("RR", true, true), ("RE", true, false), ("ER", false, true), ("EE", false, false)];

/// Which contact a scene holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// An arrival into the queue's front pressed at G0.
    G0,
    /// The cart admitted through G0 meeting P's cart.
    P,
}

impl Case {
    pub fn tag(self) -> &'static str {
        match self {
            Case::G0 => "g0",
            Case::P => "p",
        }
    }
}

/// One scene's design: `src` a queue-loop design with the escapement (`--esc-adm`), `v` the
/// arrival's stored speed (b/t), `front_frac` where the waiting cart starts along its cell (0.885:
/// at rest pressed on the gate, as the design's own tail carts rest; 0.5: the cell's middle, rolling
/// into the gate from rest as tick-exact's slope-merge lanes set theirs).
pub fn scene(src: &Design, case: Case, v: f64, front_frac: f64) -> Result<Design, String> {
    let o = crate::queue::loop_opts_of(&src.meta).map_err(|e| e.to_string())?;
    let lay = crate::queue::loop_layout(&o);
    let g0 = lay.esc_g0.ok_or("not an escapement design (no G0)")?;
    let p_ctl = lay.esc_p.ok_or("not an escapement design (no P control)")?;
    let (gp, p_rest) = *lay.fifo.last().ok_or("not an escapement design (no G_P)")?;
    // (the queue runs toward G0 along z: the travel direction from the cell before G0)
    let dir: i32 = if src.blocks.contains_key(&[g0.x, g0.y, g0.z - 1]) { 1 } else { -1 };
    let along = |k: i32| g0.z + dir * k;
    let (z_lo, z_hi) = if dir > 0 { (along(-5), along(3)) } else { (along(3), along(-5)) };
    let col: Vec<([i32; 3], String)> = src
        .blocks
        .iter()
        .filter(|(q, _)| q[0] == g0.x && (g0.y - 4..=g0.y + 5).contains(&q[1]) && (z_lo..=z_hi).contains(&q[2]))
        .map(|(q, s)| (*q, s.clone()))
        .collect();
    let g0_state = src.blocks.get(&[g0.x, g0.y, g0.z]).ok_or("no block at G0")?;
    if !g0_state.contains("fence_gate") {
        return Err(format!("G0 ({},{},{}) holds {g0_state}, not a fence gate", g0.x, g0.y, g0.z));
    }
    let name = format!("r1-contact-{}-{:03}", case.tag(), (v * 100.0).round() as i64);
    let mut d = Design { name: name.clone(), ..Default::default() };
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    let cart = |x: f64, y: f64, z: f64, sp: f64, ridden: bool| {
        let rider = if ridden { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
        EntitySpec { kind: "minecraft:minecart".into(), pos: [x, y, z], nbt: Some(format!("{{Motion:[0.0d,0.0d,{:.4}d]{rider}}}", dir as f64 * sp)) }
    };
    for (i, (_, wait_r, arr_r)) in PAIRS.iter().enumerate() {
        let dx = 2 * i as i32;
        for (q, s) in &col {
            let mut s = s.clone();
            if *q == [g0.x, g0.y, g0.z] && case == Case::P {
                s = s.replace("open=false", "open=true");
            }
            if *q == [p_ctl.x, p_ctl.y, p_ctl.z] {
                s = "minecraft:redstone_block".into();
            }
            d.blocks.insert([q[0] + dx, q[1], q[2]], s);
        }
        let x = (g0.x + dx) as f64 + 0.5;
        // (a cart on a slope descending along the travel direction: y = the cell's y + its rise
        // left to the cell's far edge + 0.0625)
        let on_slope = |cell: [i32; 3], frac: f64| {
            let z = cell[2] as f64 + if dir > 0 { frac } else { 1.0 - frac };
            (cell[1] as f64 + (1.0 - frac) + 0.0625, z)
        };
        match case {
            Case::G0 => {
                // (the front at G0's rest: 0.115 short of G0's cell, as the design's own tail
                // carts rest there)
                let (yf, zf) = on_slope([g0.x, g0.y, along(-1)], front_frac);
                d.entities.push(cart(x, yf, zf, 0.0, *wait_r));
                let (ya, za) = on_slope([g0.x, g0.y + 2, along(-3)], 0.5);
                d.entities.push(cart(x, ya, za, v, *arr_r));
            }
            Case::P => {
                let (yp, zp) = on_slope([p_rest[0], p_rest[1], p_rest[2]], front_frac);
                d.entities.push(cart(x, yp, zp, 0.0, *wait_r));
                let (ya, za) = on_slope([g0.x, g0.y + 1, along(-2)], 0.5);
                d.entities.push(cart(x, ya, za, v, *arr_r));
            }
        }
    }
    // (the output port: an air cell beside the lanes, as a mob-farm design needs one)
    let out = [g0.x - 2, g0.y, g0.z];
    d.ports.insert("output".into(), out);
    let _ = gp;
    Ok(d)
}

/// The scene's caption (its leading comment lines) and its drive (no acts: the carts move from
/// their spawn).
pub fn caption(case: Case, v: f64, src_name: &str) -> String {
    let what = match case {
        Case::G0 => "an arrival into the queue's front pressed at G0 (G0 shut): the front at G0's rest, the arrival at the middle of the slope 3 cells before G0, moving toward G0",
        Case::P => "the cart admitted through G0 meeting P's cart (G0 open): P's cart at rest on P pressed on G_P, the arrival at the middle of the slope 2 cells before G0, moving toward G0",
    };
    format!(
        "# R1's queue contact for the merge bridge (ironfarm, ironspec Nov 11 09:55): {what}, at stored {v:.2} b/t.\n# Lanes 2 apart in x, one per rider pairing (the waiting cart's, then the arrival's: R ridden, E empty): RR, RE, ER, EE.\n# The escapement column copied from {src_name} (mctas mobfarm contact-scenes); P on a redstone block (always on, as R1 runs). No zombies.\n"
    )
}

pub fn drive(case: Case, v: f64, ticks: u32) -> String {
    format!(
        "seed 1\ndays 0\nwalk random\n# ticks {ticks}\n# mctas mobfarm contact-scenes (case {}, arrival stored {v:.2} b/t)\n# start: Run::start(spec, RunOpts {{ ai: true, ..steady(seed, ticks, Random) }}); settle_tick until false; spawn the design's entity lines in order (each cart's Motion as written); t0 = the time right after that spawn; no acts\n",
        case.tag()
    )
}

/// One lane's score on redsim.
#[derive(Clone, Debug)]
pub struct LaneScore {
    pub pair: &'static str,
    /// The first contact: (tick, the arriving cart's stored speed the tick before, horizontal
    /// centres then).
    pub contact: Option<(u32, f64, f64)>,
    /// The closest centres over the run (3D) and the tick.
    pub closest: (f64, u32),
    /// Merged at the end (centres under 0.01 apart).
    pub merged_end: bool,
}

/// The scene run on redsim: settled with no entities, then the design's entity lines spawned in
/// order, `ticks` gt; per lane (carts sharing a block x) the rec-carts score.
pub fn score(spec: &FarmSpec, ticks: u32) -> Result<Vec<LaneScore>, String> {
    let mut s = spec.clone();
    let ents = std::mem::take(&mut s.entities);
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, ticks + 20, WalkMode::Random) };
    let mut run = Run::start(&s, &ro)?;
    while run.settle_tick() {}
    let mut ids: Vec<u32> = Vec::new();
    for (ty, pos, nbt) in &ents {
        ids.push(redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?);
    }
    // (per lane x: the carts' ids)
    let mut lanes: std::collections::BTreeMap<i64, Vec<u32>> = std::collections::BTreeMap::new();
    for (k, (ty, pos, _)) in ents.iter().enumerate() {
        if ty.ends_with("minecart") {
            lanes.entry(pos[0].floor() as i64).or_default().push(ids[k]);
        }
    }
    let state = |run: &Run, id: u32| run.world.entities.list.iter().find(|e| e.id == id && matches!(e.kind, EntityKind::Minecart(_))).map(|e| (e.pos, e.delta_movement.x.hypot(e.delta_movement.z)));
    let mut out: Vec<LaneScore> = lanes.keys().enumerate().map(|(i, _)| LaneScore { pair: PAIRS.get(i).map_or("?", |p| p.0), contact: None, closest: (f64::INFINITY, 0), merged_end: false }).collect();
    let mut prev: std::collections::HashMap<u32, f64> = std::collections::HashMap::new();
    for t in 0..ticks {
        run.tick();
        for (i, (_, cs)) in lanes.iter().enumerate() {
            let st: Vec<(u32, redsim::entity::Vec3, f64)> = cs.iter().filter_map(|id| state(&run, *id).map(|(p, v)| (*id, p, v))).collect();
            if st.len() == 2 {
                let (a, b) = (&st[0], &st[1]);
                let h = (a.1.x - b.1.x).hypot(a.1.z - b.1.z);
                let d3 = (h * h + (a.1.y - b.1.y).powi(2)).sqrt();
                if d3 < out[i].closest.0 {
                    out[i].closest = (d3, t);
                }
                if out[i].contact.is_none() && h < 1.18 && (a.1.y - b.1.y).abs() < 0.7 {
                    // (the arriving cart: the faster of the two the tick before)
                    let (va, vb) = (prev.get(&a.0).copied().unwrap_or(0.0), prev.get(&b.0).copied().unwrap_or(0.0));
                    out[i].contact = Some((t, va.max(vb), h));
                }
                out[i].merged_end = d3 < 0.01;
            }
            for (id, _, v) in &st {
                prev.insert(*id, *v);
            }
        }
    }
    Ok(out)
}

/// The score's lines for one scene.
pub fn score_lines(name: &str, scores: &[LaneScore]) -> Vec<String> {
    scores
        .iter()
        .map(|l| {
            format!(
                "{name} {}: {}; the closest centres {:.4} (t{}){}",
                l.pair,
                l.contact.map_or("no contact".to_string(), |(t, v, h)| format!("first contact t{t} (horizontal centres {h:.4}), the arrival's stored speed the tick before {v:.4}")),
                l.closest.0,
                l.closest.1,
                if l.merged_end { "  MERGED at the end" } else { "" }
            )
        })
        .collect()
}
