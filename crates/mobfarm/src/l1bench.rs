//! (onechunk, Nov 11; farmopt 22:07/22:12, ironspec's self-contained loaders) L1′, the cart-loop
//! loader, benched in redsim: a portal pair (overworld A, Nether B, both 2 x 3 at x 0..1, y 10..12,
//! z 0, axis x), and on each side an exit track (the landing flat one level below the flight, a
//! booster, a boosted 3-slope climb, a hairpin) into a 2-pocket queue (braked powered slopes) and
//! three flats back into the portal; 3 empty carts. A cart that crosses keeps its motion, flies
//! through the arrival portal (its bottom row one below the flight: never on ground there) and
//! lands on the exit flat; the 300-gt cooldown counts from when it leaves the frame, and every
//! tick a cart on cooldown spends inside a portal restarts it (`setAsInsidePortal`).
//!
//! EMULATED (labelled in the report): each pocket's power, set by putting a redstone block under
//! its powered rail: pocket 1 (the front) powered for `pulse` gt once its cart arrived `hold` gt
//! ago; pocket 2 powered while pocket 1 is empty and braked. The real circuit (a per-arrival delay
//! of `hold` gt that can hold 2 pending releases) is priced, not built.
//!
//! The report: each side's arrivals (a minecart id appearing in that world), their intervals
//! against the 300-gt PORTAL ticket, the Nether's ticking gaps (its ticket lapsing), every cart
//! held inside a portal (stuck: the cooldown restarting), the carts per side, each hold.

use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::EntityKind;
use redsim::pos::BlockPos;
use std::collections::{BTreeMap, BTreeSet};

type P = [i32; 3];

/// One side's cells (the frame `s` = +1: tracks at z >= 1, -1: z <= -1).
#[derive(Clone, Debug)]
pub struct Side {
    pub s: i32,
    pub out_x: i32,
    pub in_x: i32,
}

impl Side {
    fn z(&self, k: i32) -> i32 {
        self.s * k
    }
    /// The pocket rails (1 the front) and the support cells the emulated timer swaps. Pocket 2
    /// sits one slope above pocket 1 with a plain slope between them: consecutive powered rails
    /// share power (the third bench run's pocket 2 rolled with pocket 1's release).
    pub fn pocket(&self, n: usize) -> (P, P) {
        match n {
            1 => ([self.in_x, 11, self.z(4)], [self.in_x, 10, self.z(4)]),
            _ => ([self.in_x, 13, self.z(6)], [self.in_x, 12, self.z(6)]),
        }
    }
    fn blocks(&self, out: &mut BTreeMap<P, String>) {
        let away = if self.s > 0 { "south" } else { "north" };
        let rail = |shape: &str| format!("minecraft:rail[shape={shape},waterlogged=false]");
        let powered = |shape: &str, on: bool| format!("minecraft:powered_rail[powered={on},shape={shape},waterlogged=false]");
        let glass = "minecraft:glass".to_string();
        let rs = "minecraft:redstone_block".to_string();
        // the portal: a 10-obsidian frame (no corners), 6 portal blocks
        for y in 10..=12 {
            out.insert([-1, y, 0], "minecraft:obsidian".into());
            out.insert([2, y, 0], "minecraft:obsidian".into());
        }
        for x in 0..=1 {
            out.insert([x, 9, 0], "minecraft:obsidian".into());
            out.insert([x, 13, 0], "minecraft:obsidian".into());
            for y in 10..=12 {
                out.insert([x, y, 0], "minecraft:nether_portal[axis=x]".into());
            }
        }
        let (o, i) = (self.out_x, self.in_x);
        // the exit: the landing flat, a booster, four boosted slopes up, the hairpin
        out.insert([o, 10, self.z(1)], rail("north_south"));
        out.insert([o, 9, self.z(1)], glass.clone());
        out.insert([o, 10, self.z(2)], powered("north_south", true));
        out.insert([o, 9, self.z(2)], rs.clone());
        for (k, y) in [(3, 10), (4, 11), (5, 12), (6, 13)] {
            out.insert([o, y, self.z(k)], powered(&format!("ascending_{away}"), true));
            out.insert([o, y - 1, self.z(k)], rs.clone());
        }
        let toward = if self.s > 0 { "north" } else { "south" };
        let (oe, ie) = if i > o { ("east", "west") } else { ("west", "east") };
        out.insert([o, 14, self.z(7)], rail(&format!("{toward}_{oe}")));
        out.insert([o, 13, self.z(7)], glass.clone());
        out.insert([i, 14, self.z(7)], rail(&format!("{toward}_{ie}")));
        out.insert([i, 13, self.z(7)], glass.clone());
        // the queue: pocket 2, a plain slope, pocket 1 (braked powered slopes), three flats in
        for n in [2usize, 1] {
            let (r, sup) = self.pocket(n);
            out.insert(r, powered(&format!("ascending_{away}"), false));
            out.insert(sup, glass.clone());
        }
        out.insert([i, 12, self.z(5)], rail(&format!("ascending_{away}")));
        out.insert([i, 11, self.z(5)], glass.clone());
        for k in 1..=3 {
            // (k1, k2 boosters on redstone blocks: an empty cart slows 0.96 a tick on plain rail,
            // and the flight through the arrival portal needs ~0.4 b/t to clear it before it falls
            // onto the frame's obsidian; the first bench run's carts at 0.11-0.18 stalled there)
            if k <= 2 {
                // (k1, k2 powered rails on the release line: unpowered they brake a cart that
                // slipped past pocket 1, so it waits for its own release instead of entering the
                // portal on cooldown; the second bench run lost a cart that way)
                out.insert([i, 11, self.z(k)], powered("north_south", false));
                out.insert([i, 10, self.z(k)], glass.clone());
            } else {
                out.insert([i, 11, self.z(k)], rail("north_south"));
                out.insert([i, 10, self.z(k)], glass.clone());
            }
        }
    }
}

pub fn sides() -> (Side, Side) {
    (Side { s: 1, out_x: 0, in_x: 1 }, Side { s: -1, out_x: 1, in_x: 0 })
}

/// The bench design: A's cells in the overworld, B's in the Nether part (anchor 0 0 0: the Nether
/// cells as written).
pub fn design_text() -> String {
    let (a, b) = sides();
    let mut d = Design { name: "l1-cart-loop-bench".into(), ..Default::default() };
    a.blocks(&mut d.blocks);
    b.blocks(&mut d.nether_blocks);
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    d.ports.insert("output".into(), [8, 10, 8]);
    d.ports.insert("nether-anchor".into(), [0, 0, 0]);
    d.ports.insert("nether-axis".into(), [1, 0, 0]);
    let mut t = String::from("# L1' cart-loop loader bench (onechunk, Nov 11): the overworld portal A and its Nether twin B, each with an exit track into a 2-pocket queue; the pockets' power EMULATED by mobfarm l1-bench\n");
    t.push_str(&d.to_text());
    t
}

#[derive(Clone, Debug)]
pub struct Opts {
    pub ticks: i64,
    pub hold: i64,
    pub pulse: i64,
    /// The first releases, gt after the start (cart 0 from A's pocket 1, cart 2 from B's, cart 1
    /// from A's once it reaches pocket 1).
    pub stagger: [i64; 3],
    /// Entities frozen `f.1` gt from `f.0` (blocks and the emulated timers run): the login window.
    pub freezes: Vec<(i64, i64)>,
    pub trace: bool,
}

struct _Unused;

fn cart_ids(w: &redsim::world::World) -> Vec<(u32, [f64; 3], [f64; 3], i32)> {
    w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))).map(|e| (e.id, [e.pos.x, e.pos.y, e.pos.z], [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z], e.portal_cooldown)).collect()
}

fn near(p: [f64; 3], c: P) -> bool {
    (p[0] - (c[0] as f64 + 0.5)).abs() < 0.45 && (p[2] - (c[2] as f64 + 0.5)).abs() < 0.7 && p[1] > c[1] as f64 - 0.5 && p[1] < c[1] as f64 + 1.6
}

fn in_portal(p: [f64; 3]) -> bool {
    p[0] > -0.49 && p[0] < 2.49 && p[2] > -0.49 && p[2] < 1.49 && p[1] > 9.3 && p[1] < 13.0
}

pub fn bench(spec: &FarmSpec, o: &Opts) -> Result<String, String> {
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, (o.ticks + 100) as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    if run.world.nether.is_none() {
        return Err("no Nether part".into());
    }
    let (a, b) = sides();
    let rsrc = Block::parse("minecraft:redstone_block")?;
    let glass = Block::parse("minecraft:glass")?;
    // the carts: 0 in A's pocket 1, 1 in A's pocket 2, 2 in B's pocket 1 (rested on the slopes)
    let pos = |side: &Side, n: usize| {
        let (r, _) = side.pocket(n);
        [r[0] as f64 + 0.5, r[1] as f64 + 0.6, r[2] as f64 + 0.5]
    };
    redsim::entity::spawn(&mut run.world, "minecraft:minecart", pos(&a, 1), None)?;
    redsim::entity::spawn(&mut run.world, "minecraft:minecart", pos(&a, 2), None)?;
    {
        let n = run.world.nether.as_deref_mut().unwrap();
        redsim::entity::spawn(n, "minecraft:minecart", pos(&b, 1), None)?;
    }
    let start = run.world.time;
    // (each cart id's arrival time in its world; the initial carts: their scheduled first release)
    let mut arrived: BTreeMap<(usize, u32), i64> = BTreeMap::new();
    let ids0 = cart_ids(&run.world);
    let idsn0 = cart_ids(run.world.nether.as_deref().unwrap());
    let mut seen: [BTreeSet<u32>; 2] = [ids0.iter().map(|c| c.0).collect(), idsn0.iter().map(|c| c.0).collect()];
    // A's pocket 1 cart released at stagger 0, B's at stagger 1, A's pocket 2 cart at stagger 2
    if let Some(c) = ids0.iter().find(|c| near(c.1, a.pocket(1).0)) {
        arrived.insert((0, c.0), start + o.stagger[0] - o.hold);
    }
    if let Some(c) = ids0.iter().find(|c| near(c.1, a.pocket(2).0)) {
        arrived.insert((0, c.0), start + o.stagger[2] - o.hold);
    }
    if let Some(c) = idsn0.iter().find(|c| near(c.1, b.pocket(1).0)) {
        arrived.insert((1, c.0), start + o.stagger[1] - o.hold);
    }
    let mut arrivals: [Vec<i64>; 2] = [Vec::new(), Vec::new()];
    let mut stuck: Vec<String> = Vec::new();
    let mut in_portal_since: BTreeMap<(usize, u32), i64> = BTreeMap::new();
    let mut pulse_until = [i64::MIN / 4; 2];
    let mut nudge_until = [i64::MIN / 4; 2];
    let mut powered = [[false; 2]; 2];
    let mut releases: [Vec<(i64, i64)>; 2] = [Vec::new(), Vec::new()];
    let mut nether_dead: Vec<i64> = Vec::new();
    let mut log: Vec<String> = Vec::new();
    let mut frozen: Option<(Vec<(u32, [f64; 3], [f64; 3], i32)>, Vec<(u32, [f64; 3], [f64; 3], i32)>)> = None;
    let mut carts_seen_max = 0usize;
    for _ in 0..o.ticks {
        let t = run.world.time;
        let rel = t - start;
        for (wi, side) in [(0usize, &a), (1usize, &b)] {
            let world: &mut redsim::world::World = if wi == 0 { &mut run.world } else { run.world.nether.as_deref_mut().unwrap() };
            let cs = cart_ids(world);
            let present: BTreeSet<u32> = cs.iter().map(|c| c.0).collect();
            seen[wi].retain(|id| present.contains(id));
            // arrivals: an id new to this world
            for c in &cs {
                if !seen[wi].contains(&c.0) {
                    seen[wi].insert(c.0);
                    arrivals[wi].push(rel);
                    arrived.insert((wi, c.0), t);
                    if o.trace {
                        log.push(format!("t {rel}: arrival in {} (cart id {}) at ({:.2},{:.2},{:.2}) v ({:.3},{:.3},{:.3})", if wi == 0 { "A" } else { "B" }, c.0, c.1[0], c.1[1], c.1[2], c.2[0], c.2[1], c.2[2]));
                    }
                }
                // a cart held inside the portal (on cooldown, restarting) more than 40 gt
                if in_portal(c.1) {
                    let since = *in_portal_since.entry((wi, c.0)).or_insert(t);
                    if t - since == 40 {
                        stuck.push(format!("t {rel}: cart id {} inside {}'s portal for 40 gt at ({:.2},{:.2},{:.2}) v ({:.3},{:.3},{:.3}), cooldown {}", c.0, if wi == 0 { "A" } else { "B" }, c.1[0], c.1[1], c.1[2], c.2[0], c.2[1], c.2[2], c.3));
                    }
                } else {
                    in_portal_since.remove(&(wi, c.0));
                }
            }
            // the emulated pockets
            let (p1, s1) = side.pocket(1);
            let (p2, s2) = side.pocket(2);
            // (the front: a cart on pocket 1 or on the release line's cells k3..k1)
            let line: Vec<P> = vec![p1, [side.in_x, 11, side.s * 3], [side.in_x, 11, side.s * 2], [side.in_x, 11, side.s]];
            let in1 = cs.iter().find(|c| line.iter().any(|q| near(c.1, *q)));
            let in2 = cs.iter().any(|c| near(c.1, p2));
            let want1 = if t < pulse_until[wi] {
                true
            } else if let Some(c) = in1 {
                let due = arrived.get(&(wi, c.0)).copied().unwrap_or(i64::MAX / 4) + o.hold;
                if t >= due {
                    pulse_until[wi] = t + o.pulse;
                    releases[wi].push((rel, t - arrived.get(&(wi, c.0)).copied().unwrap_or(t)));
                    true
                } else {
                    false
                }
            } else {
                false
            };
            // (pocket 2 nudged 2 gt once pocket 1 and the line are clear and braked: a cart that
            // rolls down a powered slope for longer comes into pocket 1 too fast for its brake)
            if in1.is_none() && !want1 && in2 && nudge_until[wi] < t - 20 {
                nudge_until[wi] = t + 2;
            }
            let want2 = t < nudge_until[wi];
            let sup_k = [[side.in_x, 10, side.s * 2], [side.in_x, 10, side.s]];
            for (n, want, sup) in [(0usize, want1, s1), (1usize, want2, s2)] {
                if want != powered[wi][n] {
                    world.set_block(BlockPos::new(sup[0], sup[1], sup[2]), if want { rsrc.clone() } else { glass.clone() }, redsim::world::flags::ALL);
                    if n == 0 {
                        for k in sup_k {
                            world.set_block(BlockPos::new(k[0], k[1], k[2]), if want { rsrc.clone() } else { glass.clone() }, redsim::world::flags::ALL);
                        }
                    }
                    powered[wi][n] = want;
                }
            }
        }
        // the login window: entities frozen (positions, motions and cooldowns restored), blocks run
        let freeze_now = o.freezes.iter().any(|(f, d)| rel >= *f && rel < f + d);
        if freeze_now && frozen.is_none() {
            frozen = Some((cart_ids(&run.world), cart_ids(run.world.nether.as_deref().unwrap())));
        }
        run.tick();
        if let Some((fa, fb)) = &frozen {
            if freeze_now {
                for (wi, snap) in [(0usize, fa), (1usize, fb)] {
                    let world: &mut redsim::world::World = if wi == 0 { &mut run.world } else { run.world.nether.as_deref_mut().unwrap() };
                    for (id, p, v, cd) in snap {
                        if let Some(i) = redsim::entity::index_of(world, *id) {
                            let e = &mut world.entities.list[i];
                            e.set_pos(redsim::entity::Vec3::new(p[0], p[1], p[2]));
                            e.delta_movement = redsim::entity::Vec3::new(v[0], v[1], v[2]);
                            e.portal_cooldown = *cd;
                        }
                    }
                }
            } else {
                frozen = None;
            }
        }
        let n = run.world.nether.as_deref().unwrap();
        if n.portal_tickets.is_empty() && !arrivals[1].is_empty() {
            nether_dead.push(rel);
        }
        let total = cart_ids(&run.world).len() + cart_ids(n).len();
        carts_seen_max = carts_seen_max.max(total);
    }
    // ---- the report
    let mut s = format!("l1-bench (L1' cart loop, redsim): 3 empty carts, hold {} gt, pulse {} gt, stagger {:?}, {} gt; the pockets' power EMULATED (a redstone block set under each pocket's powered rail); crossings, the cooldown restart, the closest-portal search and the Nether's PORTAL ticket as redsim models them\n", o.hold, o.pulse, o.stagger, o.ticks);
    for (wi, name) in [(0usize, "A (overworld)"), (1usize, "B (Nether)")] {
        let ar = &arrivals[wi];
        let iv: Vec<i64> = ar.windows(2).map(|w| w[1] - w[0]).collect();
        let max = iv.iter().copied().max().unwrap_or(0);
        let min = iv.iter().copied().min().unwrap_or(0);
        let over = iv.iter().filter(|v| **v > 300).count();
        let holds: Vec<i64> = releases[wi].iter().map(|r| r.1).collect();
        s.push_str(&format!(
            "side {name}: {} arrivals; intervals {}..{} gt (target <= ~210, the ticket 300): {} over 300{}; releases {} (each cart's hold since its arrival {}..{} gt)\n",
            ar.len(),
            min,
            max,
            over,
            if over == 0 && !iv.is_empty() { " (PASS)" } else { " (FAIL)" },
            holds.len(),
            holds.iter().min().copied().unwrap_or(0),
            holds.iter().max().copied().unwrap_or(0)
        ));
        if o.trace {
            s.push_str(&format!("  arrivals at: {}\n", ar.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" ")));
        }
    }
    let last = run.world.time - start;
    let tail_a = arrivals[0].last().map(|v| last - v).unwrap_or(last);
    let tail_b = arrivals[1].last().map(|v| last - v).unwrap_or(last);
    s.push_str(&format!("since the last arrival at the end: A {tail_a} gt, B {tail_b} gt\n"));
    let dead = if nether_dead.is_empty() { " (PASS)".to_string() } else { format!(" (FAIL: from t {})", nether_dead[0]) };
    s.push_str(&format!("the Nether unticketed (its PORTAL ticket lapsed after the first arrival): {} gt{dead}\n", nether_dead.len()));
    s.push_str(&format!("carts held inside a portal 40 gt or more (the cooldown restarting): {}{}\n", stuck.len(), if stuck.is_empty() { " (PASS)" } else { " (FAIL)" }));
    for l in stuck.iter().take(6) {
        s.push_str(&format!("  {l}\n"));
    }
    let end_a = cart_ids(&run.world).len();
    let end_b = cart_ids(run.world.nether.as_deref().unwrap()).len();
    s.push_str(&format!("carts at the end: A {end_a}, B {end_b} (3 expected; the most at once {carts_seen_max})\n"));
    if !run.world.errors.is_empty() {
        s.push_str(&format!("model errors: {}\n", run.world.errors.join("; ")));
    }
    if let Some(n) = run.world.nether.as_deref()
        && !n.errors.is_empty()
    {
        s.push_str(&format!("Nether model errors: {}\n", n.errors.join("; ")));
    }
    if o.trace {
        for l in &log {
            s.push_str(&format!("  {l}\n"));
        }
        for (wi, side) in [(0usize, "A"), (1usize, "B")] {
            let w: &redsim::world::World = if wi == 0 { &run.world } else { run.world.nether.as_deref().unwrap() };
            for c in cart_ids(w) {
                s.push_str(&format!("  end {side}: cart {} at ({:.2},{:.2},{:.2}) v ({:.3},{:.3},{:.3}) cooldown {}\n", c.0, c.1[0], c.1[1], c.1[2], c.2[0], c.2[1], c.2[2], c.3));
            }
        }
    }
    Ok(s)
}
