# REDSTONEPEDIA (the website)

REDSTONEPEDIA lists ONE world-best design per kind of build (vjeux 2026-10-20 21:47Z: "I want to
have ONE world best implementation of each of these instead of a bunch of working but mid
implementations"): one 3x3 piston door card, one clock card, one card per circuit kind. Live:
https://vjeux.github.io/redstonepedia/ (the `gh-pages` branch; the repository vjeux/redstonepedia was
vjeux/minecraft-tas until 2026-10-21, and GitHub does not redirect a renamed project's Pages URL).

## One design per kind (`crates/site/src/kinds.rs`)

- **Circuits**: the kind is what a builder asks for (a gate's function with its mirror, a timer's
  restart rule, a sand elevator, the precise TNT duper; else the kind's first word). A
  non-parametric kind keeps one design: robust first, then fewest blocks, then the smallest box,
  then the study's speed metric, the A-first version of a mirror pair (vjeux 2026-10-21 13:11Z:
  "sort by blocks then volume"); the kinds vjeux asked by volume rank by the box first, then
  blocks (`kinds::by_box`: piston extenders, via pext 11:55Z, "the smallest VOLUME per reach in
  any 3D shape"; TNT dupers and shulker loaders and unloaders, via the coordinator 13:55Z; not the
  sand elevator; other layouts of one extender fold under its smallest box); a TNT duper's box
  counts the air it needs under it (the coordinator 15:18Z: its footprint times the lowest height
  it survives at, a stand declared higher, or a precise drop's blast depth plus one; the card says
  "fits in 4×4×4 + 6 blocks of air under it", the needs line whether its stand is the lowest safe
  one); the gates with their own
  levers (the basics) are dropped. No design is picked by name (the coordinator 2026-10-21 06:56Z:
  new records must need no site change): where builders ask for more than the smallest, `RULES`
  narrows the kind first and the ranking picks among those (shulker loaders and unloaders: tiles
  side by side and keeps sending boxes out with its chests kept full; every TNT duper: has an
  on/off lever, the coordinator 14:08Z, an on/off switch being a property and not a kind
  (`..., switchable` axes fold into their kind), the self-running one also safe at its own stand
  height, `stand: N` or the ground, its card saying "stand it N blocks up" (the site reads the
  design's `stand:` when the judge records none: it does only above the safe drop); beside each
  pick its best design without a switch ("no on/off switch, N blocks smaller") and, when the pick
  stands higher, the best one that sits right on the ground, each a page linked from the pick's,
  no card; the two-way flying machine: has an on/off lever; the BUD switch: notices a pure block
  update and resets itself, vjeux 11:46Z: "a single observer is not a BUD switch"), its card saying
  so ("the smallest of 16 tested that tiles side by side"); a rule nothing meets is a build note.
  `MENTIONS` adds one line beside a kind's pick (the BUD: the latched 2-block classic, the single
  observer). A parametric kind (reach, count, length, period, a lock's dials) keeps the best archived
  design of every size: one card (its smallest design, or the size builders know: `CARD_ROWS`,
  the 3-dial lock), and on each of their pages a table of every size.
- **Clocks, one card per mechanism** (the coordinator 2026-10-21 11:33Z, vjeux at 19 ticks: "The
  first 10 clocks look all the same to me?"; `crates/site/src/mechanism.rs`): in every period the
  clocks whose designs, or whose pictures at their thumbnails' still frames, differ only by
  swappable blocks (a note block, a lamp, a copper bulb, a redstone block, a piston, and every
  toggled component a relay can be: trapdoors, doors and fence gates of any wood or metal,
  powered and activator rails, crafters, bells, a dispenser or dropper holding nothing; an
  observer powering nothing), turned or mirrored, are one mechanism: the smallest (blocks, then
  box volume; a family's clock first among equals) keeps the card ("also works with trapdoor,
  powered rail": every wood or metal one word), the others keep their pages, listed on its page
  with their downloads, and leave the Clocks tab and the time search (`host`). The families' clocks
  fold too (vjeux 13:09Z: eleven observer rings that only swap their relay were eleven cards a
  period). A layout-independent key folds the same loop built another way (the coordinator
  13:55Z, "the entire class"): an observer ring with one relay per loop step (`ring_key`: its
  sequence of observers and relays), a hopper loop with comparators reading it (`hopper_loop_key`:
  the loop, which hoppers are read, their items); both conservative, anything else folds only by
  its layout. `mctas site mechanisms --dir site --period 19` lists a period's cards and what folds
  under each; without `--period`, one line per period (`--before <saved run>`: what changed); the
  smoke checks no period lists two cards of one mechanism, family members included.
- **Clocks hard to wire** (the judge's `usable` column: `yes: repeater` / `comparator` /
  `observer`, or `no: <why>`; webrender's `clock::usable`): a clock with no tap says so in a plain
  warning on its page (never hidden) and a "hard to wire into a circuit" chip on its card (never
  summed up over a list); it comes after the easy ones of its period on the Clocks tab and in the
  time search; a mechanism's card is a clock that can be wired when one can (its hard ones listed
  under it, marked); a clock with a tap says how to take its signal ("To use it in a circuit, put
  a repeater where the view puts its lamp"). The smoke checks the warning and the order.
- **Doors, one record per mechanism** (vjeux 13:10Z): two board leaders that are one mechanism
  (the same key) with the same numbers fold into one record leading all their boards; the smoke
  checks no two rows of the records table are one mechanism.
- **Lists sort by blocks placed, then the box's volume, then speed** (vjeux 13:11Z): the cards of
  a period, the time search, the all-builds list, the doors' records table, the "also works with"
  lists; a per-size table keeps its size order. The one-per-kind picks rank the same way, but by
  the box first for the kinds vjeux asked by volume (`kinds::by_box`: extenders, TNT dupers,
  shulker loaders and unloaders).
- **Flying machines** (vjeux 2026-10-21 00:31Z): by engine shape, one design each, no size table:
  a flat machine (engine side by side, 1 block tall), a tall machine (engine stacked; its cargo
  capacity, from its `extend n` header or its `port payload` lines, is a property) and a flying
  machine with docking stations (there and back); the other one-way machines (up, down, trains)
  are not published. The one-way picks start with their own lever (the coordinator 16:10Z: a
  property, not a kind; the judge's `start flip the lever`), the best machine without one kept
  beside each ("no start lever, 1 block smaller", a page linked from the pick's); the docking
  pick has its on/off lever.
- **Piston extenders flipped back early** (clk-misc's `flips` metric; the coordinator 2026-10-21):
  a design that strands blocks when its lever is turned off too soon says so in its page's
  warning; a size whose pick is not safe keeps that size's best flip-safe design beside it ("safe
  to flip back any time, N blocks bigger", a page linked from the pick's), like the TNT dupers'
  no-switch variants.
- **Crop farms** (w86's `harvester <crop>`, one card per crop; vjeux 2026-10-21 19:03Z): a farm's
  efficiency is the items it gathers an hour for each block placed, at the game's normal growth
  speed (random tick speed 3). The pick of each crop is the most efficient (then the smallest
  box); the card leads with it ("0.53 sugar cane an hour per block · 21 blocks · 11.2 an hour in
  all, at normal growth speed"), and the page's opening says it plainly. The judge's own `items per
  hour per block` and `items per hour` are read when they are there; until then they come from
  its `items per hour per cane` over `per cane` blocks (`html::farm_rates`).
- **Iron farms** (ironfarm's `mob-farm iron_ingot <tier>`, one card per tier; vjeux via the
  decisions inbox 2026-11-08): the day-1 tier's pick costs the least iron, the middle ground's
  gives the most iron an hour for each block placed, the fastest's the most for each group of
  villagers (then the fewest blocks: `kinds::mob_farm_rank`); the fastest pick (one module) and
  its tier's scaled build (several copies) link each other. A tierless mob farm (the brief's
  obvious design, the bar) is not published. Their pages say the farms are tested on our
  simulation of the game's mob rules (the caption gives the game check), that the view runs them
  with that same simulation (the judge's first run, tick for tick: [webrender.md](webrender.md),
  mob farms), where the chest goes, and the day-1 farm's first
  night by hand. Their titles read like a crop farm's (`html::circuit_props`' `mob-farm` arm and
  `mob_farm_title_tail`): "295 iron an hour · 62% of the ceiling · 189 blocks · 1.6 per block · 9
  iron · golems burn in lava pits", the last words from the design's `how:` line
  (`Circuit::feature`); each farm holds its own chest at OUT, so the view shows a chest there.
- **Doors**: one card, the best fully seamless door (the seamless board's record: vjeux 23:44Z,
  "This is what people usually consider to be piston doors"); every board's record keeps its page
  (no card) and every record's page has the table of records.
- **2x2 doors** (clk-item's `door-size: 2`, designs/piston-door-2x2/archive with the same caches;
  the coordinator 2026-10-21: their own boards, not mixed with the 3x3 records): a card of their
  own after the 3x3 card, its page's table holding the 2x2 records only. Their boards are the
  2x2's (WR.md: smallest, no slime, fastest); the community regulation's checks (seamless tiers,
  frame, record eligibility) are the 3x3's and are not shown on a 2x2 page. A 2x2 door named like
  a 3x3 one fails the build (one page per name).
- **Clocks** (vjeux 2026-10-21 01:24Z: "show all of them with a card grouped by timing"): the
  Clocks tab shows every clock as a card, under one heading per period (ascending, smallest first
  within a period, nothing folded; each heading links its period and counts its clocks; the card
  leads with its blocks and parts), with a jump bar on top (landmark periods with their counts,
  every period in a list): every archived clock (exact twins folded) and every family member a
  judge verified (tested in the game, or simulated to exactly its period), which is a clock of its
  own with a page, a 3D view and downloads ("extended (n = k)"; its page links its tested base and
  says what each step adds, from the family's rule); a clock page's breadcrumb is its period. On
  the all-builds list the clocks come after the circuits. Each tab has its own opening line.
- **No extendable family pages** (vjeux 2026-10-21 00:33Z: "All these variations are useless,
  remove them"): no family pages, no member lists, no circuit members, no "comes in every size";
  the smoke checks no family page is written or linked and every member design file is an
  extended clock's.
- Everything else is not published at all (no card, page, design file or search hit); the files
  stay in the repository. `mctas site kinds` prints the plan (every kind, its pick or its rows,
  how many designs it leaves out).

## Farms (`crates/site/src/farms.rs`)

The farms have their own top-level section (vjeux 2026-11-08 12:14: "a new farms top level
category, and you can drill down with iron farm and then full farm / individual components"),
the fourth tab: Doors · Clocks · Circuits · Farms. Inside, a farm is still a circuit build of a farm
study (`site_model::FARM_STUDIES`: `mobfarm`, `farm`), and its folder is what it produces
(`site_model::farm_folder_of`; vjeux 13:14: "Melons and sugarcane are two different farms"):
`farms/iron/`, one folder per crop (`farms/sugar-cane/`, `farms/bamboo/`...), melons and pumpkins
together (`farms/melon-pumpkin/`; vjeux 13:16: they grow the same way, one farm grows either). The
index's `kind=farm` lists them, and `kind=circuit` no longer does. Their old pages
(`circuits/<study>/`, `farms/crops/`) redirect to the new ones (`published::resolve`).

- `farms/index.html`: the farm types, the iron farm first, then the crops as players search for
  them. `farms/<crop>/index.html`: a crop's farms, each kind's pick first. A crop's card can carry
  its twin's yield (`designs/circuits/farm/cards.tsv`: the melon farm planted with pumpkins).
- `farms/iron/index.html`: the iron farm's hub. **Full farms** by stage (Day 1, Middle, Late), every
  variation of each stage on the site (vjeux 12:15: "ALL the promising variations"), the stage's
  pick first (a variation picked in its `variations.tsv`, else the archived design), then by iron
  an hour. **Components**, once the optimizer's catalog has members: one page each.
- **Variations**: `designs/farms/<farm>/<stage>/` (stage `day-1`, `middle` or `late`) holds each
  design (`<stem>.txt`, the judge's line on top), its judge row (`verdicts.tsv`, the archive's
  format) and `variations.tsv` (`file`, `pick` = `yes` for the stage's pick, `name` = a
  player-facing name for its card, else its `how:` line, `held` = why it is off the site: a bar's
  or a takedown's alone). Add or update one with `mctas site variation <design>... --stage <stage>
  [--pick] [--name "..."]`: it runs the kind's judge on redsim (a design it fails is refused), then
  a mob farm through the same real-game gate `circuit archive` runs (`circuit::mobfarm_game`: the
  26.3 server's builds, or terrain mode in the sample's kept world for a design with
  `terrain:`/`sample:`; about 2 minutes), and writes the three. What the gate measured goes in the
  row (ironspec's brief rev 9 iteration 14, the parent's ruling on vjeux 16:49, "Seriously, just
  upload them"): a pass adds its `real game` line; a failure is no refusal: where the game's rate
  falls outside the card's interval (either way) the card's golems and iron an hour become the
  game's, and what broke in its builds becomes `game tradeoffs` lines with their counts ("a golem
  stays alive past 1 min in 1 of 8 builds"), said on the page whatever its curated card says.
  `--no-gate` puts it up on its redsim judge alone, the gate left for later; its page says nothing
  about the gate. The site shows every variation listed in `variations.tsv` unless it has a `held`
  reason. `mctas site variation --regate [design...]` judges and gates the listed variations again
  and rewrites their rows the same way (a held one is left alone). A page that comes off the site
  redirects to `farms/iron/`. Commit the folder; the next deploy shows it, at
  `farms/iron/<stem>.html`.
- **Components**: `designs/farms/<farm>/parts/<part>/` (part `module`, `cage`, `spawn-area`, `kill`,
  `collection`, `sleep-trigger`) holds each member alone (`<stem>.txt`, no judge) and
  `members.tsv` (`file`, `name`, `stage`, `metrics` as `key value, key value`, `used_by` = the full
  farms' stems, space-separated), best first, written by the farm optimizer. A member whose file
  is not in the part's folder is a judged design of its stage (`designs/farms/<farm>/<stage>/`,
  gated like a variation): a farm module (ironspec 2026-11-08: the unit the bigger farms copy), its
  numbers read from its verdict (iron an hour, blocks, build minutes, villagers) when `metrics` is
  empty. Each part with members gets `farms/<farm>/parts/<part>/index.html` (each member's 3D
  view, its numbers and the full farms built with it), and each full farm's page lists the members
  it is built from.
- **Bars**: a judged design that is never listed (the copies of one module a stage's farms must
  beat) goes in `designs/farms/iron/bars/` with its `verdicts.tsv` row; a curated card quotes it
  (`{field@stem}`), as it can quote a stage's held variations.
- Curated cards (`designs/circuits/<study>/cards.tsv`) and the internal-names check apply to every
  farm page, the hub and the components' pages included.

Everything is Rust: `crates/site` (the generator, `mctas site ...`), `crates/site-model` (the data
model, the query engine and the card markup, shared by both sides and unit-tested natively) and
`web/site` (the filtering UI: Rust compiled to WebAssembly with wasm-bindgen; its own cargo
workspace so the main build never compiles it). The only JavaScript is what wasm-bindgen
generates and the one-line `import init ...; init();` each page uses to start a bundle.

## Commands

| command | what it does |
|---|---|
| `mctas site publish` | THE command after merging into main: `build` (all defaults) + `deploy`. No browser end-to-end gate (vjeux 2026-10-20 14:20Z: "Please remove the e2e tests"); the smoke below is an optional dev tool |
| `mctas site preview <design>... [--name <slug>]` | shows vjeux a farm until /farms/ is live on the main site (vjeux 2026-11-08 12:20: "Just use the preview while the full thing works"; [Previews](#previews)), from any lane's own branch: its page, 3D view, card and downloads at `https://vjeux.github.io/redstonepedia-preview/<slug>/<file>.html`; prints the URL and the seconds from the command's start to the page answering |
| `mctas site build --out site/` | exports `site/data/site.json`, one page per build (`doors/<stem>.html`, `clocks/<stem>.html`, `circuits/<study>/<stem>.html`) and per extendable family (`families/<card>.html`), the boards page, the design files (`designs/...`) with a Litematica schematic next to each (`<stem>.litematic`, [litematic.md](litematic.md); one that cannot be written fails the build), generates every family member with the family's own `extend generator` and checks it in redsim, builds the UI into `pkg/`, copies webrender's 3D viewer (`web/viewer/` of the checkout by default; `--viewer <dir>`, `--viewer-ref <git ref>`, `--no-viewer`) into `viewer/`, checking that the bundle was built from the checkout's viewer sources (its `SOURCE`, written by `mctas web bundle`: a fingerprint of crates/mcview-web and every crate it reaches, redsim among them): a warning by default, a failure with `--fresh-viewer`, which `site publish` always sets (the parent 2026-11-08: a page whose 3D view does not run is worse than a late page; rebuild the bundle on the deploy's commit); `--pkg-ref origin/gh-pages` reuses the deployed UI instead of building it, only when its `pkg/SOURCE` fingerprint matches the checkout's `web/site` + `crates/site-model` (else the build fails: rebuild with the wasm32 target and wasm-bindgen-cli 0.2.129) |
| `mctas site serve --dir site/ [--port 8000]` | local preview (static server on 127.0.0.1) |
| `mctas site smoke --dir site/ [--url <deployed site>]` | headless Chrome (`--dump-dom`) loads the index, a time search, a facet link, a door page and a family page and checks the DOM the WebAssembly UI built against the engine's own answers; two probe pages screenshot a door and a clock thumbnail through the 3D viewer (WebGL2 in SwiftShader) and count colors (a render has thousands, the fallback one); `--url` checks the live site. `mctas site publish` refuses to deploy without it (`--no-smoke` overrides) |
| `mctas site smoke --dir site/ --engine webkit` | WebKitGTK (the engine family of iOS Safari) through WebKitWebDriver + MiniBrowser in an Xvfb display of its own (CentOS: `dnf install webkit2gtk3 webkit2gtk3-devel xorg-x11-server-Xvfb`), desktop 1280x900 and phone 390x844 at device scale 2 with an iPhone user agent: the index's first thumbnail and the live views of a door, a clock, a circuit, a family page, two entity clocks, a TNT duper, five shulker machines (a loader, an unloader, the combo, the fastest loader and the fastest unloader) and a BUD switch must become ready and draw a real picture (colors >= 40, mean luma >= 25, at most half black), the console clean, and the picker / family swaps keep the viewer. It caught the 2026-10-20 black-texture bundle (86-100% black on every view) |
| `mctas site smoke --dir site/ --engine webkit --page <path> [--page ...] [--frames 6 --interval 700] [--click <label> ...]` | films pages in WebKit instead of checking: screenshots of each page every `--interval` ms, desktop and phone, into `$MCTAS_WEBKIT_SHOTS` (default /tmp/mctas-webkit-shots), with each frame's colors and brightness in the view, how much of the view changed since the previous and the first frame, and the console; `--click` presses viewer buttons by label first, in order (`IN` = a shulker machine's start lever; `1×` `2×` `5×` = speed 20×) |
| `mctas site chrome` | downloads Chrome for Testing's headless shell into ~/.cache/mctas (what the optional smoke uses) |
| `mctas site read --dir site/ [--kinds] [--page <path> ...] [--details]` | a built page's visitor text in reading order, as plain lines (headings `#`, list items `-`, table rows `|`; no view, scripts or design text; `--details` adds the folded In depth / measurements sections): `--kinds` reads every kind's card page (the doors' card and each circuit kind's), for a player's-eye review of the whole site in one file |
| `mctas site dominated [--all] [--hide]` | the circuits a builder has no reason to pick: another design of the same kind and main value is smaller (fewer blocks, and smaller by its kind's own ranking: the box for the kinds ranked by box) and at least as fast on the study's speed metrics, at least as robust, and the listed one's only advantage is a material restriction (it avoids slime, honey, torches, pistons, copper bulbs or sculk sensors that the smaller one uses); a design with any other property the smaller one lacks (its own button, 1 wide, a badge, a different output, an extendable family) is not listed. Three lists: strictly slower with a material, the same speed with a material, and nothing in its favour. `--all` includes the designs `crates/site/hidden.tsv` already keeps off the site. `--hide` appends one hidden.tsv line per design of the "nothing in its favour" list (`dominated: <winner> is N blocks smaller, same speed`) under a dated header, again until nothing more is dominated (a design folded under a hidden one stands on its own), never a design the site keeps (a pick, a size, a variant) nor one the file already has; rebuild mctas after it (the file is compiled in). `mctas site build` prints the same audit as notes at every build (`dominated (...)`: check, then add to hidden.tsv), so a re-archived study's new candidates show up without anyone remembering to run it. A family's base design (`extend` headers) counts as extendable, a builder's property: it is never listed |
| `mctas site deploy --dir site/ [--enable-pages]` | one orphan commit of the folder, force-pushed to `gh-pages` (temporary index: the working tree and branch are untouched; a rejected push is retried). The push carries a transport commit beside it (the same tree with the live site as parent, on `refs/mctas/gh-pages-transport`, never cloned) so git sends each changed file as a delta against its live version: a viewer-only change pushes 1.7 MB instead of 12.9 MB (every page's asset tags change); `--dry-run` prints both sizes. `--remote <path>` deploys to a local mirror (no Pages build to follow) |

Needs: `rustup target add wasm32-unknown-unknown` and the wasm-bindgen CLI of the version
`web/site/Cargo.toml` pins (`cargo install wasm-bindgen-cli --version 0.2.129 --locked`); the
build checks both. `--no-wasm` skips the UI, `--pkg <dir>` takes a prebuilt one. `smoke` uses
Chrome for Testing's `chrome-headless-shell` (`--chrome`, `$CHROME`, or unpacked in `~/tools`).
Validate the HTML with the Nu checker: `vnu --errors-only --skip-non-html site/`.

### Previews

**Until /farms/ is live on the main site** (vjeux 2026-11-08 12:20: "Just use the preview while
the full thing works", after 12:17's "Please stop with the preview mode. Just upload them to the
website."): use previews to show vjeux farms; then the main site takes over, designs going there
through the coordinator.

`mctas site preview` (`crates/site/src/preview.rs`). To show vjeux a design you are working on (vjeux 2026-11-08: "push farms to the site in less than
a minute. I want to be able to see them in order to iterate with you"), run, from your own branch:

    mctas site preview designs/circuits/mobfarm/tracks/ironfarm/d1-bed-plate.txt [more designs] [--name <slug>]

and send him the URL it prints. No merge, no review, no coordinator GO and no tests: a preview is
work in progress for the owner to look at. Publishing to the site itself stays the coordinator's
`mctas site publish`.

- **What it shows**: the site's own build page for each design as it is in your checkout, archived
  or not, judged or not: the 3D view running it (your checkout's `web/viewer/` bundle: the mob farms
  run their redsim farm), the card's numbers, the design file and its Litematica download. The
  numbers are the archive's verdict when the same structure is archived; else the kind's own judge
  on redsim, exactly `mctas circuit sim` (cached in the site cache's `preview` domain: a design
  costs its judge once); else none (a kind judged only in the game says so). A judge slower than
  `--judge-timeout` (25 s) does not hold the page: it goes up without numbers, then again with them.
  Circuit designs only (a `circuit:` line).
- **Where**: its own repository, [vjeux/redstonepedia-preview](https://github.com/vjeux/redstonepedia-preview),
  whose GitHub Pages an Actions workflow deploys on every push (checkout, upload-pages-artifact,
  deploy-pages: no shell step, and no 10-builds-an-hour cap, which the site's branch builds have).
  `<slug>/<file>.html` per design (`--name`, default the first design's file name); the index
  `https://vjeux.github.io/redstonepedia-preview/` lists every preview with its card's status, its
  time, the branch and commit it came from and its design's hash and commit. The viewer, textures
  and stylesheet are stored by content hash (`viewer/<hash>/`...), so every preview keeps the viewer
  it was made with.
- **Never the site**: a preview never touches `gh-pages`, `crates/site/published.tsv` or the site's
  Pages build budget, and the site never links to it. The no-dead-links rule is the site's, not the
  previews'.
- **Labelled**: every preview page leads with one plain status line: "Preview: numbers from our
  simulator; not yet checked in the real game" (it passes its kind's judge on redsim), "Preview: a
  published design; its numbers are the site's" (archived), or why it has no numbers yet
  (`preview::status_words`, `pages::Preview`, `pages::claims_game`). This is the status of
  unpublished work, not a caveat on a published page.
- **Site pages**: `--site-dir <a site build> --site-page <path>...` previews pages of a local `site
  build` instead of designs (a door's, a clock's, any page's new wording, for the owner to read
  before it is published): the page as that build wrote it, a banner linking the live page, its
  design files and other links the live site's.
- **Out of search**: `noindex, nofollow` on every page, `robots.txt` disallows everything, no
  sitemap.
- **Nothing internal**: the repository is public. Every text file a push writes and its commit
  message are checked for internal names first (`preview::internal_names`: company host and tool
  names, session ids, diff numbers); a hit refuses the push and names it (take it out of the
  design's comments).
- **Cleanup**: a new preview under a name replaces the old one; every preview removes the previews
  older than `--older-than` days (14) and the shared files nobody uses any more; `--clean` alone
  prunes. Two lanes pushing at once: the second is rebuilt on the first's commit (never a force
  push).
- **Time**: measured from the command's start to the page answering 200 with its new content
  (the command polls it and prints the seconds): 25.9 s for the first preview (it uploaded the 11 MB
  viewer and textures; the workflow took 21 s), 34.9 s for two designs never judged before (their
  redsim judges and cold render 19.6 s, the push 1.9 s), 46.2 s for three never-judged iron farms.
  A design already judged renders in under a second, so a re-preview costs the push and the
  deploy, about 20 s.
- `--out <dir>` writes the preview as it would be served instead of pushing it (for `mctas site
  smoke --dir <dir> --engine webkit --page <slug>/<file>.html`); `--no-wait` returns after the
  push; `--setup` created the repository and turned its Pages on (once).

### Build time: the simulation cache (`crates/site/src/cache.rs`)

Every simulation a build needs (a clock member generated and simulated, the circuit judge's run
of a design, the 3D view's session on a circuit, where a circuit's demo stops, a clock's chart and
lamp check) is a pure function of the design (`crates/site/src/simulate/`: one file per domain,
`members.rs`, `judge.rs`, `view.rs`), cached by content under
`~/.cache/mctas/site/<domain>-<code>/` (`$MCTAS_SITE_CACHE=<dir>` elsewhere, `=off` none): the key
hashes the design's text, the parameters and the code that computes it (build.rs hashes each
domain's crates: members `mctas redsim clock clock-cli` + the generator's own crate, circuits
`mctas redsim clock circuit`, the view `redsim mcview`; and each domain its own file of
simulate/ with `mod.rs`, so a change to the view's code keeps the member pass).
An unchanged design costs nothing; a change to a simulator recomputes what it touches; wording
edits elsewhere keep the cache (phase labels and sentences are written by the pages, not cached).
Failures are never stored. The build's last line says what was reused and computed, with each
domain's computed thread-seconds and its slowest computations; the 4 most recently used code
versions of each domain are kept. Every thumbnail's still frame (the viewer's
own search, `mcview::tools::still_attr`, webrender's `data-still`) is found here too, in the view
domain: 23 s cold for 4417 designs, nothing warm. The litematics carry their design's archive
time, not the build's: an unchanged site rebuilds byte for byte but for site.json's generation
time, so a deploy sends only what changed.

What a cold build costs (2026-10-21, devvm, load ~18; the coordinator: a publish had taken
1458 s): the keys hash whole crates, and those crates change all day (of main's 225 commits that
day, 26 changed the members' code, 69 the circuit runs', 22 the view's; narrowing the keys to
files would have kept the cache at 1 or 2 of them: the core files change too), so a publish after
a merge is often cold. Cold, the build was 1217 s: the clock members 168 s (the slowest single
member, clk-item-dropring-1d-r1 n = 426, 162 s), the lamp checks and stills 26 s, and the
circuits 1172 s, of which one crop farm's judge test (melon4-mc-16b, its redsim yield run) took
1146 s alone on one core, 140 GB resident at the peak. The judge's run is not simulated for the
kinds whose page shows nothing of it (`simulate::judge::NO_RUN`: crop farms, item elevators,
crafters, smelters, minecart stations and cart stops, shulker machines, automatic doors: no input
to drive, no timing diagram, no report on their pages), and every parallel pass runs on
`--threads` (default 32; it was min(cores, 64) per pass, two passes at once). Cold, the same
build then took 148 s (the members 109 s, the circuits 97 s beside them, their slowest
fa-slice-54b 73 s; the lamp checks and stills 29 s); warm, 9.8 s. Its peak was still 132 GB
resident (the members' pass 82 GB, the circuits' 49 GB, at once, on a 223 GB box shared by
agents whose services an out-of-memory kill takes down), so the circuits now run after the
members, with the allocator's freed memory given back between them (`malloc_trim`): cold
213 s, peak 81 GB (the members' pass: `clock::sim::simulate` keeps every cell's state as a
string for every tick of a long member's period).

Measured on main 2026-10-21 (4414 builds, 1983 clock members): **warm 2.8 s** (80 MB), cold
**2 min 5 s** (was 4 min 28 s, 63 GB; cold peaks at ~75 GB). The cold pass is one member long:
`clk-item-dropring-1d-r1` n = 426 (the 5-minute instance, 866 blocks), whose generator's output
cell holds a comparator, so its output is searched again (`clock::sim::lamp_outputs`: a run per
candidate, ~50 s each); the others overlap it (members costliest first, the circuits beside the
clocks, every study's circuits in one pass).

Measured on main 2026-11-08 (5038 builds, 3154 clock members; a 166-core, 223 GB box): cold
**475 s** one pass after the other (41.0 GB): the members' pass 359 s, of which one member,
`clk-rt-counter` n = 3 (period 312,816 gt), takes 338 s alone; the circuits 77 s after it; the
stills and lamp checks 29 s; writing ~9 s. With the circuits' pass beside the clocks' (`site build`
does it when the box has 96 GB available, else one after the other; `--serial-passes` forces that):
cold **376 s**, 41.6 GB, the circuits hidden under that member. Warm 11.9 s, of which 9 s was
writing 20,000 files on one core; with the schematics and the pages written on the build's
threads, 9.1 s (the rest is the file system's lock on each folder: creating 5,038 pages in one
folder serializes). One farm design changed: 15.3 s, then 13.8 s. Then `site deploy` (its index,
tree and pack: 6.5 s; a 2 MB push with the transport commit) and GitHub's Pages build (25-48 s,
median 29 s): a design-only publish is about 50 s, one after a merge that changes the simulators'
crates about 7 minutes, that member's.

Then (2026-11-08, the same tree, cold 352 s before): `clock::sim::simulate` records a period
cell by cell only where a cell's block or extras change (its state string made and interned once
per change; the frame digest the sum of the cells' keyed hashes, updated per change: frames are
only compared with each other), `exterior_scan` reads its run as it goes instead of keeping every
frame's strings, `lamp_output_for` runs its candidates four at a time, the judges' runs (`drive`)
make a cell's state string once per change and `circuit::trace::Trace::new` keeps an unchanged
cell's id, and a mob farm's page runs no judge run (`NO_RUN`: nothing on its page came from it,
and `site preview` ran it again). The same site byte for byte (every page and site.json; every
archived clock's `clock sim --json`; every archived circuit's `circuit sim --table --wave`).
`clk-rt-counter` n = 3: 319 s → 94 s (its base run, its exterior scan and its first candidates'
runs, ~31 s each, one after the other; one run was 137 s); the members' work 2786 → 870
thread-s; fa-slice-54b 64 s → 36 s, the circuits' pass 81 s → 54 s; cold **128 s** (36 GB);
`clock sim` of the whole clock archive 97 s → 38 s; `site preview`'s first render of a new
mob-farm text 54 s → 0.1 s. Then the thumbnails' still frames are found beside the clocks' long
pole, the doors', circuits' and archived clocks' after the circuits' pass and each member's after
the member (`warm_stills`; the still-frame pass after the clocks 25 s → 1 s), and a comparator's
extras are compared as its strength (`wave::comparator_read`, `wave::reads_extras`): cold **90 s**
(members 80 s, its long member's base run, exterior scan and candidates' runs ~25 s each; the
circuits 52 s beside them, their slowest a memory 40 s and fa-slice-54b 38 s), the same site. Then
`simulate`'s period record and `exterior_scan` read only the cells that change: the world's write
log (`World::log_writes`) names the cells a write changed, the comparators' and containers' are read
at every tick (their extras change with no write), a candidate's power is read again only when a
cell within `SIGNAL_REACH` of it changes, and every cell and candidate is read at the first tick, at
W, at the last and every `TRACK_CHECK` ticks (a change neither named sends the run back to reading
everything at every tick; none in the archive or the site build). The long member: 80 s → 37 s
(its base run 25 s → 14 s, its exterior scan 23 s → 7 s); cold **62 s**, now the circuits' pass
(50 s) after which the build writes for 8 s; `clock sim` of the whole clock archive 30 s. Then the
judges' runs (`drive_seeded`) read their cells the same way, a frame listing the cells that are not
air from what the reads hold: fa-slice-54b's `circuit sim` 37 s → 21 s, counter576-items 29 s → 15 s;
cold **56 s** (members 38 s, circuits 43 s beside them, their slowest a TNT engine's run, 32 s, whose
own drive `drive_tnt` reads its cells as before; then 1 s of still frames and 8 s of writing).

### No dead links: every page ever published (`crates/site/src/published.rs`)

`crates/site/published.tsv` lists every page path the site ever deployed (8490 on 2026-10-21,
rebuilt from every deploy GitHub Pages kept, since 2026-10-19 23:21Z, by `mctas site published
--recover`) with what it
was: a circuit's `study/kind`, a clock's period, a door's main board, a family's card. `site
build` writes a small redirect page at each listed path it does not build (a design rejudged
out, no longer its kind's pick, an old clock, door or family page): a meta refresh to the current
page of the same kind (the same size of a parametric kind; a clock of the same period, else the
Clocks tab at that period; its board's record; a family's base design), a link, and a one-line
note saying why, from the archive's `rejudged/` entry when there is one ("This design was replaced
by “…”, because it failed a retest (jams with a full chest: 0 of 26 boxes after 22020 gt)."); the
pages vjeux deleted (`DELETED_PAGES`) stay deleted. The site carries the whole list
(`data/published.tsv`): `site deploy` adds what it published to the repository's list (commit
it), and refuses to drop a page of the live site (the page joins the list: rebuild); `site
publish` first merges the live site's pages into the list, so a list nobody committed is never
lost. `site smoke` checks every listed path resolves (a page, or a redirect to a page). Git merges
the list by union (`.gitattributes`). The 404 page links absolutely (`/redstonepedia/...`), and
`site serve` answers under that prefix too.

## Where the data comes from (no JVM)

- Doors: every `.txt` of `designs/piston-door-3x3/archive` with door and lever ports that is OUR
  OWN work (vjeux: "only things we generated, to avoid stealing people's work"): marked `ours` in
  `archive/provenance.tsv` (track c70b) and matching no seed rule (`community*`, `c79*`,
  `closed-b-fill*`, `satwire-c79*`, `satwire-63b*`, `pockets-63b*`, `c88*`, `door fill of` a
  community design: `site::doors::community_seed`); the others are never published (pages, data,
  design files, litematics, boards); a published door matching the seed rule fails the build, and
  `site smoke` checks it again. Read through
  the judge's caches next to it (`leaderboard::cached_entries`: verdicts, swept shapes, ground
  checks, volumes; `properties-v1.tsv` and `properties-dyn-v1.tsv` for seamless tiers, frame,
  hallway, eligibility), boards and leaders as `door catalog` computes them (over the published
  doors), captions from
  `captions.tsv` / `records.tsv`, videos and board GIFs from the `videos` release (`gh`). A design
  main's judge has not cached yet is left out (listed on stderr). The `door fill` variants of one
  mechanism (`note: door fill of <design>`, with that design when archived) fold under the best
  of them (size, then blocks); the boards page still ranks them all, like LEADERBOARD.md.
- Clocks: `designs/clocks/archive/verdicts-v1.tsv`, one build per type exactly as `clock catalog`
  groups them (variants folded under their host, archived family instances hidden with them by
  default), cards from `card_groups`, judged members from `extend.tsv`, real-game traces
  (`designs/clocks/traces/<stem>.trace`) passed to the viewer as `data-trace`.
- Circuits: each study's `designs/circuits/<study>/archive/verdicts.tsv` rows and `captions.tsv`,
  variants folded like `circuit catalog` (same kind and axis value); the judge's calibration and
  selftest designs (`designs/circuits/calibration/`, `selftest/`) are not published (vjeux: "not
  useful to people"; smoke checks it). Every
  circuit is also run through `circuit::verify::sim` at build time: the judge's own stimulus
  becomes the viewer's demo (`data-stimulus`, `data-phases`, `data-input-mode`; a BUD switch's
  block changes at its target cells as `data-events`) and the page's
  timing diagram (inputs and outputs per gt, a TARGET row for the BUD judge's changes) with the
  checker's report; whether the 3D view runs a design or shows it as built comes from the viewer's
  own session (`mcview::session::Session::new`, its `unsimulated` reason in plain words), and each
  live shulker machine is run at build time the way the view runs it (the judge's chests kept full
  and emptied, the start lever on) until it sends out 4 boxes: the page gives when the first box
  goes out and how often the next ones do, and the build prints every run (WARNING on none). The
  machines' studies (shulker loaders, TNT dupers, BUD switches) get a Variations table on their
  study page: the best design for each variation of their VARIATIONS.md, from the metrics
  (badges, items/s, rate, detected classes). The page's plain-language
  texts live in `crates/site/src/plain.rs` (vjeux 13:06Z: "explain all the builds so someone can
  understand them": every build page leads with one sentence of what it does with its own numbers,
  then why you would build it and how to try it; kinds in everyday words with the community's
  name in parentheses, `explain::kind_title` / `site_model::html::kind_words`) and
  `crates/site/src/explain.rs` (the in-depth texts, metric meanings, the studies' plain intros
  `STUDY_USES` and their "How we test" texts `STUDIES`). Our test, the measurements, the
  properties and the design's file name fold into one collapsed "Details" (vjeux 13:11Z: no design
  ids in any list or card; the name stays in the URL, the downloads and that details table).
  Captions that are agent or tool logs are dropped (`explain::caption_plain`); the first sentence
  of a designer's caption shows as "How this design does it". Clock pages show "Its output over
  time" (vjeux 13:46Z): the output over the start-up (shaded) and three periods, simulated from
  the design at build time the way the 3D view runs it (`pages::clock_timeline`; a clock whose
  design names no output shows its block that turns on most often, the census rule; a clock the
  browser's simulator cannot run says "Not simulated: <why>"). One card per mechanism
  (`generate::fold_twins`): doors with the same category, box, open/close times, seamless tier
  and properties fold under the smallest (listed on its page as its versions), clocks with the
  same period, blocks, parts and output carry the `twin` flag, and census clocks the `census`
  flag unless one is the record of its period; both are hidden from the default lists like the
  variants. Clocks follow the clock catalog's own rule (`clock::catalog::Row::shown`): a clock
  ruled out of a normal survival world (the verdicts' hidden column) or whose output pulses
  unevenly (clockinfra's `gaps` column; the coordinator's C38: a clock on the site delivers evenly
  spaced pulses at its output) is not published, and an output pulsing several times evenly per
  cycle shows that period as the clock's time (`ClockInfo::pulse_every`, `every()`; the cycle is
  its "full cycle"). Every build page says what the build needs in its opening lines
  (`plain::needs`: a TNT duper's height above the ground from its safe-drop metric, the items its
  containers start with from the design's `nbt:` lines, water, open sky) next to its warning
  (sculk, facing one way, one spot in the world). One build archived under two kinds (`same_build`)
  shows once on the Circuits tab (`kind-twin`, hidden unless its own kind is picked) with an
  "also a ..." chip on the other; shulker machines timed with other items (the same blocks) are
  one group ("works with any stack size"), and each shulker card leads with the variation it is
  the best design for (`Circuit::lead`). A bigger design strictly faster than every smaller one of its kind and value gets
  the `faster` flag ("faster than the smallest", `audit::mark_faster`). Circuits sort by
  `Circuit::rank` first (the first design of every kind, then the second...), then their study's
  place (`Circuit::order`; the gates with their own levers and the sand elevators are each
  study's basics, last). Search (`site_model::Site::run`): every build carries the community's
  words for its kind (`html::search_aliases`), builds whose kind the words name rank first, the
  players' names for extender reaches map to a value search (`with_synonyms`), and an empty
  search offers the nearest kinds (`html::nearest_searches`). Family pages lead with the kind
  and a title in words, how much each size adds, and keep the formula in a closed details.
  Pages vjeux deleted (13:35Z): the
  door boards, the families index, the list of everything and the about page
  (`generate::DELETED_PAGES`: never written, removed from an old output folder, and a smoke check
  fails if one exists or is linked). The catalog's folds (`catalog.tsv`):
  equivalent layouts (`variant_of` naming a design of the same kind, value, block count and block
  kinds) hide under their shown design ("Also N other layouts"), dominance variants as before, and
  a tunable group (`group`: one mechanism, other repeater delays) shows as one card (its smallest
  design, badge with the value span) whose every member page carries the group's value picker.
  The extendable circuit families (`extend` headers, `designs/circuits/families.tsv`) are cards
  like the clocks' (`families/<card>.html`, one per `extend group`; a card with several families
  shows its first base design in the grids): every n of the verified range generated with the
  family's own generator at build time (`designs/circuits/members/<family>-n<k>.txt` + its
  .litematic) and screened in redsim (`judged` from `extend.tsv`, `simulated` when the screen
  gives exactly the formula's value, else `formula`). The Circuits tab's search box takes a value
  (`L 12`, `N 26`, `12`, `10-20 gt`, `latency 0`: `site_model::parse_value_query`) and expands the
  groups and the families to the exact member. A build page starts with its live 3D view (vjeux:
  "Put the redstone before the explanation"), a group's picker under it, then the explanations.
- Interim pictures (the fallback under every 3D view): clock `img/<stem>.jpg` and family members'
  `img/<base>.n<k>.jpg`, linked from raw.githubusercontent.com (not copied), door board GIFs.
- Members of extendable families: every n of the verified range, generated at build time; status
  `judged` (in `extend.tsv`), `redsim` (the generated design simulated at exactly the formula's
  period) or `formula`. Disagreements are printed as notes.

## The 3D viewer contract (webrender: crates/mcview*, docs/webrender.md)

Any element with `data-mcview="thumb"` (still frame) or `data-mcview="full"` (live: orbit, play,
step, toggle the input) and `data-src="<relative url of a design .txt>"` (optional `data-design`
inline text, `data-kind`, `data-trace` = a real-game recording to replay, `data-tick`, `data-yaw`,
`data-pitch`; circuits: `data-stimulus`, `data-levels` (a strength input's values over the judge's
test, `<frame> <input> <value 0..15>; ...`, the hex kinds), `data-events` (the judge's block changes at a BUD's target:
`<frame> place|clear|shape|update <x> <y> <z> [<state>]; ...`, design coordinates), `data-phases`,
`data-input-mode`, `data-circuit`) is
rendered by the viewer bundle (`viewer/mcview.js` + `viewer/mcview_bg.wasm`, copied from the
checkout's `web/viewer/`): lazily when visible, one shared WebGL context for the thumbnails,
thumbnails cached by (src, size), new elements picked up by a MutationObserver. It adds
`mcview-ready` (the site's CSS then hides the element's fallback: the interim picture or a text
card) or `mcview-error`.

Cache-busting (GitHub Pages serves everything with max-age=600, and phones kept a broken viewer
after a hotfix): every page loads the bundles by content hash, `import init from
"./viewer/mcview.js?v=<hash>"; init({module_or_path: "./viewer/mcview_bg.wasm?v=<hash>"})` (the
wasm by its own URL: its default URL would drop the query, and a cached wasm must never meet a
new glue), the same for `pkg/redstonepedia*`, `style.css?v=` and `data/site.json?v=` (the index's
`#filters` carries it in `data-src`). The hashes are FNV-1a of the files, computed at build time
(`pages::set_versions`).

In-place swaps (vjeux: "Don't reload the page when clicking on variants, update inline and keep
the current camera"): a viewer that reloads its design when `data-src` changes marks itself with
the class `mcview-live-src` (webrender). On such a ready viewer, a build page's tunable-group
picker fetches the member's page and swaps everything around the viewer (`web/site/src/swap.rs`):
the viewer element stays in place, its `data-*` attributes are updated, then `data-keep-camera`,
then `data-src` last; `history.pushState` keeps the member's URL (popstate swaps back). A family
page has one full viewer (`#member-view`) whose design each member row (`a.view[data-src]`) swaps
the same way. Without `mcview-live-src` the links simply navigate.

## The UI

Every page's header is the title, the Doors / Clocks / Circuits tabs and one search box (vjeux:
nothing else at the top; about, door boards, families, circuit studies and everything sit in the
footer). `index.html` is a shell; `pkg/redstonepedia.js` fetches `data/site.json`, marks the
header's active tab with live counts and makes the header's box the live search: text, or on the
Clocks tab (or with a unit) the clock time search (`48`, `2.5 s`, `1 min 30 s`, `40-60 gt`,
`2-3 s`, `< 1 min`: `site_model::parse_time_query`); from any other page the box is a plain form
to `index.html?kind=<page kind>&q=<text>`, which the UI turns into the same query. The side panel
holds the main facet (door category / clock family / circuit kind: the base kind in words,
`Piston extender`, never one entry per N: the number is a property),
block kinds and properties as tri-state chips (must / must not / either), numeric ranges, sort
(by default the main number, then blocks: doors by size, clocks by period, circuits by kind, then
N / L / P...), and 60 cards per page (more on scroll). The query is the URL fragment (`#kind=clock&time=2-3%20s`),
so every search is a link; facet counts are computed with every other filter applied.

Every card (grids, study pages, related lists, family members, search results:
`site_model::html::props`) leads with the build's properties, not its generated name (vjeux:
"the focus is on the randomly generated name which is useless for the viewer"): the kind in small
capitals, the headline (a piston extender's `N = 3 · 19 blocks`, a clock's period and blocks, a
door's size and blocks, a divider's `÷3 · square`), a line of secondary metrics (extend / retract,
latency, re-arm, items/s...), chips (its own button or lever, 1 wide / 1 high, slime-free,
tunable span, badges) and the design's name last, small (and in the tooltip). A build page's
title is the same property line, the name under it. Circuits listed in `crates/site/hidden.tsv`
(study / file pattern, why) are not published at all (the slime-free piston extenders, vjeux:
"Why would anyone use this monstrosity?"); `mctas site smoke` checks none is.
