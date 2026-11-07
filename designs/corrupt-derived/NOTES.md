# Derived/saved world-data staleness — notes

Track: explore/corrupt-derived, from origin/main 35bcee3c7. Brief:
`designs/briefs/corrupt-derived.md` (read it first — rankings, probe spec). Lane 5 of 5
corruption audits; scope is normal code paths that fail to recompute, invalidate, or
persist a derived/side record consistently — not the chunk-persistence lane's corruption
triggers, not POI/entity storage, not block-entity internals, not update suppression.
All source citations are the 26.3 decompiled listing (`mctas src show <Class>`,
`~/.cache/mctas/versions/26.3/src`). Source reads before this clone's first build used
the bedrock clone's binary read-only (`~/agents/bedrock/minecraft-tas/target/release/mctas
src …`); that clone is never edited.

## Tools

- `mctas corrupt light|inhabited|audit` (this track, `crates/corrupt`): official-server
  scenarios on disposable void worlds + region-file readback. See "Probe results".
- One shared-tool addition: `mctas::vanilla::read_chunk_tag` (raw chunk NBT from a saved
  world), refactored out of the private `read_chunk`.

## 2026-11-07 — source audit

### Heightmaps (`Heightmap`, `ChunkAccess`, `LevelChunk`, `ProtoChunk`, `SerializableChunkData`)

- Types/predicates: `Heightmap.Types` (145-195): WORLD_SURFACE_WG/WORLD_SURFACE =
  `!isAir`; OCEAN_FLOOR(_WG) = `BLOCKS_MOTION_IN_HEIGHTMAP` tag; MOTION_BLOCKING(_NO_LEAVES)
  = motion tag (no-leaves variant) OR non-empty fluid state. Tag membership is per-block,
  so the only state-dependent term is the fluid one.
- `Heightmap.update` (82-109): returns immediately when `localY <= firstAvailable - 2`;
  raises the top when an opaque block lands at/above it; when the top cell becomes
  non-opaque it rescans downward through *current* chunk blocks. A heightmap that is too
  high is therefore invisible to every later edit except a change of the phantom top cell
  itself; a too-low one self-corrects on the next opaque placement above the true top.
- The only live writer: `LevelChunk.setBlockState` (284-287) updates the four live maps
  unconditionally (after the `oldState == state` no-op exit, 279-281), then
  `markUnsaved` (348). Every survival block change (player, piston `finalTick`, fluids,
  explosions, falling blocks, growth features placed through `Level.setBlock`) funnels
  here. The neighbor-update cap cannot skip it (it is inside the block write, not a
  neighbor cascade entry) — cross-lane note for the suppressor lanes.
- Direct section writers that bypass it, all worldgen: noise terrain
  (`NoiseBasedChunkGenerator` 379, 424 — proto stage, maps primed after), `OreFeature`
  via `BulkSectionAccess.getSection` → `section.setBlockState` (OreFeature 148-159;
  BulkSectionAccess 22-40). Ore replacements are predicate-equivalent under both
  predicates (non-air full blocks → non-air full blocks; "discard on exposure" skips the
  write, leaving the stone), so the bypass is value-preserving for every vanilla ore
  feature. Flagged as a latent trap, not a finding.
- `ProtoChunk.setBlockState` (146-163) maintains only
  `getPersistedStatus().heightmapsAfter()`, priming missing ones by full scan; live types
  (`FINAL_HEIGHTMAPS`) are maintained from TERRAIN on (`ChunkStatus` 18-30), i.e. through
  FEATURES decoration. Promotion copies raw arrays (`LevelChunk(ProtoChunk)` 156-160).
- `ImposterProtoChunk` (a full chunk seen by a neighbor's generation) is created with
  `allowWrites=false` at both sites (`ChunkStatusTasks` 201, `SerializableChunkData`
  303): `setBlockState` returns null (61-63), `getSection` returns the imposter's own
  empty sections (56-58) — worldgen writes at a full chunk's border are silently dropped,
  never half-applied. No heightmap/light desync from this path.
- Persistence: save clones all present maps (`SerializableChunkData.copyOf` 361-366 →
  `write` 450-452); load installs them verbatim (`read` 283-291: `setHeightmap` for each
  stored type, prime only the missing; `Heightmap.setRawData` 127-135 falls back to a
  prime only on length mismatch). **No consistency check against blocks exists.** Client
  copy likewise trusted from the packet (`LevelChunk.replaceWithPacketData` 518-528).
- Consumers (grep `getHeight(`): `NaturalSpawner.getRandomPosWithin` (334-341: spawn Y
  uniform in [minY, WORLD_SURFACE+1] — stale-high dilutes attempts, stale-low truncates
  the column), `NaturalSpawner` 441 via `SpawnPlacements.getHeightmapType` (74),
  `ServerLevel` lightning-rod check 623 + `LightningRodBlock` 99, `BeaconBlockEntity`
  137 (beam scan bound), `MapItem` 138 (map colors), `PortalForcer` 64 (exit portal
  height), `PlayerSpawnFinder` 151-157, `Raid` 676, `VillageSiege` 119,
  `WanderingTraderSpawner` 129, `Ghast` 448, `Entity` 1449, `FireflyBushBlock` 31,
  `AmbientDesertBlockSoundsPlayer` 80.
- Verdict: **no vanilla survival creator with any effect; load trust makes it a perfect
  amplifier** for other lanes' corruption (L4 in the brief). Probe P4 is the negative
  control.

### Light (`LevelLightEngine`, `ThreadedLevelLightEngine`, `ChunkSkyLightSources`, save/load)

- Gate: `LevelChunk.setBlockState` (294-301) updates sky sources + queues `checkBlock`
  only when `LightEngine.hasDifferentLightProperties(old, new)` (emission, dampening, or
  either side `useShapeForLightOcclusion`, `LightEngine` 41-48). Matches the engine's own
  dampening model (`getLightDampeningInto` 50-60); shape-only state changes of
  shape-occluding blocks always trigger. No hole found here.
- Queue: `ThreadedLevelLightEngine.checkBlock` (59-67) submits a PRE_UPDATE task through
  `ChunkTaskDispatcher` at the chunk's queue level; tasks append to `lightTasks` and
  `runUpdate` applies at most 1000 per batch (33, 128-135, 191-213), scheduled via
  `tryScheduleUpdate` (182-189). Light application is therefore always behind block
  writes by (backlog/1000) batch ticks.
- Re-dirty on apply: storage change → `LightChunkGetter.onLightUpdate`
  (`LayerLightSectionStorage` 276) → `ServerChunkCache.onLightUpdate` (462-465) →
  `ChunkHolder.sectionLightChanged` (142-149) → `chunk.markUnsaved()`. So a chunk whose
  light applied *after* a save is saved again later — **if it is still loaded**.
- Save snapshot: `ChunkMap.save` (745-777) → `SerializableChunkData.copyOf` (323-391)
  reads `getDataLayerData` per section (336-339) at call time. No light drain anywhere
  in the save path; `ThreadedLevelLightEngine.waitForPendingTasks` (216-218) is used
  only as a chunk-*send* dependency (`ChunkMap` 1286).
- Unload: `ChunkMap.scheduleUnload` (513-540) runs `save(chunk)` (526) and then
  `lightEngine.updateChunkStatus(pos)` (531) — the wipe
  (`ThreadedLevelLightEngine` 69-83: retainData(false), setLightEnabled(false),
  queueSectionData(null) for all sections, section status empty), submitted with queue
  level `() -> 0` vs `checkBlock`'s ticket level. Tasks still queued then run against
  wiped storage (no sections to write) and `sectionLightChanged` finds no chunk
  (`getChunkIfPresent` 143) — no re-dirty, no resurrection.
- Load: `isLightOn` (`SerializableChunkData` 104, 121, 283, 435-437) +
  `ChunkStatusTasks` 35 (`status >= LIGHT && isLightCorrect()` ⇒ treat as lit) +
  `ThreadedLevelLightEngine.lightChunk` (164-180: `lighted=true` skips
  `propagateLightSources`; `setLightCorrect(false)` at 166 then true at 177 is the
  generation path, never touched by live `checkBlock`). Stored layers are queued into
  the engine at chunk read (`SerializableChunkData.read` 231-247) and become the light.
  `SkyLightEngine.setLightEnabled(true)` (284-301) only *fills empty* layers above the
  highest sky-source with 15; it never darkens or re-propagates borders. `onNodeAdded`
  scheduling: sky storage adjusts top-section bookkeeping only
  (`SkyLightSectionStorage` 57-69); block storage does not override it
  (`LayerLightSectionStorage` 172). **Nothing re-propagates light across a border when a
  light-correct chunk loads.**
- `ChunkSkyLightSources` (per-column lowest sky-source edge): not serialized;
  `fillFrom` (33-45) recomputes from blocks when light initializes, incremental `update`
  (77-96) has the same top-only shape as `Heightmap.update` but is only fed from the
  same `setBlockState` gate — self-healing on load, a *source* of truth for the next
  poke, not for the stored layers (L4's sky twin).
- Why the naive border case still fails (measured geometry, not folklore): light
  sections exist wherever a `ChunkHolder` does (ticket level ≤ `MAX_LEVEL = 33 +
  RADIUS_AROUND_FULL_CHUNK`, `ChunkLevel` 10-15; `fullStatus` 38-55). Player tickets:
  simulation level `31 - simulationDistance` (`DistanceManager` 36, 115, 131-133),
  loading level 31 per viewed chunk (269). Any block a player/machine can change (entity
  ticking ≤31, block ticking ≤32, one piston/fluid step beyond) has light-on neighbors
  ≥2 chunks past it (INITIALIZE_LIGHT's accumulated radius from FULL is 1:
  `ChunkPyramid` 30-31, so level-34 chunks have light initialized; level 35 may not,
  but it is ≥3 chunks — ≥33 blocks diagonally more — from the nearest editable cell,
  outside the 15-block light range). The remaining border hole is the **approach
  window**: B scheduled below INITIALIZE_LIGHT while A already ticks; edits in A during
  that window never enter B's (not-yet-storing) sections, and B then initializes from
  its stored layers. L3 in the brief.
- Clean shutdown drains light first (`MinecraftServer.stopServer` 649-660 loops on
  `ChunkMap.hasWork`, which includes `lightEngine.hasLightWork()` and the light
  dispatcher, 460-469) — **quit-to-title is not a door**; the door is unload during
  play (ChunkMap.scheduleUnload above) with a backlog still queued. L1.
- Autosave interplay: `saveChunkIfNeeded` throttles per chunk to one save per 10 s
  (728-737); the non-flush autosave clears the throttle (439) but still only saves
  unsaved chunks. A mid-backlog snapshot is repaired by the apply-then-re-dirty path
  only while the chunk stays loaded.

### Inhabited time / local difficulty

- `ChunkAccess.incrementInhabitedTime` (370-372) and `setInhabitedTime` (374-376) do
  not `markUnsaved` (contrast `setLightCorrect` 392-395 and the structure mutators
  203-240, which do). Called once per tick per *spawning* chunk
  (`ServerChunkCache.tickSpawningChunk` 404-406; spawning set = players within the
  8-chunk natural-spawn tracker, `DistanceManager` 41).
- Every save gate reads the unsaved flag: `saveChunkIfNeeded` (723-725), `save`→
  `tryMarkSaved` (747; `ChunkAccess` 264-271), shutdown flush
  (`saveAllChunks(true)` 413-437 filters through `save`). Eager save on unload also
  skips clean chunks (`saveChunksEagerly` 504-505), and unload's own `save` (526) no-ops
  via `tryMarkSaved`. **Increments persist only when something else dirtied the chunk**
  (block write 348; block-entity change `Level.blockEntityChanged` 876-880). Scheduling
  block/fluid ticks does not dirty (`LevelChunkTicks.setOnTickAdded` is wired to
  `LevelTicks` schedule bookkeeping, 35-37/58) — the repo's vanilla harness already
  discovered the tick-list twin of this (`crates/mctas/src/vanilla/mod.rs` 436-439).
- Serialization is faithful when it happens (`InhabitedTime` tag:
  `SerializableChunkData` 118, 272, 378, 400; copied proto→level `LevelChunk` 134).
- Consumer: `ServerLevel.getCurrentDifficultyAt` (692-702) →
  `DifficultyInstance.calculateDifficulty` (43-61): local scale =
  clamp(inhabited/3,600,000)·(1.0 hard / 0.75 else), i.e. 50 h of inhabited ticks to
  full local effect. Drives equipment chances, spider potion effects, zombie
  reinforcements/leaders, skeleton-trap odds (`ServerLevel.tickThunder` 560-562).
- Sequence (L2): an inert spawning chunk (e.g. stone-only sky platform, no block
  entities that change, no random-tick-sensitive blocks) near an AFK spot grows
  inhabited time in memory only; leaving unloads it back to its last dirtied value.
  A single block edit in the chunk "banks" the grown value (the control).

### Structures (starts, references, pieces)

- Storage: `ChunkAccess.structureStarts/structureReferences` (74-75); mutators
  `markUnsaved` (203-240). ProtoChunk additionally tracks per-piece generation state
  during FEATURES; LevelChunk promotion copies starts/refs (153-154).
- Creation order is enforced by the generation pyramid: STRUCTURE_REFERENCES requires
  STRUCTURE_STARTS at radius 8 (`ChunkPyramid` 14), `createReferences` scans ±8 chunks
  for starts whose bounding box intersects the center chunk (`ChunkGenerator` 638-670),
  FEATURES then places pieces for referenced starts only. Pieces reaching beyond 8
  chunks from their start chunk are silently not placed and not spawn-relevant —
  consistent truncation, both records agree.
- `createStructures` skips a set when a valid start already exists (524-528);
  `tryGenerateStructure` passes the previous start's reference count into generation
  (607, 633-636). Starts/references are packed/unpacked verbatim
  (`SerializableChunkData` 292-293, 372; unpack validates registry membership) and never
  recomputed from the seed after first save — there is no regeneration path to go stale
  against. `/locate`'s reference increment (`getStructureGeneratingAt` 317-350 →
  `StructureManager.addReference`, `StructureCheck.incrementReference` 233) is a cheat
  command side effect, out of survival scope.
- Imposter (full) chunks swallow late structure writes by design (129-148) — and their
  generation already completed, so no record is left half-written in normal order.
- Consumer that makes the permanence useful: `ChunkGenerator.getMobsAt` (478-503)
  spawn overrides by piece bounding box, `NaturalSpawner.isInNetherFortressBounds`
  (323-332). Verdict: by-design baseline (L5), no failure found.

### Loot / feature generation state

- Seeding: `RandomizableContainer.setBlockEntityLootTable` (42-48) `random.nextLong()`;
  structure pieces likewise (`StructurePiece` 466 chest, 489 dispenser; igloo/mineshaft
  grep). Archaeology uses position-derived seeds (`DesertPyramidStructure` 70
  `blockPos.asLong()`).
- Unpack: `unpackLootTable` (72-90) triggers GENERATE_LOOT, clears the table (82),
  `LootTable.fill` (145-164) inserts each generated stack. `RandomizableContainerBlockEntity`
  overrides route every container access through unpack (48-95); dirtying rides on
  `super.setItem`→`setChanged`. Zero-item roll ⇒ no insert ⇒ no dirty ⇒ the on-disk
  `LootTable` survives an unload; next load re-rolls with the same non-zero seed ⇒
  same empty result. Cosmetic side effect: `canOpen` (79-81) keeps spectators locked
  out of an opened-but-empty chest forever.
- Seed 0 is the "random each time" sentinel (`LootTable.getRandomItems(params, seed)`
  121-123 via `withOptionalRandomSeed`; saved only when non-zero,
  `trySaveLootTable` 57-70). No vanilla generation path found that leaves seed 0 with
  a table (all set seeds explicitly; template block entities get theirs from pieces or
  the template's own NBT). Would-be renewal requires non-vanilla data.
- Entity containers (minecart/boat) mirror the same interface (`ContainerEntity`
  41-107) with entity-storage dirtying instead — entity lane's surface.
- Feature generation state per se is not recorded post-hoc: decoration is a pure
  function of (seed, chunk pos) (`setDecorationSeed`, `ChunkGenerator.applyBiomeDecoration`
  352-362), so an interrupted-then-resumed generation reproduces identically; nothing
  to renew in survival.

### Trial vaults (`VaultBlockEntity`, `VaultServerData`, `VaultSharedData`)

- `VaultServerData` codec (20-35): `rewarded_players` (linked set, order = eviction
  order), `state_updating_resumes_at` (absolute game time — restart-safe),
  `items_to_eject`, `total_ejections_needed`. `lastInsertFailTimestamp` is memory-only.
- Cap: `MAX_REWARD_PLAYERS = 128` (29); `addToRewardedPlayers` (66-77) evicts the
  eldest on overflow — documented renewal across 129 distinct players, not a bug.
- Every mutator `markChanged()` (76, 85, 94, 101, 113) except `set(from)` (117-123,
  also skips `totalEjectionsNeeded`) — used only when components are applied to the
  block entity (item→world transfer), not reachable for a placed survival vault.
  The dirty flag is flushed to `setChanged` every server tick of the block entity
  (`VaultBlockEntity` 250-258). Insert path checks `hasRewardedPlayer` before
  rewarding (261-277). Verdict: sound.

### Tickets / ticking level (`TicketStorage`, `DistanceManager`, `ChunkLevel`)

- Types/timeouts/flags: `TicketType` 10-25 (PERSIST=1, LOADING=2, SIMULATION=4,
  KEEP_DIMENSION_ACTIVE=8, CAN_EXPIRE_IF_UNLOADED=16; PORTAL 300t/15, FORCED 0/15,
  ENDER_PEARL 40/14, UNKNOWN 1/18, PLAYER_SPAWN 20/2, DRAGON 0/6).
- `Ticket` serializes type+level+`ticks_left` (11-18); `TicketStorage.packTickets`
  (71-79) writes only persist types; loaded tickets start deactivated and are activated
  at server start (59-69, 96-104; `MinecraftServer` 547). All mutators `setDirty`
  (153, 177, 265, 288, 351).
- Dedup by (type, level) with timeout refresh (147-156, 181-183); removal removes the
  single entry (223-267). Safe because explicit removal exists only for single-owner
  types (DRAGON `EnderDragonFight` 184/222, FORCED command, PLAYER_SIMULATION per
  chunk-player set `DistanceManager` 109-129).
- Expiry: `purgeStaleTickets` per tick (279-288) decrements only when
  `canTicketExpire` (291-302): has timeout AND (can-expire-if-unloaded OR no holder OR
  holder ready-for-saving). A ticket therefore cannot expire while its chunk is still
  loading — the pause is bounded by generation completing, and KEEP_DIMENSION_ACTIVE
  types keep their dimension ticking so the purge runs. `deactivateTicketsOnClosing`
  (304-306) parks everything except UNKNOWN during shutdown.
- Ender pearls: ticket placed by the pearl/owner (`ServerPlayer.registerAndUpdateEnderPearlTicket`
  2305-2319, radius 2 ⇒ level 30), pearls serialized in player data (485-513) and
  respawned on login — stasis loaders are the designed behavior. Ticking level itself
  is derived live by the two tracker graphs; nothing stores it. Verdict: sound.

### Saved data, raids, maps (`SavedDataStorage` + subclasses)

- Storage writes exactly `isDirty` entries and clears the flag on encode
  (`SavedDataStorage` 188-196). Audited mutators all `setDirty`: `Raids` (ctor 49,
  tick remove 87, periodic 93-95, create 143), `RandomSequences` (wrapper marks dirty
  on every draw incl. fork/setSeed, 97-162 — its `clear/reset/setSeedDefaults` 65-83
  do not, but those are command paths), `MapIndex` (28-32), `WanderingTraderData`
  (35-51), `TicketStorage` (above), `CustomBossEvents`/`ScoreboardSaveData`/
  `EnderDragonFight`/`WorldBorder`/`GameRuleMap` mutate through command paths that set
  dirty or through `SavedDataStorage.set` (104-107).
- `Raids.tick` (75-96) ticks a raid only via `Raid.tick`, which early-outs when its
  center chunk is unloaded — the record waits, then resumes; village loss is handled
  inside `Raid`'s own inactivity rules. No survival path found to duplicate/orphan a
  raid record (creation is keyed by `getNearbyRaid` distance in `createOrExtendRaid`
  102-145).
- Maps: `MapItemSavedData` is a snapshot by design (colors from
  `chunk.getHeight(WORLD_SURFACE)` at update time, `MapItem` 138 — a heightmap
  consumer, L4). Banner decorations refresh when the map updates near them. No
  consistency promise to break.

### Biomes / noise caches

- Stored quart biomes in sections are the single source: `ChunkAccess.getNoiseBiome`
  (417-430) reads sections; `fillBiomesFromNoise` (432-443) runs only in generation
  (BIOMES step; imposter-gated 246-250). Serialization rides inside section palettes
  (`SerializableChunkData` 178, 417). `/fillbiome` marks unsaved (FillBiomeCommand
  153) — command-only. `RandomState`/climate samplers are recomputed from seed+settings
  per level instance and never persisted. No second copy exists to disagree with.
  Blending/retrogen (`BlendingData`, `BelowZeroRetrogen`) are upgrade-time records,
  out of the 26.3-born survival scope; noted, not audited further.

### Dead ends (checked, no disagreement)

- `Heightmap.update` rescan races mid-piston-move: every intermediate state goes
  through `setBlockState`, and the final placement re-runs update — converges.
- Fluid-only heightmap term: fluids are block states; waterlogging/level changes are
  state changes through the same writer.
- Neighbor-update suppression starving heightmap/light: both updates run inside
  `setBlockState`, upstream of any neighbor cascade (they are not cascade entries).
- `ChunkAccess.setHeightmap` without `markUnsaved` (168-170): load-time only.
- `LevelChunkTicks` scheduling not dirtying (known, events lane; the harness marks
  chunks manually for snapshots).
- Structure start `INVALID_START` persistence: invalid starts are not stored
  (`tryGenerateStructure` 625-630 stores only valid); regeneration on a later attempt
  is the designed retry.
- Vault display-item cycling rolls the loot table for display only; reward roll is at
  ejection (`VaultBlockEntity` 238-240 vs 261+) — no state to stale.
- `TicketStorage.replaceTicketLevelOfType` (363-381) resets `ticksLeft` on
  simulation-distance change; affected type (PLAYER_SIMULATION) has no timeout.

## Probe results

All on the unmodified official 26.3 server in disposable void worlds
(`mctas::vanilla::VanillaServer`, tick-frozen and stepped; region files read back with
`read_chunk_tag`). Commands run from this clone's repo root after
`cargo build --release -p mctas-cli`.

### P2 inhabited — CONFIRMED (`./target/release/mctas corrupt inhabited --ticks 1200`)

```
[inhabited] baseline after first save: subject=15 control=15
[inhabited] after 1200 spawning ticks + unload: subject=19 (grew 4), control=1215 (grew 1200)
[inhabited] MARKER: control banked its growth, subject rolled back — increments persist only piggybacked on an unrelated chunk dirtying
```

Reproduced again at `--ticks 200` (subject +4, control +200). The +4 is the few ticks
between the baseline snapshot's marker-save and the readback. Survival form: an inert
spawning chunk's local difficulty resets on unload; one block edit banks it.

### P1 light — fences hold at every constant tried (`mctas corrupt light`)

Runs: 1 chunk × 10 fill toggles (40,960 `checkBlock` tasks), then 9 chunks × 6 toggles
(221,184 tasks), `leave_ticks` 200 and 30, plus an immediate `save-all flush` arm with
no step after the fills. Every readback — immediate save, after unload, after
restart+load — showed complete light (per test chunk: `sum=115200 lit=11264
full15=4096`, identical to the settled control). Why, measured against the source:

- Batches run *inline during submission*: `addTask` calls `runUpdate` whenever the
  pending list reaches 1000 (`ThreadedLevelLightEngine` 128-135), and dispatcher
  submissions execute concurrently with the edits — the fills themselves take longer
  in wall-clock than the light math, so the queue never grows past ~one batch.
- Applied sections re-dirty the chunk (`sectionLightChanged`), so any snapshot taken
  mid-drain is re-saved after the drain while the chunk stays loaded.
- Unload takes multiple ticks after the ticket rise (holder futures, unload queue),
  each draining ≥1000 more tasks; the wipe arrives after the queue is empty.

So the code-level holes (save without drain, wipe submitted ahead of queued
`checkBlock`s, no load-time re-propagation) have no survival-reachable window we could
open: a same-tick machine phase would need >~2000 light-relevant writes in one
block-event phase *and* an unload completing within ~2 ticks of it (stasis-pearl
departure), which piston push limits (12 blocks/piston) make a >170-piston synchronized
build — recorded as an engineered-race residual, not a finding. The generation-frontier
variant (L3) is closed by the pyramid: INITIALIZE_LIGHT is guaranteed within
distance-to-full ≤ 2 (accumulated radius of LIGHT's requirement), so every chunk in
which survival can change a block (ticket level ≤ 32, +1 cell of reach) has
light-storing neighbors on all sides, and own-chunk-only seeding in
`BlockLightEngine.propagateLightSources` (118-127) is backstopped by the neighbor
having been storing (and seeded, for generation-era sources) before the edit. P3
therefore not built; the geometry above is its measurement.

### P4 audit — negative control clean (`mctas corrupt audit <world>`)

On the P2 world (kept with `--keep`): `450 chunks, 115200 columns, 0 WORLD_SURFACE
mismatches`. Void-world caveat: columns are mostly empty or single marker stones; the
audit exercises decode + the writer path, not terrain generation. MOTION_BLOCKING is
not audited (needs the block-tag tables; WORLD_SURFACE's NOT_AIR predicate is the one
most consumers read).

## Revised ranking (post-probe)

1. **Inhabited-time rollback** — proven, deterministic, survival-only steps (P2).
   Effect: local-difficulty control at inert farm chunks.
2. **Heightmap trust-on-load** — no vanilla creator (source + P4), but any out-of-band
   block change becomes a permanent, load-trusted lie for spawning/heights/maps. The
   amplifier other lanes' triggers would inherit.
3. **Light save/load ordering** — real code-level gaps, fenced in practice by inline
   draining, apply-re-dirties, unload latency, pyramid guarantees, and shutdown drain
   (measured, P1). Residual: synchronized machine-phase + departure race.
4. **Empty-roll loot non-persistence** — real, harmless with vanilla seeds.
5. By design: structure bounding-box permanence, pearl/portal loaders, vault FIFO.

## Next steps

- If the coordinator wants the light race closed out: a redstone light-lag machine
  (piston array toggling ≥2000 glowstone in one phase) + stasis-pearl departure on the
  official server, `corrupt light` harness with bot clicks instead of `fill`.
- MOTION_BLOCKING audit once the tag tables are pulled from the server jar
  (`data/minecraft/tags/block/blocks_motion_in_heightmap.json` + fluid check).
- Cross-lane: hand the chunk-persistence lane the heightmap/light load-trust citations
  (their triggers + these records = permanent derived-data corruption).
