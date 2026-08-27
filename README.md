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
under fog: explore to lift it, ford rivers or bridge them for transport, cross coastline, basins, and highlands, find the fields
of ore, coal, stone, clay, and forest the world guarantees within sight of the hub, fill the hub's
posted requests for insight, make the first components at a manual workshop, then unlock powered
production and supply the foundry module. Primitive furnaces smelt plates without electricity;
manual workshops make timber and simple parts one attended batch at a time. Pick the **world** as well as the seed — Continental,
Archipelago, Highlands, or Basin, with the raw generator parameters exposed behind the preset. The
founding prebuilt architecture proof remains available as the **Factory demo** scenario.

Rust/Wasm runs inside a dedicated module worker and owns environment features, resources, collision,
continuous player movement, inventories, costs, research, objectives, saves, transport, machines,
cargo, ticks, and checksums. TypeScript sends one bounded input batch per rendered frame, applies
revision-checked native snapshot deltas, and owns only controls, camera, interface, and rendering.
The world and the minimap draw on WebGL2 with a Canvas 2D overlay for the player, labels, and
machine decorations.

Research opens a large central technology map with four independent starting branches,
icon-only nodes and prerequisite connections. Hover or focus an icon for its name, costs and effects.
Select a node to inspect unlocks, blockers and exact costs; use the separate Research button to
spend insight. Search, discipline filters, keyboard navigation, zoom and a compact list view help
explore all 19 technologies. Hover previews never replace the readable detail panel.

## Controls

- The default hotbar starts with the manual workshop and primitive furnace. Existing pinned bars
  are preserved. Load a workshop's ingredients, stand within one hex, and press **Work one batch**
  in its inspector. Walking or gathering pauses work; dismantling refunds reserved ingredients.
- Move freely with `W/A/S/D` or the narrow-layout touch pad; movement is not snapped to building
  cells. Travelling past the dashed survey frontier generates new world and permanently lifts its
  fog.
- Hold `F` to keep gathering a nearby deposit, or right-click a hex to harvest that one by name.
  Press `X` beside the hub to hand over what it has actually asked for — the posted requests and the
  contract's outstanding bill. Your pack holds a fixed number of stacks, so gathering — and
  recovering a building with something inside it — stops when no slot is free. Select a container
  and use the inspector's **Take** and **Put** rows to move stock either way, in whole or in half.
- Walking runs on its own cadence: it is unaffected by the simulation speed and continues while the
  factory is paused.
- Select build tools with the hotbar or number keys `1`–`9`, and click to place on the construction
  grid. With a belt or other single-hex building selected, **drag across the map to lay a whole run
  at once** — it routes itself, turning where the drag turns, and the preview shows exactly which
  cells it will use and where it stops. `E` selects erase, and dragging with it removes a run.
- `R` rotates the pending or selected building clockwise; `Shift+R` rotates it counter-clockwise.
  `Q` copies whatever is under the cursor, and `Ctrl`+`Z` takes back the last thing you built.
  Some definitions occupy multiple cells; those are placed one at a time. The grid appears during
  editing or via its toggle.
- Middle-drag, or hold `Shift` and drag with the left button, to pan; use the wheel to zoom. `Space`
  recentres the camera on the player and resumes following. The right button is only ever the
  harvest.
- `I` opens the cargo pack, `O` research, `B` the construction catalogue, and `P` the objective and
  controls reference. They open independently and several at a time. `T` pauses, `M` mutes, and
  `Escape` returns to inspection and clears the open panels.
- Hold `Shift` while walking to run. One hexagon is about 1 m²; the walk is 3 m/s and the run is
  5 m/s. Shallow water is a 1 m/s ford and can carry bridge-supported transport; deep water still blocks.
- `C` opens creative mode, which can also be switched on before a run starts from the title screen.
  It researches the whole tree, drops construction costs and refunds, hands you any material in the
  catalogue, and lets you widen the pack up to 240 slots. Nothing else changes: recipes, power, fuel,
  belt throughput, and hub payouts run at their usual rates, so a factory that works in a creative
  run works the same in a priced one. Switching it back off restores the prices and the refunds and
  keeps the research — a creative save is still a save, and still checksummed.

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
density per material, patch sizes and purity, how far the landing site is from each material and
from each guaranteed patch, the shape of the water, and the rivers reported apart from it. A
threshold is not a proportion, so this is where a preset's claims about its own landscape come from
rather than from reading its numbers.

## Architecture

- A world is a native seed **and a parameter set**, both checksummed and both saved. Feature scale
  and threshold are separate knobs: sea level decides how much water there is, and the coarse
  elevation octave's cell size decides how big it is. A deposit is a **site** drawn on its own
  lattice rather than a per-hex decision, which is what makes a patch one material by construction;
  the lattice is cached and the field is not. Axial environment chunks generate independently of
  traversal order; terrain bands and resource fields are derived, and only the depletion overlay,
  the surveyed chunk set, and ordinary simulation state are the remaining checksum inputs.
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

See the [goal, state, and roadmap](docs/HEXFACTORY-PLAN.md),
[construction and materials plan](docs/CONSTRUCTION-MATERIALS-PLAN.md),
[research and player progression plan](docs/PROGRESSION-PLAN.md),
[architecture](docs/ARCHITECTURE.md), [art direction](docs/ART.md),
[measured capacity](docs/BENCHMARKS.md), and [agent invariants](AGENTS.md).

**Next roadmap priority:** deliver the progression and construction plans, beginning with recipe
and research foundations, before Living Lattice, Regional Discovery or other roadmap features.
The [combined delivery sequence](docs/HEXFACTORY-PLAN.md#what-to-do-next) defines the order and gates.

## Measured capacity

Capacity is measured rather than asserted, and the measurement orders the work. The same
deterministic ladder runs natively and as wasm in the browser worker — the measurement lives in Rust
and only the clock differs — so the record describes the artifact that ships. Every tier from 12 to
6,144 simultaneous buildings advances a tick, merges the result, and draws a 1440×900 frame inside
60 Hz, with the largest using 19.0% of one, and every browser tier reproduces its native checksum.
That render figure was measured against the Canvas 2D renderer the WebGL2 pass replaced, and is owed
a re-measurement.

One Chromium version on one desktop is the whole browser evidence. No claim is made beyond the
recorded ladder. Run the ladders with `npm run bench` and `npm run bench:browser`; see
[docs/BENCHMARKS.md](docs/BENCHMARKS.md) for method, results, and limits.

## License

[MIT](LICENSE) © 2026 Sidem
