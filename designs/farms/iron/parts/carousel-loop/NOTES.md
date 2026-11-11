# The single-lane carousel loop (being built)

vjeux, Nov 10 05:58: "a single lane where the villagers are riding on back to back with a
mechanism to release them 3 by 3". The brief: designs/briefs/farm-iron-carousel.md, the top
section. This folder holds the loop as it is built.

## What is real (vjeux 07:29: "You can't say it's working when nothing is wired")

| part | in this file | controls | plays on its own | missing |
|---|---|---|---|---|
| queue register (18 slots) | track built | bench-driven here; day-1's controller wires every control in its generated copy (explore/day1-sched-wip d9b20f773) | yes, in redsim, fully wired (3 seeds x 60 slots clean); real game: tick-exact bridging | the wired copy merged into this folder; the real-game bridge |
| release of the front 3 | track built | as above | as above | as above |
| stand (3 slots) | track built, a sketch (farmopt owns the real stand; a stop-based stand is being built) | its brake a real detector rail on the release run plus a delay, its departures timers: wired in day-1's copy, bench-driven here | as above (the 2-tick catch) | farmopt's stand and pad; a catch with slack (the stop-based stand) |
| return to the queue's tail | track built | timers +100/+118/+120: wired in day-1's copy, bench-driven here | as above | as above |
| zombies (PANIC) | 8 posts built | none needed | yes (bench; the game agreed tick for tick on the first 7) | the pad's own stand zombies (farmopt) |
| bed siding (naps) | not in this file (farmopt's is wired separately) | | | the two junctions |
| golem-floor gate (static) | the loop's geometry: no valid golem floor in any rider's search from any of the 116 rail cells (`queue-loop-spots`: 0 cells) | none needed | yes | the pad's own floor and the siding (farmopt) |

"Clean" below always means with bench-driven controls unless it says wired.

members.tsv (the site's list) holds only the wired loop, `carousel-single-lane-loop-wired.txt`; the
wip file is the bench's scene (its 21 controls are glass blocks the bench switches), kept here for
the benches and the generator, not a design to build.

## The queue: a sawtooth shift register

Each queue slot is one 3-cell period: a powered rail sloping down (the cart's resting cell, braked
while unpowered), a plain rail sloping up, a plain flat peak (the next slope's sturdy high side).
One power pulse on every slope (7..12 gt) rolls every cart down, up, over the peak and onto the
next braked slope: every cart one slot on, the front one out. A cart starting lower arrives slower
and stops sooner, so the resting offsets converge (0.32 of the cell down the slope): no drift. The
carts sit 3 cells apart, outside the 0.2 push range: no creep, no bounce, no momentum build-up.
tick-exact bridged the register's one-slot moves against the real server (0 differences).

## The loop (`mctas mobfarm queue-loop --detector --boost-first 8 --tail-desc --short-depart --zombie-every 8`)

Top view, the pad (farmopt's) south of z -1:

- **Stand** (sketch) along +x at z 0, S3's slope at x -1, S2 x 2, S1 x 5: catch slots, i.e.
  sawtooth slots whose preceding flat cell (the run's last cell, S3's peak, S2's peak) is a
  powered rail linked to the slope (a powered rail takes its connected neighbour's power).
- **Release run** south along x -3 from the queue's head (z 19) into the stand's west end; 8
  boosters right after the head (the carts are capped at 0.4 b/t anyway; the boosters raise the
  train's speed into the stand, which widens the catch's window from 1 to 2 ticks); a detector
  rail at (-3,1,4).
- **Departure** from the stand's east end: north along x 8 right after S1's peak, up 3 blocks at
  z 16..19, west along z 20 at y 4 into the queue's tail.
- **Queue** north of z 20 in two rows 3 apart: row A (the tail) north along x 0, 7 slots; a turn
  slot (row A's last peak is a curve, the slot runs west, its peak a curve again) so no queue move
  is longer than another; row B (the head) south along x -3, 10 slots.
- **Tail slots 0-2 are descending deep slots** (after 3 flat cells at y 4): a braked slope linked
  to the slot's own (two blocks of braked descent), one plain slope up, a peak one block lower than
  the last slot's (slot 2's peak is the queue's level). A fast returning cart stops on one of the
  two braked slopes, never on a flat cell; a queue move climbs one block out of a tail slot, as
  fast as out of any sawtooth slot (the earlier level deep slots climbed two blocks and crawled
  over the peak, ~22 gt per move: the next pulse caught them mid-move, so the advance needed
  pulses exactly 20 gt apart in every slot).
- **Zombie posts**: a zombie in a boat on glass at (3,3,5+8k), k = 0..6, and one at (-6,3,21)
  west of the queue's head (the raised jog's supports hide the head from the post at z 21),
  leaves over each; every rider within 8 blocks and in sight of one, all along the loop.

`mctas mobfarm queue-loop` prints the control cells (each the block under a powered slope); the
tail's are (0,1,25), (0,0,29), (0,-1,33).

Per slot (t0 = the release start = golem pulse + 4 in day-1's controller; the times day-1's
controller produces):
- the release group (the queue's front 3 slopes) on for 36 gt from t0: the 3 leave as a train;
- the stand's batch leaves one slot at a time: S1 on at t0+6, S2 +12, S3 +18 (6 gt apart; with 12,
  S3's cart reached the tail at ~t0+140, after the next slot's first advance pulse);
- the stand braked (all 3 slopes) 28 gt after the detector rail switches on, in redstone terms
  (the bench's +27: the bench applies a change after the tick, a repeater during it, so bench +N
  is redstone +(N+1); day-1 found it in its wired run);
- the queue advances 3 slots: pulses at t0+38, +58, +78, 12 gt each, slots 3-17 on the same tick,
  the 3 tail slots 2 gt later (day-1's wiring; `--skew 2,2,2`);
- the queue's tail opens at t0+90 (slots 1-2) and +92 (slot 0);
- the returning batch fills the tail front first, by timers from its departure: slot 2 braked at
  +100 (works +92..+114 at least), slot 1 at +118 (works +116..+120), slot 0 at +120 (works
  +118..+122); the batch is at rest in the tail by +131..+132, 8 gt before the tail's first
  advance pulse (+140), checked at rest (`--tail-check-rest`), as the wired run checks it.

## Results

- Bench-driven, position-triggered stand brake, tail timers +110/+116 (3 boosters): 100 seeds x
  1,440 slots (6 days each), 0 problems.
- Every control a timer or the detector (3 boosters, bench +28; day-1's latencies): 100 seeds x
  1,440 slots, 0 problems.
- 8 boosters: the stand works at bench +26 and +27 (redstone +27 and +28), 3 seeds x 300 slots
  each with day-1's 36/12-gt pulses; +25 and +28 fail.
- Every slot: the queue releases exactly its front 3; the batch settles at the stand at t0+81
  (golem+85); the previous batch rests in the queue's tail before the next advance.
- PANIC: with the zombie posts every rider panics 100.00% of deep-night ticks (day time
  13,670..22,330), 3 seeds x 720 slots; 7 of 7 zombies alive at the end; riders all in their own
  carts. Without the posts: 0%. Posts every 16: 72.6%. This file (8 posts): 100.00%, 1 seed x 100
  slots, 8 of 8 zombies alive (without the post at the head: 94.77%, the head's riders calm).
- Golem spawns: a villager riding anywhere on the loop's 116 rail cells finds 0 golem spawn cells
  (`queue-loop-spots`, redsim's copy of the game's LEGACY_IRON_GOLEM search).
- tick-exact bridged the stand catch against the 26.3 server: the game matches the bench tick for
  tick, the window included (the 1-tick catch, then the 2-tick one: carousel-catch2, GO
  ea7960400, 15,750 of 15,750 cart-ticks equal at +26 and +27). The riders' PANIC near the zombie
  posts is the same in the game, tick for tick.
- This file, day-1's times (tail +2, departures 6 apart, tail brakes +100/+118/+120, the
  at-rest check): 3 seeds x 200 slots clean at bench +26 and +27 (redstone +27/+28); 100 seeds x
  1,440 slots (6 days each) clean at bench +27, with day-1's v1 times (release 36, 12-gt pulses)
  and again with its v2 times (release 34, 10-gt pulses, slot 2's tail brake at +98).
- day-1's fully wired run on the previous file (9bd8f2ff9, its redstone for every control, 60 slots
  x 3 seeds): the release group holds 3 and the stand holds 3 on every slot, no rider problem; the
  one failure was the returning batch still settling when the tail's first advance came (fixed
  here: departures 6 apart, the departure 1 cell shorter each way).

## Tolerance lines (`queue-loop-run --sweep <control>|all [--corners]`)

Each timed control shifted one gt at a time, early and late, to the first failure (2 seeds x 120
slots per step, bench-driven controls, day-1's times):
- The previous file (level deep tail slots, ab96cd355): release end -10..+22, S1 -4..+4, S2
  -9..+3, S3 -8..+13, tail brakes 1-2 -4..+4 and 0 -8..+8; FLAGGED: every advance pulse's time
  0..+1 at best (the deep slots' slow moves, above), the stand brake -1..0, the release group's
  entry speed -0.00..+0.58 b/t. day-1's wiring delivered the pulses up to 10 gt apart from slot
  to slot (head first) and the loop broke at slot 4-6, in day-1's wired run and in this bench.
- This file, with a stand that always delivers (`--ideal-stand-at 86`, a diagnostic): passes
  day-1's skew and +-1 gt on each pulse alone. With the real 2-tick catch and day-1's skew it
  misses once in ~30 slots: the skew moves the release group's resting offsets a little, and the
  catch has no slack for it. Hence day-1 puts every queue control on the same tick.
- With the real 2-tick catch (9bd8f2ff9's file, day-1's times): every advance line FLAGGED (0..+1
  at best), the stand brake -1..0, the entry speed +0.00..+0.58 b/t; both corners fail. Every
  flagged line is the catch's: the advance's timing sets where the release group's carts rest,
  that sets the train's stored speed into the stand, and the catch cannot absorb it.
- **The no-catch table** (this file, a stand that always delivers, the returning batch checked at
  rest, day-1's v1 times; the target for the stop-based stand): release end -10..+16 (the sweep's
  edge), advance 1 time -7..+2 / width -5..+2, advance 2 time -2..+1 / width -3..0 (FLAGGED late,
  12-gt pulses), advance 3 time -1..+1 / width -5..+1, S1 -6..+3, S2 -2..+6, S3 -3..+6, tail slot 1
  -3..+2, tail slot 0 -2..+3, tail slot 2 +92..+114 passes; both corners (every control at its
  early limit together, then at its late limit) pass. With 10-gt advance pulses the widths pass
  -3..+4, -1..+2, -3..+3 and the times -7..+3, -3..+2, -2..+1 (no flag).
- **Again with day-1's v2 times** (release 34, 10-gt pulses, departures 6 apart, tail brakes
  +98/+118/+120) and the bench's controls as the OR of their lines (a release window running into
  the first advance no longer cuts it short): release end -8..+16 (the edge), advance 1 time -7..+3
  / width -3..+4, advance 2 time -3..+2 / width -1..+2, advance 3 time -2..+1 / width -3..+3, S1
  -6..+3, S2 -2..+6, S3 -3..+6, tail slot 1 -3..+2, tail slot 0 -2..+3. The early corner passes;
  **the late corner fails**: its smallest failing sets are {advance 1 +3 gt late and +4 wide,
  advance 2 +2 wide, advance 3 +3 wide} and {advance 2 +2 wide, advance 3 +3 wide, S1-S3 late}.
  Shifted together: the 3 advance widths pass -1..+2 (not their one-at-a-time -3..+4), the 3
  advance times pass -2..+2, S1-S3 pass -2..+3 (-3 and +4 fail). day-1's pulses are exact
  redstone, 10 gt each; these joint numbers are the margins to quote.
- **Entry speed** (a kick along each release cart's slope as the release opens, every control as
  timed): +0.00..+0.58 b/t passes, FLAGGED on the slow side: a cart drifting backward at the
  release is pushed backward by its powered slope (a powered rail boosts a moving cart the way it
  moves) and rolls back over the peak. The corners with both speed extremes: the early corner
  passes at +0.00 and +0.58, the late corner fails at both.
- **The release stagger** (farmopt's pocket variant releases the group's slots s gt apart, the
  front first): the back cart starts at +2s and needs ~26 gt to cross the front slot's slope, and
  all three slots go off at release_for, so release_for >= 2s + 26. On the pocket loop (2 seeds x
  30 slots): s = 3 release-end -2..+12, s = 4 +0..+12 (FLAGGED, no slack), s = 5 fails at 34 (the
  back cart meets the braked front slope, drops from 0.53 to 0.08 b/t and the batch arrives short)
  and passes at 40 and 44; s = 6 and 7 pass by luck (they hit the braked slope differently).
  --stagger-off does not help: it delays the slot released last, but all three carts cross the
  front slot. A wired controller holds the front slot's window to >= 2s + 30.
- **The nap visits' couplings** (the joint table's visit rows; measured in "The nap visits wired"
  below): a missed golem at the visit's release i+4 moves the hold kick +6 against an unmoved late
  edge, leaving >= 13 gt of drop window (k1 at 0.21 b/t binding; k0 >= 26, k2 >= 28); the junction
  lines divert -2..+6, kick -22..+18, T-set -17..+120 (bench controls), the early corner failing on
  {divert -2, T-set -17} (the previous visit's cart not yet past the T's).

## The bed siding's merge: the side drop (day-1's (b))

`queue-loop-run --side-drop C,SLOT,D,V[,PARK]` (bench-driven): a cart parked off the loop comes
back by rolling off a side rail beside a queue slot's braked slope (3 plain rails west of it at
the peaks' height, a stone stop east; placed with their updates, they reshape no loop cell). With
the stand forced to deliver (the queue alone):
- into slots 12, 13 and 14 during the long rest (the 3rd advance's end, ~+97, to the next +38): it
  works at 0.10..0.36 b/t, resting 0.60..0.80 down the slope 12..26 gt after rolling off; as late
  as +24 of the next slot (0.29..0.36 b/t), +18 (0.15); 0.05 is too slow, 0.38 rests 0.83 down and
  does not climb out at the next pulse;
- staged on a braked staging slope and kicked: lands 0.81..0.88 down, pass or fail with where the
  cart stopped on the staging slope; a kick block under the staging slope also powers the slot's
  own rail beside it;
- into the release group at the release's start: fails (the cart, landing moving onto a powered
  slope, outruns the train); 4..8 gt before it: works at 0.2 b/t in a ~5 gt window.
- A cart's hole visits, in the long rests, the slots of one class mod 3 (the batch's front cart
  2, 5, 8, 11, 14; middle 1, 4, ..., 13; back 0, 3, ..., 12): day-1's plan has three drop rails,
  slots 12, 13, 14.

## Nap visits on day-1's layout (bench-driven controls; R1's shared bed from the file)

`queue-loop-run F --wired=-7,-1,5 --visit S,K,REL`: during slot S the stand's cart K goes to the
bed siding (the divert on for 10 gt from golem +16+10K, T1/T2 set for its drop slot 14-K, the
siding released at +REL), and the bench reports the divert rail, the riders holding a bed and every
claim, the sleep ticks, the roll-off from the drop rail, the slot it rests in and its batch's order
before slot S+5's first advance. `--visits-nightly EVERY,REL[,FROM]`: 21 visits a night (every
EVERY slots from day time FROM, the j-th taking the stand's cart j div 7), checked besides: every
rider asleep once a night, every rider's HOME at each dawn and midday as at the start (ironspec's
check, 100 seeds x 6 nights when the layout is ready). Bench-driven: the four controls (divert, T1,
T2, release) are switched by the bench; nothing in the file drives them yet. The bench never
assigns a bed: the riders hold one only as the design states it (`port home:`, R1's shared claim
as loaded memories, no ticket taken).

On day-1's b51e4c875 layout (the pocket loop, day-1's controller, the siding pasted at (-20,2,10)),
S30 (night):
- the divert works: the corner T (9,-3,0) is north_west while its control is on, south_west after;
- **without `port home:`** the first rider to find a path takes the bed's one ticket (e14, during
  the settle, from queue slot 14 ~24 blocks away) and keeps it (panicking riders never run REST's
  give-up): only e14 sleeps; K1's and K2's carts stay at the bed spot and the loop breaks at S+6;
- **with `port home: -20 2 12`** (the only change): all 21 riders hold HOME = the bed throughout,
  no rider takes the ticket, and K0/K1/K2 sleep +211..+225, +190..+209, +200..+223;
- the exit route takes ~230 gt (0.29 b/t leaving the exit slot, 0.105 at the drop end): past the
  drop window; it needs <= ~200 gt and 0.10-0.36 b/t at the drop end (day-1: a booster after the
  exit and a longer flat run, coming);
- the 5-booster entry climb brings carts in at 0.84 (front) and ~1.1 b/t (middle), within the
  siding's tested 0.10-1.2.

The drop window on day-1's pocket loop (side drop, wired, slots 12-14): roll-offs from +88 (the
earliest tried) to ~+130 of the drop slot's long rest (later at faster speeds, to +134-138), at
0.10-0.35 b/t. The arrival speed sets the rest depth: 0.10 b/t -> 0.40 into the cell, 0.20 -> 0.32,
0.30 -> 0.24, 0.35 -> 0.21, 0.40 -> 0.16, 0.45 -> 0.12 (normal carts rest 0.5-0.66); from 0.16 the
cart crawls over the next peak at 0.09 b/t and makes or misses the 2nd advance by a tick or two
(on b223f27b3 two visits identical to that tick: S17 K0 made it, S21 K0 stopped a slot short and
its batch left with 2).

Day-1's exit routes, measured (bench controls, S30, siding release -> roll-off; slots 14 / 13 / 12):
| commit | the route | T_exit (gt) | arrival (b/t) |
|---|---|---|---|
| a60c087da | booster, drag loop | 149 / 193 / 199-223 | 0.38 / 0.32 / 0.27-0.34 |
| b223f27b3 | booster, no loop | 101 / 124 / 149 | 0.42 / 0.39 / 0.36 |
| 35a46f6ae | a hold before T1 (kick -> roll-off) | 74 / 112 / ~146 | 0.26 / 0.24 / ~0.19 |
With the hold the cart is ready on it at +330..+351 and stops 0.72 into the braked cell from 0.64
b/t; slot 12 still misses the window (kick +400 rolls off at +546). Next: a hold on each drop line,
~3 rails before its end (T_kick ~15-20 gt for all three).

### The nap visits wired (day-1's controller drives every junction; `--visits-observe`)

Day-1's 9062078a8 wired-stand file (the pocket loop, day-1's controller with the stagger, farmopt's
wired stand, the gated siding, a hold on each drop line, the night window from an inverted detector
at 11, the night-close race latched), 100 seeds x 1,440 slots (6 nights each), the bench writing no
control: **95 of 100 seeds clean**; 12,877 visits, every napper back in its own slot; roll-off speeds
(min..max, median, 99th pct) cart 0 0.29..0.36 (0.29, 0.36), cart 1 0.21..0.28 (0.28, 0.28), cart 2
0.29; 595 of 600 nights with every rider asleep; the pocket stand clean over 143,900 slots. The 5
failures are all night 1 (seeds 14, 37, 45, 64, 76: rider e9 never asleep): each seed starts at its
own day time, and at some phases the first night after power-on fits only 20 visits (the 21st falls
after the window closes; the first visit lags the window's opening by ~7 slots); nights 2-6 are
complete in every seed (day-1's: startup gives every villager its night-1 visit, or states the cost).

**The drop's speed limit per slot** (side drop on 7d947146b, 2 seeds x 5 roll-off times +100..+124):
slots 12 and 14 pass 0.30..0.44 b/t (the grid's top); slot 13 passes to 0.36 and fails 10 of 10 from
0.37 (its control pulses and drop line are the same as 12's and 14's; the cause is open). So a 0.36
roll-off into slot 13 has no speed margin (day-1 keeps k1's rest point, 0.28).

**The drop's late edge** (side drop on 9062078a8, the last clean roll-off): ~+546 at 0.21 b/t into
slot 13, >= +546 at 0.29 into 12 and 14, +539 at 0.36 into 14 (visit-relative).

**A missed golem at release i+4** (`--golem-miss`): the kick moves +6, the drop's late edge (i+5's
first advance) stays: roll-offs k0 +514 -> +520, k1 +527 -> +533 (0.21 b/t), k2 +512 -> +518, every
napper back in its own slot, the loop recovers within the slot (the slot-start snapshot catches the
late slot's third advance mid-move). Drop window left at that corner: k0 >= 26 gt, **k1 ~13 gt (the
binding case)**, k2 >= 28 gt; a 0.36 roll-off >= 27 gt.

**The junction timings** (bench-driven on 7d947146b's physical file, nightly visits k 0, 1, 2, 0 so
T1/T2 switch between consecutive visits' lines, 2 seeds x 48 slots): divert -2..+6 gt, kick
-22..+18 gt (from +470), T-set -17..+120 gt (the sweep's edge; from the divert +1). The early corner
fails on {divert -2, T-set -17} together: the T's switched before the previous visit's cart had
passed them (the coupling "the previous visit's cart clears the T's before the next T-set"); the
late corner passes. Joint margins (both shifted): divert -2..+6 with the T-set at -15..+120 (-17
fails only with the divert at -2), kick -22..+18 with the divert at either limit (divert +6 with
kick +18 and divert -2 with kick -22 and T-set -12 pass). Day-1's wired values sit inside: divert at
+2 (golem +18), T-set at the visit's +136 (+99..+119 after the bench's divert + 1), kick at +0
(+470..482; +6 with a missed golem). The coupling "the previous visit's cart clears the T's before
they switch for the next visit", measured by logging T1's and T2's rail shapes against each
visiting cart's first tick on T1: on day-1's wired file the next switch comes 171..192 gt after a
cart passes T1 and the last switch before it was 199..222 gt earlier (one night, seed 1, 56
switches); with the bench's T-set at the divert + 1, 68..99 gt after.

A T rail reshapes only on an update from a signal source, and a placed block updates its
neighbours as the old block (vanilla's `blockUpdated(pos, oldState.getBlock())`, as redsim): a
redstone block placed over glass under a T leaves it as it was. The bench switches its T's from a
target block (full, a signal source, no power); real wiring (dust, repeaters, torches powering the
support) switches them as expected.

## Wired

day-1 wired its release controller and all 21 control lines into this file:
`carousel-single-lane-loop-wired.txt` in this folder (on explore/day1-counter; v2 at 94fa2b6d9),
generated by `mctas circuit memory release-gen --out <that file> --loop
carousel-single-lane-loop-wip.txt`. Fully wired, the golem pulse the only input:
- v1 (12-gt advance pulses): `queue-loop-run --wired=-7,-1,5 --tail-check-at 43` clean on 10 seeds
  x 300 slots and 3 x 60 (the returning batch checked at rest); tick-exact bridged it against the
  26.3 server: 60 slots, 0 differences in carts, riders, zombies and every redstone component
  (PANIC 98.1% of night rider-ticks in both), placed the judge's way and as a raw paste with the
  hopper's stock merged (its build step: 23 stone in the hopper at (-5,-1,12)).
- v2 (10-gt advance pulses, release group 34 gt, every cell written settled): 10 seeds x 300 slots
  clean; `--placed-all` 3 x 60 clean placed the judge's way, as a raw paste and built in order
  (each cell's priced gap: 20 gt, 40 gt at a place:edge cell); tick-exact bridged it against the
  26.3 server placed as written (60 slots, 0 differences) and built in order at 20 gt a placement
  (the build and the run, 27,920 ticks, 0 differences; all 1,260 slot-end positions match):
  recorded scenes carousel-wired-v2 and carousel-wired-built (GO 8171a0da2).
- Its flag: the stand's 2-tick catch.

## Placement checks (the brief's "settled as placed, and built in order")

`queue-loop-run --placed-all` runs the judge's placement, the raw paste (`--placed-raw`: blocks raw,
block entities merged as `data merge`, no shape updates, no settle) and the build in order
(`--placed-in-order [GAP]`: the world empty, each cell placed with the game's updates in the
default order, each cell's priced gap or GAP for all, then the carts; cells the build leaves
different from the file are listed); `--build-gap-sweep` builds at every gap from 1 gt to the
longest priced one (40 gt). The bench-driven loop file: clean all three ways, and every gap from 1
to 40 gt passes; day-1's v2 wired file: the same (every gap from 1 to 40 gt, 2 seeds x 20 slots
each). The first build in order failed: the release run's boosters popped off, their
redstone-block supports placed after the rails; the default order now places any block that holds
up a rail or a component first. Then day-1's drop ends: placed by (y, x, z), a drop end (-4,1,z) went
down before the slot slope's peak, and the slope (-3,0,z), with one link down, took the drop end and
turned ascending_west. Rails now go down only when no rail already down could take them as a link it
was not meant to have (a rail with both its own links down is fixed); a design's `build: last
x,y,z ...` cells go after every other; the check counts the differing cells by kind and lists those
that differ in more than power. A T junction with no driver can still come out either way built in
order (day-1's bench-cell files); a driven T is set by its driver's updates.

## Riders' safety: the zombie posts' reach

A zombie riding a boat attacks over the boat's box (1.375 wide) grown 0.83 sideways: a rider (0.6
wide) is hit when its centre comes within 1.82 of the post on both axes while their heights overlap
(rails at y 1..4, the zombie at y 3). The --tail-desc return's jog (y 4 along z 20) passed inside the
post at z 21: on day-1's pocket loop (851a583a9) rider e0 took 3 damage every 7 laps at
(5.03,4.06,20.5), always at slot +1, and died at slot 49 (with or without visits). The generator now
moves a post in reach north a cell at a time (up to 4) or drops it: z 21 -> z 23. Pocket loop after:
3 seeds x 240 slots clean, no rider hurt, PANIC 98.65/99.03% (98.70/99.13% before). Every run now
reports each rider's damage and any rider gone from the world (a problem). The published wired v2
file has the post at z 21: on its own timing no rider is ever hurt (10 seeds x 1,440 slots: all
clean, 0 damage events), but a timing change could expose it; its next revision should take the
moved post.

## Golem spawn cells off the pad (open on the wired files)

A villager that has slept summons an iron golem when it panics with two more such villagers near
(within 10). The loop's riders panic all night by design, so once nappers ride it (the nap
visits), any golem spawn cell within reach of the track spawns golems off the pad. `mctas mobfarm
queue-loop-spots F` lists them (none may exist away from the pad): this part's generated loop
files have 0 (the bench file, the pocket base); **main's published wired v2 has 30**, all on its
wiring's blocks (the control lines at x -7, the cells at x 8, the release-run controls at x -4..-2,
z -3 by the stand, y 2 at z 0..4); day-1's wired night file (1152b0e38) has 24, three of them its
drop-line stone stops. There a golem spawned at slot 33 +59 on the stop (-2,1,31), walked beside
tail slot 2 and pushed the resting front cart back up its braked slope; the next advance boosted it
backward into the next cart and the tail jammed (slot 35). The loop benches without visits never
showed it: their riders never sleep. The bench now makes any iron golem in a loop run a problem.
Fix (day-1, the wiring's owner): glass for those blocks or glass covers, then queue-loop-spots 0
on the file as published.

## The stand catch's window (open)

The release train runs 3 cells apart, the stand's period. A braked catch cell slows each cart
over ~1 block, so where it stops depends on where it was when the brake came: the band that
leaves it in its slope is ~0.4 cell, one or two ticks of the train's travel. Tried and measured:
- a detector per slot instead of one: the same window;
- a deep (self-centering) stand, each slot two braked slopes and two plain slopes up: tolerant
  but 15 cells long, the batch settles at t0+93, too late for the slot at golem+100;
- a descending stand (each slot one block lower): tolerant catch, but the departing carts leave
  at different speeds and the returning batch misses the tail's window;
- a staggered release (train wider apart): no clean setting found;
- more release-run boosters: 1 tick -> 2 ticks (in the file now).

## The re-packing tail: admission control (escG6L, bench and loop; simulator only)

The tail takes the stand's 3-cart batches back into the register one cart per advance. The
candidate (ironspec Nov 11) is admission control: `queue-loop --tail-esc 6 --esc-adm --adm-ledge`.
In order: a 1-block ledge entry (a declared flight), 6 plain queue slopes, G0 (a fence gate over
P's upper end), P (a booster slope held by G_P), G_P, R and slot 0 at z 33. A short G0 pulse
admits exactly one cart: a cart already inside a gate's box when it shuts moves on out (the
gate-box rule, bridged in the game by tick-exact, 846ae9c28).
- **Timing:** G_P 2 gt at advance +6. G0 2 gt after G_P opens, for 4 gt, on advances 1 and 2. On
  advance 3, G0 opens 10 gt after G_P (`--adm-k2 10`); its next G_P is 60 gt away.
- **Why advance 3 opens later:** the queue's last cart, with nothing behind it, bounces back about
  0.14 when its predecessor leaves. It then rolls up from rest and reaches G0 about 23 gt later,
  against 20-gt openings.
- **Bench bands** (tail-bench, 100 seeds per row):
  - DL 2–3, W 3–5, K2 8–14, G_P 1–2, all 16 corners;
  - 0 holes in normal flow, the 4-group and the 3/3 group;
  - 0 doubles, clearance ≥ 0.081, cycle margin 6–7 gt;
  - 0 merges, falls, backward ticks, swaps and derails.
- **5–6 groups:** 0–15 holes per 100 seeds (capacity, open against the stand capping its groups).
- **The loop:** queue-loop-run drives G0 and P only since d4058c9a5. **Every esc_adm loop run
  before that commit is void** (G0 stayed shut, P unpowered). ironspec's buffer results were bench
  runs.
- **First loop run** (30 cycles, seed 1, K2 10): 21/21 carts, 0 merges, 0 derails. One hole, at
  slot 26.
  - **Cause:** the queue's last cart reached G0 rolling just as it opened. It was admitted at
    speed, bounced off G_P, and P (always on) boosted it backward: 7 ticks, a hard fail. It went
    back through G0, still inside G0's box, and up the queue.
  - **Bench replay:** the loop's arrival stream (`queue-loop-run --arrivals-out`, replayed by
    `tail-bench --arrivals`) does not reproduce it. The loop's third cart stalls where the bench's
    follows, so the riders or carts differ. Open.
  - **Next:** a braked P (powered only in G_P's window) damps every bounce. Its bands are being
    measured.
- **Ruled out:**
  - a booster on the last queue slope (boosts the follower backward);
  - 10 queue slopes (merges);
  - the ledge-free entry (merges);
  - the latched G0 with a braked P;
  - the short entry (carts run off west);
  - the front on a flat booster against G0: the pressed cart doesn't restart (13 of 36 openings),
    and carts move backward on the booster.
- **Checks every run makes:**
  - declared flights (`mobfarm::flight`: the ledge exempt only if the cart lands right);
  - declared states (`mobfarm::declared`: boosters powered, braked rails unpowered, gates shut, at
    tick 0 and t 20);
  - merged carts (`mobfarm::merge`).

## What did not work (same tools)

- flat brakes, carts 1.0 apart: one release dragged 10 carts out (push momentum);
- flat brakes 1.25 apart: holds and releases exactly 3, but a flat queue cannot advance;
- a gravity staircase: lockstep but drifting, no reference, and 21 tall;
- a fast train caught by plain sawtooth slots settles 0.8..1.0 down the slope and cannot climb
  out at its next departure;
- a 1-block ledge before a catch: lands too slow (~0.12 b/t), carts crawl and bump at the edge;
- V slots at the queue's tail (slope down, flat valley, slope up on its own line): carts stranded
  on the flat valley, or crested from the top of the slope up when it powered;
- a U-turn inside the queue (2 or 3 cells, with or without a booster): the cart taking it moves
  ~25 gt where the others take ~14, is carried by the next pulse, settles deep and crawls;
- the stand at the far end of the release run (return back along it): the return took ~130 gt,
  outside the tail's window;
- the queue in one straight row with the return round it: the return took ~235 gt, at the edge
  of its window;
- the stand's batch leaving like a queue advance (3 pulses): the third cart reaches the tail too
  late.
