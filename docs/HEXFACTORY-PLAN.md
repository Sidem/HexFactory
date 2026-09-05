# HexFactory — goal, state, and roadmap

This is the live product and development plan. Current engine rules belong in
[`ARCHITECTURE.md`](ARCHITECTURE.md), visual rules in [`ART.md`](ART.md), and measurements in
[`BENCHMARKS.md`](BENCHMARKS.md). Source, tests, and those focused documents are authoritative.

## Goal

Build a beautiful, open-ended factory-automation game in an unbounded hex world: deep enough to
reward large systems, pleasant to explore, and precise to control. Geography, life, industry, and
the player should affect one another visibly. Progression opens options and culminates in milestones
without ending the world.

Player experience decides scope. Determinism, native simulation ownership, sparse cost, and measured
performance protect that experience and remain non-negotiable.

### Product rules

- Every milestone must create a player-visible decision or remove a player-visible obstacle.
- Controls must be obvious initially and remain precise at scale; feedback is part of the mechanic.
- Hex geometry matters only where faces, rings, fronts, or approach directions create a clear choice.
- New content should reuse data-defined systems. Add a native branch only for genuinely new behavior.
- Idle world area and idle entities should cost almost nothing. No permanent whole-world tick.
- Keep source and documentation within the context budget. Prefer small ownership modules over broad
  coordinators, duplicated explanations, or speculative abstraction.
- Performance and balance claims require committed measurements.

## Current game

A run begins beside a landing hub in a chosen world preset. The player explores a surveyed frontier,
crosses physical landforms and water, gathers finite resource sites, completes hub requests and
contracts, researches technology and personal skills, and builds a powered factory. The current
factory includes manual and fuelled work, electricity, multi-output recipes, belts, junctions,
underpasses, pipes, storage, walls, gates, paving, roads, bridges, earthworks, and deterministic
save/restore. Forests deplete and regrow; disturbed rivers settle and erode through bounded native
work rather than a global water tick.

One construction hex is 25 m² and one height quantum is 0.25 m. Rust/Wasm owns world and simulation
truth; TypeScript sends bounded commands and renders native snapshots. The detailed contract is in
[`ARCHITECTURE.md`](ARCHITECTURE.md).

| Envelope        | Current |
| --------------- | ------: |
| Save (`HXF1`)   |      44 |
| Definitions     |      30 |
| Technologies    |      18 |
| Scenarios       |       8 |
| World generator |      16 |
| Snapshot wire   |      23 |

The latest shipped milestone is **v0.47.0 Flowing Water**. Older 1 m² worlds and worlds from another
generator version remain exportable but are not remapped. Same-generator 25 m² save formats migrate
through explicit adjacent steps.

The v0.43 browser record advances and draws 6,144 entities at 32.3% / 33.5% / 33.9% of a 60 Hz frame
on Low / Medium / High at 1440×900 on the reference desktop. That is the supported evidence, not a
claim about other hardware. See [`BENCHMARKS.md`](BENCHMARKS.md).

The game is still a polished short-form slice. Its main product gap is a sustained programme for an
established factory. The Rust entrypoint and browser application are split by ownership, and the
production payload has a measured budget; new work must preserve that modular headroom.

## Development order

Complete these items in order. Fixes and prerequisites stay with the active item; do not start a
later phase to avoid an unmet gate. A phase may ship in several small releases.

### Now — engineering quality to 9/10

Requested 2026-09-05 following the source review at `00818b9`. This engineering programme precedes
Phase 9; the relative order and content of game-design phases 9–13 stay unchanged. Implementation is
tracked in the current handoff below. No step is complete merely because this plan exists.

Objective: earn at least 9/10 separately for simulation architecture, maintainability and agent token
efficiency, native compute efficiency, and graphics efficiency. Scores remain review judgments; the
gates below are the evidence required to award them. Game design, content, balance, progression, and
visual redesign are outside this programme. Preserve current observable behavior, including tick
order, arbitration, recipe timing, player cadence, terrain, and save compatibility.

Prefer 10/10 in each category when closing the remaining gap requires only small, local changes or
validation using the existing harness, with no significant extra work or added architectural
complexity. The 9/10 gates remain the minimum. At E7, explicitly identify any remaining gap to 10/10
and close inexpensive gaps before finishing. Do not pursue a new abstraction, subsystem, broad
rewrite, or open-ended optimization for the extra point. Award 10/10 only when the final evidence
supports it within the declared workload and hardware scope; do not relabel a passing 9/10 gate.

The earlier headroom work is complete: the application entrypoint is split, context ratchets pass,
and production payload budgets exist. Remaining concerns are shared mutable ownership, inaccurate
navigation anchors, source-text interaction tests, broad presentation invalidation, and incomplete
performance coverage. Do not restart the entrypoint split or replace the engine/framework wholesale.

#### Target gates

These are proposed acceptance budgets, not current measurements. Record the full baseline before
optimization. A missed budget keeps its stage open; do not relax it silently or claim 9/10. If a
budget proves inappropriate, document the measured constraint and obtain an explicit scope/budget
decision before changing it.

| Area                             | Evidence required for 9/10                                                                                                                                                                                                                                      |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Simulation architecture          | Explicit state owners and mutation boundaries; one bounded coordinator; deterministic differential replay of optimized versus reference behavior; every derived cache/index rebuildable and checked against its oracle; wire and save compatibility gates pass. |
| Maintainability/token efficiency | No context-debt exceptions; accurate declaration routes and neighboring test routes; feature controllers own their state and declare narrow dependencies; no application prototype augmentation; measured representative task context meets the budgets below.  |
| Native compute                   | Current native and Wasm results for active, idle, blocked, power, junction, and edit workloads; sparse sleeping work; bounded invalidation; no material active-work regression; target timings and operation-count gates below pass.                            |
| Graphics                         | Snapshot preparation and UI included in measurements; incremental factory/resource updates; bounded terrain updates; visible-work scaling, verified GPU-resource lifetime, frame-time targets, and preserved visual/picking behavior.                           |

Reference desktop: Ryzen 7 5800X / Windows 11, browser version recorded, 1440×900, DPR 1, 178 px
minimap. Use the same machine and build settings for paired comparisons. Also exercise the supported
profile DPR caps (Low 1, Medium 1.25, High 1.5), a zoomed-out view, and one actual integrated-GPU
machine on Low; record its exact hardware. Missing hardware is an unfulfilled validation gate, not a
performance failure or permission to substitute an emulation result.

- At 6,144 entities in every steady workload: native tick p95 ≤ 1 ms and native advance plus delta
  encoding p95 ≤ 3 ms on the reference desktop. Measure identical workloads in Wasm separately.
- At the same size: worker operation p95 ≤ 4 ms, main-thread snapshot/visible UI/render preparation
  and submission p95 ≤ 6 ms, and measured GPU time p95 ≤ 8 ms. These overlapping spans are separate
  budgets; never add them and label the sum an observed frame.
- In real requestAnimationFrame runs at 60 Hz: p95 frame interval ≤ 18.5 ms, p99 ≤ 25 ms; at most
  1% of intervals exceed 33.4 ms. Report actual presented-frame/dropped-frame evidence when the
  browser profiler supplies it; requestAnimationFrame alone does not prove presentation.
- During scripted construction, extraction, exploration, and ground edits: no game-attributed
  main-thread task ≥ 50 ms; command-to-visible result p95 ≤ 100 ms and p99 ≤ 200 ms. Measure player
  work completion separately: a timed earthwork action must keep its intended duration.
- Integrated GPU, Low, 3,072 entities: the same frame-interval and interaction targets. Desktop
  Medium/High must also pass at their profile DPR caps. GPU timing unavailable on a browser must be
  reported explicitly and checked with a profiler on the reference browser before closure.
- Retain startup ceilings: 320 KB JavaScript, 560 KB Wasm, 48 KB interface, 896 KB total, gzipped;
  reference warm-cache ready mark p95 ≤ 750 ms across ten loads. Add cold-cache and throttled-network
  records with reproducible latency/bandwidth settings; do not describe warm results as cold startup.

#### E0 — establish trustworthy baselines

Entry points: `.agent/benchmark.md`, `src/bench/main.ts`, `src/bench/report.ts`,
`factory-wasm/src/capacity.rs`, `src/app/coreView.ts`, and `src/app/lifecycle.ts`.

1. Record HEAD, dirty-tree status, toolchain, browser, hardware, viewport, DPR, profile, and workload
   definitions. Preserve unrelated edits. Run `npm run quality` once and distinguish pre-existing
   failures from new ones. Retain the existing v0.43 records as historical comparisons.
2. Extend the deterministic ladder with named workloads: active straight lines; all-idle factory;
   fully blocked lines followed by reopening sinks; dense splitters/mergers/underpasses; powered
   production under full and insufficient supply; separate outposts with one edited component;
   mixed extraction/regrowth, river pumping, and disturbed water. Use 768/3,072/6,144 entities and
   a diagnostic 24,576 tier. Pin seeds, topology, commands, ticks, production totals, and checksums.
3. Add live browser scripts for 100 place/rotate/erase cycles, a 60-second extraction run, surveying
   100 new chunks, 100 ground-brush stamps near water, and a camera pan/zoom through outposts. Use
   native commands or actual UI, with fixed scripts and valid starting inventories. Exercise both
   ordinary and disconnected factories. Include a busy inspector and open production panels.
4. Measure advance/encode, worker round trip, decode/merge, application update, renderer
   `setSnapshot`, frame preparation/submission, GPU execution, and input-to-visible latency.
   Time the real application path as well as isolated phases. The current `measureRender` excludes
   `setSnapshot`; correct that omission without invalidating the label of historical records.
5. Warm each workload for five seconds, then collect five independent 30-second runs. Report each
   run's sample count, median/p95/p99, worst interaction, allocations where measurable, heap,
   draw calls, triangles, and rebuild counts. Exclude setup only from steady-state reports; report
   it separately. Reject background-tab, thermal, or unrelated-load contamination with a reason.
6. Add deterministic counters for visited entities/edges, rebuilt graph members, dirty records,
   meshes rebuilt, instance slots written, and panel updates. Counters and instrumentation must not
   enter saves/checksums or impose production overhead when disabled.

Exit: committed raw baseline reports and method in `docs/benchmarks/` and `BENCHMARKS.md`; harness
tests prove workloads do useful work and measure the intended spans. Timing remains outside shared
CI; deterministic workload and operation-count checks run in CI. Do not optimize in E0.

#### E1 — repair navigation and establish agent-context measurements

Entry points: `scripts/agent-map.mjs`, `scripts/context-budget.mjs`, `.agent-budget.json`,
`tests/sourceGraph.ts`, and `src/core/checkpoints.ts`.

1. Route frame/update to their actual declarations under `src/app/`; progression to
   `core/progression.rs`. Teach declaration extraction to recognize current prototype assignments
   during migration and final exported functions/classes afterward. Fail when a route's named
   behavior cannot be resolved; checking generated text alone is insufficient.
2. Give each feature route a nearest behavior-test anchor. Keep domain indexes below their existing
   limits; subdivide oversized domains instead of expanding the root instructions.
3. Correct stale checkpoint descriptions against catalogue data. Derive displayed numeric costs
   from data where possible; do not duplicate narrative balance claims across files.
4. Define six fixed read-only extension probes: recipe addition, input action, snapshot field,
   power behavior, ground edit, and machine visual variant. For each, record the exact source/test
   ranges needed to locate ownership, contracts, and validation before writing a patch. Measure
   baseline and final with the same tokenizer/version. Exclude repeated tool-output wrappers;
   report file count and source bytes as reproducible fallback measures.

Exit: routes resolve real declarations and tests; instruction entry remains ≤ 2,000 measured tokens;
final median probe read set must be ≤ 8,000 tokens, no probe > 12,000, and median must fall ≥ 30%
from E1 baseline unless already ≤ 8,000. Tokenizer unavailable means report byte proxies and leave
the token gate pending. These are bounded discovery costs, not promises about an agent's entire run.

#### E2 — replace shared application ownership with explicit controllers

Entry points: `.agent/browser.md`, `src/app/runtime.ts`, `createApp.ts`, `bootstrap.ts`, and the
controller/wiring pairs. Start with session/save lifecycle, then construction/input, then workspace
and inspector presentation. Keep each extraction reviewable and independently passing.

1. Introduce explicit session state, construction state, selection state, and panel/view owners.
   Expose read-only snapshot access and narrow command/preview services. Each mutable field has one
   owner; other controllers request an operation rather than writing that field directly.
2. Construct controllers in `createApp` with typed dependencies. Replace `Runtime.prototype` and
   interface augmentation incrementally; remove migrated fields and wiring immediately. Eliminate
   definite-assignment assertions used to hide initialization order. DOM lookups belong to the view
   that owns them, with checked construction-time requirements.
3. Keep the frame coordinator responsible only for clocks, bounded dispatch, snapshot distribution,
   and render scheduling. Keep the composition root under 250 nonblank lines, with no game/UI rules.
   Avoid a generic event bus, service locator, or a replacement context object exposing all state.
4. Replace behavioral source-string assertions for each migrated feature with execution tests.
   Cover pending previews, stale responses, held construction/gathering, focus, disposal, save/load,
   and worker failure recovery. Keep static tests only for intentional import/ownership prohibitions.
5. Add dependency checks: no cycles within `src/app`, no feature-to-bootstrap imports, no UI imports
   from core protocol modules, and no new all-purpose application state interface.

Exit: no prototype-installed application methods; controllers instantiate in isolation with small
fakes; critical interactions execute in tests; baseline command streams and visible results remain
equivalent. Maintain behavior-preserving adapters only within a work package, not after its exit.

#### E3 — establish native ownership and reference oracles

Entry points: `.agent/simulation.md`, `model/core_state.rs`, `core/mod.rs`, `runtime.rs`,
`core/commands.rs`, `core/tick.rs`, `factory_delta.rs`, `wire.rs`, and persistence/migrations.

1. Map canonical state, derived indexes, scratch, and publication dirtiness explicitly. Extract
   cohesive owners for entity storage/inventory, transport topology, power runtime, and world-edit
   state where current cross-domain writes prevent local reasoning. `Core` remains the coordinator;
   avoid passing unrestricted `&mut Core` into every extracted system or introducing an ECS rewrite.
2. Give mutations explicit effects: affected entity IDs/cells, topology invalidation, resource and
   presentation dirtiness. Inventory/placement/world APIs must record the effects at mutation time.
   Preserve atomic refusal and undo conservation. Keep the existing tick phase order unchanged.
3. Retain simple full implementations as test/bench reference paths for graph/index rebuild,
   snapshot diff, dirty membership, and any optimized scheduler. Catalog every derived cache with
   its authoritative inputs, invalidation triggers, rebuild function, and nearest oracle test.
4. Differentially replay at least 100 fixed command seeds × 1,000 advances, with place/erase/rotate,
   transfer, recipe changes, power changes, depletion, world edits, and save/load discontinuities.
   Compare canonical state/checksum, events and published snapshots at each boundary. Resume from
   three fixed mid-run checkpoints and require identical continuation. Retain failing seeds.
5. Preserve protocol limits and exact integers; test truncated/malformed wire messages, revision
   mismatch/resynchronization, rejected commands, and supported save migrations. Extend shared wire
   fixtures for intentional protocol changes. Keep optimization-only state out of save formats.

Exit: each mutation and cache has a named owner and executable oracle; optimized and full paths agree
under replay; existing wire/balance/save tests pass without unexplained fixture changes. Document the
actual contracts in `ARCHITECTURE.md`, not a second design document.

#### E4 — make native cost follow work

Depends on E0 and E3. Use measured phase/operation counts to choose the order within this stage.

1. Reuse per-network power request/plant buffers and transport scratch; avoid allocating maps/vectors
   every tick where topology already supplies membership. Preserve stable-ID ordering and exact
   proportional allocation. Prove warm steady ticks do not allocate topology-sized scratch.
2. Add derived active/due membership for empty belts, disabled machines, and blocked transfers.
   Wake on relevant input/output changes, recipe/running changes, power changes, topology edits,
   resource availability, and due transit/craft/regrowth times. Define every wake reason before
   skipping work. Reconstruct due membership on load from canonical state and clocks.
3. Compare each skipped-work optimization against E3's full tick, including same-tick wake ordering,
   merger fairness, unblocking, and multi-output reservation. A resting factory may still advance
   its clock; avoid manufacturing entity dirtiness for unchanged state.
4. Replace whole-factory depletion invalidation with reverse deposit dependencies. Batch and dedupe
   dirty IDs deterministically. Idle and untouched regions must generate no entity/resource patches.
5. Measure localized graph edit cost, including occupancy/index reconstruction, not just links
   recompiled. Retain incremental compilation only where it wins; add a deterministic full-rebuild
   fallback for large affected regions. Select the crossover from committed measurements. Keep the
   simple compiler as oracle; do not introduce stable-slot storage unless profiling justifies it.

Exit: 6,144-entity timing budgets pass; increasing a fully resting factory from 6,144 to 24,576 adds
no per-tick machine/belt/edge visits after settling. Increasing disconnected resting outposts must
not increase the number of links recompiled for a fixed local edit. Report any remaining O(N)
bookkeeping separately. No active tier regresses by > 10% in median across the five runs; repeat
borderline results before deciding. Apply the same replay gates in native and Wasm.

#### E5 — retain changes through the host and rendering pipeline

Entry points: `snapshotDelta.ts`, `FactoryHost.ts`, `coreView.ts`, `worldInstances.ts`,
`ThreeFactoryRenderer.ts`, `terrainMeshes.ts`, and `MinimapRenderer.ts`.

1. Preserve changed/removed IDs and affected terrain/resource cells as presentation change sets when
   applying native deltas. Keep the existing full snapshot API for reset/oracles. Distinguish
   topology, appearance, dynamic state, and panel dependencies; never derive simulation facts in JS.
2. Remove the per-snapshot whole-factory structural string. Maintain ID-to-instance-slot mappings
   and spatially partitioned machine/resource buckets; patch touched slots and upload changed
   ranges. Handle removals/compaction atomically. Rebuild all only on world reset or incompatible
   layout/profile change, with a documented reason.
3. Update resource instances by changed deposit, including depletion/regrowth. Local factory edits
   update only touched spatial buckets and actual linked visual dependants, such as wires/routes.
   Avoid a fixed draw-call goal that defeats useful culling; report calls and submitted geometry.
4. Retain terrain chunk/halo rebuilding and verify its bounds. Reuse change sets to avoid rescanning
   all surveyed chunks/overrides for a one-cell water/ground update. Patch minimap data similarly.
5. Replace tick-wide panel refreshes with explicit dependency revisions/changed groups. Key and patch
   existing controls, preserving focus, selection, pointer capture, and scroll. Closed expensive
   views should consume their latest state when opened rather than repainting while hidden.
6. Dispose removed InstancedMesh-owned GPU buffers and owned geometry; preserve shared geometry and
   materials until their final owner releases them. Cover world replacement, renderer disposal,
   repeated structural edits, profile changes, and context loss/restoration.

Exit: counters show zero static/resource bucket rebuilds for a progress-only tick; one changed
deposit patches only that deposit's bucket/slots; a local edit does not rewrite an unrelated outpost.
Full-rebuild and incremental render data agree after the E3 command traces. After 1,000 edit/reset
cycles, live GPU-resource counts return to baseline; after forced GC in a test browser, retained
heap stays within max(5 MB, 5%) of the warmed equivalent scene. No focus/picking regressions.

#### E6 — scale drawing with visible work and verify frame pacing

Depends on E5. Reuse E0 scripts, seeds, snapshots, and camera paths.

1. Cull spatial machine/resource buckets, and restrict CPU animation work to visible/near-visible
   buckets. Derive purely cosmetic repeated motion in shaders where measurements show savings;
   cargo identity, path and timing still derive from native snapshots. Reduced motion stays exact.
2. Profile GPU passes before changing geometry/materials. Reduce distant detail, secondary motion,
   or shadow work where measured; preserve near-view silhouettes, port direction, cargo readability,
   terrain height, and picking. Shadows update on their actual visual dependencies.
3. Replace CPU-core-count-only default quality selection with a conservative initial profile and
   measured rendering capability if the hardware matrix demonstrates misclassification. Preserve
   explicit user settings; avoid oscillation and startup regressions. This is conditional work.
4. Inspect repeatable images at near/normal/far zoom for all profiles, compare selected-cell picking
   at slopes/bridges/water/footprints, and run context loss/restoration while the factory advances.
5. Run the complete desktop/profile/DPR/integrated-GPU matrix and startup measurements. Compare
   3,072 visible entities with the same view plus disconnected offscreen entities up to 24,576:
   visible instance-write count must stay constant and renderer CPU median increase ≤ 10%.

Exit: all graphics/frame/interaction budgets pass with committed traces and raw results; visual and
input regression checks pass. If GPU time is limiting, optimize the measured pass rather than
spending native tick headroom or lowering quality without recording the tradeoff.

#### E7 — close context debt and independently reassess

Most debt should disappear while E2–E6 touch its owners. Finish the remaining bounded extractions:

- `balance.rs`: exact-ratio/economy solving, opening dependency analysis, and report/fixture output.
- `terra.rs`: drainage/province solution, sampling/cache, and landing queries, respecting oracle tests.
- `wire.rs`: group codecs around one bounded envelope dispatcher; preserve shared fixtures.
- `ground.rs`: transaction resolution, surface rules, and commit/undo around existing ground owners.
- `worldInstances.ts`: static buckets, resources, cargo/dynamic visuals, and lifecycle ownership.
- `host.test.ts` and `visualDepth.test.ts`: split by tested owner; migrate interaction source-text
  tests to executed behavior. Share only small fixtures with explicit inputs.
- `index.html`: feature-owned view templates using the existing Vite/DOM stack; preserve generated
  initial DOM, IDs, accessibility, startup size, and source-route discoverability.

Do not split solely at an arbitrary line number or hide large dependency graphs in barrel files.
New or substantially rewritten ownership modules should be ≤ 25 KiB normalized source; all scoped
source/test/template files must meet the existing hard ceilings. Remove all eight exceptions from
`.agent-budget.json`, without adding replacements or increasing thresholds. Remove obsolete renderer
paths only after confirming no runtime, test, contact-sheet, admin, or fallback consumer remains.

Exit: `npm run quality` passes; E1 context probes meet their final budgets; E0 workloads are rerun on
the final commit with raw artifacts committed. Review each target row independently with evidence
and limitations. An unmet hardware, correctness, context, or performance gate leaves that category
below 9/10. Record final measurements in `BENCHMARKS.md` and lasting ownership in `ARCHITECTURE.md`;
remove shipped execution detail from this plan into git history. Resume Phase 9 only after this
programme closes or the user explicitly changes its scope.

#### Execution protocol for the next session

- Start with the first unfinished E-stage. Read its route index, named declarations, nearest tests,
  and only the matching document section. E0 is first; do not start with a scheduler or engine rewrite.
- Each session selects one bounded work package, states its gate, patches, and runs its narrow test
  first. Expand checks only when dependencies/failures require it; run the full quality gate at stage
  exit. Run `npm run agent:map` whenever declarations move or are added.
- Prefer separate behavior-preserving refactor and optimization commits. Do not commit dependencies,
  targets, Wasm build output, or `dist`. Preserve unrelated dirty changes, including the artwork
  deletion observed at planning time. Compare against actual execution HEAD, not an assumed clean tree.
- Update this section's current-stage status with completed scope, exact checks/report paths, open
  failures, and next bounded action. Keep only the current handoff here; completed detail belongs in
  commits. Do not claim completion from a passing microbenchmark alone.

Current handoff: **E0 in progress; E1–E7 not started; no scores awarded**. Execution began at
`00818b9`, preserving the supplied programme and unrelated `docs/art/world-shape-still.png` deletion.
The first bounded E0 package was instrumentation and the active/idle reference-size baseline
(`8db0bf2`). The second is the blocked-then-reopened workload (`df3eb43`) and its recorded
reference-size distributions. The third is the dense junction workload (`bb41b8b`), its
collection-time shape guard (`3fea3c2`) and its recorded distributions. No simulation, save, balance,
gameplay or visual optimization landed.

**E0 is not complete, and E1 has not begun.** E0 exits only when all six of its numbered steps are
met; two are open entirely and three are partial. Do not start E1, and do not optimize, until this
list is closed. Status against the stage's own steps:

1. Record environment, HEAD, dirty tree, toolchain and one full quality run, distinguishing
   pre-existing failures — **done**: `environment.json`, `startup-initial.json`, `quality-final.txt`.
2. Extend the ladder with named workloads at 768 / 3,072 / 6,144 and a diagnostic 24,576 tier —
   **partial**: active, idle, blocked-then-reopened and dense junctions exist, are pinned in CI and
   are recorded, but powered production under full and insufficient supply, separate outposts with
   one edited component, and mixed extraction, regrowth, river pumping and disturbed water do not
   exist. Only the 6,144 tier has been collected; the other three sizes have not.
3. Live browser scripts — **not started**: none of the 100 place/rotate/erase cycles, the
   sixty-second extraction run, the 100-chunk survey, the 100 ground stamps near water, or the camera
   pass through outposts exists, in native-command or UI form.
4. Measure every span end to end — **partial**: the native tick and advance/encode paths are
   measured, and the renderer's `setSnapshot` omission is corrected without relabelling history, but
   worker round trip, decode/merge, application update, frame preparation and submission, GPU
   execution and input-to-visible latency are not, and the real application path is not timed.
5. Five warmed thirty-second runs per workload, with contamination rejected for a stated reason —
   **partial**: done for every native workload, and the junction record is rejected on its own
   evidence rather than kept quietly, but allocations, heap, draw calls, triangles and worst
   interaction are unmeasured, and thermal contamination stays uncertified on a host with no readable
   sensor.
6. Deterministic counters for visited entities and edges, rebuilt graph members, dirty records,
   meshes rebuilt, instance slots written and panel updates — **not started**: only publication dirty
   marks exist, which is why this record can say a jammed factory costs more per tick and a routed
   one publishes more, but not why.

- Completed: individual native tick and advance/encode distributions for the active, idle, blocked
  and junction workloads; five independent runs each, with five-second thermal warmups and
  thirty-second sample windows; setup and the reopening edit separated; raw samples,
  production/checksums and dirty-mark counts retained. A blocked run reports its blocked and reopened
  regimes as named phases with independent distributions. Fixture tests pin useful work, resting
  state, saturation to a byte-identical fixed point, resumption to at least the active production
  rate, replay identity and timing arithmetic; the packer recomputes every phase percentile and
  requires a saturated phase to have published nothing. The junction workload is a genuinely routed
  factory rather than a chain relabelled: a twenty-four entity unit, six of them junction primitives,
  in which four materials merge into one trunk through three chained mergers, cross beneath an
  independent fifth lane through an underpass pair, and fan out at a splitter. It oversubscribes the
  trunk on purpose (0.254 offered against 0.2 carried) so arbitration is a standing contest rather
  than a conflict-free schedule, and one material per lane makes `delivered_by_item` an exact
  per-lane meter. Tests pin the compiled graph, each lane's production, the trunk's sum against a
  belt's rate, an empty crossing under the underpass, and an even split where entity-id order would
  starve a lane to zero; a collection re-counts the tier's mergers, crossings and splitters outside
  every sample span. The browser harness measures both snapshot setters separately and labels
  historical sums honestly. Its quick-ladder smoke test passed, but concurrent-build timings were
  rejected.
- Validation: initial `npm run quality` passed (254 TS / 121 Rust); the current recheck passed
  (255 TS / 133 Rust, plus 14 context tests, audit, map, format, lint, typecheck, build and
  startup). Full output: `docs/benchmarks/e0/quality-final.txt`. Initial environment and payload:
  `docs/benchmarks/e0/environment.json` and `startup-initial.json`. Production asset hashes stayed
  unchanged. Measurement source/binary identity, now one entry per collection:
  `docs/benchmarks/e0/measurement.json`.
- Reports and method: `docs/BENCHMARKS.md` and `docs/benchmarks/e0/{active,idle,blocked,junction}-6144`
  raw/summary pairs. The schema-1 active/idle records taken at `8db0bf2` were re-collected under the
  phase-aware packer rather than kept unverifiable. Thermal sensors and unrelated-load monitoring are
  still not recorded; these raw measurements remain provisional, not a certified E0 exit.
- **Missed budgets, recorded not relaxed**: at 6,144 entities the 3,000 µs advance/encode p95 ceiling
  is exceeded by every blocked run (3,001–3,045 whole-window, 3,335–3,431 in the reopened phase) and
  by every junction run by 31–80% (3,920–5,396), and the 1,000 µs tick p95 ceiling is exceeded in one
  blocked phase (1,000.7) and in junction runs 3–5 (1,129–1,201). Active clears the encode ceiling by
  0.8–2.4%. E4 owns these; E0 does not optimize. A jammed factory also costs more per tick than a
  producing one, and a routed one publishes 2,480 entity marks per tick against a line's 1,041 —
  which the counters E0 still owes must explain before anything is changed.
- **Open failure, not deferred silently**: the junction collection is rejected as a percentile
  baseline and kept for its structure. Its five runs deliver an identical 59.74 items per tick, yet
  runs 3–5 are 8–25% slower than runs 1–2, an ordering a second cold collection reproduced; the rate
  wanders in both directions inside a run, so it is the host's sustained clock, not the factory
  settling. The lighter active and blocked collections spread only 3–6%. The encode miss is far
  larger than the spread and survives; the junction tick p95 result does not and is stated as a
  range. This host exposes no thermal sensor, so the cause is unestablished.
- Open gates beyond the six steps above: Wasm distributions as well as native; a junction collection
  whose runs agree closely enough to be a baseline; profile and DPR coverage; the startup timing
  matrix; and actual integrated-GPU validation — the RTX 3060 is not integrated-GPU evidence. All
  existing budgets remain unchanged. Separately, the quality gate compiles Rust only with `cfg(test)`
  and through a cached `wasm-pack` build, so a warning in the shipped profile can pass it, and clippy
  across all targets currently reports 61 pre-existing warnings; that is its own bounded package.
- Next bounded action, continuing step 2: add the powered-production workload — full supply and
  insufficient supply as two measured regimes — to `factory-wasm/src/capacity/`, with executable
  assertions that machines actually run on metered power and that the deficit regime throttles rather
  than stops. Reuse the collector, the named-phase report, the fixed-clock tests and the
  collection-time shape guard. Then the outpost and ecology shapes, then the three uncollected tier
  sizes, then steps 3, 4 and 6. E1 begins only after E0 exits; nothing here is optimized meanwhile.
  The complete programme remains unfinished.

### Phase 9 — Living Lattice

Create one sparse ecological loop rather than a catalogue of decorative resources.

- Derive a scarce fertile-riverbank ground tag from native drainage, elevation, and water state.
- Add deterministic animal populations that move, feed, breed toward local carrying capacity,
  migrate, recover, and can collapse when overharvested. Use sparse schedules or active fronts.
- Produce useful biomatter and a waste stream with at least two visible responses: recovery/refining
  and habitat damage. Reuse existing joint-output routing and costing.
- Show population health and the consequence of extraction early enough for the player to react.
- Add the first finite hub ecology programme; guidance must derive an executable route to its bill.
- Extend the generated art vocabulary only as needed to make habitat and population state readable.

Gate: the same installation has different, legible outcomes in healthy and damaged habitat; recovery,
migration, and collapse reproduce exactly across saves and checksums; all new definitions enter the
balance fixture.

### Phase 10 — The primitive human

Give the player needs only where they create factory demand.

- Build a food chain from phase 9: forage or harvest, grow, cook, preserve, store, and distribute.
- Add bounded native needs and attributes with exact effects. Failure narrows options recoverably;
  idle decay and death spirals are out of scope.
- Reconcile attributes with Carrying, Construction Reach, Surveying, and Travel Pace. One benefit may
  not be purchased through two progression currencies.
- Keep the player clock separate from the factory clock and decide explicitly which needs advance on it.
- Add a finite hub provision programme that uses the food system without becoming a repeatable chore.

Gate: each need names something worth building, a playtest shows that the player builds it because of
the need, and existing saves migrate without losing earned capability.

### Phase 11 — Supported floors and vertical transport

Ship ground plus one useful upper floor before expanding vertically.

- Represent position as axial cell plus explicit level; grade and level remain separate.
- Add definition-driven supports, loads, spans, floors, roofs, columns, stairs, shafts, and belt lifts.
  Recompute only regions affected by edits; never surprise-collapse or lose inventory.
- Compile cross-level transport through explicit endpoints. Adjacent cells on different levels never
  connect implicitly, including pipes.
- Provide a layer-aware view and picking: active level, faded context, visible openings and destinations.
- Complete the structural enclosure family with reinforced concrete and steel frames when their load
  decisions are needed.
- Introduce named machine faces for ports, heat, exhaust, or control only where direction creates a
  readable routing choice.
- Keep current deterministic pipe transport unless vertical fluid routing proves that pressure adds a
  clear decision. If it does, pressure and flow belong here as one bounded graph system.
- After one upper floor is readable and measured, underground may use separate sparse strata joined by
  explicit shafts. It is not a voxel world.

Before rendering the feature, add a deterministic stacked-floor/lift capacity tier and rerun all three
browser profiles. Gate: a useful stacked factory can be edited at normal zoom with conserved cargo,
validated loads and removals, exact restore, and performance inside the recorded target.

### Phase 12 — Regional Discovery

Turn existing large-scale variation into reasons to travel and establish outposts.

- Make advanced materials and ecological opportunities belong to recognisable regions while every
  preset remains completable.
- Add survey tools, home bearing, distant sites, and specialized outposts without revealing unsurveyed
  terrain.
- Finish organic generated seams and add biome flora and props as sparse instanced presentation. Props
  never occupy cells or enter saves and checksums.
- Add water populations using phase 9's population model, then a shore-straddling harbour and working
  vessels when a distant water site makes the route worthwhile.
- Add player-chosen hub programmes whose visible modules create sustained regional demand rather than
  random repeatable jobs.
- Use signal crystal for face/ring control only if the shipped factory has a concrete signal problem.

Gate: entering a region is recognisable without a menu, the survey records its extent and access, every
preset remains completable, and at least one hub programme requires a sustained distant site.

### Phase 13 — Day and resilient power

Add time variation only after regional factories make local power strategy meaningful.

- Ship a day cycle and solar generation together; the cycle exists for atmosphere and play, not as a
  hidden power prerequisite.
- Ship intermittent generation and accumulators together. Output is a deterministic function of tick,
  position, and published world state, never a runtime roll.
- Let regional conditions change the useful mix without making one preset strictly dominant.

Gate: the player can predict generation, size storage from visible information, and restore the same
power outcome exactly from a save.

## Active measurements and decisions

- Measure extractor starvation over its seven-cell forestry reach before changing the current
  `regrowth_ticks = 450`; visual recovery pace alone is not enough evidence.
- Keep the generic Extractor until a new machine family creates a distinct decision. Recoloured aliases
  do not deepen the game.
- `DIRECTIONS` remains the six adjacent hexes. Twelve headings are routing/orientation only.
- `fixtures/balance.json` remains the acceptance point for every new building and recipe.
- The river hierarchy moved the opening's geography, and `fixtures/balance.json` records it. Wider
  channels lay more bench and plane some water-adjacent lowland: sand sites 100 → 138, crude oil
  72 → 113, stone 93 → 99, wood 2,895 → 3,058, clay 41 → 23. Every guarantee still stands — coal's
  walk improved 16 → 15 hexes and clay's held — and mean site yields moved by under 5 per cent. Clay
  is the material to watch: it wants lowland within two hexes of water, so it is the first thing a
  change to channel width or bed depth deletes.

Release history and settled implementation reasoning live in git history and tagged releases, not in
this plan.
