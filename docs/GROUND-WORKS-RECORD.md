# Ground Works — v0.38.0

2026-08-28. The second Phase 3 delivery is the ground itself: five paved surfaces and native integer
elevation with bounded cut and fill, shipped as one slice. Paving and levelling were paired from the
start of the plan because they answer the same question — how fast a hex walks, what a route costs,
which steps a walk or a building may cross — and splitting them would have meant writing that
arithmetic twice. This release does not claim masonry, roofs, supported floors, vertical transport,
or the remaining enclosure materials.

## Player contract

The Ground works tool opens a nonmodal tray beside the world, on the same rail as Fences. Five verbs
sit in one row: Pave, Strip, Raise, Lower, Level. R cycles them, Shift+R goes back, Delete jumps to
Strip, and G opens or closes the tray. Selection is one hex, a line, or a rectangle of at most 32
hexes; click once to start and again to finish. A precise-coordinate disclosure gives the four axial
numbers for anyone who does not want to hunt pixels. Escape cancels a selection, Escape again exits,
and focus returns where it came from. Nothing is spent, dug, or recovered before Apply.

Every number the tray shows is native's answer to the exact edit Apply would send: preview and commit
are one transaction, so the quote is the charge. The tray reports how many hexes would change, the
first refusal in plain language with its hex coordinates, the net bill and recovery, the cut and fill
step counts, the projected spoil heap, how many hexes would be left standing behind an unwalkable
step, and how many deposits would be sealed. Apply commits the whole selection or nothing.

Five surfaces are available with no new research and no new recipes. Compacted earth walks a tenth
faster and costs nothing, which makes it the way a player learns where their paths actually run.
Gravel yard, timber decking, brick pavers and concrete slab cost one unit each per hex and walk a
fifth, a quarter, a quarter and a third faster. Gravel and concrete already had recipes; brick,
timber and their feedstocks were already made in the manual workshop and primitive furnace. The
ladder is a real choice rather than one correct answer: the fastest surface is the most expensive,
and the cheapest is free.

Elevation is a signed integer delta from the band the generator already produced, bounded to three
steps of cut or fill per hex. Raise and Lower move every selected hex one step; Level evens the
selection onto the first hex's finished grade, cutting and filling to match. Fill is dug, never
conjured: cutting adds to a spoil heap and filling spends from it, and an edit that would need more
spoil than exists is refused with the exact shortfall and what to do about it. The heap is shown as
a gauge that reads `0 → 1 load` while a selection stands, so the player sees what the edit leaves
behind before committing it.

A step of more than two grades cannot be walked and cannot be built across. Buildings need ground no
steeper than a walk can climb, and a preview names the hexes an edit would strand behind a retaining
step so the player can cut beside them instead. Paving over a resource field seals it, so native
refuses that edit until the tray's explicit confirmation says the player saw the warning. Stripping
a surface restores the deposit and recovers exactly what was paid.

## Accounting and compatibility

Each prepared hex stores its actual paid ingredients, on the same rule as a boundary's bill: paving
done in Creative cannot mint materials after leaving Creative. Stripping recovers that bill. The pack
is checked after both spending and recovery, so freed slots can hold refunds, and a full pack or
insufficient materials leaves the whole edit unchanged.

The spoil ledger is `spoil + cut − fill`, evaluated as one number over the whole transaction; a
result below zero is a refusal, not a clamp. Undo reverses the entire last ground transaction and
restores the heap to its exact prior value. History is bounded to 64 edits and is not saved. Ground
undo has its own control; Ctrl+Z uses it while the Ground works tool is active, and ordinary building
undo is unchanged.

Save **30**, definitions **24**, wire **17**. Technologies **12**, scenarios **7**, world **8** remain
unchanged. Save 29 receives an empty ground overlay, a zero spoil heap and the new definition
envelope; the original checksum is still verified, and no state field moves. Existing recipes,
inventories, skills, research, boundaries and factories are unchanged. Both native loading and the
save picker retain the complete released migration chain.

Wire 17 adds two groups: the sparse ground overlay and the spoil scalar. The overlay is resent whole
only after an actual edit or a full load/reset; quiet frames neither scan nor transmit it. Elevation
is a signed varint beside an unsigned surface id, which is the one place in the format where reading
the wrong way round does not fail — it returns a vast positive surface — so the shared fixture
carries a paved hex and a cut hex in the same case and both sides decode it.

Natural elevation is chosen so that no two walkable generated terrains differ by more than two steps.
The generated world is therefore exactly as passable after this release as before it, and every
impassable step in a run is one somebody dug. Shallow water sits at ground level for the same reason:
a ford is a wet hex, not a canyon. Route search keeps an admissible heuristic because no surface may
walk faster than 150, so no step can cost less than the bound assumes.

Three.js draws prepared ground through shared instanced surface meshes; terrain height is now the
generator's visual height plus the paid grade, which is the first time that height carries simulation
meaning. Picking still uses the logical axial plane.

## Measured material and work costs

`fixtures/balance.json` adds five surface rows, each priced over the same nine-hex yard. Step cost is
the route cost of one hex against 100 for untreated ground; the saving column is per hundred hexes
walked. Whole-batch feedstock counts whole crafts, so batch leftovers are not free inputs. Fuel is
process energy, separate from feedstock. The primitive stations are assumed already built.

| Surface         | Movement | Step cost | Hexes saved per 100 | Direct bill, nine hexes | Whole-batch feedstock    | Fuel energy |
| --------------- | -------: | --------: | ------------------: | ----------------------- | ------------------------ | ----------: |
| Compacted earth |      110 |        90 |                10.0 | —                       | —                        |           0 |
| Gravel yard     |      120 |        83 |                17.0 | 9 gravel                | 5 stone                  |           0 |
| Timber decking  |      125 |        80 |                20.0 | 9 timber                | 5 wood                   |           0 |
| Brick pavers    |      125 |        80 |                20.0 | 9 brick                 | 6 clay                   |         240 |
| Concrete slab   |      130 |        76 |                24.0 | 9 concrete              | 5 stone, 5 sand, 5 water |           0 |

Rust computes these from the shipped catalogue; TypeScript independently recomputes the floored step
cost, the saving, and the per-hex bill times nine, and asserts the same figures. No new item, recipe
or research is introduced, so every prior balance row is unchanged apart from the definition envelope.

## Verification and limits

Native regressions cover preview/commit/refund/undo conserving paid materials, spoil conservation and
the refusal of fill that was never dug, a route preferring a longer paved way and a retaining wall
stopping it, sealing a deposit only on confirmation with stripping restoring it, a footprint needing
ground no steeper than a walk can climb, save round-trip/migration/validation with dirty deltas
matching the full oracle, and no generated terrain being walled off by its own natural elevation. The
Rust/TypeScript wire fixture carries a paved hex and a cut hex together with the spoil scalar. Host
tests cover the ground command encoding including the deliberate cover flag and out-of-range refusal,
and the surface balance arithmetic.

`npm run quality` passes: dependency audit, map/format checks, lint, typecheck, **269** TypeScript
tests, **219** Rust tests, release Wasm compilation and production Vite build. `npm run balance`
passes; the committed report retains all prior rows unchanged except for the definition envelope.

Browser checks drove the shipped module through the tray end to end: paving a hex and reading back
the committed overlay record, an unaffordable surface refused in plain language with the bill blanked,
a cut raising the heap to one load, a fill spending it back to zero, a second fill refused with the
exact ledger and the instruction to cut somewhere, and the spoil gauge holding its projection across
frames rather than flashing back to the current value.

No screen-reader audit, timed opening validation, and no large-selection or many-hex paving benchmark
is claimed. Grade is bounded to three steps and selections to 32 hexes; nothing here says what a
thousand paved hexes cost to render or to route across. Masonry, supported floors, vertical transport
and the remaining Phase 3 enclosure materials are still outstanding, and the full Phase 3 scale and
integration gates still apply to them.
