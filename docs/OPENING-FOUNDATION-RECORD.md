# Primitive opening foundation — v0.26.0

Recorded 2026-08-27. This is the first additive delivery in phase 1 of the progression and
construction workstream. It does not complete the opening redesign or validate its final pacing.

## What changed

The Primitive Furnace costs 6 stone and 4 clay. It runs the existing iron/copper plate recipes at
twice their industrial duration, consuming their ordinary fuel charge without electricity.
The Manual Workshop costs 4 wood and 2 stone. It runs the existing component, timber, gear and
frame recipes at four times their industrial duration. Neither station needs research or its own
output to construct. Both use the existing compartment stock, recipe reservation and erase refund.

Manual work is one explicitly started batch, with native progress. The player must stand within
one hex, with no walking intent/goal or gather cooldown. Starting another workshop pauses the
first; walking/gathering pauses work; completion switches it off. A paused job resumes in place,
including after save/load. Dismantling cancels it and refunds reserved inputs, subject to normal
pack capacity. Recipe changes remain refused mid-craft. This release does not add a job queue or
a separate cancellation/refund command.

Existing construction bills, recipe quantities, research prices, rewards and industrial work rates
are unchanged. Primitive capability lists do not expose steel, glass or electronics. The raw
opening request board remains unchanged; primitive processed orders become eligible once the
relevant stations exist. This is not the planned finite insight economy.

## Reproducible arithmetic

`npm run balance` and `fixtures/balance.json` record the catalogue. Rust computes the fixture;
`tests/balance.test.ts` independently checks recipe capability, rates and attended/unattended work.
The opening resolver now selects a producer for each recipe, rather than assuming a restricted
station covers an entire category. It includes setup bills and required research, and separates
player work from machine work. It also records the unchanged 24- and 100-segment starter-belt
bills as baselines for the next recipe pass; these are not transport-kit measurements.

| First component contract               | v0.25.3 catalogue           | v0.26.0 primitive route |
| -------------------------------------- | --------------------------- | ----------------------- |
| Stations selected by report            | Composer + burner generator | Manual workshop         |
| Research cost                          | 20 insight                  | 0 insight               |
| Setup + goods before research delivery | 15 ore, 3 crystal, 4 stone  | 6 ore, 2 stone, 4 wood  |
| Reported hand-time subtotal            | 41.333 s                    | 13 s                    |
| Recipe machine work                    | 24 ticks / 2.4 s            | 0                       |
| Attended recipe work                   | 0                           | 96 ticks / 9.6 s        |

These are arithmetic subtotals, **not measured opening completion times**. The old report even
counts crystal in its raw-unit/gather estimate although crystal needs powered extraction, so it
does not price that expedition honestly. Neither subtotal includes walking, finding fields,
handling stock, thinking, board rotation, blocked output or operator delays. Existing powered
opening estimates also exclude the generator's operating fuel and brownout scheduling. Do not
add these columns and claim a before/after speedup.

For the new primitive routes the report records:

- First iron plate: 6 stone + 4 clay setup, 2 ore, 80 fuel energy, 20 furnace ticks. Fuel is quoted
  in the report's standard coal unit (one coal, with charge left over); the native regression also
  proves the same craft with two wood and no coal or electrical network.
- First frame: both stations; 2 ore, 8 stone, 4 clay, 5 wood before fuel; 80 fuel energy;
  20 furnace ticks and 64 player-work ticks. Timber and frame use their existing recipe identities.

The existing opening timer now records optional workshop, timber and plate milestones. Crystal
and industrial assembly remain visible as optional alternative-route milestones; a manual first
contract can complete the opening record without them. Splits follow recorded times, not an
assumed technology order. Creative/load taints remain explicit.

## Compatibility and regression evidence

Save 18 / definitions 16; technologies 8, scenarios 5, world 8 and wire 12 are unchanged.
Migration preserves existing stock, jobs, research, insight and checksum. No new materials are
granted and no old buildings acquire more expensive refunds. Existing hotbar preferences survive.

Native regressions cover local-fuel operation, repeated dismantle/rebuild, explicit recipe refusal,
definition validation, no unattended production, single-batch completion, movement/gather/range
refusal, exclusive attendance, full-output rejection without mutation, reserved-input refunds,
save/resume equivalence and dirty-delta/full-snapshot parity. A legacy definition-15 factory is
resumed against definition 16 and compared with its uninterrupted run. TypeScript checks shared
capability presentation, guidance, save eligibility and both opening checkpoint routes.

The local release gate passed `npm run quality`: 239 TypeScript tests, 170 Rust tests,
formatting, lint, type checking, agent-map validation, dependency audit and the production build.
The existing Vite chunk-size advisory remains; this release makes no new performance claim.

Browser smoke testing used a separate creative save at 1280 × 800, not a timed opening run.
The workshop refused distant work, consumed one wood to make two timber on an explicit press,
and kept that output unchanged while idle with more ingredients available. Its selector showed
only the four permitted recipes; the furnace showed only iron and copper plate. The furnace
smelted with wood and no electrical network. This test caught and corrected an empty furnace's
misleading "no power" status; native regression now pins its fuel, waiting and composing states.
Existing browser saves remained listed and were not overwritten by the validation save.
Reloading the final build restored the validation workshop's two timber and furnace stock;
the fuel-starved furnace correctly reported "out of fuel" without requiring a generator.

## Still required in phase 1

Credible essential bills, batched transport kits, direct foundation commissions, expanded
progression definitions and a timed standard opening comparison remain next. Do not cut repeat
income before replacement projects and guidance exist. Primitive work rates and setup costs are
initial tuning, not a claim that new-player pacing has been validated. Physical laptop evidence,
human opening comparisons and later construction acceptance gates remain outstanding.
