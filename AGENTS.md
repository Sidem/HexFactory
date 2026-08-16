# HexFactory agent notes

HexFactory is a deterministic browser factory simulator. Keep this file concise; durable design
decisions live in `docs/ARCHITECTURE.md` and MVP status in `docs/MVP.md`.

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
- Time and quantities are integers. Any blocked transfer leaves its source unchanged.
- Canvas 2D is replaceable presentation. Simulation truth comes only from native snapshots.
- Do not claim large-map performance before a benchmark exists.

## Commands

- `npm run dev` — Vite on port 5174
- `npm run build:wasm` — build `factory-wasm/pkg` with wasm-pack
- `npm run build` — Wasm + typecheck + production Vite build
- `npm run format` / `npm run lint` / `npm run typecheck`
- `npm run test:run` / `npm run test:rust`
- `npm run quality` — complete local gate

Commit both `package-lock.json` and `factory-wasm/Cargo.lock`. Do not commit `node_modules`, Rust
`target`, the generated wasm-pack `pkg`, or `dist`; CI builds them from the locked sources.
