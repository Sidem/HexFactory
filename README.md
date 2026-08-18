# HexFactory

**[Play HexFactory](https://sidem.github.io/HexFactory/)**

HexFactory is a factory-automation game in an unbounded continuous hex world, built to be fun to
play, fascinating to explore, and effortless to control. The goal is an open-ended game in the
spirit of Factorio, Satisfactory, and Minecraft — original in its own shapes and systems — and the
deterministic Rust/Wasm core exists to make a world that large stay responsive and its saves stay
exact.

What is playable today is deliberately small. Its full-viewport command surface keeps the current directive, next useful action, cargo, research,
and construction costs close to the world on desktop and touch layouts. A
new game starts beside a landing hub inside a small surveyed clearing, with the rest of the world
under fog: explore to lift it, walk across basins and highlands, gather finite ore and crystal
fields, deliver items for insight, unlock a short technology tree, build a compiled transport
line, compose three components, and win. Pick the **world** as well as the seed — Continental,
Archipelago, Highlands, or Basin, with the raw generator parameters exposed behind the preset. The
founding prebuilt architecture proof remains available as the **Factory demo** scenario.

Rust/Wasm runs inside a dedicated module worker and owns environment features, resources, collision,
continuous player movement, inventories, costs, research, objectives, saves, transport, machines,
cargo, ticks, and checksums. TypeScript sends one bounded input batch per rendered frame, applies
revision-checked native snapshot deltas, and owns only controls, camera, interface, and Canvas
presentation.

## Controls

- Move freely with `W/A/S/D` or the narrow-layout touch pad; movement is not snapped to building
  cells. Travelling past the dashed survey frontier generates new world and permanently lifts its
  fog.
- Hold `F` to keep gathering a nearby deposit; deliver the complete player inventory while beside
  the hub with `X`. Your pack holds a fixed number of stacks, so gathering — and recovering a
  building with something inside it — stops when no slot is free. Select a container and use the
  inspector's **Take** buttons to move stock back into your pack.
- Walking runs on its own cadence: it is unaffected by the simulation speed and continues while the
  factory is paused.
- Select build tools with the hotbar or number keys `1`–`9`, and click to place on the construction
  grid. With a belt or other single-hex building selected, **drag across the map to lay a whole run
  at once** — it routes itself, turning where the drag turns, and the preview shows exactly which
  cells it will use and where it stops. `E` selects erase, and dragging with it removes a run.
- `R` rotates the pending building, or the building under the cursor when no build tool is held.
  `Q` copies whatever is under the cursor, and `Ctrl`+`Z` takes back the last thing you built.
  Some definitions occupy multiple cells; those are placed one at a time. The grid appears during
  editing or via its toggle.
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

`npm run survey` reports what a world parameter set actually generates — band histogram, field
density per material, how far the landing site is from each of them, and the size of the water
bodies. A threshold is not a proportion, so this is where a preset's claims about its own landscape
come from rather than from reading its numbers.

## Architecture

- A world is a native seed **and a parameter set**, both checksummed and both saved. Feature scale
  and threshold are separate knobs: sea level decides how much water there is, and the coarse
  elevation octave's cell size decides how big it is. Resource commonness is an ordered rule table
  rather than a `match`. Axial environment chunks generate independently of traversal order; terrain
  bands and resource fields are derived, and only the depletion overlay, the surveyed chunk set, and
  ordinary simulation state are the remaining checksum inputs.
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
Rust and only the clock differs — so the record describes the artifact that ships. From v0.12.4
the browser record also times the two canvases the game draws. Every tier from 12 to 6,144
simultaneous buildings advances a tick, merges the result, and draws a 1440×900 frame inside
60 Hz, with the largest using 18.2% of one. Every browser tier reproduces its native checksum.

One Chromium version on one desktop is the whole browser evidence. No claim is made beyond the
recorded ladder. Run the ladders with `npm run bench` and `npm run bench:browser`; see
[docs/BENCHMARKS.md](docs/BENCHMARKS.md) for method, results, and limits.

## License

[MIT](LICENSE) © 2026 Sidem
