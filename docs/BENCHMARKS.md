# HexFactory capacity benchmarks

Status: Capacity Tiers v0.5.1 is the first measured record for this project. The roadmap gates finer
native dirty tracking, any renderer decision, and every scale claim behind this measurement. Nothing
here is an extrapolation: each number below was produced by the committed harness, and the raw
report is stored beside this document.

Run it with:

```bash
npm run bench
```

`npm run bench -- --quick` runs a reduced ladder, and `npm run bench -- --json <path>` writes the
machine-readable report. The benchmark is deliberately outside `npm run quality`: shared CI runners
do not produce comparable timings. The test gate instead pins the workload's checksum and asserts
the harness still runs, so recorded numbers cannot silently stop being comparable.

## What is measured

Six tiers of the same shape, differing only in how many production lines run at once. One line is:

`extractor → 6 belts → composer → belt → container → belt → consumer`

Every line sits on its own effectively inexhaustible deposit, three rows away from its neighbours,
and runs east. The consumer always accepts, so a tier reaches a steady state and stays there: the
extractor cadence of 5 and the two-ore, eight-tick recipe make each line deliver one component every
ten ticks for the whole measured run. Tiers therefore differ in size, not in behaviour.

Each tier is measured in separately timed phases. Every phase that would otherwise disturb another
runs on its own freshly warmed core, so no measurement inherits a state the previous one left:

| Metric        | What it times                                                               |
| ------------- | --------------------------------------------------------------------------- |
| `tick`        | one simulation tick, with no snapshot and no serialization                  |
| `snapshot`    | building one complete native snapshot, before serialization                 |
| `frame`       | one worker frame: bounded command batch, one tick, and one serialized delta |
| `delta bytes` | the serialized delta payload that frame sends across the worker boundary    |
| `compile`     | one full deterministic transport compile, as used on load and restore       |
| `recompile`   | the incremental transport machinery alone, for one edit                     |
| `edit`        | one complete public rotate edit, legality checks included                   |

`recompile` and `compile` are directly comparable — the incremental path is timed without the edit
path's legality work, so the comparison is not confounded by it.

## Recorded results

Host: AMD Ryzen 7 5800X (8 cores / 16 threads), Windows 11 Pro 10.0.26200, rustc 1.87.0, `factory-wasm`
0.5.0 built with the shipped release profile. Recorded 2026-08-16. Raw report:
[`benchmarks/capacity-v0.5.1.json`](benchmarks/capacity-v0.5.1.json).

| tier   | lines | entities |  tiles |  tick µs | snapshot µs |  frame µs | delta bytes | compile µs | recompile µs | edit µs |
| ------ | ----: | -------: | -----: | -------: | ----------: | --------: | ----------: | ---------: | -----------: | ------: |
| line   |     1 |       12 |    576 |      2.2 |        23.4 |      35.4 |       2,931 |        1.1 |          5.5 |     5.8 |
| small  |    16 |      192 |  1,216 |     63.5 |       130.4 |     378.6 |      46,196 |       16.0 |         57.7 |    71.5 |
| medium |    64 |      768 |  3,520 |    698.9 |     1,000.1 |   2,347.2 |     186,188 |       81.2 |        267.4 |   313.6 |
| wide   |   128 |    1,536 |  6,592 |  2,772.6 |     3,470.7 |   7,434.2 |     374,503 |      197.2 |        538.2 |   585.6 |
| large  |   256 |    3,072 | 12,736 | 13,304.1 |    15,425.8 |  31,646.0 |     751,996 |      380.5 |      1,145.4 | 1,329.8 |
| xlarge |   512 |    6,144 | 25,024 | 55,200.5 |    61,702.0 | 125,316.0 |   1,511,529 |      832.1 |      2,527.1 | 2,746.6 |

## Measured capacity tiers

Against a 16,667 µs frame at 60 Hz, using the measured `frame` cost:

| tier   | entities | share of a 60 Hz frame | verdict                            |
| ------ | -------: | ---------------------: | ---------------------------------- |
| line   |       12 |                   0.2% | comfortable                        |
| small  |      192 |                   2.3% | comfortable                        |
| medium |      768 |                  14.1% | comfortable                        |
| wide   |    1,536 |                  44.6% | sustained, roughly half the budget |
| large  |    3,072 |                   190% | misses 60 Hz; fits 30 Hz at 95%    |
| xlarge |    6,144 |                   752% | misses both                        |

**The measured native ceiling for a 60 Hz frame is between 1,536 and 3,072 entities**, and between
3,072 and 6,144 entities for 30 Hz. The shipped game asks a player to build far below the first
tier that struggles, so this is a headroom record, not a live defect.

These are native host figures. They are not browser figures, and no browser number is claimed here —
see the limits below.

## What the numbers say

**1. Tick cost is governed by extractors times generated tiles, not by entity count.** Dividing
`tick` by entity count spans a factor of 49 across the ladder (0.18 µs to 8.98 µs per entity), so
entity count does not explain it. Dividing by extractors × tiles instead stays within a factor of
1.4 (0.0031 to 0.0043 µs) across a 22,000-fold range of that product. Each extractor's per-tick
deposit lookup scans every generated tile, which makes the running simulation quadratic in world
size even where nothing has changed. This is the largest single capacity limiter measured.

**2. The snapshot delta is not sparse.** Delta payload per entity is 240–246 bytes at every tier, a
constant across the full 512-fold range. The delta omits unchanged groups, but any change to any
building resends the whole buildings array, and in a running factory something always changes. At
the `wide` tier this is 374 KB per frame — about 22 MB/s across the worker boundary at 60 Hz. The
revision-checked delta transport shipped in v0.5 is doing what it was specified to do; the
group-level granularity is simply too coarse to help a running blueprint.

**3. Incremental transport recompilation costs about three times a full compile.** The ratio is
4.8, 3.6, 3.3, 2.7, 3.0, and 3.0 across the six tiers — near-constant over a 512-fold size range.
A per-edit advantage that depended only on the affected set would vary with tier; a constant ratio
points at fixed per-edit work instead. The incremental path rebuilds several whole-blueprint maps
per edit (stable-ID links, occupancy, indices, anchors, and the old adjacency) where a full compile
builds occupancy once. Its correctness properties are pinned by tests and are not in question, but
on this workload it does not currently pay for itself.

**4. Legality checking is not the expensive part of an edit.** `edit` exceeds `recompile` by
0.3–220 µs, a minority of edit cost at every tier. Optimizing placement legality would not move the
edit budget; the transport machinery is where the time goes.

## Limits of this measurement

- **Native, not browser.** These runs are native host builds. The shipped artifact is wasm in a
  worker, where absolute costs differ. The tiers are a relative capacity record and a baseline for
  regression, not a prediction of browser frame rate. Browser-side measurement is the named
  follow-up.
- **One machine, one run per tier.** No repetition, variance, or confidence interval is recorded.
  Treat differences under roughly 20% as noise.
- **The release profile is tuned for wasm size** (`opt-level = "s"`, LTO), matching what ships.
  A speed-tuned native build would be faster and would not represent the artifact.
- **One workload shape.** Uniform straight lines with an always-accepting sink. It does not cover
  backpressure-saturated networks, long turning belt runs, dense multi-cell packing, or deposits
  running dry. Findings 1 and 2 are properties of the core loop and generalize; finding 3 is stated
  for this workload only. Its rotations deliberately include orientations that merge and split
  neighbouring components, which is demanding for the incremental path.
- **Timings include allocation.** No allocator was pinned or replaced.

## Follow-ups, in the order the measurement supports

1. Give extractors a resolved deposit reference instead of a per-tick spatial scan over all tiles.
   Finding 1 makes this the highest-value change, and it is a native-core change with no host or
   save-format consequence.
2. Make the buildings delta per-entity rather than per-group. Finding 2 shows group-level dirty
   tracking cannot help a running factory.
3. Re-examine whether incremental transport recompilation should keep persistent structures across
   edits, or whether the full compile is simply the better default at these sizes. Finding 3 is the
   evidence; do not remove the incremental path on it alone, because its tested behaviour under
   component splits and merges is a correctness asset.
4. Measure the same ladder in the browser worker, so a wasm capacity tier exists next to this
   native one.
5. Only after 1 and 2: revisit the renderer. The measurement does not currently implicate rendering.

Record new runs by adding a dated report under `docs/benchmarks/` and updating the table above.
Comparisons are only valid while the pinned workload checksum in the Rust test gate is unchanged.
