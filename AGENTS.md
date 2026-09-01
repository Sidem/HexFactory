# HexFactory agent entrypoint

HexFactory is a browser factory-automation game. Player experience is the tiebreaker; determinism,
native ownership, sparse cost, and measured claims exist to keep a large living factory responsive
and trustworthy.

Current release: **v0.46.0 Shaped Ground** — an earthworks selection is a shape and two anchors, not a
heap of hexes. Its detail is one ledger line in the plan.

On `main` since, unreleased: **the Phase 8 scale break.** One construction hex is 25 m², altitude is
physical native height, drainage shapes valleys before rivers are placed, and footprints, belt cadence,
the renderer and picking were reauthored around the new scale. Every envelope moved together to save
38, definitions 29, technologies 16, scenarios 8, world 11, wire 21. A save at 36 or below is the old
1 m² world and is refused with an export path rather than migrated; save 37 advances by stamp. On
2026-09-01 the user settled player movement for traversal: `PLAYER_SPEED` is unchanged across the
rescale, so the stated figures moved instead — a 15 m/s walk, a 25 m/s run, a 5 m/s ford. The reasoning
is on the constant in `factory-wasm/src/lib.rs`.

Next work is the numbered phase table in `docs/HEXFACTORY-PLAN.md#what-to-do-next`. Phase 8, flowing
water, is in flight: the user approved its scale break on 2026-08-31, slices 1 to 3 are complete, green
and pushed, and **slice 4 — sparse disturbed water — is the next work**. The plan's "Starting slice 4"
note says what the physical ground already gives you and what the slice must not do. Pipes were brought
forward and the ground rework was requested outright; neither reorders the table. Release numbers after
v0.46.0 are unassigned. Do not reorder those phases without the user. On 2026-08-29 the user moved
supported floors and vertical transport from row 7 to row 10, behind flowing water and Living Lattice,
so the player learns the shipped ground-level systems before the game asks them to think in levels.

## Localize before reading

1. Use the task table at the top of `docs/AGENT-MAP.md`.
2. Find named declarations with `rg -n`; read bounded ranges around matches.
3. Load only the task-specific rule below. The Shipped invariants section of `docs/ARCHITECTURE.md`
   holds the settled decisions; do not read it end-to-end for an unrelated task.
4. Inspect the nearest test before editing. Run its narrow command after the first small patch.
5. Refresh the map with `npm run agent:map` after moving or adding declarations.
6. Expand context only when a compiler, test, or dependency edge names the next file.

Never replace source with lossy summaries. The generated map is a retrieval index; source and tests
remain the authority.

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
- Presentation never becomes simulation truth. Picking names the cell whose drawn surface the pointer
  is over, and native still decides what that cell allows. Ground height is native — a millimetre bed
  plus the paid integer grade — and only the drawn surface between quanta is presentation. Lists
  containing controls are keyed and patched in place.
- No performance or scale claim without a committed measurement from the relevant harness.
- Preserve user changes in a dirty tree. Never source-import HexLife or edit its checkout unless a
  separate task explicitly authorizes a generic package release.

## Load on demand

- Goal, milestone, gameplay loop: the relevant phase brief in `docs/HEXFACTORY-PLAN.md`.
- Tick, transport, power, player, ecology: `docs/ARCHITECTURE.md`, then the matching bullet under
  its Shipped invariants.
- Belts, junctions, underpasses, upgrades: localize with
  `rg -n "drag|axis|junction|underpass|upgrade" docs/ARCHITECTURE.md`.
- World generation, deposits, terrain: localize with
  `rg -n "site lattice|bootstrap|terrain|deposit" docs/ARCHITECTURE.md`.
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
- `npm run terra` — the Phase 8 drainage prototype's survey, outside CI; `--coast` samples a
  shoreline, because the origin is wherever the seed put it and is often seabed
- `npm run quality` — complete local gate

Fixture updates remain deliberate review points:

- Wire: `UPDATE_WIRE_FIXTURE=1 cargo test wire_fixture`
- Balance: `UPDATE_BALANCE_FIXTURE=1 cargo test balance_fixture`, then Prettier

## The document set

Six files, each with one job. Architecture and settled rules are `docs/ARCHITECTURE.md`; shipped and
next work is `docs/HEXFACTORY-PLAN.md`; measured evidence is `docs/BENCHMARKS.md`; art direction is
`docs/ART.md`; the retrieval index is `docs/AGENT-MAP.md`; this file is the entrypoint. A shipped
milestone collapses to one ledger line in the plan and its brief is deleted — the detail lives in
git history and in the code. At most one release record may exist at a time, for the milestone
currently in flight; it is deleted when the next one ships. Today there is none: v0.46.0 is shipped
and its detail lives in the ledger and git history. `README.md` is the only player-facing document.

## Evidence behind the context policy

The localize-then-read routine above follows published results rather than taste. Liu et al.,
_Lost in the Middle_ (TACL 2024), found relevant information is used less reliably in the middle of
long contexts: <https://doi.org/10.1162/tacl_a_00638>. Zhang et al., _RepoCoder_ (EMNLP 2023),
reported more than 10% improvement over in-file completion from iterative retrieval:
<https://doi.org/10.18653/v1/2023.emnlp-main.151>. Xia et al., _Agentless_ (FSE 2025), localizes
files, then declaration skeletons, then exact edit regions: <https://arxiv.org/abs/2407.01489>.
Yang et al., _SWE-agent_ (NeurIPS 2024), found an interface designed for code navigation materially
improved repository task performance:
<https://proceedings.neurips.cc/paper_files/paper/2024/hash/5a7c947568c1b1328ccc5230172e1e7c-Abstract-Conference.html>.
Jiang et al., _LongLLMLingua_ (ACL 2024), shows query-aware compression cuts cost and latency;
HexFactory takes the high-information-density principle through deterministic maps and retrieval but
does not automatically delete source tokens: <https://doi.org/10.18653/v1/2024.acl-long.91>. These
results come from different tasks and models. They support the direction — localize, rank, truncate,
validate — not a token-saving percentage for this repository.
