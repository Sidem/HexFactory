# HexFactory architecture

Everything below serves the goal in `docs/HEXFACTORY-PLAN.md`: an open-ended factory game that is fun
to play and a pleasure to control. Determinism, sparse cost and native ownership of the tick are here
because a world that large has to stay responsive, restore exactly and keep growing — not as ends in
themselves.

HexFactory is not a cellular automaton. Exploration uses unbounded continuous fixed-point world
space. Pointy-top axial coordinates exist only for construction anchors/footprints and compiled
transport; the running simulation follows graph edges and sparse scheduled entities.

## Non-negotiable

Twelve rules. Where one genuinely conflicts with how the game feels to play, the architecture is what
has to find another way.

1. **Native hot path.** Rust/Wasm owns cargo movement, machine scheduling, inventories, recipes,
   conflict resolution, production counters and checksums. TypeScript owns UI, rendering, build
   commands and bounded orchestration. No per-cell or per-item JS tick loop.
2. **Separate data dimensions.** Building identity, orientation, cargo, item identity, inventory,
   recipe and progress are separate fields. Never flatten their product into one state byte.
3. **Dynamic identities.** Items, recipes and building definitions use dynamic integer IDs. Adding one
   adds definition data; it must not resize a global transition table.
4. **Chunked, non-toroidal space.** Unbounded axial coordinates, lazily allocated chunks. A finite
   viewport is not a finite world contract. Empty map area costs almost nothing.
5. **Compiled transport.** Belt tiles compile into directed paths between endpoints. The simulation
   runs the compiled representation; it does not rediscover neighbours every tick.
6. **Sparse scheduled machines.** Idle entities run no universal cell update. Wake them for due
   completions, available input, released backpressure, power or topology changes, or edits.
7. **Deterministic arbitration.** Simultaneous transfers may not depend on collection iteration order.
   Use stable entity IDs and explicit priority rules.
8. **Integer time and quantities.** The same definitions, blueprint, commands and tick count produce
   the same checksum in browser and native.
9. **Definitions, not callbacks.** Behaviours are native components fed by data-defined items, recipes
   and buildings. Do not call JS once per machine, item or tick.
10. **Simulation/render separation.** Rendering consumes snapshots or dirty deltas and never owns
    simulation truth. The renderer must be replaceable without changing the engine.
11. **Headless is first-class.** The same core runs without DOM or WebGL.
12. **No unmeasured claims.** Every performance or scale statement cites a recorded tier in
    `docs/BENCHMARKS.md`.

## Dependency boundary

`@hexlife/embed/hex@1.15.0` is the only HexLife dependency and is exactly pinned. TypeScript uses its
public, DOM-free entrypoint for clockwise directions, footprint rotation, construction coordinate
conversion, picking and render centres. The independent Rust crate pins the same direction fixture and
never reads HexLife source or `node_modules`. That package's pixel convention, origin, orientation,
direction numbering, boundary rounding and negative chunk division are public behaviour pinned by
fixtures, not implementation trivia.

**Nothing factory-shaped may enter that package.** Belts, recipes, inventories, scheduling and factory
codecs belong to HexFactory. Do not modify `/sim`, `/ca`, `/stochastic` or `/hcp` for factory
semantics, and do not broaden `/render` into a multi-layer factory renderer.

If a milestone exposes a genuine gap, first prove the feature cannot be built on the existing public
API. A blocking addition is authorized only when it is small, additive, DOM- and Wasm-free and broadly
reusable, and then requires the complete HexLife release path before HexFactory exact-pins the new
version. That exception never permits factory, player, terrain, resource, inventory, recipe or
technology semantics, or a public direction-convention break. Report such a blocker rather than
bypassing the boundary.

## Native ownership

The Rust `Core` owns all state that can change a game result. In outline:

- **World generation** is a pure function of `(params, seed, q, r)`. Terrain bands and resource fields
  derive from seed and hex; only the sparse depletion overlay, the generated chunk set and ordinary
  simulation state are checksum inputs. Flora regrows toward what generation gave a cell, walked from
  a derived set of cut cells, so an untouched forest and a fully regrown one both cost nothing.
- **A deposit is a site, not a hex.** A `site_cell` lattice hashes each cell to at most one site — a
  jittered centre, one rule, one radius — and a hex belongs to the nearest site whose disc covers it
  and whose bands it satisfies, ties broken by lattice cell. Yield falls from core to rim. This makes
  one material per patch a property of the model rather than a tuned number. The lattice is cached;
  the field never is.
- **The guaranteed opening** is a bootstrap pass that spirals outward over lattice cells claiming one
  per guaranteed material inside a stated window; a window that finds nothing widens in fixed steps
  and then the world is refused. The clearing itself generates nothing.
- **The player** has native integer `x/y`, facing, bounded movement intent, action cooldown, build
  range, a carrying slot count, an ordered `item_id → quantity` inventory and at most one cursor-held
  stack. Gathering, delivery, construction costs, erasing, withdrawal and research are native.
- **Walking runs on the player's own cadence**, not inside the simulation tick, so a paused factory
  never pins the player. The host converts elapsed time into a step count using a rate native
  publishes; it never turns a frame delta into a position. The same clock owns the cooldown between
  field actions.
- **Carrying capacity is a rule over the inventory**, not a stored slot array: each item occupies
  `ceil(quantity / stack_size)` slots against a count the scenario fixes. Every path that adds to the
  player asks first. Research can raise the floor through data-defined bonuses.
- **Placement** asks whether the hex is a field cell (extractors) or blocking terrain (everything
  else). `deposit_candidates` and `resource_at_world` share that predicate. Extractors resolve their
  deposits by cached reference rather than by search, ordered exactly as a full scan would resolve
  them, dropped whenever chunk generation adds tiles.
- **Entity fields stay separate**: definition, axial anchor, orientation, transport cargo, container
  inventory, machine ingredient/fuel/output inventories, reserved inputs, progress, fuel charge and
  scenario ownership. Definitions carry a bounded footprint; occupancy, collision, edit targeting and
  snapshots rotate the same data.
- **`compile_graph`** resolves each entity output into directed transport edges after edits. Once
  configured, each recipe output names one exterior side of one footprint tile and cargo may use only
  its own edge. Proposals sort by stable entity ID; a rejected transfer never changes its source.
- **Machines** reserve exact recipe inputs, charge fuel at craft start, run integer ticks and emit only
  on completion. Extractors, pumps and composers write to a bounded output buffer and stop when the
  next whole output will not fit, so a blocked edge never consumes what it cannot preserve.
- **The hub** awards integer insight from data-defined item values. Research prerequisites, costs,
  atomic spending, unlocks, bonuses, objective progress and persistent victory all live in Rust.

Blueprint edits retain the previous graph by stable entity ID, invalidate output rays crossing the
changed footprint and recompile only affected weak components, using both pre-edit links and newly
joined targets so merges and splits match a full deterministic rebuild without resetting machine or
cargo state. `runtime.rs` owns the derived hot-path index those boundaries rebuild — entity and
machine order, compiled outputs, reverse feeders, merger targets, occupied cells, power participants,
transfer scratch. A tick reads those indexes; it does not sort entities or invert the graph. Like the
graph, the index is never saved, hashed or checksummed.

## Definitions and scenarios

`definitions.json` carries a version plus dynamic items, recipes and buildings — construction cost,
unlock requirement, placement rule, description, collision, one-to-seven-cell footprints.
`technologies.json` carries dynamic IDs, prerequisites, costs, unlocks and bounded capability bonuses;
both host and core validate the DAG, references and bounds. `scenarios.json` defines the default **New
game** and the retained **Factory demo**; the hub and all demo objects are scenario-owned and cannot be
erased.

Items declare a `stack_size` and whether they are loose fluid. A belt definition's `transport_medium`
is solid or fluid, and the one compiled graph moves both. Fresh solid belts refuse loose water and
crude oil; pipes refuse solid items; filled barrels are ordinary sealed items and ride belts.
Containers may publish an `accepted_item_ids` filter, which is what makes the water and oil tanks
single-fluid stores.

Erasing a player-built entity returns 100% of construction cost, stored inventories and reserved
inputs. In-transit cargo does not teleport into the pack: it becomes a timed ground item at the
anchor. Native splits the refund in item-id order against a working pack — what fits is carried, the
rest spills.

**Creative mode** is one native flag on the core, entering `checksum()` and the save envelope, so a
creative run is a run like any other. Turning it on inserts every technology id into `researched`,
which is why nothing needed a second way to be unlocked. Placement skips the cost check, upgrades
neither charge nor credit, and `erase_refund` returns nothing. `SetCreative`, `Grant`, `Discard` and
`SetCarrySlots` belong to it; the last three are refused outside it. Deliberately unchanged: power,
fuel, recipe timing, belt throughput and hub payouts — a layout tested in a creative run behaves
identically in a priced one.

## Command and presentation boundary

`FactoryHost` sends at most one JSON command array per rendered frame, capped at eight commands by
both host and core. Simulation ticks and player steps travel beside it as two separate bounded counts,
because the factory and the player run on separate clocks. TypeScript does not update player
coordinates, quantities, insight, research, machines, cargo or victory.

`FactoryRenderer` is the replaceable world boundary; Three.js supplies its production implementation —
an orthographic camera on a bounded elevation dome, twelve 30-degree orbits, bounded zoom,
native-snapshot-driven
instance buckets and scene overlays, split by responsibility across `HexSceneCamera`, `terrainMeshes`,
`machineMeshes`, `worldInstances`, `overlays`, shared materials and quality profiles. Lost WebGL
contexts pause drawing and a restored context rebuilds GPU state from retained presentation state.
Renderer diagnostics expose draw calls, triangles, geometry/texture counts, CPU preparation and a
rolling frame p95.

Picking marches the pointer ray down the drawn height field and names the cell whose surface it meets
— a flank included, since the face of a rise belongs to the rise. At this tilt a column standing a
cliff above its neighbour draws more than a hex away from the plane point beneath it, so a
logical-plane picker handed native the cell _in front of_ a rise. The march runs over the terrain
build's own cell map between the tallest column and the floor, so it costs about twenty lookups. Fog
with no landform keeps the logical plane so it stays pointable, and native remains the final answer on
legality.

An orbit is an integer in `[0, 11]` and moves the instant the key is pressed; only the drawn heading
eases across the 30 degrees, at the rate the old 60-degree step turned at. The sweep raises its own
dirty frames, a step pressed mid-sweep extends the turn rather than restarting it, and reduced motion
arrives at the same view with no sweep. Directional input reads the heading the sweep is landing on.
Elevation similarly moves in five-degree steps over a fixed-radius sphere around the followed target,
bounded away from both the horizon and straight overhead so projection and picking remain stable.

The minimap is a 178 px instanced WebGL2 view rebuilt only when surveyed presentation state changes.
The command bar, guidance, panels, dock, touch pad, camera following, pan/zoom, feedback and
reduced-motion behaviour are presentation only. `ui/dom.ts` owns keyed reconciliation, `ui/panels.ts`
the one-workspace preference, and `core/frameClock.ts` the conversion of elapsed time into the two
bounded native counts — each behaviour-tested without Wasm or WebGL.

## Save contract

Load verifies the checksum using the original world stamp before granting any newer capability.
Restoring a world does not run a newer release's bootstrap eligibility gate but still validates
parameters, references and native state.

**The Phase 8 scale break is the one refusal boundary, and it is not a migration.** A file at save 36
or below describes a 1 m² world; no arithmetic resumes it as 25 m² ground, so `compatibility()`
refuses it ahead of the migration ladder and offers export. Two consequences are easy to miss: rungs
below `[37, 28, 16]` and both replay branches are now unreachable history, and the ladder must gain a
rung for **each build's own tuple**, not only for the oldest file it accepts — without one, `to`
resolves to -1, `migrates` is pinned false, and the build refuses the very saves it can open. The
regression case reads the build end from the shipped catalogues, because a synthetic fixture cannot.

**The world stamp is a second, independent gate**, and it closes on files the format ladder carries
happily. `world_generator_version` answers "which landscape is this", so it moves whenever a rule that
decides what a seed lays down moves. A pre-40 file migrates cleanly up the format ladder and is then
refused on the world stamp — deliberately, because reaching that check is what earns the player the
export message. The two are separate questions and their tests say so separately.

The shape of a release is the contract rather than the list: advance only the envelopes that actually
changed, verify the original checksum **before** applying any migration, and never issue a refund,
grant or conversion as a side effect of loading. New sparse state is stored only where it differs from
untouched ground, so an older file receives an empty overlay and keeps its checksum. A repriced bill is
a one-time revaluation, not a loop, because the refund equals the rebuild cost. A file whose envelope
number is not the expected one is left alone rather than relabelled. Derived availability is never
saved or hashed. Per-release detail is in the git history of this file and in `save_migrations.rs`,
which is the one boundary before typed envelope validation: current envelopes pass byte-for-byte,
newer saves and versions with no adjacent migration are refused, and a future migration is added one
version step at a time rather than guessed inside `from_save`.

`legacy_fluid_belts` is the shape of a grandfathering set worth copying: a sparse, checksummed list of
stable entity IDs, written only after the old checksum verifies, letting an existing belt keep carrying
loose fluid while a new one cannot. The scale break means no load can populate it any more — treat it
as a pattern, not a live path.

Rust serializes `HXF1` plus JSON: envelope versions, seed, generated chunks and quantities, player and
inventory, research, blueprint/entity IDs, machine and cargo state, counters, tick, victory and a
native checksum. The browser catalog stores that opaque string in `localStorage`; export writes the
same text to a `.hxf1` file, or the whole catalog as JSON. Save/resume and uninterrupted runs converge
on the same checksum after equal commands.

Recipes retain a primary output and may add up to seven co-products, each a positive integer quantity
with an explicit cost allocation summing to 100. The shared output compartment must hold the entire
batch before native reserves any inputs; all products complete atomically. The shared buffer does not
imply one outlet: `output_routes` is saved and checksummed by stable entity and item, each naming one
exterior side of one footprint cell, and an absent route map preserves the legacy facing outlet.
Item-level ordered `production_routes` name all producers when alternatives exist; definition cycles
are refused rather than recursively priced.

## Worker and snapshot boundary

The Wasm `Factory` lives in one dedicated module worker, its messages serialized through a single
operation queue. Each advance applies at most one bounded command batch, advances a bounded tick count
and requests one snapshot delta; previews, saves, resets, new games and loads use the same ordered
boundary. The main thread keeps only the latest presentation snapshot and never instantiates the core.

The first snapshot is complete. Every later delta carries a base revision, next revision, tick and
checksum; Rust compares deterministic snapshot groups and omits unchanged ones; the host rejects
missing or out-of-order revisions before merging. Placement preview requests are coalesced to one in
flight plus the latest pending position.

The delta is encoded by `factory-wasm/src/wire.rs` into a compact binary buffer and **transferred, not
structured-cloned** — the worker checks it owns the buffer whole first, because a view into wasm memory
would detach the module's heap. `src/core/snapshotWire.ts` decodes it inside the host's transport, so
everything above `FactoryHost` receives exactly the object the JSON path delivered, down to which keys
are absent. Entity status travels as an `EntityStatus` enum whose **serialized spelling is what the
player reads**: renaming a variant is free, respelling one changes the game's text.

Buildings and resources are the exceptions to group granularity. Buildings travel as a per-entity patch
in ascending stable id order so one linear host pass merges them; resources as a per-deposit patch,
never removed, with generation sending the group whole via `replace`. Measurement drove the buildings
patch: at group granularity one moving item resent every building, 240–246 bytes per building at every
tier against 103–110 now.

## Dirty tracking

The core marks what changed where state is mutated — entity ids, deposit tile keys, flags for terrain
and the chunk set — and builds the delta from those marks against a baseline of what the host was last
sent. Only marked entries are materialized at all. This replaced materializing a complete snapshot per
frame to diff it, which had been 55–91% of the measured frame; the largest tier fell 16.8×.

The marks are derived presentation state: never saved, hashed or checksummed, and incapable of changing
a simulation result. They are appended to vectors rather than inserted into ordered sets, with one sort
at emit time supplying the ascending duplicate-free order the wire needs. Because every marked entry is
still compared against the baseline, a mark that changes nothing costs one wasted rebuild rather than a
wrong frame — which is what lets rare structural paths mark conservatively. World generation is the
important one: it invalidates every resolved deposit reference, so it must invalidate every entity
snapshot derived from one in the same breath.

A missed mark would be a defect, so the shipped builder is pinned against the full-snapshot diff it
replaced: a scripted run covering quiet frames, ticks, gathering to depletion, delivery, research,
placement, rotation, erasure and travel into unsurveyed world asserts the two deltas are byte-identical
after every step. Reset, new game and load discard the baseline rather than patch against a core that
no longer exists.

## Capacity measurement

The ladder is one implementation running on two platforms. It lives in the Rust crate, builds its tiers
from the shipped definitions and drives them through the same entry points the worker uses. A `Clock`
supplies time — `Instant` natively, `performance.now` in wasm — and nothing else differs, so a browser
record and a native record are comparable by construction rather than by re-implementation.

The harness is measurement code and never becomes shipped code: it enters the wasm artifact only under
the `bench` feature, and `/bench.html` is not part of the production build. That page adds what a native
run cannot see — worker RPC round trip, `applySnapshotDelta`, the Three.js world, the minimap — plus
renderer identity, draw calls, triangles, CPU preparation, render p95 and heap evidence.

The measurement has reordered the work three times: the first browser record priced the worker boundary
and made a binary delta the next milestone; that encoding took the boundary out of the frame and made
the engine the cost again; the renderer measurement that followed removed the last unmeasured 89% of a
frame. `docs/BENCHMARKS.md` carries the records, the method and the limits.

## Fog of war

Chunks are the unit of world generation, so the set of generated chunks is exactly the surveyed world.
Each chunk snapshot carries its native world-space origin and span. The terrain builder fills the exact
union of surveyed cells and draws a frontier only on edges whose neighbour is outside that union, so
overlapping chunk rectangles cannot create internal crossings. **Lowland is the default fill and is
deliberately not sent as terrain**, so a surveyed hex carrying no terrain entry is lowland rather than
unknown. None of this is host-invented geography: the host derives only pixels and copy from native
chunk bounds, and travelling generates the chunks that lift the fog.

## Shipped invariants

Each was reached by hitting the defect it prevents. A change that contradicts one needs an argument,
not an oversight.

**Space and direction**

- Direction 0 is east, then clockwise E/SE/SW/W/NW/NE, pinned in both languages by
  `fixtures/hex-directions.json`. **Never widen `DIRECTIONS` past six** — a boiler that reached two
  rows would be a silent rule change. `TRANSPORT_DIRECTIONS` (twelve) is routing only, and the six edge
  headings keep their indices.
- Orientation is an axis the definition owns. `Corner` is the six vertex headings, closed under 60°
  rotation; `Any` is both. A definition that may face a corner requires a single-cell footprint,
  because no definition needs otherwise — and "may face" includes `Any` the moment it is rotated.
- **The axis is a price, not only a permission.** A vertex heading covers `3·size` of world distance
  against `√3·size` for an edge step, so a free heading is strictly dominant. `Edge` and `Corner` answer
  that as separate definitions with separate costs; `Any` answers it inside one definition with
  `corner_construction_cost` and `corner_technology_id`, and validation refuses an `Any` definition that
  gates none of its headings. Rotation walks all twelve headings in angular order, 30° per press, pinned
  against world vectors rather than the index arithmetic that produces them.
- A drag is one bounded command carrying two endpoints. The path, per-cell heading, legality and cost
  are resolved natively by the drag router and the ordinary `place`/`erase` paths, and the preview comes
  from that same resolver so it cannot promise a run the drag will not build. Belts use a bounded
  deterministic obstacle route over every heading the definition allows **and the player has
  researched**; other construction retains `hex_line`. Never expand a drag into per-cell commands on the
  host, and never give the host a line traversal of its own.

**The player**

- The player walks on its own native cadence; the host may send a count, never a position or a delta.
  That clock owns actions as well as walking — work spent per simulation tick froze gathering while
  paused and otherwise scaled the harvest rate with the speed setting.
- **A walk to a click is a standing order, and the order is state while the route is a cache.**
  `walk_goal` lives in `PlayerState`, is saved and is checksummed — two runs differing only in where the
  player is headed are not the same run. The route is derived under the `RuntimeIndex` rule, rebuilt by
  `rebuild_runtime_index` and never saved: a saved route would let a file describe a corridor the world
  no longer has.
- **The search answers with the fastest way, not the shortest, and surveys nothing to find it.** A\*
  costs shallow water by the fraction of speed a ford actually costs; an unweighted search sends the
  player wading across water they would have walked round. It reads `terrain_at` and `runtime.occupied`
  and calls neither `ensure_tile` nor `generate_chunk`, because `generated_chunks` is a checksum input.
  Bounded by `MAX_WALK_DISTANCE`, `MAX_WALK_SEARCH_NODES` and `MAX_WALK_PATH_CELLS`, breaking ties on
  `(f, g, q, r)` rather than heap order. Any `MoveIntent`, including the zero one a key release sends,
  cancels — the movement keys always take control back.
- **A route is drawn from native's own remaining path and never re-found by the host.** A host-side
  search would be a second pathfinder and would eventually draw a way the simulation would not take.
- **A harvest is work, and the work comes before the yield.** `action_cooldown` is the swing still
  running: `gather_from` arms it and takes nothing, `finish_gather` moves deposit and pack together. The
  old order made the first gather of every session free. The landing re-asks what the start asked, reach
  included; a player who walks out of reach is paid nothing.
- A gather asks the same question an extractor on that hex asks, and **facing is not part of it** — a
  facing-weighted target drained a neighbouring hex while the one underfoot stayed full. A right-click
  is different in kind: it names a hex the player pointed at, so `gather_at` takes an explicit target and
  only that target moves. Both land in `gather_from`.
- Extraction reach is a definition field, not a constant: `field_covered_at` takes the radius its caller
  reaches. It is one predicate shared by placement, the cached candidate list and both gathers. A tier
  that changes reach must drop that entity's `deposit_links`, resolved against the old radius.
- Facing is native checksummed state, so the host sends the world position to face and never a heading.
  `aim` wins over `move_intent` by arriving later in the same batch, which is why a touch layout that
  sends no `aim` keeps facing the way it walks.
- Anything drawn as a proportion is given both numbers — the swing ring takes `action_cooldown` and a
  published `action_cooldown_total`. Inferring a maximum by watching a value count down is the host
  re-deriving native truth.

**Ground, terrain and water**

- Which terrain bands are impassable is native's rule, pinned in both languages by
  `fixtures/terrain-passability.json`. The host draws impassable ground as one category before it draws
  it as a material, reading the pinned table rather than guessing which grey means cliff.
- **The band table answers for ground nobody has worked; a hex answers for itself.** `natural_elevation`
  puts a cliff exactly one step over highland, so one `Lower` cut brings the face level with the ground
  beside it — the band never moves, and the whole change lives in the overlay's signed `elevation`.
  Anything asking what may happen **on a particular hex** goes through `Core::terrain_blocks_movement` /
  `terrain_blocks_construction` or `bandAt`, never `blocks_movement` or `TERRAIN_INFO` directly.
- **The ground is physical, and one type owns every fact about it.** `GroundSpine` separates generated
  bed, substrate and initial hydrology; `FinishedGround` keeps earthwork, erosion and prepared surface
  distinct and is the one route to finished elevation and access. `GroundSpine::physical` publishes
  absolute bed height in 0.25 m quanta plus water depth, surface and discharge. Its surveyed-chunk cache
  falls back to the uncached source when world identity no longer matches, and is rebuilt rather than
  saved — the uncached source is its oracle.
- **Water is stored as a departure, never as a level.** Generated hydrology is a pure function of seed
  and coordinate, so an untouched world carries no water state. `hydrology.rs` stores only
  `DisturbedWater` and forgets a cell the moment it returns to generated depth, so a world flooded and
  drained back hashes identically to one nobody touched. `Core::water_depth_at` is the one predicate for
  movement, construction, wading, route cost, bridges, pumping and hydro. An earthwork settles water over
  the cells it moved, in a region that grows only where settling water asks for ground and never past the
  surveyed frontier — a solve cannot generate a chunk. Termination is a potential argument rather than
  the sweep budget, and a budget-truncated rim is a wall and never a sink. The undo record carries the
  exact departures its solve displaced, so putting the ground back puts the water back rather than
  running a second solve and trusting it to agree.
- **Live erosion is a sparse geomorphic epoch, never a terrain tick.** Once per in-game hour,
  `geomorphology.rs` considers only surveyed wet generated-flow edges with non-zero discharge, in stable
  coordinate order. Curvature loads the outside bank; substrate, vegetation, paving, occupancy and
  boundary resistance decide accumulation. Stress and erosion store only non-zero departures. Earthwork
  remains the paid grade and erosion its own delta in saves, checksums and the wire; presentation may sum
  them but cannot rewrite either.
- **An earthworks selection is resolved in three passes, and the footprint survives a refusal.**
  `ground_resolve` records a `blocked` reason on the hex in the way instead of aborting; `ground_footprint`
  publishes every selected cell whatever its outcome; `ground_confirm` applies the whole-selection gates.
  The order is the contract — a selection that vanishes when refused tells the player nothing about what
  to fix. Shapes are native truth (`GroundShape`, `MAX_GROUND_CELLS` = 64), and an outline is the
  hex-adjacency perimeter of its own fill, so it is one hex thick at every size.
- Terrain is the material map: each raw resource generates only in the band its geography names. A
  resource reachable from no buildable hex is a defect.
- **The site lattice and the bootstrap table are derived state** on exactly the terms `deposit_links` is.
  Do not reintroduce a per-hex gate deciding _which_ material a hex holds — that is the defect the
  lattice removed. **The clearing generates nothing**; do not re-add a hardcoded list of cells inside it.

**Transport and machines**

- A junction is a definition flag, never a `BuildingKind` and never a second tick path. `splits` compiles
  every free forward heading into `Links` and offers from `route_cursor`; `merges` accepts from behind and
  arbitrates by `merge_cursor`, the last feeder served, so a merger alternates where a plain belt starves
  whichever lane loses the ascending-id race. Both cursors are saved and checksummed. `transfer_cargo`
  runs mergers first, then everything else in ascending entity id.
- A belt is a length of conveyor, not a one-tick hop: `advance_belt_lanes` runs first, an item takes
  `belt_transit_ticks()` to cross a hex, and `can_accept` spaces entries by `belt_slot_ticks()`. `lane` is
  saved and hashed; `cargo` is only the exit slot waiting to be handed on.
- An underpass is one arm in the graph trace, not a second lattice: `trace_output` is
  `trace_underpass(...).or_else(trace_ray(...))`. Crossed cells stay singly occupied, buildable and
  connected to their own lane. Do not give them a second occupancy or a height of their own.
- An upgrade edits the entity in place and never replaces it, which is what preserves contents,
  orientation and connections. `validate_upgrade_ladders` pins kind, recipe category, footprint and axis
  across every step. The price is netted per item and both halves are checked before either is applied —
  the same all-or-nothing rule `erase` uses, and the reason a round trip cannot duplicate items.
- **Fuel is a property of the item, never an entry in a recipe's `inputs`.** A recipe that named its fuel
  would need one variant per fuel and would hardcode the bootstrap path. Ingredient, fuel and output
  inventories are separate native maps. `stock_kind_for_item` puts recipe inputs first — coal is an
  ingredient in steel — and only then admits another burnable item as fuel. `burnable_item` is the one
  fuel predicate the tick, hand transfer, transport acceptance and stop status all keep asking.
- A building's `capacity` bounds ingredient and fuel compartments **per item** and the output compartment
  and a container's store **as one pool**. `room_for_stock` is the single answer, so a belt, a hand
  transfer and a drawn slot cannot disagree. Per item, because a shared total let a full first ingredient
  close the empty slot beside it; one pool for output, because a whole batch must fit before inputs are
  reserved.
- A new machine is a `recipe_category` and a check, not a `BuildingKind` and a tick path. Add a kind only
  when a building's _source_ is genuinely different — which is why `Pump` is one.
- A `stack_size` is chosen against the recipes its item is in: every input and output quantity divides the
  stack size of its own item, so a stack is a whole number of crafts. Six wood to a charcoal against a
  stack of twenty stranded two wood at the bottom of every stack. `tests/definitions.test.ts` pins it
  arithmetically, so a new ratio fails the gate rather than the player.
- A cursor-held stack is native inventory state, not DOM drag data. Left click lifts or places a full
  stack, right click halves or places one, Ctrl-click moves one, Shift quick-moves the same quantity.
  Native owns reach, compatibility, room, save and checksum. Pointer dragging queues the same
  pickup/place commands together on release; the bounded queue accepts both or neither.

**Wire and host**

- Snapshot numbers reach the host as IEEE-754 doubles. Nothing wider than 2^53 may travel as a number, and
  nothing whose identity matters may be re-derived into one: field cells are addressed by tile key, never
  by an id packed from the same two coordinates.
- The wire format is pinned in two places and both must move together: Rust round-trips every delta a
  running factory produces, and `fixtures/snapshot-delta-wire.json` carries encoded payloads beside the
  exact JSON they decode to. Regenerate with `UPDATE_WIRE_FIXTURE=1 cargo test wire_fixture` and read the
  diff — a change there is a wire break. `BuildingKind`, `Terrain` and `EntityStatus` travel as their
  declaration index, so reordering a variant is a mistranslation rather than a decode failure.
- Any host list carrying a control is patched in place, never rebuilt. A `replaceChildren` between
  pointerdown and pointerup detaches the pressed control and the delegated click resolves to nothing —
  which is how research clicks were being dropped about once a second.
- An item is drawn one way, by `src/rendering/itemChip.ts`, and never by a second shape. `3` and `3 / 10`
  are the only two spellings of a quantity. Markup that spells a chip out by hand is the drift this
  replaced, and `tests/host.test.ts` refuses it.
- The active workspace panel and the hotbar arrangement are presentation state in `localStorage`: never
  saved with the game, never hashed, never sent, and validated against the live document or catalogue on
  load. One workspace opens at a time at every width.
- Named saves live in a version-independent catalog (`hexfactory:saves:v1`) recording envelope versions
  and the world each was started with. Incompatibilities stay visible on the row; they are never hidden by
  putting those numbers in the storage key.

**Economy and evidence**

- The economy states its curve as two rules over the data: a tier costs strictly more than the tier it
  upgrades from, and a machine costs no less than a machine of the same `kind` whose technology it is
  unlocked behind. Cost is `effort` — tree-expanded raw units plus fuel energy priced in the densest fuel
  — and every raw unit counts once, because that is the only weighting the data supplies.
- Balance figures are derived and computed once, natively, in `factory-wasm/src/balance.rs` — items per
  minute restates `advance_composer`, machines carried restates `power_progress`, a site yield restates
  `deposit_candidates`. TypeScript recomputes only the pure arithmetic over `definitions.json`, so the
  fixture is pinned by two independent expansions rather than one implementation agreeing with itself.
- No scripted guide may outrun the rules it explains. The next step is derived — the contract's outstanding
  bill through the recipe tree, plus the technologies those machines sit behind and the power branch none
  of them names — so every step it produces is achievable in the state that produced it.
- Derived caches never become truth. Resolved deposit references are rebuilt from tiles, invalidated when
  generation adds tiles, and are never saved, hashed or checksummed.
- A milestone that changes the world generator, the item roster or the entity snapshot re-runs
  `npm run bench` before it ships. A checksum change invalidates checksum comparisons, not timing ones:
  say which of the two a record is claiming.
