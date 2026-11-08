# Boat-and-lead mechanics (Java 26.3)

## Scope

Requested: understand boat/lead flyers, add faithful redsim support, and derive useful builds.
G1: [brief](../briefs/boat-leads.md). Foundation and the separate bounded vanilla probes are
approved; no parameter search, new kind or site publication is approved yet.

**No sustained or rideable flyer is demonstrated here.** A representative community clip
has been requested to pin down its version/mechanism; no claim of reproducing that exact
machine follows from the present tests. Raised-holder and nested-rider layouts remain
source-derived hypotheses until their actual game evidence says otherwise.

## Foundation implemented

* `redsim::entity::leash`: exact Java single-point boat spring, width/height/rotated anchors,
  center-distance attachment/snap gates, force-vs-torque damping, velocity coupling,
  angular momentum and yaw/friction ordering in baseTick.
* `redsim::entity::push`: AbstractBoat's nearby push loop and Entity's horizontal impulse,
  its absMax/sqrt arithmetic and passenger-bearing recipient exclusion. Existing swept
  collision boxes are reused, with same-vehicle exclusions; no vertical shove is invented.
* Read-only leash holder/angular fields in bridge entity snapshots; direct attachment
  fixture op calls the game's own attachment checks and implementation. This is a
  mechanics fixture, not proof of survival interaction or player control.
* The existing circuit `Rig`/`Recorder` and `redsim-fuzz trace-compare` carry leash actions
  and assert holder/angular state as well as all pre-existing positions, motions, rotations,
  blocks and random state. Old trace formats remain readable.

Still explicitly unsupported: ordinary mobs and their AI, player-controlled boats,
quad holders/happy ghasts, unresolved saved UUID restoration, and stochastic dropped-lead
item motion. Snap/invalid-holder/void removal unlink and fail closed; an explicit `/kill`
of a leashed entity is refused atomically before that unsupported drop. A remove operation
on a missing, removed or non-leashable entity is rejected, while a live unlinked boat is
a valid no-op. These limits are not hidden behind omitted drops, a fake moving holder or
a zero-velocity approximation.

## Measured evidence so far

The final fixed matrix recorded 16 official-code scenarios, 100 game ticks each, and the
new model matched every frame exactly. The pristine `35bcee3c7` binary matched two no-lead
controls, refused twelve leash-action cases and diverged on both actual boat-push/contact
cases. Scenarios cover slack/taut/open/mutual links, order/yaw/height changes,
ground/ice/water, chest boats, nearby push, stacked support, detach/retie and another world
location. The strengthened head-on input really contacted: minimum x separation was
exactly the boat width, 1.375; all101 recorded frames still matched. Expectations were
regenerated from the game, never hand-edited.

* Lifecycle regression tests passed: killing a leashed child is refused atomically;
  removing its holder and falling below the world fail closed on the lead drop; wrong,
  removed and missing leashee targets reject attach/remove; a live unlinked boat no-ops.
* `api-check` passed against the actual bridge: armor stand, minecart and item reject both
  attach and remove; valid boat no-op succeeds before and after; the game's `/kill` of a
  leashed boat produced exactly one lead, while the model explicitly refused its unseeded
  item motion. This tests the error path instead of merely trusting its source guard.

The tested bare mutual pair fell from y5 to the floor at y1 and settled by tick100; the
height-offset pair ended supported on the floor/other boat. These specific baselines do
not sustain flight. They do not prove all possible arrangements impossible.

## Commands

`mctas boat-leads calibrate --out-dir designs/boat-leads/traces --baseline <pristine-redsim-fuzz>`
records the fixed source-branch matrix, keeps its scenario JSON and exact traces, runs the
old binary as a positive missing-support control, then strictly compares the new model.

`mctas boat-leads record <scenario.json> --out-dir <dir>` reproduces one scenario.
`mctas boat-leads api-check` checks invalid targets and a real game `/kill` lead drop against
explicit model refusal, with valid live-boat no-op controls before and after.
`mctas boat-leads compare <trace>...` rechecks existing recordings without regenerating them.
`mctas boat-leads legacy --baseline <pristine-redsim-fuzz>` checks the complete old trace
suite and every archived boat/raft clock; it identifies boats from entity content, not names.
`mctas boat-leads legacy-audit --out-dir designs/boat-leads/evidence/legacy` validates the
saved reports without pretending to run them again. [Evidence provenance](evidence/legacy/README.md)
records the original base, the 39 positive plus four already-hidden negative records,
existing dependency flags, and the narrowly approved reuse across a docs/standalone-tool
main update. No claim of all 688 builds passing is made. The auditor requires the complete
four-position/four-rotation grid: exactly 16 builds for every record, including existing
negatives and dependent records. Reduced-grid regressions cover 1/1 robust, 8/8 dependent,
and 0/1 negative reports; internally consistent counts are not enough.

## Historical trace repair (separate change)

35 old exact recordings referenced deleted pre-strip designs. Repointing to a smaller core
was disproven by a positive/negative c3 control: two missing stones differ at frame0.
Original source fixtures restored byte-identically from git history match all35 original
recordings under the pristine model. Only trace header paths changed; recorded frames,
archives and published-page lists were not edited. See
[fixture provenance](../clocks/traces/fixtures/README.md).

## Separate vanilla evidence

The coordinator-assigned vanilla lane completed its bounded probes on an official 26.3
server with a real survival client and `allow-flight=false`. Collision-seated ordinary
cow/llama height and residual known movement matched the source; genuinely mounting a
seated unsaddled llama preserved server authority of the root raft. It did not make the
raft steerable with ordinary forward input.

No tested pair sustained flight. The ordinary, one-player mixed physical-dock and fixed
opposite-yaw arrangements all collapsed their initial spring extension and fell into the
void before400ticks. No-lead/lower-seat controls were retained; the asynchronous frozen
setup was explicitly separated from the normal-ticking physical-dock correction. These
are command-arranged mechanics fixtures with genuine attachment/boarding interactions,
not complete survival builds and not a universal impossibility proof. The pair group is
closed, not a license for a blind parameter sweep.

Full report, controls, raw evidence and viewed preview:
https://artifacts.internalmeta.com/a/2647248605707410

## Independent source review

https://artifacts.internalmeta.com/a/1678708950625598

It establishes legal directed cycles, single-point boat holders, source-valid raised pivots,
conditional lift bounds and client-authority pitfalls. None of those arithmetic results is
an achieved flight speed, an enduring orbit, or a global ceiling.
