# Batched transport kits — v0.27.0

Recorded 2026-08-27. This is the second additive delivery in phase 1 of the progression and
construction workstream. It prices transport and nothing else; the remaining essential bills are
untouched and still unvalidated.

## What changed

The [construction and materials plan](CONSTRUCTION-MATERIALS-PLAN.md) proposed
`1 iron plate + 1 timber -> 4 starter transport kits`, one kit per ordinary belt and two for a
corner heading, as a hypothesis to be measured. That hypothesis shipped unchanged.

`Assemble transport kits` is an assembly recipe of 8 ticks. It runs at the composer and at the
manual workshop, so the first kits need no research, no grid and no station the opening does not
already reach. The kit stacks to 20.

Construction bills that moved, and only these:

| Building  | v0.26.0                  | v0.27.0                         | corner        |
| --------- | ------------------------ | ------------------------------- | ------------- |
| Belt      | 1 ore                    | 1 transport kit                 | 2 kits        |
| Splitter  | 2 ore + 1 gear           | 2 kits + 1 gear                 | 3 kits + gear |
| Merger    | 2 ore + 1 gear           | 2 kits + 1 gear                 | 3 kits + gear |
| Underpass | 2 ore + 1 gear + 2 stone | 2 kits + 2 iron plate + 2 stone | 3 kits + same |

The underpass's gear becomes structural metal, which is what the plan's proposed-bills table asked
for. Every other construction cost in the catalogue is unchanged: the extractor, composer,
generator and pole still bill raw ore, and this release makes no claim that those prices are
credible.

## Reproducible arithmetic

`npm run balance` and `fixtures/balance.json` record the catalogue; Rust computes the fixture and
`tests/balance.test.ts` checks the same rates independently. The starter-belt openings the v0.26.0
record left as unchanged baselines are now the measurement.

| Starter line                | v0.26.0 (definitions 16) | v0.27.0 (definitions 17)         |
| --------------------------- | ------------------------ | -------------------------------- |
| 24 segments — gathers       | 24 ore                   | 14 ore, 8 stone, 4 clay, 8 wood  |
| 24 segments — gather total  | 32                       | 46                               |
| 24 segments — hand time     | 48.000 s                 | 53.667 s                         |
| 24 segments — player work   | 0                        | 320 ticks / 32.0 s               |
| 24 segments — machine work  | 0                        | 140 ticks / 14.0 s               |
| 24 segments — fuel          | 0                        | 4 items / 560 energy             |
| 100 segments — gathers      | 100 ore                  | 52 ore, 8 stone, 4 clay, 18 wood |
| 100 segments — gather total | 108                      | 103                              |
| 100 segments — hand time    | 162.000 s                | 129.167 s                        |
| 100 segments — player work  | 0                        | 1168 ticks / 116.8 s             |
| 100 segments — machine work | 0                        | 520 ticks / 52.0 s               |
| 100 segments — fuel         | 0                        | 13 items / 2080 energy           |

Both v0.27.0 rows include building the manual workshop and the primitive furnace, because the
report now selects producers for the whole chain rather than assuming the belt is free of it.
Gather totals include the opening insight request and the fuel gathered to run the furnace.

The shape is the one the batch was meant to produce. Raw material per belt falls from 1.000 to
0.625 units — 0.500 ore and 0.125 wood — so a long line is materially cheaper: a hundred segments
drop from 108 gathers and 162.0 s of hand work to 103 and 129.2 s. A short line gets _more_
expensive, 32 gathers to 46, because the first run now pays for the two stations standing behind
it. That crossover is the intent: transport is no longer something the ground pays for.

The honest cost is player time when the assembly is not automated. The v0.27.0 columns above quote
attended work at the manual workshop; a hundred segments add 116.8 s of pressing on top of the
gathering, so hand-assembled belting is slower overall than hand-gathered belting was. At the
composer the same recipe runs at 300,000 milli-units per minute against the workshop's 75,000, so
the answer to a long line is a machine. Do not add the hand columns together and call the result a
speedup; these are arithmetic subtotals and exclude walking, finding fields, stock handling,
blocked output and operator delay.

Junction prices moved down in raw units as a side effect, because two kits are cheaper in ore than
two ore: the splitter and merger fall from 6.000 to 5.250 raw units and the underpass from 8.000 to
7.250. They gain the kit's fuel and machine time in exchange.

## Compatibility and regression evidence

Save 19 / definitions 17; technologies 8, scenarios 5, world 8 and wire 12 are unchanged. The
definition revision is a price change, not a state change, so the migration adds, removes and
reinterprets no saved field: stock, jobs, research, insight, entity identity and checksum survive
untouched. The envelope still has to move, because `Core::from_save` refuses any save whose
definition version differs from the running catalogue.

The one thing that changes for an existing factory is what its already-placed belts hand back.
`erase_refund` quotes the _current_ bill, so a legacy belt refunds one transport kit rather than the
ore that bought it. That is deliberate and conserving: the refund is exactly what rebuilding the
segment costs, so dismantling and relaying a legacy line is still free, and no recipe turns a kit
back into ore, so the boundary cannot be farmed for raw material. The native regression builds a
factory against a catalogue that prices a belt in ore, reads it back under the revised catalogue,
and asserts that erasing yields one kit, no ore, and enough to place the belt again.

Native regressions also cover the kit recipe's batch and its two permitted stations, kit billing
across the whole belt family with the corner heading strictly dearer than the edge one, placement
and drag affordability, drag previews stopping where the kits run out, rotation onto and off a
vertex heading charging and returning the difference, erase and undo refunds, and the migration
step itself — including a version-15 file walking every definition step to 19. TypeScript checks
the workshop's capability list and the hotbar's derived cost label.

The local release gate passed `npm run quality`: 239 TypeScript tests, 173 Rust tests, formatting,
lint, type checking, agent-map validation, dependency audit and the production build.

## Still required in phase 1

Credible essential bills for the extractor, composer, generator and pole, direct foundation
commissions, expanded progression definitions and a timed standard opening comparison remain next.
The scenario catalogue is deliberately unchanged at version 5: `factory-demo` still opens with 20
iron plate, 30 stone and a wood deposit within reach, so its player can reach kits without
invalidating existing demo saves. The measured crossover above is arithmetic, not a playtest;
whether a twenty-four segment line _feels_ worth two stations is still a human question.
