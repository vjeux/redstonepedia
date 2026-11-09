//! The regular day-1 bed-plate cage, searched by its sight lines (ironspec iteration 18; vjeux
//! 18:26: "please optimize the villager room, the current layout is super weird"). D4's room has
//! its plate villager (U1) beside the zombie's window, a level up: 3 irregularities. The regular
//! room puts U1 in the watchers' row at their level, walled (blind to the zombie), its plate in
//! front of its bed; the plate powers its 6 neighbours and, strongly, the block under it, whose 6
//! neighbours are powered too. A hide (a fence gate: 1.5 tall shut, nothing open; a trapdoor's
//! halves) must sit in those cells, cut every watcher's standing ray to the zombie at night (U1
//! asleep, the plate off: they fall asleep), leave every standing ray clear by day (U1 on its
//! plate: they panic and summon), and leave the sleeping rays clear at night (they panic in bed:
//! D1U's night golems), while no hide opens a walkable way out of a pod (a hide beside U1's
//! standing cells at its level is a door when open: only the under-block's neighbours under a
//! wall qualify), U1 never sees the zombie, and every watcher stands within the zombie's 8 blocks.
//! This enumerates the layouts and checks each ray exactly (segment against the collision boxes
//! of the cells it crosses) before any judge.

/// A box in a cell's own coordinates (0..1 on each axis; a fence gate reaches 1.5 up).
type Aabb = ([f64; 3], [f64; 3]);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hide {
    /// An oak fence gate facing north/south (its panel along x) or east/west (along z).
    Gate { along_x: bool },
    /// A trapdoor: its half when shut (top or bottom), the face its panel stands on when open.
    Trap { top: bool, open_face: char },
}

impl Hide {
    /// Its collision box shut (night) or open (day), in its cell's coordinates.
    fn shape(self, open: bool) -> Option<Aabb> {
        match (self, open) {
            (Hide::Gate { .. }, true) => None,
            (Hide::Gate { along_x: true }, false) => Some(([0.0, 0.0, 0.375], [1.0, 1.5, 0.625])),
            (Hide::Gate { along_x: false }, false) => Some(([0.375, 0.0, 0.0], [0.625, 1.5, 1.0])),
            (Hide::Trap { top: true, .. }, false) => Some(([0.0, 0.8125, 0.0], [1.0, 1.0, 1.0])),
            (Hide::Trap { top: false, .. }, false) => Some(([0.0, 0.0, 0.0], [1.0, 0.1875, 1.0])),
            (Hide::Trap { open_face, .. }, true) => Some(match open_face {
                'n' => ([0.0, 0.0, 0.0], [1.0, 1.0, 0.1875]),
                's' => ([0.0, 0.0, 0.8125], [1.0, 1.0, 1.0]),
                'w' => ([0.0, 0.0, 0.0], [0.1875, 1.0, 1.0]),
                _ => ([0.8125, 0.0, 0.0], [1.0, 1.0, 1.0]),
            }),
        }
    }

    pub fn name(self) -> String {
        match self {
            Hide::Gate { along_x } => format!("fence gate facing {}", if along_x { "north" } else { "east" }),
            Hide::Trap { top, open_face } => format!("{} trapdoor opening onto its {open_face} face", if top { "top" } else { "bottom" }),
        }
    }
}

/// Does the segment a..b cross the box?
fn crosses(a: [f64; 3], b: [f64; 3], lo: [f64; 3], hi: [f64; 3]) -> bool {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for i in 0..3 {
        let d = b[i] - a[i];
        if d.abs() < 1e-12 {
            if a[i] <= lo[i] || a[i] >= hi[i] {
                return false;
            }
            continue;
        }
        let (mut u, mut v) = ((lo[i] - a[i]) / d, (hi[i] - a[i]) / d);
        if u > v {
            std::mem::swap(&mut u, &mut v);
        }
        t0 = t0.max(u);
        t1 = t1.min(v);
        if t0 >= t1 {
            return false;
        }
    }
    true
}

/// Every cell the segment a..b passes through (a fine walk: 1/64 block steps).
fn cells_on(a: [f64; 3], b: [f64; 3]) -> Vec<[i32; 3]> {
    let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
    let n = (len * 64.0).ceil().max(1.0) as usize;
    let mut out: Vec<[i32; 3]> = Vec::new();
    for i in 0..=n {
        let t = i as f64 / n as f64;
        let q = [(a[0] + (b[0] - a[0]) * t).floor() as i32, (a[1] + (b[1] - a[1]) * t).floor() as i32, (a[2] + (b[2] - a[2]) * t).floor() as i32];
        if out.last() != Some(&q) && !out.contains(&q) {
            out.push(q);
        }
    }
    out
}

/// One layout of the regular room.
#[derive(Clone, Debug)]
pub struct Layout {
    pub watchers: Vec<i32>,
    pub u1: i32,
    /// U1's plate, the cell it stands on awake: in front of its bed's head (u1, 0, -1), or beside
    /// the head in the row (u1 +/- 1, 0, 0).
    pub plate: [i32; 3],
    /// A second plate in U1's room (the parent: U1 may step onto either; a hide powered by either
    /// counts, an over-approximation: if even that finds nothing, nothing exists)
    pub plate2: Option<[i32; 3]>,
    /// The zombie's slab cell (its boat on it; the eye at slab y + 1.73, z + 0.2).
    pub zombie: [i32; 3],
    pub hides: Vec<([i32; 3], Hide)>,
}

/// The eyes: a watcher standing on its bed (feet 0.5625 + 1.62), asleep (bed + 0.7625); U1 on its
/// plate (feet 0.0625 + 1.62) or asleep.
const STAND: f64 = 0.5625 + 1.62;
const SLEEP: f64 = 0.7625;

fn zombie_eye(z: [i32; 3]) -> [f64; 3] {
    [z[0] as f64 + 0.5, z[1] as f64 + 1.73, z[2] as f64 + 0.2]
}

/// U1's room: its bed's two cells and its plates'.
fn u1_room(l: &Layout) -> Vec<[i32; 3]> {
    let mut v = vec![[l.u1, 0, 0], [l.u1, 0, 1], l.plate];
    v.extend(l.plate2);
    v
}

/// The cells that must stay solid: around U1's room at its level and the one above (its walls,
/// beside a watcher's pod too: the pods are 1 wide), its floor and the block under its plate; the
/// watchers' side walls.
fn solid_cells(l: &Layout) -> std::collections::HashSet<[i32; 3]> {
    let mut s = std::collections::HashSet::new();
    let room = u1_room(l);
    for c in &room {
        for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            for y in 0..=1 {
                let q = [c[0] + d[0], y, c[2] + d[2]];
                if !room.contains(&[q[0], 0, q[2]]) && !(l.watchers.contains(&q[0]) && (q[2] == 0 || q[2] == 1)) {
                    s.insert(q);
                }
            }
        }
        s.insert([c[0], -1, c[2]]);
    }
    for &w in &l.watchers {
        for x in [w - 1, w + 1] {
            if !room.iter().any(|c| c[0] == x) && !l.watchers.contains(&x) {
                for y in 1..=2 {
                    for z in 0..=1 {
                        s.insert([x, y, z]);
                    }
                }
            }
        }
    }
    s
}

/// Is the ray a..b blocked (by a solid cell, a pod's open front trapdoor or a bed, or a hide's box in
/// its state)? The zombie's own cell never blocks.
fn blocked(l: &Layout, solid: &std::collections::HashSet<[i32; 3]>, a: [f64; 3], b: [f64; 3], open: bool) -> bool {
    let end = [b[0].floor() as i32, b[1].floor() as i32, b[2].floor() as i32];
    let boxed = |c: [i32; 3], lo: [f64; 3], hi: [f64; 3]| {
        crosses(a, b, [c[0] as f64 + lo[0], c[1] as f64 + lo[1], c[2] as f64 + lo[2]], [c[0] as f64 + hi[0], c[1] as f64 + hi[1], c[2] as f64 + hi[2]])
    };
    for c in cells_on(a, b) {
        if c == end {
            continue;
        }
        if solid.contains(&c) {
            return true;
        }
        // the watchers' pods (D4's): the open trapdoor over the bed's head (its panel on the
        // cell's north face), the beds (9/16 high) of every pod
        if c[1] == 1 && c[2] == 0 && l.watchers.contains(&c[0]) && boxed(c, [0.0, 0.0, 0.0], [1.0, 1.0, 0.1875]) {
            return true;
        }
        if c[1] == 0 && (c[2] == 0 || c[2] == 1) && (l.watchers.contains(&c[0]) || c[0] == l.u1) && boxed(c, [0.0, 0.0, 0.0], [1.0, 0.5625, 1.0]) {
            return true;
        }
        if let Some((_, h)) = l.hides.iter().find(|(p, _)| *p == c)
            && let Some((lo, hi)) = h.shape(open)
            && boxed(c, lo, hi)
        {
            return true;
        }
    }
    false
}

/// The checks; Ok(()) when the layout's room works by its sight lines.
pub fn check(l: &Layout) -> Result<(), String> {
    let solid = solid_cells(l);
    let ze = zombie_eye(l.zombie);
    for &w in &l.watchers {
        let feet = [w as f64 + 0.5, 0.5625, 0.5];
        let zf = [l.zombie[0] as f64 + 0.5, l.zombie[1] as f64 + 0.5, l.zombie[2] as f64 + 0.2];
        let d = ((feet[0] - zf[0]).powi(2) + (feet[1] - zf[1]).powi(2) + (feet[2] - zf[2]).powi(2)).sqrt();
        if d > 8.0 {
            return Err(format!("the watcher at x {w} stands {d:.1} from the zombie (more than 8)"));
        }
        let stand = [w as f64 + 0.5, STAND, 0.5];
        let sleep = [w as f64 + 0.5, SLEEP, 0.5];
        if blocked(l, &solid, stand, ze, true) {
            return Err(format!("the watcher at x {w} does not see the zombie by day"));
        }
        if !blocked(l, &solid, stand, ze, false) {
            return Err(format!("the watcher at x {w} still sees the zombie standing at night"));
        }
        if blocked(l, &solid, sleep, ze, false) {
            return Err(format!("the watcher at x {w} does not see the zombie from its bed at night"));
        }
    }
    let pe = [l.plate[0] as f64 + 0.5, 0.0625 + 1.62, l.plate[2] as f64 + 0.5];
    let u = l.u1 as f64 + 0.5;
    for (eye, open) in [(pe, true), ([u, SLEEP, 0.5], false)] {
        if !blocked(l, &solid, eye, ze, open) {
            return Err("U1 sees the zombie".into());
        }
    }
    // no hide in a cell the rooms need solid, nor beside U1's room at its level or the one above
    // (an open hide there is a door out)
    let room = u1_room(l);
    for (p, _) in &l.hides {
        if solid.contains(p) {
            return Err(format!("a hide at {p:?} where U1's room needs a wall"));
        }
        let beside_u1 = (0..=1).contains(&p[1]) && room.iter().any(|c| (p[0] - c[0]).abs() + (p[2] - c[2]).abs() <= 1);
        if beside_u1 {
            return Err(format!("a hide at {p:?} opens a way out of U1's room"));
        }
    }
    Ok(())
}

/// The cells U1's plate powers (its 6 neighbours) and the block under it powers (its 6).
pub fn powered(plate: [i32; 3]) -> Vec<[i32; 3]> {
    let p = plate;
    let under = [p[0], p[1] - 1, p[2]];
    let mut v = Vec::new();
    for c in [p, under] {
        for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
            let q = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
            if q != p && q != under && !v.contains(&q) {
                v.push(q);
            }
        }
    }
    v
}

/// Every layout of the space: 2 watchers, U1 in the row (between them or at an end), the zombie's
/// slab anywhere ahead and below (x -2..6, z -2..-9, y 0..-9), one or two hides among the powered
/// cells that a standing ray crosses, each a fence gate or a trapdoor; the ones whose sight lines
/// work. Pruned in order: the zombie's distance, the day rays and the sleeping rays clear of the
/// rooms, U1 blind by its walls; then only the hide cells a standing ray crosses.
pub fn search() -> (usize, usize, Vec<Layout>) {
    let rows: Vec<(Vec<i32>, i32)> = vec![(vec![0, 4], 2), (vec![0, 2], 4), (vec![0, 2], -2), (vec![0, 4], 6), (vec![0, 4], -2), (vec![0, 6], 2), (vec![0, 6], 4), (vec![0, 6], 3), (vec![0, 5], 2), (vec![0, 5], 3)];
    let kinds = [Hide::Gate { along_x: true }, Hide::Gate { along_x: false }, Hide::Trap { top: true, open_face: 'e' }, Hide::Trap { top: true, open_face: 'w' }, Hide::Trap { top: false, open_face: 'e' }, Hide::Trap { top: false, open_face: 'w' }, Hide::Trap { top: true, open_face: 'n' }, Hide::Trap { top: false, open_face: 's' }];
    let (mut zombies, mut tried) = (0, 0);
    let mut found = Vec::new();
    for (watchers, u1) in &rows {
        // U1's plate: in front of its bed's head, or beside the head in the row; or two of them
        let singles: Vec<[i32; 3]> = [[*u1, 0, -1], [u1 + 1, 0, 0], [u1 - 1, 0, 0], [u1 + 1, 0, -1], [u1 - 1, 0, -1]].into_iter().filter(|p| !watchers.iter().any(|w| (w - p[0]).abs() < 1)).collect();
        let mut plates: Vec<([i32; 3], Option<[i32; 3]>)> = singles.iter().map(|p| (*p, None)).collect();
        for (i, a) in singles.iter().enumerate() {
            for b in singles.iter().skip(i + 1) {
                plates.push((*a, Some(*b)));
            }
        }
        for (plate, plate2) in plates {
        let mut cells = powered(plate);
        if let Some(p2) = plate2 {
            for c in powered(p2) {
                if !cells.contains(&c) && c != plate {
                    cells.push(c);
                }
            }
            cells.retain(|c| *c != p2);
        }
        for zx in -2..=8 {
            for zz in -9..=-2 {
                for zy in -9..=0 {
                    let mut l = Layout { watchers: watchers.clone(), u1: *u1, plate, plate2, zombie: [zx, zy, zz], hides: Vec::new() };
                    let solid = solid_cells(&l);
                    let ze = zombie_eye(l.zombie);
                    let far = watchers.iter().any(|&w| {
                        let d2 = (w as f64 + 0.5 - (zx as f64 + 0.5)).powi(2) + (0.5625 - (zy as f64 + 0.5)).powi(2) + (0.5 - (zz as f64 + 0.2)).powi(2);
                        d2 > 64.0
                    });
                    if far {
                        continue;
                    }
                    if watchers.iter().any(|&w| blocked(&l, &solid, [w as f64 + 0.5, STAND, 0.5], ze, true) || blocked(&l, &solid, [w as f64 + 0.5, SLEEP, 0.5], ze, false)) {
                        continue;
                    }
                    let u = *u1 as f64 + 0.5;
                    if !blocked(&l, &solid, [plate[0] as f64 + 0.5, 0.0625 + 1.62, plate[2] as f64 + 0.5], ze, true) || !blocked(&l, &solid, [u, SLEEP, 0.5], ze, false) {
                        continue;
                    }
                    zombies += 1;
                    // the hide cells: powered, crossed by some standing ray
                    let on: Vec<[i32; 3]> = cells.iter().copied().filter(|c| watchers.iter().any(|&w| cells_on([w as f64 + 0.5, STAND, 0.5], ze).contains(c))).collect();
                    let mut sets: Vec<Vec<([i32; 3], Hide)>> = Vec::new();
                    for (i, a) in on.iter().enumerate() {
                        for ka in kinds {
                            sets.push(vec![(*a, ka)]);
                            for b in on.iter().skip(i + 1) {
                                for kb in kinds {
                                    sets.push(vec![(*a, ka), (*b, kb)]);
                                }
                            }
                        }
                    }
                    for hides in sets {
                        tried += 1;
                        l.hides = hides;
                        if check(&l).is_ok() {
                            found.push(l.clone());
                        }
                    }
                }
            }
        }
        }
    }
    (zombies, tried, found)
}
