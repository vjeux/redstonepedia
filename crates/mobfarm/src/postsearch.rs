//! The zombie-post search for the 6-8 rule (ironspec Nov 11 10:18: the flee runs only with a
//! hostile within 6, PANIC needs a seen one within 8; so no zombie within 6 of any rider position,
//! and where panic is needed, a seen one within 8). Given a design, the posts to re-lay (removed
//! first), the rider places that need panic (a box of rail cells) and a search box, it picks posts
//! (a zombie in a boat on a glass block, leaves 3 above, as the generator builds them) greedily:
//! each new post the candidate that brings the most panic points within 8 (seen, the eyes' line
//! clear) while keeping every rider point of the design (every rail cell, three points along its
//! axis, the rider seated as `mobfarm flee-check` seats it) at 6 or more from every post, both with
//! a margin for the zombie's place in its boat. It stops when every panic point is covered or no
//! candidate adds one.

use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::{EntityKind, Vec3};
use redsim::pos::BlockPos;

/// One search's result.
pub struct Found {
    /// The posts picked: the boat's cell (the glass under it, the leaves 3 above).
    pub posts: Vec<[i32; 3]>,
    /// Per panic cell: the nearest picked post's distance (least over its points) and whether a
    /// post within 8 is seen from every point.
    pub cover: Vec<([i32; 3], f64, bool)>,
    /// The least distance from any rider point of the design to any picked post.
    pub least: f64,
}

/// The rider points of every rail cell: (cell, the seated rider's position and eye at three
/// points along the rail).
fn rider_points(spec: &FarmSpec) -> Result<(Run, Vec<([i32; 3], Vec<(Vec3, Vec3)>)>), String> {
    let mut s = spec.clone();
    s.entities.clear();
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&s, &o)?;
    while run.settle_tick() {}
    let rails: Vec<(BlockPos, String)> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, b)| (*p, b.to_string())).collect();
    let mut out = Vec::new();
    let w = &mut run.world;
    for (c, state) in &rails {
        let along = |t: f64| crate::fleecheck::along_rail(*c, state, t);
        let p0 = along(0.5);
        let cart = redsim::entity::spawn(w, "minecraft:minecart", [p0.x, p0.y, p0.z], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?;
        let Some(ci) = redsim::entity::index_of(w, cart) else { continue };
        let Some(&vid) = w.entities.list[ci].passengers.first() else { continue };
        let Some(vi) = redsim::entity::index_of(w, vid) else { continue };
        let mut pts = Vec::new();
        for t in [0.1, 0.5, 0.9] {
            w.entities.list[ci].pos = along(t);
            redsim::entity::position_rider(w, ci, vi);
            let me = w.entities.list[vi].pos;
            pts.push((me, Vec3::new(me.x, me.y + 1.62, me.z)));
        }
        let _ = redsim::entity::kill(w, vid);
        let _ = redsim::entity::kill(w, cart);
        w.entities.list.retain(|e| !e.removed);
        out.push(([c.x, c.y, c.z], pts));
    }
    Ok((run, out))
}

/// The search: `spec` with the posts to re-lay already removed; `panic` the rail cells that need a
/// seen zombie within 8; candidates every cell of `area` (x0, y0, z0, x1, y1, z1) whose glass, boat
/// and zombie cells and the leaves 3 above are air in the design; `margin` taken off both bounds.
pub fn search(spec: &FarmSpec, panic: &[[i32; 3]], area: [i32; 6], margin: f64, max_posts: usize) -> Result<Found, String> {
    search_in(spec, panic, area, margin, max_posts, false)
}

/// The design's box: the least and greatest cell of its blocks.
pub fn design_box(spec: &FarmSpec) -> ([i32; 3], [i32; 3]) {
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for (p, _) in &spec.blocks {
        for (k, v) in [p.x, p.y, p.z].into_iter().enumerate() {
            lo[k] = lo[k].min(v);
            hi[k] = hi[k].max(v);
        }
    }
    (lo, hi)
}

/// [`search`]; `in_box`: a post's glass, boat, zombie and leaves cells all inside the design's
/// box (ironspec Nov 11 11:13: a post must not grow the box).
pub fn search_in(spec: &FarmSpec, panic: &[[i32; 3]], area: [i32; 6], margin: f64, max_posts: usize, in_box: bool) -> Result<Found, String> {
    let (blo, bhi) = design_box(spec);
    let area = if in_box { [area[0].max(blo[0]), area[1].max(blo[1] + 1), area[2].max(blo[2]), area[3].min(bhi[0]), area[4].min(bhi[1] - 3), area[5].min(bhi[2])] } else { area };
    let (mut run, riders) = rider_points(spec)?;
    // (the zombie's seat in its boat: the boat's passenger attachment, then the zombie's
    // ridingOffset (`rider_seat`), as flee-check seats the design's own posts)
    let seat = {
        let w = &mut run.world;
        let b = redsim::entity::spawn(w, "minecraft:oak_boat", [0.5, 300.0, 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"))?;
        let bi = redsim::entity::index_of(w, b).ok_or("the seat probe's boat")?;
        let zid = *w.entities.list[bi].passengers.first().ok_or("the seat probe's zombie")?;
        let zi = redsim::entity::index_of(w, zid).ok_or("the seat probe's zombie")?;
        let off = match &w.entities.list[zi].kind {
            EntityKind::Mob(m) => redsim::entity::mob::rider_seat(&w.entities.list[bi].kind, m).map_or(0.0, |(s, o)| s + o),
            _ => 0.0,
        };
        let _ = redsim::entity::kill(w, zid);
        let _ = redsim::entity::kill(w, b);
        w.entities.list.retain(|e| !e.removed);
        Vec3::new(0.0, off, 0.0)
    };
    let w = &run.world;
    let solid: std::collections::HashSet<[i32; 3]> = spec.blocks.iter().map(|(p, _)| [p.x, p.y, p.z]).collect();
    let mut cands: Vec<[i32; 3]> = Vec::new();
    for x in area[0]..=area[3] {
        for y in area[1]..=area[4] {
            for z in area[2]..=area[5] {
                let free = |dy: i32| !solid.contains(&[x, y + dy, z]);
                if free(-1) && free(0) && free(1) && free(2) && free(3) {
                    cands.push([x, y, z]);
                }
            }
        }
    }
    // (a post's zombie where it sits: the boat at the cell's centre, the seat's offset added)
    let boat = |c: [i32; 3]| Vec3::new(c[0] as f64 + 0.5 + seat.x, c[1] as f64 + seat.y, c[2] as f64 + 0.5 + seat.z);
    let dist = |a: Vec3, b: Vec3| ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt();
    // (a candidate is allowed when every rider point is 6 + margin or more from it)
    let allowed: Vec<[i32; 3]> = cands.into_iter().filter(|c| riders.iter().all(|(_, pts)| pts.iter().all(|(p, _)| dist(*p, boat(*c)) >= 6.0 + margin))).collect();
    let panic_pts: Vec<([i32; 3], usize)> = riders.iter().filter(|(c, _)| panic.contains(c)).flat_map(|(c, pts)| (0..pts.len()).map(move |k| (*c, k))).collect();
    let pt = |c: [i32; 3], k: usize| riders.iter().find(|(x, _)| *x == c).map(|(_, p)| p[k]).expect("a panic point");
    // (a post covers a panic point when within 8 - margin and its zombie's eye sees the rider's)
    let covers = |post: [i32; 3], c: [i32; 3], k: usize| {
        let (me, eye) = pt(c, k);
        let b = boat(post);
        dist(me, b) <= 8.0 - margin && redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(b.x, b.y + 1.74, b.z))
    };
    let mut picked: Vec<[i32; 3]> = Vec::new();
    let mut covered = vec![false; panic_pts.len()];
    while picked.len() < max_posts && covered.iter().any(|c| !c) {
        let best = allowed
            .iter()
            .filter(|c| !picked.contains(c))
            .map(|c| (*c, panic_pts.iter().enumerate().filter(|(i, (pc, k))| !covered[*i] && covers(*c, *pc, *k)).count()))
            .max_by_key(|(_, n)| *n);
        match best {
            Some((c, n)) if n > 0 => {
                for (i, (pc, k)) in panic_pts.iter().enumerate() {
                    if covers(c, *pc, *k) {
                        covered[i] = true;
                    }
                }
                picked.push(c);
            }
            _ => break,
        }
    }
    let cover: Vec<([i32; 3], f64, bool)> = panic
        .iter()
        .filter_map(|c| riders.iter().find(|(x, _)| x == c))
        .map(|(c, pts)| {
            let d = picked.iter().flat_map(|p| pts.iter().map(move |(me, _)| dist(*me, boat(*p)))).fold(f64::INFINITY, f64::min);
            let seen = (0..pts.len()).all(|k| picked.iter().any(|p| covers(*p, *c, k)));
            (*c, d, seen)
        })
        .collect();
    let least = riders.iter().flat_map(|(_, pts)| pts.iter()).flat_map(|(me, _)| picked.iter().map(move |p| dist(*me, boat(*p)))).fold(f64::INFINITY, f64::min);
    let _ = EntityKind::Minecart;
    Ok(Found { posts: picked, cover, least })
}
