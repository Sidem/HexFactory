# HexFactory art direction — Stage A

Stage A is the palette and shape language the World Shape renderer needs. No sprite atlas
yet; buildings stay geometric hexes until the material roster in v0.12 is stable.

## Palette

Surveyed lowland is the default fill and is not sent as terrain. Everything else is a hex
cell with its own fill and edge.

| Band          | Fill      | Edge      | Role                          |
| ------------- | --------- | --------- | ----------------------------- |
| Deep water    | `#0f3550` | `#1f5f86` | Impassable basin              |
| Shallow water | `#1a5474` | `#3d8aaa` | Impassable shore water        |
| Shore         | `#c4a56a` | `#e0c88a` | Walkable, buildable, sandy    |
| Lowland       | `#1a3a32` | —         | Default surveyed ground       |
| Highland      | `#5c6b58` | `#8a9a84` | Walkable, iron fields         |
| Cliff         | `#4a4541` | `#7a736c` | Impassable edge of a landform |
| Fog           | `#18242f` | `#7fe0c0` | Unsurveyed world              |

Items keep their identity colours: ore `#e2a85f`, crystal `#b78cff`, component `#6fddd0`.

## Shape language

- Buildings are pointy-top hex prisms. Identity is a three-letter stamp and a facing tick,
  not a pictorial silhouette.
- A sprite, when one exists, occupies the inner 60% of the hex so neighbours never clip it.
- Items are geometric glyphs: a faceted hex for ore, a standing crystal for crystal, a
  framed plate for a component. The same glyph is used in the pack and on the field.

## Still

`docs/art/world-shape-still.png` is the argument-piece mockup of a running factory on the
new bands.
