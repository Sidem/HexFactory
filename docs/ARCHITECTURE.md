# HexFactory architecture

Everything below serves the goal in `docs/HEXFACTORY-PLAN.md`: an open-ended factory game that is
fun to play and a pleasure to control. Determinism, sparse cost, and native ownership of the tick
are here because a world that large has to stay responsive, restore exactly, and keep growing —
not as ends in themselves.

HexFactory is not a cellular automaton. Exploration uses unbounded continuous fixed-point world
space. Pointy-top axial coordinates exist only for construction anchors/footprints and compiled
transport; the running simulation follows graph edges and sparse scheduled entities.

## Non-negotiable

Twelve rules. None of them may be traded away for convenience; where one genuinely conflicts with
how the game feels to play, the architecture is what has to find another way.

1. **Native hot path.** Rust/Wasm owns cargo movement, machine scheduling, inventories, recipes,
   conflict resolution, production counters, and checksums. TypeScript owns UI, rendering, build
   commands, and bounded orchestration. No per-cell or per-item JS tick loop.
2. **Separate data dimensions.** Building identity, orientation, cargo, item identity, inventory,
   recipe, and progress are separate fields. Never flatten their Cartesian product into one state
   byte or lookup table.
3. **Dynamic identities.** Items, recipes, and building definitions use dynamic integer IDs. Adding
   an item or recipe adds definition data; it must not resize a global transition table.
4. **Chunked, non-toroidal space.** Unbounded axial/cube coordinates and lazily allocated chunks. A
   finite viewport is not a finite world contract. Empty map area costs almost nothing.
5. **Compiled transport.** Directional belt tiles compile into directed paths between endpoints. The
   simulation runs the compiled representation; it does not discover six neighbours for every belt
   on every tick.
6. **Sparse scheduled machines.** Idle entities do not execute a universal cell update. Wake them for
   due completions, available input, released backpressure, power or topology changes, or edits.
7. **Deterministic arbitration.** Simultaneous transfers may not depend on collection iteration
   order. Use stable entity IDs and explicit priority rules.
8. **Integer time and quantities.** The same definitions, blueprint, commands, and tick count must
   produce the same checksum in browser and native tests.
9. **Definitions, not callbacks.** Behaviours are native components fed by data-defined items,
   recipes, and buildings. Do not call JS once per machine, item, or tick.
10. **Simulation/render separation.** Rendering consumes compact snapshots or dirty deltas and never
    owns simulation truth. The renderer must be replaceable without changing the engine.
11. **Headless is first-class.** The same core runs without DOM or WebGL, so blueprints can be
    evaluated in workers or Node.
12. **No unmeasured claims.** Every performance or scale statement cites a recorded tier in
    `docs/BENCHMARKS.md`.

## Dependency boundary

`@hexlife/embed/hex@1.15.0` is the only HexLife dependency and is exactly pinned. TypeScript uses
its public, DOM-free entrypoint for clockwise directions, footprint rotation, construction
coordinate conversion, picking, and render centers. The independent Rust crate pins the same
direction fixture and never reads HexLife source or `node_modules`.

The published `/hex` contract covers one documented clockwise six-direction ordering, axial
neighbour lookup and rotation, axial/cube distance and rounding, axial-to-pixel and pixel-to-axial
conversion, line traversal, and negative-coordinate-safe mapping to fixed-size chunks. Its pixel
convention, origin, orientation, direction numbering, boundary rounding, and negative chunk division
are public behaviour pinned by fixtures, not implementation trivia.

**Nothing factory-shaped may enter that package.** Belts, recipes, inventories, scheduling,
blueprint evolution, and factory codecs belong to HexFactory. Do not modify `/sim`, `/ca`,
`/stochastic`, or `/hcp` for factory semantics, and do not broaden the binary `/render` into a
multi-layer factory renderer. A future addition qualifies only if it is a generic hex-host primitive
with at least one credible non-HexFactory consumer.

If a milestone exposes a genuine gap in `/hex`, first prove the feature cannot be implemented with
its existing public API. A blocking addition is authorized only when it is small, additive,
DOM- and Wasm-free, and broadly reusable — and it then requires the complete HexLife release path
(source, declarations, exports, build, declaration-copy list, tests, reference docs, README,
changelog, an `embed-vX.Y.Z` publish) before HexFactory exact-pins the new version. That exception
never permits factory, player, terrain, resource, inventory, recipe, or technology semantics, a
public direction-convention break, or changes to HexLife's CA engines or renderer. Report such a
blocker instead of bypassing the boundary.

## Native ownership

The Rust `Core` owns all state that can change a game result:

1. A versioned seed and integer value noise generate fixed-size axial environment chunks without
   shared traversal state. Terrain bands and resource fields are derived from seed and hex; only
   the sparse depletion overlay, the generated chunk set, and ordinary simulation state are
   checksum inputs. Collision and placement read the hex under the point. Each raw resource is
   generated only in the band its geography names, so terrain is the material map rather than a
   colour: iron and coal on the tops and the ground below them, copper on hills and never above,
   sand and clay on shores, stone against cliffs, forest in lowland. Flora is the one source that
   comes back — an item's `regrowth_ticks` makes a cut cell climb toward what generation gave it,
   walked from a derived set of cut cells so that an untouched forest and a fully regrown one both
   cost nothing.

   **The unit of a deposit is a site, not a hex.** The world is partitioned by a `site_cell`
   lattice exactly as the noise channels are; each cell hashes to at most one site — a jittered
   centre, one rule, one radius — and a hex belongs to the nearest site whose disc covers it and
   whose member bands it satisfies, ties broken by lattice cell rather than by iteration order.
   Yield falls from core to rim. This is what makes **one material per patch** a property of the
   model instead of a number that was tuned: rows no longer compete hex by hex, so two independent
   noise channels can no longer alternate two materials under one extractor disc.

   The **site lattice** is cached on `Core`, and the field never is. `field_at` is on the hot path
   — `deposit_candidates` walks a whole disc, and `resource_at_world`, both gathers, and every
   snapshot build reach it — and the uncached form evaluates every lattice cell within reach per
   hex, each deciding a band. A site cell is `site_cell²` hexes, so the map stays small and every
   hex of a chunk hits it warm. Like `deposit_links`, it is derived state: never saved, never
   hashed, never checksummed, rebuilt whenever the parameters or the seed move.

   The **guaranteed opening** is derived on the same terms. A bootstrap pass spirals outward over
   lattice cells and claims one for each guaranteed material whose patch falls inside a stated
   window and is large enough to stand an extractor in; a window that finds nothing widens in fixed
   steps and then the world is **refused**. It replaced a hardcoded list of eight single cells
   inside the clearing, which was why every material used to be visible in the first minute. The
   clearing itself still generates nothing, and no guaranteed disc may reach into it.

   Rivers are ridge noise rather than a simulation, because the map is unbounded and generated
   lazily and nothing may depend on knowing where the water upstream went: a river is where a
   dedicated channel runs near its own midpoint, gated below an elevation so none runs over a
   summit. Beaches ask the coarse elevation octave alone whether a centre stands against ocean —
   coarse-octave water is what makes a body big — which is a proxy, stated as one in the code, and
   verified by the survey reporting the size of the body nearest each patch.

2. The player has native integer `x/y`, facing and bounded movement intent vectors, action cooldown,
   world-unit build range, a carrying slot count, an ordered `item_id → quantity` inventory, and at
   most one cursor-held stack. The cursor stack is real inventory state: picking up, placing, and
   quick-moving it are bounded native commands, and it is saved and checksummed so closing the game
   while carrying a stack cannot duplicate or discard items. Gathering, delivery, construction
   costs, erasing, withdrawal, and research are native.
3. Walking runs on the player's own native cadence rather than inside the simulation tick, so a
   paused or slowed factory does not pin the player in place. The host converts elapsed real time
   into a step count using a rate native publishes, and sends that count beside the tick count; it
   never turns a frame delta into a position, so the same commands and counts still reproduce the
   same position and the same checksum. The same clock owns the cooldown between field actions: it
   used to be spent one unit per simulation tick, so pausing froze gathering after a single attempt
   and the harvest rate otherwise rode the speed multiplier.

   A second click on a selected hex is a standing order to walk there, and the split between what is
   saved and what is derived follows the `RuntimeIndex` doctrine exactly. The **goal** is real state:
   it lives in `PlayerState`, is written to the save, and is hashed into the checksum, because two
   runs that differ only in where the player is headed are not the same run — that is what took the
   envelope to 15, with an explicit `save_migrations` step writing `walk_goal: null` into a
   version-14 file rather than letting a defaulting deserializer invent it. The **route** is derived:
   an A\* over hex centres, bounded by `MAX_WALK_DISTANCE` and `MAX_WALK_SEARCH_NODES`, rebuilt by
   `rebuild_runtime_index` — which every edit and every load already funnels through — and never
   saved, hashed, or checksummed. Saving it would let a file describe a corridor the world it loads
   into no longer has.

   The search costs shallow water five, the ratio between the ford speed and the walking speed, so
   the route it returns is the fastest one rather than the shortest one; an unweighted search sends
   the player wading at 5 m/s across water they would have walked around. It reads terrain through
   the pure `terrain_at` and blocking through `runtime.occupied`, and calls neither `ensure_tile` nor
   `generate_chunk`: considering a hex must not survey it, because `generated_chunks` is a checksum
   input. Ties break on `(f, g, q, r)` rather than on heap order, so the same click answers the same
   way in every run.

   Steering writes `move_x`/`move_y` directly, ahead of the step that consumes them. Any
   `MoveIntent` — including the zero one a key release sends — cancels the walk, so touching the
   movement keys always returns control. Arrival ends it silently; a route that runs out somewhere
   other than the goal, or thirty player steps without moving, ends it with an event. Gathering
   deliberately does not.

4. Carrying capacity is a rule over the ordinary inventory, not a stored array of slots: each item
   occupies `ceil(quantity / stack_size)` slots and a scenario fixes the starting slot count. Every path that
   adds to the player — gathering, erasing, withdrawing — asks first. Gathering into a full pack is
   refused; an erase whose refund would not fit is refused whole, which is the only one of refuse,
   partially refund, and spill that keeps item conservation exact and leaves the recovery available
   once there is room; a withdrawal moves what fits and leaves the rest in the container. Research
   can raise the pack floor and build range through data-defined bonuses; native applies those
   bonuses to the same player fields every carrying and placement path already reads, and validates
   them against the researched set on load. Creative may widen the pack beyond that earned floor.
   The cursor-held stack is outside those slots while it is being moved, but remains native quantity.
5. Placement asks whether the hex is a field cell (for extractors) or blocking terrain (for
   everything). `deposit_candidates` and `resource_at_world` share that field predicate. Extractors
   harvest every field cell within hex radius 1, and a player's gather goes through the same
   `resource_at_world`, so an action reaches exactly what an extractor on that hex would. Facing is
   not an input to it: a facing-weighted target drained a neighbouring cell's amount while the hex
   underfoot stayed full. Since v0.12.3 the player does point somewhere the player chose — `aim`
   carries the world position under the cursor and native resolves the facing vector from it in
   integer arithmetic — but where a pointer rests is not a hex the player has aimed at, so the
   harvest still asks only which field the player is standing on or beside.
6. A field cell's identity on the wire is its tile key, and nothing derived from it travels beside
   it. Snapshot numbers reach the host as JavaScript numbers, which are IEEE-754 doubles, so a
   64-bit id packed from two coordinates arrived rounded past 2^53 and a whole column of the field
   shared one value. Patching by it rewrote cells the player never touched with a copy of the
   harvested one. The binary wire of v0.12.2 does not change this: it carries a varint of full
   width, but the host still holds the result as a double.
7. Fuel is a property of `ItemDefinition`, never an entry in a recipe's `inputs`. A smelting recipe
   therefore names no fuel at all, and coal, charcoal, wood, and every fuel added later are
   interchangeable at different values; naming one would force a separate recipe per fuel and
   hardcode the bootstrap path. A machine burns from its fuel compartment, lowest item id first, and
   never from its ingredient compartment — steel names coal as carbon, and a smelter that burned
   those units would starve itself on its own recipe. `stock_kind_for_item` classifies hand-fed and
   transported material; recipe inputs outrank fuel so coal goes to a steel recipe's ingredient
   compartment, while `burnable_item` remains the one predicate that decides what a firebox consumes.
   Smelter, kiln, cutter, crusher, and composer are one `BuildingKind` separated by a
   `recipe_category` field and one check, asked at placement and again at reassignment. `Pump` is a
   kind of its own only because it draws from terrain rather than a deposit and never depletes it.
8. Placed entities keep definition, axial anchor, orientation, transport cargo, general container
   inventory, machine ingredient/fuel/output inventories, reserved recipe inputs, progress, fuel
   charge, and scenario ownership separate. Definitions include a bounded axial footprint;
   occupancy, collision, edit targeting, scenario validation, and snapshots rotate the same data.
   Initial entity IDs derive from sorted anchors; later IDs are monotonic.
9. `compile_graph` resolves each entity output into directed transport edges after edits. An
   unconfigured building keeps one facing edge for every product. Once configured, each recipe
   output names one exterior side of one real footprint tile and cargo may use only its own edge.
   Runtime transfers use this compiled graph. Proposals sort by stable entity ID and a rejected
   transfer never changes its source.
10. A construction or removal drag arrives as one bounded command holding two endpoints. Edge belts
    use a deterministic shortest path through cells which pass the ordinary placement predicate,
    bounded by the same 32-cell run cap; this is what lets a run detour around an obstacle without
    putting pathfinding in the host. Other construction and erasure retain `hex_line`, whose
    lowest-numbered closing direction uses at most two directions and turns once. Each resolved cell
    then goes through the same `place` or `erase` a single-cell command uses. Belts are oriented at
    their successor, so the drag routes the line. The preview entry points share that resolver,
    spend materials against a copy of the
    inventory, and carry the recipe the drag will carry — legality depends on the recipe's category,
    so a preview asking without one would refuse a run the drag would build. Undo is a stack of
    constructed entity ids replayed through `erase`; like `deposit_links` it is derived state and is
    never saved, hashed, or checksummed. A screen-vertical run has no hex-edge direction, which is
    what `TRANSPORT_DIRECTIONS` answers: due north is the lattice vector `(q + 1, r - 2)`, a
    non-unit step the ray-cast always handled, and the two straddled hexes stay free and walkable.
    `hex_line_vertical` is the separate rule the drag resolver selects by the dragged definition's
    orientation axis. Sub-hex occupancy was refused: it would change the placement predicate, the
    compiled graph, and the checksum at once to buy a heading the lattice already contains.
11. Extractors resolve their deposit by reference rather than by search. Each extractor's covering
    deposits are resolved once into a candidate list ordered exactly as a full scan would resolve
    it, cached against its stable entity id, and dropped whenever chunk generation adds tiles.
    Remaining quantity is never part of that ordering, so a drained deposit falls through to the
    next candidate without re-resolving. Reported extractor status resolves through the same cache
    rather than a second scan. The cache is derived state: it is never saved, never hashed, and
    tests pin both the reference and the status against the scans they replace.
12. Extractors consume one unit from the finite deposit only when an output can be created.
    Composers reserve exact recipe inputs, charge the recipe's fuel at the moment the craft starts,
    run for integer ticks, and emit only on completion. Extractors, pumps, and composers write to a
    bounded output inventory and keep working until that buffer cannot fit the next whole output;
    a blocked transport edge therefore buffers several cycles without consuming inputs for an
    output it cannot retain. Pumps produce on a cadence while water is in reach and write nothing
    into terrain, because a basin cannot be depleted. Containers store exact quantities; hubs and
    demo consumers count exact deliveries.
13. The landing hub awards integer insight from data-defined item values. Research prerequisites,
    costs, atomic spending, building unlocks, cargo-slot bonuses, build-range bonuses, objective
    progress, and persistent victory all live in Rust.

Blueprint edits retain the previous graph by stable entity ID, invalidate output rays crossing the
changed footprint, and recompile only the affected weak transport components. Component closure
uses both pre-edit links and newly joined targets, so placement/rotation merges and removal splits
match a full deterministic rebuild without resetting machine or cargo state. Full compilation is
still used for scenario initialization and validated save restoration.

`runtime.rs` owns the derived hot-path index rebuilt by those compile boundaries: stable entity and
machine order, entities with compiled outputs, reverse feeders, merger targets, occupied footprint
cells, power participants, and reusable transfer scratch. A tick reads those indexes; it does not
sort entities, invert the transport graph, or scan every footprint to answer occupancy. Like the
graph itself, the index is never saved, hashed, or checksummed, and a test pins it against the
blueprint and graph after both full and incremental compilation.

## Definitions and scenarios

`definitions.json` contains a version plus dynamic items, recipes, and buildings. Buildings include
construction cost, unlock requirement, placement rule, original host icon metadata, description,
movement collision, and one-to-seven-cell footprints. `technologies.json` contains dynamic IDs,
prerequisites, positive integer costs, descriptions, definition unlocks, and bounded player-capability
bonuses; both host and core validate the DAG, references, and bonus bounds.

`scenarios.json` defines the default **New game** and retained **Factory demo**. The default seed has
guaranteed nearby finite ore and crystal plus deterministic generated terrain. The hub and all demo
objects are scenario-owned and cannot be erased.

Items carry a `stack_size`, scenarios carry a starting `carry_slots` count, and researched
technologies may add `carry_slots_bonus`; together they are the whole carrying rule. The save carries
the current count and the loader accepts anything from the earned scenario-plus-research floor up to
`MAX_CARRY_SLOTS`, because creative mode may widen it further.

Items also declare whether they are loose fluid. A belt definition's `transport_medium` is either
solid (the default) or fluid, and the existing native compiled graph moves both without a second
tick or a host-side network. Fresh solid belts refuse loose water and crude oil; pipes refuse solid
items. Filled barrels are ordinary sealed items and therefore ride belts. Containers may publish an
`accepted_item_ids` filter, which makes the water and oil tanks single-fluid stores rather than
generic inventory. Standard-mode player carry refuses loose fluid; barrelled fluid remains portable.

Erasing a player-built entity uses one fixed refund policy: return 100% of its construction cost,
stored inventories, and reserved recipe inputs. In-transit cargo does not teleport into the pack:
it becomes a timed ground item at the removed entity's anchor. Native splits the refund in item-id
order against a working pack: what fits is carried, the rest spills at the site. Single and drag
demolition ask once when stock or a batch is present. The dialog states the one-minute ground-item
timer; no save or wire envelope changes. A removal drag asks native for its released endpoints
before assembling the confirmation, so it cannot use a stale hover preview.

**Creative mode** is one native flag on the core, not a host mode. It enters `checksum()` and the
save envelope beside the pack size, so a creative run is a run like any other rather than a session
the host is quietly pretending about. Turning it on inserts every technology id into `researched`,
which is why nothing had to learn a second way to be unlocked: `technology_met`, `category_unlocked`,
and the build cards all keep asking the one question they already asked. While it is on, placement
skips the cost check and the charge, upgrades neither charge nor credit, and `erase_refund` returns
nothing — which also means a full pack can never refuse a demolition. Four commands belong to it:
`SetCreative`, `Grant`, `Discard`, and `SetCarrySlots`; the last three are refused outside it.
Deliberately unchanged: power, fuel, recipe timing, belt throughput, and hub payouts. A layout tested
in a creative run therefore behaves identically in a priced one, which is the point of having it.

## Command and presentation boundary

`FactoryHost` sends at most one JSON command array per rendered frame, capped at eight commands by
both host and core. Simulation ticks and player steps travel beside it as two separate bounded
counts, because the factory and the player run on separate clocks. TypeScript does not update player
coordinates, quantities, insight, research, machines, cargo, or victory.

Lists that carry a control are patched in place rather than rebuilt. Rebuilding one on every
snapshot destroys the element the pointer went down on, the browser retargets the click to the
container, and a delegated handler resolves nothing — which is how research clicks were being
silently dropped about once a second.

`FactoryRenderer` is the replaceable world boundary. Visual Depth v0.25 supplies its production
implementation with Three.js: an orthographic scene camera at a fixed tilt, twelve discrete
30-degree orbits, bounded zoom, native-snapshot-driven instance buckets, and scene overlays. Picking
marches the pointer ray down the drawn height field and names the cell whose surface it meets,
because a column standing a cliff and three graded steps above its neighbour draws more than a hex
away from the plane point beneath it — the player clicked the top of the rise and native was handed
the cell in front of it. The march runs over the terrain build's own cell map between the tallest
column and the floor every column is drawn down to, so it costs about twenty lookups rather than one
per instance, and fog with no landform keeps the logical plane so it stays pointable. The named cell
is still only a coordinate converted through the public `@hexlife/embed/hex` geometry; rendered
height never enters a command, and native remains the final answer on legality. Native-resolved drag
cells, twelve transport headings, and native chunk coverage are consumed rather than reconstructed.

An orbit remains an integer in `[0, 11]` and moves the instant the key is pressed; only the drawn
heading eases across the 30 degrees, at the rate the 60-degree step always turned at, so a held key
still crosses the circle in the time it used to and a stop is half as far away. The sweep
raises its own dirty frames so it still runs when nothing else redraws, a step pressed mid-sweep
extends the turn already running instead of restarting it, and reduced motion arrives at the same
view with no sweep at all. Directional input reads the heading the sweep is landing on, because a
held movement key is re-read once per turn rather than every frame.

The scene is split by responsibility: `HexSceneCamera`, `terrainMeshes`, `machineMeshes`,
`worldInstances`, `overlays`, shared materials, and quality profiles. Terrain and generated machine
parts are instanced by bounded visual buckets, animation updates existing transforms, and renderer
diagnostics expose draw calls, triangles, geometry/texture counts, CPU preparation, and a rolling
frame p95. Lost WebGL contexts pause drawing and a restored context rebuilds GPU state from retained
snapshot presentation state. New game and load reconcile or dispose scene resources without making
them simulation state. The old hybrid renderer remains source-only for regression tests and is not
reachable from a production entry point or development query switch.

The minimap remains the 178 px instanced WebGL2 view, rebuilt only when surveyed presentation state
changes. The construction grid is hidden outside editing unless explicitly toggled. The command bar,
snapshot-derived next-action guidance, inventory/research panels, construction dock, held touch pad,
camera following, pan/zoom, feedback, and reduced-motion behavior are presentation only. Touch and
keyboard movement share the same bounded native intent commands. `@hexlife/embed/hex` performs
construction projection, rotation, and picking; TypeScript does not integrate player motion.

The browser composition root delegates reusable presentation mechanics rather than growing another
copy: `ui/dom.ts` owns keyed reconciliation, `ui/panels.ts` owns the one-workspace preference and its
DOM synchronization, and `core/frameClock.ts` converts elapsed time into the two bounded native
counts. Each is behavior-tested without Wasm or WebGL; `main.ts` wires them to the live host.

## Save contract

Petroleum Roads (v0.40.0) advances save 31 to 32, definitions 25 to 26, technologies 13 to 14,
and world 9 to 10. Wire 17 and scenarios 7 are unchanged. All existing site rules and stock remain
saved facts; old worlds do not acquire oil. Load verifies the checksum using the original world
stamp, before granting newer creative capabilities. Restoring a world does not run a newer release's
new-game bootstrap eligibility gate, but still validates parameters, references and native state.
The named-save picker mirrors these adjacent envelopes and shows load failures on the title screen.

Recipes retain a primary output and may add up to seven co-products. Each output is a positive
integer quantity, with unique identities and an explicit positive integer cost allocation summing
to 100. The shared output compartment must hold the entire batch before native reserves any inputs;
all products complete atomically through the existing inventory and dirty-delta paths. The shared
buffer does not imply one outlet: `output_routes` is saved and checksummed by stable entity and item,
and each route names one exterior side of one footprint cell. The compiled graph filters offers by
item, while an absent route map preserves the legacy facing outlet. Every compatible station must
fit the batch. Item-level ordered `production_routes` name all producers when alternatives exist;
reachability selects a usable unlocked route and the balance fixture prices each named route and its
whole batch. Definition cycles are refused rather than recursively priced.

An asphalt ground record preserves the gravel base's actual paid bill and adds the top layer's bill.
Strip recovers both; creative construction cannot create a refund. No new floor layer or world-fluid
state is implied by the road; loose petroleum is now routed by the transport-medium rule above.

Every release since has followed the same shape, and the shape is the contract rather than the list:
a release advances only the envelopes it actually changes, verifies the original checksum **before**
applying any migration, and never issues a refund, grant or conversion as a side effect of loading.
New sparse state — a prepared hex, a boundary record, a skills group — is stored only where it
differs from untouched ground, so an older file receives an empty overlay and keeps its original
checksum. A repriced bill is a one-time revaluation and not a loop, because the refund equals the
rebuild cost. A file whose envelope number is not the expected one is left alone rather than
relabelled. Derived availability is never saved or hashed. The per-release detail behind each of
those is in the git history of this file and in `save_migrations.rs`.

Sealed Routes advances save 35 to 36, definitions 26 to 27 and technologies 15 to 16. Migration
records the stable IDs of existing belt-kind entities in `legacy_fluid_belts` only after the old
checksum verifies. That sparse, checksummed compatibility set lets an old liquid belt keep running,
but a newly placed belt cannot accept loose fluid. Removing a grandfathered belt removes its ID; no
replacement belt inherits the exception. Scenarios 7, world 10 and wire 19 are unchanged.

Rust serializes `HXF1` plus JSON containing save/definition/technology/scenario versions, seed,
generated chunks and resource quantities, player and inventory, research, blueprint/entity IDs,
machine and cargo state, counters, tick, victory, and a native checksum. Loading validates versions,
references, uniqueness, and checksum before accepting state. The browser catalog stores that opaque
string in `localStorage`. Export writes the same HXF1 text to a desktop `.hxf1` file, or the whole
catalog as JSON; import reads either shape back into the catalog. Native still validates the
envelope on load. Save/resume and uninterrupted runs converge on the same checksum after equal
commands.

`save_migrations.rs` is the one boundary before typed envelope validation. Current envelopes pass
through byte-for-byte; newer saves and historical versions with no explicit adjacent migration are
refused. A future released migration is added and tested there one version step at a time rather
than guessed inside `from_save`.

## Worker and snapshot boundary

v0.5 moves the Wasm `Factory` off the browser main thread into one dedicated module worker. Worker
messages are serialized through a single operation queue. Each advance applies at most one bounded
native command batch, advances a bounded native tick count, and then requests one snapshot delta;
placement previews, saves, resets, new games, and loads use the same ordered boundary. The main
thread keeps only the latest presentation snapshot and never imports or instantiates the Wasm core.

The first snapshot is complete. Every later native delta carries a base revision, the next revision,
tick, and checksum. Rust compares deterministic snapshot groups and omits unchanged scenario,
progression, player, chunks, terrain, resources, and events. The host rejects missing or
out-of-order revisions before merging a delta. Pointer-driven placement preview requests are
coalesced to one in flight plus the latest pending position.

v0.12.2 changes how that delta travels, not what it says. It is encoded by
`factory-wasm/src/wire.rs` into a compact binary buffer — varints, one byte per closed-set enum,
ascending ids and neighbouring tile coordinates coded as differences — and the worker transfers the
buffer instead of letting the structured clone copy it. `src/core/snapshotWire.ts` decodes it inside
the host's transport, so `FactoryHost` and everything above it still receive exactly the object the
JSON path delivered, down to which keys are absent. `snapshot_delta_json` remains as the oracle the
encoder is pinned against and as the capacity ladder's comparison; the game never ships on it. The
format is pinned in both languages by `fixtures/snapshot-delta-wire.json`, and in Rust by round
tripping every delta the dirty-tracking test produces.

Two properties of that encoding are easy to break and worth stating. The buffer is **transferred,
not structured-cloned**, and the worker checks it owns the buffer whole before handing it over — a
view into wasm memory would detach the module's heap. And entity status travels as an `EntityStatus`
enum whose **serialized spelling is what the player reads**: the wire carries a byte where JSON
carried up to nineteen characters per entity per delta, so renaming a variant is free and respelling
one changes the game's text.

Buildings and resources are the exceptions to group granularity. Buildings travel as a per-entity
patch: `changed` carries inserted and modified entities, `removed` carries dropped ids, and both
arrive in ascending stable entity id order so one linear host pass merges them without re-sorting.
Resources travel as a per-deposit patch keyed by stable deposit id. Deposits are never removed, and
world generation — the only path that adds one — sends the group whole with `replace`, so an
incremental patch always addresses deposits the host already holds and never disturbs their order.
Both a full delta and a post-generation resources group set an explicit `replace` flag and carry the
complete list, so a host with no prior state is still correct. Measurement drove the buildings
patch: at group granularity, one moving item resent every building, recorded as a flat 240–246 bytes
per building at every tier against 103–110 bytes now.

## Dirty tracking

v0.7 stops materializing a complete snapshot per frame in order to diff it, which
`docs/BENCHMARKS.md` had recorded as 55–91% of the measured frame. The core now marks what changed
where state is mutated — entity ids, deposit tile keys, and flags for terrain and the chunk set —
and the delta is built from those marks against a baseline of what the host was last sent. Only
marked entries are materialized at all. The frame cost at the largest measured tier fell 16.8×, and
every tier in the recorded ladder now fits inside a 60 Hz frame.

The marks are derived presentation state: never saved, never hashed, never checksummed, and
incapable of changing a simulation result. They are appended to vectors rather than inserted into
ordered sets, because the tick loop makes thousands per frame; one sort at emit time supplies the
ascending, duplicate-free order the wire format requires. Because every marked entry is still
compared against the baseline before it ships, a mark that turns out to change nothing costs one
wasted rebuild rather than a wrong frame — which is what lets the rare structural paths mark
conservatively. World generation is the important one: it invalidates every resolved deposit
reference, so it must invalidate every entity snapshot derived from one in the same breath.

A missed mark would be a defect, so the shipped builder is pinned against the full-snapshot diff it
replaced: a scripted run covering quiet frames, ticks, gathering to depletion, hub delivery,
research, placement, rotation, erasure, and travel into unsurveyed world asserts after every step
that the two deltas are byte-identical. Reset, new game, and load discard the baseline instead of
patching against a core that no longer exists, so the host receives a complete replacement.

Two scans inside the complete snapshot were quadratic and are gone: extractor status now resolves
through the same cached deposit reference the tick path uses instead of searching every generated
tile, and per-chunk entity counts come from one pass over the blueprint instead of one filter per
chunk. That path still runs for the host's first frame and after every reset, new game, and load.

## Capacity measurement

The ladder that orders this work is one implementation running on two platforms. It lives in the
Rust crate, builds its synthetic tiers from the shipped definitions, and drives them through the
same entry points the worker uses. A `Clock` supplies time — `Instant` natively, `performance.now`
in wasm — and nothing else differs, so a browser record and a native record are comparable by
construction rather than by re-implementation. Because a browser clamps `performance.now` to 100 µs
unless the page is cross-origin isolated, a phase repeats its sample block until it has run long
enough for that step to be a rounding error; only the sample count changes, and each tier's
checksum comes from a core advanced exactly once through its tick budget so extra samples cannot
move the workload's identity.

The harness is measurement code and never becomes shipped code. It enters the wasm artifact only
under the `bench` cargo feature, and the dev-only `/bench.html` page is not part of the production
build, so the deployed artifact carries none of it. That page adds the costs a native run cannot
see, measured through the game's own paths: the worker RPC round trip, `applySnapshotDelta`
merging the patch on the main thread, the Three.js world, and the minimap. Since v0.25 it also
records renderer/profile identity, draw calls, triangles, geometries, textures, CPU preparation,
render p95, and available JavaScript heap evidence.

The measurement has reordered the work three times: the first browser record priced the worker
boundary and made a binary delta encoding the next milestone, that encoding took the boundary out of
the frame and made the engine the cost again, and the renderer measurement that followed removed the
last unmeasured 89% of a frame. `docs/BENCHMARKS.md` carries the records, the method, and the
limits; nothing here restates a number it owns.

## Fog of war

Chunks are the unit of world generation, so the set of generated chunks is exactly the surveyed
world. Each chunk snapshot carries its native world-space origin and span. The Three.js terrain
builder fills the exact union of surveyed pointy-top cells (including implicit lowland) and draws a
frontier only on cell edges whose neighbour is outside that union, so overlapping chunk rectangles
cannot create internal crossings. The inspector reports an unsurveyed
selection and the game menu counts surveyed sectors. **Lowland is the default fill and is
deliberately not sent as terrain**, so a surveyed hex carrying no terrain entry is lowland rather
than an unknown tile — the inspector names every surveyed hex on that basis. None of this is host-invented geography: the
host derives only pixels and copy from native chunk bounds, and travelling generates the chunks that
lift the fog.

## Shipped invariants

The rules below are settled. They are recorded here because each one was reached by hitting the
defect it prevents; a change that contradicts one needs an argument, not an oversight.

- Direction 0 is east, then clockwise E/SE/SW/W/NW/NE. Rust and TypeScript are pinned by
  `fixtures/hex-directions.json`.
- A drag is one bounded command carrying two endpoints. The path between them, the per-cell
  heading, the legality, and the cost are resolved natively by the drag router and the ordinary
  `place` and `erase` paths — and the drag preview comes from that same resolver, so it cannot
  promise a run the drag will not build. Belts use its bounded deterministic obstacle route, over
  every heading the definition's axis allows _and the player has researched_ — so the two-row period
  enters the router the moment it is paid for, and never before; other construction and erasure
  retain `hex_line`. Never expand a drag into per-cell commands on the host, and never give the host
  a line traversal of its own.
- The player walks on its own native cadence, not inside the simulation tick, so a paused or slowed
  factory never pins it in place. The host converts elapsed real time into a step count using the
  rate native publishes and sends it beside the tick count. Frame-coupled movement stays refused:
  the host may send a count, never a position or a delta. That clock owns everything the player
  does themselves, actions as well as walking — work spent per simulation tick froze gathering
  outright while paused and otherwise scaled the harvest rate with the speed setting. So the host
  keeps the player's clock running while a swing is outstanding, not only while walking.
- **A walk to a click is a standing order, and the order is state while the route is a cache.** A
  second click on a selected hex is the player saying where they are going, so `walk_goal` lives in
  `PlayerState`, is saved, and is checksummed: two runs that differ only in where the player is
  headed are not the same run, and a walk that vanished on reload would be a held key rather than an
  order. That is what took the save envelope to 15, with an explicit migration writing
  `walk_goal: null` into a version-14 file rather than letting a defaulting deserializer invent a
  state the file never described. The route is the opposite: a derived index under the same rule as
  `RuntimeIndex`, rebuilt by `rebuild_runtime_index` — which every edit and every load already
  funnels through — and never saved, hashed, or checksummed. Saving it would let a file describe a
  corridor the world it loads into no longer has, and would make the drawn ribbon a promise the
  simulation could not keep.
- **The search answers with the fastest way, not the shortest one, and surveys nothing to find it.**
  The A\* costs shallow water `PLAYER_SPEED / (PLAYER_SPEED / 5)` because that is the fraction of
  speed a ford actually costs; an unweighted search sends the player wading at 5 m/s across water
  they would have walked round. It reads terrain through the pure `terrain_at` and blocking through
  `runtime.occupied`, and calls neither `ensure_tile` nor `generate_chunk`: considering a hex must
  not survey it, because `generated_chunks` is a checksum input. It is bounded by
  `MAX_WALK_DISTANCE`, `MAX_WALK_SEARCH_NODES`, and `MAX_WALK_PATH_CELLS`, and breaks ties on
  `(f, g, q, r)` rather than on heap order, so one click answers the same way in every run. Steering
  writes the intent directly, ahead of the step that consumes it; any `MoveIntent` — including the
  zero one a key release sends — cancels, so the movement keys always take control back. Arrival
  ends a walk silently, and a route that runs out anywhere but the goal ends it with an event.
- **A route is drawn from native's own remaining path and never re-found by the host.** Both the
  world ribbon and the minimap line read `player.walk_path`, the hexes the steering will actually
  consume. A host-side search would be a second pathfinder, and it would eventually draw a way the
  simulation would not take — across water it prices differently, or through a wall raised mid-walk.
- **A harvest is work, and the work comes before the yield.** `action_cooldown` is the swing still
  running, not a debt charged after an instant take: `gather_from` arms it and takes nothing, and
  `finish_gather` moves the deposit and the pack together on the step that completes it. The old
  order made the first gather of every session free — press, bank a unit, then wait — and drew a
  ring for work already paid. The landing re-asks what the start asked, reach included, because a
  swing takes real time; a player who walks out of reach cancels it and is paid nothing. `Core`'s
  `pending_gather` is what the counter is working on, so the two are saved and checksummed together.
- A gather asks the same question an extractor on that hex asks, and facing is not part of it. A
  target weighted by facing counted down a neighbouring hex while the one underfoot stayed full — a
  change with no visible cause. Where the mouse happens to rest is still not something a player
  reads as aiming at a hex, so facing-weighted targeting stays refused.
  **v0.14 makes the argument that rule asked for, and makes a different one.** A right-click is not
  a weighting; it is the player naming a hex on screen, deliberately, so the number that moves is
  the one they pointed at and the cause is visible. `gather_at` therefore takes an explicit target —
  and only the target moves. Reach is unchanged and still `field_covered_at` at the player's own
  radius, so a right-click can never take from a cell an extractor standing there could not. Both
  gathers land in `gather_from`, so the work a material costs, the carrying rule, and the depletion
  mark are one implementation.
- Extraction reach is a definition field, not a constant: `field_covered_at` takes the radius its
  caller reaches, `deposit_candidates` passes the extractor's own, and the hand always passes
  `EXTRACT_RADIUS`. It is still one predicate — placement, the cached candidate list, and both
  gathers share it — so a resolved reference cannot drift from the rule that allowed the building.
  A tier that changes reach must drop that entity's `deposit_links`, which were resolved against the
  old radius.
- Orientation is an axis the definition owns. `DIRECTIONS` (six) is adjacency and power;
  `TRANSPORT_DIRECTIONS` (twelve) is routing, and the six edge headings keep their indices.
  `OrientationAxis::Corner` is the six vertex headings, closed under 60° rotation, and `Any` is both.
  A definition that may face a corner still requires a single-cell footprint because no definition
  needs otherwise; lift that rule only with a real definition that tests the wider path — and note
  that "may face" includes `Any`, which reaches the same untested path the moment it is rotated.
  Never widen `DIRECTIONS`: a boiler that reached two rows would be a silent rule change.
- **The axis is a price, not only a permission.** A vertex heading covers `3 · size` of world
  distance against `√3 · size` for an edge step, so a heading a definition takes for free is strictly
  dominant. `Edge` and `Corner` answer that by being separate definitions with separate cost rows;
  `Any` answers it inside one definition, with `corner_construction_cost` and `corner_technology_id`,
  and validation refuses an `Any` definition that gates none of its headings. The belt is one
  definition on both periods for that reason — a riser is not a different kind of thing, only a
  longer step — and rotation walks all twelve headings in angular order, 30° per press, which is what
  `rotation_walks_every_heading_once_in_angular_order` pins against world vectors rather than against
  the index arithmetic that produces them.
- A junction is a definition flag, never a `BuildingKind` and never a second tick path. `splits`
  compiles every free forward heading into `Links` and offers from `route_cursor`; `merges` accepts
  from behind and arbitrates by `merge_cursor`, the last feeder served, so a merger alternates where
  a plain belt starves whichever lane loses the ascending-id race. Both cursors are saved and
  checksummed state, because a rotation the save forgets is a factory that restores differently than
  it ran. `transfer_cargo` therefore runs two passes — mergers first, then everything else in
  ascending entity id. A belt is a length of conveyor, not a one-tick hop: `advance_belt_lanes`
  runs first, an item takes `belt_transit_ticks()` to cross a hex, and `can_accept` spaces entries
  by `belt_slot_ticks()` so one belt carries one extractor's 120 items a minute. `lane` is saved
  and hashed; `cargo` is only the exit slot waiting to be handed on.
- An underpass is one arm in the graph trace, not a second lattice. `trace_output` is
  `trace_underpass(...).or_else(trace_ray(...))`: an entrance rays past the entities in between to
  the first partner within `MAX_UNDERPASS_SPAN`, and the exit is simply the underpass that found no
  partner ahead. The crossed cells stay singly occupied, buildable, and connected to their own lane,
  so the pair adds a crossing without adding a coordinate. Grade separation is presentation plus that
  one arm; do not give the covered cells a second occupancy or a height of their own. Placement is
  likewise one bounded operation: dragging an underpass resolves the nearest valid heading and span,
  previews only the two portals, checks both endpoints, and places the pair atomically. A click may
  still place a lone endpoint, which behaves as an ordinary belt or pipe until paired.
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
- **The band table answers for ground nobody has worked; a hex answers for itself.** A cliff is the
  one wall made of something the player can take apart, and one `Lower` cut takes it apart —
  `natural_elevation` puts a cliff exactly one step over highland, so the first cut brings the face
  level with the ground beside it. Nothing about the band moves: the cliff is still a cliff, still
  painted as one, and the whole change lives in the ground overlay's signed `elevation`, which is why
  no envelope, checksum or save moved for it and a world nobody has dug is exactly as passable as it
  always was. Anything asking what may happen **on a particular hex** must go through
  `Core::terrain_blocks_movement` / `terrain_blocks_construction` in Rust or `bandAt` in TypeScript,
  never through `blocks_movement` or `TERRAIN_INFO` directly — those two still state the band, which
  is what a legend and a pinned fixture are for.
- **The Phase 8 ground spine is typed before it is physical.** `GroundSpine` separates generated bed,
  substrate, initial hydrology and presentation; `FinishedGround` keeps earthwork, erosion and the
  prepared surface distinct and is the one route to finished elevation and access. Its current
  source is a legacy adapter, so it produces the shipped band steps and the seven-band presentation
  unchanged; no physical scale constant is read until the slice-3 compatibility boundary. The
  cache contains surveyed chunks only, falls back to the uncached source when its world identity no
  longer matches the Core, and is rebuilt rather than saved, hashed or checksummed. The uncached
  source is the cache oracle, and `fixtures/terrain-passability.json` pins the adapter on both sides
  of the host boundary.
- **The Phase 8 physical source is prepared native-side before activation.** Native tests may build
  `GroundSpine::physical`, which translates the drainage prototype onto a deterministic dry,
  walkable seven-hex valley shelf and publishes absolute bed height plus numeric initial water depth,
  surface and discharge through the same cache oracle. Query order and surveyed caching are pinned.
  The constructor and `terra` remain compiled out of wasm, and running `Core` still constructs
  `GroundSpine::legacy`; this is preparation for the one reviewed compatibility bundle, not a hidden
  mixed-scale world.
- **The Phase 8 heightfield renderer is prepared without becoming the live renderer.**
  `heightfieldTerrain.ts` consumes an explicit set of native-published samples, sorts them into a
  query-order-independent build, shares averaged corners across ordinary slopes, keeps water in a
  separate geometry, and emits vertical faces only at declared cliffs and the surveyed frontier.
  Its picker raycasts that visible surface before translating x/z back to an axial cell; native
  remains the legality authority. The module and its focused tests are an activation prerequisite:
  wire 19 publishes no physical samples, `ThreeFactoryRenderer` does not import it, and the shipped
  prism renderer and logical-plane picker remain live until all compatibility envelopes move.
- **An earthworks selection is resolved in three passes, and the footprint survives a refusal.**
  `ground_transaction` runs `ground_resolve` per cell, which records a `blocked` reason on the hex in
  the way instead of aborting the whole edit; then `ground_footprint`, which publishes every selected
  cell whatever its outcome; then `ground_confirm`, which applies the whole-selection gates — cover,
  extractor, escape route, price. The order is the contract: a preview that native will refuse still
  draws its shape and names the hex responsible, because a selection that vanishes at the moment it
  is rejected tells the player nothing about what to fix. Selection shapes are native truth
  (`GroundShape`, up to `MAX_GROUND_CELLS` = 64 hexes), and an outline is defined as the hex-adjacency
  perimeter of its own fill — `frame = perimeter(rect)`, `ring = perimeter(disc)` — so it is one hex
  thick at every size with no rounding rule of its own. The bright rim the renderer draws around a
  ghost is the same perimeter computed host-side and is pure presentation: it decides where to draw a
  line, never what native grades.
- Snapshot deltas cross to the host in the binary wire format, encoded by `factory-wasm/src/wire.rs`
  and decoded by `src/core/snapshotWire.ts`. The decoder's contract is that it produces exactly what
  `JSON.parse(snapshot_delta_json())` produced — the same keys, the same omissions, `null` where
  native sends `null` — so the encoding is transport and nothing above `FactoryHost` knows which one
  delivered a frame. Every value still becomes a JavaScript number on arrival, so the 2^53 rule
  below is unchanged by the format. `snapshot_delta_json` stays as the oracle the encoder is pinned
  against; it is not a fallback path and the game must not ship on it. Wire 21 puts
  `belt_transit_ticks` in every header and an optional `lane` of in-transit items on a belt, each
  coded as ticks since it stepped on rather than as a fraction that would dirty every belt every
  tick.
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
  fuel would need one variant per fuel and would hardcode the bootstrap path. Machine ingredient,
  fuel, and output inventories are separate native maps. `stock_kind_for_item` puts recipe inputs
  first — coal is ingredient in steel — and only then admits another burnable item as fuel.
  `burnable_item` is still the one fuel predicate; the tick, hand transfer, transport acceptance,
  and status that explains a stop must keep asking it.
- A building's `capacity` bounds the ingredient and fuel compartments **per item** and the output
  compartment and a container's store **as one pool**. `room_for_stock` is the single answer, so a
  belt, a hand transfer and a drawn slot cannot disagree. Per item, because a shared ingredient
  total made a full first ingredient close the empty slot beside it — twelve iron plates wedged a
  twelve-capacity composer, and a four-ingredient recipe could hold a working set of nothing. One
  pool for output, because a recipe's whole batch must fit before native reserves any inputs, and
  for a container, because choosing a tier is the storage decision the player is actually making.
- A new machine is a `recipe_category` and a check, not a `BuildingKind` and a tick path. Smelter,
  kiln, cutter, crusher, and composer are one kind. Add a kind only when a building's _source_ is
  genuinely different, which is the whole reason `Pump` is one: it draws from terrain rather than a
  deposit, and its basin never empties.
- Terrain is the material map. Each raw resource is generated only in the band its geography names,
  because a landscape the player cannot read is decoration. A resource reachable from no buildable
  hex is a defect — stone sits against impassable cliffs and is quarried from the hex beside them, or
  from on top of one once the player has cut its face down.
- **A deposit is a site, not a hex, and one patch is one material.** The `site_cell` lattice picks
  one rule per site and a hex belongs to the nearest site that covers it, so purity is a property of
  the model rather than a number that was tuned; `npm run survey` reports it and it may not fall
  below 950 per mille on a shipped preset. Do not reintroduce a per-hex gate that decides _which_
  material a hex holds — that is the defect the lattice exists to remove.
- **The site lattice and the bootstrap table are derived state**, on exactly the terms
  `deposit_links` is: never saved, never hashed, never checksummed, and rebuilt whenever the seed or
  the parameters move. Generation stays a pure function of `(params, seed, q, r)`; a test asserts the
  cached and uncached generators agree, and another asserts the cheap water test agrees with the band
  decision it skips.
- **The clearing generates nothing.** What a new world guarantees is placed by the bootstrap pass
  outside it, as real patches at stated distances, and a world whose opening cannot be placed is
  refused rather than shipped. Do not re-add a hardcoded list of cells inside the clearing; that was
  the sample platter that made every material visible in the first minute.
- Anything the host draws as a proportion must be given both numbers. The swing ring takes
  `action_cooldown` and a published `action_cooldown_total`; inferring a maximum by watching a value
  count down is the host re-deriving native truth.
- Placement asks one overlap question of deposits and obstacles alike, at two tuned depths. Two
  different tests for the same question is the defect v0.10 fixed. `deposit_candidates` and
  `resource_at_world` share that predicate and must keep sharing it, or a resolved extractor
  reference stops matching the placement rule.
- Carrying capacity is a rule over the ordinary `item_id → quantity` inventory, never a stored slot
  array: each item takes one slot per part-filled stack of its own `stack_size`, against a slot
  count the scenario fixes. Every path that adds to the player asks first. An erase carries what
  fits and spills the rest as saved, checksummed ground items: the recovery stays exactly 100%.
- A `stack_size` is chosen against the recipes that item is in, not by category feel: every input
  and output quantity of every recipe divides the stack size of its own item, so a stack is always
  a whole number of crafts and never ends mid-batch. Six wood to a charcoal against a stack of
  twenty stranded two wood at the bottom of every stack the player carried. `tests/definitions.test.ts`
  pins the rule arithmetically, so a new recipe ratio fails the gate rather than the player.
- A cursor-held stack is native inventory state, not DOM drag data. Left click lifts or places a
  full stack, right click halves a lift or places one, Ctrl-click moves one, and Shift applies the
  same quantity as a quick move between the pack and selected building. Every gesture sends a
  bounded command; native owns reach, stock compatibility, remaining room, save, and checksum.
  Pointer dragging previews a lift and queues the same pickup/place commands together on release;
  the bounded queue accepts both or neither. Outside drops and cancelled gestures send nothing.
  Slot keys exclude quantities, and the pickup freezes its source address at press time.
- Extractors, pumps, and composers write into bounded output inventories. They request work only
  while the next whole output fits, so a blocked edge fills a finite buffer and then stops without
  consuming a deposit or recipe inputs it cannot preserve. Transport offers from that buffer, not
  from a presentation-side queue.
- Any host list carrying a control is patched in place, never rebuilt. A `replaceChildren` between
  pointerdown and pointerup detaches the pressed control and the delegated click resolves to
  nothing. This now covers the hotbar slots and the catalogue cards as well as the research list and
  the inventory and machine-compartment grids. An item chip is
  created once per holder and patched from then on, so a chip inside such a list satisfies the rule
  by construction rather than by being remembered at each call site.
- An item is drawn one way, by `src/rendering/itemChip.ts`, and never by a second shape. Every
  variant is a modifier class on that one markup, every chip shows its glyph, and `3` and `3 / 10`
  are the only two spellings of a quantity — one an amount, the other progress toward a known
  target. HTML names a `.chip-host` for a chip to be built into; markup that spells a chip out by
  hand is the drift this replaced, and `tests/host.test.ts` refuses it.
- The active workspace panel is presentation state and lives in `localStorage` under
  `hexfactory:panels:v1`, on the same terms as the hotbar arrangement: never saved with the game,
  never hashed, never sent, and a stored id validated against the live document on load. One
  workspace opens at a time at every width; opening another, clicking the world, `Escape`, a new
  game, and a load clear the old one. Panels remain flow children of a rail and never position
  themselves. The inspector may stay beside a left workspace on wide screens, but yields to the
  menu and timer in its own rail.
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
- Named saves live in a version-independent catalog (`hexfactory:saves:v1`). Each slot records the
  envelope versions and the world it was started with (seed, scenario, preset or custom scales).
  Incompatibilities stay visible on the row; they are never hidden by putting those numbers in the
  storage key. Native still refuses a load the numbers cannot support. `SAVE_VERSION` is the one
  literal, because native does not publish it. Leftover `hexfactory:hxf1:` keys are imported into
  the catalog and left in place. A slot can also be written to a desktop `.hxf1` file (the native
  envelope) or the whole catalog to `hexfactory-saves.json`; importing either shape mints new slot
  ids and will not overwrite an existing name.
- No scripted guide may outrun the rules it is explaining. The next step is derived — the contract's
  outstanding bill, expanded through the recipe tree, plus the technologies those machines sit
  behind and the power branch none of them names — so every step it can produce is achievable in the
  state that produced it. A machine that draws power with nothing generating it is a factory that
  cannot run, and both the guidance and the balance openings now price that.
- Derived caches never become truth. Resolved extractor deposit references are rebuilt from tiles,
  invalidated when chunk generation adds tiles, and are never saved, hashed, or checksummed.
- A milestone that changes the world generator, the item roster, or the entity snapshot re-runs
  `npm run bench` before it ships. v0.12's re-measurement found two regressions it had introduced —
  86 KB of delta payload and a 3.9× slower snapshot — and one 3.0× saving v0.11 had shipped without
  measuring. A checksum change invalidates checksum comparisons, not timing ones: say which of the
  two a record is claiming.
