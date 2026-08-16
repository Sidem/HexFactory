# Worker Boundary v0.5 scope and acceptance

Status: Worker Boundary v0.5 is shipped on the Command Surface v0.4 experience. It moves the native
simulation off the main thread and adds revision-checked dirty snapshot transport without changing
world, transport, save, checksum, or player-facing progression contracts. Continuous Exploration
v0.3 and Playable Game v0.2 remain the simulation and historical baselines.

## Worker and delta contract

- Only the dedicated module worker imports and instantiates `factory_wasm`. The main thread sends
  ordered RPC requests and keeps a cached presentation snapshot.
- A frame combines at most one bounded command batch and a bounded native tick count into one worker
  advance. Rust remains the only running tick and state owner.
- The initial snapshot is complete. Later native deltas always carry base revision, next revision,
  tick, and checksum, and omit unchanged snapshot groups. The host rejects revision gaps before
  applying presentation patches.
- Reset, new game, load, save, and placement legality all cross the same worker boundary. Placement
  previews are coalesced so pointer movement cannot create an unbounded request queue.
- `HXF1` version 2 and deterministic checksums are unchanged. Save strings remain opaque to the
  browser host.

## Player-facing command surface

- The continuous world fills the available viewport. A compact command bar keeps the landing
  directive, progress, insight, pause state, and game menu visible without pushing play below the
  fold.
- Snapshot-derived guidance names the next useful action across gathering, delivery, research,
  automation, composition, victory, and the Factory demo. It is explanatory host presentation; it
  does not mutate or reconstruct native state.
- Inventory and exact carried quantities sit with Gather and Deliver. Research shows prerequisite,
  affordability, and completion states. The bottom construction dock keeps inspect/edit/build,
  locks, exact costs, and orientation in one spatial workflow.
- Resource labels expose identity and remaining quantity; machines expose definition identity,
  direction, progress, inventory, and snapshot-backed cargo animation in the world.
- At 390 px the map remains the primary surface. Mission, research, and session controls become
  dismissible overlays, while a held four-direction touch pad emits the same bounded movement
  intents as WASD and direct Gather/Deliver actions remain available.

## Playable loop

The loop remains intentionally compact:

`explore freely → identify/gather finite resources → deliver for insight → research → construct nearby → automate → win`

The player carries exact native inventory quantities. Field Logistics unlocks belts, Automated
Extraction unlocks extractors, Composition unlocks the composer, and Storage Planning unlocks
containers. Construction spends that inventory atomically. Extractors consume finite continuous
resource regions, transport runs on compiled edges, and delivering three components sets persistent
native victory while leaving free play enabled.

## World and interaction contract

- Player and environment positions are integer fixed-point `x/y` owned by Rust. The host sends only
  bounded held-key intent from `W/A/S/D`; each native tick owns movement, facing, sliding collision,
  continuous chunk generation, and checksums.
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

`HXF1` save version 2 and world-generator version 2 serialize continuous player/feature truth,
footprints, inventories, research, machines, cargo, objective, and checksum. The loader validates
versions, references, feature uniqueness/radii, entity IDs, footprint overlap, input bounds, and
checksum. Held movement intent is neutralized after a successful restore. v0.2 saves are rejected
with an explicit incompatible-version error; browser storage treats the native string as opaque.

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

The local release gate is npm audit, Prettier/Rust formatting, ESLint, strict TypeScript, Vitest,
Rust tests, Wasm build, and production Vite build. Deployment and live verification are separate
release actions and must not be implied by local success.

## Explicit follow-ups

1. Closed by Capacity Tiers v0.5.1. Measured tiers are recorded in `docs/BENCHMARKS.md` and now
   order the remaining native work: resolve extractor deposits instead of rescanning every tile per
   tick, then make the buildings delta per-entity rather than per-group. A renderer decision and any
   scale claim still wait on those, and on a browser-side measurement.
2. Richer biomes/resource identification, inventory capacity/equipment, footprint-aware demolition
   previews, inserters, splitters, lanes, power, fluids, circuits, trains, enemies, multiplayer, mod
   scripting, and evolutionary systems remain beyond this deliberately basic milestone.
