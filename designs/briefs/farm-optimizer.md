# The farm optimizer (design doc, gate G1)

farmopt track (ca87cf44), 2026-11-08. Gate G1 ([docs/process.md](../../docs/process.md)).
Revisions 2 and 3 (after the G1 review and re-review by b99cbe5d: the changelogs are at the end).

Asked by vjeux on 2026-11-08: "We want to find the absolute best farm given each stage of the
game, it seems that right now you come up with barely optimized concepts for each one" (10:50);
"I want some kind of pareto front where each are strictly better than other possibilities in that
space. I also don't want hundreds of tiny variations that don't meaningfully affect the farm"
(10:56); "For middle pick, the clock is huuge. We built an entire clock system to find the
smallest one for each tick. Why are we not using it. Also middle one should have more than one
module" (10:48); "The challenge of the iron farm is how do you stack multiple modules next to each
other and increase efficiency while keeping it small" (04:26); "We should be tick exact everywhere"
(08:56). The iron farm ([farm-iron.md](farm-iron.md), revision 8, approved) is the first farm;
the core is farm-agnostic so the next farms (XP, crops, mobs, raids, trading halls) plug into it.
Every mechanic below was read in the decompiled 26.3 server (`mctas src show <Class>`); no outside
design is used.

## Use

A player picking an iron farm for their stage of the game gets the few builds no other build of a
documented space beats, each measured on redsim and checked in the real game.

## The number (binding for the first search)

The stage-spec session (3883f332) owns each stage's objectives; its first specs are binding for
the first search: `designs/farms/iron/{day-1,middle,late}.spec` on explore/ironspec 687f27ff5,
argued in farm-iron.md revision 9 (under G1 there in parallel). They refine revision 8's stages
with vjeux's 10:48 changes (the middle has 2 to 4 modules, so revision 8's one-hour cap becomes a
120-minute constraint and a front axis). The first objective is the rank; the others are the axes
the front is drawn against (a front, not one winner).

| stage | objectives (first = rank) | constraints (the stage's definition) | fork |
|---|---|---|---|
| day 1 | **min t_kit_minutes**: the minutes until a fresh player owns a 24-iron kit, at the cheapest of the 12 nearest valid spots of village-edge: 2 s a block placed (covers included) + 1 s a block dug + 120 s a lava trip + 180 s a villager moved + (build iron + 24) / net iron per hour; **max iron_per_hour** (net: the judge's rate in terrain, weather and natural spawning on Hard, less the iron destroyed per hour by lightning and fire) | day-1 items (no quartz, no slime); villagers <= 8 (the sample's village); modules 1-2; dug in at village-edge | redstone: none (the bed plate) or dust (an iron pickaxe, 3 iron; the smallest day-1 catalog timing part) |
| middle | **max iron_per_hour** (net: the expected iron per hour over one game day from the running farm; a break stops it for the rest of the day); **min build_minutes** (day 1's formula) | Nether tier; modules 2-4; shared_parts >= 1 (copies are the bar); build_minutes <= 120; the void | |
| late | at N = 8: **max iron_per_hour** (net over 8 h AFK; iron the storage cannot hold is lost); **min build_minutes**; **min volume_per_module** | any item; modules 8; per_block_gain >= 1.05 and volume_gain >= 1.05 (N = 8 over N = 1: stacking must pay); afk_spots <= 1; the void | |

**Net rates, per stage, as the pinned specs define them** (each hazard from the judge's lines):

- **Day 1**: net = the rate less the iron destroyed per hour (lightning on drops, the judge's
  measure; lava's fire on drops). Breaks (a villager turned, the zombie lost, a part burnt) are
  only reported with their expected days: the player is there to fix them.
- **Middle** (W = one game day, 24000 gt = 1/3 h) and **late** (W = 8 h): a break of hazard l per
  hour stops the farm for the rest of W, so the expected iron per hour is rate x (1 - e^(-lW)) /
  (lW) with l the sum of the break hazards (lightning per storm-hour times the storm share, lava's
  fire); the late stage's is also capped by its storage hours.
- An axis whose hazards are not all numbers yet is reported as **gross** (its name says so on the
  front), never as net; it becomes net when the missing hazard is computed (no caveat lines).

The late constraints' ratios: per_block_gain = (iron/h per block at N = 8) / (iron/h per block at N =
1); volume_gain = (volume per module at N = 1) / (volume per module at N = 8); both at least 1.05
(stacking must raise the first and shrink the volume). Merge: 10% on every objective (ironspec's
argument: under a player's notice, inside the spread between spots and rotations, well above the
judge's ~1% interval).

## The loop (vjeux 10:56)

The stage-spec session (3883f332) owns the objectives: it sends a spec (constraints, 2-4
objectives, forks, the space's boundaries, the merge threshold); the engine answers with a merged
front per fork, the bounds and the gap; the spec session judges and changes the spec. The table
above is the first spec of each stage. Every evaluation is kept (the journal, below), so a new
objective re-ranks without evaluating anything again.

    farm: iron                            # the spec file: designs/farms/<farm>/<stage>.spec
    stage: middle                         # (the binding middle spec, explore/ironspec 687f27ff5)
    objective: max iron_per_hour
    objective: min build_minutes
    constraint: materials <= nether
    constraint: modules in 2..4
    constraint: shared_parts >= 1
    constraint: build_minutes <= 120
    space: placement void                 # narrows a grammar parameter, never widens it
    merge: 10%                            # within 10% on every objective: one farm (below)

(`fork: <parameter>` lines split the answer into one front per value, several forks their
product: the day-1 spec forks on `redstone`.)

The answer (`designs/farms/<farm>/<stage>/`, regenerated, never edited by hand): `front.md` (per
fork: each merged member's objectives with their 95% intervals, its bound, its runner-ups, its
design file), `ranked.tsv` (every candidate: key, fork, the level it reached, its metrics, its
bound, why it was dropped), `summary.md` (counts per level, the front, the bound front, the gap),
and one preview-ready design file per member.

## Mechanics, from the game code

The farm's own rules are farm-iron.md revision 8's "Mechanics, from the game code" (who spawns a
golem and when, sleep, where a golem spawns, the golem, the kill and the drops, the zombie, the
calm devices, weather and lightning); this doc relies on them as written there. The optimizer adds
these, each read in 26.3:

- **Clusters (the 10-box).** `Villager.spawnGolemIfNeeded` lists
  `level.getEntitiesOfClass(Villager.class, getBoundingBox().inflate(10.0, 10.0, 10.0))`, and after
  a spawn calls `GolemSensor.golemDetected` (599 gt) on every villager of that list. A villager's
  box is 0.6 x 1.95 x 0.6, so two villagers are in each other's list when their centres are less
  than 10 + 0.3 + 0.3 = **10.6** apart on x and on z and their feet less than 10 + 1.95 = **11.95**
  apart on y ("10-box" means this test). Clusters are the connected groups of that relation; each
  cluster spawns at most one golem per 600 gt.
- **The follow cube.** `NearestLivingEntitySensor` (rate 20) stores every living entity meeting
  `getBoundingBox().inflate(followRange, followRange, followRange)`; `Mob.finalizeSpawn` gives the
  villager `FOLLOW_RANGE` 16 x (1 + b), b = `random.triangle(0.0, 0.11485)` (in (-0.11485,
  0.11485)), so R is in (14.16, 17.84). With the golem's 1.4-wide box a golem is listed while
  |dx| and |dz| < R + 1.0 (0.3 + 0.7) and its feet lie in (villager feet - R - 2.7, villager feet +
  1.95 + R). `GolemSensor` (rate 200, `GOLEM_SCAN_RATE`) sets `GOLEM_DETECTED_RECENTLY` (599 gt)
  when that stored list holds an iron golem.
- **No coupling.** A golem spawns at most 8 columns from its villager's block (`trySpawnMob(...,
  8, 6, ...)`), so modules whose villagers are at least 8 + 17.84 + 1.0 = **26.84** apart on x or z
  never list each other's golems at spawn (the bar's pitch, farm-iron.md's 26.9).
- **Sensor phases.** `Sensor.randomlyDelayStart` sets `timeToTick = random.nextInt(scanRate)` from
  the villager's own random when its brain is built; `Sensor.tick` decrements it and fires at 0,
  then every `scanRate` ticks exactly. So each villager's nearest-living sensor fires on a fixed
  residue mod 20 and its golem sensor on a fixed residue mod 200, uniform over the villagers.
- **What a sighting costs, exactly.** A golem inside villager V's cube during D consecutive ticks
  is in V's stored list from the first rate-20 scan in that stay to the first scan after it (a
  window of length 20 floor(D/20) or 20 ceil(D/20)); V's golem sensor resets V when one of its
  rate-200 scans falls in the window. Over V's uniform phases that happens with probability D / 200
  (D <= 180: farm-iron.md's model); for one villager's fixed phases it is 0 or 1 per golem. For
  every phase it is 1 once the window reaches 200 gt, which D >= 220 guarantees.
- **Free fall.** `LivingEntity.travelInAir`: each tick the golem moves by its velocity, then
  v <- (v - 0.08) x 0.98 (iron golems are immune to fall damage). The engine steps this recurrence
  to get the least time a golem needs from a spawn cell to the floor of a villager's smallest cube.
- **Marks.** `SpawnUtil.trySpawnMob` makes `spawnAttempts` = 10 attempts; each draws dx, dz =
  `Mth.randomBetweenInclusive(random, -8, 8)` from the level random, scans the column from y + 6
  down (`moveToPossibleSpawnPosition`, `LEGACY_IRON_GOLEM`), then creates the golem and keeps it
  only if `checkSpawnRules` and `checkSpawnObstruction` pass. The block conditions depend only on
  the blocks; `checkSpawnObstruction`'s `isUnobstructed` adds one more condition (no entity in the
  golem's box), so an entity can only make a statically valid column fail, never a column valid.
  Each willing panicking villager runs its own 10 at a multiple of 100 (farm-iron.md).
- **The hide's length, tick exact.** `ServerLevel.tick` runs the scheduled block ticks
  (`blockTicks`, `fluidTicks`: observers, repeaters, comparators), then block events, then the
  entities. A trapdoor follows its power in `TrapDoorBlock.neighborChanged`, during the block
  phase. So a hide is shut for whole ticks, as every entity of the tick sees it. The hostile sensor
  (`VillagerHostilesSensor`, rate 20) fires once in any 20 consecutive ticks; each of its scans
  casts the zombie's ray at that tick (the visible-entity cache is filled lazily per 20 gt list, and
  in PANIC and REST no other reader asks for a zombie first: farm-iron.md). Hence: **a hide shut
  for 20 consecutive ticks during REST gives every scared villager exactly one scan without the
  zombie** (19 misses the villager whose phase falls on the uncovered tick); that scan erases
  `NEAREST_HOSTILE`, `VillagerCalmDown` switches it to REST and it starts sleeping in the same brain
  tick. **L_min = 20 gt**, which the judge's sleep check (a villager 30000 gt without sleep fails a
  run) then confirms on every member. (The ironfarm lane's first measurement on M2: an 18 gt hide
  fails and 24 gt works; it also reports a failure above 24 gt, whose cause is being traced with
  it. Until that cause is in the code reading, the grammar keeps every part whose output is at
  least 20 gt and the judge's runs decide; a part that fails the sleep check fails its build.)
- **The spot.** Day 1 is built at the cheapest of the 12 nearest valid spots of the village-edge
  sample (farm-iron.md revision 8 and the brief of this task); the day-1 lane's pricing
  (`redsim::entity::mob::site`: `spots_for`, `price_spots`, `covers_at`, `day1_minutes`, on
  explore/day1-d1u ef79f04bd) is that rule as code, called by the engine.

## Trade-offs (the axes the search moves along)

- **Villagers**: each is 180 s of build (day 1) and blocks (a pod); a 4th or 5th absorbs a
  sighting (3 of up to 5 must agree) and adds 10 attempts a mark when it is scared.
- **Scared villagers**: only panicking villagers attempt; each scared one needs the calm.
- **The pad**: more pad cells raise the mark's success (f_v) and cost blocks; a lower pad (golem
  feet down to beds - 6) shortens the fall out of the cubes.
- **The kill**: in sight (a pit corner: few blocks, about 250 gt in every cube: every villager
  reset) or out of sight (a drop past the cube's floor: blocks or dug cells, few resets).
- **The calm trigger**: the bed plate (day 1, no quartz; the hide shut all night, the sleeping eye
  waking the scared villagers), a bed observer and a timing part (quartz; a 20 gt hide), a daylight
  detector (3 quartz; it stalls in rain: a measured rain-day rate).
- **Sharing** (N modules): one calm trigger, one zombie, one kill, one collection save blocks;
  closer modules cost sightings through the follow cube; clusters closer than 10.6 merge.
- **Placement**: in the void every wall is placed; dug in, natural rock is the wall and each dug
  cell costs 1 s against 2 s placed (day 1 only).

## The layouts (the seeds), measured

Every layout below is a point of the grammar (next section) in one of two ways. **Exact seeds**:
the grammar's `seed` parameter rebuilds each layout as its own generator writes it (`mobfarm gen
...`), hand-built parts included, for calibration only: the search reproduces each one's measured
numbers before it claims anything new, and seed points never enter a front. **Searched points**:
the same geometry with every hand-built timing part replaced by catalog parts (M2-open's 48 gt
loop of about 45 blocks becomes a part of 15 or 16 blocks with its wiring, so M2-open-catalog has
about 375 blocks); their numbers are measured, never carried over from the seed. Numbers from
farm-iron.md and the ironfarm and day-1 lanes' measurements (redsim's whole judge, 64 runs,
unless marked).

| # | layout | blocks | iron/h (95%) | share of ceiling | its stage number | constraints |
|---|---|---|---|---|---|---|
| L1 | **the obvious design**: 3 villagers in pods, an inverted daylight detector hides the zombie all night, a 6 x 5 pad, a trench to a lava blade in sight | 386 | 139 (138-141) | 29% | middle 0.36 iron/h per block | 3 quartz; stops in rain |
| L3 | the archived day-1 farm: 8 spawn pits, lava beside each golem's head, a tripwire hides the zombie while one burns | 189 | 295 (292-298) | 62% | day 1 T about 54 min (34 of them its 16 lava trips); in terrain 1,272 s of blocks (185 placed, 266 covers, 370 dug) before the lava trips | 16 lava sources; zombie hidden by hand the first night |
| D1 | the bed plate (U1's plate holds the hide open by day; the scared villager's sleeping eye sees under the shut slab) over a 4 x 4 pit, one lava source | 318 (void) | 243 (241-245) | 51% | day 1 T about 30 min in the void; as a void design cleared into village-edge 2,708 s of blocks | 1 scared villager |
| D1U | D4 (two scared villagers through one hide) dug into the ground, villagers 8 below it, natural rock as walls (`mobfarm gen d1u`) | 396-427 s of blocks at village-edge | not yet judged in terrain | | T about 25 min by the day-1 lane's pricing with D4's estimated rate: the first search measures it | dug in; dark cells (hostile exposure on its card) |
| M2-open | 3 scared + 1 blind villagers, a bed observer and a hand-built 48 gt hide (bulb, comparator, 12 repeaters), the 6 x 6 dry hole, an open 16-block drop, a fire-safe lava blade | 404 | 439 (435-443) | 92% | **middle 1.09 iron/h per block at N = 1** (33 min build) | 2 quartz, 4 lava sources |
| M4 | two M2 modules stacked 43 apart (no sharing) | 1,276 | 887 (880-893) | 92% | 0.70 at N = 2 (83 min) | |
| LB | **the late bar**: M2 copies 43 apart vertically, nothing shared, no coupling | 638 per module | 443 per module | 93% | 0.69-0.70 at N = 1, 2, 4, 8 (flat); volume per module flat | |
| LS-open | M2-open modules stacked, every chamber drained into one chute, one sorter and iron-block crafter | 404 + 65 / N per module | 439 at N = 1 | 92% | 0.94 at N = 1, rising with N | |
| LT | two M2 fronts back to back sharing one pad, drop and chamber | 409 per module | 343 per module | 72% | 0.84 at N = 2 | the shared pad lies in both clusters' cubes |
| TZ | two clusters around one zombie, one hide, one bed observer; pads on the far sides (no foreign dwell) | 617 per module | 389 per module | 82% | 0.63 at N = 2 | long strip-pad water paths |

## Decision

Search the whole grammar of each stage from its seeds, not a narrowed copy of one seed. The
layouts above differ along the same few axes (villagers and scared villagers, the calm trigger,
the pad, the drop and kill, sharing and pitch, placement), and every one was hand-tuned along at
most one of them; their measured spread (0.63 to 1.09 iron/h per block, 25 to 54 minutes) is
what a joint search over those axes can improve. The grammar spans each seed (exactly, as a
calibration point, and with catalog parts, as a searched point), so the search starts from the
best seed's geometry and every claim is relative to the same space.

## Must-haves (real use), and where each is enforced

farm-iron.md revision 8's one rule ("it works as its card says, verified in the real game") and
its must-haves hold for every front member. A member cannot win by missing one: each is either a
grammar constraint (no candidate breaks it) or a check of the judge (a candidate that fails it is
dropped at that level).

| must-have / tradeoff (farm-iron.md rev 8) | grammar constraint | judge check or measured metric |
|---|---|---|
| villagers in closed 1-cell pods on their beds, Manhattan 1 from HOME | every pod part is a closed 1-cell pod | `refuse::static_checks` (the villager flood) |
| villagers out of the zombie's and the golems' reach, no damage | the zombie's cell at least 2 from every pod, glass between | refusal (attack box with the vehicle); runs (hurt check) |
| no roof, wall or part becomes a spawn base | roofs glass-capped or leaves; part blocks glass-capped where a search reaches | static stage: every spawn cell any scared villager's search finds must be a pad cell, else refused |
| the zombie rides a boat, its eyes never see the sky, never in water, no drop within its reach | boat on a slab under a roof, no water at its cell | runs (pickup reach); the card's sunburn line |
| no golem escapes, reaches the zombie or a vehicle; each dies in a kill cell within 1200 gt | | the golem exit flood (every standable position, 9 inputs), runs |
| every iron ingot reaches `port output` | the chest at `port output` | runs (iron lost at the end) |
| survival materials; static lava; no water touching lava | stage material limits; lava only in kill parts | refusal; runs |
| every villager sleeps each game day | the calm trigger meets L_min (or the bed plate) | runs (30000 gt without sleep fails) |
| robust over rotations and positions | | the judge's 8 builds |
| tradeoffs on every card: build effort, rate with its interval, the first night, a missed night, days away, weather, lightning, lava's fire, hostiles, storage and AFK hours, lag, size | | the judge's metrics (restart probe, absence probe, weather windows, lightning measure, fire line, storage, hoppers per 1000 iron/h, chunk columns); in terrain the exposures; the expected-loss terms of the stage numbers come from these lines |
| the real game | | the gate (`circuit verify`: `mobfarm_game::confirm`) on the members the spec session sends on |

## Not goals

- No new judge rule, metric or kind: the engine calls the farm's judge as it is; a rule the search
  finds necessary goes back through farm-iron.md (G1).
- No single winner per stage: the front is the answer; the spec session picks what reaches vjeux.
- The merged front is an epsilon-Pareto set (below), not the exact non-dominated set: members
  folded under another may beat it by less than the threshold on some axis; they stay listed.
- No design from outside the code and the search.

## The bar and the ceiling of each stage

| stage | the bar (measured) | the ceiling |
|---|---|---|
| day 1 | **D1** (measured: 243 iron/h (241-245), 51%, about 30 min in the void) until D1U is judged in terrain (its T about 25 min is priced, its rate not yet measured); then the better of the two | T >= the least build time of the space + (its least build iron + 24) / 480 iron/h; per candidate its own build time + (iron + 24) / its rate bound; iron/h <= 480 per cluster |
| middle | copies at a no-coupling pitch (shared_parts 0): two M2-open, about 878 iron/h, 66 min; four, about 1,756 iron/h, 132 min (over the 120-min cap) | 480 iron/h per cluster (one golem per 600 gt, 4 iron each); per candidate its rate bound |
| late | M2-open copies: 1.09 iron/h per block, flat in N; LB 0.69-0.70; LS-open 0.94 at N = 1 | 8 x 480 = 3,840 iron/h at N = 8; per candidate its rate bound |

Every result is reported as a share of its candidate's bound and of the stage ceiling.

## The engine

### The core (crates/farmopt, farm-agnostic)

- **Space**: a candidate is a typed parameter vector with a canonical key; the farm enumerates the
  valid candidates under a spec, builds one into a design file, and lists its geometric neighbours.
- **Evaluate**, cheapest first, each level adding metrics to the candidate's vector: static,
  quick, full (below), then the gate on the members the spec session picks.
- **Bound**: the proven optimistic value of every objective metric, from the static metrics.
- **Search**: multi-objective branch and bound, then local search; parallel, deterministic,
  resumable.

### The iron grammar, version 1 (what is in it)

| part | v1 choices | stage limits |
|---|---|---|
| villagers per cluster | 3, 4, 5 in a row of 1-cell pods 2 apart (shared glass walls), beds facing away from the zombie | |
| scared / blind | the scared ones see the zombie through the hide; the blind ones see glass; at least 1 scared; the calm's U1 is blind | |
| zombie and hide | a boat on a slab under a roof, within 8 (`distanceToSqr` feet to feet <= 64) of every scared villager; the hide trapdoor in the one cell every scared line of sight crosses, its half and facing solved from the rays (shut: every scared standing eye blocked; the bed plate also needs the sleeping eye clear when shut) | |
| calm trigger | **bed plate** (U1's wooden plate, farm-iron.md D1); **bed observer + the smallest catalog part** whose output for the observer's 2 gt pulse lasts at least L_min = 20 gt; **daylight detector** (an inverted detector's dusk pulse, layouts.rs `dusk_pulse`) | day 1: the bed plate; with the `dust` fork, the plate's falling edge into the smallest day-1 catalog part (no quartz, no slime) once the circuit study archives one (none today: until then `dust` = `none`) |
| spawn platform | the dry hole (L2d) in an n x n pad, n = 4, 6, 8, 10, its floor at beds - 7, - 6 or - 5 (the pit's pad for the in-sight kill) | |
| drop and kill | the pit corner (feet at - 13: in sight, one lava source, D1) or the fire-safe blade chamber (`chamber_safe`, feet at - 16, - 19, - 22 or - 25; M3's drop-and-blade is day-1 legal with the bed plate) | |
| collection | the kill's hoppers to the chest, or the shared chute with the archived sorter, iron-block crafter and poppy composter (LS: 8 h of storage for the late stage) | |
| tiling | N = 1, 2, 3, 4, 8; a row on x, a row on z, a stack on y, or a 2-wide grid; the pitch per axis from 11 (clusters apart: > 10.6) to 27 (no coupling), and 43 vertical (the stack bar); shared: the calm trigger (every module's hide driven together: all clusters rest at the same day time, `VILLAGER_SCHEDULE` REST at 12000), the zombie between two clusters (within 8 of every scared villager of both), the drop and kill, the collection | day 1: 1-2 (two clusters sharing a kill and collection, 8 villagers at most); middle 2-4; late 8 |
| placement | the void, or dug in at a village-edge spot (`terrain: cells`, natural rock as walls) | day 1: dug in |

**The calm's catalog part, worked out.** Kinds `pulse-extender`, `monostable`, `off-delay` of
`designs/circuits/catalog.tsv` (docs/circuits.md: a pulse extender gives L for every input from its
min-pulse up; a monostable gives L; an off-delay gives the input's length + L). The observer's
pulse is 2 gt; a part whose min-pulse is 4 gets a repeater at delay 4 in front (a repeater
lengthens a shorter pulse to its delay: catalog `pulse-extender-repeater-L4`, 2 blocks with its
support). Output >= 20 gt, smallest first (blocks with the repeater and its support):

| part | blocks | output for the observer's pulse | note |
|---|---|---|---|
| `pulse-extender-sculk-L30` | 2 | 30 gt | vibration-sensitive (any vibration within 8 fires it: golems, villagers): out of v1 until redsim models entity vibrations |
| `off-delay-ring-L24` + repeater | 13 + 2 = 15 | 4 + 24 = 28 gt | |
| `pulse-extender-hopper-oneshot-L24` + repeater | 13 + 2 = 15 | 24 gt | a hopper: 5 iron |
| `off-delay-ring-L16` + repeater | 14 + 2 = 16 | 4 + 16 = 20 gt | exactly L_min |
| `mono-retrig-ring-L20` | 22 | 20 gt | min-pulse 2, no repeater |
| M2's hand-built loop | about 45 | 48 gt | replaced |

The grammar keeps every part with output >= 20 gt (so the search, not this table, picks by the
whole build: wiring and placement change its real cost), and the front reports which. **Every
catalog lookup respects the stage's materials**: "the smallest archived part that meets the need"
is the smallest made only of items the stage allows (day 1: no quartz or slime, so no comparator,
observer, daylight detector or sticky piston; repeaters, torches and dust only in the
`redstone=dust` fork, which counts the iron pickaxe). The table above is the Nether stage's (every
part has a comparator or an observer, the hopper one a hopper). When nothing fits, the engine
says so on the front (day 1 has no such part today: the stage-spec session asked the coordinator to
route a torch + dust + repeater falling-edge part of 20 gt or more to the circuit study) and the
calm falls back to the bed plate.

**The space's size** (exact counts are printed by every run): day 1, about 1,500 builds (villagers
3 to 5 per cluster within 8 in all, scared 1 to 3, pad 3, floor 3, kill 5 (the pit or the blade at
4 depths), dug depth 3, modules 1 or 2; the spot chosen by pricing per build); middle, about 650
modules (villagers 3, pad 4, floor 3, drop 3, calm about 6 parts) times about 400 tilings (N 3,
arrangement 3, pitch about 6, sharing 8): about 2.6e5; late about 1e5.

### Branch and bound

The grammar is a tree of choices in a fixed order: calm, villagers, scared, pad, floor, drop and
kill, N, arrangement, pitch, sharing, placement. A partial assignment's bound is the best any
completion can reach: the rate bound uses the best mark factor over the remaining pad choices (a
table per pad, floor and villager row) and the ceiling; the block count uses the blocks of the
parts already chosen plus each remaining part's least block count (a per-part table). A subtree
whose optimistic vector is weakly dominated by the pessimistic vector of a candidate already
measured is cut without being built. Leaves are visited best bound first (ties by key). When a
space is small enough to build every leaf (under about 1e6 at about 10 ms a static build on the
box's cores), the leaves' own exact static bounds replace the partial ones.

**Pruning and promotion.** A candidate leaves the search when its optimistic vector is weakly
dominated by a measured candidate's pessimistic vector (the end of its 95% interval worse for each
objective; exact values are their own ends): it can then never be strictly better than that
candidate. Otherwise it moves to the next level. **When the bound is loose** (it prunes little:
most survivors' bounds stay above the front), the quick level does the pruning with its intervals;
each stage has a budget (quick runs, full judges); whatever is undecided when the budget ends is
listed with its bound, and the claim and the gap then cover only what was decided.

### The bound (what is proven, and what it assumes)

The bound bounds each candidate's **long-run mean rate** (the expected iron per hour of the farm
running forever from its steady state), per cluster, almost surely for every realization of the
sensor phases and the spawn draws. The judge's 64 runs estimate that same long-run mean with a 95%
interval (finite runs scatter around it), so a pruning claim reads: with 95% confidence the
measured candidate's long-run rate is at least its interval's lower end, which is at least the
pruned candidate's long-run bound.

Cycles: the time from one golem of a cluster to its next. With T the spawn tick (a multiple of
100), the cycle is 600 + 100 (K - 1) + E, where K >= 1 counts the marks (T + 600, T + 700, ...)
until one succeeds, and E >= 0 the marks lost to resets.

1. **The ceiling**: K >= 1 and E >= 0, so a cycle lasts at least 600 gt (farm-iron.md: the 599 gt
   memory clears at the start of the brain tick 600 gt later, marks fall on multiples of 100); iron
   per golem `uniform(3, 5)`, mean 4: 480 iron/h.
2. **Marks that miss**: each attempt's column is uniform over the 17 x 17 and independent of every
   earlier draw (the level random's `nextInt(17)`, as redsim mirrors it); an attempt can succeed
   only when its column holds a statically valid cell (entities only remove cells, above). So,
   given everything before it, a mark succeeds with probability at most p = 1 - prod_V (1 -
   f_v)^10 over the cluster's scared villagers V (all counted as panicking and willing: an upper
   bound; f_v = the valid columns of v's search / 289). K is then stochastically at least a
   geometric variable of parameter p, so E[cycle | the past] >= 600 + 100 (1 - p) / p for every
   cycle. The cycles' sum minus the sum of these conditional means is a martingale with bounded
   increments' variance (K has a geometric tail), so by its strong law the long-run mean cycle is at
   least 600 + 100 (1 - p) / p almost surely: per realization in the long run, not per cycle (a
   single cycle can have K = 1), and not for a finite run (the judge's interval covers that).
3. **Sure sightings, from the tick order**: a reset of villager W at tick s writes
   `GOLEM_DETECTED_RECENTLY` with 599 gt in W's sensor phase; `MemorySlot.tick` (in
   `forgetOutdatedMemories`, the first step of `Brain.tick`) decrements it each later tick and
   clears it once it reads 0, so W holds the memory through tick s + 599 and is unwilling at every
   mark before s + 600. A reset at s >= T + 1 therefore keeps W out of the mark at T + 600 (and a
   reset at s = T, in the spawn's own tick, does not: it clears by T + 600 like the spawn's own
   `golemDetected`). Now take a golem that stays at least 220 consecutive ticks from its spawn in
   W's smallest cube (R = 14.16): W's stored list holds it from W's first rate-20 scan after the
   spawn (by T + 20) to the end of the stay, and W's golem sensor fires once in [T + 20, T + 219]
   (one scan in any 200 consecutive ticks, `Sensor.tick`), so W is reset at some s in that range,
   s >= T + 1, for every phase. If that holds for at least n - 2 of the cluster's n villagers, fewer
   than 3 are willing at T + 600, so E >= 100 and the cycle lasts at least 700 gt (an in-sight lava
   kill: the golem's death takes at least 240 gt, 25 hits 10 gt apart, farm-iron.md, all inside the
   cubes). Shorter stays prove no loss for a single realization (a villager's fixed phases can miss
   every golem), so the bound counts none there: farm-iron.md's D / 200 is a mean over phases.
4. **Coupling**: clusters merged by the 10-box are one cluster (one ceiling); foreign resets count
   only where item 3's proof holds for a foreign smallest cube.

So a cluster's long-run rate is at most 480 x 600 / (600 + 100 (1 - p) / p + 100 [item 3 holds])
iron/h, and a build's at most the sum over its clusters. Left out (counted as no loss): the night
(hides, the bed plate's waking scared villager), the panic gap, the walking policy, phase-dependent
sightings. The gap next to each front is what they cost; it is reported, not hidden.

### The evaluation ladder

- **Static** (milliseconds): the design parses in redsim; `refuse::static_checks`; the built world
  (water settled, `SETTLE_TICKS`); the scared villagers (rays as built, trapdoors open, trapdoors
  shut); their spawn cells (`spawn::golem_spawn_cells_from`), f_v, every cell on the pad (else
  refused); the clusters; the follow-cube margins; blocks placed and dug, the bill, lava sources,
  villagers, materials, modules, the box volume; in terrain the spot price.
- **Quick**: K = 4 runs of 1 game day (`farm::run`, steady state, golems walking at random, seeds
  `judge_seed(0, 0..3)`), the rates' mean with Student's t interval (3 degrees of freedom); any
  must-have failure in a run drops the candidate.
- **Full**: `circuit::mobfarm::judge` with its defaults, as the card is made: the refusals, the
  golem exit flood, the 9 worst-policy runs (5 days, one seed), the restart probe (`restart: yes`),
  the absence probe (4 runs), the weather windows (16 runs), the lightning measure, then 8 builds (4
  rotations x 2 positions) x 8 seeds x 5 game days = 64 runs; pass = every build passes; the rate is
  the mean of the 64 runs' iron/h, its interval 1.96 sd / sqrt(64).
- **Gate**: `circuit verify` (`mobfarm_game::confirm`): each build in the 26.3 server with the
  weather on and a forced day of rain and of thunder; it passes when every must-have holds in the
  game and each window's game rate falls within the joint interval of redsim's (its `passed`).

### Merging (vjeux 10:56)

Members of the measured front are walked from the best on the first objective (ties: the next
objectives, then the key). A member is kept unless a kept member covers it: at least as good on
every objective within the spec's relative threshold (for each oriented objective x: x_kept >=
x - eps |x|, so a min objective's minutes count the same way as a max objective's rate). A
merged member is listed under the first kept member covering it. No grid is involved, so nothing
depends on box alignment; the result is an epsilon-Pareto set.

### The journal, invalidation, determinism

Every evaluation is one appended line (key, level, metrics, why), in a journal per farm, stage and
space (the spec's `space:` and farm lines, hashed into its path) and tagged with the evaluator's
version string: a change to the grammar or the evaluator bumps the version and the old lines are
not reused. An evaluation is a pure function of (candidate, space, version): fixed seeds, no
clock; threads only change the order lines are written. The search decides in batches taken in a
fixed order (bound, then key), so its result does not depend on thread timing.

### Exactness, restated

The evaluator is exact (redsim's model through the farm's own judge, and the game gate); the
search's bounds are proofs under the assumptions stated with them; its estimates are measured
intervals; merging is presentation. A part that needs an unmodeled mechanic stays out of the
grammar: in v1 the kills are lava only (the pit corner, the blade chamber: `LavaFluid.entityInside`
as redsim models it). Magma and suffocation (modeled, 1 damage per 10 gt: about 1,000 gt a golem)
and campfires (not modeled) wait for a version that needs them; a vibration-sensitive trigger waits
for entity vibrations (mob AI 8a0201dc).

## Milestones

1. This doc re-approved; the core with its unit tests; grammar v1 rebuilding every exact seed of
   the layout table to its measured numbers (calibration), and the count of distinct searched
   builds it spans.
2. Bounds per stage; the day-1 front at village-edge (the day-1 lane's pricing), the middle front
   (N 2-4, the catalog calm), the late front (N = 8, the curve at 1, 2, 4, 8).
3. Each front's members previewed (`mctas site preview`), the gap per objective;
   `docs/farms-optimizer.md` (how a farm plugs in: its space, its evaluator, its bound) and an
   optimizer section per stage in farm-iron.md.

## Changelog (revision 2, after the G1 review by b99cbe5d)

| point | what changed | where |
|---|---|---|
| 1 | Use in one sentence; the stage-spec session's vectors pinned (explore/ironspec 687f27ff5, farm-iron.md revision 9), reconciled with rev 8 and vjeux's 10:48 (the middle's 1 h cap becomes a 120-min constraint and a front axis since N >= 2); net rates defined | Use; The number |
| 2 | The layouts table with measured numbers (L1, L3, D1, D1U, M2-open, M4, LB, LS-open, LT, TZ); D1U and the bar defined; the decision argued | The layouts; Decision |
| 3 | Must-haves mapped to grammar constraints or judge checks | Must-haves |
| 4 | Class and method for every optimizer rule: the 10-box (10.6 / 11.95), the follow cube, 26.84, sensor phases, the sighting window, free fall, marks, the hide's tick order, the spot rule | Mechanics |
| 5 | The bar and the ceiling per stage | The bar and the ceiling |
| 6 | The bound re-derived: the sighting term only where proven for every phase (stays >= 220 gt), D / 200 kept as the mean it is; the marks' assumptions stated; what is left out and the loose-bound fallback | The bound; Branch and bound |
| 7 | L_min = 20 proven from the tick order; the catalog parts listed with blocks and outputs | Mechanics; the calm's catalog part |
| 8 | Space sizes, the branching order, the partial bound, the budget fallback | The space's size; Branch and bound |
| 9 | Quick K = 4 x 1 day; the full judge's composition and interval; the gate's pass rule; the merge's exact rule; exactness restated; v1 kills; shared parts' constraints; the journal's invalidation and determinism | The evaluation ladder; Merging; The journal; Exactness |

## Changelog (revision 3, after the G1 re-review by b99cbe5d)

| point | what changed | where |
|---|---|---|
| 1 | The loop's example is the binding middle spec (max iron_per_hour, min build_minutes, merge 10%) | The loop |
| 2 | The day-1 bar is D1 (measured) until D1U is judged in terrain; D1U's row says its rate is not measured yet | The layouts; The bar |
| 3 | Seeds are exact calibration points (the `seed` parameter, hand-built parts included, never on a front) and searched points with catalog parts (M2-open-catalog about 375 blocks, measured anew) | The layouts; Decision; Milestones |
| 4 | The bound bounds the long-run mean rate: the marks term by a martingale strong law per realization (not per cycle, not per finite run); the sure-sighting +100 proven from `MemorySlot.tick`'s order (a reset at s >= T + 1 keeps the villager out of the mark at T + 600); the pruning claim's 95% reading | The bound |
| 5 | Net rates per stage as the specs define them (day 1 subtractive, middle and late the break formula); an axis with a missing hazard is named gross; the late ratios' directions | The number |
| — | Catalog lookups filtered by the stage's materials (the parent's rule): day 1 has no legal timing part today | The calm's catalog part |
| — | The ironfarm lane's hide measurement (18 gt fails, 24 works, a failure above 24 under study) | Mechanics (the hide) |
