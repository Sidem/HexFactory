# HexFactory architecture

HexFactory is not a cellular automaton. Exploration uses unbounded continuous fixed-point world
space. Pointy-top axial coordinates exist only for construction anchors/footprints and compiled
transport; the running simulation follows graph edges and sparse scheduled entities.

## Dependency boundary

`@hexlife/embed/hex@1.15.0` remains the only HexLife dependency and is exactly pinned. TypeScript
uses its public, DOM-free entrypoint for clockwise directions, footprint rotation, construction
coordinate conversion, picking, and Canvas centers. The independent Rust crate pins the same
direction fixture and never reads HexLife source or `node_modules`. No package release was required.

## Native ownership

The Rust `Core` owns all state that can change a game result:

1. A versioned seed and coordinate hash generate fixed-size continuous environment chunks without
   shared traversal state. Ordered feature maps hold circular water/rock obstacles and finite
   resource regions; their world coordinates, radii, quantities, collision, and placement legality
   are native state and checksum inputs.
2. The player has native integer `x/y`, facing and bounded movement intent vectors, action cooldown,
   world-unit build range, and an ordered `item_id → quantity` inventory. Every native tick applies
   motion and collision. Gathering, delivery, construction costs, erasing, and research are native.
3. Placed entities keep definition, axial anchor, orientation, cargo, inventory, reserved recipe
   inputs, progress, and scenario ownership separate. Definitions include a bounded axial footprint;
   occupancy, collision, edit targeting, scenario validation, and snapshots rotate the same data.
   Initial entity IDs derive from sorted anchors; later IDs are monotonic.
4. `compile_graph` resolves each entity output into one directed transport edge after edits. Runtime
   transfers use this compiled graph. Proposals sort by stable entity ID and a rejected transfer
   never changes its source.
5. Extractors consume one unit from the finite deposit only when an output can be created. Composers
   reserve exact recipe inputs, run for integer ticks, and emit only on completion. Containers store
   exact quantities; hubs and demo consumers count exact deliveries.
6. The landing hub awards integer insight from data-defined item values. Research prerequisites,
   costs, atomic spending, unlocks, objective progress, and persistent victory all live in Rust.

Blueprint edits retain the previous graph by stable entity ID, invalidate output rays crossing the
changed footprint, and recompile only the affected weak transport components. Component closure
uses both pre-edit links and newly joined targets, so placement/rotation merges and removal splits
match a full deterministic rebuild without resetting machine or cargo state. Full compilation is
still used for scenario initialization and validated save restoration.

## Definitions and scenarios

`definitions.json` contains a version plus dynamic items, recipes, and buildings. Buildings include
construction cost, unlock requirement, placement rule, original host icon metadata, description,
movement collision, and one-to-seven-cell footprints. `technologies.json` contains dynamic IDs, prerequisites, positive integer
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

The replaceable Canvas 2D renderer consumes snapshots and draws continuous regions/resources,
multi-cell buildings, player, hover, selection, build radius, legality, definition labels, and cargo
layers. The construction grid is hidden outside editing unless explicitly toggled. The command bar,
snapshot-derived next-action guidance, inventory/research panels, construction dock, held touch pad,
camera following, pan/zoom, feedback, and reduced-motion behavior are presentation only. Touch and
keyboard movement share the same bounded native intent commands. `@hexlife/embed/hex` performs
construction projection, rotation, and picking; TypeScript does not integrate player motion.

## Save contract

Rust serializes `HXF1` plus JSON containing save/definition/technology/scenario versions, seed,
generated chunks and resource quantities, player and inventory, research, blueprint/entity IDs,
machine and cargo state, counters, tick, victory, and a native checksum. Loading validates versions,
references, uniqueness, and checksum before accepting state. `localStorage` stores only that opaque
string. Save/resume and uninterrupted runs converge on the same checksum after equal commands.

## Current cost boundary

v0.3.1 still serializes a full small snapshot and runs the core on the browser main thread. No
large-map performance claim is made. The ordered follow-ups are a worker boundary with dirty
snapshot deltas and benchmarks that establish capacity tiers before considering WebGL instancing.
