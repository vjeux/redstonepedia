# Lane 4 NOTES — POI & entity-store desynchronization (Minecraft Java 26.3)

Running notebook. Brief: `designs/briefs/corrupt-poi-entity.md`.
Clone: `~/agents/corrupt-poi-entity/minecraft-tas`, branch `explore/corrupt-poi-entity`,
base `35bcee3c7` (origin/main tip at clone time). Game source: decompiled 26.3 in
`~/.cache/mctas/versions/26.3/src` (Vineflower, unobfuscated 26.3 names; line numbers below
are that listing's). Scope guard: no file-corruption triggers from the chunk-persistence lane;
save/rollback appears only as a *control* to classify what a store mismatch does.

## 0. Process mapping (docs/process.md applied to an audit)

process.md gates new site components (brief -> obvious-design bar -> ceiling -> search -> review).
This lane adds no new kind/metric/judge rule. Adapted: the brief is the design doc; the "obvious
design" bar is vanilla synchronized behaviour (control); the "ceiling" is the strongest useful
deviation (permanent job/portal/persistence control); "search" is source audit + survival
reachability; review gate = official-server verification with deterministic markers.

## 1. POI store mechanics (source)

Files: `world/entity/ai/village/poi/{PoiManager,PoiSection,PoiRecord,PoiTypes}.java`,
`world/level/chunk/storage/SectionStorage.java`, `server/level/ServerLevel.java`,
`world/level/Level.java`.

- Types/tickets (`PoiTypes` 99-121): 13 workstations maxTickets=1; HOME (bed HEAD states only,
  44-49) =1; MEETING (bell) =32, validRange 6; BEEHIVE/BEE_NEST/NETHER_PORTAL/LODESTONE/
  TEST_INSTANCE/LIGHTNING_ROD =0 (never HAS_SPACE, never IS_OCCUPIED; queries must use ANY).
- Block->POI sync: `Level.setBlock` (Level 216-262) always calls `updatePOIOnBlockStateChange`
  (258) when the chunk accepts the state; `WorldGenRegion.setBlock` forwards too (332).
  `ServerLevel.updatePOIOnBlockStateChange` (1650-1666) does **nothing synchronously**:
  if old/new POI types differ it enqueues two separate `getServer().execute(...)` tasks,
  remove first (1655-1658), then add (1659-1664). POI is therefore stale for the rest of the
  current tick (exact drain point = MinecraftServer task loop; verify).
- `PoiManager.add` (71-73) getOrCreate+add; `remove` (75-77) getOrLoad only — a remove for a
  section absent from the store is silently dropped. `release` (204-208) `orElseThrow`s
  IllegalStateException ("POI never registered"); `Util.pauseInIde` returns its argument
  (Util 767-774), so it really throws in production. Same in `PoiSection.release` (99-108).
- Tickets are anonymous counts (`PoiRecord` 15, 39-57): acquire decrements if >0, release
  increments if <max. No owner, no reconciliation sweep. `freeTickets` persists
  (`PoiRecord.Packed` 89-101; codec default for a missing `free_tickets` field is **0** =
  fully occupied). Any acquire without matching release leaks forever; village-ness reads
  occupancy, not blocks (below).
- Village distance: `PoiManager.isVillageCenter` (228-233) = section has a VILLAGE-tag POI
  with Occupancy.IS_OCCUPIED; `DistanceTracker` (292-322) propagates levels 0..6;
  `ServerLevel.isCloseToVillage` (1680-1686). A leaked ticket = phantom village section
  (golem/raid inputs; verify tag contents + consumers in villager step).
- Rebuild: `SerializableChunkData` (228) calls `checkConsistencyWithBlocks` per section at
  chunk load. `PoiManager.checkConsistencyWithBlocks` (252-263): existing section ->
  `PoiSection.refresh`; missing + mayHavePoi -> create+populate. `PoiSection.refresh`
  (126-138) runs **only when `!isValid`** and reuses old record objects by position key via
  `oldRecords.computeIfAbsent` **without checking the type matches** the block's type —
  intended to preserve tickets, but a type change while invalid resurrects the old type +
  old ticket count. A *valid* section that disagrees with blocks is never rebuilt.
  `isValid` persists (`PoiSection.Packed` 149-160; missing `Valid` defaults false).
- Dual-index bug: `PoiSection.add(PoiRecord)` (64-80). On type mismatch at a position it
  logs "POI data mismatch", overwrites `records`, but never removes the old record from
  `byType`. `getRecords` streams `byType` (49-51) so queries see the phantom too;
  `pack` iterates `records.values()` (45-47) so the phantom is not saved (live-only until
  reload). `PoiRecord.equals/hashCode` are by position only (75-87): an A->B->A sequence
  without removes makes the second A-record fail `byType` set insertion (equal to phantom),
  leaving `records` and `byType` pointing at different objects with different tickets.
- `PoiManager.ensureLoadedAndValid` (281-290) is called **only** by `PortalForcer` (44).
  Its `loadedChunks` set only grows: a chunk once requested is never re-requested.
  Villager queries never validate; they trust whatever `getOrLoad` returns, loading POI
  sections from the poi region storage even for unloaded chunks (`getInChunk` 104-111).
- `SectionStorage` (POI persistence): sections cached in `storage` are never evicted;
  `tick` writes dirty chunks while budget lasts (80-90); `writeChunk` async via
  SimpleRegionStorage (235-247). Blocks (region/), POI (poi/), entities (entities/) are
  three separately-flushed stores — crash windows diverge; classification only (other lane).

### Step-1 candidate list (to rank after villager/portal/entity steps)

- C-POI-1 Piston-moved workstation (no-BE workstations) resets tickets, orphans villager
  memory; later release may throw. Needs: push-reaction check per workstation + villager
  validation order.
- C-POI-2 Same-tick POI staleness from deferred execute tasks (break/replace/claim races;
  portal creation vs immediate teleport).
- C-POI-3 Ticket leak on villager death/removal/unload without release -> workstation/bed
  permanently full, phantom village. Needs: death/removal callback audit.
- C-POI-4 `PoiSection.add` byType phantom on type overwrite (live-only). Trigger needs an
  add-without-remove path: refresh reuse? deferred task reordering across unload? TBD.
- C-POI-5 Valid-but-wrong POI section never rebuilt (store mismatch classification).
- C-POI-6 `ensureLoadedAndValid` one-shot loadedChunks (portal search blindness). Portal step.

## 2. Villager brain / claims (source)

Files: `npc/villager/Villager.java`, `ai/behavior/{AcquirePoi,ValidateNearbyPoi,
AssignProfessionFromJobSite,ResetProfession,YieldJobSite,PoiCompetitorScan,WorkAtPoi,
VillagerMakeLove,VillagerGoalPackages}.java`, `monster/zombie/{Zombie,ZombieVillager}.java`,
`ai/memory/MemoryModuleType.java`.

- Claim flow (CORE package, VillagerGoalPackages 43-65): Validate JOB_SITE +
  POTENTIAL_JOB_SITE every tick; AcquirePoi (validate=JOB_SITE absent, acquire=
  POTENTIAL_JOB_SITE) -> GoToPotentialJobSite -> AssignProfessionFromJobSite (within 2.0:
  POTENTIAL erased, JOB_SITE set, profession read from *current* `PoiManager.getType`,
  AssignProfessionFromJobSite 20-45) -> ResetProfession (JOB_SITE absent + XP 0 + level<=1
  -> profession NONE). HOME validated only in REST (133), MEETING only in MEET (164).
- `AcquirePoi` (96-111): HAS_SPACE candidates (48 range, 5 closest), pathfind, then
  `poiManager.take(...)` whose **Optional result is ignored** (106) and the memory is set
  unconditionally (107). A failed take leaves a ghost memory (claim without ticket).
  `YieldJobSite` (50-60) is partly a ghost-cleanup: it erases POTENTIAL without release
  when a competitor already holds that JOB_SITE. No equivalent cleanup exists for ghost
  JOB_SITE (except `PoiCompetitorScan`, XP-winner, same-profession + sensor range only,
  17-52) or ghost HOME (no competitor scan at all).
- `ValidateNearbyPoi` (23-42): only within 16 blocks. POI missing -> erase memory, **no
  release** (record gone; release would throw). Break+replace at same pos: new record has
  full tickets, `exists` passes, old villager keeps memory without holding the new ticket;
  a second villager can take the ticket -> two JOB_SITE/HOME memories, one ticket.
  `WorkAtPoi` (21-43) restocks on memory+distance (1.73) only — no POI existence/ticket
  check; combined with deferred POI removal, a just-broken workstation still restocks
  until validation sees the POI gone. Restock counters are per-villager entity state
  (Villager 339-385: 2/day via lastRestockGameTime/numberOfRestocksToday).
- Death is safe: `Villager.die` (589-598) `releaseAllPois()` (guarded by getType +
  POI_MEMORIES predicate, 618-634, 171-180) runs *before* `LivingEntity.die` invokes
  `killedEntity` (LivingEntity 1477), so zombie conversion (Zombie 400-413, 233-252)
  happens after release; witch conversion releases explicitly (Villager 746-760).
  Curing creates a fresh brain (ZombieVillager 231-272: data/gossip/offers/XP only).
  => No ticket leak on death/zombify/witch in 26.3. Remaining leak needs removal without
  die() — dimension transfer keeps GlobalPos memories (ticket held cross-dimension
  indefinitely; die() in the other dimension still releases cross-dimension correctly).
- Memories HOME/JOB_SITE/POTENTIAL_JOB_SITE/MEETING_POINT all have `GlobalPos.CODEC`
  (MemoryModuleType 34-37) -> survive chunk unload/save. Tickets therefore stay consumed
  while the villager is unloaded: POI occupancy (village-ness, raid centering) reflects
  unloaded villagers indefinitely.
- `VillagerMakeLove.tryToGiveBirth` (64-78): takes a vacant HOME ticket for the child,
  releases if breeding fails, else sets child HOME. Paired correctly; the take scans
  HAS_SPACE so leaked bed tickets silently cap breeding.
- Piston-movable workstations (no block entity): cartography table, composter, fletching
  table, cauldrons, stonecutter, loom, smithing table, grindstone (+ lightning rod).
  Pushing a claimed one destroys the old record, creates a fresh-ticket record at the
  landing cell (deferred tasks), and orphans the JOB_SITE memory until validation (<16)
  erases it. Another villager can win the fresh ticket first (job transfer/sabotage);
  beds/bells/BE-workstations are immovable (mechanics.md §2 isPushable).
- Consumers: `Raids` (116) centers raids on IS_OCCUPIED VILLAGE-tag POIs in 64;
  `Raid` validity/move logic calls `level.isVillage` (287-291, 437, 484, 679);
  raiders path to HOME POIs with Occupancy.ANY (Raider 506). Iron golems:
  `wantsToSpawnGolem` = LAST_SLEPT within 24000 gt + no GOLEM_DETECTED_RECENTLY
  (Villager 873-875, 924-927); 5 agreeing (3 when panicking) summon (846-871).
  Bed-ticket desync therefore switches golem production off ~1 day later, and phantom
  occupied POIs keep sections "village" for raids with no villagers present.
## 3. Portals (source)

Files: `world/level/portal/PortalForcer.java`, `world/level/block/NetherPortalBlock.java`,
`world/entity/{PortalProcessor,Entity}.java`.

- Search: `PortalForcer.findClosestPortalPosition` (41-50): radius 16 (nether side) / 128
  (overworld side), `ensureLoadedAndValid` first (its only caller), then POI
  NETHER_PORTAL/ANY in square, filtered by world border and **live block check**
  `getBlockState(pos).hasProperty(HORIZONTAL_AXIS)` (48). Phantom portal POIs (record,
  no block) are therefore harmless for linking; *missing* POIs (block, no record) make
  the search blind -> `createPortal` (52-156) builds a duplicate portal (link asymmetry).
  Survival missing-record paths are the deferred-add tick window and store mismatch
  (classification only); normal portal lighting adds POI via the deferred task.
- Exit geometry is re-derived from blocks (`BlockUtil.getLargestRectangleAround`,
  NetherPortalBlock 158-165), not from POI: an orphan portal block is a legal 1x1 exit.
  Orphans need suppressed shape updates (updateShape self-destructs incomplete portals,
  85-102) — trigger belongs to the update-suppression lane; here it is the *effect*.
- Timing: non-players have portal transition time 0 (118-127); players use gamerule
  delays. `PortalProcessor` (transient, not serialized; copied by reference on dimension
  change, Entity 3157) decays 4/tick when not inside (PortalProcessor 39-41), so breaking
  a portal mid-countdown cancels cleanly except for same-tick ordering edges.
- Cooldown: `Entity.portalCooldown` (default delay 300 gt, 2731-2734; vehicle inherits a
  player passenger's delay) is **refreshed to full every tick the entity stands in any
  portal block** (2690-2692) and decrements otherwise (2703-2705). Persists in NBT
  (`PortalCooldown`, 2136/2221), across dimension copies (3156) and mob conversions
  (ConversionType 132). Passengers cannot use portals themselves (3324-3326); whether a
  passenger's own cooldown is set when its vehicle teleports decides immediate
  dismount-bounce behaviour — checked with passenger serialization in §4.
## 4. Entity sections / transfer / passengers / leashes / persistence (source)

Files: `world/level/entity/PersistentEntitySectionManager.java`,
`world/entity/{Entity,Leashable,Mob}.java`.

- One manager per dimension. `knownUuids` (34) rejects duplicate UUIDs within the
  dimension: `addEntity` warns and drops the second copy (61-77), including on chunk
  load (`processPendingLoads` 232-238). Same UUID in two dimensions is possible (no
  cross-dimension registry); same UUID in two chunk entity files loads once, second
  silently vanishes. `Callback.onRemove` always frees the UUID (451), every reason.
- Save roots: `storeChunkSections` saves section entities filtered by `shouldBeSaved`
  (178-202). `Entity.shouldBeSaved` (4105-4110): never passengers (nested in vehicle
  NBT instead), never a vehicle with exactly one player passenger (that stack is saved
  in the *player's* data as RootVehicle), never when removalReason.shouldSave()==false.
  Consequences: logout while riding with a villager boat-mate parks the whole stack in
  playerdata (villager claims/tickets frozen, despawn-immune); an entity whose vehicle
  is unsavable is only as persistent as the vehicle record.
- Cross-dimension (`Entity.teleportCrossDimension` 3191-3225): passengers ejected and
  teleported **first**, recursively; then a new instance is created, `restoreFrom`
  does a full NBT round-trip (3149-3158, so codec-less brain memories and transient
  refs drop, UUID/cooldown/PortalProcessor carry over), the old instance is removed
  (3212) and the new added (3214), passengers re-mounted (3216-3218). Passenger portal
  cooldowns are never set by a vehicle-initiated teleport (only `handlePortal` sets
  them, Entity 2710): a passenger that dismounts inside the exit portal can bounce
  back instantly (mob transition time 0). If vehicle creation fails after passengers
  teleported, the stack is split across dimensions (3206-3208; vanilla types ~never).
- Leashes (`Leashable`): saved as holder UUID **or fence BlockPos, no dimension**
  (CODEC 366-378). `restoreLeashFromSave` (89-109) resolves in the *current* level:
  UUID holder missing -> lead drops once `tickCount > 100`; fence pos ->
  `LeashFenceKnotEntity.getOrCreateKnot(currentLevel, pos)` (LeashFenceKnotEntity
  127-129) re-anchors at the same coordinates in the new dimension (knot then lives
  or dies by whatever block is there). Holder teleporting/dying invalidates the
  in-memory holder ref; `tickLeash` drops the lead when holder can't interact
  (148-154). Leashing through a portal only survives if the holder is already in the
  destination dimension when the mob's leash resolves.
- Persistence/despawn: `Mob.checkDespawn` (685-711) discards (no `die()`) on Peaceful
  for disallowed types and on distance for non-persistent mobs; villagers and
  wandering traders override `removeWhenFarAway`=false (Villager 505-507,
  WanderingTrader 153-155). `PersistenceRequired` is plain saved NBT state set by
  nametags/equipment pickup etc.; nothing recomputes it, so a once-persistent mob
  stays persistent (intended, but it also means store-mismatch copies keep it).
- VILLAGE POI tag = `#acquirable_job_site` + home + meeting
  (`data/minecraft/tags/point_of_interest_type/village.json`): any occupied
  workstation/bed/bell section is a village center for raids/golem-adjacent logic.
## 5. Ranked desyncs (pre-experiment; to be updated with verdicts)

Scored: survival reachability x persistence x usefulness. Citations in §1-4.

| # | Desync | Survival sequence (sketch) | Persistence | Usefulness | Failure modes |
|---|---|---|---|---|---|
| R1 | Break+replace bed under distant claimant -> double HOME, possible ticket leak | Villager claims bed; move villager >16 away (boat/minecart); break+replace bed head same pos; second villager claims; ghost sleeps first -> claimant erases w/o release | Forever (both memories + leaked count saved) | Shared beds; bricked bed + ghost sleeper; breeding/golem disruption | Claimant sleeps first (heals); villagers must not validate within 16 before swap |
| R2 | Break+replace workstation -> ghost JOB_SITE + second take | As R1 with workstation | Until competitor scan meets (same profession, sensor range) or validation | 2 villagers restock at 1 workstation | PoiCompetitorScan in dense halls; needs separation/timing |
| R3 | Same-tick deferred POI window | Swap workstation type as villager acquires; two entities enter new portal same tick | Tick-scale (effects: ghost claim R2 / duplicate portal permanent) | Duplicate exit portals; instant ghost claims | Window may be ~0 if tasks drain before entity ticks — MEASURE |
| R4 | Piston-push claimed workstation | Push claimed composter/etc 1 cell while owner away; rival claims fresh ticket | Until owner re-validates/re-acquires | Job transfer without breaking block; trade-hall sabotage/defence | Owner nearby re-claims; BE workstations immune |
| R5 | Phantom workstation from store mismatch (CLASSIFY ONLY) | poi/ file newer than region/ (other lane's trigger) | Forever (valid section never rebuilt) | Profession + restock with no workstation block | Trigger not survival-owned here; breaking/replacing at pos heals record |
| R6 | Fence leash re-anchors at identical coords cross-dimension | Fence-leash mob by portal near origin; mob enters portal; fence pre-built at same coords in nether | Until snap/knot death | Hands-free mob parking through portal | Coords unscaled: only near origin is knot within 12; knot dies without fence block |
| R7 | Passenger portal-cooldown bypass | Boat+villager through portal; break boat inside exit portal | Instant bounce | Selective 1-tick return transport | Must dismount inside portal blocks; vehicles with player use player rules |
| R8 | RootVehicle logout parking | Ride boat with villager, log out | While offline | Claims frozen; village occupancy persists; despawn immunity | Villager unusable while parked |
| R9 | PoiSection byType phantom (CLASSIFY ONLY) | Store mismatch type-at-pos, then place other POI type there | Live session only (pack drops phantom) | Phantom queries/takes | Needs prior mismatch; heals on restart |
| R10 | Duplicate-UUID / cross-dim copies (CLASSIFY ONLY) | Crash-window saves (other lane) | Load-time drop in-dimension; coexists cross-dimension | Entity loss/dupe classification | No survival trigger here |

Audited safe (negative results): villager death, zombification (release precedes
conversion in the die() chain), witch conversion, curing; portal phantom POIs
(block recheck); POTENTIAL_JOB_SITE persistence (has codec in 26.3); all
PoiManager.release callers guard existence (the throw is unreachable in vanilla paths).

## 6. Experiments (official server, deterministic markers) — TODO

Plan: bridge sandbox for R3 window measurement; disposable official-server world +
RCON-driven Rust scenarios for R1/R2/R4/R7 (markers: Brain memory NBT via data get,
VillagerData profession, trade restock = MerchantOffers uses reset after WorkAtPoi,
sleeping pose, portal POI/block counts); one controlled save/rollback for R5/R9
classification. Setup commands are scaffolding only; desync steps are survival actions
(break/place/piston/boat/portal/leash).
