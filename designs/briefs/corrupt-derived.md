# Derived and saved world-data that can disagree with the visible world

corrupt-derived track (lane 5 of 5 corruption audits), 2026-11-07. Gate G1 (docs/process.md).
Asked by vjeux: find survival-reachable ways for derived or side-recorded data to disagree
usefully with the visible world in Minecraft Java 26.3 — heightmaps, block/sky light,
structure starts/references and piece state, feature/loot generation state, trial-vault
rewarded players, inhabited time/local difficulty, chunk tickets/ticking level,
raid/map/saved-data records, biome/noise caches — ranking effects on mob spawning, light
behavior, structure/loot renewal, farm control, chunk loading, and selective freezing. The
work, the evidence and every tool are in `designs/corrupt-derived/NOTES.md`; this brief is
what the audit and its probes rest on. It does not duplicate the chunk-persistence lane's
corruption triggers: every mechanism below is a normal code path that fails to recompute,
invalidate, or persist a record consistently.

## Use

A technical survival player makes the game's *records about the world* say something the
visible world does not: a volume stays spawn-proof after its lamps are gone (or spawns mobs
while visibly lit), a farm chunk's local difficulty resets despite hours of presence. Each
finding is a place where a derived record (a heightmap column, a light layer, a counter)
is mutated, queued, or saved on a path that can lose, skip, or freeze the update — while
the blocks themselves end up exactly where survival play put them.

## Target user

A survival player on a default 26.3 world (singleplayer or a default-properties server).
No commands, no datapacks, no server-property changes in the *sequences*; the probe harness
uses RCON only to set up and read back disposable test worlds, and every game-visible step
it performs has a survival equivalent named next to it.

## The numbers (how findings are ranked)

1. **Effect size**: how much gameplay the disagreement controls — spawn yes/no per cell,
   light level error (0-15) over how many cells, local-difficulty points, loot re-rolls.
2. **Reliability**: does the sequence work every time at any position, or does it need a
   race window; how many blocks/ticks of setup.

Reported with them: persistence (survives save/load? forever, or until the next poke?),
what heals it, and the risk (lag, world damage).

## Mechanics, from the game code (26.3, `mctas src show <Class>`)

Full audit trail with every citation: `designs/corrupt-derived/NOTES.md`. The load-bearing
facts, by record:

**Heightmaps** — one writer, trusted forever after. `LevelChunk.setBlockState` (284-287)
updates all four live maps (MOTION_BLOCKING, MOTION_BLOCKING_NO_LEAVES, OCEAN_FLOOR,
WORLD_SURFACE) on every block change; `Heightmap.update` (82-109) only ever revises the
column top (early return below `firstAvailable - 2`), so a wrong top is only repaired when
the phantom top cell itself changes. Saving clones the raw arrays
(`SerializableChunkData` 361-366); loading installs them verbatim and primes only the types
that are missing (283-291; `Heightmap.setRawData` 127-135 checks only the array length).
Nothing anywhere compares a loaded heightmap against the blocks. In live play every block
write routes through that one method (the neighbor-update cap cannot skip it: it runs
inside `setBlockState`, not in a neighbor cascade), and the one direct section writer,
`OreFeature` via `BulkSectionAccess` (OreFeature 148-159), replaces blocks with
predicate-equivalent ones (stone→ore), so vanilla survival has no impactful way to make a
heightmap wrong — but any out-of-band error (the chunk-persistence lane's triggers) is
preserved by save/load indefinitely. Consumers that would read the lie: natural spawning's
Y distribution (`NaturalSpawner.getRandomPosWithin` 334-341), precipitation/lightning,
beacon beams (`BeaconBlockEntity` 137), maps (`MapItem` 138), portal placement
(`PortalForcer` 64), raid/siege/trader spawn heights, player spawn search.

**Light** — derived on a side thread, snapshotted without draining, trusted on load.
Block changes queue light work (`LevelChunk.setBlockState` 294-301 gated by
`LightEngine.hasDifferentLightProperties`, `LightEngine` 41-48 — audited sound) into
`ThreadedLevelLightEngine`: `checkBlock` is a queued task (59-67), applied in batches of
1000 (33, 191-213). Saving copies whatever DataLayers the engine holds at that moment
(`SerializableChunkData.copyOf` 333-344 via `ChunkMap.save` 745-777) — no path waits for
pending light work first (`waitForPendingTasks` 216-218 is only a *send-to-player*
dependency, `ChunkMap` 1286). Unload saves, then wipes the engine's data for the chunk
(`ChunkMap.scheduleUnload` 526-532; the wipe `updateChunkStatus`,
`ThreadedLevelLightEngine` 69-83, is submitted at queue level 0, ahead of `checkBlock`
tasks submitted at ticket level): light tasks still queued at that point apply to nothing.
Recovery exists on the happy path — when light does apply, `ChunkHolder.sectionLightChanged`
(142-149) re-marks the chunk unsaved — so the hole is exactly *save before apply, wipe
before apply*: an edit whose light backlog (≥1000 tasks per batch-tick of lag) outlives the
chunk's unload. On load, `isLightOn` + stored layers are trusted: the LIGHT status task
treats the chunk as already lit (`ChunkStatusTasks` 35) and `lightChunk(lighted=true)`
propagates nothing (164-180); `SkyLightEngine.setLightEnabled` (284-301) only fills
*empty* layers above the recomputed sky sources with 15. Sky *sources*
(`ChunkSkyLightSources`) are not saved and are recomputed from blocks on load, so sky
heals on the next poke; the stored sky/block *layers* do not recompute. Clean shutdown is
safe (it drains: `MinecraftServer.stopServer` 649-660 loops while `ChunkMap.hasWork`,
which includes light work, 460-469). The classic chunk-border variant mostly does *not*
exist: light sections stay live ~2 chunks beyond any chunk in which survival can change a
block (ticket-level geometry: `ChunkLevel` 10-15, 38-55; player tickets
`DistanceManager` 36, 115, 131-133, 269), so a border edit always propagates into a stored
neighbor — except during the approach window while the neighbor is still scheduled below
INITIALIZE_LIGHT.

**Inhabited time** — mutated every tick, never dirtying its chunk.
`ServerChunkCache.tickSpawningChunk` (404-406) calls `ChunkAccess.incrementInhabitedTime`
(370-372), which does not `markUnsaved`. Every save path requires the unsaved flag
(`ChunkMap.saveChunkIfNeeded` 723-725, `save` 747 `tryMarkSaved`, and even the shutdown
flush filters through the same `save`, 413-437). Increments therefore reach disk only
piggybacked on an unrelated dirtying (a block write, `LevelChunk.setBlockState` 348; a
block-entity `setChanged`, `Level.blockEntityChanged` 876-880 — scheduling a block/fluid
tick does *not* dirty either, the same family the vanilla harness already works around,
`crates/mctas/src/vanilla/mod.rs` 436-439). An inert chunk (a sky farm's spawning chunk:
no random-tick block changes, no block entities changing) accumulates inhabited time only
in memory; unloading rolls it back to the last dirtied save. The value feeds local
difficulty (`ServerLevel.getCurrentDifficultyAt` 692-702; local scale caps at 3,600,000
ticks = 50 h, `DifficultyInstance` 10, 53): mob equipment, spider effects, zombie
reinforcements and leaders.

**Structures** — consistent by construction; permanent by design. Starts live in the start
chunk, references in every chunk the bounding box intersects, both created under
generation-step ordering (`ChunkGenerator.createStructures` 505-592,
`createReferences` 638-670 radius 8, `ChunkPyramid` 11-34), saved/restored verbatim
(`SerializableChunkData` 292-293, 372), never invalidated afterward: demolishing a
fortress leaves its spawn overrides (`ChunkGenerator.getMobsAt` 478-503) working forever.
That permanence is the intended record, not a failure; the only inconsistencies are
truncations — pieces outside the ±8-chunk reference radius are neither placed nor
spawn-relevant, and worldgen never writes into an already-full neighbor
(`ImposterProtoChunk` is always created with `allowWrites=false` and its structure writes
are no-ops, 61-63, 129-148). Baseline for the ranking, not a finding.

**Loot generation state** — one non-dirtying corner. `RandomizableContainer.unpackLootTable`
(72-90) clears the table, then `LootTable.fill` (145-164) inserts items; chunk dirtying
rides on `setItem`/`setChanged` per inserted item. A roll that produces *zero* items
inserts nothing and dirties nothing: the cleared table exists only in memory, and the next
load rolls again — deterministically empty again, since generation always seeds
(`RandomizableContainer.setBlockEntityLootTable` 42-48; `StructurePiece` 466, 489), so
in vanilla this is a permanently "unlooted" empty container, not a renewal. (With seed 0
— a template NBT without `LootTableSeed`, not produced by vanilla structure code — the
re-roll would be random each load.)

**Trial vaults** — audited sound. Rewarded players live in the block entity's
`VaultServerData` (codec 20-35), capped at 128 UUIDs with FIFO eviction (66-77: the
129th rewarded player makes the 1st eligible again — by design, multiplayer-only), and
every mutation marks the data dirty, flushed to `setChanged` in the block entity's server
tick (`VaultBlockEntity` 250-258). Mid-ejection state (`itemsToEject`,
`stateUpdatingResumesAt`) is persisted and resumes after unload.

**Tickets / ticking level** — audited sound. Ticket levels are recomputed live from
`TicketStorage` by two trackers fed by the same listeners; only `persist` types
(`FORCED`, `PORTAL`; `TicketType` 17-25) are saved (`TicketStorage.packTickets` 71-79),
with `ticks_left` (`Ticket` 11-18). Timed tickets are self-refreshing while their owner
ticks (ender pearls: `ServerPlayer` 2305-2319, timeout 40; portals: refreshed per travel,
timeout 300) and expiry is deliberately paused while the target chunk is not ready for
saving (`TicketStorage.canTicketExpire` 291-302). Deduplication by (type, level)
(150-156) is safe because the only explicitly removed types have a single owner. The
useful chunk loading here (pearl stasis, portal loaders) is the designed behavior.

**Saved data / raids / maps / biomes** — audited sound. All `SavedData` mutators checked
call `setDirty` (`Raids` 75-96, 143; `RandomSequences` marks dirty on *every draw*,
97-162; `MapIndex` 28-32; `WanderingTraderData` 35-51; `TicketStorage` throughout), and
storage writes exactly the dirty set (`SavedDataStorage` 188-196). A raid record outlives
its village only until its own inactivity rules stop it, and only ticks while its center
is loaded — intended. Map colors/markers are a snapshot record by design (and a heightmap
consumer, above). Chunk biomes are a write-once store read from the sections themselves
(`ChunkAccess.getNoiseBiome` 417-430), filled only during generation (432-443); noise
state is derived from the seed and never persisted, so there is no second copy to
disagree with.

## Candidate mechanisms (the "layouts")

| # | mechanism | effect | reliability | persistence | status |
|---|---|---|---|---|---|
| L1 | **Light backlog outlives unload**: a mass light edit (>1000 queued `checkBlock`s per tick of unload lag), then leave (portal/dimension) — unload snapshots partial light, the wipe discards the rest | phantom light or phantom darkness over the edited volume: spawn control with no visible source (or spawns in a lit build), light frozen selectively per chunk | needs a light-lag machine (piston-toggled glowstone/opaque field) or one huge edit + immediate exit; no per-position variance | forever (stored layers + `isLightOn` trusted), until a light-differing block changes within propagation range | verify (P1) |
| L2 | **Inhabited-time rollback**: keep a spawning chunk inert (no block/block-entity changes), AFK in range, leave | local difficulty resets toward its last dirtied save (up to 50 h of growth erased): fewer armored/equipped mobs at a farm, on demand | every time, any position; setup is one inert chunk | the rollback is the persisted state; growth restarts from the rolled-back value | verify (P2) |
| L3 | **Approach-window border light**: edit chunk A in the ticks after it starts ticking while neighbor B is still scheduled below INITIALIZE_LIGHT | like L1 but only B's border strip (≤15 blocks) | race against chunk-loading speed; widened by fresh generation under load and by chunk-load-triggered machines | as L1 | measure only (P3) |
| L4 | **Heightmap amplifier**: any out-of-band block change becomes a permanent heightmap lie (no vanilla live creator found; `OreFeature`'s direct writes are predicate-equivalent) | spawn-Y distribution, precipitation, beacons, maps, portal height read the lie | n/a in pure vanilla; combines with the chunk-persistence lane | forever (load trusts, update only fixes the phantom cell) | scan (P4 expects 0) |
| L5 | **Obvious baseline (by design)**: structure bounding boxes after demolition; pearl/portal chunk loaders; vault 128-player FIFO | fortress/bastion spawns with no structure standing; chunk loading; vault renewal across 129 players | always | forever | documented, not corruption |

## Decision

Verify L1 and L2 with deterministic probes on the official server (P1/P2 below), measure
L3's window if P1's harness makes it cheap, and run P4 as the negative control for
heightmaps. Everything else is reported from the source audit with its citations. If P1's
backlog timing does not reproduce on the harness's hardware, L1 falls back to the analytic
claim with the measured batch numbers — the brief's ranking does not change.

## Must-haves

- Default world only: no `max-chained-neighbor-updates` edits, no `redstone_experiments`,
  default view/simulation distances in the *sequences* (the harness's low distances only
  shrink the test world).
- Every survival sequence names each step's in-game action; probe commands (`fill`,
  `tp`, `save-all`) are setup/measurement only and are labeled as such.
- Persistence claims are proven across a real save/load cycle (server restart on a copied
  world), reading the region files — never from in-memory state.
- Controls next to every subject: same setup with the one difference that should make the
  record correct (let light drain before leaving; dirty the inhabited chunk once).

## Not goals

- The chunk-persistence lane's corruption triggers (crashes, torn writes, external edits):
  L4 only notes that this lane's records amplify them.
- POI and entity-storage staleness (the poi-entity lane), block-entity internal state
  (the block-entity lane), event/tick suppression (the suppressor lanes).
- Beating L5's by-design behaviors; they are the bar a corruption must exceed to matter.

## The probes (`mctas corrupt …`, official server, disposable worlds)

- **P1 `corrupt light`**: void world, one chunk. Fill a glowstone volume (thousands of
  `checkBlock`s), then (subject) teleport the bot 3000 blocks away within a tick and let
  the chunk unload, or (control) wait for the light to settle first. Stop, read the saved
  `BlockLight` layers from the region file, restart on the saved world, load the chunk,
  re-read. PASS = subject's saved light is measurably incomplete vs control (nibble sums /
  lit-cell counts printed), and stays incomplete after reload until a poke. Markers are
  the region-file nibble statistics at named coordinates.
- **P2 `corrupt inhabited`**: bot stands where two inert neighbor chunks are in spawning
  range; run N ticks; dirty only the control chunk (one `setblock`); teleport far, unload,
  stop. Read `InhabitedTime` of both chunks from the region file. PASS = control grew by
  ~N, subject stayed at its first-save value.
- **P3 `corrupt border`** (measure): approach a lit/dark border rig at several distances,
  break the border light source by bot hand the tick the near chunk loads, and report
  whether the far chunk's saved light followed. Prints the window; no PASS/FAIL.
- **P4 `corrupt audit`**: for every chunk in a saved world, recompute WORLD_SURFACE /
  MOTION_BLOCKING from the saved blocks and diff against the saved `Heightmaps`; print
  mismatch counts (expect 0 on a vanilla-played world — the negative control for L4).

## Outcome (2026-11-07, after probes — details in NOTES "Probe results")

- **L2 inhabited-time rollback: CONFIRMED** (P2: control +1200 ticks banked, inert
  subject +4 after the same 1200 spawning ticks and an unload). This is the lane's
  headline finding: deterministic, survival-only, persists.
- **L1/L3 light: fences hold.** 221k queued light tasks were fully applied before even
  an immediate `save-all` snapshot (batches run inline during submission; applied light
  re-dirties; unload latency ≫ drain; the generation pyramid guarantees
  light-initialized neighbors around every editable chunk; shutdown drains). The
  code-level gaps stand as documented, with only an engineered same-tick
  machine-phase + synchronized-departure race left open.
- **L4 heightmap amplifier: no vanilla creator** (source audit + P4: 450 chunks,
  0 mismatches). Its value is cross-lane: load trusts stored heightmaps forever.
- Revised ranking: inhabited rollback > heightmap load-trust > light ordering gaps
  (fenced) > empty-roll loot > by-design baselines.
