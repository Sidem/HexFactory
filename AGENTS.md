# HexFactory agent notes

HexFactory is a browser factory-automation game. The goal is a beautiful, open-ended game that is
fun to play and a pleasure to control, inspired by Factorio, Satisfactory, and Minecraft — never
imitating their assets, names, or branding. The deterministic Rust/Wasm core and the sparse
architecture are the means that make that game possible at scale, not the point of the project.

Keep this file concise; the durable roadmap, design pillars, and implementation handoffs live in
`docs/HEXFACTORY-PLAN.md`, architecture decisions in `docs/ARCHITECTURE.md`, shipped MVP status in
`docs/MVP.md`, and measured capacity in `docs/BENCHMARKS.md`.

## Workspace boundary

- All HexFactory code, plans, and durable project information belong in
  `X:\Programming\Projects\HexFactory`. Begin feature sessions here and read the plan first.
- The source/reference checkout for the published geometry dependency is
  `X:\Programming\Projects\HexLife`. It is not part of this project and is read-only unless a
  separate task explicitly authorizes a generic package release.
- Consume the exact published npm dependency through `@hexlife/embed/hex`; never source-import the
  HexLife checkout or reach into package internals.

## Invariants

- The player's experience is the tiebreaker. Every invariant below is load-bearing and none may be
  broken casually, but when a technical preference and how the game feels to play genuinely
  conflict, the architecture is what has to find another way. Correct, fast, and joyless is not
  done.
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
- A drag is one bounded command carrying two endpoints. The path between them, the per-cell
  heading, the legality, and the cost are resolved natively by `hex_line` and the ordinary `place`
  and `erase` paths — and the drag preview comes from that same resolver, so it cannot promise a run
  the drag will not build. Never expand a drag into per-cell commands on the host, and never give
  the host a line traversal of its own.
- The player walks on its own native cadence, not inside the simulation tick, so a paused or slowed
  factory never pins it in place. The host converts elapsed real time into a step count using the
  rate native publishes and sends it beside the tick count. Frame-coupled movement stays refused:
  the host may send a count, never a position or a delta.
- Placement asks one overlap question of deposits and obstacles alike, at two tuned depths. Two
  different tests for the same question is the defect v0.10 fixed. `deposit_candidates` and
  `resource_at_world` share that predicate and must keep sharing it, or a resolved extractor
  reference stops matching the placement rule.
- Carrying capacity is a rule over the ordinary `item_id → quantity` inventory, never a stored slot
  array: each item takes one slot per part-filled stack of its own `stack_size`, against a slot
  count the scenario fixes. Every path that adds to the player asks first. An erase whose full
  refund will not fit is refused rather than partially paid, so the policy stays exactly 100%.
- Any host list carrying a control is patched in place, never rebuilt. A `replaceChildren` between
  pointerdown and pointerup detaches the pressed control and the delegated click resolves to
  nothing.
- Arbitration is stable by native entity ID. Initial entity IDs derive from sorted coordinates, so
  JSON insertion order cannot change a run.
- Derived caches never become truth. Resolved extractor deposit references are rebuilt from tiles,
  invalidated when chunk generation adds tiles, and are never saved, hashed, or checksummed.
- Snapshot deltas are built from dirty marks made where state is mutated, not by diffing two
  complete snapshots. Marks are derived state under the same rule: never saved, hashed, or
  checksummed. Every new mutation path must mark what it changed, and every marked entry is still
  compared against the host's baseline before it ships, so over-marking is safe and under-marking is
  a defect. `dirty_tracked_deltas_match_a_full_snapshot_diff` is the gate that catches it.
- Fog of war is presentation over the generated chunk set. Chunk snapshots carry native world
  bounds; the host may draw and describe them but must not invent world outside them.
- Time and quantities are integers. Any blocked transfer leaves its source unchanged.
- Canvas 2D is replaceable presentation. Simulation truth comes only from native snapshots.
- Every performance or scale claim must cite a measured tier in `docs/BENCHMARKS.md`. Claims beyond
  the recorded ladder are not supported. Browser claims are supported only for the simulation half
  of a frame — advancing a tick, crossing the worker boundary, and merging the delta — because
  rendering is still unmeasured. One Chromium version on one desktop is the whole browser evidence.
- The capacity harness is measurement code, not shipped code. It compiles into wasm only under the
  `bench` cargo feature, and `bench.html` is served in development only. Neither may become a
  dependency of the game, the production build, or the CI gate.

## Commands

- `npm run dev` — Vite on port 5174
- `npm run build:wasm` — build `factory-wasm/pkg` with wasm-pack
- `npm run build` — Wasm + typecheck + production Vite build
- `npm run format` / `npm run lint` / `npm run typecheck`
- `npm run test:run` / `npm run test:rust`
- `npm run bench` — native capacity ladder; deliberately outside the gate, since shared runners do
  not produce comparable timings
- `npm run bench:browser` — build the `--features bench` wasm artifact and serve it; the same ladder
  plus worker round-trip cost runs at `/HexFactory/bench.html`. Also outside the gate
- `npm run quality` — complete local gate

Commit both `package-lock.json` and `factory-wasm/Cargo.lock`. Do not commit `node_modules`, Rust
`target`, the generated wasm-pack `pkg` or `pkg-bench`, or `dist`; CI builds them from the locked
sources.
