# The F1 + Q2 station (dfb89499)

Three level pockets under M1 vertical drops, the golem pad under the summoners' overlap, in
9fc3155f's budget (explore/farmopt-c3-borders `wip/c3/f1q2-budget.txt`; the station's box loop
x -3..9, y -11..19, z -6..0). Loop frame; the bench files are written at file = loop + (6,0,6).
Bench rows from `mctas mobfarm drop-bench --layout station` (labelled bench: the controller and the
lanes' feed are emulated until the circuit is laid).

## The lead's flags

- Mode: `--l1 --open-delay 4` (the controller and its timers on game ticks through anything; the
  hold's release at the controller's stage 3, 4 gt after its gate). Rows without `--l1` are L2 (the
  live clock), for the pearl alternative only.
- The build: `--slab-stop` (each hold's north stop a bottom slab). No zombie post: the hold post at
  loop (2,16,-6) came out (9fc3155f's static (c) with the summoner-only scarers: 0 never-clearing
  places without it), and with it the 6.08 question.
- The run line (cc767b78's L1 judge runs it):
  `drop-bench --layout station --pad --hold-y 16 --stagger 0,14,28 --rebrake 16 --rebrake-each --release 3 --exit-gate --open-delay 4 --pulse 12 --refill 2,10 --l1 --slab-stop`
  (launch offsets 0/14/28, ironspec's provisional s 14; at day-1's REL T+6 the station's rest
  before the next summon is 19 there, 1 under the 20 bar: see the rows; 0/12/24 holds it to T+9).
- Restarts at every phase (ironspec's reload-order rule): `--restart-sweep 0,130,1` (each tick, in
  the save order, its reverse and 8 seeded orders: cc767b78's `liveness::restart`). The cold start
  is the fresh design's row: `--dep-riders x,x,x` (G1). `--restart` with `--freezes`/`--login` is
  refused (a restart's unload resets the freeze's entity box; freeze rows are the pearl
  alternative's only).
- The committed design (onechunk's module): `f1q2-station.txt` here, in the loop frame, written by
  the run line with `--write-design` (read back through the parser before it is kept): the build,
  the start state's carts, `positions:`, the runner's `run-cells:`, the cells, the event table, the
  read path, R_1's built driver and the placement-order list in its header. Its Nether part is the
  bench's placeholder (a 2 x 3 portal at the Nether's origin), not the build: the shared kill
  room's portal at the pads' y replaces it. `mobfarm exit` on it: 48 spawn spots, all on the pad
  (x -3..8, y -6, z -6..-1), 432 golem runs every one through a portal.

## Cells (loop frame)

- Pockets: north_south powered rails at (-1,0,0), (3,0,0), (7,0,0) on switched supports (x,-1,0);
  the launcher stone (x,0,-1) with an unattached string on it (x,1,-1) (no golem spot, passes
  sight).
- Pocket strings (x,1,0), hooks (x+-1,1,0), glass (x+-2,1,0).
- Departure: plain (x,0,1), the exit detector (x,0,2) (plain rails north and south, air east, west
  and above, glass below: no powered rail, gate or junction on a face; its raw connections' rails
  have 2 potential connections each: the detector rule holds), plain (x,0,3), boosters (x,0,4..6);
  the merge row at z 7 (7b90c3f1's). The merge row's entry speed: 0.831 b/gt.
- Holds: bottom-half trapdoors at (x,16,0), carts at y 16.1875 (riders' feet 16.19); the ring's
  glass at (x+-1,16,0) and (x,16,+-1); the north stop a bottom slab at (x,17,-1); the hold string
  (x,17,0), hooks (x+-1,17,0), their supports glowstone at (x+-2,17,0) = (-3/1/5/9,17,0)
  (ironspec: solid-render stroll destinations; a stone top was a golem spot seen from the holds
  (9fc3155f's summoner flood) and a dark natural-spawn top; glowstone is on the golem rule's
  exclusion list and lit). Glowstone conducts no power: each hold string is read from its hooks
  directly (dust or a repeater beside the hook), never through the support.
- R_1 (x,17,1): a powered rail ascending south on the ring's south glass, its power cell
  (x+1,17,1) (glass over it at (x+1,18,1)). That cell is face-adjacent to the hook (x+1,17,0): the
  built driver is a repeater facing west into R_1, its input from the east (x+2,17,1), which no hook
  drives (never dust, nor a repeater or comparator reading from the north); the bench emulates it
  (a redstone block while the R line is on). The station owns the hold-string row
  (x-2..x+2,17,0) and R_1; R_2's rail (x,18,2) and its glass are the lanes'.
- The carts where the lane leaves them (tickexact's lane-handoff ca3cdb2ed, its +x our -z): the
  hold's cart 0.49 into its cell against the ring's north glass (scene x 6.51); the next one braked
  on R_1 against that cart's rider (z 1.28, y 17.3425; scene x 5.72).
- Pad: x-axis portals x -2..7, y -8..-6 on rows z -5 and -2; obsidian frames at x -3 and 8,
  bottom y -9, top y -5; glass over the frames at y -4; cobblestone floor at y -7 on rows z -6,
  -4, -3, -1 (x -3..8): golem feet -6, 48 spots (P 0.993); every golem through a portal. The seal:
  cobblestone at y -8 under the floor rows, x -2..7 (40 blocks).
- Placement order (bottom-up, each rail placed facing its own axis): in this file alone R_1
  (x,17,1) places flat (its slope needs the lanes' R_2 above it); with the lanes' R_2 rails laid in
  the same order (the module) R_1 takes its slope and every station rail its stored shape (R_2's
  own slope needs the lanes' R_3: theirs). No curve, no T beside the cell over R_1.

## The pad's seal (ironspec's coupling check 02:04, ruling 02:24)

- An adult zombified piglin (0.6 x 1.95) arriving in a portal or spawned there stands in its portal
  row on the frame's bottom obsidian (feet -8): top obsidian at y -5, the end frames at x -3 and 8,
  the floor rows' cobblestone at y -7 along both sides. It cannot pass sideways (its box at
  y -8..-6.05 meets that cobblestone) nor climb (the floor rows' top is 2 above its feet, its jump
  ~1.25).
- A baby (0.3 x 0.975) fitted the open cell under a floor row at y -8 and dropped out under the
  pad: the seal (cobblestone at y -8, x -2..7 under all four floor rows) closes every cell that
  touches a portal cell sideways (the end cells at x -3 and 8 touch only a frame diagonally); a
  baby's jump reaches feet -6.75 at most, under the floor rows' top.
- A piglin standing in a portal row (top -6.05) does not meet a golem's spawn box (feet -6); a
  jumping one can for a few ticks (onechunk's escape bench and my golem-flow bench take them).

## Events (L1, game ticks; T the summon tick)

1. Pocket k's support powered at REL + offset k (x -1: 0, x 3: 14, x 7: 28: the reverse merge
   order), unpowered (the re-brake) 16 gt later.
2. Pocket k's gate: its exit detector rises after REL + offset k, or its string is off at REL +
   offset k (an empty pocket: the cold start). Its trapdoor opens 4 gt later, not before T+3.
3. Each trapdoor shuts 12 gt after it opens.
4. All three shut: +2 gt, every R_1 powered for 10 gt: the next batch rolls into the holds.

## Timed edges at the 4-gt edge bar (cc767b78's station judge 1e4e249c7, unreviewed)

Each row the full lead matrix: REL T+3..7 x (control, cold start, restarts at every phase
T+0..150 x 10 orders), 7,560 runs; 0 failures at the run line (rest >= 34, landing lead 23).

| timed edge | run line | passing range | margin at the run line |
|---|---|---|---|
| trapdoor pulse | 12 | >= 6 | 6 gt |
| re-brake | 16 | 6..35 | 10 below, 19 above |
| R line length | 10 | >= 6 | 4 gt (the tightest) |

Outside: pulse 5, a trapdoor shuts onto its cart; re-brake 4..5, a departing cart stuck; re-brake
>= 36, the cold start lands within 4 gt of the re-brake; R line 5, the refill fails. Every edge
at the run line clears 3 gt + 1 scheduled tick.

## Rows (bench; the tool reviewed by G1 through d1a5f2985; pulse 12)

- The REL band (day-1 measured REL = T+6 at every phase with d_P 0): the rest margin before the
  next summon falls 1 gt per REL; the landing lead stays 31 (cold start 23); 0 wrong pockets,
  contacts, riders hurt, stuck carts or trapdoors shut onto a cart anywhere in T+3..15.

| offsets | rest at REL T+3 / T+6 / T+9 / T+15 | first REL under the 20 bar | cold start rest (T+3 / T+15) |
|---|---|---|---|
| 0/12/24 | 26 / 23 / 20 / 14 | T+10 | 34 / 22 |
| 0/14/28 | 22 / 19 / 16 / 10 | T+6 | - |
| 0/16/32 | 18 / 15 / 12 / 6 | below T+3 | - |

  Control REL T+3..15 x 40 seeds (520 runs a row); restarts at every tick T+0..130 x 10 orders x
  REL 3/6/9 (3,930 runs a row): the same numbers. So the station takes 0/12/24 with REL + 4 <= its
  first failure at REL T+6 (the router's line); 0/14/28 misses the 20 bar at T+6 by 1 gt and
  0/16/32 everywhere: the largest step that passes at T+6 is 13 (0/13/26: rest 21); +8 gt over
  0/12/24 does not fit (the fallback: lengthen the farther lanes' paths and keep 0/12/24).
- The measured departure variation (each lane from its launch tick REL + offset, over every row,
  phase and restart at 0/12/24, 0/14/28 and 0/16/32): its cart leaves the pocket's rest at +2, the
  station (loop z 6.9) at +27, at 0.831 b/gt, every lane, every run: no spread (L1: the taps are
  exact). The shaft entry is the merge's (7b90c3f1's), from these.
- At 0/12/24, REL T+3..15: the last trapdoor shuts T+52..64, the R line T+54..66, the next batch
  in its holds by T+67..79 (cold start T+44..56, T+46..58, T+59..71). The last departing rider
  leaves the station (feet 0.06) at T+54..66: 29 gt before T+95 at REL T+15 (41 at T+3) for the
  merge and the lift to lift its feet out of the next golem's lock (14.54).
- `positions:` (pockets and holds only) on its timing argument (summons only at gameTime % 100 ==
  0): villagers in a shaft or on a departure rail at a summon tick (T+0/100/200, before and after
  the tick): 0 in every row.
- The falling carts' largest horizontal offset from their shaft's centre line: 0.0100 in every row,
  the hold cart's rest offset (0.49, against the north glass); nothing adds to it in flight. The
  fall is tick-exact's drop-bridge 131a9b2f2 (ridden carts from closed bottom-half trapdoors down
  1 x 1 shafts onto braked pockets from 14 and 17 blocks, 600 ticks, 0 differences: a cart falling
  from rest gets only gravity and drag; the station's 16-block fall's in-flight positions are a
  prefix of the 17-block one's; the open shaft does nothing, 0.01 clearance at zero horizontal
  speed); the push before the fall (cart 2 braked on R_1 against cart 1's rider) is lane-handoff
  ca3cdb2ed (0 differences over the whole cycle: cart 2 moves 0 from the drop to R's second pulse).
- The re-brake window (my bench, bar 0): clean 6..47 (<= 5: the departing cart braked before it
  leaves; >= 48: the arriving cart lands on a powered pocket).

## Open

- The golem-flow bench (the scarer line's deciding run): the golems' crossing with the lead posts
  laid, push sources, the blocked-row control; pillagers are not modeled in redsim.
