# HexFactory performance evidence

This file records only the measurements that support current claims. Raw reports in `docs/benchmarks/`
are authoritative; experiment history belongs in git.

## Engineering E0 baseline (in progress)

The programme is open. Environment and the initial complete quality result are recorded in
[`e0/environment.json`](benchmarks/e0/environment.json); the initial production payload is in
[`e0/startup-initial.json`](benchmarks/e0/startup-initial.json). The reference CPU is a Ryzen 7
5800X, Windows 11 10.0.26200, RTX 3060 driver 32.0.15.9186. No score is awarded from these records.

The new native sampler is separate from the historical schema-4 aggregate ladder:

```powershell
cargo build --release --manifest-path factory-wasm/Cargo.toml --bin steady
factory-wasm/target/release/steady.exe active 6144 docs/benchmarks/e0/active-6144.json
factory-wasm/target/release/steady.exe idle 6144 docs/benchmarks/e0/idle-6144.json
factory-wasm/target/release/steady.exe blocked 6144 docs/benchmarks/e0/blocked-6144.json
factory-wasm/target/release/steady.exe junction 6144 docs/benchmarks/e0/junction-6144.json
node scripts/pack-steady-report.mjs docs/benchmarks/e0/active-6144.json
node scripts/pack-steady-report.mjs docs/benchmarks/e0/idle-6144.json
node scripts/pack-steady-report.mjs docs/benchmarks/e0/blocked-6144.json
node scripts/pack-steady-report.mjs docs/benchmarks/e0/junction-6144.json
```

Supported entity counts are 768, 3,072, 6,144 and 24,576. Each invocation creates five independent
runs. Each run warms for five wall-clock seconds, then reconstructs the fixed starting state before
collecting individual samples for thirty wall-clock seconds. Setup (including reconstruction and
the initial full publication) is reported separately. Collection overhead, sample-vector growth,
and disposal between spans count toward elapsed collection time, but not individual CPU spans.
Both independently owned factories advance once per sample: the first measures `advance_ticks(1)`;
the second measures bounded idle-command parsing, advance, delta construction and binary encoding.
The reported spans overlap in purpose and are **not added together**. The isolated tick path discards
publication marks outside its timing span to model the per-frame publication boundary without
unbounded accumulation. The encoded path consumes them through the shipped publication code.

Workload identity remains the historical seed 2,071,003,907 and twelve-entity eastward lines,
three rows apart, with six middle belts, unmetered power, uniform terrain, million-unit ore deposits,
and the explicitly synthetic two-ore/eight-tick component recipe. Starting state is fixed at 400
ticks. Active lines produce; idle lines suspend every switchable machine **before** those ticks and
start with empty belts/storage. Idle is not an insufficient-power substitute.

Blocked lines shut every line's delivery belt **before** the first tick, by rotating it through the
same public path the player's rotate key uses onto the one heading whose output ray leaves the
blueprint, and then hold that state for a further 4,096 ticks — long enough for the belt, the
container, the composer's output and the whole belt run back to the extractor to fill. Sinks reopen
on sample 3,600, one rotate per line, timed and reported separately and excluded from every sample
span. A blocked run therefore reports two named phases with independent distributions and production
totals rather than one blended percentile; other workloads report a single `steady` phase. The
reopening edit's own publication marks land in the first reopened sample, so that sample's encoded
payload is an edit rather than a tick.

Junction is a different factory rather than the same lines relabelled, because a straight chain
measures no routing at all. It repeats a twenty-four entity unit in which four lanes carrying four
materials merge into one trunk through three chained mergers, the trunk dives beneath an independent
fifth lane through an underpass pair, and a splitter fans it into three branches; six of the
twenty-four entities are junction primitives and every delivered item has crossed at least one
merger, one underpass pair and one splitter. The unit divides the ladder exactly — 768, 3,072, 6,144
and 24,576 entities are 32, 128, 256 and 1,024 units — and repeats without sharing a hex, a reserved
service envelope or a compiled edge, so a tier's cost is one unit's multiplied.

The four merged lanes deliberately offer 0.254 items a tick into a trunk that carries 0.2, so every
merger has cargo waiting on both feeders on every tick and the round robin arbitrates a standing
contest. Under a load the trunk could absorb, deterministic sources phase-lock into a conflict-free
schedule and the arbitration never runs, which would measure belts wearing merger icons. It is still
a live steady state and not a jam: the trunk delivers a full belt's throughput indefinitely, with
backlogs bounded by the belts' lane slots and the extractors' output. Starting state is fixed at 400
ticks plus a further 1,024 for the mergers' and splitters' cursors to settle into the phase they
keep; that figure is measured rather than estimated, by requiring an identical throughput window
after twice the warmup. One material per lane makes the core's own `delivered_by_item` an exact
per-lane throughput meter, and CI pins the graph the unit compiles, the five lanes' individual
production over three thousand ticks, the trunk's sum against a belt's rate, that the hex the trunk
passes beneath never holds a trunk material, and that the two deepest lanes split their merger
evenly where entity-id order would starve one of them to zero. Before it starts timing, a junction
collection counts the mergers, crossings and splitters in the factory it built, from the compiled
graph and outside every sample span, so a routing change that quietly turned the tier into belts
fails the workload instead of being reported as junction throughput.

Each run records its actual sample/tick count, starting and ending checksum and delivered totals;
the isolated and encoded paths must have identical canonical checksums. CI pins a twelve-entity
520-tick replay for all four workloads and checks the supplied-clock arithmetic. It also proves the
blocked workload's two claims directly: a shut line is still working three quarters of the way
through the saturation wait, is afterwards a fixed point whose entities are byte-identical after 600
further ticks, and on reopening compiles exactly one outlet per line, drains its backlog and
delivers at least the active workload's rate. Raw sample order is retained, with nearest-rank
median/p95/p99, maximum, delta bytes and entity/resource dirty-mark counts (including duplicate
marks, not unique records). Counters exist only in the harness.

The packer checks all five windows, every sample count and every reported percentile — for the whole
window and again for each phase, against that phase's own samples — then verifies that the phases
tile the window exactly and that their production totals sum to the run's. For a blocked record it
additionally requires the reopen to have happened at the same sample in all five runs, the blocked
phase to have delivered nothing and published no entity or resource mark at all, and the reopened
phase to have done both. Finally it verifies gzip round-trip equality. Committed raw records are
lossless `.json.gz`;
readable `.summary.json` files carry their uncompressed SHA-256 and per-run statistics. Uncompressed
duplicates can be removed after successful packing. Each collection's source commit, executable
hash, exact commands and limitations are in
[`e0/measurement.json`](benchmarks/e0/measurement.json), one entry per collection, so a record is
traceable to the source that produced it rather than to whatever HEAD happens to be.

At `df3eb43`, the reference-size records are
[active raw](benchmarks/e0/active-6144.json.gz),
[active summary](benchmarks/e0/active-6144.summary.json),
[idle raw](benchmarks/e0/idle-6144.json.gz),
[idle summary](benchmarks/e0/idle-6144.summary.json),
[blocked raw](benchmarks/e0/blocked-6144.json.gz) and
[blocked summary](benchmarks/e0/blocked-6144.summary.json). They replace the schema-1 active and
idle records collected at `8db0bf2`, which the phase-aware packer cannot re-verify; the two sets
agree to within a few per cent, and neither is a regression measurement of the other. The junction
record is separate, collected at `3fea3c2`:
[junction raw](benchmarks/e0/junction-6144.json.gz) and
[junction summary](benchmarks/e0/junction-6144.summary.json). Values below
are microseconds; each sample has one independent tick span and one advance/encode span. Sample
counts differ because collection uses wall time. These are provisional observations: no builds or
tests ran during collection, but thermal sensors and unrelated system load were not independently
monitored.

| Workload/run | Samples | Tick median | Tick p95 | Advance/encode median | Advance/encode p95 |
| ------------ | ------: | ----------: | -------: | --------------------: | -----------------: |
| Active 1     |  13,869 |       371.4 |    632.2 |               1,617.4 |            2,927.0 |
| Active 2     |  13,661 |       381.4 |    637.9 |               1,652.4 |            2,958.3 |
| Active 3     |  13,840 |       369.6 |    624.2 |               1,624.6 |            2,929.8 |
| Active 4     |  13,663 |       377.1 |    638.8 |               1,665.2 |            2,974.9 |
| Active 5     |  13,756 |       373.7 |    646.3 |               1,649.4 |            2,946.0 |
| Idle 1       |  51,275 |        43.5 |     77.5 |                 504.5 |              643.5 |
| Idle 2       |  54,277 |        41.1 |     70.0 |                 477.3 |              615.7 |
| Idle 3       |  57,713 |        38.3 |     61.3 |                 467.1 |              560.4 |
| Idle 4       |  57,488 |        38.5 |     62.7 |                 465.8 |              578.0 |
| Idle 5       |  57,575 |        39.2 |     62.3 |                 466.7 |              565.0 |
| Blocked 1    |  10,576 |       492.8 |    931.4 |               1,535.1 |            3,001.4 |
| Blocked 2    |  10,359 |       514.5 |    964.8 |               1,595.4 |            3,031.9 |
| Blocked 3    |  10,702 |       498.4 |    937.6 |               1,522.8 |            3,021.8 |
| Blocked 4    |  10,513 |       492.2 |    938.5 |               1,549.4 |            3,033.3 |
| Blocked 5    |  10,578 |       486.0 |    929.8 |               1,536.5 |            3,045.1 |
| Junction 1   |   8,044 |       601.1 |    799.7 |               3,001.7 |            3,979.1 |
| Junction 2   |   8,137 |       592.7 |    782.1 |               2,965.4 |            3,919.8 |
| Junction 3   |   7,281 |       643.2 |  1,128.5 |               3,163.6 |            5,074.1 |
| Junction 4   |   6,586 |       738.6 |  1,201.1 |               3,589.9 |            5,395.8 |
| Junction 5   |   6,968 |       700.9 |  1,132.7 |               3,365.9 |            5,090.6 |

A blocked run's window is two regimes and the row above blends them; the phases the record reports
separately are what the workload actually measured. Every run reopened on sample 3,600.

| Phase/run  | Samples | Tick median | Tick p95 | Advance/encode median | Advance/encode p95 |
| ---------- | ------: | ----------: | -------: | --------------------: | -----------------: |
| Blocked 1  |   3,600 |       545.6 |    961.5 |               1,425.6 |            1,972.0 |
| Blocked 2  |   3,600 |       584.0 |  1,000.7 |               1,500.4 |            2,037.3 |
| Blocked 3  |   3,600 |       548.5 |    965.0 |               1,434.7 |            1,980.8 |
| Blocked 4  |   3,600 |       557.8 |    967.9 |               1,464.9 |            1,988.5 |
| Blocked 5  |   3,600 |       540.0 |    961.7 |               1,425.3 |            1,973.7 |
| Reopened 1 |   6,976 |       386.7 |    715.0 |               1,715.0 |            3,334.9 |
| Reopened 2 |   6,759 |       417.9 |    717.0 |               1,732.5 |            3,364.8 |
| Reopened 3 |   7,102 |       399.5 |    719.5 |               1,671.5 |            3,357.1 |
| Reopened 4 |   6,913 |       384.9 |    736.6 |               1,680.8 |            3,430.8 |
| Reopened 5 |   6,978 |       382.9 |    705.0 |               1,697.7 |            3,363.6 |

**Two 6,144-entity budgets are missed, and the stages that own them stay open.** The advance/encode
p95 ceiling of 3,000 µs is exceeded by every blocked run, across the whole window (3,001–3,045) and
in the reopened phase alone (3,335–3,431), and by every junction run by 31–80% (3,920–5,396); the
active workload clears the same ceiling with only 0.8–2.4% spare. The tick p95 ceiling of 1,000 µs
is met across every blocked window but exceeded in the blocked phase of run 2 (1,000.7) and in
junction runs 3–5 (1,129–1,201). Nothing is optimized or relaxed here: E0 records the baseline, and
E4 chooses its target order from it.

**The junction record is not usable as a percentile baseline, and is kept for its structure rather
than its numbers.** Its five runs deliver an identical 59.74 items per tick to two decimal places,
so the factory does the same work in each; yet runs 3–5 are 8–25% slower per tick than runs 1–2. An
earlier collection of the same workload, from a cold start on the same host, reproduced the same
fast-fast-slow-slow-slow ordering. Within a slow run the rate wanders in both directions — run 3
starts at run 1's speed and degrades, runs 4 and 5 start slow and recover — so this is the host's
sustained clock rather than the factory's state settling. The active and blocked collections, seven
times lighter per tick, show a 3–6% spread with no such ordering. This host has no readable thermal
sensor, so the cause is not established here; what is established is that a workload this heavy is
outside the range these five-run collections can currently resolve. The budget comparison above
survives it only because the advance/encode miss is far larger than the spread; the tick p95 result
does not, and is reported as a range rather than a verdict.

What the junction workload does show is where a routing-dense factory spends. At the same entity
count it publishes 2,480 entity dirty marks per tick against the active line's 1,041, and its
advance/encode median is 2,965–3,590 µs against 1,617–1,665 — so the cost lands on the publication
and encode path rather than on the tick, which rises by only 1.6–1.9×. It also delivers seven times
as many items per tick, but that is a different factory rather than a speedup: junction units
extract and consume, while the active line runs the synthetic two-ore composer recipe. Setup is
4.2–4.9 s per run against the active line's 1.4 s, all of it the 1,424-tick fixed starting state,
reported outside every sample span.

Three things these records show, and one they do not. A completely jammed factory costs **more** per
tick than a producing one — 486–515 µs against 370–381 µs across the whole window, and 540–584 µs in
the blocked phase — while publishing nothing at all. Reopening 512 sinks through the public rotate
path cost 2.82–2.99 s, about 5.7 ms per edit, consistent with the historical ladder's 4,045 µs
affected-component recompile at this size rather than a new result. The blocked starting state costs
21.7–21.9 s of setup per run, all of it the saturation wait, reported outside every sample span.
What they do **not** show is why a still factory is the expensive one; that needs the visit and
rebuild counters E0 still owes, not an inference from these two columns.

Idle records contain zero entity and resource dirty marks throughout all five windows, and blocked
records contain zero throughout all five blocked phases — a saturated factory publishes nothing,
which the packer requires rather than merely observes. This does not prove zero visits, allocations,
or O(1) resting cost; the larger tier and visit counters remain required. Idle's maximum tick is
1.7–2.6 ms against a 39 µs median, so even a resting window carries outliers that the percentiles
hide and that nothing here attributes. No optimization is justified or declared complete by this
table alone.

Browser reports tagged `snapshot-and-draw-v1` add first-setter setup and separately timed repeated
world/minimap setters. `render_us` retains its historical draw-only meaning. `browser_frame_us`
retains its historical sum for file compatibility, but the UI labels it as an isolated sum;
`isolated_pipeline_us` also includes setters. Neither sum proves rAF, GPU execution or presentation.
Repeated setters use the identical snapshot and cannot stand in for changing-state application
updates. Renderer diagnostic p95 and smoothed preparation values include renderer history; they
are not the new sampler's steady-state percentiles. The browser quick ladder was smoke-tested in
Chromium 152, Low, DPR 1: both 12/192-entity merged snapshots were intact and both setter spans were
present. Its concurrent-build timings are rejected as baseline evidence.

Outstanding E0 gates: the remaining workload shapes — powered production under full and insufficient
supply, separate outposts with one edited component, and mixed extraction, regrowth, river pumping
and disturbed water — and a junction collection whose runs agree well enough to be a baseline; the
live browser scripts; complete native/Wasm ladder and
five-run browser distributions; real application/UI, GPU, rAF and interaction spans; operation and
rebuild counters beyond dirty marks; contamination/thermal evidence; desktop profile/DPR matrix;
integrated-GPU hardware; and warm/cold/throttled startup timing. Historical v0.43 results below are
unchanged and do not close these gates.

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

Phase 9's radius-96 fixed-seed habitat survey uses the shipped native predicate, not a host proxy. Fertile
riverbank occupies 686 cells, 33‰ of land, carries 76,050 integer capacity, and begins 13 hexes from the
landing point in each preset. This is scarce against the 10–60‰ acceptance band but near enough to support
an opening ecology route. Raw report: [`habitat-v0.47-survey.json`](benchmarks/habitat-v0.47-survey.json).

The generated figure rose from 589 cells and 28‰ when fertility stopped being a statement about intact
grade and became one about water: ground is farmland if it is dry, unbuilt and has fresh standing water in
its ring, rated by the channel's own class where the generator laid an alluvial bench and at the bottom of
the ladder everywhere else. The 97 cells that gained are fresh shorelines that carry no bench. What the
survey cannot show is the other half of the same change — a canal is standing water, so a trench cut inland
carries farmland with it, and that is measured in play rather than in the generator.

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
