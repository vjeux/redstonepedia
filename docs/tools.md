# Tools

Every tool is a `mctas` subcommand, written in Rust (`./target/release/mctas ...` after
`cargo build --release`); `--help` on any command lists all of its flags. The rules the judge
enforces are in [rules.md](rules.md); how the simulator, the bridge and the videos work is in
[internals.md](internals.md). The clock study's commands (`mctas clock sim | verify | ...`), its
judge and its design format are in [clocks.md](clocks.md). The website of every build
(`mctas site build | publish | serve | smoke | deploy`) is in [site.md](site.md); Litematica
schematics of any design (`mctas litematic export | read | check | verify-ingame`) in [litematic.md](litematic.md).
The bedrock study's commands (`mctas bedrock scan | watch | vanilla | vanilla-matrix | design |
judge | minimize | graft | row | circuit | scenario`: bedrock breakers, the portal method, the
official server's checks) are in [designs/bedrock/NOTES.md](../designs/bedrock/NOTES.md#tools); its
circuit kind (`circuit: bedrock-breaker N`) is judged like every circuit (`mctas circuit verify`).
The mob farms' commands (`mctas mobfarm gen | edit | exit | run | probe | validate | gate | weather-stats | fire-burn | bolt-check`: iron farm
layouts, a design's chest and `how:` line, the golem exit flood, single redsim runs, a built farm's
blocks, the design run in the real server against redsim, the real-game gate that `circuit
verify` runs: `mctas mobfarm gate <design> [--days 3] [--builds 0.0 1.1] [--no-rain-day]
[--no-thunder-day] [--parallel 8] [--keep] [--trace] [--terrain <sample> [--world <kept world>]]`
(a terrain sample: the farm at its Site in the sample's kept world, `--parallel` runs), `mobfarm
run ... --ticks-only` (the time a game day takes), and the weather model's checks against
the official server: the cycle's statistics, lava's fire in a design, summoned bolts' outcomes) are in the ironfarm track notes
([NOTES.md](../designs/circuits/mobfarm/tracks/ironfarm/NOTES.md#commands)); the kind
(`circuit: mob-farm iron_ingot day-1|middle|fastest`) is judged on redsim and confirmed in the real game by `circuit verify` like every circuit ([circuits.md](circuits.md)).
The mob farms' tick-exactness (vjeux 2026-11-08: "why don't we have tick perfect precision?"):
`circuit verify` and `circuit archive` play run 0 of every build the card averages over on the
deterministic bridge and on redsim, every entity, block, container, memory and random state
compared every game tick for the judge's whole run, and print each build's result (exact, or its
first difference and the lane that owns it; `circuit::mobfarm_exact`). It is a ratchet: a design
whose recorded exactness ([exact.tsv](../designs/circuits/mobfarm/exact.tsv), written by `circuit
archive` and `mctas mobfarm exact <design>... --record`) says every build was exact fails when one
now differs; a design not yet exact keeps the verdict of the unmodified server's statistical gate
above. The site shows none of it. One run by hand: `mctas mobfarm compare
<design> [--build K --run I] [--days 5] [--terrain <sample>] [--weather] [--no-spawning] [--only
world,blocks,entities,<category>] [--full] [--probe] [--draws-at T] [--twin]` (`--build`: the
judge's build K and run I, its seed, walking and weather; `--no-spawning`: no mob spawning on
either side, the random ticks alone, terrain's regression check `sim:terrain-ticks-*`; `--only`:
only those kinds of difference count; `--twin`: the bridge against itself; the last line counts
the drops each side burnt: items that left while on fire, by item), and the level
random's draws of one tick by call site on both sides: `mctas mobai farm-trace <design> --seed S
--at T [--build K --run I --day0 D --no-spawning --weather]` (`MCTAS_NATURAL_LOG=T` prints redsim's
natural spawner calls of that tick; `MCTAS_EXACT_LIGHT=x0,y0,z0,x1,y1,z1` the cells whose sky or
block light differs at the end (redsim lights a terrain world with the game's incremental light
engines, `terrain::engine`; `MCTAS_LIGHT_ENGINE=0` uses the old fixed-point relight instead, to
compare the two); `MCTAS_EXACT_RANDOM_TICKS=N` the `random_tick_speed` gamerule N on both sides (a
stress of the terrain's random ticks: 300 runs a day's random ticks in 80 ticks; a portal's zombified
piglins 100 times as often); `MCTAS_EXACT_SUMMONS=1` each golem summon and each villager's
GOLEM_DETECTED_RECENTLY set and freed, both sides). One judge run on redsim alone: `mctas mobfarm
run <design> [--terrain <sample> | --build K] --seed S --days D` (`--build K`: the judge's K-th
terrain site with its run options, so a live view's "build 7 seed S" is `--build 7 --seed S`).
Nether portals and the Nether (redsim's `World::nether`, `entity::portal`: crossings both ways, the
arrival placement, the 300-gt cooldown, the PORTAL ticket that keeps the Nether ticking, items,
ultra-warm lava) against the official server, which has a Nether (the bridge has none): `mctas
mobai portal-probe --nether` (built-in cases, each beside redsim's), and for two design files, each
pasted at its own coordinates (choose coordinates that link), `mctas mobai portal-probe --nether
--overworld <design> --nether-design <design> [--ticks N --every K --stagger S] [--redsim]`: every
K ticks and at each event each of the overworld design's entities (the k-th summoned S x k ticks
in) with its dimension, position, motion, health and PortalCooldown, and the iron in the Nether;
`--redsim` prints the same run on redsim. Known difference: a crossing into Nether chunks no longer
loaded waits for the game's chunk load (1 to 7 ticks measured) before its first tick there;
redsim, like a farm running steadily, keeps them loaded.
The judge's run on many seeds at once: `mctas mobfarm seeds <design> --from 1 --to 100 --days 3
[--jobs N] [--write]` prints each seed's verdict (PASS or its first failure, with golems, iron and
a carousel's riders at each midday), how many hold and the first failures by kind; `--write`
records the count as a row of `<stem>.seeds.tsv` beside the design (`days held of seeds`, in run
order; a run again on the same days and seeds replaces its row), which the farm's card quotes.
The carousel's release + hold part (designs/briefs/farm-iron-carousel.md, "The parts" 2): `mctas
mobfarm hold [--levels 3] [--level-gap 3] [--run 21] [--stand-z0 1] [--boost-every 2] [--margin 1]
[--pitch 1] [--unpaired] [--no-zombies] [--yard] --out <file>` writes its bench design (7 lanes of
stacked single-cart tracks, one villager a track, a batch a lane; each track a cart between two
braked one-rail slopes; zombies beside every level; one shared bed); the lanes are paired (ironspec's
ruling: x 0,1 | 3,4 | 6,7 | 9, a wiring column at x -1, 2, 5, 8 beside each lane, every other
pair's hold ends one nearer, the hold ends' zombies one level with the rails and one a block higher)
unless `--unpaired` (x = b * pitch). `mctas mobfarm slice <design> --x X [--y-from A --y-to B]
[--z-from A --z-to B]` prints a design's plane x = X as text (what each cell holds, the carts and
their riders' eye cells), for laying out wiring in a stacked part. `mctas mobfarm hold-bench <file> --from 1 --to 100
--days 6 [--jitter 20] [--jobs N] [--verbose]` runs the 7 batches on the 100-gt slot grid with the
counter's lines ideal (each switch 1 + 0..=jitter gt after a slot) and checks every villager every
tick: the selected batch at its stand slopes at the slot, every other rider past the golems' follow
box (19.3 from the pad) and outside the summoner's 10-box, never two batches at the station, every
rider on its cart and in its lane, REST ticks and CANT_REACH at REST, and every cell a rider took
scanned for a golem spawn spot; it prints the clean seeds, the arrival margin, the blocks and box;
`--verbose` adds each slot and the first calm villagers at night with the zombies they see, and
`--trace ID` prints that villager's position and its cart's to stderr. With `hold --yard` the
design carries the sleep routine's part 3 (`mobfarm/src/yard.rs`): a hump of a second braked
slope at each hold end, each track's extension past it into its level's switched rail junctions
(on paired lanes J_12, J_34, J_56 in the wiring columns, then T_0, T_25, T_f; at pitch 1 an east
chain for lanes 0-3, a west chain for lanes 6-4, a Y; the three levels' chains stacked
and merged by two more junctions), and one shared bed past them, its cage, an activator, a free
stand-up cell, a park rail and a dorm zombie behind an iron trapdoor; the bench then runs the
nightly dispatch with the scheduler's timed controls (`mobfarm/src/naps.rs`: every 4th line fall
in the deep night a batch's rider goes to the bed in its own cart; the hump, the activator and the
route timed from that fall, the hatch and the park rail from the bed's observer) and checks a nap
per villager per night (second naps counted), HOME at each dawn and midday, no other villager in
REST within 16 of the occupied bed, each visit's length, when its return clears the junctions,
the routes the scheduler would have to hold, each napper's HOME re-take and the longest interval
between a villager's naps.
The carousel's counter (the brief's part 1): `mctas circuit memory counter-bench <design>
[--pulses 100000] [--seed S] [--restarts 20] [--miss 0.05] [--burst 0.3] [--width 2..15] [--jitter
5] [--ready 40] [--tc 40] [--m 6..10] [--turns 1,2,3] [--dump FILE] [--stimulus-out FILE]` runs a
design with one driven input (the golem pulse) and seven `port output.<name>` lines on redsim
against a reactive farm (after each advance the next batch is ready `--ready` gt later and summons
at the next slot tick, its pulse rising 0..jitter gt after it; a slot is missed with `--miss`, and
again with `--burst` after a miss) and checks every tick: never two lines high, one advance within
`--tc` of each golem pulse, the cycle's order (learned on the first lap) kept, a fallback 100 + m
after the previous advance with m in `--m`, no stall (a golem pulse within 95 gt after a fallback
may be absorbed: the brief's case (c)); then the same run with `--restarts` chunk reloads at random
ticks and on the design turned `--turns` quarter turns, each identical tick for tick. `--dump`
writes the trace for `mobfarm timeline --from-dump`; `--stimulus-out` a copy of the design whose
`stimulus:` line is the run's input, which `mctas circuit trace-compare` runs in the game and on
redsim, every cell compared every gt (designs/farms/iron/parts/carousel-counter/NOTES.md). It
prints the design's blocks, box and build minutes first. `mctas circuit memory counter-v8 --out F`
regenerates counter v8 (v7 for the paired-lane frame) from v7.
The carousel's nightly scheduler (part 3): `mctas circuit memory sched-model [--policy block|pring|
...] [--every 4] [--dusk D --pring-end E] [--days N] [--miss p]` is the policy model (lost slots,
second naps); `mctas circuit memory sched-gen --out F` generates the scheduler core from the module
designs in designs/farms/iron/parts/carousel-scheduler/; `mctas circuit memory sched-bench <design>
[--days 60] [--seed S] [--miss 0.01] [--burst 0.3] [--dusk 13670 --end 22330] [--game N] [--watch
FROM..TO --probes 'x,y,z ...']` runs it on redsim with the counter's lines and the night window
driven ideal (direct-driven input cells: a redstone block while high, stone while low), reads each
visit off the design (`v` rising names the `p<b>` and `l<k>` high), naps the villager after its
line falls, and checks every slot's willingness (gameTime - LAST_SLEPT < 24,000), the hump window
(up at the line's fall, >= 100 gt past it) and prints per villager its naps a night, nap day-time
range and worst gap window, plus the blocks, box and build minutes; `--game N` replays the run's
first N days' input actions in the real game, every output probe compared every tick
(designs/farms/iron/parts/carousel-scheduler/NOTES.md).
The single-lane loop's scheduler (vjeux's one queue of 21 carts): `mctas circuit memory loop-model
[--policy pring|every|k0|first-unvisited] [--rejoin exact|tail|place|batch] [--every 4] [--days N]
[--miss p] [--visit A..B]` models the queue (the front 3 released per release, a napper's way back)
and prints lost slots, napless villager-nights and second naps; `mctas circuit memory loop-gen --out
F` generates the scheduler (one 7-state ring of D = pointer - release index, the visit gate, the
delay chain, the window latch and the slot-out module); `mctas circuit memory loop-bench <design>
[--days 60] [--seed S] [--miss 0.01] [--game N]` drives its `release` (a 10-gt pulse per release)
and `win` cells, reads each visit's napper slot off `s0..s2`, checks every slot's willingness, the
ring's token against the policy at every release, the window `v` (from the release, >= 85 gt) and
the slot lines, and prints per villager its naps and gap windows; `--game N` as for sched-bench;
`--weather clear|rain|thunder|cycle` drives the window from a normal daylight detector reading 0
under that weather (redsim's sensor model; `cycle`: random spells, the levels ramping 0.01 a tick)
instead of the ideal [dusk, end]; `--restarts R` repeats the run with R chunk reloads at random
ticks and requires the same output changes tick for tick; `--placed-raw` places the design raw (its
written states and block entities, no shape or neighbour updates, no settle).
The single-lane loop's release controller: `mctas circuit memory release-gen --out F` generates it
(golem input; a 3-comparator countdown ring with a busy threshold and an empty-ring torch for the
fallback; GO = (golem or held request or fallback) minus busy, a self-timed ~10-gt release pulse;
the held request in a waxed copper bulb set by the input's rising edge, with a merge window from
the delay chain; the release group window and the advance pulses off the same chain);
`mctas circuit memory release-bench <design> [--pulses 1000] [--seed S] [--miss 0.05] [--early
0.05] [--double 0.1] [--restarts R] [--game N]` drives its `golem` cell (golems in time, early ones
to hold, missed slots, double detections to merge), checks every `group` window's start against an
exact model and its length, the three `adv` pulses after it (start + 38/58/78, 7..12 gt, identical)
and that nothing else lights; `--restarts`/`--game` as for loop-bench, `--turns 1,2,3` repeats it
turned, `--placed-raw` places the design raw (its written states and block entities, no shape or
neighbour updates, no settle: "works as placed"). `release-gen --out F --loop <ironfarm's loop file>` builds the controller into a copy of
the single-lane loop (work in progress): turned west of row B, its lines wired to the 21 controls
(the release group, the 18 queue slots on one tick, the tail's close-off windows, the stand's three
latches and their brake from the release run's detector rail), and writes `F.probe.txt` beside it
(the controller alone, the controls as ports); `mctas circuit memory release-trace <design> --at
<golem ticks> [--det <detector ticks>] [--ticks N]` drives the golem cell (and a stand-in for the
loop's detector rail) and prints each output port's on and off ticks from the first golem
(`output.lvl_*` ports: the analog level at each change); queue-loop-run's `--wired` runs carts on F.
A design's Nether part as a design of its own: `mctas mobfarm nether-part <design> --out F` writes
its `nether layer` blocks, `nether nbt:` data and `port nether-<name>` ports as the blocks, data and
ports of F, so `web shot F` pictures the Nether side (the kill, the collection); the viewer draws a
design's overworld blocks only.
The one-chunk column's skeleton (onechunk, farmopt's lane: vjeux's "Can we have the system in one
chunk?"): `mctas mobfarm column --part NAME=FILE[%BASE][@fx,fy,fz][^tx,ty,tz][#x0,y0,z0,x1,y1,z1]...
[--reserve NAME=x0,y0,z0,x1,y1,z1...] [--reserves-from BUDGET [--reserve-skip NAME]...] [--column
-6,-6,9,9] [--chunk-phase -6,-6] [--plan] [--port NAME=x,y,z] [--ground Y] [--out-shift dx,dy,dz]
[--no-riders] [--no-settle] [--out F]` moves each
part's committed design (`#`: only its cells in that box, the file's frame; `%BASE`: only what the
file adds to BASE, its cells BASE leaves empty and its entities BASE lacks, a way-in laid onto the
module, a cell both hold left to BASE's parts, so a changed one is a `--block` override) from its file's frame
into the column's (`@`) and from its committed place into its tier (`^`); a reserved box stands for
a part not laid out yet (`--reserves-from`: every `# reserved (no blocks):` line of a budget the
command wrote, but the `--reserve-skip` names, the parts laid). It prints each part's committed and placed box, the chunk lines each
crosses (`--chunk-phase`: lines at x, z = PX + 16k; the loop frame's -6), where it leaves the
column, the cells two parts hold, reserved boxes over a part or each other, cells declared two
ways, the tiers top down and the column's blocks³. `--out` writes the skeleton (the first part's
ports, a later part's shared-bed `home`, one Nether part, `positions:` and `declared:` joined),
its junctions settled as `mobfarm paste` settles them; `--ground` so `mobfarm bill --terrain` digs
it in, `--out-shift` to write it in the farm's aligned frame (loop + 6,0,6), `--no-riders` so
`mobfarm exit --positions` sets the summoners one pocket at a time
(designs/farms/iron/carousel-assembly/NOTES.md, "One chunk"). Stand-ins for a lane's cells until
they land: `--track NAME=x,y,z;x,y,z;...` (plain rails through the waypoints, a stone under each:
each leg along x or z, a rising leg climbing in its last cells and a falling one dropping in its
first, so a curve never slopes), `--cart x,y,z[,Y][,empty]`, `--post x,y,z` (the brief's zombie
post); `--clear` boxes take committed cells out first (a lane's re-lay), `--note` writes header
lines (the joints table), `--positions-out F` writes every villager position (riders' cells and
sketch rail cells) for the flood with a summoner at each (`mobfarm exit F --positions P
--part-flood` floods an assembled module that holds its bed as a part: every villager and every
position summons, the bed a block like any other). More of the generator: a track named
`+NAME` is boosted (every straight cell a powered rail on its own redstone block, the curves plain);
a part named `NAME!=FILE` re-lays cleared cells (the clears skip it); `--pockets
NAME=x,y,z,DIR,N[,rstar]` lays N of the stand's pockets in a row (P a flat detector, R the rest slope,
G a powered slope under a gate, a flat valley and a slope up: pitch 5, net -1; the last ends at G)
with their runner cells; `--block x,y,z=STATE` lays single blocks; `--wait-box BOX` checks every
rail cell inside against the pad's golem sensor boxes (`--golem-box`: |dx|, |dz| < 18.84, golem feet
-20.54..+19.79 of the rider's); `--summon-box x,y,z` lists every villager position inside a summoner's
box grown by 10; `--bed x,y,z [--bed-exempt PART]` the bed radius (> 16 + 1); `--path
NAME=x,y,z..x,y,z[..]` declares an off-rail rider path (a drop shaft, a bubble-column lift, a fall,
a hold with no cart placed; each consecutive pair a straight run of cells), added to
`--positions-out` and judged by `--bed` like a part; `--notes-file F`; every
run lists each slope without a sturdy block below or on its high side (it pops when placed).
The golem chance per summon from a file: `mctas mobfarm summon-chance <design> --pad BOX --stand
x,y,z...` runs the game's summon search (redsim's port of SpawnUtil's LEGACY_IRON_GOLEM scan, 289
columns, floors y+5..y-7, `checkSpawnObstruction` with the file's carts placed) from each villager
riding a cart over a --stand cell: n_i on the pad, off-pad passes (must be 0), wasted landings,
no-floor columns, P = 1 - prod (1 - n_i/289)^10 and golems a day (`--per-day`, 238).
A gravity column run from its own file: `mctas mobfarm column-run <design> [--cycles N] [--warm W]
[--trace]` builds the design as placed (its carts and riders, AI off) and runs it with T at every
world time multiple of 100. Its frame is design-agnostic; the design's header names the run cells:
`run-gates: NAME x,y,z[/sx,sy,sz]; ...` (a fence gate, and the block that powers the powered rail
under it: the runner sets it to a redstone block while it powers the rail, the file's block back
after), `run-cells: NAME x,y,z; ...` (detector rails, read for their real powered state, and rest
cells, read by the carts over them; PAD0/PAD1 the pad's golem feet box) and `run-controller: NAME`
(`mobfarm column --run-gate NAME=... --run-cell NAME=... --run-controller v4` writes them, shifted with
the file). The controller is per design (`columnrun::V4`, the inverted gravity column: the R* hold
released at T+3 with its Ds-keyed stagger, ironfarm's escapement, the stand's arrival fill and its
departure at T+5, 7b90c3f1's receiver, the tail's moves and the intake); a new design gets its own
`Controller` (its gate laws from its cells' names). Every gate and source it drives is EMULATED and
the report says so; the gates the file's own redstone moves are listed. It reports every new
cart-cart contact with both stored speeds (bar 0.2, the nearest run cell named), every gate shut
onto a cart (the closed bar's box), each cart's detector passes, and at each T every rider inside a
stand summoner's box grown by 10, every rider inside a pad spot's sensor box (the worst-case
constants) and the last departing batch's feet against the lock (pad feet + 20.54) by T - 5.
The F1 + Q2 station, benched: `mctas mobfarm drop-bench --layout station --pad ...` (the M1 drop:
three level pockets under trapdoor holds at y 16, the pad below; `--l1` the controller on game
ticks; `--restart T,ORDER` / `--restart-sweep FROM,TO,STEP` the 10 load orders of
`liveness::restart`; `--write-design PATH` the station in the loop frame for the module, its
placement-order list and the detector rule in its header). `--panic-post KIND:x,y,z:OPEN[:ROOF]`
lays a scarer post (a boat at the cell's centre in a glass box open on one side, OPEN `-` none: a
part file's box; `--part FILE` lays a loop-frame part file's blocks too, e.g. a scarer part's
occluders) and prints, per
summoner, the game's summon trigger at the next summon over the rider's own 400 sensor phase pairs
(NEAREST_HOSTILE: a scarer in the living scan's box, `--follow-range`, seen eye to eye within 16 at
every tick to the hostiles scan, in its type's range; VillagerPanicTrigger's 61-tick run and its
restart gap h + 61 + 62k): the pairs it calls in, the least caller count (the riders calling in all
400; the stand's bar >= 1, margin 2), its panic window in the pocket, and the conservative windows
(in range T-19..T, seen in the 16-box T-38..T); `--panic-search LANE` ranks a zombie post's cells
for that lane inside the module's chunk (its seat 6.05..7.95 from the rider at rest, no rider ever
nearer than 6.05, a glass box seen through its open face) by the shortest panic window in the
pocket they give, with the nearest pad golem. The golem flow on that pad: `mctas mobfarm golem-flow
<station> --post KIND:x,y,z:OPEN[:ROOF]... [--cycles 240] [--twin] [--piglins N] [--block-row Z]
[--restart-phases P,..] [--restart-noop] [--restart-once TICK,ORDER] [--callers 2]` (redsim with the
mob AI on, a pillager post seating redsim's pillager (no crossbow AI: its shots are tick-exact's
official bridge): a summon at every summon tick, each golem followed to its portal crossing (the
tick of its own life it crossed in, bar 2), its targets and the posts' (printed counts), damage
taken or dealt, the posts alive at each dawn, golems in the overworld at a summon tick and the
summoners' sightings, a reload's priced costs apart; `--block-row` the positive control,
`--restart-noop` the restarts' control). `--r-repeater` (drop-bench and station-judge) drives the R
line through R_1's built driver (a repeater facing west into R_1, fed at (x+2,17,1), the
controller's line end, by a lever stand-in on glass) and prints each lane's rail power and its
cart's first move (a launch after the power ended, or none, fails). `--box-short N` (drop-bench and golem-flow) builds the world N cells short of
the design's top: the placement's positive control (since onechunk's 0f375d1c0 the run refuses the
out-of-box cells, the control's "fires"); golem-flow's `--block x,y,z,STATE` lays extra cells.
Golem-stamp path (a), the station's departures through the merge row and the shared lift:
`drop-bench --layout station --module <assembled module, loop frame> [--path-a] [--part <the record
lift file>] [--declare FILE]... [--record <the design of record>] [--pocket-lever x,y,z x3]
[--hold-drive x,y,z --hold-rail x,y,z [--hold-release R] [--hold-width W]] [--napper-release K
[--napper-held x,y,z]] [--watch-cell x,y,z]... [--summon-check [--remove-rider K]] [--out FILE]` runs
the module as the world (its six station carts kept, every entity with `--record`; with `--path-a`
this station's cells and `drop::PATH_A_LIFT`, a stand-in for 7b90c3f1's lift cells, laid over it,
then `--part` and `--block` cells; `--out` writes that composed world, its header declaring the
stand-ins, the held rails, the declared stops, and with `--record` the world box, the stand-ins'
bench verdicts and the bench's own entity), lets the departing carts roll on and tests each rider
every tick against the live callers' boxes (the arriving riders' boxes inflated by 10; the bar, a
verdict: out of every caller box after its move on S - 3, S on the summon's own clock, game time
% 100 == 0, with the stamps read from the riders' memory after S), with each lane's pass at y 12,
its lift entry and top exit (the column bar 16 on every pair), the braked slope's hold (the slope's
rail held powered from REL + R + 1 for W gt, a declared `held:` stand-in with the build's edge
updates, split into the driver's front updates (a plain rail with three or more potential
connections among those cells refuses the run) and the rail's own, and the hold check against the
declared window, `--hold-poke K` its control; no stand-in at its line end), the napper's cart
released at REL + K (with `--napper-held` its rail a held stand-in too), the departing carts' push
contacts, the riders' least air, and the watched cells (a print); every cell in the bench's frame
(loop + 6). With `--record` the composed world's cells and entities are compared with the design of
record (record-match; the rows count only by its verdict) and `--pocket-lever` puts the pockets'
lever stand-ins at the record's own line ends. `--summon-check` keeps the module's hostile posts
and prints the direct summon check at S: the stand's riders willing and ticking at the tick's start
(seated, wanting a golem, >= 3 willing in their 10-box, the panic trigger ticking: not at its run's
end + 1), the GolemTry calls up to the first success (only the stand's callers' golems count), the
class (golem born / spot miss / caller blocked by its own memory / fewer than 3 willing / no
panicking caller), and the golem rate per callers-per-S k against 1 - m^k, m = (1 - 40..44/289)^10
(failing when the binomial 99 % interval misses it; NOT JUDGED with no judged bucket). Every row
also lists the carts' block contacts (a move stopped horizontally, charged to the blocks on the
side of the motion) and fails on one that is not declared (the station's stops and the headers'
contacts, parsed strictly; PROVISIONAL entries printed as such). The run exits 1 when any row
fails the one failure list (`stationjudge::fails`, NOT JUDGED included, the scope computed) or a
REL-level verdict fails; a control run (`--hold-poke`, `--sight-control`, `--remove-rider`,
`--lever-off`, `--rail-t-control`, `--rail-t-negative`, `--contact-control x,y,z`,
`--missing-control K`, `--rate-control`, `--push-control`, `--foreign-caller-control`,
`--reseat-control`, `--fluid-control`, `--t-control`, `--activity-control`) passes only when its
control bites in every run through the failure list (a negative control: its failure absent while
its world change happened) and every other failure of the run is the control's declared
collateral (an undeclared one fails the exit); `--watch-control` is an instrument print. Every plain
rail with >= 3 potential connections at T (a T) has its shape checked at every tick's end; a runtime
re-shape fails the row. The villagers sleep as in the design's world whatever `--summon-check`
says, so a run without the check that summons reads NOT JUDGED and fails.
station-judge takes `--record` (its sight
reference) and `--sight-control` (its verdict: the sight failure in every lead run, every other
failure of those runs its declared collateral), and exits 1 when
its lead's rows fail. With `--record` on the
station layout, the design of record with this binary's station cells laid over it (ironspec 21:30:
the station synced before a static cites it; the cells that differ from the record print) is the
sight check's reference (no record-match there). The
station riders' activities are read every tick and each REST entry is printed with its stamp (a
count, not judged, until b8949c96's hook lands); `--day-time D` sets the runs' day time (a night
value samples the REST entries). Every
laid design's fluid cells get their ticks scheduled before the
settle (the count printed), and every stand-in and `--block` override prints before -> after,
failing when it changes nothing (ironspec 04:07).
Every drive the bench emulates is a lever stand-in at its line end (the pockets' inputs through
day-1's repeater stand-ins, the R line's input with `--r-repeater`, the napper's input; the slope's
hold has none: the bench holds the slope's rail powered): the bench refuses a stand-in
whose properties differ from the built cell's over the full guard list (legacy-solid, a full top
face, full-block collision, solid-render, any collision, air, a signal source, golem-excluded, a
rail, dangerous, a fluid, the golem spawn-space role; each lever against its line end, redstone
dust), checks each lever's update reach (its six neighbours and its support's: no rail with three
or more potential connections), power reach (its neighbours, and its support's if it conducts:
nothing beyond its input, redstone torches counted) and survival, and fails a row in which a sight
segment (villager to scarer, villager to villager, eye to eye) is clipped by the live world where
the reference (the module of record's cells, every stand-in back to its built cell) lets it
through, every tick from T to S read at its end; `--sight-control` is its positive control, a glass
block laid in the world at T on a live villager -> scarer segment (a world with no scarer, as
station-judge's: on a villager -> villager segment): ironspec's
17:46/18:34/20:10/20:31/23:30/00:27 guard, G1's audit (1). A departing rider missing without
`--dep-riders` fails too. The bench turns redsim's natural spawner off explicitly and prints the
controller gates it depends on (day-1's canonical list): F1 (G8), the keyed hold pulse (G15), the R
line and the slope tap are stand-ins at their declared phases, the rest out of its scope (ironspec
14:57 / 15:37). The drive-integrity bar: each pocket drive's first powered tick within
`--drive-slack j` of its clock slot REL + offset k + `drop::DRIVE_SLOT_DELTA` (+1, measured), else
"a pocket drive off its clock slot"; `--late-drive-control K,D` (pocket K's drive D gt late) must
fail by it. `--t-hold-control` is the T-shape bar's negative row: the first station T's constant
source (form (C): slime under the T, a lit torch under the slime, on glass) laid before the settle,
the same lever switched on and off beside it: the T must not re-shape. `--hold-repeater` (commit E)
drives the holds' iron trapdoors through their built drive (the ring's north cell a slime block fed
by a delay-1 repeater (x,16,-2) from a lever stand-in at the line end (x,16,-3)), the lever flipped
`HOLD_DRIVE_LEAD_ITERATIONS` (1 bench iteration = 2 gt: the lever flips between ticks; tick-exact's
hold-drive game row) before each declared open and shut, each trapdoor's open and shut checked against
its declared tick ("a hold's trapdoor off its declared tick (the holds' built drive)"), redsim's
chain printed tick by tick; `--hold-lead-control D` (D = -1 or 1) moves the lever D iterations and must
fail by that bar; station-judge uses it whenever the station is built from its design. Where a world has hold
strings (x,17,0) (the module rows' record7f12; the station file has none since day-1 21:21), their
last release before the R line rises and first press after it print per lane, a diagnostic (the
lead bar retired by name, ironspec 21:16: its consumer was G8c, not bought). ironfarm's C-fix rows:
`--plant-cart x,y,z@k` (a ridden cart planted at T + k, its rider and position read back;
`empty:x,y,z@k` an empty one), `--plant-shift FROM,TO,STEP` (one run per shift of every plant's k:
the refill phases), the slot-1 strings over R_1 (x,18,1) printed per tick on each change (every
cart over the cell, its end-of-tick box against the band and redsim's own press, which models the
raised press) and the planted empty carts' ends (a diagnostic: ironspec 22:28's slot-1 rest bar
retired by name, ironspec 23:34, (C) bought), `--plant-block x,y,z=STATE@k` (read back; a no-op refuses),
`--watch-west x,y,z` (a cart leaving
that cell westward fails; each cell's carts and exit faces are counted, and a cell no cart entered
is NOT JUDGED, a failure), `--offrail-y Y` (a cart at or above Y at rest by ironspec 10:42's stall
rule, frozen ticks not counted, with no rail in its cell nor, as the game's minecart checks, in the
cell below, fails; no cart at rest at or above Y is NOT JUDGED, a failure).
`mctas mobfarm build-order DESIGN --t x,y,z --cell x,y,z=STATE... [--want SHAPE] [--last x,y,z]...
[--sequence NAME|WANT|x,y,z=STATE;...]...`: a rail junction's cells placed into the design around
them in every order the game allows (a block never before the block it is mounted on; `--last`
cells, the build rule's, placed last), each rail in both placement facings (its default north_south
or east_west), each by the player's placement path (redsim's `World::set_block` in the game's order:
onPlace, then the neighbour updates with the old block's kind), plus two pastes and named sequences
(a wrong order, a repair, a parity positive); every build or sequence off its wanted shape fails,
and so does a test that built no order.
The portal-loop loaders, benched in redsim: `mctas mobfarm l1-bench [--ticks N] [--hold 320]
[--pulse 20] [--stagger A1,B1,A2] [--freeze T,D[,ow|nether|both]]... [--piglin T]...
[--design-out F]` (L1′, the cart loop: an overworld portal and its Nether twin, each side an exit
track into a 2-pocket queue, 3 empty carts, each released `hold` gt after it arrived; EMULATED and
listed in the report and the design header: pocket 1's release pulse, which also powers the release
line's two boosters, pocket 2's 2-gt nudge once pocket 1 and the line are clear, each release timed
from that cart's own arrival) and `mctas mobfarm l1-item-bench [--width 5] [--height 5]
[--fire-y 14] [--far-rows 2] [--floor hoppers|ice] [--items 4] [--ticks 24000] [--seeds N|A..B]
[--stop T]... [--stop-every N] [--restart-order save|reverse|seed:K|all] [--login-each N]
[--login-before-arrival] [--freeze T,D[,WORLD]]...
[--piglin T,X[,a|b|both[,adult|baby|jockey[,sword]]]]... [--mob-jump N] [--design-out F]` (L1‴, the
item loop: same-axis portals; on each side a firing dropper in the top row (y 14 of a 5-row
portal: 4.34 over its floor) fed by up-facing droppers, the exit face closed below and at the
pocket floor (its top 3.0 over the portal's floor: hoppers, no spawnable block), a 3-wide walled
pocket and a hopper row past the frame, the far face closed in its bottom `--far-rows` rows, a
backstop, a glass enclosure, and a REAL clock: an observer pair behind the firing column) write
their design and run it: each side's arrivals against the 300-gt PORTAL ticket (the worst gap), the
Nether's ticket lapsing, carts or items held inside a portal (the cooldown restarting every tick
inside), the carts or items counted (an item a mob holds is lost); L1‴ over many seeds (threads)
with the failures' Wilson interval, the landing spread against the floor's and backstop's edges,
each arrival's drift from its column's centre (over the flight and at the face crossing, with the
flight time), the flight's clearance to the frame and to a bottom-row mob's reach (standing 11.95,
a jumping baby's 12.227), mobs out of the portal's cells, and a positive control for the mobs'
pickup (a stone at each placed mob's feet; an iron sword for one placed armed with a golden sword,
as portal piglins spawn; `--mob-jump` forces jumps). Crossings, the cooldown restart, the
closest-portal search and the Nether ticket are redsim's. `--freeze` is redsim's level-32 freeze
(`reload::unload_some`: entities skip, blocks run) per world; `--stop T` / `--stop-every N` restart
both worlds between two ticks (`liveness::restart`: saved, unloaded and loaded again in
`--restart-order` save, reverse or seed:K, or `all`: one pass per order of `Restart::rows()`);
`--login-before-arrival` freezes both worlds 3 gt from every dispense. The overworld side is glass,
its far face closed in its bottom `--far-rows` rows (default 2); the Nether side's far face is closed
in every row and every glass block of its outer layer is cobblestone (ghasts), O2's back and bottom
shielded, and the casing closed in the 26-neighbourhood (cobblestone in every outside cell with an
edge or a corner on a weak cell inside: a ray samples every 0.3 blocks); the design and the run
print each side's interior spawn floors (a full top under air inside the enclosure: 0 required) and
the loop blocks (observers, portal blocks) a ray from the outside reaches by a 26-connected flood
through blocks under 4.034 (0 required on the Nether side), the mobs' highest box top (adults,
babies on foot and the jockey stack apart: the jumps' own control), and the design writer re-parses
its text and compares every cell.
The module's static checks (onechunk, ironspec's and G1's rules, Nov 12): `mctas mobfarm ghast-check
DESIGN [--nether] [--close OUT]` lists every block that matters (the storage rule: every block
resisting under 3.283 but the air, the fluids and a player's buttons, plates and levers; signs, a
lava stopper, count) that a ray from the outside air reaches by a 26-connected path through cells
under 4.034 (0 required); `--close` puts cobblestone in every outside cell with an edge or a corner
on the weak interior around those blocks until nothing is reached, and writes the closed design
(only for a layout whose working spaces are enclosed). `mctas mobfarm creeper-check DESIGN [--glass]
[--unlit-control]` floods the creeper's body (`hostile::flood_with` in a land world, so the portals'
11 and every light source propagate: spawned at block light 0, never in a rail's cell
(`#prevent_mob_spawning_inside`), or walked to from there; the world padded by 16), prints the block
light of every spawn space beside a target, the spawn origins, and the positions within 5.5 of a
portal block (`--glass`: or a glass block; 0 required) and within 10.5 (charged creepers,
information); `--unlit-control` turns every portal block to glass (unlit, the targets kept), the
positive control. `mctas mobfarm exit DESIGN
--positions P --part-flood [--spots]` floods an assembled module that holds its bed as a part and
lists every golem spawn spot by the block under it. `mobfarm column --out` re-parses what it wrote
(every cell, block entity, entity, port, `positions:`, `declared:` and run cell), merges the parts'
`run-cells:` / `run-gates:` (a later part's names as `<part>.<NAME>`), and with `--no-settle` writes
the placement-order list (every rail whose designed shape, or a neighbour's, vanilla placement
changes when it is placed last) and the rail Ts (every plain rail with 3 potential connections)
into the header, each T with its reach scan: every signal source whose state change updates the T's
cell (a detector rail its own and the block below's neighbours; a wire or a torch its neighbours'
neighbours; a repeater, comparator or observer its output block and that block's neighbours; a
lever, button or hook its own and its support's (below a floor lever or button, above a ceiling
one, else behind its facing); a redstone block never), or the nearest changing
source when none does.
`mctas mobfarm f1q2-run DESIGN [--lane-carts] [--drive] [--ticks N] [--day0 D] [--restart-every N
--restart-order O] [--control-rider K]` (onechunk, ironspec's deciding test for every rider's bed)
runs the assembled F1 + Q2 module with the exact villager brain, every villager's HOME the shared bed
(its `home` port; the bed's POI taken, as the station bench does), and prints every villager's stamp
age at each calm REST tick with no walk target (the largest, where, the ticks over 1,000), every
clear (a nap, its target reached, a complete path; the bed, a flee or a stroll point), HOME tracked
every tick and at dawn, and with `--drive` each lane's pocket turns (draws, panic ticks, with and
without a clear). `--lane-carts` seats a rider on each lane's R_1 and 5 slopes (8 a lane with the
pocket and the hold); `--drive` runs the station's L1 events, the lanes' R line and the split's
latches EMULATED from day-1's table v6 at s 12 (a golem's appearance is the golem pulse, REL =
golem + 2 + 4, the pockets at REL + 0 / 12 / 24); `--control-rider K` undoes the K-th villager's
clears every tick (it must release HOME). It also prints each rider's stamp at dusk (its first REST
tick), the clears by place (the pocket, the lane, the siding, elsewhere) and cause, a lap at a time,
the daytime ones apart, the longest stretch between two clears (all, and those ending in REST), and
per pocket turn the panic's onset (the WALK_TARGET holder, MoveToTargetSink's remainingCooldown),
the tick WALK_TARGET frees, the draws landed with and without a complete path and the panic ticks
spent on a landed draw with no path.
Each draw (a new walk target cell) is classified at the first path MoveToTargetSink computes for its
own target: complete, PARTIAL or never pathed (the target changed or cleared first), by place and
panic/calm; the partial panic draws are listed by (place, target cell, rider cell) and one row each
(the tick, the rider and its lane, its exact position, its cart at rest or rolling and its velocity,
the scarer from NEAREST_HOSTILE, the partial path's end, how long WALK_TARGET was held and whether
the hold outlasted the rider's stay at its cell, and every cart of its lane at the draw tick, for an
official replay). `--per-lane N` (default 7: ironspec's steady start, the top slope empty) sets the
riders a lane. The drive follows 7b90c3f1's K re-lay: each junction's lever (x,26,9) toggled
(redsim settles the junction), the branch detector slopes D (x,24,8) as the latches' inputs, J(-1)
re-armed at lane 7's D. A restart books the stamp the reload drops (no codec for
CANT_REACH_WALK_TARGET_SINCE) as "a reload", not a clear, and renumbers the controller's golems.
`--official-lane3` rebuilds tick-exact's official lane x 3 scene instead of the night (the night held
at 18,000, 7 riders entering lane x 3 at (3.6, 24.0, 6.49) one per 30 gt, every 100 gt the cart on
H out and a fresh rider in at the top, R_1 powered 12 gt without updates), with the landed panic
draws per rider block; `--compare-official LOG` prints tick-exact's per-draw log's tally per block
beside it (null / complete / PARTIAL / never).
`mctas mobfarm column ... --verify FILE [--keep-clear NAME=x0,y0,z0,x1,y1,z1]...` is the assembly
check: the committed FILE re-composed from the same parts and overrides must match it cell for cell
and entity for entity (a cell or an entity no part declares, a leftover of a superseded part, is
printed), with the census of its entities (each passenger counted); each keep-clear box must hold no
block (the bed stop's column over the bed; a NAME ending in "+glass" allows glass: the daylight
detector's roof skylight); and the 6.05 flee bar's sight classes (ironspec Nov 12 10:24): every
rider position every 0.1 along every rail and both bubble-column lifts, the eye at the cart + 1.62,
against every boated zombie (feet at the boat's y + 0.1875 - 0.7, eye + 1.74): within 6.05 (feet to
feet) IN SIGHT (a failure) unless a full-collider block sits on the eye-to-eye ray (HIDDEN, listed
with the blocking cell), beyond it clear; the cells of the file's `waker-places: x,y,z+...=REASON[;
...]` line (every group; the stop's waker, ironspec 11:01's ruling (a)) in sight are their own
declared class, not failures, and the rails of its `riderless:` line (same syntax: the stop's empty
cart from its park to its pickup) are no rider positions; the stop's on-foot positions join them (every cell of its box a villager can stand in:
no full cube in it or above it, a full cube under it, or a bed's top, no entity seated in it, and
none of its `unreachable:` line's cells (the stop owner's evidence that no walk gets there); feet on
the floor, eye + 1.62), and every zombie but the stop's own stays at least 6.05 from every stop cell, sight or not;
its `keep-clear:` cells must hold no block and its `keep-block: x,y,z=BLOCK[props]` cells that
block (the assembly check);
the pre-wake state (the open hatch's plates, hiding the zombie until F shuts them) rests on tick-exact's
official scene, not this test; the feet stand on the top of the cell's own collision shape (a bed
0.5625, a chest 0.875, a slab 0.5, a closed trapdoor 0.1875) or, in an empty cell, on the top of the
collider below when it reaches the cell (a full cube at y, a fence or a wall at y + 0.5), with 1.95 of
head room free above them; a cell holding water, lava, a bubble column or a waterlogged block is left
out (counted), the cells whose own shape reaches its top without a full cube (stood on from the cell
above) are listed, and a chest landing's rows are printed; `--sight-path NAME=x,z,y0,y1` adds a vertical rider path (a fall shaft; napper A's lift
follows the stop's y). It also prints the golem detection across the tiling (ironspec 12:14, G1's
per-axis rule: detected iff every box gap < 16), the pad's spawn cells moved to each neighbour (east,
west, and the southern copies mirrored in z), per rider class (the lanes, the split row and sidings,
the station and merge row, the stop and the napper's way, the lifts, the extra paths): the worst
pair's per-axis gaps and the result. Exit 1 on any failure.
`mctas mobfarm hold-check DESIGN --hold NAME=x,y,z:PULSE[@a1,a2,...]... [--max 40] [--n-pulse
6,7,8,...]` (ironspec's criterion, Nov 12 10:05): a cart with a villager settled at each rest
position along the hold's rail (none given: the cell's centre), the rail powered; the threshold is
the largest departure tick (the first tick off the hold's cell) over the rests + 1, against the
controller's pulse; each rest is also run with the rail powered exactly the hold's pulse (G1: the
threshold assumes a launch is monotone in the pulse length), a rest that never leaves with it a
FAIL. `--n-pulse` (ironfarm's convention-free test): the rail powered exactly N gt then
unpowered, each rest's speed at its first tick off the cell, and the least N at which every rest
leaves within 1% of its N = 30 speed.
f1q2-run's `--napper` (phase 3a; day-1's events, 58f770cc's stop, EMULATED) seeds one napper a night
at the stop's entry (3,-16,-5), west at 0.2, HOME the bed, its lane rotating -1 / 3 / 7 from the
seed's (seeds 1 / 2 / 3 start at 3 / 7 / -1), and drives the stop (F 10 gt after the lie-down: the
hatch's controls to stone; the release 30 gt after F: the park's supports to redstone blocks; R's
detector re-arms it), and the hold's release (c') as built (ironspec 11:49): the lift exit (7,-3,4)
as a detector rail sets the waiting RS, its Q delayed 38 gt; E samples live Q AND delayed Q at its
edge (ARM3 + 2, ARM3 the main line's detector (5,0,7) risen at or after REL + 24); locked, the 12-gt
pulse fires (the input (7,-1,8) at ARM3 + 10) and resets the RS (the departing napper's second
ARM3 rise fires nothing), and keys K(n) on at ARM3 + 93 / 108 / 124 and off at ARM3 + 125 / 140 / 156
(PRESENT_l at the reset) and J(-1)'s block ARM3 + 72..127 for an eastbound napper (7b90c3f1's
numbers), and the siding's release (D's rise + 4 while PRESENT_l: the input (x-2,23,9) on 30 gt; where
day-1's (a) feed stack is laid, its input (x-4,22,9)); the pockets' releases go under the pocket rail
(x,-1,0), or, where M16bw's side drive is laid (a repeater (x-1,0,0) fed by the riser dust
(x-2,0,0) and the step dust (x-3,-1,0)), as a floor lever in the riser's cell on its glass for the
pulse, the dust restored after (ironspec 18:10: no solid stand-in); the way-in queue's release is a
floor lever at (8,y+13,-4) on a glass support the module declares `emulated:`;
each napper's delayed bit at the sampler is logged against its rest on the hold, its departure
against E's rise, and every pulse fired with the hold empty or the napper still rolling (bar 0); every other rise of (5,0,7) in
a cycle is logged with the J/K levers, the hold's input and the
cycle's state. The napper is one of its lane's carts (its first lane starts one slope cart short).
It prints each napper's milestones and the drives; `--cart-trace ID,FROM,TO` prints a cart's cells
and every lane's H..V occupancy at each release with the ticks V (x,22,6) held a cart. With the
station's built R_1 drive (a repeater at
(x+1,17,1)) the R line is a redstone block at the repeater's input (x+2,17,1) for 10 gt. Every night
also prints the villagers within 16 of the bed head with the calm REST ticks there while the bed is
occupied (bar 0), any cart resting on another cart's top (a failure), the staleness bar (ironspec
11:02: every entry into a position within 6.05 of a zombie hidden from it at least 40 gt after the
rider's last position within 8 of a zombie in its sight, `hasLineOfSight` eye to eye; only the
stop's own waker left out, every other zombie counted everywhere), and the panic draws by the target's kind (a point or a block: the flee, the
strolls, HOME; an entity walk: one draw however far its mob moves), each entity walk's row naming
the followed mob's INTERACTION_TARGET, WALK_TARGET, activity and running behaviours (both pointing
at each other: the neighbour's `setWalkAndLookTargetMemories`) and the rider's own.
`--seat NAME=x,y,z:cx,cy,cz+...` counts an S-bend seat's stamp resets (a complete draw to one of its
clearing candidates from the seat: per candidate, per 1,000 rider-ticks, the longest stretch on it
without one); `--nap-trace FROM,TO` prints the napper's cart per tick (position, velocity, its
lane's R powered or not, every cart within 1.5); each napper's route from its K is checked against
the designed K (x-2,25,9), F (x-2,25,8), R (x-2,24,7), c1 (x-2,24,6), c2 (x-1,24,6), c3 (x-1,24,7),
C (x,24,7) (bar: identical); and the departing napper's exit from the bed's 16-ball after reaching
R (bar <= 152 gt). `--neighbour-golems dx,dy,dz` (ironspec 11:29) emulates a neighbour copy's pad
golems (ours moved by the offset, the same phase) setting GOLEM_DETECTED_RECENTLY on riders whose
16-block box they meet; every night prints each stand arrival (a cart entering a pocket cell
(x,0,0)) with the memory still set (blocked from a summon; bar 0) or expiring within 20 gt of it,
and with the offset the least time from a rider's last tick in the neighbour pad's detection box to
its next stand arrival (bar >= 619). The summons from the pockets (ironspec 12:13: the stand is the
pocket): before each tick a VillagerPanicTrigger calls the summon (game time % 100 = 0), every
scared, slept villager in a pocket cell (x,0,0) calls; a caller with GOLEM_DETECTED_RECENTLY present
(a ttl of 1 or more: the brain forgets a 0 before its behaviours run) is blocked, listed with the
memory's last setter; the villagers in a free caller's box inflated by 10 are counted or rejected,
and a summon those rejections take under 3 is blocked (participations blocked: bar 0); with each
lane's least rotation at a summon tick (the riders whose last lane is it, a napper out while away;
bar >= 7) and the least free caller's margin. Every setting of the memory is attributed: a summon's
10-box stamp (a golem born that tick; on a summoner that summon counted, or on a bystander, whose
expiry is printed against its next summon), the GolemSensor reading the living list (its source
golem and birth tick, or none alive: a departed golem in a stale list; how many within 600 gt after
the rider's own stamp), or the emulated neighbour. The stop's drives, the napper's seeding and its
milestones follow the stop's y (the `home` port's y against rev 3's -18). A stop with levers at its
control cells is 58f770cc's A6 (b36302af8): the hatch's levers (-4,y,2) and (-4,y+1,4) off at F (the
lie-down + 10), the park's (1,y+1,-6) and (0,y,-6) on 30 gt after the bed's wake edge, R's detector
(4,y-1,4) re-arming both (y the bed head's); otherwise d907's stop (its release 30 gt after F).
`--drive-out F --scene-out S` (mob-AI) records the run as a mobai drive v1 for `mctas mobai
fixture` (the 26.3 bridge and redsim, tick for tick): the world started as every bench's drive
(`Run::start` with the drive's options, the settle, the scene's entity lines spawned in file order),
the bed's POI and every rider's HOME and LAST_SLEPT as acts at 0, every emulated block write an
`act t set` (a write without every update fails the run); with `--napper` the module needs
b8949c96's way-in (the napper starts queued on its queue rail (9,y+13,-4), the drive spawning nothing
later); S is the module with the seeded carts appended as entity lines. Not with `--official-lane3`,
`--neighbour-golems`, `--control-rider` or restarts (their writes are no drive acts). On a module
with the way-in, the queue's gate is driven (b8949c96, day-1's to build): the release a redstone
block at (8,y+13,-4) when RS_ready (set at power-up and by A6's R), the hold (`--queue-hold N` gt
from the arrival detector (9,y+14,-5); default 0, ironspec 17:16: the release as soon as the stop is
free), REST (day time >= 12,000) and NOT DAY (the daylight detector (-5,15,3) at 0)
all hold, taken away at the departure detector (9,y+12,-3), which resets RS_ready; printed: each
visit's dwell (arrival to release) against its 34-gt cap (lane C binding, day-1's model), and each napper's PRESENT against its
predecessor's D pass (the predecessor passes its siding's D first). The golems born off the pad
(feet above y -4) are printed with the nearest villager at the birth (bar 0), and a model error
names them too. day-1's picker
(EMULATED, 14:46 / 16:14; the lanes per 7b90c3f1): the counter C steps on each release (C := 0 at
the daylight detector's morning RISE 0 -> 1, C := 19 at its EIGHT 7 -> 8 when C >= 136 or C <= 15);
a pick when C mod 10 is 1 / 4 / 7 (lanes A / B / C = x -1 / 3 / 7, each every 10 releases), gated by HELD (the queue occupied, from the
arrival to the departure) and the lane's away latch (set at its pick, reset when its napper is back
in its lane); the picked lane's batch cart is the one its launch sends; DAY (the daylight detector
above 0) at its row arrival turns it away; otherwise its own J and every J east of it are held off
until it passes (8,25,9) onto the leg, then the split as if it had taken its lane (lane -1's D rise:
J(3) on; lane 7's: J(-1) re-armed; J(7) back on). Each napper prints its stamp's age at its queue
arrival, at R and at its first clear after R, and its first call back in its lane against its memory's
last setting (margin >= 20); the worst queue-arrival age and the picks gated or turned away; the
villagers napped each night (bar 24 a full night), each night's picks with the villagers picked twice
and those never picked (PASS / FAIL on complete nights), and every villager's picks and naps. The chain
(day-1's accepted chain, ironspec 19:47): B is held 100 gt from each start; a golem pulse with B
expired starts a cycle at once (REL = the pulse + 6); a pulse during B re-arms B to the pulse + 100
and latches P, which starts the cycle at B's expiry; the flywheel `--watchdog W` (default 100; 0:
none) starts one when none started within W and B has expired, ties toward the start; a start
restarts the chain from any state of a cycle still in flight (liveness). Printed: the cycles, the
flywheel's starts, the re-arms, the least REL interval (bar >= 100) and the longest (liveness),
the restarts of a cycle in flight, the RELs' phases (game time mod 100) before and after the first
golem, the cycles off the locked phase by day. (a1) calm REST within 16 of the occupied bed (bar
0); (a2) two nappers (carts, or one asleep) in the stop's box at once (bar 0; the first tick and
riders), with the queue releases made before the previous napper's 16-ball exit as information;
(b) a pick while another napper is between its pick and the queue's departure (bar 0, the first
named). `--stale-start` (the recovery
row) starts every LAST_SLEPT 24,100 gt old: the first call with 3 willing villagers at the stand and
the first golem after it are printed. The summons by day: the calls, those with 3 willing in the box,
the golems, and the willing villagers counted per call. `--held-at-divert` (divert sampling (i)):
the picker reads HELD at the picked cart's row arrival instead of at the pick. The siding's release
drives day-1's (a) feed stack where it is laid (a repeater west of the siding's slime: the release
at its input (x-4,22,9)). The largest calm-REST stamp by lane (ironspec 16:55): each lane's
villagers (the lane each last rode), their largest stamp age at a calm REST tick against 1,000.
Every bar line that prints FAIL makes f1q2-run exit 1 (an "expected FAIL" token, a declared
placement-order T, is stripped first; the failing lines named last); `--report-only`
exits 0 whatever they say. Every block a drive writes keeps the built cell's golem and path
properties (G1's guard, ironspec 17:46; G1 17:50 / 17:53): a golem floor (legacy-solid and not on
LEGACY_IRON_GOLEM's list), a full top face, a full-block collision, solid-render, air, a signal
source, a rail, something dangerous, any collider, a fluid. A write that adds one the built cell
lacks fails the run, unless it is a state of the built block itself (a trapdoor, a lever, a rail) or
the module declares the cell `emulated: CELLS=STATE (why)`; one that only suppresses (a lever: not air, a
signal source) is named in the report. A controller line's end is driven by a powered lever hung on
the first sturdy face around it (below, above, the sides) whose updates (its own neighbours and its
support's, and out to taxicab 2 when it writes over dust) reach no rail T of the build (redsim's junction test) unless the module declares the T
`update-only-ts:` (state-stable for the state it holds; ironspec 18:31), whose support doesn't
conduct and with no powerable component beside it but its target (power reach, ironspec 18:34 (5):
a repeater or comparator counts only from behind, an observer facing the cell counts, copper bulbs
and crafters too),
checked to survive at its placement and its removal; a declared `emulated:` cell that is air is
laid at the start in the state its line names (a floor lever's support where the built line end isn't laid yet).
redsim's `Run::start` (every run built from a design without a site world) refuses a design with a
non-air cell outside the world's box, a cell the placement popped, or a cell where `can_survive`
fails (dust or a repeater over air, a hook without its anchor, a rail `rails::would_pop` refuses:
an ascending rail without its high side): the run fails, listing the cells,
instead of building without them; `World::place_raw` (and `place_like_judge`) panics on a non-air
cell outside a box world's bounds, listing them (a land world places anywhere).
`mctas mobfarm ticket-check --module M --loader L [--tiling 1|2|4] [--no-mirror] [--piglins DAYS
[--difficulty 2]]` (onechunk, ironspec's freeze ruling) places the module copies by the budget's
tiling around the loader's gap chunk (the loader at its frame + (17, 0, 17), chunk (1,1); modules at
(0,0), (2,0) and, at N = 4, the southern two mirrored in z: world z = 41 - frame z) and prints every
farm chunk's best level from 26.3's portal ticket (level 30 at the chunk of the exit portal block
found in the destination level: `addTicketWithRadius(PORTAL, .., 3)`; 30 + the Chebyshev distance;
<= 31 entity-ticking, 32 block-ticking only, 33 loaded; >= 32 a violation); `--piglins` adds the pad
portals' random-tick zombified piglins a copy-day (`NetherPortalBlock.randomTick`: a block's random
tick 3/4096 a tick, a spawn on difficulty/2000 of them, a player within 128) and their accumulation
over the season by the player's position. `--nether-group G [--twin-y Y]` adds every chunk of the
Nether group (the kill room, storage, shield and twin) against the twin's portal ticket alone (level
30 at the chunk of the twin's exit block, its portal blocks at or above Y; the kill room's own ticket
comes and goes with golem arrivals), each <= 31 to pass. It also prints the tiling's cells (ironspec
Nov 12 10:50, a re-lay into a gap chunk): every copy's and the loader's cells at the pitch, any cell
two of them claim, any cell inside another copy's `--keep-out x0,y0,z0,x1,y1,z1` box (the module's
frame, carried with each copy), the chunks each uses (and those used by more than one), and the least
distance from a copy's cells outside its own chunk to any other copy's cells.
The loader's bench (`mobfarm l1-item-bench`) also prints, per side, the longest time an arrival spent
inside the portal's cells (against the crossing copy's 300-gt portal cooldown) and the code-derived
bound on where a dispensed item crosses (`DefaultDispenseItemBehavior.spawnItem`'s x velocity
triangle(0, 0.103365), bounded; the item inside the portal's shape after its first move; a
non-player's transition time 0; 2 moves allowed, each decayed by the 0.98 drag).
Every portal's link as the game searches it: `mctas mobfarm portal-links --ow NAME=x0,y0,z0,x1,y1,z1[;...]...
--nether NAME=...... [--expect SOURCE=TARGET]... [--margin 0.7]` (absolute cells): from each
overworld portal the Nether portals within 16 of its x 1/8 point (y not scaled), 3D-closest, ties to
the lower y; from each Nether portal the overworld ones within 128 of its x 8 point; every
candidate's distance, the chosen portal per target, the expected partner's least margin, and any
empty search (a portal would be created there). An entity's centre is taken anywhere over a cell
grown `--margin` on x and z (a golem: 0.7; an item: 0.125).
A run's tick-level timing view (the carousel's parts and assembly): `mctas mobfarm timeline
<design> --seed S --days D [--terrain <sample> | --build K] [--tag '<tag>'...] [--from T --to T]
[--out DIR]` replays `mobfarm run`'s run (its counters printed beside the judge's: the same run tick
for tick) and writes `<stem>-seed<S>.timeline.html` (lanes on a canvas: drag, pinch or ctrl+wheel to
zoom, a tap reads every lane at that tick, `#t=<tick>&w=<ticks>` links to a moment) and
`.timeline.txt` (one line per change, `t=<ticks after the arrival> gt=<game time> day=<n>
dt=<day time> ...`, for grep): each farm villager's position class (asleep, at-bed, travelling, a
tagged region, held in a still cart, loose), activity, panic trigger, GOLEM_DETECTED_RECENTLY and
the tick it is free, wanting a golem, cart, HOME, LAST_SLEPT and CANT_REACH; each cart's position,
speed and rider; each bed's OCCUPIED and sleeper; every gameTime % 100 slot with each summoner's
10-box (redsim's box test) and result; the golems' spawns, deaths and exits. Tags, in the design
(`timeline: <tag>` lines) or `--tag` (design coordinates): `signal NAME x y z` (a block's state as a
lane: a counter's bulb, a release line) and `region NAME x0 y0 z0 x1 y1 z1` (a villager there is at
NAME; `station...` is the summoners' group). `VIOLATION <rule>` lines (the brief's timing diagram,
designs/briefs/farm-iron-carousel.md): `10.6` (a villager outside the summoning batch locked by a
spawn: the batch's late arrivals are told apart with the rest of the run, a leaving batch is not),
`follow-box` (a live golem in the follow box of a villager outside its summoning batch, the arriving
batch included: the villager's box inflated by its follow range, any tick, whatever the sensors'
phase), `follow` (a golem sensor's actual sighting away from the station, not of its own batch's golems), `arrival` (at a slot, a
member of the batch at or coming into the station that entered the station region less than 39 gt
before the slot and did not arrive panicking (the PANIC activity, or its trigger running, on the tick
before: the trigger's 60-gt run restarts with a one-tick gap); every slot's batch margin, the smallest of its
members', is a `margin at the slot` line, and the summary's min and mean), `16` (awake in REST within
16 of its home bed while the bed is OCCUPIED, or a HOME erased that way in the tick the bed fills),
`1200` (CANT_REACH older than 1200 gt in REST with a HOME; noted
whether the HOME went), `home` (no HOME at a dawn or midday), `sleep` (no nap since the last dawn:
LAST_SLEPT did not advance, by a sleep's start or a bench's ideal stand-in for the sleep routine; a
second nap the same night is allowed and counted: a `naps since the last dawn` line and the
summary's `second naps N over M villager-nights`), `gap` (a batch slot inside the golem condition's
window [LAST_SLEPT + 24,000, the next nap): that villager does not want a golem there; one flag a
window, with its slots) and `gossip` (a summon off the 100 gt grid). A part bench that is not a mob farm run
(the counter's) writes its own trace in the dump's format and draws it with `mctas mobfarm timeline
--from-dump <file> [--out DIR]` (`<stem>.timeline.html`): lines `t=<tick> gt=<game time> ...` then
`signal NAME=VALUE [(full state)]`, `mark NAME [text]` (a point event) or `VIOLATION <rule> <lane>:
<text>`, the rest skipped. A bridge recording (a `.rec` of `mctas mobai record`, the official
server's own frames) goes through the same lanes and checks with `mctas mobfarm timeline
--recording <file.rec> [--tag ...] [--out DIR]` (`<stem>.timeline.html` and `.txt`; tags in the
recording's world coordinates): each frame's villagers (pose, vehicle, activities, running
behaviors such as VillagerPanicTrigger, memories), minecarts, iron golems and block changes. What
the frames do not carry is inferred and said so: a golem's summoner (golemDetected locks its box at
599 and those that brain-tick after the summoner read 598, so the summoner is the last panicking
villager reading 599 in tick order), tries without a spot (never shown), the follow range (16, the
base) and a sensor lock (a fresh GOLEM_DETECTED_RECENTLY in a tick without a spawn). A bench whose
driver does more than `mobfarm run` (ironfarm's hold bench toggling its lanes' control blocks) keeps
the villager lanes and checks by calling `mobfarm::timeline::Timeline::start(title, &spec, &run,
seed, offset, &tags, ticks)` on its `Run`, `step(&mut run.world, k)` after each tick, `finish()`
and `mobfarm::timeline::write(&tl, dir, stem, 0, ticks)`; its control blocks can be `signal` tags.
A bench that reads the world's mob events itself and keeps them calls `observe(&run.world, k, &mut
seen)` instead of `step` (the events after `seen` are read, not drained).
`mctas mobfarm hold-bench <design> --from S --to S --timeline DIR` does so for the release + hold
part: the first seed's `hold-bench-seed<S>.timeline.html`, the stand zone as `station`, the holds as
`hold`, each lane's hold-end control as `line0`..`line6` (its arrival margins are the bench's own,
slot for slot). `mctas mobfarm station-bench <design> --from S --to S --timeline DIR` does so for the
station + portal pad part (regions `station`, `pad` and `hold`, signal `pulse`; the timeline reads
the world's mob events through `observe`, which the bench reads too, and its output is the same with
or without the flag).
The brief's two placement checks on a part's bench (farm-iron-carousel, "settled as placed" and
"built in order"; ironfarm's queue-loop-run flags on explore/ironfarm): `mctas mobfarm siding-bench`
and `linear-bench` take `--placed-raw` (the design file pasted raw, its `nbt:` lines merged, no shape
or neighbour update and no settle, a Nether part too), `--placed-in-order [GAP]` (the worlds empty,
each build step placed with the game's updates, a Nether part first, its world ticking through its
build and the player's 80 gt in the portal back, then the overworld's; each cell's priced gap, 20 gt
or 40 gt at a cell the bill charges place:edge, or GAP gt for every step; then the entities; the
cells the build leaves unlike the file fail the run), `--placed-all` (the judge's, the raw paste and
built in order, each reported) and `--build-gap-sweep` (built in order at every gap from 1 to 40 gt,
to the first that fails); `--drive-out F` with them writes the build into the drive (ironfarm's
`build:` header and `spawn S`, `act t set|nbt ...` per placement and fill, a Nether part's as `act t
nether-set|nether-nbt ...`, every time from the build's start). The order is
`mobfarm::placing::build_steps_of`: ironfarm's `build_order_of` (supports bottom up, containers
filled as placed, rails, redstone from its sources outward), each lit portal's cells at one tick
after every other block, fluid sources poured last, and the design's listed steps, its `build:` line:
`last x,y,z ...` (cells placed after everything else, in order: a latch's torch that is off at rest)
and `fills-last` (every container's contents a step of its own after every placement, no comparator
output merged: a sorter whose locks come with its redstone). `mctas mobfarm siding --out F` and
`linear --out F` write their files settled (`placing::settle_design`: every cell as the bench finds
it after the settle, flowing fluids included, comparator outputs as `nbt:` lines; `--unsettled` for
the generator's own states); `siding --alone` writes the siding by itself (no launcher, approach,
run-out, hold or carts), `--exit-brake` puts a braked powered rail past its exit cell and
`--cage-only` closes the bed's open north front (the bed reachable only from the cage);
`--wired` includes the give-up: by default `--give-up scheduler` (one input, F (-5,-3,-4), which
day-1's scheduler pulses 12 gt 118-150 gt after each arrival; the siding gates it with a copper bulb
lit from the arrival to the return and sets its visit latch, so a napper that cannot sleep still
leaves), `--give-up timer` (its own arrival latch and 220 gt comparator timer) or `none`;
`siding-bench` drives F as day-1's scheduler would (`--force-at 118,150`, `--no-force`) and prints
the pulses, the visits not asleep by them, the pulse to ready, the sleep onset after the arrival
(the earliest F's bound) and the pulse to the cart past the exit (the latest F's);
`--drop-force K` is the fault injection (visit K's napper cannot sleep and its F is dropped; it
prints what the stuck cart and the next visits do, a one-line summary per seed first), and
`--fault KIND:K` injects a missed or extra pulse at a stateful element (arr-miss, arr-extra,
r-miss, r-extra, f-double: a redstone block for 4 gt beside the element) and prints the visits
until a clean one; the sleep-onset line gives p50 to p99.99, the max and the 8 slowest. The siding's
V (visit under way) is a dropper set/reset latch (`MCTAS_SIDING_V=bulb` the old copper bulb), and
the forced exit's levers are on by default (the free cell a booster; the long nap path: the S line
four 4-tick repeaters, R's reset later, the kick chain [2,1,0,0,0]; `MCTAS_SIDING_SLONG=0` the
26-gt nap path with the chain [4,2,1,1,1]; `MCTAS_SIDING_BOOST=0`, `MCTAS_SIDING_MOVE=0..3`,
`MCTAS_SIDING_CHAIN=a,b,c,d,e` (0 = dust), `MCTAS_SIDING_RDELAY=d`, `MCTAS_SIDING_REXTRA=n` vary
them); it also prints when the siding's T turns back to entry-bed after each visit;
`siding-bench --unload LINE,SIDE,LEVEL,STATE,N[,K]` is the partial unload (tick-exact's
`reload::unload_some`, the world shifted 2 east so the loop's chunk lines fall on the siding as
day-1 pastes it: x -3|-2 and z -1|0; SIDE lo or hi the side dropped, LEVEL 32 or 33 its level;
STATE arrival, arriving, drop, nap, asleep, waking, fpending, kick, reboard or exit of visit K,
default 3; for N gt), `--unload whole,STATE,N[,K]` the whole part saved and unloaded
(`unload_all`/`load_all`, the bench following the new entity ids, its visits paused while
away), `w,SIDE,LEVEL,STATE,N` / `e,...` the one-chunk placement's junction lines (the world
shifted 8,3; STATE also `sent`, the cart on the launcher), `--unload-shift DX,DZ` overrides the
shift (0,0 writes a drive in the design's coordinates, the unload acts as drive lines), and
`--unload matrix[:K]` / `whole-matrix[:K]` / `chunk-matrix[:K]` every case at N = 3, 40, 240
(and 700), one line each: the visits until a clean one, naps lost, carts out without their
napper, jams; `MCTAS_SIDING_SEAT=2` adds design (b)'s napper-aboard sensor (a stone pressure plate
on the booster's glass ceiling, emulation only) and the bench prints its readings (turns on with
and without a seated villager in its band, seat heights). `mctas mobfarm edit <design>
--set=x,y,z=state` (repeatable; `air` removes the cell) sets blocks of a design in place: a fault
copy of a farm, say the bed set occupied=true so no napper sleeps;
`siding-bench --day-visits` (every visit by day: no napper sleeps) and `--fail-no-home N` (the
first N villagers' HOME erased before each of their visits) are its forced failures: every cart
must still be on the slot with its villager within 500 gt of its arrival;
`siding-bench --release-held` holds the release input on all run (each napped cart leaves the slot
as it reaches it). The siding bench sets no villager memory (HOME = the bed comes from the design's
`port home:`, R1's state), gives each of the 21 villagers one visit a night (the full nights after
the spawn) and prints ironspec's check: every villager's HOME at every dawn and midday, naps per
villager-night, and each HOME loss and its reclaim. A design with `port home:` pays R1's N - 1
break-and-replace steps in its bill (prices.tsv's `bed_reclaim`). `MCTAS_BUILD_WATCH="x,y,z n1,59,0 ..."` prints the watched cells (`n`: the
Nether's) on stderr whenever one changes during a build.
The judge's speed, results unchanged: `mctas farmopt bench <spec> --n 16 --quick 8 [--full N]
--threads 16 --out <file>` evaluates N candidates spread evenly over the spec's space level by
level (static, quick for the first `--quick` passing static, full for the first `--full` passing
quick), prints each level's wall time and time a candidate, and writes every result with a digest;
a speed change keeps the digest of the binary before it.
Real terrain and natural spawning (`mctas terrain gen | find | info | crop | map | spot | light |
probe | chunk | blockinfo | biomes | paste-check | biome-check | spawn-check | spawn-explain |
hostile | slab-fixture`, and `mctas mobfarm exposures | spawn-rules`: the official server's world
generation read back into `.mcterrain` samples, the natural-state and biome tables, a design pasted
at a spot, the checks against the official server, the worst-case checks for natural mobs and C1
against the game's own spawn rules) are in [designs/terrain/README.md](../designs/terrain/README.md).
One tool is not a `mctas` subcommand: `trends` (`./target/release/trends suggest | related | rank`,
crates/trends), the Google autocomplete and Google Trends ranking of the farms and redstone builds
people search for. It needs the internet, so it is never used for design work; it is in
[search-demand.md](search-demand.md).

## The workflow behind the records

1. Validate a mechanism with ideal inputs: redstone blocks placed and cleared by a `door drive`
   script, or a `door ideal` schedule. (`door mech-synth` finds mechanisms and writes both.)
2. Wire it, by hand or with `door route` / `door route2` / `door mech-wire`.
3. Run `door verify <design> --extra-offsets 28` (the judge, 32 positions).
4. Shrink it with `door minimize`, `door resynth`, `door grow` and `door regrow`.
5. Keep it: `door verify ... --archive` copies it into the archive with a header.

Proven circuits go in the gadget library, `designs/gadgets/` (a small design plus a drive script
whose `expect`s prove the behavior; catalog in [mechanics.md](mechanics.md#10-gadget-catalog)).
Run them all with `mctas door drive --quiet designs/gadgets/*.drive`.

## `mctas test`: the tests of a change

The session that changes something runs the tests of ITS change, not everything (vjeux
2026-11-08: "The session that changes the redstone simulator should run the tests to make sure
that the changes are safe. The session that changes a farm should run the tests to make sure the
farm is working. We shouldn't test everything for every change."). `mctas test`, from the
repository root, reads the change (the tree against the merge base of HEAD and `origin/main`:
committed, staged, unstaged and untracked files), prints the plan with the files behind every
piece, runs the pieces in parallel sized from the machine (CPU threads: the cores minus the load
already there; JVM workers: half of those, at most one per 1.3 GB of free memory;
`MCTAS_MAX_WORKERS` still caps every pool), prints PASS or FAIL per piece with its time and the
slowest five, and ends with the line for the commit message (`Tests: mctas test (...)`).

| the change touches | the pieces |
|---|---|
| the simulator: `crates/redsim` (its mob, entity, fluid, light and weather code too), `crates/redsim-fuzz`, `bridge/`, mctas' `sim.rs` / `bridge.rs` / `vanilla/`, the differential tools (`pistfuzz.rs`, `vanilla_*.rs`, `crates/door-redsim`, `crates/door-vanilla`) | `sim:recorded-*`: every recorded game run replayed on redsim, no JVM, seconds (below); `sim:redsim-tests`: redsim's own known answers; `sim:fuzz-<area>`: the live differential fuzzers (the bridge's game against redsim) of the areas the files belong to (redstone, pistons and moving blocks, containers, sculk; redsim's engine, `world.rs`, `ticks.rs`, `block.rs`..., all of them); `sim:vanilla-*`: the unmodified official server against the bridge (the bridge or the official-server code) |
| a judge's or a study's code (`crates/piston-door`, `crates/door-*`, `crates/clock*`, `crates/circuit*`) | that study's checks on its calibration designs, on redsim and in the game (`door selftest`'s checks of the study); a circuit study's module also picks the studies whose modules call into it, the circuit judge's shared code every circuit study; code the recordings replay through (the clock crate, the circuit judge, the sorter / smelter / brewer / composter modules, the shulker replayer) also the recorded pieces (seconds); never a live fuzzer |
| a design: a door, clock, circuit, farm or gadget | its own judge: `door verify <f> --extra-offsets 28`, `clock verify <f>`, `circuit verify <f>`, `door drive <gadget>.drive`; a calibration or selftest file: the checks that read it; an archive table: `archive:tables`; a recording or its frozen design: `sim:recorded-*` |
| a tool crate no calibration piece covers: `crates/mobfarm` (but `exact.rs`), `crates/farmopt*`, `crates/bedrock`, `crates/terrain`, the site's and the viewer's crates (`site`, `site-model`, `web-cli`, `mcview`), `crates/litematic`, `crates/trends`, the video and download code (`crates/door-video`, mctas' `video.rs` / `mcfont.rs` / `fabric.rs` / `net.rs`), the circuit judge's mob-farm and bedrock study files | `unit:<crate>`: the crate's own tests (`cargo test --release -p <crate> --all-targets`), a build failure fails with its compile tail, a failing test fails, a crate with no tests passes saying "no unit tests in <crate>" (no code gate for our process: the parent's ruling, ironspec 2026-11-12 19:42 PT), then `cargo clippy --release -p <crate> --all-targets` where the toolchain has it (a deny-level error fails only when it sits in a file the change touches; every other finding, and every finding of a full run, is counted and doesn't fail) (ironspec 2026-11-12: three `crates/mobfarm` commits had landed on "0 pieces, nothing to run") |
| docs/, web/ | nothing (`site publish` / `deploy` run no tests) |
| the test tool itself, `Cargo.toml` / `Cargo.lock`, the shared design format (the rest of `crates/mctas`) | everything |

Every crate's code also picks the crate's own unit tests (`unit:<crate>`, the verdict above), beside
its row's pieces, and a full run (`--all`, or a change that picks everything) runs every crate's:
the crates are the git root's `crates/` directories (a list the plan cannot read, or an empty one,
fails the run as `unit:workspace`, never runs less); redsim's tests run in `sim:redsim-tests`
(`testplan::TESTS_RUN_BY`), which every redsim rule picks (ironspec 2026-11-12 19:42 PT).

The rules are the table `testplan::RULES` (crates/piston-door/src/testplan.rs, most specific
first; its tests check that every crate has a rule, that every rule under `crates/` picks at least
one piece, that a redsim change runs its tests in `sim:redsim-tests` and picks no judge, that a
judge change no live simulator test); the pieces are `door selftest`'s checks (`door selftest --list`:
each check's group and JVM workers; crates/piston-door/src/selftest.rs). Flags: `--plan` (print
the plan and stop), `--all` (every piece, on demand: never a scheduled job), `--pick sim,sorter`
(add groups), `--design <f>` (add a design's judge), `--only <names>`, `--fail-fast` (stop at the
first FAIL), `--from <ref>`, `--jobs` / `--jvms` (override the machine's sizing), `--keep`.
Typical times on a 166-core box (2026-11-08): a redsim change 47 s (11 pieces: the recordings,
redsim's tests and the live fuzzers of its areas); with the bridge or the official-server code,
the `sim:vanilla-*` pieces too: 2.6 min; a study's code its study's checks
(the slowest, the shulker study, about 5 min); a
design its own judge (a door: about 35 s); docs nothing. The old single gate took 28 min.

**The mutation test of the pieces** (crates/piston-door/src/mutate.rs, [coverage.md](coverage.md#the-simulators-tests-mctas-test-2026-11-08)):
`mctas test --mutants designs/redsim/mutants.tsv --tree <a git worktree> [--suite '<mctas
args>'] [--only <mutants>] [--no-control]` applies each one-line redsim bug of the file in the
worktree, builds it, runs the suite there (default `test --from HEAD`: the pieces the bug's own
diff picks; the old suite: `--suite 'door selftest'`), records CAUGHT (by which pieces, the time
of the first FAIL) or MISSED, and restores the file; first the suite must pass without a bug
(the positive control). The report: `<tree>/target/mutants/report.txt`.

**Recordings.** The game never changes (26.3), so a run of the real game recorded once is the
game's answer for as long as the design it was recorded from stays the same; redsim replays it in
milliseconds. The exact traces (`designs/clocks/traces/exact`, clk-misc's and clockinfra's
`redsim-tests/traces`, `designs/circuits/shulker/traces/exact`, `designs/boat-leads/traces`,
`designs/redsim/recorded/doors`, `clocks` and `lifts`: `door trace-compare`, `circuit shulker compare`,
`boat-leads compare`) and the judges' recorded frames (`designs/redsim/recorded/<kind>`: `circuit
replay`) each sit in a directory with a manifest,
`recorded.tsv` (`mctas recorded check | write`, crates/mctas/src/recorded.rs): every recording's
hash, the design it was recorded from (a frozen copy in the test tree, so design work never makes
a recording stale) and its hash, the game version, when and how it was recorded. A changed
design or recording, a recording without a row and a row without its recording FAIL (`STALE`,
`EDITED`, `UNLISTED`, `MISSING`); the game's `door ideal` verdicts of `sim:recorded-ideal` carry
their plans' and designs' hashes in selftest.rs the same way. To add one: `circuit record --kind sorter-run|smelter-run|
brewer-run|composter-run|circuit-trace [--run R] --dir designs/redsim/recorded/<kind> <design>`
(the game and redsim must agree: nothing is recorded otherwise; the frames are stored gzipped) or
`clock misc trace-export --exact` on a frozen copy, then `mctas recorded write <dir> --fresh --how
'<the command>'` (`mctas recorded hash <files>` prints the hashes the manifests use).

## Setup and the game's source

| command | when to use it |
|---|---|
| `mctas setup` | once per machine: installs Java 25, the 26.3 server jar and libraries, and compiles the bridge (everything lives in `$MCTAS_HOME`, default `~/.cache/mctas`) |
| `mctas src decompile` | once: readable game source in `$MCTAS_HOME` (Vineflower) |
| `mctas src grep\|find\|show\|path` | to read the ground truth for a mechanic: `src grep 'class PistonBaseBlock'`, `src show PistonBaseBlock --grep getNeighborSignal -C 12` |
| `mctas src --client decompile\|grep\|find\|show` | the CLIENT jar's source (models, renderers: `src --client show BoatModel`), decompiled once from the client jar already downloaded (`versions/<v>/client.jar`) |
| `mctas sim repl` / `sim place` | talk to a simulator in JSON lines ([internals.md](internals.md#bridge-protocol)) / place a design and show what is pending |
| `mctas bench` | measure throughput and memory with many workers and sandboxes |

## The judge and looking inside a door

| command | when to use it |
|---|---|
| `door verify <design> [--extra-offsets 28] [--timeline] [--cycles N] [--archive] [--no-rotations] [--orientations] [--tile <spec>]` | the judge ([rules.md](rules.md)); prints blocks, category, open/close ticks and the rotation check. `--archive` keeps a copy of a working design in `archive/` next to it (skips duplicates); `--cycles 40` catches slow failures like oxidizing copper; `--orientations` runs all 8 orientations; `--tile wide:N\|period:P:N\|stack:N\|vperiod:Q:N\|step:dx,dy,dz:N` judges the design as a tile module ([rules.md](rules.md#tileable-doors-door-verify---tile); `--tile-out` writes the arrangement) |
| `door trace <design> [--close] [--ticks N]` | print every block change tick by tick after each lever flip: how a door actually moves |
| `door drive <script>...` | place a design, apply scripted inputs (`set`, `clear`, `use`, `step`, `quiet`, `print`, `plane`, probes, `expect`), print a plane per tick; any failed `expect` fails the command. Validates mechanisms with ideal inputs and runs the gadget tests. Block entities (item clocks): `place <design>` places like the clock judge without settling (a design's `nbt:` lines merged after the raw paste), `data x y z <snbt>` (= `data merge block`), `items x y z`, `probe_items <name> x y z` (a container's item count) |
| `door ideal <schedule> [--set name=value] [--sweep name=lo..hi] [--between-ticks]` | judge a mechanism driven by scheduled ideal inputs (`ideal.rs`) and sweep its timing windows: which input ticks work. Since 2026-10-19 an off-plane redstone-block input at `t >= 2` acts in the TILE-TICK phase of tick `t`, where a wired observer's output arrives (built from a wall torch on a support block, so the game and redsim agree); the old rule (between ticks, after tick `t`: 1 gt optimistic for observer-driven inputs) is `--between-ticks` or a `phase between` line in the plan; other tools that parse plans keep the old rule unless the plan says `phase tile` |
| `door ideal-rs <schedule> [--set] [--sweep] [--vary x,y=state\|state\|keep\|air] [--vary-max K] [--without x,y,z] [--drop N] [--ok-only] [--trace]` | the same on redsim in milliseconds (`ideal_rs.rs`); `--vary` screens every combination of block alternatives per cell (in memory: e.g. sculk sensors, observers, wool around a missing input: 10-20k variants/s), `--vary-max K` keeps only combinations changing at most K of the cells (pruned while building: every single or pair of added blocks over a whole region, e.g. 20 cells x 6 states = 6961 plans in 0.3 s), `--drop N` every removal of up to N blocks (a redsim minimizer), `--without x,y,z` (repeatable) drops every plan event at that input cell (the input left to the `--vary` filling: a region screen without editing the plan). Found bridge-p2-sculk and bridge-e88-sculk ([tracks/bridge](../designs/piston-door-3x3/tracks/bridge/NOTES.md)) |
| `door feeds <schedule> [--box X0..X1,Y0..Y1] [--sculk] [--strict]` | per firing of an ideal plan (each redstone block it places): the pistons it powers and where a FLAT build could feed them (`feeds.rs`): cells that stay free all cycle inside the box (regions, their sizes and their cells; cells only the plan writes count as free) next to the piston (not its front), quasi-connectivity into the cell above it, a conductor block powered from a free cell, or a fixed observer of the design pointing in; firings with no option are listed last (cores for static wiring). Not checked: component support, co-firing, timing. On d67's q7b: 3 of 42 firings unfed (b at (0,-2) twice, SU(1,-2) at open). Regions: the cells each free region can watch and their first change per edge; a region that is the only feeder of a firing but sees no change 2 gt before it cannot start (free regions are electrically isolated: no path crosses a piston or two blocks; the lever's own input is exempt). On c70b's u0-7x7: (0,-2), the top-right and the bottom-right never start (d67 NOTES "Wiring u0-7x7"); pockets3's `cosearch pockets` ISLAND START is the replay-based version. `--sculk`: per free cell, when a sculk sensor there (normal: radius 8, active 30; calibrated: radius 16, active 10) would activate on each edge, from the plan's own vibrations (lever flips, piston extend = activate 10 / retract = deactivate 9): the first event heard while idle commits it, activation T + max(1, floor(d)), deaf until INACTIVE again (d67 NOTES "Sculk on u0"). `--strict`: the screen for searches' exports (macrosat --strict SATs before they go to wiring): a `via` conductor must stay the same block all cycle (`*` otherwise), and the plan FAILS (exit code 1, one `# strict:` line with the reasons) on a firing without a feed, one with flagged options only, a one-cell conflict or a region that cannot start; macrosat's m86-a fails it (15 weak, 7 starts), pockets3's wired e78k-s11 passes |
| `door shift-plan <schedule> [--shift x,y,z=k\|@name]... [--rest k\|@name] [--out F]` | re-time a plan: every event of the listed input cells moved by its shift, the others by `--rest`; `@name` writes `name+<tick>` and a `let name = 0`, for `ideal-rs --sweep name=lo..hi` (e.g. one area started later than the lever, a stack's levels at any tick: m69T 1681/1681) |
| `door prune-plan <schedule> --out F [--only x,y,z]... [--keep x,y,z]... [--exhaustive] [--extra-offsets 28] [--cycles 2]` | drop the input pulses a plan works without, TOGETHER: greedy, latest first, each drop screened at 4 positions and confirmed at 32 (pulses `door shuttle` lists as spare one at a time often can't all go); `--only` limits the drops to one area's input cells, `--exhaustive` tries every subset of the candidates (<= 16) for the largest one; level edges, lever uses and `--keep` cells stay. It checks the mechanism only, not island starts: `--keep` any pulse a wiring area needs to SEE as its start trigger (gf16: SW(4,1)'s, pockets3). Lighter areas for pockets / satwire (q69c-sh0: 4 close pulses) |
| `door cosearch pockets <flat design> --assist <plan> --box X0,Y0:X1,Y1 --focus X0,Y0:X1,Y1 [--keep-lever] [--offplane] [--k 1 --only-pockets 99] [--why-islands] [--sculk-islands] [--pin-assist x,y;..] [--fill-palette all,sculk,calibrated,wool]` | wiring screens of an ideal plan (track pockets, `cosearch_search.rs`): the free areas ("pockets"); FEED (per input: the pocket cells next to its piston or its QC cell, a lever outside the box on a never-moved piston's back); LOAD (per pocket: the distinct ticks only it can fire); ISLAND START, timed and co-fire aware (a pocket fires only after a change it can see, caused by a pocket already started, or the lever; a direct feed also QC-powers the piston below, a QC feed through a piston's cell powers that piston; prints the pockets that never start in time and the plan groups no area can fire, `--why-islands` the trigger of each); then EXHAUSTIVE fillings of every pocket up to `--k` blocks, each screened in the whole door with the other inputs ideal (which plan groups it reproduces), and combinations across pockets. `--k 1 --only-pockets 99` = the reports alone (seconds). Wants per-piston inputs (mech-synth style); macrosat exports go through `door ideal-rs --assist-dir` first |
| `door shuttle <schedule>... --target x,y,z[;x,y,z] \| --unfed \| --spare-only [--per-cell] [--edge open\|close] [--partial] [--train] [--absorb] [--payload observer,redstone_block] [--radius 3] [--holds 3,4,5,6,8,10,12] [--out-dir DIR]` | feed pistons that have no free neighbour (a wiggle piston boxed in the frame, a piston on a drop cell) from a SHUTTLED observer or carried redstone block (`shuttle.rs`, track clk-rt): a sticky piston carries the block one cell out and back, and a moved observer fires 2 gt after it lands, into the cell behind it, so one shuttle pulse gives two observer pulses (arrival, return). Every placement (piston cell and facing, carried block and facing, free landing cell, near a fed piston) and per-pulse arrival/return timing is judged on redsim with the target inputs removed, then at 32 positions; `--absorb` greedily drops other plan pulses that the observer's free pulses or extra shuttle pulses give, and lists pulses the plan works without anyway; `--unfed` targets every input `door feeds` finds no option for, `--per-cell` searches each target alone (batch mode over many plans: one line each). Target pulses the plan works without are reported first (each alone, then the largest set dropped together, confirmed at 32 positions; `--train` drops a tick only with both edges' copies, for one-train plans) and `--spare-only` prints just that report for every input: free load cuts (fly's r69t: 9 pulses, q68 / q69c / q69h: 4). `--partial` keeps shuttles that replace only some target pulses (load relief). Re-finds mm01z (c70b's 7x7, P + 5 of Q's pulses) and s69z (fly's 6x9, Qv) in under a second; on fly's s69 a riding observer feeds Qv o6/o18, B1 o24 and Qv@ o30 (wip/s69r). In-plane plan inputs block shuttle cells: move them behind their pistons first (`ideal-rs --offplane-dir`) |
| `door reuse-check <design>` | verify many times in one reused sandbox: catches state leaking across runs (torch burnout memory) |
| `door frontier [--check <file>...] [--any-category] [--all]` | the Pareto frontier over (open, close, size: the volume) from the verdict and properties caches; `--check` says whether a candidate lands on it or which doors dominate it |
| `door shapes [<design>...] [--branches] [--rule <preset or key<=n,...>]` | every working design's shape and swept shape relative to its door, the best rotation-safe design per shape rule (wall, wall2, flat, ground, no-above, narrow1, narrow2, wall-ground) and league, and the (volume, blocks) frontier |
| `door tile-scan <design>... [--tiles wide:3,period:4..12:3] [--levers one\|each]` | every design under every tiling, in parallel: which tilings work and the smallest working period |
| `door layouts [--size 7x8x1] [--category solid] [--out FILE]` | the distinct PISTON LAYOUTS among the working doors of one size (main, origin/main and `explore/*` designs the leaderboard caches judge WORKS with those volume dims): a layout = the pistons at rest (position from the door, facing, sticky), wiring ignored; identical layouts grouped, left-right (and up-down) mirrors paired, layouts that contain a smaller found layout listed under it as its core; per layout doors, best blocks, best open/close, an example, and the first find (earliest commit carrying the exact file of one of its doors, and its track). Reads caches only (no simulation, ~15 s) |
| `door leaderboard [--fetch] [--sync-tracks] [--archive-dir DIR] [--cache-dir DIR] [--cached-only] --out designs/piston-door-3x3/LEADERBOARD.md` | judge every design on main and `explore/*` with main's verifier (verdicts cached by content in `archive/verdicts-v5.tsv`) and write the boards, each leader with its GIF, video and caption, ranked by size (the volume, [rules](rules.md#the-judge-mctas-door-verify-design---extra-offsets-28)). `--archive-dir` also archives every working design; `--cached-only` judges no new design (no cached verdict = left out); working designs without a volume in `archive/properties-v1.tsv` are measured (one recorded cycle each) and cached either way |
| `door catalog [--videos DIR]` | write the archive catalog `archive/README.md` (every archived design: size (volume), blocks placed, boards, ticks, rotations, buildability, video, how it works) from the verdict and properties caches; records nothing |

## Building designs

| command | when to use it |
|---|---|
| `door gen <name> --out <file>` | print a built-in design (`designs*.rs`, `lm.rs`, `observer_seq.rs`) in the text format; `@name` also loads a built-in anywhere a design is expected |
| `door patch --out <file> <design> <script>` | apply an edit script: `set x y z <state>` (an extended piston gets its head), `clear x y z`, `port <name> x y z` |
| `door turn <design> --out <file> [--turns N] [--mirror]` | write a design turned and/or mirrored by the game's own block-state rotate/mirror; a quarter turn gives an east/west-facing door (`door-axis: z`): the way to build a variant for that direction |
| `door route <design> --start='x,y,z;..' --goal='x,y,z' [--allow=..] [--swept-design <file>]` | dust auto-router: fewest new blocks, avoids cells the machine sweeps, the walkway, the 3x3 columns in front of and behind the door, and touching other components; `--swept-design` takes the swept cells from the working design a net was cut from |
| `door route2 <design> --out <file>` | port-aware, timing-aware net router: rips up groups of up to `--max-rip` control blocks and re-routes every receiver to get the same signal tick for tick with fewer blocks (dust, repeaters, observers, conductors); `--need` routes a new net |
| `door place <design> --groups <file> --out <file>` | try every combination of alternative patch groups: wiring a mechanism whose part placements are a 3D puzzle |
| `door mech-synth <base> [--add k] [--pulses-only] [--verify N --out-dir DIR]` | mechanism synthesis in a macro piston model (a port of `PistonStructureResolver`): pistons, slime and blocks around the door, cheapest open and close paths, one symmetric event sequence for both lever edges; writes ideal-input designs and `door ideal` schedules and replays the best. Catalogs: `tracks/mech-synth/NOTES.md` |
| `door mech-wire <schedule>...` | wire a mechanism from its ideal-input schedule: route every piston cell's wave from a lever at each lever spot, rank by total blocks, judge the best (an upper bound: ~12 blocks worse than hand wiring) |
| `door topology <families CATALOG.tsv> [--only 7x8,8x7] [--verbose]`, `door mech-synth <base> --just-base --topology x0,y0:x1,y1` | can a mechanism be wired inside a volume box with FULL seamless at all? (topology.rs, track box56): the free cells split into wiring components; a piston links two components when one feeds it and the other watches it, carrying one pulse per change of the watched cell (pulse mergers); reports WIRABLE / NOT per mechanism under strict and permissive parity (accepts e89, h44, flat-b-v7; rejects all 99 7x8/8x7 catalog entries under strict parity) |
| `door connect <families CATALOG.tsv> --boxes 7x9,7x8 [--only ids] [--cap N] [--verbose]` | connectivity + ORDER check (connect.rs, track wire64b), parity ignored: every placement of each entry in each box; regions of free cells (merged through unswept ring stones); a piston is fed from a region next to it or its quasi cell, directly, through a ring stone or through a door block parked there (conduction); a region wakes when it can watch a change; every event order (pulses, levels on pistons extended at rest) is searched with only the lever's region awake at each flip. A SEALED free cell (a region on its own with nothing beside or below it that stays put) holds an observer only: it feeds the cell opposite the change it watched, and reads no powered block. The lever is tried in every region and also OUTSIDE the box (the volume excludes it): on the back face of a piston that never moves (it holds that piston and, by quasi-connectivity, the pistons whose above cell touches the lever: e88's top trio) and on the outer face of a static block (track d67). NOT wirable = no order works (exhaustive), UNDECIDED = the state cap was hit. Accepts e89 and flat43-full in their 8x9 boxes, rejects both in 7x9; accepts the 7x8 e78s-56b-43 and the t7b family in their own boxes |

## Searching for fewer blocks or ticks

All candidates are screened at a few positions and accepted only by the full judge. The searches
(and the routers) keep the 3x3 columns in front of and behind the door clear. Every search takes
`--shape <preset or key<=n,...>` (only designs within those shape limits; the judge also checks the
swept shape) and `--tile <spec>` (judge each candidate as a tile module); `minimize`, `regrow`,
`grow` and `harvest` take `--rotation-safe` (accept only designs that also work turned). `minimize`,
`regrow`, `grow`, `harvest` and `resynth` take `--repeater-input` (accept only designs that also work
with repeater input, the Squid regulation's 3.1.3, at every judge position) and `--eligible` (that
plus the hallway check, footsteps for sculk designs, QUART SEAMLESS or better and the lever's place).

| command | when to use it |
|---|---|
| `door minimize <design> --out <file> --moves all [--plateau] [--depth 2] [--archive]` | local search (`search.rs`): removals of 1-3 blocks, repeater delays, moves, substitutions, swaps (2-3 blocks out, 1 in), dust re-routing and net re-synthesis; `--plateau` walks neutral edits; `--objective open\|close\|total\|volume` optimizes speed or the community volume (`door properties` volume; `--slack N` lets blocks grow); `--max-open/--max-close` keep a speed; `--to-sculk` lets it turn a solid design into a sculk one |
| `door harvest --out-dir DIR --moves all --plateau [--every 20]` | run that search on the best working designs of every branch and board not harvested yet |
| `door transplant` | learn edits from pairs of designs (a, then a smaller variant of a) and try each on the best designs of every board |
| `door grow <design> --out <file> --lo --hi --states [--k 2]` | add 1 or 2 blocks in a box to a design (usually one whose old wiring was cut), screening every combination; `--states` takes shorthands like `leaves` (persistent oak leaves) and `log` (3 axes); `--then-search --glue` seeds riders |
| `door regrow <design> --out <file> --size N` | cut every face-connected group of N control blocks and re-grow it with N-1 blocks; `--any` cuts any 2 blocks |
| `door synth <context> --lo --hi --from --to [--k K] [--palette ..] [--spec <script>]` | circuit synthesis by exhaustive simulation: every circuit of up to k blocks between two cell sets, judged by the door judge or a behavior script |
| `door resynth <design> --out <file> [--max-size 4] [--palette ..]` | large-neighborhood search by exact synthesis: clears every connected group of 2..max-size wiring blocks and tries every smaller circuit in its place (gravity 45 -> 42, flat 49 -> 43) |

## Correctness

| command | when to use it |
|---|---|
| `door material <designs> --out DIR [--extra-offsets 28] [--judge] [--legacy-walkway]` | OBSIDIAN ONLY WHERE NEEDED (vjeux 2026-10-21; `material.rs`, track clk-rt): every obsidian block tried as stone on the redsim door screen at the record judge's 32 positions x 3 cycles (all at once, else one at a time, then the passing ones together); a swap stays only when the door passes with the same open and close ticks. What stays obsidian is needed as an immovable block (doors have no explosions). The changed designs (header kept, plus a material line) and material.tsv go to DIR; `--judge` confirms them with the real judge at the same positions AND its rotation check (the design built turned 90/180/270 degrees, as the leaderboard judges): a stone design that breaks a turned build its original passes keeps its obsidian; `--legacy-walkway` for designs that predate the walkway rule. `door verify --archive` refuses a door whose obsidian can be stone (redsim screen, then the game's rotation check, only for such a door). 2026-10-21: 185 of the 399 archived doors with obsidian changed (198 blocks; the same ticks, rotations, category and size); 10 more passed the screen but broke at 180 degrees with stone and keep their obsidian (bridge-p2-sculk-r-49b, e78st-56b-41-2s-41b, e78st38-s2-fill-39b-*) |
| `door selftest [--quick] [--only a,b] [--list] [--jobs N] [--jvm-jobs N] [--keep]` | every known-answer check of the search and judge tools and of the simulator, the table `mctas test` picks its pieces from (above): the redsim judge and `door verify` on the archived anchors (c79, e88-t7b, t7b-nb2, s27-bl52, m79s-sculk) and on a broken c79 (must fail), the periodic judge on c79 + a once-per-cycle copper bulb, pockets re-finding c79's bottom-left and c88 s25's gapped bottom-right chain (and not without `--gaps`), satwire `--from-wired --check` = 0 mismatches on c79 and `--update-order` rejecting a saved 48-block BUD candidate, boxproof on c79 (SAT with `--split-rounds`, UNSAT without, a freed cell re-found, the model vs redsim fuzz), `door ideal-rs` vs `door ideal` on 3 plans (one with a 1 gt pulse), generator membership of docs/audit.md's acceptance list, the clock and circuit studies' calibration (redsim and the game), the simulator's recorded game runs (`sim:recorded-*`) and live fuzzers (`sim:fuzz-*`). One PASS / FAIL line per check, run like `mctas test --all` (in parallel, sized from the machine; `--quick`: the checks that start no JVM); on demand only (selftest.rs; fixtures in `designs/piston-door-3x3/selftest/`) |
| `door buildable <design>... [--positions 32] [--lever off\|on] [--out-dir DIR]` | can a survival player build it? searches for a placement order (real clicks on visible faces, the world updating after each, temporary blocks allowed) that ends exactly in the judge's rest state, then runs the judge's cycles from there; writes `*.build.txt` scripts (`tracks/buildable/scripts/`, read by the catalog) |
| `door vanilla-check <design>... [--extra-offsets 28] [--trace]` | run designs in an UNMODIFIED vanilla 26.3 server (official jar, RCON, a bot player clicks the lever, region files read back) and compare every tick with the simulator under the judge's rules, plus one real-time cycle. PASS = identical tick for tick; DIVERGES names the first differing tick and cells. Method: `tracks/vanilla-check/NOTES.md` |
| `door vanilla-build <script.build.txt>... [--real-time] [--slow N]` | replay a `door buildable` script on the unmodified server with a bot player (item in hand, the script's look, sneaking, a right-click on the given face), compare after every action, then run the judge's cycles with the bot. Method: `tracks/vanilla-build/NOTES.md` |
| `door vanilla-gadget designs/gadgets/<g>.drive...` | replay gadget drive scripts on the unmodified server and compare every tick ([mechanics.md](mechanics.md#11-vanilla-confirmation)) |
| `door vanilla-fuzz [--count 50]` | differential fuzzing: random small contraptions compared between the simulator and the official server |
| `door pistfuzz [--count N] [--mixed] [--shapes] [--slime] [--observers] [--gravity] [--gadgets] [--designs <files>] [--replay <files>] [--shrink] [--trace]` | differential fuzzing of `crates/redsim` (the exact Rust model: pistons, moving blocks, redstone) against the bridge: random piston setups (7x7x1 flat / 6x6x3) with scripted power (`--mixed`: redstone components next to the pistons; `--shapes`: slabs, trapdoors, scaffolding, fence gates), the gadget library's drive scripts, archived doors; every block state, moving block entity, queue size, drop and the idle flag compared after every tick; failures shrunk and saved as replayable scenarios ([tracks/pistmodel](../designs/piston-door-3x3/tracks/pistmodel/NOTES.md)) |
| `door mechmut <seeds> [--repair] [--add] [--validate] [--rank-unfed] [--max-actions K] [--no-inside] [--reserve X,Y;...] [--keep PIECES] [--reserve-wiring] [--macrosat-qc]` | local search from known mechanisms (`mechmut.rs`): each neighbour (drop / shift / turn a piston, toggle sticky / extended; `--add`: one more piece) solved in families' macro model (the parent's schedule replayed first), kept with a FULL frame, `--validate` replays it in redsim; ranked by redsim, extent (w86's `ideal-rs --extent`), unfed receivers, island start, d67's `feeds --strict`, feeds, pocket load, actions per piston. Seeds: lines `W H x0 y0 pieces`, or `door macrosat` dumps (a file or a folder: the rest grid as the seed, its actions as the first schedule, solved without QC coupling unless `--macrosat-qc`; a dump's own `export/c-<dump>.txt/.ideal` pair is judged directly: redsim, strict feeds, extent, and flagged CANDIDATE when all pass) |
| `door fullmech [--check <designs>] [--boxes 7x8,8x8] [--pulses 1,3] [--levels] [--pairs] [--groups] [--reduce] [--shrink] [--export DIR] [--verify-ideal <plans>]` | mechanism search on the exact model: every transition is the real tick-level game (spitting, 0-ticks, the block-event order, pistons carried by pistons keep their input, carried redstone blocks / observers); ideal inputs are physical (a redstone block behind the piston: QC powers the piston below); FULL seamless open + close back to rest. `--boxes` re-solves `door families`' candidates; `--export` writes each solved mechanism (design + exact `door ideal` plan); `--groups` merges inputs into the fewest independent signals (+ `-groups.ideal` plans); `--reduce` tries helpers (redstone block / observer) that remove inputs or signals; `--shrink` drops a piston and repairs with one helper; `--verify-ideal` replays plans in the real simulator; `--anneal` samples mechanisms outside the generator |
| `door eligible <design>... [--extra-offsets 28]` | record eligibility under the Squid regulation (section 3), one line per design: the judge + rotations, repeater input at every judge position (props-dyn's rig: the lever left off, a delay-1 repeater into its support block, both edges), the hallway check, footsteps (sculk), the SEAMLESS tier, the community volume, the lever's place; ELIGIBLE or what fails ([tracks/eligible](../designs/piston-door-3x3/tracks/eligible/NOTES.md)) |
| `door relever <design>... [--subs note,dust] [--out-dir DIR] [--why]` | make working doors repeater-input eligible: the lever's cell becomes a note block (the lever hung on it, or kept on its old support) or dust (the lever kept on its support); every variant judged like a record and checked with `--eligible`; table with ticks, seamless tier and volume |
| `door audit [designs or folders...] [--sections checks,mech,wiring,rules] [--jvm] [--out F] [--tsv MECH.tsv] [--explain]` | what our rules, checks, generators and wiring tools silently exclude, found on WORKING doors ([audit.md](audit.md)): every check on each design, its mechanism extracted from a redsim cycle and tested against families' own generator under each bound setting (the exact bound when it is out), connect / topology in its own box, what satwire / cosearch / boxproof cannot hold, every hard-coded rule with live file:line; `--explain` prints one door's paths and every move with the piston that made it |

## Videos and showcase (coordinator only: the X displays are shared)

| command | when to use it |
|---|---|
| `door video <design> --out <f.mp4> [--redo] [--relabel]` | the door opening and closing in the real Minecraft client: title card, then the design's one clip (still door camera; each flip plays until the door has settled) with the corner label (the size `80b` big, the blocks placed small). The clip is recorded only when missing or stale ([internals](internals.md#videos)); `--relabel` never records |
| `door video-all --out DIR [--upload videos] [--relabel] [--dry-run]` | every archived design's video cut from its clip: clips missing or stale are recorded first, a video is cut again (seconds, no Minecraft) when its clip, card or label changed, uploaded to the `videos` release, then the archive catalog; `--relabel` records nothing and cuts every video again; `--dry-run` lists the work |
| `door tick-video <design> --out <f.mp4>` | one open/close cycle tick by tick: still camera, captions for the flips and the settled states, a 3x3 map of the doorway per tick and a strip of the whole phase |
| `door card <design> --out <png>` | draw just a video's title card (and its corner label) |
| `door gallery --videos DIR [--upload videos] [--gif-only]` | a looping GIF of the best design of every board cut from its clip (the same clips as the videos; a leader without one is recorded first, `--gif-only` records nothing), uploaded to the release (LEADERBOARD.md shows them), plus the night's timeline chart |
| `door montage --videos DIR --out <f.mp4> [--board solid] [--upload videos]` | a board's record progression by blocks placed (the history before sizes were volumes) as one video: every record filmed by the same camera, labeled with its blocks placed and how it got there (solid: `archive/records.tsv`) |
| `door world --out DIR` | one Minecraft world (a save folder for the real game) with every working archived design in rows by board, smallest volume first, its size in red concrete above it, to try them by hand |

## Generated files

Never edit these by hand; change their generator or its inputs and regenerate:

| file | written by | inputs |
|---|---|---|
| `designs/piston-door-3x3/LEADERBOARD.md` | `door leaderboard` | every design on main and `explore/*`, the verdict and shape caches, the `videos` release (GIFs, videos, montages), `archive/captions.tsv` |
| `designs/piston-door-3x3/archive/README.md` | `door catalog`, `door video-all` | the archive files, the verdict cache, `archive/records.tsv`, `archive/captions.tsv`, `tracks/buildable/scripts/`, the `videos` release |
| the `gh-pages` branch (REDSTONEPEDIA, https://vjeux.github.io/redstonepedia/) | `site publish` | every archive and its caches, captions, traces, `web/viewer/`, `web/site/` ([site.md](site.md)) |

`archive/records.tsv` (the solid record progression, one row per record) and
`archive/captions.tsv` (one line per design: how it works) are hand-edited data: add a row when
a design takes a record or tops a board.

## Design files

Contraptions are human-readable layers (`crates/mctas/src/design.rs`):

```text
# comment
name: tiny-demo
origin: 0 0 0                  # world position of row 0, column 0 (y adds to the layer y)
port lever: 2 1 0              # named positions (inputs, outputs, ...)
palette:
  S minecraft:stone
  P minecraft:sticky_piston[facing=up]
  L minecraft:lever[face=floor,facing=east]
layer 0:
  S.
layer 1:
  PL
```

Rows run north to south (z grows downward), columns west to east (x grows rightward). `.` and
spaces are air; every other character must be in the palette. A door design names the ports
`door` and `lever` ([rules.md](rules.md#the-door)); `@name` loads a built-in. Entity doors add
`entity: <type> x y z [snbt]` lines (category entity, [rules.md](rules.md#categories)); tile
modules add a `relay:` block. Block-entity data (container contents of item clocks) are
`nbt: x y z {snbt}` lines (world coordinates like `port:`), merged into the block entity right
after the raw paste exactly like `data merge block x y z {snbt}`, e.g.
`nbt: 0 1 0 {Items:[{Slot:0b,id:"minecraft:stone",count:1}]}`.

The search palettes (`synth`, `resynth`, `grow`, `regrow`) take shorthands: `leaves`, `log`, and
the exotic components (`trapdoor`, `ctrap`, `gate`, `skull`, `shelf`, `rail`, `arail`, `disp`,
`dropper`, `crafter`, `scaffold`, `grate`, `tinted`, `slab`, `stairs`, or `exotic` for all of them;
the exotic track's gadgets prove each). `regrow --require-any` keeps only candidates that use one
of the given blocks.
