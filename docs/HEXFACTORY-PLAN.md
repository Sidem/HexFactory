# HexFactory — architecture, roadmap, and implementation handoffs

Status: Playable Game v0.2 implemented 2026-08-16; release verification is recorded after deployment.

Target repository: `https://github.com/Sidem/HexFactory`

Target live MVP: `https://sidem.github.io/HexFactory/`

Project root: `X:\Programming\Projects\HexFactory`

Published geometry dependency: `@hexlife/embed@1.15.0` (exact pin).

Local source/reference checkout for that npm package: `X:\Programming\Projects\HexLife`. HexLife is
not a source dependency: HexFactory imports only the published package. Treat that checkout as
read-only unless a future task explicitly authorizes a separately released generic package change.

## Shipped implementation record

- Generic prerequisite: `@hexlife/embed@1.15.0`, tag `embed-v1.15.0`, HexLife merge `37f3c63`.
- Factory repository: `https://github.com/Sidem/HexFactory` (`main` head `cf3d154`).
- Live MVP: `https://sidem.github.io/HexFactory/`, deployed by Actions run `31947910003`.
- The shipped slice keeps the approved boundary: `/hex` is the only HexLife dependency; factory
  simulation is an independent Rust/Wasm crate with compiled transport and native machine state.
- First follow-ups: incremental connected-component recompilation, worker + dirty snapshot deltas,
  then benchmarked capacity tiers before any renderer or scale claim.

## Product decision

HexFactory is a deterministic, browser-native, hexagonal factory-automation simulator designed for
large sparse maps, arbitrary data-defined items and recipes, headless evolutionary evaluation, and
eventual mod-extensibility.

HexLife is the engineering reference and `@hexlife/embed` is a narrow public dependency, but neither
is the factory simulation kernel. Reuse its successful patterns: Rust/Wasm hot paths, workers,
integer determinism, explicit snapshots/checksums, batched boundary crossings, dirty rendering, hex
geometry experience, reproducible builds, and isolated artifacts. Do **not** extend `WorldK`, encode
factories as CA state combinations, or make HexFactory depend on HexLife source files. A factory is
not a uniform local cellular rule.

The spatial map is a construction and rendering surface. The running simulation compiles placed
tiles into transport networks and sparse scheduled entities. Runtime work should follow active
cargo, due machines, and network changes—not every cell in the map.

## Shipped `@hexlife/embed` dependency contract

HexFactory must consume a real reusable npm surface, not add `@hexlife/embed` as a ceremonial
dependency. The founding prerequisite added the suitable unbounded 2D hex geometry without changing
the fixed row-major binary renderer or finite/toroidal CA engines:

**`@hexlife/embed/hex` provides** a DOM-free, Wasm-free, server-safe entrypoint for unbounded
pointy-top axial hex geometry. Its small frozen contract covers:

- one documented clockwise six-direction ordering;
- axial neighbor lookup and rotation;
- axial/cube distance and rounding;
- axial-to-pixel and pixel-to-axial conversion for rendering and hit testing;
- line traversal; and
- negative-coordinate-safe mapping to fixed-size storage chunks.

Names and return shapes must be deliberately designed once, fully typed, and pinned by tests. The
pixel convention, origin, orientation, direction numbering, rounding behavior on boundaries, and
negative chunk division are public behavior—not implementation trivia. Include round-trip,
six-neighbor, six-rotation, distance/line, edge/tie, and negative-chunk fixtures.

The shipped entrypoint received all of HexLife's normal package-boundary edits: source + `.d.ts`,
`vite.embed.config.js`, the package `exports` map, the explicit declaration-copy list in
`scripts/prepare-embed-package.mjs`, `docs/embed/entrypoints.md` plus a dedicated tracked reference
page, and the package README. It passed the embed release gates and was published as version 1.15.0.
HexFactory pins that exact version and imports `/hex` for host coordinates, direction tools,
placement hit testing, and Canvas rendering.

HexFactory's Rust protocol must pin the same direction numbering with cross-language fixtures, but
HexFactory remains independently buildable and owns its axial world IDs. Never reach into
`node_modules` from Rust build scripts and never source-import the HexLife repository.

No other package extension is required for the MVP:

- Do not modify `/sim`, `/ca`, `/stochastic`, or `/hcp` for factory semantics.
- Do not broaden the existing binary `/render` into a multi-layer factory renderer. The MVP owns a
  replaceable Canvas renderer. Only consider a new generic instanced-hex renderer after HexFactory
  has proven a reusable layer/delta contract; do not freeze that API speculatively.
- Do not put belts, recipes, inventories, scheduling, blueprint evolution, or factory codecs in
  `@hexlife/embed`. They belong to HexFactory.

Future package changes follow the same test: add them to `@hexlife/embed` only if they are generic
hex-host primitives with at least one credible non-HexFactory consumer. Implement factory-domain
features in HexFactory even when they happen to be useful to only one demo.

If the playable milestone exposes a genuine gap in the published `/hex` contract, first prove the
feature cannot be implemented cleanly with its existing public API. A blocking addition is
authorized only when it is small, additive, DOM/Wasm-free, broadly reusable hex-host functionality.
Read `X:\Programming\Projects\HexLife\AGENTS.md` and the relevant tracked embed docs, preserve its
unrelated worktree changes, and complete every source, declaration, export, build, declaration-copy,
test, reference-doc, README, changelog, and release edit required by HexLife. Run its relevant gates,
bump only the independently versioned `@hexlife/embed` package, commit and push the scoped change,
publish through the existing `embed-vX.Y.Z` workflow, and verify the npm artifact plus runtime and
TypeScript imports. Then exact-pin the published version in HexFactory and rerun all its gates.

That exception never permits factory/player/terrain/resource/inventory/recipe/technology semantics,
a public direction-convention break, or changes to HexLife's CA engines or renderer. Report such a
blocker instead of bypassing the boundary.

## Non-negotiable architecture

1. **Native hot path.** Rust/Wasm owns cargo movement, machine scheduling, inventories, recipes,
   conflict resolution, production counters, and checksums. JavaScript/TypeScript owns UI,
   rendering, build commands, and bounded orchestration. No per-cell or per-item JS tick loop.
2. **Separate data dimensions.** Building identity, orientation, cargo, item identity, inventory,
   recipe, and progress are separate fields. Never flatten their Cartesian product into one CA
   state byte or lookup table.
3. **Dynamic identities.** Items, recipes, and building definitions use dynamic integer IDs. Adding
   an item or recipe adds definition data; it must not resize a global transition table.
4. **Chunked, non-toroidal space.** Use unbounded axial/cube hex coordinates and lazily allocated
   chunks. A finite viewport is not a finite world contract. Empty map area should cost almost
   nothing.
5. **Compiled transport.** Directional belt tiles compile into directed paths/segments between
   endpoints. The simulation runs the compiled representation; it does not discover six neighbors
   for every belt on every tick. Turns are ordinary path geometry.
6. **Sparse scheduled machines.** Idle extractors, composers, containers, and consumers do not
   execute a universal cell update. Wake entities for due completions, available input, released
   backpressure, power/topology changes, or edits.
7. **Deterministic arbitration.** Simultaneous transfers cannot depend on Rust collection iteration
   order. Use stable entity IDs and explicit priority/round-robin rules. Avoid nondeterministic hash
   iteration in any state-affecting path.
8. **Integer time and quantities.** Core simulation uses integer ticks/fixed-point values. Same
   definitions, blueprint, commands, and tick count must produce the same checksum in browser and
   native tests.
9. **Definitions, not callbacks.** The MVP's behaviors are native components fed by data-defined
   items/recipes/buildings. Do not call JS once per machine, item, or tick. A deterministic bytecode
   escape hatch may be designed later, not improvised now.
10. **Simulation/render separation.** Rendering consumes compact snapshots or dirty deltas and never
    owns simulation truth. A simple MVP renderer is acceptable; it must be replaceable without
    changing the engine.
11. **Headless is first-class.** The same core must run without DOM/WebGL so future evolutionary
    experiments can evaluate many blueprints in workers or Node.
12. **No premature universality claims.** The initial slice proves the architecture; it does not
    claim Factorio feature parity or final performance.

## Long-term model

The intended engine has three cooperating representations:

- **Spatial chunks:** terrain/resources, placed footprints, orientation, selection/picking, and
  local dirty regions.
- **Compiled networks:** belts first; later fluids, power, signals, logistics, and long-range links
  get domain-appropriate network models rather than one universal cell rule.
- **Sparse entities:** stable entity IDs, component-oriented native arrays, inventories, recipes,
  progress, ports, and next scheduled event.

Evolution operates on a blueprint IR—place/remove/rotate/move, route, choose recipe, duplicate or
splice connected modules—not raw dense world bytes. A native evaluator will eventually return
throughput, latency, utilization, waste, footprint, resources, energy, and failure resilience.

## MVP vertical slice

The first live page must show a real native simulation, not an animation mockup:

`resource/extractor -> turning directional belt -> composer -> belt -> container -> consumer`

Minimum behavior:

- one resource deposit and extractor producing `ore` on an integer cadence;
- directional belts that may turn through the six hex directions;
- one data-defined recipe, e.g. `2 ore -> 1 component`, with integer duration;
- a container with a real integer inventory/buffer, not `empty/half/full` display states;
- a consumer that removes components and increments a native delivered counter;
- backpressure: blocked outputs wait without duplicating or deleting items;
- deterministic play, pause, single-step, reset, and speed controls;
- at least a small build/edit interaction: select a tool, place/erase, and rotate directional
  buildings/belts. A polished game editor is not required;
- visible cargo, machine progress/status, container quantity, delivered total, and current tick;
- a prebuilt working factory on initial load so the live URL demonstrates the vertical slice
  immediately;
- a stable checksum for the current simulation;
- the runtime simulates a compiled directed transport representation. For the first small MVP it is
  acceptable to recompile the complete affected blueprint after an edit; incremental connected-
  component recompilation is the next performance gate and must be recorded, not faked.

The MVP may use a straightforward Canvas 2D renderer if that is the shortest path to a correct live
proof. Do not spend the first session rebuilding HexLife's complete WebGL renderer. Keep the renderer
behind a small interface and state explicitly that GPU instancing is follow-up work.

## Suggested repository layout

```text
HexFactory/
  .github/workflows/pages.yml
  docs/ARCHITECTURE.md
  docs/MVP.md
  factory-wasm/
    Cargo.toml
    src/lib.rs
  src/
    core/            # Wasm wrapper, commands, definitions, snapshot adapter
    rendering/       # replaceable MVP renderer
    ui/
    data/            # item/recipe/building definitions
    main.ts
  tests/
  AGENTS.md
  README.md
  LICENSE
  package.json
  package-lock.json
  tsconfig.json
  vite.config.ts
```

Use Vite + TypeScript for the host, Vitest for host-side tests, Rust unit tests for simulation
invariants, and `wasm-pack` for the web artifact. Commit both npm and Cargo lockfiles in this
application repository. Configure Vite's production base for `/HexFactory/`. Pin the newly published
`@hexlife/embed` version exactly (no caret or range) and import its `/hex` entrypoint rather than
copying the TypeScript geometry implementation.

## First implementation gates

Rust tests must cover at least:

- a directed belt path containing a turn compiles in the intended order;
- cargo is neither duplicated nor lost in unblocked transport;
- backpressure preserves cargo and machine output;
- the composer consumes the exact recipe quantities and emits only after its duration;
- the container holds real quantities and releases them deterministically;
- the consumer's delivered count is exact;
- reset/replay produces the same checksum;
- behavior is independent of insertion order for any collection used to construct the blueprint.

Host tests should cover axial coordinate conversion/hit testing, command encoding, and definition
validation. Run formatting/linting, typecheck, Vitest, Rust tests, and a production build locally.

CI must run the same gates before deploying `dist/` through GitHub Pages. Pin tool/action versions
where practical; the existing HexLife Pages workflow is a useful reference but should not be copied
blindly.

## GitHub and delivery requirements

- Check whether `Sidem/HexFactory` already exists before creating it. Never overwrite an existing
  unrelated repository.
- Create a **public** GitHub repository named exactly `HexFactory`, with default branch `main`.
- Add an MIT license and a README whose first section links to the live demo.
- Push the intentional initial implementation to `origin/main`.
- Configure GitHub Pages to deploy via GitHub Actions. If repository Pages must be enabled through
  the API, do so after the first push.
- Wait for CI/Pages, inspect failures, fix them, and verify that
  `https://sidem.github.io/HexFactory/` returns the deployed app—not merely a successful local
  build or workflow dispatch.
- Report the repository URL, live URL, commit, test results, and any explicitly deferred gate.

## Explicitly out of MVP scope

Splitters, inserters, multiple belt lanes or tiers, fluids, power, circuits, trains, enemies,
multiplayer, arbitrary mod bytecode, neural agents, evolutionary UI, massive-map performance claims,
and Factorio asset/content imitation. Use original neutral shapes and names; this is a factory-
automation architecture proof.

## Historical founding-session prompt (completed)

The founding prompt created the repository, published `@hexlife/embed/hex@1.15.0`, implemented the
native factory slice, and deployed the first live page. Its durable results and boundaries are
recorded above; the obsolete prompt itself is intentionally not carried forward as project guidance.

## Next milestone — Playable Game v0.2

The next release turns the architecture proof into a small, complete game. A new game begins in a
deterministic seeded environment with the player beside a landing hub. The core loop is:

`explore → gather → build extraction and transport → deliver → research → compose → win`

Keep the founding prebuilt factory available as a selectable **Factory demo** scenario, but make
the playable new-game scenario the default live experience.

### 1. Deterministic environment and finite resources

- Rust owns a versioned world seed and traversal-order-independent chunk generation.
- The initial terrain vocabulary is ground, water, blocking rock, finite ore, and one finite
  secondary resource such as biomass or crystal. The landing hub and player spawn are deterministic.
- Terrain, resource kind and quantity, collision, and placement legality are native state. The host
  may derive purely decorative variation from the seed but may not invent simulation truth.
- The world remains unbounded and chunk-ready. Generating chunks A then B must produce the same
  state and checksum as generating B then A.

### 2. Native player and inventory

- Add native player position, facing, inventory, action cooldown, and build range.
- TypeScript sends bounded input commands. Rust resolves movement, collision, gathering, costs,
  unlocks, and placement. Browser frame rate must not affect the deterministic result.
- Provide WASD movement with a documented axial mapping, plus pointer or keyboard access to the
  remaining hex directions; interact/gather, hotbar selection, rotate, place, and erase controls.
- The player has a real `item_id → quantity` inventory with exact conservation. Define and test one
  fixed erase-refund policy. Protect the landing hub and scenario-owned objects from deletion.
- Use integer fixed-point movement or a native one-hex movement cadence. Camera following is host
  presentation only.

### 3. Data-defined construction and recipes

- Extend item, recipe, and building definitions with construction costs, unlock requirements,
  placement rules, descriptions, and host icon metadata.
- The playable set includes belts, an extractor that consumes a finite deposit, a container, a
  composer, and the landing hub/consumer.
- Rust rejects unavailable or unaffordable construction even when a forged host command requests
  it. Extractors stop when the underlying deposit is empty.
- Preserve compiled transport. Full graph recompilation after an edit remains acceptable for this
  milestone; incremental connected-component recompilation remains deferred.

### 4. Native, data-defined technology tree

- Add `src/data/technologies.json` with dynamic IDs, prerequisites, integer costs, descriptions,
  and definition unlocks.
- Rust validates unique IDs, existing prerequisites, an acyclic graph, positive costs, and valid
  unlock references.
- Use one coherent research currency: the landing hub awards integer **insight** according to
  data-defined delivered-item values, and the player explicitly spends it. Spending and unlocking
  are one atomic native operation.
- Provide a short progression through Field Logistics, Automated Extraction, and Composition, with
  an optional Storage Planning branch.
- A native objective requires delivery of a defined number of composed components. Reaching it sets
  a persistent victory state while allowing the player to continue.

### 5. Save, load, and scenarios

- Add a versioned native `HXF1` save contract containing definition/scenario version, seed,
  modified chunks and resources, player/inventory, researched technologies, blueprint, machine and
  cargo state, counters, tick, and checksum.
- A loaded save advanced by the same commands must match the uninterrupted checksum. Reject
  malformed or incompatible saves explicitly.
- `localStorage` may store the native save string; TypeScript must not reconstruct simulation state.
- Support New Game, Save, and Continue, plus the retained Factory demo scenario.

### 6. Presentation and usability

- Keep the replaceable Canvas 2D renderer. Add player-follow camera, host-only pan/zoom, and clear
  layers for terrain, deposits, buildings, belts, cargo, player, hover, selection, and placement
  legality.
- Use original neutral geometric art for the player and buildings; do not imitate commercial game
  assets or branding.
- Add a readable HUD for inventory, insight, objective, selected tool, tick, speed, and pause state;
  a technology panel; a cost/unlock-aware hotbar; and a selected-tile inspector.
- Add restrained cargo interpolation and feedback for gathering, placement, research, depletion,
  and victory. Animation never becomes simulation truth.
- Support desktop and narrow layouts, keyboard focus, visible labels, and reduced-motion preferences.

### 7. Architecture gates

- Rust owns terrain, collision, the player, gathering/depletion, inventories, build costs and
  legality, unlocks/research, objectives, saves, transport, machines, cargo, and ticks.
- TypeScript owns input sampling, UI, camera, audio, interpolation, and bounded rendering. Send no
  more than one bounded input batch per rendered frame; add no JavaScript movement, progression, or
  factory simulation loop.
- Keep environment, player, building, orientation, cargo, inventory, recipe, research, progress,
  and presentation as separate dimensions.
- Every state-affecting order is explicit and stable. Chunk visitation, JSON order, and map/hash
  iteration may not alter a checksum.
- Begin with exactly `@hexlife/embed/hex@1.15.0`. `X:\Programming\Projects\HexLife` is reference-only
  unless the controlled generic package prerequisite above is genuinely triggered. Never
  source-import it or reach into internals.
- Preserve the compiled graph, independently headless native core, and prohibition on unbenchmarked
  performance claims.

### 8. Acceptance tests and release gate

Rust tests must add coverage for:

- chunk generation independent of request order, with pinned same-seed and different-seed fixtures;
- six-direction player movement, facing, blocking terrain, and deterministic cadence;
- gathering, finite depletion, and item conservation;
- placement enforcement for terrain, occupancy, range, cost, and technology;
- extractor behavior when its deposit empties;
- research prerequisites, exact atomic spending, unlocks, and rejection of forged locked commands;
- one complete progression path from landing through the native victory objective;
- `HXF1` round-trip and save/resume checksum equivalence;
- insertion-order and chunk-visitation-order independence; and
- all founding transport, backpressure, recipe, container, delivery, and reset invariants.

Host tests must add coverage for bounded keyboard input, absence of a host movement loop, pan/zoom
picking through `/hex`, hotbar costs and locks, technology prerequisites, the expanded snapshot
adapter, native-save delegation, responsive controls, and accessible labels.

Completion requires `npm audit`, formatting, lint, strict typecheck, Vitest, Rust tests, Wasm build,
and production build before deployment. Then wait for GitHub Actions and Pages and verify in a real
browser: new game, movement/collision, gathering, research, construction/editing, running factory,
victory, save/continue, narrow layout, and a clean console.

### 9. Explicitly deferred after v0.2

Enemies, combat, survival meters, multiplayer, networking, fluids, power, circuits, trains, drones,
inserters, splitters, multi-lane belts, broad biome simulation, mod scripting, evolution/neural
features, a WebGL rewrite, large-scale claims, and substantial music/audio production.

## Exact next-session prompt — implement Playable Game v0.2

Copy everything inside the following block into a fresh Codex task:

```text
Work in X:\Programming\Projects\HexFactory. Read AGENTS.md, docs/HEXFACTORY-PLAN.md,
docs/ARCHITECTURE.md, and docs/MVP.md in full, then implement the plan's complete “Playable Game
v0.2” milestone. The goal is a polished but deliberately basic playable starting point from which
we can continue development—not a design update, scaffold, or final attempt at the whole genre.

Follow every scope, architecture, determinism, test, documentation, and acceptance requirement in
the plan. Keep all simulation and progression truth in the headless Rust/Wasm core; TypeScript owns
only bounded input, UI, camera, presentation, and rendering. Preserve compiled transport and consume
only the public, exactly pinned @hexlife/embed/hex package. If its API genuinely blocks the work,
follow the plan's controlled generic package-update and release procedure; never add factory
semantics to HexLife or source-import it.

Implement, test, and integrate the whole playable vertical slice, preserving unrelated work. Run all
local quality gates, commit and push the completed HexFactory work, fix CI and Pages failures, and
wait for deployment. Verify the live game at https://sidem.github.io/HexFactory/ in a real browser
through the planned core progression, editing, victory, save/continue, responsive layout, and a clean
console. Do not stop at a partial implementation, local build, or pending workflow. In the final
handoff report commits and any package release, every gate, deployment and browser verification, the
delivered architecture, and clearly named follow-ups. Do not make unbenchmarked performance claims.
```
