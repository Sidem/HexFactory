# HexFactory agent entrypoint

HexFactory is a browser factory-automation game. Player experience is the tiebreaker. Current state
and phase order live in `docs/HEXFACTORY-PLAN.md`; never reorder phases without the user.

## Start here

1. Choose the task route at the top of `docs/AGENT-MAP.md`.
2. Open its small `.agent/` domain index, then find named declarations with `rg -n`.
3. Read bounded ranges around those declarations and the nearest test before editing.
4. Load only the matching rule or phase section from the documents named by the route.
5. Run a narrow check after the first patch; expand only when a dependency or failure names it.
6. Run `npm run agent:map` after moving or adding declarations.

Source and tests are authoritative. Generated indexes only locate them.

## Invariants

- Rust/Wasm owns every running tick, quantity, inventory, recipe, progress, player position,
  arbitration result, checksum, ground height, and water state. TypeScript sends bounded commands and
  renders snapshots.
- Space is unbounded pointy-top axial chunks. Direction 0 is east, then clockwise E/SE/SW/W/NW/NE;
  `fixtures/hex-directions.json` pins both languages.
- Transport follows compiled native graph edges. A drag is one bounded endpoints command and preview
  uses the native resolver. Never add host traversal or per-item/per-cell JavaScript ticks.
- Time and quantities are integers. Stable native entity IDs arbitrate. A blocked transfer leaves its
  source unchanged.
- Definition IDs are dynamic integers. Identity, orientation, cargo, inventory, recipe, and progress
  remain separate fields; prefer data categories over new tick branches.
- Snapshot deltas are native dirty state encoded by `factory-wasm/src/wire.rs` and decoded by
  `src/core/snapshotWire.ts`. Identity-bearing numbers stay exact in JavaScript.
- Derived indexes and caches are not saved, hashed, or checksummed. Rebuild and test them against the
  uncached/full implementation.
- Presentation is never simulation truth. Picking names the cell under the drawn surface; native
  decides legality. Key and patch lists containing controls in place.
- Performance and scale claims require a committed measurement.

## Boundaries and checks

- Work only in this repository. HexLife is read-only unless separately authorized.
- Use exact `@hexlife/embed/hex@1.15.0` APIs. Production base is `/HexFactory/`.
- Preserve dirty-tree changes. Do not commit dependencies, Rust targets, wasm-pack output, or `dist`.
- `npm run context:check` enforces agent-context debt; `npm run quality` is the complete local gate.
- Wire fixture: `UPDATE_WIRE_FIXTURE=1 cargo test wire_fixture`.
- Balance fixture: `UPDATE_BALANCE_FIXTURE=1 cargo test balance_fixture`, then Prettier.

The permanent document set is `AGENTS.md`, `README.md`, and the five files under `docs/` named by the
route map. Shipped detail belongs in git history; only current rules, plans, art, and measurements stay.
