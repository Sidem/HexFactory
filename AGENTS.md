# HexFactory agent notes

HexFactory is a deterministic browser factory simulator. Keep this file concise; the durable
roadmap and implementation handoffs live in `docs/HEXFACTORY-PLAN.md`, architecture decisions in
`docs/ARCHITECTURE.md`, shipped MVP status in `docs/MVP.md`, and measured capacity in
`docs/BENCHMARKS.md`.

## Workspace boundary

- All HexFactory code, plans, and durable project information belong in
  `X:\Programming\Projects\HexFactory`. Begin feature sessions here and read the plan first.
- The source/reference checkout for the published geometry dependency is
  `X:\Programming\Projects\HexLife`. It is not part of this project and is read-only unless a
  separate task explicitly authorizes a generic package release.
- Consume the exact published npm dependency through `@hexlife/embed/hex`; never source-import the
  HexLife checkout or reach into package internals.

## Invariants

- Rust/Wasm owns every running tick: cargo movement, compiled transport, arbitration, machine
  progress, inventories, recipe quantities, delivery totals, and checksums. TypeScript may send
  bounded commands and render snapshots; never add a per-cell or per-item JavaScript tick loop.
- The map is unbounded pointy-top axial space partitioned into lazy storage chunks. It is not
  toroidal. Use the exact public `@hexlife/embed/hex` package for host geometry; never source-import
  HexLife or reach into package internals.
- Direction 0 is east, then clockwise E/SE/SW/W/NW/NE. Rust and TypeScript are pinned by
  `fixtures/hex-directions.json`.
- Building definition, orientation, cargo, inventory, recipe, and progress remain separate fields.
  Items, recipes, and buildings have dynamic integer definition IDs.
- Blueprint edits compile a directed transport graph. Runtime follows graph edges and scheduled
  machines; it does not discover six neighbors for every belt on every tick.
- Arbitration is stable by native entity ID. Initial entity IDs derive from sorted coordinates, so
  JSON insertion order cannot change a run.
- Derived caches never become truth. Resolved extractor deposit references are rebuilt from tiles,
  invalidated when chunk generation adds tiles, and are never saved, hashed, or checksummed.
- Fog of war is presentation over the generated chunk set. Chunk snapshots carry native world
  bounds; the host may draw and describe them but must not invent world outside them.
- Time and quantities are integers. Any blocked transfer leaves its source unchanged.
- Canvas 2D is replaceable presentation. Simulation truth comes only from native snapshots.
- Every performance or scale claim must cite a measured tier in `docs/BENCHMARKS.md`. Claims beyond
  the recorded ladder, and browser-side claims of any kind, are not yet supported.

## Commands

- `npm run dev` — Vite on port 5174
- `npm run build:wasm` — build `factory-wasm/pkg` with wasm-pack
- `npm run build` — Wasm + typecheck + production Vite build
- `npm run format` / `npm run lint` / `npm run typecheck`
- `npm run test:run` / `npm run test:rust`
- `npm run bench` — native capacity ladder; deliberately outside the gate, since shared runners do
  not produce comparable timings
- `npm run quality` — complete local gate

Commit both `package-lock.json` and `factory-wasm/Cargo.lock`. Do not commit `node_modules`, Rust
`target`, the generated wasm-pack `pkg`, or `dist`; CI builds them from the locked sources.
