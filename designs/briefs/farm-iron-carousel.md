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

A band is every gt step between its limits. Each timed control's band is swept at every step on its own, and the corner runs come on top. A failure at any step inside the claimed band fails it, and a table that runs only the ends doesn't establish a band.

Fault rows run on at least 8 seeds and print clean/total. A row whose traces are identical across those seeds (identical in every logged field, brain memories and sensor timers included, not cart positions alone) counts as seed-independent. Otherwise it passes only with at least 40 seeds and 0 failures, printing the rate and its 95% Wilson interval (as the shared runner prints it; 0 of 40 gives 0–8.8%).

A bench that drives a part from emulated neighbours is validated first: every signal it emulates matches the wired neighbour's, each by its own trace-diff with a positive control, and so do the cart traces at each interface, at nominal timing and under each perturbation the bench is used for (at least each swept band's ends, every corner run and every fault row), with the wired neighbour run under the same perturbation. Every run prints carts in and out per release, flagging any extra or missing cart as a bench fault, and names any emulated signal, and any perturbation, not yet validated. A failure found on a bench is classified as the part's or the bench's, by trace, before it drives a design decision. Why: the stand's stagger-5 failure, first read as the stand's own gate timing, came from the bench. Its release slopes all shut together at +34, which stranded a late cart on a braked slope; the next advance moved it on and it ran into the stand at the next release. The wired controller keeps each slope open 40 gt from its own opening. After the release group and register were matched, the stand's own release signal was still 3 gt early on the bench.

A detector rail turns on when a cart reaches it and rechecks every 20 gt, staying on while any cart's box meets its search box (the cell inset by 0.2 on each side, up to 0.8 high; a 0.98-wide cart counts once its centre is within about 0.79 of the cell's centre), so carts that reach it less than about 20 gt apart give one on-edge between them. An on-edge is evidence only for the first cart to meet that box after the detector was last off. An element may treat an edge as evidence for a particular cart only if that cart is the first to meet that box after the detector is shown to be off, with the margin printed per window. The tests print, per detector, its on-edges against the carts that crossed it.

No detector may sit where a cart crossing an adjacent junction can reach its search box. A stand never lets more than 3 carts out per release window, faults included. Why: a comb stand's arrival detector fired on carts snapping through the junction beside it; and when the track after a stand overflows, the boosted climb sends the extra cart back into the stand at full speed.

Every declared jump publishes its gap bands, bridged against the game in steps of a quarter tick or finer: the follower gaps at its take-off that land clean and those that don't, empty and ridden, at each stored speed it can see. The parts upstream deliver only clean gaps, faults included, and their rows print each arrival's gap at the take-off. Why: on R1's drop, followers 4 to 6.75 gt behind stuck on the take-off's lip or the landing's support, or landed overlapping the cart ahead, while 3.75 gt or less and 7 gt or more landed clean (bridged, 0 differences); from 8 gt the follower lands as a lone cart. The earlier rule of at least 5 gt sat inside the bad band.

#### Every stateful element recovers by itself (fault injection)

Every stateful element in a part (latch, copper bulb, counter, register, ring) recovers by itself from one missed pulse and from one extra pulse. The farm returns to clean operation within a stated number of cycles, with no permanent phase inversion and no hand reset. It's checked by fault injection on the part's bench: at each stateful element, drop one input pulse, and separately add one extra input pulse, in each distinct state the element can be in when the pulse is due, over several seeds. The part's line reports, per element, the worst-case cycles to recover and what was lost in the meantime (slots, naps, golems). An element that never recovers fails the part. The injected faults also include an unload and reload at each state (the player leaving and coming back mid-cycle, mid-pulse included), since that's how an AFK farm actually drops or doubles a pulse; the element's state must survive it or recover within the stated cycles. The unload/reload uses the shared act (`unload <chunks> <N>`; `queue-loop-run --act SLOT,DT,unload,N`) at each state, mid-cycle and mid-pulse, with N = 0, N = a few ticks, and N longer than the element's longest timer, and both the whole part and a partial unload with a chunk boundary through the part, in each of its three stages: (a) block-ticking, entities frozen (level 32: repeaters, observers, comparators, detector rails, block events and hoppers keep running; random ticks stop; carts and mobs freeze in place and resume with their velocity; so a release can fire on a frozen queue), (b) loaded, nothing ticking (33: scheduled ticks keep absolute times, and overdue ones fire at once when ticking resumes), (c) unloaded (34+: saved and dropped, ticks resumed at their remaining delays). The partial act `act T unload x0,z0,x1,z1 N` produces (a) and (b); the whole-part `act T unload all N` covers (c). Every stateful element's fault injection covers stage (a) at each state. Freezes are tested short and long: besides the short freezes, every phase is also run with each chunk the farm crosses frozen for 1,500 gt and for 5,000 gt. Game time keeps running in a frozen chunk, so timers that villagers keep in game time, such as the 1,200-gt release of an unreachable bed, jump by the freeze length. After any freeze, every villager must still hold the shared bed or regain it on its own before its next nap, with the naps and golems lost counted. A villager that loses the bed for good is a hard fail. A bed gives back only one claim, so if two villagers have lost it, only one can take it back. At no tick of the night may two villagers be resting calm with the bed out of reach, faults included, because a long freeze at that tick would cost both the bed. The tests print the most such villagers at any one tick, which must be 1 or less, and include two long freezes in one night at the worst moments the design has. A villager's unreachable-walk stamp is set when one of its walks can't reach. It is cleared by a walk that can reach (or reaching its target), by lying down (SleepInBed.start: on foot within 2.0 of the bed, the bed free, not woken in the last 100 gt; never a rider), or by a reload. SleepInBed.start erases WALK_TARGET and the stamp even when startSleeping fails, and sets LAST_SLEPT only when it succeeds. Panic by itself doesn't clear the stamp; a walk that reaches does, flee walks included. The first time the villager rests calm with no walk target while that stamp is over 1,200 gt old, it gives up the bed, freeze or not, and the stamp restarts, so the loss repeats every 1,200 gt while its walks keep failing. So the tests also print every tick on which a villager rests calm holding the bed with the stamp over 1,000 gt old, whatever its walk target. Outside the napper beside a free bed there must be none, and no villager may give up the bed in normal running. What clears riders' stamps. A stamp clears whenever the rider gets a walk target its path can reach, or is already at it. Under the 6-to-8 rule no rider flees, so flee walks no longer clear anything. Each design names, place by place, what clears its riders' stamps instead: the panic stroll's reachable cells (within 2 and 2 of a panicking rider, on a stretch with standable floor beside the track, with a zombie at 6.05 to 7.95 and in sight, declared with `panic:`), or a place where no behaviour can pick an unreachable target, so no stamp is ever armed. The panic stroll isn't always uniform. Within one section of an occupied village POI's section, or with none within 8 sections, it picks uniformly on land; from 2 to 8 sections away it picks toward the nearest lower-level section, within a half-plane and without snapping to land. An occupied village POI is a bed with its ticket taken, or a claimed job site or bell. The shared bed is one only while its ticket is taken, so each design's named clearing cells hold in both bed states, ticket free and taken. No other occupied village POI may sit within 8 sections (Chebyshev, in section coordinates, all three axes) of any position a villager can occupy (riders, and the napper on foot); 144 blocks or more along any one axis guarantees it. That covers a breeder's beds and job sites and any village, and the build guide and site guide say so. A static check fails the farm file if any bed other than the shared one, any job-site block or any bell lies within 8 sections of a villager position, with a seeded bed 7 sections out as its positive control. Why: bridged (scenes stroll-village and stroll-village-taken, 1,200 ticks each, 0 differences): riders panicking 2 sections from the bed picked only toward it once its ticket was taken (101 of 101 and 99 of 99 picks), and evenly while it was free. Main's figures (flee walks doing 90% of the clears, oldest stamp 643 gt) predate this rule, which main fails at 78 of 78 places. The first design that meets it measures the line again: the tests print each rider's oldest stamp at any tick in normal running, which must stay at 1,000 gt or less. The tests also print, over the whole loop and every tick, every push a rider makes on a cart other than its own, walking or not, counted at the push itself, with the number of carts within 0.2 of a rider's box, the most at once and the cells. A rider pushes, during its own tick, any cart whose box its own uninflated bounding box intersects (LivingEntity.pushEntities through AbstractMinecart.push, skipped when the centres are under 0.01 apart). A walk only moves the rider's box within its tick, so it can make or break contact, and a contact can begin and end inside one tick, which a count from end-of-tick positions misses. The count must be 0, or the part's rows must show the push harmless, with the push bridged against the game in the part's own geometry: no holes, no merges, the order kept, every timing within its slack, the edge counts unchanged, and no cart moved back past a gate, a detector or a declared rest cell, across every fault row and reload order. In escG6L-v3's pressed queue (bridged, 500 ticks) a rider pushed the cart behind it up to 0.36 blocks back up the slope, again every 9 to 22 gt until the queue advanced. Carts push each other across neighbouring tracks too. A cart pushes any other cart whose box meets its own box inflated 0.2 sideways (not vertically), and the push acts only when the line between their centres runs within about 37° of either cart's yaw (|cos| ≥ 0.8); on the next tick each cart's rail turns the push into motion along it. Two carts passing on parallel rails 1 block apart never qualify while both face along their rails. But a cart's yaw follows its motion only while it moves more than 0.0316 blocks a tick, so a cart placed or summoned at rest (yaw 0, facing +x), or one that stopped right after a curve, can keep a yaw across its rail, and a cart passing on the next track then pushes it along its rail. So at every tick where a cart at rest or creeping has its box within a passing cart's inflated box (centres within 1.18 blocks on both horizontal axes, and box heights overlapping, |Δy| < 0.7, slopes included), the line between their centres stays more than 37° off both carts' yaws, or the part shows the push harmless for both carts, the passer's slowdown included, bridged in the part's own geometry. Facing along its own rail gives that only beside a parallel track: beside a junction's diverging leg, a spur, the end of a T or a curve, the line between centres can run along the resting cart's own rail, and a passer close to that rail's line then pushes it on every pass (26.3 read). The tests count every push between carts on different tracks, moving or not, and the count must be 0, or each push shown harmless in the part's rows, as for the rider-push count. Why: bridged (scene side-push, 200 ticks, 0 differences): carts passing in opposite directions 1 block apart at 0.1 to 0.4 a tick never pushed each other; a cart at rest with yaw 0 across its rail was pushed along the passer's direction (+0.064 to +0.205 a tick), as was one creeping at 0.03, and the same resting cart turned along its rail was not; 2 blocks apart, or one level apart, nothing. A cart a player places, or one summoned without a rotation, starts at yaw 0 whichever way its rail runs (MinecartItem through createMinecart; nothing reads the player's facing), so the assembly runs the build guide's start state as its own row, every cart at its placed yaw, and shows no harmful push before each cart's first move. The ring at level 32 is also where the simulation distance ends, so it can be a steady state, not only a transient: a farm straddling it from the player's AFK spot would run its redstone with frozen carts for good. The brief states the AFK spot. Overworld parts: every block, cart path and villager position lies in entity-ticking chunks (level ≤ 31) from the stated AFK spot. The Nether side stays within the portal tickets' area (entities within 1 chunk of the arrival chunk, blocks within 2, renewed by crossings). Entity ticking covers every chunk within Chebyshev distance min(N, V) of the AFK spot's chunk (N = simulation distance, V = view distance; the player's ticket is at level 31 − N, rising by 1 per chunk step over all 8 neighbours). Ring N + 1 is level 32 and ring N + 2 is level 33. The farm is designed for N = V = 10, the dedicated-server defaults (the smaller; singleplayer defaults to 12). Every overworld chunk of the farm lies within Chebyshev distance 10 of the AFK spot's chunk, a 21 × 21-chunk square. The assembly prints the footprint's minimum, max(|Δcx|, |Δcz|) over its overworld chunks, and its margin to 10. The AFK spot is stated as a block position, and the brief picks it at least a few blocks inside its chunk (away from all four chunk edges), so a player shifting a step doesn't move the centre chunk and the 21 × 21 square with it. The assembly prints the footprint's margin for every chunk the player could stand in within that tolerance. Why: the bed siding's give-up used a copper bulb, so one dropped F pulse inverted its phase and jammed the siding for good (6 of 6 seeds, recovery only by hand); a set/reset latch replaces it.

Under a partial freeze, redstone keeps running while entities stand still, so a redstone delay can fire before the entity it was timed for has moved, or before that entity's own timers (a boarding cooldown, waking) have run. Every timed action is therefore checked at every freeze phase. It may stand only where the part then recovers on its own within its stated cycles, with no damage and nothing needing a reset by hand; otherwise the action waits on evidence of the entity's state or retries until it happens. A switch never flips while a cart is in its cell. Why: during a freeze, the siding's kick, timed in redstone, overtook the woken napper's boarding cooldown, which counts in entity time; the cart rolled into the standing napper and stalled there for good.

Fault rows include a riderless cart, one whose villager was left at the bed stop, killed, converted or dismounted, entering every part at every phase of its cycle. A riderless cart may cost its batch's golem, but it must never cause a merge, derail, back-crossing or deadlock, or meet another cart where the no-meetings rule forbids it, and the loop keeps running correctly with it. A riderless cart slows faster than a ridden one (0.96 a tick against 0.997), so the rows include every climb, gate approach and flat where it could stop short. A cart at rest on an unpowered powered rail never moves by gravity, and a cart crossing one halts on it, ridden or empty; so no unpowered powered rail may sit on any path where a design relies on a cart rolling from rest or rolling back. Why: bridged (scenes rollback-halt and rollback-booster, 0 differences); the halt ignores passengers (26.3 read). A sensor that sees only riders (a tripwire above the rail) needs its own rows showing that a riderless cart passing it is handled safely. Where the fault leaves its villager alive, the same rows track that villager: it is back in a cart within a stated number of cycles, it is never within the shared-bed radius while another villager sleeps, and the golems and naps lost until it is back are counted. A villager that is never re-boarded fails the part, like an element that never recovers. Why: main's bed stop leaves its napper behind in 6 of 40 reloads mid-nap. The empty cart rejoins the loop, where tripwire evidence can't see it, and the villager left awake beside the bed loses its claim for good when the next napper lies down.

One bed, one ticket. The shared bed has a single ticket, and each rider keeps HOME on it as memory. A rider whose HOME is lost re-claims only if the ticket is free when it tries. A loss by release while the ticket is held returns the ticket, so it strands no one: the rider re-claims at its next siding visit, or earlier, anywhere on the loop where the ticket is free and a complete path whose walked length stays under 48 ends within 1 block (Manhattan) of the bed. A failed try puts the bed in that villager's retry back-off (40 to 80 gt, growing to 400), so the re-claim rows include a homeless rider that reaches the siding with its back-off armed, and the part's stated re-claim cycles cover that case. The rows print where each re-claim happens. A loss that frees nothing (an erasure, such as ValidateNearbyPoi beside an occupied bed or any other erase without releasePoi, or a release that finds the ticket already free, such as a second release before anyone re-claims) adds a homeless rider without adding a ticket. The start state's free ticket absorbs exactly one such loss; every later one leaves a rider homeless for good, and it never sleeps again. So no part may lose HOME in a way that frees nothing. Every run prints, per cycle, the bed's free tickets and the riders without HOME, faults and reloads included, and a rider still without HOME after the re-claim cycles its part states fails. Why: bridged in the game (scene ticket-release, 600 ticks, 0 differences, free tickets compared every tick): a release frees the ticket whoever holds it, and an erasure frees nothing. In the integrated file, a forget after the spare was taken left e29 homeless for good, and two releases a tick apart lost one increment and left e5 homeless.

A rider's walk targets can come from any of its behaviours: a panicking rider's flee (a standable cell up to 16 blocks horizontally and 7 vertically away from the hostile it fears) and its short stroll (within 2 and 2); a calm rider's stroll (within 10 and 7), its walk to another villager within 8 or toward what it is looking at, its walk to an item it wants (food or seeds) lying within 4, and, when resting, its walk to its bed; walks to any job-site block or bell within 48 blocks; and walks toward a job site or meeting point it remembers, however far away. The job-site blocks are the composter, barrel, lectern, smoker, blast furnace, grindstone, loom, stonecutter, cartography table, fletching table, brewing stand, smithing table and every cauldron (empty, water, lava or powder snow). Whenever a chosen cell has no complete path from the rider, its can't-reach count starts ageing, and once the count passes 1,200 gt the rider gives the bed up at its next calm REST tick with no walk target. So no job-site block or bell may sit within 48 blocks of any place a rider can be, and no item a villager wants within 4. Every villager enters the farm with no remembered job site, potential job site or meeting point. A villager is either bred before the bed's setup (a newborn claims a free bed within 48 blocks), or bred in a breeder whose beds and job sites all lie at least 144 blocks, along some axis, from every position a villager can occupy, or brought in only after its old workstation was broken while it stood within 16 blocks of it and after it spent a whole meeting time (day time 9000 to 11000) calm within 16 blocks of its broken bell, so the game forgets both. Its trip in stays more than 48 blocks from every job-site block and bell. The build guide says how and when. A bred or moved villager also arrives with HOME on its old bed, and ValidateNearbyPoi(HOME) runs only in REST; so the R1 step breaks that bed while the villager rests calm within 16 blocks of it at night, before it claims the shared bed, and its trip in stays more than 48 blocks from any free bed. The 48-block clearance holds at each break as well as on the trip in: AcquirePoi re-claims, within 20 to 40 gt, whatever is free within 48 blocks once a memory is empty (a bed for HOME, a matching job site, a bell for the meeting point). So before each break, the village's other free beds, workstations and bells within 48 blocks are broken or claimed, or the villager is carried more than 48 blocks away right after the erase. Every run prints each villager's remembered places at the start, and three rows bring in a villager still remembering something: one a job site, one a meeting point, one an old HOME; each must show as a failure. The start print fails the run on any remembered job site, potential job site or meeting point, and on any HOME other than the shared bed, whatever its distance. Bridged (scene jobsite-memory, 24,000 ticks, 0 differences): a rider brought in remembering a job site or a bell more than 100 blocks away (Manhattan) forgets it at its first WORK or MEET tick, and that sets its can't-reach stamp to that tick, which then ages like any other. A nearer one can last all day (scene jobsite-near, 12,000 ticks, 0 differences): within 9 blocks (Manhattan) of a job site or 6 (Manhattan) of a bell the walk counts as arrived and no stamp arms; farther out, a failed path arms the stamp and any walk that reaches clears it, so the memory goes only after 1,200 gt (job site) or 200 gt (bell) with no walk reaching in between. A rider also claims a free bell within 48 blocks as its meeting point while riding, whenever a cell its path can reach lies within 6 blocks (Manhattan) of the bell. Walls around a bell stop the claim only if they keep every reachable cell farther away than that (scene jobsite-near: riders 36 to 42 blocks away, Manhattan, claimed a bell whose nearest reachable cells were 2 away). At every place a rider can panic, the flee's candidate cells must be none or all reachable, counting every cell the flee can pick from any position the rider can occupy there, its cart's motion included. The other behaviours' unreachable candidates are listed per place for design; what decides them is every normal-running run's oldest can't-reach count, against the 1,000-gt limit, in the built surroundings. A cell counts as reachable only through a complete path whose walked length stays under 48 blocks, found before the path finder's search stops (a villager's stops after 767 nodes). A walk target's path must end on the target cell itself (reach 0), after the game moves a ground mob's target to the surface: a target in the air drops to the cell above the first non-air block below it, and a solid target rises to the first non-solid cell above it, so a walled solid block is targeted on top of its cap. The target's close-enough distance (Manhattan: 1 for HOME, 9 for a job site, 6 for a bell) only decides whether the villager has already arrived, which clears the stamp with no path at all. A POI claim's path counts as reached once it ends within the POI type's valid range (Manhattan: 1 for a bed or a job site, 6 for a bell), with no surface move. 48 is every villager's required path length: the Villager constructor sets it, above any follow range. The static check uses the same limits, and never a close-enough distance as a path's reach. The follow range (14.16 to 17.84 blocks, set at spawn, or exactly 16 for a villager summoned with NBT) still sizes the villager's entity-sensor box, so rows run villagers at both ends of that range where the sensor box matters, and the golem lock box keeps 17.84. The z −5 stone and the siding exit slot's (4,2,−2) are positive controls. The HOME rows (oldest count, HOME changes) count only from runs in the farm's built surroundings, its dug cavity and ground, not from a void bench. Why: bridged in the game (0 differences). One stone in the flee range of riders panicking at a zombie 3 blocks away became a flee target no rider could reach, and HOME changed 26 times in 8 seeds against 4 without it; those 4 were two losses, each re-claimed, from calm strolls on the siding's exit slot that picked its unreachable tops.

Zombies 6 to 8 blocks away. A villager flees only from a hostile closer than 6 blocks, and panics only at one within 8 that it can see (both 3D, from the zombie's position to the villager's). So every zombie stays at least 6.05 blocks from every position a villager can occupy: riders moving, at rest and at fault positions, and the napper on foot (its sleeping position, the cell where it stands up, and its walk to and from the cart), measured against every position the zombie can take in its post. The flee then has no candidates anywhere, which meets the verdict above. A zombie that wakes the napper sits 6.05 to 7.95 from the sleeping position and from the cell where the napper stands up, in sight from both once the wake is due and until the napper boards, and out of its sight before the wake. A napper that calmed after waking would be back in REST at night and could lie down again once 100 gt had passed since it woke, away from the boarding cell; the stand-up cell's `panic:` entry keeps it panicking until it boards. From boarding on, the napper counts as a rider, so its cart at the boarding cell and onward falls under the night entry, and no tick is left uncovered. Wherever a part relies on panic, it declares the place (`panic:` in its file): at least the stand's pockets at the summoning moment, the napper's stand-up cell from the wake until it boards, and every position a villager other than the napper can occupy at night, where the night rule keeps it panicking at every tick. There a zombie stays within 7.95 and in sight. `mctas mobfarm flee-check` prints each place's least and greatest hostile distance and the worst pair, and fails a place inside 6.05, or a declared place beyond 7.95 or out of sight. Panic's own short stroll (within 2 and 2) still runs at every panic distance. Why: bridged (scene flee-distance, 600 ticks, 0 differences): riders and standing villagers with a zombie in sight at 5.80 and 5.95 got flee targets; at 6.05 to 7.95 they panicked with none; at 8.05, or at 7.0 behind a wall, they didn't panic (VillagerHostilesSensor takes a hostile within 8 and in sight; SetWalkTargetAwayFrom flees only when closer than 6). With real ground around the farm, every design before this rule failed the flee verdict, the farm on main included (its wired siding loop at 78 of 78 places). A villager whose eye is inside a block's collision shape (glass, a closed fence gate's box, any collider) sees no hostile at any distance, so such a position fails the sight requirement whatever the zombie layout; flee-check lists these positions per place. A blind villager at night is calm and in REST, and within 16 blocks of the shared bed (from the centre of its head block, where its POI sits) while another villager sleeps in it, ValidateNearbyPoi erases its HOME and no ticket is freed (it costs the spare): a loss that frees nothing. Why: bridged (scene rider-eye-block, 600 ticks, 0 differences): riders with the eye inside glass or a closed gate's box never panicked at a zombie 7 blocks away in plain view; one cell over, with the eye in air, they panicked at the first scan. ValidateNearbyPoi runs in REST for riders too, and releases the ticket only when the bed's occupant isn't a sleeping villager (tick-exact's 26.3 read). Blindness is the eye point inside a collision shape, or the sight ray from it crossing one; the ±0.24 eye box that `isInWall` tests is the suffocation test and plays no part in sight, so every blind count uses the point. Why: bridged (scene rider-eye-graze, 200 ticks, 0 differences): seated riders whose eye box overlapped a closed gate or a glass block, with the eye point outside by 0.015 to 0.20, all panicked at a zombie 7 blocks away; with the point inside, none did.

An empty cart moving at 0.1 a tick or faster takes the first eligible entity whose box overlaps its own box inflated 0.2 sideways: any mob or boat that isn't a player, an iron golem or a cart and isn't itself riding something (the first in the level's list order boards; the rest are pushed). Blocks aren't checked, so only where an entity's box can be decides it: a mob pressed against a pane across the track is taken. That is what re-boards a left-behind villager, and the riderless rows rely on it. It would also take a zombie, and a boat with a zombie in it boards whole, so no zombie and no boat may ever have its box within that reach of any place an empty cart passes at that speed, faults included. The napper's own empty cart counts in normal running. A static check proves it for every boat and free mob against every track an empty cart can reach, because redsim doesn't model a boat riding a cart. Why: a zombie riding into the loop would reach every villager; tick-exact bridged the pickup in 26.3, a zombie's boat boarding an empty cart included.

#### Chunk alignment (a design may require it)

A design may require a part to sit inside one chunk, stated in the design file as a header line `chunk: <x> <y> <z> <X0>-<X1> <Z0>-<Z1>` (the reference block's design cell and the allowed world X & 15 and Z & 15 ranges, floorMod 16, so negative coordinates read right: the chunk is X >> 4), derived by the generator from the part's box, never hand-entered; it holds for the download's facing only, since rotated or mirrored builds aren't verified, and the build steps say so. The header's ranges are the part's box ranges narrowed to the offsets at which every chunk line of the placed design was tested. The partial-unload faults (stages 32 and 33) then run at every chunk line the design has when placed at that alignment, and each must pass. The carousel requires its stand area (stand, approach, circuit, pad; x −3..9, z −6..7 in the loop's frame) inside one chunk, and the farm is placed with X & 15 = 6 and Z & 15 = 6, so its chunk lines fall at design x = −6 + 16k and z = −6 + 16k (the stand area sits between the lines at x −6 and x 10, and z −6 and z 10). The assembly check verifies the placed footprint against the stated alignment: the stand area's blocks all fall inside one chunk at the stated offsets, and it prints each chunk line the placed farm crosses. Every chunk border of the placed farm crosses track only at a part's declared allowed cells, and the assembly prints every crossing with its class. Each part's generator writes its border cells into a header line (`border:`), listing the allowed cells and, separately, the forbidden cells whose own freeze rows passed. The assembly prints each crossing as allowed, forbidden-passed or forbidden-failed. A forbidden-failed crossing fails the assembly. A forbidden-passed crossing rests on its own part's freeze rows at that cell, so a placed border that runs through forbidden-passed cells of two or more parts has its freeze rows rerun on the assembly, and fails if they fail. At every chunk border the cart path crosses, each side must stay safe while the other side's chunk is frozen. Upstream of the border, the track holds, at rest and with no strand, derail or merge, every cart that can still reach it once the downstream chunk freezes: at least one released batch, since a release that fires just before a freeze can't be stopped. Downstream, the track drains or holds without anything from the frozen side. A forbidden crossing becomes forbidden-passed only when it meets this and its freeze rows pass. A cart ticks by the chunk its own position is in, and its rider with it, so the first cart over the line freezes whole just past it, and carts pressing against it still rewrite its stored velocity. So the border rows check the resume when the freeze ends, not just the hold. Why: a boundary through the stand at level 32 splits a train (measured); aligning removes it at no block cost. Wherever a gate can shut on a cart, every position the cart can hold inside the gate's box keeps its whole box inside one chunk. Why: bridged (scene gate-frozen, 200 ticks, 0 differences): a cart frozen in a gate that shuts, and opens and shuts again, moves on through it at the thaw, ridden or empty, unless its box touches a chunk that is still unloaded; then it stays inside the shut gate's cell.

#### Reload order

After an unload, the game brings entities back in an order the design doesn't control: chunks load asynchronously, and within a chunk entities return in save order, which depends on their history. A design must not depend on that order. Every unload/reload fault runs under several orders: the save order, the reverse of the saved sequence, and at least 8 seeded orders (chunk order shuffled, rider histories seeded). Seeded orders shuffle both the chunk order and the order of top-level entities within each chunk (passengers stay with their vehicles), since within-chunk save order follows the section's insertion history, which the farm can't control. A case that fails under any sampled order fails. Each seeded order also re-rolls the brain state a reload resets, as the game can: sensor scan phases (drawn from the entity's own random), behavior timers (e.g., AcquirePoi's next scheduled start), and memories that aren't saved, which are dropped. The bridge validates the restore for a given order; the runners sample orders. Why: day-1's unload-mid-visit cases re-board in the bridged order but lose the napper in the runner's order, because a different rider history gives different entity ids; both are orders the real game can produce.

Each load re-seeds every entity's random from the clock (generateUniqueSeed), so brain sensor phases, and the wake and scan times they set, are fresh draws after every reload, whatever the order. So a reload row is a sample of those draws, not one case. Where an entity's sensor phase can decide the outcome (a napper asleep or waking, a rider about to panic or calm, a summoner's group), the row sweeps every phase of the sensors that decide it. Where the tool can't force a draw, the row runs at least 40 samples and passes only with 0 failures, printing the rate and its 95% Wilson interval. Why: main's bed stop misses its napper in 6 of 32 reloads mid-nap (19%), and 10 random orders alone pass a hole that size about one time in eight. Each of those samples also draws a seeded order, so the samples cover the 8-order minimum rather than adding to it. Tick order after a load also decides outcomes, and it isn't a draw: it follows the order in which the saved entities come back. So the row names the entities whose phases and relative order decide it, sweeps every phase of their deciding sensors under every relative tick order of those entities (for two entities, the save order and its reverse), and passes only with 0 failures in every cell. The ≥ 40 seeded samples run as well, to catch any factor the named ones miss. Why: on main, the bed stop's napper is missed in 51 of its 400 sensor-phase pairs when it ticks before its cart, and in 61 of 400 when the cart ticks first; one wake time misses only in the first order and two others only in the second (tick-exact's forced-phase sweep, matched against the game on all 400 pairs).

What a reload resets, measured against the 26.3 server (tick-exact, main 999061360; logs https://artifacts.internalmeta.com/a/4223576527773868). Here a reload means an entity save and load (the unload's stage (c), or a restart); stages (a) and (b) keep the stamp below, aged by the freeze, and the activity. A reload clears `CANT_REACH_WALK_TARGET_SINCE`, which has no codec, so the brain's save drops it and the unreachable-bed clock starts again at every load (a code read; no log line prints the stamp). The load doesn't wake a villager loaded asleep: it stays asleep until `WakeUp`, at its bed. And after a load at night every villager is in REST until its hostile sensor first sees the zombie, up to 39 gt later (two sensor phases, each drawn fresh at the load: `VillagerHostilesSensor` reads the visible living entities `NearestLivingEntitySensor` fills, and neither memory is saved), whatever it was doing before; the reload rows sweep both phases. So the Shared bed radius holds whatever the night rule: every rider position lies more than 16 + 1 blocks from the shared bed's head, the napper's own path excepted (the must-have's time part covers it), since a calm rider in REST within 16 of the occupied bed loses its HOME in that window. The build guide tells the player to wait, after placing the villagers, until each one's bed claim shows its particles before going AFK (it catches a claim the first try missed).

#### Settled as placed, and built in order (every part)

1. **Settled as placed.** The design file pasted raw, with no settle ticks and no shape or neighbour updates, passes the part's bench under the same criteria as the settled run. So every cell is written in its settled, self-consistent state (wire and rail shapes, repeater, comparator and powered states), and item stocks (a hopper's or dropper's stock, a sorter's fillers) are listed build steps, not pre-stocked file states. Why: the wired loop's release failed from slot 1 in the real game when its hopper's item stock was missing (the paste hadn't filled it); with the stock merged, the loop placed raw runs 60 slots at 0 differences against the 26.3 server. So item stocks are listed build steps, and a design must work from its file as placed.
2. **Built in order.** The file placed cell by cell in its listed build order, each placement running the updates the game runs (updateShape on its neighbours, neighborChanged, and the ticks that pass between placements), then the bench. This catches what goes wrong while a player builds: an observer firing on the cell placed in front of it, a piston extending before its stop is placed, rails reshaping as later rails go down, a half-built circuit pulsing. Every design lists its build order (the order the build-minutes bill already assumes), and a part whose build needs a temporary block or a "place this last" step says so in the list. A design that lists none is built in the default order: supports first, then rails, then redstone from its sources outward, with containers filled before power reaches them. The ticks between placements are the priced placement rate (prices.tsv's place rows: a player's real gap at the stage), and the check also sweeps the gap from 1 gt up to that rate and prints the first gap that fails, beside the tolerance line. Each cell gets its own priced gap (place 1 s = 20 gt, plus place:edge's extra 1 s middle / 0.4 s late where the bill charges it, so up to ~40 gt), and the sweep runs up to the longest priced gap, since a half-built circuit runs longer during those gaps. tick-exact checks the built-in-order mode against the real game for the wired loop.

#### No golem spots off the pad (every part that carries villagers who may have slept)

Every loop or wired part that carries villagers who may have slept passes the golem-spots check with 0 cells off the pad, on the file as published (the same file the page serves, after every wiring pass). The check:
- runs from every position a villager can occupy: the loop's rail cells, plus every position the sleep routine or a branch puts a villager on (the siding or branch to the bed, the dismount spot, the walk to the bed, the bed, the stand-up cell and the way back), taken from a recorded full bench night together with the design's listed positions;
- runs the game's summon search from each position (redsim's golem_spawn_cells_from) with entities ignored: a cell blocked only by a cart, a rider or the zombie at the moment of the check still counts as a spot, because they move.

Pad cells count as allowed only when seen from the stand cells; a pad cell reachable from any other villager position (the arriving or returning batch, the queue, the napper's path) is an out-of-turn summon onto the pad, and the report flags it (`queue-loop-spots --pad --stand`). A flagged pad cell doesn't fail the part by itself: the full assembly bench measures the out-of-turn summon rate onto the pad and its slot cost, printed beside the flag, and the geometry changes only if those summons cost slots.

Why: main's published wired v2 had 30 off-pad golem cells on its wiring, and on the day-1 lane's first wired night a golem spawned on one of them and jammed the tail. The loop benches missed it because their riders never sleep, so no golem was ever summoned away from the pad.

#### Lightning (the whole farm)

A natural lightning strike goes to the nearest lightning rod within 128 blocks, if that rod is the topmost block of its column, and a strike on a rod never spawns a skeleton trap. Without a rod in range, a strike hits a random living entity that can see the sky within 3 blocks of the struck column, and a struck villager becomes a witch. So the farm has a rod within 120 blocks of every part of it, standing on an open 1-wide pillar, with three conditions. No entity is ever inside the bolt's damage box: no mob, no cart with or without a rider, no boat. The box runs from 2 blocks below the rod's own block to 10 above it, about 3 blocks around its column, and every entity in it takes the bolt's damage, which breaks a cart or a boat in one strike. No block other than the rod is within 1 of the block above the rod: a strike sets fire on any cell there with a sturdy floor or a flammable neighbour, and with stone all round, about 3 strikes in 4 light one. And the rod's redstone pulse when struck reaches no part of the farm. The static check that keeps zombies and boats out of empty carts' reach also proves the damage box clear of every place an entity's box can be, faults included; a rider's box reaches well above its rail. Acceptance includes thunder runs, with no villager struck, no cart or boat broken and no block burned. Why: in the Day-1 farm's game runs lightning burned a farm block, and the brief's rail-carousel record lost the station zombie's boat to a strike. A rod moves the strike and makes it trap-free, but it doesn't stop fire or the bolt's damage (tick-exact, bridged; the reviewer's 26.3 read).

#### The site (the placed farm)

Five ways a rider, its cart or the farm's zombies can be lost that only the site can prevent (tick-exact's empty-cart census, G1's review, the stroll bridge). `mctas mobfarm site-check <design>` checks each against the placed farm and prints one verdict line per condition, each with its positive controls. Every condition is checked from every position a villager can occupy, taken as the golem-spots check takes them: the loop's rail cells and every position the sleep routine or a branch puts a villager on (the siding, the dismount spot, the walk to the bed, the bed, the stand-up cell and the way back), riders and the napper on foot alike, with each villager's box and each cart's box at those positions, and from every zombie post with its zombie's and its boat's box.

1. **Lightning.** A bolt destroys a cart or a boat outright (`Entity.thunderHit` deals 5, and `VehicleEntity.hurtServer` adds 10 a point and breaks the vehicle past 40; a broken boat lets its zombie loose in the farm) and turns a villager into a witch. It reaches 3 blocks sideways, 3 below and 9 above its strike point. A strike goes to the closest lightning rod within 128 blocks of the struck column, provided that rod is its column's top block (by the world-surface heightmap: any block over the rod, glass included, takes it out), and lands on the block above the rod (`ServerLevel.findLightningRod`, 26.3). With no such rod it lands on the column's top, or on a living entity that sees the sky within 3 of it. So the rods the paragraph above requires (one within 120 blocks of every part of the farm, each its column's top block) take every strike in range, and every villager's, cart's, zombie's and boat's box lies outside that reach of each rod's top. The check verifies both, and prints each box's depth below the nearest surface a bolt could strike if a rod were lost (the top block of every column within 3 sideways of the box) as a second line, not a substitute. Positive controls, each failing: a box moved into a rod's reach; a rod covered by a block, so no longer its column's top; a part of the farm more than 120 blocks from every rod. This sharpens "Lightning (the whole farm)" above, whose thunder runs stay. A strike on the AFK player is not a farm loss: the card prints it on its own line ("AFK spot struck"), and the build guide's 7 × 7 opaque, non-flammable roof over the player's column, its block 4 above the player's feet, removes it.
2. **Natural mobs.** No mob that can spawn naturally (any category), come with a patrol or another of the overworld's spawners, or come through the farm's own portal can harm a rider, a cart (empty ones included: an arrow or a blast breaks one), a zombie post or a boat, board an empty cart, block a golem spawn cell, or make a rider flee. The check is the spawn-proofing check's own (`mobfarm exposures`, run on the placed farm), with these targets added to its channels, each with its positive control:
   - **Reach**, walking (every mob), climbing (spiders) and swimming (drowned), to within attack reach of a rider: the walker floods (C2 entry) and melee reach (C5 reach). Controls: a dark spawnable cell opened beside the loop; a wall a spider climbs to the loop; a water path a drowned swims to it.
   - **Flee**, any type the villagers' hostile sensor takes (`VillagerHostilesSensor`: zombie, zombie villager, husk, drowned, the illagers (pillager, vindicator, evoker, illusioner), vex, ravager, zoglin; not the zombified piglin) reaching within 6 blocks of any villager position, the flee threshold (`SetWalkTargetAwayFrom` at 6, `VillagerGoalPackages`), well beyond attack reach: a natural zombie at 5 blocks hands riders flee targets the design can't see. Control: a walker's cell opened 5 blocks from a rider.
   - **The napper's calm**: a villager lies down and stays asleep only while calm, so no type the hostile sensor takes may reach a spot within that type's panic distance of, and in sight of, the napper's bed, its walk to the bed or its stand-up cell, from its arrival until the wake (`VillagerHostilesSensor`: 8 for zombie, husk, drowned, zombie villager and vex; 10 for vindicator and zoglin; 12 for evoker, illusioner and ravager; 15 for pillager) (the farm's own wake zombie stays out of sight until then, as the zombie rule says). Controls: a walker's cell opened 7 blocks from the bed, in sight of it; a patrol's path 14 blocks from it, in sight.
   - **Pickup**, any pushable entity (hostile, passive or water mob, slime; not a bat, which isn't pushable, nor a mob on a ladder or vine) reaching an empty cart's pickup box (its box inflated 0.2 sideways) along any track an empty cart passes at 0.1 blocks a tick or more: a moving rideable cart seats any pushable entity there but a player, an iron golem or another cart (`OldMinecartBehavior.pushAndPickupEntities`, its candidates from `EntitySelector.pushableBy`), and takes the cart away from the left-behind napper's re-board. The brief's static check covers the farm's own mobs and boats; this covers the site's. Control: a slime's or a chicken's cell beside an empty cart's track.
   - **Golem spawn cells**: no entity that blocks building (every living entity, bats included, and boats and carts) can have its box meet the golem's 1.4 × 2.7 box at a spawn cell, since that blocks the summon (`IronGolem.checkSpawnObstruction` ends in `isUnobstructed`, which refuses any entity that blocks building, every living entity among them: `EntityGetter.isUnobstructed`, `LivingEntity`'s constructor). Control: a walker's cell opened onto a spawn cell.
   - **Endermen**, which reach the farm by teleporting, not walking: an enderman teleports to a random spot up to 32 blocks away on each horizontal axis and 32 vertically, dropping to the first floor below, and lands only where its 2.9-tall box fits (`EnderMan.teleport`, 26.3). So no floor inside the farm where that box fits lies within that reach of any spot an enderman can spawn or walk to; keeping every air space in the farm under 3 blocks tall guarantees it. And no block the farm relies on is in the `enderman_holdable` tag, since an enderman can take it. Controls: a 3-tall space in the farm within reach of a dark spawn spot; a holdable block in a post's wall. Golem spawn cells need 3 clear blocks, so they can't use the under-3-tall guarantee: every golem spawn cell lies beyond an enderman's teleport reach of every spot an enderman can spawn or walk to. An enderman carrying a block puts it down in an air cell about 1 block sideways and 0 to 1 up of where it stands, on a full-block floor (`EndermanLeaveBlockGoal`), so no air cell the farm needs (a sight line, a golem cell, a rail cell or a cart's path) lies within that reach of any spot an enderman can stand.
   - **Blasts**, a creeper within its blast of a rider, a cart, a boat or a zombie post (the blast also breaks carts and boats): C8 creepers, its targets extended to carts, boats and posts. Control: a creeper's spot within blast range of a cart.
   - **Ranged hits**, in sight (a clear collider ray, eye to box) within the shooter's range: skeletons, strays and bogged within 16 (they shoot from 15, `AbstractSkeleton`'s `RangedBowAttackGoal`), witches' potions within 10 (`Witch`), drowned tridents within 10, pillagers' crossbows within 8 (`Pillager`): C5 ranged, its targets extended from golems and villagers to every rider, cart, boat and post, and its shooters to strays and witches. Control: a skeleton's spot in sight of a cart within 16.
   - **Patrols**, from their own spawner (`PatrolSpawner`), not the natural one: C14 patrols, its targets also every villager position within each member's panic distance and in sight (as in the napper's calm), since a panic the design didn't plan can summon a golem out of turn or keep the napper awake. Control: the AFK spot moved where a patrol can spawn.
   - **Other spawners**, each counted as a walker source or ruled out on the card:
     - wandering traders and their llamas (`WanderingTraderSpawner`: surface cells within 48 of the AFK player, at any light), which walk, can come down a lit access path, and can board an empty cart;
     - village sieges (`VillageSiege`: 20 zombies on dark surface cells inside a village at night, with no distance from the player kept), possible only while the AFK player stands within one section of an occupied village POI, so the AFK spot lies 2 or more sections (Chebyshev, in section coordinates) from the shared bed's section; control: the AFK spot moved within one section of the bed;
     - Raids. While the shared bed's ticket is taken (after a fault uses the spare), the bed is an occupied village POI (HOME) and the farm counts as a village to a raid: a player with Bad Omen whose section lies within 1 section (Chebyshev) of a section holding an occupied village POI (any job site, bed or bell) gets Raid Omen, and 600 gt (30 s) later a raid starts, centred on the mean position of the occupied village POIs within 64 blocks of where the player stood, here the shared bed. A raid would bring raiders, and vexes that pass through blocks, among the villagers. Bad Omen comes only from drinking an ominous bottle (dropped by a pillager captain, or found in a trial chamber's reward chest) and lasts 120,000 gt. So the build guide tells the player never to drink an ominous bottle near the farm, and never to come within 1 section of the shared bed's section while Bad Omen is active. The AFK spot's distance (at least 2 sections from the bed's section) already keeps an AFK player out of range; the guide line covers walking past. Why: 26.3 code read (b8949c96): BadOmenMobEffect.applyEffectTick checks Level.isVillage (sectionsToVillage ≤ 1 over occupied POIs of the #village tag: the acquirable job sites, home and meeting), gives RAID_OMEN for 600 gt at the player's position, and RaidOmenMobEffect calls Raids.createOrExtendRaid on its last tick, centred on the mean of the occupied #village POIs within 64; OminousBottleAmplifier is the only source of Bad Omen.
     - phantoms (`PhantomSpawner`), which need the AFK player at or above sea level with its cell seeing the sky, which the roof prevents (as does an AFK spot below sea level); control: the roof removed;
     - cats (`CatSpawner`), which need more than 4 occupied beds within 48 of the AFK player or a swamp hut, ruled out on the card.
   The spawn sources include, each with its control: slimes in slime chunks below y 40 at any light (`Slime.checkSlimeSpawnRules`), so lighting doesn't stop them and the check reads the placed farm's chunks (control: the farm placed in a slime chunk); bats in any part of the cavity under the world surface at light 3 or less on a floor they spawn on (`Bat.checkBatSpawnRules`, 26.3: under the world-surface heightmap, not sea level; control: one dark cell of the cavity); and the farm's own overworld portal, whose random ticks spawn zombified piglins on its base (`NetherPortalBlock.randomTick`: `nextInt(2000) <` the difficulty's id per portal block per random tick, on the first block under the portal if it is a valid spawn): the check proves those spawns can't reach any track, rider, post or golem cell, or that the portal's base isn't a valid spawn (control: a path opened from the portal to a track).
3. **Empty-handed zombies.** Every zombie the farm uses holds nothing in either hand, and no item can reach a zombie post. Why: a zombie holding a spear uses it (`Zombie` adds `SpearUseGoal` at priority 2), and a spear's stab dismounts its target (`KineticWeapon` computes `dealsDismount` from the spear's dismount conditions and calls `LivingEntity.stabAttack`, whose `target.stopRiding()` spares only the `cannot_be_dismounted_by_item_usage` tag, which holds only `minecraft:interaction`; 26.3, code read). And a zombie that can pick up loot takes any item within its pickup box (its box inflated by 1, 0, 1, `Mob.getPickupReach`). tick-exact's census found death, conversion, cart destruction and a powered activator rail as the only ways a rider leaves in the farm as built; a held spear would add one. The build guide says how: the zombies are brought in empty-handed and the posts are closed to items. The check reads every zombie's hands in the design and proves that no item path (a drop, a hopper or dropper, water, an item a golem or villager drops) ends within a post's pickup box. The same item-path check covers every tripwire string the farm relies on, since an item resting on a string holds it on, and every wooden or weighted pressure plate, which items press; a stone plate ignores items (`TripWireBlock`, `PressurePlateBlock`'s sensitivity, `WeightedPressurePlateBlock`, 26.3). Controls, each failing: a held item; an item path ending at a post; an item path ending on the bed stop's FC string.
4. **No village POI within 8 sections.** Any occupied village POI near the farm (a claimed bed, job site or bell) bends a panicking rider's stroll toward it from 2 to 8 sections away, which can strand its walk targets, and a homeless rider claims any free bed within 48. So no bed other than the shared one, no job-site block and no bell lies within 8 sections (Chebyshev, in section coordinates, on all three axes) of any position a villager can occupy, claimed or not, the player's own beds included; 144 blocks or more along any one axis guarantees it. That covers natural villages, trading halls, other farms' villagers and the breeder. The site guide and the build guide put the player's bed beyond 8 sections of every villager position. The farm's own shared bed is the one exclusion: it is the one occupied POI the design relies on, and its pull is bridged. Any other bed, job site or bell in the placed farm counts, as the static checks above already say. The check prints the shared bed's position and the excluded set. Control: a second bed seeded 7 sections out fails.
5. **The farm's zombies stay.** The night rule needs a zombie in sight of every night position, and one missing zombie leaves riders calm, which costs HOME. So every zombie:
   - never sees the sky in daylight (a zombie whose eye cell sees the sky burns in daylight, `Mob.isSunBurnTick`, unless it wears a helmet);
   - never despawns: a zombie riding its boat never does (`Mob.requiresCustomPersistence` returns `isPassenger() || isLeashed()`, and `checkDespawn` removes no persistent mob; 26.3, code read); a zombie out of its boat needs a name tag, which the build guide gives it; and the world is never on Peaceful (`checkDespawn` discards every hostile there);
   - is never attacked by a golem: an iron golem targets any enemy but a creeper within its follow range without needing sight (`IronGolem`'s `NearestAttackableTargetGoal`, must-see off), so from every position a golem can occupy (its spawn cells, its route to the portal, faults included) no golem's melee attack box meets a zombie's box with a clear line of sight, and no golem can path to within that reach;
   - is never reached by water: a zombie whose eye stays in water 600 gt converts to a drowned 300 gt later (`Zombie`'s drowning tracker), and what becomes of its boat seat isn't bridged.
   The check reads each zombie's sky exposure from its eye cell, its persistence (riding, leashed or name-tagged), the golems' reach to it, and any water path to its post. Controls, each failing: a post opened to the sky; a zombie out of its boat with no name tag; a post opened to the golem route; a water path to a post.

Why: each of the five loses a rider, a cart or a zombie in a way no part bench sees, because a bench places no village, no weather and no natural mobs around the part.

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
   again, the next villager claims the fresh record; N = 21 break-and-replace steps, the last one after the final claim, priced as hand steps at build time). The build's bed-sharing procedure ends with a break-and-replace after the last claim, so the bed starts with one free ticket. That spare lets a villager that lost the bed to another's sleep take it back, but only once. A release adds nothing while the spare is there (the cap is 1), so any release-and-reclaim uses it up as well; no villager gives up the bed in normal running, so the spare lasts until a fault. Every bench and integrated file starts with the bed's free tickets equal to what the build leaves (one) and prints the count at every change. The design includes the mechanism that keeps every claim, and the
   judge proves it over many nights (6 days on 100 seeds at least; every villager's HOME = the bed
   at every midday and every dawn):
   - **CANT_REACH:** SetWalkTargetFromBlockMemory(HOME), in the REST package only, releases the
     bed and erases HOME when CANT_REACH_WALK_TARGET_SINCE is older than 1200 gt. A panicking hold
     still arms that memory (MoveToTargetSink is CORE and paths to PANIC's flee targets and 2-block
     strolls; an unreachable one sets it, and it keeps its first-failure time into REST), and at the
     napper's first REST tick SetWalkTargetFromBlockMemory (priority 2) runs before SleepInBed (3).
     The default (main's measurement predates the 6-to-8 rule; see 'What clears riders' stamps'): riders' stamps clear before they reach 1,200 gt, so no villager gives up the bed in normal running (the stamp rule in the self-recovery paragraph above). Two other paths: (a) after a fault, the napper may release the bed at its first
     REST tick and reclaims it through AcquirePoi(HOME) (CORE priority 10, a scan every 20-40 gt)
     because it is the only villager without HOME, the bed is free and reachable from beside it, and the
     ticket cap is 1; the nap's timing budgets the reclaim (≤ 40 gt), and the judge checks every
     villager's HOME = the bed at every dawn and midday. The ≤ 40 gt reclaim holds only if the
     napper's first path to the bed succeeds: AcquirePoi scans when HOME is absent (at once if it
     scanned before; within nextInt(20) on a fresh brain), then every 20 + nextInt(20) gt, for a
     bed with a free ticket that isn't OCCUPIED; a failed path puts the bed in its batchCache with
     a jittered linear retry that waits longer each time. So the delivery leaves the bed unoccupied
     and reachable from where the napper stands at its release, or the bench shows the retry fits
     inside the visit. (b) The alternative: holds and lanes generate no unreachable panic walk
     target, and the bench logs every held villager's CANT_REACH every tick. A rider whose path
     from its cart reaches the bed never arms it (bridge scene bed-cart); once armed, it is
     cleared by a walk that can reach (or reaching its target), by lying down (SleepInBed.start:
     on foot within 2.0 of the bed, the bed free, not woken in the last 100 gt; never a rider), or
     by a reload; SleepInBed.start erases WALK_TARGET and the stamp even when startSleeping fails,
     and sets LAST_SLEPT only when it succeeds (the stamp rule in the self-recovery paragraph
     above).
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
- A villager that holds a navigation path re-plans it when a block's collision shape changes within the path's reach: a cell nearer the midpoint between the villager and its path's end than the number of nodes the path has left (`PathNavigation.shouldRecomputePath`). A re-plan due within 20 gt of the last one is held as pending, and a pending re-plan stays pending while the villager can't update its path (airborne, say). A pending re-plan survives the stop SleepInBed makes when the villager lies down: once the sleeper is on the ground and 20 gt have passed, it re-paths to its old target, walks off the bed and wakes. So from 20 gt before a napper lies down until it wakes, no collision shape within its path's reach may change (a gate, door, trapdoor or hatch, a piston, a placed or broken block), unless the napper holds no path and no pending re-plan at that moment; a swap between two full cubes changes no shape. The sleep routine's rows print every collision change in the part's box with its tick against every lie-down, and whether the napper then held a path or a pending re-plan (the navigation's hasDelayedRecomputation), and fail on any inside that window. Why: bridged (scene repath-nap, 20 lanes, 300 ticks, 0 differences): two cells set from air to stone near the middle of a panicked rider's flee path 14 gt before its drop made every such rider walk off the bed and wake 13 gt after lying down; the same cells swapped stone to redstone block, set 25 gt before the drop, or set 30 gt into the nap gave clean naps, as did no change. The bed siding's 8 early wakes in 168 visits came from its bench setting redstone blocks into air 14 gt before the drop.
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
- **No meetings on climbs or boosters.** A moving cart on a climb (track rising in its direction of
  travel) or a boosted cell must never meet a stopped, crawling or frozen cart (crawling: slow
  enough that the meeting leaves the pusher moving backward on that cell): on a climb gravity
  reverses the pusher, and a booster drives any backward motion harder (0.06 a tick, about 8 times
  gravity's pull on a slope). Carts touch at centre distances up to about 1.18 blocks along the
  track, so the keep-off zone is every climb and boosted cell plus every cell within that reach of
  one, in practice the first cell past each. Each part names, cause by cause, what keeps stopped,
  crawling or frozen carts out of that zone whenever carts can arrive, and its rows show it, faults and
  freezes included. The rows print the slowest cart met on each climb and boosted run. The
  keep-off zone is a minimum. Every cell where a cart can come to rest, faults included, other than
  a declared rest cell (a pocket, queue slot or hold where carts wait by design), has push-in rows:
  a cart stopped at each position across the cell, met by the next arrival at every speed it can
  have there, goes on to its destination with no reversal, merge, derail or back-crossing. Where
  the push fails, the cell is kept clear like the zone. A declared rest cell takes arrivals against
  a held cart only on plain-rail downhill track, and the downhill-queue rows below are its push-in
  rows (its destination being its place in the queue). A declared rest cell anywhere else takes no
  arrival while a cart is held in it: the part keeps the next cart back until the cell is empty, by
  its gate or by moving a cart only into an empty slot, and its rows show it, faults included. Why: on the
  (c′) dip, stops at z 9.5 and 10.5 were pushed through, but a stop at z 11.5, outside the zone,
  made the pusher reverse on the flat. A downhill
  queue on plain rail, where gravity keeps pressing carts forward, may take arrivals against
  stopped carts, shown in its own rows: per arrival, the impact speed and where it lands, no cart
  pushed back across a cell boundary, and the cart at the front still reaching its gate within its
  window with at least 2 gt to spare; a boosted cell in a queue falls under this rule, not the
  exception. Why: on the stand's approach, a cart stopped on the climb's
  flat top is met by the next train while that train is still on the last slope; the pusher
  reverses and rolls back down the boosters into the queue (0–3 clean of 12 per position), and
  powering the last slope made it worse (0–2 of 12).
- **Carts in a curve.** A cart's box stays axis-aligned (0.98 × 0.98) through a curve, so on the two perpendicular pieces of a 90° curve two carts collide whenever they are under 0.98 apart on each axis, up to 1.39 blocks apart in a straight line, and the push between carts can't separate them across a corner (|cos| about 0.71). A follower that reaches a curve while the leader's box is still in its corner stops dead and stays stalled on flat track, or, if the pair arrived chain-pushing, bounces back. So at every curve, consecutive carts keep apart, at every ±2 corner and in every fault row, by at least the time the leader needs to clear the corner: the follower's box front reaches the curve cell's near edge only once the leader's centre is 0.98 or more past the curve's centre along the exit. That is a path spacing of about 1.68 blocks, since a cart crosses the curve cell on its 0.707-long diagonal: about 4.2 gt at 0.4 a tick and 8.4 gt at 0.2. Each part prints, for every curve, its least margin against that condition and the number of ticks on which two carts' boxes touch there, which must be 0. A cart pressed against another on powered rails gets no start kick from it, and a kick from a solid block behind the rear cart can't move it through the front one, so no design relies on a powered rail's start kick to move carts resting pressed together. Why: bridged (scene curve-pair, 200 ticks, 0 differences): ridden pairs at 0.4 passed a curve from 1.4 blocks apart and stalled for good at 1.2; empty pairs at about 0.2 needed 1.8; pairs starting 1.0 apart chain-pushed and the follower bounced back; a pair touching at a curve's foot on powered rails against a stone never started in 200 ticks.
- **Rail junctions.** A plain rail joined to three rails settles by a fixed order that doesn't turn with the part: unpowered, the last valid of north-west, north-east, south-west and south-east; powered, the last valid of south-east, south-west, north-east and north-west. By its missing arm: north missing, south_east unpowered and south_west powered; east missing, south_west and north_west; south missing, north_east and north_west; west missing, south_east and north_east. A cart from the arm the shape leaves out runs straight through to the opposite arm. So a part turned or mirrored can route the other way for the same power, and every junction's routes are checked in the orientation and mirror it is placed in. A junction re-settles only when a signal source next to it, or one powering a block next to it, changes state (a repeater, a torch, dust, a lever, a detector rail, or a redstone block being removed; a repeater updates the block it faces and that block's neighbours, and dust, torches and levers update their second neighbours too); a redstone block placed next to a settled junction doesn't switch it. So a junction is powered only by its designed control, a component that changes state next to it or under it, and no other source next to it, or powering a block next to it, may power it. A detector rail on one of its arms powers it as a cart crosses and re-settles it under that cart wherever its powered shape differs from the shape the cart needs. The junction check fails a file that breaks either rule. Why: bridged (scene junction-settle, 200 ticks, 0 differences); day-1's siding, half-turned for the W place, sent every visiting cart to the exit, first through the settle order and then through its entry detector powering the T under each cart.
- **Stoppers.** A fence gate, a thin pole (chain, iron bars or end rod) or a low block in the rail's place stops a cart only where the cart meets its face from its own height. A cart climbing into a closed gate, a chain or iron bars over the first or second ascending rail is never stopped (it enters by the rail's height snap and gets as far as its speed carries it), so no gate, chain or iron bars stops a cart moving uphill into it. A chain or iron bars over a slope's foot rail never stops a descending cart (the cart drops into the foot cell with its box already overlapping the pole, and an overlap with a closed gate, a chain or iron bars never stops a move; a chain or iron bars appearing in a cell the cart already overlaps doesn't stop it either). An end rod's shape has a grid line at its centre: it stops a cart whose leading face starts in its near half (between its face and its centre line), at the centre line, and on a slope that cart rolls back or rests there; so each part prints end rods with their carts' entry phases. A pole over the first flat rail past the foot holds every descending cart from 0.1 to 0.8, ridden and empty, resting on the foot rail against it, and a cart moving along that flat passes under it. A bottom slab or bottom trapdoor in the rail's place ahead of a slope's foot holds only slow descending carts (empty up to 0.4, ridden at 0.1, the slab up to 0.2); faster ones drop over it and roll through. A stopper moved by a piston moves only while no cart's box overlaps the cells it sweeps: pushed while a cart is under it, it shoves the cart off its rail, and pushed late it can put a rider's eye inside it. Each part prints, for every stopper, the directions and speeds of the carts that reach it and whether each one stopped. Why: bridged (scenes cart-pole, cart-pole-stop, cart-pole-climb, cart-pole-push and cart-pole-head at 8268de10f, 452 lanes, and cart-pole-rod, 174 lanes, 200 ticks, 0 differences, after the redsim fixes the bridge exposed; over a slope's foot rail an end rod held 1 of 30 descending carts, ridden at 0.25, resting on the slope with its leading face on the rod's centre line, while chains and iron bars, met or appearing, held none); the backward empty cart that went up through R1's shut gates climbed through them this way.
- **Arrivals into a waiting queue.** The downhill-queue exception holds only for gentle arrivals. A cart arriving at a cart that waits or rests ahead of it on the same track can push into it and merge. A waiting rider's box acts as a wall only behind a ridden cart resting against the gate; further back it gives way to faster arrivals, and an empty waiting cart stops nothing. So wherever carts wait in a queue, every arrival meets the cart ahead at a stored speed of 0.2 or less, at every ±2 corner and in every fault and freeze row. Each part prints, per waiting place and corner, the stored speed at the contact and the closest approach, with 0 merges and no cart shoved back into a junction's or a gate's cell. Why: bridged (scenes h-contact, 352 lanes, h-contact-sweep, 384 lanes, and h-contact-p1e, 64 lanes, 0 differences each): at 0.2 or less no arrival merged or crossed into the junction's cell in any geometry or waiting pair tried (sloped, hybrid and flat, all five waiting pairs, ridden and empty arrivals; the hybrid and flat geometries one copy per lane). The worst pair, a ridden arrival behind an empty cart at the front place on the hold's slope, ran 64 copies at 0.2, each at its own place in the world, so its own entity order, with contact phases 1/64 block apart: none merged, though centres came as close as 0.175. The same pair merged in 2 of 64 at 0.25, and in 3 and 6 of 16 sampled copies at 0.3 and 0.35. At 0.4 and above, merges appeared in every geometry and waiting pair tried, some shoving the waiting cart back into the junction's cell. The S3 stands measured arrivals of 0.82 to 1.40.
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
