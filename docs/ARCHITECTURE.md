# HexFactory architecture

HexFactory is intentionally not a cellular automaton. The construction surface is an unbounded
pointy-top axial map, while the running simulation follows active transport and scheduled machines.

## Boundary

`@hexlife/embed/hex@1.15.0` is the only HexLife dependency. TypeScript imports its public `/hex`
entry for axial coordinates, clockwise direction tools, placement hit testing, and Canvas centers.
The entry is DOM/Wasm-free and contains no factory semantics. The Rust crate is independently
buildable and pins the same six directions through `fixtures/hex-directions.json`; it never reads
HexLife sources or `node_modules`.

## Native model

The Rust `Core` owns three cooperating representations:

1. Spatial blueprint data stores resource nodes and placed building definition IDs, coordinates,
   orientation, and optional recipe IDs. Coordinates are indexed into floor-divided axial chunks.
2. `compile_graph` resolves each placed output once into a directed entity edge. Turns are simply
   belts whose output direction differs from their incoming edge. The MVP recompiles the complete
   blueprint after an edit.
3. Sparse entities keep cargo slots, inventories, recipe progress, extractor cadence, and consumer
   totals as separate integer fields. Runtime transfers follow compiled edges, and machine work is
   evaluated only for machine entities.

Transfer proposals are ordered by stable entity ID; each target accepts at most one proposal per
tick. IDs for a loaded blueprint derive from sorted coordinates, so JSON insertion order cannot
change arbitration. A rejected proposal does not mutate its source, which is the backpressure
contract. Containers use a `BTreeMap<item_id, quantity>` and release the lowest available dynamic
item ID first. Checksums fold sorted integer state with FNV-1a.

Items, recipes, and building identities are dynamic IDs loaded from JSON. Native component code
implements the MVP behaviors from validated definitions; adding definition data never resizes a
global transition table or creates a JavaScript callback.

## Host and rendering

`FactoryHost` initializes Wasm, sends bounded tick/edit commands, and parses one compact JSON
snapshot per rendered update. The animation loop calls one native `tick(count)`; it never walks map
cells or cargo to simulate them. `CanvasFactoryRenderer` is behind a small snapshot interface and
may be replaced by an instanced GPU renderer without changing simulation state.

`window.__hexFactory` exposes snapshot, step, and reset for headless browser verification. The Rust
library itself has no DOM dependency and all deterministic tests run natively with `cargo test`.

## Current cost boundary

The MVP intentionally recompiles the full small blueprint after place/erase/rotate and serializes a
full small snapshot for rendering. Incremental connected-component graph recompilation and dirty
snapshot deltas are explicit next gates. No large-scale performance claim is made yet.
