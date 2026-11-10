# The single-lane carousel loop (being built)

vjeux, Nov 10 05:58: "a single lane where the villagers are riding on back to back with a
mechanism to release them 3 by 3". The brief: designs/briefs/farm-iron-carousel.md, the top
section. This folder holds the loop as it is built.

## What is real (vjeux 07:29: "You can't say it's working when nothing is wired")

| part | in this file | controls | plays on its own | missing |
|---|---|---|---|---|
| queue register (18 slots) | track built | bench-driven here; day-1 has wired its controller into a copy of this file (explore/day1-sched-wip) | not yet | day-1's wiring merged into this file, then the wired end-to-end run |
| release of the front 3 | track built | as above | not yet | as above |
| stand (3 slots) | track built, a sketch (farmopt owns the real stand; a stop-based stand is being built) | its brake is a real detector rail on the release run plus a delay (day-1 wired it); departures bench-driven here | not yet | farmopt's stand and pad; a catch with more than 2 ticks of slack |
| return to the queue's tail | track built | timers (day-1 wired them; to move to +120/+128 for this file) | not yet | as above |
| zombies (PANIC) | 7 posts built | none needed | yes (bench, and bridged: the game agrees tick for tick) | the pad's own stand zombies (farmopt) |
| bed siding (naps) | not in this file (farmopt's is wired separately) | | | the two junctions |
| golem-floor gate | none | | | all |

"Clean" below always means with bench-driven controls unless it says wired.

## The queue: a sawtooth shift register

Each queue slot is one 3-cell period: a powered rail sloping down (the cart's resting cell, braked
while unpowered), a plain rail sloping up, a plain flat peak (the next slope's sturdy high side).
One power pulse on every slope (7..12 gt) rolls every cart down, up, over the peak and onto the
next braked slope: every cart one slot on, the front one out. A cart starting lower arrives slower
and stops sooner, so the resting offsets converge (0.32 of the cell down the slope): no drift. The
carts sit 3 cells apart, outside the 0.2 push range: no creep, no bounce, no momentum build-up.
tick-exact bridged the register's one-slot moves against the real server (0 differences).

## The loop (`mctas mobfarm queue-loop --detector --boost-first 8 --tail-desc --zombie-every 8`)

Top view, the pad (farmopt's) south of z -1:

- **Stand** (sketch) along +x at z 0, S3's slope at x -1, S2 x 2, S1 x 5: catch slots, i.e.
  sawtooth slots whose preceding flat cell (the run's last cell, S3's peak, S2's peak) is a
  powered rail linked to the slope (a powered rail takes its connected neighbour's power).
- **Release run** south along x -3 from the queue's head (z 19) into the stand's west end; 8
  boosters right after the head (the carts are capped at 0.4 b/t anyway; the boosters raise the
  train's speed into the stand, which widens the catch's window from 1 to 2 ticks); a detector
  rail at (-3,1,4).
- **Departure** from the stand's east end north along x 9, up 3 blocks at z 16..19, west along z
  20 at y 4 into the queue's tail.
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
- **Zombie posts**: a zombie in a boat on glass at (3,3,5+8k), k = 0..6, leaves over it; every
  rider within 8 blocks and in sight of one, all along the loop.

`mctas mobfarm queue-loop` prints the control cells (each the block under a powered slope); the
tail's are (0,1,25), (0,0,29), (0,-1,33).

Per slot (t0 = the release start = golem pulse + 4 in day-1's controller; the times day-1's
controller produces):
- the release group (the queue's front 3 slopes) on for 36 gt from t0: the 3 leave as a train;
- the stand's batch leaves one slot at a time: S1 on at t0+6, S2 +18, S3 +30;
- the stand braked (all 3 slopes) 26 gt after the detector rail switches on, in redstone terms
  (the bench's +27: it applies a change after the tick, a repeater during it);
- the queue advances 3 slots: pulses at t0+38, +58, +78, 12 gt each, every queue control on the
  same tick;
- the queue's tail opens at t0+90 (slots 1-2) and +92 (slot 0);
- the returning batch fills the tail front first, by timers from its departure: slots 1-2 braked
  at +120 (works +118..+122), slot 0 at +128 (works +126..+130).

## Results

- Bench-driven, position-triggered stand brake, tail timers +110/+116 (3 boosters): 100 seeds x
  1,440 slots (6 days each), 0 problems.
- Every control a timer or the detector (3 boosters, bench +28; day-1's latencies): 100 seeds x
  1,440 slots, 0 problems.
- 8 boosters: the stand works at bench +26 and +27 (redstone +25 and +26), 3 seeds x 300 slots
  each with day-1's 36/12-gt pulses; +25 and +28 fail.
- Every slot: the queue releases exactly its front 3; the batch settles at the stand at t0+81
  (golem+85); the previous batch rests in the queue's tail before the next advance.
- PANIC: with the zombie posts every rider panics 100.00% of deep-night ticks (day time
  13,670..22,330), 3 seeds x 720 slots; 7 of 7 zombies alive at the end; riders all in their own
  carts. Without the posts: 0%. Posts every 16: 72.6%.
- Golem spawns: a villager riding anywhere on the loop's 118 rail cells finds 0 golem spawn cells
  (`queue-loop-spots`, redsim's copy of the game's LEGACY_IRON_GOLEM search).
- tick-exact bridged the stand catch against the 26.3 server: the game matches the bench tick for
  tick, the window included (the 1-tick catch, then the 2-tick one: carousel-catch2, GO
  ea7960400, 15,750 of 15,750 cart-ticks equal at +26 and +27). The riders' PANIC near the zombie
  posts is the same in the game, tick for tick.
- This file (descending tail slots), day-1's times, tail +120/+128: 3 seeds x 200 slots clean at
  bench +26 and +27 (redstone +25/+26).

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
