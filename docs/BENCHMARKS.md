# HexFactory capacity benchmarks

Status: **Look Systems v0.13.1 re-measured the browser ladder** against the v0.12.4 renderer
record; Power v0.13 remains the current native record. Rendering moved because Stage B and the
first Stage C motion added per-hex work; checksums did not, because presentation owns nothing.
Nothing here is an extrapolation: each number below was produced by the committed harness, and
the raw reports are stored beside this document.

**The v0.8 tables below are a historical record, not the current cost**, and so is everything the
v0.8 browser record concluded about the worker boundary. v0.11 changed the world generator, v0.12
changed the item roster, v0.12.1 bumped `WORLD_GENERATOR_VERSION` again, and v0.12.2 replaced the
JSON delta with a binary one — so both the workload's pinned checksum and the shape of a host frame
have moved.
[Binary Delta v0.12.2](#binary-delta-v0122--the-fifth-record-and-the-first-browser-re-measurement-since-v08)
is what currently holds for both platforms. The v0.8 browser tables are retained because the
current record's central comparison is drawn against them.

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
game artifact never carries it: at v0.8 the shipped wasm was 464 KB and the harness build 496 KB,
and the harness has never been part of the shipped one. (v0.12.2's shipped wasm is 520.0 KiB and
its harness build 563.8 KiB. The binary encoder accounts for 3.7 KiB of the shipped figure,
measured by building v0.12.1 and v0.12.2 the same way; the rest of the growth since v0.8 is the
material base's definitions and generation.) Neither benchmark is part of
`npm run quality`: shared CI runners do not produce comparable
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
| `world`         | browser only: `CanvasFactoryRenderer.draw` at a pinned 1440×900 viewport    |
| `minimap`       | browser only: `MinimapRenderer.draw` at the shipped 178 px square           |
| `render`        | browser only: `world + minimap`                                             |
| `browser frame` | browser only: `host frame + render`, one frame end to end                   |

`recompile` and `compile` are directly comparable — the incremental path is timed without the edit
path's legality work, so the comparison is not confounded by it.

`snapshot` is not part of a frame. Since v0.7 the complete snapshot is built only for the host's
first frame, and it is kept in the ladder as the baseline the incremental delta is measured against.

The host metrics are what a native run cannot see. `frame` stops at the edge of wasm;
`round trip` is the same work as the game asks for it — `postMessage` out, the transfer of the
delta buffer, the main thread's decode of it, and both scheduling hops — and `apply` is
`applySnapshotDelta` merging the per-entity patch on the main thread. `world` and `minimap` are
the two canvases the game draws, timed against the snapshot the merge just produced, at a
pinned 1440×900 viewport and the shipped 178 px minimap so a record is a measurement of the
renderer rather than of the bench page's layout. Each render phase uses the same 20 ms sample
budget as the rest of the browser harness.

Before v0.12.2 the delta crossed as JSON, so `round trip` covered the worker's own `JSON.parse` and
a structured clone of the resulting object graph instead of a transfer and a decode. The phase
boundary is the same in both: `round trip` ends when the main thread holds a usable delta object.
Where the decoding work sits inside it moved from one side of the boundary to the other, which is
why the harness decodes in its transport rather than in the timed loop — the game does the same.

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

### Native — v0.12 re-measurement

Same host and harness, `factory-wasm` at v0.12, recorded 2026-08-17. Raw report:
[`benchmarks/capacity-v0.12-native.json`](benchmarks/capacity-v0.12-native.json). The workload's
shape, entity counts, and delivered totals are unchanged; its checksum is not, because
`WORLD_GENERATOR_VERSION` and every machine's fuel charge are checksum inputs. **Tier checksums are
therefore not comparable to the v0.8 record — the timings are, and that is what this run is for.**

| tier   | entities | tiles | tick µs | snapshot µs | checksum µs | frame µs | delta bytes | compile µs | recompile µs | edit µs |
| ------ | -------: | ----: | ------: | ----------: | ----------: | -------: | ----------: | ---------: | -----------: | ------: |
| line   |       12 |     1 |     0.5 |        11.2 |         0.8 |      6.6 |       1,319 |        0.9 |          5.8 |     6.0 |
| small  |      192 |    16 |     5.7 |        57.8 |         9.3 |     83.1 |      19,764 |       17.3 |         54.2 |    61.3 |
| medium |      768 |    64 |    23.6 |       264.8 |        36.4 |    366.7 |      79,477 |       87.8 |        262.9 |   280.7 |
| wide   |    1,536 |   128 |    49.6 |       593.0 |        73.1 |    748.5 |     159,709 |      189.6 |        569.2 |   643.6 |
| large  |    3,072 |   256 |   113.1 |     1,131.6 |       145.0 |  1,434.9 |     320,754 |      400.7 |      1,150.6 | 1,230.8 |
| xlarge |    6,144 |   512 |   261.0 |     2,219.7 |       293.1 |  2,908.5 |     644,758 |      794.3 |      2,261.7 | 2,446.5 |

Three things moved, and only one of them is v0.12's doing.

**The checksum got 3.0× cheaper, and that is v0.11 being measured for the first time.** 890 µs to
293 µs at the largest tier. The checksum hashes the stored tile overlay, and v0.11 made that overlay
sparse: unmined field is derived and only drawn-from cells are stored, so this workload's tile count
fell from 25,024 to 512 — one per line, exactly the deposits its extractors work. v0.11 shipped
without re-measuring, so the saving is recorded here rather than there. It carries the in-wasm frame
with it: 3,516 µs to 2,908 µs, 17% cheaper at the largest tier. **Finding 5 of the v0.8 record no
longer holds** — the checksum is not the largest cost inside wasm any more, and an incremental
checksum has lost most of its case.

**Two v0.12 regressions were found by this run and fixed before it was recorded.** They are kept
here because they are what re-measuring a milestone is for:

- Publishing `fuel_charge` and `fuel_required` on every entity added 86 KB to the largest tier's
  delta — 13%, or roughly 860 µs of boundary at the v0.8 record's 10 µs/KB — to say "this is not a
  furnace" about belts and containers. Both are now omitted when zero, and the payload is back to
  644 KB with the 614 bytes of real fuel data the composers carry.
- `field_at` scanned the scenario's hand-placed resource list for every hex, and a complete snapshot
  asks it once per hex of every surveyed chunk. Against a tier that places one resource per line
  that made the snapshot O(hexes × resources): 7,480 µs at the largest tier against v0.8's 1,933 µs.
  The list is now indexed by tile key at construction, and the snapshot is 2,220 µs. This was a
  v0.11 defect that v0.12's re-measurement exposed; the snapshot is not part of a frame, so it was
  costing the first frame, reset, new game, and load rather than steady state.

Everything else sits within the harness's stated noise. `compile` and `edit` read 4–10% higher and
`tick` within 7%; treat differences under roughly 20% as noise, as the limits below say.

### Native — v0.12.1 re-measurement

Same host and harness, `factory-wasm` at v0.12.1, recorded 2026-08-17. Raw report:
[`benchmarks/capacity-v0.12.1-native.json`](benchmarks/capacity-v0.12.1-native.json). The
workload's shape, entity counts, delivered totals, and delta sizes are unchanged; its checksum is
not, because `WORLD_GENERATOR_VERSION` is a checksum input. **Tier checksums are therefore not
comparable to the v0.12 record — the timings are, and that is what this run is for.**

| tier   | entities | tiles | tick µs | snapshot µs | checksum µs | frame µs | delta bytes | compile µs | recompile µs | edit µs |
| ------ | -------: | ----: | ------: | ----------: | ----------: | -------: | ----------: | ---------: | -----------: | ------: |
| line   |       12 |     1 |     0.5 |        11.8 |         0.8 |      6.7 |       1,319 |        1.2 |          5.8 |     6.2 |
| small  |      192 |    16 |     6.1 |        60.8 |         9.7 |     95.2 |      19,764 |       17.2 |         55.0 |    62.6 |
| medium |      768 |    64 |    24.1 |       279.7 |        36.9 |    370.0 |      79,477 |       83.0 |        256.6 |   284.4 |
| wide   |    1,536 |   128 |    50.9 |       597.9 |        74.3 |    773.0 |     159,709 |      182.2 |        591.4 |   668.3 |
| large  |    3,072 |   256 |   132.6 |     1,222.2 |       148.6 |  1,511.4 |     320,754 |      389.6 |      1,224.0 | 1,335.5 |
| xlarge |    6,144 |   512 |   264.1 |     2,421.7 |       294.6 |  3,032.6 |     644,759 |      814.1 |      2,767.8 | 2,593.8 |

No new regression. Every timing sits within the harness's stated noise of the v0.12 record — the
generator bump changed which cells exist in the world, not how the synthetic ladder is built, and
the delta payload is the same 644 KB at the largest tier.

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

## Binary Delta v0.12.2 — the fifth record, and the first browser re-measurement since v0.8

Same host, `factory-wasm` at 0.12.2, recorded 2026-08-18. Raw reports:
[`benchmarks/capacity-v0.12.2-native.json`](benchmarks/capacity-v0.12.2-native.json) and
[`benchmarks/capacity-v0.12.2-browser.json`](benchmarks/capacity-v0.12.2-browser.json). This run
answers follow-up 1, and by existing at all it also answers follow-up 8.

**What changed between the records is the encoding, not the simulation.** The delta crosses the
worker boundary as a compact binary buffer that is transferred rather than structured-cloned, and
the workload is untouched: every tier reproduces its v0.12.1 checksum and delivered total. Report
schema 4 adds `delta_json_bytes` and redefines `delta_bytes` as the binary payload, so the two
columns below are the same frames measured both ways rather than two runs compared.

### Native — v0.12.2

| tier   | entities | tick µs | snapshot µs | checksum µs | frame µs | delta bytes | json bytes | ratio | compile µs | recompile µs | edit µs |
| ------ | -------: | ------: | ----------: | ----------: | -------: | ----------: | ---------: | ----: | ---------: | -----------: | ------: |
| line   |       12 |     0.5 |        11.3 |         0.8 |      3.1 |         104 |      1,319 | 12.7× |        1.1 |          6.3 |     6.4 |
| small  |      192 |     5.6 |        54.5 |         9.5 |     35.1 |       1,376 |     19,764 | 14.4× |       16.4 |         56.3 |    62.9 |
| medium |      768 |    22.9 |       235.5 |        36.9 |    143.5 |       5,803 |     79,477 | 13.7× |       82.6 |        250.5 |   274.3 |
| wide   |    1,536 |    49.3 |       524.1 |        75.2 |    298.0 |      11,819 |    159,709 | 13.5× |      181.7 |        575.0 |   642.8 |
| large  |    3,072 |   108.7 |     1,007.0 |       149.2 |    626.5 |      23,723 |    320,754 | 13.5× |      384.7 |      1,140.5 | 1,238.9 |
| xlarge |    6,144 |   255.3 |     2,056.3 |       307.3 |  1,239.4 |      47,531 |    644,759 | 13.6× |      770.9 |      2,136.7 | 2,366.0 |

The `json bytes` column reproduces the v0.12.1 record's `delta bytes` exactly at every tier, which
is the control: the workload is byte-for-byte the one that record measured. `tick`, `checksum`, and
`compile` all sit within a few percent of it. `frame` — a tick plus one encoded delta — falls from
3,032.6 µs to 1,239.4 µs at the largest tier, **2.4× cheaper, because building a 630 KB JSON string
was most of what a frame did.**

### Browser worker — v0.12.2

Chromium 148 in an Electron 42 shell
(`Mozilla/5.0 … Claude/1.32352.1 Chrome/148.0.7778.280 Electron/42.9.2 …`), 16 hardware threads,
`performance.now` observed at a 100 µs step, page not cross-origin isolated.

| tier   | entities | frame µs | round trip µs | apply µs | host frame µs | boundary µs | share of a 60 Hz frame |
| ------ | -------: | -------: | ------------: | -------: | ------------: | ----------: | ---------------------: |
| line   |       12 |      6.6 |          68.8 |      3.5 |          72.3 |        62.2 |                   0.4% |
| small  |      192 |     46.5 |         126.5 |      3.0 |         129.5 |        80.0 |                   0.8% |
| medium |      768 |    176.5 |         305.0 |     10.0 |         315.0 |       128.5 |                   1.9% |
| wide   |    1,536 |    355.0 |         533.3 |     21.7 |         555.0 |       178.3 |                   3.3% |
| large  |    3,072 |    767.5 |         962.5 |     42.5 |       1,005.0 |       195.0 |                   6.0% |
| xlarge |    6,144 |  1,440.0 |       1,720.0 |    115.0 |       1,835.0 |       280.0 |                  11.0% |

Every tier reproduced its native checksum and delivered total, and every applied snapshot kept its
full entity count — which is also the end-to-end proof that the binary buffer decodes to the delta
the merge expects, over six tiers of real frames rather than a fixture.

### What this run says

**1. The payload is 13.5–14.4× smaller, and that is a measured ratio rather than two runs
compared.** The largest tier's per-frame delta falls from 644,759 bytes to 47,531. The saving comes
from varints instead of decimal text, one byte for each closed-set enum, delta-coded entity ids and
tile coordinates, and a bit per absent option instead of a field name and a `null`.

**2. The boundary is no longer the frame's dominant cost.** At the largest tier it falls from
6,085 µs to 280 µs, and from 59% of a host frame to 15%. Finding 3 of the v0.8 record — that the
crossing cost more than the simulation it carried — no longer holds: the wasm frame is now 78% of
the host frame, and the engine is the cost again.

This comparison is against the v0.8 browser record, which is two core milestones old, so it needs
saying why it is admissible. `boundary` is `round trip − frame`, which subtracts the wasm work out;
both records carry essentially the same JSON payload at the largest tier (644,144 then, 644,759
now); and both ran on the same host and the same Chromium major. What differs for the boundary
between them is the encoding. The 21.7× is larger than the payload's 13.6× because two other costs
went with it: the worker's own `JSON.parse` and the structured clone of the resulting object graph.

**3. The 10 µs/KB law has been replaced by a fixed floor.** v0.8 measured the crossing at a flat
9.1–12.8 µs per kilobyte from `small` to `xlarge`. It is no longer flat, because the payload no
longer dominates: the `line` tier's 62 µs over 105 bytes is a round-trip floor, and it is still
62–80 µs at `small`. Only at the two largest tiers does payload cost overtake it, at roughly
5–6 µs/KB marginal. **Below `wide`, sending less would now buy almost nothing.**

**4. The 60 Hz share collapses, and the ladder stops bracketing a ceiling in the browser too.** The
largest tier goes from 62.1% of a frame to 11.0%. The v0.8 record's closing line — that the ceiling
is above 6,144 entities but not far above it — is retired. Rendering is still unmeasured and still
has to fit in what is left, but there is now 89% of a frame for it rather than 38%.

**5. The shipped artifact grew by 3.7 KiB.** 516.3 KiB at v0.12.1 against 520.0 KiB here, both
`wasm-opt -Oz`, measured by building both. `snapshot_delta_json` is retained as the encoder's test
oracle and is part of that figure.

### Limits specific to this run

- **Three full browser runs were taken; one is recorded.** The largest tier's host frame read
  2,015, 1,770, and 1,835 µs across them — about ±7% around 1,873 µs. Earlier records state a
  noise floor without evidence for it; this is the first with any repetition behind it, and it
  supports the existing "treat differences under roughly 20% as noise" rule rather than replacing
  it. Only the third run is recorded, so the table is a single run like every other here.
- **`apply` is now a larger share of a much smaller frame,** at 6.3% of the largest tier's host
  frame against 0.7% in v0.8. It has not become expensive — 115 µs against a 100 µs clock step is
  one or two ticks of that clock — but it is no longer negligible, and it is the next thing to
  measure properly if the host frame keeps falling.
- **The comparison in finding 2 spans two core milestones.** The argument for why it is still
  attributable to the encoding is given there; it is an argument, not a controlled experiment. The
  controlled measurement is the native one, where both encodings were measured in the same run.
- Everything in **Limits of this measurement** below still applies: one browser, one shell, one
  machine, one workload shape, rendering excluded.

## Power v0.13 — native re-measurement

Same host, `factory-wasm` 0.13.0, recorded 2026-08-18. Raw report:
[`benchmarks/capacity-v0.13-native.json`](benchmarks/capacity-v0.13-native.json).

Checksums moved: each entity now hashes a power remainder. Delivered totals and entity counts
match v0.12.2. An all-pairs compile over every powered machine was caught and replaced by
pole-to-pole plus machine-to-pole before this record; the first draft made `xlarge` compile 61×
slower, which is why the re-measure exists.

| tier   | entities | tick µs | snapshot µs | checksum µs | frame µs | compile µs | recompile µs |
| ------ | -------: | ------: | ----------: | ----------: | -------: | ---------: | -----------: |
| line   |       12 |     0.9 |        10.8 |         1.0 |      3.4 |        1.5 |          5.5 |
| small  |      192 |     7.9 |        55.5 |        10.1 |     38.8 |       19.2 |         59.9 |
| medium |      768 |    34.2 |       256.9 |        42.3 |    163.1 |       92.9 |        254.3 |
| wide   |    1,536 |    71.4 |       563.8 |        79.5 |    347.9 |      238.8 |        604.0 |
| large  |    3,072 |   159.3 |     1,116.1 |       172.2 |    716.9 |      425.5 |      1,220.2 |
| xlarge |    6,144 |   361.2 |     2,200.7 |       322.6 |  1,455.5 |      912.8 |      2,290.1 |

Tick is about 1.4× the v0.12.2 native figure at the largest tier (refreshing supply and demand
once per tick). Compile is within noise of that record. The browser renderer numbers from
v0.12.4 still hold: this milestone did not change a draw.

## Renderer Measure v0.12.4 — the first complete browser frame

Same host, `factory-wasm` still at 0.12.3 (the engine is unchanged), recorded 2026-08-18. Raw
report: [`benchmarks/capacity-v0.12.4-browser.json`](benchmarks/capacity-v0.12.4-browser.json).
This run answers follow-up 2.

The viewport is pinned: 1440×900 for the world, 178 px for the minimap, `BASE_HEX_SIZE` 22,
`devicePixelRatio` 1. The camera follows the player, which for this workload means standing at
the origin of the plant — the view a player actually has, not a zoomed-out census of every
entity. Chromium 151
(`Mozilla/5.0 … Chrome/151.0.0.0 Safari/537.36`), 16 hardware threads, `performance.now`
observed at a 100 µs step, page not cross-origin isolated. That is a different Chrome major
than the v0.12.2 Electron 42 / Chrome 148 record; the simulation half is reported for
continuity, not as a controlled pair.

Every tier reproduced its v0.12.2 native checksum and delivered total, and every applied
snapshot kept its full entity count.

### Browser frame — v0.12.4

| tier   | entities | host frame µs | world µs | minimap µs | render µs | browser frame µs | sim share | frame share |
| ------ | -------: | ------------: | -------: | ---------: | --------: | ---------------: | --------: | ----------: |
| line   |       12 |          79.8 |    271.6 |       18.9 |     290.5 |            370.3 |      0.5% |        2.2% |
| small  |      192 |         153.0 |    346.6 |       57.3 |     403.9 |            556.9 |      0.9% |        3.3% |
| medium |      768 |         379.0 |    481.0 |      100.0 |     581.0 |            960.0 |      2.3% |        5.8% |
| wide   |    1,536 |         588.3 |    416.7 |       66.9 |     483.6 |          1,071.9 |      3.5% |        6.4% |
| large  |    3,072 |       1,160.0 |    689.7 |       89.7 |     779.3 |          1,939.3 |      7.0% |       11.6% |
| xlarge |    6,144 |       1,970.0 |    909.1 |      160.0 |   1,069.1 |          3,039.1 |     11.8% |       18.2% |

### What this run says

**1. A complete browser frame at the largest measured tier is 18.2% of 60 Hz.** The simulation
half that v0.12.2 left at 11.0% is 11.8% here — within the stated noise floor, on a different
Chrome major. Rendering is 1,069 µs, 35% of that complete frame and 6.4% of 60 Hz. The unknown
89% is gone.

**2. The world canvas is the render cost; the minimap is not.** At every tier the minimap is
19–160 µs, 6–15% of `render`. The second canvas v0.12.3 added is visible in the number and is
not what a frame is made of.

**3. World draw has a large floor and grows slowly.** Twelve entities already cost 272 µs;
6,144 cost 909 µs. The environment, fog, and clear are most of a draw. Walking every building
and resource to clip it is cheap at these sizes. The dip at `wide` (417 µs against medium's 481) is inside the 20% noise floor and is not a finding.

**4. Stage C is no longer gated on ignorance.** The largest tier leaves 81.8% of a 60 Hz frame.
Whether an animated frame wants a different renderer is now a question with a number, not a
prohibition. Follow-up 6 — revisit the renderer itself — is allowed by this record and not
demanded by it. Nothing here says Canvas 2D is the problem at 6,144 entities.

The one-time definition lookup the v0.12.2 follow-up named is folded into this milestone so
the measurement did not have to answer it: both canvases build `item_id` / `definition_id`
maps in the constructor and stop doing a linear `find` inside the per-entity draw loops.

### Limits specific to this run

- **One run, one Chrome 151, device pixel ratio 1.** A 2× retina viewport would rasterize four
  times the pixels; this record does not speak to that. The v0.12.2 simulation-half comparison
  is the same machine and not the same browser.
- **The camera follows the player.** Off-screen entities are still walked and then clipped;
  they are not drawn. A zoomed-out view that put the whole plant on screen would be a
  different measurement.
- Everything in **Limits of this measurement** below still applies, except that rendering is
  no longer excluded.

## Look Systems v0.13.1 — Stage B and the first Stage C motion

Same host, `factory-wasm` at 0.13.0, recorded 2026-08-18. Raw report:
[`benchmarks/capacity-v0.13.1-browser.json`](benchmarks/capacity-v0.13.1-browser.json).
This run answers the Look Systems re-measure the session brief named. No save, generator,
definition, or wire version moved; presentation owns the draw and nothing above `FactoryHost`
changed. Native checksums and delivered totals match the v0.13 record. Viewport still pinned
at 1440×900 world, 178 px minimap, `BASE_HEX_SIZE` 22, `devicePixelRatio` 1. Chromium 151,
16 hardware threads, `performance.now` observed at a 100 µs step, page not cross-origin
isolated.

### Browser frame — v0.13.1

| tier   | entities | host frame µs | world µs | minimap µs | render µs | browser frame µs | sim share | frame share |
| ------ | -------: | ------------: | -------: | ---------: | --------: | ---------------: | --------: | ----------: |
| line   |       12 |          60.0 |    254.4 |       15.4 |     269.9 |            329.9 |      0.4% |        2.0% |
| small  |      192 |         126.0 |    295.6 |       43.9 |     339.4 |            465.4 |      0.8% |        2.8% |
| medium |      768 |         349.0 |    418.7 |       53.3 |     472.1 |            821.1 |      2.1% |        4.9% |
| wide   |    1,536 |         561.7 |    495.1 |       76.9 |     572.0 |          1,133.7 |      3.4% |        6.8% |
| large  |    3,072 |       1,025.0 |    648.4 |      120.5 |     768.9 |          1,793.9 |      6.1% |       10.8% |
| xlarge |    6,144 |       1,980.0 |    990.5 |      200.0 |   1,190.5 |          3,170.5 |     11.9% |       19.0% |

### What this run says

**1. Stage B and the first Stage C motion fit in the headroom v0.12.4 measured.** A complete
browser frame at the largest tier is 19.0% of 60 Hz, against 18.2% before this pass. The world
is 991 µs, against 909 µs. That is a 9% world-draw increase and 0.8 percentage points of a
60 Hz frame, not a new renderer question.

**2. The environment is still the floor.** Twelve entities cost 254 µs of world draw; 6,144
cost 991 µs. Neighbour fringes, baked tiles, and silhouettes ride on the listed terrain and
entity sets, not on a walk of every surveyed hex.

**3. The simulation half did not move.** Host frame at the largest tier is 1,980 µs against
v0.12.4's 1,970 µs. Checksums match the v0.13 native record's delivered totals. Timing
comparisons to v0.12.4 still hold; checksum comparisons do not, and they already did not
after v0.13.

### Limits specific to this run

The v0.12.4 limits still apply: one Chrome 151, device pixel ratio 1, camera on the player.
A first walk of every surveyed hex was measured at about 8 ms and refused; the shipped draw
fills implicit lowland as a surveyed field and paints only the bands native actually sends.

## Generated Shapes v0.15 — an A/B, not a comparable record

Recorded 2026-08-18, `factory-wasm` at 0.14.0. Raw report:
[`benchmarks/capacity-v0.15-browser.json`](benchmarks/capacity-v0.15-browser.json).

**Read this section before quoting a number from it.** Every pinned condition matches the v0.13.1
record — 1440×900 world, 178 px minimap, `BASE_HEX_SIZE` 22, `devicePixelRatio` 1, 16 hardware
threads, `performance.now` at a 100 µs step, page not cross-origin isolated. One thing does not:
the browser. v0.13.1 was recorded on `Chrome/151.0.0.0`; this run is `Chrome/148.0.7778.280` inside
`Electron/42.9.2`, in a pane that was not compositing. Absolute figures here are roughly twice the
v0.13.1 ones **including the parts of the frame this milestone cannot touch**, so they are not a
re-measurement of the ladder and must not be quoted as one. AGENTS.md's rule that one Chromium
version on one desktop is the whole browser evidence is what makes this a different measurement
rather than a worse one.

What the run is good for is a **same-machine A/B**, which is the question the milestone actually
had to answer: does walking a part list cost more than the `switch` it replaced?

| xlarge, world draw µs | run 1   | run 2   | run 3   | mean    |
| --------------------- | ------- | ------- | ------- | ------- |
| v0.14.1 `switch`      | 2,355.6 | 1,758.3 | —       | 2,057.0 |
| v0.15 grammar         | 1,981.8 | 1,863.6 | 1,457.1 | 1,767.5 |

The grammar's mean is 14% below the `switch`'s, but the two ranges overlap and the spread within
each is larger than the gap between them. **The honest claim is that no regression is detectable,
not that the grammar is faster.** The bake is the reason it is at worst neutral: still parts are
stamped, so what the indirection costs per entity per frame is only the parts that move.

The control that makes the A/B trustworthy is the minimap, which never imports the grammar and
draws buildings as flat colour from `BUILDING_COLORS`. Across all five runs it measured 416.7,
429.8, 439.1, 425.5, and 429.8 µs — 428 µs ± 2.6%. The renderer measurement is therefore steady;
the world-draw spread is real variance in that path, and the host frame's own spread (2,385 to
3,330 µs across the same runs, on a simulation neither version touches) is what moved the complete
browser frame between runs.

### Browser frame — v0.15, third grammar run

| tier   | entities | host frame µs | world µs | minimap µs | render µs | browser frame µs | sim share | frame share |
| ------ | -------: | ------------: | -------: | ---------: | --------: | ---------------: | --------: | ----------: |
| line   |       12 |          74.5 |    398.0 |       22.7 |     420.8 |            495.3 |      0.4% |        3.0% |
| small  |      192 |         164.0 |    670.0 |       78.1 |     748.1 |            912.1 |      1.0% |        5.5% |
| medium |      768 |         361.0 |    820.0 |      117.6 |     937.6 |          1,298.6 |      2.2% |        7.8% |
| wide   |    1,536 |         731.7 |    895.7 |      160.0 |   1,055.7 |          1,787.3 |      4.4% |       10.7% |
| large  |    3,072 |       1,487.5 |  1,312.5 |      270.3 |   1,582.8 |          3,070.3 |      8.9% |       18.4% |
| xlarge |    6,144 |       2,635.0 |  1,457.1 |      429.8 |   1,886.9 |          4,521.9 |     15.8% |       27.1% |

### What this run says

**1. Nothing simulated moved, and the checksums prove it.** All six tier checksums are identical to
the v0.13.1 record — 2161174144, 1459965991, 539603397, 1469325466, 1548543730, 452398649 — even
though the crate went 0.13.0 to 0.14.0 between them, because the ladder's scenario builds no tiered
definition. For v0.15 that is the presentation-only claim demonstrated rather than asserted: the
grammar consumes snapshots and owns nothing, so it cannot reach a checksum by construction.

**2. The A/B is the finding; the absolutes are not.** See the table above and the browser note.

**3. The ladder still needs a clean re-measure.** A record comparable to v0.12.4 and v0.13.1 has to
be taken on `Chrome/151` in a composited window. Until one is, **v0.13.1 remains the current
browser-frame record** and the frame-share figures in this section describe this Electron build
only.

### Limits specific to this run

Different browser engine from every prior browser record, a non-compositing pane, and a machine
that had just run a wasm build and the full quality gate. Three samples on one side of the A/B and
two on the other. No native ladder was run: `npm run bench` is unaffected by a renderer change.

## Measured capacity tiers

Against a 16,667 µs frame at 60 Hz. `sim share` is `host frame` — the cost of advancing a tick
and merging the result. `frame share` is the complete browser frame, render included, and
exists only from v0.12.4:

| tier   | entities | sim share v0.8 | sim share v0.12.2 | sim share v0.12.4 | frame share v0.12.4 | frame share v0.13.1 | verdict     |
| ------ | -------: | -------------: | ----------------: | ----------------: | ------------------: | ------------------: | ----------- |
| line   |       12 |           0.6% |              0.4% |              0.5% |                2.2% |                2.0% | comfortable |
| small  |      192 |           2.5% |              0.8% |              0.9% |                3.3% |                2.8% | comfortable |
| medium |      768 |           8.3% |              1.9% |              2.3% |                5.8% |                4.9% | comfortable |
| wide   |    1,536 |          15.8% |              3.3% |              3.5% |                6.4% |                6.8% | comfortable |
| large  |    3,072 |          30.1% |              6.0% |              7.0% |               11.6% |               10.8% | comfortable |
| xlarge |    6,144 |          62.1% |             11.0% |             11.8% |               18.2% |               19.0% | comfortable |

**The first complete browser frame still has headroom.** The largest tier used 62.1% of a
frame for simulation alone in v0.8, 18.2% end to end in v0.12.4, and 19.0% after Look Systems.

**Neither record locates a ceiling.** What the ladder now says about the limit is only that a
complete frame at 6,144 entities fits, and extending it (follow-up 3 below) is what would say
more.

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

> **Retired by the v0.12.2 binary delta encoding.** This finding is why that encoding was built,
> and it no longer describes the shipped artifact: the crossing is 15% of a host frame at the
> largest tier and the wasm frame is 78%. See
> [Binary Delta v0.12.2](#binary-delta-v0122--the-fifth-record-and-the-first-browser-re-measurement-since-v08).

**3. The boundary cost is the payload, at about 10 µs per kilobyte.** Dividing the boundary by the
delta it carried gives 12.8, 10.8, 9.9, 9.1, and 9.7 µs/KB from `small` to `xlarge` — flat, and
tracking bytes rather than entities. (The `line` tier's 48.9 µs/KB is its ~60 µs fixed round-trip
floor spread over 1.3 KB.) This is the same 644 KB JSON payload v0.7 named as its own next target;
the browser measurement now prices it. A compact binary encoding over a transferable buffer attacks
serialization, the parse, and the copy at once, and it is the only identified cost large enough to
change the ladder.

> **Acted on, and superseded, by v0.12.2.** The encoding was built and it did change the ladder:
> the payload fell 13.6× and the boundary 21.7× at the largest tier. The per-kilobyte figure this
> finding rests on no longer holds — the crossing is now dominated by a fixed round-trip floor of
> roughly 62 µs below the `wide` tier, not by bytes.

**4. The main-thread merge is not a problem.** Applying the per-entity patch costs 3–70 µs, which
is 0.7–1.5% of a host frame above the smallest tier. The per-entity buildings delta from v0.6 is
doing its job: the host touches only what changed, and the cost of merging 6,144 entities' worth of
frames is smaller than the cost of one native checksum. Nothing here argues for changing the merge.

> **Still true in absolute terms after v0.12.2, but no longer negligible as a share.** The merge is
> unchanged code and still costs about 115 µs at the largest tier; the host frame around it fell
> 5.6×, so the same work is now 6.3% of a frame rather than 0.7%. See follow-up 9.

**5. The checksum is still the largest single cost inside wasm, and now has a cheaper rival.**
It remains 23–26% of the in-wasm frame, linear, and unchanged in character from v0.7. But it is now
9–10% of a host frame, while the boundary is roughly 60%. An incremental checksum is
determinism-critical
work for a modest share; the encoding is not. The measurement reorders them.

> **Superseded by the v0.12 native re-measurement.** v0.11's sparse tile overlay made the checksum
> 3.0× cheaper and 10% of the in-wasm frame rather than a quarter of it. The finding stood on the
> dense overlay it was measured against. Findings 1–4 are unaffected.

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

v0.12's re-measurement changes two of them: entry 4 loses most of its case, and a new entry 8
records that the browser half of this document is now stale. v0.12.4 closes entry 2.

1. ~~Replace the JSON delta with a compact binary encoding over a transferable buffer.~~ **Done in
   v0.12.2.** It did what finding 3 predicted and slightly more: payload 13.6× smaller, boundary
   21.7× cheaper, the largest tier's host frame down from 62.1% of a 60 Hz frame to 11.0%.
2. ~~Measure the Canvas renderer against the same tiers.~~ **Done in v0.12.4.** A complete
   browser frame at the largest tier is 18.2% of 60 Hz; rendering is 1,069 µs of that, 6.4% of
   the budget. The unknown 89% is gone.
3. Extend the ladder past 6,144 entities, so the record brackets a ceiling again instead of only
   showing headroom. v0.12.2 makes this more urgent, not less: it removed the cost that made 6,144
   look close to a limit, so the ladder no longer locates one at all. v0.12.4 does not change
   that — 18.2% is still only headroom.
4. ~~An incremental checksum, after the encoding.~~ **Dropped to the bottom by the v0.12
   re-measurement.** v0.11's sparse tile overlay already took 3.0× off it; it is now about a tenth
   of the in-wasm frame and a much smaller share of a host one. Determinism-critical work for a
   small and shrinking share is the wrong thing to attack.
5. Re-examine whether incremental transport recompilation should keep persistent structures across
   edits, or whether the full compile is simply the better default at these sizes. Both records show
   the incremental path costing about three times a full compile; do not remove it on that alone,
   because its tested behaviour under component splits and merges is a correctness asset.
6. Revisit the renderer itself. v0.12.4 allows this and does not demand it: Canvas 2D at 6,144
   entities is 909 µs for the world and 160 µs for the minimap. Look Systems v0.13.1 re-measured
   against this record: the world is 991 µs and a complete frame is 19.0% of 60 Hz. A 2×
   pixel-ratio viewport is still the other thing that would change the question.
7. Batch the transport recompile inside a construction drag. v0.9 routes a drag through the tested
   per-cell `place`, so a 32-cell run recompiles 32 times. It happens once when the pointer is
   released rather than every frame, and no tier in the ladder measures it, so this is a known cost
   and not yet a measured one — measure it before optimizing it, like everything else on this list.
8. ~~Re-run the browser ladder.~~ **Done in v0.12.2**, which had to measure the browser to make any
   claim about the encoding at all. The v0.8 browser tables are kept as the historical record the
   comparison is drawn against; the current browser cost is the v0.12.2 one.
9. New, from v0.12.2: **measure `apply` properly.** The main-thread merge was 0.7–1.5% of a host
   frame when the boundary dominated, and finding 4 concluded it needed no work. It is 6.3% of the
   largest tier's frame now — not because it grew, but because everything around it shrank. At
   115 µs against a 100 µs clock step it is barely above the resolution it is measured with, so
   what it needs first is a measurement that can resolve it, not an optimization.

## World Parameters v0.16 — a flat ladder, and the first measurement of generation

Same host, `factory-wasm` 0.16.0, recorded 2026-08-18. Raw report:
[`benchmarks/capacity-v0.16-native.json`](benchmarks/capacity-v0.16-native.json).

The generator changed, so the ladder was re-run under the rule that requires it. Checksums moved:
`WorldParams` is now hashed beside the seed, so this record claims **timings, not checksums**. The
workload's shape is untouched — delivered totals and entity counts match v0.13 exactly, and so do
the delta byte counts to the byte.

| tier   | entities | tick µs | snapshot µs | checksum µs | frame µs | compile µs | recompile µs |
| ------ | -------: | ------: | ----------: | ----------: | -------: | ---------: | -----------: |
| line   |       12 |     1.9 |        15.5 |         1.2 |      3.9 |        1.7 |          6.1 |
| small  |      192 |     7.9 |        58.5 |        11.6 |     48.4 |       19.2 |         60.7 |
| medium |      768 |    35.3 |       260.5 |        40.1 |    178.4 |       97.2 |        255.1 |
| wide   |    1,536 |    85.2 |       684.3 |        80.6 |    344.1 |      208.5 |        623.0 |
| large  |    3,072 |   154.6 |     1,059.5 |       158.0 |    703.6 |      428.4 |      1,204.2 |
| xlarge |    6,144 |   378.1 |     2,263.2 |       323.0 |  1,416.8 |      872.8 |      2,314.1 |

Every tier is within noise of v0.13: xlarge tick 378.1 against 361.2 µs, frame 1,416.8 against
1,455.5. **The noise floor is stated rather than hidden** — the ladder was run twice on this build
and xlarge gave tick 359.7 / frame 1,439.5 on the other run, so a 5% swing between runs is what
this host resolves and nothing smaller than that is a finding. The table is the second run, which
is the JSON on disk.

**What this says is that the parameter indirection costs nothing in the tick, snapshot, and delta
path.** What it deliberately does not say is anything about generation — the ladder's scenario sets
`generated_environment: false` and never calls `terrain_at` at all. A record that let the ladder
stand in for the generator would be citing the wrong measurement.

### Generation — the first record for this path

`terrain_at` and `field_at` now read a `WorldParams` where version 5 read literals, so the cost of
generating a hex is worth a number rather than an assurance. Measured with `survey.exe` at two
radii, five runs each, taking medians and differencing to cancel process start-up:

| radius |  hexes | median wall time |
| -----: | -----: | ---------------: |
|     48 |  7,057 |          10.6 ms |
|     96 | 27,937 |          18.0 ms |

20,880 hexes for 7.4 ms is **0.35 µs per hex**, covering terrain, field, and the survey's own
per-hex bookkeeping — so it is an upper bound on generation alone. A chunk is 64 hexes, so
generating one costs at most ~23 µs, and `ensure_neighborhood`'s seven chunks at most ~160 µs,
paid only when the player walks into unsurveyed ground.

**This is a first record, not a comparison.** No earlier milestone measured this path, so there is
no baseline to call it a regression or an improvement against. The next generator change has one.

Record new runs by adding a dated report under `docs/benchmarks/` and updating the tables above.
**Checksum comparisons are only valid while the pinned workload checksum in the Rust test gate is
unchanged; timing comparisons survive a checksum change as long as the workload's shape, entity
counts, and delivered totals do not move.** v0.12 is the first record to rely on that distinction,
which is why it states which of the two it is claiming.

Previous records:
[`benchmarks/capacity-v0.8-native.json`](benchmarks/capacity-v0.8-native.json),
[`benchmarks/capacity-v0.7.json`](benchmarks/capacity-v0.7.json),
[`benchmarks/capacity-v0.6.json`](benchmarks/capacity-v0.6.json),
[`benchmarks/capacity-v0.5.1.json`](benchmarks/capacity-v0.5.1.json).
