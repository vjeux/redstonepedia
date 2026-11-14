# day-1's controller interface (V rev 3)

The values and readers other parts cite by path. The controller's cells are laid in their own design file when the
controller car lands; until then this file is the interface of record. The numbers come from crates/mobfarm/src/ctlcredit.rs and
ctlpick.rs (the logic benches, `mctas mobfarm ctl-credit-bench` / `ctl-pick-bench`).

## Interface values

- **release pulse = 12 gt** (ironspec 07:29r (1) / G1 07:27 (1)): the batch release pulse out of dfb89499's limiter (its
  delay-2 cell (2,6,11)); the limiter's gadget drive, ironfarm's bench `--release-pulse` and day-1's napper trim all read
  this one value.
- DM1_BLOCK7 = 35 gt: the one-credit-per-batch guard (opened by every credit), centred in the measured 5..65 gt.
- SEEN3_OLD's tap = REL + 80 gt, PROVISIONAL (the window 78..82 on train 15's offsets; train 16 re-sweeps it).
- K windows: the early half 5 gt, the late half 10 gt, riding each napper's release.
- The claim cap is in the design (a credit with no open claim is refused).

## Readers (detector -> reader)

- The queue's departure detector (9,-11,-3): its RISING EDGE sets the P1 token.
- P1's exit sensor (9,22,3): clears the token (reset-dominant on a coincident set).
- The arrival detector (9,-9,-5), ONE DETECTOR, TWO ROLES: its LEVEL is the front-clear evidence (RELEASE needs it
  unpowered; its 20-gt recheck hold only extends "occupied"), and its RISING EDGE sets the ARRIVED latch that gates the
  token's recovery. They don't interfere: while a ridden front holds it pressed no release happens, so no new arrival
  edge is needed then.
- The ball-exit detector (7,-3,4) (bought: (B)): RS_ready, set on its RISING EDGE (a cart parked on it sets it once).
- The S-bend seat detectors (x,24,8), x = -1 / 3 / 7: D(-1) / D(3) / D(7), the lanes' entry presses (the ring, the credit,
  SEEN3, the K masks, the item count). The lane-7 napper's press merges into its batch cart's under the 20-gt hold.
- The station's departure detector D (per lane): the hold reopens on a RISING EDGE AFTER THE RELEASE; a cart parked on D
  at the release gives no edge and the hold stays shut (ironspec 07:32r (2)(b)).
- Vestigial on day-1's side: the stop's R detector (4,-1,5) (RS_ready no longer reads it) and the resync detector
  (9,24,7) (no reader; harmless by power).

## Release terms

- P1 release (the next napper rolls to the front) = TOKEN AND FRONT CLEAR (the arrival detector unpowered).
- The token's recovery (a missed departure press) = ARRIVED (an arrival press since the token's last clear) AND the front
  clear for N AND a napper waiting at P1. A napper still in transit to the front has made no arrival press, so the
  recovery can't fire under it (b8949c96 07:35); a napper lost before the front leaves a stall (safe).
- The stop's release = RS_ready AND the queue hold AND REST AND NOT DAY AND RETURN ROOM (a dry place on the napper's
  return row for it and every napper in transit).
- A pick (G11) = the pointer AND the spacing AND HELD AND WAY-IN ROOM (dispatched minus departed < 3: up per pick edge,
  down per departure press, clamped >= 0, resynced at the record's quiet point).
- REL (G8) = the store armed (the read lit) AND the holds AND (if bought) LIFT CLEAR (ironfarm's foot-gate evidence).
