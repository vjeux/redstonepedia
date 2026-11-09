# The farm optimizer

How the repo finds the best farms for a stage of the game, and how to plug in the next farm. The
design note (gate G1, approved 2026-11-08) is [designs/briefs/farm-optimizer.md](../designs/briefs/farm-optimizer.md);
the iron farm's mechanics are [farm-iron.md](../designs/briefs/farm-iron.md); the stage specs are
written by the stage-spec session in `designs/farms/<farm>/<stage>.spec`.

## The loop

    spec (designs/farms/iron/middle.spec)  ->  mctas farmopt run  ->  designs/farms/iron/middle/
      objectives, constraints, forks,             front.md, summary.md, ranked.tsv,
      space narrowing, merge threshold            one design file per front member

A spec is plain lines (`farm:`, `stage:`, `objective: max|min <metric>` (1-4), `constraint:
<metric> <=|>=|=|in <value>`, `fork: <param>`, `merge: <pct>`, `space: <param> <values>`, any
other `key: value` for the farm). The answer is a merged Pareto front per fork: members strictly
better than every other build of the space on the objectives, near-duplicates (within the merge
threshold of a better member on every objective) listed as its runner-ups, the bound per
objective and the gap. Every evaluation goes into a journal (`target/farmopt/<farm>-<stage>-
<space hash>.tsv`), so changing an objective or a constraint re-ranks without evaluating again;
only new candidates (a widened `space:`) cost runs.

## Commands

| command | what it does |
|---|---|
| `mctas farmopt run <spec> [--out DIR] [--journal F] [--no-full] [--quick-budget N] [--full-budget N] [--local R] [--seeds] [--threads N] [--forget=PART] [--exhaustive]` | answer a spec: static stage on every candidate, then best-first quick runs and full judges with pruning, `R` rounds of local search around the front; `--no-full` stops at the quick level (a preview); `--seeds` also evaluates the stage's exact seeds (calibration, never on a front); `--forget=PART` (repeatable) re-evaluates the journal's keys containing PART when the evaluator changed for those candidates only (a `forget` line keeps later opens in agreement); the full budget counts this run's judges; `--exhaustive` prunes nothing (every feasible candidate measured within the budgets: a part front, kept per interface) |
| `mctas farmopt count <spec>` | the space's size per fork |
| `mctas farmopt design <spec> <key> --out F` | one candidate's design file (any dug single module by its key too: any depth, any pit; `-sorter`/`-sorterc` for a void module's sorter storages) |
| `mctas farmopt parts --stage S [--out DIR]` | the part catalogs (`designs/farms/iron/parts/<stage>-<part>.tsv`): every cage, pad, kill, calm part and collection on its own objectives, its Pareto members marked |
| `mctas farmopt parts-tsv <spec> <key> --out F [--card-minutes M]` | a grammar farm's parts (the site's `<stem>.parts.tsv`): every block by the part that placed it (the access and the storage by diffing the build without them, the calm its redstone, the cage, pad, kill, collection and transport by place), each part's blocks placed, dug and build seconds at the stage's prices with the items its block entities hold; the card's villager and zombie rows go to the cage, its lava trips to the kill, its travel, height and (given the card's minutes) the spot's rows by blocks placed, so the parts sum to the card |
| `mctas farmopt part-members [stage:key ...]` | the component catalogs a reference farm's parts fill (`designs/farms/iron/parts/<part>/members.tsv` and a design per member: storage, access, transport always; kill and cage only where the catalog lacks the member), each with its blocks, blocks dug, build seconds (storage: hours at 480 iron/h) and the listed farms whose design carries it |
| `mctas farmopt part-front <spec> --journal J --part P --out F` | a part front's table (iteration 20) from a part run's journal: each member judged in full, its own blocks, dug and seconds (the split), its farm's iron/h and time, the kill's mean and longest time, iron burnt a golem, the share of summon tries that found no spot, a fire warning; the part's front marked; the family's bound (the game's rules: lava 240 gt; fire charges, Harming II, anvils and slime launches named as not simulated) and the measured gap |
| `mctas farmopt cages <spec>` | every distinct cage of the space on its own (iron brief iteration 17): its irregularities counted (each pod whose front differs from the watchers', each villager outside the watchers' row or a level beyond the first by its bed, the zombie's window not in front of the watchers, the bed plate not beside the window's trapdoor), its bounding box and its blocks |
| `mctas farmopt day1-rooms` | the regular day-1 bed-plate room searched by its sight lines (iteration 18): U1 walled in the watchers' row at their level, one or two plates in its room, the zombie anywhere ahead and below (pits included), fence gates and trapdoors in the cells the plates and the blocks under them power, each ray checked against the crossed cells' collision boxes; D4 as the positive control |

### Part fronts (iteration 20)

A part front pins the stage's reference farm (the pick) with `space:` lines and leaves one part
free; `--exhaustive --local 0` measures every member, and `part-front` prints the table with the
part's own numbers beside its effect in the farm. The day-1 runs use dust #2 (pulsewr-s2b0, pit
21, dug 9) for the redstone fork and the bed-plate cage at dug 11 for the other; middle and late
use the 2-watcher regular module (obs0wr-s2b0, blade 22). Measured so far: the kill's mean is
309-343 gt against lava's 240 gt bound (a 29-30% gap: the dwell before a golem meets the lava, not
the burn), the blade the cheapest middle kill, the pit 21 the fastest out-of-range day-1 kill;
48-49% of summon tries find no spot (the pad, the iron gap's cause).

### Storage and access

Every day-1 dug single module has its access built in (the day-1 lane's spiral staircase: a lit
2-high corridor from the chest, bottom cobblestone-slab steps, a torch every 8 layers, closed fence
gates as its hatch), placed west, east or south of the chest where all its cells are free, 2+ from
lava and water and walled from the farm's open cells; `surface_blocks` exempts its top two layers.
Middle and late modules store in a horizontal row (vjeux 21:06): one hopper under the judge's chest
into a double chest beside it (11.8 h of one module). A shared chain of 2+ modules (the shared-kill
stacks) uses the cheap sorter and iron-block crafter (the ironfarm lane's layout with filters of 41
items and 4 plain sticks, empty lock hoppers: 1,110 s at middle against 1,786 s with named paper).

## The search

1. **Enumerate** the farm's space under the spec (`space:` lines narrow parameters; stage
   material limits are grammar constraints, so only valid builds are generated).
2. **Static stage** on every candidate, in parallel (`FARMOPT_THREADS`, default the cores less
   1/16): exact metrics (blocks, build minutes, the bill, materials, modules, volume, the farm's
   bound inputs) or a refusal with its reason. Exact constraints drop infeasible candidates here.
3. **Bound**: the farm's proven optimistic value of every objective (for the iron farm: the 600 gt
   ceiling per cluster, the mark-success term from the exact spawn cells, the sure sightings of an
   in-sight kill; the brief's "The bound").
4. **Best first**: the open candidates ordered by their optimistic vector (Pareto layers, the
   objectives taking turns); the best unmeasured get quick runs (a batch per round), the best
   measured get the full judge, and every second quick batch is followed by a full batch on the
   best measured (a loose bound would otherwise keep unmeasured candidates on top for the whole
   quick budget); once the full budget is spent the quick runs go on over the unmeasured, so their
   measured upper ends replace their bounds in the gap. After every round,
   any open candidate whose optimistic vector is weakly dominated by a measured candidate's
   pessimistic vector (the lower end of its 95% interval) is pruned: it can never be strictly
   better. A candidate the full judge then refuses reopens what its quick runs pruned.
   Candidates are one per key (a grammar reaching one build two ways evaluates it once).
5. **Local search**: the front members' geometric neighbours (outside the enumeration too) go
   through the same stages, `--local` rounds.
6. **Answer**: the measured front per fork (full-judged, feasible), merged greedily around the
   best (sorted by the first objective, ties to the farm's tie metric; the best takes every member
   within the threshold of it on every objective, then the best of the rest); the ideal point and
   the gap per objective; what stayed open (undecided when a budget ended) counts in the gap.

## Plugging in a farm

Implement `farmopt::Farm` (crates/farmopt/src/lib.rs) in a crate of the farm (the iron farm:
crates/farmopt-iron), and add its arm to `mctas farmopt`'s dispatch:

- `type Cand`: the farm's parameter vector; `key` (canonical, stable), `param` (a parameter in
  words: forks and reports), `params` (the grammar's parameters and their values);
- `enumerate(spec)`: every valid candidate under the spec; `neighbors`: local-search moves;
- `design`: the candidate as the judge's design file;
- `eval_static` / `bound` / `eval_quick` / `eval_full`: the staged evaluation, each returning
  metrics (`Value`: exact, or a mean with its 95% interval) and text lines for the card;
- `version`: bump it whenever the grammar or the evaluation changes (old journal lines are then
  ignored); `tie_metric` (the merge's tie-break) and `cards` (the stage's card rows printed first
  in front.md) are optional.

The core never looks inside a candidate: it only compares metric vectors, so the next farm's
objectives (XP per hour, crops per hour per block, trades) need nothing new in it.

## The iron farm's grammar (version 2)

Parts, each generated from the ironfarm lane's generators (crates/mobfarm) and written into one
module frame (crates/farmopt-iron/src/parts.rs): the cage (1-cell pods, scared and blind
villagers, the zombie in its boat, the hide; the bed plate or a bed observer, U1 east or west),
the dry-hole pad (n 6/8/10, floor -7..-5, shifted), the kill (the pit corner at feet -13..-23 or
the fire-safe blade at -19..-25), the collection (the kill's hoppers to a chest), the calm part
(the smallest catalog timers legal for the stage, placed and routed by `mobfarm::parts`), the
tiling (N modules in a row at pitch 17-27, each with its own calm or one calm routed through every
module), the placement (the void, dug in at village-edge cell by cell 3-8 below ground, or the void
build cleared into the surface), the L3 family (lava spawn pits with the golems' tripwire
calm), and the ironfarm lane's `stack` (modules upward, each with its own chests) and `row` (one
shared sorter, iron-block crafter and composter) generators as farm families. Day 1 also has the
dug blade (the drop and the fire-safe blade cut into the rock: M3's family), 5-villager cages (the
blind pods' sides closed at bed height, so no two sleeping villagers see each other) and two
clusters side by side within the village's 8 villagers. Exact seeds
(`--seeds`: D1, D4, D1U, L3, M2-open, M3, M4, LB, LS-open) are calibration points.
`mctas farmopt parts` measures each part on its own; `mctas farmopt count` prints the space's size.

The shared kill (vjeux 2026-11-08 14:50, on the stack of copies: "why does each module have their
own kill chamber?"): grammar modules stacked with their pad holes aligned, every upper module's
golems falling down one shaft through the pad space of the module below into ONE fire-safe blade
and ONE collection at the bottom (family `stack-shared-kill`, pitch 26-38). The shaft is a ring
every third layer (a golem, 2.7 tall, cannot leave through a 2-high gap); the upper shafts' rings
stop 7 above the lower beds, because a ring inside the lower villagers' spawn scan (y - 6 .. y + 6)
ends their scan on its top (SpawnUtil takes the first surface from the top) and starves that
module (measured: half the rate with rings all the way down). The collection holds the judge's run:
one chest per two modules, chained down from the output chest (the judge's port stays a single
chest). Measured in the void: 2 modules at pitch 30, 425 iron/h per module (89% of the ceiling),
0.98 iron/h per block; at pitch 26, 419 and 1.01; 4 modules at pitch 30, 369 per module (77%: the
bottom module sees every upper module's falls) and 0.90. A shaft walled whole costs about 20 blocks
a layer (0.61 iron/h per block at N = 2): in the open air a long walled shaft costs more than the
~150 blocks and 4 lava of a second blade chamber. Dug into plains it is not in the grammar yet:
the sample holds 48 layers under its ground (caves at y 22), so N = 2 at pitch 15 at most, and an
observer cage cut into rock needs its cavities capped in the terrain's world.

Every villager is a summoner for the stray-cell check (the day-1 lane's D1U escape: a blind
villager panics at a natural hostile and its search lands wherever it reaches); the rate's bound
still counts only the villagers that panic at the farm's own zombie.

Materials (vjeux 2026-11-08 13:09): every wall a player places is cobblestone (stone needs a smelt,
glass a smelt, leaves shears), every slab a cobblestone slab, the fire shield over the pit's signs
oak buttons, spawn-proofing from torch caps the judge's spawn scan checks until no stray golem
cell is left, and every middle or late build carries a lightning rod on a free top corner of its
box (no villager is ever struck). Build minutes are the judge card's own bill
(`circuit::mobfarm::site_bill` over `redsim::entity::mob::prices`, designs/farms/prices.tsv): the
same numbers as the card's `build minutes`, the site's seals, caps and stair included.

The calm is placed jointly with its cage (`calm::wire_direct`): the timer's output repeater powers
the hide's trapdoor from beside it (no dust), the timer sits behind it clear of every scared
villager's line to the zombie (rays from every corner of the villager's cell), the observer and the
repeater that stretches its pulse stand in line behind the timer's input, and U1's pod goes where
that line ends (in the cluster, out of the zombie's attack box): the smallest Nether timer wires in
18 blocks (its own 15), 42 with U1's pod, against 64 + the pod routed. A shared calm still routes
(`calm::share`). Day 1's dug cages carry the day-1 lane's sleeping-sight fix (the floor in front
of each scared pod dug two deep under top slabs, a wall torch): golems come at night too.

The copies bar printed beside every middle and late front is copies of the best single module of
the space (the spec's `bar: <key>`, default the grammar's M2-open-ring module) at the candidate's
own N, built exactly like the candidates, judged in full, priced at that N (villagers by their
cheapest route at N, the volume of the N copies' most compact uncoupled box): a member must beat
it by more than 10% on one objective without losing more than 10% on another, and the search
prunes by it (`bar: none` for a measuring run).

Horizontal sharing, measured (vjeux 17:54-18:24: "build the villager chambers around the kill
chamber", "a + layout", "or maybe a X"): the ring (family `ring-shared-pad`: 2 or 4 cages turned
around one pad, hole, kill and collection, each pulled `d` from the pad) is coupled by design:
every group sees every golem on the shared pad, and with 4 groups each reaches only the pad's
nearest row (N = 4: 73 iron/h a group; N = 2 facing across one 6 x 6 rim: 282 a group, -31%
against copies). The + (family `cross`: the ironfarm lane's `cross_channel`, four modules at
radius r around one deep junction kill, r 12-24) matches copies' rate from r 21, where
neighbouring arms leave each other's axis-aligned box (r 13-14: 296-297 a module; r 18: 400;
r 21-24: 444), saving 6-10% build minutes for 10-50% more blocks.

Day 1 underground (vjeux 18:02: "Don't put slabs, do it below the surface. And put the chute to the
side"): `surface_blocks` counts the design's blocks and the site's covers at or above the natural
surface of their column (caps in caves below it are blocks like any other), and a spec's
`constraint: surface_blocks <= 0` keeps only farms that place nothing on the surface (ground 9-16:
the bed-plate cages need their plate villager's search under the surface). The chute to the side
(`Kill::Side`): the hole drops straight below every villager's follow cube (the arm at the cube's
floor, -20, or -24; or shallow, 3 under the pad, its golems riding inside the cubes), then a 4-wide,
4-high water arm runs south away from the pods (a golem rides up in the water: 3 high pinned it on
a lip; the steps are counted from the source so each stretch's water spills over its step: a step 7
cells from its water is dry and pins a still golem) to a gate-and-lava kill beside the farm, beyond
18.84 of every villager, with one hopper (a trough past the kill, lava over it, its water carrying
every drop to the hopper) or a hopper under every cell of its far row. Measured (bed-plate, 11
deep, 16-cell arm, AI on): from -20 or -24 334-335 iron/h; shallow 268 (80%); one hopper 334, build
45.8 min, T 50.0 with travel.

Every build's price includes iteration 16's travel and height (`prices::add_travel` in
`site_bill`: each trip's vertical travel by the design's own access, placing more than 2 above a
standable surface, the chest walks).

Regular cages (iteration 17): the regular observer cage puts U1 in the watchers' row at their
level, in a pod like theirs, out of the zombie's 8 blocks (0 irregularities; the 2-watcher module
408 iron/h, the same rate, +7% build minutes); the dust fork's row cage walls U1 in the row at the
watchers' level, its pulse part a level down (the route ends one east of the hide's feed and one
lower, then steps up): 1 irregularity (U1's walled pod) against 3. The bed-plate calm has no regular
room: `day1-rooms` finds no layout in which a watcher's standing ray crosses a cell the plate
powers (its hide must be beside the plate villager, and the only cells that see both are by the
zombie's window: D4's).

## The stages

| stage | objectives (spec) | space | what decides it |
|---|---|---|---|
| day 1 | min T (minutes from the first block to a 24-iron kit: the build with its iron and travel, then 24 iron; measured by the terrain judge), max iron/h | one unit (or two clusters) in village-edge: bed-plate and dust-fork cages dug 3-16 deep with pits, blades and the chute to the side, nothing on the natural surface; surface clear-box builds, the L3 family | the judge's measured T; the bound T >= build + max(24 iron at the bound's rate, 4 x 600 gt) |
| middle | max iron/h per module, min build minutes per module, at the farm's own N (N >= 2) | module rows (own calms or one shared calm), stacks, shared-collection rows; N 2, 4, 8 and the curve neighbours to 16 | beating the copies bar (N copies of M2-open-ring) per module |
| late | max iron/h per module (8-hour net), min build minutes per module, min volume per module | the same families from N = 1, stacks within the world's height | the same bar; storage and the 8-hour net |

A farm is its unit at every N (iron brief rev 9, "A farm and its N"): the front shows each farm
once, at the smallest N whose iron per hour per build minute is within 10% of its best, with its
curve (every judged N) printed under it.
