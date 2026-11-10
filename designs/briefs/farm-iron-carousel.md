# Iron farm: the villager carousel (brief, G1)

## The design (vjeux Nov 9 22:25): three hard rules

vjeux 22:25: "The carousel is utterly terrible. I asked you to use the trick where we can only have
one bed. Also why did you make it a giant circle, this defeats the purpose of making it small. And
the villahers don't wake up at night... you didn't get any of the assignment". The rail carousel
(carousel-gate: three loops in a 43 x 43 x 31 box, a bed per villager, the station shut at night)
broke all three of his asks. It is the wrong design and leaves the site when the design below
lists. Every candidate meets all three rules; there is no first version without one of them.

1. **One bed for every villager.** All N = 21 villagers hold HOME = the same bed through the R1
   double claim (corrupt-poi-entity: a villager claims the bed, the bed is broken and placed
   again, the next villager claims the fresh record; N - 1 = 20 break-and-replace steps, priced as
   hand steps at build time). The design includes the mechanism that keeps every claim, and the
   judge proves it over many nights (6 days on 100 seeds at least; every villager's HOME = the bed
   at every midday and every dawn):
   - **CANT_REACH:** SetWalkTargetFromBlockMemory(HOME), in the REST package only, releases the
     bed and erases HOME when CANT_REACH_WALK_TARGET_SINCE is older than 1200 gt. A panicking hold
     still arms that memory (MoveToTargetSink is CORE and paths to PANIC's flee targets and 2-block
     strolls; an unreachable one sets it, and it keeps its first-failure time into REST), and at the
     napper's first REST tick SetWalkTargetFromBlockMemory (priority 2) runs before SleepInBed (3).
     Two paths, release-and-reclaim the default: (a) the napper may release the bed at its first
     REST tick and reclaims it through AcquirePoi(HOME) (CORE priority 10, a scan every 20-40 gt)
     because it is the only villager in REST, the bed is free and reachable from beside it, and the
     ticket cap is 1; the nap's timing budgets the reclaim (≤ 40 gt), and the judge checks every
     villager's HOME = the bed at every dawn and midday. The ≤ 40 gt reclaim holds only if the
     napper's first path to the bed succeeds: AcquirePoi scans when HOME is absent (at once if it
     scanned before; within nextInt(20) on a fresh brain), then every 20 + nextInt(20) gt, for a
     bed with a free ticket that isn't OCCUPIED; a failed path puts the bed in its batchCache with
     a jittered linear retry that waits longer each time. So the delivery leaves the bed unoccupied
     and reachable from where the napper stands at its release, or the bench shows the retry fits
     inside the visit. (b) The alternative: holds and lanes generate no unreachable panic walk
     target, and the bench logs every held villager's CANT_REACH every tick. A rider whose path
     from its cart reaches the bed never arms it (bridge scene bed-cart); reaching the walk target
     or starting to sleep clears it.
   - **The occupied erase:** no other sharer in REST within 16 of the bed while it's occupied
     (ValidateNearbyPoi(HOME), in the REST package only, erases the HOME of any villager within 16
     of an occupied bed that isn't the sleeper). So a nap happens only when every other sharer is
     farther than 16 from the bed or not in REST (panicking).
   - No second bed within any villager's POI search: a free bed would be claimed and break the
     sharing.
2. **Compact.** The target is the portal-pad station of "Golems removed by portal, and the
   smallest box" (about 24 x 12 x 5 at 99% summon success), with the queue and the bed inside a
   small box. The objective is the whole build's box volume, the queue and the bed included. No
   rail loops. Villagers move in short hops: carts on short shuttles, water streams, pistons,
   whatever measures smallest. The lock box still holds (waiting groups more than 10.6 from every
   summoner on one horizontal axis, or 11.95 vertically, at every summon tick), and so does the
   occupied-erase distance (16 from the bed during a nap); the layout is the smallest arrangement
   of the station, the queue and the bed that keeps both. GolemSensor re-locks any villager whose
   scan still sees a golem within its follow range (16, up to 17.8 with the spawn bonus; no sight
   test), and the detection is phase-locked to the slot grid (the sensor periods 20 and 200 divide
   the slots, so an exposed villager is locked at every other golem, permanently). So during each
   golem's overworld dwell (T to T+2), every villager outside the summoning batch, the arriving
   batch included, stays outside the golem's follow box (19.3 sideways, 19.8 below or 20.6 above
   the golem's feet).
3. **Runs at night.** Each villager naps once a day in the one bed, OCCUPIED for up to about 39 gt
   (the wake's latency, two sensors). It is
   delivered within 2.0 of the bed and not riding (SleepInBed's conditions; it can't sleep again
   within 100 gt of a wake). Falling asleep sets LAST_SLEPT (the golem condition needs a sleep
   within the last 24,000 gt). An observer on the bed's OCCUPIED flashes a dorm zombie into its
   sight; the panic wakes it at once (WakeUp); it stands up at the bed's one free stand-up cell
   and is carried straight back into the rotation. The station summons through the night. The
   judge prints golems per hour by day and by night; a night rate short of the day's by more than
   the naps' share is a defect to fix, not a trade-off.
4. **Seven groups of three.** 21 villagers in 7 groups of 3, one station, one golem per 100-gt
   slot. With 7 groups each group gets 700 gt between turns: 100 gt of slack after its 600-gt
   lockout, which absorbs travel jitter, the panic onset (up to 39 gt after the zombie becomes
   visible: NearestLivingEntitySensor, then VillagerHostilesSensor, each every 20 gt on its own
   random phase; the bridge measured 8-18, a few of the 400 phase pairs) and
   tick order, so every slot fills. The one bed serves all 21, visited one villager at a time
   during the night with the queue more than 16 from the bed. The compact box holds all 7 groups.
   The carousel's own line is slot fill: golems per 100-gt slot, by day and by night, printed on
   the card.

Unchanged: ranked against copies with the same villagers on volume and on rate per villager; every
summon path closed at every position (panic only at the station, gossip groups apart); golems
removed by portal, with the Nether side's exit kill and portal-ticket rules; the judge counts riders
at every midday and HOME holders at every dawn.

Who: mob AI confirms on the bridge ValidateNearbyPoi's reach and its occupied branch, SleepInBed's
conditions and the wake (bed-observer, bed-r1, bed-cart are exact; the occupied-erase scene is
pending) and models the Nether side for the portal pad; ironfarm sketches the compact layout now and
builds it; farmopt searches its parameters once one runs.

### The operating cycle

vjeux 22:29: "The idea is to send 3 villagers that get frightened spawn a golem and then send the
batch of 3 villagers and repeat. Then at night we put villagers to be and wake them up one by one
once during the night. Not at every step." A batch of 3 arrives at the station, panics, summons a
golem, and leaves; the next batch arrives 100 gt later; the 7 batches repeat in turn, day and
night. Only the summoner must panic; the other villagers it counts need only be willing
(LAST_SLEPT within 24,000 gt, no GOLEM_DETECTED_RECENTLY) and inside its 10-box; golemDetected
locks every villager in that box for 600 gt (erased at T+599 or T+600 by brain-tick order), which
is why a batch's turn comes every 700 gt. The bed is visited once per night by each villager, one villager at a time: it lies down
(which sets LAST_SLEPT), is woken at once, and rejoins its batch, the visit slotted between two of
that villager's station turns (inside its wait between turns). It is not a stop on every cycle.
The once-a-night selection (each villager drops at the bed exactly once per night, never twice,
never skipped) is part of the design, and the judge checks it: every villager's LAST_SLEPT
advances once each night.

### The timing diagram

vjeux 22:30: "The timings are tight so need to be precise. This is not an easy task". The carousel
is designed from a tick-level timing diagram, and the layout report carries it: the slots sit on
the gameTime % 100 grid (summon attempts happen only there); a batch arrives at least 39 gt
before its slot (the panic onset, up to 39 gt after the zombie becomes visible; none for a batch
that arrives already panicking with a zombie kept in its sight the whole way), with the counter's
2-miss drift (2m, 12-20 gt) on top; during each golem's dwell (T to T+2) the arriving batch is
still outside the golem's follow box; the summon happens at the slot; the golem leaves through the
portal 1-2 gt later; the leaving batch is out of the station's 10.6 box before the next slot; each
villager's once-a-night bed visit fits inside its 600-gt wait, with the rest of the queue more than
16 from the bed while it sleeps. Dispatch is event-driven from each golem spawn, so the farm
phase-locks to the game clock, and a slot with no golem still advances the rotation on the next
grid tick (a fallback clock), so one failed summon never stalls it. Every interval in the diagram
is checked tick by tick in redsim and on the bridge, and G1 checks the diagram against the code.

### The parts: built, tested and optimized alone, then assembled

vjeux 22:32: "Also you probably want to build the counter, sleep routine, 3 villager release + hold
for the rest separately and optimize each independently before coming with a final assembly".
Four parts, each with a typed interface (inputs, outputs, a timing contract), its own tick-exact
redsim bench (bridge scenes where the pieces exist), its own objectives (blocks, box volume, build
minutes, timing margin), optimized alone and archived like the clocks. The assembly comes after;
the timing diagram checks every coupling.

1. **The counter.** In: one pulse per golem spawn (the station's detector), plus the fallback
   clock's pulse on a slot with no golem. Out: 7 lines, one per batch, exactly one high at a time,
   advanced one state per pulse. Contract: the next line is high within T_c gt of the pulse (T_c
   is budgeted in the diagram); no skipped or doubled state at any pulse spacing the farm can
   produce; the state is held in blocks, so it survives an unload, a reload and a restart. Bench:
   random pulse trains on the 100-gt grid with missed slots and early or late pulses, 100,000
   pulses, zero skips or doubles, plus the restart test. Objectives: blocks, volume, build
   minutes, T_c and its margin.
2. **Release + hold.** In: the counter's 7 lines and the station's take-back pulse. Out: exactly
   the selected batch of 3 at the station's stand cells, then back in its own hold slots; the
   other 18 held more than 10.6 from every summoner on one axis at every summon tick, with paths
   that keep CANT_REACH young (rule 1). Held batches may panic (hold zombies in sight, out of
   reach) so they never run REST: then the hold's own summon search has no spawnable floor (a
   static check), and only the napper is calm. The static gate covers every position a villager
   occupies (hold lanes, travel paths, stand cells, the bed's area: a bed is likely a valid golem
   floor) for valid floors in its summon search, the pad counted only for the stand cells; with 7
   batches a batch's lock ends exactly at an attempt tick, so any valid floor anywhere a willing,
   panicking villager waits summons out of turn. Contract: the batch reaches its stand cells at
   least 39 gt before its slot (the panic onset; none for a batch that arrives already panicking),
   with the counter's 2-miss drift on top; the leaving batch is out of the 10.6 box before the
   next slot; the round trip fits the batch's 700 gt. Invariants: never two batches at the
   station, no villager left behind, every villager back in its own slot. Bench: 7 batches
   cycling with the counter driven ideal and the villager AI on, positions logged every tick, zero
   violations over 6 days on 100 seeds. Objectives: blocks, volume, the arrival margin, build
   minutes.
3. **The sleep routine.** In: the night window (a daylight-sensor signal) and the counter's
   state, so it never takes a villager whose batch is due. Out: each of the 21 villagers once a
   night, one at a time, within 2.0 of the one bed and not riding; it sleeps, the observer on
   OCCUPIED flashes the dorm zombie, it wakes, stands up at the one free cell and returns to its
   hold slot before its next turn. Sequence: out of every zombie's sight, the villager calms within
   one sensor scan pair (up to 39 gt), is within 2.0 of the bed and not riding, SleepInBed starts
   (LAST_SLEPT set at start: a one-tick nap counts), the observer on OCCUPIED opens a hatch on a
   dorm zombie, it panics and wakes within up to 39 gt (measured 18), stands up at the one free cell,
   and is taken back; SleepInBed can't restart until 40 gt after a wake (nextOkStartTime) and 100
   gt after LAST_WOKEN. With the panicking hold, only the napper is in REST, so the occupied erase
   can't hit anyone else. Invariants: every villager's HOME = the bed at every dawn and
   midday (the R1 claims kept); while the bed is occupied, every other villager is in PANIC or more
   than 16 from it; each villager's LAST_SLEPT advances exactly once a night. Contract: each visit fits
   inside that villager's wait; the 21 visits fit inside the night. Bench: 21 villagers with R1
   claims, 6 days on 100 seeds, the station and counter driven ideal, HOME, LAST_SLEPT and
   distances logged every tick. It waits on tick-exact's bed pieces (ValidateNearbyPoi's reach,
   SleepInBed's conditions, the wake delay, findStandUpPosition). Objectives: blocks, volume, the
   visit's length and its margin, build minutes.
4. **The station + portal pad.** In: a batch of 3 at the stand cells. Out: the batch panics (the
   zombie in sight, out of its melee reach) and summons at the slot; the golem lands on the trench
   pad and leaves through the portal in 1-2 gt; a golem-spawn pulse for the counter; the Nether
   side's exit kill and collection (or the drops sent back). Contract: summon success per slot (k
   valid columns: 1 - (1 - k/289)^30 for a batch's 30 attempts), the golem's overworld dwell, the
   pulse's delay. Invariants: no golem stays in the overworld; no summon path for a held villager;
   every box meant to stay keeps out of every portal cell. Bench: a batch held at the stand cells
   in the void, 10,000 slots (the Nether side once modeled). Objectives: blocks, volume, summon
   success, build minutes (obsidian priced).

The assembly places the four parts in the compact box; the timing diagram checks each coupling
(golem pulse → counter → release → arrival margin → slot → take-back; the sleep routine's visits
against the counter), and it is judged as a farm: slot fill, iron/h, volume, against copies with
the same villagers. Build order: the counter (day-1 lane), release + hold (ironfarm) and the station
(farmopt) in parallel; the sleep routine (ironfarm) once tick-exact's bed pieces are exact; then the
assembly (ironfarm).

vjeux 2026-11-08 22:53: "But the farm is soooo big and spread out. I'd love to pack tons of villagers
and move them around or something so we can have golems spawning constantly". This brief is the
family's G1 doc (docs/process.md step 1); the stages, prices, travel, parts and judge rules of
[farm-iron.md](farm-iron.md) apply unchanged unless this doc says otherwise.

## Use

One compact spawn station that makes golems far faster than a module, because the villagers come
to it in turns instead of waiting out their own lockout in place.

## The number

The stage rules of farm-iron.md, unchanged: the carousel competes in the middle and late searches
against copies with the SAME villagers (7 groups of 3 against 7 three-villager modules: 7 x 408 =
2,856 iron/h for the 6x6 module, about 2,982 with the 8x8 module at 426), on iron
an hour and build minutes per module of 3 villagers (late adds volume per module), priced the
same way. Per villager it cannot win: every golem locks at least 3 villagers for 600 gt
(`spawnGolemIfNeeded` needs 3 willing and locks everyone in its box), so no design passes 40
golems an hour per villager, the same ceiling a static 3-villager cluster has. Its win is
compactness: one pad, kill, zombie, calm and collection shared by every group, and fresh groups
that escape the static module's losses (below). Golems per hour per station is reported, not
ranked; L0, the single module, is a reference line. On rate it cannot win against the bar: its
ceiling (720 golems an hour, about 2,880 iron) is below 7 copies of the 8x8 module (about 2,982
iron an hour), so the carousel's case is volume and build minutes, not rate.

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
  sight. A group that arrives sees the zombie within up to 39 gt (two sensors: the hostile
  sensor reads a list the living-entity sensor refreshes, each every 20 gt on its own random
  phase) and tries at the next multiple of 100.
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
hour, needing 7 groups of 3 rotating (rule 4: each group locked 600 gt, a turn every 700 gt with
100 gt of slack). With 4 groups on a 200 gt cycle: 360 golems an hour, about
1,440 iron. A module's ceiling is one golem per 600 gt (120 an hour, about 480 iron).

What the carousel must beat, the static module's two losses (measured on the bridge, the
tick-exact lane, 23:15): tick order (a summon slips to T+700 when the villager that checks at
T+600 ticks before the group freed at T+599) and golems lingering in a villager's `GolemSensor`
range (a re-detection sets the lock again: a summon at t300 with a villager re-detecting the golem
at t333 slipped the next summon to t1100). A 3-villager module's intervals average 664 gt (600 x
18, 700 x 14, 800 x 3, 900 x 1 in a day), a 4-villager module's 624-629. A carousel group arrives
fresh (no golem in range, its lock long expired), so neither loss applies to it, provided the
last golem has left the station's range before the next group's sensors scan.

(Superseded by the three hard rules: the one-bed nightly visit, once per villager per night.)
Every number here assumes the station runs through the night, less the nightly stop: no villager
sleeps in a cart, so each group leaves the loop once a day to lie down (`LAST_SLEPT`), is woken
and re-boarded. With one bed (below), about 15 s a group a day, about 2.5% of a 100 gt rotation's
slots; a stop where every group lies down at once loses its own length; with no stop the night
(day time 12000 to the next 0) is lost, and 4 groups x 200 gt read about 180 golems an hour over a
day. Each estimate states its stop's length.

## Trade-offs

- Each extra group buys one more attempt slot per 600 gt, up to 6 (superseded by the three hard
  rules: 7 groups of 3, a turn every 700 gt); villagers cost their moves
  (or breeding), beds and carts.
- The queue must stay out of the station's 10-box at every spawn and out of every live golem's
  cube at every sensor scan: distance, so loop length, so blocks and travel.
- Short cycles are fragile: arrival, a 20 gt sensor scan and a panic start must fit between two
  attempt ticks, and the last golem must have left the cubes; a 200 gt cycle forgives all three.
- The nightly bed stop costs a dismount, a bed per villager (superseded by the three hard rules:
  one bed for all 21; the Beds rule: each villager's only
  reachable free bed is its own) and a re-boarding by a moving cart at dawn.
- An event-driven advance (the golem's landing or fall closes a detector) follows the farm's own
  rhythm and cannot drift; a free clock is simpler but loses slots when an attempt fails.

## Layouts (superseded by the three hard rules)

| # | layout | golems/h per station (estimate) | % of ceiling | what it brings |
|---|---|---|---|---|
| L0 | the bar: a static module (the module component, measured) | 111 (444 iron/h) | 15% | nothing moves; one cluster's lockout |
| L1 | minecart carousel: groups of 3 villagers in carts on a powered loop, a station in sight of the zombie over one pad and drop, the advance fired by the golem's fall, a nightly stop of beds by activator rails | 360 at 4 groups x 200 gt, 720 at 6 x 100 (through the night, less a 2.5% one-bed stop; about 180 at 4 x 200 with no stop) | 50-100% | rails, a loop 11+ blocks out of the station's box, the bed stop |
| L2 | boat carousel: villagers in boats on a water loop, a gated stop at the station, the gate opened by the golem's fall | 300-360 (boats move slower than carts, about 0.1-0.4 a tick on water) | 40-50% | day-1 materials (boats, water, trapdoors); gates per stop |
| L3 | static docks: 2-3 clusters more than 10.6 apart around ONE shared pad and drop, each locking only itself | 200-300 (each cluster's golems dwell in the others' cubes on the pad: the LT measurement kept 72%) | 30-40% | no moving parts; dwell on the shared pad is the loss |
| L4 | pods moved by slime-block machines past the station | up to 720 | up to 100% | heavy redstone, late only; fragile on unload |

## Decision (superseded by the three hard rules)

Build L1 first (the parent's choice): a void prototype by the ironfarm lane at the forgiving
200 gt cycle with 4 groups, the advance driven by the golem's fall; then farmopt searches its
parameters (groups of 3, 4 to 8 of them, the cycle 100-200 gt, the loop's length and shape, the bed stop, the
station's pad and drop). L2 and L3 stay in the search as the cheaper families.

## The nightly stop: a bed each, or a shared dormitory (superseded by the three hard rules)

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

## Golems removed by portal, and the smallest box

vjeux 23:24: "What's the smallest theorical bounding box for this? (Assuming we use nether portal
to move the golems away)". A golem leaves the overworld within a tick or two of touching a portal
(`NetherPortalBlock.getPortalTransitionTime` is 0 for a non-player and `Entity.canUsePortal`
holds for a golem), so it dwells 1-2 gt (T to T+2) in the follow boxes; that detection is
phase-locked to the slot grid (an exposed villager is locked at every other golem), so every
villager outside the summoning batch stays outside them (rule 2 of the design's rules), and the
station needs no drop and no overworld kill.
- **Contact is per cell.** `NetherPortalBlock` keeps the default inside shape (a full block):
  `Entity.checkInsideBlocks` counts every portal cell the entity's box (deflated by 1e-5) meets,
  and `entityInside` sends it, whatever the portal's axis. So the pad is 1-wide trenches with a
  portal wall running alongside, any orientation, every trench cell beside a portal cell (a
  golem's 1.4-wide box reaches 0.2 into each side cell); the frame's end columns stand in the
  wall's line, so longer portals (up to 21 interior) waste fewer trench cells. The same rule
  binds everything near the station: any box overlapping a portal cell crosses (a minecart with
  its villagers, the zombie's boat, drops), so every box meant to stay keeps out of every portal
  cell entirely, the station's carts and the zombie's window clear of the walls.
- **The pad's success.** Every other cell of the spawn search (17 x 17 columns, floors from y+5
  down to y-7) is unspawnable (in the void, empty). Each attempt picks a column uniformly in the
  square, and each willing, panicking villager of the group runs its own 10 attempts at the
  multiple of 100, so a group of 3 makes up to 30: with k valid columns a summon succeeds with
  1 - (1 - k/289)^30, k about 21 for 90% and about 41 for 99%. Obsidian frames are priced (middle
  and late).
- **The lock box.** Waiting groups are more than 10.6 from every summoner on one horizontal axis
  (or 11.95 vertically) at every spawn tick; entering or leaving between attempt ticks is fine.
- **No queue in the Nether.** All the overworld portals within about 15 blocks link to the same
  Nether portal (`PortalForcer.findClosestPortalPosition`: the nearest within 16 Nether blocks of
  x/8, z/8), so golems would arrive where the queue waits and lock it (`GolemSensor`, no sight
  test); separate Nether portals 19 apart need overworld portals about 150 apart, so the queue
  stays in the overworld, outside the lock box.
- **The Nether side** (each a rule): a kill at the golems' exit (they come back once their 300 gt
  cooldown runs out); with no player there, the Nether side ticks only under portal tickets
  (`TicketType.PORTAL`: 300 gt, radius 3, entities ticking within 1 chunk of the arrival chunk
  and blocks within 2), so the kill and the collection sit in that area and each kill finishes
  within a ticket (crossings every ~100 gt renew it; lava's 240 gt fits from arrival); drops keep
  out of the Nether portal's cells (items use portals too); the restart rule covers an unloaded
  Nether side (after an absence it resumes with the next crossing).
- **The smallest box, estimated** (superseded by the design's rules: 7 groups of 3, the hold past
  the golems' follow box; this estimate was 6 groups of 3, the queue in the overworld): the pad is one
  portal wall 11 long with a trench on each side (22 trench cells) at 90%, two such walls (44) at
  99%, each about 13 x 3 with its frame and 5 high (the trench floor on the frame's bottom row, a
  3-tall interior, the top row); the station's 3 carts and the zombie (within 8, in sight) beside
  it; and the waiting carts more than 10.6 out on one axis. About 24 x 9 x 5 blocks (about 1,100
  of volume) at 90%, about 24 x 12 x 5 (about 1,450) at 99%, plus the bed stop and the Nether's
  exit kill; 6 copies of a 3-villager module fill about 44,600 (6 x about 7,440). Rate: 720
  golems an hour x the pad's success x (1 - the stop's 2.5%): about 632 golems an hour (about
  2,530 iron) at 90%, about 695 (about 2,780 iron) at 99%, against 6 copies' 2,448 iron an hour
  (module-two-watchers' 408 each). The mob-AI lane models portals exactly before any of it is
  judged (the per-cell contact, the transition time and cooldowns, a cart taking its rider
  through, the spawn rules beside portal blocks, linking, portal tickets).

## Must-haves (hard rules)

- A group at the station is panicking on an attempt tick (it sees the zombie after its sensors'
  scans, up to 39 gt after the zombie becomes visible), unlocked, and slept within 24,000 gt.
- Every waiting group is more than 10.6 blocks (box) from the summoner at the spawn tick, and
  outside every golem's follow box (19.3 sideways, 19.8 below or 20.6 above the golem's feet)
  during its overworld dwell (T to T+2), the arriving batch included.
- **Every way to summon, at every position a villager occupies** (the loop, the queue, the
  approach, the bed stop), judged along the loop: either the path cannot fire, or every spawn
  search from there lands only on unspawnable cells (the pad counts only for the stand cells: a
  willing, panicking villager anywhere else summons out of turn). The panic path cannot fire when the
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

## The rail carousel, measured and parked (2026-11-09)

What the minecart carousel (`mobfarm gen carousel`, 3 concentric loops, 4 stops each, 12
villagers, the station over the pad) reached after vjeux's "Fix the sleeping station" (2026-11-09
06:35), judged in redsim with the riders counted at every midday (`mobfarm seeds <design> --from 1
--to 100 --days N`):

| variant | seeds 1-100, 3 days | 101-200, 3 days | 1-100, 6 days |
|---|---|---|---|
| `carousel` (main, the stopped night) | 21 hold | - | - |
| a closed trapdoor over each brake (the waking villager can never push its parked cart) + tuned boosters | 88 | 80 | 54 |
| `carousel-gate`: a gate at every stop (below) | 87 | 82 | 72 |

The gate (branch explore/ironfarm-gate): an iron trapdoor in the cell over each brake, held open by
a torch, its plate on that cell's front face; the slope always boosts, so every arriving cart rolls
into the plate (a cart's box is lifted to y 0-0.7 on a slope cell) and parks at one spot, 0.19
short of the pocket. The release turns the torch off (the bus climbs over the track on glass into
its block), the trapdoor shuts to its top half and the cart rolls under it. Shut, it is the brake
column's villager-proof cover, so the villager's back never comes nearer than 0.19: every pocket
position is then inside the pickup's window (a released empty cart touching its villager under 0.1
b/t stays pinned; its 5th and 6th boosted steps outgrow the 0.2 reach). An unpowered open trapdoor
over the track side of the bed's foot keeps villagers from stepping into the brake column. Left
failing: a villager whose HOME is at another stop walks into its cart as the gate opens, closing
faster than the window; its push starts the cart back up the always-boosting slope and it runs the
loop in reverse. A lightning rod in the middle of the inner loop takes every bolt within 128: a judge
build lost the station zombie's boat to lightning (redsim does not model the rider's dismount from a
destroyed vehicle), and with the rod the judge's builds fail only on the villager left unslept.

Passing seeds: 12 of 12 riding at every midday, about 108 golems and 430 iron an hour; 211 build
minutes at middle prices; box 43 x 43 x 31 = 57,319 (14,330 per group of 3). Against the late
stage's compact module (409 iron/h per group of 3, 15.2 build minutes, 3,968 volume) and four
3-villager copies (about 1,632 iron/h, 135 minutes), the rail carousel has 3.6 times the volume per
group at a quarter of the rate. It is listed as a middle variation with these numbers (vjeux's
"just upload them": a dominated candidate is listed, its robustness a measured line), and further
work on it is parked; the portal pad (golems removed by portal) stays the compact version, for when
mob AI's Nether side lands.
