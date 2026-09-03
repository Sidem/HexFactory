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
///
/// Version 43 carries no new saved field either. Three skill branches became ladders, a mobility
/// rank joined them and the milestones funding them are worth more; every owned skill is keyed by
/// a stable id, so a version-42 file owns what it owned and the new ranks are simply unbought. The
/// world moved under it as well — see [`WORLD_GENERATOR_VERSION`] — which is the version-40 case
/// again: the stamp advances so the file reaches the generator check rather than a format error.
const SAVE_VERSION: u16 = 43;
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
///
/// Bumped to 14 for Water Runs Downhill. A channel now carries a hydraulic grade line — the water
/// surface it stands at — and the bed is cut to that elevation instead of to a constant depth
/// under whatever noise the cell happened to have. Routing is a priority flood over the node
/// lattice rather than a minimum spanning tree, so a reach descends by construction. Every bed and
/// every water surface moved; a version-13 envelope is a different landscape.
///
/// Bumped to 15 for variable ground hardness. Erodibility was one constant, which is why every
/// valley in version 14 had the same cross-section: rock strength is now a layered field of place
/// and depth, so a reach that meets rock harder than its discharge can cut hangs its bed on a sill,
/// pools behind it and falls past it, and a bank climbs at the grade the rock in it holds. Every
/// bed, bank and water surface in the world moved; a version-14 envelope is a different landscape.
const WORLD_GENERATOR_VERSION: u16 = 15;
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
/// The most levels a skill catalogue may add to travel speed.
///
/// Each level is a 5/4 multiplier. A step is a jump, not a sweep: nothing is tested between where
/// the player was and where they land. [`WALK_ARRIVE_RADIUS`] is what keeps a waypoint from being
/// cleared, and a step that grew past it would let an autonomous walk skip its own waypoint and
/// circle back for it forever. At level three a full-intent step is 537 world units against that
/// radius's 768, so the margin the arrival test was written with survives the whole ladder.
const MAX_MOVE_SPEED_LEVEL: u32 = 3;
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
/// 25 m/s over a 5.373 m hex. The host sends 800 for the ordinary walk (20 m/s) and 1000 while
/// Shift is held. Paced by `PLAYER_TICKS_PER_SECOND`, not by the simulation tick, so both gaits
/// keep one speed at every simulation speed. Shallow water ignores the gait and is 5 m/s —
/// `PLAYER_SPEED / 5`.
///
/// The number did not move when the ground did. A hex became 25 m² rather than 1 m², and holding
/// the speed in metres would have made every journey in the game five times longer in the hand —
/// a biome crossing measured in quarter-hours, a river detour that costs a minute. Distance is
/// what the rescale was for; travel time is not. So the player covers hexes at the rate they
/// always did, and the honest reading of that is a vehicle, not a walk: this is 20 m/s on foot
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
/// A swimmer crosses deep water at one eighth of dry-ground speed. The route pays the exact same
/// factor, so an autonomous walk only swims where that is genuinely the fastest available way.
const WALK_SWIM_COST: u32 = 8 * WALK_STEP_COST;
const SWIM_SPEED_DIVISOR: i32 = 8;
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
/// The unmodified walking intent. A player clicks a distant hex without holding Shift, so the
/// standing order uses the same 1x pace as the movement keys. Being native's own number rather than one
/// the host sends also means the host has no say in how fast a checksummed walk crosses the world:
/// the click names *where*, and the simulation decides *how*.
const AUTO_WALK_INTENT: i16 = 800;
/// Player-clock steps a walk may make no ground before it is abandoned. One second: long enough to
/// ride out a step spent sliding along a wall, short enough that a player boxed in by a building
/// placed across the route gets their controls back rather than jogging into it forever.
const WALK_STALL_STEPS: u32 = 30;
/// How close to a waypoint's centre counts as standing on it, in world units.
///
/// The hex's inradius, which is the largest circle wholly inside it. Being inside it means
/// `world_to_axial` of that position is that hex, which is what lets arrival be told apart from a
/// route that ran out for any other reason. It also has to be comfortably larger than one step — at
/// most `PLAYER_SPEED` raised by [`MAX_MOVE_SPEED_LEVEL`], 537 — or a waypoint could be jumped clean
/// over and the walk would circle it.
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
