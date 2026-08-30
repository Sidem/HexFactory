# HexFactory art direction

The art is a **generator, not an atlas**. Six numbered rules define it; every one of them holds
today, and a new building, band, or tier costs a data row rather than a drawing.

Rendering consumes snapshots and never owns simulation truth, so none of this can reach a checksum
by construction. That invariant is what makes generated art free here rather than risky: a host-side
hash, a noise field, and a baked tile are all presentation, and presentation may vary however it
likes. The available input is also better than geometry — a hex knows its band, its neighbours'
bands, its richness, and how much has been taken from it, because native already publishes all of
it.

## The rules

1. **Transitions come from neighbours.** Where a hex's band differs from a neighbour's, draw a
   fringe toward the lower band. Shore becomes a shoreline and a cliff becomes the edge of a
   landform. This is what makes "terrain is the material map" true on screen rather than only in the
   generator.
2. **Variation comes from a hash.** Rotation, in-band value jitter, and scattered detail marks keyed
   off `hash(q, r)` in the host, so a band stops reading as one repeated tile. The hash is
   presentation-only and must never become an input to anything native.
3. **Tiles and still shapes are baked, not shipped.** Value noise, threshold, and edge darkening run
   once at startup into offscreen canvases behind a version constant — `TERRAIN_TILE_VERSION`,
   `BUILDING_SHAPE_VERSION` — rather than PNGs in the bundle. Changing a constant regenerates the
   set.
4. **A building's look is derived from its definition.** Silhouette from `kind`, `recipe_category`,
   and `power_source`; the facing tick that is already drawn. `silhouetteOf` in
   `src/rendering/buildingLook.ts` does this with no per-id case.
5. **Depletion is visible history.** `quantity` against `initial_quantity` desaturates and scars the
   ground, so a worked-out region is legible from across the map. Flora regrowth runs the same
   system in the other direction.
6. **A shape is a part list, and a tier is a modifier on it.** See Stage D below.

## Stage D — the shape grammar

The vocabulary lives in `src/rendering/shapeGrammar.ts`: **vessel, chamber, stack, rotor, aperture,
mast, band, mouth** — eight parts naming machine anatomy rather than geometry, each carrying anchor,
scale, rotation, and animation `phase`, in units of the hex size rather than pixels. One renderer
walks a declarative list of them; the list is data.

Composition is three lookups and no cases:

- `kind` / `recipe_category` / `power_source` selects the **base part list**. `BUILDING_SHAPES` in
  `src/rendering/buildingLook.ts` is that table, and it is **total over `SilhouetteKey`** — so a new
  silhouette is a compile error at its data row rather than a machine that silently draws nothing.
- `tier` applies **shape modifiers** from `TIER_LADDER`, a named documented set: add a stack, add a
  rotor blade, segment the vessel, add a plating band, widen the mouth. A tier changes the
  silhouette, not the stroke colour. `HUB_LADDER` is a second such list applied by completed
  contract stage, through the same `applyLadder`.
- Terrain and the player draw from the same vocabulary, so the world reads as one visual system
  rather than three that happen to share a palette. The walker takes whatever unit its caller works
  in — hex size for a building, player radius for the player.

`phase` (`spin`, `pulse`, `rise`, `grind`) is a property of a part, so a rotor turns because it is a
rotor rather than because a `switch` arm reached for `Math.cos`. That is also what makes the bake
safe to split: still parts are stamped from an offscreen canvas under rule 3, and only parts that
actually move are walked per entity per frame. The grammar's indirection is paid at startup, not at
60 Hz.

**Two constraints worth knowing before extending it.** `addStack` anchors off `profileTop`
deliberately — every other modifier needs a part of a particular kind to act on, so a step built
only from those could find no target and produce a tier the map cannot show; anchoring one modifier
to the profile makes every non-empty shape grow. And the ladder is cumulative and unbounded in
principle but has **two steps**, so a definition at tier 3 would wear the same shape as one at
tier 2. The roster ships nothing above tier 1; the day it does, the ladder needs a third row rather
than a wider `trimOf`.

**The contact sheet is `contact.html`**, a dev entry point beside `bench.html`: every definition ×
every tier × every status on one grid, drawn by the shipped renderer rather than by a second
illustration of it. It carries a **colour toggle**, because a silhouette judgement fails any test
that keeps the palette — a gold stroke over an identical body would pass. It names two failures on
the card itself: a definition whose silhouette has no base shape, and definitions that draw
identically to each other. Both fire today across the belt family — belt, splitter, merger, and
underpass are one `kind` and none of them uses the machine-part grammar. The shared transport mesh
supplies the raised rails and transverse treads; a corner heading then stretches that vocabulary
across a seam, and a junction is currently told apart by its link decks and its stamp rather than by
a silhouette of its own. Dev-only: like `bench.html` it must never become a dependency of the game,
the production build, or the CI gate.

**The acceptance standard is a measurement, not an eyeballing.** A tier-1 definition must be
distinguishable from its tier-0 parent by silhouette, with colour removed, at normal zoom. That was
verified by reading the contact sheet's cells back pixel by pixel inside a disc that **excludes the
hex body's own tier-coloured stroke** — a first attempt that did not exclude it reported the
shapeless belt changing by 32%. Isolating the silhouette is what makes the figure mean anything.

## Palette

Surveyed lowland is the default fill and is not sent as terrain. Everything else is a hex cell with
its own fill and edge.

| Band          | Fill      | Edge      | Role                                           |
| ------------- | --------- | --------- | ---------------------------------------------- |
| Deep water    | `#0f3550` | `#3f9ad0` | Impassable basin; pumped from the shore        |
| Shallow water | `#1a5474` | `#5cb6d8` | Walkable 1 m/s ford; bridgeable, not buildable |
| Shore         | `#c4a56a` | `#e0c88a` | Walkable, buildable; sand and clay             |
| Lowland       | `#1a3a32` | —         | Default surveyed ground; flora and clay        |
| Hills         | `#48604d` | `#6f8a6c` | Walkable, buildable; copper ore and coal       |
| Highland      | `#5c6b58` | `#8a9a84` | Walkable, buildable; iron ore and coal         |
| Cliff         | `#57493e` | `#c19a72` | Impassable landform edge; stone                |
| Fog           | `#18242f` | `#7fe0c0` | Unsurveyed world                               |

Hills sits between lowland and highland and is deliberately close to both: the bands read as one
rising landform, not as three unrelated colours. Copper belongs to rolling ground and iron to the
tops, and a player who cannot see the difference cannot choose a site.

**Impassability outranks the band.** Deep water and cliff each keep the fill above, but both carry
one shared treatment — a diagonal hatch and a bright rim — so a player reads
"cannot stand here" before they read which material it is. That is deliberately not a fourth colour:
cliff against highland was two greys a step apart and the only way to tell them apart was to walk
into one, and tuning those greys would have kept the answer in the palette when the question is a
category. Which bands are in that category is native's rule, pinned by
`fixtures/terrain-passability.json`; the renderer reads the table and never decides for itself which
grey means cliff.

The one thing that outranks the table is the grade the player has cut into a particular hex. A cliff
whose face has been quarried down keeps the cliff's brown — it is the same rock, and the diorama has
already shown it drop a step — and loses the hatch, because the hatch is the sentence "cannot stand
here" and that sentence has stopped being true. The flat view has no height to say it with, so the
hatch is the whole of what it has to get right.

## Item colours and glyphs

Items keep their identity colours, and the glyph set names material _forms_ rather than individual
items — iron and copper ore share the faceted-hex `ore` glyph and differ by colour, as do every
plate and every kind of grit. **Twelve glyphs carry twenty-three items**, which is the generator's
rule applied to items before it was applied to buildings.

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

An item is drawn one way, by `src/rendering/itemChip.ts`, and never by a second shape. Every variant
is a modifier class on one markup, every chip shows its glyph — colour alone is not an identity in a
catalogue holding three greys — and `3` and `3 / 10` are the only two spellings of a quantity, one
an amount and the other progress toward a known target.

## Shape language

- Buildings are pointy-top hex prisms. Identity is the generated silhouette first; the three-letter
  stamp is a label under the body, quieter than the anatomy it names.
- A sprite, when one exists, occupies the inner 60% of the hex so neighbours never clip it.
- The same glyph is used in the pack and on the field, so a field cell and the stack it becomes are
  visibly one material. In the Three.js world that vocabulary becomes silhouette: ore is a group
  of faceted shards, lumps are a boulder cluster, grains are low mounds, crystal is a set of tall
  spires, and wood is trunk plus canopy. Identity colour distinguishes siblings inside one form;
  colour never has to distinguish one form from another.
- State the player has to react to is drawn where it happens rather than written in the message
  strip: a machine's progress arc, the ring that closes around the player while a field action cools
  down, and the `STALL_MARKS` dot that says _why_ a machine is idle.
- A radius is drawn as a ring, and two rings that mean different things must not look the same. An
  area of effect is a filled disc with a bright rim; a distance to another building is a rim only.

## Visual Depth renderer

**Visual Depth v0.25** ships the generator as a stylized low-poly Three.js diorama. The
near-orthographic camera tilts and orbits in twelve 30-degree steps; terrain, buildings, cargo, fields,
trees, depletion, overlays, and the player have shape while native gameplay remains on the existing
axial plane. Visual terrain height is a total seven-band presentation lookup and never save,
checksum, wire, movement, or construction state. From v0.38.0 a hex is drawn at that band plus the
integer grade the player paid for; the paid grade is native state and the band is not, and the
renderer adds them rather than deriving either.

A 3D mesh hand-authored per definition would be the atlas again. `machineMeshes.ts` maps all eight
`ShapePart` kinds to a bounded reusable geometry vocabulary. `partsFor` applies the existing
`TIER_LADDER` and `HUB_LADDER`, and `worldInstances.ts` groups the resulting anatomy into instanced
part/material buckets. Belt and bridge geometry is likewise shared between the game and the contact
sheet, one mesh scaled per heading rather than one mesh per heading. No definition owns a model and
no building owns a draw call.

The grammar also names one of four bounded **material roles** — powder-coated `structure`, fired
`ceramic`, `brass`, or `dark` hardware. Three.js maps those roles to distinct roughness, metalness,
and object-space procedural grain, while Canvas may ignore them; no UV atlas or per-definition
texture enters the bundle. The Wayfinder uses the same surfaces in one generated faceted assembly:
separate dark legs and hull, ceramic arms, shoulder shell and survey helmet, a brass pack, tool and
beacon, and a bright forward visor. It is scaled as a person rather than an inventory token, turns
as one group from the native facing vector, unfolds its tool only while the published action
cooldown is live, and swings opposed limbs from the published walk path.

Scale is an authored hierarchy over the grammar. Belts remain narrow deck infrastructure, poles
remain slender and below the factory skyline, ordinary machines gain enough mass for their vessel,
bands and working head to read beside the Wayfinder, and the wind turbine is the dominant landmark.
Its rotor geometry is tilted into a vertical disc and spins about its own local shaft axis. Brass
bands are scaled to embrace the vessel they reinforce rather than disappearing inside it. These are
presentation multipliers only: they never invent occupied cells. Any larger logical footprint must
come from the versioned building definition and native placement/save contracts.

Smoke and steam are likewise presentation of published state, never a second simulation. One pooled
instanced plume mesh draws burner exhaust, hot composer smoke, boiler steam, and turbine exhaust only
while the corresponding native status says the machine is working. Ordinary motion advances three
reused low-poly puffs per emitter; reduced motion holds one fixed puff. No emitter owns a particle
system, timer, or draw call.

Terrain prisms use the exact public pointy-top axial radius. Adjacent centres therefore meet at one
apothem with no triangular holes. Grid, hover, selection, legality, native drag preview, arrows, and
reach rings use the same pointy-top start angle (`pi / 6`), so an overlay cannot present a hex rotated
away from the tile beneath it. Tests pin both the apothem and overlay orientation.

**A band is a material, not a fill.** `terrainSurface.ts` gives the seven bands four procedural
surface families — water, sand, meadow, rock — injected into the shared `MeshStandardMaterial` at
`onBeforeCompile` rather than shipped as textures, so the bounded material set and one-draw-call-per-band
instancing both survive. Water carries two crossing swells and a drifting fbm, foams on the crests,
bends its normal by the analytic wave slope, and drops roughness where it crests so it glints; sand
carries dunes, a wind ripple, and a quartz speckle that both brightens and polishes; meadow carries
clumping growth over fine blades; rock carries bedding, fracture shear, and mica. Each material
returns its own `customProgramCacheKey` — seven identical injection closures would otherwise share
one compiled program and collapse every band onto one palette. The band's identity colour in
`terrainStyle.ts` stays the legend's answer, and the surface straddles it rather than replacing it;
`tests/visualDepth.test.ts` pins the two together, and pins that the stock chunk anchors the
injection depends on still appear exactly once in the shipped three source.

**Every pattern is keyed on world position alone**, never on an axial coordinate — a pattern that
restarts per tile would re-draw the hex lattice the surfaces exist to soften. Terrain instance colour
is therefore a luminance jitter only: hue belongs to the shader now, and tinting the instance as well
would fight it. Detail follows the quality profile through `material.defines` (fbm octaves 2/3/4, and
the low profile drops the animated water terms entirely), and reduced motion holds the swell still
rather than slowing it — the same bargain every other phase in the scene makes.

Instance colour is part of the generator contract. Machine and field materials take their colour
from `InstancedMesh.instanceColor`; they do not also request a per-vertex colour attribute that the
shared geometry does not carry, because multiplying by that absent attribute collapses every
definition to black/grey. Tone mapping preserves the low-poly light while vivid kind colours and
field-specific chroma floors keep machines and resources legible against the dark landforms. A
resource never recolours or covers the terrain hex beneath it: terrain and resource remain two
independent visual facts. Belts use a coloured two-rail frame over contrasting transverse treads, so
an empty line still reads as transport before cargo arrives.

The contact sheet now renders 23 definitions, three ladder states, four status cells, and all six
orbits through one retained offscreen WebGL context using the same production geometry. Reduced
motion freezes every phase transform; colour can still be removed to judge silhouette alone.

## Longer horizon

- **Organic tileables.** The rules above are the 2D start. The procedural terrain surfaces are the
  first instalment: world-space material, no atlas, no per-tile restart. What remains is the same
  treatment for objects and for the seams between bands, so a hex lattice reads as organic terrain
  and organic objects — still generated from published snapshot facts, still never a checksum input.
- **Native elevation and underground strata.** They follow only if the shipped 3D renderer proves
  the camera, picking, readability, and laptop budget. Visual height alone does not change a save or
  checksum.

`docs/art/world-shape-still.png` is the argument-piece mockup of a running factory on the bands.
