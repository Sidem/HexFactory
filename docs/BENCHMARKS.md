# HexFactory capacity benchmarks

Capacity is measured, never asserted, and the measurement orders the work. Every number here came
from the committed harness; the raw reports live in `docs/benchmarks/` and are the source for
anything trimmed out of this file.

**Current records.** Native and browser frame: **v0.43.0**. Generation: **v0.21**. Payload:
**v0.12.2**. Phase 8 ground, water and erosion: the slice records below.

## Running the ladders

```bash
npm run bench
```

```bash
npm run bench:browser
```

`--quick` runs a reduced ladder, `--json <path>` writes the machine-readable report. The browser
command builds the harness and starts the dev server; open `/HexFactory/bench.html` and press **Run
full ladder**. `npm run survey`, `npm run terra`, `npm run water` and `npm run erosion` reproduce the
generation, drainage, water and erosion records.

Neither ladder is part of `npm run quality` — shared CI runners do not produce comparable timings.
The gate instead pins the workload checksum and asserts the harness still runs. The harness compiles
into wasm only under `--features bench`, so the deployed artifact never carries it.

## The workload

Six tiers of one shape, differing only in how many lines run at once:

`extractor → 6 belts → composer → belt → container → belt → consumer`

Each line sits on its own inexhaustible deposit and the consumer always accepts, so a tier reaches
steady state and stays there. The bench catalogue pins its own historical `2 ore -> 1 component`,
eight-tick, fuel-free recipe: gameplay moved to plates and gears, and applying that bill to this
one-input line would measure a stalled factory.

`tick` is one simulation tick alone; `frame` adds a bounded command batch and one encoded delta;
`snapshot` is one complete native snapshot, built only for the host's first frame since v0.7 and
kept as the baseline the incremental delta is measured against. Browser-only metrics are what a
native run cannot see: `round trip` (postMessage, buffer transfer, decode, both scheduling hops),
`apply` (`applySnapshotDelta` on the main thread), `world` and `minimap` draws, and `browser frame`
as the end-to-end total.

Only the sample budgets differ between the two records. A browser clamps `performance.now` to 100 µs
unless cross-origin isolated, so each phase repeats its block until it has run at least 20 ms; every
metric is a mean, so the two stay comparable, and each tier's checksum comes from a core advanced
exactly once through its tick budget.

## Native — v0.43.0, reference desktop

Ryzen 7 5800X / Windows 11, shipped release profile (`opt-level = "s"`, LTO, `wasm-opt -Oz`).
[`capacity-v0.43-native.json`](benchmarks/capacity-v0.43-native.json).

| tier   | entities | tick µs | snapshot µs | checksum µs | frame µs | compile µs | recompile µs | edit µs |
| ------ | -------: | ------: | ----------: | ----------: | -------: | ---------: | -----------: | ------: |
| line   |       12 |     0.9 |        28.3 |         1.8 |     15.4 |        4.8 |          8.6 |    16.1 |
| small  |      192 |     8.0 |        95.4 |        16.0 |     52.7 |       44.8 |        109.4 |   139.4 |
| medium |      768 |    37.0 |       483.9 |        54.6 |    199.6 |      211.7 |        432.4 |   405.4 |
| wide   |    1,536 |    79.8 |       827.3 |       119.8 |    368.5 |      447.1 |        919.4 |   954.8 |
| large  |    3,072 |   134.0 |     1,753.8 |       202.2 |    687.9 |    1,027.7 |      1,894.6 | 2,174.8 |
| xlarge |    6,144 |   286.9 |     3,488.0 |       411.9 |  1,372.2 |    1,906.1 |      4,045.5 | 3,887.8 |

The xlarge tick is 1.7% of a 60 Hz frame and the complete in-wasm frame 8.2%; at the shipped 10 tps
the tick is not the player-facing limit on this machine. This record says the work fits — it does not
say the scheduler is optimal. Every runtime-indexed machine, power participant and transport source
is still visited each tick, and power allocation builds ordered groups each time.

## Browser frame — v0.43.0, three profiles

Same desktop, Chromium 151, 1440×900 at DPR 1, 178 px minimap. Raw reports:
[Low](benchmarks/capacity-v0.43-browser-low.json),
[Medium](benchmarks/capacity-v0.43-browser-medium.json),
[High](benchmarks/capacity-v0.43-browser-high.json).

| profile | 12    | 192   | 768     | 1,536   | 3,072   | 6,144           |
| ------- | ----- | ----- | ------- | ------- | ------- | --------------- |
| Low     | 633.0 | 827.3 | 1,443.4 | 2,521.7 | 3,198.2 | 5,386.4 (32.3%) |
| Medium  | 628.4 | 748.2 | 1,639.3 | 2,012.4 | 2,978.0 | 5,576.5 (33.5%) |
| High    | 845.4 | 641.7 | 1,104.8 | 1,835.3 | 3,425.9 | 5,656.0 (33.9%) |

Complete browser frames in µs; the percentage is the share of a 60 Hz frame.

**All three pass the 35% desktop gate with only 1.1–2.7 points to spare.** That is a pass, not
headroom: one desktop, one browser, a 100 µs clock step. A milestone that adds a permanent visual
bucket adds its own workload tier and repeats all three profiles first.

**Draw calls stay 34–36 from 12 through 6,144 entities**, with 36 geometries, one or three textures
and 2.55 M triangles at xlarge. Instances rather than entity count own submission. The reference
desktop is the support target (decided 2026-08-27); integrated-GPU laptop qualification was
withdrawn, not deferred.

## Payload — v0.12.2 binary delta

The delta crosses the worker boundary as a compact buffer that is transferred, not structured-cloned.
Both columns are the same frames measured both ways.

| tier   | entities | delta bytes | json bytes | ratio |
| ------ | -------: | ----------: | ---------: | ----: |
| line   |       12 |         104 |      1,319 | 12.7× |
| medium |      768 |       5,803 |     79,477 | 13.7× |
| xlarge |    6,144 |      47,531 |    644,759 | 13.6× |

Varints instead of decimal text, one byte per closed-set enum, delta-coded ids and tile coordinates,
one bit per absent option. It cost 3.7 KiB of shipped wasm, `snapshot_delta_json` kept as its oracle
included. The boundary went from 57–61% of a host frame to a fixed ~62 µs round-trip floor: below
`wide`, sending less now buys almost nothing.

## Generation — v0.21 site lattice

`survey.exe` on `continental`, five runs, medians differenced to cancel start-up. Both builds
measured in one session, because the survey itself grew reporting between them.

| build   | radius 48 (7,057) | radius 96 (27,937) | µs per hex |
| ------- | ----------------: | -----------------: | ---------: |
| v0.20.1 |           15.2 ms |            26.0 ms |       0.52 |
| v0.21   |           20.7 ms |            50.3 ms |       1.42 |

2.7× against the model it replaced, and it buys one material per patch. A chunk is 64 hexes, so
generating one costs ≤ 91 µs and `ensure_neighborhood`'s seven chunks ≤ 640 µs — about 4% of a 60 Hz
frame, paid only when the player walks into unsurveyed ground and never in the tick. The figure is an
upper bound: it includes the survey's own bookkeeping.

**Purity is what decided the site model.** Purity is the share of resource hexes whose radius-1 disc
holds one material — whether an extractor works a field or straddles two. Target 950. Across the four
presets it rose from 474–662 to 965–992. Before it, stone had no workable patch anywhere in 26,307
land hexes on `continental`, and neither did wood on `archipelago`.

**Still unmeasured:** the site cache's hit rate under a walk rather than under a survey that sweeps a
disc in lattice order. If chunk generation ever appears in a frame, look there first.

## Phase 8 — ground, water and erosion

Seed 1213486160, world generators 14 and 15, release-native.

**The graded long profile, measured.** World 13 routed channels with a minimum spanning tree whose edge
cost was a hash four orders of magnitude larger than its climb term, and cut a constant depth under the
noise field, so a reach could climb. World 14 floods the node lattice by priority, carries a water-surface
elevation per reach, and cuts the bed to that elevation. Same seed, same nine provinces:

|                                   | inland, world 13 | inland, world 14 | coast, world 13 | coast, world 14 |
| --------------------------------- | ---------------- | ---------------- | --------------- | --------------- |
| springs                           | 6                | **12**           | 3               | **4**           |
| lakes / lake cells                | 107 / 718        | **7 / 47**       | 163 / 3,169     | **14 / 454**    |
| walks ending in a lake            | 463 of 576       | **293**          | 346 of 576      | **255**         |
| walks reaching the sea or leaving | 113              | **283**          | 230             | **321**         |
| walkable / buildable per mille    | 971 / 747        | **996** / 723    | 1000 / 959      | 1000 / 931      |
| viewport relief                   | 54.2 m           | 53.7 m           | 21.7 m          | 21.0 m          |
| solve                             | 69 ms            | **62 ms**        | 61 ms           | **53 ms**       |

Closed basins fell 15× and half again as much water leaves the region instead of pooling in it, for less
solve time. Buildable ground pays 24 per mille: a valley side is one `MAX_WALK_STEP_QUANTA` per cell by
construction, which is walkable and deliberately not buildable.

**The grade line descends to 15 mm.** A new invariant counts flow edges between two channel cells whose
water surface rises downstream: 0 of 1,915 at the coast, 4 of 2,411 inland, worst 15 mm — a sixteenth of a
height quantum, and integer rounding rather than a rise. It is reported but not gated, because a confluence
joins two reaches cut to different depths and rounding there is not a falsification.

**Two more negative results from this round.**

- **Do not fade the surface texture back into the valley sides.** Clamping the carve to a bare ramp costs
  rolling ground, so `texture_mq` was faded in with distance from the thread. Valley-side roughness traps
  drainage: lakes went 7 → 49 and walks leaving the sample 292 → 36. Reverted. A graded valley is smooth
  because that is what letting the water out costs.
- **The bank grade is the constant that decides how much world a river flattens.** At 2,500 milli-quanta
  per cell the ramp climbs 15 m before the width cap stops it, which planed the shipped opening disc flat:
  rolling ground fell 281 → 8 per mille and `highlands` lost its coal patch (largest 4 hexes against a
  floor of 19). At 6,000 the banks cost 55 per mille of walkable ground and bought no drainage. 4,000 —
  exactly `MAX_WALK_STEP_QUANTA` — gives 86 per mille rolling ground, 996 walkable, and the coal back at 51.

### World 15 — the rock decides the cross-section

Erodibility in world 14 was one constant, so every valley on every seed had the same walls and every bed
cut to exactly its grade line. World 15 gives the ground a strength: a banded field, soft in a weathered
mantle near the surface and harder in the bed beneath, and the incision stops where the discharge class's
cutting power meets it. A reach that cannot reach its grade line leaves a **sill** — a step in the bed the
water crosses and a walker may not. Same seed, same nine provinces:

|                                   | inland, world 14 | inland, world 15 | coast, world 14 | coast, world 15 |
| --------------------------------- | ---------------- | ---------------- | --------------- | --------------- |
| springs                           | 12               | 12               | 4               | 4               |
| lakes / lake cells                | 7 / 47           | **11 / 58**      | 14 / 454        | **14 / 415**    |
| walks ending in a lake            | 293 of 576       | **373**          | 255 of 576      | **248**         |
| walks reaching the sea or leaving | 283              | **203**          | 321             | **328**         |
| walkable / buildable per mille    | 996 / 723        | **946 / 731**    | 1000 / 931      | **968 / 945**   |
| channel cells on a sill           | 0                | **48**           | 0               | **0**           |
| edges falling past the wade limit | 0                | **16**           | 0               | **6**           |
| viewport relief                   | 53.7 m           | 53.7 m           | 21.0 m          | 21.0 m          |
| solve                             | 62 ms            | 82 ms            | 53 ms           | 73 ms           |

Relief is unchanged and the invariants still hold — zero cycles, zero uphill edges, 1 of 2,537 inland flow
edges rising by 31 mm, an eighth of a quantum. What moved is inland walkable ground, 50 per mille of it,
which is what a hard bed buys: sixteen crossings a walker must go around and forty-eight steps in the bed
that were flat before. The coast has no sills at all, because a channel already at sea level has nothing
left to cut.

**Two negative results this round, both from measuring the mechanism on its own.**

- **The bank grade may not go below world 14's 4,000 milli-quanta.** The soft end of the new span was
  tried at 3,000, which reads better in cross-section — a soft mantle should give a wider valley. It cost
  the shipped `continental` opening its coal: largest patch 36 → 16 hexes against a floor of 19. Deposits
  sit on bands the relief decides, so widening every valley planes them away. The span is 4,000..6,400,
  keeping world 14's single constant as its floor, so no valley is wider than one already shipped. The
  6,400 ceiling is what the 25 per mille of inland walkable ground above is spent on.
- **Sills are almost free, and were measured that way before being kept.** With the same bank span and
  `cut_power` raised until no reach can be stopped, the inland sample gives 945 per mille walkable, 7 lakes
  over 45 cells, 0 sills, 14 falls — and 373 walks to a lake, 203 off the sample, the same two numbers as
  the shipped configuration. The mechanism costs four ponds and changes no drainage-walk termination at
  all. It was kept for the crossings it creates, not for anything it does to the water.

**Drainage holds by construction, not by tuning.** `npm run terra` reports zero cycles, zero uphill
edges and zero unterminated walks in both the inland and `--coast` samples: head is a pure global
field and flow is steepest descent on it under a total coordinate order. Solve cost is 5–6 ms per
province, 460–486 ms for 81 provinces over 1.3 M cells. Sample the coast, not the origin — a
statistic about river mouths taken where there is no sea is a wrong result, not a weak one.

**Two negative results, kept because each cost a round and would otherwise be retried.**

- **Do not round the height field to quanta before depressions are resolved.** A whole-quanta field
  put 216 per mille of neighbour pairs at exactly equal height and manufactured 32,694 micro-lakes.
  Carrying it in milli-quanta internally and publishing whole quanta cut that 23×, to 1,409.
- **Do not carve the whole flow tree to remove closed basins** while incision is a constant depth per
  class: lakes rose 2.1×, lake cells 3.1× and solve cost doubled, because a thread crossing rising
  ground leaves a flat-bottomed trench with a lip. The graded long profile is what worked, and world
  14 above is that result.

**A viewport is the unit "the world looks flat" was about**, not a slope histogram: uncorrelated
centimetre noise and a hillside give similar neighbour steps, and only one accumulates over the
distance the camera frames. Measured, the world carries **54.2 m of relief across 429 m**.

- **Do not retune generation amplitude.** Raising `HILLSLOPE_QUANTA` with a meso-scale ridge octave
  took viewport relief _down_ 53.5 → 49.7 m while buildable ground fell 64 per mille. Rejected; the
  rejection is recorded on the constant in `factory-wasm/src/terra.rs`.
- **What was flat was the material map.** The old substrate rule chose Soil above 600 quanta, which
  the continental field clears almost everywhere, so one clause painted 889 per mille of the world.
  World 12 reads the gradient across a three-cell stencil instead: lowland 663, hills 214, shore 48.
- **Rivers are obstacles deliberately.** A reach's water surface stands 1.5–3.5 m below the ground it
  was routed over and its bed 0.5–2 m below that, so class 3 and up passes `WADE_LIMIT_QUANTA`: a
  small stream is a ford and a large one is not. The bank grade is past `MAX_BUILD_STEP_QUANTA` by
  construction. Both cost buildable ground and both were taken on purpose.

**Water work is paid by the disturbance that woke it.** `npm run water`: a 32-quanta command reached
a fixed point over 41 active cells in 53 sweeps and 40 transfers, and 31 quanta that reached the
surveyed frontier were retained against their named continuation cells. A settled world advanced
100,000 ticks with **no water dirty mark and no change to the departure set** — the two false fields
are the measurement, not the timer. There is no per-cell standing-water tick.

**Erosion is bounded, not budgeted-by-timer.** `npm run erosion`: one epoch inspected 121 chunks,
7,744 cells and 1,086 wet flowing edges, finding 117 bends; the first outside bank moved one quantum
after eight accelerated epochs and the paired inside bank received one. The claim is the structural
bound — at most 256 chunks, 65,536 cells, 4,096 edges and 64 bank changes per epoch in a
deterministic rotating window — and this run reached none of them. Straight, dry and protected
reaches do no work at an epoch at all.

## Limits

- **One browser, one shell, one machine.** No Firefox, Safari, mobile or low-core figure exists.
- **One workload shape.** Uniform straight lines with an always-accepting sink: no
  backpressure-saturated networks, long turning runs, dense multi-cell packing or deposits running
  dry. Dirty tracking is measured near its worst case (~43% of entities change every tick).
- **One run per tier, no confidence interval.** Treat differences under ~20% as noise. The `line`
  tier's compile, recompile and edit figures move by more than that between runs.
- **The camera follows the player** in the browser record; off-screen entities are clipped, not
  drawn. A zoomed-out view or DPR 2 is a different measurement.
- **`apply` is the coarsest number here**, against a 100 µs clock step — treat as ±10%. Round trip
  and merge are timed in separate passes; the game interleaves them.
- **The ladder locates no ceiling.** Every tier fits inside a 60 Hz frame; "above 6,144 entities" is
  the whole of what it says about the limit.
- **A checksum change invalidates checksum comparisons, not timing ones.** Several releases moved the
  pinned workload checksum without changing the workload's shape, entity counts or delivered totals.
  Say which of the two a record claims. Likewise the v0.24 and v0.25 browser records are historical
  baselines, not current renderer evidence.

## Follow-ups, in the order the measurement supports

1. **Add a milestone's workload before adding its visual cost.** Stacked floors, lifts and
   cross-level graph edges need a deterministic tier and all three profiles repeated; xlarge leaves
   1.1–2.7 points under the gate.
2. **Measure the scheduler under the shapes the game now owns** — junction-dense, backpressured and
   power-dense tiers. The straight always-accepting line cannot say whether active sets or reused
   scratch would pay.
3. **Measure the site cache under a walk**, not under a lattice-order survey sweep.
4. **Batch the transport recompile inside a construction drag.** A 32-cell run recompiles 32 times.
5. **Re-examine incremental recompilation.** At xlarge the affected-component path costs 4.05 ms
   against 1.91 ms for a full compile — but the tier is one connected line, and the incremental
   path's tested behaviour under splits and merges is a correctness asset. Add affected-size
   reporting before touching it.
6. **Extend the ladder past 6,144** only once a new workload shape exists; more copies of the same
   line do not locate a ceiling.
