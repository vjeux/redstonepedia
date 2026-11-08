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
their format, the natural block table, the light engine, the natural spawner, and the exposure
checks (channels C1-C16 below, reported on the card, never refusing: vjeux 2026-11-08, "it's all
about tradeoffs") with the API the mob-farm judge calls. The judge's wiring
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
2. **Nothing unmodeled decides a verdict silently**: the share of the ways a natural or village mob
   can affect a farm (the channels C1-C16) that are either modeled exactly or reported as an
   exposure on the card (which mob can do what, from which cells).

Reported beside them: the judge's added time per verdict, and the committed bytes per sample.

## Ceiling

Every cell equal (100%), every spawn test passing, every channel covered (16 of 16: modeled, or
reported as an exposure), no added run time for any farm (the checks cost seconds; the rate runs
skip the spawner, see D3), the committed samples a few hundred KB each. Each layout's estimate below
is given against it.

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
- **Placed light sources** (revision 5, vjeux: a day-1 player lights a dug-in farm): redsim models
  the blocks a player places for light as blocks of their own — `torch` and `soul_torch`
  (`TorchBlock`: no collision, on `canSupportCenter` of the block below, gone when it fails, washed
  away by fluids), `wall_torch` / `soul_wall_torch` (`WallTorchBlock`: on the sturdy face of the
  block behind), `soul_lantern` (`LanternBlock`'s rules), `jack_o_lantern`, `campfire` /
  `soul_campfire` (lit or not; a hay bale below makes a signal fire; a lit one is a burning block for
  spawning and paths), `end_rod`, the three froglights, `sea_pickle` (lit only waterlogged), `candle`
  (1-4, lit) — beside the ones redsim had (glowstone, sea lantern, shroomlight, magma, the lantern,
  redstone torches and lamps, a lit furnace). Their emission, dampening, shapes, solidity,
  conduction, sturdy faces, piston reaction and resistance are the game's own answers from the
  natural-state table (which holds every state of each); their survival rules are the game's code.
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
  `NoiseBasedChunkGenerator.getMobsAt` replaces the list inside a structure with its spawn override
  (`spawn_overrides` per category: the bounding box type `piece` or `full`, and the spawns). **All
  overrides are modeled**: `crates/redsim/src/terrain/structures.tsv` holds every structure's
  overrides from the game's data (`mctas terrain biomes`), the samples hold each structure start's
  pieces, and `natural::mobs_at` returns the override's list when the cell is in a piece (or the
  whole box, as the override says), else the biome's: trial chambers (all categories empty, by
  piece), swamp huts (witch and cat), outposts (pillagers, whole box), monuments, fortresses, ancient
  cities.
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
   (289 for one player in a real world). **redsim**: the chunks of that area that the terrain box
   loads (whole chunks inside it): 289 when the box covers the area (the cached full areas, where
   V1 validated the spawner); fewer in a crop (a 120 x 120 sample loads 7 x 7 = 49), so the global
   cap (`chunkCount / 289`) and the spawning chunks (only loaded ones are tried) shrink together and
   the mobs per loaded chunk stay those of the full area. Scope: V1 validated the spawner on the full
   area only; spawner rates in a crop are an extrapolation of it (no farm verdict uses them: the rate
   runs skip the spawner, D3).
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

**Draw for draw (revision 5, vjeux: "tick exact everywhere")**: every draw above comes from the
level random (`World::level_random`, mirrored from the bridge's sandbox seed; a stand-in random when
a world is not mirrored), in this order: the shuffle, per chunk and category the three
`getRandomPosWithin` draws, per group `ceil(nextFloat() * 4)`, per try the four `nextInt(6)`, the
type pick (`WeightedList.getRandom`: no draw for an empty list), the count sample (no draw when
constant), the spawn rules' draws (`Monster.isDarkEnoughToSpawn`: `nextInt(32)` then the dimension's
`monsterSpawnLightTest` sample; the bat's `nextBoolean` and `nextInt(4)`; slimes, drowned, glow
squids...), `snapTo`'s yaw, then `finalizeSpawn` per type (`natural.rs`'s `Finalize`): `Mob`'s
follow-range `triangle` (2 nextDouble) and left hand (nextFloat); zombies, zombie villagers and drowned:
the loot pick-up roll, the group's baby roll, a baby's chicken jockey (a new chicken's own
`finalizeSpawn` drawn inside the zombie's, the chicken entering first), the door-breaking roll,
`populateDefaultEquipmentSlots` (`Mob`'s armor: the chance, the material, the pieces; the zombie's
iron sword, spear or shovel; the drowned's trident or rod), the enchantment rolls and, when they
hit, `MOB_SPAWN_EQUIPMENT` (`EnchantmentsByCostWithDifficulty(#non_treasure, 5, 17)` ->
`selectEnchantment`, `entity/mob/enchant.rs`), the drowned's shell and nautilus rolls; skeletons:
armor, the bow's enchantment roll, the loot roll; spiders: the skeleton jockey (`nextInt(100)`, the
skeleton's own `finalizeSpawn`), the group's effect on Hard; cave spiders: none; slimes: the size;
sulfur cubes: `AgeableMob`'s baby roll (size 1 if a baby, else 2); squids and glow squids:
`AgeableMob` (0.05); sheep: the colour (`SheepColorSpawnRules`); pigs, cows, chickens: the variant and
the sound variant; horses: the group's variant, the markings, health, speed and jump; donkeys:
`AgeableMob` (0.2) and health; wolves: the pack's variant, the sound variant; cod and salmon: `Mob`'s;
the zombie horse: its zombie rider's `finalizeSpawn`. The draws from a mob's own random (a zombie's
`handleAttributes`, a salmon's variant, the constructors') come from its construction
(`World::construct`, the tick-exact lane). Model errors (never guessed): a drowned's zombie nautilus
(outside rivers), a zombie-horse rider's chicken, and `finalizeSpawn` of rabbits, foxes, turtles,
cats and bees (not in the samples' natural lists). In a bridge sandbox the spawner takes the game's
spawning chunks before the shuffle, its spawn chunk count and its entity-ticking chunks
(`Natural.spawn_order`, `spawn_chunk_count`, `entity_ticking`); a void sandbox's biome is plains and
its monsters are off (`Natural.void_biome`, `spawn_enemies`).

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
  them), `PatrolSpawner` (pillagers 24-47 blocks away in x and z, inclusive, never near villages), `CatSpawner` (cats in
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
| **L4** | **The game's ground, a natural-state table dumped from the game, light from the game's saved arrays with local updates after each change, static worst-case checks over every reachable position (C1-C16) reported as exposures, the spawner where it is the outcome** | 100% (test) | pass (validation) | 16 of 16 (each modeled or reported as an exposure) | + seconds (the checks; rate runs unchanged) | 381-698 KB per full-height sample (1.5 MB for three) | ~100% | a farm's exposure lines are worst cases, not measured losses, until natural-mob AI is modeled |
| L5 | L4 plus natural mobs' AI modeled exactly (goal selectors, `PathFinder` / `WalkNodeEvaluator`, targeting, attacks) | 100% | pass | 16 of 16, modeled (exposures turned into measured losses) | + hours (every mob pathfinds) | the same | ~100% | the mob-AI lane (8a0201dc); turns exposures into measured losses |

## Trade-offs

- **Fidelity vs size**: a full 17 x 17-chunk, full-height area is 28M cells (3.6 MB as a crop of the
  spawning area, 5-7 MB as the cached areas); a full-height crop of what the checks reach is 381-698
  KB. Crops are committed, full areas cached: generation is
  deterministic but the one-day settle is not (a re-made world's day-1 details differ: villagers,
  crops), so the committed crops, not the cache, are the reproducible record.
- **Fidelity vs judge time**: running the spawner in every rate run costs hours and changes nothing
  for a farm the checks clear; a proof (the checks) costs seconds.
- **Soundness vs acceptance**: static worst-case checks flag some farms that would work in
  practice (a zombie that could walk to a window but rarely would); they report it as an exposure
  (the player weighs it; vjeux 2026-11-08) rather than refusing or guessing a loss.
- **Exactness vs effort for natural states**: one variant + a table the game dumps (exact by
  construction, minutes per new state) vs hand-modeling ~100 states (weeks, error-prone).
- **Light timing**: `World::settle_light` between ticks (the changed region relit to the fixed
  point with its unchanged surroundings as the border) is exact for every read except in the tick of
  a nearby light change, which is reported (iron farms make no light-relevant change while running).

## Decision

**L4**, piece by piece:

- **Ground (A)**: the official server (L2-L5) because only the game's output is real (L1's
  hand-built world is what vjeux refused) and re-implementing generation is months; the bridge's
  sandboxes cannot generate (void levels).
- **Natural states (B)**: `Block::Terrain(u16)` with the dumped table, because hand-modeling is
  weeks and a guessed default (stone/air) fakes collisions, light and spawn floors.
- **Light (C)**: the game's saved arrays as the starting map (the game's own runtime light, stale in
  a few cells, which the fixed point would not reproduce) plus local updates after each change
  (`settle_light`), because the saved arrays alone (L2) are wrong after the paste and the per-query
  fixed point costs ~10^4 cells per read (spawning reads millions).
- **Spawning in the judge (D)**: static worst-case checks reported as **exposures** (revision 4,
  vjeux: "it's all about tradeoffs": never a refusal), because unmodeled walking makes any
  simulated run say nothing about safety (L1-L3) and modeling AI (L5, now staffed: the mob-AI lane)
  turns them into measured losses later; rate runs skip the spawner for **every** farm (natural
  mobs reach a farm only through the exposure channels, so they cannot change its measured rate
  except through what the exposure lines report): a farm with no exposure has an exact rate, a farm
  with exposures gets its rate as if unexposed plus the exposure lines (which mob, what, how many
  cells) as the caveat; the
  spawner runs where it IS the outcome (mob towers: their spawn rate on the
  tower's floors, an upper bound until a later brief models the walk-off) and in the validation.
  **Scope of the spawner's approval**: full-night and dusk rates (validated: Results V1; the dusk
  difference seen over the first 8 game nights (surface monsters 12.6 vs 16.2) did not hold over
  16 more: 17.6 vs 18.0, every type's Holm p >= 0.18) and the checks; tick-exactness against the
  deterministic bridge is the next bar (the tick-exact lane, 84579e98).
- **Extent (E)**: crops committed, full areas cached, because the checks reach a known distance
  (golem search 8, sight 15, follow box 18.84, creeper 7 + its explosion: blocks within 10.4,
  entities within 12) and the spawner's validation needs the full 128-block radius.

### Samples (pinned)

Seed **2** (`level-seed=2`, 26.3, default world). `/locate` from (0, 0): `village_plains` at
(-416, ~, 240), `forest` at (0, 64, 0) (recorded by `mctas terrain find --seed 2`).

| sample | generated area (cached) | farm spot | committed crop |
|---|---|---|---|
| `village-edge` (the day-1 set) | centre (-416, 240), radius 192 blocks (384 x 384 = 24 x 24 chunks, y -64..319) + 2 margin chunks, 1 game day run: world `2_-416_240_r192` | `place::best_spot` from (-352, 238), radius 14 (an 8 x 16 footprint, 5 layers below ground: min column (-360, 248), ground y 72, 656 blocks dug) | 120 x 120 columns around (-372, 250), the full height (698 KB): the bells (-411, 70, 241/249), 8 villagers in their beds, the iron golem, 2 cats |
| `plains` | the same area | from (-292, 222), radius 14 (8 x 16: (-306, 233), y 71, 580 dug) | 96 x 96 around (-280, 230), the full height (431 KB) |
| `forest` | centre (16, 0), radius 80 + 2 margin chunks, 1 day: world `2_16_0_r80` | from (-43, -6), radius 14 (8 x 16: (-43, -6), y 67, 643 dug) | 96 x 96 around (-16, 0), the full height (381 KB): forest and a river |

`redsim::terrain::samples::STANDARD` records each (file, seed, world and the `gen` line that
re-makes it, the search start). The spot rule (`place::refusal` + `best_spot`, deterministic, the
judge and the game check use the same), for a footprint of columns [x0..x1] x [z0..z1] whose box
reaches `below` layers under its ground layer:

- **Ground**: per column, the ground = the first free y above the highest block that is not a log,
  leaves or a block without collision (down from MOTION_BLOCKING_NO_LEAVES); the spot's ground G =
  the highest per-column ground; the spread (G minus the lowest per-column ground) must be <= 2. The
  design's ground layer sits at y = G, its box from G - `below` up; the cells of a lower column
  between its ground and the box's bottom stay natural (a gap of at most 2, or air), and the placed
  world, gaps included, is what every check judges.
- **Footprint cells** = every cell of the columns from G - `below` to the world top. Refused when:
  a structure piece's box meets the columns (above the box's bottom); a fluid cell is in a footprint
  cell, or right under the footprint (y = G - `below` - 1), or is the top MOTION_BLOCKING block of a
  footprint column; a tree trunk (`#logs`) is in a footprint cell (leaves are cleared with the site);
  **a natural state beside a footprint cell** (any of its 6 neighbours outside the footprint) is a
  fluid, or **has its support flag toward that cell**. The support flag is the game's own answer
  (the bridge's `blockinfo`, per state and direction): the state's `updateShape` changes when that
  neighbour turns from dirt into air (with dirt on its other sides), or the state cannot stand among
  dirt at all (then every direction is flagged). This catches by mechanism, not by a list, every
  state that pops off (plants on that side, wall torches, lanterns, vines, ladders, wall signs, bells)
  or reshapes (fences, walls, panes, bars, stairs, beds, chests, doors) — including light emitters,
  which a strict write would otherwise leave floating.
- Then the fewest blocks dug, then the nearest to the start.

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
`getIgniteOdds`: how fire spreads to and burns the state), **`ignited_by_lava`**
(`BlockState.ignitedByLava()`, the Properties flag `LavaFluid.randomTick` reads through
`isFlammable` / `hasFlammableNeighbours` to decide where lava starts a fire; it is set on blocks fire
can burn and on some it cannot, like signs, so it is a column of its own, not derived from the
odds), `blocksMotion`, `isPathfindable(LAND)`, `isAir`, `LeavesBlock` (the four heightmap
predicates: WORLD_SURFACE = not air, MOTION_BLOCKING = blocksMotion or a fluid, NO_LEAVES = that and
not leaves, OCEAN_FLOOR = blocksMotion), `isRandomlyTicking`, whether its class overrides
`updateShape` / `neighborChanged` / `tick` (reflection: the inert flag), the **support flags** (per
direction: whether the state's `updateShape` changes when that neighbour turns from dirt into air,
with dirt on its other sides, or all six when the state cannot stand among dirt at all), and **per (state, type)** for the
59 natural types (every type of the overworld biome lists, the village's and the custom spawners',
the sulfur cube and the bee: listed in the header) two bitsets: `isValidSpawn(level, pos, type)` and `type.isBlockDangerous(state)`.

### Light (C3)

- A **light-relevant change**: a cell whose state changes dampening, emission, occlusion faces or
  sky-source status. The map starts as **the game's own saved light** (the region files' arrays:
  the game's runtime light is not the engines' fixed point everywhere: in 0.005-0.05% of open
  cells, under leaves grown by a neighbouring chunk's decoration, the game keeps a stale 15, read
  back by `mctas terrain probe` from the running server, so the fixed point would be wrong there),
  and after changes it is updated like `LightEngine`: the column sky sources move as
  `ChunkSkyLightSources.update` moves them, and the region within 15 of the changes (and of the
  moved sources' columns) is recomputed as the fixed point with the cells around it as its fixed
  border (`World::settle_light`, run between ticks; `terrain/land.rs`).
- **Exact**: after every change the map is what the game has once its queued light work ran
  (validated: the iron-day1 farm pasted into the village sample in redsim and in the official
  server, 746,496 cells compared, 0 sky and 0 block light differences). **Reported**: a read (spawn
  check, daylight detector, sunburn) at a cell within 15 of a light-relevant change made in the same
  tick or the previous one is a model error that invalidates the run (as daylight.rs does today).

### The checks (D3): every channel, its rule

The **farm interior** = the cells of the design box that the farm's own entities, items and fluids
occupy in the judge's runs, and the air cells connected to them through cells with no collision
(the recorded envelope). Worst cases: difficulty Hard where it is worse (door breaking, spider
effects, babies), Easy where that is worse; any day time; thunder. The positions each mob can take
come from a **flood** per body class (the types' boxes; a position = a cell and the box anywhere in
it that fits, standing on a face with a collision top within step/jump/fall reach; climbing for
spiders; any air for flyers) over the **farm's surroundings** (`hostile::check`): the box of every
farm block and villager cell grown by 24 in x, y and z (clipped to the sample). **Shared floods**:
a flood shared by several types moves with the smallest width and the smallest height among them
(so it reaches every position any of them can) and tests reach and proximity with the largest box
among them at each position it reaches (`Body.reach_width/height`): zombie (zombie, drowned, husk,
zombie villager, witch: all 0.6 x 1.95), baby zombie (0.3 x 0.975, their babies), skeleton
(skeleton, stray, bogged: 0.6 x 1.99), pillager (0.6 x 1.95, its own), creeper, spider, cave spider,
cube (moves as a baby size-1 sulfur cube, 0.245; reaches as a size-4 slime, 2.08; slimes are never
babies), fox (moves as a baby, 0.3 x 0.35; reaches as an adult, 0.6 x 0.7), village villager (moves
as a baby, 0.3 x 0.975; reaches as an adult, 0.6 x 1.95), iron golem, enderman, bat — no flood uses a
box larger than one of its types, so no clearance is missed. It **starts** where a
mob of the body can appear in that region:

- every position on the region's **faces** (its 4 sides, its top and its bottom layer): a mob from
  farther away enters the region across a face, so a flood started on every face position sees
  every path that enters;
- every position where a natural type of the body can spawn **whatever the light**: its floor's
  `isValidSpawn` for that type (the table's per-type flag; zombie body: zombie, zombie villager,
  drowned, husk, witch; skeleton body: skeleton, stray, bogged; pillager; creeper; spider body:
  spider, cave spider; cube: slime, sulfur cube; fox; bat: bat, and only below the WORLD_SURFACE
  height);
- for the village's villagers and golem: the positions of the sample's own villagers and golem
  (they walk in from there or across a face);
- for endermen: every standable position (they teleport).

A position that is none of these (a sealed roof whose top is glass, a pocket no type can spawn in)
is reached only by walking from one that is, which the flood follows. Soundness: any mob in the
region either spawned at a start, stood there in the sample, or crossed a face, and the flood
covers every walk from those; every channel's reach is within 24 of the farm horizontally and
vertically (sight 15 (the pillager's, the longest), the golem sensor 17.84 + boxes, melee 0.83,
ranged 15, the golem search 8, a creeper's 7 to the AFK spot, beds 1, patrols are a separate rule).
Bound: ~120k cells x 13 floods (11 walkers and flyers, the village golem and villagers, the
endermen); ~6.5 s per placement measured on the full-height samples.

The result, `hostile::check(&world, &farm, village_golem) -> Vec<Exposure>`: one
`Exposure { channel, mob, what, cells }` per channel, mob and what it can do, with **every** cell it
can do it from (sorted), and `Exposure::line()`: "C3 sight: zombie can see a farm villager (it
panics, its sleep breaks) from 218 cells (e.g. (-343,72,257))". The judge's card prints one line per
sample (agreed with ironfarm): "exposures in village-edge (-360,248): C3 sight zombie 218 cells,
pillager 708; C4 golem sensor 1209; C5 baby zombie→villager 3; C7 golem spawns outside 158; …"
(channel, mob, cell count; the viewer paints the cells). **No exposure refuses a farm**: the farm's
rate is measured as if unexposed (rate runs skip the spawner for every farm) and its exposure lines
are the caveat beside it. (The spot rule's "refusals" are site selection: `best_spot` picks a spot
that passes it; a farm's verdict never refuses.)

| channel | exposure rule (a cell counts when) |
|---|---|
| C1 natural spawn inside | an interior cell passes ON_GROUND (or IN_WATER) placement for a listed type with block light 0 and `max(block, sky - 11) <= 7` (the gates' worst readings: `nextInt(32)` can always exceed sky light; `sample(0..7)` can be 7; darken up to 11 at night) and the type's box fits; or animal rules at brightness > 8 |
| C2 entry and contact | a position of any body class overlaps the interior (pushing, plates, tripwires, target blocks, `isUnobstructed` golem spots, beds, items) — doors a mob opens (villagers) or breaks (zombies on Hard) count as open |
| C3 sight | a hostile-sensor type's eye at one of its positions within its distance of a farm villager (standing or sleeping, any position in its pod) with a clear COLLIDER ray |
| C4 presence | an iron golem position (the village's golem: the golem flood) whose box meets a farm villager's box inflated by 17.84 on all three axes (`GolemSensor` reads `NearestLivingEntitySensor`: the follow range 16 plus `finalizeSpawn`'s bonus, no sight: the farm's villagers stop spawning golems for 600 ticks); or a village villager position (the villager flood) whose box meets a farm villager's box inflated by 10 (it joins their count in `Villager.spawnGolemIfNeeded`) |
| C5 melee and ranged reach | **melee** (the attack box: the attacker's box inflated by sqrt(2.04) - 0.6 horizontally): the zombie family (zombie, baby zombie; drowned and husks share the body) against a farm villager (`Zombie` targets `AbstractVillager`); every hostile walker but the creeper (zombie, baby zombie, skeleton, pillager, spider, cave spider, cubes) against the farm's golems — either attack box meeting the other's box (`IronGolem` targets every `Enemy` but creepers; zombies and spiders target golems; spiders do not target villagers); the village golem against the farm's zombie or its vehicle. **Ranged** (a clear COLLIDER ray from the attacker's eye within range): a skeleton against a farm golem's eye (15; skeletons target players, golems and turtles), a pillager (its own body) against a farm villager's eye (8, its crossbow), a drowned (the zombie body) throwing a trident at a farm villager (10). Witches target players and raiders only (no channel) |
| C6 pickup | a looting mob's position outside the interior whose pickup box (its box inflated by 1, 0, 1: `getItemPickUpReach`) meets an interior cell — the zombie family and its babies, skeletons, pillagers, the village's villagers, foxes (each its own flood; the shared ones move as their smallest member and reach as their largest, see the floods). A superset: every interior cell counts as a cell the farm's items may occupy; it narrows to the cells the farm's items occupied in the runs once the farm spec carries them (ironfarm), a later brief |
| C7 golem search | `LEGACY_IRON_GOLEM` from a summoning farm villager (`Farm.summoners`: the villagers that can see a farm zombie, since only a panicking villager calls `spawnGolemIfNeeded`; empty: every farm villager) finds, in any of its 17 x 17 columns, a first matching cell outside the farm's own spawn floors that passes `checkSpawnObstruction` (exhaustive, deterministic) |
| C8 creepers | a creeper position within 7 of the AFK spot (a creeper explodes at a player; no ray test, a superset) whose power-6 explosion reaches the farm: a farm block within 10.4 (the blocks the blast can break at the worst factor 1.3) — "can blow up farm blocks"; or a farm entity (a villager, the zombie, a golem at a golem cell) within 12 (the entity radius: damage and knockback, exposure taken as 1) — "can hurt a farm villager, zombie or golem" |
| C9 endermen | from an enderman position (every standable position: they teleport): **take** — a farm block in `ENDERMAN_HOLDABLE` within the take box (x and z within 2, 0..3 above: `EndermanTakeBlockGoal`); **place** — a farm air cell (an interior cell holding air) within the place box (x and z within 1, 0..2 above: `EndermanLeaveBlockGoal`), the carried block taken anywhere outside; both supersets (no OUTLINE ray, no test of the block under the placed one) |
| C10 flyers | a bat position overlapping the interior (the bat flood: reported as C2 entry); **phantoms**: the AFK player's cell is at or above the sea level (63) and `canSeeSky` (sky light 15) — `PhantomSpawner` then spawns phantoms 20-34 blocks above it once the player has not slept for 72000 ticks (the AFK player never sleeps): reported at the AFK cell |
| C11 fire | **lightning**: a farm block with `burn_odds > 0` whose top cell `canSeeSky`; **lava's fire** (`LavaFluid.randomTick` with `doFireTick`): from every lava cell within 4 of a farm block, up to 3 steps up (each +1 y and -1..1 in x and z) through cells that do not block motion, the first air cell with a neighbour that is `ignited_by_lava` catches fire; or, with no step, a cell of the 3 x 3 at the lava's level that is `ignited_by_lava` with air above takes fire above it — reported when the burning neighbour is a farm block or the fire cell is in the interior. Fire spreading on from there is not followed (the first ignition already exposes the farm) |
| C12 sculk | a sculk sensor of the design within 8 of any mob position (their steps are vibrations) |
| C13 bed claims | a village villager position from which a farm bed head is within `AcquirePoi`'s reach (a path ending within 1 block) |
| C14 raids, sieges, patrols | sieges' zombies are walkers (C2-C9 cover them); raids need a Bad Omen player: the judge's AFK player has none (no channel); **patrols** (`PatrolSpawner`, from day 5, during the day): none while the AFK player's section is within 2 sections of a village center (`ServerLevel.isCloseToVillage(pos, 2)` = `PoiManager.sectionsToVillage`: the 26-neighbour section distance to a section with an occupied `#village` POI — the farm's own claimed beds count; redsim counts bed heads and bells, not job sites, the side that never hides a patrol); else every cell a patrol leader can take 24-47 blocks out in x and z on the MOTION_BLOCKING_NO_LEAVES surface (`isValidEmptySpawnBlock`, block light <= 8, its floor's `isValidSpawn` for a pillager) is reported (its members then walk: C2-C9 from the region's faces) |
| C15 light and caps | natural mobs emit no light (no channel); they fill the mob cap, which matters only to natural-spawn farms, whose rate is the spawner's |
| C16 lightning on the farm's mobs | a farm villager whose eye cell `canSeeSky` (sky light 15: lightning there turns it into a witch); the zombie's own exposure changes nothing (no channel): the iron-farm roof must-have, reported here as an exposure |

A farm with no exposure in C1-C16 has no channel to any natural or village mob, so its rate runs
skip the spawner exactly. Each exposure names the channel, the mob, what it can do, and every cell
it can do it from (`hostile::Exposure`, its card line `Exposure::line`); the rule column above says
when a cell counts.

### The `.mcterrain` format (version 1)

Little-endian, no compression inside, decoded by pure-Rust redsim code (no dependencies, builds for
wasm): magic `MCTERRv1`, tagged sections (HEAD: seed, zoom seed, world spawn, min y, height, game
and day time, origin, size; PALT + CELL: palette and u16 cells, x fastest then z then y, in varint
runs; BPAL + BIOM: biome palette and the 4x4x4 cells the box reads; HMAP: four heightmaps; SKYL,
BLKL: saved light in runs; STR2: structure starts (id, box, pieces; STRU without pieces is still
read); ENTS: entities (id, position, SNBT); BENT: block entities (cell, SNBT: the beds, bells and
chests the tick-exact sandbox restores); INHB: each chunk's `InhabitedTime`; NOTE: provenance).
**Size** (measured): village-edge
120 x 384 x 120 = 5,529,600 cells (the full height, revision 4: natural spawning and the light
margin under a dug-in farm need the whole column), 703 KB; plains 96 x 384 x 96, 433 KB; forest
96 x 384 x 96, 382 KB (1.5 MB in git). Full areas (40M+ cells, 5-7 MB) stay in the cache. A crop
smaller than the 17 x 17 chunks around the AFK spot loads only some of the spawning chunks: the
mob cap counts the loaded ones (`getNaturalSpawnChunkCount`), so the mobs per chunk are the
game's; tick-exact spawning needs the whole 272 x 272 area (the cached world).

## Must-haves

- The terrain equals the game's (blocks, biomes, heightmaps, light, entities) cell for cell; the
  table equals the game's re-dump.
- The design placed like a player builds: footprint cleared, outside natural; no unsupported
  natural state remains (a natural state whose shape depends on a footprint cell, by its support
  flag toward it, refuses the spot); the game check writes the same cells.
- Light starts from the saved arrays and is exact after every change (the paste check), reads in
  the timing window reported.
- Natural spawning and despawning rule for rule (sections 4-6), the game's unmirrorable draws from
  the seeded stand-in with the same distribution (the mob layer's policy).
- Difficulty Normal for rates (the singleplayer default); exposures at the worst difficulty per rule.
- Every exposure names channel, mob, what, and its cells.
- The viewer runs the same world as the judge: the world hash (blocks and entity positions) of the
  viewer's wasm run equals the native run's every 20 ticks over one game day (mobviewer's test on
  the shared `farm::Run`).

## Not goals

- Generating terrain in Rust; other dimensions.
- Natural mobs' AI (walking, targeting, attacks): reported by C1-C16 as exposures instead (the
  mob-AI lane, 8a0201dc, models it next); so V2 (the
  validation with mobs walking in the game) is not run: there is nothing walking in redsim to
  compare, and the checks cover every walk.
- Tick-exact natural spawning (the next bar: the mirrored level random, with the tick-exact lane).
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
   (0 diffs); the **initial** light map is the saved arrays (0 diffs by construction); the light
   **after changes** is the criterion: a design pasted in redsim and in the official server (the same
   strict writes), every block and light value of the box compared (0 diffs; a diff is a model bug).
   The engines' fixed point vs the saved arrays is reported per sample as an explained deviation (the
   game's own stale light), not as the criterion.
2. **Natural spawning, V1 (as run)**: the area around the village-edge spot (cached, full height),
   the spot as the AFK spot. The game: the official server on that world, the bot (creative) at the
   spot, simulation and view distance 10, Normal, clear weather, `spawn_mobs` on; a one-line data
   pack (written by Rust) sets every mob's movement speed to 0 each tick (nothing walks, as in
   redsim); after the night the world is saved and the mobs read back from the region files. redsim:
   the same world and spot, 32 seeds of the stand-in random. Nights: 300 ticks at full darkness (day
   18000, 6 game nights) and 1200 ticks from dusk (day 13000, 8 game nights); the despawn curve with
   spawning stopped after 300 ticks; per-100-tick counts over RCON (`--trace`). Tests: the mobs alive
   at each night's end per type (Welch, Holm across the types), the surface/cave split and height
   bands, the mean height and distance. Pass = no Holm rejection; a rejection is a model bug, fixed
   before the spawner's push.
3. **V2 (walking on in the game)**: not run — natural mobs' walking is not modeled, so there is no
   redsim walk to compare; the worst-case checks (C1-C16) cover every walk.
4. **The checks' fixtures** (`hostile.rs`'s tests, each on `village-edge`): F1 a closed dark
   5x5 room (a C1 exposure), F2 a villager pod with an open side 6 blocks from grass (C3), F3 villager
   pods at ground level (C7: grass columns), F4 an AFK spot on open ground 3 blocks from the farm
   (C8), F5 a bed reachable from the village (C13), F6 a sealed, lit box with three podded
   villagers 25 blocks above the ground and its AFK spot inside (no exposure at all).

### Results (revision 3, 2026-11-08)

Commands: `mctas terrain light|paste-check|biome-check|probe|spawn-check|hostile|spot`.

1. **Terrain and light** — the files are the region files read back (blocks, biomes, heightmaps,
   saved light, structure pieces, entities). The initial map is the saved arrays. The engines' fixed
   point vs the saved arrays (an explained deviation, not the criterion): block light 0 differences
   over the full areas; sky light, open cells: the village area 1,521 of 32,674,884 (0.0047%), the
   forest area 1,428 of 4,210,998 (0.034%); inside the committed crops (16 columns in from their
   edges) village-edge 0 of 148,969, plains 0 of 78,770, forest 61 of 119,772 (0.051%) — all where
   the game's own light is stale under leaves (probed in the running server: it reads the saved
   value). (A crop's bottom layers show block light from sources under the crop: the reason relight
   refuses a change whose region reaches the box's bottom or sides.) **Paste check** (`terrain paste-check`, the criterion after changes): the archived
   iron-day1 pasted at (-335, 255) of the village sample in redsim and in the official server (the
   same 639 strict writes, 454 blocks dug): 746,496 cells, **0 block, 0 sky, 0 block-light
   differences**. **Placed light sources** (revision 5): `designs/terrain/fixtures/lights.txt` (every
   light source above on a stone floor) pasted at (-306, 233) of the same sample in redsim and in the
   official server: 331,776 cells, **0 block, 0 sky, 0 block-light differences**. **Biomes** (`terrain biome-check`): redsim's `biome_at` vs `execute if biome` at 300
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
     significant after Holm (lowest 0.73), but the surface share at the end looked different:
     surface monsters 12.6 vs 16.2 per night, sulfur cubes 5.5 vs 2.8 (raw p 0.06). **16 more game
     nights** (the re-made world, 32 redsim seeds): surface monsters 17.6 vs 18.0 per night (creeper
     5.9 vs 4.6, skeleton 4.1 vs 5.1, spider 3.6 vs 2.8, zombie 3.5 vs 5.3), the 64..79 band 15.6 vs
     15.4, every type's p(Holm) >= 0.18 (lowest raw: glow squid 0.008, sulfur cube 0.027, zombie
     0.038): the first 8 nights' difference did not hold; **pass**. The game's per-100-tick trace
     (two nights) shows the replacements' surface share at 37-45%, redsim's ~45%.
   Found after: the spawn box of cube mobs (`EntityType.getSpawnAABB` scales by
   `spawnDimensionsScale`: slime and magma cube 4, sulfur cube 2) — redsim used the base box, so it
   let them spawn in tight cells; fixed (revision 4: the game's own method and scales, approved by
   construction) and **re-run** at full darkness with the fix (8 game nights, 32 redsim seeds):
   sulfur cubes 3.25 vs 2.66 per night (p 0.37), slimes 0.00 vs 0.03, monsters 70.50 vs 71.00 (p
   0.18): **pass**; the tick-exact compare will confirm it draw for draw.
   Fixed during validation: `noActionTime` counted twice (the mob layer's `serverAiStep` already
   adds 1), the slime/sulfur-cube size drawn before `isValidPositionForMob` (the game checks the
   size-1 box), structure spawn overrides (trial chambers: no natural spawns in their pieces), the
   sulfur cube and the bee added to the table's types.
3. **V2 (walking)**: not run (natural mobs' walking is not modeled; the checks cover it).
4. **The checks**: `terrain hostile` on the archived iron-day1 at the village edge (an open void
   design) reports exposures on C1-C7 and C9 (revision 4d: e.g. C3: zombies see a farm villager
   from 193 cells, pillagers from 676; C4: the village golem within the golem sensor from 1,109
   cells, village villagers within 10 from 445; C5: baby zombies hit a villager from 3 cells; C6:
   the zombie family, skeletons, pillagers, foxes and village villagers can pick up an item inside
   from 60-95 cells (villagers 61); C7: golems can spawn outside the floors at
   158 cells; C9: endermen can place a block into a farm air cell from 204), as it must; ~6.5 s per placement
   (revision 4c: floods over the farm's box grown by 24 in x, y and z, the full-height sample).
5. **Tick-exact spawning against the deterministic bridge** (revision 5; the tick-exact lane's
   `mctas mobfarm compare`, every tick: the level random, randValue, every entity): the **void
   gate** (d1-bed-plate, plains void, Hard, monsters off, AFK 60 under the chest): the spawner's
   draws equal the game's through game time 200 (the shuffle of 193 chunks and every
   `getRandomPosWithin`); the first difference moved from t1 (1,520 calls short) to t201, and the
   bridge's draw log shows it is the villagers' brains (mob-AI's), not the spawner. **Terrain**
   (d1-bed-plate in village-edge, Hard, monsters on): with the sandbox's structures off on both
   sides the spawner's tokens (positions, groups, steps, type picks) are identical through 310
   positions at t1; the open sandbox differences found (the tick-exact lane's to fix): the sandbox's
   structures (now imported), its entity-ticking chunks captured before t1 (the game spawns in more
   chunks), its sea level (FlatLevelSource's -63 instead of the world's 63: no glow squid, drowned or
   surface-water rule can match). The statistical V1 (above) stays the check against the unmodified
   server.

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
| light | the initial map is the game's saved light (the game's runtime light is stale in 0.005-0.05% of open cells); updates as `ChunkSkyLightSources.update` + the local fixed point |
| samples | the actual crops (village-edge 120 x 120 around (-372, 250); forest re-cropped around (-16, 0) for a clearing), the spot rule as `best_spot` (level ground, no trunk, no fluid, no structure piece) |
| placement | full-column clearing; the removal set is empty (attached states beside cleared cells are refused); blocks dug excludes cave air |
| table | 59 types (the sulfur cube and the bee added) |
| spawner | structure spawn overrides (`structures.tsv` from the game's data); `mobsAt`; the regional difficulty's special multiplier for cube-mob sizes |
| results | the Results section: light, paste, biomes, darkening, V1 (full dark: pass; despawn: match; dusk: one open difference) |

## Changelog (revision 3b, after the focused re-review by d60ee61d)

| point | change |
|---|---|
| 1 | the spot's neighbour refusal is defined by mechanism: a natural state beside any footprint cell (6 neighbours) refuses when its **support flag** toward that cell is set — the game's own `updateShape` changes it when that neighbour turns from dirt into air, or it cannot stand among dirt (then all six); the table gained the `support` column (the bridge's `blockinfo`) and `place::refusal` uses it (the class list is gone) |
| 2 | Must-haves: no unsupported natural state remains (refused by the support flags), the removal step is gone |
| 3 | Validation §1: the initial map is the saved arrays (0 by construction); the criterion after changes is the paste check vs the game (0 diffs over 746,496 cells); the fixed-point comparison reported per sample (village area 0.0047%, forest area 0.034%, crops 0 / 0 / 0.051%) as an explained deviation; the caches were re-made on a new box (the re-made worlds' day-1 details differ: villagers, crops; the committed crops were re-cut from them, the same spots) |
| 4 | V2 not run (nothing walks in redsim; the checks cover every walk) in Validation and Not goals; the dusk observation scoped in the Decision (the spawner approved for full-night rates and the checks, not dusk rates) and Not goals |
| 5 | §3: every structure spawn override is modeled (`structures.tsv`, the pieces, `natural::mobs_at`), none refused |
| 6 | the checks' flood: its actual region (24 columns around the farm's blocks and villagers, the whole height), why it suffices (the region's ring is all starts; every channel's reach is within 24) and its measured cost |
| 7 | the spot rule written exactly: the per-column ground, the spot's ground G and the ≤2 spread, the ground layer at G, the gaps judged as placed; the fluid rules (in a footprint cell, right under the footprint, a column's top MOTION_BLOCKING block, beside a footprint cell) |
| 8 | Size: the measured crops; L4 and the Decision's light: saved start + local updates; Trade-offs: `settle_light` between ticks |

## Changelog (revision 4, 2026-11-08: vjeux's rulings "it's all about tradeoffs", "tick exact everywhere")

| part | change |
|---|---|
| the checks | report **exposures**, never refuse: `hostile::check` returns `Exposure { channel, mob, what, cells }` with every cell; the card prints one line per sample (agreed with ironfarm: "exposures in village-edge (-360,248): C3 sight zombie 218 cells, pillager 708; …"); the mob-AI lane (8a0201dc) turns them into measured losses |
| channels added | C4 village villagers within 10 of a farm villager; C5 ranged (skeleton → farm golems 15, pillager → villagers 8, drowned tridents 10); C5 melee is the zombie family's against villagers (spiders target players and golems only) and any hostile but creepers against the farm's golems; C10 phantoms over an AFK cell that sees the sky; C11 lava's fire (`LavaFluid.randomTick`, the table's new `ignited_by_lava`); C12 sculk sensors |
| flood starts | where a mob can appear: the region's outer ring, positions a natural type of the body can spawn at (its `isValidSpawn` floor) whatever the light, the sample's own villagers and golem, every standable position for endermen, air below the surface for bats (was: every standable position, which put villagers on sealed roofs) |
| samples | the full height (y -64..319; 698 / 431 / 381 KB): natural spawning and the light margin under a dug-in farm need the whole column (ironfarm's D1U) |
| spawner | the mob cap counts the loaded spawning chunks (a crop keeps the game's mobs per chunk); the spawn box is `getSpawnAABB` (cube mobs scaled 4 / 2); V1 dusk: 16 more game nights, pass |

## Changelog (revision 4b, after d60ee61d's re-review of revision 4)

| point | change |
|---|---|
| 1 | refuse language gone from Layouts (L4: "each modeled or reported as an exposure"; L5: "turns exposures into measured losses"), Ceiling ("every channel covered: modeled or reported"; no added time for any farm), C16 ("reported here as an exposure"), the fixtures ("a C1 exposure", "no exposure at all"); the spot rule's refusals stated as site selection, the farm's verdict never refuses |
| 2 | § Checks: the flood's actual region (the farm's box grown by 24 in x, y, z), its starts (the region's faces, positions a natural type of the body can spawn at whatever the light, the sample's own villagers and golem, every standable position for endermen, air below the surface for bats) and the soundness argument (any mob in the region spawned at a start, stood there, or crossed a face; every channel's reach within 24 horizontally and vertically) |
| 3 | C5: per-attacker targets (melee: the zombie family vs villagers, every hostile but creepers vs the farm's golems both ways, the village golem vs the zombie; ranged: skeleton → golems 15, pillager → villagers 8, drowned trident → villagers 10; witches: no channel) |
| 4 | C10 phantoms (the AFK cell at or above y 63 and `canSeeSky`; `PhantomSpawner`) and C14 patrols (`PatrolSpawner`; `isCloseToVillage(pos, 2)` = `PoiManager.sectionsToVillage`; the cells 24-47 out) in the channel table; the code's phantom rule gained the sea-level condition |
| 5 | the table section defines `ignited_by_lava` (`BlockState.ignitedByLava`, what `LavaFluid.randomTick` reads, why it is its own column); C11 states the lava rule with it |
| 6 | § Mechanics 1: redsim's `chunkCount` = the loaded spawning chunks, the per-chunk argument (the cap and the tried chunks shrink together), and the scope (V1 validated the full area; crop spawner rates are an extrapolation, used by no farm verdict) |
| 7 | the cube spawn box: approved by construction (the game's `getSpawnAABB`, scales 4 / 2) and re-run at full darkness with the fix: sulfur cubes 3.25 vs 2.66 (p 0.37), slimes 0 vs 0.03, monsters 70.50 vs 71.00: pass |
| 8 | § Checks carries the API (`hostile::check -> Vec<Exposure>`, `Exposure { channel, mob, what, cells }`, `line()`) and the agreed card line; Decision D: rate runs skip the spawner for every farm, a farm with exposures gets its rate as if unexposed with the exposure lines as the caveat |
| 9 | Layouts' committed sizes (381-698 KB per full-height sample, 1.5 MB for three), Trade-offs' sizes; C8 and C9 state their implemented supersets (no ray tests); C6 pickup marked not implemented; C7 uses the summoning villagers (`Farm.summoners`, ironfarm) |

## Changelog (revision 4c, after d60ee61d's re-review of revision 4b)

| point | change |
|---|---|
| 1 | C6 pickup implemented as a superset (code and table): a looting mob outside the interior whose pickup box (box inflated 1, 0, 1) meets an interior cell; the narrowing to the items' cells once ironfarm's farm spec carries them is a later brief |
| 2 | C8: both cases (code and table): a farm block within 10.4, or a farm entity (villager, zombie, golem cell) within 12, of a creeper within 7 of the AFK spot |
| 3 | C9: take and place (code and table): place = a farm air cell within x/z 1 and 0..2 above an enderman position, the carried block taken anywhere |
| minor | Results §4: the revision-4c measurement (~5.6 s, the farm's box grown by 24); the patrol range 24-47 inclusive in § Mechanics too |
| 4d | the shared-flood rule (d60ee61d): a flood shared by several types moves with the smallest width and height among them and reaches with the largest (`Body.reach_width/height`); foxes and pillagers get their own floods; the cube flood moves as a baby size-1 sulfur cube (0.245) and reaches as a size-4 slime (2.08); the village villagers' flood moves as a baby (0.3 x 0.975) and reaches as an adult; every flood listed in § Checks with its types — none uses a box larger than one of its types (~6.5 s per placement) |

## Changelog (revision 5, 2026-11-08: model extensions, no judge-rule change)

| part | what changed |
|---|---|
| § 2 Light | placed light sources (torches, wall torches, soul lantern, jack o'lantern, campfires, end rod, froglights, sea pickles, candles) as redsim blocks; Results §1: their paste check, 331,776 cells, 0 diffs |
| § 4 Natural spawning | "draw for draw": every spawner draw on the mirrored level random in the game's order, `finalizeSpawn` per type, the spawn equipment's enchantments (`MOB_SPAWN_EQUIPMENT`), the model errors left, the bridge sandbox inputs |
| format | `.mcterrain` BENT (block entities) and INHB (inhabited time) sections; the samples re-cut so the cache, the kept world and the crops are one saved state |
| Validation 5 | the tick-exact compare's results (void gate: the spawner exact through t200; terrain: identical through 310 positions with structures off) and the sandbox differences left to the tick-exact lane |
