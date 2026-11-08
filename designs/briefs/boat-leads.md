# Boat-and-lead flight: mechanics study and build brief

Owner request (Nov 7, 2026): understand boats linked by leads, add faithful redsim support,
and derive useful flying machines. Coordinator: `f1a42990-e9ee-4714-8593-b34b64ed175d`;
implementation: `06b77372-3dca-485c-b69b-3777526c2fa4`; independent reviewer:
`ae26d988-0f97-4328-a953-d32fb5b0120e`. Base: main `35bcee3c7` (Nov 7).

**G1 status: foundation/calibration approved by reviewer and coordinator (Nov 7).**
No candidate search, new judge/kind, or publication approved; revised layouts and evidence
must return through G1 before search. This is first a mechanics investigation, not a claim
that a practical flyer already works. No community designs used.

## Use and numbers

Carry a survival player through open air without repeatedly rebuilding a piston engine.
Compare (1) sustained useful horizontal blocks/second and (2) boats + leads required;
list launch supports separately, and count chest/payload capacity honestly. Altitude,
steerability, stopping, safe boarding and repeatable setup are hard usability requirements,
not rewards for a screenshot or a large transient launch velocity.

The first stage measures **whether lift is sustained at all** and records every trajectory.
An unoccupied self-running test is not evidence a ridden boat works: the pilot changes
simulation authority. A catapult or glide is useful evidence, but is not called a flyer.

## Official Java 26.3 mechanics

Source paths below are relative to `net/minecraft/` in the official decompilation.

* `world/entity/Leashable.java:55–64,302–325`: one holder per leashee, no self-link;
  attach range uses bounding-box centers; attaching stops the leashee riding another entity.
* `world/entity/Entity.java:2331–2353` (independent reviewer): secondary-use transfers
  player-held leads to the target. The checked attachment path has no cycle/reachability
  prohibition. A mutual boat pair is therefore a survival-constructible graph, not an NBT-only
  graph. Actual player interaction still needs reproduction.
* `Leashable.java:141–173`: runs at the end of `Entity.baseTick` (`Entity.java:578–583`),
  in each entity's tick order; it is **server-only**. Snap beyond center distance 12. The
  early elastic-distance gate is `6 - holder.width - entity.width`; the actual spring uses
  rotated attachment points with slack 6. A lead is not a rigid distance constraint.
* `Leashable.java:208–268`: a spring applies to the **leashee only**. Boat attachment is
  dimensions times `(0,.5,.5)`, holder attachment dimensions times `(0,.5,0)`, rotated by
  negative yaw. Torque is `lever.z*force.x - lever.x*force.z`, before force damping. If
  motion dot displacement is nonnegative, force is multiplied by the widened float `0.3F`.
  Resulting force is scaled by `(.8,.2,.8)` and relative known velocity by `.11`.
  Angular momentum adds `10*torque`; yaw subtracts it (float cast), then momentum decays
  by ground-block friction times `.91F`, liquid `.8F`, or air `.91F`.
* `vehicle/boat/AbstractBoat.java:398–404` and `Entity.java:3976–3978`: a boat can be a
  quad-leashed object, but is **not a quad holder**. Boat-to-boat uses one attachment pair,
  not the four rendered points used for a qualifying holder such as a happy ghast.
* `AbstractBoat.java:557–643`: gravity `.04`, air horizontal retained velocity `.9F`,
  no vertical air drag. Player forward acceleration `.04F`; sideways-only `.005F`, reverse
  `.005F`; left/right change angular velocity by one per input tick. This control function
  is client-side, not an input that can be faked in a dedicated server trace.
* `AbstractBoat.java:233–249`, `Entity.java:4160–4165`: the controlling player's known
  movement replaces the boat's known movement. A non-authoritative boat zeros its motion
  after baseTick; client/server transfer must be validated with the real client.
* `AbstractBoat.java:178–186,285–301`, `Entity.java:1917–1962`: entity collision and
  horizontal push are separate; boat height gates which push applies. Passenger-bearing
  entities are not pushed by the generic mutual impulse. No vertical collision impulse
  may be invented to produce lift.

## Existing implementation and proof bar

Redsim already ports unoccupied boat gravity/buoyancy, block/entity collision boxes and
fluid interaction. It currently errors on nearby boat pushing; it does not model leashes,
player-controlled boats or boat passenger positions. Extend the existing model rather than
build a looser flight-only physics engine. Preserve Java operation order and float widening.

Use the existing bridge `Worker` and exact-trace plumbing. First record unmodified game
outcomes and the pristine redsim refusal/divergence. Then require bit-identical positions,
motion and rotations tick-by-tick for server-owned boats, plus leash holder and angular
state assertions. Controls: absent/slack/taut/snap lead, reversed tick order, headings,
unequal height, two touching boats, collision with ground/water, attach/detach, chest boats.
No simulated success survives a model error or an entity freezing outside loaded chunks.

## Different layouts and estimated resources

These are source-derived hypotheses, **not demonstrated speeds**. Ticking entities,
lead items and temporary launch supports are counted separately. No percentage of an
unproved flight-speed ceiling is invented. Full source review and calculator:
https://artifacts.internalmeta.com/a/1678708950625598

1. **Externally powered happy-ghast tow:** 1 happy ghast + 1 cargo boat, 1 lead, 1 harness.
   This is the obvious real air-transport baseline, not a boat-only self-powered machine.
   Ride the ghast, not the payload boat; payload authority needs its own check.
2. **Bare boat cycle (control family):** 2–4 boats and the same number of leads, removable
   dry dock. The two-boat pair is the obvious member; larger rings are variants, not
   separate mechanisms. No elevated pivots. Slack, taut, stepped and no-lead controls
   separate transient launch/collision from continuing lift.
3. **Crossed seated-llama raft pair:** 2 bamboo rafts + 2 ordinary adult llamas, 2 leads.
   A points to the llama in B and B to the llama in A. Raising the attachment points costs
   no additional lead; rafts seat riders at .5 versus wood boats' .1875. Static fully
   damped force is insufficient; only measured oscillatory force phases could justify
   further investigation. Wood boats/cows are lower-height controls.
4. **One nested player, asymmetric pair:** the same 2 rafts + 2 tame unsaddled llamas and
   2 leads; one player rides a seated llama while holding the opposite raft's lead.
   The other raft remains linked to the first raft's llama. One higher player pivot;
   root boats remain server-owned by the inspected controller path. Practical solo
   carriage is attractive, but tension/force balance on BOTH boats must be demonstrated.
5. **Two nested players, symmetric pair:** the same build with two players, each riding
   their own seated llama and holding the opposite raft's lead. The taller pivots have
   source-derived instantaneous damped lift margin on bamboo, unlike ordinary llamas
   alone. Two-person assembly/control is a significant real cost. No sustained extension,
   speed or steering has been established. Direct boat riders are NOT substitutes.

The actual build order, launch support count and observed rates remain unmeasured. A
client-authoritative payload does not automatically inherit server leash force. The
review's hover envelope is a conditional necessary bound, not an orbit or a speed ceiling.

## Decision and G2 baseline

Implement/calibrate server-owned leash and multi-boat behavior first, then test the mutual
pair without a parameter search. The open chain, slack mutual pair and identical apparatus
without leads are explicit negative controls. If the obvious candidate does not fly,
record that result rather than weaken the flight definition. Next hypotheses need source
or trace evidence, not a blind large search. The real client must test boarding and inputs
before any optimization is called useful.

Obvious-candidate measured values: **pending**. Sustained-flight ceiling: **not yet derived**.
The closed directed spring system is not a capped motor. It is invalid to divide a speed
by a guessed theoretical ceiling; first derive a bound or report the concrete benchmark
and the missing bound. A single normally rowed boat's no-leash horizontal recurrence is
`v[t+1] = float(.9)*v[t] + float(.04)`, a reference calculation, not a flight-speed ceiling.
This explicit uncertainty needs reviewer approval before any board/search metric is added.

## Must-haves and non-goals

* Use ordinary survival boats/leads; every graph and dock must have an executable build order.
* No commands, injected movement, noGravity or noPhysics during a claimed survival flight.
  Command setup is allowed in a mechanics trace and must be labeled as such.
* Sustained evidence: 1200 continuous game ticks after launch, same flight claim in later
  windows, with height/range/lead integrity and loaded-chunk checks. Distinguish ballistic
  launch, drifting descent, tethered suspension, unoccupied flight and ridden flight.
* Test mirrored headings, entity creation order, at least two world positions, a restart,
  pilot boarding, forward/turn/stop and a disclosed recovery/landing procedure. A layout
  may have documented limitations; do not claim robustness from one favorable trajectory.
* All exact legacy traces and full/quick door selftests remain green. A new unsupported
  player or mob case fails closed, rather than silently using the unoccupied model.
* No global optimum/world record claim. No community comparison without evidence. No new
  entity families just to make an invented flyer. No site publication until G3 and owner
  authorization. No need for automatic station infrastructure before the core works.

## Phase B G1 amendment: discriminating vanilla probes (approved Nov 7)

Both reviewer and coordinator approved these bounded probes; the coordinator assigned
`7dd9ba5a-7919-433a-96af-c17431c4c3d4` / `explore/boat-lead-vanilla` on a separate OD.
No candidate parameter search, new kind, or publication is approved.

After the source review, prioritize proof of the raised-pivot mechanism over a blind
cycle-size search. Proposed next work is a small, explicit set of official-game experiments:
(1) ordinary adult llama/cow seated by boat collision, wood versus bamboo; (2) exact
passenger delta and known movement sampled at the leash phase; (3) a real client mounting
a tame unsaddled llama already in a raft, compared with direct boat mounting, recording
controller/authority; (4) hand-built crossed ordinary-holder and one-player mixed-holder
pairs, with no-lead and lower-seat controls. Record every implicated entity and all
interventions, actual attachment distances, branch duty and force budget.

No NoAI, NoGravity, noPhysics, injected movement or allow-flight change may stand in for
normal play. Commands may arrange a clearly labeled mechanics fixture; any claimed
survival contraption later needs the full genuine construction/boarding/launch sequence.
Do not port an AI approximation to get a success. The second actor is not a fake server
pilot. These experiments are not a parameter search; search permission must come from a
revised decision based on actual traces. A separate vanilla lane may own this evidence
while the parent finishes the exact server-owned boat foundation.

## Planned evidence artifacts

Versioned source-cited notes, reproducible scenario files and recorded official trajectories;
regression controls and a calibration command; designs only if they pass; real-client
visual evidence for anything called a rideable flyer. Keep source fixes separate from a
possible site/viewer extension so useful correct physics ships even if flight is elusive.

## Legacy behavior contract (coordinator request)

The unoccupied single-boat tick is intended to remain operation-for-operation identical:
new leash code is inactive with no leash; mutual-push code replaces an explicit model error
only when another pushable entity is present. Existing water/current/bubble boat clocks
must therefore retain exact trajectories and periods. Run the entire kept exact-trace suite,
all archived boat/chest-boat clock verification in both game and redsim, and full plus quick
door selftests. If any old result changes, stop and diagnose it against the game; record
whether it is a genuine old-model correction, never bless new outputs by assumption.

## Source-review addendum: elevated holders

The reviewer proposed a more plausible lift hypothesis after the first draft: each boat
leashed to an ordinary mob seated in the OTHER boat. Incoming leads can stay attached to
a passenger; attaching dismounts only the leashee. Seated mobs' higher pivots could give
both directed forces upward components with no equal downward reaction. This may replace
flat boat rings as the primary layout family, but it is not a demonstrated flyer. Audit
ordinary (not NoAI) passenger motion, permissible survival boarding, tall holder choices,
and whether the spring can remain stretched before revising the build/search decision.
