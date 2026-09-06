# TEMPORARY — Phase 9 adjustments: animals as entities

> **Lifecycle:** active only while the Phase 9 animal rework is being designed and implemented. This
> file supersedes the population sections of [`PHASE-9-IMPLEMENTATION-BRIEF.md`](PHASE-9-IMPLEMENTATION-BRIEF.md);
> the habitat, digester, guidance and programme sections of that brief still stand. Fold the accepted
> rules into [`ARCHITECTURE.md`](ARCHITECTURE.md) and [`ART.md`](ART.md) and delete both temporary files
> in the change that completes the final Phase 9 gate. Not part of the permanent document set.

**Status:** slices A1–A5 implemented. Population-as-resource-field is gone: herds are
free-moving agents, grass is a sparse stock, and `population_capacity` no longer exists. Wire version
26, save version 47, definitions version 32. Wire 26 carries the riverbank, split out of the shore band
so the player can see the fresh water the animals steer to. Save 47 is the pasture-station catalogue
step: no saved quantity moves.

Two rulings were settled against this document while implementing it, both recorded in code comments
at the point they bind:

- **Wildlife never vetoes construction.** An earlier draft refused a footprint or a fence line that
  crossed a herd. That refusal broke the founding contract's own site when a herd wandered onto it,
  and it put a fact in the checksum that undo could not put back. Animals yield instead: a herd
  standing where a machine lands walks off on its next decision, ahead of every drive including
  `Rest`, and only legs _already in flight_ are re-checked when a wall closes across them.
- **Habitat rows stay sparse.** A row travels only where the ground departs from pristine pasture —
  riverbank capacity, grass eaten below the limit, or standing waste — and a hex returning to
  pristine leaves through the same zero tombstone a depleted riverbank uses. Sending a row per grassy
  hex would put most of the world on the wire the first time anything grazed. The dirty-tracked delta
  and the full-snapshot oracle share one predicate so the two paths cannot disagree.

## Why the current animals are being replaced

The uncommitted implementation models a population as a resource field: a quantity attached to a hex,
stored in the same `tiles` overlay as trees and ore, reachable through `buried_field_at`, harvested by
decrementing `deposit_quantity`. Every property that reads wrong follows from that one decision.

- A population is addressed by `(q, r)`, so movement can only be a teleport between overlay entries.
  `write_population` therefore sets `dirty.resources_replace` and clears `deposit_links` on every herd
  step, which resends the whole resource table and invalidates every extractor binding in the world,
  once per epoch, forever.
- Harvest is `deposit_quantity - 1`, so an animal is a unit mined off a living stack. There is no death,
  no carcass, no flight.
- Populations are visible to `deposit_candidates`, so an extractor cannot tell a grazer from a rock, and
  the player's nearest-gather target can be hijacked by a herd walking past.
- A field cannot be between hexes, so animals cannot be chased, herded, penned, driven or led.
- `population_capacity` is a formula re-evaluated per query over a ring of ground items and water rather
  than a stock. **Grazing consumes nothing.** Capacity is computed, compared, and enforced by fiat.

The last point is the deepest one: the current design programs the outcome (herds move when they exceed
a computed capacity) instead of the mechanism (herds move because the grass ran out).

This codebase is unusually well equipped for free-moving agents — continuous integer player movement,
segment-tested boundaries, a saved sparse mobile-object list, and a proven tick-to-frame interpolation
idiom — and the population implementation uses none of it.

## The four decisions

### D1 — What is the simulated unit?

| Option                                         | Cost     | Buys                                                                                           | Forecloses                                         |
| ---------------------------------------------- | -------- | ---------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| Population field (current)                     | none     | nothing new                                                                                    | everything below                                   |
| Hex-hopping token, on the `ground_items` shape | very low | id stability, decoupling from deposits                                                         | still teleports; still cannot be between hexes     |
| **Herd agent plus drawn bodies**               | low      | one moving point per herd, `count` bodies rendered around it; flight, penning, luring, driving | per-animal identity                                |
| Per-animal agent                               | medium   | individual life, real predation, breeding trees                                                | needs a spatial index and a hard population budget |

**Decision: herd agent, with the per-animal upgrade left open.** The simulated object is a herd with a
continuous position, a headcount and drives. The renderer scatters `count` bodies around that point.
Every behaviour Phase 9 and Phase 10 want — fleeing, drinking, penning, luring, overgrazing, husbandry —
is a herd-level behaviour. If individual animals are ever needed, the `herd: u32` field on a creature row
is the seam to split along, and no other decision here changes.

### D2 — Position and clock

`player.x` / `player.y` are integer world units in a continuous frame (hex spacing 1774 / 1536, hex radius
1024), stepped by `player_step` with circle collision against `PLAYER_RADIUS` + `BUILDING_RADIUS` and
segment tests against boundaries. That is the free-movement idiom, and it is already deterministic and
integer-only. Animals adopt it.

The 10 TPS stepping problem is already solved for belt cargo. Native publishes the tick an item stepped on
and [`beltLaneTravel`](../src/rendering/buildingLook.ts) adds the visual remainder of the current tick, so
items creep between snapshots instead of jumping.

**Decision: publish a leg, not a position.** A herd stores `from`, `to`, `left_tick`, `arrive_tick`.
Position is derived — natively by integer interpolation when something needs it, on the host by the
belt-cargo trick at frame rate. Consequences:

- Native writes a herd row only when it picks a new leg or changes drive: a few times a minute, not every
  tick. Unchanged legs need no repeated position rows; needs and inspector changes may still require deltas.
- Per-tick cost becomes a priority queue keyed on `arrive_tick`. Each tick touches only the herds whose leg
  just ended. 500 herds on 30-tick legs average about 17 arrivals per tick; synchronized arrivals can
  still spike. This is scheduling arithmetic, not a measured total-work or performance claim.
- Animals stay on the factory tick, so pausing pauses the world and game speed keeps meaning something. The
  Phase 10 note about a separate player clock is about player needs, not wildlife.

### D3 — What makes them move?

The current schedule (move every three epochs, score neighbours) reads as a lattice random walk because it
is one. A utility system or behaviour tree is overkill and hard to debug against a checksum.

**Decision: a small explicit state machine over a priority ladder**, each state resolving to a target point
and a speed:

| Drive    | Condition        | Target                                         |
| -------- | ---------------- | ---------------------------------------------- |
| `Flee`   | `alarm > 0`      | away from the disturbance, fast                |
| `Thirst` | `thirst > limit` | reachable drinking bank within smell range     |
| `Graze`  | `hunger > 0`     | reachable grass worth the travel cost          |
| `Rest`   | otherwise        | linger, stand or lie down; occasional slow leg |

Four named states, readable straight off an inspector line ("Thirsty — heading for the river"), which is
worth as much for legibility as for behaviour.

**And grazing must consume something.** Add a sparse `grazed: BTreeMap<(i32, i32), u16>` — grass eaten off
a hex, regrowing on a cadence, on the existing `flora_regrowth` idiom (sparse, cheap on untouched ground).
Then:

- Overgrazing is emergent. A herd parks, strips the hex, `Graze` finds better grass one hex over, they move.
  Nobody writes "move when over capacity".
- Carrying capacity stops being a per-query ring scan; `population_capacity` disappears entirely.
- Waste damage remains distinguishable from eaten grass, with its own recovery cause.
- Phase 10 pasture, hay and feed all have somewhere to land.

### D4 — How does the player get value from them?

Three verbs, in unlock order.

1. **Hunt.** Aim, a windup, the herd's `alarm` spikes and they run. A kill drops a **carcass ground item** —
   `add_ground_item` already exists, already despawns, already survives saves. Butchering is a recipe, so the
   yield is meat / hide / bone rather than the current "grazer biomatter" ore. A thing that runs away is not a
   rock; this verb alone justifies free movement.
2. **Pen.** Fences and gates already block the player and transport across a hex edge and are already
   segment-tested, which is exactly what a herd's leg needs to consult. Make boundaries block creatures and a
   pen requires formation containment and active-leg invalidation: enclosed herds strip the grass, so feed must
   be brought to them. That is a factory demand created by an animal, which is the whole point of Phase 9
   feeding Phase 10.
3. **Lure and drive.** A feed item on the ground is a strong `Graze` attractor; the player's proximity is a
   `Flee` repulsor. Together they are a herding mechanic with explicit target commitment and fear thresholds — push from behind, bait
   from in front. Phase 10 domestication is then "a penned, fed herd breeds and tolerates you", not a new
   subsystem.

## Packages considered

- **A — Fauna-lite.** Hex-hopping tokens in their own saved list, still gathered by hand. Fixes the extractor
  hijack and the snapshot storm; does not fix the feel. Rejected: spends the disruption budget without buying
  the thing that is wrong.
- **B — Herd agents.** D1 herd agent, D2 legs on the tick, D3 drives with real grass, D4 hunt first, pens and
  luring behind a technology. **Chosen.**
- **C — Individual beasts.** As B plus per-animal identity, age, sex, health and predators. Held until a
  playtest says herds are not enough. It would require revisiting formation, targeting, scheduling and population budgets; a future herd reference is only an ownership seam.

## Chosen design

### Module ownership

New `factory-wasm/src/fauna.rs`, with no dependency on `deposits.rs` at all. Creatures are never fields,
never overlay rows, and never visible to `field_at`, `deposit_candidates` or `extractable_deposit`. That
decoupling is the point; do not reintroduce a lookup bridge for convenience.

Do **not** reuse the building `Entity` list. Footprint, recipe, power graph and construction-state coupling
all apply to a stationary thing on a hex, and every one of them would need a null branch for a creature.

### Data model

```rust
struct Herd {
    id: u32,
    species: SpeciesId,   // a real definitions table, not `population: bool` on an item
    from: (i32, i32),     // world units, the player's coordinate frame
    to: (i32, i32),
    left_tick: u64,
    arrive_tick: u64,
    count: u16,
    drive: Drive,         // Flee | Thirst | Graze | Rest
    hunger: u16,
    thirst: u16,
    alarm: u16,
}
```

- Saved and checksummed in id order, beside `ground_items`, with `next_herd_id` alongside
  `next_ground_item_id`.
- Position is derived from the leg and never stored twice, in line with the existing rule that derived state
  is not saved, hashed or checksummed.
- `species` comes from a definitions table carrying body size, speed, herd size, diet, flight distance and
  yields. Building the table now costs almost nothing and makes river fowl or a predator additive rather than
  structural.

### Scheduling

`BTreeMap<u64, Vec<u32>>` from `arrive_tick` to herd ids. Each tick pops the arrivals, runs one decision
each, and pushes the next leg. Plus a local `alarm` wake when the player moves inside a
radius — the same wake-set idiom as `wake_population`, but bounded by proximity to the player rather than by
explored area, so untouched empty area schedules no work. Moving herds, needs, regrowth and invalidation still require measurement.

### Movement

`walkable_hex` for terrain, `boundary_blocks_segment` for fences and walls, `circles_overlap` against
building footprints. Soft collision with the player: animals yield. Blocking bodies plus a fence is how a
player gets wedged in a corner by a cow.

### Wire and host

A new creatures group with `changed` plus `removed`, on the buildings-group shape. **Never** the resources
group's replace-only patch — `applyResourcesPatch` can only update rows in place, which is the direct cause
of the current per-epoch whole-world resend. Herd rows cross the wire only when a leg or a drive changes.

### Rendering

Keep the grazer body from `src/rendering/three/populationMeshes.ts`; it is good work and survives largely
as-is. Draw `count` instances scattered around the interpolated herd point, each with a stable per-index
offset and a gait bob keyed on the tick. It will read better than the current version because the whole
cluster translates instead of popping between hexes.

### Inspector

Click a herd, not a hex: "Herd of 4 · thirsty · heading for the river · grazing here for 40 s." That meets
the Phase 9 gate on showing population health early enough to react by construction, rather than with a
paragraph on a riverbank panel the animals are usually not standing on.

### Determinism

Integer throughout. Per-decision randomness from a coordinate hash of `(salt ^ tick, id, count)`. The flee
input is the player position, which is already checksummed. The creature list is saved and checksummed in id
order.

## Revised player-experience contract

These rules refine the outline above and take precedence over claims that penning, automation or
individual animals come for free. They preserve the phase order. The riverbank and grazing changes below
are planned work, not claims about the current renderer or native simulation.

### Living terrain, not uniformly sandy riverbanks

A riverbank describes a relationship to fresh water, not one sand tile type. Support grassy alluvial
banks, exposed mud/silt, gravel or rocky edges, and occasional sandy bars. Beaches may remain sandy, but
freshwater banks must not all read as beaches. Choose surface character from authoritative native ground,
water and substrate information; do not infer fertility from a decorative colour or random visual hash.
Only introduce additional native fields where existing snapshots cannot explain the distinction.

Keep substrate, moisture/access and vegetation condition separate. A pristine lowland has dense cover;
a grazed lowland has short, broken cover and exposed soil; a recovering one visibly fills back in. It
remains lowland: cosmetic stages do not silently change movement or terrain legality. Native grass stock
and distinct fouling state drive those stages. Connect changes to local dirty patches, including water,
paving, grazing and restoration changes; no whole-world mesh rebuild for a bite of grass.

Blend edges with neighbours and use cover density, silhouette and surface texture as well as colour.
At normal zoom the player must distinguish a sandy beach, a healthy river meadow, eaten pasture and
fouled ground. Regrowth must be visible without opening an inspector. Do not fake grazing depletion in
TypeScript while native grazing consumes nothing.

### Sustainable production must become unattended

Manual hunting teaches the resource and supplies an early batch; it must not remain the best recurring
way to fuel an established factory. Before the economy slice is accepted, specify the Field Station's
replacement interaction with mobile herds: a visible managed area, a published reserve, a physically
legible collection/harvest interaction and a safe default that stops at the reserve. No invisible remote
subtraction of animals and no return to deposit extraction. The exact harvesting interaction remains a
design decision to resolve before implementation.

Complete the animal-to-processing-to-useful-output loop in Phase 9, including automated recurring inputs
and residue handling. Keep full domestication and player food needs in Phase 10. Meat, hide and bone are
candidate outputs, not a requirement to introduce three inventory streams without current uses. Each
mandatory output needs an immediate useful route; ecology remains worth operating after its programme.

Support at least two useful layouts: spacious pasture with low recurring inputs, and compact assisted
pasture with higher infrastructure and supply costs. Sustainable output should reward geography, water
access, pasture area, routing and buffering rather than repeated chasing or clicking.

### Dependable movement and honest bodies

Targets must be reachable. Drinking ends at an accessible bank; grass behind a closed fence is not a
valid lure target. Use bounded native route decisions, meaningful target commitment and separate enter/
exit thresholds so herds do not oscillate between thirst, grazing and alarm. Standing and resting are
normal behaviours. Remove Follow until something actually separates from a simulated herd centre.

A centre-point segment test is insufficient for drawn bodies. Keep formations inside boundaries, compress
them through gates, and invalidate affected active legs when fences, buildings or water access change.
Resolve placement intersecting a herd explicitly without trapping bodies or teleporting them through a
wall. Hunting must clearly target a herd or a native-resolved victim at the visible hit location; never
click one body and drop a carcass somewhere unrelated. Per-animal identity is not required for this slice,
but rendering may not promise individual targeting that native cannot honour.

Ordinary walking causes a modest personal-space response; deliberate driving and hunting cause stronger
flight. Alarm decays, unreachable bait cannot dominate decisions, and feed does not override immediate
fear. No machine-noise radius in Phase 9, consistent with the surviving habitat brief. Soft interaction
must keep the player from being wedged while still producing a visible animal response.

### Needs, reproduction and recovery

Settle reproduction, minimum reserve, population bounds and recovery alongside grass consumption.
Shortage first suppresses reproduction; prolonged shortage warns, encourages migration, and only then
causes gradual loss. Wild and enclosed herds follow the same rules. Closing a gate cannot switch on a
special starvation rule. Recovery needs a documented migration source and a bounded fallback when all
nearby source herds are gone; no infinite respawn farm or permanent extinction.

Separate recently eaten grass from waste fouling. Resting pasture restores eaten cover; waste collection
removes ongoing pressure; restoration helps damaged ground recover. Clearing waste does not instantly
replace eaten grass, and grass does not ignore waste that is still present. Nutrient residue improves
recovery toward a habitat limit, not unlimited fertility or a self-amplifying production loop.

The inspector names the limiting cause and an executable remedy, such as water access blocked, pasture
recovering, feed unavailable or reserve reached. Warn before losses. The world communicates the same
trend through cover, posture, motion and abundance. Hunting needs clear targeting, windup and cancellation;
choose its control and unlock before the economy slice, without conflicting with ordinary gather.

### Programme and player appearance

Riverbank Renewal accepts restoration of damaged habitat OR establishment of additional healthy habitat
with demonstrated sustainable yield. Never require players to damage a healthy installation to qualify.
Reward capabilities and visible hub improvement; demonstrate an unattended installation rather than a
repeatable maintenance task.

The player should read as a small human worker: simple work clothes, a readable face and cap, slender
limbs, modest carrying equipment and a tool shown during work. Remove bulky robot armour, glowing visor,
antenna and ornamental mechanical attachments. Keep a readable silhouette and the existing movement/work
feedback. Visual scale changes do not alter native collision, position, speed or reach.

### Revised acceptance checks

- A player predicts where a herd goes and explains a refusal without guessing hidden radii.
- A gate, drinking route or pasture edit produces the expected visible response, including mid-leg edits.
- Every drawn body respects containment at corners and narrow gates; hunt feedback matches native results.
- Pristine, grazed, recovering and fouled ground read differently at normal zoom, including after reload.
- Native grazing and regrowth drive local visual updates; no simulated grass or animal needs in the host.
- Shortage is apparent before losses, and local collapse has a practical recovery route.
- At least two layouts are useful; a correct ecological line runs unattended for ten minutes.
- Hunting remains optional after automation and every mandatory joint output has a current use.
- Save/replay tests cover needs, breeding, disturbed legs, recovery and competing targets, not only arrival.
- Measure total scheduling, proximity lookup, path decisions, regrowth and invalidation cost. Arrival-count
  arithmetic alone is not performance evidence; a moving herd is not an idle entity.

The technical slices below are integration checkpoints, not independently complete gameplay releases.
The full Phase 9 gate still requires the surviving brief's processing, automation and programme loop.

## What this does to the current Phase 9 diff

Because the work is uncommitted, this is a **replacement, not a migration**: no save 46, no wire 25, no
deprecated fields to carry forever. Revert the save and wire envelope bumps rather than adding another step.

| Dies                                                                 | Survives                                                    |
| -------------------------------------------------------------------- | ----------------------------------------------------------- |
| `population: bool` item flag and its validators                      | Kiln recipes and the hub request, retargeted at hunt yields |
| `population_field` and the `.or_else()` in `buried_field_at`         | `PopulationMeshes`, largely as-is                           |
| `population_work`, `wake_population`, `seed_population_work`         | The forage table, becoming grass regrowth rates per terrain |
| `population_capacity` and `habitat_waste` ring scans                 | Waste-fouls-habitat, becoming a fouled-grass overlay        |
| `population` / `carrying_capacity` on habitat rows; wire 25; save 46 | The siting hash, becoming herd spawn siting                 |
| `resources_replace` and `deposit_links.clear()` per herd step        | The `ARCHITECTURE.md` / `ART.md` doc slots, rewritten       |
| Extractor and hand-gather coupling to animals                        |                                                             |

Three of the six defects raised in the Phase 9 review stop existing rather than being fixed: the extractor
strip-mining trap, the gather-target hijack, and the per-epoch whole-world resend.

Delete the untracked scratch scripts `phase9_view.py` and `phase9_edit.py` in the repository root; they
describe an older iteration and must not be committed.

## Slices

Each slice is independently testable. A1 removes the animal resource-table resend, but the complete
player loop is accepted only after automation, processing and recovery are integrated.

- [x] **Slice A1 — spine.** Herd list, legs, wire group, save and checksum. No behaviour beyond `Rest`:
      herds wander, are drawn, survive saves and replay identically.
- [x] **Slice A2 — grass.** Grazed overlay plus `Graze`. Grazing eats, grass regrows, herds move because the
      food ran out.
- [x] **Slice A3 — alive.** `Thirst` and `Flee`. Player proximity now matters.
- [x] **Slice A4 — economy.** Hunt, carcass, butcher. Retire biomatter-as-ore and reconnect the kiln and hub
      request to real yields.
- [x] **Slice A5 — husbandry substrate.** Boundaries block creatures; feed lures. Pens and driving fall out
      of existing systems; Phase 10 inherits a working substrate. Legs are checked by the same boundary
      authority the player walks under, so a closed ring already contains a herd. Cut feed can be packed
      by hand or mown, then placed as a lure that ordinary pickup will not immediately undo. Driving is a
      deliberate flee from the inspector. A herd station and pasture tender use one reserved rear working
      cell, keep a breeding pair, and pause rather than inventing animals or remote-subtracting them.

## Tests

All but one are written, in `factory-wasm/src/tests/fauna.rs` and the unit test in `fauna.rs`.

- [x] Replay equality: save mid-leg, restore, advance, compare checksum and herd rows.
- [x] Leg arrival is exact: a herd's derived position at `arrive_tick` equals `to`.
- [ ] Sparse cost: an idle chunk with no herds and no grazing does zero per-tick work. Not written as a
      measurement; `regrow_grass` returning early off its cadence and iterating only the sparse map is
      the mechanism, and `resting_herds_publish_nothing_between_arrivals` covers the wire half of it.
- [x] Grass is a stock: grazing decreases it, regrowth restores it, and a stripped hex pushes the herd off
      without any explicit capacity rule.
- [x] Boundaries contain a herd: a fully fenced ring keeps every leg inside it.
- [x] Flight: the player approaching within flight distance sets `alarm` and produces an away-facing leg.
- [x] Thirst arrives: a thirsty herd commits its adjacent leg to a drinking bank, checked against the
      ecology's own reading of watered ground rather than against the rule the herd steered by.
- [x] No deposit coupling: an extractor beside a herd finds nothing, and the player's nearest-gather target
      ignores animals.
- [x] Wire economy: a herd resting for many ticks produces no creature rows, and no herd change ever sets
      `resources_replace`. The cross-language fixture pins a herd mid-leg and a removal as well, so the
      TypeScript decoder cannot drift from the encoder on the new group.

## Open questions

1. **Species table breadth.** Recommended: build the table in Slice A1 with one species populated.
2. **Do animals block the player?** Recommended: no — soft collision, animals yield.
3. **Can herds starve?** Use the same needs in wild and enclosed habitat. Shortage first stops breeding, then causes visible distress and migration, then gradual local decline. Define recovery from migration and a fallback when no source herd remains; no permanent world extinction. _Partly settled: migration is implemented — an unmet drive commits a leg down the drainage rather than standing still — and seeding guarantees the shipped world is viable, so wildlife no longer dies out on its own. Restocking after the player empties a region is still open._
4. **Predators in scope?** Recommended: not in Phase 9. They are the cheapest source of "your pen needs a
   wall, not a fence", but they also make the first hour hostile in a game that currently is not.
5. **Hunt input.** Settled: the herd inspector aims for one second from the start of a run. No
   technology gate. Moving cancels; a closed fence blocks the shot at release.

## Handoff log

- Design accepted in outline after the Phase 9 review; superseded the incremental fix list for populations.
  No code written. Next action is to settle the open questions above, then start Slice A1.
- Slices A1–A4 written. Open questions 1 and 2 are settled by the code: one species is populated, and
  animals yield rather than block. Questions 3, 4 and 5 stand — shortage currently stops breeding, then
  emits one warning event, then removes one animal at a time with no migration or restocking path;
  predators are absent; and the hunt has no player-facing control yet, only the core verbs.
- Finishing pass: brought the suite green (141 native, 262 host), regenerated both cross-language
  fixtures, and split two files the context budget forbids growing — the entity and herd codecs out of
  `wire.rs`, and the world-access survey out of `balance.rs`. Fauna routes to the simulation shard of
  the agent map, which is where a tick system belongs and also what keeps the native shard under its
  ceiling.
- Runtime verification found the model's one fatal defect: every herd in a fresh world was dead by tick
  ~8,300, without the player doing anything. Three causes, each measured before it was changed. Herds only
  searched four hexes for water, and the opening world's rivers are cut into their benches, so the seeded
  herds stood six and seven legs from a drink they could not see. Finding nothing, `decide_herd` returned
  the herd's own position, so a thirsty herd stood still and ran down its own shortage timer. And
  `seed_herds` sited animals by terrain alone, with no question asked about whether water was reachable at
  all. Thirst now searches ten where grazing still searches four; a search that comes back empty migrates
  down the generated drainage instead of waiting, that being the one heading a watershed cannot dead-end;
  and seeding requires a drink within six walked legs, flooded once out of the water per chunk rather than
  once per candidate. Density moved from one hex in forty to one in five to hold the population, since the
  new rule rejects about nine candidates in ten. Measured over 15,000 ticks on `new-game`: 13 herds and 39
  animals at the start, 48 animals at the end, no losses, drives cycling Rest → Graze → Thirst. Open
  question 3's "no permanent world extinction" now holds for causes the player did not create; the
  restocking path for herds the player does wipe out is still unwritten.
- Next action is Slice A5, and then folding the accepted rules into `ARCHITECTURE.md` and `ART.md` so
  both temporary Phase 9 files can be deleted.
- Slice A5 written: hunt preview and a one-second aim, placed feed as a lure, driving, a herd station
  with a physical feed apron and a two-animal reserve, and a pasture tender that mows or restores the
  reserved cell. Lone animals can reunite; an emptied region still needs a living source. Save 46
  migrates to 47 by advancing the definition stamp only. The remaining Phase 9 gate is the programme,
  guidance, and a committed unattended-run measurement — not more verbs. Fold the accepted rules into
  `ARCHITECTURE.md` and `ART.md` and delete both temporary files when that gate closes.
