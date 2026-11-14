# day-1's controller interface (V rev 3)

The values and readers other parts cite by path. The controller's cells are laid in their own design file when the
controller car lands; until then this file is the interface of record. The logic benches READ the numbers they use from
this file (ctl-pick: the recovery delay N and P1 OCCUPIED's delay D), and a test,
`ctlcredit::tests::the_interface_file_matches_the_code`, asserts every number below against the code's constants. The
release pulse and the release offsets are requirements the logic model doesn't use: this file states them once, the
station generator and F′'s benches read them from here, and F′'s test links dfb89499's laid pulse to this one.

## Interface values

- **release pulse = 12 gt** (ironspec 07:29r (1) / G1 07:27 (1), 08:43 (4)): THE REQUIREMENT for the batch release pulse
  out of dfb89499's limiter; dfb89499's function derives the laid pulse (its delay-2 cell (2,6,11)) and F′'s test links
  the two; ironfarm's bench `--release-pulse` and day-1's napper trim read this one value.
- **release offsets = 0 / 17 / 27 gt** (ironspec 09:54a (1)): the batch's three slot releases after REL (lanes -1 / 3 /
  7; the station header's 0 / 14 / 24 is stale); controller timing, stated once here.
- DM1_BLOCK7 = 35 gt: the one-credit-per-batch guard (opened by every credit), centred in the measured 5..65 gt.
- SEEN3_OLD's tap = REL + 80 gt, PROVISIONAL (the window 78..82 on train 15's offsets; train 16 re-sweeps it).
- K windows: the early half 5 gt, the late half 10 gt, riding each napper's release.
- The claim cap is in the design (a credit with no open claim is refused).
- The token's recovery delay N = 30 gt (ironspec 09:00a (1): one N, read by both benches): counted from the arrival
  detector going UNPOWERED (after its 20-gt recheck hold); N >= a departing napper's clear time (its box out of the
  landing footprint, z > -4.01, from its last rest tick; b8949c96 09:43: 5 gt) + margin: ironspec re-rules only if that
  clear time exceeds 10 gt.
- P1 OCCUPIED's delay D = 8 gt (ironspec 09:50a (d)): OCCUPIED = the latch AND D elapsed since its SET; D >= the longest
  measured (9,23,6) -> rest-on-P1 transit + margin (b8949c96 measuring; provisional); in hardware a repeater chain from
  the latch into the trigger's AND.
- The napper's row hold (9,0,8) releases at REL + 67 gt (judged at the hold rail).
- R6 = 15 (bought, ironspec 09:00a (3)): each lane's napper leaves its R hold (x-2,24,7) (lane x) at delay(x) = REL' +
  165 + 5 - travel(x), REL' the release cycle that TAKES the napper (it reaches its R brake after its own batch's R
  release, so the next cycle's); cc767b78's D-keyed form (R = D_l + 34) names the same event; travel(x) = R(x) -> J(x)
  from ironfarm's trace (owed; read by a test from the `# travel:` lines once printed); the station row's (9,0,8) hold
  stays at REL + 67. (The logic bench still runs R6 = 10 until the shift's re-derivation lands; R6's physical bar is
  pinned failing by name meanwhile.)

## Readers (detector -> reader)

- The queue's departure detector (9,-11,-3): its RISING EDGE sets the P1 token ONLY WHILE A_SEEN (below).
- P1's exit detector (9,22,3): RESETs P1 OCCUPIED's latch (form (B), reset-dominant) and holds the second hold shut while
  powered; the token is cleared by the release pulse.
- The arrival detector (9,-9,-5), ONE DETECTOR, TWO ROLES: its LEVEL is the front-clear evidence (RELEASE needs it
  unpowered; its 20-gt recheck hold only extends "occupied"), and its RISING EDGE sets A_SEEN. They don't interfere: while
  a ridden front holds it pressed no release happens, so no new arrival edge is needed then.
- The ball-exit detector (7,-3,4) (bought: (B)): RS_ready, set on its RISING EDGE (a cart parked on it sets it once).
- The S-bend seat detectors (x,24,8), x = -1 / 3 / 7: D(-1) / D(3) / D(7), the lanes' entry presses (the ring, the credit,
  SEEN3, the K masks, the item count). The lane-7 napper's press merges into its batch cart's under the 20-gt hold.
- THE STATION's PER-LANE EXIT CELL (exit_cell, the same cells F′'s bench reads): lanes -1 / 3: the detector D at
  (x,-1,5) (A6, bought ironspec 10:16a: A4's landing became a powered rail and D moved one cell; the earlier (x,-1,3) /
  (x,-1,4) forms are withdrawn); D's tap repeater (x+1,-1,5) is F′'s (facing west: reads D, outputs east), day-1's D line
  starts at (x+2,-1,5) ((1,-1,4) stays air: lane 3's always-on torch sits at (2,-1,4)). Lane 7: (7,0,2) (a KNOWN FAIL
  until its raised-pocket car). The station's timing is re-derived for the new D position, and every K / token timing
  keyed on D re-keys with it. The hold reopens on a RISING EDGE AFTER THE RELEASE; a cart pushed back off D gives no rise
  and a cart stuck on it holds it pressed with no new rise: both safe stalls (in the one table); exactly ONE edge on the
  restart after repair. The signal path from each exit detector to day-1's D input is laid with the controller car (each
  line cell with its six neighbours; no always-on source beside any dust or repeater input).
- Vestigial on day-1's side: the stop's R detector (4,-1,5) (RS_ready no longer reads it).

## Release terms

- P1 release (the next napper rolls to the front): ruled 09:18a, THE RISING EDGE of (TOKEN AND FRONT CLEAR AND P1
  OCCUPIED) fires one release pulse; the pulse CLEARS the token and resets A_SEEN (reset-dominant). A napper already
  waiting releases when the token sets; one arriving later releases when it comes to rest on P1; nose to tail, the token
  is already consumed, so the second waits. Failure directions: stuck LOW = a stall; a false HIGH fires a pulse into an
  empty P1 = also a stall (no kill path: the token is consumed by the release act). A retry-clock form is rejected.
  START-UP: the token FREE, P1's latch 0, the second hold open: the first pulse waits for the first napper's rest + D.
- P1 OCCUPIED, FORM (B) (bought ironspec 09:50a; the drive final 10:02a; form (A), the 3-place count, rejected): A SECOND
  HOLD at (9,24,7) (powered_rail[ascending_south], braked), driven by the x-8 path: a solid conductor (stone) at (8,24,7)
  with the drive's last dust on top at (8,25,7), the line a straight north-south dust run along x 8 at y 25 ((8,25,6),
  (8,25,5), ... on glass (8,24,6), (8,24,5), ...); the conductor activates the hold rail beside it; lane 7's merge C
  (7,24,7) is a 2-connection plain curve (west (6,24,7), south (7,24,8)), which 26.3 RailBlock.updateState re-shapes only
  for a signal-source neighbour at 3 potential connections: the stone never touches it; KEEP-FREE: no rail may ever enter
  (7,23..25,6) or (8,23..25,7); (8,23,7) KEEP-EMPTY (the strongly powered stone would power anything there);
  update-consumers: (9,24,7) intended; (7,24,7), (8,23,7), (8,24,6), (8,24,8), (8,25,8) by-code. The arrival detector at
  (9,23,6) (detector_rail[ascending_south]), read by dust (8,23,6) on glass (8,22,6), a corner east + north, its line north
  along x 8 at y 23 on glass (8,22,5..3), STOPPING at z 3 ((8,23,2), air over booster 2's lit torch (8,22,2), is its end's
  pointing target: inert); the detector's strong output goes into (9,22,6), P1's face neighbour, which stays GLASS; its
  update-consumers: (8,23,6) intended; (9,22,6), (9,23,7) glass; (9,23,5), (9,24,6) air (the rider passage). The drive
  line (y 25) and the read line (y 23) never connect (no dust at y 24 between).
  - THE LATCH: SET by a 4-gt pulse on (9,23,6)'s RISING EDGE (a pulse former at the latch's input: a comparator in
    subtract mode, the read line into its back and the same line through a delay-2 repeater into its side; a detector
    stale-powered across a RESET makes no new edge, so no re-SET); RESET by (9,22,3)'s rising edge, reset-dominant; a torch
    RS latch (block states and scheduled ticks persist across save / load; burnout memory doesn't; one turn-off per cycle,
    about 5 in any 60 gt at the worst rate, < 8).
  - OCCUPIED = the latch AND D (above) elapsed since the SET.
  - THE HOLD OPEN = NOT latch AND (9,23,6) UNPOWERED AND (9,22,3) UNPOWERED (ironspec 10:05a (b): a RESET level still held
    when napper 2's SET pulse lands would eat it; each detector's 20-gt recheck after napper 1 left it ends before napper 2
    is let down, so every SET is a fresh edge); shut on SET. One napper at most between the hold and P1's exit: no merge,
    no double release. The non-powered (9,23,6) breaks the powered-rail chain between the hold and P1; (9,23,6) is
    diagonal to both.
  - The room cap stays 3: the front + P1 + the hold (a napper dispatched while P1 and the hold are full arrives after P1's
    release has freed the hold). Drift: a missed SET / RESET = a stall; the guide's reset lever (the latch to 0).
- THE TOKEN (ironspec 07:38r / 08:41a, G1 07:39): set ONLY by evidence from the napper released LAST.
  - Each release pulse CLEARS the token and A_SEEN (ARRIVED).
  - The arrival detector's rising edge after the release SETS A_SEEN; a release and an arrival in the same tick resolve
    to NOT SEEN (A_SEEN's reset dominates).
  - Token SET = (the departure press's rising edge OR THE RECOVERY) AND A_SEEN; the set clears A_SEEN.
  - THE RECOVERY (ruled 08:41a, b8949c96's amendment) = A_SEEN AND the arrival detector (9,-9,-5) UNPOWERED CONTINUOUSLY
    for N gt; it SETS the token and clears A_SEEN. ("A napper at P1" is dropped: no detector reads a resting napper.)
  - Any departure press or recovery condition OUTSIDE A_SEEN is ignored (a late press after a recovery's release can't
    re-set the token; a player's empty leaving the front can't either).
  - The arrival must be the released napper's own: no other cart enters the detector's cover between a release and that
    napper's arrival (b8949c96's plant).
  - Failure directions: A_SEEN stuck LOW (lost, or cleared by a world open) = a safe stall (priced; the guide's repair
    within ~1,100 gt: the token reset or re-carting); A_SEEN stuck HIGH or a spurious set = the deadly form: shown absent
    on a world open at every phase of a transit.
  - ⚠ THE FRONT-REST HAZARD (found by the hardware-only re-model, 10:15; ironspec ruling pending): an extra press of
    (9,-11,-3) while a napper RESTS at the front (A_SEEN true: a rollback over it, a player's empty on R1) sets the token
    and counts the room down early; P1's release still waits for FRONT CLEAR, but the stalled-stop row dispatches a 4th
    napper. Day-1's candidate (G1 backs it): the room's down-count from the FRONT EMPTYING (an R_SEEN latch set by the
    arrival edge, cleared by the arrival detector's falling edge, which counts the room down), not the press; its
    failure directions: a missed falling edge = no down-count (a stall); a spurious one (a bounce at the front) is spanned
    by the detector's 20-gt hold (rows owed).
- The stop's release = RS_ready AND the queue hold AND REST AND NOT DAY AND RETURN ROOM: a dry place on the napper's
  return row for it and every napper in transit. THE ROOM COUNTS ONLY THE NON-JOIN RETURN PLACES: (9,0,8), (7,0,7),
  (6,0,7), (5,0,7) and (4,0,7) (its contact with lane 3's departing cart declared, delaying no lane); NEVER the joins
  (3,0,7) / (-1,0,7) (a napper parked there jams that lane's departures).
- THE NAPPER's ROW HOLD (9,0,8) (bought, ironspec 08:41a (2)): releases at REL + 67 (the join-merge window 37..97, its
  centre; judged AT THE HOLD RAIL (9,0,8), the controller's source time back-computed) AND THE SENSED FREE-PLACE TERM.
  The stretch past lane -1's join has NO rest place (the slope (-2,0,7) -> (-3,1,7) up to the foot (-4,2,7)): a cart
  held at gate G leaves any cart behind it rolling back onto (-1,0,7). Gate G is never inhibited (it guards the lift's
  water). So the term is "no cart AHEAD of the napper can be held at G during its transit": gate G's foot detector
  (-4,2,7) UNPOWERED (polarity: unpowered = free) AND no lane -1 / 3 exit-cell rising edge within the last X gt (X = the
  longest exit-cell -> foot transit + margin, from F′'s re-derived station times): a cycle's batch carts are then past G,
  and a G closure mid-transit can only hold the napper itself, at the foot. If F′'s rows show a late batch cart can still
  be ahead, a second napper hold at (4,0,7) gated by the same term. Rows (owed): a gate-G hold starting at every phase
  of the ~13-block transit (the planted one: G closing one tick after the release with a cart on the ramp), the napper
  never resting at (3,0,7) / (-1,0,7); a double release at the hold (two queued, one pulse, one released).
  THE DRIVE LINE (the controller part's cells; LIFT's (8,-1,8) the boundary): repeater[facing=west] (3,-1,8) (input
  (2,-1,8); its north side the station's torch-driven slime (3,-1,7), which a repeater's side does not read), dust
  (4,-1,8), (5,-1,8), (6,-1,8) (each east-west only: (x,-1,7) / (x,-1,9) hold nothing it connects to),
  repeater[facing=west] (7,-1,8) (its sides diode-free), on glass (3..7,-2,8); the cells above (3..7,0,8) stay air.
- A pick (G11) = the pointer AND the spacing AND HELD AND WAY-IN ROOM. The room count goes up per pick edge and down
  once per TOKEN SET (a napper's departure: the press while A_SEEN, or the recovery); no quiet-point resync (an
  over-count stalls, the safe direction).
- A held exit_cell (a cart parked on a lane's exit detector) gives no rising edge: the token / room count / hold STALL
  (the safe direction) and no other lane's release reads the stuck-high cell as anything.
- REL (G8) = the store armed (the read at the last of 4 dust lit) AND THE STORE GUARD (the controller's logic count of
  stored credits, the credits in flight included, >= 1; bought ironspec 08:23a (1)) AND the holds AND (if bought) LIFT
  CLEAR (ironfarm's foot-gate evidence).
- THE RESYNC NET's mint = REL waiting for a credit AND the item count <= 0 AND an open claim; one mint per STALL (its
  latch clears only on a credit or a move of the item count).
