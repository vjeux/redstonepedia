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
  `drop-bench --layout station --pad --hold-y 16 --stagger 0,12,24 --rel 4 --rebrake 16 --rebrake-each --release 3 --exit-gate --open-delay 4 --pulse 12 --refill 2,10 --l1 --slab-stop`
  (ironspec adopted s 12: launch offsets 0/12/24, REL = golem + 4 (day-1's release tap at stage
  2)). The stand's summon bar is the game's rule (ironspec 05:26): at every summon >= 1 caller over
  all 400 sensor phase pairs, with a margin of 2 callers at REL T+2..6; the 20-gt rest bar and the
  all-three-panic rule are retired (the rest margins below stay as printed numbers).
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

1. Pocket k's support powered at REL + offset k (x -1: 0, x 3: 12, x 7: 24: the reverse merge
   order), unpowered (the re-brake) 16 gt later.
2. Pocket k's gate: its exit detector rises after REL + offset k, or its string is off at REL +
   offset k (an empty pocket: the cold start). Its trapdoor opens 4 gt later, not before T+3.
3. Each trapdoor shuts 12 gt after it opens.
4. All three shut: +2 gt, every R_1 powered for 10 gt: the next batch rolls into the holds.

## Timed edges at the 4-gt edge bar (cc767b78's station judge 1e4e249c7, G1-approved with its follow-ups through c2f1473fd)

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

## Rows (bench; the tool reviewed by G1 through 4bce82b85; pulse 12)

- The REL band: the rest before the next summon falls 1 gt per REL (a printed number now: the
  20-gt bar is retired for the summon rule); the landing lead stays 31 (cold start 23); 0 wrong
  pockets, contacts, riders hurt, stuck carts or trapdoors shut onto a cart anywhere in T+3..15.

| offsets | rest at REL T+3 / T+4 / T+6 / T+9 / T+15 | cold start rest (T+3 / T+15) |
|---|---|---|
| 0/12/24 (the run line) | 26 / 25 / 23 / 20 / 14 | 34 / 22 |
| 0/14/28 | 22 / 21 / 19 / 16 / 10 | - |
| 0/16/32 | 18 / 17 / 15 / 12 / 6 | - |

  Control REL T+3..15 x 40 seeds (520 runs a row); restarts at every tick T+0..130 x 10 orders x
  REL 3/6/9 (3,930 runs a row): the same numbers.
- At s 12, REL T+4, per lane x -1 / 3 / 7 (tick-exact's seat-clear scene): the departing cart
  leaves its rest T+6 / 18 / 30 and the station T+31 / 43 / 55; the arriving cart's trapdoor opens
  T+17 / 29 / 41, it lands at rest T+51 / 63 / 75 (a 34-gt fall, 0.624 b/gt); the dwell (landing to
  departure) 55 gt and the turn (departure to the next landing) 45 gt in every lane.
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

## The summon at every T: the panic measure (`drop-bench --panic-post`, bench)

Per summoner (the arriving rider of each lane, in its pocket at the next summon T) over its own two
sensors' phases (each rider's living scan and hostiles scan draw their own: 20 x 20 = 400 pairs):
NEAREST_HOSTILE at each hostiles scan = a scarer that was in the living scan's box (the rider's box
grown by its follow range, 14.16..17.84; rows at 16, 14.16 and 17.84 identical), seen eye to eye
(collider clip) within 16 at every tick since that scan (the cached sight), in its type's range
(pillager 15, zombie 8) now; VillagerPanicTrigger (26.3: duration 60) runs h..h+60, is dead at
h+61 and restarts at h+62, so the rider calls the summon at T if the last scan before T found a
scarer and T is not h + 61 + 62k; a panic begun before the bench's window (before T+40) has an
unknown h: T can be its dead tick, so it does not count as a guaranteed call. The least caller count
is the number of riders calling in all 400 of their own pairs (the bar >= 1, margin 2).

| scarers | s | lane x -1 | lane x 3 | lane x 7 (REL T+2 -> T+9) | least callers |
|---|---|---|---|---|---|
| S1 + S2 | 12 | 400 | 400 | 399 -> 364 | 2 |
| S1 + S2 | 13 | 400 | 400 | 394 -> 345 | 2 |
| S1 + S2 | 14 | 400 | 400 | 385 -> 322 | 2 |
| S1 + S2 | 16 | 400 | 400 | 355 -> 264 | 2 |
| S1 alone (S2 burnt by day under glass) | 14 | 0 | 400 | 385 -> 322 | 1 |
| S1 + S2 (glowstone) + F3 at (1,24,-5) | 12 | 400 | 346 | 399 -> 364 | 1 |
| S1 + S2 (glowstone) + F3 revised (ec23a9b35: (1,25,-5) and its occluders) | 12 | 400 | 400 | 399 -> 364 | 2 |
| S1 + S2 (glowstone) + F3 corrected (8fb667bca: riders at the cart's position, its occluders (-1,19,-1), (3,19,-1), (3,22,-3)) | 12 | 400 | 400 | 399 -> 364 | 2 |
| S1 open west (positive control) | 14 | 0 | 0 | 0 | 0 |
| S1 open west, no S2, F3 revised (positive control) | 12 | 0 | 0 | 0 | 0 |
| S1 open west, no S2, F3 corrected (positive control) | 12 | 0 | 0 | 0 | 0 |

- S1 the pillager at (-5,-1,-5), S2 the zombie at (-5,-4,-2), both open east; F3 the pillager at
  (1,25,-5) open south (and, corrected, its east wall's lower cell (2,25,-5)), with its glass
  occluders and its glowstone floors (its part file laid with `--part`, the scarer `--panic-post
  p:1,25,-5:-`; 9fc3155f's cells). Lanes x -1 and x 3 see S2 / S1 only after
  landing (from about T+48..56 at s 12), inside their trigger's first run: the restart gap never
  reaches T. Lane x 7 drops last and sees S1 from T+65 at s 12 (in range from T+68), so some phase
  pairs miss it. Rows at follow range 14.16, 16 and 17.84: identical.
- The first F3 (1,24,-5) saw lane x 3's rider in its hold: in 54 of its 400 pairs its panic ran from
  before the drop through to T, its trigger's phase history-dependent and T possibly its restart gap
  (1 in 62): no longer a guaranteed caller (least callers 1). The revised F3 hides the holds and the
  drop: every pocket rider's panic starts fresh in its pocket, x 3 back at 400.
- The panic in the pocket at s 12 (revised F3, every REL T+2..6 alike): each lane's panic starts
  (its first hostiles scan with a scarer) at T+51..89 / T+48..85 / T+68..100 (REL T+4; lanes x -1 /
  3 / 7: up to 38 gt after the scarer first becomes visible, the next living scan then the next
  hostiles scan), and its window in the pocket, to leaving its rest at T+106 / 118 / 130, is
  17..55 / 33..70 / 30..62 gt over the phase pairs (the shortest 17 / 33 / 30).
- The riders' positions are redsim's passengers (feet at the cart's y on flat, slope and trapdoor
  alike; tick-exact's official read: a seated villager sits at its cart's position, offset 0); the
  eye at feet + 1.62; the living scan's box from the rider's own box.
- The windows (conservative): lanes x -1 and x 3 in range from T+48..50 and seen in the 16-box
  from T+47..51 (margins 26..35 and 6..17); lane x 7 seen from T+65 at s 12 (the T-38 window fails,
  margin -1 at T+2 to -8 at T+9).
- S2 burns by day under a glass roof (sky light 15 through glass: `canSeeSky`): every zombie post's
  roof is glowstone (ironspec), golem-proof and opaque to sky light. The golem-flow bench: S2 alive
  at both dawns of a 2-day run under glowstone, dead at game time 567..581 under glass.

## The golem flow on the pad (`mobfarm golem-flow`, redsim with the mob AI on, bench)

500 summons over 2 days with S1 (9fc3155f's seated pillager, the portal exemption), S2 (glowstone
roof) and F3 laid, the summon from lanes x -1 and x 3 at every summon tick:

| run | golems | crossed at their 2nd tick | damage | at a summon tick / late sightings | posts at each dawn | verdict |
|---|---|---|---|---|---|---|
| lead | 501 | 501 | 0 | 0 / 0 | 3/3, 3/3 | PASS |
| a twin golem with each (the push case) | 1002 | 1002 (largest move 0.050) | 0 | 0 / 0 | 3/3 | PASS |
| 20 zombified piglins in the portal cells | 501 | 501 | 0 | 0 / 0 | 3/3 | PASS |
| 40 zombified piglins | 501 | 501 | 0 | 0 / 0 | 3/3 | PASS |
| S2 under glass (positive control) | 500 | 500 | 0 | 0 / 0 | 2/3 | FAIL (a post died) |
| no forced summon (`--natural`: the villagers' own panic) | 497 (496 summons; 4 no-spot, 0.8%) | 497 | 0 | 0 / 0 | 3/3 | PASS |
| the portal row at z -5 blocked (positive control) | 243 | 117 (126 never) | 2,633 | 498 / 750 | 2/3 | FAIL |

- The least overlap with a portal row stays 0.200 (spawn 0.200) in every passing run; 0 model
  errors (no seated pillager sees a villager or a golem off the portal within 8). Targets are a
  printed count (ironspec: the bar is the outcome): 145 of 501 golems took S1's pillager or S2 as
  target at their first tick, none moved. No summon lost to golem detection; no-spot misses are the
  pad's own (about 3.1% with two callers).
- A cart or rider cannot reach the pad's edge (the pockets' riders at y 0, the golems' feet -6).
- Restarts (`--restart-phases 0,1,2`: a reload in each of the 10 orders at T+0, T+1 or T+2 after
  every summon, 500 summons a run, 30 runs): every golem crosses at its 2nd tick, no summon is
  lost to golem detection, no golem is in the overworld at a summon tick, the next batch is never
  golem-detected, no post is hit or killed. A reload while the golem is on the pad has two priced
  restart costs (ironspec 2026-11-12 07:42; the bench prints them as their own class, outside the
  verdict):
  - at T+0: S2 takes the golem as target in its first tick in 10..19 runs' worth of summons per
    500, and hits it (3.0, no knockback: golems resist it) 1..7 times per 500; at T+1: 0..6 hits
    per 500; at T+2 (the golem crossing that tick): nothing, all 10 orders PASS.
  - the departing batch re-detects the crossed golem (11..36 sightings per 500 at T+0, 3..19 at
    T+1): a reload empties the sensed lists and re-phases every sensor, the first living scan lists
    the golem still on the pad and the golem sensor counts it from the stored list until the next
    scan (as the game does). In the station those riders leave (the next summoners land from
    T+51), so no summon is lost; the bench's standing summoners count only sightings from the next
    batch's first landing (T-49).
  - the control (`--restart-noop`, the same ticks, no reload) passes at T+0 and T+1.
- Steady state at either goal parity (`--restart-once 150..153`: one reload before the first summon,
  then 500 summons): 0 post-on-golem targets, 0 hits, 500/500 summons, every golem at its 2nd tick.
  The hits need the reload inside the golem's two-tick life (a freshly loaded zombie runs its goals
  in full on its first two ticks).

## Open

- The bed stop's queue rider (loop (9,-8,-6), panicking all day and night) as a caller in the panic
  measure, once b8949c96 lays it in rev 3's loop frame.
- Option (b) for lane x -1 (an S2 cell that makes it panic earlier in its fall): to search.
