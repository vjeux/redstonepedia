# Search demand: the builds people search for

vjeux asked on 2026-11-07 which Minecraft searches about farms and redstone are the most popular,
then to keep only builds we can do, not single items: farms and redstone machines. The answer is
measured with Google's own tools by `trends` (crates/trends): the ranking of 149 builds is
[search-demand/ranking.md](search-demand/ranking.md) (generated; the raw measurements are in
[ranking.tsv](search-demand/ranking.tsv), the list of searches and their kinds in
[terms.tsv](search-demand/terms.tsv)).

`trends` is the one tool that uses the internet. Design work never does
([process.md](process.md)): do not use it to look for designs or mechanics.

## Headlines

From the 2026-11-08 ranking (past 12 months, worldwide web searches, `minecraft iron farm` = 100):

- **Mob farms lead:** XP farms (122), iron farms (100) and mob farms (90), then gold (36), creeper
  (35), spawner (34), raid (21) and zombie (17) farms.
- **Elevators** are the most searched machine (85), mostly water elevators (27; redstone
  elevators 2.4).
- **Villagers:** villager farms (34), trading halls (30), villager breeders (17).
- **Crops, animals, blocks:** tree (29), sugar cane (25), kelp (20) and wheat (19) farms; chicken
  (27), cow (14) and bee (13) farms; lava farms (21) and cobblestone generators (19).
- **Machines:** the orbital strike cannon (34), autocrafters (29), chunk loaders (28), storage and
  sorting systems (28), bedrock breakers (26), item sorters (22), flying machines (21), auto
  smelters (19), quarries (13), TNT cannons (12), netherite farms (8.9, TNT tunnel bores), TNT
  dupers (7.6).
- **Doors:** secret and hidden doors (23), piston doors (21), redstone doors (16), 3x3 doors
  (7.2); "3x3 piston door" itself is 2.3 and "2x2 piston door" 0.8.
- **Circuits are niche:** redstone clocks (12), NOT gates (6.3), timers (4.2), T flip-flops (3.0),
  redstone computers (2.8), logic gates (1.7).
- **The 1.21 farms barely register** except armadillo farms (11): trial chamber 0.7, breeze and
  wind charge ~0.

## What counts as a build

Kept: every farm and machine a player builds (farms of mobs, villagers, animals, crops and
blocks; doors, elevators, sorting, crafting and smelting, TNT and flying machines, circuits,
traps, utilities). Left out: single blocks and items (TNT, hopper, piston, observer, rails...),
catch-all searches ("minecraft farm", "minecraft redstone builds", "auto farm", "afk farm"),
decorative builds (barn, farm house), mods and tools (Litematica, the Create mod), ores and
mechanics (redstone y level, quasi connectivity), and "diamond farm" (diamonds are mined, not
farmed). Netherite is in: TNT tunnel bores farm ancient debris.

## Method

- **Google Trends gives relative interest, never search counts.** A comparison holds at most five
  terms and Google scales it to its own busiest week (= 100), so two comparisons cannot be read
  side by side. Every request of `trends rank` therefore holds the same anchor,
  `minecraft iron farm`, plus four other terms, and a term's score is its mean weekly interest
  divided by the anchor's in the same request: 100 = as searched as `minecraft iron farm`, 50 =
  half as much. Means of the weekly values are finer than the rounded averages Trends shows.
- **A term counts every search that contains all of its words, in any order** (Trends' search
  terms): `minecraft iron farm` also counts "how to make an iron farm in minecraft bedrock".
  `a + b` counts searches for either (`minecraft xp farm + minecraft exp farm`), and words in
  double quotes must appear as that exact phrase (`minecraft "and gate"` leaves out "minecraft
  fence and gate"). Every term says "minecraft", so other games' farms (Fortnite XP maps, WoW
  gold farming) stay out, and a term whose words also mean something else is narrowed
  ("minecraft calculator" is mostly enchantment and portal calculators).
- **Squeezed batches are measured again.** A term far bigger than the anchor squeezes the anchor
  to a few points and its batch with it; every smaller term of such a batch is measured again
  among small terms only.
- **The long tail is measured against a smaller anchor.** Every term under 5 points next to
  iron farm is measured again against `minecraft trading hall` and chained through that term's
  own score: finer steps for the rarest searches.
- **Candidates** come from what players build and ask about, Google autocomplete of
  "minecraft redstone", "minecraft auto" and "minecraft automatic" with every letter and digit
  (`trends suggest --alphabet`), and Trends' related queries (`trends related`).

## Caveats

- Scores are relative interest, not counts. Trends samples searches: under ~5 points a score
  moves by a point or two between runs, and a term with too few searches reads 0.
- Searches without the word "minecraft" ("iron farm bedrock") are not counted. That holds for
  every row, so the order holds, but the shares of the biggest farms are understated.
- Rows overlap: `minecraft piston door` contains `minecraft 3x3 piston door`, `minecraft elevator`
  contains `minecraft water elevator`, `minecraft crop farm` some of the crop rows.
- Web searches only. YouTube searches (`--youtube`) rank builds higher: YouTube's autocomplete
  for "minecraft redstone" starts with builds, door, tutorial, computer, farm, contraptions.

## Running it again

```
cargo build --release -p trends
./target/release/trends rank --terms docs/search-demand/terms.tsv \
  --anchor 'minecraft iron farm' --low-anchor 'minecraft trading hall' --refine-below 5 \
  --cache /tmp/trends-rank-cache.tsv \
  --title 'Minecraft builds people search for: farms and redstone machines' \
  --out docs/search-demand/ranking.tsv --md docs/search-demand/ranking.md
```

- The first run (218 searches, before the builds-only cut) took 163 requests and 8 minutes with
  `--delay-ms 2000` (one HTTP 429, waited out). Google Trends answers HTTP 429 to datacenter
  addresses, so run it from a home connection (it needs only `curl`); a stopped run resumes from
  `--cache`, and a rerun after editing terms.tsv only asks for the new terms.
- New candidates: `trends suggest --alphabet 'minecraft redstone'` (`--youtube` for YouTube's
  autocomplete) and `trends related 'minecraft piston'`; add them to terms.tsv with a kind.
- `--geo US` (one country), `--time 'today 5-y'` (another range) and `--youtube` change the
  scope; the anchor and the low anchor must stay terms of the list.
