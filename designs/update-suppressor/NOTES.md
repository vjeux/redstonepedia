# Update suppressor - notes

Track: explore/update-suppressor, based on explore/bedrock 7c50171 (origin/main does not
contain the bedrock work yet; verified `merge-base --is-ancestor` = NO). Brief:
`designs/briefs/update-suppressor.md` (read it first - mechanics, layouts, judge).
All commands run from the repo root of this clone; the game binary used for source reads
before this clone's first build was the parent's `~/agents/bedrock/minecraft-tas/target/release/mctas`
(read-only `src` commands; the parent clone is never edited).

## Tools

(to fill as built: `mctas suppress probe|judge|bridge|emit`.)

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

## Probe results

(to fill.)

## Judge / bridge / official-server results

(to fill.)
