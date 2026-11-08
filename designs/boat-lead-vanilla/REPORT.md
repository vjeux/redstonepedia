# Phase B vanilla evidence — final findings

## Answer

Ordinary adult llamas/cows do board boats by collision, their elevated holder positions are real, and riding an already-seated unsaddled llama preserves **server authority of the root raft**. Those facts are now measured with a genuine survival Fabric client on the official Java 26.3 dedicated server (`allow-flight=false`). They are **not sufficient to make the tested pairs fly**: every fixed pair/control descended into the void before 400 post-release ticks. No sustained or useful flyer was demonstrated.

## What the measurements establish

- Four collision-seating cases (oak/bamboo × adult llama/cow), 40 ticks each: passenger feet sit `.1875` above an oak boat and `.5` above a raft. At the boat's leash phase, passenger delta and known y movement are exactly `-0.0784000015258789` (bits `bfb41205c28f5c29`), not zero and not the stationary boat's zero. Ordinary AI can also leave a small horizontal residual.
- Actual crosshair mounting: direct raft riding gives the real client authority and the server root controller is the player. Mounting a tame unsaddled llama already seated in the raft leaves both the intermediate and root controllers null; the root stays authoritative on the server, not the client. The nested authority probe observed 164 root ticks, including real forward input. The stationary raft did not become steerable from that input.
- The crossed graph was attached with real lead/use and secondary-use interactions. Incoming leads stay on seated holders. The mixed graph was built with a real player's held lead followed by normal llama mounting, not `/ride` or a fake ServerPlayer.
- Read-only phase telemetry captures every tagged node, passenger/holder/root/controller identity, known and raw movement, exact attachment coordinates, branch/damping/force budget, gravity phases, ordinary flags, contacts and loaded-chunk state. Across all retained runs, all nine requested phases exist and ordinary/no-flight/loading checks pass. The diagnostic's largest force reconstruction error is `2.220446049250313e-16`; every tested dry-air boat gravity update matches exactly.

## Fixed pair results

Each server ran at least 1226 ticks after release. The apparatus itself failed much sooner. Counts include the terminal removal sample; later 401–800 and 801–1200 windows are empty, **not** successful flight.

- Crossed ordinary raft pair, equal rest yaw: **130/130** samples before removal. Only **3/2** spring-active ticks, all damped; horizontal attraction collapses the extension below slack.
- Lower-seat oak ordinary pair: **130/130**, only **2/2** spring-active ticks.
- No-lead raft control: **129/129**, ballistic fall. Nested-player no-lead control: **129/127**.
- Mixed raft pair with frozen setup: **366/376**; mixed oak: **301/308**. Both show some upward velocity and undamped phases, but keep losing altitude and terminate in the void. **These are asynchronous-setup fixtures, not synchronized launches**: a player-carrying root ticks through `/tick freeze` while the other root does not.
- Normal-ticking physical-dock mixed-raft control: **136/135**. Both roots tick throughout construction; actual passengers, graph and nested authority are asserted. After the complete dock is removed there are **zero supporting-block/fluid samples**, zero unloaded samples and no tick gaps. It still fails to preserve extension/altitude. Each lead disappears only on its own root's terminal removal tick.
- One source-directed orientation contrast approved by the coordinator, A yaw0 / B yaw180: **130/130**, again only **3/2** active spring ticks and no undamped phase. Matched no-lead control: **129/129**. Opposite transverse components and same-sign initial torque do not maintain extension in this fixed fixture. Per the coordinator's limit, the pair group stops here; no other angles or parameter search were tried.

See `CONTROL-RESULTS.md` for exact per-root counts/coordinates and `INITIAL-FLIGHT.md` for setup limitations.

## Important negative controls and limitations

`TickRateManager.isEntityFrozen` excludes any entity carrying a player, even when that player does not control the root. A frozen world therefore does not synchronize a mixed pair. The normal-ticking fence dock is the discriminating correction, not an injected motion reset.

The first dock attempt hit llamas instead of raft hulls, dismounting them through the ordinary attach path. It is excluded as a wrong-graph experiment. The driver now fails before launch unless crosshair target, actual passenger UUIDs, graph edges and nested authority match. Earlier classloader/API/rendering setup failures are also excluded.

These runs are **command-arranged mechanics fixtures**: commands create the platform/entities and remove supports; real-client interaction performs the important attachment and boarding. They do not prove full survival assembly/launch, steering, landing, robustness across worlds, two-player flight, a global impossibility, or a flight-speed ceiling. The no-lead and low-seat results constrain these fixed arrangements only. The unresolved question is how a normally constructible system could keep sufficient extension and favorable force phase without support.

## Reproduce and inspect

Base `35bcee3c777a01d6102d95ab28ff8c127034a005`; branch `explore/boat-lead-vanilla`. Infrastructure initially pushed as `e8d16b06a7899fac04e48c89811597799b8b2cee`. Actor reused from `explore/bedrock-yt` commit `4b8f8f96cb111838adbfadcdeed0e9e57e4563a7`. Only additive survival-profile plumbing, read-only instrumentation/telemetry, the standalone crate and this lane's evidence files changed. No redsim, main CLI, parent clone, Sandbox mob refusal or game physics edits.

```
cargo build --release
cargo test --release -p boat-lead-vanilla
target/release/boat-lead-vanilla run designs/boat-lead-vanilla/seating-matrix.json /tmp/new-seat-run
target/release/boat-lead-vanilla run designs/boat-lead-vanilla/authority.json /tmp/new-authority-run
target/release/boat-lead-vanilla run designs/boat-lead-vanilla/pairs/normal-dock-mixed.json /tmp/new-dock-run
target/release/boat-lead-vanilla report /tmp/new-dock-run
target/release/boat-lead-vanilla regress designs/boat-lead-vanilla/evidence
```

`evidence/` holds 11 retained runs with gzipped raw phase traces (verified lossless roundtrip), force budgets, scenarios/interventions, server/client logs, profile, checks and SHA256 manifests. The preview video is a **2fps, 15-second sample of actual gameplay** showing normal-ticking dock attachment, nested boarding, release and death in the void; all 30 preview frames were viewed. The complete original 20fps recordings remain in the lane's evidence directory on its OD, not in Git.
