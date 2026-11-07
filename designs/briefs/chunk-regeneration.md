# Can a survival player force one fully generated chunk to regenerate? (26.3)

chunk-regen track, 2026-11-07. Gate G1 (docs/process.md), adapted to a research question:
there is no component to publish unless a trigger meets the success bar below, and then it is
published as a *finding with a mitigation*, never as a build to use on a shared world. Asked:
investigate whether Minecraft Java 26.3 has survival-reachable triggers, other than oversized
written-book NBT, that can make a fully generated chunk be discarded, corrupted, rolled back,
or regenerated. The owner's book mention is a hypothesis to test, not a fact. All work is on
disposable local worlds and server processes started for this study; no public, shared,
third-party or persistent survival world is ever touched. Evidence and tools:
`designs/chunk-regen/NOTES.md`.

## Use

One sentence a player would use to judge it: *know whether anything you can build or do in
survival can silently throw away one finished chunk — and if it can, exactly what to avoid,
how to spot it in a log, and how to recover from a backup.* The deliverable is that knowledge,
ranked by evidence. A negative result (every stage blocks it, with the blocking stage named)
is a complete answer.

## The numbers (what every candidate is scored on)

1. **Outcome class** (exactly one, measured on disk, never inferred from a crash alone):
   `normal` (saved chunk round-trips), `rollback` (an earlier saved version of the same
   chunk comes back), `chunk-ban` (chunk saves and loads on the server but a client cannot
   stay in it), `entity-only` / `poi-only` (that stream alone is lost or desynced, terrain
   intact), `regen` (the terrain chunk is absent or unreadable on the next load, so
   generation runs from `empty` and player changes in it are gone), `region-wide` (more
   than the target chunk changes), `crash-only` (the server dies, the saved chunk is
   unchanged). A crash alone is not a success.
2. **Blast radius**: chunks changed besides the target (must be 0 for a claim), and whether
   the entity and POI streams of the target chunk agree with its terrain afterwards.

Reported with them: reproducibility (n/n on fresh disposable worlds), the survival
build/interaction sequence (no commands, no filesystem edits after world creation, no
killing the host), resource/lag cost in game ticks and wall time, and recovery.

## Mechanics, from the game code (26.3, `mctas src show <Class>`; full audit in NOTES)

**The lifecycle.** A chunk is a `ProtoChunk` promoted through `ChunkStatus`
(`empty → structure_starts → structure_references → biomes → terrain → features →
initialize_light → light → spawn → full`, `world/level/chunk/status/ChunkStatus.java:21-30`)
by `ChunkPyramid.GENERATION_PYRAMID` steps (`ChunkStatusTasks`), or a `LevelChunk` once
`full`. Ticket levels (`server/level/ChunkLevel.java`: FULL 33, BLOCK_TICKING 32,
ENTITY_TICKING 31, `MAX_LEVEL = 33 + radius-around-full`) are the min over tickets
(`TicketType`: player, portal 300 gt, ender pearl 40 gt, forced, …) in
`ChunkMap.DistanceManager`; the ticket level decides the highest allowed status
(`GenerationChunkHolder.updateHighestAllowedStatus`) and, through `ChunkHolder.updateFutures`,
whether the chunk is accessible/ticking. Extra cost of *being* full: none of the generation
steps ever runs backwards in-game — statuses only advance, and a loaded `full` chunk returns
as an `ImposterProtoChunk` whose promotion to `full` is a pass-through
(`ChunkPyramid.LOADING_PYRAMID`, `ChunkStatusTasks.full:193-195`).

**Dirty, save, IO.** Every mutation that must persist calls `markUnsaved`
(`ChunkAccess:260-275`: one volatile boolean; `LevelChunk.markUnsaved:175-180` also feeds
`ChunkMap.chunksToEagerlySave`). Saves are scheduled four ways, all in
`server/level/ChunkMap.java`: eagerly (≤20/tick, <128 active writes, `saveChunksEagerly:495`),
by autosave (`MinecraftServer.AUTOSAVE_INTERVAL` 6000 gt → `ServerLevel.save(flush=false)` →
`saveAllChunks(false):438-445` → `saveChunkIfNeeded`, throttled to one queued save per chunk
per 10 s at `:736`), on unload (`scheduleUnload:513-541`, after the holder's save-sync
future), and flush saves (`saveAllChunks(true):413-437`, used by `save-all flush` and by
shutdown: `MinecraftServer.stopServer:649-660` first drains ticket work, then
`saveAllChunks(false, true, false)`, then `level.close()`). `ChunkMap.save:745-783` is the
one funnel: flush POI for the chunk, `tryMarkSaved()` (**clears the dirty flag before
anything is written**), refuse to let a non-`full` chunk overwrite a chunk known/observed
`full` on disk (`isExistingChunkFull:785-806`), snapshot with
`SerializableChunkData.copyOf` on the calling thread, encode `write()` on a background
executor, queue `SimpleRegionStorage.write → IOWorker.store`. `IOWorker`
(`world/level/chunk/storage/IOWorker.java`) coalesces in a `LinkedHashMap<ChunkPos,
PendingStore>`; a background-priority task drains one entry at a time into
`RegionFileStorage.write:100-111` (a `null` tag means `RegionFile.clear`); load first checks
the pending map, so a queued save is never read stale from disk (`loadAsync:138-153`).
`synchronize(flush)` joins every pending future, then optionally `RegionFile.flush`
(`FileChannel.force(true)`).

**The region file is a two-phase write, and oversized chunks leave it.**
`RegionFile` (`storage/RegionFile.java`): 4 KiB sectors, an 8 KiB header of
(offset,count,timestamp) per chunk. `write:285-315` allocates *new* sectors (the old
sectors stay allocated until the end), writes the data there, then points the header at it
(`writeHeader`), then runs a commit op, then frees the old sectors. A crash/kill before the
header write therefore keeps the old chunk exactly (rollback of that one save, nothing
else); the header write is the commit point. Data here is the *compressed* chunk stream
(5-byte chunk header + `RegionFileVersion` stream); at ≥256 sectors
(`EXTERNAL_CHUNK_THRESHOLD`, i.e. ≥1 MiB of stream) the data goes instead to an external
`c.<x>.<z>.mcc` file in the same folder and the region keeps only a 5-byte stub flagged
`version|128` (`write:294-306`, `createExternalStub:317-323`). The external file is fully
written to a temp file *before* the stub/header, but the rename over `.mcc` is the commit
op, i.e. it runs *after* `writeHeader` (`writeToExternalFile:325-334`) — the one ordering
in the whole persistence path where disk can name data that is not there yet.
On open, header entries with sector <2, count 0, or past end-of-file are zeroed with a
warning (`RegionFile.<init>:81-99`): that chunk then reads as absent. On read, a truncated
header/stream, a non-positive length, an unknown compression id, or a missing `.mcc` all
return `null` ("no chunk") rather than throwing (`getChunkDataInputStream:112-195`);
`RegionFileStorage.read:78-87` then parses with `NbtIo.read(DataInput)`, which is
`NbtAccounter.unlimitedHeap()` (`nbt/NbtIo.java:107-109`: quota `Long.MAX_VALUE`, but depth
is still capped at 512). **There is no 2 MiB quota on reading a saved chunk** — that quota
(`NbtAccounter.DEFAULT_NBT_QUOTA`) applies to network tags, not to region reads.

**Load failures almost all funnel to "regenerate".**
`ChunkMap.scheduleChunkLoad:553-577`: read tag → `upgradeChunkTag` (DataFixer;
`SimpleRegionStorage.upgradeChunkTag:50-69` rethrows any fixer failure as a
`ReportedException`) → `SerializableChunkData.parse` (returns `null` when `Status` is
empty at `SerializableChunkData.java:112-114`; a bad block/biome palette throws
`ChunkReadException extends NbtException` at `:164-177`) → `read` (builds the chunk;
block-entity and legacy-entity contents that fail are *dropped inside* `postLoadChunk:466-493`
and logged through a `ProblemReporter`, they do not fail the chunk). Any failure lands in
`handleChunkLoadFailure:579-597`: an `Error` (e.g. OOM, stack overflow) is a crash
(`ReportedException`); **anything else — `IOException`, `NbtException`, a fixer
`ReportedException` whose cause is unwrapped — is reported and replaced by
`createEmptyChunk`, a fresh `ProtoChunk` at `empty`**. That empty chunk is then promoted
through the full generation pyramid: true regeneration. An absent chunk (above) takes the
same path. So the whole research question reduces to: *can survival play make a finished
chunk's saved form absent, unreadable, unparseable, fixer-breaking, status-less, or never
written — with nothing but in-game actions?*

**Three separate persistence streams.** Terrain+block-entities+scheduled ticks+heightmaps+
structures+light live in `<dimension>/region/r.<rx>.<rz>.mca` (+ `.mcc`) via `ChunkMap`
(extending `SimpleRegionStorage`, constructed at `ChunkMap:168-174`). Entities live in
`<dimension>/entities/r.*.mca` via `EntityStorage` over its own `SimpleRegionStorage`
(`EntityStorage.java:86-109` writes `{Entities, Position}`; an empty entity chunk writes
`IOWorker.STORE_EMPTY`, which *clears* its slot); they are loaded/stored by
`PersistentEntitySectionManager` on its own schedule (`autoSave` on a non-flush save,
`saveAll`+flush on a flush save, `ServerLevel.save:894-899`) and their load failures are
reported per entity chunk without touching terrain. POI (beds, bells, workstations, …)
live in `<dimension>/poi/r.*.mca` via `PoiManager extends SectionStorage`
(`SectionStorage.java`): dirty sections are written per chunk from `tick`/`flushAll`/
`flush(pos)` (called from `ChunkMap.save:746` before every terrain save). POI is the one
coupled stream: `scheduleChunkLoad` combines the terrain future with
`poiManager.prefetch(pos)` (`ChunkMap:562-565`), and `SectionStorage.tryRead:189-210`
swallows an `IOException` to "no POI" but rethrows anything else as a
`CompletionException` — which then fails the *terrain* load into regeneration. Conversely
`SerializableChunkData.read:228` calls `poiManager.checkConsistencyWithBlocks` per section,
so saved POI that disagree with loaded blocks are repaired toward the blocks, not vice
versa. On shutdown the order is: drain chunk work, `saveAllChunks(flush)` (terrain, then
`poiManager.flushAll`, `processUnloads`, `synchronize(true).join()` at
`ChunkMap:435-437`), `entityManager.saveAll()` inside `ServerLevel.save` only when
flushing, then `level.close()` closes the entity manager (`saveAll` again + close storage)
and `ChunkMap.close:403-411` closes POI and terrain storage. A process kill bypasses all of
it: whatever headers were already committed is the world.

**Why survival data is (almost) always valid by construction.** Everything a player can put
into a chunk reaches disk by *encoding* live objects with the same codecs that decode them:
block entities via `saveWithFullMetadata` (`BlockEntity:110-121`, encode problems are
collected by a `ProblemReporter` and the field is omitted, not thrown), items via
data-component codecs with hard survival caps — writable book 100 pages × 1024 chars
(`WritableBookContent:14-23`), written-book page components ≤32767 JSON chars
(`WrittenBookContent:38-43,143-146`), sign lines ≤384 in the serverbound packet
(`ServerboundSignUpdatePacket`), a container-item component ≤256 stacks
(`ItemContainerContents:26-40`), fireworks ≤256 explosions, charges ≤1024, lore ≤256
lines; shulker boxes cannot go inside shulker boxes or container items
(`BlockItem.canFitInsideContainerItems:197-199`, `ShulkerBoxBlockEntity
.canPlaceItemThroughFace:242-243`) and bundle-in-bundle adds 1/16 weight per layer
(`BundleContents:36,73-85,213-241`), so item-component nesting depth in survival is tens of
NBT levels, nowhere near the read depth cap of 512. One silent lossy step exists on write:
`NbtIo.StringFallbackDataOutput.writeUTF:197-204` replaces any single string that does not
fit `DataOutput.writeUTF` (>65535 encoded bytes) with `""` instead of failing. Scheduled
ticks are capped only by one tick per (position, type) per chunk
(`LevelChunkTicks.schedule`, `ticksPerPosition`), i.e. volume, not a list cap.
And one pre-write lossy step exists in `ChunkMap.save`: because `tryMarkSaved` runs before
`copyOf`/encode/write, *any* exception in those (or an async storage failure, handled at
`:769-776` with only a report) leaves the chunk marked saved; it will not be retried until
modified again, and an unload after that discards it to whatever was last committed —
rollback, or regeneration if it was never committed. That is the most dangerous shape a
survival trigger could have: it needs a *deterministic save-time exception*, no kill, no
corrupt bytes.

## Trade-offs

- **External (`.mcc`) vs internal storage** is decided by compressed size alone, so the
  same build crosses the threshold or not depending on `region-file-compression`
  (dedicated default `deflate`; repetitive text compresses to almost nothing — an
  oversized claim must use incompressible content *and* state the server properties).
- **Size vs validity**: past the external threshold the game still saves and loads the
  chunk by design (that path exists for exactly this); size alone can only change *which*
  file holds the bytes and *who else* can read them (this repo's own `read_chunk` in
  `crates/mctas/src/vanilla/mod.rs` refuses `.mcc`, so tools — not the game — are the first
  casualty). Any stronger claim needs a second ingredient (a kill inside the commit
  window, a tool that copies `region/` without the `.mcc` siblings, a client-packet limit).
- **A client limit is not a disk limit**: `ClientboundLevelChunkPacketData` caps the
  section buffer at 2 MiB (`ByteBufCodecs.byteArray(2097152)`) and each block-entity update
  tag travels as an (untrusted-quota) compound tag; exceeding what a client accepts is a
  chunk-ban/disconnect question, scored separately, never as regeneration.
- **Lag is not corruption**: a build that makes a tick take seconds (scheduled ticks, POI
  scans, entity counts) can delay autosaves and unloads (`processUnloads` runs on
  `haveTime`, `ChunkMap:448-493`), widening rollback-on-kill windows, but under a clean
  shutdown everything is drained. Experiments therefore distinguish clean-stop,
  unload-and-reload, and kill-9 of *our own disposable server only*, in that order, and a
  kill result is never scored as a survival trigger.

## Five candidate families (the "layouts" of this study)

| # | family | mechanism it would need | from-source estimate | first test |
|---|---|---|---|---|
| 1 | **Obvious design — oversized via books in containers** (the bar): fill a chunk's chests/lecterns/shulkers with max-size written books until the *compressed* chunk ≥1 MiB | external `.mcc` storage; failure would need missing/renamed `.mcc` or the commit-window kill | saves+loads normally on 26.3 (unlimited read quota); expected outcome `normal`, possibly tool breakage | marker chunk + incompressible books, clean save/unload/reload, inspect region+.mcc bytes |
| 2 | **Oversized via other block entities / entities** (signs, hanging signs, banners, heads with profiles, decorated pots, campfires, furnaces with nested containers; item frames, display entities, armor stands, minecarts with containers/books) | same size threshold in the terrain stream (block entities) or the entity stream (entities), or a per-object codec that rejects on decode what encode produced | `normal` for terrain; entity-stream oversize is isolated (`entity-only` at worst) | one family at a time to the threshold, same round-trip, count block-entity bytes vs entity bytes | 
| 3 | **Deterministic save exception** (the dangerous shape): a survival state whose `copyOf`/`write()` or `BlockEntity.saveAdditional`/entity `save` throws every time | `tryMarkSaved` already cleared → chunk silently never persists → `rollback`/`regen` on unload with a clean stop, repeatable | no `throw` found in any block-entity save path (grep: only `BlockEntity.<init>` state validation); encode errors are reporter-collected. Needs the widest source sweep before any test is worth running | source sweep first; only a named class+method candidate earns a server test |
| 4 | **Volume in the bounded lists**: scheduled block/fluid ticks (~98k positions), POI records (~98k), post-processing lists, heightmaps/light (fixed size), structure references (generation-owned) | a list cap, an `int` overflow, or POI parse failure that is *not* an `IOException` (which would cascade POI→terrain regen via the combined future) | no caps found (`LevelChunkTicks`, `PoiSection` codec pending read); expected `normal` or `poi-only` | one near-full section of tick-scheduling blocks / POI blocks, round-trip + counts |
| 5 | **Ordering races without a kill**: portal travel, logout, ticket expiry (portal 300 gt / pearl 40 gt), chunk unload while an async encode/write is in flight, autosave throttle (10 s), entity moving between chunks mid-save | a stream saved from a stale snapshot, an unload discarding a chunk whose async write later fails, an entity saved in both/no chunk | unload waits on `saveSync` (`scheduleUnload`); `IOWorker` pending map makes reads see queued writes; expected at most `entity-only` duplication/loss, terrain `normal`/`rollback` of ≤10 s | unload-and-reload under continuous modification; portal round-trips with a passenger + container minecart; measure, don't kill |

Set aside by scope: filesystem editors, `/data`, `/setblock`-with-NBT and other commands,
datapacks/functions, disk-full and power loss, killing the host or a server the player does
not operate, older-version worlds (DataFixer/`blending_data`/below-zero retrogen are audited
for completeness but only 26.3-born chunks can pass the bar), and anything whose only
effect is client-side (scored `chunk-ban`, reported separately).

## Decision

Test in order 1 → 2 → 4 → 5 with one disposable-world harness (deterministic markers,
snapshot hashes per stream, clean-stop and unload/reload legs), and keep 3 as a pure source
sweep unless it produces a named throwing method reachable from survival state. The manual
positive control (delete the target chunk's header entry / its `.mcc` *on a copy of a
disposable world*, by the harness, never by hand-editing a test world) must produce `regen`
with neighbours intact before any candidate is believed — it proves the harness can tell
regen from rollback. Negative controls (empty marker chunk; a chunk just *under* every
threshold) run in every batch.

## Must-haves (the success bar for any claimed trigger)

- Repeatable on fresh disposable worlds (report n/n), exactly the target chunk changes,
  neighbouring chunks bit-identical, entity/POI consistency measured, not assumed.
- No commands or filesystem edits after world creation in the survival leg; the sequence is
  buildable/clickable by a survival player (the harness may *set up* a world with commands
  only if the scored leg is then replayed survival-clean, and says so).
- No dependence on killing the host, the server process, or on disk/power failure; kill-9
  legs against our own disposable server are characterisation only, labelled as such.
- Every heavy run under resource caps/timeouts on a copy; ≤2 heavy processes on a shared
  box; nothing is ever run against a shared or persistent world.
- A clear recovery/mitigation (restore the region+entities+poi+`.mcc` set from backup;
  the log lines that name the failure: `reportChunkLoadFailure`/`reportChunkSaveFailure` at
  `MinecraftServer:2164-2177`, the suppressed-exception crash-report files).

## Not goals

- A judge, a site kind, or a publishable "build" for deleting chunks.
- Beating any size record for its own sake; sizes are only thresholds to cross once.
- Client rendering, chunk *sending* performance, or anti-cheat questions beyond the
  chunk-ban classification.

## The obvious design's numbers and the ceiling (filled by measurement)

- Obvious (family 1) at threshold: *to be measured* — compressed bytes per max-size book
  under `deflate` vs `none`, books/chests needed for 256 sectors, round-trip outcome.
- Ceiling (from the mechanics): a saved chunk survives any size the heap can encode
  (read quota unlimited, external storage unbounded but one file per chunk); the only hard
  ceilings a survival player can hit are per-object codec caps (above), NBT *write* string
  truncation at 65535 bytes (silent `""`), and NBT *read* depth 512 (unreachable by
  survival nesting, §Mechanics). Every result is reported against those three numbers.

