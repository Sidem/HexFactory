# MVP scope and acceptance

The live page ships one complete native vertical slice:

`resource deposit → extractor → east belt → southeast turn → belt → composer → belt → container → consumer`

The extractor produces item ID 1 (`ore`) every four integer ticks. Recipe ID 1 consumes exactly two
ore, runs for six ticks, and emits one item ID 2 (`component`). The container holds up to eight real
items and releases one per native arbitration step. The consumer increments the native delivered
counter. Occupied outputs and full target inventories apply backpressure without deletion or
duplication.

The browser starts with that working factory and visibly renders machine state, progress, cargo,
container quantity, tick, delivered total, and checksum. Controls cover play, pause, single-step,
reset, four speeds, place, erase, rotate, and new-building orientation.

## Verification gates

- Rust: direction protocol, turning graph, transport conservation, backpressure, exact recipe
  quantities/timing, container quantity/release order, delivery total, reset replay, insertion-order
  independence, and negative chunk division.
- Host: published coordinate/hit-test round trips, direction fixture parity, command encoding, and
  definition validation.
- CI: formatting, lint, strict TypeScript, Vitest, Rust tests, Wasm build, and Vite production build
  all precede Pages deployment.

## Explicit follow-ups

1. Incremental connected-component graph recompilation instead of the MVP full rebuild after edits.
2. Dirty snapshot/delta transport and a Web Worker boundary for larger active factories.
3. Benchmarked capacity tiers before selecting a WebGL instanced renderer or making scale claims.
4. Multiple recipes, ports, splitters, inserters, lanes, power, blueprint codecs, and native
   evolutionary evaluation.
