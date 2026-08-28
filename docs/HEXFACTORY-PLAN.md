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

The engine arc, the generator arc, and the shipped milestones through **v0.32.0 Industrial Bills** are
present in this tree. A run today looks like this: land beside a hub in a world chosen by preset
or by raw parameters, walk out under fog across rivers and coastline — on the keys, or by
**clicking a selected hex a second time** and watching the route native found — find **fields** of
eight raw
materials rather than scattered cells, cross rivers on **bridges**, gather from forests that visibly
thin and regrow, fill the hub's posted **requests** for insight and
its staged founding **contract** for hub growth — completing Prove the line grants belts, storage,
extractors and on-site power — research the remaining technologies, and build a
powered, automated line of buildings and fourteen recipes across five machine categories — including
belt lines that **split**, **merge**, climb the two-row period on the same belt definition once it is
researched, and **pass under** the lanes they cross.
Buildings are generated as low-poly instanced geometry from the shape grammar, so a tier remains a
data row. Power is energy bought per unit of work. The world renders through Three.js and the
minimap remains WebGL2.

**Current envelope numbers** — native refuses a load on all five, and the browser's named-save
catalog shows which one moved rather than hiding the row:

| Envelope              | Version |
| --------------------- | ------: |
| `HXF1` save           |      23 |
| Definitions           |      18 |
| Technologies          |      11 |
| Scenarios             |       6 |
| World generator       |       8 |
| Wire (snapshot delta) |      13 |

**Current measured capacity.** At 6,144 entities the complete Three.js browser frame is 27.4% of
60 Hz on Low, 25.1% on Medium, and 21.7% on High on the reference desktop at 1440×900/DPR 1.
Generation costs at most 1.42 µs per hex on the v0.21 site lattice, against 0.52 µs for the model it
replaced on the same harness. **The reference desktop is the support target**, decided 2026-08-27:
integrated-GPU laptops are no longer a supported configuration, and the Iris Xe / AMD Vega-class
qualification run that was outstanding is withdrawn rather than pending. See
`docs/BENCHMARKS.md`; no claim beyond a recorded tier or machine is supported.

**The shipped ledger is at the bottom of this document**, one line per release. Read it for what
exists; read the section a milestone names when you need the reasoning behind a rule you are about
to change.

## What to do next

**Priority decision, 2026-08-27: progression and construction come immediately next, before any
other roadmap feature.** The [progression plan](PROGRESSION-PLAN.md) and
[construction and materials plan](CONSTRUCTION-MATERIALS-PLAN.md) are the active workstream, not
unassigned longer-horizon ideas. Their direction and priority are approved; detailed costs,
movement factors and other tuning hypotheses still require their stated validation.

Work through this combined sequence, using the detailed acceptance gates in both plans:

| Order | Work                                                      | Dependency and scope                                                                                                                                                                                                                                |
| ----- | --------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | Opening, recipes and progression foundations              | Audit bills, rewards and prerequisites; build the repeatable primitive manufacturing path and expandable progression definitions. Keep essentials accessible. The timed opening measurement that used to head this row was withdrawn on 2026-08-27. |
| 2     | Research map, insight projects and separate player skills | Stage the discipline branches visually; replace repeat-income farming with reachable projects; migrate cargo/reach to skill points. Ship replacement income and guidance before reward cuts. Reserve icon slots.                                    |
| 3     | Construction materials, ground works and enclosures       | Timber, gravel, masonry, cement/concrete, useful shaped metal, fences, gates and walls — **and native integer elevation with bounded cut and fill**, delivered with the surface treatments. Use the new research branches.                          |
| 4     | Oil, bitumen and asphalt                                  | Deliver the useful petroleum-to-road chain. Pull shared alternative-recipe and joint-output costing/stock work forward here; do not wait for Living Lattice.                                                                                        |
| 5     | Supported floors and vertical transport                   | Support classes, the first upper floor, stairs, belt lifts and a layer view, standing on phase 3's grades. Multi-floor machines and belts that move material within and between floors are the intent this builds toward.                           |
| 6     | Icon pass and integrated validation                       | Generate/review the planned UI icon families after the visual contract is stable, and finish opening, migration, accessibility and measured-performance acceptance across the workstream.                                                           |

These are delivery phases, not a single giant release. Icons come later within this workstream,
not before the foundations. Explicitly optional extensions in either plan remain optional; this
priority does not pull in underground strata, full fluid simulation, every chemical candidate or
every future skill merely because the plans mention them.

**Priority raise, 2026-08-27: evening the ground moves up.** Native integer elevation and bounded
cut/fill were the back half of a phase 5 that also carried floors and lifts, and they were gated on
laptop evidence that no longer applies. They now ship inside phase 3, paired with the surface
treatments, because both own the same code: walking speed, pathfinding cost, route invalidation and
building legality get rewritten once instead of twice, and a road's grade transitions are designed
beside the grades themselves. Floors, stairs and belt lifts deliberately did **not** move with them —
they need the steel beams and concrete that phase 3 is still producing, so they become phase 5 on
their own. Elevation is a save, wire and building-legality migration; it is not a licence to start
the vertical factory early.

**Phase 1 delivery: v0.26.0 Primitive Workshops.** Baseline fuel-fired smelting and an attended
manual workshop now provide a repeatable local route to plates, timber, gears, frames and the
first component contract. Existing bills and insight rewards are unchanged. See
[the opening record](OPENING-FOUNDATION-RECORD.md) for arithmetic, tests and limits.

**Phase 1 delivery: v0.27.0 Transport Kits.** The construction plan's batching hypothesis —
`1 iron plate + 1 timber -> 4 starter transport kits`, one kit per ordinary belt and two for a
corner heading — is now shipped and measured. Belt, splitter, merger and underpass are billed in
kits; the underpass also pays structural metal and stone. See
[the transport kit record](TRANSPORT-KIT-RECORD.md) for the measured 24- and 100-segment
comparison, the refund-boundary argument, and what is still unpriced.

**Phase 1 delivery: v0.28.0 Essential Bills.** The extractor, composer, container, pole and burner
generator are billed in manufactured parts rather than raw ore, and a new iron wire recipe puts the
first grid within reach without a copper expedition. The signal crystal leaves ordinary assembly:
the first circuit needs none where it needed three. The opening gets longer in exchange — first
power rises from 17 gathers to 36, because a pole now has a furnace and a workshop standing behind
it. See [the essential bills record](ESSENTIAL-BILLS-RECORD.md) for the full comparison, the curve
argument, and the refund revaluation.

**User-directed UI delivery: v0.30.0 Research Atlas.** The current 19 technologies now have a
central modal map with four independent downward trees, icon-only nodes, lock markers, hover
details, explicit purchases and search/filter/zoom controls. Clicking opens a detail pane; the
optional list remains available for text navigation. This pulls the map forward at the
user's request without changing research prices or income; the unfinished Phase 1 economy work
below remains the next priority. See [the atlas record](RESEARCH-ATLAS-RECORD.md).

**Phase 1 delivery: v0.29.0 Research Foundations.** Validated branch/stage registries classify the
existing 19 technologies without changing their prices, prerequisites or effects. Research cards
are ordered by stage and discipline, and both cards and guidance consume the native purchase
availability answer. Save 21 preserves existing state and checksum; the save picker now recognizes
every released migration from save 14 onward. See [the research foundations record](RESEARCH-FOUNDATIONS-RECORD.md).

**Phase 1 delivery: v0.31.0 Foundation Commissions.** Completing Prove the line grants Field
Logistics, Automated Extraction, Storage Planning and On-site Power. Those four are no longer
insight purchases. Technology effects are a typed native list. Save 23 / technologies 11 /
scenarios 6 migrate a finished opening commission forward without refunding insight. See
[the commissions record](FOUNDATION-COMMISSIONS-RECORD.md).

**Phase 1 delivery: v0.32.0 Industrial Bills.** Smelter, kiln, cutter, crusher and pump now use
small manufactured-part bills. Startup accounting follows construction order instead of allowing
a machine to make its own bill, and guidance names missing primitive suppliers first. Save 24 /
definitions 19 preserve running jobs and state. See [the industrial bills record](INDUSTRIAL-BILLS-RECORD.md).

Phase 1 remains active: audit the component recipe and founding bill next, then the remaining
power/tier bills, gear/frame yields and complete commission/research startup accounting. The timed human opening comparison
that used to sit alongside them was **withdrawn on 2026-08-27 by user decision**; the opening's cost
now stands on `fixtures/balance.json` and the before/after arithmetic recorded in each release
record, with no person's clock owed against it. Do not treat these additive releases as completion
of either plan or move on to ground works, oil, or floors yet.

**Only afterward:** Living Lattice, then Regional Discovery, then the remaining longer horizon.
Their former v0.26/v0.27 reservations are withdrawn; assign new version numbers when release
boundaries are chosen. Do not start those milestones or unrelated roadmap features in parallel
with this workstream unless the user changes priority. Necessary fixes, measurements and shared
prerequisites are part of delivering this work, not reasons to skip it. An unmet gate must be
resolved or brought back to the user rather than silently switching to a deferred feature.

Today, visual terrain height remains presentation-only and gameplay is two-dimensional. The new
priority does not claim native elevation, new recipes or the new research economy already exist.

### Open decisions, each with what would settle it

- **Does `regrowth_ticks` move** now that a forest cell holds one to four wood instead of ten to
  twenty-two? (v0.23 — the shape change shipped and the rate change with it; subsequent tuning slowed the
  cadence fivefold, 90 to 450, so a cut forest reads as a place that has to recover rather than one
  that refills behind the axe. That was a judgement about pace, not a measurement: what still has
  _not_ been measured is an extractor's starve rate over seven cells against a `regrowth_ticks` of
  450, which is now the number that decides whether forestry is viable at all. The
  balance report's `mean_same_material` for wood is 5–11 units at the base reach and 11–26 at the
  deep one, which says forestry is a question of area, but says nothing about the cadence.)
- **Does the board lead to meaningful new research rather than repeat-income farming?** Native
  `next_request` already reserves a deepest-reachable slot, as checked on 2026-08-27. The remaining
  problem is the earning model and the reward-to-cost ratio; the
  [progression brief](PROGRESSION-PLAN.md#current-diagnosis-what-the-catalogue-actually-does) records
  the current catalogue and the proposed project model. A depth reservation alone does not cap income.
- **Rails or free-floating panels** was settled by shipping the rail. What would reopen it: wanting
  positions the player chooses, at which point the rail becomes the docked default rather than the
  destination, and the saved-coordinate, overlap, off-screen-recovery, and touch-gesture questions
  all come back.

One decision that is **not** open and must not be reopened casually: `DIRECTIONS` stays six. Twelve
headings are transport only. Widening adjacency would let a boiler reach a turbine two rows away and
a pole span a distance no player can see.

### Open, unassigned to a milestone

**Withdrawn on 2026-08-27 by user decision.** Recorded rather than deleted, so neither returns as a
surprise gate and nobody re-derives them from an older revision:

- _A timed keyboard-and-pointer playtest of the opening, done by a person._ Owed since v0.18 and
  never delivered. `fixtures/balance.json` predicts the material work (32 gathers to contract stage
  one, 97 to stage two, a 65-second combined hand floor) and says nothing about walking, choosing,
  or placing; a casual report on 2026-08-25 reached stage one at a reported 21:25.3 but loaded a
  save mid-run and recorded two checkpoints out of elapsed-time order. The opening is now taken as
  in order, and the same withdrawal covers the companion item that the opening had not been walked
  since the nearest guaranteed patch moved nine hexes out. What that session found is already
  shipped and stays shipped: players read an accidentally paused factory as a failure, so player
  pause, single-step and variable simulation speed were removed and the rate fixed at 10 tps.
- _Physical integrated-GPU qualification for Visual Depth._ Withdrawn along with the laptop support
  target itself. Low, Medium and High on the reference desktop are the whole performance claim, and
  no phase waits on an Iris Xe / AMD Vega-class run any more.

Still open:

- **Belts on field cells stay legal, but paving a crystal field without reading it first should
  not.** The [construction brief](CONSTRUCTION-MATERIALS-PLAN.md#preparing-and-paving-the-ground)
  assigns deposit warnings, explicit cover confirmation and conserved remaining deposits to the
  surface tool. This remains planned, not an existing protection.
- **The header `Establish component production 0 / 3` never explained the 3.** Largely answered by
  v0.18's contract bill and v0.20.1's item chips; confirm in a real session before closing it.

## Roadmap decision — the world the economy stands on

Decided 2026-08-19 from play, after Standing Requests v0.20 shipped. Two reviews ran in one session:
the research economy, and the world it is played on. The economy review produced five notes and the
world review produced one, and **the ordering between them is the decision**, because one depends on
the other.

The economy notes, verbatim from the session, are: fractional deposits into containers; insight that
requires processed goods rather than only what can be mined; resources that require a building to
extract, with hand-gathering rates that differ by material; extraction and effect radii that are
apparent on every building that has one; and the fact that the entire technology tree can be
unlocked by hand-mining alone. Every one of those was confirmed against the code and is written up
in **Earned Insight v0.23** below.

The world note is that resources arrive as scattered single cells of every kind at once, and it was
asked for in the same session: fields of iron and coal, forests instead of lone high-yield wood
hexes, rivers with clay on their banks and bridges to cross them, sand on ocean beaches, stone in
mountains, and a world where standing an extractor next to a deposit is worth doing. That was
**Landforms and Fields v0.21**, followed by the bridges and canopy treatment in v0.22; both have
shipped.

**The world work came first, and the dependency was real rather than tidy.** Making a hand gather
slower per material only reads as "go and build an extractor" when there is a field worth putting an
extractor on. Applied to the generator this replaced — where a continental survey found iron in 205
scattered cells and stone in 18, and where **stone had no workable patch anywhere in 26,307 land
hexes** — slower hand mining would not have been an incentive, it would have been tedium. So the
generator landed first, and the economy is now tuned against the world that exists.

**Regional Discovery is split, not deleted.** Its _generation_ half — a landing clearing that
guarantees a bootstrap path rather than a sample platter, and a survey that proves every preset still
works — was exactly what v0.21 had to do anyway for fields to mean anything, so it moved forward into
v0.21 and shipped there. What remains in Regional Discovery is the half that is a play system
rather than a generator; its scheduling now follows the priority decision above.

## Shipped brief — Earned Insight v0.23

Historical design brief: the figures and acceptance intentions below are not a current economy
audit. The [2026-08-27 progression review](PROGRESSION-PLAN.md) records 19 entries costing 137 purchasable insight,
repeating raw payouts, and processed requests that retain their full reward. The current native
raw-cycle test covers one cycle, not an unlimited-income bound. Use that review for the next redesign.

The five economy notes, tuned against the world v0.21 and v0.22 built.

### The measured defect

The whole technology tree costs **113 insight**. One full cycle of the eight raw request rows pays
**73 insight for 72 hand gathers**. `GATHER_COOLDOWN_STEPS` is 15 at `PLAYER_TICKS_PER_SECOND` 30, so
a gather is 0.5 s flat for every material. `carry_slots` is 8 and raw stacks are 20, so ten each of
the eight raw materials is exactly eight slots — **one pack-load is one full cycle**.

112 gathers is therefore about **56 seconds of held right-click and two walks to the hub for the
entire tree**. It is not an exploit; it is the shortest path the data describes. And it is available
because `next_request` reposts raw rows forever — raw materials are always `item_reachable`, since no
building outputs them and the walk short-circuits.

`fixtures/balance.json` already prices this honestly: raw pays 1000 `insight_per_gather_milli`,
processed 1300–1867. Under 2× for a machine, its research, its power, and its fuel, against a raw row
that never runs out.

### Gathering becomes a property of the material

Move the cooldown out of the constant and into `ItemDefinition` as `hand_gather_steps`, where
**absent means the hand cannot take it at all**.

| Material                   | Steps | Seconds | Reading                                            |
| -------------------------- | ----- | ------- | -------------------------------------------------- |
| Wood                       | 15    | 0.5     | Flora, cut by hand. The bootstrap fuel stays fast. |
| Clay, Sand                 | 20    | 0.67    | Loose surface earth.                               |
| Stone                      | 30    | 1.0     | Cut off a cliff face.                              |
| Iron ore, Copper ore, Coal | 45    | 1.5     | A hard rock seam.                                  |
| Signal crystal             | —     | —       | **Machine only.** The deep extractor, tier 1.      |

Water is already machine-only and needs no rule: it is terrain rather than a field cell, so
`resource_at_world` never finds it and the pump is the only source. That is the existing precedent
this table generalizes.

### The invariant that moves, and the reason it is safe

`AGENTS.md` and v0.17's record say the hand is worth **exactly** one extractor, both at 120 a minute,
and `one_extractor_is_worth_exactly_the_hands_it_frees` pins it. Restate it as: **the hand is never
faster than an extractor working the same cells, and on hard rock it is materially slower.**

That keeps the guard v0.17 actually cared about — the curve inversion where the first machine a player
builds is slower than the hands it replaces — and adds the incentive the notes asked for, because the
new rule is strictly stronger in the direction that mattered. Move the invariant text, the test, and
the fixture's extraction section together, and say in the shipped record that the equality was
deliberately broken rather than lost.

### A raw row pays once at full price

Add `repeat_insight` to `RequestDefinition`. The first fill pays `insight`; every later fill pays
`repeat_insight`. Raw rows get **2**. Processed rows keep their full value.

`request_rounds` already counts fills and is already saved and checksummed, so nothing new enters the
envelope. The eight surveys pay ~73 once, which comfortably funds the early tree — Field Logistics,
Automated Extraction, On-site Power, Storage Planning, and Composition come to 30 — and the remaining
83 has to come from a smelter or a kiln.

A decayed repeat rather than an exhausted row, deliberately: a row that stops existing can strand a
player who has no fuel and no power and no way back, and 2 insight for ten gathers is already a rate
nobody will choose. The floor removes the soft-lock; the number removes the exploit.

`every_processed_request_pays_better_per_gather_than_raw_material` needs a companion asserting that a
**repeated** raw row pays worse per gather than every processed row, which is the claim that actually
holds this together.

### What replaces grinding, and why it is mostly already built

Closing the raw loop only answers half the note it came from. The other half — insight should require
items that are processed **according to how deep the player is in the tree**, so that funding research
is what leads them to discover more — must not be built from scratch here, because v0.20 already
built it and this milestone's job is to finish it.

Two shipped mechanisms carry it:

- **Eligibility is the recipe tree.** `Core::item_reachable` walks from a requested item down
  through
  the recipes that make it, requiring a buildable machine for every category and an unlocked source
  for every leaf. A row cannot be posted until the player could actually produce it, so the board
  opens up as research does, without an unlock column anybody has to maintain.
- **The reward curve is already depth-scaled**, in `insight_per_gather_milli`: raw 1000, crystal
  1250,
  one machine step 1300–1333, assembly 1533–1625, the deep chains 1778–1867. `fixtures/balance.json`
  computes it from the shipped tree rather than from authored intent, which is how v0.20 caught its
  own first pass putting glass at exactly 1000.

So the depth ladder exists and is measured. What v0.23 owes it is the two things that currently
undercut it:

1. **A floor that makes the ladder the only way up.** Under 2× is a weak gradient when the bottom rung
   is infinite; `repeat_insight` is what makes the gradient decisive, and it should be tuned against
   the printed curve rather than picked. If 2 leaves any raw row competitive per _minute_ once the
   new hand rates land, move it, and say which figure moved it.
2. **A board that leads rather than waits.** `next_request` posts the least-used eligible row, so
   fresh content does lead — but only in catalogue order, and nothing guarantees the three posted
   slots are not all shallow rows the player has long outgrown. Consider reserving one slot for the
   deepest eligible row. Consider it, measure it, and reject it in writing if a three-slot board
   cannot afford the reservation: a player who cannot yet build a smelter must never face a board of
   three things they cannot make, which is the trap `skip_request` exists to escape and which a
   reserved slot could quietly recreate.

Neither is a new system. Both are one predicate each, and both belong in this milestone rather than a
later one, because they are what make the hand-rate change read as an invitation instead of a tax.

### Acceptance

- The technology tree cannot be completed from raw materials alone, demonstrated by a native test
  that fills every raw row repeatedly and shows the reachable insight falling short of 113.
- Hand-gathering rates come from the item definition; a material with no `hand_gather_steps` refuses
  a hand gather with a message naming what does extract it.
- No extractor is slower than the hand on the same cells, on any material, pinned in the fixture.
- Signal crystal is obtainable only by machine, and the guidance says so rather than leaving the
  player to discover it by failing.
- `fixtures/balance.json` reports the per-material hand rate and the repeat rate, and both test
  suites expand it independently.
- A repeated raw row pays worse per gather **and per minute** than every processed row, measured
  against the new hand rates rather than the old flat one.
- The board never posts three rows the player cannot supply, whatever is decided about reserving a
  slot for depth. If the reservation is rejected, the reason is written down.

## Deferred — Living Lattice

Starts only after the progression and construction workstream above. Formerly reserved for
v0.26; its new release number is unassigned. Its ecological scope is preserved.

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
  [construction brief](CONSTRUCTION-MATERIALS-PLAN.md#several-production-routes-require-honest-costing)
  now owns replacing that assumption before refinery co-products ship. Ecology must reuse its
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

## Later — Regional Discovery

Follows Living Lattice, after progression and construction. Formerly reserved for v0.27; its new
release number is unassigned. Construction's necessary limestone/oil access and surveys arrive
with construction; the broader regional discovery system remains here.

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
- **Fluid networks.** Water ships as a belted item and says so; the real network is an improvement
  on
  a working game rather than a second network model built beside the first.
- **Intermittent generation and accumulators.** They arrive together. Intermittency has to be a
  deterministic function of tick and position, never a runtime roll.
- **A day cycle, and solar with it.** A presentation and simulation change at once, chosen for what
  it does to the game's feel rather than smuggled in as a power source.
- **Water reshaping.** Bounded dry-land levelling and foundations have moved into the active
  construction sequence. River diversion and seabed work remain deferred beyond that scope.
- **Fluid grade separation.** Tunnels shipped in v0.25.1 as the underpass pair, exactly as this list
  predicted: one arm in the graph trace, the covered cells still walkable and still their own lane.
  What is left of the entry is the half it promised for free — pipes inheriting the same arm when
  fluids land, which is a fluid-network decision rather than a transport one.
- **Organic tileables.** The later half of the art generator: systems that produce tileable textures
  and shapes so a hex lattice reads as organic terrain and organic objects. Same invariants —
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

- **v0.32.0** Industrial Bills — Reprices five industrial stations in plates, gears and mineral structure, with no brick bootstrap for the kiln. Startup reports use a valid construction order; guidance names missing construction suppliers. Save 24 / definitions 19 preserve active jobs, stock and checksums. [Verification record](INDUSTRIAL-BILLS-RECORD.md).
- **v0.31.0** Foundation Commissions — Completing the first founding stage grants belts, storage, extractors and on-site power. Those four technologies are grant-only, with typed effects replacing ad-hoc unlock and bonus fields. Save 23 advances technology 10 to 11 and scenario 5 to 6; a factory already past Prove the line receives the grants. Insight prices of later nodes are unchanged. [Verification record](FOUNDATION-COMMISSIONS-RECORD.md).
- **v0.30.0** Research Atlas — Large central technology tree with all 19 SVG emblems, prerequisite
  lines and selected ancestry, hover/focus details, deliberate purchases, search, discipline and
  in-reach filters, pan/zoom and keyboard navigation. Four independent starting technologies
  replace the universal logistics gate. Save 22 / technologies 10 migrate existing knowledge intact. [Verification record](RESEARCH-ATLAS-RECORD.md).
- **v0.29.0** Research Foundations — Branch/stage registries classify all 19 technologies; the
  keyed list shows discipline, stage, costs, blockers and corner-heading benefits. Native publishes
  the same availability used by atomic purchases, and guidance consumes it. Prices, prerequisites,
  effects and request income are unchanged. Save 21 advances technology 8 to 9 without changing
  state or checksum; the picker recognizes the released save 14–21 chain. Definitions 18, scenarios
  5 and world 8 remain; wire 13 carries the derived availability group only when its answer changes.
  Commissions, typed requirements/effects, finite insight projects and separate skills remain ahead.
- **v0.28.0** Essential Bills — The five stations the opening touches are billed in manufactured
  parts. An extractor costs 2 iron plate, a gear and 2 timber; a composer 2 plate, a gear and a
  frame; a container 3 timber; a pole a timber and an iron wire; a burner generator a plate, a frame
  and 2 wire. Iron wire is new — `1 iron plate -> 2 iron wire` in 6 ticks at the composer or the
  manual workshop — and is what keeps the first grid off copper. Signal crystal leaves ordinary
  assembly, so the first circuit needs none where it needed three, and crystal falls to one
  dependent. The cost is the opening: first power rises from 17 gathers to 36 and gains 136 ticks of
  attended work, first smelter 40 to 47, first circuit 76 to 88. The curve holds — burner at 8.500
  effort under the wind turbine's 10.000 — and the primitive furnace and workshop remain the
  research-free floor that makes the new bills reachable. Save 20 migrates existing factories
  unchanged to definitions 18; a legacy station refunds the new bill, a one-time revaluation that is
  a fixed point rather than a loop. Technologies 8, scenarios 5, world 8 and wire 12 remain. The
  smelter, kiln, cutter, crusher and pump bills are deliberately untouched and still unpriced.
- **v0.27.0** Transport Kits — Belting is manufactured, not gathered. One iron plate and one
  timber assemble into four transport kits at the manual workshop or the composer; a belt costs one
  kit, a corner heading two, a splitter or merger two plus a gear, and an underpass two plus
  structural metal and stone. Measured against the previous prices, a hundred-segment line falls
  from 100 ore to 52 ore and 18 wood — 108 gathers and 162.0 s of hand work down to 103 and
  129.2 s — while a twenty-four segment line rises from 32 gathers to 46 because the first run now
  pays for the workshop and furnace behind it. Save 19 migrates existing factories unchanged to
  definitions 17; a legacy belt refunds the kit that rebuilds it, which conserves rather than mints,
  since no recipe returns a kit to ore. Technologies 8, scenarios 5, world 8 and wire 12 remain.
  The extractor, composer, generator and pole bills are deliberately untouched and still unpriced.
- **v0.26.0** Primitive Workshops — A stone-and-clay furnace smelts plates with ordinary fuel
  and no grid. A wood-and-stone workshop makes a restricted set of existing recipes with native
  attended work, one batch per press. Walking/gathering pauses it; jobs survive save/load, and
  dismantling refunds reserved ingredients. Capability lists and duration multipliers are validated
  in both languages; guidance, hotbar defaults, recipe controls, shapes and opening checkpoints
  cover the primitive route. Balance separates player work from machine time. Save 18 migrates
  existing factories unchanged to definitions 16; technologies 8, scenarios 5, world 8 and wire 12
  remain. This is the first foundation delivery, not the full progression or construction overhaul.
- **v0.25.3** Compartment Storage — Machines keep independent ingredient, fuel, and bounded
  output inventories; blocked outputs buffer complete cycles. The inspector presents those maps as
  item-slot grids beside the cargo pack. A native cursor-held stack supports full, half, single, and
  Shift quick moves without making the host authoritative for reach, compatibility, or quantity.
  Dropped and demolished-belt cargo persists visibly in the world for one minute and can be
  collected again; Backspace/Delete demolish the hovered or selected structure. Two research rows
  expand cargo slots and construction reach through native player state. Save 17, definitions 15,
  technologies 8, wire 12.

- **v0.25.2** Wayfinding — A second click on a selected hex walks the player there. The goal joins
  `PlayerState`: saved, checksummed, and resumed, because where the player is headed is a standing
  order the simulation is executing rather than a key being held. The route is derived and never
  saved — a bounded A\* over hex centres, replanned by `rebuild_runtime_index` so every edit and
  every load rebuilds it against the world that actually exists. Shallow water costs five, the ratio
  between the ford speed and the walking speed, so the answer is the fastest way rather than the
  shortest one. The search reads the pure `terrain_at` and `runtime.occupied` and surveys nothing,
  because `generated_chunks` is a checksum input. The host sends a destination and never a route,
  and draws native's own remaining path twice — a ribbon and destination ring over the terrain, a
  line and goal pip on the minimap — so a walk that leaves the viewport stays legible and the
  picture can never promise a way the simulation would not take. Any movement command, including the
  zero one a key release sends, hands control back. Save 15, wire 10.

- **v0.25.1** Junctions — Belt lines split, merge, and cross. `splits` and `merges` are definition
  flags over the existing compiled graph and the existing tick, not new kinds: a splitter fans every
  free forward heading and offers from `route_cursor`, a merger accepts from behind and alternates on
  `merge_cursor`, and both cursors are saved and checksummed so a restored factory keeps the rotation
  it was running. The underpass is the tunnel arm the horizon list had already costed, and the
  crossed cells stay singly occupied, buildable, and connected to their own lane. The riser is gone
  as a separate definition: `OrientationAxis::Any` puts both periods on the belt, priced by
  `corner_construction_cost` and gated by `corner_technology_id`, so rotation walks all twelve
  headings 30° at a time and a drag routes on every heading the player has actually paid for. Save
  14, definitions 14, technologies 7, wire 9.

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

- **v0.21** Landforms and Fields — **A deposit is a site, not a hex.** The world is partitioned by a
  `site_cell` lattice; each cell hashes to at most one site — a jittered centre, one weighted rule,
  one radius — and a hex belongs to the nearest site whose disc covers it and whose `member` bands
  it satisfies, ties broken by lattice cell. Yield falls from core to rim. `FieldRule` became
  `SiteRule`, row order stopped being a generation input, and `relaxed()` went with the per-hex
  gates it eased — a preset now compensates in `weight` and `radius_max`, which the survey can see
  and a gate never could. The lattice is cached on `Core`; the field never is.

  Rivers are ridge noise rather than a simulation, `ShallowWater` cut after the band test and before
  the cliff test. Beaches ask the coarse elevation octave alone whether a coast is ocean. The eight
  hardcoded clearing cells are gone: a `bootstrap` pass spirals over lattice cells and guarantees
  iron and forest within 14, coal, stone, and clay within 25, and copper within 40 — never sand,
  never crystal — widening a window in fixed steps and then refusing the world. Generator 7.

  **What it was measured against.** Purity is the share of resource hexes whose radius-1 disc holds
  one material, which is what decides whether an extractor works a field or straddles two. Target
  950; the shipped seed at radius 96:

  | preset        | purity before | purity after | worst before | worst after              |
  | ------------- | ------------: | -----------: | ------------ | ------------------------ |
  | `continental` |           532 |          971 | stone 0      | sand 895                 |
  | `archipelago` |           474 |          965 | wood 36      | sand 940                 |
  | `highlands`   |           662 |          990 | crystal 28   | clay 977                 |
  | `basin`       |           631 |          992 | crystal 71   | crystal 809 _(21 cells)_ |

  Stone had **no workable patch anywhere in 26,307 land hexes** on `continental` before this — its
  largest patch was 3 hexes against a base extractor's 7 — and neither did wood on `archipelago`.
  Every preset now clears 19 hexes for iron, coal, copper, and stone and 61 for forests, and every
  material has a patch an extractor can be stood on. `archipelago`'s landform scale moved from 4 to
  5 and its blend from 45 to 52, because at the old numbers no band held a contiguous run wider than
  a deposit and every disc came out a crescent.

  Generation cost 1.42 µs/hex against 0.52 µs for the model it replaced, on the same harness — the
  site lattice cache is what keeps that from being ~350 noise samples per hex. The capacity ladder is
  flat, as it must be: its scenario never generates.

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

## Reference — the shipped presets, as measured

From `npm run survey` at seed 1,213,486,160 after the world-scale pass. Each preset is sampled at
its landform radius (continental / highlands / basin 768, archipelago 192), because a 96-hex disc
is the opening, not a landform. Bands in parts per thousand; water as bodies / mean body / largest
body, **rivers excluded** so that `largest body` still means ocean; rivers as hexes / runs / longest
run.

| preset      | water | shore | lowland | hills | highland | cliff |    water bodies |              rivers | purity |
| ----------- | ----: | ----: | ------: | ----: | -------: | ----: | --------------: | ------------------: | -----: |
| Continental |    30 |    61 |     302 |   354 |      238 |    12 |  504 / 19 / 446 |  45604 / 129 / 8801 |    991 |
| Archipelago |   110 |   190 |     317 |   142 |      202 |    35 | 253 / 48 / 2247 |                   — |    981 |
| Highlands   |    35 |     0 |     113 |   410 |      416 |    23 |     85 / 3 / 25 | 63110 / 144 / 17988 |    995 |
| Basin       |    45 |   198 |     524 |   211 |       16 |     2 | 717 / 58 / 7360 |  40103 / 98 / 19096 |    998 |

A hexagon is 1 m²; the walk is 3 m/s. Continental's 512-hex landform is a three-minute crossing,
basin's 960-hex one is six, and a river of eight to ten hexes is a real river. The opening inside
~80 hexes is still the cell-8 mix the bootstrap windows were tuned against, so the first minute
does not wait on a coast.

`basin` is the ocean preset: largest standing body 7,360 hexes and truncated, and the water
nearest a sand patch averages **2,494**. `archipelago` holds a 2,247-hex body inside a 192-hex
sample. `continental` on this seed is inland — no sand in 768 hexes, largest standing body 446 —
which is a continent, not a missing ocean; the coast is the walk. `highlands` still has no ocean
worth the name (largest body 25) and keeps `ocean_level` where those basins pass it, so the little
sand it holds (five patches, nearest 336 hexes) sits on the largest water it actually has.
