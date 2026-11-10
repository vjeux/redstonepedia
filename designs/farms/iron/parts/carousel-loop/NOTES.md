# The single-lane carousel loop (being built)

vjeux, Nov 10 05:58: "a single lane where the villagers are riding on back to back with a
mechanism to release them 3 by 3". The brief: designs/briefs/farm-iron-carousel.md, the top
section. This folder holds the loop as it is built.

## What is real (vjeux 07:29: "You can't say it's working when nothing is wired")

| part | in the file | controls | plays on its own | missing |
|---|---|---|---|---|
| queue register (18 slots) | track built | bench-driven (each slope's support switched glass / redstone block by the bench) | no | day-1's release controller (works on redsim, not yet in this file) |
| release of the front 3 | track built | bench-driven | no | day-1's controller |
| stand (3 slots) | track built, a sketch: farmopt owns the real stand (part 2) | bench-driven, position-triggered brake | no | farmopt's linear stand and pad |
| return to the queue's tail | track built | bench-driven, position-triggered brakes | no | timer circuits for the stand and tail brakes |
| zombies (PANIC), golem-floor gate | none | | | all |
| bed siding (naps) | none (farmopt is designing it) | | | all |

Nothing here is wired redstone yet. "Clean" below always means with bench-driven controls.

## The queue: a sawtooth shift register

Each queue slot is one 3-cell period: a powered rail sloping down (the cart's resting cell, braked
while unpowered), a plain rail sloping up, a plain flat peak (the next slope's sturdy high side).
One power pulse on every slope (7..12 gt) rolls every cart down, up, over the peak and onto the
next braked slope: every cart one slot on, the front one out. A cart starting lower arrives slower
and stops sooner, so the resting offsets converge (0.32 of the cell down the slope): no drift. The
carts sit 3 cells apart, outside the 0.2 push range: no creep, no bounce, no momentum build-up.
tick-exact bridged the register's one-slot moves against the real server (0 differences;
controls still bench-driven).

## The loop (`mctas mobfarm queue-loop` + `queue-loop-run`)

Top view, the pad (farmopt's) south of z -1:

- **Stand** (sketch) along +x at z 0, S3's slope at x -1: 3 catch slots, i.e. sawtooth slots
  whose preceding flat cell (the run's last cell, S3's peak, S2's peak) is a powered rail linked
  to the slope (a powered rail takes its connected neighbour's power). The fast release train
  crosses its braked catch cells and stops 0.4..0.6 down the slopes.
- **Release run** south along x -3 from the queue's head (z 19) into the stand's west end; 3
  boosters right after the head.
- **Departure** from the stand's east end north along x 9, west along z 20 into the queue's tail.
- **Queue** north of z 20 in two rows 3 apart: row A (the tail) north along x 0, 7 slots; a turn
  slot (row A's last peak is a curve, the slot runs west, its peak a curve again) so no queue move
  is longer than another; row B (the head) south along x -3, 10 slots.
- **Tail slots 0-2 are deep slots**: a braked slope linked to the slot's own (two blocks of braked
  descent), then two plain slopes up and the peak. A fast returning cart stops on one of the two
  slopes, never on a flat cell; a slow queue move stops on the first.

Per slot (t0 = the release start = golem pulse + 4 in day-1's controller; every control
bench-driven, the times day-1's controller produces):
- t0: the release group (the queue's front 3 slopes) on for 34 gt: the 3 leave as a train; the
  stand's batch leaves one slot at a time (S1 at t0, S2 +12, S3 +24);
- the stand braked (all 3 slopes) 28 gt after the release train's front cart switches on the
  detector rail at (-3,1,4) on the release run (t0+71..73). This one is tick-exact: +27 and +29
  both fail (the catch cell's window is ~0.6 block, one tick of the train's travel); the
  position trigger (front cart 0.45 into S2's peak) gives the same result;
- the queue advances 3 slots: pulses at t0+38, +58, +78, 10 gt each;
- the queue's tail opens (slots 0-2 powered) at t0+88;
- the returning train fills the tail front first, by timers from its departure: slots 1-2 braked
  at +110 (works from +108 to +114), slot 0 at +116 (works from +112 to +120).

Bench-driven results:
- position triggers: `queue-loop-run <file> --cycles 60 --seed S`, seeds 1..9, 0 problems;
- timers and the detector only (what a circuit does): `--stand-detect 28 --intake-brake 110,116`,
  4 seeds x 200 slots and 3 x 60, 0 problems.
Each slot the queue releases exactly its front 3; the batch is settled at the stand at t0+81
(offsets 0.41 / 0.55 / 0.58 down the slopes); the previous batch is in the queue's tail before
the next advance at t0+138.

Arrival margin: the batch settles at the stand at t0+81 = golem+85, 15 gt before the slot (T+100),
so the riders must be panicking on arrival (zombies along the loop, not in yet).

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
  of its window.
