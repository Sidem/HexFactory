# HexFactory agent entrypoint

HexFactory is a browser factory-automation game. Player experience is the tiebreaker; determinism,
native ownership, sparse cost, and measured claims exist to keep a large living factory responsive
and trustworthy.

Current release: **v0.27.0 Transport Kits**. **Immediate next work: progression and construction**
in `docs/PROGRESSION-PLAN.md` and `docs/CONSTRUCTION-MATERIALS-PLAN.md`, ordered by
`docs/HEXFACTORY-PLAN.md#what-to-do-next`. Complete this workstream before Living Lattice,
Regional Discovery or other roadmap features unless the user changes priority. Release numbers
after v0.27.0 are unassigned. Phase 1 is still active: essential bills for the remaining stations,
commissions, progression definitions and timed opening validation. Envelopes: save 19,
definitions 17, technologies 8, scenarios 5, world 8, wire 12.

## Localize before reading

1. Use the task table at the top of `docs/AGENT-MAP.md`.
2. Find named declarations with `rg -n`; read bounded ranges around matches.
3. Load only the task-specific rule below. `docs/ENGINEERING-RULES.md` preserves the detailed
   shipped decisions; do not read it end-to-end for an unrelated task.
4. Inspect the nearest test before editing. Run its narrow command after the first small patch.
5. Refresh the map with `npm run agent:map` after moving or adding declarations.

The evidence and limits of this context policy are in `docs/AGENT-CONTEXT.md`.

## Always-load invariants

- Rust/Wasm owns every running tick, quantity, inventory, recipe, progress, player position,
  arbitration result, and checksum. TypeScript sends bounded commands and renders snapshots.
- The world is unbounded pointy-top axial space in lazy chunks. Direction 0 is east, then clockwise
  E/SE/SW/W/NW/NE; `fixtures/hex-directions.json` pins Rust and TypeScript.
- Runtime transport follows compiled graph edges. A drag is one bounded endpoints command and its
  preview uses the same native resolver. Never add host traversal or per-item/per-cell JS ticks.
- Time and quantities are integers. Stable native entity IDs decide arbitration. A blocked transfer
  leaves its source unchanged.
- Definitions use dynamic integer IDs. Identity, orientation, cargo, inventory, recipe, and progress
  remain separate fields. New processing machines prefer data categories over new tick branches.
- Snapshot deltas are dirty-tracked native state encoded by `factory-wasm/src/wire.rs` and decoded by
  `src/core/snapshotWire.ts`. Identity-bearing numbers must stay within JavaScript's exact range.
- Derived indexes and caches are never saved, hashed, or checksummed. Rebuild them after the source
  state changes and pin them against the uncached/full implementation.
- Presentation never becomes simulation truth. Picking uses the logical axial plane; terrain height
  remains visual. Lists containing controls are keyed and patched in place.
- No performance or scale claim without a committed measurement from the relevant harness.
- Preserve user changes in a dirty tree. Never source-import HexLife or edit its checkout unless a
  separate task explicitly authorizes a generic package release.

## Load on demand

- Goal, milestone, gameplay loop: the relevant milestone in `docs/HEXFACTORY-PLAN.md`.
- Tick, transport, power, player, ecology: `docs/ARCHITECTURE.md`, then matching headings in
  `docs/ENGINEERING-RULES.md`.
- Belts, junctions, underpasses, upgrades: localize with
  `rg -n "drag|axis|junction|underpass|upgrade" docs/ENGINEERING-RULES.md`.
- World generation, deposits, terrain: localize with
  `rg -n "site lattice|bootstrap|terrain|deposit" docs/ARCHITECTURE.md docs/ENGINEERING-RULES.md`.
- Costs, cadence, power, recipes: run `npm run balance`; read `fixtures/balance.json` and the
  matching balance rule.
- Shapes, machine art, terrain materials: `docs/ART.md` Stage D and the relevant rendering route.
- Save or envelope: `factory-wasm/src/save_migrations.rs` and the Save contract in
  `docs/ARCHITECTURE.md`.
- Snapshot or wire: Worker/snapshot and Dirty tracking in `docs/ARCHITECTURE.md`, plus the wire
  fixture.
- Performance: the current record, method, limits, and follow-ups in `docs/BENCHMARKS.md`.

## Workspace and dependency boundary

- Work only in `X:\Programming\Projects\HexFactory` for this project.
- Repository: `https://github.com/Sidem/HexFactory`; production base `/HexFactory/`.
- Use exact `@hexlife/embed/hex@1.15.0` public APIs. `X:\Programming\Projects\HexLife` is read-only.
- Commit both lockfiles. Never commit `node_modules`, Rust `target`, wasm-pack `pkg`/`pkg-bench`, or
  `dist`.

## Commands

- `npm run dev` — Vite on port 5174
- `npm run agent:map` / `npm run agent:map:check`
- `npm run build:wasm` / `npm run build`
- `npm run format` / `npm run lint` / `npm run typecheck`
- `npm run test:run` / `npm run test:rust`
- `npm run bench` / `npm run bench:browser` — measured capacity, outside CI
- `npm run survey` / `npm run balance` — native measurement, outside CI
- `npm run quality` — complete local gate

Fixture updates remain deliberate review points:

- Wire: `UPDATE_WIRE_FIXTURE=1 cargo test wire_fixture`
- Balance: `UPDATE_BALANCE_FIXTURE=1 cargo test balance_fixture`, then Prettier

The full historical rule ledger is `docs/ENGINEERING-RULES.md`; architecture is
`docs/ARCHITECTURE.md`, measured evidence is `docs/BENCHMARKS.md`, and shipped/next work is
`docs/HEXFACTORY-PLAN.md`.
