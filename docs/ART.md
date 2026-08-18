# HexFactory art direction — Stage A

Stage A is the palette and shape language the World Shape renderer needs, extended by v0.12 to the
material roster. Buildings stay geometric hexes. The roster is now stable, so Stage B is unblocked —
but Stage B is **a generator that emits the art, not an atlas somebody drew**. The rule and what it
buys are in "Stage B is a generator" below.

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
plate and every kind of grit. Twelve glyphs carry twenty-three items, and Stage B's sprite atlas
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
That is N drawings, and it arrives immediately before v0.14 Upgrades and Tiers — the milestone whose
whole job is multiplying the building roster. An atlas makes a tier cost a drawing; a generator makes
a tier cost a data row. The item glyphs already follow the generator's logic — twelve glyphs carry
twenty-three items — so Stage B extends that rule to buildings rather than abandoning it.

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

Rules 1, 2, and 5 add per-hex renderer work, and the renderer is the half of the frame
`docs/BENCHMARKS.md` has never measured. Stage C is gated on that measurement and Stage B is not, so
this work is legal before it. It should still not run far ahead of it, for the same reason the binary
delta encoding should precede the milestones that grow the payload: tuning the cost of a terrain
fringe with no measurement of the frame it lands in is guessing. Measure the renderer first, or
alongside.

## Still

`docs/art/world-shape-still.png` is the argument-piece mockup of a running factory on the
new bands.
