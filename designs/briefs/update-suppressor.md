# Suppressing one neighbor update on demand

update-suppressor track, 2026-11-07. Gate G1 (docs/process.md). Asked by vjeux: find the
smallest practical survival-buildable mechanism that reliably suppresses one specified
neighbor update to one target cell (Minecraft Java 26.3), with bounded lag, no crash, a clear
trigger and reset, robust across directions, positions, mirrorings and chunk borders. The work,
the evidence and every tool are in `designs/update-suppressor/NOTES.md`; this brief is what the
study's judge rests on. The probe and judge have run: the layouts/G2/ceiling below carry the
measured numbers (the audit estimates they replaced are in NOTES). Headline: the suppressor
rig works and is proven at a lowered cap on the official server, and no survival build's
single trigger comes within a factor ~30 of the shipped 1,000,000 cap - the practical
mechanism remains never sending the update (layout 1).

## Use

A technical survival player makes one block *not hear* one specific change next to it, on
demand: the changing block changes, the target keeps its stale state (a BUD piston stays
retracted under quasi-power, a rail keeps a shape its neighbours no longer justify, ...),
until a later ordinary update wakes it. Every setup that needs a stale state built next to a
live circuit needs exactly this one swallowed update.

## Target user

A survival player on a default 26.3 world (singleplayer or a default-properties server) who
can build the machine block by block, flip one lever to suppress, flip it back to reset, and
poke the target once to wake it. No commands, no datapacks, no server-property changes.

## The numbers

1. **Blocks placed** (the whole build: flood, supports, trigger, middle block, target rig).
2. **The suppressing tick's cost**: counted neighbor-updater calls in the suppressing cascade
   and the tick's wall time on the official server (the freeze a player feels).

Reported with them: footprint (bounding box), robustness (positions incl. chunk borders x 4
rotations x mirror), reset quality (suppress again after one lever cycle, wake with one
ordinary update), and which update is suppressed (the middle block, the target, the edge).

## Mechanics, from the game code (26.3, `mctas src show <Class>`)

**One counter guards every neighbor notification and shape update.**
- `CollectingNeighborUpdater.addAndRun` (68-85) is the only door: every `neighborChanged`,
  every `updateNeighborsAt(ExceptFromFacing)` and every `shapeUpdate` increments `count` once
  *per call added* (one multi-neighbour call counts once and later delivers up to 6
  `handleNeighborChanged` executions, `MultiNeighborUpdate.runNext` 148-169, in
  `NeighborUpdater.UPDATE_ORDER` = WEST, EAST, DOWN, UP, NORTH, SOUTH).
- The call is **dropped when it is added** while `count >= maxChainedNeighborUpdates` (70-80):
  one error is logged ("Too many chained neighbor updates. Skipping the rest. First skipped
  position: ...") and the rest are silently skipped. Entries already queued always execute.
- The cap is **1,000,000**: `SharedConstants.MAX_CHAINED_NEIGHBOR_UPDATES` (135),
  `DedicatedServerProperties.max-chained-neighbor-updates` default 1000000 (100),
  `MinecraftServer.getMaxChainedNeighborUpdates` 1000000 (2033-2035, singleplayer), wired into
  each level at `ServerLevel` (239) -> `Level` (142-151).
- `count` is monotonic inside one outermost cascade and resets only when that cascade fully
  drains (`runUpdates` 87-112, the `finally`). Execution is depth-first: updates added by a
  handler run before the entry that was interrupted continues (89-105).

**What one `setBlock` emits** (`Level.setBlock` 216-262): with flag 1, one
`updateNeighborsAt` (245, one counted call); then, unless flag 16 or the limit ran out, shape
updates to the 6 neighbours (`BlockBehaviour.updateNeighbourShapes` 740-747, shape order
W,E,N,S,D,U) plus indirect shapes of the old and new state (251-255; only redstone wire
overrides those, `RedstoneWireBlock` 204-227). A `setBlock` with the state already present is a
no-op (`LevelChunk.setBlockState` 279-281). Shape chains have their own depth budget,
`Block.UPDATE_LIMIT` = 512 (106), decremented per `setBlock` hop (253-255) - a second, separate
limit that only stops *shape* propagation.
`onPlace` runs on every actual state change (`LevelChunk` 325-327), and a rail state change
also runs the old state's `affectNeighborsAfterRemoval` (317-319, `BaseRailBlock` 117-128:
two more neighbour calls for straight rails).

**What suppression therefore requires.** The protected notification must be *added* after the
millionth call of the same cascade. So the changing block **B** next to the target **T** must
change *inside* the cascade, late: B's own `setBlock` then tries `updateNeighborsAt(B)` at
`count >= 1,000,000` and that call is dropped - B has changed, T never hears it. Two corollaries:
(1) T must not be reachable by any entry queued before the cap (e.g. T must not be a direct
neighbour of the trigger: the trigger's multi-neighbour entry is queued first and always
delivers); (2) the flood and B's change must share one cascade - the trigger's first
multi-neighbour entry delivers WEST first, so put the flood head on the trigger's west side
and B on its south (last) side: the flood runs to completion (depth-first) before the entry
reaches B.

**What can never be suppressed this way.** Scheduled block ticks
(`ServerLevel.tick` 394, own cap 65536 per tick), fluid ticks (396), block events (409,
`runBlockEvents` 1267-: FIFO, drained whole) and block-entity ticks (459) are separate phases;
each handler there starts a fresh cascade with `count = 0`. A repeater, observer, torch or
piston that acts through those phases cannot be starved by a flood in another cascade. The
suppressor swallows *neighbor notifications and shape updates added late in one cascade* -
nothing else. (A lever flip is three cascades back to back: `LeverBlock.pull` 82-88 =
`setBlockAndUpdate`, then two `updateNeighborsAt` (138-139); the rig must live inside the
first and be a no-op in the other two.)

**Why the flood is big: every instant amplifier converges.** Inside one cascade the only
instant signal logic is monotone: dust (`DefaultRedstoneWireEvaluator.updatePowerStrength`
18-38: on a power change, one `setBlock` flag 2 - 6 shape calls in a flat field, indirect
shapes only across a step - plus `updateNeighborsAt` of itself and its 6 neighbours in
`HashSet` order: **13 counted calls per wire flip**, power one of 16 values moving one way on
one source edge), the mirror blocks (note block `NoteBlock.neighborChanged` 81-92, lamp,
bulb's powered bit, dropper's triggered bit, doors/gates: one flip per signal edge),
powered/activator rails (`PoweredRailBlock.updateState` 121-133: one powered flip per edge,
~11 counted calls with the chunk's rail removal calls, and a `findPoweredRailSignal` scan of
up to 8+8 rails per execution - the CPU-heaviest flip), plain rails at a 3-way junction
(`RailBlock.updateState` 22-26 -> `RailState.place` 218-347: shape depends on power, neighbours
rewritten by `connectTo` 143-206 - the one amplifier whose two shape rules could in principle
disagree and flip a rail twice; the probe tests it). Everything else schedules a tick
(observer, repeater/comparator, torch, scaffolding `ScaffoldingBlock` 88-122, leaves
`LeavesBlock` 86-106, falling blocks) or queues a block event (pistons) and so ends its branch
of the cascade. Fences/walls/stairs shapes are functions of neighbour *presence*, static
during a cascade: they do not propagate past the changed cell.
No block issues neighbour calls unconditionally from `neighborChanged` (grep of every
`updateNeighborsAt` / `level.neighborChanged` caller, NOTES), so no small oscillator is known
in 26.3: total counted calls in one cascade <= (blocks) x (flips per block) x (calls per
flip), and the build's size follows.

**The target rig (also the control).** T is a piston under constant quasi-power
(`PistonBaseBlock.getNeighborSignal`: a redstone block diagonally above powers it; it only
re-checks on a neighbour update, mechanics.md section 2): at rest retracted, any neighbour
update extends it. B, the middle block south of the lever and next to T, is a note block: on
the lever's edge it flips `powered` and its `setBlockAndUpdate` is the notification under
test. Control = the same rig with an empty flood (B's call lands, T extends). Suppressed =
B flipped, T still retracted after the horizon. Wake = one block placed and removed next to T
(T extends). Reset = lever off (B flips back; that edge's notification to T is likewise
suppressed while the flood is connected - T never heard either edge), lever on suppresses
again.

## Trade-offs

- **Size against the cap.** The cap is fixed at 1,000,000 counted calls; a generator family's
  calls-per-placed-block is the whole game. Supports count: a dust needs a sturdy top under it
  (`RedstoneWireBlock.canSurviveOn` 260-262), a rail a rigid block (`BaseRailBlock.canSurvive`
  55-57) - a support that itself flips (a note block under dust) earns its cell back.
- **Lag is structural.** Passing the cap means >= 1,000,000 added calls and every queued entry
  executed: the suppressing tick freezes for as long as ~1-6M handler executions take
  (measured, not assumed). Bounded and crash-free is the bar; fast is impossible. Families
  differ by execution cost (rail signal scans vs dust recomputes).
- **One edge or every edge.** A dust/rail field floods on power-up and on power-down (different
  counts - both measured); B sits in the same cascade either way, so both lever edges are
  suppressed. Selective suppression of one edge only would need the flood disconnected between
  edges (a second lever): not a goal.
- **The silent-feed baseline.** Where the player controls the source, never sending the update
  (quasi power through a repeater-fed conductor: silent hold, mechanics.md section 4) costs
  ~3 blocks and no lag. The suppressor is only for updates that *will* be sent by a change
  that must happen next to T.

## Five layouts (measured with `suppress probe`, 2026-11-07)

| # | layout | measured | verdict |
|---|---|---|---|
| 1 | **Obvious: don't send it.** Feed/hold T silently (QC through a repeater-fed conductor, or move B's change one cell away and update T deliberately) | ~3 blocks; proven in the game already (gadgets `silent-hold`, `qc-bud`, mechanics.md sections 4/6) | **the pick wherever it applies** - but it cannot swallow an update a change next to T *does* send |
| 2 | **Flat dust field** on solid supports, one lever at the center | counted **7,385 on / 19,345 off at 31x31 and identical at 45 and 63**: exactly the radius-15 diamond (481 wires) flips, the rest of the field is decoration. 1.9% of the cap | dead: one trigger's signal reaches 15 wire-steps and no farther, whatever is built |
| 3 | **Stacked dust fields** (layers joined by central staircases; the `updateIndirectNeighbourShapes` stepped lattice) | 5 layers of 31x31 (9,606 blocks): **30,571 counted off, 874 cells flipped**; layers past the third add ~nothing (climbing spends the 15-step budget). 3.1% of the cap | dead, same bound |
| 4 | **Powered-rail field** | not field-probed: a rail's `shouldPower` reaches 8 track steps to a rail with neighbour signal (`findPoweredRailSignal` 23-96) and that signal comes from dust - the same 15-step radius caps the flippable region, at ~11 counted calls per rail flip | dead, same bound |
| 5 | **Plain-rail junction mesh** (3-way junctions whose curve flips with power; `place`/`connectTo` rewrite neighbours) | 9 patterns x kinds x flat/slope geometries x lever/place/break triggers, sizes 12-64, plus 600 random patches (two triggers each): **best cascade 40 counted calls, size-independent**. On a survival-settled mesh `place` recomputes the shape already there and nothing propagates past the touched junction | dead: no wave, no oscillation in 26.3 |

Also considered and set aside from the source: a *shape-depth* suppressor (make a shape change
travel > 512 hops so `updateLimit` starves it) - shape changes of fences/walls/wires are
presence functions and do not travel cell to cell, and the judge's target reacts to neighbour
notifications, which the 512 budget never gates. Scaffolding/leaves distance fields: their
recompute is a scheduled tick per cell, a fresh cascade each - they cannot flood one cascade.
Two theoretical exceptions, both outside survival-practical (NOTES has the arithmetic):
**sponges** (`SpongeBlock.neighborChanged` absorbs up to 64 water cells with direct
`setBlockAndUpdate`s inside the running cascade, ~460 counted calls per sponge, chaining
through shared water: the cap needs >= ~2,174 dry sponges, one-shot, reset by smelting) and
**frosted ice** (`FrostedIceBlock.neighborChanged` chain-melts a sparse line at ~8 counted
calls per cell: 125,000 cells of a block that cannot be obtained as an item, is only made
in place by Frost Walker trails, and random-tick ages into spontaneous melts).

## Decision

**There is no survival-practical cascade-flood suppressor in 26.3 at the shipped cap.**
The mechanism itself works and is fully understood: the judge rig below suppresses cleanly
whenever the cascade can reach the cap (proven on redsim at a lowered cap and on the
official server with the stock `max-chained-neighbor-updates` property lowered - same rig,
same counts). What does not exist is a survival build whose one trigger adds 1,000,000
counted calls in one cascade: every instant amplifier in the game either converges inside
the trigger's 15-step signal radius (dust, mirrors, powered rails), is a fixed point on any
survival-settled build (rail shapes, fences), or exits into a later phase (scheduled ticks,
block events). The practical answer to the use stays layout 1 - never send the update -
with this study as the evidence for why the flooded kind is not on the table. The judge and
tools stay: any future mechanic that *can* flood drops into the same rig.

## Must-haves

- Default world: cap 1,000,000 as shipped (no `max-chained-neighbor-updates` edit, no
  `redstone_experiments` flag - the experimental evaluator counts differently,
  `ExperimentalRedstoneWireEvaluator`).
- Control and suppressor are the same rig but for the flood's presence; the control reacts.
- The suppressed target stays stale for the judge's horizon (10 gt after the edge, settled
  idle), and one later ordinary neighbour update wakes it.
- Bounded: the cascade ends, the level is idle after, counted calls and the dropped-call count
  are reported; no crash, no runaway across ticks (the flood does not re-trigger itself).
- Works at every judge position (incl. straddling chunk borders at x/z mod 16 in {15,0,1}),
  in 4 rotations and mirrored; reset by the lever and suppresses a second time.
- Survival: every block survival-obtainable and placeable in an order that settles to the
  judged rest state (flood unpowered while built; quasi source placed where its placement
  updates cannot reach T; lever last, off).

## Not goals

- Suppressing scheduled ticks, block events or block-entity ticks (separate phases, above).
- Beating the monotone floor below with a trick the source does not contain; if no oscillator
  probes out, the honest result is the smallest field plus the floor argument.
- Low lag, tileability, or suppressing more than the one specified notification.
- Experimental-redstone worlds, changed server properties, unloaded-chunk behaviour.

## The judge (`mctas suppress judge`, redsim; `mctas suppress bridge` for the game)

- The tool *generates* the rig from parameters (family, field size): a design file of the
  winning size is emitted for the record, but judging runs the generated blocks directly.
  Ports, by role: `lever` (trigger), `middle` (B, the note block whose change is under test),
  `target` (T, the quasi-powered piston), `wake` (the cell next to T used for the wake poke).
- Per variant (positions x rotations x mirror, 2 trigger cycles): place like the door judge
  (raw writes, a self update of every block), settle to idle; assert T retracted and
  quasi-powered, B unpowered; flip the lever on; run 10 gt and settle; assert B powered,
  the suppressing cascade's counted calls >= 1,000,000 and at least one call dropped, and
  T's state per the case (control: extended; suppressed: still retracted). In the suppressed
  case two more edges follow before the wake - lever off, lever on again, each settled 10 gt:
  both must also suppress (B flips each time, T still retracted; an under-sized flood on the
  power-down edge would let the off edge through and extend T here). Then the wake poke (place
  and remove a stone at `wake`), 10 gt: T extended.
- Metrics printed per variant: blocks, footprint, counted calls and executions of the
  suppressing cascade, flips per flood cell (max/mean), dropped calls, redsim wall time.
- Bridge judges the same script in the real game (one worker, isolated universe) at the
  game's own cap: identical outcomes to redsim, and the lever call's wall time is the
  cascade's game cost. `mctas suppress vanilla` runs it on the official dedicated server
  (placement by `fill`/`setblock` of the same generated blocks - a stand-in for hand
  placement at field sizes; the lever flip is the bot's real click between stepped ticks),
  at a `--cap` written to `server.properties`: the geometry proof at lowered caps and the
  no-suppression result at the shipped default.

## The obvious design, measured (G2)

Layout 1 is the bar a player has today (silent hold: ~3 blocks, proven in the game by the
gadget library, vanilla PASS in mechanics.md section 11) and it does not do the stated job.
The first *working suppressor rig* is this study's judge rig (a lever on a note block over
a 31x31 dust field, the middle note block over the lever, a quasi-powered piston beside it;
`designs/update-suppressor/rig-flood-31.txt`, 1,927 blocks with the field), measured:

- redsim, cap lowered to 5,000 (`World.nu_cap`): **96/96 variants pass** (6 positions incl.
  chunk borders x 4 target arms x mirror, control + suppress each; suppress counts
  5,475-5,748 attempts, target stale until the wake).
- redsim, the game's cap: 48/48 controls pass, **0/48 suppress** (the rig's cascade attempts
  6,580 counted calls; the target reacts like the control).
- the bridge (real game, own cap): identical outcomes at all 4 arms; the lever-pull cascade
  takes 18-38 ms of game time at 6.6k counted calls.
- the official dedicated server, bot-clicked lever: at `max-chained-neighbor-updates` 5000
  and 6000 the flood **suppresses** (middle flipped, target stale, wake extends it); at 7000
  and at the shipped 1,000,000 it does not. The real-game threshold brackets **(6500, 6600]**
  against redsim's 6,580 counted attempts for the identical cascade - the model and the
  game agree to ~1%, and the rig's geometry (starter below the lever before the middle
  above it in `UPDATE_ORDER`, target outside dust's two-step reach) is what a working
  suppressor needs, at 152x the flood this build has.

## Ceiling

- **Counted calls: exactly 1,000,000** must be added in one cascade before B's call - the
  game's own number, no slack either way.
- **What one cascade can reach (the bound the probes measured against).** A wire on one
  source edge flips monotonically (no wire's power ever rises during a power-down from a
  converged rest, by induction on the first riser; NOTES), so flips per wire <= its power,
  and only wires within 15 wire-steps of the changed signal can flip at all. Even granting
  every cell of that step-ball as a wire (14,911 cells), the flip-weighted sum is 50,625
  flips; at the most a flip can count (7 neighbour calls + 6 shape calls + 16 indirect
  shape calls across a step = 29, `DefaultRedstoneWireEvaluator` 24-36,
  `RedstoneWireBlock` 204-227, `Level.setBlock` 251-255) that is 1.47M - and halving it for
  the support every wire needs (at most every second cell of a column can be a wire) puts
  the unreachable-perfect-packing bound at **~725k < 1,000,000**. Supportable lattices fall
  far shorter (a conductor directly over a wire blocks its step reads, so layers is what
  survives): measured best 30,571 (layout 3). Mirrors add one flip per cell in the same
  region (~7 counted calls each as note-block supports), rails add nothing propagating
  (layout 5), and the only in-cascade multi-block loops in the game are the sponge's
  64-cell BFS and the frosted-ice melt chain - the two impractical exceptions above.
