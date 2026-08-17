# HexFactory — architecture, roadmap, and implementation handoffs

Status: Browser Capacity v0.8 is shipped on Sparse Snapshot v0.7, Sparse Cost v0.6, Capacity Tiers
v0.5.1, Worker Boundary v0.5, Command Surface v0.4, Continuous Exploration v0.3, and the v0.3.1
incremental transport follow-up. The capacity ladder now runs in the browser worker as well as
natively, so `docs/BENCHMARKS.md` finally measures the artifact that ships instead of a proxy for
it, and that measurement — not intuition — orders what comes next. It settled the open question:
the wasm engine costs about 1.2× native, and the worker boundary costs roughly 60% of what a frame
costs the host, tracking payload bytes at about 10 µs/KB. The next milestone is therefore the
compact binary delta encoding over a transferable buffer, not a further native optimization. A
renderer decision stays gated behind a renderer measurement, which is now the second follow-up.

Read that as the engine's internal ordering. The project goal was restated on 2026-08-17: HexFactory
is a game first, and the architecture is the means that makes the game possible at scale. See
**Product decision** below for the pillars that now govern milestone selection — engineering work is
chosen for the play it unlocks, and a milestone that improves nothing the player can feel needs a
reason to come before one that does. **Game Feel v0.9** and **Playability v0.10** both shipped under
that ordering. Next is the four-milestone arc in **Roadmap after v0.10**: World Shape v0.11,
Material Base v0.12, Power v0.13, and Upgrades and Tiers v0.14. The binary delta encoding stays the
next engine milestone and should land between v0.12 and v0.13, before those milestones grow the
snapshot it compacts; the renderer measurement gates animation; and the drag's per-cell transport
recompile is unblocked and can land anywhere.

Target repository: `https://github.com/Sidem/HexFactory`

Target live MVP: `https://sidem.github.io/HexFactory/`

Project root: `X:\Programming\Projects\HexFactory`

Published geometry dependency: `@hexlife/embed@1.15.0` (exact pin).

Local source/reference checkout for that npm package: `X:\Programming\Projects\HexLife`. HexLife is
not a source dependency: HexFactory imports only the published package. Treat that checkout as
read-only unless a future task explicitly authorizes a separately released generic package change.

## Shipped implementation record

- Playability v0.10 is the milestone playtesting asked for, and it is the first to change what the
  player may carry. Placement stopped asking the same question two ways: a deposit was tested by
  whether a hex centre fell inside it and an obstacle by whether two circles touched at all, which
  against a 1774-unit hex step made a deposit between two hex centres unminable while a rock between
  two hex centres blocked both. Both now use `placement_overlap` at two tuned interpenetration
  depths — zero for a deposit, so the smallest generated deposit stays reachable from some hex
  against the lattice's 1024-unit covering radius, and 400 for an obstacle, so a rock that grazes a
  hex no longer makes it unbuildable. `deposit_candidates` and `resource_at_world` share that one
  predicate, so a resolved extractor reference cannot drift from the placement rule.
  Research clicks are no longer dropped: `renderTechnologies` rebuilt every button on every snapshot
  update, so a rebuild landing between pointer-down and pointer-up destroyed the pressed button and
  the delegated `click` resolved to nothing. Every host list carrying a control is now patched in
  place through one reconciler, and the hotbar stopped rewriting its buttons' inner nodes for the
  same reason. Verified in a real browser: the pressed control survives a re-render and the
  delegated handler still finds it.
  The player walks on its own cadence. `advance_player` left the simulation tick, `advance` takes a
  player-step count beside the tick count, and the host derives that count from elapsed real time
  and a rate native publishes — so walking is unaffected by pause and by the speed multiplier while
  staying integer, native, and deterministic. Frame-coupled movement stayed refused; the host sends
  a count, never a position.
  Carrying capacity arrived as a rule over the existing inventory rather than as a stored slot
  array: `ceil(quantity / stack_size)` slots against a scenario slot count, so the save format, the
  checksum inputs, and every ordering guarantee are untouched, and the slot grid is presentation
  over stacks native resolves. The three paths that add to the player each answer for themselves —
  gathering into a full pack is refused, a withdrawal moves what fits, and an erase whose refund
  would not fit is refused whole. That last one was the open gameplay decision, and refusal is the
  only one of the three candidates that keeps conservation exact and leaves the recovery available
  once there is room; the removal preview reports it, so a drag cannot promise a recovery it will
  refuse. `withdraw` joins `place` and `erase` as a bounded, range-checked command, with the
  requested quantity as a ceiling rather than a demand. The research panel now states what each
  technology unlocks, what it costs, and which of the two reasons makes it unavailable.
  Save version 3 and definition version 4 reject v0.9 saves, which is correct: a pack that could not
  hold what a v0.9 save recorded is not the same game. The capacity ladder reproduces its v0.8
  checksums — the workload's player carries nothing and never walks — so the recorded ladder still
  compares directly.

- Game Feel v0.9 is the first milestone chosen by the restated game-first goal rather than by the
  capacity ladder, and it attacks the friction between intent and result. A belt run is now one
  drag: `place_line` and `erase_line` carry two endpoints as a single bounded command, and Rust
  resolves the path, the per-cell heading, the legality, and the cost. The resolver takes the
  lowest-numbered direction that closes the distance, so a run uses at most two directions and turns
  exactly once — the fewest turns a line between those endpoints can have. Illegal cells are skipped
  rather than aborting the run, and a run that stops short says why. The drag preview comes from the
  same resolver and spends materials against a copy of the inventory as it walks, so it marks the
  exact cell a run stops at; a host test pins that neither `main.ts` nor the renderer contains a
  line traversal of its own. Undo takes back the last construction through the ordinary erase path,
  from a stack that is derived session state and therefore never saved, hashed, or checksummed.
  Rotation became one idea instead of two, `Q` copies what is under the cursor, the hotbar grows to
  nine, `F` repeats while held on the native cooldown, and movement stops on the frame the key comes
  up rather than 110 ms later. It changes no simulation, save, determinism, or dependency contract:
  the placement, erase, refund, and recompile paths are the tested ones, reached from a new entry
  point. Its own follow-up is named and left to the engine track — a drag recompiles the transport
  graph once per cell, which is a one-off hitch at release of the pointer and a real optimization
  worth measuring rather than assuming.

- Browser Capacity v0.8 closes the follow-up every record since v0.5.1 has deferred: the ladder is
  measured in the browser worker, so a wasm capacity record now sits beside the native one. The
  measurement itself stays in Rust and is shared by both platforms — only the clock differs, a
  native `Instant` or `performance.now` — so the two records are comparable by construction rather
  than by re-implementation, and every browser tier reproduces its native checksum and delivered
  total. The harness compiles into wasm only under a `bench` cargo feature and is driven by a
  dev-only `/bench.html` page, so the deployed game artifact is unchanged at 464 KB and the
  production build does not include the page. Because a browser clamps `performance.now` to 100 µs
  unless cross-origin isolated, each phase repeats its sample block until it has run 20 ms; only
  the sample count changes, and each tier's checksum is taken from a separate core advanced exactly
  once through its tick budget so extra samples cannot move it. The harness also measures what no
  native run can see: the worker RPC round trip and the main-thread delta merge. It changes no
  simulation, save, determinism, or dependency contract, and the native ladder reproduces every
  v0.7 checksum and timing. Its findings reorder the roadmap. Wasm costs 1.19–1.23× native at the
  four largest tiers, so three releases of native work transferred intact and the engine is not the
  problem. The worker boundary is 57–61% of a host frame and scales with payload at about
  10 µs/KB — 6,085 µs of the largest tier's 10,345 µs frame — which prices the 644 KB JSON delta
  v0.7 named and makes a compact binary encoding over a transferable buffer the next milestone. The
  per-entity merge from v0.6 costs 0.7–1.5% of a frame and needs no work. The largest measured tier
  now uses 62.1% of a 60 Hz frame rather than the native record's 23.1%, with rendering still
  unmeasured, so the ceiling is above 6,144 entities but not far above it.

- Sparse Snapshot v0.7 closes the follow-up v0.6 named for itself: the frame no longer materializes
  a complete snapshot purely to diff it. The core marks dirty entities, deposits, terrain, and the
  chunk set where state is mutated, and the delta is built from those marks against a baseline of
  what the host was last sent, so only entries that may have moved are materialized at all. Two
  quadratic scans inside the complete snapshot are also gone — extractor status now resolves through
  the cached deposit reference the tick path already used, and per-chunk entity counts come from one
  pass over the blueprint — which makes building a full snapshot linear in entity count for the
  first-frame, reset, new-game, and load paths that still do it. Resources join buildings as a
  keyed patch on the wire. It changes no simulation, save, determinism, or dependency contract:
  every capacity tier reproduces its v0.6 checksum and delivered total, so the records compare
  directly. The frame cost falls 16.8× at the largest measured tier and the complete snapshot 26.8×,
  the delta payload is unchanged by design, and every tier in the recorded ladder now fits inside a
  60 Hz frame — which means the ladder no longer locates a native ceiling, only headroom above
  6,144 entities. Its two findings order what follows: the frame's remaining two-thirds is JSON
  serialization of a payload reaching 644 KB, and the whole-world checksum is now the largest single
  identified cost at 27–37% of a frame. Neither is worth attacking before the browser measurement
  that has been deferred since v0.5.1.

- Sparse Cost v0.6 closes both measured follow-ups and makes unexplored world visible. Extractors
  resolve a cached deposit reference instead of scanning every generated tile per tick, which makes
  tick cost linear in entity count and 233× cheaper at the largest measured tier. The buildings
  delta becomes per-entity — changed and removed entities in stable id order, merged by one linear
  host pass — cutting delta payload 2.3× at every tier. Neither change touches simulation results:
  every capacity tier reproduces its v0.5.1 checksum and delivered total, so the two records compare
  directly. It also adds a fog of war derived from native chunk bounds: a hatched veil with a dashed
  survey frontier over world the simulation has not generated, an unsurveyed-selection readout, and
  a surveyed-sector count. The re-measurement moves the 60 Hz native ceiling from between 1,536 and
  3,072 entities to between 3,072 and 6,144, and names its own successor — a complete snapshot is
  still materialized every frame only to be diffed, which is now 55–91% of the frame.

- Capacity Tiers v0.5.1 adds a deterministic headless capacity ladder to the native crate and
  records the first measured tiers. Six steady-state tiers from 12 to 6,144 buildings are timed for
  tick, snapshot, worker frame, delta payload, full compile, incremental recompile, and public edit
  cost. The harness is excluded from the wasm target and from the CI gate; the test gate instead
  pins the workload checksum so recorded numbers cannot silently stop being comparable. It changes
  no simulation, save, determinism, or dependency contract. Its three findings — extractor deposit
  lookup dominating tick cost, group-level deltas resending the whole buildings array, and
  incremental recompilation costing about three times a full compile — replace the previous
  unmeasured ordering of follow-up work.

- Worker Boundary v0.5 moves the Wasm `Factory` into a dedicated module worker with serialized RPC,
  combines each frame's bounded commands and native ticks into one advance, and transports
  revision-checked native snapshot deltas. Rust omits unchanged snapshot groups; the host caches only
  presentation state and rejects revision gaps. Placement previews are coalesced, and native save,
  load, scenario, determinism, and checksum contracts are unchanged.

- Command Surface v0.4 makes the world a full-viewport play surface with a persistent landing
  directive, snapshot-derived next-action guidance, compact cargo and research surfaces, a
  lock/cost-aware construction dock, clearer world labels/cargo, an intentional session menu, and
  narrow-layout touch movement plus direct field actions. It changes presentation only; native
  simulation, save, determinism, and dependency contracts are unchanged.

- Transport Graph v0.3.1 replaces full post-edit graph rebuilds with stable-ID invalidation and
  affected weak-component recompilation. Tests pin full-rebuild equivalence, unrelated-component
  isolation, component splits, and component merges. Initialization and save restoration retain a
  full deterministic compile.
- Continuous Exploration v0.3 replaces hex-step movement with native two-axis intent, continuous
  collision and gathering, proximity-limited construction, definition-driven rotated footprints,
  and a construction-only/toggled grid. Its HXF1 save and generator versions are intentionally
  incompatible with v0.2. The exact public geometry dependency remains unchanged.

## Shipped milestone — Playability v0.10

Sourced from playtesting on 2026-08-17, not from the capacity ladder. Two of these were defects with
arithmetic behind them, one collided with a determinism invariant and needed the resolution recorded
below, and the rest were the systems the game was missing. All six shipped; the diagnoses are kept
here because they are what the code now has to keep being true of.

### 1. Placement geometry — one bug with opposite signs

Both complaints come from `placement_legality` using two different tests for the same question:

| Check                      | Test                                 | Effective distance     |
| -------------------------- | ------------------------------------ | ---------------------- |
| Deposit under an extractor | point-in-circle, `resource_at_world` | hex centre within 720  |
| Rock or water blocking     | sum of radii, `circles_overlap`      | centres within 690+660 |

Hex spacing is 1774 world units. A deposit of radius ~720 is therefore narrower than a single hex
step, so a deposit sitting between two hex centres can host no extractor at all — while an obstacle
blocks anything within 1350 and one rock between two hexes blocks both. Adopt one overlap rule for
both and tune the thresholds by feel: an extractor should be legal when its hex meaningfully covers
the deposit, and an obstacle should block only when it meaningfully intrudes, not when it grazes.

`deposit_candidates` deliberately mirrors `resource_at_world`, and
`resolved_deposit_references_match_a_full_tile_scan_and_survive_generation` pins the two equal. They
move together, or the cached extractor reference silently stops matching the placement rule.

### 2. Research clicks that go nowhere

`renderTechnologies` calls `replaceChildren` and rebuilds every button on every snapshot update —
about once a second at speed 1 and more above it. The click listener is delegated on the container,
so a rebuild landing between pointer-down and pointer-up destroys the pressed button, the browser
retargets `click` to the container, `closest("button[data-technology-id]")` returns null, and the
research is silently dropped. Patch the buttons in place instead of recreating them, and treat the
same pattern in `renderHotbar` and `renderInventory` as suspect. This is a diagnosis from the code,
not a reproduction: the hidden-pane rAF block that stops the frame loop also stops the re-render, so
confirm it with a real click before and after.

### 3. Player movement on its own cadence

`advance_player` runs inside the simulation tick, so the player stops when the factory pauses and
walks at a quarter speed at 0.25× — which is the actual complaint. The literal fix, driving movement
from the render loop, is not available: player position is a checksum input and browser frame rate
may not change a deterministic result.

**Resolution:** give the player a fixed native cadence of its own that always advances at one rate,
independent of pause state and of the speed multiplier. Movement stays integer, native, and
deterministic; it stops being a slave to factory time. That satisfies walking while paused and
walking at full speed at any sim speed. Frame-coupled movement stays refused, and any proposal for
it has to price what it does to saves and checksums first.

### 4. A carrying inventory with stacks

Decided by playtest: the player gets a slot grid with per-stack limits, so carrying capacity becomes
a real constraint and containers exist to solve it.

**Recommended model:** keep `item_id → quantity` as the truth and express capacity as a _rule over_
it — each item occupies `ceil(quantity / stack_size)` slots, and a fixed slot count is enforced on
every path that adds to the inventory. This gives real carrying pressure and a grid UI without
changing the save format, the checksum inputs, or the ordering guarantees, and without a slot array
that has to be serialized and validated. Only adopt real per-slot state if players must rearrange slots
by hand, which is a much larger change and is not what was asked for.

Every path that _adds_ to the inventory now needs a full-inventory answer, and these are gameplay
decisions, not implementation details: gathering when full, withdrawing when full, and — the one
that is easy to miss — `erase`, which today refunds construction cost plus the building's entire
contents straight into the player. Refusing the erase, partially refunding, or spilling to the
ground are all defensible; pick one, state it, and test it.

**Decided and shipped:** gathering into a full pack is refused; a withdrawal moves what fits and
leaves the rest in the container; an erase whose full refund will not fit is refused whole. Refusal
is the only one of the three erase candidates that keeps item conservation exact and leaves the
recovery available once the player has made room, and it keeps the refund policy exactly 100% rather
than turning it into "as much as fits". The removal preview reports it, so a drag cannot promise a
recovery it will refuse. Slot sizes shipped as ore 20, crystal 10, component 10, against 6 carried
slots in the new game and 10 in the factory demo.

### 5. Withdraw from containers

A new bounded native command beside `place` and `erase`, range-checked the same way, moving a
requested quantity from a container's inventory into the player's under the capacity rule above.
Straightforward once 4 has answered what happens when the player is full.

### 6. Research that explains itself

The separate half of the research complaint. The tree does not communicate what a technology
unlocks, what it costs, or why it is unavailable, and the panel is disconnected from the buildings
it gates. Design work, not a bug fix.

### Deliberately not in v0.10

- Slots are a rule over `item_id → quantity`, not real per-slot state. Rearranging slots by hand
  would mean a serialized, validated slot array and a new ordering guarantee, which is a much larger
  change than anything playtesting asked for.
- Withdrawal is by hand from containers only, and is not an inserter. Moving items between buildings
  automatically is transport, and transport is the belt's job until a milestone says otherwise.
- Composers cannot be unloaded. A composer's reserved inputs and progress are mid-recipe state, and
  taking from them means deciding what happens to a part-finished job — a question worth its own
  pass, not an aside in this one.
- The action cooldown still runs on simulation time. Only movement moved to the player's cadence,
  because only movement was the complaint; a paused factory therefore still stops repeat gathering,
  which is the defensible reading of a paused world.

### Deferred past v0.10 — upgrades and tiers

Larger containers and upgraded buildings are the largest item raised and deserve their own
milestone: tiered definitions, an upgrade command that preserves contents and connections, and the
progression to earn them. Keeping it out of v0.10 keeps v0.10 shippable. It was originally slotted
as v0.11; the roadmap below moves it to **v0.14**, because tiers need better materials to be built
from and a power budget to improve, and both arrive first.

## Roadmap after v0.10 — the world, its materials, and its power

Sourced from a design conversation on 2026-08-17. Four milestones with a real dependency chain: the
world has to produce more kinds of matter before recipes can combine them, recipes have to exist
before generators are worth building, and all three have to exist before tiers have anything to
spend or improve. Each entry states the play it unlocks, per the design pillars.

| Milestone                | Unlocks                                                    | Depends on      |
| ------------------------ | ---------------------------------------------------------- | --------------- |
| v0.11 World Shape        | A world worth walking across and choosing a site on        | v0.10 item 1    |
| v0.12 Material Base      | A production tree instead of one recipe                    | v0.11 fields    |
| v0.13 Power              | A second constraint that reshapes layout                   | v0.12 materials |
| v0.14 Upgrades and Tiers | Growth in place; extraction radius as the flagship upgrade | v0.12 and v0.13 |

**Where the engine milestones slot.** The compact binary delta encoding should land no later than
between v0.12 and v0.13. Every milestone here grows the snapshot — more item IDs in more
inventories, terrain with more bands, then a power network with a per-entity satisfaction figure —
and `docs/BENCHMARKS.md` already priced the worker boundary at roughly 10 µs/KB. Growing the payload
before compacting it spends the measured headroom on the wrong thing. The renderer measurement is
the gate for animation (see **Art direction and sprites** below), so it wants to happen during
v0.12. The drag's per-cell transport recompile has no dependency here and can land anywhere.

### v0.11 — World Shape

#### What generation does today

`generated_tile` hashes each `(q, r)` independently and reads three unrelated moduli off it:
`hash % 31 == 0` is water, `hash % 23 == 0` is rock, `hash % 67 == 1` is an iron deposit,
`hash % 149 == 2` is a crystal deposit. Independent primes over independent hashes cannot cluster —
the output is salt-and-pepper by construction, and no amount of tuning those constants produces a
lake or a ridge. A radius-7 circle around the landing site is cleared by `near_landing`.

Two lattices are also in play, and this matters for the rewrite. Feature circles are placed at
`q * FEATURE_SPACING` with ±512 jitter — a rectangular 2048 grid — while hexes are placed by
`axial_world` at `(q * 1774 + r * 887, r * 1536)`. Both are keyed by the same `(q, r)`, and they
coincide only near the origin. Generation driven by player position (`ensure_neighborhood`) uses the
feature lattice and is self-consistent; the scenario's `ensure_tile(placed.q, placed.r)` feeds axial
coordinates into it and gets away with it only because the prebuilt factory sits near the origin.
The guaranteed scenario tiles overwrite `x, y` with `axial_world`, which is why the demo start looks
aligned and the open world does not. Collapse these to one lattice as part of this milestone.

This is diagnosed from reading `factory-wasm/src/lib.rs`, not from a reproduction.

#### Resource fields

Replace point deposits with continuous fields. A field is a deterministic function of seed and world
position returning `(item_id, richness)`, sampled per hex cell. Cells with richness above a
threshold hold extractable quantity; everything else is barren and costs nothing to store, which is
the existing sparsity invariant applied to terrain rather than to entities.

Depletion is the only mutable part. Keep the existing tile map as a **sparse depletion overlay**:
generation yields the initial quantity as a pure function, and only cells an extractor has actually
drawn from get a stored remainder. Unmined field area stays free. The overlay is real state — it is
saved, hashed, and checksummed — while the generated field underneath it is derived and must not be.
That split is the same rule the resolved deposit references already follow.

#### Extraction radius

An extractor harvests every field cell within radius R of itself, draining them in a stable order
(distance, then cell key — the ordering `deposit_candidates` already establishes). Yield per cycle
falls as the nearby cells empty, so an extractor slowly starves in place instead of stopping dead,
and the player feels the field thin out. Base R in v0.11 is fixed; **v0.14 makes R the flagship
upgrade**, which is the most legible possible demonstration of what an upgrade is for.

Two consequences to design deliberately: overlapping extractors compete for the same cells and must
resolve by stable entity ID like every other arbitration, and a large R means one placement decision
covers many cells, so the cost of a wrong site should be real but not punishing.

#### Natural terrain — basins, hills, and cliffs

Layer two integer noise fields, elevation and moisture, and read terrain out of bands rather than
out of moduli. Value noise with integer interpolation keeps it deterministic and keeps sampling
pure, so a tile still needs no neighbors outside its chunk.

- **Deep water / shallow water** below the sea-level band, which produces basins and lakes with
  actual shorelines instead of scattered puddles.
- **Shore** — the transition band, and the natural home for sand and clay.
- **Lowland** — buildable, the default.
- **Highland / hills** — buildable, gates wind generation later.
- **Cliff** — where the elevation gradient between adjacent cells is steep. Impassable and
  unbuildable until mined. Deriving cliffs from the gradient rather than from a band is what makes
  them read as edges of a landform rather than as another kind of rock.

Correlate the resource fields with the terrain so that geography is information: iron and coal in
highlands, copper in hills, stone at cliffs, sand and clay along shores, wood in moist lowlands,
water in basins. This is what makes the fog frontier worth pushing and gives the surveyed world
something to say.

#### Hex scale relative to the player

Two independent knobs, currently conflated:

- **`PLAYER_RADIUS` (360) against `HEX_X` (1774)** is the only thing that sets how large the player
  is relative to a hex. The player currently spans about 41% of one hex step. Raising
  `PLAYER_RADIUS` toward 540–620 (with `PLAYER_SPEED` raised proportionally so the walk keeps its
  feel) makes the grid read smaller against the player without touching the world lattice.
- **`BASE_HEX_SIZE` (31 px)** sets how many hexes fit on screen. It is pure presentation and free to
  change.

The renderer does not currently derive the drawn player from `PLAYER_RADIUS` at all — `drawPlayer`
hardcodes `size * 0.3` and `size * 0.48` against the pixel hex size, so the drawn body and the
collision circle are only coincidentally similar and would visibly desync the moment the ratio
changes. Publish `PLAYER_RADIUS` in the snapshot (or pin it in `fixtures/`) and derive the drawing
from it, then change the ratio once, natively. Do the derivation first; it is a correctness fix that
happens to be the prerequisite.

If the hex constants themselves should change, this is the milestone to do it in: v0.11 bumps
`WORLD_GENERATOR_VERSION` regardless, so a lattice change rides along at no extra compatibility cost.

#### Cost and compatibility

This milestone invalidates every existing save. `WORLD_GENERATOR_VERSION` goes to 3 and `load` will
reject version-2 envelopes, which is the behavior already in place and is correct — a save whose
world regenerates differently is not the same world. Say so in the release notes rather than
attempting a migration; there is no honest migration from salt-and-pepper to fields.

`resolved_deposit_references_match_a_full_tile_scan_and_survive_generation` has to be rewritten
against fields rather than merely updated, and the v0.10 placement-legality fix has to be re-tuned
here. That argues for v0.10 item 1 fixing the **inconsistency** — one overlap rule for both tests —
and deliberately not over-investing in threshold tuning that this milestone will redo.

### v0.12 — Material Base

Eight raw resources and a first processing tier. The point is not quantity; it is that a material
should arrive from somewhere specific and become something the player wanted.

#### Raw resources

| Resource   | Source                  | Terrain              |
| ---------- | ----------------------- | -------------------- |
| Iron ore   | field                   | highland             |
| Copper ore | field                   | hills                |
| Coal       | field                   | highland, near rock  |
| Stone      | field, and mined cliffs | cliffs, rock         |
| Sand       | field                   | shore, dry basin     |
| Clay       | field                   | shore, moist lowland |
| Wood       | flora, regrowing        | moist lowland        |
| Water      | pumped                  | basins               |

Regrowing flora is the one genuinely new source behavior: a harvested cell refills on an integer
cadence, which makes wood renewable while ore is finite and gives the two categories different
strategic weight.

Biomatter and waste are deliberately **not** here. They arrive later with animals, where a living
population gives biomatter a source that behaves unlike a field and gives waste somewhere to go
besides a void. Pulling them forward would mean designing that economy twice.

#### First recipes

Tier 1, each a single machine, each cheap enough to build early:

| Output       | Recipe              | Machine |
| ------------ | ------------------- | ------- |
| Iron plate   | 2 iron ore + fuel   | Smelter |
| Copper plate | 2 copper ore + fuel | Smelter |
| Glass        | 2 sand + fuel       | Smelter |
| Brick ×3     | 2 clay + fuel       | Kiln    |
| Charcoal     | 2 wood              | Kiln    |
| Timber ×2    | 1 wood              | Cutter  |
| Gravel ×2    | 1 stone             | Crusher |

Tier 2, the first recipes that combine across sources:

| Output         | Recipe                      |
| -------------- | --------------------------- |
| Copper wire ×2 | 1 copper plate              |
| Gear           | 2 iron plate                |
| Frame          | 2 timber + 1 iron plate     |
| Concrete ×2    | 2 gravel + 1 sand + 1 water |
| Circuit        | 1 glass + 3 copper wire     |
| Steel          | 2 iron plate + 2 coal       |

Charcoal is deliberately reachable without coal, so a player who lands away from a coal field can
still bootstrap smelting from trees. Concrete is the first recipe that needs water, which is what
makes basins worth building near rather than merely worth looking at.

#### What the engine actually needs

Most of this is data, which is the point of "definitions, not callbacks". Two real changes:

1. **Fuel as an item property, not a recipe input.** Give `ItemDefinition` an optional `fuel_value`
   and `BuildingDefinition` an optional fuel slot. Then a smelter recipe never names coal, and coal
   and charcoal are interchangeable at different values — as is every fuel added later. Putting fuel
   in `inputs` would force one recipe per fuel and hardcode the bootstrap path.
2. **Machine categories.** Smelter, kiln, cutter, and crusher are all the existing `Composer` kind
   with different recipes — no new `BuildingKind` for any of them. What is missing is a category tag
   on recipes and buildings so a kiln cannot run a circuit recipe. One field, one check at recipe
   assignment.

`BuildingKind` gains only **Pump** in this milestone (draws from water terrain rather than from a
deposit, so it is genuinely not an extractor).

**Multi-output recipes are not needed here.** `RecipeDefinition.output` is a single `Ingredient`,
and with byproducts deferred alongside waste, nothing in this tree produces two different items.
Quantities above one (`Brick ×3`, `Timber ×2`) are already covered by `Ingredient.quantity`.
`outputs: Vec<Ingredient>` arrives with the byproduct economy that needs it — a definition-format
version bump plus the composer's output path, with outputs emitting in declared order. Adding it
early would be a format change with no consumer.

Note also that the shipped `component` recipe's description names a crystal its `inputs` never
list — worth reconciling while the definitions are open.

### v0.13 — Power

Electricity is the second constraint. Transport is about where things go; power is about what a
region can afford to run, and it reshapes layout in a way nothing else in the game currently does.

#### The network model

A third compiled representation beside the transport graph, exactly as the **Long-term model**
section anticipates. Poles connect; connected components compile into networks; each network holds
integer supply and demand per tick. Consumers declare a draw, generators declare an output, and both
recompile on edit like the transport graph does.

**Determinism rule:** no floats, and no dependence on iteration order. Compute
`satisfied = min(supply, demand)` per network, then advance each consumer's progress by
`base * satisfied / demand` in integer arithmetic, accumulating the per-entity remainder so total
work is exact and brownouts slow machines smoothly rather than stalling an arbitrary subset. Where a
tie must be broken, break it by stable entity ID like every other arbitration.

#### Generation

| Source           | Input         | Terrain gate     | Role                |
| ---------------- | ------------- | ---------------- | ------------------- |
| Burner generator | any fuel item | none             | Bootstrap           |
| Boiler + turbine | water + fuel  | near water       | Mid-game workhorse  |
| Wind turbine     | none          | highland / hills | Fuel-free, sited    |
| Hydro            | none          | basin edge       | Scarce, high output |

The boiler-and-turbine pair is deliberately two buildings: it is the first thing the player builds
that is a _system_ rather than a machine. Wind and hydro are where v0.11's terrain pays off — a good
power site becomes a reason to have explored. Keep wind at a fixed output for this milestone;
intermittency has to be a deterministic function of tick and position, never a runtime roll, and
that is a design problem worth its own pass. Solar needs a day cycle and is deferred with it.

#### Water: an item first, a fluid network later

Water is wanted by concrete and by boilers, and the tempting move is a fluid network. Do not build
two network models in one milestone. Have the pump output a water item that rides ordinary belts;
basins become worth building near immediately, and the fluid network can arrive later as a genuine
improvement rather than as scope that sank the milestone. Say plainly in the notes that belted water
is an interim model.

#### Accumulators

Deferred. They are the natural answer to intermittent generation, and intermittent generation is
itself deferred; they arrive together.

### v0.14 — Upgrades and Tiers

The originally-deferred milestone, now with something to spend and something to improve. Tiered
building definitions, an upgrade command that preserves contents, orientation, and connections, and
the progression that earns them. **Extraction radius is the flagship upgrade** — it is visible on
the map, it changes a decision the player already made, and it demonstrates what tiers are for
better than a bigger box does. Larger containers, faster smelters, and more efficient generators
follow the same pattern.

### Deferred beyond this arc

Named here so they are decisions rather than omissions, each with the thing it is waiting for:

- **Animals, biomatter, and waste.** One milestone, not three. A living population is what gives
  biomatter a source that behaves unlike a field — it grows, it can be depleted past recovery, it
  moves — and it is what gives waste somewhere to go besides a void. Designing the byproduct economy
  before its consumer exists would mean designing it twice. This is also what brings
  `outputs: Vec<Ingredient>` into `RecipeDefinition`.
- **Fluid networks.** Water ships as a belted item in v0.13; the real network is an improvement on a
  working game rather than a second network model built in the same milestone as the first.
- **Intermittent generation and accumulators.** They arrive together. Intermittency has to be a
  deterministic function of tick and position, never a runtime roll, and that is its own design
  pass.
- **A day cycle, and solar with it.** A day cycle is a presentation and simulation change at once
  and should be chosen for what it does to the game's feel, not smuggled in as a power source.
- **Terraforming.** Cliffs are unbuildable until mined in v0.11; whether the player may reshape
  elevation, and what that costs, is a question the world has to exist before anyone can answer.

### Art direction and sprites — when

Three stages, gated on what would otherwise be redrawn or unaffordable.

**Stage A — art direction, during v0.11.** No engine change. The terrain bands need a palette
before they can be drawn at all, so the direction pass is not optional work that happens to be
early; it is a v0.11 dependency. Deliverables: palette for the elevation and moisture bands, shape
language for buildings, the rule for how a sprite fits a hex cell, and one still mockup of a running
factory to argue about. Also define the item icon system here and apply it to the current three
items — v0.10's inventory grid will be displaying `"icon": "ORE"` string codes, and that is the
cheapest visible improvement available.

**Stage B — static sprite set, after v0.12.** The building and item roster is not stable until the
material base lands; drawing sprites before that guarantees redrawing them. Once it is stable, do
the full item icon set and static building sprites as an atlas, still on Canvas 2D.

**Stage C — animation, gated on the renderer measurement.** Belt motion, machine work cycles,
extractor pulses, and water shimmer are per-frame per-entity draws, and rendering is the half of the
frame `docs/BENCHMARKS.md` has never measured. AGENTS.md forbids claiming anything about it, so
animation stays behind the renderer measurement that is already on the follow-up list — measure
first, then decide whether the animated frame wants a different renderer, then animate. This is the
one place where the schedule is set by an invariant rather than by preference.

## Shipped milestone — Game Feel v0.9

v0.4 built a command surface that presents the game well. v0.9 is about what it feels like to
operate: the moment-to-moment loop of moving, aiming, placing, routing, and correcting. The
simulation is correct and fast enough that the honest limit on enjoyment is now ergonomic. Nothing
here changes what the game means — it changes how much friction sits between intent and result.

The measured engine follow-ups in `docs/BENCHMARKS.md` are not cancelled and not reordered among
themselves; they are deferred behind this one. The binary delta encoding remains the next _engine_
milestone and the renderer measurement still gates any renderer decision.

### The friction this milestone removes

- **Building a line costs one click per cell.** Placement is a single click handler, so a ten-hex
  belt run is ten clicks plus manual rotation. This is the largest single ergonomic gap against the
  games named as inspiration, where a run is one drag.
- **Routing is manual.** Orientation is chosen before placement with `R` and corrected afterwards
  with a separate rotate tool. The player is doing the pathfinding the compiled transport graph
  already understands.
- **Rotation has two mental models for one idea.** `R` rotates the pending building; changing an
  existing one requires selecting a different tool first.
- **The hotbar is capped at four.** Build selection is a hardcoded `Digit[1-4]` match, which the
  pillars' promise of depth outgrows immediately.
- **There is no way to say "one of those."** No pick-block or pipette to adopt the tool matching
  what is under the cursor.
- **Mistakes are expensive.** No undo; a misplacement costs resources and a manual erase.
- **Repetition is unrewarded.** Gathering is one keypress per action with no hold-to-repeat.

### Design direction

- Drag to build a run, drag to erase a run. The host sends bounded path endpoints; Rust resolves the
  path, the per-cell legality, the orientations, and the cost as one atomic operation. The host must
  not expand a drag into per-cell commands — that would both break the one-bounded-batch-per-frame
  rule and put routing truth in TypeScript.
- One rotation model: the same key rotates the pending building when a build tool is held and the
  hovered building when it is not.
- A hotbar that grows with the building set, with pick-block adopting whatever is under the cursor.
- Undo for construction actions, resolved natively so the refund policy stays the tested one.
- Held actions repeat on a native cadence rather than a host timer.
- Movement and camera should feel direct: revisit the 110 ms release coalescing on movement keys,
  which exists to debounce transitions but is felt on every stop.
- Feedback is part of the control, not decoration: a placement that is refused, a belt that is
  backed up, and a deposit that is running out should each be legible the instant they happen.

### Acceptance and release gate

- A belt run is built in one drag, correctly oriented, with the same result the equivalent per-cell
  placements would produce — pinned by `one_drag_builds_exactly_what_the_equivalent_placements_build`,
  which compares checksums rather than descriptions.

  This criterion originally asked for a run with _two_ turns. That was written before the path
  resolver existed and does not describe anything the feature can produce: `hex_line` takes the
  lowest-numbered direction that closes the distance, so a direction that stops helping never helps
  again and a drag uses at most two directions — exactly one turn. That is the better behaviour, not
  a shortfall, because it is the fewest turns a belt line between two endpoints can have. An S-shaped
  run is two drags. The gate is a one-turn run.

- What the drag preview promises is what the drag builds, including where a run stops for cost —
  pinned by `a_drag_preview_is_what_the_drag_builds`, which walks the preview and the placement
  through the same core.
- Every new control is reachable by keyboard, has an accessible name, respects reduced motion, and
  works on the narrow touch layout.
- Rust still owns every placement, orientation, path, cost, refund, and legality result. Forged host
  commands are rejected exactly as before. The host adds no per-cell loop and still sends at most
  one bounded batch per rendered frame.
- Determinism, save, checksum, and dependency contracts are unchanged, and the capacity ladder
  reproduces its v0.8 checksums.
- A player new to the game builds and routes a working line without documentation. This is a stated
  acceptance criterion, not a hope, and it is checked in a real browser on desktop and narrow
  layouts with a clean console.

### What shipped

- **Drag to build, drag to erase.** `place_line` and `erase_line` are single bounded commands
  carrying two endpoints. Rust resolves the path with `hex_line`, orients each belt at its
  successor, checks legality and cost per cell, and skips what it cannot use rather than aborting
  the run. A run that stops short reports why.
- **A preview that cannot lie.** `line_preview_json` and `erase_line_preview_json` return the cells,
  headings, and per-cell legality from the same resolver the command uses, spending materials
  against a copy of the inventory as it walks. The host draws that list and derives nothing; a host
  test pins that `main.ts` and the renderer contain no line traversal of their own.
- **Undo.** `Undo` takes back the most recent construction through the ordinary `erase` path, so the
  refund is the one the erase tests already pin. The stack is derived session state — never saved,
  hashed, or checksummed — so a loaded save has nothing to take back.
- **One rotation model.** `R` turns the pending building with a build tool held, and the building
  under the cursor without one.
- **Pick-block.** `Q` adopts the definition and orientation under the cursor as the active tool.
- **A hotbar that grows.** `Digit1`–`Digit9` instead of `Digit1`–`Digit4`, and `E` selects erase.
- **Held gather.** `F` repeats while held, paced by the native action cooldown rather than a host
  timer.
- **Movement that stops when the key does.** The 110 ms release coalescing is gone; a stop intent is
  sent on the frame the key comes up.

### Deliberately not in v0.9

- Undo covers construction, not erasure. Erase already refunds cost and contents, so an accidental
  removal is recovered by rebuilding; reversing one would mean restoring an entity's exact id,
  inventory, cargo, and progress, which is a larger change than this milestone justified.
- A drag places at most `MAX_LINE_CELLS` (32) cells and recompiles the transport graph once per
  cell, because each cell goes through the tested `place`. At the largest measured tier that is a
  one-off hitch on release of the pointer, not a per-frame cost. Batching the run into a single
  recompile is a real optimization and is left for the engine track, where it can be measured.
- Multi-hex buildings are not draggable; the host only starts a drag for single-cell definitions,
  which keeps the preview exact and matches how nobody wants a row of composers.

## Shipped milestone — Command Surface v0.4

The simulation is playable, but the v0.3 interface presents the architecture before it presents the
game: a large masthead pushes the world below the fold, primary progression competes with debug and
session controls, the research path is visually disconnected from its costs, and the narrow layout
has no practical movement surface. v0.4 is an interface and onboarding release, not a new simulation
contract.

### Experience principles

- The world owns the viewport. Brand, objective, inventory, research, editing, and session controls
  sit on a compact command surface over the map instead of forming a long document around it.
- At every progression state, one contextual next action explains both the goal and the mechanic:
  gather, deliver, research, automate, compose, or complete. It is derived from native snapshots and
  never invents progression truth.
- The landing directive and its progress remain visible at all times. Insight and carried materials
  are readable at a glance; checksum, seed, and single-step controls move into an intentional game
  menu.
- Construction is a spatial mode. A bottom dock groups inspect/edit/build actions, communicates
  locks and exact costs, keeps orientation adjacent to placement, and preserves full-footprint legal
  previews.
- Desktop retains direct panels and keyboard shortcuts. Narrow and coarse-pointer layouts preserve
  the full map, expose mission/research as dismissible overlays, and add a held touch movement pad
  that sends the same bounded native movement intents as the keyboard.
- World readability must distinguish resources, machine identity, direction, inventory, progress,
  and cargo without requiring the inspector. Animation remains presentation-only.

### Acceptance and release gate

- A new player can identify the first useful action, gather and deliver without opening help, see
  when research becomes affordable, find newly unlocked buildings, understand orientation before
  placement, and recover the camera after panning.
- The first desktop and 390 px narrow view show the playable world rather than a marketing header;
  narrow play supports movement, gathering, delivery, research, construction, and panel dismissal.
- Keyboard operation includes visible focus, WASD, gather/deliver, build number shortcuts, rotate,
  pause, Escape-to-inspect, and all controls retain accessible names. Reduced motion is preserved.
- Host logic may derive copy, classes, and interpolation only. Rust/Wasm continues to own every
  tick, coordinate, quantity, unlock, legality result, objective, save, and checksum.
- Completion requires the complete local quality gate, an intentional main-branch release, a
  successful Pages deployment, and live desktop/narrow interaction plus a clean console.

- Playable Game v0.2: HexFactory commit `b636dc2`, successful quality/Pages run `31951039927`.
- The live release was verified in a real browser through movement/collision, finite gathering,
  research, construction/editing, compiled factory operation, victory, exact save/continue checksum
  restoration, the retained Factory demo, a 390 px responsive layout, and a clean console.
- The playable release did not require a HexLife change: `@hexlife/embed/hex@1.15.0` remains the
  exact public geometry dependency.

- Generic prerequisite: `@hexlife/embed@1.15.0`, tag `embed-v1.15.0`, HexLife merge `37f3c63`.
- Factory repository: `https://github.com/Sidem/HexFactory` (`main` head `cf3d154`).
- Live MVP: `https://sidem.github.io/HexFactory/`, deployed by Actions run `31947910003`.
- The shipped slice keeps the approved boundary: `/hex` is the only HexLife dependency; factory
  simulation is an independent Rust/Wasm crate with compiled transport and native machine state.
- First follow-up: benchmarked capacity tiers before finer dirty tracking, a renderer change, or any
  scale claim.

## Product decision

HexFactory is a game first. The goal is a beautiful, open-ended factory-automation game that is fun
to play for its own sake, fascinating to keep exploring, and a pleasure to control — drawing on
Factorio's automation depth, Satisfactory's sense of place and scale, and Minecraft's freedom to
build what you want where you want, expressed in hexagonal space rather than square.

The deterministic Rust/Wasm core, the sparse architecture, the compiled transport graph, and the
narrow `@hexlife/embed` geometry dependency are the means to that end, not the end itself. They exist
because a large, living world that never stutters and never loses a save is a player experience
before it is an engineering result. Where an architectural preference and the player's experience
genuinely conflict, the player's experience wins and the architecture has to find another way to pay
for it.

That ordering weakens no invariant. Determinism, native ownership of the tick, sparse cost, and
measured-before-claimed all remain non-negotiable — they are what buys the game its scale, its
trustworthy saves, and the headroom for the world to keep growing. What changes is why they are
there, and therefore how milestones are chosen: engineering work earns its place by naming the
player-visible thing it enables.

The design intent is inspiration, never imitation. Original neutral shapes, names, and systems only;
this remains true of every commercial title named above, and the existing prohibition on asset or
branding imitation is unchanged.

### Design pillars

- **Fun is a requirement, not a polish pass.** A release that is correct, fast, and joyless has not
  met its acceptance criteria. Every milestone states what it makes better to play.
- **Controls must be obvious in the first minute and precise in the hundredth hour.** Movement,
  building, rotating, routing, and inspecting should be learnable without documentation and should
  stay pleasant under heavy repetition. A control that needs explaining is a defect in the control.
- **The player should always know what just happened and what to try next.** Feedback for gathering,
  placement, blockage, depletion, research, and delivery is part of the mechanic, not decoration.
- **The world should reward looking at it.** Readability first — resources, machine identity,
  direction, throughput, and blockage legible at a glance — and beauty close behind it. The fog
  frontier, the surveyed world, and a running factory should all be things a player wants to watch.
- **Open-ended, not scripted.** Progression opens options rather than prescribing a route. The world
  is unbounded and the player decides what to build, where, and how large. Victory is a milestone in
  a longer game, never a wall.
- **Nothing may stutter.** Frame stability, instant response to input, and saves that always restore
  exactly are player-experience features. This is what the measured capacity ladder is protecting.

HexLife is the engineering reference and `@hexlife/embed` is a narrow public dependency, but neither
is the factory simulation kernel. Reuse its successful patterns: Rust/Wasm hot paths, workers,
integer determinism, explicit snapshots/checksums, batched boundary crossings, dirty rendering, hex
geometry experience, reproducible builds, and isolated artifacts. Do **not** extend `WorldK`, encode
factories as CA state combinations, or make HexFactory depend on HexLife source files. A factory is
not a uniform local cellular rule.

The spatial map is a construction and rendering surface. The running simulation compiles placed
tiles into transport networks and sparse scheduled entities. Runtime work should follow active
cargo, due machines, and network changes—not every cell in the map.

## Shipped `@hexlife/embed` dependency contract

HexFactory must consume a real reusable npm surface, not add `@hexlife/embed` as a ceremonial
dependency. The founding prerequisite added the suitable unbounded 2D hex geometry without changing
the fixed row-major binary renderer or finite/toroidal CA engines:

**`@hexlife/embed/hex` provides** a DOM-free, Wasm-free, server-safe entrypoint for unbounded
pointy-top axial hex geometry. Its small frozen contract covers:

- one documented clockwise six-direction ordering;
- axial neighbor lookup and rotation;
- axial/cube distance and rounding;
- axial-to-pixel and pixel-to-axial conversion for rendering and hit testing;
- line traversal; and
- negative-coordinate-safe mapping to fixed-size storage chunks.

Names and return shapes must be deliberately designed once, fully typed, and pinned by tests. The
pixel convention, origin, orientation, direction numbering, rounding behavior on boundaries, and
negative chunk division are public behavior—not implementation trivia. Include round-trip,
six-neighbor, six-rotation, distance/line, edge/tie, and negative-chunk fixtures.

The shipped entrypoint received all of HexLife's normal package-boundary edits: source + `.d.ts`,
`vite.embed.config.js`, the package `exports` map, the explicit declaration-copy list in
`scripts/prepare-embed-package.mjs`, `docs/embed/entrypoints.md` plus a dedicated tracked reference
page, and the package README. It passed the embed release gates and was published as version 1.15.0.
HexFactory pins that exact version and imports `/hex` for host coordinates, direction tools,
placement hit testing, and Canvas rendering.

HexFactory's Rust protocol must pin the same direction numbering with cross-language fixtures, but
HexFactory remains independently buildable and owns its axial world IDs. Never reach into
`node_modules` from Rust build scripts and never source-import the HexLife repository.

No other package extension is required for the MVP:

- Do not modify `/sim`, `/ca`, `/stochastic`, or `/hcp` for factory semantics.
- Do not broaden the existing binary `/render` into a multi-layer factory renderer. The MVP owns a
  replaceable Canvas renderer. Only consider a new generic instanced-hex renderer after HexFactory
  has proven a reusable layer/delta contract; do not freeze that API speculatively.
- Do not put belts, recipes, inventories, scheduling, blueprint evolution, or factory codecs in
  `@hexlife/embed`. They belong to HexFactory.

Future package changes follow the same test: add them to `@hexlife/embed` only if they are generic
hex-host primitives with at least one credible non-HexFactory consumer. Implement factory-domain
features in HexFactory even when they happen to be useful to only one demo.

If the playable milestone exposes a genuine gap in the published `/hex` contract, first prove the
feature cannot be implemented cleanly with its existing public API. A blocking addition is
authorized only when it is small, additive, DOM/Wasm-free, broadly reusable hex-host functionality.
Read `X:\Programming\Projects\HexLife\AGENTS.md` and the relevant tracked embed docs, preserve its
unrelated worktree changes, and complete every source, declaration, export, build, declaration-copy,
test, reference-doc, README, changelog, and release edit required by HexLife. Run its relevant gates,
bump only the independently versioned `@hexlife/embed` package, commit and push the scoped change,
publish through the existing `embed-vX.Y.Z` workflow, and verify the npm artifact plus runtime and
TypeScript imports. Then exact-pin the published version in HexFactory and rerun all its gates.

That exception never permits factory/player/terrain/resource/inventory/recipe/technology semantics,
a public direction-convention break, or changes to HexLife's CA engines or renderer. Report such a
blocker instead of bypassing the boundary.

## Non-negotiable architecture

1. **Native hot path.** Rust/Wasm owns cargo movement, machine scheduling, inventories, recipes,
   conflict resolution, production counters, and checksums. JavaScript/TypeScript owns UI,
   rendering, build commands, and bounded orchestration. No per-cell or per-item JS tick loop.
2. **Separate data dimensions.** Building identity, orientation, cargo, item identity, inventory,
   recipe, and progress are separate fields. Never flatten their Cartesian product into one CA
   state byte or lookup table.
3. **Dynamic identities.** Items, recipes, and building definitions use dynamic integer IDs. Adding
   an item or recipe adds definition data; it must not resize a global transition table.
4. **Chunked, non-toroidal space.** Use unbounded axial/cube hex coordinates and lazily allocated
   chunks. A finite viewport is not a finite world contract. Empty map area should cost almost
   nothing.
5. **Compiled transport.** Directional belt tiles compile into directed paths/segments between
   endpoints. The simulation runs the compiled representation; it does not discover six neighbors
   for every belt on every tick. Turns are ordinary path geometry.
6. **Sparse scheduled machines.** Idle extractors, composers, containers, and consumers do not
   execute a universal cell update. Wake entities for due completions, available input, released
   backpressure, power/topology changes, or edits.
7. **Deterministic arbitration.** Simultaneous transfers cannot depend on Rust collection iteration
   order. Use stable entity IDs and explicit priority/round-robin rules. Avoid nondeterministic hash
   iteration in any state-affecting path.
8. **Integer time and quantities.** Core simulation uses integer ticks/fixed-point values. Same
   definitions, blueprint, commands, and tick count must produce the same checksum in browser and
   native tests.
9. **Definitions, not callbacks.** The MVP's behaviors are native components fed by data-defined
   items/recipes/buildings. Do not call JS once per machine, item, or tick. A deterministic bytecode
   escape hatch may be designed later, not improvised now.
10. **Simulation/render separation.** Rendering consumes compact snapshots or dirty deltas and never
    owns simulation truth. A simple MVP renderer is acceptable; it must be replaceable without
    changing the engine.
11. **Headless is first-class.** The same core must run without DOM/WebGL so future evolutionary
    experiments can evaluate many blueprints in workers or Node.
12. **No premature universality claims.** The initial slice proves the architecture; it does not
    claim Factorio feature parity or final performance.

## Long-term model

The intended engine has three cooperating representations:

- **Spatial chunks:** terrain/resources, placed footprints, orientation, selection/picking, and
  local dirty regions.
- **Compiled networks:** belts first; later fluids, power, signals, logistics, and long-range links
  get domain-appropriate network models rather than one universal cell rule.
- **Sparse entities:** stable entity IDs, component-oriented native arrays, inventories, recipes,
  progress, ports, and next scheduled event.

Evolution operates on a blueprint IR—place/remove/rotate/move, route, choose recipe, duplicate or
splice connected modules—not raw dense world bytes. A native evaluator will eventually return
throughput, latency, utilization, waste, footprint, resources, energy, and failure resilience.

## MVP vertical slice

The first live page must show a real native simulation, not an animation mockup:

`resource/extractor -> turning directional belt -> composer -> belt -> container -> consumer`

Minimum behavior:

- one resource deposit and extractor producing `ore` on an integer cadence;
- directional belts that may turn through the six hex directions;
- one data-defined recipe, e.g. `2 ore -> 1 component`, with integer duration;
- a container with a real integer inventory/buffer, not `empty/half/full` display states;
- a consumer that removes components and increments a native delivered counter;
- backpressure: blocked outputs wait without duplicating or deleting items;
- deterministic play, pause, single-step, reset, and speed controls;
- at least a small build/edit interaction: select a tool, place/erase, and rotate directional
  buildings/belts. A polished game editor is not required;
- visible cargo, machine progress/status, container quantity, delivered total, and current tick;
- a prebuilt working factory on initial load so the live URL demonstrates the vertical slice
  immediately;
- a stable checksum for the current simulation;
- the runtime simulates a compiled directed transport representation. For the first small MVP it is
  acceptable to recompile the complete affected blueprint after an edit; incremental connected-
  component recompilation is the next performance gate and must be recorded, not faked.

The MVP may use a straightforward Canvas 2D renderer if that is the shortest path to a correct live
proof. Do not spend the first session rebuilding HexLife's complete WebGL renderer. Keep the renderer
behind a small interface and state explicitly that GPU instancing is follow-up work.

## Suggested repository layout

```text
HexFactory/
  .github/workflows/pages.yml
  docs/ARCHITECTURE.md
  docs/MVP.md
  factory-wasm/
    Cargo.toml
    src/lib.rs
  src/
    core/            # Wasm wrapper, commands, definitions, snapshot adapter
    rendering/       # replaceable MVP renderer
    ui/
    data/            # item/recipe/building definitions
    main.ts
  tests/
  AGENTS.md
  README.md
  LICENSE
  package.json
  package-lock.json
  tsconfig.json
  vite.config.ts
```

Use Vite + TypeScript for the host, Vitest for host-side tests, Rust unit tests for simulation
invariants, and `wasm-pack` for the web artifact. Commit both npm and Cargo lockfiles in this
application repository. Configure Vite's production base for `/HexFactory/`. Pin the newly published
`@hexlife/embed` version exactly (no caret or range) and import its `/hex` entrypoint rather than
copying the TypeScript geometry implementation.

## First implementation gates

Rust tests must cover at least:

- a directed belt path containing a turn compiles in the intended order;
- cargo is neither duplicated nor lost in unblocked transport;
- backpressure preserves cargo and machine output;
- the composer consumes the exact recipe quantities and emits only after its duration;
- the container holds real quantities and releases them deterministically;
- the consumer's delivered count is exact;
- reset/replay produces the same checksum;
- behavior is independent of insertion order for any collection used to construct the blueprint.

Host tests should cover axial coordinate conversion/hit testing, command encoding, and definition
validation. Run formatting/linting, typecheck, Vitest, Rust tests, and a production build locally.

CI must run the same gates before deploying `dist/` through GitHub Pages. Pin tool/action versions
where practical; the existing HexLife Pages workflow is a useful reference but should not be copied
blindly.

## GitHub and delivery requirements

- Check whether `Sidem/HexFactory` already exists before creating it. Never overwrite an existing
  unrelated repository.
- Create a **public** GitHub repository named exactly `HexFactory`, with default branch `main`.
- Add an MIT license and a README whose first section links to the live demo.
- Push the intentional initial implementation to `origin/main`.
- Configure GitHub Pages to deploy via GitHub Actions. If repository Pages must be enabled through
  the API, do so after the first push.
- Wait for CI/Pages, inspect failures, fix them, and verify that
  `https://sidem.github.io/HexFactory/` returns the deployed app—not merely a successful local
  build or workflow dispatch.
- Report the repository URL, live URL, commit, test results, and any explicitly deferred gate.

## Explicitly out of MVP scope

Splitters, inserters, multiple belt lanes or tiers, fluids, power, circuits, trains, enemies,
multiplayer, arbitrary mod bytecode, neural agents, evolutionary UI, massive-map performance claims,
and asset or content imitation of any commercial title. Several of these are wanted eventually —
the design pillars call for an open-ended game, and depth arrives through them — but they were out
of the founding slice's scope and each still needs its own milestone rather than an improvised
addition.

The asset rule is permanent and is not an MVP-scope item: HexFactory takes design inspiration from
Factorio, Satisfactory, and Minecraft and takes nothing else from them. Original neutral shapes,
names, and systems only.

## Historical founding-session prompt (completed)

The founding prompt created the repository, published `@hexlife/embed/hex@1.15.0`, implemented the
native factory slice, and deployed the first live page. Its durable results and boundaries are
recorded above; the obsolete prompt itself is intentionally not carried forward as project guidance.

## Historical milestone — Playable Game v0.2

The next release turns the architecture proof into a small, complete game. A new game begins in a
deterministic seeded environment with the player beside a landing hub. The core loop is:

`explore → gather → build extraction and transport → deliver → research → compose → win`

Keep the founding prebuilt factory available as a selectable **Factory demo** scenario, but make
the playable new-game scenario the default live experience.

### 1. Deterministic environment and finite resources

- Rust owns a versioned world seed and traversal-order-independent chunk generation.
- The initial terrain vocabulary is ground, water, blocking rock, finite ore, and one finite
  secondary resource such as biomass or crystal. The landing hub and player spawn are deterministic.
- Terrain, resource kind and quantity, collision, and placement legality are native state. The host
  may derive purely decorative variation from the seed but may not invent simulation truth.
- The world remains unbounded and chunk-ready. Generating chunks A then B must produce the same
  state and checksum as generating B then A.

### 2. Native player and inventory

- Add native player position, facing, inventory, action cooldown, and build range.
- TypeScript sends bounded input commands. Rust resolves movement, collision, gathering, costs,
  unlocks, and placement. Browser frame rate must not affect the deterministic result.
- Provide WASD movement with a documented axial mapping, plus pointer or keyboard access to the
  remaining hex directions; interact/gather, hotbar selection, rotate, place, and erase controls.
- The player has a real `item_id → quantity` inventory with exact conservation. Define and test one
  fixed erase-refund policy. Protect the landing hub and scenario-owned objects from deletion.
- Use integer fixed-point movement or a native one-hex movement cadence. Camera following is host
  presentation only.

### 3. Data-defined construction and recipes

- Extend item, recipe, and building definitions with construction costs, unlock requirements,
  placement rules, descriptions, and host icon metadata.
- The playable set includes belts, an extractor that consumes a finite deposit, a container, a
  composer, and the landing hub/consumer.
- Rust rejects unavailable or unaffordable construction even when a forged host command requests
  it. Extractors stop when the underlying deposit is empty.
- Preserve compiled transport. Full graph recompilation after an edit remains acceptable for this
  milestone; incremental connected-component recompilation remains deferred.

### 4. Native, data-defined technology tree

- Add `src/data/technologies.json` with dynamic IDs, prerequisites, integer costs, descriptions,
  and definition unlocks.
- Rust validates unique IDs, existing prerequisites, an acyclic graph, positive costs, and valid
  unlock references.
- Use one coherent research currency: the landing hub awards integer **insight** according to
  data-defined delivered-item values, and the player explicitly spends it. Spending and unlocking
  are one atomic native operation.
- Provide a short progression through Field Logistics, Automated Extraction, and Composition, with
  an optional Storage Planning branch.
- A native objective requires delivery of a defined number of composed components. Reaching it sets
  a persistent victory state while allowing the player to continue.

### 5. Save, load, and scenarios

- Add a versioned native `HXF1` save contract containing definition/scenario version, seed,
  modified chunks and resources, player/inventory, researched technologies, blueprint, machine and
  cargo state, counters, tick, and checksum.
- A loaded save advanced by the same commands must match the uninterrupted checksum. Reject
  malformed or incompatible saves explicitly.
- `localStorage` may store the native save string; TypeScript must not reconstruct simulation state.
- Support New Game, Save, and Continue, plus the retained Factory demo scenario.

### 6. Presentation and usability

- Keep the replaceable Canvas 2D renderer. Add player-follow camera, host-only pan/zoom, and clear
  layers for terrain, deposits, buildings, belts, cargo, player, hover, selection, and placement
  legality.
- Use original neutral geometric art for the player and buildings; do not imitate commercial game
  assets or branding.
- Add a readable HUD for inventory, insight, objective, selected tool, tick, speed, and pause state;
  a technology panel; a cost/unlock-aware hotbar; and a selected-tile inspector.
- Add restrained cargo interpolation and feedback for gathering, placement, research, depletion,
  and victory. Animation never becomes simulation truth.
- Support desktop and narrow layouts, keyboard focus, visible labels, and reduced-motion preferences.

### 7. Architecture gates

- Rust owns terrain, collision, the player, gathering/depletion, inventories, build costs and
  legality, unlocks/research, objectives, saves, transport, machines, cargo, and ticks.
- TypeScript owns input sampling, UI, camera, audio, interpolation, and bounded rendering. Send no
  more than one bounded input batch per rendered frame; add no JavaScript movement, progression, or
  factory simulation loop.
- Keep environment, player, building, orientation, cargo, inventory, recipe, research, progress,
  and presentation as separate dimensions.
- Every state-affecting order is explicit and stable. Chunk visitation, JSON order, and map/hash
  iteration may not alter a checksum.
- Begin with exactly `@hexlife/embed/hex@1.15.0`. `X:\Programming\Projects\HexLife` is reference-only
  unless the controlled generic package prerequisite above is genuinely triggered. Never
  source-import it or reach into internals.
- Preserve the compiled graph, independently headless native core, and prohibition on unbenchmarked
  performance claims.

### 8. Acceptance tests and release gate

Rust tests must add coverage for:

- chunk generation independent of request order, with pinned same-seed and different-seed fixtures;
- six-direction player movement, facing, blocking terrain, and deterministic cadence;
- gathering, finite depletion, and item conservation;
- placement enforcement for terrain, occupancy, range, cost, and technology;
- extractor behavior when its deposit empties;
- research prerequisites, exact atomic spending, unlocks, and rejection of forged locked commands;
- one complete progression path from landing through the native victory objective;
- `HXF1` round-trip and save/resume checksum equivalence;
- insertion-order and chunk-visitation-order independence; and
- all founding transport, backpressure, recipe, container, delivery, and reset invariants.

Host tests must add coverage for bounded keyboard input, absence of a host movement loop, pan/zoom
picking through `/hex`, hotbar costs and locks, technology prerequisites, the expanded snapshot
adapter, native-save delegation, responsive controls, and accessible labels.

Completion requires `npm audit`, formatting, lint, strict typecheck, Vitest, Rust tests, Wasm build,
and production build before deployment. Then wait for GitHub Actions and Pages and verify in a real
browser: new game, movement/collision, gathering, research, construction/editing, running factory,
victory, save/continue, narrow layout, and a clean console.

### 9. Explicitly deferred after v0.2

Enemies, combat, survival meters, multiplayer, networking, fluids, power, circuits, trains, drones,
inserters, splitters, multi-lane belts, broad biome simulation, mod scripting, evolution/neural
features, a WebGL rewrite, large-scale claims, and substantial music/audio production.

## Historical exact-session prompt — implement Playable Game v0.2

Copy everything inside the following block into a fresh Codex task:

```text
Work in X:\Programming\Projects\HexFactory. Read AGENTS.md, docs/HEXFACTORY-PLAN.md,
docs/ARCHITECTURE.md, and docs/MVP.md in full, then implement the plan's complete “Playable Game
v0.2” milestone. The goal is a polished but deliberately basic playable starting point from which
we can continue development—not a design update, scaffold, or final attempt at the whole genre.

Follow every scope, architecture, determinism, test, documentation, and acceptance requirement in
the plan. Keep all simulation and progression truth in the headless Rust/Wasm core; TypeScript owns
only bounded input, UI, camera, presentation, and rendering. Preserve compiled transport and consume
only the public, exactly pinned @hexlife/embed/hex package. If its API genuinely blocks the work,
follow the plan's controlled generic package-update and release procedure; never add factory
semantics to HexLife or source-import it.

Implement, test, and integrate the whole playable vertical slice, preserving unrelated work. Run all
local quality gates, commit and push the completed HexFactory work, fix CI and Pages failures, and
wait for deployment. Verify the live game at https://sidem.github.io/HexFactory/ in a real browser
through the planned core progression, editing, victory, save/continue, responsive layout, and a clean
console. Do not stop at a partial implementation, local build, or pending workflow. In the final
handoff report commits and any package release, every gate, deployment and browser verification, the
delivered architecture, and clearly named follow-ups. Do not make unbenchmarked performance claims.
```
