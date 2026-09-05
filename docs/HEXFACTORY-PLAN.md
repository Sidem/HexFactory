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

### Engineering cleanup — complete

E1–E7 closed on 2026-09-05 under the user's reduced scope: one bounded pass for obvious local
issues, using heuristics and existing correctness tests. E0 collection was closed by scope reduction;
its original exhaustive gates were not certified. Numerical scores, token studies, hardware matrices,
and proposed timing targets remain withdrawn. Existing quality/context/startup checks still apply.

| Pass | Outcome                                                                                                                                      |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| E1   | Corrected navigation routes, indexed prototype and async declarations, and removed stale checkpoint guidance.                                |
| E2   | Save UI callbacks are required at construction; removed its deferred binding step.                                                           |
| E3   | Inspected recipe/craft/switch mutation boundaries; no inexpensive ownership restructuring was needed.                                        |
| E4   | Removed copied power-meter maps and a redundant power-source vector without changing allocation order.                                       |
| E5   | Dispose retired factory/resource/dynamic instance buffers while retaining shared geometry and materials; added an execution regression test. |
| E6   | Demo smoke check passed at near/far zoom with orbit, machine picking, and live inspector updates; no drawing redesign needed.                |
| E7   | Corrected obsolete demo instructions, refreshed indexes, and completed validation.                                                           |

Validation: npm run quality passed (256 TypeScript tests, 133 Rust tests, 14 context-tool tests,
format/lint/typecheck, native/Wasm build, and startup budgets). Final guidance edits additionally
passed checkpoint/guidance tests and formatting. Browser smoke also verified save selection and
restore. These are correctness and smoke checks, not new performance measurements.

Remaining broad prototype wiring, rendering rebuilds, and context-debt exceptions are not completion
blockers. Do not reopen an audit or benchmark programme without a concrete problem. Existing raw E0
evidence remains in [BENCHMARKS.md](BENCHMARKS.md), with its limitations intact.

**Next: Phase 9 — Living Lattice.** Phases 9–13 retain their order and content. The unrelated
artwork deletion remains untouched; implementation changes are in the working tree for review.

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
