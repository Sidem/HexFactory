# HexFactory — goal, state, and roadmap

This is the live document. It says what the game is for, what exists today, and what to build next.
Architecture decisions are in `docs/ARCHITECTURE.md`, the working invariants an agent must not break
are in `AGENTS.md`, the art rules are in `docs/ART.md`, and every performance claim is backed by
`docs/BENCHMARKS.md`.

## The goal

HexFactory is a game first: a beautiful, open-ended factory-automation game that is fun to play for
its own sake, fascinating to keep exploring, and a pleasure to control — drawing on Factorio's
automation depth, Satisfactory's sense of place and scale, and Minecraft's freedom to build what you
want where you want, expressed in hexagonal space rather than square.

The deterministic Rust/Wasm core, the sparse architecture, the compiled transport graph, and the
narrow `@hexlife/embed` geometry dependency are the means to that end, not the end itself. They
exist because a large, living world that never stutters and never loses a save is a player
experience before it is an engineering result. Where an architectural preference and the player's
experience genuinely conflict, the player's experience wins and the architecture has to find another
way to pay for it. That ordering weakens no invariant — determinism, native ownership of the tick,
sparse cost, and measured-before-claimed all stay non-negotiable. What changes is _why_ they are
there, and therefore how milestones are chosen: engineering work earns its place by naming the
player-visible thing it enables.

The design intent is inspiration, never imitation. Original neutral shapes, names, and systems only.
This is permanent and is not a scope item.

### Design pillars

- **Fun is a requirement, not a polish pass.** A release that is correct, fast, and joyless has not
  met its acceptance criteria. Every milestone states what it makes better to play.
- **Controls must be obvious in the first minute and precise in the hundredth hour.** A control that
  needs explaining is a defect in the control.
- **The player should always know what just happened and what to try next.** Feedback for gathering,
  placement, blockage, depletion, research, and delivery is part of the mechanic, not decoration.
- **The world should reward looking at it.** Readability first — resources, machine identity,
  direction, throughput, and blockage legible at a glance — and beauty close behind it.
- **Open-ended, not aimless.** Progression opens options rather than prescribing a route. Victory is
  a milestone in a longer game, never a wall; visible hub programmes, regional discoveries, and
  consequences give that longer game reasons without turning it into a script.
- **The world and the factory answer each other.** Terrain is more than a placement mask and the
  factory is more than an overlay. Geography, living populations, extraction, waste, and recovery
  change one another in ways the player can see and choose around.
- **Hexagonal space earns its place.** A player-facing system becomes hex-native only when faces,
  rings, fronts, or multiple approach directions change a legible decision. Never force the factory
  into a cellular-automaton kernel or add invisible adjacency bonuses merely to justify the grid.
- **Nothing may stutter.** Frame stability, instant response to input, and saves that always restore
  exactly are player-experience features. This is what the measured capacity ladder protects.

## Where the project stands

The engine arc, the generator arc, and the shipped milestones through **v0.43.0 Closer Views and Field Survey** are
present in this tree. A run today looks like this: land beside a hub in a world chosen by preset
or by raw parameters, walk out under fog across rivers and coastline — on the keys, or by
**clicking a selected hex a second time** and watching the route native found — find **fields** of
ten raw
materials rather than scattered cells, cross rivers on **bridges**, gather from forests that visibly
thin and regrow, fill the hub's posted **requests** for insight and
its staged founding **contract** for hub growth — completing Prove the line grants belts, storage,
extractors and on-site power — research the remaining technologies, and build a
powered, automated line of buildings and nineteen recipes across industrial and primitive stations — including
belt lines that **split**, **merge**, climb the two-row period on the same belt definition once it is
researched, and **pass under** the lanes they cross. Fence a yard, or research Fired Masonry and
raise brick and concrete walls — **straight across hexes now**, anchored on lattice vertices, so a
run holds its heading and a **rectangular yard** closes in one drag. Refine oil into bitumen and
useful fuel, then mix asphalt and lay
fast roads over gravel in the Ground works tray. The tray also grades and recovers paid layers, and
**paves a rectangle**: drag two corners and every hex the rectangle touches is taken in.
Buildings are generated as low-poly instanced geometry from the shape grammar, so a tier remains a
data row. Power is energy bought per unit of work. The world renders through Three.js and the
minimap remains WebGL2.

**Current envelope numbers** — native refuses a load on all five, and the browser's named-save
catalog shows which one moved rather than hiding the row:

| Envelope              | Version |
| --------------------- | ------: |
| `HXF1` save           |      33 |
| Definitions           |      26 |
| Technologies          |      14 |
| Scenarios             |       7 |
| World generator       |      10 |
| Wire (snapshot delta) |      18 |

**Current measured capacity.** The v0.43 audit puts the complete 6,144-entity Three.js browser frame
at 32.3% of 60 Hz on Low, 33.5% on Medium, and 33.9% on High on the reference desktop at
1440×900/DPR 1. All three pass the 35% gate, but only by 1.1–2.7 percentage points. The native
current-build frame is 1.37 ms at the same tier and the tick itself is 0.287 ms.
Generation costs at most 1.42 µs per hex on the v0.21 site lattice, against 0.52 µs for the model it
replaced on the same harness. **The reference desktop is the support target**, decided 2026-08-27:
integrated-GPU laptops are no longer a supported configuration, and the Iris Xe / AMD Vega-class
qualification run that was outstanding is withdrawn rather than pending. See
`docs/BENCHMARKS.md`; no claim beyond a recorded tier or machine is supported.

**The shipped ledger is at the bottom of this document**, one line per release. Read it for what
exists; read the section a milestone names when you need the reasoning behind a rule you are about
to change.

**Latest delivery: v0.43.0 Closer Views and Field Survey.** The camera turns in twelve 30-degree stops and zooms in far enough to read one machine, and Field Survey is a third one-point skill that opens a second ring of chunks around wherever you walk. It rides on Phase 6, which v0.42.0 completed; no later phase was started. See [the release record](CLOSER-VIEWS-RECORD.md) for verification and limits. Supported floors and vertical transport are next.

### Current assessment — 2026-08-29

The game is a strong, unusually trustworthy factory-game foundation and a polished short-form
vertical slice. It is not yet a deep open-ended game: the first two hub stages and 27 finite projects
give the present roster a reason to exist, but an established factory has no programme after the
foundry module beyond self-directed building. The later Living Lattice, primitive-human and Regional
Discovery phases are the planned answer; do not invent repeatable filler quests in the meantime.

**Development and retrieval.** Native ownership, the worker boundary, data-defined catalogues,
cross-language fixtures, deterministic saves and headless balance/capacity harnesses are the right
architecture. The task-first generated map and `rg` routine make most changes cheap to localise. The
concentration behind that map is now the limiting factor: `factory-wasm/src/lib.rs` is 24,192 lines
(production ends near line 13,960 and the rest is mostly inline tests), `src/main.ts` is 5,482 lines,
and production `lib.rs` contains 110 `BuildingKind` references. The suite is strong — 239 Rust tests
and 292 TypeScript cases at this review — but three browser test files also pin important wiring by
source inspection rather than by composed interaction. The map routes added in this audit cover
ground works, skills, petroleum, scenarios, saves and the camera; the big files still cost an agent
more context and wider regression reasoning than their behaviours require.

**Compute.** The sparse wire, transferred deltas, cached fields, compiled graph indexes and instanced
renderer are effective. Constant 34–36 draw calls across 12 to 6,144 entities prove that entity count
does not own submission. This is efficient, not demonstrably optimal: every runtime-indexed machine,
power participant and transport source is still visited each tick, and power allocation constructs
ordered groups each time. The current desktop record fits, but the old v0.25 headroom no longer
describes this build. Phase 7 has to measure stacked floors and active lifts before adding permanent
scene buckets; source inspection alone does not authorize an active-set rewrite. The production
build also warns on its 816 kB minified main chunk (225 kB gzip), alongside a 1.22 MB Wasm artifact
(415 kB gzip). That is not a runtime failure or a measured loading problem, but startup has no stated
payload budget and should be measured before code-splitting is treated as necessary work.

**Player experience.** The diorama, generated machine language, resource fields, research atlas,
pack, exact costs, click-twice walking, gathering feedback and desktop/narrow layout are cohesive and
satisfying. The immediate weaknesses are clarity and scale management:

- the opening card says _Build a primitive furnace_ before the player owns its 6 stone + 4 clay,
  instead of naming the gather/search action that is actually possible;
- Mission control and the selected hub inspector can duplicate the same contract and request copy;
- the 30-definition construction catalogue is readable card by card but needs search before Phase 7
  adds another family;
- at 390×844 the dock remains usable, but off-screen tools need a visible horizontal-more affordance;
- the reviewed browser showed nine clearly diagnosed older saves and no loadable current save. The
  honesty is good; before the next envelope change, define the migration window a player can rely on.

The balance harness makes the two authored tasks reasonable on paper. Prove the line costs 24 hand
gathers plus 36.8 seconds of machine/player work with walking excluded; Raise the foundry module
costs 110 gathers plus 200.9 seconds after the first stage grants automation. That is a sensible
tutorial-to-factory escalation, and every requested product has a reachable chain. It is not a human
pace result: travel to guaranteed fields is deliberately 9–25 hexes in the opening, and the timed
playtest gate was withdrawn. Treat the task logic as validated and the lived pacing as a judgement.

**Resources, recipes and extraction.** The chains are legible compressions of reality: ore to plate,
plate to mechanisms and steel; wood to timber; clay and limestone through brick/cement into concrete;
crude through a joint-output refinery into bitumen, useful fuel and asphalt. Water on belts, unitless
oil and instant electrical adjacency are explicit abstractions. They are acceptable at this scale
because they create readable routing and backpressure without pretending to be fluid simulation;
pipes remain the later point where that bargain changes.

Keep the generic Extractor as the starter and keep its native component shared. Water already earns
a Pump and petroleum earns an Oil well because their placement, depletion and factory stories differ.
Do **not** add one machine per raw item merely for realism. Add a player-facing family only with a
distinct decision: a managed logging camp/forester belongs with Living Lattice and regrowth; a
quarry or mine head belongs with a later regional/depth system if overburden, footprint, waste or
grade changes play. Until then, recoloured synonyms would lengthen the catalogue without deepening
the factory.

## What to do next

Phases 1 to 6 are **shipped**, through v0.43.0. Read the ledger for what they delivered. The rest of the approved sequence follows in order.

| Order | Work                                    | Scope and dependency                                                                                                                                                                          |
| ----- | --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 7     | Supported floors and vertical transport | Support classes, the first upper floor, stairs, belt lifts and a layer view, standing on phase 3's grades. Needs the beams and concrete phase 3 and 4 produce.                                |
| 8     | Icon pass and integrated validation     | Generate and review the planned UI icon families once the visual contract is stable, then finish migration, accessibility and measured-performance acceptance across the whole workstream.    |
| 9     | Flowing water                           | Water becomes an entity that sits on the ground and runs downhill, instead of a terrain band. Reads the phase 3 grades; supersedes the old fluid-network and water-reshaping horizon entries. |
| 10    | Living Lattice                          | Animals, biomatter and waste as one ecological system. Reuses phase 4's joint-output costing. Brief below.                                                                                    |
| 11    | The primitive human                     | The player gains needs and attributes. Depends on 9 and 10 for a food supply worth automating, and revises the skills budget rather than sitting beside it.                                   |
| 12    | Regional Discovery                      | The play half of regional variation: survey tools, distant sites, outposts. Brief below.                                                                                                      |

These are delivery phases, not a single giant release, and a phase may ship as several versions.
Do not start a later row in parallel with an earlier one unless the user changes priority; an unmet
gate is resolved or brought back to the user rather than answered by switching rows. Necessary
fixes, measurements and shared prerequisites are part of delivering a row, not reasons to skip it.
Optional extensions stay optional: this sequence does not pull in underground strata, a full
physical fluid network, every chemical candidate or every future skill merely because a brief
mentions them.

**Phase 7 entry work from the current assessment, not a new phase:**

1. Make guidance name the first executable action and remove the initial hub/mission duplication;
   add construction search and a visible narrow-dock overflow cue before the catalogue grows.
2. Before level IDs widen native state, mechanically move the inline native tests and capacity
   harness out of `lib.rs`, then extract only the occupancy/placement/transport slices Phase 7 has to
   touch. Split the corresponding session/panel wiring out of `main.ts`. Preserve behavior,
   checksum, save and wire at each step; this is not authority for a rewrite.
3. State and test the supported save-migration window before the next envelope bump. A catalogue
   may keep diagnosing older files, but a player needs to know which recent release boundary is a
   promise rather than discovering it from a disabled Load button.
4. Add a deterministic stacked-floor/lift capacity tier and rerun Low, Medium and High before a
   floor release. The current 6,144-entity record is already near the desktop gate.

Rows 5, 6, 9 and 11 were added on 2026-08-28 at the user's direction. Their direction and priority
are approved; the costs, rates and tuning hypotheses in their briefs are not, and still need the
validation each brief names.

Masonry and the vertex lattice did not complete the enclosure work: roofs, rebar and steel frames
remain, and they attach to row 7, because they are a structural system and the shipped row 6 was a
geometry change.

Generated terrain height remains presentation-only; as of v0.38.0 the integer grade a player cuts or
fills is native, checksummed state that walking, routing and building legality all read. Gameplay is
otherwise two-dimensional: there are no floors above or below a hex, and no vertical transport.

## Phase 7 — Supported floors and vertical transport

A gated milestone after foundations, enclosure and native layer semantics. Start with ground plus
one usable upper floor; expand only after it is legible and measured. The purpose is a compact
factory, not a voxel building editor or a structural-collapse simulator.

**The destination**, stated 2026-08-27, is a genuinely multi-level building: machines processing
material on several floors, with belts moving that material both within a floor and between floors,
inside one structure the player reads as a single works. Ground plus one floor with a working lift is
the first shippable step. A machine occupying more than one level is a later extension of the same
level-ID model — it reserves its footprint on every level it stands on, exposes intake and output on
named levels, and never acquires implicit connections above or below.

- **Logical levels:** position becomes an axial cell plus an explicit level ID. A cell on floor 1 is
  not occupied by the machine at the same axial cell on floor 0. Foundation grade and floor index are
  distinct facts. Existing two-row corner belts remain planar; they are not vertical lifts.
- **Supports and loads:** definition-driven load classes and maximum spans. The preview states which
  floor cells need columns and which machines are too heavy. Recalculate changed support regions on
  edit, never all buildings every tick. Reject unsupported placement and the removal of a loaded
  support; no surprise collapse, and no inventory lost to one.
- **Floor openings:** stairs, lifts, columns and shafts reserve their full footprint and headroom
  across affected levels. An apparently empty cell cannot hide a conflicting shaft above it.
- **Belt lifts:** explicit intake/output endpoints join compiled graph edges across levels. Cargo,
  progress, buffers, direction, capacity, duration and energy demand stay native, with identical
  conservation and backpressure rules. A full destination leaves cargo at its source or in its
  reserved in-transit slot.
- **Failure and editing:** removing a loaded lift recovers its stock or refuses safely; a direction
  change cannot teleport or duplicate cargo. Test multi-output arbitration and save/load with cargo
  between floors. Existing underpasses must not acquire cross-level connections.
- **Player access:** stairs first, elevators optional and later. Walking, reach, gathering,
  construction and interaction all resolve the correct level — no reaching through a ceiling because
  the axial distance happens to be small.
- **Power and utilities:** adjacent axial positions on different floors do not connect implicitly.
  Define explicit risers; pipes adopt them when fluid networks exist.
- **Editing view:** active-floor selection, hide/fade above, ghosted context below, layer-aware
  selection and clearly marked shaft destinations. Picking intersects the selected logical plane and
  never derives authoritative height from a rendered mesh. Warnings and controls stay usable with
  roofs on, at ordinary zoom, on Low quality, and in narrow layouts.

**The structural half of the enclosure family lands here**, because it exists to carry a floor:

| Family                            | Ingredients             | Structural role                              |
| --------------------------------- | ----------------------- | -------------------------------------------- |
| Reinforced concrete wall / column | Concrete + rebar        | Heavy decks and taller supported stacks      |
| Steel frame and cladding          | Beams + plate or panels | Larger clear spans with explicit load limits |
| Roof                              | Per material            | Cosmetic first, with automatic cutaway       |

Higher floors, larger spans and heavier equipment are what should create demand for beams and rebar.
Do not require reinforced concrete for the player's first small upper room. A roof does not create a
walkable floor until the structural system exists, and keeps its appearance separate from movement
blocking and load bearing. Underground strata stay a separate decision; designing level IDs does not
commit the game to excavation.

**Acceptance.** A useful stacked factory with no hidden routing, load and removal validation, full
cargo conservation, and readability and performance evidence at the recorded tier. If one upper floor
cannot be edited confidently at normal zoom, fix the layer view before expanding structural rules,
floor count or scope.

## Phase 8 — Icon pass and integrated validation

v0.30.0 supplies original code-native SVG emblems for the current technologies. Broader image
families were deferred to here, once the visual contract is stable.

Define presentation-only manifest keys for branches, technologies, skills, materials, recipes and
buildings. A missing asset falls back to a generic emblem plus text, never a blank button or an
invalid definition. Fixed image boxes keep later artwork from shifting layout. Before generating,
agree silhouette, perspective, lighting, palette, framing, transparent background, safe area and
small-size readability, then review contact sheets of complete families. An upgrade retains a
recognisable base shape; branch accents and rank badges are UI overlays, not baked-in text. Use the
same vocabulary in research, construction and inventory. Asset paths and resolution are not gameplay
identity and never enter saves or checksums, and this UI library does not replace the
definition-driven world mesh generator in `ART.md`.

The validation half closes the workstream: migration, accessibility and measured performance
acceptance across every row above, and a re-measured capacity ladder wherever the entity or world
snapshot moved.

## Phase 9 — Flowing water

Asked for on 2026-08-28: water should stop being a property of a cell and become its own thing —
infinite sources that sit **on top of** a tile and run downhill into lower ones, in the spirit of
Minecraft's water. This supersedes the **Fluid networks** and **Water reshaping** entries on the
horizon list, which described the same ambition in weaker terms.

Today water is a terrain band. `ShallowWater` is a 1 m/s ford, deep water blocks, both are pinned by
`fixtures/terrain-passability.json`, a pump draws from the band and never depletes it, and the water
a factory moves is a belted item that says so. The change is that a water **level** becomes native,
saved, checksummed state layered over the grade, and passability becomes a question about that level
rather than about the band identity. Row 5's grades are what it runs on: water goes to the lower
grade, and a cut or a fill now floods or drains.

**Four constraints decide whether this can be built at all**, and they are not negotiable by the
implementation:

- **It is not a cellular automaton.** The architecture refuses a per-cell world kernel and this must
  not smuggle one in. Water advances as a sparse **active front**: only cells whose level changed
  this tick are scheduled, and a settled pond costs nothing. This is the same shape Living Lattice
  plans for populations, and the two should share it.
- **Spreading water may never generate a chunk.** `generated_chunks` is a checksum input, so water
  that ran off the surveyed edge and pulled new world into existence would make the world a function
  of the player's plumbing. Flow stops at the survey frontier. Say so in the model, not in a comment.
- **The ocean is not simulated.** `basin` holds a standing body of 7,360 hexes; running that as an
  entity front is exactly the per-cell cost the architecture exists to avoid. Standing water at or
  below sea level stays static and cheap. The entity model covers water the player creates,
  disturbs, or pumps — a spring, a channel, a flooded cut, a drained pond. That is the honest scope
  line and it should be written into the definition rather than discovered later.
- **Conservation is not claimed, but termination is.** An infinite source is infinite by definition,
  so this system does not conserve volume and must not pretend to. What it must guarantee is that
  every flow reaches a fixed point in bounded steps from any starting state — otherwise a player
  builds a perpetual front and the tick never settles. Minecraft's rule that two adjacent sources
  make a third is the specific thing to be careful with: on six neighbours it behaves differently
  than on four, and it is the rule that lets a player manufacture an ocean. Decide it deliberately,
  with the level count and the maximum spread distance both bounded and both stated.

What moves with it: `terrain_at` keeps its band, but movement legality, route cost, building
legality and the pump all read the water level instead of the band; the passability fixture and both
languages move together; the generator's oceans and rivers become **initial** water rather than
terrain identity, which is a world-generator version bump and a save migration; and the renderer
gains a surface that sits above the grade rather than a tinted prism.

**Acceptance.** A source placed on a slope reaches the same fixed point from a save/load in the
middle of the flow, and the checksum proves it. A flooded cut drains when reopened. No flow
generates a chunk, proven by a test that walks water to the frontier. The capacity ladder is
re-measured with a large active front, and the claim is the measured tier and nothing beyond it. The
pump draws from player-made water as readily as from a river, and says which it is drawing from.

## Phase 10 — Living Lattice

Formerly reserved for v0.26; its release number is unassigned. Its ecological scope is preserved.

Animals, biomatter, and waste remain one milestone, and the purpose is sharper than the roster: this
is the first system that makes HexFactory something other than a factory game drawn on hexes. A
living population moves, feeds, breeds, recovers, and can be depleted past recovery across hex
neighbourhoods. Biomatter comes from that population rather than from a renamed static field. Waste
is a byproduct with a visible destination: it can feed a recovery loop, damage a habitat, or be
refined. Producer, byproduct, and consumer are designed together so none is a decorative item.

This is **not** a pollution-and-enemy-wave substitute. The pressure is ecological consequence and
opportunity, not a timer that periodically sends attackers. A player should be able to preserve a
productive migration, intensify it carefully, exhaust it for an urgent contract, or repair a region
they damaged. The landscape answers the factory, and the answer is visible where it happens.

Hex topology earns itself here. Movement and propagation use six neighbours; a herd or recovery
front has a perimeter; extraction reach is a ring; machines expose meaningful faces when a process
has directional intake, output, heat, or waste. Do not add generic adjacency percentages that
collapse into one solved blueprint.

Rust/Wasm still owns every ecological tick. Use sparse scheduled populations or active fronts, not a
JavaScript cell loop and not HexLife source imports.

**Three things it should know before it starts**, all handed over by earlier milestones rather than
invented here. A fourth used to head this list — play and time the opening by hand first — and it
was withdrawn on 2026-08-27 with the rest of that debt; ecology no longer waits on it:

- **Reuse the recipe and joint-output foundation delivered by construction.** Today's
  `Economy::recipe_for` still asserts one recipe per item; the
  [petroleum row](#phase-4--petroleum-roads)
  owns replacing that assumption before refinery co-products ship. Ecology must reuse its
  named-route costing, multi-output stock handling, allocation rule, contract/request expansion
  and guidance rather than implement a second economy. Check that the allocation remains valid
  for ecological inputs and outputs; secondary outputs are not automatically free.
- **A new contract stage is a data row, and the hub already knows how to grow into it.** Stages live
  in `scenarios.json`; `HUB_LADDER` has one entry per stage the hub can finish, and
  `tests/look.test.ts` fails if a shipped contract can complete a stage the ladder cannot draw. If
  this milestone wants the hub to ask for biomatter, that is a stage and a ladder row, not a system.
- **Guidance follows the contract for free, but only through recipes.** `nextAction` walks recipe
  inputs and recipe categories. An ecological input _harvested from a population_ falls out of the
  walk as a raw material, which is right; an ecological _process_ with no recipe row will not appear
  at all. Give it a recipe row or teach the walk about it deliberately — the one thing that must not
  happen is a hub asking for something the next step cannot explain.

### Acceptance

- One complete loop produces useful biomatter and a waste stream, and the player has at least two
  legible responses to that waste with different ecological outcomes.
- The same installation in two habitat states does not have the same answer, and the reason is
  visible in the world rather than hidden in a modifier panel.
- A population can recover, migrate, and collapse deterministically; saves and checksums reproduce
  each outcome exactly.
- The founding hub asks for something from the loop, so the new system has a motive on arrival.
- Every new definition reaches `fixtures/balance.json`, and any ecological yield claim has a
  measured fixture analogous to the world survey.
- The native capacity ladder and complete browser frame are re-measured if the entity or world
  snapshot changes. No claim beyond the measured tier.

## Phase 11 — The primitive human

Asked for on 2026-08-28: the player should be a primitive human with their own needs and attributes
— strength, hunger, and the rest — rather than a camera with a pack.

**This reverses a standing guardrail, deliberately.** The progression brief said: _do not invent
endurance, hunger or a movement grind to fill empty branches._ That was written against padding —
inventing a need so that a thin skill tree would have something to sell. It was never an argument
that survival is the wrong genre, and the user has now asked for it as a designed system. The
guardrail survives in its real form, which is stricter than the old wording and applies to every
bullet below:

> A need must create a **reason to build** something. A need that only makes existing actions slower
> is a tax, and a tax is the failure mode the original guardrail was pointing at.

Hunger is the test case. Hunger that interrupts factory work to make the player walk to a food
stockpile is a tax. Hunger that makes a food chain — foraging, then farming, then cooking, then
preserving and storing — worth automating is a system, and it is the same loop the rest of the game
already rewards: notice a manual chore, build the thing that does it. Design every need to land on
that side of the line, and cut any need that cannot.

- **Attributes are bounded and legible.** Strength raises what the player can carry and how fast
  hand work goes; it does not become an invisible multiplier applied to every rate in the game. Each
  attribute states its exact effect and its ceiling, the same way a skill rank does.
- **There is one player-progression story, not two.** Skill Points already buy Carrying,
  Construction reach and — since v0.43.0 — Surveying range, and Carrying is exactly what a strength
  attribute would also touch. Reconcile them before implementing: either attributes replace those
  ranks, or attributes set the base that ranks modify, but the player must never face two currencies
  that buy the same bonus. `Surveying` is now a shipped branch rather than a reserved name, so a
  perception-style attribute has to answer to it; `Fieldcraft` is still free.
- **It is native state.** Hunger, condition and attributes are saved and checksummed, owned by the
  player clock that already runs on its own cadence independent of the simulation rate — so a need
  keeps advancing while the factory is paused, which is a decision to make explicitly rather than
  inherit.
- **It depends on rows 9 and 10.** A food supply worth automating needs the ecology milestone for
  animals and biomatter, and irrigation or drinking water needs row 9. Building needs before there
  is anything to satisfy them with produces exactly the grind the guardrail forbids. That dependency
  is the reason this row sits here and not earlier.
- **No death spiral, and no idle decay as a difficulty knob.** Failing to eat should narrow options
  visibly and recoverably. Whatever the failure state is, the player must be able to see it coming
  and act on it, per the third pillar.

**Acceptance.** Every need names the thing it makes worth building, and a playtest confirms a player
built that thing because of the need rather than in spite of it. No existing action becomes slower
than it is today without a stated compensation. Attributes and Skill Points have one reconciled
budget, with a migration that takes nothing away from an existing save. New state reaches
`fixtures/balance.json` and both test suites.

## Phase 12 — Regional Discovery

Formerly reserved for v0.27; its release number is unassigned. Construction's limestone and oil
access and their surveys arrive with their own rows; the broader regional discovery system is here.

**Its generation half moved forward into v0.21** — the bootstrap guarantee that replaces the sample
platter, and the survey that proves every preset still works. What remains here is the half that is
a play system rather than a generator, and it begins from a world that already has readable
landforms, rivers, and real deposits.

v0.16 made world shape parameterized; this makes that variation a play system. Advanced materials
and ecological opportunities belong to readable regions that require travel, surveying, and
eventually outposts. Every preset remains completable, but "completable" no longer means "sample
platter at spawn."

A third low-frequency generation channel may create dry and wet variants of the same elevation band,
but generation is not the milestone by itself. A region has to announce itself through shape,
colour, life, sound, and material behaviour; the player needs a survey tool that hints rather than
reveals the whole answer; and a distant discovery must create a reason to establish a specialized
site rather than carry one stack home and forget the place. Landing contracts and later hub modules
provide that reason.

Signal crystal is the strongest candidate for a later hex-native automation language: relays along
faces, triangular links, or closed rings can make spatial control distinct from conventional circuit
combinators. It stays a candidate until Living Lattice proves which signals the player actually
needs; do not build a programmable system in search of a problem.

### Acceptance

- Every preset remains completable, measured by an updated survey that reports first
  advanced-region distance, regional extent, and access from buildable ground.
- Crossing into a region is recognisable without opening the game menu or reading coordinates.
- At least one founding-hub project requires a sustained distant site, not a one-time hand trip.
- The minimap and home bearing support the expedition without revealing unsurveyed world or
  re-deriving native generation truth.

## Open decisions, each with what would settle it

- **Does `regrowth_ticks` move** now that a forest cell holds one to four wood instead of ten to
  twenty-two? (v0.23 — the shape change shipped and the rate change with it; subsequent tuning slowed the
  cadence fivefold, 90 to 450, so a cut forest reads as a place that has to recover rather than one
  that refills behind the axe. That was a judgement about pace, not a measurement: what still has
  _not_ been measured is an extractor's starve rate over seven cells against a `regrowth_ticks` of
  450, which is now the number that decides whether forestry is viable at all. The
  balance report's `mean_same_material` for wood is 5–11 units at the base reach and 11–26 at the
  deep one, which says forestry is a question of area, but says nothing about the cadence.)
- **Does the board lead to meaningful new research rather than repeat-income farming?** Settled in
  v0.35.0 by making demand finite: every project pays once and `repeat_insight` no longer exists, so
  there is no farm to run. The committed budget is whatever `fixtures/balance.json` currently
  measures — 706 project insight against 156 purchasable research insight as of the petroleum work,
  against 572/137 at v0.35.0 — so quote the fixture rather than a remembered pair. What would reopen
  it: playtest evidence that the surplus is loose enough that purchase order stops being a real
  choice. Repricing that budget needs measured play, which is why v0.35.0 did not attempt it.
- **Rails or free-floating panels** was settled by shipping the rail. What would reopen it: wanting
  positions the player chooses, at which point the rail becomes the docked default rather than the
  destination, and the saved-coordinate, overlap, off-screen-recovery, and touch-gesture questions
  all come back.

One decision that is **not** open and must not be reopened casually: `DIRECTIONS` stays six. Twelve
headings are transport only. Widening adjacency would let a boiler reach a turbine two rows away and
a pole span a distance no player can see.

Two gates were **withdrawn on 2026-08-27 by user decision** and are recorded here so neither returns
as a surprise: the timed human playtest of the opening, owed since v0.18 — the opening now stands on
`fixtures/balance.json` and the recorded before/after arithmetic, with no person's clock owed against
it — and physical integrated-GPU qualification, withdrawn with the laptop support target itself. What
the one informal session did find is shipped and stays shipped: players read an accidentally paused
factory as a failure, so player pause, single-step and variable speed are gone and the rate is fixed
at 10 tps.

## Longer horizon

Named as decisions rather than omissions, each with the thing it is waiting for. These follow
the active progression/construction workstream and the deferred milestones above; necessary
shared prerequisites do not bring their entire feature families forward.

- **Hub programmes.** Player-chosen modules grow around the landing hub's rings and create different
  material demands. Finite authored systems and visible construction, not endless random chores —
  what gives an established factory a reason to expand without turning one victory into a wall.
- **Six-face machines.** Ports, heat, exhaust, or control may attach to named faces where direction
  creates a readable routing choice. Closed loops and triads are available shapes, not mandatory
  bonuses on every machine.
- **Intermittent generation and accumulators.** They arrive together. Intermittency has to be a
  deterministic function of tick and position, never a runtime roll.
- **A day cycle, and solar with it.** A presentation and simulation change at once, chosen for what
  it does to the game's feel rather than smuggled in as a power source.
- **Pipes.** A pressure-and-flow transport model for water and oil, sharing the underpass arm the
  belt already uses — pipes inherit it for free when they land. This is what is left of the old
  fluid-networks entry once row 9 takes the water itself: row 9 makes water a thing in the world,
  and this makes it a thing a factory routes.
- **Organic tileables.** The later half of the art generator: systems that produce tileable textures
  and shapes so a hex lattice reads as organic terrain and organic objects. Row 5 takes the paved
  surfaces out of this entry early; what remains is terrain and organic objects. Same invariants —
  generated, presentation-only, derived from published snapshot facts, never a checksum input.
- **Underground strata.** Native surface elevation and the first supported upper floor now belong
  to the active construction sequence, with the Visual Depth evidence gates intact. Underground
  remains separate sparse axial strata joined by explicit shafts or elevators, not a voxel world
  or an automatic consequence of adding floors.

Whatever comes next, `fixtures/balance.json` remains the thing every new building or recipe has to
face: a definition that never reaches it is a definition nothing has compared against the curve, and
both test suites say so.

## Shipped ledger

One line per release, newest first. The reasoning behind a shipped rule lives in the git history of
this file and in the code that implements it; what follows is the index.

- **v0.43.0** Closer Views and Field Survey — The scene camera orbits in twelve 30-degree stops rather than six 60-degree ones, at the same turning rate, and zooms in to 4× so one machine and the hexes under it can be read. Field Survey is a third one-point skill: it opens a second ring of chunks around wherever the player reaches, derived from the purchased set rather than stored beside it, and paid out the moment it is learned. Save 34 / technologies 15; definitions, scenarios, world and wire unchanged, and a version-33 save simply gains the skill unlearned.
- **v0.42.0** Straight Walls and Yards — Boundaries move from canonical hex edges to the vertex lattice: a segment is a chord of one hex between two of its corners, twelve headings run dead straight, and off-heading runs staircase to the far end within half a hex. A rectangular yard closes from two picked corners, and Ground works takes the same two corners to pave every hex the rectangle touches. Every v0.37/v0.39 boundary loads in place through a `direction` alias with no migration pass. Save 33 / wire 18; definitions, technologies, scenarios and world unchanged.
- **v0.41.0** Handling and Clarity — Pointer stack dragging, automatic pack opening, named static belt-target refusals, confirmed carry-and-spill demolition, and continuous world-space paving. All envelopes unchanged; 625-cell Low rendering measurement committed.
- **v0.40.0** Petroleum Roads — Powered oil wells, atomic joint-output refining, refined fuel, asphalt over gravel, production-route accounting and petroleum research/projects. Save 32 / definitions 26 / technologies 14 / world 10; old worlds keep their site rules.
- **v0.39.0** Masonry Enclosures — Hill limestone, kiln-fired cement, corrected concrete, timber/wire/brick/concrete walls, and an enclosure tray on the Ground works pattern. Fired Masonry is an 8-insight masonry-branch node. Save 31 / definitions 25 / technologies 13 / world 9.
- **v0.38.0** Ground Works — Five paved surfaces and native integer elevation in one bounded transaction: signed grade deltas capped at three steps, a spoil ledger that makes fill something you dug, walking and building refused across a step of more than two grades, deliberate confirmation before a surface seals a deposit, paid recovery on stripping, and per-transaction undo. Save 30 / definitions 24 / wire 17; no new item, recipe or research.
- **v0.37.0** Timber Boundaries — Timber edge fences and unpowered gates; bounded enclosure selection, native previews and atomic accounting, paid recovery and undo, walking/transport crossing protection, and instanced geometry. Save 29 / definitions 23 / wire 16.
- **v0.36.0** Player Skills — Separate personal points, carrying/reach purchases and three finite native milestones; accessible Skills modal; one-time legacy bonus migration and persistent Creative provenance. Save 28 / technologies 12 / wire 15.
- **v0.35.0** Practical Projects — Hub demand is finite: each of 22 projects pays once and retires, `repeat_insight` is deleted, and the whole catalogue is browsable and postable by name so a finite board cannot hide the route forward. Progress moved from the board slot onto the project, so passing a part-filled project keeps what was delivered. A native invariant pins the budget at 572 insight against 137 of research, 4.175× surplus, 73 of it hand-reachable. Save 27 / definitions 22 / wire 14.
- **v0.34.0** Power and Tier Bills — Hydro generator, deep extractor and deep container repriced in manufactured parts; no buildable definition bills raw ore, and the hydro generator no longer shares the boiler's bill. Gear and frame yields audited and unchanged. Startup accounting funds research at the board's repeat reward and charges the commission behind a granted technology. Save 26 / definitions 21 reprice only; existing stations refund the current bill.
- **v0.33.0** Mechanical Components — Plate-and-gear component, one-component founding commission, matched request repricing, legacy job/contribution migration, and a working timber demo. Save 25 / definitions 20 / scenarios 7.
- **v0.32.0** Industrial Bills — Reprices five industrial stations in plates, gears and mineral structure, with no brick bootstrap for the kiln. Startup reports use a valid construction order; guidance names missing construction suppliers. Save 24 / definitions 19 preserve active jobs, stock and checksums.
- **v0.31.0** Foundation Commissions — Completing the first founding stage grants belts, storage, extractors and on-site power. Those four technologies are grant-only, with typed effects replacing ad-hoc unlock and bonus fields. Save 23 advances technology 10 to 11 and scenario 5 to 6; a factory already past Prove the line receives the grants. Insight prices of later nodes are unchanged.
- **v0.30.0** Research Atlas — Large central technology tree with all 19 SVG emblems, prerequisite
  lines and selected ancestry, hover/focus details, deliberate purchases, search, discipline and
  in-reach filters, pan/zoom and keyboard navigation. Four independent starting technologies
  replace the universal logistics gate. Save 22 / technologies 10 migrate existing knowledge intact.
- **v0.29.0** Research Foundations — Branch/stage registries classify all 19 technologies; the keyed list shows discipline, stage, costs, blockers and corner-heading benefits. Native publishes the availability answer that atomic purchases use, and guidance consumes it. Prices, prerequisites and effects unchanged. Save 21 / technologies 9.
- **v0.28.0** Essential Bills — Extractor, composer, container, pole and burner generator billed in manufactured parts; iron wire is new and keeps the first grid off copper; signal crystal leaves ordinary assembly. First power rises from 17 gathers to 36 in exchange. Save 20 / definitions 18; a legacy station refunds the new bill once.
- **v0.27.0** Transport Kits — Belting is manufactured, not gathered: one plate and one timber make four kits, one kit per belt and two for a corner. A hundred-segment line falls from 108 gathers to 103; a twenty-four segment line rises from 32 to 46 because the first run pays for the workshop behind it. Save 19 / definitions 17.
- **v0.26.0** Primitive Workshops — A stone-and-clay furnace smelts plates with no grid, and a wood-and-stone workshop makes a restricted recipe set with native attended work, one batch per press. Walking or gathering pauses it; jobs survive save/load; dismantling refunds reserved ingredients. Save 18 / definitions 16.
- **v0.25.3** Compartment Storage — Machines keep independent ingredient, fuel and bounded output inventories, presented as item-slot grids beside the pack. A native cursor-held stack supports full, half, single and Shift moves without making the host authoritative. Dropped and demolished cargo persists visibly for one minute and can be collected. Save 17 / definitions 15 / wire 12.
- **v0.25.2** Wayfinding — A second click on a selected hex walks the player there. The goal joins `PlayerState` — saved, checksummed, resumed — because a destination is a standing order the simulation executes. The route is derived and never saved: a bounded A\* replanned by `rebuild_runtime_index`, costing shallow water at five so the answer is the fastest way rather than the shortest. The host sends a destination and never a route. Save 15 / wire 10.
- **v0.25.1** Junctions — Belt lines split, merge and cross as definition flags over the existing graph and tick, with both cursors saved so a restored factory keeps its rotation. The underpass is the tunnel arm the horizon list had costed; crossed cells stay singly occupied and connected to their own lane. The riser is gone: `OrientationAxis::Any` puts both periods on the belt, so rotation walks all twelve headings. Save 14 / definitions 14 / technologies 7 / wire 9.

- **v0.25** Visual Depth — The production world is a stylized near-orthographic Three.js diorama
  with six 60-degree orbits, bounded zoom, logical-plane picking, exact pointy-top terrain prisms,
  presentation-only terrain height, native-survey fog, and generated instanced machines built from
  `ShapePart`, `TIER_LADDER`, and `HUB_LADDER`. Native drag paths, twelve headings, overlays, all
  commands, reduced motion, and context recovery retain parity. Three graphics profiles are
  recorded through 6,144 entities with 14–16 draw calls on the reference desktop; physical
  integrated-GPU qualification remains external. No save, definition, technology, scenario, world,
  wire, or checksum protocol moved.

- **v0.24** Creative Mode — A native, saved, checksummed sandbox flag unlocks every technology and
  makes construction, upgrades, and erasure free without changing power, fuel, machine timing,
  transport, or hub payouts. The host adds material and pack controls that still send bounded
  commands and read their results from snapshots. Save 13, definitions 13, wire 8.

- **v0.23** Earned Insight — Hand insight is a bounded first-discovery allowance; repeat funding
  comes from individually completable hub requests priced by recipe depth. Gathering time is a
  property of the material, machines accept hand-fed input and fuel, and run checkpoints measure
  the opening automatically. Save 11, definitions 12, wire 6.

- **v0.22** Crossings and Canopy — A bridge is a stone-and-timber support entity on shallow water;
  belts and risers may share its cell, and both layers survive a save while deep water remains a
  barrier. Forest cells draw one deterministic tree per remaining wood unit, so harvesting and
  regrowth change the canopy. Extractor, pump, pole supply, pole link, and the hand's native-published
  reach are visible as meaning-specific overlays and catalogue figures. Transport is a twelve-point
  rosette: the six unchanged adjacency edges plus all six corner headings, pinned in rotational
  order by one cross-language fixture; `DIRECTIONS` remains six. Definitions 11, technologies 6,
  wire 6.

- **World scale** (2026-08-20) — One hexagon is 1 m². The walk is 3 m/s; Shift runs at 5 m/s.
  Shallows are a 1 m/s ford; deep water still blocks. Landform cells moved from 5–20 to 128–960
  so a biome takes minutes to cross, rivers are 8–10 hexes thick, and oceans come from the coarse
  octave at last. A landing disc keeps the opening's bootstrap windows on a world that large.
  Generator 8.

- **v0.21** Landforms and Fields — **A deposit is a site, not a hex.** The world is partitioned by a `site_cell` lattice; each cell hashes to at most one site — a jittered centre, one weighted rule, one radius — and a hex belongs to the nearest site whose disc covers it and whose bands it satisfies. Yield falls from core to rim, row order stopped being a generation input, and the lattice is cached while the field never is. Rivers are ridge noise; beaches ask the coarse elevation octave alone. A `bootstrap` pass guarantees iron and forest within 14, coal, stone and clay within 25, copper within 40, and refuses a world it cannot satisfy. Radius-1 purity rose from 474–662 to 965–992 and every material gained a patch an extractor can stand on; generation costs 1.42 µs/hex against 0.52 for the model it replaced. Generator 7.

- **WebGL2 renderer** (unreleased, 2026-08-20; superseded by v0.25) — The world and the minimap drew as instanced GPU
  geometry with the camera as a uniform, so walking no longer restamps the terrain mosaic; a Canvas
  2D overlay keeps the player, labels, and machine decorations. Fixes the 4 tps sluggishness
  (per-frame fog blur, camera-keyed terrain restamp, layout forced by `pickWorld`, full HUD rebuilds
  while walking). Its final v0.24 hybrid baseline is preserved in `docs/BENCHMARKS.md`.
- **v0.20.1** Panels and Item Language — Host-only presentation pass. `itemChip` is the only place
  an item is ever drawn, replacing eight bespoke shapes; affordability is a per-line `CostLine[]`
  shortfall rather than a boolean; panels open independently in two `.panel-rail` flex columns; Take
  and Put are one `renderTransferRows` that moves a chosen quantity. No native, save, definition,
  generator, wire, or checksum movement.
- **v0.20** Standing Requests — `insight_value` is gone from every item — the hub posts a board of
  requests and filling one is the only thing that pays. Eligibility is `Core::item_reachable`
  walking the recipe tree; draw order is `request_rounds` with no randomness; every row carries a
  Pass. `hub_demand` makes the hub take only what it asked for, by belt as by hand. Named saves live
  in a version-independent catalog. Save 10, definitions 10, wire 5.
- **v0.19** Power Grid — Electricity became energy bought per unit of work, not a per-tick tax: a
  machine banks three cycles and idle time is free. Plants burn only against energy delivered.
  Coverage moved onto the pole (`supply_radius`, `pole_reach`, three rungs each) so it can be
  upgraded, and touching machines conduct. Coverage overlay and power meters. Save 9, definition 9,
  technology 5, wire 4.
- **v0.18** Founding Contract — The hub asks for an ordered contract rather than a delivered total,
  and finishing a stage visibly grows the hub through `HUB_LADDER`. The scripted guidance is gone —
  `src/core/guidance.ts` derives the next step from the contract, the recipe tree, and the
  technology graph, so it cannot recommend a factory the rules refuse. Permanent next-step chrome,
  progressive disclosure by technology distance, stall marks, six synthesised audio cues, `Shift`
  precision walk. Save 8, scenario 5, wire 3.
- **v0.17** Balance — `fixtures/balance.json` is every figure that decides whether the economy
  works, computed by `factory-wasm/src/balance.rs` and recomputed independently in
  `tests/balance.test.ts`. The curve is two rules over the data. Six numbers moved on the first run,
  each traceable to a printed figure — the hand beating the first extractor by 2.5×, steam dominated
  by hydro, an overpriced wind turbine, a break-even charcoal conversion, and a cutter cheaper than
  the smelter it follows. Definition 8.
- **v0.16** World Parameters — A world is a seed **and** a `WorldParams`, both saved and
  checksummed. Feature scale and threshold are separate axes. `field_at`'s match arms became an
  ordered `FieldRule` table. Four presets ship as data rows and `npm run survey` is where every
  claim about a landscape comes from — it found two unplayable presets before a player could.
  Generator 6.
- **v0.15** Generated Shapes — A building's drawing is a part list from an eight-part vocabulary in
  `src/rendering/shapeGrammar.ts`; `BUILDING_SHAPES` is total over `SilhouetteKey`, so a new
  silhouette is a compile error rather than a machine that draws nothing. A tier is a modifier on
  the list, motion is a part's `phase`, still parts bake behind `BUILDING_SHAPE_VERSION`.
  `contact.html` renders every definition × tier × status. Presentation only.
- **v0.14.1** Construction Catalogue — Buildings moved behind `B`, grouped by `kind`, each card
  carrying the facts that decide a choice. A recipe is drawn as materials, not a name in a dropdown.
  The bar became a nine-slot hotbar the player arranges, persisted as presentation state.
- **v0.14** Upgrades and Tiers — A tier is a data row validated once by `validate_upgrade_ladders`;
  `upgrade` edits the entity in place so contents, orientation, and connections survive. Extraction
  reach is the flagship upgrade. North and south entered `TRANSPORT_DIRECTIONS` as the riser — a
  direction-table row, not sub-hex occupancy. Right-click harvests one named hex; `store` mirrors
  `withdraw`. Save 7, definition 7, technology 4.
- **v0.13.2** Inspector Readability — A clicked hex is cards rather than a `textContent` dump:
  identity, coordinates chip, terrain swatch, field meter, facing compass, machine meters, belt
  cargo, Take rows. `Direction 0` no longer reaches the player. Presentation only.
- **v0.13.1** Look Systems — Stage B of the art generator: neighbour fringes, baked terrain tiles
  behind a version constant, host-side hash variation, depletion scars, silhouettes from
  `recipe_category`, plus the first Stage C motion pass. Re-measured at 19.0% of 60 Hz.
- **v0.13** Power — Poles compile connected components into networks with integer supply and demand;
  brownouts advance progress with an exact per-entity remainder. Burner, boiler-and-turbine, wind,
  and hydro. Water is a belted item, not a fluid network. Save 6, definition 6, technology 3.
- **v0.12.4** Renderer Measure — The first complete browser frame: 3,039 µs at 6,144 entities, 18.2%
  of 60 Hz, world 909 µs and minimap 160 µs. The unknown 89% of a frame is gone.
- **v0.12.3** Sightlines — The player faces the cursor through a bounded `aim` that carries a world
  position, never a heading. Panels behind `I` / `O` / `P`, leaving the inspector alone on the
  world. A minimap and a gold home-bearing marker. One hatched treatment for every impassable band,
  pinned by `fixtures/terrain-passability.json`.
- **v0.12.2** Binary Delta — The snapshot delta crosses as a compact binary buffer that is
  transferred rather than cloned, decoding to exactly what the JSON path produced. Payload 13.6×
  smaller, boundary 21.7× cheaper, host frame from 62.1% of 60 Hz to 11.0%. Pinned by
  `fixtures/snapshot-delta-wire.json`.
- **v0.12.1** Playtest Feel — The first-minutes follow-up: sparser fields so barren ground is the
  common case, one landing cell per material, smaller hexes, remaining amounts shown on demand
  rather than on every hex, refusals that name the missing item. Generator 5.
- **v0.12** Material Base — Eight raw resources generated where their geography says, and fourteen
  recipes across five machine categories on one `Composer` kind separated by `recipe_category`.
  **Fuel is a property of the item, never a recipe input.** Flora regrows from a derived set of cut
  cells. `Pump` is the only new kind; `set_recipe` is a new bounded command. Save 5, generator 4.
- **v0.11** World Shape — One axial lattice. Terrain bands from integer value noise, cliffs from the
  elevation gradient. Resource fields are a pure function of seed and hex with only a sparse
  depletion overlay stored. Extraction radius. Stage A art direction. Save 4, generator 3.
- **v0.10** Playability — One placement overlap rule for deposits and obstacles alike. Host lists
  carrying a control are patched in place, which is what was silently dropping research clicks. The
  player walks on a native cadence of its own. Carrying capacity as a rule over the inventory, with
  `withdraw` and an all-or-nothing refund. Save 3, definition 4.
- **v0.9** Game Feel — A belt run is one drag: `place_line` and `erase_line` are single bounded
  commands and Rust resolves the path, headings, legality, and cost. The preview comes from the same
  resolver. Undo, one rotation model, pick-block, nine hotbar slots, held gather, movement that
  stops when the key does.
- **v0.8** Browser Capacity — The capacity ladder runs in the browser worker from the same Rust
  implementation. Wasm costs 1.19–1.23× native; the worker boundary was 57–61% of a host frame at
  about 10 µs/KB, which is what made the binary encoding the next milestone.
- **v0.7** Sparse Snapshot — Deltas are built from dirty marks made where state is mutated, not by
  diffing two complete snapshots. Frame cost fell 16.8× at the largest tier and every tier fits
  inside 60 Hz.
- **v0.6** Sparse Cost — Extractors hold a resolved deposit reference instead of scanning every tile
  (tick 233× cheaper); the buildings delta became per-entity (2.3× less payload). Fog of war over
  the generated chunk set.
- **v0.5.1** Capacity Tiers — The deterministic headless capacity ladder and the first measured
  tiers, 12 to 6,144 buildings, excluded from the CI gate but pinned by a workload checksum.
- **v0.5** Worker Boundary — The Wasm `Factory` moved into a dedicated module worker with serialized
  RPC and revision-checked snapshot deltas.
- **v0.4** Command Surface — The world owns the viewport. Command bar, snapshot-derived next-action
  guidance, compact cargo and research surfaces, a lock- and cost-aware construction dock, and a
  narrow touch layout.
- **v0.3.1** Transport Graph — Stable-ID invalidation and affected weak-component recompilation
  replaced full post-edit rebuilds.
- **v0.3** Continuous Exploration — Hex-step movement became native two-axis intent with continuous
  collision and gathering, proximity-limited construction, and definition-driven rotated footprints.
- **v0.2** Playable Game — The architecture proof became a game: seeded deterministic world, native
  player and inventory, data-defined construction and recipes, a native technology tree, `HXF1`
  saves and scenarios.
- **v0.1** Founding slice — The repository, `@hexlife/embed/hex@1.15.0` published and exactly
  pinned, the native vertical slice (extractor → turning belt → composer → belt → container →
  consumer), and the first live Pages deployment.
