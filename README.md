# HexFactory

**[Open the live HexFactory MVP](https://sidem.github.io/HexFactory/)**

HexFactory is a deterministic browser-native architecture proof for factory automation on an
unbounded hexagonal map. A Rust/Wasm core runs a real resource → turning belts → composer → container
→ consumer chain; TypeScript supplies controls and a replaceable Canvas 2D view.

The initial slice demonstrates compiled directional transport, dynamic item/recipe/building IDs,
integer machine timing and quantities, true inventories, backpressure, stable arbitration,
checksums, reset/replay, and basic editing. It does not claim large-scale performance yet.

## Run locally

Requirements: Node 20+, Rust, the `wasm32-unknown-unknown` target, and wasm-pack 0.13.1.

```bash
npm ci
npm run dev
```

The first `npm run dev` expects `factory-wasm/pkg`; build it with `npm run build:wasm`. Production:

```bash
npm run quality
```

## Architecture

- Rust/Wasm owns all simulation work and compiles placed tiles into a directed transport graph.
- TypeScript sends bounded commands, reads snapshots, and uses exactly
  `@hexlife/embed/hex@1.15.0` for public pointy-top axial geometry.
- Items, recipes, buildings, orientation, cargo, inventories, and progress remain separate data
  dimensions.
- The same Rust core builds and tests natively without DOM or package internals.

See [architecture](docs/ARCHITECTURE.md), [MVP acceptance](docs/MVP.md), and [agent invariants](AGENTS.md).

## License

[MIT](LICENSE) © 2026 Sidem
