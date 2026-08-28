# v0.41.0 — Handling and Clarity

Phase 5 completes the work interrupted in the Claude and Grok sessions. Phase 6, straight walls
and gates, remains next; no later phase was started.

## Delivery

- Pointer drags reuse native pickup/place commands. Both queue together or neither does. Outside
  drops, Escape, and pointer cancellation leave the source alone. A new press cannot inherit a
  swallowed click from an earlier drag. Source addresses are frozen at the press; slot identity
  survives quantity changes. Click, right-click, Ctrl and Shift quick moves remain available.
- Selecting an accepting stock compartment offers the pack beside the inspector on wide screens.
  Explicit pack closure declines further offers for the session. Ordinary world-click panel closure
  is not a decline, and narrow layouts keep the inspector visible instead of opening over it.
- Static native delivery capability rejects belts pointing into poles, extractors or pumps and
  omits dead graph edges. A bare bridge remains a future belt support. Full storage and unconfigured
  composers remain connectable. The drag preview carries the native refusal and target coordinates.
- Demolition asks once for stored stock or an active batch, including whole removal sweeps. Sweeps
  request a final native preview for their released endpoints. Native carries what fits and spills
  the rest in stable item order. The confirmation names the **about one minute** ground-item timer;
  this is 600 simulation ticks, so simulation speed affects wall-clock lifetime. Walk over spills
  with free pack space after the existing 30-tick pickup delay. Creative refund rules are unchanged.
- Six paving materials use world coordinates, full-radius caps and no per-cell tint. Timber courses,
  brick bonds, concrete joints, earth, gravel and asphalt continue across cell boundaries. Material
  detail follows the existing quality profile; geometry changes only with published ground state.

No save, definitions, technologies, scenarios, world or wire version changed: **32 / 26 / 14 / 7 /
10 / 17**. The optional drag-preview reason is JSON RPC metadata, not snapshot-wire state.

## Verification

Focused regressions cover atomic queue admission, keyed compartment identities, explicit panel
reveal, asynchronous dialog close/reopen, static versus temporary delivery refusal, overflow
conservation, save/load checksum preservation, and later collection. Existing full-vs-dirty,
transport, save migration, balance and geometry suites remain in the quality gate.

Browser session, Windows Chromium 151, Low profile:

1. Loaded a native-generated save with full carrying slots and a full 12-unit container.
2. Selected the container and observed the pack opening. Ctrl-dragged one crystal into the pack and
   back, then released a full stack outside both grids: storage stayed at 12. The next Ctrl-click
   moved one item, and click-place restored it.
3. Dismissed the demolition dialog, reopened it and accepted. The dialog named 12 crystals and the
   expiry timer; native reported eight overflow items. Dropped two carried ore stacks to make room,
   walked over the demolition site and recovered five crystals and three timber.
4. Rendered every paving material in the focused 625-cell Low harness. Timber and asphalt were
   inspected at ordinary zoom with no internal hex lattice. Browser logs contained no warnings or
   errors during these checks.

## Measurement and limits

Run `npm run dev`, open `/HexFactory/paving-bench.html`, and press **Measure all surfaces**.
The committed harness uses the production renderer with a synthetic level yard, 60 warmup frames
and 240 measured frames per material at 1200×720, Low, DPR 1. Raw results are in
[`benchmarks/handling-clarity-paving-low.json`](benchmarks/handling-clarity-paving-low.json).

CPU render submission averaged **249–303 µs** for paved cases, versus 318 µs for the untreated
fixture; p95 was 300–400 µs. Each single-material yard added one draw call (12 to 13), one geometry
(19 to 20) and no texture (one throughout). RAF p95 intervals were 16.7–16.8 ms.

These are observations from one desktop run, not an improvement claim: clock quantization, run
order and other activity can move these small numbers. This is neither GPU completion timing nor
the full worker/browser capacity ladder. It does not establish mobile performance or a scale ceiling.
No physical-touch or screen-reader audit is claimed. Older QA saves already rejected by v0.40.0
were not rewritten or repaired; current-envelope round trips are covered.
