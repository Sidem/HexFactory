# HexFactory architecture

Everything below serves the goal in `docs/HEXFACTORY-PLAN.md`: an open-ended factory game that is
fun to play and a pleasure to control. Determinism, sparse cost, and native ownership of the tick
are here because a world that large has to stay responsive, restore exactly, and keep growing —
not as ends in themselves.

HexFactory is not a cellular automaton. Exploration uses unbounded continuous fixed-point world
space. Pointy-top axial coordinates exist only for construction anchors/footprints and compiled
transport; the running simulation follows graph edges and sparse scheduled entities.

## Dependency boundary

`@hexlife/embed/hex@1.15.0` remains the only HexLife dependency and is exactly pinned. TypeScript
uses its public, DOM-free entrypoint for clockwise directions, footprint rotation, construction
coordinate conversion, picking, and Canvas centers. The independent Rust crate pins the same
direction fixture and never reads HexLife source or `node_modules`. No package release was required.

## Native ownership

The Rust `Core` owns all state that can change a game result:

1. A versioned seed and integer value noise generate fixed-size axial environment chunks without
   shared traversal state. Terrain bands and resource fields are derived from seed and hex; only
   the sparse depletion overlay, the generated chunk set, and ordinary simulation state are
   checksum inputs. Collision and placement read the hex under the point.
2. The player has native integer `x/y`, facing and bounded movement intent vectors, action cooldown,
   world-unit build range, a carrying slot count, and an ordered `item_id → quantity` inventory.
   Gathering, delivery, construction costs, erasing, withdrawal, and research are native.
3. Walking runs on the player's own native cadence rather than inside the simulation tick, so a
   paused or slowed factory does not pin the player in place. The host converts elapsed real time
   into a step count using a rate native publishes, and sends that count beside the tick count; it
   never turns a frame delta into a position, so the same commands and counts still reproduce the
   same position and the same checksum.
4. Carrying capacity is a rule over the ordinary inventory, not a stored array of slots: each item
   occupies `ceil(quantity / stack_size)` slots and a scenario fixes the slot count. Every path that
   adds to the player — gathering, erasing, withdrawing — asks first. Gathering into a full pack is
   refused; an erase whose refund would not fit is refused whole, which is the only one of refuse,
   partially refund, and spill that keeps item conservation exact and leaves the recovery available
   once there is room; a withdrawal moves what fits and leaves the rest in the container. Like
   `build_range`, the slot count is a scenario property validated on load rather than a checksum
   input, so the save format and every existing checksum are untouched by it.
5. Placement asks whether the hex is a field cell (for extractors) or blocking terrain (for
   everything). `deposit_candidates` and `resource_at_world` share that field predicate. Extractors
   harvest every field cell within hex radius 1.
6. Placed entities keep definition, axial anchor, orientation, cargo, inventory, reserved recipe
   inputs, progress, and scenario ownership separate. Definitions include a bounded axial footprint;
   occupancy, collision, edit targeting, scenario validation, and snapshots rotate the same data.
   Initial entity IDs derive from sorted anchors; later IDs are monotonic.
7. `compile_graph` resolves each entity output into one directed transport edge after edits. Runtime
   transfers use this compiled graph. Proposals sort by stable entity ID and a rejected transfer
   never changes its source.
8. A construction or removal drag arrives as one bounded command holding two endpoints. `hex_line`
   walks between them by taking the lowest-numbered direction that closes the distance, so a run
   uses at most two directions and turns once, and each cell then goes through the same `place` or
   `erase` a single-cell command uses. Belts are oriented at their successor, so the drag routes the
   line. The preview entry points share that resolver and spend materials against a copy of the
   inventory, so what is drawn during a drag is what the drag will build. Undo is a stack of
   constructed entity ids replayed through `erase`; like `deposit_links` it is derived state and is
   never saved, hashed, or checksummed.
9. Extractors resolve their deposit by reference rather than by search. Each extractor's covering
   deposits are resolved once into a candidate list ordered exactly as a full scan would resolve it,
   cached against its stable entity id, and dropped whenever chunk generation adds tiles. Remaining
   quantity is never part of that ordering, so a drained deposit falls through to the next candidate
   without re-resolving. Reported extractor status resolves through the same cache rather than a
   second scan. The cache is derived state: it is never saved, never hashed, and tests pin both the
   reference and the status against the scans they replace.
10. Extractors consume one unit from the finite deposit only when an output can be created.
    Composers reserve exact recipe inputs, run for integer ticks, and emit only on completion.
    Containers store exact quantities; hubs and demo consumers count exact deliveries.
11. The landing hub awards integer insight from data-defined item values. Research prerequisites,
    costs, atomic spending, unlocks, objective progress, and persistent victory all live in Rust.

Blueprint edits retain the previous graph by stable entity ID, invalidate output rays crossing the
changed footprint, and recompile only the affected weak transport components. Component closure
uses both pre-edit links and newly joined targets, so placement/rotation merges and removal splits
match a full deterministic rebuild without resetting machine or cargo state. Full compilation is
still used for scenario initialization and validated save restoration.

## Definitions and scenarios

`definitions.json` contains a version plus dynamic items, recipes, and buildings. Buildings include
construction cost, unlock requirement, placement rule, original host icon metadata, description,
movement collision, and one-to-seven-cell footprints. `technologies.json` contains dynamic IDs, prerequisites, positive integer
costs, descriptions, and definition unlocks; both host and core validate the DAG and references.

`scenarios.json` defines the default **New game** and retained **Factory demo**. The default seed has
guaranteed nearby finite ore and crystal plus deterministic generated terrain. The hub and all demo
objects are scenario-owned and cannot be erased.

Items carry a `stack_size`, and scenarios carry a `carry_slots` count; together they are the whole
of the carrying rule.

Erasing a player-built entity uses one fixed refund policy: return 100% of its construction cost,
plus its cargo, inventory, and reserved recipe inputs. This is native and covered by conservation
tests. Since v0.10 the whole refund is resolved before the removal and refused if it will not fit
in the player's pack, so the policy stays exactly 100% rather than becoming "as much as fits".

## Command and presentation boundary

`FactoryHost` sends at most one JSON command array per rendered frame, capped at eight commands by
both host and core. Simulation ticks and player steps travel beside it as two separate bounded
counts, because the factory and the player run on separate clocks. TypeScript does not update player
coordinates, quantities, insight, research, machines, cargo, or victory.

Lists that carry a control are patched in place rather than rebuilt. Rebuilding one on every
snapshot destroys the element the pointer went down on, the browser retargets the click to the
container, and a delegated handler resolves nothing — which is how research clicks were being
silently dropped about once a second.

The replaceable Canvas 2D renderer consumes snapshots and draws continuous regions/resources,
multi-cell buildings, player, hover, selection, build radius, legality, definition labels, cargo
layers, and the fog of war over ungenerated world. The construction grid is hidden outside editing unless explicitly toggled. The command bar,
snapshot-derived next-action guidance, inventory/research panels, construction dock, held touch pad,
camera following, pan/zoom, feedback, and reduced-motion behavior are presentation only. Touch and
keyboard movement share the same bounded native intent commands. `@hexlife/embed/hex` performs
construction projection, rotation, and picking; TypeScript does not integrate player motion.

## Save contract

Rust serializes `HXF1` plus JSON containing save/definition/technology/scenario versions, seed,
generated chunks and resource quantities, player and inventory, research, blueprint/entity IDs,
machine and cargo state, counters, tick, victory, and a native checksum. Loading validates versions,
references, uniqueness, and checksum before accepting state. `localStorage` stores only that opaque
string. Save/resume and uninterrupted runs converge on the same checksum after equal commands.

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
build, so the deployed artifact carries none of it. That page adds the two costs a native run
cannot see, measured through the game's own paths: the worker RPC round trip, and
`applySnapshotDelta` merging the patch on the main thread.

v0.8 recorded the first browser tiers, and they moved the roadmap. The wasm engine costs about 1.2×
native, so the native work of v0.6 and v0.7 transferred intact. The worker boundary costs roughly
60% of a host frame and scales with the JSON delta at about 10 µs per kilobyte, which makes a
compact binary encoding over a transferable buffer the next change worth making. The per-entity
merge costs about 1% of a frame and needs nothing. A renderer decision remains gated on measuring
the renderer, which `docs/BENCHMARKS.md` now names as the second follow-up.

## Fog of war

Chunks are the unit of world generation, so the set of generated chunks is exactly the surveyed
world. Each chunk snapshot carries its native world-space origin and span, and the host renders
everything outside those bounds as fog: a hatched cool veil punched out by the surveyed rectangles
on an offscreen layer, so overlapping chunk edges leave no seams, with a dashed frontier drawn along
every surveyed edge whose neighbouring chunk does not exist yet. The inspector reports an unsurveyed
selection and the game menu counts surveyed sectors. None of this is host-invented geography: the
host derives only pixels and copy from native chunk bounds, and travelling generates the chunks that
lift the fog.
