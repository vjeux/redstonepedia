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
        "minecraft:red_mushroom" | "minecraft:brown_mushroom" => mushroom(w, p, id, &name),
        "minecraft:pointed_dripstone" => {
            // PointedDripstoneBlock.randomTick: maybeTransferFluid(nextFloat), then the speleothem's
            let f = r(w).next_float();
            dripstone_transfer(w, p, id, f);
            speleothem(w, p, id, "minecraft:dripstone_block");
        }
        "minecraft:sulfur_spike" => speleothem(w, p, id, "minecraft:potent_sulfur"),
        _ => error(w, format!("the random tick of {} is not modeled", blocks::props(id).name)),
    }
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

/// `setBlock(pos, state, flags)` with a natural state by its game string.
fn set(w: &mut World, p: BlockPos, state: &str, fl: u32) {
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
    let d = w.sky_darken();
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

/// `SpeleothemBlock.randomTick`: 0.011377778 and a stalactite's start (the block above not the
/// same block) under its growth base; the growth itself is a model error (not seen in the samples'
/// runs yet: it needs a water source above the base for dripstone).
fn speleothem(w: &mut World, p: BlockPos, id: u16, base: &str) {
    let f = r(w).next_float();
    if f >= 0.011_377_778 {
        return;
    }
    let name = blocks::props(id).name.clone();
    let me = name.split('[').next().unwrap_or(&name).to_string();
    let stalactite = prop(&name, "vertical_direction") == Some("down");
    if !stalactite || block_id(&w.get(p.above())) == me {
        return;
    }
    if block_id(&w.get(p.above())) != base {
        return;
    }
    error(w, format!("a {me} at {p} may grow (SpeleothemBlock.growStalactiteOrStalagmiteIfPossible is not modeled)"));
}

/// `PointedDripstoneBlock.maybeTransferFluid`: with the roll under 0.17578125 at a stalactite's
/// start below a fluid, a cauldron fill or mud to clay: a model error (no cauldron or mud in the
/// samples' caves).
fn dripstone_transfer(w: &mut World, p: BlockPos, id: u16, f: f32) {
    if f > 0.175_781_25 {
        return;
    }
    let name = blocks::props(id).name.clone();
    if prop(&name, "vertical_direction") != Some("down") || block_id(&w.get(p.above())) == "minecraft:pointed_dripstone" {
        return;
    }
    let above2 = w.get(p.above().above());
    let fluid = crate::fluid::fluid_state(w, p.above().above());
    if block_id(&above2) == "minecraft:mud" || !fluid.is_empty() || matches!(above2, Block::Lava { .. }) {
        error(w, format!("pointed dripstone at {p} may move a fluid (maybeTransferFluid is not modeled)"));
    }
}
