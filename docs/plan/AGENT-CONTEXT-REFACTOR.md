# Agent-context refactor — task board

Drop this at `docs/plan/AGENT-CONTEXT-REFACTOR.md`.

Each task below is self-contained: preconditions, commands, done-when, and a token
budget. A fresh agent should be able to pick up any unblocked task, read only this file
plus the files the task names, and finish it in one session. **One task per session.**
Do not carry context between tasks — write the handoff line and start clean.

Baseline measured at `v0.46.0`:

```
always-paid agent context   69 KB   ~19.7k tokens
factory-wasm/src/lib.rs   1,153 KB   (616 KB production, 465 KB tests, 71 KB bench)
src/main.ts                 232 KB   282 top-level decls
comments                    33% of the Rust crate
budget failures                15 files
```

## Board

| #   | Task                                          | Blocks on | Risk   | Est. tokens |
| --- | --------------------------------------------- | --------- | ------ | ----------- |
| T00 | Land the two tooling scripts                  | —         | none   | 5k          |
| T01 | Waiver file + wire into `quality`             | T00       | none   | 5k          |
| T02 | Cut root `AGENTS.md` to invariants            | —         | none   | 15k         |
| T03 | Per-directory `AGENTS.md` files               | T02       | none   | 25k         |
| T04 | Route-table `AGENT-MAP.md` + `.agent/`        | T02       | none   | 30k         |
| T05 | Move `#[cfg(test)] mod tests` out of `lib.rs` | T00       | none   | 10k         |
| T06 | Split the extracted tests by domain           | T05       | none   | 15k         |
| T07 | Bench modules to `src/bin/`                   | T05       | low    | 10k         |
| T08 | `pub(crate)` pass on `struct Core`            | T07       | low    | 10k         |
| T09 | Author the `impl Core` split map              | T08       | none   | 15k         |
| T10 | Execute the split, one module at a time       | T09       | medium | 60k         |
| T11 | `impl Factory` → `api.rs`                     | T10       | low    | 10k         |
| T12 | Residual `lib.rs` types → `core/types.rs`     | T10       | medium | 40k         |
| T13 | Comment → ADR pass (opportunistic)            | T10       | low    | ongoing     |
| T14 | `index.html` id manifest                      | —         | low    | 25k         |
| T15 | Split `styles.css`                            | T14       | low    | 25k         |
| T16 | `AppContext` type for `main.ts`               | —         | medium | 40k         |
| T17 | Convert renderers to take `ctx`, in place     | T16       | medium | 150k+       |
| T18 | Move renderer clusters to `src/panels/`       | T17       | low    | 40k         |
| T19 | Input + lifecycle out of `main.ts`            | T18       | medium | 50k         |
| T20 | Repo hygiene: PNGs out of root                | —         | none   | 5k          |

T02–T04, T14–T15 and T20 are independent of the Rust work and can run in parallel.

---

## T00 — Land the tooling scripts

Copy `scripts/rust-split.mjs` and `scripts/context-budget.mjs` (supplied alongside this
plan) into `scripts/`.

```bash
node scripts/context-budget.mjs                              # expect 15 failures
node scripts/rust-split.mjs inventory factory-wasm/src/lib.rs --impl Core   # expect 235 methods
```

**Done when** both run and report the numbers above.
**Do not** run `--check` in CI yet — T01 does that.

---

## T01 — Waiver file, then wire the gate in

Create `.agent-budget.json` listing every current failure as debt:

```json
{
  "allow": {
    "factory-wasm/src/lib.rs": "T05-T12",
    "src/main.ts": "T16-T19",
    "factory-wasm/src/balance.rs": "backlog",
    "factory-wasm/src/terra.rs": "backlog",
    "factory-wasm/src/hydrology.rs": "backlog",
    "factory-wasm/src/wire.rs": "backlog",
    "factory-wasm/src/ground.rs": "backlog",
    "src/rendering/three/worldInstances.ts": "backlog",
    "src/styles.css": "T15",
    "index.html": "T14",
    "tests/host.test.ts": "backlog",
    "tests/visualDepth.test.ts": "backlog",
    "docs/HEXFACTORY-PLAN.md": "T04",
    "docs/AGENT-MAP.md": "T04",
    "AGENTS.md": "T02"
  }
}
```

Add to `package.json`:

```json
"context:budget": "node scripts/context-budget.mjs",
"context:check": "node scripts/context-budget.mjs --check",
```

and insert `npm run context:check` into `quality`, right after `audit`.

**Done when** `npm run context:check` passes with waivers and fails if you delete one.
**The rule from here on:** every task that fixes a file deletes its waiver line. The
waiver list is the burndown.

---

## T02 — Cut root `AGENTS.md` to invariants

Current: 9,220 B, containing release history, save-envelope numbers, world migration
rules, movement figures and dates. None of it is needed to fix a CSS bug, and all of it
is paid on every task.

Target ≤ 3 KB: the simulation-authority rules, the navigation procedure, the validation
commands. Everything historical moves to `docs/plan/current.md` (current phase only) and
`docs/decisions/` (rationale). Git already holds the chronicle.

**Done when** root `AGENTS.md` ≤ 3 KB and its waiver is deleted.

---

## T03 — Per-directory `AGENTS.md`

Nested `AGENTS.md` files load when the agent works inside that subtree, so domain rules
should live there rather than in the root file or behind a link the agent may not follow.

Create/expand: `src/ui/AGENTS.md`, `src/rendering/AGENTS.md`, `src/core/AGENTS.md`,
`factory-wasm/src/AGENTS.md`, `tests/AGENTS.md`. Each ≤ 3 KB, each covering only rules
that are wrong to apply outside that directory.

**Done when** every rule removed in T02 has a home, and `context:budget` shows no
`AGENTS.md` over 3 KB.

---

## T04 — Route table + `.agent/` maps

`docs/AGENT-MAP.md` is 60 KB — a document whose purpose is to stop agents opening large
documents. Change `scripts/agent-map.mjs` to emit:

- `AGENT-MAP.md` at repo root, ≤ 4 KB: a two-column table of task → route file. Nothing else.
- `.agent/<domain>.md` per route, each ≤ 8 KB, listing only that domain's files,
  declarations and line numbers.

The `routes` array already in `agent-map.mjs` is the domain list — reuse it verbatim.
Keep `--check` staleness detection.

**Done when** a UI task never sees a Rust declaration, and both waivers are deleted.

---

## T05 — Tests out of `lib.rs`

Zero risk: unit tests keep private access as child modules, so no `pub` changes and no
API surface change.

```bash
cd factory-wasm/src
mkdir -p tests
sed -n '15922,26619p' lib.rs > tests/mod_body.rs      # verify the range first
```

Confirm line 15921 is `#[cfg(test)]` and 15922 is `mod tests {`. Then:

- `tests/mod.rs` = the body of the old `mod tests` block, minus the outer braces.
- In `lib.rs`, replace lines 15921–26619 with `#[cfg(test)]\nmod tests;`.
- Move `petroleum_tests.rs` to `tests/petroleum.rs` and update its `mod` line.

```bash
cargo test --manifest-path factory-wasm/Cargo.toml
```

**Done when** the same test count passes and `lib.rs` is ~715 KB.
**Do not** read the test bodies. This is a byte move.

---

## T06 — Split the extracted tests by domain

`tests/mod.rs` is now ~465 KB. Split into `tests/{transport,world,power,save,player,
ground,hydrology,graph}.rs` using the `mod`/`#[test]` groupings already present, with
`tests/mod.rs` reduced to `mod transport; mod world; …`.

Target: no test file over 50 KB.

**Done when** `cargo test` count is unchanged and the test waivers can start coming off.

---

## T07 — Bench modules to `src/bin/`

`pub mod water_bench` / `erosion_bench` / `capacity` / `survey` occupy lines
14,163–15,920 (71 KB) and are already invoked as `--bin` targets.
`factory-wasm/src/bin/` already exists.

Move each to its own file under `src/bin/`, add `[[bin]]` entries to `Cargo.toml` if
autodiscovery does not pick them up, keep the `bench` feature gate.

```bash
npm run bench -- --help && npm run survey -- --help
```

**Done when** all four binaries still run and `lib.rs` is ~644 KB.

---

## T08 — `pub(crate)` on `struct Core`

`struct Core` starts at line 2,468. All fields are module-private, which is the only
thing preventing `impl Core` methods from living in sibling modules.

One mechanical pass: prefix every field with `pub(crate)`. Preserve the doc comments
exactly — T13 handles those separately.

```bash
cargo check --manifest-path factory-wasm/Cargo.toml
```

**Done when** the crate compiles unchanged.

---

## T09 — Author the split map

```bash
node scripts/rust-split.mjs inventory factory-wasm/src/lib.rs --impl Core --json > /tmp/core.json
```

235 methods, 317 KB. Write `scripts/core-split.map.json` mapping each method name to a
module stem. Suggested modules, from the method names themselves:

```
definitions  building_definition item_definition recipe item_name stack_size is_fluid …
player       player_snapshot carry_stacks player_can_carry grant discard set_carry_slots …
placement    footprint_for envelope_for clearance_for oriented_cells entity_* …
range        within_world_range* within_hex_range_of_entity within_build_range_of_target …
world        generate_chunk terrain_at field_at ensure_tile ensure_neighborhood survey_* …
deposits     resource_at_world deposit_* extractor_* extract_cycle regrow_flora …
graph        compile_graph compile_links trace_* rebuild_runtime_index occupancy_maps …
power        compile_power distribute_power boiler_* generator_* power_* entity_powered …
tick         tick_many advance_ticks advance_machines advance_player_steps collect_* …
transport    advance_belt_lanes transfer_cargo hand_over is_merger accepts_item …
research     technology apply_research_effects earned_* progress_total …
save         save_string from_save checksum_for_world …
```

Leave `new` and `initialize` unmapped — they stay with the struct in `core/mod.rs`.

**Done when** the map covers every method you intend to move and
`rust-split.mjs apply … --dry-run` reports no unknown names.

---

## T10 — Execute the split, one module at a time

**Do not** move all twelve at once. Per module:

```bash
node scripts/rust-split.mjs apply factory-wasm/src/lib.rs --impl Core \
  --map scripts/core-split.map.json --out-dir factory-wasm/src/core --dry-run
# then, with a map containing ONE module's methods:
node scripts/rust-split.mjs apply factory-wasm/src/lib.rs --impl Core \
  --map /tmp/one-module.json --out-dir factory-wasm/src/core
cargo check --manifest-path factory-wasm/Cargo.toml 2>&1 | head -40
```

Add the `mod` line the script prints, then fix imports **only** from what the compiler
names. Always pipe `cargo check` through `head` — an unfiltered error dump costs more
than the edit.

Commit after each module. `cargo test` after every third.

**Done when** `core/` holds twelve modules, none over 50 KB, and the test suite is green.

---

## T11 — `impl Factory` → `api.rs`

The `wasm_bindgen` boundary: ~30 methods, 26 KB, starting line 10,064.
Same tool, `--impl Factory`, single target module `factory-wasm/src/api.rs`.

```bash
npm run build:wasm
```

**Done when** the wasm artifact builds and the game loads.

---

## T12 — Residual types out of `lib.rs`

After T10/T11, `lib.rs` still holds ~76 structs/enums and ~108 free functions.
Group them beside the `impl` module that uses them; keep genuinely shared types in
`core/types.rs`. This one needs judgement, so bounded reads only — use the T09 inventory
JSON and `.agent/native.md` to locate declarations rather than scrolling.

**Done when** `lib.rs` is a re-export shim under 15 KB and its waiver is deleted.

---

## T13 — Comment → ADR pass

33% of the Rust crate is prose; `src/core/types.ts` is 61%. Rationale belongs in
`docs/decisions/NNN-*.md`, invariants stay in the source.

**Run this opportunistically, never as its own sweep.** When a task already has a file
open, convert that file's chronicle comments and move on. Start with `struct Core`'s
field docs and the save-version narrative.

**Done when** — it never is. That is fine.

---

## T14 — `index.html` id manifest

1,954 lines, 251 `id=` attributes, and every UI task opens it to resolve a selector.
Generate `src/dom/ids.ts` from the template at build time (a small script in `scripts/`),
exporting a typed const map. Agents then read a 6 KB manifest instead of a 77 KB template.

Optionally follow with a partials split, but the manifest alone captures most of the win.

**Done when** `main.ts` resolves ids through the manifest and the waiver is deleted.

---

## T15 — Split `styles.css`

86 KB, one file. Split by panel to mirror the eventual `src/panels/` layout; import from
a small `styles.css` index.

---

## T16 — `AppContext`

The reason `main.ts` has never split: ~282 top-level declarations sharing one module
closure over mutable session state and cached DOM refs. Nothing can move until that
closure is named.

Create `src/app/context.ts` defining an `AppContext` interface covering every
module-level `let`/`const` currently captured. Get the list cheaply:

```bash
grep -nE "^(const|let) " src/main.ts
```

**Done when** the type exists and compiles. No behaviour change yet.

---

## T17 — Convert renderers to take `ctx`, in place

The long one. For each renderer — `renderInventory`, `renderHotbar`, `renderBuildPanel`,
`renderInspector*`, `renderRecipePanel`, `renderTechnologies`, `renderContract`,
`renderRequests`, `renderProjectCatalogue` … — change the signature to
`(ctx: AppContext, …)` and thread it through. **Files do not move in this task.**

`npm run test:run` between each. Commit per renderer.

Doing this before T18 is what keeps the whole thing from becoming a rewrite.

---

## T18 — Move renderer clusters to `src/panels/`

Only once a cluster takes no free variables. Pure file moves at this point.

---

## T19 — Input and lifecycle

`src/input/{pointer,keyboard,constructionInput}.ts`, then
`src/app/{createApp,frameLoop,lifecycle}.ts`. `main.ts` ends as:

```ts
import { createApp } from "./app/createApp";
const app = await createApp();
app.start();
```

**Done when** `main.ts` is under 100 lines and its waiver is deleted.

---

## T20 — Repo hygiene

`world-scale-basin.png` (1.1 MB), `world-scale-opening.png` (811 KB) and
`shallow-water-ford.png` (780 KB) sit in the repo root and appear in every directory
listing and glob result. Move to `docs/media/`. No token cost directly; it stops polluting
every `ls` an agent runs.

---

## Handoff line

At the end of a session, append one line to `docs/plan/AGENT-CONTEXT-REFACTOR.md`:

```
T05 done 2026-09-xx — lib.rs 1153 -> 715 KB, 412 tests green, waiver kept (T06 next)
```

Nothing else. The next agent reads this file and the board, not your transcript.

---

## Handoff log

```
T00 done 2026-09-02 — tooling landed, budget report 15 failures / 69 KB always-paid, inventory 235 methods (T01 next)
T01 done 2026-09-02 — .agent-budget.json waives all 15 failures, context:budget + context:check added, context:check wired into quality after audit, fail-on-delete verified (T05 next)
T05 done 2026-09-02 — lib.rs 1153 -> 687 KB, tests/mod.rs 426 KB + tests/petroleum.rs 16 KB, 97 tests green (was 97), include_str paths shifted one level, new waiver tests/mod.rs -> T06 (T06 next)
T06 done 2026-09-02 — tests/mod.rs 426 -> 23 KB across 16 domain files, largest earthworks.rs 44 KB, 97 tests green, tests/mod.rs waiver deleted; domains wire/ground/boundaries/capacity renamed wire_format/earthworks/walls/throughput to clear crate module names (T07 next)
```
