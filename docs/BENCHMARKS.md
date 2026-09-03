# HexFactory performance evidence

This file records only the measurements that support current claims. Raw reports in `docs/benchmarks/`
are authoritative; experiment history belongs in git.

## Reproduce

```bash
npm run bench          # native factory ladder
npm run bench:browser  # build harness, then open /HexFactory/bench.html
npm run startup:check  # production startup payload, after npm run build
npm run survey         # resource and opening survey
npm run terra          # landform and drainage survey
npm run water          # disturbed-water solve
npm run erosion        # geomorphic epoch
```

`--quick` reduces a ladder and `--json <path>` writes a report. Timing commands are outside
`npm run quality` because shared runners are not comparable; CI pins workload checksums and harness behavior.
The browser harness is compiled only with the `bench` feature and is absent from production.

## Factory capacity

The deterministic workload repeats this always-accepting line at six sizes:

```text
extractor → 6 belts → composer → belt → container → belt → consumer
```

It measures one steady-state transport shape, not every possible factory. `tick` is native simulation;
`frame` adds one bounded command batch and encoded delta; browser totals also include worker round trip,
decode/merge, world render, and minimap.

### Native record

v0.43.0 release profile on Ryzen 7 5800X / Windows 11.
Raw report: [`capacity-v0.43-native.json`](benchmarks/capacity-v0.43-native.json).

| Tier   | Entities | Tick µs | Full snapshot µs | Frame µs | Compile µs | Recompile µs |
| ------ | -------: | ------: | ---------------: | -------: | ---------: | -----------: |
| line   |       12 |     0.9 |             28.3 |     15.4 |        4.8 |          8.6 |
| small  |      192 |     8.0 |             95.4 |     52.7 |       44.8 |        109.4 |
| medium |      768 |    37.0 |            483.9 |    199.6 |      211.7 |        432.4 |
| wide   |    1,536 |    79.8 |            827.3 |    368.5 |      447.1 |        919.4 |
| large  |    3,072 |   134.0 |          1,753.8 |    687.9 |    1,027.7 |      1,894.6 |
| xlarge |    6,144 |   286.9 |          3,488.0 |  1,372.2 |    1,906.1 |      4,045.5 |

At xlarge, the tick is 1.7% and the complete native frame 8.2% of 60 Hz. The tick is not the current
player-facing limit on this machine. The affected-component recompile is slower than a full compile for this
single connected line; that does not establish the result for ordinary localized edits.

### Browser record

v0.43.0 on the same desktop, Chromium 151, 1440×900 DPR 1, 178 px minimap. Raw reports:
[Low](benchmarks/capacity-v0.43-browser-low.json),
[Medium](benchmarks/capacity-v0.43-browser-medium.json), and
[High](benchmarks/capacity-v0.43-browser-high.json).

| Profile |    12 |   192 |     768 |   1,536 |   3,072 |           6,144 |
| ------- | ----: | ----: | ------: | ------: | ------: | --------------: |
| Low     | 633.0 | 827.3 | 1,443.4 | 2,521.7 | 3,198.2 | 5,386.4 (32.3%) |
| Medium  | 628.4 | 748.2 | 1,639.3 | 2,012.4 | 2,978.0 | 5,576.5 (33.5%) |
| High    | 845.4 | 641.7 | 1,104.8 | 1,835.3 | 3,425.9 | 5,656.0 (33.9%) |

Totals are µs; percentages are shares of a 60 Hz frame. All profiles pass the 35% reference-desktop gate
with only 1.1–2.7 percentage points spare. Draw calls remain 34–36 across the ladder; xlarge draws about
2.55 million triangles. Add and measure a representative workload before adding a permanent visual bucket.

## Worker payload

The binary dirty delta is transferred rather than structured-cloned. The v0.12.2 measurement remains the
current evidence for that encoding:

| Tier   | Entities | Binary bytes | Equivalent JSON bytes | Ratio |
| ------ | -------: | -----------: | --------------------: | ----: |
| line   |       12 |          104 |                 1,319 | 12.7× |
| medium |      768 |        5,803 |                79,477 | 13.7× |
| xlarge |    6,144 |       47,531 |               644,759 | 13.6× |

The round-trip floor after encoding is about 62 µs on the reference desktop. Below the wide tier, payload
reduction alone is unlikely to move a frame materially.

## Production startup

Startup is everything a browser fetches before the title screen answers: the document, the
stylesheet, every eagerly loaded script including the module worker, and the Wasm that worker
instantiates. The admin page is a separate entry a player never loads and is excluded by name.

`npm run startup:check` measures the built `dist/` and fails on a breach. Sizes are gzipped, because
that is what the player waits for; the raw bytes are what the browser then parses and compiles, and
a chunk that compresses well still costs that.

| Group        | Asset                         | Gzipped |     Raw | Budget |
| ------------ | ----------------------------- | ------: | ------: | -----: |
| `javascript` | main, frameClock, worker      |  273 KB |  989 KB | 320 KB |
| `wasm`       | `factory_wasm_bg`             |  486 KB | 1408 KB | 560 KB |
| `interface`  | `index.html`, main stylesheet |   35 KB |  174 KB |  48 KB |
| total        |                               |  794 KB | 2571 KB | 896 KB |

The total budget is below the sum of the three, so all three cannot spend their headroom at once.
Raising one is a decision about what a new player waits through: it is an edit to the ceilings in
`scripts/startup-budget.mjs` and a line here, not something a feature buys by growing a chunk.

`src/main.ts` emits a `hexfactory:ready` performance mark at the moment the worker is up, Wasm is
instantiated, the first snapshot is drawn, and the title screen takes input. On the reference desktop
over `vite preview` with a warm HTTP cache that mark lands at 604, 564, and 571 ms across three
loads, with `DOMContentLoaded` at 67 ms. The budget is **750 ms** to that mark on the reference
desktop. A cold cache, a slower link, and other hardware are not measured; the mark is what any of
those runs should report.

## World generation

The site-lattice survey measured at most 1.42 µs per hex, including survey bookkeeping: at most 91 µs for a
64-cell chunk and 640 µs for the seven-chunk neighborhood generated when entering unsurveyed ground. It is
not tick work.

Radius-1 site purity—the share of resource cells whose extractor neighborhood contains one material—is
965–992 per mille across the four presets against a 950 target. `npm run survey` also checks opening
guarantees, patch size/yield, nearest sites, terrain bands, water, and river distribution. The cache hit rate
under a natural walk remains unmeasured.

## Ground, drainage, and water

The current world-16 fixed samples report:

| Metric                                              |       Inland |       Coast |
| --------------------------------------------------- | -----------: | ----------: |
| Channel cells / sample                              |          14‰ |         10‰ |
| Wet coverage                                        |         129‰ |        288‰ |
| Walkable / buildable edges                          |  915‰ / 740‰ | 973‰ / 958‰ |
| Drainage cycles / uphill edges / unterminated walks |    0 / 0 / 0 |   0 / 0 / 0 |
| Widest water / dry bench                            | 13 / 2 cells | 9 / 2 cells |
| Sill cells / drops beyond wade limit                |        6 / 8 |       0 / 1 |

Rivers are a hierarchy of six widths rather than two. The discharge ladder doubles above the channel
threshold instead of quintupling, which is what the catchments this generator produces can actually pay for,
and half-width climbs one cell per class: 3, 5, 7, 9, 11 and 13 cells of water with a 1–2 cell sand bench
outside it. The inland sample carries classes 2 and 5–7; the coastal one carries 2, 4 and 5. Widening water
is widening wetness: over the 81-province survey the wet share went from 53‰ to 111‰ and the river cells from
70,080 to 146,928, against 966‰ → 941‰ walkable and 820‰ → 812‰ buildable.

The default landing contract is a dry buildable clearing no more than 100 m above sea level with exact ocean
between 22 and 28 cells away — a band, not a ceiling, so the opening stands on the coastal plain rather than
on the sand. The reference seed lands 22 cells from the sea at 9.5 m. Tests cover three fixed seeds and forty
preset/seed pairs. A viewport carries 54.0 m of relief across 429 m in the fixed inland sample. The
81-province drainage survey over about 1.3 million cells costs 710 ms to solve and 251 ms to sweep in
release-native runs, against 489 ms and 247 ms before the widening: a wider valley is more cells for the
incision sweep to settle. Seven of its 16,817 river edges rise, worst 492 mm, against five of 18,100 at
524 mm before.

`npm run water` moved a 32-quantum disturbance to a fixed point over 41 active cells in 53 sweeps and 40
transfers. A settled world advanced 100,000 ticks with no water dirty mark and no change to the departure set:
standing water has no permanent per-cell tick.

`npm run erosion` inspected 121 chunks, 7,744 cells, and 1,086 wet flowing edges in one epoch. The structural
limits are 256 chunks, 65,536 cells, 4,096 edges, and 64 bank changes per epoch in deterministic rotating
order. Dry, straight, and protected reaches do no erosion work.

## Evidence limits

- One desktop, Chromium build, resolution, and DPR; no mobile, Firefox, Safari, or integrated-GPU claim.
- One capacity shape with straight, always-accepting lines; no junction-dense, backpressured, power-dense, or
  stacked-floor workload yet.
- One run per tier and no confidence interval. Treat differences below roughly 20% as noise.
- Camera follows the player; zoomed-out views and DPR 2 are different workloads.
- The ladder finds no ceiling above 6,144 entities.
- A checksum change invalidates checksum comparison, not a timing comparison. State which one a record uses.

## Required next measurements

1. Measure forestry extractor starvation over seven cells at the current regrowth cadence.
2. Measure startup on a cold cache and a throttled link; the current record is warm and local.
3. Add ecology population/waste work to a deterministic native tier before phase 9 rendering.
4. Add stacked floors, lifts, and cross-level graph edges to a tier before phase 11 rendering; repeat Low,
   Medium, and High.
5. Add junction-dense, backpressured, and power-dense tiers before changing scheduler or graph strategy.
6. Measure site-cache behavior under a natural walk and report affected component size before revisiting
   incremental transport compilation.
