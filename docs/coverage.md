# Coverage: what each tool can and cannot find

Every claim of the form "none found", "UNSAT", "impossible" or "blocked" is bounded by the tool
that made it. This page says, per tool, what it models EXACTLY, what it APPROXIMATES and what it
does NOT model, with the evidence. Cite the rows you rely on when you write such a claim
(see [the rule](#the-rule-for-claims)). Each tool's owner keeps its column true.

Legend: **✓** exact (with evidence) · **~** approximated (the approximation is named) · **✗** not
modeled / not searched · **—** not applicable · **?** not yet audited.

## The tools

| tool | what it does | semantics (how it predicts the game) | search space | exhaustive? |
|---|---|---|---|---|
| judge (`door verify`) | checks one design | **the real game** (26.3 server code in the bridge) | — | — |
| redsim (`door redsim-verify`, `cosearch eval`) | checks one design, ~1000x faster | **exact model**: 0/100,000 random redstone circuits, 0/100,000 random piston setups (`door pistfuzz`: every block, moving block entity, queue size, drop and the idle flag after every tick), 0/20,000 random container circuits (`redsim-fuzz fuzz --gen containers`: droppers/hoppers with items, comparators), 328/328 archived doors (2 cycles) and 72/72 gadgets identical tick by tick, 0 verdict disagreements on 287 designs, 2049/2049 with near-miss mutants (cosearch agree); block equivalence classes (`redsim::classes`) proven in the game (`redsim-fuzz classfuzz`: 3000 circuits vs class-swapped variants, 0 differ); falling blocks only **~** (row below); item entities ✗ (excluded) | — | — |
| local search (`minimize`, `harvest`, `grow`, `regrow`, `synth`, `resynth`, `route2`) | improve a working design | candidates judged by the **real judge** | edits of a few blocks near the design; palettes below | no (local) |
| mechanism catalog (`door families`) | every flat seamless mechanism per box | **~** macro piston model (PistonStructureResolver port: push limit 12, sticky pull of the block two ahead, retracted pistons movable, extended pistons / heads / obsidian immovable; one settled action at a time, a 2 gt pulse = push-and-spit or extend-into-gap-and-pull; quasi-connectivity coupling since v3; no redstone), then each entry replayed with ideal inputs in the real simulator (`sim` column; v3: 85 oc + 44 sym schedules WORK, 68 entries have no ideal plan) | in-plane; <= 11 pistons facing E/W/U/D (sticky/normal, retracted/extended at rest); no helpers; no slime; the center leaves straight in exactly 2 moves, the edge on its line in 2, the 7 others in 1 (`--two N`: N of them in 2); a mover = a sticky puller 2 ahead or a pusher 1-2 behind; a piston reused at rest or lifted 1 cell along its axis by one lift. Run only on some boxes: `--carry N` (a reused piston displaced 1-2 cells in ANY direction by a carrier, carriers carried up to depth N: 9x6/6x9 with N = 1, P <= 12; 7x7 / 7x8 generate 1.9M / 2.6M candidates, not solved) and `--vmid any` (ML or MR leaves vertically: 7x8, P <= 10: 0 found, 13032 unresolved at a 50k cap); minimal mechanisms only | within its bounds, except "unresolved" candidates (pulse graph over 300k states and A* found nothing; 7x7: 1639, of which a 5M-state re-check proved 93 have no open state, 503 reach an open state with no close found, 1043 still over the cap) |
| fullmech (`door fullmech`, pistmodel) | mechanisms solved exactly, exact `door ideal` plans exported | **✓** every transition is redsim; inputs are ideal but physical: a redstone block behind the piston's cell when switched on (it powers that piston and, by QC, the piston right below; it stays put when the piston is carried), exactly what `door ideal` replays (32/32 exported 7x8 plans pass it); actions: k-tick pulses (`--pulses`, default 1 = spit, 3 = push-pull), level toggles (pistons extended at rest; all with `--levels`), overlapping pulse pairs of pistons <= 3 cells apart (`--pairs`); every other action starts on a settled state | families' generator (the catalog's bounds, <= 10 pistons by default; `--vmid ml|mr|any|both`: its vertical-middle candidates); `--reduce`: greedy input minimization, then one helper (redstone block, or observer facing E/W/U/D) in each free cell; `--anneal`: random mechanisms anywhere in the box (`--helpers`, `--slime`); `--check`: the mechanism of a design | no: A* per candidate with a 30k-state cap (7x8: ~1500 of 3448 capped), the close tried from the first 6 open states found (`--check` then falls back to one A* over open-then-close, 60k states); an uncapped "none" covers only the action set; `--anneal` samples |
| topology (`door topology`) | rank mechanisms by wirability | **~ heuristic, NOT sound**: wiring regions, piston links, pulse parity, memory links >= 7 cells, no-feed filter; its parity premise is false (observer busy window, mechanics.md section 8) | — | no: a ranking only |
| satwire (`door satwire`) | wiring for a FIXED mechanism | **~** time-unrolled SAT encoding (details in tracks/satwire/NOTES.md "COVERAGE"); every SAT model is screened by redsim and judged by the real judge (CEGAR), so a FOUND door is real; an UNSAT is only relative to the encoding | free cells of a box; palette below; one lever among candidates (outside the box or off-plane allowed); recorded piston power per tick with a time warp (stalls/skips) | yes within palette + box + encoding |
| cosearch (`door cosearch search`) | joint mechanism + wiring search | **✓** acceptance: the redsim judge (32 positions x 3 cycles, FULL seamless, hallway), then the real judge (`--confirm`); guidance: **~** a one-position, 2-cycle screen with partial credit per door cell and phase (a door slower than `--max-ticks` 160 per phase is screened out; the screen's ring check skips the hole rule, the final check has it) | the door plane only (pistons face E/W/U/D, nothing off-plane), every cell of the box (or `--focus` / `--focus-cells`): palette below; moves: set/remove/re-face/move/swap, repeater delay, comparator mode, carried redstone block or observer in front of a piston, piston add/move/re-face/extend toggle (`--lock-pistons` keeps the seed's, `--no-mech` forbids all), lever moves (`--lever-moves`) | no: simulated annealing + exhaustive enumeration of every edit of <= k focus cells (`--enum-k`, k <= 5; e.g. all 56^4 fillings of 4 cells in 103 s on 40 threads) |
| boxproof (whole-box SAT) | prove or find a door in a box | **~** redsim's semantics in one plane, phase-parallel with race FLAGS where the game's order could matter, written once over a Boolean algebra (bools = reference, CaDiCaL = solver); validated against redsim tick by tick on random boxes (blocks, moving-block entities, comparator outputs, pending ticks, pending block events): 100,000 random 5x5 boxes with the full palette (bulbs and comparators included, seeds 11-15) -> ~42,700 compared, 9 unflagged mismatches (tracks/boxproof/NOTES.md); e89b re-found from its placement; models screened by redsim then judged (CEGAR) | every cell of a W x H x 1 box free (or fixed: `--fix`, partial: `--fix-all --free`), palette air/stone/obsidian/glass/redstone block/dust/repeater/comparator/observer/torch/wall torch/note block/lamp/waxed copper bulb/piston/sticky piston, one lever (box, outside, off-plane), T ticks per edge, FULL seamless (`--frame12`: the 12 frame cells stone when open) | exhaustive over that space and horizon; an UNSAT means "no door whose run raises no flag" (`--prove`, UNDER-approximation) or, with `--nondet`, that races may go either way (sound for the relaxed kinds; the other flags still forbid): not yet a theorem while 9 / 42,700 fuzz runs mismatch unflagged |

## Game behaviors (rows from redsim's source inventory, Minecraft 26.3)

Flat doors only below; "flat-impossible" marks behaviors that need a block outside the door plane.

| behavior | redsim | local search | catalog | fullmech | satwire | cosearch | boxproof |
|---|---|---|---|---|---|---|---|
| **Engine** | | | | | | | |
| neighbor updates (order W,E,D,U,N,S; who updates whom) | ✓ | ✓ (judge) | ✗ | ✓ | ~ every receiver re-reads its input each tick | ✓ | ~ exact WHO-updates-whom patterns (setBlock 6 neighbors, dust/torch distance 2, diode/observer front + its neighbors, lever own + support, piston moves per moved cell/arm/base, head forwards); ORDER inside a step not modeled: parallel + race flags (prove mode forbids flagged runs: UNDER) |
| shape updates (observers, dust shapes, note instrument) | ✓ | ✓ | ✗ | ✓ | ~ observers see recorded mechanism changes and wiring states per tick | ✓ | ~ observers see every state change of the watched cell made by the step (incl. moving blocks, note instrument from the block below); dust SHAPE changes without a power change not modeled (UNDER) |
| changes made BY shape updates notify no neighbors (`updateFlags & -34`); flags 2 vs 3 per component | ✓ (fixture shape-update-no-neighbor-updates) | ✓ (judge) | ✗ | ✓ | ? | ✓ | ✓ (only instrument / head-pop changes come from shape updates; neither notifies) |
| tick phases: scheduled block ticks, then block events, then entities, then block entities (moving pistons, hoppers) | ✓ (`World::tick` = ServerLevel.tick) | ✓ (judge) | ? | ✓ | ? | ✓ | ✓ repeater ticks, then observer/torch/lamp ticks, then block events in rounds, then moving blocks (fuzz vs redsim) |
| scheduled ticks: priorities, FIFO, dedup (`willTickThisTick`) | ✓ (per-chunk queues merged by trigger, priority, sub-tick like LevelTicks) | ✓ | — | ✓ | ~ phases (repeaters, observers, torches); no repeater-priority races; dedup exact | ✓ | ~ dedup and willTickThisTick exact; repeaters before priority-0 ticks exact; order among same-phase ticks parallel, a race when one changes another's input (flag) |
| block events, same-tick (0-tick) transients | ✓ (FIFO + dedup; events queued during the phase run in it) | ✓ | ✗ | ✓ inside an action; two inputs in one tick only with `--pairs` | ✗ observers miss intra-tick transients | ✓ | ~ events queued during a round run in the next round (2 rounds modeled; leftovers flagged); order inside a round parallel (write/power conflicts flagged) or, with `--split-rounds`, two sub-rounds whose order the solver picks (needed by the community 7x9; over-approximates the game's fixed queue order: CEGAR rejects the wrong orders); transient dust glitches inside a step not modeled |
| block-entity ticker order (`blockEntityTickers` + pending; a rebound wrapper keeps its slot) | ✓ moving pistons; hoppers tick before moving blocks (exact while a design's hoppers exist before its first moving block) | ✓ (judge) | — | ✓ | ? | ✓ | ~ parallel finalization (no flag yet for order-dependent placements: one unflagged fuzz mismatch class) |
| dust update order (BlockPos hash, position dependent) | ✓ | ✓ | — | — (no dust: no wiring) | ✗ (the judge's 32 positions catch it) | ✓ | ~ not modeled (one parallel semantics; order effects only through flags); exact where flags are off |
| quasi-connectivity as BUD (armed, fires on the next update) | ✓ | ✓ | ~ coupling only (v3): a retracted unpowered piston right below a powered one follows it (pulse while the upper one is out, level held / released); QC from the wiring, and arming without an update, not modeled | ✓ | ~ the firing update is assumed | ✓ | ✓ pistons re-check power only when updated (exact patterns) or placed; a check that sees dust not yet settled is flagged |
| comparator updates from changed containers / analog sources (`updateNeighbourForOutputSignal`: horizontal comparators, and through one conductor) | ✓ | ✓ (judge) | — | — | ? | ✓ | ✓ bulbs (the only analog source in the palette): comparators next to it or behind one conductor, east and west, get an update on every bulb change |
| item drops of popped / destroyed blocks (none for piston heads and moving pistons) | ✓ (recorded and compared by pistfuzz) | ✓ (judge) | — | ✓ | ? | ✓ | ✓ as a REQUIREMENT: any push that pops/destroys, any support lost, anything pushed out of the box is forbidden (a drop is an entity: the judge fails), sound |
| **Components** | | | | | | | |
| dust (levels, shapes, steps, strong/weak) | ✓ | ✓ | — | — | ✓ (cross dust only) | ✓ | ✓ levels 0-15 (unique fixpoint, step diagonals, weak/strong) / ~ shapes: cross/line only (in one plane every dust powers E, W, down), dots ✗ |
| repeater delays 1-4, pulse stretching | ✓ | ✓ | — | — | ✓ | ✓ | ✓ (E/W facings; pulse stretch self-schedule) |
| repeater locking, comparator side inputs | ✓ | ✓ | — | — | — flat-impossible | — flat-impossible | — |
| comparator (rear input, 2 gt, reads bulbs) | ✓ | ✓ | — | — | ✓ | ✓ | ✓ compare mode, E/W (in the plane its sides face out of it, so output = rear input): levels, bulbs read as 15/0 directly or behind one conductor, HIGH/NORMAL tick priority, front update on every tick (fuzz, 4c73b6b); subtract mode ✗ |
| comparator reading container fill levels (dropper/hopper flip-flops) | ✓ (fill formula with the game's float math; 0/20,000 container circuits; fixture hopper-t-flip-flop) | ✓ (judge) | — | — | ✗ | ✗ | ✗ |
| other analog sources: cake bites, composter level, end portal frame eye, respawn anchor charges, copper bulb (15/0), crafter (empty) | ✓ (fuzz family `analog`) | ✓ (judge) | — | — | ? | ? | ✗ |
| container item transfer without item entities: dropper -> facing container (4 gt, its single stack), hopper push + pull above (8 gt cooldown, the wasEmpty 8/7 rule, locked while powered) | ✓ (0/20,000; hopper removeItem/setItem do not notify: fixture hopper-remove-no-setchanged); a dropper with 2+ stacks (random slot) and item entities ✗ (flagged) | ✓ (judge) | — | — | ✗ | ✗ | ✗ |
| observer (2+2 gt, on any watched change incl. moving blocks) | ✓ | ✓ | — | ✓ as a helper (`--reduce`, `--anneal --helpers`) | ✓ (watches wiring and mechanism cells) | ✓ | ✓ (pushed observer facing its destination leaves a stale tick; placed observers fire / turn off; turning-off race flagged) |
| torch / wall torch, burnout | ✓ | ✓ | — | — | ✓ / burnout ✗ (CEGAR catches) | ✓ | ✓ / burnout ✗ (a door toggling a torch 8x in 60 gt is outside the claim) |
| redstone lamp (4 gt off) | ✓ | ✓ | — | — | ✓ | ✓ | ✓ (lit at once on update, off-tick 4 gt, its own tick phase; fuzz) |
| waxed copper bulb (rising-edge toggle) | ✓ | ✓ | — | — | ✓ | ✓ | ✓ (not a conductor; powered follows its input at once, a rising edge toggles lit; moved with its state; fuzz, 4c73b6b) / ~ an input that falls and rises within one phase (two neighbors) is seen as no edge (1 fuzz case in 42,700) |
| note block | ✓ (instrument from the block below, mob head above: fixture note-under-wall-skull) | ✓ | — | — | ✓ | ✓ | ✓ powered at each neighbor update + 6-neighbor update, instrument from the block below (no mob heads) |
| target block | ✓ | ✓ | — | — | ✗ | ✓ | ~ = stone in a plane (no projectiles); not a separate symbol |
| iron / oak trapdoor, doors, fence gates | ✓ | ✓ (trapdoors in the default resynth/regrow palette) | — | — | iron trapdoor ✓ | iron trapdoor ✓ (E/W, bottom/top); oak trapdoors, doors, gates ✗ | ✗ |
| dropper / dispenser (immovable, QC power sensor) | ✓ (dispensers' item behaviors spawn entities: ✗, flagged) | ✓ (synth palette) | — | — | dropper ✓ | dropper ✓ (U/D/E/W); dispenser ✗ | dropper (empty) ✓: immovable conductor, triggered = own input or QC at once (flags 2, observers see it), dispense tick +4, analog 0 for comparators (fuzz: 24 unflagged in ~48k runs, QC update-order cases); dispenser ✗ |
| hopper (enabled state, items), crafter | ✓ hopper items (row above); crafter: empty grid only | ✓ (judge) | — | — | ✗ | ✗ | ✗ |
| powered / activator rails | ✓ | ✗ (not in palettes) | — | — | ✗ | ✗ | ✗ |
| slabs (top/bottom), non-conducting full blocks (glass...) | ✓ (slime conducts, honey/leaves hold nothing: fixtures slime-is-a-conductor, leaves-honey-hold-nothing) | glass ✓, slabs ✗ | — | — | glass ✓, slabs ✗ | glass ✓, smooth stone slabs ✓ (top/bottom) | glass ✓, slabs ✗ |
| cake (needs a solid block below: pops off diodes, slabs...) | ✓ (fixture cake-on-unsupported-repeater) | ✓ (judge) | — | — | ✗ | ✗ | ✗ |
| leaves + log (a 1 gt delay) | ✓ | ✓ (grow palette) | — | — | ✓ (`leaves`, `--log-doors`) | ✓ (persistent oak leaves, oak log x/y) | ✗ |
| scaffolding (a 1 gt delay per block: distance recomputed 1 gt after any shape update; breaks at 7, falls when placed at 7) | ✓ (0/30,000 scaffold circuits; with pistons 0/10,000; falling scaffolds 0/10,000; fixture scaffold-delay = designs/gadgets/scaffold-delay; link table rows); a FALLING scaffold pushed or lifted by a moving piston ✗ (entities moved by pistons, pistmodel's open item: 13/10,000 circuits that have both) | ✓ (judge) | — | — | ✗ | ✗ | ✗ |
| buttons; lever | ✓ | lever | — | — | lever | lever ✓ (moved by `--lever` / `--lever-moves`); buttons ✗ | lever ✓ (in the box, just outside it, out of the plane; its support must stay put; either polarity at rest: `--lever-on`), buttons ✗ |
| redstone block as static wiring | ✓ | ✓ | — | ✓ (`--reduce`, `--anneal --helpers`) | ✗ | ✓ | ✓ (and carried by pistons) |
| **Pistons and moving blocks** | | | | | | | |
| pistons in the mechanism | ✓ | ✓ | ✓ (macro, settled actions; replayed in the simulator) | ✓ | fixed input | ✓ | ✓ free in every cell (whole box) or fixed (`--fix`): 4 in-plane facings, sticky or not, extended at rest with its head |
| piston heads forward neighbor updates to their base; a head without its base pops (no drop) | ✓ (pistfuzz) | ✓ (judge) | ✗ | ✓ | ? | ✓ | ✓ forward / ✓ a head whose base no longer fits vanishes |
| a retracting base / moving piston keeps the extended box as support (a lever or repeater on its back survives) | ✓ (`piston::sturdy_at`; found by the archived-door replay) | ✓ (judge) | ✗ | ✓ | ? | ✓ | ~ moving cells are never a support (UNDER: a lever/repeater on a retracting base's back is excluded); full faces of static pistons/heads exact |
| attachments popping when their support moves or breaks (torches, levers, buttons, dust, diodes, rails, doors, cake) | ✓ (fuzz Set steps, pistfuzz) | ✓ (judge) | ✗ | ✓ | ? | ✓ | ✓ as a requirement (forbidden: the judge fails); CENTER support (torch on a sideways base's top) ✗ (UNDER) |
| update pistons (push nothing; block updates / observer triggers) | ✓ | ✓ (judge, if a move adds one) | ✗ (minimal mechanisms drop them) | ~ only `--anneal` places one | ✗ (requested) | ✓ | ✓ (any piston anywhere, whole-box mode; fixed-pistons mode: free cells take the full palette, extra pistons included, normal and sticky, any in-plane facing, since 01:51Z; runs started before used wiring kinds only) |
| pistons moving pistons | ✓ | ✓ | ~ solver: any push line or sticky pull may move a retracted piston, along or across its axis (the "rider shuffles"); generator: lifts 1 cell along the axis; `--carry N` (1-2 cells any direction, carriers carried to depth N) ran on 9x6/6x9 only | ✓ semantics: any carry the candidate's pistons reach (the carried piston keeps its identity and input, lands and fires exactly); candidates: the catalog's | — | ✓ | ✓ (retracted pistons pushed and pulled; moved pistons re-check on placement) |
| spitting / block dropping on short pulses | ✓ | ✓ | ~ 2 gt pulse action (push and spit; no block dropping) | ✓ (1 gt pulse; exact event and finalTick rules) | recorded | ✓ | ✓ (retract-and-drop event, final tick of the block two ahead, retract while the head is still moving: fuzz) |
| 0-tick pistons | ✓ | ✓ | ✗ | ~ only through helpers moved by pistons (`--reduce`, `--anneal`): ideal inputs switch between ticks; `--pairs` starts two pistons in one tick, which is not a 0-tick | ✗ | ✓ | ~ chained events within a tick: 2 rounds (e89b needs 2); more flagged |
| helper blocks (stoppers, line fillers) | ✓ | ✓ | ✗ (the model has stone / obsidian pieces; the generator places none) | ~ `--anneal --helpers` (stone, obsidian, redstone block, observer); not in the generator | — | ✓ | ✓ (any block of the palette anywhere) |
| carried power sources (redstone block, observer) | ✓ | ✓ | ✗ (2D catalog); rows: `--rblocks` (strip agent) | ✓ semantics; searched: one helper per mechanism (`--reduce`), `--anneal --helpers` | ✗ | ✓ | ✓ (redstone blocks, observers, note blocks, lamps carried; a moving block powers nothing) |
| immovable blocks (obsidian, crying obsidian, block entities) | ✓ | ✓ | obsidian, extended pistons, heads (no other immovable in the palette) | obsidian | obsidian | ✓ obsidian, crying obsidian, droppers | obsidian ✓, extended pistons / heads / moving ✓ |
| push limit 12, glazed terracotta | ✓ | ✓ | ✓ / ✗ (mech-synth has it, families' grid model not) | ✓ / ✗ (not in the palette) | — | ✓ / ✓ | ✓ / ✗ |
| slime / honey (excluded from the flat seamless target by vjeux) | ✓ | ✓ | ✗ (not in families' grid model) | ✗ (off; `--anneal --slime` only) | ✗ | ✗ | — |
| falling blocks (sand, gravel, concrete powder: gravity category) | ~ falls and landings; falling entities pushed by moving pistons or landing on moving blocks ✗ (`pistfuzz --gravity`: 2960/3000 before the failure cap) | ✓ (judge) | ✗ | ✗ | ? | ? | ✗ |
| containers: dropper/hopper item transfer, comparator fill level (flip-flops) | ✓ (redmodel 63a9c07: 20,000 container cases, 0 mismatches; dispensers' item behaviors, item entities and multi-stack droppers out of scope) | ✓ (judge) | — | — | ✗ | ✗ | ✗ |
| **Search-space features** | | | | | | | |
| plain blocks searched by equivalence class (`redsim::classes`: 11 plain symbols, instrument splits only next to a note block) | ✓ (classes derived from every property redsim reads; game-proven by classfuzz) | ✗ | — | — | ? | ? | ~ stone (conductor), obsidian (immovable conductor), glass (non-conductor) symbols; others ✗ |
| lever outside the box (not counted in the volume) | — | ✗ | — | — | ✓ | ✓ | ✓ |
| lever off the door plane | — | ✓ | — | — | ✓ (`--lever-oop`) | ✗ | ✓ (`--oop`) |
| timing chosen by the search | — | ✓ | sym/oc trains only | oc only (open and close sequences of actions on settled states, no symmetric train); pulse widths from `--pulses` | ~ time warp (pauses stretch/shrink, pulse widths fixed) | ✓ | ✓ (the solver picks the whole placement; every timing follows from the unrolled semantics, no plan) |
| different behavior on open and close | — | ✓ | ✓ (oc plans) | ✓ | ✓ (two traces) | ✓ | ✓ (two traces from one placement, open idle at T, close back to the exact rest state; `--periodic`: two cycles, C1 == R or C2 == R or C2 == C1, like the judge's periodic rest) |

Excluded on purpose everywhere (rules or categories): random ticks (unwaxed copper, crops, leaves
decay), weather and daylight, entities and player triggers (pressure plates, tripwire, item frames,
item entities: a dropper spitting into the world, hoppers catching drops), sculk (its own category),
fluids; analog sources that depend on them (jukebox, lectern, chiseled bookshelf, cauldrons,
beehives, daylight detectors).

redsim evidence: every ✓ in its column is exercised by `redsim-fuzz fuzz` (random circuits from the
row's palette family compared with the game block by block after every tick, 0/100,000 with
`--palette all`; `--gen containers` 0/20,000) and, for pistons, by `door pistfuzz` (0/100,000);
a named fixture is also a regression test in `crates/redsim/tests/fixtures/` (replayed by
`cargo test -p redsim`). Each component's source references are in its module
(`crates/redsim/src/redstone/*.rs`, `crates/redsim/src/piston/*.rs`).

## The mob layer (redsim, mob farms)

The iron farms' judge (`circuit: mob-farm iron_ingot TIER`, [circuits.md](circuits.md)) runs on
`redsim::entity::mob` only. What it models, approximates and refuses:

| behavior | mob layer | evidence |
|---|---|---|
| golem summoning: `GolemSensor` every 200 gt, `NearestLivingEntitySensor` every 20 gt, the 600 gt cooldown, panic and `LAST_SLEEPT` within a day, 3 villagers agreeing | ✓ | known answers (`entity::mob::tests`); game runs: L1 35.0 vs 35.7 golems/h (8 x 3 days), L3 72.8 vs 74.8 (8 x 6 days), L2d 111.7 vs 112.3 (3 x 3 days), 116.0 vs 115.2 (8 x 3 days), L5 462 vs 461 (2 x 3 days) |
| spawn spots: `SpawnUtil` legacy iron golem strategy (10 attempts in +-8 x +-6 x +-8, the column scan, excluded blocks, `checkSpawnObstruction`) | ✓ | the 82% / 43% known answers on a 4x4 pad; spawn-cell chi-square vs the game: 6.25 (8 cells), 23.24 (20), 49.30 (48), all under the 5% critical value |
| villagers: schedule, REST, sleeping in a bed, waking scared, the hostile sensor (a zombie within 8, line of sight from the eyes) | ✓ | game day/night splits 50/50 in every run |
| LivingEntity physics: water push (0.014, drift 0.056), gravity in water (/16) and lava, step height, jumping in liquids, fall | ✓ | known answers (drift, 15.5-block fall in 22 gt, rider travel) |
| lava damage and fire, golem death, loot (iron 3-5, poppies 0-2) | ✓ | 3.93-3.97 iron per golem in the game |
| fluids around farms: one source per corner, signs and fence gates hold no flowing water (fence gates no lava: 26.3 `canHoldAnyFluid`) | ✓ | known answers (`signs_stop_water_and_let_golems_through`, `open_fence_gates_stop_fluids_not_golems`) |
| daylight detectors following the day time | ✓ | known answer `daylight_detector_follows_the_day`; the dusk pulse in the game runs |
| golem AI movement (goals, navigation) | ~ bounded, not reproduced: every golem is run still and under 8 constant walking inputs (jumping when blocked) from every position it can stand on, edge spots included (a box 0.0005 over a floor's edge stands while the current, felt through the box deflated by 0.001, may stop), and the farm must kill it under every one | the exit flood and the 9 worst-policy runs; the first L1 failed in the game (golems stuck on its hole's rim) and the flood now finds that spot |
| zombie AI movement | ✗ refused: a zombie must ride (a boat), it never walks | static check |
| climbing (scaffolding, ladders), job sites, bunk beds, flowing lava, gossip between villagers that see each other, NoAI | ✗ refused | static checks |
| weather: `advanceWeatherCycle` (clear 12000..180000, rain 12000..24000, thunder 12000..180000 / 3600..15600), the levels (0.01 a tick), a run's start from the cycle's stationary state | ✓ (`redsim::weather`, mobinfra) | known answers (`weather::tests`: the cycle's shares 15.8% raining, the thunder flag 9.1%; the ramps; `weather clear`) |
| the sky darkening in rain (noon 3) and thunder (noon 5), feeding daylight detectors and the light the mob rules read | ✓ | known answer `darkening_in_rain_and_thunder`; the game gate's forced rain days (the dusk pulse farms stop in the game as in redsim) |
| rain and water putting burning mobs out (`isInRain`, the reset to -1 fire ticks) | ✓ | known answer `rain_puts_fire_out` |
| lightning: `tickThunder` (1 in 100000 a chunk a tick, the AFK player's spawning chunks), `findLightningTargetAround` (rods topping their column within 128, sky-seeing living entities in the column box + 3, the column top), `isRainingAt`, the bolt (life 2, 1-3 flashes, its box x+-3 y-3..+9 every tick, through roofs): villager -> witch, 5 damage and 8 s of fire, vehicles and drops destroyed, its fires (spawnFire, Hard), lightning rods; the Hard trap horse | ✓ (a trap horse on the farm, a fire beside a flammable block and a destroyed leashed entity fail closed) | known answers (`lightning_targets`, `lightning_rods`, `lightning_hits_through_roofs`, `lightning_fires`, `every_bolt_that_can_fall`); the judge's exhaustive check (`lightning::static_check`: every bolt that can fall on the build); the game gate's forced thunder days |
| zombie sunburn (`Mob.burnUndead` / `isSunBurnTick`: the day window, brightness > 0.5 from 12, sky at the eye, not in water or rain) | ✓ | known answer `burn_window_and_light_cut` |
| lava's fire (`LavaFluid.randomTick`, only with a player within 128) and `FireBlock` spread and burn-out | ✗ not yet modeled: the real-game gate catches it (iron-middle's fence gates burn by gt 2000 in the game) | the game gate |
| the real game: every build of a farm in the 26.3 server for 3 days of natural weather, a forced day of rain and of thunder (`circuit verify`, `mctas mobfarm gate`) | ✓ gate | iron-day1: 74.8 golems/h in the game vs redsim 75.0 (8 builds x 3 days), 4.06 iron per golem, works in rain and thunder |

## Gaps that could change a 7x8 / 7x9 verdict, by priority

1. **satwire UNSATs are not proofs** while the encoding approximates block updates (re-read every
   tick), QC-as-BUD, observer transients and repeater races, and lacks update pistons, redstone
   blocks, targets, slabs and leaves. Fix: encode updates exactly or use boxproof; until then an
   UNSAT means "no wiring in this encoding". **Worse, an UNSAT is relative to the IDEAL PLAN it was
   given** (sweep56's calibration): with families' plans, e89b's own mechanism is UNSAT in e89b's
   own 8x9 box, where its wiring exists, because e89b's real power trains (a held elevator level,
   1 gt pulses, other orders) are nothing like the plan. So "satwire UNSAT" says nothing about a
   mechanism until the requirements are plan-free (satwire's macro windows, sweep56's menus) or
   the timing is chosen by the solver (boxproof's fixed-pistons mode).
2. **The catalog's mechanism space**: no helpers, lifts of 1 cell along the axis (general
   `--carry` ran on 9x6/6x9 only), no 0-ticks, <= 2 moves per block (non-center blocks 1 unless
   `--two`), no carried sources, state-capped candidates (7x8: 1861 unresolved; 7x7: 1639, a
   5M-state re-check proved only 93 of them). The community 7x8 may sit outside it. fullmech
   inherits this space (it re-solves the same candidates exactly, ~1500 of 3448 7x8 candidates
   still state-capped); only its `--anneal` samples outside it. Track mechspace is building the
   exact solver for the capped candidates and the wider generator.
3. **cosearch** is not exhaustive (annealing; exhaustive only over <= 5 edited focus cells) and
   stays in the door plane; its palette gaps (obsidian, trapdoors, droppers, slabs, leaves/log,
   glazed terracotta) are closed (explore/cosearch 0dc82bf); still missing: oak trapdoors, doors,
   fence gates, dispensers, hoppers, rails, buttons, off-plane blocks.
4. **topology** is a heuristic: never cite it as a reason to skip a mechanism.
5. **redsim** container item transfer is in (droppers, hoppers, fill levels); a dropper holding
   2+ stacks (random slot), dispensers' item behaviors and item entities stay out (flagged when a
   circuit reaches them, never silently wrong).
6. **satwire bugs found by its users so far** (each one made UNSATs spurious): off-plane ideal
   inputs ignored (wire56, fixed), front-face signal counted into a piston (wire64, explore/wire64),
   lamp relay timing (flat43-full, open). Treat older satwire UNSATs as stale.

## The rule for claims

* A FOUND design counts only when the real judge passes (`door verify --extra-offsets 28`).
* A negative claim names the tool, its bounds (box, palette, piston count, moves, timing) and the
  rows above it depends on. "satwire UNSAT" never stands alone: it is "no wiring with palette P
  in box B under satwire's encoding".
* Only an exhaustive tool whose semantics are ✓ on every row the claim touches can state
  "impossible" (today: none for wiring; boxproof is the candidate).
