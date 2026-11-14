# day-1's controller interface (V rev 3)

The values and readers other parts cite by path. The controller's cells are laid in their own design file when the
controller car lands; until then this file is the interface of record. The values below are the code's (a test,
`ctlcredit::tests::the_interface_file_matches_the_code`, asserts each one against crates/mobfarm/src/ctlcredit.rs, and the
code reads the release pulse FROM this file: one written number).

## Interface values

- **release pulse = 12 gt** (ironspec 07:29r (1) / G1 07:27 (1), 08:43 (4)): THE REQUIREMENT for the batch release pulse
  out of dfb89499's limiter; dfb89499's function derives the laid pulse (its delay-2 cell (2,6,11)) and one test links
  the two; ironfarm's bench `--release-pulse` and day-1's napper trim read this one value.
- DM1_BLOCK7 = 35 gt: the one-credit-per-batch guard (opened by every credit), centred in the measured 5..65 gt.
- SEEN3_OLD's tap = REL + 80 gt, PROVISIONAL (the window 78..82 on train 15's offsets; train 16 re-sweeps it).
- K windows: the early half 5 gt, the late half 10 gt, riding each napper's release.
- The claim cap is in the design (a credit with no open claim is refused).
- The token's recovery delay N = 30 gt (ironspec 09:00a (1): one N, read by both benches): counted from the arrival
  detector going UNPOWERED (after its 20-gt recheck hold); N >= a departing napper's clear time (its box out of the
  landing footprint, z > -4.01, from its last rest tick; b8949c96's build 9303460bc prints it) + margin: ironspec re-rules
  only if that clear time exceeds 10 gt.
- R6 = 15 (bought, ironspec 09:00a (3)): each lane's napper leaves its R hold (x,24,7) at delay(x) = REL' + 165 + 5 -
  travel(x), REL' the release cycle that TAKES the napper (it reaches its R brake after its own batch's R release, so the
  next cycle's); cc767b78's D-keyed form (R = D_l + 34) names the same event; travel(x) = R(x) -> J(x) from ironfarm's
  trace (owed); the station row's (9,0,8) hold stays at REL + 67. (The logic bench still runs R6 = 10 until the shift's
  re-derivation lands; R6's physical bar is pinned failing by name meanwhile.)

## Readers (detector -> reader)

- The queue's departure detector (9,-11,-3): its RISING EDGE sets the P1 token ONLY WHILE A_SEEN (below).
- P1's exit sensor (9,22,3): clears the token (reset-dominant on a coincident set).
- The arrival detector (9,-9,-5), ONE DETECTOR, TWO ROLES: its LEVEL is the front-clear evidence (RELEASE needs it
  unpowered; its 20-gt recheck hold only extends "occupied"), and its RISING EDGE sets A_SEEN. They don't interfere: while
  a ridden front holds it pressed no release happens, so no new arrival edge is needed then.
- The ball-exit detector (7,-3,4) (bought: (B)): RS_ready, set on its RISING EDGE (a cart parked on it sets it once).
- The S-bend seat detectors (x,24,8), x = -1 / 3 / 7: D(-1) / D(3) / D(7), the lanes' entry presses (the ring, the credit,
  SEEN3, the K masks, the item count). The lane-7 napper's press merges into its batch cart's under the 20-gt hold.
- THE STATION's PER-LANE EXIT CELL (exit_cell, the same cells F′'s bench reads; dfb89499 08:47 (2), A2 / F′ build #4):
  lanes -1 / 3: the detector on A's LANDING (x,-1,3) (pressed only after the drop; (x,0,2) a plain rail); lane 7: (7,0,2)
  (until its raised-pocket car). The hold reopens on a RISING EDGE AFTER THE RELEASE; a cart pushed back off the landing
  gives no rise and a cart stuck on it holds it pressed with no new rise: both safe stalls (in the one table); exactly ONE
  edge on the restart after repair. The signal path from each landing detector to day-1's D input is laid with the
  controller car (its cells, power-out, update set and readers listed there).
- Vestigial on day-1's side: the stop's R detector (4,-1,5) (RS_ready no longer reads it) and the resync detector
  (9,24,7) (no reader; harmless by power).

## Release terms

- P1 release (the next napper rolls to the front): ruled 09:18a, THE RISING EDGE of (TOKEN AND FRONT CLEAR AND P1
  OCCUPIED) fires one release pulse; the pulse CLEARS the token and resets A_SEEN (reset-dominant). A napper already
  waiting releases when the token sets; one arriving later releases when it comes to rest on P1; nose to tail, the token
  is already consumed, so the second waits. P1 OCCUPIED is a SENSED LEVEL a RESTING napper keeps on (its sensor: a
  detector along the track one level off P1's deterministic rest, diagonal, no face neighbour of P1 or its powered rails,
  the second-in-queue outside its box: its cell named with P1's new rest by b8949c96 / vcompose). Failure directions:
  stuck LOW = a stall; a false HIGH fires a pulse into an empty P1 = also a stall (no kill path: the token is consumed by
  the release act). A retry-clock form is rejected.
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
- The stop's release = RS_ready AND the queue hold AND REST AND NOT DAY AND RETURN ROOM: a dry place on the napper's
  return row for it and every napper in transit. THE ROOM COUNTS ONLY THE NON-JOIN RETURN PLACES: (9,0,8), (7,0,7),
  (6,0,7), (5,0,7) and (4,0,7) (its contact with lane 3's departing cart declared, delaying no lane); NEVER the joins
  (3,0,7) / (-1,0,7) (a napper parked there jams that lane's departures).
- THE NAPPER's ROW HOLD (9,0,8) (bought, ironspec 08:41a (2)): releases at REL + 67 (the join-merge window 37..97, its
  centre) AND ONLY WHEN A NON-JOIN REST PLACE PAST (-1,0,7) IS FREE (the foot gate's evidence); day-1 lays the drive line
  into (7,-1,8) (the actuator is LIFT's: the hold rail on slime (9,-1,8), the repeater (8,-1,8)); its cells, power-out
  and update set come with the controller car; a planted row where the foot gate holds while a napper is due.
- A pick (G11) = the pointer AND the spacing AND HELD AND WAY-IN ROOM. The room count goes up per pick edge and down
  once per TOKEN SET (a napper's departure: the press while A_SEEN, or the recovery); no quiet-point resync (an
  over-count stalls, the safe direction).
- A held exit_cell (a cart parked on a lane's exit detector) gives no rising edge: the token / room count / hold STALL
  (the safe direction) and no other lane's release reads the stuck-high cell as anything.
- REL (G8) = the store armed (the read lit) AND the holds AND (if bought) LIFT CLEAR (ironfarm's foot-gate evidence).
- THE RESYNC NET's mint = REL waiting for a credit AND the item count <= 0 AND an open claim; one mint per STALL (its
  latch clears only on a credit or a move of the item count).
