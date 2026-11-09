# Iron farm: the villager carousel (brief, G1)

vjeux 2026-11-08 22:53: "But the farm is soooo big and spread out. I'd love to pack tons of villagers
and move them around or something so we can have golems spawning constantly". This brief is the
family's G1 doc (docs/process.md step 1); the stages, prices, travel, parts and judge rules of
[farm-iron.md](farm-iron.md) apply unchanged unless this doc says otherwise.

## Use

One compact spawn station that makes golems far faster than a module, because the villagers come
to it in turns instead of waiting out their own lockout in place.

## The number

Golems per hour per station (and iron per hour: about 4 iron a golem), against the villagers it
needs (villagers per golem an hour), at the stage's build minutes with travel and height.

## The mechanics, from the game code (26.3)

- **The lockout belongs to the villagers, not the place.** `Villager.spawnGolemIfNeeded(level,
  t, 3)` lists the villagers whose box meets the summoner's box inflated by 10, keeps those that
  `wantsToSpawnGolem` (at most 5), and with 3 or more tries `SpawnUtil.trySpawnMob(IRON_GOLEM, …,
  10, 8, 6, LEGACY_IRON_GOLEM)`; on a spawn EVERY villager in that box gets
  `GolemSensor.golemDetected` (`GOLEM_DETECTED_RECENTLY`, 599 gt). Any villager not in the box
  keeps wanting a golem.
- **Attempts come on a 100 gt grid.** `VillagerPanicTrigger` (CORE, always active) runs
  `spawnGolemIfNeeded` when `timestamp % 100 == 0` while the villager is panicking; panic needs
  `NEAREST_HOSTILE`, which `VillagerHostilesSensor` sets from the visible-entity list
  (`NearestLivingEntitySensor`, a scan every 20 gt): a zombie within 8 blocks, feet to feet, in
  sight. A group that arrives sees the zombie within 20 gt of its first scan and tries at the next
  multiple of 100.
- **Wanting a golem needs a night in bed.** `wantsToSpawnGolem` = `LAST_SLEPT` within 24,000 gt
  and no `GOLEM_DETECTED_RECENTLY`. A villager sleeps only in its claimed bed (`SleepInBed` in the
  REST activity, walking to HOME), never in a cart.
- **Seeing a golem locks a villager too.** `GolemSensor` (every 200 gt, random phase) locks a
  villager whose living-entity list holds an iron golem: the box inflated by `FOLLOW_RANGE` (16 x
  (1 + triangle(0, 0.11485)), 14.2 to 17.8) on every axis, no sight test, so a golem whose feet
  are within R + 1 sideways or R + 2.7 below a villager's feet.
- **A golem cannot die at once**: 100 health, fall-immune; a lava kill takes 240 gt (25 hits at
  t = 0, 10, …, 240). It must LEAVE the station's cubes: a free fall to the lowest villager's
  y - 20.54 takes about 20 gt.
- **Carts.** A rideable minecart on powered rail reaches 0.4 blocks a tick (8 a second); a moving
  cart that touches a mob other than a player or an iron golem takes it as a passenger; a powered
  activator rail ejects the passenger (`AbstractMinecart.activateMinecart`). Detector rails and
  pressure plates give the events.

## The ceiling

One golem per 100 gt per station (the attempt grid): 720 golems an hour, about 2,880 iron an
hour, needing at least 6 groups of 3 rotating (each group locked 600 gt). With 4 groups on a
200 gt cycle: 360 golems an hour, about 1,440 iron. A module's ceiling is one golem per 600 gt
(120 an hour, about 480 iron).

## Trade-offs

- Each extra group buys one more attempt slot per 600 gt, up to 6; villagers cost their moves
  (or breeding), beds and carts.
- The queue must stay out of the station's 10-box at every spawn and out of every live golem's
  cube at every sensor scan: distance, so loop length, so blocks and travel.
- Short cycles are fragile: arrival, a 20 gt sensor scan and a panic start must fit between two
  attempt ticks, and the last golem must have left the cubes; a 200 gt cycle forgives all three.
- The nightly bed stop costs a dismount, a bed per villager (the Beds rule: each villager's only
  reachable free bed is its own) and a re-boarding by a moving cart at dawn.
- An event-driven advance (the golem's landing or fall closes a detector) follows the farm's own
  rhythm and cannot drift; a free clock is simpler but loses slots when an attempt fails.

## Layouts

| # | layout | golems/h per station (estimate) | % of ceiling | what it brings |
|---|---|---|---|---|
| L0 | the bar: a static module (the module component, measured) | 111 (444 iron/h) | 15% | nothing moves; one cluster's lockout |
| L1 | minecart carousel: groups of 3 villagers in carts on a powered loop, a station in sight of the zombie over one pad and drop, the advance fired by the golem's fall, a nightly stop of beds by activator rails | 360 at 4 groups x 200 gt, 720 at 6 x 100 | 50-100% | rails, a loop 11+ blocks out of the station's box, the bed stop |
| L2 | boat carousel: villagers in boats on a water loop, a gated stop at the station, the gate opened by the golem's fall | 300-360 (boats move slower than carts, about 0.1-0.4 a tick on water) | 40-50% | day-1 materials (boats, water, trapdoors); gates per stop |
| L3 | static docks: 2-3 clusters more than 10.6 apart around ONE shared pad and drop, each locking only itself | 200-300 (each cluster's golems dwell in the others' cubes on the pad: the LT measurement kept 72%) | 30-40% | no moving parts; dwell on the shared pad is the loss |
| L4 | pods moved by slime-block machines past the station | up to 720 | up to 100% | heavy redstone, late only; fragile on unload |

## Decision

Build L1 first (the parent's choice): a void prototype by the ironfarm lane at the forgiving
200 gt cycle with 4 groups, the advance driven by the golem's fall; then farmopt searches its
parameters (groups 4-8, the cycle 100-200 gt, the loop's length and shape, the bed stop, the
station's pad and drop). L2 and L3 stay in the search as the cheaper families.

## Must-haves (hard rules)

- A group at the station is panicking on an attempt tick (it sees the zombie after its sensor's
  scan), unlocked, and slept within 24,000 gt.
- Every waiting group is more than 10.6 blocks (box) from the summoner at the spawn tick, and out
  of every live golem's follow box (R + 1 sideways, R + 2.7 below) whenever its GolemSensor scans.
- The golem leaves the station's cubes within a few ticks (a drop below the cubes, or a kill that
  fast), and dies in the kill as the parent brief says (no golem outlives 1,200 gt).
- Every villager sleeps in its own bed once per game day (the nightly stop: dismount, beds,
  re-boarding by a moving cart), the Beds rule holds, and the farm restarts after days away (a
  stalled advance recovers: a fallback pulse when no golem has come for a full cycle).
- No escapes (villagers, golems, the zombie), the hostile and sky rules of the parent brief.

## Objectives

Golems per hour per station (max), villagers per golem an hour (min), blocks (min), build minutes
with travel and height (min), volume (min), regularity (min irregularities, as iteration 17
counts them).

## Not goals

The perfect 100 gt cycle (a slower, sure cycle beats a fast fragile one on the card's lowest
seed); exact cart-to-villager assignment; any order of groups.

## The obvious design's measured numbers

L0, the static module (module-two-watchers with the regular cage, AI on): 408 iron/h; the ring
module 444 iron/h (111.4 golems/h; the game 112.6-114.2).

## Ceiling

720 golems an hour per station (one per 100 gt), about 2,880 iron an hour.
