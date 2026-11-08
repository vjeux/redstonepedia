# Mob AI in redsim (design note, gate G1)

mob-AI lane (8a0201dc, child of 0e7eb68a), 2026-11-08. New simulator scope, so it goes through G1
([docs/process.md](../../docs/process.md)). Every rule below is read in the decompiled 26.3 server
(`mctas src show <Class>`); no outside design. Revision 2 answers the G1 review (CHANGES, 9 points;
changelog at the end).

## Use

A farm designer runs a farm whose mobs walk (hostiles to villagers, villagers to beds and job
sites, golems to zombies, traders to players) in redsim, thousands of runs in the time the game
takes for one, and gets the game's own outcome tick for tick.

## The number

1. **Exactness**: the share of the fixtures' ticks that match the deterministic bridge before the
   first difference (per entity and tick: pos, delta, yRot, xRot, yHeadRot, yBodyRot, onGround,
   horizontalCollision, health, the running goals, the navigation's path, the random state).
   Ceiling: 100% (zero differences over every fixture's whole run).
2. **Speed**: microseconds per mob per tick in redsim against the game's own cost for the same
   scene (`mctas mobai bench-game` / `bench-redsim`: 50 mobs on a closed 64 x 64 stone arena, 2000
   ticks after a 100-tick warm-up, minus the empty arena), reported as the speedup over the game.

## Ceiling

- Exactness: 100% (zero differences).
- Speed: an exact AI cannot cost less than the mob layer without it (the physics, damage and brain
  every mob already runs, plus the random draws the AI must make). Measured today on this box
  (devvm48543, idle):

  | mob | the game (Java, its own AI) | redsim today (no walking) | speedup ceiling |
  |---|---|---|---|
  | zombie | 13.3 us | 1.90 us | 7.0x |
  | iron golem | 18.5 us | 3.32 us | 5.6x |
  | villager | 36.8 us | 2.29 us | 16x |

  A layout's speed is reported as a share of that ceiling: (redsim today) / (redsim with the AI).

## The mechanics, from the game code (26.3)

What every layout has to reproduce; the order and the draws are the hard part.

- **Mob.serverAiStep** (`Mob.java` 715): `noActionTime++`, `Sensing.tick` (the per-tick sight
  cache), then on `(tickCount + getId()) % 2 == 0` (or `tickCount <= 1`) `targetSelector.tick()`
  and `goalSelector.tick()`, else only `tickRunningGoals(false)`; then `navigation.tick()`,
  `customServerAiStep`, `moveControl.tick()`, `lookControl.tick()`, `jumpControl.tick()`.
  `Mob.baseTick` draws `random.nextInt(1000)` every tick (ambient sound); `Mob.tick` runs
  `updateControlFlags` every 5 ticks; `Mob.tickHeadTurn` = `BodyRotationControl.clientTick`.
- **GoalSelector** (`GoalSelector.java`): goals in insertion order (`ObjectLinkedOpenHashSet`);
  stop running goals that lost a flag or `!canContinueToUse`; drop locked flags of stopped goals;
  start each idle goal whose flags it can take (`canBeReplacedBy`: interruptable and lower
  priority number) and whose `canUse` holds, stopping the holders; tick running goals
  (`requiresUpdateEveryTick` ones on the odd ticks too); `reducedTickDelay` = ceil(t / 2).
- **The draws** every goal makes from the entity random, in that order:
  `NearestAttackableTargetGoal.canUse` `nextInt(reducedTickDelay(10))`; `LookAtPlayerGoal`
  `nextFloat() >= 0.02`, `start` `nextInt(40)`; `RandomLookAroundGoal` `nextFloat() < 0.02`,
  `start` `nextDouble()`, `nextInt(20)`; `RandomStrollGoal` `nextInt(reducedTickDelay(120))` then
  `LandRandomPos` / `DefaultRandomPos` (`RandomPos.generateRandomDirection`: three `nextInt`,
  ten tries); `MeleeAttackGoal.tick` `nextFloat() < 0.05`, `nextInt(7)`; `TargetGoal.canReach`
  `nextInt(5)`; `MoveToBlockGoal` `nextInt(200)`; the brain's `ShufflingList` from the
  registry's seeds.
- **PathFinder** (`PathFinder.java`): A* over `Node`s with `g, h, f` in float, heuristic x 1.5,
  `maxVisitedNodes` = FOLLOW_RANGE x 16 x multiplier, `reachRange` by Manhattan distance,
  `maxPathLength` on the walked distance; the open set is the game's own `BinaryHeap` (ties go
  as its up/down-heap moves them); several targets in `HashMap<Target, BlockPos>` order; the
  result is the `min` path by node count (reached) or by distance then node count.
- **WalkNodeEvaluator** (`WalkNodeEvaluator.java`): `getStart`, the 4 sides then the diagonals,
  `findAcceptedNode` (jump height max(1.125, step), `tryJumpOn` and its box test, water and
  ground scans below, `getMaxFallDistance` = 3 or more with a target, partial collisions of
  fences and closed doors), `getPathTypeOfMob` over the mob's cells (an `EnumSet` in ordinal
  order), `getPathTypeStatic` (the block below, the 26 neighbours for fire, damage and water),
  `getPathTypeFromState` (trapdoors, powder snow, cactus, honey, cocoa, wither roses and
  speleothems, lava, burning blocks, doors, rails, leaves, fences, walls, closed gates,
  `isPathfindable(LAND)`, water). The level's `PathTypeCache` (4096 slots, `HashCommon.mix`) is
  invalidated only by `ServerLevel.sendBlockUpdated` (a `setBlock` with flag 2), so a piston's
  flag-68 moving block can leave a stale entry: the game's behaviour, kept.
- **PathNavigation** (`PathNavigation.java`, `GroundPathNavigation.java`): `createPath` in a
  `PathNavigationRegion` of radius maxPathLength + 8 (16 for an entity target; air outside it),
  `moveTo`, `followThePath` (waypoint distance from the width, `shouldTargetNextNodeInDirection`),
  stuck detection every 100 ticks and the waypoint timeout, `recomputePath` (20-tick rule) when a
  block's collision shape changes near the path (`ServerLevel.sendBlockUpdated` asks every
  navigating mob `shouldRecomputePath`), `findSurfacePosition`, avoid-sun truncation.
- **Controls**: `MoveControl` (MOVE_TO / STRAFE / JUMPING / WAIT, `rotlerp` 90, the jump test,
  `setSpeed` -> `zza`), `LookControl` (cooldown 2, `rotateTowards`, head clamped to the body while
  navigating), `JumpControl`, `BodyRotationControl` (15 degree head stability, 10 + 10 ticks to
  face forward).
- **Targeting** (`TargetingConditions`, `TargetGoal`, `Sensing`): range x visibility percent,
  line of sight through the per-tick cache, the unseen memory, the reach cache.
- **The mobs' goal sets**: `Zombie.registerGoals` / `addBehaviourGoals` (turtle eggs, spear,
  attack, village, stroll, looks; targets: players, villagers, golems, baby turtles; `BreakDoorGoal`
  only with `canBreakDoors` and on Hard); `Creeper.registerGoals` (float, swell, avoid cats,
  melee, stroll 0.8, looks); `IronGolem.registerGoals` (melee, move towards target, back to
  village, village stroll, flower, looks; defend village, hurt by, anger, enemies but creepers);
  `AbstractSkeleton.registerGoals` (restrict and flee sun, avoid wolves, bow or melee, stroll,
  looks, targets); `Spider.registerGoals` (float, leap, attack, stroll, looks, targets in the dark;
  `WallClimberNavigation`); `WanderingTrader.registerGoals` (despawn drink, invisibility at night,
  trade, avoid zombies and illagers, panic, wander to target, stroll, looks) and `TraderLlama`
  (follow the trader on its lead, `LlamaFollowCaravanGoal`); the pillager
  (`Pillager.registerGoals`: `PatrollingMonster.LongDistancePatrolGoal` with its leader and
  followers and `findPatrolTarget`, `RangedCrossbowAttackGoal`, `Raider` targets: villagers,
  iron golems, wandering traders, players).
- **Arrows** (`AbstractArrow`, `Arrow`), for the skeleton's bow and the pillager's crossbow: the
  shot (`getProjectile`, `shootFromRotation` / `shoot` with its spread draws), the flight (drag,
  gravity, the hit scan), the hit (damage, knockback, sticking in blocks, pickup off).
- **The villager's walking behaviours** (`VillagerGoalPackages`): `MoveToTargetSink` (WALK_TARGET
  -> path, `CANT_REACH_WALK_TARGET_SINCE`), `SetWalkTargetFromBlockMemory` (HOME 1, JOB_SITE 9,
  MEETING_POINT 6), `AcquirePoi` (the POI search, `findPathToPois` with its batch and per-POI
  back-off, the ticket), `ValidateNearbyPoi`, `StrollAroundPoi`, `StrollToPoi`,
  `VillageBoundRandomStroll`, `SetWalkTargetAwayFrom` (PANIC), `InsideBrownianWalk`,
  `GoToClosestVillage`, `SetClosestHomeAsWalkTarget`, `SetWalkTargetFromLookTarget`,
  `InteractWith`, `LookAtTargetSink`, `SetEntityLookTarget`, `DoNothing`, `RunOne` / `GateBehavior`,
  `InteractWithDoor`, `Swim`.

## The trade-offs

- **Exactness against speed.** Exactness needs the game's visit order, float arithmetic and every
  draw; that rules out shortcuts (flow fields, cached paths, skipped goals). Speed comes only from
  how the same work is done (no allocation, cheap block reads), never from doing less of it.
- **Breadth against depth.** Five hostile and golem types, the villager's behaviours and the
  trader are many classes; each is exact only when its fixture is. So the work is phased (the
  decision): a type's goals count only once its fixture reaches zero differences, and until then
  the judge keeps refusing what depends on it, as today. Depth first on the path finder, which
  every type shares.
- **Arena and reused buffers against the verbatim maps.** The game allocates a `Node` per cell in
  an `Int2ObjectOpenHashMap`, a fresh region, `EnumSet`s and a `HashMap<AABB, Boolean>` per search.
  An arena keyed by the node's position, cleared per search, gives the same nodes (the hash map's
  order is never observed: `nodes` is only looked up), with no stale state as long as every
  search starts from a cleared arena (a debug check asserts it). The only map whose order IS
  observed is `PathFinder`'s target map (several targets): that one is modeled as Java's
  `HashMap` iteration order.
- **The level's PathTypeCache: plain cache or faithful.** A plain recompute is always fresh; the
  game's cache can be stale after a flag-68 piston move. Faithful costs a 4096-slot array and a
  hook in `World::set_block` (flag 2 only), and keeps that staleness: chosen, since a farm with
  pistons next to walking mobs would otherwise diverge.
- **One fixture per behaviour family against one big scene.** Small fixtures localise a
  difference; a long terrain fixture finds the interactions. Both run.

## Five layouts (implementation strategies)

Estimates before the work (effort for this lane, 1 day = a working day); speed in us per zombie
per tick against the game's 13.3 (ceiling 1.90 = 7.0x); exactness as the share of fixture ticks
before the first difference.

| # | layout | effort | speed (zombie) | % of the speed ceiling | exactness | constraints |
|---|---|---|---|---|---|---|
| **A** | **The obvious design**: a verbatim transliteration of every class (a `HashMap` of boxed nodes per search, `Vec`s per neighbour call, a region object per search, `HashSet`s for the type sets and the collision cache) | 1.5 days | ~6-10 us (1.3-2.2x the game: Java's allocation pattern without its JIT and escape analysis) | 20-30% | 100% (by construction, once each class is checked) | none on scenes; slow searches in big terrain |
| **B** | A, then each hot spot rewritten with the trace held identical: node arena keyed by position, the heap over indices, neighbour arrays on the stack, a per-search path-type grid instead of a hash map, block facts precomputed per state (terrain rows and redsim blocks to one `PathType` table) | 2 days (A + 0.5) | ~3-4 us (3.3-4.4x) | 50-65% | 100% | every optimization must keep the trace identical (the path probe and the fixtures re-run after each) |
| C | An approximate walker: A* or BFS with the malus table but its own tie-breaking, a simple steering controller, goals reduced to "approach the nearest target" | 1 day | ~2.5 us (5x) | ~75% | <1% (the first random stroll target or heap tie differs within tens of ticks) | every result is "approximately the game": no card may cite it; it repeats the policies' weakness the floods were built to avoid |
| D | The game's own AI through the bridge: redsim asks a JVM for each mob's inputs every tick | 0.5 day | >13.3 us plus a round trip per tick (<1x) | <14% | 100% | the JVM per run: exactly what vjeux ruled out ("way too slow in java") |
| E | Precomputed flow fields: per static scene and target cell, a distance map the mobs descend | 1 day | ~2 us (6.5x) | ~95% | ~0% (A* with a 1.5 heuristic and a node budget does not take shortest paths; doors, pistons, sun, random targets change the field) | static scenes only |

(Trace replay, a sixth idea: record the game's trajectories and replay them; exact only on the
recorded seeds and designs, so useless for a judge that runs new seeds and variants.)

## Decision

**B, built as A first.** Only A, B and D are exact (number 1 at its ceiling); D gives up number 2
entirely, A gives up most of it. B is A plus optimizations that never change the visit order, so
A is the bar (the process' G2): it is written first, checked on the path probe and the first
fixtures, its speed measured; then each B change lands only with the probe and the fixtures
unchanged. C and E fail the must-have that defines the scope ("tick exact everywhere").

Phases (each type's goals count when its fixtures are exact; until then the judge refuses as
today):

1. Path finding + navigation: the path probe (0 differences).
2. Controls, GoalSelector, targeting, the zombie set: fixtures 1 and 2.
3. Creeper and iron golem: fixtures 3 and 4.
4. The villager's walking behaviours and POIs: fixtures 5, 6, 9 and 10.
5. Skeleton, spider, wandering trader and its llamas, pillager patrols, arrows: fixtures 7, 8, 11
   and 12.
6. The judges, with their owners: ironfarm switches the golem walk policies and the villager reach
   floods to the simulation; terrain turns its exposure lines into measured expected losses.

## Must-haves (from real use)

- **The AI is off by default**: a world turns it on; with it off every existing run is byte
  identical (the mob layer's known answers in `entity::mob::tests`, the iron farm judge's runs, the
  redsim fixtures and `door selftest` re-run unchanged on the branch: the commit line says so).
- **Deterministic**: the same world and seed give the same trace, run after run.
- **No draw outside the mirrored streams**: every draw comes from the entity random seeded as the
  bridge seeds it (tick-exact's registry, below) or the level random, in the game's order; none
  from a stand-in.
- **Refuse, never guess**: a behaviour reached but not modeled (an arrow shot, a trade, a raid) is
  a model error that fails the run, never a silent difference.

## Not goals (what the judge must not demand)

- Wall-clock parity with Java or with the game's tick rate.
- Exactness of the deferred scope: projectiles other than arrows (tridents, potions, fireballs),
  trading and gossip beyond the draws `TradeWithVillager` makes, raids (the raid manager; patrols
  are in), item pickup beyond the zombie's, flying and swimming navigation until a farm needs it.
- **Shooters before phase 5's arrows**: the skeleton's and pillager's movement, sun and wolf
  avoidance, patrolling, strolling, looking and target selection are exact from their own
  phase; `RangedBowAttackGoal` / `RangedCrossbowAttackGoal` are exact up to the first
  `performRangedAttack`, which spawns an arrow: until arrows are exact (phase 5, fixtures 7 and
  12) the run stops there with a model error. A mob tower's skeletons (no target in sight) never
  reach it.
- Debug-only state (`Path.debugData`, the path debug subscription).

## Validation

- **Path probe** (deterministic, no randoms): a bridge op builds a mob (not added to the world) at
  a position and returns `getNavigation().createPath(target, reach)` node by node (x, y, z, type,
  cost malus) with `reached` and the target; redsim answers the same query. A fuzzer over random
  block worlds (walls, slabs, fences, gates, doors, trapdoors, water, lava, magma, rails, carpets,
  beds, cliffs, leaves) and the terrain samples, every mob size: 0 differences.
- **The fixtures** (tick-exact's compare, each run thousands of ticks in the void and in
  `designs/terrain/*.mcterrain`, zero differences, kept as simulator-tier tests with their traces
  as regression files):
  1. a zombie pathing to a villager across obstacles;
  2. a zombie breaking a wooden door on Hard;
  3. a creeper approaching a survival player and swelling;
  4. an iron golem chasing and killing a zombie;
  5. villagers walking to their beds at dusk;
  6. `AcquirePoi` claiming a bed along a path;
  7. a skeleton at dawn fleeing the sun into shade and strolling (no target);
  8. a spider climbing a wall in the dark to an iron golem;
  9. a village day: villagers with job sites, beds and a bell through IDLE, WORK and MEET;
  10. panicking villagers fleeing a zombie (`SetWalkTargetAwayFrom`);
  11. a wandering trader and its two llamas wandering by day, drinking invisibility at night,
      fleeing a zombie;
  12. a pillager patrol (a leader and three followers) walking to its patrol target near a
      player, then shooting an iron golem with crossbows (arrows in flight and on hit).
- **Oracles**: a walking mob never enters a cell the terrain lane's flood excludes
  (`crates/redsim/src/entity/mob/hostile.rs`) nor leaves a pod `refuse.rs` calls closed.
- **Known answers** in `cargo test -p redsim` (path shapes, malus, door and fence rules).

The cross-lane contracts, as checkable files once pushed:
- tick-exact (`explore/tickexact`, first push today): the entity random of the n-th entity to
  enter a sandbox of seed s is `setSeed(mixStafford13((s ^ 0x656e74697479) + (n + 1) *
  0x9E3779B97F4A7C15))` with the constructor's draws redone (UUID, yaw, sensor delays);
  `ShufflingList` seeds `mixStafford13(S_n + hash(key) * 0x9E3779B97F4A7C15)`; the compare
  (`mctas mobfarm fixture <file>`) checks the fields of number 1.
- ironfarm: the layout below, in `crates/redsim/src/entity/mob/` (its files untouched but for the
  hooks named).

**Fallback if the harness slips**: the path probe proves navigation only. If tick-exact's
compare is not on main when a phase is ready, I add a minimal one to `mctas mobai` (the bridge's
`entities` op per tick: pos, motion, yaw, pitch, head and body yaw, plus the entity random's
state; the registry's seeding through the same bridge hook), enough for my fixtures.

## Module layout

`crates/redsim/src/entity/mob/ai/`, all new: `mod.rs` (`MobAi`: navigation, controls, selectors,
sensing, head and body yaw; `server_ai_step`), `path/` (one file per class), `control.rs`,
`target.rs`, `goal/` (selector + one file per goal family), `behavior/` (the villager's). Hooks
only elsewhere: `mob/mod.rs` (the module, an `ai` field set in `add`), `living.rs`
(`server_ai_step` -> `ai::server_ai_step` for goal mobs, `tickHeadTurn`), `world.rs` (the
`sendBlockUpdated` hook: the PathTypeCache invalidation and the recompute request, a no-op
without navigating mobs). Tools: `crates/mobai` (`mctas mobai`: the probe, the fuzzer, the
benchmarks).

## Changelog (revision 2, after the G1 review by e6b8ba09)

| point | what changed |
|---|---|
| 1 | Five layouts (A obvious transliteration, B arena, C approximate walker, D bridge, E flow fields) with effort, speed, % of ceiling, exactness, constraints; trace replay noted |
| 2 | Ceiling: exactness 100%; speed ceiling measured (the game 13.3 / 18.5 / 36.8 us, redsim without AI 1.90 / 3.32 / 2.29 us); A is the bar, measured first |
| 3 | The trade-offs section (exactness/speed, breadth/depth, arena/verbatim maps and stale state, the PathTypeCache, fixture size) |
| 4 | Must-haves: AI off by default with byte-identical runs (the tests named), deterministic, mirrored streams only, refuse never guess |
| 5 | Not goals: wall-clock parity, the deferred scope; what the skeleton scope proves without arrows |
| 6 | Use in one sentence; the Number = exactness and speed, used in every layout and the decision |
| 7 | Decision: B built as A first, argued on the number and the must-haves; the phases |
| 8 | Fixtures 7-12 (skeleton, spider, village day, panic, trader and llamas, pillager patrol): every goal family in scope has one; the parent's additions (wandering trader and llamas, pillager patrols with crossbows) and the arrows they and skeletons need, in phase 5 |
| 9 | The cross-lane contracts written out with their branch; docs/coverage.md path; the fallback if the harness slips |
