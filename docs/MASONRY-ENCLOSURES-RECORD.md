# Masonry Enclosures — v0.39.0

2026-08-28. The third Phase 3 delivery is the oil-free masonry branch: hill limestone, kiln-fired
cement, corrected concrete, and walls that stand on the same edge construction timber fences already
use. This release does not claim roofs, reinforced concrete, steel frames, oil, supported floors or
vertical transport.

## Player contract

The enclosure tool is the Fences tray rebuilt to the Ground works pattern. Four verbs sit in one
row: Place, Open, Close, Strip. R cycles them, Shift+R goes back, Delete jumps to Strip. C remains
Creative. Place shows a shelf of materials: timber fence, timber gate, timber wall,
wire fence, wire gate, brick wall, brick gate, concrete wall. Each card names what it is (fence,
wall or gate), what it costs against the pack, and — when Fired Masonry is still locked — which
research to buy. Selection is one edge or a two-corner enclosure of at most 32 hexes. Escape cancels
a selection, Escape again exits. Nothing is spent before Apply.

Timber and wire stay free of research, as the first fences did. Brick and concrete walls require
**Fired Masonry** (Material Processing and Hydrology, 8 insight). A locked card is still selectable
so the player can see the goal; Apply is refused until the node is bought.

A timber wall costs **4 timber**. A wire fence or gate costs **1 timber + 2 iron wire**. A brick
wall costs **3 brick + 1 cement**; a brick gate adds **1 iron wire** for the leaf. A concrete wall
costs **2 concrete**. Gates start open and need no power. Identical construction is free.

Cement is fired at the kiln: **2 limestone + 1 clay -> 2 cement**. Concrete is corrected to
**1 cement + 2 gravel + 1 sand + 1 water -> 2 concrete**. Mortar is not stocked; cement is the
binder billed at the wall. The Ground works concrete slab remains the prepared pad, sitting on the
grade already paid for rather than substituting for one.

Limestone is a hill quarry, not cliff scree. New worlds guarantee a workable patch 18–32 hexes from
the landing site. Old worlds keep the site rules they were generated with, so existing deposits do
not move; cement and the corrected mix need a new world, or limestone already in the pack.

## Accounting and compatibility

Each edge still stores its actual paid ingredients. Removal recovers that bill. Creative construction
cannot mint materials after leaving Creative. Replacements net the old and new bills. Undo is the
same bounded, unsaved enclosure history.

Save **31**, definitions **25**, technologies **13**, world generator **9**. Wire **17**, scenarios
**7** remain unchanged. Save 30 receives the new envelopes; stored `world_params.site_rules` are
left alone. In-progress Mix concrete jobs that reserved the old three-ingredient bill can be
cancelled from the machine.

The finite catalogue now pays **626** insight against **128** of purchasable research (4.891×). Two
new projects, Hill limestone and Bagged cement, and a repriced Concrete pour fund Fired Masonry
without touching the rest of the tree.

Three.js draws walls as solid slabs, wire as three thin rails, and timber fences as the two-rail
posts already shipped. Gate leaves still swing. Colour comes from the definition, not from the paid
bill.

## Measured material and work costs

`fixtures/balance.json` adds the new boundary projects and a nine-hex brick yard. Timber and wire
still assume only the primitive stations. Brick and concrete assume kiln, crusher, composer, pump
and smelter as well. Existing timber-fence and nine-hex-yard rows are unchanged.

## Verification and limits

Native regressions cover Fired Masonry as a purchase gate, cement billed on a brick wall, timber
walls remaining free of that node, limestone geography on hills, bootstrap reach on every preset,
and stamp-only migration of save 30. Host tests cover the envelope number, research emblems and
balance arithmetic. The Rust/TypeScript balance fixture includes limestone, cement and the brick
yard.

No screen-reader audit, timed opening validation, or large-perimeter benchmark is claimed. Roofs,
rebar, beams and a second floor remain later slices. Old worlds without a limestone site rule cannot
fire new cement.
