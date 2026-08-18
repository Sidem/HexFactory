# Material Base v0.12 scope and acceptance

Status: Renderer Measure v0.12.4 is shipped on Sightlines v0.12.3, Binary Delta v0.12.2, Playtest
Feel v0.12.1, Material Base v0.12, World Shape v0.11, Playability v0.10, Game Feel v0.9, Browser
Capacity v0.8, Sparse Snapshot v0.7, Sparse Cost v0.6, Worker Boundary v0.5, and Command Surface
v0.4. v0.11 changed what the world looks like; v0.12 changes what it is made of — eight raw
resources correlated with terrain, fourteen recipes across five machine categories, fuel as a
property of items, renewable flora, and a pump that draws from a basin. v0.12.1 thins generation
and quiets the first-minutes presentation. v0.12.2 changes nothing the player can name and
everything about what a frame costs: the snapshot delta crosses as a compact binary buffer.
v0.12.4 times the two canvases the game draws, so a browser frame is accounted for end to end.
`WORLD_GENERATOR_VERSION` 5 and `HXF1` save version 5 reject earlier envelopes. The capacity
ladder is re-pinned; a generator bump invalidates checksum comparisons, not timing ones.

## Binary delta contract

- The delta the game ships is `snapshot_delta_bytes`, encoded by `factory-wasm/src/wire.rs`. LEB128
  varints, zigzagged where signed; one byte for each closed-set enum; entity ids, removal ids, and
  tile coordinates coded as deltas against what precedes them; a footprint cell coded against its
  own entity's hex; a bit per absent option in place of a field name and a `null`.
- **The decoder produces exactly what `JSON.parse(snapshot_delta_json())` produced.** Same keys,
  same omissions, `null` where native sends `null`, `fuel_charge` absent rather than zero. That is
  the whole safety argument for the milestone: the host, the renderer, and every test above them
  were written against the JSON shape and none of them changed. Three shape divergences were caught
  by the fixture during development and fixed in the decoder rather than papered over.
- The buffer is transferred, not structured-cloned, and the worker checks it owns the buffer whole
  before handing it over — a view into wasm memory would detach the module's heap.
- `snapshot_delta_json` is retained as the encoder's oracle and as the capacity ladder's comparison.
  It is not a fallback: nothing in the game may ship on it.
- Entity status is an `EntityStatus` enum whose serialized spelling is what the player reads. It
  exists so the wire carries a byte where JSON carried up to nineteen characters per entity per
  delta; renaming a variant is free, respelling one changes the game's text.
- Both languages are pinned to `fixtures/snapshot-delta-wire.json`, in the role
  `fixtures/hex-directions.json` plays for the direction table. Rust also round trips every delta a
  running factory produces, which reaches the entity and group combinations a fixture cannot
  enumerate.
- Measured, not asserted: per-frame payload 13.6× smaller at the largest tier (644,759 → 47,531
  bytes), the worker boundary 21.7× cheaper (6,085 → 280 µs), and a host frame down from 62.1% of
  60 Hz to 11.0%. The shipped wasm grew 3.7 KiB. See `docs/BENCHMARKS.md`.

## Material base contract

- Terrain is the material map. Each of the eight raw resources is generated only where its band
  says it belongs, so reading the landscape is how a site is chosen. Water is the exception: it is
  pumped rather than mined, so a basin cannot be depleted and has no overlay entry.
- Fuel is a property of `ItemDefinition`, never an entry in a recipe's `inputs`. A machine burns
  from its own stock, lowest item id first, and never from the quantity a recipe input reserves —
  steel names coal as carbon, and a smelter that burned those units would starve itself. One
  predicate, `burnable_item`, serves both the tick that burns and the status that explains why
  nothing did.
- Smelter, kiln, cutter, crusher, and composer are one `BuildingKind` with different
  `recipe_category` values. The rule is checked at placement and again at reassignment, because a
  machine that could be reassigned past it would make the rule decorative.
- `Pump` is a `BuildingKind` because it draws from terrain rather than from a deposit. Everything
  else the material base adds is data.
- Flora regrowth walks a derived set of cut cells, not the world. The set is a pure function of the
  overlay and the item definitions, so it is rebuilt on load and never saved, hashed, or
  checksummed — the same rule `deposit_links` follows.
- `set_recipe` is a bounded, range-checked command beside `place`, `erase`, and `withdraw`. It
  refuses a machine mid-craft: reserved inputs belong to the job that reserved them.
- A drag preview carries the recipe the drag will carry. Legality now depends on the recipe's
  category, so a preview asking without one would refuse a run the drag would build.
- The wait between two field actions is drawn where the action happens, from `action_cooldown`
  against a published `action_cooldown_total`. The host draws a proportion it was given.
- The inspector names every surveyed hex. Lowland is the default fill and is deliberately not sent,
  so a surveyed hex with no terrain entry is lowland — not an unknown tile.

## World Shape contract

- Generation is a pure function of seed and axial hex. Feature circles on a rectangular lattice
  are gone. `world_to_axial` inverts `axial_world` with integer cube rounding.
- Terrain is read from elevation and moisture bands. Cliffs come from the elevation gradient.
  Deep water, shallow water, and cliffs block walking and construction; shore, lowland, and
  highland do not.
- A field cell is `(item_id, richness)` above a threshold. Only drawn-from cells are stored in
  the depletion overlay. The overlay is saved, hashed, and checksummed; the generated field is
  not.
- `deposit_candidates` and `resource_at_world` share `field_covered_at`: hex distance at most
  `EXTRACT_RADIUS` (1) and a field present. Remaining quantity is not part of the order.
- Player radius is published on the snapshot. The host draws the body from that radius.
- Fog still punches native chunk bounds. Those bounds are now the bounding square of the chunk's
  hexes.

## Playability contract

- Placement asks one overlap question of deposits and of obstacles, at two tuned interpenetration
  depths. `deposit_candidates` and `resource_at_world` share that predicate, so a cached extractor
  reference cannot drift from the rule that allowed the placement.
- Walking runs on a native cadence of its own, independent of pause and of the speed multiplier. The
  host converts elapsed real time into a step count using a rate native publishes, and sends that
  count beside the tick count. It never sends a position or a delta, so browser frame rate still
  cannot change a deterministic result.
- Carrying capacity is a rule over `item_id → quantity`: `ceil(quantity / stack_size)` slots against
  a scenario slot count. No slot array is stored, so the save format, the checksum inputs, and every
  ordering guarantee are unchanged. The slot grid draws stacks native resolved.
- Gathering into a full pack is refused, a withdrawal moves what fits, and an erase whose full
  refund will not fit is refused whole — so the refund policy stays exactly 100% and nothing is ever
  destroyed. The removal preview reports the refusal.
- `withdraw` is a bounded, range-checked command beside `place` and `erase`. Its quantity is a
  ceiling, not a demand.
- Host lists that carry a control are patched in place. Rebuilding one between pointerdown and
  pointerup detaches the pressed control and loses the click; that was the research-panel defect.
- `HXF1` save version 3 and definition version 4 reject earlier saves. A pack that cannot hold what
  an older save recorded is not the same game, so there is no migration.

## Browser capacity contract

- The capacity ladder is one implementation, in Rust, run on two platforms. A `Clock` supplies the
  time: `Instant` natively, `performance.now` in wasm. Nothing else about a measurement differs
  between them, so a browser record and a native record are comparable by construction.
- The harness enters the wasm artifact only through the `bench` cargo feature. The shipped build
  never enables it, `bench.html` is not part of the production Vite build, and neither is a
  dependency of the game or of the CI gate. Measurement code never becomes shipped code.
- A phase repeats its sample block until it has run a minimum duration, so a browser's clamped
  100 µs clock buys precision with samples rather than accuracy. Sample counts may differ between
  records; the workload may not. Each tier's checksum comes from a core advanced exactly once
  through its tick budget, so it cannot move with the sample count.
- The browser harness additionally measures the worker RPC round trip and the main-thread delta
  merge through the game's own code paths — the same bounded command batch, the same
  `snapshot_delta_json`, the same `applySnapshotDelta`. From v0.12.4 the same page also times the
  two canvases the game draws. Complete browser frame-rate claims cite that record, at its pinned
  viewport.
- Results are recorded in `docs/BENCHMARKS.md` with the machine, browser, clock resolution, and
  limits that produced them. A performance claim without a recorded tier behind it is a defect.

## Sparse cost contract

- Extractors hold a resolved deposit reference instead of scanning every generated tile each tick.
  The cached candidate list is ordered exactly as the scan it replaces, is invalidated whenever
  chunk generation adds tiles, and is derived state: never saved, never hashed, never a checksum
  input. Measured: tick cost is now linear in entity count and 233× cheaper at the largest tier.
- The buildings delta is per-entity. Rust sends changed and removed entities in stable id order; the
  host merges them in one linear pass and rejects revision gaps exactly as before. A full delta
  still carries the complete list under an explicit replace flag. Measured: 2.3× less payload per
  frame at every tier.
- Both changes are behaviour-preserving. The pinned capacity workload, every determinism test, and
  the `HXF1` contract produce identical checksums before and after.

## Fog of war

- The generated chunk set is the surveyed world. Each chunk snapshot carries its native world-space
  origin and span; the host renders everything outside them as a hatched veil with a dashed survey
  frontier, and travelling generates chunks that lift it permanently.
- The inspector names an unsurveyed selection, the game menu counts surveyed sectors, and the
  landing guidance explains that the fog is unexplored world rather than a rendering edge.
- Fog is presentation over native truth. The host derives pixels and copy from reported chunk
  bounds; it never invents terrain, resources, or geography beyond them.

## Worker and delta contract

- Only the dedicated module worker imports and instantiates `factory_wasm`. The main thread sends
  ordered RPC requests and keeps a cached presentation snapshot.
- A frame combines at most one bounded command batch and a bounded native tick count into one worker
  advance. Rust remains the only running tick and state owner.
- The initial snapshot is complete. Later native deltas always carry base revision, next revision,
  tick, and checksum, and omit unchanged snapshot groups. Buildings travel as a per-entity patch
  rather than a group. The host rejects revision gaps before applying presentation patches.
- Reset, new game, load, save, and placement legality all cross the same worker boundary. Placement
  previews are coalesced so pointer movement cannot create an unbounded request queue.
- Deterministic checksums are unchanged and save strings remain opaque to the browser host. `HXF1`
  moved to version 3 in v0.10 because the player state it records gained a carrying slot count.

## Player-facing command surface

- The continuous world fills the available viewport. A compact command bar keeps the landing
  directive, progress, insight, pause state, and game menu visible without pushing play below the
  fold.
- Snapshot-derived guidance names the next useful action across gathering, delivery, research,
  automation, composition, victory, and the Factory demo. It is explanatory host presentation; it
  does not mutate or reconstruct native state.
- The inspector is the only panel that sits over the world. The cargo pack (`I`), research (`O`), and
  the objective-and-controls guide (`P`) open one at a time and close on `Escape`; Gather, Deliver,
  and the carried-slot count stay in permanent chrome because they are the loop rather than a
  reference. Research shows prerequisite, affordability, and completion states. The bottom
  construction dock keeps inspect/edit/build, locks, exact costs, and orientation in one spatial
  workflow.
- A minimap draws the surveyed world, the landing hub, and the player; when the hub is off screen a
  gold bearing marker on the edge of the view names the direction and the distance home. `Space`
  centres the camera on the player, which is also what restores following after a pan.
- Impassable ground — deep water, shallow water, cliff — is drawn as one hatched category before it
  is drawn as a material, from the same passability table `fixtures/terrain-passability.json` pins
  against native.
- Resource labels expose identity and remaining quantity; machines expose definition identity,
  direction, progress, inventory, and snapshot-backed cargo animation in the world.
- At 390 px the map remains the primary surface. Mission, research, and session controls become
  dismissible overlays, while a held four-direction touch pad emits the same bounded movement
  intents as WASD and direct Gather/Deliver actions remain available.

## Playable loop

The loop remains intentionally compact:

`explore freely → read the terrain for materials → gather → deliver for insight → research → construct nearby → process and combine → automate → win`

The player carries exact native inventory quantities, up to a fixed number of stacks. Field Logistics
unlocks belts, Automated Extraction unlocks extractors, Composition unlocks the composer, and Storage
Planning unlocks containers — which since v0.10 are a real answer to a full pack, because stock can
be taken back out of them by hand. Construction spends that inventory atomically. Extractors consume finite continuous
resource regions, transport runs on compiled edges, and delivering three components sets persistent
native victory while leaving free play enabled.

## World and interaction contract

- Player and environment positions are integer fixed-point `x/y` owned by Rust. The host sends only
  bounded held-key intent from `W/A/S/D` plus the number of player steps the frame's real time is
  worth; native owns movement, facing, sliding collision, continuous chunk generation, and checksums.
  Movement runs on the player's own cadence rather than inside the simulation tick, so pause and the
  speed multiplier do not change how fast the player walks.
- Water, rock, and resource regions are continuous circular features. Resources show kind and
  remaining quantity in the world and can be identified while exploring. `F` gathers within native
  reach; `X` delivers within native hub distance.
- Hex geometry is reserved for building anchors, orientation, footprints, editing, and compiled
  transport. The two-cell composer and three-cell landing hub prove rotated multi-cell occupancy.
- Rust enforces technology, carried construction cost, continuous player proximity, complete
  footprint occupancy, environment collision, player collision, deposit requirements, recipe
  selection, protected scenario objects, refunds, and legal rotations—even for forged commands.
- The tile-edge overlay is absent during ordinary exploration. It appears while placing, erasing,
  or rotating, and can be explicitly toggled. Build mode also shows the native proximity radius and
  the complete rotated footprint preview.
- Canvas following, pan/zoom, responsive panels, feedback, accessibility, and reduced motion remain
  host presentation. No TypeScript player, environment, inventory, progression, or cargo tick exists.

## Determinism and persistence

Continuous features are generated into lazy ordered chunks from the versioned seed and coordinate
hash, without traversal-order state. Stable native entity IDs still arbitrate transfers. Blocked
transfers and machines leave their sources unchanged.

`HXF1` save version 3 and world-generator version 2 serialize continuous player/feature truth,
footprints, inventories, research, machines, cargo, objective, and checksum. The loader validates
versions, references, feature uniqueness/radii, entity IDs, footprint overlap, input bounds, the
carrying slot count against its scenario, and the checksum. Held movement intent is neutralized after
a successful restore. Earlier saves are rejected with an explicit incompatible-version error; browser
storage treats the native string as opaque.

## Verification coverage

Native tests cover direction fixture parity; seeded and request-order-independent generation;
continuous intent, diagonal normalization, stopping, obstacle collision, and input bounds; finite
gathering/conservation; proximity/environment/footprint/cost/technology/deposit placement;
footprint-aware graph compilation; exact refunds; extraction depletion; research atomicity; forged
commands; complete victory; HXF1 equivalence and incompatibility; stable IDs; transport conservation,
backpressure, recipe timing, container order, delivery totals, and reset/replay.

Host tests cover the exact published geometry package, camera-aware construction picking with a
continuous camera center, WASD intent normalization, bounded batching/encoding, absence of host
simulation mutation, footprint and technology definition validation, costs/locks, snapshot/save
delegation, worker-only Wasm ownership, revision enforcement, responsive breakpoints, reduced
motion, and accessible labels. Native tests pin dirty-group omission and revision metadata alongside
the existing simulation invariants.

Native tests also pin the capacity workload's checksum, delivery rate, and entity count, and assert
that the capacity ladder still produces a result for every tier. The benchmark itself stays outside
the gate because shared runners cannot produce comparable timings.

v0.6 adds native coverage for resolved deposit references matching a full tile scan across
generation, depletion, and erasure; per-entity buildings deltas reporting only changed and removed
entities while a full delta stays a complete replacement; and chunk bounds reporting the surveyed
world and growing as the player travels. Host tests add per-entity patch merging — in-place updates,
ordered inserts, removals, replacement, and untouched groups — and pin the fog to native chunk
bounds rather than host-side geometry.

v0.8 adds native coverage for the ladder's platform independence: phases reported per sample against
an injected clock, a phase budget that adds samples without moving the tier's checksum, delivered
total, or entity count, a resumable ladder that reports only the tiers it measured and re-measures a
tier in place, and the round-trip factory arriving warm and sending a complete first delta followed
by patches. Host tests cover the report assembly the browser page contributes — pairing host results
to tiers by key, rendering an unmeasured tier as absent rather than free, the 60 Hz share, the
entity-count check on the applied snapshot, and a clock-resolution probe against both a clamped and
a fine-grained clock.

v0.11 adds native coverage for hex-lattice fields and terrain bands: `world_to_axial` inverts
`axial_world`; landing water and cliffs stay legal to stand beside and illegal to build on; an
extractor at a guaranteed ore cell also sees a neighbour written into the overlay; unmined field
does not appear in the overlay; generating a chunk does not change the checksum until something
is taken. Host tests accept the new terrain names, published player radius, and item icon keys.

v0.12 adds native coverage for the material base: every band holds only the resources its geography
allows and the landing clearing guarantees one cell of each tier-1 material on foot; a machine burns
fuel from its own stock and refuses to burn the coal a steel recipe is waiting on; cut flora climbs
back to what generation gave it, leaves the regrowth set when full, never contains an ore cell, and
is rebuilt from the overlay on load; a pump produces beside water, writes nothing into the overlay,
and is refused away from it; and a machine runs only its own category of recipe, at placement and at
reassignment alike, with a mid-craft reassignment refused. The dirty-delta gate gains a flora step,
because regrowth is the only thing that moves a deposit without an extractor or a player touching it
that frame. Host tests cover the recipe picker offering each machine only its own category, the
cooldown ring drawing from published native numbers rather than an inferred maximum, and the
inspector naming every band including the one native does not send.

v0.11.1 adds native coverage for the three defects the dense field made visible in harvesting: a
gather takes from the hex the player stands on from every position inside it and at every facing;
its reach is the extractor predicate and is the same in all six directions; and the cooldown
between two gathers is paid in player steps, so it clears while the factory is paused and is not
cleared by running the factory alone. Host tests pin the resources patch to the tile key, with a
column of negative-coordinate cells whose packed 64-bit ids used to round to one JSON number —
harvesting one of them overwrote the rest with a copy of it.

v0.10 adds native coverage for the single placement overlap rule — a deposit displaced most of a hex
step is still minable, the extractor's cached reference resolves the same deposit the placement rule
allowed, and an obstacle blocks only past the intrusion depth; for the carrying rule, its stack
arithmetic, a refused gather, and the stacks the host draws; for an erase refused rather than losing
items, with the removal preview agreeing; for withdrawal clamped by stock and by space; and for the
player's cadence advancing while the factory is paused, not advancing when only ticks are spent, and
covering the same ground at any simulation speed. Host tests add the withdraw opcode, the separate
player-step count on the wire, the cadence coming from native rather than from a frame delta, and
that every list carrying a control is patched in place rather than rebuilt.

The local release gate is npm audit, Prettier/Rust formatting, ESLint, strict TypeScript, Vitest,
Rust tests, Wasm build, and production Vite build. Deployment and live verification are separate
release actions and must not be implied by local success.

## Explicit follow-ups

1. ~~Measuring the Canvas renderer against the capacity tiers.~~ **Done in v0.12.4.** A complete
   browser frame at 6,144 entities is 18.2% of 60 Hz; rendering is 1,069 µs of that. Both canvases
   now resolve definitions through maps built once, so the linear `find` the previous note named
   is not something a later measurement has to answer. Stage C is unblocked; Stage B's per-hex
   work should still not run far ahead of a re-measure.
2. ~~Power v0.13.~~ **Shipped.** Poles compile a network; brownouts are integer; water stays
   belted. Next play milestone is Upgrades and Tiers v0.14.
3. New, from v0.12.2's measurement: the main-thread merge is 6.3% of the largest tier's host frame,
   against 0.7% when the boundary dominated. The code did not change and did not get slower;
   everything around it got faster. At 115 µs against a 100 µs clock step it needs a measurement
   that can resolve it before it needs an optimization.
4. Power, upgrades and tiers, multi-output recipes and byproducts, per-slot inventory
   rearrangement, equipment, inserters, splitters, lanes, fluid networks, trains, enemies,
   multiplayer, mod scripting, and evolutionary systems remain beyond this milestone. The material,
   power, and tier arc is in `docs/HEXFACTORY-PLAN.md`.
5. Deliberately not in v0.12: intermittent generation, accumulators, and a day cycle (they belong
   with power); terraforming a cliff into buildable ground; unloading a composer, which is still the
   mid-recipe-state question `set_recipe` sidesteps by refusing rather than answering; and
   `outputs: Vec<Ingredient>`, which arrives with the byproduct economy that needs it rather than as
   a format change with no consumer.
