# Iron farms (design doc, gate G1)

ironfarm track, 2026-11-08. Gate G1 ([docs/process.md](../../docs/process.md)). Asked by vjeux on
2026-11-07: "I want you to build iron farms. Should be a mix of day 1 farm, good middle ground and
fastest scaled one", "as you go build the infrastructure to build and manage farms using redsim and
whatever else is needed. I will be asking you to build all the top ones in the future so lay out
the foundations for it", "Please implement everything you need on redsim. We can't use the game
code, it's way too slow in java". Every rule below was read in the decompiled 26.3 server
(`mctas src show <Class>`); no outside design was used. The work log is
`designs/circuits/mobfarm/tracks/ironfarm/NOTES.md`.

Revision 3 (after the G1 review and re-review by 8146b8db, 2026-11-08): the changelogs are at
the end.

## Use

A survival player builds an iron farm for iron ingots without mining (hoppers, rails, anvils,
iron blocks for beacons). They AFK near it, and every few minutes they empty a chest. The
farm needs villagers, beds and a zombie, which they bring in by hand.

## The number

Every farm reports four numbers. The board ranks by the first, as process.md says for farms.
Each of vjeux's three tiers is decided by its own number:

1. **Iron per hour per block** (the board's rank): iron ingots per hour at vanilla speed for
   the whole farm, divided by the blocks placed.
2. **Iron per hour**: golems per hour x 4 (the mean drop), the mean over every run (seeds x game
   days), with its 95% interval. For a tiled farm, also per module.
3. **The bill**: the iron the build itself costs, by recipe:
   - a hopper 5, a piston 1, an iron trapdoor 4, a minecart 5;
   - rails 6 per 16, tripwire hooks 1 per 2;
   - one bucket, 3, for the lava and water.

   It lists the other survival materials too: villagers, beds, zombies, nether quartz, string.
4. **Payback**: the bill divided by iron per hour, in minutes.

**The tiers** (each decided by its own number, so vjeux's three stay distinct):

| tier | its number | the rule |
|---|---|---|
| day 1 | the lowest bill | no nether material (no quartz), and at least 25% of the ceiling |
| middle ground | iron per hour per block | among builds at 75% of the ceiling or more with a bill of at most 20 iron |
| fastest scaled | iron per hour per module | the scaled build is reported as N x the module |

The middle tier's 75% floor is what separates it from day 1. The day-1 pick (L3) kills its
golems in place, in range of the villagers, which caps it near 2/3 of the ceiling, so the floor
rules it out by its own rate rather than by an exclusion. Iron per hour of the whole build
would grow with the module count N, not with the design, so the fastest tier ranks modules.

Reported too: golems per hour, poppies per hour, seconds per golem, the share of the ceiling,
the rate under the worst golem walking policy (see the judge), and the misses by cause:
- tries that found no spot;
- golem sightings;
- nights without sleep.

## Mechanics, from the game code (26.3)

### Who spawns a golem and when

- **The trigger is the panic behavior.** `VillagerGoalPackages.getCorePackage` puts
  `VillagerPanicTrigger` in the CORE activity, which is always active. It is a `Behavior` with
  an empty entry map, the default 60 gt duration and no `checkExtraStartConditions`, so
  `Behavior.tryStart` always succeeds. Its `start` tests `isHurt` (memory `HURT_BY`) or
  `hasHostile` (memory `NEAREST_HOSTILE`) and then makes PANIC the active activity.
  `canStillUse` repeats that test, so an unscared villager starts and stops it in the same
  tick. Its `tick` runs `if (timestamp % 100L == 0L) body.spawnGolemIfNeeded(level, timestamp,
  3)`. `Behavior.tickOrStop` stops it once `timestamp > endTimestamp`, and it starts again the
  next tick. So for a villager that stays scared, one tick in every 62 has no `tick` (gt
  `t0 + 61 + 62k`).
- **Every willing, panicking villager tries on its own.** At a multiple of 100, each villager
  whose trigger ticks runs `Villager.spawnGolemIfNeeded` with its own 10 attempts, centred on
  its own `blockPosition()`, in entity tick order. A failed try marks nothing, so the next
  villager tries too: up to 10 x N attempts from N centres. An attempt is lost to the panic gap
  only when every eligible villager is in its gap tick at once.
- **`Villager.spawnGolemIfNeeded(level, timestamp, 3)`:** if `wantsToSpawnGolem`, it lists the
  villagers whose box meets this villager's box inflated by 10 (`getEntitiesOfClass`, itself
  included). It keeps those that `wantsToSpawnGolem`, at most 5. With 3 or more it calls
  `SpawnUtil.trySpawnMob(IRON_GOLEM, MOB_SUMMONED, level, blockPosition(), 10, 8, 6,
  Strategy.LEGACY_IRON_GOLEM, false)`. **Only `wantsToSpawnGolem` is tested for the others: a
  sleeping or idle villager counts.** On success, EVERY villager in the box (willing or not)
  gets `GolemSensor.golemDetected`: `GOLEM_DETECTED_RECENTLY` with a 599 gt expiry.
  `Villager.gossip` calls it too, needing 5 willing villagers (see the judge's gossip rule).
- **`wantsToSpawnGolem` = `golemSpawnConditionsMet && !GOLEM_DETECTED_RECENTLY`**.
  `golemSpawnConditionsMet` holds when the memory `LAST_SLEPT` exists and `gameTime - lastSlept
  < 24000`. **Without a sleep in the last game day, a villager never wants a golem.**
- **Memory expiry order.** `Brain.tick` runs `forgetOutdatedMemories`, `tickSensors`,
  `startEachNonRunningBehavior`, then `tickEachRunningBehavior`. `MemorySlot.tick` clears a
  memory when `timeToLive <= 0`, else decrements it. When villager V spawns a golem at T:
  - V and the villagers before it in the entity list clear the memory at their own brain tick
    at T+600, the villagers after it at T+599;
  - so at T+600 only V and the villagers after it can count 3: one earlier in the list still
    reads V's memory at ttl 0 until V's own `forgetOutdatedMemories`.

  The next multiple of 100 is T+600. **One golem per 600 gt per villager cluster is the
  ceiling** (below).
- **The golem detector.** `GolemSensor` (scan rate 200) reads `NEAREST_LIVING_ENTITIES`. If any
  entity in it is an iron golem, it sets the same 599 gt memory. `NearestLivingEntitySensor`
  (rate 20) stores every living entity (`isAlive`) whose box meets the villager's box inflated by
  its `FOLLOW_RANGE`, sorted by distance. The golem sensor reads that stored list, so a golem
  alive and in range at the last 20 gt scan counts, even if it died since.
  - The range: `Mob.finalizeSpawn` draws the bonus `b = random.triangle(0, 0.11485)`, and the
    villager's follow range is 16 x (1 + b), so R is in 14.16..17.84.
  - With the villager's 0.6 x 1.95 box and the golem's 1.4 x 2.7 box, a golem is listed when
    `|dx|, |dz| < R + 1.0`. Vertically, its feet must be above `villager y - R - 2.7` and below
    `villager y + 1.95 + R`.
  - A golem the sensor sees between T and T+600 pushes the next attempt to the first multiple
    of 100 after its own 600 gt. **The golem must leave every villager's box (or die) before
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
  `getVisibilityPercent`, and `Mob.getSensing().hasLineOfSight`.
  - That last test is `LivingEntity.hasLineOfSight`: a `ClipContext.Block.COLLIDER` /
    `Fluid.NONE` ray from eye to eye, at most 128 blocks. Glass blocks it (it has a collision
    box); water does not.
  - `NearestVisibleLivingEntities` evaluates it lazily and caches it per entity, for one
    20 gt list.
  - In PANIC and REST the only other readers are the minimal look behaviors
    (`SetEntityLookTarget` VILLAGER and PLAYER), and they filter by type before the ray. So the
    zombie's ray in each list is first cast by the hostile sensor.

### Sleep: the condition every 26.3 iron farm has to meet

- **A panicking villager never sleeps.** `SleepInBed` is in the REST package, and its `start`
  is the only writer of `LAST_SLEPT`. While PANIC is active, the schedule is not consulted:
  only `VillagerCalmDown` (in the PANIC package) calls `updateActivityFromSchedule`, and only
  when the villager is not scared (no `HURT_BY`, no `NEAREST_HOSTILE`, no `HURT_BY_ENTITY`
  within 6).
- **So the villagers that see the zombie need a calm moment each night.** Within the REST
  window, the zombie must be hidden from their eyes during one hostile scan: at least 20 gt, to
  catch every sensor phase. A villager that never sees the zombie sleeps through every REST on
  its own, and since `spawnGolemIfNeeded` tests only `wantsToSpawnGolem`, it still counts.
  Only the scared villagers need the hide (layout L6).
- **The schedule** (`Timelines`: `VILLAGER_SCHEDULE`, adult track): day time 10 IDLE, 2000 WORK,
  9000 MEET, 11000 IDLE, 12000 REST. `Brain.updateActivityFromSchedule` runs at most once per
  21 gt (`gameTime - lastScheduleUpdate > 20`). `setActiveActivityIfPossible` falls back to the
  default IDLE when the activity's requirements fail: WORK needs `JOB_SITE`, MEET needs
  `MEETING_POINT`.
- **`SleepInBed`** (REST, priority 3) starts only under all of these:
  - the villager is not a passenger;
  - it holds a `HOME` memory;
  - it was not woken 1..99 gt ago (`LAST_WOKEN`);
  - the HOME position is `closerToCenterThan(position, 2.0)`;
  - the bed is not `OCCUPIED`.

  `start` (when `timestamp > nextOkStartTime`) calls `startSleeping(home)` and sets
  `LAST_SLEPT = timestamp`. It stays running while REST is active, `y > bed.y + 0.4` and the
  bed is within 1.14. `stop` calls `stopSleeping` and `nextOkStartTime = t + 40`. `WakeUp`
  (CORE) stops the sleep when REST is not active. A villager that calms down at night falls
  asleep in the same brain tick (CalmDown at priority 0 switches to REST before priority 3
  starts). It only needs to start sleeping once per game day: a panic right after waking it
  changes nothing.
- **`LivingEntity.startSleeping`**: position at the bed + (0.5, sleep height 0.5625 + 0.125,
  0.5), `OCCUPIED`, pose SLEEPING (0.2 x 0.2, eye 0.2), motion zero. **`stopSleeping`**: bed
  free; position from `AbstractBedBlock.findStandUpPosition`. That searches the cells around the
  bed (the side chosen by the villager's yaw), safe ones first, else the cell above the bed
  (+0.1). Pose STANDING.
- **Keeping HOME.** `SetWalkTargetFromBlockMemory(HOME, 0.5, 1, 150, 1200)` (REST, priority 2)
  sets a walk target when `distManhattan(home, blockPosition()) > 1`. A path that keeps failing
  sets `CANT_REACH_WALK_TARGET_SINCE`, and 1200 gt later the villager calls
  `releasePoi(HOME)`, erases HOME and never sleeps again. **Every cell a villager can stand in
  must be within Manhattan 1 of its HOME.**
- **Claiming the bed** (`AcquirePoi` for `HOME` in CORE) needs a path
  (`AcquirePoi.findPathToPois`). See "Refused" below.

### Where a golem spawns

- **`SpawnUtil.trySpawnMob`**: 10 attempts. Each draws `dx, dz = Mth.randomBetweenInclusive(
  random, -8, 8)` from the LEVEL random and starts at (villager block + (dx, 6, dz)).
  `moveToPossibleSpawnPosition` walks down from y+5 (with the cell above it, y+6) to y-7. It
  stops at the first cell whose block passes `LEGACY_IRON_GOLEM`:
  - not cobweb, cactus, glass, tinted glass, stained glass, any glass pane, leaves, conduit,
    ice, frosted ice, TNT, glowstone, beacon or sea lantern;
  - the block above is air or a liquid;
  - the block `isSolid()` (or is powder snow).

  The golem goes in the cell above. `BlockStateBase.calculateSolid` says solid when the block
  forces it, or when its collision shape is non-empty and its bounds average at least 0.7292 or
  are a full block tall. Slabs (top or bottom), beds and 3 px trapdoors are solid; carpets are
  not. The scan stops at the first match even when the obstruction check then fails (a bed, a
  bottom slab): that attempt is wasted. Spawn cells run from villager y-6 to y+6.
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
- **Movement**: `LivingEntity.aiStep` runs the 0.003 floors, the jump, `travel`,
  `applyEffectsFromBlocks` and `pushEntities`. `travel` picks one of:
  - `travelInAir`: block friction x 0.91, gravity 0.08, x 0.98 vertical;
  - `travelInWater`: x 0.8, speed 0.02, gravity / 16, `jumpOutOfFluid`;
  - `travelInLava`: x 0.5, gravity / 4.

  `Entity.move` is the existing redsim collide.rs. `pushEntities` calls `Entity.push`
  (0.05 / distance). Cramming above the game rule's 24 does 6 damage on
  `random.nextInt(4) == 0`.
- **Transit in water.** `Entity.updateFluidInteraction` adds the normalized current x 0.014 a
  tick to a non-player (`EntityFluidInteraction.applyCurrentTo`), and `travelInWater` keeps 0.8
  of it. The terminal drift is about 0.056 blocks/gt.
  - A 1.4-wide golem resting on a 1-wide strip stays on it until its centre has moved 1.2:
    about 25 gt from rest. It cannot drop into a 1-wide channel, so channels must be at least
    2 wide.
  - Its own walking input (`IronGolem.registerGoals`: stroll 0.6, move-towards-target 0.9,
    melee 1.0 of its 0.25 speed) adds `moveRelative(0.02, input)`, at most about 0.005 a tick.
    It never beats the current, but it moves the drift between about 0.036 and 0.076
    blocks/gt with its direction.
  - Falling: v <- (v - 0.08) x 0.98. Leaving the follow box from a pad at villager y-6 takes
    at least 15.5 blocks of fall, about 22 gt.
- **Damage**: `LivingEntity.hurtServer` with `damageCooldownTime` 20. While it is above 10,
  only damage higher than `lastHurt` lands, and only the difference.
  - Lava: `LavaFluid.entityInside` sets LAVA_IGNITE (15 s of fire) then `lavaHurt`, 4 damage.
  - Fire: `Entity.baseTick` does 1 damage every 20 fire ticks when not in lava.
  - In a wall: `isInWall`, 1.
  - Magma: `stepOn`, 1.

  A golem in lava dies in about 250 gt, and nothing environmental is faster (fall immune, no
  drowning). **`die`** drops the loot at once at the golem's position. **`tickDeath`** removes
  the body 20 gt later.
- **Loot** (`loot_table/entities/iron_golem.json`): a poppy pool `uniform(0, 2)` and an iron pool
  `uniform(3, 5)`: 4 ingots and 1 poppy on average. `Entity.spawnAtLocation` creates an
  `ItemEntity` at the golem's feet with motion (`random.nextDouble() * 0.2 - 0.1, 0.2, ...`)
  from the item's own random, and a pickup delay of 10. Items burn in lava and fire, so the
  drops must not reach the blade. Iron golems do not despawn.
- **The golem's targets.** `IronGolem.registerGoals` targets the zombie through walls
  (`NearestAttackableTargetGoal<Mob>`, mustSee false, `Enemy && !Creeper`).

### The zombie

- **`EntityTypes.ZOMBIE`**: 0.6 x 1.95, eye 1.74, riding offset -0.7. In a wooden boat it sits at
  boat y + 0.1875 - 0.7 (`Boat.rideHeight` = height / 3); in a minecart at cart y + 0.1875 - 0.7.
- **Persistence**: `Mob.requiresCustomPersistence` = `isPassenger() || isLeashed()`, so a
  zombie in a boat or minecart, or on a lead, never despawns. Otherwise `checkDespawn` removes
  it beyond 128 blocks of a player, or at random beyond 32.
- **Reach**: `Mob.getAttackBoundingBox` takes the horizontal union of the zombie's box and its
  vehicle's box, over the zombie's own vertical span, inflated horizontally by
  `DEFAULT_ATTACK_REACH = sqrt(2.04) - 0.6 = 0.828`. In a 1.375-wide boat that reaches 1.52 from
  the boat's centre.
- **Sunlight** (`Mob.isSunBurnTick`): while `MONSTERS_BURN` holds (day time 23460..12542), at
  brightness > 0.5, when the eye cell `canSeeSky` (sky light 15).
- **Drowning**: `Zombie.tick`'s drowning tracker turns it into a drowned after its eye spends
  enough time in water.
- **Pickup**: `Mob.aiStep` with `canPickUpLoot` (`Zombie.finalizeSpawn`: 55% x the difficulty
  multiplier), reach (1, 0, 1).
- **Villager kills**: `Zombie.killedEntity` may turn a villager into a zombie villager.

### The calm devices, weather and lightning

- **Tripwire** (`TripWireBlock.entityInside` -> `checkPressed`): any entity whose box meets the
  string presses it. That includes a dying golem until its removal 20 gt after death, and its
  drops. The press is re-checked every 10 gt while pressed, and the hooks
  (`TripWireHookBlock`) power while the string is pressed.
- **Daylight detector** (`DaylightDetectorBlock`): it updates when `gameTime % 20 == 0`. The
  inverted output is `15 - getEffectiveSkyBrightness`, which is the sky darkening.
- **Rain** (`ServerLevel` weather; `WeatherAttributes`): it blends the sky light level toward 4
  (rain alpha 0.3125: darkening 3 at noon; thunder 0.527: 5). An inverted detector therefore
  outputs at least 3 all through a rainy day: L1 hides its zombie and stops. It rains about
  16% of the time (`RAIN_DELAY` uniform 12000..180000, `RAIN_DURATION` 12000..24000).
- **Lightning** (`ServerLevel.findLightningTargetAround`): it strikes a living entity that can
  see the sky (sky light 15, which glass keeps) within 3 of the strike column. A struck
  villager turns into a witch.

## Trade-offs

- **The calm moment** has three options:
  - a sensor, the daylight detector (3 nether quartz, so not day 1; it fails in rain);
  - a trigger the farm already makes: a tripwire pressed by each burning golem during the 599
    gt cooldown, which costs no attempt;
  - no hide at all for the villagers that never see the zombie (L6).

  Hidden all night, the scared villagers sleep all night and no golem spawns until the zombie
  shows again: about half the rate. Hidden briefly, they fall asleep and are woken by the
  zombie, provided their sleeping eyes (bed + 0.8875) can see it.
- **The golem's exit.**
  - A golem in a villager's follow box at a 20 gt scan is in the stored list for 20 gt, and each
    golem scan (period 200) that falls in that window marks the villager: about window / 200
    per villager. Each sighting stretches the cycle by about 100 gt.
  - On a water pad the golem drifts at 0.036-0.076 blocks/gt. On a 13 x 13 pad it stays in range
    60-90 gt before its drop.
  - The fastest exit: a 1-wide strip with water across it into a 2-wide channel and a fall out
    of the box from villager y-6. That is at least about 25 + 22 = 47 gt in range, so each
    villager sees about 25-30% of the golems. With 3 villagers about 63% of the cycles stretch
    (about 90% of the ceiling); 95% needs 4 villagers.
  - A golem that burns next to the villagers (about 250 gt) is seen by every villager: about
    850-900 gt per golem instead of 600, about 2/3 of the ceiling.
- **Spawn spots.** Each attempt picks one of the 17 x 17 = 289 columns around its villager, and
  every willing panicking villager makes its own 10. With N such villagers and a share f of
  useful columns in each square, a hundred-tick mark succeeds with about 1 - (1 - f)^(10N).
  - A 4 x 4 pad inside three villagers' squares: f = 5.5%, 43% for one villager, 82% for three.
  - A failed mark waits 100 gt.
  - Columns topped by anything solid (a pod roof, a wall) waste attempts or spawn golems in the
    wrong place, which is why the pods are glass.
- **More villagers.** With 4 per cluster, one villager's sighting no longer blocks it (only 3
  must agree), and 5 willing villagers (the gossip path) can never be gathered. The cost is a
  bed and a transport each.
- **Scaling.**
  - Clusters must stay out of each other's 10-block box.
  - Every golem must stay out of every other cluster's follow box: `|dx|, |dz| < R + 1.0` with
    R up to 17.84. A golem spawns up to 8 columns from its villager, so a full-pad module
    needs a pitch of at least 26.9 horizontally (the pad's reach + 18.84 in general).
  - Stacking: a module Q blocks below another lists the upper one's golems while
    Q < 25.8 (6 + 2.7 + 17.84 - 0.74). Golems falling past a lower module must keep 18.84
    horizontally from its villagers.
  - One zombie per cluster: it scares villagers within 8 only.
  - Each module needs its own kill cell (or its own trigger): a tripwire in a shared kill cell
    is pressed by every module's golems (one arrival per 600 / N gt, each holding it about 270
    gt), which hides the zombies almost all the time.
- **The bill**: a hopper is 5 iron, so the collection line dominates. One golem (4 iron) pays
  for most small farms.

## Six layouts

Per cluster unless stated. Ceiling: 120 golems/h = 480 iron/h per cluster. The estimates are
mine, before the judge, with the review's corrections.

| # | layout | calm moment | golem exit | estimate | bill (iron) | blocks |
|---|---|---|---|---|---|---|
| L1 | **The obvious design**: 3 villagers on beds in 1-cell glass pods under an opaque roof; the zombie in a boat in view; a 9x9 pad 3 below them with water to a 2x2 hole; a lava blade over 2x2 hoppers | an inverted daylight detector pushes a block into the line of sight at night (they sleep all night; it also stops in rain) | the fall through the hole; the blade 6 below the pad, in range while burning | ~35% in clear weather (42 golems/h, 170 iron/h) | 4 hoppers 20 + piston 1 + bucket 3 = 24, +3 quartz | ~200 |
| L2 | **Middle ground**: L1's pods and zombie; a 13x13 pad with water to a 2x2 hole; the module's own kill cell 22 below the villagers | a tripwire in the kill cell: the burning golem holds it about 270 gt, so a piston hides the zombie during the cooldown; at night the villagers sleep and are woken at once (the zombie is in view of their beds) | 60-90 gt on the pad, then a ~22 gt fall out of range | ~80% (96 golems/h, 380 iron/h) | 1 hopper 5 + piston 1 + hooks 1 + bucket 3 = 10, string | ~400 |
| L3 | **Day 1**: 3 villagers, each on its bed in its own closed 1-cell pod; the zombie in a boat; golems spawn on 1x1 floors with a lava source beside their head, so they burn where they spawn; water carries the drops to one hopper | the tripwire trick on every spawn floor: one string line (2 hooks) runs through all of them at golem knee height, so any golem powers a wooden trapdoor that hides the zombie | none: they burn in range | ~65% (78 golems/h, 310 iron/h) | hopper 5 + hooks 1 + bucket 3 = 9, string, no quartz, no piston | ~120 |
| L4 | **Fast exit**: 4 villagers; 1-wide pad strips between 2-wide channels with water across; a fall of 16+ blocks to the module's kill cell | tripwire in the kill cell | at least ~47 gt in range | ~93% (112 golems/h, 450 iron/h) | ~10 | ~600 |
| L5 | **Scaled**: N modules of L4 (4 villagers each) at a 27-block pitch, each with its own kill cell and trigger, one collection line to one chest | each module's own tripwire | as L4 | N x ~110 golems/h | ~5 + 6N | ~600N |
| L6 | **One scared villager** (the reviewer's): one villager in view of the zombie, two or more that never see it; the scared one needs the night hide, the others sleep on their own | a hide for one pair of eyes only (tripwire) | as L2 or L4 | lower attempts (10 per mark instead of 30) and that villager's panic gap: ~70% with a small pad, near L2 with a full one | ~10 | ~350 |

**A calm moment made by golems cannot start the farm.** Villagers with no sleep in the last
game day never spawn the first golem, so no wire is pressed, the zombie is never hidden and
nobody sleeps. Every layout but L1 has this deadlock, so each of their captions says that the
player hides the zombie by hand (a block in front of it) on the first night. While the farm
runs, every night has golems (their villagers slept the night before), so it keeps itself
going: in L3 the string runs through every spawn floor so that no night can pass with all of
its golems on unwired floors. Only L1, whose detector hides the zombie every night by itself,
claims to restart after a missed night, and the judge checks that claim (below).

## Decision

1. **L1 first, as the bar (G2):** it is the straightforward reading of the mechanics, with a
   sensor for the night. It is measured in clear weather, and its caption says it stops in
   rain. Its numbers calibrate the judge.
2. **Day 1 = L3:** the lowest bill (9 iron: one hopper, one hook pair, the bucket; a powered
   wooden trapdoor instead of a piston), no quartz, about 120 blocks. Its kill in place costs
   rate (about 2/3 of the ceiling), which the day-1 number does not rank.
3. **Middle ground = L2 or L6,** whichever has the higher iron per hour per block among builds
   at 75% of the ceiling or more with a bill of at most 20 iron. Both use the tripwire's free
   calm moment and drop their golems out of range, which is what clears the 75% floor that L3
   cannot reach. L6 needs fewer eyes hidden, L2 makes three times the attempts.
4. **Fastest scaled = L5 of L4 modules with 4 villagers each,** ranked by iron per hour per
   module (about 450), the scaled build reported as N x the module. 4 villagers keep a cycle at
   600 gt when one of them sees the golem (93% instead of about 90%). Channels are 2 wide. The
   pitch is 27, and each module has its own kill cell and trigger. The module is searched first
   (pad strips, the drop, the trigger), then tiled.

The judge decides between estimates. Each farm is reported as a share of the ceiling.

## Must-haves (real use)

- **The villagers**:
  - each in its own closed 1-cell pod on its bed (the review's choice: one cell, so neither
    the spawn centre nor the 10-block and follow boxes move);
  - every cell they can stand in within Manhattan 1 of their HOME;
  - never within reach of the zombie (its attack box with its vehicle) or of a golem;
  - no damage;
  - under a roof that the sky cannot see through (sky light below 15), so lightning cannot
    turn them into witches. The roof must not become a golem spawn base: it is leaves (which
    LEGACY_IRON_GOLEM excludes, and which dim sky light to 14), or it sits at villager y+6 or
    higher, or it is capped with carpet or glass.
- **The zombie**:
  - a passenger of a boat or minecart (it never despawns and never walks);
  - its eye cell never sees the sky (it never burns) and is never in water (it never
    converts);
  - no drop ever comes within its pickup reach.
- **The golems**: no golem escapes the build, gets within melee reach of the zombie or touches
  its vehicle. Every golem dies in a kill cell within 1200 gt of its spawn.
- **The drops**: every iron ingot reaches the output chest (the judge's, at `port output`) or
  is still in the farm's containers at the end. None burns, none is left on the ground.
- **Materials**: survival only. No lava that can flow (static sources only), no water touching
  lava.
- **Duration**: it runs for days. Every villager starts sleeping at least once per game day.
- **Robustness**: over positions and rotations.

## Not goals

The judge does not check:

- weather: it runs in clear weather (rain only darkens the sky for daylight detectors, and
  lightning is covered by the roof must-have; L1's rain stop goes in its caption);
- natural monster spawning on the build (spawn-proofing);
- how the player brings the villagers and the zombie in;
- lag;
- chunk borders, as long as the build stays inside the simulated box.

## The judge (`circuit: mob-farm iron_ingot`, crates/circuit/src/studies/mobfarm.rs)

A registered study (`StudyDef` id `mobfarm`) that runs entirely on redsim, with its kind
`mob-farm <item>` so the next farms (gold, creeper, raid...) add a product, not a new study.
The design gives its villagers, the zombie and its vehicle as `entity:` lines (the zombie as
the vehicle's `Passengers`), its beds as blocks, and `port output` (the chest).

- **Runs**: each build runs seeds x game days at vanilla speed.
  - Every seed draws the villagers' follow-range bonus, their sensor phases and every other
    entity random.
  - **The steady state**: each run starts at a random day time in the day (10..12000), with
    each villager's HOME claimed (the bed in its pod) and its `LAST_SLEPT` at the end of the
    last REST (day time 10), as in a running farm, whose villagers last fall asleep near the
    end of the night. The first night is then fully ahead of it. The D measured days still
    cover every day time equally.
  - Each golem walks under a random policy: still, or one of 8 directions at goal speed 1.0 or
    0.6.

  Metrics:
  - iron/h per block, iron/h (mean, 95% interval), golems/h, seconds per golem;
  - share of the ceiling;
  - misses by cause;
  - blocks, bill, payback;
  - the rate under the worst policy: on the first build, 9 runs (still, the 8 directions at
    1.0), every golem of a run under that one policy, all 9 on the same seed so that the lowest
    is not biased low by noise. The card shows the lowest.
- **Builds**: 4 rotations x 2 positions. They agree when every build passes and the iron/h
  means are within 20% (`Agree::Rate`).
- **Golem exit (before the runs).** The positions a golem can stand on come from a flood: it
  starts at every spawn spot the farm offers, and each step runs the golem's own physics (its
  1.4 x 2.7 box, step height 1, a jump when blocked, falls). From every such position the
  golem's drift runs under still and the 8 constant inputs, alone in the built farm. Each of
  those runs must reach a kill (the golem dead) within 1200 gt without breaking a must-have.
  This is what proves that no input pins a golem: it catches cancelling currents, dead ends
  and side pockets. A summed-current test (a standable top whose touched cells sum to a current
  below 1e-5) only refuses early. The flood keeps every position a run visits on the ground as
  a new start, until none is new.
- **Checks** (the must-haves, as it runs; a failure names what, where and when):
  - a villager outside its cell or hurt;
  - the zombie hurt;
  - a golem outside the build, in reach of the zombie, touching a vehicle, or alive after
    1200 gt;
  - a drop that touches lava, comes within the zombie's reach, or is about to despawn;
  - iron lost at the end;
  - a villager 24000 gt without a sleep.
- **The restart probe**, for a design that claims it (the header `restart: yes`, L1): a run
  that starts with no `LAST_SLEPT` must spawn a golem within 2 game days. Designs without the
  claim report "restarts after a missed night: no" and carry the first-night caption.
- **Refusals** (before any run): what the model cannot judge, below.
- `mctas circuit run --sim` gives one build's runs. `mctas circuit sim` and `verify` run the
  whole judge (`verify` on redsim too; the game checks are the one-off validations below).
  `mctas mobfarm gen ...` generates the parametric layouts (pad, villagers, modules).

## The mob layer: what is exact, what is refused

New files in `crates/redsim/src/entity/mob/`. Existing redsim files get registrations and
hooks only, agreed with the boat-leads lane (06b77372) and the coordinator.

**Exact** (the same formulas in the same order as the 26.3 source):
- the entity list's order (the tick order that decides who counts 3 at T+600);
- LivingEntity physics (travel in air, water and lava, friction, step height, fluid
  pushing, entity pushing, cramming);
- damage and invulnerability (lava, fire, wall, magma, cramming, fall, drowning);
- death, drops and removal;
- the Brain: memory slots and their expiry order, sensors and their rates, start delays,
  activities, the schedule;
- `VillagerPanicTrigger` (with its 60 gt duration), `VillagerCalmDown`, `SleepInBed`, `WakeUp`,
  `UpdateActivityFromSchedule`;
- the hostile, nearest-living (box `R + 1.0`) and golem sensors, with COLLIDER line-of-sight
  rays;
- `spawnGolemIfNeeded` (every willing panicking villager, 10 attempts each),
  `wantsToSpawnGolem`, `golemSpawnConditionsMet`, `trySpawnMob` (LEGACY_IRON_GOLEM),
  `checkSpawnObstruction`, `isValidEmptySpawnBlock`, `isUnobstructed`;
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
- **Villager walking.** Each villager must stand in a closed 1-cell pod: the judge floods the
  cells its 0.6 x 1.95 box can reach (steps of 0.6, jumps of 1.25, falls; a wooden door counts
  as open, a climbable or fluid as passable) and refuses more than its own cell. In that cell:
  - every check holds at the corners of the region the box can take (the 0.6 box anywhere in
    the cell): the bed within 2.0 (`SleepInBed`), Manhattan 1 to HOME, the zombie out of reach;
  - the stand-up spot for both bed sides (the yaw that picks it is not modeled) lands in the
    cell.
- **Golem walking.** Not modeled as AI; instead the golem exit check above runs the drift from
  every position a golem can stand on, under still and the 8 constant inputs, and refuses the
  design unless every run reaches a kill within 1200 gt. Inputs that change direction are
  sequences of those 9 at positions the flood already covers. In water the golem's input (at
  most about 0.005/gt) never beats the current (0.014), so a design passes only where the
  currents carry it out from everywhere. The base runs give each golem a random policy, and
  the rate is also reported under the worst constant policy (above).
- **Golem targeting** (refused if a golem gets within melee reach of the zombie) and
  **zombie attacks** (refused if a villager's cell meets the zombie's attack box with its
  vehicle).
- **POI claiming** (refused if a pod holds no bed head within its cell's reach, or a bunk bed).
- **Gossip and trading.** The only gossip call is `TradeWithVillager.tick` (IDLE and MEET
  packages). It needs an `INTERACTION_TARGET` from `InteractWith`: a villager within 8 in
  `NEAREST_VISIBLE_LIVING_ENTITIES`, whose test is a COLLIDER ray. Then `distanceToSqr <= 5`,
  both 1200 gt cooldowns, and 5 willing villagers. The code-true rule: refused when a villager
  can see another within 8 (an eye-to-eye ray, standing or sleeping, from the corners of its
  cell) while 5 or more villagers share its 10-block box. Clusters of 4 or fewer, or glass
  between the pods, pass.
- **Job sites and breeding** (refused if a workstation is in the build).
- Raids, bells, weather and natural spawning (not judged, listed in docs/coverage.md).

## Validation against the game (one-off checks, recorded here)

1. **Spawn spots**: three platforms, with real villagers (a `LAST_SLEPT` memory in their summon
   data) and a zombie in a boat. Every golem's spawn cell over many cycles goes into a
   histogram, compared with redsim's for the same build (chi-square).
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
- Below it, in practice:
  - the panic gap, when every eligible villager is in its gap tick at once;
  - the sightings: at least about 47 gt in range for a pad and a drop;
  - the failed marks of a small pad.

  The judge reports each. The ceiling is a derivation; the judge's measured numbers are the
  authority, and this section is revised if the measured best stays well below it.

## Changelog (revision 2, after the G1 review)

| point | what changed | where |
|---|---|---|
| 1 | Every willing panicking villager tries (10 x N attempts); the 4x4 pad is 82% with three villagers; tick order at T+600 (only V and later villagers count 3); the panic gap costs only when every eligible villager is in its gap tick; the citation fixed (empty entry map, test in `start`/`canStillUse`) | Who spawns; Trade-offs (spawn spots) |
| 2 | Only the villagers that see the zombie need the hide; new layout L6 (one scared villager) | Sleep; Trade-offs; L6; Decision |
| 3 | Transit in water (0.056 terminal, 0.036-0.076 with walking), 1.4-wide golems need 2-wide channels, at least ~47 gt in range, 90% with 3 villagers and 95% needs 4 | The golem; Trade-offs; L4; Decision |
| 4 | The follow box is `R + 1.0`, pitch 26.9 (pad reach + 18.84), the stacking rule (Q < 25.8), per-module kill cells or triggers | Golem detector; Trade-offs (scaling); L5 |
| 5 | The gossip refusal replaced by the code-true rule (eye-to-eye ray within 8 while 5+ share a box); 4 villagers per cluster in L4/L5 | Refused; L4; L5 |
| 6 | Golem walking: an exit current on every standable cell (static refusal), plus the rate under the worst constant policy; villagers in 1-cell pods | Refused; Must-haves; The judge |
| 7 | The flood's passability (doors, climbables, fluids) and continuous-region checks; the zombie's attack box with its vehicle (1.52 from a boat's centre); the HOME-loss refusal (Manhattan 1); L3 with closed 1-cell pods | Sleep (keeping HOME); The zombie; Refused; L3 |
| 8 | Tripwire and daylight detector cited; rain stops L1; lightning; decision: clear weather as a not-goal, an opaque-roof (sky light < 15) must-have for the villagers, L1 measured in clear weather | Calm devices; Not goals; Must-haves; L1 |
| 9 | The number per tier (day 1: lowest bill; middle: iron/h per block under 20 iron; fastest: iron/h of the build and per module); the board ranks by iron/h per block; L3's bill includes the bucket; the decision re-argued | The number; Six layouts; Decision |
| — | Also fixed: bottom slabs and 3 px trapdoors are solid (the scan stops there, the attempt is wasted); carpets are not | Where a golem spawns |

## Changelog (revision 3, after the G1 re-review)

| point | what changed | where |
|---|---|---|
| 1 | Golem exit: the static current rule is gone. A flood of the positions a golem can stand on (from every spawn spot, its own physics: 1.4 box, step 1, jumps, falls), and from each the drift under still and the 8 constant inputs must reach a kill within 1200 gt. The summed-current test (below 1e-5) is only an early refusal. The 9 worst-policy runs share one seed | The judge (golem exit, metrics); Refused (golem walking) |
| 2 | Deadlock: runs start in the steady state (a random day time in 10..12000, `LAST_SLEPT` at the end of the last REST, day time 10). L3 wires every spawn floor with one string line (no extra iron), so no night's golems can all miss the wire. Every golem-triggered layout's caption says the player hides the zombie by hand on the first night. The restart probe checks only designs that claim it (`restart: yes`, L1): with no `LAST_SLEPT`, a golem within 2 days | The judge (runs, restart probe); Six layouts (L3, the deadlock note) |
| 3 | Tiers: day 1 = the lowest bill (no quartz, at least 25% of the ceiling); middle = iron/h per block among builds at 75% of the ceiling or more with a bill of at most 20 iron (the 75% floor separates L3, capped near 2/3 by its kill in place); fastest = iron/h per module, the scaled build reported as N x the module | The number; Decision |
| note | The roof cannot be a spawn base: leaves (excluded by LEGACY_IRON_GOLEM, sky light 14 under them), or at villager y+6 or higher, or capped with carpet or glass | Must-haves |
| — | L3's sight blocker is a powered wooden trapdoor: its bill drops to 9 iron (no piston) | Six layouts; Decision |
