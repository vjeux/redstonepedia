# day-1's controller NETLIST (V rev 3.1, normal operation only)

The controller is laid in five subsystems, each its own small car (settle, power-out and update-reach checks, then G1,
then ironspec; vcompose composes V rev 3.1 as each lands). day-1 owns this netlist and the interface file
(f1q2-day1-controller-interface.md: the SOURCE of every timing value; this netlist names the values and lists cells, and
where it quotes a number the interface wins). Scope: NORMAL OPERATION ONLY (ironspec
11:20a): no cells for faults, empty carts, repairs, jams, long pauses (OUT: the resync net, the claim cap, the store
guard, the token's recovery and A_SEEN, the record's resync, the premature-release latch, the merge-row admission count,
gate G's foot logic, RETURN ROOM unless a normal run fills the return row, HELD, the room count, FRONT EMPTIED).

Conventions: coordinates in V's loop frame; a diode's `facing` names its INPUT side (26.3); "edge" = a rising-edge pulse
former (a comparator in subtract mode: the signal into its back, the same signal through a delay-2 repeater into its
side: a 4-gt pulse), "latch" = a torch RS latch (states and scheduled ticks persist across save / load), "level" = a
plain signal. Each subsystem's FILE (its car) lists each of its cells with its six neighbours, its power-out and
update-consumers (part-60 form), and shows no always-on source (torch, powered block) beside any dust or repeater input
of a controller line; this netlist names the interface cells (each signal's sensor and consumers) and the cells kept as
named below.

## Signals shared between subsystems (the interface)

| signal | from | to | form | timing (the interface's) |
|---|---|---|---|---|
| REL | core | lines, the record | a 2-gt pulse on the controller's spine | once per period when the store is armed and the holds are full (the holds' sensor: the core names it; V's station has no hold strings since ironspec 23:01) |
| REL + k taps | core | lines, the arbiter / record | repeater-chain taps off REL | the slot releases (line C's ports at REL + k - 4), the napper hold (line A's lever at REL + 61), the record's write / shift (REL + 102 / + 61), G's capture / clear (REL + 92 / + 33) |
| POP(l) | the station's D (x,0,2) (A6 stripped) | picks (the lane's ring steps), core | the detector's press | once per REL per lane |
| ENTRY(l) | the S-bend seat detectors D(x) (x,24,8) | picks (the positional release), core (credit, SEEN3, the K masks) | each detector's level, read through its own repeater facing away | the lane-7 napper's press merges into its batch cart's under the 20-gt hold |
| SIDING(l) | the napper at its lane's R hold (x-2,24,7) | picks (the release's AND) | presence | while a napper waits |
| R_REL(l) | picks (the positional release) | line B (the R holds) | a pulse | at the napper's predecessor's ENTRY |
| REQ(l) | picks (each ring) | the static-priority gates (cc767b78) | a level | from the pop's step to the lane's next pop |
| W7 | the gates (cc767b78) | the gates' next decide | a latch | written from G7 at REL + 84, read at REL + 92 (holds through a freeze) |
| REL + 40 / 57 / 67 / 80 | core (dfb89499's chain; + 40 new) | each ring's filler window (40 / 57 / 67 for lanes -1 / 3 / 7) and up-fire (67 for lane -1, 80 for lanes 3 / 7) | tap pulses | exact (REL-keyed) |
| GRANT(l) | the gates' G (cc767b78) | line `pocket:<lane>` (the pick drive), the lane's ring (T's window, the filler's window, the up-fire) | a level | captured at REL + 92, held until REL' + 85 (cc767b78 13:18) |
| ARM3 | core (the napper being taken at (9,0,8)) | the record (its shift) | a press | REL' + 57 |
| HOME(l) | the record's head (cc767b78) | core (the K(l) selection, latched at REL' + 57) | one-hot level | valid by REL' + 57 |
| K(l) | core | the lift's J(l) T's (branch), the D(l) masks | a window pulse riding the napper's release: [napper - K_EARLY, napper + K_HALF] | K_EARLY, K_HALF, R6 (the interface's); the napper reaches J(l) NAPPER_T - SLOT_T[2] behind slot 3 (ctl-credit) |
| RS_ready | the ball-exit detector (7,-3,4) | way-in (the front's release) | a latch set on its rising edge, reset at the front's departure | once per visit |
| REST / DAY | core (its day counter and daylight detector, inside the chunk) | the gates' WIN, way-in (the front's release) | levels | the core's decode |

## Subsystem 1: THE CORE (REL / store / credit + K windows) -> dfb89499 (after its original-station normal run)

File: `designs/farms/iron/carousel-assembly/wip/controller/f1q2-day1-ctl-core.txt`.

- THE STORE: a counting hopper (non-stackable credit items, ONE ballast of its own type in the last slot), a dropper
  feeding it, its comparator and a 4-dust read line; ARMED = the read at the 4th dust lit (comparator 3 / 6 / 9 / 12 for
  0..3 credits -> the read 0 / 3 / 6 / 9: unlit only on the ballast alone). REL opens the hopper's unlock window for
  UNLOCK_W = 4 gt (< the 8-gt transfer cooldown: one item per REL back into the dropper).
- THE CREDIT: D(7)'s rising edge AND SEEN3 fires the dropper once (one item into the hopper); SEEN3 = a latch SET by D(3)
  (masked by K(3)), RESET by every credit; DM1_BLOCK7 (35 gt, the one-credit-per-batch guard) and SEEN3_OLD's own tap are
  KEPT only if the normal battery shows them needed (day-1 is running it now).
- REL (G8) = the clock's slot (P = 100) AND ARMED AND the holds full (the station's lane holds; the lanes' slot-1 carts
  present) -> one 2-gt REL pulse; its taps (above) feed the lines.
- K(l) for each napper (lanes -1 / 3 / 7): a window pulse [napper's J(l) arrival - 5, + 10] keyed on the napper's release
  (ARM3 REL + 57 -> the hold rail REL + 65 + R6's shift): holds J(l) in BRANCH and masks D(l) (all of D(l)'s outputs).
- THE RING RESET (G10) at REL + 75.
- Normal rows (ctl-credit, the reference): the normal run (25 batches, returning nappers in every lane, outbound picks of
  every lane): P = 100, every batch in phase, the store's bars, no cart misrouted; A WORLD OPEN at every tick of t
  400..719 (idle 0 / 1 / 2, the carts 1 / 3 behind) on both trains, the nappers on time and 2 gt early: 0 failing rows;
  R6's physical constraints over the swept train-16 runs: (i) 5, (ii) 3, (iii) 25 (minimums 0 / 0 / 1); the store's read
  per count; the planted ballast rows. Wired vs bench: the REL ticks, the credit ticks and the hopper's count per batch.

## Subsystem 2: THE PICKS (the (i-b) per-lane item rings; static priority and the record -> cc767b78) -> day-1

Files: `designs/farms/iron/carousel-assembly/wip/controller/f1q2-day1-ctl-picks.txt` (rings 1 and 2, day-1; ring 3, mob-AI,
the parent 13:47, from ring 1's file); the mechanics and their bench: `crates/mobfarm/src/ctlring8.rs` (ring-local cells,
one FLAT ring per lane). PLACEMENT (ironspec 13:28p, vcompose / 9fc3155f): ring 1 = B-low (x -3..8, y 5..8, z -6..-1),
ring 2 = B-high (y 9..12), ring 3 = C' (x -4..7, y 17..20, z -6..-2: the 5-deep variant, its southmost row dropped);
ring 1 keeps (-3,5,-2), (-3,5,-1), (-2,5,-1), (-3,6,-1) AIR (the shaft zombie's rays to the lift riders), and cc767b78's
cells (-1|2|5, 5, -4) glass or air (above its record's lock torches), with glass over its exposed code-column tops
(-2,5,-6) / (-2,5,-2). World = ring-local + (3, 7, -3) for ring 1 (+ (0, 4, 0) for ring 2). cc767b78's priority gates and
record: BOX A (x -3..8, y -3..4, z -6..-2), the 14 sight cells collider-free.

- Per lane, A FLAT ITEM RING (ironspec 11:56a / 12:02p; ctl-pick's sensed model `sensed` + `positional_release`): 8 droppers
  and 8 hoppers at one level in two rows (the north row flowing east, the south row west; P0..P7 the droppers in flow
  order), every PRESENT slot holding a filler (stone: the tap reads 1), the TARGET slot T (a wooden shovel: reads 2), an
  AWAY slot empty (reads 0). A dust patch over the 16 cells is the STEP (and the golem cap).
- Its signals (each with its sensor -> consumers):
  - POP(l) (the station's D (x,0,2) press, line D) -> STEP (6 gt later: the POP line's delay) and T's window;
  - STEP = POP delayed 6 gt OR SKIP -> the dust patch (every ring dropper fires one item on; the hoppers carry it on);
  - TAP(l) = the tap comparator on P2's north face (P2 the tap): 2 / 1 / 0 for T / filler / empty -> REQ, EMPTY;
  - SKIP = POP delayed 20 gt AND the tap EMPTY at that tick -> STEP (the away slot passed);
  - SPACING = a latch SET by G_l's rising edge, RESET by T leaving the tap (TAP falls from 2) while G_l is low -> REQ;
  - REQ(l) = TAP reads 2 AND NOT SPACING -> cc767b78's static-priority gates (a level);
  - G(l) (cc767b78: captured at REL + 92, held until REL' + 85) -> the pick drive `pocket:<lane>` and the ring:
    T's window = G_l AND the POP edge, 22 gt (T's extractor under P2 lifts T before the step and pushes it, one hop, into
    the up-dropper under P6's feeder; P6 = the tap + 4, where the slot picked + 5 sits after the pop's step); the
    filler's window = G_l AND the lane's REL' + 40 / 57 / 67 tap, 10 gt (the extractor under P6 sends P6's filler to
    the stock); the up-fire = G_l AND REL' + 67 (lane -1) / REL' + 80 (lanes 3, 7) (T into P6's feeder, so into P6);
  - ENTRY(l) (the S-bend seat detector D(x) (x,24,8)), SIDING(l) (the napper at its R hold (x-2,24,7)) and P4's state
    (P4 = the tap + 2, read by a comparator) -> RELEASE = ENTRY AND P4 EMPTY AND SIDING -> the stock's fire (one filler
    into P4 through a relay hopper and P4's feeder) and R_REL(l) (line B).
- Its locks: each extractor is locked by a SUBTRACT comparator whose rear is a chest holding one stone (reads 1) and
  whose output powers the hopper (T's through its powered support block, which also carries the tap comparator); a
  window is 15 on the comparator's side. QUASI-CONNECTIVITY (a dropper is powered through the block above it; every ring
  cell is powered at each step): the stock sits east of the ring under a relay hopper with no powered neighbour; the
  up-dropper sits under P6's feeder, so T reaches it only after the step's pulse (its push is held by the step's power
  on the tap), after the up-dropper's quasi-connected fire (step + 6; measured margin 4 gt).
- ONE PICK PER REL = STATIC PRIORITY + THE WIDENING (b'+w), cc767b78's gates: g_-1 = REQ_-1 AND NOT W7; g_3 = REQ_3 AND
  NOT (REQ_-1 AND NOT W7); g_7 = REQ_7 AND NOT (REQ_-1 AND NOT W7) AND NOT REQ_3; W7 = a latch written from G7 at REL + 84,
  read by the next decide at REL + 92; G captured at REL + 92 (gated by WIN = REST AND NOT DAY), held until REL' + 85.
- THE RECORD (cc767b78's (a') compacting registers): written from G at REL + 102; SHIFTED at REL + 61 in a REL whose ARM3
  fired (RLS not read); HOME_l = its head, latched by the core at REL' + 57; HOME's default when empty: LANE 3 (codes 7 =
  (1,0), -1 = (0,1), 3 = (0,0); ironspec 13:58p). Depth: 3 stages (at most 3 entries alive at once, measured per tick at
  the measured stop -> (9,0,8) trip 113..126 gt, each end +-2, over the (b'+w) rows).
- OUT / superseded: HELD, the room count, FRONT EMPTIED, bunched picks as a normal row, the pointer's spacing counter
  (the latch replaces it), the record's resync, the mark FIFO, the X tokens (an away slot is an empty dropper).
- Normal rows: ctl-pick (the sensed model; the (b'+w) rows: the seven sweep points, 129 true whole-farm freezes, the 27
  +-2 gt leg corners, the positional release): 24 distinct every night, the least H2 arrival gap 64 gt at the corners
  (over the 41-gt cycle), the record's depth 3; ctl-ring8 (the ring's mechanics in redsim): the 16-step reference with a
  grant and a release, the skip, a world open at every tick of the grant and the release (199 rows), the +-2 gt corners;
  the whole-lane bench (driven by the lane's inputs only) with the logic.

## Subsystem 3: THE WAY-IN ((B)'s latch, hold and read, the token, the P1 trigger, the front's release) -> b8949c96

File: `designs/farms/iron/carousel-assembly/wip/controller/f1q2-day1-ctl-wayin.txt`.

- THE SECOND HOLD H2 (9,24,7): `powered_rail[ascending_south]`, braked; driven by the x-8 path: stone (8,24,7) under dust
  (8,25,7), a straight north-south dust run along x 8 at y 25 on glass (8,24,6..); keep-free: no rail may enter
  (7,23..25,6) or (8,23..25,7); (8,23,7) keep-empty; lane 7's merge C (7,24,7) is a 2-connection plain curve.
- THE ARRIVAL READ: `detector_rail[ascending_south]` (9,23,6), read by dust (8,23,6) on glass (8,22,6) (a corner east +
  north), its line north along x 8 at y 23 on glass (8,22,5..3), ending at (8,23,3) (its pointing target (8,23,2): air).
- KEPT AS NAMED (G1 11:43): (8,23,2) AIR or GLASS, never a conductor (booster 2's lit torch (8,22,2) strongly powers it,
  and a conductor would hold the read's end dust (8,23,3) on: H2 never opening); (9,22,6) GLASS (the detector's support:
  a pressed (9,23,6) strongly powers it; the pressed detector's six neighbours: (9,22,6) inert glass below, (8,23,6) the
  read (west), (10,23,6) (east: air in V rev 3.1; kept non-conductive), (9,23,5) (north: air), (9,23,7) (south: the hold
  column's rail support, inert), (9,24,6) (above: air); the hold (9,24,7) is diagonal); (9,25,8) H2's upper connection, a PLAIN rail (no powered rail there: one would be powered
  whenever H2 is); (8,25,8) the drive dust's south pointing target: AIR; (2,-1,8) line A's input: KEPT FREE until the
  core's port lands. Ownership in rev 3.1: the (B) cells (9,24,7) / (9,23,6) belong to vcompose's stand-in part (owner
  day-1, then b8949c96's way-in car); b8949c96's delta and V's leg cede them by `# stand-in:` marks (G1's pick).
- THE LATCH (P1 OCCUPIED): SET by the 4-gt edge of the read; RESET by P1's exit detector (9,22,3), a level,
  reset-dominant. OCCUPIED = the latch delayed D = 16 gt (two delay-4 repeaters). HOLD OPEN = NOT latch AND (9,23,6)
  unpowered AND (9,22,3) unpowered -> the x-8 drive.
- THE TOKEN: a latch SET by the edge of the queue's departure detector (9,-11,-3), CLEARED by the P1 release pulse
  (reset-dominant). START-UP: the token SET (free), the latch 0, H2 open.
- THE P1 TRIGGER: the rising edge of TOKEN AND FRONT CLEAR ((9,-9,-5) unpowered) AND OCCUPIED fires one 12-gt pulse into
  P1 (9,22,5) (braked; powered = released).
- THE FRONT's RELEASE (the queue front (9,-10,-4), braked): RS_ready AND the queue hold (80 gt from the front's arrival
  press) AND REST AND NOT DAY.
- FRONT EMPTIED: DROPPED (the room count is superseded; nothing reads it).
- Normal rows: ctl-pick's (B) rows (the lag from the RESET level's fall to the next SET > 0 over the normal rows and the
  planted three-in-one-REL rows, its control the hold without its (9,22,3) term; no napper behind H2), b8949c96's redsim
  way-in bench on these cells (3 nappers 20 gt apart, a slow 0.1 b/t arrival; the measured (9,23,6) -> P1 transit 9..11
  gt), world reloads and unloads at every phase.

## Subsystem 4: THE DRIVE LINES -> loaderlay

File: `designs/farms/iron/carousel-assembly/wip/controller/f1q2-day1-ctl-lines.txt`.

- LINE A, THE (9,0,8) NAPPER HOLD's line (the hold rail rises at REL + 67; its input (2,-1,8) thrown at REL + 61, 6 gt
  measured): `repeater[facing=west]` (3,-1,8) (input (2,-1,8)), dust (4,-1,8), (5,-1,8), (6,-1,8) (east-west only),
  `repeater[facing=west]` (7,-1,8) into LIFT's (8,-1,8); glass (3..7,-2,8); the cells above (3..7,0,8) air. (Gate G's
  free-place term: OUT.)
- LINE B, THE R-HOLD RELEASES (the S-bend's napper holds R(x-2,24,7)): the ring's positional release (subsystem 2) at the
  napper's predecessor's entry; the line ends per the lanes file: lane -1 a delay-3 repeater (-4,22,9) outputting east
  into the slime (-3,22,9); lane 3 a repeater (2,22,9) outputting west into the slime (1,22,9); lane 7 the feed path's
  input (7,23,9).
- LINE C, THE STATION's SLOT RELEASES (R_1 at (x,17,1)) at REL + 0 / 17 / 27 (loaderlay's car): the drivers
  `repeater[delay=1,facing=east]` (x+1,17,1) (day-1's, with their 6 `# update-consumers:` declarations), the end dust
  (x+2,17,1) (west + north, on the keep-block glass (x+2,16,1)), the arrival `repeater[facing=north,delay=1]` (x+2,17,0)
  on glass (x+2,16,0), the PORT (x+2,17,-1) rising at REL + k - 4 (4 gt to R_1, every lane).
- LINE D, THE STATION's D (A6 stripped, dfb89499 12:30): every lane's D is (x,0,2) (`detector_rail[north_south]` on glass
  (x,-1,2)); F′ lays no tap; the old tap cells (x+1,-1,5) / (x+2,-1,5) and (1,-1,5) / (5,-1,5) are VOID. D is the ring's
  POP input (its press) and the core's: each reader reads D from its own cell's neighbours (power-out listed).
- LINE E, LANE 7's SLOPE RELEASE (restored, ironfarm / loaderlay 14:05; it was dropped from this netlist by mistake): lane
  7's rest is the BRAKED powered slope (-4,12,7); a repeater at (-4,12,6) (its input (-4,12,5)) powers it for 18 gt from
  the core's REL + 95 tap (every bench throws REL + 95 / 96 for 18 gt; dfb89499's M line has lane 7's column entry at
  T + 107). The core provides the REL + 95 tap (an 18-gt pulse); LOADERLAY's lines car lays the line from that tap to
  (-4,12,5) and the repeater (-4,12,6) (its CTL stand-in lever (-4,12,6), `slope-release:7`, tagged owner LINES, drive =
  the core's REL + 95); priced in the lines' tally (~3 cells + the route).
- THE STOP's INPUT CELLS (58f770cc 11:42 / 12:06, the re-lay car's latch-form stop, V = stop-local + (0,-20,-1); NO slope
  hold (the exit hold removed 05:33 / 06:11): NO tap line into (1,-21..-23,3..4); NO PARK line (PARK is made inside the
  stop file: the latch's Q -> the OR line -> its repeater -> the park slime): K's drive under K's slime V (-2,-22,2); the
  shutter's drive V (-5,-21,6) (the shutter at V (-5,-21,7)); RLS_IN V (-1,-22,-3) and ARM_IN V (6,-22,-6) (the latch row
  at V z -6): all subsystem 5's outputs, so line E carries no stop end. KEEP-FREE: V (1,-21,-1) and V (0,-21,0) (the OR
  line's step-down links). These stop cells are 58f770cc's RE-LAY car's (c0662d566, its latch file md5 2e57375162d6), NOT
  V rev 2.1's FOLD placeholder (which no stop figure cites on). R1's inputs in V (the queue raised 4; b8949c96's bench
  stand-ins in V rev 2.1, not laid): REST_l (3,-10,-4), DAY (4,-10,-2), NOT-ready (8,-10,-5).
- Normal rows: each line's delay from its tap to its target printed against the interface's timing; world reloads at
  every phase of each pulse.

## Subsystem 5: THE STOP's LOGIC (ctl-shutter's form of record) -> mobviewer (the parent 12:33; 58f770cc keeps the stop file
## and reviews the interface)

File: `designs/farms/iron/carousel-assembly/wip/controller/f1q2-day1-ctl-stop.txt` (or inside 58f770cc's stop file).

- From ctl-shutter (crates/mobfarm/src/ctlshutter.rs, its header): the release keys on sustained presence, anchored at the
  bed-head pulse: SINCE (S = the pulse AND P_STRING's onset, R = NOT P_STRING), WIN (S = the pulse through the D line 8,
  R = the pulse or the pulse through the W line 48), PARKED = SENSED AND COOL, RLS = P_STRING AND SINCE AND WIN AND SENSED
  AND COOL (one pulse a visit); SO (slept once: S the pulse, R RLS); OPEN (S = PARKED AND SO AND NOT P_STRING; R = RLS OR
  P_STRING), SHUT = NOT OPEN. SENSED (ironspec 11:57a, accepted): set by the PARK STRING's first press after ARM (V
  (1,-19,2)), through d taken from the string's measured press -> rest band at +-2 gt (58f770cc prints it); reset at RLS
  or ARM's edge. COOL: set by ARM's edge through the 52 tap, reset at RLS or ARM's edge. PARK is the stop file's own
  (the latch form: [RLS + 2, RLS + 26)).
- Inputs (V's frame, 58f770cc 11:55): the bed-head observer pulse V (-3,-21,3); P_STRING V (-2,-19,3) through its east
  hook V (-1,-19,3); the park string V (1,-19,2); ARM's sensor: the detector V (-2,-20,0) on glass V (-2,-21,0). (The
  latches' cells are mobviewer's file.)
- Outputs: the shutter's drive V (-5,-21,6); K's drive V (-2,-22,2); RLS_IN V (-1,-22,-3); ARM_IN V (6,-22,-6). (The record
  no longer reads RLS: its shift is keyed on ARM3.)
- The interface hazards (58f770cc 12:35): RLS_IN V (-1,-22,-3) is a HUB of three latch dusts: driven as a SOLID block,
  strongly powered by a repeater from V (-2,-22,-3), never dust; K's drive powers the slime V (-2,-21,2) STRONGLY (a torch
  at V (-2,-22,2) or a repeater into the slime); P_STRING is read from the WEST hook's support, glass V (-4,-19,3), never
  the east hook's V (0,-19,3) (beside the iron trapdoor V (0,-19,4) carrying a guard rail); no reader dust beside the park
  rail V (1,-20,2); the bed-head observer V (-3,-21,3) outputs down into the free V (-3,-22,3); ARM's detector rail
  V (-2,-20,0) on glass V (-2,-21,0); keep-free V (1,-21,-1) / V (0,-21,0). K's SET: 58f770cc's RLS + ~30 form allowed
  (ARM's edge only as the power-up set); ironspec rules.
- Normal rows: ctl-shutter's normal rows (every phase of a visit, world opens), wired vs bench.

## ETAs (as each owner states them)

- picks (day-1): first car ~5 h from 11:35 PT.
- core, way-in, lines: their owners state theirs.
