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

## Subsystem 2: THE PICKS (pointers x3, spacing, HELD, the pick-rate arbiter, the record) -> day-1

File: `designs/farms/iron/carousel-assembly/wip/controller/f1q2-day1-ctl-picks.txt`.

- Per lane: a POINTER (a ring counter over the lane's 8 positions, stepping D = 5 positions per pick: 5 is coprime with 7 /
  8 / 9), the SPACING (>= 11 REAL RELs since the lane's last pick: a counter clocked by REL), HELD (the station's napper in
  flight: <= 3, a per-lane timer).
- THE PICK-RATE ARBITER (bought ironspec 11:05a): at most ONE pick per REL; a lane deferred by it goes first at the next
  REL (fair); the pointer keeps its target (remembered).
- THE ROOM TERM (cap 3: the front + P1 + the second hold, part of (B)'s design): a pick only while dispatched - emptied <
  3; up per PICK, down per FRONT EMPTIED.
- THE RECORD: a 3-deep FIFO of lane codes, written per pick, its head read as HOME at the napper's release from the stop.
- Normal rows (ctl-pick, the reference): the normal nights (24 distinct per night, every nap interval < 24,000 - 1,000);
  the no-hold sweep (the bed cycle 197 / 168 gt x (B)'s travel 0..120: 12 points); bunched picks (three planted in each of
  the first 10 RELs, the model's legs and the measured 16-gt spacing): 0 nappers behind the second hold; the cold start.

## Subsystem 3: THE WAY-IN ((B)'s latch, hold and read, the token, the P1 trigger, the front's release) -> b8949c96

File: `designs/farms/iron/carousel-assembly/wip/controller/f1q2-day1-ctl-wayin.txt`.

- THE SECOND HOLD H2 (9,24,7): `powered_rail[ascending_south]`, braked; driven by the x-8 path: stone (8,24,7) under dust
  (8,25,7), a straight north-south dust run along x 8 at y 25 on glass (8,24,6..); keep-free: no rail may enter
  (7,23..25,6) or (8,23..25,7); (8,23,7) keep-empty; lane 7's merge C (7,24,7) is a 2-connection plain curve.
- THE ARRIVAL READ: `detector_rail[ascending_south]` (9,23,6), read by dust (8,23,6) on glass (8,22,6) (a corner east +
  north), its line north along x 8 at y 23 on glass (8,22,5..3), ending at (8,23,3) (its pointing target (8,23,2): air).
- THE LATCH (P1 OCCUPIED): SET by the 4-gt edge of the read; RESET by P1's exit detector (9,22,3), a level,
  reset-dominant. OCCUPIED = the latch delayed D = 16 gt (two delay-4 repeaters). HOLD OPEN = NOT latch AND (9,23,6)
  unpowered AND (9,22,3) unpowered -> the x-8 drive.
- THE TOKEN: a latch SET by the edge of the queue's departure detector (9,-11,-3), CLEARED by the P1 release pulse
  (reset-dominant). START-UP: the token SET (free), the latch 0, H2 open.
- THE P1 TRIGGER: the rising edge of TOKEN AND FRONT CLEAR ((9,-9,-5) unpowered) AND OCCUPIED fires one 12-gt pulse into
  P1 (9,22,5) (braked; powered = released).
- THE FRONT's RELEASE (the queue front (9,-10,-4), braked): RS_ready AND the queue hold (80 gt from the front's arrival
  press) AND REST AND NOT DAY.
- FRONT EMPTIED (to the picks' room count): the front's arrival detector falling after its recheck.
- Normal rows: ctl-pick's (B) rows (the RESET -> SET lag >= 22 gt, no napper behind H2), b8949c96's redsim way-in bench
  on these cells (3 nappers 20 gt apart, a slow 0.1 b/t arrival; the measured (9,23,6) -> P1 transit 9..11 gt), world
  reloads and unloads at every phase.

## Subsystem 4: THE DRIVE LINES -> loaderlay

File: `designs/farms/iron/carousel-assembly/wip/controller/f1q2-day1-ctl-lines.txt`.

- THE (9,0,8) NAPPER HOLD's line (REL + 67, judged at the hold rail): `repeater[facing=west]` (3,-1,8) (input (2,-1,8)),
  dust (4,-1,8), (5,-1,8), (6,-1,8) (east-west only), `repeater[facing=west]` (7,-1,8) into LIFT's (8,-1,8); glass
  (3..7,-2,8); the cells above (3..7,0,8) air. (Gate G's free-place term: OUT.)
- THE R-HOLD RELEASES (the S-bend's napper holds R(x-2,24,7)): delay(x) = REL' + 165 + 4 - travel(x) (REL' the cycle that
  takes the napper; travel from ironfarm's trace); the line ends per the lanes file: lane -1 a delay-3 repeater (-4,22,9)
  outputting east into the slime (-3,22,9); lane 3 a repeater (2,22,9) outputting west into the slime (1,22,9); lane 7
  the feed path's input (7,23,9).
- THE STATION's SLOT RELEASES (R_1 at (x,17,1), the station's R lines) at REL + 0 / 17 / 27, the line ends (x+2,17,1) on
  day-1's glass supports (x+2,16,1).
- THE STATION's D LINES: lanes -1 / 3 from F′'s taps into (1,-1,5) / (5,-1,5) (a solid conductor read from below by dust,
  the line going north at y -2, off A4's glass and torch faces and the napper lift's water glass); lane 7 from (7,0,2).
  Pending dfb89499's original-station run (if A6 is stripped, the D lines read the original station's D).
- THE STOP's LINES: the slope hold's `repeater[facing=north]` (1,-29,3) on glass (1,-30,3) into the slime (1,-29,4); the
  park drive's input (3,-27,3) into the slime (2,-27,3); DAY's line end into R1's repeater (5,-12,-2); NOT-ready
  (8,-14,-5).
- Normal rows: each line's delay from its tap to its target printed against the interface's timing; world reloads at
  every phase of each pulse.

## ETAs (as each owner states them)

- picks (day-1): first car ~5 h from 11:35 PT.
- core, way-in, lines: their owners state theirs.
