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
   the player wading at 1 m/s across water they would have walked around. It reads terrain through
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
9. `compile_graph` resolves each entity output into one directed transport edge after edits. Runtime
   transfers use this compiled graph. Proposals sort by stable entity ID and a rejected transfer
   never changes its source.
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

Erasing a player-built entity uses one fixed refund policy: return 100% of its construction cost,
stored inventories, and reserved recipe inputs. In-transit cargo does not teleport into the pack:
it becomes a timed ground item at the removed entity's anchor. The pack refund is resolved before
removal and refused if it will not fit, while spilled cargo remains world-owned and collectible.

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
implementation with Three.js: an orthographic scene camera at a fixed tilt, six discrete 60-degree
orbits, bounded zoom, native-snapshot-driven instance buckets, and scene overlays. Picking
intersects the unchanged logical axial plane and converts through the public
`@hexlife/embed/hex` geometry; rendered terrain height never enters a command. Native-resolved drag
cells, twelve transport headings, and native chunk coverage are consumed rather than reconstructed.

An orbit remains an integer in `[0, 5]` and moves the instant the key is pressed; only the drawn
heading eases across the 60 degrees, over roughly half a second and never more than one. The sweep
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

Primitive processing (v0.26.0) uses the existing composer kind and compartment inventories.
An optional `recipe_ids` capability list replaces category matching; `duration_multiplier` scales
the same recipe's native work duration. `manual_work` stations start disabled. Their existing
saved `disabled` flag is a one-batch work permit, with at most one attended workshop active.
Native checks stationary attendance within one hex and an idle gather action before working,
pauses when those conditions fail, and disables the station at completion. Pausing retains the
job; dismantling cancels and refunds reserved inputs through the normal erase path. No host timer
or new wire state is involved. Save 18 advances definition catalog 15 to 16 without changing
existing entity state, research, quantities, bills or checksum; the adjacent migration is tested
against a running legacy factory. Scenario text changes do not change scenario version 5.

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
