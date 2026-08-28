# Power and tier bills — v0.34.0

Recorded 2026-08-28. This phase 1 delivery closes the bill audit: the last three station bills that
were still wrong, the gear and frame yields, and the two startup-accounting defects that were making
every opening look cheaper than it is. It completes the phase 1 workstream opened in v0.31.0. It
does not complete progression and construction, which continues into phase 2.

## Bills

| Station         | v0.33.0                              | v0.34.0                                               | Expanded effort, before → after |
| --------------- | ------------------------------------ | ----------------------------------------------------- | ------------------------------- |
| Hydro generator | 6 iron plate + 4 brick               | 4 iron plate + 1 gear + 1 frame                       | 18.333 → 18.500                 |
| Deep extractor  | 4 ore + 1 crystal + 6 stone + 2 gear | 2 iron plate + 2 gear + 1 frame + 1 crystal + 2 stone | 21.000 → 21.500                 |
| Deep container  | 3 ore + 4 stone + 2 iron plate       | 3 iron plate + 5 timber + 2 stone                     | 12.000 → 12.000                 |

Effort is tree-expanded raw units plus recipe fuel energy / 160, the same comparator every previous
bill pass used. All 20 curve relationships still hold: the deep extractor follows the shallow one at
21.500 against 11.000, the deep container follows its own tier at 12.000 against 1.500, and the
hydro generator follows the burner at 18.500 against 8.500.

Three separate defects were being fixed, not one:

- **Raw ore in a tier bill.** The deep extractor and the deep container were the last two buildings
  in the catalogue that asked for ore straight out of the ground. A tier upgrade billed in raw
  material is a tier that can be bought without running the factory that the tier below built. A
  native test now walks every buildable definition and fails if any construction cost names ore at
  all, so the property is checked rather than the three rows.
- **A shared bill.** The hydro generator and the boiler are unlocked in the same technology and
  quoted the same 6 plate + 4 brick, so choosing between them was a coin flip rather than a
  decision. The river wheel is now rotor, gearing and bracing — nothing fired, nothing laid in
  brick — and the boiler keeps the kiln work. A test asserts the two bills differ.
- **The deep container skipped its own tier.** It now costs the shallow container's timber and the
  plate a factory is already making, which is the same relationship every other tier has.

The deep extractor is the first station in the game to want both a gear and a frame, which is what
its position in the tree is supposed to mean.

Definitions 20 advances to 21 and save 25 to 26. Technologies 11, scenarios 7, world 8 and wire 13
stay put. No state field changes; the migration only relabels the two envelope numbers. As at every
price boundary since the transport kits, a station bought under the old bill refunds the new one
when erased. That is a one-time revaluation and provably not a loop — the refund equals the rebuild
cost, so a second demolition returns the player to where the first one left them, and none of these
parts has a recipe back to raw ore. Native tests pin exact placement and refund for all three.

## Gear and frame yields: reviewed, unchanged

The roadmap asked for the gear and frame yields to be audited. They were, and the finding is that
they are correct as they stand. Recording the evidence rather than the absence of a change:

| Item  | Recipe             | Effort | Board price | Per unit vs. a plate  |
| ----- | ------------------ | ------ | ----------- | --------------------- |
| Plate | 2 ore, 80 fuel     | 2.500  | 26 for 8    | 1.00× cost, 1.00× pay |
| Gear  | 2 plate            | 5.000  | 47 for 6    | 2.00× cost, 2.41× pay |
| Frame | 2 timber + 1 plate | 3.500  | 32 for 5    | 1.40× cost, 1.97× pay |

Both processed rows already pay a premium over what they cost to make, which is the property the
board is supposed to have and which a native test gates for every processed row. Halving the gear
recipe to one plate would have made a gear cost exactly what a plate costs while the hub kept paying
2.41× for it — a request-farming loop with no machine time worth speaking of between the two rows.
It would also have contradicted v0.33.0, which shipped a founding commission whose brief narrates
the two-plates-to-a-gear arithmetic. The yields stay.

## Startup accounting

Two defects in the balance resolver, both of which made the opening look shorter than it is. Both
were fixed before any bill was touched, so the arithmetic below is like-for-like.

**Research was funded at a price the hub withdraws.** Every raw request pays 10 insight the first
time it is filled and 2 for ever after. The resolver divided the technology bill by the first-fill
reward, which prices research against an unlimited supply of a reward paid exactly once. Funding is
now one fill at the first price and the remainder at the repeat price, and a row whose repeat reward
is zero funds nothing beyond its first fill rather than dividing by it. Processed requests have no
decay and are unaffected.

**A granted technology is not a free technology.** Four technologies cannot be bought at any price;
the founding commission hands them over for finishing a stage. Pricing them at their insight cost of
zero told the harness the early stations were unlocked from a standing start, which skipped the
delivery every real opening makes before placing one. The resolver now folds an owed stage's bill
into the opening it blocks, iterating to a fixed point because a folded stage can itself need
buildings that owe another. A stage does not commission itself, or nothing would resolve.

| Target                 | Gathers, before → corrected | Of which research |
| ---------------------- | --------------------------- | ----------------- |
| First smelter          | 44 → 51                     | 8 → 8             |
| First kiln             | 43 → 51                     | 8 → 8             |
| First cutter           | 55 → 86                     | 16 → 40           |
| First crusher          | 57 → 88                     | 16 → 40           |
| First pump             | 68 → 103                    | 20 → 48           |
| First power            | 28 → 36                     | 0 → 0             |
| First extractor        | 36 → 43                     | 0 → 0             |
| First composer         | 46 → 54                     | 8 → 8             |
| First circuit          | 73 → 95                     | 16 → 30           |
| 24 starter belts       | 38 → 45                     | 0 → 0             |
| 100 starter belts      | 95 → 103                    | 0 → 0             |
| new-game/foundry stage | 101 → 108                   | 8 → 8             |

First primitive plate (13) and first manual frame (20) are unchanged, which is the control: neither
needs a technology, so neither correction can touch them. The `new-game/components` stage is
unchanged at 24 for the same reason and because it does not commission itself. The repricing itself
moved none of these rows — the three repriced buildings appear in no opening, being later-tier — so
the corrected column above is also the shipped column.

**Limits:** these rows price construction, recipe batches and a research-item floor. They exclude
walking, placement, stock handling, operator delay and grid operating fuel. Folding a commission in
counts that stage's bill once per opening; it does not model a player who filled the stage earlier
for other reasons, and openings are priced independently rather than as one shared run, so the same
commission is counted in each row that owes it. Independent recipe expansion rounds each demand to
batches without reusing leftovers across bills. The research floor assumes the cheapest standing
request the landing clearing can supply and one uninterrupted funding route. Do not label these
numbers a complete playable opening, a minimum playtime, or a human pacing validation. The human
timed comparison remains withdrawn.

## Verification and next work

The local quality gate passed dependency audit, generated-map consistency, formatting, lint, types,
254 TypeScript tests and 193 Rust tests, and a production Wasm/Vite build. The existing Vite
large-chunk warning remains. New coverage: the three bills and a no-raw-ore-anywhere sweep, exact
placement and refund for all three stations, the funding decay and the commission fold in both
languages, and two migration cases pinning that save 25 → 26 moves only the definition envelope and
leaves an unexpected one alone.

Phase 1 is complete. The bill audit that began with the essential stations in v0.31.0 and continued
through the industrial stations in v0.32.0 and the mechanical components in v0.33.0 now covers every
buildable definition, and the startup accounting the earlier records deferred is done. Phase 2's
finite practical projects and separate player skills are next; ground works, oil and floors do not
move ahead of them. No performance or play-feel claim is made by this release.
