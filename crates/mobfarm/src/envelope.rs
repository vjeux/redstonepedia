//! (9fc3155f, ironspec Nov 11 21:24: the smallest one-bed farm) The rules-only envelope of a layout
//! family: the station (3 summoners over a pad whose golem cells are v4's measured 44), the waiting
//! villagers out of every lockout (rule 4: sight 18.84 sideways, 20.54 above / 19.79 below a
//! golem's feet; the mark 10.6 sideways / 11.95 vertically, contained in the sight box), placed by
//! the family (F1 above, F2 sideways, F3 below, F5 beside with no clearance), the queue as Q1 (one
//! sloped lane, a block a cart) or Q2 (3 parallel lanes), the bed (rule 6: every non-napper >= 17
//! from it, its tier a fixed box) placed where it grows the envelope least, and the pitch a copy of
//! the family tiles at (a neighbour's waiting villagers against this station's golems and
//! summoners, out of phase). Every constant is printed with its source.

use redsim::entity::Vec3;

/// The rule constants (tags: code / derived / verify).
pub struct Consts {
    pub sight_side: f64,
    pub sight_above: f64,
    pub sight_below: f64,
    pub mark_side: f64,
    pub mark_vert: f64,
    pub bed_r: f64,
    pub margin: f64,
    pub lift: f64,
    pub cart: f64,
    /// The bed tier's box (x, y, z) around the bed head (v4's minimal tier: 7 x 6 x 11).
    pub tier: [i32; 3],
    /// Scarer posts' room beside a waiting row (S1: zombies 6.05-7.95 away).
    pub posts: i32,
}

pub const C: Consts = Consts { sight_side: 18.84, sight_above: 20.54, sight_below: 19.79, mark_side: 10.6, mark_vert: 11.95, bed_r: 17.0, margin: 1.0, lift: 0.695, cart: 0.4, tier: [7, 7, 11], posts: 0 };

/// The station, in v4's file frame: the summoners' feet, the golem cells' feet centres, its block
/// box (the stand, the pad and the portal below it).
pub struct Station {
    pub summoners: Vec<Vec3>,
    pub golems: Vec<Vec3>,
    pub lo: [i32; 3],
    pub hi: [i32; 3],
}

/// v4's station (main d9b86b133): summoners 13.885/9.885/5.885 at z 6.5, feet -1.8225/-0.8225/0.1775;
/// the 44 golem cells x 4..13, z 0..5, feet -6; blocks x 3..15, y -11..2, z 0..8.
pub fn station_v4() -> Station {
    let summoners = vec![Vec3::new(13.885, -1.8225, 6.5), Vec3::new(9.885, -0.8225, 6.5), Vec3::new(5.885, 0.1775, 6.5)];
    let mut golems = Vec::new();
    for x in 4..=13 {
        for z in 0..=5 {
            golems.push(Vec3::new(x as f64 + 0.5, -6.0, z as f64 + 0.5));
        }
    }
    Station { summoners, golems, lo: [3, -11, 0], hi: [15, 2, 8] }
}

/// The queue option.
#[derive(Clone, Copy)]
pub enum Queue {
    /// One sloped lane of `carts` waiting carts, a block a cart.
    Q1(usize),
    /// `lanes` parallel sloped lanes of `per` waiting carts each.
    Q2(usize, usize),
}

/// One family's layout: the waiting riders' feet, the boxes (with their names) and notes.
pub struct Layout {
    pub family: &'static str,
    pub riders: Vec<Vec3>,
    pub boxes: Vec<([i32; 3], [i32; 3], String)>,
    pub notes: Vec<String>,
}

/// The out-of-range bounds of `st` (a rider's centre and feet clear of every golem's sight box,
/// which contains every summoner's mark box): x <= .0 or x >= .1, z <= .2 or z >= .3, feet >= .4
/// or <= .5.
pub fn clear_bounds(st: &Station) -> [f64; 6] {
    let gx0 = st.golems.iter().map(|g| g.x).fold(f64::MAX, f64::min);
    let gx1 = st.golems.iter().map(|g| g.x).fold(f64::MIN, f64::max);
    let gz0 = st.golems.iter().map(|g| g.z).fold(f64::MAX, f64::min);
    let gz1 = st.golems.iter().map(|g| g.z).fold(f64::MIN, f64::max);
    let gy0 = st.golems.iter().map(|g| g.y).fold(f64::MAX, f64::min);
    let gy1 = st.golems.iter().map(|g| g.y).fold(f64::MIN, f64::max);
    [gx0 - C.sight_side, gx1 + C.sight_side, gz0 - C.sight_side, gz1 + C.sight_side, gy1 + C.sight_above, gy0 - C.sight_below]
}

/// A queue's waiting riders from a start: Q1 a 45-degree lane folded around the rectangle x0..x1,
/// z0..z1 (perimeter cells), its lowest cart at feet `base`, each next cart a block higher (or
/// lower with `down`); Q2 parallel lanes along x from x0 at z0, z0 + 2, ..., each cart a block
/// along and a block up.
pub fn queue_riders(q: Queue, x0: i32, x1: i32, z0: i32, z1: i32, base: f64, down: bool) -> Vec<Vec3> {
    let mut out = Vec::new();
    let dy = if down { -1.0 } else { 1.0 };
    match q {
        Queue::Q1(n) => {
            let mut ring: Vec<(i32, i32)> = Vec::new();
            for x in x0..=x1 {
                ring.push((x, z0));
            }
            for z in z0 + 1..=z1 {
                ring.push((x1, z));
            }
            for x in (x0..x1).rev() {
                ring.push((x, z1));
            }
            for z in (z0 + 1..z1).rev() {
                ring.push((x0, z));
            }
            for k in 0..n {
                let (x, z) = ring[k % ring.len().max(1)];
                out.push(Vec3::new(x as f64 + 0.5, base + dy * k as f64, z as f64 + 0.5));
            }
        }
        Queue::Q2(lanes, per) => {
            for l in 0..lanes {
                for k in 0..per {
                    // (lanes 1.0 apart never exchange speed: tick-exact, Nov 11)
                    out.push(Vec3::new((x0 + k as i32) as f64 + 0.5, base + dy * k as f64, (z0 + l as i32) as f64 + 0.5));
                }
            }
        }
    }
    out
}

/// The cell box around riders (their rails below, their bodies above, `side` cells of posts
/// beside them in x and z).
pub fn rider_box(r: &[Vec3], side: i32) -> ([i32; 3], [i32; 3]) {
    let x0 = r.iter().map(|p| p.x.floor() as i32).min().unwrap_or(0) - side;
    let x1 = r.iter().map(|p| p.x.floor() as i32).max().unwrap_or(0) + side;
    let y0 = r.iter().map(|p| p.y.floor() as i32).min().unwrap_or(0) - 1;
    let y1 = r.iter().map(|p| p.y.floor() as i32).max().unwrap_or(0) + 2;
    let z0 = r.iter().map(|p| p.z.floor() as i32).min().unwrap_or(0) - side;
    let z1 = r.iter().map(|p| p.z.floor() as i32).max().unwrap_or(0) + side;
    ([x0, y0, z0], [x1, y1, z1])
}

/// The bounding box of the boxes.
pub fn bound(bs: &[([i32; 3], [i32; 3], String)]) -> ([i32; 3], [i32; 3]) {
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for (a, b, _) in bs {
        for k in 0..3 {
            lo[k] = lo[k].min(a[k]);
            hi[k] = hi[k].max(b[k]);
        }
    }
    (lo, hi)
}

/// The volume of an inclusive box.
pub fn vol(lo: [i32; 3], hi: [i32; 3]) -> i64 {
    (0..3).map(|k| (hi[k] - lo[k] + 1) as i64).product()
}

/// The bed placed where its tier grows the envelope least, every rider (the station's summoners
/// and the waiting ones) >= 17 from the bed head's centre; the tier box (`tier`) centred on the
/// head; a tier inside the module's chunk columns (x 0..15, z 0..15) first, else anywhere.
pub fn place_bed(lay: &mut Layout, st: &Station, tlo: [i32; 3], thi: [i32; 3], name: &str) {
    let all: Vec<Vec3> = st.summoners.iter().chain(lay.riders.iter()).copied().collect();
    let (lo, hi) = bound(&lay.boxes);
    let search = |inside: bool| -> Option<(i64, [i32; 3])> {
        let mut best: Option<(i64, [i32; 3])> = None;
        for y in lo[1] - 30..=hi[1] + 30 {
            for z in lo[2] - 20..=hi[2] + 20 {
                for x in lo[0] - 20..=hi[0] + 20 {
                    let t0 = [x + tlo[0], y + tlo[1], z + tlo[2]];
                    let t1 = [x + thi[0], y + thi[1], z + thi[2]];
                    if inside && (t0[0] < 0 || t1[0] > 15 || t0[2] < 0 || t1[2] > 15) {
                        continue;
                    }
                    let c = Vec3::new(x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5);
                    if all.iter().any(|r| ((r.x - c.x).powi(2) + (r.y - c.y).powi(2) + (r.z - c.z).powi(2)).sqrt() < C.bed_r) {
                        continue;
                    }
                    let mut lo2 = lo;
                    let mut hi2 = hi;
                    for k in 0..3 {
                        lo2[k] = lo2[k].min(t0[k]);
                        hi2[k] = hi2[k].max(t1[k]);
                    }
                    let v = vol(lo2, hi2);
                    if best.is_none_or(|b| v < b.0) {
                        best = Some((v, [x, y, z]));
                    }
                }
            }
        }
        best
    };
    let (best, inside) = match search(true) {
        Some(b) => (Some(b), true),
        None => (search(false), false),
    };
    if let Some((_, b)) = best {
        let t0 = [b[0] + tlo[0], b[1] + tlo[1], b[2] + tlo[2]];
        let t1 = [b[0] + thi[0], b[1] + thi[1], b[2] + thi[2]];
        let d = all.iter().map(|r| ((r.x - b[0] as f64 - 0.5).powi(2) + (r.y - b[1] as f64 - 0.5).powi(2) + (r.z - b[2] as f64 - 0.5).powi(2)).sqrt()).fold(f64::MAX, f64::min);
        lay.boxes.push((t0, t1, format!("{name} (head {},{},{}, least rider {:.2}{})", b[0], b[1], b[2], d, if inside { "" } else { "; no place inside the chunk's columns" })));
    }
}

/// A rough block count of a layout's parts (the rules-only estimate; every figure a part budget to
/// revise): the station 401 (v4's committed stand, pad and portal), 3 a waiting cart (rail,
/// support, a share of the posts), the bed tier 257 (today's tier + 1 level), the transport
/// (`transport`), the controller (`ctrl`) and the loader's share (`loader`).
pub fn blocks(n_wait: usize, transport: i64, ctrl: i64, loader: i64) -> i64 {
    401 + 3 * n_wait as i64 + 200 + transport + ctrl + loader
}


/// One family's layout around `st` with queue `q`.
pub fn layout(family: &str, q: Queue, st: &Station, z4: bool) -> Result<Layout, String> {
    let cb = clear_bounds(st);
    let mut lay = Layout { family: "", riders: Vec::new(), boxes: vec![(st.lo, st.hi, "station (stand, pad, portal)".into())], notes: Vec::new() };
    let n_wait = match q {
        Queue::Q1(n) => n,
        Queue::Q2(l, p) => l * p,
    };
    match family {
        "F1" => {
            lay.family = "F1 stack";
            let base = cb[4] + C.margin;
            lay.riders = queue_riders(q, 0, 15, 0, 15, base, false);
            lay.notes.push(format!("waiting riders' feet >= {:.2} (the pad's golem feet + 20.54, +{} margin): rule 4b above", cb[4], C.margin));
            lay.notes.push(format!("entry: M1 drop from the hold into the pockets ({:.1} blocks); exit: M2 lift from the stand to the queue's top", base));
            let top = lay.riders.iter().map(|r| r.y).fold(f64::MIN, f64::max);
            lay.notes.push(format!("the departing batch clears 14.54 by lift in {:.0} gt; the queue's top {:.1}", (cb[4] - 0.0) / C.lift, top));
            let t = 1.0 + 3.0 / C.cart + (cb[4] + 2.0) / C.lift;
            lay.notes.push(format!("normal-running bar (>= 5 gt): out of range at about T+{t:.0} (release +1, 3 cells to the lift, the lift from the stand's feet to {:.2}): margin {:.0} gt", cb[4], 100.0 - t));
        }
        "F2" => {
            lay.family = "F2 side";
            let zq = (cb[3] + C.margin).ceil() as i32;
            let base = (st.hi[1] + 1) as f64;
            lay.riders = match q {
                Queue::Q1(_) => queue_riders(q, 0, 15, zq, zq + 15, base, false),
                Queue::Q2(..) => queue_riders(q, 0, 15, zq, zq + 3, base, false),
            };
            lay.notes.push(format!("waiting riders' centres z >= {:.2} (the pad's last golem row + 18.84, +{} margin): rule 4b sideways", cb[3], C.margin));
            lay.notes.push(format!("exit: M4 sideways {:.1} blocks by T+100 ({:.0} gt at {} b/t); entry: the hold rolls back {:.1} blocks", cb[3] - st.summoners[0].z, (cb[3] - st.summoners[0].z) / C.cart, C.cart, cb[3] - st.summoners[0].z));
            let t = 1.0 + (cb[3] - st.summoners[0].z) / C.cart;
            lay.notes.push(format!("normal-running bar (>= 5 gt): out of range at about T+{t:.0} (release +1, the sideways run at the cap): margin {:.0} gt", 100.0 - t));
        }
        "F3" => {
            lay.family = "F3 inverted stack";
            let base = cb[5] - C.margin;
            lay.riders = queue_riders(q, 0, 15, 0, 15, base, true);
            let bottom = lay.riders.iter().map(|r| r.y).fold(f64::MAX, f64::min);
            lay.notes.push(format!("waiting riders' feet <= {:.2} (the pad's golem feet - 19.79, -{} margin): rule 4b below", cb[5], C.margin));
            lay.notes.push(format!("exit: M1 drop to the queue's top ({:.1} blocks); entry: M2 lift {:.1} blocks in {:.0} gt", -base, -bottom, -bottom / C.lift));
            // (a cart's free fall: 0.04 b/t^2 down, its motion x 0.98 a tick in the air)
            let (mut y, mut v, mut t) = (0.0f64, 0.0f64, 0);
            while y > cb[5] && t < 400 {
                v = (v - 0.04) * 0.98;
                y += v;
                t += 1;
            }
            lay.notes.push(format!("normal-running bar (>= 5 gt): out of range at about T+{} (release +1, the fall to {:.2}): margin {} gt", 1 + t, cb[5], 100 - 1 - t));
        }
        "F5" => {
            lay.family = "F5 close";
            let base = (st.hi[1] + 1) as f64;
            lay.riders = queue_riders(q, 0, 15, 9, 15, base, false);
            lay.notes.push("no clearance: every waiting rider inside a golem's sight box (golems live ~1 tick): ~1/200 lock per rider per golem; 51b2aacd: no count meets a golem every 100 gt".into());
        }
        _ => return Err(format!("unknown family {family} (F1, F2, F3, F5)")),
    }
    let (a, b) = rider_box(&lay.riders, C.posts);
    lay.boxes.push((a, b, format!("queue ({n_wait} waiting{}, S1 posts {} beside)", match q { Queue::Q1(_) => ", Q1".to_string(), Queue::Q2(l, p) => format!(", Q2 {l} x {p}") }, C.posts)));
    if z4 {
        // (Z4, 58f770cc's estimate: 3 beds side by side, 31 x 7 x 14 (stacked 11 x 21 x 14), each
        // bed 17 from its own lane's riders; with the beds and lanes a cell apart that is every
        // rider from the tier, as for one bed)
        place_bed(&mut lay, st, [-3, -2, -9], [27, 4, 4], "bed tier Z4 (3 beds)");
    } else {
        // (58f770cc's family-agnostic bed stop, explore/farmopt-station fdf06af67: 11 x 7 x 14
        // around the head at local (-3,0,4): x -6..4, y -2..4 (rider air to 4.3), z -5..8)
        place_bed(&mut lay, st, [-3, -2, -9], [7, 4, 4], "bed tier (1 bed)");
    }
    Ok(lay)
}

/// The transport's blocks (estimates): M2 lift ~8H + 30 (7b90c3f1's bench), one a lane; M1 drop hold
/// cells ~30 a lane's hold; M4 sideways and return tracks 2 a cell; the drop chute ~10 a lane.
pub fn transport_blocks(family: &str, q: Queue, lay: &Layout, st: &Station) -> (i64, String) {
    let lanes = match q {
        Queue::Q1(_) => 1,
        Queue::Q2(l, _) => l as i64,
    };
    let top = lay.riders.iter().map(|r| r.y).fold(f64::MIN, f64::max);
    let bottom = lay.riders.iter().map(|r| r.y).fold(f64::MAX, f64::min);
    let feet = st.summoners.iter().map(|s| s.y).fold(f64::MAX, f64::min);
    match family {
        "F1" => {
            let h = (top - feet).ceil() as i64 + 2;
            (lanes * (8 * h + 30) + lanes * 30, format!("M2 lift a lane ({lanes} x (8 x {h} + 30)) + M1 drop hold cells ({lanes} x 30)"))
        }
        "F2" => {
            let run = (lay.riders.iter().map(|r| r.z).fold(f64::MIN, f64::max) - st.summoners[0].z).ceil() as i64;
            (lanes * 2 * 2 * run, format!("M4 out and back, 2 tracks a lane of {run} cells at 2 blocks ({lanes} lanes)"))
        }
        "F3" => {
            let h = (feet - bottom).ceil() as i64 + 2;
            (lanes * (8 * h + 30) + lanes * 10, format!("M2 lift a lane ({lanes} x (8 x {h} + 30)) + M1 drop chutes ({lanes} x 10)"))
        }
        _ => (lanes * 30, format!("short links ({lanes} x 30)")),
    }
}

/// The binding boxes of the envelope per axis: the box setting each axis's low and high end.
pub fn binding(lay: &Layout) -> [String; 3] {
    let (lo, hi) = bound(&lay.boxes);
    let name = |k: usize, low: bool| -> String {
        lay.boxes
            .iter()
            .find(|(a, b, _)| if low { a[k] == lo[k] } else { b[k] == hi[k] })
            .map_or("-".to_string(), |(_, _, n)| n.split(" (").next().unwrap_or("").to_string())
    };
    [0, 1, 2].map(|k| format!("{} .. {}", name(k, true), name(k, false)))
}

/// The least pitch of a pair mirrored in z (the copy's z' = d - z, its box beyond this one's) at
/// which both clear each other (out of phase); the pair's z period is then the least `d` for a
/// third module the same way round as this one, returned too.
pub fn pitch_mirror_z(lay: &Layout, st: &Station) -> (i32, i32) {
    let ours: Vec<Vec3> = st.summoners.iter().chain(lay.riders.iter()).copied().collect();
    let (lo, hi) = bound(&lay.boxes);
    let ok = |gs: &[Vec3], ss: &[Vec3], rs: &[Vec3]| rs.iter().all(|r| gs.iter().all(|g| crate::tile::sight_margin(*g, *r) >= 0.0) && ss.iter().all(|s| crate::tile::mark_margin(*s, *r) >= 0.0));
    let mut pair = -1;
    for d in (2 * hi[2] + 1)..(2 * hi[2] + 200) {
        let m = |p: Vec3| Vec3::new(p.x, p.y, d as f64 - p.z);
        let (tr, tg, ts): (Vec<Vec3>, Vec<Vec3>, Vec<Vec3>) = (ours.iter().map(|p| m(*p)).collect(), st.golems.iter().map(|p| m(*p)).collect(), st.summoners.iter().map(|p| m(*p)).collect());
        if ok(&st.golems, &st.summoners, &tr) && ok(&tg, &ts, &ours) {
            pair = d;
            break;
        }
    }
    // (the next pair: this module again at +P, against the mirrored copy at d)
    let mut period = -1;
    if pair > 0 {
        let m = |p: Vec3| Vec3::new(p.x, p.y, pair as f64 - p.z);
        let (mr, mg, ms): (Vec<Vec3>, Vec<Vec3>, Vec<Vec3>) = (ours.iter().map(|p| m(*p)).collect(), st.golems.iter().map(|p| m(*p)).collect(), st.summoners.iter().map(|p| m(*p)).collect());
        let mz1 = pair - lo[2];
        for p in (mz1 + 1 - lo[2])..(mz1 + 300) {
            let sh = |q: Vec3| Vec3::new(q.x, q.y, q.z + p as f64);
            let (nr, ng, ns): (Vec<Vec3>, Vec<Vec3>, Vec<Vec3>) = (ours.iter().map(|q| sh(*q)).collect(), st.golems.iter().map(|q| sh(*q)).collect(), st.summoners.iter().map(|q| sh(*q)).collect());
            if lo[2] + p > mz1 && ok(&mg, &ms, &nr) && ok(&ng, &ns, &mr) {
                period = p;
                break;
            }
        }
    }
    (pair, period)
}

/// The least pitch along one axis (`axis` 0: x, 2: z) at which a copy's waiting riders and
/// summoners clear this station's golems and summoners both ways (out of phase), scanning up from
/// 1 block.
pub fn pitch(lay: &Layout, st: &Station, axis: usize) -> i32 {
    let ours: Vec<Vec3> = st.summoners.iter().chain(lay.riders.iter()).copied().collect();
    for d in 1..200 {
        let sh = |p: Vec3| if axis == 0 { Vec3::new(p.x + d as f64, p.y, p.z) } else { Vec3::new(p.x, p.y, p.z + d as f64) };
        let theirs: Vec<Vec3> = ours.iter().map(|p| sh(*p)).collect();
        let tg: Vec<Vec3> = st.golems.iter().map(|p| sh(*p)).collect();
        let ts: Vec<Vec3> = st.summoners.iter().map(|p| sh(*p)).collect();
        let ok = |gs: &[Vec3], ss: &[Vec3], rs: &[Vec3]| {
            rs.iter().all(|r| gs.iter().all(|g| crate::tile::sight_margin(*g, *r) >= 0.0) && ss.iter().all(|s| crate::tile::mark_margin(*s, *r) >= 0.0))
        };
        if ok(&st.golems, &st.summoners, &theirs) && ok(&tg, &ts, &ours) {
            return d;
        }
    }
    -1
}
