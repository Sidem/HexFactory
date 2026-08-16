# HexFactory architecture

HexFactory is not a cellular automaton. Its construction surface is unbounded pointy-top axial
space, while the running simulation follows compiled transport edges and sparse machine entities.

## Dependency boundary

`@hexlife/embed/hex@1.15.0` remains the only HexLife dependency and is exactly pinned. TypeScript
uses its public, DOM-free entrypoint for clockwise directions, coordinate conversion, picking, and
Canvas centers. The independent Rust crate pins the same direction fixture and never reads HexLife
source or `node_modules`. No v0.2 package release was required.

## Native ownership

The Rust `Core` owns all state that can change a game result:

1. A versioned seed and coordinate hash generate fixed-size chunks without shared traversal state.
   Generated chunk and tile maps are ordered. Terrain, finite resource kind/quantity, collision, and
   placement legality are native state and checksum inputs.
2. The player has native axial position, facing, integer action cooldown, build range, and a real
   ordered `item_id → quantity` inventory. Movement, gathering, delivery, construction costs,
   erasing, and research are native commands.
3. Placed entities keep definition, orientation, cargo, inventory, reserved recipe inputs, progress,
   and scenario ownership separate. Initial entity IDs derive from sorted coordinates; later IDs
   are monotonic.
4. `compile_graph` resolves each entity output into one directed transport edge after edits. Runtime
   transfers use this compiled graph. Proposals sort by stable entity ID and a rejected transfer
   never changes its source.
5. Extractors consume one unit from the finite deposit only when an output can be created. Composers
   reserve exact recipe inputs, run for integer ticks, and emit only on completion. Containers store
   exact quantities; hubs and demo consumers count exact deliveries.
6. The landing hub awards integer insight from data-defined item values. Research prerequisites,
   costs, atomic spending, unlocks, objective progress, and persistent victory all live in Rust.

The full small graph is recompiled after an edit. Entity machine/cargo state is preserved; an edit
does not reset the simulation. Incremental connected-component recompilation remains the next graph
performance gate.

## Definitions and scenarios

`definitions.json` contains a version plus dynamic items, recipes, and buildings. Buildings include
construction cost, unlock requirement, placement rule, original host icon metadata, description,
and movement collision. `technologies.json` contains dynamic IDs, prerequisites, positive integer
costs, descriptions, and definition unlocks; both host and core validate the DAG and references.

`scenarios.json` defines the default **New game** and retained **Factory demo**. The default seed has
guaranteed nearby finite ore and crystal plus deterministic generated terrain. The hub and all demo
objects are scenario-owned and cannot be erased.

Erasing a player-built entity uses one fixed refund policy: return 100% of its construction cost,
plus its cargo, inventory, and reserved recipe inputs. This is native and covered by conservation
tests.

## Command and presentation boundary

`FactoryHost` sends at most one JSON command array per rendered frame, capped at eight commands by
both host and core. Native ticks are a separate bounded call. TypeScript does not update player
coordinates, quantities, insight, research, machines, cargo, or victory.

The replaceable Canvas 2D renderer consumes snapshots and draws ordered terrain, deposit, building,
belt, cargo, player, hover, selection, and legality layers. Camera following, pan/zoom, cargo
interpolation, responsive panels, feedback, and reduced-motion behavior are presentation only.
`@hexlife/embed/hex` performs camera-aware projection and picking.

## Save contract

Rust serializes `HXF1` plus JSON containing save/definition/technology/scenario versions, seed,
generated chunks and resource quantities, player and inventory, research, blueprint/entity IDs,
machine and cargo state, counters, tick, victory, and a native checksum. Loading validates versions,
references, uniqueness, and checksum before accepting state. `localStorage` stores only that opaque
string. Save/resume and uninterrupted runs converge on the same checksum after equal commands.

## Current cost boundary

v0.2 deliberately serializes a full small snapshot and runs the core on the browser main thread.
No large-map performance claim is made. The ordered follow-ups are incremental connected-component
graph recompilation, a worker boundary with dirty snapshot deltas, and benchmarks that establish
capacity tiers before considering WebGL instancing.
