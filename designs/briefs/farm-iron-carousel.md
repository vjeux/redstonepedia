# Iron farm: the villager carousel (brief, G1)

vjeux 2026-11-08 22:53: "But the farm is soooo big and spread out. I'd love to pack tons of villagers
and move them around or something so we can have golems spawning constantly". This brief is the
family's G1 doc (docs/process.md step 1); the stages, prices, travel, parts and judge rules of
[farm-iron.md](farm-iron.md) apply unchanged unless this doc says otherwise.

## Use

One compact spawn station that makes golems far faster than a module, because the villagers come
to it in turns instead of waiting out their own lockout in place.

## The number

The stage rules of farm-iron.md, unchanged: the carousel competes in the middle and late searches
against copies with the SAME villagers (6 groups of 3 against 6 three-villager modules), on iron
an hour and build minutes per module of 3 villagers (late adds volume per module), priced the
same way. Per villager it cannot win: every golem locks at least 3 villagers for 600 gt
(`spawnGolemIfNeeded` needs 3 willing and locks everyone in its box), so no design passes 40
golems an hour per villager, the same ceiling a static 3-villager cluster has. Its win is
compactness: one pad, kill, zombie, calm and collection shared by every group, and fresh groups
that escape the static module's losses (below). Golems per hour per station is reported, not
ranked; L0, the single module, is a reference line.

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
  sight. A group that arrives sees the zombie within about 40 gt (the hostile sensor reads a list
  the living-entity sensor refreshes every 20 gt on its own phase) and tries at the next
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
- **Carts.** A rideable minecart on powered rail reaches 0.4 blocks a tick
  (`OldMinecartBehavior.getMaxSpeed`; the new behaviour needs the minecart_improvements flag); a
  cart moving at least 0.1 blocks a tick horizontally takes one mob it touches as its rider (not a
  player or an iron golem; `OldMinecartBehavior.pushAndPickupEntities`), one rider a cart, and a
  slower cart takes none; a powered activator rail ejects the rider (`Minecart.activateMinecart`).
  Detector rails and pressure plates give the events. Rails are priced (prices.tsv:
  `powered_rail`, `activator_rail`, `detector_rail`), so L1 is a middle or late design.
- **No sleeping in a cart.** `SleepInBed.checkExtraStartConditions` refuses a passenger, and a
  sleeper stays asleep while REST lasts (`canStillUse`) unless a hostile in sight from its sleeping
  eye (bed + 0.8875) makes it panic, when `WakeUp` wakes it.

## The ceiling

One golem per 100 gt per station (the attempt grid): 720 golems an hour, about 2,880 iron an
hour, needing 6 groups of 3 rotating (each group locked 600 gt: a villager due at T+600 is free
in its own behavior phase at T+600). With 4 groups on a 200 gt cycle: 360 golems an hour, about
1,440 iron. A module's ceiling is one golem per 600 gt (120 an hour, about 480 iron).

What the carousel must beat, the static module's two losses (measured on the bridge, the
tick-exact lane, 23:15): tick order (a summon slips to T+700 when the villager that checks at
T+600 ticks before the group freed at T+599) and golems lingering in a villager's `GolemSensor`
range (a re-detection sets the lock again: a summon at t300 with a villager re-detecting the golem
at t333 slipped the next summon to t1100). A 3-villager module's intervals average 664 gt (600 x
18, 700 x 14, 800 x 3, 900 x 1 in a day), a 4-villager module's 624-629. A carousel group arrives
fresh (no golem in range, its lock long expired), so neither loss applies to it, provided the
last golem has left the station's range before the next group's sensors scan.

Every number here assumes the station runs through the night, less the nightly stop: no villager
sleeps in a cart, so each group leaves the loop once a day to lie down (`LAST_SLEPT`), is woken
and re-boarded. With one bed (below), about 15 s a group a day, about 2.5% of a 100 gt rotation's
slots; a stop where every group lies down at once loses its own length; with no stop the night
(day time 12000 to the next 0) is lost, and 4 groups x 200 gt read about 180 golems an hour over a
day. Each estimate states its stop's length.

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
| L1 | minecart carousel: groups of 3 villagers in carts on a powered loop, a station in sight of the zombie over one pad and drop, the advance fired by the golem's fall, a nightly stop of beds by activator rails | 360 at 4 groups x 200 gt, 720 at 6 x 100 (through the night, less a 2.5% one-bed stop; about 180 at 4 x 200 with no stop) | 50-100% | rails, a loop 11+ blocks out of the station's box, the bed stop |
| L2 | boat carousel: villagers in boats on a water loop, a gated stop at the station, the gate opened by the golem's fall | 300-360 (boats move slower than carts, about 0.1-0.4 a tick on water) | 40-50% | day-1 materials (boats, water, trapdoors); gates per stop |
| L3 | static docks: 2-3 clusters more than 10.6 apart around ONE shared pad and drop, each locking only itself | 200-300 (each cluster's golems dwell in the others' cubes on the pad: the LT measurement kept 72%) | 30-40% | no moving parts; dwell on the shared pad is the loss |
| L4 | pods moved by slime-block machines past the station | up to 720 | up to 100% | heavy redstone, late only; fragile on unload |

## Decision

Build L1 first (the parent's choice): a void prototype by the ironfarm lane at the forgiving
200 gt cycle with 4 groups, the advance driven by the golem's fall; then farmopt searches its
parameters (groups of 3, 4 to 8 of them, the cycle 100-200 gt, the loop's length and shape, the bed stop, the
station's pad and drop). L2 and L3 stay in the search as the cheaper families.

## The nightly stop: a bed each, or a shared dormitory

Every villager must lie down once a game day (`LAST_SLEPT` is set as it starts sleeping). Two
options, judged against each other on the objectives:
- **A bed each** (the default): a stop with one bed per villager, each reachable only by its
  owner (the Beds rule), the riders ejected by activator rails and re-boarded by a moving cart.
- **A shared dormitory** (vjeux 23:10: "In the notes of the project there was a mention of being
  able to confuse villagers for their bed / working station, can you find it and see if that could
  be used here"; designs/corrupt-poi-entity/NOTES.md, R1, verified on the official server): a bed
  claimed by villager A, A taken more than 16 blocks away, the bed broken and replaced at the same
  position (a fresh POI record with a free ticket), and villager B claims it; both HOME memories
  persist through save and load, and repeating adds sharers (for example 3 beds for 7 groups of
  3). Rules: `ValidateNearbyPoi` runs only within 16 blocks and HOME only in REST, and a villager
  that validates its HOME in REST within 16 while its bed is OCCUPIED by another drops its claim
  (whichever order), so the dormitory sits more than 16 blocks from the station and from the
  waiting groups, one group is at the dormitory at a time, and the sharers lie down in turn
  (panicking villagers do not validate). Costs: a one-time break-and-replace setup per extra
  sharer (its hand steps priced), and a longer nightly stop.
- **One bed for all** (vjeux 23:16: "Okay can you figure out how to use a single bed"): the
  dormitory reduced to ONE bed shared by all 18 villagers (6 groups of 3) through the same double
  claim. Setup once: 17 break-and-replace steps, each with every earlier claimant more than 16
  away, priced as hand steps. Each night (REST), per villager: a cart drops it within 2.0 of the
  bed's head, and `SleepInBed` starts within a tick (HOME present, not a passenger, `LAST_WOKEN`
  at least 100 gt ago, the bed not OCCUPIED), setting `LAST_SLEPT`; an observer facing the bed
  sees OCCUPIED flip and opens a hatch on a dorm zombie; the villager panics within one
  `VillagerHostilesSensor` scan (20 gt), and the `WakeUp` core behavior wakes it (no longer in
  REST); OCCUPIED flips back, the observer closes the hatch, a cart takes the villager away, and
  the next comes in. While the bed is occupied, no other sharer in REST may be within 16 of it
  (`ValidateNearbyPoi` erases an awake validator's HOME when its bed is OCCUPIED), so the queue
  waits more than 16 out, and the leaver clears 16 (or is still panicking) before the next lies
  down: about 4-5 s a villager, about 90 s a night. A cart cannot take a sleeper (its box is
  0.2 x 0.2 at the bed head's centre, `setPosToBed`, and a cart's pickup box reaches about 0.69
  from its centre), so the pickup comes after the wake: `LivingEntity.stopSleeping` stands the
  villager up at `BedBlock.findStandUpPosition` (a fixed order of candidate cells around the bed,
  or on top of it when none is free), and the dormitory leaves exactly ONE stand-up cell free, on
  the rail, where a cart passing at 0.1 blocks a tick or more takes it. The dormitory sits more than 16 from the
  station and the waiting groups, and the dorm zombie and the station's zombie are each out of
  the other villagers' sight. The mob-AI lane confirms the pieces redsim must model exactly
  before it is judged: an observer on a bed's OCCUPIED change, `SleepInBed`'s start conditions,
  `WakeUp` on panic, and the double claims. **The nightly HOME release can break the sharing for
  all** (the G1 review): `SetWalkTargetFromBlockMemory(HOME, …, 1200)` in REST has no passenger
  check, so a sharer whose `CANT_REACH_WALK_TARGET_SINCE` is older than 1200 gt runs
  `Villager.releasePoi` (`PoiManager.release` on the bed's record, HOME erased); the bed's one
  ticket (`PoiRecord.releaseTicket`, capped at 1) then lets `AcquirePoi` give it to one homeless
  villager only, and the other releasers stay homeless and stop wanting golems a day later. Riders
  arm that memory (`MoveToTargetSink.tryComputePath` runs for passengers), and only a reached or
  reachable target or `SleepInBed.start` clears it. So the one-bed stop stands only with a
  mechanism, shown over many nights in the game gate, that keeps every sharer's memory younger than
  1200 gt at every REST tick (or keeps it unset), with mob-AI confirming the release, the shared
  bed's `releasePoi` and `AcquirePoi`'s re-claim first (and `CANT_REACH` arming for passengers);
  candidate mechanisms: every position a villager waits in has open, reachable floor around it,
  so its strolls clear the memory rather than arm it, or the loop gives each villager a reached
  walk target within 1200 gt before REST. Otherwise re-claiming the bed every night
  is priced as hand steps (the farm is then not AFK), or the stop falls back to a bed each. The
  bed-each stop goes first: a release there heals itself, each villager re-claiming its own free
  bed.

## Must-haves (hard rules)

- A group at the station is panicking on an attempt tick (it sees the zombie after its sensors'
  scans, up to about 40 gt after it arrives), unlocked, and slept within 24,000 gt.
- Every waiting group is more than 10.6 blocks (box) from the summoner at the spawn tick, and out
  of every live golem's follow box (R + 1 sideways, R + 2.7 below) whenever its GolemSensor scans.
- **Every way to summon, at every position a villager occupies** (the loop, the queue, the
  approach, the bed stop), judged along the loop: either the path cannot fire, or every spawn
  search from there lands only on pad or unspawnable cells. The panic path cannot fire when the
  villager is more than 8 from the farm's zombie or out of its sight except at the station, and no
  natural hostile can stand in its sight on a dark surface (the parent brief's every-villager
  rule); the gossip path (`InteractWith`, `TradeWithVillager`, `Villager.gossip`,
  `spawnGolemIfNeeded(5)`, off the 100 gt grid: a calm pair within the square root of 5 that see
  each other within 8, both 1,200 gt cooldowns over) cannot fire when waiting groups are more than
  10.6 apart and the bed stop is split so that no 5 villagers share a 10-box in sight (at dawn all
  of them are slept, unlocked and calm in IDLE).
- The golem leaves the station's cubes within a few ticks (a drop below the cubes, or a kill that
  fast), and dies in the kill as the parent brief says (no golem outlives 1,200 gt).
- Every villager lies down once per game day (the nightly stop: dismount, a bed, woken by a
  hostile in sight, re-boarded by a cart moving at least 0.1 blocks a tick), the Beds rule holds
  (a shared dormitory keeps its claims through the nightly HOME release of iteration 14, judged
  over many nights, not only through a save and load), and the farm restarts after days away (a
  stalled advance recovers: a fallback pulse when no golem has come for a full cycle).
- No escapes (villagers, golems, the zombie), the hostile and sky rules of the parent brief.

## Objectives

The stage's objectives against copies with the same villagers (The number). Reported: golems per
hour per station, villagers per golem an hour, the nightly stop's length, blocks, volume,
regularity (iteration 17's counts).

## Not goals

The perfect 100 gt cycle (a slower, sure cycle beats a fast fragile one on the card's lowest
seed); exact cart-to-villager assignment; any order of groups.

## The obvious design's measured numbers

L0, the static module (module-two-watchers with the regular cage, AI on): 408 iron/h; the ring
module 444 iron/h (111.4 golems/h; the game 112.6-114.2).

## Ceiling

720 golems an hour per station (one per 100 gt), about 2,880 iron an hour.
