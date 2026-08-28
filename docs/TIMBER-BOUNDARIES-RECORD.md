# Timber Boundaries — v0.37.0

2026-08-28. The first Phase 3 delivery is edge construction: timber fences and manual gates.
Paving and native integer levelling remain paired in the next ground-work delivery. This release
does not claim the full ground-work slice, masonry, roofs, floors or vertical transport.

## Player contract

The Fences tool opens a nonmodal tray beside the world. Select an edge, or choose two corner
hexes for an enclosure of at most 32 hexes. Native resolves the perimeter; the host sends only
the endpoints, selection mode, material and action. Apply commits the whole selection or nothing.
The preview shows affected edges, the first refusal with its edge location, and the net bill and
recovery. Existing identical construction is free and unchanged. Precise placement exposes hex
coordinates and all six sides without requiring pixel accuracy. Controls stay mounted while
snapshots and asynchronous previews arrive; superseded previews cannot enable Apply.

A timber fence costs **2 timber**. A timber gate costs **2 timber + 1 iron wire**, and starts open.
Replacing a paid fence with a gate therefore costs one wire. Both materials are already made in
the manual workshop; wire's plate comes from the primitive furnace. No new research or income
is required. Open/closed state is explicit, and gates need no power. This is primitive woodwork,
not a new compulsory research tax on the founding commission.

Fences occupy canonical hex edges, leaving both cell centres available. They block walking and
transport across the edge, including corner headings and underpass spans. There are no implicit
transport ports. Building or closing a boundary across an existing compiled connection is refused
with instructions to reroute it; later connections cannot compile through a closed boundary.
Gates can be opened, removed, or replaced through the same tool. Buildings cannot span an edge
boundary, including an open gate, and rotation cannot introduce that overlap.

Boundaries require dry buildable ground on both sides. They do not clear, cover, harvest or move
resources. The player cannot build or close an edge through their collision radius. Manual movement
uses integer segment/capsule checks; click routes use the same closed edges and replan after edits.
Animal exclusion, combat, thermal effects, native terrain height and structural support are absent.

## Accounting and compatibility

Each edge stores its actual paid ingredients. Removal recovers that bill; a boundary built free
in Creative cannot mint materials after leaving Creative. Replacements net the old and new bills.
The final pack is checked after both spending and recovery, so freed slots can hold refunds.
A full pack or insufficient materials leaves the whole edit unchanged.

Undo reverses the entire last boundary transaction, including removal and gate changes, after
rechecking reach, protected crossings and affordability. A refused undo keeps its history entry.
Boundary undo has a separate, explicit control; Ctrl+Z uses it while the Fences tool is active.
History is bounded to 64 edits and is not saved. Ordinary building undo remains unchanged.

Save **29**, definitions **23**, wire **16**. Technologies **12**, scenarios **7**, world **8** remain
unchanged. Save 28 receives an empty boundary set and the new definition envelope; the original
checksum is still verified. Existing recipes, inventories, skills, research and factories are
unchanged. Both native loading and the save picker retain the complete released migration chain.

Wire 16 carries canonical coordinates, definition identity, gate state and paid bills. The group
is resent only after an actual edit (or a full load/reset). Quiet frames do not scan or transmit
the boundary collection. The checksum memoizes a pure digest of source records, invalidated by
every edit and rebuilt on load; tests pin it to the uncached digest. Cache contents and undo
history are never serialized or treated as authoritative state. Edit-time group replacement still
costs the size of the boundary collection; no large-perimeter capacity claim is made.

Three.js uses shared instanced posts, rails and gate braces. Gate identity comes from definitions,
not its paid bill. Open gates swing visibly and use a distinct tint; closed gates keep their brace.
Selection strips mark native-resolved edges. Terrain height remains presentation only.

## Measured material and work costs

`fixtures/balance.json` adds three boundary projects. The primitive stations are assumed already
built; these are material production counts, **not** full startup or elapsed player times.
Fuel is listed as process energy, separate from feedstock. Batch leftovers are not free inputs.

| Project                               | Direct bill            | Whole-batch feedstock | Fuel energy | Furnace ticks | Attended workshop ticks |
| ------------------------------------- | ---------------------- | --------------------- | ----------: | ------------: | ----------------------: |
| One fence edge                        | 2 timber               | 1 wood                |           0 |             0 |                      24 |
| One gate                              | 2 timber, 1 iron wire  | 1 wood, 2 iron ore    |          80 |            20 |                      48 |
| Nine-hex yard, 21 fences and one gate | 44 timber, 1 iron wire | 22 wood, 2 iron ore   |          80 |            20 |                     552 |

The yard uses the perimeter of a 3×3 axial selection: 22 edges. Rust pins the native perimeter;
TypeScript independently checks its combined bill and primitive work counts. Existing balance
rows and the finite research/skill budget remain unchanged, apart from the definition version.

## Verification and limits

Native regressions cover canonical identity from both sides, all six edges and corner crossings,
bounded and atomic selection, preview/commit agreement, player collision, route replanning,
transport and cargo preservation, multicell placement/rotation, paid refunds, Creative provenance,
failed undo, save migration, malformed boundary records, source-digest caching and dirty/full
snapshot equivalence. The Rust/TypeScript wire fixture includes boundary records and empty removal.
Host tests cover endpoint encoding, picking, migration eligibility, balance and mesh lifecycle.

`npm run quality` passes: dependency audit, map/format checks, lint, typecheck, 264 TypeScript tests,
212 Rust tests, release Wasm compilation and production Vite build. `npm run balance` passes;
the committed report retains all prior rows unchanged except for the definition envelope.

Browser checks used the production build at 1280×720 and a 390×844 viewport. They exercised
single-edge construction, gate replacement/open/close and undo, two-corner enclosure preview,
whole-enclosure undo, missing-material refusal, a paid two-timber fence and its exact refund,
precise coordinates, the compact tray and Escape cancellation/focus return. These were fresh
throwaway factories; existing player saves were not overwritten.

No screen-reader audit, timed opening validation, or large-perimeter benchmark is claimed. The
full Phase 3 scale and integration gates still apply to subsequent ground works and enclosures.
