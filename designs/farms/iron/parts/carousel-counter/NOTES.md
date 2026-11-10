# Carousel counter (brief farm-iron-carousel.md, part 1)

The 7-state sequencer of the iron carousel: one golem pulse per slot advances it by one, a fallback
inside it advances it when a slot is missed, and 7 level lines say which batch goes next. Bench:
`mctas circuit memory counter-bench <design>` (crates/circuit-memory/src/seqbench.rs).

## Members (best first: `members.tsv`)

| design | blocks | box | build min (middle) | T_c | gap | m |
|---|---|---|---|---|---|---|
| carousel-counter-v7.txt | 95 | 11x3x13 | 15.2 | 21 gt | <= 7 gt, never two lines | 6..8 |
| carousel-counter-v6.txt | 106 | 11x5x13 | 16.6 | 21 gt | <= 7 gt, never two lines | 6..8 |

Lines: lane b's line at x = b (output ports `b0`..`b6`), even lanes on the ring's north face, odd
on its south face (ironfarm 2026-11-10: either side, any height). Cycle 0 -> 2 -> 4 -> 6 -> 5 -> 3
-> 1 (every lane's turn every 700 gt; ironspec: fine). Each line is a comparator reading a dropper
holding one stone: strength 1, so a lane's wiring starts with a repeater. Keep drops and mobs off
the 7 open hopper tops (ironspec 2026-11-10: an assembly constraint of the timing diagram).

## Evidence (v7)

- redsim bench, 100,003 golem pulses each, every tick checked (one line high, one advance per
  pulse within 40 gt, the cycle's order, fallback 100 + m after the previous advance, no stall),
  then the same run with 20 chunk reloads at random ticks and on the design turned 90, 180 and 270
  degrees, all identical tick for tick: see the commit that added v7 for the two seeds' lines.
- the real game (`circuit trace-compare` on the copy `--stimulus-out` writes, every cell of the
  watched box every gt, the game against redsim): IDENTICAL over 39,220 frames (302 pulses, 79
  missed slots, 89 fallbacks) and over 292,618 frames (2,003 pulses, 796 missed slots, 911
  fallbacks).

## How it works

- Ring: 7 droppers and 7 hoppers alternating around a 2x7 rectangle, one stone. A dust loop on top
  fires every dropper at once; the dropper holding the stone pushes it into the hopper after it,
  which passes it to the next dropper 8 gt later, after the firing, so firing order never matters.
  A comparator on each dropper is its lane's line. (A ring of adjacent containers is always even,
  so 7 droppers need 7 transit cells; distinct line columns x = 0..6 need the 14-cell ring anyway.)
- Front and merge: the golem pulse and the fallback torch power one dust; a 2-rt repeater makes it
  a >= 4-gt charge of a 2-comparator loop (stage 1), which stays up ~28 gt after the last charge:
  every retrigger within that window is one burst.
- Edge: a subtract comparator reads stage 1's output from behind and, through 10 gt of repeaters,
  from the side: ONE 10-gt pulse per burst. It fires the ring and refills stage 2 (a 3-comparator
  loop) at 15, so the countdown always starts from the burst's first edge.
- Fallback: stage 2's output stone carries a wall torch; when the countdown ends (m = 6..8 after
  the slot) the torch lights and re-enters the front, which advances the ring and refills stage 2.

## What did not work

- v1: the catalog's ring without a fallback (no slot is ever missed in its bench).
- v2-v3: the catalog's retriggerable monostable (mono-retrig-2stage-k1, re-tuned) on the input:
  it re-arms ~10 gt after its input, so a golem 95-105 gt after the previous advance came too late
  to cancel the countdown it was meant to restart: the fallback fired anyway and the ring stepped
  twice (3 skips in 2,000 pulses).
- v4: golem and torch into the monostable, the ring fired from stage 1's edge: no more double
  steps, but the torch's flicker restarted the countdown late, so a short golem pulse pushed the
  next fallback to m = 18-19. Fixed in v5 by refilling stage 2 from the edge pulse; v6 tuned m.

## Next ideas

- Fewer hoppers (37% of the build time): a dropper-only ring needs two firing phases on cells that
  alternate like a checkerboard; dust bridges over a dropper quasi-connect it (a dust on a block on
  its top powers it), so the bridges must be two blocks up on glass, or observers fire each set
  from opposite sides (14 observers cost about what the 7 hoppers save).
- Sealing: the loop's dust leaves the hopper tops open to stray items (the catalog's sealed ring
  puts glass on them). Tried (v7s): dust on glass over the hoppers stops the pulse (power climbs a
  glass step but never comes down one: a dust only takes power from a dust diagonally above when
  the step block conducts); dust on stone over the hoppers carries it, but each powered stone
  quasi-connects the droppers beside it (a dropper also fires on power at the cell above it), and
  when a dropper's own dust turns off before the stones, nothing updates it again: it stays
  triggered and the stone stops in it for good. So the assembly keeps drops off the counter (a roof
  or its place), or the ring gets another firing network.
