//! Weather (mobinfra, 2026-11-08; vjeux: farms are judged with rain, thunder and lightning, on
//! Hard), read in the 26.3 source (`mctas src show ServerLevel`, `Level`, `WeatherAttributes`,
//! `Entity`, `LevelReader`):
//!
//! * **The cycle** (`ServerLevel.advanceWeatherCycle`, first in `ServerLevel.tick` after the world
//!   border): `WeatherData` (server-wide) holds two independent timers. A timer at 0 draws a
//!   duration from the level random: `RAIN_DELAY` / `THUNDER_DELAY` (uniform 12000..180000) while
//!   its flag is off, `RAIN_DURATION` (12000..24000) / `THUNDER_DURATION` (3600..15600) while it is
//!   on; otherwise it counts down and flips its flag at 0. `clearWeatherTime` (the `weather clear`
//!   command) forces both flags off while it counts down, leaving both timers at 1. Then
//!   `thunderLevel` and `rainLevel` move 0.01 a tick toward their flags, clamped to 0..1.
//! * **The levels** (`Level`): `getRainLevel(a) = lerp(a, oRainLevel, rainLevel)`,
//!   `getThunderLevel(a) = lerp(a, oThunderLevel, thunderLevel) * getRainLevel(a)`; `isRaining` =
//!   rain level > 0.2, `isThundering` = thunder level > 0.9 (so thunder needs rain).
//! * **The darkening** (`Level.updateSkyBrightness`, each tick after the weather, before
//!   `tickTime`; `EnvironmentAttributeSystem`: the day timeline's `SKY_LIGHT_LEVEL`, then the
//!   weather layer of `WeatherAttributes`): rain blends the level toward 4 by 0.3125, weighted by
//!   rain level minus thunder level; thunder toward 4 by 0.52734375, weighted by the thunder level;
//!   clamped to 0..15; `skyDarken = (int)(15 - level)`. Daylight detectors read sky light minus it
//!   (`daylight.rs`), mobs the raw brightness `max(block, sky - skyDarken)`.
//! * **Rain at a block** (`Level.precipitationAt`): raining, the block sees the sky (sky light 15),
//!   no motion-blocking block above it (`Heightmap.Types.MOTION_BLOCKING`), and its biome rains
//!   there (the judge's world is plains: temperature 0.8, rain at every height).
//! * **Lightning**: `lightning.rs`.
//!
//! The game draws the weather from the level random, which everything else in the level also
//! draws from: redsim cannot mirror it, so [`Weather::rng`] is a seeded stand-in with the same
//! distributions (the mob layer's policy). A run starts from the cycle's stationary state
//! ([`Weather::stationary`]): a farm runs for weeks, so the weather it meets is the cycle's
//! long-run mix, drawn exactly (no warm-up).
//!
//! Off by default ([`Plan::Off`]): rain and thunder stay at 0 and nothing is drawn, so every world
//! built before weather (the other studies) is unchanged.

pub mod fire;
pub mod lightning;

use crate::block::Block;
use crate::pos::BlockPos;
use crate::rng::LegacyRandom;
use crate::world::World;

/// `ServerLevel.RAIN_DELAY`, `RAIN_DURATION`, `THUNDER_DELAY`, `THUNDER_DURATION` (`UniformInt`
/// bounds, inclusive).
pub const RAIN_DELAY: (i32, i32) = (12000, 180000);
pub const RAIN_DURATION: (i32, i32) = (12000, 24000);
pub const THUNDER_DELAY: (i32, i32) = (12000, 180000);
pub const THUNDER_DURATION: (i32, i32) = (3600, 15600);

/// The dimension's lowest block (`DimensionType.minY` of the overworld): where a bolt lands in a
/// column with nothing in it (the judge's world is void around the farm, like the game's void
/// superflat).
pub const WORLD_MIN_Y: i32 = -64;

/// `UniformInt.sample` = `Mth.randomBetweenInclusive(random, min, max)`.
pub fn uniform(r: &mut LegacyRandom, (min, max): (i32, i32)) -> i32 {
    r.next_int(max - min + 1) + min
}

/// `Mth.lerp(float, float, float)`: `p0 + alpha * (p1 - p0)`.
pub fn lerp(alpha: f32, p0: f32, p1: f32) -> f32 {
    p0 + alpha * (p1 - p0)
}

/// Java's `Float.toString`: the shortest decimal that tells the float apart (the digits Rust's
/// `Display` gives), in plain form with at least one decimal between 10^-3 and 10^7 ("1.0",
/// "0.29999998"), else in Java's scientific form ("7.450581E-9").
pub fn java_float(x: f32) -> String {
    if x == 0.0 {
        return if x.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let a = x.abs();
    if (1.0e-3..1.0e7).contains(&a) {
        let s = format!("{x}");
        if s.contains('.') { s } else { format!("{s}.0") }
    } else {
        let s = format!("{x:e}");
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let m = if m.contains('.') { m.to_string() } else { format!("{m}.0") };
        format!("{m}E{e}")
    }
}

/// `WeatherData`: the timers and flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeatherData {
    pub clear_weather_time: i32,
    pub rain_time: i32,
    pub thunder_time: i32,
    pub raining: bool,
    pub thundering: bool,
}

/// A `weather` command (`WeatherCommand` -> `MinecraftServer.setWeatherParameters(clearTime,
/// rainTime, raining, thundering)`, which sets the thunder timer to the rain timer too).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// `weather clear <ticks>`.
    Clear(i32),
    /// `weather rain <ticks>`.
    Rain(i32),
    /// `weather thunder <ticks>`.
    Thunder(i32),
}

impl Command {
    pub fn name(self) -> String {
        match self {
            Command::Clear(t) => format!("clear for {t} gt"),
            Command::Rain(t) => format!("rain for {t} gt"),
            Command::Thunder(t) => format!("thunder for {t} gt"),
        }
    }
}

/// How a world's weather goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Plan {
    /// No weather: rain and thunder stay at 0, nothing is drawn (the worlds of every other study).
    #[default]
    Off,
    /// The natural cycle (`advance_weather` on) from its stationary state.
    Cycle,
    /// The natural cycle from its stationary state, and a `weather` command `at` game ticks after
    /// the farm's entities arrive (the judge's forced windows: a game day of rain, of thunder).
    Force { at: i64, cmd: Command },
    /// The natural cycle, then `weather rain <rain>` `at` game ticks after the arrival and
    /// `weather thunder <thunder>` when it ends (the game gate's schedule: its days of natural
    /// weather, a forced day of rain, a forced day of thunder).
    Windows { at: i64, rain: i32, thunder: i32 },
}

/// What the weather did, in order (the judge's evidence; cleared by whoever reads it).
#[derive(Clone, Debug, PartialEq)]
pub enum WeatherEvent {
    /// A bolt fell (`visual`: a trap's, which hurts nothing): its id, block, what it aimed at.
    Strike { t: i64, bolt: u32, at: [i32; 3], target: lightning::Target, visual: bool },
    /// A bolt hit an entity (`thunderHit`).
    Hit { t: i64, bolt: u32, entity: u32, what: &'static str },
    /// A villager struck by lightning (a witch in the game: `Villager.thunderHit`).
    Converted { t: i64, entity: u32 },
    /// A boat or minecart destroyed by a bolt (`VehicleEntity.hurtServer`: 50 > 40).
    VehicleDestroyed { t: i64, entity: u32 },
    /// An item entity destroyed by a bolt or a fire (`ItemEntity.hurtServer`).
    ItemDestroyed { t: i64, entity: u32, what: &'static str },
    /// A fire set (`LightningBolt.spawnFire`, `LavaFluid.randomTick`, a fire's spread):
    /// `flammable_near` names lava's fire's lava.
    Fire { t: i64, at: [i32; 3], flammable_near: Option<String> },
    /// A fire went out in rain (`FireBlock.tick`).
    FireOut { t: i64, at: [i32; 3] },
    /// A block burned away (`FireBlock.checkBurnOut`): removed, or turned into fire.
    Burned { t: i64, at: [i32; 3], what: String, into_fire: bool },
    /// A skeleton trap horse spawned (`ServerLevel.tickThunder`, Hard): its block.
    TrapHorse { t: i64, at: [i32; 3] },
    /// A burning entity put out by rain (`Entity.applyEffectsFromBlocks`: `isInRain`).
    RainedOut { t: i64, entity: u32 },
    /// A zombie set on fire by the sun (`Mob.burnUndead`).
    Sunburn { t: i64, entity: u32 },
}

/// The weather of a level: `WeatherData`, the levels, the darkening's inputs, the lightning
/// state, the seeded stand-in random.
#[derive(Clone, Debug)]
pub struct Weather {
    /// The cycle runs (`ServerLevel.canHaveWeather` and `advance_weather`): any plan but `Off`.
    pub on: bool,
    pub data: WeatherData,
    /// `Level.oRainLevel`, `rainLevel`, `oThunderLevel`, `thunderLevel` (the viewer lerps them by
    /// the partial tick like `WeatherEffectRenderer`).
    pub o_rain_level: f32,
    pub rain_level: f32,
    pub o_thunder_level: f32,
    pub thunder_level: f32,
    /// The stand-in for the level random's weather, lightning and fire draws.
    pub rng: LegacyRandom,
    /// Commands to apply at game times, in order (`Plan::Force`, `Plan::Windows`).
    pub pending: Vec<(i64, Command)>,
    /// Lightning: the judge's game settings and the bolts (entities in the game, ticked in the
    /// entity order by id); their fires are blocks (`Block::Fire`, [`fire`]).
    pub lightning: lightning::Settings,
    pub bolts: Vec<lightning::Bolt>,
    /// The record.
    pub events: Vec<WeatherEvent>,
}

impl Default for Weather {
    fn default() -> Weather {
        Weather {
            on: false,
            data: WeatherData::default(),
            o_rain_level: 0.0,
            rain_level: 0.0,
            o_thunder_level: 0.0,
            thunder_level: 0.0,
            rng: LegacyRandom::new(0x7765_6174_6865_72),
            pending: Vec::new(),
            lightning: lightning::Settings::default(),
            bolts: Vec::new(),
            events: Vec::new(),
        }
    }
}

/// One timer of the cycle in its stationary state: (flag, timer, ticks since the flag last
/// flipped). An alternating renewal process: an off interval lasts a delay + 1 ticks, an on one a
/// duration + 1 (the tick that draws holds the flag, the tick that reaches 0 flips it); the
/// interval the process is in is length-biased and the position in it uniform.
fn stationary_timer(r: &mut LegacyRandom, off: (i32, i32), on: (i32, i32)) -> (bool, i32, i32) {
    let mean = |(a, b): (i32, i32)| (a as f64 + b as f64) / 2.0 + 1.0;
    let flag = r.next_double() < mean(on) / (mean(on) + mean(off));
    let (a, b) = if flag { on } else { off };
    // length-biased: draw s uniform in a..=b, keep it with chance (s + 1) / (b + 1)
    let s = loop {
        let s = uniform(r, (a, b));
        if r.next_double() * (b as f64 + 1.0) < s as f64 + 1.0 {
            break s;
        }
    };
    let k = r.next_int(s + 1);
    let timer = if k == 0 { 0 } else { s - (k - 1) };
    (flag, timer, k)
}

/// A level `k + 1` ticks after its flag flipped (from the other end, which the previous interval
/// reached: every interval outlasts the 100 ticks of a full sweep), as `advanceWeatherCycle`'s
/// float steps leave it: (the level before the last step, the level).
fn ramp(up: bool, k: i32) -> (f32, f32) {
    let mut level = if up { 0.0f32 } else { 1.0f32 };
    let mut old = level;
    for _ in 0..=k.min(120) {
        old = level;
        level = if up { level + 0.01f32 } else { level - 0.01f32 };
        level = level.clamp(0.0, 1.0);
    }
    (old, level)
}

impl Weather {
    /// The weather of a plan, seeded: the stationary state of the cycle (`Off`: none).
    pub fn new(plan: Plan, seed: i64) -> Weather {
        let mut w = Weather { rng: LegacyRandom::new(seed ^ 0x5745_4154_4845_52), ..Weather::default() };
        if plan != Plan::Off {
            w.on = true;
            w.stationary();
        }
        w
    }

    /// Draw the cycle's stationary state: each timer, its flag and its level.
    pub fn stationary(&mut self) {
        let (raining, rain_time, rk) = stationary_timer(&mut self.rng, RAIN_DELAY, RAIN_DURATION);
        let (thundering, thunder_time, tk) = stationary_timer(&mut self.rng, THUNDER_DELAY, THUNDER_DURATION);
        self.data = WeatherData { clear_weather_time: 0, rain_time, thunder_time, raining, thundering };
        (self.o_rain_level, self.rain_level) = ramp(raining, rk);
        (self.o_thunder_level, self.thunder_level) = ramp(thundering, tk);
    }

    /// `MinecraftServer.setWeatherParameters` from a `weather` command.
    pub fn command(&mut self, c: Command) {
        let (clear, rain, raining, thundering) = match c {
            Command::Clear(t) => (t, 0, false, false),
            Command::Rain(t) => (0, t, true, false),
            Command::Thunder(t) => (0, t, true, true),
        };
        self.data = WeatherData { clear_weather_time: clear, rain_time: rain, thunder_time: rain, raining, thundering };
    }

    /// `ServerLevel.advanceWeatherCycle` (`advance_weather` on), then the levels.
    /// `ServerLevel.advanceWeatherCycle` on the weather's stand-in random (a run without a
    /// mirrored level random).
    pub fn advance(&mut self) {
        let mut r = self.rng.clone();
        self.advance_with(&mut r);
        self.rng = r;
    }

    /// `ServerLevel.advanceWeatherCycle` drawing from `r` (the level random when the world mirrors
    /// the bridge's: `UniformInt.sample` -> `nextIntBetweenInclusive`, in the game's order: thunder
    /// first, then rain; tick-exact lane 2026-11-08).
    pub fn advance_with(&mut self, r: &mut LegacyRandom) {
        if !self.on {
            return;
        }
        let d = &mut self.data;
        if d.clear_weather_time > 0 {
            d.clear_weather_time -= 1;
            d.thunder_time = if d.thundering { 0 } else { 1 };
            d.rain_time = if d.raining { 0 } else { 1 };
            d.thundering = false;
            d.raining = false;
        } else {
            if d.thunder_time > 0 {
                d.thunder_time -= 1;
                if d.thunder_time == 0 {
                    d.thundering = !d.thundering;
                }
            } else if d.thundering {
                d.thunder_time = uniform(r, THUNDER_DURATION);
            } else {
                d.thunder_time = uniform(r, THUNDER_DELAY);
            }
            if d.rain_time > 0 {
                d.rain_time -= 1;
                if d.rain_time == 0 {
                    d.raining = !d.raining;
                }
            } else if d.raining {
                d.rain_time = uniform(r, RAIN_DURATION);
            } else {
                d.rain_time = uniform(r, RAIN_DELAY);
            }
        }
        self.o_thunder_level = self.thunder_level;
        self.thunder_level += if self.data.thundering { 0.01f32 } else { -0.01f32 };
        self.thunder_level = self.thunder_level.clamp(0.0, 1.0);
        self.o_rain_level = self.rain_level;
        self.rain_level += if self.data.raining { 0.01f32 } else { -0.01f32 };
        self.rain_level = self.rain_level.clamp(0.0, 1.0);
    }

    /// `Level.getRainLevel(a)`.
    pub fn rain(&self, a: f32) -> f32 {
        lerp(a, self.o_rain_level, self.rain_level)
    }

    /// `Level.getThunderLevel(a)`.
    pub fn thunder(&self, a: f32) -> f32 {
        lerp(a, self.o_thunder_level, self.thunder_level) * self.rain(a)
    }

    /// The `WeatherData` and levels the tick-exact bridge starts from (`(clear, rain, thunder,
    /// raining, thundering, rain level, thunder level)`): the harness hands redsim's to the bridge
    /// so both begin equal without a shared draw.
    pub fn bridge_state(&self) -> (i32, i32, i32, bool, bool, f32, f32) {
        let d = &self.data;
        (d.clear_weather_time, d.rain_time, d.thunder_time, d.raining, d.thundering, self.rain_level, self.thunder_level)
    }

    /// The weather as the bridge's traced frame prints it (`Exact.java`: `"clear C rain R thunder
    /// T[ raining][ thundering] levels <getRainLevel(1)> <getThunderLevel(1)>"`, Java's
    /// `Float.toString` for the levels).
    pub fn bridge_text(&self) -> String {
        let d = &self.data;
        format!(
            "clear {} rain {} thunder {}{}{} levels {} {}",
            d.clear_weather_time,
            d.rain_time,
            d.thunder_time,
            if d.raining { " raining" } else { "" },
            if d.thundering { " thundering" } else { "" },
            java_float(self.rain(1.0)),
            java_float(self.thunder(1.0))
        )
    }

    /// `Level.isRaining()`.
    pub fn is_raining(&self) -> bool {
        self.on && self.rain(1.0) as f64 > 0.2
    }

    /// `Level.isThundering()`.
    pub fn is_thundering(&self) -> bool {
        self.on && self.thunder(1.0) as f64 > 0.9
    }
}

/// The cycle's thunderstorms in the long run (lightning strikes only while `isRaining()` and
/// `isThundering()`, `ServerLevel.tickChunk`): their share of the ticks and their mean length (gt).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Storms {
    pub share: f64,
    pub mean_len: f64,
}

/// [`storm_stats`] over 200,000 game days, once per process.
pub fn storms() -> Storms {
    static S: std::sync::OnceLock<Storms> = std::sync::OnceLock::new();
    *S.get_or_init(|| storm_stats(0x5354_4f52_4d53, 200_000))
}

/// The thunderstorms of [`Weather::advance`] from the stationary state of `seed` over `days` game
/// days: exact, skipping only the stretches where no timer ends and both levels sit at their
/// bound (nothing is drawn and nothing changes there).
pub fn storm_stats(seed: i64, days: i64) -> Storms {
    let mut wx = Weather::new(Plan::Cycle, seed);
    let total = days * 24000;
    let (mut t, mut on, mut ticks, mut storms) = (0i64, false, 0i64, 0i64);
    let settled = |level: f32, up: bool| if up { level >= 1.0 } else { level <= 0.0 };
    while t < total {
        let d = wx.data;
        if d.clear_weather_time == 0 && d.thunder_time > 1 && d.rain_time > 1 && settled(wx.thunder_level, d.thundering) && settled(wx.rain_level, d.raining) && wx.o_thunder_level == wx.thunder_level && wx.o_rain_level == wx.rain_level {
            let n = ((d.thunder_time.min(d.rain_time) - 1) as i64).min(total - t);
            wx.data.thunder_time -= n as i32;
            wx.data.rain_time -= n as i32;
            if on {
                ticks += n;
            }
            t += n;
            continue;
        }
        wx.advance();
        t += 1;
        let now = wx.is_raining() && wx.is_thundering();
        if now {
            ticks += 1;
            if !on {
                storms += 1;
            }
        }
        on = now;
    }
    Storms { share: ticks as f64 / total as f64, mean_len: if storms > 0 { ticks as f64 / storms as f64 } else { 0.0 } }
}

/// The weather's part of `ServerLevel.tick`, first in it (after the world border, which redsim
/// does not model): a `weather` command due now (commands run before the levels tick:
/// `MinecraftServer.tickChildren`), then `advanceWeatherCycle`. (`updateSkyBrightness` follows in
/// the game; [`World::sky_darken`] computes the same value from the same inputs on demand.)
pub fn tick(w: &mut World) {
    if !w.weather.on {
        return;
    }
    while let Some(&(at, c)) = w.weather.pending.first()
        && w.time >= at
    {
        w.weather.command(c);
        w.weather.pending.remove(0);
    }
    // the level random when the world mirrors a bridge sandbox (`exact_seed`: set with the level
    // random by `mirror_level_random`): its draws in the game's place, first in the tick; else
    // the weather's stand-in (a farm run's level random is a stand-in too: its weather keeps its
    // own stream)
    let mirrored = w.exact_seed.is_some();
    match w.level_random.as_mut() {
        Some(r) if mirrored => w.weather.advance_with(r),
        _ => w.weather.advance(),
    }
}

/// The weather over `ticks` game ticks from `w.time` while the farm's chunks do not tick (an
/// absence: the level keeps advancing its weather, `ServerLevel.advanceWeatherCycle` runs for every
/// level each tick): the cycle's timers, the levels, the forced commands that come due; no bolt.
/// Call it before moving `w.time`.
pub fn skip(w: &mut World, ticks: i64) {
    if !w.weather.on {
        return;
    }
    for k in 1..=ticks {
        let now = w.time + k;
        while let Some(&(at, c)) = w.weather.pending.first()
            && now >= at
        {
            w.weather.command(c);
            w.weather.pending.remove(0);
        }
        let mirrored = w.exact_seed.is_some();
        match w.level_random.as_mut() {
            Some(r) if mirrored => w.weather.advance_with(r),
            _ => w.weather.advance(),
        }
    }
}

/// `Level.updateSkyBrightness` at day time `day` with these weather levels
/// (`getRainLevel(1)`, `getThunderLevel(1)`): the day timeline's level, the rain then the thunder
/// layer (`WeatherAttributes.addLayer`, `FloatModifier.ALPHA_BLEND`, the float type's state-change
/// lerp), `sanitizeValue` (0..15), `(int)(15 - level)`.
pub fn sky_darken_at(day: i64, rain: f32, thunder: f32) -> i32 {
    let mut v = crate::daylight::sky_light_level(day);
    let rain_part = rain - thunder;
    if rain_part > 0.0 {
        let target = lerp(0.3125, v, 4.0);
        v = lerp(rain_part, v, target);
    }
    if thunder > 0.0 {
        let target = lerp(0.527_343_75, v, 4.0);
        v = lerp(thunder, v, target);
    }
    (15.0f32 - v.clamp(0.0, 15.0)) as i32
}

impl World {
    /// `Level.getSkyDarken()`: the day timeline (the mob layer's day time when it runs, else the
    /// judge's frozen [`crate::daylight::DAY_TIME`]) with this level's rain and thunder.
    pub fn sky_darken(&self) -> i32 {
        let day = if self.mob.rng.is_some() { self.day_time() } else { crate::daylight::DAY_TIME };
        if !self.weather.on {
            return crate::daylight::sky_darken(day);
        }
        sky_darken_at(day, self.weather.rain(1.0), self.weather.thunder(1.0))
    }

    /// `Level.getRainLevel(partial)` (for the viewer).
    pub fn rain_level(&self, partial: f32) -> f32 {
        self.weather.rain(partial)
    }

    /// `Level.getThunderLevel(partial)` (for the viewer).
    pub fn thunder_level(&self, partial: f32) -> f32 {
        self.weather.thunder(partial)
    }

    pub fn is_raining(&self) -> bool {
        self.weather.is_raining()
    }

    pub fn is_thundering(&self) -> bool {
        self.weather.is_thundering()
    }

    /// `Level.isRainingAt(pos)` (`precipitationAt == RAIN`; the heightmap first: it is cheaper than
    /// the sky light, and both must hold). A sky light the model cannot classify is a model error
    /// and counts as no rain.
    pub fn is_raining_at(&mut self, p: BlockPos) -> bool {
        if !self.weather.is_raining() || motion_blocking_height(self, p.x, p.z) > p.y {
            return false;
        }
        match can_see_sky(self, p) {
            Ok(v) => v,
            Err(e) => {
                let why = format!("rain at {p}: {e}");
                if !self.errors.contains(&why) {
                    self.errors.push(why);
                }
                false
            }
        }
    }
}

/// `#minecraft:blocks_motion` (26.3's data tag: `blocks_motion_no_leaves` and `leaves`, read in
/// the jar's data) among redsim's blocks: every block but air, signs, scaffolding, the plants
/// without a collision (sugar cane, kelp, bamboo shoots, stems, cactus flowers, big dripleaves),
/// skulls, tripwire and hooks, levers, buttons, rails, dust, repeaters, comparators, torches,
/// powder snow, fluids and fire. Lava's fire stops at one (`#blocks_lava_fire_spread`).
pub fn blocks_motion(b: &Block) -> bool {
    if let Block::Terrain(id) = b {
        // a natural state: MOTION_BLOCKING without its fluid (the table's flags, terrain lane)
        let p = crate::terrain::blocks::props(*id);
        return p.heightmaps[1] && p.fluid == crate::terrain::blocks::FluidKind::Empty;
    }
    !matches!(
        b,
        Block::Air
            | Block::Sign { .. }
            | Block::Scaffolding { .. }
            | Block::SugarCane { .. }
            | Block::Kelp { .. }
            | Block::KelpPlant
            | Block::BambooSapling
            | Block::Stem { .. }
            | Block::AttachedStem { .. }
            | Block::CactusFlower
            | Block::BigDripleaf { .. }
            | Block::Skull { .. }
            | Block::WallSkull { .. }
            | Block::Tripwire { .. }
            | Block::TripwireHook { .. }
            | Block::Lever { .. }
            | Block::Button { .. }
            | Block::Rail { .. }
            | Block::PlainRail { .. }
            | Block::DetectorRail { .. }
            | Block::Wire { .. }
            | Block::Repeater { .. }
            | Block::Comparator { .. }
            | Block::Torch { .. }
            | Block::WallTorch { .. }
            | Block::PowderSnow
            | Block::Water { .. }
            | Block::BubbleColumn { .. }
            | Block::Lava { .. }
            | Block::Fire { .. }
    )
}

/// `Heightmap.Types.MOTION_BLOCKING`'s predicate (26.3): `#blocks_motion_in_heightmap` (=
/// [`blocks_motion`]) or a fluid inside.
pub fn motion_blocking(b: &Block) -> bool {
    if let Block::Terrain(id) = b {
        // a natural state: the table's own MOTION_BLOCKING flag (terrain lane)
        return crate::terrain::blocks::props(*id).heightmaps[1];
    }
    blocks_motion(b) || !crate::fluid::fluid_state_of(b).is_empty() || matches!(b, Block::Lava { .. })
}

/// `Level.getHeight(MOTION_BLOCKING, x, z)`: one above the highest motion-blocking block of the
/// column; [`WORLD_MIN_Y`] when the column holds none (the judge's world is void around the farm,
/// like the game's void superflat, whose empty columns answer the dimension's bottom).
pub fn motion_blocking_height(w: &World, x: i32, z: i32) -> i32 {
    // a terrain world: the land's own heightmap (the game's, kept up to date by the terrain lane)
    if w.land.is_some() {
        return w.height(crate::terrain::Heightmap::MotionBlocking, x, z);
    }
    if x < w.min.x || x >= w.min.x + w.size[0] || z < w.min.z || z >= w.min.z + w.size[2] {
        return WORLD_MIN_Y;
    }
    let top = w.min.y + w.size[1] - 1;
    for y in (w.min.y..=top).rev() {
        if motion_blocking(&w.get(BlockPos::new(x, y, z))) {
            return y + 1;
        }
    }
    WORLD_MIN_Y
}

/// `Level.getHeight(WORLD_SURFACE, x, z)`: one above the highest block that is not air.
pub fn world_surface_height(w: &World, x: i32, z: i32) -> i32 {
    if w.land.is_some() {
        return w.height(crate::terrain::Heightmap::WorldSurface, x, z);
    }
    if x < w.min.x || x >= w.min.x + w.size[0] || z < w.min.z || z >= w.min.z + w.size[2] {
        return WORLD_MIN_Y;
    }
    let top = w.min.y + w.size[1] - 1;
    for y in (w.min.y..=top).rev() {
        if !w.get(BlockPos::new(x, y, z)).is_air() {
            return y + 1;
        }
    }
    WORLD_MIN_Y
}

/// `BlockAndLightGetter.canSeeSky`: sky light 15, which is being a sky light source
/// (`daylight::sky_source`: a column scan; outside the box, the open sky above the farm).
pub fn can_see_sky(w: &World, p: BlockPos) -> Result<bool, String> {
    // a terrain world: sky light 15 from the terrain lane's light engine
    if w.land.is_some() {
        return w.can_see_sky(p);
    }
    if !w.contains(p) {
        return Ok(p.y >= w.min.y + w.size[1] || motion_blocking_height(w, p.x, p.z) <= p.y);
    }
    crate::daylight::sky_source(w, p)
}

/// `Entity.isInRain`: rain at the entity's block or at the block of its box's top (its column).
pub fn entity_in_rain(w: &mut World, i: usize) -> bool {
    if !w.weather.is_raining() {
        return false;
    }
    let e = &w.entities.list[i];
    let p = e.block_position();
    let top = BlockPos::new(p.x, e.bb.max_y.floor() as i32, p.z);
    w.is_raining_at(p) || w.is_raining_at(top)
}

/// The tail of `Entity.applyEffectsFromBlocks` for mob `i` (`fire_before`: its fire ticks before
/// the inside-block effects): rain puts the fire out (`isInRain` -> `clearFire`), then a mob that
/// is not burning and was not set on fire this tick goes back to `-getFireImmuneTicks()` (0 in
/// 26.3 for every entity but a player, so a bolt's `thunderHit` (+1, 8 s of fire only from -1)
/// burns a mob for a tick, not 8 s). Skipped for a removed mob (`isAffectedByBlocks`).
pub fn after_block_effects(w: &mut World, i: usize, fire_before: i32) {
    let e = &w.entities.list[i];
    if e.removed || e.no_physics {
        return;
    }
    let crate::entity::EntityKind::Mob(m) = &e.kind else { return };
    if m.fire_ticks > 0 && entity_in_rain(w, i) {
        let id = w.entities.list[i].id;
        let m = crate::entity::mob::mob_mut(w, i);
        m.fire_ticks = m.fire_ticks.min(0);
        let t = w.time;
        w.weather.events.push(WeatherEvent::RainedOut { t, entity: id });
    }
    let m = crate::entity::mob::mob_mut(w, i);
    let on_fire = !m.ty.def().fire_immune && m.fire_ticks > 0;
    // `setRemainingFireTicks(-getFireImmuneTicks())`: 0 for every entity but a player in 26.3
    // (ironfarm's tick-exact compare: the game's mobs keep fire 0, not -1)
    if !on_fire && m.fire_ticks <= fire_before {
        m.fire_ticks = 0;
    }
}

#[cfg(test)]
mod tests;
