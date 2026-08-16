# HexFactory

**[Play HexFactory v0.2](https://sidem.github.io/HexFactory/)**

HexFactory is a deliberately small, deterministic factory game on an unbounded hexagonal world. A
new game starts beside a landing hub: explore, gather finite ore and crystal, deliver items for
insight, unlock a short technology tree, build a compiled transport line, compose three components,
and win. The founding prebuilt architecture proof remains available as the **Factory demo** scenario.

Rust/Wasm owns terrain, resources, collision, player movement, inventories, costs, research,
objectives, saves, transport, machines, cargo, ticks, and checksums. TypeScript sends one bounded
input batch per rendered frame and owns only controls, camera, interface, and Canvas presentation.

## Controls

- Move in all six directions: `D` east, `S` southeast, `Q` southwest, `A` west, `W` northwest, and
  `E` northeast.
- Gather here or ahead with `F`; deliver the complete player inventory while beside the hub with
  `X`.
- Select build tools with the hotbar or number keys, rotate new buildings with `R`, and click to
  place. Inspect, erase, and rotate-existing tools are also available.
- Drag, middle-drag, or right-drag the map to pan; use the wheel to zoom and **Recenter player** to
  resume camera following.

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

- The versioned native seed generates lazy chunks independently of traversal order. Terrain,
  finite resources, placement legality, collision, and player state are native checksum inputs.
- Data files define dynamic items, recipes, buildings, costs, descriptions, icons, unlock
  requirements, and the acyclic technology graph. Native code validates and enforces them against
  forged host commands.
- Blueprint edits compile a directed transport graph. Runtime arbitration is stable by entity ID;
  rejected transfers leave their sources unchanged.
- `HXF1` saves are emitted and restored by Rust. Browser storage holds only the opaque native save
  string.
- The host consumes exactly `@hexlife/embed/hex@1.15.0` for public pointy-top axial geometry. It
  never imports HexLife source or package internals.

See the [roadmap and implementation handoff](docs/HEXFACTORY-PLAN.md),
[architecture](docs/ARCHITECTURE.md), [v0.2 acceptance](docs/MVP.md), and
[agent invariants](AGENTS.md).

No large-map performance claim is made. The next performance gates are incremental connected-
component recompilation, worker-hosted simulation with dirty snapshot deltas, and measured capacity
tiers before any renderer rewrite.

## License

[MIT](LICENSE) © 2026 Sidem
