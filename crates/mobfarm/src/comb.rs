//! S2, the comb stand (farmopt's stand-faults lane, child cc767b78, Nov 11 2026): each pocket of
//! the pocket stand on its own branch, entered through a switch flipped by single-cart evidence
//! (`queue-loop --stand-comb N`, emulation; the switches' latches and the gates are bench-driven,
//! the junction rails real).
//!
//! The pockets, their rest cells, gates and the series departure stay today's (so rider rest
//! cells, pad spots and golems equal main's). The arrival is a tree of two stem-in switches:
//! - T, the approach's flat top (-3,6,4), the climb coming in from +z: powered (south_west) to the
//!   P3 line (west curve (-4,6,4), down x -4, curve (-4,3,0), (-3,2,0), (-2,1,0), pocket 3);
//!   unpowered (south_east) to the east line along z 4 down to S_y (2,3,4).
//! - S_y, stem west: unpowered (south_west) to the P1 line (+z curve (2,3,5), along z 5 at height
//!   3, curve (6,3,5), down x 6 into J1 (6,-1,0)); powered (north_west) to the P2 line (down x 2
//!   into J2 (2,0,0)). Variant 2: S_y passes the P1 line straight east (a wrong-side entry,
//!   pending its bridge): unpowered north_east, the P1 line along z 4 at height 2.
//! - J2 and J1, pocket 2's and pocket 1's flats, fixed curves south_east (+z into the pocket);
//!   departing carts cross them from the west (wrong-side entry).
//! - The ring (cart edges only, no timer): D_P3 (the P3 line's 2nd slope) sets T east; D_P1 (the P1
//!   line's first cell past S_y's curve) sets S_y to P2; D_P2 (the P2 line's first slope) resets
//!   both. Order: cart 0 to P3, 1 to P1, 2 to P2.
//! - Gates: G_k opens at REL and shuts on the next line-k arrival edge E_k; GF_k (a fence gate on
//!   line k above E_k) shuts on E_k (FULL_k set) and reopens on pocket k's own exit edge X_k
//!   (FULL_k reset). E_k and X_k are read as ideal detectors (a cart's box meeting the cell's
//!   search box, re-checked every 20 gt), flagged, until the gravity pockets' real detectors land.
//! - The head latch: set when every pocket's occupancy latch (E_k set, X_k reset) is clear, reset
//!   by the launch; printed against every launch (it binds only when the stand never emptied).
//!   Pockets are indexed as `LoopLayout::pockets`: 0 = pocket 1 (G0), 1 = pocket 2 (G1), 2 =
//!   pocket 3 (G2).

use redsim::block::Block;
use redsim::entity::Vec3;
use redsim::pos::BlockPos;
use redsim::world::World;
use std::collections::HashMap;

/// The comb's cells in the loop's frame (the approach-5 bridge loop: row B at x -3).
#[derive(Clone, Debug)]
pub struct CombCells {
    /// 1: stem-in S_y (the P1 line via z 5); 2: straight S_y (wrong-side, along z 4).
    pub variant: u32,
    /// The gravity pockets ((b)): gate slopes plain, the cells past G2 and G1 detector rails.
    pub gravity: bool,
    /// The branch rails off the loop's path (cell, block state), each on a glass support.
    pub rails: Vec<([i32; 3], String)>,
    pub t: [i32; 3],
    pub t_ctl: [i32; 3],
    pub sy: [i32; 3],
    pub sy_ctl: [i32; 3],
    /// S_y's shape for the P1 route (unpowered) and the P2 route (powered).
    pub sy_shapes: [&'static str; 2],
    /// The ring's three detectors: D_P3 (on the loop's path), D_P1, D_P2.
    pub d_p3: [i32; 3],
    pub d_p1: [i32; 3],
    pub d_p2: [i32; 3],
    /// Per pocket (0 = P1, 1 = P2, 2 = P3): the arrival cell E_k, the exit cell X_k, the line gate
    /// GF_k and its facing.
    pub e: [[i32; 3]; 3],
    pub x: [[i32; 3]; 3],
    pub gf: [([i32; 3], &'static str); 3],
    /// The station zombie moved off the P1 line: boat position, its glass, its leaves.
    pub zombie: ([f64; 3], [i32; 3], [i32; 3]),
    /// The approach top's post (the climb, T and the lines).
    pub top_post: ([f64; 3], [i32; 3], [i32; 3]),
    /// J2 and J1, the fixed junctions departing carts cross from the west (wrong-side entry).
    pub j: [[i32; 3]; 2],
}

impl CombCells {
    pub fn new(variant: u32, gravity: bool) -> CombCells {
        let r = |s: &str| format!("rail[shape={s},waterlogged=false]");
        let d = |s: &str| format!("detector_rail[powered=false,shape={s},waterlogged=false]");
        let mut rails: Vec<([i32; 3], String)> = vec![
            ([-2, 5, 4], r("ascending_west")),
            ([-1, 4, 4], r("ascending_west")),
            ([0, 3, 4], r("ascending_west")),
            ([1, 3, 4], r("east_west")),
            ([2, 3, 4], r("south_west")),
            // the P2 line: three slopes into J2 (D_P2, read ideally on the first: a real detector
            // there would open GF2 above the second, the line being too short for both)
            ([2, 2, 3], r("ascending_south")),
            ([2, 1, 2], r("ascending_south")),
            ([2, 0, 1], r("ascending_south")),
        ];
        let (d_p1, sy_shapes) = if variant == 2 {
            rails.extend([
                ([3, 2, 4], d("ascending_west")),
                ([4, 2, 4], r("east_west")),
                ([5, 2, 4], r("east_west")),
                ([6, 2, 4], r("north_west")),
                ([6, 1, 3], r("ascending_south")),
                ([6, 0, 2], r("ascending_south")),
                ([6, -1, 1], r("ascending_south")),
            ]);
            ([3, 2, 4], ["north_east", "north_west"])
        } else {
            rails.extend([
                ([2, 3, 5], r("north_east")),
                ([3, 3, 5], d("east_west")),
                ([4, 3, 5], r("east_west")),
                ([5, 3, 5], r("east_west")),
                ([6, 3, 5], r("north_west")),
                ([6, 2, 4], r("ascending_south")),
                ([6, 1, 3], r("ascending_south")),
                ([6, 0, 2], r("ascending_south")),
                ([6, -1, 1], r("ascending_south")),
            ]);
            ([3, 3, 5], ["south_west", "north_west"])
        };
        CombCells {
            variant,
            gravity,
            rails,
            t: [-3, 6, 4],
            t_ctl: [-3, 5, 4],
            sy: [2, 3, 4],
            sy_ctl: [2, 2, 4],
            sy_shapes,
            d_p3: [-4, 4, 2],
            d_p1,
            d_p2: [2, 2, 3],
            // (E: past each line gate; pocket 3's is its rest cell, the line being one slope
            // short; G2 shuts on its own cart's exit edge, the only cart through it)
            e: [[6, -1, 1], [2, 0, 1], [-1, 0, 0]],
            x: [[9, -3, 1], [5, -2, 0], [1, -1, 0]],
            gf: [([6, 1, 2], "north"), ([2, 2, 2], "north"), ([-2, 2, 0], "east")],
            zombie: ([4.5, -2.0, 2.5], [4, -3, 2], [4, 1, 2]),
            top_post: ([0.5, 3.0, 9.5], [0, 2, 9], [0, 6, 9]),
            j: [[2, 0, 0], [6, -1, 0]],
        }
    }

    pub fn shifted(&self, sx: i32, sz: i32) -> CombCells {
        let c = |p: [i32; 3]| [p[0] + sx, p[1], p[2] + sz];
        let f = |p: [f64; 3]| [p[0] + sx as f64, p[1], p[2] + sz as f64];
        let mut n = self.clone();
        n.rails = self.rails.iter().map(|(p, s)| (c(*p), s.clone())).collect();
        n.t = c(self.t);
        n.t_ctl = c(self.t_ctl);
        n.sy = c(self.sy);
        n.sy_ctl = c(self.sy_ctl);
        n.d_p3 = c(self.d_p3);
        n.d_p1 = c(self.d_p1);
        n.d_p2 = c(self.d_p2);
        n.e = self.e.map(c);
        n.x = self.x.map(c);
        n.gf = self.gf.map(|(p, s)| (c(p), s));
        n.zombie = (f(self.zombie.0), c(self.zombie.1), c(self.zombie.2));
        n.top_post = (f(self.top_post.0), c(self.top_post.1), c(self.top_post.2));
        n.j = self.j.map(c);
        n
    }
}

fn bp(p: [i32; 3]) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// A detector read ideally: on while a cart's box meets the cell's search box (inset 0.2, 0.8
/// high), re-checked every 20 gt from its on-edge, as the game's detector rail.
#[derive(Clone, Copy, Debug, Default)]
struct VDet {
    on: bool,
    next: i64,
}

impl VDet {
    /// Returns true on the on-edge.
    fn step(&mut self, present: bool, t: i64) -> bool {
        if !self.on {
            if present {
                self.on = true;
                self.next = t + 20;
                return true;
            }
            return false;
        }
        if t >= self.next {
            if present {
                self.next = t + 20;
            } else {
                self.on = false;
            }
        }
        false
    }
}

/// A cart's box (0.98 wide) meets the cell's search box: its centre within 0.79 of the cell's
/// centre on both horizontal axes, its feet within the cell's lower metre.
fn meets(p: Vec3, c: [i32; 3]) -> bool {
    (p.x - (c[0] as f64 + 0.5)).abs() < 0.79 && (p.z - (c[2] as f64 + 0.5)).abs() < 0.79 && p.y >= c[1] as f64 - 0.3 && p.y < c[1] as f64 + 1.0
}

/// A cart's box overlaps the cell (a junction's or a gate's occupant).
fn in_cell(p: Vec3, c: [i32; 3]) -> bool {
    (p.x - (c[0] as f64 + 0.5)).abs() < 0.99 && (p.z - (c[2] as f64 + 0.5)).abs() < 0.99 && p.y > c[1] as f64 - 1.2 && p.y < c[1] as f64 + 1.5
}

#[derive(Clone, Copy, Debug)]
enum Pend {
    TEast(bool),
    SyP2(bool),
    Gate(usize, bool),
    Gf(usize, bool),
}

/// The comb's bench state (one run).
pub struct CombRun {
    pub c: CombCells,
    t_east: bool,
    sy_p2: bool,
    pending: Vec<(i64, Pend)>,
    det_prev: [bool; 3],
    e_det: [VDet; 3],
    x_det: [VDet; 3],
    dp2_det: VDet,
    pub occ: [bool; 3],
    full: [bool; 3],
    pub head: bool,
    head_since: Option<i64>,
    prev: HashMap<u32, Vec3>,
    speeds: HashMap<u32, Vec<(f64, f64)>>,
    /// Per slot: each pocket's rest (dt, the cart, its peak stored and visible speed over the 6 gt
    /// before it stopped).
    rest: [Option<(i64, u32, f64, f64)>; 3],
    still: HashMap<u32, i64>,
    /// Per pocket: the gate's shut tick (from an arrival edge), the last tick a cart overlapped
    /// the gate's cell while it was open.
    gate_shut_at: [Option<i64>; 3],
    gate_last_busy: [Option<i64>; 3],
    gate_open: [bool; 3],
    gf_open: [bool; 3],
    rel_at: Option<i64>,
    window_count: i64,
    corner_seen: HashMap<u32, i64>,
    pub flips: Vec<String>,
    pub flips_occupied: usize,
    pub slot_lines: Vec<String>,
    pub problems: Vec<String>,
    edge_log: Vec<String>,
    gate_margin_min: Option<(i64, String)>,
    clear_margin_min: Option<(i64, String)>,
    group_max: i64,
    head_binds: Vec<String>,
    head_slack_min: Option<(i64, String)>,
    rest_last: Vec<i64>,
    impact_max: (f64, f64, String),
    /// Wrong-side crossings of J2/J1 from the west: (min entry visible speed, its record), (min
    /// exit), the count, the first records.
    ws_entry_min: Option<(f64, String)>,
    ws_exit_min: Option<(f64, String)>,
    ws_count: usize,
    ws_log: Vec<String>,
    ws_open: HashMap<(u32, usize), f64>,
}

impl CombRun {
    pub fn new(c: CombCells) -> CombRun {
        CombRun {
            c,
            t_east: false,
            sy_p2: false,
            pending: Vec::new(),
            det_prev: [false; 3],
            e_det: [VDet::default(); 3],
            x_det: [VDet::default(); 3],
            dp2_det: VDet::default(),
            occ: [true; 3],
            full: [true; 3],
            head: true,
            head_since: None,
            prev: HashMap::new(),
            speeds: HashMap::new(),
            rest: [None; 3],
            still: HashMap::new(),
            gate_shut_at: [None; 3],
            gate_last_busy: [None; 3],
            gate_open: [false; 3],
            gf_open: [false; 3],
            rel_at: None,
            window_count: 0,
            corner_seen: HashMap::new(),
            flips: Vec::new(),
            flips_occupied: 0,
            slot_lines: Vec::new(),
            problems: Vec::new(),
            edge_log: Vec::new(),
            gate_margin_min: None,
            clear_margin_min: None,
            group_max: 0,
            head_binds: Vec::new(),
            head_slack_min: None,
            rest_last: Vec::new(),
            impact_max: (0.0, 0.0, String::new()),
            ws_entry_min: None,
            ws_exit_min: None,
            ws_count: 0,
            ws_log: Vec::new(),
            ws_open: HashMap::new(),
        }
    }

    fn set(w: &mut World, p: [i32; 3], s: &str) {
        if let Ok(b) = Block::parse(&format!("minecraft:{s}")) {
            w.set_block(bp(p), b, redsim::world::flags::ALL);
        }
    }

    fn junction(&mut self, w: &mut World, carts: &[(u32, Vec3, Vec3)], name: &str, cell: [i32; 3], ctl: [i32; 3], powered: bool, shape: &str, t: i64, slot: i64) {
        Self::set(w, ctl, if powered { "redstone_block" } else { "glass" });
        Self::set(w, cell, &format!("rail[shape={shape},waterlogged=false]"));
        let occ: Vec<String> = carts.iter().filter(|(_, p, _)| in_cell(*p, cell)).map(|(id, p, v)| format!("e{id} at {:.2},{:.2},{:.2} v{:.3}", p.x, p.y, p.z, (v.x * v.x + v.z * v.z).sqrt())).collect();
        if !occ.is_empty() {
            self.flips_occupied += 1;
        }
        if self.flips.len() < 400 {
            self.flips.push(format!("slot {slot} +{}: {name} -> {shape}{}", t - slot * 100, if occ.is_empty() { String::from(" (empty)") } else { format!(" WITH {}", occ.join(", ")) }));
        }
    }

    fn gate(w: &mut World, p: [i32; 3], facing: &str, open: bool) {
        Self::set(w, p, &format!("oak_fence_gate[facing={facing},in_wall=false,open={open},powered=false]"));
    }

    /// The REL's rise: every pocket gate opens (the bench's own pocket code sets the gate blocks),
    /// each gate slope powered with its gate (unless the gravity pockets), a new departure window.
    pub fn on_rel(&mut self, w: &mut World, t: i64, pockets: &[(BlockPos, [i32; 3])]) {
        if self.rel_at.is_some() {
            self.group_max = self.group_max.max(self.window_count);
        }
        self.rel_at = Some(t);
        self.window_count = 0;
        self.gate_open = [true; 3];
        if !self.c.gravity {
            for (_, s) in pockets.iter().take(3) {
                Self::set(w, [s[0], s[1] - 1, s[2]], "redstone_block");
            }
        }
    }

    /// The register's launch: the head latch read (printed), then reset.
    pub fn on_launch(&mut self, t: i64, slot: i64) {
        if !self.head {
            if self.head_binds.len() < 20 {
                self.head_binds.push(format!("slot {slot}"));
            }
        } else if let Some(s) = self.head_since {
            let slack = t - s;
            if self.head_slack_min.as_ref().is_none_or(|(m, _)| slack < *m) {
                self.head_slack_min = Some((slack, format!("slot {slot}")));
            }
        }
        self.head = false;
        self.head_since = None;
    }

    /// Before the world's tick: the ring's detectors, the ideal arrival and exit edges, the
    /// latches, every due junction and gate change.
    pub fn pre_tick(&mut self, w: &mut World, t: i64, slot: i64, dt: i64, pockets: &[(BlockPos, [i32; 3])], stand_cells: &[[i32; 3]]) {
        let carts: Vec<(u32, Vec3, Vec3)> = w.entities.list.iter().filter(|e| matches!(e.kind, redsim::entity::EntityKind::Minecart(_))).map(|e| (e.id, e.pos, e.delta_movement)).collect();
        // (speeds: stored |v| and the distance moved last gt)
        for (id, p, v) in &carts {
            let moved = self.prev.get(id).map_or(0.0, |q| ((p.x - q.x).powi(2) + (p.z - q.z).powi(2)).sqrt());
            let h = self.speeds.entry(*id).or_default();
            h.push(((v.x * v.x + v.z * v.z).sqrt(), moved));
            if h.len() > 8 {
                h.remove(0);
            }
            // (a wrong-side crossing of J2/J1: entering from the west edge, leaving by the east)
            if let Some(q) = self.prev.get(id).copied() {
                for (ji, jc) in self.c.j.iter().enumerate() {
                    let near = (p.z - (jc[2] as f64 + 0.5)).abs() < 0.75 && (p.y - jc[1] as f64).abs() < 1.5;
                    let (x0, x1) = (jc[0] as f64, jc[0] as f64 + 1.0);
                    if near && q.x < x0 && p.x >= x0 {
                        self.ws_open.insert((*id, ji), moved);
                        let rec = format!("t {t} J{} e{id} in {moved:.3} (stored {:.3})", 2 - ji, (v.x * v.x + v.z * v.z).sqrt());
                        if self.ws_entry_min.as_ref().is_none_or(|(m, _)| moved < *m) {
                            self.ws_entry_min = Some((moved, rec.clone()));
                        }
                        if self.ws_log.len() < 40 {
                            self.ws_log.push(rec);
                        }
                    }
                    if near && q.x < x1 && p.x >= x1 && self.ws_open.remove(&(*id, ji)).is_some() {
                        self.ws_count += 1;
                        let rec = format!("t {t} J{} e{id} out {moved:.3}", 2 - ji);
                        if self.ws_exit_min.as_ref().is_none_or(|(m, _)| moved < *m) {
                            self.ws_exit_min = Some((moved, rec.clone()));
                        }
                        if self.ws_log.len() < 40 {
                            self.ws_log.push(rec);
                        }
                    }
                }
            }
            self.prev.insert(*id, *p);
        }
        // the ring's detectors (D_P3 and D_P1 real rails; D_P2 read ideally, see `new`)
        let on = |p: [i32; 3]| w.get(bp(p)).to_string().contains("powered=true");
        let dp2_present = carts.iter().any(|(_, p, _)| meets(*p, self.c.d_p2));
        let dp2_edge = self.dp2_det.step(dp2_present, t);
        let now = [on(self.c.d_p3), on(self.c.d_p1), self.dp2_det.on];
        for (k, name) in ["D_P3", "D_P1", "D_P2"].iter().enumerate() {
            let edge = if k == 2 { dp2_edge } else { now[k] && !self.det_prev[k] };
            if edge {
                if self.edge_log.len() < 200 {
                    self.edge_log.push(format!("slot {slot} +{dt}: {name}"));
                }
                match k {
                    0 => {
                        self.pending.push((t + 2, Pend::TEast(true)));
                        // (G2 still open: pocket 3 was empty at the release, no exit edge shut
                        // it; the next pocket-3 arrival's line edge does)
                        if self.gate_open[2] {
                            self.pending.push((t + 2, Pend::Gate(2, false)));
                        }
                    }
                    1 => self.pending.push((t + 4, Pend::SyP2(true))),
                    _ => {
                        self.pending.push((t + 2, Pend::TEast(false)));
                        self.pending.push((t + 4, Pend::SyP2(false)));
                    }
                }
            }
        }
        self.det_prev = now;
        // the ideal arrival and exit edges
        for k in 0..3 {
            let pe = carts.iter().any(|(_, p, _)| meets(*p, self.c.e[k]));
            if self.e_det[k].step(pe, t) {
                self.occ[k] = true;
                self.full[k] = true;
                if k < 2 && self.gate_open[k] {
                    self.pending.push((t + 2, Pend::Gate(k, false)));
                }
                self.pending.push((t + 2, Pend::Gf(k, false)));
                if let Some(lb) = self.gate_last_busy[k] {
                    let m = t - lb;
                    if self.clear_margin_min.as_ref().is_none_or(|(x, _)| m < *x) {
                        self.clear_margin_min = Some((m, format!("slot {slot} pocket {} (last departure in G at t {lb}, E edge t {t})", 3 - k)));
                    }
                }
            }
            let px = carts.iter().any(|(_, p, _)| meets(*p, self.c.x[k]));
            if self.x_det[k].step(px, t) {
                self.occ[k] = false;
                if self.full[k] {
                    self.full[k] = false;
                    self.pending.push((t + 2, Pend::Gf(k, true)));
                }
                // (G2 passes only pocket 3's cart: it shuts on that cart's exit edge, its tail
                // past the plate by then)
                if k == 2 && self.gate_open[2] {
                    self.pending.push((t + 4, Pend::Gate(2, false)));
                }
            }
        }
        if !self.head && self.occ.iter().all(|o| !o) {
            self.head = true;
            self.head_since = Some(t);
        }
        // the due changes
        let due: Vec<Pend> = self.pending.iter().filter(|(at, _)| *at == t).map(|(_, p)| *p).collect();
        self.pending.retain(|(at, _)| *at != t);
        for p in due {
            match p {
                Pend::TEast(e) => {
                    if self.t_east != e {
                        self.t_east = e;
                        let (cell, ctl) = (self.c.t, self.c.t_ctl);
                        self.junction(w, &carts, "T", cell, ctl, !e, if e { "south_east" } else { "south_west" }, t, slot);
                    }
                }
                Pend::SyP2(e) => {
                    if self.sy_p2 != e {
                        self.sy_p2 = e;
                        let (cell, ctl) = (self.c.sy, self.c.sy_ctl);
                        let shape = self.c.sy_shapes[e as usize];
                        self.junction(w, &carts, "S_y", cell, ctl, e, shape, t, slot);
                    }
                }
                Pend::Gate(k, open) => {
                    if let Some((g, s)) = pockets.get(k) {
                        let g = [g.x, g.y, g.z];
                        Self::gate(w, g, "east", open);
                        if !self.c.gravity {
                            Self::set(w, [s[0], s[1] - 1, s[2]], if open { "redstone_block" } else { "glass" });
                        }
                        if !open {
                            self.gate_open[k] = false;
                            self.gate_shut_at[k] = Some(t);
                            if carts.iter().any(|(_, p, _)| in_cell(*p, g)) {
                                self.problems.push(format!("slot {slot} +{dt}: G{} shut with a cart in its cell", 2 - k));
                            }
                        }
                    }
                }
                Pend::Gf(k, open) => {
                    let (g, f) = self.c.gf[k];
                    Self::gate(w, g, f, open);
                    self.gf_open[k] = open;
                    if !open && carts.iter().any(|(_, p, _)| in_cell(*p, g)) {
                        self.problems.push(format!("slot {slot} +{dt}: GF{} shut with a cart in its cell", 3 - k));
                    }
                }
            }
        }
        // the gates' busy record (a cart in an open gate's cell: a departure)
        for (k, (g, _)) in pockets.iter().enumerate().take(3) {
            let g = [g.x, g.y, g.z];
            if self.gate_open[k] && carts.iter().any(|(_, p, _)| in_cell(*p, g)) {
                self.gate_last_busy[k] = Some(t);
            }
        }
        // departures per window: a cart's first entry into the corner cell east of G0
        if let Some((g0, _)) = pockets.first() {
            let corner = [g0.x + 1, g0.y - 1, g0.z];
            for (id, p, _) in &carts {
                if in_cell(*p, corner) && (p.x - (corner[0] as f64 + 0.5)).abs() < 0.5 {
                    let last = self.corner_seen.get(id).copied();
                    if last.is_none_or(|l| t - l > 60) {
                        self.window_count += 1;
                    }
                    self.corner_seen.insert(*id, t);
                }
            }
        }
        // the rests: a cart still (moved < 0.002 for 2 gt) on a pocket's rest cell, the first per
        // slot from +30, with its peak speeds over the gt before
        for (k, rc) in stand_cells.iter().enumerate().take(3) {
            if self.rest[k].is_some() || dt < 30 {
                continue;
            }
            for (id, p, _) in &carts {
                if p.x.floor() as i32 != rc[0] || p.z.floor() as i32 != rc[2] || (p.y - rc[1] as f64).abs() > 1.2 {
                    continue;
                }
                let h = self.speeds.get(id).cloned().unwrap_or_default();
                let moved = h.last().map_or(1.0, |x| x.1);
                if moved < 0.002 {
                    let n = self.still.entry(*id).or_insert(0);
                    *n += 1;
                    if *n >= 2 {
                        // (stopped during the gt before the first still read: dt - 3)
                        let dt_rest = dt - 3;
                        let peak_s = h.iter().map(|x| x.0).fold(0.0, f64::max);
                        let peak_v = h.iter().map(|x| x.1).fold(0.0, f64::max);
                        self.rest[k] = Some((dt_rest, *id, peak_s, peak_v));
                        if peak_s > self.impact_max.0 {
                            self.impact_max = (peak_s, peak_v, format!("slot {slot} pocket {} +{dt_rest}", 3 - k));
                        }
                        // (E's edge to G shut, before the arriving cart reached G: the shut tick
                        // against the rest's first still tick)
                        if let Some(s) = self.gate_shut_at[k] {
                            let m = (t - 3) - s;
                            if self.gate_margin_min.as_ref().is_none_or(|(x, _)| m < *x) {
                                self.gate_margin_min = Some((m, format!("slot {slot} pocket {}", 3 - k)));
                            }
                        }
                    }
                } else {
                    self.still.remove(id);
                }
            }
        }
        if dt == 99 {
            let f = |k: usize| self.rest[k].map_or(String::from("-"), |(d, id, s, v)| format!("+{d} e{id} (stored {s:.2}, visible {v:.2})"));
            self.slot_lines.push(format!("comb slot {slot}: rests P3 {} | P1 {} | P2 {} | head {} | window {}", f(2), f(0), f(1), if self.head { "set" } else { "-" }, self.window_count));
            if let Some(last) = self.rest.iter().filter_map(|r| r.map(|x| x.0)).max() {
                if self.rest.iter().all(|r| r.is_some()) {
                    self.rest_last.push(last);
                }
            }
            self.rest = [None; 3];
            self.still.clear();
        }
    }

    pub fn report(&mut self) -> Vec<String> {
        self.group_max = self.group_max.max(self.window_count);
        let mut out = Vec::new();
        out.push(format!("comb (S2, variant {}{}): ideal E/X edges (flagged), bench-driven latches and gates, real junctions and ring detectors", self.c.variant, if self.c.gravity { ", gravity pockets" } else { "" }));
        let mut rl = self.rest_last.clone();
        rl.sort();
        out.push(format!("comb: batch at rest (last of 3) over {} slots: min +{} median +{} max +{}; margin to +98 (the summon tick - 2): {}", rl.len(), rl.first().copied().unwrap_or(-1), rl.get(rl.len() / 2).copied().unwrap_or(-1), rl.last().copied().unwrap_or(-1), rl.last().map_or(String::from("-"), |m| format!("{}", 98 - m))));
        out.push(format!("comb: peak impact into a rest {:.2} stored / {:.2} visible ({})", self.impact_max.0, self.impact_max.1, self.impact_max.2));
        out.push(format!("comb: switch flips {} ({} with a cart in the switch's cell)", self.flips.len(), self.flips_occupied));
        out.push(format!("comb: E edge to G shut, before the arrival reached G: min {} gt ({})", self.gate_margin_min.as_ref().map_or(String::from("-"), |x| x.0.to_string()), self.gate_margin_min.as_ref().map_or("", |x| x.1.as_str())));
        out.push(format!("comb: last departure clear of G to the next E edge: min {} gt ({})", self.clear_margin_min.as_ref().map_or(String::from("-"), |x| x.0.to_string()), self.clear_margin_min.as_ref().map_or("", |x| x.1.as_str())));
        out.push(format!("comb: largest group per departure window: {} (bar 3)", self.group_max));
        out.push(format!("comb: wrong-side crossings of J2/J1 (departures from the west): {}; min entry {} ; min exit {} (bar: entry >= 0.1 b/t)", self.ws_count, self.ws_entry_min.as_ref().map_or(String::from("-"), |x| format!("{:.3} ({})", x.0, x.1)), self.ws_exit_min.as_ref().map_or(String::from("-"), |x| format!("{:.3} ({})", x.0, x.1))));
        if let Some((m, r)) = &self.ws_entry_min {
            if *m < 0.1 {
                self.problems.push(format!("a wrong-side entry at {m:.3} b/t, under 0.1 ({r})"));
            }
        }
        if !self.ws_open.is_empty() {
            self.problems.push(format!("{} wrong-side crossing(s) entered and never left by the east (a cart stranded in J2/J1?)", self.ws_open.len()));
        }
        out.push(format!("comb: head latch binds at {} launches{}; min slack (stand empty to launch) {}", self.head_binds.len(), if self.head_binds.is_empty() { String::new() } else { format!(" ({})", self.head_binds.join(", ")) }, self.head_slack_min.as_ref().map_or(String::from("-"), |x| format!("{} gt ({})", x.0, x.1))));
        if self.group_max > 3 {
            self.problems.push(format!("a departure window released {} carts (bar 3)", self.group_max));
        }
        out
    }

    pub fn detail(&self) -> Vec<String> {
        let mut out = self.slot_lines.clone();
        out.extend(self.ws_log.iter().cloned());
        out.extend(self.edge_log.iter().take(60).cloned());
        out.extend(self.flips.iter().take(60).cloned());
        out
    }
}
