# HexFactory art direction — Stage A

Stage A is the palette and shape language the World Shape renderer needs, extended by v0.12 to the
material roster. Buildings stay geometric hexes. The roster is now stable, Power has shipped, and
**Stage B shipped as Look Systems v0.13.1** — a generator that emits the art, not an atlas
somebody drew. The rule and what it buys are in "Stage B is a generator" below. Upgrades and Tiers
shipped as v0.14, and **the next milestone is Generated Shapes v0.15** — Stage D below, which
finishes Stage B's rule 4 by making the drawing itself a data row and making a tier visible as a
machine rather than as a stroke colour. The session brief is at the top of
`docs/HEXFACTORY-PLAN.md`.

## Palette

Surveyed lowland is the default fill and is not sent as terrain. Everything else is a hex
cell with its own fill and edge.

| Band          | Fill      | Edge      | Role                                     |
| ------------- | --------- | --------- | ---------------------------------------- |
| Deep water    | `#0f3550` | `#3f9ad0` | Impassable basin; pumped from the shore  |
| Shallow water | `#1a5474` | `#5cb6d8` | Impassable shore water                   |
| Shore         | `#c4a56a` | `#e0c88a` | Walkable, buildable; sand and clay       |
| Lowland       | `#1a3a32` | —         | Default surveyed ground; flora and clay  |
| Hills         | `#48604d` | `#6f8a6c` | Walkable, buildable; copper ore and coal |
| Highland      | `#5c6b58` | `#8a9a84` | Walkable, buildable; iron ore and coal   |
| Cliff         | `#57493e` | `#c19a72` | Impassable landform edge; stone          |
| Fog           | `#18242f` | `#7fe0c0` | Unsurveyed world                         |

Hills sits between lowland and highland and is deliberately close to both: the bands read as one
rising landform, not as three unrelated colours. v0.12 added it because copper belongs to rolling
ground and iron to the tops, and a player who cannot see the difference cannot choose a site.

**Impassability outranks the band.** Deep water, shallow water, and cliff each keep the fill above,
but all three carry one shared treatment — a diagonal hatch and a bright rim — so a player reads
"cannot stand here" before they read which material it is. That is deliberately not a fourth colour:
cliff against highland was two greys a step apart and the only way to tell them apart was to walk
into one, and tuning those greys would have kept the answer in the palette when the question is a
category. Which bands are in that category is native's rule, pinned by
`fixtures/terrain-passability.json`; the renderer reads the table and never decides for itself which
grey means cliff.

## Item colours and glyphs

Items keep their identity colours, and the glyph set names material _forms_ rather than individual
items — iron and copper ore share the faceted-hex `ore` glyph and differ by colour, as do every
plate and every kind of grit. Twelve glyphs carry twenty-three items, and Stage B's generator
inherits the same rule.

| Glyph       | Items                                                   |
| ----------- | ------------------------------------------------------- |
| `ore`       | iron ore, copper ore                                    |
| `lump`      | coal, stone, charcoal                                   |
| `grains`    | sand, clay, gravel                                      |
| `log`       | wood, timber                                            |
| `droplet`   | water                                                   |
| `plate`     | iron plate, copper plate, glass, brick, concrete, steel |
| `wire`      | copper wire                                             |
| `gear`      | gear                                                    |
| `frame`     | frame                                                   |
| `circuit`   | circuit                                                 |
| `crystal`   | signal crystal                                          |
| `component` | component                                               |

## Shape language

- Buildings are pointy-top hex prisms. Identity is a three-letter stamp and a facing tick,
  not a pictorial silhouette.
- A sprite, when one exists, occupies the inner 60% of the hex so neighbours never clip it.
- The same glyph is used in the pack and on the field, so a field cell and the stack it becomes are
  visibly one material.
- State that the player has to react to is drawn where it happens rather than written in the message
  strip: a machine's progress arc, and the ring that closes around the player while a field action
  is cooling down.

## Stage B is a generator

Stage B was originally written as "the full item icon set and static building sprites as an atlas".
That is N drawings, and it used to sit immediately before v0.14 Upgrades and Tiers — the milestone
whose whole job is multiplying the building roster. An atlas makes a tier cost a drawing; a
generator makes a tier cost a data row. Pulled in front of v0.14 on 2026-08-18 so the generator
exists before the roster multiplies, and so the colored mosaic is not what the next play session
stares at. The item glyphs already follow the generator's logic — twelve glyphs carry twenty-three
items — so Stage B extends that rule to buildings rather than abandoning it.

Rendering consumes snapshots and never owns simulation truth, so none of this can reach a checksum by
construction. That invariant is what makes generated art free here rather than risky: a host-side
hash, a noise field, and a baked tile are all presentation, and presentation may vary however it
likes. The available input is also better than geometry — a hex knows its band, its neighbours'
bands, its richness, and how much has been taken from it, because native already publishes all of it.

### The rules

1. **Transitions come from neighbours.** A hex is currently a flat fill and a stroke, which reads as
   a colour-block mosaic. Where a hex's band differs from a neighbour's, draw a fringe toward the
   lower band. Shore becomes a shoreline and a cliff becomes the edge of a landform. No new art and
   no new native data. This is the largest readability return available, and it is what makes
   "terrain is the material map" true on screen rather than only in the generator.
2. **Variation comes from a hash.** Rotation, in-band value jitter, and a few scattered detail marks
   keyed off `hash(q, r)` in the host, so a band stops reading as one repeated tile. The hash is
   presentation-only and must never become an input to anything native.
3. **Tiles are baked, not shipped.** Value noise, threshold, and edge darkening, run once at startup
   into offscreen canvases behind a version constant, rather than PNGs in the bundle. `veilCanvas()`
   in the renderer is already this pattern. Changing a constant regenerates the whole set.
4. **A building's look is derived from its definition.** Silhouette from `recipe_category`, which
   already distinguishes smelter, kiln, cutter, crusher, and composer; trim from tier; the facing
   tick that is already drawn. This is the rule that makes v0.14 cheap.
5. **Depletion is visible history.** `quantity` against `initial_quantity` is already stored, saved,
   and read by the renderer — today only to decide whether to draw a number. Let it desaturate and
   scar the ground as well, so a worked-out region is legible from across the map. Flora regrowth
   runs the same system in the other direction: a cut forest visibly recovering is already simulated,
   and needs only to be drawn.

### Sequencing

Shipped as Look Systems v0.13.1, before v0.14. Order of work was in `docs/HEXFACTORY-PLAN.md`
under **Next session — Look Systems**: fringes, baked tiles, hash variation, depletion, building
silhouettes, one Stage C motion pass, then `npm run bench:browser`.

Rules 1, 2, and 5 add per-hex renderer work. v0.12.4 measured the frame this pass started from:
the world was 909 µs at the largest tier and a complete browser frame was 18.2% of 60 Hz. The
Look Systems re-measure is 991 µs for the world and 19.0% of 60 Hz. Stage B's per-hex work
shipped with that number, not ahead of one.

## Stage D — the shape grammar, as Generated Shapes v0.15

Directed 2026-08-18. Stage B established that a look is _derived_ and shipped that rule for
terrain, for depletion, and for the choice of which building silhouette to draw. Stage D applies it
to the drawing itself, which is the one place Stage B left imperative.

### What Stage B left behind

`silhouetteOf` in `src/rendering/buildingLook.ts` is correct and stays: `recipe_category` splits the
composer kinds and `power_source` splits the generators, from the definition, with no per-id case.
Two things under it are not finished.

- **`drawSilhouette` is a two-hundred-line `switch` of hand-written canvas calls.** A new building
  costs a new arm. That is an atlas whose drawings happen to be written in TypeScript, and it fails
  the same test Stage B was created to pass: a new definition should cost a data row.
- **`trimOf` renders a tier as stroke colour and width only.** So a deep extractor is an extractor
  with a gold outline. The milestone whose subject was growth in place produced no visible growth,
  which makes rule 4 half-true: the look is derived from the definition, but not from the part of
  the definition that changed.

### The rule

**6. A shape is a part list, and a tier is a modifier on it.** One renderer walks a declarative list
of parts. The vocabulary is small and names machine anatomy rather than geometry — vessel, chamber,
stack, rotor, aperture, mast, band, mouth — and each part carries anchor, scale, rotation, and
animation phase. Phase is what keeps Stage C's motion inside the grammar instead of beside it: a
rotor already turns on `workCycle`, and in a part list that is a property rather than a bespoke arm.

Composition is three lookups and no cases. `kind` / `recipe_category` / `power_source` selects the
base part list. `tier` applies modifiers from a named, documented set — add a stack, add a rotor
blade, segment the vessel, add a plating band, widen the mouth — so **an upgrade changes the
silhouette**. Terrain and the player draw from the same vocabulary, so the world reads as one system
rather than three sharing a palette.

The rules Stage B already set all still hold and are what make this safe: baked behind a version
constant (rule 3), varied by host hash (rule 2), and presentation-only, so nothing here can reach a
checksum by construction.

### The contact sheet

A dev page rendering **every definition × every tier × every status** on one grid, committed as an
entry point. It reuses the renderer, so it costs little, and it is the only way to notice that two
buildings read alike or that a tier modifier changed nothing visible without playing the game and
happening to build both. The grammar is half of "maintained systematically"; this is the other half.

### Acceptance

A tier-1 definition must be distinguishable from its tier-0 parent **by silhouette, with colour
removed**, at normal zoom. A new definition must render as a distinct readable machine with no new
drawing code. And the grammar adds an indirection to a per-entity draw, so it ships with a
`npm run bench:browser` re-measure against the v0.13.1 record — the same rule Stage B shipped under.

## Longer horizon

Named 2026-08-18. Stage B's five rules — the 2D start of the organic item — shipped as Look
Systems, and Stage D is the next step in the same programme rather than a detour from it. 3D
presentation and the later tileable-texture systems are still the destination, not v0.15. Full
write-up is in `docs/HEXFACTORY-PLAN.md` under **Longer horizon**.

- **Organic tileables.** Stage B's five rules are the 2D start and have shipped. The later systems
  produce tileable textures and shapes so a hex lattice reads as organic terrain and organic
  objects, still generated from published snapshot facts, still never a checksum input.
- **3D presentation.** The camera tilts and orbits the player; the player, terrain, and buildings
  gain 3D shape. Canvas 2D stays replaceable presentation. A 3D mesh hand-authored per definition is
  the atlas again; a mesh derived from `recipe_category` and tier is this generator in another
  dimension. **Stage D is the cheapest available preparation for it**: a part list with anchors and
  scales is a description of a machine rather than a sequence of canvas calls, and that description
  is what a mesh generator would consume. The 2D walker is one consumer of the grammar, not the
  grammar itself. A renderer replacement is still a measured decision; v0.12.4 is the baseline it is
  measured against. Not this session.

## Still

`docs/art/world-shape-still.png` is the argument-piece mockup of a running factory on the
new bands.
