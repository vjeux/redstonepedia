//! (ironspec 03:31..04:36, the scarer line's deciding run) The golem flow on the F1 + Q2 station's
//! pad, in redsim with the mob AI on: a summon at every summon tick over a game day with the scarer
//! posts laid (each a boat in a glass box: floor below, roof 2 over the seat, two-high walls on
//! three sides, open on one), and every golem followed from its spawn to its portal crossing: the
//! ticks it lives in the overworld (the longest printed; the thin margin: every spawn cell meets a
//! portal row by 0.2, so a golem crosses at the start of its second tick unless something frees it
//! in its first), its targets (a post, a piglin, anything), its least overlap with a portal row and
//! its largest move; the cadence (a golem every cycle); the next batch never golem-detected by a
//! lingering golem. Push sources: a twin golem beside each one (`twin`), pad-born zombified piglins
//! standing in the portal cells (`piglins`). Restarts in the 20 restart rows at a phase after every summon
//! (cc767b78's `liveness::restart`). The positive control: one portal row blocked (`block_row`),
//! whose golems must be caught parked and the next summon lost. A pillager post seats redsim's
//! pillager (9fc3155f's model: box, seat and eye; a villager, or a golem off a portal, it sees
//! within 8 is a loud model error, reported here as "needs the bridge"; a golem overlapping a portal
//! block with cooldown 0 is exempt); its crossbow AI is not modeled: the shots are tick-exact's
//! official bridge (0 in 136 golems).
//!
//! The station's frame is the loop frame (`wip/station/f1q2-station.txt`): the pad's portal rows at
//! z -5 and -2 (x -2..7, y -8..-6), the spawn cells on the floor rows z -6, -4, -3, -1 (x -3..8,
//! golem feet -6), the pockets' summoners at (-1/3/7, 0, 0).

use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::mob::MobEvent;
use redsim::entity::mob::types::MobType;
use redsim::entity::{EntityKind, Vec3};

/// The pad's portal rows (loop z) and their x span (portal blocks x -2..7).
pub const PORTAL_ROWS: [i32; 2] = [-5, -2];
pub const PORTAL_X: (i32, i32) = (-2, 7);
/// The spawn cells' floor rows (loop z) and x span.
pub const FLOOR_ROWS: [i32; 4] = [-6, -4, -3, -1];
pub const FLOOR_X: (i32, i32) = (-3, 8);

/// A scarer post: a boat in a glass box. `kind` 'z' (a zombie seated in the boat) or 'p' (a
/// pillager seated: redsim's seated pillager, its crossbow AI not modeled); `open` the box's open
/// side ('e', 'w', 'n', 's'); `roof` its roof
/// block (default the bench's: glass; ironspec 05:39: every zombie post's roof glowstone).
#[derive(Clone, Debug)]
pub struct Post {
    pub kind: char,
    pub cell: [i32; 3],
    pub open: char,
    pub roof: Option<String>,
}

/// `KIND:x,y,z:OPEN[:ROOF]`, e.g. `z:-5,-4,-2:e:glowstone`.
pub fn parse_post(s: &str) -> Result<Post, String> {
    let parts: Vec<&str> = s.splitn(4, ':').collect();
    if parts.len() < 3 {
        return Err(format!("--post {s}: KIND:x,y,z:OPEN[:ROOF]"));
    }
    let kind = parts[0].chars().next().filter(|c| *c == 'z' || *c == 'p').ok_or(format!("--post {s}: KIND z (zombie) or p (pillager)"))?;
    let v: Vec<i32> = parts[1].split(',').map(|x| x.trim().parse::<i32>().map_err(|e| format!("--post {s}: {e}"))).collect::<Result<_, _>>()?;
    if v.len() != 3 {
        return Err(format!("--post {s}: x,y,z"));
    }
    let open = parts[2].chars().next().filter(|c| "ewns-".contains(*c)).ok_or(format!("--post {s}: OPEN e, w, n, s, or - (no box: a part file lays it)"))?;
    let roof = parts.get(3).map(|r| if r.contains(':') { r.to_string() } else { format!("minecraft:{r}") });
    Ok(Post { kind, cell: [v[0], v[1], v[2]], open, roof })
}

/// The bench's options.
#[derive(Clone, Debug)]
pub struct FlowOpts {
    pub cycles: i64,
    pub seed: i64,
    pub posts: Vec<Post>,
    /// The positive control: this portal row (loop z) blocked (its portal blocks air).
    pub block_row: Option<i32>,
    /// Zombified piglins standing in the pad's portal cells (feet on the frame's bottom obsidian,
    /// a portal cooldown that the portal keeps restarting): push sources.
    pub piglins: usize,
    /// A second golem spawned with each one, at the next spawn cell (the push case).
    pub twin: bool,
    /// A restart at this many ticks after every summon tick, in this order.
    pub restart: Option<(i64, crate::liveness::Restart)>,
    /// The restart's control: the same ticks with no reload.
    pub restart_noop: bool,
    /// One reload only, at this game time in this order.
    pub restart_once: Option<(i64, crate::liveness::Restart)>,
    /// (G1 on 44d4bbfae) The placement count's positive control: the world built this many cells
    /// short of the design's top.
    pub box_short: i32,
    /// Extra blocks set in the design, loop frame (the runtime count's control: a water source at
    /// the top of an empty column, flowing out of the box's bottom).
    pub blocks: Vec<([i32; 3], String)>,
    /// No forced summon: the villagers' own panic summons (or not).
    pub natural: bool,
    /// The forced summon's callers: the first N summoners in lane order (x -1, 3, 7).
    pub callers: usize,
    /// The posts' roof block (glass: the zombie burns by day under it).
    pub roof: String,
    pub trace: bool,
}

/// The station's design text with the posts, the piglins and the blocked row laid.
pub fn design_text(station: &str, o: &FlowOpts) -> Result<String, String> {
    let mut d = mctas::design::Design::parse(station).map_err(|e| e.to_string())?;
    for (c, st) in &o.blocks {
        d.blocks.insert(*c, if st.contains(':') { st.clone() } else { format!("minecraft:{st}") });
    }
    for p in &o.posts {
        let [x, y, z] = p.cell;
        let sides = [('e', [x + 1, z]), ('w', [x - 1, z]), ('n', [x, z - 1]), ('s', [x, z + 1])];
        // (OPEN '-': no box here, a part file lays it)
        if p.open != '-' {
            d.blocks.insert([x, y - 1, z], "minecraft:glass".into());
            d.blocks.insert([x, y + 2, z], p.roof.clone().unwrap_or_else(|| o.roof.clone()));
            for (side, [sx, sz]) in sides {
                if side == p.open {
                    continue;
                }
                for dy in 0..2 {
                    d.blocks.insert([sx, y + dy, sz], "minecraft:glass".into());
                }
            }
        }
        let nbt = if p.kind == 'z' {
            Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".to_string())
        } else {
            // (9fc3155f's seated pillager: box, seat and eye; its AI not modeled, a villager or a
            // golem off the portal it sees within 8 a loud model error)
            Some("{Passengers:[{id:\"minecraft:pillager\",PersistenceRequired:1b}]}".to_string())
        };
        d.entities.push(mctas::design::EntitySpec { kind: "minecraft:oak_boat".into(), pos: [x as f64 + 0.5, y as f64, z as f64 + 0.5], nbt });
    }
    if let Some(row) = o.block_row {
        for x in PORTAL_X.0..=PORTAL_X.1 {
            for y in -8..=-6 {
                d.blocks.remove(&[x, y, row]);
            }
        }
    }
    // the piglins: one per portal cell column, the rows alternating, then a second round
    let mut cells: Vec<[f64; 3]> = Vec::new();
    for round in 0..4 {
        for x in PORTAL_X.0..=PORTAL_X.1 {
            for row in PORTAL_ROWS {
                cells.push([x as f64 + 0.5 + 0.2 * (round % 2) as f64 - 0.1, -8.0, row as f64 + 0.5]);
            }
        }
    }
    for c in cells.iter().take(o.piglins) {
        d.entities.push(mctas::design::EntitySpec { kind: "minecraft:zombified_piglin".into(), pos: *c, nbt: Some("{PortalCooldown:300,PersistenceRequired:1b}".into()) });
    }
    Ok(d.to_text())
}

/// The depth a golem's box (feet `p`, 1.4 wide) meets the nearest portal row (0 when none).
fn portal_overlap(p: Vec3, blocked: Option<i32>) -> f64 {
    let (x0, x1, z0, z1) = (p.x - 0.7, p.x + 0.7, p.z - 0.7, p.z + 0.7);
    if p.y < -9.0 || p.y > -5.0 || x1 <= PORTAL_X.0 as f64 || x0 >= (PORTAL_X.1 + 1) as f64 {
        return 0.0;
    }
    PORTAL_ROWS.iter().filter(|r| Some(**r) != blocked).map(|r| (z1.min(*r as f64 + 1.0) - z0.max(*r as f64)).max(0.0)).fold(0.0, f64::max)
}

/// One golem followed.
#[derive(Clone, Debug)]
struct Golem {
    id: u32,
    born: i64,
    /// The first tick it ran (its entity tick count past 0 after that tick).
    first: Option<i64>,
    at: Vec3,
    twin: bool,
    crossed: Option<i64>,
    died: Option<i64>,
    targets: Vec<(i64, String)>,
    least_overlap: f64,
    most_move: f64,
    fastest: f64,
}

impl Golem {
    fn new(id: u32, born: i64, at: Vec3, twin: bool, blocked: Option<i32>) -> Golem {
        Golem { id, born, first: None, at, twin, crossed: None, died: None, targets: Vec::new(), least_overlap: portal_overlap(at, blocked), most_move: 0.0, fastest: 0.0 }
    }

    /// The tick of its own life in which it crossed (its first tick 1; the thin margin's bar 2), or
    /// `None` (it never crossed).
    fn crossed_in(&self) -> Option<i64> {
        self.crossed.map(|c| c - self.first.unwrap_or(self.born) + 1)
    }
}

/// The bench: lines to print, and whether every bar held.
pub fn bench(spec_in: &FarmSpec, o: &FlowOpts) -> Result<(Vec<String>, bool), String> {
    let mut out = Vec::new();
    let ticks = (o.cycles + 2) * 100 + 200;
    let ro = RunOpts { ai: true, ..RunOpts::steady(o.seed, ticks as u32, WalkMode::Random) };
    let spec_run = crate::drop::spec_for_run(spec_in, o.box_short);
    let spec = &spec_run;
    let mut run = Run::start(spec, &ro)?;
    // (G1 on 44d4bbfae: the placement's drops structurally, redsim's records of them beside)
    let place_drop = crate::drop::placement_drops(spec_in, &run.world);
    let oob_at_start = crate::drop::oob_records(&run.world);
    run.arrive_homeless()?;
    let w0 = &run.world;
    let mob_of = |e: &redsim::entity::Entity| if let EntityKind::Mob(m) = &e.kind { Some(m.ty) } else { None };
    let mut pocket: Vec<u32> = Vec::new();
    let mut holds: Vec<u32> = Vec::new();
    let mut posts: Vec<u32> = Vec::new();
    let mut piglins: Vec<u32> = Vec::new();
    let mut others: Vec<u32> = Vec::new();
    for e in w0.entities.list.iter().filter(|e| !e.removed) {
        match mob_of(e) {
            // (the pockets' summoners at y 0..2, the holds' riders at y 16..18; any other villager
            // (a queue or lane rider, a napper) is neither)
            Some(MobType::Villager) if (0.0..2.0).contains(&e.pos.y) => pocket.push(e.id),
            Some(MobType::Villager) if (15.0..19.0).contains(&e.pos.y) => holds.push(e.id),
            Some(MobType::Villager) => others.push(e.id),
            Some(MobType::Zombie) => posts.push(e.id),
            Some(MobType::Pillager) => posts.push(e.id),
            Some(t) if t.def().game_id == "minecraft:zombified_piglin" => piglins.push(e.id),
            _ => {}
        }
    }
    pocket.sort_by(|a, b| {
        let pa = w0.entities.list.iter().find(|e| e.id == *a).map_or(0.0, |e| e.pos.x);
        let pb = w0.entities.list.iter().find(|e| e.id == *b).map_or(0.0, |e| e.pos.x);
        pa.total_cmp(&pb)
    });
    // (ironspec's half-cell ruling: each post's mob and boat where they start)
    let starts: Vec<String> = w0
        .entities
        .list
        .iter()
        .filter(|e| !e.removed && (posts.contains(&e.id) || matches!(&e.kind, EntityKind::Boat(_))))
        .map(|e| format!("{} #{} at {:.4},{:.4},{:.4}", if posts.contains(&e.id) { "the post's mob" } else { "a post's boat" }, e.id, e.pos.x, e.pos.y, e.pos.z))
        .collect();
    if pocket.len() != 3 {
        return Err(format!("golem flow: {} summoners in the pockets (3 wanted)", pocket.len()));
    }
    out.push(format!(
        "golem flow: {} summons from game time {} (day time {}), seed {}; summoners {:?} (lanes x -1/3/7), the holds' riders {}, other villagers {} (queue or lane riders: never forced to summon), posts: {} mob(s) seated{}, {} zombified piglin(s) in the portal cells{}{}{}",
        o.cycles,
        run.world.time,
        run.world.day_time().rem_euclid(24000),
        o.seed,
        pocket,
        holds.len(),
        others.iter().map(|id| w0.entities.list.iter().find(|e| e.id == *id).map_or(format!("#{id}"), |e| format!("#{id} at {:.2},{:.2},{:.2}", e.pos.x, e.pos.y, e.pos.z))).collect::<Vec<_>>().join(", "),
        posts.len(),
        if o.posts.iter().any(|p| p.kind == 'p') { ", the pillager seated in its boat (9fc3155f's model: box, seat and eye; its AI not modeled)" } else { "" },
        piglins.len(),
        if o.twin { ", a twin golem with each" } else { "" },
        o.block_row.map_or(String::new(), |r| format!(", THE PORTAL ROW AT z {r} BLOCKED (the positive control)")),
        o.restart.map_or(String::new(), |(p, h)| format!(", a restart at T+{p} in the {} order after every summon{}", h.label(), if o.restart_noop { " (THE CONTROL: no reload)" } else { "" })) + &o.restart_once.map_or(String::new(), |(at, h)| format!(", one reload only at game time {at} in the {} order", h.label()))
    ));
    out.push(format!("posts as they start: {}", if starts.is_empty() { "none".to_string() } else { starts.join("; ") }));
    out.push(format!(
        "the summon at every summon tick: {}; the posts' roofs {}",
        if o.natural { "the villagers' own panic only".to_string() } else { format!("forced from the first {} summoner(s) in lane order (as their panic triggers call it), and the villagers' own panic", o.callers) },
        o.posts.iter().map(|p| format!("{}:{}", p.kind, p.roof.clone().unwrap_or_else(|| o.roof.clone()))).collect::<Vec<_>>().join(", ")
    ));
    let first = (run.world.time / 100 + 1) * 100 + 100;
    let mut golems: Vec<Golem> = Vec::new();
    let mut crossed_seen = run.world.crossings.len();
    let mut sightings: Vec<(i64, u32, bool, bool)> = Vec::new();
    let mut priced_hits: Vec<String> = Vec::new();
    let mut summons: Vec<(i64, Option<u32>)> = Vec::new();
    let mut other_tries: Vec<String> = Vec::new();
    let mut lost: Vec<String> = Vec::new();
    let mut lingering_at_summon = 0usize;
    let mut piglin_moves = 0usize;
    let mut post_deaths: Vec<(i64, u32)> = Vec::new();
    let mut dawns: Vec<(i64, usize)> = Vec::new();
    // (the model's loud errors: a seated pillager seeing a villager, or a golem off the portal,
    // within 8 (9fc3155f's rule): each "needs the bridge", with its tick and positions)
    let mut errs_seen = run.world.errors.len();
    let mut model_errors: Vec<String> = Vec::new();
    let mut golem_hurt: Vec<String> = Vec::new();
    let mut golem_hits: Vec<String> = Vec::new();
    let mut diag: std::collections::BTreeMap<i64, String> = std::collections::BTreeMap::new();
    let mut post_targets: Vec<String> = Vec::new();
    let mut last_restart: Option<i64> = None;
    let end = first + o.cycles * 100;
    let follow_all = |w: &redsim::world::World, v: &mut Vec<u32>| {
        for id in v.iter_mut() {
            *id = crate::liveness::follow(w, *id);
        }
    };
    while run.world.time < end + 100 {
        let t = run.world.time;
        // (ironspec 05:39: the posts' zombies alive at each dawn, day time 0)
        if run.world.day_time().rem_euclid(24000) == 0 {
            let alive = posts.iter().filter(|id| redsim::entity::index_of(&run.world, **id).is_some_and(|i| matches!(&run.world.entities.list[i].kind, EntityKind::Mob(m) if !m.dead && m.health > 0.0))).count();
            dawns.push((t, alive));
        }
        // the next batch, fresh: in the station the summoners at T are new riders (landed from
        // T - 49 at s 12 REL T+4), so their golem memory is cleared unless a golem was sensed
        // after that landing (a lingering golem); the bench's summoners stand all run long, and a
        // sighting before it is the departing batch's (a reload's stale list); slept
        if t % 100 == 99 && t + 1 >= first && t + 1 < end {
            let since = t + 1 - 49;
            let lingering = sightings.iter().any(|(st, _, _, _)| *st >= since);
            for v in &pocket {
                if let Some(i) = redsim::entity::index_of(&run.world, *v) {
                    if let Some(b) = redsim::entity::mob::mob_mut(&mut run.world, i).brain.as_deref_mut() {
                        if !lingering {
                            b.golem_detected = None;
                        }
                        b.last_slept = Some(t - 1000);
                    }
                }
            }
            let alive = golems.iter().filter(|g| g.crossed.is_none() && g.died.is_none()).count();
            if alive > 0 {
                lingering_at_summon += 1;
            }
        }
        // the summon (as the panicking summoners' VillagerPanicTrigger calls it at gameTime % 100
        // == 0, lane by lane: the game's robust callers, lanes x -1 and x 3 with the lead posts;
        // forced, the scarers' coverage is the panic measure's)
        if t % 100 == 0 && t >= first && t < end && !o.natural {
            for v in pocket.iter().take(o.callers) {
                if let Some(i) = redsim::entity::index_of(&run.world, *v) {
                    redsim::entity::mob::spawn::spawn_golem_if_needed(&mut run.world, i, 3);
                }
            }
            // (why a summon found no willing caller: each summoner's golem memory and last sleep,
            // kept for the cycles that end with no golem at all)
            if o.trace {
                let why: Vec<String> = pocket
                    .iter()
                    .map(|v| {
                        redsim::entity::index_of(&run.world, *v).map_or(format!("#{v} gone"), |i| match &run.world.entities.list[i].kind {
                            EntityKind::Mob(m) => m.brain.as_deref().map_or(format!("#{v} no brain"), |b| format!("#{v} golem_detected {:?} last_slept {:?}", b.golem_detected, b.last_slept.map(|s| t - s))),
                            _ => format!("#{v} not a mob"),
                        })
                    })
                    .collect();
                diag.insert(t, why.join(", "));
            }
        }
        // the restart after each summon (`restart_noop`: the same tick with no reload, the control);
        // or one reload only (`restart_once`)
        let periodic = o.restart.is_some_and(|(phase, _)| t >= first && (t - phase).rem_euclid(100) == 0 && t - phase >= first && t - phase < end);
        let once_now = o.restart_once.is_some_and(|(at, _)| at == t);
        if let Some(how) = if periodic { o.restart.map(|r| r.1) } else if once_now { o.restart_once.map(|r| r.1) } else { None } {
            {
                last_restart = Some(t);
                if !o.restart_noop {
                    crate::liveness::restart(&mut run.world, how)?;
                }
                follow_all(&run.world, &mut pocket);
                follow_all(&run.world, &mut holds);
                follow_all(&run.world, &mut posts);
                follow_all(&run.world, &mut piglins);
                for g in golems.iter_mut().filter(|g| g.crossed.is_none() && g.died.is_none()) {
                    g.id = crate::liveness::follow(&run.world, g.id);
                }
                crossed_seen = run.world.crossings.len();
            }
        }
        // the events since the last tick (the forced summon's too), then the tick's
        for pass in 0..2 {
            let tk = run.world.time;
            if pass == 1 {
                run.tick();
                while errs_seen < run.world.errors.len() {
                    model_errors.push(run.world.errors[errs_seen].clone());
                    errs_seen += 1;
                }
                // a golem that has run a tick: its first tick is the one just run (the world's
                // time moves on at a tick's start: the tick just run is game time tk + 1)
                for g in golems.iter_mut().filter(|g| g.first.is_none() && g.crossed.is_none() && g.died.is_none()) {
                    if let Some(i) = redsim::entity::index_of(&run.world, g.id) {
                        if run.world.entities.list[i].tick_count > 0 {
                            g.first = Some(tk + 1);
                        }
                    }
                }
            }
            let tnow = run.world.time;
            let events = std::mem::take(&mut run.world.mob.events);
            for ev in events {
                match ev {
                    MobEvent::GolemTry { t: et, villager, golem, .. } if !pocket.contains(&villager) => {
                        // (another villager's attempt: a lane or queue rider, a napper)
                        other_tries.push(format!("#{villager} at game time {et}{}", if golem.is_some() { " (a golem)" } else { " (no spot)" }));
                    }
                    MobEvent::GolemTry { t: et, villager, golem, at, .. } if pocket.contains(&villager) => {
                        if !summons.iter().any(|(st, g)| *st == et && g.is_some()) {
                            summons.retain(|(st, _)| *st != et);
                            summons.push((et, golem));
                        }
                        if let Some(g) = golem {
                            let p = redsim::entity::index_of(&run.world, g).map(|i| run.world.entities.list[i].pos).unwrap_or(Vec3::new(0.0, 0.0, 0.0));
                            golems.push(Golem::new(g, et, p, false, o.block_row));
                            if o.twin {
                                // the next spawn cell: the adjacent floor row toward this one's
                                // portal side when there is one (rows -4 and -3), else x + 1
                                let (cx, cz) = (p.x.floor() as i32, p.z.floor() as i32);
                                let (tx, tz) = match cz {
                                    -4 => (cx, -3),
                                    -3 => (cx, -4),
                                    _ => (if cx < FLOOR_X.1 { cx + 1 } else { cx - 1 }, cz),
                                };
                                let _ = at;
                                if let Ok(tid) = redsim::entity::spawn(&mut run.world, "minecraft:iron_golem", [tx as f64 + 0.5, -6.0, tz as f64 + 0.5], None) {
                                    golems.push(Golem::new(tid, et, Vec3::new(tx as f64 + 0.5, -6.0, tz as f64 + 0.5), true, o.block_row));
                                }
                            }
                        }
                    }
                    // (a summoner's sighting, as the summoner was when it saw: the ids change at a
                    // reload; priced (ironspec 07:42) when a reload came between the summon and it)
                    MobEvent::GolemSeen { t: et, villager } if pocket.contains(&villager) => {
                        let priced = last_restart.is_some_and(|r| r >= et - et.rem_euclid(100) && r <= et);
                        // (G1: a golem sensor reads the stored living list, which keeps a crossed
                        // golem up to 20 gt: a sighting with no golem left in the overworld is that
                        // stale list's, with or without a reload)
                        let stale = !golems.iter().any(|g| g.crossed.is_none() && g.died.is_none());
                        sightings.push((et, villager, priced, stale));
                    }
                    MobEvent::Hurt { id, src, amount, .. } => {
                        // (ironspec 06:14: the bar is the outcome: no golem takes or deals damage,
                        // nothing shoots it; ironspec 07:42: a reload inside a new golem's first two
                        // ticks can let the zombie scarer hit it once, a priced restart cost)
                        if let Some(g) = golems.iter().find(|g| g.id == id && g.crossed.is_none() && g.died.is_none()) {
                            let s = format!("golem #{id} (spawned at {:.1},{:.1},{:.1}) hurt {amount:.1} ({src:?}) at game time {tnow}", g.at.x, g.at.y, g.at.z);
                            if matches!(src, redsim::entity::mob::Damage::MobAttack) && last_restart.is_some_and(|r| r >= g.born && r <= tnow) {
                                priced_hits.push(s);
                            } else {
                                golem_hurt.push(s);
                            }
                        } else if matches!(src, redsim::entity::mob::Damage::MobAttack) && golems.iter().any(|g| g.crossed.is_none() && g.died.is_none()) {
                            golem_hits.push(format!("#{id} hit by a mob {amount:.1} at game time {tnow} with a golem in the overworld"));
                        }
                    }
                    MobEvent::Death { t: et, id, .. } => {
                        if let Some(g) = golems.iter_mut().find(|g| g.id == id && g.died.is_none() && g.crossed.is_none()) {
                            g.died = Some(et);
                        }
                        if posts.contains(&id) {
                            post_deaths.push((et, id));
                        }
                    }
                    _ => {}
                }
            }
            if let Some(n) = run.world.nether.as_deref_mut() {
                n.mob.events.clear();
            }
            while crossed_seen < run.world.crossings.len() {
                let (ct, from, _) = run.world.crossings[crossed_seen];
                crossed_seen += 1;
                if let Some(g) = golems.iter_mut().find(|g| g.id == from && g.crossed.is_none()) {
                    g.crossed = Some(ct);
                    // (it crossed before running a tick: its first tick is the crossing's)
                    g.first.get_or_insert(ct);
                }
                if piglins.contains(&from) {
                    piglin_moves += 1;
                }
            }
            // each live golem: its target, its overlap with a portal row, its move
            let w = &run.world;
            // (the posts' own targets while a golem is in the overworld: a post's mob taking a
            // golem, the reload's effect on a zombie's melee)
            for p in &posts {
                let Some(i) = redsim::entity::index_of(w, *p) else { continue };
                let EntityKind::Mob(m) = &w.entities.list[i].kind else { continue };
                if let Some(tid) = m.ai.as_deref().and_then(|a| a.target) {
                    if let Some(g) = golems.iter().find(|g| g.id == tid && g.crossed.is_none() && g.died.is_none()) {
                        let s = format!("post #{p} targets golem #{tid} at game time {tnow} (the golem's tick {}){}", tnow - g.born, if last_restart.is_some_and(|r| tnow - r <= 3) { ", just after a reload" } else { "" });
                        if post_targets.last() != Some(&s) {
                            post_targets.push(s);
                        }
                    }
                }
            }
            for g in golems.iter_mut().filter(|g| g.crossed.is_none() && g.died.is_none()) {
                let Some(i) = redsim::entity::index_of(w, g.id) else { continue };
                let e = &w.entities.list[i];
                let EntityKind::Mob(m) = &e.kind else { continue };
                g.least_overlap = g.least_overlap.min(portal_overlap(e.pos, o.block_row));
                g.most_move = g.most_move.max(((e.pos.x - g.at.x).powi(2) + (e.pos.z - g.at.z).powi(2)).sqrt());
                g.fastest = g.fastest.max((e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt());
                if let Some(tid) = m.ai.as_deref().and_then(|a| a.target) {
                    if g.targets.last().is_none_or(|(_, s)| !s.ends_with(&format!("#{tid}"))) {
                        let what = if posts.contains(&tid) {
                            "a post's mob"
                        } else if piglins.contains(&tid) {
                            "a zombified piglin"
                        } else if pocket.contains(&tid) || holds.contains(&tid) {
                            "a villager"
                        } else {
                            "another mob"
                        };
                        g.targets.push((tnow - g.born, format!("{what} #{tid}")));
                    }
                }
            }
        }
        if o.trace && golems.iter().any(|g| g.crossed.is_none() && g.died.is_none()) {
            for g in golems.iter().filter(|g| g.crossed.is_none() && g.died.is_none()) {
                if let Some(i) = redsim::entity::index_of(&run.world, g.id) {
                    let e = &run.world.entities.list[i];
                    out.push(format!("  t {} golem #{} (born {}): {:.3},{:.3},{:.3} v {:.3},{:.3} overlap {:.3}", run.world.time, g.id, g.born, e.pos.x, e.pos.y, e.pos.z, e.delta_movement.x, e.delta_movement.z, portal_overlap(e.pos, o.block_row)));
                }
            }
        }
    }
    // the cadence: a golem at every summon tick
    for c in 0..o.cycles {
        let st = first + c * 100;
        match summons.iter().find(|(t, _)| *t == st) {
            Some((_, Some(_))) => {}
            Some((_, None)) => lost.push(format!("T {st}: no spot")),
            None => lost.push(format!("T {st}: no summon (a summoner golem-detected or not willing){}", diag.get(&st).map_or(String::new(), |d| format!(" [{d}]")))),
        }
    }
    let main: Vec<&Golem> = golems.iter().filter(|g| !g.twin).collect();
    // (the cadence: a summon lost to golem detection is the posts' or a lingering golem's; a
    // summon that found no spot is the pad's own P: each caller's 10 tries over the spawn cells in
    // its 17 x 17 square, against 289)
    let no_call = lost.iter().filter(|l| l.contains("no summon")).count();
    let no_spot = lost.len() - no_call;
    let n_of = |cx: f64| -> f64 {
        let bx = cx.floor() as i32;
        FLOOR_ROWS.iter().map(|_| (FLOOR_X.0..=FLOOR_X.1).filter(|x| (x - bx).abs() <= 8).count()).sum::<usize>() as f64
    };
    let lane_x = [-0.5f64, 3.5, 7.5];
    let p_miss: f64 = lane_x.iter().take(o.callers.max(1)).map(|x| (1.0 - n_of(*x) / 289.0).powi(10)).product();
    let p_miss3: f64 = lane_x.iter().map(|x| (1.0 - n_of(*x) / 289.0).powi(10)).product();
    let all_crossed = golems.iter().all(|g| g.crossed.is_some());
    let longest = golems.iter().map(|g| g.crossed_in().unwrap_or(i64::MAX)).max().unwrap_or(0);
    let mut hist: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
    for g in &golems {
        *hist.entry(g.crossed_in().unwrap_or(i64::MAX)).or_default() += 1;
    }
    let targeting: Vec<String> = golems.iter().filter(|g| !g.targets.is_empty()).map(|g| format!("golem #{} (born T {}{}): {}", g.id, g.born, if g.twin { ", a twin" } else { "" }, g.targets.iter().map(|(a, s)| format!("{s} at its tick {a}")).collect::<Vec<_>>().join(", "))).collect();
    let at_post = golems.iter().filter(|g| g.targets.iter().any(|(_, s)| s.starts_with("a post"))).count();
    let late_sightings = sightings.iter().filter(|(st, _, priced, stale)| st.rem_euclid(100) >= 3 && !priced && !stale).count();
    let stale_sightings = sightings.iter().filter(|(st, _, priced, stale)| st.rem_euclid(100) >= 3 && !priced && *stale).count();
    let priced_sightings = sightings.iter().filter(|(st, _, priced, _)| st.rem_euclid(100) >= 3 && *priced).count();
    // (the sightings that matter to the next batch: from its first landing, T - 49 at s 12 REL
    // T+4; in the station the next summoners are new riders, so a sighting by this bench's
    // standing summoners before that is the departing batch's)
    let next_batch_sightings = sightings.iter().filter(|(st, _, _, _)| st.rem_euclid(100) >= 51).count();
    out.push(format!(
        "summons: {} of {} cycles made a golem ({} lost to no spot: the pad's own miss, expected {:.1}% with the first {} caller(s) forced, {:.1}% with all three calling; {} lost to golem detection or no willing caller){}; golems {} ({} twins): crossed {}, died {}, parked {}; the tick of its own life in which each crossed (its first tick 1): {}; the latest {}",
        o.cycles as usize - lost.len(),
        o.cycles,
        no_spot,
        100.0 * p_miss,
        o.callers,
        100.0 * p_miss3,
        no_call,
        if lost.is_empty() { String::new() } else { format!(" (lost: {})", lost.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) },
        golems.len(),
        golems.len() - main.len(),
        golems.iter().filter(|g| g.crossed.is_some()).count(),
        golems.iter().filter(|g| g.died.is_some()).count(),
        golems.iter().filter(|g| g.crossed.is_none() && g.died.is_none()).count(),
        hist.iter().map(|(k, v)| if *k == i64::MAX { format!("never: {v}") } else { format!("{k}: {v}") }).collect::<Vec<_>>().join(", "),
        if longest == i64::MAX { "never crossed".to_string() } else { format!("its tick {longest}") }
    ));
    out.push(format!(
        "golem targets: {} golem(s) took a target before crossing, {} a post's zombie{}; the least overlap with a portal row {:.3} (spawn 0.200), the largest move {:.3}, the fastest {:.3} b/t",
        targeting.len(),
        at_post,
        if targeting.is_empty() { String::new() } else { format!(" ({})", targeting.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) },
        golems.iter().map(|g| g.least_overlap).fold(f64::INFINITY, f64::min),
        golems.iter().map(|g| g.most_move).fold(0.0, f64::max),
        golems.iter().map(|g| g.fastest).fold(0.0, f64::max)
    ));
    out.push(format!(
        "the next batch: golems still in the overworld at a summon tick {lingering_at_summon}; the summoners' golem sightings 3 or more ticks after a summon tick with a golem in the overworld {late_sightings} (from the stored living list after every golem crossed, no reload: {stale_sightings}; from the next batch's first landing, T - 49: {next_batch_sightings}); the posts' mobs dead {}{}; at each dawn (day time 0) the posts' mobs alive: {}; zombified piglins {} placed, {} through a portal",
        post_deaths.len(),
        if post_deaths.is_empty() { String::new() } else { format!(" (first at game time {})", post_deaths[0].0) },
        if dawns.is_empty() { "no dawn in the run".to_string() } else { dawns.iter().map(|(t, a)| format!("game time {t}: {a}/{}", posts.len())).collect::<Vec<_>>().join(", ") },
        piglins.len(),
        piglin_moves
    ));
    out.push("a cart or rider on the pad edge: none can reach it (the pockets' riders at y 0, the pad's golems at feet -6..-3.3); a pillager's arrow: its AI is not modeled here (tick-exact's bridge: 136 golems crossed at +2, 0 shots)".into());
    out.push(format!(
        "model errors (each needs the bridge): {}{}",
        model_errors.len(),
        if model_errors.is_empty() { String::new() } else { format!(": {}", model_errors.iter().take(8).cloned().collect::<Vec<_>>().join(" | ")) }
    ));
    out.push(format!(
        "golem attempts by villagers other than the pockets' summoners (a queue or lane rider, a napper): {}{}",
        other_tries.len(),
        if other_tries.is_empty() { String::new() } else { format!(" ({})", other_tries.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) }
    ));
    let oob_run = crate::drop::oob_records(&run.world).saturating_sub(oob_at_start);
    let dropped = place_drop + oob_run;
    out.push(format!(
        "cells outside the world's box (the design's extent + 4; dropped): the design's at the placement {place_drop} (redsim's records of them {oob_at_start}), the run's writes {oob_run}{}{}",
        if o.box_short > 0 { format!(" (THE CONTROL: the world built {} short of the design's top)", o.box_short) } else { String::new() },
        if dropped + oob_at_start == 0 { String::new() } else { format!(": {}", run.world.errors.iter().filter(|e| e.starts_with("setBlock outside the box")).take(4).cloned().collect::<Vec<_>>().join(" | ")) }
    ));
    let ok = no_call == 0 && all_crossed && longest <= 2 && golem_hurt.is_empty() && golem_hits.is_empty() && lingering_at_summon == 0 && late_sightings == 0 && next_batch_sightings == 0 && post_deaths.is_empty();
    out.push(format!(
        "golem damage: taken {}, dealt (a mob's hit while a golem was in the overworld) {}{}; the posts' mobs taking a golem as target: {}{}",
        golem_hurt.len(),
        golem_hits.len(),
        if golem_hurt.is_empty() && golem_hits.is_empty() { String::new() } else { format!(": {}", golem_hurt.iter().chain(golem_hits.iter()).take(6).cloned().collect::<Vec<_>>().join("; ")) },
        post_targets.len(),
        if post_targets.is_empty() { String::new() } else { format!(" ({})", post_targets.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) }
    ));
    if o.restart.is_some() || o.restart_once.is_some() {
        out.push(format!(
            "priced restart costs (ironspec 2026-11-12 07:42: a reload landing in a new golem's first two ticks can let the zombie scarer hit it once for 3 HP, it still leaves on time; the departing batch re-detecting the crossed golem from its stored list costs no summon): S2's hits {}{}; the departing batch's sightings after a reload {priced_sightings}",
            priced_hits.len(),
            if priced_hits.is_empty() { String::new() } else { format!(" ({})", priced_hits.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) }
        ));
    }
    out.push(format!(
        "verdict: {}{}",
        if ok {
            "PASS (every golem through a portal by its second tick, no damage taken or dealt, nothing shot, no summon lost to golem detection, no golem in the overworld at a summon tick, the summoners see no golem 3+ ticks after a summon, the posts alive; targets a printed count)".to_string()
        } else {
            format!(
                "FAIL ({})",
                [
                    (no_call > 0, "summons lost to golem detection"),
                    (!all_crossed, "a golem did not cross"),
                    (longest > 2 && longest != i64::MAX, "a golem crossed after its second tick"),
                    (!golem_hurt.is_empty() || !golem_hits.is_empty(), "golem damage"),
                    (lingering_at_summon > 0, "a golem in the overworld at a summon tick"),
                    (late_sightings > 0, "a summoner sensed a golem 3+ ticks after a summon"),
                    (next_batch_sightings > 0, "the next batch golem-detected"),
                    (!post_deaths.is_empty(), "a post died"),
                ]
                .iter()
                .filter(|(b, _)| *b)
                .map(|(_, s)| *s)
                .collect::<Vec<_>>()
                .join(", ")
            )
        },
        // (G1 on 44d4bbfae: a verdict beside the model's gaps, each needing the bridge, and the
        // cells dropped outside the world's box)
        if model_errors.is_empty() { String::new() } else { format!(" WITH {} model errors (each needs the bridge; listed above)", model_errors.len()) }
            + &if dropped == 0 { String::new() } else { format!(" WITH {dropped} cells dropped outside the world's box") }
    ));
    Ok((out, ok))
}
