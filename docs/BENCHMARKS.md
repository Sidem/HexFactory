# HexFactory capacity benchmarks

## Handling and Clarity paving check — v0.41.0

The focused [625-cell Low paving report](benchmarks/handling-clarity-paving-low.json) uses
`paving-bench.html` and the production renderer. At 1200×720, DPR 1, Chromium 151 on Windows,
60 warmup frames and 240 measured frames per material yielded 249–303 µs mean CPU submission
for paved cases (p95 300–400 µs). The untreated control measured 318 µs mean. Each single-material
yard adds one draw call and one geometry, with no added texture. RAF p95 was 16.7–16.8 ms.
This is a synthetic level yard, not a native simulation or GPU completion measurement, and is not
evidence of a speedup over the previous release. The full capacity records below remain unchanged.
This section is the surviving procedure and limitation record; the shipped release itself has
collapsed to its one ledger line in `HEXFACTORY-PLAN.md`.

Capacity is measured, never asserted, and the measurement orders the work. Every number here was
produced by the committed harness; the raw reports live in `docs/benchmarks/` and are the source for
any table that was trimmed out of this document.

**Current records.** Native: **v0.43.0 current-build audit**. Browser frame: **v0.43.0 profile ladders**.
Generation: **v0.21**. Payload: **Binary Delta v0.12.2**.

v0.37 shipped boundary correctness and small-browser interaction checks but no large-perimeter
capacity measurement, and the historical factory ladders do not establish boundary capacity.

**Two caveats travel with those records.**

- **The v0.24 and v0.25 browser records are historical baselines, not current renderer evidence.**
  Current Three.js claims must name the v0.43 Low, Medium, or High record. The reference desktop is
  the support target; integrated-GPU laptop qualification was withdrawn by user decision on
  2026-08-27.
- **Every tier checksum below is historical.** v0.18, v0.19, v0.20, v0.21, and the world-scale
  pass, and v0.22 each moved the pinned workload checksum: `2402899979` → `1679299541` →
  `914129621` → `780276626` → `325426962` → `3745973835` → `1543489001` → `841205484` →
  `3799495709`. The first three added
  saved, checksummed state; v0.21 and the scale pass moved `WORLD_GENERATOR_VERSION`, and v0.22
  moved the definition protocol and entity roster, all of which the checksum reads. None of them
  changed the workload's shape, entity counts, or delivered totals, so **the timings remain
  comparable while the checksums do not.** A checksum change invalidates checksum comparisons, not
  timing ones — say which of the two a record claims.

Run the ladders:

```bash
npm run bench
```

```bash
npm run bench:browser
```

`npm run bench -- --quick` runs a reduced ladder and `--json <path>` writes the machine-readable
report. The browser command builds the harness artifact and starts the dev server; open
`/HexFactory/bench.html` and press **Run full ladder**.

Neither is part of `npm run quality`: shared CI runners do not produce comparable timings. The test
gate instead pins the workload's checksum and asserts the harness still runs, so recorded numbers
cannot silently stop being comparable. The harness compiles into wasm only under `--features bench`,
so the deployed artifact never carries it (v0.12.2: shipped 520.0 KiB, harness build 563.8 KiB).

## What is measured

Since v0.33.0 the harness explicitly pins its historical `2 ore -> 1 component`, eight-tick,
fuel-free recipe inside the bench-only catalogue. Gameplay now uses plates and gears; applying
that bill to this isolated one-input line would measure a stalled factory. The existing pinned
checksum (2,222,187,037), entity count and four deliveries per line still pass. This preserves the
workload, not a new timing measurement or evidence for the gameplay component chain.

Six tiers of the same shape, differing only in how many production lines run at once. One line is:

`extractor → 6 belts → composer → belt → container → belt → consumer`

Every line sits on its own effectively inexhaustible deposit, three rows away from its neighbours,
and runs east. The consumer always accepts, so a tier reaches a steady state and stays there: the
extractor cadence of 5 and the two-ore, eight-tick recipe make each line deliver one component every
ten ticks for the whole measured run. Tiers differ in size, not in behaviour.

Each tier is measured in separately timed phases, each on its own freshly warmed core so no
measurement inherits a state the previous one left.

| Metric          | What it times                                                               |
| --------------- | --------------------------------------------------------------------------- |
| `tick`          | one simulation tick, with no snapshot and no serialization                  |
| `snapshot`      | building one complete native snapshot, before serialization                 |
| `checksum`      | one native checksum, which every delta carries                              |
| `frame`         | one worker frame: bounded command batch, one tick, and one encoded delta    |
| `delta bytes`   | the encoded delta payload that frame sends across the worker boundary       |
| `json bytes`    | what the same frames would have cost as JSON, the encoding's own comparison |
| `compile`       | one full deterministic transport compile, as used on load and restore       |
| `recompile`     | the incremental transport machinery alone, for one edit                     |
| `edit`          | one complete public rotate edit, legality checks included                   |
| `round trip`    | browser only: the same frame, requested and received over the worker RPC    |
| `apply`         | browser only: the main thread merging that delta into its cached snapshot   |
| `host frame`    | browser only: `round trip + apply`, one simulated frame, excluding render   |
| `world`         | browser only: the world renderer's `draw` at a pinned 1440×900 viewport     |
| `minimap`       | browser only: the minimap renderer's `draw` at the shipped 178 px square    |
| `render`        | browser only: `world + minimap`                                             |
| `browser frame` | browser only: `host frame + render`, one frame end to end                   |

`recompile` and `compile` are directly comparable — the incremental path is timed without the edit
path's legality work.

`snapshot` is not part of a frame. Since v0.7 the complete snapshot is built only for the host's
first frame, and it is kept in the ladder as the baseline the incremental delta is measured against.

The host metrics are what a native run cannot see. `frame` stops at the edge of wasm; `round trip`
is `postMessage` out, the transfer of the delta buffer, the main thread's decode, and both
scheduling hops; `apply` is `applySnapshotDelta` merging the per-entity patch.

**Sample budgets differ between the two records, and only the sample budgets.** A native clock
resolves nanoseconds, so each phase runs its tier's fixed sample block once. A browser clamps
`performance.now` to 100 µs unless the page is cross-origin isolated, so each phase repeats its
block until it has run at least 20 ms — holding the clock step to 0.5% of the phase. Every metric is
a mean per tick, per frame, or per edit, so the two remain comparable; the workload never changes,
and each tier's checksum comes from a separate core advanced exactly once through its tick budget so
extra samples cannot move it.

## Native — the current record (v0.43.0 audit)

Recorded 2026-08-29 on the same Ryzen 7 5800X / Windows 11 reference desktop with the shipped
release profile. Raw report:
[`capacity-v0.43-native.json`](benchmarks/capacity-v0.43-native.json), SHA-256
`7C6F28C832D6D4D21A77A112DF0D0A3FBFD6E8DCA3FBEAFADA5696C426F9ED0F`.

| tier   | entities | tick µs | snapshot µs | checksum µs | frame µs | compile µs | recompile µs | edit µs |
| ------ | -------: | ------: | ----------: | ----------: | -------: | ---------: | -----------: | ------: |
| line   |       12 |     0.9 |        28.3 |         1.8 |     15.4 |        4.8 |          8.6 |    16.1 |
| small  |      192 |     8.0 |        95.4 |        16.0 |     52.7 |       44.8 |        109.4 |   139.4 |
| medium |      768 |    37.0 |       483.9 |        54.6 |    199.6 |      211.7 |        432.4 |   405.4 |
| wide   |    1,536 |    79.8 |       827.3 |       119.8 |    368.5 |      447.1 |        919.4 |   954.8 |
| large  |    3,072 |   134.0 |     1,753.8 |       202.2 |    687.9 |    1,027.7 |      1,894.6 | 2,174.8 |
| xlarge |    6,144 |   286.9 |     3,488.0 |       411.9 |  1,372.2 |    1,906.1 |      4,045.5 | 3,887.8 |

The xlarge tick is 1.7% of a 60 Hz frame and the complete in-wasm frame is 8.2%; at the shipped
10 tps the tick itself is not the player-facing limit on this machine. Complete snapshots remain an
opening/load baseline rather than a recurring frame cost. The current workload still walks every
runtime-indexed machine, power participant and transport source, and power allocation constructs
ordered request and plant groups each tick. This record says that work fits; it does not prove that
the scheduler or its allocations are optimal.

Do not read the higher absolute values than v0.25.1 as a regression percentage. Definitions,
geometry-era state, generation and the pinned checksum all moved across eight releases, so the old
and new records are not a same-build A/B. What is settled is narrower: v0.25.1 can no longer be
quoted as current-build cost.

## Native — v0.25.1 runtime-index baseline

Host: AMD Ryzen 7 5800X (8 cores / 16 threads), Windows 11 Pro 10.0.26200, rustc 1.87.0,
`factory-wasm` built with the shipped release profile (`opt-level = "s"`, LTO, `wasm-opt -Oz`).
Recorded 2026-08-24. Raw reports: the current
[`runtime-index record`](benchmarks/capacity-v0.25.1-native-runtime-index.json) and its immediately
preceding [`same-build baseline`](benchmarks/capacity-v0.25.1-native.json).

| tier   | entities | tick µs | snapshot µs | checksum µs | frame µs | compile µs | recompile µs |
| ------ | -------: | ------: | ----------: | ----------: | -------: | ---------: | -----------: |
| line   |       12 |     0.4 |        12.3 |         1.7 |      4.3 |        3.0 |          7.9 |
| small  |      192 |     5.3 |        59.0 |        13.9 |     27.0 |       36.2 |        100.7 |
| medium |      768 |    24.6 |       239.3 |        52.0 |    104.4 |      174.0 |        344.5 |
| wide   |    1,536 |    48.9 |       621.5 |       106.8 |    221.7 |      374.7 |        778.4 |
| large  |    3,072 |    89.9 |     1,207.2 |       216.9 |    522.9 |      778.6 |      1,559.8 |
| xlarge |    6,144 |   219.9 |     2,851.2 |       497.4 |    906.8 |    1,724.0 |      3,320.2 |

The runtime index moves stable entity order, machine order, reverse feeder edges, and merger targets
to topology compilation rather than rebuilding them every tick. Against the same-build baseline,
xlarge tick fell 275.2 → 219.9 µs (20.1%) while the complete native frame moved 902.8 → 906.8 µs
(0.4%, inside the historical noise floor). Full compile rose 1,316.1 → 1,724.0 µs because it now
builds those indexes once; a topology edit pays that cost so every subsequent tick does not. The
record supports the tick improvement and the placement of work, not a claim that the complete frame
meaningfully changed. `line` is below the useful timer scale; read the larger tiers.

What this deliberately does **not** say is anything about generation. The ladder's scenario
sets `generated_environment: false` and never calls `terrain_at` or `field_at`, which is exactly why
the site lattice needs its own measurement below — and the ladder being flat across a milestone that
rewrote generation is the evidence that the two paths are as separate as that flag claims.

### Payload, from v0.12.2

The delta crosses the worker boundary as a compact binary buffer that is transferred rather than
structured-cloned. The two columns are the same frames measured both ways, not two runs compared.

| tier   | entities | delta bytes | json bytes | ratio |
| ------ | -------: | ----------: | ---------: | ----: |
| line   |       12 |         104 |      1,319 | 12.7× |
| small  |      192 |       1,376 |     19,764 | 14.4× |
| medium |      768 |       5,803 |     79,477 | 13.7× |
| wide   |    1,536 |      11,819 |    159,709 | 13.5× |
| large  |    3,072 |      23,723 |    320,754 | 13.5× |
| xlarge |    6,144 |      47,531 |    644,759 | 13.6× |

The saving comes from varints instead of decimal text, one byte per closed-set enum, delta-coded
entity ids and tile coordinates, and a bit per absent option instead of a field name and a `null`.
The encoder cost 3.7 KiB of shipped wasm, `snapshot_delta_json` retained as its oracle included.

## Generation — the current record (v0.21)

v0.21 made a deposit a **site** rather than a per-hex decision, so `field_at` stopped reading three
noise channels at one hex and started scanning every lattice cell within reach of it. The naive form
of that is roughly 350 noise samples per hex and was never shippable; the shipped form caches the
site lattice, which is `site_cell²` hexes per entry, so every hex of a chunk hits it warm.

Measured with `survey.exe` on `continental` at two radii, five runs each, taking medians and
differencing to cancel process start-up. **Both builds were measured on this host in the same
session**, because the v0.16 figure below is not comparable to either: the survey itself grew patch
statistics in v0.20.1 and river, beach, and bootstrap reporting in v0.21, and none of that is
generation.

| build                         | radius 48 (7,057) | radius 96 (27,937) | µs per hex |
| ----------------------------- | ----------------: | -----------------: | ---------: |
| v0.16, as recorded 2026-08-18 |           10.6 ms |            18.0 ms |       0.35 |
| v0.20.1 (`535f8d8`)           |           15.2 ms |            26.0 ms |       0.52 |
| **v0.21**                     |       **20.7 ms** |        **50.3 ms** |   **1.42** |

**2.7× against the model it replaced, and it buys the milestone's whole point.** The figure covers
terrain, field, and the survey's own bookkeeping, so it is an upper bound on generation alone and
the v0.21 row carries more bookkeeping than the v0.20.1 row does.

What it means in the game: a chunk is 64 hexes, so generating one costs at most ~91 µs and
`ensure_neighborhood`'s seven chunks at most ~640 µs — about 4% of a 60 Hz frame, paid only when the
player walks into unsurveyed ground, and never in the tick.

**What is still not measured** is the cache's hit rate under a real walk, as opposed to under a
survey that sweeps a disc in lattice order. A walking player crosses cells in a worse order than
that, and the map only ever grows. If chunk generation ever shows up in a frame, that is the first
place to look.

## Generation — what the shipped presets actually contain

A threshold is not a proportion, so a preset's claims about its own landscape come from
`npm run survey` rather than from reading its parameters. This is the reference survey for the four
shipped presets, and it is measurement rather than roadmap, which is why it lives here.

At seed 1,213,486,160 after the world-scale pass. Each preset is sampled at its landform radius
(continental / highlands / basin 768, archipelago 192), because a 96-hex disc is the opening, not a
landform. Bands in parts per thousand; water as bodies / mean body / largest body, **rivers
excluded** so that `largest body` still means ocean; rivers as hexes / runs / longest run.

| preset      | water | shore | lowland | hills | highland | cliff |    water bodies |              rivers | purity |
| ----------- | ----: | ----: | ------: | ----: | -------: | ----: | --------------: | ------------------: | -----: |
| Continental |    30 |    61 |     302 |   354 |      238 |    12 |  504 / 19 / 446 |  45604 / 129 / 8801 |    991 |
| Archipelago |   110 |   190 |     317 |   142 |      202 |    35 | 253 / 48 / 2247 |                   — |    981 |
| Highlands   |    35 |     0 |     113 |   410 |      416 |    23 |     85 / 3 / 25 | 63110 / 144 / 17988 |    995 |
| Basin       |    45 |   198 |     524 |   211 |       16 |     2 | 717 / 58 / 7360 |  40103 / 98 / 19096 |    998 |

A hexagon is 1 m²; the walk is 3 m/s. Continental's 512-hex landform is a three-minute crossing,
basin's 960-hex one is six, and a river of eight to ten hexes is a real river. The opening inside
~80 hexes is still the cell-8 mix the bootstrap windows were tuned against, so the first minute does
not wait on a coast.

`basin` is the ocean preset: largest standing body 7,360 hexes and truncated, and the water nearest
a sand patch averages **2,494**. `archipelago` holds a 2,247-hex body inside a 192-hex sample.
`continental` on this seed is inland — no sand in 768 hexes, largest standing body 446 — which is a
continent, not a missing ocean; the coast is the walk. `highlands` still has no ocean worth the name
(largest body 25) and keeps `ocean_level` where those basins pass it, so the little sand it holds
(five patches, nearest 336 hexes) sits on the largest water it actually has.

**Purity is what decided the v0.21 site model.** Purity is the share of resource hexes whose
radius-1 disc holds one material, which is what decides whether an extractor works a field or
straddles two. Target 950; the shipped seed at radius 96, before and after v0.21:

| preset        | purity before | purity after | worst before | worst after              |
| ------------- | ------------: | -----------: | ------------ | ------------------------ |
| `continental` |           532 |          971 | stone 0      | sand 895                 |
| `archipelago` |           474 |          965 | wood 36      | sand 940                 |
| `highlands`   |           662 |          990 | crystal 28   | clay 977                 |
| `basin`       |           631 |          992 | crystal 71   | crystal 809 _(21 cells)_ |

Stone had **no workable patch anywhere in 26,307 land hexes** on `continental` before this — its
largest patch was 3 hexes against a base extractor's 7 — and neither did wood on `archipelago`.
Every preset now clears 19 hexes for iron, coal, copper and stone and 61 for forests.
`archipelago`'s landform scale moved from 4 to 5 and its blend from 45 to 52, because at the old
numbers no band held a contiguous run wider than a deposit and every disc came out a crescent.

## Browser frame — v0.24 hybrid renderer baseline

Recorded 2026-08-23 before Visual Depth renderer work, from the dirty-but-green v0.24 worktree at
base commit `34b68d5e3a3f9fcc9d5db50ebd1898b272f7f4de`. The only source changes present were the
intentional harvest work-before-yield change and the uncommitted Visual Depth planning documents;
`npm run quality` passed before the run. Raw report:
[`benchmarks/capacity-v0.24-browser.json`](benchmarks/capacity-v0.24-browser.json), SHA-256
`9B1A3D06BA566937848D875BD28AE7F2B99E4539E0A8C234609CEE3CEF7FCC98`.

Host: AMD Ryzen 7 5800X, Windows 11, Chromium 151, 16 hardware threads. The page was not
cross-origin isolated and both clocks reported a 100 µs step. The renderer viewport was the pinned
1440×900 at DPR 1 and the minimap was 178 px. Reproduce with `npm run bench:browser`, open
`/HexFactory/bench.html`, and press **Run full ladder**.

| tier   | entities | host frame µs | world µs | minimap µs | render µs | browser frame µs | frame share |
| ------ | -------: | ------------: | -------: | ---------: | --------: | ---------------: | ----------: |
| line   |       12 |          80.8 |    303.0 |        4.3 |     307.3 |            388.1 |        2.3% |
| small  |      192 |         111.0 |    227.3 |        5.3 |     232.6 |            343.6 |        2.1% |
| medium |      768 |         250.0 |    277.8 |        3.1 |     280.9 |            530.9 |        3.2% |
| wide   |    1,536 |         453.3 |    628.1 |        3.5 |     631.6 |          1,085.0 |        6.5% |
| large  |    3,072 |         835.0 |    600.0 |        3.7 |     603.7 |          1,438.7 |        8.6% |
| xlarge |    6,144 |       1,440.0 |  1,281.3 |        3.8 |   1,285.1 |          2,725.1 |       16.4% |

The complete xlarge browser frame used 16.4% of a 60 Hz frame on this desktop. All six applied
snapshots retained their full entity counts. This baseline supports only the renderer it measured;
it is the comparison row v0.25 must replace.

The shipped opening was also captured at 1440×900, 1366×768, and 390×844, and the Factory demo at
the same three viewports, under `docs/screenshots/`. At 1440×900 the real game canvas measured
1280×656 CSS/device pixels after its two desktop rails; at 1366×768 it measured 1366×704; at
390×844 it measured 390×786. Browser logs contained no warnings or errors during the captures.

Context loss was not recoverable inside the v0.24 renderer: both WebGL renderers prevent the
`webglcontextlost` default and set a permanent `lost` flag, but register no
`webglcontextrestored` handler. Drawing therefore stops until a page reload constructs fresh
contexts. Visual Depth must replace this source-inspected baseline with an exercised restore path.

## Browser frame — the current v0.43.0 audit

Recorded 2026-08-29 on the Ryzen 7 5800X / Windows 11 reference desktop in Chromium 151, at
1440×900, DPR 1 and a 178 px minimap. Each profile ran the complete six-tier ladder and every
applied snapshot retained its full entity count. Raw reports and SHA-256:

- [`benchmarks/capacity-v0.43-browser-low.json`](benchmarks/capacity-v0.43-browser-low.json) —
  `4BCF85F7C70B9DC09DE0D9A506DC11BDA33E3808B33DDC9463B0DE8739683BAD`
- [`benchmarks/capacity-v0.43-browser-medium.json`](benchmarks/capacity-v0.43-browser-medium.json) —
  `8F65194CF3A434796F1079F809DFFA1FF1B7A6655EC5081608D4593A72F3CFEC`
- [`benchmarks/capacity-v0.43-browser-high.json`](benchmarks/capacity-v0.43-browser-high.json) —
  `6C69DEEB4C6445C8182B3BA025C4CE382DB58599578040F6A8312CEBA4DC3D50`

| profile | tier   | entities | browser frame µs | frame share | draw calls | triangles | geometries | textures |
| ------- | ------ | -------: | ---------------: | ----------: | ---------: | --------: | ---------: | -------: |
| Low     | line   |       12 |            633.0 |        3.8% |         34 |    24,464 |         36 |        1 |
| Low     | small  |      192 |            827.3 |        5.0% |         35 |    96,232 |         36 |        1 |
| Low     | medium |      768 |          1,443.4 |        8.7% |         34 |   329,960 |         36 |        1 |
| Low     | wide   |    1,536 |          2,521.7 |       15.1% |         36 |   654,312 |         36 |        1 |
| Low     | large  |    3,072 |          3,198.2 |       19.2% |         34 | 1,268,744 |         36 |        1 |
| Low     | xlarge |    6,144 |          5,386.4 |       32.3% |         35 | 2,546,696 |         36 |        1 |
| Medium  | line   |       12 |            628.4 |        3.8% |         34 |    24,464 |         36 |        3 |
| Medium  | small  |      192 |            748.2 |        4.5% |         35 |    96,232 |         36 |        3 |
| Medium  | medium |      768 |          1,639.3 |        9.8% |         34 |   329,960 |         36 |        3 |
| Medium  | wide   |    1,536 |          2,012.4 |       12.1% |         36 |   654,312 |         36 |        3 |
| Medium  | large  |    3,072 |          2,978.0 |       17.9% |         34 | 1,268,744 |         36 |        3 |
| Medium  | xlarge |    6,144 |          5,576.5 |       33.5% |         35 | 2,546,696 |         36 |        3 |
| High    | line   |       12 |            845.4 |        5.1% |         34 |    24,464 |         36 |        3 |
| High    | small  |      192 |            641.7 |        3.9% |         35 |    96,232 |         36 |        3 |
| High    | medium |      768 |          1,104.8 |        6.6% |         34 |   329,960 |         36 |        3 |
| High    | wide   |    1,536 |          1,835.3 |       11.0% |         36 |   654,312 |         36 |        3 |
| High    | large  |    3,072 |          3,425.9 |       20.6% |         34 | 1,268,744 |         36 |        3 |
| High    | xlarge |    6,144 |          5,656.0 |       33.9% |         35 | 2,546,696 |         36 |        3 |

Every profile remains under the plan's 35% desktop ceiling, but the xlarge tier now leaves only
1.1–2.7 percentage points of margin. That is a pass, not abundant headroom: the browser clocks step
in 100 µs increments and this is one desktop. Phase 7 must add a stacked-floor/lift workload and
repeat the three profiles before it adds another permanent visual bucket.

Draw calls remain 34–36 from 12 through 6,144 entities, so instances rather than entity count still
own submission. The fixed vocabulary is larger than v0.25's 14–16 calls and 18 geometries: this
record has 36 geometries, one or three textures and 2.55 million triangles at xlarge. That is the
cost of the current shipped scene, not evidence that one particular release caused it; the older
record is not a same-build A/B. This browser API still provides no GPU-completion or GPU-memory
telemetry.

## Browser frame — v0.25 Visual Depth baseline

Recorded 2026-08-23 after the production Three.js cutover, on the same Ryzen 7 5800X / Windows 11 /
Chromium 151 desktop, at 1440×900, DPR 1, a 178 px minimap, and the same 100 µs browser clocks. Each
profile ran the complete six-tier ladder and every applied snapshot retained its full entity count.
Raw reports and SHA-256:

- [`benchmarks/capacity-v0.25-browser-low.json`](benchmarks/capacity-v0.25-browser-low.json) —
  `916D678270300D05E31738214A3CD6AD7829BA177E156A5E7777BD4C552DE190`
- [`benchmarks/capacity-v0.25-browser-medium.json`](benchmarks/capacity-v0.25-browser-medium.json) —
  `9B3A30AA7FBC6DD159605DABA57F61312BCE2FA18B2FA17280C2AD03023E14A1`
- [`benchmarks/capacity-v0.25-browser-high.json`](benchmarks/capacity-v0.25-browser-high.json) —
  `1465FDC37DE1B7539C500CBF1706476C0B867934C6267F311B389A9DB358BF3F`

| profile | tier   | entities | browser frame µs | frame share | draw calls | triangles | geometries | textures |
| ------- | ------ | -------: | ---------------: | ----------: | ---------: | --------: | ---------: | -------: |
| Low     | line   |       12 |            433.4 |        2.6% |         14 |    21,650 |         18 |        1 |
| Low     | small  |      192 |            430.3 |        2.6% |         15 |    54,598 |         18 |        1 |
| Low     | medium |      768 |            760.5 |        4.6% |         14 |   164,102 |         18 |        1 |
| Low     | wide   |    1,536 |          1,356.9 |        8.1% |         16 |   322,822 |         18 |        1 |
| Low     | large  |    3,072 |          2,042.4 |       12.3% |         14 |   605,990 |         18 |        1 |
| Low     | xlarge |    6,144 |          4,562.5 |       27.4% |         15 | 1,221,414 |         18 |        1 |
| Medium  | line   |       12 |            347.7 |        2.1% |         14 |    21,650 |         18 |        3 |
| Medium  | small  |      192 |            392.1 |        2.4% |         15 |    54,598 |         18 |        3 |
| Medium  | medium |      768 |            806.4 |        4.8% |         14 |   164,102 |         18 |        3 |
| Medium  | wide   |    1,536 |          1,183.6 |        7.1% |         16 |   322,822 |         18 |        3 |
| Medium  | large  |    3,072 |          2,539.6 |       15.2% |         14 |   605,990 |         18 |        3 |
| Medium  | xlarge |    6,144 |          4,191.2 |       25.1% |         15 | 1,221,414 |         18 |        3 |
| High    | line   |       12 |            386.1 |        2.3% |         14 |    21,650 |         18 |        3 |
| High    | small  |      192 |            488.4 |        2.9% |         15 |    54,598 |         18 |        3 |
| High    | medium |      768 |            854.9 |        5.1% |         14 |   164,102 |         18 |        3 |
| High    | wide   |    1,536 |          1,445.9 |        8.7% |         16 |   322,822 |         18 |        3 |
| High    | large  |    3,072 |          2,088.5 |       12.5% |         14 |   605,990 |         18 |        3 |
| High    | xlarge |    6,144 |          3,623.3 |       21.7% |         15 | 1,221,414 |         18 |        3 |

The desktop gate is green: every recorded profile stays below the plan's 35% ceiling at every tier.
The largest tier's render p95 was 2.6 ms Low, 2.2 ms Medium, and 2.1 ms High. The draw-call range is
14–16 from 12 through 6,144 entities, proving that visual buckets rather than building count own the
calls. JavaScript heap at the largest tier was 94.3 MiB Low, 101.2 MiB Medium, and 137.5 MiB High;
this browser API is not GPU-memory telemetry. Renderer memory stayed at 18 geometries and one or
three textures. Repeated New Game, Factory demo, and load transitions in the real game converged on
22 retained geometries and one texture after both scene vocabularies had been visited, rather than
growing per transition.

An interactive Low-profile run exercised walking, all six orbits, zoom extremes, panel changes, and
construction with a rolling 240-frame p95 of 0.6 ms (0.9 ms once immediately after restoring a
save). Forced context loss/restoration rebuilt and redrew a nonblank retained scene; background
benchmark work followed by returning to the game also redrew cleanly. Reduced motion, desktop,
laptop-size, narrow, and mobile layouts were exercised. PNG pixel checks found nonblank, varied
output at every required viewport, and browser logs contained no warnings or errors.

No qualifying physical Intel Iris Xe / AMD Vega-class-or-weaker laptop was available for this
historical record. That support target was later withdrawn on 2026-08-27; this baseline makes no
integrated-GPU support claim.

## Browser frame — historical v0.13.1 record

Same host, `factory-wasm` 0.13.0, recorded 2026-08-18. Raw report:
[`benchmarks/capacity-v0.13.1-browser.json`](benchmarks/capacity-v0.13.1-browser.json). Viewport
pinned at 1440×900 world, 178 px minimap, `BASE_HEX_SIZE` 22, `devicePixelRatio` 1. Chromium 151, 16
hardware threads, `performance.now` observed at a 100 µs step, page not cross-origin isolated.

| tier   | entities | host frame µs | world µs | minimap µs | render µs | browser frame µs | sim share | frame share |
| ------ | -------: | ------------: | -------: | ---------: | --------: | ---------------: | --------: | ----------: |
| line   |       12 |          60.0 |    254.4 |       15.4 |     269.9 |            329.9 |      0.4% |        2.0% |
| small  |      192 |         126.0 |    295.6 |       43.9 |     339.4 |            465.4 |      0.8% |        2.8% |
| medium |      768 |         349.0 |    418.7 |       53.3 |     472.1 |            821.1 |      2.1% |        4.9% |
| wide   |    1,536 |         561.7 |    495.1 |       76.9 |     572.0 |          1,133.7 |      3.4% |        6.8% |
| large  |    3,072 |       1,025.0 |    648.4 |      120.5 |     768.9 |          1,793.9 |      6.1% |       10.8% |
| xlarge |    6,144 |       1,980.0 |    990.5 |      200.0 |   1,190.5 |          3,170.5 |     11.9% |       19.0% |

**1. A complete browser frame at the largest measured tier was 19.0% of 60 Hz** on the Canvas 2D
renderer, against 18.2% before Stage B and the first Stage C motion pass. That is a 9% world-draw
increase for the whole art generator.

**2. The environment is the floor, not the entity count.** Twelve entities cost 254 µs of world
draw; 6,144 cost 991 µs. Fringes, baked tiles, and silhouettes ride on the listed terrain and entity
sets, not on a walk of every surveyed hex — a first version that did walk every surveyed hex
measured about 8 ms and was refused.

**3. The world canvas was the render cost; the minimap was not**, at 6–15% of `render` at every
tier.

**4. Neither record locates a ceiling.** What the ladder says about the limit is only that a
complete frame at 6,144 entities fits.

### Capacity tiers against a 16,667 µs frame

`sim share` is the cost of advancing a tick and merging the result. `frame share` is the complete
browser frame, render included, and exists only from v0.12.4.

| tier   | entities | sim share v0.8 | sim share v0.12.2 | frame share v0.12.4 | frame share v0.13.1 | verdict     |
| ------ | -------: | -------------: | ----------------: | ------------------: | ------------------: | ----------- |
| line   |       12 |           0.6% |              0.4% |                2.2% |                2.0% | comfortable |
| small  |      192 |           2.5% |              0.8% |                3.3% |                2.8% | comfortable |
| medium |      768 |           8.3% |              1.9% |                5.8% |                4.9% | comfortable |
| wide   |    1,536 |          15.8% |              3.3% |                6.4% |                6.8% | comfortable |
| large  |    3,072 |          30.1% |              6.0% |               11.6% |               10.8% | comfortable |
| xlarge |    6,144 |          62.1% |             11.0% |               18.2% |               19.0% | comfortable |

## Record history

Each row is a full report in `docs/benchmarks/`. The headline is what the run was for.

- v0.43.0 (both, 2026-08-29) — **Current native and browser records.** The 6,144-entity native
  frame was 1,372 µs. Complete browser frames were 5,386 µs Low, 5,577 µs Medium and 5,656 µs
  High: all pass the 35% desktop gate, with only 1.1–2.7 percentage points left. Draw calls remained
  34–36 across the ladder. This is a current-build audit, not an A/B against v0.25.
- v0.25 (browser, 2026-08-23) — **Visual Depth baseline.** Low, Medium, and High each
  completed the six-tier Three.js ladder. The 6,144-entity browser frame was 4,562 µs Low,
  4,191 µs Medium, and 3,623 µs High on this desktop; all stayed below 35% of 60 Hz. Draw calls
  remained 14–16 and geometry counts remained 18.
- v0.24 (browser, 2026-08-23) — **Pre-Visual-Depth browser baseline.** The hybrid
  instanced-WebGL2/Canvas world completed the 6,144-entity tier in 2,725 µs, 16.4% of 60 Hz, with
  1,281 µs in the world draw. Six viewport comparison captures record the shipped opening and demo.
- v0.21 (native, 2026-08-20) — **Current generation record.** Flat against v0.16, which is the point:
  the ladder never generates, and a milestone that rewrote generation had to leave it untouched.
  Generation itself moved 0.52 → 1.42 µs/hex, both re-measured on this host so the comparison is
  against the same harness rather than against the v0.16 line.
- v0.16 (native, 2026-08-18) — Flat against v0.13; first measurement of generation at 0.35 µs/hex,
  on a survey that has since grown patch, river, and bootstrap reporting.
- v0.15 (browser, 2026-08-18) — An A/B, **not a comparable record** — different browser and a
  non-compositing pane. World draw 1,767 µs for the shape grammar against 2,057 µs for the `switch`
  it replaced, ranges overlapping: no regression detectable, not demonstrably faster. The minimap,
  which never imports the grammar, held at 428 µs ± 2.6% across all five runs, which is what makes
  the A/B trustworthy. All six tier checksums identical — the presentation-only claim demonstrated
  rather than asserted.
- v0.13.1 (browser, 2026-08-18) — Historical simulation-half record; Stage B and first Stage C
  motion fit in the measured headroom.
- v0.13 (native, 2026-08-18) — Tick about 1.4× v0.12.2 at the largest tier, from refreshing supply
  and demand once per tick. An all-pairs compile over every powered machine was caught here — it
  made xlarge compile 61× slower — and replaced by pole-to-pole plus machine-to-pole before the
  record was taken.
- v0.12.4 (browser, 2026-08-18) — The first complete browser frame: 18.2% of 60 Hz at 6,144
  entities, render 1,069 µs of it. The unknown 89% of a frame is gone, and Stage C stopped being
  gated on ignorance.
- v0.12.2 (both, 2026-08-18) — **Current payload record.** Payload 13.6× smaller, boundary 21.7×
  cheaper (6,085 → 280 µs), host frame from 62.1% of 60 Hz to 11.0%. The 10 µs/KB law is replaced by
  a fixed round-trip floor of ~62 µs: below `wide`, sending less now buys almost nothing.
- v0.12.1 (native, 2026-08-17) — No new regression; the generator bump changed which cells exist,
  not how the ladder is built.
- v0.12 (native, 2026-08-17) — The checksum got 3.0× cheaper — v0.11's sparse tile overlay, measured
  for the first time, taking this workload from 25,024 stored tiles to 512. Found and fixed two
  v0.12 regressions before recording: 86 KB of delta spent publishing `fuel_charge` on belts, and a
  `field_at` that scanned the scenario's resource list per hex (snapshot 7,480 µs → 2,220 µs).
- v0.8 (both, 2026-08-17) — The first browser tiers, and they moved the roadmap. Wasm costs
  1.19–1.23× native at the four largest tiers, so three releases of native work transferred intact.
  The worker boundary was 57–61% of a host frame at a flat 9–13 µs/KB — the finding the binary delta
  encoding was built from.
- v0.7 (native, —) — Sparse snapshot: frame 16.8× cheaper at the largest tier.
- v0.6 (native, —) — Sparse cost: tick 233× cheaper, delta payload 2.3× smaller.
- v0.5.1 (native, —) — The first recorded ladder.

## Limits of this measurement

- **One browser, one shell, one machine.** No Firefox, Safari, mobile, or low-core-count figure
  exists, and a phone is not represented by anything here. One Chromium version on one desktop is
  the whole browser evidence.
- **One workload shape.** Uniform straight lines with an always-accepting sink. It does not cover
  backpressure-saturated networks, long turning belt runs, dense multi-cell packing, or deposits
  running dry.
- **Dirty tracking is measured near its worst case.** Roughly 43% of this workload's entities change
  every tick, so a quiet blueprint sends proportionally less.
- **One run per tier, no confidence interval.** Treat differences under roughly 20% as noise. Three
  full browser runs at v0.12.2 put the largest tier's host frame at ±7%, which supports that rule
  rather than replacing it. The `line` tier's compile, recompile, and edit figures move by more than
  20% between runs and should not be read closely.
- **The camera follows the player** in the browser record. Off-screen entities are walked and
  clipped, not drawn. A zoomed-out view, or a 2× device pixel ratio, is a different measurement.
- **`apply` is the coarsest number here**, running against a 100 µs clock step; treat it as ±10%.
- **The round trip and the merge are timed in separate passes.** The game interleaves them; their
  sum is a fair account of the work, but the cache behaviour of the interleaving is not represented.
- **The release profile is tuned for wasm size** (`opt-level = "s"`, LTO, `wasm-opt -Oz`), matching
  what ships. A speed-tuned build would be faster and would not represent the artifact.
- **Timings include allocation.** No allocator was pinned or replaced.
- **The ladder no longer brackets a ceiling.** Every tier fits inside a 60 Hz frame. Treat "above
  6,144 entities" as the whole of what it says about the limit.

## Live follow-ups, in the order the measurement supports

1. **Add Phase 7's workload before adding Phase 7's visual cost.** Stack visible/faded floor cells,
   active lifts and cross-level graph edges into a deterministic tier. Repeat all three desktop
   profiles before shipping: the current xlarge profile leaves only 1.1–2.7 percentage points under
   the 35% gate, and the present straight-line factory contains no upper floor to price.
2. **Measure the scheduler under the shapes the game now owns.** Add junction-dense,
   backpressured and power-dense tiers. Source inspection finds bounded but recurring ordered-map
   and vector construction in power allocation, and runtime orders are still visited every tick;
   the straight always-accepting line cannot say whether active sets or reused scratch would pay.
3. **Measure the site cache under a walk rather than under a survey.** The generation record sweeps
   a disc in lattice order, which is the cache's best case. A walking player crosses cells in a worse
   one, and `generate_chunk` is the path that pays for it. Nothing here resolves that today.
4. **Batch the transport recompile inside a construction drag.** A 32-cell run recompiles 32 times,
   once on pointer release rather than per frame. No tier measures it, so it is a known cost and not
   yet a measured one.
5. **Re-examine incremental transport recompilation.** At xlarge the current affected-component
   path costs 4.05 ms against 1.91 ms for a full compile. Do not remove it on that alone: the tier is
   one connected line, while its tested behaviour under local component splits and merges is a
   correctness asset. Add affected-size reporting first.
6. **Measure `apply` with a finer clock before optimizing it.** The current 120–185 µs xlarge means
   are only one or two 100 µs browser steps per sample.
7. **Extend the ladder past 6,144 only after the Phase 7 workload exists.** The present ladder is
   close to the support gate but still does not locate a technical ceiling; more copies of the old
   shape would answer a less urgent question than the first stacked factory.
8. **An incremental checksum** remains last. It is determinism-critical work for a small share of
   the current native frame; optimize it only if a later record moves that share materially.

Record a new run by adding a dated report under `docs/benchmarks/` and updating the tables above.
**Checksum comparisons are valid only while the pinned workload checksum in the Rust test gate is
unchanged; timing comparisons survive a checksum change as long as the workload's shape, entity
counts, and delivered totals do not move.**
