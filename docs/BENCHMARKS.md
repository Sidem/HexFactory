# HexFactory capacity benchmarks

Capacity is measured, never asserted, and the measurement orders the work. Every number here was
produced by the committed harness; the raw reports live in `docs/benchmarks/` and are the source for
any table that was trimmed out of this document.

**Current records.** Native: **Landforms and Fields v0.21**. Browser frame: **Look Systems v0.13.1**.
Generation: **v0.21**. Payload: **Binary Delta v0.12.2**.

**Two caveats travel with those records.**

- **The browser frame record describes a renderer the game no longer ships.** The unversioned WebGL2
  pass (2026-08-20) replaced both Canvas 2D draws that v0.13.1 timed. The simulation half of that
  record still holds; the `world`, `minimap`, `render`, and `browser frame` columns do not, and no
  frame claim may be made until the ladder is re-run.
- **Every tier checksum below is historical.** v0.18, v0.19, v0.20, v0.21, and the world-scale
  pass each moved the pinned workload checksum: `2402899979` → `1679299541` → `914129621` →
  `780276626` → `325426962` → `3745973835`. The first three added saved, checksummed state; v0.21
  and the scale pass moved `WORLD_GENERATOR_VERSION`, which the checksum reads. None of them
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

## Native — the current record (v0.21)

Host: AMD Ryzen 7 5800X (8 cores / 16 threads), Windows 11 Pro 10.0.26200, rustc 1.87.0,
`factory-wasm` built with the shipped release profile (`opt-level = "s"`, LTO, `wasm-opt -Oz`).
Recorded 2026-08-20. Raw report:
[`benchmarks/capacity-v0.21-native.json`](benchmarks/capacity-v0.21-native.json).

| tier   | entities | tick µs | snapshot µs | checksum µs | frame µs | compile µs | recompile µs |
| ------ | -------: | ------: | ----------: | ----------: | -------: | ---------: | -----------: |
| line   |       12 |     0.6 |        12.9 |         1.7 |      6.2 |        2.3 |          6.3 |
| small  |      192 |     8.7 |        63.7 |        12.0 |     83.9 |       32.8 |         93.3 |
| medium |      768 |    41.3 |       360.0 |        47.5 |    188.3 |      140.6 |        385.2 |
| wide   |    1,536 |    76.0 |       613.5 |        92.5 |    382.0 |      307.4 |        734.9 |
| large  |    3,072 |   168.3 |     1,172.1 |       177.7 |    797.4 |      644.9 |      1,426.0 |
| xlarge |    6,144 |   361.2 |     2,504.1 |       376.0 |  1,541.3 |    1,331.0 |      2,786.3 |

Every tier is within noise of the v0.16 record. **The noise floor is stated rather than hidden**:
the v0.16 ladder was run twice on its build and xlarge gave tick 359.7 / frame 1,439.5 on one run
against 378.1 / 1,416.8 on the other, so a 5% swing between runs is what this host resolves and
nothing smaller is a finding. xlarge tick moved 378.1 → 361.2 and frame 1,416.8 → 1,541.3, both
inside that band. The `line` tier's absolute numbers are microseconds and are dominated by timer
resolution; read the larger tiers.

What this deliberately does **not** say is anything about the v0.21 generator. The ladder's scenario
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

## Browser frame — the current record (v0.13.1)

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

- v0.21 (native, 2026-08-20) — **Current native record.** Flat against v0.16, which is the point:
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
- v0.13.1 (browser, 2026-08-18) — **Current browser record** (simulation half only; the renderer has
  since changed). Stage B and first Stage C motion fit in the measured headroom.
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

1. **Re-measure the browser ladder against the WebGL2 renderer.** It replaced both draws the current
   record times, so there is currently no supported claim about a complete frame. Highest priority
   of anything on this list, because the others are read against it.
2. **Extend the ladder past 6,144 entities**, so the record brackets a ceiling again instead of only
   showing headroom.
3. **Measure the site cache under a walk rather than under a survey.** The generation record sweeps
   a disc in lattice order, which is the cache's best case. A walking player crosses cells in a worse
   one, and `generate_chunk` is the path that pays for it. Nothing here resolves that today.
4. **Measure `apply` properly.** The main-thread merge did not grow — everything around it shrank,
   taking it from 0.7–1.5% of a host frame to 6.3%. At 115 µs against a 100 µs clock step, what it
   needs first is a measurement that can resolve it, not an optimization.
5. **Batch the transport recompile inside a construction drag.** A 32-cell run recompiles 32 times,
   once on pointer release rather than per frame. No tier measures it, so it is a known cost and not
   yet a measured one.
6. **Re-examine incremental transport recompilation.** Both records show the incremental path
   costing about three times a full compile. Do not remove it on that alone: its tested behaviour
   under component splits and merges is a correctness asset.
7. **An incremental checksum** sits at the bottom. v0.11's sparse overlay already took 3.0× off it;
   it is about a tenth of the in-wasm frame. Determinism-critical work for a small and shrinking
   share is the wrong thing to attack.

Record a new run by adding a dated report under `docs/benchmarks/` and updating the tables above.
**Checksum comparisons are valid only while the pinned workload checksum in the Rust test gate is
unchanged; timing comparisons survive a checksum change as long as the workload's shape, entity
counts, and delivered totals do not move.**
