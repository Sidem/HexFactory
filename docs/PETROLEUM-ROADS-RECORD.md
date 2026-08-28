# Petroleum Roads — v0.40.0

2026-08-28. Phase 4 delivers the optional oil-to-road branch and its shared recipe infrastructure.
It does not deliver pipes, pressure, physical fluids, curing timers, supported floors or lifts.

## Player contract

Survey a lowland oil field, power an oil well, and send the crude units to a refinery. These are
explicit belt-carried item units, like the game's water, not disposable or magically returning
barrels. The well is made from existing plate, gears and frames: no oil is required to obtain oil.

The powered refinery makes **2 bitumen + 2 refined fuel from 4 crude oil in 30 ticks**. Both products
share one outlet and a 24-unit output compartment. All four output units must fit before another
batch reserves ingredients. A blocked refinery preserves everything; storage or compatible
consumers must handle both streams. The inspector names held outputs and the remedy. Recipe cards,
accessible descriptions and the Studio's recipe/item/chain views name both products.

The powered asphalt mixer makes **4 asphalt from 3 gravel + 1 bitumen in 20 ticks**. Feed refined
fuel to existing burners, boilers or other fuel-accepting machinery. Process energy stays separate
from ingredients. The refinery draws 8 grid energy per work tick, the mixer 6, and the well 6.

**Petroleum Processing** costs 16 insight after Material Processing and On-site Power, unlocking
the well and refinery. **Asphalt Roads** costs 12 after Petroleum Processing and Mechanical Shaping,
unlocking the mixer and road. These nodes do not gate starter automation or oil-free construction.
Three one-time projects add 80 insight: road binder, useful side stream and petroleum roads. The
committed catalogue pays **706 insight against 156 purchasable research insight**; starter grants
and personal skills are unchanged.

Ground works exposes asphalt on its existing material shelf. First lay a gravel yard, then pay
**2 asphalt per hex** for the top layer. Asphalt gives **50% faster movement than untreated ground**;
gravel remains 20% faster. The paid gravel stays in the road, not in the player's pack. Stripping
recovers both paid layers; undo restores the exact prior surface and stock. Reapplying the same
surface never charges or refunds anything. Free creative paving cannot mint materials later.
Native preview and commit enforce the same research, base, terrain and bounded-selection rules.
Material controls are keyed and updated in place to preserve focus as snapshots change.

## Accounting and compatibility

Save **32**, definitions **26**, technologies **14**, world **10**; scenarios **7** and wire **17**
stay unchanged. The adjacent migration preserves stored world parameters, deposits, inventory and
jobs. Load checks the original world-stamp checksum before updating creative capabilities. It no
longer subjects existing worlds to newer new-game bootstrap promises. New worlds still require a
valid opening. Load failures now appear on the title screen and do not dismiss it.

**Existing worlds do not acquire oil sites.** Their saved site rules remain authoritative. The
research panel warns when a world's generation rules contain no oil; start a new world for this
branch. Pre-masonry worlds likewise do not gain limestone. This release does not reroll old maps.

Recipes retain a primary output and add bounded co-products. Every product has a positive integer
quantity, a unique identity, and a positive integer allocation share; shares sum to exactly 100.
Every compatible machine must fit the whole output batch. Joint completion uses native inventory
maps and the existing dirty snapshot protocol, not a host tick or a new transport kind.

Multiple producers require an explicit, complete item-level `production_routes` order. Native
reachability tries usable unlocked routes in that order; guidance has an ordered fallback. No new
alternative recipe is introduced without a gameplay tradeoff. Cyclic routes and ambiguous shares
are rejected. The refinery allocation is **50% to bitumen and 50% to refined fuel**. Allocation is
for accounting, not a fractional runtime quantity. Whole-batch costing carries co-product surplus
so requiring both outputs does not purchase the refinery batch twice.

[`fixtures/balance.json`](../fixtures/balance.json) includes named recipe routes, raw inputs,
whole-batch quantities, allocation shares, source machines, research prerequisites, per-machine
grid/fuel energy and rates, plus the road's separate base bill. At full recipe utilization the
refinery produces 40 units/minute of each product and spends 240 grid energy per batch. These are
recipe-machine figures, not total upstream supply-chain power. The four crude units contain the
same fuel energy as the two refined units (320), so refining does not create combustible energy.

## Measured evidence

The native opening survey at seed **1,213,486,160**, radius **96**, samples 27,937 hexes per preset.
This is an opening-scale sample, not the wider landform census or a promise about every seed.

| Preset      | Oil hexes | Oil patches | Nearest workable oil patch (hexes) |
| ----------- | --------: | ----------: | ---------------------------------: |
| Continental |       112 |          11 |                                 28 |
| Archipelago |       194 |           7 |                                 36 |
| Highlands   |        76 |           3 |                                 49 |
| Basin       |        77 |           7 |                                 28 |

Raw evidence: [petroleum-roads-survey.json](benchmarks/petroleum-roads-survey.json), reproduced by
`npm run survey -- --radius 96 --json docs/benchmarks/petroleum-roads-survey.json`.
Original starter-material patch and geography assertions remain in place; oil is not a guaranteed
starter deposit. Lowland oil uses a weight of 8; the higher prototype displaced too much forest.

The committed `petroleum_road_journeys_keep_gravel_useful_and_make_long_routes_faster` test compares
native click-to-walk journeys on an isolated level corridor. These are **simulation player steps at
30 Hz**, not wall-clock performance or a human playtest. Run with `cargo test --manifest-path
factory-wasm/Cargo.toml petroleum_road_journeys -- --nocapture`.

| Journey  | Untreated | Gravel | Asphalt |
| -------- | --------: | -----: | ------: |
| 6 hexes  |        37 |     31 |      25 |
| 24 hexes |       154 |    128 |     103 |
| 60 hexes |       386 |    322 |     258 |

Asphalt cuts about 20% from gravel journey time. That incremental benefit is deliberately a road
specialty, not a reason to replace every inexpensive yard. The initial 4-unit mixer yield supports
two paved hexes per batch without making petroleum a startup requirement. Whether an oil outpost
feels worthwhile over a long play session remains a tuning question, not a measured fun claim.

The six-tier release-profile native ladder is recorded in
[petroleum-roads-native.json](benchmarks/petroleum-roads-native.json). At 6,144 entities this run
measured **340.4 µs/tick**, **1,315.4 µs native frame**, **3,238.4 µs full snapshot**, and
**17,469.9 bytes mean binary delta**. It was run on the local Windows desktop with the development
browser open; it is not an isolated same-machine comparison to the historical reference records.
The workload is the existing synthetic belt/component ladder, not a petroleum megafactory, road
network or complete browser frame. No new renderer-capacity claim follows from these numbers.

## Verification and limits

The complete local `npm run quality` gate passed: audit (zero vulnerabilities), agent-map freshness,
Prettier/rustfmt, ESLint, TypeScript, **279 Vitest tests**, **232 Rust tests**, Wasm and production
build. Vite's existing large-chunk advisory remains; it is not a failed build or a new frame budget.

Native regressions cover powered well → refinery → splitter → mixer/storage and fuel consumer;
joint-output backpressure and resume; partial jobs and both outputs refunded on erase; full-vs-dirty
snapshots; specialized extraction and depletion; surface gating/base/payment/strip/undo; ordered
route fallback; and envelope/checksum/site-table preservation with tamper rejection. The migration
tests include pre-masonry worlds and re-saving/reloading the migrated result.

Browser checks covered research search and road prerequisites, both refinery output chips,
base-required refusal, gravel then asphalt application, undo, and a 390×844 material tray without
horizontal overflow. Current-build browser save/load succeeds. Two older local QA slots (save 25
and 29) still reject with checksum mismatch; they were left untouched and their checksums were not
rewritten. Their historical-state compatibility is not claimed by the synthetic migration tests.
The title-screen rejection is now visible instead of appearing to do nothing.

No screen-reader audit, timed opening validation, full petroleum browser-capacity ladder, or
long-session player study is claimed. Supported floors remain later work. The concurrently edited
roadmap's handling/clarity and straight-boundary briefs are not implemented by this release.
