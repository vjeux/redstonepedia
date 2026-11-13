# Working in redstonepedia (for agents)

Agent workflow only. What counts as a door: [docs/rules.md](docs/rules.md). Every command:
[docs/tools.md](docs/tools.md). Mechanics read from the source: [docs/mechanics.md](docs/mechanics.md).
What was tried before (read it before starting a direction): [docs/history.md](docs/history.md) and
the track notebooks ([tracks/README.md](designs/piston-door-3x3/tracks/README.md)).

## House rules (from the repo owner, vjeux)
- **Farms go straight to the main site** (vjeux 2026-11-08 16:27: "I asked you to stop using
  previews and push to the site directly"): the Farms section (/farms/) is live, so a farm
  candidate goes up as a variation of its stage with `mctas site variation <design> --stage
  <stage>` (it runs the kind's judge and the real-game gate itself) and the coordinator's fast
  path (GO, main, deploy), and its live URL goes to vjeux. Previews (`mctas site preview`) are
  retired.
- **New components follow [docs/process.md](docs/process.md):** a brief, the obvious design as
  the bar and the mechanics' ceiling, a search against that bar, then a review. No internet and
  no one else's designs. No new kind, metric or judge rule without an approved brief.
- **Rust only.** Every tool, harness, search, driver and one-off is Rust in this workspace
  (`crates/*`, usually a new subcommand of `mctas`). No bash or Python scripts, no `sh -c` blobs
  with logic: the shell only types one command. When a tool is one flag short of the job, add
  the flag. Edit files with the editor tools, not sed/perl.
- A new `mctas door` command goes in the door CLI crate of its area (`crates/door-*`: a variant
  in its `DoorCmd`, a line in its `run`, its `<Name>Args` and `cmd_<name>` in `src/cmd/<family>.rs`;
  no function over 150 lines); its work goes in piston-door, since the door crates are
  compiled unoptimized for fast builds ([internals.md#build](docs/internals.md#build)).
- **Never message anyone.** Report to whoever started you (your coordinator or vjeux).
- Never push untested code. A commit exists only when `git log -1` shows it.
- Commit identity: `Christopher Chedeau <vjeuxx@gmail.com>`.

## The judge, in short
- A door counts only if `mctas door verify <design> --extra-offsets 28` passes when built from
  **main** (3 open/close cycles at 32 positions); the coordinator re-verifies every record that
  way. The full rules, categories and boards are in [docs/rules.md](docs/rules.md).
- Never weaken or bypass the checks in `crates/piston-door/src/lib.rs` and never change how designs
  are placed (`piston-door/src/video.rs::place`). A design that only passes with a modified verifier
  does not count.
- Validate a mechanism with ideal inputs (`door drive`, `door ideal`) before wiring it; then wire,
  verify, shrink ([docs/tools.md](docs/tools.md#the-workflow-behind-the-records)). Proven circuits
  go in `designs/gadgets/` with a drive script.

## Shared boxes
- Several agents share each box. `MCTAS_MAX_WORKERS` (set in `~/.bashrc`) caps the worker JVMs
  of every pool; run at most two heavy processes at once. Never kill processes you did not start.
- Never record videos (the X displays are shared); the coordinator records records (`door video`,
  `video-all`, `gallery`, `montage`).

## Branches, files and pushing
- Work in your own clone (`~/agents/<track>/minecraft-tas`) on branch `explore/<track>`; never
  push to main or to another agent's branch. Push:
  `git -c http.extraHeader="Authorization: Basic $(printf 'vjeux:%s' "$(gh auth token)" | base64 -w0)" push origin HEAD:refs/heads/explore/<track>`
- Gate every push on the build (`cargo build --release` ends with `Finished`) and on no merge
  markers ANYWHERE the merge touched — `git grep -n '^<<<<<<< ' -- crates docs designs` (the selftest's own marker fixture aside), since a
  marker in a design file broke the site build on 2026-10-20 and `crates/`+`docs/` alone missed
  it (the judge now refuses such a file: `designs/circuits/selftest/merge-markers.txt`).
- Tests belong to the session that changes the code (vjeux 2026-11-08): the session that changes the redstone simulator runs the simulator tests to make sure its changes are safe; the session that changes a farm runs the farm tests to make sure the farm is working. Not everything for every change, no central authority running tests, no scheduled test jobs. Before every push run `mctas test` from the repository root: it reads YOUR change (the tree against `origin/main`), prints the plan and why, and runs only what the change can break, in parallel sized from the machine ([tools.md](docs/tools.md#mctas-test-the-tests-of-a-change)):
  - the simulator (redsim with its mob, entity, fluid and light code, redsim-fuzz, the bridge) → every recorded game trace replayed on redsim (seconds; a recording whose design changed fails as stale) and the differential fuzzers of the areas you touched;
  - a judge or study's code → that study's checks on its calibration designs;
  - a door, clock, circuit or farm design → that design's own judge;
  - every crate's code → its own unit tests too, beside the pieces above, and every crate's in a full run (`unit:<crate>`: `cargo test --release -p <crate> --all-targets`, a build failure or a failing test fails, a crate with no tests passes saying so, then clippy where the toolchain has it, its deny-level errors failing only in the files the change touches; redsim's tests run in `sim:redsim-tests`); a tool crate no calibration piece covers (the mob farm tools, the farm optimizer, the site's and the viewer's code, …) → its unit tests only; a change under crates/ that still selects nothing fails;
  - docs and web/ → nothing.
  Every piece must PASS; its last line (`Tests: mctas test (...)`) goes in the commit message. `mctas test --all` (= `door selftest`) runs everything, on demand only. A known answer that changes on purpose is updated in `crates/piston-door/src/selftest.rs` in the same commit, with the reason; a game run recorded again (`circuit record`, `clock misc trace-export --exact`) is followed by `mctas recorded write <its dir>`.
- The site push (deploy) requires no tests: the coordinator deploys from main with build + publish only.
- Keep your track's files in `designs/piston-door-3x3/tracks/<track>/`: `BEST.txt` (your best
  verified design), `NOTES.md` (what you tried, what worked, what did not, and why), `archive/`.
  Add your track's line to `designs/piston-door-3x3/tracks/README.md`.
- Every time you beat your own best: `door verify <file> --extra-offsets 28 --archive`, copy it to
  `tracks/<track>/BEST.txt`, update NOTES.md, commit, push.
- Tool improvements that help everyone go in their own commits with clear messages; the
  coordinator merges them into main.
- Generated files are never edited by hand: `designs/piston-door-3x3/LEADERBOARD.md`
  (`door leaderboard`) and `designs/piston-door-3x3/archive/README.md` (`door catalog`). A new
  record or board leader gets a row in `archive/records.tsv` (solid records) or
  `archive/captions.tsv` (one line: how it works) instead.
- Docs have one home per kind of content (rules, mechanics, tools, internals, history: `docs/`);
  link to them instead of restating them.

## Reporting to the coordinator
- Every time you verify a design worth keeping (a best or an interesting variant, any board), push
  it to your branch and send ONE line to the coordinator session (its id is in your brief):
  `<track>: <blocks> blocks <category>[, slime-free][, open/close ticks], <path on your branch>, <what changed>`.
- End with a short report: best block count and category, the file path on your branch, the
  mechanism in a few lines, what failed and why, the next ideas worth trying. Push everything first.
