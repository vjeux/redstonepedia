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
  `drop-bench --layout station --pad --hold-y 16 --stagger 0,14,24 --rel 4 --rebrake 16 --rebrake-each --release 3 --exit-gate --open-delay 4 --pulse 12 --refill 2,10 --r-repeater --hold-repeater --l1 --slab-stop`
  (`--hold-repeater`, commit E: the holds' trapdoors through their built drive, not the
  emulation's state write; see the holds below)
  (`--refill 2,10`: the R line at all three trapdoors shut + 2 for 10 gt, labelled, verbatim
  (ironspec 20:24): "(the refill: the R line at all shut + 2 for 10 gt, unfixed; the game's direct
  bars V1–V3 pass at the deepest normal depth, ironspec 20:24; the design accepts the measured
  rests, V4)" (tick-exact 18:01: in all 42 game refills the new slot-1 cart reached R_1 0..7 gt after
  its last powered tick and rested short, z 1.79..1.90 against the designed 1.28; tick-exact 20:22:
  the unfixed control passes V1–V3 in the game at depth 6 in both states). `--r-repeater`: R_1's built drive laid and driven, farmopt 10:13: per lane a repeater (delay 1)
  at (x+1,17,1) facing west into R_1 on glass (x+1,16,1), fed from the controller's cell
  (x+2,17,1); against the old glass stub, +3 glass a copy, the repeaters in the stubs' cells).
  (ironspec adopted s 12: launch offsets 0/12/24, REL = golem + 4 (day-1's release tap at stage
  2); ironspec 20:00/20:03/20:07: lane x 3's offset 14, final: at 12 the lane x -1 -> lane x 3 gap
  at the lift's exit top, where carts touch, is 14 in the freeze rows, under the column bar 16). The
  stand's summon bar is the game's rule (ironspec 05:26): at every summon >= 1 caller over all 400
  sensor phase pairs, with a margin of 2 callers at REL T+2..6; the 20-gt rest bar and the
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

- Pockets: north_south powered rails at (-1,0,0), (3,0,0), (7,0,0) on glass supports (x,-1,0)
  (V15: no golem floor or stroll candidate; the drive from the side, M16bw (ironspec 16:03):
  day-1's repeater (x-1,0,0) into the rail (facing west, on glass (x-1,-1,0)), laid in this file as
  a stand-in until day-1's file, its riser and feed west of it (day-1's); the bench drives the
  repeater's input from a lever stand-in at (x-2,0,0) on glass (G1 on d8394ed6e, ironspec 17:46/
  17:56: no block in the support cell), switched 1 gt ahead so the rail keeps its tick). The
  supports stay glass and are never switched: each lane's step dust (x-3,-1,0) touches the support
  (x-4,-1,0) west of it (with that support a redstone block, lane -1's drive reached lane 3's dust
  and lane 3's lane 7's: both launched 9 gt early in the bench). The launcher's back (x,0,-1) slime
  (V16s, ironspec 15:08/15:11: a cart at rest on a powered rail is kicked only away from a redstone
  conductor; with V15's glass back all three carts stayed in their pockets; tick-exact's official
  kick row 15:18) with an unattached string on it (x,1,-1) (no golem spot, passes sight).
- Every drive the bench emulates is a lever stand-in at its line end (ironspec 17:46/17:49/18:34/
  20:31): the pockets' inputs (x-2,0,0) on glass, the R line's input (x+2,17,1) on glass
  (`--r-repeater`; the old redstone-block emulation in R_1's side cell is refused) and the napper's
  (7,-1,8) on glass (7,-2,8) (module mode; the wall lever on (7,-1,7) was retired, ironspec 18:31:
  its toggle updates the merge T (7,0,7)). The slope's line end (-4,12,5) gets no stand-in: on every
  cell a lever could hang on there (glass under, over, east, west or north of it) some rider's sight
  ray between T and S crossed the support (13..39 ray-ticks a run: lane -1 rising in the column to
  lane 3's arriving rider falling into its pocket, lane 7 climbing to S1's and S2's posts), so the
  bench holds the braked slope's own rail (-4,12,7) powered, from REL + release + 1 for the width:
  the tick and the span day-1's repeater (-4,12,6) gave it from a lever on (-4,12,5). It is a
  declared `held:` stand-in (ironspec 01:07; this file's header and the bench's `--out` header name
  its driver and phase): the rail's own state, the same block, so the guard's list and its survival
  pass; at each edge the bench makes the neighbour updates the build's path makes (the repeater's
  updateNeighborsInFront, the rail's setBlock with its affectNeighborsAfterRemoval and
  PoweredRailBlock.updateState's updates below and, ascending, above), all but the rail's own
  re-evaluation, which the built rail answers by keeping its state and the bench's undriven one by
  dropping, and prints the cells it updated; a plain rail with three or more potential connections
  among those cells fails the row (G1's note on f7e3a825, a condition of its GO: these updates carry
  the rail's kind where the build's front updates carry the repeater's, a signal source, which
  re-settles a plain rail at exactly 3 potential connections (RailBlock.updateState 22-26,
  tick-exact's citation; 4 is flagged conservatively); none today; `--block` a third rail beside an
  edge rail is the control); the
  hold check fails a row whose rail reads unpowered
  at the end of a tick inside the window or powered outside it, and `--hold-poke K` (a neighbour
  update to the rail itself at REL + K) is its positive control (G1 on 82f9546f). The bench
  refuses a stand-in whose properties differ from the built cell's over the full guard list
  (ironspec 23:30, until cc767b78's `recordmatch::guard_intrinsics`): legacy-solid, a full top
  face, full-block collision, solid-render, any collision, air, a signal source, golem-excluded, a
  rail, dangerous, a fluid and the golem spawn-space role (each lever against its line end,
  redstone dust); whose update reach (a lever's six neighbours and its support's) holds a rail with
  three or more potential connections; whose power reach (its neighbours, and its support's if that
  conducts) holds a redstone component or rail besides its input; or that can't survive where it
  stands or is written.
- Pocket strings (x,1,0), hooks (x+-1,1,0), glass (x+-2,1,0).
- Departure: plain (x,0,1), the exit detector (x,0,2) (a plain rail north; south a plain rail in
  lane x -1, an always-on powered rail in lanes x 3 (its booster) and x 7 (its ramp's foot), never
  switched (G1: harmless); air east, west and above, glass below: no gate or junction on a face;
  its raw connections' rails have 2 potential connections each: the detector rule holds). Lanes x
  -1 and x 3: (x,0,3) plain (lane x 3's a booster, path (a)'s (1), powered through z 4's chain; z 1
  stays plain: powered, it would chain into the pocket and kill its brake), boosters (x,0,4..6) on
  glass powered by an always-on redstone torch (x-1,0,5) on glass (x-1,-1,5) (V15); the merge row
  at z 7 (7b90c3f1's).
  The merge row's entry speed: 0.831 b/gt (lane x 3's 0.950). Lane x 7: path (a)'s climb (below).
- Holds: bottom-half IRON trapdoors at (x,16,0) (redstone-only: no villager, golem or zombie
  opens one; the bench's pulse keeps the trapdoor kind the world holds), carts at y 16.1875 (riders'
  feet 16.19), each resting on its shut trapdoor's top (a designed floor contact, face up, ironspec
  16:45: its bounded declaration waits for onechunk's closed-loop bases); the ring's
  glass at (x+-1,16,0) and (x,16,+-1); the north stop a bottom slab at (x,17,-1). THE TRAPDOOR'S
  BUILT DRIVE (commit E, ironspec 00:43 (2), day-1 00:38 / 00:45: the holds open per lane 4 gt after
  D_k rises after REL + offset k and shut `pulse` gt later, day-1's controller wiring the HOLD pulse
  to the cells named here): the ring's NORTH glass (x,16,-1) is a SLIME block, the conductor (it
  conducts and isn't solid-render, the pocket back's choice, ironspec 15:08; its full side is still
  the hold cart's designed north rest; its top is covered by the north stop slab (x,17,-1), so no
  stroll or spawn top; a full collider like the glass, so sight is unchanged), fed by a repeater
  (delay 1) at (x,16,-2) facing south into it (block state facing=north) on glass (x,15,-2); its
  input (x,16,-3) is the controller's line end (day-1's, on its own support (x,15,-3)); the bench
  drives it from a lever stand-in on glass there (left out of the written file), HOLD_DRIVE_LATENCY
  gt before each declared open and shut, and checks each trapdoor opens and shuts at its declared
  tick. Power-out: the powered slime's other neighbours are the north stop slab and the empty cells
  (x+-1,16,-1), (x,15,-1), so it powers only the trapdoor; neither the R-line repeater (x+1,17,1) nor
  a pocket drive (x-1,0,0) touches the slime or its repeater. Not the ring's other cells: the east /
  west glass's feeding repeater would have its back in the next lane's ring glass; the south glass
  (x,16,1) is R_1's support and would power R_1. +2 blocks per lane (the repeater, its glass).
  The hold strings
  (x,17,0), their hooks (x+-1,17,0) and their supports (x+-2,17,0) = (-3/1/5/9,17,0), GLOWSTONE on
  main (ironspec 02:59; the farmopt 05:18 / ironspec 05:23 glass ruling never reached main: ironspec
  21:56, "ruled but unlanded"), are gone
  (day-1 21:21, ironspec 21:22 / 22:12): no gate of day-1's list (rev 20:00) reads them, they have no
  physical role (9fc3155f's audit: they don't separate empty from ridden), and no part rests on or
  is supported by the supports (dfb89499: the supports held only the hooks; the slot-1 strings'
  supports are (x+-2,18,1), the controller's line end (x+2,17,1) stands on (x+2,16,1)). Four
  light-15 sources leave with them; ironspec 02:59's reasons, each with its check (9fc3155f 22:07 /
  22:20, a/4551510055096892, md5 4d220c7e9f38307dbbceacc0c0f48321: the prints, the records' md5s and
  the commands): lit: 9fc3155f's light-aware spawn static on V rev 0 with F3's record end rods and
  glass (ironspec 22:44 / 22:47 / 22:52; `exterior-spawn --lit`; a/2372781836828254, md5
  6c0cbc4cbfaf4b3690012fe2b2c3cba8): 0 cells spawnable with or without the four (interior 878 / 878,
  tops 588 -> 584, LIGHT-DEPENDENT 73 -> 69, the 4 dropped the tops themselves): no cell needs them;
  it holds with F3 r3's lanterns (ironspec 23:34 (a): light only rises, 15 against the end rods'
  14, and glass passes light, so no cell needs the four a fortiori); (the earlier rev-3 run,
  a/4551510055096892, read onechunk's glowstone F3 lights);
  golem-proof: the golem rule identical (52 / 48 / 4 off-pad, 317 refused, FRAGILE 19, PAD 16: rev
  3's run, 9fc3155f 22:07; V's: 0 off-pad); a
  solid-render stroll destination: that reason is WITHDRAWN (ironspec 23:01): the tops were
  unreachable idle-stroll candidates arming CANT_REACH (9fc3155f's 05:18 statics: an unreached
  idle-stroll STAMP class), so the removal IMPROVES the stamp: the stroll-cell print on V (the same
  artifact, `flee-check --auto-columns --start-state free --stroll-cells=-4,12,7`): (-4,12,7)'s calm
  stroll has 3 candidates with the glowstone tops (-3|1|5,18,0) and 0 without (on rev 3: 12 -> 9, the
  lost 3 exactly those, a/4551510055096892); the next composed night's per-rider max stamp age
  against r8's <= 530 / 1,200 is a confirmation, not a gate
  (ironspec 22:12 / 23:01); a sturdy hook face: moot without the hooks. Sight and push-scan unchanged; the
  cell diff of main's station vs this file is exactly its 16 cells (3 trapdoors oak -> iron, 13
  removed). The hold-string lead bar
  is retired by name (ironspec 21:16: its consumer was G8c, not bought at 20:24); the bench prints
  the leads only where a world still has hold strings (the module rows' record7f12). Banked
  (ironspec 21:16): a string's release lands on its own 10-gt re-check grid (TripWireBlock
  reschedules its check every 10 gt while pressed), and a world open shifts that grid against the
  carts by N (1 or 2): any bar keyed on a release allows up to 10 gt of quantization plus N
  (station-judge's restarts at T+k, k mod 10 != 0, moved lane 2's release from T+51 to T+59 / 60).
- R_1's north ends: the rails (x,17,1)'s north exits into the freed (x,17,0) are DECLARED track ends
  for flee-check's walk (9fc3155f 21:31, day-1 21:33: a declaration, not a stop block), per lane
  `--track-end x,17,0=REASON` with x -1 / 3 / 7 and REASON "R_1's north end: the refill's designed
  drop into the empty hold (the R line launches the slot-1 cart off it onto the shut trapdoor
  (x,16,0) inside the ring and the north stop); against a full hold the launched cart rests on R_1
  (z 1.28 in the bench, 1.79..1.89 in the game, V4) and never reaches it". The planted rows (this
  file, the station line with record7f12): the empty holds, the refill: the next batch in its holds
  T+70, none at rest off its rails; ridden carts planted in all three holds at T+55, so the R line
  launches each slot-1 cart north into a full hold: each rests on its R_1 at z 1.28, on the rail,
  none off its rails (the refill rows fail by design: the holds were full).
- The station's string classes (9fc3155f's string-audit, tools10 part 20; ironspec 01:11 / farmopt
  01:20: declared in commit E): the slot-1 strings (-1|3|7,18,1) are CART-KEYED (a cart on R_1
  presses it, ridden or empty (ironspec 14:26; tick-exact AI / AH)). PROSE until 9fc3155f's part 35
  (tools11) reads the header line; then the station file's header carries, verbatim:
  `# string-class: -1,18,1+3,18,1+7,18,1=cart-keyed (a cart on R_1 presses it, ridden or empty
  (ironspec 14:26; tick-exact AI / AH))` (today's header check would flag the unknown key), with the
  other station strings' classes from the same audit. Until then string-audit takes them as flags:
  `--expect -1,18,1=cart-keyed --expect 3,18,1=cart-keyed --expect 7,18,1=cart-keyed`.
- The slot-1 presence strings over R_1 (F1's input, day-1's G8: REL = the three strings pressed AND
  RE_ARMED; ironspec 11:46's form): the string (x,18,1), its hooks (x-1,18,1) facing east and
  (x+1,18,1) facing west (in the cell that held the glass over R_1's driver), their supports glass at
  (x+-2,18,1) (shared at x 1 and 5, each over that lane's driver input (x+2,17,1): 9fc3155f's
  update-reach on a shared support covers both lanes' hooks, ironspec 20:12). A cart on R_1 presses
  it, ridden or empty (ironspec 14:26 / 14:29: F1 needs a cart in each R_1; a riderless cart is a
  counted fault in the fault note; ironspec 20:31 / 20:41: an empty cart at slot 1's measured rest is
  cart-keyed by the raised move, a cart on the ascending R_1 meeting the band one block up each
  move). The bench prints each string's edges, the least margin of its slab inside each resting
  rider's box, and every cart over the string's cell per tick (its end-of-tick box against the band
  and redsim's own press, which models the raised press inside move_along_track, cc767b78 22:43;
  near-edge figures, within ~0.05 of z 1.30, don't cite until cc767b78's fixtures). The press
  (D's bench, then tick-exact's game rows): an empty cart at the measured rests (z 1.79..1.89) meets
  the band with its end box by ~0.5 (tick-exact AI); at the drawn 1.28 only the raised check presses
  it, released 1 tick in 11 (tick-exact AH); an empty cart planted at 1.28 behind a full hold is
  pushed to z 1.3011 on its first tick, in the game too (tick-exact AM c9064c497). (C) was bought at
  22:41, so the slot-1 empty-cart rest bar (ironspec 22:28) is retired by name (ironspec 23:34: "its
  purpose ended when (C) was bought at 22:41"), its dependency recorded: the next arrival pushes an
  empty slot-1 cart to z 1.2590 (hold empty) / 1.2800 (hold full) (tick-exact 23:25), F1 then held
  by slot 2's rider; if (C) is ever removed, the analysis starts there. The bench keeps
  `--plant-cart empty:...` and `--plant-shift` (the refill phases), the empty carts' ends printed.
  (The refill latch that would have read this string as its sensor (B) was cancelled, ironspec
  20:24.) Against V rev 1 (vcompose fac314245, md5 297b5254d2ba), the station line's sight check
  fails at loop (5,16,1): V's F3 r3 lanterns stand at (1,16,1) and (5,16,1), lanes -1 / 3's
  R-line line-end support cells (x+2,16,1), where the bench's lever stand-in has its glass:
  EXPECTED and LABELLED (ironspec 00:43): a pre-existing conflict between F3 r3 and day-1's line end
  at (x+2,17,1), whose owners (9fc3155f / day-1) propose; lane 7's (9,16,1) is free.
- R_1 (x,17,1): a powered rail ascending south on the ring's south glass, its power cell
  (x+1,17,1) (the slot-1 string's east hook over it at (x+1,18,1)): the
  built driver, laid in the file (farmopt 10:13), is a repeater (delay 1) there facing west into
  R_1, on glass (x+1,16,1), its input from the east (x+2,17,1) (the controller's cell)
  (never dust, nor a repeater or comparator reading from the north); the bench drives the
  repeater's input with a redstone block while the R line is on (`--r-repeater`). The station owns
  the hold row and R_1 with its driver; R_2's rail (x,18,2) and its glass
  are the lanes'.
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
4. All three shut: +2 gt, every R_1 repeater's input powered for 10 gt (the rail powered 1 gt later
   for 10 gt): the next batch rolls into the holds.

## Timed edges at the 4-gt edge bar (cc767b78's station judge 1e4e249c7, G1-approved with its follow-ups through c2f1473fd)

Each row the full lead matrix: REL T+3..7 x (control, cold start, restarts at every phase
T+0..150 x 10 orders), 7,560 runs; 0 failures at the run line (rest >= 34, landing lead 23).

| timed edge | run line | passing range | margin at the run line |
|---|---|---|---|
| trapdoor pulse | 12 | >= 6 | 6 gt |
| re-brake | 16 | 6..35 | 10 below, 19 above |
| R line length | 10 | >= 6 | 4 gt (the tightest) |
| R line length, R_1's built drive (`--r-repeater`) | 10 | >= 5 | 5 gt |

Outside: pulse 5, a trapdoor shuts onto its cart; re-brake 4..5, a departing cart stuck; re-brake
>= 36, the cold start lands within 4 gt of the re-brake; R line 5, the refill fails. Every edge
at the run line clears 3 gt + 1 scheduled tick.

R_1 with its built drive (ironspec 09:32; the emulated rows above put a redstone block in R_1's side
cell): a repeater (delay 1) at (x+1,17,1) facing west into R_1, on glass at (x+1,16,1), fed by the
controller's block at (x+2,17,1) for the R line's pulse. The station judge with `--r-repeater`,
REL T+2..6 x (the control, the cold start, 1,510 restarts: every phase T+0..150 x 10 orders),
7,560 runs a row: the run line's 10 gt 0 failing; 7, 6 and 5 gt 0 failing; 4 gt 7,560 failing
(each next cart starts but does not reach its hold). The bench reads R_1's own powered state after
every tick and each next cart's first move after the R line comes on: the rail powered T+56..T+66
at REL T+4 (1 gt after the emulation's T+55..T+65), every cart moving at T+57, no launch after its
power ended (ironfarm's bar for R) in any row. Keyed on leaving R_1's cell instead (ironfarm's and
tick-exact's rows, G1 on b4549f35a): every cart is off R_1's cell at T+60, 6 gt before the run
line's power ends (3 at 7 gt, 2 at 6, 1 at 5; at 4 gt it leaves at T+61, after its power: late);
the judge fails a row on either key, and the rows are the same: 5 gt passes, 4 fails every run.
The next cart rests on R_1 70 gt or more before its
R line in L1 (the lanes refill R_1 right after each R line), pressed on the hold cart's rider
(tick-exact's lane-handoff position): the bench's start, reloaded at every phase by the restarts.

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
- The sensors' phases (`--seeds 10`, no reload): in 1 of 10 seeds the summoners' golem sensor
  (every 200 gt) falls in the 20 gt after a crossing and re-detects the crossed golem from the
  stored living list (250 per 500 summons, every other cycle): no golem is in the overworld, the
  next batch never sees one, no summon is lost. The bench counts a sighting with no golem left in
  the overworld apart (G1's note on cc951af97); the bar counts a sighting while a golem is there.

- The bed stop's queue (b8949c96's module file, explore/farmopt-waycell f15c4a464, on rev 3
  e9eeeee52: the queue post's zombie in its boat at (6.5,-11,1.5), the queue rider in its cart at
  (8.5,-13.44,-5.5); its S2 still the old post): the golem flow on the module file, 500 summons
  over 2 days: PASS (every golem at its 2nd tick, 0 damage taken or dealt, the five posts alive at
  both dawns, 0 golems in the overworld at a summon tick, 0 sightings); the queue rider's golem
  attempts 0 in this bench (no napper walks the bed stop here). In the farm its 10-box (x
  -1.8..18.8, y -23.4..-1.5, z -15.8..4.8) covers the bed stop's walk from its entry (3,-16,-5) to
  its exit R (4,-19,4), and a napper there that slept wants a golem, so the queue rider can count 3
  (G1): its summons are harmless because no golem spawn cell lies within its search (SpawnUtil's
  LEGACY_IRON_GOLEM, +-8 / +-6; glass and glowstone excluded): 9fc3155f's flood with the queue
  rider as a caller ("queue: 0", "bedstop2: 0"), and my part flood of the module file with the
  rider's cell (8,-14,-6) declared: 48 spawn spots, all on the pad (golem feet -6, 8 over the
  rider: outside its +-6). The flood must re-run when the queue is re-laid east.
- Its model errors: 2,523 on main's redsim, all one gap (the sky light through a bubble column, at
  the queue post zombie's eye (6,-10,1) under the lift's column and at the daylight detector
  (-5,15,3), whose light range meets the column); 0 with onechunk's daylight.rs hunk (9f3d965a2:
  a bubble column dampens light like water), the same verdict.
- The east re-lay (explore/farmopt-waycell b01d2a4ed: the queue post's zombie in its boat at
  (15.5,-20,-3.5) in the east gap chunk, the queue rider's cart at (13.5,-13.44,-5.5) on its rail
  (13,-14,-6) on glowstone), with onechunk's hunk in the build: 500 summons PASS, 0 model errors,
  every golem at its 2nd tick, 0 damage, the five posts alive at both dawns, the queue rider's
  attempts 0, 0 cells set outside the world's box. The guard: the module file's part flood with
  the rider's cell (13,-14,-6) declared: 48 spawn spots, all on the pad (8 above the rider:
  outside its +-6); 9fc3155f's flood with the queue rider as a caller is to re-run on b01d2a4ed.
- ironspec's audit (11:05: a cell set outside a design-sized world is dropped): the bench's world
  is the design's extent + 4 (`watch_box`), and every post, part and controller cell is in the
  design, so the placement can drop nothing by construction. Both benches now count, per run:
  the design's cells outside the world's box (read structurally against the built world: the cells
  `place_raw` cannot write; the Nether part's too), redsim's records of those ("setBlock outside
  the box": `set_block_limit` records every non-air write outside the box, `place_raw` included),
  and the run's writes outside the box from those records; the station judge fails a run with any,
  golem-flow's verdict carries "WITH N cells dropped". Positive controls (G1 on 44d4bbfae): the
  world built 5 short of the design's top (`--box-short 5`): golem-flow 1 dropped (F3's roof
  (1,26,-5), a post cell; 1 record), drop-bench 6 (R_2's rails and glass at y 18; 6 records): the
  placement's records are not blind; a water source at the top of an empty column (golem-flow
  `--block=-6,16,-6,water`, drop-bench `--block 0,16,0,water`) falls out of the box's bottom: 8 and
  4 runtime writes recorded. 0 in every run of today's rows re-run (the golem flow's lead, S2's two
  positions, twin, 40 piglins, the blocked row, a restart, the natural row, both queue files; the
  panic measure with F3's part, the option (b) search, the R_1 rows, the restart sweep, the judge
  with the built drive).

## Option (b) for lane x -1 (`--panic-search 0`, bench)

A zombie post's cell inside the module's chunk (file x, z 0..15) with its seat 6.05..7.95 from
lane x -1's rider at rest and no rider ever nearer than 6.05 (2,858 cells with room, 428 clear of
riders, about 30 calling at all 400 pairs at each REL): the pick, **S2 at (-5,3,-5) open south**,
glowstone roof (-5,5,-5), glass floor (-5,2,-5) and walls (-5,3..4,-6), (-6,3..4,-5), (-4,3..4,-5),
in S1's column above S1's roof (-5,1,-5): its seat 6.85 from the rider at rest, the nearest any
rider comes 6.40, the nearest pad golem 8.78 (out of the boat rider's melee reach).

| REL | x -1 calls (dead-tick margin) | x 3 | x 7 | least callers | x -1's window in the pocket |
|---|---|---|---|---|---|
| T+2 | 400 (1) | 400 | 399 | 2 | 32..64 |
| T+3 | 400 (2) | 400 | 397 | 2 | 32..64 |
| T+4 | 400 (3) | 400 | 394 | 2 | 32..64 |
| T+5 | 400 (4) | 400 | 390 | 2 | 32..64 |
| T+6 | 400 (5) | 400 | 385 | 2 | 32..64 |

- Lane x -1 sees it 9 gt before landing (T+42 at REL T+4): its shortest window 32 gt against 17 with
  today's S2; lanes x 3 and x 7 unchanged. At REL T+2 its panic can start at T+40, 1 gt after the
  start that would put T on its dead tick; (-5,2,-5) gives margin 2 and window 30..63. Higher posts
  ((-5,4,-5), (-5,5,-5): windows 34, 36) start it at T+38..39 at REL T+2 and lose 15 of 400 pairs.
- The golem flow with it: 500 summons PASS; reloads at T+0 and T+1 give S2 no hit (the priced
  restart cost goes away). Not yet: the flood with its box, 9fc3155f's stroll and sight checks.
- **Adopted provisionally at (-5,2,-5)** (ironspec, the margin-2 variant; its floor is S1's glass
  roof (-5,1,-5)): the boat `minecraft:oak_boat` at (-4.5,2,-4.5) with a persistent zombie, glass
  walls (-5,2..3,-6), (-6,2..3,-5), (-4,2..3,-5), the glowstone roof (-5,4,-5), open south
  ((-5,2..3,-4) air); the old post (-5,-4,-2) and its blocks come out. Its rows, s 12 with S1 and
  F3 (8fb667bca): least callers 2 at every REL T+2..9; lane x -1 400/400 with the dead-tick margin
  2 at T+2 (to 9 at T+9) and its window in the pocket 30..63 gt; lanes x 3 400 and x 7 399 -> 364;
  the control (S1 turned away, no S2) 0. The golem flow: 500 summons PASS (every golem at its 2nd
  tick, 0 damage, the posts alive at both dawns); the 30 restart rows (10 orders x T+0/1/2) all PASS
  with S2's hits 0 (the nearest pad golem 7.81); the departing batch's re-detections after a reload
  0..35 per 500 (priced, no summon lost).

## Golem-stamp path (a) (ironspec 13:09 the bar, 14:11 the package, 14:16 the braked slope)

- The finding (onechunk): a departing rider still inside a next-batch caller's 10-box at the next
  summon (T + 100) is stamped GOLEM_DETECTED_RECENTLY until T + 700; with 7 in a lane's rotation that
  is exactly its next summon. The bar (RETIRED: ironspec 19:07 found its 20 gt inherited; the
  physical bar, below, is out of every caller box after its move on S - 3 = T + 97): every departing
  rider out of every caller box by T + 80. A
  pocket rider's box inflated by 10 spans x -10.8..17.8, z -9.8..10.8 and up to y 12.0125: only
  height clears it.
- The bench: `drop-bench --module <onechunk's rev 3 213bda2bf> --path-a --part <7b90c3f1's
  wip/lift/f1q2-lift-split-path-a.txt, 83878113f>` runs the assembled module (the merge row and the
  shared lift laid) with this station's own cells and the record lift file laid over it (without
  `--part`, `drop::PATH_A_LIFT` is a stand-in: its lift foot and powered merge-end ramp ran the
  napper 3 gt fast; with 7b90c3f1's file the napper's rail -> y 12 is 63 gt, tick-exact's official
  ride); the departing carts roll on and each rider is tested every tick against the three callers'
  boxes. `--hold-drive/--hold-rail` drive the braked slope's release (the hold rail held powered
  from the drive + 1, the declared `held:` stand-in above, where day-1's repeater (-4,12,6) from its
  input (-4,12,5) puts the rail's rise);
  `--napper-release` adds the napper's cart at its hold (9,0,8), its drive at (7,-1,8) through the
  module's repeater and slime (the rail at the drive + 1, as the official ride's pulse + 1);
  `--watch-cell` counts any cart or villager box meeting a cell (day-1's side-drive cells).
- Today's layout (rev 3): lane x 3 out at T + 80..84 (margin 0..-4 at REL T+2..6), lane x 7 at
  T + 100..106, stamped at T + 100 in 1300-1310 of 1310 runs per REL. Legs: launch to the station's
  exit 25 gt, the merge row 0.4 b/gt (the cap), the lift 0.695 b/gt (the cap), the lift's entries 21
  gt apart (the shaft bar): a single bottom-entry lift can't clear lane x 7 by T + 80.
- The package: (2c) lane x 7 climbs from loop z 3: four ascending powered rails (7,0..3,3..6) (a
  torch on glass at (6,1,4)), the curve (7,4,7), nine ascending rails west (6..-2,4..12,7), the first
  two powered (a torch on glass at (6,4,6)), the other seven plain (ironspec 16:46: with all nine
  powered the cart crested at a stored speed ~1.39, braked to rest at the slope's foot with its box
  in the column's water, crept on the release and, after a restart as it settled (REL + 83, all 10
  orders), rose under the lift wall's glass (-4,14,7) and stuck; plain, it crests at a crawl), the
  crest (-3,13,7) on the lift's glass (-3,12,7), the braked slope (-4,12,7)
  (`powered_rail[ascending_east]` in the lift's east wall, unpowered: the cart brakes to rest on it),
  an oak wall sign (-4,13,7) over it on glass (-4,13,6) (no collision box for the cart tipping onto
  the slope, the lift's wall glass there stopped it on the crest; dry: waterloggable, it holds the
  column's water); (2b) the merge row's west end ramped, (-2,0,7) and (-3,1,7) ascending powered
  rails (a torch on glass at (-3,1,6)), the intake (-4,2,7): lanes x -1 and x 3 and the napper enter
  the lift at y 2; (1) lane x 3's booster at z 3. The lift part's cells are 7b90c3f1's.
- Rows on the record files (bench; `--path-a --part <7b90c3f1's 83878113f>`; the pockets driven
  through day-1's repeater stand-ins from levers, the slope tapped through day-1's repeater
  (-4,12,6) laid, the slope rail held powered by the bench (no stand-in at its line end (-4,12,5)), the napper from a floor lever at (7,-1,8)
  on glass (7,-2,8); in rail-rise ticks: lane x 7's slope rail L7 = REL + 96 (ironspec 16:46 /
  18:16: the slot L7 / L7 - 32), the napper's rail L7 - 32 = REL + 64, the slope's pulse 18 gt).
  Restarts (6,550 runs: REL T+2..6 x a restart at every tick T+0..130 in all 10 orders, on the old
  block-only restart helper: they re-run on cc767b78's world-open model (N whole-chunk idle ticks,
  ironspec 23:12) before they are cited; the cart rows below hold without restarts too): every lane
  leaves its rest at + 2 (the bench's convention: the first tick the cart is more than 0.05 off its
  pocket's centre, after REL + offset; in tick-exact's official ride its Motion is nonzero after + 0,
  its position first changes at + 1 and its cell at + 5, every downstream tick matching; the rails
  rise on the same tick as with the old redstone blocks; wrong 0,
  contacts 0, stuck 0); the passes at y 12 lane x -1 REL + 54..55, lane x 3 REL + 73..74, lane x 7's
  column entry REL + 102 in every run (its rail + 6; tick-exact's official + 6..7), the napper
  REL + 126..127 (its rail + 63, official); lane x 3 -> lane x 7 >= 28, behind (lane x 7 -> the
  napper) 24..25, ahead (the napper -> the next batch's lane x -1 at REL + 154) 27..28; the lane x
  -1 -> lane x 3 foot pair (their lift entries) >= 19 (the shaft-entry bar 14 + 2 = 16, ironspec
  20:05), at the lift's top >= 17, lane x 3 -> lane x 7 at the top >= 31; lane x 7's dip back into
  a caller box >= 4 gt after T + 100; lane x 7 at rest on the slope from REL + 83 at x -3.286, its
  rider's feet 12.777 (official 12.72..12.98), drift 0, the slope dry, at the lift's top in every
  run; the arriving riders at rest >= 23 gt before the summon, landing >= 31 gt after their
  pocket's re-brake; 0 push contacts; air >= 299; day-1's 24 side-drive cells met by no box.
- Freezes (2,820 runs: logins of 1..3 gt and the pearl's 4 at every tick T+0..140; a freeze delays
  lane x 7 and the napper but not the next batch): the passes lane x -1 REL + 55..59, lane x 3
  74..78, lane x 7 102..106, the napper 127..131; lane x 3 -> lane x 7 >= 24, behind 25..29, ahead
  >= 23, the total window >= 48; the foot pair >= 16, at the top >= 14 and >= 28; the dip >= 4; at
  rest >= 19 gt before the summon; 0 push contacts. The napper's rail at L7 - 30 fails ahead (21);
  L7 - 33 passes (behind >= 23 in the restart rows, ahead >= 24). With lane x 3's offset 14
  (day-1's next grid point) the rows are in the commit's report.
- The out-of-box bar, physical (ironspec 19:07, replacing 13:09's inherited "by T + 100 - 20"):
  the panic path summons only at game time % 100 == 0 (vanilla `VillagerPanicTrigger.tick`; redsim
  brain/run.rs `if t % 100 == 0 { spawn_golem_if_needed(w, i, 3) }`) and a summon stamps every
  villager in the summoner's box inflated by 10 (`GolemSensor.golemDetected`, 600 gt; redsim
  spawn.rs), so a departing rider must be out of every next-batch caller's box after its move on
  S - 3 = T + 97 (1 tick for entity order + 2 margin), measured every tick in every row. Worst exits
  (inside last): lane x -1 T + 64, lane x 3 T + 79 (restarts) / T + 83 (freezes), lane x 7 T + 81
  (freezes): slack >= 13 for every lane.
- On the bench's stand-in lift cells (no `--part`) the napper ran 60 gt from its rail to y 12 and
  lane x 7 crested fast enough to rest at the slope's foot: the record file is the one to run.
- Inherited numbers these benches print against (ironspec 19:07 (6): labelled, not physical):
  the summon bar's margin of 2 callers (ironspec 05:26; the >= 1 caller is the game's rule); the R
  line's 10 gt against its measured threshold 5 (the margin >= 2, ironspec 09:32); cc767b78's
  REST_BAR = EDGE_BAR = 4 (3 gt + 1 scheduled tick, farmopt 02:39) for the landing lead over the
  re-brake and the rest before the summon (rows: >= 31 and >= 23); every "+ 2" over a physical bar
  (S - 3's 2, the column and foot-pair bars' 14 + 2). Retired: the T + 80 out-of-box bar (its 20 gt
  inherited; now S - 3, physical) and the y-12 23 / window 46 (now the column bar + 2).
- The direct summon check (`--summon-check`, ironspec 19:24..20:07; it replaces REST_BAR in
  cc767b78's judge): the module's hostile posts kept, the bench's villagers slept as the steady runs
  set it; at the due summon tick S = T + 100 (the bench is one cycle) the stand's three riders at
  the tick's start (seated in their pocket, wanting a golem, >= 3 willing in their 10-box, and
  their panic trigger ticking: VillagerPanicTrigger's 60-gt run is timed out without a tick at its
  end + 1, tick-exact's official skip at s + 61 + 62k), the GolemTry calls up to the first success
  and the class (golem born / spot miss / caller blocked by its own memory / fewer than 3 willing /
  no panicking caller); every golem born is removed after its tick. The gate: every due S with >= 1
  call from the stand; the control `--remove-rider K` (pocket K's arriving rider out) must show the
  miss; the golem rate per callers-per-S k beside 1 - m^k, m = (1 - 40..44/289)^10 (77.4-80.9 /
  94.9-96.3 / 98.8-99.3 % at k = 1 / 2 / 3), failing only when its binomial 99 % interval misses.
  The rate validates the bench's pad until the bench runs on record7 (`--out` writes the composed
  world for cc767b78's diff over the riders' search volumes, its stand-in cells listed in a
  `# stand-in:` header line); the large-N row is onechunk's. Rows (module rev 3, lane x 3's offset
  14, record lift file): 300 seeds x REL T+5 and T+6, no disturbance: 0 due S without a call from
  the stand, the three riders willing and ticking at every S (k = 3), 298 / 300 golems born (99.3 %,
  99 % interval 96.7..99.9 against 98.9..99.3 %: passes), 2 spot misses; each caller's search
  reaches 44 / 48 / 40 spots on the pad (P0 / P1 / P2) and none elsewhere. The controls (20 seeds x
  REL T+5, T+6, each pocket's arriving rider removed in turn): every S missed, "fewer than 3
  willing" (the two left have 2 willing each). The restart rows share one seed, so their per-k
  rates are correlated draws, not a binomial sample: they carry the gate (calls), not the rate.
  Restart rows (6,550, one seed): every S without a call (122..129 per REL) comes within 30 gt after
  a restart ("no panicking caller": the reload re-phases the panic); freeze rows (2,820): the 10 per
  REL without a call are the freezes over S itself ("frozen at S": no entity ticks, no summon that S);
  0 otherwise, so the gate holds at one summon per disturbance.
- The carts' block contacts (ironspec 21:25 (4)): every tick a cart's move is stopped horizontally,
  the blocks with a collision shape on the side it moved toward are listed per cell; a contact with
  a cell that is not declared fails the row. The station declares none for the bench (G1 08:18,
  cc767b78's staleness check: m14's six station stops, each pocket's slime back (x,0,-1) and each
  hold's north stop (x,17,-1), met 0 times in 26 runs; no bench row meets a station cell: each
  arriving cart falls straight from its hold into its pocket, z 0.490 throughout, and no row rolls a
  cart into a hold); this file's header keeps the holds' rest against the ring's north glass
  (x,16,-1) on tick-exact's lane-handoff row ca3cdb2ed (ironspec 02:41 (b): the cart 0.49 into its
  cell), whose runs meet it. The declared contacts are the `# contacts:` / `# stops:` cells of each
  `--part`, `--module` and
  `--declare` file's header (`cells=what`, cells `x,y,z` joined by '+', parsed strictly: a malformed
  cell refuses the header; a file without any prints "no contacts declared"; an entry whose `what`
  says PROVISIONAL or whose official row is pending is printed as provisional). The station riders
  hold HOME (the design's `port home`, the carousel's shared bed; ironspec 08:19): each row's start
  line names its HOME holders and the per-REL line counts them. INTERIM: the contacts move to ironfarm's
  shared face filter when it lands; until then these contact results are not cited. On module rev 3
  with 7b90c3f1's record lift file and on record7e every row meets the lift's foot entry back wall
  (-6,2,7) and lane 7's side-entry back wall (-6,12,7) / (-6,13,7), which 7b90c3f1's r3 declares
  (the foot pair provisional); record7e's own header declares ironfarm's lane-7 backstop (7,24,5).
- The sight check (ironspec 20:10 (2) / 23:30 / 00:27; G1's audit (1) and (5); tick-exact's 26.3
  citation: a mob's sight is one eye-to-eye segment clipped against collision shapes, fluids
  ignored): every villager -> scarer and villager -> villager segment, eye to eye, is walked cell by
  cell (a voxel walk) through the LIVE world's collision shapes and the reference's (the module of
  record's cells with every stand-in and `--block` override back to the cell it replaced; on the
  station layout the design of record given with `--record` (the build: a stand-in's support where
  the build has a collider hides nothing; record7f12 has glowstone at (1,16,1) and (5,16,1), under
  two of R_1's input levers), else the station as written), every tick from T to S read at the
  tick's end; a cell whose block TYPE differs from the reference's (a stand-in or override: dust ->
  lever, air -> glass) and clips the segment where the reference's doesn't is a crossing, a failure;
  a cell of the reference's own type in another state (a trapdoor open, a rail powered) is the
  design operating. The
  verdict comes only through the world: `--sight-control` lays a glass block in the world at T on
  the first live villager -> scarer ray (refused when that cell is already a full collider or no such
  ray exists), and its row must fail by the check. The scope is computed (the laid world differs from
  the reference somewhere at T, or the control runs); in scope with 0 rays judged is NOT JUDGED. The
  eye heights come from redsim's entity data (`MobType::def().eye`: villager 1.62, zombie 1.74,
  pillager 1.6575), 9fc3155f's sight-diff's data; a seated villager stays standing, its feet the
  cart's position.
- A departing rider missing from the lift tracking (removed or never spawned) without `--dep-riders`
  declaring it absent fails the row (G1 on 4a09c256).

## The checklist audit (ironspec Nov 13 04:38, G1's six additions): what the bench judges

- ONE failure list per row, `stationjudge::fails`, for the drop bench and the station judge alike,
  and the run exits 1 when any row fails it or a REL-level verdict fails (the golem rate; the
  record-match verdict with `--record`); the per-REL verdict block runs for every invocation, a
  single `--rel` included. NOT JUDGED counts as a failure wherever the bar is in scope, the scope
  computed from the run: the S - 3 bar and the column pairs (the world holds the lift but the run
  didn't track it, or a tracked lane tested on 0 ticks), the summon check (0 due S; a restart or a
  freeze over S), the golem rate (an empty or k = 0 bucket only), the held rails (no edge made), the
  sight check (0 rays).
- The bars: wrong pockets, push contacts between carts, hurt or unseated riders, a departing cart
  stuck, a trapdoor shut onto its cart, the refill, cells outside the box, R_1's late launches, the
  landing lead (EDGE_BAR, inherited) and the rest margin (REST_BAR, inherited); the S - 3 bar on the
  summon's own clock (S = the first tick after T at game time % 100 == 0; the stand's first
  GolemTry must fall on it) against the LIVE callers' 10-boxes (the arriving riders' boxes inflated
  by 10, each tick), and the stamps themselves (a departing rider whose GOLEM_DETECTED_RECENTLY this
  S's summon set, read from its memory after S's tick); the column bar 16 (ironspec 20:14) on every
  pair: the foot pair, both top pairs, lane x 7's entry after lane x 3's pass at y 12, the napper's
  pass after lane x 7's entry; the summon gate (a due S with no call from the stand; only the
  stand's callers' golems count; a stand GolemTry off the summon's clock fails) and the golem rate
  (its 99 % interval missing 1 - m^k's range, over the independent rows only: freeze, login and
  restart rows share their seed's draws, so a run of those alone leaves the rate out of its scope);
  undeclared contacts; sight crossings; a departing rider missing; the held rails (the hold check
  against the DECLARED window, REL + R + 1 ..= REL + R + W; the rail-T refusal on the driver's update
  cells); the hold rail wet or its cart through the hold before the release; the laid design's fluid
  cells each with its tick scheduled; overrides and lever edges that change nothing (04:07).
- Prints, not bars: the watched cells (`--watch-cell`, a probe), the riders' feet at T+80, lane x
  7's dip back into a box after S, the held cart's drift, the panic window and the napper's times.
- The controls judge themselves: a run with one passes only when the control bites in every run,
  else it exits 1: `--hold-poke K` (refused outside the held window; the hold check must fail),
  `--sight-control` (a crossing), `--remove-rider K` (the stand's call missed), `--lever-off K`
  (0..2; its cart stuck), `--rail-t-control` (the run stops at the on edge through the driver
  path: "a held rail's driver edge reaching a rail T"),
  `--rail-t-negative` (a T among the rail-kind cells printed, no rail-T failure), `--contact-control
  x,y,z` (an undeclared contact on planted glass), `--missing-control K` (a departing rider
  missing), `--rate-control` (every golem spawn spot blocked: the rate fails), `--push-control`
  (lane x 3 released with lane x -1: a push contact),
  `--foreign-caller-control` (ironspec 06:55 (c): every pocket rider dismounted 5 gt before S,
  keeping its brain and panic: its calls at S are not the stand's, and the gate must read a miss
  while those calls happen), `--reseat-control` (G1 on a3b306a05 (3): a fresh villager, a new id,
  seated in pocket 0's cart 40 gt before S, the old rider dismounted beside it: no miss at S, the
  fresh one's call attributed to P0 by its id, the old one no pocket's), `--fluid-control` (the first
  laid fluid cell left unscheduled: the fluid bar, read back from the scheduler, must fail),
  `--t-control` (a lever on glass planted beside the first station T, switched on at T+5 and off at
  T+10: the T-shape bar must fail, its first re-shape at T+11 or later), `--activity-control`
  (rider 0's activity set off its schedule before the t1 read: G1 on 834d6329d (T3)). Every control
  bites through the failure list (its own failure string, or for a negative control its absence)
  and the exit status; every other failure of a control run is its declared collateral or fails the
  exit (G1 on 834d6329d (T1)); `--watch-control` (pocket 0's rest cell met) is an instrument print;
  the guard's refusals are unit tests (`guard_refuses_a_differing_stand_in`).
- The T-shape bar (farmopt 09:46, ironspec 09:44): every plain rail with >= 3 potential connections
  in the world at T (a T: the pocket-exit joins (-1,0,7) and (3,0,7) among them) is recorded with its
  shape and checked at every tick's end; any change is a runtime re-shape, a failure; the Ts and
  their shapes print in each run.
- The hold check reads the station's DECLARED held phases (`drop::HELD_SLOPE` REL + 96 ..= REL + 113,
  `drop::HELD_NAPPER` REL + 64 ..= REL + 73, this header's `held:` lines); a CLI drive window that
  differs is refused (G1 on a3b306a05 (4)). The S - 3 bar on redsim's tick semantics: the bench's
  iteration dt runs game tick T + dt + 1, its post-tick state that tick's end; the end of S - 3 is
  iteration S - 4, so a rider's last inside iteration may be S - 5; a tick without all three callers
  is no sample.
- The summon's accounting (ironspec 06:55 (c)/(e), G1 on cc767b78's call gate): each GolemTry is
  attributed by its villager's entity id against the pocket's seated rider, read from the world;
  `calls` and `born` count only those; each due S carries the villagers counted willing in the
  pocket riders' 10-boxes (`willing_in_box`: entity id, position, whether a pocket rider; a
  non-pocket villager among them is named in the riders' lines) and the non-pocket calls
  (`foreign_calls`). The setup (G1 item 9): redsim's Run sets the day time before it spawns the
  villagers; the bench gives the design's villagers LAST_SLEPT = the summon day's dawn + 10 (they
  slept the night before) and asserts each station rider's activity at T against its schedule's
  (WORK and MEET fall back to IDLE without a job site or meeting point; PANIC while scared), a
  mismatch failing the row; each pocket rider's LAST_SLEPT age and activity print at S.
- station-judge prints the verdicts its world can't reach as NOT JUDGED (no lift, no held stand-in,
  no summon check, no reference world for the sight check); its rows are never cited for them.
- The at-entry bar's station half (ironspec 08:23, G1 08:22): every station rider's activity is read
  every tick (the transitions printed) and each entry into REST is recorded with its
  GOLEM_DETECTED_RECENTLY stamp, the first tick's REST (a night world open's blip) included;
  drop-bench and station-judge print the count (labelled: a printed count, not judged, until
  b8949c96's SetWalkTargetFromBlockMemory hook lands). `--day-time D` (both tools) sets the runs'
  day time at their start; a night value (12000..23999) samples the REST entries.
- Ranges: a run whose ticks end before its last judged tick (S, the hold window's end, the
  napper's pulse's end) is refused, as are a hold window outside the run (`--hold-width` > 0),
  `--hold-poke` outside it, `--lever-off` / `--remove-rider` / `--missing-control` outside 0..2. The
  defaults are the rows' (`--hold-release 95`: L7 = REL + 96; `--hold-width 18`: day-1's pulse).
- Tolerances, each with its reason at its site: 0.05 off the pocket's centre (the "+2" launch
  convention against tick-exact's official ride); 0.01 off a rest position (a cart at rest keeps
  its position exactly; a powered rail's first push moves it more); under 0.01 of motion at the
  hold (the braked rail halves motion each tick); the contacts' 0.05 widening, 1e-6 motion sign and
  0.01 inset (interim, until the face filter); push contacts at a clearance under 0.05.

## Open

- The bed stop's queue rider (loop (9,-13,-6)) and its zombie post (9,-7,-3) in the panic measure
  and the golem flow, once b8949c96 lays them in rev 3's loop frame; 9fc3155f's S1 merge-row
  occluder in the caller measure.
