# HexFactory

**[Play HexFactory](https://sidem.github.io/HexFactory/)**

HexFactory is a deliberately small, deterministic factory game in an unbounded continuous world.
Its full-viewport command surface keeps the current directive, next useful action, cargo, research,
and construction costs close to the world on desktop and touch layouts. A
new game starts beside a landing hub inside a small surveyed area, with the rest of the world under
fog: explore to lift it, gather finite ore and crystal, deliver items for insight, unlock a short
technology tree, build a compiled transport line, compose three components, and win. The founding prebuilt architecture proof remains available as the **Factory demo** scenario.

Rust/Wasm runs inside a dedicated module worker and owns environment features, resources, collision,
continuous player movement, inventories, costs, research, objectives, saves, transport, machines,
cargo, ticks, and checksums. TypeScript sends one bounded input batch per rendered frame, applies
revision-checked native snapshot deltas, and owns only controls, camera, interface, and Canvas
presentation.

## Controls

- Move freely with `W/A/S/D` or the narrow-layout touch pad; movement is not snapped to building
  cells. Travelling past the dashed survey frontier generates new world and permanently lifts its
  fog.
- Gather a nearby deposit with `F`; deliver the complete player inventory while beside the hub with
  `X`.
- Select build tools with the hotbar or number keys, rotate new buildings with `R`, and click to
  place on the construction grid. Some definitions occupy multiple cells. Inspect, erase, and
  rotate-existing tools are also available; the grid appears during editing or via its toggle.
- Drag, middle-drag, or right-drag the map to pan; use the wheel to zoom and **Recenter player** to
  resume camera following.
- Press `Space` to pause/resume and `Escape` to return to inspection and close open panels.

## Run locally

Requirements: Node 24, Rust 1.87 with `wasm32-unknown-unknown`, and wasm-pack 0.13.1.

```bash
npm ci
npm run build:wasm
npm run dev
```

The complete release gate is:

```bash
npm run quality
```

The capacity ladders sit outside that gate, because shared runners cannot produce comparable
timings. Run the native one with `npm run bench`, and the browser one with `npm run bench:browser`,
which builds the measurement-only wasm artifact and serves `/HexFactory/bench.html`.

## Architecture

- The versioned native seed generates continuous environment chunks independently of traversal order. Obstacles,
  finite resources, placement legality, collision, and player state are native checksum inputs.
- Data files define dynamic items, recipes, buildings, costs, descriptions, icons, unlock
  requirements, and the acyclic technology graph. Native code validates and enforces them against
  forged host commands.
- Blueprint edits compile a directed transport graph. Runtime arbitration is stable by entity ID;
  rejected transfers leave their sources unchanged.
- `HXF1` saves are emitted and restored by Rust. Browser storage holds only the opaque native save
  string. v0.3 intentionally rejects incompatible v0.2 saves.
- The worker advances commands and ticks in order and returns native dirty snapshot groups. Static
  terrain and resource arrays do not cross the worker boundary when unchanged, and buildings cross
  as a per-entity patch of changed and removed entities.
- Fog covers world the simulation has not generated. It is drawn from the native chunk bounds in
  each snapshot, so exploring — not a host-side reveal rule — is what lifts it.
- The host consumes exactly `@hexlife/embed/hex@1.15.0` for public pointy-top axial geometry. It
  never imports HexLife source or package internals.

See the [roadmap and implementation handoff](docs/HEXFACTORY-PLAN.md),
[architecture](docs/ARCHITECTURE.md), [current acceptance](docs/MVP.md),
[measured capacity](docs/BENCHMARKS.md), and [agent invariants](AGENTS.md).

## Measured capacity

Capacity is measured rather than asserted, and the measurement orders the work. The same
deterministic ladder now runs natively and as wasm in the browser worker — the measurement lives in
Rust and only the clock differs — so the record finally describes the artifact that ships. In the
browser, every tier from 12 to 6,144 simultaneous buildings advances a tick and merges the result
inside a 60 Hz frame, with the largest using 62% of one. Every browser tier reproduces its native
checksum, so the two records compare directly.

The measurement answered the question three releases had deferred. The wasm engine is not the
limit: it costs about 1.2× native, so the earlier native work transferred intact. The worker
boundary is — it accounts for roughly 60% of what a frame costs the host and scales with payload at
about 10 µs per kilobyte, which is why a compact binary delta encoding is the next milestone rather
than another simulation optimization.

Rendering is not included in any of this, and one Chromium version on one desktop is the whole
browser evidence. No claim is made beyond the recorded ladder. Run the ladders with `npm run bench`
and `npm run bench:browser`; see [docs/BENCHMARKS.md](docs/BENCHMARKS.md) for method, results, and
limits.

## License

[MIT](LICENSE) © 2026 Sidem
