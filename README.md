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

Capacity is measured rather than asserted, and the measurement orders the work. On the recorded
native host, a worker frame now stays within a 60 Hz budget through 3,072 simultaneous buildings —
up from 1,536 — after v0.6 gave extractors a resolved deposit reference instead of a per-tick scan
over every generated tile (233× cheaper ticks at the largest tier) and made the buildings delta
per-entity instead of per-group (2.3× less payload at every tier). Every tier reproduces its
previous checksum, so the two records compare directly. The new measurement names its own next
target: a complete snapshot is still materialized every frame purely to diff it, which is now most
of the frame.

These are native figures. No browser performance claim is made, and no claim is made beyond the
recorded ladder. Run the ladder with `npm run bench`; see [docs/BENCHMARKS.md](docs/BENCHMARKS.md)
for method, results, and limits.

## License

[MIT](LICENSE) © 2026 Sidem
