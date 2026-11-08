# Iron farms (design doc, gate G1)

ironfarm track, 2026-11-08. Gate G1 ([docs/process.md](../../docs/process.md)). Asked by vjeux on
2026-11-07: "I want you to build iron farms. Should be a mix of day 1 farm, good middle ground and
fastest scaled one", "as you go build the infrastructure to build and manage farms using redsim and
whatever else is needed. I will be asking you to build all the top ones in the future so lay out
the foundations for it", "Please implement everything you need on redsim. We can't use the game
code, it's way too slow in java". Every rule below was read in the decompiled 26.3 server
(`mctas src show <Class>`); no outside design was used. The work log is
`designs/circuits/mobfarm/tracks/ironfarm/NOTES.md`.

## Use

A survival player builds an iron farm for iron ingots without mining (hoppers, rails, anvils,
iron blocks for beacons). They AFK near it, and every few minutes they empty a chest. The
farm needs villagers, beds and a zombie, which they bring in by hand.

## The number

1. **Iron ingots per hour** at vanilla speed for the whole farm (golems per hour x 4, the
   mean drop). It is a mean over K seeds x D in-game days, with its spread. Per module for a
   tiled farm.
2. **The bill**: the iron the build itself costs (hoppers 5, a piston 1, a minecart 5, a
   bucket of lava 3, tripwire hooks 1 per 2...), plus the other survival materials (villagers,
   beds, the zombie, quartz, string). **Payback**: hours of output to repay the iron.
   **Blocks** placed.

Reported too: golems per hour, poppies per hour, seconds per golem (the cycle), the share of
the ceiling, and the losses by cause (an attempt that found no spot, a cycle stretched by a
golem the villagers saw, nights without sleep).

## Mechanics, from the game code (26.3)

### Who spawns a golem and when

- **The trigger is the panic behavior.** `VillagerGoalPackages.getCorePackage` puts
  `VillagerPanicTrigger` in the CORE activity, which is always active. `VillagerPanicTrigger`
  is a `Behavior` with the default 60 gt duration. `tryStart` needs `isHurt` (memory
  `HURT_BY`) or `hasHostile` (memory `NEAREST_HOSTILE`). `start` makes PANIC the active
  activity. `tick` runs `if (timestamp % 100L == 0L) body.spawnGolemIfNeeded(level, timestamp, 3)`.
  `Behavior.tickOrStop` stops it once `timestamp > endTimestamp`, and it starts again the
  next tick, so one tick in every 62 has no `tick` (gt `t0 + 61 + 62k`). When that tick is a
  multiple of 100, the attempt is lost.
- **`Villager.spawnGolemIfNeeded(level, timestamp, 3)`:** if `wantsToSpawnGolem`, it lists the
  villagers whose box meets this villager's box inflated by 10 (`getEntitiesOfClass`, itself
  included). It keeps those that `wantsToSpawnGolem`, at most 5. With 3 or more it calls
  `SpawnUtil.trySpawnMob(IRON_GOLEM, MOB_SUMMONED, level, blockPosition(), 10, 8, 6,
  Strategy.LEGACY_IRON_GOLEM, false)`. On success, EVERY villager in the box (not only the
  willing ones) gets `GolemSensor.golemDetected`: `GOLEM_DETECTED_RECENTLY` with a 599 gt
  expiry. (`Villager.gossip` calls it too, needing 5 willing villagers. That path is
  refused: see the judge.)
- **`wantsToSpawnGolem` = `golemSpawnConditionsMet && !GOLEM_DETECTED_RECENTLY`**.
  `golemSpawnConditionsMet` holds when the memory `LAST_SLEPT` exists and `gameTime - lastSlept
  < 24000`. **Without a sleep in the last game day, a villager never wants a golem.**
- **Memory expiry order** (`Brain.tick` = `forgetOutdatedMemories`, `tickSensors`,
  `startEachNonRunningBehavior`, `tickEachRunningBehavior`; `MemorySlot.tick`: cleared when
  `timeToLive <= 0`, else decremented). A memory set at tick T with 599 is gone at the
  start of T+600's brain tick for villagers that ticked before the spawner, and at T+599 for
  those after it. The next panic tick at a multiple of 100 is T+600. **One golem per 600 gt
  per villager cluster is the ceiling** (below).
- **The golem detector.** `GolemSensor` (scan rate 200) reads `NEAREST_LIVING_ENTITIES`. If any
  entity in it is an iron golem, it sets the same 599 gt memory. `NearestLivingEntitySensor`
  (rate 20) stores every living entity (`isAlive`) whose box meets the villager's box inflated by
  its `FOLLOW_RANGE`, sorted by distance. The stored list is what the golem sensor reads, so a
  golem alive and in range at the last 20 gt scan counts, even if it died since. The villager's
  follow range is 16 times (1 + b). `Mob.finalizeSpawn` draws the bonus
  `b = random.triangle(0, 0.11485)`, so each villager has a range in 14.16..17.84. A golem
  that the sensor sees between T and T+600 pushes the next attempt to the first multiple of
  100 after its own 600 gt. **The golem must leave every villager's box (or die) before
  their scans see it.**
- **Sensor phases.** `Brain`'s constructor calls `Sensor.randomlyDelayStart(body.getRandom())`
  for each sensor in registration order (`Villager.BRAIN_PROVIDER`: nearest living, players,
  items, bed, hurt by, hostiles, babies, secondary POIs, golem). That is `nextInt(scanRate)`
  from the entity's own random. `Sensor.tick` runs `doTick` when `--timeToTick <= 0`, then waits
  `scanRate`.
- **The hostile sensor.** `VillagerHostilesSensor` (rate 20) reads `NEAREST_VISIBLE_LIVING_ENTITIES`.
  It takes the nearest hostile within its type's distance: zombie, zombie villager, husk,
  drowned and vex 8; vindicator and zoglin 10; evoker, illusioner and ravager 12; pillager 15.
  The distance is `mob.distanceToSqr(body)` from feet to feet. The hostile must also pass
  `Sensor.isEntityTargetable`: `TargetingConditions.forNonCombat` with the follow range,
  `getVisibilityPercent`, and `Mob.getSensing().hasLineOfSight`. That is
  `LivingEntity.hasLineOfSight`, a `ClipContext.Block.COLLIDER` / `Fluid.NONE` ray from eye to
  eye, at most 128 blocks. `NearestVisibleLivingEntities` evaluates it lazily and caches it per
  entity. Glass blocks the ray (it has a collision box); water does not.

### Sleep: the condition every 26.3 iron farm has to meet

- **A panicking villager never sleeps.** `SleepInBed` (the only place `LAST_SLEPT` is set) is in
  the REST package. While PANIC is active, the schedule is not consulted: only
  `VillagerCalmDown` (in the PANIC package) calls `updateActivityFromSchedule`, and only when
  the villager is not scared (no `HURT_BY`, no `NEAREST_HOSTILE`, no `HURT_BY_ENTITY` within 6).
  **A zombie in permanent view means no sleep, so the farm stops spawning one game day after
  the villagers last slept.** Every farm needs a calm moment each night: the zombie hidden
  from every villager's eyes during one hostile scan (at least 20 gt to catch every phase)
  within the REST window.
- **The schedule** (`Timelines`: `VILLAGER_SCHEDULE`, adult track): day time 10 IDLE, 2000 WORK,
  9000 MEET, 11000 IDLE, 12000 REST. `Brain.updateActivityFromSchedule` runs at most once per
  21 gt (`gameTime - lastScheduleUpdate > 20`). `setActiveActivityIfPossible` falls back to IDLE
  when the activity's requirements fail (WORK needs `JOB_SITE`).
- **`SleepInBed`** (REST, priority 3) starts only under all of these:
  - the villager is not a passenger;
  - it holds a `HOME` memory;
  - it was not woken 1..99 gt ago (`LAST_WOKEN`);
  - the HOME position is `closerToCenterThan(position, 2.0)`;
  - the bed is not `OCCUPIED`.

  `start` (when `timestamp > nextOkStartTime`) calls `startSleeping(home)` and sets
  `LAST_SLEPT = timestamp`. It stays running while REST is active, `y > bed.y + 0.4` and the
  bed is within 1.14. `stop` calls `stopSleeping` and `nextOkStartTime = t + 40`. `WakeUp`
  (CORE) stops the sleep when REST is not active. A villager that calms down at night therefore
  falls asleep in the same brain tick (CalmDown at priority 0 switches to REST before priority
  3 starts). It only needs to start sleeping once per game day: a panic right after waking
  it changes nothing.
- **`LivingEntity.startSleeping`**: position at the bed + (0.5, sleep height 0.5625 + 0.125,
  0.5), `OCCUPIED`, pose SLEEPING (0.2 x 0.2, eye 0.2), motion zero. **`stopSleeping`**: bed
  free; position from `AbstractBedBlock.findStandUpPosition`, the cells around the bed, safe
  ones first, else the cell above the bed (+0.1); pose STANDING. A villager that wakes can
  end up somewhere else in its pod.
- **Claiming the bed** (`AcquirePoi` for `HOME` in CORE) needs a path
  (`AcquirePoi.findPathToPois`). See "Not modeled" below.

### Where a golem spawns

- **`SpawnUtil.trySpawnMob`**: 10 attempts. Each draws `dx, dz = Mth.randomBetweenInclusive(
  random, -8, 8)` from the LEVEL random and starts at (villager block + (dx, 6, dz)).
  `moveToPossibleSpawnPosition` walks down from y+5 (with the cell above it, y+6) to y-7. It
  stops at the first cell whose block passes `LEGACY_IRON_GOLEM`:
  - not cobweb, cactus, glass, tinted glass, stained glass, any glass pane, leaves, conduit,
    ice, frosted ice, TNT, glowstone, beacon or sea lantern;
  - the block above is air or a liquid;
  - the block `isSolid()` (or is powder snow).

  The golem goes in the cell above. `BlockStateBase.calculateSolid`: forced on or off by the
  block, else a non-empty collision shape whose bounds average at least 0.7292 or whose height
  is at least 1. A top slab is solid; a bottom slab, trapdoor or carpet is not. Spawn cells
  run from villager y-6 to y+6.
- **`EntityType.create`** puts the golem at (x + 0.5, y, z + 0.5) with yaw
  `level.random.nextFloat() * 360`. `Mob.finalizeSpawn` draws the follow-range triangle and
  left-handedness from the level random, whether the spawn succeeds or not.
- **`IronGolem.checkSpawnObstruction`** has four conditions:
  - the block below `entityCanStandOn` (its collision face UP is full);
  - the cells at +1 and +2 pass `NaturalSpawner.isValidEmptySpawnBlock`: not a full collision
    cube, not a signal source, no fluid, not `PREVENT_MOB_SPAWNING_INSIDE` (rails), not
    dangerous (fire, lava, magma, campfire, wither rose, berry bush, cactus, powder snow);
  - the feet cell passes the same check with an EMPTY fluid, so **a golem may spawn standing
    in water**, but not with water at head height;
  - `isUnobstructed`: no entity with `blocksBuilding` meets the 1.4 x 2.7 box. Living entities
    count, dying ones until their removal 20 gt after death, and so do boats and minecarts;
    items do not.

  Block collisions are not checked (`checkCollisions` false): the box may overlap walls.

### The golem, the kill and the drops

- **`EntityTypes.IRON_GOLEM`**: 1.4 x 2.7 (eye 2.295). `IronGolem.createAttributes`: 100
  health, speed 0.25, knockback resistance 1, step height 1. Fall damage immune (tag
  `fall_damage_immune`). It does not drown (`decreaseAirSupply` returns the supply). It is
  pushed by water (`isPushedByFluid`).
- **Movement**: `LivingEntity.aiStep` (0.003 floors, jump, `travel`, `applyEffectsFromBlocks`,
  `pushEntities`). `travel` picks `travelInAir` (block friction x 0.91, gravity 0.08, x 0.98
  vertical), `travelInWater` (x 0.8, speed 0.02, gravity/16, `jumpOutOfFluid`) or `travelInLava`
  (x 0.5, gravity/4). `Entity.move` is the existing redsim collide.rs. Fluid pushing uses
  0.014 for water. `pushEntities` calls `Entity.push` (0.05 / distance). Cramming above the
  game rule's 24 does 6 damage on `random.nextInt(4) == 0`.
- **Damage**: `LivingEntity.hurtServer` with `damageCooldownTime` 20. While it is above 10,
  only damage higher than `lastHurt` lands, and only the difference.
  - Lava: `LavaFluid.entityInside` sets LAVA_IGNITE (15 s of fire) then `lavaHurt`, 4 damage.
  - Fire: `Entity.baseTick` does 1 damage every 20 fire ticks when not in lava.
  - In a wall: `isInWall`, 1.
  - Magma: `stepOn`, 1.

  A golem standing in a lava blade dies in about 250 gt. **`die`** drops the loot at once at
  the golem's position. **`tickDeath`** removes the body 20 gt later.
- **Loot** (`loot_table/entities/iron_golem.json`): a poppy pool `uniform(0, 2)` and an iron pool
  `uniform(3, 5)`: 4 ingots and 1 poppy on average. `Entity.spawnAtLocation` creates an
  `ItemEntity` at the golem's feet with motion (`random.nextDouble() * 0.2 - 0.1, 0.2, ...`)
  from the item's own random, and a pickup delay of 10. Items burn in lava and fire, so the
  drops must not reach the blade. Iron golems do not despawn.

### The zombie

- **`EntityTypes.ZOMBIE`**: 0.6 x 1.95, eye 1.74, riding offset -0.7.
- **Persistence**: `Mob.requiresCustomPersistence` = `isPassenger() || isLeashed()`, so a
  zombie in a boat or minecart, or on a lead, never despawns. Otherwise `checkDespawn` removes
  it beyond 128 blocks of a player, or at random beyond 32.
- **Sunlight** (`Mob.isSunBurnTick`): while `MONSTERS_BURN` holds (day time 23460..12542), at
  brightness > 0.5, when the eye cell `canSeeSky`.
- **Drowning**: `Zombie.tick`'s drowning tracker turns it into a drowned after its eye spends
  enough time in water.
- **Pickup**: `Mob.aiStep` with `canPickUpLoot` (`Zombie.finalizeSpawn`: 55% x the difficulty
  multiplier), reach (1, 0, 1).
- **Villager kills**: `Zombie.killedEntity` may turn a villager into a zombie villager.

## Trade-offs

- **The calm moment** costs either a sensor (a daylight detector: 3 nether quartz, so not day
  1) or a trigger the farm already makes. Hidden all night, the villagers sleep all night and
  no golem spawns until the zombie shows again: about half the rate. Hidden briefly, they fall
  asleep and are woken by the zombie, provided their sleeping eyes (bed + 0.8875) can see it.
- **The golem's exit.** A golem alive in a villager's follow-range box at a 20 gt scan is seen
  by that villager's golem scan with probability about window / 200 per villager. Each
  sighting stretches a cycle by about 100 gt. One that burns next to the villagers (about 250
  gt) is seen almost surely: about 800 gt per golem instead of 600. A drop of about 20 blocks
  out of range costs a tall tower and a long redstone line.
- **Spawn spots.** Each attempt picks one of 17 x 17 = 289 columns. With f the share of useful
  columns, a 10-attempt try succeeds with 1 - (1 - f)^10. A failed try waits 100 gt. A 4 x 4
  pad gives f = 5.5% and 43%; a full 17 x 17 pad about 100%. Columns topped by anything solid
  (a pod roof, a wall) can spawn golems in the wrong place, which is why the pods are glass.
- **More villagers** (4-5 per cluster): a golem that one or two of them see no longer blocks
  the cluster, since only 3 must agree. The cost is more beds and transport. With 5 or more,
  gossip could spawn golems too: refused unless they are never calm outside REST.
- **Scaling**: clusters must stay out of each other's 10-block box, and every golem must stay
  out of every other cluster's follow-range box. That sets the pitch: about 22 blocks
  horizontally, more vertically, with one zombie per cluster (a zombie scares villagers within
  8 only).
- **The bill**: a hopper is 5 iron, so the collection line dominates. One golem (4 iron) pays
  for most small farms.

## Five layouts

All per cluster of 3 villagers. Ceiling: 120 golems/h = 480 iron/h. The estimates are mine,
before the judge.

| # | layout | calm moment | golem exit | estimate | bill (iron) | blocks |
|---|---|---|---|---|---|---|
| L1 | **The obvious design**: three villagers on beds in 1x1 glass pods; the zombie in a boat 3-4 blocks away in view; a 9x9 pad 3 below them with water flowing to a 2x2 hole; a lava-blade kill cell with a hopper and a chest under it | an inverted daylight detector pushes a block into the line of sight at night (sleep all night) | the fall through the hole, the blade 6 below the pad (in range while burning) | ~45% (55 golems/h, 220 iron/h) | 5 hopper + 1 piston (+3 quartz) | ~150 |
| L2 | **Middle ground**: L1's pod and zombie; a 13x13 pad with water to a 2x2 hole; the kill cell 22 below the villagers | a tripwire across the kill cell: the burning golem holds it about 270 gt, which drives a piston that hides the zombie during the cooldown; at night the villagers sleep and wake at once (the zombie is in view of their beds) | the drop out of range (~25 gt) | ~85% (100 golems/h, 400 iron/h) | 5 + 1 + 1 (hooks) + string | ~350 |
| L3 | **Day 1**: three villagers on beds side by side under one glass roof; the zombie in a boat; a small pad around them; water pushes golems straight into a blade beside the pad; one hopper, one chest | the same tripwire trick, the shortest redstone | none: they burn in range | ~60% (70 golems/h, 280 iron/h) | 5 + 1 + 1 + string, no quartz | ~80 |
| L4 | **Fast exit**: pad strips 1 wide between 1-wide channels; water pushes a golem off its strip within a few gt; a drop of about 20 blocks | tripwire | out of range in < 20 gt | ~95% (115 golems/h, 460 iron/h) | ~7 | ~500 |
| L5 | **Scaled**: N copies of L4 (5 villagers each) on a grid at the separation pitch, all dropping into one kill cell (one hopper line) | each module's own tripwire | as L4 | N x ~110 golems/h | ~7 + 2N | ~400N |

## Decision

1. Build **L1** first as the bar (G2): it is the straightforward reading of the mechanics,
   with a sensor for the night. Its measured numbers calibrate the judge.
2. **Day 1 = L3**: no quartz, the least iron, one hopper.
3. **Middle ground = L2**, or L4 if the search finds it cheap. **Fastest scaled = L5**, the
   module searched first (the pad, the exit, 3 or 5 villagers), then tiled.

The judge decides between estimates. Each farm is reported as a percentage of the ceiling.

## Must-haves (real use)

- The villagers can never be reached by the zombie (its melee box) or by a golem's attack,
  can never leave their pods, and take no damage.
- The zombie never despawns (a passenger or on a lead), never burns (its eye cell never sees
  the sky), never drowns or converts, and no drop can reach its pickup box.
- No golem escapes the build, touches the zombie or its vehicle, or gets within melee reach of
  the zombie. Every golem dies in the kill cell.
- Every drop reaches the output chest (the judge's, at `port output`). None burns or is left
  on the ground.
- Survival materials only: no lava flowing out of its cell (static sources only), no blocks
  a player cannot get.
- It runs for days: the villagers sleep every night and the rate does not decay.
- Robust over positions and rotations.

## Not goals

The judge does not check:

- natural monster spawning on the build (spawn-proofing);
- how the player brings the villagers and the zombie in;
- the gossip path (refused instead, see the judge);
- lag;
- chunk borders, as long as the build stays inside the simulated box.

## The judge (`circuit: mob-farm iron_ingot`, crates/circuit/src/studies/mobfarm.rs)

A registered study (`StudyDef` id `mobfarm`) that runs entirely on redsim, with its kind
`mob-farm <item>` so the next farms (gold, creeper, raid...) add a product, not a new study.
The design gives its villagers, zombie and vehicle as `entity:` lines, its beds as blocks,
`port output` (the chest), and `port afk` (where the player stands, for despawn distances).

- **Runs**: K seeds x D game days at vanilla speed (K = 8, D = 5 by default), each from a
  random day time.
  - Every seed draws the villagers' follow-range bonus, their sensor phases and every other
    entity random.
  - Each villager starts with its HOME claimed (the bed in its pod) and a `LAST_SLEPT` from
    the night before.

  Metrics:
  - iron/h (mean, 95% interval), golems/h, poppies/h, seconds per golem;
  - share of the ceiling;
  - losses by cause: no spot, golem seen, panic gap, no sleep;
  - blocks, bill, payback.
- **Builds**: 4 rotations x 2 positions. They agree when every build passes and the iron/h
  means are within the interval (`Agree::Rate`).
- **Checks (the must-haves)**: a failing check names what, where and when.
  - A villager outside its pod or hurt.
  - The zombie reachable, burning, despawnable or in water.
  - A golem outside the build, in reach of the zombie or its vehicle, or not dead within N gt.
  - A drop lost, burnt, loose at the end, or near the zombie.
  - Flowing lava.
  - A night without sleep.
- **Refusals (unmodeled AI)**, before any run: see the scope below.
- `mctas circuit run --sim` gives one run's table: every golem's spawn gt, spot, exit, death
  and drops, plus each villager's activity changes. `mctas circuit sim` and `verify` run the
  whole judge (`verify` also runs on redsim; the game checks are the one-off validations
  below). `mctas mobfarm gen ...` generates the parametric layouts (pad size, villagers,
  modules, stacking).

## The mob layer: what is exact, what is refused

New files in `crates/redsim/src/entity/mob/`. Existing redsim files get registrations and
hooks only, agreed with the boat-leads lane (06b77372) and the coordinator.

**Exact** (the same formulas in the same order as the 26.3 source):
- the entity list's order;
- LivingEntity physics (travel in air, water and lava, friction, step height, fluid
  pushing, entity pushing, cramming);
- damage and invulnerability (lava, fire, wall, magma, cramming, fall, drowning);
- death, drops and removal;
- the Brain: memory slots and their expiry order, sensors and their rates, start delays,
  activities, the schedule;
- `VillagerPanicTrigger` (with its 60 gt duration), `VillagerCalmDown`, `SleepInBed`, `WakeUp`,
  `UpdateActivityFromSchedule`;
- the hostile, nearest-living and golem sensors, with COLLIDER line-of-sight rays;
- `spawnGolemIfNeeded`, `wantsToSpawnGolem`, `golemSpawnConditionsMet`, `trySpawnMob`
  (LEGACY_IRON_GOLEM), `checkSpawnObstruction`, `isValidEmptySpawnBlock`, `isUnobstructed`;
- sleeping and waking (`findStandUpPosition`), beds and `OCCUPIED`;
- the golem loot; zombie persistence; day time.

**Seeded, same distribution** (the game draws from sources we cannot mirror: unseeded entity
randoms, and the level random that behaviors we do not run also draw from):
- sensor phases and follow-range bonuses;
- spawn columns, yaw and the finalizeSpawn draws;
- loot counts;
- item motions;
- cramming rolls.

**Refused, never approximated** (the judge rejects a design where it could change the outcome):
- **Walking and pathfinding.** Villagers must be confined: the judge floods their reachable
  feet positions, with steps and jumps. Every check that depends on their position must hold
  over that region: the line of sight to the zombie standing and sleeping, the 2.0 bed reach,
  the stand-up spot, staying out of every follow box that matters. The zombie must ride a
  vehicle or be leashed and stay in its cell. Golems are run under several walking inputs
  (none, the 8 directions at their goal speeds, jumping when blocked). Every one must end in
  the kill cell.
- **Golem targeting** (refused if a golem can get within melee reach of the zombie) and
  **zombie attacks** (refused if within reach of a villager).
- **POI claiming** (refused if a pod holds no bed in reach or could reach another pod's bed).
- **Gossip and trading** (refused if 5+ villagers of a cluster can be calm outside REST).
- **Job sites and breeding** (refused if a workstation is in the build, or if villagers
  could be fed).
- Raids, bells, and natural spawning (not judged, listed in docs/coverage.md).

## Validation against the game (one-off checks, recorded here)

1. **Spawn spots**: three platforms, with real villagers (a `LAST_SLEPT` memory in their summon
   data) and a zombie in a boat. Every golem's spawn cell over many cycles goes into a
   histogram, compared with redsim's draw for the same build (chi-square).
2. **A whole farm**: L1 or L3 run for several game days in the game (the bridge's `exec
   summon`). Golems/h and iron/h are compared with redsim's mean within its 95% interval.

## The obvious design's measured numbers

(Filled in after the judge runs L1.)

## Ceiling

- **Per cluster**: one golem per 600 gt (the 599 gt memory, cleared at the start of the brain
  tick, and the multiple-of-100 attempts). That is 120 golems/h = **480 iron/h** and 120
  poppies/h at vanilla speed.
- A cluster cannot do better: every golem marks every villager within 10 blocks.
- A farm of N clusters: N x 480 iron/h.
- The panic gap (one tick in 62) and the scans set the practical ceiling slightly lower; the
  judge reports both.
