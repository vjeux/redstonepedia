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
| `mctas farmopt run <spec> [--out DIR] [--journal F] [--no-full] [--quick-budget N] [--full-budget N] [--local R] [--seeds] [--threads N]` | answer a spec: static stage on every candidate, then best-first quick runs and full judges with pruning, `R` rounds of local search around the front; `--no-full` stops at the quick level (a preview); `--seeds` also evaluates the stage's exact seeds (calibration, never on a front) |
| `mctas farmopt count <spec>` | the space's size per fork |
| `mctas farmopt design <spec> <key> --out F` | one candidate's design file (`mctas site preview F` puts it online) |
| `mctas farmopt parts --stage S [--out DIR]` | the part catalogs (`designs/farms/iron/parts/<stage>-<part>.tsv`): every cage, pad, kill, calm part and collection on its own objectives, its Pareto members marked |

## The search

1. **Enumerate** the farm's space under the spec (`space:` lines narrow parameters; stage
   material limits are grammar constraints, so only valid builds are generated).
2. **Static stage** on every candidate, in parallel (`FARMOPT_THREADS`, default the cores less
   1/16): exact metrics (blocks, build minutes, the bill, materials, modules, volume, the farm's
   bound inputs) or a refusal with its reason. Exact constraints drop infeasible candidates here.
3. **Bound**: the farm's proven optimistic value of every objective (for the iron farm: the 600 gt
   ceiling per cluster, the mark-success term from the exact spawn cells, the sure sightings of an
   in-sight kill; the brief's "The bound").
4. **Best first**: the open candidates ordered by their optimistic vector; the best unmeasured
   get quick runs (a batch per round), the best measured get the full judge. After every round,
   any open candidate whose optimistic vector is weakly dominated by a measured candidate's
   pessimistic vector (the lower end of its 95% interval) is pruned: it can never be strictly
   better. A candidate the full judge then refuses reopens what its quick runs pruned.
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

## The iron farm's grammar (version 1)

Parts, each generated from the ironfarm lane's generators (crates/mobfarm) and written into one
module frame (crates/farmopt-iron/src/parts.rs): the cage (1-cell pods, scared and blind
villagers, the zombie in its boat, the hide; the bed plate or a bed observer, U1 east or west),
the dry-hole pad (n 6/8/10, floor -7..-5, shifted), the kill (the pit corner at feet -13..-23 or
the fire-safe blade at -19..-25), the collection (the kill's hoppers to a chest), the calm part
(the smallest catalog timers legal for the stage, placed and routed by `mobfarm::parts`), the
tiling (N modules in a row at pitch 17-27, each with its own calm or one calm routed through every
module), the placement (the void, dug in at village-edge cell by cell 3-8 below ground, or the void
build cleared into the surface), and the L3 family (lava spawn pits with the golems' tripwire
calm). Exact seeds (`--seeds`: D1, D4, D1U, L3, M2-open, M3, M4, LB, LS-open) are calibration
points. `mctas farmopt parts` measures each part on its own; `mctas farmopt count` prints the
space's size.
