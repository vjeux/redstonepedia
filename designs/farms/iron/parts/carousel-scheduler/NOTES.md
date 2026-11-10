# Carousel nightly scheduler (brief farm-iron-carousel.md, part 3)

## Single-lane loop scheduler (current, 2026-11-10)

**Works as placed (12:04, ironspec's rule).** carousel-loop-scheduler-v0.txt now holds every cell in its settled at-rest state (wire shapes, comparator outputs as block entities: `loop-gen` settles it).
- Build steps: one item (stone) in each ring's container: the D ring's token at (0,0,0) and the slot ring's at (-3,0,17).
- `loop-bench --placed-raw` (written states and block entities, no updates, no settle), 60 days seed 2 at 5% misses: 0 lost, every window T + 10..T + 88, slot lines in place. The judge's placement, 60 days seed 1: the same.
- Status: wired (290 blocks), inputs bench-driven, not connected to the loop.
- Tolerance: its timed outputs feed the napper siding, which isn't built yet, so the margins are against ironfarm's contract only: v opens T + 10 (by T + 20: 10 late, 0 early); v closes T + 88 (between T + 60 and T + 120: 28 early, 32 late). No physics sweep until the siding exists.

vjeux's single lane: one queue of 21 carts back to back, the front 3 released per release pulse
(a golem, or the fallback), rejoining the tail. ironfarm: the napper alone diverts to a siding at
the bed and re-merges into its own gap, so the queue keeps its order and release i sends batch
i mod 7. `mctas circuit memory loop-model` shows why that matters: ironspec's re-keyed policy loses
0 slots with the order kept, and 12,000-28,000 in 120 days for every counting policy (even an
oracle) if the napper rejoins at the tail.

The policy (pointer P +4 per visit, release index R +1 per release, a visit when the batch leaving
the station is P's) needs only D = P - R (mod 7): ONE 7-state dropper/hopper ring.
`mctas circuit memory loop-gen --out F` (crates/circuit-memory/src/loopgen.rs) builds it:
- each release pulse fires the ring once (D - 1); a tap on d0 is D = 0;
- the gate V = release - (NOT D0 or NOT win), one subtract comparator: a release finding D = 0
  inside the night window is a visit;
- V runs down a delay chain (stages of 4, 14, 14 gt) whose stage ends fire the ring 3 more times
  (D 0 -> 6 -> 3: the pointer's +4), done 60 gt after the release (releases come >= 95 apart);
- a 4th stage (44 gt) resets a comparator latch that V set: v = [T+10, T+88] (T = the release),
  ironfarm's window (the napper's batch passes the siding junction at T+20..60);
- the slot-out module (`slotout.txt`: a 3-state level ring whose taps are subtract comparators
  against NOT v) gives s_j = v AND l_j, about [T+16, T+94]; the reset steps the ring 8 gt later,
  so the slot holds through the window.

Evidence (v0, 290 blocks, 30.3 build minutes): `mctas circuit memory loop-bench` (release pulses
and the window driven, visits read off the design, the ring's token checked against the policy at
every release): 120 days at 1% and at 5% misses: 0 slots lost to an unwilling villager, 2,580
visits, every villager napped every night, 59 second naps in 2,499 villager-nights, every window
[T+10, T+88], every slot line clean; real game IDENTICAL over 96,001 ticks (4 days, 5% misses).
Weather (`--weather`): the window from a normal daylight detector reading 0 opens at day time 13,670
and closes at 22,331 in clear, rain, thunder and a 60-day random weather cycle (5% misses: 0 lost,
0 napless villager-nights); restarts (`--restarts 20`, 20 days): the same 1,722 output changes tick
for tick.
The release input is a stub cell until ironfarm's loop geometry names it.

## The 7-lane scheduler (archived)

**Archived (2026-11-10 06:01):** vjeux chose a single-lane loop (21 villagers in carts back to back,
released 3 by 3, rejoining the tail) instead of the 7-lane carousel, so this 7-lane scheduler (and
the planned counter + scheduler block with 21 per-track feeds) stops here, kept as it is.

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
