# HexFactory agent notes

HexFactory is a browser factory-automation game. The goal is a beautiful, open-ended game that is
fun to play and a pleasure to control, inspired by Factorio, Satisfactory, and Minecraft — never
imitating their assets, names, or branding. The deterministic Rust/Wasm core and the sparse
architecture are the means that make that game possible at scale, not the point of the project.

Keep this file concise; the durable roadmap, design pillars, and implementation handoffs live in
`docs/HEXFACTORY-PLAN.md`, architecture decisions in `docs/ARCHITECTURE.md`, shipped MVP status in
`docs/MVP.md`, and measured capacity in `docs/BENCHMARKS.md`. Upgrades and Tiers shipped as
**v0.14**: tiered definitions with an in-place `upgrade`, extraction reach as the flagship
upgrade, north and south in the transport direction table as the riser, a right-click that
harvests one named hex, and two-way hand transfer with containers. `HXF1` save version is 7,
definition version 7, technology version 4; v0.13 saves are rejected.

Generated Shapes shipped as **v0.15**: a building's drawing is a part list from an eight-part
vocabulary in `src/rendering/shapeGrammar.ts`, `BUILDING_SHAPES` in `src/rendering/buildingLook.ts`
is that table and is total over `SilhouetteKey`, and a tier is a modifier on the list through
`TIER_LADDER` rather than a stroke colour. Still parts bake behind `BUILDING_SHAPE_VERSION`; only
parts carrying a `phase` are walked per frame. The player draws from the same vocabulary. The
contact sheet is `contact.html`, a dev entry point beside `bench.html` — dev-only, and like
`bench.html` it must not become a dependency of the game, the production build, or the CI gate.
Presentation only: no save, definition, generator, wire, or checksum movement.

World Parameters shipped as **v0.16**: a world is a seed plus a `WorldParams`, which travels in the
save envelope and the checksum, so `WORLD_GENERATOR_VERSION` is 6 and a version-5 envelope is
rejected. Feature scale and threshold are separate axes — sea level decides how much water there
is, the coarse elevation octave's cell size and blend share decide how big it is. `field_at`'s
match arms are a `FieldRule` table evaluated in declared order. Four presets ship as data rows
(`continental` is version 5's frozen numbers). `npm run survey` measures what a parameter set
actually generates and is where every claim a preset makes comes from; `--set name=value` surveys
one nobody shipped. The browser save key is `hexfactory:hxf1:v7w6`.

Balance shipped as **v0.17**, finishing the arc. `fixtures/balance.json` is every figure that
decides whether the economy works — machine rates, what a generator carries and what it burns and
drinks to carry it, fuel conversions, and the full raw-material cost of every building expanded
through its whole recipe tree — computed by `factory-wasm/src/balance.rs` from the shipped
catalogues, printed by `npm run balance`, and pinned in both languages. The first tuning pass moved
six numbers, each traceable to a printed figure; definition version is 8 and the browser save key
is `hexfactory:hxf1:v7w6d8t4`. Read the shipped record in `docs/HEXFACTORY-PLAN.md` before changing
a cost, a cadence, or a power figure, and `docs/ART.md` Stage D before touching
`src/rendering/buildingLook.ts` or the grammar.

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
  the host may send a count, never a position or a delta. That clock owns everything the player
  does themselves, actions as well as walking — a cooldown spent per simulation tick froze
  gathering outright while paused and otherwise scaled the harvest rate with the speed setting. So
  the host keeps the player's clock running while a cooldown is outstanding, not only while walking.
- A gather asks the same question an extractor on that hex asks, and facing is not part of it. A
  target weighted by facing counted down a neighbouring hex while the one underfoot stayed full — a
  change with no visible cause. Where the mouse happens to rest is still not something a player
  reads as aiming at a hex, so facing-weighted targeting stays refused.
  **v0.14 makes the argument that rule asked for, and makes a different one.** A right-click is not
  a weighting; it is the player naming a hex on screen, deliberately, so the number that moves is
  the one they pointed at and the cause is visible. `gather_at` therefore takes an explicit target —
  and only the target moves. Reach is unchanged and still `field_covered_at` at the player's own
  radius, so a right-click can never take from a cell an extractor standing there could not. Both
  gathers land in `gather_from`, so the cooldown, the carrying rule, and the depletion mark are one
  implementation.
- Extraction reach is a definition field, not a constant: `field_covered_at` takes the radius its
  caller reaches, `deposit_candidates` passes the extractor's own, and the hand always passes
  `EXTRACT_RADIUS`. It is still one predicate — placement, the cached candidate list, and both
  gathers share it — so a resolved reference cannot drift from the rule that allowed the building.
  A tier that changes reach must drop that entity's `deposit_links`, which were resolved against the
  old radius.
- Orientation is an axis the definition owns. `DIRECTIONS` (six) is adjacency and power;
  `TRANSPORT_DIRECTIONS` (eight) is routing, and the six keep their indices so every saved
  orientation still means what it meant. `OrientationAxis::Vertical` is the two-row period and
  requires a single-cell footprint, because `@hexlife/embed` rotates by 60° and the vertical
  headings have no 60° equivalent. Never widen `DIRECTIONS`: a boiler that reached two rows would
  be a silent rule change.
- An upgrade edits the entity in place and never replaces it, which is what preserves contents,
  orientation, and connections without special handling. `validate_upgrade_ladders` pins kind,
  recipe category, footprint, and axis across every step, so the command does not have to re-ask
  whether any of them still apply. The price is netted per item against the old construction cost
  and both halves are checked before either is applied — the same all-or-nothing rule `erase` uses,
  and the reason an upgrade / erase round trip cannot duplicate items.
- Facing is native, checksummed state, so the host may send the world position it wants the player to
  face and never a heading. `aim` carries the point under the cursor; native resolves the unit vector
  in integer arithmetic. `move_intent` still sets facing, and an aim wins by arriving later in the
  same batch, which is why a touch layout that sends no `aim` keeps facing the way it walks.
- Which terrain bands are impassable is native's rule and is pinned in both languages by
  `fixtures/terrain-passability.json`, against `Terrain::blocks_movement` and
  `Terrain::blocks_construction` in Rust and against `src/core/terrain.ts` in TypeScript. The host
  draws impassable ground as one category before it draws it as a material; that treatment reads the
  pinned table and never a palette-side guess about which grey means cliff.
- Snapshot deltas cross to the host in the binary wire format, encoded by `factory-wasm/src/wire.rs`
  and decoded by `src/core/snapshotWire.ts`. The decoder's contract is that it produces exactly what
  `JSON.parse(snapshot_delta_json())` produced — the same keys, the same omissions, `null` where
  native sends `null` — so the encoding is transport and nothing above `FactoryHost` knows which one
  delivered a frame. Every value still becomes a JavaScript number on arrival, so the 2^53 rule
  below is unchanged by the format. `snapshot_delta_json` stays as the oracle the encoder is pinned
  against; it is not a fallback path and the game must not ship on it.
- The wire format is pinned in two places at once and both must move together. Rust round trips
  every delta a running factory produces inside
  `dirty_tracked_deltas_match_a_full_snapshot_diff`; `fixtures/snapshot-delta-wire.json` carries
  encoded payloads beside the exact JSON they decode to, Rust asserts it writes those bytes, and
  `tests/snapshotWire.test.ts` asserts TypeScript reads them back. Regenerate it with
  `UPDATE_WIRE_FIXTURE=1 cargo test wire_fixture` and read the diff — a change there is a wire
  break. `BuildingKind`, `Terrain`, and `EntityStatus` travel as their declaration index, so
  reordering a variant is a mistranslation rather than a decode failure, which is what the fixture's
  enum tables exist to catch.
- Snapshot numbers reach the host as IEEE-754 doubles. Nothing wider than 2^53 may travel as a
  number, and nothing whose identity matters may be re-derived into one: field cells are addressed
  by their tile key, never by an id packed from the same two coordinates.
- Fuel is a property of the item, never an entry in a recipe's `inputs`. A recipe that named its
  fuel would need one variant per fuel and would hardcode the bootstrap path. A machine burns from
  its own stock and never from the quantity a recipe input reserves — steel names coal as carbon,
  and a smelter that burned those units starves itself on its own recipe. `burnable_item` is the one
  predicate; the tick that burns and the status that explains why nothing did must keep asking it.
- A new machine is a `recipe_category` and a check, not a `BuildingKind` and a tick path. Smelter,
  kiln, cutter, crusher, and composer are one kind. Add a kind only when a building's _source_ is
  genuinely different, which is the whole reason `Pump` is one: it draws from terrain rather than a
  deposit, and its basin never empties.
- Terrain is the material map. Each raw resource is generated only in the band its geography names,
  because a landscape the player cannot read is decoration. A resource reachable from no buildable
  hex is a defect — stone sits on impassable cliffs and is quarried from the hex beside them.
- Anything the host draws as a proportion must be given both numbers. The cooldown ring takes
  `action_cooldown` and a published `action_cooldown_total`; inferring a maximum by watching a value
  count down is the host re-deriving native truth.
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
  nothing. This now covers the hotbar slots and the catalogue cards as well as the research list,
  the Take rows, and the Put rows.
- The hotbar arrangement is presentation state and lives in `localStorage`: never saved with the
  game, never hashed, never sent. It is a preference about a keyboard, not a fact about a factory.
  Definitions are dynamic, so a stored slot is validated against the live catalogue on load and
  dropped if its id no longer exists. Buildings live in the `B` catalogue, grouped by `kind`; the
  bar holds only what the player pinned there.
- The economy states its curve, and the curve is two rules over the data rather than a mood. A tier
  costs strictly more than the tier it upgrades from; a machine costs no less than a machine of the
  same `kind` whose technology it is unlocked behind. Cost is `effort` — tree-expanded raw units
  plus fuel energy priced in the densest fuel item — and every raw unit counts once, because that is
  the only weighting the data supplies. A cutter does not follow a kiln, so they are not compared.
- Balance figures are derived and are computed once, natively. Items per minute restates
  `advance_composer`, machines carried restates `power_progress`, and a site yield restates
  `deposit_candidates`; recomputing any of them in the host would be a second implementation of a
  native rule. `factory-wasm/src/balance.rs` is native-only measurement code like the survey and the
  ladder, and never enters the wasm artifact. What TypeScript does recompute is the pure arithmetic
  over `definitions.json`, in `tests/balance.test.ts`, so the fixture is pinned by two independent
  expansions rather than by one implementation agreeing with itself.
- The browser `SAVE_KEY` names every version the envelope refuses a load on — save, world generator,
  definition, and technology. A bump the key cannot see is a Continue button that can only fail.
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
- A milestone that changes the world generator, the item roster, or the entity snapshot re-runs
  `npm run bench` before it ships. v0.12's re-measurement found two regressions it had introduced —
  86 KB of delta payload and a 3.9× slower snapshot — and one 3.0× saving v0.11 had shipped without
  measuring. A checksum change invalidates checksum comparisons, not timing ones: say which of the
  two a record is claiming.
- Every performance or scale claim must cite a measured tier in `docs/BENCHMARKS.md`. Claims beyond
  the recorded ladder are not supported. Browser claims may cite a complete frame — advancing a
  tick, crossing the worker boundary, merging the delta, and drawing the two canvases — only from
  the v0.12.4 record onward, at the pinned 1440×900 viewport and device-pixel-ratio 1 that record
  used. One Chromium version on one desktop is the whole browser evidence.
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
- `npm run survey` — what a world parameter set actually generates: band histogram, field density
  per material, distance from the landing site, and water body sizes. A threshold is not a
  proportion, so this is where a preset's claims about its own landscape come from. Also outside
  the gate, and like the ladder it is native-only measurement code that never enters the wasm
  artifact
- `npm run balance` — what the shipped numbers add up to: machine rates, generator budgets, fuel
  conversions, tree-expanded building costs, the curve, material access, site yields, and the
  openings. A cost row says what a building costs and nothing about what its inputs cost to make, so
  a tuning pass argued from the data file alone is argued from a quarter of the numbers. Outside the
  gate like the ladder and the survey, and native-only measurement code that never enters the wasm
  artifact. `fixtures/balance.json` is the recorded form: regenerate with
  `UPDATE_BALANCE_FIXTURE=1 cargo test balance_fixture`, then
  `npx prettier --write fixtures/balance.json`
- `npm run bench:browser` — build the `--features bench` wasm artifact and serve it; the same ladder
  plus worker round-trip cost runs at `/HexFactory/bench.html`. Also outside the gate
- `npm run quality` — complete local gate

Commit both `package-lock.json` and `factory-wasm/Cargo.lock`. Do not commit `node_modules`, Rust
`target`, the generated wasm-pack `pkg` or `pkg-bench`, or `dist`; CI builds them from the locked
sources.
