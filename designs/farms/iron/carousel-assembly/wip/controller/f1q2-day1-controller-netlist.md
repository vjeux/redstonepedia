# day-1's controller NETLIST (V rev 3.1, normal operation only)

The controller is laid in four subsystems, each its own small car (settle, power-out and update-reach checks, then G1,
then ironspec; vcompose composes V rev 3.1 as each lands). day-1 owns this netlist and the interface file
(f1q2-day1-controller-interface.md: every number cited here is stated there once). Scope: NORMAL OPERATION ONLY (ironspec
11:20a): no cells for faults, empty carts, repairs, jams, long pauses (OUT: the resync net, the claim cap, the store
guard, the token's recovery and A_SEEN, the record's resync, the premature-release latch, the merge-row admission count,
gate G's foot logic, RETURN ROOM unless a normal run fills the return row).

Conventions: coordinates in V's loop frame; a diode's `facing` names its INPUT side (26.3); "edge" = a rising-edge pulse
former (a comparator in subtract mode: the signal into its back, the same signal through a delay-2 repeater into its
side: a 4-gt pulse), "latch" = a torch RS latch (states and scheduled ticks persist across save / load), "level" = a
plain signal. Every layer lists each cell with its six neighbours, its power-out and update-consumers (part-60 form),
and shows no always-on source (torch, powered block) beside any dust or repeater input of a controller line.

## Signals shared between subsystems (the interface)

| signal | from | to | form | timing |
|---|---|---|---|---|
| REL | core | picks, way-in, lines | a 2-gt pulse on one dust line along the controller's spine | once per period (P = 100 gt) when the store is armed and the holds are full |
| REL + k taps | core | lines | repeater-chain taps off REL | k = 0 / 17 / 27 (the station's slot releases), 57 (ARM3), 67 ((9,0,8)), 75 (the ring reset), 80 (SEEN3_OLD's tap: OUT if the normal run shows it unneeded) |
| D(l) | the S-bend seat detectors (x,24,8), x = -1 / 3 / 7 | core (credit, SEEN3, the K masks), picks (the ring) | each detector's level, read through its own repeater facing away | the lane-7 napper's press merges into its batch cart's under the 20-gt hold |
| K(l) | core | the lift's J(l) T's (branch), the D(l) masks | a window pulse riding the napper's release: [napper - K_EARLY, napper + K_HALF] | K_EARLY = 5, K_HALF = 10, R6 = 14 (the napper 14 gt behind slot 3) |
| PICK(l) | picks | lines (the lane's pick drive at the station) | a pulse per granted pick | at most one per REL (the pick-rate arbiter) |
| HOME | picks (the record's head) | lines (the stop's slope tap / RETURN bit) | a 2-bit lane code held until the napper's release from the stop | written per pick, read per release |
| FRONT EMPTIED | way-in (the front's arrival detector (9,-9,-5) falling after its 20-gt recheck) | picks (the room count) | a pulse | once per napper leaving the front |
| RS_ready | the ball-exit detector (7,-3,4) | way-in (the front's release) | a latch set on its rising edge, reset at the front's departure | once per visit |

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

Files: `designs/farms/iron/carousel-assembly/wip/controller/f1q2-day1-ctl-picks.txt` (the three rings, day-1, in vcompose's
BOX B: x -3..8, y 5..12, z -6..-3, the rings at x -3..-1 / 1..3 / 5..7); cc767b78's arbiter + record file in BOX A
(x -3..8, y -3..4, z -6..-3), its overflow in BOX B's columns x 0 / 4 / 8 (agreed with cc767b78 and vcompose).

- Per lane, AN ITEM RING (ironspec 11:56a / 12:02p; ctl-pick's sensed model): 8 droppers in a dropper -> hopper cycle (the
  counter-ring generator's form; hopper tops sealed, tokens resting in droppers), holding the target token T (unstackable:
  the tap reads 2) and the absent markers X (stackable: 1). The ring STEPS on the lane's SENSED POP (the station's D
  (x,0,2) press); an X at the tap steps it once more (the away slot skipped). REQ_l = the tap reads T AND NOT the spacing
  latch (set at the grant, cleared by T's first pass: the >= 11-pop spacing). On a GRANT: a T-filter hopper under the tap
  lifts T into the dropper 5 steps upstream (TARGET + 5), and an X goes into the tap slot from the X stock dropper.
- THE REJOIN (the positional release, ctl-pick's `positional_release`: 0 differences from the identity rule): at a circuit
  ENTRY (the S-bend seat detector D(x) (x,24,8)), an X in the dropper 2 steps downstream of the tap AND the siding's
  presence (a napper at its R hold) -> the R release for that lane (line B), and the X-filter hopper under that dropper
  lifts the X back to the stock. (No mark FIFO, one X type, no flags.)
- ONE PICK PER REL = STATIC PRIORITY (ironspec 12:30p): g_-1 = REQ_-1, g_3 = REQ_3 AND NOT REQ_-1, g_7 = REQ_7 AND NOT
  REQ_-1 AND NOT REQ_3 -> G captured at REL + 92 (gated by WIN = REST AND NOT DAY), cleared at REL + 33: cc767b78's. GRANT_l
  goes to the lane's pick drive (line `pocket:<lane>`) and back to its ring.
- THE RECORD (cc767b78's (a') compacting registers): written from G at REL + 102; SHIFTED at REL + 61 in a REL whose ARM3
  fired (RLS not read); HOME_l = its head, latched by the core at REL' + 57. Depth: at most 4 nappers picked and not yet
  ARM3'd over ctl-pick's (b') rows (conservative ARM3 stamp).
- OUT / superseded: HELD, the room count, the pointer's spacing counter (the latch replaces it), the record's resync.
- Normal rows (ctl-pick, the reference; 10,800 rows under (b'): the seven sweep points with the cold start, 129 true
  whole-farm freezes, all 64 T starting offsets, each at all 27 +-2 gt leg corners, with the FIFO and the positional
  release): 24 distinct every night, the worst interval 22,400 gt (margin 600 under 23,000, at T 0 / 5 / 0, legs -2 gt),
  the least H2 arrival gap 42 gt at the corners (margin 1 over the 41-gt cycle), lane 7's longest wait 16 RELs (two ring
  revolutions), golems lost per day 0; the forced id-permutation test unchanged.

## Subsystem 3: THE WAY-IN ((B)'s latch, hold and read, the token, the P1 trigger, the front's release) -> b8949c96

File: `designs/farms/iron/carousel-assembly/wip/controller/f1q2-day1-ctl-wayin.txt`.

- THE SECOND HOLD H2 (9,24,7): `powered_rail[ascending_south]`, braked; driven by the x-8 path: stone (8,24,7) under dust
  (8,25,7), a straight north-south dust run along x 8 at y 25 on glass (8,24,6..); keep-free: no rail may enter
  (7,23..25,6) or (8,23..25,7); (8,23,7) keep-empty; lane 7's merge C (7,24,7) is a 2-connection plain curve.
- THE ARRIVAL READ: `detector_rail[ascending_south]` (9,23,6), read by dust (8,23,6) on glass (8,22,6) (a corner east +
  north), its line north along x 8 at y 23 on glass (8,22,5..3), ending at (8,23,3) (its pointing target (8,23,2): air).
- KEPT AS NAMED (G1 11:43): (8,23,2) AIR or GLASS, never a conductor (booster 2's lit torch (8,22,2) strongly powers it,
  and a conductor would hold the read's end dust (8,23,3) on: H2 never opening); (9,22,6) GLASS (the detector's support:
  a pressed (9,23,6) strongly powers it; its power-out: (9,22,6) inert, (8,23,6) the read, (9,23,5) / (9,24,6) air, the
  hold (9,24,7) diagonal); (9,25,8) H2's upper connection, a PLAIN rail (no powered rail there: one would be powered
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
- THE STOP's INPUT CELLS (58f770cc 11:42 / 12:06, the re-lay car's latch-form stop, V = stop-local + (0,-20,-1); NO slope
  hold (the exit hold removed 05:33 / 06:11): NO tap line into (1,-21..-23,3..4); NO PARK line (PARK is made inside the
  stop file: the latch's Q -> the OR line -> its repeater -> the park slime): K's drive under K's slime V (-2,-22,2); the
  shutter's drive V (-5,-21,6) (the shutter at V (-5,-21,7)); RLS_IN V (-1,-22,-3) and ARM_IN V (6,-22,-6) (the latch row
  at V z -6): all subsystem 5's outputs, so line E carries no stop end. KEEP-FREE: V (1,-21,-1) and V (0,-21,0) (the OR
  line's step-down links). DAY's line end into R1's repeater (5,-12,-2); NOT-ready (8,-14,-5).
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
  hook V (-1,-19,3); the park string V (1,-19,2).
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
