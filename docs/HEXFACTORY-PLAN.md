# HexFactory — architecture, roadmap, and implementation handoffs

Status: Look Systems v0.13.1 is shipped on Power v0.13, Renderer Measure v0.12.4, Sightlines
v0.12.3, Binary Delta v0.12.2, Playtest Feel v0.12.1, Material Base v0.12, World Shape v0.11,
Playability v0.10, Game Feel v0.9, Browser Capacity v0.8, Sparse Snapshot v0.7, Sparse Cost v0.6,
Capacity Tiers v0.5.1, Worker Boundary v0.5, Command Surface v0.4, Continuous Exploration v0.3,
and the v0.3.1 incremental transport follow-up.
The world now produces eight raw materials, each where its geography says it should be, and fourteen
recipes across five machine categories turn them into something the player wanted. The compact
binary delta encoding landed on the v0.12/v0.13 boundary the roadmap named as its deadline: the
per-frame payload is 13.6× smaller. **A complete browser frame is a measured number**: 19.0% of
60 Hz at the largest tier after Look Systems, of which the world draw is 991 µs against the
v0.12.4 baseline of 909 µs. The world is the view: shorelines come from neighbours, buildings
from `recipe_category`, and a worked-out field is a scar rather than a missing glyph.

**Upgrades and Tiers shipped as v0.14.** Tiered definitions with an in-place `upgrade` that
conserves items exactly, extraction reach as the flagship upgrade, north and south in the transport
direction table as the riser, a right-click that harvests one named hex, and two-way hand transfer
with containers. Save version 7, definition version 7, technology version 4. The shipped record is
the section below; the brief it was built from is kept beneath it.

**The generator arc is finished: Generated Shapes as v0.15, World Parameters as v0.16, and Balance
as v0.17.** None of the three was a play system, and each replaced a hardcoded thing with a
generated one. A building's drawing is a part list from an eight-part vocabulary that a tier
modifies. A world is a seed plus a `WorldParams` that travels in the save and the checksum,
`WORLD_GENERATOR_VERSION` is 6, four presets ship as data rows, and `npm run survey` is what every
claim any of them makes about its own landscape comes from. **And the economy now states its own
curve**: `fixtures/balance.json` is every figure that decides whether the numbers work — items per
minute, what a generator carries, and the full raw-material cost of every building expanded
through its whole recipe tree — computed from the shipped catalogues and pinned in both languages.
Definition version is 8, because the first pass at the numbers moved six of them.

**The roadmap pivoted from substrate to motive, and the first milestone of that pivot is shipped.**
A 2026-08-19 hands-on play review found a responsive editor and a clean factory substrate, but the
playable purpose was still the founding vertical slice: three Components made from Iron ore,
followed by free build. Most of the material base, power choices, tiers, and world variation could
be ignored before that finish, and the opening exposed the whole locked roster before the player
had a reason to want it. The next three milestones spend the generators on play, in order:
**Founding Contract v0.18** repairs the first twenty minutes and makes the landing hub visibly grow;
**Living Lattice v0.20** turns animals/biomatter/waste into one coupled ecological system;
**Regional Discovery v0.21** makes an unbounded world something the player must read and travel
through rather than a larger background. Fluid networks and intermittency remain behind those
milestones. Tunnels are still small enough to ride a version bump rather than claim one.

**Founding Contract shipped as v0.18.** The landing hub asks for an ordered contract rather than a
delivered total: stage one is the founding three components, stage two is a foundry module of
smelted plate and fired brick from two landscapes, and finishing a stage visibly builds the hub. The
scripted guidance is gone — the next step is derived from the contract, the recipe tree, and the
technology graph, so it cannot recommend a factory the rules refuse, which the old script did every
time it named an extractor without naming power. Save version 8, scenario version 5, and the browser
save key now reads the catalogue versions instead of restating them. The shipped record is the
section below.

**Power Grid shipped as v0.19.** Electricity stopped being a per-tick tax and became energy bought
per unit of work: a machine banks three cycles' worth and spends it as it makes progress, so idle
time is free and a plant burns only for what it actually handed over. Coverage moved onto the pole,
where it can be upgraded — three rungs, 3 → 4 → 6 hexes — and machines that touch each other
conduct, so a pole is what distance costs rather than what power costs. Save version 9, definition
version 9, technology version 5, wire version 4. The shipped record is the section below.

**North-south belts are resolved and have left the longer horizon.** Due north is a lattice vector
on this grid — `(q + 1, r - 2)` shares a world-x with `(q, r)` — and `compile_graph_target` is
already a ray-cast that never assumed a unit step. So the fix is a direction-table row, not sub-hex
occupancy, and it rides v0.14's version bump. The write-up below supersedes the half-covered-tile
proposal.

## Shipped milestone — Generated Shapes v0.15

Shipped 2026-08-18. The brief below is kept as written; this section records what it became.

**The vocabulary is eight parts and the walker is the only place they become canvas calls.**
`src/rendering/shapeGrammar.ts` carries `vessel, chamber, stack, rotor, aperture, mast, band,
mouth`, each with anchor, scale, rotation, and phase, in units of the hex size rather than pixels.
`drawSilhouette`'s two-hundred-line `switch` is gone; what replaced it is `BUILDING_SHAPES`, a
`Record<SilhouetteKey, ShapePart[]>` in `buildingLook.ts`. The table is **total over the key type**,
so a new silhouette is a compile error at the data row rather than a machine that silently draws
nothing — the property that makes "a new building costs a data row" enforced instead of intended.
`silhouetteOf` and `trimOf` are unchanged; they were the half of rule 4 that was already right.

**A tier is a modifier on the part list, so an upgrade changes the machine.** `TIER_LADDER` is two
named steps — `reinforced` (plating band, vent, wider mouth) and `overbuilt` (segmented vessel,
another rotor blade, a second vent) — applied cumulatively, which is the drawing's version of the
rule `upgrade` already follows: it adds to what the player recognises rather than replacing it.
`addStack` anchors off `profileTop`, and that is deliberate: every other modifier needs a part of a
particular kind to act on, so a step built only from those could find no target and produce a tier
the map cannot show. Anchoring one modifier to the profile makes every non-empty shape grow.

**Motion moved inside the grammar.** `phase` (`spin`, `pulse`, `rise`, `grind`) is a property of a
part, so a rotor turns because it is a rotor rather than because a `switch` arm reached for
`Math.cos`. That is also what makes the bake safe to split: still parts are stamped from an
offscreen canvas behind `BUILDING_SHAPE_VERSION` (ART.md rule 3, and `TERRAIN_TILE_VERSION`'s
pattern), and only the parts that actually move are walked per entity per frame. The grammar's
indirection is therefore paid at startup, not at 60 Hz. The bake is sized at a hex of 128 px
against a camera that can reach 96.8 device pixels, so a stamp is always scaled down, never up.

**The player draws from the same vocabulary.** Its ring and body are part lists now, in units of
the player radius — the walker takes whatever unit its caller works in. The heading tick stays
outside the grammar for the same reason a building's does: it is an indicator, not anatomy.
Terrain's baked-tile system is untouched. It shipped in v0.13.1 against rules 1–3 and rewriting it
would have bought no visible gain in a milestone that must not move the frame.

**The contact sheet is `contact.html`, a dev entry point beside `bench.html`.** Twenty definitions
by three tiers by four statuses, 240 cells, drawn by the shipped renderer rather than by a second
illustration of it. It carries a **colour toggle**, because the acceptance is a silhouette
judgement and a gold stroke over an identical body would pass any test that kept the palette. It
also names two failures on the card itself: a definition whose silhouette has no base shape, and
definitions that draw identically to each other. Both fire today on the belt and the riser, which
is correct — a belt's look is its heading tick and the cargo riding it — and is exactly the kind of
thing that is invisible from the table and expensive to find by playing.

Presentation only. No command, no save, definition, generator, wire, or checksum movement. This is
Stage B's rule 4 finished rather than a new direction: the rule shipped, the mechanism did not.

Directed 2026-08-18: "actual assets, ideally generated in some way — we generate base shapes for
each building/terrain/player and then have some system to upgrade them show change in them. It's
important that everything can be maintained systematically."

### The defect, named

`silhouetteOf` genuinely derives look from the definition — `recipe_category` splits the composer
kinds, `power_source` splits the generators — and that part is right and stays. What sits under it
does not. `drawSilhouette` is a two-hundred-line `switch` whose every arm is hand-written canvas
calls, so **a new building costs a new arm**, which is the atlas again with the drawings written in
TypeScript instead of painted in a PNG. And `trimOf` renders a tier as nothing but a stroke colour
and width, so an upgraded extractor is the same machine with a gold outline. The milestone whose
whole purpose was growth in place produced no visible growth.

Neither is a bug. Both are the point at which "derived from the definition" stopped being applied
to the drawing itself.

### A shape is a part list, not a function

One renderer walks a declarative list of parts; the list is data. The primitive vocabulary is
deliberately small and names machine anatomy rather than geometry — **vessel, chamber, stack,
rotor, aperture, mast, band, mouth** — each carrying anchor, scale, rotation, and animation phase.
The phase field is what keeps Stage C's motion in the same system instead of beside it: a rotor
already spins by `workCycle`, and in the grammar that is a property of the part rather than a
bespoke arm.

Then the composition rule is three lookups and no cases:

- `kind` / `recipe_category` / `power_source` selects the **base part list** — a data row, exactly
  as `silhouetteOf` already selects a key today.
- `tier` applies **shape modifiers** from a named, documented set — add a stack, add a rotor blade,
  segment the vessel, add a plating band, widen the mouth. A tier changes the silhouette, not the
  stroke. This is the half of "growth in place" the map never showed.
- Terrain and the player draw from the same vocabulary, so the world reads as one visual system
  rather than three that happen to share a palette.

Baked to offscreen canvases behind a version constant, which `TERRAIN_TILE_VERSION` in
`src/rendering/terrainLook.ts` already establishes as the pattern. Changing a constant regenerates
the set.

### The contact sheet is what makes it maintainable

A dev page that renders **every definition × every tier × every status** on one grid. It reuses the
renderer, so it is cheap, and it is the only way to see that two buildings read alike or that a
tier modifier changed nothing visible — without playing the game and happening to build both.
"Maintained systematically" is this artifact; the grammar alone is only half of it.

### What this is not

Not an atlas, not hand-authored per definition, not 3D, and not a renderer replacement. Still
Canvas 2D. Rendering consumes snapshots and owns nothing, so none of this can reach a checksum by
construction — that invariant is what makes generated art free here rather than risky, and it is
unchanged.

### Acceptance

- A new building definition with no new drawing code renders as a distinct, readable machine.
- A tier-1 definition is distinguishable from its tier-0 parent **by silhouette**, at normal zoom,
  with colour removed.
- The contact sheet renders the full roster and is committed as a dev entry point.
- `npm run bench:browser` re-measured against the v0.13.1 record. The grammar adds an indirection to
  a per-entity draw; the frame is a number in this project and stays one.

**How each was met.** The first is structural and asserted: `BUILDING_SHAPES` is total over
`SilhouetteKey`, and a test pins that no `switch` over silhouettes survives in `buildingLook.ts`
while the only two left in `shapeGrammar.ts` are over the fixed part vocabulary, which does not
grow per definition. The second was **measured, not eyeballed** — the contact sheet's cells were
read back pixel by pixel with colour off, inside a disc that excludes the hex body's own
tier-coloured stroke, and every shaped definition gains ink and lifts its topmost drawn row at each
step (extractor 34 → 30 → 25, smelter 30 → 26 → 21, belt and riser zero throughout by design). The
first attempt at that measurement did not exclude the body stroke and reported the shapeless belt
changing by 32%; isolating the silhouette is what makes the figure mean anything. Seven tests were
added, taking the TypeScript suite to 71.

**The fourth acceptance criterion is met with a caveat that has to travel with it.** The re-measure
ran, and it is **not comparable to the v0.13.1 record**: every pinned condition matches, but the
browser does not — v0.13.1 was `Chrome/151`, this run is `Chrome/148` inside `Electron/42.9.2` in a
non-compositing pane, and the absolute figures are about twice v0.13.1's _including the host frame
and the minimap, neither of which this milestone can touch_. So the useful measurement is a
same-machine A/B against the `switch` it replaced: world draw at the largest tier averaged 1,767 µs
for the grammar against 2,057 µs for the `switch`, with the ranges overlapping. **No regression is
detectable; the grammar is not demonstrably faster either.** The minimap, which never imports the
grammar, held at 428 µs ± 2.6% across all five runs and is what makes the A/B trustworthy. All six
tier checksums are identical to the v0.13.1 record, which is the presentation-only claim
demonstrated rather than asserted. v0.13.1 stays the current browser-frame record, and a comparable
re-measure on `Chrome/151` in a composited window is owed. Full write-up in `docs/BENCHMARKS.md`.

One thing worth naming for whoever picks this up: the tier ladder is cumulative and unbounded in
principle but has two steps, so a definition at tier 3 would wear the same shape as one at tier 2
while its trim kept climbing. That is deliberate — a visibly odd machine beats a silent duplicate —
but the roster ships no tier above 1 today, and the day it does, the ladder needs a third row
rather than a wider `trimOf`.

## Shipped milestone — World Parameters v0.16

Shipped 2026-08-18. The brief below is kept as written; this section records what it became.

**A world is a seed and a parameter set, and the checksum says so.** `WorldParams` carries twelve
integers and the resource table, `Core` holds one, `terrain_at` and `field_at` take it, and
`hash_world_params` folds every field of it — the rule rows included, in order — into the checksum
beside the seed. It is in `SavedState`, so a save records the generation its overlay was cut from.
`WORLD_GENERATOR_VERSION` is **6**: a version-5 envelope names no parameters at all, so it cannot
be read as the default set and is refused. `SAVE_VERSION` stays 7, and the browser's save key is
now `hexfactory:hxf1:v7w6`, which retires a stored v7/w5 save instead of offering a Continue button
that can only fail.

**Feature scale and threshold are separate axes, and the acceptance test is the proof.** Sea level
decides how _much_ water there is; the coarse elevation octave's cell size — and its share of the
blend — decides how _big_ it is. `feature_scale_makes_seas_and_sea_level_only_makes_more_ponds`
asserts both halves against measured body counts. Moving only the cell size from 4 to 24 at a fixed
sea level took water from 209 bodies averaging 10 hexes to 15 bodies averaging 84, with the total
water within a factor of two either way. Moving only the sea level from 18,000 to 26,000 took water
from 1,630 hexes to 6,905 while the body _count_ barely moved, 191 to 236. **The measurement that
separates them is body count, not largest body** — whether one landform in a sample happens to dip
under the sea swung that figure by 3× across a sweep whose trend was otherwise perfectly monotone,
and the first version of the test asserted on it and failed for that reason.

**Resource commonness is a table.** `field_at`'s eleven `match` arms are `FieldRule` rows — terrain,
item, three gates, base, spread — evaluated in declared order, first match wins. The hazard that
used to live in a prose comment is now the row order itself: clay is the leftover of the band wood
takes first, and `field_rule_order_decides_which_band_holds_what` swaps the two rows and asserts
that clay's cell count rises and the checksum changes. Gates use `ANY = -1` rather than 0, because
noise samples zero at a lattice point and a zero gate would be wrong once in a billion hexes and
never reproduce.

**The survey tool found two unplayable presets before a player could.** `npm run survey` samples a
disc for a parameter set and reports the band histogram, field density per material, nearest and
mean distance from the landing site to each, and the count and size of connected water bodies. Run
against the first draft of the presets, it reported that **Basin generated zero stone anywhere in a
27,937-hex sample** and that **Archipelago read 182 per mille of the world as cliff**. Both came
from the same cause: `cliff_step` is a gradient threshold, and a gradient scales with feature size,
so a step tuned for Continental's cell 8 means "sheer" at cell 4 and "nothing is ever steep" at
cell 20. That is precisely the failure the brief predicted, and it was found by counting rather
than by playing. `--set name=value` surveys a parameter set nobody shipped, which is how the fixed
numbers were chosen instead of guessed.

**The shipped presets, measured at seed 1,213,486,160 and radius 96 (27,937 hexes).** Bands in
parts per thousand; water as bodies / mean body / largest body; nearest is the furthest any of the
eight materials sits from the landing site.

| preset      | water | shore | lowland | hills | highland | cliff |   water bodies | furthest material |
| ----------- | ----: | ----: | ------: | ----: | -------: | ----: | -------------: | ----------------: |
| Continental |    57 |   126 |     345 |   324 |      138 |     7 |  191 / 8 / 104 |                23 |
| Archipelago |   230 |   175 |     274 |   207 |       81 |    29 | 575 / 11 / 179 |                23 |
| Highlands   |    10 |    26 |     216 |   398 |      315 |    32 |    41 / 7 / 46 |                32 |
| Basin       |   103 |   131 |     365 |   260 |      106 |    31 | 28 / 103 / 997 |                25 |

Continental is version 5's frozen numbers unchanged, so the default world is the world the game
already had. Basin's 28 bodies averaging 103 hexes against Archipelago's 575 averaging 11 is the
milestone's claim in one row of a table.

**Every preset reaches all eight materials, asserted rather than assumed.** `LANDING_FIELD` still
guarantees one cell of each at the clearing, and
`every_preset_reaches_every_material_from_the_landing_site` checks that promise under every preset
_and_ the stronger claim behind it: that the generator itself puts every material within 40 hexes
of the landing site, and that barren ground stays the common case at v0.15's density floor and
ceiling. A preset that made a band scarce is not allowed to make its materials unfindable, which is
what `relaxed` does — it lowers the gates on one band's rows, so Highlands' thin shore holds sand
and clay densely and Archipelago's broken highland holds its ore.

**Validation refuses parameter sets that are not worlds; it does not pretend to judge playability.**
Cells outside 1..64, a weight that is not a percentage, band cuts out of ascending order, an empty
rule table, an unknown item, a zero spread. Bands out of order do not make a band rare — they make
it unreachable — so that one is a refusal rather than a warning. Whether a valid set is _playable_
is what the survey measures, and no validator can decide it.

**The new-world flow is a preset picker with the raw parameters behind it.** The preset table is
native's, served by `Factory.world_presets_json()`, so the host renders a table it cannot drift
from — the same relationship the catalogue has to the definitions. Editing any of the twelve
scalars switches the picker to Custom. `world_params_json()` is a getter rather than a snapshot
field: a world's parameters change only when the world does, so the host asks after `new_game` and
`load` instead of paying for them on every frame. The resource table comes from the preset whole;
the panel says so.

**Measured.** The native ladder is unchanged: xlarge frame 1,416.8 µs against v0.13's 1,455.5 µs,
tick 378.1 against 361.2, delta bytes identical at 49,476, delivered totals identical. The ladder
was run twice and the two runs differ by about 5% at that tier, which is the host's resolution and
is recorded in `docs/BENCHMARKS.md` rather than smoothed away. That is
evidence the parameter indirection costs nothing in the tick, snapshot, and delta path — and it is
**not** evidence about generation, because the ladder's scenario sets `generated_environment: false`
and never calls `terrain_at`. Generation was measured separately and for the first time: surveying
radius 48 (7,057 hexes) and radius 96 (27,937 hexes) with the same binary, five runs each, medians
10.6 ms and 18.0 ms, gives **0.35 µs per hex** for terrain, field, and the survey's own bookkeeping
— an upper bound on generation alone. A chunk is 64 hexes, so generating one costs at most ~23 µs
and the seven-chunk neighbourhood refresh at most ~160 µs, and only when the player walks into new
ground. There is no prior figure to compare it against; v0.15 and earlier never measured this path,
so this is a first record and not a regression claim.

Directed 2026-08-18: "generate new worlds with parameters, like how common certain resources and
terrain types are, how water and other biomes show up, in large lakes/seas/oceans or just ponds."

The brief this was built from follows.

Unlike v0.15, this is **simulation truth**. That is the line to hold across both milestones: art
parameters are free to vary because presentation owns nothing, while world parameters are part of a
run's identity and therefore saved, checksummed, and covered by the envelope.

### What generation does today

Every number is a literal. `terrain_at` reads bands off hardcoded thresholds — `elevation < 18_000`
is water, `< 24_000` is shore, `> 42_000` is highland, `> 33_000` is hills, and a gradient
`> 14_000` is cliff. `elevation_at` is frozen at `value_noise(cell 8) / 2 + value_noise(cell 3) / 2`
and `moisture_at` at `cell 7`. `field_at` is a `match` of hardcoded richness, moisture, and vein
gates with literal quantity ranges. The seed is the only thing a world can differ by.

### Feature size and threshold are two knobs, and today they are one

The most important thing this milestone gets right, because it is the thing that is easy to get
wrong: **raising sea level makes more water, not bigger water.** It produces more ponds. Large
lakes, seas, and oceans come from a **larger elevation feature size** — the `cell` argument to
`value_noise` and its weight in the blend — which is exactly what is frozen at `8 / 2 + 3 / 2`
today.

So the parameter set separates them, for every band and not only for water:

- **Feature scale** — the low-frequency octave's cell size and its share of the blend. This is
  "ponds or oceans", "hillocks or ranges".
- **Thresholds** — where the bands cut. This is "how much".

Both are integers, both feed the existing pure `value_noise`, and neither changes the sampling
contract: a hex still needs no neighbour outside its chunk.

### Resource commonness becomes a table

Lift the `field_at` match arms into a `FieldRule` table — `terrain`, `item`, `moisture_min`,
`richness_min`, `vein_min`, `base`, `spread` — evaluated in declared order. This is what makes
"how common is copper" a parameter instead of an edit. It also makes an existing hazard explicit
rather than a comment: clay's richness gate **must** sit below wood's or wood takes every cell, and
right now that ordering is load-bearing and documented only in prose at `field_at`.

### Presets are what a player picks; parameters are what makes a preset a data row

Ship named presets — Archipelago, Continental, Highlands, Basin — as rows, with the raw parameters
exposed behind them in the new-world flow. A preset is the usable surface; the parameter set is the
maintainable one. Same relationship the shape grammar has to a building definition.

### Tune against a measurement, not a guess

Value noise is not uniformly distributed, so **a threshold is not a proportion**. A tool that
samples N hexes for a parameter set and reports the actual band histogram, field density per item,
and mean distance from the landing site to each material is a requirement of this milestone, not a
nicety — it is the only way a preset can claim to be what it says it is. This is the same
measured-before-claimed rule the frame budget and the capacity ladder already live under, applied
to the generator.

### Biomes — what is in, and what is deferred

What the direction asks for — water and the other bands appearing at chosen scales and
frequencies — is delivered by feature scale plus thresholds above, and applies to every band.

What is **not** in this milestone: a third low-frequency channel (temperature, or a categorical
region noise) that would let the same elevation band read and yield differently in different parts
of the world — a dry highland here and a forested highland there. Today elevation × moisture
produces bands, and bands are the same everywhere they occur. That is a genuine design pass with
its own questions about how a region announces itself to a player who is walking into it, and it is
added to the deferred list below with its trigger named rather than smuggled in beside a
parameterization.

### Costs named

- **`WORLD_GENERATOR_VERSION` goes 5 → 6, and `WorldParams` enters the save envelope and the
  checksum.** A world whose parameters differ is not the same world, so version-5 envelopes are
  rejected. That is the behaviour already in place and it is correct; say so in the notes.
- **The landing clearing has to survive every parameter set.** `LANDING_FIELD` guarantees one cell
  of each of eight materials so the first hour of any seed reaches every tier-1 recipe on foot.
  Under a parameter set that makes a material rare, that guarantee is doing more work than it does
  today and must be asserted, not assumed.
- **A parameter set that generates an unplayable world is a real failure mode** — no reachable
  coal, or a landing site in the middle of an ocean. The histogram tool is what makes it detectable;
  the presets are what make the default safe.

### Acceptance

- Two worlds on the same seed and different parameter sets are visibly different landforms.
- `water_scale` low produces ponds and high produces contiguous seas, **at a fixed sea level** —
  the claim this milestone rests on, asserted directly.
- The band histogram for each shipped preset is recorded in the notes, not estimated.
- Every preset reaches all eight raw materials from the landing site, asserted.
- Save round-trip and checksum cover `WorldParams`; a version-5 envelope is rejected.

## Shipped milestone — Balance v0.17

Shipped 2026-08-19. The brief below is kept as written; this section records what it became.

**`fixtures/balance.json` is the representation balance did not have.** It carries nine sections —
the reference units, machine rates, power plants, fuel conversions, item costs, building costs, the
curve, material access, site yields, and the openings — computed from `definitions.json` and
`technologies.json` by `factory-wasm/src/balance.rs` and asserted in test. `npm run balance` prints
the readable form. Regenerate with `UPDATE_BALANCE_FIXTURE=1 cargo test balance_fixture`, then
`npx prettier --write fixtures/balance.json`, because serde and prettier disagree about short
arrays; the assertion is over parsed JSON, so the formatting pass cannot change what it checks.

**It lives in Rust because every figure in it is a restatement of a rule the tick implements.**
Items per minute restates `advance_composer`, machines carried restates `power_progress`, and a
site's yield restates `deposit_candidates` walking its whole list regardless of which material a
cell holds. Computing those in the host would be a second implementation of native truth, which is
the defect this project has refused everywhere else. The module is native-only for the same reason
the survey and the capacity ladder are: nothing in it runs a tick, and the wasm artifact must not
carry it.

**The expansion is exact rational arithmetic, and it rounds once.** A kiln fires three bricks at a
time, so a pump wanting four costs four thirds of a batch; rounding that at every level of a tree
and then comparing two buildings compares rounding errors. Costs are carried as `i128` fractions
and written down in thousandths, rounded half up, at the edge. Both the proportional cost and the
**batch** cost are reported — the second is what a player actually spends, crafting whole batches
and eating the leftovers, and the gap between them is the lumpiness.

**TypeScript recomputes the arithmetic rather than reading it back.** `tests/balance.test.ts`
implements the tree expansion, the fuel round trip, and the machine rates a second time, against
the same `definitions.json`, and asserts it lands on the same numbers. That is what makes the
fixture evidence: one implementation agreeing with its own output would say only that the file was
written. What is _not_ recomputed there is what a preset generates and what a site is worth, because
re-deriving those in the host would be a second world generator.

### What the tool found in its first run, and what moved

Six numbers changed. Every one of them is traceable to a printed figure, and the pre-tuning report
is what each argument is made from.

- **The player's hands beat the first machine that replaces them, by two and a half times.** The
  hand gathered at 300 items a minute against an extractor's 120 at the default 10 tps, working the
  same seven cells. That is a curve inversion at the very start of the game and no cost row could
  ever have shown it, because it is not a cost. `GATHER_COOLDOWN_STEPS` goes 6 → 15, which makes the
  hand worth **exactly one extractor**, and the test asserts the equality rather than the direction.
  What automation buys is not a bigger number; it is that the player can walk away. This is the one
  change outside the definition file — the constant is code, and moving it into the scenario beside
  `carry_slots` and `build_range` is the obvious next step for whoever wants a scenario to set its
  own pace.
- **Steam, the described "mid-game workhorse", was strictly dominated by the generator before it.**
  A boiler drinks one water every tick it runs, and the pump made one every six, so a steam plant
  needed **six pumps drawing 24 of the turbine's 48** before a single machine ran. Net 24 — against
  a hydro generator that costs _exactly the same as the boiler alone_ (both 6 iron plate and 4
  brick, effort 18.333 to the thousandth), produces 36, and needs neither fuel nor plumbing. Pump
  cadence goes 6 → 2, so one boiler is fed by two pumps, and hydro's output goes 36 → 24. The power
  ladder now reads wind 12, burner 20, hydro 24, steam 40 net — free power is capped and sited,
  fuel-burning power scales, and the plant that costs the most to run carries the most.
- **The wind turbine cost more than the hydro generator for a third of the output.** Effort 20
  against 18.333 for 12 power against 36. Siting excuses a difference; it does not excuse paying
  more for a third as much. Its cost drops to 2 iron plate and 1 gear (effort 10), which makes it
  what it is described as: the cheap fuel-free trickle you can put on the highland the ore sits on.
- **Charring wood returned exactly the energy it consumed.** Two wood at 2 energy each into one
  charcoal at 4 — a gain of 1.000×, for a kiln, ten ticks, and a hundred power. A fuel conversion
  that breaks even is a recipe with no reason to run, and fuel being a property of the item is
  precisely why nothing in a recipe row could notice. Charcoal's `fuel_value` goes 4 → 8, so the
  conversion doubles, and the one renewable material in the game becomes a fuel worth processing.
- **The cutter cost less than the smelter it is unlocked two technologies behind.** Effort 11.5
  against 12. Its stone goes 4 → 6, which puts the mechanical-shaping pair above the
  material-processing pair while keeping the cutter the cheaper of the two shaping machines. This is
  the one break the curve rules caught on the shipped data, and putting the 4 back is the negative
  case the curve test asserts against.

### The curve is two rules, and only two

"Meant to follow" had to be a claim about the data rather than a mood, or a tuning pass ends up
inflating numbers to satisfy an ordering nothing in the game asserts. So: a tier costs **strictly**
more than the tier it upgrades from, and a machine costs **no less** than a machine of the same
`kind` whose technology it is unlocked behind. A cutter does not follow a kiln — they sit beside
each other in the same tech tier — and the rules say so by not comparing them.

Cost is one scalar, `effort`: raw units plus fuel energy priced in the densest fuel item. Every raw
unit counts once, which is the only weighting the data supplies. An insight value is a research
price, not a scarcity, and inventing a scarcity weight would be tuning by feel inside the tool that
exists to stop it.

### Measured, not argued

- **Every recipe input is reachable from the landing site under the default preset**, and reachable
  is the second of two questions. The world generating some is the first; something being able to
  stand where it reaches any is the second, and stone is why — it is quarried off cliffs nothing can
  stand on. The check asks for a non-blocking hex within the one reach every extractor and the
  player's own hand share, and water is asked the pump's question instead, since a pump stands
  beside a basin and never in it. Water's nearest basin is 3 hexes from the landing site, inside the
  clearing radius.
- **Site yields are sampled, not derived from the rule table**, over the same radius-96 disc the
  survey uses and under all four presets. An extractor empties every field cell in reach whatever
  material it holds, so the figure is the whole site: 118 units at reach 1 and 237 at reach 2 for an
  iron site under `continental`, of which 81 and 132 are actually iron. The rest arrives on the same
  belt, which is a fact about a layout rather than a rounding note.
- **The landing clearing is 258 units on every seed** — the entire bootstrap budget before a belt
  exists — and it is reported separately from geography for the same reason the survey excludes it:
  it is a promise, not a landscape.
- **The openings are floors and say so.** First smelter is 19 gathers and 14 insight; first power is
  13 gathers and 7 insight; first circuit is 39 gathers, 22 insight, two machines, and 58 machine
  ticks. Walking is excluded, so what the number leaves out is exactly what a playtest measures.

### The save key names every version the envelope refuses on

`SAVE_KEY` becomes `hexfactory:hxf1:v7w6d8t4`. Native refuses a load on four numbers — save,
world generator, definition, and technology — and the key carried only the first two. v0.17 moves
the definition version and nothing else, so a v0.16 save would have sat under an unchanged key
behind a Continue button that could only fail, which is the exact failure v0.16 named the key to
prevent. Verified in the browser: a v0.16 envelope under the old key leaves Continue disabled and
the status reading "No local save yet", and a v0.17 save round trips through Continue at an
identical checksum.

### Gate

`npm run quality` green: 83 Rust tests and 78 TypeScript tests, lint, format, build. Six Rust tests
and six TypeScript tests are new, and the curve test carries its own negative case — it puts the
cutter's stone back to four and requires the curve to break.

No bench re-measure. The rule asks for one when a milestone changes the world generator, the item
roster, or the entity snapshot; this one changes none of those. The roster is the same
twenty-three items, the snapshot shape is untouched, and the wire fixture is unchanged.

Browser verification was partial for the third session running: the pane reports
`document.hidden`, so `requestAnimationFrame` never fires and the frame loop cannot run, which
means the gather rate itself could not be measured in a browser. What was verified there is the
boot with both catalogues validating at definition version 8, no console errors, the tuned numbers
reaching the player where they are read (hydro `+24 power`, wind `×2 Iron plate ×1 Gear`, cutter
`×3 Iron plate ×6 Stone`), the simulation advancing across the worker boundary under the step
button, and the save behaviour above. The cooldown itself is asserted natively, by a test that
already existed and one that is new.

## Roadmap decision — play after the generator arc

Decided 2026-08-19 after a hands-on pass through the opening, the panels, the Factory demo, the
construction drag, and the generated-shape sheet. The build is already a strong editor: native
preview and placement agree, a dragged belt run is one precise gesture, the inspector is readable,
the world remains the main surface, and the runtime is clean. Those are not the next problem.

The problem is motive. `new-game` still ends at three Components, and a Component is two Iron ore.
That objective proves the original transport slice and almost none of the game that now exists.
Delivering arbitrary material for one generic insight currency also makes the opening's eight raw
materials differ less than their geography claims, while guaranteeing all eight in the landing
clearing answers exploration before the player has asked it. Open-ended play is still the goal, but
**open-ended must not mean aimless**: a victory may be a milestone rather than a wall only when the
world continues to offer visible things worth choosing.

One correctness defect is part of this diagnosis, not an unrelated polish item. After Automated
Extraction, `renderNextAction` says to build a supply line with extractors and belts. An extractor
draws power, `power_progress` returns zero without a network, and On-site Power is a separate branch
the guidance never names. The game can therefore recommend a factory that cannot run. No scripted
guide is allowed to outrun the rules it is explaining.

The next work is three milestones in dependency order. v0.18 gives the existing economy a purpose
and proves the first twenty minutes. v0.20 adds the first genuinely new play system and makes the
world answer the factory. v0.21 makes distance and regional difference earn the unbounded map. Do
not pull fluids, intermittency, a day cycle, or 3D in front of them.

That order gained a milestone at the front. **Power Grid v0.19** was pulled in ahead of Living
Lattice because electricity was the one substrate the pivot had not touched, and the ecological
milestone would have been built on top of it: a byproduct loop metered by a per-tick tax that
charges idle machines is a loop whose costs no player could read. It is the last substrate
milestone, and it was asked for from play rather than from this document.

It then gained three more, for the same reason and from the same source. **The numbers in the
paragraph above are superseded** — see "Roadmap decision — the world the economy stands on" below,
which renumbers Living Lattice to v0.24 and Regional Discovery to v0.25, and puts Landforms and
Fields v0.21, Crossings and Canopy v0.22, and Earned Insight v0.23 in front of them. The reasoning in
this section stands; only the ordering moved.

## Shipped milestone — Standing Requests v0.20

Shipped 2026-08-19, and asked for from play rather than from this document: pressing `X` beside the
hub emptied the whole pack into it and handed back a generic currency at a rate nothing on screen
ever stated. That is the defect the roadmap decision above already named — "delivering arbitrary
material for one generic insight currency makes the opening's eight raw materials differ less than
their geography claims" — so it is fixed here rather than inside the ecological milestone.

**Insight is no longer a property of an item.** `insight_value` is gone from every item row. The
landing hub posts a board of three **requests** — a named quantity of one item, for a stated number
of insight — and filling one is the only thing in the game that pays. `definitions.json` ships
twenty-two rows, from `10 Iron ore` for 10 up to `5 Steel` for 57, and validation refuses a
catalogue with none: insight buys research, and a game where nothing pays insight is a game where
nothing can be learned.

**Eligibility is the recipe tree, not an unlock column.** `Core::item_reachable` walks from the
requested item down through the recipes that make it, requiring a buildable machine for every
category and an unlocked source for every leaf — so water is gated on the pump, and a plate cannot
be asked for until somebody may build a smelter. A new item is gated correctly by existing.
`the_board_only_posts_what_the_player_could_make` passes every slot twelve times over and refuses
any posted row it could not produce.

**The draw order is a function of state, with no randomness in it.** `request_rounds` counts how
many times each row has left the board, filled or passed, and the least-used eligible row is posted
first. So fresh content leads in catalogue order, old standing orders come round again once nothing
new is left, and a save restores the board exactly rather than rerolling it. Every row carries a
**Pass**, because three materials the player has not found yet would otherwise hold the whole
economy; passing costs the row a place in the queue and forfeits whatever was already delivered
against it.

**The hub takes what it asked for and nothing else, by belt exactly as by hand.** `hub_demand` is
the board's outstanding units plus the founding contract's — every remaining stage, which is what
keeps the v0.18 surplus rule true — and it is the same predicate `can_accept` and `X` both use. A
line pointed at a satisfied hub backs up, which the belt shows; voiding the cargo for a coin was
not something the player could see.

**The balance tool priced the ladder, and the first run failed its own rule.** A new `requests`
section expands every row through the whole tree and reports insight per thousand gathers, fuel
included. Raw sits at 1000 by construction. The first authored pass put glass at exactly 1000 and
plate at 1100 — a tier that pays no better than the ground for a machine, its research, its power,
and its fuel — so eleven rewards moved before anything shipped. The curve is now raw 1000, crystal
1250, one machine step 1300–1333, assembly 1533–1625, and the deep chains 1778–1867, and
`every_processed_request_pays_better_per_gather_than_raw_material` is what holds it there.

**Guidance names a row, not an accounting entry.** The funding step was "Gather material for
insight", which is no longer true of gathering. It now names the posted request closest to done,
what is still wanted, and what it pays — `fill-request:ore-assay` or `deliver-request:ore-assay` —
and `tests/guidance.test.ts` walks the guide against a modelled board, refusing any row that is not
on the board the snapshot carried.

**Versions.** Save 10, definitions 10, wire 5 with a new requests group written between the contract
and the player. `npm run quality` is green: 98 Rust tests
(six new) and 85 TypeScript tests, lint, format, typecheck, and the production build. The pinned
capacity workload checksum moved to `780_276_626` because the board is in the checksum; the world
generator, item roster, and entity snapshot are unchanged, so no tier was re-measured.

**The browser no longer hides a save when a version moves.** Named slots live in
`hexfactory:saves:v1`. Each row remembers the seed, scenario, and world (preset or the landform
scale and sea level) plus the five numbers native refuses a load on. An incompatible run stays in
the list and says which number moved. Leftover single-slot keys `hexfactory:hxf1:…` are imported
and not deleted. The HXF1 envelope is unchanged; this is host catalog only.

**Also in this pass — the camera left the right mouse button.** Dragging with the right button
panned _and_ harvested, arbitrated by a five-pixel drift threshold that fired exactly when a player
was working one hex for several seconds. Panning is the middle button, or shift with the left; the
right button is only ever the harvest, and dragging now walks the hold to the hex under the cursor
instead of cancelling it.

## Shipped milestone — Power Grid v0.19

Shipped 2026-08-19. There was no brief for this one: it came from play, and the four things it was
asked for were a larger and upgradable pole reach, power through touching buildings, plants that
burn only what is used, and output proportional to consumption rather than to the clock. This
section records what those became, because one model turned out to answer all four.

**Power is energy now, not a rate.** `power_draw` kept its name and its number and changed what it
measures: it is spent per tick _of progress_, not per tick of the clock. A machine that is blocked,
starved, or unassigned spends nothing, and one craft costs `power_draw × duration` however long the
machine stood waiting first. Every number in `fixtures/balance.json` therefore holds for a saturated
factory — a component is still 64 units of electricity, a burner still turns one unit of fuel energy
into twenty — and the only thing that changed is that idle time stopped being billed.

**The buffer is the "one unit, three cycles" rule, and it is what makes a grid sized by average
load.** Each machine banks `POWER_BUFFER_CYCLES` whole cycles of work and asks for nothing more.
An extractor on a five-tick cadence, or a smelter waiting on ore, stops reserving capacity it is not
using, so one generator carries a much larger and lumpier factory than a per-tick tax ever could.
A brownout is no longer a slowdown factor applied to a machine: it is a machine that ran out of what
it was given, and it resumes at full speed the moment the grid hands it more.

**Demand-driven fuel needed no rule of its own.** A machine with nothing to do asks the grid for
nothing, so the grid draws nothing from its plants, so the plants burn nothing — there is no
"throttle the generator" step anywhere, because there is nothing to throttle. `advance_power_plants`
is gone; what replaced it is `burn_for_output`, billed against energy actually delivered, with
`burn_progress` holding the fraction of a fuel unit still owed so a plant at a fifth of load pays a
fifth as often rather than being either free or rounded up to a whole coal every tick. A turbine has
no firebox of its own, so its bill lands on the boiler beside it, where the coal and the water are.

**Energy is conserved, and that is the property the tests are built on.** What the machines banked
equals what the plants produced, to the unit — `apportion` splits by largest remainder so the two
sides balance exactly with no per-entity remainder to store and no drift to audit. Throughput comes
out proportional to generation with no scaling factor anywhere: an undersupplied factory is not
scaled down, it is handed less to spend.

**Coverage belongs to the pole, which is what makes it upgradable.** Before this, the distance a
machine could stand from a pole was read off _the machine_ (`power_reach`, default 2), so every pole
in the game reached exactly as far as every other one and no upgrade could move it — the test
`a_better_pole_lights_a_wider_disc_and_the_machine_does_not_change` could not have been written.
`supply_radius` is now the pole's, and the ladder is three data rows: pole 3, relay pole 4, trunk
pole 6, with pole-to-pole links at 6, 8, and 12 behind two new technologies. Validation refuses a
pole that supplies further than it links, and refuses one that names neither number, so the host can
read the ring straight off the file with no hidden default to miss.

**Touching machines conduct, which finally makes the balance tool honest.** `fixtures/balance.json`
has priced openings since v0.18 on the claim that "a generator's own reach covers what stands beside
it, so a pole is what distance costs, not what power costs." That claim was false against the
simulation: machines linked only to poles, so a burner built hard against a smelter powered nothing.
Now anything that draws or generates conducts to its neighbours. Belts and containers deliberately
do not — a line of the cheapest building in the game as free wire would mean nobody ever places the
second pole.

**The overlay is half the milestone.** A pole's reach was a number in a data file and nothing on
screen: the only way to find the edge of a network was to build a machine and watch it stay dark.
Selecting a pole now draws the disc it lights, rim brighter than field because where coverage
_stops_ is the reason to draw it; the pending-tool preview draws the same ring before the pole is
paid for. The inspector's power meter shows a machine's own bank against its buffer and a
generator's grid against its load, so "brownout" has a picture.

**What was verified, and what could not be.** 92 Rust tests and 85 TypeScript tests pass, with
`lint`, `typecheck`, and `format` clean. Five new native tests were each checked by breaking the
rule they name and watching them fail — disabling the adjacency relay, making plants burn flat out,
widening pole coverage by two, and letting idle machines hold the grid. Two of them were descriptive
on the first attempt and were rewritten: `power_of` puts an unconnected machine on a network of its
own, so `is_some()` is true of every machine ever built and proves nothing. In the real browser, the
factory demo runs, delivers, banks and drains, and a save round-trips to an identical checksum both
at load and after 25 further ticks. The coverage ring was confirmed by pixel diff rather than by
screenshot — **the browser pane still does not composite, so the frame loop never runs**, and the
timed keyboard-and-pointer playtest v0.18 asked for is still outstanding.

**Versions.** Save 9, definition 9, technology 5, wire 4. The wire's per-entity flag field became a
uvarint rather than the fixed byte it was: ten flags do not fit in eight bits, and widening to a
fixed `u16` would have charged every belt in the world a second byte to say nothing. The pinned
capacity workload checksum moved from `1679299541` to `914129621` because banked energy is in the
checksum; `docs/BENCHMARKS.md` says which of its numbers that invalidates and which it does not.

## Shipped milestone — Founding Contract v0.18

Shipped 2026-08-19. The brief below is kept as written; this section records what it became.

**The hub asks for a project, and the project is native truth.** A scenario now carries a
`contract`: an ordered list of stages, each a bill of materials with a name, a one-paragraph brief,
and a sentence saying what completing it does to the hub. `new-game` ships two. Stage one is the
founding `3 Component` delivery, kept deliberately — it is the whole loop in miniature and it is now
the _first beat_ rather than the whole game. Stage two is the foundry module: `16 Iron plate` and
`20 Brick`. Every hub delivery accumulates in `contract_contributed`; a stage closes when its whole
bill is met, consumes exactly that bill, and carries the surplus forward, so a player who automates
a line early is credited for it when the stage that wants it arrives. `advance_contract` loops
rather than closing one stage per delivery, because a stage a previous surplus already covers must
close in the same delivery instead of waiting for one more item to re-ask the question. Victory is
"every stage done", and it still opens free play rather than ending anything.

**The bill was chosen from `fixtures/balance.json`, not from the catalogue rows.** The balance
report gained a `contracts` section that expands every shipped stage through its whole recipe tree
and prices it exactly the way an opening is priced: research, the machines the tree needs, the raw
units, and the fuel. The foundry module reports `40 ore + 18 stone + 18 clay`, 97 gathers, 12 fuel
items, a 48.5-second hand floor and a 24.4-second machine floor, and — the number the milestone was
actually asking for — **2 raw materials**, which is the assertion that a founding project needs more
than one landscape. Iron is highland and clay is wet shore, so the bill is two chains in two
geographies, and both machines draw power. Stage one reports 32 gathers and 1 raw material, and a
Rust test pins that the last stage costs strictly more than the first.

**The balance tool found the defect the milestone was named after, in data.** An opening that needs
a machine drawing power now also prices the cheapest generator, because `power_progress` returns
zero off a network and a plan naming a smelter with no generator is a plan for a factory that stands
still. That rule moved three existing rows: `first smelter`, `first circuit`, and both contract
stages now all name `on-site-power` and a burner generator. **Even three components need power** —
a composer draws 8 — which is exactly what the old scripted guidance never said. One generator and
not a generator plus a pole: a generator's own `power_reach` already covers what stands beside it,
so a pole is what _distance_ costs rather than what power costs.

**The scripted next action is gone, and what replaced it is a dependency walk.** `src/core/guidance.ts`
reads the contract's outstanding bill, expands it through the shipped recipe tree, collects the
machines those recipes need and the technologies those machines sit behind — adding the power branch
explicitly, since it is nobody's recipe category — and reports the first prerequisite the player has
not met, in a fixed order: research, then power, then machines, then material, then the delivery. So
every answer it can give is achievable in the state that produced it: a research whose prerequisites
hold and whose cost is paid, or a building whose technology is already researched. It also names the
physical action rather than the accounting behind it — "Gather material for insight", not "Fund
Field Logistics" — because funding is not something a player can do.

`tests/guidance.test.ts` walks the guide the way a player would, doing exactly what it says one step
at a time for forty steps, and refuses any step whose prerequisites are not already met in the state
that produced it. It has to reach `complete`, and On-site Power has to appear before any machine
that draws it. **Deleting the one line that adds the power branch reproduces the v0.17 defect and
the test fails**, which is how the test is known to be load-bearing rather than descriptive.

**The next step is permanent chrome.** A `#next-step` card sits at the top-left of the world from
the first frame with the same title and sentence the panel carries, and pressing it opens the full
brief behind the same `P` toggle. The mission header names the thing behind the number: `0 / 3
Component`, `0 / 16 Iron plate · 0 / 20 Brick`, with the contract name and the stage index in the
kicker and the stage's own name as the title. The panel carries the bill as lines — one row per
item, each with both its published numbers and its own bar. Below 720px the card is hidden, because
there it would cover the world it is describing, and the header still names the stage.

**Progressive disclosure is a distance over the technology graph, not a curated list.**
`technologyReach()` measures how many unresearched technologies stand between the player and each
one; both catalogues lead with everything within two. At minute zero that is five technologies of
twelve and six buildings of eighteen — the belt, riser, extractor, container, pole, and burner
generator, which is the whole early game — and each panel carries a control that says exactly how
many it is holding back and hands them over. A new technology needs no thought here at all.

**The hub grows through the same vocabulary a tier does.** `HUB_LADDER` is a second `TierStep` list
applied by completed stage count, and `applyTier` is now one call into a general `applyLadder`. A
finished stage makes the hub's outline about a quarter taller and adds seams, plating, and a second
mast — the same order of change a tier step makes, which is the legibility standard v0.15 already
shipped and documented. Growth reads the published `contract.stage`, so the drawing, the sentence,
and the saved state are one number.

**The world says why a machine is doing nothing.** A working machine and one starved for ten minutes
drew identically, and the only way to tell them apart was to click one. `STALL_MARKS` is a table
over the status string the wire already carries, split by cause — waiting for inputs, out of fuel,
output blocked, nothing left to take — and drawn as one dot on the hex. `no power` and `brownout`
are deliberately absent, because a dimmed machine already says that. And the three-letter stamp
moved: it was drawn across the middle in bright white, covering the anatomy it was labelling and
doing all the identifying work at ordinary zoom. It is smaller, quieter, and under the body now, so
the shape is what the eye reaches first.

**Feedback has a voice, and it is synthesised rather than shipped.** `src/audio/feedback.ts` is six
cues — gather, place, reject, deliver, unlock, project — as a table of specs over one envelope, the
same shape as the shape grammar: a new cue is a data row, not an asset. Pitch direction carries the
meaning, up for progress and down for refusal. The cue is chosen from the native event, so a
delivery made by a belt and one made by hand sound the same, and refusals are picked out by the
convention the core already follows: native writes what happened in sentence case and what was
refused in lower case. Mute is on the command bar, on `M`, and in the menu; reduced motion joins it
there and can only ever quieten further than the system preference, never argue with it.

**The single-hex overshoot is a walk speed, not a native change.** At `PLAYER_SPEED` 242 over 30
steps a second the player crosses a hex column in about a quarter of a second, which is a human
reaction time — so holding a direction overshoots one hex, every time. Holding `Shift` sends an
intent at 0.4 magnitude, which native's `move_intent` has always accepted, so the host sends a
smaller intent and never a smaller step. No rule about the player's clock moved.

**Versions, and the save key that stopped being copied by hand.** Save version 8 and scenario
version 5: contract stage and contributions are saved and checksummed state, and a version-7
envelope carries neither. The browser key now _reads_ the definition, technology, and scenario
catalogue versions instead of restating them, because a number a person has to remember to copy is a
number that will eventually not be copied, and twice now it has not been. Only `SAVE_VERSION` stays
a literal, since native does not publish it. Verified in the browser: a v0.17 save sits under the
old key with Continue disabled and "No local save yet", and a new save round-trips at an identical
checksum. The wire format is version 3 — the objective group became the contract group and carries
names and a bill — regenerated in `fixtures/snapshot-delta-wire.json` and read back by TypeScript.

**What was verified, and what could not be.** A stage-one save generated natively and loaded in the
real browser advanced the header to "Raise the foundry module", rendered both bill lines, and
produced correct guidance; two saves differing only in contract stage produced a measurably different
hub on the canvas at the same player position. `npm run quality` is green — 85 TypeScript tests, 86
Rust tests, the production build, and no audit findings. **The timed keyboard-and-pointer playtest
the brief asks for was not done**, and the reason is the same one v0.17 recorded: the browser pane
does not composite, so the frame loop never runs and nothing on the player's own clock — walking,
gathering, the cooldown, the new precision walk — can be exercised here. The material floor is
measured and written down above; the interaction cost is not, and no claim is made about it. That
playtest is the first thing the next milestone should do, and it should be done by a person.

**The capacity ladder was not re-run.** The world generator, the item roster, and the entity snapshot
are all exactly what v0.17 measured, so no trigger fired. The pinned workload checksum moved from
`2402899979` to `1679299541` because the contract is in the checksum; `docs/BENCHMARKS.md` says which
of its numbers that invalidates and which it does not.

## Historical brief — Founding Contract v0.18

The landing hub stops being a gold delivery counter and becomes the first visible project in the
world. The three-Component delivery may survive as its first beat, but not as the whole arc. The
player establishes a powered, automated material base and completes one **founding module** whose
construction visibly changes the hub. The exact bill is chosen from `fixtures/balance.json` and a
timed first-run playtest, not guessed from catalogue rows; it must require automation, power, and at
least two geographically distinct material chains without demanding the whole roster.

This is one bounded contract, not an infinite quest generator. Later contracts may branch and large
projects may keep a mature world purposeful, but v0.18 proves that a demand can be native truth,
saved, checksummed, visible on the hub, and completed without closing free play. The hub's growth
uses the existing generated-shape vocabulary or another systematic modifier; it is not a one-off
sprite.

### The first twenty minutes are part of the milestone

- **Repair the dependency path.** Guidance must lead through every prerequisite a running first
  factory actually needs, including power. The default answer is an explicit On-site Power step.
  If a timed playtest proves that the resulting hand-gather phase is too long, a deliberately
  limited landing-hub bootstrap supply may replace it — never silent free power, and never a host
  exception to `power_progress`.
- **Put the next action in permanent chrome.** A new player should not have to know that `P` hides
  the useful sentence. The mission header names the item or project behind `0 / 3`, and the one next
  actionable dependency is visible without opening the full controls reference.
- **Progressively disclose complexity.** Research defaults to completed, available, and immediate
  dependants; the full tree remains one explicit view for planning. Construction leads with
  unlocked buildings and the next relevant locks rather than every late machine at minute zero.
  Empty hotbar slots remain configurable but must not make the opening read as nine disabled tools.
- **Make manual work short and physical.** Gathering remains the bootstrap, not the primary source
  of insight after automation. Fix the recorded single-hex movement overshoot, and give gather,
  delivery, placement, blockage, completion, and a running machine distinct audio/visual responses.
  Prefer a small generated or procedural sound vocabulary over an unmaintainable pile of one-off
  assets. Reduced-motion and mute controls are part of the same pass.
- **Let the scene carry identity.** At ordinary play zoom, motion and anatomy must compete with the
  three-letter stamp rather than disappear under it. The stamp remains a useful label; it is not
  allowed to be the only reliable difference between machines. The Factory demo starts framed on
  the production line and makes flow or throughput legible without selecting every entity.

### Acceptance

- A fresh player can reach a powered automated line by following only the permanent next action;
  every instruction is achievable in the state that displays it.
- The first completed project uses more than the founding `2 Iron ore -> 1 Component` chain and
  visibly changes the landing hub. Completion still leaves the world playable.
- A timed keyboard-and-pointer playtest records time spent gathering, walking, choosing research,
  placing the first powered machine, and completing the contract. `fixtures/balance.json` predicts
  the material work; the playtest states the interaction cost it cannot measure.
- Default research and construction views do not present the whole locked roster at minute zero,
  while an explicit full view still supports long-term planning.
- Gather, place, reject, deliver, project-complete, machine-working, and machine-starved states are
  distinguishable with the world visible. Audio is optional to the player, not absent from the
  product.
- The old three-Component save/objective contract moves only with an explicit save/scenario version
  decision. The browser save key still names every native version the envelope refuses.

## Where to start

Two independent fronts, and picking either is correct:

1. **Panels and Item Language v0.20.1**, immediately below. Host-only, no native code, no version
   movement, independent of everything else in this document. Shortest path to a visible improvement,
   and it is the one a session with limited room should take.
2. **Landforms and Fields v0.21**, after the roadmap decision below. The milestone arc, and the
   order v0.21 → v0.22 → v0.23 is load-bearing rather than a preference — read the roadmap decision
   for why the world work has to precede the economy work.

Do not start v0.23 first. It is written to be tuned against a world that v0.21 builds, and against
transfer rows that v0.20.1 collapses into one function.

**v0.21 and v0.22 are one version train.** v0.21 moves `WORLD_GENERATOR_VERSION` and rejects every
existing save; v0.22's twelve-heading routing table wants a save break of its own and rides that one
instead of spending a second. If they are ever split, v0.22 has to pay for its own break and the
orientation-index decision in that brief reopens.

Four decisions are deliberately left open for whoever implements, and each is marked where it sits:
whether panels dock in rails or float freely (v0.20.1); whether `regrowth_ticks` moves when a forest
cell drops to three wood (v0.21); whether the single-cell footprint restriction is lifted now that
its stated reason is gone (v0.22 — the brief recommends **not** now, and says why); and whether one
board slot is reserved for the deepest eligible request (v0.23). Each says what would settle it.

One decision that is **not** open, and must not be reopened casually: `DIRECTIONS` stays six. Twelve
headings are transport only. Widening adjacency would let a boiler reach a turbine two rows away and
a pole span a distance no player can see. Decide them from measurement and write the answer
down; do not leave them open a second time.

## Presentation pass — Panels and Item Language v0.20.1

A point release rather than a milestone, on the precedent Inspector Readability v0.13.2 and
Construction Catalogue v0.14.1 already set: host-only work, no native change, no save, definition,
generator, wire, or checksum movement. It is independent of the whole v0.21–v0.23 arc and can ship
before, between, or after any of it.

Asked for from play: _"the user has to tediously open the development and cargo panels separately to
check if they have the right resource in the right amounts"_, and _"items in inventory should look
the same everywhere"_. Both are true, and both have a cause more specific than the complaint.

### Defect one — affordability is a boolean, and the reason is in another panel

`buildingAvailability` computes `affordable` as **all or nothing**: every cost line is checked, and
one `false` comes back. `fillBuildCard` renders it as a CSS class, `unaffordable`. So the card knows
the answer and throws away the reason. A player told "no" has to open the cargo pack to find out
which of the three lines they are short on, and by how much.

**That is the whole of the tedium, and it does not need a panel to fix.** The build card already
draws the cost through `fillIngredients`; it simply never says what is held against it. Give every
ingredient a held count beside its required one, and the card answers its own question.

- `BuildAvailability.affordable: boolean` becomes a per-ingredient shortfall. Keep the boolean as a
  derived convenience if the call sites want it, but the shortfall is the value, because "you need 2
  more Iron plate" is a different fact from "no".
- `src/core/availability.ts` is plain TypeScript over a snapshot, so this is testable without a DOM,
  and it should get a test. It is the only piece of this pass with any logic in it at all.
- The same treatment goes everywhere the game names a quantity the player might be expected to
  supply: recipe inputs, the contract bill, and the request board. One component, four call sites —
  see below — not four features.

### Defect two — four panels share one rectangle

`togglePanel` calls `closePanels(target)`, so opening any panel closes every other. That is not a
policy choice that can simply be deleted: in `src/styles.css`, the cargo, research, quest, **and**
build panels are all `position: absolute; top: 108px; left: 14px`. Four panels at one origin. Remove
the exclusivity and they stack on top of each other.

So the fix is a layout, not a flag. **Two rails, each holding independently collapsible panels:**

- **Left rail** — cargo, research, build, quest. Each opens and closes on its own; several open at
  once flow down the rail, each scrolling inside its own box.
- **Right rail** — inspector, session. The inspector keeps its standing exception: it is the one
  panel that lives on the world rather than behind a key, and it stays open by default.
- Open state is **presentation state in `localStorage`**, on exactly the terms the hotbar
  arrangement already establishes: never saved with the game, never hashed, never sent. It is a
  preference about a screen, not a fact about a factory.
- `closePanels()` survives untouched as the **reset** it already is — `Escape`, a new game, and a
  load all call it, and all three should still clear the screen. What changes is that opening a panel
  stops calling it.
- Below a viewport threshold the rails collapse back to one-at-a-time, so the mobile layout is
  unregressed rather than newly cramped.

**On draggable windows.** They were raised as a "maybe", and the recommendation is to hold them one
step back. Free positioning brings saved coordinates, overlap policy, off-screen recovery, and a
touch story where dragging fights the world pan that already owns the same gestures — a lot of
machinery for a need whose actual shape is "let me see two of these at once", which the rail answers
outright. The rail is also the honest prerequisite: once panels open independently and remember it,
dragging is an increment on top rather than a rewrite. **This is a live decision point, not a settled
one** — if free-floating panels are wanted as the goal, say so and the rail becomes their docked
default position rather than the destination.

### Defect three — an item is drawn eight different ways

Every one of these renders the same conceptual object, "an item and a quantity", and no two agree:

| Where                    | Markup                           | Icon                        | Quantity             |
| ------------------------ | -------------------------------- | --------------------------- | -------------------- |
| Cargo grid               | `.inventory-slot`                | SVG glyph                   | `3`                  |
| Build cost, recipe sides | `.ingredient`                    | SVG glyph                   | `×3`                 |
| Container Take rows      | `.inspect-stock-row`             | SVG glyph                   | `3`                  |
| Container Put rows       | `.inspect-stock-row`, duplicated | SVG glyph                   | `3`                  |
| A machine's cargo        | `.inspect-cargo`                 | SVG glyph                   | bare                 |
| A field cell             | `.inspect-field`                 | SVG glyph                   | `3 / 20` and a meter |
| Contract bill            | `.contract-line`                 | **colour swatch, no glyph** | `3 / 10` and a bar   |
| Request board            | `.request-line`                  | **colour swatch, no glyph** | `3 / 10` and a bar   |

Eight shapes, three orderings of glyph, name and count, three spellings of the same number, and two
places where an item has no icon at all.

**One component, `itemChip`, becomes the only way an item is ever drawn:**

```html
<span class="item-chip" data-item-id="11">
  <span class="item-chip-glyph"></span>
  <strong class="item-chip-name"></strong>
  <span class="item-chip-count"></span>
  <i class="item-chip-meter"><b></b></i>
</span>
```

Variants are modifier classes on one markup — `.named`, `.metered`, `.short` — never a second shape.
Three rules it exists to enforce:

- **An item always shows its glyph.** The bare colour swatch goes. Colour alone is not an identity in
  this catalogue: Iron plate `#c3ced6`, Gravel `#9a9188`, and Concrete `#9aa0a4` are three greys, and
  the contract bill and request board are precisely the two places a player is being asked to fetch
  something specific.
- **One spelling per meaning.** `3` is an amount; `3 / 10` is progress toward a known target. `×3` is
  a third spelling of the first and goes.
- **A shortfall is a state of the chip**, not a class on a card. That is what makes defect one a
  two-line change at four call sites instead of four bespoke treatments.

Every chip is patched in place through `syncChildren`, so the "any host list carrying a control is
patched in place, never rebuilt" invariant is satisfied by the component rather than remembered at
each call site.

### Defect four — Take and Put are the same function twice

`renderInspectorActions` and `renderInspectorLoad` are near-identical: same row markup, same glyph
call, same button class, differing only in the data source, the button label, and the command. They
collapse into one `renderTransferRows(list, entries, direction)` where the direction supplies all
three.

**The fractional deposit moves here from v0.22**, because it is the same code and doing it twice is
how the two halves drift. Native already carries a quantity on `store` and `withdraw`, already clamps
it, and already reports how much moved; the host has only ever sent the maximum. Add a quantity
control defaulting to the full amount, plus a half — once, in the one function.

### Acceptance

- A build card states what is held against every line of its cost, and names the shortfall. Opening
  the cargo pack is never required to learn whether something can be built.
- `src/core/availability.ts` returns per-ingredient shortfalls and has a test that does not touch the
  DOM.
- Cargo, research, build, and quest open and close independently, several at once, without
  overlapping. `Escape`, a new game, and a load still clear the screen.
- Panel open state survives a reload and never enters a save, a checksum, or a snapshot.
- One `itemChip` component is the only place an item glyph, name, or count is written, and the
  contract bill and request board show glyphs.
- Take and Put are one function, and it moves a chosen quantity.
- No native, save, definition, generator, wire, or checksum change. `npm run quality` green, and the
  Rust suite untouched.

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

Numbering, since v0.20 was taken by Standing Requests while two later briefs still claimed it:

| Milestone            | Number | Was                                        |
| -------------------- | ------ | ------------------------------------------ |
| Landforms and Fields | v0.21  | new                                        |
| Crossings and Canopy | v0.22  | new                                        |
| Earned Insight       | v0.23  | new                                        |
| Living Lattice       | v0.24  | v0.20                                      |
| Regional Discovery   | v0.25  | v0.21, generation half absorbed into v0.21 |

Regional Discovery is not deleted. Its **generation** half — a landing clearing that guarantees a
bootstrap path rather than a sample platter, and a survey that measures whether every preset still
works — is exactly what v0.21 has to do anyway to make fields mean anything, so it moves forward.
What stays at v0.25 is the half that is a play system rather than a generator: a region announcing
itself through shape, colour, life and sound; an in-game survey that hints; and a distant discovery
that creates a reason to establish a specialized site rather than carry one stack home.

## Next session — Landforms and Fields v0.21

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

- Shallow water stops being an accident of sea level and becomes **common and linear**, which is what
  makes a bridge a necessity rather than an ornament.
- `PlacementRule::Water` — buildable ground with open water inside `PUMP_RADIUS` — starts matching
  inland. Pumps, hydro, and boilers gain sites everywhere, which is a real balance change and belongs
  in `fixtures/balance.json`'s access section, not in a footnote.
- The survey's water figures start mixing bodies and rivers. Report river hexes, river runs, and mean
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
- If a window finds nothing, widen it in fixed steps to a hard cap and then **fail loudly**. A preset
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
- Rust tests that must be rewritten rather than nudged: `every_material_is_generated_where_its_geography_says_it_should_be`,
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
- Every preset produces iron, coal, copper, and stone patches of at least 19 hexes, and forests of at
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

## Following milestone — Crossings and Canopy v0.22

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

**Fractional deposits into containers** are in Panels and Item Language v0.20.1, not here. They are
the Take and Put rows, those two renderers collapse into one function in that pass, and adding a
quantity control to two near-identical copies is how the two halves drift apart. If v0.20.1 has not
shipped when this milestone starts, do that pass first rather than duplicating it.

The radius **chips** in step 4 below share v0.20.1's `itemChip`-era markup conventions but are a
building's stat line rather than an item, so they stay here with the rings they belong to.

### Acceptance

- A bridge crosses shallow water, carries a belt, refuses deep water, and `fixtures/terrain-passability.json`
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

## Following milestone — Earned Insight v0.23

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

- **Eligibility is the recipe tree.** `Core::item_reachable` walks from a requested item down through
  the recipes that make it, requiring a buildable machine for every category and an unlocked source
  for every leaf. A row cannot be posted until the player could actually produce it, so the board
  opens up as research does, without an unlock column anybody has to maintain.
- **The reward curve is already depth-scaled**, in `insight_per_gather_milli`: raw 1000, crystal 1250,
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

## Deferred milestone — Living Lattice v0.24

Four things it should know before it starts, all of them handed over by v0.18 rather than invented
here:

- **Play the opening first, with hands, and time it.** v0.18 repaired the first twenty minutes
  against the rules and could not measure them against a person: the frame loop does not run in the
  agent's browser, so nothing on the player's clock was exercised. `fixtures/balance.json` predicts
  the material work — 32 gathers to stage one, 97 to stage two, a 65-second combined hand floor —
  and says nothing about walking, choosing, or placing. Before adding a system, find out whether the
  contract's second stage takes five minutes or forty. A number from a person outranks every number
  in that file.
- **`Economy::recipe_for` still asserts one recipe per item, and ecology is what breaks it.** That
  assert is the handoff v0.17 named and v0.18 did not touch: a byproduct is a second producer, and
  "what does a plate cost" has no answer without a stated rule for dividing a craft's cost between
  its outputs. Pick the rule deliberately and write it down beside the fixture. The new `contracts`
  section expands bills through the same tree, so it breaks in the same place and for the same
  reason.
- **A new contract stage is a data row, and the hub already knows how to grow into it.** Stages
  live in `scenarios.json`; `HUB_LADDER` has one entry per stage the hub can finish, and
  `tests/look.test.ts` fails if a shipped contract can complete a stage the ladder cannot draw. If
  Living Lattice wants the hub to ask for biomatter, that is a stage and a ladder row, not a system.
- **Guidance follows the contract for free, but only through recipes.** `nextAction` walks recipe
  inputs and recipe categories. An ecological input that is _harvested from a population_ rather
  than crafted will fall out of the walk as a raw material, which is right; an ecological _process_
  with no recipe row will not appear at all. Whatever v0.20 adds, give it a recipe row or teach the
  walk about it deliberately — the one thing that must not happen is a hub asking for something the
  next step cannot explain.

### Living Lattice v0.24 — the brief

Animals, biomatter, and waste remain one milestone, but their purpose is now sharper: this is the
first system that makes HexFactory something other than a factory game drawn on hexes. A living
population moves, feeds, breeds, recovers, and can be depleted past recovery across hex
neighbourhoods. Biomatter comes from that population rather than from a renamed static field.
Waste is a byproduct with a visible destination: it can feed a recovery loop, damage a habitat, or
be refined. Producer, byproduct, and consumer are designed together so none is a decorative item.

This is **not** a Factorio pollution/enemy-wave substitute. The pressure is ecological consequence
and opportunity, not a timer that periodically sends attackers. A player should be able to preserve
a productive migration, intensify it carefully, exhaust it for an urgent contract, or repair a
region they damaged. The landscape answers the factory, and the answer is visible where it happens.

Hex topology earns itself here. Movement and propagation use six neighbours; a herd or recovery
front has a perimeter; extraction reach is a ring; machines expose meaningful faces when a process
has directional intake, output, heat, or waste. Do not add generic adjacency percentages that
collapse into one solved blueprint. A hex mechanic belongs only when rings, faces, fronts, or
multiple approach directions change a decision the player can see.

Rust/Wasm still owns every ecological tick. Use sparse scheduled populations or active fronts, not
a JavaScript cell loop and not HexLife source imports. `@hexlife/embed/hex` remains the host geometry
contract; it does not become the simulation kernel merely to justify the name.

### Balance and data handoff

`outputs: Vec<Ingredient>` arrives here because the economy now needs a real byproduct. That breaks
`Economy::recipe_for` deliberately: a tree-expanded cost through a multi-output craft needs a stated
allocation rule. Choose and document that rule beside the fixture; do not make the secondary output
free, charge every output the full craft, or silently select one producer. Every new definition must
reach `fixtures/balance.json`, and any ecological yield claim needs a measured fixture analogous to
the world survey.

### Acceptance

- One complete loop produces useful biomatter and a waste stream, and the player has at least two
  legible responses to that waste with different ecological outcomes.
- The same installation in two habitat states does not have the same answer, and the reason is
  visible in the world rather than hidden in a modifier panel.
- A population can recover, migrate, and collapse deterministically; saves and checksums reproduce
  each outcome exactly.
- The founding hub asks for something from the loop, so the new system has a motive on arrival.
- The native capacity ladder and complete browser frame are re-measured if the entity or world
  snapshot changes. No claim is made beyond the measured tier.

## Later milestone — Regional Discovery v0.25

**Its generation half moved forward into Landforms and Fields v0.21.** The bootstrap guarantee that
replaces the sample platter, and the survey that proves every preset still works, are what v0.21 has
to build anyway for fields to mean anything, so they are specified there and are not repeated here.
What remains at v0.25 is the half that is a play system rather than a generator, and it should be
read as beginning from a world that already has readable landforms, rivers, and real deposits.

v0.16 made world shape parameterized; this makes that variation a play system. Advanced materials and
ecological opportunities belong to readable regions that require travel, surveying, and eventually
outposts. Every preset remains completable, but "completable" no longer means "sample platter at
spawn."

A third low-frequency generation channel may create dry and wet variants of the same elevation
band, but generation is not the milestone by itself. A region has to announce itself through shape,
colour, life, sound, and material behaviour; the player needs a survey tool that hints rather than
reveals the entire answer; and a distant discovery must create a reason to establish a specialized
site rather than carry one stack home and forget the place. Landing contracts and later hub modules
provide that reason.

Signal crystal is the strongest candidate for a later hex-native automation language: relays along
faces, triangular links, or closed rings can make spatial control distinct from conventional circuit
combinators. It stays a candidate until Living Lattice proves which signals the player actually
needs; do not build a programmable system in search of a problem.

### Acceptance

- Every preset remains completable, measured by an updated survey that reports first advanced-region
  distance, regional extent, and access from buildable ground. The bootstrap half of this is v0.21's
  and is asserted there.
- Crossing into a region is recognisable without opening the game menu or reading coordinates.
- At least one founding-hub project requires a sustained distant site, not a one-time hand trip.
- The minimap and home bearing support the expedition without revealing unsurveyed world or
  re-deriving native generation truth.

## Longer play horizon after v0.25

- **Hub programmes.** Player-chosen modules grow around the landing hub's rings and create different
  material demands. They are finite authored systems and visible construction, not endless random
  chores. Large projects give an established factory a reason to expand without turning one victory
  into a wall.
- **Six-face machines.** Ports, heat, exhaust, or control may attach to named faces where direction
  creates a readable routing choice. Closed loops and triads are available shapes, not mandatory
  bonuses on every machine.
- **Fluid networks, intermittency, accumulators, and the day cycle** remain improvements on systems
  that already work. They follow the motive, ecology, and regional spine rather than preceding it.
- **Tunnels** still cost one trace-loop match arm and may ride any compatible version bump; they do
  not become a milestone by themselves.

Whatever comes next, `fixtures/balance.json` remains the thing every new building or recipe has to
face: a definition that never reaches it is a definition nothing has compared against the curve,
and both test suites say so.

## Historical brief — Balance v0.17

The first deliberate pass at the numbers, and the reason it is third: balance is tuned against
resource density, and v0.16 is what turns density into a parameter. Tuning before that would be
tuning against a constant that is about to move.

### Balance is the one system with no representation

Everything else in this project is pinned in two languages — hex directions, terrain passability,
the wire format, the upgrade ladders. The economy is twenty buildings, fourteen recipes, and
twenty-three items in `src/data/definitions.json`, correctly data-driven, with **nothing anywhere
that states or tests what the curve is meant to be**. A steam turbine outputs 48 and a smelter draws
10, so one turbine runs nearly five smelters. That may be exactly right. Nothing says.

### Compute the deciding numbers; do not tune by feel

The fixture is not a table of costs — those already exist. It is the **derived** figures that
actually decide whether an economy works, none of which anything currently computes:

- Items per minute per machine at its cadence and recipe duration.
- Power output per generator against draw per consumer, and the machine count each generator carries.
- **The full raw-material cost of every building expanded through its entire recipe tree.** This is
  the number that exposes a broken curve, because a building's own cost row hides everything its
  inputs cost to make.
- Extraction yield per site — cells in reach × quantity per cell — under each v0.16 preset.
- Time to first smelter, first power, first circuit, from a standing start.

Pin them in `fixtures/balance.json`. Tuning then becomes editing a data row and reading the diff of
what it did to the whole tree, which is the same systematic maintenance v0.15 gives shapes and v0.16
gives worlds, applied to numbers.

### One thing the fixture already surfaces

The deep extractor goes reach 1 → 2, which is **7 cells → 19 cells**, and cadence 5 → 4 at the same
time. That is roughly **3.4× the throughput** for a cost that merely contains the base cost. It may
be the intended flagship feel. It is currently invisible, and after this milestone it is a number in
a file that moves when someone changes it.

### Acceptance

- `fixtures/balance.json` exists, is generated from the shipped definitions, and is asserted in test.
- A deliberate tuning pass has been made **with reasons recorded** — this milestone is not the
  fixture alone.
- Every recipe's inputs are reachable from the landing site under the default preset.
- No building's tree-expanded cost is cheaper than a building it is meant to follow.

## Presentation pass — Construction Catalogue v0.14.1

Shipped 2026-08-18. Presentation only: no command, no save, definition, wire, or checksum movement.

**The defect, named.** The dock was every buildable definition in id order. That was fine at six
and unreadable at eighteen: a row of three-letter stamps that grows every milestone, mixing a belt
with a steam turbine, showing cost as a truncated caption and a machine's recipes not at all. The
player's own words were "overwhelming and hard to understand."

**Buildings moved into a catalogue behind `B`.** Grouped by what a thing is _for_ — Extraction,
Transport, Processing, Storage, Power — and the grouping is derived from `kind`, so a new
definition lands in the right section by being what it is rather than by a per-building case. Each
card carries the stamp in its own `BUILDING_COLORS` hue, the description, and chips for the facts
that decide a choice: what research it needs, its tier, its reach, what it holds, what it draws or
makes, and whether it runs north–south.

**A recipe is materials now, not a name in a dropdown.** `Steel` said nothing about what steel
takes. Every recipe on a machine's card is written as glyphs with counts, an arrow, and the output
— the same twelve-glyph set the pack and the fields already use — with ticks and fuel beside it.
Clicking one holds that machine set to it. The `<select>` remains for the pending build, but the
choice can now be made where its reason is visible.

**The bar became a hotbar the player owns.** Four fixed tools, then nine slots bound to `1`–`9`,
with the digit drawn on the slot so the binding is visible rather than memorized. `Pin` drops a
building into the first free slot; dragging a card onto a slot chooses which; dragging a slot onto
another swaps them; `×` clears one. The arrangement persists in `localStorage` and is
**presentation state under the usual rule** — never saved with the game, never hashed, never sent:
it is a preference about a keyboard, not a fact about a factory. A stored slot naming a definition
this build retired is dropped rather than rendered as a button that selects nothing, and a slot the
player deliberately cleared does not refill itself with a default.

Both new lists carry controls, so both are patched in place rather than rebuilt — the rule that
already exists because a `replaceChildren` between pointerdown and pointerup loses the click.

Layout was measured, not assumed: nine slots beside four tools and the catalogue opener overflowed
a 1440-wide window at the dock's original button size. Slots are compact and caption-less (stamp,
digit, and the full name in the tooltip) and the fixed tools are narrower, which brings the bar to
747 px inside 747 px with nothing clipped.

## Shipped milestone — Upgrades and Tiers v0.14

Shipped 2026-08-18. The brief below is kept as written; this section records what it became.

**A tier is a data row, and the ladder is validated once at load.** `BuildingDefinition` gained
`tier`, `upgrades_to`, `extract_radius`, and `orientation_axis`. `validate_upgrade_ladders` pins
kind, recipe category, footprint, and axis across every step of a ladder and requires a strictly
increasing tier — which is what lets `upgrade` stay a short command instead of a second copy of the
placement rules, and what makes a ladder finite. Two ladders ship: the extractor grows into the
**deep extractor** (reach 2, cadence 4 — the flagship, visible on the map), and the container into
the **deep container** (capacity 24).

**`upgrade` edits the entity in place and never replaces it.** That is what preserves contents,
orientation, and connections with no special handling: only `definition_id` moves. Progress is
clamped rather than reset, because a tier may change the cadence under a part-finished craft.
The price is netted per item against the old construction cost, both halves are checked before
either is applied, and a refund that will not fit is refused rather than partially paid — the same
all-or-nothing rule `erase` uses. Both shipped tier-1 costs _contain_ their tier-0 cost, so an
upgrade is a pure surcharge and nothing round-trips through the pack. `place → upgrade → erase` is
asserted item-neutral. One correctness note found while testing: an upgrade that returns nothing
must not consult carrying capacity at all, or a full pack refuses an edit that does not touch it.

**North and south are a direction-table row, exactly as the write-up predicted.** `DIRECTIONS`
stays six for adjacency and power; `TRANSPORT_DIRECTIONS` is eight for routing, with the six at
their original indices. `compile_graph_target` needed one line — the table it indexes — and nothing
else, because it always was a ray-cast that never assumed a unit step. A test pins the claim the
whole design rests on: `(q + 1, r - 2)` lands on _exactly_ the same world-x, two rows up.

**The vertical drag rule needs no tuned angle constant.** `hex_line` is untouched, so every drag
that resolved before v0.14 resolves identically; `hex_line_vertical` is a separate rule selected by
the dragged definition's axis. It takes a two-row step only when that step closes the full two rows
it spans — and in the hex norm that is true exactly when the target lies in the closed cone between
`NE` and `NW`, which is 60° wide and centred on due north. So the rule reads, precisely, _within
30° of vertical_. An erase drag has no definition to ask, so it takes its axis from the hex it
started on.

**The riser is priced as a data row.** `OrientationAxis::Vertical` requires a single-cell footprint,
because `@hexlife/embed` rotates by 60° and the vertical headings have no 60° equivalent. A belt
cannot take a vertical heading and a riser cannot take an edge one, which is what stops a riser
being strictly dominant: it is a separate definition, so it has a separate cost row — 2× a belt,
asserted in both languages. The two straddled hexes are never occupied and need no code to stay
free: the riser is one cell, and its belt spans the seam.

**Two player-facing additions arrived mid-milestone, both asked for directly.** A right-click that
does not drag harvests one _named_ hex — which is the argument the facing invariant demanded, and a
different argument from the one it refused: the player named the hex on screen, so the cause of the
number moving is visible. Reach is unchanged and still the shared `field_covered_at` predicate, and
both gathers land in one `gather_from`. And `store` is the exact mirror of `withdraw`, so the
inspector now carries a **Put** row per carried stack beneath its **Take** rows and stock moves both
ways by hand without a belt.

`HXF1` save version is 7, definition version 7, technology version 4. v0.13 saves are rejected —
the envelope covers both the tier definitions and `orientation` becoming an index into eight.
No wire change: `orientation` was already a `u8`, and tier is read from the definition table the
host already holds, so the entity snapshot is untouched and the v0.13.1 render baseline stands.

Gate: `npm run quality` green — 71 Rust tests, 62 TypeScript tests, lint, format, build.
Interactive browser verification was **not** available this session: the Browser pane was not
compositing, so `requestAnimationFrame` never fired and the frame loop could not run. Boot was
verified instead (no console errors, both catalogs validated at load, the new dock entries,
compass spokes, and panels all present in the DOM).

## Historical next-session brief — Upgrades and Tiers v0.14

The originally-deferred play milestone, now with materials to spend, a power budget to improve,
and a generator that can paint a new tier without a new drawing. Begin in
`X:\Programming\Projects\HexFactory`. Read this section and the v0.14 write-up below before
editing definitions or commands.

- **Tiered building definitions.** A later tier is a data row: same `recipe_category`, trim from
  `tier`. Do not add a drawing, a `BuildingKind`, or a tick path.
- **An upgrade command** that preserves contents, orientation, and connections. Bounded and
  range-checked beside `place`, `erase`, `withdraw`, and `set_recipe`.
- **Extraction radius is the flagship upgrade.** It is visible on the map, it changes a
  decision the player already made, and it demonstrates what tiers are for better than a bigger
  box does. Larger containers, faster smelters, and more efficient generators follow the same
  pattern.
- **North and south enter the transport direction table**, riding the version bump this milestone
  already pays for. `TRANSPORT_DIRECTIONS` becomes eight — the six unit steps plus `(1, -2)` north
  and `(-1, 2)` south — while `DIRECTIONS` stays six for adjacency and power. `compile_graph_target`
  is unchanged; `hex_line` gains an explicit vertical rule; risers are single-cell and cost 2× a
  belt. See the resolved write-up below before touching either table. Tunnels are a later pass and
  are not in this milestone.
- A save / definition version bump is expected; say which, and reject the previous envelope. It
  covers both the tier definitions and `orientation % 6` becoming `% 8`.
- **Upgrade must conserve items.** The erase path already refuses a refund that will not fit,
  because that is the only rule that keeps conservation exact. An upgrade in place needs the same
  care, or an upgrade / downgrade round trip becomes a duplication exploit. Test it explicitly.
- Re-measure the native ladder if the entity snapshot changes. The Look Systems browser record
  (`docs/BENCHMARKS.md`, v0.13.1) stays the render baseline unless the draw itself moves.

Out of scope: 3D, tunnels, a hand-drawn atlas, fluid networks.

## Remaining playtest diagnoses (after v0.12.1)

v0.12.1 took the three player directions from the 2026-08-17 first-minutes playtest. These are
what that session should not lose, still open:

- ~~Dual glass panels (mission + research) leave a corridor of world at 1440×900.~~ Closed by
  v0.12.3: the inspector is the only panel left over the world. The dock still shows every locked
  building from minute zero. Cargo slots are icon + count with no name.
- Gather and deliver copy is honest (`stand on or beside a field hex to gather`, `Gathered
Iron ore`). The guide loop (gather → gold hub → research) is the one thing that already
  coaches. The header `Establish component production 0 / 3` never explains the 3.
- ~~Walking overshoots a single hex easily at hold-to-move speed.~~ Closed by v0.18, below.
- Console was clean except `favicon.ico` 404.
- Belts-on-fields may stay legal; paving the rare landing crystal without a read should not.
- ~~**The inspector is a wall of text.**~~ Closed by Inspector Readability: a clicked hex is
  cards, not a `textContent` dump. Coordinates are a chip, facing is a compass plus
  `DIRECTION_NAMES`, and every meter writes both published numbers.
- ~~**The scripted next action can recommend an impossible powered line.**~~ Closed by Founding
  Contract v0.18: there is no script. The step is a dependency walk over the contract, the recipe
  tree, and the technology graph, and `tests/guidance.test.ts` fails if the power branch leaves it.
- ~~**The objective proves only the founding slice.**~~ Closed by v0.18. Three Components are stage
  one of a two-stage contract; the project the hub actually builds needs two chains from two
  landscapes, and `fixtures/balance.json` is where that claim is checked rather than asserted.
- ~~**Minute zero shows the whole locked game.**~~ Closed by v0.18. Both catalogues lead with
  everything within two unresearched technologies — five of twelve and six of eighteen at minute
  zero — and each says how many it is holding back, behind one control.
- ~~**The world is clean but quiet.**~~ Closed by v0.18: six synthesised cues chosen from native
  events, a stall mark that says _why_ a machine is idle, and a stamp that no longer covers the
  anatomy it labels. Mute and reduced motion are controls, not assumptions.
- ~~**Walking overshoots a single hex easily at hold-to-move speed.**~~ Closed by v0.18. `Shift`
  walks at 0.4 magnitude through the intent field native already accepted.
- **The landing clearing answers exploration too early.** All eight raw materials at spawn make the
  terrain legible but make four presets and an unbounded map less consequential. v0.21 replaces the
  sample platter with a measured bootstrap guarantee and regional discovery.

## Presentation pass — Inspector Readability v0.13.2

The inspector is the one panel Sightlines left on the world, and the reason it stayed is that it
answers the hex the player just clicked. That answer is currently a preformatted paragraph.
Coordinates lead. Status, heading, stock, and cargo share one line. Direction is an integer.
Terrain is a clause. Nothing has a shape the eye can pre-attentively group. A player who already
knows the factory still has to parse; a player who does not cannot learn from it.

This is presentation over published snapshot facts. No save, definition, generator, or wire
version moves. No new native field. Any proportion still takes both numbers native already
publishes. Lists that carry a control are still patched in place. The host still does not
re-derive a maximum by watching a value count down.

### The defect, named

`renderInspector` joins an array of strings and assigns `textContent`. The stylesheet then
honours the newlines with `white-space: pre-line`. That is a log, not a readout. It also steals
the panel heading for the active tool (`Inspect`, `Extractor`, `Erase`), so the largest type on
the panel is often not even about the hex.

What a click has to answer, in the order a player actually asks:

1. **What is this?** A building, a field, a band, or fog.
2. **What state is it in?** Working, waiting, starved, protected.
3. **Where is it?** Axial `q, r` as a reference, not as the title.
4. **What ground is it?** Band, and whether the player may stand or build.
5. **What is on it?** Remaining field, stored stacks, belt cargo, craft, fuel, power.
6. **Which way does it face?** A heading a player can match to the world, not `Direction 0`.

The dock already names the active tool. The inspector heading is the hex.

### Visual language — reuse, do not invent a second one

The world already has a vocabulary. The inspector uses it rather than translating back into
prose.

| Fact                 | Visual                                                                                      | Source                                                                                                       |
| -------------------- | ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| Terrain band         | Pointy-top hex swatch, fill and edge from `TERRAIN_INFO`                                    | Same table as the map and the legend                                                                         |
| Impassable           | The shared diagonal hatch and the coral access label                                        | `fixtures/terrain-passability.json`, same treatment as the world                                             |
| Unsurveyed           | Fog fill `#18242f` and mint rim `#7fe0c8`, no band name invented                            | `docs/ART.md` fog row                                                                                        |
| Item                 | The twelve-glyph set at the item's identity colour, plus the name                           | `itemIconSvg`, same glyph as the pack and the field                                                          |
| Field remaining      | A meter of `quantity` against `initial_quantity`, both numbers written                      | Resource snapshot; host draws a proportion it was given                                                      |
| Flora                | A small `Regrows` chip, not a mid-dot clause                                                | `regrowth_ticks` on the item                                                                                 |
| Building             | Pointy-top hex in `BUILDING_COLORS[kind]`, three-letter stamp from `definition.icon`        | The same stamp the dock and the silhouette already use                                                       |
| Building status      | A coloured pill, not `Name · status`                                                        | Native spelling, grouped only for colour                                                                     |
| Facing               | A six-tick pointy-top compass with the live spoke lit, plus the word from `DIRECTION_NAMES` | Direction 0 is still east; the integer never reaches the player                                              |
| Craft / fuel / power | One meter each, both published numbers on the right                                         | `progress`/`progress_total`, `fuel_charge`/`fuel_required`, `power_satisfied`/`power_demand`                 |
| Belt cargo           | Glyph + name + count, labelled `On the belt`                                                | `building.cargo`                                                                                             |
| Container stock      | The pack's slot grid (glyph + count), each row a `Take`                                     | Patched in place; `quantity` is still the whole stored amount                                                |
| Protected hub        | A gold `Protected` chip                                                                     | `scenario_owned`                                                                                             |
| Empty ground note    | The band's `note` (`iron ore, coal, crystal`)                                               | Only when the cell is not already a field or a building — potentials on an iron cell were the v0.12.1 defect |

Status colour is a grouping, not a new spelling. Live (mint): `extracting`, `composing`,
`pumping`, `generating`, `carrying`, `receiving`. Waiting (muted): `idle`, `waiting for inputs`,
`buffered`. Stopped (coral): `output blocked`, `deposit depleted`, `out of fuel`, `no power`,
`brownout`, `no water in reach`, `no boiler`. Hub (gold): `landing hub`. The string the player
reads is still the native one.

### Layout

A static skeleton, patched every snapshot, never rebuilt. The panel heading is the identity; the
body is cards that hide when they have nothing to say. Nothing that is absent leaves a blank
ruled box.

```
┌─────────────────────────────────┐
│ BUILDING                    [×] │  kicker = Building / Field / Ground / Unsurveyed
│ Extractor          extracting   │  title = definition, item, or band name
├─────────────────────────────────┤
│  [EXT]          q  3            │  portrait is a hex, not a square
│   hex           r  0            │  coords are a chip, tabular, labelled
├─────────────────────────────────┤
│ [swatch] Highland    BUILDABLE  │  hatch on the swatch if impassable
│          (note only if empty)   │
├─────────────────────────────────┤
│ [ore] Iron ore        Regrows   │  field card, only when a resource is here
│ ████████░░  35 / 48             │
├─────────────────────────────────┤
│ Craft  ██████░░░░  12 / 20      │  machine meters, each omitted when zero
│ Fuel   ███░░░░░░░   8 / 12      │
│ Power  ██████████   4 /  4      │
│  [*]   Facing East   Protected  │  compass, not "Direction 0"
│ On the belt  [plate] Iron  1    │
│ Stored  [slot] [slot]  Take     │
└─────────────────────────────────┘
```

Empty selection: the sheet hides and the heading returns to `World inspector` / `Select a hex`.
Unsurveyed: fog portrait, title `Fog`, one line `Travel here to lift the fog`, no invented band.
A building on a field is still one hex: the heading is the building, the field card stays, the
band note does not — the cell already says what it holds.

The recipe `<select>` stays a patched control under the sheet, shown only when the machine has
two or more recipes and is not scenario-owned. Placement legality stays the gold line under
that, because it is about the pending tool, not about the hex.

### What this is not

Not a second panel. Not a tooltip that replaces the inspector. Not a native change. Not a
heading that still says `Inspect` while the body describes an extractor. Not a meter whose
maximum the host inferred. Not a `replaceChildren` of the Take rows. Not 3D, not a hand-drawn
atlas, not a new `BuildingKind`.

### Acceptance

- A clicked hex is readable in one glance: identity, status, band, and the fact that matters
  (remaining field, facing, starvation) each have a shape, not only a sentence.
- Coordinates are present and never the lead.
- Facing is a compass plus `East` / `Southeast` / … — `Direction 0` does not appear.
- Impassable ground in the inspector is the same hatch as impassable ground on the map.
- A field hex still leads with the field; band potentials stay on empty ground.
- Every meter writes both numbers. Every Take row is patched, not rebuilt.
- Desktop keeps the inspector pinned; the narrow layout still toggles it. No save, wire,
  definition, or checksum movement.

Target repository: `https://github.com/Sidem/HexFactory`

Target live MVP: `https://sidem.github.io/HexFactory/`

Project root: `X:\Programming\Projects\HexFactory`

Published geometry dependency: `@hexlife/embed@1.15.0` (exact pin).

Local source/reference checkout for that npm package: `X:\Programming\Projects\HexLife`. HexLife is
not a source dependency: HexFactory imports only the published package. Treat that checkout as
read-only unless a future task explicitly authorizes a separately released generic package change.

## Shipped implementation record

- Inspector Readability v0.13.2 is presentation over published snapshot facts. The inspector heading is
  the hex, not the active tool. Identity, axial `q, r`, terrain swatch (hatched when impassable),
  field meter, facing compass, craft/fuel/power meters, belt cargo, and container Take rows are
  each a shape. No save, generator, definition, or wire version moves. Take rows are still
  patched in place.

- Look Systems v0.13.1 is presentation over published snapshot facts. Neighbour fringes, baked
  terrain tiles, a host-side hex hash, depletion scars, silhouettes from `recipe_category`, and
  one Stage C motion pass (belt cargo, machine cycles, extractor pulses, water shimmer). Trim is
  reserved for `tier` so v0.14 is a data row. No save, generator, definition, or wire version
  moves. A complete browser frame at 6,144 entities is 3,170 µs, 19.0% of 60 Hz; the world is
  991 µs of that against the v0.12.4 baseline of 909 µs. Cite `docs/BENCHMARKS.md`.

- Power v0.13 is the second constraint. Poles compile connected components; each network holds
  integer supply and demand; brownouts advance `base * satisfied / demand` with a per-entity
  remainder so total work is exact. Extractors, composers, and pumps draw; belts, boxes, and the
  hub do not. Burner generators bootstrap from any fuel item; wind stands on hills and highland
  (the same ground as the ore); hydro and the boiler sit at a basin edge; the turbine is a
  generator that is live only beside a firing boiler. Water is still a belted item — say so in
  the notes, this is not a fluid network. `HXF1` save version is 6; definition version is 6;
  technology version is 3. v0.12.4 saves are rejected.

- Renderer Measure v0.12.4 is an engine record, not a play milestone. The browser harness now
  times the two canvases the game draws — `CanvasFactoryRenderer.draw` at a pinned 1440×900
  viewport and `MinimapRenderer.draw` at the shipped 178 px square — against the same tiers and
  the same snapshot the merge just produced. A complete browser frame at 6,144 entities is
  3,039 µs, 18.2% of 60 Hz; the world is 909 µs of that, the minimap 160 µs. The unknown 89% is
  gone. Both canvases build definition maps once so a per-entity linear `find` is not something
  the measurement had to answer. No save, generator, definition, or wire version moves.

- Sightlines v0.12.3 is a control and legibility release: it changes what the player can see and
  where their attention is allowed to go, and it changes no simulation result. The player points
  where the cursor points, through a new bounded `aim` command that carries the world position under
  the cursor and lets native resolve the facing vector in integer arithmetic — the host sends a
  target, never a heading. Facing is still not an input to gathering, and the reason has only
  half-changed: it is visible now, but a gather that took from a neighbouring hex because of where
  the mouse sat would still be a change with no cause the player can see. `Space` centres the camera
  on the player, so pause moved to `T`. Inventory, research, and the objective-and-controls guide are
  behind `I`, `O`, and `P`, leaving the inspector as the only panel on the world; gather, deliver,
  and the carried-slot count moved into permanent chrome rather than behind a key, because they are
  the loop rather than a reference. A minimap draws the surveyed world, the landing hub, and the
  player, and a gold bearing marker on the world edge names the distance home whenever the hub is off
  screen. Every impassable band — deep water, shallow water, cliff — now carries one shared hatched
  treatment, driven by `fixtures/terrain-passability.json`, which pins the band-to-passability rule
  in Rust and TypeScript at once so a decorative choice can never disagree with what
  `Terrain::blocks_movement` actually does. No save, generator, definition, or wire version moves,
  so v0.12.2 saves load and the recorded capacity ladder still compares directly.

- Playtest Feel v0.12.1 is the first-minutes follow-up v0.12 asked for, not a play milestone.
  Fields are sparser so barren ground is the common case: richness and vein gates sit around
  `50_000`–`56_000` instead of `22_000`–`46_000`, and the landing clearing stamps one cell of
  each material rather than nine including a second iron. `BASE_HEX_SIZE` is 22 px, so more of
  the lattice fits on screen; `PLAYER_RADIUS` stays 580. Resource counts are no longer written
  on every field hex — remaining amount is on the hovered cell (with the item name), on a cell
  that has been drawn from, and in the inspector, which now leads with the actual field rather
  than the band's potentials. A refused place names the missing item (`need 1 Signal crystal
(have 0)`) instead of `construction cost is not available`. Belt cargo is drawn larger so a
  shrinking hex does not make a running line look idle. `WORLD_GENERATOR_VERSION` is 5; the
  HXF1 save envelope is still version 5, and a v0.12 world is rejected because the generator
  changed, not because the bytes did. Checksum comparisons are invalidated; timing ones are
  not.

- Material Base v0.12 gives the world more than one thing to be made of. Eight raw resources —
  iron ore, copper ore, coal, stone, sand, clay, wood, and water — are generated where their
  geography says they belong, which is what turns terrain from a colour into information: iron and
  coal on highland, copper on the new **hills** band between lowland and highland, sand and clay on
  shores, stone on cliffs the player cannot stand on and quarries from the hex beside, wood in moist
  lowland, and water pumped rather than mined. Fourteen recipes across five machine categories run
  on one `Composer` kind: smelter, kiln, cutter, crusher, and the existing composer differ by a
  `recipe_category` field and one check at assignment, not by a new tick path each. **Fuel is a
  property of the item, never a recipe input** — a smelting recipe names no fuel, so coal, charcoal,
  and wood are interchangeable at different values and every fuel added later is too; a machine
  burns from its own stock and never from the quantity a recipe input reserves, which is what keeps
  steel (whose inputs name coal as carbon) from starving itself. Charcoal is deliberately fuel-free,
  so a player who lands away from a coal field still bootstraps smelting from trees. Wood is the one
  renewable source: `regrowth_ticks` on the item makes a cut cell climb back to what generation gave
  it, walked from a derived set of cut cells rather than from the world, so an untouched forest costs
  nothing and a regrown one costs nothing again. `Pump` is the only new `BuildingKind`, because it
  draws from terrain rather than from a deposit and its basin never empties. `set_recipe` joins
  `place`, `erase`, and `withdraw` as a bounded, range-checked command; it refuses a machine
  mid-craft, because the inputs it reserved belong to the job it is running.
  Two player-facing changes ride along. The wait between one field action and the next is now a ring
  that closes around the player instead of a "cooling down" line in the message strip — both numbers
  are native, `action_cooldown` against a published `action_cooldown_total`, so the host draws a
  proportion it was given. And the inspector names every surveyed hex and what its band is good for,
  where before it described only buildings and resources and left the coloured tiles unexplained.
  `WORLD_GENERATOR_VERSION` is 4 and `HXF1` save version is 5; there is no migration from a
  three-item world to a twenty-three-item one.

- World Shape v0.11 replaces salt-and-pepper circles with a single axial lattice. Elevation and
  moisture are integer value noise; terrain is read from bands (deep water, shallow water, shore,
  lowland, highland) and cliffs from the elevation gradient. Resource fields are a pure function of
  seed and hex that returns `(item_id, richness)`; only a sparse depletion overlay is stored, saved,
  or checksummed. Extractors harvest every field cell within hex radius 1, nearest first. Player
  radius is published on the snapshot and the host draws from it (580 world units, speed 242).
  `WORLD_GENERATOR_VERSION` is 3 and `HXF1` save version is 4; earlier envelopes are rejected. Stage
  A art direction lives in `docs/ART.md`.

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

## Shipped milestone — Sightlines v0.12.3

Sourced from seven directions given on 2026-08-18, recorded here as they were given:

- player should always face towards the cursor (will add weapons and shooting later)
- space key should center the player in the camera
- inventory and research should be behind menus to open/close by pressing "I" and "O"
- only the inspector should remain in the main view
- we need a minimap and way to find back to the base
- "p" hotkey should open the quest/objective and controls menu
- impassible and passable terrain needs to be clearly apparent, right now its hard to distinguish

They are one milestone because they are one complaint. The design pillars ask that the world reward
looking at it and that the player always know what just happened; six of these seven say that the
screen is currently spending itself on panels and flat colour instead. The seventh — facing the
cursor — is the aiming half of a control scheme whose other half arrives with weapons, and it is
worth landing early because it costs a bounded command and it is what makes the player read as
something with a front.

### Facing the cursor

Facing is native state and a checksum input, so this is not a renderer change. The host sends
`aim`, carrying the **world position under the cursor**, and native resolves the facing unit vector
from the delta to the player in integer arithmetic. The host sends a target and never a heading:
normalizing a continuous pointer angle in host floating point would be TypeScript computing a
checksummed value, which is a different thing from the fixed eight-entry table `move_intent` picks
from.

Three consequences worth stating rather than discovering:

- **Movement still sets facing, and aim simply wins.** No flag, no stored aiming mode, no save
  change: an aim is enqueued last in the frame it belongs to, so with a pointer over the world the
  cursor decides, and on the touch layout — where nothing sends `aim` — the walk direction still
  decides, exactly as before.
- **The aim is re-sent as the player walks.** A stationary cursor and a moving player is a changing
  angle, so the host recomputes the bearing every frame from the pointer's last position and sends
  an `aim` only when the whole-degree bearing changes. That threshold is a host decision about when
  to speak, not about what facing is.
- **Gathering still ignores facing.** The invariant that recorded this said facing was not an input
  _because nothing showed it_. Half of that reason has now expired, and the other half has not: a
  harvest that drained a neighbouring hex because of where the mouse was resting is still an effect
  whose cause the player cannot see. If a future milestone wants facing-weighted targeting it has to
  argue for it on its own, with a visible target.

### The camera, and where pause went

`Space` recentres the camera on the player and restores following after a pan — the existing
`recenter` path, now on the key the direction asked for. Pause moves to `T`. There is no free
mnemonic left once `I`, `O`, and `P` are spoken for, and pause keeps its on-screen button, its
`aria-pressed` state, and its place in the controls panel.

### Panels behind keys

`I` opens the cargo pack, `O` opens research, `P` opens the objective and the controls reference,
and `Escape` closes whatever is open. They are mutually exclusive through the same `closePanels`
path the game menu already used, so at most one panel is ever over the world and the inspector is
the only thing pinned there.

Two things deliberately did **not** go behind a key. Gather and deliver moved into the build dock,
and the carried-slot count into the command bar, because they are the minute-to-minute loop rather
than a reference the player consults: putting the first action a new player needs behind a keypress
they have not learned yet would trade one legibility defect for another. The dock widened to take
them, which the corridor complaint from the v0.12.1 playtest had made room for.

### The minimap and the way home

A second canvas, drawn only when a snapshot arrives, showing a fixed-radius window on the world:
surveyed chunks against unsurveyed fill, terrain by band, buildings by kind, the landing hub in gold,
and the player with a facing tick. It derives nothing the snapshot does not already carry.

Finding the base again is the harder half, because a minimap only answers it while the base is on
the minimap. So the hub also gets a **bearing marker on the world edge**: whenever the landing hub is
off screen, a gold chevron sits at the edge of the viewport in its direction, labelled with the
distance in hexes. The player is never more than one glance from knowing which way home is, at any
distance, which is what makes walking to the fog frontier a decision rather than a risk.

### Impassable ground

Cliff against highland was two greys a step apart, and the player learned the difference by walking
into it. The fix is not a better pair of greys — it is one shared treatment that says _impassable_
regardless of which band it is: a hatched fill and a bright rim on deep water, shallow water, and
cliff alike, so the category is legible before the material is.

What decides that category is native's, not the renderer's. `fixtures/terrain-passability.json`
lists every band with whether it blocks movement and whether it blocks construction; Rust asserts
the file against `Terrain::blocks_movement` and `Terrain::blocks_construction` over an exhaustive
match, and TypeScript asserts it against the host's terrain table, which is now one module feeding
the renderer, the inspector, and the new legend. That is the `fixtures/hex-directions.json` idiom
applied to a second cross-language rule, and it costs the wire nothing: no byte was added to the
terrain group to carry a fact both sides can be pinned to instead.

### Acceptance and release gate

- The player's facing tracks the cursor in a real browser, keeps tracking it while walking, and
  falls back to the walk direction on the touch layout where no pointer aims.
- `Space` recentres; `I`, `O`, `P` open and close their panels; `Escape` closes them; `T` pauses.
  Every one of them is reachable by keyboard, has an accessible name, and is listed in the controls
  panel.
- The inspector is the only panel over the world at 1440×900, and the narrow layout still plays.
- The minimap shows the hub and the player, and the bearing marker names the distance home whenever
  the hub is off screen.
- Impassable ground is distinguishable from passable ground at a glance, and the rule that decides
  which is which is pinned in both languages.
- Determinism, save, wire, definition, and dependency contracts are unchanged. `aim` is bounded and
  range-checked like every other command, and a forged one is rejected the same way.

## Shipped milestone — World Shape v0.11

Sourced from the 2026-08-17 design conversation. The world is now worth walking across: fields
instead of point deposits, landforms instead of hashed puddles, and one lattice so generation and
construction agree.

### What shipped

- **One lattice.** Every tile sits at `axial_world(q, r)`. `ensure_neighborhood` converts the
  player to axial with cube rounding and generates that hex chunk plus its six neighbours.
  `FEATURE_SPACING` is gone.
- **Terrain bands.** Integer value noise for elevation and moisture. Deep water, shallow water,
  shore, lowland, highland, and cliffs from the elevation gradient. A radius-7 landing stays
  lowland except for a small pond and two cliffs that keep the tutorial readable.
- **Resource fields.** A pure function of seed and hex. Iron ore clusters in highlands, crystal in
  moist highland and lowland. Guaranteed landing cells keep the first loop stable (v0.12.1 thinned
  them to one of each material). The tile map is a sparse depletion overlay: unmined field is not
  stored, saved, or checksummed.
- **Extraction radius.** An extractor harvests every field cell within hex distance 1, nearest
  first, then by cell key. Yield continues from farther cells as nearby ones empty. Overlap still
  arbitrates by stable entity ID.
- **Player scale.** `PLAYER_RADIUS` is 580 and `PLAYER_SPEED` is 242. The snapshot publishes the
  radius; the host draws the body and the collision ring from it.
- **Stage A art.** Palette and shape language in `docs/ART.md`, geometric item icons for ore,
  crystal, and component, and a still mockup at `docs/art/world-shape-still.png`.

### Compatibility

`WORLD_GENERATOR_VERSION` is 3 and `HXF1` save version is 4. There is no migration from
salt-and-pepper circles to fields.

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

| Milestone                  | Unlocks                                                    | Depends on                        |
| -------------------------- | ---------------------------------------------------------- | --------------------------------- |
| v0.11 World Shape          | A world worth walking across and choosing a site on        | v0.10 item 1                      |
| v0.12 Material Base        | A production tree instead of one recipe                    | v0.11 fields                      |
| v0.13 Power                | A second constraint that reshapes layout                   | v0.12 materials                   |
| Look Systems (Stage B + C) | A world that rewards looking at it; tiers stay a data row  | v0.12 roster and v0.12.4 measure  |
| v0.14 Upgrades and Tiers   | Growth in place; extraction radius as the flagship upgrade | v0.13; cheaper after Look Systems |

All five have shipped. What follows records what v0.12 actually decided where it differed from the
plan it was written against.

### The generator arc — v0.15, v0.16, v0.17

Sourced from a design conversation on 2026-08-18. Three milestones that are not play systems: they
are what the next play systems get built on. The dependency chain is as real as the last one —
shapes are presentation and independent, world parameters change what a world contains, and balance
is tuned against what a world contains, so it goes last. Full briefs are at the top of this file.

| Milestone              | Unlocks                                                          | Depends on                  |
| ---------------------- | ---------------------------------------------------------------- | --------------------------- |
| v0.15 Generated Shapes | A tier that is visible as a machine, not as a stroke colour      | v0.14 roster; nothing else  |
| v0.16 World Parameters | Worlds chosen by shape and abundance, not only by seed           | nothing; bumps the envelope |
| v0.17 Balance          | An economy with a stated curve that a change can be read against | v0.16 density parameters    |

**Why generators before play systems, again.** This is the argument Look Systems already made and
won, applied twice more. Every deferred play milestone multiplies a roster: animals add creatures
and byproducts, fluid networks add pipes and pumps, intermittency adds accumulators. Against a
switch statement each of those costs drawings; against a grammar each costs a data row. The same
holds for the world — a biome milestone against hardcoded thresholds is a rewrite, and against a
parameter table is a preset. Taking the three cheap-forever passes now is what keeps the expensive
ones cheap.

**What each one costs on the version axis.** v0.15 moves nothing: presentation owns no truth.
v0.16 bumps `WORLD_GENERATOR_VERSION` to 6 and puts `WorldParams` in the envelope and the checksum.
v0.17 moves definition data, so it bumps the definition version and nothing else — the fixture it
adds is a test artifact, not a wire or save concern.

**All three have shipped, and v0.17's version cost was one line off.** The definition version went
to 8 as predicted and the fixture stayed a test artifact as predicted, but the paragraph missed
that the browser `SAVE_KEY` named only the save and world versions while native refuses a load on
four. A definition-only bump was therefore the one bump the key could not see, so it now names all
four: `hexfactory:hxf1:v7w6d8t4`. The other thing the prediction missed is that one tuned number
was not in the definition file at all — `GATHER_COOLDOWN_STEPS`, the player's own gather rate,
which is the number the fixture found most out of line and which no data file could have carried.

**Where the engine milestones slot.** The compact binary delta encoding landed as v0.12.2, on the
boundary this paragraph set for it and for the reason it gave: every milestone here grows the
snapshot — more item IDs in more inventories, terrain with more bands, then a power network with a
per-entity satisfaction figure — and growing the payload before compacting it would have spent the
measured headroom on the wrong thing. Power can now add a per-entity figure to the wire against a
payload that is 13.6× smaller than the one that priced the worry.

The renderer measurement that paragraph moved to the front of the queue landed as v0.12.4: a
complete browser frame at the largest tier is 18.2% of 60 Hz, rendering is 6.4% of it, and Stage
C is no longer gated on ignorance. On 2026-08-18 the look systems were pulled in front of
v0.14: the roster is stable, the frame is a number, and a generator now is what keeps a later
tier from costing a drawing. The drag's per-cell transport recompile has no dependency here
and can land anywhere.

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

### v0.12 — Material Base (shipped)

Eight raw resources and a first processing tier. The point is not quantity; it is that a material
should arrive from somewhere specific and become something the player wanted.

**What shipped differently from the plan below.** Three deliberate departures, plus one addition:

- **Hills became a real terrain band.** The plan wanted copper in "hills" while v0.11 had shipped a
  single raised band. Rather than correlate copper and iron by noise inside one band — which would
  have made the distinction invisible on the map — `Hills` was added between lowland and highland.
  Terrain being the material map only works if the player can see the bands apart.
- **Stone lives on cliffs, which nothing can stand on.** The plan listed stone at "cliffs, rock".
  Cliffs are impassable and unbuildable, so a stone field there is reached from the hex beside it,
  through the extraction radius. That is the cheapest possible lesson in what the radius means, and
  it makes cliffs geography rather than an obstacle.
- **Fuel is charged at craft start, not spread over the duration.** A recipe declares `fuel`, a
  machine banks a `fuel_charge` from whatever it burns, and the charge is spent when the craft
  begins — beside the inputs it reserves. A half-finished job can never hold energy it has not paid
  for, and no per-tick fuel arithmetic enters the hot loop.
- **`set_recipe` was added.** Not in the plan, and needed by it: fourteen recipes across five
  categories make "erase and rebuild to change a job" friction on every layout decision. It refuses
  a machine mid-craft rather than deciding what happens to reserved inputs, which is the same
  question that still keeps composers from being unloaded.

The rest of this section is the plan as written, kept for the reasoning behind each choice.

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
power site becomes a reason to have explored.

Site the terrain gates so that power and extraction **compete for the same ground**. Wind wants
highland and hills; iron, coal, and copper are already there. A player who takes the ridge for
turbines gives up the ore under them, and that is a real decision that neither system authored —
which is the whole argument for a second constraint. Tune the gates for that collision deliberately
rather than letting the two roster tables land on disjoint bands by accident. Keep wind at a fixed output for this milestone;
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

### Look Systems — Stage B generator, then first Stage C motion

Shipped 2026-08-18 as v0.13.1. Not a play-systems milestone: no new recipe, no new building
kind, no save bump. The five rules live in `docs/ART.md`. The 2D start of the organic-generation
horizon landed here, plus one motion pass, then a re-measure: 19.0% of 60 Hz at 6,144 entities,
world 991 µs. 3D, north-south belts, and the later tileable-texture systems stay on the longer
horizon.

### v0.14 — Upgrades and Tiers

Shipped 2026-08-18; the record is at the top of this file. The originally-deferred milestone, with
something to spend and something to improve, and with a generator that can paint a new tier without
a new drawing — the half of that promise the map did not show is what v0.15 finishes.
Tiered building definitions, an upgrade command that preserves contents, orientation, and
connections, and the progression that earns them. **Extraction radius is the flagship upgrade**
— it is visible on the map, it changes a decision the player already made, and it demonstrates
what tiers are for better than a bigger box does. Larger containers, faster smelters, and more
efficient generators follow the same pattern.

### Deferred beyond this arc

Named here so they are decisions rather than omissions, each with the thing it is waiting for:

- ~~**Animals, biomatter, and waste.**~~ Promoted to **Living Lattice v0.20** above, still one
  milestone rather than three. The promoted brief adds the missing purpose: ecology is the first
  system in which the world answers the factory, and the first player-facing reason hex
  neighbourhoods must matter.
- **Fluid networks.** Water ships as a belted item in v0.13; the real network is an improvement on a
  working game rather than a second network model built in the same milestone as the first.
- **Intermittent generation and accumulators.** They arrive together. Intermittency has to be a
  deterministic function of tick and position, never a runtime roll, and that is its own design
  pass.
- **A day cycle, and solar with it.** A day cycle is a presentation and simulation change at once
  and should be chosen for what it does to the game's feel, not smuggled in as a power source.
- **Terraforming.** Cliffs are unbuildable until mined in v0.11; whether the player may reshape
  elevation, and what that costs, is a question the world has to exist before anyone can answer.
- ~~**Regional biomes — a third generation channel.**~~ Promoted to **Regional Discovery v0.21**
  above. The generation channel remains an implementation option; the milestone is the readable
  expedition, the sustained distant site, and the measured replacement for "all materials near
  spawn," not another noise field by itself.

### Longer horizon — 3D, north-south belts, organic generation

Named 2026-08-18 from three directions given in one sitting. Power and the renderer measurement
have since shipped, and Look Systems (the 2D start of the organic-generation item) has shipped.
**North-south belts have since left this horizon**: the design resolved to a direction-table row
and moved into v0.14, and its write-up below is kept here because that is where the reasoning
lives. 3D presentation is still not the next session. It is the
destination the current architecture has to stay pointed at, so a 2D choice that would make
them expensive later is the thing to refuse.

#### 3D presentation

The game eventually leaves the top-down hex view. The camera tilts and is free to orbit the player;
the player, the terrain, and the buildings gain 3D shape.

This is a renderer replacement, which the existing invariant already allows: Canvas 2D is
replaceable presentation and simulation truth comes only from native snapshots. The axial lattice,
the compiled graphs, and the native tick stay. Height is not implied as a gameplay dimension until
a later pass names what it is for; smuggling a z-axis into the checksum because the camera can tilt
would be the same class of defect as frame-coupled movement.

The renderer measurement still gates any renderer decision. Stage C already says: measure first,
then decide whether the animated frame wants a different renderer. A 3D renderer is that decision,
made later, with a number in hand.

#### North-south belts — resolved, and no longer a longer-horizon item

Superseded 2026-08-18. This section previously proposed a two-row anchor period whose offset tiles
were _half-covered_ by the belt footprint, and named the open question as what a half-covered hex
_is_ — blocked, shareable left-half / right-half, or presentation-only. **That question is
withdrawn.** Sub-hex occupancy is the most expensive answer available: it would change the
placement predicate, the compiled transport graph, and the checksum at once. It is not needed,
because the lattice already contains the direction.

**Due north is a lattice vector.** Pointy-top world-x is proportional to `q + r/2`, so `(q + 1,
r - 2)` has exactly the same world-x as `(q, r)`, two rows up. `(+1, -2)` is due north and
`(-1, +2)` is due south. They are lattice vectors; they are simply not _unit_ vectors, which is the
only reason they were never in the direction table.

**The transport graph is already a ray-cast.** `compile_graph_target` steps `(dq, dr)` up to
`GRAPH_TRACE_LIMIT`, skipping the entity's own footprint cells, and returns the first other
occupied cell. Nothing in that loop assumes a unit step. Given a non-unit step it is already
correct, unchanged.

So north-south is a **direction-table change, not a geometry change**:

```rust
const TRANSPORT_DIRECTIONS: [(i32, i32); 8] = [
    (1, 0), (0, 1), (-1, 1), (-1, 0), (0, -1), (1, -1),  // the six, unchanged
    (1, -2),                                              // 6 = North
    (-1, 2),                                              // 7 = South
];
```

A riser at `(q, r)` facing north links to whatever occupies `(q + 1, r - 2)`. The two straddling
hexes `(q, r - 1)` and `(q + 1, r - 1)` are **never occupied** — they stay free, buildable, and
walkable. The belt spans the seam where those two hexes meet, which is what it looks like: a short
gantry over the crack, not a tile that is half of something.

##### What this costs, named

- **`orientation % 6` becomes `% 8`, and that is checksum-affecting.** It needs a save and
  definition version bump. v0.14 already pays for one, so landing this in or beside v0.14 is nearly
  free on that axis and expensive outside it.
- **Two tables, not one.** `DIRECTIONS` (six) stays the _adjacency_ table — `adjacent_live_boiler`,
  `adjacent_turbine`, power. Only _routing_ gets eight. Conflating them would silently let a boiler
  reach two rows.
- **Risers are single-cell only.** `@hexlife/embed` rotates footprints by 60°; orientations 6 and 7
  have no 60° equivalent. `place` validation rejects a north or south orientation on any multi-cell
  definition. Belts and pipes are what need this, so it is not a practical limit.
- **`hex_line` needs an explicit vertical rule** — the one genuinely fiddly part. Its greedy
  "lowest-numbered direction that strictly closes the distance" would never select north or south,
  because a unit step almost always closes too and a north step is `axial_distance` 2. The drag
  resolver needs a real rule: within some angle of vertical, use the two-row period. Integer-only
  and deterministic, like everything else on a state-affecting path. Appending north and south at
  indices 6 and 7 means **every existing drag resolves identically**, which is the property that
  keeps the existing tests meaningful.
- **Balance is a data row, not a mechanism.** A north step covers `3 · size` of world distance
  against `√3 · size ≈ 1.73 · size` for a unit step, so an unpriced riser is strictly dominant.
  Cost 2× a belt at the same throughput. The alternative — a two-cell footprint `{(q, r),
(q + 1, r - 2)}`, which the trace loop already handles because it skips own-footprint cells —
  is only available if non-contiguous footprints survive picking and silhouette drawing. Check
  that before choosing it.

##### Tunnels and bridges — the second axis, and one match arm

Span is a separate question from direction, and the same loop answers it:

```rust
Some(target) if target == index => { q += dq; r += dr; }
None if entity.is_tunnel() && steps < span => { q += dq; r += dr; }
target => return target,
```

A tunnel entrance rays through empty ground and binds to the first entity it reaches. Belts can
cross; the covered cells stay free and walkable; the surface is undisturbed, which suits a game
with a walking player and collision. It composes with all eight directions, and because it lives in
the graph trace rather than in the belt, **pipes inherit it for free** when fluid networks land.
That is the argument for solving both of these at the direction and graph level rather than as a
belt special case.

##### Why this is the answer to "people love square grids"

What a square grid actually offers is not squareness — it is that **both screen axes are
available**, so builds are axis-aligned, compose into rectangles, and can be read and copied. Six
hex directions give a horizontal axis and four diagonals, and no vertical. Eight give a full
compass with true axis alignment, the same expressive set a square grid offers, while terrain,
adjacency, and world generation stay hex. Players think in rows and columns; the lattice stays what
it is.

A 30° camera rotation is explicitly **not** the answer: aligning a hex edge to screen-up only swaps
which axis is awkward, and rotates the terrain with it.

The six hex-edge directions already run straight and do not change.

#### Organic generation

Stage B already said the art is a generator, not an atlas. The longer form of that rule is the one
that makes a hex world stop looking like a hex world: systems that procedurally produce **tileable
textures and shapes**, so rigid hexagonal tiling reads as organic terrain and organic objects.

Stage B's neighbour fringes, host-side hash variation, and depletion scarring are the 2D start of
that, and they shipped as Look Systems. They stay 2D. The later systems inherit the same
invariants: generated, presentation-only, derived from published snapshot facts (band, neighbours,
richness, remaining quantity, `recipe_category`, tier), never a checksum input, and a new
building or a new terrain band costs a data row rather than a drawing.

That is also why 3D shape and this generator are the same programme. A 3D building mesh that is
hand-authored per definition is the atlas again. A 3D building mesh derived from `recipe_category`
and tier is the generator, just in another dimension.

### Art direction and sprites — when

Three stages, gated on what would otherwise be redrawn or unaffordable.

**Stage A — art direction, during v0.11.** No engine change. The terrain bands need a palette
before they can be drawn at all, so the direction pass is not optional work that happens to be
early; it is a v0.11 dependency. Deliverables: palette for the elevation and moisture bands, shape
language for buildings, the rule for how a sprite fits a hex cell, and one still mockup of a running
factory to argue about. Also define the item icon system here and apply it to the current three
items — v0.10's inventory grid will be displaying `"icon": "ORE"` string codes, and that is the
cheapest visible improvement available.

**Stage B — a sprite generator, shipped as Look Systems v0.13.1, before v0.14.** The building
and item roster is not stable until the material base lands; drawing sprites before that
guarantees redrawing them. It was stable, Power had shipped, the frame was measured, and the
thing to build was the generator rather than the atlas. Pulled in front of v0.14 on 2026-08-18
so a later tier costs a data row rather than a drawing. This entry originally read "the full
item icon set and static building sprites as an atlas". The item glyphs already work the
generator's way, with twelve glyphs carrying twenty-three items; Stage B extends that rule to
buildings instead of abandoning it. Five rules, stated in full in `docs/ART.md`. Simulation
truth is never involved — rendering consumes snapshots and owns nothing. Three of the five
rules add per-hex renderer work; they shipped with a re-measure. Still Canvas 2D.

**Stage C — animation, in the same session after Stage B's stills.** Belt motion, machine work
cycles, extractor pulses, and water shimmer are per-frame per-entity draws. The first motion
pass shipped with Look Systems. A complete frame at 6,144 entities is 19.0% of 60 Hz and the
world draw is 991 µs. A renderer replacement still waits on a later measure.

## Shipped milestone — Game Feel v0.9

v0.4 built a command surface that presents the game well. v0.9 is about what it feels like to
operate: the moment-to-moment loop of moving, aiming, placing, routing, and correcting. The
simulation is correct and fast enough that the honest limit on enjoyment is now ergonomic. Nothing
here changes what the game means — it changes how much friction sits between intent and result.

The measured engine follow-ups in `docs/BENCHMARKS.md` are not cancelled and not reordered among
themselves; they are deferred behind this one. (Both that paragraph named have since moved: the
encoding shipped as v0.12.2, and the renderer measurement it promoted shipped as v0.12.4.)

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
- **Open-ended, not aimless.** Progression opens options rather than prescribing a route. The world
  is unbounded and the player decides what to build, where, and how large. Victory is a milestone in
  a longer game, never a wall; visible hub programmes, regional discoveries, and consequences give
  that longer game reasons without turning it into a script.
- **The world and the factory answer each other.** Terrain is more than a placement mask and the
  factory is more than an overlay. Geography, living populations, extraction, waste, and recovery
  change one another in ways the player can see and choose around.
- **Hexagonal space earns its place.** The package dependency is already real geometry, but a
  player-facing system becomes hex-native only when faces, rings, fronts, or multiple approach
  directions change a legible decision. Never force the factory into a cellular-automaton kernel or
  add invisible adjacency bonuses merely to justify the grid.
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
