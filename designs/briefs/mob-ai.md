# Mob AI in redsim (design note, gate G1)

mob-AI lane (8a0201dc), 2026-11-08. New simulator scope, so it goes through G1
([docs/process.md](../../docs/process.md)). Every rule below is read in the decompiled 26.3 server
(`mctas src show <Class>`); no outside design.

## Why

vjeux, 2026-11-07: "build the infrastructure to build and manage farms using redsim ... I will be
asking you to build all the top ones in the future", "We can't use the game code, it's way too slow
in java"; 2026-11-08: "We should be tick exact everywhere", "It's all about tradeoffs".

The mob layer (`crates/redsim/src/entity/mob/`) models physics, damage, the villager brain's golem
and sleep rules and natural spawning, but no mob walks: the iron farm judge bounds golems with 9
constant walking policies and refuses villagers that can leave a 1-cell pod; the terrain checks
refuse any farm a mob could reach by any path (`hostile.rs` floods C1-C16). Every top farm after
the iron farm needs mobs that move: hostiles walking to villagers, players and doors; villager
breeders and trading halls (villagers walking to beds, job sites, each other); golems targeting;
mob towers (hostiles wandering off edges). And the tradeoffs vjeux wants measured ("zombies reach
a villager once every N days") are outcomes of walking, which floods can only refuse.

## The number

Zero differences against the deterministic bridge (tick-exact lane 84579e98's compare: per entity
and per tick pos, delta, yRot, xRot, yHeadRot, yBodyRot, onGround, horizontalCollision, health,
the running goals, the navigation's path, the random state) on the six fixtures below, each for
thousands of ticks, in the void and in the terrain samples. Then: path searches per second and
microseconds per mob per tick.

## Scope: the classes modeled, method by method

Same formulas, same order, same draws from the mirrored streams (the entity random seeded
`mixStafford13((s ^ 0x656e74697479) + (n + 1) * 0x9E3779B97F4A7C15)` after the constructor's
draws; `ShufflingList` keys per tick-exact's registry). Floats stay floats (`Node.g/h/f`,
`Mth.sqrt`), `Mth.atan2`, `Mth.sin/cos` tables, HotSpot's `Math.sin/cos` (`jmath.rs`).

1. **Path finding** (`net.minecraft.world.level.pathfinder`):
   - `PathType` (27 types, default malus), `Node` (hash `createHash`, `distanceTo` in float),
     `Target` (`updateBest`), `BinaryHeap` (insert, pop, changeCost, upHeap/downHeap as written:
     the tie order is the game's), `Path` (`advance`, `getEntityPosAtNode`, `truncateNodes`,
     `replaceNode`, `distToTarget`, `sameAs`).
   - `PathFinder.findPath`: open set, `maxVisitedNodes x multiplier` budget, `reachRange` by
     Manhattan distance, `maxPathLength` cut, heuristic x 1.5 (`FUDGING`), several targets in
     `HashMap` order (Java's spread hash of `Node.hash`), the `min` over reached / best paths.
   - `NodeEvaluator` + `WalkNodeEvaluator`: `getStart` (floatable fluids, the four box corners),
     `getNeighbors` (4 sides, then diagonals by `isDiagonalValid`), `findAcceptedNode` (jump
     height 1.125, `tryJumpOn` with its box test, `tryFindFirstNonWaterBelow`,
     `tryFindFirstGroundNodeBelow` with `getMaxFallDistance`, `getClosedNode` for partial
     collisions, `canReachWithoutCollision`), `getPathTypeOfMob` (the box's `EnumSet` in ordinal
     order, FENCE / UNPASSABLE_RAIL, highest malus, BIG_MOBS_CLOSE_TO_DANGER for wide mobs),
     `getPathTypeWithinMobBB` (doors opened by mobs that can, rails), `getPathTypeStatic`,
     `checkNeighbourBlocks`, `getPathTypeFromState` (trapdoors, powder snow, cactus and berry
     bushes, honey, cocoa, wither roses and speleothems, lava, burning blocks, doors, rails,
     leaves, fences, walls, closed gates, `isPathfindable(LAND)`, water). Per-search caches
     (`pathTypesByPosCacheByMob`, `collisionCache`) and the level's `PathTypeCache`
     (invalidated by `ServerLevel.sendBlockUpdated`) are transparent and modeled as plain caches.
   - Block facts come from redsim's own blocks (`Block`) and, for terrain states, the natural-state
     table (`terrain/blocks.tsv`: classes, tags, collision boxes, `isPathfindable`), with the
     columns this needs added through `mctas terrain blockinfo` if any is missing.
   - Later, as the farms need them: `SwimNodeEvaluator` (drowned), `FlyNodeEvaluator` (bats,
     phantoms), `AmphibiousNodeEvaluator`.
2. **Navigation** (`ai.navigation`): `PathNavigation` (`createPath` with its region, radius and
   `requiredPathLength` / FOLLOW_RANGE x 16 node budget, `moveTo`, `tick`, `followThePath` with
   `maxDistanceToWaypoint`, `shouldTargetNextNodeInDirection`, `doStuckDetection` every 100 gt
   and the waypoint timeout, `recomputePath` / `shouldRecomputePath` with the 20 gt rule,
   `trimPath`, `getGroundY`, `canCutCorner`), `GroundPathNavigation` (`canUpdatePath`,
   `getTempMobPos`, `findSurfacePosition`, avoid-sun truncation, `getSurfaceY`),
   `WallClimberNavigation` (spiders). The level hook: a block change whose collision shape changes
   (`sendBlockUpdated`) asks every navigating mob `shouldRecomputePath`.
3. **Controls** (`ai.control`): `MoveControl` (MOVE_TO / STRAFE / JUMPING / WAIT, `rotlerp` 90,
   the jump test against `maxUpStep` and the block's collision top, `setSpeed` -> `zza`),
   `LookControl` (`lookAtCooldown`, `rotateTowards`, `clampHeadRotationToBody`),
   `JumpControl`, `BodyRotationControl` (`clientTick` from `Mob.tickHeadTurn`: head stable time,
   facing forward). They feed the existing `LivingEntity.aiStep` / `travel` (`living.rs`).
4. **Goals** (`ai.goal`, `ai.goal.target`, `ai.targeting`, `ai.sensing`, `ai.util`):
   - `GoalSelector` (insertion order of `ObjectLinkedOpenHashSet`, locked flags, disabled flags,
     `tick` vs `tickRunningGoals` on `(tickCount + id) % 2`, `requiresUpdateEveryTick`,
     `reducedTickDelay` / `adjustedTickDelay`), `WrappedGoal`, `Mob.updateControlFlags` every
     5 gt, `Mob.serverAiStep`'s order (sensing, target selector, goal selector, navigation,
     `customServerAiStep`, move, look, jump), `Sensing` (per-tick sight cache).
   - `TargetingConditions` (range x visibility percent, the sight test through `Sensing`),
     `TargetGoal` (`canContinueToUse`, unseen memory, the reach cache with its draw),
     `NearestAttackableTargetGoal` (the `nextInt(randomInterval)` gate, `getNearestEntity` over
     `getEntitiesOfClass` in section order), `HurtByTargetGoal` (alert others),
     `DefendVillageTargetGoal`, `ResetUniversalAngerTargetGoal`.
   - `DefaultRandomPos`, `LandRandomPos`, `RandomPos` (`generateRandomDirection`,
     `generateRandomPosTowardDirection`, `moveUpOutOfSolid`, `moveUpToAboveSolid`, the
     walk-target-value scan), `GoalUtils`.
   - Zombie (`Zombie.registerGoals` / `addBehaviourGoals`): ZombieAttackTurtleEggGoal
     (MoveToBlockGoal's start delay draw and scan), SpearUseGoal, ZombieAttackGoal /
     MeleeAttackGoal, MoveThroughVillageGoal (POIs), WaterAvoidingRandomStrollGoal, LookAtPlayerGoal,
     RandomLookAroundGoal, BreakDoorGoal / DoorInteractGoal on Hard (`setCanBreakDoors`), the
     targets (players, villagers, golems, baby turtles).
   - Skeleton (`AbstractSkeleton`): RestrictSunGoal, FleeSunGoal, AvoidEntityGoal (wolves),
     RangedBowAttackGoal (and the melee swap), stroll, looks, targets. Arrows are an entity of their
     own: modeled when a farm needs a skeleton to shoot (a mob tower does not).
   - Creeper: FloatGoal, SwellGoal (and `Creeper.tick`'s swell and explosion), AvoidEntityGoal
     (cats, ocelots), MeleeAttackGoal, stroll 0.8, looks, targets.
   - Spider: FloatGoal, LeapAtTargetGoal, SpiderAttackGoal, SpiderTargetGoal (daylight), stroll,
     looks, WallClimberNavigation and the climbing flag.
   - Iron golem (`IronGolem.registerGoals`): MeleeAttackGoal (and `doHurtTarget`'s knockback),
     MoveTowardsTargetGoal, MoveBackToVillageGoal, GolemRandomStrollInVillageGoal (POIs, villager
     search), OfferFlowerGoal, looks, DefendVillageTargetGoal, HurtByTargetGoal, the anger goals.
5. **The villager behaviours that walk** (`ai.behavior`), in the brain that ironfarm already
   models (`villager.rs` keeps the order and calls them): `MoveToTargetSink` (WALK_TARGET ->
   path, CANT_REACH_WALK_TARGET_SINCE, the `remainingCooldown` and its draw),
   `SetWalkTargetFromBlockMemory`, `AcquirePoi` (the POI search, `findPathToPois` with its
   batch and per-POI retry back-off, the ticket), `ValidateNearbyPoi`, `StrollAroundPoi`,
   `StrollToPoi`, `VillageBoundRandomStroll`, `RandomStroll`, `SetWalkTargetAwayFrom`,
   `InsideBrownianWalk`, `GoToClosestVillage`, `SetClosestHomeAsWalkTarget`,
   `SetWalkTargetFromLookTarget`, `InteractWith`, `LookAtTargetSink`, `SetEntityLookTarget`,
   `DoNothing`, `RunOne` / `GateBehavior` / `ShufflingList` (the registry's seeds),
   `InteractWithDoor`, `Swim`. The POI index (`PoiManager`: beds, job sites, meeting points, with
   free tickets and `findAllClosestFirstWithType`) is shared with ironfarm.

Not in scope until a farm needs it: trading, gossip spread beyond what `TradeWithVillager` draws,
raids, item pickup beyond the zombie's (refused while modeled flags say so), projectiles.

## Module layout (agreed with ironfarm 1372cc16)

`crates/redsim/src/entity/mob/ai/`, all new: `mod.rs` (`MobAi`: navigation, controls, selectors,
sensing, head/body yaw; `server_ai_step`), `path/` (one file per class: node, heap, path,
path_type, walk, finder, navigation), `control.rs`, `target.rs`, `goal/` (selector + one file per
goal family), `behavior/` (the villager's). Hooks only elsewhere: `mob/mod.rs` (the module, an
`ai` field set in `add`), `living.rs` (`server_ai_step` -> `ai::server_ai_step` for goal mobs,
`tickHeadTurn`), `world.rs` (the `sendBlockUpdated` recompute hook, a no-op without navigating
mobs). The AI is off unless a world turns it on, so every existing judge run stays byte-identical
until the owner of a judge flips it.

## Order of implementation

1. Path finding + navigation, validated alone (step 6's path probe).
2. Controls, GoalSelector, the zombie set (stroll, looks, targets, melee, village, doors): fixture
   1 and 2.
3. Creeper (fixture 3), iron golem (fixture 4), skeleton, spider.
4. Villager walking behaviours + POIs: fixtures 5 and 6.
5. The judges: ironfarm switches the golem walk policies and the villager reach floods to the
   simulation, terrain turns its exposure lines into measured expected losses (with each owner).

## Validation

- **Path probe (deterministic, no randoms):** a bridge op that builds a mob (not added to the
  world) at a position and returns `getNavigation().createPath(target, reach)` node by node (x, y,
  z, type, cost malus) with `reached` and the target; redsim answers the same query. A fuzzer over
  random block worlds (walls, slabs, fences, doors, water, lava, trapdoors, rails, carpets, beds,
  cliffs) and over the terrain samples, every mob size: 0 differences required. This proves
  navigation before the full compare exists.
- **Full compare** (tick-exact's `mctas mobfarm fixture`): 1. a zombie pathing to a villager across
  obstacles; 2. a zombie breaking a door on Hard; 3. a creeper approaching a survival player;
  4. an iron golem chasing a zombie; 5. villagers walking to their beds at dusk; 6. AcquirePoi
  claiming a bed along a path. Each thousands of ticks, in the void and in `designs/terrain/*.mcterrain`.
  Zero differences, kept as simulator-tier tests (`mctas test`) with the recorded traces as
  regression fixtures; `cargo test -p redsim` holds known answers (path shapes, malus, door rules).
- **Oracles:** a walking mob never enters a cell the terrain lane's flood excludes
  (`hostile.rs`), never leaves a pod that `refuse.rs` calls closed.

## Performance

The game allocates a `Node` per visited cell in a hash map and a fresh `PathNavigationRegion`
per search; redsim keeps the same visit order with an arena (`Vec<Node>` indexed by a
position-keyed map reused between searches), per-search path-type and collision caches, and the
block reads through redsim's world. Measured and reported: path searches per second (zombie
across a 30-block terrain sample, golem in a farm), microseconds per mob per tick (stroll,
chase, idle), and a farm run's slowdown with AI on. Target: a mob tick under 5 us idle and a
search under 0.5 ms at the default 560-node budget (35 x 16), so a 3-day farm run with tens of
mobs stays seconds.

## What stays refused, and how it closes

Until a behaviour is exact on its fixture, the judge keeps refusing what depends on it (the golem
policies, the pod flood, the terrain floods). Each refusal falls when its fixture reaches zero
differences and the owner of that judge switches it; coverage.md's mob-layer rows move from ✗/~
to ✓ with the fixture as evidence.
