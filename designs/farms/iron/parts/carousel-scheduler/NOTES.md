# Carousel nightly scheduler (brief farm-iron-carousel.md, part 3) — work in progress

The sleep routine's scheduler: inside the night window it sends one villager at a time to the bed,
each exactly once a night (a rare second nap for the head), never a batch due at the station.
ironspec's ruling: the counting-free BLOCK policy — a head pointer P (7-state dropper/hopper ring)
and a level ring L (3-state), both stepped once per visit; a visit fires when the line of the batch
P names falls inside the window; visits every 4 advances inside the weather-proof deep night
13,670..22,330 (a daylight detector's normal output 0 / inverted 11 exactly there in clear, rain and
thunder); the pointer's ring order for +4 is lane 0 -> 5 -> 2 -> 3 -> 4 -> 1 -> 6.

Generated: `mctas circuit memory sched-gen --out F` (crates/circuit-memory/src/schedgen.rs) from
the module designs here. Bench: `mctas circuit memory sched-bench F` (docs/tools.md).

## Core v3 (carousel-scheduler-core3.txt, 456 blocks, 45.7 build minutes)

- Pointer ring (two columns along z), a comparator tap per dropper, a wire to each lane cell
  (`vcell3`/`vcell3w`, pitch 3): G = line AND P (subtract comparator: rear the line, side NOT P from
  a wall torch on P's block), onto plain-dust buses meeting under the ring.
- Arm (`varm`): W = bus AND night, then W's rising edge = a 4-gt pulse while the named batch is
  still at the station.
- Window X: a 220-gt two-stage off-delay (`offdelay-k3`, catalog family off-delay-2stage, k 3)
  squared by a repeater: up from before the line's fall to 147..178 gt past it (ironfarm's hump
  needs a level over [T, T+100], T the fall).
- STEP: X's falling edge (`fedge-n`) fires the pointer's loop (a wire at y 2) and the level ring's.
- Per track (to come with ironfarm's cells): window(b,k) = X AND P_b AND L_k, feed(b,k) = line_b OR
  window(b,k), the OR next to the counter.

Evidence (v3): 120 days, 1% misses: 28,797 slots, 0 lost to an unwilling villager, 2,580 visits all
to the batch that just left, every villager napped every night, 59 second naps in 2,499
villager-nights, window up at every dispatch (0 gaps, 147..178 gt past the fall); the real game
IDENTICAL over 240,001 ticks (10 days, 5% misses; 11 output probes every tick).

## History

- core0 (605 blocks): first circuit, V = the window-gated falling edge of the bus; judged by ironspec
  on the contract (0 lost over 120 days, = the policy model).
- core2 (396 blocks): pitch-3 cells, STEP = V; superseded when the hump window had to cover the
  line's fall (core0/core2's window started 12 gt after it).
