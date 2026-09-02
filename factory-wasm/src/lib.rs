use serde::{Deserialize, Serialize};
use std::cell::RefCell;
#[cfg(test)]
use std::cmp::Ordering;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use wasm_bindgen::prelude::*;

mod boundaries;
mod geomorphology;
mod ground;
mod ground_spine;
/// Phase 8 slice 4: departure from generated water equilibrium, and the bounded solve that settles
/// it.
///
/// Movement, construction, earthwork, pumps, bounded flood/drain commands and the snapshot all read
/// it. Frontier departure waits without claiming world and resumes when survey exposes its chunk.
#[allow(dead_code)]
mod hydrology;
mod recipes;
mod runtime;
mod save_migrations;
use boundaries::*;
use ground::*;
use ground_spine::*;
mod skills;
use skills::*;
/// The binary encoding the snapshot delta crosses the worker boundary in.
mod wire;

use runtime::RuntimeIndex;

/// Derived economy figures: what the shipped numbers actually say the curve is.
///
/// Measurement code like the capacity ladder and the survey, and native only for the same reason:
/// nothing here runs a tick, and the wasm artifact the game ships must not carry it.
#[cfg(not(target_arch = "wasm32"))]
pub mod balance;

/// The Phase 8 physical scale contract. New worlds read it for cadence, height, walking and
/// construction; a save from before the 25 m² hex is refused rather than reinterpreted.
pub mod scale;

/// The Phase 8 drainage-first world generator. Slice 3 selects it for new worlds.
pub mod terra;

type ItemId = u16;
type RecipeId = u16;
type DefinitionId = u16;
type TechnologyId = u16;
type RequestId = u16;

const SAVE_PREFIX: &str = "HXF1\n";
/// Bumped to 7 for Upgrades and Tiers. Two things move under it at once: building definitions
/// carry a tier and an upgrade ladder, and `orientation` is now an index into eight routing
/// directions rather than six. Both are checksum-affecting, so a v0.13 envelope is rejected rather
/// than reinterpreted — an old save's orientation would still read correctly, but its definition
/// table would not.
///
/// Bumped to 8 for the Founding Contract. A run's progress is no longer one delivered total against
/// one item: the stage the hub has reached, and what it is holding against the current bill, are
/// saved state and are in the checksum. A version-7 envelope carries neither, and inventing a stage
/// for it would be the loader guessing at a founding project's history.
///
/// Bumped to 9 for the Power Grid. Electricity became a quantity a machine holds rather than a rate
/// it is taxed at, so every machine now carries banked energy and every plant carries its progress
/// toward the next unit of fuel — both checksummed. A version-8 envelope has neither, and defaulting
/// them to zero is not a translation: the same factory would restart with every buffer empty and
/// every plant's part-burned fuel forgotten.
///
/// Bumped to 10 for Standing Requests. Insight is no longer a property of an item the hub is handed;
/// it is paid for filling a request the hub posted, so the board a run is holding and how many times
/// each request has been filled are saved and checksummed. A version-9 envelope carries neither, and
/// a loader that drew a fresh board for it would hand a finished run three requests it may already
/// have filled.
///
/// Bumped to 11 for Earned Insight. Skip and fill both leave a request off the board, but only a
/// fill should decay the payout, so `request_fills` is saved and checksummed beside `request_rounds`.
/// A version-10 envelope has no fill count, and treating its rounds as fills would turn a pass into
/// a two-insight survey.
///
/// Bumped to 13 for Creative Mode. Whether a run is creative, and how many slots the pack has been
/// widened to, are both facts about that run rather than about its scenario: creative builds for
/// free and can raise its own carrying capacity, so both are saved and checksummed. A version-12
/// envelope carries neither, and reading one as an ordinary run would hand a creative save back
/// with its pack silently narrowed to the scenario's number.
///
/// Bumped to 14 for Belt Junctions. A splitter remembers which output it fed last and a merger
/// remembers which feeder it took from last, so both cursors are saved and checksummed. A
/// version-13 envelope carries neither, and defaulting them is not the harmless zero it looks
/// like: every junction in a loaded factory would restart its rotation from the same place, and a
/// run reloaded mid-tick would deal a round it had already dealt.
///
/// Bumped to 15 for click-to-walk. The player carries where they are walking to, because a walk is
/// a standing order the simulation is executing rather than a key being held: it survives a save
/// the way `pending_gather` does, it is checksummed, and two runs that differ only in it are not
/// the same run. A version-14 envelope has no goal, which is a real and representable state — the
/// 14 → 15 step in `save_migrations` writes `null` rather than leaving the field absent, so an old
/// file loads standing still instead of being refused.
///
/// Bumped to 16 for compartment storage and the held stack. Inputs, fuel, and buffered outputs are
/// now distinct native inventories, and the stack on the cursor is player state: all four survive
/// a save and participate in its checksum. Version 15 has none of those fields; its original
/// `inventory` and `cargo` remain valid legacy stock and drain through the new rules without being
/// reinterpreted at the migration boundary.
///
/// Bumped to 17 for player-capability research. The new technology rows can widen the pack and the
/// build radius, so a version-16 envelope using technology catalog 7 is advanced explicitly to
/// catalog 8. Its researched set contains neither new id, which makes both earned bonuses zero and
/// preserves the old player's exact state and checksum.
/// Version 18 adds primitive definitions without changing existing jobs, stock, bills, or checksum.
///
/// Bumped to 19 for transport kits. Definition revision 17 adds the kit item and its batch recipe
/// and moves the belt family off raw ore, which is a **price** change rather than a state change:
/// no saved field appears, moves, or is reinterpreted, and the checksum of a loaded factory is
/// exactly what it was. The envelope still has to move, because `from_save` refuses any file whose
/// `definition_version` is not the running one, and a definition-16 file describes belts bought at
/// a price this build no longer quotes. What a legacy belt refunds is therefore one kit rather than
/// one ore â€” exactly what rebuilding it now costs, which conserves the line rather than paying a
/// premium on it. Kits have no recipe back to ore, so the boundary cannot mint raw material.
/// Version 21 adds progression registries (technology 9); saved state and checksum are unchanged.
/// Version 26 reprices the two tier bills that still asked for raw ore and the hydro generator that
/// shared the boiler's bill; like every price boundary before it, state and checksum are untouched
/// and a placed station refunds what rebuilding it now costs.
///
/// Bumped to 27 for Practical Projects. The hub's demand is finite: a request pays once and retires,
/// so `repeat_insight` leaves the catalogue and a row's progress belongs to the *row* rather than to
/// the board slot it happens to occupy. `request_delivered` is therefore saved and checksummed, and
/// a version-26 envelope carries that progress inside its posted slots instead. Reading one without
/// the migration below would forfeit whatever the player had already handed over against a row that
/// is no longer re-earnable, which is precisely the loss finite demand makes permanent.
/// Version 28 separates personal skills. The old checksum is verified before legacy research
/// bonuses become granted ranks; skill points and persistent sandbox provenance are native state.
/// Version 30 adds prepared ground: the sparse surface and grade overlay and the spoil ledger. A
/// version-29 file simply has neither, so the migration is the version stamp and the definitions it
/// travels with — an untouched world is exactly the world it already was, which is why the checksum
/// contribution stays guarded on emptiness.
/// Version 33 moves boundaries onto the hex vertex lattice: a boundary is a chord of one hex, and
/// the three shared hex edges are the first three chords under the numbers they already had. A
/// version-32 file is therefore migrated by its version stamp alone, and its boundaries load
/// unchanged through a serde alias rather than being rewritten.
///
/// Version 34 adds the surveying skill. It carries no new saved field: what a player has learned is
/// already in `skills`, and how far that opens the world is derived from it by `Core::survey_rings`.
/// A version-33 file therefore moves on its version stamp and the technology envelope alone, and
/// keeps exactly the `generated_chunks` it was saved with — the wider survey applies from the next
/// hex the player reaches.
/// Version 35 adds per-product output ports. The map is empty on every older building, which is
/// exactly the legacy behaviour: all products leave from the building's facing. The checksum
/// contribution is guarded on emptiness, so the 34 -> 35 migration can verify the original run
/// before adding no state at all.
///
/// Version 38 names foundation class, a service/upgrade envelope and overhead clearance on the
/// definition. Occupancy is still derived from the catalogue rather than saved per entity, so a
/// version-37 file is the same factory under the new stamps: the original checksum verifies, then
/// the envelope numbers move.
///
/// Version 40 carries no new saved field at all — it moves because the world under the save did.
/// The stamp advances so the ladder is complete and the file reaches the world-generator check,
/// which is where a player is told to export it. See [`WORLD_GENERATOR_VERSION`].
///
/// Version 41 carries sparse live-erosion deltas and outside-bank stress. Both default to nothing,
/// so a version-40 factory verifies before adopting the new catalogue resistance data.
const SAVE_VERSION: u16 = 41;
/// Bumped to 6 for World Parameters. `WorldParams` is now part of a run's identity — it is in the
/// save envelope and in the checksum — so a version-5 envelope carries no answer to the question
/// "which world is this" and is rejected rather than assumed to be the default.
///
/// Bumped to 7 for Landforms and Fields. A deposit is a **site** now rather than a per-hex
/// decision, rivers cut inland water, and the guaranteed opening is placed by the generator instead
/// of by a hardcoded list of eight cells inside the clearing. Every one of those changes what a
/// seed generates, so a version-6 envelope describes a landscape this build cannot reproduce and is
/// rejected rather than reinterpreted. The named-save catalog shows the row rather than hiding it.
///
/// Bumped to 12 for Ground You Can See. Two rules changed what a seed lays down. The substrate rule
/// used to select Soil on any bed above 150 m, which the continental field clears almost
/// everywhere: `npm run survey` measured 889 per mille of the world in a single band, with no
/// Lowland and no Highland at all. It now reads the gradient a cell sits on, measured across three
/// cells so the fine relief grain averages out, and elevation only names genuinely high ground. And
/// a river class cuts two to ten metres where it cut half a metre to five, so a channel is a thing
/// you bridge or ford rather than a stripe of blue laid on a plain. Both are the bed itself, so a
/// version-11 envelope names a landscape this build cannot reproduce and is rejected rather than
/// reinterpreted — export the file to keep a copy.
const WORLD_GENERATOR_VERSION: u16 = 12;
const MAX_COMMANDS_PER_BATCH: usize = 8;
/// A drag is one bounded command, so the run it expands into has to be bounded too. This is the
/// native cap on cells a single `place_line` or `erase_line` may touch.
const MAX_LINE_CELLS: usize = 32;
/// How many constructions back one session can be taken. Derived state, so it costs nothing saved.
const MAX_UNDO_DEPTH: usize = 64;
/// The widest a pack may ever be. Creative mode lets a player raise their own carrying capacity, so
/// the ceiling lives here rather than in the host: a slot count arrives as a command like anything
/// else, and an unbounded one would be a host-chosen number in the checksum and a host-chosen number
/// of cells for the host to draw.
const MAX_CARRY_SLOTS: u32 = 240;
/// Rings of chunks generated around a hex the player reaches, before any surveying skill.
///
/// One ring — the chunk plus its six neighbours — is what the game has always opened, so the value
/// is here to be widened rather than to change anything on its own.
const BASE_SURVEY_RINGS: u32 = 1;
/// The most a skill catalogue may add to it. A survey of `n` rings generates `3n(n+1)+1` chunks, so
/// this is a cost ceiling on world generation, not a taste: at the shipped chunk size, two rings is
/// 1,216 hexes against one ring's 448, and three would be 2,432.
const MAX_SURVEY_RING_BONUS: u32 = 2;
const GRAPH_TRACE_LIMIT: i32 = 8;
/// The most outgoing transport links one entity may compile: its facing, and — for a splitter —
/// the two flanks 60° either side of it.
///
/// A fixed width rather than a vector per entity. The graph is compiled for every building in the
/// world and re-compiled on every edit, so `Links` staying a `Copy` value the graph holds inline
/// is what keeps a splitter from costing an allocation on entities that will never have one.
/// One compiled edge per recipe output. A splitter still uses only three wildcard edges; a joint
/// recipe can name up to eight outputs and route each independently.
const MAX_LINKS: usize = 8;
/// The furthest an underpass may reach for its partner, counted in hexes along its own heading.
///
/// This is the crossing budget and nothing else: an entrance rays *past* whatever stands between,
/// so the span is what stops it from being a free belt over any distance. Four covers a doubled
/// main line and the pair of hexes a corner heading straddles, and stays well inside
/// `GRAPH_TRACE_LIMIT`.
const MAX_UNDERPASS_SPAN: u32 = 4;
/// The six hex edges: east, then clockwise. This is the *adjacency* table. Power reach, boiler and
/// turbine neighbours, and every "what is next to this hex" question use it and only it.
const DIRECTIONS: [(i32, i32); 6] = [(1, 0), (0, 1), (-1, 1), (-1, 0), (0, -1), (1, -1)];
/// The twelve *routing* directions: the six unit edge steps, unchanged and at their original
/// indices, then all six vertex headings in clockwise rotational order.
///
/// North and south are lattice vectors on this grid and always were. Pointy-top world-x is
/// proportional to `q + r/2`, so `(q + 1, r - 2)` sits at exactly the same world-x as `(q, r)`,
/// two rows up. They are simply not *unit* vectors, which is the only reason they were never in
/// the table. `compile_graph_target` is a ray-cast that never assumed a unit step, so it needs no
/// change to route through them.
///
/// This is deliberately a second table rather than a longer `DIRECTIONS`. Conflating them would
/// silently let a boiler reach a turbine two rows away, and a pole span a distance the player
/// cannot see. Only transport gets twelve.
///
/// The two straddled hexes `(q, r - 1)` and `(q + 1, r - 1)` are never occupied by a riser: it is
/// a single-cell building whose belt spans the seam where those two hexes meet, so both stay free,
/// buildable, and walkable.
const TRANSPORT_DIRECTIONS: [(i32, i32); 12] = [
    (1, 0),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (0, -1),
    (1, -1),
    (1, -2),
    (2, -1),
    (1, 1),
    (-1, 2),
    (-2, 1),
    (-1, -1),
];
/// Orientation index of due north in `TRANSPORT_DIRECTIONS`, and the first index off the six-edge
/// table. An orientation below this is an edge heading; at or above it, a corner one.
const NORTH: u8 = 6;
const HEX_X: i32 = 1774;
const HEX_Y: i32 = 1536;
/// Center-to-vertex of a pointy-top hex. `HEX_Y * 2 / 3` and `HEX_X / √3` both land on 1024.
///
/// Neighbour centres are `HEX_X` world units apart. Phase 8 reads that spacing as 5.373 m
/// (25 m²); the lattice numbers do not change. [`PLAYER_SPEED`] is unchanged across the rescale,
/// so the player still crosses a hex in the time they always did and every stated speed is five
/// times what it was.
const HEX_RADIUS: i32 = 1024;
/// How many hex steps a *hand* gather reaches. Also the reach of any extractor whose definition
/// names no `extract_radius` of its own, so the base extractor is unchanged by tiers existing.
const EXTRACT_RADIUS: i32 = 1;
/// The largest reach a definition may claim. Reach is the flagship upgrade, so it is data — but
/// `deposit_candidates` walks the whole disc, and a definition file is not allowed to make that
/// walk unbounded.
const MAX_EXTRACT_RADIUS: u32 = 4;
/// The most cells a definition's footprint may claim: the complete two-ring hexagon.
///
/// Nineteen is the largest structure Phase 8's physical catalogue asks for — a refinery, the
/// landing hub, a structural process plant — and it is a shape rather than a round number, so a
/// definition that reaches the ceiling is still something a player can read as one building. The
/// bound exists because every cell is a key in the occupancy index, a preview cell, and a
/// footprint entry on the wire; a definition file is not allowed to make any of those unbounded.
///
/// Nothing shipped is near it. The catalogue's largest building is the three-cell hub, and this
/// ceiling moves before the catalogue does precisely so raising it is not part of the change that
/// reauthors thirty buildings.
const MAX_FOOTPRINT_CELLS: usize = 19;
/// Service/upgrade envelope and overhead clearance each share the occupied-footprint ceiling.
/// They are reservations rather than occupancy, but they are still keys in a derived index and
/// preview cells, so a definition file may not make them unbounded either.
const MAX_ENVELOPE_CELLS: usize = MAX_FOOTPRINT_CELLS;
const MAX_CLEARANCE_CELLS: usize = MAX_FOOTPRINT_CELLS;
/// Hexes around the hub forced to lowland so the landing is always a buildable clearing.
const LANDING_CLEAR_RADIUS: i32 = 7;
/// World units the player covers per player step at full intent (1000). That is the **run**:
/// 25 m/s over a 5.373 m hex. The host sends 600 for the ordinary walk (15 m/s) and 1000 while
/// Shift is held. Paced by `PLAYER_TICKS_PER_SECOND`, not by the simulation tick, so both gaits
/// keep one speed at every simulation speed. Shallow water ignores the gait and is 5 m/s —
/// `PLAYER_SPEED / 5`.
///
/// The number did not move when the ground did. A hex became 25 m² rather than 1 m², and holding
/// the speed in metres would have made every journey in the game five times longer in the hand —
/// a biome crossing measured in quarter-hours, a river detour that costs a minute. Distance is
/// what the rescale was for; travel time is not. So the player covers hexes at the rate they
/// always did, and the honest reading of that is a vehicle, not a walk: this is 15 m/s on foot
/// and 25 m/s at a run. Belts were given real transit instead of a relabelling because a belt is
/// a machine the factory's throughput is measured against; the player is not, and nothing in the
/// simulation reads a speed in metres per second.
const PLAYER_SPEED: i32 = 275;
/// The player's own cadence, in steps per real second. Walking used to run inside the simulation
/// tick, which made it stop when the factory paused and crawl at a low speed multiplier. It is
/// still integer, still native, and still deterministic — a given step count always produces the
/// same position — it is simply no longer measured in factory time.
const PLAYER_TICKS_PER_SECOND: u32 = 30;
const PLAYER_RADIUS: i32 = 580;
const BUILDING_RADIUS: i32 = 690;
/// How far a click may send the player, in hexes. A bound on a command rather than a play rule, for
/// the same reason `MAX_AIM_DISTANCE` is one: the search below is the only unbounded-looking thing
/// the player can trigger by clicking, and an order to walk to a hex a thousand chunks away would
/// generate terrain for the whole corridor between here and there. Ninety-six hexes is several
/// screens at every zoom the camera offers, which is as far as a player can mean by pointing.
const MAX_WALK_DISTANCE: i32 = 96;
/// How many hexes one route search may settle before it gives up and reports no route.
///
/// A* over open ground settles roughly the disc it crosses, so a 96-hex walk in the clear costs a
/// few hundred nodes; the budget only binds when the goal is walled off, which is precisely the case
/// that would otherwise flood-fill a continent looking for a way round. Refusing at the budget is
/// the same answer as refusing for want of a route, and it arrives in bounded time.
const MAX_WALK_SEARCH_NODES: usize = 12_000;
/// The longest route that may be published and followed, in hexes. The straight-line bound above is
/// 96; this allows a route to more than quintuple that going round things before it is treated as
/// no route, and keeps the path a bounded thing to cross the wire.
const MAX_WALK_PATH_CELLS: usize = 512;
/// What crossing one hex of untreated dry ground costs the route.
///
/// A hundred rather than one so that a prepared surface can make a hex genuinely cheaper in integer
/// arithmetic. The route search prices travel *time*, and a road that is 25% faster has to be able
/// to say so in whole numbers: at a base of 1 every surface would round to the same 1 and the search
/// would never prefer the longer paved way round, which is the entire point of paving it.
const WALK_STEP_COST: u32 = 100;
/// What crossing one hex of shallow water costs the route, against [`WALK_STEP_COST`] for dry ground.
///
/// `player_step` fords shallows at `PLAYER_SPEED / 5`, so this is not a preference the search
/// invents — it is the fifth the walk actually takes, which makes the route the *fastest* way to
/// the goal rather than the shortest. A river is a real obstacle to a route because it is a real
/// obstacle to the player, and a bridge is worth building because the search will use it.
const WALK_SHALLOW_COST: u32 = 5 * WALK_STEP_COST;
/// Walking speed on untreated ground, in percent — the number every surface is measured against.
const UNTREATED_MOVEMENT: u32 = 100;
/// The fastest a surface may ever declare itself, in percent.
///
/// Asphalt's 150 is the ceiling the materials plan sets, and this is what keeps the route search's
/// heuristic admissible: no step may cost less than [`WALK_STEP_COST`] scaled by this, so a bound
/// checked once at load time is what lets A* trust its own estimate on every hex thereafter.
const MAX_SURFACE_MOVEMENT: u32 = 150;
/// The cheapest one hex can ever be, and therefore the per-hex weight of the route heuristic.
const MIN_WALK_STEP_COST: u32 = WALK_STEP_COST * UNTREATED_MOVEMENT / MAX_SURFACE_MOVEMENT;
/// What climbing one step of grade adds to a hex, against [`WALK_STEP_COST`] for the hex itself.
///
/// Only climbing: going down a step you could have climbed costs nothing extra, which is true of
/// walking and keeps a route from preferring to stay level across ground it has already paid to
/// descend. A step up costs about as much as another hex of ground, so a ramp is worth grading and
/// a route will happily go round a mound rather than over it.
const WALK_CLIMB_COST: u32 = WALK_STEP_COST;
/// The most a hex may be cut or filled away from the band the generator gave it.
///
/// Three steps each way is a two-storey retaining wall, which is as much terraforming as this slice
/// promises. It also keeps `elevation` an `i8` whose every legal value is checked on load.
const MAX_GRADE_STEPS: i8 = 3;
/// The tallest step between neighbours that can still be walked. A taller one is a retaining wall.
///
/// Two, because that is exactly the widest gap `natural_elevation` puts between any two terrains a
/// player can walk on. The generated world is therefore as passable after this release as before it,
/// and every wall in a run is one somebody dug.
const MAX_WALK_STEP: i32 = 2;
/// The tallest spread a single building's footprint may span.
///
/// Deliberately the same number as [`MAX_WALK_STEP`], so there is one rule about steep ground rather
/// than two: if a player can walk between two hexes, a building can span them, and a face nobody can
/// climb is a face nobody can build across. Making it *stricter* than walking would have made ground
/// the generator already produced unbuildable, breaking bases that were legal a release ago; a level
/// pad earns its keep the moment somebody terraces, where a full cut beside untouched ground is a
/// three-step face and refuses.
const MAX_BUILD_STEP: i32 = MAX_WALK_STEP;
/// How many hexes one ground edit may cover. A preview is priced before it is drawn, so a stray
/// drag must be refused rather than costed.
///
/// Sixty-four rather than the thirty-two boundary edits use, because a ground selection and a wall
/// selection are not the same size of thing: a walled yard is a rectangle's *edge*, and the floor
/// inside it is the rectangle's area. A 6×6 yard is thirty-six hexes, so the old bound refused to
/// floor a yard it would happily wall. Sixty-four also gives the circular selections room to be
/// worth having — a radius-4 disc is sixty-one hexes and a radius-10 ring is sixty.
const MAX_GROUND_CELLS: u64 = 64;
/// The gait an autonomous walk travels at, as a movement intent.
///
/// Full intent — the run — rather than the 0.6 the unmodified movement keys ask for. A player
/// clicks a distant hex precisely because they do not want to hold a key across it, and there is no
/// modifier to hold on a click that has already happened. Being native's own number rather than one
/// the host sends also means the host has no say in how fast a checksummed walk crosses the world:
/// the click names *where*, and the simulation decides *how*.
const AUTO_WALK_INTENT: i16 = 1000;
/// Player-clock steps a walk may make no ground before it is abandoned. One second: long enough to
/// ride out a step spent sliding along a wall, short enough that a player boxed in by a building
/// placed across the route gets their controls back rather than jogging into it forever.
const WALK_STALL_STEPS: u32 = 30;
/// How close to a waypoint's centre counts as standing on it, in world units.
///
/// The hex's inradius, which is the largest circle wholly inside it. Being inside it means
/// `world_to_axial` of that position is that hex, which is what lets arrival be told apart from a
/// route that ran out for any other reason. It also has to be comfortably larger than one step — at
/// most `PLAYER_SPEED`, 275 — or a waypoint could be jumped clean over and the walk would circle it.
const WALK_ARRIVE_RADIUS: i32 = HEX_Y / 2;
/// Fastest hand gather, in player-clock steps: wood. Counted on the player's own cadence like the
/// walk, so holding the action key harvests at one rate whether the factory is paused, running at
/// 4 tps, or running at 60.
///
/// **Fifteen steps is one extractor**, and it is now the *ceiling* rather than the only rate.
/// The value was six — 0.2s — which is 300 items a minute against an extractor's 120, so the first
/// machine a player built was two and a half times slower than the hands it was supposed to
/// replace. At fifteen the hand is never faster than an extractor working the same cells, and on
/// hard rock it is materially slower. The per-item figure lives on `ItemDefinition::hand_gather_steps`;
/// this constant is wood, the bootstrap fuel, and the rate the cooldown helper falls back to.
const GATHER_COOLDOWN_STEPS: u32 = 15;
/// How many hex steps from any occupied cell of the landing hub a hand delivery is allowed.
///
/// Two, and from the whole footprint: a three-cell hub is not a one-cell target with two
/// decorative lobes. The previous figure was a 1900-unit circle around the *anchor*, which is
/// barely one hex centre-to-centre — standing beside a far lobe, or even at the outer edge of an
/// origin-adjacent hex, was "out of reach" of a building the player was next to.
const HUB_REACH_HEXES: i32 = 2;
/// How many requests the landing hub posts at once.
///
/// Three, because a board of one is an errand and a board of ten is a spreadsheet. Three is enough
/// that a material the player cannot find yet never blocks the whole economy, and few enough that
/// every line of it fits in the panel beside the contract it is funding.
const REQUEST_SLOTS: usize = 3;
/// How deep the reachability walk over the recipe tree may go before it gives up. The shipped tree
/// is four deep; this is a guard against a catalogue that cycles, not a limit on content.
const MAX_RECIPE_DEPTH: u32 = 8;
/// How far from the player an `aim` target may sit, in world units — about 600,000 hexes, which is
/// far past anything a viewport can be showing. It is a bound on a command, not a play rule: the
/// squared distance an aim resolves through has to stay inside an `i64`, and a forged aim is
/// refused for the same reason a forged movement intent is.
const MAX_AIM_DISTANCE: i64 = 1 << 30;
/// Default reach for a water-sited definition that predates data-defined source reach.
const PUMP_RADIUS: i32 = 1;
/// How far a pole supplies machines around it, and how far it links to the next pole, for a pole
/// definition that names neither. Coverage is a property of the *pole*: before v0.19 the distance a
/// machine could stand from a pole was read off the machine, which made "how far does this pole
/// reach" a question no pole could answer and no upgrade could change.
const DEFAULT_POLE_SUPPLY_RADIUS: i32 = 3;
const DEFAULT_POLE_REACH: i32 = 6;
/// The largest coverage a pole definition may claim, for the same reason `MAX_EXTRACT_RADIUS`
/// exists: the pole-to-machine pass walks a disc, and a definition file is not allowed to make that
/// walk unbounded.
const MAX_POLE_SUPPLY_RADIUS: u32 = 8;
/// How many crafts' worth of electricity a machine banks before it stops asking for more.
///
/// This is the whole of the "1 unit, 3 cycles" rule, and it is what makes a grid sized by *average*
/// load rather than peak. An extractor on a five-tick cadence, or a smelter waiting on ore, stops
/// reserving capacity it is not using, so the same generator carries a much larger and lumpier
/// factory than a per-tick tax ever could.
const POWER_BUFFER_CYCLES: u32 = 3;
/// Water as a belted item. Boilers drink it; a fluid network is not this milestone.
const WATER_ITEM: ItemId = 10;

fn default_footprint() -> Vec<Coordinate> {
    vec![Coordinate { q: 0, r: 0 }]
}

#[derive(Clone, Deserialize)]
struct DefinitionsInput {
    #[serde(default)]
    boundaries: Vec<BoundaryDefinition>,
    #[serde(default)]
    surfaces: Vec<SurfaceDefinition>,
    version: u16,
    items: Vec<ItemDefinition>,
    recipes: Vec<RecipeDefinition>,
    buildings: Vec<BuildingDefinition>,
    /// What the landing hub is willing to pay insight for, and how much. See
    /// [`Core::refill_requests`] for how a row becomes a posted request.
    requests: Vec<RequestDefinition>,
}

/// One standing order the landing hub can post: a named quantity of one item, for a stated
/// number of insight.
///
/// Insight used to be a property of the item — every delivery paid `insight_value × quantity`,
/// whatever the hub had any use for — which made the eight raw materials differ less than their
/// geography claims and left the player with no way to find out what anything was worth except by
/// handing it over. A request states the price *before* the delivery, and it is the only thing in
/// the game that pays insight at all.
///
/// It also pays **once**. A row used to repost after it was filled, at a decayed price for the raw
/// surveys and at full price for every processed row, which made insight an unbounded income: the
/// answer to "can I afford the deepest branch" was always yes, given enough repetitions of the one
/// delivery the player had already automated. A project is now a finite piece of practical work —
/// a stated bill, a stated price, completed exactly once — so the catalogue is a budget rather than
/// a tap. What bounds research is what the hub still has left to learn.
#[derive(Clone, Deserialize)]
struct RequestDefinition {
    id: RequestId,
    key: String,
    name: String,
    /// One sentence saying why the hub wants it. Shown on the board, so it is content rather than
    /// a comment.
    brief: String,
    item_id: ItemId,
    quantity: u32,
    /// What completing this project pays, once. Priced against the raw gathers underneath the item —
    /// see the `requests` section of `fixtures/balance.json`, which reports exactly that ratio.
    insight: u32,
}

#[derive(Clone, Deserialize)]
struct ItemDefinition {
    id: ItemId,
    key: String,
    name: String,
    color: String,
    icon: String,
    description: String,
    /// How many of this item occupy one carried slot. Carrying capacity is a rule over the
    /// player's ordinary `item_id → quantity` map rather than a stored array of slots, so the save
    /// format, the checksum inputs, and every ordering guarantee are unchanged by it.
    stack_size: u32,
    /// Loose bulk liquid. It may occupy native machine stock and pipe cargo, but it does not enter
    /// a player's pack or a newly built solid belt. Filled barrels are ordinary non-fluid items.
    #[serde(default)]
    fluid: bool,
    /// Energy one unit releases when burned. Fuel is a property of the item, never an entry in a
    /// recipe's `inputs`: naming a fuel in a recipe would force one recipe per fuel and hardcode
    /// the bootstrap path, where this way coal and charcoal are the same recipe at different
    /// values and every fuel added later is too.
    #[serde(default)]
    fuel_value: Option<u32>,
    /// Ticks between one unit of regrowth and the next, for a resource that is flora rather than
    /// ore. A harvested cell climbs back toward the quantity generation gave it and stops there,
    /// which is what makes wood renewable while every ore field is finite.
    #[serde(default)]
    regrowth_ticks: Option<u32>,
    /// Root and cover resistance when this item is a living field resource. Ordinary cargo leaves
    /// it at zero; geomorphology reads it only while a non-empty field stands on the bank.
    #[serde(default)]
    erosion_resistance: u16,
    /// Player-clock steps between hand gathers of this item. Absent means the hand cannot take it
    /// at all: water is pumped, signal crystal is extracted. Fifteen is wood, and no material is
    /// faster — that is the restated invariant `fixtures/balance.json` pins.
    #[serde(default)]
    hand_gather_steps: Option<u32>,
    /// Simulation ticks a tier-one extractor spends on one unit of this material, before its own
    /// `extract_speed` scales it. Extraction rate is a property of what is being dug, for the same
    /// reason `hand_gather_steps` is: coal and sand are not the same work, and a single building
    /// cadence said they were.
    ///
    /// The figures are set against the hand at the default ten ticks per second, where a tier-one
    /// extractor takes twice as long as a hand on the same material. That inverts the rule v0.23
    /// shipped — the hand used to be the thing that could never outrun a machine. A slower machine
    /// that works unattended is still the better deal, and it makes automation a question of how
    /// many you can afford to run rather than of raw speed.
    ///
    /// Absent means an extractor cannot resolve a rate for it and falls back to the building's own
    /// `cadence`, which is what a pump does: water is the one source with no per-material figure
    /// because the pump is the only thing that draws it.
    #[serde(default)]
    extract_steps: Option<u32>,
    #[serde(default)]
    production_routes: Option<Vec<RecipeId>>,
    #[serde(default)]
    extraction_building_id: Option<DefinitionId>,
}

#[derive(Clone, Deserialize)]
struct RecipeDefinition {
    id: RecipeId,
    key: String,
    name: String,
    description: String,
    /// Which machines may run this. A kiln and a smelter are the same `BuildingKind` with
    /// different recipe categories — one field and one check at recipe assignment, rather than a
    /// new kind and a new tick path for every machine the material tree adds.
    category: String,
    inputs: Vec<Ingredient>,
    output: Ingredient,
    #[serde(default)]
    co_products: Vec<Ingredient>,
    #[serde(default)]
    cost_allocation: Vec<u32>,
    duration: u32,
    /// Energy one craft consumes, paid from whatever fuel item the machine has been fed. Zero for
    /// every recipe that needs no heat, which is what keeps charcoal reachable without coal.
    #[serde(default)]
    fuel: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct Ingredient {
    item_id: ItemId,
    quantity: u32,
}

#[derive(Clone, Deserialize)]
struct BuildingDefinition {
    id: DefinitionId,
    key: String,
    name: String,
    kind: BuildingKind,
    description: String,
    icon: String,
    #[serde(default)]
    cadence: Option<u32>,
    #[serde(default)]
    capacity: Option<u32>,
    /// The recipe category this machine can be assigned, for a composer-kind building. A kiln
    /// cannot be given a circuit recipe because its category does not match, not because a new
    /// `BuildingKind` exists for it.
    #[serde(default)]
    recipe_category: Option<String>,
    /// An explicit capability list replaces the category match for primitive equipment. It
    /// reuses recipe identities, so teaching a workshop timber does not create a second recipe.
    #[serde(default)]
    recipe_ids: Option<Vec<RecipeId>>,
    /// Manual stations run one batch only while the player is attending them. The existing
    /// disabled flag is their saved work permit; placement starts with that permit off.
    #[serde(default)]
    manual_work: bool,
    #[serde(default)]
    duration_multiplier: Option<u32>,
    /// What a pump produces. Data-defined for the same reason recipes are: a source building's
    /// output is content, not a branch in the tick.
    #[serde(default)]
    output_item_id: Option<ItemId>,
    /// Electricity this machine spends per tick of work. Zero or absent: no draw.
    ///
    /// A *rate against progress*, not against the clock. A machine that is blocked, starved, or
    /// out of recipe spends nothing, which is the difference between this and a per-tick tax: one
    /// craft costs `power_draw × duration` however long the machine stood idle first.
    #[serde(default)]
    power_draw: Option<u32>,
    /// Electricity offered every tick this generator is live, and the rate at which its fuel is
    /// worth that electricity: a generator running flat out spends exactly one unit of fuel energy
    /// per tick, so `power_output` is also the grid energy one fuel unit buys.
    #[serde(default)]
    power_output: Option<u32>,
    /// How far this pole supplies the machines around it.
    #[serde(default)]
    supply_radius: Option<u32>,
    /// How far this pole links to the next pole. Longer than `supply_radius` because spanning
    /// distance is what a line of poles is for.
    #[serde(default)]
    pole_reach: Option<u32>,
    #[serde(default)]
    power_source: Option<PowerSource>,
    /// Which orientations this building may take. Absent means the six hex edges, which is what
    /// every building built before v0.14 takes.
    #[serde(default)]
    orientation_axis: OrientationAxis,
    /// What one of the six corner headings costs, when that differs from `construction_cost`.
    ///
    /// The price of the two-row period, and the whole reason a belt and a riser can be one
    /// definition. A corner step covers `3 · size` against `√3 · size`, so charging it the edge
    /// price would make it strictly dominant; charging it here keeps the old riser's economics
    /// exactly while retiring the second building. Absent means the heading costs what every other
    /// heading on this definition costs, which is true of everything that is not transport.
    #[serde(default)]
    corner_construction_cost: Option<Vec<Ingredient>>,
    /// The technology this definition's corner headings wait behind, separately from the
    /// technology that unlocks the definition itself.
    ///
    /// A capability, not a building. The belt is the first thing the player ever builds and the
    /// two-row reach is a mid-game unlock, so the two cannot be the same gate — and inventing a
    /// second belt definition to carry the second gate is exactly the split this replaces.
    #[serde(default)]
    corner_technology_id: Option<TechnologyId>,
    /// Whether this transport building also rays its two flanks, and round-robins its cargo
    /// between every output that will take it.
    ///
    /// One flag rather than a `BuildingKind`, on the same terms a kiln is a composer: a splitter's
    /// *source* is not different, only the number of edges it compiles. The tick is unchanged —
    /// `transfer_cargo` still walks compiled edges — so this adds outputs to the graph and no path
    /// to the loop.
    #[serde(default)]
    splits: bool,
    /// Whether this transport building accepts from its feeders in rotation rather than in entity
    /// id order, so no lane that shares a junction can starve another.
    #[serde(default)]
    merges: bool,
    /// How many hexes this building's output ray may pass *over* before it binds.
    ///
    /// An underpass, and the only thing in the game whose ray does not stop at the first occupied
    /// cell it meets. Absent — every other building — means the ray binds to whatever it first
    /// reaches, which is the rule the transport graph has always had. Bounded by
    /// `MAX_UNDERPASS_SPAN` at load, because an unbounded span is a belt that costs nothing per
    /// hex.
    #[serde(default)]
    underpass_span: Option<u32>,
    /// Which cargo family a belt-kind transport carries. Existing definitions default to solid;
    /// pipes reuse the compiled graph and arbitration with a fluid-only acceptance boundary.
    #[serde(default)]
    transport_medium: TransportMedium,
    /// Optional exact filter for a container. Tanks name one loose fluid; an ordinary shelf omits
    /// the field and remains general storage.
    #[serde(default)]
    accepted_item_ids: Option<Vec<ItemId>>,
    /// Where this definition sits on its own upgrade ladder. Presentation reads it for trim; the
    /// simulation only ever compares it, and never branches on it.
    #[serde(default)]
    tier: u8,
    /// The definition `upgrade` turns this one into. A ladder is a chain of these, so a tier is a
    /// data row rather than a kind, a tick path, or a drawing.
    #[serde(default)]
    upgrades_to: Option<DefinitionId>,
    /// How many hex steps this extractor reaches, counting its own cell. Absent means
    /// `EXTRACT_RADIUS`. This is what makes reach the flagship upgrade: a longer arm is one number
    /// in this file, visible on the map, changing a decision the player already made.
    #[serde(default)]
    extract_radius: Option<u32>,
    /// How fast this extractor works its material, as a percentage of the item's `extract_steps`.
    /// Absent or 100 is the tier-one baseline: twice as long as the hand. 200 halves the cycle and
    /// puts the machine level with the hand; anything above that beats it.
    ///
    /// A percentage rather than a per-tier cadence because the ladder is the point — the same
    /// eight material figures are shared by every tier, so a new extractor is one number here and
    /// never a second table that can drift out of step with the first.
    #[serde(default)]
    extract_speed: Option<u32>,
    construction_cost: Vec<Ingredient>,
    #[serde(default)]
    unlock_technology_id: Option<TechnologyId>,
    placement_rule: PlacementRule,
    buildable: bool,
    blocks_movement: bool,
    #[serde(default = "default_footprint")]
    footprint: Vec<Coordinate>,
    /// How this building sits on uneven ground. Absent means a level pad: the occupied foundation
    /// may not span more than [`MAX_BUILD_STEP`] (legacy) or [`scale::MAX_BUILD_STEP_QUANTA`]
    /// (physical). `span` may follow a slope a player can still walk; `retaining` is the exception
    /// for walls, stairs and prepared foundations that create the grade they sit on.
    #[serde(default)]
    foundation_class: FoundationClass,
    /// Cells reserved at placement that are not solid occupancy. Neighbours cannot occupy them;
    /// a later upgrade may grow onto them without a second occupancy check. The player may still
    /// walk through. Empty means the atomic growth path: prove the extra cells at upgrade time.
    #[serde(default)]
    service_envelope: Vec<Coordinate>,
    /// Cells this building reserves in the air without occupying the ground. A turbine rotor is
    /// the type case: belts, poles and bridges may pass underneath, machines may not. Empty means
    /// the occupied footprint is the whole of what this building claims.
    #[serde(default)]
    overhead_clearance: Vec<Coordinate>,
}

/// How a building's occupied foundation may sit on finished grade.
///
/// Walking and construction no longer share one threshold. Ordinary machines need a pad; a belt
/// or a stair can follow a walkable slope; a retaining wall is the thing that *makes* the face.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum FoundationClass {
    #[default]
    Pad,
    Span,
    Retaining,
}

impl BuildingDefinition {
    fn supports_recipe(&self, recipe: &RecipeDefinition) -> bool {
        self.kind == BuildingKind::Composer
            && self.recipe_ids.as_ref().map_or_else(
                || self.recipe_category.as_deref() == Some(recipe.category.as_str()),
                |ids| ids.contains(&recipe.id),
            )
    }

    fn recipe_duration(&self, recipe: &RecipeDefinition) -> u32 {
        recipe.duration * self.duration_multiplier.unwrap_or(1)
    }

    /// What one of this building costs when built at that heading.
    ///
    /// The single place the two-row price lives. Every charge, refund, preview budget, and upgrade
    /// netting goes through here, so a corner belt is priced the same whichever of those five paths
    /// reaches it — the way the riser's own `construction_cost` row used to guarantee by existing.
    fn cost_at(&self, orientation: u8) -> &[Ingredient] {
        match &self.corner_construction_cost {
            Some(cost) if is_corner_heading(orientation) => cost,
            _ => &self.construction_cost,
        }
    }

    /// The technology this building waits behind at that heading: its own gate, and — on a corner —
    /// the separate gate the two-row reach waits behind.
    fn gates_at(&self, orientation: u8) -> [Option<TechnologyId>; 2] {
        let corner = if is_corner_heading(orientation) {
            self.corner_technology_id
        } else {
            None
        };
        [self.unlock_technology_id, corner]
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum BuildingKind {
    Extractor,
    Belt,
    Composer,
    Container,
    Consumer,
    Hub,
    /// Draws from water terrain rather than from a field cell, and never depletes it. That is why
    /// it is a kind of its own and the smelter, kiln, cutter, and crusher are not: they are all a
    /// composer running a recipe, and a pump is a different source.
    Pump,
    Pole,
    Generator,
    Boiler,
    /// A support deck on shallow water. Terrain stays water; this entity is what permits a
    /// transport building to occupy an otherwise unbuildable ford.
    Bridge,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum TransportMedium {
    #[default]
    Solid,
    Fluid,
}

/// Whether a building of this kind could ever take a delivered item — whatever it holds, whatever
/// recipe it is given, whatever the hub is asking for today.
///
/// This is the *static* question, over the kind alone, and it is deliberately a separate predicate
/// from `accepts_item`. That one answers *would you want this one item, right now*, which changes
/// with a recipe, a fuel, or a contract, and construction must not be decided by an answer that can
/// change a tick later. This one never changes, so a graph edge into such a target is a dead edge
/// worth refusing to compile and worth refusing to build.
fn never_accepts_deliveries(kind: BuildingKind) -> bool {
    matches!(
        kind,
        BuildingKind::Extractor | BuildingKind::Pump | BuildingKind::Pole | BuildingKind::Bridge
    )
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum PowerSource {
    Burner,
    Wind,
    Hydro,
    Turbine,
}

/// Which of the twelve routing headings a definition may be built at.
///
/// `Edge` is the six hex edges and the default, so every definition that predates tiers keeps
/// exactly the orientations it had. `Corner` is the six vertex headings, for anything that spans
/// only the two-row period. `Any` is both, and is what the belt takes.
///
/// The axis is a price as much as a permission. A vertex heading covers `3 · size` of world
/// distance against `√3 · size` for an edge step, so a heading a definition may take for free
/// would be strictly dominant. `Edge` and `Corner` answer that by being separate definitions with
/// separate `construction_cost` rows; `Any` answers it inside one definition, with
/// `corner_construction_cost` and `corner_technology_id`.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum OrientationAxis {
    #[default]
    Edge,
    Corner,
    /// Both families, so rotation walks all twelve headings in clockwise order.
    ///
    /// This is what makes a belt and a riser one building rather than two. The reason the axes
    /// were separated — that a corner heading covers `3 · size` against `√3 · size` and would be
    /// strictly dominant at a belt's price — is answered by `corner_construction_cost` instead:
    /// the heading still costs what it covers, so the choice stays a real one while the player
    /// builds, drags, and rotates a single thing. A definition on this axis must also name the
    /// research its corner headings wait behind, or the two-row reach would arrive with the first
    /// belt of the game.
    Any,
}

impl OrientationAxis {
    /// The half-open range of orientation indices this axis allows.
    fn range(self) -> std::ops::Range<u8> {
        match self {
            Self::Edge => 0..NORTH,
            Self::Corner => NORTH..TRANSPORT_DIRECTIONS.len() as u8,
            Self::Any => 0..TRANSPORT_DIRECTIONS.len() as u8,
        }
    }

    fn allows(self, orientation: u8) -> bool {
        self.range().contains(&orientation)
    }

    /// The next orientation one `rotate` along. Rotation stays inside the axis, so edge and corner
    /// definitions each walk six headings in clockwise order.
    ///
    /// `Any` walks all twelve, and walks them in *angular* order rather than in table order. The
    /// table lists the six edges and then the six corners, so stepping its indices would turn a
    /// belt through every edge before it reached the first corner — six presses of `R` to nudge a
    /// heading by 30°. The two interleavings below are that ordering and nothing more: a corner
    /// heading sits in the 30° gap after edge `e` at `NORTH + (e + 2) % 6`, and the edge after that
    /// corner is `(k + 5) % 6`. `rotation_walks_every_heading_once_in_angular_order` pins both
    /// against the world vectors rather than against these expressions.
    fn next(self, orientation: u8) -> u8 {
        if self == Self::Any {
            return if orientation < NORTH {
                NORTH + (orientation + 2) % 6
            } else {
                (orientation - NORTH + 5) % 6
            };
        }
        let range = self.range();
        let span = range.end - range.start;
        let offset = orientation.wrapping_sub(range.start);
        range.start + (offset.wrapping_add(1) % span)
    }

    fn previous(self, orientation: u8) -> u8 {
        if self == Self::Any {
            return if orientation < NORTH {
                NORTH + (orientation + 1) % 6
            } else {
                (orientation - NORTH + 4) % 6
            };
        }
        let range = self.range();
        let span = range.end - range.start;
        let offset = orientation.wrapping_sub(range.start);
        range.start + (offset.wrapping_add(span - 1) % span)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Link {
    /// `None` is the legacy/default route shared by every offered item. A named item is one
    /// independently configured product outlet.
    item_id: Option<ItemId>,
    target: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LinkId {
    item_id: Option<ItemId>,
    target_id: u32,
}

/// One entity's outgoing edges named by stable entity id rather than by vector index.
///
/// What an incremental recompile carries across an edit: erasing shifts every index after the hole,
/// so the edges that were *not* affected have to survive as ids and be resolved back afterwards.
type LinkIds = [Option<LinkId>; MAX_LINKS];

/// One entity's outgoing transport edges, in the order they were compiled.
///
/// Ordinary transport has exactly one and the whole game had exactly one before splitters existed,
/// which is why `primary` is kept as its own word: everything that asks "where does this belt
/// deliver" — the snapshot's `next_id`, the blocked-output status, the connecting deck the renderer
/// draws — is still asking about the first edge, and reads the same on a building that will never
/// have a second.
///
/// Fixed width and `Copy`. See `MAX_LINKS`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Links {
    edges: [Option<Link>; MAX_LINKS],
}

impl Links {
    /// The one-edge graph every non-splitting building compiles.
    fn single(target: Option<usize>) -> Self {
        let mut links = Self::default();
        if let Some(target) = target {
            links.edges[0] = Some(Link {
                item_id: None,
                target,
            });
        }
        links
    }

    /// Every distinct target this entity delivers to, in compile order.
    ///
    /// Two products may use the same belt. Reverse feeder indexes still need that source once,
    /// not once per product, or merger arbitration would silently weight the source.
    fn iter(self) -> impl Iterator<Item = usize> {
        self.edges
            .into_iter()
            .enumerate()
            .filter_map(move |(index, edge)| {
                let edge = edge?;
                (!self.edges[..index]
                    .iter()
                    .flatten()
                    .any(|previous| previous.target == edge.target))
                .then_some(edge.target)
            })
    }

    /// The edges this item may actually take. Once a product has a named route, the wildcard is
    /// no longer a fallback for it — a disconnected configured port must stay disconnected.
    fn iter_for(self, item_id: ItemId) -> impl Iterator<Item = usize> {
        let named = self
            .edges
            .iter()
            .flatten()
            .any(|edge| edge.item_id == Some(item_id));
        self.edges.into_iter().flatten().filter_map(move |edge| {
            (edge.item_id == Some(item_id) || (!named && edge.item_id.is_none()))
                .then_some(edge.target)
        })
    }

    /// The first outgoing edge, which for everything but a splitter is the only one.
    fn primary(self) -> Option<usize> {
        self.edges[0].map(|edge| edge.target)
    }

    fn is_empty(self) -> bool {
        self.edges[0].is_none()
    }

    /// Add one edge, keeping the slots packed from the front.
    ///
    /// A repeated target is dropped rather than stored twice. A splitter whose flank ray reaches
    /// the same building its facing ray reached has *one* consumer, not two, and storing it twice
    /// would hand that consumer two of every three items — a round robin that silently weights
    /// itself by geometry.
    fn push(&mut self, target: usize) {
        self.push_item(None, target);
    }

    fn push_item(&mut self, item_id: Option<ItemId>, target: usize) {
        if self
            .edges
            .iter()
            .flatten()
            .any(|existing| existing.item_id == item_id && existing.target == target)
        {
            return;
        }
        if let Some(slot) = self.edges.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(Link { item_id, target });
        }
    }
}

/// A product outlet stored relative to the entity anchor, in world orientation.
///
/// The cell is one real footprint tile and the direction is one of its six exterior sides. It is
/// saved and checksummed: two otherwise equal refineries that send fuel in different directions
/// are different factories and must reload that way.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
struct OutputRoute {
    q: i32,
    r: i32,
    direction: u8,
}

/// Whether a routing heading is one of the six vertex headings rather than one of the six edges.
///
/// The one predicate for "does this heading span the two-row period", asked by the cost rule, by
/// the drag router's step weights, and by the flank rule. `NORTH` is the boundary and always was;
/// this names it so the comparison is not spelled out at each call site.
fn is_corner_heading(orientation: u8) -> bool {
    orientation >= NORTH && usize::from(orientation) < TRANSPORT_DIRECTIONS.len()
}

/// The two headings 60° either side of this one, inside its own family.
///
/// A splitter's flanks. Rotation here is *within the six* the heading belongs to — an edge heading
/// flanks to edges and a corner heading to corners — because 60° either side of a heading is the
/// pair of headings that share its period. Taking a flank across families would hand a belt-priced
/// splitter a two-row output, which is the same dominance `corner_construction_cost` exists to
/// price.
fn flanks_of(orientation: u8) -> [u8; 2] {
    let base = if is_corner_heading(orientation) {
        NORTH
    } else {
        0
    };
    let offset = orientation - base;
    [base + (offset + 1) % 6, base + (offset + 5) % 6]
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum PlacementRule {
    Ground,
    Resource,
    /// Buildable ground with open water inside `PUMP_RADIUS`.
    Water,
    /// Hills or highland — the same bands iron, coal, and copper already occupy.
    Elevated,
    /// On a shallow-water hex. Deep water remains a barrier and terrain itself is unchanged.
    Shallows,
}

#[derive(Clone, Deserialize)]
struct TechnologiesInput {
    version: u16,
    branches: Vec<ProgressionGroup>,
    stages: Vec<ProgressionGroup>,
    technologies: Vec<TechnologyDefinition>,
    skills: Vec<SkillDefinition>,
    skill_milestones: Vec<SkillMilestone>,
}

/// Authored presentation metadata. Never a purchase gate or saved simulation state.
#[derive(Clone, Deserialize)]
struct ProgressionGroup {
    key: String,
    name: String,
    description: String,
    order: u32,
}

#[derive(Clone, Deserialize)]
struct TechnologyDefinition {
    id: TechnologyId,
    key: String,
    name: String,
    description: String,
    branch: String,
    stage: String,
    prerequisites: Vec<TechnologyId>,
    cost: u32,
    #[serde(default)]
    effects: Vec<TechnologyEffect>,
    /// Insight purchase unless a contract stage grants this on completion.
    #[serde(default)]
    grant: TechnologyGrant,
}

/// A supported native capability this technology grants when complete.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TechnologyEffect {
    UnlockBuilding { building_id: DefinitionId },
    UnlockBoundary { boundary_id: DefinitionId },
    UnlockSurface { surface_id: DefinitionId },
    CarrySlots { amount: u32 },
    BuildRange { amount: u32 },
}

/// How this technology enters the researched set. Purchases spend insight;
/// contract-stage grants are issued by native on stage completion and cannot be bought.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TechnologyGrant {
    #[default]
    Purchase,
    ContractStage {
        key: String,
        name: String,
    },
}

impl TechnologyDefinition {
    fn purchasable(&self) -> bool {
        matches!(self.grant, TechnologyGrant::Purchase)
    }

    fn building_unlocks(&self) -> impl Iterator<Item = DefinitionId> + '_ {
        self.effects.iter().filter_map(|effect| match effect {
            TechnologyEffect::UnlockBuilding { building_id } => Some(*building_id),
            _ => None,
        })
    }

    fn boundary_unlocks(&self) -> impl Iterator<Item = DefinitionId> + '_ {
        self.effects.iter().filter_map(|effect| match effect {
            TechnologyEffect::UnlockBoundary { boundary_id } => Some(*boundary_id),
            _ => None,
        })
    }

    fn carry_slots_bonus(&self) -> u32 {
        self.effects
            .iter()
            .filter_map(|effect| match effect {
                TechnologyEffect::CarrySlots { amount } => Some(*amount),
                _ => None,
            })
            .fold(0, u32::saturating_add)
    }

    fn build_range_bonus(&self) -> u32 {
        self.effects
            .iter()
            .filter_map(|effect| match effect {
                TechnologyEffect::BuildRange { amount } => Some(*amount),
                _ => None,
            })
            .fold(0, u32::saturating_add)
    }
}

#[derive(Clone, Deserialize)]
struct ScenariosInput {
    version: u16,
    scenarios: Vec<ScenarioDefinition>,
}

#[derive(Clone, Deserialize)]
struct ScenarioDefinition {
    id: u16,
    key: String,
    name: String,
    description: String,
    version: u16,
    seed: u32,
    /// The preset this scenario generates under when the caller names none. A scenario that
    /// generates no environment does not need one.
    #[serde(default)]
    world_preset: Option<String>,
    chunk_size: i32,
    generated_environment: bool,
    player_spawn: Coordinate,
    player_facing: u8,
    build_range: u32,
    /// How many stacks the player can carry at once. Containers exist to solve this.
    carry_slots: u32,
    /// What the landing hub is actually asking for, in order. A scenario states a demand rather
    /// than a single delivery total, because a founding project is the thing that gives an economy
    /// a reason to exist and one item's counter cannot express it.
    contract: ContractDefinition,
    #[serde(default)]
    initial_inventory: Vec<Ingredient>,
    #[serde(default)]
    initial_researched: Vec<TechnologyId>,
    #[serde(default)]
    resources: Vec<ScenarioResource>,
    buildings: Vec<PlacedBuilding>,
}

/// The landing hub's standing demand: an ordered list of stages, each a bill of materials.
///
/// A stage is not a quest generator and not a wall. It is one bounded thing the hub is building,
/// stated as data so it can be delivered against, saved, checksummed, and read on screen without
/// any of the three re-deriving what the other two believe.
#[derive(Clone, Deserialize)]
struct ContractDefinition {
    key: String,
    name: String,
    stages: Vec<ContractStage>,
}

#[derive(Clone, Deserialize)]
struct ContractStage {
    key: String,
    name: String,
    /// One paragraph the host can put in front of the player. Native owns it so the sentence and
    /// the bill can never disagree about which stage is current.
    brief: String,
    /// What completing this stage does to the hub on screen, in words, so the drawing has
    /// something to be checked against the same way `TierStep::reads` does.
    reads: String,
    requirements: Vec<Ingredient>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct Coordinate {
    q: i32,
    r: i32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct ScenarioResource {
    q: i32,
    r: i32,
    item_id: ItemId,
    quantity: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct PlacedBuilding {
    q: i32,
    r: i32,
    definition_id: DefinitionId,
    orientation: u8,
    #[serde(default)]
    recipe_id: Option<RecipeId>,
    #[serde(default)]
    scenario_owned: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct Cargo {
    item_id: ItemId,
    quantity: u32,
}

/// One item crossing a belt's lane, and the tick it stepped onto it.
///
/// The tick is stored rather than a countdown so that nothing has to be decremented every tick: a
/// lane is pure arithmetic against `Core::tick`, which keeps a hundred thousand belts free when
/// nothing about them is changing, keeps a delta snapshot from re-sending every belt every tick,
/// and lets the host extrapolate an item's position between snapshots from a number that does not
/// go stale.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct LaneItem {
    cargo: Cargo,
    entered: u64,
}

/// Ticks an item spends crossing one belt hex, from [`scale::belt_transit_ticks`].
const BELT_TRANSIT_TICKS: u64 = scale::belt_transit_ticks() as u64;
/// Items one belt hex holds while they cross it, from [`scale::belt_lane_slots`].
const BELT_LANE_SLOTS: usize = scale::belt_lane_slots() as usize;
/// The gap a belt insists on between two items entering it, from [`scale::belt_slot_ticks`].
///
/// This is the number that sets belt throughput — one item every five ticks, 120 a minute, exactly
/// one extractor — and it is derived from the belt's speed and the spacing of the items on it
/// rather than chosen. See `scale::belt_cadence_follows_from_speed_and_spacing`.
const BELT_SLOT_TICKS: u64 = scale::belt_slot_ticks() as u64;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct GroundItem {
    pub(crate) id: u32,
    pub(crate) q: i32,
    pub(crate) r: i32,
    pub(crate) item_id: ItemId,
    pub(crate) quantity: u32,
    pub(crate) despawn_tick: u64,
}

/// Ticks a dropped item stays on the ground before disappearing (1 minute = 600 ticks at 10 TPS).
pub(crate) const GROUND_ITEM_LIFETIME_TICKS: u64 = 600;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Terrain {
    DeepWater,
    ShallowWater,
    Shore,
    Lowland,
    /// The band between lowland and highland. v0.11 read one raised band; the material base needs
    /// two, because copper belongs to rolling ground and iron and coal to the tops, and a player
    /// who cannot see the difference cannot choose a site from the terrain.
    Hills,
    Highland,
    Cliff,
}

impl Terrain {
    fn blocks_movement(self) -> bool {
        // Shallows are a ford, not a wall: the player can wade them at 5 m/s. Construction still
        // refuses them, which is why `blocks_construction` is a separate predicate and not this
        // one reused. Deep water and cliff stay impassable.
        matches!(self, Terrain::DeepWater | Terrain::Cliff)
    }

    fn blocks_construction(self) -> bool {
        matches!(
            self,
            Terrain::DeepWater | Terrain::ShallowWater | Terrain::Cliff
        )
    }

    fn is_water(self) -> bool {
        matches!(self, Terrain::DeepWater | Terrain::ShallowWater)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct ResourceState {
    item_id: ItemId,
    quantity: u32,
    initial_quantity: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct TileState {
    q: i32,
    r: i32,
    x: i32,
    y: i32,
    radius: u32,
    terrain: Terrain,
    resource: Option<ResourceState>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct PlayerState {
    x: i32,
    y: i32,
    facing_x: i16,
    facing_y: i16,
    move_x: i16,
    move_y: i16,
    inventory: BTreeMap<ItemId, u32>,
    /// The stack currently carried by the pointer. It is outside the pack's slot count but remains
    /// native-owned inventory: picking it up removes it from its source, placing it commits it to a
    /// destination, and a save in between loses neither quantity nor identity.
    #[serde(default)]
    hand: Option<Cargo>,
    action_cooldown: u32,
    build_range: u32,
    /// Slots the player can carry, from the scenario. Like `build_range` it is a fixed scenario
    /// property rather than a simulation result, so it is validated against the scenario on load
    /// instead of being hashed into the checksum.
    carry_slots: u32,
    /// The hex an autonomous walk is headed for, if one is running.
    ///
    /// This is the whole of the walk's *state*. The route to it is not: a path is a derived answer
    /// about a world that can change under it, and `Core::walk_path` rebuilds it from this goal
    /// whenever the world does, under the same rule as every other derived index. Saving the goal
    /// and rebuilding the route is also the only version of this that survives a reload honestly —
    /// a saved route would come back describing a corridor that the loaded factory may no longer
    /// have, and the player would watch themselves walk into a wall they built before saving.
    ///
    /// Saved and checksummed beside `move_x`/`move_y`, for the reason those are: it is an input the
    /// simulation is still executing, and two runs that differ only in where the player is headed
    /// will not stay identical for long.
    #[serde(default)]
    walk_goal: Option<Coordinate>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct Entity {
    id: u32,
    placed: PlacedBuilding,
    kind: BuildingKind,
    cargo: Option<Cargo>,
    inventory: BTreeMap<ItemId, u32>,
    /// Native storage compartments. `inventory` remains the general store used by containers and
    /// by version-15 machine saves; new machine deliveries go only to these named buffers.
    #[serde(default)]
    input_inventory: BTreeMap<ItemId, u32>,
    #[serde(default)]
    fuel_inventory: BTreeMap<ItemId, u32>,
    #[serde(default)]
    output_inventory: BTreeMap<ItemId, u32>,
    reserved_inputs: BTreeMap<ItemId, u32>,
    progress: u32,
    /// Energy left in the machine from fuel it has already burned. Real state: it is saved,
    /// hashed, and checksummed, because a smelter that is a quarter of the way through a coal is
    /// not the same machine as one that has just been fed.
    #[serde(default)]
    fuel_charge: u32,
    /// Electricity this machine has been given and has not spent yet. Real state for the same
    /// reason `fuel_charge` is: a smelter holding two crafts' worth of power is not the same
    /// machine as one that has just been connected, and the difference survives a save.
    #[serde(default)]
    power_charge: u32,
    /// A generator's progress toward its next whole unit of fuel energy, numerator over
    /// `power_output`. A plant carrying a tenth of the load burns a tenth of the coal, and this is
    /// where the other nine tenths of the unit waits rather than being rounded away.
    #[serde(default)]
    burn_progress: u32,
    /// Switched off by hand. Real state, saved and hashed: a smelter the player deliberately
    /// stopped is not the same machine as one that happens to be out of inputs this tick, and the
    /// difference has to survive a save or every reload would silently restart the factory.
    ///
    /// Suspension is *total and free*. A disabled machine does no work, draws no electricity, asks
    /// for none to bank, and burns no fuel — which is the whole point of the switch: it is how a
    /// player stops a burner eating coal while they rebuild the line it feeds. What it keeps is
    /// everything it was holding: stock, reserved inputs, part-finished progress, banked charge.
    /// Switching back on resumes rather than restarts.
    #[serde(default)]
    disabled: bool,
    /// Which of a splitter's compiled outputs gets the next item it can take.
    ///
    /// Real state, saved and hashed on exactly the terms `fuel_charge` is: a splitter that has just
    /// fed its left branch is not the same machine as one that has just fed its right, and a reload
    /// that forgot which would re-bias every junction in the factory toward the same branch. An
    /// index into the compiled link list, so it is meaningless — and unread — on anything else.
    #[serde(default)]
    route_cursor: u8,
    /// The id of the feeder a merger served last, so the next one it serves is the next id round
    /// the ring rather than the lowest.
    ///
    /// Stored as the feeder's *id* and not as a slot, because a merger's feeders are whatever
    /// happens to point at it: a lane erased and rebuilt changes the set, and a rotation that
    /// counted slots would silently restart. Real state for the same reason `route_cursor` is.
    #[serde(default)]
    merge_cursor: u32,
    /// Items still crossing a belt, oldest first, each stamped with the tick it stepped on.
    ///
    /// A belt hex is 5.37 m of conveyor, and an item takes [`BELT_TRANSIT_TICKS`] to cross it. That
    /// is a latency, not a throughput: a belt that could only hold the one item it hands on would
    /// move twenty-two items a minute and no chain in the game would run. So the hex holds
    /// [`BELT_LANE_SLOTS`] of them at once, spaced [`BELT_SLOT_TICKS`] apart, which is what a real
    /// conveyor does — items sit on it in a line rather than teleporting one at a time.
    ///
    /// `cargo` remains the exit slot: an item that has finished crossing leaves the lane and waits
    /// there to be handed on, so everything that offers, subtracts, splits or merges cargo goes on
    /// reading exactly one item per belt and did not have to learn about lanes.
    ///
    /// Real state, saved and hashed: a belt with four items halfway along it is not a belt with one
    /// at the end, and a reload that forgot would evaporate the contents of every line in the
    /// factory.
    #[serde(default)]
    lane: Vec<LaneItem>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct Snapshot {
    boundaries: Vec<Boundary>,
    ground: Vec<GroundCell>,
    /// Cells whose standing water has left the generated equilibrium. Sparse, like `ground`: the
    /// tile still carries the generated depth, published once, and the host adds this departure
    /// exactly as native does.
    water: Vec<hydrology::WaterCell>,
    spoil: u64,
    scenario: String,
    scenario_name: String,
    world_version: u16,
    seed: u32,
    tick: u64,
    checksum: u32,
    /// How many ticks an item takes to cross one belt hex. Published for the same reason the
    /// player's radius and the action cooldown total are: the host draws an item partway along a
    /// conveyor, and the fraction it draws has to be measured against the number the simulation
    /// actually uses rather than one the renderer keeps its own copy of.
    belt_transit_ticks: u32,
    delivered: u64,
    delivered_by_item: Vec<Ingredient64>,
    insight: u64,
    victory: bool,
    contract: ContractSnapshot,
    requests: Vec<RequestSnapshot>,
    player: PlayerSnapshot,
    researched: Vec<TechnologyId>,
    research_availability: Vec<ResearchAvailability>,
    skills: SkillsSnapshot,
    chunks: Vec<ChunkSnapshot>,
    terrain: Vec<TileSnapshot>,
    resources: Vec<ResourceSnapshot>,
    buildings: Vec<EntitySnapshot>,
    #[serde(default)]
    ground_items: Vec<GroundItem>,
    events: Vec<String>,
}

/// The same native answer used by the atomic purchase command. Derived, never saved or hashed.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct ResearchAvailability {
    technology_id: TechnologyId,
    complete: bool,
    missing_prerequisites: Vec<TechnologyId>,
    insight_shortfall: u64,
}

/// The player as the host sees it: the saved state plus the carried stacks resolved against the
/// native stack rule. The host draws `carry_stacks` one slot at a time and pads to `carry_slots`,
/// so the grid is presentation over a native answer rather than the same arithmetic written twice.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct PlayerSnapshot {
    #[serde(flatten)]
    state: PlayerState,
    carry_stacks: Vec<Ingredient>,
    /// Collision and drawing radius in world units. Published so the host draws the body that
    /// native actually walks, rather than a hardcoded fraction of the hex size.
    radius: i32,
    /// What a fresh action cooldown is worth. The host draws the wait as a proportion of this, so
    /// it never has to infer the maximum by watching a number count down.
    action_cooldown_total: u32,
    /// What the hand can gather, in hexes. Published so its held-action ring is native truth.
    extract_radius: u32,
    /// Whether this run is creative. It rides with the player because it is a fact about what the
    /// player may spend and carry, and because the host needs it in the same breath as `carry_slots`
    /// to decide whether to draw prices, refunds, and the creative panel's controls at all.
    creative: bool,
    /// The hexes still ahead on the current walk, nearest first and ending on `walk_goal`; empty
    /// when no walk is running. Published rather than re-derived host-side for the reason
    /// `carry_stacks` and `radius` are: the host draws the route the simulation is going to take,
    /// not a second opinion about it computed from the same goal by different arithmetic.
    walk_path: Vec<Coordinate>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct Ingredient64 {
    item_id: ItemId,
    quantity: u64,
}

/// The contract as the host sees it: which stage is current, what that stage is asking for, and
/// how much of each line the hub has already been given. `stage` is also how far the hub has grown,
/// so the drawing and the sentence come from the same number.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct ContractSnapshot {
    key: String,
    name: String,
    /// How many stages are finished, which is the index of the current one while any remain.
    stage: u16,
    stages: u16,
    stage_key: String,
    stage_name: String,
    stage_brief: String,
    /// Every line of the current stage's bill, with what the hub holds against it. Empty once the
    /// whole contract is complete.
    requirements: Vec<ContractRequirement>,
    complete: bool,
}

/// One posted request as the hub is holding it: which row occupies this slot.
///
/// How much has arrived against that row is *not* here. Progress belongs to the project, in
/// `Core::request_delivered`, so passing on a project and calling it back later does not throw away
/// the goods already handed over. Under finite demand that forfeit would be permanent, and a board
/// that quietly destroys deliveries is not a board a player can experiment with.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct RequestState {
    request_id: RequestId,
}

/// Where a project stands for the player who is looking at the catalogue.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ProjectState {
    /// The player cannot yet make what it asks for.
    Locked,
    /// Makeable, and not currently occupying a board slot.
    Available,
    /// On the board now.
    Posted,
    /// Finished. It has paid, and it will never be posted again.
    Complete,
}

/// One line of the project catalogue as the host sees it. Everything needed to draw the row travels
/// with it — the price above all, because a price the player has to discover by delivering is the
/// defect this whole system exists to remove.
///
/// The catalogue is published whole, not just the three posted slots, for the same reason: with a
/// finite budget the player has to be able to see what is left to earn and what it will pay before
/// choosing what to build. A board that only ever shows three rotating rows would hide the shape of
/// the remaining economy behind a draw order.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct RequestSnapshot {
    key: String,
    name: String,
    brief: String,
    item_id: ItemId,
    delivered: u32,
    required: u32,
    insight: u32,
    state: ProjectState,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct ContractRequirement {
    item_id: ItemId,
    /// Contributed toward this stage, already clamped to what the line asks for. The host draws a
    /// proportion from two published numbers rather than inferring a maximum.
    delivered: u32,
    required: u32,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct ChunkSnapshot {
    chunk_q: i32,
    chunk_r: i32,
    entity_count: usize,
    /// World-space origin and side length of the generated square this chunk owns. A chunk is the
    /// unit of world generation, so these bounds are exactly the surveyed area: everything outside
    /// the reported chunks is world the simulation has not generated yet. The square is the
    /// bounding box of the chunk's hexes on the single axial lattice.
    x: i32,
    y: i32,
    span: i32,
}

/// One surveyed cell of generated ground, as the host draws it.
///
/// Every cell of every surveyed chunk appears, including plain lowland. The band used to be the
/// whole payload and a lowland tile carried no information, so it was skipped and the host defaulted
/// the gaps; a per-cell height has no default, so the omission cannot survive it. What keeps that
/// affordable is the group being a patch rather than a resend — a newly surveyed chunk travels once
/// and is never repeated — and delta coding, which prices a neighbouring cell at the hop to it.
///
/// `height`, `water_depth` and the substrate are generated facts and nothing else: the earthwork the
/// player paid for arrives separately in the ground group, and the water they moved arrives
/// separately in the water group. The host adds each overlay exactly as native does. That is what
/// lets this list be published once and never revisited.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct TileSnapshot {
    q: i32,
    r: i32,
    x: i32,
    y: i32,
    radius: u32,
    terrain: Terrain,
    /// Generated bed elevation in the active source's native height unit: signed, absolute, and
    /// with sea level at zero once the physical source is the one answering.
    height: i32,
    /// What the bed is made of, independent of the water standing on it.
    substrate: Substrate,
    /// Standing water above the bed in the same unit as `height`. Zero is dry ground.
    water_depth: i32,
    /// Integer drainage class at this cell. Zero is still water or none at all.
    discharge: u8,
}

/// One field cell. `q`/`r` is its identity: the tile key it is stored under, and what the host
/// addresses it by in a patch. It deliberately carries no separate id — a `u64` packed from the
/// two coordinates used to travel beside them, and JSON numbers are IEEE-754 doubles, so every
/// such id past 2^53 arrived at the host rounded. Whole columns of the field collapsed onto one
/// value, and patching by it rewrote unrelated cells with a copy of the harvested one.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct ResourceSnapshot {
    q: i32,
    r: i32,
    x: i32,
    y: i32,
    radius: u32,
    item_id: ItemId,
    quantity: u32,
    initial_quantity: u32,
}

/// The water cell a pump has resolved, and the native rate that limits it.
///
/// `discharge` zero names finite standing water: `available` is the depth left and pumping moves
/// the departure. A non-zero discharge names a replenishing river and is the number of withdrawals
/// that cell can supply per tick, arbitrated by stable entity id.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct WaterSourceSnapshot {
    q: i32,
    r: i32,
    available: u32,
    discharge: u8,
    rate: u32,
}

/// Why a machine is doing what it is doing, as the inspector says it.
///
/// This is a closed set, and naming it as one is what lets the binary wire carry a byte where JSON
/// carried up to nineteen characters per entity per delta. The serialized spelling is the contract:
/// these strings are what the host renders, so a variant may not be renamed without changing what
/// the player reads. Wire codes are the declaration order and are pinned by
/// `fixtures/snapshot-delta-wire.json`.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
enum EntityStatus {
    #[serde(rename = "output blocked")]
    OutputBlocked,
    #[serde(rename = "deposit depleted")]
    DepositDepleted,
    #[serde(rename = "extracting")]
    Extracting,
    #[serde(rename = "no water in reach")]
    NoWaterInReach,
    #[serde(rename = "pumping")]
    Pumping,
    #[serde(rename = "composing")]
    Composing,
    #[serde(rename = "out of fuel")]
    OutOfFuel,
    #[serde(rename = "waiting for inputs")]
    WaitingForInputs,
    #[serde(rename = "buffered")]
    Buffered,
    #[serde(rename = "carrying")]
    Carrying,
    #[serde(rename = "receiving")]
    Receiving,
    #[serde(rename = "landing hub")]
    LandingHub,
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "no power")]
    NoPower,
    #[serde(rename = "generating")]
    Generating,
    #[serde(rename = "brownout")]
    Brownout,
    #[serde(rename = "no boiler")]
    NoBoiler,
    /// Switched off by hand. It outranks every other reason a machine is not working, because it
    /// is the only one the player chose: "out of fuel" on a burner they deliberately stopped would
    /// send them looking for a problem that is not there.
    #[serde(rename = "switched off")]
    SwitchedOff,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct EntitySnapshot {
    id: u32,
    q: i32,
    r: i32,
    definition_id: DefinitionId,
    kind: BuildingKind,
    orientation: u8,
    recipe_id: Option<RecipeId>,
    scenario_owned: bool,
    cargo: Option<Cargo>,
    /// What this belt is still carrying across its own hex, oldest first, each with the tick it
    /// stepped on. `cargo` is the item that has finished crossing and is waiting to be handed on.
    ///
    /// The host draws each of these at `(tick - entered) / belt_transit_ticks` of the way over the
    /// belt. It is published as the entry tick rather than as a fraction on purpose: a fraction
    /// changes every tick, so every belt in the factory would be a changed entity in every delta,
    /// and a line standing still would cost as much to send as one that just started.
    ///
    /// Omitted when empty, which is every machine, container and idle belt in the game.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    lane: Vec<LaneItem>,
    inventory: Vec<Ingredient>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    input_inventory: Vec<Ingredient>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fuel_inventory: Vec<Ingredient>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    output_inventory: Vec<Ingredient>,
    /// One effective port per product this building can make. Defaults are published too, so the
    /// inspector never has to reconstruct where a multi-cell building's facing exits its hull.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    output_routes: Vec<OutputRouteSnapshot>,
    /// Present only on a pump with standing water in reach. It names the cell the deterministic
    /// resolver chose and the limiting rate the tick enforces.
    #[serde(skip_serializing_if = "Option::is_none")]
    water_source: Option<WaterSourceSnapshot>,
    progress: u32,
    progress_total: u32,
    /// Energy the machine is holding, and what one craft of its recipe costs. Both are published
    /// so the inspector can say "out of fuel" for the reason the machine actually stopped rather
    /// than re-deriving the fuel rule in the host.
    ///
    /// Omitted when zero, which is what they are for every belt, container, and fuel-free machine.
    /// Sent unconditionally they cost two numbers per entity per delta — 86 KB at the largest
    /// measured tier, against a boundary priced at about 10 µs/KB — to say "this is not a furnace"
    /// about entities that never will be.
    #[serde(skip_serializing_if = "is_zero")]
    fuel_charge: u32,
    #[serde(skip_serializing_if = "is_zero")]
    fuel_required: u32,
    /// Network supply and demand, both sent so the host draws a proportion it was given.
    #[serde(skip_serializing_if = "is_zero")]
    power_satisfied: u32,
    #[serde(skip_serializing_if = "is_zero")]
    power_demand: u32,
    /// Electricity this machine is holding, against the buffer it fills to. Published for the same
    /// reason `fuel_charge` is: "brownout" is a word, and a bank draining is the picture.
    #[serde(skip_serializing_if = "is_zero")]
    power_charge: u32,
    #[serde(skip_serializing_if = "is_zero")]
    power_capacity: u32,
    status: EntityStatus,
    next_id: Option<u32>,
    /// The compiled outputs *after* the first, which only a splitter ever has.
    ///
    /// `next_id` stays the primary edge so every reader that predates junctions — the connecting
    /// deck, the inspector's downstream line, the hover trace — is unchanged on every building that
    /// will never have a second output. Omitted when empty, which is every belt, riser, underpass,
    /// merger, and machine in the game: sent unconditionally it would cost a length on every entity
    /// of every delta to say "this is not a splitter".
    #[serde(skip_serializing_if = "Vec::is_empty")]
    branch_ids: Vec<u32>,
    footprint: Vec<Coordinate>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct OutputRouteSnapshot {
    item_id: ItemId,
    q: i32,
    r: i32,
    direction: u8,
    target_id: Option<u32>,
}

/// A per-entity buildings patch. `changed` carries inserted and modified entities and `removed`
/// carries the ids the host must drop, both in ascending stable-id order. Group-level dirty
/// tracking cannot help a running factory, because one moving item resends every building.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct BuildingsDelta {
    /// Set only on a full delta, where `changed` is the complete list rather than a patch.
    #[serde(skip_serializing_if = "is_false")]
    replace: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    changed: Vec<EntitySnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    removed: Vec<u32>,
}

/// A per-cell terrain patch, addressed by tile key.
///
/// Generation is the only thing that adds a tile and nothing ever changes or removes one, so an
/// incremental patch is exactly the chunks surveyed since the host last heard — the phase brief's
/// "publish newly surveyed height chunks once". `replace` is set only by a full snapshot, where the
/// host holds nothing to patch.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct TerrainDelta {
    #[serde(skip_serializing_if = "is_false")]
    replace: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    changed: Vec<TileSnapshot>,
}

/// A per-deposit resources patch, addressed by tile key. Resource tiles are inserted by
/// world generation and updated by extraction and gathering; the tile map has no removal path, so
/// the patch needs no removal list. Generation is the only thing that adds a deposit, and it sets
/// `replace`, so an incremental patch never disturbs the order the host already holds.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct ResourcesDelta {
    /// Set when `changed` is the complete list rather than a patch.
    #[serde(skip_serializing_if = "is_false")]
    replace: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    changed: Vec<ResourceSnapshot>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

/// Which parts of the next snapshot may differ from the one the host already holds, marked where
/// state is mutated rather than discovered by diffing a freshly materialized snapshot.
///
/// This is derived presentation state: it is never saved, hashed, or checksummed, and it can never
/// change a simulation result. Every mark is a hint to rebuild one entry, and the emitted delta is
/// still filtered against the baseline the host was actually sent — so marking something that did
/// not change costs one wasted rebuild, never a wrong frame. Missing a mark would be a defect, so
/// `dirty_tracked_deltas_match_a_full_snapshot_diff` pins the whole set against a full diff.
/// Marks are appended, not inserted into an ordered set: the tick loop makes thousands of them per
/// frame, and an ordered insert costs a tree descent each time where a push costs nothing. Order
/// and uniqueness are what the delta needs, and it gets both from one sort at emit time — see
/// `drain_marks`.
#[derive(Clone, Debug, Default)]
struct SnapshotDirty {
    boundaries: bool,
    /// Set when a surface or grade changed. Sparse and small, so the group is resent whole.
    ground: bool,
    /// Set when a water departure changed. Sparse and small, so the group is resent whole.
    water: bool,
    /// Stable entity ids whose snapshot may differ, including newly placed ones.
    entities: Vec<u32>,
    /// Stable entity ids the host must drop.
    removed: BTreeSet<u32>,
    /// Tile keys of deposits whose quantity may differ.
    resources: Vec<(i32, i32)>,
    /// Set when generation may have added deposits, so the resources group is resent whole and the
    /// host's ordering stays exactly the native one.
    resources_replace: bool,
    /// Chunk keys generation has surveyed since the host last heard. Terrain only ever grows, and it
    /// grows a whole chunk at a time, so the chunk key is the whole mark: the tiles it names have
    /// never been published and every other tile in the world is already correct at the host.
    terrain: Vec<(i32, i32)>,
    /// Set when the generated chunk set or any chunk's entity count may differ.
    chunks: bool,
    /// Set when dropped ground items change.
    ground_items: bool,
}

/// Take a mark list as the ascending, duplicate-free order the delta must travel in.
fn drain_marks<T: Ord>(marks: &mut Vec<T>) -> Vec<T> {
    let mut marks = std::mem::take(marks);
    marks.sort_unstable();
    marks.dedup();
    marks
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct SnapshotDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    boundaries: Option<Vec<Boundary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ground: Option<Vec<GroundCell>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    spoil: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    water: Option<Vec<hydrology::WaterCell>>,
    base_revision: u64,
    revision: u64,
    tick: u64,
    checksum: u32,
    /// See [`Snapshot::belt_transit_ticks`]. Sent in the header of every delta rather than behind a
    /// group bit: it is a constant, it costs one byte, and a host that joined mid-run needs it to
    /// draw the very first belt it is told about.
    belt_transit_ticks: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    scenario: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scenario_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    world_version: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivered: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivered_by_item: Option<Vec<Ingredient64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    insight: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    victory: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contract: Option<ContractSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    requests: Option<Vec<RequestSnapshot>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    player: Option<PlayerSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    researched: Option<Vec<TechnologyId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    research_availability: Option<Vec<ResearchAvailability>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skills: Option<SkillsSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chunks: Option<Vec<ChunkSnapshot>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    terrain: Option<TerrainDelta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resources: Option<ResourcesDelta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    buildings: Option<BuildingsDelta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ground_items: Option<Vec<GroundItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    events: Option<Vec<String>>,
}

impl SnapshotDelta {
    fn full(base_revision: u64, revision: u64, current: &Snapshot) -> Self {
        Self {
            base_revision,
            revision,
            tick: current.tick,
            checksum: current.checksum,
            belt_transit_ticks: current.belt_transit_ticks,
            scenario: Some(current.scenario.clone()),
            scenario_name: Some(current.scenario_name.clone()),
            world_version: Some(current.world_version),
            seed: Some(current.seed),
            delivered: Some(current.delivered),
            delivered_by_item: Some(current.delivered_by_item.clone()),
            insight: Some(current.insight),
            victory: Some(current.victory),
            contract: Some(current.contract.clone()),
            requests: Some(current.requests.clone()),
            player: Some(current.player.clone()),
            researched: Some(current.researched.clone()),
            research_availability: Some(current.research_availability.clone()),
            skills: Some(current.skills.clone()),
            chunks: Some(current.chunks.clone()),
            terrain: Some(TerrainDelta {
                replace: true,
                changed: current.terrain.clone(),
            }),
            resources: Some(ResourcesDelta {
                replace: true,
                changed: current.resources.clone(),
            }),
            buildings: Some(BuildingsDelta {
                replace: true,
                changed: current.buildings.clone(),
                removed: Vec::new(),
            }),
            ground_items: Some(current.ground_items.clone()),
            boundaries: Some(current.boundaries.clone()),
            ground: Some(current.ground.clone()),
            spoil: Some(current.spoil),
            water: Some(current.water.clone()),
            events: Some(current.events.clone()),
        }
    }

    /// The reference diff between two complete snapshots. The shipped path no longer materializes a
    /// complete snapshot per frame, so this is retained as the oracle the dirty-tracked builder is
    /// pinned against — see `dirty_tracked_deltas_match_a_full_snapshot_diff`.
    #[cfg(test)]
    fn between(base_revision: u64, revision: u64, previous: &Snapshot, current: &Snapshot) -> Self {
        Self {
            base_revision,
            revision,
            tick: current.tick,
            checksum: current.checksum,
            belt_transit_ticks: current.belt_transit_ticks,
            scenario: changed(&previous.scenario, &current.scenario),
            scenario_name: changed(&previous.scenario_name, &current.scenario_name),
            world_version: changed_copy(previous.world_version, current.world_version),
            seed: changed_copy(previous.seed, current.seed),
            delivered: changed_copy(previous.delivered, current.delivered),
            delivered_by_item: changed(&previous.delivered_by_item, &current.delivered_by_item),
            insight: changed_copy(previous.insight, current.insight),
            victory: changed_copy(previous.victory, current.victory),
            contract: changed(&previous.contract, &current.contract),
            requests: changed(&previous.requests, &current.requests),
            player: changed(&previous.player, &current.player),
            researched: changed(&previous.researched, &current.researched),
            research_availability: changed(
                &previous.research_availability,
                &current.research_availability,
            ),
            skills: changed(&previous.skills, &current.skills),
            chunks: changed(&previous.chunks, &current.chunks),
            terrain: terrain_delta(&previous.terrain, &current.terrain),
            resources: resources_delta(&previous.resources, &current.resources),
            buildings: buildings_delta(&previous.buildings, &current.buildings),
            ground_items: changed(&previous.ground_items, &current.ground_items),
            boundaries: changed(&previous.boundaries, &current.boundaries),
            ground: changed(&previous.ground, &current.ground),
            spoil: changed_copy(previous.spoil, current.spoil),
            water: changed(&previous.water, &current.water),
            events: changed(&previous.events, &current.events),
        }
    }
}

/// The reference terrain diff, retained alongside `SnapshotDelta::between` as the oracle for the
/// dirty-tracked builder. A tile is never altered or removed once generation publishes it, so this
/// is exactly the cells the previous snapshot did not have — the chunks surveyed in between, in the
/// order the surveyed-chunk set already holds them.
#[cfg(test)]
fn terrain_delta(previous: &[TileSnapshot], current: &[TileSnapshot]) -> Option<TerrainDelta> {
    let before: BTreeSet<(i32, i32)> = previous.iter().map(|tile| (tile.q, tile.r)).collect();
    let changed: Vec<TileSnapshot> = current
        .iter()
        .filter(|tile| !before.contains(&(tile.q, tile.r)))
        .copied()
        .collect();
    (!changed.is_empty()).then_some(TerrainDelta {
        replace: false,
        changed,
    })
}

/// The reference resources diff, retained alongside `SnapshotDelta::between` as the oracle for the
/// dirty-tracked builder. A changed deposit set means generation ran, which resends the group whole
/// so the host's ordering stays exactly the native one; otherwise only altered deposits travel.
#[cfg(test)]
fn resources_delta(
    previous: &[ResourceSnapshot],
    current: &[ResourceSnapshot],
) -> Option<ResourcesDelta> {
    let key = |resource: &ResourceSnapshot| (resource.q, resource.r);
    let before: BTreeSet<(i32, i32)> = previous.iter().map(key).collect();
    let after: BTreeSet<(i32, i32)> = current.iter().map(key).collect();
    if before != after {
        return Some(ResourcesDelta {
            replace: true,
            changed: current.to_vec(),
        });
    }
    let existing: BTreeMap<(i32, i32), &ResourceSnapshot> = previous
        .iter()
        .map(|resource| (key(resource), resource))
        .collect();
    let changed: Vec<ResourceSnapshot> = current
        .iter()
        .filter(|resource| existing.get(&key(resource)) != Some(resource))
        .copied()
        .collect();
    (!changed.is_empty()).then_some(ResourcesDelta {
        replace: false,
        changed,
    })
}

/// Both snapshots list buildings in ascending stable entity id order, so one linear pass finds
/// every insert, update, and removal without comparing the arrays as a whole.
#[cfg(test)]
fn buildings_delta(
    previous: &[EntitySnapshot],
    current: &[EntitySnapshot],
) -> Option<BuildingsDelta> {
    let mut changed: Vec<EntitySnapshot> = Vec::new();
    let mut removed: Vec<u32> = Vec::new();
    let mut before = previous.iter().peekable();
    let mut after = current.iter().peekable();
    loop {
        match (before.peek(), after.peek()) {
            (Some(old), Some(new)) => match old.id.cmp(&new.id) {
                Ordering::Less => {
                    removed.push(old.id);
                    before.next();
                }
                Ordering::Greater => {
                    changed.push((*new).clone());
                    after.next();
                }
                Ordering::Equal => {
                    if old != new {
                        changed.push((*new).clone());
                    }
                    before.next();
                    after.next();
                }
            },
            (Some(old), None) => {
                removed.push(old.id);
                before.next();
            }
            (None, Some(new)) => {
                changed.push((*new).clone());
                after.next();
            }
            (None, None) => break,
        }
    }
    (!changed.is_empty() || !removed.is_empty()).then_some(BuildingsDelta {
        replace: false,
        changed,
        removed,
    })
}

#[cfg(test)]
fn changed<T: Clone + PartialEq>(previous: &T, current: &T) -> Option<T> {
    (previous != current).then(|| current.clone())
}

#[cfg(test)]
fn changed_copy<T: Copy + PartialEq>(previous: T, current: T) -> Option<T> {
    (previous != current).then_some(current)
}

#[derive(Serialize)]
struct PlacementPreview {
    legal: bool,
    reason: String,
}

/// One cell of a drag preview. The host draws these and nothing else: it never derives the path,
/// the heading, or the legality itself, so what is shown during a drag is what `place_line` and
/// `erase_line` will do with the same endpoints.
#[derive(Serialize)]
struct LinePreviewCell {
    q: i32,
    r: i32,
    orientation: u8,
    legal: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

/// A building's native stock compartment. `Auto` exists only at the command boundary for quick
/// transfers; explicit slot clicks always name the field they are interacting with.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum StockKind {
    #[default]
    Auto,
    Inventory,
    Input,
    Fuel,
    Output,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum InputCommand {
    BoundaryEdit {
        #[serde(flatten)]
        edit: BoundaryEdit,
    },
    UndoBoundary,
    GroundEdit {
        #[serde(flatten)]
        edit: GroundEdit,
    },
    UndoGround,
    /// Creative hydrology probe: move a bounded depth at one surveyed cell, then let native settle
    /// it. Earthworks and pumps use the same internal edit path without becoming host-side loops.
    WaterEdit {
        q: i32,
        r: i32,
        action: hydrology::WaterAction,
        quanta: u16,
    },
    MoveIntent {
        x: i16,
        y: i16,
    },
    /// Point the player at a world position — the point under the host's cursor. The host sends a
    /// target and never a heading: facing is a checksum input, so normalizing a continuous pointer
    /// angle in host floating point would be TypeScript deciding a value the simulation hashes.
    Aim {
        x: i32,
        y: i32,
    },
    Gather,
    /// Harvest one named hex inside the player's own reach. The target is explicit — the player
    /// right-clicked it — which is why this is not the facing-weighted targeting the gather
    /// invariant refuses.
    GatherAt {
        q: i32,
        r: i32,
    },
    Deposit {
        #[serde(default)]
        item_id: Option<ItemId>,
    },
    Place {
        q: i32,
        r: i32,
        definition_id: DefinitionId,
        orientation: u8,
        #[serde(default)]
        recipe_id: Option<RecipeId>,
    },
    /// One drag, resolved natively. The host sends only the two endpoints it dragged between; the
    /// path, the per-cell orientation, the legality, and the cost are all resolved here.
    PlaceLine {
        q: i32,
        r: i32,
        to_q: i32,
        to_r: i32,
        definition_id: DefinitionId,
        orientation: u8,
        #[serde(default)]
        recipe_id: Option<RecipeId>,
    },
    Erase {
        q: i32,
        r: i32,
    },
    EraseLine {
        q: i32,
        r: i32,
        to_q: i32,
        to_r: i32,
    },
    Rotate {
        q: i32,
        r: i32,
        #[serde(default)]
        reverse: bool,
    },
    /// Route one product through one exterior side of one footprint tile. The building target and
    /// the port cell are both explicit so a multi-cell refinery is never reduced to its anchor.
    SetOutputRoute {
        q: i32,
        r: i32,
        item_id: ItemId,
        output_q: i32,
        output_r: i32,
        direction: u8,
    },
    /// Grow a building into the next tier of itself, keeping its contents, its heading, and its
    /// connections. Bounded and range-checked like every other edit.
    Upgrade {
        q: i32,
        r: i32,
    },
    /// Take stock out of a container by hand. Bounded and range-checked like every other edit.
    Withdraw {
        q: i32,
        r: i32,
        item_id: ItemId,
        quantity: u32,
        #[serde(default)]
        stock: StockKind,
    },
    /// Put stock into a container by hand — the mirror of `Withdraw`, on the same contract.
    Store {
        q: i32,
        r: i32,
        item_id: ItemId,
        quantity: u32,
        #[serde(default)]
        stock: StockKind,
    },
    /// Lift a bounded amount out of the pack and hold it on the cursor.
    PickupPlayerStack {
        item_id: ItemId,
        quantity: u32,
    },
    /// Lift a bounded amount out of one named building compartment.
    PickupBuildingStack {
        q: i32,
        r: i32,
        stock: StockKind,
        item_id: ItemId,
        quantity: u32,
    },
    /// Return some or all of the cursor stack to the pack.
    PlacePlayerStack {
        quantity: u32,
    },
    /// Put some or all of the cursor stack into one named building compartment.
    PlaceBuildingStack {
        q: i32,
        r: i32,
        stock: StockKind,
        quantity: u32,
    },
    /// Drop some or all of the cursor stack onto the ground in the world.
    DropPlayerStack {
        q: i32,
        r: i32,
        quantity: u32,
    },
    /// Give a machine a different job. With fourteen recipes across five machine categories,
    /// erasing and rebuilding to change one assignment is friction the material base would add to
    /// every layout decision.
    SetRecipe {
        q: i32,
        r: i32,
        recipe_id: RecipeId,
    },
    Undo,
    PurchaseSkill {
        skill_id: u16,
    },
    Research {
        technology_id: TechnologyId,
    },
    /// Switch a machine off, or back on. Bounded and range-checked like every other edit.
    ///
    /// The state is carried, not toggled: the host sends what it wants the machine to *be*, so a
    /// press that arrives twice — a doubled tap, a replayed frame — lands on the same answer. A
    /// toggle would not, and this queue is allowed to coalesce.
    SetEnabled {
        q: i32,
        r: i32,
        enabled: bool,
    },
    /// Pass on one posted request, so the hub asks for something else in that slot.
    SkipRequest {
        slot: usize,
    },
    /// Ask the hub for one named project, taking a board slot for it. The catalogue is finite, so
    /// which project is posted has to be the player's choice rather than the draw order's.
    PostRequest {
        request_id: RequestId,
    },
    /// Turn creative mode on, or back off. Carried rather than toggled, for the same reason
    /// `SetEnabled` is: a press that arrives twice lands on the same answer.
    ///
    /// Turning it on researches everything, permanently — a technology is a thing the settlement
    /// knows, and creative teaching it then leaving does not unteach it. Turning it back off
    /// restores the prices and the refunds, so a run can be set up in creative and then played.
    SetCreative {
        enabled: bool,
    },
    /// Put an item straight into the pack, out of nowhere. Creative only, and bounded by the pack
    /// like every other way stock arrives: what will not fit is not granted.
    Grant {
        item_id: ItemId,
        quantity: u32,
    },
    /// Take an item straight back out of the pack and destroy it. Creative only. `item_id: None`
    /// empties the pack entirely, mirroring `Deposit`, so clearing it is one command rather than one
    /// per stack against a batch that holds eight.
    Discard {
        #[serde(default)]
        item_id: Option<ItemId>,
        #[serde(default)]
        quantity: u32,
    },
    /// Widen or narrow the pack. Creative only, bounded by the scenario's own number below and
    /// `MAX_CARRY_SLOTS` above, and refused outright while it would strand stock already carried.
    SetCarrySlots {
        slots: u32,
    },
    /// Walk to a hex the player pointed at, finding the way there natively.
    ///
    /// The host sends a destination and never a route, for the same reason `Aim` sends a target and
    /// never a heading and a drag sends two endpoints and never a line: the path is a checksum input
    /// and a collision question, so resolving it in TypeScript would be the host deciding a value
    /// the simulation hashes and then walks the player through.
    WalkTo {
        q: i32,
        r: i32,
    },
}

struct Core {
    boundaries: BTreeMap<Segment, Boundary>,
    boundary_hash_cache: RefCell<Option<u32>>,
    boundary_undo: Vec<BoundaryUndo>,
    /// Prepared ground: surface and graded elevation, sparse over the untreated world.
    ground: BTreeMap<(i32, i32), GroundCell>,
    /// Memoized digest of `ground` and `spoil`. Derived state under the same rule as
    /// `boundary_hash_cache`: never saved, never hashed, and the uncached walk is its oracle.
    ground_hash_cache: RefCell<Option<u32>>,
    ground_undo: Vec<GroundUndo>,
    /// Water that has left the depth the generator publishes, and only that. An untouched world
    /// carries none: the ocean, the lakes and the rivers are answers `terra` computes, not saved
    /// entities. See `hydrology` for why departure rather than depth is the thing stored.
    water: hydrology::DisturbedWater,
    /// Outside-bank stress accumulated only at coarse geomorphic epochs. Sparse saved state; a
    /// straight or untouched world carries none.
    bank_stress: geomorphology::BankStress,
    /// Rated river withdrawals already granted this tick, by source cell. Derived tick-local
    /// arbitration: cleared before each machine pass, never saved or checksummed.
    water_draws: BTreeMap<(i32, i32), u32>,
    /// Excavated material held for fill, in whole steps of one hex.
    ///
    /// Cut adds, fill spends, and nothing else touches it. Making raising ground *cost* something
    /// that can only come from lowering ground is what stops levelling being an infinite source of
    /// terrain, on the same rule that closed the insight loop in v0.35.0.
    spoil: u64,
    definitions: DefinitionsInput,
    technologies: TechnologiesInput,
    scenario: ScenarioDefinition,
    seed: u32,
    /// What the world is generated from. Saved and checksummed beside the seed, because the two
    /// answer the same question together and neither answers it alone.
    world_params: WorldParams,
    /// The resource field derived from `world_params` and `seed`, with its site lattice and its
    /// bootstrap table cached. Derived state under the same rule as `deposit_links`: never saved,
    /// never hashed, never checksummed, and rebuilt whenever the world it is derived from changes.
    fields: WorldFields,
    /// Generated bed, substrate and initial hydrology behind the current presentation. Derived
    /// from the same world identity as `fields`, cached only for surveyed chunks, and never saved,
    /// hashed or checksummed. The uncached source is its oracle.
    ground_spine: GroundSpine,
    generated_chunks: BTreeSet<(i32, i32)>,
    tiles: BTreeMap<(i32, i32), TileState>,
    /// Deposit references resolved per extractor entity id, so a running extractor never scans the
    /// tile map. Derived cache only: it is rebuilt from tiles on demand and never saved or hashed.
    deposit_links: BTreeMap<u32, Vec<(i32, i32)>>,
    /// The scenario's hand-placed resources, keyed by tile. `field_at` is asked once per hex of
    /// every surveyed chunk when a complete snapshot is built, so scanning the scenario's list for
    /// each of them made that snapshot O(hexes × placed resources) — 3.9× slower at the largest
    /// measured tier, which places one per line. Derived from the scenario definition, so it is
    /// never saved, hashed, or checksummed.
    scenario_resources: BTreeMap<(i32, i32), ResourceState>,
    /// Flora cells standing below the quantity generation gave them. Regrowth walks this set
    /// rather than the world, so a forest costs nothing until somebody cuts it and nothing again
    /// once it has grown back. Derived state under the same rule as `deposit_links`: it is a pure
    /// function of the overlay and the item definitions, so it is rebuilt on load rather than
    /// saved, and it is never hashed or checksummed.
    flora_regrowth: BTreeSet<(i32, i32)>,
    entities: Vec<Entity>,
    /// Per-entity, per-product outlet choices keyed by stable entity id. Empty means the legacy
    /// facing outlet for every product. Real saved state; compiled graph edges remain derived.
    output_routes: BTreeMap<u32, BTreeMap<ItemId, OutputRoute>>,
    /// Stable ids of belt-kind entities created before fluid transport existed. They retain the
    /// old accept-any-cargo behavior so a migrated factory keeps running; no new placement enters
    /// this set. Saved and checksummed because it changes transfer eligibility.
    legacy_fluid_belts: BTreeSet<u32>,
    ground_items: Vec<GroundItem>,
    next_ground_item_id: u32,
    graph: Vec<Links>,
    /// Stable hot-path orders and reverse transport edges derived from `entities` and `graph`.
    /// Rebuilt after edits and loads; never saved, hashed, or checksummed.
    runtime: RuntimeIndex,
    /// Per-entity power network id (`None` = not on a network). Derived like `graph`.
    power_of: Vec<Option<u32>>,
    /// Last tick's supply and demand per network id.
    power_supply: BTreeMap<u32, u32>,
    power_demand: BTreeMap<u32, u32>,
    /// Capacity harness only: consumers run at full speed so the ladder still measures transport.
    power_unmetered: bool,
    player: PlayerState,
    /// The field hex the player is currently working, while a swing is in flight.
    ///
    /// A harvest is work, and work takes time *before* it pays. `action_cooldown` used to be a wait
    /// imposed after an instant take, which handed the player the first unit of every material the
    /// moment they pressed the button and only then made them wait — the one gather in a session
    /// that was free was the first one. The counter now measures the swing that is still running
    /// and this is the hex it will land on, so the ring the host already draws is progress toward a
    /// unit rather than a debt against one already banked.
    ///
    /// Saved and checksummed beside `action_cooldown`, because the two are one fact: a save that
    /// carried the remaining work without what it is working on would come back counting down to
    /// nothing.
    pending_gather: Option<Coordinate>,
    /// The hexes still ahead of the player on the current walk, nearest first, ending on
    /// `player.walk_goal`. Derived state under the same rule as `deposit_links`: it is a pure
    /// function of the goal, the terrain, and the occupied cells, so it is rebuilt whenever the
    /// topology changes and on load, and it is never saved, hashed, or checksummed.
    walk_path: Vec<Coordinate>,
    /// Player-clock steps the current walk has made no ground. Derived session state: a walk that
    /// reloads mid-stall simply gets its second to prove itself again.
    walk_stall: u32,
    /// Where the player stood at the top of the last walk step, so `walk_stall` measures ground
    /// actually covered rather than intent issued. Derived like `walk_stall`.
    walk_last_position: (i32, i32),
    /// Whether this run builds for free with everything unlocked. Saved and checksummed: it changes
    /// what a construction costs and what an erase gives back, so two runs that differ only in this
    /// are not the same run, and a save that lost it would come back priced.
    ///
    /// It is deliberately narrow. Creative changes what the *player* may spend and carry; it does
    /// not touch power, recipe timing, belt throughput, machine behaviour, or what the hub pays. A
    /// factory built in creative runs exactly as one built in a priced run does, which is the whole
    /// point of testing in it.
    creative: bool,
    researched: BTreeSet<TechnologyId>,
    skills: SkillsState,
    next_entity_id: u32,
    tick: u64,
    delivered: u64,
    delivered_by_item: BTreeMap<ItemId, u64>,
    insight: u64,
    victory: bool,
    /// How many contract stages the hub has finished. Saved and checksummed: it is the state a
    /// founding project consists of, and the host draws the hub's growth from it.
    contract_stage: usize,
    /// What the hub has been given since the contract started, less what completed stages consumed.
    /// Every hub delivery lands here, not only the items the current stage names, so a player who
    /// automates a line early is credited for it when the stage that wants it arrives.
    contract_contributed: BTreeMap<ItemId, u64>,
    /// The requests the hub has posted, in slot order. Saved and checksummed: which standing orders
    /// are open, and how far each one has been filled, is as much a run's progress as the contract
    /// stage is, and it is the only thing that pays insight.
    requests: Vec<RequestState>,
    /// How many times each request has left the board — filled or passed on. It is also the draw
    /// order: the least-used eligible row is posted first, so fresh content leads and old standing
    /// orders come round again once there is nothing new left to post.
    request_rounds: BTreeMap<RequestId, u32>,
    /// How many times each request has been *paid*. Skip increments `request_rounds` so the row
    /// goes behind unseen content; it must not retire the project, so fills are counted apart.
    /// Saved and checksummed: a project with a fill against it is finished for this run and is
    /// never posted again.
    request_fills: BTreeMap<RequestId, u32>,
    /// How much has been handed over against each project so far, whether or not it is posted now.
    /// Saved and checksummed: under finite demand this is a run's unfinished work, and losing it on
    /// a pass would destroy goods the player cannot re-earn the reward for.
    request_delivered: BTreeMap<RequestId, u32>,
    produced: BTreeMap<ItemId, u64>,
    /// What the current (or last) swing was worth when it started. Snapshot-only: the host draws
    /// the work still outstanding against this, and a save mid-gather republishes the remaining
    /// count so the ring resumes where it stood. Never saved, hashed, or checksummed.
    last_action_cooldown_total: u32,
    events: Vec<String>,
    /// Derived presentation state: what has changed since the host's last delta. Never saved,
    /// hashed, or checksummed.
    dirty: SnapshotDirty,
    /// Ids of entities this session constructed, most recent last, so one misplacement can be taken
    /// back. Derived session state under the same rule as `deposit_links` and `dirty`: never saved,
    /// hashed, or checksummed, so a loaded save starts with nothing to undo. Undo runs the ordinary
    /// erase path, which is why it cannot invent a refund the erase tests do not already pin.
    undo_stack: Vec<u32>,
}

impl Core {
    fn new(
        definitions: &DefinitionsInput,
        technologies: &TechnologiesInput,
        scenario: &ScenarioDefinition,
        seed_override: Option<u32>,
        world_params: Option<WorldParams>,
    ) -> Result<Self, String> {
        Self::initialize(
            definitions,
            technologies,
            scenario,
            seed_override,
            world_params,
            true,
        )
    }

    /// Saved worlds validate their stored state, not a newer release's opening promises.
    fn initialize(
        definitions: &DefinitionsInput,
        technologies: &TechnologiesInput,
        scenario: &ScenarioDefinition,
        seed_override: Option<u32>,
        world_params: Option<WorldParams>,
        require_opening: bool,
    ) -> Result<Self, String> {
        let seed = seed_override.unwrap_or(scenario.seed);
        let world_params = match world_params {
            Some(params) => params,
            None => scenario
                .world_preset
                .as_deref()
                .map(|key| preset_params(key).ok_or_else(|| format!("unknown world preset {key}")))
                .transpose()?
                .unwrap_or_else(default_world_params),
        };
        world_params.validate(definitions)?;
        let ground_spine =
            GroundSpine::physical(&world_params, seed, scenario.generated_environment);
        let fields = WorldFields::new(&world_params, seed, &ground_spine);
        // A world whose opening cannot be placed is refused here rather than papered over. It is
        // the one generator failure a validator cannot see — `validate` is asked before a seed
        // exists — and shipping it would mean a run that cannot reach its own first extractor.
        if require_opening && scenario.generated_environment {
            if let Some(&(item_id, gave_up_at)) = fields.unmet.first() {
                return Err(format!(
                    "this world guarantees no item {item_id} within {gave_up_at} hexes of the \
                     landing site"
                ));
            }
        }
        let mut inventory = BTreeMap::new();
        add_ingredients(&mut inventory, &scenario.initial_inventory);
        let mut core = Self {
            definitions: definitions.clone(),
            technologies: technologies.clone(),
            scenario: scenario.clone(),
            seed,
            world_params,
            fields,
            ground_spine,
            boundaries: BTreeMap::new(),
            boundary_hash_cache: RefCell::new(None),
            boundary_undo: Vec::new(),
            ground: BTreeMap::new(),
            ground_hash_cache: RefCell::new(None),
            ground_undo: Vec::new(),
            water: hydrology::DisturbedWater::new(),
            bank_stress: geomorphology::BankStress::new(),
            water_draws: BTreeMap::new(),
            spoil: 0,
            generated_chunks: BTreeSet::new(),
            tiles: BTreeMap::new(),
            deposit_links: BTreeMap::new(),
            scenario_resources: scenario
                .resources
                .iter()
                .map(|resource| {
                    (
                        (resource.q, resource.r),
                        ResourceState {
                            item_id: resource.item_id,
                            quantity: resource.quantity,
                            initial_quantity: resource.quantity,
                        },
                    )
                })
                .collect(),
            flora_regrowth: BTreeSet::new(),
            entities: Vec::new(),
            output_routes: BTreeMap::new(),
            legacy_fluid_belts: BTreeSet::new(),
            ground_items: Vec::new(),
            next_ground_item_id: 1,
            graph: Vec::new(),
            runtime: RuntimeIndex::default(),
            power_of: Vec::new(),
            power_supply: BTreeMap::new(),
            power_demand: BTreeMap::new(),
            power_unmetered: false,
            player: PlayerState {
                x: axial_world(scenario.player_spawn.q, scenario.player_spawn.r).0,
                y: axial_world(scenario.player_spawn.q, scenario.player_spawn.r).1,
                facing_x: world_direction(scenario.player_facing).0,
                facing_y: world_direction(scenario.player_facing).1,
                move_x: 0,
                move_y: 0,
                inventory,
                hand: None,
                action_cooldown: 0,
                build_range: scenario.build_range.saturating_mul(HEX_X as u32),
                carry_slots: scenario.carry_slots,
                walk_goal: None,
            },
            pending_gather: None,
            walk_path: Vec::new(),
            walk_stall: 0,
            walk_last_position: (0, 0),
            creative: false,
            researched: scenario.initial_researched.iter().copied().collect(),
            skills: SkillsState::default(),
            next_entity_id: 1,
            tick: 0,
            delivered: 0,
            delivered_by_item: BTreeMap::new(),
            insight: 0,
            victory: false,
            contract_stage: 0,
            contract_contributed: BTreeMap::new(),
            requests: Vec::new(),
            request_rounds: BTreeMap::new(),
            request_fills: BTreeMap::new(),
            request_delivered: BTreeMap::new(),
            produced: BTreeMap::new(),
            last_action_cooldown_total: 0,
            events: vec![format!("{} ready", scenario.name)],
            dirty: SnapshotDirty::default(),
            undo_stack: Vec::new(),
        };
        core.apply_research_effects();
        core.ensure_neighborhood(core.player.x, core.player.y);
        for resource in &scenario.resources {
            core.ensure_tile(resource.q, resource.r);
            core.write_overlay(
                resource.q,
                resource.r,
                resource.item_id,
                resource.quantity,
                resource.quantity,
            );
        }
        let mut buildings = scenario.buildings.clone();
        buildings.sort_by_key(placed_sort_key);
        for placed in buildings {
            core.ensure_tile(placed.q, placed.r);
            let manual_work = core
                .building_definition(placed.definition_id)
                .is_some_and(|definition| definition.manual_work);
            let kind = core
                .building_definition(placed.definition_id)
                .ok_or_else(|| format!("unknown building definition {}", placed.definition_id))?
                .kind;
            core.entities.push(Entity {
                id: core.next_entity_id,
                placed,
                kind,
                cargo: None,
                inventory: BTreeMap::new(),
                input_inventory: BTreeMap::new(),
                fuel_inventory: BTreeMap::new(),
                output_inventory: BTreeMap::new(),
                reserved_inputs: BTreeMap::new(),
                progress: 0,
                fuel_charge: 0,
                power_charge: 0,
                burn_progress: 0,
                disabled: manual_work,
                route_cursor: 0,
                merge_cursor: 0,
                lane: Vec::new(),
            });
            core.next_entity_id += 1;
        }
        core.compile_graph();
        core.refill_requests();
        Ok(core)
    }

    fn building_definition(&self, id: DefinitionId) -> Option<&BuildingDefinition> {
        self.definitions
            .buildings
            .iter()
            .find(|value| value.id == id)
    }

    fn item_definition(&self, id: ItemId) -> Option<&ItemDefinition> {
        self.definitions.items.iter().find(|value| value.id == id)
    }

    fn recipe(&self, id: RecipeId) -> Option<&RecipeDefinition> {
        self.definitions.recipes.iter().find(|value| value.id == id)
    }

    /// What to call an item in something the player reads. Numbered only when the definitions have
    /// nothing to say, which a validated catalogue never does.
    fn item_name(&self, item: ItemId) -> String {
        self.item_definition(item)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| format!("item {item}"))
    }

    fn stack_size(&self, item: ItemId) -> u32 {
        self.item_definition(item)
            .map(|definition| definition.stack_size)
            .unwrap_or(1)
            .max(1)
    }

    fn is_fluid(&self, item: ItemId) -> bool {
        self.item_definition(item)
            .is_some_and(|definition| definition.fluid)
    }

    fn transport_medium(&self, index: usize) -> TransportMedium {
        self.building_definition(self.entities[index].placed.definition_id)
            .map_or(TransportMedium::Solid, |definition| {
                definition.transport_medium
            })
    }

    fn transport_accepts(&self, index: usize, item: ItemId) -> bool {
        // Saved pre-pipe transport remains a working compatibility line. The migration records
        // exactly those stable ids; newly placed belts never enter this set.
        if self.legacy_fluid_belts.contains(&self.entities[index].id) {
            return true;
        }
        match self.transport_medium(index) {
            TransportMedium::Solid => !self.is_fluid(item),
            TransportMedium::Fluid => self.is_fluid(item),
        }
    }

    /// Whether a transport entity can ever hand an item to this target. Capacity and current stock
    /// stay dynamic; this only removes permanently dead joins such as a fresh belt into a pipe or a
    /// solid belt into a water-only tank.
    fn transport_target_compatible(&self, source: usize, target: usize) -> bool {
        if self.entities[source].kind != BuildingKind::Belt {
            return true;
        }
        let Some(target_definition) =
            self.building_definition(self.entities[target].placed.definition_id)
        else {
            return false;
        };
        if self.entities[target].kind == BuildingKind::Belt {
            return self.definitions.items.iter().any(|item| {
                self.transport_accepts(source, item.id) && self.transport_accepts(target, item.id)
            });
        }
        target_definition
            .accepted_item_ids
            .as_ref()
            .is_none_or(|accepted| {
                accepted
                    .iter()
                    .any(|&item| self.transport_accepts(source, item))
            })
    }

    /// The placement-time half of `transport_target_compatible`: the source has a definition but
    /// no stable entity id yet, while an existing target may still be a grandfathered liquid belt.
    fn prospective_transport_target_compatible(
        &self,
        source: &BuildingDefinition,
        target: usize,
    ) -> bool {
        let source_accepts = |item: ItemId| match source.transport_medium {
            TransportMedium::Solid => !self.is_fluid(item),
            TransportMedium::Fluid => self.is_fluid(item),
        };
        let Some(target_definition) =
            self.building_definition(self.entities[target].placed.definition_id)
        else {
            return false;
        };
        if self.entities[target].kind == BuildingKind::Belt {
            return self
                .definitions
                .items
                .iter()
                .any(|item| source_accepts(item.id) && self.transport_accepts(target, item.id));
        }
        target_definition
            .accepted_item_ids
            .as_ref()
            .is_none_or(|accepted| accepted.iter().any(|&item| source_accepts(item)))
    }

    /// How many slots an inventory occupies: one per part-filled stack of each item. This is the
    /// whole of the carrying rule — the inventory itself stays an `item_id → quantity` map, so
    /// nothing about the save format, the checksum, or transfer ordering changes with it.
    fn slots_used(&self, inventory: &BTreeMap<ItemId, u32>) -> u32 {
        inventory
            .iter()
            .map(|(&item, &quantity)| {
                let stack = self.stack_size(item);
                quantity.div_ceil(stack)
            })
            .sum()
    }

    fn player_snapshot(&self) -> PlayerSnapshot {
        let cooldown_total = if self.player.action_cooldown > 0 {
            self.last_action_cooldown_total
                .max(self.player.action_cooldown)
        } else {
            self.last_action_cooldown_total.max(GATHER_COOLDOWN_STEPS)
        };
        PlayerSnapshot {
            state: self.player.clone(),
            carry_stacks: self.carry_stacks(),
            radius: PLAYER_RADIUS,
            action_cooldown_total: cooldown_total,
            extract_radius: EXTRACT_RADIUS as u32,
            creative: self.creative,
            walk_path: self.walk_path.clone(),
        }
    }

    /// The carried inventory laid out one slot at a time, in item id order and full stacks first.
    fn carry_stacks(&self) -> Vec<Ingredient> {
        let mut stacks = Vec::new();
        for (&item_id, &quantity) in &self.player.inventory {
            let stack = self.stack_size(item_id);
            let mut remaining = quantity;
            while remaining > 0 {
                let taken = remaining.min(stack);
                stacks.push(Ingredient {
                    item_id,
                    quantity: taken,
                });
                remaining -= taken;
            }
        }
        stacks
    }

    /// Whether the player could carry these items in addition to what they already hold. Every path
    /// that adds to the player's inventory has to ask, which is what makes capacity a real
    /// constraint rather than a number on the interface.
    fn player_can_carry(&self, additions: &BTreeMap<ItemId, u32>) -> bool {
        if !self.creative
            && additions
                .iter()
                .any(|(&item, &quantity)| quantity > 0 && self.is_fluid(item))
        {
            return false;
        }
        let mut prospective = self.player.inventory.clone();
        add_inventory(&mut prospective, additions);
        self.slots_used(&prospective) <= self.player.carry_slots
    }

    /// How many more of one item the player can take. A part-filled stack absorbs its remainder for
    /// free; past that, each free slot is worth a whole stack.
    fn player_room_for(&self, item_id: ItemId) -> u32 {
        if !self.creative && self.is_fluid(item_id) {
            return 0;
        }
        let stack = self.stack_size(item_id);
        let held = self.player.inventory.get(&item_id).copied().unwrap_or(0);
        let free_slots = self
            .player
            .carry_slots
            .saturating_sub(self.slots_used(&self.player.inventory));
        let partial = match held % stack {
            0 => 0,
            filled => stack - filled,
        };
        partial.saturating_add(free_slots.saturating_mul(stack))
    }

    /// Split a recovery into what the pack can still hold and what will not fit.
    ///
    /// Deliberately a walk over a working copy rather than a sum of `player_room_for` calls: each
    /// item taken consumes slots the next item would otherwise have been offered, so per-item
    /// answers would promise the same free slot twice. `BTreeMap` order makes the split itself
    /// deterministic, which matters because the remainder becomes ground items in the checksum.
    fn split_by_carry(
        &self,
        additions: &BTreeMap<ItemId, u32>,
    ) -> (BTreeMap<ItemId, u32>, BTreeMap<ItemId, u32>) {
        let mut prospective = self.player.inventory.clone();
        let mut carried = BTreeMap::new();
        let mut spilled = BTreeMap::new();
        for (&item, &quantity) in additions {
            if !self.creative && self.is_fluid(item) {
                spilled.insert(item, quantity);
                continue;
            }
            let stack = self.stack_size(item);
            let held = prospective.get(&item).copied().unwrap_or(0);
            let free_slots = self
                .player
                .carry_slots
                .saturating_sub(self.slots_used(&prospective));
            let partial = match held % stack {
                0 => 0,
                filled => stack - filled,
            };
            let room = partial.saturating_add(free_slots.saturating_mul(stack));
            let take = quantity.min(room);
            if take > 0 {
                *prospective.entry(item).or_default() += take;
                carried.insert(item, take);
            }
            if quantity > take {
                spilled.insert(item, quantity - take);
            }
        }
        (carried, spilled)
    }

    /// Turn creative mode on or off.
    ///
    /// Switching it on researches the whole tree. That is the entire implementation of "everything
    /// is unlocked": every gate in this file — `technology_met`, `category_unlocked`,
    /// `placement_legality`, and the availability the host draws its build panel from — already asks
    /// `researched`, so teaching the settlement everything unlocks all of it through the paths the
    /// ordinary game uses rather than through a second set of creative-only exceptions.
    ///
    /// What is learned stays learned when creative is switched back off, the way a Minecraft world
    /// keeps what was built in creative. Prices and refunds do come back, so a run can be laid out
    /// in creative and then played for real.
    fn set_creative(&mut self, enabled: bool) {
        if self.creative == enabled {
            return;
        }
        self.creative = enabled;
        if enabled {
            self.grant_creative_skills();
            let known = self.researched.len();
            for technology in &self.technologies.technologies {
                self.researched.insert(technology.id);
            }
            self.apply_research_effects();
            if self.researched.len() != known {
                self.refill_requests();
            }
            self.events.push("Creative mode on".into());
        } else {
            self.events.push("Creative mode off".into());
        }
    }

    /// Put an item into the pack out of nowhere. Creative only.
    ///
    /// Capacity still applies. A grant that would overflow the pack is trimmed to what fits rather
    /// than refused outright, so holding the button on a full pack tops it up and stops, and the
    /// carrying rule stays the one thing every route into the inventory obeys.
    fn grant(&mut self, item_id: ItemId, quantity: u32) -> Result<(), String> {
        if !self.creative {
            return Err("granting items needs creative mode".into());
        }
        if self.item_definition(item_id).is_none() {
            return Err("unknown item".into());
        }
        let room = self.player_room_for(item_id);
        let granted = quantity.min(room);
        if granted == 0 {
            return Err("no room to carry that".into());
        }
        *self.player.inventory.entry(item_id).or_default() += granted;
        let name = self
            .item_definition(item_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_default();
        self.events.push(format!("Granted {granted} {name}"));
        Ok(())
    }

    /// Destroy carried stock. Creative only. `item_id: None` empties the pack.
    fn discard(&mut self, item_id: Option<ItemId>, quantity: u32) -> Result<(), String> {
        if !self.creative {
            return Err("discarding items needs creative mode".into());
        }
        let Some(item_id) = item_id else {
            if self.player.inventory.is_empty() {
                return Err("nothing to discard".into());
            }
            self.player.inventory.clear();
            self.events.push("Pack cleared".into());
            return Ok(());
        };
        let held = self.player.inventory.get(&item_id).copied().unwrap_or(0);
        if held == 0 {
            return Err("nothing to discard".into());
        }
        // A quantity of zero means the whole stack, so the host can offer "drop all of this" without
        // first having to read back how much of it is held.
        let dropped = if quantity == 0 {
            held
        } else {
            quantity.min(held)
        };
        subtract_item(&mut self.player.inventory, item_id, dropped);
        let name = self
            .item_definition(item_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_default();
        self.events.push(format!("Discarded {dropped} {name}"));
        Ok(())
    }

    /// Widen or narrow the pack. Creative only.
    ///
    /// The scenario plus researched bonuses is the floor: creative may hand out room, never take
    /// away room the run earned. `MAX_CARRY_SLOTS` is the ceiling. Narrowing below what is already
    /// carried is refused rather than dropping the difference, because there is no honest place
    /// for stranded stock to go.
    fn set_carry_slots(&mut self, slots: u32) -> Result<(), String> {
        if !self.creative {
            return Err("resizing the pack needs creative mode".into());
        }
        if slots < self.earned_carry_slots() || slots > MAX_CARRY_SLOTS {
            return Err("that pack size is out of range".into());
        }
        if slots < self.slots_used(&self.player.inventory) {
            return Err("too much carried for a pack that small".into());
        }
        if slots == self.player.carry_slots {
            return Ok(());
        }
        self.player.carry_slots = slots;
        self.events.push(format!("Pack resized to {slots} slots"));
        Ok(())
    }

    fn footprint_for(&self, placed: PlacedBuilding, orientation: u8) -> Vec<Coordinate> {
        self.building_definition(placed.definition_id)
            .map(|definition| {
                Self::oriented_cells(&definition.footprint, placed.q, placed.r, orientation)
            })
            .unwrap_or_else(|| {
                vec![Coordinate {
                    q: placed.q,
                    r: placed.r,
                }]
            })
    }

    fn envelope_for(&self, placed: PlacedBuilding, orientation: u8) -> Vec<Coordinate> {
        self.building_definition(placed.definition_id)
            .map(|definition| {
                Self::oriented_cells(
                    &definition.service_envelope,
                    placed.q,
                    placed.r,
                    orientation,
                )
            })
            .unwrap_or_default()
    }

    fn clearance_for(&self, placed: PlacedBuilding, orientation: u8) -> Vec<Coordinate> {
        self.building_definition(placed.definition_id)
            .map(|definition| {
                Self::oriented_cells(
                    &definition.overhead_clearance,
                    placed.q,
                    placed.r,
                    orientation,
                )
            })
            .unwrap_or_default()
    }

    /// Rotate authored offsets onto a heading and translate them to a world anchor.
    ///
    /// No definition needs a multi-cell corner-heading footprint yet, and the validator keeps
    /// that axis single-cell (envelope and clearance included). A single `(0, 0)` cell is
    /// invariant under rotation, so leaving a corner heading unrotated is exact.
    fn oriented_cells(offsets: &[Coordinate], q: i32, r: i32, orientation: u8) -> Vec<Coordinate> {
        offsets
            .iter()
            .map(|offset| {
                let offset = match orientation {
                    NORTH.. => *offset,
                    turns => rotate_coordinate(*offset, turns),
                };
                Coordinate {
                    q: q + offset.q,
                    r: r + offset.r,
                }
            })
            .collect()
    }

    fn entity_footprint(&self, entity: &Entity) -> Vec<Coordinate> {
        self.footprint_for(entity.placed, entity.placed.orientation)
    }

    fn entity_envelope(&self, entity: &Entity) -> Vec<Coordinate> {
        self.envelope_for(entity.placed, entity.placed.orientation)
    }

    fn entity_clearance(&self, entity: &Entity) -> Vec<Coordinate> {
        self.clearance_for(entity.placed, entity.placed.orientation)
    }

    /// True when this kind may share a cell with someone else's overhead clearance.
    ///
    /// A rotor reserves air, not the ground: belts, poles and bridge decks can pass under it.
    /// Machines cannot.
    fn is_low_infrastructure(kind: BuildingKind) -> bool {
        matches!(
            kind,
            BuildingKind::Belt | BuildingKind::Pole | BuildingKind::Bridge
        )
    }

    fn pad_step_limit(&self, class: FoundationClass) -> i32 {
        match class {
            FoundationClass::Pad => self.build_step_limit(),
            FoundationClass::Span => self.walk_step_limit(),
            FoundationClass::Retaining => self.grade_limit(),
        }
    }

    /// Squared world-unit distance from the player to a hex centre.
    fn player_range_to_hex(&self, q: i32, r: i32) -> i64 {
        let (x, y) = axial_world(q, r);
        squared_distance(self.player.x, self.player.y, x, y)
    }

    fn within_world_range(&self, q: i32, r: i32, range: u32) -> bool {
        self.player_range_to_hex(q, r) <= i64::from(range).pow(2)
    }

    /// True when the player is within `range` world units of any cell this building occupies.
    ///
    /// Access is a disc around the whole footprint, not around the anchor tile: standing beside a
    /// three-cell hub's far lobe is standing beside the hub.
    fn within_world_range_of_entity(&self, index: usize, range: u32) -> bool {
        let limit = i64::from(range).pow(2);
        self.entity_footprint(&self.entities[index])
            .iter()
            .any(|cell| self.player_range_to_hex(cell.q, cell.r) <= limit)
    }

    /// True when the player stands within `radius` hex steps of any cell this building occupies.
    fn within_hex_range_of_entity(&self, index: usize, radius: i32) -> bool {
        let player = world_to_axial(self.player.x, self.player.y);
        self.entity_footprint(&self.entities[index])
            .iter()
            .any(|cell| axial_distance(player, (cell.q, cell.r)) <= radius)
    }

    /// Build-range for a named hex: the building that occupies it, measured from its whole
    /// footprint, or the hex itself when nothing stands there.
    fn within_build_range_of_target(&self, q: i32, r: i32) -> bool {
        match self.entity_at(q, r) {
            Some(index) => self.within_world_range_of_entity(index, self.player.build_range),
            None => self.within_world_range(q, r, self.player.build_range),
        }
    }

    /// `entities` is always ordered by stable id: initial ids are assigned in sorted-anchor order,
    /// placement appends the next monotonic id, erasing preserves relative order, and restoring a
    /// save re-sorts. So one marked id resolves in log time rather than a scan of the blueprint.
    fn index_of_entity(&self, id: u32) -> Option<usize> {
        self.entities
            .binary_search_by_key(&id, |entity| entity.id)
            .ok()
    }

    fn entity_at(&self, q: i32, r: i32) -> Option<usize> {
        // `occupied_entities` inserts in stable-id order, so a support below transport is replaced
        // by the later transport index just as the former reverse scan required.
        self.runtime.occupied.get(&(q, r)).copied()
    }

    fn bridge_at(&self, q: i32, r: i32) -> bool {
        self.entities.iter().any(|entity| {
            entity.kind == BuildingKind::Bridge
                && self
                    .entity_footprint(entity)
                    .iter()
                    .any(|cell| cell.q == q && cell.r == r)
        })
    }

    /// Whether an extractor (or a gather) at this hex can draw from `cell`. One predicate: the
    /// cell is a field hex inside the reach it was asked about. Placement and the cached candidate
    /// list share it, so a resolved reference cannot drift from the rule that allowed the building.
    ///
    /// Reach is a parameter rather than a constant because it is the flagship upgrade. It is still
    /// one predicate and one rule — the caller says how far *it* reaches, and a hand gather and a
    /// tier-1 extractor standing on the same hex are asking the same question at two reaches, not
    /// two questions.
    fn field_covered_at(&self, extractor: (i32, i32), cell: (i32, i32), radius: i32) -> bool {
        axial_distance(extractor, cell) <= radius && self.field_at(cell.0, cell.1).is_some()
    }

    /// How far an extractor built from this definition reaches, counting its own cell. Absent in
    /// the data means the base reach, so the tier-0 extractor is exactly what it always was.
    fn extract_radius_of(&self, definition_id: DefinitionId) -> i32 {
        self.building_definition(definition_id)
            .and_then(|definition| definition.extract_radius)
            .map(|radius| radius as i32)
            .unwrap_or(EXTRACT_RADIUS)
    }

    /// The field cell a player standing at this world point draws from: their own hex while it
    /// still holds stock, otherwise the nearest covered neighbour. `gather` and placement both go
    /// through here, so one rule decides what a hex reaches.
    /// The field cell the *player* draws from at this world point. The hand reaches `EXTRACT_RADIUS`
    /// and no tier changes that: an upgrade is something the player builds, not something they grow.
    fn resource_at_world(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        let (q, r) = world_to_axial(x, y);
        self.deposit_candidates(q, r, EXTRACT_RADIUS)
            .into_iter()
            .find(|&key| self.deposit_quantity(key) > 0)
    }

    /// Every field cell something at `(q, r)` reaching `radius` covers, ordered nearest first and
    /// then by tile key — the exact order `resource_at_world` resolves. Remaining quantity is
    /// deliberately not part of the ordering, so one resolved list stays correct for the whole life
    /// of the field.
    fn deposit_candidates(&self, q: i32, r: i32, radius: i32) -> Vec<(i32, i32)> {
        let origin = (q, r);
        let mut candidates: Vec<(i64, (i32, i32))> = hexes_in_radius(origin, radius)
            .into_iter()
            .filter(|&cell| self.field_covered_at(origin, cell, radius))
            .map(|cell| {
                let (x, y) = axial_world(q, r);
                let (cx, cy) = axial_world(cell.0, cell.1);
                (squared_distance(x, y, cx, cy), cell)
            })
            .collect();
        candidates.sort_unstable();
        candidates.into_iter().map(|(_, key)| key).collect()
    }

    fn deposit_quantity(&self, key: (i32, i32)) -> u32 {
        // The surface gate has to be here as well as in `field_at`, because a partly worked deposit
        // is answered from the tile overlay before `field_at` is ever consulted.
        if self.surface_at(key.0, key.1) != 0 {
            return 0;
        }
        if let Some(resource) = self.tiles.get(&key).and_then(|tile| tile.resource.as_ref()) {
            return resource.quantity;
        }
        self.field_at(key.0, key.1)
            .map(|field| field.quantity)
            .unwrap_or(0)
    }

    fn write_overlay(&mut self, q: i32, r: i32, item_id: ItemId, quantity: u32, initial: u32) {
        let (x, y) = axial_world(q, r);
        let terrain = self.terrain_at(q, r);
        self.tiles.insert(
            (q, r),
            TileState {
                q,
                r,
                x,
                y,
                radius: HEX_RADIUS as u32,
                terrain,
                resource: Some(ResourceState {
                    item_id,
                    quantity,
                    initial_quantity: initial,
                }),
            },
        );
        // Every overlay write is where a cell enters or leaves regrowth, so the set stays exact
        // without anything scanning the world for it.
        if quantity < initial && self.regrowth_ticks(item_id).is_some() {
            self.flora_regrowth.insert((q, r));
        } else {
            self.flora_regrowth.remove(&(q, r));
        }
    }

    /// How often one unit of this item grows back, for a resource that is flora. `None` for every
    /// ore, which is what makes ore finite.
    fn regrowth_ticks(&self, item_id: ItemId) -> Option<u32> {
        self.item_definition(item_id)
            .and_then(|item| item.regrowth_ticks)
            .filter(|&ticks| ticks > 0)
    }

    /// Rebuild the regrowth set from the overlay. It is a pure function of the stored tiles and the
    /// item definitions, so a save records the tiles and this recovers the set — the file never
    /// carries derived state.
    fn rebuild_flora_regrowth(&mut self) {
        self.flora_regrowth = self
            .tiles
            .iter()
            .filter_map(|(&key, tile)| {
                let resource = tile.resource.as_ref()?;
                // A sealed cell leaves the set: paving suppresses regrowth exactly as it suppresses
                // access, and stripping the surface rebuilds the set and lets it grow again.
                (resource.quantity < resource.initial_quantity
                    && self.surface_at(key.0, key.1) == 0
                    && self.regrowth_ticks(resource.item_id).is_some())
                .then_some(key)
            })
            .collect();
    }

    /// Grow every cut flora cell back by one unit on the cadence its item declares. Walking the
    /// marked set rather than the world is the same sparsity rule the rest of the tick follows: an
    /// untouched forest is not in the set, and a fully regrown cell leaves it.
    fn regrow_flora(&mut self) {
        if self.flora_regrowth.is_empty() {
            return;
        }
        let due: Vec<(i32, i32)> = self
            .flora_regrowth
            .iter()
            .copied()
            .filter(|key| {
                self.tiles
                    .get(key)
                    .and_then(|tile| tile.resource.as_ref())
                    .and_then(|resource| self.regrowth_ticks(resource.item_id))
                    .is_some_and(|ticks| self.tick % u64::from(ticks) == 0)
            })
            .collect();
        for key in due {
            let Some(resource) = self.tiles.get(&key).and_then(|tile| tile.resource.as_ref())
            else {
                continue;
            };
            let (item_id, quantity, initial) = (
                resource.item_id,
                resource.quantity,
                resource.initial_quantity,
            );
            if quantity >= initial {
                self.flora_regrowth.remove(&key);
                continue;
            }
            self.write_overlay(key.0, key.1, item_id, quantity + 1, initial);
            self.dirty.resources.push(key);
            if quantity == 0 {
                // A cell that had been cut to nothing can restart an extractor that reported it
                // exhausted, and every extractor covering it may now report a different status.
                self.mark_all_entities_dirty();
                self.events
                    .push(format!("Flora at {},{} regrew", key.0, key.1));
            }
        }
    }

    /// The one water source a pump resolves inside its data-defined reach.
    ///
    /// Nearest wins, then tile key. The answer names finite standing water by remaining depth and
    /// a river by its replenishing discharge class. Physical sources must be surveyed: a pump may
    /// not draw through fog, and querying one never claims a gameplay chunk.
    fn pump_source_within_reach(&self, q: i32, r: i32, radius: i32) -> Option<WaterSourceSnapshot> {
        hexes_in_radius((q, r), radius)
            .into_iter()
            .filter(|&(cell_q, cell_r)| {
                let size = self.scenario.chunk_size;
                self.generated_chunks
                    .contains(&(floor_div(cell_q, size), floor_div(cell_r, size)))
                    && self.water_depth_at(cell_q, cell_r) > 0
            })
            .min_by_key(|&(cell_q, cell_r)| {
                (axial_distance((q, r), (cell_q, cell_r)), cell_q, cell_r)
            })
            .map(|(cell_q, cell_r)| {
                let generated = self.generated_ground_at(cell_q, cell_r);
                let available = self.water_depth_of(generated, cell_q, cell_r) as u32;
                let discharge = generated.hydrology.discharge_class;
                WaterSourceSnapshot {
                    q: cell_q,
                    r: cell_r,
                    available,
                    discharge,
                    rate: if discharge == 0 {
                        available.min(1)
                    } else {
                        u32::from(discharge)
                    },
                }
            })
    }

    /// Whether open water sits inside the caller's data-defined reach.
    ///
    /// The legacy band fixture keeps its old answer. Every running physical world reads actual
    /// depth, so a drained lake stops a pump and a flooded meadow can site one.
    fn water_within_reach(&self, q: i32, r: i32, radius: i32) -> bool {
        if self.ground_is_physical() {
            self.pump_source_within_reach(q, r, radius).is_some()
        } else {
            hexes_in_radius((q, r), radius)
                .into_iter()
                .any(|(cell_q, cell_r)| self.terrain_at(cell_q, cell_r).is_water())
        }
    }

    /// The deposit an extractor draws from this tick, resolved from its cached candidate list
    /// instead of a scan over every generated tile. `generate_chunk` drops the cache whenever new
    /// tiles appear, so a reference can never outlive the tile set it was resolved against.
    fn extractor_deposit(&mut self, index: usize) -> Option<(i32, i32)> {
        let id = self.entities[index].id;
        if !self.deposit_links.contains_key(&id) {
            let placed = self.entities[index].placed;
            let radius = self.extract_radius_of(placed.definition_id);
            let candidates = self.deposit_candidates(placed.q, placed.r, radius);
            self.deposit_links.insert(id, candidates);
        }
        self.deposit_links[&id]
            .iter()
            .copied()
            .find(|&key| self.extractable_deposit(self.entities[index].placed.definition_id, key))
    }

    /// The material an extractor is working right now, read without touching the cache.
    ///
    /// `extractor_deposit` resolves the same answer but has to be able to populate `deposit_links`,
    /// so it needs `&mut self` and cannot be called from a snapshot. This reads the cache and
    /// answers `None` when it is cold, which is only ever true before the entity's first tick.
    fn extractor_material(&self, index: usize) -> Option<ItemId> {
        self.deposit_links
            .get(&self.entities[index].id)?
            .iter()
            .copied()
            .find(|&key| self.extractable_deposit(self.entities[index].placed.definition_id, key))
            .and_then(|key| self.field_at(key.0, key.1))
            .map(|field| field.item_id)
    }

    /// One extraction cycle in ticks: the material's own figure, scaled by what is digging it.
    ///
    /// Resolved per tick from the deposit actually being worked, so an arm spanning two materials
    /// runs each at its own rate rather than at whichever one it happened to see first. Falls back
    /// to the building's `cadence` when the material names no figure — that is the pump and water.
    fn extract_cycle(&self, definition_id: DefinitionId, item_id: Option<ItemId>) -> u32 {
        let definition = self.building_definition(definition_id);
        let fallback = definition.and_then(|value| value.cadence).unwrap_or(1);
        let Some(steps) = item_id
            .and_then(|id| self.item_definition(id))
            .and_then(|item| item.extract_steps)
        else {
            return fallback;
        };
        let speed = definition
            .and_then(|value| value.extract_speed)
            .unwrap_or(100)
            .max(1);
        // Rounded up, and never zero: a tier makes a cycle shorter, not free. A zero-length cycle
        // would emit one unit every tick whatever the material said.
        ((steps * 100 + speed - 1) / speed).max(1)
    }

    /// What one full cycle of this entity costs in ticks — a source's cadence, a composer's recipe
    /// duration, and zero for everything that does not run a cycle at all. Published as
    /// `progress_total` so the host draws a proportion it was given, and asked again by `upgrade`,
    /// because a tier may change the cadence under a part-finished job.
    fn progress_total(&self, index: usize) -> u32 {
        let entity = &self.entities[index];
        match entity.kind {
            BuildingKind::Extractor => {
                self.extract_cycle(entity.placed.definition_id, self.extractor_material(index))
            }
            BuildingKind::Pump => self
                .building_definition(entity.placed.definition_id)
                .and_then(|definition| definition.cadence)
                .unwrap_or(1),
            BuildingKind::Composer => entity
                .placed
                .recipe_id
                .and_then(|id| self.recipe(id))
                .map(|recipe| {
                    self.building_definition(entity.placed.definition_id)
                        .map_or(recipe.duration, |definition| {
                            definition.recipe_duration(recipe)
                        })
                })
                .unwrap_or(0),
            _ => 0,
        }
    }

    fn technology(&self, id: TechnologyId) -> Option<&TechnologyDefinition> {
        self.technologies
            .technologies
            .iter()
            .find(|value| value.id == id)
    }

    fn earned_carry_slots(&self) -> u32 {
        let (legacy, _) = research_bonuses(&self.technologies, &self.researched);
        let skills = self.skills.bonuses(&self.technologies);
        let carry_slots = legacy.saturating_add(skills.carry_slots);
        self.scenario
            .carry_slots
            .saturating_add(carry_slots)
            .min(MAX_CARRY_SLOTS)
    }

    fn earned_build_range(&self) -> u32 {
        let (_, legacy) = research_bonuses(&self.technologies, &self.researched);
        let skills = self.skills.bonuses(&self.technologies);
        let build_range = legacy.saturating_add(skills.build_range);
        self.scenario
            .build_range
            .saturating_add(build_range)
            .saturating_mul(HEX_X as u32)
    }

    /// How far the world opens around a hex the player reaches, in rings of chunks.
    ///
    /// Derived rather than stored, under the rule every other derived value follows: the skills
    /// that widen it are already saved and validated, so a saved copy of this could only ever
    /// disagree with them. It is also not a `PlayerState` field for a second reason — the surveyed
    /// world lives in `generated_chunks`, which is a checksum input, and a stored radius would be a
    /// second, unhashed account of the same thing.
    fn survey_rings(&self) -> u32 {
        BASE_SURVEY_RINGS
            .saturating_add(self.skills.bonuses(&self.technologies).survey_rings)
            .min(BASE_SURVEY_RINGS + MAX_SURVEY_RING_BONUS)
    }

    /// Apply earned skills through the same native player fields placement and carrying use.
    /// Pack size is a floor because creative mode may have widened it further; build range has no
    /// separate editor and is therefore exactly the researched value. Survey range is not here at
    /// all: `survey_rings` reads the skills directly, so there is nothing to write back.
    fn apply_research_effects(&mut self) {
        self.player.carry_slots = self.player.carry_slots.max(self.earned_carry_slots());
        self.player.build_range = self.earned_build_range();
    }

    fn generate_chunk(&mut self, chunk_q: i32, chunk_r: i32) {
        if !self.generated_chunks.insert((chunk_q, chunk_r)) {
            return;
        }
        self.ground_spine
            .cache_chunk(chunk_q, chunk_r, self.scenario.chunk_size);
        // A departure may be waiting on the far side of the old surveyed frontier. It was stored
        // without asking for this chunk's bed; now the player has opened the chunk, the bed exists
        // in the surveyed cache and the same bounded solver can continue from the first cells the
        // flux entered. Merely querying water still cannot reach this path — only survey does.
        let size = self.scenario.chunk_size;
        let resumed: Vec<(i32, i32)> = self
            .water
            .iter()
            .map(|(&(q, r), _)| (q, r))
            .filter(|&(q, r)| floor_div(q, size) == chunk_q && floor_div(r, size) == chunk_r)
            .collect();
        if !resumed.is_empty() {
            let report = self.settle_water(&resumed);
            if !report.settled {
                self.events.push(format!(
                    "Water front paused at its bound after {} cells",
                    report.cells
                ));
            }
        }
        // New tiles can cover an existing extractor, so every resolved deposit reference is stale —
        // and so is every extractor status derived from one. The two must be invalidated together:
        // dropping the entity marks would make snapshot correctness depend on generated deposits
        // never reaching an existing extractor, which nothing here enforces. Generation is rare, and
        // marks that turn out to change nothing are filtered against the baseline before they ship.
        self.deposit_links.clear();
        self.mark_all_entities_dirty();
        self.dirty.chunks = true;
        // Every cell of a new chunk is a cell the host has never seen a height for, so the whole
        // chunk is the mark. It is not narrowed to "interesting" cells the way it was when the band
        // was the only payload: a plain lowland tile now carries an elevation, a substrate and a
        // water depth that nothing else in the frame can supply.
        self.dirty.terrain.push((chunk_q, chunk_r));
        for local_r in 0..size {
            for local_q in 0..size {
                let q = chunk_q * size + local_q;
                let r = chunk_r * size + local_r;
                // Resources still narrow to a cell that actually appears in the group. Generation
                // is the only path that adds one, and resending them whole keeps the host's order
                // exactly the native one, so later patches can address field cells in place.
                self.dirty.resources_replace |= self.field_at(q, r).is_some();
            }
        }
    }

    /// Every entity snapshot is now suspect. Used by the rare paths that can change what a snapshot
    /// derives from state outside the entity itself: the compiled graph behind `next_id`, and the
    /// deposits behind an extractor's status.
    fn mark_all_entities_dirty(&mut self) {
        for index in 0..self.entities.len() {
            let id = self.entities[index].id;
            self.dirty.entities.push(id);
        }
    }

    fn terrain_at(&self, q: i32, r: i32) -> Terrain {
        self.generated_ground_at(q, r).presentation
    }

    /// What is naturally on this hex, if anything is reachable.
    ///
    /// A paved hex reports nothing. Covering is *suppression*, not harvesting: the tile overlay
    /// keeps whatever quantity was left, the surface hides it from hands, extractors, the snapshot
    /// and regrowth alike, and stripping the surface hands back exactly the deposit that was sealed.
    fn field_at(&self, q: i32, r: i32) -> Option<ResourceState> {
        if self.surface_at(q, r) != 0 {
            return None;
        }
        self.buried_field_at(q, r)
    }

    /// The deposit a hex holds regardless of what has been laid over it.
    ///
    /// A sealed deposit is invisible to every consumer, which is the point — but sealing and
    /// unsealing both have to know a deposit is there to decide whether the published field and the
    /// regrowth roster just changed, and lifting a surface has to put back exactly what went under
    /// it. This is the one view that still sees through the paving.
    fn buried_field_at(&self, q: i32, r: i32) -> Option<ResourceState> {
        if let Some(resource) = self
            .tiles
            .get(&(q, r))
            .and_then(|tile| tile.resource.as_ref())
        {
            return Some(ResourceState {
                item_id: resource.item_id,
                quantity: resource.initial_quantity,
                initial_quantity: resource.initial_quantity,
            });
        }
        if let Some(resource) = self.scenario_resources.get(&(q, r)) {
            return Some(resource.clone());
        }
        self.fields.field_at(
            q,
            r,
            self.scenario.generated_environment,
            &self.ground_spine,
        )
    }

    fn ensure_tile(&mut self, q: i32, r: i32) {
        let size = self.scenario.chunk_size;
        self.generate_chunk(floor_div(q, size), floor_div(r, size));
    }

    /// How far a survey opens around the player's own hex, in cells.
    ///
    /// Rings are the unit the skills speak in, but a ring of the chunk lattice is not a distance
    /// from the player. Standing at a chunk's edge left the frontier one cell ahead and fifteen
    /// behind, and because a chunk is an axial parallelogram rather than a disc, the opened world
    /// read as a stepped, lopsided blot instead of a horizon. The radius restates the same
    /// envelope as a distance instead, so the buffer is equal in every direction wherever inside a
    /// chunk the player happens to stand.
    ///
    /// `rings * size + size / 2` is that restatement, and it is deliberately area-preserving: at
    /// one ring it is 12 cells, a disc of 469 cells against the 448 the seven-chunk opening
    /// covered, and it stays within a few per cent at two and three rings as well. The surveying
    /// skill still widens it and nothing else changed hands.
    fn survey_radius(&self) -> i32 {
        let size = self.scenario.chunk_size;
        self.survey_rings() as i32 * size + size / 2
    }

    /// Survey the world around a point: every chunk holding a cell within [`Core::survey_radius`]
    /// of it.
    ///
    /// Chunks stay the unit of generation, so the outermost opened cell still lands on a chunk
    /// boundary. What is uniform is the guarantee — no direction is ever surveyed less far than
    /// the radius — and that guarantee is the part a player reads as an even frontier.
    fn ensure_neighborhood(&mut self, x: i32, y: i32) {
        let size = self.scenario.chunk_size;
        let (q, r) = world_to_axial(x, y);
        let radius = self.survey_radius();
        let center = (floor_div(q, size), floor_div(r, size));
        // A cell within `radius` differs by at most `radius` on each axis, so no chunk further
        // than this many chunks away can hold one. Candidates outside the disc are then dropped.
        let span = radius.div_euclid(size) + 1;
        for dq in -span..=span {
            for dr in -span..=span {
                let (chunk_q, chunk_r) = (center.0 + dq, center.1 + dr);
                if hexes_in_chunk(chunk_q, chunk_r, size)
                    .any(|cell| axial_distance((q, r), cell) <= radius)
                {
                    self.generate_chunk(chunk_q, chunk_r);
                }
            }
        }
    }

    fn compile_graph(&mut self) {
        let (occupied, envelope, clearance) = self.occupancy_maps();
        self.graph = self
            .entities
            .iter()
            .enumerate()
            .map(|(index, _)| self.compile_links(index, &occupied))
            .collect();
        self.rebuild_runtime_index(occupied, envelope, clearance);
        self.compile_power();
        // A full compile can move any entity's outgoing link, and `next_id` is part of its snapshot.
        self.mark_all_entities_dirty();
    }

    fn rebuild_runtime_index(
        &mut self,
        occupied: BTreeMap<(i32, i32), usize>,
        envelope: BTreeMap<(i32, i32), usize>,
        clearance: BTreeMap<(i32, i32), usize>,
    ) {
        let mergers = (0..self.entities.len())
            .map(|index| self.is_merger(index))
            .collect();
        self.runtime.rebuild(
            &self.entities,
            &self.graph,
            mergers,
            occupied,
            envelope,
            clearance,
        );
        // Every edit and every load funnels through here, and `occupied` is half of what a route is
        // made of, so this is the one place a standing walk has to be re-answered against the world
        // it is crossing. It is also what builds the route after a load, since the goal is saved
        // and the path deliberately is not.
        self.replan_walk();
    }

    #[cfg(test)]
    fn occupied_entities(&self) -> BTreeMap<(i32, i32), usize> {
        self.occupancy_maps().0
    }

    /// Occupied foundation, service envelope and overhead clearance as three derived maps.
    ///
    /// Occupied is the only one the transport graph and the walk read. Envelope and clearance are
    /// placement reservations: they are never saved or checksummed, and they rebuild with the
    /// occupancy index after every edit.
    fn occupancy_maps(
        &self,
    ) -> (
        BTreeMap<(i32, i32), usize>,
        BTreeMap<(i32, i32), usize>,
        BTreeMap<(i32, i32), usize>,
    ) {
        let mut occupied = BTreeMap::new();
        let mut envelope = BTreeMap::new();
        let mut clearance = BTreeMap::new();
        for (index, entity) in self.entities.iter().enumerate() {
            for cell in self.entity_footprint(entity) {
                occupied.insert((cell.q, cell.r), index);
            }
            for cell in self.entity_envelope(entity) {
                envelope.insert((cell.q, cell.r), index);
            }
            for cell in self.entity_clearance(entity) {
                clearance.insert((cell.q, cell.r), index);
            }
        }
        (occupied, envelope, clearance)
    }

    fn reserved_name(&self, index: usize) -> String {
        self.entities
            .get(index)
            .and_then(|entity| self.building_definition(entity.placed.definition_id))
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| "that building".into())
    }

    /// Whether this cell is already claimed in a way this kind cannot share.
    ///
    /// `ignore` is the building whose own envelope or clearance we are growing into, so an
    /// upgrade does not refuse the reservation it already holds.
    fn reservation_conflict(
        &self,
        q: i32,
        r: i32,
        kind: BuildingKind,
        ignore: Option<usize>,
        occupied_ok: bool,
    ) -> Result<(), String> {
        if !occupied_ok {
            if let Some(index) = self.entity_at(q, r) {
                if ignore != Some(index) {
                    return Err("building footprint overlaps an occupied hex".into());
                }
            }
        }
        if let Some(&index) = self.runtime.envelope.get(&(q, r)) {
            if ignore != Some(index) {
                return Err(format!(
                    "this hex is reserved around the {}",
                    self.reserved_name(index).to_lowercase()
                ));
            }
        }
        if let Some(&index) = self.runtime.clearance.get(&(q, r)) {
            if ignore != Some(index) && !Self::is_low_infrastructure(kind) {
                return Err(format!(
                    "the {}'s overhead clearance occupies this hex",
                    self.reserved_name(index).to_lowercase()
                ));
            }
        }
        Ok(())
    }

    /// Every outgoing transport edge one entity compiles.
    ///
    /// One edge for everything the game had before splitters: its facing. A splitter additionally
    /// rays the two headings 60° either side, which is the entire difference between it and a belt
    /// — the tick still walks compiled edges and never discovers a neighbour.
    fn compile_links(&self, index: usize, occupied: &BTreeMap<(i32, i32), usize>) -> Links {
        let entity = &self.entities[index];
        let Some(definition) = self.building_definition(entity.placed.definition_id) else {
            return Links::default();
        };
        if let Some(routes) = self.output_routes.get(&entity.id) {
            if !routes.is_empty() {
                let mut links = Links::default();
                for (&item_id, &route) in routes {
                    let origin = (entity.placed.q + route.q, entity.placed.r + route.r);
                    if let Some(target) =
                        self.trace_output_from(index, origin, route.direction, None, occupied)
                    {
                        links.push_item(Some(item_id), target);
                    }
                }
                return links;
            }
        }
        let facing = entity.placed.orientation;
        let span = definition.underpass_span;
        let mut links = Links::single(self.trace_output(index, facing, span, occupied));
        if definition.splits {
            for flank in flanks_of(facing) {
                if let Some(target) = self.trace_output(index, flank, span, occupied) {
                    links.push(target);
                }
            }
        }
        links
    }

    /// The building one output ray binds to, on one heading.
    ///
    /// An underpass tries its partner first and falls back to the ordinary ray, and that fallback
    /// is what makes the pair work with one definition and no placement mode: the *entrance* is
    /// simply the underpass that found a partner ahead of it, and the *exit* is the one that did
    /// not, so it delivers to whatever it is pointed at like any other belt.
    fn trace_output(
        &self,
        index: usize,
        orientation: u8,
        underpass_span: Option<u32>,
        occupied: &BTreeMap<(i32, i32), usize>,
    ) -> Option<usize> {
        let placed = self.entities[index].placed;
        self.trace_output_from(
            index,
            (placed.q, placed.r),
            orientation,
            underpass_span,
            occupied,
        )
    }

    fn trace_output_from(
        &self,
        index: usize,
        origin: (i32, i32),
        orientation: u8,
        underpass_span: Option<u32>,
        occupied: &BTreeMap<(i32, i32), usize>,
    ) -> Option<usize> {
        let target = underpass_span
            .and_then(|span| self.trace_underpass(index, orientation, span, occupied))
            .or_else(|| self.trace_ray_from(index, origin, orientation, occupied));
        target.filter(|&target| {
            // A dead edge is not compiled at all. Binding one and letting every tick's transfer
            // refuse it spends arbitration on a delivery that can never land, and draws the player
            // a connected line that silently is not one.
            if never_accepts_deliveries(self.entities[target].kind) {
                return false;
            }
            if !self.transport_target_compatible(index, target) {
                return false;
            }
            let to = &self.entities[target].placed;
            !self.boundary_blocks_segment(axial_world(origin.0, origin.1), axial_world(to.q, to.r))
        })
    }

    /// The entity an output ray on this heading would bind to for a building that is not placed
    /// yet, with the hex it binds at.
    ///
    /// Deliberately mirrors `trace_underpass` then `trace_ray`, in that order and with the same
    /// step table, limit, and skip-own-footprint rule, so construction refuses exactly the edge the
    /// graph compile would otherwise go on to build. An entrance that finds its partner delivers
    /// past whatever stands between, so it must not be judged on what stands between.
    fn prospective_output(
        &self,
        footprint: &[Coordinate],
        definition: &BuildingDefinition,
        orientation: u8,
    ) -> Option<(usize, (i32, i32))> {
        let anchor = footprint.first().map(|cell| (cell.q, cell.r))?;
        let (dq, dr) = TRANSPORT_DIRECTIONS[usize::from(orientation) % TRANSPORT_DIRECTIONS.len()];
        if let Some(span) = definition.underpass_span {
            let (mut q, mut r) = (anchor.0 + dq, anchor.1 + dr);
            for _ in 1..=span.min(GRAPH_TRACE_LIMIT as u32) {
                if let Some(target) = self.entity_at(q, r) {
                    let placed = &self.entities[target].placed;
                    if placed.definition_id == definition.id && placed.orientation == orientation {
                        return None;
                    }
                }
                q += dq;
                r += dr;
            }
        }
        let (mut q, mut r) = (anchor.0 + dq, anchor.1 + dr);
        for _ in 0..GRAPH_TRACE_LIMIT {
            if footprint.iter().any(|cell| cell.q == q && cell.r == r) {
                q += dq;
                r += dr;
                continue;
            }
            let target = self.entity_at(q, r)?;
            let to = &self.entities[target].placed;
            if self
                .boundary_blocks_segment(axial_world(anchor.0, anchor.1), axial_world(to.q, to.r))
            {
                return None;
            }
            return Some((target, (q, r)));
        }
        None
    }

    /// The ordinary transport ray, unchanged since the graph existed.
    ///
    /// Routing, so twelve. The loop is a ray-cast: it steps `(dq, dr)` up to `GRAPH_TRACE_LIMIT`,
    /// skipping its own footprint, and returns the first other occupied cell. Nothing in it ever
    /// assumed the step was a unit vector, which is why the six corner headings cost table rows
    /// here and nothing else.
    fn trace_ray_from(
        &self,
        index: usize,
        origin: (i32, i32),
        orientation: u8,
        occupied: &BTreeMap<(i32, i32), usize>,
    ) -> Option<usize> {
        let (dq, dr) = TRANSPORT_DIRECTIONS[usize::from(orientation) % TRANSPORT_DIRECTIONS.len()];
        let mut q = origin.0 + dq;
        let mut r = origin.1 + dr;
        for _ in 0..GRAPH_TRACE_LIMIT {
            match occupied.get(&(q, r)).copied() {
                Some(target) if target == index => {
                    q += dq;
                    r += dr;
                }
                target => return target,
            }
        }
        None
    }

    /// The partner an underpass hands its cargo to, or `None` if there is none within its span.
    ///
    /// This is the whole of "belts cross belts": the ray passes *over* every occupied cell instead
    /// of binding to the first one, so the line that runs between the two ends is untouched, keeps
    /// its own cargo, and never sees the cargo going over it. What stops that from being a free
    /// belt of unlimited reach is the span, and what stops it from stealing an ordinary delivery is
    /// that it binds to nothing except another underpass of the same definition on the same
    /// heading. The covered hexes stay ordinary ground: buildable, walkable, and erasable.
    fn trace_underpass(
        &self,
        index: usize,
        orientation: u8,
        span: u32,
        occupied: &BTreeMap<(i32, i32), usize>,
    ) -> Option<usize> {
        let entity = &self.entities[index];
        let definition_id = entity.placed.definition_id;
        let (dq, dr) = TRANSPORT_DIRECTIONS[usize::from(orientation) % TRANSPORT_DIRECTIONS.len()];
        let mut q = entity.placed.q + dq;
        let mut r = entity.placed.r + dr;
        let reach = span.min(GRAPH_TRACE_LIMIT as u32);
        for _ in 1..=reach {
            if let Some(target) = occupied.get(&(q, r)).copied() {
                let partner = target != index
                    && self.entities[target].placed.definition_id == definition_id
                    && self.entities[target].placed.orientation == orientation;
                if partner {
                    return Some(target);
                }
            }
            q += dq;
            r += dr;
        }
        None
    }

    fn compile_power(&mut self) {
        let n = self.entities.len();
        self.power_of = vec![None; n];
        self.power_supply.clear();
        self.power_demand.clear();
        if n == 0 {
            self.runtime.rebuild_power(&self.power_of);
            return;
        }
        let mut parent: Vec<usize> = (0..n).collect();
        let find = |parent: &mut [usize], mut index: usize| -> usize {
            while parent[index] != index {
                parent[index] = parent[parent[index]];
                index = parent[index];
            }
            index
        };
        let union = |parent: &mut [usize], a: usize, b: usize, ids: &[u32]| {
            let pa = find(parent, a);
            let pb = find(parent, b);
            if pa == pb {
                return;
            }
            if ids[pa] < ids[pb] {
                parent[pb] = pa;
            } else {
                parent[pa] = pb;
            }
        };
        let ids: Vec<u32> = self.entities.iter().map(|entity| entity.id).collect();
        let poles: Vec<usize> = (0..n)
            .filter(|&index| {
                self.building_definition(self.entities[index].placed.definition_id)
                    .is_some_and(|definition| definition.kind == BuildingKind::Pole)
            })
            .collect();
        let machines: Vec<usize> = (0..n)
            .filter(|&index| {
                let Some(definition) =
                    self.building_definition(self.entities[index].placed.definition_id)
                else {
                    return false;
                };
                definition.kind != BuildingKind::Pole
                    && (definition.power_output.unwrap_or(0) > 0
                        || definition.power_draw.unwrap_or(0) > 0)
            })
            .collect();
        // Poles form the long-range graph, and each pole supplies the machines inside its own
        // coverage. Machines attach to poles rather than to each other at range, so a plant of
        // extractors with no poles is linear rather than quadratic.
        for (offset, &left) in poles.iter().enumerate() {
            for &right in &poles[offset + 1..] {
                if self.power_linked(left, right) {
                    union(&mut parent, left, right, &ids);
                }
            }
        }
        for &machine in &machines {
            for &pole in &poles {
                if self.power_linked(machine, pole) {
                    union(&mut parent, machine, pole, &ids);
                }
            }
        }
        // Touching machines conduct. A generator standing beside a smelter runs it, a block of
        // machines built shoulder to shoulder wires itself, and a pole becomes what *distance*
        // costs rather than what power costs — which is what the balance tool's opening prices
        // have always assumed.
        //
        // Only buildings that draw or generate conduct. If belts and containers carried current,
        // a line of the cheapest building in the game would be free wire across the map and no
        // player would ever place the second pole.
        //
        // Walked through a cell index rather than pairwise, so the pass is linear in machines
        // instead of quadratic: `entity_at` is a scan, and six of them per footprint cell per
        // machine is the shape of a compile that gets slower the more factory there is.
        let mut cells: BTreeMap<(i32, i32), usize> = BTreeMap::new();
        for &machine in &machines {
            for cell in self.entity_footprint(&self.entities[machine]) {
                cells.insert((cell.q, cell.r), machine);
            }
        }
        for &machine in &machines {
            for cell in self.entity_footprint(&self.entities[machine]) {
                for &(dq, dr) in &DIRECTIONS {
                    if let Some(&other) = cells.get(&(cell.q + dq, cell.r + dr)) {
                        if other != machine {
                            union(&mut parent, machine, other, &ids);
                        }
                    }
                }
            }
        }
        for index in poles.into_iter().chain(machines) {
            let root = find(&mut parent, index);
            self.power_of[index] = Some(ids[root]);
        }
        self.runtime.rebuild_power(&self.power_of);
        self.refresh_power_meters();
    }

    fn power_linked(&self, left: usize, right: usize) -> bool {
        let Some(a) = self.building_definition(self.entities[left].placed.definition_id) else {
            return false;
        };
        let Some(b) = self.building_definition(self.entities[right].placed.definition_id) else {
            return false;
        };
        let distance = self.power_distance(left, right);
        let a_pole = a.kind == BuildingKind::Pole;
        let b_pole = b.kind == BuildingKind::Pole;
        if a_pole && b_pole {
            let reach = i32::max(
                a.pole_reach.unwrap_or(DEFAULT_POLE_REACH as u32) as i32,
                b.pole_reach.unwrap_or(DEFAULT_POLE_REACH as u32) as i32,
            );
            return distance <= reach;
        }
        if a_pole || b_pole {
            // Coverage is the pole's, not the machine's. This is the whole of the upgrade: a
            // better pole lights a wider disc, and every machine already standing in it connects
            // without being touched.
            let pole = if a_pole { a } else { b };
            let radius = pole
                .supply_radius
                .unwrap_or(DEFAULT_POLE_SUPPLY_RADIUS as u32) as i32;
            return distance <= radius;
        }
        false
    }

    fn power_distance(&self, left: usize, right: usize) -> i32 {
        let mut best = i32::MAX;
        for a in self.entity_footprint(&self.entities[left]) {
            for b in self.entity_footprint(&self.entities[right]) {
                best = best.min(axial_distance((a.q, a.r), (b.q, b.r)));
            }
        }
        best
    }

    /// What the meters read: live generation and the standing draw of the machines that have work.
    ///
    /// Deliberately *not* the buffer-fill requests `distribute_power` allocates against. A machine
    /// with a full bank asks for nothing that tick, and a needle that dropped to zero every time
    /// the factory got comfortable would be telling the player about the accounting rather than
    /// about their grid. Supply against standing draw is the number that answers "can this plant
    /// carry this factory".
    fn refresh_power_meters(&mut self) {
        let previous_supply = self.power_supply.clone();
        let previous_demand = self.power_demand.clone();
        self.power_supply.clear();
        self.power_demand.clear();
        for offset in 0..self.runtime.power_order.len() {
            let index = self.runtime.power_order[offset];
            let Some(net) = self.power_of.get(index).copied().flatten() else {
                continue;
            };
            let definition_id = self.entities[index].placed.definition_id;
            let draw = self
                .building_definition(definition_id)
                .and_then(|definition| definition.power_draw)
                .unwrap_or(0);
            let output = self
                .building_definition(definition_id)
                .and_then(|definition| definition.power_output)
                .unwrap_or(0);
            if draw > 0 && self.power_work_wanted(index) {
                *self.power_demand.entry(net).or_default() += draw;
            }
            if output > 0 {
                *self.power_supply.entry(net).or_default() += self.generator_output_now(index);
            }
        }
        if self.power_unmetered {
            self.power_supply = self.power_demand.clone();
        }
        if self.power_supply != previous_supply || self.power_demand != previous_demand {
            for offset in 0..self.runtime.power_order.len() {
                let index = self.runtime.power_order[offset];
                self.dirty.entities.push(self.entities[index].id);
            }
        }
    }

    /// Whether this machine has work its next tick of power would actually buy.
    ///
    /// This predicate is the entire fuel rule. A machine with nothing to do asks the grid for
    /// nothing, so the grid draws nothing from its plants, so the plants burn nothing — there is no
    /// separate "throttle the generator" step anywhere, because there is nothing to throttle.
    fn power_work_wanted(&self, index: usize) -> bool {
        let entity = &self.entities[index];
        // A machine switched off has no work its next tick of power would buy, so it asks for
        // none — and by the rule above, nothing burns anywhere to supply it.
        if entity.disabled {
            return false;
        }
        match entity.kind {
            // A blocked extractor or pump has produced something nobody has taken. It is not
            // waiting on power and must not hold a share of it.
            BuildingKind::Extractor | BuildingKind::Pump => {
                self.room_for_stock(index, StockKind::Output, 0) > 0
            }
            BuildingKind::Composer => {
                let Some(recipe) = entity.placed.recipe_id.and_then(|id| self.recipe(id)) else {
                    return false;
                };
                if !self.room_for_recipe(index, recipe) {
                    return false;
                }
                // Mid-craft always wants power: the inputs are already spent and the only thing
                // between the machine and its output is time it has to be paid for.
                if entity.progress > 0 {
                    return true;
                }
                let stocked = recipe.inputs.iter().all(|ingredient| {
                    self.stock_quantity(index, StockKind::Input, ingredient.item_id)
                        >= ingredient.quantity
                });
                stocked && self.fuel_ready(entity)
            }
            _ => false,
        }
    }

    /// How much electricity this machine wants banked: `POWER_BUFFER_CYCLES` whole cycles of the
    /// work it is set up to do. A machine with no recipe, or no work, wants nothing.
    fn power_capacity(&self, index: usize) -> u32 {
        let draw = self
            .building_definition(self.entities[index].placed.definition_id)
            .and_then(|definition| definition.power_draw)
            .unwrap_or(0);
        if draw == 0 {
            return 0;
        }
        // `progress_total` is already the length of one cycle for every kind that has one — a
        // cadence for an extractor or pump, a recipe duration for a composer — so the buffer is
        // sized off the same number the progress bar fills against rather than a second opinion.
        draw.saturating_mul(self.progress_total(index).max(1))
            .saturating_mul(POWER_BUFFER_CYCLES)
    }

    /// One tick of the grid: every network is filled from its plants, and every plant burns for
    /// exactly the energy it was asked to hand over.
    ///
    /// Energy is conserved. What machines bank equals what plants produced, to the unit, which is
    /// why throughput comes out exactly proportional to generation without a slowdown factor
    /// anywhere: an undersupplied factory is not scaled down, it is simply given less to spend.
    fn distribute_power(&mut self) {
        self.refresh_power_meters();
        if self.power_unmetered {
            return;
        }
        // Requests, by network, in ascending entity id — which is index order, so every
        // apportionment below is over a list whose order is a save's order.
        let mut requests: BTreeMap<u32, Vec<(usize, u64)>> = BTreeMap::new();
        let mut plants: BTreeMap<u32, Vec<(usize, u64)>> = BTreeMap::new();
        for offset in 0..self.runtime.power_order.len() {
            let index = self.runtime.power_order[offset];
            let Some(net) = self.power_of.get(index).copied().flatten() else {
                continue;
            };
            let definition_id = self.entities[index].placed.definition_id;
            let Some(definition) = self.building_definition(definition_id) else {
                continue;
            };
            let draw = definition.power_draw.unwrap_or(0);
            let output = definition.power_output.unwrap_or(0);
            if output > 0 {
                let live = self.generator_output_now(index);
                if live > 0 {
                    plants
                        .entry(net)
                        .or_default()
                        .push((index, u64::from(live)));
                }
            } else if draw > 0 && self.power_work_wanted(index) {
                let want = u64::from(self.power_capacity(index))
                    .saturating_sub(u64::from(self.entities[index].power_charge));
                if want > 0 {
                    requests.entry(net).or_default().push((index, want));
                }
            }
        }
        for (net, asked) in requests {
            let Some(offers) = plants.get(&net) else {
                continue;
            };
            let available: u64 = offers.iter().map(|&(_, offer)| offer).sum();
            let wanted: u64 = asked.iter().map(|&(_, want)| want).sum();
            let used = available.min(wanted);
            if used == 0 {
                continue;
            }
            let weights: Vec<u64> = asked.iter().map(|&(_, want)| want).collect();
            for (&(index, _), granted) in asked.iter().zip(apportion(used, &weights)) {
                if granted == 0 {
                    continue;
                }
                self.entities[index].power_charge += granted as u32;
                let id = self.entities[index].id;
                self.dirty.entities.push(id);
            }
            // The same split over the plants, so what was produced equals what was banked and no
            // generator burns for a unit that never reached a machine.
            let offered: Vec<u64> = offers.iter().map(|&(_, offer)| offer).collect();
            let sources: Vec<usize> = offers.iter().map(|&(index, _)| index).collect();
            for (index, produced) in sources.into_iter().zip(apportion(used, &offered)) {
                self.burn_for_output(index, produced as u32);
            }
        }
    }

    /// Charge a plant for the electricity it just produced.
    ///
    /// A generator running flat out spends one unit of fuel energy per tick, so `power_output` is
    /// the exchange rate, and a plant carrying a fifth of the load pays a fifth as often.
    /// `burn_progress` is where the fraction waits, which is what keeps a lightly loaded burner
    /// honest instead of either free or rounded up to a whole coal every tick.
    fn burn_for_output(&mut self, index: usize, produced: u32) {
        if produced == 0 {
            return;
        }
        let Some(definition) = self.building_definition(self.entities[index].placed.definition_id)
        else {
            return;
        };
        let (source, rate) = (
            definition.power_source,
            definition.power_output.unwrap_or(0),
        );
        if rate == 0 {
            return;
        }
        // Wind and water are paid for once, at construction. Only a plant with a bill has one.
        if !matches!(
            source,
            Some(PowerSource::Burner) | Some(PowerSource::Turbine)
        ) {
            return;
        }
        self.entities[index].burn_progress += produced;
        let units = self.entities[index].burn_progress / rate;
        if units == 0 {
            return;
        }
        self.entities[index].burn_progress -= units * rate;
        match source {
            Some(PowerSource::Burner) => {
                if self.charge_fuel(index, units, &[]) {
                    self.entities[index].fuel_charge -= units;
                }
            }
            // A turbine has no firebox of its own: the bill lands on the boiler beside it, which is
            // where the coal and the water actually are.
            Some(PowerSource::Turbine) => {
                if let Some(boiler) = self.adjacent_live_boiler_index(index) {
                    let water = self.entities[boiler]
                        .input_inventory
                        .get(&WATER_ITEM)
                        .copied()
                        .unwrap_or(0)
                        .saturating_add(
                            self.entities[boiler]
                                .inventory
                                .get(&WATER_ITEM)
                                .copied()
                                .unwrap_or(0),
                        )
                        .min(units);
                    if water > 0 {
                        self.subtract_stock(boiler, StockKind::Input, WATER_ITEM, water);
                    }
                    if self.charge_fuel(boiler, units, &[]) {
                        self.entities[boiler].fuel_charge -= units;
                    }
                    let id = self.entities[boiler].id;
                    self.dirty.entities.push(id);
                }
            }
            _ => {}
        }
        let id = self.entities[index].id;
        self.dirty.entities.push(id);
    }

    fn generator_output_now(&self, index: usize) -> u32 {
        // A plant switched off offers nothing to its network, which is what stops a burner eating
        // coal on behalf of a line the player has deliberately stopped.
        if self.entities[index].disabled {
            return 0;
        }
        let Some(definition) = self.building_definition(self.entities[index].placed.definition_id)
        else {
            return 0;
        };
        let output = definition.power_output.unwrap_or(0);
        if output == 0 {
            return 0;
        }
        match definition.power_source {
            Some(PowerSource::Burner) => {
                if self.generator_has_fuel(index) {
                    output
                } else {
                    0
                }
            }
            Some(PowerSource::Wind) => output,
            Some(PowerSource::Hydro) => {
                let placed = self.entities[index].placed;
                let radius = definition.extract_radius.unwrap_or(PUMP_RADIUS as u32) as i32;
                if self.water_within_reach(placed.q, placed.r, radius) {
                    output
                } else {
                    0
                }
            }
            Some(PowerSource::Turbine) => {
                if self.adjacent_live_boiler(index) {
                    output
                } else {
                    0
                }
            }
            None => 0,
        }
    }

    fn generator_has_fuel(&self, index: usize) -> bool {
        let entity = &self.entities[index];
        entity.fuel_charge > 0
            || self.burnable_item(&entity.fuel_inventory, &[]).is_some()
            || self.burnable_item(&entity.inventory, &[]).is_some()
    }

    fn boiler_live(&self, index: usize) -> bool {
        let entity = &self.entities[index];
        // A boiler switched off raises no steam, so the turbines beside it read as having no
        // boiler at all — the switch travels the pair the same way fuel and water do.
        !entity.disabled
            && self.stock_quantity(index, StockKind::Input, WATER_ITEM) >= 1
            && (entity.fuel_charge > 0
                || self.burnable_item(&entity.fuel_inventory, &[]).is_some()
                || self.burnable_item(&entity.inventory, &[]).is_some())
    }

    fn adjacent_live_boiler(&self, index: usize) -> bool {
        self.adjacent_live_boiler_index(index).is_some()
    }

    /// The boiler a turbine's bill lands on: the lowest-id live one it touches, so a turbine
    /// wedged between two boilers always empties the same one and a save reproduces which.
    fn adjacent_live_boiler_index(&self, index: usize) -> Option<usize> {
        let mut best: Option<usize> = None;
        for cell in self.entity_footprint(&self.entities[index]) {
            for &(dq, dr) in &DIRECTIONS {
                if let Some(other) = self.entity_at(cell.q + dq, cell.r + dr) {
                    if self.entities[other].kind == BuildingKind::Boiler && self.boiler_live(other)
                    {
                        best = Some(match best {
                            Some(current)
                                if self.entities[current].id <= self.entities[other].id =>
                            {
                                current
                            }
                            _ => other,
                        });
                    }
                }
            }
        }
        best
    }

    /// Spend banked electricity on `base` ticks of progress, returning the ticks actually paid for.
    ///
    /// The machine buys work out of its own bank rather than out of a network ratio. A brownout is
    /// therefore not a slowdown factor applied to a machine: it is a machine that ran out of what
    /// it was given, and it resumes at full speed the moment the grid hands it more.
    fn power_progress(&mut self, index: usize, base: u32) -> u32 {
        if self.power_unmetered || base == 0 {
            return base;
        }
        let draw = self
            .building_definition(self.entities[index].placed.definition_id)
            .and_then(|definition| definition.power_draw)
            .unwrap_or(0);
        if draw == 0 {
            return base;
        }
        let charge = self.entities[index].power_charge;
        let afforded = base.min(charge / draw);
        if afforded == 0 {
            return 0;
        }
        self.entities[index].power_charge = charge - afforded * draw;
        afforded
    }

    /// Whether this machine can pay for a tick of work right now — it holds at least one tick's
    /// draw. What gates a craft is the bank, not the network, so a machine on a dead grid keeps
    /// running until the energy it was already given runs out.
    fn entity_powered(&self, index: usize) -> bool {
        if self.power_unmetered {
            return true;
        }
        let draw = self
            .building_definition(self.entities[index].placed.definition_id)
            .and_then(|definition| definition.power_draw)
            .unwrap_or(0);
        draw == 0 || self.entities[index].power_charge >= draw
    }

    /// Whether this machine is wired to anything that is generating. Separates "no power" from
    /// "brownout": the first is a grid problem the player fixes with a pole or a plant, the second
    /// is a capacity problem they fix with more generation.
    fn entity_connected(&self, index: usize) -> bool {
        if self.power_unmetered {
            return true;
        }
        let Some(net) = self.power_of.get(index).copied().flatten() else {
            return false;
        };
        self.power_supply.get(&net).copied().unwrap_or(0) > 0
    }

    fn network_of(&self, index: usize) -> (u32, u32) {
        let Some(net) = self.power_of.get(index).copied().flatten() else {
            return (0, 0);
        };
        (
            self.power_supply.get(&net).copied().unwrap_or(0),
            self.power_demand.get(&net).copied().unwrap_or(0),
        )
    }

    // `advance_power_plants` used to live here: one pass that burned a unit of fuel per plant per
    // tick whenever its network had any demand at all, so one extractor cost a burner exactly what
    // five composers did. Its work is now `burn_for_output`, charged against energy the grid
    // actually delivered, which is why there is no longer a separate plant phase in the tick.

    fn graph_links_by_id(&self) -> BTreeMap<u32, LinkIds> {
        self.entities
            .iter()
            .enumerate()
            .map(|(index, entity)| {
                (
                    entity.id,
                    self.graph[index].edges.map(|edge| {
                        edge.map(|edge| LinkId {
                            item_id: edge.item_id,
                            target_id: self.entities[edge.target].id,
                        })
                    }),
                )
            })
            .collect()
    }

    fn recompile_graph_components(
        &mut self,
        old_links: &BTreeMap<u32, LinkIds>,
        changed_cells: &BTreeSet<(i32, i32)>,
        edited_ids: &BTreeSet<u32>,
    ) -> usize {
        // Erasing shifts vector indices, so preserve unaffected edges through stable entity IDs.
        let (occupied, envelope, clearance) = self.occupancy_maps();
        let indices_by_id: BTreeMap<u32, usize> = self
            .entities
            .iter()
            .enumerate()
            .map(|(index, entity)| (entity.id, index))
            .collect();
        let mut ray_origins: BTreeMap<(i32, i32), Vec<u32>> = BTreeMap::new();
        for entity in &self.entities {
            ray_origins
                .entry((entity.placed.q, entity.placed.r))
                .or_default()
                .push(entity.id);
            if let Some(routes) = self.output_routes.get(&entity.id) {
                for route in routes.values() {
                    ray_origins
                        .entry((entity.placed.q + route.q, entity.placed.r + route.r))
                        .or_default()
                        .push(entity.id);
                }
            }
        }

        let mut graph: Vec<Links> = self
            .entities
            .iter()
            .map(|entity| {
                let mut links = Links::default();
                for link in old_links
                    .get(&entity.id)
                    .copied()
                    .unwrap_or_default()
                    .into_iter()
                    .flatten()
                {
                    if let Some(&index) = indices_by_id.get(&link.target_id) {
                        links.push_item(link.item_id, index);
                    }
                }
                links
            })
            .collect();

        let mut old_adjacency: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
        for (&source, targets) in old_links {
            old_adjacency.entry(source).or_default();
            for link in targets.iter().copied().flatten() {
                old_adjacency
                    .entry(source)
                    .or_default()
                    .insert(link.target_id);
                old_adjacency
                    .entry(link.target_id)
                    .or_default()
                    .insert(source);
            }
        }

        let mut affected = edited_ids.clone();
        // An edit can change the edited entity's own output or an output ray that crosses any cell
        // in its old/new footprint. The trace bound matches the full compiler's footprint walk.
        //
        // Twelve headings, not six: routing is what this walk inverts, and a splitter's flanks and
        // an underpass's covered cells are rays like any other. A cell reached from `source` along
        // heading `d` at distance `k` means `source` sits at `cell - k · d`, so inverting all
        // twelve is exactly the set of buildings whose output could have crossed this cell.
        for &(q, r) in changed_cells {
            if let Some(&index) = occupied.get(&(q, r)) {
                affected.insert(self.entities[index].id);
            }
            for (dq, dr) in TRANSPORT_DIRECTIONS {
                for distance in 1..=GRAPH_TRACE_LIMIT {
                    if let Some(sources) = ray_origins.get(&(q - dq * distance, r - dr * distance))
                    {
                        affected.extend(sources.iter().copied());
                    }
                }
            }
        }

        expand_components(&mut affected, &old_adjacency);
        // New edges can merge previously independent components. Recompile and expand until every
        // newly reached target's prior weak component is included.
        loop {
            let current_ids: Vec<u32> = affected
                .iter()
                .filter(|id| indices_by_id.contains_key(id))
                .copied()
                .collect();
            let mut joined = false;
            for id in current_ids {
                let index = indices_by_id[&id];
                let links = self.compile_links(index, &occupied);
                graph[index] = links;
                for target in links.iter() {
                    joined |= affected.insert(self.entities[target].id);
                }
            }
            let before = affected.len();
            expand_components(&mut affected, &old_adjacency);
            if !joined && affected.len() == before {
                break;
            }
        }

        let recompiled = affected
            .iter()
            .filter(|id| indices_by_id.contains_key(id))
            .count();
        self.graph = graph;
        self.rebuild_runtime_index(occupied, envelope, clearance);
        self.compile_power();
        // Exactly the entities whose outgoing link was recomputed, so their `next_id` may differ.
        self.dirty.entities.extend(
            affected
                .iter()
                .filter(|id| indices_by_id.contains_key(id))
                .copied(),
        );
        recompiled
    }

    fn tick_many(&mut self, count: u32) {
        if count > 0 {
            self.events.clear();
        }
        self.advance_ticks(count);
    }

    fn advance_ticks(&mut self, count: u32) {
        for _ in 0..count {
            // Fill the grid, then spend it. Both halves of power happen before any machine moves,
            // so no machine can be paid out of energy a later machine in the same tick produced.
            self.distribute_power();
            self.water_draws.clear();
            self.advance_machines();
            self.transfer_cargo();
            self.tick += 1;
            self.advance_geomorphology();
            self.regrow_flora();
            self.advance_ground_items();
        }
    }

    fn advance_ground_items(&mut self) {
        if self.ground_items.is_empty() {
            return;
        }
        let before_len = self.ground_items.len();
        self.ground_items
            .retain(|item| item.despawn_tick > self.tick);
        if self.ground_items.len() != before_len {
            self.dirty.ground_items = true;
        }
    }

    /// Walk the player on its own cadence. Movement deliberately no longer rides the simulation
    /// tick: a paused factory should not pin the player in place, and a 0.25× factory should not
    /// make walking feel broken. Frame-coupled movement stays refused — the host sends a step
    /// count, not a delta — so the same command sequence still reproduces the same position and the
    /// same checksum.
    /// The player's clock. It runs on elapsed real time rather than factory time, so everything
    /// the player does themselves — walking, and the work one field action costs — keeps the same
    /// pace whether the factory is paused, slowed, or running flat out. A swing lands on the step
    /// that finishes it, on this clock and no other, which is why a paused factory can still be
    /// mined and a fast one cannot be mined any faster.
    fn advance_player_steps(&mut self, count: u32) {
        for _ in 0..count {
            let working = self.player.action_cooldown > 0;
            self.player.action_cooldown = self.player.action_cooldown.saturating_sub(1);
            if working && self.player.action_cooldown == 0 {
                self.finish_gather();
            }
            // Steering before the step, so the intent the step consumes is the one this step's
            // position asked for. A walk is deliberately not interrupted by gathering: the swing
            // above runs on the same clock and the two have never blocked each other.
            self.steer_walk();
            self.advance_player();
            self.collect_ground_items();
        }
    }

    fn collect_ground_items(&mut self) {
        if self.ground_items.is_empty() {
            return;
        }
        let moving =
            self.player.move_x != 0 || self.player.move_y != 0 || self.player.walk_goal.is_some();
        if !moving {
            return;
        }
        let player_hex = world_to_axial(self.player.x, self.player.y);
        let mut changed = false;
        let mut idx = 0;
        while idx < self.ground_items.len() {
            let item = self.ground_items[idx];
            if item.despawn_tick.saturating_sub(self.tick) > GROUND_ITEM_LIFETIME_TICKS - 30 {
                idx += 1;
                continue;
            }
            let hex_dist = axial_distance(player_hex, (item.q, item.r));
            let within_reach = if hex_dist == 0 {
                true
            } else if hex_dist == 1 {
                self.within_world_range(item.q, item.r, (PLAYER_RADIUS as u32) + 400)
            } else {
                false
            };
            if within_reach {
                let room = self.player_room_for(item.item_id);
                if room > 0 {
                    let collected = item.quantity.min(room);
                    *self.player.inventory.entry(item.item_id).or_default() += collected;
                    let name = self
                        .item_definition(item.item_id)
                        .map(|definition| definition.name.clone())
                        .unwrap_or_else(|| format!("item {}", item.item_id));
                    self.events.push(format!("Picked up {collected} × {name}"));
                    changed = true;
                    if collected == item.quantity {
                        self.ground_items.remove(idx);
                        continue;
                    } else {
                        self.ground_items[idx].quantity -= collected;
                    }
                }
            }
            idx += 1;
        }
        if changed {
            self.dirty.ground_items = true;
        }
    }

    fn advance_machines(&mut self) {
        for offset in 0..self.runtime.machine_order.len() {
            let index = self.runtime.machine_order[offset];
            // A machine the player switched off does nothing at all — one check here rather than a
            // guard at the head of each `advance_*`, so a new machine kind cannot be added and quietly
            // ignore the switch.
            if !self.entity_running(index) {
                continue;
            }
            match self.entities[index].kind {
                BuildingKind::Extractor => self.advance_extractor(index),
                BuildingKind::Composer => self.advance_composer(index),
                BuildingKind::Pump => self.advance_pump(index),
                _ => {}
            }
        }
    }

    fn advance_extractor(&mut self, index: usize) {
        if self.room_for_stock(index, StockKind::Output, 0) == 0 {
            return;
        }
        let (q, r, definition_id) = {
            let placed = self.entities[index].placed;
            (placed.q, placed.r, placed.definition_id)
        };
        let id = self.entities[index].id;
        let resource_key = self.extractor_deposit(index);
        let available = resource_key
            .map(|key| self.deposit_quantity(key))
            .unwrap_or(0);
        self.dirty.entities.push(id);
        if available == 0 {
            self.entities[index].progress = 0;
            return;
        }
        let add = self.power_progress(index, 1);
        if add == 0 {
            return;
        }
        let resource_key = resource_key.expect("available resource key exists");
        let field = self
            .field_at(resource_key.0, resource_key.1)
            .expect("available resource exists");
        // The cycle is read from the material under the arm, so an extractor that finishes a coal
        // deposit and falls through to the clay beside it changes rate with it.
        let cadence = self.extract_cycle(definition_id, Some(field.item_id));
        self.entities[index].progress += add;
        if self.entities[index].progress < cadence {
            return;
        }
        let remaining = self.deposit_quantity(resource_key) - 1;
        self.write_overlay(
            resource_key.0,
            resource_key.1,
            field.item_id,
            remaining,
            field.initial_quantity,
        );
        let item_id = field.item_id;
        let depleted = remaining == 0;
        self.dirty.resources.push(resource_key);
        *self.entities[index]
            .output_inventory
            .entry(item_id)
            .or_default() += 1;
        self.entities[index].progress = 0;
        *self.produced.entry(item_id).or_default() += 1;
        if depleted {
            // Any extractor covering this deposit may now report a different status or fall
            // through to a different candidate.
            self.mark_all_entities_dirty();
            self.events
                .push(format!("Deposit at {q},{r} is worked out"));
        }
    }

    /// Draw one loose-water item from the source this pump names.
    ///
    /// A river's discharge class is its replenishing per-tick allowance. Stable machine order
    /// arbitrates pumps sharing it. Standing water has no allowance: one item removes one depth
    /// quantum, then the bounded solve lets the surrounding pond answer the draw.
    fn advance_pump(&mut self, index: usize) {
        let (q, r, definition_id) = {
            let placed = self.entities[index].placed;
            (placed.q, placed.r, placed.definition_id)
        };
        let id = self.entities[index].id;
        self.dirty.entities.push(id);
        let definition = self.building_definition(definition_id);
        let cadence = definition.and_then(|value| value.cadence).unwrap_or(1);
        let Some(item_id) = definition.and_then(|value| value.output_item_id) else {
            return;
        };
        if self.room_for_stock(index, StockKind::Output, item_id) == 0 {
            return;
        }
        let radius = definition
            .and_then(|value| value.extract_radius)
            .unwrap_or(PUMP_RADIUS as u32) as i32;
        let source = if self.ground_is_physical() {
            self.pump_source_within_reach(q, r, radius)
        } else {
            None
        };
        if self.ground_is_physical() && source.is_none()
            || !self.ground_is_physical() && !self.water_within_reach(q, r, radius)
        {
            self.entities[index].progress = 0;
            return;
        }
        if source.is_some_and(|source| {
            source.discharge > 0
                && self
                    .water_draws
                    .get(&(source.q, source.r))
                    .copied()
                    .unwrap_or(0)
                    >= u32::from(source.discharge)
        }) {
            return;
        }
        let add = self.power_progress(index, 1);
        if add == 0 {
            return;
        }
        self.entities[index].progress += add;
        if self.entities[index].progress < cadence {
            return;
        }
        if source.is_some_and(|source| !self.draw_pump_source(source)) {
            // Hold completed work rather than burning another tick of power. The next tick clears
            // river arbitration and the lowest stable id asks first again.
            return;
        }
        *self.entities[index]
            .output_inventory
            .entry(item_id)
            .or_default() += 1;
        self.entities[index].progress = 0;
        *self.produced.entry(item_id).or_default() += 1;
    }

    fn draw_pump_source(&mut self, source: WaterSourceSnapshot) -> bool {
        if source.discharge > 0 {
            let drawn = self.water_draws.entry((source.q, source.r)).or_default();
            if *drawn >= u32::from(source.discharge) {
                return false;
            }
            *drawn += 1;
            return true;
        }
        if self.water_depth_at(source.q, source.r) <= 0 {
            return false;
        }
        let departure = i32::from(self.water.delta_at(source.q, source.r).get()) - 1;
        self.water.set(
            source.q,
            source.r,
            hydrology::WaterDelta::new(
                i16::try_from(departure).expect("one pump draw fits the water store"),
            ),
        );
        let report = self.settle_water(&[(source.q, source.r)]);
        if !report.settled {
            self.events.push(format!(
                "Pump water paused at its bound after {} cells",
                report.cells
            ));
        }
        // Any pump, route or hydro source may now resolve a different answer.
        self.mark_all_entities_dirty();
        self.replan_walk();
        true
    }

    fn fuel_value(&self, item_id: ItemId) -> u32 {
        self.item_definition(item_id)
            .and_then(|item| item.fuel_value)
            .unwrap_or(0)
    }

    /// The lowest-id item a machine holding this inventory may burn. Never the quantity a recipe
    /// input reserves: steel names coal in its `inputs`, and a smelter that burned the very coal it
    /// was waiting on would starve itself on its own recipe. One predicate serves both the tick
    /// that burns and the status line that explains why nothing burned.
    fn burnable_item(
        &self,
        inventory: &BTreeMap<ItemId, u32>,
        inputs: &[Ingredient],
    ) -> Option<ItemId> {
        inventory
            .iter()
            .find(|&(&item_id, &quantity)| {
                let reserved = inputs
                    .iter()
                    .find(|input| input.item_id == item_id)
                    .map_or(0, |input| input.quantity);
                quantity > reserved && self.fuel_value(item_id) > 0
            })
            .map(|(&item_id, _)| item_id)
    }

    /// Burn stored fuel until the machine holds at least `required` energy, reporting whether it
    /// got there.
    fn charge_fuel(&mut self, index: usize, required: u32, inputs: &[Ingredient]) -> bool {
        while self.entities[index].fuel_charge < required {
            let item_id = self
                .burnable_item(&self.entities[index].fuel_inventory, &[])
                .or_else(|| self.burnable_item(&self.entities[index].inventory, inputs));
            let Some(item_id) = item_id else {
                return false;
            };
            let value = self.fuel_value(item_id);
            if self.entities[index]
                .fuel_inventory
                .get(&item_id)
                .copied()
                .unwrap_or(0)
                > 0
            {
                subtract_item(&mut self.entities[index].fuel_inventory, item_id, 1);
            } else {
                subtract_item(&mut self.entities[index].inventory, item_id, 1);
            }
            self.entities[index].fuel_charge += value;
            // Burning is a visible change even on a tick the craft does not start, because the
            // machine banks the charge and its stock went down.
            let id = self.entities[index].id;
            self.dirty.entities.push(id);
        }
        true
    }

    fn advance_composer(&mut self, index: usize) {
        let manual = self.is_manual_workshop(index);
        if manual && !self.can_work_here(index) {
            self.entities[index].disabled = true;
            self.dirty.entities.push(self.entities[index].id);
            return;
        }
        let Some(recipe_id) = self.entities[index].placed.recipe_id else {
            return;
        };
        let Some(recipe) = self.recipe(recipe_id).cloned() else {
            return;
        };
        if !self.room_for_recipe(index, &recipe) {
            return;
        }
        if self.entities[index].progress > 0 {
            let id = self.entities[index].id;
            self.dirty.entities.push(id);
            self.entities[index].progress += self.power_progress(index, 1);
            if self.entities[index].progress >= self.progress_total(index) {
                for output in recipe.outputs() {
                    *self.entities[index]
                        .output_inventory
                        .entry(output.item_id)
                        .or_default() += output.quantity;
                }
                self.entities[index].progress = 0;
                self.entities[index].reserved_inputs.clear();
                if manual {
                    self.entities[index].disabled = true;
                    self.events.push(format!("Finished {}", recipe.name));
                    self.observe_skill_event(SkillEvent::WorkshopCraft);
                } else if self
                    .building_definition(self.entities[index].placed.definition_id)
                    .is_some_and(|d| d.power_draw.unwrap_or(0) > 0)
                {
                    self.observe_skill_event(SkillEvent::PoweredCraft);
                }
            }
            return;
        }
        let can_start = recipe.inputs.iter().all(|ingredient| {
            self.stock_quantity(index, StockKind::Input, ingredient.item_id) >= ingredient.quantity
        });
        // Fuel is charged at the moment a craft starts, beside the inputs it reserves, so a
        // half-finished job can never be holding energy it has not paid for.
        if can_start
            && self.entity_powered(index)
            && self.charge_fuel(index, recipe.fuel, &recipe.inputs)
        {
            let id = self.entities[index].id;
            self.dirty.entities.push(id);
            self.entities[index].fuel_charge -= recipe.fuel;
            for ingredient in &recipe.inputs {
                self.subtract_stock(
                    index,
                    StockKind::Input,
                    ingredient.item_id,
                    ingredient.quantity,
                );
                *self.entities[index]
                    .reserved_inputs
                    .entry(ingredient.item_id)
                    .or_default() += ingredient.quantity;
            }
            self.entities[index].progress = 1;
        }
    }

    /// What one building is offering to hand on this tick, if anything.
    ///
    /// A container feeds from its store and everything else from the single cargo it is holding.
    /// Lifted out of `transfer_cargo` unchanged so the two arbitration passes below ask it once
    /// each rather than each carrying its own copy of the rule.
    fn cargo_on_offer(&self, source: usize) -> Option<(Cargo, StockKind)> {
        let entity = &self.entities[source];
        if entity.kind == BuildingKind::Container {
            let (&item_id, _) = entity.inventory.iter().find(|(_, value)| **value > 0)?;
            return Some((
                Cargo {
                    item_id,
                    quantity: 1,
                },
                StockKind::Inventory,
            ));
        }
        if let Some(cargo) = entity.cargo {
            return Some((cargo, StockKind::Auto));
        }
        let (&item_id, _) = entity
            .output_inventory
            .iter()
            .find(|(_, value)| **value > 0)?;
        Some((
            Cargo {
                item_id,
                quantity: 1,
            },
            StockKind::Output,
        ))
    }

    /// Move one cargo out of `source` and into `target`, with no question left to ask.
    fn hand_over(&mut self, source: usize, target: usize, cargo: Cargo, stock: StockKind) {
        match stock {
            StockKind::Auto => self.entities[source].cargo = None,
            _ => self.subtract_stock(source, stock, cargo.item_id, cargo.quantity),
        }
        let (source_id, target_id) = (self.entities[source].id, self.entities[target].id);
        self.dirty.entities.push(source_id);
        self.dirty.entities.push(target_id);
        self.accept(target, cargo);
    }

    /// Whether this definition serves its feeders in rotation instead of in entity id order.
    fn is_merger(&self, index: usize) -> bool {
        self.building_definition(self.entities[index].placed.definition_id)
            .is_some_and(|definition| definition.merges)
    }

    /// One tick of deliveries along the compiled graph.
    ///
    /// Two passes, because two different junctions want two different answers to "who goes first".
    ///
    /// **Mergers first.** A belt holds one cargo, so several lanes pointed into one hex are
    /// competing every tick, and the historical rule — sort by entity id, first proposal claims the
    /// target — hands the win to the same lane forever. That is not a tie-break, it is a starved
    /// lane: whichever feeder happened to be built first drinks the junction dry. A merger walks
    /// its feeders from the one *after* the one it served last, so a junction of two full lanes
    /// alternates and a junction of three cycles. Ordinary belts keep the id order exactly, so
    /// nothing that worked before behaves differently.
    ///
    /// **Everything else second**, in ascending entity id, unchanged. A splitter differs only in
    /// having more than one compiled output to offer its cargo to, and it offers them starting from
    /// its own cursor so consecutive items go to different branches.
    ///
    /// Either pass, only a belt's *exit slot* is on offer: everything else it is carrying is still
    /// somewhere along its 5.37 m of conveyor, and gets there by [`Core::advance_belt_lanes`],
    /// which runs first so an item that finished crossing this tick can leave on it.
    fn transfer_cargo(&mut self) {
        self.advance_belt_lanes();
        self.runtime.clear_transfer_scratch();
        if self.runtime.merger_targets.is_empty() {
            self.transfer_along_links();
            return;
        }
        for target_offset in 0..self.runtime.merger_targets.len() {
            let target = self.runtime.merger_targets[target_offset];
            let cursor = self.entities[target].merge_cursor;
            // Start after the id served last and wrap. An id that no longer exists simply means
            // every feeder sorts after it, which is the same as starting from the beginning.
            let start = self.runtime.feeders[target]
                .iter()
                .position(|&source| self.entities[source].id > cursor)
                .unwrap_or(0);
            let feeder_count = self.runtime.feeders[target].len();
            for offset in 0..feeder_count {
                let source = self.runtime.feeders[target][(start + offset) % feeder_count];
                if self.runtime.delivered[source] {
                    continue;
                }
                let Some((cargo, stock)) = self.cargo_on_offer(source) else {
                    continue;
                };
                if !self.can_accept(target, cargo) {
                    // A full merger refuses every feeder, so there is nothing left to try and no
                    // reason to move the cursor: the rotation resumes where it stopped.
                    break;
                }
                self.hand_over(source, target, cargo, stock);
                self.entities[target].merge_cursor = self.entities[source].id;
                self.runtime.claimed[target] = true;
                self.runtime.delivered[source] = true;
                break;
            }
        }

        // Pass two: everything else, in the entity id order arbitration has always used.
        self.transfer_along_links();
    }

    /// Move every belt's queue along: an item that has finished crossing its hex leaves the lane and
    /// waits in the exit slot for someone to take it.
    ///
    /// This is the whole of a belt's motion, and it costs one comparison per belt per tick because
    /// nothing is counted down — a lane item carries the tick it stepped on, and crossing is over
    /// when [`BELT_TRANSIT_TICKS`] have passed since. A belt nobody is feeding does no work and
    /// reports itself unchanged, so an idle line neither ticks nor re-sends.
    ///
    /// Only one item leaves the lane per tick, which is the one the exit slot can hold. A blocked
    /// belt therefore backs up: the items behind finish crossing, find the slot taken, and wait
    /// where they are, which is what the player is looking at when a line jams.
    fn advance_belt_lanes(&mut self) {
        for offset in 0..self.runtime.belt_order.len() {
            let index = self.runtime.belt_order[offset];
            let entity = &self.entities[index];
            if entity.cargo.is_some() {
                continue;
            }
            let Some(head) = entity.lane.first() else {
                continue;
            };
            if self.tick.saturating_sub(head.entered) < BELT_TRANSIT_TICKS {
                continue;
            }
            let arrived = self.entities[index].lane.remove(0);
            self.entities[index].cargo = Some(arrived.cargo);
            let id = self.entities[index].id;
            self.dirty.entities.push(id);
        }
    }

    /// Everything a belt is holding, exit slot first, for the paths that have to account for all of
    /// it at once — erasing the belt, and hashing it.
    fn belt_contents(entity: &Entity) -> impl Iterator<Item = Cargo> + '_ {
        entity
            .cargo
            .into_iter()
            .chain(entity.lane.iter().map(|item| item.cargo))
    }

    /// Every source that has not already delivered offers its cargo along its compiled edges, in
    /// ascending entity id — the arbitration order the game has always had.
    ///
    /// A splitter is the only thing here with more than one edge, and it starts from its own cursor
    /// so consecutive items leave by different branches.
    fn transfer_along_links(&mut self) {
        for source_offset in 0..self.runtime.transport_order.len() {
            let source = self.runtime.transport_order[source_offset];
            if self.runtime.delivered[source] || self.graph[source].is_empty() {
                continue;
            }
            let Some((cargo, stock)) = self.cargo_on_offer(source) else {
                continue;
            };
            let links = self.graph[source];
            let outputs: Vec<usize> = links.iter_for(cargo.item_id).collect();
            if outputs.is_empty() {
                continue;
            }
            let cursor = usize::from(self.entities[source].route_cursor);
            for offset in 0..outputs.len() {
                let slot = (cursor + offset) % outputs.len();
                let target = outputs[slot];
                // Merging targets were settled by the rotation pass, including the case where every
                // feeder was refused. Re-offering here would put the id order back in front of it.
                if self.runtime.claimed[target]
                    || self.runtime.mergers[target]
                    || !self.can_accept(target, cargo)
                {
                    continue;
                }
                self.hand_over(source, target, cargo, stock);
                // The next item starts at the branch after the one this item took, which is the
                // whole of the round robin. Left where it is on a blocked branch, so a splitter
                // with one jammed output keeps feeding the other rather than stalling every other
                // item against the jam.
                self.entities[source].route_cursor = ((slot + 1) % outputs.len()) as u8;
                self.runtime.claimed[target] = true;
                self.runtime.delivered[source] = true;
                break;
            }
        }
    }

    /// Whether this building has any use for that item, ignoring whether it has room for one.
    ///
    /// Split from `can_accept` so the two questions a delivery asks — *would you want this* and
    /// *have you space* — can be asked apart. A belt only ever needs both at once, but a hand
    /// transfer has to tell "a burner does not eat iron" from "the burner is full", and answering
    /// with one bit made those the same refusal.
    fn accepts_item(&self, target: usize, item_id: ItemId) -> bool {
        let entity = &self.entities[target];
        match entity.kind {
            BuildingKind::Belt => self.transport_accepts(target, item_id),
            BuildingKind::Container => self
                .building_definition(entity.placed.definition_id)
                .and_then(|definition| definition.accepted_item_ids.as_ref())
                .is_none_or(|ids| ids.contains(&item_id)),
            BuildingKind::Consumer => true,
            BuildingKind::Composer => {
                let Some(recipe) = entity.placed.recipe_id.and_then(|id| self.recipe(id)) else {
                    return false;
                };
                // A machine takes its recipe's inputs, and — when the recipe needs heat — anything
                // that burns. Fuel is not in `inputs`, so this is where a belt of coal is allowed
                // into a smelter without every smelting recipe having to name a fuel.
                let burns = recipe.fuel > 0 && self.fuel_value(item_id) > 0;
                burns || recipe.inputs.iter().any(|input| input.item_id == item_id)
            }
            // The hub takes what it asked for and nothing else, by belt exactly as by hand. A line
            // pointed at it backs up once the board and the contract are satisfied, which is a
            // legible answer — the belt shows it — where silently voiding the cargo was not.
            BuildingKind::Hub => self.hub_demand(item_id) > 0,
            BuildingKind::Extractor
            | BuildingKind::Pump
            | BuildingKind::Pole
            | BuildingKind::Bridge => false,
            // Fuel goes only where fuel is burned. A wind turbine keeps an `inventory` like every
            // other generator and has no firebox to spend it in, so coal delivered to one used to
            // sit there forever — a belt could quietly bury a stack in a machine that would never
            // touch it. A boiler additionally drinks, and that is the only thing it does not burn.
            BuildingKind::Generator | BuildingKind::Boiler => {
                let burns = self.fuel_value(item_id) > 0
                    && matches!(
                        self.building_definition(entity.placed.definition_id)
                            .and_then(|definition| definition.power_source),
                        Some(PowerSource::Burner) | None
                    );
                burns || (entity.kind == BuildingKind::Boiler && item_id == WATER_ITEM)
            }
        }
    }

    /// Resolve the one compartment that may receive this item. Inputs outrank fuel deliberately:
    /// coal is an ingredient in steel as well as something that burns, and feeding that recipe must
    /// not silently divert its bill into the firebox.
    fn stock_kind_for_item(&self, target: usize, item_id: ItemId) -> Option<StockKind> {
        let entity = &self.entities[target];
        match entity.kind {
            BuildingKind::Container => Some(StockKind::Inventory),
            BuildingKind::Composer => {
                let recipe = entity.placed.recipe_id.and_then(|id| self.recipe(id))?;
                if recipe.inputs.iter().any(|input| input.item_id == item_id) {
                    Some(StockKind::Input)
                } else if recipe.fuel > 0 && self.fuel_value(item_id) > 0 {
                    Some(StockKind::Fuel)
                } else {
                    None
                }
            }
            BuildingKind::Boiler if item_id == WATER_ITEM => Some(StockKind::Input),
            BuildingKind::Generator | BuildingKind::Boiler
                if self.fuel_value(item_id) > 0
                    && matches!(
                        self.building_definition(entity.placed.definition_id)
                            .and_then(|definition| definition.power_source),
                        Some(PowerSource::Burner) | None
                    ) =>
            {
                Some(StockKind::Fuel)
            }
            _ => None,
        }
    }

    fn stock_accepts_item(&self, target: usize, stock: StockKind, item_id: ItemId) -> bool {
        match stock {
            StockKind::Auto => self.stock_kind_for_item(target, item_id).is_some(),
            StockKind::Output => false,
            named => self.stock_kind_for_item(target, item_id) == Some(named),
        }
    }

    /// Quantity visible in one compartment. Version-15 machine stock still lives in `inventory`;
    /// classifying it here lets an old kiln immediately present clay as input and coal as fuel while
    /// preserving the old save checksum until either stack is next moved.
    fn stock_quantity(&self, target: usize, stock: StockKind, item_id: ItemId) -> u32 {
        let entity = &self.entities[target];
        let explicit = match stock {
            StockKind::Inventory => entity.inventory.get(&item_id),
            StockKind::Input => entity.input_inventory.get(&item_id),
            StockKind::Fuel => entity.fuel_inventory.get(&item_id),
            StockKind::Output => entity.output_inventory.get(&item_id),
            StockKind::Auto => None,
        }
        .copied()
        .unwrap_or(0);
        let legacy = if stock != StockKind::Inventory
            && self.stock_kind_for_item(target, item_id) == Some(stock)
        {
            entity.inventory.get(&item_id).copied().unwrap_or(0)
        } else {
            0
        };
        let cargo = if stock == StockKind::Output {
            entity
                .cargo
                .filter(|cargo| cargo.item_id == item_id)
                .map_or(0, |cargo| cargo.quantity)
        } else {
            0
        };
        explicit.saturating_add(legacy).saturating_add(cargo)
    }

    fn stock_total(&self, target: usize, stock: StockKind) -> u32 {
        let entity = &self.entities[target];
        let explicit = match stock {
            StockKind::Inventory => inventory_total(&entity.inventory),
            StockKind::Input => inventory_total(&entity.input_inventory),
            StockKind::Fuel => inventory_total(&entity.fuel_inventory),
            StockKind::Output => inventory_total(&entity.output_inventory),
            StockKind::Auto => 0,
        };
        let legacy = if matches!(stock, StockKind::Input | StockKind::Fuel) {
            entity
                .inventory
                .iter()
                .filter(|&(&item_id, _)| self.stock_kind_for_item(target, item_id) == Some(stock))
                .map(|(_, &quantity)| quantity)
                .sum()
        } else {
            0
        };
        let cargo = if stock == StockKind::Output {
            entity.cargo.map_or(0, |cargo| cargo.quantity)
        } else {
            0
        };
        explicit.saturating_add(legacy).saturating_add(cargo)
    }

    fn add_stock(&mut self, target: usize, stock: StockKind, cargo: Cargo) {
        let map = match stock {
            StockKind::Inventory => &mut self.entities[target].inventory,
            StockKind::Input => &mut self.entities[target].input_inventory,
            StockKind::Fuel => &mut self.entities[target].fuel_inventory,
            StockKind::Output => &mut self.entities[target].output_inventory,
            StockKind::Auto => return,
        };
        *map.entry(cargo.item_id).or_default() += cargo.quantity;
    }

    fn subtract_stock(&mut self, target: usize, stock: StockKind, item_id: ItemId, quantity: u32) {
        let explicit = match stock {
            StockKind::Inventory => self.entities[target]
                .inventory
                .get(&item_id)
                .copied()
                .unwrap_or(0),
            StockKind::Input => self.entities[target]
                .input_inventory
                .get(&item_id)
                .copied()
                .unwrap_or(0),
            StockKind::Fuel => self.entities[target]
                .fuel_inventory
                .get(&item_id)
                .copied()
                .unwrap_or(0),
            StockKind::Output => self.entities[target]
                .output_inventory
                .get(&item_id)
                .copied()
                .unwrap_or(0),
            StockKind::Auto => 0,
        };
        let from_explicit = explicit.min(quantity);
        if from_explicit > 0 {
            match stock {
                StockKind::Inventory => {
                    subtract_item(&mut self.entities[target].inventory, item_id, from_explicit)
                }
                StockKind::Input => subtract_item(
                    &mut self.entities[target].input_inventory,
                    item_id,
                    from_explicit,
                ),
                StockKind::Fuel => subtract_item(
                    &mut self.entities[target].fuel_inventory,
                    item_id,
                    from_explicit,
                ),
                StockKind::Output => subtract_item(
                    &mut self.entities[target].output_inventory,
                    item_id,
                    from_explicit,
                ),
                StockKind::Auto => {}
            }
        }
        let remainder = quantity - from_explicit;
        if remainder == 0 {
            return;
        }
        if stock == StockKind::Output
            && self.entities[target]
                .cargo
                .is_some_and(|cargo| cargo.item_id == item_id && cargo.quantity <= remainder)
        {
            self.entities[target].cargo = None;
        } else if stock != StockKind::Inventory {
            subtract_item(&mut self.entities[target].inventory, item_id, remainder);
        }
    }

    /// How much more this compartment will hold of *this item*.
    ///
    /// Ingredient and fuel compartments are bounded per item: every ingredient a recipe names gets
    /// the definition's whole capacity to itself, and so does every fuel. They used to share one
    /// undifferentiated total, which made a composer refuse its second ingredient the moment the
    /// first one filled the buffer — twelve iron plates in a twelve-capacity machine left the empty
    /// gear slot unfillable, and the only way out was to take plates back into the pack. A recipe
    /// with four ingredients could not hold a working set of any of them.
    ///
    /// The output compartment and a container's store stay one shared pool, and deliberately so: a
    /// recipe's whole batch has to fit in the former before any input is reserved, and the latter is
    /// the storage decision the player is actually making when they choose a container's tier.
    fn room_for_stock(&self, target: usize, stock: StockKind, item_id: ItemId) -> u32 {
        let entity = &self.entities[target];
        let capacity = self
            .building_definition(entity.placed.definition_id)
            .and_then(|definition| definition.capacity)
            .unwrap_or(u32::MAX);
        let held = match stock {
            StockKind::Input | StockKind::Fuel => self.stock_quantity(target, stock, item_id),
            StockKind::Inventory | StockKind::Output | StockKind::Auto => {
                self.stock_total(target, stock)
            }
        };
        capacity.saturating_sub(held)
    }

    /// Whether that capacity would hold everything this entity is holding, under exactly the rule
    /// `room_for_stock` applies: per item where a compartment is bounded per item, one pool where it
    /// is shared. Asked when a tier changes, so the two answers cannot drift apart.
    fn stock_fits_capacity(&self, index: usize, capacity: u32) -> bool {
        // Only a container is capped on `inventory` as a pool. A machine's legacy version-15 stock
        // still lives there, but it is classified into input and fuel below and capped per item.
        let shared = if self.entities[index].kind == BuildingKind::Container {
            self.stock_total(index, StockKind::Inventory)
        } else {
            0
        };
        shared.max(self.stock_total(index, StockKind::Output)) <= capacity
            && [StockKind::Input, StockKind::Fuel]
                .into_iter()
                .flat_map(|stock| self.stock_snapshot(index, stock))
                .all(|held| held.quantity <= capacity)
    }

    fn can_accept(&self, target: usize, cargo: Cargo) -> bool {
        let entity = &self.entities[target];
        if !self.accepts_item(target, cargo.item_id) {
            return false;
        }
        match entity.kind {
            // Two rules, and both of them are the conveyor rather than the bookkeeping. A hex of
            // belt holds [`BELT_LANE_SLOTS`] items because that is how many are on 5.37 m of moving
            // conveyor at once, and it will not take another until [`BELT_SLOT_TICKS`] have passed
            // since the last one stepped on, because the space behind that item has not cleared
            // yet. The second rule is what sets belt throughput; the first is what lets a blocked
            // line back up instead of stopping dead at its head, and it is derived so that it never
            // bites first — see `scale::a_belt_has_room_for_everything_in_flight_at_cadence`.
            BuildingKind::Belt => {
                entity.lane.len() + usize::from(entity.cargo.is_some()) < BELT_LANE_SLOTS
                    && entity.lane.last().is_none_or(|item| {
                        self.tick.saturating_sub(item.entered) >= BELT_SLOT_TICKS
                    })
            }
            BuildingKind::Consumer => true,
            BuildingKind::Hub => self.hub_demand(cargo.item_id) >= u64::from(cargo.quantity),
            _ => self
                .stock_kind_for_item(target, cargo.item_id)
                .is_some_and(|stock| {
                    self.room_for_stock(target, stock, cargo.item_id) >= cargo.quantity
                }),
        }
    }

    fn accept(&mut self, target: usize, cargo: Cargo) {
        match self.entities[target].kind {
            // Onto the far end of the lane, not into the hand-off slot: the item has just stepped
            // onto the belt and has the whole hex still to cross.
            BuildingKind::Belt => {
                let entered = self.tick;
                self.entities[target].lane.push(LaneItem { cargo, entered });
            }
            BuildingKind::Composer
            | BuildingKind::Container
            | BuildingKind::Generator
            | BuildingKind::Boiler => {
                if let Some(stock) = self.stock_kind_for_item(target, cargo.item_id) {
                    self.add_stock(target, stock, cargo);
                }
            }
            BuildingKind::Consumer => {
                // A consumer is a sink, not the landing hub. It records what left the factory and
                // nothing more: the contract is what the *hub* was handed, so a scenario cannot
                // finish a founding project by voiding cargo somewhere else on the map.
                self.delivered += u64::from(cargo.quantity);
                *self.delivered_by_item.entry(cargo.item_id).or_default() +=
                    u64::from(cargo.quantity);
            }
            BuildingKind::Hub => self.deliver_to_hub(cargo.item_id, cargo.quantity),
            BuildingKind::Extractor
            | BuildingKind::Pump
            | BuildingKind::Pole
            | BuildingKind::Bridge => {
                unreachable!("sources reject cargo")
            }
        }
    }

    fn deliver_to_hub(&mut self, item_id: ItemId, quantity: u32) {
        self.delivered += u64::from(quantity);
        *self.delivered_by_item.entry(item_id).or_default() += u64::from(quantity);
        self.credit_requests(item_id, quantity);
        *self.contract_contributed.entry(item_id).or_default() += u64::from(quantity);
        self.advance_contract();
    }

    fn request_definition(&self, id: RequestId) -> Option<&RequestDefinition> {
        self.definitions
            .requests
            .iter()
            .find(|request| request.id == id)
    }

    /// Put a delivery against the board, and pay for whatever it finishes.
    ///
    /// This is the only path in the game that adds insight. Before it, every hub delivery paid
    /// `insight_value × quantity` whether the hub had a use for the item or not, which meant the
    /// price of a material was a number the player could only learn by giving it away. Now the
    /// price is posted first and paid on completion — once, and only once.
    ///
    /// A filled slot is replaced in place rather than compacted out, so the row the player was
    /// reading does not jump to another slot the moment it completes. The replacement is not filled
    /// from the same delivery: it starts empty, and the next delivery is what moves it. The
    /// completed project does not come back into the draw, and when nothing is left that the player
    /// can reach the slot closes rather than reposting paid work.
    fn credit_requests(&mut self, item_id: ItemId, quantity: u32) {
        let mut remaining = quantity;
        let mut slot = 0;
        while slot < self.requests.len() && remaining > 0 {
            let Some(definition) = self
                .request_definition(self.requests[slot].request_id)
                .cloned()
            else {
                slot += 1;
                continue;
            };
            if definition.item_id != item_id {
                slot += 1;
                continue;
            }
            // A project pays once. Posting is already gated on this, so reaching it here means a
            // save was edited or a slot survived a migration it should not have — and the failure
            // mode is minting insight without bound, which is the one thing finite demand exists to
            // prevent. Cheaper to refuse it at the till than to trust every path in.
            if self.project_complete(definition.id) {
                slot += 1;
                continue;
            }
            let held = self.project_delivered(definition.id);
            let take = definition.quantity.saturating_sub(held).min(remaining);
            remaining -= take;
            let now = held + take;
            self.request_delivered.insert(definition.id, now);
            if now < definition.quantity {
                slot += 1;
                continue;
            }
            self.insight += u64::from(definition.insight);
            *self.request_rounds.entry(definition.id).or_default() += 1;
            *self.request_fills.entry(definition.id).or_default() += 1;
            // The bill is consumed by completion. Keeping the count would leave a retired project
            // reading as permanently full, and the catalogue draws its progress from this map.
            self.request_delivered.remove(&definition.id);
            self.events.push(format!(
                "{} complete — the hub pays {} insight",
                definition.name, definition.insight
            ));
            let posted = self.posted_requests(Some(slot));
            match self.next_request(&posted) {
                Some(id) => {
                    self.requests[slot] = RequestState { request_id: id };
                    slot += 1;
                }
                // Nothing left the player can reach. The slot closes rather than reposting the row
                // that was just paid for, and `refill_requests` opens it again when research does.
                None => {
                    self.requests.remove(slot);
                    if self.requests.is_empty() {
                        self.events.push(
                            "The hub has nothing further to ask for — its demand is satisfied"
                                .into(),
                        );
                    }
                }
            }
        }
        self.refill_requests();
    }

    /// How much has been handed over against one project, posted or not.
    fn project_delivered(&self, id: RequestId) -> u32 {
        self.request_delivered.get(&id).copied().unwrap_or_default()
    }

    /// Whether this project has been completed and paid. A skipped project has not.
    fn project_complete(&self, id: RequestId) -> bool {
        self.request_fills.get(&id).copied().unwrap_or_default() > 0
    }

    /// The request ids currently on the board, optionally ignoring one slot.
    fn posted_requests(&self, ignore: Option<usize>) -> BTreeSet<RequestId> {
        self.requests
            .iter()
            .enumerate()
            .filter(|&(slot, _)| Some(slot) != ignore)
            .map(|(_, state)| state.request_id)
            .collect()
    }

    /// Whether this project can be drawn into a slot: unfinished, and something the player could
    /// actually supply.
    fn request_eligible(&self, request: &RequestDefinition) -> bool {
        !self.project_complete(request.id) && self.item_reachable(request.item_id, 0)
    }

    /// The row that should be posted next: the least-used one the player can actually supply,
    /// unless the board currently holds no row at the deepest reachable depth — then that depth
    /// is reserved, so a three-slot board still leads once processing unlocks rather than cycling
    /// eight raw surveys first.
    ///
    /// A finished project is never a candidate. The catalogue is finite, so the draw order is
    /// walking a budget down rather than cycling forever, and it ends.
    ///
    /// There is no randomness here, and that is deliberate. A board that is a pure function of
    /// state is a board a save restores exactly, a checksum agrees about, and a test can walk —
    /// and one whose progression a player can learn rather than reroll. Reservation still walks
    /// `item_reachable`, so a player who cannot yet build a smelter never faces a board of three
    /// things they cannot make.
    fn next_request(&self, posted: &BTreeSet<RequestId>) -> Option<RequestId> {
        let eligible: Vec<&RequestDefinition> = self
            .definitions
            .requests
            .iter()
            .filter(|request| !posted.contains(&request.id))
            .filter(|request| self.request_eligible(request))
            .collect();
        if eligible.is_empty() {
            return None;
        }
        let max_depth = self
            .definitions
            .requests
            .iter()
            .filter(|request| self.request_eligible(request))
            .map(|request| self.item_depth(request.item_id))
            .max()
            .unwrap_or(0);
        let posted_has_max = self
            .definitions
            .requests
            .iter()
            .filter(|request| posted.contains(&request.id))
            .any(|request| self.item_depth(request.item_id) == max_depth);
        let pool = if posted_has_max {
            eligible
        } else {
            eligible
                .into_iter()
                .filter(|request| self.item_depth(request.item_id) == max_depth)
                .collect()
        };
        pool.into_iter()
            .min_by_key(|request| {
                (
                    self.request_rounds
                        .get(&request.id)
                        .copied()
                        .unwrap_or_default(),
                    request.id,
                )
            })
            .map(|request| request.id)
    }

    /// Recipe-tree depth of an item: zero for something that comes out of the ground or a source
    /// building, one plus the deepest input for a craft. The reserved board slot is this number,
    /// not catalogue order, so a plate leads a second ore assay once a smelter is unlocked.
    fn item_depth(&self, item: ItemId) -> u32 {
        self.item_depth_at(item, 0)
    }

    fn item_depth_at(&self, item: ItemId, guard: u32) -> u32 {
        if guard > MAX_RECIPE_DEPTH {
            return 0;
        }
        match self
            .reachable_recipe(item, guard)
            .or_else(|| self.definitions.production_routes(item).into_iter().next())
        {
            Some(recipe) => {
                let inner = recipe
                    .inputs
                    .iter()
                    .map(|input| self.item_depth_at(input.item_id, guard + 1))
                    .max()
                    .unwrap_or(0);
                inner + 1
            }
            None => 0,
        }
    }

    /// Post requests into every empty slot.
    fn refill_requests(&mut self) {
        let capacity = REQUEST_SLOTS.min(self.definitions.requests.len());
        while self.requests.len() < capacity {
            let posted = self.posted_requests(None);
            let Some(id) = self.next_request(&posted) else {
                return;
            };
            self.requests.push(RequestState { request_id: id });
        }
    }

    /// Whether the player could actually produce this item with what they have researched.
    ///
    /// The board is drawn against this rather than against an unlock column written by hand, so a
    /// request can never ask for something the rules do not yet allow, and a new item is gated
    /// correctly by existing. The walk is the recipe tree: every craft along it needs a machine the
    /// player may build, and every leaf needs a source they may use — water is nobody's field, so
    /// an item a building outputs directly is reachable exactly when that building is.
    fn item_reachable(&self, item: ItemId, depth: u32) -> bool {
        if depth > MAX_RECIPE_DEPTH {
            return false;
        }
        match self.reachable_recipe(item, depth) {
            Some(_) => true,
            None if !self.definitions.production_routes(item).is_empty() => false,
            None => {
                let mut sources = self
                    .definitions
                    .buildings
                    .iter()
                    .filter(|building| building.output_item_id == Some(item))
                    .peekable();
                if sources.peek().is_some() {
                    return sources.any(|building| self.technology_met(building));
                }
                // A field item the hand can take is reachable from a standing start. A field item
                // it cannot — signal crystal — is reachable once an extractor is unlocked, the
                // same way water waits on a pump.
                match self
                    .item_definition(item)
                    .and_then(|definition| definition.hand_gather_steps)
                {
                    Some(_) => true,
                    None => self.definitions.buildings.iter().any(|building| {
                        building.kind == BuildingKind::Extractor
                            && building.buildable
                            && self.technology_met(building)
                    }),
                }
            }
        }
    }

    fn recipe_unlocked(&self, recipe: &RecipeDefinition) -> bool {
        self.definitions.buildings.iter().any(|building| {
            building.buildable
                && building.supports_recipe(recipe)
                && self.technology_met(building)
                // Baseline primitive knowledge should not put gears on a brand-new player's
                // board before they have any station. Purchased industrial knowledge keeps its
                // existing eligibility rule; primitive requests appear once their station exists.
                && (building.recipe_ids.is_none() || self.entities.iter().any(|entity| entity.placed.definition_id == building.id))
        })
    }

    fn technology_met(&self, building: &BuildingDefinition) -> bool {
        match building.unlock_technology_id {
            Some(id) => self.researched.contains(&id),
            None => true,
        }
    }

    /// How much of one item the landing hub still has a use for: what the posted requests are
    /// short, plus what the founding contract has not been given yet.
    ///
    /// The contract half counts every remaining stage rather than only the current one, which is
    /// what keeps the v0.18 surplus rule true — a player who automates a line early is still
    /// credited when the stage that wants it arrives. What the hub does *not* want, it no longer
    /// takes: an item nobody asked for used to vanish into the hub for a coin of insight, and the
    /// player had no way to see that happening.
    fn hub_demand(&self, item: ItemId) -> u64 {
        let posted: u64 = self
            .requests
            .iter()
            .filter_map(|state| {
                self.request_definition(state.request_id)
                    .map(|definition| (definition, state))
            })
            .filter(|(definition, _)| definition.item_id == item)
            .map(|(definition, _)| {
                u64::from(
                    definition
                        .quantity
                        .saturating_sub(self.project_delivered(definition.id)),
                )
            })
            .sum();
        let billed: u64 = self
            .scenario
            .contract
            .stages
            .get(self.contract_stage..)
            .unwrap_or_default()
            .iter()
            .flat_map(|stage| stage.requirements.iter())
            .filter(|need| need.item_id == item)
            .map(|need| u64::from(need.quantity))
            .sum();
        let held = self
            .contract_contributed
            .get(&item)
            .copied()
            .unwrap_or_default();
        posted + billed.saturating_sub(held)
    }

    /// Pass on a posted request, so another takes its slot.
    ///
    /// Without this the board is a trap rather than an offer: three materials the player has not
    /// found yet would hold every slot, and the only source of insight in the game with them.
    /// Passing costs the row one place in the draw order — it comes round again behind everything
    /// not yet seen.
    ///
    /// It no longer forfeits what has been delivered against the row. That forfeit was affordable
    /// when a row could be filled again for the same price; under finite demand it would destroy
    /// goods whose reward can never be re-earned, turning an offer to look at something else into a
    /// trap of its own. Progress lives in `request_delivered` and waits for the project to come
    /// back.
    fn skip_request(&mut self, slot: usize) -> Result<(), String> {
        let state = *self
            .requests
            .get(slot)
            .ok_or("no request is posted in that slot")?;
        let name = self
            .request_definition(state.request_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| format!("request {}", state.request_id));
        // The round is counted first, because what is being passed on is still a candidate for the
        // slot it is leaving — and it must not win it back while anything less used is waiting.
        let rounds = self.request_rounds.entry(state.request_id).or_default();
        *rounds += 1;
        let posted = self.posted_requests(Some(slot));
        let Some(id) = self.next_request(&posted) else {
            *self.request_rounds.entry(state.request_id).or_default() -= 1;
            return Err("the hub has nothing else to ask for".into());
        };
        self.requests[slot] = RequestState { request_id: id };
        self.events.push(format!("Passed on {name}"));
        Ok(())
    }

    /// Put one named project on the board, in place of whichever posted row the player is least
    /// committed to.
    ///
    /// This is what makes a finite catalogue browsable rather than a lottery. The draw order is a
    /// good default and a bad constraint: once each project pays only once, "the row I need is not
    /// posted" is no longer a wait, it is a route the player cannot take. Choosing costs the
    /// displaced row nothing — its progress persists like any other — and the chosen project keeps
    /// whatever it had already been given.
    ///
    /// The displaced slot is the posted row with the least delivered against it, ties broken by
    /// slot, so asking for a project never silently unposts the one being worked on.
    fn post_request(&mut self, request_id: RequestId) -> Result<(), String> {
        let definition = self
            .request_definition(request_id)
            .ok_or_else(|| format!("no project {request_id}"))?
            .clone();
        if self.project_complete(definition.id) {
            return Err(format!("{} is already complete", definition.name));
        }
        if !self.item_reachable(definition.item_id, 0) {
            return Err(format!(
                "{} asks for something you cannot make yet",
                definition.name
            ));
        }
        if let Some(slot) = self
            .requests
            .iter()
            .position(|state| state.request_id == definition.id)
        {
            // Already posted. Saying so is a better answer than moving it to another slot.
            let _ = slot;
            return Err(format!("{} is already on the board", definition.name));
        }
        let target = self
            .requests
            .iter()
            .enumerate()
            .min_by_key(|(slot, state)| (self.project_delivered(state.request_id), *slot))
            .map(|(slot, _)| slot);
        match target {
            Some(slot) => {
                let displaced = self.requests[slot].request_id;
                // The displaced row leaves the board the same way a pass leaves it: one place back
                // in the draw order, its progress intact.
                *self.request_rounds.entry(displaced).or_default() += 1;
                self.requests[slot] = RequestState {
                    request_id: definition.id,
                };
            }
            None => self.requests.push(RequestState {
                request_id: definition.id,
            }),
        }
        self.events
            .push(format!("{} posted to the board", definition.name));
        Ok(())
    }

    /// Close every stage the hub can now afford, in order.
    ///
    /// The loop is not decoration: contributions carry forward, so a stage whose bill a previous
    /// surplus already covers must complete in the same delivery rather than wait for one more
    /// item to arrive and re-ask the question.
    fn advance_contract(&mut self) {
        self.advance_contract_with_rewards(true);
    }

    fn advance_contract_with_rewards(&mut self, award_skill_points: bool) {
        while let Some(stage) = self.scenario.contract.stages.get(self.contract_stage) {
            let met = stage.requirements.iter().all(|need| {
                self.contract_contributed
                    .get(&need.item_id)
                    .copied()
                    .unwrap_or(0)
                    >= u64::from(need.quantity)
            });
            if !met {
                return;
            }
            let consumed = stage.requirements.clone();
            let name = stage.name.clone();
            let key = stage.key.clone();
            for need in &consumed {
                let held = self.contract_contributed.entry(need.item_id).or_default();
                *held = held.saturating_sub(u64::from(need.quantity));
            }
            self.contract_stage += 1;
            self.events
                .push(format!("{name} complete — the landing hub grows"));
            self.grant_contract_stage(&key);
            if award_skill_points {
                self.observe_skill_event(SkillEvent::ContractStage { key });
            }
            if self.contract_stage >= self.scenario.contract.stages.len() {
                self.victory = true;
                self.events
                    .push("Founding contract complete — free play continues".into());
            }
        }
    }

    /// The host's own movement intent — a key going down, or coming back up.
    ///
    /// Any such command cancels an autonomous walk, including the zero one a key release sends. The
    /// moment the player touches the controls they are driving, and a walk that kept steering
    /// against them would be fighting for the same two numbers. This is why the walk writes
    /// `move_x`/`move_y` directly in [`Core::steer_walk`] rather than calling through here: the
    /// command path is the *cancellation* path, and routing the walk through it would cancel it on
    /// its own first step.
    fn set_move_intent(&mut self, x: i16, y: i16) -> Result<(), String> {
        if !(-1000..=1000).contains(&x) || !(-1000..=1000).contains(&y) {
            return Err("movement intent must be in -1000..1000".into());
        }
        self.clear_walk();
        self.player.move_x = x;
        self.player.move_y = y;
        if x != 0 || y != 0 {
            self.player.facing_x = x;
            self.player.facing_y = y;
        }
        Ok(())
    }

    /// Start walking to a hex, resolving the route here and now.
    ///
    /// A refusal is an event rather than a silent no-op: the player pointed at something, and
    /// "there is no way there" is the answer to what they asked.
    fn walk_to(&mut self, q: i32, r: i32) -> Result<(), String> {
        let here = world_to_axial(self.player.x, self.player.y);
        if (q, r) == here {
            // Already standing on it. Cancelling any walk in flight is the useful reading of a
            // click on your own feet, and it costs no search to answer.
            self.clear_walk();
            return Ok(());
        }
        if axial_distance(here, (q, r)) > MAX_WALK_DISTANCE {
            self.clear_walk();
            return Err("That is too far to walk to in one go".into());
        }
        let Some(path) = self.walk_route(here, (q, r)) else {
            self.clear_walk();
            return Err("No way through to there".into());
        };
        self.player.walk_goal = Some(Coordinate { q, r });
        self.walk_path = path;
        self.walk_stall = 0;
        self.walk_last_position = (self.player.x, self.player.y);
        Ok(())
    }

    /// Stop walking, wherever the walk had got to. Idempotent, and it drops the intent it was
    /// holding so a cancelled walk does not leave the player drifting.
    fn clear_walk(&mut self) {
        if self.player.walk_goal.is_none() {
            return;
        }
        self.player.walk_goal = None;
        self.walk_path.clear();
        self.walk_stall = 0;
        self.player.move_x = 0;
        self.player.move_y = 0;
    }

    /// Rebuild the route to the standing goal against the world as it now is.
    ///
    /// Called from [`Core::rebuild_runtime_index`], which every edit and every load funnels through,
    /// so a wall built across the player's own route is answered the moment it is built rather than
    /// when they arrive at it. That matters as much for the drawing as for the walking: the ribbon
    /// on screen is this path, and a path through a building the player just placed would be the
    /// host promising a walk the simulation will not take.
    ///
    /// A route that no longer exists empties the path and leaves the goal standing. Ending the walk
    /// is [`Core::steer_walk`]'s job alone, for two reasons: it keeps one place deciding what a
    /// finished walk means, and this runs inside a load's `compile_graph`, where clearing a goal the
    /// file recorded would move the checksum out from under the very check that is about to verify
    /// it.
    fn replan_walk(&mut self) {
        let Some(goal) = self.player.walk_goal else {
            return;
        };
        let here = world_to_axial(self.player.x, self.player.y);
        self.walk_path = self.walk_route(here, (goal.q, goal.r)).unwrap_or_default();
        self.walk_stall = 0;
    }

    /// Whether the player's body can stand at the centre of this hex.
    ///
    /// The centre, and not the whole hex: an adjacent pair of walkable centres is 1774 apart, and
    /// the nearest a blocking building's centre can sit to the segment between them is 1536 — both
    /// comfortably clear of the 1270 that `PLAYER_RADIUS + BUILDING_RADIUS` needs. So a route made
    /// of hex centres is one the continuous collision in `player_blocked` will actually let the
    /// player walk, without the route having to model the body it is routing.
    ///
    /// It asks `terrain_at`, which is a pure function of the world parameters and the seed, and
    /// `runtime.occupied`, which is maintained with the compiled topology. Neither generates a
    /// chunk — deliberately. `generated_chunks` is a checksum input, so a search that surveyed the
    /// ground it considered would make *thinking about* a route change the run's checksum.
    fn walkable_hex(&self, q: i32, r: i32) -> bool {
        if self.terrain_blocks_movement(q, r) {
            return false;
        }
        !self
            .runtime
            .occupied
            .get(&(q, r))
            .map(|&index| {
                self.entities
                    .get(index)
                    .and_then(|entity| self.building_definition(entity.placed.definition_id))
                    .map(|definition| definition.blocks_movement)
                    .unwrap_or(true)
            })
            .unwrap_or(false)
    }

    /// What entering this hex from `from` costs the route, in hundredths of a dry-ground hex.
    ///
    /// Three things, and each of them is something `player_step` or `advance_player` actually does.
    /// A ford is a fifth speed, so it is priced at five hexes; a prepared surface is faster, so it is
    /// priced at less than one; a step up is real work, so it costs extra. Charging the route for
    /// anything the walk does not pay, or failing to charge it for something the walk does, produces
    /// a route that is short on the map and slow in the hand.
    ///
    /// The surface does not modify the ford. Shallows are a 5 m/s crawl in `player_step` regardless,
    /// and pretending a decked river bank crosses faster would be the search inventing a preference.
    fn walk_step_cost(&self, from: (i32, i32), q: i32, r: i32) -> u32 {
        let base = if self.shallow_water_at(q, r) {
            WALK_SHALLOW_COST
        } else {
            WALK_STEP_COST * UNTREATED_MOVEMENT / self.movement_factor_at(q, r)
        };
        let climb = (self.ground_elevation_at(q, r) - self.ground_elevation_at(from.0, from.1))
            .max(0) as u32;
        base + climb * WALK_CLIMB_COST
    }

    /// A* over hex centres, returning the cells still to be walked — nearest first, ending on the
    /// goal — or `None` when there is no route inside the bounds.
    ///
    /// Read-only and integer-only, so it is as reproducible as everything else the checksum covers.
    /// Ties break on `(f, g, q, r)`, which is a total order over distinct cells, so the frontier
    /// never depends on how a heap happened to order two equal keys.
    ///
    /// Three separate bounds hold it: the goal must be within `MAX_WALK_DISTANCE`, the frontier
    /// never leaves that disc around the *start*, and `MAX_WALK_SEARCH_NODES` caps the settle count
    /// for the case the bounds cannot help with — a goal that is reachable-looking and walled off,
    /// where an unbounded search would sweep the whole disc before admitting it.
    fn walk_route(&self, from: (i32, i32), goal: (i32, i32)) -> Option<Vec<Coordinate>> {
        if from == goal {
            return Some(Vec::new());
        }
        if axial_distance(from, goal) > MAX_WALK_DISTANCE || !self.walkable_hex(goal.0, goal.1) {
            return None;
        }

        let mut open: BinaryHeap<Reverse<(u32, u32, i32, i32)>> = BinaryHeap::new();
        let mut best: BTreeMap<(i32, i32), u32> = BTreeMap::new();
        let mut came_from: BTreeMap<(i32, i32), (i32, i32)> = BTreeMap::new();
        best.insert(from, 0);
        open.push(Reverse((
            axial_distance(from, goal) as u32 * MIN_WALK_STEP_COST,
            0,
            from.0,
            from.1,
        )));

        let mut settled = 0usize;
        while let Some(Reverse((_, cost, q, r))) = open.pop() {
            let cell = (q, r);
            if cell == goal {
                return self.walk_path_from(&came_from, from, goal);
            }
            // A cheaper way here was found after this entry was pushed; the heap keeps no
            // decrease-key, so the stale entry is simply skipped.
            if best.get(&cell).copied().is_some_and(|known| known < cost) {
                continue;
            }
            settled += 1;
            if settled > MAX_WALK_SEARCH_NODES {
                return None;
            }
            for (dq, dr) in DIRECTIONS {
                let next = (q.saturating_add(dq), r.saturating_add(dr));
                if axial_distance(from, next) > MAX_WALK_DISTANCE
                    || !self.walkable_hex(next.0, next.1)
                    || self.grade_blocks(cell, next)
                    || self.boundary_blocks_segment(axial_world(q, r), axial_world(next.0, next.1))
                {
                    continue;
                }
                let step = cost + self.walk_step_cost(cell, next.0, next.1);
                if best.get(&next).copied().is_some_and(|known| known <= step) {
                    continue;
                }
                best.insert(next, step);
                came_from.insert(next, cell);
                open.push(Reverse((
                    step + axial_distance(next, goal) as u32 * MIN_WALK_STEP_COST,
                    step,
                    next.0,
                    next.1,
                )));
            }
        }
        None
    }

    /// Walk the predecessor map back from the goal, dropping the cell the player is already on.
    ///
    /// A route longer than `MAX_WALK_PATH_CELLS` is reported as no route at all. It is not a real
    /// shape — inside a 96-hex disc it would have to double back on itself for hundreds of cells —
    /// and the alternative is an unbounded list crossing the wire every frame of the walk.
    fn walk_path_from(
        &self,
        came_from: &BTreeMap<(i32, i32), (i32, i32)>,
        from: (i32, i32),
        goal: (i32, i32),
    ) -> Option<Vec<Coordinate>> {
        let mut path = Vec::new();
        let mut cursor = goal;
        while cursor != from {
            path.push(Coordinate {
                q: cursor.0,
                r: cursor.1,
            });
            if path.len() > MAX_WALK_PATH_CELLS {
                return None;
            }
            cursor = *came_from.get(&cursor)?;
        }
        path.reverse();
        Some(path)
    }

    /// One player-clock step of an autonomous walk: retire the waypoints reached, then aim at the
    /// next one. Writes the intent directly, for the reason [`Core::set_move_intent`] explains.
    fn steer_walk(&mut self) {
        if self.player.walk_goal.is_none() {
            return;
        }
        let (px, py) = (self.player.x, self.player.y);

        // Ground actually covered, not intent issued. A walk pressed against something the route
        // did not predict gets a second to slide out of it and is then handed back to the player,
        // rather than jogging into a wall until they take the controls themselves.
        if (px, py) == self.walk_last_position {
            self.walk_stall += 1;
            if self.walk_stall >= WALK_STALL_STEPS {
                self.clear_walk();
                self.events.push("Stopped — the way is blocked".into());
                return;
            }
        } else {
            self.walk_stall = 0;
        }
        self.walk_last_position = (px, py);

        let reach = i64::from(WALK_ARRIVE_RADIUS).pow(2);
        while let Some(&next) = self.walk_path.first() {
            let (wx, wy) = axial_world(next.q, next.r);
            if squared_distance(px, py, wx, wy) > reach {
                break;
            }
            self.walk_path.remove(0);
        }
        let Some(&target) = self.walk_path.first() else {
            // The goal is the last waypoint, so a route that has run out is either arrival or a
            // route `replan_walk` could not rebuild. Standing on the goal tells the two apart —
            // `WALK_ARRIVE_RADIUS` is the inradius, so being inside it means being in that hex —
            // and only the second is worth saying out loud.
            let here = world_to_axial(px, py);
            let blocked = self.player.walk_goal
                != Some(Coordinate {
                    q: here.0,
                    r: here.1,
                });
            self.clear_walk();
            if blocked {
                self.events.push("The way there is blocked".into());
            }
            return;
        };

        let (tx, ty) = axial_world(target.q, target.r);
        let dx = i64::from(tx) - i64::from(px);
        let dy = i64::from(ty) - i64::from(py);
        let length = integer_sqrt(dx * dx + dy * dy);
        if length == 0 {
            return;
        }
        self.player.move_x = (dx * i64::from(AUTO_WALK_INTENT) / length) as i16;
        self.player.move_y = (dy * i64::from(AUTO_WALK_INTENT) / length) as i16;
        self.player.facing_x = self.player.move_x;
        self.player.facing_y = self.player.move_y;
    }

    /// Face the world position the host is pointing at, resolved here in integer arithmetic so the
    /// checksummed facing vector is native's answer rather than the host's.
    ///
    /// [`Core::set_move_intent`] still sets facing, and an aim wins by arriving later in the same
    /// batch — which is what lets a touch layout that sends no aim keep facing the way it walks,
    /// without a stored aiming mode that the save format and the checksum would then have to carry.
    fn set_aim(&mut self, x: i32, y: i32) -> Result<(), String> {
        let dx = i64::from(x) - i64::from(self.player.x);
        let dy = i64::from(y) - i64::from(self.player.y);
        if dx.abs() > MAX_AIM_DISTANCE || dy.abs() > MAX_AIM_DISTANCE {
            return Err("aim target is out of range".into());
        }
        // The cursor resting exactly on the player names no direction, so the last one stands.
        let length = integer_sqrt(dx * dx + dy * dy);
        if length == 0 {
            return Ok(());
        }
        self.player.facing_x = (dx * 1000 / length) as i16;
        self.player.facing_y = (dy * 1000 / length) as i16;
        Ok(())
    }

    fn advance_player(&mut self) {
        let (dx, dy) = self.player_step();
        if dx == 0 && dy == 0 {
            return;
        }
        self.ensure_neighborhood(self.player.x + dx, self.player.y + dy);
        let next_x = self.player.x + dx;
        if !self.player_blocked(next_x, self.player.y) {
            self.player.x = next_x;
        }
        let next_y = self.player.y + dy;
        if !self.player_blocked(self.player.x, next_y) {
            self.player.y = next_y;
        }
    }

    /// One player-clock step, in world units. Land uses the host's intent against `PLAYER_SPEED`,
    /// scaled by the surface underfoot — the same integer percentage the route search prices, so
    /// the road that looked faster on the map is the road that is faster in the hand.
    /// Shallows are a 5 m/s ford: walk and run collapse to the same crawl, so holding Shift in a
    /// river does not buy a faster crossing, and neither does decking its bank.
    fn player_step(&self) -> (i32, i32) {
        let mut intent_x = self.player.move_x;
        let mut intent_y = self.player.move_y;
        let (q, r) = world_to_axial(self.player.x, self.player.y);
        let mut speed =
            PLAYER_SPEED * self.movement_factor_at(q, r) as i32 / UNTREATED_MOVEMENT as i32;
        if self.in_or_entering_shallows() {
            speed = PLAYER_SPEED / 5;
            let diagonal = intent_x != 0 && intent_y != 0;
            let magnitude = if diagonal { 707 } else { 1000 };
            intent_x = intent_x.signum() * magnitude;
            intent_y = intent_y.signum() * magnitude;
        }
        (
            i32::from(intent_x) * speed / 1000,
            i32::from(intent_y) * speed / 1000,
        )
    }

    fn in_or_entering_shallows(&self) -> bool {
        let dx = i32::from(self.player.move_x) * PLAYER_SPEED / 1000;
        let dy = i32::from(self.player.move_y) * PLAYER_SPEED / 1000;
        self.shallows_at(self.player.x, self.player.y)
            || self.shallows_at(self.player.x + dx, self.player.y)
            || self.shallows_at(self.player.x, self.player.y + dy)
    }

    fn shallows_at(&self, x: i32, y: i32) -> bool {
        let (q, r) = world_to_axial(x, y);
        self.shallow_water_at(q, r)
    }

    /// The wading half of the one native water predicate. Physical water is shallow by depth,
    /// including a flood on a meadow and excluding a drained river band; the legacy fixture keeps
    /// its presentation-only rule.
    fn shallow_water_at(&self, q: i32, r: i32) -> bool {
        if !self.ground_is_physical() {
            return self.terrain_at(q, r) == Terrain::ShallowWater;
        }
        let depth = self.water_depth_at(q, r);
        depth > 0 && depth < scale::WADE_LIMIT_QUANTA
    }

    fn player_blocked(&self, x: i32, y: i32) -> bool {
        let (q, r) = world_to_axial(x, y);
        let feature_collision = self.terrain_blocks_movement(q, r);
        feature_collision
            // A retaining face stops the body exactly where it stops the route. The step is measured
            // between the hex being left and the hex being entered, so standing still on a terrace
            // is always legal and only the crossing is refused.
            || self.grade_blocks(world_to_axial(self.player.x, self.player.y), (q, r))
            || self.boundary_blocks_player(x, y)
            || self.boundary_blocks_segment((self.player.x, self.player.y), (x, y))
            || self.entities.iter().any(|entity| {
                self.building_definition(entity.placed.definition_id)
                    .map(|definition| definition.blocks_movement)
                    .unwrap_or(true)
                    && self.entity_footprint(entity).iter().any(|cell| {
                        let (building_x, building_y) = axial_world(cell.q, cell.r);
                        circles_overlap(
                            x,
                            y,
                            PLAYER_RADIUS,
                            building_x,
                            building_y,
                            BUILDING_RADIUS,
                        )
                    })
            })
    }

    fn gather(&mut self) -> Result<(), String> {
        if self.player.action_cooldown > 0 {
            return Err("action cooling down".into());
        }
        self.ensure_neighborhood(self.player.x, self.player.y);
        let (player_q, player_r) = world_to_axial(self.player.x, self.player.y);
        let origin = (player_q, player_r);
        if let Some(pos) = self
            .ground_items
            .iter()
            .position(|item| axial_distance(origin, (item.q, item.r)) <= EXTRACT_RADIUS)
        {
            let item_id = self.ground_items[pos].item_id;
            let room = self.player_room_for(item_id);
            if room == 0 {
                return Err("carrying capacity is full".into());
            }
            let quantity = self.ground_items[pos].quantity.min(room);
            *self.player.inventory.entry(item_id).or_default() += quantity;
            let name = self
                .item_definition(item_id)
                .map(|definition| definition.name.clone())
                .unwrap_or_else(|| format!("item {item_id}"));
            self.events.push(format!("Picked up {quantity} × {name}"));
            if quantity == self.ground_items[pos].quantity {
                self.ground_items.remove(pos);
            } else {
                self.ground_items[pos].quantity -= quantity;
            }
            self.dirty.ground_items = true;
            return Ok(());
        }
        // The same question placement and every extractor ask — the field cells the player's own
        // hex covers, nearest first — so a gather can never reach a cell an extractor standing
        // here could not. Facing is deliberately not part of it. Nothing on screen shows which way
        // the player points, so weighting the target by facing drained a neighbour's number while
        // the hex underfoot stayed full, which is a change the player cannot connect to an action.
        let key = self
            .resource_at_world(self.player.x, self.player.y)
            .ok_or("stand on or beside a field hex to gather")?;
        self.gather_from(key)
    }

    /// Harvest one named hex rather than whichever the nearest-first order picks.
    ///
    /// This is the argument the facing invariant asked for, and it is a different argument.
    /// Facing-weighted targeting was refused because *where the mouse rests* is not something a
    /// player reads as aiming at a hex, so the harvest moved to a neighbour with no visible cause.
    /// A right-click **is** the cause: the player named that hex, on screen, deliberately. So the
    /// target is explicit and the reach is unchanged — `field_covered_at` at the player's own
    /// reach, the same predicate placement and every extractor use, so a right-click can still
    /// never take from a cell an extractor standing here could not.
    fn gather_at(&mut self, q: i32, r: i32) -> Result<(), String> {
        if self.player.action_cooldown > 0 {
            return Err("action cooling down".into());
        }
        self.ensure_neighborhood(self.player.x, self.player.y);
        let origin = world_to_axial(self.player.x, self.player.y);
        if axial_distance(origin, (q, r)) > EXTRACT_RADIUS {
            return Err("that hex is out of reach".into());
        }
        if let Some(pos) = self
            .ground_items
            .iter()
            .position(|item| item.q == q && item.r == r)
        {
            let item_id = self.ground_items[pos].item_id;
            let room = self.player_room_for(item_id);
            if room == 0 {
                return Err("carrying capacity is full".into());
            }
            let quantity = self.ground_items[pos].quantity.min(room);
            *self.player.inventory.entry(item_id).or_default() += quantity;
            let name = self
                .item_definition(item_id)
                .map(|definition| definition.name.clone())
                .unwrap_or_else(|| format!("item {item_id}"));
            self.events.push(format!("Picked up {quantity} × {name}"));
            if quantity == self.ground_items[pos].quantity {
                self.ground_items.remove(pos);
            } else {
                self.ground_items[pos].quantity -= quantity;
            }
            self.dirty.ground_items = true;
            return Ok(());
        }
        if !self.field_covered_at(origin, (q, r), EXTRACT_RADIUS) {
            return Err("that hex is out of reach".into());
        }
        self.gather_from((q, r))
    }

    /// Start working a field cell that has already been resolved and range-checked. Both gathers
    /// land here, so the work a material costs, the carrying rule, and the refusals are one
    /// implementation and cannot drift apart.
    ///
    /// Nothing is taken here. The swing is armed and `finish_gather` pays it out when the player's
    /// clock has actually spent the work — the deposit counts down and the item appears together,
    /// at the end, which is the only moment either of them is true.
    fn gather_from(&mut self, key: (i32, i32)) -> Result<(), String> {
        let (_, steps) = self.gather_check(key)?;
        self.player.action_cooldown = steps;
        self.last_action_cooldown_total = steps;
        self.pending_gather = Some(Coordinate { q: key.0, r: key.1 });
        Ok(())
    }

    /// Everything that has to hold for one unit to come out of a field cell: it is a field, it
    /// still holds stock, the hand can work that material at all, and there is room to carry what
    /// comes back. Answered twice for every harvest — once when the swing starts, so a refusal is
    /// immediate and says why, and once when it lands, because a swing takes real time and the
    /// world may have moved under it.
    fn gather_check(&self, key: (i32, i32)) -> Result<(ItemId, u32), String> {
        let field = self
            .field_at(key.0, key.1)
            .ok_or("stand on or beside a field hex to gather")?;
        // `resource_at_world` filters empty cells for the untargeted gather, but a named hex has
        // not been through that filter — and an empty one would underflow the subtraction the
        // payout makes.
        if self.deposit_quantity(key) == 0 {
            return Err("this deposit is worked out".into());
        }
        if self.player_room_for(field.item_id) == 0 {
            return Err("carrying capacity is full".into());
        }
        let steps = self
            .item_definition(field.item_id)
            .and_then(|item| item.hand_gather_steps)
            .ok_or_else(|| {
                format!(
                    "{} cannot be gathered by hand — place an extractor on the field",
                    self.item_name(field.item_id)
                )
            })?;
        Ok((field.item_id, steps))
    }

    /// The swing lands: one unit leaves the deposit and enters the pack, in the same step.
    ///
    /// It asks again what it asked when the swing started, because the work took real time: the
    /// cell may have run out under an extractor, the pack may have filled from an erase refund, and
    /// the player may have walked off the hex they were working. Reach is the same predicate the
    /// start used, so a swing can never land on a cell an extractor standing here could not reach —
    /// walking away cancels the harvest rather than dragging it along.
    ///
    /// A swing that no longer holds pays nothing and says nothing. The refusal for a harvest the
    /// player can still start is the one they get when they start it, and the ring already showed
    /// them the work; a toast at the end of it would be an error message for an action they had
    /// already stopped taking.
    fn finish_gather(&mut self) {
        let Some(target) = self.pending_gather.take() else {
            return;
        };
        let key = (target.q, target.r);
        let origin = world_to_axial(self.player.x, self.player.y);
        if !self.field_covered_at(origin, key, EXTRACT_RADIUS) {
            return;
        }
        if self.gather_check(key).is_err() {
            return;
        }
        let Some(field) = self.field_at(key.0, key.1) else {
            return;
        };
        let remaining = self.deposit_quantity(key) - 1;
        self.write_overlay(
            key.0,
            key.1,
            field.item_id,
            remaining,
            field.initial_quantity,
        );
        self.dirty.resources.push(key);
        *self.player.inventory.entry(field.item_id).or_default() += 1;
        // Named, not numbered. "Gathered item 6" was serviceable when the world held three items;
        // against a material base of twenty-three it tells the player nothing they can act on.
        self.events
            .push(format!("Gathered {}", self.item_name(field.item_id)));
        if remaining == 0 {
            // Any extractor covering this deposit may now report a different status.
            self.mark_all_entities_dirty();
            self.events.push("Deposit depleted".into());
        }
    }

    #[allow(dead_code)]
    fn deposit_inventory(&mut self) -> Result<(), String> {
        self.deposit_item(None)
    }

    fn deposit_item(&mut self, target_item: Option<ItemId>) -> Result<(), String> {
        let hub = self
            .entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Hub);
        let Some(hub) = hub else {
            return Err("this scenario has no landing hub".into());
        };
        if !self.within_hex_range_of_entity(hub, HUB_REACH_HEXES) {
            return Err("move beside the landing hub to deliver".into());
        }
        if self.player.inventory.is_empty() {
            return Err("inventory is empty".into());
        }
        if let Some(target) = target_item {
            if !self.player.inventory.contains_key(&target) {
                return Err("you are not carrying that item".into());
            }
        }
        // Only what the hub is actually asking for, and only as much of it as is still wanted. If
        // a target item was specified, deliver only that item; otherwise deliver all demanded items.
        let cargo: Vec<(ItemId, u32)> = self
            .player
            .inventory
            .iter()
            .filter(|(&item, _)| target_item.map_or(true, |target| item == target))
            .map(|(&item, &carried)| (item, self.hub_demand(item).min(u64::from(carried)) as u32))
            .filter(|&(_, quantity)| quantity > 0)
            .collect();
        if cargo.is_empty() {
            if target_item.is_some() {
                return Err("the landing hub is not asking for that item".into());
            }
            return Err("the landing hub is not asking for anything you carry".into());
        }
        let handed: u32 = cargo.iter().map(|&(_, quantity)| quantity).sum();
        self.events
            .push(format!("Delivered {handed} to the landing hub"));
        for (item, quantity) in cargo {
            let carried = self.player.inventory.entry(item).or_default();
            *carried -= quantity;
            if *carried == 0 {
                self.player.inventory.remove(&item);
            }
            self.deliver_to_hub(item, quantity);
        }
        Ok(())
    }

    /// Mark every technology this completed stage grants. Insight is not charged, and a
    /// technology already researched is left untouched so a legacy factory that bought the
    /// same unlock is neither refunded nor double-granted.
    fn grant_contract_stage(&mut self, stage_key: &str) {
        let granted: Vec<(TechnologyId, String)> = self
            .technologies
            .technologies
            .iter()
            .filter(|technology| {
                matches!(
                    &technology.grant,
                    TechnologyGrant::ContractStage { key, .. } if key == stage_key
                ) && !self.researched.contains(&technology.id)
            })
            .map(|technology| (technology.id, technology.name.clone()))
            .collect();
        if granted.is_empty() {
            return;
        }
        for (id, name) in granted {
            self.researched.insert(id);
            self.events.push(format!("The hub grants {name}"));
        }
        self.apply_research_effects();
        self.refill_requests();
    }

    fn research_availability(&self, technology: &TechnologyDefinition) -> ResearchAvailability {
        ResearchAvailability {
            technology_id: technology.id,
            complete: self.researched.contains(&technology.id),
            missing_prerequisites: technology
                .prerequisites
                .iter()
                .copied()
                .filter(|id| !self.researched.contains(id))
                .collect(),
            insight_shortfall: u64::from(technology.cost).saturating_sub(self.insight),
        }
    }

    fn research_availability_snapshot(&self) -> Vec<ResearchAvailability> {
        self.technologies
            .technologies
            .iter()
            .map(|technology| self.research_availability(technology))
            .collect()
    }

    fn research(&mut self, technology_id: TechnologyId) -> Result<(), String> {
        let technology = self
            .technology(technology_id)
            .cloned()
            .ok_or_else(|| format!("unknown technology {technology_id}"))?;
        let availability = self.research_availability(&technology);
        if availability.complete {
            return Err("technology already researched".into());
        }
        if !technology.purchasable() {
            return Err(match &technology.grant {
                TechnologyGrant::ContractStage { name, .. } => {
                    format!("granted by completing {name}")
                }
                TechnologyGrant::Purchase => "technology cannot be purchased".into(),
            });
        }
        if !availability.missing_prerequisites.is_empty() {
            return Err("technology prerequisites are not complete".into());
        }
        if availability.insight_shortfall > 0 {
            return Err(format!("requires {} insight", technology.cost));
        }
        self.insight -= u64::from(technology.cost);
        self.researched.insert(technology_id);
        self.apply_research_effects();
        // A breakthrough can make a request reachable that was not, which matters only when the
        // board is short of a slot — the usual case is a full board that turns over on its own.
        self.refill_requests();
        self.events.push(format!("Researched {}", technology.name));
        Ok(())
    }

    fn placement_legality(
        &self,
        q: i32,
        r: i32,
        definition_id: DefinitionId,
        orientation: u8,
        recipe_id: Option<RecipeId>,
        check_cost: bool,
    ) -> Result<(), String> {
        let definition = self
            .building_definition(definition_id)
            .ok_or_else(|| format!("unknown building definition {definition_id}"))?;
        if !definition.buildable {
            return Err("this scenario object cannot be constructed".into());
        }
        if !definition.orientation_axis.allows(orientation) {
            let range = definition.orientation_axis.range();
            return Err(format!(
                "{} must be oriented in {}..{}",
                definition.name, range.start, range.end
            ));
        }
        for (gate, message) in definition.gates_at(orientation).into_iter().zip([
            "building is locked by research",
            "this heading is locked by research",
        ]) {
            if let Some(required) = gate {
                if !self.researched.contains(&required) {
                    return Err(message.into());
                }
            }
        }
        let placed = PlacedBuilding {
            q,
            r,
            definition_id,
            orientation,
            recipe_id,
            scenario_owned: false,
        };
        let footprint = self.footprint_for(placed, orientation);
        if footprint.is_empty() {
            return Err("building footprint is empty".into());
        }
        if !footprint
            .iter()
            .any(|cell| self.within_world_range(cell.q, cell.r, self.player.build_range))
        {
            return Err("placement is outside build range".into());
        }
        if self.boundary_crosses_footprint(&footprint) {
            return Err("A boundary crosses this building footprint; remove it first".into());
        }
        let envelope = self.envelope_for(placed, orientation);
        let clearance = self.clearance_for(placed, orientation);
        if self.boundary_crosses_footprint(&envelope) {
            return Err(
                "A boundary crosses this building's service envelope; remove it first".into(),
            );
        }
        for cell in &footprint {
            let supported_transport = definition.kind == BuildingKind::Belt
                && self.bridge_at(cell.q, cell.r)
                && self
                    .entity_at(cell.q, cell.r)
                    .is_some_and(|index| self.entities[index].kind == BuildingKind::Bridge);
            self.reservation_conflict(cell.q, cell.r, definition.kind, None, supported_transport)?;
            let (cell_x, cell_y) = axial_world(cell.q, cell.r);
            if circles_overlap(
                self.player.x,
                self.player.y,
                PLAYER_RADIUS,
                cell_x,
                cell_y,
                BUILDING_RADIUS,
            ) {
                return Err("the player blocks this footprint".into());
            }
            let shallow_support = definition.placement_rule == PlacementRule::Shallows
                && self.shallow_water_at(cell.q, cell.r);
            let bridged_transport = definition.kind == BuildingKind::Belt
                && self.shallow_water_at(cell.q, cell.r)
                && self.bridge_at(cell.q, cell.r);
            if self.terrain_blocks_construction(cell.q, cell.r)
                && !shallow_support
                && !bridged_transport
            {
                return Err("environment blocks construction".into());
            }
        }
        for cell in &envelope {
            self.reservation_conflict(cell.q, cell.r, definition.kind, None, false)?;
            let shallow_support = definition.placement_rule == PlacementRule::Shallows
                && self.shallow_water_at(cell.q, cell.r);
            if self.terrain_blocks_construction(cell.q, cell.r) && !shallow_support {
                return Err("environment blocks construction".into());
            }
        }
        for cell in &clearance {
            // Clearance is air: low infrastructure may already stand here, and the ground does
            // not have to be a pad. Other machines, envelopes and rotors still cannot share it.
            self.reservation_conflict(cell.q, cell.r, definition.kind, None, true)?;
            if let Some(index) = self.entity_at(cell.q, cell.r) {
                if !Self::is_low_infrastructure(self.entities[index].kind) {
                    return Err("building footprint overlaps an occupied hex".into());
                }
            }
        }
        // A footprint has to sit on ground level enough to stand a building on. Measuring the whole
        // occupied foundation's spread rather than each neighbouring pair is what makes a level pad
        // worth grading: a multi-hex machine on a hillside asks the player to prepare a site first,
        // and the site they prepare is exactly the one the preview showed them. Envelope and
        // clearance are reservations, not the pad.
        if let (Some(low), Some(high)) = (
            footprint
                .iter()
                .map(|cell| self.ground_elevation_at(cell.q, cell.r))
                .min(),
            footprint
                .iter()
                .map(|cell| self.ground_elevation_at(cell.q, cell.r))
                .max(),
        ) {
            if high - low > self.pad_step_limit(definition.foundation_class) {
                return Err("This ground is too uneven; level a pad for this footprint".into());
            }
        }
        if definition.placement_rule == PlacementRule::Resource
            && !self.extractable_deposit(definition.id, (q, r))
        {
            return Err(if let Some(item) = definition.output_item_id {
                format!(
                    "{} requires a non-empty {} deposit",
                    definition.name,
                    self.item_name(item)
                )
            } else if self
                .field_at(q, r)
                .and_then(|field| self.item_definition(field.item_id))
                .is_some_and(|item| item.extraction_building_id.is_some())
            {
                "This deposit requires an oil well, not an ordinary extractor".into()
            } else {
                "extractors require a non-empty deposit".into()
            });
        }
        let source_radius = definition.extract_radius.unwrap_or(PUMP_RADIUS as u32) as i32;
        if definition.placement_rule == PlacementRule::Water
            && !self.water_within_reach(q, r, source_radius)
        {
            return Err("must be placed beside open water".into());
        }
        if definition.placement_rule == PlacementRule::Elevated {
            let terrain = self.terrain_at(q, r);
            if !matches!(terrain, Terrain::Hills | Terrain::Highland) {
                return Err("wind turbines must stand on hills or highland".into());
            }
        }
        if definition.placement_rule == PlacementRule::Shallows && !self.shallow_water_at(q, r) {
            return Err("bridges require shallow water".into());
        }
        if definition.kind == BuildingKind::Composer {
            let id = recipe_id.ok_or("this machine requires a recipe")?;
            let recipe = self
                .recipe(id)
                .ok_or_else(|| format!("unknown recipe {id}"))?;
            // One field, one check: a kiln cannot be given a circuit recipe because the categories
            // disagree, not because there is a separate building kind for every machine.
            if !definition.supports_recipe(recipe) {
                return Err(format!(
                    "{} cannot run a {} recipe",
                    definition.name, recipe.category
                ));
            }
        }
        // Transport exists to deliver. A belt aimed at something that can never take an item is not
        // a slow belt, it is a dead one, and the old game only told the player so much later, when
        // the line silently backed up. So the question moves from delivery time to construction
        // time, and the refusal names the hex that is refusing and why.
        //
        // Only the facing is judged, and only for transport. A splitter's flanks may legitimately
        // point at anything, and a machine that happens to face a power pole is still a perfectly
        // good machine — refusing those would be hostile. A belt exists for one purpose and a drag
        // chooses its own heading, so it is the one that can be held to it.
        if definition.kind == BuildingKind::Belt {
            if let Some((target, (cell_q, cell_r))) =
                self.prospective_output(&footprint, definition, orientation)
            {
                let blocked = &self.entities[target];
                // A bridge is a support a belt may itself stand on, so a belt aimed at a bare
                // bridge hex is aimed at the belt that will stand there: not accepting *yet*,
                // rather than never.
                if never_accepts_deliveries(blocked.kind) && blocked.kind != BuildingKind::Bridge {
                    let name = self
                        .building_definition(blocked.placed.definition_id)
                        .map(|value| value.name.clone())
                        .unwrap_or_else(|| "that building".into());
                    return Err(format!(
                        "this {} would deliver into the {name} at {cell_q}, {cell_r}, which never takes items",
                        definition.name.to_lowercase()
                    ));
                }
                if !self.prospective_transport_target_compatible(definition, target) {
                    let name = self
                        .building_definition(blocked.placed.definition_id)
                        .map(|value| value.name.clone())
                        .unwrap_or_else(|| "that building".into());
                    return Err(format!(
                        "the {} and {name} carry incompatible cargo",
                        definition.name.to_lowercase()
                    ));
                }
            }
        }
        // Creative builds for free, so it is asked for nothing. Every other rule above still
        // applies — terrain, footprint, overlap, reach, orientation — because a creative layout that
        // could not be built in a priced run would be no use as a test of one.
        if check_cost && !self.creative {
            let missing: Vec<String> = definition
                .cost_at(orientation)
                .iter()
                .filter_map(|ingredient| {
                    let have = self
                        .player
                        .inventory
                        .get(&ingredient.item_id)
                        .copied()
                        .unwrap_or(0);
                    if have >= ingredient.quantity {
                        return None;
                    }
                    let name = self
                        .item_definition(ingredient.item_id)
                        .map(|item| item.name.as_str())
                        .unwrap_or("item");
                    Some(format!("{} {name} (have {have})", ingredient.quantity))
                })
                .collect();
            if !missing.is_empty() {
                return Err(format!("need {}", missing.join(" · ")));
            }
        }
        Ok(())
    }

    fn place(
        &mut self,
        q: i32,
        r: i32,
        definition_id: DefinitionId,
        orientation: u8,
        recipe_id: Option<RecipeId>,
    ) -> Result<(), String> {
        let old_links = self.graph_links_by_id();
        let (x, y) = axial_world(q, r);
        self.ensure_neighborhood(x, y);
        self.placement_legality(q, r, definition_id, orientation, recipe_id, true)?;
        let definition = self.building_definition(definition_id).unwrap().clone();
        if !self.creative {
            for ingredient in definition.cost_at(orientation) {
                subtract_item(
                    &mut self.player.inventory,
                    ingredient.item_id,
                    ingredient.quantity,
                );
            }
        }
        let id = self.next_entity_id;
        let placed = PlacedBuilding {
            q,
            r,
            definition_id,
            orientation,
            recipe_id,
            scenario_owned: false,
        };
        self.entities.push(Entity {
            id,
            placed,
            kind: definition.kind,
            cargo: None,
            inventory: BTreeMap::new(),
            input_inventory: BTreeMap::new(),
            fuel_inventory: BTreeMap::new(),
            output_inventory: BTreeMap::new(),
            reserved_inputs: BTreeMap::new(),
            progress: 0,
            fuel_charge: 0,
            power_charge: 0,
            burn_progress: 0,
            disabled: definition.manual_work,
            route_cursor: 0,
            merge_cursor: 0,
            lane: Vec::new(),
        });
        self.next_entity_id += 1;
        self.undo_stack.push(id);
        if self.undo_stack.len() > MAX_UNDO_DEPTH {
            self.undo_stack.remove(0);
        }
        self.dirty.entities.push(id);
        // A chunk's reported entity count changes with the blueprint.
        self.dirty.chunks = true;
        let changed_cells = self
            .footprint_for(placed, orientation)
            .into_iter()
            .map(|cell| (cell.q, cell.r))
            .collect();
        self.recompile_graph_components(&old_links, &changed_cells, &BTreeSet::from([id]));
        self.events.push(format!("Placed {}", definition.name));
        Ok(())
    }

    /// One drag of construction. The host sends the endpoints it dragged between and nothing else:
    /// every cell, orientation, legality result, and cost is resolved here, and each cell goes
    /// through the same `place` the single-cell command uses, so a drag can only ever produce what
    /// the equivalent individual placements would have produced.
    ///
    /// Illegal cells are skipped rather than aborting the run, so dragging a belt past a rock or
    /// past the end of the materials builds everything it legally can. The per-cell events are
    /// replaced by one summary, because ten "Placed Belt" lines is not feedback.
    fn place_line(
        &mut self,
        from: (i32, i32),
        to: (i32, i32),
        definition_id: DefinitionId,
        orientation: u8,
        recipe_id: Option<RecipeId>,
    ) -> Result<(), String> {
        let definition = self
            .building_definition(definition_id)
            .ok_or_else(|| format!("unknown building definition {definition_id}"))?;
        let routed = definition.kind == BuildingKind::Belt;
        let paired_underpass = definition.underpass_span.is_some() && from != to;
        let name = definition.name.clone();
        let cells = self.drag_route(from, to, definition_id, orientation, recipe_id);
        if paired_underpass {
            let preview = self.line_preview(from, to, definition_id, orientation, recipe_id);
            if preview.len() != 2 || preview.iter().any(|cell| !cell.legal) {
                return Err(preview
                    .iter()
                    .find_map(|cell| cell.reason.clone())
                    .unwrap_or_else(|| {
                        "both underpass portals must be clear and affordable".into()
                    }));
            }
        }
        let before = self.events.len();
        let mut placed = 0usize;
        let mut last_error = None;
        for (index, &(q, r)) in cells.iter().enumerate() {
            // A belt run points every cell at the next one, so the drag routes the line and the
            // player never orients a segment by hand. The final cell keeps the run's heading.
            let cell_orientation = if routed {
                Self::run_orientation(&cells, index, orientation)
            } else {
                orientation
            };
            match self.place(q, r, definition_id, cell_orientation, recipe_id) {
                Ok(()) => placed += 1,
                Err(error) => last_error = Some(error),
            }
        }
        self.events.truncate(before);
        match (placed, last_error) {
            (0, Some(error)) => Err(error),
            (0, None) => Err("nothing to build along that drag".into()),
            (count, reason) => {
                self.events.push(if paired_underpass && count == 2 {
                    format!("Placed {name} pair")
                } else if count == 1 {
                    format!("Placed {name}")
                } else {
                    format!("Placed {count} × {name}")
                });
                // A run that stopped short says why. Silently building four of ten is the kind of
                // thing a player notices only much later, when the line does not work.
                if let Some(reason) = reason {
                    self.events.push(format!("Run stopped: {reason}"));
                }
                Ok(())
            }
        }
    }

    /// The heading a belt takes at `index` along `cells`: toward its successor, or — for the last
    /// cell — continuing the heading it arrived on. Shared by the drag and its preview so the two
    /// cannot disagree.
    fn run_orientation(cells: &[(i32, i32)], index: usize, fallback: u8) -> u8 {
        let (q, r) = cells[index];
        match cells.get(index + 1) {
            Some(&next) => step_direction((q, r), next),
            None => index
                .checked_sub(1)
                .and_then(|previous| cells.get(previous))
                .and_then(|&previous| step_direction(previous, (q, r))),
        }
        .unwrap_or(fallback)
    }

    /// What a construction drag between these endpoints would do, without doing it. Materials are
    /// spent against a copy of the player's inventory as the run is walked, so the preview shows
    /// exactly where a run will stop for cost rather than implying the whole line is affordable.
    /// `recipe_id` travels with the preview for the same reason it travels with the drag: a
    /// machine's legality now depends on whether its recipe belongs to its category, so a preview
    /// that asked without one would refuse every cell of a run the drag would happily build.
    fn line_preview(
        &self,
        from: (i32, i32),
        to: (i32, i32),
        definition_id: DefinitionId,
        orientation: u8,
        recipe_id: Option<RecipeId>,
    ) -> Vec<LinePreviewCell> {
        let Some(definition) = self.building_definition(definition_id) else {
            return Vec::new();
        };
        let routed = definition.kind == BuildingKind::Belt;
        let definition = definition.clone();
        let cells = self.drag_route(from, to, definition_id, orientation, recipe_id);
        let mut budget = self.player.inventory.clone();
        let mut taken = BTreeSet::new();
        cells
            .iter()
            .enumerate()
            .map(|(index, &(q, r))| {
                let cell_orientation = if routed {
                    Self::run_orientation(&cells, index, orientation)
                } else {
                    orientation
                };
                // A run that turns can change price partway along it, so the budget is charged the
                // heading each cell actually takes rather than the heading the drag started at.
                let cost = definition.cost_at(cell_orientation);
                let reason = self
                    .placement_legality(q, r, definition_id, cell_orientation, recipe_id, false)
                    .err();
                let legal = !taken.contains(&(q, r))
                    && reason.is_none()
                    && (self.creative || has_ingredients(&budget, cost));
                if legal {
                    if !self.creative {
                        for ingredient in cost {
                            subtract_item(&mut budget, ingredient.item_id, ingredient.quantity);
                        }
                    }
                    taken.insert((q, r));
                }
                LinePreviewCell {
                    q,
                    r,
                    orientation: cell_orientation,
                    legal,
                    reason,
                }
            })
            .collect()
    }

    /// The path a construction drag uses. Ordinary buildings retain the exact line resolver they
    /// have always used. Belts additionally get a bounded deterministic *cheapest* path around
    /// cells on which that belt cannot be placed, so an obstacle produces a connected detour rather
    /// than a straight run with a hole in it.
    ///
    /// The search walks every heading the definition's axis allows — all twelve, for a belt that
    /// has both periods — rather than the six edges the old breadth-first version knew about. Four
    /// keys order it, in this order: what the run costs, how many belts it takes, how often it
    /// turns, and how far it strays from the straight line between the endpoints.
    ///
    /// Cost first because a detour that spends less is the one a player would have drawn. Cells
    /// second because a corner step is priced at what it covers, which leaves the two periods level
    /// on cost and lets the route that turns the same distance into fewer entities win. Turns third
    /// because two runs that cost the same and are the same length are told apart by which one
    /// staircases. Straying last, and it is what settles the ordinary case: an unobstructed drag has
    /// several equally short, equally straight routes, and the one the player was shown while
    /// dragging — and the one the reverse *erase* drag retraces, since removal still resolves by
    /// straight line — is the line itself. Every key only grows along a path, which is what makes
    /// this a shortest-path search and not a guess.
    ///
    /// Counting turns is why a search node is a cell *and* the heading it was reached on: whether a
    /// step turns is a fact about the step before it, so a cell reached along two headings is two
    /// states rather than one that has to forget how it got there.
    ///
    /// A heading whose research is not done is not offered to the search at all, so the route
    /// simply does not use it — the path a player gets widens when they unlock the two-row reach,
    /// with no separate branch here to say so.
    ///
    /// Start and destination are allowed into the route even when occupied. That preserves the
    /// useful gesture of dragging out of, or into, an existing belt: the ordinary `place` call will
    /// skip that endpoint while the neighbouring new segment still points at it. Interior cells
    /// must pass the ordinary placement predicate with cost disabled. Heading order is the explicit
    /// tie-break, and the route never exceeds `MAX_LINE_CELLS`.
    fn drag_route(
        &self,
        from: (i32, i32),
        to: (i32, i32),
        definition_id: DefinitionId,
        _orientation: u8,
        recipe_id: Option<RecipeId>,
    ) -> Vec<(i32, i32)> {
        let Some(definition) = self.building_definition(definition_id) else {
            return Vec::new();
        };
        let axis = definition.orientation_axis;
        if let Some(span) = definition.underpass_span.filter(|_| from != to) {
            // One drag places the two portals, never a carpet of underpass entities. The endpoint
            // snaps to the closest reachable heading/length, so a pointer does not need pixel-
            // perfect axial alignment; the native preview publishes the exact snapped pair.
            let target_world = axial_world(to.0, to.1);
            let best = axis
                .range()
                .filter(|&heading| {
                    definition
                        .gates_at(heading)
                        .into_iter()
                        .flatten()
                        .all(|required| self.researched.contains(&required))
                })
                .flat_map(|heading| {
                    let (dq, dr) = TRANSPORT_DIRECTIONS[usize::from(heading)];
                    (2..=span).map(move |steps| {
                        let candidate = (from.0 + dq * steps as i32, from.1 + dr * steps as i32);
                        let world = axial_world(candidate.0, candidate.1);
                        let dx = i64::from(world.0 - target_world.0);
                        let dy = i64::from(world.1 - target_world.1);
                        ((dx * dx + dy * dy, steps, heading), candidate)
                    })
                })
                .min_by_key(|(key, _)| *key)
                .map(|(_, candidate)| candidate);
            return best.map_or_else(|| vec![from], |end| vec![from, end]);
        }
        if definition.kind != BuildingKind::Belt || axis == OrientationAxis::Corner || from == to {
            return line_between(from, to, axis);
        }

        let weights: Vec<u32> = (0..TRANSPORT_DIRECTIONS.len() as u8)
            .map(|heading| {
                definition
                    .cost_at(heading)
                    .iter()
                    .map(|ingredient| ingredient.quantity)
                    .sum()
            })
            .collect();
        // Research is a fact about the heading, so it is settled once here rather than per step.
        // The per-cell predicate below would say the same thing everywhere except the destination,
        // which this search lets the route reach even when it is occupied — and occupancy is the
        // only thing that exemption was ever meant to forgive.
        let headings: Vec<u8> = axis
            .range()
            .filter(|&heading| {
                definition
                    .gates_at(heading)
                    .into_iter()
                    .flatten()
                    .all(|required| self.researched.contains(&required))
            })
            .collect();

        // The line to stay near is the one the player could actually draw, which is the one their
        // research allows: measured against a line that uses a heading no route here may take, the
        // key would push every route toward cells none of them can reach.
        let reachable = if headings.iter().any(|&heading| is_corner_heading(heading)) {
            axis
        } else {
            OrientationAxis::Edge
        };
        let line: BTreeSet<(i32, i32)> = line_between(from, to, reachable).into_iter().collect();

        // The heading a node was reached on, for the start, which turned nothing to get there.
        const UNTURNED: u8 = u8::MAX;
        // Key first, node second, so the heap orders on the key and the node only ever breaks a tie
        // — which is what keeps two equal routes from resolving differently on two machines.
        type Node = ((i32, i32), u8);
        type Key = (u32, usize, usize, usize);
        let start: Node = (from, UNTURNED);
        let mut frontier =
            BinaryHeap::from([std::cmp::Reverse(((0u32, 0usize, 0usize, 0usize), start))]);
        let mut best: BTreeMap<Node, Key> = BTreeMap::from([(start, (0, 0, 0, 0))]);
        let mut previous: BTreeMap<Node, Node> = BTreeMap::new();
        while let Some(std::cmp::Reverse((key, current))) = frontier.pop() {
            // A cheaper route to this node already left the heap, so this entry is stale.
            if best.get(&current).is_some_and(|&known| known < key) {
                continue;
            }
            let (cell, arrived_on) = current;
            if cell == to {
                let mut route = vec![to];
                let mut node = current;
                while let Some(&step) = previous.get(&node) {
                    route.push(step.0);
                    node = step;
                }
                route.reverse();
                return route;
            }
            let (spent, cells, turns, strayed) = key;
            if cells + 1 >= MAX_LINE_CELLS {
                continue;
            }
            for &heading in &headings {
                let (dq, dr) = TRANSPORT_DIRECTIONS[usize::from(heading)];
                let next: Node = ((cell.0 + dq, cell.1 + dr), heading);
                if next.0 != to
                    && self
                        .placement_legality(
                            next.0 .0,
                            next.0 .1,
                            definition_id,
                            heading,
                            recipe_id,
                            false,
                        )
                        .is_err()
                {
                    continue;
                }
                let candidate = (
                    spent + weights[usize::from(heading)],
                    cells + 1,
                    turns + usize::from(arrived_on != UNTURNED && arrived_on != heading),
                    strayed + usize::from(!line.contains(&next.0)),
                );
                if best.get(&next).is_some_and(|&known| known <= candidate) {
                    continue;
                }
                best.insert(next, candidate);
                previous.insert(next, current);
                frontier.push(std::cmp::Reverse((candidate, next)));
            }
        }

        // A destination outside the bounded legal search still gets the historical line preview,
        // including its visible refused cells, instead of disappearing from the drag entirely.
        line_between(from, to, axis)
    }

    /// What a removal drag between these endpoints would take back. Refunds accumulate against a
    /// copy of the player's inventory as the run is walked, for the same reason the construction
    /// preview spends materials against one: the cell a run stops at has to be visible before the
    /// drag is released, whether it stops for cost or for carrying space.
    fn erase_line_preview(&self, from: (i32, i32), to: (i32, i32)) -> Vec<LinePreviewCell> {
        let mut taken = BTreeSet::new();
        line_between(from, to, self.erase_line_axis(from))
            .into_iter()
            .map(|(q, r)| {
                let in_range = self.within_build_range_of_target(q, r);
                let removable = self.entity_at(q, r).filter(|&index| {
                    !self.entities[index].placed.scenario_owned
                        && !taken.contains(&self.entities[index].id)
                });
                // A full pack no longer refuses a recovery — whatever will not fit falls at the
                // site — so the preview no longer walks a running total of what the pack could
                // still take. `taken` stays: a multi-cell footprint is reached from several cells
                // of the drag, and only the first of them removes anything.
                if in_range {
                    if let Some(index) = removable {
                        taken.insert(self.entities[index].id);
                    }
                }
                let legal = in_range && removable.is_some();
                LinePreviewCell {
                    q,
                    r,
                    orientation: 0,
                    legal,
                    reason: None,
                }
            })
            .collect()
    }

    /// Which axis a removal drag walks. Erasure carries no definition to ask, so it asks the hex
    /// the drag started on: a run that begins on a two-row belt takes back the two-row column, and
    /// every other run walks the six edges exactly as it did before v0.14. Deterministic and
    /// native, like the path itself.
    ///
    /// A definition that takes every heading cannot answer this on its own — that was the one thing
    /// the riser's separate definition was carrying that the unified belt does not. The *entity's*
    /// heading carries it instead, which is the same fact in the place it actually belongs: this
    /// run is in the period the belt under the player's cursor is in.
    fn erase_line_axis(&self, from: (i32, i32)) -> OrientationAxis {
        let Some(index) = self.entity_at(from.0, from.1) else {
            return OrientationAxis::default();
        };
        let orientation = self.entities[index].placed.orientation;
        match self
            .building_definition(self.entities[index].placed.definition_id)
            .map(|definition| definition.orientation_axis)
        {
            Some(OrientationAxis::Any) if is_corner_heading(orientation) => OrientationAxis::Corner,
            Some(OrientationAxis::Any) => OrientationAxis::Edge,
            Some(axis) => axis,
            None => OrientationAxis::default(),
        }
    }

    /// One drag of removal, resolved exactly as `place_line` resolves construction.
    fn erase_line(&mut self, from: (i32, i32), to: (i32, i32)) -> Result<(), String> {
        let cells = line_between(from, to, self.erase_line_axis(from));
        let before = self.events.len();
        let mut removed = 0usize;
        let mut last_error = None;
        for &(q, r) in &cells {
            // A multi-cell footprint is reached from several cells of the drag; the first one
            // removes it and the rest simply find nothing there.
            match self.erase(q, r) {
                Ok(()) => removed += 1,
                Err(error) => last_error = Some(error),
            }
        }
        self.events.truncate(before);
        match (removed, last_error) {
            (0, Some(error)) => Err(error),
            (0, None) => Err("nothing to remove along that drag".into()),
            (count, _) => {
                // Dragging across ground that holds nothing is the normal case for erasure, so
                // unlike construction it is not worth reporting a reason for each empty cell.
                self.events.push(if count == 1 {
                    "Recovered 1 building".into()
                } else {
                    format!("Recovered {count} buildings")
                });
                Ok(())
            }
        }
    }

    /// Take back the most recent construction this session made, through the ordinary erase path so
    /// the refund is the tested one. A construction that has already been removed is skipped, and a
    /// failed undo keeps its entry so the player can walk back into range and retry.
    fn undo(&mut self) -> Result<(), String> {
        while let Some(&id) = self.undo_stack.last() {
            let Some(index) = self.index_of_entity(id) else {
                self.undo_stack.pop();
                continue;
            };
            let (q, r) = (self.entities[index].placed.q, self.entities[index].placed.r);
            let before = self.events.len();
            let result = self.erase(q, r);
            self.events.truncate(before);
            result?;
            self.undo_stack.pop();
            self.events.push("Undid the last construction".into());
            return Ok(());
        }
        Err("nothing to undo".into())
    }

    fn erase(&mut self, q: i32, r: i32) -> Result<(), String> {
        if !self.within_build_range_of_target(q, r) {
            return Err("erase target is outside build range".into());
        }
        let index = self.entity_at(q, r).ok_or("no building to erase")?;
        if self.entities[index].placed.scenario_owned {
            return Err("scenario-owned objects are protected".into());
        }
        // Construction cost and stored contents come back to the pack, and whatever will not fit
        // falls at the site as real ground items — the same treatment in-transit cargo has always
        // had. Refusing the demolition instead was the worse trade: it left the player holding a
        // full pack and a full building with no order of operations that emptied either, and the
        // building they wanted gone stayed. The host warns first and says the ground items are on a
        // timer, so the loss is a decision rather than a surprise.
        let refund = self.erase_refund(index);
        let (carried, spilled) = self.split_by_carry(&refund);
        let old_links = self.graph_links_by_id();
        let changed_cells = self
            .entity_footprint(&self.entities[index])
            .into_iter()
            .map(|cell| (cell.q, cell.r))
            .collect();
        let entity = self.entities.remove(index);
        self.deposit_links.remove(&entity.id);
        self.output_routes.remove(&entity.id);
        self.legacy_fluid_belts.remove(&entity.id);
        self.dirty.removed.insert(entity.id);
        self.dirty.chunks = true;
        let name = self
            .building_definition(entity.placed.definition_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| "building".into());
        add_inventory(&mut self.player.inventory, &carried);
        // Everything the belt was carrying, not just what had reached its far end: an item halfway
        // along a conveyor is as real as the one waiting at the end of it, and demolishing the
        // conveyor under it drops it on the ground rather than deleting it.
        for cargo in Self::belt_contents(&entity).collect::<Vec<_>>() {
            self.add_ground_item(
                entity.placed.q,
                entity.placed.r,
                cargo.item_id,
                cargo.quantity,
            );
        }
        for (&item, &quantity) in &spilled {
            self.add_ground_item(entity.placed.q, entity.placed.r, item, quantity);
        }
        self.recompile_graph_components(&old_links, &changed_cells, &BTreeSet::from([entity.id]));
        self.events.push(format!("Recovered {name}"));
        if !spilled.is_empty() {
            let total: u32 = spilled.values().sum();
            self.events.push(format!(
                "{total} items would not fit your pack and fell at the site"
            ));
        }
        Ok(())
    }

    /// Everything erasing this entity hands back: its construction cost, stored inventory, and
    /// reserved recipe inputs. In-transit cargo is deliberately absent because `erase` spills it
    /// on the ground at the removed entity's anchor.
    ///
    /// Creative recovers nothing. Building costs nothing there, so there is nothing owed back, and
    /// nothing to spill either — a creative player clearing a full factory leaves no litter behind
    /// them. One rule here covers every route: single erase, drag erase, the drag's preview, and
    /// undo.
    fn erase_refund(&self, index: usize) -> BTreeMap<ItemId, u32> {
        if self.creative {
            return BTreeMap::new();
        }
        let entity = &self.entities[index];
        let mut refund = BTreeMap::new();
        if let Some(definition) = self.building_definition(entity.placed.definition_id) {
            add_ingredients(&mut refund, definition.cost_at(entity.placed.orientation));
        }
        add_inventory(&mut refund, &entity.inventory);
        add_inventory(&mut refund, &entity.input_inventory);
        add_inventory(&mut refund, &entity.fuel_inventory);
        add_inventory(&mut refund, &entity.output_inventory);
        add_inventory(&mut refund, &entity.reserved_inputs);
        refund
    }

    /// Whether a building's stock is the player's to reach into.
    ///
    /// Every kind that keeps an `inventory` a hand could sensibly hold: a box, and the three
    /// machines that stand around holding fuel and inputs. A belt's cargo is a position on a lane
    /// rather than a store, the hub's intake is the contract, and an extractor, pole, or bridge
    /// keeps nothing — so those are refused rather than silently doing nothing.
    fn stock_is_reachable_by_hand(kind: BuildingKind) -> bool {
        matches!(
            kind,
            BuildingKind::Extractor
                | BuildingKind::Pump
                | BuildingKind::Container
                | BuildingKind::Composer
                | BuildingKind::Generator
                | BuildingKind::Boiler
        )
    }

    /// Resolve the building a hand transfer names, at the range every other edit is held to.
    fn hand_transfer_target(&self, q: i32, r: i32, verb: &str) -> Result<usize, String> {
        if !self.within_build_range_of_target(q, r) {
            return Err(format!("{verb} target is outside build range"));
        }
        let index = self.entity_at(q, r).ok_or("nothing to reach into there")?;
        if !Self::stock_is_reachable_by_hand(self.entities[index].kind) {
            return Err("that building has no stock you can reach".into());
        }
        Ok(index)
    }

    /// Move stock out of a building and into the player's pack. A bounded command beside `place`
    /// and `erase`, range-checked exactly as they are. The requested quantity is a ceiling, not a
    /// demand: what actually moves is limited by what the building holds and by what the player can
    /// still carry, so a partial withdrawal succeeds and destroys nothing.
    ///
    /// **Only free stock comes back.** `inventory` is exactly that — inputs a running craft has
    /// claimed have already moved to `reserved_inputs`, and energy already released from a coal
    /// sits in `fuel_charge`. Neither is reachable, which is what keeps "take the coal back out of
    /// a burner" honest: the unburned lumps are yours, the heat already in the firebox is spent.
    #[cfg(test)]
    fn withdraw(&mut self, q: i32, r: i32, item_id: ItemId, quantity: u32) -> Result<(), String> {
        self.withdraw_from(q, r, StockKind::Auto, item_id, quantity)
    }

    fn withdraw_from(
        &mut self,
        q: i32,
        r: i32,
        stock: StockKind,
        item_id: ItemId,
        quantity: u32,
    ) -> Result<(), String> {
        if !self.creative && self.is_fluid(item_id) {
            return Err("loose fluid needs a pipe or a barrel".into());
        }
        let index = self.hand_transfer_target(q, r, "withdraw")?;
        let stock = if stock == StockKind::Auto {
            self.stock_kind_for_item(index, item_id)
                .or_else(|| {
                    (self.stock_quantity(index, StockKind::Output, item_id) > 0)
                        .then_some(StockKind::Output)
                })
                .unwrap_or(StockKind::Inventory)
        } else {
            stock
        };
        let stored = self.stock_quantity(index, stock, item_id);
        if stored == 0 {
            return Err("this building holds none of that item".into());
        }
        let moved = quantity.min(stored).min(self.player_room_for(item_id));
        if moved == 0 {
            return Err("carrying capacity is full".into());
        }
        self.subtract_stock(index, stock, item_id, moved);
        *self.player.inventory.entry(item_id).or_default() += moved;
        let id = self.entities[index].id;
        self.dirty.entities.push(id);
        let name = self
            .item_definition(item_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| format!("item {item_id}"));
        self.events.push(format!("Withdrew {moved} × {name}"));
        Ok(())
    }

    /// Grow the building at this hex into the next tier of itself. A new bounded command beside
    /// `place`, `erase`, `withdraw`, and `set_recipe`, range-checked exactly as they are.
    ///
    /// **Contents, orientation, and connections all survive**, and none of them needs special
    /// handling to do so: the entity is never removed and re-created, only its `definition_id`
    /// moves. `validate_upgrade_ladders` has already pinned kind, recipe category, footprint, and
    /// orientation axis across the step, so nothing the entity holds can become invalid by taking
    /// it. Progress is the one exception, because a tier may run at a different cadence, and it is
    /// clamped rather than reset — a machine most of the way through a craft stays most of the way
    /// through it.
    ///
    /// **The price is exact, and it is exact in the same way `erase` is.** An upgrade is priced as
    /// the erase-and-rebuild it stands in for: the old cost comes back and the new cost is paid,
    /// netted per item so nothing round-trips through the pack. Both halves are checked before
    /// either is applied, and a refund that will not fit is refused rather than partially paid —
    /// the same choice, for the same reason, that `erase` makes. That is what keeps an
    /// upgrade / erase round trip from being a duplication exploit: whatever ladder a player walks
    /// up, erasing at the top hands back exactly the sum of what they paid.
    /// The price of an edit that replaces one cost row with another, netted per item and applied
    /// all or nothing.
    ///
    /// Both halves are checked before either is moved — the same rule `erase` keeps — which is what
    /// stops an edit and its undo from minting items between them. Netting rather than paying the
    /// two halves separately is what lets a player with a full pack make an edit that costs them
    /// nothing: the difference is what the change actually costs, so the difference is what travels.
    ///
    /// Creative is neither billed nor credited, so both maps stay empty and nothing moves — the same
    /// answer `place` and `erase_refund` give, and the reason a ladder walked up and erased at the
    /// top still balances at zero.
    fn charge_difference(
        &mut self,
        charge_row: &[Ingredient],
        credit_row: &[Ingredient],
    ) -> Result<(), String> {
        let mut charge: BTreeMap<ItemId, u32> = BTreeMap::new();
        let mut credit: BTreeMap<ItemId, u32> = BTreeMap::new();
        if !self.creative {
            add_ingredients(&mut charge, charge_row);
            add_ingredients(&mut credit, credit_row);
        }
        let mut owed: BTreeMap<ItemId, u32> = BTreeMap::new();
        let mut back: BTreeMap<ItemId, u32> = BTreeMap::new();
        for item_id in charge
            .keys()
            .chain(credit.keys())
            .copied()
            .collect::<BTreeSet<_>>()
        {
            let charged = charge.get(&item_id).copied().unwrap_or(0);
            let returned = credit.get(&item_id).copied().unwrap_or(0);
            if charged > returned {
                owed.insert(item_id, charged - returned);
            } else if returned > charged {
                back.insert(item_id, returned - charged);
            }
        }
        let missing: Vec<String> = owed
            .iter()
            .filter_map(|(item_id, quantity)| {
                let have = self.player.inventory.get(item_id).copied().unwrap_or(0);
                if have >= *quantity {
                    return None;
                }
                let name = self
                    .item_definition(*item_id)
                    .map(|item| item.name.as_str())
                    .unwrap_or("item");
                Some(format!("{quantity} {name} (have {have})"))
            })
            .collect();
        if !missing.is_empty() {
            return Err(format!("need {}", missing.join(" · ")));
        }
        // Only when the step actually hands something back. An edit whose new cost contains the old
        // one — which is the shape a ladder should have — returns nothing, and refusing it because
        // the pack is full would be refusing an edit that does not touch the pack.
        if !back.is_empty() && !self.player_can_carry(&back) {
            return Err("no room to carry what this would return".into());
        }
        for (item_id, quantity) in &owed {
            subtract_item(&mut self.player.inventory, *item_id, *quantity);
        }
        add_inventory(&mut self.player.inventory, &back);
        Ok(())
    }

    /// One atomic legality check for the cells a taller tier would newly occupy.
    ///
    /// This is the alternative to reserving an envelope at initial placement: nothing is held
    /// empty in advance, so the moment of growth is the moment the ground has to be proved. It
    /// asks of the new cells exactly what `placement_legality` asks of a fresh site — free of
    /// buildings, free of the player, buildable terrain, no boundary through the shape — and it
    /// asks the level-pad question of the *whole* enlarged footprint, because a machine that grew
    /// onto a slope is as unstandable as one placed there.
    ///
    /// Refusing here also protects the ports. An output ray leaves the anchor and skips the
    /// building's own cells, so a longer footprint changes where it first meets something else —
    /// unless the ground it grew onto was empty, which is what this refuses to assume.
    fn upgrade_growth_legality(
        &self,
        index: usize,
        next: &BuildingDefinition,
        current: &[Coordinate],
        grown: &[Coordinate],
        next_envelope: &[Coordinate],
        next_clearance: &[Coordinate],
    ) -> Result<(), String> {
        let held: BTreeSet<(i32, i32)> = current.iter().map(|cell| (cell.q, cell.r)).collect();
        let growth: Vec<Coordinate> = grown
            .iter()
            .copied()
            .filter(|cell| !held.contains(&(cell.q, cell.r)))
            .collect();
        if self.boundary_crosses_footprint(grown) {
            return Err("A boundary crosses this building footprint; remove it first".into());
        }
        if self.boundary_crosses_footprint(next_envelope) {
            return Err(
                "A boundary crosses this building's service envelope; remove it first".into(),
            );
        }
        for cell in &growth {
            // Own envelope is the reserved path: the cell was held empty at placement, so growth
            // does not re-ask occupancy. Anything else — a neighbour, another envelope, a rotor —
            // is the atomic path, and a refusal here leaves the building unchanged.
            match self.reservation_conflict(cell.q, cell.r, next.kind, Some(index), false) {
                Ok(()) => {}
                Err(reason) if reason.contains("occupied hex") => {
                    return Err(format!(
                        "{} needs more room than this one has; clear the hexes beside it",
                        next.name
                    ));
                }
                Err(reason) => return Err(reason),
            }
            let (cell_x, cell_y) = axial_world(cell.q, cell.r);
            if circles_overlap(
                self.player.x,
                self.player.y,
                PLAYER_RADIUS,
                cell_x,
                cell_y,
                BUILDING_RADIUS,
            ) {
                return Err("the player blocks this footprint".into());
            }
            // Only the definition's own water rule, not the belt-on-a-bridge exemption: a bridge
            // carries transport it does not own, and growing a machine over one would take the
            // crossing away from the line already using it.
            let shallow_support = next.placement_rule == PlacementRule::Shallows
                && self.terrain_at(cell.q, cell.r) == Terrain::ShallowWater;
            if self.terrain_blocks_construction(cell.q, cell.r) && !shallow_support {
                return Err("environment blocks construction".into());
            }
        }
        for cell in next_envelope {
            if held.contains(&(cell.q, cell.r)) {
                continue;
            }
            self.reservation_conflict(cell.q, cell.r, next.kind, Some(index), false)?;
            let shallow_support = next.placement_rule == PlacementRule::Shallows
                && self.terrain_at(cell.q, cell.r) == Terrain::ShallowWater;
            if self.terrain_blocks_construction(cell.q, cell.r) && !shallow_support {
                return Err("environment blocks construction".into());
            }
        }
        for cell in next_clearance {
            self.reservation_conflict(cell.q, cell.r, next.kind, Some(index), true)?;
            if let Some(other) = self.entity_at(cell.q, cell.r) {
                if other != index && !Self::is_low_infrastructure(self.entities[other].kind) {
                    return Err("building footprint overlaps an occupied hex".into());
                }
            }
        }
        let elevations: Vec<_> = grown
            .iter()
            .map(|cell| self.ground_elevation_at(cell.q, cell.r))
            .collect();
        if let (Some(low), Some(high)) = (
            elevations.iter().min().copied(),
            elevations.iter().max().copied(),
        ) {
            if high - low > self.pad_step_limit(next.foundation_class) {
                return Err("This ground is too uneven; level a pad for this footprint".into());
            }
        }
        Ok(())
    }

    fn upgrade(&mut self, q: i32, r: i32) -> Result<(), String> {
        if !self.within_build_range_of_target(q, r) {
            return Err("upgrade target is outside build range".into());
        }
        let index = self.entity_at(q, r).ok_or("no building to upgrade")?;
        if self.entities[index].placed.scenario_owned {
            return Err("scenario-owned objects are protected".into());
        }
        let current = self
            .building_definition(self.entities[index].placed.definition_id)
            .ok_or("this building has no definition")?;
        let next_id = current
            .upgrades_to
            .ok_or_else(|| format!("{} is already at its highest tier", current.name))?;
        // An upgrade keeps the entity's heading, and the ladder pins both definitions to the same
        // orientation axis, so both halves of the netting are priced at that one heading.
        let orientation = self.entities[index].placed.orientation;
        let refund = current.cost_at(orientation).to_vec();
        let next = self
            .building_definition(next_id)
            .ok_or("the next tier has no definition")?
            .clone();
        if let Some(required) = next.unlock_technology_id {
            if !self.researched.contains(&required) {
                return Err(format!("{} is locked by research", next.name));
            }
        }
        // A container that already holds more than the next tier can is a capacity *downgrade*
        // dressed as an upgrade. Refuse rather than silently strand the overflow.
        if let Some(capacity) = next.capacity {
            if !self.stock_fits_capacity(index, capacity) {
                return Err(format!(
                    "{} holds more than the next tier stores",
                    current.name
                ));
            }
        }
        // A taller tier may claim more ground than the one standing here. Judge the whole enlarged
        // footprint once, before anything is charged or written: the ladder guarantees the cells it
        // already occupies are kept, so what is left to prove is that the new ones are free, legal
        // and level enough to stand on together with the old. Every refusal below leaves the
        // building exactly as it was.
        let current_cells = self.entity_footprint(&self.entities[index]);
        let next_placed = PlacedBuilding {
            definition_id: next_id,
            ..self.entities[index].placed
        };
        let grown = self.footprint_for(next_placed, orientation);
        let next_envelope = self.envelope_for(next_placed, orientation);
        let next_clearance = self.clearance_for(next_placed, orientation);
        self.upgrade_growth_legality(
            index,
            &next,
            &current_cells,
            &grown,
            &next_envelope,
            &next_clearance,
        )?;
        // Netted per item, so the two halves of the price never travel through the pack. A player
        // upgrading with a full pack is charged the difference and asked to carry the difference,
        // which is what an in-place edit actually costs them.
        let old_links = self.graph_links_by_id();
        // Both footprints, because the graph has to forget rays that used to cross a cell the
        // building now covers as surely as it has to recompile the ones that touched it before.
        let changed_cells: BTreeSet<(i32, i32)> = current_cells
            .iter()
            .chain(grown.iter())
            .map(|cell| (cell.q, cell.r))
            .collect();
        self.charge_difference(next.cost_at(orientation), &refund)?;
        let id = self.entities[index].id;
        self.entities[index].placed.definition_id = next_id;
        // A taller tier may reach further, so the resolved deposit list was answered against the
        // wrong radius. It is derived state and rebuilds itself on the next tick.
        self.deposit_links.remove(&id);
        self.dirty.entities.push(id);
        let total = self.progress_total(index);
        if total > 0 {
            self.entities[index].progress = self.entities[index].progress.min(total);
        }
        self.recompile_graph_components(&old_links, &changed_cells, &BTreeSet::from([id]));
        self.events.push(format!("Upgraded to {}", next.name));
        Ok(())
    }

    /// Put stock from the player's pack into a building. The exact mirror of `withdraw`, and it
    /// keeps the same contract: the requested quantity is a ceiling, not a demand, so what actually
    /// moves is limited by what the player holds and by the room the building has left. A partial
    /// store succeeds and destroys nothing.
    ///
    /// **What a building will take is `accepts_item` — the same predicate a belt is held to.** A
    /// hand feeding a smelter is the same event as a lane feeding it, so a machine that refuses
    /// iron ore off a belt refuses it off a palm too, and there is one place where "a furnace takes
    /// its recipe's inputs and anything that burns" is written down. The room is asked separately,
    /// which is what lets a refusal say whether the building had no use for the item or simply no
    /// space left — two different problems the player fixes two different ways.
    #[cfg(test)]
    fn store(&mut self, q: i32, r: i32, item_id: ItemId, quantity: u32) -> Result<(), String> {
        self.store_into(q, r, StockKind::Auto, item_id, quantity)
    }

    fn store_into(
        &mut self,
        q: i32,
        r: i32,
        stock: StockKind,
        item_id: ItemId,
        quantity: u32,
    ) -> Result<(), String> {
        let index = self.hand_transfer_target(q, r, "store")?;
        let held = self.player.inventory.get(&item_id).copied().unwrap_or(0);
        if held == 0 {
            return Err("you are not carrying any of that item".into());
        }
        let stock = if stock == StockKind::Auto {
            self.stock_kind_for_item(index, item_id)
                .ok_or("this building has no use for that")?
        } else {
            stock
        };
        if !self.stock_accepts_item(index, stock, item_id) {
            return Err("this building has no use for that".into());
        }
        let moved = quantity
            .min(held)
            .min(self.room_for_stock(index, stock, item_id));
        if moved == 0 {
            return Err("this building is full".into());
        }
        subtract_item(&mut self.player.inventory, item_id, moved);
        self.add_stock(
            index,
            stock,
            Cargo {
                item_id,
                quantity: moved,
            },
        );
        let id = self.entities[index].id;
        self.dirty.entities.push(id);
        let name = self
            .item_definition(item_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| format!("item {item_id}"));
        self.events.push(format!("Stored {moved} × {name}"));
        Ok(())
    }

    fn pickup_player_stack(&mut self, item_id: ItemId, quantity: u32) -> Result<(), String> {
        if self.player.hand.is_some() {
            return Err("your hand is already holding a stack".into());
        }
        let held = self.player.inventory.get(&item_id).copied().unwrap_or(0);
        let moved = quantity.min(held).min(self.stack_size(item_id));
        if moved == 0 {
            return Err("you are not carrying any of that item".into());
        }
        subtract_item(&mut self.player.inventory, item_id, moved);
        self.player.hand = Some(Cargo {
            item_id,
            quantity: moved,
        });
        Ok(())
    }

    fn pickup_building_stack(
        &mut self,
        q: i32,
        r: i32,
        stock: StockKind,
        item_id: ItemId,
        quantity: u32,
    ) -> Result<(), String> {
        if !self.creative && self.is_fluid(item_id) {
            return Err("loose fluid needs a pipe or a barrel".into());
        }
        if self.player.hand.is_some() {
            return Err("your hand is already holding a stack".into());
        }
        if matches!(stock, StockKind::Auto) {
            return Err("pick a named building compartment".into());
        }
        let index = self.hand_transfer_target(q, r, "pick up")?;
        let stored = self.stock_quantity(index, stock, item_id);
        let moved = quantity.min(stored).min(self.stack_size(item_id));
        if moved == 0 {
            return Err("this compartment holds none of that item".into());
        }
        self.subtract_stock(index, stock, item_id, moved);
        self.player.hand = Some(Cargo {
            item_id,
            quantity: moved,
        });
        self.dirty.entities.push(self.entities[index].id);
        Ok(())
    }

    fn place_player_stack(&mut self, quantity: u32) -> Result<(), String> {
        let hand = self.player.hand.ok_or("your hand is empty")?;
        let moved = quantity
            .min(hand.quantity)
            .min(self.player_room_for(hand.item_id));
        if moved == 0 {
            return Err("carrying capacity is full".into());
        }
        *self.player.inventory.entry(hand.item_id).or_default() += moved;
        if moved == hand.quantity {
            self.player.hand = None;
        } else if let Some(held) = &mut self.player.hand {
            held.quantity -= moved;
        }
        Ok(())
    }

    fn place_building_stack(
        &mut self,
        q: i32,
        r: i32,
        stock: StockKind,
        quantity: u32,
    ) -> Result<(), String> {
        if matches!(stock, StockKind::Auto) {
            return Err("pick a named building compartment".into());
        }
        let hand = self.player.hand.ok_or("your hand is empty")?;
        let index = self.hand_transfer_target(q, r, "place")?;
        if !self.stock_accepts_item(index, stock, hand.item_id) {
            return Err("that item does not belong in this compartment".into());
        }
        let moved =
            quantity
                .min(hand.quantity)
                .min(self.room_for_stock(index, stock, hand.item_id));
        if moved == 0 {
            return Err("this compartment is full".into());
        }
        self.add_stock(
            index,
            stock,
            Cargo {
                item_id: hand.item_id,
                quantity: moved,
            },
        );
        if moved == hand.quantity {
            self.player.hand = None;
        } else if let Some(held) = &mut self.player.hand {
            held.quantity -= moved;
        }
        self.dirty.entities.push(self.entities[index].id);
        Ok(())
    }

    fn drop_player_stack(&mut self, q: i32, r: i32, quantity: u32) -> Result<(), String> {
        let hand = self.player.hand.ok_or("your hand is empty")?;
        if !self.within_build_range_of_target(q, r) {
            return Err("that hex is out of reach".into());
        }
        self.ensure_neighborhood(self.player.x, self.player.y);
        self.ensure_tile(q, r);
        if self.terrain_blocks_movement(q, r) {
            return Err("items cannot land on impassable terrain".into());
        }
        let moved = quantity.min(hand.quantity);
        if moved == 0 {
            return Err("nothing to drop".into());
        }
        let item_id = hand.item_id;
        if moved == hand.quantity {
            self.player.hand = None;
        } else if let Some(held) = &mut self.player.hand {
            held.quantity -= moved;
        }
        self.add_ground_item(q, r, item_id, moved);
        let name = self
            .item_definition(item_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| format!("item {item_id}"));
        self.events.push(format!("Dropped {moved} × {name}"));
        Ok(())
    }

    /// Add deterministic world-owned cargo at one hex. Player drops and demolished transport both
    /// use this path, so stacking, lifetime refresh, dirty tracking, saves, and wire snapshots
    /// cannot disagree about what counts as an item on the ground.
    fn add_ground_item(&mut self, q: i32, r: i32, item_id: ItemId, quantity: u32) {
        if quantity == 0 {
            return;
        }
        let despawn_tick = self.tick + GROUND_ITEM_LIFETIME_TICKS;
        if let Some(existing) = self
            .ground_items
            .iter_mut()
            .find(|item| item.q == q && item.r == r && item.item_id == item_id)
        {
            existing.quantity += quantity;
            existing.despawn_tick = despawn_tick;
        } else {
            let id = self.next_ground_item_id;
            self.next_ground_item_id = self.next_ground_item_id.wrapping_add(1);
            self.ground_items.push(GroundItem {
                id,
                q,
                r,
                item_id,
                quantity,
                despawn_tick,
            });
        }
        self.dirty.ground_items = true;
    }

    /// Give the machine at this hex a different recipe. Bounded and range-checked like every other
    /// edit, and it enforces the same category rule placement does, so a kiln can no more be
    /// reassigned to a circuit than it could be built with one.
    ///
    /// A machine mid-craft is refused rather than reassigned: its reserved inputs belong to the job
    /// it is running, and deciding what happens to a part-finished one is a question worth its own
    /// pass — the same reason `withdraw` reaches into a machine's free stock and never into
    /// `reserved_inputs`.
    fn set_recipe(&mut self, q: i32, r: i32, recipe_id: RecipeId) -> Result<(), String> {
        if !self.within_build_range_of_target(q, r) {
            return Err("recipe target is outside build range".into());
        }
        let index = self.entity_at(q, r).ok_or("no machine at that hex")?;
        if self.entities[index].kind != BuildingKind::Composer {
            return Err("only machines that run recipes can be reassigned".into());
        }
        if self.entities[index].placed.scenario_owned {
            return Err("scenario-owned objects are protected".into());
        }
        if self.entities[index].progress > 0 {
            return Err("this machine is mid-craft".into());
        }
        if self.entities[index].placed.recipe_id == Some(recipe_id) {
            return Err("this machine already runs that recipe".into());
        }
        let recipe = self
            .recipe(recipe_id)
            .ok_or_else(|| format!("unknown recipe {recipe_id}"))?
            .clone();
        let definition = self
            .building_definition(self.entities[index].placed.definition_id)
            .ok_or("unknown building definition")?;
        if !definition.supports_recipe(&recipe) {
            return Err(format!(
                "{} cannot run a {} recipe",
                definition.name, recipe.category
            ));
        }
        let manual = definition.manual_work;
        self.entities[index].placed.recipe_id = Some(recipe_id);
        if manual {
            self.entities[index].disabled = true;
        }
        let id = self.entities[index].id;
        // A recipe chooses the product identities. Ports for the old recipe cannot silently become
        // ports for the new one, so reassignment returns the machine to its single facing outlet.
        self.output_routes.remove(&id);
        self.compile_graph();
        self.dirty.entities.push(id);
        self.events.push(format!("Set recipe to {}", recipe.name));
        Ok(())
    }

    fn output_items(&self, index: usize) -> Vec<ItemId> {
        let entity = &self.entities[index];
        let mut items: Vec<ItemId> = entity
            .placed
            .recipe_id
            .and_then(|id| self.recipe(id))
            .map(|recipe| recipe.outputs().map(|output| output.item_id).collect())
            .unwrap_or_default();
        if items.is_empty() {
            if let Some(item_id) = self
                .building_definition(entity.placed.definition_id)
                .and_then(|definition| definition.output_item_id)
            {
                items.push(item_id);
            }
        }
        items.sort_unstable();
        items.dedup();
        items
    }

    /// The legacy facing translated into one unambiguous exterior footprint port.
    fn default_output_route(&self, index: usize) -> OutputRoute {
        let entity = &self.entities[index];
        let direction = entity.placed.orientation;
        let mut cell = Coordinate {
            q: entity.placed.q,
            r: entity.placed.r,
        };
        if usize::from(direction) < DIRECTIONS.len() {
            let footprint: BTreeSet<(i32, i32)> = self
                .entity_footprint(entity)
                .into_iter()
                .map(|cell| (cell.q, cell.r))
                .collect();
            let (dq, dr) = DIRECTIONS[usize::from(direction)];
            while footprint.contains(&(cell.q + dq, cell.r + dr)) {
                cell.q += dq;
                cell.r += dr;
            }
        }
        OutputRoute {
            q: cell.q - entity.placed.q,
            r: cell.r - entity.placed.r,
            direction,
        }
    }

    fn set_output_route(
        &mut self,
        q: i32,
        r: i32,
        item_id: ItemId,
        output_q: i32,
        output_r: i32,
        direction: u8,
    ) -> Result<(), String> {
        if !self.within_build_range_of_target(q, r) {
            return Err("output target is outside build range".into());
        }
        let index = self.entity_at(q, r).ok_or("no building at that hex")?;
        if self.entities[index].placed.scenario_owned {
            return Err("scenario-owned objects are protected".into());
        }
        let items = self.output_items(index);
        if !items.contains(&item_id) {
            return Err("that building does not produce this item".into());
        }
        if usize::from(direction) >= DIRECTIONS.len() {
            return Err("an output port must use one of the six footprint sides".into());
        }
        let footprint: BTreeSet<(i32, i32)> = self
            .entity_footprint(&self.entities[index])
            .into_iter()
            .map(|cell| (cell.q, cell.r))
            .collect();
        if !footprint.contains(&(output_q, output_r)) {
            return Err("output port is not on this building's footprint".into());
        }
        let (dq, dr) = DIRECTIONS[usize::from(direction)];
        if footprint.contains(&(output_q + dq, output_r + dr)) {
            return Err("output port is on an internal footprint seam".into());
        }
        let id = self.entities[index].id;
        let anchor = self.entities[index].placed;
        // The first edit materializes every current default before changing one. No co-product is
        // disconnected merely because the player started configuring its neighbour.
        if !self.output_routes.contains_key(&id) {
            let default = self.default_output_route(index);
            self.output_routes.insert(
                id,
                items.iter().copied().map(|item| (item, default)).collect(),
            );
        }
        self.output_routes.entry(id).or_default().insert(
            item_id,
            OutputRoute {
                q: output_q - anchor.q,
                r: output_r - anchor.r,
                direction,
            },
        );
        self.compile_graph();
        self.dirty.entities.push(id);
        self.events.push("Changed product output".into());
        Ok(())
    }

    /// Switch the machine at this hex off, or back on. Bounded and range-checked like every other
    /// edit, and protected objects are protected here too.
    ///
    /// **Only buildings that do work can be switched**, because only they have anything to stop. A
    /// belt is a lane, a container is a shelf, a pole is a wire — none of them consume, produce, or
    /// burn, so a switch on them would be a control that changes nothing. Refusing is more honest
    /// than a dead toggle.
    ///
    /// Nothing is discarded. Progress, stock, reserved inputs, and banked charge all survive being
    /// switched off, so this is a pause and never a partial `erase`.
    fn set_enabled(&mut self, q: i32, r: i32, enabled: bool) -> Result<(), String> {
        if !self.within_build_range_of_target(q, r) {
            return Err("switch target is outside build range".into());
        }
        let index = self.entity_at(q, r).ok_or("no building at that hex")?;
        if !Self::can_be_switched(self.entities[index].kind) {
            return Err("that building has no work to switch off".into());
        }
        if self.entities[index].placed.scenario_owned {
            return Err("scenario-owned objects are protected".into());
        }
        if self.entities[index].disabled != enabled {
            return Err(if enabled {
                "this building is already running".into()
            } else {
                "this building is already switched off".into()
            });
        }
        let manual = self.is_manual_workshop(index);
        if manual && enabled {
            if !self.can_work_here(index) {
                return Err(
                    "stand beside the workshop and stop walking or gathering to work".into(),
                );
            }
            let recipe = self.entities[index]
                .placed
                .recipe_id
                .and_then(|id| self.recipe(id))
                .ok_or("choose a workshop recipe first")?;
            if !self.room_for_recipe(index, recipe) {
                return Err("workshop output is full".into());
            }
            if self.entities[index].progress == 0
                && !recipe.inputs.iter().all(|input| {
                    self.stock_quantity(index, StockKind::Input, input.item_id) >= input.quantity
                })
            {
                return Err("load the workshop ingredients before starting work".into());
            }
            // One player can attend one station. This scan runs only on an explicit work command,
            // never on a player step or as a separate per-tick traversal.
            for other in 0..self.entities.len() {
                if other != index
                    && !self.entities[other].disabled
                    && self.is_manual_workshop(other)
                {
                    self.entities[other].disabled = true;
                    self.dirty.entities.push(self.entities[other].id);
                }
            }
        }
        self.entities[index].disabled = !enabled;
        let id = self.entities[index].id;
        self.dirty.entities.push(id);
        let name = self
            .building_definition(self.entities[index].placed.definition_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| "Building".into());
        self.events.push(if manual && enabled {
            format!("Working at {name} — one batch")
        } else if manual {
            format!("Paused work at {name}")
        } else if enabled {
            format!("Switched {name} on")
        } else {
            format!("Switched {name} off")
        });
        Ok(())
    }

    fn is_manual_workshop(&self, index: usize) -> bool {
        self.building_definition(self.entities[index].placed.definition_id)
            .is_some_and(|definition| definition.manual_work)
    }

    fn can_work_here(&self, index: usize) -> bool {
        self.player.move_x == 0
            && self.player.move_y == 0
            && self.player.walk_goal.is_none()
            && self.player.action_cooldown == 0
            && self.within_hex_range_of_entity(index, 1)
    }

    /// The kinds that have work a switch can suspend: anything that extracts, crafts, pumps, or
    /// burns. The same list every arm of the tick consults through `entity_running`.
    fn can_be_switched(kind: BuildingKind) -> bool {
        matches!(
            kind,
            BuildingKind::Extractor
                | BuildingKind::Pump
                | BuildingKind::Composer
                | BuildingKind::Generator
                | BuildingKind::Boiler
        )
    }

    /// Whether this entity is doing its job at all. One predicate, asked by every arm of the tick
    /// that could otherwise forget the switch — the extractor, the pump, the composer, the plant,
    /// and the power network's demand.
    fn entity_running(&self, index: usize) -> bool {
        !self.entities[index].disabled
    }

    fn rotate(&mut self, q: i32, r: i32, reverse: bool) -> Result<(), String> {
        if !self.within_build_range_of_target(q, r) {
            return Err("rotate target is outside build range".into());
        }
        let index = self.entity_at(q, r).ok_or("no building to rotate")?;
        if self.entities[index].placed.scenario_owned {
            return Err("scenario-owned objects are protected".into());
        }
        let old_links = self.graph_links_by_id();
        let old_footprint = self.entity_footprint(&self.entities[index]);
        let id = self.entities[index].id;
        // Rotation stays on the definition's own axis: an edge-axis machine walks the six edges, and
        // an `Any` belt walks all twelve in angular order. A building can never be turned into a
        // heading it could not have been built at, which on the any axis means the research gate
        // too: an unresearched heading is stepped over rather than landed on, so `R` cycles exactly
        // the six edges until the two-row reach is paid for and all twelve afterwards. The current
        // heading is one the building was built at, so the walk always terminates.
        let definition = self
            .building_definition(self.entities[index].placed.definition_id)
            .cloned();
        let axis = definition
            .as_ref()
            .map(|definition| definition.orientation_axis)
            .unwrap_or_default();
        let orientation = self.entities[index].placed.orientation;
        let mut next_orientation = orientation;
        for _ in 0..axis.range().len() {
            next_orientation = if reverse {
                axis.previous(next_orientation)
            } else {
                axis.next(next_orientation)
            };
            let researched = definition.as_ref().is_none_or(|definition| {
                definition
                    .gates_at(next_orientation)
                    .into_iter()
                    .flatten()
                    .all(|required| self.researched.contains(&required))
            });
            if researched {
                break;
            }
        }
        if next_orientation == orientation {
            return Err("no other heading is researched for this building".into());
        }
        let next_placed = PlacedBuilding {
            orientation: next_orientation,
            ..self.entities[index].placed
        };
        let next_footprint = self.footprint_for(next_placed, next_orientation);
        let next_envelope = self.envelope_for(next_placed, next_orientation);
        let next_clearance = self.clearance_for(next_placed, next_orientation);
        if self.boundary_crosses_footprint(&next_footprint) {
            return Err("A boundary crosses the rotated footprint; remove it first".into());
        }
        if self.boundary_crosses_footprint(&next_envelope) {
            return Err(
                "A boundary crosses this building's service envelope; remove it first".into(),
            );
        }
        let rotating_kind = self.entities[index].kind;
        for cell in &next_footprint {
            let supported_transport = rotating_kind == BuildingKind::Belt
                && self.bridge_at(cell.q, cell.r)
                && self
                    .entity_at(cell.q, cell.r)
                    .is_some_and(|other| self.entities[other].kind == BuildingKind::Bridge);
            match self.reservation_conflict(
                cell.q,
                cell.r,
                rotating_kind,
                Some(index),
                supported_transport,
            ) {
                Ok(()) => {}
                Err(_) => return Err("rotated footprint would overlap another building".into()),
            }
        }
        for cell in &next_envelope {
            self.reservation_conflict(cell.q, cell.r, rotating_kind, Some(index), false)?;
        }
        for cell in &next_clearance {
            self.reservation_conflict(cell.q, cell.r, rotating_kind, Some(index), true)?;
            if let Some(other) = self.entity_at(cell.q, cell.r) {
                if other != index && !Self::is_low_infrastructure(self.entities[other].kind) {
                    return Err("rotated footprint would overlap another building".into());
                }
            }
        }
        // A heading is a price on the any axis, so turning onto one is an edit that costs the
        // difference — otherwise a player could buy the cheap heading and rotate onto the expensive
        // one for free, which is exactly the dominance `corner_construction_cost` exists to prevent.
        // For every other definition both rows are the same row, the netting cancels, and rotation
        // stays the free adjustment it has always been.
        if let Some(definition) = &definition {
            let charge = definition.cost_at(next_orientation).to_vec();
            let credit = definition.cost_at(orientation).to_vec();
            self.charge_difference(&charge, &credit)?;
        }
        self.entities[index].placed.orientation = next_orientation;
        // Product ports are attached to the hull. Rotating the building turns both the chosen
        // footprint tile and its exterior side, rather than leaving a world-fixed outlet behind.
        let old_turn = if orientation >= NORTH {
            orientation - NORTH
        } else {
            orientation
        };
        let next_turn = if next_orientation >= NORTH {
            next_orientation - NORTH
        } else {
            next_orientation
        };
        let turns = (next_turn + 6 - old_turn) % 6;
        if let Some(routes) = self.output_routes.get_mut(&id) {
            for route in routes.values_mut() {
                let rotated = rotate_coordinate(
                    Coordinate {
                        q: route.q,
                        r: route.r,
                    },
                    turns,
                );
                route.q = rotated.q;
                route.r = rotated.r;
                route.direction = (route.direction + turns) % 6;
            }
        }
        self.dirty.entities.push(id);
        let changed_cells = old_footprint
            .into_iter()
            .chain(next_footprint)
            .map(|cell| (cell.q, cell.r))
            .collect();
        self.recompile_graph_components(&old_links, &changed_cells, &BTreeSet::from([id]));
        self.events.push("Rotated building".into());
        Ok(())
    }

    fn apply_commands(&mut self, commands_json: &str) -> Result<(), String> {
        let commands: Vec<InputCommand> =
            serde_json::from_str(commands_json).map_err(|error| error.to_string())?;
        self.apply_command_batch(commands, true)
    }

    /// One frame of native work: the host's bounded command batch, the player steps that frame's
    /// real time is worth, and the simulation ticks its speed setting is worth. The two counts are
    /// separate because the player and the factory now run on separate clocks.
    fn advance(
        &mut self,
        commands_json: &str,
        count: u32,
        player_steps: u32,
    ) -> Result<(), String> {
        let commands: Vec<InputCommand> =
            serde_json::from_str(commands_json).map_err(|error| error.to_string())?;
        let should_clear_events = !commands.is_empty() || count > 0 || player_steps > 0;
        self.apply_command_batch(commands, should_clear_events)?;
        self.advance_player_steps(player_steps.min(240));
        self.advance_ticks(count.min(240));
        Ok(())
    }

    fn apply_command_batch(
        &mut self,
        commands: Vec<InputCommand>,
        clear_events: bool,
    ) -> Result<(), String> {
        if commands.len() > MAX_COMMANDS_PER_BATCH {
            return Err(format!(
                "input batch exceeds the native limit of {MAX_COMMANDS_PER_BATCH}"
            ));
        }
        if clear_events {
            self.events.clear();
        }
        for command in commands {
            let result = match command {
                InputCommand::BoundaryEdit { edit } => self.edit_boundaries(&edit),
                InputCommand::UndoBoundary => self.undo_boundary(),
                InputCommand::GroundEdit { edit } => self.edit_ground(&edit),
                InputCommand::UndoGround => self.undo_ground(),
                InputCommand::WaterEdit {
                    q,
                    r,
                    action,
                    quanta,
                } => {
                    if !self.creative {
                        Err("water edits are available in creative mode".into())
                    } else if !self.within_build_range_of_target(q, r) {
                        Err("water target is out of build range".into())
                    } else {
                        self.edit_water(q, r, action, quanta).map(|report| {
                            self.mark_all_entities_dirty();
                            self.replan_walk();
                            self.events.push(format!(
                                "Water settled over {} cells in {} sweeps",
                                report.cells, report.sweeps
                            ));
                        })
                    }
                }
                InputCommand::MoveIntent { x, y } => self.set_move_intent(x, y),
                InputCommand::Aim { x, y } => self.set_aim(x, y),
                InputCommand::Gather => self.gather(),
                InputCommand::GatherAt { q, r } => self.gather_at(q, r),
                InputCommand::Deposit { item_id } => self.deposit_item(item_id),
                InputCommand::Place {
                    q,
                    r,
                    definition_id,
                    orientation,
                    recipe_id,
                } => self.place(q, r, definition_id, orientation, recipe_id),
                InputCommand::PlaceLine {
                    q,
                    r,
                    to_q,
                    to_r,
                    definition_id,
                    orientation,
                    recipe_id,
                } => self.place_line((q, r), (to_q, to_r), definition_id, orientation, recipe_id),
                InputCommand::Erase { q, r } => self.erase(q, r),
                InputCommand::EraseLine { q, r, to_q, to_r } => {
                    self.erase_line((q, r), (to_q, to_r))
                }
                InputCommand::Rotate { q, r, reverse } => self.rotate(q, r, reverse),
                InputCommand::SetOutputRoute {
                    q,
                    r,
                    item_id,
                    output_q,
                    output_r,
                    direction,
                } => self.set_output_route(q, r, item_id, output_q, output_r, direction),
                InputCommand::Upgrade { q, r } => self.upgrade(q, r),
                InputCommand::Withdraw {
                    q,
                    r,
                    item_id,
                    quantity,
                    stock,
                } => self.withdraw_from(q, r, stock, item_id, quantity),
                InputCommand::Store {
                    q,
                    r,
                    item_id,
                    quantity,
                    stock,
                } => self.store_into(q, r, stock, item_id, quantity),
                InputCommand::PickupPlayerStack { item_id, quantity } => {
                    self.pickup_player_stack(item_id, quantity)
                }
                InputCommand::PickupBuildingStack {
                    q,
                    r,
                    stock,
                    item_id,
                    quantity,
                } => self.pickup_building_stack(q, r, stock, item_id, quantity),
                InputCommand::PlacePlayerStack { quantity } => self.place_player_stack(quantity),
                InputCommand::PlaceBuildingStack {
                    q,
                    r,
                    stock,
                    quantity,
                } => self.place_building_stack(q, r, stock, quantity),
                InputCommand::DropPlayerStack { q, r, quantity } => {
                    self.drop_player_stack(q, r, quantity)
                }
                InputCommand::SetRecipe { q, r, recipe_id } => self.set_recipe(q, r, recipe_id),
                InputCommand::SetEnabled { q, r, enabled } => self.set_enabled(q, r, enabled),
                InputCommand::Undo => self.undo(),
                InputCommand::PurchaseSkill { skill_id } => self.purchase_skill(skill_id),
                InputCommand::Research { technology_id } => self.research(technology_id),
                InputCommand::SkipRequest { slot } => self.skip_request(slot),
                InputCommand::PostRequest { request_id } => self.post_request(request_id),
                InputCommand::SetCreative { enabled } => {
                    self.set_creative(enabled);
                    Ok(())
                }
                InputCommand::Grant { item_id, quantity } => self.grant(item_id, quantity),
                InputCommand::Discard { item_id, quantity } => self.discard(item_id, quantity),
                InputCommand::SetCarrySlots { slots } => self.set_carry_slots(slots),
                InputCommand::WalkTo { q, r } => self.walk_to(q, r),
            };
            if let Err(error) = result {
                self.events.push(error);
            }
        }
        Ok(())
    }

    /// Whether a machine can pay for its next craft's heat: already charged, or holding something
    /// it may burn. Read-only, and it asks `burnable_item` exactly as the tick does.
    fn fuel_ready(&self, entity: &Entity) -> bool {
        let Some(recipe) = entity.placed.recipe_id.and_then(|id| self.recipe(id)) else {
            return true;
        };
        recipe.fuel == 0
            || entity.fuel_charge >= recipe.fuel
            || self.burnable_item(&entity.fuel_inventory, &[]).is_some()
            || self
                .burnable_item(&entity.inventory, &recipe.inputs)
                .is_some()
    }

    /// `deposit_available` is whether the source this entity draws from still has anything in it —
    /// a covering deposit for an extractor, open water for a pump. It is passed in rather than
    /// searched for: resolving it through the cached candidate list keeps a snapshot linear in
    /// entity count, where the equivalent tile scan made it quadratic.
    fn status_of(
        &self,
        index: usize,
        deposit_available: bool,
        fuel_ready: bool,
        powered: bool,
        brownout: bool,
    ) -> EntityStatus {
        let entity = &self.entities[index];
        if entity.disabled {
            return EntityStatus::SwitchedOff;
        }
        match entity.kind {
            BuildingKind::Extractor if self.room_for_stock(index, StockKind::Output, 0) == 0 => {
                EntityStatus::OutputBlocked
            }
            BuildingKind::Extractor if !deposit_available => EntityStatus::DepositDepleted,
            BuildingKind::Extractor if !powered => EntityStatus::NoPower,
            BuildingKind::Extractor if brownout => EntityStatus::Brownout,
            BuildingKind::Extractor if entity.progress > 0 => EntityStatus::Extracting,
            BuildingKind::Pump if self.room_for_stock(index, StockKind::Output, 0) == 0 => {
                EntityStatus::OutputBlocked
            }
            BuildingKind::Pump if !deposit_available => EntityStatus::NoWaterInReach,
            BuildingKind::Pump if !powered => EntityStatus::NoPower,
            BuildingKind::Pump if brownout => EntityStatus::Brownout,
            BuildingKind::Pump => EntityStatus::Pumping,
            BuildingKind::Composer
                if entity
                    .placed
                    .recipe_id
                    .and_then(|id| self.recipe(id))
                    .is_some_and(|recipe| !self.room_for_recipe(index, recipe)) =>
            {
                EntityStatus::OutputBlocked
            }
            BuildingKind::Composer if entity.progress > 0 && brownout => EntityStatus::Brownout,
            BuildingKind::Composer if entity.progress > 0 => EntityStatus::Composing,
            BuildingKind::Composer if !powered => EntityStatus::NoPower,
            BuildingKind::Composer if !fuel_ready => EntityStatus::OutOfFuel,
            BuildingKind::Composer => EntityStatus::WaitingForInputs,
            BuildingKind::Container if inventory_total(&entity.inventory) > 0 => {
                EntityStatus::Buffered
            }
            BuildingKind::Belt if entity.cargo.is_some() || !entity.lane.is_empty() => {
                match entity.cargo {
                    // Nothing has finished crossing yet, so there is nothing for the far end to
                    // refuse. A belt with items still travelling along it is carrying, whatever the
                    // building it points at is doing.
                    None => EntityStatus::Carrying,
                    // A splitter is carrying while *any* branch will take the item. Reading only the
                    // first would paint a working junction as blocked every time its cursor happened
                    // to rest on the branch that is full.
                    Some(cargo)
                        if self.graph[index]
                            .iter_for(cargo.item_id)
                            .any(|target| self.can_accept(target, cargo)) =>
                    {
                        EntityStatus::Carrying
                    }
                    Some(_) => EntityStatus::OutputBlocked,
                }
            }
            BuildingKind::Consumer => EntityStatus::Receiving,
            BuildingKind::Hub => EntityStatus::LandingHub,
            BuildingKind::Generator => self.generator_status(index),
            BuildingKind::Boiler if self.boiler_live(index) => EntityStatus::Generating,
            BuildingKind::Boiler
                if self.stock_quantity(index, StockKind::Input, WATER_ITEM) == 0 =>
            {
                EntityStatus::WaitingForInputs
            }
            BuildingKind::Boiler => EntityStatus::OutOfFuel,
            _ => EntityStatus::Idle,
        }
    }

    fn generator_status(&self, index: usize) -> EntityStatus {
        let source = self
            .building_definition(self.entities[index].placed.definition_id)
            .and_then(|definition| definition.power_source);
        match source {
            Some(PowerSource::Burner) if !self.generator_has_fuel(index) => EntityStatus::OutOfFuel,
            Some(PowerSource::Turbine) if !self.adjacent_live_boiler(index) => {
                EntityStatus::NoBoiler
            }
            _ if self.generator_output_now(index) > 0 => EntityStatus::Generating,
            _ => EntityStatus::Idle,
        }
    }

    fn stock_snapshot(&self, index: usize, stock: StockKind) -> Vec<Ingredient> {
        let entity = &self.entities[index];
        let mut item_ids: BTreeSet<ItemId> = match stock {
            StockKind::Inventory => entity.inventory.keys().copied().collect(),
            StockKind::Input => entity.input_inventory.keys().copied().collect(),
            StockKind::Fuel => entity.fuel_inventory.keys().copied().collect(),
            StockKind::Output => entity.output_inventory.keys().copied().collect(),
            StockKind::Auto => BTreeSet::new(),
        };
        if matches!(stock, StockKind::Input | StockKind::Fuel) {
            item_ids.extend(
                entity
                    .inventory
                    .keys()
                    .copied()
                    .filter(|&item_id| self.stock_kind_for_item(index, item_id) == Some(stock)),
            );
        }
        if stock == StockKind::Output {
            if let Some(cargo) = entity.cargo {
                item_ids.insert(cargo.item_id);
            }
        }
        item_ids
            .into_iter()
            .filter_map(|item_id| {
                let quantity = self.stock_quantity(index, stock, item_id);
                (quantity > 0).then_some(Ingredient { item_id, quantity })
            })
            .collect()
    }

    /// One entity's snapshot. Every path that reports an entity to the host — the complete
    /// snapshot and the incremental delta alike — builds it here, so the sparse path cannot drift
    /// from the full one.
    fn entity_snapshot(&mut self, index: usize) -> EntitySnapshot {
        // Resolving through the cached candidate list rather than scanning the tile map is what
        // keeps this O(1) in world size. The cache is derived state, so filling it changes nothing.
        let (deposit_available, water_source) = match self.entities[index].kind {
            BuildingKind::Extractor => (self.extractor_deposit(index).is_some(), None),
            // A physical pump names its current source. A finite pond can disappear from this
            // answer; a river keeps its depth and publishes its discharge rate.
            BuildingKind::Pump => {
                let placed = self.entities[index].placed;
                let radius = self
                    .building_definition(placed.definition_id)
                    .and_then(|definition| definition.extract_radius)
                    .unwrap_or(PUMP_RADIUS as u32) as i32;
                let source = self
                    .ground_is_physical()
                    .then(|| self.pump_source_within_reach(placed.q, placed.r, radius))
                    .flatten();
                (
                    source.is_some()
                        || !self.ground_is_physical()
                            && self.water_within_reach(placed.q, placed.r, radius),
                    source,
                )
            }
            _ => (false, None),
        };
        let inventory = if self.entities[index].kind == BuildingKind::Container {
            self.stock_snapshot(index, StockKind::Inventory)
        } else {
            Vec::new()
        };
        let input_inventory = self.stock_snapshot(index, StockKind::Input);
        let fuel_inventory = self.stock_snapshot(index, StockKind::Fuel);
        let output_inventory = self.stock_snapshot(index, StockKind::Output);
        let output_routes: Vec<OutputRouteSnapshot> = self
            .output_items(index)
            .into_iter()
            .map(|item_id| {
                let entity = &self.entities[index];
                let route = self
                    .output_routes
                    .get(&entity.id)
                    .and_then(|routes| routes.get(&item_id))
                    .copied()
                    .unwrap_or_else(|| self.default_output_route(index));
                OutputRouteSnapshot {
                    item_id,
                    q: entity.placed.q + route.q,
                    r: entity.placed.r + route.r,
                    direction: route.direction,
                    target_id: self.graph[index]
                        .iter_for(item_id)
                        .next()
                        .map(|target| self.entities[target].id),
                }
            })
            .collect();
        let entity = &self.entities[index];
        let fuel_required = entity
            .placed
            .recipe_id
            .and_then(|id| self.recipe(id))
            .map_or(0, |recipe| recipe.fuel);
        let fuel_ready = self.fuel_ready(entity);
        // Two different failures, and the player fixes them with two different buildings: `powered`
        // is "wired to something that generates" and wants a pole or a plant, `brownout` is "wired
        // in but the bank ran dry" and wants more generation. Fuel-only and manual stations
        // have no grid requirement and must report their actual input/fuel condition instead.
        let needs_power = self
            .building_definition(entity.placed.definition_id)
            .and_then(|definition| definition.power_draw)
            .unwrap_or(0)
            > 0;
        let powered = !needs_power || self.entity_connected(index);
        let brownout = powered && !self.entity_powered(index);
        let (power_satisfied, power_demand) = self.network_of(index);
        let power_charge = entity.power_charge;
        let power_capacity = self.power_capacity(index);
        let progress_total = self.progress_total(index);
        let footprint = self.entity_footprint(entity);
        let links = self.graph[index];
        let next_id = links.primary().map(|target| self.entities[target].id);
        let branch_ids: Vec<u32> = links
            .iter()
            .skip(1)
            .map(|target| self.entities[target].id)
            .collect();
        let snapshot = EntitySnapshot {
            id: entity.id,
            q: entity.placed.q,
            r: entity.placed.r,
            definition_id: entity.placed.definition_id,
            kind: entity.kind,
            orientation: entity.placed.orientation,
            recipe_id: entity.placed.recipe_id,
            scenario_owned: entity.placed.scenario_owned,
            cargo: entity.cargo,
            lane: entity.lane.clone(),
            inventory,
            input_inventory,
            fuel_inventory,
            output_inventory,
            output_routes,
            water_source,
            progress: entity.progress,
            progress_total,
            fuel_charge: entity.fuel_charge,
            fuel_required,
            power_satisfied,
            power_demand,
            power_charge,
            power_capacity,
            status: EntityStatus::Idle,
            next_id,
            branch_ids,
            footprint,
        };
        let mut snapshot = snapshot;
        snapshot.status = self.status_of(index, deposit_available, fuel_ready, powered, brownout);
        snapshot
    }

    /// Every entity, in ascending stable id order.
    fn entity_snapshots(&mut self) -> Vec<EntitySnapshot> {
        let mut indices: Vec<usize> = (0..self.entities.len()).collect();
        indices.sort_by_key(|&index| self.entities[index].id);
        indices
            .into_iter()
            .map(|index| self.entity_snapshot(index))
            .collect()
    }

    /// The generated chunk set with its per-chunk entity counts. Counting in one pass over the
    /// blueprint keeps this linear; asking each chunk to filter the whole blueprint did not.
    fn chunk_snapshots(&self) -> Vec<ChunkSnapshot> {
        let size = self.scenario.chunk_size;
        let mut counts: BTreeMap<(i32, i32), usize> = BTreeMap::new();
        for entity in &self.entities {
            let chunk = (
                floor_div(entity.placed.q, size),
                floor_div(entity.placed.r, size),
            );
            *counts.entry(chunk).or_default() += 1;
        }
        self.generated_chunks
            .iter()
            .map(|&(chunk_q, chunk_r)| {
                let (x, y, span) = chunk_world_bounds(chunk_q, chunk_r, size);
                ChunkSnapshot {
                    chunk_q,
                    chunk_r,
                    x,
                    y,
                    span,
                    entity_count: counts.get(&(chunk_q, chunk_r)).copied().unwrap_or(0),
                }
            })
            .collect()
    }

    /// One cell of generated ground, as the wire carries it. The band travels beside the height
    /// rather than instead of it: it is what the shipped presentation still reads, and it is a
    /// derived output of the same generated facts, so nothing here is a second source of truth.
    fn tile_snapshot(&self, q: i32, r: i32) -> TileSnapshot {
        let generated = self.generated_ground_at(q, r);
        let (x, y) = axial_world(q, r);
        TileSnapshot {
            q,
            r,
            x,
            y,
            radius: HEX_RADIUS as u32,
            terrain: generated.presentation,
            height: generated.bed.get(),
            substrate: generated.substrate,
            water_depth: generated.hydrology.depth_quanta,
            discharge: generated.hydrology.discharge_class,
        }
    }

    /// Every cell of one surveyed chunk, in the chunk's own iteration order.
    fn chunk_terrain_snapshots(&self, chunk_q: i32, chunk_r: i32) -> Vec<TileSnapshot> {
        hexes_in_chunk(chunk_q, chunk_r, self.scenario.chunk_size)
            .map(|(q, r)| self.tile_snapshot(q, r))
            .collect()
    }

    fn terrain_snapshots(&self) -> Vec<TileSnapshot> {
        let mut tiles = Vec::new();
        for &(chunk_q, chunk_r) in &self.generated_chunks {
            tiles.extend(self.chunk_terrain_snapshots(chunk_q, chunk_r));
        }
        tiles
    }

    /// One field cell's snapshot, looked up by tile key. Used by the incremental path, which knows
    /// which cells moved but not where they sit in the overlay.
    fn resource_snapshot(&self, key: (i32, i32)) -> Option<ResourceSnapshot> {
        let field = self.field_at(key.0, key.1)?;
        let quantity = self.deposit_quantity(key);
        Some(resource_snapshot_of(
            key,
            field.item_id,
            quantity,
            field.initial_quantity,
        ))
    }

    /// Every field cell in the surveyed world, in tile order. Derived cells with no overlay still
    /// appear, because the host has to draw the field; only remaining quantity comes from the
    /// stored overlay.
    fn resource_snapshots(&self) -> Vec<ResourceSnapshot> {
        let mut resources = Vec::new();
        let size = self.scenario.chunk_size;
        for &(chunk_q, chunk_r) in &self.generated_chunks {
            for (q, r) in hexes_in_chunk(chunk_q, chunk_r, size) {
                if let Some(snapshot) = self.resource_snapshot((q, r)) {
                    if snapshot.quantity > 0 || self.tiles.contains_key(&(q, r)) {
                        resources.push(snapshot);
                    }
                }
            }
        }
        resources
    }

    fn delivered_by_item_snapshot(&self) -> Vec<Ingredient64> {
        self.delivered_by_item
            .iter()
            .map(|(&item_id, &quantity)| Ingredient64 { item_id, quantity })
            .collect()
    }

    fn contract_snapshot(&self) -> ContractSnapshot {
        let contract = &self.scenario.contract;
        let stage = contract.stages.get(self.contract_stage);
        ContractSnapshot {
            key: contract.key.clone(),
            name: contract.name.clone(),
            stage: self.contract_stage as u16,
            stages: contract.stages.len() as u16,
            stage_key: stage.map(|stage| stage.key.clone()).unwrap_or_default(),
            stage_name: stage.map(|stage| stage.name.clone()).unwrap_or_default(),
            stage_brief: stage.map(|stage| stage.brief.clone()).unwrap_or_default(),
            requirements: stage
                .map(|stage| {
                    stage
                        .requirements
                        .iter()
                        .map(|need| ContractRequirement {
                            item_id: need.item_id,
                            // Clamped natively, because the bar the host draws is a proportion and
                            // a surplus carried forward is not progress against this line.
                            delivered: self
                                .contract_contributed
                                .get(&need.item_id)
                                .copied()
                                .unwrap_or(0)
                                .min(u64::from(need.quantity))
                                as u32,
                            required: need.quantity,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            complete: self.contract_stage >= contract.stages.len(),
        }
    }

    /// The whole project catalogue, posted rows first in slot order, then the rest in catalogue
    /// order — with the price and the state on every row.
    ///
    /// Posted rows lead so the board can draw the first `REQUEST_SLOTS` entries without a filter
    /// and without the row a player is reading jumping when something further down completes.
    fn request_snapshots(&self) -> Vec<RequestSnapshot> {
        let posted: Vec<RequestId> = self.requests.iter().map(|state| state.request_id).collect();
        let row = |definition: &RequestDefinition, state: ProjectState| RequestSnapshot {
            key: definition.key.clone(),
            name: definition.name.clone(),
            brief: definition.brief.clone(),
            item_id: definition.item_id,
            delivered: self
                .project_delivered(definition.id)
                .min(definition.quantity),
            required: definition.quantity,
            insight: definition.insight,
            state,
        };
        let mut rows: Vec<RequestSnapshot> = posted
            .iter()
            .filter_map(|&id| self.request_definition(id))
            .map(|definition| row(definition, ProjectState::Posted))
            .collect();
        rows.extend(
            self.definitions
                .requests
                .iter()
                .filter(|definition| !posted.contains(&definition.id))
                .map(|definition| {
                    let state = if self.project_complete(definition.id) {
                        ProjectState::Complete
                    } else if self.item_reachable(definition.item_id, 0) {
                        ProjectState::Available
                    } else {
                        ProjectState::Locked
                    };
                    row(definition, state)
                }),
        );
        rows
    }

    /// The complete snapshot. It is the host's first frame and the oracle the incremental delta
    /// builder is pinned against; the shipped per-frame path no longer materializes one.
    fn snapshot(&mut self) -> Snapshot {
        let checksum = self.checksum();
        let chunks = self.chunk_snapshots();
        let terrain = self.terrain_snapshots();
        let resources = self.resource_snapshots();
        let buildings = self.entity_snapshots();
        Snapshot {
            scenario: self.scenario.key.clone(),
            scenario_name: self.scenario.name.clone(),
            world_version: WORLD_GENERATOR_VERSION,
            seed: self.seed,
            tick: self.tick,
            checksum,
            belt_transit_ticks: BELT_TRANSIT_TICKS as u32,
            delivered: self.delivered,
            delivered_by_item: self.delivered_by_item_snapshot(),
            insight: self.insight,
            victory: self.victory,
            contract: self.contract_snapshot(),
            requests: self.request_snapshots(),
            player: self.player_snapshot(),
            researched: self.researched.iter().copied().collect(),
            research_availability: self.research_availability_snapshot(),
            skills: self.skills_snapshot(),
            chunks,
            terrain,
            resources,
            buildings,
            boundaries: self.boundary_snapshot(),
            ground: self.ground_snapshot(),
            water: self.water.cells(),
            spoil: self.spoil,
            ground_items: self.ground_items.clone(),
            events: self.events.clone(),
        }
    }

    fn checksum(&self) -> u32 {
        self.checksum_for_world(WORLD_GENERATOR_VERSION)
    }

    fn checksum_for_world(&self, world_version: u16) -> u32 {
        let mut hash = 0x811c9dc5u32;
        hash_bytes(&mut hash, self.scenario.key.as_bytes());
        hash_u32(&mut hash, u32::from(world_version));
        hash_u32(&mut hash, self.seed);
        hash_world_params(&mut hash, &self.world_params);
        hash_u64(&mut hash, self.tick);
        hash_u64(&mut hash, self.delivered);
        hash_u64(&mut hash, self.insight);
        hash_u32(&mut hash, u32::from(self.victory));
        hash_i32(&mut hash, self.player.x);
        hash_i32(&mut hash, self.player.y);
        hash_i32(&mut hash, i32::from(self.player.facing_x));
        hash_i32(&mut hash, i32::from(self.player.facing_y));
        hash_i32(&mut hash, i32::from(self.player.move_x));
        hash_i32(&mut hash, i32::from(self.player.move_y));
        hash_u32(&mut hash, self.player.action_cooldown);
        // The swing that counter is measuring, so the two cannot be separated by an edit or by a
        // save. An idle player hashes nothing here, which is what keeps a file written before the
        // harvest became work — where no swing could be in flight — checksumming to the same value
        // it did then.
        if let Some(target) = self.pending_gather {
            hash_i32(&mut hash, target.q);
            hash_i32(&mut hash, target.r);
        }
        // Where a walk is headed, on the same terms as the swing above: it is an order the
        // simulation is still executing, so a run carrying one is not the same run as one standing
        // still, and a player who is not walking hashes nothing here. The route itself is derived
        // and is deliberately absent — it is rebuilt from this goal, so hashing it would be hashing
        // the same fact twice and pinning the search's internals into the save format.
        if let Some(goal) = self.player.walk_goal {
            hash_i32(&mut hash, goal.q);
            hash_i32(&mut hash, goal.r);
        }
        // Both of these are now run state rather than scenario state: creative changes what a
        // construction costs, and creative can widen the pack. A save that carried either without
        // hashing it could come back describing a different run than the one that was saved.
        hash_u32(&mut hash, self.player.carry_slots);
        hash_u32(&mut hash, u32::from(self.creative));
        for (&item, &quantity) in &self.player.inventory {
            hash_u32(&mut hash, u32::from(item));
            hash_u32(&mut hash, quantity);
        }
        if let Some(hand) = self.player.hand {
            hash_u32(&mut hash, u32::MAX - 20);
            hash_u32(&mut hash, u32::from(hand.item_id));
            hash_u32(&mut hash, hand.quantity);
        }
        hash_u32(&mut hash, u32::MAX);
        for &technology in &self.researched {
            hash_u32(&mut hash, u32::from(technology));
        }
        hash_u32(&mut hash, u32::MAX - 1);
        for &(chunk_q, chunk_r) in &self.generated_chunks {
            hash_i32(&mut hash, chunk_q);
            hash_i32(&mut hash, chunk_r);
        }
        for tile in self.tiles.values() {
            hash_i32(&mut hash, tile.q);
            hash_i32(&mut hash, tile.r);
            if let Some(resource) = &tile.resource {
                hash_u32(&mut hash, u32::from(resource.item_id));
                hash_u32(&mut hash, resource.quantity);
                hash_u32(&mut hash, resource.initial_quantity);
            } else {
                hash_u32(&mut hash, 0);
            }
        }
        let mut entities: Vec<&Entity> = self.entities.iter().collect();
        entities.sort_by_key(|entity| entity.id);
        for entity in entities {
            hash_u32(&mut hash, entity.id);
            hash_i32(&mut hash, entity.placed.q);
            hash_i32(&mut hash, entity.placed.r);
            hash_u32(&mut hash, u32::from(entity.placed.definition_id));
            hash_u32(&mut hash, u32::from(entity.placed.orientation));
            hash_u32(&mut hash, u32::from(entity.placed.recipe_id.unwrap_or(0)));
            hash_u32(&mut hash, u32::from(entity.placed.scenario_owned));
            hash_u32(&mut hash, entity.progress);
            hash_u32(&mut hash, entity.fuel_charge);
            hash_u32(&mut hash, entity.power_charge);
            hash_u32(&mut hash, entity.burn_progress);
            hash_u32(&mut hash, u32::from(entity.disabled));
            // Where each junction is in its rotation. A factory that reloaded with these reset
            // would deal its next round differently from the one that was saved, so they are as
            // much of the run's state as a machine's progress is.
            hash_u32(&mut hash, u32::from(entity.route_cursor));
            hash_u32(&mut hash, entity.merge_cursor);
            hash_inventory(&mut hash, &entity.inventory);
            hash_inventory(&mut hash, &entity.reserved_inputs);
            if !entity.input_inventory.is_empty() {
                hash_u32(&mut hash, u32::MAX - 21);
                hash_inventory(&mut hash, &entity.input_inventory);
            }
            if !entity.fuel_inventory.is_empty() {
                hash_u32(&mut hash, u32::MAX - 22);
                hash_inventory(&mut hash, &entity.fuel_inventory);
            }
            if !entity.output_inventory.is_empty() {
                hash_u32(&mut hash, u32::MAX - 23);
                hash_inventory(&mut hash, &entity.output_inventory);
            }
            if let Some(cargo) = entity.cargo {
                hash_u32(&mut hash, u32::from(cargo.item_id));
                hash_u32(&mut hash, cargo.quantity);
            } else {
                hash_u32(&mut hash, 0);
            }
            // What a belt is still carrying, and how far along it each item has got. Two factories
            // that agree about the exit slots and disagree about the four items behind them are not
            // the same factory, and they will not stay in step for a second: the tick each item
            // stepped on is what decides when it arrives. Written only when there is a lane, so
            // every checksum in the game that has no belt in flight is the one it always was.
            if !entity.lane.is_empty() {
                hash_u32(&mut hash, u32::MAX - 24);
                for item in &entity.lane {
                    hash_u32(&mut hash, u32::from(item.cargo.item_id));
                    hash_u32(&mut hash, item.cargo.quantity);
                    hash_u64(&mut hash, item.entered);
                }
            }
        }
        if !self.output_routes.is_empty() {
            hash_u32(&mut hash, u32::MAX - 31);
            for (&entity_id, routes) in &self.output_routes {
                hash_u32(&mut hash, entity_id);
                for (&item_id, route) in routes {
                    hash_u32(&mut hash, u32::from(item_id));
                    hash_i32(&mut hash, route.q);
                    hash_i32(&mut hash, route.r);
                    hash_u32(&mut hash, u32::from(route.direction));
                }
                hash_u32(&mut hash, u32::MAX);
            }
        }
        if !self.legacy_fluid_belts.is_empty() {
            hash_u32(&mut hash, u32::MAX - 32);
            for &entity_id in &self.legacy_fluid_belts {
                hash_u32(&mut hash, entity_id);
            }
        }
        for (&item, &quantity) in &self.delivered_by_item {
            hash_u32(&mut hash, u32::from(item));
            hash_u64(&mut hash, quantity);
        }
        hash_u32(&mut hash, u32::MAX - 2);
        hash_u64(&mut hash, self.contract_stage as u64);
        for (&item, &quantity) in &self.contract_contributed {
            hash_u32(&mut hash, u32::from(item));
            hash_u64(&mut hash, quantity);
        }
        hash_u32(&mut hash, u32::MAX - 3);
        for state in &self.requests {
            hash_u32(&mut hash, u32::from(state.request_id));
        }
        hash_u32(&mut hash, u32::MAX - 25);
        for (&request, &delivered) in &self.request_delivered {
            hash_u32(&mut hash, u32::from(request));
            hash_u32(&mut hash, delivered);
        }
        hash_u32(&mut hash, u32::MAX - 4);
        for (&request, &rounds) in &self.request_rounds {
            hash_u32(&mut hash, u32::from(request));
            hash_u32(&mut hash, rounds);
        }
        hash_u32(&mut hash, u32::MAX - 5);
        for (&request, &fills) in &self.request_fills {
            hash_u32(&mut hash, u32::from(request));
            hash_u32(&mut hash, fills);
        }
        if !self.ground_items.is_empty() {
            hash_u32(&mut hash, u32::MAX - 24);
            for item in &self.ground_items {
                hash_u32(&mut hash, item.id);
                hash_i32(&mut hash, item.q);
                hash_i32(&mut hash, item.r);
                hash_u32(&mut hash, u32::from(item.item_id));
                hash_u32(&mut hash, item.quantity);
                hash_u64(&mut hash, item.despawn_tick);
            }
        }
        if !self.boundaries.is_empty() {
            hash_u32(&mut hash, u32::MAX - 29);
            hash_u32(&mut hash, self.boundary_state_hash());
        }
        // Guarded on emptiness for the same reason: a run that has never touched the ground hashes
        // exactly what it hashed a release ago, so v0.37 files keep their checksums.
        if !self.ground.is_empty() || self.spoil != 0 {
            hash_u32(&mut hash, u32::MAX - 30);
            hash_u32(&mut hash, self.ground_state_hash());
        }
        // Guarded on the same rule once more. Disturbed water is the departure set and nothing else,
        // so a world at its generated equilibrium hashes what it hashed before this field existed,
        // and every save 38 file keeps the checksum it was written with. The envelope moves when a
        // player can first create a departure, not when native learns to carry one.
        if !self.water.is_empty() {
            hash_u32(&mut hash, u32::MAX - 33);
            self.water.hash_into(&mut hash);
        }
        if !self.bank_stress.is_empty() {
            hash_u32(&mut hash, u32::MAX - 34);
            self.bank_stress.hash_into(&mut hash);
        }
        self.skills.hash(&mut hash);
        hash
    }

    fn save_string(&self) -> Result<String, String> {
        let state = SavedState {
            seed: self.seed,
            world_params: self.world_params.clone(),
            generated_chunks: self
                .generated_chunks
                .iter()
                .map(|&(q, r)| Coordinate { q, r })
                .collect(),
            tiles: self.tiles.values().cloned().collect(),
            entities: self.entities.clone(),
            output_routes: self.output_routes.clone(),
            legacy_fluid_belts: self.legacy_fluid_belts.clone(),
            player: self.player.clone(),
            pending_gather: self.pending_gather,
            researched: self.researched.clone(),
            skills: self.skills.clone(),
            next_entity_id: self.next_entity_id,
            tick: self.tick,
            delivered: self.delivered,
            delivered_by_item: self.delivered_by_item.clone(),
            insight: self.insight,
            victory: self.victory,
            contract_stage: self.contract_stage,
            contract_contributed: self.contract_contributed.clone(),
            requests: self.requests.clone(),
            request_rounds: self.request_rounds.clone(),
            request_fills: self.request_fills.clone(),
            request_delivered: self.request_delivered.clone(),
            produced: self.produced.clone(),
            creative: self.creative,
            boundaries: self.boundary_snapshot(),
            ground: self.ground_snapshot(),
            water: self.water.cells(),
            bank_stress: self.bank_stress.cells(),
            spoil: self.spoil,
            ground_items: self.ground_items.clone(),
            next_ground_item_id: self.next_ground_item_id,
        };
        let envelope = SaveEnvelope {
            save_version: SAVE_VERSION,
            world_generator_version: WORLD_GENERATOR_VERSION,
            definition_version: self.definitions.version,
            technology_version: self.technologies.version,
            scenario_key: self.scenario.key.clone(),
            scenario_version: self.scenario.version,
            checksum: self.checksum(),
            state,
        };
        serde_json::to_string(&envelope)
            .map(|json| format!("{SAVE_PREFIX}{json}"))
            .map_err(|error| error.to_string())
    }

    fn from_save(
        definitions: &DefinitionsInput,
        technologies: &TechnologiesInput,
        scenarios: &ScenariosInput,
        save: &str,
    ) -> Result<Self, String> {
        let json = save
            .strip_prefix(SAVE_PREFIX)
            .ok_or("save must begin with HXF1")?;
        // Verify the original world stamp before moving a legacy run onto the current envelope.
        // Its saved site table is unchanged: adding oil must not reroll an existing landscape.
        let original: serde_json::Value =
            serde_json::from_str(json).map_err(|error| error.to_string())?;
        let original_world = original
            .get("world_generator_version")
            .and_then(serde_json::Value::as_u64)
            .and_then(|version| u16::try_from(version).ok())
            .ok_or("save has no valid world version")?;
        let original_save_version = original
            .get("save_version")
            .and_then(serde_json::Value::as_u64)
            .and_then(|version| u16::try_from(version).ok())
            .ok_or("save has no valid save version")?;
        if original_save_version <= 36 && SAVE_VERSION >= 37 {
            return Err(
                "this factory was built at one square metre per hex; export the file to keep a copy. New worlds use a 25 m² hex"
                    .into(),
            );
        }
        let migrated = save_migrations::migrate(json, SAVE_VERSION)?;
        let legacy_component_bill = matches!(migrated, std::borrow::Cow::Owned(_));
        let envelope: SaveEnvelope = serde_json::from_str(&migrated)
            .map_err(|error| format!("malformed HXF1 save: {error}"))?;
        if envelope.world_generator_version != WORLD_GENERATOR_VERSION {
            return Err(
                "this factory stands on a world this build no longer generates; export the file to keep a copy"
                    .into(),
            );
        }
        if envelope.definition_version != definitions.version
            || envelope.technology_version != technologies.version
        {
            return Err("save definitions are incompatible".into());
        }
        let scenario = scenarios
            .scenarios
            .iter()
            .find(|scenario| scenario.key == envelope.scenario_key)
            .ok_or("save scenario is unavailable")?;
        if scenario.version != envelope.scenario_version {
            return Err("save scenario version is incompatible".into());
        }
        let mut core = Core::initialize(
            definitions,
            technologies,
            scenario,
            Some(envelope.state.seed),
            Some(envelope.state.world_params.clone()),
            false,
        )?;
        validate_saved_state(
            definitions,
            technologies,
            scenario,
            &envelope.state,
            legacy_component_bill,
        )?;
        let restored_legacy_fluid_belts = envelope.state.legacy_fluid_belts.clone();
        core.seed = envelope.state.seed;
        core.world_params = envelope.state.world_params;
        // The lattice and the bootstrap table are derived from exactly these two, so they are
        // rebuilt the moment either moves rather than carried in the file.
        core.fields = WorldFields::new(&core.world_params, core.seed, &core.ground_spine);
        core.generated_chunks = envelope
            .state
            .generated_chunks
            .iter()
            .map(|coordinate| (coordinate.q, coordinate.r))
            .collect();
        core.ground_spine
            .rebuild_cache(&core.generated_chunks, core.scenario.chunk_size);
        core.tiles = envelope
            .state
            .tiles
            .into_iter()
            .map(|tile| ((tile.q, tile.r), tile))
            .collect();
        core.deposit_links.clear();
        // Regrowth is derived from the overlay the save just restored, so it is recovered here
        // rather than carried in the file.
        core.rebuild_flora_regrowth();
        // Undo history is session state, not saved state: a restored save has nothing to take back.
        core.undo_stack.clear();
        core.entities = envelope.state.entities;
        core.output_routes = envelope.state.output_routes;
        if original_save_version >= SAVE_VERSION {
            core.legacy_fluid_belts = restored_legacy_fluid_belts.clone();
        }
        // A save records entities in stable id order; sorting makes that an invariant of the loaded
        // core rather than a property of the file. Entity order is not a simulation input — the
        // checksum and every arbitration order sort by id — so this cannot change a result.
        core.entities.sort_by_key(|entity| entity.id);
        core.player = envelope.state.player;
        core.pending_gather = envelope.state.pending_gather;
        core.researched = envelope.state.researched;
        core.skills = envelope.state.skills;
        // Restored directly rather than through set_creative: the saved researched set is the
        // checksum truth. A migrated creative save is upgraded only after that original truth has
        // been verified below.
        core.creative = envelope.state.creative;
        core.next_entity_id = envelope.state.next_entity_id;
        core.tick = envelope.state.tick;
        core.delivered = envelope.state.delivered;
        core.delivered_by_item = envelope.state.delivered_by_item;
        core.insight = envelope.state.insight;
        core.victory = envelope.state.victory;
        core.contract_stage = envelope.state.contract_stage;
        core.contract_contributed = envelope.state.contract_contributed;
        // The board is restored, never redrawn: `Core::new` posted one for a fresh run and this run
        // is not fresh. A redraw would hand a finished game three requests it may already have
        // filled, and the checksum below would be the first thing to say so.
        core.requests = envelope.state.requests;
        core.request_rounds = envelope.state.request_rounds;
        core.request_fills = envelope.state.request_fills;
        core.request_delivered = envelope.state.request_delivered;
        core.last_action_cooldown_total = core.player.action_cooldown;
        core.produced = envelope.state.produced;
        core.boundaries = envelope
            .state
            .boundaries
            .into_iter()
            .map(|b| (b.segment, b))
            .collect();
        core.ground = envelope
            .state
            .ground
            .into_iter()
            .map(|cell| ((cell.q, cell.r), cell))
            .collect();
        core.water = hydrology::DisturbedWater::from_cells(&envelope.state.water);
        core.bank_stress = geomorphology::BankStress::from_cells(&envelope.state.bank_stress);
        core.spoil = envelope.state.spoil;
        core.ground_items = envelope.state.ground_items;
        core.next_ground_item_id = envelope.state.next_ground_item_id.max(
            core.ground_items
                .iter()
                .map(|item| item.id.saturating_add(1))
                .max()
                .unwrap_or(1),
        );
        core.events = vec!["HXF1 save restored".into()];
        if core.checksum_for_world(original_world) != envelope.checksum {
            return Err("save checksum does not match its native state".into());
        }
        // A v35 checksum knew nothing about this compatibility set. Apply it only after that
        // original state has passed tamper detection; the next v36 save hashes the new fact.
        if original_save_version < SAVE_VERSION {
            core.legacy_fluid_belts = restored_legacy_fluid_belts;
        }
        // Verify saved facts before rebuilding derived topology and route caches.
        core.compile_graph();
        // v0.33 asks for one component instead of three. Honor existing contributions only after
        // verifying their saved checksum, through the ordinary consumption/grant path. Completed
        // commissions are never replayed and any surplus stays credited at the hub.
        if legacy_component_bill && core.scenario.key == "new-game" {
            core.advance_contract_with_rewards(false);
        }
        if legacy_component_bill {
            core.migrate_player_skills();
        }
        // Creative means the whole current tree, including technologies added after the save was
        // written. Verify the saved state first, then extend it through the ordinary capability
        // path; this preserves tamper detection without leaving an older creative world partially
        // locked. A current save already containing the whole tree is unchanged.
        if core.creative {
            core.grant_creative_skills();
            for technology in &core.technologies.technologies {
                core.researched.insert(technology.id);
            }
            core.apply_research_effects();
        }
        core.player.move_x = 0;
        core.player.move_y = 0;
        Ok(core)
    }
}

#[derive(Serialize, Deserialize)]
struct SaveEnvelope {
    save_version: u16,
    world_generator_version: u16,
    definition_version: u16,
    technology_version: u16,
    scenario_key: String,
    scenario_version: u16,
    checksum: u32,
    state: SavedState,
}

#[derive(Serialize, Deserialize)]
struct SavedState {
    #[serde(default)]
    boundaries: Vec<Boundary>,
    #[serde(default)]
    ground: Vec<GroundCell>,
    /// Cells whose water has left the generated equilibrium. Defaulted, because a file written
    /// before water could be disturbed describes a world that never departed from it — which is
    /// exactly an empty set, not a missing one.
    #[serde(default)]
    water: Vec<hydrology::WaterCell>,
    /// Non-zero outside-bank stress. Version 40 had none and therefore defaults to the empty set.
    #[serde(default)]
    bank_stress: Vec<geomorphology::StressCell>,
    #[serde(default)]
    spoil: u64,
    seed: u32,
    /// Beside the seed, because a world is both. The overlay a save carries is only meaningful
    /// against the generation it was cut from.
    world_params: WorldParams,
    generated_chunks: Vec<Coordinate>,
    tiles: Vec<TileState>,
    entities: Vec<Entity>,
    #[serde(default)]
    output_routes: BTreeMap<u32, BTreeMap<ItemId, OutputRoute>>,
    #[serde(default)]
    legacy_fluid_belts: BTreeSet<u32>,
    player: PlayerState,
    /// The hex a swing in flight is working. Optional and defaulted, because a save written before
    /// a harvest cost work has no swing to carry and reads back as an idle player — and because
    /// `checksum` hashes an absent one as nothing, such a file still checksums to what it did when
    /// it was written.
    #[serde(default)]
    pending_gather: Option<Coordinate>,
    researched: BTreeSet<TechnologyId>,
    #[serde(default)]
    skills: SkillsState,
    next_entity_id: u32,
    tick: u64,
    delivered: u64,
    delivered_by_item: BTreeMap<ItemId, u64>,
    insight: u64,
    victory: bool,
    contract_stage: usize,
    contract_contributed: BTreeMap<ItemId, u64>,
    requests: Vec<RequestState>,
    request_rounds: BTreeMap<RequestId, u32>,
    #[serde(default)]
    request_fills: BTreeMap<RequestId, u32>,
    /// Progress against each project, moved out of the posted slots at save 27 so a pass no longer
    /// destroys it. Defaulted rather than required: the migration writes it, and a file that
    /// somehow lacks it describes a run with nothing part-delivered.
    #[serde(default)]
    request_delivered: BTreeMap<RequestId, u32>,
    produced: BTreeMap<ItemId, u64>,
    /// Whether the run was creative. Checksummed like the rest of this struct, so it cannot be
    /// edited out of a file to turn a creative run back into a priced one.
    creative: bool,
    #[serde(default)]
    ground_items: Vec<GroundItem>,
    #[serde(default)]
    next_ground_item_id: u32,
}

/// The snapshot state the host was last sent, retained so the next delta can be built from the
/// core's dirty marks instead of a freshly materialized snapshot.
///
/// The cheap groups are kept by value and compared directly. Buildings are kept keyed by stable id,
/// so one marked entity costs one rebuild and one comparison rather than a rebuild of the whole
/// blueprint. Terrain and resources are kept by neither: generation is the only path that adds
/// either, and it marks them, so the marks alone are exact.
#[derive(Clone, Debug)]
struct SnapshotBaseline {
    boundaries: Vec<Boundary>,
    ground: Vec<GroundCell>,
    water: Vec<hydrology::WaterCell>,
    spoil: u64,
    scenario: String,
    scenario_name: String,
    world_version: u16,
    seed: u32,
    delivered: u64,
    delivered_by_item: Vec<Ingredient64>,
    insight: u64,
    victory: bool,
    contract: ContractSnapshot,
    requests: Vec<RequestSnapshot>,
    player: PlayerSnapshot,
    researched: Vec<TechnologyId>,
    research_availability: Vec<ResearchAvailability>,
    skills: SkillsSnapshot,
    chunks: Vec<ChunkSnapshot>,
    buildings: BTreeMap<u32, EntitySnapshot>,
    ground_items: Vec<GroundItem>,
    events: Vec<String>,
}

impl SnapshotBaseline {
    fn from_snapshot(snapshot: &Snapshot) -> Self {
        Self {
            scenario: snapshot.scenario.clone(),
            scenario_name: snapshot.scenario_name.clone(),
            world_version: snapshot.world_version,
            seed: snapshot.seed,
            delivered: snapshot.delivered,
            delivered_by_item: snapshot.delivered_by_item.clone(),
            insight: snapshot.insight,
            victory: snapshot.victory,
            contract: snapshot.contract.clone(),
            requests: snapshot.requests.clone(),
            player: snapshot.player.clone(),
            researched: snapshot.researched.clone(),
            research_availability: snapshot.research_availability.clone(),
            skills: snapshot.skills.clone(),
            chunks: snapshot.chunks.clone(),
            buildings: snapshot
                .buildings
                .iter()
                .map(|entity| (entity.id, entity.clone()))
                .collect(),
            boundaries: snapshot.boundaries.clone(),
            ground: snapshot.ground.clone(),
            water: snapshot.water.clone(),
            spoil: snapshot.spoil,
            ground_items: snapshot.ground_items.clone(),
            events: snapshot.events.clone(),
        }
    }
}

/// Advance one baseline field, yielding the delta entry only when it actually changed.
fn take_changed<T: Clone + PartialEq>(baseline: &mut T, current: T) -> Option<T> {
    (*baseline != current).then(|| {
        baseline.clone_from(&current);
        current
    })
}

fn take_changed_copy<T: Copy + PartialEq>(baseline: &mut T, current: T) -> Option<T> {
    (*baseline != current).then(|| {
        *baseline = current;
        current
    })
}

#[wasm_bindgen]
pub struct Factory {
    definitions: DefinitionsInput,
    technologies: TechnologiesInput,
    scenarios: ScenariosInput,
    core: Core,
    snapshot_revision: u64,
    baseline: Option<SnapshotBaseline>,
}

impl Factory {
    /// The next delta, built from the core's dirty marks against the baseline the host holds.
    ///
    /// With no baseline — the first delta, and every reset, new game, and load — the host has no
    /// state to patch, so it gets a complete replacement. Otherwise only marked entries are
    /// materialized at all, and only those that genuinely differ from the baseline travel.
    fn build_delta(&mut self) -> SnapshotDelta {
        let base_revision = self.snapshot_revision;
        let revision = base_revision.saturating_add(1);
        self.snapshot_revision = revision;

        if self.baseline.is_none() {
            let snapshot = self.core.snapshot();
            self.baseline = Some(SnapshotBaseline::from_snapshot(&snapshot));
            self.core.dirty = SnapshotDirty::default();
            return SnapshotDelta::full(base_revision, revision, &snapshot);
        }

        let core = &mut self.core;
        let baseline = self.baseline.as_mut().expect("baseline exists");
        let mut dirty = std::mem::take(&mut core.dirty);
        let marked_entities = drain_marks(&mut dirty.entities);
        let marked_resources = drain_marks(&mut dirty.resources);
        let marked_terrain = drain_marks(&mut dirty.terrain);

        let mut removed: Vec<u32> = Vec::new();
        for id in &dirty.removed {
            if baseline.buildings.remove(id).is_some() {
                removed.push(*id);
            }
        }
        let mut changed: Vec<EntitySnapshot> = Vec::new();
        for id in marked_entities {
            // Ids are monotonic, so an erased id never returns and needs no rebuild.
            if !dirty.removed.is_empty() && dirty.removed.contains(&id) {
                continue;
            }
            let Some(index) = core.index_of_entity(id) else {
                continue;
            };
            // A mark only says an entry may have moved. Comparing against what the host already
            // holds is what keeps a conservative mark from becoming wasted payload.
            let entity = core.entity_snapshot(index);
            if baseline.buildings.get(&id) != Some(&entity) {
                baseline.buildings.insert(id, entity.clone());
                changed.push(entity);
            }
        }
        // Both lists are in ascending id order, so the host merges them in one linear pass.
        let buildings = (!changed.is_empty() || !removed.is_empty()).then_some(BuildingsDelta {
            replace: false,
            changed,
            removed,
        });

        let resources = if dirty.resources_replace {
            Some(ResourcesDelta {
                replace: true,
                changed: core.resource_snapshots(),
            })
        } else {
            let changed: Vec<ResourceSnapshot> = marked_resources
                .into_iter()
                .filter_map(|key| core.resource_snapshot(key))
                .collect();
            (!changed.is_empty()).then_some(ResourcesDelta {
                replace: false,
                changed,
            })
        };

        let terrain = {
            let changed: Vec<TileSnapshot> = marked_terrain
                .into_iter()
                .flat_map(|(chunk_q, chunk_r)| core.chunk_terrain_snapshots(chunk_q, chunk_r))
                .collect();
            (!changed.is_empty()).then_some(TerrainDelta {
                replace: false,
                changed,
            })
        };

        let ground_items = if dirty.ground_items || baseline.ground_items != core.ground_items {
            baseline.ground_items = core.ground_items.clone();
            Some(core.ground_items.clone())
        } else {
            None
        };

        let research_availability = if baseline.insight != core.insight
            || !baseline
                .researched
                .iter()
                .copied()
                .eq(core.researched.iter().copied())
        {
            take_changed(
                &mut baseline.research_availability,
                core.research_availability_snapshot(),
            )
        } else {
            None
        };

        SnapshotDelta {
            base_revision,
            revision,
            research_availability,
            skills: if baseline.skills.state != core.skills
                || baseline.player.state.carry_slots != core.player.carry_slots
                || baseline.player.state.build_range != core.player.build_range
            {
                take_changed(&mut baseline.skills, core.skills_snapshot())
            } else {
                None
            },
            tick: core.tick,
            checksum: core.checksum(),
            belt_transit_ticks: BELT_TRANSIT_TICKS as u32,
            scenario: take_changed(&mut baseline.scenario, core.scenario.key.clone()),
            scenario_name: take_changed(&mut baseline.scenario_name, core.scenario.name.clone()),
            world_version: take_changed_copy(&mut baseline.world_version, WORLD_GENERATOR_VERSION),
            seed: take_changed_copy(&mut baseline.seed, core.seed),
            delivered: take_changed_copy(&mut baseline.delivered, core.delivered),
            delivered_by_item: take_changed(
                &mut baseline.delivered_by_item,
                core.delivered_by_item_snapshot(),
            ),
            insight: take_changed_copy(&mut baseline.insight, core.insight),
            victory: take_changed_copy(&mut baseline.victory, core.victory),
            contract: take_changed(&mut baseline.contract, core.contract_snapshot()),
            requests: take_changed(&mut baseline.requests, core.request_snapshots()),
            player: take_changed(&mut baseline.player, core.player_snapshot()),
            researched: take_changed(
                &mut baseline.researched,
                core.researched.iter().copied().collect(),
            ),
            chunks: dirty
                .chunks
                .then(|| take_changed(&mut baseline.chunks, core.chunk_snapshots()))
                .flatten(),
            // Terrain is never retained for comparison: `generate_chunk` is the only path that can
            // add a tile, nothing ever changes or removes one, and the mark names the chunks it
            // added. The surveyed-chunk set is ordered, and so are the marks, so the tiles travel in
            // the same order a full snapshot would have listed them in.
            terrain,
            resources,
            buildings,
            ground_items,
            boundaries: dirty
                .boundaries
                .then(|| take_changed(&mut baseline.boundaries, core.boundary_snapshot()))
                .flatten(),
            ground: dirty
                .ground
                .then(|| take_changed(&mut baseline.ground, core.ground_snapshot()))
                .flatten(),
            // Spoil is a single number and the tray shows it on every preview, so it is compared
            // rather than marked: the comparison is cheaper than the mark would be.
            spoil: (baseline.spoil != core.spoil).then(|| {
                baseline.spoil = core.spoil;
                core.spoil
            }),
            water: dirty
                .water
                .then(|| take_changed(&mut baseline.water, core.water.cells()))
                .flatten(),
            events: take_changed(&mut baseline.events, core.events.clone()),
        }
    }
}

/// The largest preview native will raster, per side. A preview is a picture on a settings panel
/// rather than a viewport, and one pixel of it costs seven elevations, so the ceiling lives here
/// rather than in whatever asks for one.
const MAX_PREVIEW_SIDE: u32 = 480;
/// The widest span a preview may frame, in hexes. Sized well above the largest shipped landform
/// cell rather than to a round number: a preview that could not frame one landform would be a
/// picture of noise.
const MAX_PREVIEW_SPAN: u32 = 16_384;
/// How many deposits a preview will plot before it reports a count instead.
///
/// Deposits stand a dozen hexes apart, so a window wide enough to frame a coastline holds tens of
/// thousands of them. Drawn, that is a texture rather than a map, and sent, it is a megabyte of JSON
/// per slider nudge. Past this the overlay says how many there are and leaves the terrain visible.
const MAX_PREVIEW_SITES: usize = 1_200;
/// The most lattice cells a preview will walk. A span of `MAX_PREVIEW_SPAN` over a `site_cell` of
/// one is a legal parameter set and tens of millions of cells; the count above is a property of what
/// came out, and this is the bound on the looking.
const MAX_PREVIEW_SITE_CELLS: i64 = 262_144;

/// One deposit site as a preview draws it: where its centre lands in preview pixels, how far it
/// reaches there, and what it holds.
#[derive(Serialize)]
struct PreviewSite {
    item_id: ItemId,
    x: i32,
    y: i32,
    radius: i32,
}

/// Why one guarantee could not be placed, in the terms the panel explains it in.
#[derive(Serialize)]
struct PreviewNeed {
    item_id: ItemId,
    /// The bands a rule could seat this material's centre in.
    bands: Vec<Terrain>,
    /// Whether the opening holds any of those bands at all. False is "this world has no such ground
    /// near the landing site", which no seed will fix; true is "the ground is there and no patch on
    /// it was big enough", which one often will.
    ground: bool,
}

/// One knob a repair turns, named as the form names it so the host can label it without a table of
/// its own.
#[derive(Serialize)]
struct PreviewChange {
    field: &'static str,
    from: i32,
    to: i32,
}

/// A verified way out of a world that cannot be started. Both halves are optional and both may be
/// present: they are two different prices, and which one is worth paying is the player's call.
#[derive(Serialize)]
struct PreviewRepair {
    /// A seed that opens the world with every parameter left where the player put it.
    seed: Option<u32>,
    /// Parameter changes that open the world with the seed left alone. Empty when the ladder found
    /// nothing, which is itself worth saying: it means the shape of this world is the problem.
    changes: Vec<PreviewChange>,
}

#[derive(Serialize)]
struct PreviewSites {
    sites: Vec<PreviewSite>,
    /// Deposits the window holds, which is not always how many of them are in `sites`.
    total: u32,
    /// Whether the window holds more deposits than are worth drawing, or more than were counted.
    /// Set either way, so an empty `sites` never has to be read as "this world has no deposits".
    dense: bool,
    /// Materials the bootstrap pass could not place anywhere. `Core::new` refuses a world over
    /// exactly this list, so it travels with the picture rather than being discovered on start.
    unmet: Vec<ItemId>,
    /// What each of those materials was looking for. Empty whenever `unmet` is.
    needs: Vec<PreviewNeed>,
    /// A way out, when one was found. Searched only for a world that is already refused, so a
    /// parameter set that opens costs nothing to preview.
    repair: Option<PreviewRepair>,
}

/// Every scalar a parameter set carries, under the name the form gives it. `site_rules` is the one
/// field left out: it is a table rather than a knob, and nothing here moves it.
const WORLD_SCALARS: [(&str, fn(&WorldParams) -> i32); 17] = [
    ("elevation_coarse_cell", |p| p.elevation_coarse_cell),
    ("elevation_fine_cell", |p| p.elevation_fine_cell),
    ("elevation_coarse_weight", |p| p.elevation_coarse_weight),
    ("moisture_cell", |p| p.moisture_cell),
    ("richness_cell", |p| p.richness_cell),
    ("water_level", |p| p.water_level),
    ("shore_level", |p| p.shore_level),
    ("hills_level", |p| p.hills_level),
    ("highland_level", |p| p.highland_level),
    ("cliff_step", |p| p.cliff_step),
    ("deep_water_moisture", |p| p.deep_water_moisture),
    ("site_cell", |p| p.site_cell),
    ("site_jitter", |p| p.site_jitter),
    ("river_cell", |p| p.river_cell),
    ("river_width", |p| p.river_width),
    ("river_max_elevation", |p| p.river_max_elevation),
    ("ocean_level", |p| p.ocean_level),
];

/// What a repair did, as a diff rather than as a list the repair writes for itself. A move that
/// turned a knob nobody expected still reports that knob, which is the property worth having: the
/// button says what it is about to change because the change is read off the result.
fn world_changes(before: &WorldParams, after: &WorldParams) -> Vec<PreviewChange> {
    WORLD_SCALARS
        .iter()
        .filter_map(|&(field, read)| {
            let (from, to) = (read(before), read(after));
            (from != to).then_some(PreviewChange { field, from, to })
        })
        .collect()
}

impl Factory {
    /// The parameters, the clamped preview size, and the world units one preview pixel covers.
    ///
    /// Shared by both preview exports so the terrain raster and the site overlay are pictures of
    /// one window: two windows a pixel apart would be an overlay that does not line up.
    fn preview_window(
        &self,
        world_params_json: &str,
        width: u32,
        height: u32,
        hexes_across: u32,
    ) -> Result<(WorldParams, i32, i32, i64), String> {
        let params = world_params_from_json(world_params_json)?;
        // The same gate `Core::new` puts a new world through, so the panel cannot draw a set the
        // start button would refuse — and so a slider mid-drag cannot hand the generator a cell
        // size of zero to divide by.
        params.validate(&self.definitions)?;
        let width = width.clamp(1, MAX_PREVIEW_SIDE) as i32;
        let height = height.clamp(1, MAX_PREVIEW_SIDE) as i32;
        let across = i64::from(hexes_across.clamp(1, MAX_PREVIEW_SPAN));
        let step = (across * i64::from(HEX_X) / i64::from(width)).max(1);
        Ok((params, width, height, step))
    }

    /// The terrain raster behind {@link Factory::world_preview_bytes}, failing in `String` so a
    /// native test can drive the refusal as well as the picture.
    fn preview_cells(
        &self,
        world_params_json: &str,
        seed: u32,
        width: u32,
        height: u32,
        hexes_across: u32,
    ) -> Result<Vec<u8>, String> {
        let (params, width, height, step) =
            self.preview_window(world_params_json, width, height, hexes_across)?;
        let mut cells = Vec::with_capacity((width * height) as usize);
        for py in 0..height {
            let y = (i64::from(py) - i64::from(height) / 2) * step;
            for px in 0..width {
                let x = (i64::from(px) - i64::from(width) / 2) * step;
                let (q, r) = hex_at_world(x, y);
                cells.push(terrain_at(&params, seed, q, r, true) as u8);
            }
        }
        Ok(cells)
    }

    /// The deposit overlay behind {@link Factory::world_preview_sites_json}, on the same terms.
    fn preview_sites(
        &self,
        world_params_json: &str,
        seed: u32,
        width: u32,
        height: u32,
        hexes_across: u32,
    ) -> Result<PreviewSites, String> {
        let (params, width, height, step) =
            self.preview_window(world_params_json, width, height, hexes_across)?;
        let spine = GroundSpine::physical(&params, seed, true);
        let fields = WorldFields::new(&params, seed, &spine);
        // The lattice cells the window can see, from the axial extent of its four corners. A site
        // wanders inside its own cell by `site_jitter` and reaches out by `radius_max`, so the
        // range is widened by `reach` — the same derivation `field_at` scans with, for the same
        // reason: a range one cell short drops deposits off the edge of the picture in silence.
        let corners = [
            (0, 0),
            (width - 1, 0),
            (0, height - 1),
            (width - 1, height - 1),
        ];
        let cells: Vec<(i32, i32)> = corners
            .iter()
            .map(|&(px, py)| {
                let x = (i64::from(px) - i64::from(width) / 2) * step;
                let y = (i64::from(py) - i64::from(height) / 2) * step;
                let (q, r) = hex_at_world(x, y);
                (
                    floor_div(q, params.site_cell),
                    floor_div(r, params.site_cell),
                )
            })
            .collect();
        let min_q = cells.iter().map(|cell| cell.0).min().unwrap_or(0) - fields.reach - 1;
        let max_q = cells.iter().map(|cell| cell.0).max().unwrap_or(0) + fields.reach + 1;
        let min_r = cells.iter().map(|cell| cell.1).min().unwrap_or(0) - fields.reach - 1;
        let max_r = cells.iter().map(|cell| cell.1).max().unwrap_or(0) + fields.reach + 1;
        let unmet: Vec<ItemId> = fields.unmet.iter().map(|&(item_id, _)| item_id).collect();
        let (needs, repair) = self.preview_diagnosis(&params, seed, &unmet);
        // The bootstrap verdict does not depend on the scan, so a window too wide to walk still
        // reports whether the world can be started at all — and how to fix it.
        if i64::from(max_q - min_q + 1) * i64::from(max_r - min_r + 1) > MAX_PREVIEW_SITE_CELLS {
            return Ok(PreviewSites {
                sites: Vec::new(),
                total: 0,
                dense: true,
                unmet,
                needs,
                repair,
            });
        }
        let mut sites = Vec::new();
        for cell_q in min_q..=max_q {
            for cell_r in min_r..=max_r {
                let Some(site) = fields.site_at((cell_q, cell_r), &spine) else {
                    continue;
                };
                let (x, y) = axial_world(site.center.0, site.center.1);
                sites.push(PreviewSite {
                    item_id: params.site_rules[site.rule].item_id,
                    x: (i64::from(x) / step + i64::from(width) / 2) as i32,
                    y: (i64::from(y) / step + i64::from(height) / 2) as i32,
                    // Hexes to pixels through the same step, so a patch covering a tenth of the
                    // window is drawn covering a tenth of the window.
                    radius: (i64::from(site.radius) * i64::from(HEX_X) / step).max(1) as i32,
                });
            }
        }
        let total = sites.len() as u32;
        let dense = sites.len() > MAX_PREVIEW_SITES;
        if dense {
            sites.clear();
        }
        Ok(PreviewSites {
            sites,
            total,
            dense,
            unmet,
            needs,
            repair,
        })
    }

    /// Why a world was refused, and a way out of it, or nothing at all when it was not refused.
    ///
    /// Both halves are searched here rather than by the host because both are answers about the
    /// generator: the bands come from this world's own rules, and every repair offered has been put
    /// through a real bootstrap pass. Nothing is proposed on the strength of the reasoning that
    /// produced it.
    ///
    /// The cost is paid only by a world that already cannot be started, so a parameter set that
    /// opens previews at the price it always did.
    fn preview_diagnosis(
        &self,
        params: &WorldParams,
        seed: u32,
        unmet: &[ItemId],
    ) -> (Vec<PreviewNeed>, Option<PreviewRepair>) {
        if unmet.is_empty() {
            return (Vec::new(), None);
        }
        let spine = GroundSpine::physical(params, seed, true);
        let census = bootstrap_band_census(params, seed, &spine);
        let needs = unmet
            .iter()
            .map(|&item_id| {
                let bands = bootstrap_bands(params, item_id);
                PreviewNeed {
                    ground: bands.iter().any(|band| census.contains(band)),
                    item_id,
                    bands,
                }
            })
            .collect();
        let repair = PreviewRepair {
            seed: repair_seed(params, seed),
            changes: repair_params(params, seed)
                .map(|fixed| world_changes(params, &fixed))
                .unwrap_or_default(),
        };
        // A repair with neither half is not a repair; saying so lets the panel fall back to the
        // hint rather than offering a button that does nothing.
        let repair = (repair.seed.is_some() || !repair.changes.is_empty()).then_some(repair);
        (needs, repair)
    }
}

#[wasm_bindgen]
impl Factory {
    #[wasm_bindgen(constructor)]
    pub fn new(
        definitions_json: &str,
        technologies_json: &str,
        scenarios_json: &str,
        scenario_key: &str,
        seed_override: Option<u32>,
        world_params_json: Option<String>,
        creative: Option<bool>,
    ) -> Result<Factory, JsValue> {
        let definitions: DefinitionsInput = parse_json(definitions_json)?;
        let technologies: TechnologiesInput = parse_json(technologies_json)?;
        let scenarios: ScenariosInput = parse_json(scenarios_json)?;
        validate_all(&definitions, &technologies, &scenarios).map_err(js_error)?;
        let scenario = scenarios
            .scenarios
            .iter()
            .find(|scenario| scenario.key == scenario_key)
            .ok_or_else(|| js_error(format!("unknown scenario {scenario_key}")))?;
        let world_params = parse_world_params(world_params_json.as_deref())?;
        let mut core = Core::new(
            &definitions,
            &technologies,
            scenario,
            seed_override,
            world_params,
        )
        .map_err(js_error)?;
        // Set after construction rather than threaded through Core::new: creative is a switch the
        // run can throw at any time, so the opening state is the same thing as throwing it on tick
        // zero and there is one implementation of what creative does rather than two.
        core.set_creative(creative.unwrap_or(false));
        Ok(Factory {
            definitions,
            technologies,
            scenarios,
            core,
            snapshot_revision: 0,
            baseline: None,
        })
    }

    pub fn tick(&mut self, count: u32) {
        self.core.tick_many(count.min(240));
    }

    pub fn reset(&mut self) -> Result<(), JsValue> {
        // Reset restarts the run, not the mode: a creative sandbox that came back priced would be
        // the one button a creative player cannot press.
        let creative = self.core.creative;
        self.core = Core::new(
            &self.definitions,
            &self.technologies,
            &self.core.scenario,
            Some(self.core.seed),
            Some(self.core.world_params.clone()),
        )
        .map_err(js_error)?;
        self.core.set_creative(creative);
        // The core the baseline described is gone, so the next delta is a complete replacement.
        self.baseline = None;
        Ok(())
    }

    pub fn new_game(
        &mut self,
        scenario_key: &str,
        seed_override: Option<u32>,
        world_params_json: Option<String>,
        creative: Option<bool>,
    ) -> Result<(), JsValue> {
        let scenario = self
            .scenarios
            .scenarios
            .iter()
            .find(|scenario| scenario.key == scenario_key)
            .ok_or_else(|| js_error(format!("unknown scenario {scenario_key}")))?;
        let world_params = parse_world_params(world_params_json.as_deref())?;
        self.core = Core::new(
            &self.definitions,
            &self.technologies,
            scenario,
            seed_override,
            world_params,
        )
        .map_err(js_error)?;
        self.core.set_creative(creative.unwrap_or(false));
        self.baseline = None;
        Ok(())
    }

    /// The parameters this world was generated from. Not part of the per-frame delta: it changes
    /// only when a world does, so the host asks for it after `new_game` and `load` rather than
    /// paying for it on every frame that could not have changed it.
    pub fn world_params_json(&self) -> String {
        serde_json::to_string(&self.core.world_params).expect("world params serialize")
    }

    /// The shipped presets, with their full parameter sets. The new-world flow is built from this
    /// the same way the catalogue is built from the definitions: the host renders a table native
    /// owns rather than keeping a copy of its own that can drift.
    pub fn world_presets_json() -> String {
        serde_json::to_string(&world_presets()).expect("world presets serialize")
    }

    /// A rectangle of generated terrain for a parameter set nobody has played yet: one byte per
    /// preview pixel, holding the band's index in the `Terrain` declaration order that
    /// `fixtures/terrain-passability.json` already pins on both sides of the wire.
    ///
    /// This is what lets the new-world panel show a world rather than describe one. It goes through
    /// the same `terrain_at` a played hex goes through, so a preview and the world the start button
    /// generates cannot disagree — which is the whole reason it is a native export and not a second
    /// generator written in the host.
    ///
    /// `hexes_across` is the span the width frames. A pixel is square in world units, so a taller
    /// preview shows more world rather than a stretched copy of the same world.
    ///
    /// Takes `&self` for the definitions alone: the parameter set is validated against the same
    /// catalogue `Core::new` validates it against, so the panel cannot draw a world the start
    /// button would then refuse. Nothing about the run in progress is read or moved.
    pub fn world_preview_bytes(
        &self,
        world_params_json: &str,
        seed: u32,
        width: u32,
        height: u32,
        hexes_across: u32,
    ) -> Result<Vec<u8>, JsValue> {
        self.preview_cells(world_params_json, seed, width, height, hexes_across)
            .map_err(js_error)
    }

    /// Where the deposit lattice puts a site inside that same window, in preview pixels.
    ///
    /// Sites are reported as centres rather than sampled per pixel because a patch is smaller than
    /// a pixel at any zoom wide enough to frame a landform — and because a centre is the thing
    /// `site_cell` and `site_jitter` actually move, so it is the thing worth drawing.
    ///
    /// `unmet` carries the guarantees the bootstrap pass gave up on. `Core::new` refuses a world
    /// over exactly that list, so a preview that stayed quiet about it would be a picture of a
    /// world the start button then declines to generate.
    ///
    /// A window wide enough to frame a coastline holds tens of thousands of deposits, which is a
    /// texture rather than a map and a megabyte rather than a payload. Past `MAX_PREVIEW_SITES` the
    /// list is dropped and `total` and `dense` travel alone — `unmet` either way.
    pub fn world_preview_sites_json(
        &self,
        world_params_json: &str,
        seed: u32,
        width: u32,
        height: u32,
        hexes_across: u32,
    ) -> Result<String, JsValue> {
        let sites = self
            .preview_sites(world_params_json, seed, width, height, hexes_across)
            .map_err(js_error)?;
        serde_json::to_string(&sites).map_err(|error| js_error(error.to_string()))
    }

    pub fn apply_commands_json(&mut self, commands_json: &str) -> Result<(), JsValue> {
        self.core.apply_commands(commands_json).map_err(js_error)
    }

    /// One frame: the bounded command batch, `count` simulation ticks, and `player_steps` steps of
    /// player movement. The two counts are separate because the player runs on its own cadence —
    /// see `PLAYER_TICKS_PER_SECOND`, which the host reads to decide how many steps a frame is
    /// worth rather than inventing a rate of its own.
    pub fn advance_json(
        &mut self,
        commands_json: &str,
        count: u32,
        player_steps: u32,
    ) -> Result<(), JsValue> {
        self.core
            .advance(commands_json, count, player_steps)
            .map_err(js_error)
    }

    /// The player's fixed walking cadence in steps per real second. Native owns the rate; the host
    /// only converts elapsed real time into a step count with it.
    #[wasm_bindgen(js_name = playerTicksPerSecond)]
    pub fn player_ticks_per_second() -> u32 {
        PLAYER_TICKS_PER_SECOND
    }

    pub fn placement_preview_json(
        &self,
        q: i32,
        r: i32,
        definition_id: DefinitionId,
        orientation: u8,
        recipe_id: Option<RecipeId>,
    ) -> String {
        let result =
            self.core
                .placement_legality(q, r, definition_id, orientation, recipe_id, true);
        let preview = match result {
            Ok(()) => PlacementPreview {
                legal: true,
                reason: "Ready to build".into(),
            },
            Err(reason) => PlacementPreview {
                legal: false,
                reason,
            },
        };
        serde_json::to_string(&preview).expect("preview is serializable")
    }

    /// The cells, headings, and legality a construction drag between these endpoints would produce.
    pub fn line_preview_json(
        &self,
        q: i32,
        r: i32,
        to_q: i32,
        to_r: i32,
        definition_id: DefinitionId,
        orientation: u8,
        recipe_id: Option<RecipeId>,
    ) -> String {
        let cells =
            self.core
                .line_preview((q, r), (to_q, to_r), definition_id, orientation, recipe_id);
        serde_json::to_string(&cells).expect("preview is serializable")
    }

    /// The cells a removal drag between these endpoints would take back.
    pub fn erase_line_preview_json(&self, q: i32, r: i32, to_q: i32, to_r: i32) -> String {
        let cells = self.core.erase_line_preview((q, r), (to_q, to_r));
        serde_json::to_string(&cells).expect("preview is serializable")
    }

    pub fn boundary_preview_json(&self, edit_json: &str) -> Result<String, JsValue> {
        let edit: BoundaryEdit =
            serde_json::from_str(edit_json).map_err(|e| js_error(e.to_string()))?;
        serde_json::to_string(&self.core.boundary_preview(&edit))
            .map_err(|e| js_error(e.to_string()))
    }

    pub fn ground_preview_json(&self, edit_json: &str) -> Result<String, JsValue> {
        let edit: GroundEdit =
            serde_json::from_str(edit_json).map_err(|e| js_error(e.to_string()))?;
        serde_json::to_string(&self.core.ground_preview(&edit)).map_err(|e| js_error(e.to_string()))
    }

    pub fn snapshot_json(&mut self) -> String {
        let snapshot = self.core.snapshot();
        self.snapshot_revision = 0;
        self.baseline = Some(SnapshotBaseline::from_snapshot(&snapshot));
        self.core.dirty = SnapshotDirty::default();
        serde_json::to_string(&snapshot).expect("snapshot is serializable")
    }

    /// The delta the game actually ships, in the binary wire format.
    ///
    /// wasm-bindgen hands this to the worker as a `Uint8Array`, which the worker then transfers to
    /// the main thread rather than letting the structured clone copy it. `docs/BENCHMARKS.md`
    /// finding 3 is what this exists for: the boundary tracked payload bytes at about 10 µs/KB and
    /// cost more than the simulation it carried.
    pub fn snapshot_delta_bytes(&mut self) -> Vec<u8> {
        let delta = self.build_delta();
        wire::encode_delta(&delta)
    }

    /// The same delta as JSON.
    ///
    /// This is no longer the shipped path. It is retained as the oracle `snapshot_delta_bytes` is
    /// pinned against — the binary buffer must decode to exactly this object — and as the
    /// comparison the capacity ladder reports the encoding's saving against.
    pub fn snapshot_delta_json(&mut self) -> String {
        let delta = self.build_delta();
        serde_json::to_string(&delta).expect("snapshot delta is serializable")
    }

    pub fn save_string(&self) -> Result<String, JsValue> {
        self.core.save_string().map_err(js_error)
    }

    pub fn load_string(&mut self, save: &str) -> Result<(), JsValue> {
        self.core = Core::from_save(&self.definitions, &self.technologies, &self.scenarios, save)
            .map_err(js_error)?;
        self.baseline = None;
        Ok(())
    }

    pub fn checksum(&self) -> u32 {
        self.core.checksum()
    }

    pub fn tick_count(&self) -> u64 {
        self.core.tick
    }
}

fn parse_json<T: for<'de> Deserialize<'de>>(json: &str) -> Result<T, JsValue> {
    serde_json::from_str(json).map_err(|error| js_error(error.to_string()))
}

fn js_error(error: impl AsRef<str>) -> JsValue {
    JsValue::from_str(error.as_ref())
}

/// What the host may name a world with: a preset key, or a complete parameter set. Both are the
/// same table read at two depths — the preset is the usable surface and the parameter set is the
/// maintainable one — so the caller picking one is not picking a different mechanism.
#[derive(Deserialize)]
#[serde(untagged)]
enum WorldParamsInput {
    Preset { preset: String },
    Params(Box<WorldParams>),
}

/// `None` means "whatever the scenario names", which is how every call site that does not care
/// about generation stays unaware that parameters exist.
fn parse_world_params(json: Option<&str>) -> Result<Option<WorldParams>, JsValue> {
    let Some(json) = json.map(str::trim).filter(|json| !json.is_empty()) else {
        return Ok(None);
    };
    world_params_from_json(json).map(Some).map_err(js_error)
}

/// The same read with the failure left as a string.
///
/// A `JsValue` can only be constructed inside wasm — building one on the host aborts the process —
/// so anything a native test drives has to fail in `String` and be wrapped at the export.
fn world_params_from_json(json: &str) -> Result<WorldParams, String> {
    let input: WorldParamsInput = serde_json::from_str(json)
        .map_err(|error| format!("malformed world parameters: {error}"))?;
    Ok(match input {
        WorldParamsInput::Preset { preset } => {
            preset_params(&preset).ok_or_else(|| format!("unknown world preset {preset}"))?
        }
        WorldParamsInput::Params(params) => *params,
    })
}

fn validate_all(
    definitions: &DefinitionsInput,
    technologies: &TechnologiesInput,
    scenarios: &ScenariosInput,
) -> Result<(), String> {
    validate_definitions(definitions)?;
    validate_technologies(definitions, technologies)?;
    validate_skills(technologies)?;
    for milestone in &technologies.skill_milestones {
        if let SkillEvent::ContractStage { key } = &milestone.event {
            if !scenarios
                .scenarios
                .iter()
                .any(|s| s.contract.stages.iter().any(|stage| &stage.key == key))
            {
                return Err("skill milestone references an unknown commission".into());
            }
        }
    }
    validate_research_budget(definitions, technologies)?;
    validate_scenarios(definitions, technologies, scenarios)
}

/// The finite catalogue has to be able to pay for everything the tree sells.
///
/// While the board reposted paid rows this question could not be asked: income was unbounded, so
/// the answer was trivially yes and the catalogue's size meant nothing. Finite demand turns the
/// catalogue into a budget, and a budget that does not cover the shipped research would strand a
/// run with technologies it can see, needs, and can never buy — a defect no test of an individual
/// price would catch, because every price in it would be defensible on its own.
///
/// The margin is required rather than merely reported. The plan asks for "an explicit surplus for
/// route choice": a catalogue that funds the tree to the last insight would technically pass while
/// forcing one exact purchase order, which is not a choice.
fn validate_research_budget(
    definitions: &DefinitionsInput,
    technologies: &TechnologiesInput,
) -> Result<(), String> {
    let income: u64 = definitions
        .requests
        .iter()
        .map(|request| u64::from(request.insight))
        .sum();
    let research: u64 = technologies
        .technologies
        .iter()
        .map(|technology| u64::from(technology.cost))
        .sum();
    if income < research {
        return Err(format!(
            "the project catalogue pays {income} insight but research costs {research}: \
             finite demand would strand the tree"
        ));
    }
    if income * 4 < research * 5 {
        return Err(format!(
            "the project catalogue pays {income} insight against {research} of research: \
             too little surplus to leave the purchase order to the player"
        ));
    }
    Ok(())
}

fn validate_definitions(definitions: &DefinitionsInput) -> Result<(), String> {
    validate_boundaries(definitions)?;
    validate_surfaces(definitions)?;
    if definitions.version == 0 {
        return Err("definition version must be positive".into());
    }
    unique_positive_ids(definitions.items.iter().map(|item| item.id), "item")?;
    unique_positive_ids(definitions.recipes.iter().map(|recipe| recipe.id), "recipe")?;
    unique_positive_ids(
        definitions.buildings.iter().map(|building| building.id),
        "building",
    )?;
    unique_positive_ids(
        definitions.requests.iter().map(|request| request.id),
        "request",
    )?;
    let item_ids: BTreeSet<_> = definitions.items.iter().map(|item| item.id).collect();
    // Requests are the only thing in the game that pays insight, and insight is the only thing that
    // buys research. A catalogue with none of them is a catalogue where nothing can ever be learned.
    if definitions.requests.is_empty() {
        return Err("no hub requests: nothing would ever pay insight".into());
    }
    for request in &definitions.requests {
        if request.key.trim().is_empty()
            || request.name.trim().is_empty()
            || request.brief.trim().is_empty()
            || request.quantity == 0
            || request.insight == 0
            || !item_ids.contains(&request.item_id)
        {
            return Err(format!("request {} is incomplete", request.id));
        }
    }
    for item in &definitions.items {
        if item.key.trim().is_empty()
            || item.name.trim().is_empty()
            || item.color.trim().is_empty()
            || item.icon.trim().is_empty()
            || item.description.trim().is_empty()
            || item.stack_size == 0
        {
            return Err(format!(
                "item {} has incomplete display/value data",
                item.id
            ));
        }
    }
    // A fuel item has to be worth burning, or a machine could consume one for nothing.
    for item in &definitions.items {
        if item.fuel_value == Some(0)
            || item.regrowth_ticks == Some(0)
            || item.hand_gather_steps == Some(0)
            || item.extract_steps == Some(0)
        {
            return Err(format!(
                "item {} has a zero fuel, regrowth, hand gather, or extract rate",
                item.id
            ));
        }
    }
    // Every material the world can actually generate must name an extraction rate, because an
    // extractor may be stood on any of them. Without this a new site rule would silently inherit
    // whatever cadence its building carried, which is exactly the flat rate this replaced — and it
    // would do it quietly, on one material, long after the row was written.
    let generated: BTreeSet<ItemId> = world_presets()
        .iter()
        .flat_map(|preset| preset.params.site_rules.iter())
        .map(|rule| rule.item_id)
        .collect();
    for item_id in generated {
        let Some(item) = definitions.items.iter().find(|item| item.id == item_id) else {
            return Err(format!("world presets name unknown item {item_id}"));
        };
        if item.extract_steps.is_none() {
            return Err(format!(
                "item {} ({}) can be generated as a field but names no extract_steps",
                item.id, item.key
            ));
        }
    }
    for building in &definitions.buildings {
        if building.extract_speed == Some(0) {
            return Err(format!("building {} has a zero extract speed", building.id));
        }
        // Anything a belt or a hand can load has to say how much it holds. The capacity lookup
        // falls back to "unbounded", which is a sensible default for a kind that stores nothing and
        // a silent one for a kind that stores plenty — a burner-generator shipped without this line
        // and swallowed an unlimited stack of coal, because nothing anywhere had to notice.
        if Core::stock_is_reachable_by_hand(building.kind) && building.capacity.is_none() {
            return Err(format!(
                "building {} ({}) holds stock but names no capacity",
                building.id, building.key
            ));
        }
    }
    for recipe in &definitions.recipes {
        if recipe.key.trim().is_empty()
            || recipe.name.trim().is_empty()
            || recipe.description.trim().is_empty()
            || recipe.category.trim().is_empty()
            || recipe.duration == 0
            || recipe.inputs.is_empty()
            || recipe.output.quantity == 0
        {
            return Err(format!("recipe {} is incomplete", recipe.id));
        }
        // A recipe no machine can be assigned is content that cannot be reached, which is a defect
        // in the catalog rather than something to discover in play.
        if !definitions
            .buildings
            .iter()
            .any(|building| building.supports_recipe(recipe))
        {
            return Err(format!(
                "recipe {} has category {}, which no building runs",
                recipe.id, recipe.category
            ));
        }
        for ingredient in recipe.inputs.iter().chain(recipe.outputs()) {
            if ingredient.quantity == 0 || !item_ids.contains(&ingredient.item_id) {
                return Err(format!("recipe {} references an invalid item", recipe.id));
            }
        }
    }
    for building in &definitions.buildings {
        if building.key.trim().is_empty()
            || building.name.trim().is_empty()
            || building.description.trim().is_empty()
            || building.icon.trim().is_empty()
        {
            return Err(format!("building {} is incomplete", building.id));
        }
        if matches!(building.kind, BuildingKind::Extractor | BuildingKind::Pump)
            && building.cadence.unwrap_or(0) == 0
        {
            return Err(format!("source {} requires a cadence", building.id));
        }
        if building.kind == BuildingKind::Pump
            && !building
                .output_item_id
                .is_some_and(|item_id| item_ids.contains(&item_id))
        {
            return Err(format!("pump {} requires a known output item", building.id));
        }
        if building
            .output_item_id
            .is_some_and(|item_id| !item_ids.contains(&item_id))
        {
            return Err(format!(
                "source {} requires a known output item",
                building.id
            ));
        }
        // A machine that runs recipes needs a category, and one that does not must not claim one.
        if (building.kind == BuildingKind::Composer) != building.recipe_category.is_some() {
            return Err(format!(
                "building {} has a recipe category that does not match its kind",
                building.id
            ));
        }
        if let Some(ids) = &building.recipe_ids {
            if building.kind != BuildingKind::Composer
                || ids.is_empty()
                || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
                || ids
                    .iter()
                    .any(|id| !definitions.recipes.iter().any(|recipe| recipe.id == *id))
            {
                return Err(format!(
                    "building {} has invalid recipe capabilities",
                    building.id
                ));
            }
        }
        if let Some(multiplier) = building.duration_multiplier {
            if building.kind != BuildingKind::Composer
                || !(1..=60).contains(&multiplier)
                || definitions
                    .recipes
                    .iter()
                    .filter(|recipe| building.supports_recipe(recipe))
                    .any(|recipe| recipe.duration.checked_mul(multiplier).is_none())
            {
                return Err(format!(
                    "building {} has invalid recipe duration multiplier",
                    building.id
                ));
            }
        }
        if building.manual_work
            && (building.kind != BuildingKind::Composer
                || building.recipe_ids.is_none()
                || building.power_draw.unwrap_or(0) != 0
                || definitions
                    .recipes
                    .iter()
                    .any(|recipe| building.supports_recipe(recipe) && recipe.fuel != 0))
        {
            return Err(format!(
                "building {} has invalid manual work capabilities",
                building.id
            ));
        }
        if building.placement_rule == PlacementRule::Water
            && !matches!(
                building.kind,
                BuildingKind::Pump | BuildingKind::Boiler | BuildingKind::Generator
            )
        {
            return Err(format!(
                "building {} places on water but cannot draw from a basin",
                building.id
            ));
        }
        if building.placement_rule == PlacementRule::Shallows
            && building.kind != BuildingKind::Bridge
        {
            return Err(format!(
                "building {} places on shallows but is not a bridge",
                building.id
            ));
        }
        if building.kind == BuildingKind::Generator
            && (building.power_source.is_none() || building.power_output.unwrap_or(0) == 0)
        {
            return Err(format!(
                "generator {} needs a power source and an output",
                building.id
            ));
        }
        let footprint: BTreeSet<_> = building
            .footprint
            .iter()
            .map(|cell| (cell.q, cell.r))
            .collect();
        if footprint.len() != building.footprint.len()
            || !footprint.contains(&(0, 0))
            || footprint.len() > MAX_FOOTPRINT_CELLS
        {
            return Err(format!("building {} has an invalid footprint", building.id));
        }
        // One building is one connected thing. Two lobes with a gap between them would still map
        // every cell into the occupancy index, but reach, routing and the ground pad would all be
        // measuring a shape the player cannot see as a single machine — and the gap cell would be
        // walkable ground inside a building. Contiguity is cheap to state here and impossible to
        // recover later, once a save holds the disconnected entity.
        if !footprint_is_contiguous(&footprint) {
            return Err(format!(
                "building {} has a footprint in disconnected pieces",
                building.id
            ));
        }
        let envelope = unique_offsets(&building.service_envelope, "service envelope", building.id)?;
        let clearance = unique_offsets(
            &building.overhead_clearance,
            "overhead clearance",
            building.id,
        )?;
        if envelope.len() > MAX_ENVELOPE_CELLS {
            return Err(format!(
                "building {} has an invalid service envelope",
                building.id
            ));
        }
        if clearance.len() > MAX_CLEARANCE_CELLS {
            return Err(format!(
                "building {} has an invalid overhead clearance",
                building.id
            ));
        }
        for cell in envelope.iter().chain(clearance.iter()) {
            if footprint.contains(cell) {
                return Err(format!(
                    "building {} reserves a cell it already occupies",
                    building.id
                ));
            }
        }
        if envelope.iter().any(|cell| clearance.contains(cell)) {
            return Err(format!(
                "building {} uses the same cell as envelope and clearance",
                building.id
            ));
        }
        if !envelope.is_empty() {
            let mut with_envelope = footprint.clone();
            with_envelope.extend(envelope.iter().copied());
            if !footprint_is_contiguous(&with_envelope) {
                return Err(format!(
                    "building {} has a service envelope in disconnected pieces",
                    building.id
                ));
            }
        }
        if !clearance.is_empty() {
            let mut with_clearance = footprint.clone();
            with_clearance.extend(clearance.iter().copied());
            if !footprint_is_contiguous(&with_clearance) {
                return Err(format!(
                    "building {} has overhead clearance in disconnected pieces",
                    building.id
                ));
            }
        }
        // No shipped definition needs a multi-cell corner-heading footprint yet. Keep the narrow
        // rule until a real definition asks for the extra path and can test it. The test is "may
        // face a corner", not "faces only corners": an any-axis definition reaches the same
        // untested path the moment it is rotated onto a vertex heading. Envelope and clearance
        // rotate the same way, so they stay empty on that axis too.
        if building.orientation_axis.allows(NORTH)
            && (building.footprint.len() != 1
                || !building.service_envelope.is_empty()
                || !building.overhead_clearance.is_empty())
        {
            return Err(format!(
                "building {} spans the two-row period, which only a single-cell footprint can do",
                building.id
            ));
        }
        if let Some(radius) = building.extract_radius {
            if !matches!(building.kind, BuildingKind::Extractor | BuildingKind::Pump) {
                return Err(format!(
                    "building {} claims a source reach but is not an extractor or pump",
                    building.id
                ));
            }
            if radius == 0 || radius > MAX_EXTRACT_RADIUS {
                return Err(format!(
                    "extractor {} needs a reach in 1..={MAX_EXTRACT_RADIUS}",
                    building.id
                ));
            }
        }
        if let Some(radius) = building.supply_radius {
            if building.kind != BuildingKind::Pole {
                return Err(format!(
                    "building {} claims a supply radius but is not a pole",
                    building.id
                ));
            }
            if radius == 0 || radius > MAX_POLE_SUPPLY_RADIUS {
                return Err(format!(
                    "pole {} needs a supply radius in 1..={MAX_POLE_SUPPLY_RADIUS}",
                    building.id
                ));
            }
        }
        // A pole that supplies further than it can pass current on is a pole that cannot be
        // chained at its own coverage — a line of them would leave dark gaps between lit discs.
        if let (Some(radius), Some(link)) = (building.supply_radius, building.pole_reach) {
            if link < radius {
                return Err(format!(
                    "pole {} reaches less far than it supplies",
                    building.id
                ));
            }
        }
        // Every pole states both of its distances. The defaults above exist so the rule has one
        // definition, not so a data row can stay silent: the host draws the coverage ring straight
        // off this file, and a pole that named no radius would be drawn at a radius nobody chose.
        if building.kind == BuildingKind::Pole
            && (building.supply_radius.is_none() || building.pole_reach.is_none())
        {
            return Err(format!(
                "pole {} must name both the distance it supplies and the distance it links",
                building.id
            ));
        }
        for ingredient in building
            .construction_cost
            .iter()
            .chain(building.corner_construction_cost.iter().flatten())
        {
            if ingredient.quantity == 0 || !item_ids.contains(&ingredient.item_id) {
                return Err(format!("building {} has an invalid cost", building.id));
            }
        }
        // A corner price and a corner gate are answers to a question a building that cannot face a
        // corner is never asked. Refusing them here keeps the data row honest about what it does,
        // rather than carrying a number nothing ever reads.
        if (building.corner_construction_cost.is_some() || building.corner_technology_id.is_some())
            && !building.orientation_axis.allows(NORTH)
        {
            return Err(format!(
                "building {} names a corner price or gate but cannot face a corner",
                building.id
            ));
        }
        // The whole point of retiring the riser is that the two-row reach stays a research step. An
        // any-axis definition without its own corner gate would hand the player that reach at the
        // first belt they place.
        if building.orientation_axis == OrientationAxis::Any
            && building.corner_technology_id.is_none()
        {
            return Err(format!(
                "building {} takes every heading but gates none of them",
                building.id
            ));
        }
        // Bounded, because an unbounded span is a belt that costs nothing per hex.
        if let Some(span) = building.underpass_span {
            if span == 0 || span > MAX_UNDERPASS_SPAN {
                return Err(format!(
                    "building {} spans {span}, outside 1..={MAX_UNDERPASS_SPAN}",
                    building.id
                ));
            }
        }
        if building.transport_medium != TransportMedium::Solid
            && building.kind != BuildingKind::Belt
        {
            return Err(format!(
                "building {} has a transport medium but is not transport",
                building.id
            ));
        }
        if let Some(ids) = &building.accepted_item_ids {
            if building.kind != BuildingKind::Container
                || ids.is_empty()
                || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
                || ids.iter().any(|id| !item_ids.contains(id))
            {
                return Err(format!(
                    "building {} has an invalid storage filter",
                    building.id
                ));
            }
        }
        // Splitting, merging, and spanning are all rules about compiled transport edges, and a
        // building that is not transport compiles none.
        if (building.splits || building.merges || building.underpass_span.is_some())
            && building.kind != BuildingKind::Belt
        {
            return Err(format!(
                "building {} is not transport but claims a transport rule",
                building.id
            ));
        }
        // One entity, one arbitration rule. A definition that both fans out and rotates its feeders
        // would have two answers for which link a single item takes.
        if building.splits && building.merges {
            return Err(format!(
                "building {} cannot both split and merge",
                building.id
            ));
        }
    }
    recipes::validate_routes(definitions)?;
    validate_upgrade_ladders(definitions)?;
    Ok(())
}

/// What an upgrade ladder has to be, checked once at load so `upgrade` itself stays a short
/// command rather than a second copy of the placement rules.
///
/// A tier is a data row, and these are the constraints that make that true: an upgrade may only
/// grow a building into a taller version of itself, never turn it into a different machine. Kind,
/// recipe category, and footprint are all pinned, which is what lets the command preserve
/// contents, orientation, and connections without asking whether any of them still apply. The
/// strictly increasing tier is what makes the ladder finite, so a chain can never cycle.
fn validate_upgrade_ladders(definitions: &DefinitionsInput) -> Result<(), String> {
    for building in &definitions.buildings {
        let Some(next_id) = building.upgrades_to else {
            continue;
        };
        let Some(next) = definitions
            .buildings
            .iter()
            .find(|candidate| candidate.id == next_id)
        else {
            return Err(format!(
                "building {} upgrades to unknown building {next_id}",
                building.id
            ));
        };
        if next.tier <= building.tier {
            return Err(format!(
                "building {} upgrades to {next_id}, which is not a higher tier",
                building.id
            ));
        }
        if next.kind != building.kind
            || next.recipe_category != building.recipe_category
            || next.recipe_ids != building.recipe_ids
            || next.manual_work != building.manual_work
        {
            return Err(format!(
                "building {} upgrades into a different machine, not a higher tier of itself",
                building.id
            ));
        }
        if next.orientation_axis != building.orientation_axis {
            return Err(format!(
                "building {} upgrades onto a different orientation axis",
                building.id
            ));
        }
        if next.foundation_class != building.foundation_class {
            return Err(format!(
                "building {} upgrades onto a different foundation class",
                building.id
            ));
        }
        let footprint: BTreeSet<_> = building
            .footprint
            .iter()
            .map(|cell| (cell.q, cell.r))
            .collect();
        let next_footprint: BTreeSet<_> =
            next.footprint.iter().map(|cell| (cell.q, cell.r)).collect();
        // A tier may take more ground; it may never give up ground it already stands on. Growing
        // into free cells leaves every existing cell, and therefore every connection bound to one,
        // exactly where it was — `upgrade` refuses unless the new cells are empty, so an output ray
        // that used to leave the footprint at some cell still leaves it at the same one. Shrinking
        // or sliding would strand a belt against a hex the building no longer occupies, which is
        // the failure this rule has always been about.
        if !footprint.is_subset(&next_footprint) {
            return Err(format!(
                "building {} upgrades off a cell it stands on, which would move its connections",
                building.id
            ));
        }
        if !next.buildable {
            return Err(format!(
                "building {} upgrades to {next_id}, which cannot be constructed",
                building.id
            ));
        }
    }
    Ok(())
}

fn validate_technologies(
    definitions: &DefinitionsInput,
    technologies: &TechnologiesInput,
) -> Result<(), String> {
    if technologies.version == 0 {
        return Err("technology version must be positive".into());
    }
    for (label, groups) in [
        ("branch", &technologies.branches),
        ("stage", &technologies.stages),
    ] {
        if groups.is_empty() || groups.len() > 64 {
            return Err(format!(
                "technology {label} registry requires 1 to 64 entries"
            ));
        }
        let mut keys = BTreeSet::new();
        for group in groups {
            // `order` is a u32 on both sides. Equal orders are valid; key is the stable tie-breaker.
            let _order = group.order;
            if !group
                .key
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_lowercase)
                || !group
                    .key
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                || group.name.trim().is_empty()
                || group.description.trim().is_empty()
                || !keys.insert(&group.key)
            {
                return Err(format!(
                    "technology {label} registry has an invalid or duplicate entry"
                ));
            }
        }
    }
    if technologies.technologies.len() > 1024 {
        return Err("technology catalog exceeds 1024 entries".into());
    }
    let branches: BTreeSet<_> = technologies
        .branches
        .iter()
        .map(|group| &group.key)
        .collect();
    let stages: BTreeSet<_> = technologies.stages.iter().map(|group| &group.key).collect();
    let mut keys = BTreeSet::new();
    unique_positive_ids(
        technologies
            .technologies
            .iter()
            .map(|technology| technology.id),
        "technology",
    )?;
    let ids: BTreeSet<_> = technologies
        .technologies
        .iter()
        .map(|value| value.id)
        .collect();
    let building_ids: BTreeSet<_> = definitions.buildings.iter().map(|value| value.id).collect();
    let boundary_ids: BTreeSet<_> = definitions
        .boundaries
        .iter()
        .map(|value| value.id)
        .collect();
    for technology in &technologies.technologies {
        if technology.key.trim().is_empty()
            || technology.name.trim().is_empty()
            || technology.description.trim().is_empty()
            || !keys.insert(&technology.key)
            || !branches.contains(&technology.branch)
            || !stages.contains(&technology.stage)
            || technology
                .prerequisites
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != technology.prerequisites.len()
            || !valid_technology_grant(technology)
            || !valid_technology_effects(
                technology,
                &building_ids,
                &boundary_ids,
                &definitions
                    .surfaces
                    .iter()
                    .map(|surface| surface.id)
                    .collect(),
            )
        {
            return Err(format!("technology {} is incomplete", technology.id));
        }
        if technology.prerequisites.iter().any(|id| !ids.contains(id)) {
            return Err(format!(
                "technology {} has an unknown prerequisite",
                technology.id
            ));
        }
    }
    for building in &definitions.buildings {
        if let Some(id) = building.unlock_technology_id {
            if !ids.contains(&id) {
                return Err(format!(
                    "building {} has an unknown unlock requirement",
                    building.id
                ));
            }
        }
    }
    for boundary in &definitions.boundaries {
        if let Some(id) = boundary.unlock_technology_id {
            if !ids.contains(&id) {
                return Err(format!(
                    "boundary {} has an unknown unlock requirement",
                    boundary.id
                ));
            }
        }
    }
    for surface in &definitions.surfaces {
        if let Some(id) = surface.unlock_technology_id {
            if !technologies.technologies.iter().any(|technology| technology.id == id && technology.effects.iter().any(|effect| matches!(effect, TechnologyEffect::UnlockSurface { surface_id } if *surface_id == surface.id))) {
                return Err(format!("surface {} has an invalid unlock requirement", surface.id));
            }
        }
    }
    let mut complete = BTreeSet::new();
    loop {
        let before = complete.len();
        for technology in &technologies.technologies {
            if technology
                .prerequisites
                .iter()
                .all(|id| complete.contains(id))
            {
                complete.insert(technology.id);
            }
        }
        if complete.len() == technologies.technologies.len() {
            break;
        }
        if complete.len() == before {
            return Err("technology graph must be acyclic".into());
        }
    }
    Ok(())
}

fn valid_technology_grant(technology: &TechnologyDefinition) -> bool {
    match &technology.grant {
        TechnologyGrant::Purchase => technology.cost > 0,
        TechnologyGrant::ContractStage { key, name } => {
            technology.cost == 0
                && key.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                && key
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                && !name.trim().is_empty()
        }
    }
}

fn valid_technology_effects(
    technology: &TechnologyDefinition,
    building_ids: &BTreeSet<DefinitionId>,
    boundary_ids: &BTreeSet<DefinitionId>,
    surface_ids: &BTreeSet<DefinitionId>,
) -> bool {
    let mut buildings = BTreeSet::new();
    for building_id in technology.building_unlocks() {
        if !building_ids.contains(&building_id) || !buildings.insert(building_id) {
            return false;
        }
    }
    let mut boundaries = BTreeSet::new();
    for boundary_id in technology.boundary_unlocks() {
        if !boundary_ids.contains(&boundary_id) || !boundaries.insert(boundary_id) {
            return false;
        }
    }
    let mut surfaces = BTreeSet::new();
    technology.effects.iter().all(|effect| {
        if let TechnologyEffect::UnlockSurface { surface_id } = effect {
            return surface_ids.contains(surface_id) && surfaces.insert(*surface_id);
        }
        matches!(
            effect,
            TechnologyEffect::UnlockBuilding { .. } | TechnologyEffect::UnlockBoundary { .. }
        )
    })
}

fn validate_scenarios(
    definitions: &DefinitionsInput,
    technologies: &TechnologiesInput,
    scenarios: &ScenariosInput,
) -> Result<(), String> {
    if scenarios.version == 0 {
        return Err("scenario catalog version must be positive".into());
    }
    unique_positive_ids(
        scenarios.scenarios.iter().map(|scenario| scenario.id),
        "scenario",
    )?;
    let item_ids: BTreeSet<_> = definitions.items.iter().map(|value| value.id).collect();
    let building_ids: BTreeSet<_> = definitions.buildings.iter().map(|value| value.id).collect();
    let recipe_ids: BTreeSet<_> = definitions.recipes.iter().map(|value| value.id).collect();
    let technology_ids: BTreeSet<_> = technologies
        .technologies
        .iter()
        .map(|value| value.id)
        .collect();
    let mut keys = BTreeSet::new();
    for scenario in &scenarios.scenarios {
        if scenario.key.trim().is_empty()
            || scenario.name.trim().is_empty()
            || scenario.description.trim().is_empty()
            || scenario.version == 0
            || scenario.chunk_size <= 0
            || scenario.player_facing >= 6
            || scenario.build_range == 0
            || scenario.carry_slots == 0
            || !keys.insert(scenario.key.clone())
        {
            return Err(format!("scenario {} is incomplete", scenario.id));
        }
        // A contract is the scenario's whole purpose, so an empty stage, an empty bill, a zero
        // line, or an item this build does not have is a scenario that can never be finished
        // rather than a scenario that is merely odd.
        let contract = &scenario.contract;
        if contract.key.trim().is_empty()
            || contract.name.trim().is_empty()
            || contract.stages.is_empty()
            || contract.stages.iter().any(|stage| {
                stage.key.trim().is_empty()
                    || stage.name.trim().is_empty()
                    || stage.brief.trim().is_empty()
                    || stage.reads.trim().is_empty()
                    || stage.requirements.is_empty()
                    || stage
                        .requirements
                        .iter()
                        .any(|need| need.quantity == 0 || !item_ids.contains(&need.item_id))
            })
        {
            return Err(format!(
                "scenario {} has an unfinishable contract",
                scenario.id
            ));
        }
        let mut occupied = BTreeSet::new();
        for building in &scenario.buildings {
            let definition = definitions
                .buildings
                .iter()
                .find(|definition| definition.id == building.definition_id);
            let footprint_clear = definition.map(|definition| {
                definition.footprint.iter().all(|offset| {
                    let turns = if building.orientation >= NORTH {
                        building.orientation - NORTH
                    } else {
                        building.orientation
                    };
                    let offset = rotate_coordinate(*offset, turns);
                    occupied.insert((building.q + offset.q, building.r + offset.r))
                })
            });
            if !building_ids.contains(&building.definition_id)
                || !definition
                    .is_some_and(|value| value.orientation_axis.allows(building.orientation))
                || footprint_clear != Some(true)
                || building
                    .recipe_id
                    .is_some_and(|id| !recipe_ids.contains(&id))
            {
                return Err(format!("scenario {} has an invalid building", scenario.id));
            }
        }
        if scenario
            .resources
            .iter()
            .any(|resource| resource.quantity == 0 || !item_ids.contains(&resource.item_id))
            || scenario
                .initial_inventory
                .iter()
                .any(|item| item.quantity == 0 || !item_ids.contains(&item.item_id))
            || scenario
                .initial_researched
                .iter()
                .any(|id| !technology_ids.contains(id))
        {
            return Err(format!(
                "scenario {} has invalid initial state",
                scenario.id
            ));
        }
        // A scenario that hands the player more than they can carry would start unplayable, so the
        // carrying rule is checked against the starting pack rather than discovered during play.
        let mut initial = BTreeMap::new();
        add_ingredients(&mut initial, &scenario.initial_inventory);
        let initial_slots: u32 = initial
            .iter()
            .map(|(item_id, &quantity)| {
                let stack = definitions
                    .items
                    .iter()
                    .find(|item| item.id == *item_id)
                    .map(|item| item.stack_size)
                    .unwrap_or(1)
                    .max(1);
                quantity.div_ceil(stack)
            })
            .sum();
        if initial_slots > scenario.carry_slots {
            return Err(format!(
                "scenario {} starts the player over their carrying capacity",
                scenario.id
            ));
        }
    }
    Ok(())
}

fn validate_saved_state(
    definitions: &DefinitionsInput,
    technologies: &TechnologiesInput,
    scenario: &ScenarioDefinition,
    state: &SavedState,
    legacy_skills: bool,
) -> Result<(), String> {
    validate_saved_boundaries(definitions, &state.boundaries)?;
    validate_saved_ground(definitions, &state.ground)?;
    hydrology::validate_saved_water(&state.water)?;
    geomorphology::validate_saved_stress(&state.bank_stress)?;
    validate_skill_state(technologies, &state.skills)?;
    let item_ids: BTreeSet<_> = definitions.items.iter().map(|value| value.id).collect();
    let technology_ids: BTreeSet<_> = technologies
        .technologies
        .iter()
        .map(|value| value.id)
        .collect();
    let mut coordinates = BTreeMap::new();
    let mut entity_ids = BTreeSet::new();
    let mut active_workshops = 0;
    for entity in &state.entities {
        let definition = definitions
            .buildings
            .iter()
            .find(|value| value.id == entity.placed.definition_id)
            .ok_or("save references an unknown building")?;
        if definition.manual_work && !entity.disabled {
            active_workshops += 1;
            if active_workshops > 1 {
                return Err("save contains multiple attended workshops".into());
            }
        }
        if definition.recipe_ids.is_some()
            && entity.placed.recipe_id.is_some_and(|id| {
                !definitions
                    .recipes
                    .iter()
                    .any(|recipe| recipe.id == id && definition.supports_recipe(recipe))
            })
        {
            return Err("save contains an unsupported workshop recipe".into());
        }
        let footprint_valid = definition.footprint.iter().all(|offset| {
            let turns = if entity.placed.orientation >= NORTH {
                entity.placed.orientation - NORTH
            } else {
                entity.placed.orientation
            };
            let offset = rotate_coordinate(*offset, turns);
            let cell = (entity.placed.q + offset.q, entity.placed.r + offset.r);
            match coordinates.get(&cell).copied() {
                None => {
                    coordinates.insert(cell, entity.kind);
                    true
                }
                Some(BuildingKind::Bridge) if entity.kind == BuildingKind::Belt => {
                    coordinates.insert(cell, entity.kind);
                    true
                }
                _ => false,
            }
        });
        let footprint: BTreeSet<(i32, i32)> = definition
            .footprint
            .iter()
            .map(|offset| {
                let turns = if entity.placed.orientation >= NORTH {
                    entity.placed.orientation - NORTH
                } else {
                    entity.placed.orientation
                };
                let offset = rotate_coordinate(*offset, turns);
                (entity.placed.q + offset.q, entity.placed.r + offset.r)
            })
            .collect();
        let allowed_outputs: BTreeSet<ItemId> = entity
            .placed
            .recipe_id
            .and_then(|id| definitions.recipes.iter().find(|recipe| recipe.id == id))
            .map(|recipe| recipe.outputs().map(|output| output.item_id).collect())
            .unwrap_or_else(|| definition.output_item_id.into_iter().collect());
        let routes_valid = state.output_routes.get(&entity.id).is_none_or(|routes| {
            routes.len() <= MAX_LINKS
                && routes.iter().all(|(&item_id, route)| {
                    if !allowed_outputs.contains(&item_id)
                        || usize::from(route.direction) >= DIRECTIONS.len()
                    {
                        return false;
                    }
                    let cell = (entity.placed.q + route.q, entity.placed.r + route.r);
                    let (dq, dr) = DIRECTIONS[usize::from(route.direction)];
                    footprint.contains(&cell) && !footprint.contains(&(cell.0 + dq, cell.1 + dr))
                })
        });
        if entity.kind != definition.kind
            || !definition
                .orientation_axis
                .allows(entity.placed.orientation)
            || !footprint_valid
            || !routes_valid
            || !entity_ids.insert(entity.id)
            || entity
                .inventory
                .keys()
                .chain(entity.input_inventory.keys())
                .chain(entity.fuel_inventory.keys())
                .chain(entity.output_inventory.keys())
                .chain(entity.reserved_inputs.keys())
                .any(|item| !item_ids.contains(item))
        {
            return Err("save contains invalid entity state".into());
        }
    }
    if state
        .output_routes
        .keys()
        .any(|entity_id| !entity_ids.contains(entity_id))
    {
        return Err("save contains output routes for an unknown entity".into());
    }
    if state.legacy_fluid_belts.iter().any(|id| {
        !state
            .entities
            .iter()
            .any(|entity| entity.id == *id && entity.kind == BuildingKind::Belt)
    }) {
        return Err("save contains an invalid legacy fluid belt".into());
    }
    let (carry, reach) = research_bonuses(technologies, &state.researched);
    let skills = state.skills.bonuses(technologies);
    let (carry_slots_bonus, build_range_bonus) =
        (carry + skills.carry_slots, reach + skills.build_range);
    let earned_carry_slots = scenario
        .carry_slots
        .saturating_add(carry_slots_bonus)
        .min(MAX_CARRY_SLOTS);
    let earned_build_range = scenario
        .build_range
        .saturating_add(build_range_bonus)
        .saturating_mul(HEX_X as u32);
    if !(-1000..=1000).contains(&state.player.facing_x)
        || !(-1000..=1000).contains(&state.player.facing_y)
        || !(-1000..=1000).contains(&state.player.move_x)
        || !(-1000..=1000).contains(&state.player.move_y)
        || state.player.build_range != earned_build_range
        // A range rather than an equality: creative may widen the pack, so the earned
        // scenario-plus-research number is the floor a save may not go under and
        // `MAX_CARRY_SLOTS` is the ceiling it may not go over. Which value inside that range is
        // right for this run is the checksum's answer, not this function's.
        || state.player.carry_slots < earned_carry_slots
        || state.player.carry_slots > MAX_CARRY_SLOTS
        || state
            .player
            .inventory
            .keys()
            .any(|item| !item_ids.contains(item))
        || state.player.hand.is_some_and(|hand| {
            hand.quantity == 0
                || !item_ids.contains(&hand.item_id)
                || definitions
                    .items
                    .iter()
                    .find(|item| item.id == hand.item_id)
                    .is_none_or(|item| hand.quantity > item.stack_size)
        })
        || state
            .researched
            .iter()
            .any(|id| !technology_ids.contains(id) && !(legacy_skills && technologies.skills.iter().any(|skill| skill.legacy_technology_id == Some(*id))))
    {
        return Err("save contains invalid player or research state".into());
    }
    // A board is restored rather than redrawn, so it is checked instead: a slot naming a row this
    // build no longer ships, a duplicate slot, or one holding more than it ever asked for would all
    // survive the checksum and then be drawn as a request nobody can read.
    let mut posted = BTreeSet::new();
    for slot in &state.requests {
        if !definitions
            .requests
            .iter()
            .any(|request| request.id == slot.request_id)
        {
            return Err("save references an unknown hub request".into());
        }
        if !posted.insert(slot.request_id) {
            return Err("save contains invalid hub request state".into());
        }
    }
    // Progress now belongs to the project rather than the slot, so it is checked here: a count
    // above the bill, or one standing against a project already paid for, would survive the
    // checksum and then read as a project permanently one delivery from completion.
    for (id, &delivered) in &state.request_delivered {
        let definition = definitions
            .requests
            .iter()
            .find(|request| request.id == *id)
            .ok_or("save references an unknown hub request")?;
        if delivered > definition.quantity
            || state.request_fills.get(id).copied().unwrap_or_default() > 0
        {
            return Err("save contains invalid hub request state".into());
        }
    }
    if state.requests.len() > REQUEST_SLOTS
        || state
            .request_rounds
            .keys()
            .any(|id| !definitions.requests.iter().any(|request| request.id == *id))
        || state
            .request_fills
            .keys()
            .any(|id| !definitions.requests.iter().any(|request| request.id == *id))
    {
        return Err("save contains invalid hub request state".into());
    }
    let unique_tiles: BTreeSet<_> = state.tiles.iter().map(|tile| (tile.q, tile.r)).collect();
    if unique_tiles.len() != state.tiles.len()
        || state.tiles.iter().any(|tile| tile.resource.is_none())
    {
        return Err("save contains duplicate or empty overlay tiles".into());
    }
    for ground_item in &state.ground_items {
        if ground_item.quantity == 0 || !item_ids.contains(&ground_item.item_id) {
            return Err("save contains invalid ground item state".into());
        }
    }
    Ok(())
}

fn research_bonuses(
    technologies: &TechnologiesInput,
    researched: &BTreeSet<TechnologyId>,
) -> (u32, u32) {
    let mut legacy = SkillsState::default();
    for skill in &technologies.skills {
        if skill
            .legacy_technology_id
            .is_some_and(|id| researched.contains(&id))
        {
            legacy.granted.insert(skill.id);
        }
    }
    let initial = legacy.bonuses(technologies);
    technologies
        .technologies
        .iter()
        .filter(|technology| researched.contains(&technology.id))
        .fold(
            (initial.carry_slots, initial.build_range),
            |(carry_slots, build_range), technology| {
                (
                    carry_slots.saturating_add(technology.carry_slots_bonus()),
                    build_range.saturating_add(technology.build_range_bonus()),
                )
            },
        )
}

fn unique_positive_ids(ids: impl Iterator<Item = u16>, label: &str) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if id == 0 || !seen.insert(id) {
            return Err(format!("{label} ids must be positive and unique"));
        }
    }
    Ok(())
}

fn placed_sort_key(placed: &PlacedBuilding) -> (i32, i32, u16, u8, Option<u16>) {
    (
        placed.q,
        placed.r,
        placed.definition_id,
        placed.orientation,
        placed.recipe_id,
    )
}

fn coordinate_hash(seed: u32, q: i32, r: i32) -> u32 {
    let mut value =
        seed ^ (q as u32).wrapping_mul(0x9e3779b1) ^ (r as u32).wrapping_mul(0x85ebca77);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846ca68b);
    value ^ (value >> 16)
}

fn floor_div(value: i32, divisor: i32) -> i32 {
    value.div_euclid(divisor)
}

fn axial_world(q: i32, r: i32) -> (i32, i32) {
    (q * HEX_X + r * (HEX_X / 2), r * HEX_Y)
}

/// How finely `hex_at_world` divides a hex before it rounds. A twelfth-of-a-thousandth of a hex is
/// far below anything a preview pixel can show, and keeping it a power of two keeps the fixed point
/// exact.
const HEX_SUBDIVISION: i64 = 1 << 12;

/// The hex holding a world point: `axial_world` run backwards, then rounded to the nearest centre.
///
/// Fixed point rather than floating point, for the same reason the generator is integer — this maps
/// a preview pixel onto a hex, and a rounding that differed between two builds would be two
/// different pictures of one parameter set. It is not a checksum input, but it is compared: by a
/// player moving one slider and looking at what changed.
fn hex_at_world(x: i64, y: i64) -> (i32, i32) {
    let r = y * HEX_SUBDIVISION / i64::from(HEX_Y);
    let q = x * HEX_SUBDIVISION / i64::from(HEX_X) - r / 2;
    round_axial(q, r)
}

/// Cube rounding: round all three axes, then rebuild whichever moved furthest from the other two,
/// so the result always satisfies `q + r + s == 0` and is the centre actually nearest the point.
fn round_axial(q: i64, r: i64) -> (i32, i32) {
    let s = -q - r;
    let (rounded_q, rounded_r, rounded_s) = (round_hex(q), round_hex(r), round_hex(s));
    let drift_q = (rounded_q * HEX_SUBDIVISION - q).abs();
    let drift_r = (rounded_r * HEX_SUBDIVISION - r).abs();
    let drift_s = (rounded_s * HEX_SUBDIVISION - s).abs();
    if drift_q > drift_r && drift_q > drift_s {
        ((-rounded_r - rounded_s) as i32, rounded_r as i32)
    } else if drift_r > drift_s {
        (rounded_q as i32, (-rounded_q - rounded_s) as i32)
    } else {
        (rounded_q as i32, rounded_r as i32)
    }
}

/// One subdivided axis to the nearest whole hex, halves away from zero. Written out because Rust's
/// integer division truncates toward zero, which would round the negative half of the map the wrong
/// way and shear the picture across the origin.
fn round_hex(value: i64) -> i64 {
    (value * 2 + HEX_SUBDIVISION * value.signum()) / (HEX_SUBDIVISION * 2)
}

fn world_direction(direction: u8) -> (i16, i16) {
    const WORLD_DIRECTIONS: [(i16, i16); 6] = [
        (1000, 0),
        (500, 866),
        (-500, 866),
        (-1000, 0),
        (-500, -866),
        (500, -866),
    ];
    WORLD_DIRECTIONS[usize::from(direction % 6)]
}

/// True when every cell of a definition's footprint is reachable from its anchor through the six
/// edge steps.
///
/// Asked of the authored offsets only. Rotation by whole sixths is a symmetry of this lattice, so
/// a contiguous footprint stays contiguous at every heading a definition may face, and translation
/// to a placement anchor cannot separate it either. Checking the definition once is therefore the
/// same as checking every placement of it.
fn unique_offsets(
    cells: &[Coordinate],
    label: &str,
    building_id: DefinitionId,
) -> Result<BTreeSet<(i32, i32)>, String> {
    let unique: BTreeSet<_> = cells.iter().map(|cell| (cell.q, cell.r)).collect();
    if unique.len() != cells.len() {
        return Err(format!("building {building_id} has an invalid {label}"));
    }
    Ok(unique)
}

fn footprint_is_contiguous(cells: &BTreeSet<(i32, i32)>) -> bool {
    let mut reached = BTreeSet::from([(0, 0)]);
    let mut frontier = vec![(0, 0)];
    while let Some((q, r)) = frontier.pop() {
        for (dq, dr) in DIRECTIONS {
            let step = (q + dq, r + dr);
            if cells.contains(&step) && reached.insert(step) {
                frontier.push(step);
            }
        }
    }
    reached.len() == cells.len()
}

fn rotate_coordinate(mut coordinate: Coordinate, turns: u8) -> Coordinate {
    for _ in 0..turns % 6 {
        coordinate = Coordinate {
            q: -coordinate.r,
            r: coordinate.q + coordinate.r,
        };
    }
    coordinate
}

/// Split `total` across `weights` so the parts sum to exactly `total`.
///
/// Integer floor first, then the leftover units to the largest fractional remainders, ties broken
/// by position — and callers always pass entities in ascending id order, so the tie-break is a
/// save's own order. Exactness is the point: this is how energy is conserved between what plants
/// produced and what machines banked, with no per-entity remainder to store and no drift to audit.
fn apportion(total: u64, weights: &[u64]) -> Vec<u64> {
    let sum: u64 = weights.iter().sum();
    if sum == 0 || total == 0 {
        return vec![0; weights.len()];
    }
    let mut parts: Vec<u64> = weights
        .iter()
        .map(|&weight| (weight as u128 * total as u128 / sum as u128) as u64)
        .collect();
    let mut leftover = total - parts.iter().sum::<u64>();
    if leftover == 0 {
        return parts;
    }
    let mut order: Vec<usize> = (0..weights.len()).collect();
    order.sort_by_key(|&index| {
        let remainder = (weights[index] as u128 * total as u128) % sum as u128;
        (std::cmp::Reverse(remainder), index)
    });
    for index in order {
        if leftover == 0 {
            break;
        }
        parts[index] += 1;
        leftover -= 1;
    }
    parts
}

fn axial_distance(from: (i32, i32), to: (i32, i32)) -> i32 {
    let dq = to.0 - from.0;
    let dr = to.1 - from.1;
    (dq.abs() + dr.abs() + (dq + dr).abs()) / 2
}

/// The routing orientation that steps from one hex to another in a single transport step, or
/// `None` if no direction connects them.
///
/// Searches `TRANSPORT_DIRECTIONS`, so it answers for the two-row period as well as the six edges.
/// The six come first and keep their indices, so every delta that resolved before resolves to the
/// same number now.
fn step_direction(from: (i32, i32), to: (i32, i32)) -> Option<u8> {
    let delta = (to.0 - from.0, to.1 - from.1);
    TRANSPORT_DIRECTIONS
        .iter()
        .position(|direction| *direction == delta)
        .map(|index| index as u8)
}

/// The cells one drag covers, resolved on the axis the dragged definition builds on.
///
/// The two rules are kept apart rather than merged into one greedy loop over twelve directions,
/// because a unit step almost always closes the distance and a two-row step closes it only from
/// inside a narrow cone — so a single greedy loop would never select north or south at all. The
/// consequence of splitting them is the property that matters most: `hex_line` is untouched, so
/// **every drag that resolved before v0.14 resolves to exactly the same cells now.**
fn line_between(from: (i32, i32), to: (i32, i32), axis: OrientationAxis) -> Vec<(i32, i32)> {
    match axis {
        OrientationAxis::Edge => hex_line(from, to),
        OrientationAxis::Corner => hex_line_corner(from, to),
        OrientationAxis::Any => hex_line_any(from, to),
    }
}

/// The cells one drag covers when the definition may take every heading.
///
/// The greedy rule the two axis-specific resolvers keep apart can finally be merged, because with
/// both periods available the objection that sank a twelve-direction loop no longer holds: an edge
/// step always closes one, so the run can never stall in the way a corner-only greedy can, and a
/// corner step closes two only inside the 30°-of-vertical cone. Taking the largest closure and
/// tie-breaking on the lowest heading therefore selects the two-row period exactly where it is
/// worth taking and the unit period everywhere else — one rule, no tuned constant.
///
/// This is geometry alone. It is what `place_line` and the drag's out-of-range fallback walk;
/// `drag_route` prices the same lattice against the player's inventory and what is actually legal
/// to build, and that — not this — is what a live drag follows.
fn hex_line_any(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let mut cells = vec![from];
    let mut current = from;
    while current != to && cells.len() < MAX_LINE_CELLS {
        let remaining = axial_distance(current, to);
        let Some((_, &(dq, dr))) = TRANSPORT_DIRECTIONS
            .iter()
            .enumerate()
            .filter_map(|(heading, step)| {
                let closed =
                    remaining - axial_distance((current.0 + step.0, current.1 + step.1), to);
                (closed > 0).then_some((closed, heading, step))
            })
            .max_by_key(|&(closed, heading, _)| (closed, std::cmp::Reverse(heading)))
            .map(|(_, heading, step)| (heading, step))
        else {
            break;
        };
        current = (current.0 + dq, current.1 + dr);
        cells.push(current);
    }
    cells
}

/// The cells one corner-heading drag covers — the explicit rule the two-row period needs.
///
/// A step is taken only when it closes the full two rows it spans. That single condition *is* the
/// angle rule, and it needs no tuned constant to say so: in the hex norm, `(1, -2)` is the sum of
/// `NE` and `NW`, and a sum closes the distance by its whole length exactly when the target lies
/// in the closed cone those two span. That cone is 60° wide and centred on due north — `NE` sits
/// 30° east of vertical and `NW` 30° west of it — so the rule reads, precisely, **within 30° of
/// vertical, use the two-row period**.
///
/// A drag that leaves the cone stops rather than wandering: the run builds the risers it can and
/// the player places the corner themselves, which is the same "build what is legal and say where
/// it stopped" contract `place_line` already keeps for cost and for terrain.
fn hex_line_corner(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let mut cells = vec![from];
    let mut current = from;
    while current != to && cells.len() < MAX_LINE_CELLS {
        let remaining = axial_distance(current, to);
        // The lexicographic minimum is an explicit tie-break. The exhaustive lattice test below
        // also pins that the shipped rosette presents no ties, but determinism does not rely on it.
        let Some(&(dq, dr)) = TRANSPORT_DIRECTIONS[usize::from(NORTH)..]
            .iter()
            .filter(|(dq, dr)| {
                axial_distance((current.0 + dq, current.1 + dr), to) == remaining - 2
            })
            .min_by_key(|(dq, dr)| (*dq, *dr))
        else {
            break;
        };
        current = (current.0 + dq, current.1 + dr);
        cells.push(current);
    }
    cells
}

/// The cells one drag covers, from `from` through `to` inclusive.
///
/// Each step takes the lowest-numbered of the six directions that moves strictly closer to the
/// target. Once a direction stops closing the distance it never starts again, so a run uses at most
/// two directions and turns exactly once — the fewest turns a belt line between those endpoints can
/// have, and the same path every time. Integer-only and independent of iteration order, so it is
/// safe on a state-affecting path. The result is capped at `MAX_LINE_CELLS`; a longer drag builds
/// as far as the cap and stops.
fn hex_line(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let mut cells = vec![from];
    let mut current = from;
    while current != to && cells.len() < MAX_LINE_CELLS {
        let remaining = axial_distance(current, to);
        let Some(&(dq, dr)) = DIRECTIONS
            .iter()
            .find(|(dq, dr)| axial_distance((current.0 + dq, current.1 + dr), to) < remaining)
        else {
            break;
        };
        current = (current.0 + dq, current.1 + dr);
        cells.push(current);
    }
    cells
}

fn squared_distance(ax: i32, ay: i32, bx: i32, by: i32) -> i64 {
    let dx = i64::from(ax) - i64::from(bx);
    let dy = i64::from(ay) - i64::from(by);
    dx * dx + dy * dy
}

fn circles_overlap(ax: i32, ay: i32, ar: i32, bx: i32, by: i32, br: i32) -> bool {
    squared_distance(ax, ay, bx, by) < i64::from(ar + br).pow(2)
}

/// Newton's method, in integers. `aim` resolves to a checksum input, so the float square root the
/// same job would normally use is not available: the same aim has to produce the same facing on
/// every platform that runs this core, and `f64::sqrt` is only required to be correctly rounded,
/// not to be the same instruction everywhere.
fn integer_sqrt(value: i64) -> i64 {
    if value <= 0 {
        return 0;
    }
    let mut guess = value;
    let mut next = (guess + 1) / 2;
    while next < guess {
        guess = next;
        next = (guess + value / guess) / 2;
    }
    guess
}

fn resource_snapshot_of(
    key: (i32, i32),
    item_id: ItemId,
    quantity: u32,
    initial_quantity: u32,
) -> ResourceSnapshot {
    let (x, y) = axial_world(key.0, key.1);
    ResourceSnapshot {
        q: key.0,
        r: key.1,
        x,
        y,
        radius: HEX_RADIUS as u32,
        item_id,
        quantity,
        initial_quantity,
    }
}

fn hexes_in_radius(origin: (i32, i32), radius: i32) -> Vec<(i32, i32)> {
    let mut cells = Vec::new();
    for dq in -radius..=radius {
        for dr in -radius..=radius {
            let cell = (origin.0 + dq, origin.1 + dr);
            if axial_distance(origin, cell) <= radius {
                cells.push(cell);
            }
        }
    }
    cells
}

fn hexes_in_chunk(chunk_q: i32, chunk_r: i32, size: i32) -> impl Iterator<Item = (i32, i32)> {
    (0..size).flat_map(move |local_r| {
        (0..size).map(move |local_q| (chunk_q * size + local_q, chunk_r * size + local_r))
    })
}

fn chunk_world_bounds(chunk_q: i32, chunk_r: i32, size: i32) -> (i32, i32, i32) {
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for (q, r) in [
        (chunk_q * size, chunk_r * size),
        (chunk_q * size + size - 1, chunk_r * size),
        (chunk_q * size, chunk_r * size + size - 1),
        (chunk_q * size + size - 1, chunk_r * size + size - 1),
    ] {
        let (x, y) = axial_world(q, r);
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    let origin_x = min_x - HEX_RADIUS;
    let origin_y = min_y - HEX_RADIUS;
    let width = (max_x + HEX_RADIUS) - origin_x;
    let height = (max_y + HEX_RADIUS) - origin_y;
    (origin_x, origin_y, width.max(height))
}

/// Inverse of `axial_world` with cube rounding, so a world point maps to the hex whose centre is
/// nearest. Integer-only: numerators stay in `HEX_X * HEX_Y` space and rounding picks the cube
/// axis with the largest residual.
fn world_to_axial(x: i32, y: i32) -> (i32, i32) {
    let den = i64::from(HEX_X) * i64::from(HEX_Y);
    let q_num = i64::from(x) * i64::from(HEX_Y) - i64::from(y) * i64::from(HEX_X / 2);
    let r_num = i64::from(y) * i64::from(HEX_X);
    cube_round_num(q_num, r_num, -q_num - r_num, den)
}

fn cube_round_num(q: i64, r: i64, s: i64, den: i64) -> (i32, i32) {
    let rq = div_round(q, den);
    let rr = div_round(r, den);
    let rs = div_round(s, den);
    let dq = (rq * den - q).abs();
    let dr = (rr * den - r).abs();
    let ds = (rs * den - s).abs();
    if dq >= dr && dq >= ds {
        ((-rr - rs) as i32, rr as i32)
    } else if dr >= ds {
        (rq as i32, (-rq - rs) as i32)
    } else {
        (rq as i32, rr as i32)
    }
}

fn div_round(num: i64, den: i64) -> i64 {
    if den == 0 {
        return 0;
    }
    if num >= 0 {
        (num + den / 2) / den
    } else {
        -((-num + den / 2) / den)
    }
}

/// Integer value noise on the axial lattice. Samples a `cell`-sized grid and bilinearly
/// interpolates, so a hex still needs no stored neighbors.
fn value_noise(seed: u32, q: i32, r: i32, cell: i32, octave: u32) -> i32 {
    let cell = cell.max(1);
    let cq = floor_div(q, cell);
    let cr = floor_div(r, cell);
    let fq = q - cq * cell;
    let fr = r - cr * cell;
    let n00 = i32::from((coordinate_hash(seed ^ octave, cq, cr) >> 16) as u16);
    let n10 = i32::from((coordinate_hash(seed ^ octave, cq + 1, cr) >> 16) as u16);
    let n01 = i32::from((coordinate_hash(seed ^ octave, cq, cr + 1) >> 16) as u16);
    let n11 = i32::from((coordinate_hash(seed ^ octave, cq + 1, cr + 1) >> 16) as u16);
    let nx0 = lerp_i32(n00, n10, fq, cell);
    let nx1 = lerp_i32(n01, n11, fq, cell);
    lerp_i32(nx0, nx1, fr, cell)
}

fn lerp_i32(a: i32, b: i32, t: i32, span: i32) -> i32 {
    a + (b - a) * t / span.max(1)
}

/// The value every noise channel is bounded by. `value_noise` interpolates `u16` lattice samples,
/// so every channel lands in `0..=NOISE_MAX` and every threshold below is a point on this scale.
const NOISE_MAX: i32 = 65_535;

/// A gate that admits everything. Noise is never negative, so a rule carrying this on a channel is
/// not asking about that channel at all. Zero would *almost* mean the same thing and would be
/// wrong at exactly the lattice points where a channel samples zero, which is the kind of defect
/// that shows up once in a billion hexes and never reproduces.
const ANY: i32 = -1;

/// One row of the resource table: what a *deposit* is made of, how wide it is, and where its
/// centre is allowed to stand.
///
/// v0.21 moved the unit of a deposit from the hex to the **site**. The row this replaced decided
/// each hex on its own from three noise channels, so a patch's size and a patch's purity were
/// emergent accidents of channel cell size and gate height — neither controllable, nor
/// defaultable, nor measurable. The mixed-material case was the proof: iron gated on richness and
/// coal on vein, two *independent* channels, so wherever both ran high the two alternated hex by
/// hex and an extractor placed there covered both and cleanly worked neither. No pair of numbers
/// fixes that, because the two numbers are not asking one question.
///
/// Rows no longer compete per hex. The lattice picks one rule per site, so **one material per
/// patch** is a property of the model rather than a figure that was tuned into place.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct SiteRule {
    /// The band the site's *centre* must stand in for this rule to be eligible.
    terrain: Terrain,
    item_id: ItemId,
    /// Relative share among the eligible rules for a band. Zero means never — which is how a
    /// preset drops a material from a band without deleting the row that documents it.
    weight: u32,
    /// Inclusive radius range, in hexes. A disc of radius R holds 3R² + 3R + 1 hexes: 7, 19, 37,
    /// 61, 91, 127 at radius 1 through 6.
    radius_min: u32,
    radius_max: u32,
    /// Exclusive lower gate on the richness channel at the *centre*, so the world still has rich
    /// and poor country. `ANY` disables it, on the same reasoning `ANY` already carries.
    #[serde(default = "any_gate")]
    site_min: i32,
    /// Yield at the centre and at the rim, interpolated linearly by distance and then jittered.
    yield_core: u32,
    yield_rim: u32,
    /// Per-hex jitter on the interpolated yield, at least 1: `base + hash % spread` semantics.
    /// Keep it small enough that the core still reads as a core.
    yield_jitter: u32,
    /// Bands a hex must itself be in to belong to this site. Empty means the rule's own band. This
    /// is the clipping that makes a beach a strip and a scree field hug its cliffs.
    #[serde(default)]
    member: Vec<Terrain>,
    /// If set, a member hex must also be within this many hexes of water. `0` disables it.
    #[serde(default)]
    member_water_within: u32,
    /// If set, the centre must stand against *ocean* rather than against any pond: the coarse
    /// elevation octave alone — which is what makes a body big, established and proved in v0.16 —
    /// has to dip below `ocean_level` within `OCEAN_PROBE_RADIUS` of the centre.
    ///
    /// This is a proxy rather than a measurement, deliberately. The map is unbounded and generated
    /// lazily, so nothing here may flood-fill to find out how large a body is. The survey is what
    /// verifies it: it reports the size of the water body nearest each patch of an ocean-gated
    /// material, and a pond-sized number there means the proxy is wrong.
    #[serde(default)]
    center_ocean: bool,
    /// If set, the centre must stand next to the shore band. Cheaper than asking `terrain_at` —
    /// shore is an elevation cut, so one octave answers it — and the right question for a beach
    /// that is not an ocean: a lake and a sea both grow sandy tiles, and a rule that only asked
    /// the ocean proxy turned every inland beach into clay.
    #[serde(default)]
    center_shore: bool,
}

fn any_gate() -> i32 {
    ANY
}

/// The guaranteed opening, keyed by the lattice cell each promise claimed.
type BootstrapTable = BTreeMap<(i32, i32), Site>;

/// One step of the bootstrap pass's outward spiral: how far a lattice cell's centre stands from the
/// landing site, the cell, and that centre. Sorted, so the distance leads and the cell breaks ties.
type SpiralStep = (i32, (i32, i32), (i32, i32));

/// One deposit, resolved from the lattice cell that owns it. Derived from `(params, seed, cell)`
/// and nothing else, which is what lets the lattice be cached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Site {
    center: (i32, i32),
    /// Index into the parameter set's rule table.
    rule: usize,
    radius: i32,
    /// A guaranteed physical-world outcrop may replace the legacy presentation-band gate with dry
    /// substrate. This is derived bootstrap identity and is never saved or checksummed.
    forced_opening: bool,
}

/// The salt the site lattice hashes under, kept clear of every noise octave so which material a
/// deposit holds never correlates with the elevation under it.
const SITE_SALT: u32 = 0x5175E;
/// The row every derived field of a cell hash is drawn on.
const SITE_FIELD_ROW: i32 = 0x517E;
/// The octave the river channel is sampled on.
const RIVER_OCTAVE: u32 = 0xF10DE;
/// The octave the richness channel is sampled on. It gates a site's *centre* now rather than every
/// hex, which is what leaves the world with rich and poor country without deciding materials.
const RICHNESS_OCTAVE: u32 = 0x0E55;
/// How far from an ocean-gated centre the coarse octave is probed for open sea.
const OCEAN_PROBE_RADIUS: i32 = 2;
/// How far a shore-gated centre may stand from the shore band and still count as a beach site.
/// Sand's disc is radius 3–5, so a probe shorter than that would refuse the inland side of a
/// beach the disc itself can still paint.
const SHORE_PROBE_RADIUS: i32 = 4;
/// The largest radius a rule may claim, and the largest wander a centre may take inside its cell.
/// `field_at` scans every lattice cell within reach of a hex and reach grows with both, so a
/// parameter set is not allowed to make that scan unbounded — the same judgement `MAX_FEATURE_CELL`
/// already makes about a lattice stride.
const MAX_SITE_RADIUS: u32 = 8;
const MAX_SITE_JITTER: i32 = 16;
/// The hexes a base extractor covers, and so the smallest patch worth standing one on: a disc of
/// radius R holds 3R² + 3R + 1 hexes, which is 7 at the reach the hand and the base extractor
/// share. Derived from the reach rather than written down, so raising one moves the other.
const WORKABLE_PATCH_HEXES: u32 =
    (3 * EXTRACT_RADIUS * EXTRACT_RADIUS + 3 * EXTRACT_RADIUS + 1) as u32;

/// The knobs a world is generated from.
///
/// Unlike the shape grammar, this is **simulation truth**: two worlds sharing a seed and differing
/// here are different worlds, so parameters travel in the save envelope and the checksum, and
/// `WORLD_GENERATOR_VERSION` moved to 6 when they entered.
///
/// Feature scale and threshold are separate axes on purpose, because they are the pair a generator
/// most easily conflates. **Raising the sea level makes more water, not bigger water** — it
/// produces more ponds. Lakes, seas, and oceans come from a larger `elevation_coarse_cell` and a
/// larger share of the blend for that octave. The same split holds for every other band: where the
/// cuts sit is "how much", and the cell sizes are "ponds or oceans, hillocks or ranges".
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct WorldParams {
    /// The low-frequency elevation octave: the landform scale.
    elevation_coarse_cell: i32,
    /// The high-frequency octave that breaks up a coastline.
    elevation_fine_cell: i32,
    /// The coarse octave's share of the blend, in percent; the fine octave takes the rest. 50
    /// reproduces the `noise(8) / 2 + noise(3) / 2` that generator version 5 was frozen at.
    elevation_coarse_weight: i32,
    moisture_cell: i32,
    richness_cell: i32,
    /// Band cuts on the noise scale, in ascending order.
    water_level: i32,
    shore_level: i32,
    hills_level: i32,
    highland_level: i32,
    /// A hex whose steepest neighbour step exceeds this reads as cliff.
    cliff_step: i32,
    /// Water wetter than this is deep.
    deep_water_moisture: i32,
    /// The lattice a deposit is drawn on. One site cell holds at most one site, so this is how far
    /// apart deposits stand; `site_jitter` is how far a centre may wander inside its own cell, so
    /// that a world of deposits is not a world on a visible grid.
    site_cell: i32,
    site_jitter: i32,
    /// Rivers. `river_cell` is how far apart they run, `river_width` is the half-width of the band
    /// the channel is read against and so how wide a river is — `0` is a world without rivers —
    /// and `river_max_elevation` is where they stop, so no river runs over a summit.
    river_cell: i32,
    river_width: i32,
    river_max_elevation: i32,
    /// The cut the *coarse* elevation octave alone is read against when a rule asks for ocean.
    /// A pond exists only in the fine octave and fails it; an ocean coast passes.
    ocean_level: i32,
    site_rules: Vec<SiteRule>,
}

impl WorldParams {
    /// Whether the four elevation cuts ascend. Ascending cuts are what makes each band reachable:
    /// out of order, a band is not rare — it is unreachable, and the world silently loses whatever
    /// the table put in it. Its own predicate because a repair has to ask it before it offers a set
    /// `validate` would then refuse.
    fn band_levels_ascend(&self) -> bool {
        self.water_level < self.shore_level
            && self.shore_level < self.hills_level
            && self.hills_level < self.highland_level
    }

    /// Every way a parameter set can be nonsense, asked once, before a world is built from it.
    /// A set that generates an unplayable world is a real failure mode and this is not what
    /// catches it — the survey tool is. This catches the sets that are not worlds at all.
    fn validate(&self, definitions: &DefinitionsInput) -> Result<(), String> {
        let cells = [
            ("elevation_coarse_cell", self.elevation_coarse_cell),
            ("elevation_fine_cell", self.elevation_fine_cell),
            ("moisture_cell", self.moisture_cell),
            ("richness_cell", self.richness_cell),
            ("site_cell", self.site_cell),
            ("river_cell", self.river_cell),
        ];
        for (name, cell) in cells {
            if !(1..=MAX_FEATURE_CELL).contains(&cell) {
                return Err(format!(
                    "world parameter {name} must be between 1 and {MAX_FEATURE_CELL}"
                ));
            }
        }
        if !(0..=100).contains(&self.elevation_coarse_weight) {
            return Err("world parameter elevation_coarse_weight must be a percentage".into());
        }
        let levels = [
            ("water_level", self.water_level),
            ("shore_level", self.shore_level),
            ("hills_level", self.hills_level),
            ("highland_level", self.highland_level),
        ];
        for (name, level) in levels {
            if !(0..=NOISE_MAX).contains(&level) {
                return Err(format!("world parameter {name} is outside the noise range"));
            }
        }
        if !self.band_levels_ascend() {
            return Err("world band levels must ascend: water < shore < hills < highland".into());
        }
        if !(0..=NOISE_MAX).contains(&self.cliff_step) || self.cliff_step == 0 {
            return Err("world parameter cliff_step is outside the noise range".into());
        }
        if !(ANY..=NOISE_MAX).contains(&self.deep_water_moisture) {
            return Err("world parameter deep_water_moisture is outside the noise range".into());
        }
        if !(0..=MAX_SITE_JITTER).contains(&self.site_jitter) {
            return Err(format!(
                "world parameter site_jitter must be between 0 and {MAX_SITE_JITTER}"
            ));
        }
        for (name, level) in [
            ("river_width", self.river_width),
            ("river_max_elevation", self.river_max_elevation),
            ("ocean_level", self.ocean_level),
        ] {
            if !(0..=NOISE_MAX).contains(&level) {
                return Err(format!("world parameter {name} is outside the noise range"));
            }
        }
        if self.site_rules.is_empty() {
            return Err("world parameters need at least one site rule".into());
        }
        // A site rule that could name a water band would make the cheap water test `field_at`
        // opens with unsound, and a deposit in a basin is not a thing a pump or an extractor can
        // reach anyway. Refusing it here is what lets the fast path skip the band decision.
        let dry = |terrain: Terrain| !terrain.is_water();
        let mut placeable = false;
        for rule in &self.site_rules {
            let named = definitions
                .items
                .iter()
                .find(|item| item.id == rule.item_id);
            let Some(named) = named else {
                return Err(format!("site rule names unknown item {}", rule.item_id));
            };
            // An extractor can be stood on anything a rule can place, so anything a rule can place
            // has to price extraction. Custom parameters come through here too, which is why the
            // check lives beside the rule rather than only beside the built-in presets.
            if named.extract_steps.is_none() {
                return Err(format!(
                    "site rule names item {} ({}), which has no extract_steps",
                    named.id, named.key
                ));
            }
            if !dry(rule.terrain) || !rule.member.iter().copied().all(dry) {
                return Err("a site rule may not name a water band".into());
            }
            if rule.radius_min == 0 || rule.radius_min > rule.radius_max {
                return Err("site rule radii must ascend from at least 1".into());
            }
            if rule.radius_max > MAX_SITE_RADIUS {
                return Err(format!(
                    "site rule radius_max may not exceed {MAX_SITE_RADIUS}"
                ));
            }
            // Yield is `interpolated + hash % yield_jitter`, so a zero jitter is a division by zero.
            if rule.yield_jitter == 0 {
                return Err("site rule yield_jitter must be at least 1".into());
            }
            if rule.yield_core == 0 || rule.yield_rim == 0 {
                return Err("site rule yields must be at least 1".into());
            }
            if !(ANY..=NOISE_MAX).contains(&rule.site_min) {
                return Err("site rule gate is outside the noise range".into());
            }
            placeable |= rule.weight > 0;
        }
        if !placeable {
            return Err("every site rule is weighted zero, so the world holds nothing".into());
        }
        Ok(())
    }
}

/// The largest feature cell a parameter set may ask for. A cell is a lattice stride, so this is a
/// bound on how far apart two sampled corners may be — not a taste judgement. It keeps a
/// pathological value from making an entire surveyed world one interpolated slope. 1024 hexes is
/// a six-minute walk at 15 m/s, which is the scale oceans and ranges are allowed to ask for.
const MAX_FEATURE_CELL: i32 = 1024;
/// Landforms smaller than this are opening-sized: the bootstrap windows were tuned against a
/// coarse cell of 8, and a synthetic scale sweep (cell 4 vs 24) has to measure feature size, not a
/// landing pad. Shipped presets all sit well above it.
const LANDING_SCALE_CELL: i32 = 32;
/// The opening's own landform scale — v0.21 continental's cell 8 / 3 / 50, which is what the
/// bootstrap windows were measured against. A frozen regional coarse sample cannot produce
/// highland, lowland, and water inside 14 hexes of each other; this one can.
const OPENING_COARSE_CELL: i32 = 8;
const OPENING_FINE_CELL: i32 = 3;
const OPENING_COARSE_WEIGHT: i32 = 50;
/// Hexes around the hub that stay free of rivers, so the first minute is not a moat. Clay's
/// bootstrap window starts at 15, so a river can still be the first pump site.
const RIVER_CLEAR_RADIUS: i32 = LANDING_CLEAR_RADIUS + 6;

/// How far the opening scale fades into the regional one. Half a landform, clamped so a
/// 1024-cell custom world does not force a kilometre of fake continent.
fn landing_radius(params: &WorldParams) -> i32 {
    if params.elevation_coarse_cell < LANDING_SCALE_CELL {
        return 0;
    }
    (params.elevation_coarse_cell / 2).clamp(64, 200)
}

/// Ridge-noise half-width that reads as `hex_width` hexes of river at this `river_cell`.
/// The channel is interpolated over `river_cell`, so a wider cell at the same threshold is a
/// wider river — this inverts that, so a preset can name a width in hexes.
fn river_width_for(river_cell: i32, hex_width: i32) -> i32 {
    (hex_width * NOISE_MAX) / (2 * river_cell.max(1))
}

fn blend_elevation(coarse: i32, fine: i32, weight: i32) -> i32 {
    (coarse * weight + fine * (100 - weight)) / 100
}

/// Neighbour steps scale with the cell, so a `cliff_step` tuned for a 512-hex landform reads as
/// "everything is sheer" at the opening's cell 8. The inner disc uses the step that cell 8 / 3
/// actually needs; the regional value takes over with the landform.
fn cliff_step_at(params: &WorldParams, dist: i32) -> i32 {
    let radius = landing_radius(params);
    if radius == 0 || dist >= radius {
        return params.cliff_step;
    }
    let opening = 14_000;
    let inner = radius * 2 / 5;
    if dist <= inner {
        return opening;
    }
    let t = (dist - inner) * 100 / (radius - inner).max(1);
    (opening * (100 - t) + params.cliff_step * t) / 100
}

/// Same split for rivers: an 8-hex river on a 320-hex channel is a lake across the whole
/// opening, because the channel does not move. The inner disc keeps the one-hex creeks the
/// bootstrap was measured against; the wide river starts with the regional landform.
fn river_params_at(params: &WorldParams, dist: i32) -> (i32, i32) {
    let radius = landing_radius(params);
    let inner = radius * 2 / 5;
    if radius == 0 || dist >= inner || params.river_width == 0 {
        return (params.river_cell, params.river_width);
    }
    let cell = params.river_cell.min(32);
    (cell, river_width_for(cell, 1).min(params.river_width))
}

fn elevation_at(params: &WorldParams, seed: u32, q: i32, r: i32) -> i32 {
    let regional = blend_elevation(
        value_noise(seed, q, r, params.elevation_coarse_cell, 0xA11CE),
        value_noise(seed, q, r, params.elevation_fine_cell, 0xB0A7),
        params.elevation_coarse_weight,
    );
    let radius = landing_radius(params);
    let dist = axial_distance((0, 0), (q, r));
    if radius == 0 || dist >= radius {
        return regional;
    }
    // The inner two-fifths is the opening the bootstrap was tuned against. Past that the
    // regional landform takes over, so a three-minute plains is still a three-minute plains
    // once you leave the first minute.
    let local = blend_elevation(
        value_noise(
            seed,
            q,
            r,
            params.elevation_coarse_cell.min(OPENING_COARSE_CELL),
            0xA11CE,
        ),
        value_noise(
            seed,
            q,
            r,
            params.elevation_fine_cell.min(OPENING_FINE_CELL),
            0xB0A7,
        ),
        OPENING_COARSE_WEIGHT.min(params.elevation_coarse_weight),
    );
    let inner = radius * 2 / 5;
    if dist <= inner {
        return local;
    }
    let t = (dist - inner) * 100 / (radius - inner).max(1);
    (local * (100 - t) + regional * t) / 100
}

fn moisture_at(params: &WorldParams, seed: u32, q: i32, r: i32) -> i32 {
    value_noise(seed, q, r, params.moisture_cell, 0xC0A5)
}

fn terrain_at(
    params: &WorldParams,
    seed: u32,
    q: i32,
    r: i32,
    generated_environment: bool,
) -> Terrain {
    if !generated_environment {
        return Terrain::Lowland;
    }
    if axial_distance((0, 0), (q, r)) <= LANDING_CLEAR_RADIUS {
        return match (q, r) {
            (2, 1) | (2, 2) | (1, 2) => Terrain::ShallowWater,
            (1, -1) | (2, -1) => Terrain::Cliff,
            _ => Terrain::Lowland,
        };
    }
    let elevation = elevation_at(params, seed, q, r);
    let moisture = moisture_at(params, seed, q, r);
    if elevation < params.water_level {
        return if moisture > params.deep_water_moisture {
            Terrain::DeepWater
        } else {
            Terrain::ShallowWater
        };
    }
    if elevation < params.shore_level {
        return Terrain::Shore;
    }
    let dist = axial_distance((0, 0), (q, r));
    if is_river(params, seed, q, r, elevation) {
        return Terrain::ShallowWater;
    }
    let mut max_step = 0;
    for &(dq, dr) in &DIRECTIONS {
        max_step = max_step.max((elevation - elevation_at(params, seed, q + dq, r + dr)).abs());
    }
    if max_step > cliff_step_at(params, dist) {
        return Terrain::Cliff;
    }
    if elevation > params.highland_level {
        Terrain::Highland
    } else if elevation > params.hills_level {
        Terrain::Hills
    } else {
        Terrain::Lowland
    }
}

/// A river hex, which is inland `ShallowWater` rather than an accident of sea level.
///
/// A flow simulation is refused outright: the map is unbounded and generated lazily, so nothing
/// here may depend on knowing where the water upstream went. A river is instead where a dedicated
/// channel runs near its own midpoint, which is O(1) per hex, purely local, and fits the pure
/// `(params, seed, q, r)` contract exactly. `elevation` is passed in because every caller has just
/// computed it, and it is the gate that stops a river at the highland cut.
fn is_river(params: &WorldParams, seed: u32, q: i32, r: i32, elevation: i32) -> bool {
    let dist = axial_distance((0, 0), (q, r));
    if params.river_width == 0
        || elevation >= params.river_max_elevation
        || dist <= RIVER_CLEAR_RADIUS
    {
        return false;
    }
    let (cell, width) = river_params_at(params, dist);
    if width == 0 {
        return false;
    }
    let channel = value_noise(seed, q, r, cell, RIVER_OCTAVE);
    (channel - NOISE_MAX / 2).abs() < width
}

/// Water, asked the cheap way.
///
/// `terrain_at` samples seven elevations to answer the cliff question and a water test needs none
/// of them, so the hot paths that only want "is this wet" — the clay clipping and the barren
/// early-out in `field_at` — ask here instead. It mirrors `terrain_at` exactly, clearing included,
/// and a test asserts the two never disagree.
#[allow(dead_code)]
fn is_water_at(params: &WorldParams, seed: u32, q: i32, r: i32) -> bool {
    if axial_distance((0, 0), (q, r)) <= LANDING_CLEAR_RADIUS {
        return matches!((q, r), (2, 1) | (2, 2) | (1, 2));
    }
    let elevation = elevation_at(params, seed, q, r);
    elevation < params.water_level
        || (elevation >= params.shore_level && is_river(params, seed, q, r, elevation))
}

/// Item ids the generator writes into the world. Generation is content, so these name the shipped
/// catalog the same way the guaranteed opening below does.
const IRON_ORE: ItemId = 1;
const CRYSTAL: ItemId = 3;
const COPPER_ORE: ItemId = 4;
const COAL: ItemId = 5;
const STONE: ItemId = 6;
const SAND: ItemId = 7;
const CLAY: ItemId = 8;
const WOOD: ItemId = 9;
const LIMESTONE: ItemId = 26;
const CRUDE_OIL: ItemId = 28;

/// The shipped resource table. Order is no longer a generation input — the lattice weights one
/// rule against the others eligible for a band rather than taking the first that matches — so this
/// reads top to bottom as the bands do, from the tops to the coast.
///
/// Every number here was chosen against `npm run survey`, the way `cliff_step` was chosen in v0.16.
fn default_site_rules() -> Vec<SiteRule> {
    let rule =
        |terrain, item_id, weight, radius_min, radius_max, site_min, core, rim, jitter| SiteRule {
            terrain,
            item_id,
            weight,
            radius_min,
            radius_max,
            site_min,
            yield_core: core,
            yield_rim: rim,
            yield_jitter: jitter,
            member: Vec::new(),
            member_water_within: 0,
            center_ocean: false,
            center_shore: false,
        };
    // Iron and coal both belong to the tops and the rolling ground under them, so both name the
    // pair as members and neither is clipped to the band its centre happened to land in. That is
    // the whole mixed-material fix seen from the data side: they are separate *sites* now, so two
    // neighbouring fields is what a smelting site looks like, never one alternating hex.
    let ore_bands = vec![Terrain::Hills, Terrain::Highland];
    vec![
        SiteRule {
            member: ore_bands.clone(),
            ..rule(Terrain::Highland, IRON_ORE, 34, 3, 4, 28_000, 20, 8, 3)
        },
        SiteRule {
            member: ore_bands.clone(),
            ..rule(Terrain::Hills, IRON_ORE, 24, 3, 4, 28_000, 20, 8, 3)
        },
        SiteRule {
            member: ore_bands.clone(),
            ..rule(Terrain::Highland, COAL, 26, 2, 4, ANY, 18, 8, 3)
        },
        // Scree around mountains. Cliff hexes are members and are unworkable, so the buildable rim
        // is where you quarry — v0.11's extraction-radius lesson intact, at fifty times the supply
        // the eighteen cliff cells of version 6 could offer.
        SiteRule {
            member: vec![Terrain::Highland, Terrain::Cliff],
            ..rule(Terrain::Highland, STONE, 26, 3, 5, ANY, 12, 12, 2)
        },
        SiteRule {
            member: vec![Terrain::Hills, Terrain::Highland, Terrain::Cliff],
            ..rule(Terrain::Hills, STONE, 20, 3, 5, ANY, 12, 12, 2)
        },
        // Rare, finite, remote, and never guaranteed near the landing site. It is the reason to
        // leave. The rarity is the *radius*: one disc of seven hexes, usually clipped to less. A
        // richness gate on top of that read as scarcity on `continental` and as absence on
        // `basin`, whose highland is a tenth of the world — and a material that a preset can
        // simply not hold is not rare, it is missing.
        rule(Terrain::Highland, CRYSTAL, 18, 1, 2, ANY, 10, 10, 2),
        // Copper belongs to rolling ground and iron and coal to the tops, which is what the `Hills`
        // doc comment already promises. The pair above may spill down into hills; copper never
        // climbs.
        rule(Terrain::Hills, COPPER_ORE, 34, 2, 4, 30_000, 18, 8, 3),
        // Limestone is a hill quarry, not cliff scree. It is the binder feedstock, so it has to be
        // a readable site with buildable ground around it rather than a first-belt gift.
        rule(Terrain::Hills, LIMESTONE, 22, 2, 4, 28_000, 16, 8, 3),
        SiteRule {
            member: ore_bands,
            ..rule(Terrain::Hills, COAL, 16, 2, 3, 40_000, 18, 8, 3)
        },
        // A forest: 150–250 units across a large area, renewable through the `regrowth_ticks` the
        // item already carries, with a soft edge. Three per cell is a rate change as well as a
        // shape change — a base extractor drains its seven hexes and then runs at whatever regrowth
        // supplies — which is why forestry is a question of area rather than of throughput.
        rule(Terrain::Lowland, WOOD, 30, 5, 6, ANY, 3, 1, 2),
        rule(Terrain::Hills, WOOD, 18, 4, 6, ANY, 3, 1, 2),
        rule(Terrain::Lowland, CRUDE_OIL, 8, 2, 3, ANY, 40, 20, 4),
        rule(Terrain::Hills, CRUDE_OIL, 10, 2, 3, ANY, 40, 20, 4),
        // Riverbanks and lake shores. Rivers are what make this common rather than decorative,
        // which is why the two ship together. Shore-centred clay is the lighter of the two: the
        // sandy-looking tiles are the shore band, and sand has to be what you find on them first.
        SiteRule {
            member: vec![Terrain::Lowland, Terrain::Shore],
            member_water_within: 2,
            ..rule(Terrain::Lowland, CLAY, 24, 2, 3, ANY, 14, 14, 3)
        },
        rule(Terrain::Hills, CLAY, 12, 2, 3, ANY, 14, 14, 3),
        SiteRule {
            member: vec![Terrain::Lowland, Terrain::Shore],
            member_water_within: 2,
            ..rule(Terrain::Shore, CLAY, 16, 2, 3, ANY, 14, 14, 3)
        },
        // Sand sits on the shore band, clipped to it so a beach is a strip rather than a blob.
        // Any shore: a lake, a sea, and a pond all look sandy, and a player walking those tiles
        // should find sand. The ocean proxy used to refuse every inland beach.
        SiteRule {
            ..rule(Terrain::Shore, SAND, 40, 3, 5, ANY, 16, 16, 3)
        },
        // The same beach, reached from the land side. A shore band is a thin ribbon — 26 per mille
        // of `highlands` — so a rule that can only start *on* it is a coin flip on how many of a
        // handful of lattice cells happen to land in the ribbon. A centre just inland clips to
        // exactly the same strip; the shore gate keeps a forest cell from spending itself on an
        // empty disc that never reaches a beach.
        SiteRule {
            member: vec![Terrain::Shore],
            center_shore: true,
            ..rule(Terrain::Lowland, SAND, 26, 3, 5, ANY, 16, 16, 3)
        },
    ]
}

/// The opening a new world guarantees: a material, and the window its patch must fall in.
///
/// This replaced `LANDING_FIELD`, a hardcoded list of eight single cells — one of every material —
/// sitting inside the clearing. That constant, and not the generator, is why every material used
/// to be visible in the first minute; it was the sample platter the roadmap decision named.
///
/// A window is a distance from the landing site to the **nearest hex of the patch**, so it is what
/// the player actually walks, and its floor is what keeps a guaranteed disc from reaching inside
/// the clearing whose field suppression stays exactly as it was. Sand is not guaranteed by
/// distance — the ocean gate decides where a coast is — and crystal is never guaranteed at all.
const BOOTSTRAP_GUARANTEES: [(ItemId, i32, i32); 7] = [
    // The first extractor and the first thing a player walks into, both in sight of the hub.
    // Distances are hexes on the 25 m² lattice (~5.37 m), so 9 hexes is a short walk, not a
    // neighbouring tile.
    (IRON_ORE, 9, 24),
    (WOOD, 9, 24),
    // A short walk, chosen rather than stumbled on.
    (COAL, 15, 40),
    (STONE, 15, 40),
    // Carries a river or a shore with it, which is also the first pump site.
    (CLAY, 15, 40),
    // Binder feedstock: past the opening, before the copper expedition.
    (LIMESTONE, 18, 48),
    // The second metal is an expedition, not an errand.
    (COPPER_ORE, 25, 64),
];

/// How far a window is widened, per step and in total, when a seed puts nothing inside it. Past
/// the cap the world is refused rather than papered over: a preset that cannot bootstrap is the
/// failure the survey exists to make visible.
const BOOTSTRAP_WIDEN_STEP: i32 = 12;
const BOOTSTRAP_WIDEN_CAP: i32 = 96;

/// Make one band's deposits commoner and wider.
///
/// A preset that makes a band scarce is not allowed to make the materials in it unfindable as
/// well. `relaxed()` used to buy that by lowering the per-hex gates on the band's rows, and there
/// are no per-hex gates left to lower — a site is gated at its centre and nowhere else. Weight and
/// radius are the direct form of the same compensation, and they are the honest one: `npm run
/// survey` can see a patch that got wider or commoner, and could never see a gate that moved.
fn favoured(
    rules: Vec<SiteRule>,
    terrain: Terrain,
    weight_gain: u32,
    radius_gain: u32,
) -> Vec<SiteRule> {
    rules
        .into_iter()
        .map(|rule| {
            if rule.terrain != terrain || rule.weight == 0 {
                return rule;
            }
            SiteRule {
                weight: rule.weight + weight_gain,
                radius_max: (rule.radius_max + radius_gain).min(MAX_SITE_RADIUS),
                ..rule
            }
        })
        .collect()
}

/// A named parameter set. A preset is what a player picks; the parameter set is what makes a
/// preset a data row — the same relationship the shape grammar has to a building definition. The
/// raw parameters stay exposed behind the preset in the new-world flow, so the usable surface and
/// the maintainable one are the same table read at two depths.
#[derive(Clone, Debug, Serialize)]
struct WorldPreset {
    key: &'static str,
    name: &'static str,
    description: &'static str,
    params: WorldParams,
}

/// The preset a scenario generates under when nothing names another.
const DEFAULT_PRESET_KEY: &str = "continental";

fn world_presets() -> Vec<WorldPreset> {
    vec![
        WorldPreset {
            key: "continental",
            name: "Continental",
            description: "Mixed coasts and inland ranges. The shipped default.",
            params: WorldParams {
                // A hex is 25 m² and the walk is 15 m/s, so a landform of 512 hexes is a three-minute
                // crossing — plains and ranges you travel, not tiles you glance over. Weight 68
                // lets the coarse octave hold a coastline together; the fine octave is local
                // relief, not a second landform scale.
                elevation_coarse_cell: 512,
                elevation_fine_cell: 10,
                elevation_coarse_weight: 68,
                moisture_cell: 96,
                richness_cell: 64,
                water_level: 18_000,
                shore_level: 24_000,
                hills_level: 33_000,
                highland_level: 42_000,
                // Neighbour steps shrink as the cell grows. 2_400 is "sheer" at this fine scale;
                // the shipped 14_000 at cell 8 would never fire.
                cliff_step: 2_400,
                deep_water_moisture: 40_000,
                site_cell: 18,
                site_jitter: 5,
                // Eight hexes thick, about 320 hexes apart: a real river, and still a sparse wall
                // until v0.22 builds a bridge. Density is ~2.5% of walked hexes against the ~3%
                // the one-hex network ran at.
                river_cell: 320,
                river_width: river_width_for(320, 8),
                river_max_elevation: 42_000,
                ocean_level: 16_000,
                site_rules: default_site_rules(),
            },
        },
        WorldPreset {
            key: "archipelago",
            name: "Archipelago",
            description: "Small islands in scattered water. Short coasts, long walks.",
            params: WorldParams {
                // Islands you walk across, not tiles you step over: ~690 m / 45 s at 15 m/s, still
                // the small end of the four. Weight 60 holds a shore together at this cell without
                // turning the preset into one continent.
                elevation_coarse_cell: 128,
                elevation_fine_cell: 6,
                elevation_coarse_weight: 60,
                moisture_cell: 48,
                richness_cell: 40,
                water_level: 26_000,
                shore_level: 31_000,
                hills_level: 38_000,
                // 46_000 left almost no highland in the opening: cell 8 / 3 with a 26_000 sea
                // cut spends its top on a thin cap, and iron and stone both start on it. 42_000
                // is the same cap continental uses, so an island still has a top and a default
                // extractor can still be stood on it.
                highland_level: 42_000,
                // Broken ground is steep ground: the step that means "sheer" has to scale with
                // the gradient the feature scale produces.
                cliff_step: 4_200,
                deep_water_moisture: 44_000,
                site_cell: 24,
                site_jitter: 4,
                // Scattered water everywhere already; a river network on top of it would leave the
                // walkable ground in shreds.
                river_cell: 80,
                river_width: 0,
                river_max_elevation: 42_000,
                ocean_level: 26_000,
                // Every band here is scarce or shredded, so every band compensates in its own rows.
                // The tops survive least, the rolling ground carries the copper nothing else can,
                // and a forest on an island only reaches a workable size if its disc starts wider.
                site_rules: favoured(
                    favoured(
                        favoured(default_site_rules(), Terrain::Highland, 12, 2),
                        Terrain::Hills,
                        8,
                        2,
                    ),
                    Terrain::Lowland,
                    0,
                    2,
                ),
            },
        },
        WorldPreset {
            key: "highlands",
            name: "Highlands",
            description: "High ground and hard rock. Little water, much cliff.",
            params: WorldParams {
                // Ranges you walk: ~690 m / four minutes. The finest cliffs of the four, because
                // this is the hard-rock preset.
                elevation_coarse_cell: 640,
                elevation_fine_cell: 12,
                elevation_coarse_weight: 72,
                moisture_cell: 80,
                richness_cell: 64,
                water_level: 12_000,
                shore_level: 16_000,
                hills_level: 26_000,
                highland_level: 36_000,
                cliff_step: 1_600,
                deep_water_moisture: 38_000,
                site_cell: 20,
                site_jitter: 5,
                // The preset with the least standing water is the one rivers do the most for: they
                // are where its clay, its pumps, and its hydro come from. Ten hexes thick so a
                // highland river reads as a river.
                river_cell: 240,
                river_width: river_width_for(240, 10),
                river_max_elevation: 36_000,
                // The one preset with no ocean at all: 41 bodies in a 27,937-hex sample and the
                // largest of them 46 hexes. A gate its own basins cannot clear does not make its
                // beaches rarer, it deletes sand from the world — so the cut sits where those
                // basins pass it. "Sand sits on the largest water this world has" is the honest
                // reading of the same rule, and the survey prints the body size that says so.
                ocean_level: 22_000,
                // Almost no shore band, so the sand and clay it does hold are common inside it.
                // Lowland is the valley floor and is scarce too: a forest has to start wider or
                // the largest patch cannot fill a deep extractor.
                site_rules: favoured(
                    favoured(default_site_rules(), Terrain::Shore, 40, 2),
                    Terrain::Lowland,
                    0,
                    2,
                ),
            },
        },
        WorldPreset {
            key: "basin",
            name: "Basin",
            description: "Great contiguous seas around broad land. Ocean, not ponds.",
            params: WorldParams {
                // The sea end of the same scale: 960 hexes is a six-minute landform, and a body
                // that spans two of those is an ocean you do not walk around. Weight 82 is what
                // holds a coastline together at this cell.
                elevation_coarse_cell: 960,
                elevation_fine_cell: 16,
                elevation_coarse_weight: 82,
                moisture_cell: 120,
                richness_cell: 72,
                water_level: 22_000,
                shore_level: 27_000,
                hills_level: 36_000,
                highland_level: 45_000,
                cliff_step: 1_000,
                deep_water_moisture: 40_000,
                site_cell: 18,
                site_jitter: 5,
                river_cell: 400,
                river_width: river_width_for(400, 10),
                river_max_elevation: 45_000,
                ocean_level: 22_000,
                site_rules: default_site_rules(),
            },
        },
    ]
}

fn preset_params(key: &str) -> Option<WorldParams> {
    world_presets()
        .into_iter()
        .find(|preset| preset.key == key)
        .map(|preset| preset.params)
}

fn default_world_params() -> WorldParams {
    preset_params(DEFAULT_PRESET_KEY).expect("the default preset is in the table")
}

/// One field of a lattice cell's hash. Four are drawn — two for the centre offset, one for the
/// weighted pick, one for the radius — and they are separate hashes rather than bit slices of one
/// value, so a weight sum that happens to sit near a power of two is not quietly biased. A site
/// cell covers `site_cell²` hexes and the lattice is cached, so this is paid once per deposit
/// rather than once per hex.
fn site_field(hash: u32, index: i32) -> u32 {
    coordinate_hash(hash, index, SITE_FIELD_ROW)
}

fn site_hash(seed: u32, cell: (i32, i32)) -> u32 {
    coordinate_hash(seed ^ SITE_SALT, cell.0, cell.1)
}

/// Where in its own cell a site stands. The jitter is what keeps a world of deposits from reading
/// as a world on a grid.
fn site_center(params: &WorldParams, hash: u32, cell: (i32, i32)) -> (i32, i32) {
    let span = (2 * params.site_jitter + 1) as u32;
    let offset = |index: i32| (site_field(hash, index) % span) as i32 - params.site_jitter;
    (
        cell.0 * params.site_cell + offset(0),
        cell.1 * params.site_cell + offset(1),
    )
}

/// Whether the coarse elevation octave alone dips below `ocean_level` near a centre — the proxy
/// `SiteRule::center_ocean` documents. Coarse-octave water is what makes a body big, so a pond
/// edge, which exists only in the fine octave, fails this and an ocean coast passes.
fn center_on_ocean(
    params: &WorldParams,
    seed: u32,
    center: (i32, i32),
    spine: &GroundSpine,
) -> bool {
    hexes_in_radius(center, OCEAN_PROBE_RADIUS)
        .into_iter()
        .any(|(q, r)| {
            if spine.is_physical() {
                let ground = spine.generated_at(q, r);
                ground.bed.get() <= scale::SEA_LEVEL_QUANTA
                    || ground.hydrology.depth_quanta >= scale::WADE_LIMIT_QUANTA
            } else {
                value_noise(seed, q, r, params.elevation_coarse_cell, 0xA11CE) < params.ocean_level
            }
        })
}

/// Whether the shore band sits next to a centre — the cheap elevation-cut form of "this is a
/// beach site". `terrain_at` would also sample cliffs; a water test would also fire on rivers,
/// which are clay country. Shore is the sandy-looking tiles, and that is the only band asked.
fn center_on_shore(
    params: &WorldParams,
    seed: u32,
    center: (i32, i32),
    spine: &GroundSpine,
) -> bool {
    hexes_in_radius(center, SHORE_PROBE_RADIUS)
        .into_iter()
        .any(|(q, r)| {
            if spine.is_physical() {
                spine.presentation_at(q, r) == Terrain::Shore
            } else {
                let elevation = elevation_at(params, seed, q, r);
                elevation >= params.water_level && elevation < params.shore_level
            }
        })
}

/// The rules a centre is eligible for, and the pick among them. Returns an index into the rule
/// table. `None` means this cell holds no site at all, which is how barren ground stays the common
/// case.
fn eligible_rule(
    params: &WorldParams,
    seed: u32,
    hash: u32,
    center: (i32, i32),
    spine: &GroundSpine,
) -> Option<usize> {
    let band = spine.presentation_at(center.0, center.1);
    let richness = value_noise(
        seed,
        center.0,
        center.1,
        params.richness_cell,
        RICHNESS_OCTAVE,
    );
    let mut ocean: Option<bool> = None;
    let mut shore: Option<bool> = None;
    let mut admits = |rule: &SiteRule| {
        if rule.weight == 0 || rule.terrain != band || richness <= rule.site_min {
            return false;
        }
        if rule.center_ocean {
            // Asked at most once per cell, and only for a rule that got this far.
            return *ocean.get_or_insert_with(|| center_on_ocean(params, seed, center, spine));
        }
        if rule.center_shore {
            return *shore.get_or_insert_with(|| center_on_shore(params, seed, center, spine));
        }
        true
    };
    let mut total = 0u32;
    for rule in &params.site_rules {
        if admits(rule) {
            total += rule.weight;
        }
    }
    if total == 0 {
        return None;
    }
    let mut pick = site_field(hash, 2) % total;
    for (index, rule) in params.site_rules.iter().enumerate() {
        if !admits(rule) {
            continue;
        }
        if pick < rule.weight {
            return Some(index);
        }
        pick -= rule.weight;
    }
    None
}

/// The site a lattice cell holds before the bootstrap pass has its say. A pure function of
/// `(params, seed, cell)`, which is exactly what lets the lattice be cached.
fn natural_site(
    params: &WorldParams,
    seed: u32,
    cell: (i32, i32),
    spine: &GroundSpine,
) -> Option<Site> {
    let hash = site_hash(seed, cell);
    let center = site_center(params, hash, cell);
    let index = eligible_rule(params, seed, hash, center, spine)?;
    let rule = &params.site_rules[index];
    let span = rule.radius_max - rule.radius_min + 1;
    Some(Site {
        center,
        rule: index,
        radius: (rule.radius_min + site_field(hash, 3) % span) as i32,
        forced_opening: false,
    })
}

/// Whether a site admits one hex, and how far that hex is from its centre.
///
/// `band` is passed in because every caller has just computed it and a band decision costs seven
/// elevation samples. The member test is the clipping that makes a beach a strip rather than a
/// blob and keeps a scree field against its cliffs.
fn site_covers(
    params: &WorldParams,
    _seed: u32,
    site: &Site,
    q: i32,
    r: i32,
    band: Terrain,
    spine: &GroundSpine,
) -> Option<i32> {
    let distance = axial_distance(site.center, (q, r));
    if distance > site.radius {
        return None;
    }
    let rule = &params.site_rules[site.rule];
    let admitted = if spine.is_physical() && site.forced_opening {
        !spine.wet_at(q, r)
    } else if rule.member.is_empty() {
        band == rule.terrain
    } else {
        rule.member.contains(&band)
    };
    if !admitted {
        return None;
    }
    if rule.member_water_within > 0
        && !hexes_in_radius((q, r), rule.member_water_within as i32)
            .into_iter()
            .any(|(cell_q, cell_r)| spine.wet_at(cell_q, cell_r))
    {
        return None;
    }
    Some(distance)
}

/// The guaranteed opening, resolved once from `(params, seed)`.
///
/// Spirals outward over lattice cells in a fixed order and, for each guarantee, claims the first
/// unclaimed cell whose centre band admits that material, whose forced disc lands inside the
/// window, and which actually holds a workable patch once the member test has clipped it. A
/// claimed cell is forced to that rule at `radius_max`.
///
/// Two things make this correct rather than merely deterministic. The window is a floor as well as
/// a ceiling, so a guaranteed disc can never reach inside the clearing. And a window that finds
/// nothing widens in fixed steps to a hard cap and then reports the guarantee as unmet, which
/// `Core::new` refuses the world over — `highlands` has almost no Shore band and is the preset that
/// will find this.
///
/// Derived state on the same terms as the site cache: recomputed from `(params, seed)`, never
/// saved, never hashed. The free function is shared by `Core`, the survey, and the balance report,
/// so a surveyed world and a played world cannot disagree about the opening.
fn bootstrap_sites(
    params: &WorldParams,
    seed: u32,
    spine: &GroundSpine,
) -> (BootstrapTable, Vec<(ItemId, i32)>) {
    let mut claimed: BootstrapTable = BTreeMap::new();
    let mut unmet = Vec::new();
    let cells = bootstrap_cells(params, seed);
    for &(item_id, floor, ceiling) in &BOOTSTRAP_GUARANTEES {
        let mut reach = ceiling;
        let placed = loop {
            let found = cells.iter().find_map(|&(distance, cell, center)| {
                if claimed.contains_key(&cell) {
                    return None;
                }
                let (index, forced_opening) = bootstrap_rule(params, seed, center, item_id, spine)?;
                let site = Site {
                    center,
                    rule: index,
                    radius: params.site_rules[index].radius_max as i32,
                    forced_opening,
                };
                let edge = distance - site.radius;
                if edge < floor || edge > reach {
                    return None;
                }
                (member_hexes(params, seed, &site, spine) >= WORKABLE_PATCH_HEXES)
                    .then_some((cell, site))
            });
            if let Some(found) = found {
                break Some(found);
            }
            if reach >= ceiling + BOOTSTRAP_WIDEN_CAP {
                break None;
            }
            reach += BOOTSTRAP_WIDEN_STEP;
        };
        match placed {
            Some((cell, site)) => {
                claimed.insert(cell, site);
            }
            None => unmet.push((item_id, ceiling + BOOTSTRAP_WIDEN_CAP)),
        }
    }
    (claimed, unmet)
}

/// Every lattice cell the bootstrap pass may claim, nearest centre first.
///
/// The spiral, written as a sort rather than as a ring walk. The order has to be fixed and a
/// hand-rolled ring walk is exactly where that goes wrong; the centre distance is what makes it a
/// spiral, and the cell breaks every tie so nothing is decided by iteration order.
///
/// Shared with the diagnosis below, which is the point of it being a function: what a repair
/// measures has to be the ground the pass actually looked at, not a disc that resembles it.
fn bootstrap_cells(params: &WorldParams, seed: u32) -> Vec<SpiralStep> {
    let furthest = BOOTSTRAP_GUARANTEES
        .iter()
        .map(|&(_, _, ceiling)| ceiling)
        .max()
        .unwrap_or(0)
        + BOOTSTRAP_WIDEN_CAP;
    let span = (furthest + MAX_SITE_RADIUS as i32) / params.site_cell + 2;
    let mut cells: Vec<SpiralStep> = Vec::new();
    for cell_q in -span..=span {
        for cell_r in -span..=span {
            let cell = (cell_q, cell_r);
            let center = site_center(params, site_hash(seed, cell), cell);
            cells.push((axial_distance((0, 0), center), cell, center));
        }
    }
    cells.sort_unstable();
    cells
}

/// The rule a guaranteed cell is forced to: the first row for this material whose band the centre
/// stands in and whose ocean gate it clears. The richness gate is deliberately *not* asked — a
/// guarantee that poor country could veto is not a guarantee.
fn bootstrap_rule(
    params: &WorldParams,
    seed: u32,
    center: (i32, i32),
    item_id: ItemId,
    spine: &GroundSpine,
) -> Option<(usize, bool)> {
    let band = spine.presentation_at(center.0, center.1);
    let exact = params.site_rules.iter().position(|rule| {
        rule.weight > 0
            && rule.item_id == item_id
            && rule.terrain == band
            && (!rule.center_ocean || center_on_ocean(params, seed, center, spine))
            && (!rule.center_shore || center_on_shore(params, seed, center, spine))
    });
    if let Some(index) = exact {
        return Some((index, false));
    }
    if !spine.is_physical() || spine.wet_at(center.0, center.1) {
        return None;
    }
    // The translated physical opening is a valley shelf rather than a miniature sample of every
    // old presentation band. When a shelf does not expose the band a material used to name, force
    // its first authored rule as a dry local outcrop; yield, radius and water-proximity policy
    // still come from that rule.
    params
        .site_rules
        .iter()
        .position(|rule| rule.weight > 0 && rule.item_id == item_id)
        .map(|index| (index, true))
}

/// How many hexes a site actually admits once its member test has clipped the disc. A guarantee
/// that lands a highland rule on a peak with nothing around it is not a guarantee, so the
/// bootstrap pass asks this before it claims a cell.
fn member_hexes(params: &WorldParams, seed: u32, site: &Site, spine: &GroundSpine) -> u32 {
    hexes_in_radius(site.center, site.radius)
        .into_iter()
        .filter(|&(q, r)| {
            !spine.wet_at(q, r)
                && axial_distance((0, 0), (q, r)) > LANDING_CLEAR_RADIUS
                && site_covers(params, seed, site, q, r, spine.presentation_at(q, r), spine)
                    .is_some()
        })
        .count() as u32
}

/// The bands a rule could seat this material's guaranteed centre in.
///
/// The centre's band is what `bootstrap_rule` gates on, so this is the ground a guarantee is
/// actually looking for — not the ground its disc ends up covering, which the member test decides
/// afterwards.
fn bootstrap_bands(params: &WorldParams, item_id: ItemId) -> Vec<Terrain> {
    let mut bands: Vec<Terrain> = params
        .site_rules
        .iter()
        .filter(|rule| rule.weight > 0 && rule.item_id == item_id)
        .map(|rule| rule.terrain)
        .collect();
    bands.sort_unstable();
    bands.dedup();
    bands
}

/// The bands the bootstrap pass could actually stand on, as the set of every lattice centre's band.
///
/// This is what separates the two ways an opening fails. A band that is not in here at all means
/// the world holds no such ground near the landing site and no seed will find any; a band that is
/// in here means the ground exists and the guarantee failed on room, distance, or a patch too
/// small — which is a different sentence and a different fix.
fn bootstrap_band_census(
    params: &WorldParams,
    seed: u32,
    spine: &GroundSpine,
) -> BTreeSet<Terrain> {
    bootstrap_cells(params, seed)
        .iter()
        .map(|&(_, _, center)| spine.presentation_at(center.0, center.1))
        .collect()
}

/// Whether a parameter set opens at this seed, which is the only question a repair candidate is
/// judged on. Every suggestion below is put through it, so nothing is offered on the strength of
/// the reasoning that produced it.
fn bootstraps(params: &WorldParams, seed: u32) -> bool {
    let spine = GroundSpine::physical(params, seed, true);
    bootstrap_sites(params, seed, &spine).1.is_empty()
}

/// The share of the opening a band is widened to when a guarantee cannot find it. A starting point
/// rather than a rule: what decides a repair is the verification, and this is only where the search
/// for one begins.
const REPAIR_BAND_SHARE: usize = 15;
/// Deposit spacings a repair will try, widest first, so a fix settles on the largest lattice that
/// still opens the world rather than the smallest one that certainly does.
const REPAIR_SPACINGS: [i32; 5] = [32, 24, 16, 12, 8];
/// Seeds a repair will try before it touches a parameter at all. A seed is the one thing on the
/// form the player did not choose, so rerolling it is the fix that costs them nothing — but a
/// world that drowns every material drowns them under every seed, which is why the list is short.
const REPAIR_SEEDS: u32 = 8;

/// A seed that opens this world with every parameter left alone.
fn repair_seed(params: &WorldParams, seed: u32) -> Option<u32> {
    (1..=REPAIR_SEEDS)
        .map(|step| seed.wrapping_add(step))
        .find(|&candidate| bootstraps(params, candidate))
}

/// One way a repair may turn a knob. Takes the bands the failed guarantees were looking for,
/// because a repair that widened every band would be a reset rather than a fix.
type RepairMove = fn(&WorldParams, u32, &[Terrain]) -> WorldParams;

/// Ways to repair a world, fewest knobs first. Every rung is verified, so a rung that does not open
/// the world is simply never offered — which is what lets the list stay a list of guesses.
const REPAIR_LADDER: [&[RepairMove]; 4] = [
    &[],
    &[repair_cuts],
    &[repair_landform],
    &[repair_cuts, repair_landform, repair_rivers],
];

/// A parameter set that opens this world at the seed the player is on, or none that was found.
///
/// The search is a ladder rather than a solver: a handful of candidates, ordered so the first one
/// that works is also the one that does least to what the player asked for. Deposit spacing is the
/// outer loop because it is the knob a repair would rather not touch — a player who set it to an
/// expedition per material meant it — so everything else is tried at their spacing first.
fn repair_params(params: &WorldParams, seed: u32) -> Option<WorldParams> {
    let spine = GroundSpine::physical(params, seed, true);
    let unmet = bootstrap_sites(params, seed, &spine).1;
    let mut needed: Vec<Terrain> = unmet
        .iter()
        .flat_map(|&(item_id, _)| bootstrap_bands(params, item_id))
        .collect();
    needed.sort_unstable();
    needed.dedup();
    let spacings = std::iter::once(params.site_cell).chain(
        REPAIR_SPACINGS
            .into_iter()
            .filter(|&cell| cell < params.site_cell),
    );
    for site_cell in spacings {
        for moves in REPAIR_LADDER {
            let mut candidate = WorldParams {
                site_cell,
                ..params.clone()
            };
            for step in moves {
                candidate = step(&candidate, seed, &needed);
            }
            // The unchanged set is the one that is already known to fail, and a candidate native
            // would refuse is not a fix — `Core::new` would decline it on arrival.
            if candidate != *params
                && candidate.band_levels_ascend()
                && bootstraps(&candidate, seed)
            {
                return Some(candidate);
            }
        }
    }
    None
}

/// Give the landform back the scale the opening was tuned against.
///
/// Below `LANDING_SCALE_CELL` there is no opening blend at all and the ground near the hub is a
/// mosaic at the regional cell: every band is present and none of them holds a patch big enough to
/// stand an extractor on. Raising the cell is what turns that mosaic back into country.
fn repair_landform(params: &WorldParams, _seed: u32, _needed: &[Terrain]) -> WorldParams {
    WorldParams {
        elevation_coarse_cell: params.elevation_coarse_cell.max(LANDING_SCALE_CELL),
        ..params.clone()
    }
}

/// Narrow rivers to the creeks the bootstrap was measured against. A river is shallow water
/// whatever the elevation cuts say, so a wide enough channel drowns an opening the cuts alone
/// cannot rescue.
fn repair_rivers(params: &WorldParams, _seed: u32, _needed: &[Terrain]) -> WorldParams {
    WorldParams {
        river_width: params
            .river_width
            .min(river_width_for(params.river_cell, 1)),
        ..params.clone()
    }
}

/// Move the band cuts so every band a failed guarantee was looking for has room in the ground the
/// opening actually holds.
///
/// The cuts are quantiles of the elevation around the landing site, which is what makes "give
/// highland a share of the ground" a thing that can be computed rather than guessed: a band is a
/// slice of that distribution and not a number on a slider. Each starving band takes its room from
/// *below*, so its own lower cut is the one that moves — a world missing highland is repaired
/// without raising the sea, and a drowned world is repaired by lowering the cut that drowned it.
fn repair_cuts(params: &WorldParams, seed: u32, needed: &[Terrain]) -> WorldParams {
    let mut samples: Vec<i32> = bootstrap_cells(params, seed)
        .iter()
        .map(|&(_, _, center)| elevation_at(params, seed, center.0, center.1))
        .collect();
    if samples.is_empty() {
        return params.clone();
    }
    samples.sort_unstable();
    let room = (samples.len() * REPAIR_BAND_SHARE / 100).max(1);
    let at = |index: usize| samples[index.min(samples.len() - 1)];
    let mut next = params.clone();
    // Top down, so each band is measured against a ceiling that has already stopped moving.
    if needed.contains(&Terrain::Highland) {
        let above = samples.len() - samples.partition_point(|&e| e <= next.highland_level);
        if above < room {
            next.highland_level = at(samples.len() - room) - 1;
        }
    }
    if needed.contains(&Terrain::Hills) {
        let ceiling = samples.partition_point(|&e| e <= next.highland_level);
        if ceiling
            - samples
                .partition_point(|&e| e <= next.hills_level)
                .min(ceiling)
            < room
        {
            next.hills_level = at(ceiling.saturating_sub(room)) - 1;
        }
    }
    if needed.contains(&Terrain::Lowland) {
        let ceiling = samples.partition_point(|&e| e <= next.hills_level);
        if ceiling
            - samples
                .partition_point(|&e| e < next.shore_level)
                .min(ceiling)
            < room
        {
            next.shore_level = at(ceiling.saturating_sub(room));
        }
    }
    if needed.contains(&Terrain::Shore) {
        let ceiling = samples.partition_point(|&e| e < next.shore_level);
        if ceiling
            - samples
                .partition_point(|&e| e < next.water_level)
                .min(ceiling)
            < room
        {
            next.water_level = at(ceiling.saturating_sub(room));
        }
    }
    for level in [
        &mut next.water_level,
        &mut next.shore_level,
        &mut next.hills_level,
        &mut next.highland_level,
    ] {
        *level = (*level).clamp(0, NOISE_MAX);
    }
    // A band that took its room from below can leave a cut stranded above the one over it — a sea
    // higher than its own shore, which `validate` refuses. Each cut follows the one above it back
    // down, which keeps the rule the moves are built on: the sea falls, it never rises. A chain
    // that bottoms out at zero simply fails to ascend, and an unascending candidate is discarded
    // rather than offered.
    next.hills_level = next.hills_level.min(next.highland_level.saturating_sub(1));
    next.shore_level = next.shore_level.min(next.hills_level.saturating_sub(1));
    next.water_level = next.water_level.min(next.shore_level.saturating_sub(1));
    next
}

/// The resource field of one world: a pure function of parameters, seed, and hex, with the lattice
/// those answers are derived from cached.
///
/// The cache is the site lattice and never the field. `field_at` is not only called during
/// `generate_chunk` — `deposit_candidates` walks a whole disc, and `resource_at_world`, both
/// gathers, and every snapshot build reach it — and the naive form evaluates every lattice cell
/// within reach per hex, each one deciding a band, which is roughly 350 noise samples per hex and
/// is not shippable. A site cell is `site_cell²` hexes, so the map stays small and every hex in a
/// chunk hits it warm.
///
/// Both the lattice and the bootstrap table are derived state under the existing invariant: never
/// saved, never hashed, never checksummed, rebuilt whenever the world changes, exactly as
/// `deposit_links` is.
struct WorldFields {
    params: WorldParams,
    seed: u32,
    /// How far from the cell holding a hex a site may still reach it, in lattice cells.
    ///
    /// A site's centre sits inside its own cell plus `site_jitter`, and `axial_distance <= radius`
    /// implies each axial component is at most `radius`, so a cell more than
    /// `(radius_max + site_jitter + site_cell - 1) / site_cell` away cannot cover the hex. That is
    /// a derivation rather than a margin: a reach one cell short loses deposits silently.
    reach: i32,
    bootstrap: BootstrapTable,
    /// Guarantees the bootstrap pass could not place, with the distance it gave up at, so a caller
    /// can refuse the world instead of shipping a world that cannot be opened.
    unmet: Vec<(ItemId, i32)>,
    sites: RefCell<BTreeMap<(i32, i32), Option<Site>>>,
}

impl WorldFields {
    fn new(params: &WorldParams, seed: u32, spine: &GroundSpine) -> Self {
        let (bootstrap, unmet) = bootstrap_sites(params, seed, spine);
        let radius_max = params
            .site_rules
            .iter()
            .map(|rule| rule.radius_max as i32)
            .max()
            .unwrap_or(0);
        Self {
            reach: (radius_max + params.site_jitter + params.site_cell - 1) / params.site_cell,
            params: params.clone(),
            seed,
            bootstrap,
            unmet,
            sites: RefCell::new(BTreeMap::new()),
        }
    }

    fn site_at(&self, cell: (i32, i32), spine: &GroundSpine) -> Option<Site> {
        if let Some(&site) = self.sites.borrow().get(&cell) {
            return site;
        }
        let site = self.site_uncached(cell, spine);
        self.sites.borrow_mut().insert(cell, site);
        site
    }

    /// The same answer with the cache bypassed. The survey and the tests call the generator without
    /// a warm lattice, and one test asserts the two paths agree over a disc.
    fn site_uncached(&self, cell: (i32, i32), spine: &GroundSpine) -> Option<Site> {
        self.bootstrap
            .get(&cell)
            .copied()
            .or_else(|| natural_site(&self.params, self.seed, cell, spine))
    }

    /// What the bootstrap pass actually placed, per guaranteed material: the walk from the landing
    /// site to the nearest hex of the patch, and how many hexes the patch holds once the member
    /// test has clipped it. A guarantee the pass gave up on is simply absent, which is the shape
    /// every caller wants — the survey prints it as `none` and `Core::new` refuses the world.
    #[cfg(not(target_arch = "wasm32"))]
    fn guarantees(&self, spine: &GroundSpine) -> Vec<(ItemId, u32, u32)> {
        self.bootstrap
            .values()
            .map(|site| {
                (
                    self.params.site_rules[site.rule].item_id,
                    (axial_distance((0, 0), site.center) - site.radius).max(0) as u32,
                    member_hexes(&self.params, self.seed, site, spine),
                )
            })
            .collect()
    }

    fn field_at(
        &self,
        q: i32,
        r: i32,
        generated_environment: bool,
        spine: &GroundSpine,
    ) -> Option<ResourceState> {
        if !generated_environment {
            return None;
        }
        // The clearing is a promise rather than a landscape, and its field suppression is what the
        // bootstrap windows are measured against.
        if axial_distance((0, 0), (q, r)) <= LANDING_CLEAR_RADIUS {
            return None;
        }
        // No rule may name a water band — `validate` refuses one that tries — so the cheap water
        // test comes before the lattice scan and before the seven elevations a band costs.
        if spine.wet_at(q, r) {
            return None;
        }
        let band = spine.presentation_at(q, r);
        let cell = (
            floor_div(q, self.params.site_cell),
            floor_div(r, self.params.site_cell),
        );
        let mut best: Option<((i32, i32, i32), Site)> = None;
        for step_q in -self.reach..=self.reach {
            for step_r in -self.reach..=self.reach {
                let candidate = (cell.0 + step_q, cell.1 + step_r);
                let Some(site) = self.site_at(candidate, spine) else {
                    continue;
                };
                let Some(distance) = site_covers(&self.params, self.seed, &site, q, r, band, spine)
                else {
                    continue;
                };
                // Nearest centre wins, and the lattice cell breaks the tie. Ties must be broken
                // explicitly: a tie resolved by iteration order is a tie resolved by nothing, and
                // this is a checksum input.
                let key = (distance, candidate.0, candidate.1);
                if best.as_ref().is_none_or(|(current, _)| key < *current) {
                    best = Some((key, site));
                }
            }
        }
        let ((distance, _, _), site) = best?;
        let rule = &self.params.site_rules[site.rule];
        // Linear from core to rim, so the middle of a field is worth aiming an extractor at.
        let span = site.radius.max(1);
        let core = rule.yield_core as i32;
        let rim = rule.yield_rim as i32;
        let interpolated = rim + (core - rim) * (span - distance) / span;
        let quantity =
            interpolated.max(1) as u32 + coordinate_hash(self.seed, q, r) % rule.yield_jitter;
        Some(ResourceState {
            item_id: rule.item_id,
            quantity,
            initial_quantity: quantity,
        })
    }
}

fn inventory_total(inventory: &BTreeMap<ItemId, u32>) -> u32 {
    inventory.values().sum()
}

fn has_ingredients(inventory: &BTreeMap<ItemId, u32>, ingredients: &[Ingredient]) -> bool {
    ingredients.iter().all(|ingredient| {
        inventory.get(&ingredient.item_id).copied().unwrap_or(0) >= ingredient.quantity
    })
}

fn subtract_item(inventory: &mut BTreeMap<ItemId, u32>, item_id: ItemId, quantity: u32) {
    let stored = inventory
        .get_mut(&item_id)
        .expect("validated inventory item exists");
    *stored -= quantity;
    if *stored == 0 {
        inventory.remove(&item_id);
    }
}

fn add_ingredients(inventory: &mut BTreeMap<ItemId, u32>, ingredients: &[Ingredient]) {
    for ingredient in ingredients {
        *inventory.entry(ingredient.item_id).or_default() += ingredient.quantity;
    }
}

fn add_inventory(target: &mut BTreeMap<ItemId, u32>, source: &BTreeMap<ItemId, u32>) {
    for (&item, &quantity) in source {
        *target.entry(item).or_default() += quantity;
    }
}

fn expand_components(affected: &mut BTreeSet<u32>, adjacency: &BTreeMap<u32, BTreeSet<u32>>) {
    let mut pending: Vec<u32> = affected.iter().copied().collect();
    while let Some(id) = pending.pop() {
        let Some(neighbors) = adjacency.get(&id) else {
            continue;
        };
        for &neighbor in neighbors {
            if affected.insert(neighbor) {
                pending.push(neighbor);
            }
        }
    }
}

fn hash_inventory(hash: &mut u32, inventory: &BTreeMap<ItemId, u32>) {
    for (&item, &quantity) in inventory {
        hash_u32(hash, u32::from(item));
        hash_u32(hash, quantity);
    }
    hash_u32(hash, u32::MAX);
}

/// Every field of a parameter set, in declared order. A world's identity is its seed *and* its
/// parameters, so a checksum that hashed only the seed would call two different worlds the same
/// one — including the rule table, whose row order is itself a generation input.
fn hash_world_params(hash: &mut u32, params: &WorldParams) {
    for value in [
        params.elevation_coarse_cell,
        params.elevation_fine_cell,
        params.elevation_coarse_weight,
        params.moisture_cell,
        params.richness_cell,
        params.water_level,
        params.shore_level,
        params.hills_level,
        params.highland_level,
        params.cliff_step,
        params.deep_water_moisture,
        params.site_cell,
        params.site_jitter,
        params.river_cell,
        params.river_width,
        params.river_max_elevation,
        params.ocean_level,
    ] {
        hash_i32(hash, value);
    }
    for rule in &params.site_rules {
        hash_u32(hash, rule.terrain as u32);
        hash_u32(hash, u32::from(rule.item_id));
        hash_u32(hash, rule.weight);
        hash_u32(hash, rule.radius_min);
        hash_u32(hash, rule.radius_max);
        hash_i32(hash, rule.site_min);
        hash_u32(hash, rule.yield_core);
        hash_u32(hash, rule.yield_rim);
        hash_u32(hash, rule.yield_jitter);
        for &band in &rule.member {
            hash_u32(hash, band as u32);
        }
        hash_u32(hash, u32::MAX);
        hash_u32(hash, rule.member_water_within);
        hash_u32(hash, u32::from(rule.center_ocean));
        hash_u32(hash, u32::from(rule.center_shore));
    }
    hash_u32(hash, u32::MAX);
}

fn hash_bytes(hash: &mut u32, bytes: &[u8]) {
    for &byte in bytes {
        *hash ^= u32::from(byte);
        *hash = hash.wrapping_mul(0x01000193);
    }
}

fn hash_u32(hash: &mut u32, value: u32) {
    hash_bytes(hash, &value.to_le_bytes());
}

fn hash_i32(hash: &mut u32, value: i32) {
    hash_u32(hash, value as u32);
}

fn hash_u64(hash: &mut u32, value: u64) {
    hash_bytes(hash, &value.to_le_bytes());
}

/// Reproducible Phase 8 disturbed-water measurement.
///
/// The active case records the solver's own bounded work counters. The quiet case advances a
/// settled world and checks that no water dirty mark or state change appears: settled water has no
/// scheduled kernel, so its measured water work is exactly zero rather than a small per-cell cost.
#[cfg(not(target_arch = "wasm32"))]
pub mod water_bench {
    use super::*;
    use std::time::Instant;

    #[derive(Debug, Serialize)]
    pub struct Report {
        pub seed: u32,
        pub command_quanta: u16,
        pub active_cells: usize,
        pub sweeps: u32,
        pub transfers: u64,
        pub frontier_quanta: i64,
        pub settled: bool,
        pub active_micros: u128,
        pub quiet_ticks: u32,
        pub quiet_micros: u128,
        pub quiet_water_dirty: bool,
        pub quiet_state_changed: bool,
    }

    pub fn run() -> Report {
        const SEED: u32 = 1_213_486_160;
        const QUIET_TICKS: u32 = 100_000;
        let definitions: DefinitionsInput =
            serde_json::from_str(include_str!("../../src/data/definitions.json")).unwrap();
        let technologies: TechnologiesInput =
            serde_json::from_str(include_str!("../../src/data/technologies.json")).unwrap();
        let scenarios: ScenariosInput =
            serde_json::from_str(include_str!("../../src/data/scenarios.json")).unwrap();
        let scenario = scenarios
            .scenarios
            .iter()
            .find(|scenario| scenario.key == "new-game")
            .unwrap();
        let mut core = Core::new(&definitions, &technologies, scenario, Some(SEED), None).unwrap();
        let size = core.scenario.chunk_size;
        let (q, r) = core
            .generated_chunks
            .iter()
            .flat_map(|&(chunk_q, chunk_r)| hexes_in_chunk(chunk_q, chunk_r, size))
            .find(|&(q, r)| {
                let ground = core.generated_ground_at(q, r);
                ground.hydrology.depth_quanta == 0
                    && !ground.presentation.is_water()
                    && !core.terrain_blocks_movement(q, r)
            })
            .expect("the opening shelf contains dry ground");
        let started = Instant::now();
        let active = core
            .edit_water(
                q,
                r,
                hydrology::WaterAction::Flood,
                hydrology::WATER_COMMAND_LIMIT_QUANTA,
            )
            .unwrap();
        let active_micros = started.elapsed().as_micros();

        core.dirty.water = false;
        let before = core.water.clone();
        let quiet_started = Instant::now();
        core.tick_many(QUIET_TICKS);
        let quiet_micros = quiet_started.elapsed().as_micros();
        Report {
            seed: SEED,
            command_quanta: hydrology::WATER_COMMAND_LIMIT_QUANTA,
            active_cells: active.cells,
            sweeps: active.sweeps,
            transfers: active.transfers,
            frontier_quanta: active.outflow_quanta,
            settled: active.settled,
            active_micros,
            quiet_ticks: QUIET_TICKS,
            quiet_micros,
            quiet_water_dirty: core.dirty.water,
            quiet_state_changed: core.water != before,
        }
    }

    pub fn format(report: &Report) -> String {
        serde_json::to_string_pretty(report).expect("water benchmark serializes")
    }
}

/// Reproducible accelerated proof of the exact coarse geomorphic sequence production runs hourly.
#[cfg(not(target_arch = "wasm32"))]
pub mod erosion_bench {
    use super::*;
    use std::time::Instant;

    #[derive(Debug, Serialize)]
    pub struct Report {
        pub seed: u32,
        pub epoch_ticks: u64,
        pub chunk_budget: usize,
        pub cell_budget: usize,
        pub edge_budget: usize,
        pub change_budget: usize,
        pub surveyed_chunks: usize,
        pub accelerated_epochs: u32,
        pub chunks: usize,
        pub cells: usize,
        pub edges: usize,
        pub bends: usize,
        pub stressed_banks: usize,
        pub changes: usize,
        pub truncated: bool,
        pub elapsed_micros: u128,
        pub save_load_checksum_stable: bool,
    }

    pub fn run() -> Report {
        const SEED: u32 = 1_213_486_160;
        const MAX_ACCELERATED_EPOCHS: u32 = 512;
        let definitions: DefinitionsInput =
            serde_json::from_str(include_str!("../../src/data/definitions.json")).unwrap();
        let technologies: TechnologiesInput =
            serde_json::from_str(include_str!("../../src/data/technologies.json")).unwrap();
        let scenarios: ScenariosInput =
            serde_json::from_str(include_str!("../../src/data/scenarios.json")).unwrap();
        let scenario = scenarios
            .scenarios
            .iter()
            .find(|scenario| scenario.key == "new-game")
            .unwrap();
        let mut core = Core::new(&definitions, &technologies, scenario, Some(SEED), None).unwrap();
        // Measurement-only survey window: production still opens chunks only through player survey.
        for chunk_r in -5..=5 {
            for chunk_q in -5..=5 {
                core.generate_chunk(chunk_q, chunk_r);
            }
        }
        let started = Instant::now();
        let mut report = geomorphology::EpochReport::default();
        let mut epochs = 0;
        while epochs < MAX_ACCELERATED_EPOCHS && report.changes == 0 {
            report = core.run_geomorphic_epoch();
            epochs += 1;
        }
        let elapsed_micros = started.elapsed().as_micros();
        let saved = core.save_string().unwrap();
        let restored = Core::from_save(&definitions, &technologies, &scenarios, &saved).unwrap();
        Report {
            seed: SEED,
            epoch_ticks: geomorphology::EPOCH_TICKS,
            chunk_budget: geomorphology::CHUNK_BUDGET,
            cell_budget: geomorphology::CELL_BUDGET,
            edge_budget: geomorphology::EDGE_BUDGET,
            change_budget: geomorphology::CHANGE_BUDGET,
            surveyed_chunks: core.generated_chunks.len(),
            accelerated_epochs: epochs,
            chunks: report.chunks,
            cells: report.cells,
            edges: report.edges,
            bends: report.bends,
            stressed_banks: report.stressed_banks,
            changes: report.changes,
            truncated: report.truncated,
            elapsed_micros,
            save_load_checksum_stable: restored.checksum() == core.checksum(),
        }
    }

    pub fn format(report: &Report) -> String {
        serde_json::to_string_pretty(report).expect("erosion benchmark serializes")
    }
}

/// Deterministic headless capacity measurement.
///
/// The roadmap gates finer dirty tracking, any renderer decision, and every scale claim behind
/// measured tiers. This module builds synthetic steady-state factories from the shipped
/// definitions, drives them through the same entry points the worker uses, and reports per-phase
/// cost so capacity is measured instead of asserted.
///
/// The same measurement code runs natively and in the browser worker: only the clock differs, so
/// the two records are comparable by construction rather than by re-implementation. The wasm build
/// is behind the `bench` feature, so the deployed game artifact still never carries it.
#[cfg(any(not(target_arch = "wasm32"), feature = "bench"))]
pub mod capacity {
    use super::*;

    /// Monotonic microseconds. Only differences between readings are meaningful, and a platform's
    /// reading may be quantized — the browser clamps `performance.now` unless the page is
    /// cross-origin isolated — so every phase below times many samples at once.
    pub trait Clock {
        fn now_us(&self) -> f64;
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub struct SystemClock(std::time::Instant);

    #[cfg(not(target_arch = "wasm32"))]
    impl SystemClock {
        pub fn new() -> Self {
            Self(std::time::Instant::now())
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl Default for SystemClock {
        fn default() -> Self {
            Self::new()
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl Clock for SystemClock {
        fn now_us(&self) -> f64 {
            self.0.elapsed().as_secs_f64() * 1e6
        }
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = performance, js_name = now)]
        fn performance_now() -> f64;
    }

    /// `performance.now` in both a window and a worker global scope, converted to microseconds.
    #[cfg(target_arch = "wasm32")]
    pub struct PerformanceClock;

    #[cfg(target_arch = "wasm32")]
    impl Clock for PerformanceClock {
        fn now_us(&self) -> f64 {
            performance_now() * 1e3
        }
    }

    /// How long a phase must run before its mean is trusted.
    ///
    /// A native clock resolves nanoseconds, so a fixed sample count is enough and the budget is
    /// zero. A browser clamps `performance.now` to 100 µs unless the page is cross-origin
    /// isolated, which is coarser than most of the phases below; there, a phase repeats its sample
    /// block until it has run long enough for that step to be a rounding error. Only the sample
    /// count changes, never the workload, so both records stay per-unit comparable.
    #[derive(Clone, Copy, Debug)]
    pub struct Budget {
        pub min_phase_us: f64,
    }

    impl Budget {
        /// Run each phase exactly once through its sample block.
        pub const FIXED: Budget = Budget { min_phase_us: 0.0 };

        /// 20 ms, which holds a 100 µs clock step to 0.5% of a phase.
        pub const CLAMPED_CLOCK: Budget = Budget {
            min_phase_us: 20_000.0,
        };
    }

    /// Time one phase, repeating its sample block until the budget is met, and report the mean
    /// cost per sample together with the number of samples that produced it.
    fn phase(
        clock: &dyn Clock,
        budget: Budget,
        samples_per_block: u32,
        mut block: impl FnMut(),
    ) -> (f64, u32) {
        let start = clock.now_us();
        let mut samples = 0u32;
        loop {
            block();
            samples = samples.saturating_add(samples_per_block);
            let elapsed = (clock.now_us() - start).max(0.0);
            if elapsed >= budget.min_phase_us || samples_per_block == 0 {
                return (mean(elapsed, samples), samples);
            }
        }
    }

    pub fn default_clock() -> Box<dyn Clock> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Box::new(SystemClock::new())
        }
        #[cfg(target_arch = "wasm32")]
        {
            Box::new(PerformanceClock)
        }
    }

    const DEFINITIONS: &str = include_str!("../../src/data/definitions.json");
    const TECHNOLOGIES: &str = include_str!("../../src/data/technologies.json");

    const EXTRACTOR: DefinitionId = 1;
    const BELT: DefinitionId = 2;
    const COMPOSER: DefinitionId = 3;
    const CONTAINER: DefinitionId = 4;
    const CONSUMER: DefinitionId = 5;
    const COMPONENT_RECIPE: RecipeId = 1;
    const ORE: ItemId = 1;

    /// Report format version, so recorded JSON stays interpretable as the metric set changes.
    /// Version 2 adds `checksum_us`, which the sparse-snapshot release needed to see. Version 3
    /// adds `platform`, because the same ladder now runs natively and as wasm in a browser worker
    /// and a record must say which one it is. Version 4 adds `delta_json_bytes`, and changes what
    /// `delta_bytes` means: it is now the binary wire payload the game ships rather than the JSON
    /// one, so the two figures are not comparable across the boundary between schema 3 and 4.
    pub const REPORT_SCHEMA: u32 = 4;
    /// Lines sit three rows apart so one line's three-cell composer cannot touch the next.
    const ROW_PITCH: i32 = 3;
    /// How far east of its anchor each multi-cell machine in the workload reaches, so the line is
    /// spaced by the catalogue's own footprints rather than by a remembered one-cell world.
    const EXTRACTOR_CELLS: i32 = 2;
    const COMPOSER_CELLS: i32 = 2;
    /// The first belt of a line, and so the workload's rotate target. It is a belt rather than the
    /// extractor it sits beside, because rotating a source is a different edit to rotating a link.
    const EDIT_TARGET_Q: i32 = EXTRACTOR_CELLS;
    /// Large enough that no deposit empties inside a measured run, so every tier measures the same
    /// steady state rather than a decaying one.
    const DEPOSIT_QUANTITY: u32 = 1_000_000;
    /// Reach far past the generated blueprint so edit measurements are never range-rejected.
    const BUILD_RANGE_HEXES: u32 = 100_000;
    /// The bounded idle batch the host sends on a frame with no held key.
    const IDLE_COMMANDS: &str = "[{\"type\":\"move_intent\",\"x\":0,\"y\":0}]";
    /// Rotation restores a belt's original orientation every six edits.
    const ROTATION_CYCLE: u32 = 6;

    /// One measured tier: `lines` independent
    /// `extractor → belts → composer → belt → container → belt → consumer` production lines.
    ///
    /// Sample budgets shrink as tiers grow so a complete run stays interactive; per-unit results
    /// stay comparable because every metric is reported per tick, per frame, or per edit.
    #[derive(Clone, Copy, Debug)]
    pub struct TierSpec {
        pub key: &'static str,
        pub lines: u32,
        pub belt_span: u32,
        pub warmup_ticks: u32,
        pub measured_ticks: u32,
        pub frames: u32,
        pub snapshots: u32,
        pub edits: u32,
    }

    impl TierSpec {
        /// Entities per line: extractor, transport belts, composer, output belt, container,
        /// delivery belt, and consumer.
        pub fn entities_per_line(&self) -> u32 {
            self.belt_span + 6
        }

        pub fn entities(&self) -> u32 {
            self.lines * self.entities_per_line()
        }
    }

    /// Measured cost for one tier. Every field is a primitive so the report stays a stable,
    /// machine-readable record.
    #[derive(Clone, Debug, Serialize)]
    pub struct TierResult {
        pub key: String,
        pub lines: u32,
        pub entities: usize,
        pub tiles: usize,
        pub chunks: usize,
        /// Ticks actually timed. Equal to the tier's tick budget under `Budget::FIXED`, and a
        /// multiple of it when a coarse clock made the phase repeat.
        pub measured_ticks: u32,
        /// Mean cost of one simulation tick with no snapshot or serialization.
        pub tick_us: f64,
        pub ticks_per_second: f64,
        /// Mean cost of building one complete native snapshot, before serialization. The shipped
        /// frame no longer pays this — it is the host's first frame, and the baseline the
        /// incremental delta is measured against.
        pub snapshot_us: f64,
        /// Mean cost of one native checksum. Every delta carries one, so this is a floor under the
        /// frame that no amount of snapshot sparsity can remove.
        pub checksum_us: f64,
        /// Mean cost of one worker frame: bounded command batch, one tick, and a serialized delta.
        pub frame_us: f64,
        pub frames_per_second: f64,
        /// Mean encoded delta payload crossing the worker boundary per frame, in the binary wire
        /// format the game ships.
        pub delta_bytes: f64,
        /// What the same frames would have cost as JSON, which is what they did cost until the
        /// binary wire replaced it. Recorded beside `delta_bytes` so the encoding's saving is a
        /// measured ratio in the record rather than an inference from two different runs.
        pub delta_json_bytes: f64,
        /// Mean cost of one full deterministic transport compile.
        pub full_compile_us: f64,
        /// Mean cost of the incremental transport machinery alone, for the same edit: stable-ID
        /// link capture plus affected-component recompilation. Directly comparable to
        /// `full_compile_us`.
        pub incremental_recompile_us: f64,
        /// Mean cost of one complete public rotate edit, including legality checks. The difference
        /// from `incremental_recompile_us` is what the edit path spends outside transport.
        pub edit_us: f64,
        /// Native checksum after the measured tick phase, pinning the workload against drift.
        pub checksum: u32,
        pub delivered: u64,
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct Report {
        pub schema: u32,
        pub crate_version: String,
        pub profile: String,
        /// `native` or `wasm32`. The build reports it rather than the caller, so a record cannot
        /// claim a platform it was not measured on.
        pub platform: String,
        pub tiers: Vec<TierResult>,
    }

    fn platform() -> String {
        if cfg!(target_arch = "wasm32") {
            "wasm32".into()
        } else {
            "native".into()
        }
    }

    /// The recorded tier ladder. It spans one line to a blueprint far past anything the current
    /// game asks a player to build, so the measurement shows where cost stops being linear.
    pub fn default_tiers() -> Vec<TierSpec> {
        vec![
            tier("line", 1, 2000, 400, 400, 60),
            tier("small", 16, 1000, 200, 200, 60),
            tier("medium", 64, 400, 100, 100, 60),
            tier("wide", 128, 240, 60, 60, 60),
            tier("large", 256, 120, 40, 40, 30),
            tier("xlarge", 512, 60, 20, 20, 12),
        ]
    }

    /// A reduced ladder for smoke coverage inside the test gate.
    pub fn quick_tiers() -> Vec<TierSpec> {
        vec![tier("line", 1, 20, 5, 5, 6), tier("small", 16, 20, 5, 5, 6)]
    }

    fn tier(
        key: &'static str,
        lines: u32,
        measured_ticks: u32,
        frames: u32,
        snapshots: u32,
        edits: u32,
    ) -> TierSpec {
        TierSpec {
            key,
            lines,
            belt_span: 6,
            // Long enough for the first components to reach the consumer, so every tier is timed
            // with cargo actually moving. The belt run sets this floor now that a hex of belt is
            // 5.37 m of conveyor: eight belts at `BELT_TRANSIT_TICKS` is 216 ticks of travel on its
            // own, on top of the 30 ticks a tier-one extractor spends on one ore and the craft
            // between them. The measured window is unchanged, and so is what it measures — the line
            // is extraction-bound either way — but it now starts after a longer pipeline has
            // filled.
            warmup_ticks: 400,
            measured_ticks,
            frames,
            snapshots,
            edits,
        }
    }

    fn catalogs() -> (DefinitionsInput, TechnologiesInput) {
        let mut definitions: DefinitionsInput =
            serde_json::from_str(DEFINITIONS).expect("shipped definitions parse");
        // This synthetic transport workload keeps its historical two-ore/eight-tick recipe.
        // v0.33's gameplay component needs upstream smelting and gears; silently swapping that
        // into this isolated line would benchmark a stalled machine and invalidate old records.
        let recipe = definitions
            .recipes
            .iter_mut()
            .find(|recipe| recipe.id == COMPONENT_RECIPE)
            .unwrap();
        recipe.inputs = vec![Ingredient {
            item_id: ORE,
            quantity: 2,
        }];
        recipe.duration = 8;
        recipe.output = Ingredient {
            item_id: 2,
            quantity: 1,
        };
        recipe.fuel = 0;
        (
            definitions,
            serde_json::from_str(TECHNOLOGIES).expect("shipped technologies parse"),
        )
    }

    fn placed(
        q: i32,
        r: i32,
        definition_id: DefinitionId,
        recipe_id: Option<RecipeId>,
    ) -> PlacedBuilding {
        PlacedBuilding {
            q,
            r,
            definition_id,
            // Every line runs east, so compiled transport is a straight directed chain.
            orientation: 0,
            recipe_id,
            // Left unowned so the edit phase can exercise the ordinary player rotate path.
            scenario_owned: false,
        }
    }

    /// Build the synthetic scenario for a tier. It is an ordinary scenario definition, validated by
    /// the same rules as the shipped catalog.
    pub(crate) fn tier_scenario(spec: &TierSpec) -> ScenarioDefinition {
        let mut resources = Vec::new();
        let mut buildings = Vec::new();
        for line in 0..spec.lines {
            let r = line as i32 * ROW_PITCH;
            resources.push(ScenarioResource {
                q: 0,
                r,
                item_id: ORE,
                quantity: DEPOSIT_QUANTITY,
            });
            buildings.push(placed(0, r, EXTRACTOR, None));
            // Machines stand on more than their anchor now, so the line is laid out from each
            // one's eastern edge rather than from its anchor. The belt span, the building count
            // and the order of the chain are unchanged; only the empty ground between them moved.
            let belt_start = EXTRACTOR_CELLS;
            for q in belt_start..belt_start + spec.belt_span as i32 {
                buildings.push(placed(q, r, BELT, None));
            }
            let composer_q = belt_start + spec.belt_span as i32;
            buildings.push(placed(composer_q, r, COMPOSER, Some(COMPONENT_RECIPE)));
            let tail_q = composer_q + COMPOSER_CELLS;
            buildings.push(placed(tail_q, r, BELT, None));
            buildings.push(placed(tail_q + 1, r, CONTAINER, None));
            buildings.push(placed(tail_q + 2, r, BELT, None));
            buildings.push(placed(tail_q + 3, r, CONSUMER, None));
        }
        ScenarioDefinition {
            id: 1,
            key: format!("capacity-{}", spec.key),
            name: format!("Capacity tier {}", spec.key),
            description: "Synthetic steady-state capacity workload".into(),
            version: 1,
            seed: 2_071_003_907,
            // Generation is off below, so a preset would name a table nothing reads.
            world_preset: None,
            chunk_size: 8,
            // Terrain is uniform lowland so a tier measures transport and machines, not the
            // incidental obstacle layout of a generated seed.
            generated_environment: false,
            // Away from every line, so the idle player never blocks a footprint.
            player_spawn: Coordinate { q: -6, r: -6 },
            player_facing: 0,
            build_range: BUILD_RANGE_HEXES,
            // The workload's player never picks anything up, so this only has to be valid.
            carry_slots: 12,
            contract: ContractDefinition {
                key: "capacity".into(),
                name: "Capacity workload".into(),
                stages: vec![ContractStage {
                    key: "steady-state".into(),
                    name: "Run the line".into(),
                    brief: "A measured workload rather than a game.".into(),
                    reads: "nothing — the harness draws no hub".into(),
                    // Never reached, so a completed stage cannot change the measured workload
                    // partway through. The harness delivers into a consumer in any case, and a
                    // consumer is deliberately not the hub.
                    requirements: vec![Ingredient {
                        item_id: 2,
                        quantity: u32::MAX,
                    }],
                }],
            },
            initial_inventory: Vec::new(),
            initial_researched: vec![1, 2, 3, 4],
            resources,
            buildings,
        }
    }

    /// A warmed core for a tier, advanced far enough that cargo is already flowing.
    pub(crate) fn warm_core(spec: &TierSpec) -> Core {
        let (definitions, mut technologies) = catalogs();
        technologies.skills.clear();
        technologies.skill_milestones.clear();
        let scenario = tier_scenario(spec);
        validate_all(
            &definitions,
            &technologies,
            &ScenariosInput {
                version: 1,
                scenarios: vec![scenario.clone()],
            },
        )
        .expect("capacity scenario is valid");
        let mut core = Core::new(&definitions, &technologies, &scenario, None, None)
            .expect("capacity core builds");
        // The ladder measures transport, not the power constraint. Unmetered supply keeps
        // delivered totals and the tick path honest without adding a pole per line.
        core.power_unmetered = true;
        core.advance_ticks(spec.warmup_ticks);
        core
    }

    /// A warmed `Factory` for a tier, ready for the host to drive over the ordinary worker RPC.
    /// The browser harness measures its round trip through exactly this object, so the boundary
    /// cost is measured against the same steady state the in-wasm phases are.
    pub(crate) fn warm_factory(spec: &TierSpec) -> Factory {
        let (definitions, mut technologies) = catalogs();
        technologies.skills.clear();
        technologies.skill_milestones.clear();
        let scenario = tier_scenario(spec);
        Factory {
            definitions,
            technologies,
            scenarios: ScenariosInput {
                version: 1,
                scenarios: vec![scenario],
            },
            core: warm_core(spec),
            snapshot_revision: 0,
            baseline: None,
        }
    }

    pub fn measure_tier(spec: &TierSpec) -> TierResult {
        measure_tier_with(spec, default_clock().as_ref(), Budget::FIXED)
    }

    pub fn measure_tier_with(spec: &TierSpec, clock: &dyn Clock, budget: Budget) -> TierResult {
        let mut core = warm_core(spec);
        let entities = core.entities.len();
        let tiles = core.tiles.len();
        let chunks = core.generated_chunks.len();

        let (tick_us, measured_ticks) = phase(clock, budget, spec.measured_ticks, || {
            core.advance_ticks(spec.measured_ticks)
        });

        let (snapshot_us, _) = phase(clock, budget, spec.snapshots, || {
            for _ in 0..spec.snapshots {
                let snapshot = core.snapshot();
                std::hint::black_box(&snapshot);
            }
        });

        let (checksum_us, _) = phase(clock, budget, spec.snapshots, || {
            for _ in 0..spec.snapshots {
                std::hint::black_box(core.checksum());
            }
        });

        // Pinned on its own core, advanced exactly once through the tier's tick budget. A browser
        // run repeats the timed phase and therefore ends somewhere else entirely; taking the
        // workload's identity from here is what keeps its checksum comparable to a native record.
        let (checksum, delivered) = pinned_state(spec);

        let (frame_us, delta_bytes) = measure_frames(spec, clock, budget);
        let delta_json_bytes = measure_json_payload(spec);
        let full_compile_us = measure_full_compile(spec, clock, budget);
        let incremental_recompile_us = measure_recompiles(spec, clock, budget);
        let edit_us = measure_edits(spec, clock, budget);

        TierResult {
            key: spec.key.into(),
            lines: spec.lines,
            entities,
            tiles,
            chunks,
            measured_ticks,
            tick_us,
            ticks_per_second: rate(tick_us),
            snapshot_us,
            checksum_us,
            frame_us,
            frames_per_second: rate(frame_us),
            delta_bytes,
            delta_json_bytes,
            full_compile_us,
            incremental_recompile_us,
            edit_us,
            checksum,
            delivered,
        }
    }

    /// The tier's identity: the checksum and delivered total after exactly one tick budget from a
    /// warm core. Recorded rather than timed, so it cannot move with the sample count.
    fn pinned_state(spec: &TierSpec) -> (u32, u64) {
        let mut core = warm_core(spec);
        core.advance_ticks(spec.measured_ticks);
        (core.checksum(), core.delivered)
    }

    /// One worker frame, measured through the exact entry points the host RPC calls.
    fn measure_frames(spec: &TierSpec, clock: &dyn Clock, budget: Budget) -> (f64, f64) {
        let mut factory = warm_factory(spec);
        // The first delta is a complete snapshot; take it outside the measurement so the reported
        // payload is the steady-state per-frame cost.
        let _ = factory.snapshot_delta_bytes();
        let mut bytes = 0usize;
        let (frame_us, frames) = phase(clock, budget, spec.frames, || {
            for _ in 0..spec.frames {
                // No player steps: the capacity workload measures the factory, and the idle player
                // has no movement intent to spend them on anyway.
                if factory.advance_json(IDLE_COMMANDS, 1, 0).is_err() {
                    panic!("capacity frame commands must be accepted");
                }
                bytes += factory.snapshot_delta_bytes().len();
            }
        });
        (frame_us, mean(bytes as f64, frames))
    }

    /// The same frames' payload had they been encoded as JSON, which is what they were until the
    /// binary wire landed.
    ///
    /// A second factory rather than a second call, because building a delta consumes the dirty
    /// marks and advances the baseline, so one frame cannot be asked for both encodings. The
    /// workload is deterministic, so this run produces the identical sequence of deltas — and it is
    /// untimed, because what is wanted from it is the byte count the shipped encoding is measured
    /// against, not the cost of an encoding the game no longer performs.
    fn measure_json_payload(spec: &TierSpec) -> f64 {
        let mut factory = warm_factory(spec);
        let _ = factory.snapshot_delta_json();
        let mut bytes = 0usize;
        for _ in 0..spec.frames {
            if factory.advance_json(IDLE_COMMANDS, 1, 0).is_err() {
                panic!("capacity frame commands must be accepted");
            }
            bytes += factory.snapshot_delta_json().len();
        }
        mean(bytes as f64, spec.frames)
    }

    /// The full deterministic compile used on load and restore, as the incremental baseline.
    fn measure_full_compile(spec: &TierSpec, clock: &dyn Clock, budget: Budget) -> f64 {
        let mut core = warm_core(spec);
        let samples = spec.edits.max(1);
        phase(clock, budget, samples, || {
            for _ in 0..samples {
                core.compile_graph();
            }
        })
        .0
    }

    /// The complete public rotate path. Rotating a belt through all six orientations covers edits
    /// that merge and split neighbouring components, not only the cheap self-contained case.
    fn measure_edits(spec: &TierSpec, clock: &dyn Clock, budget: Budget) -> f64 {
        let mut core = warm_core(spec);
        let edits = rotation_edits(spec);
        if edits == 0 {
            return 0.0;
        }
        phase(clock, budget, edits, || {
            for edit in 0..edits {
                // Spread edits across lines so no single component stays warm in cache.
                core.rotate(EDIT_TARGET_Q, edit_row(spec, edit), false)
                    .expect("capacity belt rotates");
            }
        })
        .0
    }

    /// The incremental transport machinery alone, driving the same rotations. Isolating it from
    /// the edit path's legality checks is what makes the comparison against a full compile fair.
    fn measure_recompiles(spec: &TierSpec, clock: &dyn Clock, budget: Budget) -> f64 {
        let mut core = warm_core(spec);
        let edits = rotation_edits(spec);
        if edits == 0 {
            return 0.0;
        }
        // Entity lookup is part of the edit path, not the transport machinery, so resolve targets
        // before timing. No entity is added or removed here, so the indices stay valid.
        let targets: Vec<(usize, u32)> = (0..edits)
            .map(|edit| {
                let index = core
                    .entity_at(EDIT_TARGET_Q, edit_row(spec, edit))
                    .expect("capacity belt exists");
                (index, core.entities[index].id)
            })
            .collect();

        phase(clock, budget, edits, || {
            for &(index, id) in &targets {
                let old_links = core.graph_links_by_id();
                let old_footprint = core.entity_footprint(&core.entities[index]);
                let orientation = (core.entities[index].placed.orientation + 1) % 6;
                let next_footprint = core.footprint_for(core.entities[index].placed, orientation);
                core.entities[index].placed.orientation = orientation;
                let changed_cells = old_footprint
                    .into_iter()
                    .chain(next_footprint)
                    .map(|cell| (cell.q, cell.r))
                    .collect();
                core.recompile_graph_components(&old_links, &changed_cells, &BTreeSet::from([id]));
            }
        })
        .0
    }

    fn rotation_edits(spec: &TierSpec) -> u32 {
        spec.edits - (spec.edits % ROTATION_CYCLE)
    }

    fn edit_row(spec: &TierSpec, edit: u32) -> i32 {
        ((edit / ROTATION_CYCLE) % spec.lines) as i32 * ROW_PITCH
    }

    pub fn run(specs: &[TierSpec]) -> Report {
        run_with(specs, |_| {})
    }

    /// Run the ladder, reporting each tier as it completes so a long run shows progress.
    pub fn run_with(specs: &[TierSpec], mut observe: impl FnMut(&TierResult)) -> Report {
        let clock = default_clock();
        let mut ladder = Ladder::new(specs.to_vec());
        for index in 0..ladder.len() {
            let result = ladder
                .measure(index, clock.as_ref())
                .expect("ladder index is in range");
            observe(&result);
        }
        ladder.report()
    }

    /// The ladder as resumable state: one tier is measured per call, and the report is assembled
    /// from whatever has been measured so far.
    ///
    /// A native run has no reason to stop between tiers, but a browser one does — the harness
    /// reports each tier as it lands and yields to the event loop in between — so both drive the
    /// ladder through this one type instead of two loops that could drift apart.
    pub struct Ladder {
        specs: Vec<TierSpec>,
        tiers: Vec<TierResult>,
        budget: Budget,
    }

    impl Ladder {
        pub fn new(specs: Vec<TierSpec>) -> Self {
            Self {
                specs,
                tiers: Vec::new(),
                budget: Budget::FIXED,
            }
        }

        /// Give every phase a minimum duration, for a platform whose clock is too coarse to time
        /// a fixed sample block.
        pub fn with_budget(mut self, budget: Budget) -> Self {
            self.budget = budget;
            self
        }

        pub fn len(&self) -> usize {
            self.specs.len()
        }

        pub fn is_empty(&self) -> bool {
            self.specs.is_empty()
        }

        pub fn spec(&self, index: usize) -> Option<&TierSpec> {
            self.specs.get(index)
        }

        pub fn specs(&self) -> &[TierSpec] {
            &self.specs
        }

        /// Measure one tier and retain it for the report. Measuring the same index twice replaces
        /// the earlier result rather than recording the tier twice.
        pub fn measure(&mut self, index: usize, clock: &dyn Clock) -> Option<TierResult> {
            let spec = *self.specs.get(index)?;
            let result = measure_tier_with(&spec, clock, self.budget);
            match self.tiers.iter_mut().find(|tier| tier.key == spec.key) {
                Some(existing) => *existing = result.clone(),
                None => self.tiers.push(result.clone()),
            }
            Some(result)
        }

        pub fn report(&self) -> Report {
            Report {
                schema: REPORT_SCHEMA,
                crate_version: env!("CARGO_PKG_VERSION").into(),
                profile: if cfg!(debug_assertions) {
                    "debug".into()
                } else {
                    "release".into()
                },
                platform: platform(),
                tiers: self.tiers.clone(),
            }
        }
    }

    pub fn format_json(report: &Report) -> String {
        serde_json::to_string_pretty(report).expect("report is serializable")
    }

    pub fn table_header() -> String {
        format!(
            "{:<8}{:>7}{:>10}{:>11}{:>10}{:>12}{:>12}{:>11}{:>10}{:>13}{:>13}{:>12}{:>13}{:>10}",
            "tier",
            "lines",
            "entities",
            "tick us",
            "ticks/s",
            "snapshot us",
            "checksum us",
            "frame us",
            "frames/s",
            "delta bytes",
            "json bytes",
            "compile us",
            "recompile us",
            "edit us",
        )
    }

    pub fn table_row(tier: &TierResult) -> String {
        format!(
            "{:<8}{:>7}{:>10}{:>11.1}{:>10.0}{:>12.1}{:>12.1}{:>11.1}{:>10.0}{:>13.0}{:>13.0}{:>12.1}{:>13.1}{:>10.1}",
            tier.key,
            tier.lines,
            tier.entities,
            tier.tick_us,
            tier.ticks_per_second,
            tier.snapshot_us,
            tier.checksum_us,
            tier.frame_us,
            tier.frames_per_second,
            tier.delta_bytes,
            tier.delta_json_bytes,
            tier.full_compile_us,
            tier.incremental_recompile_us,
            tier.edit_us,
        )
    }

    pub fn format_table(report: &Report) -> String {
        let mut lines = vec![
            format!(
                "HexFactory capacity tiers — factory-wasm {} ({} {} profile)",
                report.crate_version, report.platform, report.profile
            ),
            table_header(),
        ];
        lines.extend(report.tiers.iter().map(table_row));
        lines.join("\n")
    }

    fn mean(total: f64, samples: u32) -> f64 {
        if samples == 0 {
            0.0
        } else {
            total / f64::from(samples)
        }
    }

    fn rate(microseconds: f64) -> f64 {
        if microseconds <= 0.0 {
            0.0
        } else {
            1e6 / microseconds
        }
    }

    /// The browser entry point for the same ladder, built only by `--features bench`.
    ///
    /// The harness drives one tier per call so the page can report progress, and can hand back a
    /// warmed `Factory` for the tier so the host can measure what the game actually pays per
    /// frame: the worker RPC round trip around these same native phases.
    #[cfg(all(target_arch = "wasm32", feature = "bench"))]
    #[wasm_bindgen]
    pub struct CapacityBench {
        ladder: Ladder,
        clock: PerformanceClock,
    }

    #[cfg(all(target_arch = "wasm32", feature = "bench"))]
    #[wasm_bindgen]
    impl CapacityBench {
        #[wasm_bindgen(constructor)]
        pub fn new(quick: bool) -> CapacityBench {
            CapacityBench {
                ladder: Ladder::new(if quick {
                    quick_tiers()
                } else {
                    default_tiers()
                })
                .with_budget(Budget::CLAMPED_CLOCK),
                clock: PerformanceClock,
            }
        }

        pub fn tier_count(&self) -> usize {
            self.ladder.len()
        }

        /// `{ key, lines, entities }` for every tier, so the page can list the run before it
        /// starts instead of discovering its shape as results arrive.
        pub fn tiers_json(&self) -> String {
            let tiers: Vec<serde_json::Value> = self
                .ladder
                .specs()
                .iter()
                .map(|spec| {
                    serde_json::json!({
                        "key": spec.key,
                        "lines": spec.lines,
                        "entities": spec.entities(),
                        // The host times its round trip over the same frame budget the in-wasm
                        // frame phase uses, so the two costs describe the same amount of work.
                        "frames": spec.frames,
                    })
                })
                .collect();
            serde_json::Value::Array(tiers).to_string()
        }

        /// Measure one tier, returning its `TierResult` as JSON.
        pub fn measure(&mut self, index: usize) -> Result<String, JsValue> {
            let result = self
                .ladder
                .measure(index, &self.clock)
                .ok_or_else(|| js_error(format!("no capacity tier at index {index}")))?;
            serde_json::to_string(&result).map_err(|error| js_error(error.to_string()))
        }

        /// A warmed factory for the tier, in the same steady state the in-wasm phases measure.
        pub fn factory(&self, index: usize) -> Result<Factory, JsValue> {
            let spec = self
                .ladder
                .spec(index)
                .ok_or_else(|| js_error(format!("no capacity tier at index {index}")))?;
            Ok(warm_factory(spec))
        }

        pub fn report_json(&self) -> String {
            format_json(&self.ladder.report())
        }
    }
}

/// What a parameter set actually generates, counted rather than estimated.
///
/// Value noise is not uniformly distributed, so **a threshold is not a proportion**: nothing about
/// `water_level: 26_000` says what share of a world is water, and a preset that claimed one from
/// arithmetic would be guessing. This module samples a disc of hexes for a parameter set and
/// reports the band histogram, the field density per material, how far the landing site is from
/// each of them, and the shape of the water — the same measured-before-claimed rule the frame
/// budget and the capacity ladder already live under, applied to the generator.
///
/// Measurement code, like the capacity ladder: native only, never compiled into the wasm artifact,
/// and never a dependency of the game or the production build. The acceptance tests use it because
/// the claims they check are claims about proportions, which is exactly what it counts.
#[cfg(not(target_arch = "wasm32"))]
pub mod survey {
    use super::*;

    /// The shipped catalogue, for material names. The survey reports what a player would read.
    const DEFINITIONS: &str = include_str!("../../src/data/definitions.json");

    /// How far out a survey samples by default. This is the *opening*: bootstrap windows, purity,
    /// and patch statistics all live inside a few dozen hexes of the hub. Landscape claims —
    /// oceans, ranges, how long a biome takes to walk — need a radius of a couple of landform
    /// cells, which is what `landscape_radius` returns; this number stays small so the gate does
    /// not walk a million hexes.
    pub const DEFAULT_RADIUS: i32 = 96;

    /// A radius that can actually see a landform of this cell size, capped so a 960-cell ocean
    /// preset does not walk eleven million hexes on every `npm run survey`.
    pub fn landscape_radius(coarse_cell: i32) -> i32 {
        DEFAULT_RADIUS.max((coarse_cell * 3) / 2).min(768)
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct BandCount {
        /// The band's name as a label. A survey is a report, not a wire contract, so this is the
        /// readable spelling rather than the enum the snapshot travels as.
        pub band: String,
        pub hexes: u32,
        /// Share of the sampled disc, in parts per thousand.
        pub per_mille: u32,
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct MaterialCount {
        pub item_id: ItemId,
        pub name: String,
        pub cells: u32,
        /// Cells per thousand land hexes. Land, not total, because water holds nothing and a
        /// wetter preset would otherwise look poorer than it plays.
        pub per_mille_land: u32,
        /// Axial distance from the landing site to the nearest generated cell of this material,
        /// and the mean over every cell in the sample. `None` means the sample found none, which
        /// is the failure this tool exists to make visible.
        pub nearest: Option<u32>,
        pub mean_distance: Option<u32>,
    }

    /// Connected runs of one material, which is what an extractor is actually offered and what the
    /// survey has never reported. Totals, densities, and distances all look healthy for a world of
    /// scattered single cells, so a generator that mixes two materials under one extractor disc can
    /// pass every figure this tool printed before. `purity` is the number Landforms and Fields
    /// v0.21 is for; the rest say whether a patch is worth walking to.
    #[derive(Clone, Debug, Serialize)]
    pub struct PatchCount {
        pub item_id: ItemId,
        pub name: String,
        /// Connected runs of this material inside the sample.
        pub patches: u32,
        /// Hexes the fill visited. This must equal the material's `cells`, and it is carried
        /// rather than inferred so a test can say so — a flood fill that loses or double-counts a
        /// hex would otherwise quietly move every mean below it.
        pub hexes: u32,
        /// Hexes per patch, and the largest single patch, both in hexes.
        pub mean_patch: u32,
        pub largest_patch: u32,
        /// Total units in a patch, averaged over patches. Size alone understates a rich small
        /// deposit and overstates a wide thin one, and yield is what the extractor draws down.
        pub mean_patch_yield: u32,
        /// Axial distance from the landing site to the nearest patch of at least
        /// `WORKABLE_PATCH_HEXES`, which is a different and more useful number than `nearest`: a
        /// lone cell two hexes away is not a deposit an extractor can be stood on.
        pub nearest_workable_patch: Option<u32>,
        /// Share of this material's hexes whose radius-1 disc holds exactly one material, in parts
        /// per thousand. An extractor on a mixed hex covers both and cleanly works neither.
        pub purity_per_mille: u32,
        /// Patches touching the edge of the sample, on the same reasoning as `truncated_bodies`: a
        /// patch the sample cuts off is a floor, not a measurement.
        pub truncated_patches: u32,
        /// The size of the water body nearest each patch, averaged over patches. This is what
        /// verifies the beach proxy: a sand rule asks the coarse elevation octave alone whether a
        /// centre stands against ocean, and the generator may not flood-fill to check. The survey
        /// can, so a small number here means the proxy is wrong. `None` means the sample holds no
        /// water at all.
        pub mean_nearest_body: Option<u32>,
    }

    /// The running totals a patch flood fill accumulates, before names and means are attached.
    #[derive(Clone, Copy, Debug, Default)]
    struct PatchTotals {
        patches: u32,
        hexes: u32,
        yield_total: u64,
        largest_patch: u32,
        nearest_workable_patch: Option<u32>,
        pure_hexes: u32,
        truncated_patches: u32,
        nearest_body_total: u64,
        nearest_body_patches: u32,
    }

    /// Ponds or oceans, counted. This is the measurement the milestone's central claim rests on:
    /// sea level decides how *much* water there is, and feature scale decides how *big* it is, so
    /// the two are told apart by body size at a fixed `water_level`.
    ///
    /// Rivers are **not** counted here. They read as `ShallowWater` like everything else and are
    /// common and linear, so folding them in would quietly stop `largest_body` from meaning ocean.
    #[derive(Clone, Debug, Serialize)]
    pub struct WaterShape {
        pub water_hexes: u32,
        pub bodies: u32,
        pub largest_body: u32,
        pub mean_body: u32,
        /// Bodies reaching the edge of the sample, whose true size the sample cannot see. A
        /// largest-body figure carrying these is a floor, not a measurement.
        pub truncated_bodies: u32,
    }

    /// Inland water that is a line rather than a basin, reported on its own for the reason above.
    /// Shallow water stops being an accident of sea level once rivers exist and becomes common and
    /// linear, which is what makes a bridge a necessity rather than an ornament.
    #[derive(Clone, Debug, Serialize)]
    pub struct RiverShape {
        pub river_hexes: u32,
        /// Connected runs of river, and the mean length of one in hexes.
        pub runs: u32,
        pub mean_run: u32,
        pub longest_run: u32,
    }

    /// One guaranteed material of the opening, as the generator actually placed it. The bootstrap
    /// pass is a promise rather than geography, so it is reported here instead of being folded
    /// into the counts — the same split the clearing already lives under.
    #[derive(Clone, Debug, Serialize)]
    pub struct BootstrapRow {
        pub item_id: ItemId,
        pub name: String,
        /// Distance from the landing site to the nearest hex of the guaranteed patch, and how many
        /// hexes that patch holds once its member test has clipped it. `None` means the pass gave
        /// up, which is the failure the survey exists to make visible.
        pub edge: Option<u32>,
        pub hexes: u32,
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct WorldSurvey {
        pub preset: String,
        pub seed: u32,
        pub radius: i32,
        pub hexes: u32,
        pub land_hexes: u32,
        pub bands: Vec<BandCount>,
        pub materials: Vec<MaterialCount>,
        pub patches: Vec<PatchCount>,
        /// Share of every generated resource hex, of any material, whose radius-1 disc holds
        /// exactly one material. This is the single figure v0.21 is measured against, and it is
        /// reported over the whole sample rather than per material because an extractor does not
        /// care which two materials it straddles.
        pub purity_per_mille: u32,
        pub water: WaterShape,
        pub rivers: RiverShape,
        pub bootstrap: Vec<BootstrapRow>,
    }

    /// Survey a shipped preset by key.
    pub fn survey_preset(key: &str, seed: u32, radius: i32) -> Result<WorldSurvey, String> {
        survey_overridden(key, &[], seed, radius)
    }

    /// Survey a preset with named scalar parameters replaced. The milestone's whole point is that
    /// a world is a parameter set rather than a preset, so the tool that measures one has to be
    /// able to measure a set nobody shipped — which is how a preset's numbers get chosen.
    pub fn survey_overridden(
        key: &str,
        overrides: &[(String, i32)],
        seed: u32,
        radius: i32,
    ) -> Result<WorldSurvey, String> {
        let mut params = preset_params(key).ok_or_else(|| format!("unknown world preset {key}"))?;
        let mut label = key.to_string();
        for (name, value) in overrides {
            let slot: &mut i32 = match name.as_str() {
                "elevation_coarse_cell" => &mut params.elevation_coarse_cell,
                "elevation_fine_cell" => &mut params.elevation_fine_cell,
                "elevation_coarse_weight" => &mut params.elevation_coarse_weight,
                "moisture_cell" => &mut params.moisture_cell,
                "richness_cell" => &mut params.richness_cell,
                "water_level" => &mut params.water_level,
                "shore_level" => &mut params.shore_level,
                "hills_level" => &mut params.hills_level,
                "highland_level" => &mut params.highland_level,
                "cliff_step" => &mut params.cliff_step,
                "deep_water_moisture" => &mut params.deep_water_moisture,
                "site_cell" => &mut params.site_cell,
                "site_jitter" => &mut params.site_jitter,
                "river_cell" => &mut params.river_cell,
                "river_width" => &mut params.river_width,
                "river_max_elevation" => &mut params.river_max_elevation,
                "ocean_level" => &mut params.ocean_level,
                other => return Err(format!("unknown world parameter {other}")),
            };
            *slot = *value;
            label.push_str(&format!(" {name}={value}"));
        }
        Ok(run(&label, &params, seed, radius))
    }

    pub fn preset_keys() -> Vec<String> {
        world_presets()
            .into_iter()
            .map(|preset| preset.key.to_string())
            .collect()
    }

    /// The shipped landform cell of a preset, so the survey binary can size its disc without
    /// generating anything.
    pub fn preset_coarse_cell(key: &str) -> Option<i32> {
        preset_params(key).map(|params| params.elevation_coarse_cell)
    }

    /// The default seed of the shipped `new-game` scenario, so a survey and a played world are
    /// talking about the same landscape unless the caller says otherwise.
    pub fn default_seed() -> u32 {
        1_213_486_160
    }

    pub(crate) fn run(label: &str, params: &WorldParams, seed: u32, radius: i32) -> WorldSurvey {
        let definitions: DefinitionsInput =
            serde_json::from_str(DEFINITIONS).expect("shipped definitions parse");
        // The survey and a played world share one evaluator, so a surveyed world and a played one
        // cannot disagree about either the lattice or the opening.
        let spine = GroundSpine::physical(params, seed, true);
        let fields = WorldFields::new(params, seed, &spine);
        let cells: Vec<(i32, i32)> = disc(radius);
        let mut bands: BTreeMap<Terrain, u32> = BTreeMap::new();
        let mut terrain_of: BTreeMap<(i32, i32), Terrain> = BTreeMap::new();
        let mut river_cells: BTreeSet<(i32, i32)> = BTreeSet::new();
        let mut land_hexes = 0u32;
        let mut found: BTreeMap<ItemId, (u32, u32, u32)> = BTreeMap::new();
        let mut field_of: BTreeMap<(i32, i32), (ItemId, u32)> = BTreeMap::new();
        for &(q, r) in &cells {
            let terrain = spine.presentation_at(q, r);
            terrain_of.insert((q, r), terrain);
            *bands.entry(terrain).or_default() += 1;
            if !terrain.is_water() {
                land_hexes += 1;
            } else if is_survey_river(params, seed, q, r) {
                river_cells.insert((q, r));
            }
            if let Some((item_id, quantity)) = surveyed_field(&fields, &spine, q, r) {
                field_of.insert((q, r), (item_id, quantity));
                let distance = axial_distance((0, 0), (q, r)) as u32;
                let entry = found.entry(item_id).or_insert((0, u32::MAX, 0));
                entry.0 += 1;
                entry.1 = entry.1.min(distance);
                entry.2 += distance;
            }
        }
        let hexes = cells.len() as u32;
        let bands = bands
            .into_iter()
            .map(|(terrain, count)| BandCount {
                band: format!("{terrain:?}"),
                hexes: count,
                per_mille: per_mille(count, hexes),
            })
            .collect();
        let (water, body_of) = water_shape(&terrain_of, &river_cells, radius);
        let (totals, pure_hexes) = patch_shape(&fields, &spine, &field_of, &body_of, radius);
        let name_of = |item_id: ItemId| {
            definitions
                .items
                .iter()
                .find(|item| item.id == item_id)
                .map(|item| item.name.clone())
                .unwrap_or_else(|| format!("item {item_id}"))
        };
        // Every generated item, whether or not this parameter set produced any — a material the
        // table names and the world does not hold is the row a reader most needs to see.
        let mut materials = Vec::new();
        let mut patches = Vec::new();
        for &item_id in &[
            IRON_ORE, CRYSTAL, COPPER_ORE, COAL, STONE, SAND, CLAY, WOOD, LIMESTONE, CRUDE_OIL,
        ] {
            let name = name_of(item_id);
            let totals = totals.get(&item_id).copied().unwrap_or_default();
            patches.push(PatchCount {
                item_id,
                name: name.clone(),
                patches: totals.patches,
                hexes: totals.hexes,
                mean_patch: if totals.patches == 0 {
                    0
                } else {
                    totals.hexes / totals.patches
                },
                largest_patch: totals.largest_patch,
                mean_patch_yield: if totals.patches == 0 {
                    0
                } else {
                    (totals.yield_total / u64::from(totals.patches)) as u32
                },
                nearest_workable_patch: totals.nearest_workable_patch,
                purity_per_mille: per_mille(totals.pure_hexes, totals.hexes),
                truncated_patches: totals.truncated_patches,
                mean_nearest_body: (totals.nearest_body_patches > 0).then(|| {
                    (totals.nearest_body_total / u64::from(totals.nearest_body_patches)) as u32
                }),
            });
            let stats = found.get(&item_id).copied();
            materials.push(MaterialCount {
                item_id,
                name,
                cells: stats.map(|(count, _, _)| count).unwrap_or(0),
                per_mille_land: per_mille(
                    stats.map(|(count, _, _)| count).unwrap_or(0),
                    land_hexes,
                ),
                nearest: stats.map(|(_, nearest, _)| nearest),
                mean_distance: stats.map(|(count, _, total)| total / count.max(1)),
            });
        }
        WorldSurvey {
            preset: label.to_string(),
            seed,
            radius,
            hexes,
            land_hexes,
            bands,
            materials,
            patches,
            purity_per_mille: per_mille(pure_hexes, field_of.len() as u32),
            water,
            rivers: river_shape(&river_cells),
            bootstrap: bootstrap_rows(&fields, &spine, &name_of),
        }
    }

    /// What the survey counts as a generated cell. The clearing is a promise, not geography, so it
    /// is no evidence about what a parameter set generates — `field_at` already suppresses it, and
    /// the guaranteed opening is reported on its own in `bootstrap`.
    fn surveyed_field(
        fields: &WorldFields,
        spine: &GroundSpine,
        q: i32,
        r: i32,
    ) -> Option<(ItemId, u32)> {
        fields
            .field_at(q, r, true, spine)
            .map(|field| (field.item_id, field.quantity))
    }

    /// A river hex, told apart from sea and lake by the test that made it one. Both read as
    /// `ShallowWater`, and the whole point of reporting them apart is that a linear inland water
    /// and an ocean are different facts about a world.
    fn is_survey_river(params: &WorldParams, seed: u32, q: i32, r: i32) -> bool {
        if axial_distance((0, 0), (q, r)) <= LANDING_CLEAR_RADIUS {
            return false;
        }
        let elevation = elevation_at(params, seed, q, r);
        elevation >= params.shore_level && is_river(params, seed, q, r, elevation)
    }

    /// Connected runs of river, filled over the same six directions everything else here uses.
    fn river_shape(river_cells: &BTreeSet<(i32, i32)>) -> RiverShape {
        let mut unvisited = river_cells.clone();
        let river_hexes = unvisited.len() as u32;
        let mut runs = Vec::new();
        while let Some(&start) = unvisited.iter().next() {
            unvisited.remove(&start);
            let mut stack = vec![start];
            let mut length = 0u32;
            while let Some((q, r)) = stack.pop() {
                length += 1;
                for (dq, dr) in DIRECTIONS {
                    if unvisited.remove(&(q + dq, r + dr)) {
                        stack.push((q + dq, r + dr));
                    }
                }
            }
            runs.push(length);
        }
        RiverShape {
            river_hexes,
            runs: runs.len() as u32,
            mean_run: if runs.is_empty() {
                0
            } else {
                river_hexes / runs.len() as u32
            },
            longest_run: runs.into_iter().max().unwrap_or(0),
        }
    }

    /// The opening the generator promised, measured rather than assumed: how far the player walks
    /// to each guaranteed patch, and how much of it survived the member clipping.
    fn bootstrap_rows(
        fields: &WorldFields,
        spine: &GroundSpine,
        name_of: &dyn Fn(ItemId) -> String,
    ) -> Vec<BootstrapRow> {
        let placed: BTreeMap<ItemId, (u32, u32)> = fields
            .guarantees(spine)
            .into_iter()
            .map(|(item_id, walk, hexes)| (item_id, (walk, hexes)))
            .collect();
        BOOTSTRAP_GUARANTEES
            .iter()
            .map(|&(item_id, _, _)| BootstrapRow {
                item_id,
                name: name_of(item_id),
                edge: placed.get(&item_id).map(|&(walk, _)| walk),
                hexes: placed.get(&item_id).map_or(0, |&(_, hexes)| hexes),
            })
            .collect()
    }

    /// Patches, flood filled over the six adjacency directions exactly as `water_shape` fills
    /// bodies, plus the purity count. Returns the per-material totals and the number of resource
    /// hexes of any material that stand in a single-material disc.
    ///
    /// The fill stays inside the sample — a patch reaching the edge is counted as truncated rather
    /// than followed out of the disc — but purity reads `surveyed_field` directly, so a hex on the
    /// rim is judged against its real neighbours rather than against a sample boundary.
    fn patch_shape(
        fields: &WorldFields,
        spine: &GroundSpine,
        field_of: &BTreeMap<(i32, i32), (ItemId, u32)>,
        body_of: &BTreeMap<(i32, i32), u32>,
        radius: i32,
    ) -> (BTreeMap<ItemId, PatchTotals>, u32) {
        let nearest_body = nearest_body_size(body_of, radius);
        let mut totals: BTreeMap<ItemId, PatchTotals> = BTreeMap::new();
        let mut unvisited: BTreeSet<(i32, i32)> = field_of.keys().copied().collect();
        while let Some(&start) = unvisited.iter().next() {
            let item_id = field_of[&start].0;
            unvisited.remove(&start);
            let mut stack = vec![start];
            let mut hexes = 0u32;
            let mut yield_total = 0u64;
            let mut nearest = u32::MAX;
            let mut touches_edge = false;
            // The body nearest the patch is the body nearest whichever of its hexes is closest to
            // one, which is what the multi-source walk below already answers per hex.
            let mut body: Option<(u32, u32)> = None;
            while let Some((q, r)) = stack.pop() {
                hexes += 1;
                yield_total += u64::from(field_of[&(q, r)].1);
                let distance = axial_distance((0, 0), (q, r));
                nearest = nearest.min(distance as u32);
                if distance >= radius {
                    touches_edge = true;
                }
                if let Some(&(reach, size)) = nearest_body.get(&(q, r)) {
                    if body.is_none_or(|(best, _)| reach < best) {
                        body = Some((reach, size));
                    }
                }
                for (dq, dr) in DIRECTIONS {
                    let next = (q + dq, r + dr);
                    // The material test comes first: a neighbour of another material must stay
                    // unvisited so its own patch is still found.
                    if field_of
                        .get(&next)
                        .is_some_and(|&(other, _)| other == item_id)
                        && unvisited.remove(&next)
                    {
                        stack.push(next);
                    }
                }
            }
            let entry = totals.entry(item_id).or_default();
            entry.patches += 1;
            entry.hexes += hexes;
            entry.yield_total += yield_total;
            entry.largest_patch = entry.largest_patch.max(hexes);
            if touches_edge {
                entry.truncated_patches += 1;
            }
            if let Some((_, size)) = body {
                entry.nearest_body_total += u64::from(size);
                entry.nearest_body_patches += 1;
            }
            if hexes >= WORKABLE_PATCH_HEXES {
                entry.nearest_workable_patch = Some(
                    entry
                        .nearest_workable_patch
                        .map_or(nearest, |best| best.min(nearest)),
                );
            }
        }

        let mut pure_hexes = 0u32;
        for (&(q, r), &(item_id, _)) in field_of {
            let mixed = DIRECTIONS.iter().any(|&(dq, dr)| {
                surveyed_field(fields, spine, q + dq, r + dr)
                    .is_some_and(|(other, _)| other != item_id)
            });
            if !mixed {
                pure_hexes += 1;
                totals.entry(item_id).or_default().pure_hexes += 1;
            }
        }
        (totals, pure_hexes)
    }

    /// For every hex in the sample, how far the nearest water body is and how big that body is.
    ///
    /// One multi-source walk out from every body hex at once, rather than a scan per patch: the
    /// per-patch form is patches × water hexes and this is hexes × six.
    fn nearest_body_size(
        body_of: &BTreeMap<(i32, i32), u32>,
        radius: i32,
    ) -> BTreeMap<(i32, i32), (u32, u32)> {
        let mut reached: BTreeMap<(i32, i32), (u32, u32)> = body_of
            .iter()
            .map(|(&cell, &size)| (cell, (0, size)))
            .collect();
        let mut frontier: Vec<(i32, i32)> = reached.keys().copied().collect();
        let mut distance = 0u32;
        while !frontier.is_empty() {
            distance += 1;
            let mut next = Vec::new();
            for (q, r) in frontier {
                let size = reached[&(q, r)].1;
                for (dq, dr) in DIRECTIONS {
                    let cell = (q + dq, r + dr);
                    if axial_distance((0, 0), cell) > radius || reached.contains_key(&cell) {
                        continue;
                    }
                    reached.insert(cell, (distance, size));
                    next.push(cell);
                }
            }
            frontier = next;
        }
        reached
    }

    /// Connected water bodies inside the sample, by flood fill over the six adjacency directions.
    /// Returns the shape and, per body hex, the size of the body it belongs to — which is what
    /// verifies the beach proxy the generator is not allowed to measure for itself.
    ///
    /// River hexes are excluded. They read as `ShallowWater` like a lake does, and folding a
    /// continent-spanning line into the body fill would join every basin it touches into one and
    /// call the result an ocean.
    fn water_shape(
        terrain_of: &BTreeMap<(i32, i32), Terrain>,
        river_cells: &BTreeSet<(i32, i32)>,
        radius: i32,
    ) -> (WaterShape, BTreeMap<(i32, i32), u32>) {
        let mut unvisited: BTreeSet<(i32, i32)> = terrain_of
            .iter()
            .filter(|(cell, terrain)| terrain.is_water() && !river_cells.contains(cell))
            .map(|(&cell, _)| cell)
            .collect();
        let water_hexes = unvisited.len() as u32;
        let mut sizes = Vec::new();
        let mut truncated = 0u32;
        let mut body_of: BTreeMap<(i32, i32), u32> = BTreeMap::new();
        while let Some(&start) = unvisited.iter().next() {
            unvisited.remove(&start);
            let mut stack = vec![start];
            let mut members = Vec::new();
            let mut touches_edge = false;
            while let Some((q, r)) = stack.pop() {
                members.push((q, r));
                if axial_distance((0, 0), (q, r)) >= radius {
                    touches_edge = true;
                }
                for (dq, dr) in DIRECTIONS {
                    let next = (q + dq, r + dr);
                    if unvisited.remove(&next) {
                        stack.push(next);
                    }
                }
            }
            if touches_edge {
                truncated += 1;
            }
            let size = members.len() as u32;
            for cell in members {
                body_of.insert(cell, size);
            }
            sizes.push(size);
        }
        let bodies = sizes.len() as u32;
        (
            WaterShape {
                water_hexes,
                bodies,
                largest_body: sizes.iter().copied().max().unwrap_or(0),
                mean_body: if bodies == 0 { 0 } else { water_hexes / bodies },
                truncated_bodies: truncated,
            },
            body_of,
        )
    }

    fn disc(radius: i32) -> Vec<(i32, i32)> {
        let mut cells = Vec::new();
        for q in -radius..=radius {
            for r in -radius..=radius {
                if axial_distance((0, 0), (q, r)) <= radius {
                    cells.push((q, r));
                }
            }
        }
        cells
    }

    fn per_mille(count: u32, total: u32) -> u32 {
        if total == 0 {
            0
        } else {
            (u64::from(count) * 1000 / u64::from(total)) as u32
        }
    }

    pub fn format_json(survey: &WorldSurvey) -> String {
        serde_json::to_string_pretty(survey).expect("survey serializes")
    }

    /// The human-readable form the notes are written from.
    pub fn format_report(survey: &WorldSurvey) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "preset {} | seed {} | radius {} | {} hexes ({} land)\n",
            survey.preset, survey.seed, survey.radius, survey.hexes, survey.land_hexes
        ));
        out.push_str("  band            hexes    per mille\n");
        for band in &survey.bands {
            out.push_str(&format!(
                "  {:<14} {:>7}   {:>6}\n",
                band.band, band.hexes, band.per_mille
            ));
        }
        out.push_str("  material        cells  per mille land   nearest    mean\n");
        for material in &survey.materials {
            let show = |value: Option<u32>| {
                value.map_or_else(|| "  none".to_string(), |value| format!("{value:>6}"))
            };
            out.push_str(&format!(
                "  {:<14} {:>6}   {:>13}   {}  {}\n",
                material.name,
                material.cells,
                material.per_mille_land,
                show(material.nearest),
                show(material.mean_distance)
            ));
        }
        out.push_str(
            "  material       patches    mean   largest   mean yield   workable   purity   cut   \
             near body\n",
        );
        for patch in &survey.patches {
            let show = |value: Option<u32>| {
                value.map_or_else(|| "  none".to_string(), |value| format!("{value:>6}"))
            };
            out.push_str(&format!(
                "  {:<14} {:>7}  {:>6}   {:>7}   {:>10}     {}   {:>6}  {:>4}   {}\n",
                patch.name,
                patch.patches,
                patch.mean_patch,
                patch.largest_patch,
                patch.mean_patch_yield,
                show(patch.nearest_workable_patch),
                patch.purity_per_mille,
                patch.truncated_patches,
                show(patch.mean_nearest_body)
            ));
        }
        out.push_str(&format!(
            "  purity: {} per mille of resource hexes stand in a single-material disc\n",
            survey.purity_per_mille
        ));
        out.push_str("  guaranteed     walk   hexes\n");
        for row in &survey.bootstrap {
            out.push_str(&format!(
                "  {:<14} {}  {:>6}\n",
                row.name,
                row.edge
                    .map_or_else(|| "  none".to_string(), |value| format!("{value:>6}")),
                row.hexes
            ));
        }
        out.push_str(&format!(
            "  water: {} hexes in {} bodies | largest {} | mean {} | {} reach the sample edge\n",
            survey.water.water_hexes,
            survey.water.bodies,
            survey.water.largest_body,
            survey.water.mean_body,
            survey.water.truncated_bodies
        ));
        out.push_str(&format!(
            "  rivers: {} hexes in {} runs | mean {} | longest {}\n",
            survey.rivers.river_hexes,
            survey.rivers.runs,
            survey.rivers.mean_run,
            survey.rivers.longest_run
        ));
        out
    }
}

#[cfg(test)]
mod tests;
