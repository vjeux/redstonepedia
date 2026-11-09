# Contraption studies (`mctas circuit`)

Seven studies in the clock study's spirit ([clocks.md](clocks.md)): for each kind of contraption,
find as many structurally different designs as possible for each value of its main number,
smallest first, every one judged in the REAL game, archived and cataloged. The brief is vjeux's
(2026-10-19, "Let's do 1-7"; study 9, TNT dupers, 2026-10-20); clockinfra owns the judge
(crates/circuit, crates/circuit-cli, this page), each track owns its crate
(`crates/circuit-<track>`, `mctas circuit <track> ...`).

| study | track | kinds | axis (the gallery's main number) | metrics |
|---|---|---|---|---|
| 1 pulse circuits | pulse | rising-edge, falling-edge, dual-edge, pulse-limiter, pulse-extender | output pulse length L | delay, min-pulse |
| 2 T flip-flops | tff | tff | mechanism family | latency, edge, min-pulse, min-period |
| 3 delay lines | delay | delay | N | min-pulse, min-gap (one-edge delays flagged) |
| 4 logic gates | gates | gate [not, and, or, xor, xnor, nand, nor, imply, nimply] | the function | latency, glitches, instant class |
| 5 memory | memory | rs-latch, d-latch, counter N, tff-reset, dff, dff-enable, shift N | the function (N) | latency, enable polarity, output style, reset, edge, setup, hold |
| 6 piston extenders | pext | piston-extender N | reach N | extend, retract |
| 7 flying machines | fly | flying-machine, elevator N, elevator-floors N S | kind + speed (gt per block) / height N / N floors S apart | latency, period / trip up and down |
| 8 shulker loaders and unloaders | shulker | shulker-loader [stack], shulker-unloader [stack] | load or unload, item stack size | boxes/min, items/s, cycle, first box |
| 9 TNT dupers | tnt | tnt-duper, tnt-duper-triggered | rate (TNT per minute) / TNT per pulse | period, latency, gap ([variations](../designs/circuits/tnt/VARIATIONS.md)) |
| 10 BUD switches | clk-obs (bud) | bud | the update classes it detects | latency, pulse, re-arm, jams |
| 11 pulse multipliers, dividers, monostables | pulsemult | burst N, divider N, monostable [retriggerable \| non-retriggerable], off-delay, on-delay, pulse-train | N / N / L + class / L / T / P | pulse, period, retrigger, repeat; duty, min-period; re-arm; latency ([variations](../designs/circuits/pulsemult/VARIATIONS.md)) |
| 12 computational redstone | clk-obs (compute), pext | adder N, subtractor N, incrementer N, comparator N, mux N, demux N, decoder N, encoder N [priority], binary-counter N | the kind and N | latency (worst transition), glitches, per bit, slice law ([variations](../designs/circuits/compute/VARIATIONS.md)) |
| 13 item sorters | clk-obs, sorter | sorter ITEM | the item stack size it sorts | items/s, first item, hoppers awake, slice width, badges overflow-proof / stream-direct ([targets](../designs/circuits/tracks/sorter/WR.md)) |
| 17 auto-smelters | sorter | smelter N | N furnaces / a tileable slice of N | items/min per furnace, first item, items per coal, spread, hoppers awake, at rest, blocks per furnace |
| 18 crop farms | w86 (farm) | harvester CROP (sugar_cane) | the crop | items per hour per block (efficiency, ranks), items per hour, blocks per cane, items per hour per cane, harvest height, wait; tileable ([board](../designs/circuits/farm/WR.md)) |
| 19 minecart stations | clk-obs (rail) | station 1 (one-way line), station 2 (two-way line), cart-unloader, cart-loader | the line (one-way / two-way; through line / terminus) | stop distance, departure delay, departure speed; unload / load rate; real carts, ridden and empty, hopper minecarts with every item counted ([board](../designs/circuits/rail/WR.md)) |
| 20 auto-crafters | clk-item | crafter RESULT (iron_block, crafting_table, stick, tnt) | the recipe class: 2x2, 3x3, multi-ingredient | crafts/min, first craft, hoppers awake ([board](../designs/circuits/crafter/WR.md)) |
| 21 automatic doors | clk-obs (autodoor) | auto-door | the door (2x2) | open wait, close delay, stop in doorway (reported); walkers both ways, walking and sprinting, two in a row, two at every gap ([board](../designs/circuits/autodoor/WR.md)) |
| 21 auto-brewers | sorter | brewer N | N brewing stands / a tileable slice of N | potions/min per stand, first potion, blaze powder, hoppers awake, at rest, blocks per stand |
| 22 auto-composters | sorter | composter N | N composters / a tileable slice of N (one crop a slice) | bone meal/h per composter, items composted/min, first bone meal, hoppers awake, at rest, blocks per composter |
| 23 bedrock breakers | bedrock (`crate::bedrock`, [brief](../designs/briefs/bedrock-breaker.md)) | bedrock-breaker N | N bedrock side by side | armed every (the longest wait between two ticks a press removes the bedrock), miss (the one-press repair); ports `bedrock[.k]`, `swap[.k]`, `press[.k]`, `start`; bedrock is not counted |
| mob farms | ironfarm (`crate::studies::mobfarm`, [brief](../designs/briefs/farm-iron.md)) | mob-farm iron_ingot day-1, mob-farm iron_ingot middle, mob-farm iron_ingot fastest (a farm in no tier: `mob-farm iron_ingot`, catalog only) | the tier: day 1 (lowest bill), middle ground (iron/h per block at 75%+ of the ceiling, 20 iron at most), fastest (iron/h per module); a farm in no tier: its modules | iron/h per block (ranks), iron/h and its range, golems/h, share of the ceiling, modules, iron/h per module, kill, worst policy, restarts after a missed night, bill, payback; on redsim's mob layer ([notes](../designs/circuits/mobfarm/tracks/ironfarm/NOTES.md)) |

No pictures: the website renders every build with redsim (vjeux 2026-10-19: "we don't need images
anymore, we should render them"); the input and output cells of a design are its markers.

## Design format

The door/clock design text format (`crates/mctas/src/design.rs`: `name`, `origin`, `palette`,
`layer Y:` rows = z, columns = x) plus:

```text
name: sub-rising-4
circuit: rising-edge             # the kind (+ its argument: `gate xor`, `counter 4`, `piston-extender 3`)
port input: 0 1 0                # one input; several: port input.<name>: (input.a / input.b, input.s / input.r, input.d / input.e, input.t / input.r, input.clk / input.d [/ input.e])
port output: 3 1 0               # one output; several: port output.<name>: (output.q / output.qbar)
port target: 0 1 2               # bud only: a cell (air) where the judge performs its block updates
stimulus: in 10+ 14- 40+ 41-     # optional, for exploring (the kind's checker makes its own)
input-device: far                # sculk designs only: the input comes from beyond the sensors' reach (no lever events)
vibration: proof                 # sculk designs only: claim vibration-proof (refused if ambient events change it)
```

* **Inputs** (`port input[.<name>]: x y z [args]`): the args say how the judge drives the cell.
  None (or `minecraft:redstone_block <off-state>`): the input is DRIVEN like a real build drives
  it (R7, 2026-10-20: "must work in a normal survival world"). The judge's delay-1 repeater
  stands in a neighbour cell of the input cell and points into it, with its support stone under
  it and its switch (a redstone block the judge places and removes) one cell further out. The
  cell itself holds the design's own receiving block (dust for a comparator's or repeater's side,
  which reads only a signal source; a repeater; the off-state block), counted, or the judge's
  smooth stone when the design leaves it air. The judge never places or removes a block next to
  the design, and the design must never move the input cell's block. The repeater takes the
  first side in the order north, south, west, east whose repeater, support and switch cells are
  outside the build's box. Failing that, a static design may be fed through its own air cells
  (switch outside; the card says "input fed through the build"), and the repeater touches no
  design block but the input cell's. Otherwise the design is refused. The switch is placed 2 gt
  before each edge, so the input cell's power changes exactly at the stimulus ticks; the change
  comes DURING a tick, as from any redstone line, so a piston or TNT reacts in that same tick,
  1 gt earlier than with the old placement between ticks. A repeater makes no pulse or gap
  under 2 gt: the stimuli drop the tests that need one, and a minimum at that floor reads
  `2 gt (the shortest a real input makes)` (a period: 4 gt). `mctas circuit r7` lists the
  archived designs this changes (designs/circuits/R7-input-devices.txt);
  `MCTAS_INPUT_DEVICE=block` brings back the old placement for comparisons. `<on-state>
  [<off-state>]` with another on-state: custom states written with updates (the design holds the
  off-state, default air; custom states are not turned with the design). `lever`: the design's
  lever there (off at rest), right-clicked at every edge. `button`: the design's button there,
  right-clicked at every rising edge (its own pulse length). Levers and buttons change between
  ticks: on at frames t..t+L-1 is an L gt pulse. Latch inputs may be spelled
  `set`/`reset`/`data`/`enable`/`clock`/`toggle` (= s, r, d, e, clk, t).
* **Outputs** (`port output[.<name>]: x y z`), read every gt: redstone dust gives its power; a
  repeater, torch or observer its own output (15 when on); a comparator its strength; any other
  cell (air, a solid block, a lamp, a piston, a bulb) the best signal INTO it (what a lamp there
  would see). On = level > 0. Put the output where a lamp would go, not on a lamp (a lamp's lit
  state lags 4 gt when it turns off). The main output is `q` when there is one, else the first.
* **A circuit must drive something** (vjeux/coordinator 2026-10-19): the judge puts a redstone
  lamp in every AIR output cell (not a design block: not counted, not part of the type; none for
  kinds read through payloads, piston extenders and flying machines, nor where a piston can move a
  block in: its head cell or the cell after the row of blocks it pushes, any cell within 13 in
  front of it when the design has slime or honey) and the lamp must follow the output: lit
  whenever the output is on, unlit within 5 gt after it turns off (a lamp turns off 4 gt after the
  update that tells it, so shorter gaps stay lit; a later 4 gt flash from a state inside one tick,
  like two inputs placed in the same tick, is a glitch, not a failure), from the first frame the
  circuit may be driven. The metrics still come from the level above, so they do not change. The
  judge's lamp only observes (clockinfra 2026-10-20): redsim runs the stimulus once more with every
  such cell left as AIR, and the output must see the same signal in every frame, else the design
  fails with `the judge's lamp is part of the circuit` (a lamp is a conductor: a comparator's
  feedback loop closing through it would not run with another consumer). An
  output that changes without a block update fails (`update-less output: ... it drives
  nothing`): a dust that is only REDIRECTED (it changes shape, its power stays the same) updates
  nothing, so a lamp, a torch on a block or a repeater there never reacts (instant
  dust-redirection gates, track delay's 1 gt line). An output cell is therefore air or holds a
  redstone component or consumer (dust, repeater, comparator, torch, observer, lamp, piston, copper
  bulb, note block, door, trapdoor, fence gate, dispenser, dropper, crafter, rail, TNT, bell,
  hopper; a lamp the design puts there is watched like the judge's): a passive block there (a
  stone...) drives nothing by itself and is refused. A dust redirected WITH a power change (the
  input stronger than the power it had: `gate-not-bump-4b`) does update, and passes.
  **Vibrations** (coordinator and clockinfra 2026-10-20, `crate::vibration`, shared events in
  `clock::vibration`): a design that holds a sculk sensor (plain or calibrated) hears game events,
  and the judge's own input (a redstone block written with `setBlock`) makes none. After its
  plain run passes, two more runs of the same schedule:
  * DEVICE, a hard rule: the game events a lever makes at every plain input cell
    (`block_activate` when it turns on, `block_deactivate` when it turns off; a button makes the
    same two) must leave the result unchanged, else `hears its input device`. A design whose input
    comes by a line from beyond its sensors' reach says `input-device: far` and gets none (its
    metric says `input from afar`).
  * AMBIENT, a property: the device run plus seeded pseudo-random game events every 5..15 gt at
    cells within the sensors' listener radius of the design's swept box (8 blocks, 16 with a
    calibrated sensor), never inside it: footsteps, items hitting the ground, blocks placed or
    broken, entities placed (frequencies 1, 2, 13, 12, 14). The metric `vibration-proof yes`
    when the result is unchanged, else `no`; a `no` design is its own archive class (axis
    `..., vibration-sensitive`). A design that says `vibration: proof` and is not is refused.
  Every event is posted at its cell's center (redsim measures block centers), and the generator
  is seeded by the build (the design as rotated and placed), so redsim and the game get the same
  events for the same build. A calibrated sensor fed a constant back signal (a comparator reading
  a chest) accepts that frequency only: `cal-sculk-filter-rise` (the note block's 10) is
  vibration-proof.
  `mctas circuit archive --rejudge` re-judges an archive after a judge change.
* **Machines**: `port start:` (the clock kick: `last`, `use [T]`, `<state> T`), `travel: dx dy dz`
  (the push or travel direction, a unit vector), `distance: k` (how far a flying machine must
  go), `port payload[.<name>]: x y z` (tracked blocks: the pushed block, the payload),
  `port station[.<name>]: x y z` (a two-way flying machine's return-station block: a design block
  that counts, left out of the machine's tracked shape; calibration: `cal-two-way-machine`).
* **Block count**: every design block except piston heads (part of their piston) and the blocks in
  input cells (a lever or button there is the stimulus device), plus design entities. A block in
  the output cell counts: leave it air when it is only a marker.
* **Stimulus** (explicit, for `--stimulus` or a design without `circuit:`): `;`-separated parts
  `[<input>] <edges>` with edges `10+ 14-` (on at frame 10, off at 14), or `pulse L every T x K
  [from S]` (from 40 by default). The input name may be left out with one input.

## The judge

`mctas circuit verify` builds the design at 4 positions x 4 rotations (the clock judge's offsets;
the game turns every block state, the ports and `travel` turn with it), each in its own sandbox,
placed exactly like `clock verify` places a clock (raw writes, a shape update of every wire, a
`self` update of every block), runs the kind's stimulus and records every gt: each input's state
read from its cell, each output's level, every non-air cell of the watched box (the circuit's box
plus a 3-cell margin; swept along `travel` for machines). A build fails when a design block cannot
stand where it is (the placement removes a floating dust; a floating lever, torch or repeater
fails `canSurvive`: a player could not build it), when anything reaches the
outer layer of the watched box, when an entity appears (a drop, a popped block), or when the
kind's checker says so. All builds passing with the same metrics: `robust`; builds that
disagree, with other metrics or failing (as for clocks): `position-dependent` /
`rotation-dependent` (allowed, flagged; the archive row notes how many builds fail and why, e.g. a
flying machine that flies east-west only). No build passing: `FAIL`. One line:

`CIRCUIT: <name> <kind> <metrics>, <blocks> blocks, <WxHxD>, robust` or `FAIL: <name> <kind>: <why>`.

`mctas circuit sim` runs the same stimulus on redsim (exact for the blocks it models, in
milliseconds) and prints the same line ending in `redsim`. Both size the stimulus from redsim's
recovery time: how long the circuit takes to come back to rest after every input turns on for 40 gt
and after they turn off, plus 10 gt (at least 20; 100 when redsim does not model the design or it
never rests; `--gap N` overrides). When an event of the kind's own stimulus (run on redsim spaced by
1000 gt) takes longer to settle than that, the gap is that settling plus 10 gt instead (a slow path
the all-on / all-off pair never takes: a crafter latch's nine ingots coming back through a hopper,
79 gt). The circuit stands 40 gt at rest after placement (and after its
kick) before the first input.

### What each kind is judged on

* **Pulse circuits**: one input pulse of each length 1, 2, 3, 4, 5, 6, 8, 10, 15, 20, 30, 40 gt,
  then 60, 80, 120, 160, 240, 320, 480... gt up to twice the gap (the gap is the recovery time, so
  a long limiter's or extender's L is exceeded by the longest input)
  (a button: 3 presses), each followed by the gap. The output must be off at rest and back off
  at the end of every gap. Per input pulse, the output pulses (delay from the input's edge,
  length):
  * `rising-edge`: exactly one pulse at the same delay after every rising edge, the same length L,
    nothing on the falling edge; `falling-edge`: the same after the falling edge; `dual-edge`: one
    pulse after each edge (L and delay per edge when they differ);
  * `pulse-limiter`: one pulse from the rising edge, length L for every input >= L (shorter inputs
    may pass shorter); L must be shorter than the longest input, and at least two inputs longer
    than L must come out exactly L (one would not tell a limiter from a delay line that shortens
    every pulse by a fixed amount; a longer gap or a `stimulus:` hint adds longer inputs);
  * `pulse-extender`: one pulse from the rising edge, length L for every input shorter than L and
    never shorter than L; at least two inputs shorter than L (every tested one when fewer exist,
    as for L 2) must come out exactly L inside the handled inputs (from the min-pulse up), so a
    stretcher that adds a fixed amount to every pulse fails.
  * `min-pulse`: the shortest input from which every longer one is handled (at least the 3 longest
    must be).
* **tff**: 8 pulses of 4 gt (A), then two each of 10, 3, 2 and 1 gt (B), then trains of 6 pulses
  (half on, half off) at periods 30, 20, 16, 12, 10, 8, 6, 5, 4, 3, 2 gt (C). Every A pulse must
  change the output exactly once (steady until the next), and so must every longer pulse (a lever
  held down). `latency`: input edge to output change;
  `edge`: rising when the change keeps its offset as the pulse lengthens, falling when it moves
  with the falling edge; `min-pulse`: the shortest pulse from which every longer one toggles;
  `min-period`: the fastest train period from which every slower one gives exactly 6 changes and a
  steady output.
* **delay**: pulses of 1, 2, 3, 4, 5, 6, 8, 10, 15, 20, 30 gt, then pairs of 20 gt pulses with
  gaps of 1, 2, 3, 4, 5, 6, 8, 10 gt. `N`: the shift of the longest pulse's rising edge; a pulse
  (or pair) is handled when the output is exactly the input shifted, both edges. `min-pulse` /
  `min-gap`: the shortest pulse / gap from which every longer one is handled. When the falling edge
  moves by another amount than the rising edge, the line says `one-edge delay` with both shifts.
* **gate**: an Euler walk through all input combinations (2 inputs: 13 states, every one of the 12
  ordered transitions once, both inputs changing in the same tick included), each held for the
  gap, from 00 at rest. The output must be steady in the second half of every hold and the same
  for every visit of a combination; the truth table names the function (inputs in name order: a,
  b; `imply` = a -> b, `imply-ba` = b -> a), a claimed function must match, a constant or
  single-input output fails. `latency`: the worst gt from an input change to the output's final
  value; `glitches`: extra output changes; latency 0 = the `instant` class.
* **rs-latch** (inputs r, s): pulses s, s, r, r, s, r of 4 gt and s, r of 20 gt; q must be set by
  s and cleared by r and stay; a declared `qbar` must be !q. **d-latch** (inputs d, e): 12 steps of
  (d, e) held for the gap; q must follow d while enabled and hold otherwise; both polarities pass
  (`enable high`: transparent while e is on; `enable low`: while it is off, a repeater lock).
  `latency`: the worst change time. **counter N**: 3N + 1 pulses of 4 gt; the output must change
  once every N pulses (`toggles`), pulse once every N pulses (`pulses`), or its level at the end of
  each pulse must repeat with period exactly N (`level`, comparator counters). `counts` (site QA
  2026-10-21): the axis is `counter N` only for ONE output pulse per N input pulses (a pulse in
  every Nth input's phase, or a level with one rising edge per N inputs, `on k of N` noted); a
  design that toggles every N pulses gives one pulse per 2N inputs (a square wave, a divider by 2N)
  and is its own class (`counter N, one pulse per 2N inputs (...)`), as are `levels only` and more
  than one pulse per N.
* **tff-reset** (inputs r, t; [crate::seq](../crates/circuit/src/seq.rs)): pulses t, t, t, r, r,
  t, r, t, t, r of 4 gt, then t, r, t, r of 20 gt, each followed by the gap: every t pulse must
  change q exactly once, every r pulse must leave q off with one change from q = 1 and none from
  q = 0 (so a reset from 0 and a repeated reset change nothing). Then t pulses of 3, 2, 1 gt (two
  each) and an r pulse, for the metric only. `latency`: the worst toggle time of the 4 gt t
  pulses; `reset`: the worst clear time from q = 1; `min-pulse`: the shortest t pulse from which
  every longer one toggles.
* **dff** (inputs clk, d): an edge-triggered D flip-flop (a 1-bit register). 8 clock cycles, each
  4 steps held for the gap: d := a with the clock low, the clock rises, d := b, the clock falls,
  with (a, b) = 10, 01, 11, 00, 10, 00, 01, 11. The run is compared with an ideal register run on
  the same schedule (q takes d at each clock edge of its type, holds otherwise), for rising edges,
  then falling edges: at the end of every step q must equal the ideal's q, steady over the second
  half of the step, with exactly one change in a step where the ideal changes and none elsewhere
  (a level latch fails: it follows d while transparent). The type that passes is reported
  (`edge`). Then setup and hold sweeps, each test a prime clock pulse with d = !v (both types end
  with q = !v) and then: `setup-r x` d := v x gt before a rising edge (x = 8, 4, 2, 1, 0: 0 = in
  the edge's own frame), `setup-f x` the same before a falling edge, `hold-r x` / `hold-f x` d := v
  well before the edge and back to !v x gt after it (x = 8, 4, 2, 1). The other type's tests must
  match the ideal (d is steady around that type's edges); the own type's give `setup` / `hold`:
  the smallest tested offset from which every larger one matches (`over 8 gt` when none).
  `latency`: the worst edge-to-q time. **dff-enable** (inputs clk, d, e): the same main sequence
  with e set at the start of each cycle, 10 cycles (a, b, e) = 101, 011, 110, 001, 100, 111, 000,
  011, 110, 101: the ideal loads only when e is on at its edge; metrics `latency`, `edge`.
* **shift N** (inputs clk, d): a shift register of N cells, q = d as sampled N clock edges ago. N +
  12 clock cycles as for dff, with (a, b) taken from two offsets of a 31-bit pseudo-random
  pattern (so each edge type shifts in a different sequence, and a register that samples on one
  edge and shifts on the other fails); compared with an ideal N-cell register of each type (q
  unknown, and free to take one value, until N edges have filled its cells). Metrics `N`,
  `latency`, `edge`. A shift-register cell is judged as a chain of 2 (`shift 2`), so cells that
  race through each other fail.
* **piston-extender N** (payload, travel): 3 cycles of the input on for the gap then off for it
  (a button: one press, then 2 gaps). The payload's state must be unique in the design. Per
  cycle: from some frame after the on edge to the off edge (a button: at some frame) the payload
  stands at p0 + N x travel, nothing moving; after the off edge the whole rest state (every cell)
  comes back and stays. `extend`: on edge to the first frame of the held extension; `retract`: off
  edge (a button: the press) to the first frame of the final rest; every cycle must give the same
  numbers (track pext's own measure).
* **elevator N** (`circuit: elevator N`, `travel: 0 1 0`; crate::elevator, track fly 2026-10-21): a
  vertical two-way flying machine between floors N blocks apart, a button at each floor (`port
  input.bottom`, `port input.top`: the design's buttons) and a platform; the judge's armor stand rides
  it from `port rider` (the cell above the platform, the machine at the bottom floor). Rule B
  (coordinator): a press at the floor where it stands sends it, anywhere else calls it, presses
  mid-trip latch (it rests at the floor implied by the last press); a press within 20 gt of an
  arrival or a departure at that floor may go either way; the same button is not pressed again
  within 21 gt. Phases: fixed calls, sends and mid-trip calls, 6 seeded random sequences, a second
  press at either floor at 16 offsets across a trip and every 3 gt around the arrival; each phase
  ends with the machine still for 60 gt, intact, at the expected floor, the rider alone on the
  platform there. Each floor's button must be within a player's reach (4.5) of the rider's eyes.
  Metrics: `height`, `trip` (up and down), `rider`.
* **elevator-floors N S** (`circuit: elevator-floors N S`, `travel: 0 1 0`; crate::elevator, track
  fly 2026-10-21): the multi-floor elevator, N floors S blocks apart. Destination buttons: every
  floor has one button per floor (`port input.<floor>-<dest>`, floors counted from 0 at the
  bottom), each within reach of the rider at its floor. Rule: the car rests at the floor named by
  the last press; it never reverses mid-trip (a target ahead stops it there, one behind it runs on
  to the shaft's end and comes back). Presses at least 21 gt apart (closer presses are
  simultaneous). Phases: every send and call from every floor, a press at the floor where it
  stands, re-targets at four offsets of each trip, 12 seeded random sequences, and a second press
  for each floor at 12 offsets across a full-shaft trip; each ends with the car still for 60 gt,
  intact, at exactly the expected floor, the rider on it. Metrics: `floors`, `trip` (the full
  shaft up and down), `rider`.
* **flying-machine** (kick, travel, distance): one flight of 200 + 60 x distance gt in a corridor
  swept `distance` + 3 cells along travel (a machine reaching its far end ends the run there; any
  other face fails). Track fly's tracker ([`crate::fly`], from crates/circuit-fly): per frame the
  machine's translation (structural states, piston heads part of their piston, a moving block = in
  motion; station cells holding their own block left out of the shape, never out of the frame) and
  a canonical frame; the steady state is frame t + P = frame t shifted by d cells for at least 2
  periods, an in-place run (d = 0, frames changing) winning over a moving run it outlasts. It must
  fly along `travel` and stay intact over `distance` cells (or to the corridor's end); a steady
  shuttle in place over 2+ positions (a two-way machine between its return stations: the round
  trip repeats in place) passes as two-way. `speed`: P / d gt per block; `two-way`: the round
  trip's period and the positions it stands intact at; `latency`: the kick to the first change;
  the box is the design's own.
  **Start device** (a one-way machine with a lever and no kick, or a button: `flying_start`): parked
  at rest (no cell changes before the first flip), launched by the first flip, intact over its
  distance in a steady flight while the lever keeps flipping; its station (the device and its
  support, `port station`) ends as built; one run per flip gap of the real-use audit
  (`check::LEVER_FLIPS`: 4, 5, 6, 8, 10, 12, 16, 20 gt; off k gt after the launch, on again, off
  again, then twice long after it left), each judged; metric `start flip the lever` (or `press the
  button`). **On/off lever with a kick** (docking stations: `flying_switch`): off at rest, then four
  on/off cycles; off must dock at the same place each time, on must leave, fly 3+ cells and come
  back, the round trip repeating until the switch turns off. **Ground** (every horizontal machine:
  `robust::fly_clearance`): the fewest air blocks over grass (on dirt, under the whole watched box,
  `Circuit::grounded`) at which it still passes, measured on redsim and flown at in every game
  build; an observer in the lowest layer watching down needs air (grass under it turns to dirt on a
  random tick); metric `ground flies along the ground` / `needs N blocks of air above the ground`;
  `circuit sim --ground N` runs one design over it.
* **tnt-duper** (study 9, [`crate::tnt`]; no input, an optional kick): the runners record every
  primed TNT (redsim: a prime record; the game: each new TNT entity, read one tick after it spawns,
  is killed by its UUID before it can explode, so explosions never happen). 1800 gt after the kick
  (8 periods of up to 110 gt fit in its second half)
  the frames must repeat with a period P through the run's second half for at least 8 periods,
  every period must spawn the same n >= 1 primed TNT, and every TNT block of the design must stand
  again in every period (a moved TNT is in flight for 2 gt: the most seen in the period counts).
  `rate` (the gallery's axis): 1200 n / P TNT per minute; `period`, `per period`, `latency` (the
  kick to the first spawn). The spread of the primed TNT's position and motion (precision: the
  game's random 0.02 kick) is in the table, not in the metrics the builds are compared on. In the
  real game each TNT is followed to fuse 1 (removed there): when every TNT of the periodic region
  that reached fuse 1 is at the same point then, bit for bit (8 or more), the line ends with
  `(precise: N TNT at one point at fuse 1)` (the PRECISE variation; it keeps its own catalog card).
  **Explosions are on by default** (vjeux 2026-10-20: "You didn't model tnt explosion. This will
  blow up the machine"), and the duper stands **on the ground** (vjeux 13:13Z: "There's no way
  this tnt duper doesn't explode after a few seconds": the first judge's void sandbox let every
  dropped TNT fall away forever). The ground (`Circuit::ground_blocks`) is a superflat floor, grass
  over two dirt over bedrock (explosions leave craters, as in a real world), or obsidian (no
  craters); a slime frame resting on grass drags it, so a design that needs air under its moving
  slimes carries its own foot (an obsidian block one lower: the ground goes under it).
  **SAFE DROP HEIGHT** (vjeux 13:31Z, the metric `safe drop N blocks`, `0 blocks (on the ground)`;
  a card reads `safe on the ground` / `needs an N-block drop`): the smallest number of air blocks
  between the duper's lowest block and the ground at which it stays intact on BOTH grounds for
  every kick (`circuit::robust::safe_drop`: a screen at 16 kicks per ground for drops 0, 1, 2 ...
  40, then the full all-kicks sweep at the first drop that passes); none up to 40 fails (`it needs
  a pit deeper than 40`). Every redsim run is strict: no TNT is removed, each explodes where it
  lands, and the ground grows (24, 48, 96 blocks around) until no burning TNT ever leaves it. The
  game run stands the duper that high above a grass ground covering its whole loaded area,
  `run::GAME_REACH` = 40 blocks around; no TNT is removed, and a burning TNT reaching the loaded
  area's edge (where it would freeze, never exploding) fails the run. The duper must survive: a
  self-running one's block frames (the ground below its lowest layer left out) repeat with its
  period from before its first explosion through the end of the run (`blown up: its first
  explosion (frame F) changes it` otherwise); a triggered one (pulses at least `tnt::TRIGGER_GAP`
  = 100 gt apart: one TNT's fuse) waits 100 gt after its last pulse and must be back in its rest
  state; a switchable one's running windows end with the same frames as its first period, its
  stopped windows the same; no entity at the end but the craters' items and burning TNT. The line
  then carries `(survives its own explosions: N exploded, design intact)`. **Useful TNT** (vjeux
  2026-10-20: "a tnt duper that explodes in water is useless"; `tnt::useful`): every TNT must go off
  where it destroys blocks: never with a fluid in its center's cell (water, lava, a waterlogged
  block: every ray loses 30 power there), and with something to destroy around it on open ground
  (`redsim::explosion::open_ground`: stone under the blast's own cell layer, the fluids where they
  are, the weakest rolls; 0 blocks fails). The game runner takes each blast's center from the TNT's
  last sighting (fuse 1) plus its last move and the fluids from that frame; redsim records it per
  explosion (`Blast::in_fluid`, `open_ground`). A passing duper's line adds `useful: every blast
  clear of fluids, each destroys blocks on open ground`; the table lists where every blast went off.
  No drop height cures a useless TNT (`robust::safe_drop` reports it at once). On redsim (clk-misc's
  exact explosion port, `clock::sim::drive_tnt`) every primed TNT is kicked by one `d` and every
  ray is at its largest roll (`Rolls::Max`: surviving it is surviving every roll). **All randoms**
  (vjeux 2026-10-20: "pass every possible random number"): the kick's `nextDouble()` swept over
  [0, 1) at 4096 points, every change bisected to 1e-12 (`redsim::robust::critical_points`), every
  piece and boundary judged on both grounds; every one must pass with the same metrics (and
  PRECISE only when every kick is precise), then 64 runs per ground where every TNT gets its own
  kick (`robust::PER_SPAWN_RUNS`, a fixed generator) must pass with the same metrics (`mctas
  circuit tnt robust` alone, at the safe drop; `mctas circuit tnt judge` one run). `Rolls::Max` covers every roll because a blast's destroyed set only
  grows with the rolls and knockback ignores them; a destroyed machine block fails the run, the
  ground's craters do not (clockinfra's check). The evidence: `mctas circuit tnt blasts` lists
  every explosion's distance to the machine and the blocks destroyed, in the void and on both
  grounds at the safe drop (`--ticks` for a long run). A clock-held engine (the study's one duper,
  tnt-clocked-22b): `mctas circuit tnt hold-scan <engine> --piston x,y,z` tries every archived
  clock and turn for a cell where a piston is powered on all but one short window of the period,
  puts the engine's piston there and judges each machine on redsim. MINIMUM VOLUME (vjeux
  2026-10-21 "treat this as the door experiment where we want to find the minimum volume"):
  `mctas circuit tnt compact <seed> [--region box] [--depth lo..hi | any | above[:CLEAR]] --out DIR`
  anneals inside a box on redsim (moves: delete / step / move / replace / add / turn / swap /
  translate a cluster / relocate a cluster turned / push a face layer inward, ports following their
  blocks, repair bursts after big moves), scored in tiers (dupes 1 per period > one fuse-1 point over
  the screening kicks > the burst depth in range, or an air burst clear of the machine > the redsim
  judge passes with precise in mid-air, design intact and useful) then by volume then blocks; a
  `--region` smaller than the seed is a squeeze (blocks outside are moved in one at a time, never at
  the cost of the tier). `mctas circuit tnt graft <design> --module cells --anchor x,y,z --targets
  cells|box [--rotations all|y]` re-places one module (a clock with its converter) at every target
  in 24 or 4 orientations and judges each. With them the precise drop went 324 -> 240 (6x5x8, burst
  10 blocks under the machine) and the air burst 528 -> 384 (designs/circuits/tracks/clk-rt/).
  A lever-input seed is screened with its lever used at tick 10 (the judge's action; TNT carried by
  a moving piston counts as kept), and `--effective` ranks by the judge's `effective volume`
  (footprint x (height + max(stand, safe drop))) without the fast duper's on-the-ground gate, for
  stand and pit designs (archive those only after `circuit tnt robust`).
  ON/OFF LEVER (vjeux 2026-10-21 "Can we have an on off switch for all the tnt"): a `tnt-duper`
  with a `lever` input is judged by `circuit::tnt::check_lever` (off as built: nothing primed,
  nothing moving; 4 on/off cycles, each off edge 11 ticks later than the last; each off stops
  priming within 40 ticks and stands still with the same blocks as at rest, every TNT block back;
  each on restarts at the same period and TNT per period; flags measured over every TNT, start-ups
  and stops included); the card adds `switch lever off: stops within X gt; on: restarts within Y gt`
  and the archive axis `, switchable`. `mctas circuit tnt lever-scan <design> [--gate] [--want
  precise:] [--near cells] [--out DIR]` gives a design its lever: every inverter around it (a torch
  standing or on a wall, a new stone under or behind it, the lever on another face of the stone) or
  with `--gate` every sticky piston the lever drives that pushes one of the design's blocks home;
  each judged on redsim with the lever stimulus (a machine the lever stopped elsewhere than the file
  draws it is tried again built as it stops), the passing ones written smallest box first.
  `circuit tnt turn <design> --turns N --out F` writes a design turned N quarter turns (to record one
  of the judge's rotated builds): `circuit sim` judges a TNT duper's 3 turned builds too and flags a
  run `rotation-dependent` when one fails or measures otherwise (the game judge's verdict).
  `explode: off` is for analysis only (the
  measuring calibrations): the archive
  refuses such a design. The catalog axis reads `<rate> precise` / `<rate> switchable`...; the
  survival is no longer a variation of its own but the baseline.
  Direction flags (real game, each TNT followed to fuse 1): `launched at (vx, vy, vz) blocks/gt,
  spread (...)` when every TNT of the periodic region reaches 0.4+ blocks/gt sideways (its largest
  horizontal motion, averaged); `stationary: every TNT at rest for N+ gt before fuse 1` (the same
  position bit for bit for 10+ gt); `dropped: every TNT falls straight down, N+ blocks by fuse 1`
  (3+ blocks below its first sighting, horizontal motion under 0.01 at fuse 1); `slime-free` (no
  slime block in the design; `: a honey frame` with honey). The axis takes the first of
  switchable, survivable, precise, dropped, launched, slime-free; each keeps its own catalog card.
* **tnt-duper-triggered** (one input): 6 pulses of 4 gt spaced by the gap; nothing may spawn before
  the first pulse, each pulse's window must spawn the same k >= 1 primed TNT and end with every TNT
  block standing. `per pulse`, `latency` (the pulse's start to its first spawn), `gap`.
* **tnt-duper with `port input:`** (the lever on/off variation): windows of 400 gt with the input
  off, on, off, on; it must dupe in exactly one input state and spawn nothing in the other from 40
  gt after the switch. A run window is judged from 40 gt after its switch (a start-up may differ):
  the smallest P with the same spawn count at every tick and P ticks later, 8+ periods (so P <= 45
  gt), n >= 1 TNT per period (several per period, same tick or offset, are fine), every TNT block
  standing again over the last period; both run windows give the same (P, n). `rate`, `period`,
  `per period`, `runs input on|off`, `stop` (the switch to the last TNT), `start` (the switch to
  the first); the axis reads `<rate> switchable`. In the game run, a burning TNT touching a
  pressure plate, tripwire or detector rail fails the run (it would react in the game, not on
  redsim), and a TNT whose fuse is missing from its saved data is a judge error.
* **shulker-loader [64|16|1]** and **shulker-unloader [64|16|1]** (study 8, track shulker,
  crates/circuit/src/shulker.rs; the variations and how each is measured:
  designs/circuits/shulker/VARIATIONS.md). The number is the scenario item's max stack:
  cobblestone (the default), snowballs, iron shovels. These kinds are not judged by the waveform.
  The design declares air cells `port items[.k]:`, `port boxes[.k]:` and `port output[.k]:`, and
  the judge puts a single chest on each (not counted, not part of the type). A loader's `items`
  chest is a source the judge keeps topped up between ticks until 3 boxes' worth is supplied; its
  `boxes` chest holds 4 empty boxes. An unloader's `boxes` chest holds 3 full boxes, and its
  `items` chest is a sink the judge empties and counts. The judge empties the `output` chests and
  records every box that arrives (the gt, colour, name, contents) from per-gt block-entity frames.
  An optional `port input: x y z lever` is turned on once at the start; without it the design is
  always on. The design is placed and left 40 gt, then the chests are filled. Pass:
  * a loader delivers 3 boxes, each holding 27 full stacks of the item;
  * an unloader delivers 3 empty boxes, and every item reaches `items`;
  * nothing else reaches an output, no item or box is left in the machine, no entity is alive at
    the end, and the design stands still for 400 gt (it gets up to 5 windows to settle).
  The run is capped at one hopper's pace x 1.25 + 2000 gt. Metrics: `items/s` is, for a loader,
  the items in its delivered boxes over the gt from the start to the last box (hoppers and
  droppers buffer what leaves the source, so the source's outflow would overstate it; a serial
  loader's first box takes about one cycle, so this is its cycle rate, and boxes filled in
  parallel count together); for an unloader, the steady flow into its items chests (per-gt
  block-entity frames) between 10% and 90% of the total, over the gt between those two moments;
  `boxes/min` is that rate over the items per box; `cycle` is the first to the
  last box over the boxes between; `first box` is the gt from the start. A piston drops the box at
  a random offset with a random drift, which can shift a pickup by a gt. Builds within 4 gt (and
  3% of the flow) of the base build are the same circuit (`robust`), and the reported metrics are
  the builds' medians. `circuit sim` reports UNMODELED until redsim models shulker boxes and chests.
  A design that passes its BASE also gets the line `badges`: the variations below it handles, each
  judged at 4 rotations with unstackables (27 items per box: fast runs), unless noted:
  * `always-on`: no start lever;
  * `16-stack` and `unstackable` (64 kinds): the same run with snowballs or iron shovels;
  * `colours`: the boxes are red, blue, a plain one named "Box A", then plain, and each delivered
    box is one the judge supplied, with its colour and name;
  * `restart` (loaders): the source stops for 3000 gt once 1.5 boxes' worth was supplied;
  * `top-up` (loaders): the 3 supplied boxes already hold 10 stacks and a half, 1 item and 26
    stacks (no spare: the dispenser places a random slot), and must leave full;
  * `flush` (loaders): 2.5 boxes' worth: the half box must arrive within 6000 gt after the source
    runs dry for good (longer than `restart`'s pause: a design that waits 3000 to 6000 gt gets
    both; one that flushes at once cannot have `restart`);
  * `mixed` (loaders): 3 iron ingots once, then 16 rounds of cobblestone 64, snowballs 16, dirt
    37, cobblestone 20, snowballs 5, dirt 64 (6 empty boxes): every item must end in a delivered
    box, every box but the last with 27 occupied slots, the last within 6000 gt of the source
    running dry. The rare item's slot stays partial for good, so a design that waits for a box to
    read full jams;
  * `filter` (loaders with a `port reject:` chest): one box's worth of the item in full stacks,
    8 dirt or 2 sticks after each: the box must leave full of the item alone, and every other
    item must reach the reject chest. What the design's own containers hold at the start (a
    sorter's filter hopper) is theirs: it is left out of the end inventory;
  * `box-starved`: 2 boxes at the start (empty or full); the judge adds the rest 3000 gt after the
    2nd box arrives;
  * `drop-free`: entities polled every gt: every item entity is picked up within 8 gt and never
    touches the ground;
  * `tileable(N)` (a `tile: dx dy dz [copies]` header, the step between copies, N its length):
    the judge builds the copies (3 by default) side by side, each copy's ports renamed
    `<port>.t<k>`, and each copy must deliver its own 3 boxes from its own chests (no block or
    port may land on another copy);
  * `combo` (a `port input.mode: x y z lever`): the other job on the same chests with the mode
    lever turned on at the start (a loader judged as an unloader: its `boxes` chest gets full
    boxes, `items` becomes the sink, `output` gets the empties; or the other way). The base run
    leaves the mode lever off.
  After the builds pass, the DROP STRESS: 64 more runs at the rotations in turn, each with other
  drop draws (unstackables, or the design's own item when it sorts; every other run a burst:
  boxes that already hold all but one stack, so they break right after they are placed and the
  drops follow each other within the collection hopper's cooldown). Two failing runs fail the
  design (`fragile`): a drop drifts about half a block before a hopper takes it, longer when that
  hopper is busy, so a missing wall loses one every few runs; one failure alone is tolerated (a
  drop pushed into a block takes a random way out: a sound design can lose one in a few hundred
  bursts). `MCTAS_SHULKER_STRESS=N` runs N; `MCTAS_SHULKER_BADGE=burst`
  (with `MCTAS_SHULKER_SALT=k`) judges a burst run in place of the base, with its tables.
  Then the FULL CHEST (`shulker::FULL_CHEST`, 2026-10-21): a chest full of boxes as a player fills
  it (a loader: 27 empty boxes and 26 boxes' worth of unstackables; an unloader: 27 full boxes),
  every box loaded or emptied and delivered, at every rotation; a failure fails the design
  (`jams with a full chest`). A box-arrival line that fires the dispenser on the box chest
  comparator's on and off jams there: with the hoppers and the dispenser full the chest never
  empties. Then the ENDLESS SUPPLY (`shulker::endless_scenario`, 2026-10-21): that chest kept
  full, its starting boxes set back before every gt (a hopper line from storage keeps a box chest
  full; the site viewer's run does exactly this); every delivered box right and at least the
  scenario's number of them (the boxes left inside are the supply's, so the conservation and rest
  checks are skipped); a failure fails the design (`jams with an endless box supply`). A design
  that fires its dispenser on the box chest comparator's level changes jams there: the level never
  changes. Then ONE BOX AT A TIME (`shulker::one_box_scenario`, 2026-10-21): the plain boxes come
  one per visit of the judge, as a hopper feeding the box chest brings them; a box that lands in an
  empty chest is pulled out before a comparator on the chest updates, so a design that sees boxes
  arrive through the chest jams (`jams with boxes coming one at a time`); reading the first hopper
  works. Then (loaders) the same trickle of nearly full boxes (the drop stress's burst boxes): a box
  can land right after a break and fill at once, so a once-per-box switch that needs time between a
  break and the next full box (a sticky-piston shuttle needs about 6 gt) misses it
  (`jams with nearly full boxes coming one at a time`); the copper bulb does not. This one runs at
  three visit paces (the default 20 gt, 17 and 23: `shulker::ONE_BOX_BURST_PACES`), so the boxes
  land at different points of a fast cycle: a loader that fires its breaker on both edges of the
  full test passes at 20 gt and jams at 23. `circuit shulker watch --trickle EVERY --sweep` runs
  every phase of a pace on redsim (a design aid).
  `circuit shulker rs --only full-chest` / `--only endless` / `--only one-box` /
  `--only one-box-burst` (`-17`, `-23`) are their redsim screens (and `--only burst` the drop
  stress's burst run). Last, ITEM INTRUSION (`shulker::intrusion`, 2026-10-21, the real-use
  audit, docs "Real use"): the intakes are the open hopper tops a stray item can reach
  (`circuit::intrusion::intakes` with a chest in every port cell, `shulker::intakes`); the
  unstackable run again with the audit's stray stacks (1 cobblestone, 64 iron ingots, 1 iron
  shovel) dropped on each at gt 150, 350 and 550 must deliver every box as without them (as many,
  each holding as much: the strays may end in a chest and the timing may shift), rotation 0
  (`loses a box to stray items`). `circuit shulker intakes <designs>` lists the intakes, whether a
  comparator reads the hopper under each (the timing path), and the redsim run with strays.
  The `bill` line is the survival cost of the blocks: iron, quartz, redstone dust, slime balls
  (hopper 5 iron; piston 1 iron + 1 dust, sticky + 1 slime; observer 1 quartz + 2 dust;
  comparator 1 quartz + 3 dust; repeater 3 dust; redstone block 9 dust; dispenser, dropper, torch
  and wire 1 dust). `hoppers awake` (lag) counts the design's hoppers left unlocked at rest: each
  looks for items every gt. What a design's own container entities (a hopper minecart) hold at
  the end counts as left in the machine. The table adds `badge X: yes` or `no (why)` lines.
  `MCTAS_SHULKER_DEBUG=N` (snapshots of the first N gt) and `MCTAS_SHULKER_WATCH=a-b` (1 gt steps,
  snapshots of a..b) add what changed to `--table`: a design aid. `MCTAS_SHULKER_BADGE=<badge>`
  judges that badge's scenario in place of the base (with its tables), the way to see why a badge
  fails. Each build seeds the level's random from where it stands (the stress runs mix in their
  number), and the bridge seeds each drop's own random from the sandbox's seed (docs/mechanics.md,
  "Drops and the bridge's seeds"), so a run is reproducible and the 16 builds, the badge runs and
  the stress runs sample different drops (a drop's offset and drift are random): a design that
  loses a drop now and then shows up as a failing build, a missing badge or `fragile`.
  `mctas circuit shulker shrink <design> [--badges a,b]` (crates/circuit-shulker) removes blocks
  greedily, each candidate judged in the game on the plain scenario at 16 builds and every kept
  badge at 4 rotations, then verifies the result. `circuit archive --remove <study>/<file>` takes a
  design another one dominates out of the archive. `mctas circuit shulker trace <designs>
  [--exact] [--stack N]` records real-game traces for the viewer
  (designs/circuits/shulker/traces/README.md); `mctas circuit shulker compare <exact traces>`
  replays them in redsim (the judge's run: the chests, the settle, the supply, the lever, then
  each gt a tick, the frame and the judge's visit; drops from the trace's seed), every cell and
  entity compared every tick. redsim models shulker boxes (27-slot block entities, comparators,
  hoppers in and out, never a box into a box), single chests, a dispenser placing a box (the
  clicked face: its facing over air, else up) and a piston destroying one into the item entity
  the bridge spawns (`redsim::rng::Drop`, docs/mechanics.md "Drops and the bridge's seeds"), the
  box's contents carried as the stack's payload; a dispenser whose filled slots all hold the
  same stack picks the first (every pick gives the same world). 10 of the 12 archived designs'
  traces are tick-identical; a dropper choosing among different stacks (the level random) and a
  minecart pushing entities are not modeled. The judge itself stays in the game. The in-browser
  viewer runs the designs live with the same driver of the judge's chests (endless, docs/webrender.md).
* **bud** (study 10, [crate::bud](../crates/circuit/src/bud.rs), the variations and why:
  [designs/circuits/bud/VARIATIONS.md](../designs/circuits/bud/VARIATIONS.md)): no input (a latched
  design: one reset input, below); one or
  more `port target[.<name>]: x y z [<state-a> <state-b>]` cells, AIR in the design (default states
  `oak_log[axis=y]` / `[axis=x]`), and one output; optional claims `bud: place remove state shape
  update`. The judge performs five classes of block update at every target, 3 times each with a
  gap between (the recovery time, 30..200 gt): `place` (air -> state-a, flags 3 like a player),
  `remove` (-> air, flags 3), `state` (state-a <-> state-b, flags 3), `shape` (the same change with
  flags 2: shape updates only, no block update, how crops grow) and `update` (a pure block update
  around the cell: `updateNeighborsAt`, nothing changes), with idle spans before and after. A class
  is DETECTED when every one of its events gives exactly one output pulse with one latency and one
  length (some but not all: `unreliable`; changing: `unsteady`), not detected when none does; at
  least one class must be detected, every claimed class too; idle gives nothing (`false trigger`)
  and the output is off at the end of every gap (`latches`). Then pairs of events 30, 20, 15, 10,
  8, 6, 4, 2 gt apart (per family: update, state, shape; place, remove, place for the presence
  family, so a place-only or remove-only design is measured too) give the `re-arm` time (the
  shortest gap down to which every detected event gives its own pulse); a pair that leaves the
  output on JAMS the design (`jams: <family> pair N gt apart`: reported, and nothing after it is
  measured).
  LATCHED designs (manual reset): one input, `port input.reset: x y z [<on-state> <off-state>]`
  (any states: `minecraft:air minecraft:redstone_block` makes the reset take a redstone block away).
  The judge pulses it (4 gt) once right after placement and half a gap after every event, probe and
  re-arm event; the output must hold from detection until that reset and go off after it (`not
  latched` otherwise), and hold through a removal while it is on (`not a latch`: a toggle lets
  go). `reset` replaces `pulse`: how long after the reset starts the output goes off; `re-arm`:
  how soon after the reset ends the next event is seen. The axis says `latched (manual reset)`.
  TILEABLE (`tile: dx dy dz [copies]`, the shulker study's header and copies: `shulker::tiled`):
  the judge also builds the copies (3 by default) side by side along the step, each copy's ports
  renamed `<port>.t<k>`, runs every class at every copy's target (the classes part of the
  stimulus, the same gap) and checks each copy's output: its own copy's events give exactly what
  the single design gives (detected classes, latency, pulse; latched designs: every copy's reset at
  once), the other copies' events nothing (`not tileable: ... cross-talk`). The judge's lamp sits
  in every copy's air output cell, so outputs that touch can cross-talk through the lamps (a lamp
  strongly powered by an observer or a repeater lights the next one; one lit through a block does
  not). Metric `tileable every N (dx dy dz)`, N the step's length: targets every N blocks.
  `mctas circuit bud tile <design>...` finds each design's densest pitch per axis on redsim, and
  `bud census --tile N` each candidate's.
  Before the pairs, the FOOTPRINT: a place, a removal and a pure update at every air cell next to
  the design that is no port of it (`Circuit::probe_cells`); `footprint` counts the targets plus
  the probe cells where any of them gives a pulse (`>=` when a probe jams it). Each probe must
  leave the design as it found it (every cell as just before the probe once its block is gone):
  one that does not (the design carried the probe block away, or broke) makes it `fragile`, and the
  later probes and the re-arm pairs are not measured. The box leaves out the target cells and the
  probe block in the probe cells.
  Axis: `detects <classes>`; metrics: latency, pulse, re-arm (per class when they differ:
  `latency 2 gt place / 6 gt remove`), jams, footprint, fragile, tileable.
  The judge's actions in the game: the bridge's `set` (modes `update` and `shape`) and `update`
  (`neighbors`) ops; on redsim `World::set_block` (flags 3 / 2) and `update_neighbors_at`.
* **Study 11 timers** ([crate::pulsemult](../crates/circuit/src/pulsemult.rs); what earlier studies
  already judge and why each kind is new: [designs/circuits/pulsemult/VARIATIONS.md](../designs/circuits/pulsemult/VARIATIONS.md)):
  one input, a judge-placed block or a lever (a button cannot give the judge's lengths), one output,
  off at rest (dividers excepted: they hold a state). Every input is followed by the gap; the output
  must be back off at the end of every gap.
  * `burst N` (N >= 2): input pulses of 1, 2, 3, 4, 6, 10, 20, 40 gt: every input from the min-pulse
    up (the 3 longest at least) gives exactly N output pulses, at the same delays and lengths as
    the longest input's (a burst does not depend on the input's length: a dual-edge detector's
    second pulse follows the falling edge and fails). Then pairs of 4 gt inputs 5..160 gt
    apart: a second input inside the burst's span is `ignores` (N pulses in all), `adds` (2N),
    `restarts` (between), `breaks` (other) or `mixed` over the delays; then trains of 6 inputs every
    40..4 gt: `repeat` is the fastest period from which every slower train gives exactly 6N (the
    xN frequency multiplier). With a min-pulse over 4 gt both read `n/a` (the pairs and trains are
    4 gt inputs). Metrics: N (axis), pulse and period (output lengths and rising-edge
    spacing, min-max within the burst: one value each when the burst is evenly spaced), latency,
    span, min-pulse, retrigger, repeat.
  * `divider N` (N >= 3; /2 is `tff`): square clocks of periods 40, 30, 20, 16, 12, 10, 8, 6, 5, 4,
    3, 2 gt, each N warm-up periods + 4N, then the gap: the output must repeat with period exactly
    N x P with 4 rises in the 4N periods. Metrics at 40 gt: duty, `square` (on half the time, to
    1 gt) or `pulse (<on> gt)`, latency (input rise -> output rise); `min-period`: the fastest clock
    from which every slower one divides. A counter of slow pulses is study 5's `counter N`.
  * `monostable [retriggerable | non-retriggerable]`: isolated pulses of 1..160 gt give one pulse of
    L at one delay from the min-pulse up (as `rising-edge`); then pairs of 4 gt inputs whose second
    rising edge comes d = 5..160 gt after the first: while d < L a `non-retriggerable` one gives one
    pulse of exactly L, a `retriggerable` one a pulse ending L after the second edge; any other
    shape, or a class that changes with d, fails (L >= 6 and a min-pulse of 4 gt or less to be
    tested). `re-arm`: the smallest d
    from which every pair gives two full pulses. Axis `L = <L> <class>`.
  * `off-delay`: isolated pulses of 1..80 gt: one output pulse at a fixed latency, the input's
    length + L (L >= 1); 3-pulse trains of 4 gt inputs with gaps of 1..32 gt: gaps shorter than L
    give one pulse ending L after the last fall, longer ones three pulses (a min-pulse of 4 gt or
    less, to run them). A repeater ORed with the
    input fails: it holds one pending tick, so it ends L after the second input of a close train.
    Site QA 2026-10-21 (a pressure plate keeping a light on: a short input): the stay-on L must
    come from a 20 gt input; a design whose min-pulse is longer (an item timer needs its input
    held about as long as its delay to reload) is its own class, `off L = .., needs a long input`.
  * `on-delay`: isolated pulses of 1..80 gt: no output below the threshold T (>= 2: the input must
    be held T gt), one pulse of the input length - the `shortening` at a fixed latency from the
    input's fall for every input from T up. Metrics T (axis, `on T = ..`), shortening, latency;
    an off-delay's axis reads `off L = ..`.
  * `pulse-train` (a gated clock): the input held 20, 60, 200 gt: output pulses every P gt while it
    is on, the first at the same latency each time, none starting more than P after the release.
    Metrics P (axis), pulse, latency, stop (release -> the output's last fall).
* **Study 12, computational redstone** ([crate::compute](../crates/circuit/src/compute.rs), the
  variations and why: [designs/circuits/compute/VARIATIONS.md](../designs/circuits/compute/VARIATIONS.md)).
  Unsigned binary on one cell per bit: bit k of `a` is `port input.a<k>` (k = 0 the lowest), of the
  sum `port output.s<k>`. Kinds and ports (`?` = optional):
  * `adder N`: a, b, cin? -> s, cout?; `subtractor N`: a, b, bin? -> d, bout? (d - 2^N bout = a - b -
    bin); `incrementer N`: a, cin? (else +1) -> s, cout?;
  * `comparator N` (N >= 2): a, b -> any of eq, ne, lt, gt, le, ge;
  * `mux N`: s0..s(N-1), d0..d(2^N - 1) -> q = d[s]; `demux N`: s, d -> q0..q(2^N - 1);
    `decoder N` (N >= 2): a, e? (enable) -> q one-hot; `encoder N [priority]` (N >= 2): d0..d(2^N - 1)
    -> q, valid? (a plain encoder is judged on one-hot and zero inputs only, a priority encoder on
    every input: q = the highest set index);
  * `binary-counter N` (N >= 2): clk, r? (reset) -> q0..q(N-1).
  Combinational kinds (at most 16 inputs): every input state in Gray-code order, then (10 inputs or
  fewer) every single-input transition in both directions (an Euler tour of the n-cube), then every
  state reached from its complement (every input at once), each held for the gap (redsim's settle
  time, at least 10 gt so that no torch toggles 8 times in 60 gt). At rest the outputs must read
  the all-zero state's values; in every hold they must settle to the ideal values and keep them to
  its end (`wrong output: <transition>: <output> is 1, want 0`). Metrics: `latency` (the worst gt
  from an input change to the frame from which every output holds its final value, with the
  transition: `b0 rises from a=0111 b=0000 cin=0`; 0 = class `instant`), `glitches` (holds where an
  output changes more than once before it settles), `per bit` (blocks / N, arithmetic kinds).
  `binary-counter N`: 2^N + 3 clock pulses of 4 gt each followed by the gap (with a reset input,
  resets at three counts): the outputs read the count mod 2^N at the end of every gap; `latency`
  from the edge that moves them (`edge` rising or falling), `per bit`.
  **Bit slices** (`slice: dx dy dz`): the design is ONE bit (`adder 1`, `subtractor 1`,
  `incrementer 1`, `binary-counter 1`) with chain ports, input cin (bin; a counter's clk) and output
  cout (bout), cout a design block exactly one step from the cin cell (`compute::sliced`). The judge
  judges the slice alone, then stacks 2, 3 and 4 copies (copy k one step further, its per-bit ports
  renamed a<k>, b<k>, s<k> / q<k>, copy k's cin cell being copy k - 1's cout block; copy 0's cin
  stays the input and keeps its own idle block, the last copy's cout the output) and judges each
  stack as the N-bit kind. Metric `slice`: blocks per bit and the latency law (`latency 2 + 4
  (N-1) gt` when it is linear). A chain that is on at rest (a torch) needs copy 0's chain input to
  idle on as well: `port input.clk: x y z minecraft:air minecraft:redstone_block` (on = air).
  **Hex** (`adder N hex`, `subtractor N hex`, `incrementer N hex`, `comparator N hex`, N <= 4): each
  value on ONE line as a signal strength. Value inputs `port input.a: x y z strength` (and b) sit on
  a single chest of the design (an input device, not counted): the judge rewrites its contents to
  the fewest cobblestones whose comparator strength is the value (`compute::strength_items`:
  floor(14 f) + 1 for a fill f > 0 of 27 slots of 64; the chest's state is unchanged, the contents
  change updates the comparators reading it), and the design reads it with its own comparator.
  The value outputs `s` / `d` are levels that must EQUAL the value (mod 2^N); carries and
  comparisons stay on/off lines. The state walks treat a value as N bits.
* **seven-segment 4** (study 15, [crate::compute](../crates/circuit/src/compute.rs) `Op::SevenSegment`,
  board: [designs/circuits/display/WR.md](../designs/circuits/display/WR.md)): inputs a0..a3 (a0
  the lowest), outputs a..g, the hex font 0-9 A b C d E F (`compute::SEGMENTS`), every value and
  transition walked like the other compute kinds. A DISPLAY a player can read (vjeux 2026-10-21):
  the seven outputs are lamps (an air output cell takes the judge's lamp, or the design's own
  redstone lamp) forming the standard 3-wide x 5-tall figure-8 (`compute::FACE`, column and row
  from the top: a (1,0), f (0,1), b (2,1), g (1,2), e (0,3), c (2,3), d (1,4)) on ONE vertical
  exterior face, read from outside (top row up, f on the viewer's left); no design block and no
  judge input device in front of that face (the face's 8 other cells may hold design blocks).
  Robustness: builds agree when their settled outputs and latency bound agree; the card's glitch
  count is the worst build's (2026-10-21, this kind only).
* **sorter ITEM** (study 13, [crate::sorter](../crates/circuit/src/sorter.rs); targets and the
  mechanics: [designs/circuits/tracks/sorter/WR.md](../designs/circuits/tracks/sorter/WR.md)): an
  item sorter slice: every item of ITEM (`circuit: sorter iron_ingot`) from a line into its chest,
  every other item on along the line. No input; judge chests (air cells, not counted) on
  `port items:` (the source, above the first line hopper), `port pass:` (where the line ends) and
  `port output:` (the slice's chest), and a `tile: dx dy dz` header (the step to the next slice:
  a sorter is a ROW of slices, one kind each; none: refused). Judged in the game by its own runner
  (like the shulker kinds) on what a player does with it (vjeux 2026-10-21: "REAL USE IS THE BAR";
  track sorter): the judge supplies a stream, counts and empties `pass` and the outputs every 20 gt,
  then waits for rest. The stream: ITEM mixed with 64-stack, 16-stack and non-stackable items (full
  and partial stacks, runs of ITEM back to back, two tools in a row; kinds the design itself holds
  are left out). Every run must deliver every ITEM to its own chest and every other item to `pass`,
  nothing lost or duplicated, the design's containers as they started (each filter recipe intact),
  no entity (a stream item left over included), rest. The runs (`circuit::sorter::Run`):
  * `base` (4 positions x 4 rotations, the metrics): one slice fed from an INPUT CHEST on `items`
    (its first hopper pulls one item every 8 gt).
  * `tiled` (required, 4 rotations): 3 copies chained along the line at the `tile:` step, each
    sorting its own kind (the judge rewrites the recipe's item id in copy k's containers; ports:
    copy 0's `items`, the last copy's `pass`, every copy's output), fed by a HOPPER LINE: the
    judge's line hopper where the slice before copy 0 would have its last line hopper (the
    design's hopper that pushes into copy 1's first hopper, one step back: the row's `feed` cell,
    next to copy 0's first hopper), pointing into it, fed from a chest above
    it. (A hopper pointing DOWN onto the first line hopper is no hopper line: the line hopper pulls
    from it while it pushes, two items every 8 gt, and the classic slice lets half its kind past.)
  * `stream` (required, 4 rotations): the same row fed by a WATER STREAM ending on that line
    hopper: each stack an item entity at rest above it, 2 waiting at a time; the hopper takes
    whole stacks and passes them on one item at a time.
  * `jam` (required, 4 rotations): the hopper-fed row with the middle copy's chest FULL from the
    start (a music disc in every slot) and 8 stacks of its kind coming. Once the line is quiet
    (200 gt with nothing delivered and nothing taken from the supply) the judge empties that chest,
    waits for the line to settle and supplies a tail. Required: nothing ever reaches a wrong chest,
    no other copy's kind reaches `pass`, every copy ends with its recipe, everything is delivered,
    and the tail's items of the jammed copy's kind reach its chest (it sorts again: `the slice does
    not sort again` otherwise). Badge `overflow-proof`: the line kept moving while the chest was
    full (every other item supplied had reached its place when the line went quiet: the full
    slice's surplus went on to `pass`), else `no (the line stops while copy 1's chest is full: ...
    held back)`.
  * `direct` (badge `stream-direct`, 4 rotations): the water stream ending right on the first
    slice (its item entities in the `items` cell, onto the design's own first hopper, which takes
    whole stacks). The classic slice fails it: its line hopper pushes one iron on for every one its
    filter takes (69 of 139 iron go past it): a player must end a stream on a line hopper before
    the first slice (the site says so for every sorter without the badge).
  * `intrusion` (required, 4 rotations; the real-use audit's ITEM INTRUSION, `crate::intrusion`):
    the hopper-fed row with stray items dropped at rest on every open hopper top of the row
    (`intrusion::intakes`: line hoppers included; the judge's own blocks close the tops they stand
    on) at a quarter, half and three quarters of the stream's time: 1 cobblestone, 64 iron ingots,
    1 iron shovel. Required: no stray in a wrong chest, every recipe intact, every stream item
    sorted, every stray delivered or still lying on a top that cannot take it (a full filter); a
    stray of a slice's own kind dropped on the line may pass by it (as from a stream ending there).
    A filter whose fillers are a plain item fails when that item comes by
    (calibration/cobble-filler.txt).
  * `row` (STORAGE ROWS only: a design with a `storage: row` header, the core of the storage
    system most players build; coordinator 2026-10-21): 8 slices (a 16-stack recipe: 4), each its
    own kind, fed from an INPUT CHEST on copy 0's `items`, everything else ending in the OVERFLOW
    CHEST (the last copy's `pass`): a full stack of every kind with junk between them, then a
    partial stack of every kind in reverse order. A storage row must also keep its line moving
    while a chest is full: the `jam` run's overflow-proof is required (calibration
    storage-jam-lock: the line-locking design, refused). Metric `storage`; archive axis `storage
    row`. The reference: designs/circuits/tracks/sorter/storage-classic.txt (sorter-classic with
    the header: the same blocks, so the archive keeps sorter-classic). BULK rows: a second output
    port `output.b` beside `output` makes the slice's store a DOUBLE chest (the judge places the
    `output` half `type=right`, the `.b` half `type=left`, facing so; 54 stacks; the `jam` run fills
    both halves): metric `storage` starts `bulk`, archive axis `bulk storage row`
    (designs/circuits/sorter/archive/storage-bulk.txt). redsim models double chests
    (`CompoundContainer`: hoppers fill the right half then the left, a comparator on either half
    reads both; tick-identical with the game: calibration/double-chest.txt).
  Metrics: items/s (the 10%..90% delivery rate; a hopper line carries 2.5), first item, hoppers
  awake at rest (lag), `slice width` (the step's length: 1 = slices 1 block wide), badges, bill.
  The stream's kinds are all in redsim's item registry and every run is tick-identical in redsim
  for sorter-classic (`--compare`; redsim interns item names, so papers renamed alike stack as in
  the game): `mctas circuit sorter-run <designs> [--run base|tiled|stream|jam|direct] [--sim]` runs
  one run at rotation 0 on redsim (milliseconds) or in the game and prints what reached each chest;
  `--sim --run all` is the whole judge on redsim, one line per design (`SORTER: ...` / `FAIL: ...`,
  the failing run named). Calibration (`door selftest` circuit:sorter-judge, redsim, and
  circuit:sorter-verify, the game; designs/circuits/tracks/sorter/calibration/): no `tile:` header
  (refused); a filter hopper pointing sideways (one slice sorts; side by side it pushes its named
  fillers onto the next filter's, which then takes junk: `tiled`); a filter missing one filler (junk
  takes the gap: `base`); the classic 18 + 4 recipe with a 2-step threshold (the full slice's
  signal 3 reaches its neighbour's lock, which drains its recipe, then sorts junk: `jam`); an empty
  drain hopper under a down-facing filter (drains the recipe); a recipe the threshold does not
  match (keeps items); plain cobblestone fillers (a stray cobblestone stacks onto a filler: the
  `intrusion` run). Kept without overflow-proof: jam-lock (a 2-wide slice whose chest comparator,
  in compare mode against a redstone block, locks its line hopper only while its chest is full:
  the line stops until the chest is emptied; a design that locks its line hopper whenever its chest
  holds anything mis-sorts: the backlog that builds in a locked line hopper is pushed on before
  its filter can take it, at some rotations).
* **smelter N** (study 17, [crate::smelter](../crates/circuit/src/smelter.rs); notes:
  [designs/circuits/tracks/smelter/NOTES.md](../designs/circuits/tracks/smelter/NOTES.md)): an
  auto-smelter of N plain furnaces (`circuit: smelter 4`; a slice players tile: `smelter 2` with a
  `tile:` header, judged as rows of 4, 8 and 16 furnaces: coordinator 2026-10-21, players build 8
  to 16 furnace arrays; a row may have fixed END PIECES: `tile slice: lo hi` names the cells along
  the tile's axis that repeat, the cells before them are the row's start piece (chests, clocks,
  ring turns) and those after its end piece, moved past the last copy). Ports: `port items:` (the input chest, above the
  first hopper of the input path), `port fuel:` (the fuel chest), `port output:` (the output chest;
  every port a judge chest on an air cell, except that the input and fuel chests may replace a
  COVER of the design: a full block on a line hopper, there to keep strays off the copies after the
  first). Judged in the game by its own runner on the sorter's benches: the judge fills the input
  chest with 16 items a furnace + 2 (raw gold, cobblestone, sand, oak logs, kelp, raw copper, two
  iron shovels: six smeltables, two of them fuels or non-stacking, none whose result is a stray's
  kind) and the fuel chest with coal (one per 8 items + 2 a furnace, at least a stack), empties the
  output chest every 20 gt, then waits for rest (every furnace out) and reads each furnace's
  `RecipesUsed`. Every run must: deliver every item's result (per source kind: results + strays
  passed raw + strays still lying on a hopper top = loaded + dropped; an input burnt as fuel or
  stuck fails), put no coal in the output nor a raw load item (a stray passed raw is allowed),
  leave nothing in the machine but coal (and the design brings no fuel of its own), burn no more
  coal than the items need plus one a furnace (`items / 8 + furnaces`, rounded down: each furnace
  burns its last coal out and a short wait between items costs a little; one lit again and again
  for a few items at a time burns far more; coal kept in a fuel slot is not burnt), share the load
  out (every furnace smelts at least two thirds of an even share: a row whose first or last
  furnace does most of the work fails), and end with every furnace out (a design's own clock may run on:
  metric `at rest` no); a jam is diagnosed (coal or a stray in a furnace's input, a furnace with
  items and no fuel). Robust: every build passes the whole judge, rests alike and smelts within
  20% of the base build's pace (a clock of its own starts at another phase of the load in each
  build). The
  runs (`circuit::smelter::Run`): `base` (4 positions x 4 rotations, the metrics); `tiled`,
  `tiled8`, `tiled16` (required with `tile:`, 4 rotations each: rows of 4, 8 and 16 furnaces, at
  least 2 copies, one input chest and one fuel chest on copy 0, one output chest after the last or
  in the end piece); `intrusion` (required, 4 rotations, on the row of 4 for a slice:
  1 cobblestone, 64 iron ingots and 1 iron shovel dropped on every open hopper top at a quarter,
  half and three quarters of the run; iron ingots do not smelt, so an open top over a furnace's
  input path jams it). Metrics: items/min per furnace (10% to 90% of the results), first item,
  items per coal, spread (items per furnace, min-max), hoppers awake, at rest (a design's own
  clock running on: no; checked twice, 7 gt apart, so a clock caught at the same state twice is
  not taken for rest), blocks per furnace (with end pieces: the repeated slice's blocks per
  furnace, and `end pieces` = the rest), and for a slice `tileable` and the rows' items/min per
  furnace. The furnace is in redsim
  (`AbstractFurnaceBlockEntity.serverTick`: cook and burn timers, the lit state, the hopper faces:
  top = input, sides = fuel, bottom = results then an empty bucket), tick-identical with the game
  on smelter-1's base run and the naive slice's tiled run (`smelter-run --compare`).
  `mctas circuit smelter-run <designs> [--run base|tiled|tiled8|tiled16|intrusion|chest|all] [--sim [--coal-log] [--trace]] [--compare] [--rotations N]`
  (`--coal-log`: the gt each coal reaches each furnace's fuel slot, and where the coal sits every
  2000 gt: how a row's coal is shared out; `--trace`: every hopper's and furnace's contents every
  20 gt and tick by tick through `TRACE_FINE`).
  **The super smelter** ([brief](../designs/briefs/super-smelter.md)): a design with `judge: chest`
  is judged by the `chest` run instead (`circuit verify`: N positions x 4 rotations; `smelter-run
  --run chest --rotations N` the same as a design aid): a double chest of raw iron (3456) in the
  input chest, the coal it needs plus a stack (496) in the fuel chest, never refilled; it must smelt
  everything (else `X% smelted`), only ingots out, nothing left but coal. Metrics: minutes for a
  double chest against the ceiling (3456 / 6 per furnace-minute, blast furnaces twice as fast),
  first ingot, spread, coal left, at rest, coal needed, bill. No even-share or coal-accounting
  rule. `port items.b:` / `port fuel.b:` beside `port items:` / `port fuel:` make the judge's
  chest a double chest (left and right halves joined).
  Calibration (`door selftest` circuit:smelter-judge, redsim, circuit:smelter-verify, the game;
  designs/circuits/tracks/smelter/calibration/): the naive slice (a line hopper over each furnace's
  feeder: the feeder under the first line hopper takes every item and every coal as it arrives, so
  the second furnace gets the one item that does not fit and no coal: `tiled` jams); a furnace fed
  through an open-topped hopper (strays: iron ingots in its input, `intrusion` jams); the fuel
  hopper pointing into the input hopper (coal never reaches the fuel slot); smelter-clock-2 with a
  plain line instead of its pair of hoppers passing the items back and forth (the clock rations
  each furnace, but between beats the items run on to the line's last hopper: the last furnace
  smelts 26 of 34, not shared out). Sharing a load needs the feeders rationed and the items kept
  where every feeder sees them: smelter-clock-2 powers both furnaces from an item clock (two
  hoppers passing two swords: 63 of every 65 gt), which locks every hopper touching them, so each
  furnace takes one item and one coal and gives one result per beat, from two hoppers that pass
  the load back and forth over them. A row that tiles (smelter-row, 2 furnaces a slice, 14 blocks a
  furnace + 210 in its end pieces): the items circle a ring of hoppers through every slice (the
  turns in the end pieces), one furnace under each line of the ring, and the coal a second ring;
  each furnace's top hopper is fed by an INTAKE hopper locked except for 8 gt every 12 s (a latch
  clock in the start piece, its dust line along the row strongly powering one block per slice
  between the two intakes' repeaters), so the top hopper always holds the next item (no idle burn)
  and each furnace takes one item per window wherever it stands; the side hoppers are locked by
  two slower clocks (one coal per furnace every 32 s), so the coal spreads round its ring before
  it is taken. Calibration: the same row with one clock for items and coal (fast-fuel-row): rows
  of 4 and 8 share, but in a row of 16 the first slices take the coal stream, one coal per window
  each, and the last furnaces get none. smelter-row-2 keeps the slice with end pieces of 138
  blocks: both rings turn at the row's ends (short rings stay full of coal for three windows) and
  a second latch clock gives coal windows every 830 gt starting once the coal has filled its ring
  (coal hoarded by furnaces served early, and windows a whole number of ring laps apart, are what
  starve a furnace: designs/circuits/tracks/smelter/NOTES.md). smelter-row-3 narrows the slice to
  12 blocks a furnace (furnaces 4 apart, coal hoppers on the inner faces sharing one lock).
  Intrusion: the judge's three chests stand in every run, so the open hopper tops are found with
  their cells closed too (a stray can't pass through a chest).
* **brewer N** (study 21, [crate::brewer](../crates/circuit/src/brewer.rs); notes:
  [designs/circuits/tracks/brewer/NOTES.md](../designs/circuits/tracks/brewer/NOTES.md);
  coordinator 2026-10-21): an auto-brewer of N brewing stands (`circuit: brewer 1`; a slice players
  tile: `brewer 1` with a `tile:` header and `tile slice:` end pieces as the smelter's, judged as rows
  of 4 and 8 stands). The recipe has two stages: water bottle + nether wart -> awkward potion,
  awkward + sugar -> potion of swiftness (sugar on a water bottle makes a mundane potion: the wrong
  stage). Ports: `port bottles:` (water bottles), `port fuel:` (blaze powder), `port wart:`,
  `port sugar:` and `port output:`, each a judge chest on an air cell (or on a cover, as the
  smelter's). Judged in the game by its own runner on the sorter's benches: the judge fills the
  chests for 4 batches a stand (2 on a row; 3 bottles, 1 wart and 1 sugar a batch; blaze powder:
  one a stand plus one per 20 brews plus one), empties the output chest every 20 gt, samples every
  stand's brew timer, then waits for rest. Every run must: put every bottle in the output chest as
  a potion of swiftness and nothing else but strays (no water, awkward or mundane potion: an
  unfinished batch or the wrong order), use up every wart and sugar, leave the supply chests empty
  but for blaze powder and nothing in the machine but blaze powder (the design brings no brewing
  item), burn no more blaze powder than `stands + brews / 20`, brew exactly two brews a batch, use
  every stand (two thirds of an even share), leave no item on the floor and end with every stand
  idle. Robust: every build passes and brews within 20% of the base build's pace. The runs
  (`circuit::brewer::Run`): `base` (4 positions x 4 rotations, the metrics); `tiled`, `tiled8`
  (required with `tile:`); `intrusion` (required, on the row of 4 for a slice: the smelter's
  strays dropped on every open hopper top). Metrics: potions/min per stand, first potion, blaze
  powder burnt for the brews, hoppers awake, at rest, blocks per stand. The brewing stand is in
  redsim (`BrewingStandBlockEntity.serverTick`: the 400-gt brew, blaze powder as 20 uses, the
  ingredient check every tick, the bottle bits of the block state, the hopper faces: top = the
  ingredient, sides = bottles and fuel, bottom = every slot, the ingredient slot only for an empty
  bottle; the comparator reads all five slots; potions are items `potion[KIND]`, one a stack),
  tick-identical with the game on the calibration stands and on brewer-small's base run, both
  stages (`brewer-run --compare`).
  `mctas circuit brewer-run <designs> [--run base|tiled|tiled8|intrusion|all] [--sim] [--compare] [--trace]`
  (`--trace`: every stand's slots, the hoppers above and under it and its brew timer every 20 gt).
  Calibration (`door selftest` circuit:brewer-judge, designs/circuits/tracks/brewer/calibration/):
  an unlocked hopper under the stand (every water bottle taken out before anything brews), the
  same hopper locked for good (nothing delivered: it stops short), brewer-1 without its slow
  release (a water bottle of the second batch reaches the output).
  A stand stacks its ingredient slot, so each stage needs ONE item put in at the right time, and a
  stage's end shows only when the hopper above can push the next ingredient. The one-stand
  records run on a latch clock (two hoppers passing 106 stones, 42 s a half): the bulb's
  comparator line locks the bottles hopper in one half, its wall torch locks the hopper under the
  stand in the other. The torch lags the comparator line by 2 gt, so a slow release (a repeater
  reading the clock's powered stone, under a dust cross beside the bottles hopper) keeps the
  bottles hopper locked until the hopper under the stand is. brewer-1 (49 blocks, 5x6x5, in the
  track) fires a wart dropper and, 8 gt later, a sugar dropper from a repeater line off the wall
  torch; brewer-small (42 blocks, 3x6x5) fires both from the clock's other torch, which lights for
  a few ticks under the block beside the bulb whenever the brewing half starts: both droppers
  touch that block, the wart rises straight into the hoppers over the stand and the sugar takes
  two more hoppers, so it arrives 16 gt later and waits in the top hopper until the wart is used
  up. A dropper is a conductor: powered through a block beside it, never by a repeater pointing
  into it (that would lock the hopper next to it), and never above the wall torch (its block
  touches the clock's dust: a 4-gt oscillation).
* **composter N** (study 22, [crate::composter](../crates/circuit/src/composter.rs); notes:
  [designs/circuits/tracks/composter/NOTES.md](../designs/circuits/tracks/composter/NOTES.md);
  coordinator 2026-10-21): an auto-composter of N composters fed by a hopper line (`circuit:
  composter 1` with `tile:` for a slice of one crop). A composter takes items only from a hopper or
  dropper above it and only compostable ones (`ComposterBlock.InputContainer`), so a hopper over a
  composter keeps every other item for good: the crops are sorted out of the line first, one crop
  a slice (the item sorter's filter). Ports: `port items:` (the input chest, which may stand on
  the first line hopper's cover), `port pass:` (the overflow chest at the end of the line) and
  `port output:` (the bone meal chest; a row: one a slice). The judge feeds a farm's mixed output
  (two rounds a composter of wheat seeds 16, cobblestone 10, wheat 10, kelp 10, iron ingots 5,
  sugar cane 10 and an iron shovel), empties the overflow and bone meal chests every 20 gt and
  waits until nothing has moved for 1200 gt. Every run must: compost every crop (none in the
  overflow or left in the machine), pass every other item to the overflow chest (strays: passed
  or lying where they fell), put all the bone meal in its chests and leave no composter full, no
  item on the floor. A slice (`tile:`) is judged as a row of 4 (base: the four crops in turn,
  copy k's filter holding crop k as the sorter's storage rows, `circuit::sorter::tiled`) and 8
  (`tiled8`: every crop twice); `intrusion` drops the sorter's strays on every open hopper top.
  Robust: every build passes and composts within 20% of the base build's pace (the bone meal is
  random: every build draws its own chances). Metrics: bone meal/h per composter, items
  composted/min, first bone meal, hoppers awake, at rest, blocks per composter. The composter is
  in redsim (the input container from above, the 26.3 compostable chances: wheat seeds and kelp
  30%, sugar cane 50%, wheat 65%, the first layer always, drawn as `nextInt(100)` from the level
  random mirrored per build; level 7 turns 8 after 20 gt; the bone meal out from below empties
  it), tick-identical with the game on the naive calibration and compost-row's base run.
  compost-row (18 blocks a composter, 1x6x5): storage-classic's slice with the composter where
  its chest was: the line hopper on top, the filter hopper under it (41 of the crop and four named
  papers: it takes only its crop), the hopper under that kept full (64 and four papers) and
  unlocked for a moment by the filter's comparator whenever one more crop arrives, so it drops one
  into the composter; a hopper under the composter pulls the bone meal into the chest; a block on
  each line hopper keeps strays out (with open tops, the strays crowd the line and a crop can pass
  its filter: rotation 1 sent 10 wheat to the overflow). Calibration: the naive build (a hopper
  over the composter fed straight from the input chest: it keeps the cobblestone, iron ingots and
  shovels for good).
* **randomizer N** (study 16, [crate::random](../crates/circuit/src/random.rs); coordinator
  2026-10-21): one press lights exactly one of N outputs, uniformly at random. The randomness is
  the game's level random (a dropper's or dispenser's random slot): every build's sandbox is
  seeded from its watch box and a salt, and redsim mirrors that random draw for draw, so the
  screen and the game see the same outcomes (`circuit trace-compare` is IDENTICAL on the
  calibration rand2-token). Ports: one input, pressed for real (a button of the design is pressed
  and holds for its own time, any other input is held on 30 then 20 gt, alternately), and N
  outputs, each the design's repeater or comparator with a free line out of its box (the TAP a
  builder takes the signal from: every cell from its front straight to the box's edge is empty).
  Stimulus: 24 presses per output (at least 48), each window the press, the
  recovery gap and a varying wait (0, 7, 3, 13, 1, 22, 5, 9 gt: several paces). Each window:
  exactly one output turns on, once, no other comes on; style `pulse` (all off again by the
  window's end) or `hold` (the chosen output stays on until a press changes it; exactly one on at
  every window's end), the same for every press; every output wins at least once. Metrics: style,
  latency (the slowest press to its output), pulse (the shortest, pulse style), presses.
  Uniformity: the counts of every build pooled (the judge's 16 builds, each its own seed; the
  screen's 8 salts) and tested against equal shares with a chi-square test at p = 0.001; the
  result rides in the verdict's flag (`uniform: chi2 X (limit Y) over P presses in R runs,
  counts ...`). Item parts are judged by ITEM INTRUSION like every kind (seal hopper tops).
* **crafter RESULT** (the auto-crafter study, [crate::crafter](../crates/circuit/src/crafter.rs); board and
  mechanics: [designs/circuits/crafter/WR.md](../designs/circuits/crafter/WR.md)): an auto-crafter
  (`circuit: crafter iron_block`; the recipes: `iron_block` (3x3: nine iron ingots),
  `crafting_table` and `stick` (2x2: four or two planks), `tnt` (multi-ingredient: five gunpowder
  and four sand, where each goes in the grid matters)). No input. Judge chests (air cells, not
  counted): `port items:` (the supply: every ingredient into one chest) or one
  `port items.<ingredient>:` per ingredient (`items.gunpowder`, `items.sand`), and `port output:`.
  Judged in the game by its own runner on the sorter's benches, on a PURE supply (coordinator
  2026-10-21: "only the recipe's ingredients arrive, as players usually feed a crafter from a sorted
  line"): full and partial stacks kept topped up (the design's intake runs at hopper speed); once
  nothing is delivered and nothing taken from the supply for 400 gt the judge checks every complete
  set was crafted and the partial set waits (a partly filled grid must not craft: one ingot makes
  nine nuggets, one plank a button), then supplies a tail that completes it, and checks again.
  PASS: exactly floor(supplied / per craft) crafts in the output (the scarcest ingredient's) and
  nothing else, every ingredient accounted for (crafted, waiting in the design, or left in its
  chest), the design's other contents unchanged, no item entity, rest; at N positions x 4
  rotations, then the intrusion run (strays dropped on every open hopper top: none may get in).
  Metrics: crafts/min, first craft, hoppers awake; the board ranks the box (volume) per recipe
  class. `mctas circuit crafter-run [--sim] [--trace]` runs one run (redsim or the game).
* **sequence-lock N L** (the lock study, [crate::seqlock](../crates/circuit/src/seqlock.rs); board:
  [lock/WR.md](../designs/circuits/lock/WR.md#button-sequence-locks-sequence-lock-n-l)): N buttons
  (`port input.a..`, pressed for real) and L code frames (`port input.k0..`, the rotation names the
  button of each step, hung behind the buttons' plane facing away, as for `lock N`). Sliding window:
  open iff the last L presses equal the code (a keypad; coordinator 2026-10-21); the next press
  closes it. A design that resets strictly instead (a wrong press goes back to 0 and does not count
  as a first press) passes too, badged `semantics: strict reset`. Every sequence of L presses for three codes
  (as built, every step the last button, a mixed one), as fast as the buttons allow and at mixed
  paces; the output changes at most once per press and settles before the next one.
* **harvester CROP** (study 18, farms, [crate::farm](../crates/circuit/src/farm.rs); board and
  mechanics: [designs/circuits/farm/WR.md](../designs/circuits/farm/WR.md)): an automatic crop
  harvester (`circuit: harvester sugar_cane`, or `bamboo`: no water needed, growths counted by height
  since bamboo has no age counter, and a stalk has stopped by itself at 16 or when its top reads
  `stage=1`; or `kelp`: the crop cell is one of the design's water sources (the player plants kelp in
  water) with water above it to grow into, growths counted by height (a harvested stalk's new top
  takes a random age), a stalk has stopped at age 25 or with no water source above; or `pumpkin` /
  `melon`: the judge plants the stem on the design's farmland, the fruit grows where the game puts
  it (a random side of the four, only on air with dirt or farmland below), a fruit is counted when
  the stem attaches to it and harvested when it detaches, and one standing more than 40 gt is a
  missed harvest; a melon breaks into 3 to 7 slices in one stack, so the run is recorded tick by
  tick and every crop item stack is counted when first seen: the delivered slices must equal what
  the drops held, and a drop taken in the tick it appeared (not seen) fails the run as not exactly
  counted; pumpkins are counted the same way, since a fruit broken in the very tick it grew never
  shows on its stem). Cactus (`cactus`): the judge plants it on the design's sand (sand falls: it needs a block
  under it), a growth is counted when the top block's age drops, like sugar cane; a cactus item that
  touches a cactus (its whole cell) dies after 5 ticks, so a drop falling back onto the plant is lost.
  No input; `port crop:`
  (one per plant: an air cell on the design's soil, where the judge plants the crop at age 0; not
  counted) and `port output:` (the judge's chest, an air cell, not counted). Judged in the game by
  its own runner: `random_tick_speed` 300 (100 times vanilla) for
  24000 gt (20 min of game time, about a hundred growths per plant), every crop column recorded
  after every tick; ITEM INTRUSION like every kind (crate::intrusion): at a quarter, half and three
  quarters of the run one cobblestone, 64 iron ingots, then an iron shovel are dropped on every open
  hopper intake and above every crop column (they may end anywhere); then growth stops, 800 gt more
  are recorded, and the run rests until nothing moves. A growth is counted when a standing block's
  age drops (the grower restarts at 0), so a block destroyed within the tick it grew still counts;
  harvested = grown minus what stands above the start. PASS: no plant lost (its bottom block stays),
  no crop still standing at full height at the end of the run (a missed harvest; a wait during the
  run is only a note, since it costs rate, which the efficiency counts), every harvested item collected
  (in the judge's chests, or still in the design's own hoppers and carts at the end: the backlog of
  a collection that the judge's 100x speed outruns; none loose, none despawned), nothing else
  loose, rest. The judge empties its chests every 500 gt during the run and counts what it takes,
  as a player empties a farm's chests (at 100x a chest fills 100 times faster; w86's melon/pumpkin
  brief, designs/briefs/farm-melon-pumpkin.md). After growth stops the collection drains (coordinator 2026-10-21): the run goes on
  until nothing changes, up to 24000 gt, so items backed up in hoppers, carts and the chest path at
  the judge's 100x random-tick speed (a rate no vanilla farm reaches) still arrive; an item on the
  ground or lost still fails.
  - **Items on a crop:** a crop item resting on a living crop (a bamboo stump) is not lost: the
    crop's next growth pushes it out of the new block. So before the end check, growth resumes, at
    the run's speed, while one lies there, for at most 1600 gt at rts 300 (scaled by 300 / rts;
    `REGROW`), shown as a `regrown:` line. An item still on a crop after that fails (w86's fuel-farm
    brief, 2026-10-22: the obvious bamboo line left one on a stump in 9 of 16 builds).
  - **Rest:** a harvester rests when its items have settled: no crop or intruder item moving inside
    the box, and the same farm items in every container over a 400 gt window. Its blocks may keep
    moving: a clocked or flying-machine farm shows `rests: no`.
  - **Leaving the build:** crop items seen below or beside the build's box are listed (`left:`) with
    their last positions, since an item that falls out of the world leaves no other trace; so are
    crop items about to despawn (age 5800 of 6000), with where they lie.
  - **Fuel:** a crop whose item is a furnace fuel (bamboo, 50 gt) also gets `items smelted per hour
    per block` and `items smelted per hour`: the efficiency and the rate x burn time / 200 (w86's
    fuel-farm brief, designs/briefs/fuel-farm.md).
  - **Game rts:** every passing harvester's card records `game rts N`, just before its bill: the
    random_tick_speed its robustness runs used (`farm::game_rts`: `rts_for`, no faster than its
    collection carries, from the vanilla twin run). The site's viewer plays a big farm at that
    speed (`data-farm-rts`), so it reads the number here instead of making its own twin run for
    each big farm at every deploy (coordinator 2026-10-23). Like the rates, it is kept out of the
    robustness comparison. `circuit archive --fill-game-rts` writes it into older verdicts from the
    twin run alone.
  The first metric is EFFICIENCY, `items per hour per block` (vjeux
  2026-10-21: "the efficiency of the farm is number of blocks placed compared to number of items
  gathered per hour"): the design's items per hour at vanilla speed (`items per hour`, all its
  plants) ÷ its blocks (soil, water and carts included; one slice of a tileable design). The
  catalog ranks each crop's harvesters by it, highest first. `circuit verify` measures the rates at
  vanilla speed (coordinator 2026-10-21): one run of the redsim farm twin at random_tick_speed 3 for
  720000 gt (10 game hours; `VANILLA_RUN`). The game builds judge robustness at `rts_for`: 300 for
  up to 8 plants, else 300 x 8 / plants (at least 30), so a full-size farm's items arrive about as
  fast as an 8-plant farm's, and ITEM INTRUSION drops on at most 8 intakes and 8 crop columns. The
  robustness runs also go no faster than the farm's own collection carries (`game_rts`): at most
  2.5 items/s per output over the farm's vanilla rate from its twin run, with the run up to 4 x
  longer when that slows it. A design the twin does not model, or whose twin run fails, keeps the
  game builds' mean rate (noted on its card), with the speed-up loss added back: a random tick that lands while a moving piston or a piston
  head stands in a plant's next cell (above its top) is lost, and at rts 300 a harvester's head
  stands there after every growth step for a few gt, a loss 100 times smaller at vanilla speed. The
  judge counts those ticks and adds back their growths (rts / 4096 random ticks a tick, times 1/16
  for cane and cactus, 1/3 bamboo, 0.14 kelp; stems are not blocked), shown as a `speed-up:` line
  next to the measured rate (cane-mc-8b: 87% of free growth measured, 98% counted). A yield line (informational) compares
  the harvests with the plants' free growth: the count with nothing in their way, from the crop's
  own odds per random tick (rts / 4096 per gt for a block). Sugar cane and cactus grow once every
  16 random ticks of the top block (a cane ages only with air above it), bamboo on 1 in 3, kelp on
  0.14. A stem alone on dry farmland rolls 1 in 13, needs 7 rolls to grow up, then grows a fruit on
  1 roll in 4 for each free side. Observer-and-piston cane designs reach about 85-89% at rts 300:
  the piston head in the cane's second cell after each age step wastes the random ticks that land
  then (negligible at the game's own speed).
  Metrics: items per hour per block, items per hour, blocks per cane, items per hour per cane at vanilla speed (the delivered
  items scaled by 3 / 300), harvest height (2: as soon as it grows; 3: when full), wait (the longest
  a grown block stood before its harvest); the builds agree on blocks per cane (rate, wait and
  height follow the random growth: the card shows the builds' mean rate and worst wait and height).
  A `tile:` design is also judged as its copies side by side (`crate::tiled`, every copy's crop and
  output, 4 rotations). Escaped entities (leaving the build's box) are noted. `mctas circuit
  farm-run` is one game run (see Commands).
  THE REDSIM TWIN (`crate::farm::sim_run`, clk-misc 2026-10-21): the same run step for step on
  redsim, the game's random ticks mirrored draw for draw (`redsim::crops`: the bridge seeds
  `Level.randValue` from the sandbox seed and queues its chunk tickets so the chunk tick order is
  the box's alone; the intruders are summoned by the bridge, whose seeded entity randoms make an
  item pushed into a block repeat), so a build's twin run IS the game run of that build and seed:
  `mctas circuit farm-run --compare` prints both records (cane-mc-8b, cane2-mc-13b, cane-8b,
  cane-7b, cane-mc-9b, bamboo-8b, bamboo-down-9b, tiled and moved builds: identical). `circuit sim` judges a harvester with
  it: the whole judge, every build of `circuit verify` (4 positions x 4 rotations, each turned
  without the game: `Circuit::turned_pure` turns entities like the game) and the tiled run, the same
  verdict and card (about 6 s), so `circuit compact` and the other searches judge exactly what
  `circuit verify` judges; `farm-run --compare-verify` runs both build by build (cane-mc-8b,
  cane2-mc-13b, cane4-mc-21b, cane4-mc-23b, cane-mc-9b, cane-7b, cane-8b, bamboo-hc-12b, bamboo-8b,
  kelp-17b, kelp-v1, kelp-down-17b, melon-mc-7b, pumpkin-mc-7b, melon-6b, pumpkin-6b,
  cal-melon-lost, cactus-wool-9b, cactus-cap-7b, cactus-sculk-6b, cactus-walls-9b,
  cactus-wool-8b, cactus-wool-9b-t3: the same verdict, notes and metrics in every build; found on the
  way: entities in a box are visited section by section in the game's order, and an item's water
  splash draws from its own random; the stem crops' melon slices draw from a loot random sequence
  the bridge reseeds per sandbox; a cactus farm's sculk sensor hears entities at their exact
  distance). `farm-run --sim` is one twin run.
* **item-elevator N** (`crate::lift`, clk-misc 2026-10-21): move items up N blocks. The design
  declares `port items:` (the input chest: an air cell the design empties, usually with a hopper
  under it) and `port output:` (the output chest, an air cell exactly N blocks higher); both chests
  are the judge's (not counted). After the design settles, the input chest is filled with a mixed
  load of 193 items (64-stackables, 16-stackables: snowballs, ender pearls; non-stackables: 2 iron
  shovels, a diamond sword; topped up as it empties) and the output chest is emptied every gt.
  PASS: the whole load arrives, kind by kind (nothing lost, nothing left in the input chest or the
  design's containers, nothing loose); the flow between 10% and 90% of the load keeps up with a
  hopper (at least 98% of 2.5 items/s); rest, or a clock of its own (`at rest no`). Runs: the base
  run at 4 positions x 4 rotations (the builds agree when every one passes, rests alike and flows
  within 2%), and ITEM INTRUSION (4 rotations): a cobblestone, 64 iron ingots and an iron shovel on
  every open hopper top at a quarter, half and three quarters of the run; the load must still arrive
  whole and every stray reach the output chest or stay where it fell. `placement: player` and
  `port start` kicks work as for clocks. Metrics: items/s, first item (gt from the fill to the first
  delivered item: the trip up), at rest, blocks; the archive's axis is N. The redsim twin
  (`lift::sim_verify`, `circuit sim`) runs the same builds, droppers shooting items into the world
  and bubble columns included (`lift-run --compare-verify` runs the game judge and the twin build
  by build: bubble5-b, bubble5-rest, lift5-chain-14b SAME); `mctas circuit lift-run` is one run,
  `--sim --turns T --offset X,Y,Z --watch ITEM` one twin build with the watched items every gt.
  Designs: `designs/circuits/elevator-items/` (dropper columns: dropper-pair and dropper-bend, 2.5
  blocks a level; bubble columns: bubble-column, 5 blocks a level but a bill that does not grow, and
  bubble-rest, whose clock runs only while its dropper holds an item).
* **station 1 | station 2** (study 19, minecart stations, [crate::station](../crates/circuit/src/station.rs);
  board and mechanics: [designs/circuits/rail/WR.md](../designs/circuits/rail/WR.md); clk-obs,
  coordinator 2026-10-21): a station on a rail line, `station 1` on a one-way line, `station 2` on a
  two-way line. Ports: `port line.in:` and `port line.out:` (the first cell of the judge's own track
  at each end: air beside one of the design's rails, level or one up or down; the judge lays 8
  cells of straight plain rail outward from each, on stone where the design has nothing below; the
  design must stand without them), `port input.go: x y z button` (the departure button). Judged in
  the game by its own runner with the game's own minecarts: a rider is an armor stand riding the
  cart (`/ride`), so the cart runs the ridden physics (`OldMinecartBehavior`: its move scaled by
  0.75, slowed by 0.997 a tick instead of 0.96). The REDSIM TWIN (clk-misc 2026-10-21:
  `station::sim_verify`, `circuit sim`, `station-run --sim | --builds | --compare-verify`) runs the
  same run on redsim ([`crate::rig`]: one runner over the game's sandbox or a redsim world), riders
  included; the judge's game runs are tick-identical in redsim (`station-run --trace DIR` records one
  as an exact trace for `redsim-fuzz trace-compare`; station-slope-v2 and station-slope2-v4 are in
  the trace suite). One run sends 17 carts in, ridden and empty, with 0.02 to 2.0 blocks per tick of
  momentum toward the station (2.0: a cart off a booster line keeps that much), summoned at the
  station's edge; some cycles press the button while the station is empty first (it must change
  nothing), some press while the cart is still braking (it may be sent straight on or stop and wait
  for the next press, but must leave in its direction). PASS, for every cart: it comes to rest
  inside the station within 300 gt; a ridden one where the rider can click the button from the cart
  (`station::clickable`: a look ray from a player's eyes, 1.2075 above the cart, reaches the
  button's outline within 4.5 blocks past every block's outline; the player's own cart does not
  take the click, `ProjectileUtil.getEntityHitResult`); it stays put for a seeded random 5 to 154
  gt; the press moves it within 80 gt and it leaves through `line.out` and rolls on by itself 6
  blocks along the judge's track within 300 gt; the rider is still in the cart. `station 2` also
  sends 8 carts the other way (in through `line.out`, fast enough to roll through that much plain
  track): each must pass through without coming to rest and leave through `line.in`. Item parts
  get ITEM INTRUSION like every kind (drops on open hopper tops before three of the cycles).
  Metrics: stop distance (the farthest a cart rests past the station's entry), departure delay (the
  longest from a press to the cart's first move), leaves at (the slowest leaving cart where it
  leaves the station), other way passes at. `mctas circuit station-run` is one game run;
  `STATION_TRACE=1` prints the cart and the design's rails every tick.
* **cart-unloader | cart-loader** (study 19, hopper-minecart storage transport,
  [crate::cartio](../crates/circuit/src/cartio.rs); board: [designs/circuits/rail/WR.md](../designs/circuits/rail/WR.md);
  clk-obs, coordinator 2026-10-21). Ports: `port line.in:` (required) and `port line.out:`
  (optional: a through line, the cart leaves out through it; without it a terminus, the cart leaves
  back through `line.in`), the judge's chest (`port output:` for an unloader: every item must end in
  it; `port items:` for a loader: the supply), an air cell, not counted; no input. Judged in the
  game with real hopper minecarts, 12 carts a run (a loader 13) at 0.02 to 2.0 b/t: mixed loads,
  single items, full stacks, 16-stackables, non-stackables, empty carts (unloader) or an empty supply
  (loader), a stack in the last slot only; a loader's last cart comes after its supply waited
  3000 gt. UNLOADER PASS: every cart with items comes to rest, stays while it holds any item, and
  leaves its way within 200 gt of its last item, rolling 6 blocks; an empty cart may leave without
  stopping; after each cart, once the design's containers hold what they held at the start, the
  chest holds exactly that cart's load. LOADER PASS: every cart comes to rest (or goes on without
  stopping only if, as it leaves, it is full or the supply is dry, and it holds something), stays
  while it can still take items (not full: every slot at its item's max stack; and the supply, the
  judge's chest plus what the design holds beyond its start, not empty), and leaves within 200 gt of
  becoming full or dry; an empty cart at a dry supply WAITS (coordinator 2026-10-22: an idle farm's
  cart must not shuttle empty): it may not move for 600 gt, then the judge adds 10 glass that it
  must take before it leaves (the waiting time is not loading time in the rate);
  the supplies use at most 5 kinds a cart; at the end every supplied item rode away in a cart.
  Loads never use the real-use audit's stray items; its strays fall on every open hopper top (one the
  judge's chest covers is not open) before 3 carts and may end in the chest or a cart: they are
  cargo, so that cart may leave later by their own transfer time (8 gt an item), its delay left out
  of the metric. A cart the judge removes is emptied first (a killed container minecart drops its
  contents). Metrics: line (through / terminus), unload or load rate (items per second while carts
  stood in the station; the worst build's), departure delay (the longest of any build). Robust
  (coordinator 2026-10-22): every build passes with every item delivered; the rate is not matched
  build to build (a slow test cart resting 0.1 block apart moves it by a tick).
  `mctas circuit cart-run` is one game run; its redsim twin
  (`cartio::sim_verify`, `circuit sim`, `cart-run --sim | --builds | --compare-verify`; `--trace DIR`
  records the game run for trace-compare: cart-unloader-t3 and cart-loader-t5 in the trace suite) runs
  the same carts, hopper-minecart transfers, detector rails and strays.
* **auto-door** (study 21, automatic doors, [crate::autodoor](../crates/circuit/src/autodoor.rs);
  board and mechanics: [designs/circuits/autodoor/WR.md](../designs/circuits/autodoor/WR.md);
  clk-obs, coordinator 2026-10-21; real-use walks [designs/briefs/auto-door.md](../designs/briefs/auto-door.md),
  2026-10-22): a 2x2 piston door that opens when someone comes near from either side and closes
  after they have passed, never on someone walking through. Ports
  `door.a` and `door.b`: opposite corners of the 2x2 opening, in one vertical plane; no input port.
  At rest the opening holds 4 identical non-walkable blocks, and the walkway (3 cells each way
  from the opening, the opening's two columns, two high) is walkable (air, dust, plates, rails,
  carpet...). The judge lays its floor under the walkway (7 cells each way) where the design has no
  block. Walkers are armor stands (0.5 wide, 1.975 tall like a player, not markers: they press
  stone and wooden plates and tripwire), teleported every gt: 8 walks, both ways, through the
  left, middle and right of the opening, walking (0.216 b/gt) and sprinting (0.281 b/gt), two in
  a row (2 blocks apart); then the
  two-walker sweeps (the real-use bar, coordinator 2026-10-21): two one behind the other at every
  gap from touching (0.5) to 6 blocks in 0.25 steps, walking and sprinting, both ways (92 walks),
  and two from opposite sides in the two columns crossing in the doorway at every start offset
  from -6 to 6 blocks in 0.5 steps, walking and sprinting (50 walks); then the lateral sweeps
  (coordinator 2026-10-22: a door that opens by halves shuts a half in the path of someone a little
  off centre): one walker at every offset across the doorway in 1/32 steps, from touching its left
  edge to touching its right edge, with the stand's 0.5-wide box (49 offsets) and a player's 0.6-wide
  one (46 offsets: four stands 0.05 off its middle each way, whose boxes' union is a player's
  footprint; the bridge allows no mob), walking and sprinting, both ways (380 walks). A walker waits
  where its next step would enter a block, up to 100 gt (the door did not open). It must never be
  moved by anything but the judge, never stand in a door cell that holds a block (the door closed
  on it), and the door must be closed and flush within 300 gt after the last walker is 6 blocks
  past it, then stay closed 100 gt. Last, two REPORTED walks, not required: a walker that stops
  in the middle of the doorway for 400 gt (walking, and after a sprint the other way); if the door
  closes on it, the run goes on (the door must still come to rest closed) and the table says
  where the walker ends, left alone 40 gt. Metrics: open wait (the longest a walker waited at the
  door), close delay (the longest from the last walker leaving the opening to the door closed),
  stop in doorway (`the door stays open` or `the door closes on them`, the worst build's). `mctas
  circuit door-run` is one game run; `AUTODOOR_TRACE=1` prints every design cell that changes,
  tick by tick.
* **mob-farm iron_ingot TIER** (the mob farms, [crate::studies::mobfarm](../crates/circuit/src/studies/mobfarm.rs);
  brief [farm-iron.md](../designs/briefs/farm-iron.md), track notes
  [ironfarm](../designs/circuits/mobfarm/tracks/ironfarm/NOTES.md)): an iron farm (villagers in beds,
  a zombie that scares them, golems that spawn and die) judged on redsim's mob layer
  (`redsim::entity::mob`: the villager brain, sleep and `LAST_SLEPT`, golem spawning, LivingEntity
  physics in water and lava, damage, loot; `redsim::weather`: rain, thunder, lightning, sunburn)
  and confirmed in the real game: `circuit verify` (so `circuit archive`) runs every build in the
  26.3 server after the redsim judge passes (`crate::mobfarm_game`, `mctas mobfarm gate`). One
  `port output:` (the farm's single chest in the design, or an air cell where the judge puts one;
  every archived farm holds its chest, so its download and its 3D view show it). The judge refuses what the mob layer does not model (static checks:
  job sites, a zombie that does not ride, villagers that can walk,
  gossip, climbable blocks...), floods every position a golem can stand on from every spawn spot
  (with its edge spots: box sides 0.0005 inside or outside block boundaries, where a current can
  stop while the floor still holds the golem) and drifts it still and under 8 walking inputs (each
  golem must die within 1200 gt), then runs
  each of 8 builds (4 rotations, 2 positions) for 8 seeds x 5 days from the steady state (day time
  drawn in 10..12000, `LAST_SLEPT` at the end of the last rest), the 9 worst-policy runs on one
  seed, and the restart probe for a design that claims `restart: yes`. Every drop must reach the
  chest (or stay in the farm's containers); a burnt or despawned drop fails. The ceiling is one
  golem per 600 gt per villager group (`modules`). `mctas mobfarm` holds the generators
  (`gen l1|l2|l2d|l3|l4|l5 [--tier day-1|middle|fastest]`), the exit flood, single runs, `probe`
  and `validate` (the design run in the real server against redsim: golems/h, the day/night split,
  the spawn-cell chi-square). The tier is the kind's last word (vjeux via the decisions inbox
  2026-11-08, like a crop farm's one kind per crop): each tier is its own kind and site page (its
  pick ranked by the tier's rule: the least iron to build, the most iron an hour per block, the
  most per group of villagers); the fastest tier's pick links its scaled build (several copies)
  both ways; a farm in no tier (the brief's obvious design) stays in the catalog only. The site's
  pages of a mob farm say "Verified in the real game" like every study's; a farm without the
  real-game gate's `real game` line on its card is catalog only. Its card measures its
  tradeoffs (vjeux 2026-11-08: "It's all about tradeoffs"): `lightning` (exact sums over every
  strike column: iron burned per thunderstorm, how often villagers turn into witches and how many
  one strike can take (every villager within 3 blocks of it: the farm's spacing), a broken boat, a
  fire, a trap horse, the game days until it breaks; a sky the model cannot decide is an error,
  never "no sky"), `thunder loss` (iron per hour of thunder, breaks
  included), `rain` (a clear, a rain and a thunder day's rates), `sunburn` and `fire` (lava's: the
  farm's own lava only; when a run's fire burns one of the farm's blocks the run goes on, the lava
  flows into it with all it then does to the farm, and the line says what burns and after how
  long on average; a terrain sample's natural fires are the world's, simulated, not the farm's).
  The 3D view runs the
  farm with that simulation (its judge's first run, tick for tick: [webrender.md](webrender.md)).

  **The real-game gate** (mobinfra 2026-11-08; vjeux: "Why all of these caveats? Just do the real
  work!?"). Each of the judge's 8 builds runs in its own unmodified 26.3 server: a void superflat
  in plains (it rains there), Hard, `spawn_mobs` on (trap horses, reinforcements) and natural
  monsters off (the terrain lane judges spawning), the day clock and the weather running. The
  official server and not the bridge: lightning needs a player (`tickThunder` runs only in the
  chunks within 128 blocks of one, `ChunkMap.collectSpawningChunks`), so does lava's and fire's
  spread (`canSpreadFireAround`, 128 blocks), and the day clock and the weather are the server's,
  which the bridge's sandboxes replace or share. The bot is the farm's AFK player: creative, 60
  blocks under the farm's chest with a stone over its head (within 128 of every chunk of the farm,
  out of every mob's sight, never a bolt's target, never within 10 of a trap horse on the farm).
  The weather first runs alone for a random 150k..400k gt; the farm is the redsim judge's own
  build after its water settled, pasted with `setblock ... strict`, then the day time of the
  judge's steady state, the daylight detectors, the villagers (`LAST_SLEPT` at the end of the last
  rest) and the zombie in its vehicle (the vehicle summoned alone, the zombie at the seat redsim
  gives it, then `ride`, all before the first tick: summoned in one command, a passenger starts at
  the vehicle's own position and spends its first tick there before `positionRider` seats it,
  where a low roof costs it a hit no farm ever deals). It runs 3 game days of natural weather, then `weather rain
  24000`, then `weather thunder 24000`, stepped at `tick rate 10000` (about 15 s per build-day;
  the 8 builds in parallel take about 2 minutes). Every 20 gt it reads every golem, villager,
  witch, zombie villager, zombie, vehicle, trap horse and item, and every 1000 gt the farm's
  flammable blocks and lava. The gate checks the card, not perfection (vjeux 2026-11-08: "You
  can't expect day 1 farm to pass everything. It's all about tradeoffs"). Lightning's breaks (a
  strike that made witches — one bolt converts every villager in its box, so a sample's new
  witches are one strike, the card's unit — a lost vehicle, a trap horse on the farm) are counted,
  and their numbers over the
  thunder the 8 builds saw must fall within the 99% Poisson band of the card's `lightning` rates
  (`lightning::measure_farm` over redsim's runs of the same schedule; rates the model cannot
  measure, such as a sky it cannot decide, fail the gate); what follows a break is its
  consequence and goes unchecked. Until a build breaks it must hold the rest of its card: golems
  spawn; no golem leaves the farm's box, vanishes outside it or lives over 1200 gt; the villagers
  stay as many and unhurt; the zombie stays unhurt in its vehicle; none of the farm's drops (its
  iron and poppies) is ever outside the box (a trap horse a bolt left in the void falls and drops
  its bones far below: not the farm's); no flammable block burns and no lava moves; and at the end the iron in the farm's
  containers and on its floor is at least 4 ingots per dead golem minus 3 standard deviations of
  the loot (3..5 uniform: a leak shows, a few unlucky draws do not). The rates: each window's
  golems per hour, the mean of the builds unbroken by then, must fall within redsim's interval for 8 builds of that
  window under the same schedule (the judge's `farm::run` from the stationary weather with the same
  forced windows, 64 runs; a reference run that fails fails the gate): Bonferroni over the k
  windows checked, the natural days at a family-wise 95% and each forced day at a family-wise 99%
  (z of 0.05/k and 0.01/k two-sided: 2.394 and 2.935 for the three windows; about 2.3% of good
  farms refused by chance, where 95/99/99 per window refused about 7%: ironfarm 2026-11-08). The
  design's block entities (its `nbt:` lines) are merged after the paste and read back (the gate
  fails unless the game holds every field the design gives), each comparator gets redsim's
  settled output, and the iron the design placed in its containers (a sorter's lock and filter
  stacks) is not counted as the farm's.

  **In a terrain sample** (`--terrain village-edge`, mobinfra 2026-11-08: D1U must pass the real
  game in its own world): the farm at its Site (the terrain judge's spot, offset and C7 glass
  fills) in a copy of the sample's kept world, the one the committed sample was cut from
  (`$MCTAS_HOME/terrain/worlds/<world>` or `--world`; a re-generated world differs in the day-1
  details), `--parallel` runs (8) of that one placement instead of the 8 builds. Each server
  starts with the world's saved weather (`data/minecraft/weather.dat`) written from redsim's
  stationary weather draw, so no warm-up ticks age the village; the farm's changed cells are
  pasted (`setblock ... strict`: its blocks, air where it digs, the fills); the AFK bot stands at
  the design's `afk:` (creative counts for spawning); Hard, natural monsters on, phantoms,
  patrols and traders off (as the terrain judge); simulation distance 10 (every chunk within 128
  of the bot ticks, as redsim's natural spawning); only the farm's box forced (forced chunks spawn
  too). The samples read only the farm's own entities (summoned with the tag `mf`; a converted
  villager keeps it, so its witches and zombie villagers are the farm's), items within 8 blocks of
  the box, and the golems first seen inside the farm (the village's golem is not the farm's); a
  trap horse is a `SkeletonTrap` one on the farm (the horsemen of a trap triggered elsewhere ride
  to the AFK player and are not counted); a run whose AFK bot is no longer on the server fails as
  a harness failure (the chunks around the farm would unload). A golem that leaves the farm fails
  the run with where it was first seen and when; one first seen outside the farm but within a farm
  villager's golem search (8 across, 6 up or down) fails it too, naming the cell and the tick: the
  farm spawned it at a spot it leaves open (D1U, 2026-11-08: such a golem lingering near the
  villagers stalled every spawn for ~9000 gt). Each build line and failure carries the run's
  seed. In a terrain sample a drop outside the farm is the farm's only if it was first seen inside
  the farm's box (a leak); one never inside is the world's (a village golem killed over the farm).
  The paste takes the farm's box and
  3 around it plus the Site's seals and caps. `circuit verify` and `circuit archive` run a design
  made for terrain (`terrain: cells`, `sample:`) through this mode, and say where the kept world is
  missing. The rates are redsim's terrain runs at the same Site. A
  terrain build-day takes about 160 s (the village and every chunk within 128 tick); D1U's first
  run, 1 day: 39.0 golems/h vs redsim 29.4 (95% for one run 4.1-54.6), the zombie unhurt, 4.00
  iron per dead golem. Why this size (the void gate): one
  build-day costs about 15 s of a server (the parent's budget was 45 s); 8 builds keep every
  rotation and both chunk alignments (strikes and spawning are per chunk), 3 natural days give
  each build about 3 rest cycles and the rate's standard error over 8 builds about 5% of a
  middle farm's rate, and a forced day of each covers a rainy dusk and a thunderstorm's bolts
  (about 50 a thunder day over the bot's 128 blocks). Its first catches (2026-11-08): lava's
  random-tick fire burns the L2d chamber's oak fence gates by gt 2000 (iron-middle,
  iron-fast-module, iron-scaled-2x2: 0.04-0.07 iron per golem reach the chest), bolts strike
  villagers (a witch) and break a boat, and the dusk-pulse farms stop in rain (0-36 golems/h on a
  rain day, 0 in thunder).

### Real use (the audit of 2026-10-21)

How a player uses each kind, and the judge exercising it (coordinator 2026-10-21, after vjeux
found gaps by hand: a lock's code in view, a display's buried lamps, an observer called a BUD,
shulker chests full, endless or trickling, TNT button presses, timers needing long inputs).
`mctas circuit real-use [--study S]` judges every archived design on redsim without and with the
audit's parts (`check::set_real_use`) and lists what changes.

* **Real presses** (`check::PRESSES`: a stone button 20 gt, a wooden one 30 gt, a lever held 200
  gt): a T flip-flop's B pulses start with 200, 30 and 20 gt (each must toggle once); a counter
  gets N more inputs after its 4 gt pulses, stone and wooden presses with a lever held down every
  8th (`check::COUNTER_PRESSES`: the count must go on, else `counts 4 gt pulses but not real
  presses`); an RS latch's s and r, a T flip-flop with reset's t and r, 30 and 200 gt each; a
  delay line a 200 gt hold that must come out shifted (`a lever held 200 gt does not come out
  shifted`: a limiter with a long L passes every pulse up to 30 gt). Calibrations: a gated clock
  in front of a copper bulb (a T flip-flop and a counter a held input keeps toggling), a 38 gt
  pulse limiter claiming `delay`.
* **A lever flipped back too soon** (`check::LEVER_FLIPS`, 4..20 gt; shared with the flying
  machines' start check): a piston extender's lever on for k gt then off, then off for k gt in a
  held extension and on again, longest first, then a full cycle; the metric `flips` reads `safe`
  or `breaks: off k gt after on strands blocks` (a metric, not a failure: of the 64 archived
  extenders only the single sticky piston is safe; most strand a piston and the payload when
  turned off within 20 gt). From 4 gt: a 2 gt flip makes any sticky piston spit its block.
* **Outputs a player can wire** (`check::buried_output`): from every output cell inside the box,
  a path of cells nothing occupies during the run (no design block, no block a piston moves
  through, no judge device; the BUD judge's footprint probes do not count) must lead out of the
  box, else `output ... is buried`. No archived design is.
* **ITEM INTRUSION** ([crate::intrusion](../crates/circuit/src/intrusion.rs), vjeux 15:24Z on the
  precise drop's exposed ring: "very likely an item gets dropped, messed up the timing and the
  whole machine explodes"): a hopper takes every item that comes to rest in the cell above it, so
  every hopper whose top is open (no full block or container over it) and that a stray item can
  reach from outside the design (cells without a design block or an input device) is an intake.
  The judge runs the design again with 1 cobblestone, 64 iron ingots and 1 iron shovel dropped
  at rest (`Action::Drop`: an item entity at the cell's center, the game's `summon`, redsim's
  `entity::spawn`) on every intake at a quarter, half and three quarters of the run (each when
  the base run has that cell empty); it must pass with the same metrics (and keep a precise
  flag), else `item intrusion: <where, when>: <why>`. On redsim a run whose dropper then chooses
  between two kinds of items stops as not modeled, a failure too (the item is in its timing);
  the game judge (the first build) decides. Sealed hoppers and item-free timing have no intake.
  Calibration: the 52-block precise drop as archived (its release ring open on top).
* **Chunk reloads, tiling, the ground**: a reload at rest saves block states and scheduled ticks
  (nothing a circuit at rest holds is lost; the clock judge's reload check covers running
  machines); `tile:` is a badge where the kind has one; standing on the ground is the TNT
  judge's safe drop and the flying machines' clearance (track fly).
* **TNT, what breaks first**: a blown-up verdict names the first machine block an explosion
  destroyed and, on redsim, the TNT behind it (its spawn frame and kick); the safe drop's report
  says why one block less fails, and the culprit's path (its position at each of its first frames,
  then every 8). `mctas circuit tnt robust <design> --spawn-run R --drop N [--ground obsidian]`
  replays one per-spawn run. 2026-10-21, the fast engine (tnt-engine-lever-17b, safe drop 6): the
  culprit is no dud of the engine but a launched TNT that lands about 20 blocks out and is knocked
  back by the explosions in its landing zone (run 58: TNT 7 lands at z = -21, comes back along the
  ground and explodes under the machine, breaking the wall lever; run 51: TNT 6 comes back in the
  air over it). A floor in the box takes the ground blasts: `mctas circuit tnt floor <design>...
  [--block B] --out DIR` adds one layer under the footprint (obsidian where a moving slime would
  grab it: `circuit material`); tnt-engine-lever-29b (the inverter under the engine, 12 stone
  under it) needs 4 blocks: effective volume 108, vs 150 and 160.
  The metric `effective volume` (the site ranks TNT dupers by it) is the footprint times the box
  height plus the height it stands at (`max(stand, safe drop)`), the footprint's sides short side
  first (a rotated build swaps them; a metric that changes with the rotation would make every
  non-square duper rotation-dependent).
* **Item-timed TNT releases, redesigned**: the precise drop and the precise air burst with a lever
  now use ONE clock and ONE item-free delay (a repeater line from an observer watching a part
  the engine moves or fires once per TNT to the release), each TNT released by its own push:
  tnt-precise-drop-lever-54b and tnt-airburst-lever-66b replaced the 52b and 56b (hopper rings
  open on top; the 66b keeps the 56b's dropper, empty, as the delay's start, in the 56b's box).
* **The start device**: a `port start` kick must be one a player can give (`check::buried_kick`):
  an air path leads from the kick's cell out of the box at rest, else `the start kick ... is
  buried` (none of the 19 kicked archived designs is; unit test `kick_tests`). A flying machine
  started with its own lever (`start flip the lever`, track fly) is its own class on the archive
  axis (`, lever start`), so the picks choose between it and a kick-started one.

## Adding a study (the registry)

A new study edits no shared file (coordinator 2026-10-21: four branches had conflicted in the same
core files). It is one module, `crates/circuit/src/studies/<module>.rs`, which
`crates/circuit/build.rs` finds and declares as `crate::<module>`; the module registers the study
and its kinds in `pub static STUDY: registry::StudyDef`
([crates/circuit/src/registry.rs](../crates/circuit/src/registry.rs)):

* the study: its catalog directory (`designs/circuits/<id>/`), its title in the circuit list, the
  site's words (its page's title and intro, what its builds are for, its line on the index);
* each kind (`KindDef`): its `circuit:` word and how the rest of the line parses, its input ports
  and port checks, its game judge (`verify`), its redsim twin (`Twin::Exact(sim_verify)` or
  `Twin::GameOnly(why)`), how its builds agree (`Agree`: every metric, a rate within a tolerance,
  or every metric but the random ones), the circuit list's axis, `mctas circuit run`'s runs and the
  site's words for the kind (title, summary, metric definitions).

Everything shared reads the registration: `Kind::parse`, `Kind::name`, `Kind::study` (a
`Kind::Study`), `Circuit::check_ports`, `circuit verify` and `circuit sim`, the circuit list and
the catalog (`archive::studies()`), `mctas circuit run` (no new command per study) and the site
(crates/site/src/explain.rs asks the registry first; `explain::studies()` gives the study its page).
Redsim items a study adds go in its own redsim module's `pub const MODULE_ITEMS: &[(&str, u8)]`
(crates/redsim/build.rs collects every such list; their ids follow the frozen
`container::MORE_ITEMS`), then `redsim-fuzz recipes-gen` if a crafter may hold them.

The kinds older than the registry keep their `Kind` variants and their arms in the shared files;
one moves by turning its variant into a `KindDef`. The item elevator (crate::lift,
`src/studies/lift.rs`, `circuit: item-elevator N`) is the worked example: the frozen tables
(`archive::STUDIES`, the site's `STUDIES`) keep its place in the order, and its registered words
win.

## Archive and catalog

**After every merge: `mctas clock tables --fix`** (crate::tables in crates/clock; `circuit catalog`
and `clock catalog` run it first). The archive tables (clock verdicts, captions, extend, aliases,
vibration, type keys; each study's verdicts, captions, extend) merge with git's union driver, so
a row one branch UPDATED comes back twice (2026-10-21: every clock's verdict row doubled, its new
`usable` column hidden). The fix keeps one row per key: the most columns, then the newest
`archived` stamp (circuit verdicts) or the design file's current hash (clock verdicts), then the
side the merge changed (not in the merge base, from git), else the last (reported). The readers
apply the same rules. A union merge also brings back the rows of a design one branch DELETED
(2026-10-21: `site publish` failed copying a removed clock), so the fix then drops every orphan row:
a verdict, caption, alias, vibration or type-key row whose design file is gone, an extend row of a
family no archived design declares. The quick selftest `archive:tables` fails while a key is held
twice or an orphan row is left.

`mctas circuit archive <design>... [--caption '...'] [--root designs/circuits]` judges each design
(`circuit verify`) and keeps a passing one in its study's archive,
`designs/circuits/<study>/archive/`: the design with two header lines (`# archived <date> from
<path> by mctas circuit archive`, `# CIRCUIT: ...`), its row in `verdicts.tsv` (kind, the study's
axis value, metrics, blocks, box, family = the kinds of the blocks whose state changes,
robustness, block kinds, type key) and its caption in `captions.tsv` (default: the design's first
comment block). Refused: a failing design, or a structure already archived as the same kind (the
TYPE: up to translation, the 4 rotations and the mirror image, input cells marked with their
device (a lever, a stone or wooden button, the judge's redstone block: their timings differ; not
their facing), output and payload cells marked, after the swaps that keep the run identical on redsim under
the kind's stimulus, as for clocks: sticky pistons that never pull = pistons, inert solids =
stone: `same type as <file>`). `--dedupe` (no game) keeps one design per kind and type in every
study (the earliest archived) and lists the removed twins in `aliases.tsv` ("built with oak_planks,
without stone"); the catalog shows them as "also found as". It also tidies what a union merge of
the TSVs leaves: one verdict row and one caption per file, and no row whose design file is gone
(`circuit catalog` refuses such a row). A name taken by another structure
gets `-<6 hex digits of its type hash>`. The axis value: pulse kinds `L = <L>`, delay `N = <N>`,
gates the function, T flip-flops their family, latches and registers the kind, `counter N`,
`shift N`, extenders `N = <N>`,
flying machines their speed (or `two-way`).

`mctas circuit catalog [--root designs/circuits]` (no game) writes, per study,
`designs/circuits/<study>/CATALOG.md` (by axis value, numbers ascending, the smallest design first:
design link, kind, metrics, blocks, box, robustness, caption) and `variants.md` (the dominance fold,
as for clocks: within a kind and axis value, a design with more blocks that uses every block kind of
a smaller one, plus others, is its variant, listed under it with the kinds it adds); the index
`designs/circuits/CATALOG.md`; and `designs/circuits/catalog.tsv`, one row per archived design
(study, kind, axis, name, path, metrics, blocks, box, family, robustness, variant_of, caption): the
data the website renders.

**Extendable families** use the clock `extend` headers ([clocks.md](clocks.md#extendable-clocks-extend-headers-vjeux-2026-10-19)) with
`extend axis:` for the formula of the study's axis value in n (L, N, the reach; `extend period:`
is read the same), e.g. track pulse's `extend family: limiter-u`, `extend axis: 4 + 4n`, `extend
blocks: 10 + 4n`, `extend generator: mctas circuit pulse family limiter-u --n {n} --out {out}`,
and the same `extend group` headers. `circuit archive` of a family base judges n = low, low+1,
low+2, the middle and the high end in the game (built with the generator; each must pass with the
formulas' axis value and block count) and records them in `extend.tsv`; the catalog shows one
card per family (or group) at the top of the study page (the formula as the heading, the two
smallest members' value and size, the rule, the n range and the judged n), its members leave the
value tables, and `designs/circuits/families.tsv` lists the families for the website.
`mctas circuit gen repeater-line --n N --out F` is the judge's own calibration family (n + 1
delay-1 repeaters: N = 2 + 2n gt).

**Tunable groups** (vjeux 2026-10-19: "we don't need 20 variations of the same design"): designs
that are ONE mechanism with other repeater delays, comparator modes or a longer repeater line are
one card. A design joins a group with a `group: <name>` header (optional `group title:` and `group
note:`, read from the first member carrying them; track generators stamp them, e.g. `mctas circuit
pulse stamp-group`), or automatically when its TUNABLE key (the type key with every repeater's
delay and every comparator's mode masked, `archive::tunable_key`) and kind equal another
design's. A group of two or more designs gets a card under "Tunable groups" (the value range as
the heading, the two smallest members, a table value -> blocks -> design, and the extendable
families carrying the same `group:`); its members leave the value tables. **Equivalent layouts**:
designs with the same kind, value, block count and block kinds as a shown design (the one with the
smallest box, then archived first) are listed on its row as "also N other layouts". `catalog.tsv`
has a `group` column (the member's group) and its `variant_of` names a layout's shown design too.
`mctas circuit catalog` prints, per study, the card count before and after the folds.

## Commands

| command | what |
|---|---|
| `mctas circuit info <design>...` | parse: kind (study), inputs and how they are driven, outputs, payloads, block count |
| `mctas circuit sim <design>... [--table] [--wave] [--stimulus S] [--gap N]` | the redsim screen: the kind's stimulus, the checker's table (`--table`), the waveform (`--wave`: `#` on, `.` off, an output's level as a hex digit), the CIRCUIT line |
| `mctas circuit verify <design>... [--positions 4] [--no-rotations] [--workers 8] [--table] [--stimulus S] [--gap N]` | the judge in the real game (exit 1 unless every design passes); several designs are judged side by side, up to the pool's size at once, every build on the one worker pool (`MCTAS_MAX_WORKERS` caps it), their reports printed in the given order |
| `mctas circuit material <designs> --out DIR [--judge]` | OBSIDIAN ONLY WHERE NEEDED (vjeux 2026-10-21; [crate::material](../crates/circuit/src/material.rs)): every obsidian block tried as stone on the kind's redsim judge (all at once, else one at a time, then the passing ones together); a swap stays only with the same kind, axis and metrics (a TNT duper still intact). Each kept obsidian gets its reason from a netherite-block probe (blast-proof like obsidian, but pistons move it): kept for blast resistance when the design passes with netherite there, else for immovability. The changed designs (their comment lines plus a material line naming each kept block's reason) and material.tsv go to DIR; `--judge` confirms them in the game. `circuit archive` refuses a design whose obsidian can be stone |
| `mctas circuit compact <design> --out DIR [--region x0,y0,z0,x1,y1,z1] [--palette "state;state"] [--iterations 4000] [--workers 24] [--game 2]` / `mctas circuit compact --all [--studies a,b] --out DIR` | MINIMUM VOLUME for any kind ([crate::compact](../crates/circuit/src/compact.rs), the moves and chains of [crate::anneal](../crates/circuit/src/anneal.rs), shared with `circuit tnt compact`): box-bounded annealing from the seed on the kind's redsim judge; a candidate passes with the seed's kind and axis value and ranks by the judge's box (every non-air cell over the run), then blocks; the palette defaults to the seed's states plus stone; the front goes to DIR/front/, `--game N` judges the smallest that beat the seed in the real game (robust, the same axis). Every worker judges at once, so the workers are capped to fit a quarter of the box's available memory at the seed judgement's measured peak (a 4-bit adder slice's judgement peaks at ~9.5 GB: 5 workers on a 223 GB box; 32 filled two boxes on 2026-10-21 and oomd killed their node agents), and each judgement's TNT robustness runs use one thread (`verify::Options::sim_threads`). An item sorter (study 13) is judged by its own redsim runs (`sorter::sim_score`: every required run, broken candidates ranked by the runs passed and the items put in the right place), its box the slice's own, and keeps the seed's badges (`overflow-proof`) unless `--any-badges`; an auto-smelter (study 17) likewise by `smelter::sim_score` (runs passed x 1000 + the items its first failing run delivered), an auto-brewer (study 21) by `brewer::sim_score` (runs passed x 1000 + the potions of swiftness its first failing run delivered). `--all`: each study's minimum-volume robust design per (kind, axis) from the archive, one run each, DIR/sweep.tsv; `--collect SWEEP --out TRACKDIR` writes the sweep's game-OK designs with captions built from their seeds' (variants drop the seed's `extend` family headers), ready for `circuit archive` |
| `mctas circuit archive <design>... [--caption C] [--root designs/circuits]` | (coordinator) judge and keep in `designs/circuits/<study>/archive/` |
| `mctas circuit archive --rejudge [--study S]...` | judge every archived design again (the game; family bases with their sampled members): passing ones get their new metrics and `# CIRCUIT:` line, failing ones leave the archive and go back next to their track file (`update-less/` or `rejudged/`, first line: why) |
| `mctas circuit archive --fill-game-rts [--study S]...` | no game: every archived harvester's `game rts` (the farm judge's robustness speed, from its vanilla twin run alone) written into its verdict row, summary and `# CIRCUIT:` line, before its bill; every other metric kept |
| `mctas circuit catalog [--root designs/circuits]` | (coordinator) every study's CATALOG.md + variants.md, the index, catalog.tsv |
| `mctas circuit local-census <design> --remove R --cells=x,y,z..x,y,z [--palette stone,wire,torch,wtorch-n,rep-s,obs-u,...] [--keep hopper,...] [--out F]` | a shrink in one step on redsim ([crate::local](../crates/circuit/src/local.rs)): every design with up to R of the base's blocks removed (none whose state contains a `--keep` word) and fewer palette blocks added in the box's air cells; a sorter is judged by its runs, any other kind by its checker with the base's main value (its first metric). About 100k candidates/s for a sorter, 1k/s for a slow hopper timer. `sorter-census` is the same for sorters (hoppers and comparators kept) |
| `mctas circuit sorter-run <designs> [--run base\|tiled\|stream\|jam\|direct\|all] [--sim] [--compare]` | study 13: one run of the sorter judge at rotation 0, in the game or on redsim, and what reached each chest (`--sim --run all`: the whole judge on redsim, one line per design); `--compare` is the exact gate: the run in the game and on redsim, every cell of the watched box (block state, container contents, comparator outputs) compared every gt from the placement to the last item |
| `mctas circuit smelter-run <designs> [--run base\|tiled\|tiled8\|tiled16\|intrusion\|chest\|all] [--sim [--coal-log] [--trace]] [--compare] [--rotations N]` | study 17: one run of the auto-smelter judge at rotation 0, in the game or on redsim, its table (what reached the output, items per furnace, a jam's diagnosis); `--sim --run all`: the whole judge on redsim (`SMELTER: ...` / `FAIL: ...`); `--run chest`: the super smelter's run (a double chest of raw iron, the coal it needs plus a stack); `--rotations N`: the chest run in the game at 4 rotations x N positions; `--coal-log`: per furnace, the gt each coal reached it (stderr); `--trace`: hopper and furnace contents (stderr); `--compare`: every cell of the watched box (furnace timers and lit state included) compared every gt |
| `mctas circuit brewer-run <designs> [--run base\|tiled\|tiled8\|intrusion\|all] [--sim] [--compare] [--trace]` | study 21: one run of the auto-brewer judge at rotation 0, in the game or on redsim, its table (what reached the output, brews per stand, what is left in each stand); `--sim --run all`: the whole judge on redsim (`BREWER: ...` / `FAIL: ...`); `--compare`: every cell of the watched box (brew and fuel timers, the bottle bits included) compared every gt; `--trace`: every stand's slots, the hoppers above and under it and its brew timer every 20 gt of the first 4000 (stderr) |
| `mctas circuit composter-run <designs> [--run base\|tiled8\|intrusion\|all] [--sim] [--compare]` | study 22: one run of the auto-composter judge at rotation 0, in the game or on redsim (the level random mirrored), its table (the bone meal and overflow chests, the composter levels at the end); `--sim --run all`: the whole judge on redsim (`COMPOSTER: ...` / `FAIL: ...`); `--compare`: every cell of the watched box compared every gt (the composting chances drawn alike) |
| `mctas circuit lift-run <design>... [--run base\|intrusion] [--margin 3] [--sim [--turns T --offset X,Y,Z --watch ITEM] \| --builds \| --compare-verify]` | one item-elevator run in the game (`--sim`: on its redsim twin, `--turns`/`--offset` one of the judge's builds, `--watch` printing the matching item entities every gt), its table (loaded, delivered, the first and last item's gt, items/s between 10% and 90%, where items stay at the end) and outcome; `--builds` the twin's whole judge, every build's line; `--compare-verify` the game judge and the twin, build by build (SAME or each difference) |
| `mctas circuit run <design>... [--run R] [--margin 3] [--sim [--turns T --offset X,Y,Z] [--watch W]] [--trace DIR] \| --builds \| --compare-verify` | any registered study's kind ([Adding a study](#adding-a-study-the-registry)): one run (`--run`: the kind's runs, the first by default) in the game or on its redsim twin (`--sim`; `--turns`/`--offset` one of the judge's builds), its table and outcome (`RUN:` or `FAIL:`); `--builds` the twin's whole judge, every build's line; `--compare-verify` the game judge and the twin, build by build |
| `mctas circuit farm-run <design> [--tiled] [--rts 300] [--ticks 24000] [--turns T --offset x,y,z] [--trace] [--margin 3] [--sim \| --compare [--probe] \| --compare-verify]` | study 18: one harvester run in the game (real random ticks), its table (growths, harvests, items, waits, intruders, escaped and stranded items) and outcome; `--turns/--offset` replays one `circuit verify` build (a fresh run is deterministic per build), `--trace` steps one tick per batch (a different run) and prints the last 40 moves of every crop item that goes off its way or comes to rest on a crop (its position, motion, the block it is in and on, the crop column) and the items lingering 40+ ticks in a crop column; `--sim` runs the judge's redsim twin instead (`--turns` turned without the game), `--compare` both, their records side by side (they must be the same; the build's box and seed printed; `--probe`: every tick of the run compared, blocks, entities, level random and randValue, the first difference printed with the drop number and cell of a differing item), `--compare-verify` the whole judge (`circuit verify`, every build and the tiled run) in the game and on redsim build by build, every turned design checked to turn alike without the game |
| `mctas circuit station-run <design> [--turns T --offset x,y,z] [--margin 3] [--trace DIR] [--sim \| --builds \| --compare-verify]` | study 19: one minecart station run in the game (every cart: where it rests, how long it is held, its departure delay and speed; the carts the other way), its outcome; `--turns/--offset` replays one `circuit verify` build; `STATION_TRACE=1` prints the cart's position and the design's rails' states every tick on stderr; `--trace DIR` records the run as an exact trace (`DIR/<name>-judged.txt`, `DIR/traces/<name>-judged.trace`) for `redsim-fuzz trace-compare`; `--sim` the run on the redsim twin, `--builds` the twin's whole judge (every build), `--compare-verify` the game judge and the twin build by build |
| `mctas circuit cart-run <design> [--turns T --offset x,y,z] [--margin 3] [--trace DIR] [--sim \| --builds \| --compare-verify]` | study 19: one hopper-minecart unloader or loader run in the game (every cart: where it rests, when it became empty / full / dry, when it left, what it took or what reached the chest; the strays), its outcome; `--trace`, `--sim`, `--builds`, `--compare-verify` as for `station-run` |
| `mctas circuit cart-line --loader <file> --unloader <file> [--track 128] [--spacing 8] --out <file>` | cart transport ([brief](../designs/briefs/cart-transport.md)): a whole line from two stations, the unloader turned half a turn at the west end, the track (a powered rail every `--spacing` blocks, each lit by a torch), the loader at the east end, aligned on their `line.in` cells; the double supply and storage chests beside the stations' chests (crate::cartsys) |
| `mctas circuit cart-system <lines> [--sim] [--trickle]` | the whole-line run (crate::cartsys): the double supply chest filled with 3456 items, one empty hopper minecart on the track; prints `SYSTEM: <line> minutes for a double chest, items/s, trips, bill (track and cart included), blocks` or `FAIL:` (an item lost or left, the cap); `--trickle`: the supply fed 5 items every 100 gt up to 300, then nothing: every item must arrive and the cart must end parked at the loader (still for 2000 gt within 4 blocks of the supply chest) |
| `mctas circuit door-run <design> [--turns T --offset x,y,z] [--margin 3]` | study 21: one automatic door run in the game (every walk: how long the walkers waited at the door, how long they stood in the doorway, when the door closed after them), its outcome; `--turns/--offset` replays one `circuit verify` build; `AUTODOOR_TRACE=1` prints every design cell that changes, tick by tick, on stderr |
| `mctas circuit arith synth --kind K --in a0[=x,y,z] --out s0[=x,y,z]... --grid WxD [--height H] [--auto-ports --in-sides nesw --out-sides nesw] [--r7] [--sym a0,b0,..] [--chain cin,cout] [--fix <design>] [--ring-all] --out-dir D` | SAT synthesis of a combinational circuit ([crates/circuit-arith/src/synth.rs](../crates/circuit-arith/src/synth.rs)): a CNF model of a WxDxH grid (stone, redstone block, dust with its shape and dust steps up and down, torches, repeaters, comparators), every input combination solved at once, then minimized by block count; each model is judged by `circuit sim` and a rejected one is blocked (CEGAR). `--r7`: input cells are the judge's stone driven by its repeater (the compute kinds' R7); `--sym`: ring-order symmetry breaking between interchangeable inputs; `--chain cin,cout`: a bit slice (cout a lamp directly powered, `slice:` = cout - cin); `--fix`: complete a partial design (its blocks fixed); `--ring-all`: auto ports on every component layer (3D). Ports given `=x,y,z` are fixed, anywhere (inside the grid too). Adders: designs/circuits/adder/tracks/w86/NOTES.md. Kinds: adder, subtractor, incrementer, comparator, decoder (optional enable `e`), mux N. `--sample K` starts the model with K combinations and adds each combination the judge's redsim run fails on (with `--r7`, read from its `X rises from <state>` line), so a big table (a 4:1 mux: 64) need not be in the model at once. Building blocks: designs/circuits/compute/tracks/w86/NOTES.md |
| `mctas circuit <track> <args...>` | track commands: `pulse`, `tff`, `memory`, `delay`, `gates`, `pext`, `fly`, `bud` -> `circuit_<track>::run(args)` in `crates/circuit-<track>` |
| `mctas circuit bud census --k K [--palette all\|qc\|dropper\|"<state>;..."] [--reach 2] [--threads 6] [--out-dir D --keep N --only <classes>] [--base D] [--tile N] [--count]` | every connected design of K blocks around a target cell (one cell set per symmetry class, one design per structure), screened on redsim with the BUD judge (classes only) at every air cell beside any cell a block takes during the run; per detected class set: how many, the fastest. `--base D`: keep D's blocks and add K blocks in the air cells of its box grown by 1. `--tile N`: each passing design's densest tiling (pitch 1..=N; with `--only`, that class set's only), listed first and written with its `tile:` header |
| `mctas circuit bud trace <design> [--from F \| --phase <label>] [--frames 40] [--full]` | a BUD design's redsim run, frame by frame: the phase, the output level, the blocks that changed (`--full`: the footprint probes and re-arm pairs too) |
| `mctas circuit bud tile <design>... [--max 4]` | each BUD design's densest tiling on redsim: the smallest pitch per axis (x, y, z) at which 3 copies pass the judge's tile check |

Tracks: put your designs in `designs/circuits/<study>/tracks/<track>/`; the coordinator archives
the verified ones.
