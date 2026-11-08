# Search demand: what people search about farms and redstone

vjeux asked on 2026-11-07 which Minecraft searches about farms and redstone are the most popular,
to pick what the site covers first and what its pages are called. The answer is measured with
Google's own tools by `trends` (crates/trends): the ranking is
[search-demand/ranking.md](search-demand/ranking.md) (generated; the raw measurements are in
[ranking.tsv](search-demand/ranking.tsv), the list of searches in
[terms.tsv](search-demand/terms.tsv)).

`trends` is the one tool that uses the internet. Design work never does
([process.md](process.md)): do not use it to look for designs or mechanics.

## Headlines

From the 2026-11-08 ranking (past 12 months, worldwide web searches, `minecraft iron farm` = 100):

- **Farms are searched far more than redstone.** Every search with "minecraft" and "farm" adds up
  to ~1,300, every search with "minecraft" and "redstone" to ~300.
- **The farms people look for:** XP farms (122), iron farms (100), mob farms (90) and "auto /
  automatic farm" (89), then a step down to gold, creeper and spawner farms (~35), villager farms
  and trading halls (~30), tree, chicken and sugar cane farms (25 to 29). Decorative farm builds
  are big too: barn (61), farm house (23). The 1.21 farms barely register yet: armadillo 11,
  trial chamber 0.7, breeze and wind charge ~0.
- **Redstone searches are mostly about components**, how to craft or use one block: TNT (240),
  rails (162), piston (115), hopper (97), crafter (83), dispenser (71), observer (69), dropper and
  repeater (43), comparator (38), lever (31).
- **The most searched redstone builds:** elevators (85, mostly water elevators), the orbital
  strike cannon (34), autocrafters (29), chunk loaders (28), storage and sorting systems (28),
  bedrock breakers (26), secret doors (23), item sorters (22), piston doors (21), flying machines
  (21), auto smelters (19). Redstone computers (2.8), 3x3 piston doors (2.3) and logic gates
  (1.7) are niche.
- **Tools around builds:** Litematica (128) and the Create mod (128) are as searched as the top
  farms; command blocks reach 164.

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
  double quotes must appear as that exact phrase (`minecraft "create mod"` leaves out "how to
  create a mod in minecraft"). Every term says "minecraft", so other games' farms (Fortnite XP
  maps, WoW gold farming) stay out, and a term whose words also mean something else is narrowed
  ("minecraft calculator" is mostly enchantment and portal calculators, "minecraft buttons"
  controller buttons).
- **Squeezed batches are measured again.** A term far bigger than the anchor (`minecraft farm`
  is ~15x iron farm) squeezes the anchor to a few points and its batch with it; every smaller
  term of such a batch is measured again among small terms only.
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
- Rows overlap: an umbrella row (`minecraft farm + minecraft farms`) contains the rows under
  it, and `minecraft piston` contains `minecraft piston door`.
- Web searches only. YouTube searches (`--youtube`) rank builds higher: YouTube's autocomplete
  for "minecraft redstone" starts with builds, door, tutorial, computer, farm, contraptions.

## Running it again

```
cargo build --release -p trends
./target/release/trends rank --terms docs/search-demand/terms.tsv \
  --anchor 'minecraft iron farm' --low-anchor 'minecraft trading hall' --refine-below 5 \
  --cache /tmp/trends-rank-cache.tsv --title 'Minecraft farm and redstone searches' \
  --out docs/search-demand/ranking.tsv --md docs/search-demand/ranking.md
```

- 163 requests for the 218 terms, 8 minutes with `--delay-ms 2000` (one HTTP 429, waited out).
  Google Trends answers HTTP 429 to datacenter addresses, so run it from a home connection (it
  needs only `curl`); a stopped run resumes from `--cache`, and a rerun after editing terms.tsv
  only asks for the new terms.
- New candidates: `trends suggest --alphabet 'minecraft redstone'` (`--youtube` for YouTube's
  autocomplete) and `trends related 'minecraft piston'`; add them to terms.tsv with a kind.
- `--geo US` (one country), `--time 'today 5-y'` (another range) and `--youtube` change the
  scope; the anchor and the low anchor must stay terms of the list.
