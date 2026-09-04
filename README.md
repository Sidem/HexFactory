# HexFactory

**[Play HexFactory](https://sidem.github.io/HexFactory/)**

HexFactory is an open-ended factory-automation game in an unbounded pointy-top hex world. Explore a
physical landscape, establish supply chains, and grow a deterministic factory that restores exactly.
The current build is a polished short-form slice; the live direction is in
[`docs/HEXFACTORY-PLAN.md`](docs/HEXFACTORY-PLAN.md).

## How the game works

A new game starts beside a landing hub on a dry coastal shelf. Choose a preset or edit the raw world
parameters, then survey beyond the initial clearing to reveal landforms, rivers, coast, forests, and
resource sites. The opening guarantees the materials needed to begin without turning the starting area
into a sample platter.

Gather by hand, complete finite hub requests and founding contracts, research technologies, and spend
personal Skill Points. Manual workshops and primitive furnaces establish the first production; powered
machines, extractors, storage, belts, pipes, junctions, bridges, and underpasses scale it. Multi-output
recipes can route each product separately. Buildings stop under backpressure rather than losing cargo.

Ground is physical native state. One hex is 25 m² and height changes in 0.25 m steps. Paving, roads,
walls, gates, bridges, and shaped earthworks change movement and construction. Water is generated
equilibrium plus sparse disturbances, so rivers can react to bounded edits without a permanent world tick.
Deep water becomes traversable after learning Open-water Swimming but remains unbuildable.

Rust/Wasm owns the world, player, economy, factory, legality, saves, and checksums in a dedicated worker.
TypeScript sends bounded intent and renders revision-checked snapshots through Three.js. This keeps the
simulation exact and makes idle world area nearly free. See
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the current contract.

## Controls

| Action                               | Control                                                     |
| ------------------------------------ | ----------------------------------------------------------- |
| Walk / run                           | `W A S D`; hold `Shift` to run                              |
| Click-to-move                        | Click the selected ground hex again                         |
| Gather                               | Hold `F` nearby, or right-click one hex                     |
| Deliver to hub                       | `X` beside the hub                                          |
| Select/build                         | Hotbar or `1`–`9`; click to place; drag linear construction |
| Rotate / copy / undo                 | `R` / `Q` / `Ctrl+Z` (`Shift+R` reverses)                   |
| Erase                                | `E`, then click or drag                                     |
| Shape the ground                     | `G`, then drag on the map; `R` mode, `[` `]` brush size     |
| Look / pan / zoom                    | Smooth middle-drag or arrows / `Ctrl`+middle-drag / wheel   |
| Follow player                        | `Space`                                                     |
| Pack / research / build / objectives | `I` / `O` / `B` / `P`                                       |
| Skills / creative tools              | `K` / `C` (creative games only)                             |
| Mute / close panels                  | `M` / `Escape`                                              |

Inventory interaction uses the same native cursor whether clicked or dragged: left moves a stack,
right handles half or one, `Ctrl` moves one, and `Shift` quick-moves. A refused transfer leaves its
source unchanged. Inspect a producing building to assign product outlets to exact exterior footprint sides.

Construction tools preview the same native result they commit. Belt and pipe underpasses show only their
two portals because the cells between remain available to the crossing lane. The ground brush is one held
gesture: press on the height you want to keep, then paint. Grade blends what you cross into a walkable
slope from that sampled height, Surface paints the chosen paving, and Strip lifts it again. The footprint
under the cursor is priced by the transaction that would commit it, so blocked cells stay visible and named
before the stroke reaches them, and each stamp commits on its own and undoes on its own.

The **Factory demo** scenario is a prebuilt architecture proof. **Creative mode** is chosen when a world is
created; it unlocks research and removes construction prices while keeping production, transport, fuel,
power, and hub rates unchanged.

## Saves

Named saves live in a version-independent browser catalogue and can be exported as `.hxf1`. Rust verifies
their checksum before migration. Same-generator 25 m² save formats migrate through explicit adjacent steps.
Older 1 m² worlds and different generator versions remain listed and exportable but are not remapped into a
different landscape.

## Run locally

Requirements: Node 24, Rust 1.87 with `wasm32-unknown-unknown`, and wasm-pack 0.13.1.

```bash
npm ci
npm run build:wasm
npm run dev
```

The complete local gate is:

```bash
npm run quality
```

Useful focused commands:

```bash
npm run context:check   # documentation/source context budgets
npm run survey          # world-generation and opening guarantees
npm run balance         # economy fixture
npm run bench           # native capacity ladder
npm run bench:browser   # worker, merge, and rendered-frame ladder
```

Capacity timings are machine-specific and therefore remain outside the shared quality gate. The committed
method, results, and limits are in [`docs/BENCHMARKS.md`](docs/BENCHMARKS.md).

## Project documents

- [`AGENTS.md`](AGENTS.md) — entry rules and invariants for repository work
- [`docs/AGENT-MAP.md`](docs/AGENT-MAP.md) — generated route to the smallest source index
- [`docs/HEXFACTORY-PLAN.md`](docs/HEXFACTORY-PLAN.md) — current product state and exact development order
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — current ownership and engine contracts
- [`docs/ART.md`](docs/ART.md) — generated visual language
- [`docs/BENCHMARKS.md`](docs/BENCHMARKS.md) — committed performance evidence

## License

[MIT](LICENSE) © 2026 Sidem
