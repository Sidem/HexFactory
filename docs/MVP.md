# Playable Game v0.2 scope and acceptance

Status: implemented for the v0.2 release gate. The founding architecture proof is retained as the
selectable **Factory demo**; **New game** is the default live scenario.

## Playable loop

A deterministic seed places the native player beside a protected landing hub in an unbounded,
chunk-generated environment. The shipped loop is:

`explore → gather finite ore/crystal → deliver for insight → research → construct → compose → win`

Field Logistics unlocks belts, Automated Extraction unlocks finite-deposit extractors, Composition
unlocks the composer, and optional Storage Planning unlocks containers. The recipe consumes exactly
two ore for one component over eight integer ticks. Delivering three components to the landing hub
sets persistent native victory and leaves free play enabled.

## Interaction and presentation

- Six native step directions use `D/S/Q/A/W/E` for E/SE/SW/W/NW/NE. `F` gathers on the player hex
  or the facing neighbor and `X` delivers the inventory beside the hub.
- The cost- and lock-aware hotbar covers belts, extractors, composers, and containers. Build range,
  terrain, deposits, occupancy, costs, recipe selection, and technology are enforced by Rust even
  for forged commands.
- Inspect, place, erase, rotate-existing, rotate-new, play/pause, single-step, reset, four speeds,
  native New Game/Factory demo, Save, and Continue are exposed with visible labels.
- The Canvas renderer follows the player until the user pans/zooms, shows the required simulation
  layers and legality feedback, and supports desktop, narrow screens, keyboard focus, and reduced
  motion.
- Erase refunds full construction cost and all currently represented contents/reserved inputs.
  Scenario-owned hub/demo objects are protected.

## Verification coverage

Native Rust tests cover the cross-language direction protocol; same/different-seed chunk fixtures;
chunk request order; six-direction movement, facing, cadence, and blocking; finite gathering and
conservation; placement range/terrain/occupancy/cost/technology/deposit rules; exact erase refunds;
extractor depletion; research prerequisites and atomic spending; forged locked commands; the full
victory path; `HXF1` round-trip, incompatibility rejection, and resumed checksum equivalence; sorted
initial IDs and insertion independence; turning compiled paths; cargo conservation; backpressure;
exact recipe quantities/timing; container order; delivery totals; and reset/replay.

Host tests cover published geometry parity, camera-aware pan/zoom picking, bounded six-direction
keyboard input, command encoding, absence of host movement/progression mutation, dynamic definition
and technology validation, hotbar costs/locks, research prerequisites, expanded snapshot parsing,
native save delegation, responsive breakpoints, reduced motion, and accessible labels.

The release gate is npm audit, Prettier/Rust formatting, ESLint, strict TypeScript, Vitest, Rust
tests, Wasm build, and production Vite build before GitHub Pages deployment and real-browser
verification.

## Explicit follow-ups

1. Incremental connected-component graph recompilation instead of the current full small-graph
   rebuild after edits.
2. A Web Worker simulation boundary plus dirty snapshot/delta transport.
3. Benchmarked capacity tiers before selecting WebGL instancing or making scale claims.
4. Inserters, splitters, multiple lanes, power, fluids, circuits, trains, enemies, multiplayer,
   mod scripting, and evolutionary systems remain deliberately out of v0.2.
