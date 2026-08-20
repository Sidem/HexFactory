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

The engine arc, the generator arc, and the first three milestones of the pivot from substrate to
motive are all shipped. A run today looks like this: land beside a hub in a world chosen by preset
or by raw parameters, walk out under fog, read the terrain for eight raw materials, gather, fill the
hub's posted **requests** for insight and its staged founding **contract** for hub growth, research a
twelve-technology tree, and build a powered, automated line of twenty buildings and fourteen recipes
across five machine categories. Buildings are drawn by a shape grammar, so a tier is a data row.
Power is energy bought per unit of work. The world and the minimap render on WebGL2.

**Current envelope numbers** — native refuses a load on all five, and the browser's named-save
catalog shows which one moved rather than hiding the row:

| Envelope              | Version |
| --------------------- | ------: |
| `HXF1` save           |      10 |
| Definitions           |      10 |
| Technologies          |       5 |
| Scenarios             |       5 |
| World generator       |       6 |
| Wire (snapshot delta) |       5 |

**Current measured capacity.** A complete browser frame at 6,144 entities is 19.0% of 60 Hz
(v0.13.1 record, Canvas 2D — the WebGL2 pass has not been re-measured). Generation costs at most
0.35 µs per hex. See `docs/BENCHMARKS.md`; no claim beyond a recorded tier is supported.

**The shipped ledger is at the bottom of this document**, one line per release. Read it for what
exists; read the section a milestone names when you need the reasoning behind a rule you are about
to change.

## What to do next

**Landforms and Fields v0.21.** The order v0.21 → v0.22 → v0.23 is load-bearing rather than a
preference; the roadmap decision below is why.

Do not start v0.23 first. It is written to be tuned against a world that v0.21 builds. Its other
prerequisite is already met: v0.20.1 collapsed the transfer rows into one function, so the
fractional deposit that arrived with them is not waiting on anything.

**v0.21 and v0.22 are one version train.** v0.21 moves `WORLD_GENERATOR_VERSION` and rejects every
existing save; v0.22's twelve-heading routing table wants a save break of its own and rides that one
instead of spending a second. If they are ever split, v0.22 has to pay for its own break and the
orientation-index decision in that brief reopens.

### Open decisions, each with what would settle it

- **Does `regrowth_ticks` move** when a forest cell drops to three wood? (v0.21 — measure the
  extractor's starve rate over seven cells before calling it a design.)
- **Is the single-cell footprint restriction lifted** now that its stated reason is gone? (v0.22 —
  the brief recommends **not** now, and says why. It needs a definition asking for it.)
- **Is one board slot reserved for the deepest eligible request?** (v0.23 — consider it, measure it,
  and reject it in writing if a three-slot board cannot afford the reservation.)
- **Rails or free-floating panels** was settled by shipping the rail. What would reopen it: wanting
  positions the player chooses, at which point the rail becomes the docked default rather than the
  destination, and the saved-coordinate, overlap, off-screen-recovery, and touch-gesture questions
  all come back.

One decision that is **not** open and must not be reopened casually: `DIRECTIONS` stays six. Twelve
headings are transport only. Widening adjacency would let a boiler reach a turbine two rows away and
a pole span a distance no player can see.

### Open, unassigned to a milestone

- **A timed keyboard-and-pointer playtest of the opening, done by a person.** Owed since v0.18 and
  still outstanding: the agent's browser pane does not composite, so `requestAnimationFrame` never
  fires and nothing on the player's own clock — walking, gathering, the cooldown, the precision walk
  — has ever been exercised. `fixtures/balance.json` predicts the material work (32 gathers to
  contract stage one, 97 to stage two, a 65-second combined hand floor) and says nothing about
  walking, choosing, or placing. A number from a person outranks every number in that file.
- **The WebGL2 renderer has not been benchmarked.** It replaced the Canvas 2D world and minimap
  draws that `docs/BENCHMARKS.md` records, so the current browser-frame record describes a renderer
  the game no longer ships. Re-measure before quoting a frame number.
- **Belts on field cells stay legal, but paving the rare landing crystal without reading it first
  should not.**
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
mountains, and a world where standing an extractor next to a deposit is worth doing. That is
**Landforms and Fields v0.21** below.

**The world work comes first, and the dependency is real rather than tidy.** Making a hand gather
slower per material only reads as "go and build an extractor" when there is a field worth putting an
extractor on. Applied to today's generator — where a continental survey finds iron in 205 scattered
cells and stone in 18 — slower hand mining is not an incentive, it is tedium. So the generator
lands, then the economy is tuned against the world that exists rather than against the one being
replaced.

**Regional Discovery is split, not deleted.** Its _generation_ half — a landing clearing that
guarantees a bootstrap path rather than a sample platter, and a survey that proves every preset still
works — is exactly what v0.21 has to do anyway for fields to mean anything, so it moves forward into
v0.21. What stays at v0.25 is the half that is a play system rather than a generator.

## Next — Landforms and Fields v0.21

### The defect, named, and measured before it is argued

`npm run survey` at the shipped seed and the default radius of 96, continental preset, 27,937 hexes
of which 26,307 are land:

| material       | cells | per mille land | nearest |
| -------------- | ----- | -------------- | ------- |
| Iron ore       | 205   | 7              | 20      |
| Copper ore     | 469   | 17             | 15      |
| Coal           | 444   | 16             | 8       |
| Clay           | 420   | 15             | 14      |
| Sand           | 219   | 8              | 19      |
| Wood           | 111   | 4              | 15      |
| Signal crystal | 85    | 3              | 20      |
| Stone          | 18    | 0              | 23      |

Stone is eighteen cells in twenty-six thousand hexes of land. Wood is a hundred and eleven isolated
cells holding ten to twenty-two units each, which is the opposite of a forest in both directions: too
much in one hex and no continuity between hexes. And the survey reports none of what actually
decides whether a deposit is worth automating, because **it has no measurement of patch size at
all** — only totals, densities, and distances. That absence is why this has never been caught.

### Why this is structural rather than a tuning pass

`field_at` decides each hex independently. It reads three noise channels and walks the `FieldRule`
table, first match wins. There is no object anywhere in the generator that means "a deposit", so a
patch's size and a patch's purity are emergent accidents of channel cell size and gate height —
neither controllable, nor defaultable, nor measurable.

The mixed-material case is the clearest proof. In Highland, iron gates on `richness > 54_000` and
coal on `vein > 56_000`, and those are two **independent** channels. Wherever both run high the row
order decides, so along every iron/coal boundary the two alternate hex by hex, and an extractor
placed there covers both and cleanly works neither. No amount of moving those two numbers fixes it,
because the two numbers are not asking one question.

So the model changes, and the bands stay. Terrain remains the material map and the reason a landscape
can be read; what stops being per-hex is the decision about what a patch is made of.

### First commit: measure patches against the current generator

Before a generation rule moves, grow `survey` a `PatchCount` per material, reported alongside
`MaterialCount` and computed by the same flood fill `water_shape` already uses over `DIRECTIONS`:

- `patches` — connected runs of one material.
- `mean_patch`, `largest_patch` — in hexes.
- `mean_patch_yield` — total units in a patch, which is what an extractor is actually being offered.
- `nearest_patch_of_at_least(7)` — the distance to the first patch a base extractor could fill its
  own disc from, which is a different and more useful number than `nearest`.
- `purity` — the share of resource hexes whose radius-1 disc holds exactly **one** material. This is
  the number the whole milestone is for. Target after the change: **at least 950 per mille**.
- `truncated_patches`, on the same reasoning as `truncated_bodies`: a patch touching the sample edge
  is a floor, not a measurement.

Record the before figures in this document in the same commit. A tuning claim without a before
number is the failure mode `fixtures/balance.json` exists to prevent, and generation deserves the
same treatment.

### A site is the unit of a deposit

Partition the world by a `site_cell` lattice, exactly as the noise channels are partitioned. Each
site cell hashes to at most one **site**: a jittered center, one material, and one radius. A hex
belongs to the nearest site whose disc covers it and whose member gate it satisfies. Yield falls off
from core to rim.

`FieldRule` becomes `SiteRule`:

```rust
struct SiteRule {
    /// The band the site's *center* must stand in for this rule to be eligible.
    terrain: Terrain,
    item_id: ItemId,
    /// Relative share among the eligible rules for a band. Zero means never.
    weight: u32,
    /// Inclusive radius range, in hexes. A disc of radius R holds 3R² + 3R + 1 hexes:
    /// 7, 19, 37, 61, 91, 127 at radius 1 through 6.
    radius_min: u32,
    radius_max: u32,
    /// Exclusive lower gate on the richness channel at the *center*, so the world still has rich
    /// and poor country. `ANY` disables it, on the same reasoning `ANY` already carries.
    site_min: i32,
    /// Yield at the center and at the rim. Interpolated linearly by distance, then jittered.
    yield_core: u32,
    yield_rim: u32,
    /// Per-hex jitter on the interpolated yield. At least 1; `base + hash % spread` semantics.
    yield_jitter: u32,
    /// Bands a hex must itself be in to belong to this site. Empty means the rule's own band.
    /// This is the clipping that makes a beach a strip and a scree field hug its cliffs.
    member: Vec<Terrain>,
    /// If set, a member hex must also be within this many hexes of water. `0` disables it.
    member_water_within: u32,
}
```

The evaluation, which must stay a pure function of `(params, seed, q, r)`:

1. `reach = ceil(max_radius_max / site_cell) + 1`, over the whole rule table.
2. For every site cell within `reach` of the cell containing `(q, r)`, in a fixed iteration order:
   - `h = coordinate_hash(seed ^ SITE_SALT, cell_q, cell_r)`.
   - The center is the cell origin offset by two independent fields of `h`, each taken modulo
     `2 * site_jitter + 1` and shifted down by `site_jitter`.
   - The band is `terrain_at` at the center. Eligible rules are those whose `terrain` matches and
     whose `site_min` the richness channel at the center clears. No eligible rule means no site.
   - A weighted pick over the eligible rules, by a third field of `h` against the summed weights.
   - `radius = radius_min + (fourth field of h) % (radius_max - radius_min + 1)`.
   - The cell is a candidate when `axial_distance(center, (q, r)) <= radius` **and** `(q, r)`
     satisfies the rule's `member` bands and `member_water_within`.
3. Among candidates take the smallest `axial_distance(center, (q, r))`; break ties by `(cell_q,
cell_r)` in that order. Ties must be broken explicitly — a tie resolved by iteration order is a
   tie resolved by nothing, and this is a checksum input.
4. `yield = yield_rim + (yield_core - yield_rim) * (radius - distance) / radius`, then
   `+ coordinate_hash(seed, q, r) % yield_jitter`, clamped to at least 1. Keep the jitter small
   enough that the core still reads as a core.

That gives, by construction rather than by tuning: **one material per patch**, a patch size that is a
parameter, and a rich middle worth aiming an extractor at.

### The cost of this, and the cache that pays it

`Core::field_at` is not only called during `generate_chunk` — `deposit_candidates`, `resource_at_world`,
both gathers, and every snapshot build reach it, and `deposit_candidates` walks a whole disc. The
naive form evaluates up to `(2·reach + 1)²` site cells per hex and each one calls `terrain_at`, which
itself samples seven elevations. That is roughly 350 noise samples per hex and it is not shippable.

Cache the **site lattice**, not the field: a `BTreeMap<(i32, i32), Option<Site>>` on `Core`, filled
lazily per site cell. A site cell is ~144 hexes, so the map is small and every hex in a chunk hits it
warm. It is derived state under the existing invariant — never saved, never hashed, never
checksummed, cleared whenever the world changes, exactly as `deposit_links` is. The free `field_at`
keeps an uncached path so the survey and the tests can call it without a `Core`, and one test asserts
the cached and uncached answers are identical over a disc. Re-run `npm run bench` before shipping:
this touches the world generator, so the ladder is not optional.

### Rivers are ridge noise, not a simulation

A flow simulation is refused: the map is unbounded and generated lazily, so nothing may depend on
knowing where the water upstream went. A river is instead where a dedicated channel runs near its
midpoint — `abs(value_noise(river_cell) - NOISE_MAX / 2) < river_width` — gated to
`elevation < river_max_elevation` so rivers do not run over summits. That is O(1) per hex, purely
local, and fits the existing contract exactly.

A river hex reads as `Terrain::ShallowWater`, evaluated **after** the band cut and before the cliff
test, so a river cuts through lowland and hills and stops where the highland gate says it does. Three
consequences the milestone wants and should not be surprised by:

- Shallow water stops being an accident of sea level and becomes **common and linear**, which is
  what
  makes a bridge a necessity rather than an ornament.
- `PlacementRule::Water` — buildable ground with open water inside `PUMP_RADIUS` — starts matching
  inland. Pumps, hydro, and boilers gain sites everywhere, which is a real balance change and belongs
  in `fixtures/balance.json`'s access section, not in a footnote.
- The survey's water figures start mixing bodies and rivers. Report river hexes, river runs, and
  mean
  run length separately, or the existing `largest_body` claim quietly stops meaning ocean.

### Beaches need an ocean, and an ocean cannot be flood-filled here

Sand should sit on real coast, not on the rim of every pond, and the generator cannot flood-fill to
find out which is which. Use the split v0.16 already established and proved: **coarse-octave water is
what makes a body big**. A sand site therefore requires the coarse elevation octave alone — not the
blended elevation — to sit below `water_level` across the center's neighbourhood. Pond edges, which
exist only in the fine octave, fail it; ocean coasts pass it.

State plainly in the code comment that this is a proxy rather than a measurement, and let the survey
be what verifies it: the flood fill in `water_shape` already knows body sizes, so report the mean
size of the body nearest each sand patch. If that number is small, the proxy is wrong and the survey
said so.

### The resource table, resource by resource

Starting points, not shipped numbers. Every one of them is chosen against the survey the way
`cliff_step` was chosen in v0.16, and the survey is what settles them.

| Material       | Center band          | Radius               | Yield core → rim | Member clipping                     | What it is for                                                                                                                                                                                      |
| -------------- | -------------------- | -------------------- | ---------------- | ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Iron ore       | Hills, Highland      | 3–4 (37–61)          | 20 → 8           | own bands                           | The flagship early field. One is guaranteed near spawn; it is what the first extractor and the first smelter are for.                                                                               |
| Coal           | Highland, some Hills | 2–4 (19–61)          | 18 → 8           | own bands                           | Its own patch, often the next valley from iron. A smelting site is two neighbouring fields, never one mixed hex.                                                                                    |
| Copper ore     | Hills only           | 2–3 (19–37)          | 18 → 8           | own band                            | Keeps "copper belongs to rolling ground, iron and coal to the tops", which is what the `Hills` doc comment already promises.                                                                        |
| Stone          | Highland             | 3–5 (37–91)          | 12 flat          | Highland + Cliff                    | Scree around mountains. Cliff hexes are members and are unworkable; the buildable rim is where you quarry — so v0.11's extraction-radius lesson survives intact, at fifty times the current supply. |
| Wood           | Lowland              | 4–6 (61–127)         | **3 → 1**        | own band                            | A forest: roughly 150–250 units across a large area, renewable through the `regrowth_ticks` the item already carries, with a soft edge.                                                             |
| Clay           | Lowland, Shore       | 2–3 (19–37)          | 14 flat          | own bands, `member_water_within: 2` | Riverbanks and lake shores. Depends on rivers existing, which is why the two ship together.                                                                                                         |
| Sand           | Shore                | 3–5, heavily clipped | 16 flat          | Shore only, ocean gate              | The disc clipped to coastline yields a beach strip rather than a blob.                                                                                                                              |
| Signal crystal | Highland             | 1 (7)                | 10 flat          | own band                            | Rare, finite, remote, and never guaranteed near spawn. With v0.23 making it machine-only, it is a genuine prize.                                                                                    |

Two notes the next session should not have to rediscover:

- **Wood at 3 per cell is a rate change, not only a shape change.** A base extractor covers seven
  hexes, so it drains 21 wood and then runs at whatever regrowth supplies across those seven cells;
  at `regrowth_ticks: 90` that is one unit per ~13 ticks against an extraction cadence of 5. The
  extractor spends most of its life starved. That is not obviously wrong — it makes forestry a
  question of area rather than of throughput, and it makes the deep extractor's nineteen cells the
  forestry upgrade — but it **must be measured before it is called a design**, and it is the reason
  `regrowth_ticks` may need to move in this milestone rather than in v0.23.
- **Stone is deliberately the cheapest material to find and the least valuable per hex.** It is
  structural, it is in every construction cost, and the current 18-cell figure is why nobody builds
  with it. Flat 12 across a 37–91 hex field is the fix.

### The landing clearing stops being a supermarket

`LANDING_FIELD` is a hardcoded list of eight single cells, one of every material, inside
`LANDING_CLEAR_RADIUS`. That, and not the generator, is why every material is visible in the first
minute. It is the sample platter the roadmap decision already named, sitting in a constant.

Replace it with a **bootstrap pass**: a pure function `bootstrap_sites(params, seed) -> BTreeMap<(i32,
i32), SiteOverride>` that spirals outward from the landing site over site cells in a fixed order and,
for each guaranteed material, claims the first cell whose center band admits that material and whose
distance falls inside a stated window. A claimed cell is forced to that material at `radius_max`.

| Guarantee      | Window                | Why that window                                                      |
| -------------- | --------------------- | -------------------------------------------------------------------- |
| Iron patch     | 9–14                  | The first extractor, in sight of the hub.                            |
| Forest edge    | 9–14                  | Fuel and timber, and the first thing a player walks into.            |
| Coal field     | 15–25                 | A short walk, chosen rather than stumbled on.                        |
| Stone field    | 15–25                 | Same.                                                                |
| Clay on water  | 15–25                 | Carries a river or shore with it, which is also the first pump site. |
| Copper field   | 25–40                 | The second metal is an expedition, not an errand.                    |
| Sand           | wherever the coast is | Not guaranteed by distance; the ocean gate decides.                  |
| Signal crystal | **never**             | It is the reason to leave.                                           |

Constraints that make this correct rather than merely deterministic:

- A window is a floor as well as a ceiling. Centers sit at `distance >= radius + 8` so a guaranteed
  disc cannot reach inside the clearing, whose field suppression stays exactly as it is.
- If a window finds nothing, widen it in fixed steps to a hard cap and then **fail loudly**. A
  preset
  that cannot bootstrap is the failure the survey exists to make visible, not something to paper over
  — `highlands` has almost no Shore band and is the case that will find this.
- The table is derived state on the same terms as the site cache: recomputed from `(params, seed)`,
  never saved, never hashed. The free function is shared by `Core` and by `survey`, so a surveyed
  world and a played world cannot disagree.

### Parameters, presets, and the control surface

New scalars on `WorldParams`, all hashed by `hash_world_params`, all validated by
`WorldParams::validate`, all bounded the way `MAX_FEATURE_CELL` bounds the existing cells:
`site_cell`, `site_jitter`, `river_cell`, `river_width`, `river_max_elevation`, and the coarse-octave
threshold the sand gate reads.

The host cannot fall behind by accident: `WorldScalar` is `Exclude<keyof WorldParams, "field_rules">`,
so every scalar added in Rust is a **typecheck failure** in `src/main.ts` until
`WORLD_PARAMETER_FIELDS` grows a labelled, range-checked control for it. Keep it that way; do not
widen the type to make the error go away.

`relaxed()` goes. It eased per-hex gates on one band, and there are no per-hex gates left to ease. A
preset that makes a band scarce now compensates by raising that band's `weight` and `radius_max` in
its own rule rows, which is both more direct and more honest — the survey can see it.

All four presets are re-authored against the new survey, `continental` included. The default is the
world being complained about, so "the shipped default is version 5's frozen numbers" stops being a
virtue here and becomes the thing to fix.

### What moves

- `WORLD_GENERATOR_VERSION` 6 → 7. Every existing save is refused, which is the established and
  correct behaviour, and the named-save catalog already shows the row rather than hiding it.
- `fixtures/balance.json`: the `access` section, and every site-yield figure. Rivers make water and
  hydro available far more widely, so the openings move too.
- Rust tests that must be rewritten rather than nudged:
  `every_material_is_generated_where_its_geography_says_it_should_be`,
  `every_preset_reaches_every_material_from_the_landing_site` (becomes a bootstrap-window assertion
  per preset), `generated_fields_follow_terrain_and_only_the_overlay_is_state`,
  `parameter_sets_that_are_not_worlds_are_refused`, `feature_scale_makes_seas_and_sea_level_only_makes_more_ponds`,
  `every_recipe_input_is_reachable_from_the_landing_site`, and
  `cut_flora_grows_back_to_what_generation_gave_it_and_then_stops`.
  `field_rule_order_decides_which_band_holds_what` is retired — row order stops being a generation
  input in that sense — and a purity test replaces it.
- `a_save_restores_the_parameters_its_world_was_generated_from` must cover the new scalars and the
  rule table's new fields, or a parameter can drift across a save without anything noticing.
- `chunk_generation_is_order_independent_and_seeded` is the test that catches a site cache leaking
  order-dependence into generation. Do not let it stay unchanged and unexamined.

### Acceptance

- The survey reports patch statistics, and **purity is at least 950 per mille** on every shipped
  preset. The before figures are recorded in this document alongside the after.
- Every preset produces iron, coal, copper, and stone patches of at least 19 hexes, and forests of
  at
  least 61, within the sample.
- Standing one base extractor anywhere inside a patch of at least 19 hexes yields one material only.
- Rivers appear, reach water or terminate at the highland gate, and are reported separately from
  bodies in the survey.
- Sand patches sit against measurably large water; the survey prints the nearest body size per sand
  patch and it is not a pond.
- A new world guarantees iron and forest within 14 hexes and coal, stone, and clay within 25, on
  every preset and on ten sampled seeds — and no preset guarantees crystal.
- Generation stays a pure function of parameters, seed, and hex. The site cache and the bootstrap
  table are never saved, hashed, or checksummed, and a test asserts the cached and uncached generators
  agree.
- `npm run bench` is re-run and recorded, because the world generator moved. No claim beyond a
  measured tier.

## Then — Crossings and Canopy v0.22

v0.21 makes the world; v0.22 makes it legible and crossable. It is deliberately second because a
bridge over no river and a forest renderer with no forest are both untestable.

### A bridge is an entity override, never a terrain change

`Terrain::blocks_movement` is pinned in both languages by `fixtures/terrain-passability.json`, and
that rule stays **literally unchanged**. A bridge does not turn shallow water into land. It is an
entity whose presence `player_blocked` and the placement path consult — both already walk entities —
so the pinned table keeps saying exactly what it says today and gains a note explaining that a
bridged hex is passable by entity, not by band.

- `BuildingKind::Bridge` is **appended** after `Boiler`. Kinds travel as their declaration index, so
  inserting it anywhere else is a silent mistranslation rather than a decode failure; the wire
  fixture's enum table is what catches that, and it must be regenerated and its diff read.
- A new `PlacementRule::Shallows` — _on_ a shallow-water hex. Do not reuse `Water`, which means
  buildable ground _beside_ water and is what the pump uses. Deep water takes no bridge, so deep
  water finally becomes a real barrier and the deep/shallow split earns itself.
- Adding to `BuildingKind` forces a `BUILDING_SHAPES` entry, since the table is total over
  `SilhouetteKey` and `SilhouetteKey` includes `BuildingKind`. That is the compiler asking for the
  drawing, and `docs/ART.md` Stage D is what it should be answered from.
- Belts and risers may be built on a bridged hex. That is the point of it.
- Cost stone and timber, behind its own cheap technology after Field Logistics. Crossing the first
  river should be an early, satisfying unlock rather than a late convenience.

### A forest has to look like a forest

Today every resource cell draws identically: a pulsing hex outline plus the item's icon glyph, one
per hex, whatever the quantity. A forest of 1–3 wood per cell drawn that way is a field of log icons.

Draw **one tree per remaining unit**, deterministically jittered inside the hex from the same shape
vocabulary the buildings use, so a forest visibly thins as it is cut and thickens as it regrows.
`quantity` and `initial_quantity` are already in the resource snapshot, so this costs no new wire and
no new native state — it is presentation over numbers that already cross. Rivers and bridges get the
same treatment: a river should read as a river at ordinary zoom, not as a line of ponds.

### Every radius that exists is drawn

`drawPowerCoverage` draws exactly two rings and both come from `supply_radius` — the pending pole
under the cursor, and the selected pole. Extractors and pumps draw nothing, and it is worse than a
missing ring: the catalogue chip "Reaches N" is conditional on `extract_radius` being present, and
the **base** extractor omits the field entirely, so the first extractor a player ever builds states no
reach anywhere in the UI. The pump's radius is `PUMP_RADIUS`, a bare native constant that reaches no
definition and no panel.

Fix it in the order that matters, and fix the data first:

1. `extract_radius: 1` on the base extractor and on the pump in `definitions.json`, with native
   reading the field instead of the constants. `EXTRACT_RADIUS` survives as the **hand's** reach
   only. This is the v0.19 pole lesson applied where it was not: reach is a property of the thing
   that has it, never a default the host guesses.
2. Generalize `drawPowerCoverage` into a reach pass over **every radius a definition states**, for
   the pending tool and for the selection, colour-coded by meaning, because two rings that mean
   different things must not look the same. The complete list, and a definition growing a radius
   later must join it:

   | Field            | Building        | What it means                       | Treatment                                                                                                                                     |
   | ---------------- | --------------- | ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
   | `extract_radius` | Extractor, Pump | Cells this machine can draw from    | Filled disc, bright rim                                                                                                                       |
   | `supply_radius`  | Pole            | Machines this pole powers           | The existing blue disc                                                                                                                        |
   | `pole_reach`     | Pole            | How far the **next pole** may stand | Rim only, no fill — it is a distance to another pole, not an area of effect, and drawing it as a disc would claim it powers everything inside |
   | `EXTRACT_RADIUS` | The player      | What the hand can take              | Drawn on the player, see 3                                                                                                                    |

   `pole_reach` is the one that is currently invisible everywhere — no ring, no chip, nothing — and
   it is the number that decides whether a grid can be extended at all. A player laying a line of
   poles is guessing at it.

3. Draw the hand's own reach around the player while the harvest key is held. After v0.23 that ring
   is the question the opening is about.
4. Every radius in that table also states itself as a chip on the catalogue card. "Supplies 3" ships
   today; "Reaches 1" and "Links 6" do not.

`extraction_reach_comes_from_the_definition_and_the_hand_keeps_its_own` is the test that already
guards half of this and must be extended, not replaced.

### Twelve headings, not eight

Asked for from play, and it is a generalization rather than an addition: **the eight-direction
routing table is an irregular subset of the regular twelve, and due north is the only member of its
family that was ever implemented.**

A pointy-top hex has six neighbours through its edge midpoints, at 0°, 60°, 120°, 180°, 240°, 300°,
and six vertices at 30°, 90°, 150°, 210°, 270°, 330°. `TRANSPORT_DIRECTIONS` holds the six edges plus
`(1, -2)` and `(-1, 2)` — which are two of the six _vertex_ directions. The other four are simply
absent, for no reason the geometry supplies.

Applying the axial 60° clockwise rotation `(q, r) → (-r, q + r)` — the same rotation the six edges
already follow in their table order — to due north closes the family:

| Index | Axial      | Screen angle | World length     | Heading         |
| ----- | ---------- | ------------ | ---------------- | --------------- |
| 6     | `(1, -2)`  | 270°         | `3 · HEX_RADIUS` | North           |
| 7     | `(2, -1)`  | 330°         | `3 · HEX_RADIUS` | East-north-east |
| 8     | `(1, 1)`   | 30°          | `3 · HEX_RADIUS` | East-south-east |
| 9     | `(-1, 2)`  | 90°          | `3 · HEX_RADIUS` | South           |
| 10    | `(-2, 1)`  | 150°         | `3 · HEX_RADIUS` | West-south-west |
| 11    | `(-1, -1)` | 210°         | `3 · HEX_RADIUS` | West-north-west |

Every corner heading is exactly `3 · HEX_RADIUS` long against `√3 · HEX_RADIUS` for an edge step, so
edges and corners together are a **uniform twelve-point rosette at 30° spacing with two alternating
lengths**. Three edge axes and three corner axes, six headings each.

**The straddle generalizes exactly**, which is what makes this safe. A north riser passes between
`(q, r-1)` and `(q+1, r-1)` and leaves both free, buildable, and walkable. The midpoint of `(2, -1)`
from the origin lands at `(1330.5, -768)` in world units, which is precisely the midpoint between the
centres of `(1, 0)` and `(1, -1)`. Same structure on all six, so the "single-cell building whose belt
spans a seam" note on `TRANSPORT_DIRECTIONS` holds unchanged.

#### What this repairs, and what it deliberately does not

`OrientationAxis::Vertical` requires a single-cell footprint, and both the Rust comment and
`AGENTS.md` explain that as "`@hexlife/embed` rotates by 60° and the vertical headings have no 60°
equivalent." That is true **only because there were two of them**: rotating north by 60° lands on
`(2, -1)`, which was not in the table, so the only available turn was the 180° flip between north and
south. With all six present the corner group is closed under 60° rotation and that explanation stops
being true. `src/rendering/buildingLook.ts` already says the quiet part — _"There is no third
vertical heading, so these are named rather than indexed"_ — and `DUE_NORTH` / `DUE_SOUTH` become an
indexed table like the edges.

**Do not lift the single-cell restriction in the same change.** No shipped definition wants a
multi-cell corner-heading building, and lifting a constraint nobody is pushing against is how an
untested path ships. What must change is the _reason_: replace the now-false explanation with "no
definition needs it yet", in the Rust comment, in `src/core/definitions.ts`, and in `AGENTS.md`, so
the next person does not inherit a justification that no longer holds. Whether to lift it is a
separate, deliberate call with a definition asking for it.

#### Three things to decide rather than assume

1. **Index order versus saved orientations — settled by the release train.** `OrientationAxis::next`
   advances by `offset + 1 % span`, so it assumes index order _is_ rotation order. Putting the six
   corners in the rotational order above changes index 7 from South to ENE, and every saved riser at
   orientation 7 would silently re-aim. **v0.21 and v0.22 ship as one version train** — v0.21 moves
   `WORLD_GENERATOR_VERSION` and rejects every existing save already, so v0.22 rides that break and
   the rotational ordering costs nothing. Take the clean order; do not build a lookup table to
   preserve a compatibility that the train has already spent.
2. **`hex_line_vertical`'s determinism argument does not survive.** It uses `.find()` over
   `TRANSPORT_DIRECTIONS[NORTH..]` — first match wins — justified by _"north and south are opposites,
   so at most one of them can ever close, and the choice cannot depend on iteration order."_ With six
   corner headings that sentence is no longer a proof. A spot check of the 30° boundary suggests at
   most one still closes by two, because a target 30° off a corner heading is an edge heading and
   there neither corner closes — but **that is a spot check, not a proof.** Either prove it and write
   the new argument into the comment, or add an explicit tie-break, and pin it with an exhaustive
   test over the corner headings. This is the one place in the change where a wrong assumption makes
   a drag depend on table order, which is exactly what the existing comment forbids.
3. **`Vertical` is now the wrong name.** `Corner` or `Vertex` is what the axis is. That is an
   `orientation_axis` value in `definitions.json` and a definition-version bump, plus the
   `ORIENTATION_AXES` set in `src/core/definitions.ts`, the `OrientationAxis` union in
   `src/core/types.ts`, the `"North / south"` catalogue chip, and the "risers run due north and
   south" line in the transport tool's blurb.

#### The fixture has to grow

`fixtures/hex-directions.json` pins only the six edges. The two corner headings are currently
duplicated by hand in `buildingLook.ts` as `DUE_NORTH` and `DUE_SOUTH`, and **nothing checks that
they agree with Rust.** Widening to twelve is the moment to fix that: pin all twelve in the fixture,
with index, name, and axial offset, asserted from both languages exactly as
`public_direction_protocol_matches_cross_language_fixture` already asserts the six. Adding four
hand-copied vectors to a host file with no cross-language guard would be the defect this milestone
introduces.

#### What does not change

`hex_line_vertical` scans `TRANSPORT_DIRECTIONS[NORTH..]` generically rather than special-casing two
entries, so it needs no structural change beyond point 2 above. `VERTICAL_TIP_SCALE = 1 / √3` is
correct for all six, since the length ratio between a corner heading and an edge step is identical
for every pair. `DIRECTIONS` stays six and must never widen — adjacency, power, boiler and turbine
neighbours are unchanged, and only transport gets twelve.

The economics are unchanged and should still be re-measured. A riser gains four headings at no extra
cost, but the trade is the one north already offered and which was already accepted: travelling ENE
is two belts for 2 iron ore across two hexes, or one riser for 2 iron ore across one hex with the
straddled pair left free. That deal is being applied symmetrically rather than sweetened, so
`fixtures/balance.json` is predicted not to move — run `npm run balance` and confirm the prediction
rather than assuming it.

### What moved out of this milestone

**Fractional deposits into containers shipped in v0.20.1** and are not here. The radius **chips** in
step 4 above share that pass's `itemChip` markup conventions but are a building's stat line rather
than an item, so they stay here with the rings they belong to.

### Acceptance

- A bridge crosses shallow water, carries a belt, refuses deep water, and
  `fixtures/terrain-passability.json`
  is unchanged.
- The wire fixture is regenerated, its diff read, and `Bridge` sits last in `BuildingKind`.
- A forest reads as trees at ordinary zoom, thins as it is cut, and recovers as it regrows.
- Every radius any definition states is drawn as a ring when pending and when selected, and is
  stated as a number on the catalogue card — `extract_radius` on the base extractor and the pump,
  and `pole_reach` on all three poles, none of which appear anywhere today.
- A pole's supply disc and its link distance are visually distinguishable, because one is an area of
  effect and the other is not.
- Transport routes on twelve headings: six edge steps at `√3 · HEX_RADIUS` and six corner steps at
  `3 · HEX_RADIUS`, in rotational order, and a riser can be turned to all six corners.
- `fixtures/hex-directions.json` pins all twelve with index, name, and axial offset, and both
  languages assert against it. No routing vector is written by hand in a host file.
- A corner drag resolves to the same cells every time, and the reason is a stated argument or an
  explicit tie-break rather than the superseded "north and south are opposites".
- The single-cell footprint rule still stands, and every comment explaining it says "no definition
  needs it yet" rather than the 60°-rotation reason, which is no longer true.
- `npm run balance` is re-run and the prediction that nothing moves is confirmed or corrected.

## Then — Earned Insight v0.23

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

## Deferred — Living Lattice v0.24

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

**Four things it should know before it starts**, all handed over by earlier milestones rather than
invented here:

- **Play the opening first, with hands, and time it.** See the open items above — this playtest has
  been owed since v0.18 and no system should be added before it.
- **`Economy::recipe_for` still asserts one recipe per item, and ecology is what breaks it.** A
  byproduct is a second producer, and "what does a plate cost" has no answer without a stated rule
  for dividing a craft's cost between its outputs. `outputs: Vec<Ingredient>` arrives here for the
  same reason. Pick the allocation rule deliberately and write it down beside the fixture; do not
  make the secondary output free, charge every output the full craft, or silently select one
  producer. The `contracts` and `requests` sections expand bills through the same tree, so they
  break in the same place and for the same reason.
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

## Later — Regional Discovery v0.25

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

Named as decisions rather than omissions, each with the thing it is waiting for.

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
- **Terraforming.** Whether the player may reshape elevation, and what that costs.
- **Tunnels.** One match arm in the graph trace — `None if entity.is_tunnel() && steps < span`, so a
  tunnel entrance rays through empty ground and binds to the first entity it reaches, the covered
  cells stay walkable, and pipes inherit it for free when fluids land. It may ride any compatible
  version bump and does not become a milestone by itself.
- **Organic tileables.** The later half of the art generator: systems that produce tileable textures
  and shapes so a hex lattice reads as organic terrain and organic objects. Same invariants —
  generated, presentation-only, derived from published snapshot facts, never a checksum input.
- **3D presentation.** The camera tilts and orbits the player; terrain, buildings, and the player
  gain shape. This is a renderer replacement, which the invariants already allow, and the WebGL2
  pass is a step toward it. Height is not implied as a gameplay dimension until a later pass names
  what it is for — smuggling a z-axis into the checksum because the camera can tilt would be the
  same class of defect as frame-coupled movement. A hand-authored mesh per definition is the atlas
  again; a mesh derived from `recipe_category` and tier is the shape grammar in another dimension.
  A renderer decision is a measured decision.

Whatever comes next, `fixtures/balance.json` remains the thing every new building or recipe has to
face: a definition that never reaches it is a definition nothing has compared against the curve, and
both test suites say so.

## Shipped ledger

One line per release, newest first. The reasoning behind a shipped rule lives in the git history of
this file and in the code that implements it; what follows is the index.

- **WebGL2 renderer** (unreleased, 2026-08-20) — The world and the minimap draw as instanced GPU
  geometry with the camera as a uniform, so walking no longer restamps the terrain mosaic; a Canvas
  2D overlay keeps the player, labels, and machine decorations. Fixes the 4 tps sluggishness
  (per-frame fog blur, camera-keyed terrain restamp, layout forced by `pickWorld`, full HUD rebuilds
  while walking). **Not yet benchmarked.**
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

From `npm run survey` at seed 1,213,486,160 and radius 96 (27,937 hexes). Bands in parts per
thousand; water as bodies / mean body / largest body; "furthest" is the distance from the landing
site to the most distant of the eight materials. **v0.21 re-authors all four presets**, so this table
is the before-figure that change is measured against.

| preset      | water | shore | lowland | hills | highland | cliff |   water bodies | furthest material |
| ----------- | ----: | ----: | ------: | ----: | -------: | ----: | -------------: | ----------------: |
| Continental |    57 |   126 |     345 |   324 |      138 |     7 |  191 / 8 / 104 |                23 |
| Archipelago |   230 |   175 |     274 |   207 |       81 |    29 | 575 / 11 / 179 |                23 |
| Highlands   |    10 |    26 |     216 |   398 |      315 |    32 |    41 / 7 / 46 |                32 |
| Basin       |   103 |   131 |     365 |   260 |      106 |    31 | 28 / 103 / 997 |                25 |

`cliff_step` is a gradient threshold and a gradient scales with feature size, so a step tuned for
Continental's cell 8 means "sheer" at cell 4 and "nothing is ever steep" at cell 20. That is what
made Basin generate zero stone and Archipelago read 182 per mille cliff on their first draft, and it
is the class of error the survey exists to catch.
