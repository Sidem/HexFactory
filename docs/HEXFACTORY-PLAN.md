# HexFactory — goal, state, and roadmap

This is the live document. It says what the game is for, what exists today, and what to build next.
Architecture decisions are in `docs/ARCHITECTURE.md`, the working invariants an agent must not break
are in `AGENTS.md`, the art rules are in `docs/ART.md`, and every performance claim is backed by
`docs/BENCHMARKS.md`.

## The goal

HexFactory is a game first: a beautiful, open-ended factory-automation game that is fun to play for
its own sake, fascinating to keep exploring, and a pleasure to control — drawing on Factorio's
automation depth, Satisfactory's sense of place and scale, and Minecraft's freedom to build what you
want where you want, expressed in hexagonal space rather than square.

The deterministic Rust/Wasm core, the sparse architecture, the compiled transport graph, and the
narrow `@hexlife/embed` geometry dependency are the means to that end, not the end itself. They
exist because a large, living world that never stutters and never loses a save is a player
experience before it is an engineering result. Where an architectural preference and the player's
experience genuinely conflict, the player's experience wins and the architecture has to find another
way to pay for it. That ordering weakens no invariant — determinism, native ownership of the tick,
sparse cost, and measured-before-claimed all stay non-negotiable. What changes is _why_ they are
there, and therefore how milestones are chosen: engineering work earns its place by naming the
player-visible thing it enables.

The design intent is inspiration, never imitation. Original neutral shapes, names, and systems only.
This is permanent and is not a scope item.

### Design pillars

- **Fun is a requirement, not a polish pass.** A release that is correct, fast, and joyless has not
  met its acceptance criteria. Every milestone states what it makes better to play.
- **Controls must be obvious in the first minute and precise in the hundredth hour.** A control that
  needs explaining is a defect in the control.
- **The player should always know what just happened and what to try next.** Feedback for gathering,
  placement, blockage, depletion, research, and delivery is part of the mechanic, not decoration.
- **The world should reward looking at it.** Readability first — resources, machine identity,
  direction, throughput, and blockage legible at a glance — and beauty close behind it.
- **Open-ended, not aimless.** Progression opens options rather than prescribing a route. Victory is
  a milestone in a longer game, never a wall; visible hub programmes, regional discoveries, and
  consequences give that longer game reasons without turning it into a script.
- **The world and the factory answer each other.** Terrain is more than a placement mask and the
  factory is more than an overlay. Geography, living populations, extraction, waste, and recovery
  change one another in ways the player can see and choose around.
- **Hexagonal space earns its place.** A player-facing system becomes hex-native only when faces,
  rings, fronts, or multiple approach directions change a legible decision. Never force the factory
  into a cellular-automaton kernel or add invisible adjacency bonuses merely to justify the grid.
- **Nothing may stutter.** Frame stability, instant response to input, and saves that always restore
  exactly are player-experience features. This is what the measured capacity ladder protects.

## Where the project stands

The engine arc, the generator arc, and the first four milestones of the pivot from substrate to
motive are all shipped. A run today looks like this: land beside a hub in a world chosen by preset
or by raw parameters, walk out under fog across rivers and coastline, find **fields** of eight raw
materials rather than scattered cells, gather, fill the hub's posted **requests** for insight and
its staged founding **contract** for hub growth, research a twelve-technology tree, and build a
powered, automated line of twenty buildings and fourteen recipes across five machine categories.
Buildings are drawn by a shape grammar, so a tier is a data row. Power is energy bought per unit of
work. The world and the minimap render on WebGL2.

**Current envelope numbers** — native refuses a load on all five, and the browser's named-save
catalog shows which one moved rather than hiding the row:

| Envelope              | Version |
| --------------------- | ------: |
| `HXF1` save           |      10 |
| Definitions           |      10 |
| Technologies          |       5 |
| Scenarios             |       5 |
| World generator       |       8 |
| Wire (snapshot delta) |       5 |

**Current measured capacity.** A complete browser frame at 6,144 entities is 19.0% of 60 Hz
(v0.13.1 record, Canvas 2D — the WebGL2 pass has not been re-measured). Generation costs at most
1.42 µs per hex on the v0.21 site lattice, against 0.52 µs for the model it replaced on the same
harness. See `docs/BENCHMARKS.md`; no claim beyond a recorded tier is supported.

**The shipped ledger is at the bottom of this document**, one line per release. Read it for what
exists; read the section a milestone names when you need the reasoning behind a rule you are about
to change.

## What to do next

**Crossings and Canopy v0.22.** The order v0.21 → v0.22 → v0.23 is load-bearing rather than a
preference; the roadmap decision below is why, and v0.21 has now shipped.

**v0.22 is the second half of a version train and it is owed a debt.** v0.21 put rivers in the
world and there is no bridge yet. Shallows are now a 1 m/s ford rather than a wall, so inland
water is a slog instead of a closed door — eight hexes of river is still several seconds of
wading, and a belt still cannot cross. That is the argument the bridge still has. Rivers are
8–10 hexes thick and a few hundred hexes apart. `archipelago` ships with `river_width: 0` because
scattered water everywhere plus a river network would have left its walkable ground in shreds.
**Do not raise the share of river hexes until the bridge exists.** Thickness is not density.

v0.22 was written to ride v0.21's save break rather than spend a second one. That break has now been
spent — the world generator is at 8 — so if v0.22 changes the wire's orientation index it pays for
its own bump, and the orientation-index decision in that brief reopens on those terms.

Do not start v0.23 first. It is written to be tuned against the world v0.21 has now built, and the
figures it should be tuned against are in the shipped ledger's v0.21 entry.

### Open decisions, each with what would settle it

- **Does `regrowth_ticks` move** now that a forest cell holds one to four wood instead of ten to
  twenty-two? (v0.23 — the shape change shipped and the rate change with it; what has _not_ been
  measured is an extractor's starve rate over seven cells against a `regrowth_ticks` of 90. The
  balance report's `mean_same_material` for wood is 5–11 units at the base reach and 11–26 at the
  deep one, which says forestry is a question of area, but says nothing about the cadence.)
- **Is the single-cell footprint restriction lifted** now that its stated reason is gone? (v0.22 —
  the brief recommends **not** now, and says why. It needs a definition asking for it.)
- **Is one board slot reserved for the deepest eligible request?** (v0.23 — consider it, measure it,
  and reject it in writing if a three-slot board cannot afford the reservation.)
- **Rails or free-floating panels** was settled by shipping the rail. What would reopen it: wanting
  positions the player chooses, at which point the rail becomes the docked default rather than the
  destination, and the saved-coordinate, overlap, off-screen-recovery, and touch-gesture questions
  all come back.

One decision that is **not** open and must not be reopened casually: `DIRECTIONS` stays six. Twelve
headings are transport only. Widening adjacency would let a boiler reach a turbine two rows away and
a pole span a distance no player can see.

### Open, unassigned to a milestone

- **A timed keyboard-and-pointer playtest of the opening, done by a person.** Owed since v0.18 and
  still outstanding: the agent's browser pane does not composite, so `requestAnimationFrame` never
  fires and nothing on the player's own clock — walking, running, gathering, the cooldown
  — has ever been exercised. `fixtures/balance.json` predicts the material work (32 gathers to
  contract stage one, 97 to stage two, a 65-second combined hand floor) and says nothing about
  walking, choosing, or placing. A number from a person outranks every number in that file.
- **The WebGL2 renderer has not been benchmarked.** It replaced the Canvas 2D world and minimap
  draws that `docs/BENCHMARKS.md` records, so the current browser-frame record describes a renderer
  the game no longer ships. Re-measure before quoting a frame number.
- **Belts on field cells stay legal, but paving a crystal field without reading it first should
  not.** The clearing no longer holds a crystal cell to pave, so this is now about the highland
  disc a player walked to rather than about the first minute.
- **The opening has not been walked since it stopped being a supermarket.** Every material used to
  be inside the clearing; now the nearest guaranteed patch is nine hexes out, coal and stone and
  clay are fifteen to twenty-five, and copper is twenty-five to forty. `fixtures/balance.json`
  prices the gathers and says nothing about the walking. This is the same debt as the playtest item
  above, and v0.21 made it bigger rather than smaller.
- **The header `Establish component production 0 / 3` never explained the 3.** Largely answered by
  v0.18's contract bill and v0.20.1's item chips; confirm in a real session before closing it.

## Roadmap decision — the world the economy stands on

Decided 2026-08-19 from play, after Standing Requests v0.20 shipped. Two reviews ran in one session:
the research economy, and the world it is played on. The economy review produced five notes and the
world review produced one, and **the ordering between them is the decision**, because one depends on
the other.

The economy notes, verbatim from the session, are: fractional deposits into containers; insight that
requires processed goods rather than only what can be mined; resources that require a building to
extract, with hand-gathering rates that differ by material; extraction and effect radii that are
apparent on every building that has one; and the fact that the entire technology tree can be
unlocked by hand-mining alone. Every one of those was confirmed against the code and is written up
in **Earned Insight v0.23** below.

The world note is that resources arrive as scattered single cells of every kind at once, and it was
asked for in the same session: fields of iron and coal, forests instead of lone high-yield wood
hexes, rivers with clay on their banks and bridges to cross them, sand on ocean beaches, stone in
mountains, and a world where standing an extractor next to a deposit is worth doing. That was
**Landforms and Fields v0.21**, which has shipped; the bridges are the one part of it that did not,
and they are v0.22.

**The world work came first, and the dependency was real rather than tidy.** Making a hand gather
slower per material only reads as "go and build an extractor" when there is a field worth putting an
extractor on. Applied to the generator this replaced — where a continental survey found iron in 205
scattered cells and stone in 18, and where **stone had no workable patch anywhere in 26,307 land
hexes** — slower hand mining would not have been an incentive, it would have been tedium. So the
generator landed first, and the economy is now tuned against the world that exists.

**Regional Discovery is split, not deleted.** Its _generation_ half — a landing clearing that
guarantees a bootstrap path rather than a sample platter, and a survey that proves every preset still
works — was exactly what v0.21 had to do anyway for fields to mean anything, so it moved forward into
v0.21 and shipped there. What stays at v0.25 is the half that is a play system rather than a
generator.

## Then — Crossings and Canopy v0.22

v0.21 makes the world; v0.22 makes it legible and crossable. It is deliberately second because a
bridge over no river and a forest renderer with no forest are both untestable.

### A bridge is an entity override, never a terrain change

`Terrain::blocks_movement` is pinned in both languages by `fixtures/terrain-passability.json`.
Shallows are already a ford (passable, not buildable, 1 m/s); deep water and cliff still block.
A bridge does not turn shallow water into land. It is an entity whose presence `player_blocked`
and the placement path consult — both already walk entities — so the pinned table keeps saying
exactly what it says and gains a note explaining that a bridged hex is passable by entity, not by
band, which is what lets a belt cross what a player can already wade.

- `BuildingKind::Bridge` is **appended** after `Boiler`. Kinds travel as their declaration index, so
  inserting it anywhere else is a silent mistranslation rather than a decode failure; the wire
  fixture's enum table is what catches that, and it must be regenerated and its diff read.
- A new `PlacementRule::Shallows` — _on_ a shallow-water hex. Do not reuse `Water`, which means
  buildable ground _beside_ water and is what the pump uses. Deep water takes no bridge, so deep
  water finally becomes a real barrier and the deep/shallow split earns itself.
- Adding to `BuildingKind` forces a `BUILDING_SHAPES` entry, since the table is total over
  `SilhouetteKey` and `SilhouetteKey` includes `BuildingKind`. That is the compiler asking for the
  drawing, and `docs/ART.md` Stage D is what it should be answered from.
- Belts and risers may be built on a bridged hex. That is the point of it.
- Cost stone and timber, behind its own cheap technology after Field Logistics. Crossing the first
  river should be an early, satisfying unlock rather than a late convenience.

### A forest has to look like a forest

Today every resource cell draws identically: a pulsing hex outline plus the item's icon glyph, one
per hex, whatever the quantity. A forest of 1–3 wood per cell drawn that way is a field of log icons.

Draw **one tree per remaining unit**, deterministically jittered inside the hex from the same shape
vocabulary the buildings use, so a forest visibly thins as it is cut and thickens as it regrows.
`quantity` and `initial_quantity` are already in the resource snapshot, so this costs no new wire and
no new native state — it is presentation over numbers that already cross. Rivers and bridges get the
same treatment: a river should read as a river at ordinary zoom, not as a line of ponds.

### Every radius that exists is drawn

`drawPowerCoverage` draws exactly two rings and both come from `supply_radius` — the pending pole
under the cursor, and the selected pole. Extractors and pumps draw nothing, and it is worse than a
missing ring: the catalogue chip "Reaches N" is conditional on `extract_radius` being present, and
the **base** extractor omits the field entirely, so the first extractor a player ever builds states no
reach anywhere in the UI. The pump's radius is `PUMP_RADIUS`, a bare native constant that reaches no
definition and no panel.

Fix it in the order that matters, and fix the data first:

1. `extract_radius: 1` on the base extractor and on the pump in `definitions.json`, with native
   reading the field instead of the constants. `EXTRACT_RADIUS` survives as the **hand's** reach
   only. This is the v0.19 pole lesson applied where it was not: reach is a property of the thing
   that has it, never a default the host guesses.
2. Generalize `drawPowerCoverage` into a reach pass over **every radius a definition states**, for
   the pending tool and for the selection, colour-coded by meaning, because two rings that mean
   different things must not look the same. The complete list, and a definition growing a radius
   later must join it:

   | Field            | Building        | What it means                       | Treatment                                                                                                                                     |
   | ---------------- | --------------- | ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
   | `extract_radius` | Extractor, Pump | Cells this machine can draw from    | Filled disc, bright rim                                                                                                                       |
   | `supply_radius`  | Pole            | Machines this pole powers           | The existing blue disc                                                                                                                        |
   | `pole_reach`     | Pole            | How far the **next pole** may stand | Rim only, no fill — it is a distance to another pole, not an area of effect, and drawing it as a disc would claim it powers everything inside |
   | `EXTRACT_RADIUS` | The player      | What the hand can take              | Drawn on the player, see 3                                                                                                                    |

   `pole_reach` is the one that is currently invisible everywhere — no ring, no chip, nothing — and
   it is the number that decides whether a grid can be extended at all. A player laying a line of
   poles is guessing at it.

3. Draw the hand's own reach around the player while the harvest key is held. After v0.23 that ring
   is the question the opening is about.
4. Every radius in that table also states itself as a chip on the catalogue card. "Supplies 3" ships
   today; "Reaches 1" and "Links 6" do not.

`extraction_reach_comes_from_the_definition_and_the_hand_keeps_its_own` is the test that already
guards half of this and must be extended, not replaced.

### Twelve headings, not eight

Asked for from play, and it is a generalization rather than an addition: **the eight-direction
routing table is an irregular subset of the regular twelve, and due north is the only member of its
family that was ever implemented.**

A pointy-top hex has six neighbours through its edge midpoints, at 0°, 60°, 120°, 180°, 240°, 300°,
and six vertices at 30°, 90°, 150°, 210°, 270°, 330°. `TRANSPORT_DIRECTIONS` holds the six edges plus
`(1, -2)` and `(-1, 2)` — which are two of the six _vertex_ directions. The other four are simply
absent, for no reason the geometry supplies.

Applying the axial 60° clockwise rotation `(q, r) → (-r, q + r)` — the same rotation the six edges
already follow in their table order — to due north closes the family:

| Index | Axial      | Screen angle | World length     | Heading         |
| ----- | ---------- | ------------ | ---------------- | --------------- |
| 6     | `(1, -2)`  | 270°         | `3 · HEX_RADIUS` | North           |
| 7     | `(2, -1)`  | 330°         | `3 · HEX_RADIUS` | East-north-east |
| 8     | `(1, 1)`   | 30°          | `3 · HEX_RADIUS` | East-south-east |
| 9     | `(-1, 2)`  | 90°          | `3 · HEX_RADIUS` | South           |
| 10    | `(-2, 1)`  | 150°         | `3 · HEX_RADIUS` | West-south-west |
| 11    | `(-1, -1)` | 210°         | `3 · HEX_RADIUS` | West-north-west |

Every corner heading is exactly `3 · HEX_RADIUS` long against `√3 · HEX_RADIUS` for an edge step, so
edges and corners together are a **uniform twelve-point rosette at 30° spacing with two alternating
lengths**. Three edge axes and three corner axes, six headings each.

**The straddle generalizes exactly**, which is what makes this safe. A north riser passes between
`(q, r-1)` and `(q+1, r-1)` and leaves both free, buildable, and walkable. The midpoint of `(2, -1)`
from the origin lands at `(1330.5, -768)` in world units, which is precisely the midpoint between the
centres of `(1, 0)` and `(1, -1)`. Same structure on all six, so the "single-cell building whose belt
spans a seam" note on `TRANSPORT_DIRECTIONS` holds unchanged.

#### What this repairs, and what it deliberately does not

`OrientationAxis::Vertical` requires a single-cell footprint, and both the Rust comment and
`AGENTS.md` explain that as "`@hexlife/embed` rotates by 60° and the vertical headings have no 60°
equivalent." That is true **only because there were two of them**: rotating north by 60° lands on
`(2, -1)`, which was not in the table, so the only available turn was the 180° flip between north and
south. With all six present the corner group is closed under 60° rotation and that explanation stops
being true. `src/rendering/buildingLook.ts` already says the quiet part — _"There is no third
vertical heading, so these are named rather than indexed"_ — and `DUE_NORTH` / `DUE_SOUTH` become an
indexed table like the edges.

**Do not lift the single-cell restriction in the same change.** No shipped definition wants a
multi-cell corner-heading building, and lifting a constraint nobody is pushing against is how an
untested path ships. What must change is the _reason_: replace the now-false explanation with "no
definition needs it yet", in the Rust comment, in `src/core/definitions.ts`, and in `AGENTS.md`, so
the next person does not inherit a justification that no longer holds. Whether to lift it is a
separate, deliberate call with a definition asking for it.

#### Three things to decide rather than assume

1. **Index order versus saved orientations — settled by the release train.** `OrientationAxis::next`
   advances by `offset + 1 % span`, so it assumes index order _is_ rotation order. Putting the six
   corners in the rotational order above changes index 7 from South to ENE, and every saved riser at
   orientation 7 would silently re-aim. **v0.21 and v0.22 ship as one version train** — v0.21 moves
   `WORLD_GENERATOR_VERSION` and rejects every existing save already, so v0.22 rides that break and
   the rotational ordering costs nothing. Take the clean order; do not build a lookup table to
   preserve a compatibility that the train has already spent.
2. **`hex_line_vertical`'s determinism argument does not survive.** It uses `.find()` over
   `TRANSPORT_DIRECTIONS[NORTH..]` — first match wins — justified by _"north and south are opposites,
   so at most one of them can ever close, and the choice cannot depend on iteration order."_ With six
   corner headings that sentence is no longer a proof. A spot check of the 30° boundary suggests at
   most one still closes by two, because a target 30° off a corner heading is an edge heading and
   there neither corner closes — but **that is a spot check, not a proof.** Either prove it and write
   the new argument into the comment, or add an explicit tie-break, and pin it with an exhaustive
   test over the corner headings. This is the one place in the change where a wrong assumption makes
   a drag depend on table order, which is exactly what the existing comment forbids.
3. **`Vertical` is now the wrong name.** `Corner` or `Vertex` is what the axis is. That is an
   `orientation_axis` value in `definitions.json` and a definition-version bump, plus the
   `ORIENTATION_AXES` set in `src/core/definitions.ts`, the `OrientationAxis` union in
   `src/core/types.ts`, the `"North / south"` catalogue chip, and the "risers run due north and
   south" line in the transport tool's blurb.

#### The fixture has to grow

`fixtures/hex-directions.json` pins only the six edges. The two corner headings are currently
duplicated by hand in `buildingLook.ts` as `DUE_NORTH` and `DUE_SOUTH`, and **nothing checks that
they agree with Rust.** Widening to twelve is the moment to fix that: pin all twelve in the fixture,
with index, name, and axial offset, asserted from both languages exactly as
`public_direction_protocol_matches_cross_language_fixture` already asserts the six. Adding four
hand-copied vectors to a host file with no cross-language guard would be the defect this milestone
introduces.

#### What does not change

`hex_line_vertical` scans `TRANSPORT_DIRECTIONS[NORTH..]` generically rather than special-casing two
entries, so it needs no structural change beyond point 2 above. `VERTICAL_TIP_SCALE = 1 / √3` is
correct for all six, since the length ratio between a corner heading and an edge step is identical
for every pair. `DIRECTIONS` stays six and must never widen — adjacency, power, boiler and turbine
neighbours are unchanged, and only transport gets twelve.

The economics are unchanged and should still be re-measured. A riser gains four headings at no extra
cost, but the trade is the one north already offered and which was already accepted: travelling ENE
is two belts for 2 iron ore across two hexes, or one riser for 2 iron ore across one hex with the
straddled pair left free. That deal is being applied symmetrically rather than sweetened, so
`fixtures/balance.json` is predicted not to move — run `npm run balance` and confirm the prediction
rather than assuming it.

### What moved out of this milestone

**Fractional deposits into containers shipped in v0.20.1** and are not here. The radius **chips** in
step 4 above share that pass's `itemChip` markup conventions but are a building's stat line rather
than an item, so they stay here with the rings they belong to.

### Acceptance

- A bridge crosses shallow water, carries a belt, refuses deep water, and
  `fixtures/terrain-passability.json`
  is unchanged.
- The wire fixture is regenerated, its diff read, and `Bridge` sits last in `BuildingKind`.
- A forest reads as trees at ordinary zoom, thins as it is cut, and recovers as it regrows.
- Every radius any definition states is drawn as a ring when pending and when selected, and is
  stated as a number on the catalogue card — `extract_radius` on the base extractor and the pump,
  and `pole_reach` on all three poles, none of which appear anywhere today.
- A pole's supply disc and its link distance are visually distinguishable, because one is an area of
  effect and the other is not.
- Transport routes on twelve headings: six edge steps at `√3 · HEX_RADIUS` and six corner steps at
  `3 · HEX_RADIUS`, in rotational order, and a riser can be turned to all six corners.
- `fixtures/hex-directions.json` pins all twelve with index, name, and axial offset, and both
  languages assert against it. No routing vector is written by hand in a host file.
- A corner drag resolves to the same cells every time, and the reason is a stated argument or an
  explicit tie-break rather than the superseded "north and south are opposites".
- The single-cell footprint rule still stands, and every comment explaining it says "no definition
  needs it yet" rather than the 60°-rotation reason, which is no longer true.
- `npm run balance` is re-run and the prediction that nothing moves is confirmed or corrected.

## Then — Earned Insight v0.23

The five economy notes, tuned against the world v0.21 and v0.22 built.

### The measured defect

The whole technology tree costs **113 insight**. One full cycle of the eight raw request rows pays
**73 insight for 72 hand gathers**. `GATHER_COOLDOWN_STEPS` is 15 at `PLAYER_TICKS_PER_SECOND` 30, so
a gather is 0.5 s flat for every material. `carry_slots` is 8 and raw stacks are 20, so ten each of
the eight raw materials is exactly eight slots — **one pack-load is one full cycle**.

112 gathers is therefore about **56 seconds of held right-click and two walks to the hub for the
entire tree**. It is not an exploit; it is the shortest path the data describes. And it is available
because `next_request` reposts raw rows forever — raw materials are always `item_reachable`, since no
building outputs them and the walk short-circuits.

`fixtures/balance.json` already prices this honestly: raw pays 1000 `insight_per_gather_milli`,
processed 1300–1867. Under 2× for a machine, its research, its power, and its fuel, against a raw row
that never runs out.

### Gathering becomes a property of the material

Move the cooldown out of the constant and into `ItemDefinition` as `hand_gather_steps`, where
**absent means the hand cannot take it at all**.

| Material                   | Steps | Seconds | Reading                                            |
| -------------------------- | ----- | ------- | -------------------------------------------------- |
| Wood                       | 15    | 0.5     | Flora, cut by hand. The bootstrap fuel stays fast. |
| Clay, Sand                 | 20    | 0.67    | Loose surface earth.                               |
| Stone                      | 30    | 1.0     | Cut off a cliff face.                              |
| Iron ore, Copper ore, Coal | 45    | 1.5     | A hard rock seam.                                  |
| Signal crystal             | —     | —       | **Machine only.** The deep extractor, tier 1.      |

Water is already machine-only and needs no rule: it is terrain rather than a field cell, so
`resource_at_world` never finds it and the pump is the only source. That is the existing precedent
this table generalizes.

### The invariant that moves, and the reason it is safe

`AGENTS.md` and v0.17's record say the hand is worth **exactly** one extractor, both at 120 a minute,
and `one_extractor_is_worth_exactly_the_hands_it_frees` pins it. Restate it as: **the hand is never
faster than an extractor working the same cells, and on hard rock it is materially slower.**

That keeps the guard v0.17 actually cared about — the curve inversion where the first machine a player
builds is slower than the hands it replaces — and adds the incentive the notes asked for, because the
new rule is strictly stronger in the direction that mattered. Move the invariant text, the test, and
the fixture's extraction section together, and say in the shipped record that the equality was
deliberately broken rather than lost.

### A raw row pays once at full price

Add `repeat_insight` to `RequestDefinition`. The first fill pays `insight`; every later fill pays
`repeat_insight`. Raw rows get **2**. Processed rows keep their full value.

`request_rounds` already counts fills and is already saved and checksummed, so nothing new enters the
envelope. The eight surveys pay ~73 once, which comfortably funds the early tree — Field Logistics,
Automated Extraction, On-site Power, Storage Planning, and Composition come to 30 — and the remaining
83 has to come from a smelter or a kiln.

A decayed repeat rather than an exhausted row, deliberately: a row that stops existing can strand a
player who has no fuel and no power and no way back, and 2 insight for ten gathers is already a rate
nobody will choose. The floor removes the soft-lock; the number removes the exploit.

`every_processed_request_pays_better_per_gather_than_raw_material` needs a companion asserting that a
**repeated** raw row pays worse per gather than every processed row, which is the claim that actually
holds this together.

### What replaces grinding, and why it is mostly already built

Closing the raw loop only answers half the note it came from. The other half — insight should require
items that are processed **according to how deep the player is in the tree**, so that funding research
is what leads them to discover more — must not be built from scratch here, because v0.20 already
built it and this milestone's job is to finish it.

Two shipped mechanisms carry it:

- **Eligibility is the recipe tree.** `Core::item_reachable` walks from a requested item down
  through
  the recipes that make it, requiring a buildable machine for every category and an unlocked source
  for every leaf. A row cannot be posted until the player could actually produce it, so the board
  opens up as research does, without an unlock column anybody has to maintain.
- **The reward curve is already depth-scaled**, in `insight_per_gather_milli`: raw 1000, crystal
  1250,
  one machine step 1300–1333, assembly 1533–1625, the deep chains 1778–1867. `fixtures/balance.json`
  computes it from the shipped tree rather than from authored intent, which is how v0.20 caught its
  own first pass putting glass at exactly 1000.

So the depth ladder exists and is measured. What v0.23 owes it is the two things that currently
undercut it:

1. **A floor that makes the ladder the only way up.** Under 2× is a weak gradient when the bottom rung
   is infinite; `repeat_insight` is what makes the gradient decisive, and it should be tuned against
   the printed curve rather than picked. If 2 leaves any raw row competitive per _minute_ once the
   new hand rates land, move it, and say which figure moved it.
2. **A board that leads rather than waits.** `next_request` posts the least-used eligible row, so
   fresh content does lead — but only in catalogue order, and nothing guarantees the three posted
   slots are not all shallow rows the player has long outgrown. Consider reserving one slot for the
   deepest eligible row. Consider it, measure it, and reject it in writing if a three-slot board
   cannot afford the reservation: a player who cannot yet build a smelter must never face a board of
   three things they cannot make, which is the trap `skip_request` exists to escape and which a
   reserved slot could quietly recreate.

Neither is a new system. Both are one predicate each, and both belong in this milestone rather than a
later one, because they are what make the hand-rate change read as an invitation instead of a tax.

### Acceptance

- The technology tree cannot be completed from raw materials alone, demonstrated by a native test
  that fills every raw row repeatedly and shows the reachable insight falling short of 113.
- Hand-gathering rates come from the item definition; a material with no `hand_gather_steps` refuses
  a hand gather with a message naming what does extract it.
- No extractor is slower than the hand on the same cells, on any material, pinned in the fixture.
- Signal crystal is obtainable only by machine, and the guidance says so rather than leaving the
  player to discover it by failing.
- `fixtures/balance.json` reports the per-material hand rate and the repeat rate, and both test
  suites expand it independently.
- A repeated raw row pays worse per gather **and per minute** than every processed row, measured
  against the new hand rates rather than the old flat one.
- The board never posts three rows the player cannot supply, whatever is decided about reserving a
  slot for depth. If the reservation is rejected, the reason is written down.

## Deferred — Living Lattice v0.24

Animals, biomatter, and waste remain one milestone, and the purpose is sharper than the roster: this
is the first system that makes HexFactory something other than a factory game drawn on hexes. A
living population moves, feeds, breeds, recovers, and can be depleted past recovery across hex
neighbourhoods. Biomatter comes from that population rather than from a renamed static field. Waste
is a byproduct with a visible destination: it can feed a recovery loop, damage a habitat, or be
refined. Producer, byproduct, and consumer are designed together so none is a decorative item.

This is **not** a pollution-and-enemy-wave substitute. The pressure is ecological consequence and
opportunity, not a timer that periodically sends attackers. A player should be able to preserve a
productive migration, intensify it carefully, exhaust it for an urgent contract, or repair a region
they damaged. The landscape answers the factory, and the answer is visible where it happens.

Hex topology earns itself here. Movement and propagation use six neighbours; a herd or recovery
front has a perimeter; extraction reach is a ring; machines expose meaningful faces when a process
has directional intake, output, heat, or waste. Do not add generic adjacency percentages that
collapse into one solved blueprint.

Rust/Wasm still owns every ecological tick. Use sparse scheduled populations or active fronts, not a
JavaScript cell loop and not HexLife source imports.

**Four things it should know before it starts**, all handed over by earlier milestones rather than
invented here:

- **Play the opening first, with hands, and time it.** See the open items above — this playtest has
  been owed since v0.18 and no system should be added before it.
- **`Economy::recipe_for` still asserts one recipe per item, and ecology is what breaks it.** A
  byproduct is a second producer, and "what does a plate cost" has no answer without a stated rule
  for dividing a craft's cost between its outputs. `outputs: Vec<Ingredient>` arrives here for the
  same reason. Pick the allocation rule deliberately and write it down beside the fixture; do not
  make the secondary output free, charge every output the full craft, or silently select one
  producer. The `contracts` and `requests` sections expand bills through the same tree, so they
  break in the same place and for the same reason.
- **A new contract stage is a data row, and the hub already knows how to grow into it.** Stages live
  in `scenarios.json`; `HUB_LADDER` has one entry per stage the hub can finish, and
  `tests/look.test.ts` fails if a shipped contract can complete a stage the ladder cannot draw. If
  this milestone wants the hub to ask for biomatter, that is a stage and a ladder row, not a system.
- **Guidance follows the contract for free, but only through recipes.** `nextAction` walks recipe
  inputs and recipe categories. An ecological input _harvested from a population_ falls out of the
  walk as a raw material, which is right; an ecological _process_ with no recipe row will not appear
  at all. Give it a recipe row or teach the walk about it deliberately — the one thing that must not
  happen is a hub asking for something the next step cannot explain.

### Acceptance

- One complete loop produces useful biomatter and a waste stream, and the player has at least two
  legible responses to that waste with different ecological outcomes.
- The same installation in two habitat states does not have the same answer, and the reason is
  visible in the world rather than hidden in a modifier panel.
- A population can recover, migrate, and collapse deterministically; saves and checksums reproduce
  each outcome exactly.
- The founding hub asks for something from the loop, so the new system has a motive on arrival.
- Every new definition reaches `fixtures/balance.json`, and any ecological yield claim has a
  measured fixture analogous to the world survey.
- The native capacity ladder and complete browser frame are re-measured if the entity or world
  snapshot changes. No claim beyond the measured tier.

## Later — Regional Discovery v0.25

**Its generation half moved forward into v0.21** — the bootstrap guarantee that replaces the sample
platter, and the survey that proves every preset still works. What remains here is the half that is
a play system rather than a generator, and it begins from a world that already has readable
landforms, rivers, and real deposits.

v0.16 made world shape parameterized; this makes that variation a play system. Advanced materials
and ecological opportunities belong to readable regions that require travel, surveying, and
eventually outposts. Every preset remains completable, but "completable" no longer means "sample
platter at spawn."

A third low-frequency generation channel may create dry and wet variants of the same elevation band,
but generation is not the milestone by itself. A region has to announce itself through shape,
colour, life, sound, and material behaviour; the player needs a survey tool that hints rather than
reveals the whole answer; and a distant discovery must create a reason to establish a specialized
site rather than carry one stack home and forget the place. Landing contracts and later hub modules
provide that reason.

Signal crystal is the strongest candidate for a later hex-native automation language: relays along
faces, triangular links, or closed rings can make spatial control distinct from conventional circuit
combinators. It stays a candidate until Living Lattice proves which signals the player actually
needs; do not build a programmable system in search of a problem.

### Acceptance

- Every preset remains completable, measured by an updated survey that reports first
  advanced-region distance, regional extent, and access from buildable ground.
- Crossing into a region is recognisable without opening the game menu or reading coordinates.
- At least one founding-hub project requires a sustained distant site, not a one-time hand trip.
- The minimap and home bearing support the expedition without revealing unsurveyed world or
  re-deriving native generation truth.

## Longer horizon

Named as decisions rather than omissions, each with the thing it is waiting for.

- **Hub programmes.** Player-chosen modules grow around the landing hub's rings and create different
  material demands. Finite authored systems and visible construction, not endless random chores —
  what gives an established factory a reason to expand without turning one victory into a wall.
- **Six-face machines.** Ports, heat, exhaust, or control may attach to named faces where direction
  creates a readable routing choice. Closed loops and triads are available shapes, not mandatory
  bonuses on every machine.
- **Fluid networks.** Water ships as a belted item and says so; the real network is an improvement
  on
  a working game rather than a second network model built beside the first.
- **Intermittent generation and accumulators.** They arrive together. Intermittency has to be a
  deterministic function of tick and position, never a runtime roll.
- **A day cycle, and solar with it.** A presentation and simulation change at once, chosen for what
  it does to the game's feel rather than smuggled in as a power source.
- **Terraforming.** Whether the player may reshape elevation, and what that costs.
- **Tunnels.** One match arm in the graph trace — `None if entity.is_tunnel() && steps < span`, so a
  tunnel entrance rays through empty ground and binds to the first entity it reaches, the covered
  cells stay walkable, and pipes inherit it for free when fluids land. It may ride any compatible
  version bump and does not become a milestone by itself.
- **Organic tileables.** The later half of the art generator: systems that produce tileable textures
  and shapes so a hex lattice reads as organic terrain and organic objects. Same invariants —
  generated, presentation-only, derived from published snapshot facts, never a checksum input.
- **3D presentation.** The camera tilts and orbits the player; terrain, buildings, and the player
  gain shape. This is a renderer replacement, which the invariants already allow, and the WebGL2
  pass is a step toward it. Height is not implied as a gameplay dimension until a later pass names
  what it is for — smuggling a z-axis into the checksum because the camera can tilt would be the
  same class of defect as frame-coupled movement. A hand-authored mesh per definition is the atlas
  again; a mesh derived from `recipe_category` and tier is the shape grammar in another dimension.
  A renderer decision is a measured decision.

Whatever comes next, `fixtures/balance.json` remains the thing every new building or recipe has to
face: a definition that never reaches it is a definition nothing has compared against the curve, and
both test suites say so.

## Shipped ledger

One line per release, newest first. The reasoning behind a shipped rule lives in the git history of
this file and in the code that implements it; what follows is the index.

- **World scale** (2026-08-20) — One hexagon is 1 m². The walk is 3 m/s; Shift runs at 5 m/s.
  Shallows are a 1 m/s ford; deep water still blocks. Landform cells moved from 5–20 to 128–960
  so a biome takes minutes to cross, rivers are 8–10 hexes thick, and oceans come from the coarse
  octave at last. A landing disc keeps the opening's bootstrap windows on a world that large.
  Generator 8.

- **v0.21** Landforms and Fields — **A deposit is a site, not a hex.** The world is partitioned by a
  `site_cell` lattice; each cell hashes to at most one site — a jittered centre, one weighted rule,
  one radius — and a hex belongs to the nearest site whose disc covers it and whose `member` bands
  it satisfies, ties broken by lattice cell. Yield falls from core to rim. `FieldRule` became
  `SiteRule`, row order stopped being a generation input, and `relaxed()` went with the per-hex
  gates it eased — a preset now compensates in `weight` and `radius_max`, which the survey can see
  and a gate never could. The lattice is cached on `Core`; the field never is.

  Rivers are ridge noise rather than a simulation, `ShallowWater` cut after the band test and before
  the cliff test. Beaches ask the coarse elevation octave alone whether a coast is ocean. The eight
  hardcoded clearing cells are gone: a `bootstrap` pass spirals over lattice cells and guarantees
  iron and forest within 14, coal, stone, and clay within 25, and copper within 40 — never sand,
  never crystal — widening a window in fixed steps and then refusing the world. Generator 7.

  **What it was measured against.** Purity is the share of resource hexes whose radius-1 disc holds
  one material, which is what decides whether an extractor works a field or straddles two. Target
  950; the shipped seed at radius 96:

  | preset        | purity before | purity after | worst before | worst after              |
  | ------------- | ------------: | -----------: | ------------ | ------------------------ |
  | `continental` |           532 |          971 | stone 0      | sand 895                 |
  | `archipelago` |           474 |          965 | wood 36      | sand 940                 |
  | `highlands`   |           662 |          990 | crystal 28   | clay 977                 |
  | `basin`       |           631 |          992 | crystal 71   | crystal 809 _(21 cells)_ |

  Stone had **no workable patch anywhere in 26,307 land hexes** on `continental` before this — its
  largest patch was 3 hexes against a base extractor's 7 — and neither did wood on `archipelago`.
  Every preset now clears 19 hexes for iron, coal, copper, and stone and 61 for forests, and every
  material has a patch an extractor can be stood on. `archipelago`'s landform scale moved from 4 to
  5 and its blend from 45 to 52, because at the old numbers no band held a contiguous run wider than
  a deposit and every disc came out a crescent.

  Generation cost 1.42 µs/hex against 0.52 µs for the model it replaced, on the same harness — the
  site lattice cache is what keeps that from being ~350 noise samples per hex. The capacity ladder is
  flat, as it must be: its scenario never generates.

- **WebGL2 renderer** (unreleased, 2026-08-20) — The world and the minimap draw as instanced GPU
  geometry with the camera as a uniform, so walking no longer restamps the terrain mosaic; a Canvas
  2D overlay keeps the player, labels, and machine decorations. Fixes the 4 tps sluggishness
  (per-frame fog blur, camera-keyed terrain restamp, layout forced by `pickWorld`, full HUD rebuilds
  while walking). **Not yet benchmarked.**
- **v0.20.1** Panels and Item Language — Host-only presentation pass. `itemChip` is the only place
  an item is ever drawn, replacing eight bespoke shapes; affordability is a per-line `CostLine[]`
  shortfall rather than a boolean; panels open independently in two `.panel-rail` flex columns; Take
  and Put are one `renderTransferRows` that moves a chosen quantity. No native, save, definition,
  generator, wire, or checksum movement.
- **v0.20** Standing Requests — `insight_value` is gone from every item — the hub posts a board of
  requests and filling one is the only thing that pays. Eligibility is `Core::item_reachable`
  walking the recipe tree; draw order is `request_rounds` with no randomness; every row carries a
  Pass. `hub_demand` makes the hub take only what it asked for, by belt as by hand. Named saves live
  in a version-independent catalog. Save 10, definitions 10, wire 5.
- **v0.19** Power Grid — Electricity became energy bought per unit of work, not a per-tick tax: a
  machine banks three cycles and idle time is free. Plants burn only against energy delivered.
  Coverage moved onto the pole (`supply_radius`, `pole_reach`, three rungs each) so it can be
  upgraded, and touching machines conduct. Coverage overlay and power meters. Save 9, definition 9,
  technology 5, wire 4.
- **v0.18** Founding Contract — The hub asks for an ordered contract rather than a delivered total,
  and finishing a stage visibly grows the hub through `HUB_LADDER`. The scripted guidance is gone —
  `src/core/guidance.ts` derives the next step from the contract, the recipe tree, and the
  technology graph, so it cannot recommend a factory the rules refuse. Permanent next-step chrome,
  progressive disclosure by technology distance, stall marks, six synthesised audio cues, `Shift`
  precision walk. Save 8, scenario 5, wire 3.
- **v0.17** Balance — `fixtures/balance.json` is every figure that decides whether the economy
  works, computed by `factory-wasm/src/balance.rs` and recomputed independently in
  `tests/balance.test.ts`. The curve is two rules over the data. Six numbers moved on the first run,
  each traceable to a printed figure — the hand beating the first extractor by 2.5×, steam dominated
  by hydro, an overpriced wind turbine, a break-even charcoal conversion, and a cutter cheaper than
  the smelter it follows. Definition 8.
- **v0.16** World Parameters — A world is a seed **and** a `WorldParams`, both saved and
  checksummed. Feature scale and threshold are separate axes. `field_at`'s match arms became an
  ordered `FieldRule` table. Four presets ship as data rows and `npm run survey` is where every
  claim about a landscape comes from — it found two unplayable presets before a player could.
  Generator 6.
- **v0.15** Generated Shapes — A building's drawing is a part list from an eight-part vocabulary in
  `src/rendering/shapeGrammar.ts`; `BUILDING_SHAPES` is total over `SilhouetteKey`, so a new
  silhouette is a compile error rather than a machine that draws nothing. A tier is a modifier on
  the list, motion is a part's `phase`, still parts bake behind `BUILDING_SHAPE_VERSION`.
  `contact.html` renders every definition × tier × status. Presentation only.
- **v0.14.1** Construction Catalogue — Buildings moved behind `B`, grouped by `kind`, each card
  carrying the facts that decide a choice. A recipe is drawn as materials, not a name in a dropdown.
  The bar became a nine-slot hotbar the player arranges, persisted as presentation state.
- **v0.14** Upgrades and Tiers — A tier is a data row validated once by `validate_upgrade_ladders`;
  `upgrade` edits the entity in place so contents, orientation, and connections survive. Extraction
  reach is the flagship upgrade. North and south entered `TRANSPORT_DIRECTIONS` as the riser — a
  direction-table row, not sub-hex occupancy. Right-click harvests one named hex; `store` mirrors
  `withdraw`. Save 7, definition 7, technology 4.
- **v0.13.2** Inspector Readability — A clicked hex is cards rather than a `textContent` dump:
  identity, coordinates chip, terrain swatch, field meter, facing compass, machine meters, belt
  cargo, Take rows. `Direction 0` no longer reaches the player. Presentation only.
- **v0.13.1** Look Systems — Stage B of the art generator: neighbour fringes, baked terrain tiles
  behind a version constant, host-side hash variation, depletion scars, silhouettes from
  `recipe_category`, plus the first Stage C motion pass. Re-measured at 19.0% of 60 Hz.
- **v0.13** Power — Poles compile connected components into networks with integer supply and demand;
  brownouts advance progress with an exact per-entity remainder. Burner, boiler-and-turbine, wind,
  and hydro. Water is a belted item, not a fluid network. Save 6, definition 6, technology 3.
- **v0.12.4** Renderer Measure — The first complete browser frame: 3,039 µs at 6,144 entities, 18.2%
  of 60 Hz, world 909 µs and minimap 160 µs. The unknown 89% of a frame is gone.
- **v0.12.3** Sightlines — The player faces the cursor through a bounded `aim` that carries a world
  position, never a heading. Panels behind `I` / `O` / `P`, leaving the inspector alone on the
  world. A minimap and a gold home-bearing marker. One hatched treatment for every impassable band,
  pinned by `fixtures/terrain-passability.json`.
- **v0.12.2** Binary Delta — The snapshot delta crosses as a compact binary buffer that is
  transferred rather than cloned, decoding to exactly what the JSON path produced. Payload 13.6×
  smaller, boundary 21.7× cheaper, host frame from 62.1% of 60 Hz to 11.0%. Pinned by
  `fixtures/snapshot-delta-wire.json`.
- **v0.12.1** Playtest Feel — The first-minutes follow-up: sparser fields so barren ground is the
  common case, one landing cell per material, smaller hexes, remaining amounts shown on demand
  rather than on every hex, refusals that name the missing item. Generator 5.
- **v0.12** Material Base — Eight raw resources generated where their geography says, and fourteen
  recipes across five machine categories on one `Composer` kind separated by `recipe_category`.
  **Fuel is a property of the item, never a recipe input.** Flora regrows from a derived set of cut
  cells. `Pump` is the only new kind; `set_recipe` is a new bounded command. Save 5, generator 4.
- **v0.11** World Shape — One axial lattice. Terrain bands from integer value noise, cliffs from the
  elevation gradient. Resource fields are a pure function of seed and hex with only a sparse
  depletion overlay stored. Extraction radius. Stage A art direction. Save 4, generator 3.
- **v0.10** Playability — One placement overlap rule for deposits and obstacles alike. Host lists
  carrying a control are patched in place, which is what was silently dropping research clicks. The
  player walks on a native cadence of its own. Carrying capacity as a rule over the inventory, with
  `withdraw` and an all-or-nothing refund. Save 3, definition 4.
- **v0.9** Game Feel — A belt run is one drag: `place_line` and `erase_line` are single bounded
  commands and Rust resolves the path, headings, legality, and cost. The preview comes from the same
  resolver. Undo, one rotation model, pick-block, nine hotbar slots, held gather, movement that
  stops when the key does.
- **v0.8** Browser Capacity — The capacity ladder runs in the browser worker from the same Rust
  implementation. Wasm costs 1.19–1.23× native; the worker boundary was 57–61% of a host frame at
  about 10 µs/KB, which is what made the binary encoding the next milestone.
- **v0.7** Sparse Snapshot — Deltas are built from dirty marks made where state is mutated, not by
  diffing two complete snapshots. Frame cost fell 16.8× at the largest tier and every tier fits
  inside 60 Hz.
- **v0.6** Sparse Cost — Extractors hold a resolved deposit reference instead of scanning every tile
  (tick 233× cheaper); the buildings delta became per-entity (2.3× less payload). Fog of war over
  the generated chunk set.
- **v0.5.1** Capacity Tiers — The deterministic headless capacity ladder and the first measured
  tiers, 12 to 6,144 buildings, excluded from the CI gate but pinned by a workload checksum.
- **v0.5** Worker Boundary — The Wasm `Factory` moved into a dedicated module worker with serialized
  RPC and revision-checked snapshot deltas.
- **v0.4** Command Surface — The world owns the viewport. Command bar, snapshot-derived next-action
  guidance, compact cargo and research surfaces, a lock- and cost-aware construction dock, and a
  narrow touch layout.
- **v0.3.1** Transport Graph — Stable-ID invalidation and affected weak-component recompilation
  replaced full post-edit rebuilds.
- **v0.3** Continuous Exploration — Hex-step movement became native two-axis intent with continuous
  collision and gathering, proximity-limited construction, and definition-driven rotated footprints.
- **v0.2** Playable Game — The architecture proof became a game: seeded deterministic world, native
  player and inventory, data-defined construction and recipes, a native technology tree, `HXF1`
  saves and scenarios.
- **v0.1** Founding slice — The repository, `@hexlife/embed/hex@1.15.0` published and exactly
  pinned, the native vertical slice (extractor → turning belt → composer → belt → container →
  consumer), and the first live Pages deployment.

## Reference — the shipped presets, as measured

From `npm run survey` at seed 1,213,486,160 after the world-scale pass. Each preset is sampled at
its landform radius (continental / highlands / basin 768, archipelago 192), because a 96-hex disc
is the opening, not a landform. Bands in parts per thousand; water as bodies / mean body / largest
body, **rivers excluded** so that `largest body` still means ocean; rivers as hexes / runs / longest
run.

| preset      | water | shore | lowland | hills | highland | cliff |    water bodies |              rivers | purity |
| ----------- | ----: | ----: | ------: | ----: | -------: | ----: | --------------: | ------------------: | -----: |
| Continental |    30 |    61 |     302 |   354 |      238 |    12 |  504 / 19 / 446 |  45604 / 129 / 8801 |    991 |
| Archipelago |   110 |   190 |     317 |   142 |      202 |    35 | 253 / 48 / 2247 |                   — |    981 |
| Highlands   |    35 |     0 |     113 |   410 |      416 |    23 |     85 / 3 / 25 | 63110 / 144 / 17988 |    995 |
| Basin       |    45 |   198 |     524 |   211 |       16 |     2 | 717 / 58 / 7360 |  40103 / 98 / 19096 |    998 |

A hexagon is 1 m²; the walk is 3 m/s. Continental's 512-hex landform is a three-minute crossing,
basin's 960-hex one is six, and a river of eight to ten hexes is a real river. The opening inside
~80 hexes is still the cell-8 mix the bootstrap windows were tuned against, so the first minute
does not wait on a coast.

`basin` is the ocean preset: largest standing body 7,360 hexes and truncated, and the water
nearest a sand patch averages **2,494**. `archipelago` holds a 2,247-hex body inside a 192-hex
sample. `continental` on this seed is inland — no sand in 768 hexes, largest standing body 446 —
which is a continent, not a missing ocean; the coast is the walk. `highlands` still has no ocean
worth the name (largest body 25) and keeps `ocean_level` where those basins pass it, so the little
sand it holds (five patches, nearest 336 hexes) sits on the largest water it actually has.
