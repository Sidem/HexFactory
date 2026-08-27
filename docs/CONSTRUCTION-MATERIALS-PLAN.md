# Construction, materials, and an accessible recipe economy

Planning brief, 2026-08-27. **Approved workstream; the primitive foundation shipped in v0.26.0,
with the remaining mechanics and tuning still proposed.** Together with progression, this plan takes immediate priority over Living
Lattice, Regional Discovery and other roadmap features. The
[roadmap's combined sequence](HEXFACTORY-PLAN.md#what-to-do-next) is authoritative; release numbers
after v0.26.0 are unassigned and the old v0.26/v0.27 reservations for those deferred milestones are withdrawn.
Begin with the shared recipe/bootstrap audit. Pull required recipe infrastructure forward from
Living Lattice; native elevation and floors still require their own evidence and compatibility gates.

The companion [research, insight and player skills plan](PROGRESSION-PLAN.md) stages these branches
in a visual technology map, replaces unlimited delivery-funded research with finite practical
projects, and separates cargo/build-reach upgrades into a skill tree with its own points. It is
the progression design for this workstream; costs, unlocks and recipes must be developed together.

## What the player should gain

Delivery status: [Primitive Workshops](OPENING-FOUNDATION-RECORD.md) implements the recoverable
furnace/workshop capability, stock and attended-job foundation, with initial rates and regression
coverage. Slice 1 is not complete: essential bills, belt kits, commissions and timed standard
opening comparisons remain. The diagnosis below records the pre-v0.26.0 starting point.

Turn a clearing into a place: compact a footpath, lay a gravel yard, pave a fast route between
outposts, enclose a workshop, and eventually stack production floors with explicit belt lifts.
Choose materials for their availability, appearance, and job. An attractive timber workshop and
a brick factory should remain useful after steel and reinforced concrete become available.

The economy should explain what things are made of without making every useful object a research
project. Belts, storage, basic power, and the machinery that makes their ingredients must stay
accessible. More realistic recipes should create production choices, not a longer compulsory wait
before automation.

## Starting point and problems to solve

Checked against `src/data/definitions.json`, `src/data/technologies.json`,
`fixtures/balance.json`, and a successful `npm run balance` on 2026-08-27:

| Already present                                                                         | Current use or issue                                                                                                                                                      |
| --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Sand, stone, clay, wood, water, iron ore, copper ore, coal                              | Existing resource chains; water is transported as an item. Sand and water are not guaranteed starter materials.                                                           |
| Timber, gravel, brick, glass, iron plate, copper plate, copper wire, gear, frame, steel | Reuse these before adding near-duplicate materials.                                                                                                                       |
| Concrete                                                                                | Currently `2 gravel + 1 sand + 1 water -> 2 concrete`; there is no binder.                                                                                                |
| Belts                                                                                   | One iron ore per edge segment, two per corner-heading segment. Accessible, but no metal processing or mechanical construction is represented.                             |
| Component                                                                               | Currently `2 iron ore -> 1 component`; too vague for its foundational manufacturing role.                                                                                 |
| Extractor, composer, container, pole, burner generator                                  | Several construction bills consume raw ore directly. The composer also demands signal crystal.                                                                            |
| Smelter and kiln                                                                        | Both require grid power today. Simply changing generators and belts to plates would introduce bootstrap dependencies.                                                     |
| Recipe expansion                                                                        | Native `Economy::recipe_for` requires one recipe per output item. The TypeScript balance test also uses a single-producer map. Alternative recipes need explicit support. |

The recorded opening guarantees iron and wood at 9–14 walk distance, coal/stone/clay at 15–25,
and copper at 25–40. Do not casually make copper, shoreline sand, limestone, oil, steel, or signal
crystal prerequisites for a player's first belt line.

## Parallel construction paths

| Path                          | Inputs and production                               | What it is good for                                            | What keeps other paths useful                                                      |
| ----------------------------- | --------------------------------------------------- | -------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Earth and aggregate           | Local earthwork, stone, crushed gravel              | First paths, cheap yards, foundations and road sub-base        | Earliest access; no oil, cement, or steel dependency                               |
| Timber                        | Wood -> timber; small amounts of metal for fittings | Fences, gates, boardwalks, simple rooms and later light floors | Renewable supply and low setup cost; restricted loads and spans once floors exist  |
| Masonry                       | Clay -> brick; limestone -> binder; sand and water  | Pavers, brick walls, ordinary concrete foundations and slabs   | Durable industrial spaces without petroleum; local geology matters                 |
| Steel and reinforced concrete | Iron -> steel -> beams, mesh or rebar; concrete     | Open workshops, heavy floors, long spans, substantial walls    | Carries demanding installations; unnecessary for a basic enclosure                 |
| Petroleum and asphalt         | Oil -> bitumen; bitumen + aggregate -> asphalt      | Fast roads and large paved logistics yards                     | Movement and road-building specialization; asphalt is not a structural upper floor |

These are overlapping branches, not five compulsory ages. A player may build a useful gravel and
timber outpost without ever producing asphalt, or build a brick factory before pursuing oil.
Paving must not become a hidden machine-speed bonus, and walls must not be mandatory decoration
that every machine needs to function.

## Materials: keep, add, and defer

### Add for the first construction branches

| Material               | Proposed source                                       | Uses and limits                                                                                    |
| ---------------------- | ----------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Limestone              | Readable quarry sites with adjacent accessible ground | Cement and, if needed later, lime; a regional resource, not a first-belt gate                      |
| Cement                 | Limestone and clay through a heated cement process    | Binder for concrete and mortar. Abstract clinker grinding and minor additives initially.           |
| Mortar                 | Cement + sand + water                                 | Brick walls and masonry joints; direct ingredients may suffice until stockpiling mortar is useful  |
| Steel rebar            | Steel through shaping                                 | Reinforced concrete slabs, columns and walls                                                       |
| Steel beam             | Steel through shaping                                 | Columns, frames, long spans and industrial floors; separate from rebar only when their jobs differ |
| Iron wire / fence mesh | Shaped iron; later steel upgrade if useful            | Wire fences without spending electrical copper; a separate mesh item is optional                   |
| Crude oil              | Powered wells on surveyed deposits                    | Petroleum feedstock; not hand-gathered and not needed for starter machines                         |
| Bitumen                | Oil refining                                          | Asphalt binder; distinguish it from both crude oil and finished road material                      |
| Asphalt mix            | Bitumen + gravel + sand/fines, with process heat      | Ground paving; no fictional structural strength for elevated factories                             |
| Refined fuel           | Useful co-product of the chosen oil process           | Generator/boiler fuel and a reason to handle the other refinery output                             |

The useful real-world distinctions are simple: cement binds aggregate and water into concrete;
asphalt combines mineral aggregate with bituminous binder. Cement production uses processed
mineral feedstock, not gravel and water alone. See the
[American Cement Association](https://www.cement.org/cement-concrete/how-cement-is-made/) and
[EAPA](https://eapa.org/what-is-asphalt/). These support the material relationships, not game ratios.
Oil refining as the source of bitumen is described by
[Eurobitume](https://eurobitume.eu/bitumen-life-cycle-assessment/).

### Important candidates that should not all become mandatory items

- **Lime, clinker, and gypsum:** possible later binder specializations. Keep their operations or
  minor constituents abstract initially; do not make concrete depend on three new rare deposits.
- **Coke and slag:** later steelworks and recovery opportunities. Preserve the current simplified
  iron-plus-carbon route initially, with process fuel separate from carbon used as an ingredient.
  It represents manufacturing, not a literal chemical mass ratio; the basic relationship is
  supported by [worldsteel](https://worldsteel.org/about-steel/what-is-steel/).
- **Rubber, resin, and plant fibre:** candidates for advanced flexible belts, seals, insulation,
  roofing or biological alternatives. Starter transport can visibly use timber slats and metal
  fittings instead of demanding a petroleum rubber chain.
- **Fasteners and bearings:** useful manufacturing abstractions, but fold ordinary fasteners into
  kits initially. Promote them to items only when they support multiple meaningful demands.
- **Glass:** already exists; use for windows and rooflights. It must not gate an ordinary wall.
- **Excavated soil and demolition rubble:** add only when earthwork or recycling needs an explicit
  destination. Do not spawn hundreds of loose items from one brush stroke.
- **Plastic, sulfur, lubricant, and roofing products:** later oil consumers, not compulsory
  refinery output slots in the first release. No item without a useful consumer or bounded,
  visible handling route.

## Recipe reform without a bootstrap trap

### Establish a repeatable primitive manufacturing path

Recommended opening: gather local wood, stone, clay and iron; build a primitive furnace and a
small manual workshop; make plates, timber, and simple fittings; assemble several belt sections;
then build powered production. Neither primitive station requires its own output to construct.
No unique starting gift may be the only way to recover after dismantling equipment.

The primitive furnace burns ordinary available fuel and does not require grid power. The workshop
supports a small explicit set of cutting and simple assembly recipes, with native player work
time rather than free unattended production. It needs separate stock and job state, not the hub's
request-delivery sink. These capabilities now exist in v0.26.0, but the initial prices and rates
still need the full opening comparison before affected essential construction costs change.

Prefer the same recipe identity and quantities on primitive and industrial equipment, with
equipment capability and work rate distinguishing them. Do not create duplicate output recipes
merely to represent a slower workstation. Native must validate capabilities and own job progress,
reservation, cancellation, and refunds. Advanced electronics and oil processing are not hand jobs.

Research must follow this order too. Primitive processing and basic assembly cannot require
Automated Extraction, and Field Logistics cannot require delivered goods that need belts to make.
Rework the current prerequisite graph and opening requests together; there must be enough genuinely
reachable insight without grinding repeated raw requests. Guidance must show the furnace/workshop
step before asking for plate-based machines.

The [progression plan](PROGRESSION-PLAN.md#protect-the-opening-and-completion) proposes baseline
primitive knowledge and short foundation commissions that grant essential automation unlocks
directly. Do not replace the raw-ore recipe trap with an insight trap, or put both a commission and
the old paid prerequisite in front of the same essential capability.

### Proposed construction bills

Ingredient families below are design direction, not final quantities. Prefer two or three visible
ingredients for common construction; put depth into their production rather than enormous bills.

| Building or product                 | Proposed direction                                                              | Accessibility guard                                                                                      |
| ----------------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Primitive furnace / workshop        | Stone, clay, raw wood as appropriate                                            | No plates, gears, electricity, or product needed to build its own producer                               |
| Starter belt                        | A batch of transport kits made from iron plate + timber, with fittings included | No rubber, steel, copper, circuit, oil or signal crystal                                                 |
| Splitter / merger                   | Transport kits + gear                                                           | Same starter line with one understandable mechanical step                                                |
| Underpass                           | Transport kits + structural metal + stone                                       | Pay for the crossing mechanism; do not require petroleum                                                 |
| Small container                     | Timber; metal storage is an upgrade                                             | Cheap first stock buffer before a powered cutter exists                                                  |
| Extractor                           | Iron plate + gear + timber/frame                                                | All parts available at primitive stations before extraction automation                                   |
| Basic assembler/composer            | Iron plate + gear + frame                                                       | Remove signal crystal from ordinary assembly; reserve it for advanced controls                           |
| Basic generator                     | Plate + gear/frame + simple conductor                                           | Prototype with iron conductors; copper improves later electrical equipment instead of gating first power |
| Basic pole                          | Timber + simple conductor                                                       | No compulsory copper expedition before first power; art shows the conductor                              |
| Industrial furnace / kiln / crusher | Metal parts and stone/brick; steel only for later tiers                         | The primitive chain already makes the inputs; a kiln never requires brick as its sole bootstrap          |
| Component                           | Give it a clear mechanical role; plate + a fitting/gear input                   | Update the founding contract so the deeper recipe does not silently multiply its opening bill            |
| Concrete                            | Cement + sand + gravel + water                                                  | Oil-free; binder cost is priced, and useful supply is reachable at its unlock                            |
| Reinforced concrete construction    | Concrete + rebar at placement, or a precast kit if worth stocking               | No need for a separate reinforced-concrete commodity before prefab logistics justify it                  |
| Asphalt paving                      | Asphalt mix over a prepared base                                                | Paving is optional; essential construction has a non-oil route                                           |

The existing bridge's stone-and-timber bill is already readable. Keep good recipes. Copper plates,
wire, gears, and frames need tuning, not replacement just because the catalogue is being reviewed.

**Rows shipped in v0.28.0:** small container, extractor, basic assembler/composer, basic generator
and basic pole, each as this table proposed them. The generator's "simple conductor" and the pole's
became a new iron wire recipe (`1 iron plate -> 2 iron wire`), runnable at the manual workshop, so
neither the first grid nor the first assembler requires copper or signal crystal. The measured
consequence — including a first power that costs 36 gathers where it cost 17 — is in
[the essential bills record](ESSENTIAL-BILLS-RECORD.md). The industrial furnace/kiln/crusher row and
everything below it are still unshipped, so the second tier of stations has the credibility problem
this pass fixed for the first.

### Make the belt inexpensive by yield

**Starting hypothesis for testing:** `1 iron plate + 1 timber -> 4 starter transport kits`;
one kit buys one ordinary belt segment, and a corner-heading segment retains its two-unit charge.
This is an abstract bundle of material, not four full-sized belts made from one literal plate.
Keep batched outputs, raw inputs, fuel, player work, and industrial work visible in the balance report.

There is no buildable-item inventory today: buildings consume construction ingredients directly.
Transport kits would be an ordinary new item consumed by that existing placement model, not a second
inventory of serialized buildings. A drag should still be one native endpoints command, pay per
placed segment, and report the exact point where stock or legality stops it.

**Shipped and measured in v0.27.0.** The hypothesis went out unchanged. The 24- and 100-segment
comparison — stations, research, energy, fuel, machine time and manual handling — is recorded in
[the transport kit record](TRANSPORT-KIT-RECORD.md): a hundred segments fall from 108 gathers and
162.0 s of hand work to 103 and 129.2 s, while twenty-four rise from 32 gathers to 46 because the
first line now pays for the workshop and furnace behind it. The batch yield was not increased and
the primitive path was not shortened; the setup is two stations that need no research. Whether the
short-line crossover _feels_ right is still a human playtest question. The rest of this section
stands as the reasoning behind the shipped bill.

### Several production routes require honest costing

Parallel material branches can ship without alternative recipes for the same item. Introduce
alternatives only when geography, fuel, recovery, or scale gives them a useful tradeoff. Examples
to evaluate are quarried versus manufactured sand, direct versus recovered aggregate, and later
different carbon routes for steel. Manufactured sand would be an intentional inland supply option,
not an unnoticed removal of the reason to explore beaches.

Before adding any such recipe or refinery co-product:

- Replace the single-producer assumption in native balance, TypeScript tests, reachability,
  contract expansion, requests, and guidance. Unlocks must select a route that is actually usable.
- Report named routes separately with their machine, research, fuel, energy, and raw-material
  requirements. A default guidance route must be explicit, deterministic, and visible to the player;
  neither array order nor a cheapest theoretical route is sufficient.
- For joint outputs, report the whole batch and an explicit allocation rule whose shares sum to
  the whole cost. Do not declare the secondary output free or charge each output the entire batch.
- Reject unresolved dependency cycles; price recycling against an external supply of recoverable
  material. Requests must not fund an endless refine/recycle loop.
- Extend native recipe stock and completion to reserve all outputs atomically. If any required
  output has no room, the machine waits without consuming the next batch or deleting co-products.
- Keep process heat/fuel separate from material ingredients. Coal used as steel feedstock is an
  ingredient; fuel burned to run the process belongs in the existing fuel compartment.

## Preparing and paving the ground

### First delivery: surface treatment, with explicit limits

Add a sparse native surface overlay independent of deposits, buildings, cargo, and visual terrain
height. Initial treatments operate on currently legal dry ground. They can make it look prepared
and change walking speed, but cannot make impassable cliffs walkable, fill water, move deposits,
or claim to have physically levelled the landscape.

| Surface                       | Proposed role                                           | Initial speed hypothesis |
| ----------------------------- | ------------------------------------------------------- | ------------------------ |
| Untreated ground              | Existing baseline                                       | 100%                     |
| Compacted earth               | Cheapest path; player work with minimal material demand | 110%                     |
| Gravel                        | Easy early road and industrial yard                     | 120%                     |
| Timber decking / brick pavers | Distinct local material and visual choices              | 125%                     |
| Concrete                      | Factory slabs and good walking surface                  | 130%                     |
| Asphalt                       | Dedicated fast routes between frequently visited places | 150%                     |

These are tuning candidates, not measured improvements. Store integer movement factors; apply
them to native walking and running without increasing the simulation tick rate. Pathfinding must
price travel time so it can prefer a somewhat longer paved route. Test mixed surfaces, diagonals,
boundary crossing, collision at higher speeds, route invalidation after edits, and manual versus
click movement. The renderer does not decide which surface the player is standing on.

Give the player a bounded path/area tool with material choice, cost, affected cells, and reasons for
refusal. Resolve preview and commit in native with the same rules. Keep paths efficient to draw;
do not require a click on every cell. Partial construction must use a stable order and report
exactly what was built and charged. Repainting an identical surface must do nothing and cost nothing.

Paving is a deliberate land-use decision. The preview warns about deposits and occupied flora;
require explicit confirmation to cover a resource field. Covering suppresses access/regrowth
without turning the deposit into free harvested stock; removing the surface restores access to
the unchanged remaining deposit. Reject paving beneath an active extractor until it is relocated.
Record actual cleared vegetation separately from covered resources so removal does not duplicate it.
Living Lattice may later attach habitat consequences to sealed ground through its own native rules.

Surface removal, upgrades, and undo need stated recovery bills. Until a reclamation system exists,
use a documented deterministic refund rule; never promise full refund and rubble from the same
material. Changes under a building are allowed only when its support, footprint, and stock remain
valid. The tool must refuse an unsafe change without charging it.

### Later delivery: real levelling and earthworks

The user should eventually be able to choose a target grade and genuinely even the ground. This
requires **native integer elevation**, slope and retaining rules, and revised movement and building
legality. Do not infer it from Three.js terrain vertices. It follows the evidence gate in
[Visual Depth](VISUAL-DEPTH-PLAN.md#decision-after-v025).

Start with bounded cut/fill on dry land and foundations that provide level pads. A preview shows
excavation, required fill, final grade, blocked edges, and affected structures before applying work.
Excavated material either fills another selected cell or becomes accounted stock; fill cannot
appear from unlimited lowering/raising or undo. Define maximum grade changes and retaining support
before permitting cliffs. Deposits must not be created or duplicated by moving their surface.

Defer river diversion, seabed work, tunnels, and free-form voxels. Initially reject grading that
would invalidate buildings, strand the player, intersect water, or expose unsupported neighbouring
ground. Roads need explicit grade transitions; paving by itself never solves a vertical step.

## Fences, walls, gates, and roofs

| Family                            | Ingredients                            | First purpose                             | Later structural role                        |
| --------------------------------- | -------------------------------------- | ----------------------------------------- | -------------------------------------------- |
| Timber fence and gate             | Timber, small fittings allowance       | Mark areas and control crossings cheaply  | None; a fence is not a load-bearing wall     |
| Wire fence and gate               | Timber/metal posts + iron wire or mesh | See through an enclosure; readable routes | None                                         |
| Timber wall                       | Timber + fittings                      | First complete workshop                   | Light floors over limited spans              |
| Brick wall                        | Brick + mortar                         | Masonry factory and visual identity       | Defined load-bearing spans                   |
| Concrete wall                     | Concrete                               | Industrial enclosure                      | Support only where its definition permits    |
| Reinforced concrete wall / column | Concrete + rebar                       | Heavy structural construction             | Heavy decks and taller supported stacks      |
| Steel frame and cladding          | Beams + plate or other panels          | Open interior with visible columns        | Larger clear spans with explicit load limits |

Place boundaries on canonical hex edges, not as full-cell machines that waste a lane of floor area.
The same shared edge has one identity from either adjacent hex; six neighbours remain six.
Keep window/roof appearance separate from movement blocking and structural support.

Gates are explicit native crossing rules, with clear open/closed state and a manual unpowered
option. Belts and later pipes need planned openings or ports through solid walls; they do not clip
through them automatically. Preview any live connection a wall would block. Recompile affected
graph components and replan affected walking routes after construction or gate changes.

First walls provide enclosure and routing, not an invented enemy combat system. Animal exclusion,
sound, heat or pollution containment must wait for the corresponding simulation and state their
actual effect. Roofs may be cosmetic initially, with automatic cutaway so interior editing remains
clear. Roofs do not create walkable floors until the structural system exists.

## Multiple floors and vertical material transport

This is a separate, gated milestone after foundations, enclosure UX, and native layer semantics.
Start with ground plus one usable upper floor; expand only after it is legible and measured. The
purpose is a compact factory, not a voxel building editor or a structural-collapse simulator.

- **Logical levels:** extend position to an axial cell plus an explicit level ID. A cell on floor 1
  is not occupied by the machine at the same axial cell on floor 0. Foundation grade and floor index
  are distinct facts. Existing two-row corner belts remain planar; they are not vertical lifts.
- **Supports and loads:** use simple definition-driven load classes and maximum spans. The preview
  states which floor cells need columns and which machines are too heavy. Calculate changed support
  regions on edits, not all buildings every tick. Reject unsupported placement and removal of a
  loaded support; no surprise collapsing inventories in the first implementation.
- **Floor openings:** stairs, lifts, columns and shafts reserve their full footprint and headroom
  across affected levels. An apparently empty cell cannot hide a conflicting shaft above it.
- **Belt lifts:** explicit intake/output endpoints join compiled graph edges across levels. Cargo,
  progress, buffers, direction, capacity, travel duration and any energy demand remain native.
  Support up and down transport, with legible endpoints and identical conservation/backpressure
  rules. A full destination leaves cargo at its current source or reserved in-transit slot.
- **Failure and editing:** removing a loaded lift must recover its stock or refuse safely; changing
  direction cannot teleport or duplicate cargo. Test multi-output arbitration and save/load with
  cargo between floors. Existing underpasses must not acquire arbitrary cross-level connections.
- **Player access:** stairs are the initial reliable route, with optional elevators later. Walking,
  reach, gathering, construction and interactions must resolve the correct level; no reaching
  through a ceiling because axial distance happens to be small.
- **Power and utilities:** adjacent axial positions on different floors do not connect implicitly.
  Define explicit risers/connectors; pipes can adopt them when fluid networks exist.
- **Editing view:** active-floor selection, hide/fade above, ghosted context below, layer-aware
  selection, and clearly marked shaft destinations. Picking intersects the selected logical plane;
  it never derives authoritative height from a rendered mesh. Keep warnings and controls usable
  with roofs on, at ordinary zoom, on Low quality, and in narrow layouts.

Higher floors, larger spans and heavier equipment should create a reason for beams and rebar.
Do not require reinforced concrete for the player's first small upper room. Underground strata
remain a separate decision; designing level IDs does not commit the game to excavation gameplay.

## Oil logistics and meaningful side streams

Start oil with a useful end-to-end route: surveyed deposit -> well -> refinery -> bitumen -> asphalt
mixer -> roads, with refined fuel consumed by ordinary compatible energy equipment. Wells use
equipment built from already accessible metalwork; no oil-dependent part may be required to obtain
the first oil. The refinery and asphalt mixer should use recipe categories where their behaviour
fits existing processing; a well's source behaviour may justify a distinct native kind.

Choose the logistics scope before implementation. A first slice may use explicitly abstract item
units on belts, as water does today, and bounded native tanks. Do not show returning barrels unless
containers actually exist and are conserved. This does not claim to be a physical fluid network.
Pipes, pumps and pressure/flow rules are a later shared water/oil decision, not a prerequisite for
trying asphalt. Real networks need their own deterministic compiled transport model.

Initially keep the refinery output roster small: bitumen and one useful fuel stream. Show which
output is blocking work. Stopping extraction when storage fills is valid; silent deletion or an
infinite sink is not. Recovery, flaring, or waste treatment can arrive with explicit costs and
ecological consequences, but asphalt should not demand a dozen unrelated chemical factories.
No spoilage, cooling, or curing timers in the first material release unless testing shows they
improve construction; stored concrete/asphalt units are a stated game abstraction.

## Delivery sequence and acceptance

| Slice                          | Scope                                                                                                    | Must be demonstrated before shipping                                                                                                                                                                      |
| ------------------------------ | -------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1. Recipe and opening audit    | Primitive furnace/workshop, credible essential bills, cheap batched belts, revised research and guidance | A fresh player can repeatedly reach belts, storage, power, extraction and assembly without circular dependencies or distant materials. Compare timed opening playtests and recorded balance before/after. |
| 2. Paths and enclosure         | Compacted/gravel/timber paths, fences and gates, bounded surface tools                                   | Visible travel benefit; exact bills/refunds; native movement and route costs agree; edge occupancy and gate edits do not strand cargo or create phantom crossings. No real levelling claim yet.           |
| 3. Masonry and foundations     | Limestone, cement, corrected concrete, brick/concrete walls and prepared pads                            | A complete oil-free construction branch with reachable supply; distinct foundation and wall roles; all new items priced and explained. Pads are not native terrain elevation.                             |
| 4. Petroleum roads             | Well, small refinery, bitumen, asphalt, fuel consumer                                                    | Requires alternative/joint-output recipe work; whole chain runs, backs up safely and resumes; roads are attractive but not required to keep a factory operating.                                          |
| 5. Native earthworks           | Integer grades, cut/fill, retaining and slope rules                                                      | Visual Depth evidence gate met; conserved earthwork; preview parity; explicit save/wire migration; water and occupied-site exclusions are clear.                                                          |
| 6. Structural floors and lifts | Support classes, first upper floor, stairs, belt lifts, layer view                                       | Useful stacked factory with no hidden routing; load/removal validation; full cargo conservation; desktop and physical laptop readability/performance evidence.                                            |

Across these slices, use the [progression foundation](PROGRESSION-PLAN.md) for branch identities,
stages, prerequisite groups, project rewards, separate player skills and later icon references.
Adding material or construction nodes must also review the insight budget and guidance path.

Slices are planning units, not release promises; their priority is now set by the roadmap's
combined sequence. Masonry can proceed without oil. Earthworks and floors do not block the early
recipe correction, but remain part of this workstream before the older roadmap resumes. Implement
required multi-output costing here for later reuse by Living Lattice. Deliver useful limestone/oil
sites and their access measurements with construction; the broader Regional Discovery system and
ecological land-use consequences follow later. Optional extensions remain optional.

### Measurement and regression checklist

Extend existing tests and harnesses before inventing parallel ones:

- **Balance and accessibility:** native `balance.rs`, `tests/balance.test.ts`, definitions and
  guidance tests. Price full startup equipment, unlock costs, work, power, fuel and batch leftovers;
  compare first 24 belts, 100 belts, a small yard, a walled workshop, and a two-floor line. These are
  proposed workloads, not existing fixture coverage. Include recovery after dismantling stations.
- **World access:** extend `npm run survey` and landing/reachability tests for limestone and oil.
  Every preset and accepted custom world must retain a starter path and access to later required
  materials. New deposits cannot silently appear in an old seeded world; version the generator and
  choose a deliberate migration or an explicit incompatibility notice.
- **Native correctness:** extend movement, placement/drag, refunds, upgrades, graph/backpressure,
  save/replay, and dirty-delta tests. New state belongs in native saves/checksums; caches, support
  indexes, path searches and render meshes do not. Test the cached result against a full rebuild.
- **Compatibility:** recipe-cost changes must not make old buildings refund new expensive
  ingredients they never cost. Specify migration/recovery bills and in-progress craft handling.
  Preserve item and entity identities; version affected definitions, technologies, scenarios,
  saves, world and wire deliberately. Update Rust/TypeScript wire fixtures together; never encode
  layer-plus-coordinate identities into an unsafe JavaScript number.
- **Presentation:** preview/commit parity, protected deposits, wall apertures, active-floor picking,
  readable travel benefit, undo feedback and keyboard/touch controls. Run a real opening playtest;
  an arithmetic gather-time floor excludes walking, planning and placement.
- **Scale:** record native and complete-browser measurements for large static paved areas, wall
  perimeters, edited support regions and vertical networks. Static construction must not add a
  per-cell host tick or a full-world scan to each frame. Make no capacity claim before committing
  the relevant evidence in `BENCHMARKS.md`.

For this brief only, `npm run balance` was run against the unchanged catalogue. No new recipe,
movement factor, deposit guarantee, construction price, or floor capacity has been validated yet.

## Decisions to settle in the first prototype

1. Does a manual workshop plus primitive furnace make the first few minutes clearer and faster
   enough? Time it, including the trip for stone and clay; shorten the path if it does not.
2. Do transport kits improve bulk building, or is a simple direct plate/timber bill clearer?
   Compare equal lengths including refund behaviour and the cost of the extra inventory slot.
3. Is mortar worth stocking, and do beams and rebar already have distinct demand? Keep only the
   items whose production choices the player can explain.
4. Is asphalt's proposed speed worth an oil outpost while gravel remains pleasant to use? Compare
   journeys of several lengths and visually distinct routes; tune speed and yield together.
5. Can one upper floor be edited confidently at normal zoom? If not, fix the layer view before
   expanding structural rules, floor count, or underground scope.
