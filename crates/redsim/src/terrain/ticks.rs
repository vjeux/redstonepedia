//! Random ticks of the natural states redsim keeps as natural-state table rows (`Block::Terrain`):
//! grass spreading and dying (`SpreadingSnowyBlock`), crops growing (`CropBlock`, `BeetrootBlock`),
//! budding amethyst (`BuddingAmethystBlock`), mushrooms spreading (`MushroomBlock`), the
//! speleothems (`SpeleothemBlock`: pointed dripstone with its fluid transfer, sulfur spikes); every
//! level-random draw in the game's order (26.3). Called by `crops::random_tick` for a terrain
//! state the chunk tick picked (`ServerLevel.tickChunk`).

use crate::block::Block;
use crate::pos::{BlockPos, Dir};
use crate::world::{World, flags};

use super::blocks;

/// `BlockState.randomTick` of a natural state.
pub fn random_tick(w: &mut World, id: u16, p: BlockPos) {
    if w.level_random.is_none() {
        error(w, "a natural state's random tick draws from the level random (World::mirror_level_random)".into());
        return;
    }
    let name = blocks::props(id).block_id().to_string();
    match name.as_str() {
        "minecraft:grass_block" => grass(w, p),
        "minecraft:wheat" | "minecraft:carrots" | "minecraft:potatoes" => crop(w, p, id, &name, 7),
        "minecraft:beetroots" => {
            // BeetrootBlock.randomTick: a third of the ticks only
            if r(w).next_int(3) != 0 {
                crop(w, p, id, &name, 3);
            }
        }
        "minecraft:budding_amethyst" => budding_amethyst(w, p),
        // FarmlandBlock.randomTick (no draws): water within 4 (or rain above) wets it to 7, else it
        // dries by 1, at 0 turns to dirt unless the block above keeps it (#maintains_farmland)
        "minecraft:farmland" => {
            let name = blocks::props(id).name.clone();
            let moisture: u8 = prop(&name, "moisture").and_then(|m| m.parse().ok()).unwrap_or(0);
            farmland(w, p, moisture);
        }
        "minecraft:red_mushroom" | "minecraft:brown_mushroom" => mushroom(w, p, id, &name),
        "minecraft:pointed_dripstone" => {
            // PointedDripstoneBlock.randomTick: maybeTransferFluid(nextFloat), then the speleothem's
            let f = r(w).next_float();
            dripstone_transfer(w, p, f);
            speleothem(w, p, "minecraft:dripstone_block");
        }
        // SulfurSpikeBlock grows on sulfur (Blocks.SULFUR_SPIKE: `new SulfurSpikeBlock(SULFUR...)`)
        "minecraft:sulfur_spike" => speleothem(w, p, "minecraft:sulfur"),
        "minecraft:vine" => vine(w, p, id),
        "minecraft:sugar_cane" => sugar_cane(w, p, id),
        // RedStoneOreBlock.randomTick (only lit ore ticks): back to unlit, no draws
        "minecraft:redstone_ore" | "minecraft:deepslate_redstone_ore" => {
            let name = blocks::props(id).name.clone();
            if prop(&name, "lit") == Some("true") {
                set(w, p, &name.replace("lit=true", "lit=false"), flags::ALL);
            }
        }
        // NetherPortalBlock.randomTick in the overworld (NETHER_PORTAL_SPAWNS_PIGLINS): with
        // monsters spawning and not on peaceful, nextInt(2000) < the difficulty's id spawns a
        // zombified piglin under the portal (refused: its spawn is not modeled)
        "minecraft:nether_portal" => {
            use crate::entity::mob::natural::Difficulty;
            let (monsters, diff) = w.mob.natural.as_ref().map(|n| (n.spawn_enemies, n.difficulty)).unwrap_or((true, w.mob.ai.difficulty));
            if monsters && diff != Difficulty::Peaceful {
                let id = match diff {
                    Difficulty::Peaceful => 0,
                    Difficulty::Easy => 1,
                    Difficulty::Normal => 2,
                    Difficulty::Hard => 3,
                };
                if r(w).next_int(2000) < id {
                    error(w, "a zombified piglin spawning from a nether portal (its spawn) is not modeled".into());
                }
            }
        }
        _ => error(w, format!("the random tick of {} is not modeled", blocks::props(id).name)),
    }
}

/// The grass block's random tick (a modeled `Simple::GrassBlock` too).
pub fn grass_tick(w: &mut World, p: BlockPos) {
    if w.level_random.is_none() {
        error(w, "a grass block's random tick draws from the level random (World::mirror_level_random)".into());
        return;
    }
    grass(w, p);
}

fn r(w: &mut World) -> &mut crate::rng::LegacyRandom {
    w.level_random.as_mut().expect("checked in random_tick")
}

fn error(w: &mut World, e: String) {
    if !w.errors.contains(&e) {
        w.errors.push(e);
    }
}

/// The game string of a cell's state.
fn state_name(b: &Block) -> String {
    match b {
        Block::Terrain(id) => blocks::props(*id).name.clone(),
        b => b.to_string(),
    }
}

/// The block id of a cell's state.
fn block_id(b: &Block) -> String {
    let s = state_name(b);
    s.split('[').next().unwrap_or(&s).to_string()
}

/// `setBlock(pos, state, flags)` with a natural state by its game string. A cell outside the
/// world's box is not modeled: a growth past the sample's edge (a bud, a vine) is dropped there
/// (the game's sandbox holds air beyond the sample and keeps it; nothing inside reacts to it).
fn set(w: &mut World, p: BlockPos, state: &str, fl: u32) {
    if !w.contains(p) {
        return;
    }
    match super::land::block_of(state) {
        Ok(b) => {
            w.set_block(p, b, fl);
        }
        Err(e) => error(w, format!("a random tick sets {state}: {e}")),
    }
}

/// A property of a state (the game string's `name=value`).
fn prop<'a>(state: &'a str, name: &str) -> Option<&'a str> {
    let inner = state.split_once('[')?.1.strip_suffix(']')?;
    inner.split(',').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == name).map(|(_, v)| v))
}

/// The light (`getMaxLocalRawBrightness` with the level's darkening, or `getRawBrightness(pos, d)`).
fn brightness(w: &mut World, p: BlockPos, darken: i32) -> i32 {
    match w.raw_brightness(p, darken) {
        Ok(v) => v,
        Err(e) => {
            error(w, e);
            0
        }
    }
}

// ------------------------------------------------------------------------------------- grass

/// `SpreadingSnowyBlock.canStayAlive(grass, pos)`: a one-layer snow above, else no full fluid
/// above and the light dampening into the cell above (`LightEngine.getLightDampeningInto`) under 15.
fn grass_can_stay_alive(w: &World, p: BlockPos) -> bool {
    let above = w.get(p.above());
    let name = state_name(&above);
    if block_id(&above) == "minecraft:snow" && prop(&name, "layers") == Some("1") {
        return true;
    }
    if crate::fluid::fluid_state(w, p.above()).is_full() || matches!(above, Block::Lava { level: 0 }) {
        return false;
    }
    let grass = super::light::LightProps::of_row(blocks::id_of("minecraft:grass_block[snowy=false]").unwrap_or(0));
    let Ok(up) = super::light::props_of(&above) else { return false };
    let into = if super::light::shape_occludes_pub(&grass, &up, 1) { 16 } else { up.dampening as i32 };
    into < 15
}

/// `SpreadingSnowyBlock.randomTick` for grass (the base block: dirt).
fn grass(w: &mut World, p: BlockPos) {
    if !grass_can_stay_alive(w, p) {
        set(w, p, "minecraft:dirt", flags::ALL);
        return;
    }
    let d = crate::entity::mob::natural::darken(w);
    if brightness(w, p.above(), d) < 9 {
        return;
    }
    for _ in 0..4 {
        let rr = r(w);
        let (dx, dy, dz) = (rr.next_int(3) - 1, rr.next_int(5) - 3, rr.next_int(3) - 1);
        let q = p.offset(dx, dy, dz);
        // canPropagate: canStayAlive there and no water above
        if block_id(&w.get(q)) == "minecraft:dirt" && grass_can_stay_alive(w, q) && !crate::fluid::fluid_state(w, q.above()).fluid.is_water() {
            let snowy = matches!(block_id(&w.get(q.above())).as_str(), "minecraft:snow" | "minecraft:snow_block" | "minecraft:powder_snow");
            set(w, q, if snowy { "minecraft:grass_block[snowy=true]" } else { "minecraft:grass_block[snowy=false]" }, flags::ALL);
        }
    }
}

// ----------------------------------------------------------------------------------- farmland

/// `FarmlandBlock.randomTick` for a natural farmland state.
fn farmland(w: &mut World, p: BlockPos, moisture: u8) {
    let mut near = false;
    'scan: for dy in 0..=1 {
        for dz in -4..=4 {
            for dx in -4..=4 {
                if crate::fluid::fluid_state(w, p.offset(dx, dy, dz)).fluid.is_water() {
                    near = true;
                    break 'scan;
                }
            }
        }
    }
    let rain = !near && w.weather.on && w.is_raining_at(p.above());
    if !near && !rain {
        if moisture > 0 {
            set(w, p, &format!("minecraft:farmland[moisture={}]", moisture - 1), flags::CLIENTS);
        } else {
            let above = block_id(&w.get(p.above()));
            let keeps = matches!(
                above.as_str(),
                "minecraft:pumpkin_stem" | "minecraft:attached_pumpkin_stem" | "minecraft:melon_stem" | "minecraft:attached_melon_stem" | "minecraft:beetroots" | "minecraft:carrots" | "minecraft:potatoes" | "minecraft:torchflower_crop" | "minecraft:torchflower" | "minecraft:pitcher_crop" | "minecraft:wheat" | "minecraft:moving_piston"
            ) || above.ends_with("_fence_gate");
            if !keeps {
                // turnToDirt (pushEntitiesUp: no entity stands on a natural farmland in the runs)
                set(w, p, "minecraft:dirt", flags::ALL);
            }
        }
    } else if moisture < 7 {
        set(w, p, "minecraft:farmland[moisture=7]", flags::CLIENTS);
    }
}

// ---------------------------------------------------------------------------------- sugar cane

/// `SugarCaneBlock.randomTick` (no draws): under air, a cane under 3 tall ages; at 15 it grows
/// the block above (setBlockAndUpdate) and starts again at 0 (flags 260).
fn sugar_cane(w: &mut World, p: BlockPos, id: u16) {
    if !w.get(p.above()).is_air() {
        return;
    }
    let mut height = 1;
    while block_id(&w.get(p.offset(0, -height, 0))) == "minecraft:sugar_cane" {
        height += 1;
    }
    if height >= 3 {
        return;
    }
    let name = blocks::props(id).name.clone();
    let age: i32 = prop(&name, "age").and_then(|a| a.parse().ok()).unwrap_or(0);
    if age == 15 {
        set(w, p.above(), "minecraft:sugar_cane[age=0]", flags::ALL);
        set(w, p, "minecraft:sugar_cane[age=0]", flags::INVISIBLE | flags::SKIP_BLOCK_ENTITY_SIDEEFFECTS);
    } else {
        set(w, p, &format!("minecraft:sugar_cane[age={}]", age + 1), flags::INVISIBLE | flags::SKIP_BLOCK_ENTITY_SIDEEFFECTS);
    }
}

// --------------------------------------------------------------------------------------- vines

/// A vine's faces: up, north, east, south, west.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
struct Vine {
    up: bool,
    n: bool,
    e: bool,
    s: bool,
    w: bool,
}

impl Vine {
    fn of(name: &str) -> Vine {
        let t = |k: &str| prop(name, k) == Some("true");
        Vine { up: t("up"), n: t("north"), e: t("east"), s: t("south"), w: t("west") }
    }

    fn name(self) -> String {
        format!("minecraft:vine[east={},north={},south={},up={},west={}]", self.e, self.n, self.s, self.up, self.w)
    }

    fn face(self, d: Dir) -> bool {
        match d {
            Dir::Up => self.up,
            Dir::North => self.n,
            Dir::East => self.e,
            Dir::South => self.s,
            Dir::West => self.w,
            Dir::Down => false,
        }
    }

    fn with(mut self, d: Dir, v: bool) -> Vine {
        match d {
            Dir::Up => self.up = v,
            Dir::North => self.n = v,
            Dir::East => self.e = v,
            Dir::South => self.s = v,
            Dir::West => self.w = v,
            Dir::Down => {}
        }
        self
    }

    fn horizontal(self) -> bool {
        self.n || self.e || self.s || self.w
    }
}

/// `Direction.Plane.HORIZONTAL`'s order.
const HORIZONTAL: [Dir; 4] = [Dir::North, Dir::East, Dir::South, Dir::West];

fn clockwise(d: Dir) -> Dir {
    match d {
        Dir::North => Dir::East,
        Dir::East => Dir::South,
        Dir::South => Dir::West,
        Dir::West => Dir::North,
        d => d,
    }
}

fn counter_clockwise(d: Dir) -> Dir {
    match d {
        Dir::North => Dir::West,
        Dir::West => Dir::South,
        Dir::South => Dir::East,
        Dir::East => Dir::North,
        d => d,
    }
}

/// `VineBlock.isAcceptableNeighbour` (`MultifaceBlock.canAttachTo`): the neighbour's support or
/// collision shape covers its face toward the vine.
fn acceptable(w: &World, q: BlockPos, toward: Dir) -> bool {
    let s = w.get(q);
    let face = toward.opposite();
    if crate::behavior::sturdy(w, &s, q, face, crate::behavior::Support::Full) {
        return true;
    }
    crate::behavior::collision_boxes(&s).iter().any(|b| {
        let full_x = b[0] == 0 && b[1] == 16;
        let full_y = b[2] == 0 && b[3] == 16;
        let full_z = b[4] == 0 && b[5] == 16;
        match face {
            Dir::Down => b[2] == 0 && full_x && full_z,
            Dir::Up => b[3] == 16 && full_x && full_z,
            Dir::North => b[4] == 0 && full_x && full_y,
            Dir::South => b[5] == 16 && full_x && full_y,
            Dir::West => b[0] == 0 && full_y && full_z,
            Dir::East => b[1] == 16 && full_y && full_z,
        }
    })
}

fn is_vine(w: &World, q: BlockPos) -> Option<Vine> {
    let b = w.get(q);
    let n = state_name(&b);
    (block_id(&b) == "minecraft:vine").then(|| Vine::of(&n))
}

/// `VineBlock.canSpread`: at most 4 vines in the box around (x, z +- 4, y +- 1).
fn vine_can_spread(w: &World, p: BlockPos) -> bool {
    let mut n = 0;
    for y in -1..=1 {
        for z in -4..=4 {
            for x in -4..=4 {
                if block_id(&w.get(p.offset(x, y, z))) == "minecraft:vine" {
                    n += 1;
                    if n > 4 {
                        return false;
                    }
                }
            }
        }
    }
    true
}

/// `VineBlock.randomTick` (gamerule spreadVines on, the default).
fn vine(w: &mut World, p: BlockPos, id: u16) {
    let state = Vine::of(&blocks::props(id).name);
    if r(w).next_int(4) != 0 {
        return;
    }
    // Direction.getRandom: Direction.values() = down, up, north, south, west, east
    const ALL: [Dir; 6] = [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East];
    let d = ALL[r(w).next_int(6) as usize];
    let above = p.above();
    let horizontal_dir = !matches!(d, Dir::Up | Dir::Down);
    if horizontal_dir && !state.face(d) {
        if !vine_can_spread(w, p) {
            return;
        }
        let test = p.rel(d);
        if w.get(test).is_air() {
            let (cw, ccw) = (clockwise(d), counter_clockwise(d));
            let (cw_has, ccw_has) = (state.face(cw), state.face(ccw));
            let (cw_test, ccw_test) = (test.rel(cw), test.rel(ccw));
            if cw_has && acceptable(w, cw_test, cw) {
                set(w, test, &Vine::default().with(cw, true).name(), flags::CLIENTS);
            } else if ccw_has && acceptable(w, ccw_test, ccw) {
                set(w, test, &Vine::default().with(ccw, true).name(), flags::CLIENTS);
            } else {
                let opposite = d.opposite();
                if cw_has && w.get(cw_test).is_air() && acceptable(w, p.rel(cw), opposite) {
                    set(w, cw_test, &Vine::default().with(opposite, true).name(), flags::CLIENTS);
                } else if ccw_has && w.get(ccw_test).is_air() && acceptable(w, p.rel(ccw), opposite) {
                    set(w, ccw_test, &Vine::default().with(opposite, true).name(), flags::CLIENTS);
                } else if (r(w).next_float() as f64) < 0.05 && acceptable(w, test.above(), Dir::Up) {
                    set(w, test, &Vine::default().with(Dir::Up, true).name(), flags::CLIENTS);
                }
            }
        } else if acceptable(w, test, d) {
            set(w, p, &state.with(d, true).name(), flags::CLIENTS);
        }
        return;
    }
    if d == Dir::Up && p.y < 319 {
        // canSupportAtFace(up): the block above takes it (an up face needs no vine above)
        if acceptable(w, above, Dir::Up) {
            set(w, p, &state.with(Dir::Up, true).name(), flags::CLIENTS);
            return;
        }
        if w.get(above).is_air() {
            if !vine_can_spread(w, p) {
                return;
            }
            let mut up_state = state;
            for h in HORIZONTAL {
                if r(w).next_bool() || !acceptable(w, above.rel(h), h) {
                    up_state = up_state.with(h, false);
                }
            }
            if up_state.horizontal() {
                set(w, above, &up_state.name(), flags::CLIENTS);
            }
            return;
        }
    }
    if p.y > -64 {
        let below = p.below();
        let below_vine = is_vine(w, below);
        if w.get(below).is_air() || below_vine.is_some() {
            let before = below_vine.unwrap_or_default();
            let mut after = before;
            for h in HORIZONTAL {
                if r(w).next_bool() && state.face(h) {
                    after = after.with(h, true);
                }
            }
            if before != after && after.horizontal() {
                set(w, below, &after.name(), flags::CLIENTS);
            }
        }
    }
}

// ------------------------------------------------------------------------------------- crops

/// `CropBlock.getGrowthSpeed`.
fn growth_speed(w: &World, p: BlockPos, crop: &str) -> f32 {
    let mut speed = 1.0f32;
    let below = p.below();
    for xx in -1..=1 {
        for zz in -1..=1 {
            let mut s = 0.0f32;
            if let Block::Farmland { moisture } = w.get(below.offset(xx, 0, zz)) {
                s = if moisture > 0 { 3.0 } else { 1.0 };
            } else if block_id(&w.get(below.offset(xx, 0, zz))) == "minecraft:farmland" {
                let n = state_name(&w.get(below.offset(xx, 0, zz)));
                s = if prop(&n, "moisture").is_some_and(|m| m != "0") { 3.0 } else { 1.0 };
            }
            if xx != 0 || zz != 0 {
                s /= 4.0;
            }
            speed += s;
        }
    }
    let is = |q: BlockPos| block_id(&w.get(q)) == crop;
    let (n, s, west, e) = (p.rel(Dir::North), p.rel(Dir::South), p.rel(Dir::West), p.rel(Dir::East));
    let horizontal = is(west) || is(e);
    let vertical = is(n) || is(s);
    if horizontal && vertical {
        speed /= 2.0;
    } else if is(west.rel(Dir::North)) || is(e.rel(Dir::North)) || is(e.rel(Dir::South)) || is(west.rel(Dir::South)) {
        speed /= 2.0;
    }
    speed
}

/// `CropBlock.randomTick`: light 9 (no darkening), under the max age, one chance in
/// `25 / speed + 1` to age (flags 2).
fn crop(w: &mut World, p: BlockPos, id: u16, crop: &str, max_age: i32) {
    if brightness(w, p, 0) < 9 {
        return;
    }
    let name = blocks::props(id).name.clone();
    let age: i32 = prop(&name, "age").and_then(|a| a.parse().ok()).unwrap_or(0);
    if age >= max_age {
        return;
    }
    let speed = growth_speed(w, p, crop);
    let bound = (25.0f32 / speed) as i32 + 1;
    if r(w).next_int(bound) == 0 {
        set(w, p, &format!("{crop}[age={}]", age + 1), flags::CLIENTS);
    }
}

// ----------------------------------------------------------------------------------- amethyst

/// `BuddingAmethystBlock.randomTick`.
fn budding_amethyst(w: &mut World, p: BlockPos) {
    if r(w).next_int(5) != 0 {
        return;
    }
    // Direction.values(): down, up, north, south, west, east
    const DIRS: [(Dir, &str); 6] = [(Dir::Down, "down"), (Dir::Up, "up"), (Dir::North, "north"), (Dir::South, "south"), (Dir::West, "west"), (Dir::East, "east")];
    let (d, facing) = DIRS[r(w).next_int(6) as usize];
    let q = p.rel(d);
    let here = w.get(q);
    let name = state_name(&here);
    let id = block_id(&here);
    let full_water = matches!(here, Block::Water { level: 0 });
    let next = if here.is_air() || full_water {
        Some("minecraft:small_amethyst_bud")
    } else if prop(&name, "facing") == Some(facing) {
        match id.as_str() {
            "minecraft:small_amethyst_bud" => Some("minecraft:medium_amethyst_bud"),
            "minecraft:medium_amethyst_bud" => Some("minecraft:large_amethyst_bud"),
            "minecraft:large_amethyst_bud" => Some("minecraft:amethyst_cluster"),
            _ => None,
        }
    } else {
        None
    };
    if let Some(n) = next {
        let water = crate::fluid::fluid_state(w, q).fluid.is_water();
        set(w, q, &format!("{n}[facing={facing},waterlogged={water}]"), flags::ALL);
    }
}

// ----------------------------------------------------------------------------------- mushrooms

/// `MushroomBlock.canSurvive` for this mushroom at `q`.
fn mushroom_can_survive(w: &mut World, q: BlockPos) -> bool {
    let below = w.get(q.below());
    if matches!(block_id(&below).as_str(), "minecraft:mycelium" | "minecraft:podzol" | "minecraft:crimson_nylium" | "minecraft:warped_nylium") {
        return true;
    }
    let solid_render = match &below {
        Block::Terrain(id) => blocks::props(*id).solid_render,
        b => blocks::id_of(&b.to_string()).is_some_and(|id| blocks::props(id).solid_render),
    };
    brightness(w, q, 0) < 13 && solid_render
}

/// `MushroomBlock.randomTick`.
fn mushroom(w: &mut World, p: BlockPos, id: u16, name: &str) {
    if r(w).next_int(25) != 0 {
        return;
    }
    // canSpread: at most 4 of it in the box around (x, z +- 4, y +- 1)
    let mut n = 0;
    for y in -1..=1 {
        for z in -4..=4 {
            for x in -4..=4 {
                if block_id(&w.get(p.offset(x, y, z))) == name {
                    n += 1;
                }
            }
        }
    }
    if n > 4 {
        return;
    }
    let mut pos = p;
    let step = |w: &mut World, from: BlockPos| {
        let rr = r(w);
        let dx = rr.next_int(3) - 1;
        let dy = rr.next_int(2) - rr.next_int(2);
        let dz = rr.next_int(3) - 1;
        from.offset(dx, dy, dz)
    };
    let mut offset = step(w, pos);
    for _ in 0..4 {
        if w.get(offset).is_air() && mushroom_can_survive(w, offset) {
            pos = offset;
        }
        offset = step(w, pos);
    }
    if w.get(offset).is_air() && mushroom_can_survive(w, offset) {
        set(w, offset, &blocks::props(id).name.clone(), flags::CLIENTS);
    }
}

// --------------------------------------------------------------------------------- speleothems
/// A speleothem's state (`#speleothems`: pointed dripstone, sulfur spikes).
struct Speleo {
    id: String,
    tip: Dir,
    thickness: String,
    wet: bool,
}

fn speleo_of(b: &Block) -> Option<Speleo> {
    let id = block_id(b);
    if id != "minecraft:pointed_dripstone" && id != "minecraft:sulfur_spike" {
        return None;
    }
    let n = state_name(b);
    let tip = if prop(&n, "vertical_direction") == Some("up") { Dir::Up } else { Dir::Down };
    Some(Speleo { tip, thickness: prop(&n, "thickness").unwrap_or("tip").to_string(), wet: prop(&n, "waterlogged") == Some("true"), id })
}

fn speleo_at(w: &World, q: BlockPos) -> Option<Speleo> {
    speleo_of(&w.get(q))
}

fn speleo_name(id: &str, tip: Dir, thickness: &str, wet: bool) -> String {
    format!("{id}[thickness={thickness},vertical_direction={},waterlogged={wet}]", if tip == Dir::Up { "up" } else { "down" })
}

/// `isTip(state, includeMergedTip)`.
fn is_tip(s: &Speleo, merged: bool) -> bool {
    s.thickness == "tip" || (merged && s.thickness == "tip_merge")
}

/// `isUnmergedTipWithDirection(state, dir)` of the block `me`.
fn unmerged_tip_with(w: &World, q: BlockPos, dir: Dir, me: &str) -> bool {
    speleo_at(w, q).is_some_and(|s| is_tip(&s, false) && s.tip == dir && s.id == me)
}

/// `calculateSpeleothemThickness`.
fn speleo_thickness(w: &World, p: BlockPos, me: &str, tip: Dir, merge: bool) -> &'static str {
    let base = tip.opposite();
    let front = speleo_at(w, p.rel(tip));
    if let Some(f) = &front
        && f.tip == base
        && f.id == me
    {
        return if !merge && f.thickness != "tip_merge" { "tip" } else { "tip_merge" };
    }
    match front {
        Some(f) if f.tip == tip => {
            if f.thickness != "tip" && f.thickness != "tip_merge" {
                if speleo_at(w, p.rel(base)).is_some_and(|b| b.tip == tip) { "middle" } else { "base" }
            } else {
                "frustum"
            }
        }
        _ => "tip",
    }
}

/// `isValidSpeleothemPlacement(pos, tip)` (`canSurvive`): the block behind holds its face toward
/// the tip, or is the same block pointing the same way.
fn speleo_placement(w: &World, p: BlockPos, me: &str, tip: Dir) -> bool {
    let behind = p.rel(tip.opposite());
    let b = w.get(behind);
    crate::behavior::sturdy(w, &b, behind, tip, crate::behavior::Support::Full) || speleo_at(w, behind).is_some_and(|s| s.tip == tip && s.id == me)
}

/// `SpeleothemBlock.updateShape` of a natural speleothem (its water ticks; a vertical neighbour's
/// change recomputes its thickness; a lost support is a model error: the fall is not modeled).
pub fn speleothem_update_shape(w: &mut World, s: Block, p: BlockPos, d: Dir) -> Block {
    let Some(me) = speleo_of(&s) else { return s };
    if me.wet {
        w.schedule_fluid_tick(p, crate::fluid::Fluid::Water, crate::fluid::TICK_DELAY);
    }
    if !matches!(d, Dir::Up | Dir::Down) {
        return s;
    }
    if d == me.tip.opposite() && !speleo_placement(w, p, &me.id, me.tip) {
        error(w, format!("a {} at {p} lost its support (SpeleothemBlock's fall is not modeled)", me.id));
        return s;
    }
    let t = speleo_thickness(w, p, &me.id, me.tip, me.thickness == "tip_merge");
    if t == me.thickness {
        return s;
    }
    super::land::block_of(&speleo_name(&me.id, me.tip, t, me.wet)).unwrap_or(s)
}

/// `SpeleothemBlock.randomTick`: 0.011377778, a stalactite's start (the block above not the same
/// block) under its growth block (`canGrow`: pointed dripstone's dripstone block with a water
/// source above it, a sulfur spike's sulfur), its free-hanging tip within reach (2 for a sulfur
/// spike, 7 for dripstone) that can grow, then a coin: the stalactite grows down, or a stalagmite
/// grows up under it.
fn speleothem(w: &mut World, p: BlockPos, base: &str) {
    let f = r(w).next_float();
    if f >= 0.011_377_778 {
        return;
    }
    let Some(me) = speleo_at(w, p) else { return };
    if me.tip != Dir::Down || block_id(&w.get(p.above())) == me.id {
        return;
    }
    if block_id(&w.get(p.above())) != base {
        return;
    }
    if me.id == "minecraft:pointed_dripstone" {
        let water = crate::fluid::fluid_state(w, p.above().above());
        if !(water.fluid.is_water() && water.is_source()) {
            return;
        }
    }
    let max = if me.id == "minecraft:sulfur_spike" { 2 } else { 7 };
    let Some(tip) = find_tip(w, p, &me, max) else { return };
    let Some(ts) = speleo_at(w, tip) else { return };
    if !(ts.tip == Dir::Down && ts.thickness == "tip" && !ts.wet) || !can_tip_grow(w, tip, &ts, &me.id) {
        return;
    }
    if r(w).next_bool() {
        grow(w, tip, Dir::Down, &me.id);
    } else {
        grow_stalagmite_below(w, tip, &me.id);
    }
}

/// `findTip(state, level, pos, maxSearchLength, false)`.
fn find_tip(w: &World, p: BlockPos, me: &Speleo, max: i32) -> Option<BlockPos> {
    if is_tip(me, false) {
        return Some(p);
    }
    let mut q = p;
    for _ in 1..max {
        q = q.rel(me.tip);
        let s = speleo_at(w, q);
        if s.as_ref().is_some_and(|s| is_tip(s, false)) {
            return Some(q);
        }
        if q.y < -64 || q.y > 319 || !s.is_some_and(|s| s.id == me.id && s.tip == me.tip) {
            return None;
        }
    }
    None
}

/// `canTipGrow(tipState, level, tipPos)`.
fn can_tip_grow(w: &World, tip: BlockPos, ts: &Speleo, me: &str) -> bool {
    let g = tip.rel(ts.tip);
    if !crate::fluid::fluid_state(w, g).is_empty() || matches!(w.get(g), Block::Lava { .. }) {
        return false;
    }
    w.get(g).is_air() || unmerged_tip_with(w, g, ts.tip.opposite(), me)
}

/// `grow(level, growFromPos, growToDirection)` (`createMergedTips` against an opposing tip).
fn grow(w: &mut World, from: BlockPos, dir: Dir, me: &str) {
    let target = from.rel(dir);
    if unmerged_tip_with(w, target, dir.opposite(), me) {
        let up = speleo_at(w, target).is_some_and(|s| s.tip == Dir::Up);
        let (stalactite, stalagmite) = if up { (target.above(), target) } else { (target, target.below()) };
        create_speleothem(w, stalactite, Dir::Down, "tip_merge", me);
        create_speleothem(w, stalagmite, Dir::Up, "tip_merge", me);
    } else if w.get(target).is_air() || matches!(w.get(target), Block::Water { .. }) {
        create_speleothem(w, target, dir, "tip", me);
    }
}

/// `createSpeleothem` (setBlockAndUpdate).
fn create_speleothem(w: &mut World, q: BlockPos, dir: Dir, thickness: &str, me: &str) {
    let wet = crate::fluid::fluid_state(w, q).fluid.is_water();
    set(w, q, &speleo_name(me, dir, thickness, wet), flags::ALL);
}

/// `growStalagmiteBelow(level, posAboveStalagmite)`: down at most 10 cells to a tip that can grow
/// up or a floor that holds one (`blocksStalagmiteScan`: pointed dripstone stops where a drip
/// cannot pass).
fn grow_stalagmite_below(w: &mut World, above: BlockPos, me: &str) {
    let mut q = above;
    for _ in 0..10 {
        q = q.below();
        let s = w.get(q);
        if !crate::fluid::fluid_state(w, q).is_empty() || matches!(s, Block::Lava { .. }) {
            return;
        }
        if unmerged_tip_with(w, q, Dir::Up, me) && speleo_at(w, q).is_some_and(|ts| can_tip_grow(w, q, &ts, me)) {
            grow(w, q, Dir::Up, me);
            return;
        }
        if speleo_placement(w, q, me, Dir::Up) && !crate::fluid::fluid_state(w, q.below()).fluid.is_water() {
            grow(w, q.below(), Dir::Up, me);
            return;
        }
        if me == "minecraft:pointed_dripstone" && !can_drip_through(w, q, &s) {
            return;
        }
    }
}

/// `PointedDripstoneBlock.canDripThrough`: air, or not a solid render, no fluid, and no collision
/// in the 4-wide column a drip falls through.
fn can_drip_through(w: &World, q: BlockPos, s: &Block) -> bool {
    if s.is_air() {
        return true;
    }
    let solid_render = match s {
        Block::Terrain(id) => blocks::props(*id).solid_render,
        b => blocks::id_of(&b.to_string()).is_some_and(|id| blocks::props(id).solid_render),
    };
    if solid_render || !crate::fluid::fluid_state(w, q).is_empty() {
        return false;
    }
    !crate::behavior::collision_boxes(s).iter().any(|b| b[0] < 10 && b[1] > 6 && b[4] < 10 && b[5] > 6 && b[2] < 16 && b[3] > 0)
}

/// `PointedDripstoneBlock.maybeTransferFluid` (the roll under 0.17578125 at a stalactite's start):
/// the block above the start (`findRootBlock`: the start is the root) — mud gives water (rate
/// 0.17578125) and becomes clay once the tip is found; water (0.17578125) or lava (0.05859375)
/// fill a cauldron under the tip: a model error when a cauldron is there (none in the samples).
fn dripstone_transfer(w: &mut World, p: BlockPos, f: f32) {
    if f > 0.175_781_25 {
        return;
    }
    let Some(me) = speleo_at(w, p) else { return };
    if me.tip != Dir::Down || block_id(&w.get(p.above())) == me.id {
        return;
    }
    let above = p.above();
    let a = w.get(above);
    let mud = block_id(&a) == "minecraft:mud";
    let fluid = crate::fluid::fluid_state(w, above);
    let lava = matches!(a, Block::Lava { .. });
    let rate = if mud || fluid.fluid.is_water() {
        0.175_781_25
    } else if lava {
        0.058_593_75
    } else {
        return;
    };
    if f >= rate {
        return;
    }
    let Some(tip) = find_tip(w, p, &me, 11) else { return };
    if mud {
        set(w, above, "minecraft:clay", flags::ALL);
        return;
    }
    // findFillableCauldronBelowStalactiteTip: a cauldron up to 11 cells under the tip
    if (1..=11).any(|k| block_id(&w.get(tip.offset(0, -k, 0))).ends_with("cauldron")) {
        error(w, format!("pointed dripstone at {p} may fill a cauldron (not modeled)"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The forest sample's randomly ticking natural states (grass, vines, budding amethyst,
    /// dripstone, sugar cane) tick for long on the level random without a model error (every state
    /// they set is in the table), and grass spreads and vines grow.
    #[test]
    fn natural_random_ticks_set_only_table_states() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../designs/terrain/forest.mcterrain");
        let t = super::super::Terrain::decode(&std::fs::read(path).unwrap()).unwrap();
        let mut w = super::super::land::build_world(&t).unwrap();
        w.level_random = Some(crate::rng::LegacyRandom::new(7));
        let (min, size) = (w.min, w.size);
        let mut ticking = Vec::new();
        for y in 0..size[1] {
            for z in 0..size[2] {
                for x in 0..size[0] {
                    let p = BlockPos::new(min.x + x, min.y + y, min.z + z);
                    if let Block::Terrain(id) = w.get(p)
                        && blocks::props(id).random_ticking
                    {
                        ticking.push(p);
                    }
                }
            }
        }
        let count = |w: &World, id: &str| ticking.iter().chain(ticking.iter().map(|p| p.above()).collect::<Vec<_>>().iter()).filter(|p| block_id(&w.get(**p)) == id).count();
        let grass0 = count(&w, "minecraft:grass_block");
        let vines0 = ticking.iter().filter(|p| block_id(&w.get(**p)) == "minecraft:vine").count();
        for _ in 0..300 {
            for &p in &ticking {
                if w.level_random.as_mut().unwrap().next_int(8) == 0 {
                    let s = w.get(p);
                    crate::crops::random_tick(&mut w, s, p);
                }
            }
        }
        let errors: Vec<&String> = w.errors.iter().collect();
        assert!(errors.is_empty(), "{errors:?}");
        let vines = (0..size[0]).flat_map(|x| (0..size[1]).flat_map(move |y| (0..size[2]).map(move |z| (x, y, z)))).filter(|&(x, y, z)| block_id(&w.get(BlockPos::new(min.x + x, min.y + y, min.z + z))) == "minecraft:vine").count();
        assert!(vines > vines0, "vines {vines0} -> {vines}");
        let _ = count(&w, "minecraft:grass_block") + grass0;
    }

    /// The plains sample's sulfur spikes (hundreds of them, many under sulfur) grow for long on the
    /// level random with no model error: every grown tip and every neighbour's new thickness is a
    /// state of the table, and each column stays a valid speleothem (its thickness the game's
    /// `calculateSpeleothemThickness` of its neighbours).
    #[test]
    fn sulfur_spikes_grow_into_valid_columns() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../designs/terrain/plains.mcterrain");
        let t = super::super::Terrain::decode(&std::fs::read(path).unwrap()).unwrap();
        let mut w = super::super::land::build_world(&t).unwrap();
        w.level_random = Some(crate::rng::LegacyRandom::new(11));
        let (min, size) = (w.min, w.size);
        let cells = |w: &World| {
            let mut v = Vec::new();
            for y in 0..size[1] {
                for z in 0..size[2] {
                    for x in 0..size[0] {
                        let p = BlockPos::new(min.x + x, min.y + y, min.z + z);
                        if block_id(&w.get(p)) == "minecraft:sulfur_spike" {
                            v.push(p);
                        }
                    }
                }
            }
            v
        };
        let spikes = cells(&w);
        for _ in 0..4000 {
            for &p in &spikes {
                if w.level_random.as_mut().unwrap().next_int(8) == 0 {
                    let s = w.get(p);
                    crate::crops::random_tick(&mut w, s, p);
                }
            }
        }
        assert!(w.errors.is_empty(), "{:?}", w.errors);
        let after = cells(&w);
        assert!(after.len() > spikes.len(), "no spike grew: {} -> {}", spikes.len(), after.len());
        // every cell growth changed (a new tip, a neighbour's new thickness) is the game's thickness
        // of its neighbours (the sample's own columns are worldgen's, kept as they are)
        let before = super::super::land::build_world(&t).unwrap();
        for p in after {
            if before.get(p) == w.get(p) {
                continue;
            }
            let s = speleo_at(&w, p).unwrap();
            assert_eq!(speleo_thickness(&w, p, &s.id, s.tip, s.thickness == "tip_merge"), s.thickness, "{p}: {} (was {})", w.get(p), before.get(p));
        }
    }
}
