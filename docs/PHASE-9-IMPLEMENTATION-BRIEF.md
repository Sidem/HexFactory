# TEMPORARY — Phase 9 Living Lattice implementation brief

> **Lifecycle:** active only while Phase 9 is being implemented. Update the status and handoff notes after
> each accepted slice. Delete this file in the same change that completes the final Phase 9 gate. It is not
> part of HexFactory's permanent document set.

## Mission

Turn ecology into infrastructure: a renewable production network with spatial memory that the player can
read, automate, damage, repair, and improve. Phase 9 must give an established factory a durable reason to
expand and optimize without adding survival pressure, repetitive maintenance, or a detached wildlife
simulation.

The desired player loop is:

> Observe the landscape → predict an outcome → build a living supply chain → see the world respond →
> understand the bottleneck → make one more improvement.

The lasting fantasy is: **I built a factory that works with a living landscape, and both visibly became
more productive because I understood the system.**

## Current status

- [x] Slice 1 — measured fertile-riverbank habitat truth and player-visible habitat
- [ ] Slice 2 — one harvestable population and a safe Field Station vertical slice
- [ ] Slice 3 — sparse recovery, migration, local collapse, and restoration
- [ ] Slice 4 — Digester joint-output loop and Habitat Tender automation
- [ ] Slice 5 — finite Riverbank Renewal hub programme and derived guidance
- [ ] Slice 6 — tuning, art/audio feedback, accessibility, saves, balance, and performance gate
- [ ] Final Phase 9 acceptance gate passed
- [ ] This temporary file deleted

Do not mark a slice complete because its data structures exist. Mark it complete only when its player-facing
behavior, closest tests, and stated gate are complete.

## Product principles

1. **Every ecology rule creates a decision.** Movement, feeding, recovery, and migration exist only when
   they change placement, throughput, routing, buffering, corridors, or recovery choices.
2. **The best long-term strategy is enjoyable.** A regenerative line must outperform repeated depletion
   over the measured sustained-production window. Intensive harvest may buy a useful short burst, but must
   not dominate permanent play.
3. **Forecast before consequence.** Native preview and the world view show expected output, reserve, health
   trend, and blocked migration before a player commits a harmful configuration.
4. **Failure is local, early, and recoverable.** No global ecological fail state, permanent extinction, or
   delayed surprise. Damage is bounded to named habitat and has an executable recovery route.
5. **Learning ends in automation.** Manual actions may teach the loop once. A correct ecological factory
   then runs unattended; recovery cannot require waiting or recurring clicks.
6. **One deep loop beats a content catalogue.** Ship one habitat family, one population archetype, one
   primary harvest, one joint-output process, and no more than two genuinely new ecological machine
   behaviors.
7. **State reads in the world.** Density, stress, collapse, migration, and recovery are legible at normal
   zoom without relying on color or opening a dashboard. The inspector supplies exact explanation second.
8. **Stopping play is safe.** No real-time decay, offline change, daily demand, random disaster, or FOMO.
9. **Ecology strengthens the factory game.** It must exercise existing belts, pipes, storage, power,
   backpressure, joint outputs, terrain, walls, gates, and guidance rather than bypassing them.

## Target content model

Names are provisional until they are tested in the UI. Keep roles stable even if names change.

### Habitat

- Derive a scarce `fertile riverbank` habitat capacity from native drainage, elevation, ground, and water
  state. It is world truth, not a hand-authored resource patch.
- Expose four presentation bands backed by exact integer state: **Absent**, **Stressed**, **Stable**, and
  **Thriving**.
- Buildings and paving remove capacity only from the cells they visibly occupy. Walls block migration on
  their exact graph edges; an open gate restores passage. Do not add an invisible pollution or noise radius
  in this phase.
- Every world preset remains completable. Distribution and distance claims require a committed survey
  measurement before tuning is accepted.

### Population

- Use one archetype, provisionally `river grazers`.
- Simulation truth is integer abundance in native state. Presentation may instance several animals to
  communicate a band; it never owns an individual animal.
- Stable habitat sleeps. Harvest, habitat edits, boundary edits, restoration, and due recovery wake only a
  bounded local frontier.
- Local collapse reduces or stops harvest. Recovery comes from connected migration plus restored habitat;
  it never requires loading an earlier save.

### Factory loop

```text
fertile habitat → population surplus → Field Station → raw biomatter
                                                    ↓ + water
                                                 Digester
                                                ↙        ↘
                                useful concentrate      nutrient residue
                                         ↓                    ↓
                               current industry /       Habitat Tender
                                future Phase 10               ↓
                                                   recovery and future yield
```

- **Field Station:** draws only surplus inside a visible range. Its safe default stops at a published
  reserve and reports why. If policies survive playtesting, use Conservative, Sustained, and Intensive;
  show the projected health/output effect before selection.
- **Digester:** consumes biomatter plus water and emits one useful concentrate plus nutrient residue as
  mandatory joint outputs. Reuse the existing routing and backpressure contract.
- **Habitat Tender:** consumes residue and water to accelerate recovery or improve a damaged habitat. It
  idles when no legal work remains and must not require manual feeding.
- Give concentrate immediate Phase 9 utility, such as energy or industrial feedstock, and preserve a clear
  role in Phase 10's food chain. The line must remain worth running after its programme completes.
- Prefer an alternative residue use in an existing machine over adding another ecology-only building. It
  may trade future yield for immediate value, but may not create a dominant waste-disposal chore.

### Viable strategies

Balance at least two, and aim for all three:

1. **Distributed stewardship:** several low-rate stations, longer logistics, little restoration.
2. **Regenerative cell:** concentrated Station–Digester–Tender loop; higher infrastructure cost and best
   sustained output per habitat area.
3. **Frontier pulse:** intensive temporary extraction followed by relocation or restoration; useful for an
   urgent batch but expensive as a permanent pattern.

Geography should alter which is attractive. Avoid one universal blueprint.

## Riverbank Renewal programme

Add a post-founding, finite programme rather than another large commodity bill or repeatable request. It is
the first implementation of a programme model that later phases may extend; do not generalize beyond what
this one programme proves.

1. **Read the living edge:** discover the habitat and population, then produce a first Field Station batch.
   Reveal ecology only after the player encounters it. Reward the habitat overlay and Digester capability.
2. **Prove a stable yield:** deliver useful concentrate and complete a small number of clearly displayed
   harvest cycles without dropping below Stable. Reward the Habitat Tender and visible hub growth.
3. **Close the loop:** route residue into restoration, return a stressed habitat to Stable, and sustain the
   line for a bounded interval without manual ecological input. Reward Phase 10's entry capability and
   permanent hub growth.

Use capability grants, visible construction, and at most a justified skill milestone as rewards. Do not add
another large insight payout; the current finite project catalogue already funds the research tree with a
large surplus. Guidance must derive the next executable action from actual habitat, recipes, inventory,
power, and programme state.

## Player feedback contract

At normal zoom, the player must be able to distinguish healthy, stressed, and collapsed habitat through
shape/density/motion, not color alone. Add only the generated visual vocabulary needed to show:

- population density and habitat cover;
- directional migration when it is actionable;
- thinning and quieting under stress;
- visible return during recovery;
- Field Station state: harvesting surplus, reserve reached, recovering, or migration blocked;
- hub construction after each programme stage.

The habitat overlay and inspector should name current band, capacity, population/reserve, trend, limiting
cause, and a useful next action. Placement/policy preview must show projected sustainable output and the
first predicted health-band change. Reduced motion preserves the final information. Prefer spatial feedback
and existing item chips/status marks over a new global dashboard.

## Architecture and ownership

Repository invariants in `AGENTS.md` and `docs/ARCHITECTURE.md` remain authoritative. In particular:

- Rust/Wasm owns habitat capacity, population quantity, recovery/migration, harvesting, programme state,
  legality, time, saves, checksums, and snapshot dirty state. TypeScript sends bounded intent and renders
  snapshots.
- Add an ecology domain module rather than growing `lib.rs`, the core tick, or another coordinator. The
  exact split should follow nearby ownership patterns. If one ecology file approaches a context ratchet,
  split model/advance/preview responsibilities before adding more behavior.
- The core tick only invokes a bounded ecology operation; it does not contain ecology rules. Construction,
  ground, and boundary mutations notify ecology through a small domain boundary rather than reaching into
  its collections.
- Save/hash only minimal causal truth: population departures from generated equilibrium, habitat amendments,
  and any timing value that changes future results. Rebuild queues, spatial indexes, connected components,
  and other caches; test them against an uncached/full oracle.
- Use a sparse due schedule or active frontier. Never scan surveyed chunks, all habitat, or every population
  each tick. Stable equilibrium has no permanent cost.
- Deterministic arbitration uses stable native IDs/cell order. A blocked harvest or transfer leaves its
  source unchanged.
- Extend definitions with the smallest data category that expresses the behavior. Do not add a new native
  kind merely to give a definition a new name or appearance.
- Keep habitat, population, building identity, cargo, recipe, progress, and presentation bands separate.
  Do not flatten them into a combined status type.
- Snapshot additions use native dirty state and exact integers. Prefer a separate bounded ecology wire group
  and decoder/renderer ownership over enlarging unrelated terrain or entity logic without cause.
- Rendering uses sparse instancing and generated forms. Visual animal instances are presentation derived
  from exact published state and world position; they do not enter saves or checksums.
- Do not add Phase 9 application behavior to `src/main.ts`. Keep UI/controller changes in the smallest
  existing owner or a new focused module, with keyed DOM controls patched in place.
- Split another large existing module only when the active slice must change it, and move its nearest tests
  with the behavior. No broad preparatory rewrite.

## Implementation slices

### Slice 1 — habitat truth and legibility

Derive and measure fertile-riverbank habitat, publish it through the narrowest snapshot/wire representation,
and make it recognizable in the world and inspector. Do not add animals, harvesting, recipes, or programme
state yet.

Gate:

- deterministic across generation order, chunk seams, saves, and exact seeds;
- derived from existing native physical fields, not a second host approximation;
- scarce but present at useful distances in every preset, backed by a committed survey measurement;
- visually distinct at normal zoom and without color, while remaining separate from resources;
- narrow native, cross-language wire, renderer, and inspector tests pass;
- `npm run context:check` and `npm run agent:map` remain green.

### Slice 2 — first harvestable population

Add generated equilibrium abundance, sparse departure state, one visible population archetype, a minimal
Field Station, safe-reserve harvesting, native preview, one raw biomatter item, and exact stop reasons.

Gate: a player can find a habitat, predict a safe station, see output reach existing transport/storage, and
understand reserve stoppage without documentation. Save/resume and uninterrupted play converge.

### Slice 3 — living response

Add bounded feeding/recovery, migration through native edges, wall/gate effects, local collapse, and a
recoverable route. Implement full/uncached or small-world oracles before optimizing caches.

Gate: healthy, stressed, isolated, recovering, and collapsed cases are visually and mechanically distinct;
all reproduce across saves/checksums; stable habitat performs no recurring scan.

### Slice 4 — regenerative factory loop

Add the Digester, concentrate and residue, joint-output routing, Habitat Tender, alternative residue use if
it survives playtesting, and the smallest necessary technology/definition changes.

Gate: at least two layouts are viable; the regenerative layout wins the sustained-production measurement;
the completed line runs ten simulated/player-observed minutes without manual ecological maintenance.

### Slice 5 — programme and guidance

Add Riverbank Renewal as one finite post-founding programme with three stages, capability rewards, derived
guidance, and hub visual growth. Do not implement a speculative general programme framework beyond what the
single shipped programme needs.

Gate: the guidance test can execute a legal route through every stage; the player is never asked for an
unreachable item or hidden ecological condition; the completed line remains useful.

### Slice 6 — acceptance and tuning

Tune thresholds and costs from measurements and playtests. Finish audio/visual feedback, reduced-motion and
color-independent reads, migrations, fixtures, and the ecology performance tier.

Gate:

- players can identify habitat bands and predict station impact before harm;
- local collapse is recoverable without reload, abandonment, or idle waiting;
- at least two materially different layouts are viable across representative geography;
- sustainable production beats depletion over the committed comparison window;
- a correct line needs no repeated manual ecological input for ten minutes;
- all definitions enter `fixtures/balance.json` and every distribution/performance claim is committed;
- recovery, migration, harvest, programme progress, save/restore, wire deltas, and checksums are exact;
- `npm run quality` passes and the production startup/capacity budgets still hold.

## Explicitly out of scope

- multiple species, predators, disease, genetics, seasons, weather, or a day/night dependency;
- individually simulated animal AI or a permanent whole-world ecology tick;
- random population disasters or permanent extinction;
- global pollution, ecological morality, or approval meters;
- player hunger, survival decay, or other Phase 10 needs;
- repeatable ecology contracts, daily tasks, offline change, or FOMO;
- manual feeding, replanting, waste clearing, or waiting as the recovery strategy;
- bespoke models/draw calls per definition or decorative props that become simulation truth;
- speculative frameworks for later regional, vertical, or power phases.

## Session workflow and token discipline

Every implementation session must:

1. Read `AGENTS.md`, choose the current task route in `docs/AGENT-MAP.md`, then open only its small `.agent/`
   index. Locate named declarations with `rg -n` and read bounded ranges plus the nearest test. Do not load
   oversized modules end to end.
2. Read only this brief's current slice, the matching portion of `docs/HEXFACTORY-PLAN.md`, and the relevant
   rule sections of `ARCHITECTURE.md`, `ART.md`, or `BENCHMARKS.md`.
3. Inspect `git status` and preserve unrelated changes. Do not commit generated dependencies, Rust targets,
   wasm-pack output, or `dist`.
4. Implement the smallest end-to-end player-visible increment. Avoid preparatory abstractions whose first
   consumer is in a later slice.
5. Run the narrowest relevant test after the first patch. Expand only when a dependency or failure names the
   next check. Run `npm run agent:map` after declarations move or are added.
6. Keep public boundaries small, names domain-specific, state separated, and coordinators thin. When a file
   would breach a context ratchet, create a focused owner instead of raising the ratchet.
7. Update the checklist and handoff below with only decisions that remain relevant to the next session.
   Settled implementation detail belongs in source, tests, fixtures, and git history—not in growing prose.

## Handoff log

Keep this short. Replace stale entries rather than accumulating a diary.

- **Current slice:** Slice 1 complete; Slice 2 is next.
- **Next executable step:** add generated equilibrium abundance over exact habitat, sparse departure state,
  one visible population archetype, and the minimal native Field Station safe-reserve harvest path.
- **Decisions carried forward:** fertile riverbank is an intact, dry, untreated, unoccupied generated river
  bench with capacity `drainage class × 25`; wire v24 publishes sparse exact cells with zero tombstones.
- **Measurement:** the committed radius-96 fixed-seed survey finds 589 cells (28 per mille of land), capacity
  73,625, and a nearest habitat 13 hexes away in every preset.
- **Open for Slice 2:** measure equilibrium abundance, safe reserve, and Station rates before fixing them.
