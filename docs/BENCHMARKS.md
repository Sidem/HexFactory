# HexFactory capacity benchmarks

Status: Sparse Snapshot v0.7 is the third measured record. The roadmap gates any renderer decision
and every scale claim behind this measurement. Nothing here is an extrapolation: each number below
was produced by the committed harness, and the raw reports are stored beside this document.

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
| `checksum`    | one native checksum, which every delta carries                              |
| `frame`       | one worker frame: bounded command batch, one tick, and one serialized delta |
| `delta bytes` | the serialized delta payload that frame sends across the worker boundary    |
| `compile`     | one full deterministic transport compile, as used on load and restore       |
| `recompile`   | the incremental transport machinery alone, for one edit                     |
| `edit`        | one complete public rotate edit, legality checks included                   |

`recompile` and `compile` are directly comparable — the incremental path is timed without the edit
path's legality work, so the comparison is not confounded by it.

`snapshot` is no longer part of a frame. As of v0.7 the complete snapshot is built only for the
host's first frame, and it is kept in the ladder as the baseline the incremental delta is measured
against. `checksum` is new in this record (report schema 2), because the frame it still sits inside
is now small enough that it matters.

## Recorded results

Host: AMD Ryzen 7 5800X (8 cores / 16 threads), Windows 11 Pro 10.0.26200, rustc 1.87.0,
`factory-wasm` 0.7.0 built with the shipped release profile. Recorded 2026-08-16. Raw report:
[`benchmarks/capacity-v0.7.json`](benchmarks/capacity-v0.7.json).

| tier   | lines | entities |  tiles | tick µs | snapshot µs | checksum µs | frame µs | delta bytes | compile µs | recompile µs | edit µs |
| ------ | ----: | -------: | -----: | ------: | ----------: | ----------: | -------: | ----------: | ---------: | -----------: | ------: |
| line   |     1 |       12 |    576 |     0.5 |        22.8 |        17.0 |     27.5 |       1,318 |        2.2 |         12.1 |    11.7 |
| small  |    16 |      192 |  1,216 |     6.0 |        74.7 |        45.0 |    122.8 |      19,745 |       17.8 |         57.8 |    67.1 |
| medium |    64 |      768 |  3,520 |    24.7 |       291.8 |       136.0 |    448.8 |      79,400 |       85.6 |        252.7 |   281.3 |
| wide   |   128 |    1,536 |  6,592 |    49.9 |       540.6 |       266.7 |    964.9 |     159,555 |      199.3 |        607.6 |   760.4 |
| large  |   256 |    3,072 | 12,736 |   107.7 |     1,120.7 |       518.0 |  1,828.5 |     320,447 |      469.3 |      1,198.9 | 1,314.5 |
| xlarge |   512 |    6,144 | 25,024 |   263.9 |     2,093.0 |     1,043.5 |  3,852.2 |     644,144 |      798.5 |      2,405.2 | 2,444.1 |

Every tier reproduces the v0.6 checksum and delivered total, so the two records measure the same
workload and can be compared directly. Against the previous record:

| tier   | frame before | frame after | factor | snapshot before | snapshot after | factor | delta bytes before | delta bytes after |
| ------ | -----------: | ----------: | -----: | --------------: | -------------: | -----: | -----------------: | ----------------: |
| line   |         28.5 |        27.5 |   1.0× |            22.0 |           22.8 |   1.0× |              1,316 |             1,318 |
| small  |        246.9 |       122.8 |   2.0× |           136.0 |           74.7 |   1.8× |             19,742 |            19,745 |
| medium |      1,433.1 |       448.8 |   3.2× |           988.5 |          291.8 |   3.4× |             79,398 |            79,400 |
| wide   |      4,380.5 |       964.9 |   4.5× |         3,397.4 |          540.6 |   6.3× |            159,553 |           159,555 |
| large  |     14,511.8 |     1,828.5 |   7.9× |        13,210.7 |        1,120.7 |  11.8× |            320,444 |           320,447 |
| xlarge |     64,881.8 |     3,852.2 |  16.8× |        56,083.4 |        2,093.0 |  26.8× |            644,142 |           644,144 |

## Measured capacity tiers

Against a 16,667 µs frame at 60 Hz, using the measured `frame` cost:

| tier   | entities | share of a 60 Hz frame | verdict     |
| ------ | -------: | ---------------------: | ----------- |
| line   |       12 |                   0.2% | comfortable |
| small  |      192 |                   0.7% | comfortable |
| medium |      768 |                   2.7% | comfortable |
| wide   |    1,536 |                   5.8% | comfortable |
| large  |    3,072 |                  11.0% | comfortable |
| xlarge |    6,144 |                  23.1% | comfortable |

**The recorded ladder no longer contains a tier that misses 60 Hz.** In v0.6 the ceiling sat between
3,072 and 6,144 entities; the largest tier now uses under a quarter of a 60 Hz frame. This
measurement therefore does not locate the native ceiling any more — it only shows that the ceiling
is above 6,144 entities. Naming a specific new limit would require tiers this ladder does not have,
so none is claimed here. Extending the ladder is a follow-up below.

These are native host figures. They are not browser figures, and no browser number is claimed here —
see the limits below.

## What the numbers say

**1. The frame no longer materializes a snapshot it throws away.** This was finding 3 of v0.6, and
closing it is what this release is. Rust now marks dirty entities, deposits, terrain, and chunks
where they are mutated, and builds the delta from those marks against a retained baseline of what
the host was last sent. A frame that changes forty entities builds forty entity snapshots instead of
6,144. The effect grows with the blueprint exactly as the previous cost did: 1.0× at one line,
16.8× at 512.

**2. Building a complete snapshot is now linear in entity count.** Two scans inside it were
quadratic and are gone. An extractor's reported status asked `resource_at_world` to search every
generated tile, so a snapshot cost entities × tiles; it now resolves through the same cached deposit
reference the tick path has used since v0.6. Each generated chunk counted its entities by filtering
the whole blueprint; one pass over the blueprint now counts them all. Subtracting the checksum, the
cost per entity across the ladder is 0.48, 0.16, 0.20, 0.18, 0.20, and 0.17 µs — flat apart from the
twelve-entity tier, where fixed costs dominate. In v0.6 the same figures rose 12-fold across the
range. This matters even though a frame no longer builds a full snapshot: the host's first frame
does, and so does every reset, new game, and load.

**3. The payload did not shrink, and was not meant to.** Delta bytes are within 3 bytes of v0.6 at
every tier — 103–110 bytes per entity, unchanged. v0.6 made the delta sparse on the wire; v0.7 makes
it sparse to compute. The 2-byte increase is the resources group gaining an object wrapper now that
it is a keyed patch rather than a bare array. This workload sends no less because every line's
extractor and composer advance on the same tick and every deposit is drawn from together, so almost
everything genuinely changes. A quieter blueprint, and any blueprint where deposits are drawn at
different times, sends proportionally less.

**4. The checksum is now the largest single identified cost in a frame.** It is 27–37% of the frame
at every tier above the smallest, and 62% at the smallest. It is linear — 0.029 to 0.033 µs per
tile-plus-entity across the ladder — but it walks every generated tile and every entity on every
tick, which is now more work than anything else the frame does per entity. Subtracting tick and
checksum leaves 58–67% of the frame above the smallest tier, which is the delta build plus JSON
serialization of a payload that reaches 644 KB at the largest tier.

**5. Tick, compile, recompile, and edit are unchanged within noise.** Tick sits 4–18% above v0.6
across the ladder, which is inside this harness's stated noise floor; the dirty marks it now makes
are appends to a vector, deliberately not inserts into an ordered set, and the delta sorts them once
per frame instead. Incremental recompilation still costs about three times a full compile — the
ratios are 5.5, 3.2, 3.0, 3.0, 2.6, and 3.0 — and this release did not touch the transport path, so
the v0.5.1 reading stands unchanged.

## Limits of this measurement

- **Native, not browser.** These runs are native host builds. The shipped artifact is wasm in a
  worker, where absolute costs differ. The tiers are a relative capacity record and a baseline for
  regression, not a prediction of browser frame rate. Browser-side measurement is still the named
  follow-up, and is now the only thing standing between this record and a real capacity claim.
- **The ladder no longer brackets the ceiling.** Every tier fits inside a 60 Hz frame, so this run
  measures headroom rather than a limit. Treat "above 6,144 entities" as the whole of what it says.
- **One machine, one run per tier.** No repetition, variance, or confidence interval is recorded.
  Treat differences under roughly 20% as noise. The `line` tier's compile, recompile, and edit
  figures move by more than that between runs and should not be read closely.
- **The release profile is tuned for wasm size** (`opt-level = "s"`, LTO), matching what ships.
  A speed-tuned native build would be faster and would not represent the artifact.
- **One workload shape.** Uniform straight lines with an always-accepting sink. It does not cover
  backpressure-saturated networks, long turning belt runs, dense multi-cell packing, or deposits
  running dry. Findings 1, 2, and 4 are properties of the core loop and generalize. Finding 3's
  ratio is specific to this workload's change rate, and finding 5 is stated for this workload only.
  Its rotations deliberately include orientations that merge and split neighbouring components,
  which is demanding for the incremental path.
- **Dirty tracking is measured at a high change rate.** Roughly 43% of this workload's entities
  change every tick, so the sparse path is measured near its worst case for payload but also near
  its worst case for rebuild count. A quiet blueprint is not represented.
- **Deposit references are measured with a static tile set.** No chunk is generated mid-run in this
  workload, so the cost of re-resolving deposits after generation — which also re-marks every
  entity — is not represented. That path is correctness-tested but unmeasured, and it runs at most
  once per generated chunk.
- **Timings include allocation.** No allocator was pinned or replaced.

## Follow-ups, in the order the measurement supports

1. Measure the same ladder in the browser worker, so a wasm capacity tier exists next to this native
   one. This was follow-up 2 in v0.6 and is now first: nothing here predicts browser frame rate, and
   every remaining native cost is small enough that the wasm and postMessage boundary is plausibly
   the real limit.
2. Extend the ladder past 6,144 entities, so the record brackets a native ceiling again instead of
   only showing headroom.
3. Attack serialization and the checksum together, in that order — findings 3 and 4. The frame's
   remaining two-thirds is a 644 KB JSON payload; a compact binary encoding over a transferable
   buffer would cut both the serialization cost and the copy. The whole-world checksum is the next
   largest, and an incremental one is possible, but it is determinism-critical and should not be
   touched before a browser measurement says it is worth the risk.
4. Re-examine whether incremental transport recompilation should keep persistent structures across
   edits, or whether the full compile is simply the better default at these sizes. Finding 5 is the
   evidence; do not remove the incremental path on it alone, because its tested behaviour under
   component splits and merges is a correctness asset.
5. Only after a browser measurement: revisit the renderer. The measurement still does not implicate
   rendering.

Record new runs by adding a dated report under `docs/benchmarks/` and updating the table above.
Comparisons are only valid while the pinned workload checksum in the Rust test gate is unchanged.

Previous records: [`benchmarks/capacity-v0.6.json`](benchmarks/capacity-v0.6.json),
[`benchmarks/capacity-v0.5.1.json`](benchmarks/capacity-v0.5.1.json).
