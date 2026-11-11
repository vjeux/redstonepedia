# Carousel bed siding (vjeux's single lane; ironfarm's hand-over 2026-11-10 07:11)

The one shared bed. A napper's cart alone leaves its batch at the stand's departure and runs into
the siding. The napper is dropped beside the bed, naps, is woken by the dorm zombie through the
hatch, and re-boards the same cart. The cart then waits on the exit slot until ironfarm's release
pulse sends it out to rejoin its batch on the release run (~600 gt after it left).

Status, in vjeux's terms (07:29: "you can't say it's working when nothing is wired"), the page's
line: **Real redstone; tested on its own; arrivals and release are driven by the test bench; not yet
joined to the loop.**
- **Wired:** `carousel-siding-wired.txt`, real redstone.
  - The bench touches no control and no villager memory: every villager's HOME = the bed comes
    from the file's `port home:` line (R1's state; its 20 break-and-replace steps priced).
  - It does two things in place of ironfarm's loop. It brings each arriving cart in: on the page's
    run, launched from rest on the bench's launcher (three boosters west of the approach); in the
    statistics runs, also teleported onto the approach at a drawn speed. And it places the 10-gt
    release as a block at the release input.
  - The riders wait in a bench hold (21 carts, over 16 from the bed) made to panic all night by
    hold zombies, as the loop's riders must.
- **Real build, but a bench design.** The launcher, the approach, the run-out past the exit and the
  hold are the bench's.
- **Plays in the viewer:** `carousel-siding-wired.drive.txt` (`siding-bench --wired --launch --from
  1 --to 1 --nights 1 --drive-out`, captioned for the view's badge). `mctas web drive-check` holds
  the view's own run to it: "ok: the view's run is the bench's" (219 acts, 325 cart positions,
  48,000 ticks).
- **Missing:** ironfarm's two junctions (the entry off the departure run, the merge into the
  release run) and the zombies along the loop.
- **The give-up (day-1's force input F):**
  - **F's window, met** (2026-11-10 21:30, the long nap path below): the stuck cart's F to past
    the exit is 55-58 gt against ironspec's 60.5 at k2 with the +6 missed-golem corner (day-1's
    +4 gt of D at k2): the double-fault corner closes by 2.5 gt; at nominal F the margins are k0
    25, k1 12.5, k2 8.5.
  - **Open: self-recovery after F dropped** while the napper cannot sleep: 3 of 16 seeds jam (13
    recover within one visit); the cause is traced (below), not yet fixed.
  - **Open: unloads** (the loop's chunk lines through the siding, and the whole part; below): a
    freeze of the bed's entities during a nap jams the siding.
- **Also kept:** `carousel-siding-bench-driven.txt`, the same siding with the controls switched
  by the bench (set_block): the first bench, before the circuit.

Tools: `mctas mobfarm siding [--wired] [--alone] [--exit-brake] [--unsettled] --out F` (settled by
default), `siding-check F` (every rider/napper cell's golem search, the siding's own block count and
box), `siding-bench F [--wired] [--launch] --from --to --nights --speed a,b --gap a,b --release a,b
[--visits N] [--release-held] [--placed-raw | --placed-in-order [GAP] | --placed-all |
--build-gap-sweep] [--drive-out F] [--trace --trace-window a,b]`.

## Interface (local frame: x east, z south, y up)

| what | cell | |
|---|---|---|
| entry | (0,0,0) | plain east_west rail; the cart arrives moving +x, 0.10 to 1.2 b/t stored all work (ironfarm's cap ~1.0) |
| release input | (4,-2,0) | the slot slope's support; strongly power it for 10 gt (ironfarm) |
| exit | (6,-1,0) | plain east_west rail; the cart leaves moving +x at 0.28-0.44 b/t stored (median 0.29) |
| box | x -7..6, y -4..4, z -3..9 | 262 blocks (the mechanical part alone: 60) |

Rules for the loop:
- Arrivals at least ~270 gt after the previous visit's arrival; the ≥ 4-release spacing gives 400.
- Every rider panics all night except the napper in the cage.
- No rider calm within 16 of the bed while a napper sleeps: ValidateNearbyPoi forgets the bed for
  good, and the sleeper keeps the ticket.

## How it works

- **Track.**
  - A T at (1,0,0) with its stem south: powered south_west (entry to the bed line), unpowered
    south_east (the bed line to the exit branch).
  - The bed line, from the T south: an always-on booster (1,0,1), so slow arrivals still park
    against the stop; the free cell (1,0,2); the activator (1,0,3); a slow run; the park rail
    (1,0,6) against a cobblestone.
  - The bed: head (0,0,2), foot (0,0,3), facing north, in ironfarm's glass cage. The iron hatch
    is at (2,1,2) and the dorm zombie in a boat at (4.5,1,2.5).
  - The exit branch, from the T east: (2,0,0), then a detector rail at (3,0,0), then the slot (a
    braked powered rail sloping down east at (4,-1,0)), a plain flat, and the exit.
- **Circuit.**
  - The nap is sensed by an observer in the cage wall south of the foot (0,0,4). Its pulse is
    stretched by a 4-tick repeater into an RS-NOR latch west of the siding (y -3).
  - Latch set means a visit is in progress. Torches under the T's support, the activator's
    support and the hatch's control turn the T to the exit branch, turn the activator off (the
    empty cart meets the napper over it) and shut the hatch (the zombie is seen and the napper
    wakes, 16-35 gt later).
  - The latch is reset by the detector rail: only the returning cart passes it. The detector sits
    two rails from the T because its power turned the T under a cart still on it.
  - The kick: a wall torch under the park rail's support on a block BK. BK is fed by "not visit"
    through five 4-tick repeaters (the cart meets the napper 71+ gt after the drop, past the
    60-gt boarding cooldown), OR by a tripwire across the slot (the slot is taken, so the kick
    waits for the release).

## Evidence

(Before 2026-11-10 14:30, with the bench's bed acts and its old schedule of about 22 visits a night;
the run without them is in "The one bed" above.)

Wired, `siding-bench --wired`, 100 seeds × 6 nights:
- Settings: visit gaps 400-600 gt, release 560-610 gt after arrival, arrival speed 0.15-1.1 b/t.
- **15,490 visits**: all dropped, napped, woken, re-boarded their own cart, ready on the slot and
  released. 0 problems.
- Ready 115-290 gt after arrival (median 145, p99 263); 0 over the 500 gt limit.
- Naps last 16-35 gt. The re-board comes 71+ gt after the drop.
- Waiting riders panic 99.50% of night ticks; the only calm ticks are the first ≤ 8 gt after the
  cart stops.
- Exit speed 0.28-0.44 b/t.

The same bench with the controls bench-driven gave 15,490 of 15,490 as well (ready 119-277 gt).

Wired, launched (the page's design and its arrivals), `siding-bench --wired --launch`, 100 seeds × 6
nights: 15,490 of 15,490 visits napped, re-boarded their own cart and were released; ready 118-283
gt after arrival (median 151); naps 16-35 gt; re-board 73+ gt after the drop; waiting riders
panicking 99.58% of night ticks; exit 0.36 b/t; 0 problems. (The member's metrics.)

`siding-check`: 0 golem-valid spots in the searches from every rider and napper cell
(entry, T, bed line, free cell, branch, slot, exit).

## The one bed: R1's claims are the design's (ironspec's check, 2026-11-10)

Until 2026-11-10 14:30 the bench handed the bed out itself at the start (`poi-take` of the bed's
ticket, every villager's `home` and `last-slept` set): a bench act the status never named. Now the
bench touches no villager memory:
- **The state is in the file.** `port home: 0 0 2` (the bed's head) gives every villager HOME = the
  bed as it spawns, the state R1's build steps leave (brief farm-iron-carousel rule 1: a villager
  claims the bed, the bed is broken and placed again, the next claims the fresh record). The bed's
  ticket starts **free**, in redsim and in the game alike (tick-exact 2026-11-10: AiWorld starts
  with explicit empty tickets since dd744b4378, and a load takes none; corrected here: this note
  said redsim counted the claims as taken). R1's real end state has it taken by the last claim;
  the difference matters only to a villager without HOME before the first nap (none in normal
  visits: every napper's release-and-reclaim then takes and frees it as in the game).
- **The steps are priced.** prices.tsv's `bed_reclaim` (5 s a step: break and place the bed again,
  bring the next villager within a path, wait for its scan), N - 1 = 20 steps for the 21 villagers:
  100 s in the bill at every stage (`mobfarm bill`: 103.7 build minutes middle, 33.3 late, the
  bench's 21 carts included).
- **ironspec's check**, `siding-bench --wired`, 100 seeds x 6 nights, every villager once a night
  (the schedule now gives each of the 21 one visit a night, 400-500 gt apart; arrivals teleported
  onto the approach, the release placed by the bench): 1,400 dawn and midday checks, all 21 holding
  HOME = the bed at every one; 12,600 villager-nights, each with exactly one nap; 0 problems.
- **What moves HOME:** the napper drops it at its first REST tick and takes it back (the brief's
  release-and-reclaim): its CANT_REACH_WALK_TARGET_SINCE, armed while it panicked in the hold, is
  older than 1200 gt, so SetWalkTargetFromBlockMemory releases the bed; AcquirePoi(HOME) takes it
  back from the cart or the cage. 1,195 times in the 100 x 6 run, without HOME p50 10, p99 98, max
  121 gt, every one taken back before its nap; no other loss (no ValidateNearbyPoi erase: no rider
  calm in REST within 16 while a napper sleeps).
- **(a) cage-only, measured and not taken.** Closing the bed's open north front (0,0,1) (`siding
  --cage-only`) leaves the bed reachable only from the cage, but the napper's reclaim from the
  approach then fails and backs off: drop to asleep up to 370 gt, ready up to 489 gt (8 seeds x 2
  nights) against 114 and 287. Every other villager already holds HOME = the bed (R1), so none
  looks for one.
- **(c) the give-up: built** (below).

## The give-up, driven by day-1's scheduler (2026-11-10, the parent's direction): +19 blocks

A napper that cannot sleep must still leave in time, or one bad night stalls the queue. The parent
chose to reuse day-1's scheduler timing over the siding's own timer (below, now `--give-up timer`):
the siding takes one input, F, "force the re-board now" (`mobfarm siding --wired`, the default
`--give-up scheduler`), agreed with day-1 (16:34, 16:47; wired on explore/day1-counter 7d947146b,
17:01):
- **F, the input:** the cell (-5,-3,-4), just north of the siding's box; day-1's line (its visit
  register's D3, the 3rd release after the visit, gating REL+20) ends there in a dust and pulses
  12 gt, on at visit +328..+340 (the same for every k), 118-150 gt after the arrival, on **every**
  visit: its register cannot see the slot or the return.
- **V, a visit under way** (since 2026-11-10 18:30 a set/reset latch; the copper bulb's phase
  inverted after a fault and blocked every later F): two droppers and one item. DA (0,-1,-1),
  beside the arrival's stone (0,-1,0) (the entry rail (0,0,0) a detector rail), fires on ARR and
  drops the item north into DB; DB (0,-1,-2), beside the R line's support stone (0,-1,-3), fires
  on R and drops it back south. A second ARR or R finds its dropper empty and does nothing (set
  and reset idempotent). The item in DA = no visit: a comparator (-1,-1,-1) reads DA, a repeater
  lifts its strength 1 to 15, dust west and down north into the repeater (-4,-3,-3) into the
  gate's side. The item is a build step (`DA` filled with 1 cobblestone). Glass caps on both
  droppers. (`MCTAS_SIDING_V=bulb` writes the old bulb.)
- **The gate:** a comparator (-5,-3,-3) reading F at its back and subtracting not V at its side (a
  torch on P (-3,-3,-2), P powered by V through the bulb's comparator and dust; a repeater from the
  torch into the side), out through a repeater into the H latch's D2: F sets H only during a visit.
  A napper asleep by then set H long ago (nothing changes); a cart already back has turned V off
  (the pulse dies at the gate); a napper still awake is sent out (the T to the exit, the activator
  off, the hatch shut, the kick 20 gt later; the kick picks it up on the free cell).
- **Cost:** 19 blocks with the bulb (the siding 234 -> 253, the box unchanged 14x9x13, 19.8 ->
  22.1 build minutes at middle prices); 20 with the dropper latch (254).
- **The forced exit's speed (ironspec's part A, 0 blocks):** the free cell (1,0,2) a booster (a
  powered rail on a redstone block, always on), and delay moved from the kick chain (H to the
  kick, which F also waits through) to the nap path (S to H, which F skips): two 4-tick repeaters
  in the S line at (-1,0,5), (-2,0,5), the chain **[4,2,1,1,1]** (was five 4-tick). A nap's onset
  to its kick stays about the same (drop to re-board min 65 against the 60-gt boarding cooldown: 5
  gt of margin; the nap lengthens to 19-51 gt, the hatch shutting later). F to the cart past the
  exit: 95-109 -> **67-70 gt** (`MCTAS_SIDING_MOVE`, `MCTAS_SIDING_BOOST`, `MCTAS_SIDING_CHAIN`
  vary them; the chain [4,4,1,1,1] gives 71-74 with drop to re-board min 69). Where the 95-109
  went (a trace): F to H 7 gt, H to the kick 42 (the chain), the kick to the exit 47 (the empty
  cart crawling 0.13-0.23 b/t from the park to the free cell, the pickup at z 3, 18 gt to the T, 9
  to the exit).
- **The dropper latch's build:** its item a build step after every placement (`build:
  fills-last`), then the glass caps, then the two torches: merged into DA's placement the
  comparator reading DA got no update and the not-V line stayed dark, unlike the file.
- **F to the cart on the slot with its villager:** 94-96 gt with the slot free (132 when the
  bench's previous cart was still on the slot), so day-1 fires F 95 gt before its +420 deadline.

Results, `siding-bench --wired` with F driven as day-1's scheduler will (a 12 gt pulse 118-150 gt
after each arrival, every visit; redsim; arrivals and the release bench-driven):
- normal visits, 100 seeds x 6 nights: 100 of 100 clean, 12,600 visits, ironspec's check unchanged
  (all 21 holding HOME at all 1,400 dawn and midday checks, one nap per villager-night), ready
  115-287 gt; every napper asleep before its pulse (none cut short; none either with the pulse at
  110-135 or 114-144 gt);
- every napper failing (`--day-visits`), 16 seeds x 2 days, 672 visits: 16 of 16 clean, every cart
  on the slot with its villager 213-279 gt after its arrival (the pulse to ready 94-150, median 95)
  and released, all 21 keeping HOME;
- three villagers' HOME erased before each visit (`--fail-no-home 3`), 16 x 2, 672 visits: 16 of 16
  clean: the 80 visits not asleep by the pulse all on the slot with their villager within 280 gt
  of the arrival; the other 18 napped every night (the cascade above did not show in these seeds);
- placement, 24 seeds x 2 nights: the judge's, settled as placed and built in order (338 cells,
  8,600 gt) 1,008 visits each, all clean; siding-check 0 off-pad golem spots (the bench's and the
  alone file); the drives recorded again with the force pulses as `set` acts (126 acts), web
  drive-check ok.

**F's tolerance line** (ironspec 2026-11-10; F swept earlier and later to the first failure; release
held, as day-1's loop runs it, unless said):
- **Earliest F** (it must come after the latest sleep onset, or it sends an awake napper out):
  sleep onset after the arrival p50 17, p99 39, **max 112 gt** (100 seeds x 6 nights, 12,600 naps).
  F at a fixed time after the arrival, 24 seeds x 2 nights (1,008 visits) each: 100 and 80 gt cut
  no nap; 60 and 50 gt cut one (1 in 1,008); 40 gt none cut but one villager not asleep by it; 30
  gt fails (16 of 24 seeds: F lands before or at the drop). Day-1's F at 118-150 gt (118 at k = 2,
  the arrival at +210): **6 gt over the slowest onset seen**; 0 cut in 25,200 visits at 118-150
  (both release modes), and 0 in 12,600 each at 114-144 and 110-135.
  **The tail over 52,500 naps** (100 seeds x 25 nights, the dropper latch and chain [4,2,1,1,1],
  release held, F at 118-150): onset p50 17, p99 39, p99.9 78, p99.99 113, **max 121 gt**; over
  60 gt: 64 naps, over 80: 50, over 100: 18; the 8 slowest 111, 113, 114, 114, 115, 116, 116,
  121; 0 cut. The >100 gt onsets are the napper's R1 release at its first REST tick and its
  AcquirePoi reclaim (HOME lost up to 118 gt; parked by ironspec: R1 is the core mechanism).
  Margin per k (F - arrival - max onset) with day-1's B3a F at +320 and B2's arrivals: k0 (+174)
  25 gt, k1 (+182) 17 gt, k2 (+192) **7 gt** (15 over p99.99).
- **Latest F** (a stuck cart must be past the exit before the slot it must make): F to the cart
  past the exit cell (6,-1,0) is **95-109 gt (median 96)** whatever F's time (118, 150 and 200 gt
  after the arrival, every napper failing, 16 seeds each: 16 of 16 clean; at 300 gt, 2 of 16: the
  next arrival, 400-500 gt later on the bench, meets the cart still inside). So **F at most D - 109
  gt**, D the latest time the loop lets a cart pass the exit; with day-1's D = +420 (its estimate:
  the hold kick at +470..482, exit to hold about 45 gt), F by +311; day-1's F at +328..+340 has a
  stuck cart past the exit at +423..+449: **up to 29 gt late if D = +420** (asked of day-1: its
  measured D). Day-1 measured D (seed 1, the first visit of each k, release held): exit to hold
  57, 72.5 and 88.5 gt for k0..k2, so D +413, +398 and +382 (382.5 at k2 after its k1 fix);
  ironspec's bar since: F→exit ≤ 56.5 gt at k2 with B3a's F (+320, +326 with the missed-golem
  corner) and the cart resting on its hold ≥ 5 gt before the kick. With the 0-block levers
  (the free cell a booster, the delay moved to the nap path, chain [4,2,1,1,1]) F→exit is
  **67-70 gt**: not met; the fast kick on F is next.
- **Corners:** k = 0..2 moves the arrival +190..+210 while F stays at +328..+340 in slot time: F
  118-150 gt after the arrival, covered by the bench's spread; the release held and pulsed both
  100 x 6 clean (a held release has napped carts on the slot 108-206 gt after the arrival, some
  before F: their return has turned V off, so F dies at the gate).

**The fault line: F dropped** (ironspec 2026-11-10: `siding-bench --drop-force K`: on visit K the
napper cannot sleep (its HOME erased and the bed's ticket taken, as R1's end state leaves it) and F
is not pulsed; the next visits are sent on schedule, as the loop would). 6 seeds x 1 night, release
held: **the siding jams.**
- The stuck visit's cart parks on the park rail and is never kicked (no nap and no F: nothing sets
  H); its villager stands on the free cell (1,0,2), on the bed line.
- The next visit's cart stops behind that villager on the booster (1,0,1), its rider never dropped,
  still there 2,000 gt later. After the fault no visit completed cleanly in any of the six seeds:
  most next carts stayed stuck; a few went out at last with their rider never dropped (58-1,543 gt
  after their arrival); one napper slept but its cart reached the slot without it.
- The stuck cart came out with its villager after 2,726-4,800 gt (3 seeds; what freed it is not
  traced) or stayed to the end of the run, 8,500-9,400 gt (3 seeds).
- The bulb's phase inverts: the next arrival toggles it off with a visit under way, so the next
  visits' F is blocked at the gate: a later F cannot clear the jam either.
- Recovery by hand: break the stuck cart free or reset the siding (the visit latch H set by a
  button on D2, an arrival through the entry to turn the bulb back) and re-run R1 for the villager
  that lost HOME. (The timer version had no such dependency: its own 220 gt timer.)
- **With the dropper latch for V** (2 x 16 seeds, F dropped on visit 3 and on visit 8): in 26 of
  32 seeds the next visit's F clears the jam: the stuck cart leaves with its villager 594-705 gt
  after its arrival, the next visit loses its nap (its rider never dropped; its cart goes out with
  it) and every visit after is clean. In 6 of 32 it does not (the stuck cart stays, or leaves
  after 2,428 gt; 4-5 visits fail). What still blocks (a trace, seed 3): the next cart has pushed
  in behind the stuck villager and stopped against it on the booster at z 1.76, the villager
  wedged at z 2.59; the next F sets H and the kick sends the stuck cart north, but it stops dead
  at z 3.38 against the wedged villager (moving 0.21 b/t, its box touching the villager's) and
  does not pick it up; nothing moves after. Not yet fixed. On the long-nap-path design 3 of 16
  seeds jam (13 recover within one visit; `--drop-force 3`, 16 seeds). **Why no pickup** (traced
  2026-11-10 22:14, seed 3; the villager's boarding cooldown is 0 and it rides nothing): the
  kicked cart meets it at 0.203 b/t. A cart picks up a mob within 0.2 of its box while it moves
  at least 0.1 b/t, but its own move stops at a pushable mob's box (a vehicle collides with
  pushable entities). The wedged villager stands 0.09 nearer the park than a woken napper (z 2.59
  against 2.50), so the cart's last step jumps the 0.2 band (gap 0.21, then blocked at contact):
  its speed is zeroed by the clipped move and the pickup test finds it not moving. A woken napper
  at 2.50 is met below 0.2 b/t and never skipped. A fix needs the empty cart under 0.2 b/t within
  a block of the free cell (without costing F's exit time), or the wedge prevented. Tried and
  rejected (2026-11-10 19:20):
  the slow run's two rails (1,0,4), (1,0,5) as boosters (so the kicked cart meets the villager
  fast): it breaks the normal path (0 of 16 seeds clean: arriving carts no longer settle on the
  park, riders lose HOME) and makes the jam worse (10 of 16 seeds never free the stuck cart).

**The fast forced exit: the long nap path (2026-11-10 21:30; ironspec's decision after path A's
cost; +8 blocks, 262):**
- **The limit it lifts:** the kick chain is shared. A nap needs drop to H (the nap path) + H to the
  kick + the kick to the free cell ≥ 60 gt (the boarding cooldown) plus margin; F needs F to H (8)
  + H to the kick + the kick to the exit. Subtracted: F's exit ≥ the re-board time - 18 + 20, so
  62-64 gt at least while the nap path is 26 gt, whatever the chain. Every shorter chain on that
  path broke the normal path (0 of 16 seeds: chains [2,0,0,0,0] 53-56 gt, [2,1,0,0,0] 55-58,
  [4,0,0,0,0] 57-60, dust 49-52); [4,1,1,1,1] 65-68 and [3,1,1,1,1] 63-66 pass (re-board min 63
  and 61).
- **What it is:** the S line from the observer four 4-tick repeaters long (south of the observer
  (0,0,5), (0,0,6), dust (0,0,7), west (-1,0,7), (-2,0,7), dust down x -3 to the old stair): the
  nap path 16 gt longer (H about 44 gt after the onset). R's reset later: its two repeaters
  (3,0,-1), (3,0,-2) at 4 ticks and the R line's dust at (2,0,-3), (1,0,-3) 4-tick repeaters (0
  blocks), so the wake's echo down the long S line reaches N1 while H is still set (with R as
  before it set H again after the return). The chain **[2,1,0,0,0]** (a 2-tick and a 1-tick
  repeater, three dust).
- **The other limit:** the hatch shuts at H and the villager wakes up to ~23 gt after it, so the
  kick must come at least ~10 gt after H for the napper to be up when the empty cart passes:
  [2,0,0,0,0] and [1,1,0,0,0] fail (7 of 24 seeds), [2,1,0,0,0] passes.
- **Results** (`siding-bench --wired`, F driven as day-1's scheduler at 118-150 gt after each
  arrival; redsim; arrivals and the release bench-driven):
  - F to the cart past the exit **55-58 gt** (median 56; 672 visits, every napper failing, 16 x 2;
    the same with F at 134 gt, k2's +6 corner); ready 166-201 gt after the arrival.
  - Margins against day-1's limits (with its +4 gt of D at k2): nominal F k0 25, k1 12.5, k2 8.5
    gt; the +6 corner k0 19, k1 6.5, k2 **2.5** (closed). Pick rule (ironspec): the larger of
    min(corner margin, re-board margin): [2,1,0,0,0] 2.5 against [4,0,0,0,0] 0.5.
  - Drop to re-board **min 69** (9 gt over the cooldown; was 65).
  - Normal visits 100 seeds x 6 nights, release held and pulsed: 100 of 100 clean each, 12,600
    visits, all re-boarded their own cart, 0 naps cut by F, ironspec's check unchanged (all 21
    holding HOME at all 1,400 checks, one nap per villager-night).
  - The tail, 100 x 25 nights, release held: 52,500 visits, every napper back in its own cart and
    released (0 stranded), 0 cut; onset p50 17, p99 39, p99.9 75, p99.99 111, max 121 gt (per-k
    sleep margins unchanged: k0 25, k1 17, k2 7).
  - HOME erased x 3 (16 x 2): 16 of 16 clean. Placements 24 x 2: the judge's, raw and built in
    order (348 cells, 8,900 gt) all clean. siding-check 0 golem spots (bench and alone files). The
    slot and exit cells unchanged (the rider rest cells too).
  - Faults (`--fault KIND:K`, 16 seeds): ARR missed or extra, R missed or extra, F doubled: 0
    visits lost each (R missed was 2 visits: H now resets after the echo).
  - The siding's T back to entry-bed: 140-226 gt after a nap visit's arrival, 86-88 gt after F
    on a visit sent out by F (visit +406..+414 with B3a's F), 44-54 gt after the cart came onto
    the exit branch (the cart then past the exit): about 250 gt before the next slot's earliest
    arrival (+174 a slot later).
- **Path A (the "napped since arrival" memory N) not taken:** 15-25 blocks and uncertain against
  this variant's 8.

**Fault injection at the stateful elements** (`--fault KIND:K`, 8-16 seeds, release held, visit
3; each pulse a redstone block for 4 gt beside the element): ARR missed (DB pulsed just after the
arrival), ARR extra (DA pulsed mid-visit), R extra (DB pulsed and the H latch reset mid-visit),
F doubled: 0 visits lost (the napper slept; V only gates F). R missed (DA pulsed and the H latch
set again after the return): the next visit loses its nap (H set with no visit: it goes straight
to the exit), then all clean: worst 2 visits to recover, 1 nap lost a seed.

## Unloads (farmopt and ironspec 2026-11-10: the brief's chunk alignment, X & 15 = 6, Z & 15 = 6)

- **The chunk lines the siding crosses** at day-1's paste offset (the siding's (-5,-3,-4) at the
  loop's (-25,-1,6): loop = siding + (-20, 2, 10)): the loop's x -23|-22 and z 9|10, i.e. the
  siding's **x -3|-2** and **z -1|0**. The others (x -7|-6 and 9|10; z -7|-6 and 25|26) miss its
  box (x -7..6, z -3..9) and its junction tracks (the entry row z 0 west of the entry, the exit and
  run-out east of x 6 on z 0). Four chunks:
  - west-north (x ≤ -3, z ≤ -1): the latch's N2, W2 and D2's start; the R line's west end; F's
    cell and the gate; the not-V line's west end;
  - east-north (x ≥ -2, z ≤ -1): V's droppers and comparator; the R line's east part and its
    repeaters; the slot's tripwire hook;
  - west-south (x ≤ -3, z ≥ 0): N1, W1, D1; the kick chain's x -6 column; the S line's stair; the
    approach west of x -2 (on the bench, the launcher);
  - east-south (x ≥ -2, z ≥ 0): the entry, the T, the bed and cage, the hatch and zombie, the bed
    line and park, K, the H line, the observer and the S line's start, the slot and the exit.
- **The partial unload** (`siding-bench --unload matrix:3`; tick-exact's act, anchored on the 26.3
  bridge at these lines, GO 244b9196d; the bench's world shifted 2 east so the lines fall as in
  the loop). Visit 3, 6 seeds x 10 visits, each line, each side dropped, that side at level 32
  (blocks tick, entities do not) or 33 (nothing ticks; the other side then at 32), at seven visit
  phases (arrival, nap onset, waking, 10 gt before F, kick, re-board, exit), for N = 3, 40, 240 and
  700 gt. Full table: `unload-partial.txt`.
  - N = 3: every case clean.
  - N = 40 and 240: clean except at the nap. Every case that freezes the bed's entities while
    blocks still tick (x lo 33, x hi 32, z lo 33, z hi 32; x hi 33 at 240) **jams for good**: all
    6 seeds, every later visit stuck (a hand reset). x lo 33 at 10 gt before F loses about 1 nap
    a seed and recovers within 2 visits: R is frozen in the west chunk, so H stays set; the next
    cart goes straight out and its R resets.
  - The north side alone (z lo 32: R, V, F's gate) is clean at every phase and every N, 700
    included.
  - N = 700 (past every timer, and past the next visit, which the bench still sends into the
    frozen part on schedule) jams in every other case. The loop's own tracks there would freeze
    too, so these overstate it.
- **Why the nap jams:** the napper is dropped about 17 gt after the arrival. Its 60-gt boarding
  cooldown counts its own ticks, so frozen with its chunk it ends N gt late, while H, the hatch and
  the kick keep the game's clock. The empty cart meets the woken napper on the free cell before
  the cooldown ends, does not pick it up, stops against it (z 3.29) and stays; each next cart stops
  behind. The re-board margin (9 gt) covers a freeze up to about 9 gt in that window.
- **The whole part unloaded** (`--unload whole-matrix:3`: every entity saved and back as a new one,
  `unload_all`/`load_all`; 6 seeds; the phases above plus the cart arriving, the napper dropped and
  asleep; table `unload-whole.txt`):
  - N = 3, 40 and 240: clean at every phase but the kick. Unloaded as the park rail powers (any
    N, 3 included) the cart leaves without its napper in 3 of 6 seeds, and the siding jams. The
    napper, asleep, comes back with a rebuilt brain (its sensors restart, tick-exact's load
    model), wakes after the empty cart has passed the free cell, and stands on the bed line.
  - N = 700: jams in 2-4 of 6 seeds at every phase. The visit after the reloaded one, sent as soon
    as the part is back, has a napper whose brain is fresh from the load: it wakes late (+104 gt),
    misses its cart, and the next carts stop behind it.
- **What they share:** the siding cannot recover from a napper left standing on the free cell. No
  later kick picks it up (blocked by the cart's last step or by the cooldown), and the next arrival
  stops behind it: the same end state as the dropped-F jam. ironspec's possible requirement (a
  reload-proof re-board: no cart leaves without its napper; a napper left out re-boards, or goes
  back with its cart, within a stated number of visits; the loop stays in order) needs a design
  change there. Not built. **On hold** (ironspec and farmopt 2026-11-10 22:58): tick-exact bridges
  the three cases first (its own bridged whole-farm unloads mid-visit, +250/+280, 4,000 gt, matched
  the game and every napper re-boarded). Their drives, in `faults/` (`siding-bench --launch
  --unload ... --drive-out`, the unload acts as drive lines; `web drive-check` holds both
  whole-part drives to the bench's run: 344 and 265 cart positions):
  `whole-kick-3-seed4.drive.txt`, `whole-asleep-700-seed1.drive.txt`,
  `partial-x-hi-32-nap-40-seed1.drive.txt` (the design's own coordinates, `--unload-shift 0,0`:
  chunk x 0, z -1..0 dropped, forced box x -1..0, z -1..0 set at t0, written in its header).
- **The bench as a fault runner** (farmopt's questions): it calls tick-exact's act itself
  (`reload::unload_some` / `unload_all` between two ticks; the world ticks on), so the freeze takes
  the bed's villager exactly as the act does (its chunk dropped). A whole-part unload pauses the
  bench's arrivals while the part is away and sends the next as soon as it is back; a partial one
  keeps them on schedule (none falls inside an absence of 240 gt or less: arrivals are 400-600 gt
  apart and each waits for the last cart to leave the siding). Unlike the loop, in the partial
  runs the bench's hold sits outside the forced box (its waiting riders frozen until sent).

**The one-chunk placement** (ironspec and farmopt 2026-11-10 22:58: day-1 pastes the siding at ox
in [-15,-13], oz in [13,16], inside one chunk; emulated with the bench's world shifted (8, 3), ox
-14, oz 13; arrivals launched so they cross the approach's line). The only lines left are the
junction tracks': `w` the approach's at the siding's x -9|-8, `e` the run-out's at 7|8.
`--unload chunk-matrix:3`, 6 seeds x 10 visits, visit 3, every phase (sent, arrival, nap, waking,
10 gt before F, kick, re-board, exit), N = 3, 40, 240; table `unload-chunk.txt`:
- The junction chunks alone frozen (w lo 32: the approach; e hi 32: the run-out): clean at every
  phase and N.
- The siding's own chunk at 32 (the edge one chunk further in, from either side: w hi 32 = e hi
  33, e lo 32 = w lo 33): clean at every phase except two. At the nap onset with N 40 or 240 it
  jams for good (the frozen cooldown, above). At the arrival with N 240 it loses one nap and
  recovers within a visit (F fires on time while the cart is frozen at the entry: the visit goes
  out unnapped).
- w hi 33 and e lo 33 would leave the far junction's chunk at 34 (its entities saved and
  unloaded): not modeled, not run.

## The give-up's first version, its own timer (2026-11-10; `--give-up timer`, superseded)

Without it a napper that cannot sleep parks its cart for good and the loop stalls. Built into the
wired siding, all of it north of the R line (z -1 to -12, y -5 to -1), in redsim:
- **ARR, the arrival:** the entry rail (0,0,0) is a detector rail (only arrivals pass it; its power
  also reaches the T rail beside it, which is powered at idle anyway). Its block below is a stone; a
  repeater north reads it, into a stone, dust under it, repeaters north at y -2 under the R line.
- **A, a visit under way:** an RS-NOR latch at y -2, the H latch's own shape turned (N_A1 (0,-2,-6)
  set from the south, its torch W_A1 = not A; N_A2 (3,-2,-8), its torch W_A2 = A), set by ARR, reset
  by R (a repeater north off the R line's corner (3,0,-3), dust along x 4 at y 0, down into N_A2's
  north face) or by G (a 4-tick repeater into its east face).
- **The bound:** day-1's two-stage comparator off-delay (`circuit timer long --k 3`, L = 220 gt,
  `carousel-scheduler/offdelay-k3.txt`) west of the latch, its frame moved by (-6,-3,-10), fed not A
  through three repeaters west and one north; held off 220 gt, its output stone A2 goes dark.
- **G = A2 dark AND A:** a wall torch (-15,-2,-10) on a stone X that a repeater off A2 powers, or not
  A (dust west from the timer's input line, a repeater into X). G is lit only while a visit is under
  way, and goes dark as soon as A resets.
- **G sets H:** dust down to y -4, east along z -5, south along x 1, a repeater north of a stone at
  (1,-3,-1), west through repeaters along z -1 into the H latch's D2 (-5,-3,-1). The second of them
  is a comparator subtracting the larger of H (the H-out dust south of it) and not A (a repeater
  from W_A1's south side): G reaches D2 only while H is unset and A still set. (a late return, its
  nap long done, fires G with H set and nothing reaches D2; a G fired just as R resets A dies there
  too, so G never sets H after R's reset).
- **G also resets A** (east along z -5, north along x 6, up into the 4-tick repeater): the timer
  refills and G goes out.
What it took (redsim traces): the timer's input line powered the stone X, and dust beside X fed the
line back into the timer's own input (the timer held itself; a repeater into X now); dust beside Ta
(which H powers strongly) fed H back into D2 (a stone and a repeater there now); built from the
generator's states, unpowered repeaters on the not-A line let G light once and set H (written
powered, their state at rest); and a G the timer fired just as the return reset A set H after R's
reset (the comparator). Its two torches unlit at rest (W_A2, G's) are the build's last steps,
after the timer's input repeater, G's line along z -5 and the glass caps: the timer's ring takes
about 12 gt to fill and holds 15 or 14 at rest depending on how it was filled, so it fills once,
whole, before the build ends (built at 1 gt a step, it ended at 14, unlike the file). A glass cap
on the timer's stone at z -12, within a golem's search of the bench's hold row (the viewer's replay
spawned a golem there).

Results, `siding-bench --wired` (redsim; the arrivals and the release bench-driven):
- normal visits with the give-up in place, 100 seeds x 6 nights: 100 of 100 clean, 12,600 visits,
  ironspec's check unchanged (all 21 holding HOME at all 1,400 dawn and midday checks, one nap per
  villager-night, ready 115-287 gt, every HOME loss reclaimed within 120 gt): G never reached D2
  in a normal visit; the placement checks below are run with it in place;
- **every napper failing** (`--day-visits`: every visit by day, when no villager sleeps), 16 seeds x 2
  days, 672 visits: 16 of 16 clean: every napper dropped, none slept, and every cart was back on the
  slot with its villager (the kick finds it on the free cell) 358-376 gt after its arrival and
  released; all 21 kept HOME = the bed at every check;
- **three nappers without a bed claim** (`--fail-no-home 3`: their HOME erased before each visit),
  16 seeds x 2 nights, 672 visits: every visit dropped, every cart on the slot with its villager
  within 371 gt and released; 586 naps (the other 18 every night) in 12 of 16 seeds; in 4 seeds one
  more villager lost HOME or missed a nap: a villager without HOME races the others for the bed's
  ticket when a napper releases it at its first REST tick (the failure itself, not the give-up).
  ironspec (2026-11-10): outside the contract (rule 1 keeps erases at 0; the bench prints them);
  recorded as **the known failure mode**: a villager that has lost HOME can win the bed's ticket in
  a napper's release-and-reclaim, and the napper then loses its HOME too. **The player's recovery
  step:** run R1's break-and-replace again for every villager without HOME (the bench's HOME checks
  name them).
- Cost: the siding by itself 234 -> 472 blocks (box 14x9x13 -> 23x10x22), build 19.8 -> 39.6
  minutes at middle prices (10 comparators: the timer's 9 and the gate).

## Settled as placed, and built in order (the brief's two checks; redsim, not yet the real game)

The wired file is written settled (`mobfarm siding --wired --out`, `placing::settle_design`): every
cell as the bench finds it after the settle. 65 cells changed: every dust in its settled shape and
power (the generator wrote dots at power 0) and the powered repeaters (the five 4-tick repeaters of
the delayed "not visit" line). Its `build: last -6,-3,-1 1,-2,6` line lists the two torches that are
off at rest as the build's last steps.

`siding-bench --wired --launch`, 24 seeds x 2 nights each (arrivals launched from rest on the bench's
launcher, the release placed as a block by the bench; since 14:30 no bed act, each villager once a
night, ironspec's HOME and nap checks in every run):
- the judge's placement: 24 of 24 seeds clean, 1,008 visits;
- settled as placed (`--placed-raw`, pasted raw, no settle): 24 of 24 clean, 1,008 visits;
- built in order (`--placed-in-order`, the priced gaps: 338 cells in 338 steps, 8,600 gt with the
  scheduler give-up; 557 cells, 13,700 gt with the timer one; 319 cells, 8,360 gt without): 24 of
  24 clean, 1,008 visits; the build ends exactly as the file (the night under way at the spawn is
  skipped: the bench's nights are the full ones after it);
- `--build-gap-sweep`: every gap from 1 to 40 gt passes (4 seeds x 1 night each, 84 visits a gap,
  every villager's HOME at every dawn and midday and one nap each).
Clean: every visit dropped, napped, woken, back in its own cart, ready on the slot and released;
all 21 holding HOME = the bed at every dawn and midday; each villager one nap a night.

What the build needed: the latch is bistable. In the default order its first source is W2, the H
torch (-6,-3,-1), placed unlit with its block not yet powered; the first neighbour update lit it, so
the build ended with a visit set and the first arrival went to the exit unnapped (59 cells unlike
the file). Placed last, N2 is already powered through D1 by W1, and W2 stays unlit. K, the kick
torch (1,-2,6), placed early, was lit by the first update; BK's power arrived later through dust
written at its settled power, which tells K nothing (no power change), so K stayed lit and the park
rail powered. Placed last, BK is powered already.

Drives for tick-exact: `carousel-siding-wired-raw.drive.txt` (`--placed-raw`) and
`carousel-siding-wired-built.drive.txt` (the build's `act t set` lines, then the bench's acts;
`# spawn 8600`), both `siding-bench --wired --launch --from 1 --to 1 --nights 1`. Besides `set`, the
bench's acts are `tp` (a cart onto the launcher, back to the hold), as in the viewer's drive (since
14:30 no `home`, `last-slept` or `poi-take`: HOME comes from the file's `port home:`), and the force
input's pulses as day-1's scheduler would send them (`set -5 -3 -4 redstone_block`, `air` 12 gt
later).

`mobfarm siding --wired --alone` writes the siding by itself for ironfarm's loop bench (no launcher,
approach, run-out, hold or carts): 234 blocks in x -7..6, y -4..4, z -3..9, and the dorm zombie.

## Ready time, the release held, and the exit speed (day-1's questions, 2026-11-10)

Bench-driven arrivals as before (teleported onto the approach at 0.15-1.1 b/t, visits 400-600 gt
apart), `siding-bench --wired`, 100 seeds x 6 nights, 15,490 visits a run, every one clean:
- **Released by the bench's pulses** (560-610 gt after the arrival): ready (napped, back in its own
  cart, at rest on the slot) p50 145, p90 229, p99 263, max 290 gt. The tail is the kick, not the
  walk or the nap: woken to re-boarded 69/155/191/225 gt (p50/p90/p99/max; the slowest 1%: 195 gt
  against 86 for all), the nap 25/33/35/35, and the napper wakes standing in the free cell. The kick
  waits while the slot is taken (the slot's tripwire is OR'd into it), and the bench's previous cart
  is often still waiting there for its release: visits come 400-600 gt apart, releases 560-610 gt
  after arrival.
- **The release held on** (`--release-held`): every visit napped, re-boarded its own cart and left;
  ready (on the slot with its villager) p50 111, p90 125, p99 141, max 225 gt; woken to re-boarded
  46/54/56/58. What is left of the tail is the lie-down: drop to asleep 0/0/9/115 gt (a few nappers
  walk round the bed first).
- **Exit speed** (stored, leaving the exit cell): with the pulses 0.281-0.436 b/t (median 0.286), 926
  of 15,490 over 0.36: exactly the carts that came to rest higher on the slot's slope (x 4.3-4.5);
  every cart at rest at x 4.7-4.8 leaves at 0.28-0.29. With the release held 0.536-0.719, every one
  over 0.36.
- **One slowing cell does not fix it.** A braked powered rail is a stop: it halves the speed every
  tick the cart is on it and stops it under 0.03 b/t. One just past the exit (`siding --exit-brake`,
  x 7) stopped 346 of 350 carts with the pulses and 356 of 356 with the release held (8 seeds x 1
  night). On the exit cell itself it is powered whenever the slot's tripwire fires (the stone under
  the tripwire line's dust at (6,-1,1) is beside it), and boosted the carts to 0.66-0.84 b/t with the
  release held. An uphill cell takes off only the slope's gravity, 0.0078 b/t per tick on it (the
  game's rule; not benched).

## What failed on the way (and why)

- **Slow arrivals stopped short of the cobblestone.** The kick then lost the park rail's length,
  and the empty cart met the napper under 0.1 b/t (no pickup). The always-on booster at (1,0,1)
  fixed it for every speed from 0.10 to 1.2.
- **The held riders were calm at REST.** They gave up HOME after 1200 gt out of reach and took
  the bed's one ticket, or backed off 40-400 gt after failed paths. Hold zombies fixed it. A
  released rider teleported back within 16 of the bed while the next napper slept forgot the bed
  for good.
- **The circuit (three bugs, all fixed):**
  - The detector rail beside the T turned it under the cart.
  - Dust beside a block a repeater powered strongly fed the visit line back to itself and latched
    it.
  - Moving the dorm zombie one block east put the sleeping napper's sight line on the shut
    hatch's top, and a nap never ended.
