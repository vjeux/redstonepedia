//! `mctas mobai farm-trace`: a farm's exact compare (`mobfarm compare`'s two sides, the mob AI on)
//! with both sides' draws of one tick: the game's log by call site and redsim's level random (or an
//! entity's own random) by call site.

use anyhow::{Result, anyhow};
use mobfarm::exact::{GameFarm, Opts, Place, follow_bonuses, load_place, redsim_start, set_bonuses};
use redsim::entity::EntityKind;
use redsim::entity::mob::farm::SETTLE_TICKS;

pub fn run(version: &str, design: &std::path::Path, seed: i64, at: i64, entity: Option<u32>, terrain: Option<String>, day0: Option<i64>, build: Option<u32>, no_spawning: bool, weather: bool) -> Result<()> {
    let opts = Opts { seed, ticks: (at + 1).max(1) as u32, weather, probe: false, batch: 100, full: true, bare: false, draws_at: Some(at), terrain, day0, build: build.map(|k| (k, 0)), no_spawning, only: Vec::new() };
    let (c, spec, place) = load_place(design, &opts)?;
    // the compare's run options (its day time, the judge's build and weather when asked)
    let mut o = mobfarm::exact::opts_of(&opts);
    if let Place::Terrain { site, .. } = place {
        o.site = Some(site);
    }
    let mut game = GameFarm::start_with(version, &c, &spec, &o, mobfarm::exact::weather_on(&opts), true, place, !no_spawning)?;
    let mut run = redsim_start(&spec, &o, &game, opts.bare)?;
    let mut t: i64 = 0;
    let step = |run: &mut redsim::entity::mob::farm::Run, game_t: i64, settle: bool| {
        if game_t == at {
            let w = &run.world;
            let addr = match entity {
                None => w.level_random.as_ref().map(|r| r as *const _ as usize).unwrap_or(0),
                Some(id) => w.entities.list.iter().find(|e| e.id == id).and_then(|e| e.random.as_ref()).map(|r| r as *const _ as usize).unwrap_or(0),
            };
            eprintln!("redsim's draws at game time {at} ({}):", entity.map(|e| format!("entity #{e}'s random")).unwrap_or("the level random".into()));
            redsim::rng::TRACE.store(addr, std::sync::atomic::Ordering::Relaxed);
        }
        if settle {
            run.settle_tick();
        } else {
            run.tick();
        }
        redsim::rng::TRACE.store(0, std::sync::atomic::Ordering::Relaxed);
    };
    let print = |f: &redsim::exact::Frame| {
        if f.t == at {
            eprintln!("the game's draws at game time {at}:");
            for (s, site, n) in &f.draws {
                eprintln!("  {s} {site} x{n}");
            }
        }
    };
    let mut left = SETTLE_TICKS as u32;
    while left > 0 {
        let n = left.min(opts.batch);
        for f in game.trace(n, f_window(t, n, at))? {
            print(&f);
            step(&mut run, f.t, true);
        }
        t += n as i64;
        left -= n;
    }
    run.arrive().map_err(|e| anyhow!(e))?;
    let mobs = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Mob(_))).count();
    let bonus = follow_bonuses(opts.seed, mobs);
    set_bonuses(&mut run.world, &bonus);
    game.arrive(&c, &spec, &run.world, &bonus)?;
    while t < at + 1 {
        let n = ((at + 1 - t) as u32).min(opts.batch);
        for f in game.trace(n, f_window(t, n, at))? {
            print(&f);
            step(&mut run, f.t, false);
        }
        t += n as i64;
    }
    if !run.world.errors.is_empty() {
        eprintln!("model errors: {:?}", run.world.errors);
    }
    Ok(())
}

fn f_window(t0: i64, n: u32, at: i64) -> bool {
    at >= t0 && at < t0 + n as i64 + 3
}
