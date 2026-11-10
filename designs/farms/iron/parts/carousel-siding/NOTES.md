# Carousel bed siding (vjeux's single lane; ironfarm's hand-over 2026-11-10 07:11)

The one shared bed. A napper's cart alone leaves its batch at the stand's departure and runs into
the siding. The napper is dropped beside the bed, naps, is woken by the dorm zombie through the
hatch, and re-boards the same cart. The cart then waits on the exit slot until ironfarm's release
pulse sends it out to rejoin its batch on the release run (~600 gt after it left).

Status, in vjeux's terms (07:29: "you can't say it's working when nothing is wired"), the page's
line: **Real redstone; tested on its own; arrivals and release are driven by the test bench; not yet
joined to the loop.**
- **Wired:** `carousel-siding-wired.txt`, real redstone.
  - The bench touches no control and no villager memory: every villager's HOME = the bed comes
    from the file's `port home:` line (R1's state; its 20 break-and-replace steps priced).
  - The bench touches no control.
  - It does two things in place of ironfarm's loop. It brings each arriving cart in: on the page's
    run, launched from rest on the bench's launcher (three boosters west of the approach); in the
    statistics runs, also teleported onto the approach at a drawn speed. And it places the 10-gt
    release as a block at the release input.
  - The riders wait in a bench hold (21 carts, over 16 from the bed) made to panic all night by
    hold zombies, as the loop's riders must.
- **Real build, but a bench design.** The launcher, the approach, the run-out past the exit and the
  hold are the bench's.
- **Plays in the viewer:** `carousel-siding-wired.drive.txt` (`siding-bench --wired --launch --from
  1 --to 1 --nights 1 --drive-out`, captioned for the view's badge). `mctas web drive-check` holds
  the view's own run to it: "ok: the view's run is the bench's" (219 acts, 325 cart positions,
  48,000 ticks).
- **Missing:** ironfarm's two junctions (the entry off the departure run, the merge into the
  release run) and the zombies along the loop.
- **Also kept:** `carousel-siding-bench-driven.txt`, the same siding with the controls switched
  by the bench (set_block): the first bench, before the circuit.

Tools: `mctas mobfarm siding [--wired] [--alone] [--exit-brake] [--unsettled] --out F` (settled by
default), `siding-check F` (every rider/napper cell's golem search, the siding's own block count and
box), `siding-bench F [--wired] [--launch] --from --to --nights --speed a,b --gap a,b --release a,b
[--visits N] [--release-held] [--placed-raw | --placed-in-order [GAP] | --placed-all |
--build-gap-sweep] [--drive-out F] [--trace --trace-window a,b]`.

## Interface (local frame: x east, z south, y up)

| what | cell | |
|---|---|---|
| entry | (0,0,0) | plain east_west rail; the cart arrives moving +x, 0.10 to 1.2 b/t stored all work (ironfarm's cap ~1.0) |
| release input | (4,-2,0) | the slot slope's support; strongly power it for 10 gt (ironfarm) |
| exit | (6,-1,0) | plain east_west rail; the cart leaves moving +x at 0.28-0.44 b/t stored (median 0.29) |
| box | x -7..6, y -4..4, z -3..9 | 234 blocks (the mechanical part alone: 60) |

Rules for the loop:
- Arrivals at least ~270 gt after the previous visit's arrival; the ≥ 4-release spacing gives 400.
- Every rider panics all night except the napper in the cage.
- No rider calm within 16 of the bed while a napper sleeps: ValidateNearbyPoi forgets the bed for
  good, and the sleeper keeps the ticket.

## How it works

- **Track.**
  - A T at (1,0,0) with its stem south: powered south_west (entry to the bed line), unpowered
    south_east (the bed line to the exit branch).
  - The bed line, from the T south: an always-on booster (1,0,1), so slow arrivals still park
    against the stop; the free cell (1,0,2); the activator (1,0,3); a slow run; the park rail
    (1,0,6) against a cobblestone.
  - The bed: head (0,0,2), foot (0,0,3), facing north, in ironfarm's glass cage. The iron hatch
    is at (2,1,2) and the dorm zombie in a boat at (4.5,1,2.5).
  - The exit branch, from the T east: (2,0,0), then a detector rail at (3,0,0), then the slot (a
    braked powered rail sloping down east at (4,-1,0)), a plain flat, and the exit.
- **Circuit.**
  - The nap is sensed by an observer in the cage wall south of the foot (0,0,4). Its pulse is
    stretched by a 4-tick repeater into an RS-NOR latch west of the siding (y -3).
  - Latch set means a visit is in progress. Torches under the T's support, the activator's
    support and the hatch's control turn the T to the exit branch, turn the activator off (the
    empty cart meets the napper over it) and shut the hatch (the zombie is seen and the napper
    wakes, 16-35 gt later).
  - The latch is reset by the detector rail: only the returning cart passes it. The detector sits
    two rails from the T because its power turned the T under a cart still on it.
  - The kick: a wall torch under the park rail's support on a block BK. BK is fed by "not visit"
    through five 4-tick repeaters (the cart meets the napper 71+ gt after the drop, past the
    60-gt boarding cooldown), OR by a tripwire across the slot (the slot is taken, so the kick
    waits for the release).

## Evidence

(Before 2026-11-10 14:30, with the bench's bed acts and its old schedule of about 22 visits a night;
the run without them is in "The one bed" above.)

Wired, `siding-bench --wired`, 100 seeds × 6 nights:
- Settings: visit gaps 400-600 gt, release 560-610 gt after arrival, arrival speed 0.15-1.1 b/t.
- **15,490 visits**: all dropped, napped, woken, re-boarded their own cart, ready on the slot and
  released. 0 problems.
- Ready 115-290 gt after arrival (median 145, p99 263); 0 over the 500 gt limit.
- Naps last 16-35 gt. The re-board comes 71+ gt after the drop.
- Waiting riders panic 99.50% of night ticks; the only calm ticks are the first ≤ 8 gt after the
  cart stops.
- Exit speed 0.28-0.44 b/t.

The same bench with the controls bench-driven gave 15,490 of 15,490 as well (ready 119-277 gt).

Wired, launched (the page's design and its arrivals), `siding-bench --wired --launch`, 100 seeds × 6
nights: 15,490 of 15,490 visits napped, re-boarded their own cart and were released; ready 118-283
gt after arrival (median 151); naps 16-35 gt; re-board 73+ gt after the drop; waiting riders
panicking 99.58% of night ticks; exit 0.36 b/t; 0 problems. (The member's metrics.)

`siding-check`: 0 golem-valid spots in the searches from every rider and napper cell
(entry, T, bed line, free cell, branch, slot, exit).

## The one bed: R1's claims are the design's (ironspec's check, 2026-11-10)

Until 2026-11-10 14:30 the bench handed the bed out itself at the start (`poi-take` of the bed's
ticket, every villager's `home` and `last-slept` set): a bench act the status never named. Now the
bench touches no villager memory:
- **The state is in the file.** `port home: 0 0 2` (the bed's head) gives every villager HOME = the
  bed as it spawns, the state R1's build steps leave (brief farm-iron-carousel rule 1: a villager
  claims the bed, the bed is broken and placed again, the next claims the fresh record). redsim
  counts the 21 claims as the bed's one ticket taken, as the real procedure leaves it (the last
  claim takes it); a paste in the game leaves the ticket free, which only a homeless villager
  could use.
- **The steps are priced.** prices.tsv's `bed_reclaim` (5 s a step: break and place the bed again,
  bring the next villager within a path, wait for its scan), N - 1 = 20 steps for the 21 villagers:
  100 s in the bill at every stage (`mobfarm bill`: 103.7 build minutes middle, 33.3 late, the
  bench's 21 carts included).
- **ironspec's check**, `siding-bench --wired`, 100 seeds x 6 nights, every villager once a night
  (the schedule now gives each of the 21 one visit a night, 400-500 gt apart; arrivals teleported
  onto the approach, the release placed by the bench): 1,400 dawn and midday checks, all 21 holding
  HOME = the bed at every one; 12,600 villager-nights, each with exactly one nap; 0 problems.
- **What moves HOME:** the napper drops it at its first REST tick and takes it back (the brief's
  release-and-reclaim): its CANT_REACH_WALK_TARGET_SINCE, armed while it panicked in the hold, is
  older than 1200 gt, so SetWalkTargetFromBlockMemory releases the bed; AcquirePoi(HOME) takes it
  back from the cart or the cage. 1,195 times in the 100 x 6 run, without HOME p50 10, p99 98, max
  121 gt, every one taken back before its nap; no other loss (no ValidateNearbyPoi erase: no rider
  calm in REST within 16 while a napper sleeps).
- **(a) cage-only, measured and not taken.** Closing the bed's open north front (0,0,1) (`siding
  --cage-only`) leaves the bed reachable only from the cage, but the napper's reclaim from the
  approach then fails and backs off: drop to asleep up to 370 gt, ready up to 489 gt (8 seeds x 2
  nights) against 114 and 287. R1's taken ticket already keeps every other villager off the bed.
- **(c) a give-up: not built yet.** Today a napper that cannot sleep leaves its cart parked for good.

## Settled as placed, and built in order (the brief's two checks; redsim, not yet the real game)

The wired file is written settled (`mobfarm siding --wired --out`, `placing::settle_design`): every
cell as the bench finds it after the settle. 65 cells changed: every dust in its settled shape and
power (the generator wrote dots at power 0) and the powered repeaters (the five 4-tick repeaters of
the delayed "not visit" line). Its `build: last -6,-3,-1 1,-2,6` line lists the two torches that are
off at rest as the build's last steps.

`siding-bench --wired --launch`, 24 seeds x 2 nights each (arrivals launched from rest on the bench's
launcher, the release placed as a block by the bench; since 14:30 no bed act, each villager once a
night, ironspec's HOME and nap checks in every run):
- the judge's placement: 24 of 24 seeds clean, 1,008 visits;
- settled as placed (`--placed-raw`, pasted raw, no settle): 24 of 24 clean, 1,008 visits;
- built in order (`--placed-in-order`, the priced gaps: 319 cells in 319 steps, 8,360 gt): 24 of 24
  clean, 1,008 visits; the build ends exactly as the file (the night under way at the spawn is
  skipped: the bench's nights are the full ones after it);
- `--build-gap-sweep`: every gap from 1 to 40 gt passes (4 seeds x 1 night each, 84 visits a gap,
  every villager's HOME at every dawn and midday and one nap each).
Clean: every visit dropped, napped, woken, back in its own cart, ready on the slot and released;
all 21 holding HOME = the bed at every dawn and midday; each villager one nap a night.

What the build needed: the latch is bistable. In the default order its first source is W2, the H
torch (-6,-3,-1), placed unlit with its block not yet powered; the first neighbour update lit it, so
the build ended with a visit set and the first arrival went to the exit unnapped (59 cells unlike
the file). Placed last, N2 is already powered through D1 by W1, and W2 stays unlit. K, the kick
torch (1,-2,6), placed early, was lit by the first update; BK's power arrived later through dust
written at its settled power, which tells K nothing (no power change), so K stayed lit and the park
rail powered. Placed last, BK is powered already.

Drives for tick-exact: `carousel-siding-wired-raw.drive.txt` (`--placed-raw`) and
`carousel-siding-wired-built.drive.txt` (the build's `act t set` lines, then the bench's acts;
`# spawn 8360`), both `siding-bench --wired --launch --from 1 --to 1 --nights 1`. Besides `set`, the
bench's acts are `tp` (a cart onto the launcher, back to the hold), as in the viewer's drive (since
14:30 no `home`, `last-slept` or `poi-take`: HOME comes from the file's `port home:`).

`mobfarm siding --wired --alone` writes the siding by itself for ironfarm's loop bench (no launcher,
approach, run-out, hold or carts): 234 blocks in x -7..6, y -4..4, z -3..9, and the dorm zombie.

## Ready time, the release held, and the exit speed (day-1's questions, 2026-11-10)

Bench-driven arrivals as before (teleported onto the approach at 0.15-1.1 b/t, visits 400-600 gt
apart), `siding-bench --wired`, 100 seeds x 6 nights, 15,490 visits a run, every one clean:
- **Released by the bench's pulses** (560-610 gt after the arrival): ready (napped, back in its own
  cart, at rest on the slot) p50 145, p90 229, p99 263, max 290 gt. The tail is the kick, not the
  walk or the nap: woken to re-boarded 69/155/191/225 gt (p50/p90/p99/max; the slowest 1%: 195 gt
  against 86 for all), the nap 25/33/35/35, and the napper wakes standing in the free cell. The kick
  waits while the slot is taken (the slot's tripwire is OR'd into it), and the bench's previous cart
  is often still waiting there for its release: visits come 400-600 gt apart, releases 560-610 gt
  after arrival.
- **The release held on** (`--release-held`): every visit napped, re-boarded its own cart and left;
  ready (on the slot with its villager) p50 111, p90 125, p99 141, max 225 gt; woken to re-boarded
  46/54/56/58. What is left of the tail is the lie-down: drop to asleep 0/0/9/115 gt (a few nappers
  walk round the bed first).
- **Exit speed** (stored, leaving the exit cell): with the pulses 0.281-0.436 b/t (median 0.286), 926
  of 15,490 over 0.36: exactly the carts that came to rest higher on the slot's slope (x 4.3-4.5);
  every cart at rest at x 4.7-4.8 leaves at 0.28-0.29. With the release held 0.536-0.719, every one
  over 0.36.
- **One slowing cell does not fix it.** A braked powered rail is a stop: it halves the speed every
  tick the cart is on it and stops it under 0.03 b/t. One just past the exit (`siding --exit-brake`,
  x 7) stopped 346 of 350 carts with the pulses and 356 of 356 with the release held (8 seeds x 1
  night). On the exit cell itself it is powered whenever the slot's tripwire fires (the stone under
  the tripwire line's dust at (6,-1,1) is beside it), and boosted the carts to 0.66-0.84 b/t with the
  release held. An uphill cell takes off only the slope's gravity, 0.0078 b/t per tick on it (the
  game's rule; not benched).

## What failed on the way (and why)

- **Slow arrivals stopped short of the cobblestone.** The kick then lost the park rail's length,
  and the empty cart met the napper under 0.1 b/t (no pickup). The always-on booster at (1,0,1)
  fixed it for every speed from 0.10 to 1.2.
- **The held riders were calm at REST.** They gave up HOME after 1200 gt out of reach and took
  the bed's one ticket, or backed off 40-400 gt after failed paths. Hold zombies fixed it. A
  released rider teleported back within 16 of the bed while the next napper slept forgot the bed
  for good.
- **The circuit (three bugs, all fixed):**
  - The detector rail beside the T turned it under the cart.
  - Dust beside a block a repeater powered strongly fed the visit line back to itself and latched
    it.
  - Moving the dorm zombie one block east put the sleeping napper's sight line on the shut
    hatch's top, and a nap never ended.
