# Iron farm: the villager carousel (brief, G1)

## The design (vjeux Nov 10 05:58): one lane, back to back, released 3 by 3

vjeux 05:58: "Okay, I don't think that my request has been correctly understood. I don't want one lane per villager. I want a single lane where the villagers are riding on back to back with a mechanism to release them 3 by 3".

- **One lane.** All 21 villagers ride minecarts back to back in a single queue on one track. A release at the queue's head lets exactly the front 3 carts go to the station; after the summon they ride on and rejoin the queue's tail (a compact loop; the volume objective decides its shape).
- **No counter.** The queue's order is the 7-batch rotation: each golem pulse releases the next 3, and a fallback releases them on a slot with no golem (the counter's fallback contract carries over: the last release + 100 + m, m in [6, 10], only without a golem pulse).
- **What stands:** rules 1-4 and the timing diagram: one bed for all 21 (R1) with its claims kept; compact; runs at night, each villager napping once a night (a second nap only as the rare escape); 7 batches of 3, one golem per 100-gt slot. And every per-position rule: the lock box at every summon tick; the follow box for every villager outside the summoning batch during each golem's dwell, the leaving batch included (a re-lock at T+100..T+102 would outlast its next slot); the static gate on every position a villager occupies (queue, release, station, return path, bed); PANIC at every night tick except the napper; the occupied-erase distance.

### Parts (single lane), built and benched alone, then assembled

1. **Queue + release-3** (ironfarm). In: the golem pulse (1 gt after the spawn, 10 gt long) and the fallback. Out: exactly the front 3 carts released per trigger, the rest held still, back to back; the released 3 at the station's stand by T+100 − 39 gt (or arriving panicking, a zombie in sight throughout) and outside the follow box until T+2. The core risk, benched first: carts in contact (pushOtherMinecart averages the two carts' motion and keeps 0.2x each, which drove the 3-cart trains to the speed cap), so the held queue must stay still and the release must take exactly 3 every time, over 100 seeds x 6 days.
2. **The station, re-fit to a linear 3-cart stand** (farmopt): the 3 riders back to back on the track; each rider's ±8 search keeps the k the 99% bar needs; the rest of its contract stands (the portal pad, the piglins, the Nether kill and storage).
3. **The return path to the tail** (ironfarm): the leaving batch out of the station's 10.6 box before the next slot and out of every later golem's follow box during its dwell; PANIC at night; no valid golem floor in any rider's search; it rejoins the tail without disturbing the held queue.
4. **The sleep routine inside the loop** (ironfarm's mechanics, day-1's scheduler): each villager once a night, alone, to the one bed and back into its own cart before its batch reaches the head again (a cart reaching the station empty leaves a batch of 2, which can't summon); the BLOCK policy and the gap-window contract carry over, keyed to the queue's release order instead of the counter's lines.
5. **Assembly**: the timing diagram checks every coupling (golem pulse → release → arrival margin → slot → leave → return → tail; the sleep visits against the queue).

#### Part 1: the stand catch (decided Nov 10, 9:49 AM)

Baseline: the 2-tick redstone catch, which powers the stand's brake at +27/+28 gt after the stand detector fires (the bench's +26/+27; redstone +28 is wired). It is accepted so the wired run proceeds, and it is bridged against the real game (a bridge scene of the catch, tick-exact against redsim). In parallel, farmopt and ironfarm build a stop-based stand: a physical stop with no brake timing (about 15 gt faster), with re-spacing on the return path to restore the queue's 3-cell period. The stop-based stand replaces the baseline once it passes the same benches (the full-loop bench and the station bench with the real release).

#### Tolerance, a measured line for every part with timed controls

Every carousel part with timed controls prints its tolerance: how many ticks each control's timing can move early and late, and how much cart-speed perturbation at the part's entry (± blocks per tick) it survives with its contract met. Each is measured on the part's bench by sweeping one perturbation at a time to the first failure. The assembly prints the same for its couplings. A part with 0 ticks of tolerance in either direction on any control is flagged.

Each part's tolerance line also includes two corner runs: every timed control at its measured early limit together, then every one at its late limit, each with the entry-speed extreme, so a part whose margins hold only one at a time shows up; the assembly repeats the corners across its couplings.

#### Every stateful element recovers by itself (fault injection)

Every stateful element in a part (latch, copper bulb, counter, register, ring) recovers by itself from one missed pulse and from one extra pulse. The farm returns to clean operation within a stated number of cycles, with no permanent phase inversion and no hand reset. It's checked by fault injection on the part's bench: at each stateful element, drop one input pulse, and separately add one extra input pulse, in each distinct state the element can be in when the pulse is due, over several seeds. The part's line reports, per element, the worst-case cycles to recover and what was lost in the meantime (slots, naps, golems). An element that never recovers fails the part. The injected faults also include an unload and reload at each state (the player leaving and coming back mid-cycle, mid-pulse included), since that's how an AFK farm actually drops or doubles a pulse; the element's state must survive it or recover within the stated cycles. The unload/reload uses the shared act (`unload <chunks> <N>`; `queue-loop-run --act SLOT,DT,unload,N`) at each state, mid-cycle and mid-pulse, with N = 0, N = a few ticks, and N longer than the element's longest timer, and both the whole part and a partial unload with a chunk boundary through the part, in each of its three stages: (a) block-ticking, entities frozen (level 32: repeaters, observers, comparators, detector rails, block events and hoppers keep running; random ticks stop; carts and mobs freeze in place and resume with their velocity; so a release can fire on a frozen queue), (b) loaded, nothing ticking (33: scheduled ticks keep absolute times, and overdue ones fire at once when ticking resumes), (c) unloaded (34+: saved and dropped, ticks resumed at their remaining delays). The partial act `act T unload x0,z0,x1,z1 N` produces (a) and (b); the whole-part `act T unload all N` covers (c). Every stateful element's fault injection covers stage (a) at each state. Freezes are tested short and long: besides the short freezes, every phase is also run with each chunk the farm crosses frozen for 1,500 gt and for 5,000 gt. Game time keeps running in a frozen chunk, so timers that villagers keep in game time, such as the 1,200-gt release of an unreachable bed, jump by the freeze length. After any freeze, every villager must still hold the shared bed or regain it on its own before its next nap, with the naps and golems lost counted. A villager that loses the bed for good is a hard fail. A bed gives back only one claim, so if two villagers have lost it, only one can take it back. At no tick of the night may two villagers be resting calm with the bed out of reach, faults included, because a long freeze at that tick would cost both the bed. The tests print the most such villagers at any one tick, which must be 1 or less, and include two long freezes in one night at the worst moments the design has. A villager's unreachable-walk stamp is set when one of its walks can't reach, and cleared only by a walk that can, by the villager lying down in the bed (SleepInBed's start, which a passenger never reaches), or by a reload. Panic doesn't clear it. The first time the villager rests calm with no walk target while that stamp is over 1,200 gt old, it gives up the bed, freeze or not, and the stamp restarts, so the loss repeats every 1,200 gt while its walks keep failing. So the tests also print every tick on which a villager rests calm holding the bed with the stamp over 1,000 gt old, whatever its walk target. Outside the napper beside a free bed there must be none, and no villager may give up the bed in normal running. The ring at level 32 is also where the simulation distance ends, so it can be a steady state, not only a transient: a farm straddling it from the player's AFK spot would run its redstone with frozen carts for good. The brief states the AFK spot. Overworld parts: every block, cart path and villager position lies in entity-ticking chunks (level ≤ 31) from the stated AFK spot. The Nether side stays within the portal tickets' area (entities within 1 chunk of the arrival chunk, blocks within 2, renewed by crossings). Entity ticking covers every chunk within Chebyshev distance min(N, V) of the AFK spot's chunk (N = simulation distance, V = view distance; the player's ticket is at level 31 − N, rising by 1 per chunk step over all 8 neighbours). Ring N + 1 is level 32 and ring N + 2 is level 33. The farm is designed for N = V = 10, the dedicated-server defaults (the smaller; singleplayer defaults to 12). Every overworld chunk of the farm lies within Chebyshev distance 10 of the AFK spot's chunk, a 21 × 21-chunk square. The assembly prints the footprint's minimum, max(|Δcx|, |Δcz|) over its overworld chunks, and its margin to 10. The AFK spot is stated as a block position, and the brief picks it at least a few blocks inside its chunk (away from all four chunk edges), so a player shifting a step doesn't move the centre chunk and the 21 × 21 square with it. The assembly prints the footprint's margin for every chunk the player could stand in within that tolerance. Why: the bed siding's give-up used a copper bulb, so one dropped F pulse inverted its phase and jammed the siding for good (6 of 6 seeds, recovery only by hand); a set/reset latch replaces it.

#### Chunk alignment (a design may require it)

A design may require a part to sit inside one chunk, stated in the design file as a header line `chunk: <x> <y> <z> <X0>-<X1> <Z0>-<Z1>` (the reference block's design cell and the allowed world X & 15 and Z & 15 ranges, floorMod 16, so negative coordinates read right: the chunk is X >> 4), derived by the generator from the part's box, never hand-entered; it holds for the download's facing only, since rotated or mirrored builds aren't verified, and the build steps say so. The header's ranges are the part's box ranges narrowed to the offsets at which every chunk line of the placed design was tested. The partial-unload faults (stages 32 and 33) then run at every chunk line the design has when placed at that alignment, and each must pass. The carousel requires its stand area (stand, approach, circuit, pad; x −3..9, z −6..7 in the loop's frame) inside one chunk: X & 15 ∈ 3..6, Z & 15 ∈ 6..8. The assembly check verifies the placed footprint against the stated alignment: the stand area's blocks all fall inside one chunk at the stated offsets, and it prints each chunk line the placed farm crosses. Why: a boundary through the stand at level 32 splits a train (measured); aligning removes it at no block cost.

#### Reload order

After an unload, the game brings entities back in an order the design doesn't control: chunks load asynchronously, and within a chunk entities return in save order, which depends on their history. A design must not depend on that order. Every unload/reload fault runs under several orders: the save order, the reverse of the saved sequence, and at least 8 seeded orders (chunk order shuffled, rider histories seeded). Seeded orders shuffle both the chunk order and the order of top-level entities within each chunk (passengers stay with their vehicles), since within-chunk save order follows the section's insertion history, which the farm can't control. A case that fails under any sampled order fails. Each seeded order also re-rolls the brain state a reload resets, as the game can: sensor scan phases (drawn from the entity's own random), behavior timers (e.g., AcquirePoi's next scheduled start), and memories that aren't saved, which are dropped. The bridge validates the restore for a given order; the runners sample orders. Why: day-1's unload-mid-visit cases re-board in the bridged order but lose the napper in the runner's order, because a different rider history gives different entity ids; both are orders the real game can produce.

#### Settled as placed, and built in order (every part)

1. **Settled as placed.** The design file pasted raw, with no settle ticks and no shape or neighbour updates, passes the part's bench under the same criteria as the settled run. So every cell is written in its settled, self-consistent state (wire and rail shapes, repeater, comparator and powered states), and item stocks (a hopper's or dropper's stock, a sorter's fillers) are listed build steps, not pre-stocked file states. Why: the wired loop's release failed from slot 1 in the real game when its hopper's item stock was missing (the paste hadn't filled it); with the stock merged, the loop placed raw runs 60 slots at 0 differences against the 26.3 server. So item stocks are listed build steps, and a design must work from its file as placed.
2. **Built in order.** The file placed cell by cell in its listed build order, each placement running the updates the game runs (updateShape on its neighbours, neighborChanged, and the ticks that pass between placements), then the bench. This catches what goes wrong while a player builds: an observer firing on the cell placed in front of it, a piston extending before its stop is placed, rails reshaping as later rails go down, a half-built circuit pulsing. Every design lists its build order (the order the build-minutes bill already assumes), and a part whose build needs a temporary block or a "place this last" step says so in the list. A design that lists none is built in the default order: supports first, then rails, then redstone from its sources outward, with containers filled before power reaches them. The ticks between placements are the priced placement rate (prices.tsv's place rows: a player's real gap at the stage), and the check also sweeps the gap from 1 gt up to that rate and prints the first gap that fails, beside the tolerance line. Each cell gets its own priced gap (place 1 s = 20 gt, plus place:edge's extra 1 s middle / 0.4 s late where the bill charges it, so up to ~40 gt), and the sweep runs up to the longest priced gap, since a half-built circuit runs longer during those gaps. tick-exact checks the built-in-order mode against the real game for the wired loop.

#### No golem spots off the pad (every part that carries villagers who may have slept)

Every loop or wired part that carries villagers who may have slept passes the golem-spots check with 0 cells off the pad, on the file as published (the same file the page serves, after every wiring pass). The check:
- runs from every position a villager can occupy: the loop's rail cells, plus every position the sleep routine or a branch puts a villager on (the siding or branch to the bed, the dismount spot, the walk to the bed, the bed, the stand-up cell and the way back), taken from a recorded full bench night together with the design's listed positions;
- runs the game's summon search from each position (redsim's golem_spawn_cells_from) with entities ignored: a cell blocked only by a cart, a rider or the zombie at the moment of the check still counts as a spot, because they move.

Pad cells count as allowed only when seen from the stand cells; a pad cell reachable from any other villager position (the arriving or returning batch, the queue, the napper's path) is an out-of-turn summon onto the pad, and the report flags it (`queue-loop-spots --pad --stand`). A flagged pad cell doesn't fail the part by itself: the full assembly bench measures the out-of-turn summon rate onto the pad and its slot cost, printed beside the flag, and the geometry changes only if those summons cost slots.

Why: main's published wired v2 had 30 off-pad golem cells on its wiring, and on the day-1 lane's first wired night a golem spawned on one of them and jammed the tail. The loop benches missed it because their riders never sleep, so no golem was ever summoned away from the pad.

#### Release stagger (measured)

The release window has to cover the carts' stagger: release_for ≥ 2·stagger + ~26 gt, measured on the wired loop. The release controller's front window is widened to ≥ 2·stagger + 30 gt (day-1 lane).

### Archived: measured, superseded by vjeux's single lane

The 7-lane hold (stacked single carts, 100 of 100 seeds clean), the 7-state counter (v7 and v8, exact in the real game), the paired-lane geometry with its 7-lane junction yard (12,600 visits clean) and the scheduler core (0 lost slots over 120 days) stay as evidence with their benches and numbers. Their mechanisms carry over where the single lane needs them: the counter's fallback, the sensor budgets, the bed's release-and-reclaim, the BLOCK policy.

## The design (vjeux Nov 9 22:25): three hard rules (archived Nov 10: the 7-lane carousel below is superseded by the single lane above; rules 1-4 and the timing diagram stand)

vjeux 22:25: "The carousel is utterly terrible. I asked you to use the trick where we can only have
one bed. Also why did you make it a giant circle, this defeats the purpose of making it small. And
the villahers don't wake up at night... you didn't get any of the assignment". The rail carousel
(carousel-gate: three loops in a 43 x 43 x 31 box, a bed per villager, the station shut at night)
broke all three of his asks. It is the wrong design and leaves the site when the design below
lists. Every candidate meets all three rules; there is no first version without one of them.

1. **One bed for every villager.** All N = 21 villagers hold HOME = the same bed through the R1
   double claim (corrupt-poi-entity: a villager claims the bed, the bed is broken and placed
   again, the next villager claims the fresh record; N = 21 break-and-replace steps, the last one after the final claim, priced as hand steps at build time). The build's bed-sharing procedure ends with a break-and-replace after the last claim, so the bed starts with one free ticket. That spare is what lets a villager that lost the bed to another's sleep take it back, and it can be used only once. Every bench and integrated file starts with the bed's free tickets equal to what the build leaves (one) and prints it. The design includes the mechanism that keeps every claim, and the
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
     from its cart reaches the bed never arms it (bridge scene bed-cart); once armed, only a walk
     that reaches, lying down in the bed (never for a rider), or a reload clears it (the stamp
     rule in the self-recovery paragraph above).
   - **The occupied erase:** no other sharer in REST within 16 of the bed while it's occupied
     (ValidateNearbyPoi(HOME), in the REST package only, erases the HOME of any villager within 16
     of an occupied bed that isn't the sleeper). So a nap happens only when every other sharer is
     more than 16 + the stated margin from the bed, panic or not (the Shared bed radius must-have).
   - No second bed within any villager's POI search: a free bed would be claimed and break the
     sharing.
2. **Compact.** The target is the portal-pad station of "Golems removed by portal, and the
   smallest box" (about 24 x 12 x 5 at 99% summon success), with the queue and the bed inside a
   small box. The objective is the whole build's box volume, the queue and the bed included. No
   giant loops: short return paths or a compact loop are allowed when they measure smallest; the
   volume objective decides. Volume decides: the assembled overworld box's volume is the measure,
   with dig depth priced in build minutes, and horizontal over vertical as the tie-breaker (vjeux's
   rules, via the parent): at equal volume (with dig depth already priced in build minutes), the
   lower box, the one with the smaller y extent, wins; stacking is allowed whenever it gives the
   smaller volume, and the tie-breaker only decides ties. A layout change prints box extents, volume, dug cells, build minutes and
   blocks before and after. Villagers move in short hops: carts on short shuttles, water streams, pistons,
   whatever measures smallest. The lock box still holds (waiting groups more than 10.6 from every
   summoner on one horizontal axis, or 11.95 vertically, at every summon tick), and so does the
   occupied-erase distance (more than 16 + the stated margin from the bed during a nap, panic or
   not); the layout is the smallest arrangement
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
summon path closed at every position (panic only at the station, gossip closed by PANIC and the
static gate); golems
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
The once-a-night selection is part of the design: each villager naps once a night (vjeux: "wake
them up one by one once during the night. Not at every step"); a second nap the same night is only
the scheduler's rare escape (allowed by SleepInBed), so the judge accepts at least one nap per
villager-night and never fails a legal second nap, but prints second naps per villager-night as a
measured line, which the scheduler keeps near 0.

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
16 + the stated margin from the bed while it sleeps, panic or not. Dispatch is event-driven from each golem spawn, so the farm
phase-locks to the game clock, and a slot with no golem still advances the rotation on the next
grid tick (a fallback clock), so one failed summon never stalls it. Every interval in the diagram
is checked tick by tick in redsim and on the bridge, and G1 checks the diagram against the code.

### The parts: built, tested and optimized alone, then assembled (7 lanes, archived Nov 10: superseded by the single lane's parts at the top; the paired-lane geometry and the yard below stay as evidence)

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
   static check), and only the napper is calm. At every night tick, every villager except the
   napper is in PANIC, the batch in transit included. In PANIC at every night tick holds only
   while NEAREST_HOSTILE never clears, i.e. a zombie within 8 and in the villager's sight at every
   VillagerHostilesSensor scan; transit and hand-offs keep a zombie in sight throughout (a gap
   longer than the visible list's 20-gt window lets VillagerCalmDown hand the villager to REST).
   A villager that calms at night enters REST,
   and if its CANT_REACH (armed by day) is older than 1200 gt it releases the shared bed at its
   first REST tick; with one ticket, only one releaser can re-take it, so a second releaser loses
   HOME for good. The static gate covers every position a villager
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
   minutes. Geometry (ruled Nov 10, 4:14 AM, volume decides): paired lanes at x 0,1 | 3,4 | 6,7 |
   9 with wiring columns at x −1, 2, 5, 8 and the pair's hold ends staggered 1 in z, so every
   hold, hump, stand and yard control has a side neighbour for a direct ≤ 2 gt feed; the 7-wide
   pitch-1 stack can't be wired (the middle level's controls have no free neighbour). Box ~7,700
   for release + hold + sleep (+27% over the unwirable 6,050). A later optimization pass looks for
   a cheaper wirable geometry (staggered or offset levels), measured against 7,700.
3. **The sleep routine.** In: the night window (a daylight-sensor signal) and the counter's
   state, so it never takes a villager whose batch is due. Out: each of the 21 villagers once a
   night, one at a time, within 2.0 of the one bed and not riding; it sleeps, the observer on
   OCCUPIED flashes the dorm zombie, it wakes, stands up at the one free cell and returns to its
   hold slot before its next turn. Sequence: out of every zombie's sight, the villager calms within
   one sensor scan pair (up to 39 gt), is within 2.0 of the bed and not riding, SleepInBed starts
   (LAST_SLEPT set at start: a one-tick nap counts), the observer on OCCUPIED opens a hatch on a
   dorm zombie, it panics and wakes within up to 39 gt (measured 18), stands up at the one free cell,
   and is taken back; SleepInBed can't restart until 40 gt after a wake (nextOkStartTime) and 100
   gt after LAST_WOKEN. Every other villager stays more than 16 + the stated margin from the bed,
   panic or not (the Shared bed radius must-have), so the occupied erase
   can't hit anyone else. The napper's release at its first REST tick is expected; AcquirePoi(HOME)
   re-takes the bed 20-39 gt after it is unoccupied (measured 28), from the cart too, provided a
   path with reach 1 succeeds and no other bed with a free ticket lies within 48 (a failed path
   backs off 40-79 gt, growing to 400). Invariants: every villager's HOME = the bed at every dawn and
   midday (the R1 claims kept); while the bed is occupied, every other villager is more than 16 + the
   stated margin from it, panic or not; each villager's LAST_SLEPT advances at least once a night. Contract: each visit fits
   inside that villager's wait; the 21 visits fit inside the night. The golem condition is
   gameTime − LAST_SLEPT < 24,000 (strict), so for each villager no gap window [its last nap +
   24,000, its next nap) may contain one of its batch's slots; that window condition is the
   contract. Each villager naps once a night (vjeux's target); a second nap the same night is only
   the scheduler's rare escape (SleepInBed permits it 100 gt after LAST_WOKEN and 40 gt after its
   nextOkStartTime): the judge accepts it and prints second naps per villager-night. Because 240
   mod 7 = 2, a batch's slots move 200 gt earlier in day time each day, so the scheduler's rule is:
   each nap at most the drift earlier than the night before, or later only when the window it opens
   is slot-free (e.g. a second nap at the later time that same night). Part 3a's bench runs the
   scheduler alone for at least 60 in-game days and prints each villager's nap day-time range and
   worst gap window. Bench: 21 villagers with R1
   claims, 6 days on 100 seeds, the station and counter driven ideal, HOME, LAST_SLEPT and
   distances logged every tick. It waits on tick-exact's bed pieces (ValidateNearbyPoi's reach,
   SleepInBed's conditions, the wake delay, findStandUpPosition). Objectives: blocks, volume, the
   visit's length and its margin, build minutes.
   3a. **The nightly scheduler** (day-1 lane): in, the night window and the counter's advance and
   lines; out, one dispatch at a time naming (b, k), each villager napping once a night (vjeux: "wake
   them up one by one once during the night. Not at every step"; a second nap the same night only as
   the scheduler's rare escape, accepted by the judge and printed per villager-night as a measured
   line kept near 0), spaced for the
   visit length, each visit ending before its batch's next release.
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
  search from there lands only on unspawnable cells or the pad. Off-pad golem spots are a hard 0.
  Pad cells reachable from a non-stand villager position are a measured line, not a hard fail: the
  full assembly bench prints the out-of-turn summon rate onto the pad and its slot cost, including
  the lock spill (an out-of-turn summoner locks every villager in its 10.6 box for 600 gt,
  neighbouring batches included, so one summon can cost several turns). The out-of-turn rate and
  its cost are measured both on the healthy bench and under the fault-injection runs, since a
  timing fault is what would put an approach or departure rider in view at a summon tick. The
  pad's geometry changes only if that cost is non-zero. The panic path cannot fire when the
  villager is more than 8 from the farm's zombie or out of its sight except at the station, and no
  natural hostile can stand in its sight on a dark surface (the parent brief's every-villager
  rule); the gossip path (`InteractWith`, `TradeWithVillager`, `Villager.gossip`,
  `spawnGolemIfNeeded(5)`, off the 100 gt grid: a calm pair within the square root of 5 that see
  each other within 8, both 1,200 gt cooldowns over) is closed by PANIC (no TradeWithVillager
  while panicking: every held villager panics at every tick, day and night) and by the static gate
  (spawnGolemIfNeeded from gossip finds no valid floor from any held position); held batches sit
  1-3 blocks apart on paired lanes; the bed stop is split so that no 5 villagers share a 10-box in
  sight (at dawn all of them are slept, unlocked and calm in IDLE).
- The golem leaves the station's cubes within a few ticks (a drop below the cubes, or a kill that
  fast), and dies in the kill as the parent brief says (no golem outlives 1,200 gt).
- Every villager lies down once per game day (the nightly stop: dismount, a bed, woken by a
  hostile in sight, re-boarded by a cart moving at least 0.1 blocks a tick), the Beds rule holds
  (a shared dormitory keeps its claims through the nightly HOME release of iteration 14, judged
  over many nights, not only through a save and load), and the farm restarts after days away (a
  stalled advance recovers: a fallback pulse when no golem has come for a full cycle).
- **Shared bed radius.** While a napper sleeps, the shared bed is OCCUPIED, and any other villager
  in REST within 16 blocks of it (Euclidean, from the center of the bed's head block, its POI
  position, which HOME holds, to the villager's position) has its HOME erased (ValidateNearbyPoi). Because the occupied POI isn't released, it can
  never re-claim the bed, so its naps stop for good. The rule has two parts:
  - **Distance:** every position a villager can occupy on the loop, the queue, the tail, the stand
    and the pad lies more than 16 + the stated margin (≥ 1, for rider position spread) from the
    centre of the bed's head block, panic or not. The assembly prints the minimum distance and
    where it occurs.
  - **Time:** in the siding's own cells and on its junction rails (where a napper must be, within 16
    of the bed, to use it), no non-sleeping villager is present while another villager is asleep in
    the bed, faults included. The time part covers siding positions within 16 blocks plus the
    margin of the bed; siding positions farther out fall under the distance part instead. The named case: a napper still asleep when the next visit arrives
    (tick-exact's asleep700 late wake). Either the scheduler doesn't divert while the bed is
    occupied, or the design shows that the arriving napper keeps HOME.

  Panic is not a protection: a reload drops the hostile memory, and riders stay calm until
  their sensor re-scans. Why: a stacked placement put the bed about 12 from the stand's riders; the
  current side-by-side layout has only about 18.
- **No derails.** A cart off the rails (no rail in its cell or the one below for 3 ticks) is a hard
  fail anywhere in the loop. An intended flight declared in the design (take-off, landing rail,
  maximum airtime) is not a derail if the cart lands on the declared rail within the bound with its
  rider seated; every other off-rail stretch of 3 ticks or more is. A landing on another cart, on a cell other than the
  declared landing, or not moving forward is a derail too. The design file states its flights as a
  header line (`flights:`); the one check is `mobfarm::flight`, which every runner calls.
- **No merged carts.** Merged carts (two carts within 0.01 of each other, which the game's cart push
  never separates) are a hard fail anywhere in the loop; every runner checks for them. "Within
  0.01" is the horizontal distance between two carts whose boxes the push would pair (they overlap
  once inflated 0.2 sideways, as the minecart's own collector OldMinecartBehavior.pushAndPickupEntities collects them; AbstractMinecart.push skips a
  pair whose squared horizontal distance is under 1.0E-4), so carts on stacked tracks a block apart
  are not merged. The one check is `mobfarm::merge` (with its test), which every runner calls. Why:
  a cart sliding into a resting one on a slope gets pressed to the identical spot, and a merged pair
  later swaps order (measured on the tail's 10-slope queue, 2026-11-10).
- No escapes (villagers, golems, the zombie), the hostile and sky rules of the parent brief.
- **Compact, measured** (rule 2 made measurable). The assembly prints the assembled overworld
  bounding box (x × y × z extents and its volume) and each part's own box, next to the block count;
  minimizing the assembled box is rule 2's objective. A part placed outside the loop's footprint
  (beside it, or above or below it) states why it can't fold in, and no part is placed just where
  space happened to be free. Any change that grows the box states by how much and why. These
  reasons live in the part's NOTES and the assembly report, not on the site page. The Nether side
  follows its own portal-ticket rule. Why: the controller's parts were spread over 107 × 82 blocks,
  and vjeux rejected the first carousel as "a giant circle".

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
