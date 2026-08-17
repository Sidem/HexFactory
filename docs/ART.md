# HexFactory art direction — Stage A

Stage A is the palette and shape language the World Shape renderer needs, extended by v0.12 to the
material roster. No sprite atlas yet; buildings stay geometric hexes. The roster is now stable, so
Stage B — the full item icon set and static building sprites as an atlas — is unblocked.

## Palette

Surveyed lowland is the default fill and is not sent as terrain. Everything else is a hex
cell with its own fill and edge.

| Band          | Fill      | Edge      | Role                                     |
| ------------- | --------- | --------- | ---------------------------------------- |
| Deep water    | `#0f3550` | `#1f5f86` | Impassable basin; pumped from the shore  |
| Shallow water | `#1a5474` | `#3d8aaa` | Impassable shore water                   |
| Shore         | `#c4a56a` | `#e0c88a` | Walkable, buildable; sand and clay       |
| Lowland       | `#1a3a32` | —         | Default surveyed ground; flora and clay  |
| Hills         | `#48604d` | `#6f8a6c` | Walkable, buildable; copper ore and coal |
| Highland      | `#5c6b58` | `#8a9a84` | Walkable, buildable; iron ore and coal   |
| Cliff         | `#4a4541` | `#7a736c` | Impassable landform edge; stone          |
| Fog           | `#18242f` | `#7fe0c0` | Unsurveyed world                         |

Hills sits between lowland and highland and is deliberately close to both: the bands read as one
rising landform, not as three unrelated colours. v0.12 added it because copper belongs to rolling
ground and iron to the tops, and a player who cannot see the difference cannot choose a site.

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

## Still

`docs/art/world-shape-still.png` is the argument-piece mockup of a running factory on the
new bands.
