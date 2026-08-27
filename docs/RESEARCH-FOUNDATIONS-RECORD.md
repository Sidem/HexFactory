# Research foundations — v0.29.0

Phase 1 delivery, 2026-08-27. This is the registry and shared-availability portion of the
[progression plan](PROGRESSION-PLAN.md), not its research map, commission system or finite economy.

## What ships

Technology catalog 9 gives each of the existing 19 research nodes a primary branch and stage.
Each registry entry has a stable key, name, description and display order. Eight populated branches
and three stages are authored; adding another is data work. Stages impose no global gameplay gate.
The research list sorts by stage, then branch, then stable technology ID, retaining its keyed
controls and near/full-tree toggle. Each card exposes its classification in visible and accessible
text. Disabled cards keep their explanations readable. Corner Transport now lists the heading
capabilities it provides even though its building-unlocks array is empty.

The current broad bundles are deliberately retained: Material Processing is under Metallurgy but
still unlocks both smelter and kiln; Mechanical Shaping is under Woodwork but still includes the
crusher; Machine Tiers still includes storage. Field capabilities is an explicitly temporary home
for the existing insight-priced pack and reach purchases. It is not a separate skill tree or budget.
Splitting those bundles and migrating their effects remains future work.

The native `research_availability` function is used by both atomic purchases and snapshots. Each
row carries the stable technology ID, completion flag, missing prerequisite IDs and insight
shortfall. Research cards and guidance consume these answers instead of reimplementing purchase
eligibility in TypeScript. An absent row disables the purchase rather than guessing. The snapshot
baseline rebuilds this derived group only when insight or researched IDs change, and sends it only
when the answer differs. No entity scan or per-item history is added.

The studio can assign branch and stage when creating or editing a technology. Rust and TypeScript
validate nonempty registries, unique stable keys, display-order bounds, references, duplicate
prerequisites/unlocks and the existing acyclic prerequisite graph. Registries are bounded at 64
entries each and the technology catalog at 1,024 entries. This is not yet a typed AND/OR/evidence
or project-reward schema; flat prerequisites retain their existing AND semantics.

## Economy and compatibility

All 19 IDs, costs, prerequisites, unlocks and player bonuses are unchanged: 153 insight buys the
same research. No recipes, construction bills, request quantities or payouts changed. Regenerating
`fixtures/balance.json` changes only `reference.technology_version` from 8 to 9; its economy values
remain identical. This does not validate the existing repeat-income economy or new-player pacing.

Save 21 / definitions 18 / technologies 9 / scenarios 5 / world 8 / wire 13.
The adjacent save migration advances technology 8 to 9 without touching state, rewards, inventory,
jobs, research or checksum. Availability and registry metadata are never saved or hashed. Wire 13
appends a bounded availability group; the shared JSON/binary fixture pins its exact representation.

The browser save picker previously recognized migrations only through save 18, despite native
support for later releases. It now mirrors the exact released save/definition/technology tuples
from save 14 through 21. Unknown tuples, newer files, and mismatched world/scenario versions remain
incompatible; native validation is still authoritative.

## Verification and limits

Native tests compare published availability against successful and refused purchases across the
catalog, assert no mutation on rejection and no second charge, and cover income/purchase/creative
deltas with quiet-frame omission. A technology-8 factory with a partially worked manual job loads
at the same checksum and converges after equal commands and ticks. Reordering and relabelling
presentation does not alter purchases. Existing capability, replay, wire and full-diff tests remain.

TypeScript checks registry rejection, deterministic presentation ordering, native-only availability,
guidance, wire decoding and the released save-picker migration chain. The full local quality gate
passes 242 TypeScript tests and 181 Rust tests, formatting, lint, type checking, map freshness,
dependency audit and the production build. The existing Vite bundle-size advisory remains.

Browser checks cover normal-run blockers, all 19 cards, classification labels and heading benefits.
The existing named `Workshop validation 2026-08-27` save (save 18 / definitions 16 / technology 8)
loads successfully through the full migration chain without replacing that named save.

Still required in Phase 1: direct foundation commissions, typed progression requirements/effects,
remaining industrial bills and timed human opening validation. Finite insight income, separate
skills, a spatial research map, icon families and physical laptop evidence remain unshipped.
No opening-time, player-study or performance improvement is claimed here.
