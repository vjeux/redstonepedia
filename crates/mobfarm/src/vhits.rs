//! (ironspec 14:52 / 15:37 / 16:45, G1 14:51 (4) / 15:29 / 15:47 / 16:47; G1's CHANGES 5 on R4, 18:30 / 18:34)
//! The vertical judge's normal-operation negative, as a planter: plant carts in a design (positions and
//! stored motion given, a crest file's rows, or the file's own carts), tick, and judge EVERY cart each
//! tick through the ONE contact rule (`contacts::cart_hits` and `contacts::cart_vertical_hits`, as the
//! way-cell ride rows' bounded judge reads them); every hit is printed with its class, face and depth,
//! and counted. With `--judge`, every hit is judged by main's bounded judge against the declarations
//! inside a required, printed scope. The set FAILS (exit 1, unless --report-only) on any failure, and
//! on a set that judged no cart-tick (NOT JUDGED).

use redsim::entity::EntityKind;
use redsim::entity::mob::farm::{FarmSpec, RunOpts, WalkMode};

/// A planted cart: its position and stored motion (deltaMovement) at the summon, whether it carries a
/// rider (the --rider NBT), and where it came from (printed).
#[derive(Clone, Debug, PartialEq)]
pub struct Planted {
    pub pos: [f64; 3],
    pub motion: [f64; 3],
    pub rider: bool,
    pub label: String,
}

/// `X,Y,Z` or `X,Y,Z,VX,VY,VZ` (spaces allowed; a unicode minus read as '-'): strict, every value finite
/// (G1's B8 on R4: "nan" is no coordinate). The cart carries the --rider passenger when one is given.
pub fn parse_planted(s: &str) -> Result<Planted, String> {
    let v: Vec<f64> = s
        .split(',')
        .map(|t| {
            let x = t.trim().replace('\u{2212}', "-").parse::<f64>().map_err(|e| format!("planted cart {s:?}: {t:?}: {e}"))?;
            if x.is_finite() { Ok(x) } else { Err(format!("planted cart {s:?}: {t:?} is not a finite number")) }
        })
        .collect::<Result<_, _>>()?;
    match v[..] {
        [x, y, z] => Ok(Planted { pos: [x, y, z], motion: [0.0; 3], rider: true, label: format!("planted {s}") }),
        [x, y, z, vx, vy, vz] => Ok(Planted { pos: [x, y, z], motion: [vx, vy, vz], rider: true, label: format!("planted {s}") }),
        _ => Err(format!("planted cart {s:?}: X,Y,Z or X,Y,Z,VX,VY,VZ")),
    }
}

/// A carts file: one planted cart a line (`#` starts a comment; blank lines skipped); strict. Returns the
/// carts and the file's comments (printed with the set: what the set is, which carts were left out and
/// why; G1's B5 on R4).
pub fn parse_carts_file(text: &str) -> Result<(Vec<Planted>, Vec<String>), String> {
    let mut carts = Vec::new();
    let mut notes = Vec::new();
    for (n, l) in text.lines().enumerate() {
        let (body, note) = l.split_once('#').map_or((l, None), |(b, c)| (b, Some(c.trim())));
        if let Some(c) = note.filter(|c| !c.is_empty()) {
            notes.push(c.to_string());
        }
        let body = body.trim();
        if body.is_empty() {
            continue;
        }
        let mut p = parse_planted(body).map_err(|e| format!("line {}: {e}", n + 1))?;
        p.label = format!("line {}: {body}", n + 1);
        carts.push(p);
    }
    Ok((carts, notes))
}

/// (tick-exact 18:17: the crest set) A crest file's rows `crest <cart> t<tick> | <TAG> |t<tick>: <cart>
/// pos [x d, y d, z d] motion [vx d, vy d, vz d] rider <id>|none block ...`: for every crest, its row
/// tagged `tag` (e.g. `crest-3`) planted, y moved by `y_offset` (the file's world y to the design's
/// frame), with a rider when the row has one; strict: every crest has exactly one such row.
pub fn parse_crest_file(text: &str, tag: &str, y_offset: f64) -> Result<Vec<Planted>, String> {
    let mut crests: std::collections::BTreeMap<String, Option<Planted>> = std::collections::BTreeMap::new();
    let vec3 = |l: &str, key: &str| -> Result<[f64; 3], String> {
        let i = l.find(key).ok_or_else(|| format!("{l:?}: no {key}"))? + key.len();
        let j = l[i..].find(']').ok_or_else(|| format!("{l:?}: {key} not closed"))? + i;
        let v: Vec<f64> = l[i..j]
            .split(',')
            .map(|t| t.trim().trim_end_matches('d').parse::<f64>().map_err(|e| format!("{l:?}: {key}: {t:?}: {e}")))
            .collect::<Result<_, _>>()?;
        match v[..] {
            [a, b, c] if a.is_finite() && b.is_finite() && c.is_finite() => Ok([a, b, c]),
            _ => Err(format!("{l:?}: {key} is not 3 finite numbers")),
        }
    };
    for l in text.lines().filter(|l| l.starts_with("crest ")) {
        let parts: Vec<&str> = l.splitn(3, '|').collect();
        let [id, t, rest] = parts[..] else { return Err(format!("{l:?}: not `crest <cart> t<tick> | <tag> | <row>`")) };
        let id = id.trim().to_string();
        let e = crests.entry(id.clone()).or_insert(None);
        if t.trim() != tag {
            continue;
        }
        if e.is_some() {
            return Err(format!("{id}: two {tag} rows"));
        }
        let pos = vec3(rest, "pos [")?;
        let motion = vec3(rest, "motion [")?;
        let rider = match rest.split("rider ").nth(1).and_then(|r| r.split_whitespace().next()) {
            Some("none") => false,
            Some(_) => true,
            None => return Err(format!("{l:?}: no rider field")),
        };
        *e = Some(Planted { pos: [pos[0], pos[1] + y_offset, pos[2]], motion, rider, label: format!("{id}, its {tag} row{}", rest.split(':').next().map_or(String::new(), |s| format!(" ({})", s.trim()))) });
    }
    if crests.is_empty() {
        return Err("no `crest` row in the file".to_string());
    }
    crests.into_iter().map(|(id, p)| p.ok_or_else(|| format!("{id}: no {tag} row"))).collect()
}

/// The planter's run.
pub struct Opts<'a> {
    pub ticks: u32,
    pub own_carts: bool,
    pub seed: i64,
    /// each planted cart in its own copy of the world (a crest set: carts from different ticks)
    pub separately: bool,
    /// the passenger NBT of a planted cart with a rider
    pub rider: Option<&'a str>,
    /// with --judge: the in-scope declarations and the scope's printed line
    pub judge: Option<(&'a [crate::contacts::Decl], String)>,
}

fn is_rail(b: &redsim::block::Block) -> bool {
    b.to_string().contains("rail")
}

/// Plant the carts (each in its own world copy with `separately`), add the design's own carts when
/// `own_carts`, tick `ticks` times and judge every cart each tick. Returns the lines and the set's
/// failure count: without --judge every hit; with it, main's bounded judge's failures (undeclared hits,
/// exceeded bounds, stalls, stale, wiring errors) over the in-scope declarations; a set with no judged
/// cart-tick fails as NOT JUDGED.
pub fn vhits_lines(spec: &FarmSpec, carts: &[Planted], o: &Opts) -> Result<(Vec<String>, usize), String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(o.seed, o.ticks + 10, WalkMode::Random) };
    let (base, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    // (G1's B13 on R4; ironspec 14:57) the seed and the spawner's state: the planter's world has no
    // natural spawner
    if base.world.mob.natural.is_some() {
        return Err("the planter's world has a natural spawner (mob.natural set): planted sets are judged without one".to_string());
    }
    let mut lines = vec![format!(
        "(vhits) seed {}; natural spawning OFF (mob.natural None); {} tick(s) a set; a SAMPLE is one planted cart (or one of the design's own), the rate's unit \"carts\"{}",
        o.seed,
        o.ticks,
        if o.separately { "; each planted cart alone in its own copy of the world" } else { "" }
    )];
    if carts.iter().any(|p| p.rider) && o.rider.is_none() && !carts.is_empty() {
        return Err("a planted cart carries a rider: give its passenger NBT with --rider".to_string());
    }
    let groups: Vec<Vec<&Planted>> = if o.separately { carts.iter().map(|p| vec![p]).collect() } else { vec![carts.iter().collect()] };
    let (mut horizontal, mut vertical) = (0usize, 0usize);
    let mut by_class: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let (mut judged, mut n_carts) = (0u64, 0u32);
    // every hit and each cart's motion, keyed by the set's cart number (unique over the groups: the
    // judge's cart key and the hit's sample)
    let mut all_hits: Vec<crate::contacts::Hit> = Vec::new();
    let mut motion: std::collections::HashMap<(u32, i64), (f64, f64)> = std::collections::HashMap::new();
    // the set's carts' last judged tick: an event still open there has no stall window
    let mut last_tick: std::collections::HashMap<u32, i64> = std::collections::HashMap::new();
    for g in &groups {
        let mut run = base.clone();
        let mut ids: Vec<(u32, u32)> = Vec::new();
        for p in g {
            let passengers = if p.rider { o.rider.map(|r| format!(",Passengers:[{r}]")).unwrap_or_default() } else { String::new() };
            let nbt = format!("{{Motion:[{:.6}d,{:.6}d,{:.6}d]{passengers}}}", p.motion[0], p.motion[1], p.motion[2]);
            let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", p.pos, Some(&nbt))?;
            n_carts += 1;
            let key = n_carts;
            ids.push((id, key));
            // (G1's B8 on R4) the plant read back: the cart's stored position and motion, its cell's
            // block, its passenger
            let e = run.world.entities.list.iter().find(|e| e.id == id).ok_or("the planted cart is gone at once")?;
            let cell = redsim::pos::BlockPos::new(e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32);
            let passenger = run.world.entities.list.iter().find(|r| r.vehicle == Some(id)).map_or("none".to_string(), |r| match &r.kind {
                EntityKind::Mob(m) => format!("{:?}", m.ty),
                k => format!("{:?}", std::mem::discriminant(k)),
            });
            lines.push(format!(
                "(vhits) cart #{key} ({}): read back at ({:.4}, {:.4}, {:.4}) motion ({:.4}, {:.4}, {:.4}), in {} at ({},{},{}), passenger {passenger}",
                p.label, e.pos.x, e.pos.y, e.pos.z, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z, run.world.get(cell), cell.x, cell.y, cell.z
            ));
        }
        if o.own_carts {
            // the design's own entities are summoned by the farm runner's arrival (after the settle): its
            // carts (and their riders) as laid
            run.arrive()?;
            let own: Vec<u32> = run.world.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !ids.iter().any(|(i, _)| *i == e.id)).map(|e| e.id).collect();
            for id in own {
                n_carts += 1;
                let key = n_carts;
                ids.push((id, key));
                let e = run.world.entities.list.iter().find(|e| e.id == id).ok_or("the design's cart is gone")?;
                let passenger = run.world.entities.list.iter().find(|r| r.vehicle == Some(id)).map_or("none".to_string(), |r| match &r.kind {
                    EntityKind::Mob(m) => format!("{:?}", m.ty),
                    k => format!("{:?}", std::mem::discriminant(k)),
                });
                lines.push(format!("(vhits) cart #{key} (the design's own, entity {id}): at ({:.4}, {:.4}, {:.4}), passenger {passenger}", e.pos.x, e.pos.y, e.pos.z));
            }
        }
        // (ironspec 14:52: classify before declaring) each cart's first contact tick printed with its moves
        let mut evidenced: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        for _ in 0..o.ticks {
            run.tick();
            let t = run.world.time;
            for &(id, key) in &ids {
                let Some(i) = run.world.entities.list.iter().position(|e| e.id == id && !e.removed) else { continue };
                // (G1's B7 on R4) a cart-tick counts as judged only when the cart made its move this tick
                if !run.world.entities.list[i].self_moves.iter().any(|m| m.tick == t) {
                    continue;
                }
                judged += 1;
                last_tick.insert(key, t);
                let hz = crate::contacts::cart_hits(&run.world, i);
                let vt = crate::contacts::cart_vertical_hits(&run.world, i);
                {
                    let dm = run.world.entities.list[i].delta_movement;
                    motion.insert((key, t), ((dm.x * dm.x + dm.z * dm.z).sqrt(), dm.y));
                }
                let tag = |h: crate::contacts::Hit| crate::contacts::Hit { cart: key, sample: Some(key as u64), ..h };
                all_hits.extend(hz.iter().cloned().map(tag));
                all_hits.extend(vt.iter().cloned().map(crate::contacts::Hit::from).map(tag));
                if (!hz.is_empty() || !vt.is_empty()) && evidenced.insert(key) {
                    let e = &run.world.entities.list[i];
                    let cell = redsim::pos::BlockPos::new(e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32);
                    // (G1's B6 on R4) the cart's rail state read from the world: a rail in its cell or the cell below
                    let on_rail = is_rail(&run.world.get(cell)) || is_rail(&run.world.get(cell.below()));
                    for m in e.self_moves.iter().filter(|m| m.tick == t) {
                        let b = m.bb_before;
                        lines.push(format!(
                            "(vhits) t {t} cart #{key}: the first contact tick's move (evidence): Entity.move(SELF) requested ({:.4}, {:.4}, {:.4}) returned ({:.4}, {:.4}, {:.4}); the box before the move x {:.4}..{:.4} y {:.4}..{:.4} z {:.4}..{:.4}; the cart in {} ({})",
                            m.requested.x, m.requested.y, m.requested.z, m.returned.x, m.returned.y, m.returned.z, b.min_x, b.max_x, b.min_y, b.max_y, b.min_z, b.max_z,
                            run.world.get(cell),
                            if on_rail { "a rail in its cell or below" } else { "off-rail: no rail in its cell or below" }
                        ));
                    }
                }
                for h in hz {
                    horizontal += 1;
                    *by_class.entry(format!("horizontal face {}", h.face.name())).or_default() += 1;
                    lines.push(format!("(vhits) t {t} cart #{key}: HIT horizontal ({},{},{}) {} face {} depth {:.4} speed {:.4} -> {:.4}", h.cell.x, h.cell.y, h.cell.z, run.world.get(h.cell), h.face.name(), h.depth, h.speed_in, h.speed_out));
                }
                for v in vt {
                    vertical += 1;
                    *by_class.entry(format!("{:?}", v.class)).or_default() += 1;
                    lines.push(format!("(vhits) t {t} cart #{key}: HIT {:?} ({},{},{}) {} face {} depth {:.4} speed {:.4} -> {:.4}", v.class, v.cell.x, v.cell.y, v.cell.z, run.world.get(v.cell), v.class.face().name(), v.depth, v.speed_in, v.speed_out));
                }
            }
        }
    }
    if n_carts == 0 {
        return Err("no cart to judge: plant one (--cart / --carts-file / --crest-file) or take the design's own (--own-carts)".to_string());
    }
    let n = horizontal + vertical;
    lines.push(format!(
        "(vhits) {n_carts} cart(s) in {} world(s) x {} tick(s) ({judged} cart-ticks judged through contacts::cart_hits + cart_vertical_hits, a cart-tick counted when the cart made its move): {n} hit(s) ({horizontal} horizontal, {vertical} vertical{}){}",
        groups.len(),
        o.ticks,
        if by_class.is_empty() { String::new() } else { format!(": {}", by_class.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ")) },
        if judged == 0 { "; NOT JUDGED (0 cart-ticks): FAIL" } else { "" }
    ));
    // (G1 15:47 (3)) a set with no judged cart-tick is NOT JUDGED and fails
    if judged == 0 {
        return Ok((lines, n.max(1)));
    }
    let Some((decls, scope_line)) = &o.judge else { return Ok((lines, n)) };
    // (G1 16:47; G1's B3 on R4) the hits judged against the in-scope bounded declarations: the judge's
    // failures count, STALE included (the scope holds the declarations the set is to meet)
    lines.push(scope_line.clone());
    let v = crate::contacts::judge(decls, &all_hits, &|c, t| motion.get(&(c, t)).copied(), crate::contacts::StallRule::IRONSPEC_1042, Some((n_carts as u64, "carts")));
    let first = |v: &[String]| if v.is_empty() { String::new() } else { format!(" (first: {})", v.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) };
    let und: Vec<String> = v.undeclared.iter().map(|h| format!("t {} cart #{} ({},{},{}) face {} depth {:.4}", h.tick, h.cart, h.cell.x, h.cell.y, h.cell.z, h.face.name(), h.depth)).collect();
    // an event still open at its cart's last judged tick (a rest the planter's world never releases) has
    // no stall window: printed apart, not a stall (G1's B13 on R4: run --ticks past an event's end + 5 to
    // judge its stall)
    let is_open = |s: &String| -> bool {
        let Some(i) = s.find("cart ") else { return false };
        let key: Option<u32> = s[i + 5..].split(|c: char| !c.is_ascii_digit()).next().and_then(|d| d.parse().ok());
        key.and_then(|k| last_tick.get(&k)).is_some_and(|t| s.contains(&format!("..{t}:")))
    };
    let (open, stalls): (Vec<String>, Vec<String>) = v.stalls.iter().cloned().partition(is_open);
    let failures = v.undeclared.len() + v.exceeded.len() + stalls.len() + v.stale.len() + v.errors.len();
    lines.push(format!(
        "(vhits) main's bounded judge over the {} in-scope declaration(s) (contacts::judge, IRONSPEC_1042, the samples ({n_carts}, \"carts\")): {} undeclared hit(s){}; {} bound(s) exceeded{}; {} stall(s){}; {} event(s) open at their cart's last tick (no stall window: not judged){}; {} stale{}; {} stale not judged{}; {} wiring error(s){}; events per declaration {:?}: {}",
        decls.len(),
        und.len(),
        first(&und),
        v.exceeded.len(),
        first(&v.exceeded),
        stalls.len(),
        first(&stalls),
        open.len(),
        first(&open),
        v.stale.len(),
        first(&v.stale),
        v.stale_not_judged.len(),
        first(&v.stale_not_judged),
        v.errors.len(),
        first(&v.errors),
        v.events,
        if failures > 0 { "FAILS" } else { "PASSES" }
    ));
    Ok((lines, failures))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planted_carts_parse_strictly() {
        let p = parse_planted("1.5, 16.1875,0.49").unwrap();
        assert_eq!((p.pos, p.motion, p.rider), ([1.5, 16.1875, 0.49], [0.0; 3], true));
        assert_eq!(parse_planted("1.5,16.1875,0.49,0,\u{2212}0.04,0.1").unwrap().motion, [0.0, -0.04, 0.1]);
        assert!(parse_planted("1.5,16.1875").is_err());
        assert!(parse_planted("1.5,16.1875,x").is_err());
        // (G1's B8) a non-finite value is no coordinate
        assert!(parse_planted("nan,16,0").unwrap_err().contains("finite"));
        assert!(parse_planted("1,inf,0").is_err());
        let (f, notes) = parse_carts_file("# rests\n1.5,16.1875,0.49\n\n2.5,16.9775,1.28 # slot 2\n").unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!(notes, vec!["rests".to_string(), "slot 2".to_string()]);
        assert!(f[1].label.starts_with("line 4"));
        assert!(parse_carts_file("1,2,3\n1,2\n").is_err());
    }

    /// (tick-exact 18:17) the crest file's rows read strictly: each crest's tagged row, y moved to the
    /// frame, the rider read; a crest without the tagged row, or with two, is refused.
    #[test]
    fn crest_rows_parse_strictly() {
        let f = "header\ncrest e0 t140 | crest-1 |t139: e0 pos [-2.6d, 81.68d, 7.5d] motion [-0.70d, 0.0d, 0.0d] rider e0p0 block minecraft:rail at (-3,1,7)\n\
                 crest e0 t140 | crest-3 |t137: e0 pos [-1.825d, 80.8875d, 7.5d] motion [-0.7669d, 0.0d, 0.0d] rider e0p0 block minecraft:rail at (-2,0,7)\n\
                 crest e9 t500 | crest-3 |t497: e9 pos [3.5d, 96.0d, 1.5d] motion [0.0d, 0.1d, 0.2d] rider none block minecraft:air at (3,16,1)\n";
        let c = parse_crest_file(f, "crest-3", -80.0).unwrap();
        assert_eq!(c.len(), 2);
        assert!((c[0].pos[0] + 1.825).abs() < 1e-9 && (c[0].pos[1] - 0.8875).abs() < 1e-9 && (c[0].pos[2] - 7.5).abs() < 1e-9, "{:?}", c[0].pos);
        assert_eq!(c[0].motion, [-0.7669, 0.0, 0.0]);
        assert!(c[0].rider && !c[1].rider);
        assert!(c[0].label.contains("crest e0 t140") && c[0].label.contains("t137"));
        // a crest without the tagged row
        assert!(parse_crest_file(f, "crest-1", -80.0).unwrap_err().contains("e9 t500: no crest-1 row"));
        // two tagged rows for one crest
        let dup = format!("{f}crest e9 t500 | crest-3 |t497: e9 pos [3.5d, 96.0d, 1.5d] motion [0.0d, 0.1d, 0.2d] rider none block x\n");
        assert!(parse_crest_file(&dup, "crest-3", -80.0).unwrap_err().contains("two crest-3 rows"));
        // a non-number refused
        assert!(parse_crest_file("crest e1 t1 | crest-3 |t0: e1 pos [a, 1, 2] motion [0d, 0d, 0d] rider none\n", "crest-3", 0.0).is_err());
    }
}
