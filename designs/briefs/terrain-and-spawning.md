# Terrain and natural spawning (design doc, gate G1)

terrain lane (session 81eaf43b), 2026-11-08. Gate G1 ([docs/process.md](../../docs/process.md)).
Revision 2, after the G1 review by d60ee61d (14 points); the changelog is at the end.

Asked by vjeux: "The day 1 farm needs to run in a real looking terrain, not a void world"
(2026-11-08 04:22 PT); "Why all of these caveats? Just do the real work!?" (04:19 PT); "Please
implement everything you need on redsim. We can't use the game code, it's way too slow in java"
(2026-11-07). The iron-farm stages (04:29 PT, relayed by the parent session 0e7eb68a): the day-1
farm is built beside a NATURAL VILLAGE in real terrain, the village keeps working around it, and
every stage is judged in real terrain with natural spawning. Every rule below was read in the
decompiled 26.3 server (`mctas src show <Class>`). No outside design, no internet.

**What this brief approves**: the terrain lane's own rules and code: the terrain samples and
their format, the natural block table, the light engine, the natural spawner, and the worst-case
checks (the refusal rules C1-C16 below) with the API the mob-farm judge calls. The judge's wiring
and card (ironfarm, 1372cc16), weather and darkening (infra, bf3de8fa) and the viewer (mobviewer,
80a994cf) are their lanes' work; the agreed split is at the end.

## Use

A player builds a mob farm on ground the game generated (beside a village on day 1) and needs to
know what it yields there and that nothing in that world, ground, night mobs or village, breaks it.

## The number

This is infrastructure, so its number is not a farm's (a farm keeps its own: iron per hour...). It
is **fidelity**, in two parts a player would care about:

1. **The world is the game's**: the share of cells (block states, biomes, heightmaps, light) equal
   to the game's own saved world, and natural spawning statistically indistinguishable from the
   game's (the validation's tests pass).
2. **Nothing unmodeled decides a verdict**: the share of the ways a natural or village mob can
   affect a farm (the channels C1-C16) that are either modeled exactly or refused by a sound check.

Reported beside them: the judge's added time per verdict, and the committed bytes per sample.

## Ceiling

Every cell equal (100%), every spawn test passing, every channel covered (16 of 16), no added run
time for a farm that passes (the checks cost seconds; the rate runs skip the spawner, see D3), the
committed samples a few hundred KB. Each layout's estimate below is given against it.

## Mechanics, from the game code (26.3)

### 1. World generation: run once, offline, never modeled

- The official server jar, `level-type=minecraft:normal`, `level-seed=S`, structures on (the
  existing `crates/mctas/src/vanilla` driver, which `bedrock video` already runs this way).
  `forceload add` holds the area's chunks with a FORCED ticket: `TicketType.FORCED =
  register("forced", 0L, 15)`, flags 15 include `FLAG_SIMULATION` (4), so the chunks tick entities
  and the generated village claims beds and job sites while time runs. Mob spawning is off (the
  driver sets `spawn_mobs false`, `spawn_monsters false`).
- What the region files hold (`SerializableChunkData.write`): per section `block_states`
  (palette + packed longs), `biomes` (palette + packed, one per 4x4x4 cell), `BlockLight` and
  `SkyLight` (2048-byte nibble arrays, a layer with `DataLayer.isEmpty()` (no data, default 0) not
  written, lines 336-339); per chunk `Heightmaps` (packed `ceillog2(height+1)` bits), `isLightOn`,
  `structures.starts` (each piece's `BB`). Entities are in `entities/r.X.Z.mca` with their full NBT
  (villagers' `Brain.memories` home / job_site / meeting_point, the golem's `PlayerCreated: 0b`).
  The world spawn is in `level.dat`. A section without a saved sky layer reads 15 when no section
  above it in the chunk has one (open sky above the chunk's top layer:
  `SkyLightSectionStorage.getLightValue`, `sectionY >= topSection`), else 0 (an all-0 layer is never
  written, `DataLayer.isEmpty`); `mctas terrain probe` confirmed the running game reads exactly that
  (`getMaxLocalRawBrightness` 0 in such caves at noon, 15 above the top layer).

### 2. Light (modeled exactly)

- **Sky light** (`SkyLightEngine`, `ChunkSkyLightSources`): each column holds 15 from the top
  down to its lowest source y, the first occluded edge (`isEdgeOccluded`: the lower state's
  dampening is non-zero, or the two faces' occlusion shapes cover the face). From there light
  spreads to the six neighbours losing `LightEngine.getOpacity` = `max(1, getLightDampening())`,
  never across `shapeOccludes` faces (`getFaceOcclusionShape` of `useShapeForLightOcclusion` states:
  slabs, stairs, farmland, paths, snow layers...).
- **Block light** (`BlockLightEngine`): each state's `getLightEmission()` spreads the same way.
- **Queries** (`LevelLightEngine.getRawBrightness`): `max(block, sky - darken)`;
  `Level.getSkyDarken()` from the day timeline blended toward rain and thunder (infra's
  `w.sky_darken()`); `LevelReader.getMaxLocalRawBrightness` = `getRawBrightness(pos,
  getSkyDarken())` inside the world border; `Level.canSeeSky`.
- **Timing**: the server queues light work (`ThreadedLevelLightEngine.checkBlock`) and runs it
  between ticks. redsim recomputes at once (below), so a game read in the tick of a light-relevant
  change, or the next, within 15 blocks of it may differ: such a read is a model error (C3 of the
  light rules below), never a guess.

### 3. Biomes

- `BiomeManager.getBiome(pos)`: the 4x4x4 cells read with the fuzzy zoom (`getFiddledDistance`
  over the 8 corner cells of (pos - 2) / 4) seeded by `biomeZoomSeed = obfuscateSeed(seed)` =
  `Hashing.sha256().hashLong(seed).asLong()` (lines 23-24). The client gets the same hashed seed at
  login, so tints use the same per-block biome.
- Spawn lists: `data/minecraft/worldgen/biome/<b>.json`, attribute
  `minecraft:gameplay/natural_mob_spawns` (per category: type, `count` constant or uniform,
  `weight`; `spawn_costs`), read through `EnvironmentAttributes.NATURAL_MOB_SPAWNS`.
  `NoiseBasedChunkGenerator.getMobsAt` replaces the list inside a structure with spawn overrides
  (swamp hut, monument, outpost, fortress...): a sample holding one is refused.
- Colours: `Biome.getGrassColor(x, z)` = the modifier applied to (the override, or
  `GrassColor.get(clamp(temperature), clamp(downfall))`), `getFoliageColor`, `getDryFoliageColor`,
  `getWaterColor` (checked by mobviewer in the 26.3 client: not environment attributes); modifiers
  `dark_forest` ((c & 0xFEFEFE) + 0x28340A) >> 1 and `swamp` (BIOME_INFO_NOISE);
  `ClientLevel.calculateBlockTint` averages the (2r+1)^2 columns at the same y, r = 2.

### 4. Natural spawning, the per-tick cycle (modeled exactly)

`ServerChunkCache.tickChunks`, after the scheduled block and fluid ticks and raids, before block
events and entities (redsim's slot for crops' random ticks, `world.rs`):

1. `NaturalSpawner.createState(chunkCount, ...)`: every entity that is not a persistent mob
   (`isPersistenceRequired() || requiresCustomPersistence()`) and not MISC is counted in its
   `MobCategory`, its spawn charge added (`PotentialCalculator`), and added to the local cap of each
   player near its chunk. `chunkCount` = `DistanceManager.naturalSpawnChunkCounter`, a
   `FixedPlayerDistanceChunkTracker(8)`: the chunks within Chebyshev distance 8 of a player's chunk
   (289 for one player).
2. `spawnPersistent` = game time % 400 == 0; `getFilteredSpawningCategories`: enemies on (difficulty
   not peaceful) or a friendly category; persistent ones (CREATURE) only when due; under the global
   cap `getMaxInstancesPerChunk() * chunkCount / 289` (MONSTER 70, CREATURE 10, AMBIENT 15,
   AXOLOTLS 5, UNDERGROUND_WATER_CREATURE 5, WATER_CREATURE 5, WATER_AMBIENT 20).
3. `ChunkMap.collectSpawningChunks`: ticking chunks within 128 blocks of a non-spectator player
   (`euclideanDistanceSquared`: chunk centre (16x+8, 16z+8) to the player, 2D, < 16384), then
   `Util.shuffle(list, level.random)`.
4. Per chunk and category: the local cap (`LocalMobCapCalculator.canSpawn`: a near player whose own
   count is below the category's max), then `getRandomPosWithin` (x, z uniform in the chunk, y
   uniform in [minY, WORLD_SURFACE(x, z) + 1]; nothing below minY + 1).
5. `spawnCategoryForPosition`: nothing if the start `isRedstoneConductor`; 3 groups of
   `ceil(nextFloat() * 4)` tries walking `x += nextInt(6) - nextInt(6)` (and z) at the start's y; per
   try: the nearest player (3D) farther than 24, the world spawn not within 24, the chunk loaded;
   the type drawn once per group (`getRandomSpawnMobAt`), then `max = count.sample(random)`;
   `isValidSpawnPostitionForType`: not MISC; within the category's despawn distance (128) unless
   `canSpawnFarFromPlayer`; `canSummon`; still listed there (`canSpawnMobAt`);
   `SpawnPlacements.isSpawnPositionOk` (`SpawnPlacementTypes`: ON_GROUND = the block below
   `isValidSpawn(level, below, type)`, feet and head `isValidEmptySpawnBlock` = not a full collision
   cube, not a signal source, no fluid, not `PREVENT_MOB_SPAWNING_INSIDE`, not
   `type.isBlockDangerous`; IN_WATER, IN_LAVA, NO_RESTRICTIONS); `SpawnPlacements.checkSpawnRules`
   (section 5); `level.noCollision(type.getSpawnAABB(x+.5, y, z+.5))`; the potential test. Then the
   mob is created, `snapTo(x+.5, y, z+.5, nextFloat()*360, 0)`, `isValidPositionForMob` (a
   `PathfinderMob` needs `getWalkTargetValue >= 0`, a `Monster`'s being
   `-getPathfindingCostFromLightLevels`; `checkSpawnObstruction`: no liquid in its box and
   `isUnobstructed`), `finalizeSpawn` (babies, jockeys, equipment: their draws and the entities they
   add), `addFreshEntityWithPassengers`, `afterSpawn`; it stops at `getMaxSpawnClusterSize()` (4 by
   default) and the group at `isMaxGroupSizeReached`.
6. Then `ServerLevel.tickChunk` (random ticks) and `tickCustomSpawners` (section 7).

### 5. Spawn rules per mob (`SpawnPlacements` registrations)

- **Monsters** (zombie, skeleton, creeper, spider, enderman, witch, zombie villager, zombie horse
  in 26.3's plains list): `Monster.checkMonsterSpawnRules` = `isDarkEnoughToSpawn` and
  `Mob.checkMobSpawnRules` (the block below `isValidSpawn`). `isDarkEnoughToSpawn`: refused when
  sky light `> nextInt(32)`; refused when block light `> monsterSpawnBlockLightLimit` (0 in the
  overworld); else passes when `getMaxLocalRawBrightness` (darken 10 while thundering) `<=
  monsterSpawnLightTest.sample` (uniform 0..7 in the overworld).
- `checkSurfaceMonstersSpawnRules` (husk, parched, camel husk): plus `canSeeSky`.
- **Animals** (`Animal.checkAnimalSpawnRules`): the block below in `ANIMALS_SPAWNABLE_ON` and
  `getRawBrightness(pos, 0) > 8`; CREATURE spawns only every 400 gt.
- Slime, bat, drowned, squid, fish, wolf, rabbit, fox...: each its own method, read and modeled for
  every type a sample's biome lists.
- `isValidSpawn` per block (`BlockBehaviour.Properties.isValidSpawn`): default
  `isFaceSturdy(UP) && lightEmission < 14`; leaves ocelots and parrots only, glass never, ice polar
  bears only, magma fire-immune only.

### 6. Despawning and persistence (modeled exactly)

- `Mob.checkDespawn`: peaceful removes monsters; a non-persistent mob is discarded at once beyond
  its category's despawn distance (128; WATER_AMBIENT 64) of the nearest player; beyond 32 when
  `noActionTime > 600 && random.nextInt(800) == 0`; within 32 `noActionTime` resets.
  `noActionTime` grows by one per AI step (`Mob.serverAiStep`), two more for a monster in light
  above 0.5 (`Monster.updateNoActionTime`), and resets on damage.
- `requiresCustomPersistence`: a passenger or leashed mob; `isPersistenceRequired`: a name tag or
  picked-up loot. Villagers, golems and generated animals do not despawn.
- The player: the design declares an AFK spot (`afk: x y z`, design coordinates; default: on the
  design's top centre); the judge puts one non-spectator player there (creative in the game check:
  counted for spawning, never targeted).

### 7. What else appears, and the village (not modeled as AI: covered by the checks)

- `ServerLevel.tickCustomSpawners`: `PhantomSpawner` (players awake for 3 days; in the sky above
  them), `PatrolSpawner` (pillagers 24-48 blocks away, never near villages), `CatSpawner` (cats in
  villages), `VillageSiege` (at `ROLL_VILLAGE_SIEGE`, one night in ten, `level.isVillage(player)`:
  20 zombies 32 blocks from the player; the farm's beds make a village), `WanderingTraderSpawner`.
- The village's villagers walk to beds, job sites and the bell (doors: `InteractWithDoor` opens only
  `MOB_INTERACTABLE_DOORS` that are `DoorBlock`s, never fence gates or trapdoors); its golem
  strolls (`GolemRandomStrollInVillageGoal`, `MoveBackToVillageGoal`).
- Why it matters (the iron-farm brief): a villager within 10 of a farm villager counts in
  `spawnGolemIfNeeded`; any iron golem in a farm villager's follow box stops it 600 gt
  (`NearestLivingEntitySensor` lists every living entity in the box, no line of sight; `GolemSensor`);
  a villager reaching a free bed claims it (`AcquirePoi` HOME); a seen hostile keeps a villager awake.

### 8. What a mob can do to a farm

- Bodies (`EntityTypes`): zombie, husk, drowned, zombie villager 0.6 x 1.95, babies 0.3 x 0.975;
  skeleton 0.6 x 1.99; creeper 0.6 x 1.7; spider 1.4 x 0.9; enderman 0.6 x 2.9; witch 0.6 x 1.95;
  villager 0.6 x 1.95 (baby 0.3 x 0.975); iron golem 1.4 x 2.7; cat 0.6 x 0.7; chicken 0.4 x 0.7;
  rabbit 0.4 x 0.5; bat 0.5 x 0.9 and phantom 0.9 x 0.5 (fly). Step 0.6 (golem 1.0), jump 1.25.
- Moving: spiders climb what they push against (`Spider.onClimbable` = horizontal collision);
  endermen teleport (`EnderMan.teleport`: within 32 on each axis, moved down to a `blocksMotion`
  block, no collision, no liquid); zombies break wooden doors on Hard (`DOOR_BREAKING_PREDICATE`).
- Seeing: `VillagerHostilesSensor` (zombie, zombie villager, husk, drowned, vex 8; vindicator,
  zoglin 10; evoker, illusioner, ravager 12; pillager 15; feet to feet) through
  `Sensor.isEntityTargetable` (follow range, `hasLineOfSight`: a COLLIDER ray eye to eye).
- Reaching: `Mob.getAttackBoundingBox` (its box joined with its vehicle's, inflated by sqrt(2.04) -
  0.6); item pickup (`Mob.aiStep`: its box inflated (1, 0, 1), no line of sight); creepers swell
  when the target is within 3 (`SwellGoal.canUse`: distanceToSqr < 9), keep swelling while within 7
  (`tick`: > 49 stops) and in sight, explode at power 3 (6 when charged by lightning); endermen take
  a block in `(x-2..x+2) x (y..y+3) x (z-2..z+2)` they can see (`EnderManTakeBlockGoal`,
  `ENDERMAN_HOLDABLE`) and place one in `(x-1..x+1) x (y..y+2) x (z-1..z+1)`
  (`EnderManLeaveBlockGoal.canPlaceBlock`).
- The golem search: `SpawnUtil.trySpawnMob(IRON_GOLEM, ..., villager block, 10 tries, dx dz in
  -8..8, start y+6, scanning down to y-6, LEGACY_IRON_GOLEM)` (`entity/mob/spawn.rs`), over terrain
  too: grass beside a farm is a valid golem floor.
- Fire: `LightningBolt.spawnFire` (Normal/Hard: at the strike and 4 cells within 1), `FireBlock`
  spreading to flammable blocks (`x±1, y-1..y+4, z±1`), `LavaFluid.randomTick` igniting air next to
  flammable blocks.

## Five layouts (end to end)

Each is a coherent choice of ground, block model, light, spawning and safety. Estimates against
the ceiling (fidelity part 1 = cells equal and spawn tests; part 2 = channels covered).

| # | layout | world cells equal | spawn tests | channels covered | judge time per verdict | committed size | % of ceiling | constraints |
|---|---|---|---|---|---|---|---|---|
| L1 | **The obvious design**: a hand-built world (a grass plane, placed oak trees), only states redsim knows, daylight.rs's per-query light, the full spawner in every rate run with spawned mobs standing still, no checks | 0% (not the game's world) | not testable (no game twin) | 0 of 16 (a mob that would walk to the villagers never does) | + ~2.5 h (~800 spawn tries a tick x 38M judge ticks) | ~10 KB | ~5% | fake ground; slow; proves nothing |
| L2 | The game's ground from the region files, light taken from the saved arrays and never updated, the spawner in every run, no checks | 100% at load; light wrong wherever the farm or its box changed it (~1-5% of a crop's cells) | pass at load, drift after the paste | 0 of 16 | + ~2.5 h | ~300 KB | ~35% | light wrong after any change |
| L3 | The game's ground, an exact light engine, the spawner in every run, natural mobs moved by walking policies like the golems' (still + 8 directions) | 100% | pass | ~5 of 16 (policies walk straight lines; a path around a corner to a window is never tried) | + ~25 h (9 policies x the spawner) | ~300 KB | ~55% | policies prove nothing about paths |
| **L4** | **The game's ground, a natural-state table dumped from the game, the exact light engine, static worst-case checks over every reachable position (C1-C16), the spawner where it is the outcome** | 100% (test) | pass (validation) | 16 of 16 (each modeled or refused) | + seconds (the checks; rate runs unchanged) | ~250 KB per sample | ~100% | refuses farms whose safety depends on mob AI (sound, conservative) |
| L5 | L4 plus natural mobs' AI modeled exactly (goal selectors, `PathFinder` / `WalkNodeEvaluator`, targeting, attacks) | 100% | pass | 16 of 16, modeled instead of refused | + hours (every mob pathfinds) | ~250 KB | ~100% and fewer refusals | weeks of work; out of this time box |

## Trade-offs

- **Fidelity vs size**: a full 17 x 17-chunk, full-height area is 28M cells (5-15 MB); a crop of
  what the checks reach is ~250 KB. Crops are committed, full areas cached (regenerable).
- **Fidelity vs judge time**: running the spawner in every rate run costs hours and changes nothing
  for a farm the checks clear; a proof (the checks) costs seconds.
- **Soundness vs acceptance**: static worst-case checks refuse some farms that would work in
  practice (a zombie that could walk to a window but rarely would); the house policy prefers that
  to an unmodeled guess.
- **Exactness vs effort for natural states**: one variant + a table the game dumps (exact by
  construction, minutes per new state) vs hand-modeling ~100 states (weeks, error-prone).
- **Light timing**: recomputing at once is exact for every read except in the tick of a nearby
  light change, which is reported (iron farms make no light-relevant change while running).

## Decision

**L4**, piece by piece:

- **Ground (A)**: the official server (L2-L5) because only the game's output is real (L1's
  hand-built world is what vjeux refused) and re-implementing generation is months; the bridge's
  sandboxes cannot generate (void levels).
- **Natural states (B)**: `Block::Terrain(u16)` with the dumped table, because hand-modeling is
  weeks and a guessed default (stone/air) fakes collisions, light and spawn floors.
- **Light (C)**: an engine with incremental updates, because the saved arrays (L2) are wrong after
  the paste and the per-query fixed point costs ~10^4 cells per read (spawning reads millions).
- **Spawning in the judge (D)**: static worst-case checks, because unmodeled walking makes any
  simulated run say nothing about safety (L1-L3) and modeling AI (L5) is out of the time box; when
  every check passes no natural or village mob has any channel to the farm, so rate runs without the
  spawner are exact; the spawner runs where it IS the outcome (mob towers: their spawn rate on the
  tower's floors, an upper bound until a later brief models the walk-off) and in the validation.
- **Extent (E)**: crops committed, full areas cached, because the checks reach a known distance
  (golem search 8, sight 15, follow box 18.84, creeper 7 + its explosion: blocks within 10.4,
  entities within 12) and the spawner's validation needs the full 128-block radius.

### Samples (pinned)

Seed **2** (`level-seed=2`, 26.3, default world). `/locate` from (0, 0): `village_plains` at
(-416, ~, 240), `forest` at (0, 64, 0) (recorded by `mctas terrain find --seed 2`).

| sample | generated area (cached) | farm spot | committed crop |
|---|---|---|---|
| `village-edge` (the day-1 set) | centre (-416, 240), radius 192 blocks (384 x 384 = 24 x 24 chunks, y -64..319) + 2 margin chunks, 1 game day run: world `2_-416_240_r192` | `place::best_spot` from (-352, 238), radius 14 (an 8 x 16 footprint, 5 layers below ground: min column (-360, 248), ground y 72, 656 blocks dug) | 120 x 120 columns around (-372, 250), y 30..86 (285 KB): the bells (-411, 70, 241/249), 8 villagers in their beds, the iron golem, 2 cats |
| `plains` | the same area | from (-292, 222), radius 14 (8 x 16: (-306, 233), y 71, 580 dug) | 96 x 96 around (-280, 230), y 25..89 (149 KB) |
| `forest` | centre (16, 0), radius 80 + 2 margin chunks, 1 day: world `2_16_0_r80` | from (-43, -6), radius 14 (8 x 16: (-43, -6), y 67, 643 dug) | 96 x 96 around (-16, 0), y 37..88 (167 KB): forest and a river |

`redsim::terrain::samples::STANDARD` records each (file, seed, world and the `gen` line that
re-makes it, the search start). The spot rule (`place::refusal` + `best_spot`, deterministic, the
judge and the game check use the same): the ground level (the highest ground over the footprint:
down from MOTION_BLOCKING_NO_LEAVES past logs, leaves and blocks without collision) within 2 blocks
everywhere in it; no structure piece in its columns; no fluid in them nor at their surface; no tree
trunk in the cleared cells (overhanging leaves are cleared with the site); no fluid or attached state
(wall plants and lights, fences, walls, panes, stairs, beds, chests, doors, lanterns, bells) beside a
cleared cell; then the fewest blocks dug, then the nearest to the start.

### Placement

The design's `ground: Y` line names its layer that sits on the surface; the judge puts that layer
one above the spot's ground and **clears the footprint** (every cell in the design box's columns
from its lowest layer to the world top: a player clears the site; cave air is rewritten as air and
not counted), then pastes the design with the judge's usual code. Nothing natural is left without
support: the spot rule refuses any attached or reshaping state beside a cleared cell, so the
removal set is empty. The game check writes the same cells (`setblock x y z <state> strict`:
`SetBlockCommand`'s `strict` mode, flags 2|816, no neighbour or shape updates, as the repo's vanilla
checks already use) so both worlds are identical. **Blocks dug** = the natural cells cleared that
are not air of some kind.

### The natural-state table (B3)

`crates/redsim/src/terrain/blocks.tsv`, generated by `mctas terrain blockinfo` from every committed
sample's palette (sorted, deduplicated), the game version and the type list in its header; a test
checks every committed sample's palette is in it, and `blockinfo --check` re-dumps and compares. Per
state, the game's own answers (a new bridge op calls them on each state in a sandbox):
collision boxes (`getCollisionShape(...).toAabbs()`), `getLightDampening`, `getLightEmission`,
`useShapeForLightOcclusion` + the six `getFaceOcclusionShape` grids (16 x 16 bits), `propagatesSkylightDown`,
`isSolid`, `isRedstoneConductor`, `isCollisionShapeFullBlock`, `isFaceSturdy(UP)`,
`isSignalSource`, the fluid (type, amount, source), `PREVENT_MOB_SPAWNING_INSIDE`,
`ANIMALS_SPAWNABLE_ON`, `ENDERMAN_HOLDABLE`, flammability (`FireBlock.getBurnOdds` /
`getIgniteOdds`), `blocksMotion`, `isPathfindable(LAND)`, `isAir`, `LeavesBlock` (the four heightmap
predicates: WORLD_SURFACE = not air, MOTION_BLOCKING = blocksMotion or a fluid, NO_LEAVES = that and
not leaves, OCEAN_FLOOR = blocksMotion), `isRandomlyTicking`, whether its class overrides
`updateShape` / `neighborChanged` / `tick` (reflection: the inert flag), the directions whose
neighbour turning to air makes `canSurvive` false (support flags), and **per (state, type)** for the
59 natural types (every type of the overworld biome lists, the village's and the custom spawners',
the sulfur cube and the bee: listed in the header) two bitsets: `isValidSpawn(level, pos, type)` and `type.isBlockDangerous(state)`.

### Light (C3)

- A **light-relevant change**: a cell whose state changes dampening, emission, occlusion faces or
  sky-source status. The map starts as **the game's own saved light** (the region files' arrays:
  the game's runtime light is not the engines' fixed point everywhere: in 0.0075-0.04% of open
  cells, under leaves grown by a neighbouring chunk's decoration, the game keeps a stale 15, read
  back by `mctas terrain probe` from the running server, so the fixed point would be wrong there),
  and after changes it is updated like `LightEngine`: the column sky sources move as
  `ChunkSkyLightSources.update` moves them, and the region within 15 of the changes (and of the
  moved sources' columns) is recomputed as the fixed point with the cells around it as its fixed
  border (`World::settle_light`, run between ticks; `terrain/land.rs`).
- **Exact**: after every change the map is what the game has once its queued light work ran
  (validated: the iron-day1 farm pasted into the village sample in redsim and in the official
  server, 744,192 cells compared, 0 sky and 0 block light differences). **Reported**: a read (spawn
  check, daylight detector, sunburn) at a cell within 15 of a light-relevant change made in the same
  tick or the previous one is a model error that invalidates the run (as daylight.rs does today).

### The checks (D3): every channel, its rule

The **farm interior** = the cells of the design box that the farm's own entities, items and fluids
occupy in the judge's runs, and the air cells connected to them through cells with no collision
(the recorded envelope). Worst cases: difficulty Hard where it is worse (door breaking, spider
effects, babies), Easy where that is worse; any day time; thunder. The positions each mob can take
come from a **flood** per body class (the types' boxes; a position = a cell and the box anywhere in
it that fits, standing on a face with a collision top within step/jump/fall reach; climbing for
spiders; any air for flyers; every valid teleport target for endermen), started at **every standable
position of the sample whatever its light** (this covers natural spawns, custom spawners' walkers,
the village's mobs and the edges) and every air cell for flyers. Bound: ~1M cells x 9 body classes,
26-neighbour steps: ~2 x 10^8 operations, ~1-2 s; sight rays from the cells within 15 of a
villager: ~10^5 rays.

| channel | refusal rule |
|---|---|
| C1 natural spawn inside | an interior cell passes ON_GROUND (or IN_WATER) placement for a listed type with block light 0 and `max(block, sky - 11) <= 7` (the gates' worst readings: `nextInt(32)` can always exceed sky light; `sample(0..7)` can be 7; darken up to 11 at night) and the type's box fits; or animal rules at brightness > 8 |
| C2 entry and contact | a position of any body class overlaps the interior (pushing, plates, tripwires, target blocks, `isUnobstructed` golem spots, beds, items) — doors a mob opens (villagers) or breaks (zombies on Hard) count as open |
| C3 sight | a hostile-sensor type's eye at one of its positions within its distance of a farm villager (standing or sleeping, any position in its pod) with a clear COLLIDER ray |
| C4 presence | an iron golem position within a farm villager's follow box (`|dx|, |dz| < 17.84 + 0.3 + 0.7`, its vertical span), or a villager position within 10 of a farm villager's box |
| C5 melee and ranged reach | an attacker's attack box (with vehicle) meeting a farm villager, the farm's zombie or its vehicle (the village golem attacks the zombie: `IronGolem` targets `Enemy`); a ranged attacker (skeleton 15, pillager 8, witch 10, drowned trident 10) with a COLLIDER ray to a farm entity within its range |
| C6 pickup | a looting type's position (zombie family, skeletons, pillagers, villagers, foxes) whose box inflated (1, 0, 1) meets a cell the farm's items occupied in the runs |
| C7 golem search | `LEGACY_IRON_GOLEM` from a farm villager finds, in any of its 17 x 17 columns, a first matching cell outside the farm's own spawn floors that passes `checkSpawnObstruction` (exhaustive, deterministic) |
| C8 creepers | a creeper position within 7 of the AFK spot with a ray to the player's eye, whose power-6 explosion (the worst factor 1.3, `explosion.rs`) breaks a farm block or whose entity radius (12) reaches a farm entity with exposure > 0 |
| C9 endermen | a farm block in `ENDERMAN_HOLDABLE` within the take box of an enderman position with a clear OUTLINE ray, or a farm air cell within its place box |
| C10 flyers | a bat or phantom position overlapping the interior |
| C11 fire | a flammable farm block within `FireBlock`'s reach of a cell that can hold fire: lightning-strikable tops (canSeeSky), lava's ignition cells, and the closure through flammable blocks |
| C12 sculk | a sculk sensor of the design within 8 of any mob position (their steps are vibrations) |
| C13 bed claims | a village villager position from which a farm bed head is within `AcquirePoi`'s reach (a path ending within 1 block) |
| C14 raids and sieges | sieges' zombies are walkers (C2-C9 cover them); raids need a Bad Omen player: the judge's AFK player has none (no channel) |
| C15 light and caps | natural mobs emit no light (no channel); they fill the mob cap, which matters only to natural-spawn farms, whose rate is the spawner's |
| C16 lightning on the farm's mobs | a farm villager whose eye cell `canSeeSky` (sky light 15: lightning there turns it into a witch); the zombie's own exposure changes nothing (no channel): the iron-farm roof must-have, refused here too |

A farm passing C1-C16 has no channel to any natural or village mob, so its rate runs skip the
spawner exactly. Each refusal names the channel, the mob type, its position, and the cell or
entity it reaches.

### The `.mcterrain` format (version 1)

Little-endian, no compression inside, decoded by pure-Rust redsim code (no dependencies, builds for
wasm): magic `MCTERRv1`, tagged sections (HEAD: seed, zoom seed, world spawn, min y, height, game
and day time, origin, size; PALT + CELL: palette and u16 cells, x fastest then z then y, in varint
runs; BPAL + BIOM: biome palette and the 4x4x4 cells the box reads; HMAP: four heightmaps; SKYL,
BLKL: saved light in runs; STRU: structure boxes; ENTS: entities (id, position, SNBT); NOTE:
provenance). **Size**: a 96 x 96 x ~60 crop (550k cells) ~150-250 KB raw, ~60 KB in git; three
samples under 1 MB. Full areas (28M+ cells) stay in the cache. Measured sizes replace these.

## Must-haves

- The terrain equals the game's (blocks, biomes, heightmaps, light, entities) cell for cell; the
  table equals the game's re-dump.
- The design placed like a player builds: footprint cleared, outside natural, unsupported natural
  states removed; the game check writes the same cells.
- Light reproduces the saved arrays; exact after every change, reads in the timing window reported.
- Natural spawning and despawning rule for rule (sections 4-6), the game's unmirrorable draws from
  the seeded stand-in with the same distribution (the mob layer's policy).
- Difficulty Normal for rates (the singleplayer default); refusals at the worst difficulty per rule.
- Every refusal names channel, mob, position, target.
- The viewer runs the same world as the judge: the world hash (blocks and entity positions) of the
  viewer's wasm run equals the native run's every 20 ticks over one game day (mobviewer's test on
  the shared `farm::Run`).

## Not goals

- Generating terrain in Rust; other dimensions.
- Natural mobs' AI (walking, targeting, attacks): refused by C1-C16 instead (L5 later).
- A mob tower's full rate (walk-off into water and drops): its spawn rate only, until a later brief.
- Random ticks of natural states: redsim skips them (natural states are inert by declaration; the
  farm runs' random ticks touch only the design's own blocks). The randomly ticking states in the
  committed samples (`mctas terrain info <file> --ticking`) and why none can change a check:
  `grass_block` (spread and decay flip grass and dirt: both opaque, sturdy, valid monster floors;
  only the creature spawner's grass rule reads the difference), `farmland`, `wheat`, `carrots`,
  `potatoes`, `beetroots` (the village fields: crops have no collision and no light, are not a
  floor; farmland stays farmland while wet), `sugar_cane` (no collision, no light), `vine` (one, in
  the forest crop, far from any spot), `sulfur_spike` (sulfur caves, deep underground), `lava`
  (147 cells in the plains crop's caves, 2 in the forest's: lava's fire only matters next to
  flammable blocks, and no spot is within reach). Leaves are not in the list: natural leaves have
  `persistent=false` and distance <= 6, and their random tick only acts at distance 7; the site
  clearing is a strict write, so no leaf's distance ever changes. Mushrooms would add block light
  1 for the brown one (`Blocks.BROWN_MUSHROOM ... lightLevel(state -> 1)`, Blocks.java:1146); none
  is in a committed sample.

## Validation (pass/fail; results recorded here)

1. **Terrain and light**: every cell of every sample: the file vs a fresh read of the region files
   (0 diffs), redsim's light engine vs the saved arrays (0 diffs; a diff is a model bug until
   explained by the game's own code).
2. **Natural spawning, V1 (exact)**: the `village-edge` area (cached, full), the farm spot as the
   AFK spot. The game: the official server on that world, the bot (creative) at the spot,
   simulation distance 10, difficulty Normal, weather clear, `spawn_mobs` on, 8 nights (day time
   13000 to 23000 = 10000 ticks each), every non-persistent mob removed at each dusk; a datapack
   (written by Rust) sets each new mob's movement speed to 0 at its first tick (so nothing walks,
   as in redsim) and logs (type, block, game time) to a storage read over RCON. redsim: the same world,
   spot and nights, 32 seeds of the stand-in random. Tests, each at 5% with Holm's correction across
   them: monsters per night (Welch), type mix (chi-square), cluster sizes (chi-square), spawn cells by
   16-block ring x height band (cave / surface / above; chi-square), monsters alive at each 1000-tick
   mark (Welch per mark). Pass = no rejection. A rejection is a model bug, fixed before any push of
   the spawner.
3. **V2 (walking on in the game)**: the same, mobs walking: the tests on the first 1200 ticks of
   each night (pass at 5%, Holm); after that only the alive-count difference is reported.
4. **The checks' fixtures** (`designs/terrain/fixtures/`, each on `village-edge`): F1 a closed dark
   5x5 room (C1 refuses), F2 a villager pod with an open side 6 blocks from grass (C3), F3 villager
   pods at ground level (C7: grass columns), F4 an AFK spot on open ground 3 blocks from the farm
   (C8), F5 a bed reachable from the village (C13), F6 a sealed, lit box with three podded
   villagers 25 blocks above the ground and its AFK spot inside (passes every check).

### Results (revision 3, 2026-11-08)

Commands: `mctas terrain light|paste-check|biome-check|probe|spawn-check|hostile|spot`.

1. **Terrain and light** — the files are the region files read back (blocks, biomes, heightmaps,
   saved light, structure pieces, entities). Light vs the engines' fixed point over the whole
   village area (47,579,136 cells, 32,674,816 open): block light 0 differences; sky light 2,437 open
   cells (0.0075%), all where the game's own light is stale under leaves (probed in the running
   server: it reads the saved value) — redsim therefore starts from the saved arrays (exact by
   construction) and relights around changes. **Paste check** (`terrain paste-check`): the archived
   iron-day1 pasted at (-335, 255) of the village sample in redsim and in the official server (the
   same 639 strict writes, 454 blocks dug): 744,192 cells, **0 block, 0 sky, 0 block-light
   differences**. **Biomes** (`terrain biome-check`): redsim's `biome_at` vs `execute if biome` at 300
   random cells of the validation area: **0 differences**. **Darkening** (`terrain probe --times`):
   the game's surface light at day 12000-23000 equals 15 - redsim's `sky_darken` at every probe.
2. **Natural spawning, V1** (`terrain spawn-check`; area `2_-342_255_r160`: seed 2, centre (-342,
   255) radius 160, the village and its east edge; AFK (-341.5, 72, 255.5); the game: the bot
   (creative), simulation and view distance 10, Normal, clear weather, a one-line data pack setting
   every mob's movement speed to 0 each tick; redsim: the same world, 32 seeds; the mobs alive at
   the night's end compared per type with Welch's test, Holm-corrected):
   - **Full darkness** (day 18000, 300 ticks, 6 game nights): monsters 70.83 vs 71.25 (p 0.29);
     every type's p(Holm) = 1.000 except none below 0.56; surface (y >= 60) 28.83 vs 28.97 per night;
     sulfur cubes 2.00 vs 2.94; mean height 39.5 vs 37.5 (p 0.27): **pass**.
   - **Despawning** (spawning off after 300 ticks, 1600 ticks): the game 71 → 19, redsim 72 → 23,
     both starting at tick ~650 (`noActionTime > 600`, `nextInt(800)`): the curves match.
   - **Dusk** (day 13000, 1200 ticks, 8 game nights): monsters 70.75 vs 71.22 (p 0.41), no type
     significant after Holm (lowest 0.73), **but** the surface share after the first despawns
     differs: surface monsters 12.6 vs 16.2 per night at the end, sulfur cubes 5.5 vs 2.8 (raw p
     0.06); the game's per-100-tick counts show it replaces despawned mobs mostly underground at
     dusk, redsim at the full-night ratio. Open: not explained yet (the darkening, the despawn rate
     and the full-night spawn mix all match); the worst-case checks do not depend on it; a farm whose
     rate is natural spawning (mob towers) gets its rate from full-night runs until it is.
   Fixed during validation: `noActionTime` counted twice (the mob layer's `serverAiStep` already
   adds 1), the slime/sulfur-cube size drawn before `isValidPositionForMob` (the game checks the
   size-1 box), structure spawn overrides (trial chambers: no natural spawns in their pieces), the
   sulfur cube and the bee added to the table's types.
3. **V2 (walking)**: not run (natural mobs' walking is not modeled; the checks cover it).
4. **The checks**: `terrain hostile` on the archived iron-day1 at the village edge (an open void
   design) refuses C1, C2, C3, C4, C5 and C7, as it must; ~7 s per placement (floods over the
   design's surroundings, 24 columns around it, the whole height).

## File split (agreed)

- terrain lane: `redsim/src/terrain/{mod,format,blocks,light,biome}.rs`, `entity/mob/natural.rs`,
  `entity/mob/hostile.rs`, `crates/terrain` (`mctas terrain gen|find|info|crop|blockinfo|spot|validate`),
  `designs/terrain/`, the bridge op; small arms in `block.rs`, `world.rs` (light map, spawning hook),
  `daylight.rs`, `entity/collide.rs`, `entity/mob/blocks.rs` (diff to ironfarm first),
  `entity/mob/mod.rs` (`pub mod natural; pub mod hostile;`, `MobWorld.natural`).
- ironfarm (1372cc16, agreed 2026-11-08 12:37Z): `MobType::Natural(u16)` reading `natural::DEFS`,
  the runner's terrain wiring (`farm::Run`), the mob-farm judge, its card (terrains, blocks dug).
- infra (bf3de8fa, agreed 12:39Z): `w.sky_darken()`, rain, thunder, lightning, sunburn; it reads
  this lane's light accessors.
- mobviewer (80a994cf, agreed 12:37Z): the page bundle (`data-terrain`), drawing `Block::Terrain`
  by its state string, tints from `terrain::biome_at` and the resolved biome colours.

## Changelog (revision 2, after the G1 review by d60ee61d)

| point | what changed |
|---|---|
| 1, 2 | Five end-to-end layouts with world-cells, spawn-test, channel, time and size estimates and % of ceiling; L1 labelled the obvious design; a Ceiling section |
| 3 | Use in one sentence; the number: fidelity in two parts (the infrastructure adaptation stated) |
| 4 | A Trade-offs section; the decision argued per piece against the alternatives |
| 5 | Seed 2, `/locate` results, areas (blocks and chunks), days, the spot rules, crop dimensions |
| 6 | The table's fields, isValidSpawn and isBlockDangerous per (state, type) over 47 listed types, the heightmap predicates, versioning; the placement and inert refusal defined |
| 7 | Light: relevant change, update passes and bound, exact vs reported |
| 8 | Sixteen channels with a rule each; the flood's starts and bound |
| 9 | Worst readings per gate (C1); creeper 3/7 and power 6; the golem search's numbers; the enderman take/place boxes; drops = the cells items occupied in the runs; the village golem and villagers decided (C4) |
| 10 | The flood starts at every standable position whatever its light and every air cell for flyers: custom spawners included |
| 11 | Mob towers: spawn rate only; V1 and V2 fixed (nights, setup, seeds, tests, pass/fail); fixtures F1-F6 |
| 12 | Random ticks: listed states with reasons, tests; grass modeled |
| 13 | `setblock ... strict` cited (`SetBlockCommand`, flags 2\|816); the removal of unsupported states justified (`updateShape` → `canSurvive`) and written to both worlds; the viewer's equality test defined |
| 14 | Doors (only `DoorBlock`), FORCED's flags, obfuscateSeed's code, sky sources reworded; the agreements cited; what this brief approves stated |

## Changelog (revision 3, the implementation; the G1 approval's nits)

| what | change |
|---|---|
| nit a | random ticks: the explicit list of the committed samples' randomly ticking states (`terrain info --ticking`) with reasons; leaves' decay argued from strict writes; the brown mushroom's light 1 cited (Blocks.java:1146) |
| nit b | the creeper's reach stated once: 7 to start, its explosion 10.4 for blocks, 12 for entities |
| nit c | §1's missing sky layer rule rewritten as implemented and probed; C16 reworded (the villager's eye cell; the zombie: no channel) |
| light | the initial map is the game's saved light (the game's runtime light is stale in 0.0075-0.04% of open cells); updates as `ChunkSkyLightSources.update` + the local fixed point |
| samples | the actual crops (village-edge 120 x 120 around (-372, 250); forest re-cropped around (-16, 0) for a clearing), the spot rule as `best_spot` (level ground, no trunk, no fluid, no structure piece) |
| placement | full-column clearing; the removal set is empty (attached states beside cleared cells are refused); blocks dug excludes cave air |
| table | 59 types (the sulfur cube and the bee added) |
| spawner | structure spawn overrides (`structures.tsv` from the game's data); `mobsAt`; the regional difficulty's special multiplier for cube-mob sizes |
| results | the Results section: light, paste, biomes, darkening, V1 (full dark: pass; despawn: match; dusk: one open difference) |
