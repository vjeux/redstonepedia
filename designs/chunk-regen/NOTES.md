# Chunk regeneration in 26.3 — notes

Research notebook for `designs/briefs/chunk-regeneration.md`. Source citations are
`mctas src show/grep` (decompiled 26.3 under `~/.cache/mctas/versions/26.3/src`), file:line
in that tree. Conventions: **H** = hypothesis, **F** = finding from source, **T** = test
result (disposable world, own server process, clean stop unless a leg is labelled
kill/offline). Outcome classes are the brief's: normal / rollback / chunk-ban /
entity-only / poi-only / regen / region-wide / crash-only.

## 0. Ground rules observed

- Own clone `~/agents/chunk-regen/minecraft-tas`, branch `explore/chunk-regen` from
  `origin/main` @ `80aec1bc7` (2026-11-07). No other clone edited; nothing pushed to main;
  site never published.
- Shared box checked before any heavy work (`uptime`, `ps`): parent viewer JVM + server
  already running = the allowed heavy pair; this study's heavy work (cargo build, server
  legs) runs one at a time and waits when the box is busy. Nothing started by others is
  ever killed.
- All worlds disposable, under this track's scratch dirs; region/entities/poi sets are
  hashed before/after; offline mutations happen only on *copies* and are the positive
  control, never the scored leg.

## 1. Chunk lifecycle map (F, from source)

### Generation and promotion
- Statuses (`world/level/chunk/status/ChunkStatus.java:21-30`): empty, structure_starts,
  structure_references, biomes, terrain, features, initialize_light, light, spawn, full.
  Only `full` is `ChunkType.LEVELCHUNK`; all earlier are `PROTOCHUNK`.
- Steps and requirements (`world/level/chunk/status/ChunkPyramid.java:11-46`):
  GENERATION_PYRAMID — structure_starts: `generateStructureStarts`; -references: needs
  starts radius 8; biomes: starts 8; terrain: starts 8 + biomes 1, writeRadius 0;
  features: starts 8 + terrain 1, **blockStateWriteRadius 1**; initialize_light: light
  engine attach; light: initialize_light 1; spawn: biomes 1 (skipped if upgrading a
  pre-existing chunk, `ChunkStatusTasks.generateSpawn:177`); full: wrap/convert to
  `LevelChunk` (`ChunkStatusTasks.full:184-212`). LOADING_PYRAMID is identical except
  starts/references/biomes/terrain/features/spawn are no-ops — a chunk loaded at any
  persisted status only *continues* from that status; there is no step whose task
  regenerates a chunk that already has terrain.
- Scheduling: `GenerationChunkHolder.scheduleChunkGenerationTask/applyStep`
  (futures per status, `acquireStatusBump` enforces strictly increasing started work);
  `ChunkMap.applyStep:624-651` — the EMPTY step is `scheduleChunkLoad`, every other step
  runs its task, and a step exception (except load, see §2) becomes a crash report
  ("Exception generating new chunk"). `ChunkHolder.updateFutures:273-326` maps ticket-level
  changes onto accessible/block-ticking/entity-ticking futures, each adding a save
  dependency. `GenerationChunkHolder.updateHighestAllowedStatus`: when the ticket level
  *drops*, futures above the new allowed status are failed+cleared — the in-memory
  chunk object and its persisted status are untouched (status "downgrade" is a scheduling
  view, not a data change; re-raising the ticket re-runs from persisted status).
- Feature re-entry into a full chunk: `WorldGenRegion.setBlock:324-363` may write into the
  8 neighbours (features writeRadius 1) and explicitly handles a LEVELCHUNK neighbour
  (`:336-342`, creates a real block entity). But a chunk only becomes full when everything
  its FULL step accumulated (radius via `ChunkLevel.RADIUS_AROUND_FULL_CHUNK`) is already
  generated past features, so a full, player-edited chunk with a not-yet-features neighbour
  cannot arise from ordinary generation order. (H-border: interrupted generation saved a
  proto chunk — see below, autosave only persists imposter/level chunks eagerly, but a
  proto chunk *is* saved on unload via `scheduleUnload → save` guarded by
  `isExistingChunkFull`. Whether a first-exploration frontier can leave a features-pending
  chunk next to a buildable full chunk is testable cheaply and, if real, is "feature
  writes into the edge of a built chunk", bounded to feature replaceability rules — not
  regeneration. Ranked low.)

### Tickets
- `server/level/TicketType.java`: player_spawn (20 gt, loading), spawn_search (1),
  dragon (persist+loading+simulation), player_loading, player_simulation
  (simulation+keep-dimension), forced (all), portal (300 gt, all), ender_pearl (40 gt),
  unknown (1 gt, can-expire-if-unloaded). `ChunkLevel`: ≤31 entity ticking, 32 block
  ticking, 33 full, >33 inaccessible up to `MAX_LEVEL`; `generationStatus(level)` derives
  the allowed `ChunkStatus` from distance to 33. Tickets only *delay* unload; no ticket
  path writes or deletes chunk data directly. `TicketStorage` persists forced tickets as
  saved data.

### Dirty tracking and save scheduling
- `ChunkAccess.unsaved` (volatile boolean, `ChunkAccess.java:64,260-275`);
  `LevelChunk.markUnsaved` notifies `ChunkMap::setChunkUnsaved` → `chunksToEagerlySave`.
  `ChunkHolder.sectionLightChanged:148` also marks unsaved. Note (known in this repo:
  `VanillaServer::snapshot` comment) scheduling/running a block tick does *not* dirty a
  chunk — tick-list-only changes ride the next block change's save.
- Eager: ≤20 saves/tick while <128 active writes and tick time remains
  (`ChunkMap.saveChunksEagerly:495-511`).
- Autosave: `MinecraftServer` every 6000 gt (adaptive ≥100, `:204-205,975-1036`) →
  `saveAllChunks(flush=false)` → per-holder `saveChunkIfNeeded`: must have been accessible
  since last save AND `isReadyForSaving` (save-sync future done) AND be
  imposter/level AND unsaved AND ≥10 s since its last queued save (`:716-743`).
- Unload: `processUnloads` (toDrop → pendingUnloads → `scheduleUnload:513-541`) waits the
  holder's current save-sync future, re-checks it didn't change, then `save(chunk)`,
  `level.unload(levelChunk)`, light-engine status update. An exception there is only
  logged ("Failed to save chunk", `:536-540`) — after `save`'s own catch, effectively the
  async-encode join path.
- Flush: `saveAllChunks(true):413-437` loops `save` over accessible chunks until a full
  pass saves nothing, then `poiManager.flushAll()`, `processUnloads(true)`,
  `synchronize(true).join()` (joins all pending `IOWorker` futures + `flush()` forces every
  region file). `ServerLevel.save:882-901`: level data, chunk source save, then entity
  manager `saveAll` if flush else `autoSave`.
- Shutdown (`MinecraftServer.stopServer:626-681`): stop network, save+remove players,
  spin ticks with tickets deactivating while any `chunkMap.hasWork()`, then
  `saveAllChunks(false,true,false)`, then `level.close()` each (entity manager
  `saveAll`+close, then chunk storage close — `IOWorker.close:238-250` waits a SHUTDOWN-
  priority barrier, so queued stores drain even at close unless `shutdownRequested`
  already blocks new foreground tasks; background `storePendingChunk` is scheduled after
  every task via `tellStorePending`).
- **`tryMarkSaved` before work** (`ChunkMap.save:747-749` runs before the try): a
  synchronous exception in `copyOf`/status guards, a background encode failure
  (`data::write` joined in the IO supplier), or an async storage `IOException` all surface
  only as `reportChunkSaveFailure` and the chunk is now *clean*. It stays playable in
  memory; it is re-dirtied only by a new mutation; an eventual unload discards to the last
  committed header. **This is the exact precondition a survival regen/rollback trigger
  needs: a block/entity state that makes `SerializableChunkData.copyOf`, `.write()`, a
  `saveAdditional`, or the region write throw deterministically.** (§4 sweeps for one;
  none found in block entities so far — every `saveAdditional` writes through
  `ValueOutput`, whose codec-encode failures go to a `ProblemReporter.ScopedCollector`
  and omit the field; `BlockEntity.java` has no throw in any save method; the only throw
  in the packet is constructor state validation `:58-62`.)

### IO worker and storage
- `IOWorker` per stream (`PriorityConsecutiveExecutor` over `Util.ioPool`): foreground =
  load/store-registration/scan; background = one `storePendingChunk` at a time (FIFO over
  insertion of `LinkedHashMap`; a re-store of a pending chunk replaces the tag, keeps the
  same result future — `:129-136`). `runStore:228-236` completes the future exceptionally
  on any storage exception. Reads check the pending map first (`copyData`), so save/load
  races inside one process cannot read a half-written sector.
- `SimpleRegionStorage` adds only DataFixer upgrade on top (`upgradeChunkTag:50-69` —
  version ≥ current returns as-is; so on a 26.3-born chunk the fixer never runs).
- `RegionFileStorage`: region cache ≤256 files (evict closes, `flush` forces);
  `write(pos, null)` = `RegionFile.clear` (header zero + delete `.mcc` + free sectors,
  `RegionFile:273-283`) — used for empty entity chunks and by region-level tooling, never
  for terrain from `ChunkMap.save` (terrain always writes a non-null tag).
- `RegionFile` two-phase write and external files: see the brief. Additional details:
  `FileChannel.open(..., DSYNC)` iff `sync` (`sync-chunk-writes`, dedicated default true;
  this repo's vanilla harness sets it false — tests must say which); file writes are
  single `FileChannel.write` calls whose return count is **not** checked (`write:301,305`,
  `writeHeader:336-339`, external `extFile.write(data):330`) — a short write (full disk)
  would commit a header over partial data, but short writes on regular files surface as
  exceptions in practice; disk-full is out of scope except as the async-save-failure shape
  above. `close()` pads to a full sector and forces. Header-open validation zeroing
  (`:81-99`) is process-local until the next `writeHeader` persists the zeroed header
  buffer — merely *opening then writing anything* in a region bakes in the erasure of any
  entry it judged invalid (e.g. after a truncated copy).
- Timestamps in the header are diagnostics only; nothing compares chunk timestamps across
  streams (no cross-stream versioning: terrain/entities/poi commit independently, so a
  kill can always leave streams from different saves of the same chunk — sketch of every
  desync below).

### Serialization (terrain stream)
- `SerializableChunkData.copyOf:323-392` snapshots sections (defensive `copy()`), light
  layers, block entities (`getBlockEntityNbtForSaving` per position — LevelChunk:
  `saveWithFullMetadata`), proto-only entity tags, heightmaps (cloned), scheduled ticks
  (`getTicksForSerialization(gameTime)` → delays relative to now), post-processing lists,
  structure starts/references packed. `canBeSerialized` false → `IllegalArgumentException`
  (caught by `ChunkMap.save` — Empty/placeholder chunks only).
- `write:394-455` emits xPos/yPos/zPos, LastUpdate, InhabitedTime, Status string, blending,
  upgrade, sections (palette codecs), isLightOn, block_entities, entities (proto only),
  block/fluid_ticks, PostProcessing, Heightmaps, structures. DataVersion added.
- `parse:109-208`: null iff `Status` string empty; status codec orElse EMPTY (an *unknown*
  status string therefore also regenerates); heightmap arrays taken as-is
  (`chunk.setHeightmap(type, longs)` in `read:286-289` — length is trusted; priming fills
  only missing types); tick lists codec orElse empty and filtered to this chunk; section
  palette parse uses `promotePartial(log) + getOrThrow(ChunkReadException)` — partial
  palette recoveries are logged and kept, hard failures throw (-> regen, §2). Sections
  with Y outside the level are kept as light-only entries with null block data (`:163-181`).
- `read:210-317`: wrong-position tag is *relocated* (honours the tag's own xPos/zPos? No —
  logs, reports misplaced, and builds at the *requested* pos, reading positions of block
  entities relative to requested chunk at `BlockEntity.getPosFromTag:68-81`, which rewrites
  out-of-chunk BE coordinates into this chunk). POI consistency is checked per section as
  sections attach (`:228`). LevelChunk gets post-load BE/entity creation (§Mechanics);
  proto keeps raw tags. `setLightCorrect(isLightOn)`, structure starts/references unpacked
  (unknown ids logged + dropped, references >8 chunks away dropped, `:545-570`).

### Entity stream
- `EntityStorage.loadEntities:46-79`: cached-empty fast path; wrong `Position` is reported
  (misplaced) but the entities still load into the requested chunk; `upgradeChunkTag(-1)`
  (fixer — a throw here fails the returned future, *not* the `reportLoadFailureIfPresent`
  future which is attached to the raw read only: `:51-53` vs `:70`. The caller,
  `PersistentEntitySectionManager`, treats a failed entity load as an empty/failed chunk
  of entities — terrain unaffected either way; exact catch to verify if an entity-stream
  candidate ever needs it). Entities are created via
  `EntityType.loadEntitiesRecursive(..., LOAD)` under a ProblemReporter: an entity that
  fails to construct/load is dropped, its passengers handled by the same recursive loader.
- `storeEntities:86-109`: empty → clear-slot write, only if the chunk wasn't already known
  empty; else each entity `save(output)` returns boolean (false = skipped silently, e.g.
  removed/non-persistent entities); no throw path except a bug in an entity's save — same
  sweep status as block entities (ValueOutput/reporter).
- Duplication window (characterisation, not terrain): an entity crossing a chunk border is
  moved between sections in memory; the two entity chunks save independently, so a kill
  between their commits can duplicate or lose it. Portal transfer removes from the old
  level and spawns a copy in the new (`Entity` teleport machinery) — again entity-only.

### POI stream
- `SectionStorage`: per-section data keyed by SectionPos long; dirty *chunk* set;
  `tick` writes while time remains, `flush(chunkPos)`/`flushAll` write now (the write
  itself is async through IOWorker; `flush` does **not** join the future — `:240-243`
  only reports failure). So even `ChunkMap.save`'s `poiManager.flush(pos):746` does not
  order POI-vs-terrain commits on disk; they race in two IOWorkers. `tryRead:189-210`:
  `IOException` → POI empty for that chunk (poi-only loss, silently re-derived later only
  by `checkConsistencyWithBlocks` where blocks exist); anything else → CompletionException
  → **terrain regen** via ChunkMap's combined future (the one cross-stream cascade).
  `PackedChunk.parse:303-326` codec-parses per section with `resultOrPartial(LOGGER::error)`
  — a bad section yields "no section", not a throw; the fixer upgrade inside parse
  (`upgradeChunkTag(Dynamic,1945)`) *can* throw — but again only runs when DataVersion <
  current, i.e. not on 26.3-born data.
- POI volume: one record per POI block position (`PoiSection` map). Max ~1 record/block;
  a full chunk of beds/workstations ≈ 98k records ≈ a few MB raw in the POI stream, which
  has the same external-file path. No list cap found (codec read still TODO before tests).

## 2. What each failure becomes (F, decision table)

| failure | where caught | outcome |
|---|---|---|
| region entry absent (offset 0 / no file) | `RegionFileStorage.read` null → `IOWorker.loadAsync` empty → `scheduleChunkLoad` empty | **regen** (by design: never generated, or cleared) |
| header entry invalid at open (sector<2, 0 sectors, past EOF) | zeroed in `RegionFile.<init>` | **regen** once any write persists the header |
| stub external-flag set, `.mcc` missing/not a file | `createExternalChunkInputStream` null | **regen** |
| `.mcc` present but stale (kill between header commit and rename; or region copied without `.mcc` update ordering) | reads fine | **rollback** to the stale external version |
| stream truncated / bad length / unknown compression id | `getChunkDataInputStream` null | **regen** |
| NBT bytes unreadable (decompress error, tag type error, IO) | `NbtIo.readTagSafe` → ReportedNbtException(cause=IO) → `handleChunkLoadFailure` non-Error | **regen**, crash-report file stored |
| NBT depth >512 (`NbtAccounterException extends NbtException`, quota unlimited but depth checked) | propagates (not an IOException) → handleChunkLoadFailure, ioException=true branch | **regen** |
| DataFixer throws on an old-version chunk | ReportedException, cause unwrapped §handleChunkLoadFailure | **regen** (n/a for 26.3-born: fixer skipped) |
| `Status` empty/unknown | parse null/orElse EMPTY → empty proto | **regen** |
| palette/biome codec hard error | ChunkReadException (NbtException) | **regen**; partial palette errors: logged, chunk loads degraded |
| block entity tag fails BE construction/load | `postLoadChunk` null-skip / ProblemReporter | **normal** chunk, that BE (and its contents) gone |
| entity-stream anything fails | EntityStorage/PersistentEntitySectionManager | **entity-only** (chunk's entities empty/duplicated), terrain normal |
| POI IOException | `SectionStorage.tryRead` exceptionally → empty | **poi-only** |
| POI non-IO (fixer throw, runtime bug) | rethrown → ChunkMap combined future → handleChunkLoadFailure | **regen** (cascade) |
| any save exception after tryMarkSaved (copyOf, encode, IO write) | `ChunkMap.save` catch / async handle → reportChunkSaveFailure | in-memory chunk lives; on unload: **rollback** (or **regen** if never committed) |
| load/save hits an `Error` (OOM, StackOverflow) | handleChunkLoadFailure alwaysThrow; save path catches only Exception → Error propagates | **crash-only** (saved chunk unchanged), or crash-loop if the same chunk reloads (**chunk-ban** by crash for anyone entering it) |
| clientbound chunk data >2 MiB section buffer / oversized BE update tags | client disconnects decoding (`ClientboundLevelChunkPacketData.STREAM_CODEC`), server logs only | **chunk-ban**, disk normal |
| kill -9 during internal region write before header | old header still points at old sectors | **rollback** of that save only |
| kill -9 between external header commit and `.mcc` rename | header=stub, `.mcc` missing/stale | **regen**/**rollback** (narrow window; kill-dependent, never a survival claim) |
| kill -9 with writes pending in IOWorker | pending map lost | **rollback** to last committed per stream (streams may be from different saves → entity-only/poi-only desync on top) |

Neighbour blast radius from the game code itself: every path above is keyed by
`ChunkPos` and frees/allocates sectors per entry; nothing in a single-chunk failure
rewrites another chunk's sectors (allocation can *reuse freed* sectors of other chunks
only after their headers moved — the old-bytes-resurrect shape requires a torn header
write, i.e. power loss/full disk, out of scope). Expected blast radius of any legit
candidate: exactly 1 chunk in exactly 1 stream (POI-non-IO cascade: 1 chunk, 2 streams).

## 3. Survival reachability audit per payload (F/H)

Caps are encode+decode symmetric unless noted; "bytes" = contribution to the *compressed*
chunk stream (what the 256-sector threshold measures). Repetitive payload compresses
(deflate) — a threshold-crossing build needs ~1 MiB of *incompressible* content (random-
ish distinct text), independent of which block holds it.

- **Written books** (chest/shulker/lectern/bookshelf/dispenser/hopper minecart…): writable
  100×1024 chars; signed page = text component, JSON ≤32767 (`flatRestrictedCodec`,
  rechecked on resolve `:143-146`). One book ≤ ~100 KiB raw writable / pages-as-components
  similar scale; a double chest holds 54 stacks ×1 book. Reaching 1 MiB *incompressible* ≈
  tens of max books — trivially buildable, hours of typing or a book-duplication-free copy
  chain (crafting copies keep pages; generation ≤3 for copies of copies). Failure beyond
  external storage: none found. Client side: opening a chest sends container content —
  items ride `ItemStack` stream codecs (2 MiB tag quota is *per tag* network-side), not the
  chunk packet; chunk packet sends the chest BE's *update tag*, which for containers is
  minimal (no Items in `getUpdateTag` for chests — verify during tests: this is why books
  in chests are a disk-size story, not necessarily a packet one).
- **Lectern**: one book, same numbers. **Chiseled bookshelf**: 6 books.
- **Signs / hanging signs**: 4+4 lines, packet cap 384 chars/line, stored as components
  (click/hover events can nest components only via commands — player edit path is plain
  text via `SignText`/filtering). Volume per block small; 98k signs in a chunk is a *count*
  test (block-entity count ≈ 98k × small tags still crosses sizes; BE count itself has no
  cap). **Banners**: pattern list — survival applies ≤6 layers via loom (cap to verify in
  `BannerPatternLayers`/loom menu; codec list itself unbounded? — check before testing,
  if the codec is unbounded but loom caps, survival is capped). **Heads/skulls**:
  `ResolvableProfile` (name ≤16? uuid, properties from session server) — player-obtainable
  values are server-filled, not free text; note-block sounds etc. no payload. **Decorated
  pots**: 4 sherds + one item stack (can be a book/shulker? pot accepts any single item —
  a nested-container delivery slot). **Campfire**: 4 item stacks. **Jukebox/disc, brewing,
  furnace, crafter, dispenser, hopper**: item stacks only. **Beehive**: ≤3 occupants (bee
  entity data incl. custom name, health… bounded). **Shelf** (new): item stacks.
  **Brushable blocks**: one loot item. None has a save-throw or an asymmetric codec found.
- **Entities**: item frames/glow frames (1 stack each, incl. a filled shulker/bundle/book),
  armor stands (6 equipment + poses — pose floats only settable by data/command? survival
  places default pose), display entities (**not survival-obtainable**: `/summon` only — any
  display-text finding is commands-only, exclude), minecart-with-chest/hopper (full
  containers), chest boats, donkeys/llamas with chests (str·3 slots; llama carpet+chest),
  allays/villagers (inventories filled by giving items — books count, 8 slots villager),
  wandering trader, item entities (ground stacks — a dropped max book is an entity),
  painting (variant only). Entity chunks use separate region files with the same 256-sector
  external rule (`c.*.mcc` under `entities/`), so entity oversize alone predicts
  `entity-only` worst case. Entity *count*: no per-chunk entity cap in storage (tick-time
  mob caps don't apply to frames/stands); save writes the whole chunk entity list as one
  tag — same unlimited-read story.
- **Nested containers**: chest(BE) > shulker item (CONTAINER ≤27 real slots; codec cap
  256) > bundle (≤~16 deep by weight, items cannot include shulkers) > book. Also ender
  chest → same (player ender inventory is playerdata, not chunk). Max NBT depth estimate:
  chunk root→block_entities→Items→stack→components→container→stack→bundle_contents→… each
  item layer ≈4-6 tag levels ⇒ 16 bundles ≈ ≤110 levels. **Depth-512 unreachable.** Same for
  charged crossbow (projectiles ≤1024, each may be a firework rocket ≤256 explosions —
  width, not depth).
- **Scheduled ticks**: blocks that self-schedule on placement/update (repeaters locked on,
  observers, fluid, sand/falling handled as entities, leaves decay checks, ice, crops,
  sculk…). One per (pos,type); max ≈ 2×98304 entries ≈ list codec unbounded; delay/priority
  bounded by machinery. Predicted normal; cost is tick time at unpack (`unpack` schedules
  all pending, `LevelChunkTicks:120-131`) — lag, not loss. Proto ticks filter on load to
  in-chunk positions (`SavedTick.filterTickListForChunk`).
- **Light/heightmaps**: fixed-size (2048 bytes/layer-section; heightmap longs sized by
  world height) — no player-controlled size. Corrupt-length arrays are a filesystem-edit
  case; `setHeightmap` trusts the array (offline control can probe `ArrayIndexOutOfBounds`
  = crash-only/Error? AIOOBE is RuntimeException → regen, but unreachable from survival).
- **Structures**: starts/references written by generation; players cannot add starts.
  A structure start that fails to load is dropped (`unpackStructureStart`), references
  filtered by distance — no chunk failure.
- **PostProcessing**: short lists per section from generation (liquid/carver leftovers);
  players cannot add entries; `LevelChunk.postProcessGeneration` drains them on first
  ticking load only.
- **InhabitedTime/LastUpdate**: longs; players control them only by standing there/waiting.

## 4. Save-throw sweep (family 3) — status: no candidate yet

Greps over 26.3 (2026-11-07): `throw new` in `world/level/block/entity/*` → only
`BlockEntity.<init>` validation (needs a block whose state doesn't match an *existing*
BE at save time — saves don't construct BEs). `saveAdditional`/`save` overrides in BEs and
`Entity.save` implementations write via `ValueOutput` (codec errors → reporter). Remaining
unswept: `ItemStack`/component codecs whose *encode* can `IllegalArgumentException` on a
value their own *constructor* accepted from gameplay (e.g. list caps enforced in codec but
not in the record constructor — `ItemContainerContents:39` and `WritableBookContent:26`
enforce in the constructor too; `WrittenBookContent` pages list is the interesting
exception: **PAGES_CODEC is `listOf()` unbounded and the record constructor checks only
generation; the 100-page cap lives in the writable→written path (`WritableBookContent`,
sign packet in `ServerGamePacketListenerImpl.handleEditBook`? to read). If some survival
path (copying, lectern, trading?) can produce >100 pages, encode still works (unbounded),
so no throw — size only.**) Also unswept: `TagValueOutput.buildResult` on >2^31 bytes
(byte-array int overflow) — needs a >2 GiB chunk tag in memory, heap dies first (Error →
crash-only). Family stays source-only until a named candidate appears.

## 5. Harness plan (`mctas chunk` subcommands, Rust)

New crate area (mctas-cli subcommand, following `bedrock::cli` shape; work functions in a
`crates/chunk-*`/mctas module, no fn >150 lines, editor-tool edits, Rust only):

1. `chunk inspect <world> [--chunk x,z]…`: for each dimension (`dimensions/minecraft/
   overworld` layout like `VanillaServer::snapshot_inner`), parse region headers of
   region/entities/poi, list per chunk: stream present?, sectors, stream bytes,
   external? (+.mcc filename+size), timestamp; decompress (deflate/gzip/none per
   `RegionFileVersion`; our existing `read_chunk` must learn `.mcc` + this — add support
   rather than bailing), parse NBT (existing `vanilla::nbt`), report Status, xPos/zPos,
   DataVersion, counts (block_entities, ticks, entities, POI records), and a content hash
   (hash the *decompressed* tag bytes — stable across recompression). Handles absent
   files/entry as first-class output (that's the regen signal).
2. `chunk snap <world>`: whole-world variant → TSV/JSON snapshot manifest (per chunk per
   stream hash+class), used to diff before/after legs and prove neighbours bit-identical.
3. `chunk control-delete <world-copy> --chunk x,z [--stream region|entities|poi]
   [--mode header-zero|drop-mcc|zero-status]`: positive controls on copies (Rust, header
   surgery on the copy only): header-zero = set entry 0 (+delete .mcc); drop-mcc = delete
   only the external file; zero-status = rewrite tag with `Status:""` (requires NBT
   *writer* — add minimal writer to `vanilla::nbt` if this control is needed; else skip
   mode 3 until then).
4. Server legs reuse `mctas::vanilla::VanillaServer` but **not** its frozen-tick defaults:
   need extras it doesn't expose — `sync-chunk-writes`, `region-file-compression`,
   gamemode survival, save-on allowed, level-type default (markers must survive on real
   terrain for the regen test: void-flat regenerates to the same void, indistinguishable!
   → regen detection needs either a normal world (terrain differs from edited) or marker
   blocks placed *in survival leg* that generation would never place, e.g. a bedrock-free
   signature: diamond-block pillar pattern + a named armor stand + a bed (POI) at a fixed
   spot; regen = markers gone AND chunk tag LastUpdate/InhabitedTime reset-ish AND
   `handleChunkLoadFailure`/empty-chunk evidence in the log. Add options as new fields to
   VanillaOptions with defaults preserving current behaviour, so door tooling is untouched.
   Extend, don't fork.)

Existing harnesses to reuse: `bedrock::vanilla` (official-server survival-bot patterns),
`VanillaServer::snapshot/read_box` for block reads (needs the `.mcc` fix), RCON `cmd`
for setup-only actions (scored legs: bot clicks/`data` free — book insertion via bot
container interaction if feasible; else via `item replace`/chest `data merge` in setup
and re-scored as "setup-assisted, survival-equivalent state" with the *state* being
exactly survival-reachable — flagged honestly, since the save/load path doesn't care who
filled the chest; the full-survival replay is then a small spot-check, not every leg).

Budgets: server heap 1536 MiB default (as harness), timeout per leg; one heavy process at
a time on the shared box; worlds in `/dev/shm` default harness dir or track scratch with
`keep_dir` for post-mortems; every leg ends with `snap` diff + log grep (Failed to load/
save chunk, oversized/external lines).

## 6. Experiments (T, in order)

*(pending — filled as legs run; each: world seed/properties, setup, scored leg, snapshot
diff, log lines, outcome class, n/n.)*

## 7. Tools added / changed

*(pending)*

## 8. Open source questions (cheap greps before their tests)

Resolved 2026-11-07:
- POI: `PoiSection` records keyed by `SectionPos.sectionRelativePos` (a short) — ≤4096
  records/section *by position uniqueness*; `Packed.CODEC = {Valid: bool,
  Records: PoiRecord.Packed.listOf()}` unbounded. A duplicate add with a different type
  overwrites + logs in IDE only; same type is a no-op.
- Banner: `BannerPatternLayers.CODEC` is `listOf()` unbounded; survival is loom-capped at
  6 layers (`LoomMenu:179 hasMaxPatterns = size >= 6`). Tooltip shows min(size,6).
- Books: the 100-page/1024-char caps are enforced in the *packet*
  (`ServerboundEditBookPacket.STREAM_CODEC`: `stringUtf8(1024).list(100)`, title ≤32) and
  in `WritableBookContent`'s constructor/codec; `handleEditBook:1000-1033` then only
  checks the carried item has writable content. `signBook` converts pages to literal
  components (depth 1). Written-book *decode* accepts more (32767/component, unbounded
  page count) than survival *creation* can produce.
- Update tags (chunk-ban surface): default `BlockEntity.getUpdateTag:217-219` is EMPTY.
  Containers (chest etc.) don't override it, so a joining client does **not** receive
  chest contents (books in chests are disk-only size). Overrides that do send data:
  sign (text), banner (patterns+name), skull (profile), campfire/decorated-pot/shelf
  (visible items — pot/shelf can carry a filled book/shulker stack to every joining
  client), lectern/bookshelf via… (lectern has no getUpdateTag override in grep; its book
  sync is via container/menu packets — verify in tests if chunk-ban candidates matter),
  beacon/conduit/spawner/trial (state-sized).
- `STORE_EMPTY` users: only `EntityStorage` (grep across src). Terrain is never
  intentionally cleared by the game.

Still open:
- `PersistentEntitySectionManager` handling of an exceptionally-failed entity-chunk load
  future (from `upgradeChunkTag` throw).
- `LevelStorageSource` dimension path + where `entities/` folder is created
  (`ServerLevel` constructor) for inspect's defaults.
- Lectern book client sync path (only matters for chunk-ban scoring).
