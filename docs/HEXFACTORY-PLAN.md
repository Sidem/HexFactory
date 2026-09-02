# HexFactory — goal, state, and roadmap

This is the live document: what the game is for, what exists today, and what to build next.
Architecture decisions are in `docs/ARCHITECTURE.md`, the invariants an agent must not break are in
`AGENTS.md`, art rules are in `docs/ART.md`, and every performance claim is backed by
`docs/BENCHMARKS.md`.

**Keep this file short.** It is a working document, not a book. A shipped release collapses to one
ledger line; a completed phase collapses to the constraints that outlived it. Reasoning lives in the
git history and on the constants it constrains.

## The goal

A beautiful, open-ended factory-automation game that is fun to play for its own sake, fascinating to
keep exploring, and a pleasure to control — Factorio's automation depth, Satisfactory's sense of place
and scale, and Minecraft's freedom to build what you want where you want, in hexagonal space.

The deterministic Rust/Wasm core, the sparse architecture, the compiled transport graph and the narrow
`@hexlife/embed` dependency are means, not ends. Where an architectural preference and the player's
experience genuinely conflict, the player's experience wins and the architecture finds another way to
pay for it. That weakens no invariant — determinism, native ownership of the tick, sparse cost and
measured-before-claimed stay non-negotiable — but it decides how milestones are chosen: engineering
work earns its place by naming the player-visible thing it enables.

Inspiration, never imitation. Original neutral shapes, names and systems only. Permanent, not a scope
item.

### Design pillars

- **Fun is a requirement, not a polish pass.** A release that is correct, fast and joyless has not met
  its acceptance criteria. Every milestone states what it makes better to play.
- **Controls must be obvious in the first minute and precise in the hundredth hour.** A control that
  needs explaining is a defect in the control.
- **The player should always know what just happened and what to try next.** Feedback is part of the
  mechanic, not decoration.
- **The world should reward looking at it.** Readability first, beauty close behind.
- **Open-ended, not aimless.** Progression opens options rather than prescribing a route. Victory is a
  milestone in a longer game, never a wall.
- **The world and the factory answer each other.** Geography, living populations, extraction, waste and
  recovery change one another visibly.
- **Hexagonal space earns its place.** A system becomes hex-native only when faces, rings, fronts or
  multiple approach directions change a legible decision. No invisible adjacency bonuses.
- **Nothing may stutter.** Frame stability, instant response and exact restores are player-experience
  features. This is what the measured capacity ladder protects.

## Where the project stands

A run today: land beside a hub in a world chosen by preset or raw parameters; walk out under fog across
rivers and coastline, on the keys or by clicking a selected hex a second time; find fields of ten raw
materials; cross rivers on bridges; gather from forests that visibly thin and regrow; fill the hub's
posted requests and its staged founding contract; research the rest; and build a powered, automated line
of buildings and 26 recipes — belts that split, merge, climb the two-row period and pass under the lanes
they cross. Fence a yard or raise brick and concrete walls straight across hexes on the vertex lattice.
Refine oil into bitumen and fuel, mix asphalt, lay roads over gravel. No landform is permanent: one
`Lower` cut takes a cliff face down. Buildings are generated low-poly instanced geometry from the shape
grammar, so a tier stays a data row. The world renders through Three.js; the minimap is WebGL2.

**Envelope numbers** — native refuses a load on all six, and the save catalog shows which one moved:

| Envelope              | Version |
| --------------------- | ------: |
| `HXF1` save           |      41 |
| Definitions           |      30 |
| Technologies          |      16 |
| Scenarios             |       8 |
| World generator       |      12 |
| Wire (snapshot delta) |      23 |

These are the completed Phase 8 numbers, on `main` and not yet carrying a release number. Save 36 and
below is the old 1 m² world, refused with an export path rather than migrated; 37 advances by stamp to
38, 38 to 39, 39 to 40, and 40 adopts empty live-erosion state at 41.

**Measured capacity.** The v0.43 audit puts the complete 6,144-entity browser frame at 32.3% / 33.5% /
33.9% of 60 Hz on Low / Medium / High on the reference desktop at 1440×900 DPR 1 — all pass the 35%
gate, by only 1.1–2.7 points. The native frame is 1.37 ms at the same tier, the tick 0.287 ms.
Generation costs at most 1.42 µs per hex. **The reference desktop is the support target** (2026-08-27);
integrated-GPU laptops are not a supported configuration. No claim beyond a recorded tier.

**Last release: v0.46.0 Shaped Ground.** **On `main` since, unreleased: the complete Phase 8 scale
break** — one construction hex is 25 m², altitude is physical native height, drainage and sparse water
answer the ground, and bounded geomorphic epochs let surveyed rivers answer what the player built.

**Where it is weak.** The foundation is strong and unusually trustworthy; the game is still a polished
short-form vertical slice. The first two hub stages and 27 finite projects give the present roster a
reason to exist, but an established factory has no programme after the foundry module. Rows 9, 11 and 12
are the planned answer; do not invent repeatable filler quests in the meantime. Two concrete debts: file
concentration still costs an agent more context than the behaviours require (`factory-wasm/src/lib.rs`
is over 27,000 lines, `src/main.ts` 5,482), and the production build warns on an 816 kB main chunk
beside a 1.22 MB Wasm artifact with no stated startup payload budget. Keep the generic Extractor as the
starter and add a player-facing machine family only when it brings a distinct decision — recoloured
synonyms lengthen the catalogue without deepening the factory.

## What to do next

### Playtest report — 2026-09-02

Four defects and six requests. The defects sit ahead of row 9; each request is placed against the row that
owns its system. Three of the four are traced to a named line, so work starts from the cause.

**Defect — a smelter refuses coal into Fuel while Steel is selected.** Select Sand → Glass, put coal in Fuel,
switch back to Steel, and the same smelter accepts it. `stock_kind_for_item` in
`factory-wasm/src/core/transport.rs` resolves exactly one compartment per item and lets inputs outrank fuel
deliberately, so feeding steel does not divert its own bill into the firebox. Steel bills two coal → `Input`,
so `stock_accepts_item` refuses the named Fuel slot; Glass bills none, so the same lump is `Fuel`. **The
precedence rule is right and the arbitration is the defect** — an item that is both has two honest
destinations and one compartment to express them in. Settle the rule first: an explicitly named target stock
is honoured whenever the item qualifies for it at all, and only `StockKind::Auto` keeps inputs-outrank-fuel.
Then apply it to the belt path (`accepts_item`) and the hand path in one change, covering coal into Fuel and
into Input by hand and belt, automatic delivery still filling inputs first, burner-only machines unchanged.

**Defect — black dots along the unexplored border.** Measure before guessing: capture the rim at each zoom
step on Low, Medium and High. Three suspects interact. The frontier dither in
`src/rendering/three/terrainSurface.ts` discards fragments on a screen-space hash, so the last two survey
rings are deliberately full of holes; the frontier skirt from `heightfieldTerrain.ts`, dropped to
`WORLD_FLOOR` in `terrainMeshes.ts`, stands behind them unlit from above; and the distance fog in
`ThreeFactoryRenderer.ts`, meant to swallow that seam, is scaled to a fraction of the screen rather than to
fixed scene units, so its cover changes with zoom — the axis to check along. The dither is opaque by design
because draw ordering and shadow baking depend on it; do not answer this by making the rim transparent
without re-checking both.

**Defect — WASD walking has no gait, and click-to-move hitches.** Both causes are in the player pose update in
`src/rendering/three/worldInstances.ts`, and both are presentation only. The gait is gated on
`player.walk_path.length > 0` — the pathfinding route rather than motion — so a player on the keys is not
"walking" by that test and both legs hold at zero; gate it on displacement. The stride phase is
`sin(((player.x + player.y) / WORLD_SCALE) * 8)`, a function of position rather than distance travelled, so a
heading along which `x + y` barely changes nearly freezes the cycle, every waypoint turn moves the phase
discontinuously, and arrival snaps to zero — the hitching. Drive the phase from accumulated distance or the
player clock, and ease the last step into idle.

**Defect — clay and sand are hard to find, and deposits read as circles.** Two claims; only the second is safe
to act on directly. `default_site_rules` in `factory-wasm/src/model/world_sites.rs` already carries four clay
rules and two sand rules, and its comments record two earlier corrections of this exact shape. Scarcity is a
measurement question first: run `npm run survey` across the presets and read the clay and sand counts and
first distances before any weight moves — if both are present within reach, the defect is legibility and the
fix is not in the generator. The shape half is not in doubt: sites are placed from a jittered lattice with a
radius, which is what makes a deposit read as a blob. Replacing that falloff with a noise-masked,
drainage-aware shape moves the world generator envelope, must keep every preset completable and must re-pass
the survey's access gates — scheduled work, not a tuning pass.

**Request — a composer stopped mid-craft can be neither reassigned nor cleared.** `Core::set_recipe` in
`factory-wasm/src/core/configuration.rs` refuses whenever `progress > 0`, because the reserved inputs belong
to the running job — but a manual-work machine is disabled the moment a recipe is set, so a composer stopped
at 55% keeps that progress indefinitely and the only move is demolition. Decide the abort rule first: a full
refund is the honest default, since demolishing already returns `reserved_inputs` and a lossy abort would make
demolish-and-rebuild cheaper. Then add a cancel beside `set_recipe` that clears progress, returns the reserved
inputs and leaves fuel and output alone, stating its bound, undo behaviour and checksum exactness.
`set_recipe` then stops being a dead end: mid-craft it asks for confirmation naming what will be discarded.
The confirmation is UI; the accounting is native.

**Request — fertile riverbank soil**, placed on row 9. Phase 8 shipped the drainage model, so a fertile bank
is derived rather than invented: a cell adjacent to a drainage edge and within a stated height of the water
surface, tagged by the same deterministic generation that produced the bed. Two costs first. `Substrate` is a
four-value wire-coded enum in `ground_spine.rs`, decoded in `wire.rs`, so a fifth value moves the wire and
world generator envelopes; and a fertile band along every river is a great deal of favoured ground, so it
needs a scarcity rule before it becomes the answer to every placement question. Row 9 tags the ground; row 11
decides what grows on it.

**Four further requests sit in the rows that own them**: animals that breed and can be overhunted (row 9), a
swimming rung between the ford and the bridge (row 11), and biome flora and props and a coastal harbour with
vessels (both on the longer horizon, each named with what it waits on).

### The phase order

Phases 1 to 7 are **shipped**; pipes shipped between rows 7 and 8 as v0.45.0 without reordering the sequence.

- **8 — Flowing water.** Complete on `main`, unreleased.
- **9 — Living Lattice.** Animals, biomatter and waste as one ecological system, plus the riverbank
  fertile-soil tag row 11's food chain needs. Reuses phase 4's joint-output costing.
- **10 — Supported floors and vertical transport.** Support classes, the first upper floor, stairs, belt
  lifts, a layer view. Needs the beams and concrete phases 3 and 4 produce.
- **11 — The primitive human.** Needs and attributes. Depends on rows 9 and 10 for a food supply worth
  automating; revises the skills budget rather than sitting beside it.
- **12 — Regional Discovery.** Survey tools, distant sites, outposts — the play half of regional variation.

These are delivery phases, not one giant release, and a phase may ship as several versions. Do not start a
later row in parallel with an earlier one unless the user changes priority; an unmet gate is resolved or
brought back to the user rather than answered by switching rows. Necessary fixes, measurements and shared
prerequisites are part of delivering a row; optional extensions stay optional.

**Floors moved behind Living Lattice on 2026-08-29 at the user's direction**, for player progression rather
than engineering: the player should learn the ground-level systems before the game asks them to think in
levels. Rows 5 and 6, flowing water and the primitive human were added 2026-08-28 at the user's direction;
their priority is approved, their costs and tuning hypotheses are not. Masonry and the vertex lattice did not
complete the enclosure work — roofs, rebar and steel frames attach to row 10, because they are structural.

**Entry work, not a new phase.** Item 1 belongs to row 7; items 2–4 are gates on row 10.

1. Make guidance name the first executable action and remove the initial hub/mission duplication; add
   construction search and a visible narrow-dock overflow cue.
2. Before level IDs widen native state, mechanically move the inline native tests and capacity harness out of
   `lib.rs`, then extract only the occupancy/placement/transport slices row 10 must touch. Split the
   corresponding session/panel wiring out of `main.ts`. Preserve behaviour, checksum, save and wire at each
   step; this is not authority for a rewrite.
3. State the supported save-migration window in player-facing text. Phase 8 made the boundary explicit, tested
   and exportable; what is missing is the promise itself, rather than a player discovering it from a disabled
   Load button.
4. Add a deterministic stacked-floor/lift capacity tier and rerun Low, Medium and High before a floor release.
   The 6,144-entity record is already near the desktop gate.

## Phase 8 — Flowing water

**Complete on `main`, unreleased.** Water stopped being a property of a cell. The phase owned the scale,
altitude, footprint and rendering break that realistic mountains, valleys, springs and rivers required. The
shipped rules live in [`ARCHITECTURE.md`](ARCHITECTURE.md); the measurements, including three rejected
changes, live in [`BENCHMARKS.md`](BENCHMARKS.md) and are reproduced by `npm run terra`, `npm run water` and
`npm run erosion`. What follows is only what still constrains work on this ground.

### The scale contract

One physical system, not independent knobs, and it supersedes the old 1 m² ledger globally — physical scale
is not another world slider.

- **One construction hex is 25 m².** The axial lattice and pointy-top topology are unchanged; the physical
  interpretation moved by five in linear scale, so neighbouring centres stand about 5.37 m apart. Never
  multiply `HEX_X`, `HEX_Y` or saved coordinates — retune metre-derived rates and reaches instead.
- **One height quantum is 0.25 m.** Generated bed elevation is a signed absolute integer with sea level at
  zero, so a 2,000 m summit is ordinary data rather than a terrain enum.
- **Earthworks name metres.** Raise and Lower are 0.5, 1.0 and 1.5 m; the content limit is ±8 m from the
  generated bed, not the storage limit; one quarter-metre cell layer is one spoil unit (6.25 m³).
- **Movement and construction do not share one threshold.** Walking reads slope, steps, surface and water
  depth; a multi-cell building needs a pad within one quantum unless its foundation class says otherwise.
- **`PLAYER_SPEED` stays at 275** (settled 2026-09-01). A hex still takes about 0.36 s to cross and the metre
  figures moved instead — a 15 m/s walk, a 25 m/s run, a 5 m/s ford. Holding 3 m/s would have multiplied every
  journey by five, which is the one thing a 25 m² hex was not meant to buy. This is not the belt case in
  different clothes: the factory reads a belt's speed and balances against it; nothing reads the player's.

Four facts the old `Terrain` band conflated are now separate, and the split is permanent:

```text
generated bed elevation + earthwork delta + erosion/deposition delta = finished ground elevation
surface/substrate material                                           = separate derived identity
water depth, surface and discharge                                   = separate hydrology
resource field                                                       = already separate
```

Generated bed, substrate and initial hydrology are pure functions of generator version, seed, parameters and
coordinate, cached as derived data only. Only the earthwork delta, the erosion delta, departures from water
equilibrium and non-zero erosion accumulators are saved and checksummed. Never save a generated height because
a renderer needs it, and never duplicate the height generator in TypeScript.

### Water is equilibrium plus sparse disturbance

A spring is a boundary condition with a finite rated discharge, not a cell that injects a water item forever.
Generated rivers carry a stable flow field whose surface can animate while the simulation is settled, costing
no tick work.

The player creates sparse hydrology state only by changing the equilibrium — cutting a channel, raising a dam,
opening an outlet, pumping a pond, diverting a spring, flooding a pad, draining a basin. Such a change
schedules a bounded **active region**; native resolves depth, surface and discharge to a fixed point, then
removes the region from the schedule. **No full-world or permanent per-cell water kernel is allowed.**

- Spreading water may never insert a chunk into `generated_chunks`. Disturbed flow stops at the surveyed
  frontier against a deterministic boundary flux and resumes when survey exposes the next region.
- Oceans and untouched rivers are derived boundary conditions, not running entities. Only disturbed cells
  carry saved departure state.
- Source discharge creates water and terminal outlets remove it, so global conservation is not claimed; local
  transfers must neither duplicate nor lose depth, and every solve terminates within an explicit budget.
- Neighbouring sources never manufacture another source. Springs are generator identity or an explicit
  construction, never an emergent adjacency trick.
- A pump draws against local depth and replenishing discharge, and reports its named source and limiting rate.
  Loose water in pipes stays factory cargo; pipe transport is not a hydraulic pressure simulation.
- **Live erosion is a sparse geomorphic epoch, not a fast terrain tool.** It exists so an old factory can watch
  a river answer what the player built. Erosion may expose or bury a surface resource only through an explicit
  rule — never as a side effect of lowering ground.

### Still owed

A distant aggregated terrain LOD, before mountain scale is claimed: a peak the camera cannot show until the
player stands on it has not solved sense of place. And the player-facing migration window, per entry work item
3 above.

## Phase 9 — Living Lattice

Animals, biomatter and waste as one milestone. This is the first system that makes HexFactory something other
than a factory game drawn on hexes: a living population moves, feeds, breeds, recovers and can be depleted
past recovery across hex neighbourhoods. Biomatter comes from that population rather than from a renamed
static field, and waste is a byproduct with a visible destination — a recovery loop, a damaged habitat, or a
refining step. Producer, byproduct and consumer are designed together so none is decorative.

This is **not** a pollution-and-enemy-wave substitute. The pressure is ecological consequence and opportunity,
not a timer that sends attackers. A player should be able to preserve a productive migration, intensify it
carefully, exhaust it for an urgent contract, or repair a region they damaged.

**The 2026-09-02 playtest asked for the concrete form of that**, and it is this brief rather than an addition
to it: sparse herd entities that graze, grow toward a local carrying capacity, and thin toward local
extinction when harvested past it. That is also the producer the waste and recovery loops need. Two things
must be true before it ships — overharvesting is recoverable in some places and permanent in others, so the
choice is real, and the player can see which of the two they are doing while there is still time to stop.
**Riverbank fertile soil lands here too**, as ground rather than gameplay: row 9 owes the tag and nothing that
grows on it, on the terms the playtest report states.

Hex topology earns itself here: movement and propagation use six neighbours, a herd or recovery front has a
perimeter, extraction reach is a ring, and machines expose meaningful faces when a process has directional
intake, output, heat or waste. Do not add generic adjacency percentages that collapse into one solved
blueprint. Rust/Wasm owns every ecological tick, through sparse scheduled populations or active fronts.

Three things earlier milestones already hand over:

- **Reuse the joint-output foundation the petroleum row delivered** — named-route costing, multi-output stock
  handling, allocation, contract expansion and guidance. Check the allocation stays valid for ecological
  inputs and outputs; secondary outputs are not automatically free.
- **A new contract stage is a data row.** Stages live in `scenarios.json`, `HUB_LADDER` has one entry per
  stage the hub can finish, and `tests/look.test.ts` fails if a shipped contract can complete a stage the
  ladder cannot draw.
- **Guidance follows the contract for free, but only through recipes.** `nextAction` walks recipe inputs and
  categories, so an ecological _process_ with no recipe row will not appear at all. The one thing that must
  not happen is a hub asking for something the next step cannot explain.

**Acceptance.** One complete loop produces useful biomatter and a waste stream with at least two legible
responses of different ecological outcome. The same installation in two habitat states does not have the same
answer, and the reason is visible in the world. A population recovers, migrates and collapses
deterministically, reproduced exactly by saves and checksums. The founding hub asks for something from the
loop, and every new definition reaches `fixtures/balance.json`.

## Phase 10 — Supported floors and vertical transport

Ground plus one usable upper floor first; expand only after it is legible and measured. The destination is a
multi-level building — machines on several floors with belts moving material within and between them, inside
one structure the player reads as a single works. Not a voxel editor and not a structural-collapse simulator.

- **Logical levels:** position becomes an axial cell plus an explicit level ID; floor 1 is not occupied by the
  machine below it. Foundation grade and floor index stay distinct. Existing corner belts stay planar.
- **Supports and loads:** definition-driven load classes and maximum spans; the preview names the cells needing
  columns and the machines that are too heavy. Recalculate changed support regions on edit, never every
  building every tick. No surprise collapse, and no inventory lost to one.
- **Floor openings:** stairs, lifts, columns and shafts reserve full footprint and headroom across levels. An
  apparently empty cell cannot hide a conflicting shaft above it.
- **Belt lifts:** explicit intake/output endpoints join compiled graph edges across levels; cargo, progress,
  buffers, capacity and energy stay native, with identical conservation and backpressure. Removing a loaded
  lift recovers its stock or refuses safely; a direction change cannot teleport or duplicate cargo, and
  existing underpasses must not acquire cross-level connections.
- **Player access:** stairs first, elevators later; walking, reach and interaction resolve the correct level.
  Adjacent positions on different floors never connect implicitly — define explicit risers, pipes included.
- **Editing view:** active-floor selection, hide/fade above, ghosted context below, layer-aware selection,
  marked shaft destinations. Picking intersects the selected logical plane and never reads height from a mesh.

**The structural half of the enclosure family lands here**, because it exists to carry a floor: reinforced
concrete wall and column for heavy decks, steel frame and cladding for larger clear spans with stated load
limits, roofs per material with automatic cutaway. Higher floors and heavier equipment should be what creates
demand for beams and rebar — a first small upper room must not require reinforced concrete. Underground strata
stay a separate decision.

**Acceptance.** A useful stacked factory with no hidden routing, load and removal validation, full cargo
conservation, and readability and performance evidence at the recorded tier. If one upper floor cannot be
edited confidently at normal zoom, fix the layer view before expanding scope.

## Phase 11 — The primitive human

The player becomes a primitive human with needs and attributes rather than a camera with a pack. **This
reverses a standing guardrail deliberately.** The old wording — do not invent endurance, hunger or a movement
grind to fill empty branches — was written against padding, not against the genre, and it survives in a
stricter form that governs every bullet below:

> A need must create a **reason to build** something. A need that only makes existing actions slower is a
> tax, and a tax is the failure mode the original guardrail was pointing at.

Hunger is the test case: hunger that interrupts factory work to walk to a stockpile is a tax; hunger that makes
foraging, farming, cooking, preserving and storing worth automating is a system.

- **Attributes are bounded and legible.** Strength raises carry weight and hand-work speed; it never becomes an
  invisible multiplier on every rate. Each attribute states its exact effect and ceiling.
- **One player-progression story, not two.** Skill Points already buy Carrying, Construction reach and
  Surveying range. Either attributes replace those ranks or they set the base the ranks modify — never two
  currencies buying the same bonus.
- **Native state.** Hunger, condition and attributes are saved and checksummed, owned by the player clock that
  runs independent of the simulation rate — so whether a need advances while the factory is paused is an
  explicit decision.
- **It depends on rows 9 and 10.** Needs built before there is anything to satisfy them with are the grind.
- **Deep water is the missing traversal rung, and it belongs here because it is a need.** Asked for 2026-09-02.
  Both ends exist: a wade limit separates the ford from water that refuses, and bridges cross what the ford
  cannot. Swimming is the middle, and the guardrail decides its shape — it earns its place by making something
  worth building or carrying, such as a shore swum once and then bridged because it will be used daily. Stamina
  in water is where a condition need can legitimately bite, because the alternative is visible and buildable.
- **No death spiral, and no idle decay as a difficulty knob.** Failing to eat narrows options recoverably.

**Acceptance.** Every need names the thing it makes worth building, and a playtest confirms a player built that
thing because of the need rather than in spite of it. No existing action becomes slower without a stated
compensation. Attributes and Skill Points have one reconciled budget, migrated without taking anything from an
existing save.

## Phase 12 — Regional Discovery

The generation half moved forward into v0.21 — the bootstrap guarantee and the survey that proves every preset
works. What remains is the play system, starting from a world that already has readable landforms, rivers and
real deposits.

Advanced materials and ecological opportunities belong to readable regions that require travel, surveying and
eventually outposts. Every preset stays completable, but "completable" no longer means "sample platter at
spawn." A third low-frequency generation channel may create dry and wet variants of one elevation band, but
generation is not the milestone: a region has to announce itself through shape, colour, life, sound and
material behaviour; the survey tool hints rather than reveals; and a distant discovery must create a reason to
establish a specialized site rather than carry one stack home. Landing contracts and later hub modules provide
that reason.

Signal crystal is the strongest candidate for a later hex-native automation language — relays along faces,
triangular links, closed rings. It stays a candidate until Living Lattice proves which signals the player
actually needs; do not build a programmable system in search of a problem.

**Acceptance.** Every preset remains completable, measured by a survey reporting first advanced-region
distance, regional extent and access from buildable ground. Crossing into a region is recognisable without
opening the menu. At least one founding-hub project requires a sustained distant site. The minimap and home
bearing support the expedition without revealing unsurveyed world.

## Open decisions

- **Does `regrowth_ticks` move** now that a forest cell holds one to four wood instead of ten to twenty-two?
  Tuning slowed the cadence fivefold (90 → 450) so a cut forest reads as a place that has to recover. That was
  a judgement about pace, not a measurement: what is still unmeasured is an extractor's starve rate over seven
  cells against 450, which is what decides whether forestry is viable at all.
- **Does the hub board lead to meaningful research rather than repeat-income farming?** Settled in v0.35.0 by
  making demand finite — every project pays once and `repeat_insight` is gone. Quote
  `fixtures/balance.json` for the current budget rather than a remembered pair. What would reopen it: playtest
  evidence that the surplus is loose enough that purchase order stops being a real choice.
- **Rails or free-floating panels** was settled by shipping the rail. What would reopen it: wanting positions
  the player chooses, at which point saved coordinates, overlap, off-screen recovery and touch gestures all
  come back.

**Not open:** `DIRECTIONS` stays six. Twelve headings are transport only. Widening adjacency would let a boiler
reach a turbine two rows away and a pole span a distance no player can see.

**Two gates were withdrawn on 2026-08-27 by user decision** and are recorded so neither returns as a surprise:
the timed human playtest of the opening (the opening stands on `fixtures/balance.json` instead), and physical
integrated-GPU qualification, withdrawn with the laptop support target. What the one informal session found is
shipped and stays shipped: players read an accidentally paused factory as a failure, so player pause,
single-step and variable speed are gone and the rate is fixed at 10 tps.

## Longer horizon

Decisions rather than omissions, each with the thing it is waiting for. Necessary shared prerequisites do not
bring their whole feature families forward.

- **Hub programmes.** Player-chosen modules grow around the hub's rings and create different material demands.
  Finite authored systems and visible construction, not endless random chores.
- **Six-face machines.** Ports, heat, exhaust or control attached to named faces where direction creates a
  readable routing choice. Available shapes, not mandatory bonuses.
- **Intermittent generation and accumulators.** They arrive together. Intermittency is a deterministic function
  of tick and position, never a runtime roll.
- **A day cycle, and solar with it.** Chosen for what it does to the game's feel, not smuggled in as a power
  source.
- **Pressure and flow.** v0.45 ships deterministic one-unit pipe transport. A pressure model may deepen it only
  when it creates readable routing choices; it is no longer a prerequisite for keeping fluid off belts.
- **Organic tileables.** The later half of the art generator: tileable textures and shapes so a hex lattice
  reads as organic terrain and objects. Generated, presentation-only, never a checksum input.
- **Biome flora and props.** Asked for 2026-09-02: cacti and rocks in desert, alpine conifers and boulders on
  the tops, reeds and driftwood on the coast. A prop table per biome drawn as sparse instanced geometry on the
  pattern the resource fields already use, presentation only — a prop never occupies a construction cell, never
  refuses a placement, and never reaches a save or checksum. It waits on organic tileables rather than leading
  them, because a prop and the ground under it must be authored against one terrain vocabulary. Row 12 is where
  it pays off.
- **A harbour and working vessels.** Asked for 2026-09-02: a coastal building half on land and half on water,
  producing fishing cutters or diving craft to work water-based nodes. It waits on three things, none of them
  boat geometry. A footprint straddling the shore is the first placement whose legality is a water question
  rather than a ground one, and that rule has to be designed rather than discovered. Water-based harvestable
  nodes are row 9's population model in another medium and should reuse it. And a vessel needs somewhere worth
  sailing to, which is row 12.
- **Underground strata.** Separate sparse axial strata joined by explicit shafts or elevators, not a voxel world
  and not an automatic consequence of adding floors.

Whatever comes next, `fixtures/balance.json` remains the thing every new building or recipe has to face: a
definition that never reaches it is a definition nothing has compared against the curve.

## Shipped ledger

An index, newest first — one line per release, and the envelope numbers only where one moved. The reasoning
behind any shipped rule is in the git history and in the code that implements it.

- **v0.46.0** Shaped Ground — earthworks named by a shape and two anchors, with a depth, a levelling datum, a
  64-hex ceiling, and a refused edit that keeps its footprint and names the obstructing hex.
- **v0.45.0** Sealed Routes — pipes carry loose fluid through the compiled graph, belts carry solids and sealed
  barrels; single-fluid tanks and two-portal underpasses. Save 36 / definitions 27.
- **v0.44.0** Emblems and Clarity — one emblem library over every machine, category and branch, tiers as UI
  badges, catalogue search.
- **v0.43.0** Closer Views and Field Survey — twelve 30-degree orbit stops, 4× zoom, Field Survey as a third
  skill. Save 34 / technologies 15.
- **v0.42.0** Straight Walls and Yards — boundaries on the vertex lattice; twelve headings run dead straight and
  a yard closes from two corners. Save 33 / wire 18.
- **v0.41.0** Handling and Clarity — pointer stack dragging, named belt-target refusals, carry-and-spill
  demolition, continuous world-space paving.
- **v0.40.0** Petroleum Roads — powered oil wells, atomic joint-output refining, asphalt over gravel,
  production-route accounting. Save 32 / definitions 26 / technologies 14 / world 10.
- **v0.39.0** Masonry Enclosures — hill limestone, kiln-fired cement, four wall materials. Save 31 /
  definitions 25 / technologies 13 / world 9.
- **v0.38.0** Ground Works — five paved surfaces and native integer elevation in one bounded transaction, with a
  spoil ledger, paid recovery and undo. Save 30 / definitions 24 / wire 17.
- **v0.37.0** Timber Boundaries — edge fences and gates with bounded selection and atomic accounting. Save 29 /
  definitions 23 / wire 16.
- **v0.36.0** Player Skills — separate personal points and three finite native milestones. Save 28 /
  technologies 12 / wire 15.
- **v0.35.0** Practical Projects — hub demand becomes finite: 22 projects each pay once. Save 27 /
  definitions 22 / wire 14.
- **v0.34.0** Power and Tier Bills — no buildable definition bills raw ore. Save 26 / definitions 21.
- **v0.33.0** Mechanical Components — a plate-and-gear component and a one-component founding commission.
  Save 25 / definitions 20 / scenarios 7.
- **v0.32.0** Industrial Bills — five industrial stations repriced in manufactured parts. Save 24 /
  definitions 19.
- **v0.31.0** Foundation Commissions — the first founding stage grants four technologies with typed effects.
  Save 23 / technologies 11 / scenarios 6.
- **v0.30.0** Research Atlas — a central technology tree with prerequisite lines, search, filters and keyboard
  navigation. Save 22 / technologies 10.
- **v0.29.0** Research Foundations — branch and stage registries; native publishes the availability answer.
  Save 21 / technologies 9.
- **v0.28.0** Essential Bills — five starter buildings billed in manufactured parts. Save 20 / definitions 18.
- **v0.27.0** Transport Kits — belting is manufactured, not gathered. Save 19 / definitions 17.
- **v0.26.0** Primitive Workshops — a furnace that smelts with no grid, and native attended work. Save 18 /
  definitions 16.
- **v0.25.3** Compartment Storage — separate ingredient, fuel and output inventories with a native cursor-held
  stack. Save 17 / definitions 15 / wire 12.
- **v0.25.2** Wayfinding — a second click walks the player there: goal saved, bounded A\* route derived.
  Save 15 / wire 10.
- **v0.25.1** Junctions — split, merge and underpass as definition flags over the existing graph. Save 14 /
  definitions 14 / technologies 7 / wire 9.
- **v0.25** Visual Depth — the production world becomes a near-orthographic Three.js diorama with instanced
  machines from the shape grammar.
- **v0.24** Creative Mode — a saved, checksummed sandbox flag that changes no rate, price or payout. Save 13 /
  definitions 13 / wire 8.
- **v0.23** Earned Insight — hand insight becomes a bounded first-discovery allowance. Save 11 /
  definitions 12 / wire 6.
- **v0.22** Crossings and Canopy — bridges, a canopy that answers harvesting, and the twelve-point rosette.
  Definitions 11 / technologies 6 / wire 6.
- **World scale** (2026-08-20, superseded by Phase 8) — one hexagon is 1 m²; landform cells 128–960.
  Generator 8.
- **v0.21** Landforms and Fields — **a deposit is a site, not a hex**; a bootstrap pass guarantees the opening
  or refuses the world. Radius-1 purity rose from 474–662 to 965–992. Generator 7.
- **WebGL2 renderer** (unreleased, superseded by v0.25) — world and minimap as instanced GPU geometry.
- **v0.20.1** Panels and Item Language — `itemChip` becomes the only place an item is drawn.
- **v0.20** Standing Requests — filling a posted hub request is the only thing that pays. Save 10 / wire 5.
- **v0.19** Power Grid — electricity becomes energy bought per unit of work. Save 9 / technologies 5 / wire 4.
- **v0.18** Founding Contract — an ordered contract that grows the hub, and guidance derived rather than
  scripted. Save 8 / scenarios 5 / wire 3.
- **v0.17** Balance — `fixtures/balance.json` becomes every figure that decides whether the economy works.
- **v0.16** World Parameters — a world is a seed **and** a `WorldParams`; four presets ship as data rows.
  Generator 6.
- **v0.15** Generated Shapes — a building's drawing is a part list, total over `SilhouetteKey`.
- **v0.14.1** Construction Catalogue — buildings grouped by kind, recipes drawn as materials, a nine-slot hotbar.
- **v0.14** Upgrades and Tiers — `upgrade` edits the entity in place. Save 7 / technologies 4.
- **v0.13.2** Inspector Readability — a clicked hex becomes cards rather than a text dump.
- **v0.13.1** Look Systems — neighbour fringes, baked terrain tiles, depletion scars, category silhouettes.
- **v0.13** Power — poles compile connected components into networks with exact brownout remainders. Save 6 /
  technologies 3.
- **v0.12.4** Renderer Measure — the first complete browser frame.
- **v0.12.3** Sightlines — the player faces the cursor through a bounded `aim` carrying a world position.
- **v0.12.2** Binary Delta — a transferred buffer decoding to exactly what JSON produced: payload 13.6× smaller.
- **v0.12.1** Playtest Feel — sparser fields; refusals that name the missing item. Generator 5.
- **v0.12** Material Base — resources generated where their geography says, and **fuel as a property of the
  item**. Save 5 / generator 4.
- **v0.11** World Shape — resource fields as a pure function of seed and hex, with only a sparse depletion
  overlay stored. Generator 3.
- **v0.10** Playability — one placement overlap rule, host lists patched in place, a native player cadence.
- **v0.9** Game Feel — a belt run becomes one drag, resolved natively.
- **v0.8** Browser Capacity — the ladder runs in the browser worker; the worker boundary made the binary
  encoding the next milestone.
- **v0.7** Sparse Snapshot — deltas built from dirty marks; frame cost fell 16.8×.
- **v0.6** Sparse Cost — extractors hold a resolved deposit reference (tick 233× cheaper); fog of war.
- **v0.5.1** Capacity Tiers — the deterministic headless ladder, 12 to 6,144 buildings.
- **v0.5** Worker Boundary — the Wasm `Factory` moved into a dedicated module worker.
- **v0.4** Command Surface — the world owns the viewport; derived guidance and a construction dock.
- **v0.3.1** Transport Graph — stable-ID invalidation and affected weak-component recompilation.
- **v0.3** Continuous Exploration — native two-axis intent with continuous collision.
- **v0.2** Playable Game — the architecture proof became a game, with `HXF1` saves and scenarios.
- **v0.1** Founding slice — the repository, `@hexlife/embed/hex` exactly pinned, and the first Pages deployment.
