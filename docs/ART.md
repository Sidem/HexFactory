# HexFactory art direction

HexFactory uses generated low-poly forms and procedural materials, not a per-definition asset atlas.
Rendering consumes native snapshots and never owns simulation truth; hashes, animation, and decorative
variation are presentation only.

## Visual rules

1. **Readability before detail.** Impassability, selection, status, direction, and resource identity must
   remain legible at normal zoom and without relying on colour alone.
2. **Transitions use neighbours.** Shore, slope, and material boundaries respond to adjacent cells rather
   than drawing isolated hex tiles.
3. **Variation uses world position.** Host-side hashes and procedural material patterns do not restart per
   tile and never enter native state.
4. **Definitions generate appearance.** Building kind, recipe category, power source, tier, and status select
   reusable parts and modifiers. No definition owns a bespoke model or draw call.
5. **State is visible where it occurs.** Depletion, regrowth, progress, range, routes, and stall reasons are
   shown on the world object instead of only in text.
6. **One concept has one visual implementation.** Items use `itemChip`; terrain style, overlays, and machine
   parts each have one shared generator.

## Terrain and palette

Native physical height supplies the rendered surface: generated bed plus earthwork and erosion departures.
The renderer may interpolate it but never derives it. Surveyed lowland is the default fill; fog covers the
complement of native surveyed chunks.

| Ground        | Fill      | Edge      | Read                       |
| ------------- | --------- | --------- | -------------------------- |
| Deep water    | `#0f3550` | `#3f9ad0` | Impassable, pumpable basin |
| Shallow water | `#1a5474` | `#5cb6d8` | Slow ford, bridgeable      |
| Shore         | `#c4a56a` | `#e0c88a` | Sand and clay              |
| Lowland       | `#1a3a32` | —         | Default surveyed ground    |
| Hills         | `#48604d` | `#6f8a6c` | Copper and coal country    |
| Highland      | `#5c6b58` | `#8a9a84` | Iron and coal country      |
| Cliff         | `#57493e` | `#c19a72` | Impassable rock face       |
| Fog           | `#18242f` | `#7fe0c0` | Unsurveyed world           |

Impassable ground adds one shared hatch/rim treatment based on native access rules in
`fixtures/terrain-passability.json`. A worked cliff keeps its rock material but loses the impassable mark
when native says it is walkable.

`terrainSurface.ts` maps the ground to four procedural families—water, sand, meadow, and rock—inside a
bounded shared material set. Patterns are keyed only by world position. Quality profiles change bounded
detail; reduced motion freezes motion rather than slowing simulation. Terrain and resource remain separate
visual facts, so deposits never recolour or replace the ground under them.

## Shape grammar

`src/rendering/shapeGrammar.ts` defines eight machine parts: **vessel, chamber, stack, rotor, aperture,
mast, band, and mouth**. Each part has a normalized anchor, scale, rotation, material role, and optional
animation phase. One renderer maps them to reusable geometry.

Appearance is composed in this order:

1. `SilhouetteKey` selects a total base part list from kind, recipe category, and power source.
2. `TIER_LADDER` and `HUB_LADDER` apply cumulative shape modifiers.
3. Native status activates bounded phases such as spin, pulse, rise, and grind.
4. The renderer instances equal part/material combinations across entities.

The four material roles are powder-coated **structure**, fired **ceramic**, **brass**, and **dark** hardware.
Tier differences must change silhouette, not merely colour. A larger visual form never invents a larger
logical footprint.

Transport uses its own shared geometry vocabulary: narrow decks, rails, treads, portals, and link marks.
Junctions and headings remain identifiable when empty. Smoke and steam are pooled presentation of published
working state; no emitter owns a timer or particle system.

## Items and resources

Item glyphs describe material form while colour distinguishes siblings:

| Glyph                                                                               | Current forms                         |
| ----------------------------------------------------------------------------------- | ------------------------------------- |
| `ore`                                                                               | Iron and copper ore                   |
| `lump`                                                                              | Coal, stone, charcoal                 |
| `grains`                                                                            | Sand, clay, gravel                    |
| `log`                                                                               | Wood, timber                          |
| `droplet`                                                                           | Water, crude oil, and refined fuel    |
| `plate`                                                                             | Plates, glass, brick, concrete, steel |
| `kit`, `wire`, `gear`, `frame`, `circuit`, `crystal`, `component`, `barrel`, `pipe` | Manufactured forms                    |

`src/rendering/itemChip.ts` is the only item-markup path. Every chip includes a glyph; quantities are shown
as either `3` or `3 / 10`. Field geometry uses the same form language so the resource and the stack it
becomes are visibly related.

## Camera, overlays, and validation

The production Three.js view is a near-orthographic diorama with twelve orbit headings. Terrain prisms,
picking, grid, hover, selection, native previews, arrows, and reach overlays use the same pointy-top geometry.
Picking follows the drawn native surface; overlays do not guess at logical height.

Different meanings use different marks: area effects are filled with a rim, distances are rims only,
construction refusals name and mark the blocking cell, and progress/status marks stay attached to their
entity. Reduced motion preserves the final information without animated transitions.

`contact.html` renders every definition, tier, status, and orbit through the production geometry. Validate
new art there with colour disabled, then verify normal zoom in the game. The acceptance rule is simple: the
player can distinguish identity, tier, direction, and actionable state from silhouette and marks alone.

Phase 12 extends this vocabulary to organic seams and biome props. Props remain sparse instanced
presentation: they never occupy construction cells or enter saves and checksums. Underground geometry may
depict only native level and stratum state.

`docs/art/world-shape-still.png` is the current visual reference.
