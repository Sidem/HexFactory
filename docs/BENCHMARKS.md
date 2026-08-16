# HexFactory capacity benchmarks

Status: Sparse Cost v0.6 is the second measured record. The roadmap gates any renderer decision and
every scale claim behind this measurement. Nothing here is an extrapolation: each number below was
produced by the committed harness, and the raw reports are stored beside this document.

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

Host: AMD Ryzen 7 5800X (8 cores / 16 threads), Windows 11 Pro 10.0.26200, rustc 1.87.0,
`factory-wasm` 0.6.0 built with the shipped release profile. Recorded 2026-08-16. Raw report:
[`benchmarks/capacity-v0.6.json`](benchmarks/capacity-v0.6.json).

| tier   | lines | entities |  tiles | tick µs | snapshot µs | frame µs | delta bytes | compile µs | recompile µs | edit µs |
| ------ | ----: | -------: | -----: | ------: | ----------: | -------: | ----------: | ---------: | -----------: | ------: |
| line   |     1 |       12 |    576 |     0.5 |        22.0 |     28.5 |       1,316 |        1.0 |          6.2 |     6.5 |
| small  |    16 |      192 |  1,216 |     5.1 |       136.0 |    246.9 |      19,742 |       15.3 |         52.5 |    60.1 |
| medium |    64 |      768 |  3,520 |    21.0 |       988.5 |  1,433.1 |      79,398 |       80.6 |        259.8 |   274.1 |
| wide   |   128 |    1,536 |  6,592 |    46.2 |     3,397.4 |  4,380.5 |     159,553 |      175.6 |        596.5 |   626.7 |
| large  |   256 |    3,072 | 12,736 |   103.9 |    13,210.7 | 14,511.8 |     320,444 |      355.3 |      1,111.5 | 1,119.9 |
| xlarge |   512 |    6,144 | 25,024 |   236.6 |    56,083.4 | 64,881.8 |     644,142 |      864.8 |      2,648.1 | 2,513.3 |

Every tier reproduces the v0.5.1 checksum and delivered total, so the two records measure the same
workload and can be compared directly. Against the previous record:

| tier   | tick before | tick after | factor | frame before | frame after | delta bytes before | delta bytes after |
| ------ | ----------: | ---------: | -----: | -----------: | ----------: | -----------------: | ----------------: |
| line   |         2.2 |        0.5 |   4.4× |         35.4 |        28.5 |              2,931 |             1,316 |
| small  |        63.5 |        5.1 |  12.5× |        378.6 |       246.9 |             46,196 |            19,742 |
| medium |       698.9 |       21.0 |  33.3× |      2,347.2 |     1,433.1 |            186,188 |            79,398 |
| wide   |     2,772.6 |       46.2 |  60.0× |      7,434.2 |     4,380.5 |            374,503 |           159,553 |
| large  |    13,304.1 |      103.9 | 128.0× |     31,646.0 |    14,511.8 |            751,996 |           320,444 |
| xlarge |    55,200.5 |      236.6 | 233.3× |    125,316.0 |    64,881.8 |          1,511,529 |           644,142 |

## Measured capacity tiers

Against a 16,667 µs frame at 60 Hz, using the measured `frame` cost:

| tier   | entities | share of a 60 Hz frame | verdict                              |
| ------ | -------: | ---------------------: | ------------------------------------ |
| line   |       12 |                   0.2% | comfortable                          |
| small  |      192 |                   1.5% | comfortable                          |
| medium |      768 |                   8.6% | comfortable                          |
| wide   |    1,536 |                  26.3% | comfortable                          |
| large  |    3,072 |                  87.1% | sustained, with little headroom left |
| xlarge |    6,144 |                   389% | misses 60 Hz and 30 Hz (195%)        |

**The measured native ceiling for a 60 Hz frame is now between 3,072 and 6,144 entities**, up from
between 1,536 and 3,072 in v0.5.1. The 30 Hz ceiling sits in the same interval, because the tier
that fails 60 Hz also fails 30 Hz. The shipped game still asks a player to build far below the first
tier that struggles, so this remains a headroom record, not a live defect.

These are native host figures. They are not browser figures, and no browser number is claimed here —
see the limits below.

## What the numbers say

**1. Tick cost is now linear in entity count, and no longer touches world size.** Dividing `tick` by
entity count gives 0.042, 0.027, 0.027, 0.030, 0.034, and 0.039 µs across the ladder — a factor of
1.6, against the factor of 49 recorded in v0.5.1. Over the same range the generated tile count grows
43-fold and no longer appears in the result. Extractors resolve their deposit once, from a cached
candidate list invalidated when new tiles appear, instead of scanning every tile every tick. Tick is
now 0.4% of a frame at the largest tier; it was 44%.

**2. The buildings delta is 2.3× smaller, and what remains is the workload's real change rate.**
Payload per entity fell from 240–246 bytes to 103–110 bytes, a near-identical ratio across the full
512-fold range. The delta now carries only entities that actually changed, so the residual
constant is a property of this workload — roughly 43% of its entities change on any given tick,
because every line holds an extractor and a composer whose progress advances every tick. A quieter
blueprint sends proportionally less; the previous group-level delta sent everything regardless.

**3. The frame is now dominated by materializing the snapshot, not by simulating it.** `snapshot` is
55–91% of `frame` at every tier and 86% at the largest. Rust still builds one complete snapshot per
frame purely to diff it against the previous one, and the diff then discards most of that work. This
is the largest remaining cost in the measured frame, and it is the natural successor to finding 2:
the delta is now sparse on the wire but is still computed densely.

**4. Incremental transport recompilation still costs about three times a full compile.** The ratios
are 6.2, 3.4, 3.2, 3.4, 3.1, and 3.1 across the six tiers, effectively unchanged from v0.5.1. This
release did not touch the transport path, and the v0.5.1 reading stands: the incremental path
rebuilds several whole-blueprint maps per edit where a full compile builds occupancy once.

**5. Legality checking is still not the expensive part of an edit.** `edit` and `recompile` are
within 15% of each other at every tier — inside this harness's noise floor — so the transport
machinery, not placement legality, is where edit time goes.

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
  running dry. Findings 1 and 3 are properties of the core loop and generalize. Finding 2's ratio is
  specific to this workload's change rate, and finding 4 is stated for this workload only. Its
  rotations deliberately include orientations that merge and split neighbouring components, which is
  demanding for the incremental path.
- **Deposit references are measured with a static tile set.** No chunk is generated mid-run in this
  workload, so the cost of re-resolving deposits after generation is not represented. That path is
  correctness-tested but unmeasured, and it runs at most once per generated chunk.
- **Timings include allocation.** No allocator was pinned or replaced.

## Follow-ups, in the order the measurement supports

1. Stop materializing a complete snapshot every frame. Finding 3 makes this the highest-value
   remaining change: track dirty entities at mutation time and build the delta from that set, so the
   frame stops paying for a full snapshot it immediately discards.
2. Measure the same ladder in the browser worker, so a wasm capacity tier exists next to this native
   one. Nothing here predicts browser frame rate.
3. Re-examine whether incremental transport recompilation should keep persistent structures across
   edits, or whether the full compile is simply the better default at these sizes. Finding 4 is the
   evidence; do not remove the incremental path on it alone, because its tested behaviour under
   component splits and merges is a correctness asset.
4. Only after 1: revisit the renderer. The measurement still does not implicate rendering.

Record new runs by adding a dated report under `docs/benchmarks/` and updating the table above.
Comparisons are only valid while the pinned workload checksum in the Rust test gate is unchanged.

Previous record: [`benchmarks/capacity-v0.5.1.json`](benchmarks/capacity-v0.5.1.json).
