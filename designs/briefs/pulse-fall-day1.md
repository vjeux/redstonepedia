# Day-1 falling-edge pulse for the iron farm's day-1 redstone fork (design doc, gate G1)

Pulse lane, 2026-11-08. Gate G1 ([docs/process.md](../../docs/process.md)). G1 APPROVED
(reviewer eaca3470 + coordinator f1a42990) on brief v2; v2.1 below applies the reviewer's
three non-blocking corrections (boundary 20-22 gt, Layout E invalid-state timeline, Layout D
timing as a lower bound), none of which changes the decision, target, accept window, or
must-haves. The design built from it is
`designs/circuits/mobfarm/tracks/ironfarm/pulse-fall-torch-delay-L22.txt`.

## Use

When the iron farm's cycle signal falls, emit one positive pulse to open the 1-3 oak
trapdoors of the hide (M2 geometry) once per cycle.

## Confirmed signal spec (ironspec, recorded here)

**S — input.** Source: wooden pressure plate under blind villager U1 (bed plate). HIGH all
day (U1 stands on it), LOW all night. Falling edge at day time 12000 + up to 21 gt
(schedule update) + up to 20 gt (plate `pressedTime`). S arrives as redstone wire from
the plate, strength 15 at the plate, ≤ 4 dust into the pulse part (≥ 11 at the part —
any > 0 suffices for a repeater input or a torch block).

- High phase H ≈ 12,000 gt (day). Low phase L ≈ 12,000 gt (night, to waking at day
  time ~0-10). Period 24,000 gt.
- Glitches: brief by-day drops if U1 steps off the plate; each glitch low ≥ 20 gt (plate
  `pressedTime`). A glitch pulse is harmless (villagers calm for L gt, don't sleep by day,
  re-panic at the next scan); a glitch pulse missed for H < 22 gt is equally harmless.
- Polarity: positive pulse (Out HIGH for L after S falls) fits the OPEN-trapdoor hide;
  inverted would fit the shut-trapdoor hide (D1).

**Out — output.** Load: 1-3 oak trapdoors, fed directly or through dust on the blocks
above them (≤ 4 dust, farm wiring, not part of the pulse part). Trapdoors switch on any
> 0 power; Out is torch output (15, ≥ 11 after ≤ 4 dust). Fan-out: 1-3 trapdoors in
parallel on the same Out wire. The farm may add a repeater after the part — it only
shifts the hide, it does not shorten it.

## The Number

**Accept 20-100 gt; target 22 gt (Layout A nominal); reject < 20 gt; reject/rework
> 100 gt.** L ≥ 20 gt is the only hard requirement: the zombie's ray is cast only by
`VillagerHostilesSensor` (rate 20), once per 20 gt `NearestVisibleLivingEntities` list, so
any 20 consecutive hidden ticks hold one hostile scan for every phase. No hard upper
bound: cost is L / 12,000 of a night's spawning per gt (22 gt → 0.18%, 100 gt → 0.83%
< 1%). The farm takes the smallest archived part with L in 20-100 — Layout A is the
first verified part in that window, not claimed smallest.

## The mechanics, from the game code (26.3)

Units: **1 gt = 50 ms; 1 redstone tick (rt) = 2 gt = 100 ms.** Every redstone-tick
delay is even.

| Element | 26.3 file / class / method | Verified value |
|---|---|---|
| Torch delay | `net/minecraft/world/level/block/RedstoneTorchBlock.java` — `RedstoneTorchBlock.neighborChanged` → `Level.scheduleTick(pos, this, 2)`; `tick(ServerLevel, BlockPos, BlockState, RandomSource)` | **1 rt = 2 gt.** Torch = NOR: `getSignal` = 15 iff attachment block unpowered, else 0. |
| Torch burnout | `RedstoneTorchBlock` — recent-toggles list | > 8 toggles in 60 gt (`RECENT_TOGGLE_TIMER = 60`, `MAX_RECENT_TOGGLES = 8`) → burnout, restart after 160 gt. |
| Repeater delay | `net/minecraft/world/level/block/RepeaterBlock.java` — `RepeaterBlock.getDelay(BlockState)` = `DELAY × 2`; `net/minecraft/world/level/block/DiodeBlock.java` — `tick`, `shouldTurnOn`, `isLocked`, `shouldPrioritize` | **DELAY 1-4 → 2 / 4 / 6 / 8 gt.** Diode: forward only, refreshes to 15; side power locks it (`isLocked`); `shouldPrioritize` can move a repeater ahead of a non-diode scheduled the same gt. |
| Scheduling | `net/minecraft/world/level/Level.java` — `scheduleTick(BlockPos, Block, int)` (delay in gt); `net/minecraft/world/level/ticks/LevelTicks.java` — ordered by scheduled time, then priority, then schedule order (FIFO) | Ticks run at their scheduled gt; same-gt order can only reorder within that gt, never slip a tick to the next gt. |
| Wire | `net/minecraft/world/level/block/RedstoneWireBlock.java` — `getSignal` / power calculation | −1 strength per block, max 15; repeaters refresh. |
| Plate | `net/minecraft/world/level/block/BasePressurePlateBlock.java` — `getPressedTime()` | 20 gt. |
| Hopper | `net/minecraft/world/level/block/entity/HopperBlockEntity.java` — `tick` / `pushItemsTick`, `setCooldown(8)`; `HopperBlock.neighborChanged` locks when powered | 8 gt per item transfer. |
| Piston | `net/minecraft/world/level/block/PistonBlock.java` — `triggerEvent`; `PistonMovingBlockEntity.tick` | Extend/retract animate over ~2 gt, block-event driven, **not tunable** to 20 gt. Non-sticky cannot pull a block back. |
| Trapdoor load | `TrapDoorBlock.neighborChanged` | Follows power the same gt; adds no delay. |

## Layout A — timing derivation

Circuit: `Out = (NOT S) AND (S delayed 20 gt)`, as one torch `T_out` whose attachment
block `B_out` is powered by S (dust branch) OR by `T_inv = NOT(delayed S)`.

- Delay chain: 5 repeaters @ 4 gt = **20 gt**. A repeater chain is a pure delay line: it
  preserves pulse width for widths > first-stage delay (4 gt); a pulse ≤ 4 gt is
  swallowed at the first stage (`tick` re-evaluates `shouldTurnOn` at tick time).
- Rise: S falls → `T_out` block loses S power → `T_out` tick at **fall + 2 gt** → Out
  rises at +2.
- Fall: delayed S falls at fall + 20 → `T_inv` on at +22 → `T_out` off at **fall + 24**.
- **Pulse = 24 − 2 = 22 gt = 20 + (2 fall-path torches − 1 rise-path torch) × 2 gt.**
  Nominal 22, granularity **2 gt**: this family can only target even L; odd L is not a
  design target, only boundary skew.
- Preconditions (measured, `mctas circuit verify`, 26.3 server, 4 positions ×
  4 rotations — see "Build verification"): the judge's falling-edge table is
  `in 2..20 gt: +26 for 4..20 (not handled); in 30 gt: +34 for 22; in 40 gt: +44 for 22;
  in 60 gt: +64 for 22` (delay measured from the input's rising edge; output rise is
  falling edge + 4 gt for handled inputs). So the judge's min-pulse is **30 gt**: inputs
  < 30 gt are not handled to exactly 22 gt (short inputs give pulses of their own length,
  up to 20 gt). In isolated tests (one pulse, 200 gt gap) H = 20 gives a 20 gt pulse and
  H ≥ 24 gives exactly 22 gt; in trains (two pulses with a short low gap) the pulse is
  min(L, 22) for H ≥ 24 (L = 10 → 10 gt, L = 20 → 20 gt, L ≥ 22 → 22 gt). The farm's
  night edge has H ≈ L ≈ 12,000 gt, so it gives exactly 22 gt, delay 4 gt.
- Re-arm: after S rises, armed for a nominal next pulse after **22 gt** of high (chain +
  `T_inv` settle). Minimum nominal cycle = **44 gt** (22 high + 22 low) ≪ 24,000 gt period.
- **Tick-order skew (derived, not asserted):** nominal events are staggered ≥ 2 gt apart,
  so there is no same-gt coincidence and the nominal pulse is exactly 22 gt. Coincidences
  occur only at boundaries — L = 20–22 (S-rise off vs delayed-fall off), H = 20–22
  (`T_inv`-off vs S-fall), H = 4 (first-repeater tick vs S-fall) — where `LevelTicks`
  ordering (priority, then FIFO; `shouldPrioritize` may move a repeater ahead of a torch)
  can resolve the pair either way, moving the affected edge by at most 1 gt. The ±1-on-nominal
  derivation misses the truncation floor: the S-rise path already turns `T_out` off at
  fall + 22 (pulse 20 at L = 20), and the H = 20 rise delay already gives pulse 20 (rise
  at S-rise + 24). Hence boundary coincidences / truncation give **20–22 gt**, never 23,
  and never below 20 for an edge with one phase ≥ 22 and the other ≥ 20. Inside the
  20–100 accept window. Ticks never slip to the next gt.
- **Wiring constraint:** a torch senses only its attachment block. Only one dust cell can
  sit on `B_out`'s top, so the second OR input must arrive as dust pointing into
  `B_out`'s side from an adjacent cell (or via a repeater). In the built design, S dust
  runs into `B_out`'s west side and `T_inv` (a wall torch on `B_inv`) powers `B_out`
  from the east; both power `B_out`. `T_inv` and Out wires run away from the delay chain
  and never sit adjacent to its repeaters, or side power would lock a repeater
  (`DiodeBlock.isLocked`) and freeze the delayed line.

## Four or five genuinely different layouts

Total BOM = crafting + placed, not logic-only. The built design (Layout A) is **35 judge
blocks in 7×2×5** (7 repeaters, 2 torches, 19 stone blocks, 7 placed dust), not the
brief's first footprint estimate — the judge count is the authority. Built crafting BOM:
16 torches / 40 stone (21 in repeaters + 19 placed) / 30 dust (7 repeater-internal +
14 repeater torches + 2 logic torches + 7 placed) / 16 sticks (8 planks) / 0 iron.
The judge reports: **falling-edge L 22 gt, delay 4 gt, min-pulse 30 gt** (redsim; real-game
verify below).

| Layout | Footprint (built/estimate) | Total BOM (torches / stone / dust / sticks / iron) | Wire runs | Timing | Verdict |
|---|---|---|---|---|---|
| **A — delayed-AND** | 7×2×5, **35 blocks** (built, judge) | **16 torches / 40 stone / 30 dust / 16 sticks / 0 iron** (crafting; judge: 35 blocks = 7 repeaters + 2 torches + 19 stone + 7 dust) | dust wire ≤ 7 cells, power ≥ 9 at repeater A | **22 gt, delay 4 gt, min-pulse 30 gt** | **Built + verified (below)** |
| B — torch + repeater-feedback | 6×3×2 (est.) | 11 torches / 16 stone / 20 dust / 11 sticks / 0 iron | ≤ 2 dust | 22 gt ON, then repeats every 44 gt | Reject: pulse train, not single-shot. Out torch toggles 2× per 44 gt → max 4 in any 60 gt window < 8-toggle burnout threshold, so **no burnout** — failure is re-firing while S stays low. |
| C — regular piston + redstone block | 3×3×2 (est.) | 0 torches / 1 stone / 12 dust / 0 sticks / **1 iron** | ≤ 2 dust | ~2–4 gt contact, not tunable | Reject: non-sticky cannot pull back; still needs a repeater chain for 20 gt, so it adds iron + a moving part for zero timing benefit. |
| D — hopper-item timer (bounded variant) | 4×3×2 (est.) | 0 torches / 0 stone / **3 dust** / 0 sticks / **12 iron** | ≤ 2 dust | **≳ 38 gt (lower bound)**, not 24 | Reject: v1's 24 gt was hopper-only; it omitted dropper 4 gt (`DispenserBlock` tick), item fall 1–2 gt, and at least one collector transfer (8 gt) before the plate can fall — 24 + 4 + 2 + 8 = ≳ 38 gt lower bound, with full sequential clear of 3 items (3 × 8 gt) able to push it well past that. Iron-heavy, entity-dependent. Still in 20–100, but not day-1-cheap. |
| E — RS-NOR latch (SET on fall, RESET delayed) | 7×4×2 (est.) | **14 torches / 19 stone / 29 dust / 14 sticks / 0 iron** | ≤ 3 dust | 22 gt, holds through S bounce | Fallback only. At reset, SET = 1 and RESET = 1 simultaneously → latch forced to invalid 0,0 (reset-dominant, Out = 0, pulse ends correctly); the invalid 0,0 ends ~4 gt after S rises (SET torch 2 gt + latch torch 2 gt), when the latch settles to valid reset Q = 0, Qbar = 1 while RESET is still 1 — so a second falling edge with H < 22 while RESET is still 1 is missed. Power-on: latch state persists from disk, so the part must be built/armed in RESET (Q = 0) and given a full H ≥ 22 gt arming phase before its first trusted edge. Same H/L preconditions as A, plus latch state to initialise. |

## Trade-offs

- **Footprint vs precision:** A is ~2× the banned sticky/comparator bar's footprint, for
  22 gt ± boundary skew instead of exact.
- **Iron vs reliability:** A/E use 0 iron; D buys long timing with 12 iron + entity fragility.
- **Single-shot vs retrigger:** A is self-clearing, cannot re-fire while low; B re-fires; E
  holds through bounce but can sit in invalid 0,0.
- **State vs stateless:** A has only 22 gt of memory (the delay chain) and self-corrects
  within 22 gt of stable S; E has unbounded latch state that must be initialised.
- **Truncation vs stickiness:** A's only failure is a short H/L edge; E's is a missed reset /
  missed re-arm. For an unattended farm, truncation on a provably-long phase is safer.

## Decision

**Build Layout A first** (pulse lane decision; G1 is the review gate, not the measurer).
Adopt E only if ironspec/coordinator measure a farm signal with H < 22 or L < 20 on
edges that must fire — the measured signal has H ≈ L ≈ 12,000, so this fallback is not
triggered.

## Must-haves (qualified)

- ≥ 20 gt on every falling edge **with H ≥ 22 and L ≥ 20** (nominal 22 gt with H, L ≥ 22,
  boundary 20–22 gt) — the night edge satisfies this by ~545×; glitch edges are exempt
  (harmless whether they fire, truncate, or miss).
- Exactly one pulse per qualifying falling edge; no re-fire while S stays low; no output
  on a rising edge.
- No output on power-on / chunk load if S is stable (see below).
- Day-1 only: dust, torches, repeaters, levers, wood/stone. Banned: quartz, observer,
  comparator, slime, sticky piston, sculk.
- Retriggerable: re-arm 22 gt of high; period 24,000 gt ≫ 44 gt nominal cycle.
- No burnout (each torch ≤ 2 toggles per cycle), no QC dependence, no manual reset,
  survival-buildable.

## Not goals

Rising/both-edge pulses; adjustable length beyond the fixed 20–100 acceptance (A targets
22); sub-20 gt pulses; minimal footprint, tileability, silence, 0-tick; the farm, its
collection mechanism, or anything beyond the pulse generator; piston/hopper/minecart
integration in v1.

## The obvious design's measured numbers and the ceiling

- **Bar:** catalog sticky-piston redstone-block monostable, or comparator pulse extender:
  ~3×3, tunable to exactly 20 gt, single-shot. Both banned (slime / quartz). A matches
  their behaviour with torches + repeaters at +5 repeaters, ~2× footprint, +2 gt torch skew.
- **Ceiling:** without a comparator there is no analog timing; without observer/sculk no
  1-gt edge detection; without sticky piston no moving-block memory. Timing is quantized
  to repeater 2-gt steps + 2-gt torch skew, granularity **2 gt**, even L only. Best
  achievable day-1 design is the combinational delayed-AND at 22 gt (20–22 gt at boundary
  coincidences / truncation) — not exact 20.0 gt, not finer than 2 gt.

## Power-on / chunk-load — Layout A

- **S = 1 stable on load:** chain `POWERED` (restored), `T_inv` OFF, `T_out` OFF, Out = 0.
  No pulse.
- **S = 0 stable on load:** chain unpowered, `T_inv` ON, `T_out` OFF (block powered by
  `T_inv`), Out = 0. No pulse.
- **Unload mid-pulse:** block states (repeater `POWERED`, torch `LIT`) restore
  consistently — S cannot change while the chunk is unloaded (villager does not tick
  there), and remaining scheduled ticks resume. The pulse truncates or completes on
  reload; it **never re-fires** (`T_inv` ON holds `T_out` off) and never starts spuriously
  from stable S. Worst case is one shortened night hide — a rare corner, not nominal
  (farm chunk is normally loaded).
- General guarantee: any power-on state self-corrects within 22 gt of stable S; A emits
  only if S actually fell in the last 22 gt. E, by contrast, restores its latch Q and
  must be armed in RESET before its first trusted edge (see Layout E).

## Build verification (measured, not estimated)

Design file: `designs/circuits/mobfarm/tracks/ironfarm/pulse-fall-torch-delay-L22.txt`
(35 blocks, 7×2×5). Gates run on the build tree (branch `explore/iron-farm-pulse`):

- `cargo build --release -p mctas-cli` ends with `Finished` (LIBCLANG_PATH=/opt/llvm/lib,
  BINDGEN_EXTRA_CLANG_ARGS for the cadical bindgen, cargo proxy http://fwdproxy:8080).
- `mctas circuit sim` (redsim) on the design: **CIRCUIT: pulse-fall-torch-delay-L22
  falling-edge L 22 gt, delay 4 gt, min-pulse 30 gt, 35 blocks, 7x2x5, redsim**.
- `mctas circuit verify --table` (real 26.3 game, 4 positions × 4 rotations):
  judge table `in 2..20 gt: +26 for 4..20 (not handled); in 30 gt: +34 for 22;
  in 40 gt: +44 for 22; in 60 gt: +64 for 22` →
  **CIRCUIT: pulse-fall-torch-delay-L22 falling-edge L 22 gt, delay 4 gt,
  min-pulse 30 gt, 35 blocks, 7x2x5, robust**.
- Boundary probes (redsim, explicit stimulus): isolated pulse H = 20 → 20 gt pulse,
  H ≥ 24 → exactly 22 gt; trains with low gap L = 10 → 10 gt, L = 20 → 20 gt,
  L ≥ 22 → 22 gt (pulse = min(L, 22) for H ≥ 24).
- Build deviations from the v2.1 brief (no decision/target/window/must-have change):
  T_inv feeds repeater B, not B_out directly (torch direct signal is DOWN only);
  S reaches B_out only through repeater A (a diode) — a dust wire from the input stone
  to B_out feeds back and sticks the chain on; repeater delays are notches
  (getDelay = DELAY × 2 gt). Chain = 5 × delay-2 = 20 gt; A/B = delay-1 = 2 gt each.
- No merge markers in the two files (`git grep` run before commit).
