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

## 5. Harness as built (`mctas chunk`, Rust)

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

**As built (47fad2c2d + ecf52ef8b).** `mctas::vanilla::chunkfile`
(`crates/mctas/src/vanilla/chunkfile.rs`) reads all three streams like `RegionFile`
(internal + external `.mcc`, gzip/deflate/none; LZ4 reported, not decoded) and classifies
absent/external-missing/truncated/bad-version/decode-failed/nbt-failed instead of
throwing; `VanillaServer::read_box` now loads `.mcc` chunks through it (it used to bail).
Every parsed chunk gets a raw `sha256` and a `content_sha256` (the tag tree hashed with
root `LastUpdate`/`InhabitedTime` skipped — the "neighbours intact" proof, since a plain
re-save changes the raw hash). New crate `crates/chunk-regen` = `mctas chunk
inspect|snap|diff|control|exp` (manifest TSVs; `control` refuses without `--on-copy`).
`VanillaOptions` gained `sync_chunk_writes`, `region_file_compression`, `level_seed`,
`normal_world`, `frozen_at_start`, `save_off_at_start` (defaults preserve every existing
caller), and `VanillaServer::{world_dir,dimension_dir,abort}`. Protocol deviation from
the plan: RCON packets are a single ≤1460-byte read on the server
(`server/rcon/thread/RconClient.java:24,48`), so payload states are built from *many
small* legal objects (≤900-char single book pages, ≤90-char sign lines) instead of one
large command; the `exp` setups log loudly if a command is rejected. Manifests of record:
this clone's `target/chunk-regen/<case>/{before,after}.tsv` (not committed; ~0.3 MB each
because they include spawn chunks).

## 6. Experiments (T, 2026-11-07, `mctas chunk exp <name>`)

Common conditions unless a row says otherwise: 26.3 (DataVersion 5023), void superflat,
seed 424242, view/simulation distance 3, `region-file-compression=deflate`,
`sync-chunk-writes=false`, `network-compression-threshold=-1` (the harness's hardcoded
value — matters only for the chunk-ban row), unfrozen, saving on, heap 2048 MiB. Target
chunk (4,4); markers = diamond+gold at y=20 corners, a composter (POI), a named armor
stand, a diamond in each neighbour; setup by RCON commands (legal-valued state, §5), then
`save-all flush` + clean stop + manifest, then a fresh server process on a copy, survival
bot teleports in, markers read back by RCON, clean stop, manifest. "Neighbours []" = no
neighbour's content hash changed. Runs before ecf52ef8b compared raw hashes and reported
`neighbours-resaved` for the same states (the pre-content-hash wording in the table).

| case | payload / control | before (target) | outcome (n) | evidence |
|---|---|---|---|---|
| `baseline` | markers only | region internal 655 B stored / 20,589 decomp, `full` | **normal** (1/1) | markers 6/6, stand 1, all streams round-trip |
| `control-header` | offline: zero terrain header entry | internal 717 B | **regen** (1/1 void) | markers gone (4/6 = neighbours only), regenerated chunk saved (501 B / 14,327, `full`); entities + POI streams untouched (stand fell into the void, POI record stale) |
| `control-header --normal-world` | same, seeded normal world | internal 8,239 / 54,011, 43 block ticks | **regen** (1/1) | seed terrain back (6,989 / 40,308), markers gone, stand survived on regenerated terrain |
| `control-garbage` | offline: scribble 64 stream bytes | internal 655 B | **regen** (1/1) | log `Failed to load chunk` + `Failed to read chunk` once each; same regenerated shape as header-zero |
| `books-under` | 120 writable books (1 page × 900 random chars) in 256 barrels | internal 71,978 / 166,031, 256 BEs | **normal** (1/1) | negative control; neighbours [] |
| `books --count 1400` | 1,400 such books | internal 800,691 / 1,446,508, 255 BEs | **normal** (1/1) | under the external threshold (deflate eats the first ~35%: 900 chars ≈ 570 stream bytes/book at 26-letter entropy) |
| `books --count 2200` | 2,200 such books | **external** 1,266,208 / 2,264,108, 256 BEs, log `Saving oversized chunk [4, 4] … to external file … c.4.4.mcc` | **normal** (2/2: also with `--sync`, 1,281,897 / 2,291,791) | reload + re-save both external; markers 6/6; neighbours []; no failure log lines |
| `control-dropmcc --count 2200` | the books world, then delete only `c.4.4.mcc` | external 1,266,209 → `external-missing` | **regen** (1/1) | log `External chunk` ×2; markers gone; entity stream kept its stand (fell), POI kept **256 stale barrel records** for barrels that no longer exist |
| `signs` (3 layers) | 768 oak signs, 8×90-char random lines each | internal 369,515 / 750,994, 768 BEs | **normal** (1/1) | book-free oversized-BE data, well under every packet cap |
| `signs --count 10` | 2,560 such signs | **external** 1,229,706 / 2,459,382, 2,560 BEs | **chunk-ban** (3/3 setups; disk **normal** every time) | joining/teleporting bot kicked: `EncoderException: Packet too large: size 2282103 is over 8` (`Varint21LengthFieldPrepender`: frame body ≥2²¹) + `Failed to deliver packet, sending fallback`; RCON-side markers 6/6, stand 1, chunk re-saved external; server did not crash. **Compression-off caveat in §9** |
| `poi` | 2,048 composters + marker | POI stream 7,368 / 137,375, 2,049 records | **normal** (1/1) | POI compresses 19:1; terrain stream unaffected (667 B) |
| `items` | 900 dropped writable books | entities 481,854 / 1,037,682, 871 entities | **normal** (1/1) | 870 items + stand counted after reload |
| `items --count 2600` | 2,600 dropped books | entities **external** 1,394,643 / 3,003,290, 2,520 entities | **normal** (1/1) | external *entity* storage round-trips; terrain untouched (648 B); 2,519 items counted |
| `control-entities` | offline: zero entity-stream header | entities internal 465 B | **entity-only** (1/1) | stand 0, region+POI content hashes identical, entity entry stays absent after re-save |
| `control-poi` | offline: zero POI-stream header | POI internal 122 B, 1 record | **normal (self-heal)** (1/1) | record rebuilt from the composter block on load (`checkConsistencyWithBlocks` create-branch), byte-identical POI stream |

**T-findings that cut across cases.**
1. **Oversized by itself is normal on 26.3.** External (`.mcc`) terrain, entity and sign
   chunks all survive stop → restart → reload → re-save, with `sync-chunk-writes` both
   ways. The owner's book hypothesis, as a *size* claim, does not reproduce in 26.3: the
   read path has no quota (`NbtIo.read → unlimitedHeap`), and oversized storage is a
   designed path, logged at WARN (`Saving oversized chunk …`) — fatal only to tools that
   don't follow `.mcc` (this repo's `read_box` bailed on it until this study).
2. **The three streams are independent in both directions.** Deleting the entity chunk
   costs exactly the entities; deleting the terrain chunk regenerates terrain but keeps
   entities (they fall / persist) and keeps POI records **stale forever in practice**:
   a saved POI section is `Valid=true`, and `PoiSection.refresh` (the re-derive from
   blocks) only runs when `!isValid`, while `checkConsistencyWithBlocks`'s create-branch
   only runs when the section is *missing* (`PoiManager.java` checkConsistency,
   `PoiSection.refresh:127`). Missing POI + present blocks heals; present POI + missing
   blocks does not — a villager-facing desync, not corruption, and the precise state a
   kill between independent stream commits leaves too.
3. **A joining player is the fragile consumer, not the disk.** The same 2,560-sign chunk
   that saves/loads cleanly kicks the bot with a 2,282,103-byte chunk packet under
   `network-compression-threshold=-1`. Server stays up, the chunk keeps saving. On a
   dedicated server left at the default threshold 256
   (`DedicatedServerProperties.java:107`) the same packet would be deflated first
   (`network/CompressionEncoder.java:19-21` throws only above 8,388,608 *uncompressed*;
   the 21-bit frame then applies to the compressed bytes), so this run proves the ban
   *mechanism and its exact encoder*, not the dose under defaults. Our bot cannot speak
   compression (`Bot::connect` bails on the SetCompression packet) — §10.
4. Item entities: 900 summoned → 870 stored, 2,600 → 2,519 (the rest crossed borders or
   merged while settling; the *stream* kept every stored entity). Not a loss mechanism:
   the missing few moved before the first save, visible in `before`.
5. Barrels are fisherman POIs: the books cases also exercised 256–257 POI records.
   Noted so the books rows are not read as POI-free.
6. No case produced `rollback`, `region-wide`, or `crash-only`, and no survival-built
   case produced any load/save failure log line.
7. Wall cost: each case ≈30–150 s end to end (two server starts dominate; setup RCONs:
   books-2200 ≈2,200 commands). No tick freezes, timeouts, or heap incidents at 2 GiB.

## 7. Tools added / changed

- `crates/mctas/src/vanilla/chunkfile.rs` (new) + `vanilla/mod.rs`: region/stream
  reading as above; `VanillaOptions` knobs; `read_box` external-chunk fix.
- `crates/chunk-regen` (new): `mctas chunk inspect|snap|diff|control|exp`.
- Unit tests: `cargo test -p mctas -p chunk-regen --lib` (synthetic region files:
  internal/external round-trip, missing-external, header-zero, garbage; manifest diff
  kinds) — 10 passed.
- Docs: this file + `designs/briefs/chunk-regeneration.md`.

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

## 9. Conclusions: every candidate, ranked by evidence (2026-11-07)

**Answer in one line: no survival-reachable trigger for terrain regeneration, corruption
or rollback was found in 26.3 — every stage that could produce one either encodes
survival state symmetrically, caps it far below the failure threshold, or degrades to
dropping just the offending object; the one repeatable survival-reachable *harm* in this
area is the chunk-ban (a joining client is kicked while the chunk itself is fine), and
the one true regeneration *mechanism* needs a process kill inside `RegionFile`'s
external-file commit window.**

| # | candidate | outcome class | evidence | survival legality | blast |
|---|---|---|---|---|---|
| 1 | Oversized chunk via books (barrels of written/writable books) | **normal** (incl. external `.mcc`, sync on/off) | T: `books` ×2 external + sub-threshold rows; F: `NbtIo.read → unlimitedHeap`, `RegionFile.write:294` external path is designed | legal values (≤1024 chars/page, ≤100 pages; we used 1×900); human cost of 2,200 distinct texts is the practical limit, not a rule | 0 |
| 2 | Oversized via non-book block entities (signs) | **normal** on disk; **chunk-ban** for joiners under compression-off | T: signs normal ×1, ban ×3 setups (2,282,103-byte packet, `Packet too large`, server alive, chunk re-saves); F: `Varint21LengthFieldPrepender` 2²¹ frame, `CompressionEncoder:20` 8,388,608 uncompressed | signs legal (used 90/384 chars per line); ban dose under **default threshold 256 untested** (bot lacks compression) — do not quote 2,560 signs as a default-server recipe | players entering only |
| 3 | Oversized entity stream (book item entities) | **normal** (incl. external) | T: 871 + 2,520 entities, entity `.mcc` round-trip | legal | 0 |
| 4 | POI volume (2,049 records) | **normal** | T: `poi`; POI codec unbounded `listOf`, count bounded by blocks | legal | 0 |
| 5 | Deterministic save exception (the `tryMarkSaved`-first shape) | **none found** — blocked at encode | F: no `throw` in any BE save path; `ValueOutput`/`ProblemReporter` collects codec errors and omits fields; `ChunkMap.save` would turn one into silent rollback/regen, which is why this family stays the top *source-watch* item | n/a | would be 1 chunk |
| 6 | Nested-container NBT depth >512 | **unreachable** | F: shulker-in-container bans + bundle weight 1/16/layer ⇒ ≲16 layers ≈ <110 NBT levels vs 512 read cap | — | 0 |
| 7 | Scheduled-tick list volume | **untested** (no cap found in source) | F: one tick per (pos,type) ⇒ ≤~196k/chunk, list codecs unbounded | legal | lag only (predicted) |
| 8 | Status downgrade / feature re-entry over a built chunk | **blocked by construction** | F: statuses only advance (`acquireStatusBump`), `ChunkMap.save` refuses proto-over-full, FULL requires its pyramid radius already past features | — | 0 |
| 9 | Kill `-9` inside an *internal* region write | **rollback of that one save** (characterisation) | F: new sectors written before the header commit; T by code-reading + the controls' disk states | not a survival action (process kill) | 1 chunk, latest save only |
| 10 | Kill `-9` between external header commit and `.mcc` rename | **regen** (or rollback to a stale `.mcc`) — *characterisation only* | F: `RegionFile.write:294-334` ordering; T state-equivalent: `control-dropmcc` = **regen** 1/1 with the exact post-kill disk shape. The live window (one header write + one rename) was **not** raced in this study | not a survival action | 1 chunk (+stale entity/POI) |
| 11 | Copying a world without its `c.*.mcc` files (backups/tools that take `region/*.mca` only) | **regen** of every oversized chunk in that copy | F+T: identical disk shape to #10 (`External chunk … is not file` → null → empty chunk) | operator/tool error, not a player trigger; the practical takeaway of this study | every oversized chunk copied |
| 12 | DataFixer failure / unknown or empty `Status` / palette error | **regen by design** (`handleChunkLoadFailure` → empty chunk for any non-`Error`) | F + T (`control-garbage` exercises the decode branch) | unreachable from 26.3-born survival data (fixer skipped at current version; statuses/palettes are written by the same codecs that read them) | 1 chunk |
| 13 | Block-entity / entity content that fails its own load | **object dropped, chunk normal** | F: `postLoadChunk` null-skip, `EntityType.loadEntitiesRecursive` reporter; T indirectly (2,560 BEs + 2,520 entities round-tripped) | — | 1 object |
| 14 | Stream desync (stale POI, surviving entities after any regen) | **poi-only/entity-only desync** | T: dropmcc + both regen controls; F: `Valid=true` POI sections never re-derive | a *consequence*, not a trigger | 1 chunk's records |

**Recovery / mitigation (every case).** A backup is only complete as the *set*
`region/ + entities/ + poi/ + **all `c.*.mcc` siblings** + `level.dat` + `data/`; restore
the set, never one file. Triage from logs: `Failed to load chunk` / `chunk/load` +
stored crash reports ⇒ that chunk regenerated; `Failed to save chunk` ⇒ its last save
was dropped after `tryMarkSaved` (rollback on next unload — re-touching the chunk
re-dirties it and is the in-game retry); `Saving oversized chunk` is a WARN naming the
`.mcc` path (check it exists; check your backup tool copies it); `Packet too large` /
`Packet too big` on join ⇒ chunk-ban: the world is fine, operate from RCON/console or
restore the chunk from backup, and prefer `network-compression-threshold=256` (default)
over `-1`. `mctas chunk inspect <world> --chunk x,z` names the exact per-stream state
offline.

## 10. Next experiments worth doing (in order)

1. **Bot network compression** (small, in `mctas::vanilla::bot`): speak SetCompression,
   then rerun `signs --count 10` with `network-compression-threshold=256` and scale
   lines/layers until the *uncompressed* 8,388,608 ceiling (`CompressionEncoder:20`) is
   the kicker. Converts the chunk-ban row from mechanism to a default-server dose, and
   sizes decorations (pots/shelves holding filled shulkers — far fewer placements per
   packet byte than signs) against the same ceiling.
2. **Scheduled-tick volume leg**: a chunk of continuously-scheduling blocks (fluid
   flowing on a ledge, observer pairs) saved mid-flight; report tick counts + round-trip.
   Cheap; closes the last untested family.
3. **External-commit kill race, counted**: repeated abort()-during-save attempts on an
   oversized chunk (own disposable server, `--sync`), report hits/attempts and the
   measured window; today the state-equivalent control is 1/1 and the live race is
   unrun, by design (bounded kills against our own process are allowed for exactly
   this predicted race).
4. **Save-throw sweep, continued**: component codecs whose constructors accept values
   the codec rejects (`WrittenBookContent` pages was checked: codec also unbounded).
   One named throwing (class, method) reachable from survival state would reopen
   family 3 — the only identified shape that yields clean-stop regen without a kill.
5. **Portal/logout entity races**: minecart-with-chest + passenger portal round-trips
   under continuous saves; predicted entity-only duplication/loss, terrain normal.
