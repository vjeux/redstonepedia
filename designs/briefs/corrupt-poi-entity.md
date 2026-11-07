# Brief: POI & entity-store desynchronization (Lane 4 of 5 corruption audits)

Audit notebook with full citations: `designs/corrupt-poi-entity/NOTES.md`.
Game: Minecraft Java 26.3, decompiled source in `$MCTAS_HOME/versions/26.3/src`
(line numbers below are that listing's). No internet, no community designs; everything
derived from the game code. File-corruption *triggers* belong to the chunk-persistence
lane; this lane may use controlled save/rollback only to classify what a store mismatch
does once it exists.

## Use

Find survival-reachable ways for blocks, POI records, villager memories/claims, portal
records, entity sections, passengers/vehicles, leashes, UUID/persistence state or portal
cooldowns to become usefully stale — prioritising villager jobs/restocking, portal
linking, entity persistence/despawn, transport, raids and village/golem behaviour.

## The number

Each candidate is scored on (a) ticks-to-set-up by a survival player, (b) persistence
(same tick only / until validation / across save-load forever), (c) effect size
(blocks or villagers freed, portals duplicated, transport ticks saved). The bar is
vanilla synchronized behaviour (control in every experiment); the ceiling is a
permanent, save-load-stable rewiring of claims/links (e.g. workstation-free restocking).

## The mechanics, from the game code

- POI store: `PoiManager/PoiSection/PoiRecord/PoiTypes`
  (`world/entity/ai/village/poi/`). Workstations/HOME have 1 ticket, MEETING 32,
  NETHER_PORTAL 0 (`PoiTypes` 99-121). Tickets are anonymous persisted counts
  (`PoiRecord` 39-57, Packed 89-101) with no reconciliation sweep.
- Block->POI sync is **deferred**: `ServerLevel.updatePOIOnBlockStateChange` (1650-1666)
  enqueues remove/add as separate `getServer().execute` tasks. Rebuild on chunk load
  (`SerializableChunkData` 228 -> `PoiSection.refresh`, only when `!isValid`, reuses old
  records by position without type check, `PoiSection` 126-138). `PoiSection.add`
  overwrites `records` but not `byType` on type mismatch (64-80).
- Villagers: `AcquirePoi` ignores the `take()` result and sets the memory anyway
  (106-107); `ValidateNearbyPoi` only checks within 16 blocks and erases without
  release when the POI is gone (28-35); `AssignProfessionFromJobSite` reads profession
  from the *current* POI type on arrival (34-45); `WorkAtPoi` restocks on
  memory+distance with no POI check (21-43). Death/witch/zombie conversion all release
  correctly (`Villager` 589-634, 746-760; `Zombie` 400-413). HOME/JOB_SITE/POTENTIAL/
  MEETING memories all persist (`MemoryModuleType` 34-37).
- Village-ness is occupancy-derived: `PoiManager.isVillageCenter` (228-233) +
  DistanceTracker (292-322); VILLAGE tag = job sites + home + meeting. Raids centre on
  occupied POIs (`Raids` 116, `Raid` 287-291); golem summoning needs LAST_SLEPT < 24000
  (`Villager` 846-875, 924-927).
- Portals: search is POI-driven but block-rechecked (`PortalForcer` 41-50); a missing
  POI record means a duplicate portal is built (`NetherPortalBlock` 152-184).
  Cooldown (300 gt) refreshes while standing in a portal, persists in NBT and across
  copies/conversions (`Entity` 2690-2734, 2136, 3156); passengers get no cooldown from
  a vehicle teleport.
- Entities: per-dimension UUID set drops duplicates at load
  (`PersistentEntitySectionManager` 61-77); save roots skip passengers and
  exactly-one-player vehicles (`Entity.shouldBeSaved` 4105-4110 — those live in
  playerdata RootVehicle); cross-dimension copy is an NBT round-trip with passengers
  teleported first (`Entity` 3149-3225); leash data is UUID-or-BlockPos with **no
  dimension** (`Leashable` 89-109, 366-378).

## Candidates (the "layouts"): ranked desyncs to verify

1. **Break+replace bed/workstation under a distant claimant** — old villager keeps
   HOME/JOB_SITE memory, new record's ticket is free for a second villager. Bed variant
   can leak the ticket permanently via the bed-occupied validation race. Persists
   across save/load. (NOTES §1-2)
2. **Deferred-sync tick window** — same-tick workstation swap in front of a claiming
   villager (ghost job site, profession from the new block), and back-to-back same-tick
   portal teleports creating duplicate exit portals. Window length must be measured.
3. **Piston-pushed claimed workstation** (8 no-block-entity professions) — ticket
   resets at the landing cell, job can be stolen. Persists until re-validation.
4. **Store-mismatch phantoms (classification only)** — valid-but-wrong POI sections are
   never rebuilt: phantom workstation = profession + restock with no block; phantom
   portal POI is filtered by the block recheck (safe); type-mismatch place triggers the
   live-only `byType` phantom. Trigger owned by the chunk-persistence lane.
5. **Transport leftovers** — dimension-blind fence leashes re-anchoring at identical
   coordinates, passenger portal-cooldown bypass (dismount bounce), RootVehicle logout
   parking a villager's claims indefinitely.

## The decision

Verify 1-3 + 5 on the official 26.3 server with deterministic markers (brain-memory
NBT reads, trade restock counters, portal counts), bridge sandbox first for tick-level
questions (candidate 2's window). Candidate 4 gets one controlled rollback experiment
to classify effect only. No new judge/metric/kind; any Rust tooling is a scenario
driver under `mctas` conventions, gated by the usual push gate.

## Must-haves

- Every claimed sequence uses only survival actions for the desync step (commands only
  as test scaffolding: summon/teleport/NBT reads).
- Persistence stated per candidate: same-tick / until validation / across save-load.
- Negative results recorded (death/zombify release correctly in 26.3 — audited safe).

## Not goals

- New file-corruption triggers, update-suppression triggers (sibling lanes), client-side
  desync, anything needing commands/creative in the desync step itself.
