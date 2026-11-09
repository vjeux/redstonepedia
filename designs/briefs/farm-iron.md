# Iron farms (design doc, gate G1)

ironfarm track, 2026-11-08. Gate G1 ([docs/process.md](../../docs/process.md)). Asked by vjeux on
2026-11-07: "I want you to build iron farms. Should be a mix of day 1 farm, good middle ground and
fastest scaled one", "as you go build the infrastructure to build and manage farms using redsim and
whatever else is needed. I will be asking you to build all the top ones in the future so lay out
the foundations for it", "Please implement everything you need on redsim. We can't use the game
code, it's way too slow in java". Every rule below was read in the decompiled 26.3 server
(`mctas src show <Class>`); no outside design was used. The work log is
`designs/circuits/mobfarm/tracks/ironfarm/NOTES.md`.

Revision 3 (after the G1 review and re-review by 8146b8db, 2026-11-08): the changelogs are at
the end. Revision 4 (vjeux 04:19-04:29, 2026-11-08): the three stages replace the tiers
("Stages"); the Decision of revisions 1-3 and the Not goals they contradict are superseded.
Revision 5: the stages' candidates, the absence, natural hostiles, lightning and lava's fire
(after the G1 review of revision 4). Revision 8 (vjeux 08:26): tradeoffs, not pass/fail; each
stage ranks by what its player values. Revision 9 (vjeux 10:48-10:56): each stage is a spec for
the farm optimizer ([farm-optimizer.md](farm-optimizer.md)): the player's hard limits, an
objective vector of 2-3 numbers with the player's reason for each, the forks, the merge rule; the
optimizer answers with a merged Pareto front per fork and its bounds, and this brief judges it
and revises the spec (Stages; the loop's record is the changelog).

## Use

A survival player builds an iron farm for iron ingots without mining (hoppers, rails, anvils,
iron blocks for beacons). They AFK near it, and every few minutes they empty a chest. The
farm needs villagers, beds and a zombie, which they bring in by hand.

## The number

Every farm reports these numbers; which one ranks it depends on its stage (below):

1. **Iron per hour per block**: iron ingots per hour at vanilla speed for the whole farm,
   divided by the blocks placed.
2. **Iron per hour**: golems per hour x 4 (the mean drop), the mean over every run (seeds x game
   days), with its 95% interval. For a tiled farm, also per module.
3. **The bill**: the iron the build itself costs, by recipe, the tools it needs counted (Stages):
   - a hopper 5, a piston 1, an iron trapdoor 4, a minecart 5;
   - rails 6 per 16, tripwire hooks 1 per 2;
   - one bucket, 3, for the lava and water.

   It lists the other survival materials too: villagers, beds, zombies, nether quartz, string.
4. **Payback**: the bill divided by iron per hour, in minutes.

The stages (revision 4) replace the three tier rules of revisions 1-3 (day 1: the lowest bill;
middle: iron per hour per block at 75% of the ceiling; fastest: iron per hour per module).

Reported too: golems per hour, poppies per hour, seconds per golem, the share of the ceiling,
the rate under the worst golem walking policy (see the judge), and the misses by cause:
- tries that found no spot;
- golem sightings;
- nights without sleep.

## Stages

Revision 8 (vjeux, 2026-11-08 08:26: "You can't expect day 1 farm to pass everything. It's all
about tradeoffs"). Hard limits only DEFINE a stage, by what its player has. One rule holds for
every farm: it works as its card says, verified in the real game. Everything else is a measured
tradeoff printed on the card as a number a player can weigh, never a refusal by itself, and each
stage ranks its candidates by what ITS player values. Every line is argued from the 26.3 server
(`mctas src show <Class>`; recipes and loot tables from its data). Revision 4 (04:29) set up the
three stages; revisions 4-7 made robustness pass/fail for every stage, which revision 8 replaces.

Revision 9 (vjeux, 10:50: "We want to find the absolute best farm given each stage of the game";
10:56: "I want some kind of pareto front where each are strictly better than other possibilities
in that space. I also don't want hundreds of tiny variations that don't meaningfully affect the
farm. You should use your judgement to present a small number to the user"). A stage's farms come
from the farm optimizer's search over a documented space, never from a handful of hand concepts.
Each stage below is the optimizer's input: who the player is (the hard limits), 2-3 objectives
with the player's reason for each, what is reported but not ranked, and the forks. The same lines
in the optimizer's format are `designs/farms/iron/<stage>.spec`. Revision 8's single scalars
(day 1's T, middle's iron per hour for an hour's build, late's iron per hour per block at N = 8)
become one objective or a printed ratio of the vectors below; the hand candidates become the
seeds the space must reproduce.

### The one rule (every stage)

- **It works as its card says, verified in the real game, and runs in the page's viewer.**
  Each build runs in the 26.3 server for game days (the game gate) and does what its card says
  (golems spawn, the drops the card counts reach the chest, no golem escapes, its villagers stay
  as the card says), and every tradeoff number the card prints that the gate can observe (rain,
  thunder, the first night) matches. The page's 3D viewer runs the judge's own run
  (`farm::Run`). That is every other study's standard. How the gate meets publishing (iteration
  14, vjeux 16:49: "Seriously, just upload them"):
  - **Before the gate:** a farm may go up before its gate has run (`site variation --no-gate`);
    the gate runs after it (`site variation --regate`) within the same fast-path cycle (by the
    next deploy).
  - **While the gate runs:** the page says nothing about the gate (our process stays off pages,
    vjeux 10:10).
  - **When it finishes:** a pass adds the page's real-game line. A failure stays on the page,
    printed, never held (the parent's ruling on vjeux 16:49), as a measured tradeoff with its
    counts ("a golem alive past 1,200 ticks in 1 of 8 game builds"), never as a pass (the code
    follows: `--regate` records the failure on the card instead of `held`).
  - **Wherever the game and redsim disagree, either way:** the card shows the game's measured
    numbers, and the gap is a model bug in redsim to attribute before redsim ranks that family
    again (a card that under-claims is not "as its card says" either; farmopt's day-1 #2: its
    AI-off card about 78 golems an hour, the game 88, which also misranks it against #1 on the
    front).
  `circuit archive` keeps refusing a mob farm without a gate pass: the archive (the record a
  stage's pick stands on) needs the pass; a listed variation does not.
- **Caveats are not tradeoffs.** A caveat admits a gap in our tools ("not simulated", "only on our
  simulation"): none may appear; the gap is closed by work (vjeux, 04:19). A tradeoff is a
  measured, game-verified fact about the design ("hide the zombie by hand the first night",
  "about 3 iron lost per thunderstorm"): every one is required on the card.

### Tradeoffs every card measures

Numbers, never refusals by themselves; each stage weighs them in its own rank:

- **Build effort**: blocks placed and blocks dug in the terrain; iron, counting the tools the
  build needs (a bucket 3, a hopper 5, tripwire hooks 1 per 2, shears 2 for leaves, an iron
  pickaxe 3 for redstone dust: `redstone_ore` is in `needs_iron_tool`); lava sources (each one
  bucket trip: `LavaFluid.canConvertToSource` is the `lava_source_conversion` game rule, off by
  default); villagers moved (from their village bed to the farm).
- **Rate**: iron per hour with its 95% interval (redsim's runs, the gate's builds).
- **The start and a missed night**: whether it starts by itself the first night after it is
  finished or needs the zombie hidden by hand once, and whether it restarts by itself after a
  missed night (`Villager.golemSpawnConditionsMet`: no `LAST_SLEPT` in the last 24000 gt, no
  golem; a night is missed in rain at dusk, in a thunderstorm or while the player is away).
- **Days away**: the time to the first golem after the player comes back, for both kinds of
  absence (unloaded: chunks saved and loaded again, `Villager.readAdditionalSaveData` ->
  `refreshBrain`, memories without a codec dropped, fresh sensor delays; loaded but not ticking)
  and a return by day and at night; "no" when it never restarts.
- **Weather**: the rate in a day of rain and in a day of thunder
  (`ServerLevel.advanceWeatherCycle`: rain on about 15.8% of the time, thunder about 9.1%; rain
  and thunder darken the sky by 3 and 5, which daylight detectors read; rain reaches only cells
  under the open sky, `Level.precipitationAt`).
- **Lightning** (only within 128 blocks of a player, while it thunders: each chunk rolls
  `random.nextInt(100000) == 0` a tick, `ServerLevel.tickThunder`): the iron lost per
  thunderstorm, and the chance per storm-hour of each break (a villager made a witch,
  `Villager.thunderHit`; the zombie's boat or minecart destroyed; a fire lit, `spawnFire(4)` on
  Normal and Hard), turned into the expected in-game days before it breaks at the stage's
  weather. The bolt goes to the closest copper lightning rod within 128 that tops its column
  (`findLightningRod`), else to a living entity that sees the sky near the strike column
  (`findLightningTargetAround`), else to the column top; it hits every entity in x ± 3,
  y - 3..+9, z ± 3 through any roof (`LightningBolt`). The infra track's model enumerates every
  bolt the farm can draw.
- **Lava's fire** (`LavaFluid.randomTick` with `doFireTick`, only within 128 of a player): the
  expected time before a lava source burns a part of the farm (a flammable block within its
  reach: 2 steps up, each ± 1 sideways; `FireBlock`'s table), "never" when nothing is in reach.
- **Hostiles, on Hard** (zombies break doors only on Hard, `Zombie.DOOR_BREAKING_PREDICATE`; a
  villager killed by a zombie always becomes a zombie villager on Hard, `Zombie.killedEntity`):
  the expected time before a villager is lost to a natural hostile (the terrain track's flood,
  `entity/mob/hostile.rs`, and natural spawning in the game gate); and whether the village's golem
  can reach the zombie or its villagers can claim the farm's beds (`AcquirePoi`: only along a
  path that reaches the bed).
- **Burnt drops** (iteration 11): the iron lost per golem to the lava (a golem dying where its
  drops fall in it), measured on redsim with the mob AI on and in the game; the card's iron per
  hour already nets it out. A kill that loses none (a blade below a drop) competes on the
  objectives, not by refusal.
- **Beds** (iteration 14, the mob-AI lane, 0 differences on the bridge): a pod villager releases
  its bed at nearly every REST (a failed daytime IDLE stroll sets CANT_REACH_WALK_TARGET_SINCE,
  nothing in a 1-cell pod clears it, and SetWalkTargetFromBlockMemory drops HOME past 1200 gt)
  and re-claims it through AcquirePoi; when the re-claim fails it sleeps no more and the cluster
  stops. The card prints the nights a villager spent homeless and the rate's spread across seeds
  (the lowest seed's rate beside the mean), never only a mean (four modules packed tight: 78 to
  423 golems an hour over 6 seeds; m2-open: 110 on every seed). A cage must make the re-claim
  certain (its own bed the only free bed AcquirePoi can reach from the pod: within 48 blocks
  along a path), and the grammar checks it for every multi-module layout.
- **Storage, AFK and lag**: hours of output the chests and hoppers hold at the card's rate,
  poppies included (they take slots ahead of the iron); the hours it runs unattended; golem life,
  hoppers per 1000 iron per hour, entities alive at once, peak loose items.
- **Size**: the bounding box, its chunk columns, density and scaling.

### Common to every stage (revision 9)

- **Build minutes**: placing every block, digging, and GETTING every item, at the stage's prices
  (iteration 3, vjeux 12:01 on d1-bed-plate's 98 glass: "this amount of glass is def not
  accessible day 1"): `designs/farms/prices.tsv`, seconds per item per stage, each argued in its
  row from the item's FULL chain at that stage, tools included (iteration 5, vjeux 13:09: "to
  get leaves you need sheers for which you need iron. It uses stone but you need silk touch or
  smelt which you're not going to do day 1"). Placing 1 s a block; digging 1 s on day 1 (stone
  tools), 0.5 middle, 0.3 late; every dug block is also one free filler block (stone dug without
  silk touch gives cobblestone, so filler is cobblestone or dirt). Stone itself is smelted
  cobblestone (6.5 s on day 1, no silk touch): a design names the block it places, so a day-1
  wall is cobblestone, not stone. Leaves are a trip to the trees with shears (2 iron, once:
  broken without shears they drop saplings). Gathering and crafting by recipe: on day 1 filler
  1 s, a log 2 s (planks 0.5), sand 1.5 s, coal 4 s, an iron ingot 30 s (caving for ore with a
  stone pickaxe, smelted), glass 7 s (sand, fuel, and half of 10 s in a furnace: a day-1 player
  runs 2), so a hopper is 154 s and the tools the build needs count once (bucket 90 s, shears
  60 s, the iron pickaxe 90 s in the redstone fork: vjeux 12:29, iron counted with the tools); a middle player has iron tools, 8
  furnaces, the Nether and a day-1 farm's iron (filler 0.5 s, glass 2 s, iron 10 s, quartz 5 s);
  a late player has farms and super smelters for everything (filler 0.2 s, glass 0.4 s, iron
  2 s). Every price is per realistic action, not per block (iteration 6, vjeux 13:56 on the
  middle bar's 38 minutes: "What takes 38min to build?"): lava is a trip (120 / 60 / 30 s) plus
  a fill per source (5 / 3 / 2 s), a trip carrying the buckets the player has (day 1 one, each
  extra bucket 3 iron, the cheaper of an extra bucket and an extra trip taken; a middle or late
  player keeps a stack of 16); villagers go by the cheapest route the stage has: moved from a
  village bed by boat on day 1 (180 s), by minecart or boat on the player's own paths in the
  middle (120 s) or bred at the farm (the first pair moved, then 30 s of actions per villager:
  bread, a bed, sealing its pod; the code's game time, counted only where it outlasts the rest of
  the build: a pair breeds only by day, calm and out of the zombie's sight (breeding runs only in
  IDLE, day time 10 to 12000 for a villager with no job site, never while it panics), a birth
  275-324 gt after the pair meets, then 6000 gt before the pair breeds again, so at most 2 births
  per pair per game day; a free bed the parent can path to stands before each birth (without it
  the food is eaten and no baby comes); the baby grows up in 24000 gt and walks to its bed at its
  first REST, and only then is its pod sealed), and delivered by a breeder's rail or water line
  late (30 s); the zombie
  180 / 120 / 60 s (lured at night into its boat). An item the table does not price fails the
  line (no silent default). Blocks alone would count a villager, a lava trip, an iron ingot and a
  smelted glass block as free; these prices make a glass-capped or hopper-heavy day-1 farm lose
  on its own merits, and the same build cheaper for a player with farms.
- **Vertical travel and height** (iteration 16; vjeux 17:59: "Building horizontally is much
  easier than vertically"; 18:00: "Even digging down a lot is annoying because you have to go
  down and up way farther"). Depth and height are priced in both directions, per trip, at the
  game's own movement speeds (`designs/farms/prices.tsv`, the `travel:*`, `place:edge` and
  `villager:*` rows; the access's blocks by their own rows):
  - **Trips.** Every trip between the surface (where materials, the village and the lava come
    from) and a work level pays its vertical travel down and up: one per full inventory of
    placed blocks (27 stacks, 1,728 blocks), each lava trip, each villager moved into its pod
    (the villager travels too: down by a drop into water at the access shaft's foot, up by a
    bubble column where the stage has one, else a rail ramp), the zombie, and each hand step.
    Collecting is a trip too: 2 x the output chest's depth below the AFK or standing level, once
    per collection; day 1's T includes the trip down for its 24 iron, and middle and late count
    one per collection (sized by the card's storage hours) on the card.
  - **The access, built in the design** (the G1 review): the way between the surface and the
    work levels is a generated part of the design, placed and priced by its blocks like every
    other part (no separate per-layer charge), and judged by every check: the golem caps and the
    escape flood from every villager's search (a stair's top face is not full, so a stair wastes
    a spawn attempt; a bare landing inside a villager's search spawns a golem in the shaft, since
    SpawnUtil ignores block collisions; a ladder cell is not air and stops the scan basing below
    it; a soul-sand column within a golem's reach is an elevator to the surface), the hostile
    flood (an open access is a hostile path down: a closed trapdoor or fence gate at its top
    keeps hostiles out, zombies break only doors), the sky cap and lightning rules (an open
    shaft brings sky light 15 down) and fire clearance. Its top hatch is the one block the
    surface rule below allows on the natural surface. Up and down may go by different means.
    Travel per layer per trip, from the code: stairs dug through rock, 0.25 s each way on foot
    (walking 4.317 blocks/s, one layer a block); a ladder, up 0.43 s (LivingEntity's climb of
    0.1176 blocks/tick: the 0.2 boost after gravity 0.08 and drag 0.98), down 0.33 s (the climb
    clamp, 0.15 blocks/tick); building up, a pillar, up 0.5 s (a 10 gt jump-and-place cycle),
    down 0.45 s (the dirt under the player broken with a shovel, 4 gt, and a one-block fall, 5
    gt); scaffolding late, climbed like a ladder; a soul-sand bubble column in the middle and
    late, up 0.105 s for a player holding jump (about 0.48 blocks/tick: the column's +0.06 and
    jumpInLiquid's +0.04 against travelInWater's x 0.8 and -0.005) and 0.18 s for a riding
    villager (0.275 blocks/tick); a drop into water at the shaft's foot, down for players and
    villagers alike, about 0.06 s a layer over a long fall (gravity 0.08, drag 0.98), faster
    than a magma column (0.15 s holding sneak: the -0.3 cap) or stairs; elytra and rockets late
    (3 s a trip plus 0.05 s a layer). Digging itself stays priced per block.
  - **Placing at height.** A block more than 2 above every standable surface the player already
    has (the ground, or a block of the design with two air cells above it, within 3 horizontally)
    costs the sneak-edge placement on top of its place: +1 s on day 1 and in the middle (sneaking
    to an edge at 0.3 of walking speed and placing against a side face), +0.4 s late (one
    scaffolding cell to stand on).
  - **Where the surface is.** A terrain design measures from the natural surface at its spot; a
    void design (middle, late) stands on the surface at its lowest layer.
- **Below the surface, the chute to the side** (iteration 16, every terrain farm, and 19; vjeux 18:02:
  "Don't put slabs, do it below the surface. And put the chute to the side"): no block is placed
  on the natural surface (a cover there is refused; the access's top hatch is the one
  exception; caps and seals inside caves or the rock are below the surface and allowed; the
  natural surface of a column is the untouched terrain's top, its heightmap, so a ravine or cave
  mouth open to the sky is surface and a cap on its floor is refused), so the
  farm's depth must keep every villager's golem search (its cell and cell+1,
  from y+6 down, across the +/-8 reach) below the surface. The chute to the side is a family every
  search must include and every stage lists, not a refusal (iteration 19, the parent's decision,
  which vjeux may overrule): the kill chamber and the output beside the farm, beyond R + 1 = 18.84
  blocks horizontally from every villager, fed by a water channel cut through the rock (priced as
  its digging plus its water sources). A golem inside any villager's follow cube resets it whether
  seen or not, and a horizontal channel carries golems at 0.036-0.076 blocks/tick (250-520 gt
  across 18.84 blocks), so the search may drop them out of the cubes first (a shaft beside the pad
  down to villager y - 20.54, then sideways); the judge measures the dwell either way. A drop
  under the pad competes on the objectives, which now price its depth in every trip (vjeux's
  18:00 point, which the side chute served): on equal terms (the access built in, the AI on,
  travel priced) the day-1 lane's d1u-drop-side as currently listed (D1U's pods with the access built in, the golems dropped below the follow cubes and carried sideways through the rock to its kill; not run 5's searched chute parts) reached its armour in 48.4-49.1 minutes against d1u-deep's 38.7
  for 2.5% more iron, and its chest was farther to walk (40 s a collection against 31 s).
- **Material roles** (iteration 5): a design says what each block is FOR, and the generators and
  the optimizer place the cheapest block that does that job at the stage's prices
  (`designs/farms/roles.tsv`: structural, fluid holder, sky cap, golem cap with the scan going on
  to the pad, golem cap elsewhere, hostile spawn-proofing, sight blocker, see-through, walk
  blocker, floor; each with what the block must do, from the 26.3 code, and its candidates). The
  design file names the exact block placed, and the judge checks its behaviour on the assembled
  build (collision, spawn rules, sky light, the zombie's ray, fire). A cell with several roles
  takes a candidate that passes each: any block inside a summoner's scan columns (17 x 17, the
  summoner's y - 7 to y + 5) whose top is open also has a golem role, golem_cap_scan_through above
  the pad or golem_cap elsewhere, whatever else it does (a slab roof over the pad within y + 5
  would stop every attempt there; revision 3's roof rule: leaves, or y + 6 or higher, or capped).
  Day 1 thus builds with
  cobblestone and dirt and caps stray golem cells with a cobblestone bottom slab (3 cobblestone
  make 6; its top face is not full, so neither a golem nor a hostile spawns on it, and it ends the
  attempt there) or a button on top where the scan must go on to the pad; leaves and glass only
  where nothing cheaper does the job.
- **Net iron per hour**: the iron the player actually collects, per hour, over the stage's
  unattended window W, from the running farm's steady state with the weather cycle, lightning and
  natural spawning on Hard. A break (a villager killed, or turned into a witch or a zombie
  villager; the zombie or its vehicle lost; a part burnt) stops the farm for the rest of W; iron
  the storage cannot hold is lost. With a break hazard λ per hour (the infra track's lightning
  model per storm-hour times the storm share, the terrain track's exposures, lava's fire), the
  farm runs a share (1 - e^(-λW)) / (λW) of the window. W: **day 1, 0** (the player is there and
  repairs at once: only the iron destroyed is subtracted, and each break is printed); **middle,
  one game day** (20 minutes: the player looks in once a day); **late, 8 h** (AFK overnight).
- **Timing parts are lookups** (vjeux 10:48): every clock, pulse, extender or delay is the
  smallest archived design of its kind in `designs/circuits/catalog.tsv` or the clock catalog that
  meets the need (the hide: shut at least 20 gt, so every phase of the hostile sensor, rate 20,
  sees it shut once), never hand-built. A need the catalog cannot meet within the stage's
  materials goes to the circuit study as a request, and the fork that needs it waits for it.
- **One farm = 10%** (the merge rule): two builds whose objectives are all within 10% of each
  other are the same farm. The optimizer merges greedily: the best build on the first objective
  (ties to fewer blocks) takes every build within 10% of it on every objective as its runner-ups,
  then the best of the rest, and so on; so no two members are within 10% on every objective, and
  no twins sit on either side of a fixed box's edge. 10%, not less: a player
  does not pick a different farm for 3 minutes in 30 or 40 iron in 440 an hour; the 12 spots and
  the rotations already spread a build's numbers by several percent, and redsim reads 2% low
  against the game (L2d, 8 game runs); the judge's own interval (about 1% each side) is far inside
  it. Not more: the redstone fork of day 1 is worth about 15% (79% of the ceiling for M3's bed
  plate, 92% for M2's brief hide) and must not merge away.
- **Bounds**: every front comes with each objective's bound (the ideal point), the front of the
  bounds of everything not measured, and the gap per objective (farm-optimizer.md, Bounds). A gap
  above the merge rule's 10% is printed with its cause (a loose bound or a missing part of the
  space) and worked on in the next iteration; it does not hold the presented set. A stage's pick
  replaces the live page when it passes the game gate and beats the live farm on the stage's
  objectives (beats: better by more than 10% on at least one objective and not worse by more than
  10% on any, the merge rule's threshold, so pages don't churn on noise; a farm that wins one
  objective but loses another by more than 10% joins the stage's presented set instead of
  replacing the live one); the search keeps working any open gap, and the page is replaced again
  when it finds better. A gap is never printed on a page; nothing waits on vjeux (10:59: "I want
  your process to be autonomous, don't wait for any of my decisions"; a page never claims "the
  best", so the best farm found so far makes no false claim, and the site only ever improves).
- **Parts** (vjeux 11:23: "breakdown the various part of the farms to optimize them individually.
  The villager "cage", the golem spawn area, the kill chamber, the loot collection system... like
  we did for the clocks"). The optimizer searches each part on its own (the cage, the spawn area
  with its exit, the kill chamber, the collection, the calm trigger), keeps per part and stage the
  non-dominated ones on what that part changes (its build seconds; the rate it allows: the cage's
  sight lines and spawn centres, the spawn area's mark share and exit time, the kill's time and
  fire safety, the collection's hours and hoppers, the trigger's hide length and restart), then
  assembles builds from them. Per-stage part limits are written only where the player needs them
  (each stage's Parts line below); the rest is the stage's objectives.
- **The cage's objectives** (iteration 17; vjeux 18:26: "Also please optimize the villager room,
  the current layout is super weird"). The cage (the villagers' pods and beds, the zombie's window
  and boat, the calm device) is searched on its own vector, under the four rules that make it
  work: the watchers see the zombie by day and it is hidden at night (the stage's calm: the bed
  plate on day 1); each villager's only reachable free bed is its own (Beds); every villager's
  golem search reaches only the pad (the every-villager rule); it restarts after days away.
  Objectives, each minimized, merged at 10% as everywhere: (1) the irregularities a builder
  notices, counted: each pod whose shape differs from the first pod's, villagers outside one row
  or one 2 x 2 block, each villager level beyond the first (all villagers on one level is the
  target), the zombie's window not in front of the watchers, the plate villager's pod outside the
  watchers' row, its plate not beside the window's trapdoor; (2) the cage's bounding-box volume;
  (3) its blocks and build minutes, with the iteration-16 travel and height costs (the
  irregularity count merges only when equal: 0 against 1 is never within 10%). The calm rule's
  sight check keeps one row on one level honest: every watcher within the zombie's 8 blocks, feet
  to feet (VillagerHostilesSensor), the plate villager blind to the zombie (it would panic and
  never lie down). The farm is measured where a cage is assembled into it: a more regular cage
  replaces the farm's cage when the farm stays within 10% on every objective of its stage (the
  merge rule: the same farm), and the new farm's card comes from its own judge and game gate.
  Inside that set, the best cage per stage (fewest irregularities, then build minutes, then
  volume) replaces the cage in d1u-deep, the Farm module and the + and X candidates (re-judged
  with the mob AI on), and goes on the cage component page.
- **Every part's objectives and hard rules** (iteration 20; vjeux 20:59: "All of them look super
  unoptimized. Can you make sure to optimize every single part of the system during the
  night."). Iteration 17's cage vector generalizes to every part at every stage: the cage, the
  calm (sleep trigger), the spawn pad and its caps, the golem transport (drop, chute, channel),
  the kill, the collection, the storage, the access and the containment (walls, caps, seals).
  - **Objectives of every part**, each minimized and merged at 10% (counts only when equal):
    blocks placed; blocks dug; build minutes with iteration 16's travel and height; compactness
    (the part's bounding-box volume and its levels); regularity (the irregularities a builder
    notices: cells off the part's grid or mirror axis, mirrored halves that differ, pods or
    units of differing shapes, and the number of distinct block types); and the part's measured
    effect inside a reference farm (the stage's pick with that part swapped in, judged with the
    AI on): the golems lost to dwell in a villager's follow cube, kill time (mean and longest),
    iron burnt, spawn attempts that find no spot, iron lost before the chest.
  - **Hard rules, per part** (each already in this brief; a part breaking one is refused): the
    cage, iteration 17's four; the calm, the watchers see the zombie by day and it is hidden at
    night long enough that every scared villager sleeps, every night, restarting after days
    away, and no hide opens a walkable path out of a pod (golems by night are a measured line,
    the share of golems by night, which the rate already ranks: L1, the obvious design, sleeps
    all night); the pad and its caps, every villager's
    golem search reaches only pad cells (cell and cell+1, from y+6 down, across +/-8), a golem
    cap stops the scan and caps above the pad let it through, no hostile spawn on the pad; the
    transport, no escape under any input, no dead water under the still policy, lips counted,
    no golem within melee reach of the zombie or its vehicle; the kill, no golem outlives
    1,200 gt in any run, fire clearance, no golem within melee reach of the zombie, no drop
    within the zombie's pickup reach; the collection, every drop not burnt reaches a container,
    no drop within the zombie's pickup reach; the access, iteration 16's (built in, judged by
    every check, its hatch the one surface block); the containment, the confinement, escape and
    hostile floods, the sky cap, fire clearance, no golem within melee reach of the zombie.
  - **Bounds and gaps**, printed beside each part's front, each labelled with the family it
    holds for, beside the part's bound over the stage's materials (the G1 review): the kill, a
    lava kill takes 240 gt (100 health at lava's 4 a hit; LivingEntity.hurtServer lands a full
    hit once the cooldown has fallen to 10, so 25 hits at t = 0, 10, ..., 240), and over the
    materials: day 1 lava is the bound; middle, fire charges from a dispenser (5 a hit plus
    fire) about 190 gt, though they light blocks and drops can burn; late, splash Harming II
    (12 a hit) about 80 gt and falling anvils (falling_anvil is not in #is_fall, so a golem's
    fall immunity does not stop it) about 20 gt, where late's "golems die fast" lag line points;
    the transport, a gravity drop leaves every follow cube in about 20 gt (a free fall to the
    lowest villager's y - 20.54), and in the middle and late stages a slime block's push
    (PistonMovingBlockEntity sets the motion on its axis to 1 block a tick) clears the 14.5
    blocks from the lowest spawn cell in about 11 gt; the pad catches at most every attempt (a
    mark share of 1); one hopper moves an item every 8 gt (9,000 items an hour), which carries
    day 1 (under 1,000 iron and 300 poppies an hour), the middle (900-1,800 iron an hour at 2-4
    modules) and late up to about 16 modules (about 7,000 iron and 1,800 poppies an hour), with a
    second line beyond; the containment needs at least the blocks of its floods' minimal cut;
    storage needs 8 hours of output for late's objective.
  - **Search and assembly.** farmopt searches each part's front per stage exhaustively where
    the space allows, kept per interface (the cage fixes the spawn centres and so the pad; the
    pad and the cage fix the dwell and so the transport), so a member dominated in one reference
    farm but fitting another assembly is kept; then it assembles each stage's farms from the part
    fronts (day 1 per redstone value, middle, late); the stage objectives decide between
    assemblies.
  - **No redundant block** in a listed farm or component member: each passes the trim
    (`mctas mobfarm trim`: remove every block no check needs, swap each block for the cheapest
    of its role, confirm with the AI-on judge), and takes the regular parts wherever the farm
    stays within 10% on every objective of its stage.
  - **Storage, a proper system** (vjeux 21:06: "You also need a proper storage system. I don't
    know what this chest + hopper chain is supposed to be but that's not it"; the parent's
    form): the kill's hopper line feeds an item filter (the cheapest legal comparator sorter at
    the stage) that separates iron from poppies; the poppies go to a disposal (lava or cactus,
    priced) or, as an option, their own chest; the iron goes along a hopper line over a
    HORIZONTAL row of double chests (each chest's hopper pulls from the line above it and the
    line carries the rest on), at the player's own level beside the access or the AFK spot,
    never chests hanging in a vertical chain under the farm; in the middle and late stages a
    crafter packs the ingots 9 to 1 into iron blocks first, when it pays. Hard rules: every
    item the kill drops reaches storage or the poppies' disposal, iron is never voided, nothing
    backs up into the kill or the pad, and it restarts after days away. Objectives: capacity per
    block (items held per block of storage), build minutes, the collection's seconds per trip
    (iteration 16), and regularity (one straight row on one level). Capacity against the
    stage's AFK hours (day 1 a kit, middle a session, late 8 hours) is a measured tradeoff line
    on the card, not a gate.
- **What reaches vjeux**: per stage, the stage's pick first (Day 1: one pick per value of its
  redstone fork, each card saying plainly whether it needs redstone and how much, e.g. "needs 12
  redstone dust"; iteration 19), then every promising candidate as a
  variation of its stage (iteration 14, vjeux 16:27: "I asked you to stop using previews and
  push to the site directly"; this supersedes iteration 10's listing rule), each with a one-line
  reason a player would pick it and its tradeoffs in plain words, published to the site's Farms
  section (`farms/iron/`, stages and components) with `mctas site variation <design> --stage
  <stage> [--pick]` (it runs the kind's judge and the real-game gate itself and refuses a farm the
  gate fails) and the coordinator's fast path (GO, main, deploy); every candidate discussed with
  vjeux carries its live site URL. Previews are retired. Every candidate that passes the gate is
  listed, dominated ones included (vjeux 16:49: "Seriously, just upload them"); a candidate whose
  gate is still running or has failed may go up at once (`site variation --no-gate`), its page
  stating the gate's result as a measured fact (a failure as a tradeoff line with its counts:
  "a golem stuck in the channel, never killed, in 1 of 16 game builds"), never a pass it did not
  get; near-duplicates
  within 10% on every objective stay one farm (vjeux 10:56, "I also don't want hundreds of tiny
  variations"). Only a bar (copies at the no-coupling pitch, each with its own kill chamber and
  chests) stays off, quoted as the reference line on every listed farm's card and beside the
  fronts (vjeux 14:51 on the stack of 8 copies: "Sounds like a stupid design, why does each module
  have their own kill chamber?"). The stage's pick is the farm that beats the stage's bar: day 1, the live farm (the
  archived iron-day1), replaced only by a game-gated farm that beats it; middle and late, copies
  of the best single module at the no-coupling pitch (this overrides process.md G3's fallback for
  copies, by vjeux's word, 04:26: copies "gain nothing: that is the bar, not a candidate"; 14:51).
  The best single module is listed as a component, the unit the middle and late farms are built
  from, not as a stage's farm.
- **A farm and its N** (iteration 4, the G1 review): a farm is a stacked unit (one cluster, or
  several sharing parts) and the parts its copies share; builds that differ only in how many
  copies of one unit they hold are ONE farm, never a ladder of front members. Each farm is
  measured over N (the curve at 1, 2, 4, 8 and 16 modules, as far as its unit allows) and compared
  at its own N: the smallest N whose iron per hour per build minute is within the merge rule's 10%
  of its best on the curve (past that point more modules only repeat the same numbers), and for
  the middle the smallest such N of at least 2 (its "more than one module" limit, so a unit that
  gains nothing from stacking is still compared at two). Middle and
  late compare farms by their per-module numbers there; day 1, whose T is a whole build, compares
  them at one unit. The card prints the curve, and the player picks the farm from the front and
  its N from the curve. The copies bar is measured at the candidate's own N, priced the same way
  (the same cheapest villager route: breeding's cost per module falls with N for copies too), so
  no farm beats copies by the amortization of its villagers alone. Two clusters are two only when
  their villagers are more than 10.6 apart on x or z or 11.95 on y (`Villager.spawnGolemIfNeeded`
  lists the villagers in its box inflated by 10, with no sight test): pods sharing a wall are one
  cluster.
- **The card adapts to the stage and the farm** (vjeux 11:17: "Different farms have different
  tradeoffs. We don't care about lava trips for the high end farm. Please have the ability to
  adapt what is shown based on the objective"). Everything stays measured; what the card SHOWS is
  chosen here. Each stage has default rows (the facts its player weighs, in order: Day 1, Middle
  and Late below); each presented farm gets its own rows, 3-6 facts plus how it differs from its
  neighbours on the front. The rows use the page-copy lane's format (`key`, `slot`,
  `line`: a design's rows win over its kind's, the kind's over the built-in list) in
  `designs/circuits/mobfarm/cards.tsv`; a kind's rows name only fields every design of the kind
  has (the archived ones keep the metrics of their verdict), so a fact a newer judge measures
  joins the kind's rows once the archive carries it; every number is a placeholder bound to a
  judge field (`{iron_per_hour|round}`), and the build refuses an unknown field or a digit
  outside a placeholder, so a card cannot drift from the measurement.

### Day 1

The player has wood and stone tools, a furnace, a bucket and a few iron from a cave, and a
village nearby; no Nether.

Hard limits (what the player has):
- **Day-1 items only**: wood, stone, cobblestone, dirt, sand and glass, copper, coal, iron, and
  what the village gives (its villagers, their beds). No Nether item (no quartz, so no
  comparator, observer or daylight detector), no slime. Iron and lava are not capped: they count
  in T.
- **At most the village's villagers**: 8 (village-edge's village, asleep in their 8 beds), so one
  or two clusters.
- **Real terrain** (vjeux 04:22): built at village-edge (`designs/terrain/village-edge.mcterrain`),
  priced at the cheapest of the 12 nearest valid spots (the day-1 lane's pricing: blocks placed,
  blocks dug, and a cap on every natural cell where a golem could spawn outside the farm: the
  cheapest at day-1 prices, a torch or a button on top (a block above that is not air stops the
  golem's scan there, `SpawnUtil` LEGACY_IRON_GOLEM) where it works, glass only where it must be).

Objectives:

| objective | the player's reason |
|---|---|
| **min T, the minutes to a full iron kit**, measured (iterations 2-3): from the first block until the farm's chest holds 24 iron (the armour) and the player has walked down to it and back (iteration 16, vjeux 18:00: 2 x the chest's depth below the standing level at the access's speed), for a player who starts at dawn (day time 0) of day 1 with nothing but stone tools, mines the iron the build needs (in the build minutes at 30 s an ingot, tools included), and builds for the build minutes (vertical travel included) at the cheapest spot (the farm stands complete at day time 1200 x build minutes, mod 24000); the villagers come from the village at that moment, each with the `LAST_SLEPT` of its last night in its village bed (a saved memory: moving a villager keeps it), and the zombie in its boat, caught on an earlier night and parked by the site; the judge's terrain run from there, with the weather cycle and natural spawning on Hard, the mean over its seeds | "Iron armour as soon as possible": what a fresh player builds it for |
| **max net iron per hour** once it runs (W = 0: the rate less the iron destroyed) | it keeps running for days after the kit: tools, a shield, buckets, hoppers, rails, an anvil |

Why these two. T alone (revision 8) values the rate only through the first 24 iron, so it would
pick the cheapest build even where 5 more minutes double the rate; the rate alone forgets that a
day-1 player has an hour, not a weekend. Together they cover every day-1 player, from "a kit now"
to "a farm for the week", and T's best is one end of the front. T is measured, not computed from
the steady rate (iteration 2): a farm earns nothing until its villagers are willing and its calm
has worked, and when the build finishes in the day matters (a farm whose scared villagers can't
wake at night makes its first golems only the next morning); the steady rate, objective 2, is
what it makes after that. The T run performs every hand step the card prints (the first-night
hide: the zombie hidden at the first REST start for at least 20 gt), since the player is there.
Not objectives: the build effort alone (it is most of T); the iron up front (in T: the build
minutes mine it at 30 s an ingot, so the chest owes only the 24 of the kit; and printed, since a
fresh player must get it before building); the villagers taken (in T at 180 s each, capped by the
village); the exposure to loss (the player is right there to fix it: printed as the days before
each break).

**Fork: redstone (none | dust).** Has the player dived for redstone ore yet? Without it, the hide
is driven by what the village gives (the bed plate: a blind villager's wooden pressure plate).
With it (the iron pickaxe counted, 3 iron; repeaters, torches and dust, still no quartz or slime)
the hide can shut only briefly at dusk, from the smallest day-1 catalog falling-edge pulse of 20
gt or more, so every scared villager sleeps and wakes into panic, as M2 does with its observer:
79% to 92% of the ceiling on a 4-villager drop module. A fork, not an objective: the player cannot
buy redstone with minutes on the spot; they have it or they don't. The catalog has no day-1
falling-edge pulse of 20 gt or more yet (every archived one uses a sticky piston, a sculk sensor
or a comparator): requested from the circuit study; until it exists the dust front is the none
front.

Not forks (the front itself trades them): one or two clusters (T against the rate); the kill (the
pit's corner or a blade below a drop: lava trips are in T); dug in or built up (the spot pricing
takes the cheaper, and the build minutes on the flat plains sample are printed for a village
without rock).

Parts: the calm trigger is the bed plate, or (dust) the bed plate with the smallest day-1 catalog
pulse; the kill chambers rank by lava sources first (2 minutes a trip, the costliest thing a
day-1 part can need), the collections by hoppers (5 iron each: one holds hours at a day-1 rate);
the cage's villagers and beds are the village's.

Shown on the card, in this order (the stage's default rows, `designs/circuits/mobfarm/cards.tsv`):
the minutes to the armour and the iron per hour after it;
the iron to have in hand, the lava trips and the villagers taken from the village (a fresh
player's scarce things); the first night (starts by itself, or hide the zombie by hand once);
after a trip away; what it makes for a player who sleeps through every night (the share of golems
by day) and the build on flat ground, for a village without rock (both join the kind's rows once
the archived day-1 farm's verdict and the terrain card carry them; a presented farm shows them in
its own rows). Measured and kept under all
measurements: the build's split, the rain and thunder day rates, the exposures (expected days
before natural mobs, lightning or fire break it), storage hours, the size.

Seeds (the hand candidates the space must reproduce; T by revision 8's formula, blocks in the
void until the terrain pricing, so blocks dug 0; M3 of the Middle table is day-1 legal too: the
bed plate, 4 villagers, a drop and a blade, 13 iron, no quartz, 378 iron/h):

| # | the calm moment | the kill | iron | lava | estimate | blocks (void) |
|---|---|---|---|---|---|---|
| **L3** (archived iron-day1) | a tripwire through every spawn floor: a burning golem powers a wooden trapdoor that hides the zombie; the zombie hidden by hand the first night, and by hand again after a missed night or days away (no golem, no trigger) | in place: each spawn floor has a lava source beside the golem's head | 11 (bucket 3, hopper 5, hooks 1, shears 2 for the leaf roofs) | 16 | **redsim, the whole judge: 295 iron/h (292-298), 62%; the gate: PASS (74.8 golems/h vs 75.0), works in rain and thunder** | 189 |
| **D1** | **the bed plate**: of the 3 villagers only S sees the zombie; U1 stands by day on a wooden pressure plate by its bed, which powers the stone under it and the top-half oak trapdoor in S's line of sight, so the trapdoor stands open. At REST U1 lies down, the plate lets go, the shut trapdoor's slab crosses S's line (1.78-1.84 up in its cell), S calms and sleeps. S's sleeping eye (bed + 0.8875) sees under the slab, so it wakes scared and spawns golems at night too | L2d's dry-hole pad (6 x 6) over a 4 x 4 pit 5 deep; one water source drives every golem into the pit's corner under one lava source held by 3 wall signs; 1 hopper | 8 | 1 | **redsim, the whole judge: 243 iron/h (241-245, 64 runs), 61 golems/h, 51% of the ceiling, restarts by itself after a missed night, robust over 8 builds**; 59 of 100 golems by day. The same runs with the weather cycle, a forced day of rain and a forced day of thunder (the infra track's model) give the same golems: the calm reads no sky | 318 |
| D2 | the bed plate | no pit: the pad's water carries golems into a corner 2 x 2 under one lava source, burning in the villagers' sight | 8 | 1 | about 150-180 iron/h: every villager sees each golem burn (about 300 gt), so each cycle stretches to about 850-900 gt; the prototype's corner flush left golems on the pad, unsolved | about 200 |
| D3 | the egg plate: a chicken lays every 6000-12000 gt (`Chicken.eggTime`), so at least one egg falls in every REST window; it floats over a wooden plate that shuts the hide 20 gt or more (the plate's `pressedTime`), S sleeps, the zombie shows again | D1's pit | 8 | 1 | about 280-320 iron/h (only brief hides, day and night); the egg must reach the plate without the chicken pressing it | about 360 |
| D4 | the bed plate driving two hides for two scared villagers (S1, S2, U1: 3 villagers) | D1's pit | 8 | 1 | about 280 iron/h: two panicking villagers make 20 tries a mark instead of 10 (a mark succeeds about 93% instead of 74%) | about 330 |

Rejected calm moments: the zombie's own state (water converts it, the sun burns it, nothing else
of it changes with day time); a hide moved by water, sand or a door (each needs a trigger); a
copper lightning rod (it powers only when struck: storms only); village villagers walking over a
plate at dusk (random). Redstone dust is allowed (its iron pickaxe counts 3 iron in T), but no
day-1 calm above needs it.

**Pick: the search** (the optimizer's day-1 fronts, judged here). Revision 8's pick by T was D1:
2 x 318 + 120 + 540 = 1,296 s, plus (8 + 24) / 243 h, about 30 minutes, against the archived L3's
about 54 (2 x 189 + 120 x 16 + 180 x 3 = 2,838 s, 34 minutes of it lava trips); D1U, dug into rock
at village-edge, about 25. These are seeds: each is a point of the space, measured, and the front
says what beats it. D1U in terrain at iteration 3's prices (the day-1 lane): a 27-minute build
(36 on flat ground), T measured 32 minutes, 254 iron/h, no glass, leaves or smelted stone.

**Open lever: hostile suppression around the farm** (the parent, from D1U's thunder days; vjeux
14:04: "Seems legit that iron golems go after mobs?"). A golem targets the nearest hostile
(`Enemy`, not a creeper) it can SEE within its follow range, 16 x (1 + triangle(0, 0.11485)) =
14.2-17.8 blocks (`IronGolem.registerGoals`: `NearestAttackableTargetGoal<Mob>` with
`TargetingConditions.forCombat()`, which checks line of sight: a COLLIDER ray eye to eye; once
acquired it keeps chasing behind walls; or one that pushes into it, 1 time in 20,
`IronGolem.doPush`), and walks toward it instead of on into the kill: it lingers in the villagers'
follow cubes and holds back the next summon. In a thunderstorm the surface is dark enough for
monsters by day too (`Monster.isDarkEnoughToSpawn` reads the local brightness with the sky
darkened by 10). D1U made about 58 golems an hour in the game on thunder days against 63
predicted. Attributed (the 8-server game trace): neither channel above: a golem escaped the farm
to the surface in one run and, lingering near the villagers, blocked spawns for about 9,000 gt
until it walked out of the loaded area; runs without the escape match the prediction (64.0 vs
64.0), and no natural mob came within 8 of a villager. The escape breaks the one rule, so it is a
design fix (the day-1 lane), not this lever. The other channel stays possible on other farms: a
hostile within 8 of the blind villager U1 and in its sight at the start of REST keeps U1
panicking (`VillagerHostilesSensor`), so it never lies down, the plate holds the hide open, the
scared villager never sleeps, and the night is missed. Monsters need block light 0 (the
overworld's `monster_spawn_block_light_limit`), so torches lighting the cells that matter (a
torch reaches block light 1 at 13 steps) remove either loss. Role `hostile_suppression`
(`designs/farms/roles.tsv`): it enters the optimizer's grammar, torches weighed against a loss,
once the judge runs golem targeting with natural hostiles on (mob AI, 8a0201dc); until then the
card's thunder-day rate measures it.

### Middle

The player has a base, iron gear and the Nether (quartz, redstone, magma, slime), villagers from a
village or a small breeder; no End.

Hard limits (each a real fact, iteration 4: vjeux 12:18, "You just invented the two hours build
limit. This is not a real restriction"):
- **Nether-tier items**: nothing from the End (no shulker box, no end rod): what the player has.
- **More than one module** (vjeux 10:48: "middle one should have more than one module"), with no
  maximum: how many is the player's choice, traded by the objectives.
- **One AFK spot**: entities tick only within a player's simulation distance (`DistanceManager`),
  so every module runs from where the player stands.

Not limits (iteration 4 took them out): the build's size (build minutes is an objective: the front
shows what each extra hour buys), a maximum module count, a shared part (copies at the no-coupling
pitch are the bar, printed beside the front; a member that shares nothing says so), the placement
(built in the open, or dug into flat ground where the grammar has it). The search tiles N up to 16:
a search boundary, not a player limit, since past the sharing's period each added module repeats
the last one's numbers.

Objectives (per module, at the farm's own N: Common, "A farm and its N"):

| objective | the player's reason |
|---|---|
| **max net iron per hour per module** (W = one game day) | a steady supply (hoppers for the other farms, rails, anvils, a beacon's blocks: a full pyramid is 164 iron blocks), and each module costs its villagers and its zombie |
| **min build minutes per module** | the time it takes to build; the player then picks how many modules from the curve |

Why these two. Revision 8 ranked the middle by iron per hour among builds of at most an hour, and
the first cut of revision 9's task by iron per hour per block: a ratio that ranks a farm without
saying how much iron it makes. The two per-module axes show both, and the best iron per hour per
build minute is always on the front (a farm beating it on both axes would beat its ratio). Iron
per hour per block is printed on every member. Per module, at the farm's own N, so that a farm
and its copies are one member (iteration 4).

Where a middle farm can win (iteration 12, from middle run 2 and the module's priced build): the
module alone already reaches 92% of the ceiling (m2-open-ring: about 441 iron an hour per module
against 480), so no farm beats copies by 10% on iron per hour per module; it beats them only on
build minutes per module, at most 90% of the copies' (the role-materialized module costs about
25 minutes at middle prices) while keeping at least 90% of their rate. A module's minutes go to
its villagers (4 moved, 8 minutes, about a third: never shared), placing its blocks (about 7),
its kill chamber and collection (2 hoppers, 4 lava in one trip and the chamber's blocks: about 5),
its zombie (2) and its calm (1-3: the timer and its wiring). So the gain is in sharing the kill
chamber and collection among modules with almost free transport to it (golems from every pad
reach one chamber more than 20 blocks below every villager, outside every follow cube), and the
zombie and calm only where the coupling they bring costs less than they save.

Forks: none. The calm (the bed plate, the bed observer with its catalog pulse, daylight
detectors), the kill, the tiling and the villagers per cluster are mechanisms the front chooses
between; a fork is kept only for what the player cannot trade on the spot (day 1's redstone).

Parts: one calm trigger may serve every module (every cluster rests at the same day time), its
pulse a catalog part with the stage's materials; the kill chamber, the drop and the collection
are shared wherever the search finds it pays.

Shown on the card, in this order (the stage's default rows): the iron per hour and the build
minutes; the rain and thunder day rates (a middle farm runs through the weather); the quartz and
iron it needs; where it fits by the base (chunk columns, dimensions); back after days away; how
long the storage lasts. No time to a kit and no iron up front: a middle player has the iron.
Measured and kept under all measurements: iron per hour per block, the share of the ceiling, the
lava trips, the exposures, the poppies, hoppers per 1000 iron per hour.

Seeds (one module each but M4; the space builds their multi-module versions, sharing parts or not):

| # | the calm moment | module | iron | estimate | blocks |
|---|---|---|---|---|---|
| M1 | inverted daylight detectors' dusk pulse (the archived iron-middle, pulled) | L2d: 4 villagers, a 6 x 6 dry-hole pad, a 16-block drop, a 2-hopper chamber whose oak fence gates sit beside the lava | 13 | 443 iron/h in clear weather; the gate: lava's fire burns the gates by about gt 2,000 and the drops are lost (0.04-0.07 iron per golem), 0-36 golems/h on a rainy day, 0 in thunder. Expected loss about its whole rate: last | 579 |
| M2 | the bed observer: an observer reads U1's bed going OCCUPIED at REST and, through a repeater line, shuts the hide for about 2 s; the scared villagers fall asleep, then panic all night | M1's module with the fire-safe chamber (its ceiling right above the lava: no air cell in `LavaFluid.randomTick`'s reach touches a gate) | 16 + 2 quartz | redsim, the whole judge: 439 iron/h (435-443), 92%, restarts after a missed night and after days away; the same with rain and thunder (it reads no sky). Build 2 x 638 + 4 lava + 4 villagers: 41 min | 638 |
| M2-open | M2 | M2 with the drop walled only in its bottom 4 layers (a golem leaving the dry hole falls straight) | 16 + 2 quartz | redsim, the whole judge: 439 iron/h (435-443), 92%, the same tradeoffs as M2. Build 2 x 404 + 480 + 720: 33 min | 404 |
| M3 | the bed plate (D1's calm) | M1's module with 4 villagers (3 scared, U1 on the plate) | 13 | redsim: 378 iron/h, 79% (the sleeping-eye flicker) | about 590 |
| M4 | M2 | two M2 modules stacked 43 apart | 32 + 4 quartz | redsim: 887 iron/h (880-893); build 2 x 1,276 + 960 + 1,440: 83 min, over the evening | 1,276 |

**Pick: the search.** Revision 8's pick, M2-open (one module, 439 iron/h, a 33-minute build), is
out of the stage now: one module, and its 48 gt hide is 12 hand-placed repeaters where the catalog
has smaller parts. Its multi-module versions with a catalog pulse are seeds.

### Late

The player has everything, End included; villagers come from breeding. The challenge (vjeux,
04:26): stack modules next to each other and gain efficiency while keeping the build small. Copies
at a pitch where they never interact gain nothing: they are the bar, printed beside the front.

Hard limits (each a real fact, iteration 4):
- **Any survival item.**
- **The whole build runs from one AFK spot**: entities tick only in chunks within the simulation
  distance of a player (`DistanceManager`: the player ticket at level `ENTITY_TICKING -
  simulationDistance`). A dedicated server defaults to 10 (`simulation-distance`). (This is what
  "works as its card says" means for a farm of N modules.)
- **The world's build height**, y -64..319, for a stack.

Not limits (iteration 4 took them out): a fixed module count (revisions 6-9 compared at N = 8:
the number of modules is the player's choice, traded by the objectives; every member prints its
curve at N = 1, 2, 4, 8 and 16), a minimum gain from stacking (the copies bar beside the front
shows it: a member that does not beat the bar on volume per module or iron per hour per build
minute is reported as gaining nothing from stacking), 8 h of storage (the 8-h net rate already
loses what overflows). The search tiles N up to 16: a search boundary, not a player limit, since
past the sharing's period each added module repeats the last one's numbers.

Objectives (per module, at the farm's own N: Common, "A farm and its N"):

| objective | the player's reason |
|---|---|
| **max net iron per hour per module** (W = 8 h: a break stops it for the rest of the night; iron the storage cannot hold is lost) | what one night AFK brings, module by module |
| **min build minutes per module** | the effort: the villagers moved, the lava trips, the blocks |
| **min volume per module** | the room it takes around the AFK spot, beside the other farms; height is not free (the 15:01 miss: footprint metrics rewarded it); it falls as modules share parts |

Why not iron per hour per block alone (revisions 6-8): it ranks a 150-block module at 70% of the
ceiling over a 400-block module at 92%, although the second brings about 30% more iron every night
for a one-time hour of building, and a late farm runs for many nights. The three axes keep both,
and iron per hour per block is printed on every member. Robustness weighs heavily here through W:
a farm that stalls in rain, loses a villager to lightning or fills its chests at hour 3 collects
that much less over the night.

Forks: none (the tiling, the storage, the kill and the villagers per cluster are on the front).

Parts: the collection's hours and hoppers are part metrics (iron packed into blocks by the
catalog's crafter, or chests; the poppies composted or destroyed); a collection that fills before
the night ends loses iron in the net rate. The kill chamber, the collection, the zombie and the
calm trigger are shared between modules wherever the search finds it pays.

Shown on the card, in this order (the stage's default rows): the iron per hour and what a night
AFK brings; the hours it runs AFK before its storage fills (iron packed into blocks by a crafter,
or chests; the poppies composted or destroyed); the lag (entities alive at once, hoppers per
thousand iron an hour); back after days away; the room per module and the chunk columns; the
build minutes and the villagers moved. No lava trips, no time to a kit, no iron up front: a late
player has all of it (vjeux 11:17: "We don't care about lava trips for the high end farm").
Measured and kept under all measurements: iron per hour per block and per module, each module's
share of its ceiling with its neighbours present (revision 7), the curve over N, the exposures.

What couples modules (26.3):
- **The spawn's 10-box.** `Villager.spawnGolemIfNeeded` counts the villagers that want a golem in
  the summoner's box inflated by 10 (at most 5 counted, 3 needed) and, after a spawn, calls
  `GolemSensor.golemDetected` on every villager in that box (`GOLEM_DETECTED_RECENTLY` for 599 gt).
  Villagers within 10 of each other are one cluster: one golem per 600 gt (the ceiling).
- **The follow cube.** `NearestLivingEntitySensor` (every 20 gt) lists every living entity in the
  villager's box inflated by its `FOLLOW_RANGE` on all three axes (16 x (1 + triangle(0,
  0.11485)): 14.2 to 17.8), with no sight test; `GolemSensor` (every 200 gt) resets the villager
  when that list holds an iron golem. A golem that stays D gt inside a foreign cube is listed from
  the first 20 gt scan inside its stay to the first scan after it leaves, a window that averages D
  gt, so it resets that villager with a chance of about D / 200 (G1, 8146b8db).
- **The spawn search** reaches 8 columns out and 6 up or down (`SpawnUtil.trySpawnMob(…, 10, 8, 6,
  LEGACY_IRON_GOLEM)`), so it can land on a neighbour's pad.
- **A golem cannot die fast**: 100 health, immune to falls (`fall_damage_immune` lists the iron
  golem), lava 4 a hit every 10 gt (the damage cooldown: 25 hits, about 250 gt to die). So a golem
  must leave the cubes of foreign villagers, not die in them; free fall from rest covers 3 blocks
  in 9 gt, 8 in 14 and 14 in 19, and a cube reaches 14 to 18 below a villager.
- **Measured** (redsim, two M2 modules side by side, 5-day runs): 16 apart (adjacent chunk
  columns) each module keeps 83-85% of the ceiling, 20 apart 87-94%, 24 apart 92-95% (no
  coupling).

Where the gain is: shared parts (the kill chamber, the drop, the collection, water, the calm: one
zombie within 8 of two clusters, one hide, one bed observer), golems that leave every foreign cube
fast (pads at the cube's edge, drops away from all villagers), redundancy (a 4th or 5th villager
in a cluster absorbs foreign resets: 3 of up to 5 are needed) and a smaller module (most of M2's
638 blocks are its own drop and chamber).

Candidates (the search on redsim; "measured" is the whole judge, 64 runs):

| # | packing | per module | blocks per module | iron/h per block | volume per module |
|---|---|---|---|---|---|
| LB | the bar: M2 copies 43 apart vertically, each with its own everything | 443 iron/h, 93% (measured) | 638 | 0.69-0.70 at N = 1, 2, 4, 8 (measured) | 16 x 16 x 43, flat |
| M2-open copies | LB with M2-open (the drop walled only in its bottom 4 layers) | 439, 92% (measured) | 404 | 1.09, flat | flat |
| LS-open | M2-open modules sharing one collection: every chamber drains into one chute, two sorter slices (iron to the iron-block crafter, poppies to a composter) | 439, 92% at N = 1 (measured) | 404 + 65 / N | 0.94 at N = 1 (measured), rising with N | falling with N |
| LT | two M2 fronts back to back sharing one pad, drop and chamber | 343, 72% (measured: the shared pad sits in both clusters' cubes) | 409 | 0.84 at N = 2 (measured) | |
| TZ | two clusters around one zombie, one hide, one bed observer; pads on the far sides, out of the other's cube (zero foreign dwell, proved by `mobfarm cubes`) | 389, 82% (measured: the strip pads' long water paths keep golems in their own cube) | 617 | 0.63 at N = 2 (measured) | |
| LP | up/down pairs sharing a zombie and a chamber, the upper cluster's golems lifted by a soul-sand bubble column | not built: no golem spawns in or under the column (`IronGolem.checkSpawnObstruction` wants its 2 cells above dry) and a golem pushed in off-centre catches on its walls; a gate on every layer lifts it for about 260 blocks more | about 560 | about 0.87 (estimate) | |

The coupling numbers point the search: short pad paths (the dry hole in the middle, M2's
geometry) matter as much as anything, since a villager also lists its own cluster's golems;
shared parts pay only where no golem dwells in a foreign cube. **Pick: the search**; these are
its seeds (LS-open, the first measured gain from sharing, first).

## Mechanics, from the game code (26.3)

### Who spawns a golem and when

- **The trigger is the panic behavior.** `VillagerGoalPackages.getCorePackage` puts
  `VillagerPanicTrigger` in the CORE activity, which is always active. It is a `Behavior` with
  an empty entry map, the default 60 gt duration and no `checkExtraStartConditions`, so
  `Behavior.tryStart` always succeeds. Its `start` tests `isHurt` (memory `HURT_BY`) or
  `hasHostile` (memory `NEAREST_HOSTILE`) and then makes PANIC the active activity.
  `canStillUse` repeats that test, so an unscared villager starts and stops it in the same
  tick. Its `tick` runs `if (timestamp % 100L == 0L) body.spawnGolemIfNeeded(level, timestamp,
  3)`. `Behavior.tickOrStop` stops it once `timestamp > endTimestamp`, and it starts again the
  next tick. So for a villager that stays scared, one tick in every 62 has no `tick` (gt
  `t0 + 61 + 62k`).
- **Every willing, panicking villager tries on its own.** At a multiple of 100, each villager
  whose trigger ticks runs `Villager.spawnGolemIfNeeded` with its own 10 attempts, centred on
  its own `blockPosition()`, in entity tick order. A failed try marks nothing, so the next
  villager tries too: up to 10 x N attempts from N centres. An attempt is lost to the panic gap
  only when every eligible villager is in its gap tick at once.
- **`Villager.spawnGolemIfNeeded(level, timestamp, 3)`:** if `wantsToSpawnGolem`, it lists the
  villagers whose box meets this villager's box inflated by 10 (`getEntitiesOfClass`, itself
  included). It keeps those that `wantsToSpawnGolem`, at most 5. With 3 or more it calls
  `SpawnUtil.trySpawnMob(IRON_GOLEM, MOB_SUMMONED, level, blockPosition(), 10, 8, 6,
  Strategy.LEGACY_IRON_GOLEM, false)`. **Only `wantsToSpawnGolem` is tested for the others: a
  sleeping or idle villager counts.** On success, EVERY villager in the box (willing or not)
  gets `GolemSensor.golemDetected`: `GOLEM_DETECTED_RECENTLY` with a 599 gt expiry.
  `Villager.gossip` calls it too, needing 5 willing villagers (see the judge's gossip rule).
- **`wantsToSpawnGolem` = `golemSpawnConditionsMet && !GOLEM_DETECTED_RECENTLY`**.
  `golemSpawnConditionsMet` holds when the memory `LAST_SLEPT` exists and `gameTime - lastSlept
  < 24000`. **Without a sleep in the last game day, a villager never wants a golem.**
- **Memory expiry order.** `Brain.tick` runs `forgetOutdatedMemories`, `tickSensors`,
  `startEachNonRunningBehavior`, then `tickEachRunningBehavior`. `MemorySlot.tick` clears a
  memory when `timeToLive <= 0`, else decrements it. When villager V spawns a golem at T:
  - V and the villagers before it in the entity list clear the memory at their own brain tick
    at T+600, the villagers after it at T+599;
  - so at T+600 only V and the villagers after it can count 3: one earlier in the list still
    reads V's memory at ttl 0 until V's own `forgetOutdatedMemories`.

  The next multiple of 100 is T+600. **One golem per 600 gt per villager cluster is the
  ceiling** (below).
- **The golem detector.** `GolemSensor` (scan rate 200) reads `NEAREST_LIVING_ENTITIES`. If any
  entity in it is an iron golem, it sets the same 599 gt memory. `NearestLivingEntitySensor`
  (rate 20) stores every living entity (`isAlive`) whose box meets the villager's box inflated by
  its `FOLLOW_RANGE`, sorted by distance. The golem sensor reads that stored list, so a golem
  alive and in range at the last 20 gt scan counts, even if it died since.
  - The range: `Mob.finalizeSpawn` draws the bonus `b = random.triangle(0, 0.11485)`, and the
    villager's follow range is 16 x (1 + b), so R is in 14.16..17.84.
  - With the villager's 0.6 x 1.95 box and the golem's 1.4 x 2.7 box, a golem is listed when
    `|dx|, |dz| < R + 1.0`. Vertically, its feet must be above `villager y - R - 2.7` and below
    `villager y + 1.95 + R`.
  - A golem the sensor sees between T and T+600 pushes the next attempt to the first multiple
    of 100 after its own 600 gt. **The golem must leave every villager's box (or die) before
    their scans see it.**
- **Sensor phases.** `Brain`'s constructor calls `Sensor.randomlyDelayStart(body.getRandom())`
  for each sensor in registration order (`Villager.BRAIN_PROVIDER`: nearest living, players,
  items, bed, hurt by, hostiles, babies, secondary POIs, golem). That is `nextInt(scanRate)`
  from the entity's own random. `Sensor.tick` runs `doTick` when `--timeToTick <= 0`, then waits
  `scanRate`.
- **The hostile sensor.** `VillagerHostilesSensor` (rate 20) reads `NEAREST_VISIBLE_LIVING_ENTITIES`.
  It takes the nearest hostile within its type's distance: zombie, zombie villager, husk,
  drowned and vex 8; vindicator and zoglin 10; evoker, illusioner and ravager 12; pillager 15.
  The distance is `mob.distanceToSqr(body)` from feet to feet. The hostile must also pass
  `Sensor.isEntityTargetable`: `TargetingConditions.forNonCombat` with the follow range,
  `getVisibilityPercent`, and `Mob.getSensing().hasLineOfSight`.
  - That last test is `LivingEntity.hasLineOfSight`: a `ClipContext.Block.COLLIDER` /
    `Fluid.NONE` ray from eye to eye, at most 128 blocks. Glass blocks it (it has a collision
    box); water does not.
  - `NearestVisibleLivingEntities` evaluates it lazily and caches it per entity, for one
    20 gt list.
  - In PANIC and REST the only other readers are the minimal look behaviors
    (`SetEntityLookTarget` VILLAGER and PLAYER), and they filter by type before the ray. So the
    zombie's ray in each list is first cast by the hostile sensor.

### Sleep: the condition every 26.3 iron farm has to meet

- **A panicking villager never sleeps.** `SleepInBed` is in the REST package, and its `start`
  is the only writer of `LAST_SLEPT`. While PANIC is active, the schedule is not consulted:
  only `VillagerCalmDown` (in the PANIC package) calls `updateActivityFromSchedule`, and only
  when the villager is not scared (no `HURT_BY`, no `NEAREST_HOSTILE`, no `HURT_BY_ENTITY`
  within 6).
- **So the villagers that see the zombie need a calm moment each night.** Within the REST
  window, the zombie must be hidden from their eyes during one hostile scan: at least 20 gt, to
  catch every sensor phase. A villager that never sees the zombie sleeps through every REST on
  its own, and since `spawnGolemIfNeeded` tests only `wantsToSpawnGolem`, it still counts.
  Only the scared villagers need the hide (layout L6).
- **The schedule** (`Timelines`: `VILLAGER_SCHEDULE`, adult track): day time 10 IDLE, 2000 WORK,
  9000 MEET, 11000 IDLE, 12000 REST. `Brain.updateActivityFromSchedule` runs at most once per
  21 gt (`gameTime - lastScheduleUpdate > 20`). `setActiveActivityIfPossible` falls back to the
  default IDLE when the activity's requirements fail: WORK needs `JOB_SITE`, MEET needs
  `MEETING_POINT`.
- **`SleepInBed`** (REST, priority 3) starts only under all of these:
  - the villager is not a passenger;
  - it holds a `HOME` memory;
  - it was not woken 1..99 gt ago (`LAST_WOKEN`);
  - the HOME position is `closerToCenterThan(position, 2.0)`;
  - the bed is not `OCCUPIED`.

  `start` (when `timestamp > nextOkStartTime`) calls `startSleeping(home)` and sets
  `LAST_SLEPT = timestamp`. It stays running while REST is active, `y > bed.y + 0.4` and the
  bed is within 1.14. `stop` calls `stopSleeping` and `nextOkStartTime = t + 40`. `WakeUp`
  (CORE) stops the sleep when REST is not active. A villager that calms down at night falls
  asleep in the same brain tick (CalmDown at priority 0 switches to REST before priority 3
  starts). It only needs to start sleeping once per game day: a panic right after waking it
  changes nothing.
- **`LivingEntity.startSleeping`**: position at the bed + (0.5, sleep height 0.5625 + 0.125,
  0.5), `OCCUPIED`, pose SLEEPING (0.2 x 0.2, eye 0.2), motion zero. **`stopSleeping`**: bed
  free; position from `AbstractBedBlock.findStandUpPosition`. That searches the cells around the
  bed (the side chosen by the villager's yaw), safe ones first, else the cell above the bed
  (+0.1). Pose STANDING.
- **Keeping HOME** (rewritten at iteration 14 from the code; revisions 1-13 had it wrong).
  `SetWalkTargetFromBlockMemory(HOME, 0.5, 1, 150, 1200)` (REST, priority 2, needs WALK_TARGET
  absent and HOME present) runs its release branch (`releasePoi(HOME)`, HOME erased,
  CANT_REACH_WALK_TARGET_SINCE set) whenever CANT_REACH_WALK_TARGET_SINCE is present and older
  than 1200 gt, whatever the distance to HOME; otherwise it sets a walk target when
  `distManhattan(home, blockPosition()) > 1`. Any unreachable walk target arms it:
  `MoveToTargetSink.tryComputePath` writes CANT_REACH_WALK_TARGET_SINCE on any unreachable target
  (an IDLE stroll, InteractWith, a look-target walk), only when the memory is absent, so the
  oldest failure stands; only a reached or reachable target, or `SleepInBed.start`, clears it. So
  a villager in a 1-cell pod releases HOME at nearly every REST start. Every cell it can stand in
  within Manhattan 1 of HOME is necessary (no walk toward HOME) but not sufficient.
- **Claiming the bed back** (`AcquirePoi` for `HOME` in CORE): every 20-39 gt it takes up to 5 of
  the closest free beds within 48 (validBedPoi: not OCCUPIED), makes one path to the set and takes
  whichever bed that path reaches, so a neighbour's free, reachable bed can be taken instead of
  its own; a failed path retries after 40-80 gt, growing to 400. Claiming runs every night, so a
  cage must make its own bed the only free bed AcquirePoi can reach from the pod (Stages, the
  Beds line).

### Where a golem spawns

- **`SpawnUtil.trySpawnMob`**: 10 attempts. Each draws `dx, dz = Mth.randomBetweenInclusive(
  random, -8, 8)` from the LEVEL random and starts at (villager block + (dx, 6, dz)).
  `moveToPossibleSpawnPosition` walks down from y+5 (with the cell above it, y+6) to y-7. It
  stops at the first cell whose block passes `LEGACY_IRON_GOLEM`:
  - not cobweb, cactus, glass, tinted glass, stained glass, any glass pane, leaves, conduit,
    ice, frosted ice, TNT, glowstone, beacon or sea lantern;
  - the block above is air or a liquid;
  - the block `isSolid()` (or is powder snow).

  The golem goes in the cell above. `BlockStateBase.calculateSolid` says solid when the block
  forces it, or when its collision shape is non-empty and its bounds average at least 0.7292 or
  are a full block tall. Slabs (top or bottom), beds and 3 px trapdoors are solid; carpets are
  not. The scan stops at the first match even when the obstruction check then fails (a bed, a
  bottom slab): that attempt is wasted. Spawn cells run from villager y-6 to y+6.
- **`EntityType.create`** puts the golem at (x + 0.5, y, z + 0.5) with yaw
  `level.random.nextFloat() * 360`. `Mob.finalizeSpawn` draws the follow-range triangle and
  left-handedness from the level random, whether the spawn succeeds or not.
- **`IronGolem.checkSpawnObstruction`** has four conditions:
  - the block below `entityCanStandOn` (its collision face UP is full);
  - the cells at +1 and +2 pass `NaturalSpawner.isValidEmptySpawnBlock`: not a full collision
    cube, not a signal source, no fluid, not `PREVENT_MOB_SPAWNING_INSIDE` (rails), not
    dangerous (fire, lava, magma, campfire, wither rose, berry bush, cactus, powder snow);
  - the feet cell passes the same check with an EMPTY fluid, so **a golem may spawn standing
    in water**, but not with water at head height;
  - `isUnobstructed`: no entity with `blocksBuilding` meets the 1.4 x 2.7 box. Living entities
    count, dying ones until their removal 20 gt after death, and so do boats and minecarts;
    items do not.

  Block collisions are not checked (`checkCollisions` false): the box may overlap walls.

### The golem, the kill and the drops

- **`EntityTypes.IRON_GOLEM`**: 1.4 x 2.7 (eye 2.295). `IronGolem.createAttributes`: 100
  health, speed 0.25, knockback resistance 1, step height 1. Fall damage immune (tag
  `fall_damage_immune`). It does not drown (`decreaseAirSupply` returns the supply). It is
  pushed by water (`isPushedByFluid`).
- **Movement**: `LivingEntity.aiStep` runs the 0.003 floors, the jump, `travel`,
  `applyEffectsFromBlocks` and `pushEntities`. `travel` picks one of:
  - `travelInAir`: block friction x 0.91, gravity 0.08, x 0.98 vertical;
  - `travelInWater`: x 0.8, speed 0.02, gravity / 16, `jumpOutOfFluid`;
  - `travelInLava`: x 0.5, gravity / 4.

  `Entity.move` is the existing redsim collide.rs. `pushEntities` calls `Entity.push`
  (0.05 / distance). Cramming above the game rule's 24 does 6 damage on
  `random.nextInt(4) == 0`.
- **Transit in water.** `Entity.updateFluidInteraction` adds the normalized current x 0.014 a
  tick to a non-player (`EntityFluidInteraction.applyCurrentTo`), and `travelInWater` keeps 0.8
  of it. The terminal drift is about 0.056 blocks/gt.
  - A 1.4-wide golem resting on a 1-wide strip stays on it until its centre has moved 1.2:
    about 25 gt from rest. It cannot drop into a 1-wide channel, so channels must be at least
    2 wide.
  - Its own walking input (`IronGolem.registerGoals`: stroll 0.6, move-towards-target 0.9,
    melee 1.0 of its 0.25 speed) adds `moveRelative(0.02, input)`, at most about 0.005 a tick.
    It never beats the current, but it moves the drift between about 0.036 and 0.076
    blocks/gt with its direction.
  - Falling: v <- (v - 0.08) x 0.98. Leaving the follow box from a pad at villager y-6 takes
    at least 15.5 blocks of fall, about 22 gt.
- **Damage**: `LivingEntity.hurtServer` with `damageCooldownTime` 20. While it is above 10,
  only damage higher than `lastHurt` lands, and only the difference.
  - Lava: `LavaFluid.entityInside` sets LAVA_IGNITE (15 s of fire) then `lavaHurt`, 4 damage.
  - Fire: `Entity.baseTick` does 1 damage every 20 fire ticks when not in lava.
  - In a wall: `isInWall`, 1.
  - Magma: `stepOn`, 1.

  A golem in lava dies in about 250 gt, and nothing environmental is faster (fall immune, no
  drowning). **`die`** drops the loot at once at the golem's position. **`tickDeath`** removes
  the body 20 gt later.
- **Loot** (`loot_table/entities/iron_golem.json`): a poppy pool `uniform(0, 2)` and an iron pool
  `uniform(3, 5)`: 4 ingots and 1 poppy on average. `Entity.spawnAtLocation` creates an
  `ItemEntity` at the golem's feet with motion (`random.nextDouble() * 0.2 - 0.1, 0.2, ...`)
  from the item's own random, and a pickup delay of 10. Items burn in lava and fire, so the
  drops must not reach the blade. Iron golems do not despawn.
- **The golem's targets.** `IronGolem.registerGoals`'s `NearestAttackableTargetGoal<Mob>`
  (`Enemy && !Creeper`) acquires a hostile only in sight: its conditions are
  `TargetingConditions.forCombat()`, which keeps `checkLineOfSight`, so `test` needs a COLLIDER
  ray from the golem's eye (2.295) to the target's eye (`Mob.getSensing().hasLineOfSight`) and
  the feet-to-feet distance within the follow range times the target's visibility percent.
  mustSee false only keeps a target already acquired (`TargetGoal.canContinueToUse` skips the
  unseen-ticks drop: once seen, a hostile is chased behind walls). The one sight-free path is
  contact: `IronGolem.doPush` targets an Enemy that pushes into it, 1 time in 20.

### The zombie

- **`EntityTypes.ZOMBIE`**: 0.6 x 1.95, eye 1.74, riding offset -0.7. In a wooden boat it sits at
  boat y + 0.1875 - 0.7 (`Boat.rideHeight` = height / 3); in a minecart at cart y + 0.1875 - 0.7.
- **Persistence**: `Mob.requiresCustomPersistence` = `isPassenger() || isLeashed()`, so a
  zombie in a boat or minecart, or on a lead, never despawns. Otherwise `checkDespawn` removes
  it beyond 128 blocks of a player, or at random beyond 32.
- **Reach**: `Mob.getAttackBoundingBox` takes the horizontal union of the zombie's box and its
  vehicle's box, over the zombie's own vertical span, inflated horizontally by
  `DEFAULT_ATTACK_REACH = sqrt(2.04) - 0.6 = 0.828`. In a 1.375-wide boat that reaches 1.52 from
  the boat's centre.
- **Sunlight** (`Mob.isSunBurnTick`): while `MONSTERS_BURN` holds (day time 23460..12542), at
  brightness > 0.5, when the eye cell `canSeeSky` (sky light 15).
- **Drowning**: `Zombie.tick`'s drowning tracker turns it into a drowned after its eye spends
  enough time in water.
- **Pickup**: `Mob.aiStep` with `canPickUpLoot` (`Zombie.finalizeSpawn`: 55% x the difficulty
  multiplier), reach (1, 0, 1).
- **Villager kills**: `Zombie.killedEntity` may turn a villager into a zombie villager.

### The calm devices, weather and lightning

- **Tripwire** (`TripWireBlock.entityInside` -> `checkPressed`): any entity whose box meets the
  string presses it. That includes a dying golem until its removal 20 gt after death, and its
  drops. The press is re-checked every 10 gt while pressed, and the hooks
  (`TripWireHookBlock`) power while the string is pressed.
- **Daylight detector** (`DaylightDetectorBlock`): it updates when `gameTime % 20 == 0`. The
  inverted output is `15 - getEffectiveSkyBrightness`, which is the sky darkening.
- **Rain** (`ServerLevel` weather; `WeatherAttributes`): it blends the sky light level toward 4
  (rain alpha 0.3125: darkening 3 at noon; thunder 0.527: 5). An inverted detector therefore
  outputs at least 3 all through a rainy day: L1 hides its zombie and stops. It rains about
  16% of the time (`RAIN_DELAY` uniform 12000..180000, `RAIN_DURATION` 12000..24000).
- **Lightning** (`ServerLevel.findLightningTargetAround`): it strikes a living entity that can
  see the sky (sky light 15, which glass keeps) within 3 of the strike column. A struck
  villager turns into a witch.

## Trade-offs

- **The calm moment** has three options:
  - a sensor, the daylight detector (3 nether quartz, so not day 1; it fails in rain);
  - a trigger the farm already makes: a tripwire pressed by each burning golem during the 599
    gt cooldown, which costs no attempt;
  - no hide at all for the villagers that never see the zombie (L6).

  Hidden all night, the scared villagers sleep all night and no golem spawns until the zombie
  shows again: about half the rate. Hidden briefly, they fall asleep and are woken by the
  zombie, provided their sleeping eyes (bed + 0.8875) can see it.
- **The golem's exit.**
  - A golem in a villager's follow box at a 20 gt scan is in the stored list for 20 gt, and each
    golem scan (period 200) that falls in that window marks the villager: about window / 200
    per villager. Each sighting stretches the cycle by about 100 gt.
  - On a water pad the golem drifts at 0.036-0.076 blocks/gt. On a 13 x 13 pad it stays in range
    60-90 gt before its drop.
  - The fastest exit: a 1-wide strip with water across it into a 2-wide channel and a fall out
    of the box from villager y-6. That is at least about 25 + 22 = 47 gt in range, so each
    villager sees about 25-30% of the golems. With 3 villagers about 63% of the cycles stretch
    (about 90% of the ceiling); 95% needs 4 villagers.
  - A golem that burns next to the villagers (about 250 gt) is seen by every villager: about
    850-900 gt per golem instead of 600, about 2/3 of the ceiling.
- **Spawn spots.** Each attempt picks one of the 17 x 17 = 289 columns around its villager, and
  every willing panicking villager makes its own 10. With N such villagers and a share f of
  useful columns in each square, a hundred-tick mark succeeds with about 1 - (1 - f)^(10N).
  - A 4 x 4 pad inside three villagers' squares: f = 5.5%, 43% for one villager, 82% for three.
  - A failed mark waits 100 gt.
  - Columns topped by anything solid (a pod roof, a wall) waste attempts or spawn golems in the
    wrong place, which is why the pods are glass.
- **More villagers.** With 4 per cluster, one villager's sighting no longer blocks it (only 3
  must agree), and 5 willing villagers (the gossip path) can never be gathered. The cost is a
  bed and a transport each.
- **Scaling.**
  - Clusters must stay out of each other's 10-block box.
  - Every golem must stay out of every other cluster's follow box: `|dx|, |dz| < R + 1.0` with
    R up to 17.84. A golem spawns up to 8 columns from its villager, so a full-pad module
    needs a pitch of at least 26.9 horizontally (the pad's reach + 18.84 in general).
  - Stacking: a module Q blocks below another lists the upper one's golems while
    Q < 25.8 (6 + 2.7 + 17.84 - 0.74). Golems falling past a lower module must keep 18.84
    horizontally from its villagers.
  - One zombie per cluster: it scares villagers within 8 only.
  - Each module needs its own kill cell (or its own trigger): a tripwire in a shared kill cell
    is pressed by every module's golems (one arrival per 600 / N gt, each holding it about 270
    gt), which hides the zombies almost all the time.
- **The bill**: a hopper is 5 iron, so the collection line dominates. One golem (4 iron) pays
  for most small farms.

## Six layouts

Per cluster unless stated. Ceiling: 120 golems/h = 480 iron/h per cluster. The estimates are
mine, before the judge, with the review's corrections.

| # | layout | calm moment | golem exit | estimate | bill (iron) | blocks |
|---|---|---|---|---|---|---|
| L1 | **The obvious design**: 3 villagers on beds in 1-cell glass pods under an opaque roof; the zombie in a boat in view; a 9x9 pad 3 below them with water to a 2x2 hole; a lava blade over 2x2 hoppers | an inverted daylight detector pushes a block into the line of sight at night (they sleep all night; it also stops in rain) | the fall through the hole; the blade 6 below the pad, in range while burning | ~35% in clear weather (42 golems/h, 170 iron/h) | 4 hoppers 20 + piston 1 + bucket 3 = 24, +3 quartz | ~200 |
| L2 | **Middle ground**: L1's pods and zombie; a 13x13 pad with water to a 2x2 hole; the module's own kill cell 22 below the villagers | a tripwire in the kill cell: the burning golem holds it about 270 gt, so a piston hides the zombie during the cooldown; at night the villagers sleep and are woken at once (the zombie is in view of their beds) | 60-90 gt on the pad, then a ~22 gt fall out of range | ~80% (96 golems/h, 380 iron/h) | 1 hopper 5 + piston 1 + hooks 1 + bucket 3 = 10, string | ~400 |
| L3 | **Day 1**: 3 villagers, each on its bed in its own closed 1-cell pod; the zombie in a boat; golems spawn on 1x1 floors with a lava source beside their head, so they burn where they spawn; water carries the drops to one hopper | the tripwire trick on every spawn floor: one string line (2 hooks) runs through all of them at golem knee height, so any golem powers a wooden trapdoor that hides the zombie | none: they burn in range | ~65% (78 golems/h, 310 iron/h) | hopper 5 + hooks 1 + bucket 3 = 9, string, no quartz, no piston | ~120 |
| L4 | **Fast exit**: 4 villagers; 1-wide pad strips between 2-wide channels with water across; a fall of 16+ blocks to the module's kill cell | tripwire in the kill cell | at least ~47 gt in range | ~93% (112 golems/h, 450 iron/h) | ~10 | ~600 |
| L5 | **Scaled**: N modules of L4 (4 villagers each) at a 27-block pitch, each with its own kill cell and trigger, one collection line to one chest | each module's own tripwire | as L4 | N x ~110 golems/h | ~5 + 6N | ~600N |
| L6 | **One scared villager** (the reviewer's): one villager in view of the zombie, two or more that never see it; the scared one needs the night hide, the others sleep on their own | a hide for one pair of eyes only (tripwire) | as L2 or L4 | lower attempts (10 per mark instead of 30) and that villager's panic gap: ~70% with a small pad, near L2 with a full one | ~10 | ~350 |

**A calm moment made by golems cannot start the farm.** Villagers with no sleep in the last
game day never spawn the first golem, so no wire is pressed, the zombie is never hidden and
nobody sleeps. Every layout but L1 has this deadlock, so each of their captions says that the
player hides the zombie by hand (a block in front of it) on the first night. While the farm
runs, every night has golems (their villagers slept the night before), so it keeps itself
going: in L3 the string runs through every spawn floor so that no night can pass with all of
its golems on unwired floors. Only L1, whose detector hides the zombie every night by itself,
claims to restart after a missed night, and the judge checks that claim (below).

## Decision

Superseded by the stages (revision 4): each stage's pick is redesigned against its hard limits
and chosen by its own rule (the work log, NOTES.md). The decision of revisions 1-3 stays below
as the record of the first archive (L3 day 1, L2d middle, L2d 8x8 fastest, L5 scaled):

1. **L1 first, as the bar (G2):** it is the straightforward reading of the mechanics, with a
   sensor for the night. It is measured in clear weather, and its caption says it stops in
   rain. Its numbers calibrate the judge.
2. **Day 1 = L3:** the lowest bill (9 iron: one hopper, one hook pair, the bucket; a powered
   wooden trapdoor instead of a piston), no quartz, about 120 blocks. Its kill in place costs
   rate (about 2/3 of the ceiling), which the day-1 number does not rank.
3. **Middle ground = L2 or L6,** whichever has the higher iron per hour per block among builds
   at 75% of the ceiling or more with a bill of at most 20 iron. Both use the tripwire's free
   calm moment and drop their golems out of range, which is what clears the 75% floor that L3
   cannot reach. L6 needs fewer eyes hidden, L2 makes three times the attempts.
4. **Fastest scaled = L5 of L4 modules with 4 villagers each,** ranked by iron per hour per
   module (about 450), the scaled build reported as N x the module. 4 villagers keep a cycle at
   600 gt when one of them sees the golem (93% instead of about 90%). Channels are 2 wide. The
   pitch is 27, and each module has its own kill cell and trigger. The module is searched first
   (pad strips, the drop, the trigger), then tiled.

The judge decides between estimates. Each farm is reported as a share of the ceiling.

## Must-haves (real use)

What "works as its card says" asks of every build in normal running (clear weather, the player
nearby); the rare events (lightning, natural hostiles, rain and thunder, days away, lava's fire)
are the measured tradeoffs of Stages.

- **The villagers**:
  - each in its own closed 1-cell pod on its bed (the review's choice: one cell, so neither
    the spawn centre nor the 10-block and follow boxes move);
  - every cell they can stand in within Manhattan 1 of their HOME;
  - never within reach of the zombie (its attack box with its vehicle) or of a golem;
  - no damage;
  - a roof over a pod, if any, must not become a golem spawn base: leaves (which
    LEGACY_IRON_GOLEM excludes), at villager y+6 or higher, or capped with carpet or glass. (A
    roof the sky cannot see through lowers the lightning tradeoff; it is not required.)
- **The zombie**:
  - a passenger of a boat or minecart (it never despawns and never walks);
  - its eye cell never sees the sky (it never burns) and is never in water (it never
    converts);
  - no drop ever comes within its pickup reach.
- **The golems**: no golem escapes the build, gets within melee reach of the zombie or touches
  its vehicle. Every golem dies in a kill cell within 1200 gt of its spawn. This holds for the
  spawn search of EVERY farm villager, not only those that see the farm's zombie: any villager
  panics when a natural zombie, husk, drowned or other hostile of `VillagerHostilesSensor`'s list
  comes into its sight (a dark or thundering surface), and a willing panicking villager runs its
  own 10 attempts around itself (D1U's escape in the game came from its blind plate villager's
  search reaching the surface). So the stray-spawn caps and the golem exit flood start from every
  villager's search.
- **The drops**: every iron ingot reaches the output chest (the judge's, at `port output`), is
  still in the farm's containers at the end, or is MEASURED as lost: the iron that burns (a golem
  dying where its drops fall in the lava: in a pit kill, golems bobbing in the corner's water by
  their goals' jumps, `jumpInLiquid`) is a measured tradeoff on the card, iron lost per golem,
  and the card's iron per hour counts only what reaches the chest (iteration 11, the parent's
  ruling on vjeux 08:26: "It's all about tradeoffs"; the game confirms about 1.2% on the pit
  family: 3.95 iron a golem against the loot's 4). None is left on the ground.
- **Materials**: survival only. No lava that can flow (static sources only), no water touching
  lava.
- **Duration**: it runs for days. Every villager starts sleeping at least once per game day.
- **Robustness**: over positions and rotations.

## Not goals

The judge does not check:

- how the player brings the villagers and the zombie in (reported for day 1: the villagers
  moved, from their village bed to the pod);
- chunk borders for day 1, as long as the build stays inside the simulated box (the middle stage
  fits one chunk column; the late stage's footprint is counted in chunk columns).

Weather, natural spawning and lag, not goals in revisions 1-3, are measured from revision 4 on;
from revision 8 they are tradeoffs on every card (Stages: Tradeoffs every card measures), not
refusals.

## The judge (`circuit: mob-farm iron_ingot`, crates/circuit/src/studies/mobfarm.rs)

A registered study (`StudyDef` id `mobfarm`) that runs entirely on redsim, with its kind
`mob-farm <item>` so the next farms (gold, creeper, raid...) add a product, not a new study.
The design gives its villagers, the zombie and its vehicle as `entity:` lines (the zombie as
the vehicle's `Passengers`), its beds as blocks, and `port output` (the chest).

- **Runs**: each build runs seeds x game days at vanilla speed.
  - Every seed draws the villagers' follow-range bonus, their sensor phases and every other
    entity random.
  - **The steady state**: each run starts at a random day time in the day (10..12000), with
    each villager's HOME claimed (the bed in its pod) and its `LAST_SLEPT` at the end of the
    last REST (day time 10), as in a running farm, whose villagers last fall asleep near the
    end of the night. The first night is then fully ahead of it. The D measured days still
    cover every day time equally.
  - Each golem walks under a random policy: still, or one of 8 directions at goal speed 1.0 or
    0.6.

  Metrics:
  - iron/h per block, iron/h (mean, 95% interval), golems/h, seconds per golem;
  - share of the ceiling;
  - misses by cause;
  - blocks, bill, payback;
  - the rate under the worst policy: on the first build, 9 runs (still, the 8 directions at
    1.0), every golem of a run under that one policy, all 9 on the same seed so that the lowest
    is not biased low by noise. The card shows the lowest.
- **Builds**: 4 rotations x 2 positions. They agree when every build passes and the iron/h
  means are within 20% (`Agree::Rate`).
- **Golem exit (before the runs).** The positions a golem can stand on come from a flood: it
  starts at every spawn spot the farm offers, and each step runs the golem's own physics (its
  1.4 x 2.7 box, step height 1, a jump when blocked, falls). From every such position the
  golem's drift runs under still and the 8 constant inputs, alone in the built farm. Under the
  still policy each run must reach a kill (the golem dead) within 1200 gt (no dead water: it
  catches cancelling currents, dead ends and side pockets); under every input no run may leave
  the build or break another must-have; the 8 walking inputs' kill times are reported, not
  refused (iteration 13: the AI-on runs and the game gate judge walking). A summed-current test
  (a standable top whose touched cells sum to a current below 1e-5) only refuses early. The
  flood keeps every position a run visits on the ground as a new start, until none is new.
- **Checks** (the must-haves, as it runs; a failure names what, where and when):
  - a villager outside its cell or hurt;
  - the zombie hurt;
  - a golem outside the build, in reach of the zombie, touching a vehicle, or alive after
    1200 gt;
  - a drop that comes within the zombie's reach, or is about to despawn (a drop that touches lava
    is measured, iron lost per golem, not a failure: iteration 11);
  - iron lost at the end;
  - a villager 24000 gt without a sleep.
- **The restart probe**, for a design that claims it (the header `restart: yes`, L1): a run
  that starts with no `LAST_SLEPT` must spawn a golem within 2 game days. Designs without the
  claim report "restarts after a missed night: no" and carry the first-night caption.
- **Refusals** (before any run): what the model cannot judge, below.
- `mctas circuit run --sim` gives one build's runs. `mctas circuit sim` and `verify` run the
  whole judge (`verify` on redsim too; the game checks are the one-off validations below).
  `mctas mobfarm gen ...` generates the parametric layouts (pad, villagers, modules).

## The mob layer: what is exact, what is refused

New files in `crates/redsim/src/entity/mob/`. Existing redsim files get registrations and
hooks only, agreed with the boat-leads lane (06b77372) and the coordinator.

**Exact** (the same formulas in the same order as the 26.3 source):
- the entity list's order (the tick order that decides who counts 3 at T+600);
- LivingEntity physics (travel in air, water and lava, friction, step height, fluid
  pushing, entity pushing, cramming);
- damage and invulnerability (lava, fire, wall, magma, cramming, fall, drowning);
- death, drops and removal;
- the Brain: memory slots and their expiry order, sensors and their rates, start delays,
  activities, the schedule;
- `VillagerPanicTrigger` (with its 60 gt duration), `VillagerCalmDown`, `SleepInBed`, `WakeUp`,
  `UpdateActivityFromSchedule`;
- the hostile, nearest-living (box `R + 1.0`) and golem sensors, with COLLIDER line-of-sight
  rays;
- `spawnGolemIfNeeded` (every willing panicking villager, 10 attempts each),
  `wantsToSpawnGolem`, `golemSpawnConditionsMet`, `trySpawnMob` (LEGACY_IRON_GOLEM),
  `checkSpawnObstruction`, `isValidEmptySpawnBlock`, `isUnobstructed`;
- sleeping and waking (`findStandUpPosition`), beds and `OCCUPIED`;
- the golem loot; zombie persistence; day time.

**Exact, tick for tick** (tickexact, 2026-11-08; vjeux: "We should be tick exact everywhere"):
every random the farm's world draws from is the game's own stream, seeded by the bridge from the
sandbox seed and mirrored by redsim ([mechanics.md, tick exactness](../../docs/mechanics.md#tick-exactness-the-bridges-seeds-for-mobs-sensors-and-terrain--track-tickexact)):
- sensor phases: each entity's own random from its entity number, the constructor's draws
  (UUID, yaw, every sensor's start delay) replayed; follow-range bonuses: a harness input on both
  sides for the design's mobs, the level random's triangle for a summoned golem;
- spawn columns, yaw and the finalizeSpawn draws: the level random, in the game's order
  (`SpawnUtil`, `EntityType.create`, `Mob.finalizeSpawn`), each created mob a construction;
- loot counts: the loot table's random sequence (`entities/iron_golem`, `entities/zombie`);
- item motions: the item's own random from its entity number;
- cramming rolls, ambient sounds, strolls: each mob's own random; behaviors' durations: the level
  random;
- natural spawning, the five custom spawners (siege, cats, phantoms, patrols, the trader),
  weather, thunder, fire: the level random and the trader's own seeded random.
`mctas mobfarm compare <design> [--terrain village-edge]` runs the farm in the bridge and in
redsim and stops at the first difference: a difference is a model error to fix, never a
distribution to match. The judge's own runs (no bridge) still draw a module's randoms from the
mob layer's seeded stand-in until that module's exact path is the default (the exact paths run
whenever a world mirrors the level random: `World::exact_seed`).

**Refused, never approximated** (the judge rejects a design where it could change the outcome):
- **Villager walking.** Each villager must stand in a closed 1-cell pod: the judge floods the
  cells its 0.6 x 1.95 box can reach (steps of 0.6, jumps of 1.25, falls; a wooden door counts
  as open, a climbable or fluid as passable) and refuses more than its own cell. In that cell:
  - every check holds at the corners of the region the box can take (the 0.6 box anywhere in
    the cell): the bed within 2.0 (`SleepInBed`), Manhattan 1 to HOME, the zombie out of reach;
  - the stand-up spot for both bed sides (the yaw that picks it is not modeled) lands in the
    cell.
- **Golem walking** (iteration 13: the mob AI is now modeled, 26.3's goals tick for tick, on
  main). Judged by the AI-on runs and the game gate, as every other robustness property is
  measured (vjeux 08:26), with two static guards kept from the exit flood, under every input:
  no reachable position leaves the build (an escape breaks the one rule), and no dead water
  (under the still policy every reachable position reaches a kill: a golem that stops walking
  always arrives). The 8 constant walking inputs no longer refuse a design: their kill times and
  the worst policy's rate are reported. A golem alive past 1200 gt in any AI-on run or in the
  game gate fails the build, and the card prints the kill-time tail with its sample bound ("no
  golem outlived 1200 gt in G golems": a 95% upper bound of 3 / G per golem; a golem that
  outlives its kill never despawns and, inside a villager's follow cube, stops the cluster for
  good) and the golems that targeted the farm's zombie per run (a target once seen is kept out of
  sight, `TargetGoal.canContinueToUse`, and walked toward). (Before the AI was modeled,
  a constant input stood in for it and refused every water channel a walking golem could
  out-walk; the real goals, a stroll toward a random village position or a target in sight,
  are what the runs now play.)
- **Golem targeting** (refused if a golem gets within melee reach of the zombie) and
  **zombie attacks** (refused if a villager's cell meets the zombie's attack box with its
  vehicle).
- **POI claiming** (refused if a pod holds no bed head within its cell's reach, or a bunk bed).
  The real guard is the Beds cage rule (Stages): its own bed the only free bed AcquirePoi can
  reach from the pod, since a pod villager releases and re-claims HOME at nearly every REST
  (Mechanics, Keeping HOME), now under the modeled AI.
- **Gossip and trading.** The only gossip call is `TradeWithVillager.tick` (IDLE and MEET
  packages). It needs an `INTERACTION_TARGET` from `InteractWith`: a villager within 8 in
  `NEAREST_VISIBLE_LIVING_ENTITIES`, whose test is a COLLIDER ray. Then `distanceToSqr <= 5`,
  both 1200 gt cooldowns, and 5 willing villagers. The code-true rule: refused when a villager
  can see another within 8 (an eye-to-eye ray, standing or sleeping, from the corners of its
  cell) while 5 or more villagers share its 10-block box. Clusters of 4 or fewer, or glass
  between the pods, pass.
- **Job sites and breeding** (refused if a workstation is in the build).
- Raids, bells, weather and natural spawning (not judged, listed in docs/coverage.md).

## Validation against the game (one-off checks, recorded here)

1. **Spawn spots**: three platforms, with real villagers (a `LAST_SLEPT` memory in their summon
   data) and a zombie in a boat. Every golem's spawn cell over many cycles goes into a
   histogram, compared with redsim's for the same build (chi-square).
2. **A whole farm**: L1 or L3 run for several game days in the game (the bridge's `exec
   summon`). Golems/h and iron/h are compared with redsim's mean within its 95% interval.

## The obvious design's measured numbers

(Filled in after the judge runs L1.)

## Ceiling

- **Per cluster**: one golem per 600 gt (the 599 gt memory, cleared at the start of the brain
  tick, and the multiple-of-100 attempts). That is 120 golems/h = **480 iron/h** and 120
  poppies/h at vanilla speed.
- A cluster cannot do better: every golem marks every villager within 10 blocks.
- A farm of N clusters: N x 480 iron/h.
- Below it, in practice:
  - the panic gap, when every eligible villager is in its gap tick at once;
  - the sightings: at least about 47 gt in range for a pad and a drop;
  - the failed marks of a small pad.

  The judge reports each. The ceiling is a derivation; the judge's measured numbers are the
  authority, and this section is revised if the measured best stays well below it.

## Changelog (revision 2, after the G1 review)

| point | what changed | where |
|---|---|---|
| 1 | Every willing panicking villager tries (10 x N attempts); the 4x4 pad is 82% with three villagers; tick order at T+600 (only V and later villagers count 3); the panic gap costs only when every eligible villager is in its gap tick; the citation fixed (empty entry map, test in `start`/`canStillUse`) | Who spawns; Trade-offs (spawn spots) |
| 2 | Only the villagers that see the zombie need the hide; new layout L6 (one scared villager) | Sleep; Trade-offs; L6; Decision |
| 3 | Transit in water (0.056 terminal, 0.036-0.076 with walking), 1.4-wide golems need 2-wide channels, at least ~47 gt in range, 90% with 3 villagers and 95% needs 4 | The golem; Trade-offs; L4; Decision |
| 4 | The follow box is `R + 1.0`, pitch 26.9 (pad reach + 18.84), the stacking rule (Q < 25.8), per-module kill cells or triggers | Golem detector; Trade-offs (scaling); L5 |
| 5 | The gossip refusal replaced by the code-true rule (eye-to-eye ray within 8 while 5+ share a box); 4 villagers per cluster in L4/L5 | Refused; L4; L5 |
| 6 | Golem walking: an exit current on every standable cell (static refusal), plus the rate under the worst constant policy; villagers in 1-cell pods | Refused; Must-haves; The judge |
| 7 | The flood's passability (doors, climbables, fluids) and continuous-region checks; the zombie's attack box with its vehicle (1.52 from a boat's centre); the HOME-loss refusal (Manhattan 1); L3 with closed 1-cell pods | Sleep (keeping HOME); The zombie; Refused; L3 |
| 8 | Tripwire and daylight detector cited; rain stops L1; lightning; decision: clear weather as a not-goal, an opaque-roof (sky light < 15) must-have for the villagers, L1 measured in clear weather | Calm devices; Not goals; Must-haves; L1 |
| 9 | The number per tier (day 1: lowest bill; middle: iron/h per block under 20 iron; fastest: iron/h of the build and per module); the board ranks by iron/h per block; L3's bill includes the bucket; the decision re-argued | The number; Six layouts; Decision |
| — | Also fixed: bottom slabs and 3 px trapdoors are solid (the scan stops there, the attempt is wasted); carpets are not | Where a golem spawns |

## Changelog (revision 3, after the G1 re-review)

| point | what changed | where |
|---|---|---|
| 1 | Golem exit: the static current rule is gone. A flood of the positions a golem can stand on (from every spawn spot, its own physics: 1.4 box, step 1, jumps, falls), and from each the drift under still and the 8 constant inputs must reach a kill within 1200 gt. The summed-current test (below 1e-5) is only an early refusal. The 9 worst-policy runs share one seed | The judge (golem exit, metrics); Refused (golem walking) |
| 2 | Deadlock: runs start in the steady state (a random day time in 10..12000, `LAST_SLEPT` at the end of the last REST, day time 10). L3 wires every spawn floor with one string line (no extra iron), so no night's golems can all miss the wire. Every golem-triggered layout's caption says the player hides the zombie by hand on the first night. The restart probe checks only designs that claim it (`restart: yes`, L1): with no `LAST_SLEPT`, a golem within 2 days | The judge (runs, restart probe); Six layouts (L3, the deadlock note) |
| 3 | Tiers: day 1 = the lowest bill (no quartz, at least 25% of the ceiling); middle = iron/h per block among builds at 75% of the ceiling or more with a bill of at most 20 iron (the 75% floor separates L3, capped near 2/3 by its kill in place); fastest = iron/h per module, the scaled build reported as N x the module | The number; Decision |
| note | The roof cannot be a spawn base: leaves (excluded by LEGACY_IRON_GOLEM, sky light 14 under them), or at villager y+6 or higher, or capped with carpet or glass | Must-haves |
| — | L3's sight blocker is a powered wooden trapdoor: its bill drops to 9 iron (no piston) | Six layouts; Decision |

## Changelog (revision 4, the stages)

| point | what changed | where |
|---|---|---|
| 1 | Three stages from what the player has (day 1: wood/stone tools, a few iron, a village, no Nether; middle: iron gear and the Nether, no End; late: everything, bred villagers), each with hard limits, its own ranking rule and must-haves, every line argued from the 26.3 code. They replace the tier rules | The number; Stages |
| 2 | Common to every stage: the real-game archive gate and the page's viewer running the judge's run; real terrain with natural spawning, the weather cycle and lightning, on Hard; villagers never reached, converted or struck; the restart after a missed night; no caveat lines | Stages: Common |
| 3 | Day 1: at most 10 iron with the tools counted (bucket, hoppers, hooks, shears for leaves, an iron pickaxe for redstone dust), at most 1 lava source, 3 villagers and beds from the village beside it; ranked by blocks placed + dug among designs at 100 iron/h or more; it starts by itself the first night, its zombie survives the day, the village cannot break it | Stages: Day 1 |
| 4 | Middle: Nether-tier, at most 64 iron, one chunk column, about 1,000 blocks placed + dug; ranked by iron/h then blocks; the restart after days away (modeled), 2 h of storage, poppies never block, modular | Stages: Middle |
| 5 | Late: any item, every module at 90% of its ceiling packed against its neighbours (the couplings from the code), one AFK spot (simulation distance 10); ranked by iron/h per chunk column with the scaling shown; 8 h AFK, lag reported | Stages: Late |
| 6 | Every card reports the stage lines (iron with tools, lava sources, materials by stage, blocks placed and dug, villagers moved, chunk footprint, storage hours, the absence restart, lag, density and scaling) | Stages: What every card reports |
| — | The Decision of revisions 1-3 is kept as the record of the first archive; weather, natural spawning and lag leave the Not goals | Decision; Not goals |

## Changelog (revision 5, after the G1 review of revision 4 by 8146b8db)

| point | what changed | where |
|---|---|---|
| 1 | Each stage now shows that it can be met: 4 candidates each, within the hard limits, with a calm moment that does not come from golems, estimated on the stage's own number, and the pick argued in the brief. Day 1's pick D1 (the bed plate: a blind villager lying down at nightfall lets go of a wooden plate, which shuts a trapdoor across the scared villager's sight) passed the whole judge on redsim: 243 iron/h, 51%, 8 iron, 1 lava source, no quartz, restarts by itself. No redstone dust is possible on day 1 (the bucket and the hopper leave 2 iron; a pickaxe is 3) | Stages: Day 1, Middle, Late (Candidates) |
| 2 | The absence: both kinds judged (unloaded and reloaded: `refreshBrain` -> `updateActivityFromSchedule`, the save codecs, fresh sensor delays; loaded but not ticking), each for a return by day and at night | Stages: Middle |
| 3 | Natural hostiles: a static refusal (no hostile spawn cell within reach, no path to a villager's attack box, none within its sensor distance in sight during the calm), the terrain track's flood; the game gate runs with natural spawning on | Stages: Common |
| 4 | Lightning: a copper lightning rod whose bolt box meets nothing of the farm, or every villager, vehicle and drop out of every reachable bolt box and nothing flammable under the sky; the bolt's 26.3 reach (x ± 3, y - 3..+9, through roofs; boats and drops destroyed) from the infra track's reading | Stages: Common |
| — | Lava's fire (`LavaFluid.randomTick`): no flammable block within its reach. The archived L2d chamber holds its lava with fence gates (flammable), so the middle module is fixed with signs | Stages: Common; Middle (M1) |
| — | The weather numbers corrected with the infra track's reading: rain about 15.8% of the time, thunder 9.1%; darkening 3 (rain) and 5 (thunder) at noon; precipitation only under the open sky; sunburn stops at a darkening of 3 under the open sky | Stages: Common |

## Changelog (revisions 6-8)

| rev | what changed | where |
|---|---|---|
| 6 | The Late stage ranked by iron per hour per block of the whole build at N = 8, shown at N = 1, 2, 4, 8 and rising with N; the volume per module must fall; the coupling model (the spawn's 10-box, the follow cube, D / 200, golems immune to falls, about 250 gt to die) and its calibration; candidates LB, LS, LP, LH (G1: 8146b8db, f1a42990) | Stages: Late |
| 7 | The 90% per-module floor becomes a reported number (the parent's ruling: coupling losses already show in iron per hour per block) | Stages: Late |
| 8 | vjeux 08:26, "You can't expect day 1 farm to pass everything. It's all about tradeoffs": hard limits only define a stage (day-1 items; Nether-tier; any survival item); one rule for every farm (it works as its card says, verified in the real game); every robustness rule of revisions 4-7 (terrain and hostiles, weather, lightning and the rod, lava's fire, the restart after a missed night or days away, storage, lag) becomes a measured tradeoff on the card; each stage ranks by what its player values: day 1 by the minutes to a full iron kit (build time + (build iron + 24) / rate + expected losses), middle by iron per hour for an evening's build (at most an hour) less a day's expected losses, late by iron per hour per block at N = 8 less 8 h's expected losses. Caveats (tool gaps) stay banned; tradeoffs are required. Candidates re-scored with the measured numbers: day 1 D1 (about 30 min) over the archived L3 (about 54 min, 34 of them lava trips); middle M2-open (33-min build, 439 iron/h, its chamber fire-safe by geometry) over M2; late LS-open first | Stages; Must-haves; Not goals |

## Changelog (revision 9: the stage specs and the optimizer loop)

vjeux 10:48-10:56: the farms come from a search, the spec session sets each stage's constraints and
objectives, the optimizer (farmopt, [farm-optimizer.md](farm-optimizer.md)) answers with merged
Pareto fronts and their bounds, the spec session judges and revises. Each iteration is a row: what
the spec was, what came back (the front's size before and after merging), what changed and why.

| it | spec (designs/farms/iron/*.spec) | what came back | what changed and why |
|---|---|---|---|
| 1 | Day 1: min T (minutes to a 24-iron kit), max net iron/h; day-1 items, at most 8 villagers (the village's), 1-2 modules, village-edge terrain; fork redstone (none, dust). Middle: max net iron/h over a game day, min build minutes; Nether tier, 2-4 modules sharing a part, at most 120 build minutes. Late, at N = 8: max net iron/h over 8 h AFK, min build minutes, min volume per module; stacking must pay (5% per block and per volume over N = 1). One build-minutes formula; net iron per hour over each stage's window W (0, a game day, 8 h); timing parts from the catalogs; merge 10% | (running) | From revision 8: each stage's single scalar becomes a 2-3 objective vector with the player's reason for each; the middle gets N >= 2 sharing a part and catalog timing parts (vjeux 10:48); the late per-block rank becomes a printed ratio beside rate, build and volume (per block alone prefers a cheap 70% module to a 92% one that brings 30% more iron a night); M2-open leaves the middle (one module, a hand-built pulse); the hand candidates become seeds |
| 1b | the same objectives and constraints | G1 APPROVE (8146b8db, f1a42990) | After the approval: late.spec's comment corrected (the reviewer's note: the best iron/h per build minute is on the front, the per-block best is listed beside it when it falls off); the merge written as greedy around the best (no twins on either side of a fixed box's edge; still no chaining); a gap over 10% is printed with its cause and worked on in the next iteration instead of holding the presented set (vjeux 10:59: "I want your process to be autonomous"); the card adapts to the stage and the farm (vjeux 11:17): each stage's shown facts, in order, as default rows (the page-copy lane's format, staged in designs/farms/iron/cards.tsv until the site reads them), lava trips and iron up front only where the player cares (day 1); the day-1 hide pulse's window is L >= 20 gt with no hard upper bound (answered to the pulse lane: the zombie's ray is cast only by the hostile sensor, once per 20 gt list); the re-check (8146b8db, the parent's rule): a pick replaces the live page when it passes the game gate and beats the live farm (more than 10% better on one objective, not more than 10% worse on any), the search keeps closing any gap, a gap is never printed on a page, nothing waits on vjeux; parts searched on their own (vjeux 11:23), the late collection holding the night |
| 2 | Day 1's T measured: from the first block at dawn of day 1 until the chest holds bill + 24 iron; the farm stands complete after the build minutes (day time 1200 x minutes), the villagers arrive willing (their village's `LAST_SLEPT`), the zombie parked in its boat; the judge's terrain run, mean over seeds | D1U's first full terrain day (the day-1 lane, seed 3): 13 golems, 50 iron, against about 84 at D4's steady 252 iron/h; D1U in terrain 153 iron/h (32%): golems by day only, the sleeping villagers' ray to the zombie runs through the natural rock floor (air in the void), so they never wake scared at night | T = build + (bill + 24) / rate assumed the steady rate from the last block on; the start-up (willing villagers, the first calm, when in the day the build ends) is real for a fresh player and is what T is for. Objective 2 stays the steady rate. The T run performs every hand step the card prints (the first-night hide at the first REST start, at least 20 gt: G1, 8146b8db), so a golem-triggered calm cannot leave a seed's T infinite. The day-1 lane prints build minutes flat, villagers left and hostile break days ("never", or the channels that reach a villager, the zombie or its boat: no invented rate) |
| 3 | Build minutes place, dig and GET every item at the stage's prices (designs/farms/prices.tsv, seconds per item per stage, each row argued from its recipe and the stage's tools): day 1 glass 7 s (sand, fuel, half of a furnace's 10 s: 2 furnaces), an iron ingot 30 s (caving with a stone pickaxe), a hopper 154 s, the tools the build needs once (bucket 90 s), filler 1 s and free for every dug block, the zombie 180 s; middle (iron tools, 8 furnaces, the Nether, a day-1 farm's iron) and late (farms for everything) cheaper; lava 120 / 60 / 30 s and villagers 180 / 180 / 60 s by stage. Day 1's T stops at 24 iron in the chest (the build's iron is mined in the build minutes) | vjeux 12:01 on d1-bed-plate: "this amount of glass is def not accessible day 1" (98 glass; D1U uses 5) | Placing 2 s "gathering included" priced a glass block like a dirt block and an iron ingot not at all; at day-1 prices d1-bed-plate's 98 glass alone add about 11 minutes. The terrain pricing caps stray spawn cells with the cheapest block at the stage's prices (a torch or button on top stops the golem's scan as glass does) |
| 4 | Every hard limit is a real fact about the player or the game, one line each in the spec files: day 1 the day-1 materials, the village's 8 villagers, real terrain by the village (vjeux 04:22); middle the Nether-tier materials, more than one module (vjeux 10:48), one AFK spot; late any item, one AFK spot, the world's build height. Out: middle's 120 build minutes, its 4-module maximum and its shared part; late's comparison at exactly 8 modules, its 5% stacking gains and its 8 h of storage; day 1's 1-2 modules (the 8 villagers already allow at most 2 clusters). The copies bar is printed beside the middle and late fronts instead of being a limit; N is searched up to 16 (a search boundary: past the sharing's period each added module repeats the last one's numbers) | vjeux 12:18: "You just invented the two hours build limit. This is not a real restriction"; farmopt's run 1 middle front was cut at exactly 120 minutes (#1: 4 modules, 1716 iron/h at 120 min, under run 1's old effort formula) | A cap the player never chose hides the farms past it; build minutes is already an objective, so the front itself shows what each extra hour buys, and the number of modules is the player's choice. Some of the removed limits were the parent's earlier stage constraints, treated the same. With N free, a farm is its stacked unit (the G1 review): builds that differ only in copies of one unit are one farm, compared at its own N (the smallest N whose iron/h per build minute is within 10% of its best on the curve), by per-module numbers in middle and late (iron/h per module, build minutes per module, and late's volume per module) and at one unit on day 1; the card prints the curve over N. Previews stopped (vjeux 12:18: "Just upload them to the website"): presented farms go to the site through the coordinator |
| 5 | Every price is the item's full chain at its stage, tools included: stone is smelted cobblestone (no silk touch on day 1: 6.5 s), stone slabs 3.25 s, cobblestone and its slabs filler; leaves a tree trip with shears (2 iron, once; without shears they drop saplings); a lightning rod 3 copper; stone tools in hand on day 1. Material roles (designs/farms/roles.tsv): a design says what each block is for, the cheapest block that does the job at the stage is placed and named, the judge checks its behaviour | vjeux 13:09: "We use leaves to prevent mob spawning but to get leaves you need sheers for which you need iron. It uses stone but you need silk touch or smelt which you're not going to do day 1" | The table priced a placed stone block as filler on the assumption the player builds it of cobblestone; the design must say cobblestone instead, and stone costs its smelting. Roles let a day-1 farm cap stray golem cells with a cobblestone bottom slab (0.5 s: its top face is not full, so IronGolem.checkSpawnObstruction and a hostile's isValidSpawn both fail) or a button on top where the scan must reach the pad, instead of glass (7 s) or leaves (2 s and shears) |
| 6 | Every price per realistic action, not per block: lava = trips (120 / 60 / 30 s) + a fill per source (5 / 3 / 2 s), a trip carrying the buckets the player has (day 1 one, an extra bucket 90 s where it saves a trip; middle and late a stack of 16); villagers by the cheapest route per stage: 180 s moved from the village on day 1; 120 s by minecart or boat in the middle, or bred at the farm (the pair moved, 30 s of actions per further villager, the code's game time where it outlasts the build: a pair breeds only by day, calm and out of the zombie's sight, at most 2 births per pair per game day, 275-324 gt to a birth, a free reachable bed before each birth, 24000 gt to grow up, the pod sealed after the baby walks in: the G1 review); 30 s delivered by a breeder late; water 1 s a source (a refill). The copies bar and every seed are re-materialized by roles before they are compared again | vjeux 13:56 on the middle copies bar (m2-open-ring at middle prices): "What takes 38min to build?"; its breakdown: 4 villagers 12.0 min, 199 stone 6.3, placing 365 blocks 6.1, 4 lava 4.0, 99 glass 3.3, the zombie 2.0, 2 hoppers 1.7, the rest 2.8 | A source of lava is not a trip once the player carries a stack of buckets, and a middle player breeds villagers or carts them on paths they have; the bar still carried the stone and glass of before the roles (iteration 5), which the roles replace with cobblestone and slab or button caps |
| 7 | An open lever, recorded until the judge models it: hostile suppression around the farm (role hostile_suppression: the cells from which a spawned hostile can come into a golem's sight or touch it, or within 8 of a calm-critical villager and into its sight, lit to block light 1 or capped), weighed against the thunder-day loss once the game gate attributes it and the judge runs golem targeting with natural hostiles (mob AI 8a0201dc). The golem's targeting restated (G1, correcting an error carried since the first review): acquisition needs line of sight (forCombat conditions, a COLLIDER ray eye to eye), mustSee false only keeps an acquired target, contact targets 1 time in 20 | D1U's thunder days in the game: about 58 golems/h against 63 predicted; thunder darkens the surface so more hostiles spawn; vjeux 14:04: "Seems legit that iron golems go after mobs?" | Legit game behaviour (IronGolem.registerGoals, Monster.isDarkEnoughToSpawn), so it is a tradeoff the search should weigh (a few torches against the loss), not a model gap to hide; but which channel loses the golems (a golem drawn off by a hostile it sees, or the blind villager kept panicking at REST by one it sees) is attributed in the game gate before torches are priced, and only cells in sight count, so a dug-in farm pays for no torch its rock already covers |
| 8 | Day 1, run 1 judged (farmopt, 7,195 builds in village-edge, the terrain judge's measured T): the front is 2 members after merging, the T end proven (gap 0%); its #1 and the hand seed D1U are one farm by the merge rule (T 31.3 vs 32.1 min, 255 vs 254 iron/h). Asked: the space grows on the rate end (the drop + blade kill with the bed plate, two clusters within the village's 8 villagers, 5 villagers in a cluster); role `fire_clearance` (nothing flammable within any lava's fire reach, natural cells dug at the spot); the escape seal on the whole D1U family before anything is published; the day-1 card row counts lava trips (per action), not sources | Front: #1 T 31.3 min, 255 iron/h (golems burn in the villagers' sight, pit 13 below the beds); #2 T 34.4 min, 308 iron/h (out of sight, 19 below; +21% for 3 more minutes); the rate end 308 against a 479 bound (35.7%, 5,394 builds open: the space is 1 module, 3-4 villagers, pits only). #2's fire line: its short grass burnt in 2 of 96 runs, then lava flowed in. The thunder-day loss of D1U: a golem escape in the game (8-server trace), not golem targeting | The T objective rewards what the player wants and the search confirms the hand design is T-optimal; the rate end shows what the space lacks, not the farms' limit; a fire break fixed for seconds and an escape that breaks the one rule are fixes, not card lines. By the brief's own rules at these numbers #2 alone would be presented: it beats the live farm (T far lower, iron/h 308 vs 295) and beats #1 too (21% more iron, only 9.9% more time), and #1 beats no presented farm by 10% on any objective; the set is decided by those rules once the escape seal and the fire clearance have moved the numbers (G1 note). fire_clearance's reach as the code walks it (G1): through any block outside #blocks_lava_fire_spread (= #blocks_motion), each step +1 in y and up to 1 sideways in x and z, fire only in an air cell with a flammable neighbour; the terrain pricing computes the clearance from that walk |
| 9 | Must-have restated: no golem escapes from ANY farm villager's spawn search: every villager panics at a natural hostile in its sight and then summons around itself, so the stray-spawn caps and the golem exit flood start from every villager's search, not only those that see the farm's zombie; a poppy price row (a sorter's named filler) | The day-1 lane's root cause of D1U's game escape: the flood and the caps started only from the villagers scared of the farm's zombie; the golem came from the blind plate villager U1's search (U1 panics at a natural zombie on a dark day), whose floors reach the surface north of the farm. Fixed on the day-1 lane's branch (D1U one layer deeper, slab caps; 253 iron/h, T 33.0 min) and in farmopt's grammar | A rule the judge must enforce for every farm, not a D1U detail: any farm villager is a potential summoner |
| 10 | The Farms section lists only presented farms and the variations named here as promising; a bar is never listed, only quoted as the reference line; a stage with nothing that beats its bar lists nothing. Off the site: late stack-8x43 and stack-2x43 (the copies bar), middle four-modules-one-timer and its tight version (worse per module than one module), row-2x27 (443 iron/h per module for more blocks and build per module than two copies; its 18 h of storage is no middle objective, the middle window being a game day), m2-open (dominated by m2-open-ring: the same rate, more blocks, a hand-built repeater pulse), day-1 d1-bed-plate (its real-game gate passes only in the void; in terrain with the mob AI on it fails, its drops burning). m2-open-ring moves to Components as the middle module (the unit of the middle and late farms and of the copies bar), not a middle farm; Middle and Late list nothing until a search result beats copies (empty sections hidden, no placeholder line: our process stays off pages, vjeux 10:10). The archived iron-day1 stays the live day-1 farm | vjeux 14:51 on farms/iron/stack-8x43.html: "Sounds like a stupid design, why does each module have their own kill chamber?"; the parent: "never a bar" | The stacks were put up by a lane outside this loop: the spec already called copies the bar. The late search must find sharing the coupling allows (one shaft, one kill chamber and one collection fed by every module, each golem's fall kept out of every other module's follow cube; a zombie and calm shared by 2-4 clusters) |
| 11 | Burnt drops are a measured tradeoff, not a failure: the judge's "a drop touches lava" check becomes a measured line (iron lost per golem), the card's iron per hour counts only what reaches the chest, and a kill that loses none competes on the objectives. Spot pricing against the village's own villagers: no rule until a measured run shows coupling | The day-1 lane: the pit family's burnt drops come from golems bobbing in the corner's floor water (their goals' jumps, jumpInLiquid), dying anywhere from the floor to about 3.6 up; the AI-on judge burns drops on d1-bed-plate in 4 of 8 seeds; the game: D1U 3.95 iron a golem (about 1.2% lost); lava higher stalls the farm, a corner pillar pins golems on the hole's corner. At D1U's spot the village's villagers are at least 47.8 blocks from any farm villager (no 10-box coupling) | The parent's ruling on vjeux 08:26 ("It's all about tradeoffs"): about 1% of the iron for a design that is otherwise the T-optimal day-1 farm is a number a player weighs, not a refusal; the blade kill of day-1 run 2 competes with it on T and the rate |
| 12 | Middle run 2 judged: nothing to present. "Where a middle farm can win" added to Middle (the module is at 92% of the ceiling, so a middle farm beats copies only on build minutes per module, at most 90% of theirs at 90% of their rate, by sharing the kill chamber and collection with almost free transport). Asked of run 3: current prices and roles (iron-v5), the copies bar priced the same way; families that share the kill and collection (iron-v5's shared-kill stack, the ironfarm lane's shared chamber, modules around one central shaft and chamber with every pad outside every foreign follow cube); candidates that cannot beat the bar pruned before their runs; the low-build end measured first; a sight blocker between pods so 5-villager clusters pass the gossip rule; one calm per module where routing a shared one fails | 230,419 builds (iron-v4: pre-roles materials, the old lava and villager prices), 3,000 quick runs, 4 full judges, 62,066 left open. Front 2, merged 1: two copies stacked 43 apart with 10 x 10 pads, 454 iron/h per module, 42.8 build minutes per module, against the copies bar's 444 and 31.8 under the same pricing: 2% more iron for 35% more build. The best per build minute (4 modules in a row sharing one calm, a pit kill) only ties the bar (420 iron/h per module at 30.1 minutes). About 130,000 candidates refused by the gossip rule (5+ villagers in a 10-box with sight), 25,758 by a shared calm with no route | The front's member is the bar's own shape (copies at a no-coupling pitch), so the middle section stays empty; run 2's space lacked the families where sharing can pay, and its build minutes are stale (glass and stone priced, one lava trip per source, 180 s a villager), so its numbers are not compared with run 3's |
| 13 | Golem walking is judged by the AI-on runs and the game gate (a golem alive past 1200 gt in any run or the gate fails the build; the card prints the kill-time tail), with the exit flood's static guards kept under every input (no escape) and under the still policy (no dead water); the 8 constant walking inputs are reported, not refused | The ironfarm lane's shared kill: its water channel carries AI-driven golems (N = 2 at pitch 27: 880 iron/h, 92%) but fails the drift check under a constant walking input (recorded by the ironfarm lane: two westward constant walks from the pad cells ended at the channel's first step, the golem held at the step's lip against the side walls, 1.2 above the water's feet level; no still-policy failure, so a golem that stops walking is carried over the lip). Under the new rule the AI-on runs measure the same weakness: the channel's kill averages 671 gt with the longest at 1,107 gt, close enough to 1,200 to fail now and then, while drop blades over one shared items channel (the drops only) kill in 304 gt, longest 381, at 873 iron/h; so the blades are the shared-kill default, and the dropped refusal hid no hold the measurement misses; farmopt: that check alone blocks every family with horizontal golem transport to a shared chamber (the central-shaft ring) | The constant inputs stood in for walking AI before it was modeled (the brief's "not modeled as AI; instead ..."); now that the goals run tick for tick, a refusal on an input the real AI may never sustain contradicts vjeux 08:26 (robustness is measured, never a refusal by itself) and hides the families where sharing can pay. The game gate stays the backstop where the AI-on model still differs from the game (m2-open-ring reads about 5% low with the AI on, under investigation) |
| 14 | Every promising candidate goes up as a variation of its stage, the stage's pick first, through `mctas site variation` (which runs the real-game gate) and the coordinator's fast path; candidates carry their live site URL; previews are retired (also in AGENTS.md's house rule). Every candidate that passes the gate is listed, dominated ones included (vjeux 16:49: "Seriously, just upload them"); only bars (plain copies) stay off. New tradeoff line "Beds": the nights a villager spends homeless and the rate's spread across seeds | vjeux 16:27: "I asked you to stop using previews and push to the site directly, what's wrong!?"; the mob-AI lane (0 differences on the bridge): pod villagers release their bed at nearly every REST and re-claim it through AcquirePoi; m2-open re-claims every night (110 golems/h on every seed), four modules packed tight often fail (78 to 423 golems/h over 6 seeds) | Iteration 10's "list nothing unless it beats the bar" left vjeux nothing to look at; his word is to see every promising farm, near-duplicates still merged (10:56). A rate that swings with the seed is a property of the design a player must see, not noise for the mean to hide; the AI-on start-state fix (villagers arrive without HOME, as in the gate) closed the 5% gap of row 13. G1 points folded: the one rule restated for publishing (up before the gate; nothing about it while it runs; a pass adds the real-game line; a failure stays on the page with its counts, printed, not held, the parent's ruling on vjeux 16:49; the card shows the game's measured numbers wherever the game and redsim disagree, either way, and the gap is a model bug to attribute; the archive still needs a pass); Mechanics "Keeping HOME" rewritten from the code (the release ignores distance; any unreachable walk target arms it; a pod villager releases HOME at nearly every REST and AcquirePoi re-claims it), the reviewer's correction of its own revision-1 point; the POI-claiming refusal points at the Beds cage rule |
| 15 | Day 1, run 2 judged; the Day 1 set takes shape: the pick = the day-1 lane's d1u-deep (the out-of-sight pit farm, ground 11) once its terrain gate passes; farmopt's run-1 #2 (the same idea, dug 7) is re-judged before it is listed or merged: its card counted 2 surface covers where main's every-villager and cell+1 rule needs about 323 (`mctas mobfarm spots`: 66 placed + 323 covers + 10 seals), so its T of 34.4 min is stale; D1U (in sight) a variation, the fastest armour; a two-cluster "farm for the week" once the search finds its best form. Asked of run 3: two clusters from 12 up to >= 27 apart (the 12-16 cap of runs 1-2 was no real limit: only the village's 8 villagers and the spot's terrain price are), run-1 members carried into later fronts re-judged under the current rules, both forks (none and dust, the archived day-1 falling-edge pulse) | Run 2 (10,916 builds, AI off): #1 the D1U family (T 31.9 min, 252 iron/h; T's bound gap 4.9%), #2 two clusters 14 apart (T 60.1 min, 396 iron/h); blades and 5-villager cages off the front. D1U passed the real-game gate in terrain (64.1 golems/h against 63.9). D1U-deep: T 34.6, 312 iron/h. Farmopt's #2 in the real game: 88 golems/h against its AI-off card's 78. Mob AI: pod villagers re-claim their own bed every night after the release, after a reload too; a bed is lost only to another homeless villager that can path to it (shared or open pods) | Two clusters 14 apart sit inside each other's follow cubes (396 = 2 x 198, each at 63% of a lone farm), so two copies at the no-coupling pitch (about 2 x 312 for about the same T) are the form to measure before the week farm is presented; the dust fork's first runs (420 and 284 iron/h on two seeds, identical sleep logs) are a spawn-luck spread for the full judge to bound |
| 16 | Vertical travel and height priced (Common, "Vertical travel and height"; prices.tsv's `travel:*`, `place:edge` and `villager:*` rows, and rows for the access's blocks: ladder, cobblestone stairs, scaffolding, soul sand): every trip between the surface and a work level pays its travel both ways at the game's movement speeds by the farm's cheapest access, up and down by different means if cheaper (stairs, a ladder, a pillar, scaffolding late, a soul-sand bubble column middle and late, elytra late; down, a drop into water); the access is built in the design, priced by its blocks and judged by every check, its top hatch the one block allowed on the natural surface; trips are the inventories of blocks, the lava trips, each villager moved (it travels too), the zombie, the hand steps and each collection (day 1's T includes the walk down to its 24 iron); a block more than 2 above every standable surface pays the sneak-edge placement. Every terrain farm builds below the surface: no block on the natural surface but the access's top hatch, every villager's golem search below it, and the chute to the side (the kill and the output beyond 18.84 blocks of every villager, fed by a water channel cut through the rock; the golems may drop out of the follow cubes first, since a slow channel inside them resets every villager it passes) | vjeux 17:59: "Building horizontally is much easier than vertically"; 18:00: "Even digging down a lot is annoying because you have to go down and up way farther"; 18:02: "Don't put slabs, do it below the surface. And put the chute to the side". The speeds from the code: climbing 0.1176 blocks/tick up and 0.15 down (LivingEntity); a bubble column, inside water, about 0.48 blocks/tick up for a player holding jump and 0.275 for a riding villager (the column's +0.06 a tick against travelInWater's x 0.8 and -0.005; the 1.8 and 0.9 caps apply only above a column's top), and at most 0.34 down; a free fall into water about 0.06 s a layer over a long drop; walking 4.317 blocks/s on stairs; a pillar's jump-and-place cycle 10 gt (G1 corrected the bubble speeds and asked that the access be built in the design and judged by every check, not priced alone) | A block 40 up was priced like one on the ground and a chest 28 down like one at the surface, so stacks and deep pits looked cheaper than a player finds them; depth now trades against the surface rule (d1u-deep's ground 11 against farmopt's #2's 323 covers at 7, which the surface rule now refuses), and the stage searches re-rank on these prices |
| 17 | The cage gets its own objective vector (Common, "The cage's objectives"): under the four rules that make it work (the calm by day and night, Beds, the every-villager rule, restart after days away), minimize the irregularities a builder notices (pods of differing shapes, villagers outside one row or a 2 x 2 block, extra villager levels, the zombie's window not in front of the watchers, the plate villager's pod out of the row, its plate not beside the window's trapdoor), the bounding-box volume, and the blocks and build minutes with travel and height. A more regular cage replaces a farm's cage when the farm stays within 10% on every objective of its stage (the merge rule: the same farm; the G1 review), its card from its own judge and game gate; irregularity counts merge only when equal; the best cage per stage goes into d1u-deep, the Farm module and the + and X candidates, and onto the cage component page. The + or X arrangement is decided on the measurements | vjeux 18:26: "Okay you figure it out. Also please optimize the villager room, the current layout is super weird". d1u-deep's cage today: two villagers on beds at layer 0, a 3 x 5 x 2 air space beside them, the plate villager's bed perched at layer 2 with an oak pressure plate, the zombie off to the side in its boat | A cage was searched only on what it changes in the farm's numbers (build seconds, sight lines, spawn centres), so any layout that worked scored the same; a player builds from the picture, and a cage that reads as random costs them time and mistakes the build minutes do not count |
| 18 | Measured since iteration 17, recorded: (a) the surface rule refuses natural-surface blocks only (caps and seals in caves or rock are below the surface; farmopt's run 3 had refused cave caps too, which made the bed-plate family look infeasible above 13 deep); (b) a regular day-1 bed-plate cage (its plate villager in the watchers' row, on their level) was not found in the cage space searched so far: a top-half oak trapdoor as the hide, the zombie in its boat on a slab ahead at about the watchers' level, the plate at the plate villager's feet in the row or one level up, driving only the trapdoors it touches; the space still open is lower zombies (in a pit ahead, which lowers the ray at its end: the ray runs from the watchers' eye, feet + 1.62, to the zombie's, boat y + 1.23), fence gates as the hide (1.5 tall when closed, empty when open), and the plate strongly powering the block under it, whose six neighbours it powers too (no hide may open a walkable path out of a pod); iteration 17's cage search settles it: a regular day-1 cage found, none in that space, or one costing more than 10% on T; D4 (3 irregularities) stays until then; the dust fork, which has its redstone line, gets the regular row cage; the middle and late module takes the regular cage (module-two-watchers: 0 irregularities against 3, the same 408 iron/h, +7% build); (c) the side chute (vjeux 18:02): dropping the golems to the follow cubes' floor and then sideways keeps the rate (335 iron/h against d1u-deep's 312) but its first build costs 22 more minutes to the armour (a 4 x 4 arm and four hoppers); a shallow arm inside the cubes keeps 80% of the rate; its parts (arm width and length, one hopper) are searched in day-1 run 5 with travel priced; (d) the + (four modules on one cross channel) equals copies from radius 21 (444 iron/h per module, 6% fewer build minutes): the same farm by the merge rule until travel and height separate them | farmopt's runs 3-4 and part judgements on main's AI-on judge; the day-1 lane's check of D1U and d1u-deep (their 26 caps at world y 55-58 under a surface at about 72) | The surface rule as written was right, but its implementation over-refused; the chute numbers are measured, and the day-1 cage's open space is named so the cage search covers it rather than taking a narrow search for an impossibility (the G1 review) |
| 19 | The parent's decision (vjeux may overrule it): the chute to the side is a family every search must include and every stage lists, not a refusal; a drop under the pad competes on the objectives, which price its depth in every trip. Day 1 keeps one pick per value of its redstone fork (a fork, not an objective: a player has redstone or does not; redstone is priced 0 only in the dust fork, so the two fronts' T are not like for like): the no-redstone pick stays d1u-deep; the dust pick is the best member of the redstone fork's own front, never compared with the no-redstone pick: the dust farm with the regular row cage, its kill moved to 21 below the beds (outside every follow cube; on d1u-deep's terms 409 iron/h, T 42.9 min, 31 s a collection, "needs 45 redstone dust"), once its real-game gate passes and unless another redstone member on the same terms beats it by the rule; both are listed and each card says plainly whether it needs redstone and how much, in a player's words (e.g. "needs 12 redstone dust"); this is the parent's decision, replacing its 20:51 "switch once" wording. A kill 19 below the beds lies inside the follow cubes of a villager whose follow range draws above 16.3 (about 35% of draws: the sensor lists a golem whose feet are within R + 2.7 below a villager's feet, with no sight test), which made 1 of 12 positions fail; at 21 below it clears every draw. The bed-plate calm has no regular cage (settled by the second outcome of iteration 17's search), so D1U's cage stays in the bed-plate farms | vjeux 18:00: "Even digging down a lot is annoying because you have to go down and up way farther"; 18:02: "Don't put slabs, do it below the surface. And put the chute to the side". Equal terms: d1u-deep 320 iron/h, T 38.7 min, 31 s a collection; d1u-drop-side 326-328, T 48.4-49.1, 40 s. The dust farms with the regular row cage (1 irregularity against 3), AI on, travel priced: 301 iron/h at T 39.2, 367 at 39.8, 445 at 43.4. farmopt's day1-rooms enumeration: 946 zombie positions pass the rooms and in none does a watcher's standing ray cross a cell the plate powers (D4 reproduced as the positive control) | The side chute served the 18:00 point, and on equal terms it does not deliver it (a longer walk to the chest, 25% longer to the armour for 2.5% more iron); with travel priced into T, his concern sits in the objective itself. The dust farm (+15% iron at +3% time against d1u-deep) is the dust fork's pick, not a replacement for the no-redstone pick: its T leaves out the dive for redstone ore that the fork assumes done (the G1 review) |
| 20 | Every part gets an objective vector and hard rules at every stage (Common, "Every part's objectives and hard rules"): the cage, the calm, the pad and its caps, the transport, the kill, the collection, the storage, the access and the containment; objectives: blocks placed, blocks dug, build minutes with travel and height, compactness, regularity (iteration 17's counts generalized, distinct block types included) and the part's measured effect in a reference farm; bounds printed where they exist, each labelled with its family beside the part's bound over the stage's materials (a lava kill 240 gt, fire charges about 190 middle, splash Harming II about 80 and falling anvils about 20 late; a gravity drop out of the cubes about 20 gt, a slime push about 11 middle and late; a mark share of 1; one hopper's 9,000 items an hour, enough up to about 16 late modules; the floods' minimal cut; 8 hours of storage); the calm keeps iteration 17's working rule, golems by night a measured line; part fronts kept per interface; farmopt searches part fronts per stage and assembles from them; every listed farm and component member passes the trim (`mctas mobfarm trim`) and takes the regular parts within 10%; storage becomes a proper system (a sorter for iron and poppies, the iron in a horizontal row of double chests on a hopper line at the player's level, a crafter packing iron blocks in the middle and late stages when it pays; capacity per block, build minutes, seconds per collection, regularity; capacity against the stage's AFK hours a measured line) | vjeux 20:59: "All of them look super unoptimized. Can you make sure to optimize every single part of the system during the night." | Searching whole farms left each part as the generator first wrote it; a part front per stage, measured in a reference farm, finds the cheapest part that keeps the farm's numbers, and the trim removes what no check needs |
