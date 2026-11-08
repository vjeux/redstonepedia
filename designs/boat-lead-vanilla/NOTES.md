# Phase B vanilla evidence lane

Base: `35bcee3c777a01d6102d95ab28ff8c127034a005` (Nov 7). Branch: `explore/boat-lead-vanilla`.
Coordinator assigned this lane and confirmed reviewer + coordinator GOs on Nov 7 at 20:27 PT.

## Scope and evidence contract

Only the four approved probe groups: ordinary collision seating (adult cow/llama, oak/bamboo), exact passenger motion at the leash phase, genuine client direct/nested mounting authority, then crossed ordinary-holder and one-player mixed pairs with controls. No parameter search or redsim edits. No flight result yet.

Official Java 26.3 jars/decompilation only. Read-only phase instrumentation must preserve original instruction order and mechanics. Commands can arrange labeled fixtures, never substitute for survival attachment/boarding/launch in any survival claim. No NoAI, NoGravity, noPhysics, injected motion, or enabling allow-flight. Default shared Sandbox mob refusal stays intact.

Every trace records all involved entity IDs/holder IDs, position, raw motion, known motion, authority, root/controller, dimensions, contact flags, center distance, attachment distance, height gap, spring dot/branch, and pre/post leash/gravity states. A sustained flight claim requires >=1200 continuous ticks, later altitude windows, intact leads, loaded ticking entities, and no terrain/fluid support. Negative results remain bounded to the tested fixtures.

## Preparation findings

- Main's `mctas::vanilla::VanillaServer` starts a dedicated official server but hardcodes creative and `allow-flight=true`. It is unsuitable unchanged for the default-survival authority proof.
- The named `Harness` and real actor are not tracked on current main. Parent has been asked for the existing implementation's exact source/commit to avoid duplicating it.
- Seating matrix completed in a real Fabric survival client on a dedicated official server with `allow-flight=false`: 40 ticks each for oak/llama, bamboo/llama, oak/cow, bamboo/cow; normal collision boarding in all four. At every sampled post-first-boarding leash phase, the seated passenger delta and known movement are `[0,-0.0784000015258789,0]` (`y` bits `bfb41205c28f5c29`), not zero and not the stationary boat's zero. Measured passenger foot offsets: oak `.1875`, bamboo `.5`.
- Trace: `/home/vjeux/boat-evidence/seating-matrix-04/trace.jsonl`, 1880 read-only phase records. Reproduce: `target/release/boat-lead-vanilla run designs/boat-lead-vanilla/seating-matrix.json <fresh-output-dir>`; summarize with `boat-lead-vanilla summary <trace>`.
- Earlier attempt01 had missing Vulkan libraries; attempt02 exposed bundler classloader isolation; attempt03 exposed the 26.3 `entityTags` getter rename. Those attempts are not accepted phase evidence. The official server jar is unchanged; the observer helper is bootstrap-visible and reflection-only, and errors fail the Rust run.
- Infrastructure: coordinator approved the additive `VanillaServer::start_survival` profile (no bot, survival, no allow-flight, optional observer agent). The actor is reused from pushed `explore/bedrock-yt` commit `4b8f8f96cb111838adbfadcdeed0e9e57e4563a7`; only read-only hit identity and `probe_state` telemetry are added.
- **Genuine nested mounting verified:** `authority-01` has 2633 phase records. The real client clicks the raft (entity2), then later the tame unsaddled llama already seated in a raft (llama4, root3). Direct mount: root controller1, client root-authoritative=true, server boat-authoritative=false. Nested mount: intermediate controller=null and root controller=null; client root-authoritative=false, server root-authoritative=true. The nested configuration remains intact for the observed 164 root ticks, including genuine forward input. No `/ride`, synthetic player, or injected movement used. The first-person screenshot shows the llama mount/disembark prompt. This is a stationary boarding/authority proof, not flight.
- The ordinary seated llama can also retain small horizontal AI residuals (observed x=.0022294999222122113); the exact negative vertical residual is not a general all-axis-zero rule.
- Final release build Finished (1.87s incremental); final quick selftest passed34/34 in51s; Rust1/1; 11 retained evidence regressions passed, including exact gravity phases and <=2.23e-16 force reconstruction; cargo fmt check and git diff check clean; marker audit134changedtextfiles and function spans133core/110authored. Full door selftest was not required/run: no judge, redsim/model, search or wiring tool changed.
- The pair group is complete and stopped per coordinator after the fixed opposite-yaw contrast also failed. Read REPORT.md and CONTROL-RESULTS.md for final findings; no further angles/search authorized.

## Planned narrow hooks

Read-only `Entity.rideTick` observations before tick / after passenger tick / after positioning; `Leashable.tickLeash` before/after; `AbstractBoat.floatBoat` before/after; boat tick boundaries. Derive force-budget diagnostics from actual sampled states using official operation order, and retain raw values alongside them. This is instrumentation, not a replacement simulator.
