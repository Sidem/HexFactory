# HexFactory architecture

HexFactory is an unbounded continuous-world game with a sparse deterministic factory simulation.
Pointy-top axial cells anchor construction, ground, and transport; the player moves in fixed-point
world space. Runtime work follows active entities and compiled graph edges, never a universal cell loop.

The product direction is in [`HEXFACTORY-PLAN.md`](HEXFACTORY-PLAN.md). This document states how the
current system works and the boundaries future work must preserve.

## System boundary

| Owner            | Responsibility                                                                                                                                               |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Rust/Wasm `Core` | World generation, ground and water, player state, inventories, recipes, research, objectives, construction, transport, ticks, saves, checksums, and legality |
| Module worker    | Ordered Wasm operations, bounded command/tick batches, save/load, previews, and snapshot emission                                                            |
| TypeScript host  | Input intent, UI, camera, rendering, snapshot merge, and browser persistence of opaque saves                                                                 |
| Data catalogues  | Items, recipes, buildings, technologies, scenarios, costs, effects, and art keys                                                                             |

Presentation never becomes simulation truth. TypeScript may request an action and draw the answer; it
may not update a quantity, position, progress value, legal state, or checksum itself.

`@hexlife/embed/hex@1.15.0` is the only HexLife dependency. TypeScript uses its public DOM-free axial
geometry API. Rust independently pins the same direction fixture. Factory semantics never enter that
package, and package internals are never imported.

## Core invariants

- Rust/Wasm owns every running tick and result. No per-cell, per-item, or per-machine JavaScript loop.
- State dimensions remain separate: definition, identity, position, orientation, cargo,
  compartments, recipe, progress, power, and scenario ownership are not flattened into combined types.
- Definition identities are dynamic integers. New content is data unless it introduces a genuinely new
  source or simulation behavior.
- Time, quantities, rates, coordinates, and arbitration are integer and deterministic. Stable entity IDs
  break conflicts; collection iteration order never does.
- Unbounded space is chunked and lazy. Empty space and idle entities have negligible cost.
- Derived indexes and caches are rebuilt. They are never saved, hashed, or checksummed.
- Performance claims cite [`BENCHMARKS.md`](BENCHMARKS.md); balance claims cite generated fixtures.

Direction 0 is east, followed clockwise by E/SE/SW/W/NW/NE. `DIRECTIONS` is always the six adjacent
cells and is pinned by `fixtures/hex-directions.json`. Twelve transport/orientation headings do not
widen adjacency.

## World, ground, and water

World generation is a pure function of generator version, seed, parameters, and coordinate. Surveying
adds generated chunks; it does not change what a coordinate means. Terrain, resource sites, bed height,
substrate, and equilibrium hydrology are derived and cached only for speed.

A deposit is a site on a jittered lattice, not an independent roll per cell. A site owns one material,
a centre, radius, geographic rule, and core-to-rim yield. The bootstrap pass guarantees required
opening materials or refuses the world. Depletion and regrowth are sparse overlays over that derived field.

One construction hex is 25 m². Native ground keeps these facts separate:

```text
generated bed + earthwork delta + erosion/deposition delta = finished elevation
substrate and prepared surface                         = material state
water depth, surface, and discharge                    = hydrology
resource site                                          = independent field
```

Height uses 0.25 m integer quanta. `GroundSpine` is the generated physical source;
`FinishedGround` is the path to current elevation and access. Movement, construction, picking,
earthworks, pumping, and rendering consume native answers rather than reimplementing the generator.

Drainage is denser than permanent rivers. Generated channels descend through the native drainage
hierarchy; discharge class controls their width and dry alluvial bench, one cell of half-width per
class, so a confluence widens the water the eye sees. Inside its own wetted width a channel has no
bank: the bed climbs to the waterline at a quarter-metre cross grade, and the rock's bank grade starts
outside it. The landing search chooses a dry, walkable, buildable coastal shelf close to sea level and
a set distance back from the surf, without translating world rules differently.

Untouched water is derived equilibrium. `hydrology.rs` saves only `DisturbedWater` departures and
removes them when they return to equilibrium. Earthworks schedule a bounded settle region that cannot
cross the surveyed frontier or generate chunks. Springs and outlets are boundary conditions; local
transfers conserve depth and terminate within explicit structural bounds.

`ecology.rs` derives fertile-riverbank capacity on demand from exact current water, surface, and occupied
footprint: ground is fertile when it is dry and unbuilt and fresh standing water lies in its ring. The
generated alluvial bench only rates that water; a canal the player cuts waters ground exactly as the river
it came from does, so fertility is a ring question and a depth change dirties its neighbours too. The
positive cells travel as a separate sparse habitat patch with zero-capacity tombstones. No habitat cache,
presentation state, or stable equilibrium work enters saves, checksums, or the tick.

Erosion is a sparse geomorphic epoch over surveyed, flowing edges. It stores only non-zero stress and
ground departures. It is not a terrain tick, and it cannot expose or bury resources without an explicit
rule. Loose water in pipes remains factory cargo, not hydraulic terrain state.

## Player and construction

The player owns native fixed-point position, facing, movement intent, route goal, inventory, cursor-held
stack, action cooldown, skills, and scenario capabilities. Walking and field actions use a native player
cadence separate from the 10 tps factory clock. The host submits bounded step counts, never elapsed-time
positions.

Click-to-move saves the goal and derives the fastest bounded route from current native ground, water, and
occupancy. The route is a cache: it is never saved and never searched by the host. Keyboard movement
cancels it immediately.

Gathering, transfer, delivery, research, building, upgrade, erase, and undo are native transactions.
Capacity is computed from item quantities and stack sizes rather than stored as slot objects. A refusal
leaves the source unchanged. Removal returns paid materials and stored state in stable item order; overflow
becomes timed ground cargo.

Placement uses rotated definition footprints and one native occupancy index. A drag is one bounded
endpoints command; native resolves its route, legality, price, and preview. The host never expands it into
per-cell commands. The ground brush is the same contract under a held pointer: one bounded disc per stamp,
carrying the hex the stroke sampled its height from, and each stamp is its own priced transaction that keeps
its full footprint visible when one cell refuses the edit. A stamp that resolves to cut or fill occupies the
native player clock in proportion to that resolved earth volume and commits only when the work finishes;
surface-only stamps remain immediate.

## Factory graph and machines

Blueprint edits compile directed transport edges. Runtime uses the compiled graph and derived indexes;
edits rebuild only affected components when that matches the full deterministic implementation. The full
rebuild remains the correctness oracle.

- Plain belts and pipes move cargo through timed lanes with capacity and backpressure.
- Splitters and mergers are definition flags over the same graph. Saved cursors make arbitration fair and
  deterministic.
- Underpasses add guarded portal edges; crossed cells retain ordinary occupancy for the other lane.
- Configured product routes name an exterior side of a footprint cell per output item.
- Cross-medium rules are explicit: belts carry solids and sealed barrels; pipes carry loose fluid.

Machines reserve exact inputs, charge fuel, advance integer progress, and emit a complete batch only when
its output fits. Ingredient, fuel, output, reserved input, and transit cargo remain separate. Fuel is an
item property, not a recipe input. Multi-output recipes complete atomically and carry explicit cost
allocation totaling 100%.

A new machine normally adds a recipe category and definition row. Add a new native kind only when the
machine obtains or transforms state in a way existing components cannot express. Upgrades edit an entity in
place and preserve its identity, contents, orientation, and graph connections.

## Data, progression, and economy

`src/data/definitions.json`, `technologies.json`, and `scenarios.json` are versioned catalogues validated
by both host and core. References, bounds, acyclic prerequisites, stack/craft divisibility, upgrade ladders,
effects, and placement rules fail at load rather than during play.

Research, personal skills, hub requests, finite projects, ordered contracts, and victory are native.
Guidance derives the next executable action from outstanding bills, recipes, technologies, and power; it
must not outrun the rules it explains.

Economy comparisons expand recipes into raw effort plus fuel. Tier upgrades must cost more than their
parents, and later machines may not undercut earlier machines of the same kind. Rust computes the balance
record from native behavior; TypeScript independently expands catalogue arithmetic. Their committed fixture
is the acceptance point for new content.

Creative mode changes prices, refunds, research access, and administrative commands through one saved native
flag. It does not change recipe time, transport throughput, fuel, power, or payouts, so layouts behave the
same in priced play.

## Saves and compatibility

Rust emits `HXF1` plus a JSON envelope and checksum. The save includes only simulation state and sparse
departures; the browser stores the opaque string in a version-independent catalogue. Save/resume and an
uninterrupted run must converge after equal commands.

Load order is fixed:

1. Parse the original envelope and verify its original checksum.
2. Apply explicit adjacent save-format migrations only.
3. Validate current typed state and the world-generator stamp.
4. Rebuild all derived indexes and presentation baselines.

The 1 m²-to-25 m² scale change and any world-generator change are refusal boundaries, not terrain
conversions. Incompatible saves remain visible and exportable. A release advances only the envelopes it
actually changes; loading never grants, refunds, or silently relabels state.

## Worker and snapshot wire

One dedicated module worker owns the only Wasm `Factory`. Its single operation queue orders commands,
ticks, previews, saves, resets, new games, and loads. Each rendered frame sends at most one command batch,
capped at eight commands, plus bounded factory and player step counts.

The first snapshot is complete. Later snapshots are native dirty deltas with base/next revisions, tick, and
checksum. The host rejects revision gaps before merging. Buildings and resources use sorted per-entity or
per-site patches; unchanged static world data does not cross the worker boundary.

`factory-wasm/src/wire.rs` encodes a transferred binary buffer and
`src/core/snapshotWire.ts` decodes it. Identity-bearing numbers must remain exact in JavaScript. Rust
round-trip tests and `fixtures/snapshot-delta-wire.json` pin the same format; update both deliberately with
`UPDATE_WIRE_FIXTURE=1 cargo test wire_fixture`.

Dirty marks are derived presentation state. Mutation sites mark changed groups or identities; emit compares
them against the last baseline. Reset, new game, and load discard the baseline. Tests compare the sparse
builder with a full-snapshot diff over representative commands.

## Rendering and interface

The production renderer is a replaceable Three.js consumer of snapshots. It uses an orthographic camera,
twelve orbit headings, native physical terrain, instanced generated geometry, bounded quality profiles, and
a WebGL2 minimap. Rendering may interpolate presentation but cannot invent simulation state.

Picking intersects the drawn native height field and names the cell under that surface; native still decides
legality. Fog is the complement of native surveyed chunks. The renderer never draws generated detail as if it
were surveyed or reveals resource fields through a distant view.

Generated machine shapes, item glyphs, terrain materials, state marks, and palette rules live in
[`ART.md`](ART.md). Presentation hashes, animation, props, and procedural material variation never enter a
save or checksum. An item is always rendered through `src/rendering/itemChip.ts`.

UI controls are reconciled by stable keys and patched in place. Replacing a control between pointer-down and
click loses the interaction. Panel choice, camera state, and hotbar arrangement are local presentation
preferences, not game state.

The save UI owns selection and receives its load/export/refresh callbacks at construction. Factory,
resource, and dynamic instance groups release retired instance buffers on replacement. Shared geometry
remains owned by the world instance layer, and shared materials by the renderer, until their disposal.

## Performance and maintainability

The same Rust capacity workload runs natively and in the browser worker. Browser records add RPC, snapshot
merge, world render, and minimap cost. Measurement code is feature-gated and excluded from production.

Every hot-path structure must be sparse or bounded. Add the workload for a new system before adding its
rendering cost, and compare derived/cached behavior with a full or uncached oracle. A world-generator, item
roster, or snapshot-shape change reruns the relevant balance, survey, wire, and capacity records.

`npm run context:check` is an architecture gate. Large files are split by ownership near the behavior and its
tests, not by arbitrary line ranges. Composition roots coordinate modules but do not absorb feature logic.
Generated route indexes locate declarations; they are not an excuse to leave behavior concentrated.

`npm run startup:check` is the other one. The production first load has a stated ceiling for JavaScript, for
Wasm, and for the two together; `src/main.ts` marks `hexfactory:ready` when the shell can answer, so the time
that payload costs is measurable rather than argued. A new client system spends declared headroom or moves
the ceiling on purpose in [`BENCHMARKS.md`](BENCHMARKS.md).
