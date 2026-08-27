# Essential station bills — v0.28.0

Recorded 2026-08-27. This is the third additive delivery in phase 1 of the progression and
construction workstream. It prices the five stations the opening actually touches and nothing else;
the industrial bills below the smelter are untouched and still unvalidated.

## What changed

The [construction and materials plan](CONSTRUCTION-MATERIALS-PLAN.md) proposed that the extractor,
composer, generator and pole stop being billed in raw ore and start being billed in manufactured
parts, with two named constraints: remove the signal crystal from ordinary assembly, and require no
copper expedition before the first power. Both hold now.

| Building         | v0.27.0           | v0.28.0                              |
| ---------------- | ----------------- | ------------------------------------ |
| Extractor        | 4 ore + 2 stone   | 2 iron plate + 1 gear + 2 timber     |
| Composer         | 5 ore + 3 crystal | 2 iron plate + 1 gear + 1 frame      |
| Container        | 3 ore             | 3 timber                             |
| Pole             | 1 ore             | 1 timber + 1 iron wire               |
| Burner generator | 4 ore + 4 stone   | 1 iron plate + 1 frame + 2 iron wire |

One item and one recipe are new. **Iron wire** is a 6-tick assembly of `1 iron plate -> 2 iron wire`,
stacking to 20. It runs at the composer and at the manual workshop, so the wire a pole and a
generator are wound with exists before either of them does — which is what keeps the first grid off
the copper the plan wanted it off. The manual workshop's recipe list grows to six.

Every other construction cost in the catalogue is unchanged. The smelter, kiln, cutter, crusher,
pump, boiler, turbines and the tier-two ladder still bill raw material, and this release makes no
claim that those prices are credible.

## Reproducible arithmetic

`npm run balance` writes `fixtures/balance.json`; Rust computes the fixture and
`tests/balance.test.ts` re-derives the same expansions independently, so each number below is
reached twice from different code.

Station cost through the whole tree, in raw units, fuel energy and the single `effort` scalar
(`raw + fuel / 160`, coal being the densest fuel the catalogue ships):

| Building         | v0.27.0 effort | v0.28.0 raw | fuel | v0.28.0 effort |
| ---------------- | -------------- | ----------- | ---- | -------------- |
| Extractor        | 6.000          | 9.000       | 320  | 11.000         |
| Composer         | 8.000          | 11.000      | 400  | 13.500         |
| Container        | 3.000          | 1.500       | 0    | 1.500          |
| Pole             | 1.000          | 1.500       | 40   | 1.750          |
| Burner generator | 8.000          | 7.000       | 240  | 8.500          |

The container and the generator got _cheaper_ in raw material — timber and wire are batch recipes,
so three boxes' worth of timber is less wood than three ore was ore. What they gained is fuel and
machine time, which is the trade the whole pass is making: material for process.

Openings, from a standing start, as the harness reports them:

| Opening         | v0.27.0 gathers | v0.28.0 gathers | v0.28.0 hand | player work | machine |
| --------------- | --------------- | --------------- | ------------ | ----------- | ------- |
| First power     | 17              | 36              | 40.667 s     | 136 ticks   | 80      |
| First smelter   | 40              | 47              | 43.833 s     | 88 ticks    | 30      |
| First circuit   | 76              | 88              | 88.000 s     | 48 ticks    | 172     |
| First extractor | —               | 52              | 64.667 s     | 144 ticks   | 140     |
| First composer  | —               | 58              | 57.000 s     | 48 ticks    | 194     |

The last two rows are new measurements, added because a station billed in parts is the first thing
in the game whose opening is worth stating separately. There is no v0.27.0 column for them.

**The opening got more expensive, and that is a real difficulty change, not a rounding artifact.**
First power more than doubles, 17 gathers to 36, because a pole is now wound with wire that a
furnace and a workshop have to exist to make: the shortest path to electricity runs through
`stone + clay -> primitive furnace`, `wood + stone -> manual workshop`, `ore -> iron plate ->
iron wire`. First power also stops being free of the clock — 136 ticks of attended pressing and 80
of machine time where v0.27.0 had none of either.

What the pass buys for that is the composer's bill. First circuit needed **3 signal crystal** in
v0.27.0 and needs **none** in v0.28.0: the gather list falls from
`13 ore + 3 crystal + 4 copper-ore + 18 stone + 2 sand + 4 clay` to
`20 ore + 4 copper-ore + 16 stone + 2 sand + 6 wood + 4 clay`. A player can now build their first
assembler out of things the landing clearing guarantees, instead of finding a crystal seam and
powering an extractor on it first. Crystal falls from two dependents to one — the tier-two
extractor — and stays on the hub's `crystal-array` request.

The founding contract moves once, at the foundry stage: 106 gathers to 112, from the kiln's opening
now including the workshop. The components stage is unchanged at 12.

Do not add the hand and player-work columns together and call the total a play time. These are
arithmetic subtotals: they exclude walking, finding fields, stock handling, blocked output and
operator delay.

## The curve still holds

`effort` has to rise along every unlock path, and the generator was the tight one: the burner lands
at 8.500 against the wind turbine's 10.000, so the first plant is still the cheapest plant. The
extractor at 11.000 stays under the deep extractor's 21.000 and the pole at 1.750 under `pole-ii`'s
7.500. All 20 curve steps assert `holds`, and both ladders still appear as strict `upgrade` steps.

The bootstrap floor is unchanged and is what makes the new bills reachable at all: the primitive
furnace (6 stone + 4 clay) and the manual workshop (2 stone + 4 wood) need no research and no
station, and native still refuses any catalogue in which a station requires its own output.

## Compatibility and regression evidence

Save 20 / definitions 18; technologies 8, scenarios 5, world 8 and wire 12 are unchanged. The
definition revision is a price change, not a state change, so the migration adds, removes and
reinterprets no saved field: stock, jobs, research, insight, entity identity and checksum survive
untouched. The envelope still has to move, because `Core::from_save` refuses any save whose
definition version differs from the running catalogue.

The boundary an existing factory crosses is what its already-placed stations hand back.
`erase_refund` quotes the _current_ bill, so an extractor bought for 4 ore and 2 stone refunds
2 iron plate, 1 gear and 2 timber — more raw value than it cost. That is a one-time revaluation and
provably not a loop: the refund is exactly the rebuild cost, so place-and-erase is a fixed point,
and no recipe converts plate, gear, timber or wire back into ore. Three native regressions pin it —
one asserts the five bills and that each station's erase returns exactly what its placement took,
one builds an extractor against a version-17 catalogue that prices it in ore, reads the save back
under version 18, and asserts the refund is the new bill and that erasing again returns the same
thing rather than more, and one asserts iron wire is drawable at the manual workshop before a
composer exists.

Around 30 existing native tests moved from stocking raw ore to a `stock_for` helper that reads the
bill out of the catalogue, so a future repricing changes one function instead of thirty call sites.

Player-facing guidance needed one fix that the bills exposed rather than caused: `cheapestFor` in
`src/core/guidance.ts` ranked buildings by counting the lines of their bill, which was the same
ordering as cost only while every bill was a pile of ore. A composer is four lines and eleven raw
units; a manual workshop is six lines and six. Guidance was sending a player with nothing built to
the dearer station. It now expands each bill through the recipe tree, the same quantity
`balance.rs` computes.

The local release gate passed `npm run quality`: 239 TypeScript tests, 177 Rust tests, formatting,
lint, type checking, agent-map validation, dependency audit and the production build.

## Still required in phase 1

Direct foundation commissions, expanded progression definitions and the timed standard opening
comparison remain. That last one is the item this record cannot close on its own: the numbers above
say the opening is 19 gathers longer and now has an attended-work component, and only a person
playing it can say whether trading a crystal expedition for a furnace and a workshop is the better
game.

Gear and frame yields are unexamined, and the industrial bills — smelter, kiln, cutter, crusher,
pump — are still raw material, so the second tier of stations has the same credibility problem this
pass fixed for the first. The scenario catalogue is deliberately unchanged at version 5, which
leaves `factory-demo`'s 12 signal crystal vestigial but harmless rather than invalidating existing
demo saves. No hub request buys iron wire; it exists only as an input.
