# Industrial bills — v0.32.0

Recorded 2026-08-28. This phase 1 delivery owns the five industrial station bills and the
construction-order correction to startup accounting. It does not complete the progression and
construction workstream.

## Bills and recovery

| Station | v0.31.0                | v0.32.0                         | Expanded effort, before → after |
| ------- | ---------------------- | ------------------------------- | ------------------------------- |
| Smelter | 8 stone + 4 ore        | 6 stone + 2 iron plate          | 12.000 → 11.000                 |
| Kiln    | 6 stone + 4 clay       | 6 stone + 2 clay + 1 iron plate | 10.000 → 10.500                 |
| Cutter  | 3 iron plate + 6 stone | 4 stone + 2 iron plate + 1 gear | 13.500 → 14.000                 |
| Crusher | 4 iron plate + 6 stone | 6 stone + 2 iron plate + 1 gear | 16.000 → 16.000                 |
| Pump    | 4 iron plate + 4 brick | 2 iron plate + 1 gear + 3 brick | 13.333 → 12.500                 |

Effort is tree-expanded raw units plus recipe fuel energy / 160, the existing native comparison.
All 20 curve relationships still hold. The cutter and crusher now pay for a mechanical drive;
the pump uses one complete three-brick batch rather than requiring a second batch for one brick.
The kiln uses unfired clay and primitive iron, never its own bricks. The furnace and workshop can
make the plates and gears before industrial processing exists. Their raw construction bills,
recipe identities, yields, work rates, fuel values and research prices are unchanged.

Save 24 advances definitions 18 to 19. Technologies 11, scenarios 6, world 8 and wire 13 stay put.
No state field changes. Five native legacy-factory cases preserve serialized state, checksum,
inventory, insight and active work, then compare resumed ticks with the old factory.

Browser testing also exposed a pre-existing picker mismatch: native migrates scenario 5 to 6
at save 23, but the picker required an exact scenario version. It now recognizes that specific
step throughout the supported save chain. Tests cover real scenario versions and continue to
reject unknown versions and a scenario-5 envelope claiming to be save 23 or newer.

Existing stations refund the current bill, following the essential-bills migration precedent.
This deliberately revalues already-placed stations once; it does not record historical purchase
prices. Rebuilding spends the refund and another demolition returns the same stock, including
previous machine contents. None of these parts has a reverse recipe to raw ore. Native tests pin
both exact fresh placement/refund and legacy rebuild/refund conservation for all five stations.

## Construction before production

The old balance resolver treated every machine in its final set as available for every ingredient.
It could use a smelter to make its own plates or the first generator's parts, and a composer to
make its own gear. This omitted primitive setup and understated attended work.

The report now publishes a deterministic `construction_order`. It builds suppliers first, excludes
machines currently awaiting their own construction, and supplies power before a powered station
enters the available set. Construction work uses only earlier stations; requested output uses the
completed set. This is one reproducible route, not an optimizer for all possible factory layouts.
The TypeScript fixture tests independently replay that order, reject a missing primitive supplier,
and rederive recipe fuel, raw expansion, machine ticks and attended ticks.

Guidance also checks missing construction suppliers before naming a generator or processing
station. Existing equipment and carried parts avoid redundant primitive-station advice. Its
station ordering now includes recipe fuel: raw-only sorting tied the new smelter with the primitive
furnace and could suggest industrial smelting before the first power.

## Like-for-like arithmetic

Both columns below use the corrected construction-order resolver; the before column uses the
v0.31 bills. `npm run balance` was run before editing costs, then the corrected resolver was run
against those unchanged bills before the repricing. New fixture rows cover kiln, cutter, crusher
and pump as well as the existing smelter row.

| Target        | Gathers, before → after | Recipe fuel energy, before → after | Machine ticks, before → after | Attended ticks, before → after |
| ------------- | ----------------------- | ---------------------------------- | ----------------------------- | ------------------------------ |
| First smelter | 45 → 44                 | 240 → 400                          | 60 → 100                      | 88 → 88                        |
| First kiln    | 43 → 43                 | 240 → 320                          | 60 → 80                       | 88 → 88                        |
| First cutter  | 54 → 55                 | 480 → 560                          | 120 → 140                     | 88 → 120                       |
| First crusher | 57 → 57                 | 560 → 560                          | 140 → 140                     | 88 → 120                       |
| First pump    | 70 → 68                 | 720 → 720                          | 164 → 172                     | 88 → 120                       |

First power remains 28 gathers, first extractor 36, first composer 46, and 24/100 starter belts
38/95. First circuit moves 74 → 73. The components commission remains 12; the foundry stage moves
100 → 101. At 10 factory ticks per second the extra gear work is 3.2 seconds of attended work,
not an estimate of elapsed playtime.

**Limits:** these rows price construction, recipe batches and a research-item floor. They exclude
walking, placement, stock handling, operator delay, grid operating fuel and the opening commission's
delivery bill from the individual station rows. Research still uses the old cheapest-request
arithmetic, not a replay of finite first payouts and decayed repeat rewards. Independent recipe
expansion rounds each demand to batches without reusing leftovers across bills. Do not label these
numbers a complete playable opening, minimum playtime, or a human pacing validation. The human
timed comparison remains withdrawn.

## Verification and next work

The local quality gate passed formatting, lint, types, dependency audit (zero vulnerabilities),
generated-map consistency, 251 TypeScript tests, 186 Rust tests, and a production Wasm/Vite build.
The existing Vite large-chunk warning remains. Browser checks confirmed all five catalogue bills,
the starting workshop guidance, save-24 reload, and enabled Load controls for supported legacy
saves. The rendered factory loaded without browser errors. Legacy running-job migration is
covered by native tests; the browser did not overwrite the older saved factories.
In an isolated QA factory with creative mode switched back off, smelter placement at (3, -1)
spent 6 stone and 2 plates (20/10 → 14/8); demolition restored 20/10. The unpowered smelter
correctly reported No power. This checks the compiled browser command/preview/refund path.

Phase 1 stays active. Next audit the component recipe and its founding contract together, then
remaining power/tier bills, gear/frame yields and full commission/research startup accounting.
Several of those existing bills already use manufactured parts: the old blanket description that
everything below the smelter was raw material was inaccurate. Phase 2's finite practical projects
and separate player skills remain next after that audit; ground works, oil and floors do not move
ahead of it. No performance or play-feel claim is made by this release.
