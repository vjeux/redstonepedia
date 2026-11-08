//! The mob-farm refusals (ironfarm; brief `designs/briefs/farm-iron.md`, "Refused"): what the
//! mob layer cannot judge in a build, found before any run. An empty list: judgeable.
//!
//! * Lava that can flow, water touching lava, job sites, bunk beds.
//! * The zombie: it must ride a vehicle; its eye cell must not see the sky or be in water; its
//!   attack box (`Mob.getAttackBoundingBox`: its box and its vehicle's, inflated 0.828) must not
//!   meet a villager's cell.
//! * The villagers: each confined to ONE cell (the flood of its 0.6 x 1.95 box: steps, jumps,
//!   drops; water passable; a wooden door next to it refused, villagers open them), on its bed's
//!   reach (`SleepInBed`'s 2.0 from every corner of the cell), its standing cell within
//!   Manhattan 1 of HOME (`SetWalkTargetFromBlockMemory(HOME, .., 1, ..)`, else it loses HOME),
//!   its wake-up spot (both bed sides) inside the cell, and under a roof the sky does not see
//!   through (lightning turns a villager that sees the sky into a witch).
//! * Gossip (`TradeWithVillager`, the IDLE/MEET packages): refused when a villager can see
//!   another within 8 (an eye-to-eye COLLIDER ray, standing or sleeping, from the corners of its
//!   cell) while 5 or more villagers share its 10-block box.

use super::farm::{FarmSpec, RunOpts, WalkMode, attack_box, build};
use super::types::MobType;
use super::{mob, villager};
use crate::block::Block;
use crate::entity::{Aabb, EntityKind, Vec3};
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// Every refusal of a build (empty: the mob layer can judge it).
pub fn static_checks(spec: &FarmSpec) -> Vec<String> {
    let o = RunOpts { seed: 1, ticks: 0, day0: 6000, walk: WalkMode::Random, slept: true };
    let w = match build(spec, &o) {
        Ok(w) => w,
        Err(e) => return vec![e],
    };
    let mut out = block_checks(&w, spec);
    out.extend(zombie_checks(&w));
    out.extend(villager_checks(&w));
    out.extend(gossip_check(&w));
    out
}

fn block_checks(w: &World, spec: &FarmSpec) -> Vec<String> {
    let mut out = Vec::new();
    for (p, s) in &spec.blocks {
        match s {
            Block::Lava { level } => {
                if *level != 0 {
                    out.push(format!("flowing lava at {p}: only lava sources that cannot flow are modeled"));
                } else if let Some(q) = super::blocks::lava_can_flow(w, *p) {
                    out.push(format!("the lava at {p} can flow into {q} (only lava that cannot flow is modeled)"));
                }
                for d in Dir::ALL {
                    if !crate::fluid::fluid_state(w, p.rel(d)).is_empty() {
                        out.push(format!("water touches the lava at {p}: it would turn to obsidian or cobblestone (not modeled)"));
                    }
                }
            }
            // a job site: `AcquirePoi` claims one (JOB_SITE, POTENTIAL_JOB_SITE) only along a path
            // that ends within 1 of it, and every villager is confined to one cell
            // (`villager_checks` refuses the rest), so only a job site touching a villager's
            // cell could give it a profession (its working is not modeled)
            Block::Composter { .. } | Block::BrewingStand { .. } if villagers(w).iter().any(|&i| {
                let c = w.entities.list[i].block_position();
                (p.x - c.x).abs() <= 2 && (p.y - c.y).abs() <= 2 && (p.z - c.z).abs() <= 2
            }) => out.push(format!("a job site block at {p} within 2 of a villager: it could take a profession (not modeled)")),
            Block::Scaffolding { .. } => out.push(format!("scaffolding at {p}: a mob in a climbable climbs and is not pushable (not modeled)")),
            Block::Bed { head: true, .. } if matches!(w.get(p.below()), Block::Bed { .. }) => out.push(format!("a bunk bed at {p}: its stand-up search is not modeled")),
            _ => {}
        }
    }
    out
}

fn villagers(w: &World) -> Vec<usize> {
    (0..w.entities.list.len()).filter(|&i| matches!(&w.entities.list[i].kind, EntityKind::Mob(m) if m.ty == MobType::Villager)).collect()
}

/// The box a villager can take in its cell `c` (floor height `h`).
fn region(c: BlockPos, h: f64) -> Aabb {
    Aabb::new(c.x as f64, h, c.z as f64, c.x as f64 + 1.0, h + 1.95, c.z as f64 + 1.0)
}

/// The feet positions at the corners of the region a villager's 0.6 box can take in cell `c`,
/// and its centre.
fn corners(c: BlockPos, h: f64) -> Vec<Vec3> {
    let mut v = vec![Vec3::new(c.x as f64 + 0.5, h, c.z as f64 + 0.5)];
    for (dx, dz) in [(0.3, 0.3), (0.3, 0.7), (0.7, 0.3), (0.7, 0.7)] {
        v.push(Vec3::new(c.x as f64 + dx, h, c.z as f64 + dz));
    }
    v
}

fn zombie_checks(w: &World) -> Vec<String> {
    let mut out = Vec::new();
    let zombies: Vec<usize> = (0..w.entities.list.len()).filter(|&i| matches!(&w.entities.list[i].kind, EntityKind::Mob(m) if m.ty == MobType::Zombie)).collect();
    if zombies.is_empty() {
        out.push("no zombie: nothing scares the villagers".into());
    }
    for &i in &zombies {
        let e = &w.entities.list[i];
        let Some(vi) = e.vehicle.and_then(|v| crate::entity::index_of(w, v)) else {
            out.push("the zombie must ride a boat or minecart: it would despawn (Mob.checkDespawn) and walk (not modeled)".into());
            continue;
        };
        let vehicle = &w.entities.list[vi];
        let Some(off) = super::rider_offset(&vehicle.kind, mob(w, i)) else {
            out.push("the zombie rides a vehicle whose seat is not modeled".into());
            continue;
        };
        let seat = vehicle.pos.add(0.0, off, 0.0);
        let eye = BlockPos::new(seat.x.floor() as i32, (seat.y + mob(w, i).eye() as f64).floor() as i32, seat.z.floor() as i32);
        if !super::blocks::sky_hidden(w, eye) {
            out.push(format!("the zombie's eye cell {eye} may see the sky (no leaves or opaque block above it): it burns in daylight"));
        }
        if !crate::fluid::fluid_state(w, eye).is_empty() {
            out.push(format!("the zombie's eye is in water at {eye}: it turns into a drowned"));
        }
        let reach = attack_box(crate::entity::make_bounding_box(seat, 0.6, 1.95), Some(vehicle.bb));
        for &v in &villagers(w) {
            let p = w.entities.list[v].pos;
            let c = BlockPos::new(p.x.floor() as i32, (p.y + 1.0e-3).floor() as i32, p.z.floor() as i32);
            if reach.intersects(region(c, floor_height(w, c))) {
                out.push(format!("the zombie's attack box (with its vehicle) meets the cell of the villager at {:.1} {:.1} {:.1}: it could kill or convert it", p.x, p.y, p.z));
            }
        }
    }
    out
}

fn villager_checks(w: &World) -> Vec<String> {
    let mut out = Vec::new();
    for i in villagers(w) {
        let e = &w.entities.list[i];
        let at = format!("the villager at {:.1} {:.1} {:.1}", e.pos.x, e.pos.y, e.pos.z);
        let Some(home) = mob(w, i).brain.as_deref().and_then(|b| b.home) else { continue };
        let cells = match pod(w, e.pos) {
            Err(why) => {
                out.push(format!("{at}: {why}"));
                continue;
            }
            Ok(cells) => cells,
        };
        if cells.len() != 1 {
            out.push(format!("{at} can stand in {} cells: pods must be one cell (its spawn centre and sensing boxes move with it; walking is not modeled)", cells.len()));
            continue;
        }
        let c = cells[0];
        let h = floor_height(w, c);
        let feet_cell = BlockPos::new(c.x, (h + 1.0e-3).floor() as i32, c.z);
        let manhattan = (feet_cell.x - home.x).abs() + (feet_cell.y - home.y).abs() + (feet_cell.z - home.z).abs();
        if manhattan > 1 {
            out.push(format!("{at} stands {manhattan} (Manhattan) from its bed {home}: SetWalkTargetFromBlockMemory walks it home and it loses HOME after 1200 gt of failed paths"));
        }
        for p in corners(c, h) {
            if !villager::closer_to_center(home, p, 2.0) {
                out.push(format!("{at} can stand at {:.1} {:.1} {:.1}, 2 or more from its bed {home}: SleepInBed would not start", p.x, p.y, p.z));
            }
        }
        if let Block::Bed { facing, .. } = w.get(home) {
            for side in [facing.clockwise(), facing.counter_clockwise()] {
                let stand = villager::stand_up_with_side(w, MobType::Villager, home, facing, side).unwrap_or(Vec3::new(home.x as f64 + 0.5, home.y as f64 + 1.1, home.z as f64 + 0.5));
                let (sx, sz) = (stand.x.floor() as i32, stand.z.floor() as i32);
                if (sx, sz) != (c.x, c.z) {
                    out.push(format!("{at} woken from {home} stands up at {:.1} {:.1} {:.1}, outside its cell", stand.x, stand.y, stand.z));
                }
            }
        }
        for d in Dir::HORIZONTAL {
            for dy in 0..3 {
                if let Block::Door { iron: false, .. } = w.get(feet_cell.rel(d).offset(0, dy, 0)) {
                    out.push(format!("{at}: a wooden door at {} (villagers open wooden doors: InteractWithDoor)", feet_cell.rel(d).offset(0, dy, 0)));
                }
            }
        }
        for dy in 0..3 {
            let q = feet_cell.offset(0, dy, 0);
            if !super::blocks::sky_hidden(w, q) {
                out.push(format!("{at} may see the sky at {q} (no leaves or opaque block above): lightning would turn it into a witch (a roof of leaves, or one at y+6, or capped)"));
                break;
            }
        }
    }
    out
}

/// The code-true gossip rule: a villager that can see another within 8 while 5 or more share
/// its 10-block box.
fn gossip_check(w: &World) -> Vec<String> {
    let vs = villagers(w);
    let mut out = Vec::new();
    for &a in &vs {
        let box10 = w.entities.list[a].bb.inflate(10.0, 10.0, 10.0);
        let sharing = vs.iter().filter(|&&b| w.entities.list[b].bb.intersects(box10)).count();
        if sharing < 5 {
            continue;
        }
        for &b in &vs {
            if b == a || w.entities.list[a].pos.distance_to_sqr(w.entities.list[b].pos) > 64.0 {
                continue;
            }
            let (ea, eb) = (eyes(w, a), eyes(w, b));
            if ea.iter().any(|p| eb.iter().any(|q| super::los::has_line_of_sight(w, *p, *q))) {
                let (pa, pb) = (w.entities.list[a].pos, w.entities.list[b].pos);
                out.push(format!("{sharing} villagers share a 10-block box and the villager at {:.1} {:.1} {:.1} can see the one at {:.1} {:.1} {:.1}: gossip could spawn golems (not modeled)", pa.x, pa.y, pa.z, pb.x, pb.y, pb.z));
                return out;
            }
        }
    }
    out
}

/// A villager's eye positions over its cell: standing at the corners, and sleeping on its bed.
fn eyes(w: &World, i: usize) -> Vec<Vec3> {
    let p = w.entities.list[i].pos;
    let c = BlockPos::new(p.x.floor() as i32, (p.y + 1.0e-3).floor() as i32, p.z.floor() as i32);
    let h = floor_height(w, c);
    let mut v: Vec<Vec3> = corners(c, h).into_iter().map(|q| q.add(0.0, 1.62, 0.0)).collect();
    if let Some(home) = mob(w, i).brain.as_deref().and_then(|b| b.home) {
        v.push(Vec3::new(home.x as f64 + 0.5, home.y as f64 + 0.6875 + 0.2, home.z as f64 + 0.5));
    }
    v
}

/// The floor height in a cell (`BlockGetter.getBlockFloorHeight`): its collision top, else 0 when
/// the block below is full height.
pub fn floor_height(w: &World, c: BlockPos) -> f64 {
    let top = |p: BlockPos| crate::entity::collide::boxes_at(w, &w.get(p), p).iter().map(|b| b.max_y - p.y as f64).fold(f64::NEG_INFINITY, f64::max);
    let here = top(c);
    if here.is_finite() && here > 0.0 {
        c.y as f64 + here
    } else {
        let below = top(c.below());
        if below >= 1.0 { c.y as f64 } else { c.y as f64 - 1.0 }
    }
}

/// The cells a villager standing at `feet` can walk to (the pod): a flood over horizontally
/// adjacent cells whose floor is within a jump (1.25) above or any drop below, where its 0.6 x
/// 1.95 box fits (water and every block without a collision box pass). More than 9 cells, or a
/// cell 3 away: not confined.
pub fn pod(w: &World, feet: Vec3) -> Result<Vec<BlockPos>, String> {
    let start = BlockPos::new(feet.x.floor() as i32, (feet.y + 1.0e-3).floor() as i32, feet.z.floor() as i32);
    let fits = |c: BlockPos, h: f64| {
        let bb = Aabb::new(c.x as f64 + 0.2, h + 1.0e-3, c.z as f64 + 0.2, c.x as f64 + 0.8, h + 1.95, c.z as f64 + 0.8);
        crate::entity::collide::block_collisions(w, bb).iter().all(|(_, bs)| bs.iter().all(|b| !b.intersects(bb)))
    };
    // the box swept from one cell's centre to the next's (a straight step across their face)
    let passes = |c: BlockPos, m: BlockPos, h: f64| {
        let (x0, z0) = (c.x.min(m.x) as f64 + 0.2, c.z.min(m.z) as f64 + 0.2);
        let (x1, z1) = (c.x.max(m.x) as f64 + 0.8, c.z.max(m.z) as f64 + 0.8);
        let bb = Aabb::new(x0, h + 1.0e-3, z0, x1, h + 1.95, z1);
        crate::entity::collide::block_collisions(w, bb).iter().all(|(_, bs)| bs.iter().all(|b| !b.intersects(bb)))
    };
    let mut cells = vec![start];
    let mut k = 0;
    while k < cells.len() {
        let c = cells[k];
        let h = floor_height(w, c);
        k += 1;
        for d in Dir::HORIZONTAL {
            let n = c.rel(d);
            // the neighbor at the same level, one up (a jump) or one down
            for dy in [0, 1, -1] {
                let m = n.offset(0, dy, 0);
                let hm = floor_height(w, m);
                if hm - h > 1.25 || cells.iter().any(|q| q.x == m.x && q.z == m.z) {
                    continue;
                }
                // the villager moves across at the higher of the two floors (then drops, or
                // after a jump), so it must fit there over both cells
                let top = h.max(hm);
                if fits(m, top) && fits(c, top) && passes(c, m, top) {
                    cells.push(m);
                    break;
                }
            }
        }
        if cells.len() > 9 || cells.iter().any(|q| (q.x - start.x).abs() > 2 || (q.z - start.z).abs() > 2) {
            return Err("not confined: it can walk out of its pod (walking is not modeled)".into());
        }
    }
    Ok(cells)
}
