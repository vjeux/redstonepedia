# Update suppressor - notes

Track: explore/update-suppressor, based on explore/bedrock 7c50171 (origin/main does not
contain the bedrock work yet; verified `merge-base --is-ancestor` = NO). Brief:
`designs/briefs/update-suppressor.md` (read it first - mechanics, layouts, judge).
All commands run from the repo root of this clone; the game binary used for source reads
before this clone's first build was the parent's `~/agents/bedrock/minecraft-tas/target/release/mctas`
(read-only `src` commands; the parent clone is never edited).

## Tools

- redsim (`crates/redsim/src/world.rs`): `World.nu_stats_on` records every outermost
  cascade's `CascadeStats` (counted calls by kind, executions, dropped calls, first dropped
  position) into `nu_history`/`nu_last`; `World.nu_cap` overrides the level's
  `maxChainedNeighborUpdates` (default the game's 1,000,000). Neither touches the state key.
- `mctas suppress probe --family <f,...> --trigger <t,...> --sizes <n,...> [--cascades]`:
  families `dust`, `dust-stack:<layers>`, `rail:<pattern>:<kinds>:<geom>` (patterns
  full|comb|t|t1|ladder|grid3; kinds checker|plain|detector|jp; geoms flat|slope|stepx),
  `rail-rand --seeds K`. Builds are player-like (supports raw-put, rails/wires set with
  updates in a fixed order - random patches shuffled), rest states are the game's.
  `SUPPRESS_DUMP=1` prints ASCII planes of the wire/rail layers before/after the trigger.
- `mctas suppress judge [--cap N] [--control] [--field N] [--positions ..] [--arms ENSW]`:
  the brief's judge on redsim (positions x arms x mirror; control + suppress contracts).
- `mctas suppress bridge [--field N] [--arms ..]`: the same script in the bridge sandbox
  (the game's cap), lever-call wall time included.
- `mctas suppress vanilla [--cap N] [--field N]`: the official dedicated server
  (`mctas::vanilla`, `VanillaOptions.extra_properties` sets
  `max-chained-neighbor-updates`), bot-clicked lever, `fill`/`setblock` placement.
- `mctas suppress emit [--control] --out F`: the judge rig as a design file
  (`rig-flood-31.txt`, `rig-control.txt` here; the `redstone` port block is placed last).

## 2026-11-07 - source audit (26.3 decompiled, `~/.cache/mctas/versions/26.3/src`)

- Cap: `SharedConstants.java:135` `MAX_CHAINED_NEIGHBOR_UPDATES = 1000000`;
  `DedicatedServerProperties.java:100` property default 1000000;
  `MinecraftServer.java:2033-2035` returns 1000000 (singleplayer);
  `ServerLevel.java:239` passes it to `Level` (142-151) -> `CollectingNeighborUpdater`.
- `CollectingNeighborUpdater.addAndRun` (68-85): `count++` per added call; drop at add time
  when `count >= max`; one LOGGER.error at exactly `count-1 == max`, rest silent. Queued
  entries are NEVER dropped at execution. `runUpdates` (87-112): depth-first via
  `addedThisLayer` reversed onto the stack; `count = 0` in `finally` (per outermost cascade).
- `MultiNeighborUpdate.runNext` (148-169) delivers up to 6 `executeUpdate`s per counted call,
  order `NeighborUpdater.UPDATE_ORDER` (18) = W,E,D,U,N,S.
- `Level.setBlock` (216-262): flag1 -> one `updateNeighborsAt` (245); shapes (251-255) with
  `neighbourUpdateFlags = flags & -34`; `LevelChunk.setBlockState` (267-350): same-state =
  null/no-op (279-281); `onPlace` on every real change unless flag 512 (325-327);
  `(blockChanged || BaseRailBlock) && (flags&1 || moved)` -> old state's
  `affectNeighborsAfterRemoval` (317-319).
- `Block.UPDATE_LIMIT = 512` (106) is the separate shape-chain depth budget.
- Phases (`ServerLevel.tick` 358-460): scheduled block ticks 394 (cap 65536/tick), fluid 396,
  block events 409 (`runBlockEvents` 1267-, drain whole), block entities 459. Each starts
  fresh cascades; none can be starved by a flood elsewhere.
- Dust: `DefaultRedstoneWireEvaluator.updatePowerStrength` (18-38): per power flip,
  `setBlock(pos, ..., 2)` then `updateNeighborsAt` for pos + 6 neighbours (HashSet order).
  Flat field flip = 6 shape counts (setBlock's `updateNeighbourShapes`) + 7 multi = 13.
  Indirect shapes (`RedstoneWireBlock` 204-227) only fire toward a wire one step up/down
  across a non-wire cell: flat fields get 0, stepped lattices up to +8/flip (old+new state).
- Rails: `PoweredRailBlock.updateState` (121-133) one flip/edge:
  `setBlockAndUpdate` (1 multi + 6 shape) + chunk rail-removal calls
  (`BaseRailBlock.affectNeighborsAfterRemoval` 117-128: pos + below, +above if slope)
  + own `updateNeighborsAt(below)` (+above if slope) =~ 10-11 counts/flip;
  `findPoweredRailSignal` (23-96) scans <=8 per direction per execution (CPU cost).
  `RailBlock.updateState` (22-26): plain rails re-`place` only when the changed block is a
  signal source and potential connections == 3. `RailState.place` (218-347) and `connectTo`
  (143-206) compute shapes by different rules - the one candidate non-monotone amplifier;
  to probe on small junction meshes.
- Scaffolding (`ScaffoldingBlock` 88-122) and leaves (`LeavesBlock` 86-121): distance
  recompute is a 1 gt scheduled tick; `updateShape` only schedules. Cannot flood one cascade.
- Every `updateNeighborsAt(` / `level.neighborChanged(` caller in the game was listed
  (`src grep`): all guarded by a state change or in scheduled/block-event/block-entity
  handlers. No block issues neighbour calls unconditionally from `neighborChanged`.
  Consequence: one cascade's counted calls <= blocks x flips/block x counts/flip, flips
  monotone per source edge (dust power 16 values, mirrors 1, rails 1). Floor in the brief.
- Lever: `LeverBlock.pull` (82-88) = `setBlockAndUpdate` + `updateNeighbours` (133-140, two
  more outermost cascades). Rig lives in cascade #1; #2/#3 must be no-ops for T (T is not a
  neighbour of lever or support, B already flipped in #1).

## redsim fidelity (before instrumentation)

`crates/redsim/src/world.rs` already ports `CollectingNeighborUpdater` exactly: `Nu` enum
(Full/Multi/Shape/Simple) 36-41, `MAX_CHAINED_NEIGHBOR_UPDATES = 1_000_000` (32),
`add_and_run` (637-653) drops at add time and pushes `errors: "too many chained neighbor
updates"` once, `run_updates` (655-677) resets `nu_count = 0` at the end. `set_block_limit`
(468-522) mirrors `Level.setBlock` incl. rail `affect_neighbors_after_removal` on every rail
change (492). Dust evaluator port `redstone/wire.rs` (HashSet order via `java_hashset_order`),
rails port `rails.rs` (RailState place/connectTo). What is missing is *observability*:
`nu_count` resets unseen; nothing reports a cascade's total counts, per-kind counts,
executions, or which calls were dropped. Smallest faithful addition: an opt-in stats struct
on `World` updated in `add_and_run`/`run_next`, not part of any state key.

## Dead ends so far

- Shape-depth (512) starvation as the suppressor: presence-function shapes don't travel;
  pistons (the judge target) react to neighbour notifications, never gated by `updateLimit`.
- Scaffolding/leaves distance fields: scheduled, per-cell cascades.
- Piston 0-tick chains: block-event phase, every `moveBlocks` runs at cascade top level.
- **Dust field fed from one trigger: radii, not fields.** The only instant signal medium is
  dust, its power decays 1 per wire, and every instantaneous mirror block (note block, lamp,
  bulb `powered`, dropper `triggered`) emits no signal; every true source besides dust
  (torch, repeater, comparator, observer) changes output in a scheduled tick - there is no
  instant repeater element in 26.3. So in one cascade only wires within 15 wire-steps of the
  trigger's signal can flip: <= ~481 wires in a plane, <= ~4991 cells in a full 3D Manhattan
  ball of radius 15 - at 13 counted calls/flip and 1-3 flips/wire that is tens of thousands
  of counts, not a million, however big the field is built. Distributed static sources
  (redstone blocks, torches) cannot change inside the cascade either (torches schedule).
  The brief's layout 2/3 estimates die on this; layout 5 (rails) and the placement wave
  below are the live candidates. Ceiling section of the brief to be rewritten from probe data.
- Powered-rail power wave: `findPoweredRailSignal` reaches ~8-9 rails from a powered source
  rail; intermediate rails have no neighbor signal, so the wave stops (counted in the source:
  search depth caps at 8). Rails need a shape wave, not a power wave.

## The wave candidates the probe tests (in order)

1. **Rail shape wave (lever trigger).** A plain rail with exactly 3 potential connections
   re-runs `RailState.place` when a *signal source* changes next to it
   (`RailBlock.updateState`), and `place` chooses its curve by `hasNeighborSignal`;
   `place` then rewrites its connected neighbours with `connectTo` (a different shape rule).
   Detector rails are signal sources (`DetectorRailBlock.isSignalSource` = true), plain and
   powered rails are not. In a mixed plain+detector junction mesh a lever flip next to one
   junction could therefore propagate: plain flips curve -> `connectTo` setBlocks a detector
   neighbour -> that setBlock's updates have a signal-source kind -> the next plain junction
   re-places. Wave or oscillation is empirical: probe counts vs mesh size on small meshes.
2. **Rail placement wave (place-one-rail trigger).** `onPlace` -> `place(first=true)` has no
   signal-source gate for the placed rail and `connectTo`s neighbours; with detector rails in
   the mesh the wave can continue as in (1). Trigger = place the last rail, reset = break it
   (repeatability to measure: removal does not re-place plain neighbours, source = air).
3. **Sponge absorption** (fallback): `SpongeBlock` absorbs up to 64 water cells with direct
   `setBlock`s inside its `neighborChanged` - calls accumulate in the running cascade, and
   neighbouring sponges may chain. One-shot (wet sponge), reset by drying: poor reset quality.
4. Dust ball (3D, radius 15) measured as the bound case for the report's floor argument.

## Probe results (2026-11-07, `./target/release/mctas suppress probe ...`, at (0,64,0))

Rails (the wave hunt, layout 5):
- 9 patterns (full/comb/t/t1/ladder/grid3) x kinds x flat/slope/stepx x lever/place/break,
  sizes 12-64: counted calls between 1 and 18, **independent of size**; the one deliberate
  junction flip (rail:t:jp, lever) flips exactly the lever's junction (`north_east` ->
  `north_west`) and stops: counted 8, cells_flipped 2, at every size.
- `rail-rand`: 300 patches at n=6 and n=10 (density 55-99%, heights 1-2, shuffled build
  order, lever+break): counted p50=1, p90=8, p99=18-22, **max 40** (n=10, break).
- Why: on a settled mesh `RailState.place` recomputes the shape already present (no
  setBlock, no `connectTo` loop), `connectTo`'s axis rule (EW wins both axes present)
  never flips contested straight rails in these topographies, and plain rails only
  re-place for signal-source sources (detectors) with exactly 3 potential connections.
  Build cascades peak at 8-24 counted too: placing rails into a mesh does not flood either.

Dust (layouts 2-3), lever at the field center:
| family | n | blocks | counted on | counted off | cells flipped | flips_max |
|---|---|---|---|---|---|---|
| dust | 15 | 449 | 4,278 | 13,105 | 225 | 5 / 8 |
| dust | 21 | 881 | 6,371 | 17,525 | 381 | 4 / 8 |
| dust | 31 | 1,921 | 7,385 | 19,345 | 481 | 3 / 8 |
| dust | 45 | 4,049 | 7,476 | 19,345 | 481 | 3 / 8 |
| dust | 63 | 7,937 | 7,385 | 19,345 | 481 | 3 / 8 |
| dust-stack:3 | 31 | 5,762 | 13,434 | 30,101 | 850 | 3 / 8 |
| dust-stack:5 | 31 | 9,606 | 13,905 | 30,571 | 874 | 3 / 8 |

481 = cells of the radius-15 diamond exactly. Off-edge flips each wire up to 8 times
(step-wise collapse), on-edge up to 3 - both under the monotone bound (flips <= power).
An east-edge ramp to upper layers flips nothing up there (the edge sits at power ~0);
the measured stacks use central staircases through a shaft in each upper support plane
(a conductor directly over a ramp wire blocks its diagonal step reads - found by dump).

## Judge / bridge / official-server results

Judge rig (`crates/suppress/src/judge.rs`): lever (0,3,0) on note block N0 (0,2,0);
head wire (0,1,0) under N0 in a 31x31 dust field (supports y=0); middle note block
(0,4,0); target piston facing up beside it; redstone block diagonally over the target
(quasi cell) placed last; wake cell perpendicular to the target.

- redsim `--cap 5000 --control`: **96/96 PASS** (6 positions (0,64,0), (1,64,0),
  (15,64,0), (16,64,0), (14,64,13), (-16,64,-1) x 4 arms x mirror x control/suppress).
  Suppress counts 5,475-5,748 attempts (475-748 dropped); middle flipped, target stale,
  wake extends, reset clean, idle at the end, no unexpected model errors.
- redsim `--cap 6000`: 8/8 PASS at (0,64,0) (6,242 attempts, 242 dropped).
- redsim, game cap `--control`: 48/48 control PASS, **0/48 suppress** (6,580 attempts in
  the rig's feed geometry; target reacts).
- bridge (real game): 4 arms x control/flood identical to redsim; lever-call wall time
  18-38 ms for the flood cascade (~6.6k counted, ~25k handler executions): the same
  ratio puts a hypothetical cap-hitting cascade at roughly **4-7 s of freeze** -
  bounded, no crash, but that is what 1,000,000 counted calls costs in 26.3.
- official server (`suppress vanilla`, bot click, tick-stepped): control reacts at every
  cap; flood suppresses at cap 5000 and 6000, **not at 7000, not at 1,000,000**; binary
  bracket: suppresses at 6500, not at 6600. Redsim counts 6,580 attempts for the identical
  cascade, and B's `updateNeighborsAt` is among its last adds (its multi precedes its
  6 shape calls): predicted threshold ~6,57x, inside the measured bracket - **redsim's
  counter is calibrated against the game to ~1%**.

Survival build sequence of the rig (for a lowered-cap world, or as the shape any future
flood must feed): lay the support plane and field wires (unpowered), the starter note
block over the head wire (click any adjacent face - it needs no support), the lever on
it (last of the trigger parts, left off), the middle note block over the lever (against
a temporary block), the piston beside it facing up (look down from a temporary scaffold),
then the redstone block diagonally over the piston **last** - its placement updates reach
its own neighbours only, never the piston. Trigger: flip the lever. Wake: place and break
any block in the wake cell (or update the target any other ordinary way). Reset: lever
off (that edge is suppressed too while the flood is connected: the target simply never
heard either). Failure mode worth knowing: after a suppressed edge, flood wires whose
updates were dropped can sit at stale powers; in the judge (fresh builds per variant,
counts ~10% over the cap) both edges still suppressed - at counts far over the cap the
field would need a settle check per use.

## The bound, in one place

One cascade's counted calls from a single trigger are bounded by what can *change* inside
it: per changing cell, its flips x counted-calls-per-flip (dust <= 29, rails ~11, mirrors
~7, shape corrections ~6-7 - a `setBlock` with an unchanged state counts nothing).
Changes need a cause inside the cascade: (a) signal - bounded to 15 wire-steps of the
trigger's changed cells (dust is the only instant medium; every other source changes
output in a scheduled tick; mirrors reflect signal but emit none); flip-monotonicity on
one edge caps the dust contribution at the 50,625-flip shell sum over the step-ball,
halved by the support under every wire: ~725k counted calls for an impossible perfect
packing, ~31k measured for the best supportable lattice tried; (b) shape corrections -
functions of neighbour presence (fences, walls, wire connections) propagate one cell from
a presence change; the state-dependent ones (bamboo leaves, dripstone thickness) are
vertical columns <= world height per trigger column; rail shapes are a fixed point of
`place`/`connectTo` on any settled survival mesh (probe); (c) the two in-cascade
multi-block loops - sponge BFS (<= 64 water cells, ~460 counted calls per dry sponge,
chaining only through shared water: >= ~2,174 sponges to reach the cap, one-shot) and
the frosted-ice melt chain (`FrostedIceBlock`: melts when a frosted neighbour changed
and < 2 frosted neighbours remain, ~8 counted calls per cell: a 125k-cell narrow line of
a block that (i) has no item form, (ii) is only created by Frost Walker on water,
(iii) random-tick ages toward melting on its own). Everything else schedules a tick,
queues a block event, or waits for a block-entity/entity phase - fresh cascades.

## Phase 2 (2026-11-07): the exponential hunt - a source-wide census

Owner's direction: no more block spam; find a branching (superlinear) update sequence or
prove there is none family by family. Every `neighborChanged` override (24 block classes),
every `updateShape` override (~118, minus level/chunk bookkeeping), `onPlace` loops, and
the `updateNeighborsAt`/`neighborChanged` callers were (re)read. (`onNeighborChange` does
not exist in 26.3 - zero grep hits. `handlePrecipitation` runs from weather/chunk ticks,
never inside a neighbor cascade.) Classification of every family that can change a block
state inside a cascade, by what one execution can cause downstream:

| class of behaviour | families | downstream per execution | revisit bound per cell per cascade |
|---|---|---|---|
| schedules a tick / queues an event / block-entity (phase exit) | observer, repeater, comparator, torch, lamps-off, buttons/plates, diodes' outputs, pistons (events), droppers/crafter (events), sculk, daylight, hopper/container analog, leaves + scaffolding distance, cactus/cane/bamboo-survival, coral death, frosted aging, fire spread/burnout, fluids, gravity, kelp/bamboo growth, command blocks | 0 state changes in this cascade (scheduling is free and uncounted) | n/a |
| mirror of neighbour signal (one flip per edge; signal is monotone inside a cascade - emitters are dust (monotone), static sources, and scheduled devices frozen mid-cascade) | note block, lamp (on), copper bulb `powered` (+ <= 1 `lit` toggle), dropper/dispenser/crafter `triggered`, skull, shelf `powered`, bell `powered`, doors/trapdoors/fence gates (+/- the second half), big dripleaf tilt-reset, hopper `enabled` | own 1 setBlock (~7-9 counted calls) | 1 |
| static shape function of static inputs (neighbour *presence/type/facing/half*, own static props) | **stairs** (`getStairsShape` reads neighbours' FACING/HALF only - both immutable in a cascade), fences, iron bars/panes, glass panes, chorus plant, huge mushroom, fence-gate in-wall, wire connection sides, fire faces, snow/grass `snowy`, multiface (glow lichen/sculk vein face support), chest *shape*, campfire | <= 1 correction, only in the ring around a presence change (the correction cannot change any other cell's inputs) | 1 |
| pairs / bounded lines | chest + copper chest adoption (below), beds, doors' halves, double plants/pitcher, piston head, end portal frames, shelf side-chains (max 3, `getMaxChainLength`), tripwire lines (<= 41 cells scanned, attached flips once per line), nether portals (interior <= 21x21, one-shot air) | <= 1 partner, or <= 41 setBlocks (tripwire `calculateState`) | 1-2 |
| vertical/linear automatons (branching factor 1) | **walls** (below), pale moss carpet (2-high), vines (face inherited from above), pointed dripstone thickness (TIP->FRUSTUM->MIDDLE->BASE by distance from tip), bamboo AGE (0->1 downwards), kelp head/body conversion (one cell), snow-layer support pops, big-dripleaf stems | 1 cell (the next in line) | <= 2 (derived for walls; measured 1) |
| survival pops (one-shot, the cell leaves) | torches, rails, levers/buttons, signs, plants, corals, cake, snow, attached everything | the cell's own removal + its updates | 1 (gone) |
| signal propagation (phase 1) | dust powers (<= 15 steps, monotone flips), powered rails (<= ~9 track steps to a signalled rail), rail junction curves (fixed point on settled meshes) | bounded ball, see phase 1 | dust <= its power; rails 1 |
| **in-cascade multi-block loops (the only branching > 1 executions in the game)** | **sponge** BFS (<= 64 water cells, ~7 counted calls each + wet-sponge write; neighbour sponge executions chain through shared water), frosted-ice melt condition (< 2 frosted neighbours: ~8 counted per melted cell, sparse lines only), rail `RailState.place` (self + <= 2 `connectTo` writes - never observed to take a second step on settled meshes, phase 1 + this phase's meshes) | sponge <= 64, frosted 1 (next in line), rails <= 2 | sponge 1 (wet), frosted 1 (gone), rails measured <= 2 |

Derivations for the three families a reader would doubt:

- **Walls** (`WallBlock.updateShape` 128-247). A wall's new state is
  F(presence pattern (static in a cascade), above-cell summary (its UP + arm presence,
  through `topNeighbourHasPost` and the down-face `isCovered` tests)). Shape updates from
  BELOW are ignored (`directionToNeighbour == DOWN` -> super), and a horizontal
  neighbour's recompute reads the changed cell only through `connectsTo` (wall tag /
  sturdy face / gate facing - all presence-level, unchanged by a same-block flip). So
  changes propagate strictly downward, one column at a time; the topmost changed cell's
  inputs are final after the trigger, so it flips <= 1 time to its final value, and by
  induction each cell below sees its above-input change at most once and flips <= 2 times
  even if a stray horizontal delivery makes it evaluate early against the stale above.
  Measured (bridge `wallrun`): n+1 cells change in an n-high run - exactly the placed wall
  plus one UP flip per wall under it.
- **Chest adoption** (`ChestBlock.updateShape` 144-172). A SINGLE chest adopts only when
  the changed neighbour is non-SINGLE and *its* connected direction points back. LEFT's
  partner is clockwise of its facing, RIGHT's counter-clockwise, so after one adoption
  both outer neighbours fail the direction equality on both sides: adoption extends a
  pair by exactly one cell and stops. Measured: 2 changed cells at every row length.
- **Wire shape x power.** Stored wire sides only change in `updateShape`, whose writes go
  through `updateOrDestroy` with neighbour bit cleared and never re-enter the evaluator;
  power obeys the phase-1 monotonicity proof regardless of sides (incoming signal reads
  neighbour *powers* and conductor presence, both static/monotone in a cascade;
  `getIncomingWireSignal` 29-47). Presence changes inside a cascade (the only thing that
  could rewrite sides) come from pops/removals/destruction in the same one-shot families.

Phase-2 probes (`suppress probe shape:*`, `fence-rand`, `suppress bprobe`):
fence fields constant 7 counted (600 random patches likewise, flips_max 1); bamboo column
counted = 6n+1 exactly (one AGE flip per segment); tripwire completion fires up to its
41-cell scan cap as many small cascades (hook `calculateState` runs from `onPlace`, at
cascade top level, like the game) and dies whole at line length 64; bridge snapshot diffs:
walls 1 (column) / **n+1 (run)** / 10 (field), stairs 1, dripstone 3 (tip-local), chests
2, snow tower n (instant pop wave up one column), chorus 2 after 1 tick then scheduled
falls only (12-27 by idle), vines 2 at a top break, panes 5, moss 2, portal interiors
3(n-2)+1 (19/43/64 at widths 8/16/23) - every entry constant or exactly linear in **one**
line/column, flips per cell <= 2 everywhere.

Conclusion: no recurrence with branching factor > 1 exists among survival blocks' cascade
handlers in 26.3. The only executions that touch > 2 cells are the sponge BFS (capped at
64 one-shot cells) and tripwire's 41-cell scan (one-shot `attached`); both are linear in
placed blocks with small constants, as is every other family (signal ball included). The
phase-1 verdict stands, now by census instead of sampling: exceeding 1,000,000 counted
calls in one cascade needs on the order of 10^5-10^6 placed blocks through the linear
families (dust's own ball caps at ~31k counted no matter the build), or the two named
impractical exceptions (≥ ~2,174 dry sponges in ~140k water; ~125k frosted-ice cells).

## Next highest-value steps (not done)

1. Sponge threshold hunt in the bridge sandbox (fill-built sponge/water volume, watch for
   the game's "Too many chained neighbor updates" log): would turn the sponge bound from
   arithmetic into a measurement, at sandbox-chunk sizes well past survival sense.
2. A `circuit:` kind for the judge rig if this becomes a Redstonepedia study (the
   circuit study's archive wants kinds + briefs; the brief is already in that shape).
3. Model sponge/frosted ice in redsim only if (1) says they matter.
