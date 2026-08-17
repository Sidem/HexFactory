# HexFactory capacity benchmarks

Status: Browser Capacity v0.8 is the fourth measured record and the first that is not native. The
roadmap gates any renderer decision and every scale claim behind this measurement. Nothing here is
an extrapolation: each number below was produced by the committed harness, and the raw reports are
stored beside this document.

The same ladder now runs in both places. Rust owns the measurement; only the clock differs — a
native `Instant`, or `performance.now` inside the browser worker — so the two records are
comparable by construction rather than by re-implementation. Both are recorded here, measured on
the same machine on the same day.

Run the native ladder with:

```bash
npm run bench
```

`npm run bench -- --quick` runs a reduced ladder, and `npm run bench -- --json <path>` writes the
machine-readable report. Run the browser ladder with:

```bash
npm run bench:browser
```

That builds the harness artifact and starts the dev server; open `/HexFactory/bench.html` and press
**Run full ladder**. The harness is compiled into wasm only by `--features bench`, so the deployed
game artifact never carries it: the shipped wasm is unchanged at 464 KB, while the harness build is
496 KB. Neither benchmark is part of `npm run quality`: shared CI runners do not produce comparable
timings. The test gate instead pins the workload's checksum and asserts the harness still runs, so
recorded numbers cannot silently stop being comparable.

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
| `round trip`  | browser only: the same frame, requested and received over the worker RPC    |
| `apply`       | browser only: the main thread merging that delta into its cached snapshot   |
| `host frame`  | browser only: `round trip + apply`, one simulated frame end to end          |

`recompile` and `compile` are directly comparable — the incremental path is timed without the edit
path's legality work, so the comparison is not confounded by it.

`snapshot` is not part of a frame. Since v0.7 the complete snapshot is built only for the host's
first frame, and it is kept in the ladder as the baseline the incremental delta is measured against.

The three browser metrics are what a native run cannot see. `frame` stops at the edge of wasm;
`round trip` is the same work as the game asks for it — `postMessage` out, the worker's own
`JSON.parse`, the structured clone of the delta, and both scheduling hops — and `apply` is
`applySnapshotDelta` merging the per-entity patch on the main thread. Neither includes rendering.

**Sample budgets differ between the two records, and only the sample budgets.** A native clock
resolves nanoseconds, so each phase runs its tier's fixed sample block once. A browser clamps
`performance.now` to 100 µs unless the page is cross-origin isolated, which is coarser than most
phases here, so each phase repeats its block until it has run at least 20 ms — holding the clock
step to 0.5% of the phase. Every metric is a mean per tick, per frame, or per edit, so the two
remain comparable; the workload itself never changes, and each tier's checksum is taken from a
separate core advanced exactly once through its tick budget so that extra samples cannot move it.

## Recorded results

Host: AMD Ryzen 7 5800X (8 cores / 16 threads), Windows 11 Pro 10.0.26200, rustc 1.87.0,
`factory-wasm` 0.8.0 built with the shipped release profile (`opt-level = "s"`, LTO, `wasm-opt -Oz`).
Recorded 2026-08-17. Raw reports:
[`benchmarks/capacity-v0.8-native.json`](benchmarks/capacity-v0.8-native.json) and
[`benchmarks/capacity-browser-v0.8.json`](benchmarks/capacity-browser-v0.8.json).

### Native

| tier   | lines | entities |  tiles | tick µs | snapshot µs | checksum µs | frame µs | delta bytes | compile µs | recompile µs | edit µs |
| ------ | ----: | -------: | -----: | ------: | ----------: | ----------: | -------: | ----------: | ---------: | -----------: | ------: |
| line   |     1 |       12 |    576 |     0.5 |        19.6 |        14.5 |     20.2 |       1,318 |        1.1 |          5.8 |     6.4 |
| small  |    16 |      192 |  1,216 |     5.2 |        65.9 |        38.2 |    111.0 |      19,745 |       16.2 |         54.0 |    60.8 |
| medium |    64 |      768 |  3,520 |    21.9 |       258.5 |       119.5 |    435.3 |      79,400 |       79.5 |        248.1 |   263.5 |
| wide   |   128 |    1,536 |  6,592 |    46.3 |       491.5 |       229.9 |    858.9 |     159,555 |      176.2 |        567.5 |   626.1 |
| large  |   256 |    3,072 | 12,736 |   103.6 |       985.6 |       447.6 |  1,741.2 |     320,447 |      372.6 |      1,080.1 | 1,205.1 |
| xlarge |   512 |    6,144 | 25,024 |   245.1 |     1,932.8 |       890.2 |  3,515.7 |     644,144 |      752.6 |      2,069.6 | 2,226.8 |

Every tier reproduces its v0.7 checksum and delivered total exactly, and every timing sits within
the harness's stated noise floor of the v0.7 record. This run is the control the browser record is
measured against, not a new native finding.

### Browser worker

Chromium 148 in an Electron 42 shell
(`Mozilla/5.0 … Claude/1.30096.5 Chrome/148.0.7778.280 Electron/42.7.0 …`), 16 hardware threads,
`performance.now` observed at a 100 µs step, page not cross-origin isolated.

| tier   | entities | tick µs | snapshot µs | checksum µs | frame µs | delta bytes | compile µs | recompile µs | edit µs |
| ------ | -------: | ------: | ----------: | ----------: | -------: | ----------: | ---------: | -----------: | ------: |
| line   |       12 |     0.7 |        28.1 |        15.1 |     34.2 |       1,318 |        1.6 |          9.5 |     7.9 |
| small  |      192 |     7.4 |        77.0 |        40.8 |    157.5 |      19,745 |       23.9 |         79.0 |    89.6 |
| medium |      768 |    27.9 |       244.0 |       129.5 |    534.0 |      79,401 |      120.0 |        375.0 |   386.7 |
| wide   |    1,536 |    58.1 |       496.7 |       250.8 |  1,055.0 |     159,555 |      265.0 |        680.0 |   706.7 |
| large  |    3,072 |   123.8 |     1,057.5 |       491.3 |  2,132.5 |     320,447 |      521.7 |      1,306.7 | 1,396.7 |
| xlarge |    6,144 |   296.7 |     2,040.0 |       975.0 |  4,190.0 |     644,144 |    1,116.7 |      2,633.3 | 2,891.7 |

And the part only the browser can measure — what one frame costs the host, outside wasm:

| tier   | entities | frame µs | round trip µs | apply µs | host frame µs | boundary µs | share of a 60 Hz frame |
| ------ | -------: | -------: | ------------: | -------: | ------------: | ----------: | ---------------------: |
| line   |       12 |     34.2 |          97.3 |      3.0 |         100.3 |        63.0 |                   0.6% |
| small  |      192 |    157.5 |         405.0 |      4.0 |         409.0 |       247.5 |                   2.5% |
| medium |      768 |    534.0 |       1,372.0 |     10.0 |       1,382.0 |       838.0 |                   8.3% |
| wide   |    1,536 |  1,055.0 |       2,596.7 |     38.3 |       2,635.0 |     1,541.7 |                  15.8% |
| large  |    3,072 |  2,132.5 |       4,990.0 |     35.0 |       5,025.0 |     2,857.5 |                  30.1% |
| xlarge |    6,144 |  4,190.0 |      10,275.0 |     70.0 |      10,345.0 |     6,085.0 |                  62.1% |

`boundary` is `round trip − frame`: everything the crossing costs on top of the work wasm did.

### Browser against native

| tier   | native frame | wasm frame | wasm/native | host frame | host/native |
| ------ | -----------: | ---------: | ----------: | ---------: | ----------: |
| line   |         20.2 |       34.2 |       1.70× |      100.3 |       4.96× |
| small  |        111.0 |      157.5 |       1.42× |      409.0 |       3.68× |
| medium |        435.3 |      534.0 |       1.23× |    1,382.0 |       3.17× |
| wide   |        858.9 |    1,055.0 |       1.23× |    2,635.0 |       3.07× |
| large  |      1,741.2 |    2,132.5 |       1.22× |    5,025.0 |       2.89× |
| xlarge |      3,515.7 |    4,190.0 |       1.19× |   10,345.0 |       2.94× |

Every browser tier reproduces the native checksum and delivered total for its tier, so the two
records measure the same simulation and can be compared directly.

## Measured capacity tiers

Against a 16,667 µs frame at 60 Hz, using the browser's measured `host frame` — the whole cost of
advancing the simulation one tick and merging the result, excluding rendering:

| tier   | entities | share of a 60 Hz frame | verdict     |
| ------ | -------: | ---------------------: | ----------- |
| line   |       12 |                   0.6% | comfortable |
| small  |      192 |                   2.5% | comfortable |
| medium |      768 |                   8.3% | comfortable |
| wide   |    1,536 |                  15.8% | comfortable |
| large  |    3,072 |                  30.1% | workable    |
| xlarge |    6,144 |                  62.1% | tight       |

**The browser ladder still does not miss 60 Hz, but it no longer has the headroom the native one
showed.** The largest tier used 23.1% of a frame in the v0.7 native record and uses 62.1% here,
with rendering still to pay for out of the remaining 38%. The ceiling is above 6,144 entities in
both records; this one says it is not far above, and — for the first time — says so about the
artifact that actually ships.

## What the numbers say

**1. Wasm costs about 1.2× native, and that was never the question.** Across the four largest tiers
the in-wasm frame is 1.19–1.23× its native counterpart, and the individual phases agree: tick
1.19–1.27×, checksum 1.04–1.10×, snapshot within noise of parity, compile about 1.45×. The 1.70× at
the smallest tier is fixed cost against a 20 µs frame, not a scaling effect. Three releases of
native optimization transfer to the browser essentially intact.

**2. The worker boundary costs more than the simulation it carries.** At every tier above the
smallest, the crossing is 57–61% of what a frame costs the host: 6,085 µs of the xlarge tier's
10,345 µs frame is spent getting a delta wasm already built onto the main thread. The native record
has no way to see this, which is why it read 23.1% of a frame where the shipped artifact reads
62.1%.

**3. The boundary cost is the payload, at about 10 µs per kilobyte.** Dividing the boundary by the
delta it carried gives 12.8, 10.8, 9.9, 9.1, and 9.7 µs/KB from `small` to `xlarge` — flat, and
tracking bytes rather than entities. (The `line` tier's 48.9 µs/KB is its ~60 µs fixed round-trip
floor spread over 1.3 KB.) This is the same 644 KB JSON payload v0.7 named as its own next target;
the browser measurement now prices it. A compact binary encoding over a transferable buffer attacks
serialization, the parse, and the copy at once, and it is the only identified cost large enough to
change the ladder.

**4. The main-thread merge is not a problem.** Applying the per-entity patch costs 3–70 µs, which
is 0.7–1.5% of a host frame above the smallest tier. The per-entity buildings delta from v0.6 is
doing its job: the host touches only what changed, and the cost of merging 6,144 entities' worth of
frames is smaller than the cost of one native checksum. Nothing here argues for changing the merge.

**5. The checksum is still the largest single cost inside wasm, and now has a cheaper rival.**
It remains 23–26% of the in-wasm frame, linear, and unchanged in character from v0.7. But it is now
9–10% of a host frame, while the boundary is roughly 60%. An incremental checksum is
determinism-critical
work for a modest share; the encoding is not. The measurement reorders them.

## Limits of this measurement

- **One browser, one shell.** Chromium 148's V8 and wasm engine, running inside an Electron 42
  host, not a standalone browser. No Firefox, Safari, mobile, or low-core-count figure exists, and
  a phone is not represented by anything here.
- **Rendering is excluded.** `host frame` covers advancing the simulation and merging the delta.
  The Canvas renderer's cost is not measured, and the remaining 38% of a 60 Hz frame at the largest
  tier is what it would have to fit inside.
- **The round trip and the merge are timed in separate passes.** A clamped clock cannot resolve one
  400 µs frame, so each phase is timed once around its whole budget: every delta is collected over
  the RPC first, then merged in arrival order. The game interleaves them. Their sum is a fair
  account of the work; the cache behaviour of the interleaving is not represented.
- **`apply` is the coarsest number here.** Its phase runs 0.8–2.3 ms against a 100 µs clock step,
  so treat it as ±10%. It is small enough that this does not affect any conclusion above.
- **The ladder no longer brackets a native ceiling.** Every tier fits inside a 60 Hz frame in both
  records. Treat "above 6,144 entities" as the whole of what they say about the limit.
- **One machine, one run per tier.** No repetition, variance, or confidence interval is recorded.
  Treat differences under roughly 20% as noise. The `line` tier's compile, recompile, and edit
  figures move by more than that between runs and should not be read closely.
- **The release profile is tuned for wasm size** (`opt-level = "s"`, LTO, `wasm-opt -Oz`), matching
  what ships. A speed-tuned build would be faster and would not represent the artifact.
- **One workload shape.** Uniform straight lines with an always-accepting sink. It does not cover
  backpressure-saturated networks, long turning belt runs, dense multi-cell packing, or deposits
  running dry. Findings 1, 2, and 4 are properties of the core loop and generalize. Finding 3's
  ratio is specific to this workload's change rate.
- **Dirty tracking is measured at a high change rate.** Roughly 43% of this workload's entities
  change every tick, so the sparse path is measured near its worst case for payload. A quiet
  blueprint sends proportionally less and would cross the boundary proportionally faster.
- **Timings include allocation.** No allocator was pinned or replaced.

## Follow-ups, in the order the measurement supports

These are the _engine's_ follow-ups, ordered by evidence. The restated game-first goal placed the
Game Feel v0.9 milestone ahead of them; that milestone has shipped, so these are next. The ordering
among them is unchanged, and v0.9 added one new entry at the end.

1. Replace the JSON delta with a compact binary encoding over a transferable buffer. Finding 3 is
   the evidence: the boundary is 60% of a host frame and tracks payload bytes at about 10 µs/KB, so
   this is the only identified change that can move the browser ladder. It attacks serialization,
   the parse, and the copy together.
2. Measure the Canvas renderer against the same tiers, so a browser frame is accounted for end to
   end rather than up to the point rendering begins. Until then no complete browser frame-rate claim
   is supported, only the simulation half of one.
3. Extend the ladder past 6,144 entities, so the record brackets a ceiling again instead of only
   showing headroom.
4. An incremental checksum, after the encoding. Finding 5: it is the largest cost inside wasm but a
   ninth of a host frame, and it is determinism-critical, so it should not be touched first.
5. Re-examine whether incremental transport recompilation should keep persistent structures across
   edits, or whether the full compile is simply the better default at these sizes. Both records show
   the incremental path costing about three times a full compile; do not remove it on that alone,
   because its tested behaviour under component splits and merges is a correctness asset.
6. Only after the renderer measurement: revisit the renderer itself. Nothing measured yet implicates
   it, and nothing measured yet exonerates it.
7. Batch the transport recompile inside a construction drag. v0.9 routes a drag through the tested
   per-cell `place`, so a 32-cell run recompiles 32 times. It happens once when the pointer is
   released rather than every frame, and no tier in the ladder measures it, so this is a known cost
   and not yet a measured one — measure it before optimizing it, like everything else on this list.

Record new runs by adding a dated report under `docs/benchmarks/` and updating the tables above.
Comparisons are only valid while the pinned workload checksum in the Rust test gate is unchanged.

Previous records: [`benchmarks/capacity-v0.7.json`](benchmarks/capacity-v0.7.json),
[`benchmarks/capacity-v0.6.json`](benchmarks/capacity-v0.6.json),
[`benchmarks/capacity-v0.5.1.json`](benchmarks/capacity-v0.5.1.json).
