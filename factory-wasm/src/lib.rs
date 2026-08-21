use serde::{Deserialize, Serialize};
use std::cell::RefCell;
#[cfg(test)]
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use wasm_bindgen::prelude::*;

/// The binary encoding the snapshot delta crosses the worker boundary in.
mod wire;

/// Derived economy figures: what the shipped numbers actually say the curve is.
///
/// Measurement code like the capacity ladder and the survey, and native only for the same reason:
/// nothing here runs a tick, and the wasm artifact the game ships must not carry it.
#[cfg(not(target_arch = "wasm32"))]
pub mod balance;

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
const SAVE_VERSION: u16 = 11;
/// Bumped to 6 for World Parameters. `WorldParams` is now part of a run's identity — it is in the
/// save envelope and in the checksum — so a version-5 envelope carries no answer to the question
/// "which world is this" and is rejected rather than assumed to be the default.
///
/// Bumped to 7 for Landforms and Fields. A deposit is a **site** now rather than a per-hex
/// decision, rivers cut inland water, and the guaranteed opening is placed by the generator instead
/// of by a hardcoded list of eight cells inside the clearing. Every one of those changes what a
/// seed generates, so a version-6 envelope describes a landscape this build cannot reproduce and is
/// rejected rather than reinterpreted. The named-save catalog shows the row rather than hiding it.
const WORLD_GENERATOR_VERSION: u16 = 8;
const MAX_COMMANDS_PER_BATCH: usize = 8;
/// A drag is one bounded command, so the run it expands into has to be bounded too. This is the
/// native cap on cells a single `place_line` or `erase_line` may touch.
const MAX_LINE_CELLS: usize = 32;
/// How many constructions back one session can be taken. Derived state, so it costs nothing saved.
const MAX_UNDO_DEPTH: usize = 64;
const GRAPH_TRACE_LIMIT: i32 = 8;
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
/// A hexagon's area is **1 m²**. Neighbour centres are `HEX_X` apart, which is √(2/√3) ≈ 1.075 m,
/// and that is the metre the walk and the run are paced against.
const HEX_RADIUS: i32 = 1024;
/// How many hex steps a *hand* gather reaches. Also the reach of any extractor whose definition
/// names no `extract_radius` of its own, so the base extractor is unchanged by tiers existing.
const EXTRACT_RADIUS: i32 = 1;
/// The largest reach a definition may claim. Reach is the flagship upgrade, so it is data — but
/// `deposit_candidates` walks the whole disc, and a definition file is not allowed to make that
/// walk unbounded.
const MAX_EXTRACT_RADIUS: u32 = 4;
/// Hexes around the hub forced to lowland so the landing is always a buildable clearing.
const LANDING_CLEAR_RADIUS: i32 = 7;
/// World units the player covers per player step at full intent (1000). That is the **run**:
/// 5 m/s, with a hex of 1 m². The host sends 600 for the ordinary walk (3 m/s) and 1000 while
/// Shift is held. Paced by `PLAYER_TICKS_PER_SECOND`, not by the simulation tick, so both gaits
/// keep one speed at every simulation speed. Shallow water ignores the gait and is 1 m/s —
/// `PLAYER_SPEED / 5`.
const PLAYER_SPEED: i32 = 275;
/// The player's own cadence, in steps per real second. Walking used to run inside the simulation
/// tick, which made it stop when the factory paused and crawl at a low speed multiplier. It is
/// still integer, still native, and still deterministic — a given step count always produces the
/// same position — it is simply no longer measured in factory time.
const PLAYER_TICKS_PER_SECOND: u32 = 30;
const PLAYER_RADIUS: i32 = 580;
const BUILDING_RADIUS: i32 = 690;
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
const HUB_RANGE: i32 = 1900;
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
    /// What the first fill pays. Priced against the raw gathers underneath the item — see the
    /// `requests` section of `fixtures/balance.json`, which reports exactly that ratio.
    insight: u32,
    /// What every later fill pays. Absent means later fills keep `insight`. Raw rows set this so
    /// the first survey funds the early tree and grinding the same row does not.
    #[serde(default)]
    repeat_insight: Option<u32>,
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
    /// Player-clock steps between hand gathers of this item. Absent means the hand cannot take it
    /// at all: water is pumped, signal crystal is extracted. Fifteen is wood, and no material is
    /// faster — that is the restated invariant `fixtures/balance.json` pins.
    #[serde(default)]
    hand_gather_steps: Option<u32>,
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
    construction_cost: Vec<Ingredient>,
    #[serde(default)]
    unlock_technology_id: Option<TechnologyId>,
    placement_rule: PlacementRule,
    buildable: bool,
    blocks_movement: bool,
    #[serde(default = "default_footprint")]
    footprint: Vec<Coordinate>,
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
/// exactly the orientations it had. `Corner` is the six vertex headings — the riser, and anything
/// later that spans the two-row period.
///
/// Two axes rather than one free range, because the split is also the price. A riser covers
/// `3 · size` of world distance against `√3 · size` for a unit step; letting a belt take a
/// corner heading would make a riser strictly dominant at a belt's cost. Separate axes mean
/// separate definitions, and separate definitions mean separate `construction_cost` rows.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum OrientationAxis {
    #[default]
    Edge,
    Corner,
}

impl OrientationAxis {
    /// The half-open range of orientation indices this axis allows.
    fn range(self) -> std::ops::Range<u8> {
        match self {
            Self::Edge => 0..NORTH,
            Self::Corner => NORTH..TRANSPORT_DIRECTIONS.len() as u8,
        }
    }

    fn allows(self, orientation: u8) -> bool {
        self.range().contains(&orientation)
    }

    /// The next orientation one `rotate` along. Rotation stays inside the axis, so edge and corner
    /// definitions each walk six headings in clockwise order.
    fn next(self, orientation: u8) -> u8 {
        let range = self.range();
        let span = range.end - range.start;
        let offset = orientation.wrapping_sub(range.start);
        range.start + (offset.wrapping_add(1) % span)
    }
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
    technologies: Vec<TechnologyDefinition>,
}

#[derive(Clone, Deserialize)]
struct TechnologyDefinition {
    id: TechnologyId,
    key: String,
    name: String,
    description: String,
    prerequisites: Vec<TechnologyId>,
    cost: u32,
    unlocks: Vec<DefinitionId>,
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
        // Shallows are a ford, not a wall: the player can wade them at 1 m/s. Construction still
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
    action_cooldown: u32,
    build_range: u32,
    /// Slots the player can carry, from the scenario. Like `build_range` it is a fixed scenario
    /// property rather than a simulation result, so it is validated against the scenario on load
    /// instead of being hashed into the checksum.
    carry_slots: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct Entity {
    id: u32,
    placed: PlacedBuilding,
    kind: BuildingKind,
    cargo: Option<Cargo>,
    inventory: BTreeMap<ItemId, u32>,
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
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct Snapshot {
    scenario: String,
    scenario_name: String,
    world_version: u16,
    seed: u32,
    tick: u64,
    checksum: u32,
    delivered: u64,
    delivered_by_item: Vec<Ingredient64>,
    insight: u64,
    victory: bool,
    contract: ContractSnapshot,
    requests: Vec<RequestSnapshot>,
    player: PlayerSnapshot,
    researched: Vec<TechnologyId>,
    chunks: Vec<ChunkSnapshot>,
    terrain: Vec<TileSnapshot>,
    resources: Vec<ResourceSnapshot>,
    buildings: Vec<EntitySnapshot>,
    events: Vec<String>,
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

/// One posted request as the hub is holding it: which row, and how much of it has arrived.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct RequestState {
    request_id: RequestId,
    delivered: u32,
}

/// One line of the request board as the host sees it. Everything needed to draw the row travels
/// with it — the price above all, because a price the player has to discover by delivering is the
/// defect this whole system exists to remove.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct RequestSnapshot {
    key: String,
    name: String,
    brief: String,
    item_id: ItemId,
    delivered: u32,
    required: u32,
    insight: u32,
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

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct TileSnapshot {
    q: i32,
    r: i32,
    x: i32,
    y: i32,
    radius: u32,
    terrain: Terrain,
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
    inventory: Vec<Ingredient>,
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
    footprint: Vec<Coordinate>,
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
    /// Stable entity ids whose snapshot may differ, including newly placed ones.
    entities: Vec<u32>,
    /// Stable entity ids the host must drop.
    removed: BTreeSet<u32>,
    /// Tile keys of deposits whose quantity may differ.
    resources: Vec<(i32, i32)>,
    /// Set when generation may have added deposits, so the resources group is resent whole and the
    /// host's ordering stays exactly the native one.
    resources_replace: bool,
    /// Set when generation adds tiles. Terrain only ever grows, so a flag is exact.
    terrain: bool,
    /// Set when the generated chunk set or any chunk's entity count may differ.
    chunks: bool,
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
    base_revision: u64,
    revision: u64,
    tick: u64,
    checksum: u32,
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
    chunks: Option<Vec<ChunkSnapshot>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    terrain: Option<Vec<TileSnapshot>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resources: Option<ResourcesDelta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    buildings: Option<BuildingsDelta>,
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
            chunks: Some(current.chunks.clone()),
            terrain: Some(current.terrain.clone()),
            resources: Some(ResourcesDelta {
                replace: true,
                changed: current.resources.clone(),
            }),
            buildings: Some(BuildingsDelta {
                replace: true,
                changed: current.buildings.clone(),
                removed: Vec::new(),
            }),
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
            chunks: changed(&previous.chunks, &current.chunks),
            terrain: changed(&previous.terrain, &current.terrain),
            resources: resources_delta(&previous.resources, &current.resources),
            buildings: buildings_delta(&previous.buildings, &current.buildings),
            events: changed(&previous.events, &current.events),
        }
    }
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
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum InputCommand {
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
    },
    /// Put stock into a container by hand — the mirror of `Withdraw`, on the same contract.
    Store {
        q: i32,
        r: i32,
        item_id: ItemId,
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
    Research {
        technology_id: TechnologyId,
    },
    /// Pass on one posted request, so the hub asks for something else in that slot.
    SkipRequest {
        slot: usize,
    },
}

struct Core {
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
    graph: Vec<Option<usize>>,
    /// Per-entity power network id (`None` = not on a network). Derived like `graph`.
    power_of: Vec<Option<u32>>,
    /// Last tick's supply and demand per network id.
    power_supply: BTreeMap<u32, u32>,
    power_demand: BTreeMap<u32, u32>,
    /// Capacity harness only: consumers run at full speed so the ladder still measures transport.
    power_unmetered: bool,
    player: PlayerState,
    researched: BTreeSet<TechnologyId>,
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
    /// goes behind unseen content; it must not burn the first-fill bonus, so fills are counted
    /// apart. Saved and checksummed: what a later fill pays depends on it.
    request_fills: BTreeMap<RequestId, u32>,
    produced: BTreeMap<ItemId, u64>,
    /// What the current (or last) action cooldown was worth when it started. Snapshot-only: the
    /// host draws remaining against this, and a save mid-gather republishes the remaining count so
    /// the ring starts full of what is left. Never saved, hashed, or checksummed.
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
        let fields = WorldFields::new(&world_params, seed);
        // A world whose opening cannot be placed is refused here rather than papered over. It is
        // the one generator failure a validator cannot see — `validate` is asked before a seed
        // exists — and shipping it would mean a run that cannot reach its own first extractor.
        if scenario.generated_environment {
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
            graph: Vec::new(),
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
                action_cooldown: 0,
                build_range: scenario.build_range.saturating_mul(HEX_X as u32),
                carry_slots: scenario.carry_slots,
            },
            researched: scenario.initial_researched.iter().copied().collect(),
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
            produced: BTreeMap::new(),
            last_action_cooldown_total: 0,
            events: vec![format!("{} ready", scenario.name)],
            dirty: SnapshotDirty::default(),
            undo_stack: Vec::new(),
        };
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
                reserved_inputs: BTreeMap::new(),
                progress: 0,
                fuel_charge: 0,
                power_charge: 0,
                burn_progress: 0,
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

    fn stack_size(&self, item: ItemId) -> u32 {
        self.item_definition(item)
            .map(|definition| definition.stack_size)
            .unwrap_or(1)
            .max(1)
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
        let mut prospective = self.player.inventory.clone();
        add_inventory(&mut prospective, additions);
        self.slots_used(&prospective) <= self.player.carry_slots
    }

    /// How many more of one item the player can take. A part-filled stack absorbs its remainder for
    /// free; past that, each free slot is worth a whole stack.
    fn player_room_for(&self, item_id: ItemId) -> u32 {
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

    fn footprint_for(&self, placed: PlacedBuilding, orientation: u8) -> Vec<Coordinate> {
        self.building_definition(placed.definition_id)
            .map(|definition| {
                definition
                    .footprint
                    .iter()
                    .map(|offset| {
                        // No definition needs a multi-cell corner-heading footprint yet, and the
                        // validator keeps that axis single-cell. A single `(0, 0)` cell is
                        // invariant under rotation, so leaving it unrotated is exact.
                        let offset = match orientation {
                            NORTH.. => *offset,
                            turns => rotate_coordinate(*offset, turns),
                        };
                        Coordinate {
                            q: placed.q + offset.q,
                            r: placed.r + offset.r,
                        }
                    })
                    .collect()
            })
            .unwrap_or_else(|| {
                vec![Coordinate {
                    q: placed.q,
                    r: placed.r,
                }]
            })
    }

    fn entity_footprint(&self, entity: &Entity) -> Vec<Coordinate> {
        self.footprint_for(entity.placed, entity.placed.orientation)
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
        // Supports sit below transport, so the most recently placed entity is the one a click,
        // erase, rotate, or copy action reaches first on a shared bridge hex.
        self.entities.iter().rposition(|entity| {
            self.entity_footprint(entity)
                .iter()
                .any(|cell| cell.q == q && cell.r == r)
        })
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
                (resource.quantity < resource.initial_quantity
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

    /// Whether open water sits inside the caller's data-defined reach. Terrain is a pure function
    /// of the seed, so this needs no generated tile and works at the frontier.
    fn water_within_reach(&self, q: i32, r: i32, radius: i32) -> bool {
        hexes_in_radius((q, r), radius)
            .into_iter()
            .any(|(cell_q, cell_r)| self.terrain_at(cell_q, cell_r).is_water())
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
            .find(|&key| self.deposit_quantity(key) > 0)
    }

    /// What one full cycle of this entity costs in ticks — a source's cadence, a composer's recipe
    /// duration, and zero for everything that does not run a cycle at all. Published as
    /// `progress_total` so the host draws a proportion it was given, and asked again by `upgrade`,
    /// because a tier may change the cadence under a part-finished job.
    fn progress_total(&self, index: usize) -> u32 {
        let entity = &self.entities[index];
        match entity.kind {
            BuildingKind::Extractor | BuildingKind::Pump => self
                .building_definition(entity.placed.definition_id)
                .and_then(|definition| definition.cadence)
                .unwrap_or(1),
            BuildingKind::Composer => entity
                .placed
                .recipe_id
                .and_then(|id| self.recipe(id))
                .map(|recipe| recipe.duration)
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

    fn generate_chunk(&mut self, chunk_q: i32, chunk_r: i32) {
        if !self.generated_chunks.insert((chunk_q, chunk_r)) {
            return;
        }
        // New tiles can cover an existing extractor, so every resolved deposit reference is stale —
        // and so is every extractor status derived from one. The two must be invalidated together:
        // dropping the entity marks would make snapshot correctness depend on generated deposits
        // never reaching an existing extractor, which nothing here enforces. Generation is rare, and
        // marks that turn out to change nothing are filtered against the baseline before they ship.
        self.deposit_links.clear();
        self.mark_all_entities_dirty();
        self.dirty.chunks = true;
        let size = self.scenario.chunk_size;
        for local_r in 0..size {
            for local_q in 0..size {
                let q = chunk_q * size + local_q;
                let r = chunk_r * size + local_r;
                let terrain = self.terrain_at(q, r);
                // A chunk of plain lowland adds nothing to either group, so both marks are
                // narrowed to a cell that actually appears in one. Generation is the only path
                // that adds to either: resending resources whole keeps the host's order exactly
                // the native one, so later patches can address field cells in place.
                self.dirty.terrain |= terrain != Terrain::Lowland;
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
        terrain_at(
            &self.world_params,
            self.seed,
            q,
            r,
            self.scenario.generated_environment,
        )
    }

    fn field_at(&self, q: i32, r: i32) -> Option<ResourceState> {
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
        self.fields
            .field_at(q, r, self.scenario.generated_environment)
    }

    fn ensure_tile(&mut self, q: i32, r: i32) {
        let size = self.scenario.chunk_size;
        self.generate_chunk(floor_div(q, size), floor_div(r, size));
    }

    fn ensure_neighborhood(&mut self, x: i32, y: i32) {
        let size = self.scenario.chunk_size;
        let (q, r) = world_to_axial(x, y);
        let center = (floor_div(q, size), floor_div(r, size));
        self.generate_chunk(center.0, center.1);
        for (dq, dr) in DIRECTIONS {
            self.generate_chunk(center.0 + dq, center.1 + dr);
        }
    }

    fn compile_graph(&mut self) {
        let occupied = self.occupied_entities();
        self.graph = self
            .entities
            .iter()
            .enumerate()
            .map(|(index, _)| self.compile_graph_target(index, &occupied))
            .collect();
        self.compile_power();
        // A full compile can move any entity's outgoing link, and `next_id` is part of its snapshot.
        self.mark_all_entities_dirty();
    }

    fn occupied_entities(&self) -> BTreeMap<(i32, i32), usize> {
        let mut occupied = BTreeMap::new();
        for (index, entity) in self.entities.iter().enumerate() {
            for cell in self.entity_footprint(entity) {
                occupied.insert((cell.q, cell.r), index);
            }
        }
        occupied
    }

    fn compile_graph_target(
        &self,
        index: usize,
        occupied: &BTreeMap<(i32, i32), usize>,
    ) -> Option<usize> {
        let entity = &self.entities[index];
        // Routing, so twelve. The loop below is unchanged and always was a ray-cast: it steps
        // `(dq, dr)` up to `GRAPH_TRACE_LIMIT`, skipping its own footprint, and returns the first
        // other occupied cell. Nothing in it ever assumed the step was a unit vector, which is why
        // the six corner headings cost table rows here and nothing else.
        let (dq, dr) = TRANSPORT_DIRECTIONS
            [usize::from(entity.placed.orientation) % TRANSPORT_DIRECTIONS.len()];
        let mut q = entity.placed.q + dq;
        let mut r = entity.placed.r + dr;
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

    fn compile_power(&mut self) {
        let n = self.entities.len();
        self.power_of = vec![None; n];
        self.power_supply.clear();
        self.power_demand.clear();
        if n == 0 {
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
        for index in 0..self.entities.len() {
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
            for index in 0..self.entities.len() {
                if self.power_of.get(index).copied().flatten().is_some() {
                    self.dirty.entities.push(self.entities[index].id);
                }
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
        match entity.kind {
            // A blocked extractor or pump has produced something nobody has taken. It is not
            // waiting on power and must not hold a share of it.
            BuildingKind::Extractor | BuildingKind::Pump => entity.cargo.is_none(),
            BuildingKind::Composer => {
                if entity.cargo.is_some() {
                    return false;
                }
                let Some(recipe) = entity.placed.recipe_id.and_then(|id| self.recipe(id)) else {
                    return false;
                };
                // Mid-craft always wants power: the inputs are already spent and the only thing
                // between the machine and its output is time it has to be paid for.
                if entity.progress > 0 {
                    return true;
                }
                let stocked = recipe.inputs.iter().all(|ingredient| {
                    entity
                        .inventory
                        .get(&ingredient.item_id)
                        .copied()
                        .unwrap_or(0)
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
        for index in 0..self.entities.len() {
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
                        .inventory
                        .get(&WATER_ITEM)
                        .copied()
                        .unwrap_or(0)
                        .min(units);
                    if water > 0 {
                        subtract_item(&mut self.entities[boiler].inventory, WATER_ITEM, water);
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
        entity.fuel_charge > 0 || self.burnable_item(&entity.inventory, &[]).is_some()
    }

    fn boiler_live(&self, index: usize) -> bool {
        let entity = &self.entities[index];
        entity.inventory.get(&WATER_ITEM).copied().unwrap_or(0) >= 1
            && (entity.fuel_charge > 0 || self.burnable_item(&entity.inventory, &[]).is_some())
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

    fn graph_links_by_id(&self) -> BTreeMap<u32, Option<u32>> {
        self.entities
            .iter()
            .enumerate()
            .map(|(index, entity)| {
                (
                    entity.id,
                    self.graph[index].map(|target| self.entities[target].id),
                )
            })
            .collect()
    }

    fn recompile_graph_components(
        &mut self,
        old_links: &BTreeMap<u32, Option<u32>>,
        changed_cells: &BTreeSet<(i32, i32)>,
        edited_ids: &BTreeSet<u32>,
    ) -> usize {
        // Erasing shifts vector indices, so preserve unaffected edges through stable entity IDs.
        let occupied = self.occupied_entities();
        let indices_by_id: BTreeMap<u32, usize> = self
            .entities
            .iter()
            .enumerate()
            .map(|(index, entity)| (entity.id, index))
            .collect();
        let anchors: BTreeMap<(i32, i32), u32> = self
            .entities
            .iter()
            .map(|entity| ((entity.placed.q, entity.placed.r), entity.id))
            .collect();

        let mut graph: Vec<Option<usize>> = self
            .entities
            .iter()
            .map(|entity| {
                old_links
                    .get(&entity.id)
                    .copied()
                    .flatten()
                    .and_then(|target| indices_by_id.get(&target).copied())
            })
            .collect();

        let mut old_adjacency: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
        for (&source, &target) in old_links {
            old_adjacency.entry(source).or_default();
            if let Some(target) = target {
                old_adjacency.entry(source).or_default().insert(target);
                old_adjacency.entry(target).or_default().insert(source);
            }
        }

        let mut affected = edited_ids.clone();
        // An edit can change the edited entity's own output or an output ray that crosses any cell
        // in its old/new footprint. The trace bound matches the full compiler's footprint walk.
        for &(q, r) in changed_cells {
            if let Some(&index) = occupied.get(&(q, r)) {
                affected.insert(self.entities[index].id);
            }
            for (dq, dr) in DIRECTIONS {
                for distance in 1..=GRAPH_TRACE_LIMIT {
                    if let Some(&source) = anchors.get(&(q - dq * distance, r - dr * distance)) {
                        affected.insert(source);
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
                let target = self.compile_graph_target(index, &occupied);
                graph[index] = target;
                if let Some(target) = target {
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
            self.advance_machines();
            self.transfer_cargo();
            self.tick += 1;
            self.regrow_flora();
        }
    }

    /// Walk the player on its own cadence. Movement deliberately no longer rides the simulation
    /// tick: a paused factory should not pin the player in place, and a 0.25× factory should not
    /// make walking feel broken. Frame-coupled movement stays refused — the host sends a step
    /// count, not a delta — so the same command sequence still reproduces the same position and the
    /// same checksum.
    /// The player's clock. It runs on elapsed real time rather than factory time, so everything
    /// the player does themselves — walking, and the cooldown between one action and the next —
    /// keeps the same pace whether the factory is paused, slowed, or running flat out.
    fn advance_player_steps(&mut self, count: u32) {
        for _ in 0..count {
            self.player.action_cooldown = self.player.action_cooldown.saturating_sub(1);
            self.advance_player();
        }
    }

    fn advance_machines(&mut self) {
        let mut order: Vec<usize> = (0..self.entities.len()).collect();
        order.sort_by_key(|&index| self.entities[index].id);
        for index in order {
            match self.entities[index].kind {
                BuildingKind::Extractor => self.advance_extractor(index),
                BuildingKind::Composer => self.advance_composer(index),
                BuildingKind::Pump => self.advance_pump(index),
                _ => {}
            }
        }
    }

    fn advance_extractor(&mut self, index: usize) {
        if self.entities[index].cargo.is_some() {
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
        let cadence = self
            .building_definition(definition_id)
            .and_then(|definition| definition.cadence)
            .unwrap_or(1);
        self.entities[index].progress += add;
        if self.entities[index].progress < cadence {
            return;
        }
        let resource_key = resource_key.expect("available resource key exists");
        let field = self
            .field_at(resource_key.0, resource_key.1)
            .expect("available resource exists");
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
        self.entities[index].cargo = Some(Cargo {
            item_id,
            quantity: 1,
        });
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

    /// A pump draws from the basin it stands beside. Water is the one source in the game that is
    /// not finite: there is no overlay entry to write down and nothing to deplete, so a pump is an
    /// extractor without the deposit rather than a special case of one.
    fn advance_pump(&mut self, index: usize) {
        if self.entities[index].cargo.is_some() {
            return;
        }
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
        let radius = definition
            .and_then(|value| value.extract_radius)
            .unwrap_or(PUMP_RADIUS as u32) as i32;
        if !self.water_within_reach(q, r, radius) {
            self.entities[index].progress = 0;
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
        self.entities[index].cargo = Some(Cargo {
            item_id,
            quantity: 1,
        });
        self.entities[index].progress = 0;
        *self.produced.entry(item_id).or_default() += 1;
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
            let Some(item_id) = self.burnable_item(&self.entities[index].inventory, inputs) else {
                return false;
            };
            let value = self.fuel_value(item_id);
            subtract_item(&mut self.entities[index].inventory, item_id, 1);
            self.entities[index].fuel_charge += value;
            // Burning is a visible change even on a tick the craft does not start, because the
            // machine banks the charge and its stock went down.
            let id = self.entities[index].id;
            self.dirty.entities.push(id);
        }
        true
    }

    fn advance_composer(&mut self, index: usize) {
        let Some(recipe_id) = self.entities[index].placed.recipe_id else {
            return;
        };
        let Some(recipe) = self.recipe(recipe_id).cloned() else {
            return;
        };
        if self.entities[index].cargo.is_some() {
            return;
        }
        if self.entities[index].progress > 0 {
            let id = self.entities[index].id;
            self.dirty.entities.push(id);
            self.entities[index].progress += self.power_progress(index, 1);
            if self.entities[index].progress >= recipe.duration {
                self.entities[index].cargo = Some(Cargo {
                    item_id: recipe.output.item_id,
                    quantity: recipe.output.quantity,
                });
                self.entities[index].progress = 0;
                self.entities[index].reserved_inputs.clear();
            }
            return;
        }
        let can_start = recipe.inputs.iter().all(|ingredient| {
            self.entities[index]
                .inventory
                .get(&ingredient.item_id)
                .copied()
                .unwrap_or(0)
                >= ingredient.quantity
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
                subtract_item(
                    &mut self.entities[index].inventory,
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

    fn transfer_cargo(&mut self) {
        let mut proposals: Vec<(u32, usize, usize, Cargo, bool)> = self
            .entities
            .iter()
            .enumerate()
            .filter_map(|(source, entity)| {
                let target = self.graph[source]?;
                let (cargo, from_inventory) = if entity.kind == BuildingKind::Container {
                    let (&item_id, _) = entity.inventory.iter().find(|(_, value)| **value > 0)?;
                    (
                        Cargo {
                            item_id,
                            quantity: 1,
                        },
                        true,
                    )
                } else {
                    (entity.cargo?, false)
                };
                Some((entity.id, source, target, cargo, from_inventory))
            })
            .collect();
        proposals.sort_by_key(|proposal| proposal.0);
        let mut claimed = BTreeSet::new();
        for (_, source, target, cargo, from_inventory) in proposals {
            if claimed.contains(&target) || !self.can_accept(target, cargo) {
                continue;
            }
            if from_inventory {
                subtract_item(
                    &mut self.entities[source].inventory,
                    cargo.item_id,
                    cargo.quantity,
                );
            } else {
                self.entities[source].cargo = None;
            }
            let (source_id, target_id) = (self.entities[source].id, self.entities[target].id);
            self.dirty.entities.push(source_id);
            self.dirty.entities.push(target_id);
            self.accept(target, cargo);
            claimed.insert(target);
        }
    }

    fn can_accept(&self, target: usize, cargo: Cargo) -> bool {
        let entity = &self.entities[target];
        match entity.kind {
            BuildingKind::Belt => entity.cargo.is_none(),
            BuildingKind::Composer => {
                let Some(recipe_id) = entity.placed.recipe_id else {
                    return false;
                };
                let Some(recipe) = self.recipe(recipe_id) else {
                    return false;
                };
                // A machine takes its recipe's inputs, and — when the recipe needs heat — anything
                // that burns. Fuel is not in `inputs`, so this is where a belt of coal is allowed
                // into a smelter without every smelting recipe having to name a fuel.
                let burns = recipe.fuel > 0
                    && self
                        .item_definition(cargo.item_id)
                        .and_then(|item| item.fuel_value)
                        .unwrap_or(0)
                        > 0;
                let accepts = burns
                    || recipe
                        .inputs
                        .iter()
                        .any(|input| input.item_id == cargo.item_id);
                let capacity = self
                    .building_definition(entity.placed.definition_id)
                    .and_then(|definition| definition.capacity)
                    .unwrap_or(u32::MAX);
                accepts && inventory_total(&entity.inventory) + cargo.quantity <= capacity
            }
            BuildingKind::Container => {
                let capacity = self
                    .building_definition(entity.placed.definition_id)
                    .and_then(|definition| definition.capacity)
                    .unwrap_or(u32::MAX);
                inventory_total(&entity.inventory) + cargo.quantity <= capacity
            }
            BuildingKind::Consumer => true,
            // The hub takes what it asked for and nothing else, by belt exactly as by hand. A line
            // pointed at it backs up once the board and the contract are satisfied, which is a
            // legible answer — the belt shows it — where silently voiding the cargo was not.
            BuildingKind::Hub => self.hub_demand(cargo.item_id) >= u64::from(cargo.quantity),
            BuildingKind::Extractor
            | BuildingKind::Pump
            | BuildingKind::Pole
            | BuildingKind::Bridge => false,
            BuildingKind::Generator | BuildingKind::Boiler => {
                let burns = self
                    .item_definition(cargo.item_id)
                    .and_then(|item| item.fuel_value)
                    .unwrap_or(0)
                    > 0;
                let water = entity.kind == BuildingKind::Boiler && cargo.item_id == WATER_ITEM;
                let capacity = self
                    .building_definition(entity.placed.definition_id)
                    .and_then(|definition| definition.capacity)
                    .unwrap_or(u32::MAX);
                (burns || water) && inventory_total(&entity.inventory) + cargo.quantity <= capacity
            }
        }
    }

    fn accept(&mut self, target: usize, cargo: Cargo) {
        match self.entities[target].kind {
            BuildingKind::Belt => self.entities[target].cargo = Some(cargo),
            BuildingKind::Composer
            | BuildingKind::Container
            | BuildingKind::Generator
            | BuildingKind::Boiler => {
                *self.entities[target]
                    .inventory
                    .entry(cargo.item_id)
                    .or_default() += cargo.quantity;
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
    /// price is posted first and paid on completion.
    ///
    /// A filled slot is replaced in place rather than compacted out, so the row the player was
    /// reading does not jump to another slot the moment it completes. The replacement is not filled
    /// from the same delivery: it starts empty, and the next delivery is what moves it.
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
            let take = definition
                .quantity
                .saturating_sub(self.requests[slot].delivered)
                .min(remaining);
            remaining -= take;
            self.requests[slot].delivered += take;
            if self.requests[slot].delivered < definition.quantity {
                slot += 1;
                continue;
            }
            let pay = self.request_payout(&definition);
            self.insight += u64::from(pay);
            *self.request_rounds.entry(definition.id).or_default() += 1;
            *self.request_fills.entry(definition.id).or_default() += 1;
            self.events.push(format!(
                "{} complete — the hub pays {} insight",
                definition.name, pay
            ));
            let posted = self.posted_requests(Some(slot));
            match self.next_request(&posted) {
                Some(id) => {
                    self.requests[slot] = RequestState {
                        request_id: id,
                        delivered: 0,
                    };
                    slot += 1;
                }
                // Nothing left the player can reach. The slot closes rather than reposting the row
                // that was just paid for, and `refill_requests` opens it again when research does.
                None => {
                    self.requests.remove(slot);
                }
            }
        }
        self.refill_requests();
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

    /// The row that should be posted next: the least-used one the player can actually supply,
    /// unless the board currently holds no row at the deepest reachable depth — then that depth
    /// is reserved, so a three-slot board still leads once processing unlocks rather than cycling
    /// eight raw surveys first.
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
            .filter(|request| self.item_reachable(request.item_id, 0))
            .collect();
        if eligible.is_empty() {
            return None;
        }
        let max_depth = self
            .definitions
            .requests
            .iter()
            .filter(|request| self.item_reachable(request.item_id, 0))
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
            .definitions
            .recipes
            .iter()
            .find(|recipe| recipe.output.item_id == item)
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

    /// What filling this row pays *now*: the first completion is `insight`, every later one is
    /// `repeat_insight` (or `insight` again, when the row does not decay). Skip does not count.
    fn request_payout(&self, definition: &RequestDefinition) -> u32 {
        let fills = self
            .request_fills
            .get(&definition.id)
            .copied()
            .unwrap_or_default();
        if fills == 0 {
            definition.insight
        } else {
            definition.repeat_insight.unwrap_or(definition.insight)
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
            self.requests.push(RequestState {
                request_id: id,
                delivered: 0,
            });
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
        match self
            .definitions
            .recipes
            .iter()
            .find(|recipe| recipe.output.item_id == item)
        {
            Some(recipe) => {
                self.category_unlocked(&recipe.category)
                    && recipe
                        .inputs
                        .iter()
                        .all(|input| self.item_reachable(input.item_id, depth + 1))
            }
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

    fn category_unlocked(&self, category: &str) -> bool {
        self.definitions.buildings.iter().any(|building| {
            building.buildable
                && building.recipe_category.as_deref() == Some(category)
                && self.technology_met(building)
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
            .map(|(definition, state)| {
                u64::from(definition.quantity.saturating_sub(state.delivered))
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
    /// not yet seen — and it forfeits whatever has already been delivered against it, which is why
    /// it is a decision rather than a free reroll.
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
        self.requests[slot] = RequestState {
            request_id: id,
            delivered: 0,
        };
        self.events.push(format!("Passed on {name}"));
        Ok(())
    }

    /// Close every stage the hub can now afford, in order.
    ///
    /// The loop is not decoration: contributions carry forward, so a stage whose bill a previous
    /// surplus already covers must complete in the same delivery rather than wait for one more
    /// item to arrive and re-ask the question.
    fn advance_contract(&mut self) {
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
            for need in &consumed {
                let held = self.contract_contributed.entry(need.item_id).or_default();
                *held = held.saturating_sub(u64::from(need.quantity));
            }
            self.contract_stage += 1;
            self.events
                .push(format!("{name} complete — the landing hub grows"));
            if self.contract_stage >= self.scenario.contract.stages.len() {
                self.victory = true;
                self.events
                    .push("Founding contract complete — free play continues".into());
            }
        }
    }

    fn set_move_intent(&mut self, x: i16, y: i16) -> Result<(), String> {
        if !(-1000..=1000).contains(&x) || !(-1000..=1000).contains(&y) {
            return Err("movement intent must be in -1000..1000".into());
        }
        self.player.move_x = x;
        self.player.move_y = y;
        if x != 0 || y != 0 {
            self.player.facing_x = x;
            self.player.facing_y = y;
        }
        Ok(())
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

    /// One player-clock step, in world units. Land uses the host's intent against `PLAYER_SPEED`.
    /// Shallows are a 1 m/s ford: walk and run collapse to the same crawl, so holding Shift in a
    /// river does not buy a faster crossing.
    fn player_step(&self) -> (i32, i32) {
        let mut intent_x = self.player.move_x;
        let mut intent_y = self.player.move_y;
        let mut speed = PLAYER_SPEED;
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
        self.terrain_at(q, r) == Terrain::ShallowWater
    }

    fn player_blocked(&self, x: i32, y: i32) -> bool {
        let (q, r) = world_to_axial(x, y);
        let feature_collision = self.terrain_at(q, r).blocks_movement();
        feature_collision
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
        if !self.field_covered_at(origin, (q, r), EXTRACT_RADIUS) {
            return Err("that hex is out of reach".into());
        }
        self.gather_from((q, r))
    }

    /// Take one unit out of a field cell that has already been resolved and range-checked. Both
    /// gathers land here, so the cooldown, the carrying rule, the depletion mark, and the event
    /// are one implementation and cannot drift apart.
    fn gather_from(&mut self, key: (i32, i32)) -> Result<(), String> {
        let field = self
            .field_at(key.0, key.1)
            .ok_or("stand on or beside a field hex to gather")?;
        // `resource_at_world` filters empty cells for the untargeted gather, but a named hex has
        // not been through that filter — and an empty one would underflow the subtraction below.
        if self.deposit_quantity(key) == 0 {
            return Err("this deposit is worked out".into());
        }
        if self.player_room_for(field.item_id) == 0 {
            return Err("carrying capacity is full".into());
        }
        let name = self
            .item_definition(field.item_id)
            .map(|item| item.name.clone())
            .unwrap_or_else(|| format!("item {}", field.item_id));
        let steps = self
            .item_definition(field.item_id)
            .and_then(|item| item.hand_gather_steps)
            .ok_or_else(|| {
                format!("{name} cannot be gathered by hand — place an extractor on the field")
            })?;
        let remaining = self.deposit_quantity(key) - 1;
        self.write_overlay(
            key.0,
            key.1,
            field.item_id,
            remaining,
            field.initial_quantity,
        );
        let (item_id, depleted) = (field.item_id, remaining == 0);
        self.dirty.resources.push(key);
        *self.player.inventory.entry(item_id).or_default() += 1;
        self.player.action_cooldown = steps;
        self.last_action_cooldown_total = steps;
        // Named, not numbered. "Gathered item 6" was serviceable when the world held three items;
        // against a material base of twenty-three it tells the player nothing they can act on.
        self.events.push(format!("Gathered {name}"));
        if depleted {
            // Any extractor covering this deposit may now report a different status.
            self.mark_all_entities_dirty();
            self.events.push("Deposit depleted".into());
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn deposit_inventory(&mut self) -> Result<(), String> {
        self.deposit_item(None)
    }

    fn deposit_item(&mut self, target_item: Option<ItemId>) -> Result<(), String> {
        let hub = self
            .entities
            .iter()
            .find(|entity| entity.kind == BuildingKind::Hub);
        let Some(hub) = hub else {
            return Err("this scenario has no landing hub".into());
        };
        let (hub_x, hub_y) = axial_world(hub.placed.q, hub.placed.r);
        if squared_distance(self.player.x, self.player.y, hub_x, hub_y)
            > i64::from(HUB_RANGE).pow(2)
        {
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

    fn research(&mut self, technology_id: TechnologyId) -> Result<(), String> {
        let technology = self
            .technology(technology_id)
            .cloned()
            .ok_or_else(|| format!("unknown technology {technology_id}"))?;
        if self.researched.contains(&technology_id) {
            return Err("technology already researched".into());
        }
        if technology
            .prerequisites
            .iter()
            .any(|required| !self.researched.contains(required))
        {
            return Err("technology prerequisites are not complete".into());
        }
        if self.insight < u64::from(technology.cost) {
            return Err(format!("requires {} insight", technology.cost));
        }
        self.insight -= u64::from(technology.cost);
        self.researched.insert(technology_id);
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
        if let Some(required) = definition.unlock_technology_id {
            if !self.researched.contains(&required) {
                return Err("building is locked by research".into());
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
        let (anchor_x, anchor_y) = axial_world(q, r);
        if squared_distance(self.player.x, self.player.y, anchor_x, anchor_y)
            > i64::from(self.player.build_range).pow(2)
        {
            return Err("placement is outside build range".into());
        }
        for cell in &footprint {
            let supported_transport = definition.kind == BuildingKind::Belt
                && self.bridge_at(cell.q, cell.r)
                && self
                    .entity_at(cell.q, cell.r)
                    .is_some_and(|index| self.entities[index].kind == BuildingKind::Bridge);
            if self.entity_at(cell.q, cell.r).is_some() && !supported_transport {
                return Err("building footprint overlaps an occupied hex".into());
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
            let terrain = self.terrain_at(cell.q, cell.r);
            let shallow_support = definition.placement_rule == PlacementRule::Shallows
                && terrain == Terrain::ShallowWater;
            let bridged_transport = definition.kind == BuildingKind::Belt
                && terrain == Terrain::ShallowWater
                && self.bridge_at(cell.q, cell.r);
            if terrain.blocks_construction() && !shallow_support && !bridged_transport {
                return Err("environment blocks construction".into());
            }
        }
        if definition.placement_rule == PlacementRule::Resource
            && self.deposit_quantity((q, r)) == 0
        {
            return Err("extractors require a non-empty deposit".into());
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
        if definition.placement_rule == PlacementRule::Shallows
            && self.terrain_at(q, r) != Terrain::ShallowWater
        {
            return Err("bridges require shallow water".into());
        }
        if definition.kind == BuildingKind::Composer {
            let id = recipe_id.ok_or("this machine requires a recipe")?;
            let recipe = self
                .recipe(id)
                .ok_or_else(|| format!("unknown recipe {id}"))?;
            // One field, one check: a kiln cannot be given a circuit recipe because the categories
            // disagree, not because there is a separate building kind for every machine.
            if definition.recipe_category.as_deref() != Some(recipe.category.as_str()) {
                return Err(format!(
                    "{} cannot run a {} recipe",
                    definition.name, recipe.category
                ));
            }
        }
        if check_cost {
            let missing: Vec<String> = definition
                .construction_cost
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
        for ingredient in &definition.construction_cost {
            subtract_item(
                &mut self.player.inventory,
                ingredient.item_id,
                ingredient.quantity,
            );
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
            reserved_inputs: BTreeMap::new(),
            progress: 0,
            fuel_charge: 0,
            power_charge: 0,
            burn_progress: 0,
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
        let name = definition.name.clone();
        let cells = line_between(from, to, definition.orientation_axis);
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
                self.events.push(if count == 1 {
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
        let cost = definition.construction_cost.clone();
        let cells = line_between(from, to, definition.orientation_axis);
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
                let legal = !taken.contains(&(q, r))
                    && self
                        .placement_legality(q, r, definition_id, cell_orientation, recipe_id, false)
                        .is_ok()
                    && has_ingredients(&budget, &cost);
                if legal {
                    for ingredient in &cost {
                        subtract_item(&mut budget, ingredient.item_id, ingredient.quantity);
                    }
                    taken.insert((q, r));
                }
                LinePreviewCell {
                    q,
                    r,
                    orientation: cell_orientation,
                    legal,
                }
            })
            .collect()
    }

    /// What a removal drag between these endpoints would take back. Refunds accumulate against a
    /// copy of the player's inventory as the run is walked, for the same reason the construction
    /// preview spends materials against one: the cell a run stops at has to be visible before the
    /// drag is released, whether it stops for cost or for carrying space.
    fn erase_line_preview(&self, from: (i32, i32), to: (i32, i32)) -> Vec<LinePreviewCell> {
        let mut carried = self.player.inventory.clone();
        let mut taken = BTreeSet::new();
        line_between(from, to, self.erase_line_axis(from))
            .into_iter()
            .map(|(q, r)| {
                let (x, y) = axial_world(q, r);
                let in_range = squared_distance(self.player.x, self.player.y, x, y)
                    <= i64::from(self.player.build_range).pow(2);
                let removable = self.entity_at(q, r).filter(|&index| {
                    !self.entities[index].placed.scenario_owned
                        && !taken.contains(&self.entities[index].id)
                });
                let legal = in_range
                    && removable.is_some_and(|index| {
                        let refund = self.erase_refund(index);
                        let mut prospective = carried.clone();
                        add_inventory(&mut prospective, &refund);
                        let fits = self.slots_used(&prospective) <= self.player.carry_slots;
                        if fits {
                            carried = prospective;
                            taken.insert(self.entities[index].id);
                        }
                        fits
                    });
                LinePreviewCell {
                    q,
                    r,
                    orientation: 0,
                    legal,
                }
            })
            .collect()
    }

    /// Which axis a removal drag walks. Erasure carries no definition to ask, so it asks the hex
    /// the drag started on: a run that begins on a riser takes back the riser column, and every
    /// other run walks the six edges exactly as it did before v0.14. Deterministic and native, like
    /// the path itself.
    fn erase_line_axis(&self, from: (i32, i32)) -> OrientationAxis {
        self.entity_at(from.0, from.1)
            .and_then(|index| self.building_definition(self.entities[index].placed.definition_id))
            .map(|definition| definition.orientation_axis)
            .unwrap_or_default()
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
        let (target_x, target_y) = axial_world(q, r);
        if squared_distance(self.player.x, self.player.y, target_x, target_y)
            > i64::from(self.player.build_range).pow(2)
        {
            return Err("erase target is outside build range".into());
        }
        let index = self.entity_at(q, r).ok_or("no building to erase")?;
        if self.entities[index].placed.scenario_owned {
            return Err("scenario-owned objects are protected".into());
        }
        // Erase refunds the construction cost plus everything the building held, so it is the
        // largest single addition to the player's inventory in the game. Of the three defensible
        // answers to a refund that will not fit — refuse, refund partially, or spill on the ground
        // — this picks refusal, because it is the only one that keeps item conservation exact and
        // leaves the recovery available once the player has made room.
        let refund = self.erase_refund(index);
        if !self.player_can_carry(&refund) {
            return Err("no room to carry what this would recover".into());
        }
        let old_links = self.graph_links_by_id();
        let changed_cells = self
            .entity_footprint(&self.entities[index])
            .into_iter()
            .map(|cell| (cell.q, cell.r))
            .collect();
        let entity = self.entities.remove(index);
        self.deposit_links.remove(&entity.id);
        self.dirty.removed.insert(entity.id);
        self.dirty.chunks = true;
        let name = self
            .building_definition(entity.placed.definition_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| "building".into());
        add_inventory(&mut self.player.inventory, &refund);
        self.recompile_graph_components(&old_links, &changed_cells, &BTreeSet::from([entity.id]));
        self.events.push(format!("Recovered {name}"));
        Ok(())
    }

    /// Everything erasing this entity hands back: its construction cost, its stored inventory, its
    /// reserved recipe inputs, and any cargo in transit through it. Resolved before the removal so
    /// the carrying check and the refund cannot describe different things.
    fn erase_refund(&self, index: usize) -> BTreeMap<ItemId, u32> {
        let entity = &self.entities[index];
        let mut refund = BTreeMap::new();
        if let Some(definition) = self.building_definition(entity.placed.definition_id) {
            add_ingredients(&mut refund, &definition.construction_cost);
        }
        add_inventory(&mut refund, &entity.inventory);
        add_inventory(&mut refund, &entity.reserved_inputs);
        if let Some(cargo) = entity.cargo {
            *refund.entry(cargo.item_id).or_default() += cargo.quantity;
        }
        refund
    }

    /// Move stock out of a container and into the player's pack. A new bounded command beside
    /// `place` and `erase`, range-checked exactly as they are. The requested quantity is a ceiling,
    /// not a demand: what actually moves is limited by what the container holds and by what the
    /// player can still carry, so a partial withdrawal succeeds and destroys nothing.
    fn withdraw(&mut self, q: i32, r: i32, item_id: ItemId, quantity: u32) -> Result<(), String> {
        let (target_x, target_y) = axial_world(q, r);
        if squared_distance(self.player.x, self.player.y, target_x, target_y)
            > i64::from(self.player.build_range).pow(2)
        {
            return Err("withdraw target is outside build range".into());
        }
        let index = self.entity_at(q, r).ok_or("no building to unload")?;
        if self.entities[index].kind != BuildingKind::Container {
            return Err("only containers can be unloaded by hand".into());
        }
        let stored = self.entities[index]
            .inventory
            .get(&item_id)
            .copied()
            .unwrap_or(0);
        if stored == 0 {
            return Err("this container holds none of that item".into());
        }
        let moved = quantity.min(stored).min(self.player_room_for(item_id));
        if moved == 0 {
            return Err("carrying capacity is full".into());
        }
        subtract_item(&mut self.entities[index].inventory, item_id, moved);
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
    fn upgrade(&mut self, q: i32, r: i32) -> Result<(), String> {
        let (target_x, target_y) = axial_world(q, r);
        if squared_distance(self.player.x, self.player.y, target_x, target_y)
            > i64::from(self.player.build_range).pow(2)
        {
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
        let refund = current.construction_cost.clone();
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
            let stored: u32 = self.entities[index].inventory.values().copied().sum();
            if stored > capacity {
                return Err(format!(
                    "{} holds {stored}, more than the next tier stores",
                    current.name
                ));
            }
        }
        // Netted per item, so the two halves of the price never travel through the pack. A player
        // upgrading with a full pack is charged the difference and asked to carry the difference,
        // which is what an in-place edit actually costs them.
        let mut charge: BTreeMap<ItemId, u32> = BTreeMap::new();
        add_ingredients(&mut charge, &next.construction_cost);
        let mut credit: BTreeMap<ItemId, u32> = BTreeMap::new();
        add_ingredients(&mut credit, &refund);
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
        // Only when the step actually hands something back. A tier whose cost contains the tier
        // below it — which is the shape a ladder should have — returns nothing, and refusing that
        // upgrade because the pack is full would be refusing an edit that does not touch the pack.
        if !back.is_empty() && !self.player_can_carry(&back) {
            return Err("no room to carry what this would return".into());
        }
        let old_links = self.graph_links_by_id();
        let changed_cells = self
            .entity_footprint(&self.entities[index])
            .into_iter()
            .map(|cell| (cell.q, cell.r))
            .collect();
        for (item_id, quantity) in &owed {
            subtract_item(&mut self.player.inventory, *item_id, *quantity);
        }
        add_inventory(&mut self.player.inventory, &back);
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

    /// Put stock from the player's pack into a container. The exact mirror of `withdraw`, and it
    /// keeps the same contract: the requested quantity is a ceiling, not a demand, so what actually
    /// moves is limited by what the player holds and by the room the container has left. A partial
    /// store succeeds and destroys nothing.
    ///
    /// Containers only. A machine's inputs belong to the recipe that reserved them — the same
    /// reason composers still cannot be unloaded by hand.
    fn store(&mut self, q: i32, r: i32, item_id: ItemId, quantity: u32) -> Result<(), String> {
        let (target_x, target_y) = axial_world(q, r);
        if squared_distance(self.player.x, self.player.y, target_x, target_y)
            > i64::from(self.player.build_range).pow(2)
        {
            return Err("store target is outside build range".into());
        }
        let index = self.entity_at(q, r).ok_or("no building to load")?;
        if self.entities[index].kind != BuildingKind::Container {
            return Err("only containers can be loaded by hand".into());
        }
        let held = self.player.inventory.get(&item_id).copied().unwrap_or(0);
        if held == 0 {
            return Err("you are not carrying any of that item".into());
        }
        let capacity = self
            .building_definition(self.entities[index].placed.definition_id)
            .and_then(|definition| definition.capacity)
            .unwrap_or(0);
        let room = capacity.saturating_sub(inventory_total(&self.entities[index].inventory));
        let moved = quantity.min(held).min(room);
        if moved == 0 {
            return Err("this container is full".into());
        }
        subtract_item(&mut self.player.inventory, item_id, moved);
        *self.entities[index].inventory.entry(item_id).or_default() += moved;
        let id = self.entities[index].id;
        self.dirty.entities.push(id);
        let name = self
            .item_definition(item_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| format!("item {item_id}"));
        self.events.push(format!("Stored {moved} × {name}"));
        Ok(())
    }

    /// Give the machine at this hex a different recipe. Bounded and range-checked like every other
    /// edit, and it enforces the same category rule placement does, so a kiln can no more be
    /// reassigned to a circuit than it could be built with one.
    ///
    /// A machine mid-craft is refused rather than reassigned: its reserved inputs belong to the job
    /// it is running, and deciding what happens to a part-finished one is a question worth its own
    /// pass — the same reason composers still cannot be unloaded.
    fn set_recipe(&mut self, q: i32, r: i32, recipe_id: RecipeId) -> Result<(), String> {
        let (target_x, target_y) = axial_world(q, r);
        if squared_distance(self.player.x, self.player.y, target_x, target_y)
            > i64::from(self.player.build_range).pow(2)
        {
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
        if definition.recipe_category.as_deref() != Some(recipe.category.as_str()) {
            return Err(format!(
                "{} cannot run a {} recipe",
                definition.name, recipe.category
            ));
        }
        self.entities[index].placed.recipe_id = Some(recipe_id);
        let id = self.entities[index].id;
        self.dirty.entities.push(id);
        self.events.push(format!("Set recipe to {}", recipe.name));
        Ok(())
    }

    fn rotate(&mut self, q: i32, r: i32) -> Result<(), String> {
        let (target_x, target_y) = axial_world(q, r);
        if squared_distance(self.player.x, self.player.y, target_x, target_y)
            > i64::from(self.player.build_range).pow(2)
        {
            return Err("rotate target is outside build range".into());
        }
        let index = self.entity_at(q, r).ok_or("no building to rotate")?;
        if self.entities[index].placed.scenario_owned {
            return Err("scenario-owned objects are protected".into());
        }
        let old_links = self.graph_links_by_id();
        let old_footprint = self.entity_footprint(&self.entities[index]);
        let id = self.entities[index].id;
        // Rotation stays on the definition's own axis: a belt walks the six edges and a riser the
        // six corners. A building can never be turned into a heading it could not have
        // been built at.
        let axis = self
            .building_definition(self.entities[index].placed.definition_id)
            .map(|definition| definition.orientation_axis)
            .unwrap_or_default();
        let next_orientation = axis.next(self.entities[index].placed.orientation);
        let next_footprint = self.footprint_for(self.entities[index].placed, next_orientation);
        let rotating_kind = self.entities[index].kind;
        if next_footprint.iter().any(|cell| {
            self.entities.iter().enumerate().any(|(other, entity)| {
                other != index
                    && !(rotating_kind == BuildingKind::Belt && entity.kind == BuildingKind::Bridge)
                    && self
                        .entity_footprint(entity)
                        .iter()
                        .any(|occupied| occupied == cell)
            })
        }) {
            return Err("rotated footprint would overlap another building".into());
        }
        self.entities[index].placed.orientation = next_orientation;
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
                InputCommand::Rotate { q, r } => self.rotate(q, r),
                InputCommand::Upgrade { q, r } => self.upgrade(q, r),
                InputCommand::Withdraw {
                    q,
                    r,
                    item_id,
                    quantity,
                } => self.withdraw(q, r, item_id, quantity),
                InputCommand::Store {
                    q,
                    r,
                    item_id,
                    quantity,
                } => self.store(q, r, item_id, quantity),
                InputCommand::SetRecipe { q, r, recipe_id } => self.set_recipe(q, r, recipe_id),
                InputCommand::Undo => self.undo(),
                InputCommand::Research { technology_id } => self.research(technology_id),
                InputCommand::SkipRequest { slot } => self.skip_request(slot),
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
        match entity.kind {
            BuildingKind::Extractor if entity.cargo.is_some() => EntityStatus::OutputBlocked,
            BuildingKind::Extractor if !deposit_available => EntityStatus::DepositDepleted,
            BuildingKind::Extractor if !powered => EntityStatus::NoPower,
            BuildingKind::Extractor if brownout => EntityStatus::Brownout,
            BuildingKind::Extractor if entity.progress > 0 => EntityStatus::Extracting,
            BuildingKind::Pump if entity.cargo.is_some() => EntityStatus::OutputBlocked,
            BuildingKind::Pump if !deposit_available => EntityStatus::NoWaterInReach,
            BuildingKind::Pump if !powered => EntityStatus::NoPower,
            BuildingKind::Pump if brownout => EntityStatus::Brownout,
            BuildingKind::Pump => EntityStatus::Pumping,
            BuildingKind::Composer if entity.cargo.is_some() => EntityStatus::OutputBlocked,
            BuildingKind::Composer if entity.progress > 0 && brownout => EntityStatus::Brownout,
            BuildingKind::Composer if entity.progress > 0 => EntityStatus::Composing,
            BuildingKind::Composer if !powered => EntityStatus::NoPower,
            BuildingKind::Composer if !fuel_ready => EntityStatus::OutOfFuel,
            BuildingKind::Composer => EntityStatus::WaitingForInputs,
            BuildingKind::Container if inventory_total(&entity.inventory) > 0 => {
                EntityStatus::Buffered
            }
            BuildingKind::Belt if entity.cargo.is_some() => EntityStatus::Carrying,
            BuildingKind::Consumer => EntityStatus::Receiving,
            BuildingKind::Hub => EntityStatus::LandingHub,
            BuildingKind::Generator => self.generator_status(index),
            BuildingKind::Boiler if self.boiler_live(index) => EntityStatus::Generating,
            BuildingKind::Boiler
                if entity.inventory.get(&WATER_ITEM).copied().unwrap_or(0) == 0 =>
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

    /// One entity's snapshot. Every path that reports an entity to the host — the complete
    /// snapshot and the incremental delta alike — builds it here, so the sparse path cannot drift
    /// from the full one.
    fn entity_snapshot(&mut self, index: usize) -> EntitySnapshot {
        // Resolving through the cached candidate list rather than scanning the tile map is what
        // keeps this O(1) in world size. The cache is derived state, so filling it changes nothing.
        let deposit_available = match self.entities[index].kind {
            BuildingKind::Extractor => self.extractor_deposit(index).is_some(),
            // A pump's "deposit" is the basin it stands beside, which never empties, so the only
            // question is whether one is in reach at all.
            BuildingKind::Pump => {
                let placed = self.entities[index].placed;
                let radius = self
                    .building_definition(placed.definition_id)
                    .and_then(|definition| definition.extract_radius)
                    .unwrap_or(PUMP_RADIUS as u32) as i32;
                self.water_within_reach(placed.q, placed.r, radius)
            }
            _ => false,
        };
        let entity = &self.entities[index];
        let fuel_required = entity
            .placed
            .recipe_id
            .and_then(|id| self.recipe(id))
            .map_or(0, |recipe| recipe.fuel);
        let fuel_ready = self.fuel_ready(entity);
        // Two different failures, and the player fixes them with two different buildings: `powered`
        // is "wired to something that generates" and wants a pole or a plant, `brownout` is "wired
        // in but the bank ran dry" and wants more generation.
        let powered = self.entity_connected(index);
        let brownout = powered && !self.entity_powered(index);
        let (power_satisfied, power_demand) = self.network_of(index);
        let power_charge = entity.power_charge;
        let power_capacity = self.power_capacity(index);
        let progress_total = self.progress_total(index);
        let footprint = self.entity_footprint(entity);
        let next_id = self.graph[index].map(|target| self.entities[target].id);
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
            inventory: entity
                .inventory
                .iter()
                .map(|(&item_id, &quantity)| Ingredient { item_id, quantity })
                .collect(),
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

    fn terrain_snapshots(&self) -> Vec<TileSnapshot> {
        let mut tiles = Vec::new();
        let size = self.scenario.chunk_size;
        for &(chunk_q, chunk_r) in &self.generated_chunks {
            for (q, r) in hexes_in_chunk(chunk_q, chunk_r, size) {
                let terrain = self.terrain_at(q, r);
                if terrain == Terrain::Lowland {
                    continue;
                }
                let (x, y) = axial_world(q, r);
                tiles.push(TileSnapshot {
                    q,
                    r,
                    x,
                    y,
                    radius: HEX_RADIUS as u32,
                    terrain,
                });
            }
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

    /// The board, in slot order, with the price on every row.
    fn request_snapshots(&self) -> Vec<RequestSnapshot> {
        self.requests
            .iter()
            .filter_map(|state| {
                let definition = self.request_definition(state.request_id)?;
                Some(RequestSnapshot {
                    key: definition.key.clone(),
                    name: definition.name.clone(),
                    brief: definition.brief.clone(),
                    item_id: definition.item_id,
                    delivered: state.delivered.min(definition.quantity),
                    required: definition.quantity,
                    insight: self.request_payout(definition),
                })
            })
            .collect()
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
            delivered: self.delivered,
            delivered_by_item: self.delivered_by_item_snapshot(),
            insight: self.insight,
            victory: self.victory,
            contract: self.contract_snapshot(),
            requests: self.request_snapshots(),
            player: self.player_snapshot(),
            researched: self.researched.iter().copied().collect(),
            chunks,
            terrain,
            resources,
            buildings,
            events: self.events.clone(),
        }
    }

    fn checksum(&self) -> u32 {
        let mut hash = 0x811c9dc5u32;
        hash_bytes(&mut hash, self.scenario.key.as_bytes());
        hash_u32(&mut hash, u32::from(WORLD_GENERATOR_VERSION));
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
        for (&item, &quantity) in &self.player.inventory {
            hash_u32(&mut hash, u32::from(item));
            hash_u32(&mut hash, quantity);
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
            hash_inventory(&mut hash, &entity.inventory);
            hash_inventory(&mut hash, &entity.reserved_inputs);
            if let Some(cargo) = entity.cargo {
                hash_u32(&mut hash, u32::from(cargo.item_id));
                hash_u32(&mut hash, cargo.quantity);
            } else {
                hash_u32(&mut hash, 0);
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
            hash_u32(&mut hash, state.delivered);
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
            player: self.player.clone(),
            researched: self.researched.clone(),
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
            produced: self.produced.clone(),
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
        let envelope: SaveEnvelope =
            serde_json::from_str(json).map_err(|error| format!("malformed HXF1 save: {error}"))?;
        if envelope.save_version != SAVE_VERSION {
            return Err(format!(
                "unsupported save version {}",
                envelope.save_version
            ));
        }
        if envelope.world_generator_version != WORLD_GENERATOR_VERSION {
            return Err("save world generator version is incompatible".into());
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
        let mut core = Core::new(
            definitions,
            technologies,
            scenario,
            Some(envelope.state.seed),
            Some(envelope.state.world_params.clone()),
        )?;
        validate_saved_state(definitions, technologies, scenario, &envelope.state)?;
        core.seed = envelope.state.seed;
        core.world_params = envelope.state.world_params;
        // The lattice and the bootstrap table are derived from exactly these two, so they are
        // rebuilt the moment either moves rather than carried in the file.
        core.fields = WorldFields::new(&core.world_params, core.seed);
        core.generated_chunks = envelope
            .state
            .generated_chunks
            .iter()
            .map(|coordinate| (coordinate.q, coordinate.r))
            .collect();
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
        // A save records entities in stable id order; sorting makes that an invariant of the loaded
        // core rather than a property of the file. Entity order is not a simulation input — the
        // checksum and every arbitration order sort by id — so this cannot change a result.
        core.entities.sort_by_key(|entity| entity.id);
        core.player = envelope.state.player;
        core.researched = envelope.state.researched;
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
        core.last_action_cooldown_total = core.player.action_cooldown;
        core.produced = envelope.state.produced;
        core.events = vec!["HXF1 save restored".into()];
        core.compile_graph();
        if core.checksum() != envelope.checksum {
            return Err("save checksum does not match its native state".into());
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
    seed: u32,
    /// Beside the seed, because a world is both. The overlay a save carries is only meaningful
    /// against the generation it was cut from.
    world_params: WorldParams,
    generated_chunks: Vec<Coordinate>,
    tiles: Vec<TileState>,
    entities: Vec<Entity>,
    player: PlayerState,
    researched: BTreeSet<TechnologyId>,
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
    produced: BTreeMap<ItemId, u64>,
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
    chunks: Vec<ChunkSnapshot>,
    buildings: BTreeMap<u32, EntitySnapshot>,
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
            chunks: snapshot.chunks.clone(),
            buildings: snapshot
                .buildings
                .iter()
                .map(|entity| (entity.id, entity.clone()))
                .collect(),
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

        SnapshotDelta {
            base_revision,
            revision,
            tick: core.tick,
            checksum: core.checksum(),
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
            // add a non-ground tile, and it marks exactly that case.
            terrain: dirty.terrain.then(|| core.terrain_snapshots()),
            resources,
            buildings,
            events: take_changed(&mut baseline.events, core.events.clone()),
        }
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
        let core = Core::new(
            &definitions,
            &technologies,
            scenario,
            seed_override,
            world_params,
        )
        .map_err(js_error)?;
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
        self.core = Core::new(
            &self.definitions,
            &self.technologies,
            &self.core.scenario,
            Some(self.core.seed),
            Some(self.core.world_params.clone()),
        )
        .map_err(js_error)?;
        // The core the baseline described is gone, so the next delta is a complete replacement.
        self.baseline = None;
        Ok(())
    }

    pub fn new_game(
        &mut self,
        scenario_key: &str,
        seed_override: Option<u32>,
        world_params_json: Option<String>,
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
    let input: WorldParamsInput = serde_json::from_str(json)
        .map_err(|error| js_error(format!("malformed world parameters: {error}")))?;
    Ok(Some(match input {
        WorldParamsInput::Preset { preset } => preset_params(&preset)
            .ok_or_else(|| js_error(format!("unknown world preset {preset}")))?,
        WorldParamsInput::Params(params) => *params,
    }))
}

fn validate_all(
    definitions: &DefinitionsInput,
    technologies: &TechnologiesInput,
    scenarios: &ScenariosInput,
) -> Result<(), String> {
    validate_definitions(definitions)?;
    validate_technologies(definitions, technologies)?;
    validate_scenarios(definitions, technologies, scenarios)
}

fn validate_definitions(definitions: &DefinitionsInput) -> Result<(), String> {
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
            || request.repeat_insight == Some(0)
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
        {
            return Err(format!(
                "item {} has a zero fuel, regrowth, or hand gather rate",
                item.id
            ));
        }
    }
    let categories: BTreeSet<&str> = definitions
        .buildings
        .iter()
        .filter_map(|building| building.recipe_category.as_deref())
        .collect();
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
        if !categories.contains(recipe.category.as_str()) {
            return Err(format!(
                "recipe {} has category {}, which no building runs",
                recipe.id, recipe.category
            ));
        }
        for ingredient in recipe.inputs.iter().chain(std::iter::once(&recipe.output)) {
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
        // A machine that runs recipes needs a category, and one that does not must not claim one.
        if (building.kind == BuildingKind::Composer) != building.recipe_category.is_some() {
            return Err(format!(
                "building {} has a recipe category that does not match its kind",
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
            || footprint.len() > 7
        {
            return Err(format!("building {} has an invalid footprint", building.id));
        }
        // No shipped definition needs a multi-cell corner-heading footprint yet. Keep the narrow
        // rule until a real definition asks for the extra path and can test it.
        if building.orientation_axis == OrientationAxis::Corner && building.footprint.len() != 1 {
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
        for ingredient in &building.construction_cost {
            if ingredient.quantity == 0 || !item_ids.contains(&ingredient.item_id) {
                return Err(format!("building {} has an invalid cost", building.id));
            }
        }
    }
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
        if next.kind != building.kind || next.recipe_category != building.recipe_category {
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
        let footprint: BTreeSet<_> = building
            .footprint
            .iter()
            .map(|cell| (cell.q, cell.r))
            .collect();
        let next_footprint: BTreeSet<_> =
            next.footprint.iter().map(|cell| (cell.q, cell.r)).collect();
        if footprint != next_footprint {
            return Err(format!(
                "building {} upgrades to a different footprint, which would move its connections",
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
    for technology in &technologies.technologies {
        if technology.key.trim().is_empty()
            || technology.name.trim().is_empty()
            || technology.description.trim().is_empty()
            || technology.cost == 0
        {
            return Err(format!("technology {} is incomplete", technology.id));
        }
        if technology.prerequisites.iter().any(|id| !ids.contains(id)) {
            return Err(format!(
                "technology {} has an unknown prerequisite",
                technology.id
            ));
        }
        if technology
            .unlocks
            .iter()
            .any(|id| !building_ids.contains(id))
        {
            return Err(format!(
                "technology {} has an unknown unlock",
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
) -> Result<(), String> {
    let item_ids: BTreeSet<_> = definitions.items.iter().map(|value| value.id).collect();
    let technology_ids: BTreeSet<_> = technologies
        .technologies
        .iter()
        .map(|value| value.id)
        .collect();
    let mut coordinates = BTreeMap::new();
    let mut entity_ids = BTreeSet::new();
    for entity in &state.entities {
        let definition = definitions
            .buildings
            .iter()
            .find(|value| value.id == entity.placed.definition_id)
            .ok_or("save references an unknown building")?;
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
        if entity.kind != definition.kind
            || !definition
                .orientation_axis
                .allows(entity.placed.orientation)
            || !footprint_valid
            || !entity_ids.insert(entity.id)
        {
            return Err("save contains invalid entity state".into());
        }
    }
    if !(-1000..=1000).contains(&state.player.facing_x)
        || !(-1000..=1000).contains(&state.player.facing_y)
        || !(-1000..=1000).contains(&state.player.move_x)
        || !(-1000..=1000).contains(&state.player.move_y)
        || state.player.build_range != scenario.build_range.saturating_mul(HEX_X as u32)
        || state.player.carry_slots != scenario.carry_slots
        || state
            .player
            .inventory
            .keys()
            .any(|item| !item_ids.contains(item))
        || state
            .researched
            .iter()
            .any(|id| !technology_ids.contains(id))
    {
        return Err("save contains invalid player or research state".into());
    }
    // A board is restored rather than redrawn, so it is checked instead: a slot naming a row this
    // build no longer ships, a duplicate slot, or one holding more than it ever asked for would all
    // survive the checksum and then be drawn as a request nobody can read.
    let mut posted = BTreeSet::new();
    for slot in &state.requests {
        let definition = definitions
            .requests
            .iter()
            .find(|request| request.id == slot.request_id)
            .ok_or("save references an unknown hub request")?;
        if slot.delivered > definition.quantity || !posted.insert(slot.request_id) {
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
    Ok(())
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
    }
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
        // Ascending cuts are what makes each band reachable. Out of order, a band is not rare —
        // it is unreachable, and the world silently loses whatever the table put in it.
        if !(self.water_level < self.shore_level
            && self.shore_level < self.hills_level
            && self.hills_level < self.highland_level)
        {
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
            if !definitions.items.iter().any(|item| item.id == rule.item_id) {
                return Err(format!("site rule names unknown item {}", rule.item_id));
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
/// a six-minute walk at 3 m/s, which is the scale oceans and ranges are allowed to ask for.
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
            ..rule(Terrain::Highland, COAL, 26, 2, 4, ANY, 18, 8, 3)
        },
        // Scree around mountains. Cliff hexes are members and are unworkable, so the buildable rim
        // is where you quarry — v0.11's extraction-radius lesson intact, at fifty times the supply
        // the eighteen cliff cells of version 6 could offer.
        SiteRule {
            member: vec![Terrain::Highland, Terrain::Cliff],
            ..rule(Terrain::Highland, STONE, 26, 3, 5, ANY, 12, 12, 2)
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
        SiteRule {
            member: ore_bands,
            ..rule(Terrain::Hills, COAL, 16, 2, 3, 40_000, 18, 8, 3)
        },
        // A forest: 150–250 units across a large area, renewable through the `regrowth_ticks` the
        // item already carries, with a soft edge. Three per cell is a rate change as well as a
        // shape change — a base extractor drains its seven hexes and then runs at whatever regrowth
        // supplies — which is why forestry is a question of area rather than of throughput.
        rule(Terrain::Lowland, WOOD, 30, 5, 6, ANY, 3, 1, 2),
        // Riverbanks and lake shores. Rivers are what make this common rather than decorative,
        // which is why the two ship together.
        SiteRule {
            member: vec![Terrain::Lowland, Terrain::Shore],
            member_water_within: 2,
            ..rule(Terrain::Lowland, CLAY, 24, 2, 3, ANY, 14, 14, 3)
        },
        SiteRule {
            member: vec![Terrain::Lowland, Terrain::Shore],
            member_water_within: 2,
            ..rule(Terrain::Shore, CLAY, 24, 2, 3, ANY, 14, 14, 3)
        },
        // Sand sits on real coast, not on the rim of every pond: the disc is clipped to the shore
        // band, so what survives is a beach strip rather than a blob.
        SiteRule {
            center_ocean: true,
            ..rule(Terrain::Shore, SAND, 40, 3, 5, ANY, 16, 16, 3)
        },
        // The same beach, reached from the land side. A shore band is a thin ribbon — 26 per mille
        // of `highlands` — so a rule that can only start *on* it is a coin flip on how many of a
        // handful of lattice cells happen to land in the ribbon, and `highlands` lost sand from
        // the world entirely on that coin flip. A centre just inland clips to exactly the same
        // strip, and the ocean gate still decides which coast qualifies.
        SiteRule {
            member: vec![Terrain::Shore],
            center_ocean: true,
            ..rule(Terrain::Lowland, SAND, 10, 3, 5, ANY, 16, 16, 3)
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
const BOOTSTRAP_GUARANTEES: [(ItemId, i32, i32); 6] = [
    // The first extractor and the first thing a player walks into, both in sight of the hub.
    (IRON_ORE, 9, 14),
    (WOOD, 9, 14),
    // A short walk, chosen rather than stumbled on.
    (COAL, 15, 25),
    (STONE, 15, 25),
    // Carries a river or a shore with it, which is also the first pump site.
    (CLAY, 15, 25),
    // The second metal is an expedition, not an errand.
    (COPPER_ORE, 25, 40),
];

/// How far a window is widened, per step and in total, when a seed puts nothing inside it. Past
/// the cap the world is refused rather than papered over: a preset that cannot bootstrap is the
/// failure the survey exists to make visible.
const BOOTSTRAP_WIDEN_STEP: i32 = 8;
const BOOTSTRAP_WIDEN_CAP: i32 = 40;

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
                // A hex is 1 m² and the walk is 3 m/s, so a landform of 512 hexes is a three-minute
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
                site_cell: 14,
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
                // Islands you walk across, not tiles you step over: ~130 m / 45 s at 3 m/s, still
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
                site_cell: 14,
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
                site_cell: 16,
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
                site_cell: 14,
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
fn center_on_ocean(params: &WorldParams, seed: u32, center: (i32, i32)) -> bool {
    hexes_in_radius(center, OCEAN_PROBE_RADIUS)
        .into_iter()
        .any(|(q, r)| {
            value_noise(seed, q, r, params.elevation_coarse_cell, 0xA11CE) < params.ocean_level
        })
}

/// The rules a centre is eligible for, and the pick among them. Returns an index into the rule
/// table. `None` means this cell holds no site at all, which is how barren ground stays the common
/// case.
fn eligible_rule(params: &WorldParams, seed: u32, hash: u32, center: (i32, i32)) -> Option<usize> {
    let band = terrain_at(params, seed, center.0, center.1, true);
    let richness = value_noise(
        seed,
        center.0,
        center.1,
        params.richness_cell,
        RICHNESS_OCTAVE,
    );
    let mut ocean: Option<bool> = None;
    let mut admits = |rule: &SiteRule| {
        if rule.weight == 0 || rule.terrain != band || richness <= rule.site_min {
            return false;
        }
        if rule.center_ocean {
            // Asked at most once per cell, and only for a rule that got this far.
            return *ocean.get_or_insert_with(|| center_on_ocean(params, seed, center));
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
fn natural_site(params: &WorldParams, seed: u32, cell: (i32, i32)) -> Option<Site> {
    let hash = site_hash(seed, cell);
    let center = site_center(params, hash, cell);
    let index = eligible_rule(params, seed, hash, center)?;
    let rule = &params.site_rules[index];
    let span = rule.radius_max - rule.radius_min + 1;
    Some(Site {
        center,
        rule: index,
        radius: (rule.radius_min + site_field(hash, 3) % span) as i32,
    })
}

/// Whether a site admits one hex, and how far that hex is from its centre.
///
/// `band` is passed in because every caller has just computed it and a band decision costs seven
/// elevation samples. The member test is the clipping that makes a beach a strip rather than a
/// blob and keeps a scree field against its cliffs.
fn site_covers(
    params: &WorldParams,
    seed: u32,
    site: &Site,
    q: i32,
    r: i32,
    band: Terrain,
) -> Option<i32> {
    let distance = axial_distance(site.center, (q, r));
    if distance > site.radius {
        return None;
    }
    let rule = &params.site_rules[site.rule];
    let admitted = if rule.member.is_empty() {
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
            .any(|(cell_q, cell_r)| is_water_at(params, seed, cell_q, cell_r))
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
fn bootstrap_sites(params: &WorldParams, seed: u32) -> (BootstrapTable, Vec<(ItemId, i32)>) {
    let mut claimed: BootstrapTable = BTreeMap::new();
    let mut unmet = Vec::new();
    let furthest = BOOTSTRAP_GUARANTEES
        .iter()
        .map(|&(_, _, ceiling)| ceiling)
        .max()
        .unwrap_or(0)
        + BOOTSTRAP_WIDEN_CAP;
    let span = (furthest + MAX_SITE_RADIUS as i32) / params.site_cell + 2;
    // The spiral, written as a sort rather than as a ring walk. The order has to be fixed and a
    // hand-rolled ring walk is exactly where that goes wrong; the centre distance is what makes it
    // a spiral, and the cell breaks every tie so nothing is decided by iteration order.
    let mut cells: Vec<SpiralStep> = Vec::new();
    for cell_q in -span..=span {
        for cell_r in -span..=span {
            let cell = (cell_q, cell_r);
            let center = site_center(params, site_hash(seed, cell), cell);
            cells.push((axial_distance((0, 0), center), cell, center));
        }
    }
    cells.sort_unstable();
    for &(item_id, floor, ceiling) in &BOOTSTRAP_GUARANTEES {
        let mut reach = ceiling;
        let placed = loop {
            let found = cells.iter().find_map(|&(distance, cell, center)| {
                if claimed.contains_key(&cell) {
                    return None;
                }
                let index = bootstrap_rule(params, seed, center, item_id)?;
                let site = Site {
                    center,
                    rule: index,
                    radius: params.site_rules[index].radius_max as i32,
                };
                let edge = distance - site.radius;
                if edge < floor || edge > reach {
                    return None;
                }
                (member_hexes(params, seed, &site) >= WORKABLE_PATCH_HEXES).then_some((cell, site))
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

/// The rule a guaranteed cell is forced to: the first row for this material whose band the centre
/// stands in and whose ocean gate it clears. The richness gate is deliberately *not* asked — a
/// guarantee that poor country could veto is not a guarantee.
fn bootstrap_rule(
    params: &WorldParams,
    seed: u32,
    center: (i32, i32),
    item_id: ItemId,
) -> Option<usize> {
    let band = terrain_at(params, seed, center.0, center.1, true);
    params.site_rules.iter().position(|rule| {
        rule.weight > 0
            && rule.item_id == item_id
            && rule.terrain == band
            && (!rule.center_ocean || center_on_ocean(params, seed, center))
    })
}

/// How many hexes a site actually admits once its member test has clipped the disc. A guarantee
/// that lands a highland rule on a peak with nothing around it is not a guarantee, so the
/// bootstrap pass asks this before it claims a cell.
fn member_hexes(params: &WorldParams, seed: u32, site: &Site) -> u32 {
    hexes_in_radius(site.center, site.radius)
        .into_iter()
        .filter(|&(q, r)| {
            !is_water_at(params, seed, q, r)
                && axial_distance((0, 0), (q, r)) > LANDING_CLEAR_RADIUS
                && site_covers(
                    params,
                    seed,
                    site,
                    q,
                    r,
                    terrain_at(params, seed, q, r, true),
                )
                .is_some()
        })
        .count() as u32
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
    fn new(params: &WorldParams, seed: u32) -> Self {
        let (bootstrap, unmet) = bootstrap_sites(params, seed);
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

    fn site_at(&self, cell: (i32, i32)) -> Option<Site> {
        if let Some(&site) = self.sites.borrow().get(&cell) {
            return site;
        }
        let site = self.site_uncached(cell);
        self.sites.borrow_mut().insert(cell, site);
        site
    }

    /// The same answer with the cache bypassed. The survey and the tests call the generator without
    /// a warm lattice, and one test asserts the two paths agree over a disc.
    fn site_uncached(&self, cell: (i32, i32)) -> Option<Site> {
        self.bootstrap
            .get(&cell)
            .copied()
            .or_else(|| natural_site(&self.params, self.seed, cell))
    }

    /// What the bootstrap pass actually placed, per guaranteed material: the walk from the landing
    /// site to the nearest hex of the patch, and how many hexes the patch holds once the member
    /// test has clipped it. A guarantee the pass gave up on is simply absent, which is the shape
    /// every caller wants — the survey prints it as `none` and `Core::new` refuses the world.
    fn guarantees(&self) -> Vec<(ItemId, u32, u32)> {
        self.bootstrap
            .values()
            .map(|site| {
                (
                    self.params.site_rules[site.rule].item_id,
                    (axial_distance((0, 0), site.center) - site.radius).max(0) as u32,
                    member_hexes(&self.params, self.seed, site),
                )
            })
            .collect()
    }

    fn field_at(&self, q: i32, r: i32, generated_environment: bool) -> Option<ResourceState> {
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
        if is_water_at(&self.params, self.seed, q, r) {
            return None;
        }
        let band = terrain_at(&self.params, self.seed, q, r, true);
        let cell = (
            floor_div(q, self.params.site_cell),
            floor_div(r, self.params.site_cell),
        );
        let mut best: Option<((i32, i32, i32), Site)> = None;
        for step_q in -self.reach..=self.reach {
            for step_r in -self.reach..=self.reach {
                let candidate = (cell.0 + step_q, cell.1 + step_r);
                let Some(site) = self.site_at(candidate) else {
                    continue;
                };
                let Some(distance) = site_covers(&self.params, self.seed, &site, q, r, band) else {
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
    /// Lines sit three rows apart so one line's two-cell composer cannot touch the next.
    const ROW_PITCH: i32 = 3;
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
            // with cargo actually moving.
            warmup_ticks: 40,
            measured_ticks,
            frames,
            snapshots,
            edits,
        }
    }

    fn catalogs() -> (DefinitionsInput, TechnologiesInput) {
        (
            serde_json::from_str(DEFINITIONS).expect("shipped definitions parse"),
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
            for q in 1..=spec.belt_span as i32 {
                buildings.push(placed(q, r, BELT, None));
            }
            let composer_q = spec.belt_span as i32 + 1;
            buildings.push(placed(composer_q, r, COMPOSER, Some(COMPONENT_RECIPE)));
            buildings.push(placed(composer_q + 1, r, BELT, None));
            buildings.push(placed(composer_q + 2, r, CONTAINER, None));
            buildings.push(placed(composer_q + 3, r, BELT, None));
            buildings.push(placed(composer_q + 4, r, CONSUMER, None));
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
        let (definitions, technologies) = catalogs();
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
        let (definitions, technologies) = catalogs();
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
                core.rotate(1, edit_row(spec, edit))
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
                    .entity_at(1, edit_row(spec, edit))
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
        let fields = WorldFields::new(params, seed);
        let cells: Vec<(i32, i32)> = disc(radius);
        let mut bands: BTreeMap<Terrain, u32> = BTreeMap::new();
        let mut terrain_of: BTreeMap<(i32, i32), Terrain> = BTreeMap::new();
        let mut river_cells: BTreeSet<(i32, i32)> = BTreeSet::new();
        let mut land_hexes = 0u32;
        let mut found: BTreeMap<ItemId, (u32, u32, u32)> = BTreeMap::new();
        let mut field_of: BTreeMap<(i32, i32), (ItemId, u32)> = BTreeMap::new();
        for &(q, r) in &cells {
            let terrain = terrain_at(params, seed, q, r, true);
            terrain_of.insert((q, r), terrain);
            *bands.entry(terrain).or_default() += 1;
            if !terrain.is_water() {
                land_hexes += 1;
            } else if is_survey_river(params, seed, q, r) {
                river_cells.insert((q, r));
            }
            if let Some((item_id, quantity)) = surveyed_field(&fields, q, r) {
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
        let (totals, pure_hexes) = patch_shape(&fields, &field_of, &body_of, radius);
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
        for &item_id in &[IRON_ORE, CRYSTAL, COPPER_ORE, COAL, STONE, SAND, CLAY, WOOD] {
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
            bootstrap: bootstrap_rows(&fields, &name_of),
        }
    }

    /// What the survey counts as a generated cell. The clearing is a promise, not geography, so it
    /// is no evidence about what a parameter set generates — `field_at` already suppresses it, and
    /// the guaranteed opening is reported on its own in `bootstrap`.
    fn surveyed_field(fields: &WorldFields, q: i32, r: i32) -> Option<(ItemId, u32)> {
        fields
            .field_at(q, r, true)
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
        name_of: &dyn Fn(ItemId) -> String,
    ) -> Vec<BootstrapRow> {
        let placed: BTreeMap<ItemId, (u32, u32)> = fields
            .guarantees()
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
                surveyed_field(fields, q + dq, r + dr).is_some_and(|(other, _)| other != item_id)
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
mod tests {
    use super::*;

    const DEFINITIONS: &str = include_str!("../../src/data/definitions.json");
    const TECHNOLOGIES: &str = include_str!("../../src/data/technologies.json");
    const SCENARIOS: &str = include_str!("../../src/data/scenarios.json");

    fn catalogs() -> (DefinitionsInput, TechnologiesInput, ScenariosInput) {
        let definitions = serde_json::from_str(DEFINITIONS).unwrap();
        let technologies = serde_json::from_str(TECHNOLOGIES).unwrap();
        let scenarios = serde_json::from_str(SCENARIOS).unwrap();
        validate_all(&definitions, &technologies, &scenarios).unwrap();
        (definitions, technologies, scenarios)
    }

    /// The bounded idle batch the host sends on a frame with no held key.
    const IDLE: &str = r#"[{"type":"move_intent","x":0,"y":0}]"#;
    /// The bounded batch the host sends on a frame with the east movement key held.
    const IDLE_MOVE_EAST: &str = r#"[{"type":"move_intent","x":1000,"y":0}]"#;

    fn test_factory(key: &str) -> Factory {
        let (definitions, technologies, scenarios) = catalogs();
        let scenario = scenarios
            .scenarios
            .iter()
            .find(|value| value.key == key)
            .unwrap()
            .clone();
        let core = Core::new(&definitions, &technologies, &scenario, None, None).unwrap();
        Factory {
            definitions,
            technologies,
            scenarios,
            core,
            snapshot_revision: 0,
            baseline: None,
        }
    }

    /// Assert that the delta the shipped builder produces from its dirty marks is byte-identical to
    /// the one a full diff of two complete snapshots would have produced, then advance the oracle.
    fn assert_delta_matches_full_diff(factory: &mut Factory, previous: &mut Snapshot, step: &str) {
        let current = factory.core.snapshot();
        let base_revision = factory.snapshot_revision;
        let oracle = SnapshotDelta::between(base_revision, base_revision + 1, previous, &current);
        let actual = factory.build_delta();
        assert_eq!(
            serde_json::to_string(&actual).unwrap(),
            serde_json::to_string(&oracle).unwrap(),
            "dirty-tracked delta diverged from the full snapshot diff after {step}"
        );
        // The binary wire has to carry exactly this object and nothing less. Round-tripping here
        // rather than in a test of its own means every delta this run produces is covered — a full
        // replace, an incremental patch, a removal list, terrain arriving, a deposit running dry,
        // a fuelled machine mid-craft — which is the entity and group variety a hand-written
        // fixture cannot enumerate. `fixtures/snapshot-delta-wire.json` pins the other half: that
        // the TypeScript decoder reads the same bytes the same way.
        assert_eq!(
            wire::decode::decode_delta(&wire::encode_delta(&actual)),
            actual,
            "binary wire round trip lost part of the delta after {step}"
        );
        *previous = current;
    }

    #[test]
    fn a_pole_and_burner_run_an_extractor_and_a_dark_one_does_not() {
        let mut dark = game("new-game");
        dark.power_unmetered = false;
        dark.researched.extend([1, 2, 8]);
        dark.player.inventory.insert(1, 20);
        dark.player.inventory.insert(3, 4);
        dark.player.inventory.insert(6, 8);
        dark.player.inventory.insert(5, 8);
        set_player_hex(&mut dark, 1, 0);
        dark.place(3, 0, 1, 0, None).unwrap();
        dark.tick_many(20);
        let extractor = dark
            .entities
            .iter()
            .find(|entity| entity.kind == BuildingKind::Extractor)
            .unwrap();
        assert!(extractor.cargo.is_none());
        assert_eq!(
            dark.entity_snapshot(
                dark.entities
                    .iter()
                    .position(|entity| entity.kind == BuildingKind::Extractor)
                    .unwrap()
            )
            .status,
            EntityStatus::NoPower
        );

        let mut lit = game("new-game");
        lit.power_unmetered = false;
        lit.researched.extend([1, 2, 8]);
        lit.player.inventory.insert(1, 20);
        lit.player.inventory.insert(3, 4);
        lit.player.inventory.insert(6, 8);
        lit.player.inventory.insert(5, 8);
        set_player_hex(&mut lit, 1, 0);
        lit.place(3, 0, 1, 0, None).unwrap();
        let pole = try_place_near(&mut lit, (3, 0), 12);
        try_place_near(&mut lit, pole, 13);
        let burner = lit
            .entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Generator)
            .unwrap();
        lit.entities[burner].inventory.insert(5, 8);
        lit.tick_many(20);
        let extractor = lit
            .entities
            .iter()
            .find(|entity| entity.kind == BuildingKind::Extractor)
            .unwrap();
        assert!(extractor.cargo.is_some() || extractor.progress > 0);
        let snapshot = lit.entity_snapshot(burner);
        assert_eq!(snapshot.status, EntityStatus::Generating);
        assert!(snapshot.power_satisfied > 0);
        // The extractor has produced and nobody has taken it: it is blocked, not waiting on power,
        // so it is not on the meter. Before v0.19 it would still have been booking its full draw
        // and costing the burner a unit of coal a tick for work it could not do.
        assert!(lit.entities[extractor_index(&lit)].cargo.is_some());
        assert_eq!(snapshot.power_demand, 0);
    }

    /// The other half of the same rule, and the one the player pays for: a plant carrying a small
    /// load burns proportionally less fuel, and a plant carrying none burns none at all.
    #[test]
    fn a_generator_burns_for_the_work_it_powers_and_not_for_the_clock() {
        let coal = |core: &Core, index: usize| {
            let entity = &core.entities[index];
            u32::from(entity.inventory.get(&5).copied().unwrap_or(0)) * 8 + entity.fuel_charge
        };

        // One burner, one pole, one extractor with a deposit to work.
        let mut working = game("new-game");
        working.power_unmetered = false;
        working.researched.extend([1, 2, 8]);
        working.player.inventory.insert(1, 20);
        working.player.inventory.insert(3, 4);
        working.player.inventory.insert(6, 8);
        working.player.inventory.insert(5, 8);
        set_player_hex(&mut working, 1, 0);
        working.place(3, 0, 1, 0, None).unwrap();
        let pole = try_place_near(&mut working, (3, 0), 12);
        try_place_near(&mut working, pole, 13);
        let burner = working
            .entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Generator)
            .unwrap();
        working.entities[burner].inventory.insert(5, 20);
        let before = coal(&working, burner);
        working.tick_many(40);
        let spent_working = before - coal(&working, burner);
        let output = working
            .building_definition(working.entities[burner].placed.definition_id)
            .unwrap()
            .power_output
            .unwrap();
        let taken = grid_energy_received(&working);
        let owed = working.entities[burner].burn_progress;

        // The same grid with nothing on it: a generator wired to a pole and no machine at all.
        let mut idle = game("new-game");
        idle.power_unmetered = false;
        idle.researched.extend([1, 2, 8]);
        idle.player.inventory.insert(1, 20);
        idle.player.inventory.insert(3, 4);
        idle.player.inventory.insert(6, 8);
        idle.player.inventory.insert(5, 8);
        set_player_hex(&mut idle, 1, 0);
        let pole = try_place_near(&mut idle, (3, 0), 12);
        try_place_near(&mut idle, pole, 13);
        let idle_burner = idle
            .entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Generator)
            .unwrap();
        idle.entities[idle_burner].inventory.insert(5, 20);
        let before = coal(&idle, idle_burner);
        idle.tick_many(40);

        assert_eq!(
            before - coal(&idle, idle_burner),
            0,
            "a plant with nothing to power burns nothing"
        );
        assert!(spent_working > 0, "a plant doing work burns something");
        // The bill is the load, exactly. Fuel spent buys `output` units of electricity each, and
        // the part-unit still owed sits in `burn_progress` — so this equality is the whole rule,
        // and it is what fails the moment a plant burns for the clock instead of for the work.
        assert_eq!(
            spent_working * output + owed,
            taken,
            "a plant burns for what it handed over"
        );
        // And what one extractor asks for over forty ticks is nowhere near what this plant could
        // have made in them. The old rule charged a unit of fuel energy per tick regardless —
        // forty — where this is a handful.
        assert!(
            spent_working * 4 < 40,
            "one extractor must not cost a burner its full output: spent {spent_working}"
        );
    }

    /// Coverage belongs to the pole, and it is the whole of the upgrade.
    ///
    /// The same machine at the same hex is dark under a base pole and lit under a relay pole,
    /// with nothing else in the world changed. Before v0.19 this test could not have been written:
    /// the distance came off the machine, so every pole in the game reached exactly as far as
    /// every other one and no upgrade could move it.
    #[test]
    fn a_better_pole_lights_a_wider_disc_and_the_machine_does_not_change() {
        let base = building_by_key("pole");
        let relay = building_by_key("pole-ii");
        let trunk = building_by_key("pole-iii");
        assert_eq!(base.supply_radius, Some(3));
        assert_eq!(relay.supply_radius, Some(4));
        assert_eq!(trunk.supply_radius, Some(6));
        // The ladder is a chain, so an upgrade never skips a rung or turns a pole into a machine.
        assert_eq!(base.upgrades_to, Some(relay.id));
        assert_eq!(relay.upgrades_to, Some(trunk.id));

        // A pole and a machine exactly four hexes apart: outside a base pole, inside a relay.
        for (definition_id, expected) in [(base.id, false), (relay.id, true)] {
            let mut core = game("new-game");
            core.power_unmetered = false;
            core.researched.extend([1, 2, 8, 5, 13]);
            core.player.inventory.insert(1, 40);
            core.player.inventory.insert(3, 8);
            core.player.inventory.insert(6, 20);
            core.player.inventory.insert(11, 8);
            core.player.inventory.insert(18, 8);
            core.player.build_range = 1 << 20;
            set_player_hex(&mut core, 0, 0);
            core.place(3, 0, 1, 0, None).unwrap();
            let extractor = extractor_index(&core);
            core.place(3 + 4, 0, definition_id, 0, None).unwrap();
            let pole = core
                .entities
                .iter()
                .position(|entity| entity.kind == BuildingKind::Pole)
                .unwrap();
            // An unconnected machine sits on a network of its own, so what says "covered" is
            // sharing the pole's network rather than merely having one.
            assert_eq!(
                core.power_of[extractor] == core.power_of[pole],
                expected,
                "a pole with radius {:?} four hexes away",
                core.building_definition(definition_id)
                    .unwrap()
                    .supply_radius
            );
        }
    }

    /// Machines that touch conduct, and only the ones that carry current do.
    ///
    /// This is what makes a pole cost *distance* rather than power, and it is what
    /// `fixtures/balance.json` has priced openings against since v0.18 — one generator, no pole,
    /// for a machine standing beside it. Until v0.19 that price was simply wrong.
    #[test]
    fn a_generator_powers_what_stands_against_it_and_a_belt_carries_nothing() {
        let mut core = game("new-game");
        core.power_unmetered = false;
        core.researched.extend([1, 2, 8]);
        core.player.inventory.insert(1, 40);
        core.player.inventory.insert(3, 8);
        core.player.inventory.insert(6, 20);
        core.player.inventory.insert(5, 20);
        core.player.build_range = 1 << 20;
        set_player_hex(&mut core, 0, 0);
        core.place(3, 0, 1, 0, None).unwrap();
        let extractor = extractor_index(&core);

        // No pole anywhere: the generator is simply built against the extractor's footprint.
        let mut placed = None;
        for &(dq, dr) in &DIRECTIONS {
            if core.place(3 + dq, dr, 13, 0, None).is_ok() {
                placed = Some((3 + dq, dr));
                break;
            }
        }
        placed.expect("a burner fits beside the extractor");
        let generator = core
            .entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Generator)
            .unwrap();
        // Sharing the generator's network, not merely having one: an unconnected machine is put on
        // a network of its own, so `is_some` is true of every machine ever built and proves nothing.
        assert_eq!(
            core.power_of[extractor], core.power_of[generator],
            "a machine touching a generator is on its network"
        );
        assert!(
            core.entities
                .iter()
                .all(|entity| entity.kind != BuildingKind::Pole),
            "and it got there without a pole"
        );

        // A belt is not wire. One built hard against the pair still joins no network, so a line of
        // the cheapest building in the game cannot carry current across the map and no player ever
        // stops placing the second pole.
        let belt = try_place_near(&mut core, (3, 0), 2);
        let belt_index = core
            .entities
            .iter()
            .position(|entity| entity.placed.q == belt.0 && entity.placed.r == belt.1)
            .unwrap();
        assert_eq!(axial_distance((3, 0), belt), 1, "the belt is touching");
        assert!(core.power_of[belt_index].is_none());
    }

    /// A scarce grid feeds the machine that can work, not the one that is holding an output.
    ///
    /// The gate that makes this true is also the whole of the fuel rule, and under a full grid the
    /// two are indistinguishable — a blocked machine with a full bank asks for nothing either way.
    /// It takes scarcity and an empty bank to tell them apart, which is exactly the state a player
    /// is in when they are wondering why the factory got slow.
    #[test]
    fn a_blocked_machine_does_not_take_a_share_of_a_grid_it_cannot_use() {
        let mut core = game("new-game");
        core.power_unmetered = false;
        core.researched.extend([1, 2, 8]);
        core.player.inventory.insert(1, 60);
        core.player.inventory.insert(3, 12);
        core.player.inventory.insert(6, 20);
        core.player.inventory.insert(5, 40);
        core.player.build_range = 1 << 20;
        set_player_hex(&mut core, 0, 0);
        core.place(3, 0, 1, 0, None).unwrap();
        let first = extractor_index(&core);
        try_place_near(&mut core, (3, 0), 1);
        let second = core
            .entities
            .iter()
            .enumerate()
            .filter(|(index, entity)| *index != first && entity.kind == BuildingKind::Extractor)
            .map(|(index, _)| index)
            .next()
            .expect("a second extractor");
        let pole = try_place_near(&mut core, (3, 0), 12);
        try_place_near(&mut core, pole, 13);
        let burner = core
            .entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Generator)
            .unwrap();
        core.entities[burner].inventory.insert(5, 40);
        core.tick_many(2);
        assert_eq!(core.power_of[first], core.power_of[second]);

        // Both banks empty, and both machines holding an output nobody has taken. The only
        // difference between them is the one we are about to make.
        for index in [first, second] {
            core.entities[index].power_charge = 0;
            core.entities[index].cargo = Some(Cargo {
                item_id: 1,
                quantity: 1,
            });
        }
        // A belt takes the first one's output. Now it has work and the other still does not.
        core.entities[first].cargo = None;
        core.tick_many(1);

        assert!(
            core.entities[first].power_charge > 0,
            "the machine that can work was given power"
        );
        assert_eq!(
            core.entities[second].power_charge, 0,
            "the machine holding an output took none of it"
        );
    }
    /// Electricity is conserved: what the machines banked is what the plants produced, to the unit.
    ///
    /// The reason throughput comes out exactly proportional to generation with no slowdown factor
    /// anywhere. An undersupplied factory is not scaled down — it is handed less to spend.
    #[test]
    fn every_unit_a_plant_produced_is_a_unit_a_machine_banked() {
        let mut core = game("new-game");
        core.power_unmetered = false;
        core.researched.extend([1, 2, 8]);
        core.player.inventory.insert(1, 60);
        core.player.inventory.insert(3, 12);
        core.player.inventory.insert(6, 20);
        core.player.inventory.insert(5, 40);
        core.player.build_range = 1 << 20;
        set_player_hex(&mut core, 0, 0);
        core.place(3, 0, 1, 0, None).unwrap();
        let pole = try_place_near(&mut core, (3, 0), 12);
        try_place_near(&mut core, pole, 13);
        let burner = core
            .entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Generator)
            .unwrap();
        core.entities[burner].inventory.insert(5, 40);

        let received_before = grid_energy_received(&core);
        let plant_energy_before = core.entities[burner].fuel_charge
            + core.entities[burner]
                .inventory
                .get(&5)
                .copied()
                .unwrap_or(0)
                * 8;
        core.tick_many(30);
        let received_after = grid_energy_received(&core);
        let plant_energy_after = core.entities[burner].fuel_charge
            + core.entities[burner]
                .inventory
                .get(&5)
                .copied()
                .unwrap_or(0)
                * 8;

        // Fuel energy spent, times the exchange rate, is grid energy produced. That grid energy
        // either sits in a bank or has already been turned into progress, and it is never anything
        // else — there is no third place for a unit of electricity to go.
        let output = core
            .building_definition(core.entities[burner].placed.definition_id)
            .unwrap()
            .power_output
            .unwrap();
        let produced = (plant_energy_before - plant_energy_after) * output
            + core.entities[burner].burn_progress;
        assert!(produced > 0, "the plant produced something");
        assert_eq!(
            produced,
            received_after - received_before,
            "the plant produced {produced} and the machines received {}",
            received_after - received_before
        );
    }

    /// Every unit of electricity the grid has handed to a machine, wherever it now sits.
    ///
    /// Three places and only three: still banked, already turned into progress, or turned into a
    /// finished thing. A machine's `progress` resets when it produces, so the last of those has to
    /// be counted from the cargo or conservation would read as a leak every time something came
    /// out of a machine.
    fn grid_energy_received(core: &Core) -> u32 {
        core.entities
            .iter()
            .map(|entity| {
                let Some(definition) = core.building_definition(entity.placed.definition_id) else {
                    return entity.power_charge;
                };
                let draw = definition.power_draw.unwrap_or(0);
                let finished = match (entity.cargo, definition.cadence) {
                    (Some(_), Some(cycle)) => cycle * draw,
                    _ => 0,
                };
                entity.power_charge + entity.progress * draw + finished
            })
            .sum()
    }

    /// The split that makes allocation exact without storing a remainder on every entity.
    #[test]
    fn apportioning_hands_out_every_unit_and_no_more() {
        for total in [0u64, 1, 7, 20, 1000] {
            for weights in [
                vec![1u64, 1, 1],
                vec![64, 20, 20],
                vec![1, 999],
                vec![5],
                vec![],
            ] {
                let parts = apportion(total, &weights);
                assert_eq!(parts.len(), weights.len());
                let handed: u64 = parts.iter().sum();
                let cap: u64 = weights.iter().sum();
                assert_eq!(handed, total.min(if cap == 0 { 0 } else { total }));
                // Nobody is given more than the whole, and the split follows the weights.
                for (part, weight) in parts.iter().zip(&weights) {
                    if *weight == 0 {
                        assert_eq!(*part, 0);
                    }
                }
            }
        }
    }

    fn building_by_key(key: &str) -> BuildingDefinition {
        let (definitions, _, _) = catalogs();
        definitions
            .buildings
            .iter()
            .find(|building| building.key == key)
            .unwrap_or_else(|| panic!("building {key} exists"))
            .clone()
    }
    fn extractor_index(core: &Core) -> usize {
        core.entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Extractor)
            .unwrap()
    }

    /// The cells the mechanics suite draws from, and the reason they are a fixture.
    ///
    /// These are the eight the landing clearing used to guarantee. They stopped being geography in
    /// v0.21 — the generator places the opening outside the clearing now, and inside it there is
    /// nothing at all — but a test about belts, power, gathering reach, or an upgrade wants a
    /// deposit at a hex it can *name*. Standing those on generated ground would turn every one of
    /// them into a test about the generator, and a tuning pass would break forty tests that are
    /// not about tuning. Writing them into the overlay is exactly what "only the overlay is state"
    /// already means, and it is what a scenario file does for a hand-authored map.
    ///
    /// Stone sits on the cliff at `(1, -1)`, which nothing can stand on: it is taken from the hex
    /// beside it, and that is what `extraction_reach_comes_from_the_definition` is checking.
    const TEST_FIELD: [(i32, i32, ItemId, u32); 8] = [
        (3, 0, IRON_ORE, 48),
        (-2, 2, CRYSTAL, 32),
        (0, -3, COPPER_ORE, 40),
        (2, -3, COAL, 28),
        (1, -1, STONE, 40),
        (1, 3, SAND, 30),
        (-1, 3, CLAY, 26),
        (-3, 1, WOOD, 14),
    ];

    fn bare_game(key: &str) -> Core {
        let (definitions, technologies, scenarios) = catalogs();
        let scenario = scenarios
            .scenarios
            .iter()
            .find(|value| value.key == key)
            .unwrap();
        let mut core = Core::new(&definitions, &technologies, scenario, None, None).unwrap();
        // Isolated machine tests are not the power suite. They opt into the constraint.
        core.power_unmetered = true;
        core
    }

    fn game(key: &str) -> Core {
        let mut core = bare_game(key);
        for &(q, r, item_id, quantity) in &TEST_FIELD {
            core.write_overlay(q, r, item_id, quantity, quantity);
        }
        core.dirty = SnapshotDirty::default();
        core
    }

    /// Wait out a gather cooldown the way a player does — on their own clock, with the factory
    /// untouched. Drains whatever the last gather actually cost, so a coal seam and a wood cell
    /// share one helper.
    fn cooldown(core: &mut Core) {
        let remaining = core.player.action_cooldown.max(1);
        core.advance_player_steps(remaining);
    }

    fn set_player_hex(core: &mut Core, q: i32, r: i32) {
        (core.player.x, core.player.y) = axial_world(q, r);
        core.ensure_neighborhood(core.player.x, core.player.y);
    }

    fn try_place_near(
        core: &mut Core,
        origin: (i32, i32),
        definition_id: DefinitionId,
    ) -> (i32, i32) {
        for radius in 1..=6 {
            for dq in -radius..=radius {
                for dr in -radius..=radius {
                    if axial_distance((0, 0), (dq, dr)) != radius {
                        continue;
                    }
                    let q = origin.0 + dq;
                    let r = origin.1 + dr;
                    if core.place(q, r, definition_id, 0, None).is_ok() {
                        return (q, r);
                    }
                }
            }
        }
        panic!("no legal site for definition {definition_id} near {origin:?}");
    }

    fn add_test_belt(core: &mut Core, q: i32, r: i32, orientation: u8) -> u32 {
        let id = core.next_entity_id;
        core.next_entity_id += 1;
        core.entities.push(Entity {
            id,
            placed: PlacedBuilding {
                q,
                r,
                definition_id: 2,
                orientation,
                recipe_id: None,
                scenario_owned: false,
            },
            kind: BuildingKind::Belt,
            cargo: None,
            inventory: BTreeMap::new(),
            reserved_inputs: BTreeMap::new(),
            progress: 0,
            fuel_charge: 0,
            power_charge: 0,
            burn_progress: 0,
        });
        id
    }

    #[test]
    fn public_direction_protocol_matches_cross_language_fixture() {
        let fixture: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../../fixtures/hex-directions.json")).unwrap();
        let actual: Vec<(i32, i32)> = fixture
            .iter()
            .map(|entry| {
                (
                    entry["q"].as_i64().unwrap() as i32,
                    entry["r"].as_i64().unwrap() as i32,
                )
            })
            .collect();
        assert_eq!(actual, TRANSPORT_DIRECTIONS);
    }

    /// Which bands the player cannot stand on is native's rule, and since v0.12.3 the renderer
    /// draws that category before it draws the material — so the host holds a copy of the rule and
    /// a copy is a thing that drifts. This is the `fixtures/hex-directions.json` idiom applied to
    /// it: Rust asserts the file against the predicates, `tests/host.test.ts` asserts it against
    /// `src/core/terrain.ts`, and neither side may move without the other.
    #[test]
    fn terrain_passability_matches_the_cross_language_fixture() {
        #[derive(Deserialize)]
        struct PassabilityEntry {
            terrain: Terrain,
            passable: bool,
            buildable: bool,
        }

        const BANDS: [Terrain; 7] = [
            Terrain::DeepWater,
            Terrain::ShallowWater,
            Terrain::Shore,
            Terrain::Lowland,
            Terrain::Hills,
            Terrain::Highland,
            Terrain::Cliff,
        ];
        // A band added to the enum makes this match non-exhaustive, which is what sends whoever
        // added it to `BANDS` above and to the fixture beside it.
        for band in BANDS {
            match band {
                Terrain::DeepWater
                | Terrain::ShallowWater
                | Terrain::Shore
                | Terrain::Lowland
                | Terrain::Hills
                | Terrain::Highland
                | Terrain::Cliff => {}
            }
        }

        let fixture: Vec<PassabilityEntry> =
            serde_json::from_str(include_str!("../../fixtures/terrain-passability.json")).unwrap();
        assert_eq!(fixture.len(), BANDS.len(), "a band has no fixture entry");
        for (entry, band) in fixture.iter().zip(BANDS) {
            assert_eq!(entry.terrain, band, "fixture is in declaration order");
            assert_eq!(
                entry.passable,
                !band.blocks_movement(),
                "{band:?} passability disagrees with the fixture"
            );
            assert_eq!(
                entry.buildable,
                !band.blocks_construction(),
                "{band:?} buildability disagrees with the fixture"
            );
        }
    }

    #[test]
    fn chunk_generation_is_order_independent_and_seeded() {
        let mut a = game("new-game");
        let mut b = game("new-game");
        a.generate_chunk(8, -4);
        a.generate_chunk(-6, 3);
        b.generate_chunk(-6, 3);
        b.generate_chunk(8, -4);
        assert_eq!(a.checksum(), b.checksum());
        assert_eq!(coordinate_hash(1213486160, 81, -33), 166_969_415);
        assert_ne!(
            coordinate_hash(1213486160, 81, -33),
            coordinate_hash(1213486161, 81, -33)
        );
        // The site lattice is a cache, and a cache is exactly where order-dependence gets into a
        // generator: `a` walked one chunk first and `b` the other, so their lattices were filled
        // in different orders. Every cell they both hold has to agree, and the cached answer has
        // to be the uncached one — the two halves of "derived state, and derived from what".
        for (&cell, &site) in a.fields.sites.borrow().iter() {
            assert_eq!(site, a.fields.site_uncached(cell));
            if let Some(&other) = b.fields.sites.borrow().get(&cell) {
                assert_eq!(site, other);
            }
        }
    }

    /// The cache pays for the site model and must not change it. `field_at` is asked over a disc
    /// wide enough to cross many lattice cells, warm and cold, and the two must never disagree.
    #[test]
    fn the_site_cache_answers_exactly_what_the_uncached_generator_does() {
        let params = preset_params("continental").unwrap();
        let seed = survey::default_seed();
        let warm = WorldFields::new(&params, seed);
        for (q, r) in hexes_in_radius((14, -9), 24) {
            let cold = WorldFields::new(&params, seed);
            assert_eq!(
                warm.field_at(q, r, true),
                cold.field_at(q, r, true),
                "the cache changed the world at {q},{r}"
            );
            let cell = (
                floor_div(q, params.site_cell),
                floor_div(r, params.site_cell),
            );
            assert_eq!(warm.site_at(cell), warm.site_uncached(cell));
        }
        // And the cheap water test the fast path opens with agrees with the band decision it
        // skips, clearing included. If it ever did not, `field_at` would drop deposits silently.
        for (q, r) in hexes_in_radius((0, 0), 40) {
            assert_eq!(
                is_water_at(&params, seed, q, r),
                terrain_at(&params, seed, q, r, true).is_water(),
                "the cheap water test disagrees at {q},{r}"
            );
        }
    }

    #[test]
    fn world_to_axial_inverts_axial_world_and_rounds_to_the_nearest_hex() {
        for q in -12..=12 {
            for r in -12..=12 {
                let (x, y) = axial_world(q, r);
                assert_eq!(world_to_axial(x, y), (q, r));
            }
        }
        let (x, y) = axial_world(3, -2);
        assert_eq!(world_to_axial(x + 200, y - 150), (3, -2));
    }

    #[test]
    fn generated_fields_follow_terrain_and_only_the_overlay_is_state() {
        // The one test that must see an untouched world: the claim is that an unmined field costs
        // nothing stored, and a fixture that pre-writes eight tiles would answer it in advance.
        let mut core = bare_game("new-game");
        assert_eq!(core.terrain_at(0, 0), Terrain::Lowland);
        assert_eq!(core.terrain_at(2, 1), Terrain::ShallowWater);
        assert_eq!(core.terrain_at(1, -1), Terrain::Cliff);
        // The clearing holds no field at all now: the eight hardcoded cells it used to carry were
        // a sample platter, and the opening is placed by the generator outside it.
        for cell in hexes_in_radius((0, 0), LANDING_CLEAR_RADIUS) {
            assert_eq!(core.field_at(cell.0, cell.1), None);
        }
        let cell = *core
            .fields
            .bootstrap
            .values()
            .map(|site| site.center)
            .min()
            .as_ref()
            .expect("a new world guarantees an opening");
        let quantity = core
            .field_at(cell.0, cell.1)
            .expect("a site centre")
            .quantity;
        assert!(quantity > 0);
        assert_eq!(core.deposit_quantity(cell), quantity);
        // Unmined field is derived: the overlay is empty until something is taken, but the
        // snapshot still reports the cell so the host can draw it.
        assert!(core.tiles.is_empty());
        core.ensure_neighborhood(axial_world(cell.0, cell.1).0, axial_world(cell.0, cell.1).1);
        assert!(core
            .resource_snapshots()
            .iter()
            .any(|resource| resource.q == cell.0
                && resource.r == cell.1
                && resource.quantity == quantity));
        let before = core.checksum();
        set_player_hex(&mut core, cell.0, cell.1);
        core.gather().unwrap();
        assert_eq!(core.deposit_quantity(cell), quantity - 1);
        assert_eq!(
            core.tiles[&cell].resource.as_ref().unwrap().quantity,
            quantity - 1
        );
        assert_ne!(core.checksum(), before);
    }

    #[test]
    fn an_extractor_harvests_every_field_cell_inside_its_radius() {
        let mut core = game("new-game");
        core.researched.insert(2);
        core.player.inventory.insert(1, 8);
        core.player.inventory.insert(6, 2);
        set_player_hex(&mut core, 3, 1);
        // Two ore cells one step apart, written into the overlay because the clearing generates
        // none: this is a test about which cell inside a reach is drawn from first, and standing
        // it on geography would make it a test about geography.
        core.write_overlay(3, 0, 1, 48, 48);
        core.write_overlay(4, 0, 1, 3, 3);
        core.place(3, 0, 1, 0, None).unwrap();
        let index = core.entity_at(3, 0).unwrap();
        let candidates = core.deposit_candidates(3, 0, EXTRACT_RADIUS);
        assert_eq!(candidates[0], (3, 0));
        assert!(candidates.contains(&(4, 0)));
        assert_eq!(core.extractor_deposit(index), Some((3, 0)));
        core.write_overlay(3, 0, 1, 0, 48);
        assert_eq!(core.extractor_deposit(index), Some((4, 0)));
    }

    /// Geography is still the material map. A deposit is a site rather than a per-hex decision now,
    /// so what a band holds is the set of rules that may *reach* into it — the member table — and
    /// this asserts that set exactly, band by band.
    #[test]
    fn every_material_is_generated_where_its_geography_says_it_should_be() {
        let core = game("new-game");
        // Stone is quarried from a cliff, which nothing can stand on or build on. It is reached
        // from the hex beside it, through the same radius an extractor uses — the v0.11 lesson,
        // which survives the model change because cliffs are still members of a scree field.
        assert_eq!(core.terrain_at(1, -1), Terrain::Cliff);
        assert!(core.terrain_at(1, -1).blocks_construction());

        let mut seen: BTreeMap<Terrain, BTreeSet<ItemId>> = BTreeMap::new();
        let mut land = 0u32;
        let mut fields = 0u32;
        for q in -80..80 {
            for r in -80..80 {
                // The clearing is deliberately not geography, so it is not evidence about which
                // band holds what.
                if axial_distance((0, 0), (q, r)) <= LANDING_CLEAR_RADIUS {
                    continue;
                }
                let terrain = terrain_at(&core.world_params, core.seed, q, r, true);
                if !terrain.is_water() {
                    land += 1;
                }
                if let Some(field) = core.fields.field_at(q, r, true) {
                    fields += 1;
                    seen.entry(terrain).or_default().insert(field.item_id);
                }
            }
        }
        // A field is a place. Barren ground has to be the common case, or the landscape is a
        // carpet and a site is stumbled over rather than chosen. The floor keeps a weight change
        // from emptying a band by accident.
        assert!(land > 0);
        assert!(
            fields * 100 < land * 22,
            "fields too dense: {fields} of {land} land hexes"
        );
        assert!(
            fields * 100 > land * 3,
            "fields too sparse: {fields} of {land} land hexes"
        );
        // Iron and coal share the tops and the ground below them, copper never climbs, stone hugs
        // its cliffs, clay follows water across two bands, and sand is clipped to the coast.
        assert_eq!(seen.get(&Terrain::Cliff), Some(&BTreeSet::from([STONE])));
        let shore = seen.get(&Terrain::Shore).expect("the opening has a shore");
        assert!(
            shore.contains(&CLAY),
            "clay follows water onto the shore, saw {shore:?}"
        );
        // Sand is clipped to the regional ocean. A 160-hex window of a 512-hex landform often
        // never reaches a coast, so the shore here may be clay alone.
        assert!(
            shore.is_subset(&BTreeSet::from([SAND, CLAY])),
            "the shore holds {shore:?}"
        );
        assert_eq!(
            seen.get(&Terrain::Hills),
            Some(&BTreeSet::from([IRON_ORE, COPPER_ORE, COAL]))
        );
        assert_eq!(
            seen.get(&Terrain::Highland),
            Some(&BTreeSet::from([IRON_ORE, COAL, STONE, CRYSTAL]))
        );
        assert_eq!(
            seen.get(&Terrain::Lowland),
            Some(&BTreeSet::from([WOOD, CLAY]))
        );
        // Water is pumped, not mined, which is why a basin can never be emptied. `validate` refuses
        // a rule that names a water band, and this is that refusal seen from the world.
        assert!(!seen.contains_key(&Terrain::DeepWater));
        assert!(!seen.contains_key(&Terrain::ShallowWater));
    }

    /// The seed is no longer the only thing a world can differ by. Two parameter sets on the same
    /// seed have to be different *landforms*, not the same landform with the cuts moved.
    #[test]
    fn two_parameter_sets_on_one_seed_are_different_landforms() {
        let seed = survey::default_seed();
        let continental = preset_params("continental").unwrap();
        let basin = preset_params("basin").unwrap();
        // The landing disc is an opening, not a landform: both presets fade toward the same
        // local blend there. The claim is about the world beyond it.
        let inner = landing_radius(&continental).max(landing_radius(&basin)) + 8;
        let outer = inner + 48;
        let mut differing = 0u32;
        let mut hexes = 0u32;
        for q in -outer..outer {
            for r in -outer..outer {
                let distance = axial_distance((0, 0), (q, r));
                if distance <= inner || distance > outer {
                    continue;
                }
                hexes += 1;
                if terrain_at(&continental, seed, q, r, true)
                    != terrain_at(&basin, seed, q, r, true)
                {
                    differing += 1;
                }
            }
        }
        assert!(
            differing * 100 > hexes * 60,
            "only {differing} of {hexes} hexes differ between two parameter sets"
        );
    }

    /// The claim this milestone rests on, asserted directly rather than argued from the numbers.
    ///
    /// **Feature scale decides how big water is; sea level decides how much of it there is.** The
    /// two halves below each hold one of them fixed, and the measurement that separates them is the
    /// number of bodies, not the size of the largest — whether one landform in a sample happens to
    /// dip under the sea is a fact about that landform, and it swung this figure by 3x across a
    /// scale sweep in which the trend was perfectly monotone.
    #[test]
    fn feature_scale_makes_seas_and_sea_level_only_makes_more_ponds() {
        let seed = survey::default_seed();
        // A coarse octave carrying most of the blend, so this half is about the cell size alone.
        // At an even blend the fine octave breaks up every coastline and no cell size can hold a
        // sea together — which is exactly why the weight is a parameter beside the cell.
        let base = WorldParams {
            elevation_coarse_weight: 78,
            elevation_fine_cell: 5,
            ..preset_params("continental").unwrap()
        };
        let at_scale = |cell| {
            survey::run(
                "scale",
                &WorldParams {
                    elevation_coarse_cell: cell,
                    ..base.clone()
                },
                seed,
                survey::DEFAULT_RADIUS,
            )
        };
        let ponds = at_scale(4);
        let seas = at_scale(24);
        assert!(
            ponds.water.bodies > seas.water.bodies * 4,
            "{} bodies at scale 4 against {} at scale 24",
            ponds.water.bodies,
            seas.water.bodies
        );
        assert!(
            seas.water.mean_body > ponds.water.mean_body * 4,
            "mean body {} at scale 24 against {} at scale 4",
            seas.water.mean_body,
            ponds.water.mean_body
        );
        // And it is the *shape* that moved, not the amount: the sea level never changed, so the
        // two worlds hold water within a factor of two of each other.
        assert!(
            ponds.water.water_hexes < seas.water.water_hexes * 2
                && seas.water.water_hexes < ponds.water.water_hexes * 2,
            "a feature-scale change must not be a sea-level change in disguise: {} against {}",
            ponds.water.water_hexes,
            seas.water.water_hexes
        );

        // The other half. Raising the sea level at a fixed feature scale adds water and leaves the
        // count of bodies where it was: more ponds, not bigger ones. The shore cut moves with it
        // only to keep the band order valid; it touches nothing water is measured by.
        let shipped = preset_params("continental").unwrap();
        let low = survey::run("low", &shipped, seed, survey::DEFAULT_RADIUS);
        let high = survey::run(
            "high",
            &WorldParams {
                water_level: 26_000,
                shore_level: 31_000,
                ..shipped
            },
            seed,
            survey::DEFAULT_RADIUS,
        );
        assert!(
            high.water.water_hexes > low.water.water_hexes * 3,
            "a higher sea level must make much more water: {} against {}",
            high.water.water_hexes,
            low.water.water_hexes
        );
        assert!(
            high.water.bodies * 100 < low.water.bodies * 175,
            "a higher sea level must not be a feature-scale change in disguise: {} bodies \
             against {}",
            high.water.bodies,
            low.water.bodies
        );
    }

    /// What the opening promises, asserted rather than assumed.
    ///
    /// The eight hardcoded clearing cells are gone, so the guarantee is now something the
    /// generator has to *find*: a patch of each material, in its window, big enough to stand an
    /// extractor in. Every preset generates all eight materials somewhere in the sample, and the
    /// six guaranteed ones land where they were promised — which is what makes the first hour
    /// playable rather than just survivable.
    ///
    /// Sand and crystal are deliberately not guaranteed. Sand goes where the ocean gate says a
    /// coast is, and crystal is the reason to leave.
    #[test]
    fn every_preset_reaches_every_material_from_the_landing_site() {
        let (definitions, _, _) = catalogs();
        for preset in world_presets() {
            let params = preset.params.clone();
            params
                .validate(&definitions)
                .unwrap_or_else(|error| panic!("preset {} is invalid: {error}", preset.key));
            let report = survey::run(
                preset.key,
                &params,
                survey::default_seed(),
                survey::DEFAULT_RADIUS,
            );
            for material in &report.materials {
                let nearest = match material.nearest {
                    Some(value) => value,
                    None if (material.item_id == SAND || material.item_id == CRYSTAL)
                        && report.radius
                            < survey::landscape_radius(params.elevation_coarse_cell) =>
                    {
                        // Sand sits on the regional ocean; crystal is the reason to leave. A
                        // 96-hex opening sample of a 512-hex landform often never reaches either,
                        // and that is the world working.
                        continue;
                    }
                    None => panic!(
                        "preset {} generates no {} anywhere in a {}-hex sample",
                        preset.key, material.name, report.hexes
                    ),
                };
                let ceiling = if material.item_id == CRYSTAL || material.item_id == SAND {
                    survey::DEFAULT_RADIUS as u32
                } else {
                    40 + BOOTSTRAP_WIDEN_CAP as u32
                };
                assert!(
                    nearest <= ceiling,
                    "preset {}: nearest {} is {nearest} hexes from the landing site",
                    preset.key,
                    material.name
                );
            }
            for (row, &(item_id, _, ceiling)) in report.bootstrap.iter().zip(&BOOTSTRAP_GUARANTEES)
            {
                assert_eq!(row.item_id, item_id);
                let walk = row.edge.unwrap_or_else(|| {
                    panic!(
                        "preset {} cannot place its guaranteed {}",
                        preset.key, row.name
                    )
                });
                // The ceiling is the window's, plus whatever widening the seed needed. The floor
                // is what keeps a guaranteed disc out of the clearing and is never widened.
                assert!(
                    walk > LANDING_CLEAR_RADIUS as u32
                        && walk <= (ceiling + BOOTSTRAP_WIDEN_CAP) as u32,
                    "preset {}: guaranteed {} is {walk} hexes out",
                    preset.key,
                    row.name
                );
                assert!(
                    row.hexes >= WORKABLE_PATCH_HEXES,
                    "preset {}: guaranteed {} is {} hexes, which no extractor can fill from",
                    preset.key,
                    row.name,
                    row.hexes
                );
            }
            // Barren ground stays the common case under every preset, or a site is stumbled over
            // rather than chosen. This is the v0.15 density floor and ceiling, per preset.
            let fields: u32 = report.materials.iter().map(|entry| entry.cells).sum();
            assert!(
                fields * 100 < report.land_hexes * 22 && fields * 100 > report.land_hexes * 3,
                "preset {}: {fields} fields on {} land hexes",
                preset.key,
                report.land_hexes
            );
        }
    }

    /// The patch fill is a second pass over the same cells the material counts walked, and every
    /// mean, the purity share, and the workable-patch distance are all divided out of its totals.
    /// A fill that lost a hex, followed a neighbour of another material, or visited one twice would
    /// move all of them at once and none of them visibly, so the accounting is asserted directly
    /// rather than inferred from a figure looking plausible.
    ///
    /// This is the measurement Landforms and Fields v0.21 is tuned against. It has to be trusted
    /// before the generator moves, which is why it lands in the same commit as the before figures
    /// and ahead of any generation rule.
    #[test]
    fn patch_statistics_account_for_every_generated_cell() {
        let seed = survey::default_seed();
        for preset in world_presets() {
            let report = survey::run(preset.key, &preset.params, seed, 48);
            let mut counted = 0u32;
            let mut pure = 0u32;
            for (material, patch) in report.materials.iter().zip(&report.patches) {
                assert_eq!(
                    material.item_id, patch.item_id,
                    "preset {}: the two material tables are in different orders",
                    preset.key
                );
                assert_eq!(
                    patch.hexes, material.cells,
                    "preset {}: the {} fill visited {} hexes against {} counted cells",
                    preset.key, material.name, patch.hexes, material.cells
                );
                assert_eq!(
                    patch.patches == 0,
                    patch.hexes == 0,
                    "preset {}: {} has {} patches over {} hexes",
                    preset.key,
                    material.name,
                    patch.patches,
                    patch.hexes
                );
                assert!(
                    patch.largest_patch <= patch.hexes && patch.truncated_patches <= patch.patches,
                    "preset {}: {} reports a largest patch of {} and {} truncated of {} over {} \
                     hexes",
                    preset.key,
                    material.name,
                    patch.largest_patch,
                    patch.truncated_patches,
                    patch.patches,
                    patch.hexes
                );
                // A workable patch is at least seven hexes, so claiming one means the largest
                // patch is at least that big, and no patch can start nearer than the nearest cell.
                match patch.nearest_workable_patch {
                    Some(distance) => {
                        assert!(
                            patch.largest_patch >= 7,
                            "preset {}: {} claims a workable patch with a largest patch of {}",
                            preset.key,
                            material.name,
                            patch.largest_patch
                        );
                        assert!(
                            distance >= material.nearest.expect("a patch implies a cell"),
                            "preset {}: {} puts a workable patch at {distance}, nearer than its \
                             nearest cell",
                            preset.key,
                            material.name
                        );
                    }
                    None => assert!(
                        patch.largest_patch < 7,
                        "preset {}: {} has a {}-hex patch and reports none workable",
                        preset.key,
                        material.name,
                        patch.largest_patch
                    ),
                }
                counted += patch.hexes;
                pure += patch.purity_per_mille * patch.hexes / 1000;
            }
            assert!(
                counted > 0,
                "preset {} generates nothing at all",
                preset.key
            );
            // The whole-sample purity is the same count divided by the same denominator, so it has
            // to agree with the per-material shares to within their rounding.
            let overall = report.purity_per_mille * counted / 1000;
            assert!(
                overall.abs_diff(pure) <= report.patches.len() as u32,
                "preset {}: whole-sample purity implies {overall} pure hexes against {pure} from \
                 the material rows",
                preset.key
            );
        }
    }

    /// **The number this milestone exists for.**
    ///
    /// A deposit used to be decided per hex from independent noise channels, so along every
    /// iron/coal boundary the two alternated hex by hex and an extractor covered both and cleanly
    /// worked neither. Purity is the share of resource hexes whose radius-1 disc holds exactly one
    /// material, and the measured before figures were `continental` 532, `archipelago` 474,
    /// `highlands` 662, `basin` 631 — every preset failing, the wettest failing hardest.
    ///
    /// It is asserted at 950 rather than at whatever the presets happen to reach, because the
    /// point is the model and not the tuning: a rule table that could not clear this bar would
    /// mean the lattice had stopped being the thing that decides what a patch is made of.
    #[test]
    fn one_extractor_disc_holds_one_material() {
        let seed = survey::default_seed();
        for preset in world_presets() {
            let report = survey::run(preset.key, &preset.params, seed, survey::DEFAULT_RADIUS);
            assert!(
                report.purity_per_mille >= 950,
                "preset {}: purity is {} per mille",
                preset.key,
                report.purity_per_mille
            );
            // A patch worth automating, per material an extractor is stood on for its own sake.
            // Forests are the one that is measured in area rather than in throughput, so their
            // bar is the deep extractor's disc rather than the base one's.
            for (item_id, floor) in [
                (IRON_ORE, 19),
                (COAL, 19),
                (COPPER_ORE, 19),
                (STONE, 19),
                (WOOD, 61),
            ] {
                let patch = report
                    .patches
                    .iter()
                    .find(|entry| entry.item_id == item_id)
                    .expect("every generated item has a row");
                assert!(
                    patch.largest_patch >= floor,
                    "preset {}: the largest {} patch is {} hexes",
                    preset.key,
                    patch.name,
                    patch.largest_patch
                );
            }
        }
    }

    /// The opening is a promise about every seed, not about the shipped one.
    ///
    /// A guarantee that only holds on the seed it was tuned against is not a guarantee, and the
    /// bootstrap pass is the one part of generation that can fail outright — it widens a window in
    /// fixed steps and then gives up, and `Core::new` refuses a world it gave up on. So the claim
    /// is checked where it would break: every preset, ten seeds, including the presets whose bands
    /// are scarce enough to make a window hard to fill.
    #[test]
    fn every_preset_can_open_a_world_on_any_seed() {
        let (definitions, technologies, scenarios) = catalogs();
        let scenario = scenarios
            .scenarios
            .iter()
            .find(|value| value.key == "new-game")
            .unwrap();
        for preset in world_presets() {
            for step in 0..10u32 {
                let seed = survey::default_seed().wrapping_add(step.wrapping_mul(0x9E3779B1));
                let fields = WorldFields::new(&preset.params, seed);
                assert!(
                    fields.unmet.is_empty(),
                    "preset {} on seed {seed} cannot place {:?}",
                    preset.key,
                    fields.unmet
                );
                let placed: BTreeMap<ItemId, (u32, u32)> = fields
                    .guarantees()
                    .into_iter()
                    .map(|(item_id, walk, hexes)| (item_id, (walk, hexes)))
                    .collect();
                for &(item_id, floor, _) in &BOOTSTRAP_GUARANTEES {
                    let (walk, hexes) = placed[&item_id];
                    // The floor is never widened: a guaranteed disc that reached inside the
                    // clearing would put a deposit where field suppression deletes it.
                    assert!(
                        walk >= floor as u32,
                        "preset {} on seed {seed}: item {item_id} is {walk} hexes out, inside its \
                         floor of {floor}",
                        preset.key
                    );
                    assert!(
                        hexes >= WORKABLE_PATCH_HEXES,
                        "preset {} on seed {seed}: item {item_id} is {hexes} hexes",
                        preset.key
                    );
                }
                // Crystal is the reason to leave, so nothing may guarantee it.
                assert!(!placed.contains_key(&CRYSTAL));
                Core::new(
                    &definitions,
                    &technologies,
                    scenario,
                    Some(seed),
                    Some(preset.params.clone()),
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "preset {} on seed {seed} is unplayable: {error}",
                        preset.key
                    )
                });
            }
        }
    }

    /// A large landform must not strand the player on the 7-hex clearing. The landing disc fades
    /// toward the opening blend and lifts a sea-spawn origin, so the first two dozen hexes stay
    /// mostly walkable on every seed of every preset.
    #[test]
    fn the_landing_disc_is_not_an_ocean_raft() {
        for preset in world_presets() {
            for step in 0..10u32 {
                let seed = survey::default_seed().wrapping_add(step.wrapping_mul(0x9E3779B1));
                let mut blocked = 0u32;
                let mut hexes = 0u32;
                for (q, r) in hexes_in_radius((0, 0), 24) {
                    if axial_distance((0, 0), (q, r)) <= LANDING_CLEAR_RADIUS {
                        continue;
                    }
                    hexes += 1;
                    if terrain_at(&preset.params, seed, q, r, true).blocks_movement() {
                        blocked += 1;
                    }
                }
                assert!(
                    blocked * 100 < hexes * 40,
                    "preset {} on seed {seed}: {blocked} of {hexes} hexes in the first 24 are \
                     impassable",
                    preset.key
                );
            }
        }
    }

    /// A world's identity is its seed *and* its parameters, so a scalar the checksum does not read
    /// is a scalar two different worlds can silently share. Every one of them is moved, one at a
    /// time, and the hash has to move with it.
    #[test]
    fn every_world_parameter_reaches_the_checksum() {
        let base = preset_params("continental").unwrap();
        let hash_of = |params: &WorldParams| {
            let mut hash = 0x811c9dc5u32;
            hash_world_params(&mut hash, params);
            hash
        };
        let baseline = hash_of(&base);
        let mut moved: Vec<WorldParams> = Vec::new();
        for shift in [
            |p: &mut WorldParams| p.elevation_coarse_cell += 1,
            |p: &mut WorldParams| p.elevation_fine_cell += 1,
            |p: &mut WorldParams| p.elevation_coarse_weight += 1,
            |p: &mut WorldParams| p.moisture_cell += 1,
            |p: &mut WorldParams| p.richness_cell += 1,
            |p: &mut WorldParams| p.water_level += 1,
            |p: &mut WorldParams| p.shore_level += 1,
            |p: &mut WorldParams| p.hills_level += 1,
            |p: &mut WorldParams| p.highland_level += 1,
            |p: &mut WorldParams| p.cliff_step += 1,
            |p: &mut WorldParams| p.deep_water_moisture += 1,
            |p: &mut WorldParams| p.site_cell += 1,
            |p: &mut WorldParams| p.site_jitter += 1,
            |p: &mut WorldParams| p.river_cell += 1,
            |p: &mut WorldParams| p.river_width += 1,
            |p: &mut WorldParams| p.river_max_elevation += 1,
            |p: &mut WorldParams| p.ocean_level += 1,
            |p: &mut WorldParams| p.site_rules[0].weight += 1,
            |p: &mut WorldParams| p.site_rules[0].radius_min += 1,
            |p: &mut WorldParams| p.site_rules[0].radius_max += 1,
            |p: &mut WorldParams| p.site_rules[0].site_min += 1,
            |p: &mut WorldParams| p.site_rules[0].yield_core += 1,
            |p: &mut WorldParams| p.site_rules[0].yield_rim += 1,
            |p: &mut WorldParams| p.site_rules[0].yield_jitter += 1,
            |p: &mut WorldParams| p.site_rules[0].member_water_within += 1,
            |p: &mut WorldParams| p.site_rules[0].center_ocean = true,
            |p: &mut WorldParams| p.site_rules[0].member.push(Terrain::Cliff),
            |p: &mut WorldParams| p.site_rules[0].item_id = CRYSTAL,
            |p: &mut WorldParams| p.site_rules[0].terrain = Terrain::Shore,
        ] {
            let mut params = base.clone();
            shift(&mut params);
            assert_ne!(
                hash_of(&params),
                baseline,
                "a world parameter changed and the checksum did not"
            );
            moved.push(params);
        }
        // And no two of them collide, which is the failure a per-field test on its own cannot see.
        let mut hashes: Vec<u32> = moved.iter().map(hash_of).collect();
        let total = hashes.len();
        hashes.sort_unstable();
        hashes.dedup();
        assert_eq!(hashes.len(), total, "two parameter changes hash the same");
    }

    /// A site's yield falls from its core to its rim, which is what makes the middle of a field
    /// worth aiming an extractor at rather than any hex of it being as good as any other.
    #[test]
    fn a_site_is_richest_at_its_core() {
        let params = preset_params("continental").unwrap();
        let seed = survey::default_seed();
        let fields = WorldFields::new(&params, seed);
        let mut compared = 0u32;
        let mut core_wins = 0u32;
        for cell in (-8..8).flat_map(|q| (-8..8).map(move |r| (q, r))) {
            let Some(site) = fields.site_at(cell) else {
                continue;
            };
            let rule = &params.site_rules[site.rule];
            if rule.yield_core == rule.yield_rim || site.radius < 2 {
                continue;
            }
            let Some(center) = fields.field_at(site.center.0, site.center.1, true) else {
                continue;
            };
            for rim in hexes_in_radius(site.center, site.radius)
                .into_iter()
                .filter(|&cell| axial_distance(site.center, cell) == site.radius)
            {
                let Some(edge) = fields.field_at(rim.0, rim.1, true) else {
                    continue;
                };
                if edge.item_id != center.item_id {
                    continue;
                }
                compared += 1;
                core_wins += u32::from(center.quantity > edge.quantity);
            }
        }
        assert!(compared > 20, "only {compared} core/rim pairs to compare");
        // Jitter is deliberately allowed to invert a single pair; a gradient it could hide would
        // be a gradient no player could read.
        assert!(
            core_wins * 100 > compared * 85,
            "the core beat the rim in only {core_wins} of {compared} pairs"
        );
    }

    /// A parameter set that is not a world at all is refused before one is built from it. What this
    /// deliberately does not try to catch is a set that is a world but an unplayable one — that is
    /// what the survey measures, and no validator can decide it.
    #[test]
    fn parameter_sets_that_are_not_worlds_are_refused() {
        let (definitions, technologies, scenarios) = catalogs();
        let base = preset_params("continental").unwrap();
        // One valid row, so each case below differs from a world by exactly the thing it names.
        let one_rule = || SiteRule {
            terrain: Terrain::Hills,
            item_id: IRON_ORE,
            weight: 1,
            radius_min: 1,
            radius_max: 2,
            site_min: ANY,
            yield_core: 4,
            yield_rim: 2,
            yield_jitter: 1,
            member: Vec::new(),
            member_water_within: 0,
            center_ocean: false,
        };
        let scenario = scenarios
            .scenarios
            .iter()
            .find(|value| value.key == "new-game")
            .unwrap();
        let refused = [
            WorldParams {
                elevation_coarse_cell: 0,
                ..base.clone()
            },
            WorldParams {
                elevation_coarse_weight: 140,
                ..base.clone()
            },
            // Bands out of order do not make a band rare; they make it unreachable.
            WorldParams {
                hills_level: 10_000,
                ..base.clone()
            },
            WorldParams {
                site_rules: Vec::new(),
                ..base.clone()
            },
            WorldParams {
                site_rules: vec![SiteRule {
                    item_id: 9999,
                    ..one_rule()
                }],
                ..base.clone()
            },
            // Yield is `interpolated + hash % yield_jitter`, so a zero jitter is a division by zero.
            WorldParams {
                site_rules: vec![SiteRule {
                    yield_jitter: 0,
                    ..one_rule()
                }],
                ..base.clone()
            },
            // A radius of zero is a deposit that is not anywhere, and an inverted range would make
            // `radius_max - radius_min + 1` wrap.
            WorldParams {
                site_rules: vec![SiteRule {
                    radius_min: 4,
                    radius_max: 2,
                    ..one_rule()
                }],
                ..base.clone()
            },
            WorldParams {
                site_rules: vec![SiteRule {
                    radius_max: MAX_SITE_RADIUS + 1,
                    ..one_rule()
                }],
                ..base.clone()
            },
            // A water band would make the cheap water test `field_at` opens with unsound, and a
            // deposit in a basin is nothing a pump or an extractor could reach anyway.
            WorldParams {
                site_rules: vec![SiteRule {
                    member: vec![Terrain::Hills, Terrain::DeepWater],
                    ..one_rule()
                }],
                ..base.clone()
            },
            // Every row weighted zero is a table that generates nothing at all.
            WorldParams {
                site_rules: vec![SiteRule {
                    weight: 0,
                    ..one_rule()
                }],
                ..base.clone()
            },
            WorldParams {
                site_jitter: MAX_SITE_JITTER + 1,
                ..base.clone()
            },
        ];
        for params in refused {
            assert!(
                Core::new(&definitions, &technologies, scenario, None, Some(params)).is_err(),
                "a parameter set that is not a world must be refused"
            );
        }
        assert!(preset_params("no-such-preset").is_none());
    }

    /// A world's parameters survive the round trip, and the world that comes back is the one that
    /// was saved rather than the scenario's default.
    #[test]
    fn a_save_restores_the_parameters_its_world_was_generated_from() {
        let (definitions, technologies, scenarios) = catalogs();
        let scenario = scenarios
            .scenarios
            .iter()
            .find(|value| value.key == "new-game")
            .unwrap();
        let basin = preset_params("basin").unwrap();
        let mut core = Core::new(
            &definitions,
            &technologies,
            scenario,
            None,
            Some(basin.clone()),
        )
        .unwrap();
        assert_ne!(core.world_params, default_world_params());
        core.tick_many(30);
        let save = core.save_string().unwrap();
        let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
        assert_eq!(restored.world_params, basin);
        assert_eq!(restored.checksum(), core.checksum());
        // The default-parameter core is the same scenario, the same seed, and a different world.
        let default = Core::new(&definitions, &technologies, scenario, None, None).unwrap();
        assert_eq!(default.seed, core.seed);
        assert_ne!(
            default.checksum(),
            Core::new(&definitions, &technologies, scenario, None, Some(basin),)
                .unwrap()
                .checksum()
        );
    }

    /// Fuel is a property of the item, so a smelting recipe never names one and coal, charcoal, and
    /// wood are interchangeable at different values. The one case that has to be got right is a
    /// recipe that names a fuel item as an input: steel takes two coal as carbon, and a smelter
    /// that burned those two would starve itself on its own recipe.
    #[test]
    fn a_machine_burns_fuel_from_its_stock_and_never_the_input_it_is_waiting_on() {
        let mut core = game("new-game");
        core.researched.extend([1, 2, 3, 5]);
        core.player.inventory.insert(1, 40);
        core.player.inventory.insert(6, 40);
        set_player_hex(&mut core, 0, 3);
        core.place(0, 4, 7, 0, Some(2)).unwrap();
        let smelter = core.entity_at(0, 4).unwrap();

        // Inputs but no fuel: the smelter holds everything and says exactly why it is stopped.
        core.entities[smelter].inventory.insert(1, 4);
        core.tick_many(30);
        assert_eq!(core.entities[smelter].progress, 0);
        assert_eq!(
            core.entity_snapshot(smelter).status,
            EntityStatus::OutOfFuel
        );
        assert_eq!(core.entities[smelter].inventory.get(&1), Some(&4));

        // One coal is eight energy against a four-energy craft, so the change is banked.
        core.entities[smelter].inventory.insert(5, 1);
        core.tick_many(30);
        assert_eq!(
            core.entities[smelter].cargo,
            Some(Cargo {
                item_id: 11,
                quantity: 1
            })
        );
        assert_eq!(core.entities[smelter].fuel_charge, 4);
        assert_eq!(core.entities[smelter].inventory.get(&5), None);
        assert_eq!(core.entities[smelter].inventory.get(&1), Some(&2));

        // Steel, whose inputs name coal. Exactly the two it needs must not be burned.
        core.player.inventory.insert(1, 40);
        core.player.inventory.insert(6, 40);
        core.place(0, 5, 7, 0, Some(5)).unwrap();
        let steel = core.entity_at(0, 5).unwrap();
        core.entities[steel].inventory.insert(11, 2);
        core.entities[steel].inventory.insert(5, 2);
        core.tick_many(30);
        assert_eq!(core.entities[steel].progress, 0);
        assert_eq!(core.entity_snapshot(steel).status, EntityStatus::OutOfFuel);
        assert_eq!(core.entities[steel].inventory.get(&5), Some(&2));

        // A third coal is surplus, and surplus is what burns.
        core.entities[steel].inventory.insert(5, 3);
        core.tick_many(40);
        assert_eq!(
            core.entities[steel].cargo,
            Some(Cargo {
                item_id: 23,
                quantity: 1
            })
        );
        assert_eq!(core.entities[steel].inventory.get(&5), None);
    }

    /// Flora is the one source that comes back, which is what gives wood and ore different
    /// strategic weight. Regrowth walks a set of cut cells rather than the world, and that set is
    /// derived from the overlay — so a save records the tiles and the set is rebuilt from them.
    #[test]
    fn cut_flora_grows_back_to_what_generation_gave_it_and_then_stops() {
        let (definitions, technologies, scenarios) = catalogs();
        let mut core = game("new-game");
        let cell = (-3, 1);
        let initial = core.deposit_quantity(cell);
        set_player_hex(&mut core, cell.0, cell.1);
        core.gather().unwrap();
        assert_eq!(core.deposit_quantity(cell), initial - 1);
        assert!(core.flora_regrowth.contains(&cell));

        let save = core.save_string().unwrap();
        let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
        assert_eq!(restored.flora_regrowth, core.flora_regrowth);

        let ticks = core
            .item_definition(WOOD)
            .unwrap()
            .regrowth_ticks
            .expect("wood regrows");
        core.tick_many(ticks);
        assert_eq!(core.deposit_quantity(cell), initial);
        // Back to what generation gave it, so it costs nothing again until somebody cuts it.
        assert!(core.flora_regrowth.is_empty());

        // Ore is finite: cutting into a deposit never puts it in the set at all.
        cooldown(&mut core);
        set_player_hex(&mut core, 3, 0);
        core.gather().unwrap();
        assert_eq!(core.deposit_quantity((3, 0)), 47);
        assert!(core.flora_regrowth.is_empty());
    }

    /// A pump is a source without a deposit: it draws from the basin beside it, writes nothing into
    /// the overlay, and the basin never runs down. Away from water it is refused outright, which is
    /// what makes a basin a reason to build somewhere.
    #[test]
    fn a_pump_draws_from_the_basin_beside_it_and_never_empties_it() {
        let mut core = game("new-game");
        core.researched.extend([1, 2, 5, 7]);
        core.player.inventory.insert(11, 20);
        core.player.inventory.insert(14, 20);
        set_player_hex(&mut core, 2, 0);
        assert!(core.terrain_at(2, 1).is_water());
        core.place(3, 1, 11, 0, None).unwrap();
        let index = core.entity_at(3, 1).unwrap();
        core.tick_many(6);
        assert_eq!(
            core.entities[index].cargo,
            Some(Cargo {
                item_id: 10,
                quantity: 1
            })
        );
        assert_eq!(
            core.entity_snapshot(index).status,
            EntityStatus::OutputBlocked
        );
        assert!(core.tiles.get(&(2, 1)).is_none());
        assert!(core
            .place(3, -1, 11, 0, None)
            .unwrap_err()
            .contains("beside open water"));
    }

    #[test]
    fn a_bridge_supports_transport_on_shallows_and_refuses_deep_water() {
        let mut core = game("new-game");
        core.researched.extend([1, 11, 15]);
        core.player.inventory.insert(1, 10);
        core.player.inventory.insert(6, 10);
        core.player.inventory.insert(16, 10);
        let shallow = (-24..=24)
            .flat_map(|q| (-24..=24).map(move |r| (q, r)))
            .find(|&(q, r)| core.terrain_at(q, r) == Terrain::ShallowWater)
            .expect("the new-game landscape has shallow water");
        let deep = (-512..=512)
            .flat_map(|q| (-512..=512).map(move |r| (q, r)))
            .find(|&(q, r)| core.terrain_at(q, r) == Terrain::DeepWater)
            .expect("the new-game landscape has deep water");

        set_player_hex(&mut core, shallow.0 + 2, shallow.1);
        core.place(shallow.0, shallow.1, 23, 0, None).unwrap();
        core.place(shallow.0, shallow.1, 2, 0, None).unwrap();
        assert_eq!(
            core.entities
                .iter()
                .filter(|entity| { entity.placed.q == shallow.0 && entity.placed.r == shallow.1 })
                .count(),
            2,
            "the support and transport are distinct entities"
        );
        core.rotate(shallow.0, shallow.1).unwrap();
        assert_eq!(
            core.entities[core.entity_at(shallow.0, shallow.1).unwrap()]
                .placed
                .orientation,
            1
        );
        let (definitions, technologies, scenarios) = catalogs();
        let save = core.save_string().unwrap();
        let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
        assert_eq!(
            restored
                .entities
                .iter()
                .filter(|entity| entity.placed.q == shallow.0 && entity.placed.r == shallow.1)
                .count(),
            2,
            "a bridge and its transport survive a save"
        );
        assert_eq!(
            core.entities[core.entity_at(shallow.0, shallow.1).unwrap()].kind,
            BuildingKind::Belt
        );
        core.erase(shallow.0, shallow.1).unwrap();
        core.place(shallow.0, shallow.1, 18, NORTH, None).unwrap();
        assert_eq!(
            core.entities[core.entity_at(shallow.0, shallow.1).unwrap()]
                .placed
                .definition_id,
            18
        );
        core.erase(shallow.0, shallow.1).unwrap();
        assert_eq!(
            core.entities[core.entity_at(shallow.0, shallow.1).unwrap()].kind,
            BuildingKind::Bridge
        );

        set_player_hex(&mut core, deep.0 + 2, deep.1);
        assert!(core.place(deep.0, deep.1, 23, 0, None).is_err());
        assert_eq!(Terrain::ShallowWater.blocks_construction(), true);
    }

    /// A kiln and a smelter are the same `BuildingKind` running different recipe categories, so the
    /// rule that keeps a circuit out of a kiln is one field and one check — asked once at placement
    /// and again at reassignment, because a machine that could be reassigned past the rule would
    /// make the rule decorative.
    #[test]
    fn a_machine_runs_only_its_own_category_and_is_reassigned_only_between_crafts() {
        let mut core = game("new-game");
        core.researched.extend([1, 2, 3, 5, 6]);
        core.player.inventory.insert(1, 40);
        core.player.inventory.insert(6, 40);
        core.player.inventory.insert(8, 20);
        set_player_hex(&mut core, 0, 3);
        assert!(core
            .place(0, 4, 8, 0, Some(2))
            .unwrap_err()
            .contains("cannot run a smelting recipe"));
        core.place(0, 4, 8, 0, Some(6)).unwrap();
        let index = core.entity_at(0, 4).unwrap();

        assert!(core
            .set_recipe(0, 4, 2)
            .unwrap_err()
            .contains("cannot run a smelting recipe"));
        core.set_recipe(0, 4, 7).unwrap();
        assert_eq!(core.entities[index].placed.recipe_id, Some(7));

        // Mid-craft it keeps the job it is running: the inputs it reserved belong to that job.
        core.entities[index].inventory.insert(9, 4);
        core.tick_many(2);
        assert!(core.entities[index].progress > 0);
        assert!(core.set_recipe(0, 4, 6).unwrap_err().contains("mid-craft"));
    }

    #[test]
    fn continuous_movement_intent_and_collision_are_native() {
        let mut core = game("new-game");
        // Stay inside the landing clearing so derived water and cliffs cannot interrupt the walk.
        set_player_hex(&mut core, 0, 3);
        let start = (core.player.x, core.player.y);
        core.set_move_intent(707, -707).unwrap();
        core.advance_player_steps(3);
        let step = 707 * PLAYER_SPEED / 1000;
        assert_eq!(core.player.x, start.0 + 3 * step);
        assert_eq!(core.player.y, start.1 - 3 * step);
        assert_eq!((core.player.facing_x, core.player.facing_y), (707, -707));
        core.set_move_intent(0, 0).unwrap();
        core.advance_player_steps(3);
        assert_eq!(
            (core.player.x, core.player.y),
            (start.0 + 3 * step, start.1 - 3 * step)
        );
        assert!(core.set_move_intent(1001, 0).is_err());

        // A guaranteed landing cliff still blocks: stand just west of (1, -1) and walk east.
        let (cliff_x, cliff_y) = axial_world(1, -1);
        core.player.x = cliff_x - HEX_X / 2 - 20;
        core.player.y = cliff_y;
        let blocked_x = core.player.x;
        core.set_move_intent(1000, 0).unwrap();
        core.advance_player_steps(1);
        assert_eq!(core.player.x, blocked_x);
        assert_eq!(core.terrain_at(1, -1), Terrain::Cliff);
    }

    /// Shallows are a 1 m/s ford: walkable, not buildable, and the gait does not matter once
    /// you are in the water. Deep water stays a wall.
    #[test]
    fn shallow_water_is_a_slow_ford() {
        assert!(!Terrain::ShallowWater.blocks_movement());
        assert!(Terrain::ShallowWater.blocks_construction());
        assert!(Terrain::DeepWater.blocks_movement());
        assert!(Terrain::DeepWater.blocks_construction());

        let mut core = game("new-game");
        set_player_hex(&mut core, 2, 1);
        assert_eq!(core.terrain_at(2, 1), Terrain::ShallowWater);
        let start = (core.player.x, core.player.y);
        let ford = PLAYER_SPEED / 5;

        core.set_move_intent(1000, 0).unwrap();
        core.advance_player_steps(1);
        assert_eq!(core.player.x, start.0 + ford);

        core.player.x = start.0;
        core.set_move_intent(600, 0).unwrap();
        core.advance_player_steps(1);
        assert_eq!(
            core.player.x,
            start.0 + ford,
            "wading is 1 m/s at any gait, not 3/5 of it"
        );

        // Still not a building site: the player can stand in it, a pump cannot.
        set_player_hex(&mut core, 0, 3);
        core.researched.extend([1, 2, 5, 7]);
        core.player.inventory.insert(11, 20);
        core.player.inventory.insert(14, 20);
        assert!(core
            .place(2, 1, 11, 0, None)
            .unwrap_err()
            .contains("environment blocks construction"));
    }

    /// Facing became something the player aims rather than a side effect of walking, so the command
    /// that sets it has to resolve as natively as the movement it sits beside: the host names a
    /// world point and this turns it into the vector the checksum hashes.
    #[test]
    fn aiming_faces_the_world_position_the_host_names() {
        let mut core = game("new-game");
        set_player_hex(&mut core, 0, 3);
        let (x, y) = (core.player.x, core.player.y);

        core.set_aim(x + 5_000, y).unwrap();
        assert_eq!((core.player.facing_x, core.player.facing_y), (1000, 0));
        core.set_aim(x, y - 5_000).unwrap();
        assert_eq!((core.player.facing_x, core.player.facing_y), (0, -1000));

        // A diagonal resolves to a unit vector, not to whatever delta the host happened to send,
        // and pushing the same direction ten times further does not change the answer.
        core.set_aim(x - 3_000, y + 3_000).unwrap();
        assert_eq!((core.player.facing_x, core.player.facing_y), (-707, 707));
        core.set_aim(x - 30_000, y + 30_000).unwrap();
        assert_eq!((core.player.facing_x, core.player.facing_y), (-707, 707));

        // A cursor resting exactly on the player names no direction, so the last one stands.
        core.set_aim(x, y).unwrap();
        assert_eq!((core.player.facing_x, core.player.facing_y), (-707, 707));
        assert!(core.set_aim(x + (MAX_AIM_DISTANCE as i32) + 1, y).is_err());

        // What an aim resolves to is ordinary player state: it is saved, and the save validator
        // that bounds facing accepts it, because native produced it rather than the host.
        let (definitions, technologies, scenarios) = catalogs();
        let save = core.save_string().unwrap();
        let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
        assert_eq!(
            (restored.player.facing_x, restored.player.facing_y),
            (-707, 707)
        );
    }

    /// What keeps a pointer aiming and a touch layout facing the way it walks, with no stored
    /// aiming mode for the save format and the checksum to carry: both commands write facing, and
    /// whichever the host sent last in the batch is the one that stands.
    #[test]
    fn an_aim_later_in_the_batch_outranks_the_walk_direction() {
        let mut core = game("new-game");
        set_player_hex(&mut core, 0, 3);
        let (x, y) = (core.player.x, core.player.y);
        let batch = format!(
            r#"[{{"type":"move_intent","x":1000,"y":0}},{{"type":"aim","x":{x},"y":{}}}]"#,
            y - 4_000
        );
        core.advance(&batch, 0, 0).unwrap();
        assert_eq!((core.player.facing_x, core.player.facing_y), (0, -1000));

        // A frame with no aim in it — every frame of the touch layout — still faces the walk.
        core.advance(IDLE_MOVE_EAST, 0, 0).unwrap();
        assert_eq!((core.player.facing_x, core.player.facing_y), (1000, 0));
    }

    #[test]
    fn integer_square_root_is_exact_on_squares_and_truncates_between_them() {
        assert_eq!(integer_sqrt(0), 0);
        assert_eq!(integer_sqrt(-9), 0);
        for root in [1_i64, 2, 3, 1_000, 46_341, 3_037_000_499] {
            assert_eq!(integer_sqrt(root * root), root);
            assert_eq!(integer_sqrt(root * root - 1), root - 1);
        }
    }

    #[test]
    fn the_player_walks_on_its_own_cadence_not_the_factorys() {
        // The complaint this answers: the player stopped when the factory paused and crawled at a
        // low speed multiplier, because walking ran inside the simulation tick.
        let mut core = game("new-game");
        set_player_hex(&mut core, 0, 3);
        let start = (core.player.x, core.player.y);
        core.set_move_intent(1000, 0).unwrap();

        // A paused factory advances no ticks at all, and the player still walks.
        core.advance(IDLE_MOVE_EAST, 0, 10).unwrap();
        assert_eq!(core.tick, 0);
        assert_eq!(core.player.x, start.0 + 10 * PLAYER_SPEED);

        // Ticking the factory without spending player steps moves nothing.
        let held = core.player.x;
        core.advance("[]", 30, 0).unwrap();
        assert_eq!(core.tick, 30);
        assert_eq!(core.player.x, held);

        // The same step count always covers the same ground, whatever the factory is doing, so a
        // replay of the same commands and counts still reproduces the same position.
        let mut slow = game("new-game");
        let mut fast = game("new-game");
        for core in [&mut slow, &mut fast] {
            set_player_hex(core, 0, 3);
        }
        for _ in 0..4 {
            slow.advance(IDLE_MOVE_EAST, 1, 8).unwrap();
            fast.advance(IDLE_MOVE_EAST, 16, 8).unwrap();
        }
        assert_eq!(slow.player.x, fast.player.x);
        assert_eq!(slow.player.y, fast.player.y);
        assert_eq!(Factory::player_ticks_per_second(), PLAYER_TICKS_PER_SECOND);
    }

    /// A hexagon is 1 m², the walk is 3 m/s, the run is 5 m/s. Native stores one step size — the
    /// run, at intent 1000 — and the host sends 600 for the walk, which is exactly 3/5 of full
    /// intent. The metre itself is neighbour spacing: `HEX_X` world units = √(2/√3) m.
    #[test]
    fn walk_is_three_metres_a_second_and_run_is_five() {
        const WALK_INTENT: i32 = 600;
        let walk = WALK_INTENT * PLAYER_SPEED / 1000;
        assert_eq!(walk * 5, PLAYER_SPEED * 3);
        assert_eq!(PLAYER_SPEED, 275);
    }

    #[test]
    fn gathering_depletes_finite_resources_and_conserves_items() {
        let mut core = game("new-game");
        set_player_hex(&mut core, 3, 0);
        let before = core.deposit_quantity((3, 0));
        for _ in 0..before {
            core.gather().unwrap();
            cooldown(&mut core);
        }
        assert_eq!(core.player.inventory.get(&1), Some(&before));
        assert_eq!(core.deposit_quantity((3, 0)), 0);
        assert!(core.gather().is_err());
    }

    /// A gather takes from the hex the player is standing on, wherever they stand inside it and
    /// whichever way they face. The old target was pushed half a gather range along the facing and
    /// then resolved to the nearest field cell, so stepping off-centre inside your own hex silently
    /// moved the harvest to the neighbour ahead: the number under your feet stayed put while a
    /// different hex counted down. Nothing on screen shows facing, so that was unattributable.
    #[test]
    fn a_gather_takes_from_the_hex_the_player_stands_on_whatever_way_they_face() {
        for (facing_x, facing_y) in [(1000, 0), (-1000, 0), (500, 866), (-500, -866)] {
            for offset in [-880, -400, 0, 400, 880] {
                let mut core = game("new-game");
                set_player_hex(&mut core, 3, 0);
                // Field cells on both sides, so a target that drifts either way is visible.
                core.write_overlay(4, 0, 1, 20, 20);
                core.write_overlay(2, 0, 1, 20, 20);
                core.player.x += offset;
                core.player.facing_x = facing_x;
                core.player.facing_y = facing_y;
                core.gather().unwrap();
                assert_eq!(
                    (
                        core.deposit_quantity((2, 0)),
                        core.deposit_quantity((3, 0)),
                        core.deposit_quantity((4, 0)),
                    ),
                    (20, 47, 20),
                    "offset {offset} facing {facing_x},{facing_y} took from the wrong hex"
                );
            }
        }
    }

    /// Reach is exactly what an extractor on the same hex would cover, and it does not depend on
    /// facing. Standing on the field takes from it; standing one step away still reaches it, which
    /// is what lets a player work a field edge; two steps away is out of reach from every angle.
    #[test]
    fn gather_reach_is_the_extractor_predicate_and_is_the_same_in_every_direction() {
        for &(dq, dr) in &DIRECTIONS {
            for steps in 0..=2 {
                for facing in 0..6u8 {
                    let mut core = game("new-game");
                    let (x, y) = axial_world(3 + dq * steps, dr * steps);
                    core.player.x = x;
                    core.player.y = y;
                    (core.player.facing_x, core.player.facing_y) = world_direction(facing);
                    core.ensure_neighborhood(core.player.x, core.player.y);
                    let reached = core.gather().is_ok();
                    // One step out only reaches back if no nearer field cell outbids (3,0); the
                    // rule is the shared candidate list, so ask it rather than restating it.
                    let expected = core.resource_at_world(x, y) == Some((3, 0));
                    assert_eq!(
                        reached && core.deposit_quantity((3, 0)) == 47,
                        expected,
                        "step {steps} along {dq},{dr} facing {facing}"
                    );
                    if steps == 2 {
                        assert_eq!(core.deposit_quantity((3, 0)), 48, "reach ran past one hex");
                    }
                }
            }
        }
    }

    /// The cooldown between two gathers runs on the player's clock, not the factory's. It used to
    /// be decremented once per simulation tick, so pausing froze it outright — one gather, then
    /// "action cooling down" for as long as the factory stayed paused — and the harvest rate
    /// otherwise rode the speed setting, six times faster at 60 tps than at 4.
    #[test]
    fn the_gather_cooldown_runs_on_the_players_clock_not_the_factorys() {
        let mut core = game("new-game");
        set_player_hex(&mut core, 3, 0);
        core.gather().unwrap();
        assert!(core.gather().is_err(), "the cooldown has to hold at all");
        // The factory is paused for the whole of this: not one tick is advanced.
        let total = core.player.action_cooldown;
        assert!(total > 1, "iron ore is slower than a single step");
        core.advance_player_steps(total - 1);
        assert!(core.gather().is_err(), "cleared early");
        core.advance_player_steps(1);
        core.gather().unwrap();
        assert_eq!(core.tick, 0);
        assert_eq!(core.deposit_quantity((3, 0)), 46);

        // And running the factory on its own no longer clears it.
        let mut core = game("new-game");
        set_player_hex(&mut core, 3, 0);
        core.gather().unwrap();
        core.tick_many(240);
        assert!(
            core.gather().is_err(),
            "factory time paid the player's debt"
        );
    }

    #[test]
    fn placement_enforces_terrain_occupancy_range_cost_and_technology() {
        let mut core = game("new-game");
        core.player.inventory.insert(1, 100);
        core.player.inventory.insert(3, 100);
        assert!(core.place(2, 0, 2, 0, None).unwrap_err().contains("locked"));
        core.researched.extend([1, 2, 3, 4]);
        assert!(core
            .place(2, 1, 2, 0, None)
            .unwrap_err()
            .contains("environment"));
        assert!(core
            .place(20, 20, 2, 0, None)
            .unwrap_err()
            .contains("range"));
        core.player.inventory.clear();
        assert!(core
            .place(2, 0, 2, 0, None)
            .unwrap_err()
            .contains("Iron ore"));
        core.player.inventory.insert(1, 8);
        core.player.inventory.insert(8, 7);
        // Extractor wants iron ore and stone. Naming the missing item is the message;
        // "construction cost is not available" did not say which.
        assert!(core.place(3, 0, 1, 0, None).unwrap_err().contains("Stone"));
        core.player.inventory.clear();
        core.player.inventory.insert(1, 3);
        core.place(2, 0, 2, 0, None).unwrap();
        assert!(core
            .place(2, 0, 2, 0, None)
            .unwrap_err()
            .contains("occupied"));
        assert!(core
            .place(2, -2, 1, 0, None)
            .unwrap_err()
            .contains("deposit"));
        set_player_hex(&mut core, 100, 100);
        core.player.inventory.insert(1, 2);
        let checksum_before_preview = core.checksum();
        assert!(core.placement_legality(101, 100, 2, 0, None, true).is_ok());
        assert_eq!(core.checksum(), checksum_before_preview);
        assert!(core
            .placement_legality(100, 100, 2, 0, None, true)
            .unwrap_err()
            .contains("player"));
    }

    /// The six corner vectors are one rotational family, not six hand-written special cases.
    #[test]
    fn corner_headings_form_a_clockwise_six_point_rosette() {
        let corners = &TRANSPORT_DIRECTIONS[usize::from(NORTH)..];
        for index in 0..corners.len() {
            let (q, r) = corners[index];
            assert_eq!(corners[(index + 1) % corners.len()], (-r, q + r));
        }
        // The six edges keep their indices, which is what makes every saved orientation, every
        // fixture, and every existing drag mean the same thing after the table grew.
        assert_eq!(TRANSPORT_DIRECTIONS[..DIRECTIONS.len()], DIRECTIONS);
        // Adjacency stays six. A boiler must never reach two rows.
        assert_eq!(DIRECTIONS.len(), 6);
    }

    /// Every corner heading resolves symmetrically, and no target in a wide lattice window gives
    /// two headings the same full two-row close. The resolver still carries an explicit tie-break.
    #[test]
    fn a_corner_drag_uses_the_two_row_period_only_within_thirty_degrees_of_a_heading() {
        use OrientationAxis::{Corner, Edge};
        for &(dq, dr) in &TRANSPORT_DIRECTIONS[usize::from(NORTH)..] {
            assert_eq!(
                line_between((0, 0), (dq * 3, dr * 3), Corner),
                vec![(0, 0), (dq, dr), (dq * 2, dr * 2), (dq * 3, dr * 3)]
            );
        }
        for q in -64..=64 {
            for r in -64..=64 {
                let remaining = axial_distance((0, 0), (q, r));
                let candidates = TRANSPORT_DIRECTIONS[usize::from(NORTH)..]
                    .iter()
                    .filter(|&&(dq, dr)| axial_distance((dq, dr), (q, r)) == remaining - 2)
                    .count();
                assert!(candidates <= 1, "corner drag tie at {q},{r}");
            }
        }
        // Bounded like every other drag.
        assert_eq!(
            line_between((0, 0), (900, -1800), Corner).len(),
            MAX_LINE_CELLS
        );
        // And the property that keeps every existing test meaningful: the edge axis is the old
        // resolver, untouched.
        for &to in &[(3, 0), (4, 1), (5, 3), (0, -6), (-3, 2)] {
            assert_eq!(line_between((0, 0), to, Edge), hex_line((0, 0), to));
        }
    }

    #[test]
    fn a_drag_resolves_one_turn_and_stays_bounded() {
        // A straight run along a hex axis.
        assert_eq!(
            hex_line((0, 0), (3, 0)),
            vec![(0, 0), (1, 0), (2, 0), (3, 0)]
        );
        // An off-axis run turns exactly once rather than staircasing, so a belt line between two
        // endpoints carries the fewest direction changes it can.
        assert_eq!(
            hex_line((2, 0), (4, 1)),
            vec![(2, 0), (3, 0), (4, 0), (4, 1)]
        );
        let turns = hex_line((0, 0), (5, 3))
            .windows(2)
            .filter_map(|pair| step_direction(pair[0], pair[1]))
            .collect::<Vec<_>>()
            .windows(2)
            .filter(|step| step[0] != step[1])
            .count();
        assert_eq!(turns, 1);
        // Both endpoints are always included, and a single-cell drag is a single placement.
        assert_eq!(hex_line((-3, 2), (-3, 2)), vec![(-3, 2)]);
        // One command can only ever expand into a bounded run.
        assert_eq!(hex_line((0, 0), (900, 0)).len(), MAX_LINE_CELLS);
        assert_eq!(step_direction((0, 0), (0, 1)), Some(1));
        assert_eq!(step_direction((0, 0), (4, 4)), None);
    }

    #[test]
    fn one_drag_builds_exactly_what_the_equivalent_placements_build() {
        // The path and per-cell headings `a_drag_resolves_one_turn_and_stays_bounded` pins, written
        // out so this test does not re-derive them from the code it is checking.
        let equivalent = [((2, 0), 0u8), ((3, 0), 0), ((4, 0), 1), ((4, 1), 1)];

        let mut dragged = game("new-game");
        dragged.researched.extend([1, 2, 3, 4]);
        dragged.player.inventory.insert(1, 100);
        dragged.place_line((2, 0), (4, 1), 2, 0, None).unwrap();

        let mut individual = game("new-game");
        individual.researched.extend([1, 2, 3, 4]);
        individual.player.inventory.insert(1, 100);
        for ((q, r), orientation) in equivalent {
            individual.place(q, r, 2, orientation, None).unwrap();
        }

        // Same world, same blueprint, same materials spent: a drag is exactly its placements.
        assert_eq!(dragged.checksum(), individual.checksum());
        assert_eq!(dragged.entities.len(), individual.entities.len());
        // The drag routed the run itself — every belt points at its successor and the last one
        // keeps the run's heading — so the player never oriented a segment by hand.
        let headings: Vec<u8> = dragged
            .entities
            .iter()
            .filter(|entity| !entity.placed.scenario_owned)
            .map(|entity| entity.placed.orientation)
            .collect();
        assert_eq!(headings, vec![0, 0, 1, 1]);
        // One drag reports one result, not one per cell.
        assert_eq!(dragged.events.last().unwrap(), "Placed 4 × Belt");
    }

    #[test]
    fn a_drag_builds_what_it_legally_can_and_reports_why_it_stopped() {
        let mut core = game("new-game");
        core.researched.extend([1, 2, 3, 4]);
        // Enough for two of the four cells the drag covers.
        core.player.inventory.insert(1, 2);
        core.place_line((2, 0), (4, 1), 2, 0, None).unwrap();
        assert_eq!(
            core.entities
                .iter()
                .filter(|entity| !entity.placed.scenario_owned)
                .count(),
            2
        );
        assert_eq!(core.player.inventory.get(&1).copied().unwrap_or(0), 0);
        // Running out of materials part-way is reported, and what was affordable still stands.
        assert!(core.events.iter().any(|event| event.contains("Iron ore")));

        // A drag that can place nothing at all fails as the single placement would have.
        let mut empty = game("new-game");
        empty.researched.extend([1, 2, 3, 4]);
        assert!(empty
            .place_line((2, 0), (4, 1), 2, 0, None)
            .unwrap_err()
            .contains("Iron ore"));
        assert!(empty
            .entities
            .iter()
            .all(|entity| entity.placed.scenario_owned));
    }

    #[test]
    fn a_drag_preview_is_what_the_drag_builds() {
        let mut core = game("new-game");
        core.researched.extend([1, 2, 3, 4]);
        // Materials for two of the four cells, so the preview has to show the run stopping.
        core.player.inventory.insert(1, 2);

        let preview = core.line_preview((2, 0), (4, 1), 2, 0, None);
        assert_eq!(preview.len(), 4);
        let promised: Vec<(i32, i32, u8)> = preview
            .iter()
            .filter(|cell| cell.legal)
            .map(|cell| (cell.q, cell.r, cell.orientation))
            .collect();
        assert_eq!(promised.len(), 2);
        // The preview spends materials as it walks, so it marks the exact cell the run stops at
        // rather than implying the whole line is affordable.
        assert!(!preview[2].legal && !preview[3].legal);

        core.place_line((2, 0), (4, 1), 2, 0, None).unwrap();
        let built: Vec<(i32, i32, u8)> = core
            .entities
            .iter()
            .filter(|entity| !entity.placed.scenario_owned)
            .map(|entity| (entity.placed.q, entity.placed.r, entity.placed.orientation))
            .collect();
        assert_eq!(built, promised);

        // Removal previews the same way: only cells actually holding something removable.
        let erasable = core.erase_line_preview((2, 0), (4, 1));
        assert_eq!(
            erasable
                .iter()
                .filter(|cell| cell.legal)
                .map(|cell| (cell.q, cell.r))
                .collect::<Vec<_>>(),
            vec![(2, 0), (3, 0)]
        );
    }

    #[test]
    fn one_drag_removes_the_run_it_covers() {
        let mut core = game("new-game");
        core.researched.extend([1, 2, 3, 4]);
        core.player.inventory.insert(1, 100);
        core.place_line((2, 0), (4, 1), 2, 0, None).unwrap();
        let spent = *core.player.inventory.get(&1).unwrap();
        core.erase_line((2, 0), (4, 1)).unwrap();
        assert!(core
            .entities
            .iter()
            .all(|entity| entity.placed.scenario_owned));
        // Removal refunds through the ordinary erase path, so a built-then-removed run is free.
        assert_eq!(core.player.inventory.get(&1), Some(&(spent + 4)));
        assert_eq!(core.events.last().unwrap(), "Recovered 4 buildings");
        // A drag across empty ground reports the same refusal a single erase would.
        assert!(core
            .erase_line((2, 0), (4, 1))
            .unwrap_err()
            .contains("no building"));
    }

    #[test]
    fn undo_takes_back_the_last_construction_through_the_erase_path() {
        let mut core = game("new-game");
        core.researched.extend([1, 2, 3, 4]);
        core.player.inventory.insert(1, 100);
        let before = core.checksum();

        core.place(2, 0, 2, 0, None).unwrap();
        core.undo().unwrap();
        // Undo is exactly an erase of what was just built, so the world returns to where it was.
        assert_eq!(core.checksum(), before);
        assert_eq!(core.events.last().unwrap(), "Undid the last construction");

        // It unwinds a drag one construction at a time, most recent first.
        core.place_line((2, 0), (4, 1), 2, 0, None).unwrap();
        for _ in 0..4 {
            core.undo().unwrap();
        }
        assert_eq!(core.checksum(), before);
        assert!(core.undo().unwrap_err().contains("nothing to undo"));

        // A construction already removed by hand is skipped rather than undoing something else.
        core.place(2, 0, 2, 0, None).unwrap();
        core.place(3, 0, 2, 0, None).unwrap();
        core.erase(3, 0).unwrap();
        core.undo().unwrap();
        assert!(core
            .entities
            .iter()
            .all(|entity| entity.placed.scenario_owned));

        // Undo history is session state: a save carries none of it, so a restored game has nothing
        // to take back and cannot erase across a load boundary.
        core.place(2, 0, 2, 0, None).unwrap();
        let (definitions, technologies, scenarios) = catalogs();
        let save = core.save_string().unwrap();
        let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
        assert!(restored.undo_stack.is_empty());
        assert_eq!(restored.checksum(), core.checksum());
    }

    #[test]
    fn erase_refunds_full_cost_and_contents_but_protects_scenario_objects() {
        let mut core = game("new-game");
        core.researched.insert(1);
        core.player.inventory.insert(1, 2);
        core.place(2, 0, 2, 0, None).unwrap();
        let index = core
            .entities
            .iter()
            .position(|entity| entity.placed.q == 2)
            .unwrap();
        core.entities[index].cargo = Some(Cargo {
            item_id: 3,
            quantity: 1,
        });
        core.erase(2, 0).unwrap();
        assert_eq!(core.player.inventory.get(&1), Some(&2));
        assert_eq!(core.player.inventory.get(&3), Some(&1));
        assert!(core.erase(0, 0).unwrap_err().contains("protected"));
    }

    #[test]
    fn one_overlap_rule_answers_both_placement_questions() {
        // Fields are hex cells. Placement and the extractor's cached candidates share
        // `field_covered_at`, so a resolved reference cannot drift from the rule that allowed
        // the building. Cliffs occupy their own hex and do not make the neighbour unbuildable.
        let mut core = game("new-game");
        core.researched.extend([1, 2, 3, 4]);
        core.player.inventory.insert(1, 20);
        core.player.inventory.insert(6, 4);

        let (hex_x, hex_y) = axial_world(3, 0);
        set_player_hex(&mut core, 3, 1);
        assert!(
            core.resource_at_world(hex_x, hex_y).is_some(),
            "a field cell must be reachable from its own hex"
        );
        core.place(3, 0, 1, 0, None).unwrap();

        let index = core.entity_at(3, 0).unwrap();
        assert_eq!(core.extractor_deposit(index), Some((3, 0)));
        assert_eq!(
            core.deposit_candidates(3, 0, EXTRACT_RADIUS),
            core.deposit_links[&core.entities[index].id]
        );

        let mut ground = game("new-game");
        ground.researched.extend([1, 2, 3, 4]);
        ground.player.inventory.insert(1, 20);
        // The landing cliff sits on (1, -1); the neighbouring lowland hex stays buildable.
        assert_eq!(ground.terrain_at(1, -1), Terrain::Cliff);
        ground.place(0, -1, 2, 0, None).unwrap();
        assert!(ground
            .place(1, -1, 2, 0, None)
            .unwrap_err()
            .contains("environment"));
    }

    #[test]
    fn carrying_capacity_is_a_slot_rule_over_the_ordinary_inventory() {
        let mut core = game("new-game");
        let slots = core.player.carry_slots;
        assert!(slots > 0);
        let stack = core.stack_size(1);

        // Capacity is expressed in stacks of the item's own size, not in item count.
        core.player.inventory.insert(1, stack);
        assert_eq!(core.slots_used(&core.player.inventory), 1);
        core.player.inventory.insert(1, stack + 1);
        assert_eq!(core.slots_used(&core.player.inventory), 2);
        assert_eq!(core.player_room_for(1), (slots - 2) * stack + stack - 1);

        // Filling the pack refuses further gathering rather than silently overflowing it.
        core.player.inventory.insert(1, slots * stack);
        assert_eq!(core.player_room_for(1), 0);
        set_player_hex(&mut core, 3, 0);
        assert!(core.gather().unwrap_err().contains("capacity"));
        // A different item has no room either, because every slot is spoken for.
        assert_eq!(core.player_room_for(3), 0);

        // The stacks the host draws come from native, one entry per occupied slot.
        core.player.inventory.insert(1, stack + 3);
        core.player.inventory.insert(3, 1);
        assert_eq!(
            core.carry_stacks(),
            vec![
                Ingredient {
                    item_id: 1,
                    quantity: stack
                },
                Ingredient {
                    item_id: 1,
                    quantity: 3
                },
                Ingredient {
                    item_id: 3,
                    quantity: 1
                },
            ]
        );
    }

    #[test]
    fn an_erase_that_cannot_be_carried_is_refused_rather_than_losing_items() {
        let mut core = game("new-game");
        core.researched.extend([1, 2, 3, 4]);
        core.player.inventory.insert(1, 4);
        set_player_hex(&mut core, 1, 0);
        core.place(2, 0, 4, 0, None).unwrap();
        let index = core.entity_at(2, 0).unwrap();
        core.entities[index].inventory.insert(3, 9);

        // A full pack refuses the recovery: the container and its contents stay exactly as they
        // were, so nothing is destroyed and the erase is available again once there is room.
        let stack = core.stack_size(1);
        core.player
            .inventory
            .insert(1, core.player.carry_slots * stack);
        let before = core.checksum();
        assert!(core.erase(2, 0).unwrap_err().contains("no room"));
        assert_eq!(core.checksum(), before);
        // The removal preview says the same thing, so a drag cannot promise a recovery it will
        // refuse on release.
        assert!(core
            .erase_line_preview((2, 0), (2, 0))
            .iter()
            .all(|cell| !cell.legal));

        // With room, the same erase returns the cost and every stored item.
        core.player.inventory.clear();
        core.erase(2, 0).unwrap();
        assert_eq!(core.player.inventory.get(&1), Some(&3));
        assert_eq!(core.player.inventory.get(&3), Some(&9));
    }

    #[test]
    fn withdrawing_moves_what_fits_and_leaves_the_rest_in_the_container() {
        let mut core = game("new-game");
        core.researched.extend([1, 2, 3, 4]);
        core.player.inventory.insert(1, 3);
        set_player_hex(&mut core, 1, 0);
        core.place(2, 0, 4, 0, None).unwrap();
        let index = core.entity_at(2, 0).unwrap();
        core.entities[index].inventory.insert(2, 12);

        // Out of range, wrong building, and an item the container does not hold are all refused.
        assert!(core.withdraw(2, 0, 1, 1).unwrap_err().contains("none"));
        assert!(core.withdraw(9, 9, 2, 1).unwrap_err().contains("range"));
        assert!(core
            .withdraw(0, 0, 2, 1)
            .unwrap_err()
            .contains("only containers"));

        // The request is a ceiling: what moves is limited by the stock and by carrying space.
        core.withdraw(2, 0, 2, 5).unwrap();
        assert_eq!(core.player.inventory.get(&2), Some(&5));
        assert_eq!(core.entities[index].inventory.get(&2), Some(&7));

        // Filling the pack stops the transfer without destroying what stayed behind.
        let stack = core.stack_size(1);
        core.player
            .inventory
            .insert(1, core.player.carry_slots * stack);
        core.player.inventory.remove(&2);
        assert!(core.withdraw(2, 0, 2, 7).unwrap_err().contains("capacity"));
        assert_eq!(core.entities[index].inventory.get(&2), Some(&7));

        // A partial withdrawal takes exactly what the part-filled stack still has room for, and
        // says how much moved rather than pretending the request was met.
        core.player
            .inventory
            .insert(1, (core.player.carry_slots - 1) * stack);
        core.player.inventory.insert(2, 6);
        core.withdraw(2, 0, 2, 99).unwrap();
        assert_eq!(core.player.inventory.get(&2), Some(&core.stack_size(2)));
        assert_eq!(core.entities[index].inventory.get(&2), Some(&3));
        assert_eq!(core.events.last().unwrap(), "Withdrew 4 × Component");
    }

    #[test]
    fn multi_cell_footprints_drive_occupancy_snapshots_and_edit_targeting() {
        let mut core = game("new-game");
        core.researched.extend([1, 2, 3]);
        core.player.inventory.insert(1, 20);
        core.player.inventory.insert(3, 10);
        core.place(-2, 0, 3, 0, Some(1)).unwrap();
        let composer = core
            .snapshot()
            .buildings
            .into_iter()
            .find(|entity| entity.definition_id == 3)
            .unwrap();
        assert_eq!(
            composer.footprint,
            vec![Coordinate { q: -2, r: 0 }, Coordinate { q: -2, r: -1 }]
        );
        assert!(core
            .place(-2, -1, 2, 0, None)
            .unwrap_err()
            .contains("footprint"));
        core.erase(-2, -1).unwrap();
        assert!(core.entity_at(-2, 0).is_none());
    }

    #[test]
    fn extractor_stops_exactly_when_its_deposit_empties() {
        let mut core = game("new-game");
        core.researched.insert(2);
        core.player.inventory.insert(1, 4);
        core.player.inventory.insert(6, 2);
        set_player_hex(&mut core, 3, 1);
        core.write_overlay(3, 0, 1, 2, 48);
        core.place(3, 0, 1, 0, None).unwrap();
        for _ in 0..2 {
            core.tick_many(5);
            let index = core
                .entities
                .iter()
                .position(|entity| entity.placed.q == 3)
                .unwrap();
            assert!(core.entities[index].cargo.is_some());
            core.entities[index].cargo = None;
        }
        core.tick_many(100);
        let entity = core
            .entities
            .iter()
            .find(|entity| entity.placed.q == 3)
            .unwrap();
        assert_eq!(core.deposit_quantity((3, 0)), 0);
        assert_eq!(core.produced.get(&1), Some(&2));
        assert_eq!(entity.progress, 0);
    }

    #[test]
    fn resolved_deposit_references_match_a_full_tile_scan_and_survive_generation() {
        let mut core = game("new-game");
        core.researched.insert(2);
        core.player.inventory.insert(1, 8);
        core.player.inventory.insert(6, 2);
        set_player_hex(&mut core, 3, 1);
        core.place(3, 0, 1, 0, None).unwrap();
        let index = core
            .entities
            .iter()
            .position(|entity| entity.placed.q == 3 && entity.placed.r == 0)
            .unwrap();
        let scan = |core: &Core| {
            let (x, y) = axial_world(3, 0);
            core.resource_at_world(x, y)
        };

        let expected = scan(&core);
        assert_eq!(core.extractor_deposit(index), expected);
        assert_eq!(expected, Some((3, 0)));
        // The second lookup is served from the cache and must not drift from the scan.
        assert_eq!(core.extractor_deposit(index), scan(&core));
        assert_eq!(core.deposit_links.len(), 1);

        // Generating tiles invalidates every resolved reference, and the extractor re-resolves.
        core.generate_chunk(-9, 7);
        assert!(core.deposit_links.is_empty());
        assert_eq!(core.extractor_deposit(index), scan(&core));

        // A drained field cell falls through to the scan's next choice without re-resolving.
        core.write_overlay(3, 0, 1, 0, 48);
        assert_eq!(core.extractor_deposit(index), scan(&core));
        assert_eq!(core.extractor_deposit(index), None);

        // Erasing the extractor releases its entry rather than leaking one per placement.
        core.erase(3, 0).unwrap();
        assert!(core.deposit_links.is_empty());
    }

    #[test]
    fn research_is_atomic_validates_prerequisites_and_unlocks() {
        let mut core = game("new-game");
        core.insight = 20;
        assert!(core.research(2).unwrap_err().contains("prerequisites"));
        assert_eq!(core.insight, 20);
        core.research(1).unwrap();
        assert_eq!(core.insight, 17);
        core.research(2).unwrap();
        assert_eq!(core.insight, 12);
        core.player.inventory.insert(1, 1);
        core.place(2, 0, 2, 0, None).unwrap();
        assert!(core.research(2).is_err());
    }

    #[test]
    fn turning_demo_compiles_and_transport_recipe_backpressure_delivery_stay_exact() {
        let mut core = game("factory-demo");
        let mut index = core
            .entities
            .iter()
            .position(|entity| (entity.placed.q, entity.placed.r) == (-4, 0))
            .unwrap();
        let mut path = Vec::new();
        loop {
            path.push((core.entities[index].placed.q, core.entities[index].placed.r));
            let Some(next) = core.graph[index] else { break };
            index = next;
        }
        assert_eq!(
            path,
            vec![
                (-4, 0),
                (-3, 0),
                (-2, 0),
                (-2, 1),
                (-1, 1),
                (0, 1),
                (1, 1),
                (2, 1)
            ]
        );
        core.tick_many(400);
        let produced = core.produced.get(&1).copied().unwrap_or(0);
        let ore_in_system: u64 = core
            .entities
            .iter()
            .map(|entity| {
                u64::from(
                    entity
                        .cargo
                        .filter(|cargo| cargo.item_id == 1)
                        .map(|cargo| cargo.quantity)
                        .unwrap_or(0),
                ) + u64::from(entity.inventory.get(&1).copied().unwrap_or(0))
                    + u64::from(entity.reserved_inputs.get(&1).copied().unwrap_or(0))
            })
            .sum();
        let component_equivalent = core.delivered_by_item.get(&2).copied().unwrap_or(0) * 2;
        assert_eq!(produced, ore_in_system + component_equivalent);
        assert!(core.delivered > 0);
    }

    #[test]
    fn incremental_recompile_matches_full_graph_and_skips_unrelated_components() {
        let mut core = game("factory-demo");
        add_test_belt(&mut core, 100, 100, 0);
        add_test_belt(&mut core, 101, 100, 0);
        core.compile_graph();

        let index = core
            .entities
            .iter()
            .position(|entity| (entity.placed.q, entity.placed.r) == (-3, 0))
            .unwrap();
        let old_links = core.graph_links_by_id();
        let id = core.entities[index].id;
        let changed_cells = BTreeSet::from([(-3, 0)]);
        core.entities[index].placed.orientation = 1;

        let recompiled =
            core.recompile_graph_components(&old_links, &changed_cells, &BTreeSet::from([id]));
        assert!(recompiled > 0);
        assert!(recompiled < core.entities.len());
        let incremental = core.graph_links_by_id();
        core.compile_graph();
        assert_eq!(core.graph_links_by_id(), incremental);
        assert_eq!(
            incremental.get(&(core.next_entity_id - 2)),
            old_links.get(&(core.next_entity_id - 2))
        );
    }

    #[test]
    fn incremental_recompile_handles_component_splits_and_merges() {
        let mut core = game("new-game");
        core.entities.clear();
        core.graph.clear();
        core.next_entity_id = 1;
        let left = add_test_belt(&mut core, 0, 0, 0);
        let bridge = add_test_belt(&mut core, 1, 0, 0);
        let right = add_test_belt(&mut core, 2, 0, 0);
        core.compile_graph();
        assert_eq!(core.graph_links_by_id()[&left], Some(bridge));
        assert_eq!(core.graph_links_by_id()[&bridge], Some(right));

        let old_links = core.graph_links_by_id();
        let bridge_index = core
            .entities
            .iter()
            .position(|entity| entity.id == bridge)
            .unwrap();
        core.entities.remove(bridge_index);
        let changed_cells = BTreeSet::from([(1, 0)]);
        let recompiled =
            core.recompile_graph_components(&old_links, &changed_cells, &BTreeSet::from([bridge]));
        assert_eq!(recompiled, 2);
        assert_eq!(core.graph_links_by_id()[&left], None);
        let incremental_split = core.graph_links_by_id();
        core.compile_graph();
        assert_eq!(core.graph_links_by_id(), incremental_split);

        let old_links = core.graph_links_by_id();
        let replacement = add_test_belt(&mut core, 1, 0, 0);
        let recompiled = core.recompile_graph_components(
            &old_links,
            &changed_cells,
            &BTreeSet::from([replacement]),
        );
        assert_eq!(recompiled, 3);
        assert_eq!(core.graph_links_by_id()[&left], Some(replacement));
        assert_eq!(core.graph_links_by_id()[&replacement], Some(right));
        let incremental_merge = core.graph_links_by_id();
        core.compile_graph();
        assert_eq!(core.graph_links_by_id(), incremental_merge);
    }

    #[test]
    fn blocked_outputs_preserve_cargo_and_container_order_is_stable() {
        let mut core = game("factory-demo");
        let container = core
            .entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Container)
            .unwrap();
        let consumer = core
            .entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Consumer)
            .unwrap();
        core.graph[container] = Some(consumer);
        core.entities[container].inventory.insert(3, 2);
        core.entities[container].inventory.insert(1, 1);
        core.transfer_cargo();
        assert_eq!(core.delivered_by_item.get(&1), Some(&1));
        assert_eq!(core.entities[container].inventory.get(&3), Some(&2));
        core.entities[container].cargo = Some(Cargo {
            item_id: 2,
            quantity: 1,
        });
        let before = core.entities[container].cargo;
        core.graph[container] = None;
        core.transfer_cargo();
        assert_eq!(core.entities[container].cargo, before);
    }

    #[test]
    fn composer_consumes_exact_inputs_and_emits_only_after_integer_duration() {
        let mut core = game("factory-demo");
        let composer = core
            .entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Composer)
            .unwrap();
        core.graph[composer] = None;
        core.entities[composer].inventory.insert(1, 2);
        core.advance_composer(composer);
        assert!(core.entities[composer].inventory.is_empty());
        assert_eq!(core.entities[composer].reserved_inputs.get(&1), Some(&2));
        assert_eq!(core.entities[composer].cargo, None);
        for _ in 1..8 {
            core.advance_composer(composer);
        }
        assert_eq!(
            core.entities[composer].cargo,
            Some(Cargo {
                item_id: 2,
                quantity: 1
            })
        );
        assert!(core.entities[composer].reserved_inputs.is_empty());
        core.advance_composer(composer);
        assert_eq!(core.entities[composer].cargo.unwrap().quantity, 1);
    }

    #[test]
    fn machine_backpressure_and_consumer_totals_are_exact() {
        let mut core = game("factory-demo");
        let extractor = core
            .entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Extractor)
            .unwrap();
        core.graph[extractor] = None;
        let resource_before = core.deposit_quantity((-4, 0));
        core.tick_many(100);
        assert_eq!(core.entities[extractor].cargo.unwrap().quantity, 1);
        assert_eq!(core.deposit_quantity((-4, 0)), resource_before - 1);
        let container = core
            .entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Container)
            .unwrap();
        let consumer = core
            .entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Consumer)
            .unwrap();
        core.entities[container].inventory.insert(2, 7);
        core.graph[container] = Some(consumer);
        for _ in 0..7 {
            core.transfer_cargo();
        }
        assert_eq!(core.delivered_by_item.get(&2), Some(&7));
        assert!(core.entities[container].inventory.is_empty());
    }

    #[test]
    fn the_founding_contract_advances_stage_by_stage_and_victory_is_persistent() {
        let mut core = game("new-game");
        core.power_unmetered = false;
        set_player_hex(&mut core, 1, 0);
        // Research is funded by filling what the hub posted, one board row at a time. The opening
        // three are ore, stone, and wood, and each is worth ten insight.
        for (item, quantity) in [(1, 10), (6, 10), (9, 10)] {
            core.player.inventory.insert(item, quantity);
            core.deposit_inventory().unwrap();
        }
        assert_eq!(core.insight, 30);
        core.research(1).unwrap();
        core.research(8).unwrap();
        core.research(2).unwrap();
        core.research(3).unwrap();
        core.player.inventory.insert(1, 30);
        core.player.inventory.insert(3, 8);
        core.player.inventory.insert(5, 16);
        core.player.inventory.insert(6, 8);
        set_player_hex(&mut core, 3, 1);
        core.place(3, 0, 1, 3, None).unwrap();
        core.place(2, 0, 2, 3, None).unwrap();
        core.place(1, 0, 3, 3, Some(1)).unwrap();
        set_player_hex(&mut core, 6, 0);
        let pole = try_place_near(&mut core, (3, 0), 12);
        let burner = try_place_near(&mut core, pole, 13);
        try_place_near(&mut core, (1, 0), 12);
        let _ = burner;
        if let Some(burner) = core
            .entities
            .iter_mut()
            .find(|entity| entity.kind == BuildingKind::Generator)
        {
            burner.inventory.insert(5, 16);
        }
        core.tick_many(500);
        // The running line closes the first stage, and closing it is deliberately not the end of
        // the contract: the hub has grown once, and free play has not been declared yet.
        assert_eq!(core.contract_stage, 1);
        assert!(!core.victory);
        assert_eq!(core.contract_snapshot().stage_key, "foundry");
        // The foundry module, delivered by hand. What this pins is the stage machinery, not a
        // second smelting line: the bill is two items from two chains, and both have to arrive.
        set_player_hex(&mut core, 0, -1);
        core.player.inventory.insert(11, 16);
        core.deposit_inventory().unwrap();
        assert_eq!(core.contract_stage, 1, "half a bill is not a stage");
        assert!(!core.victory);
        core.player.inventory.insert(14, 20);
        core.deposit_inventory().unwrap();
        assert_eq!(core.contract_stage, 2);
        assert!(core.victory);
        // Nothing is left to ask for, and the requirement list says so rather than repeating the
        // last bill at full.
        assert!(core.contract_snapshot().requirements.is_empty());
        assert!(core.contract_snapshot().complete);
        let checksum = core.checksum();
        core.tick_many(1);
        assert!(core.victory);
        assert_ne!(core.checksum(), checksum);
    }

    #[test]
    fn a_stage_consumes_its_bill_and_carries_the_surplus_to_the_next_one() {
        let mut core = game("new-game");
        set_player_hex(&mut core, 0, -1);
        // Everything the whole contract asks for, in one delivery, plus one component too many.
        // The hub takes a later stage's materials as well as the current one's, which is the
        // surplus rule: a line automated early is credited when the stage that wants it arrives.
        core.player.inventory.insert(2, 4);
        core.player.inventory.insert(11, 16);
        core.player.inventory.insert(14, 20);
        core.deposit_inventory().unwrap();
        // Both stages close in the same delivery, which is the reason the advance loops rather
        // than closing one stage per arriving item.
        assert_eq!(core.contract_stage, 2);
        assert!(core.victory);
        // Each stage consumed exactly its own bill, and the fourth component was never taken at
        // all: the hub accepts what it asked for and leaves the rest in the pack.
        assert_eq!(core.contract_contributed.get(&2), Some(&0));
        assert_eq!(core.contract_contributed.get(&11), Some(&0));
        assert_eq!(core.contract_contributed.get(&14), Some(&0));
        assert_eq!(core.player.inventory.get(&2), Some(&1));
        // A finished contract does not close the hub. The board is still posting, filling a row is
        // still what pays, and no stage index runs off the end of the list.
        let insight = core.insight;
        core.player.inventory.insert(1, 10);
        core.deposit_inventory().unwrap();
        assert!(core.insight > insight);
        assert_eq!(core.contract_stage, 2);
    }

    /// The price is posted, and it is paid on completion — never before, and never for anything the
    /// hub did not ask for.
    #[test]
    fn a_request_pays_on_completion_and_the_board_moves_on() {
        let mut core = game("new-game");
        set_player_hex(&mut core, 1, 0);
        let board = |core: &Core| -> Vec<String> {
            core.request_snapshots()
                .iter()
                .map(|request| request.key.clone())
                .collect()
        };
        assert_eq!(board(&core), ["ore-assay", "cliff-stone", "cordwood"]);
        // Half a request is worth nothing. This is the whole difference from the currency it
        // replaced, where five ore was five insight and the player never saw the rate.
        core.player.inventory.insert(1, 5);
        core.deposit_inventory().unwrap();
        assert_eq!(core.insight, 0);
        assert_eq!(core.request_snapshots()[0].delivered, 5);
        core.player.inventory.insert(1, 5);
        core.deposit_inventory().unwrap();
        assert_eq!(core.insight, 10);
        // The slot that was filled holds the next row, in its own place: the board does not
        // shuffle, and it does not repost the row that was just paid for while others are unseen.
        assert_eq!(board(&core), ["clay-survey", "cliff-stone", "cordwood"]);
        assert_eq!(core.request_rounds.get(&1), Some(&1));
        assert_eq!(core.request_fills.get(&1), Some(&1));
    }

    /// Passing a row costs it a place in the queue, not its first-fill bonus. Skip used to share
    /// `request_rounds` with payment, which would have turned "I have not found this yet" into
    /// two insight for ten gathers.
    #[test]
    fn passing_a_request_does_not_burn_the_first_fill_bonus() {
        let mut core = game("new-game");
        set_player_hex(&mut core, 1, 0);
        core.skip_request(0).unwrap();
        assert_eq!(core.request_rounds.get(&1), Some(&1));
        assert!(core.request_fills.get(&1).is_none());
        core.requests[0] = RequestState {
            request_id: 1,
            delivered: 0,
        };
        let before = core.insight;
        core.player.inventory.insert(1, 10);
        core.deposit_inventory().unwrap();
        assert_eq!(
            core.insight - before,
            10,
            "a skipped row still pays its first fill"
        );
        assert_eq!(core.request_fills.get(&1), Some(&1));
    }

    /// A later fill of a raw row pays `repeat_insight`, not the opening survey.
    #[test]
    fn a_repeated_raw_request_pays_the_decayed_rate() {
        let mut core = game("new-game");
        set_player_hex(&mut core, 1, 0);
        core.player.inventory.insert(1, 10);
        core.deposit_inventory().unwrap();
        assert_eq!(core.insight, 10);
        // Force ore-assay back onto the board and fill it again.
        core.requests[0] = RequestState {
            request_id: 1,
            delivered: 0,
        };
        core.player.inventory.insert(1, 10);
        core.deposit_inventory().unwrap();
        assert_eq!(core.insight, 12);
        assert_eq!(core.request_fills.get(&1), Some(&2));
    }

    /// The hub takes what it asked for and leaves the rest in the pack — by hand and by belt, at one
    /// predicate, so a line cannot void cargo the key would have refused.
    #[test]
    fn the_hub_refuses_what_nobody_asked_for() {
        let mut core = game("new-game");
        set_player_hex(&mut core, 1, 0);
        core.player.inventory.insert(3, 6);
        assert!(core
            .deposit_inventory()
            .unwrap_err()
            .contains("not asking for anything"));
        assert_eq!(core.player.inventory.get(&3), Some(&6));
        let hub = core
            .entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Hub)
            .expect("the landing hub");
        assert!(!core.can_accept(
            hub,
            Cargo {
                item_id: 3,
                quantity: 1
            }
        ));
        assert!(core.can_accept(
            hub,
            Cargo {
                item_id: 1,
                quantity: 1
            }
        ));
        // Ten ore is the whole standing order, so the eleventh has nowhere to go either.
        core.player.inventory.insert(1, 10);
        core.deposit_inventory().unwrap();
        assert!(!core.can_accept(
            hub,
            Cargo {
                item_id: 1,
                quantity: 1
            }
        ));
    }

    /// The board is drawn from the rules, so it can never post something the rules refuse.
    #[test]
    fn the_board_only_posts_what_the_player_could_make() {
        let mut core = game("new-game");
        assert!(core.item_reachable(1, 0), "ore is in the ground");
        assert!(
            !core.item_reachable(11, 0),
            "a plate needs a smelter nobody may build yet"
        );
        assert!(
            !core.item_reachable(10, 0),
            "water needs a pump, and water is nobody's field"
        );
        assert!(
            !core.item_reachable(CRYSTAL, 0),
            "signal crystal is machine only until an extractor is unlocked"
        );
        // Passing every slot repeatedly walks the whole eligible list. Nothing that needs a machine
        // may appear in it, however far up the catalogue that row stands.
        for _ in 0..12 {
            for slot in 0..REQUEST_SLOTS {
                let item = core.request_snapshots()[slot].item_id;
                assert!(
                    core.item_reachable(item, 0),
                    "the board posted item {item}, which cannot be produced yet"
                );
                core.skip_request(slot).unwrap();
            }
        }
        core.insight = 100;
        for technology in [1, 2, 5] {
            core.research(technology).unwrap();
        }
        assert!(core.item_reachable(11, 0), "the smelter unlocks the plate");
        assert!(
            core.item_reachable(CRYSTAL, 0),
            "an extractor unlocks the crystal field"
        );
    }

    /// Passing a row costs it a place in the queue, and costs the player whatever they had already
    /// put against it. It is a decision, not a free reroll.
    #[test]
    fn passing_a_request_forfeits_it_and_puts_it_behind_the_unseen() {
        let mut core = game("new-game");
        set_player_hex(&mut core, 1, 0);
        core.player.inventory.insert(1, 5);
        core.deposit_inventory().unwrap();
        assert_eq!(core.request_snapshots()[0].delivered, 5);
        core.skip_request(0).unwrap();
        assert_eq!(core.request_snapshots()[0].key, "clay-survey");
        assert_eq!(core.request_snapshots()[0].delivered, 0);
        assert_eq!(core.insight, 0);
        assert!(core.skip_request(9).unwrap_err().contains("no request"));
    }

    /// Once a smelter is unlocked, a free slot is reserved for the deepest reachable row rather
    /// than the next unseen ore assay. The other two slots still cycle, and nothing unmakeable is
    /// posted — reservation walks the same `item_reachable` predicate the rest of the board does.
    #[test]
    fn the_board_reserves_one_slot_for_the_deepest_reachable_row() {
        let mut core = game("new-game");
        set_player_hex(&mut core, 1, 0);
        core.insight = 100;
        for technology in [1, 2, 5] {
            core.research(technology).unwrap();
        }
        assert!(core.item_reachable(11, 0));
        let before: Vec<String> = core
            .request_snapshots()
            .iter()
            .map(|request| request.key.clone())
            .collect();
        assert!(
            before.iter().all(|key| {
                let item = core
                    .definitions
                    .requests
                    .iter()
                    .find(|request| request.key == *key)
                    .map(|request| request.item_id)
                    .unwrap();
                core.item_depth(item) == 0
            }),
            "the opening board is raw, got {before:?}"
        );
        core.player.inventory.insert(1, 10);
        core.deposit_inventory().unwrap();
        let after: Vec<String> = core
            .request_snapshots()
            .iter()
            .map(|request| request.key.clone())
            .collect();
        let depths: Vec<u32> = after
            .iter()
            .map(|key| {
                let item = core
                    .definitions
                    .requests
                    .iter()
                    .find(|request| request.key == *key)
                    .map(|request| request.item_id)
                    .unwrap();
                core.item_depth(item)
            })
            .collect();
        assert!(
            depths.iter().any(|&depth| depth > 0),
            "the freed slot should post the deepest reachable row, got {after:?} at {depths:?}"
        );
        for request in core.request_snapshots() {
            assert!(
                core.item_reachable(request.item_id, 0),
                "reserved slot posted item {}, which cannot be produced",
                request.item_id
            );
        }
    }

    #[test]
    fn deposit_can_deliver_an_individual_item_leaving_other_demanded_items_in_pack() {
        let mut core = game("new-game");
        // Give player iron ore (id 1) and wood (id 8). Both are standing requests in new game.
        core.player.inventory.insert(1, 10);
        core.player.inventory.insert(8, 10);
        set_player_hex(&mut core, 0, 1);
        // Deliver only iron ore
        core.deposit_item(Some(1)).unwrap();
        // Iron ore was delivered, wood remains in pack
        assert_eq!(core.player.inventory.get(&1), None);
        assert_eq!(core.player.inventory.get(&8), Some(&10));
    }

    /// A board is saved state, restored rather than redrawn.
    #[test]
    fn a_save_restores_the_board_it_was_holding() {
        let (definitions, technologies, scenarios) = catalogs();
        let mut core = game("new-game");
        set_player_hex(&mut core, 1, 0);
        core.player.inventory.insert(1, 10);
        core.player.inventory.insert(6, 4);
        core.deposit_inventory().unwrap();
        let before = core.request_snapshots();
        let save = core.save_string().unwrap();
        let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
        assert_eq!(restored.request_snapshots(), before);
        assert_eq!(restored.request_rounds, core.request_rounds);
        assert_eq!(restored.request_fills, core.request_fills);
        assert_eq!(restored.insight, 10);
        // A row this build does not ship would survive the file and then be drawn as a request
        // nobody can read, so the loader refuses it before the checksum ever gets the chance.
        let forged = save.replace("\"request_id\":4", "\"request_id\":9999");
        assert_ne!(forged, save);
        let refusal = Core::from_save(&definitions, &technologies, &scenarios, &forged)
            .err()
            .expect("a forged board is refused");
        assert!(refusal.contains("unknown hub request"), "{refusal}");
    }

    #[test]
    fn hxf1_round_trip_and_resume_match_uninterrupted_run() {
        let (definitions, technologies, scenarios) = catalogs();
        let mut uninterrupted = game("factory-demo");
        // Metered on both sides, which is the shipped rule and the only way this test is honest.
        // `power_unmetered` is a harness hook that no save carries, so a resumed core always comes
        // back metered; leaving the running one unmetered compared two different games. It passed
        // until v0.19 only because a fully supplied grid used to make the two paths agree by
        // arithmetic — with banked energy they no longer do, and the resume is exactly what should
        // catch that.
        uninterrupted.power_unmetered = false;
        uninterrupted.tick_many(120);
        let save = uninterrupted.save_string().unwrap();
        assert!(save.starts_with(SAVE_PREFIX));
        let mut resumed = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
        uninterrupted.tick_many(180);
        resumed.tick_many(180);
        assert_eq!(uninterrupted.checksum(), resumed.checksum());
        assert_eq!(uninterrupted.delivered, resumed.delivered);
        assert!(Core::from_save(&definitions, &technologies, &scenarios, "bad").is_err());
        // Written against the live version rather than a literal, so bumping a version is a
        // one-line change in one place and this test keeps testing the rejection it names.
        let incompatible = save.replacen(
            &format!("\"definition_version\":{}", definitions.version),
            "\"definition_version\":999",
            1,
        );
        assert!(Core::from_save(&definitions, &technologies, &scenarios, &incompatible).is_err());
        // v0.14 bumps the envelope because `orientation` now indexes eight routing directions and
        // definitions carry a tier. The previous envelope is rejected, not reinterpreted.
        let previous_envelope = save.replacen(
            &format!("\"save_version\":{SAVE_VERSION}"),
            &format!("\"save_version\":{}", SAVE_VERSION - 1),
            1,
        );
        assert!(
            Core::from_save(&definitions, &technologies, &scenarios, &previous_envelope).is_err(),
            "a v0.13 save must be refused rather than read with six-direction orientations"
        );
        // v0.16 takes the generator to 6 because `WorldParams` entered the envelope and the
        // checksum. A version-5 envelope names no parameters at all, so it cannot be read as the
        // default set — it is rejected.
        let old_world = save.replacen(
            &format!("\"world_generator_version\":{WORLD_GENERATOR_VERSION}"),
            &format!(
                "\"world_generator_version\":{}",
                WORLD_GENERATOR_VERSION - 1
            ),
            1,
        );
        assert!(Core::from_save(&definitions, &technologies, &scenarios, &old_world).is_err());
        // The parameters are checksummed, so editing them in a saved file is caught as tampering
        // rather than quietly regenerating a different world under the same overlay.
        let edited_params = save.replacen("\"water_level\":18000", "\"water_level\":19000", 1);
        assert_ne!(edited_params, save, "the save carries its world parameters");
        assert!(Core::from_save(&definitions, &technologies, &scenarios, &edited_params).is_err());
    }

    #[test]
    fn reset_replay_and_scenario_insertion_order_are_deterministic() {
        let (definitions, technologies, scenarios) = catalogs();
        let scenario = scenarios
            .scenarios
            .iter()
            .find(|value| value.key == "factory-demo")
            .unwrap();
        let mut reversed = scenario.clone();
        reversed.buildings.reverse();
        let mut a = Core::new(&definitions, &technologies, scenario, None, None).unwrap();
        let mut b = Core::new(&definitions, &technologies, &reversed, None, None).unwrap();
        a.tick_many(300);
        b.tick_many(300);
        assert_eq!(a.checksum(), b.checksum());
        let expected = a.checksum();
        let mut replay = Core::new(&definitions, &technologies, scenario, None, None).unwrap();
        replay.tick_many(300);
        assert_eq!(replay.checksum(), expected);
    }

    /// Every status spelling the host can render. The wire carries the index, so a reordering here
    /// is a wire break; the fixture is what makes that break visible in both languages at once.
    const WIRE_STATUSES: [(EntityStatus, &str); 17] = [
        (EntityStatus::OutputBlocked, "output blocked"),
        (EntityStatus::DepositDepleted, "deposit depleted"),
        (EntityStatus::Extracting, "extracting"),
        (EntityStatus::NoWaterInReach, "no water in reach"),
        (EntityStatus::Pumping, "pumping"),
        (EntityStatus::Composing, "composing"),
        (EntityStatus::OutOfFuel, "out of fuel"),
        (EntityStatus::WaitingForInputs, "waiting for inputs"),
        (EntityStatus::Buffered, "buffered"),
        (EntityStatus::Carrying, "carrying"),
        (EntityStatus::Receiving, "receiving"),
        (EntityStatus::LandingHub, "landing hub"),
        (EntityStatus::Idle, "idle"),
        (EntityStatus::NoPower, "no power"),
        (EntityStatus::Generating, "generating"),
        (EntityStatus::Brownout, "brownout"),
        (EntityStatus::NoBoiler, "no boiler"),
    ];

    const WIRE_KINDS: [(BuildingKind, &str); 11] = [
        (BuildingKind::Extractor, "extractor"),
        (BuildingKind::Belt, "belt"),
        (BuildingKind::Composer, "composer"),
        (BuildingKind::Container, "container"),
        (BuildingKind::Consumer, "consumer"),
        (BuildingKind::Hub, "hub"),
        (BuildingKind::Pump, "pump"),
        (BuildingKind::Pole, "pole"),
        (BuildingKind::Generator, "generator"),
        (BuildingKind::Boiler, "boiler"),
        (BuildingKind::Bridge, "bridge"),
    ];

    const WIRE_TERRAIN: [(Terrain, &str); 7] = [
        (Terrain::DeepWater, "deep_water"),
        (Terrain::ShallowWater, "shallow_water"),
        (Terrain::Shore, "shore"),
        (Terrain::Lowland, "lowland"),
        (Terrain::Hills, "hills"),
        (Terrain::Highland, "highland"),
        (Terrain::Cliff, "cliff"),
    ];

    #[test]
    fn entity_status_spellings_are_what_the_host_renders() {
        // The enum exists so the wire can carry a byte, but what reaches the player is still the
        // string. Renaming a variant is allowed; changing its spelling changes the game's text.
        for (status, spelling) in WIRE_STATUSES {
            assert_eq!(
                serde_json::to_value(status).unwrap(),
                serde_json::Value::String(spelling.to_owned()),
                "status spelling changed"
            );
        }
    }

    fn hex_encode(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// Deltas chosen to walk the whole surface of the encoding rather than to look like a frame:
    /// an empty group mask, every scalar group at once, both patch kinds carrying entries, and the
    /// replace form with nothing in it.
    fn wire_fixture_cases() -> Vec<(&'static str, SnapshotDelta)> {
        // A closure rather than a value: every case below fills a different handful of groups in
        // and leaves the rest absent, and `..` moves what it spreads from.
        let empty = || SnapshotDelta {
            base_revision: 0,
            revision: 1,
            tick: 0,
            checksum: 0,
            scenario: None,
            scenario_name: None,
            world_version: None,
            seed: None,
            delivered: None,
            delivered_by_item: None,
            insight: None,
            victory: None,
            contract: None,
            requests: None,
            player: None,
            researched: None,
            chunks: None,
            terrain: None,
            resources: None,
            buildings: None,
            events: None,
        };

        // A frame that changed nothing but the clock. The mask is zero and the body is empty, which
        // is the case a quiet factory spends most of its frames in.
        let quiet = SnapshotDelta {
            base_revision: 41,
            revision: 42,
            tick: 1_000_000,
            checksum: 0xdead_beef,
            ..empty()
        };

        // Every scalar group, with negative coordinates and multi-byte varints, so a decoder that
        // reads a field in the wrong order or forgets to zigzag cannot pass.
        let scalars = SnapshotDelta {
            base_revision: 2,
            revision: 3,
            tick: 300,
            checksum: 7,
            scenario: Some("new-game".to_owned()),
            scenario_name: Some("New game".to_owned()),
            world_version: Some(5),
            seed: Some(4_294_967_295),
            // Exactly 2^53 - 1. The invariant is that nothing wider than that travels as a number,
            // and the host still receives these as JavaScript numbers, so the boundary itself is
            // the largest value worth pinning — a fixture above it would pin rounding, not the
            // encoding.
            delivered: Some(9_007_199_254_740_991),
            delivered_by_item: Some(vec![
                Ingredient64 {
                    item_id: 1,
                    quantity: 1_000_000_000_000,
                },
                Ingredient64 {
                    item_id: 300,
                    quantity: 0,
                },
            ]),
            insight: Some(64_000),
            victory: Some(true),
            // A multi-line bill with one line over-delivered and one untouched, so a decoder that
            // loses the count, swaps `delivered` and `required`, or reads the trailing flag before
            // the list cannot pass.
            contract: Some(ContractSnapshot {
                key: "founding".to_owned(),
                name: "Founding contract".to_owned(),
                stage: 1,
                stages: 2,
                stage_key: "foundry".to_owned(),
                stage_name: "Raise the foundry module".to_owned(),
                stage_brief: "Plate and brick, from two landscapes.".to_owned(),
                requirements: vec![
                    ContractRequirement {
                        item_id: 11,
                        delivered: 16,
                        required: 16,
                    },
                    ContractRequirement {
                        item_id: 14,
                        delivered: 0,
                        required: 20,
                    },
                ],
                complete: false,
            }),
            // A board with one row part-filled and one untouched, so a decoder that loses the
            // count, swaps `delivered` and `required`, or reads the price before the numbers cannot
            // pass. The brief carries the multi-byte case the events list carries too.
            requests: Some(vec![
                RequestSnapshot {
                    key: "plate-stock".to_owned(),
                    name: "Plate stock".to_owned(),
                    brief: "Smelted iron — not ore.".to_owned(),
                    item_id: 11,
                    delivered: 3,
                    required: 8,
                    insight: 22,
                },
                RequestSnapshot {
                    key: "cliff-stone".to_owned(),
                    name: "Cliff stone".to_owned(),
                    brief: "Cut stone for the apron.".to_owned(),
                    item_id: 6,
                    delivered: 0,
                    required: 10,
                    insight: 10,
                },
            ]),
            player: Some(PlayerSnapshot {
                state: PlayerState {
                    x: -123_456,
                    y: 654_321,
                    facing_x: -1000,
                    facing_y: 866,
                    move_x: 0,
                    move_y: -1,
                    inventory: BTreeMap::from([(1, 40), (3, 20), (65_535, 1)]),
                    action_cooldown: 5,
                    build_range: 4096,
                    carry_slots: 12,
                },
                carry_stacks: vec![
                    Ingredient {
                        item_id: 1,
                        quantity: 40,
                    },
                    Ingredient {
                        item_id: 3,
                        quantity: 20,
                    },
                ],
                radius: 580,
                action_cooldown_total: 6,
                extract_radius: 1,
            }),
            researched: Some(vec![1, 2, 3, 4]),
            chunks: Some(vec![
                ChunkSnapshot {
                    chunk_q: 0,
                    chunk_r: 0,
                    entity_count: 3,
                    x: -8192,
                    y: -8192,
                    span: 16_384,
                },
                ChunkSnapshot {
                    chunk_q: -2,
                    chunk_r: 1,
                    entity_count: 0,
                    x: -40_960,
                    y: 8192,
                    span: 16_384,
                },
            ]),
            events: Some(vec![
                "Gathered Iron ore".to_owned(),
                // Multi-byte UTF-8, because the string length is written in bytes and a decoder
                // that reads it as characters would desynchronise the rest of the buffer.
                "Delivered 3 × Steel — objective met".to_owned(),
            ]),
            ..empty()
        };

        // Both patches carrying entries: a bare belt beside a machine with every option set, a
        // removal list, a deposit patch over negative coordinates, and terrain.
        let patches = SnapshotDelta {
            base_revision: 10,
            revision: 11,
            tick: 512,
            checksum: 0x0102_0304,
            terrain: Some(vec![
                TileSnapshot {
                    q: -3,
                    r: -4,
                    x: -8_870,
                    y: -6_144,
                    radius: 1024,
                    terrain: Terrain::Cliff,
                },
                TileSnapshot {
                    q: -2,
                    r: -4,
                    x: -7_096,
                    y: -6_144,
                    radius: 1024,
                    terrain: Terrain::DeepWater,
                },
            ]),
            resources: Some(ResourcesDelta {
                replace: false,
                changed: vec![
                    ResourceSnapshot {
                        q: -32,
                        r: 0,
                        x: -56_768,
                        y: 0,
                        radius: 1024,
                        item_id: 1,
                        quantity: 0,
                        initial_quantity: 48,
                    },
                    ResourceSnapshot {
                        q: -32,
                        r: 3,
                        x: -54_107,
                        y: 4_608,
                        radius: 1024,
                        item_id: 2,
                        quantity: 17,
                        initial_quantity: 60,
                    },
                ],
            }),
            buildings: Some(BuildingsDelta {
                replace: false,
                changed: vec![
                    EntitySnapshot {
                        id: 7,
                        q: 2,
                        r: 0,
                        definition_id: 2,
                        kind: BuildingKind::Belt,
                        orientation: 3,
                        recipe_id: None,
                        scenario_owned: false,
                        cargo: None,
                        inventory: Vec::new(),
                        progress: 0,
                        progress_total: 0,
                        fuel_charge: 0,
                        fuel_required: 0,
                        power_satisfied: 0,
                        power_demand: 0,
                        // A belt sets no high flag, so its flag field is still the one byte it was
                        // before the field became a uvarint. That is the whole point of the change
                        // and this entity is what pins it.
                        power_charge: 0,
                        power_capacity: 0,
                        status: EntityStatus::Idle,
                        next_id: None,
                        footprint: vec![Coordinate { q: 2, r: 0 }],
                    },
                    EntitySnapshot {
                        id: 4_294_967_295,
                        q: -1,
                        r: 6,
                        definition_id: 3,
                        kind: BuildingKind::Composer,
                        orientation: 5,
                        recipe_id: Some(11),
                        scenario_owned: true,
                        cargo: Some(Cargo {
                            item_id: 4,
                            quantity: 2,
                        }),
                        inventory: vec![
                            Ingredient {
                                item_id: 1,
                                quantity: 6,
                            },
                            Ingredient {
                                item_id: 5,
                                quantity: 300,
                            },
                        ],
                        progress: 17,
                        progress_total: 40,
                        fuel_charge: 250,
                        fuel_required: 100,
                        power_satisfied: 8,
                        power_demand: 12,
                        // Both high bits set, so this entity's flag field is two bytes and the
                        // fixture carries a decoder that has to widen past the old fixed byte.
                        power_charge: 96,
                        power_capacity: 360,
                        status: EntityStatus::Composing,
                        next_id: Some(9),
                        // A multi-cell footprint, coded against the entity's own hex.
                        footprint: vec![
                            Coordinate { q: -1, r: 6 },
                            Coordinate { q: 0, r: 6 },
                            Coordinate { q: -1, r: 7 },
                        ],
                    },
                ],
                removed: vec![1, 2, 900],
            }),
            ..empty()
        };

        // The full-replace form both patches take on the first frame, a reset, a new game, and a
        // load — here with nothing in it, so the replace flag is what is being read rather than
        // the entries after it.
        let replace = SnapshotDelta {
            base_revision: 0,
            revision: 1,
            tick: 0,
            checksum: 1,
            resources: Some(ResourcesDelta {
                replace: true,
                changed: Vec::new(),
            }),
            buildings: Some(BuildingsDelta {
                replace: true,
                changed: Vec::new(),
                removed: Vec::new(),
            }),
            events: Some(Vec::new()),
            ..empty()
        };

        vec![
            ("a quiet frame", quiet),
            ("every scalar group", scalars),
            ("both patches with entries", patches),
            ("the empty full replace", replace),
        ]
    }

    /// The one artifact both languages are pinned to, in the same role
    /// `fixtures/hex-directions.json` plays for the direction table.
    ///
    /// Rust asserts it encodes these deltas to exactly these bytes and serializes them to exactly
    /// this JSON. `tests/snapshotWire.test.ts` asserts the shipped TypeScript decoder turns those
    /// same bytes back into that same JSON. Together they say the binary path delivers what the
    /// JSON path delivered, which is the whole claim of the encoding.
    ///
    /// Regenerate with `UPDATE_WIRE_FIXTURE=1 cargo test wire_fixture` and read the diff: a change
    /// here is a wire break, and the decoder on the other side has to move with it.
    #[test]
    fn wire_fixture_pins_the_format_for_both_languages() {
        let cases: Vec<serde_json::Value> = wire_fixture_cases()
            .into_iter()
            .map(|(name, delta)| {
                serde_json::json!({
                    "name": name,
                    "bytes": hex_encode(&wire::encode_delta(&delta)),
                    "delta": serde_json::to_value(&delta).unwrap(),
                })
            })
            .collect();
        let generated = serde_json::json!({
            "magic": std::str::from_utf8(&wire::WIRE_MAGIC).unwrap(),
            "version": wire::WIRE_VERSION,
            "kinds": WIRE_KINDS.map(|(_, name)| name).to_vec(),
            "terrain": WIRE_TERRAIN.map(|(_, name)| name).to_vec(),
            "statuses": WIRE_STATUSES.map(|(_, name)| name).to_vec(),
            "cases": cases,
        });

        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures/snapshot-delta-wire.json");
        if std::env::var("UPDATE_WIRE_FIXTURE").is_ok() {
            let mut text = serde_json::to_string_pretty(&generated).unwrap();
            text.push('\n');
            std::fs::write(&path, text).unwrap();
        }
        let recorded: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect(
                "fixtures/snapshot-delta-wire.json exists — regenerate with UPDATE_WIRE_FIXTURE=1",
            ))
            .unwrap();
        assert_eq!(
            generated, recorded,
            "the wire format moved; the TypeScript decoder has to move with it"
        );
    }

    /// The economy's own fixture, in the role `fixtures/hex-directions.json` plays for the
    /// direction table and `fixtures/snapshot-delta-wire.json` plays for the wire.
    ///
    /// Balance was the one system here with no representation: the costs were data, but every
    /// figure that decides whether the data works — items per minute, what a generator carries,
    /// what a building costs once its inputs are expanded to raw materials — existed nowhere and
    /// was checked by nothing. This is that file. Rust computes it from the shipped catalogues and
    /// `tests/balance.test.ts` recomputes the cost trees in TypeScript against the same
    /// `definitions.json`, so the recorded numbers are pinned by two independent expansions rather
    /// than by one implementation agreeing with its own output.
    ///
    /// Regenerate with `UPDATE_BALANCE_FIXTURE=1 cargo test balance_fixture`, then
    /// `npx prettier --write fixtures/balance.json` because serde and prettier disagree about
    /// short arrays, and read the diff: a change here is a change to what the game plays like, and
    /// it should be one somebody meant. The comparison is over parsed JSON, so the formatting pass
    /// cannot change what the test asserts.
    #[test]
    fn balance_fixture_pins_the_economy_for_both_languages() {
        let generated = serde_json::to_value(balance::compute()).unwrap();
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/balance.json");
        if std::env::var("UPDATE_BALANCE_FIXTURE").is_ok() {
            let mut text = serde_json::to_string_pretty(&generated).unwrap();
            text.push('\n');
            std::fs::write(&path, text).unwrap();
        }
        let recorded: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&path)
                .expect("fixtures/balance.json exists — regenerate with UPDATE_BALANCE_FIXTURE=1"),
        )
        .unwrap();
        assert_eq!(
            generated, recorded,
            "the economy moved; say so in the plan and regenerate the fixture"
        );
    }

    /// The stated curve, and the proof that stating it is not the same as describing it.
    ///
    /// Two rules, both claims about the data rather than about taste. A tier costs strictly more
    /// than the tier it upgrades from, and a machine costs no less than a machine of the same kind
    /// whose technology it is unlocked behind. The negative case is the point: put the cutter's
    /// stone back to the four it shipped with through v0.16 and the curve breaks, because a cutter
    /// two technologies past a smelter cost less than the smelter.
    #[test]
    fn every_step_of_the_curve_holds_and_a_broken_one_is_caught() {
        let report = balance::compute();
        assert!(!report.curve.is_empty());
        for step in &report.curve {
            assert!(
                step.holds,
                "{} ({}) follows {} ({}) by {} and does not cost more",
                step.building,
                step.effort_milli,
                step.follows,
                step.follows_effort_milli,
                step.relation
            );
        }

        let mut broken: DefinitionsInput = serde_json::from_str(DEFINITIONS).unwrap();
        let technologies: TechnologiesInput = serde_json::from_str(TECHNOLOGIES).unwrap();
        let cutter = broken
            .buildings
            .iter_mut()
            .find(|building| building.key == "cutter")
            .expect("the cutter is in the catalogue");
        let stone = cutter
            .construction_cost
            .iter_mut()
            .find(|ingredient| ingredient.item_id == STONE)
            .expect("a cutter is built out of stone");
        stone.quantity = 4;
        let broken = balance::compute_from(broken, technologies);
        assert!(
            broken.curve.iter().any(|step| !step.holds),
            "a cheaper-than-its-predecessor building has to fail the curve"
        );
    }

    /// The two rates a player compares without being told they are comparing them: their own
    /// hands, and the first machine that replaces them.
    ///
    /// These are measured against the same wall clock and they must not invert. Through v0.16 the
    /// hand ran at 300 items a minute against an extractor's 120, so the first automation in the
    /// game was two and a half times slower than doing it yourself. v0.17 made them equal at
    /// fifteen steps. v0.23 keeps the guard and adds the incentive: the hand is never *faster*
    /// than an extractor on the same cells, wood still matches it, and hard rock is materially
    /// slower. Crystal has no hand rate.
    #[test]
    fn no_extractor_is_slower_than_the_hand_on_the_same_cells() {
        let report = balance::compute();
        let extractor = report
            .machines
            .iter()
            .find(|machine| machine.building == "extractor")
            .expect("the extractor is a machine");
        assert!(
            !report.reference.hand_gathers.is_empty(),
            "the hand still takes something"
        );
        for gather in &report.reference.hand_gathers {
            assert!(
                extractor.per_minute_milli >= u64::from(gather.items_per_minute) * 1000,
                "{} at {} /min is faster than the extractor at {}",
                gather.item,
                gather.items_per_minute,
                extractor.per_minute_milli
            );
        }
        let wood = report
            .reference
            .hand_gathers
            .iter()
            .find(|gather| gather.item == "wood")
            .expect("wood is the fastest hand");
        assert_eq!(
            u64::from(wood.items_per_minute) * 1000,
            extractor.per_minute_milli
        );
        assert!(
            report
                .reference
                .hand_gathers
                .iter()
                .any(|gather| gather.item == "ore"
                    && gather.items_per_minute < wood.items_per_minute),
            "hard rock has to be slower than wood"
        );
        assert!(
            !report
                .reference
                .hand_gathers
                .iter()
                .any(|gather| gather.item == "crystal"),
            "signal crystal is machine only"
        );
        // Both work the same seven cells: reach is what an upgrade buys, never what a hand grows.
        assert_eq!(report.reference.cells_in_reach.first(), Some(&7));
    }

    /// A fuel recipe that hands back the energy it was given is a recipe with no reason to run.
    ///
    /// Charcoal was exactly that: two wood at two energy each into one charcoal at four, for a
    /// kiln, ten ticks, and a hundred power. Fuel is a property of the item, so this is the one
    /// place the round trip can be checked at all — nothing in a recipe row knows what its inputs
    /// burn for.
    #[test]
    fn every_fuel_conversion_ends_up_ahead() {
        let report = balance::compute();
        let converted: Vec<_> = report
            .fuel
            .iter()
            .filter(|entry| entry.recipe.is_some())
            .collect();
        assert!(!converted.is_empty(), "some fuel is crafted");
        for entry in converted {
            assert!(
                entry.gain_milli.unwrap_or(0) > 1000,
                "{} returns {} energy for {} — it costs a machine to break even",
                entry.item,
                entry.output_energy,
                entry.input_energy
            );
        }
    }

    /// Processing has to pay for itself, and the request board is where it is paid.
    ///
    /// A request that pays no better per gather than a raw one is a request nobody would ever build
    /// a machine for: the smelter costs research, construction, power, and fuel, and the hub would
    /// be offering the same rate for two ore as for the plate they became. So every row whose item
    /// comes out of a recipe has to beat every row whose item comes out of the ground — measured
    /// through the whole tree, fuel included, which is the only comparison that is not a guess.
    #[test]
    fn every_processed_request_pays_better_per_gather_than_raw_material() {
        let report = balance::compute();
        let raw: Vec<_> = report
            .requests
            .iter()
            .filter(|request| request.machine_ticks == 0)
            .collect();
        let processed: Vec<_> = report
            .requests
            .iter()
            .filter(|request| request.machine_ticks > 0)
            .collect();
        assert!(raw.len() >= 7, "the eight opening materials, less water");
        assert!(processed.len() >= 10, "a ladder, not one processed row");
        let best_raw = raw
            .iter()
            .map(|request| request.insight_per_gather_milli)
            .max()
            .expect("a raw request");
        for request in processed {
            assert!(
                request.insight_per_gather_milli > best_raw,
                "{} pays {} insight per thousand gathers and the best raw row pays {} — nobody \
                 would build the machine",
                request.request,
                request.insight_per_gather_milli,
                best_raw
            );
        }
    }

    /// The first cycle of every raw row is what funds the early tree. Repeating those rows is
    /// a floor, not a path: worse per gather *and* per minute than every processed row, measured
    /// against the new hand rates. Machine-only raw (crystal) is not a hand grind and is not in
    /// this comparison.
    #[test]
    fn a_repeated_raw_row_pays_worse_than_every_processed_row() {
        let report = balance::compute();
        let handable: BTreeSet<_> = report
            .reference
            .hand_gathers
            .iter()
            .map(|gather| gather.item.as_str())
            .collect();
        let raw: Vec<_> = report
            .requests
            .iter()
            .filter(|request| {
                request.machine_ticks == 0 && handable.contains(request.item.as_str())
            })
            .collect();
        let processed: Vec<_> = report
            .requests
            .iter()
            .filter(|request| request.machine_ticks > 0)
            .collect();
        assert!(
            raw.len() >= 7,
            "the eight opening materials, less water and crystal"
        );
        let best_repeat_gather = raw
            .iter()
            .map(|request| request.repeat_insight_per_gather_milli)
            .max()
            .expect("a raw request");
        let best_repeat_minute = raw
            .iter()
            .map(|request| request.repeat_insight_per_minute_milli)
            .max()
            .expect("a raw request");
        for request in processed {
            assert!(
                request.insight_per_gather_milli > best_repeat_gather,
                "{} pays {} /gather against a repeated raw row at {}",
                request.request,
                request.insight_per_gather_milli,
                best_repeat_gather
            );
            assert!(
                request.insight_per_minute_milli > best_repeat_minute,
                "{} pays {} /min against a repeated raw row at {}",
                request.request,
                request.insight_per_minute_milli,
                best_repeat_minute
            );
        }
    }

    /// One cycle of every raw request, first fill, is less than the technology tree. Repeats exist
    /// so a player without fuel is not stranded; they are not a way to finish research.
    #[test]
    fn the_technology_tree_cannot_be_funded_by_one_cycle_of_raw_requests() {
        let report = balance::compute();
        let tree: u32 = {
            let technologies: TechnologiesInput = serde_json::from_str(TECHNOLOGIES).unwrap();
            technologies
                .technologies
                .iter()
                .map(|technology| technology.cost)
                .sum()
        };
        let raw_cycle: u32 = report
            .requests
            .iter()
            .filter(|request| request.machine_ticks == 0)
            .map(|request| request.insight)
            .sum();
        assert!(
            raw_cycle < tree,
            "one cycle of raw requests pays {raw_cycle} and the tree costs {tree}"
        );
        assert!(tree >= 113, "the tree grew, it must not have shrunk");
    }

    /// Every material the economy bottoms out in can actually be had, from the site the game
    /// starts you on, under the preset it starts you in.
    ///
    /// Two separate questions, and the second is the one that bites: stone is generated on cliffs
    /// that nothing can stand on, so "the world holds some" and "you can reach some" are different
    /// claims and only the second one makes it a material rather than scenery.
    #[test]
    fn every_recipe_input_is_reachable_from_the_landing_site() {
        let report = balance::compute();
        assert!(report.access.len() >= 9, "eight fields and water");
        for material in &report.access {
            if material.material == "sand" || material.material == "crystal" {
                // Sand is the regional ocean and crystal is the reason to leave. Neither is
                // guaranteed, and a 96-hex sample of a 512-hex landform often never reaches them.
                continue;
            }
            assert!(
                material.reachable,
                "{} is required by {} rows and nothing can reach any of it",
                material.material, material.required_by
            );
            assert!(
                material.nearest_generated.is_some(),
                "{} is required by {} rows and the default world generates none",
                material.material,
                material.required_by
            );
            // A guaranteed material is guaranteed as a *patch*, not as a cell: the clearing holds
            // nothing now, so a promise that could be kept with one hex would be no promise.
            assert!(
                material.guaranteed_walk.is_none()
                    || material.guaranteed_hexes >= WORKABLE_PATCH_HEXES,
                "{} is guaranteed as {} hexes, which no extractor can fill from",
                material.material,
                material.guaranteed_hexes
            );
        }
        // Water is nobody's field: a pump makes it out of terrain, so it is the one raw material
        // the opening does not guarantee and it still has to be within reach of the landing site.
        let water = report
            .access
            .iter()
            .find(|material| material.material == "water")
            .expect("water is a raw material");
        assert_eq!(water.guaranteed_walk, None);
        assert!(water.nearest_generated.unwrap_or(u32::MAX) <= LANDING_CLEAR_RADIUS as u32);
    }

    /// The founding contract has to be a founding *project*, and that is a claim about its bill.
    ///
    /// Three components prove one chain out of one landscape, which was the whole of v0.13's
    /// objective and is deliberately only the first stage now. What the milestone asserts is that
    /// the project the hub actually builds cannot be paid for out of that chain: it needs more than
    /// one raw material, it costs strictly more, and — like every powered machine in this game — it
    /// cannot be run at all without an On-site Power branch nothing else forces.
    #[test]
    fn the_founding_project_needs_more_than_the_chain_it_starts_from() {
        let report = balance::compute();
        let founding: Vec<_> = report
            .contracts
            .iter()
            .filter(|stage| stage.scenario == "new-game")
            .collect();
        assert!(
            founding.len() >= 2,
            "a single-stage contract is the old objective wearing a new name"
        );
        let first = founding.first().expect("a first stage");
        let last = founding.last().expect("a last stage");
        assert!(
            last.raw_materials >= 2,
            "the founding project bottoms out in {} raw material(s); a project that needs one \
             landscape is a longer version of the opening",
            last.raw_materials
        );
        assert!(
            last.opening.gather_total > first.opening.gather_total,
            "the project must cost more than the beat that proves the line"
        );
        for stage in &founding {
            assert!(
                stage
                    .opening
                    .technologies
                    .iter()
                    .any(|key| key == "on-site-power"),
                "{} is payable without power, so the guidance may lead somewhere the rules refuse",
                stage.stage
            );
        }
    }

    /// An opening that needs a machine the rules will not run is not an opening.
    ///
    /// `power_progress` returns zero off a network, so a plan naming a smelter and no generator is
    /// a plan for a factory that stands still. This is the same defect the scripted next action
    /// had, asserted here against the numbers rather than against the sentence.
    #[test]
    fn every_opening_that_draws_power_also_pays_for_it() {
        let report = balance::compute();
        let definitions: DefinitionsInput =
            serde_json::from_str(include_str!("../../src/data/definitions.json")).unwrap();
        let building = |key: &str| {
            definitions
                .buildings
                .iter()
                .find(|building| building.key == key)
                .expect("opening names a shipped building")
        };
        let openings = report
            .openings
            .iter()
            .chain(report.contracts.iter().map(|stage| &stage.opening));
        for opening in openings {
            let draws = opening
                .buildings
                .iter()
                .any(|key| building(key).power_draw.unwrap_or(0) > 0);
            if !draws {
                continue;
            }
            assert!(
                opening
                    .buildings
                    .iter()
                    .any(|key| building(key).power_output.unwrap_or(0) > 0),
                "{} draws power and generates none",
                opening.name
            );
        }
    }

    /// A generator whose upkeep eats its own output is not a generator.
    ///
    /// A boiler drinks one water every tick it runs and a turbine is dead without one beside it,
    /// so the pumps are part of the plant whether or not the definition file says so. Through
    /// v0.16 the pump made one water every six ticks, which is six pumps drawing 24 of the
    /// turbine's 48 before a single machine ran — leaving the mid-game workhorse behind a hydro
    /// generator that cost exactly the same and needed neither fuel nor plumbing.
    #[test]
    fn every_generator_is_worth_more_than_its_own_upkeep() {
        let report = balance::compute();
        for plant in &report.power {
            assert!(
                plant.net_output > 0,
                "{} produces {} and spends {} keeping itself fed",
                plant.building,
                plant.output,
                plant.upkeep_draw
            );
        }
        // The one that burns fuel and drinks water is the one that carries the most, or the cost
        // of running it buys nothing.
        let best_free = report
            .power
            .iter()
            .filter(|plant| plant.fuel_energy_per_tick == 0)
            .map(|plant| plant.net_output)
            .max()
            .unwrap_or(0);
        let steam = report
            .power
            .iter()
            .find(|plant| plant.source == "turbine")
            .expect("steam is in the catalogue");
        assert!(
            steam.net_output > best_free,
            "steam nets {} against {} for a generator that needs nothing",
            steam.net_output,
            best_free
        );
    }

    #[test]
    fn snapshot_delta_omits_unchanged_world_groups_and_pins_revisions() {
        let mut core = game("new-game");
        let previous = core.snapshot();
        core.tick_many(1);
        let current = core.snapshot();
        let delta = SnapshotDelta::between(7, 8, &previous, &current);
        assert_eq!(delta.base_revision, 7);
        assert_eq!(delta.revision, 8);
        assert_eq!(delta.tick, 1);
        assert!(delta.terrain.is_none());
        assert!(delta.resources.is_none());
        assert!(delta.buildings.is_none());
        assert!(delta.events.is_some());
        let json = serde_json::to_string(&delta).unwrap();
        assert!(!json.contains("\"terrain\""));
        assert!(!json.contains("\"resources\""));
        assert!(!json.contains("\"buildings\""));
    }

    #[test]
    fn generated_chunk_bounds_report_the_surveyed_world_area() {
        let mut core = game("new-game");
        let snapshot = core.snapshot();
        let size = core.scenario.chunk_size;
        assert!(!snapshot.chunks.is_empty());
        for chunk in &snapshot.chunks {
            let (x, y, span) = chunk_world_bounds(chunk.chunk_q, chunk.chunk_r, size);
            assert_eq!(chunk.x, x);
            assert_eq!(chunk.y, y);
            assert_eq!(chunk.span, span);
        }
        let contains = |chunk: &ChunkSnapshot, x: i32, y: i32| {
            (chunk.x..chunk.x + chunk.span).contains(&x)
                && (chunk.y..chunk.y + chunk.span).contains(&y)
        };
        // The player always stands inside surveyed world.
        assert!(snapshot
            .chunks
            .iter()
            .any(|chunk| contains(chunk, core.player.x, core.player.y)));
        // Distant world stays unreported, which is what the host renders as fog.
        let (far_q, far_r) = (size * 4, size * 4);
        let (far_x, far_y) = axial_world(far_q, far_r);
        assert!(!snapshot
            .chunks
            .iter()
            .any(|chunk| contains(chunk, far_x, far_y)));

        // Travelling there surveys it, so the fogged area shrinks as the player explores.
        core.ensure_neighborhood(far_x, far_y);
        let explored = core.snapshot();
        assert!(explored.chunks.len() > snapshot.chunks.len());
        assert!(explored
            .chunks
            .iter()
            .any(|chunk| contains(chunk, far_x, far_y)));
    }

    #[test]
    fn buildings_delta_sends_only_the_entities_that_changed() {
        let mut core = game("new-game");
        core.researched.insert(2);
        core.player.inventory.insert(1, 12);
        core.player.inventory.insert(6, 4);
        set_player_hex(&mut core, 3, 1);
        core.place(3, 0, 1, 0, None).unwrap();
        add_test_belt(&mut core, 4, 1, 0);
        core.compile_graph();

        // One tick advances only the extractor's progress; the hub and the belt are untouched.
        let previous = core.snapshot();
        core.tick_many(1);
        let current = core.snapshot();
        let patch = buildings_delta(&previous.buildings, &current.buildings).unwrap();
        assert!(!patch.replace);
        assert!(patch.removed.is_empty());
        assert_eq!(
            patch
                .changed
                .iter()
                .map(|entity| entity.id)
                .collect::<Vec<_>>(),
            vec![2]
        );
        assert!(current.buildings.len() > patch.changed.len());
        let json =
            serde_json::to_string(&SnapshotDelta::between(0, 1, &previous, &current)).unwrap();
        assert!(json.len() < serde_json::to_string(&current.buildings).unwrap().len());

        // Erasing reports the id instead of resending every surviving entity.
        let previous = current;
        core.erase(3, 0).unwrap();
        let current = core.snapshot();
        let patch = buildings_delta(&previous.buildings, &current.buildings).unwrap();
        assert_eq!(patch.removed, vec![2]);
        assert!(patch.changed.is_empty());

        // A full delta stays a complete replacement, so a host with no prior state is correct.
        let full = SnapshotDelta::full(0, 1, &current).buildings.unwrap();
        assert!(full.replace);
        assert_eq!(full.changed, current.buildings);
    }

    /// The shipped delta is built from marks made where state is mutated, not by diffing two
    /// complete snapshots, so a missed mark would silently strand the host on stale state. This
    /// pins the builder against the full diff it replaces, step by step, across every path that
    /// touches a snapshot group: quiet frames, ticks, gathering to depletion, hub delivery,
    /// research, placement, rotation, erasure, and travel into unsurveyed world.
    #[test]
    fn dirty_tracked_deltas_match_a_full_snapshot_diff() {
        let mut factory = test_factory("new-game");
        // Setup pokes happen before the baseline is taken, so the checked run only exercises real
        // native paths. Shrinking a guaranteed deposit lets the run reach depletion. The starting
        // pack stays inside the carrying rule, or the gathering steps below would be refused.
        factory.core.player.inventory.insert(1, 40);
        factory.core.player.inventory.insert(3, 20);
        factory.core.player.inventory.insert(6, 8);
        set_player_hex(&mut factory.core, 4, -2);
        factory.core.write_overlay(4, -2, 1, 2, 36);
        // The clearing generates nothing since v0.21, so the deposit the extractor further down
        // stands on is written here rather than found. Same reasoning as `TEST_FIELD`: this is a
        // test about which marks a delta carries, not about where a generator puts iron.
        factory.core.write_overlay(3, 0, 1, 48, 48);
        let surveyed_at_start = factory.core.generated_chunks.len();

        // Establish the baseline exactly as the worker does on its first frame.
        let _ = factory.snapshot_json();
        let mut previous = factory.core.snapshot();
        let mut check = |factory: &mut Factory, step: &str| {
            assert_delta_matches_full_diff(factory, &mut previous, step);
        };

        factory.core.advance("[]", 0, 0).unwrap();
        check(&mut factory, "an empty frame");
        factory.core.advance(IDLE, 1, 1).unwrap();
        check(&mut factory, "one idle tick");

        // Gathering, through the frame the deposit runs dry and one rejected attempt after it.
        // The cooldown between attempts is paid in player steps, because that is the clock the
        // player's own actions run on — the factory ticks here only exercise the tick paths.
        for round in 0..3 {
            factory
                .core
                .advance(r#"[{"type":"gather"}]"#, 2, 60)
                .unwrap();
            check(&mut factory, &format!("gather attempt {round}"));
        }
        assert_eq!(factory.core.deposit_quantity((4, -2)), 0);

        // Delivery and research: insight, delivered totals, the objective, and unlocks.
        set_player_hex(&mut factory.core, 1, 0);
        check(&mut factory, "walking to the landing hub");
        factory
            .core
            .advance(r#"[{"type":"deposit"}]"#, 1, 0)
            .unwrap();
        check(&mut factory, "delivering inventory to the hub");
        // Four technologies cost twenty insight and one board row pays ten, so the rest is funded
        // directly. Insight is compared against the baseline rather than marked, so a direct change
        // is exactly what the host would see from any native path that moves it.
        factory.core.insight += 20;
        check(&mut factory, "funding the research");
        for technology in [1, 2, 3, 4] {
            let command = format!(r#"[{{"type":"research","technology_id":{technology}}}]"#);
            factory.core.advance(&command, 1, 0).unwrap();
            check(
                &mut factory,
                &format!("researching technology {technology}"),
            );
        }
        assert_eq!(factory.core.researched.len(), 4);

        // Player state is compared against the baseline rather than marked, so restocking directly
        // is exactly what the host would see from any native path that changes inventory.
        // Kept inside the carrying rule, so the erase further down still has somewhere to refund to.
        factory.core.player.inventory.insert(1, 60);
        factory.core.player.inventory.insert(3, 10);
        factory.core.player.inventory.insert(6, 8);
        check(&mut factory, "restocking the player");

        // Construction: inserted entities, recompiled transport, and per-chunk entity counts.
        set_player_hex(&mut factory.core, 3, 1);
        check(&mut factory, "walking to the build site");
        factory.core.place(3, 0, 1, 3, None).unwrap();
        check(&mut factory, "placing an extractor");
        factory.core.place(2, 0, 2, 3, None).unwrap();
        check(&mut factory, "placing a belt");
        factory.core.place(1, 0, 3, 3, Some(1)).unwrap();
        check(&mut factory, "placing a composer");

        // The factory running: machine progress, cargo transfer, hub deliveries, and victory.
        for round in 0..8 {
            factory.core.advance(IDLE, 20, 0).unwrap();
            check(&mut factory, &format!("running the factory, round {round}"));
        }
        assert!(factory.core.delivered > 0, "the scripted run must produce");

        // Edits against a live blueprint, including orientations that split and rejoin components.
        for turn in 0..6 {
            factory.core.rotate(2, 0).unwrap();
            check(&mut factory, &format!("rotating a belt, turn {turn}"));
        }
        factory.core.erase(2, 0).unwrap();
        check(&mut factory, "erasing a belt");
        factory.core.advance(IDLE, 5, 0).unwrap();
        check(&mut factory, "ticking with the belt gone");
        factory.core.place(2, 0, 2, 3, None).unwrap();
        check(&mut factory, "replacing the belt");

        // Cutting flora and letting it grow back. Regrowth is the one thing that changes a deposit
        // without an extractor or a player touching it that frame, so it has to mark what it moved.
        set_player_hex(&mut factory.core, -3, 1);
        check(&mut factory, "walking to the flora");
        factory
            .core
            .advance(r#"[{"type":"gather"}]"#, 1, GATHER_COOLDOWN_STEPS)
            .unwrap();
        check(&mut factory, "cutting flora");
        let regrowth = factory
            .core
            .item_definition(WOOD)
            .unwrap()
            .regrowth_ticks
            .expect("wood regrows");
        factory.core.advance(IDLE, regrowth, 0).unwrap();
        check(&mut factory, "flora growing back");
        assert!(
            factory.core.flora_regrowth.is_empty(),
            "the cut cell must have grown back inside its own cadence"
        );

        // Travel into unsurveyed world: terrain, deposits, chunk bounds, and every extractor's
        // resolved deposit reference at once. The neighborhood generator is the same one walking
        // uses; a far hex is used so derived water or cliffs cannot stall the survey.
        for (label, (q, r)) in [("east", (24, 0)), ("south", (24, 16))] {
            set_player_hex(&mut factory.core, q, r);
            factory.core.advance(IDLE, 1, 1).unwrap();
            check(
                &mut factory,
                &format!("travelling {label} into unsurveyed world"),
            );
        }
        factory.core.advance(IDLE, 1, 1).unwrap();
        check(&mut factory, "standing still again");
        assert!(
            factory.core.generated_chunks.len() > surveyed_at_start,
            "the scripted run must survey new world"
        );

        // A load replaces the core the baseline described, so the host is sent a complete
        // replacement rather than a patch against state that no longer exists.
        let save = factory.core.save_string().unwrap();
        factory.load_string(&save).unwrap();
        let delta = factory.build_delta();
        assert!(
            delta
                .buildings
                .expect("full delta carries buildings")
                .replace
        );
        assert!(
            delta
                .resources
                .expect("full delta carries resources")
                .replace
        );
        assert!(delta.terrain.is_some());
        assert!(delta.chunks.is_some());
        assert!(delta.player.is_some());
    }

    /// World generation invalidates resolved deposit references, so it must invalidate the entity
    /// snapshots derived from them in the same breath. Today's deposit radii are smaller than the
    /// tile spacing, so a generated deposit does not in fact reach an existing extractor and the
    /// scripted equivalence run cannot observe this — which is exactly why the coupling is pinned
    /// here directly rather than left to depend on that geometry holding.
    #[test]
    fn world_generation_invalidates_resolved_deposits_and_the_snapshots_built_from_them() {
        let mut core = game("new-game");
        core.researched.insert(2);
        core.player.inventory.insert(1, 8);
        core.player.inventory.insert(6, 2);
        set_player_hex(&mut core, 3, 1);
        core.place(3, 0, 1, 0, None).unwrap();
        let index = core.entity_at(3, 0).unwrap();
        core.extractor_deposit(index);
        assert_eq!(core.deposit_links.len(), 1);

        core.dirty = SnapshotDirty::default();
        core.generate_chunk(-9, 7);

        assert!(core.deposit_links.is_empty(), "references are re-resolved");
        let marked: Vec<u32> = core.entities.iter().map(|entity| entity.id).collect();
        assert_eq!(
            drain_marks(&mut core.dirty.entities),
            marked,
            "every entity snapshot derived from a deposit is suspect too"
        );
        assert!(core.dirty.chunks, "the surveyed chunk set grew");
    }

    /// An extractor's reported status is resolved through its cached deposit reference instead of
    /// a scan over every generated tile. The two must agree exactly, including after the deposit
    /// under it runs dry.
    #[test]
    fn extractor_status_matches_a_full_deposit_scan() {
        let mut core = game("new-game");
        core.researched.extend([1, 2]);
        core.player.inventory.insert(1, 40);
        core.player.inventory.insert(6, 8);
        set_player_hex(&mut core, 3, 1);
        core.place(3, 0, 1, 0, None).unwrap();
        let index = core.entity_at(3, 0).unwrap();

        let scanned = |core: &Core| {
            let (x, y) = axial_world(core.entities[index].placed.q, core.entities[index].placed.r);
            core.resource_at_world(x, y)
                .map(|key| core.deposit_quantity(key))
                .unwrap_or(0)
                > 0
        };

        for _ in 0..3 {
            let expected = scanned(&core);
            assert_eq!(core.extractor_deposit(index).is_some(), expected);
            assert_eq!(
                core.status_of(index, expected, true, true, false),
                core.entity_snapshot(index).status
            );
            core.tick_many(20);
        }

        // Draining the field must flip both the scan and the cached reference together.
        core.write_overlay(3, 0, 1, 0, 48);
        assert!(!scanned(&core));
        assert!(core.extractor_deposit(index).is_none());
        core.entities[index].cargo = None;
        assert_eq!(
            core.entity_snapshot(index).status,
            EntityStatus::DepositDepleted
        );
    }

    #[test]
    fn combined_advance_preserves_command_events_through_native_ticks() {
        let mut core = game("new-game");
        core.player.inventory.insert(1, 8);
        core.player.inventory.insert(3, 4);
        set_player_hex(&mut core, 1, 0);
        core.advance(r#"[{"type":"deposit"}]"#, 1, 0).unwrap();
        assert_eq!(core.tick, 1);
        // Eight ore, because the opening board asks for ore and nobody has asked for crystal yet.
        assert!(core
            .events
            .iter()
            .any(|event| event.contains("Delivered 8 to the landing hub")));
        assert_eq!(core.player.inventory.get(&3), Some(&4));
    }

    #[test]
    fn malformed_technology_graphs_and_locked_forged_commands_are_rejected() {
        let (definitions, mut technologies, scenarios) = catalogs();
        technologies.technologies[0].prerequisites = vec![3];
        assert!(validate_technologies(&definitions, &technologies).is_err());
        let mut core = game("new-game");
        core.player.inventory.insert(1, 100);
        core.apply_commands(r#"[{"type":"place","q":2,"r":0,"definition_id":2,"orientation":0}]"#)
            .unwrap();
        assert!(core.entities.iter().all(|entity| entity.placed.q != 2));
        assert!(core.events[0].contains("locked"));
        assert!(validate_scenarios(&definitions, &catalogs().1, &scenarios).is_ok());
    }

    /// A riser routes two rows, and the hexes it spans stay free. This is the whole answer to
    /// north-south transport: a direction-table row, resolved by the ray-cast the graph compiler
    /// already was, with no sub-hex occupancy anywhere.
    #[test]
    fn a_riser_routes_two_rows_and_leaves_the_hexes_it_spans_free() {
        let mut core = game("new-game");
        core.researched.extend([1, 4, 11]);
        core.player.inventory.insert(1, 40);

        // A riser at (0, 3) facing north reaches (1, 1) — the same world column, two rows up.
        set_player_hex(&mut core, 1, 2);
        core.place(1, 1, 4, 0, None).unwrap();
        set_player_hex(&mut core, 1, 3);
        core.place(0, 3, 18, NORTH, None).unwrap();

        let riser = core.entity_at(0, 3).unwrap();
        let container = core.entity_at(1, 1).unwrap();
        assert_eq!(
            core.graph[riser],
            Some(container),
            "a north-facing riser must bind to what sits two rows above it"
        );
        // The seam it spans is two ordinary hexes, and neither is occupied by anything.
        assert_eq!(core.entity_at(0, 2), None);
        assert_eq!(core.entity_at(1, 2), None);
        // So they stay buildable, and the riser never claims them for collision either.
        assert!(core.placement_legality(0, 2, 2, 0, None, true).is_ok());
        assert!(!core.building_definition(18).unwrap().blocks_movement);
        // The riser occupies exactly one hex.
        assert_eq!(core.entity_footprint(&core.entities[riser]).len(), 1);

        // Rotation visits all six corners and returns to north.
        for expected in (NORTH + 1)..(NORTH + 6) {
            core.rotate(0, 3).unwrap();
            assert_eq!(
                core.entities[core.entity_at(0, 3).unwrap()]
                    .placed
                    .orientation,
                expected
            );
        }
        core.rotate(0, 3).unwrap();
        assert_eq!(
            core.entities[core.entity_at(0, 3).unwrap()]
                .placed
                .orientation,
            NORTH,
            "rotation stays on the definition's own axis"
        );
    }

    /// Orientation is an axis the definition owns, and that is what prices the riser. A belt may
    /// never take a corner heading, because a belt that could would reach twice as far for a
    /// belt's cost.
    #[test]
    fn orientation_axes_keep_the_riser_priced_and_the_belt_horizontal() {
        let mut core = game("new-game");
        core.researched.extend([1, 11]);
        core.player.inventory.insert(1, 40);
        set_player_hex(&mut core, 1, 3);

        assert!(core
            .placement_legality(0, 3, 2, NORTH, None, true)
            .unwrap_err()
            .contains("oriented in 0..6"));
        assert!(core
            .placement_legality(0, 3, 18, 0, None, true)
            .unwrap_err()
            .contains("oriented in 6..12"));
        assert!(core.placement_legality(0, 3, 18, NORTH, None, true).is_ok());

        // And the price is a data row, not a mechanism: the riser simply costs twice the belt.
        let belt = core
            .building_definition(2)
            .unwrap()
            .construction_cost
            .clone();
        let riser = core
            .building_definition(18)
            .unwrap()
            .construction_cost
            .clone();
        assert_eq!(riser.len(), belt.len());
        for (riser, belt) in riser.iter().zip(&belt) {
            assert_eq!(riser.item_id, belt.item_id);
            assert_eq!(riser.quantity, belt.quantity * 2);
        }

        // No definition needs a multi-cell corner footprint yet, so that untested combination is
        // still refused at load.
        let (mut definitions, _, _) = catalogs();
        let index = definitions
            .buildings
            .iter()
            .position(|building| building.id == 18)
            .unwrap();
        definitions.buildings[index]
            .footprint
            .push(Coordinate { q: 1, r: 0 });
        assert!(validate_definitions(&definitions)
            .unwrap_err()
            .contains("two-row period"));
    }

    /// An upgrade grows a building in place: contents, heading, and connections all survive, and
    /// the ladder conserves items exactly. The round trip is the assertion that matters — an
    /// upgrade that paid out more than it took in would be a duplication exploit, which is the
    /// same failure `erase`'s all-or-nothing refund exists to prevent.
    #[test]
    fn an_upgrade_preserves_contents_connections_and_conserves_items_exactly() {
        let mut core = game("new-game");
        core.researched.extend([1, 4, 12]);
        // Everything the ladder can possibly charge, so the test measures conservation and not
        // whether the player happened to be able to afford a step.
        for item_id in [1, 3, 6, 11, 19] {
            core.player.inventory.insert(item_id, 60);
        }
        core.player.carry_slots = 99;
        let before = core.player.inventory.clone();

        set_player_hex(&mut core, 1, 3);
        core.place(0, 3, 4, 2, None).unwrap();
        // Give it contents and a downstream connection to preserve.
        let index = core.entity_at(0, 3).unwrap();
        let id = core.entities[index].id;
        core.entities[index].inventory.insert(5, 9);
        core.place(0, 4, 2, 0, None).unwrap();
        let linked_before = core.graph[core.entity_at(0, 4).unwrap()];

        core.upgrade(0, 3).unwrap();

        let index = core.entity_at(0, 3).unwrap();
        assert_eq!(
            core.entities[index].id, id,
            "the entity is edited, not replaced"
        );
        assert_eq!(core.entities[index].placed.definition_id, 20);
        assert_eq!(
            core.entities[index].placed.orientation, 2,
            "heading survives"
        );
        assert_eq!(
            core.entities[index].inventory.get(&5),
            Some(&9),
            "stock survives"
        );
        assert_eq!(
            core.graph[core.entity_at(0, 4).unwrap()],
            linked_before,
            "the belt feeding it still points at it"
        );
        assert!(core.events.iter().any(|event| event.contains("Upgraded")));

        // The ladder ends: a tier with no `upgrades_to` says so rather than failing quietly.
        assert!(core
            .upgrade(0, 3)
            .unwrap_err()
            .contains("already at its highest tier"));

        // Round trip. Erasing the upgraded container hands back exactly the sum of both payments,
        // so the player's pack returns to where it started — plus only the stock the container was
        // holding, which erase has always returned and which no step of the ladder created.
        core.erase(0, 3).unwrap();
        core.erase(0, 4).unwrap();
        let mut expected = before.clone();
        *expected.entry(5).or_default() += 9;
        assert_eq!(
            core.player.inventory, expected,
            "place → upgrade → erase must be item-neutral"
        );

        // The same holds for the reach ladder, which charges a different item set.
        let mut ore = game("new-game");
        ore.researched.extend([1, 2, 12]);
        for item_id in [1, 3, 6, 11, 19] {
            ore.player.inventory.insert(item_id, 60);
        }
        ore.player.carry_slots = 99;
        let before = ore.player.inventory.clone();
        set_player_hex(&mut ore, 3, 1);
        ore.place(3, 0, 1, 0, None).unwrap();
        ore.upgrade(3, 0).unwrap();
        assert_eq!(
            ore.entities[ore.entity_at(3, 0).unwrap()]
                .placed
                .definition_id,
            19
        );
        ore.erase(3, 0).unwrap();
        assert_eq!(ore.player.inventory, before);
    }

    /// Reach is the flagship upgrade, so it has to be a number the definition owns — and the hand
    /// must not inherit it. The predicate stays single; only its argument moves.
    #[test]
    fn extraction_reach_comes_from_the_definition_and_the_hand_keeps_its_own() {
        let mut core = game("new-game");
        core.researched.extend([1, 2, 12]);
        for item_id in [1, 3, 6, 11, 19] {
            core.player.inventory.insert(item_id, 60);
        }
        set_player_hex(&mut core, 3, 1);
        core.place(3, 0, 1, 0, None).unwrap();

        let shallow = core.entity_at(3, 0).unwrap();
        core.extractor_deposit(shallow);
        let shallow_reach = core.deposit_links[&core.entities[shallow].id].clone();
        assert_eq!(shallow_reach, core.deposit_candidates(3, 0, 1));

        core.upgrade(3, 0).unwrap();
        assert_eq!(
            core.deposit_links.get(&core.entities[shallow].id),
            None,
            "a change of reach must drop the list resolved against the old one"
        );
        let deep = core.entity_at(3, 0).unwrap();
        core.extractor_deposit(deep);
        let deep_reach = core.deposit_links[&core.entities[deep].id].clone();
        assert_eq!(deep_reach, core.deposit_candidates(3, 0, 2));
        assert!(
            deep_reach.len() >= shallow_reach.len(),
            "a deeper extractor can only ever cover more"
        );
        assert_eq!(core.extract_radius_of(1), EXTRACT_RADIUS);
        assert_eq!(core.extract_radius_of(19), 2);
        assert_eq!(core.building_definition(1).unwrap().extract_radius, Some(1));
        assert_eq!(
            core.building_definition(11).unwrap().extract_radius,
            Some(1)
        );
        assert_eq!(core.player_snapshot().extract_radius, EXTRACT_RADIUS as u32);

        // The hand is unchanged. A gather still reaches exactly one hex, whatever is built on it.
        let (x, y) = axial_world(3, 0);
        let by_hand = core.resource_at_world(x, y);
        assert!(by_hand.map_or(true, |cell| axial_distance((3, 0), cell) <= EXTRACT_RADIUS));

        // And a definition may not claim an unbounded arm.
        let (mut definitions, _, _) = catalogs();
        let index = definitions
            .buildings
            .iter()
            .position(|building| building.id == 19)
            .unwrap();
        definitions.buildings[index].extract_radius = Some(MAX_EXTRACT_RADIUS + 1);
        assert!(validate_definitions(&definitions)
            .unwrap_err()
            .contains("reach in 1..="));
    }

    /// A right-click names the hex. That is a different thing from facing-weighted targeting, and
    /// the difference is the whole reason this is allowed: the player chose the cell, on screen,
    /// so the number that moves is the one they pointed at. Reach is unchanged.
    #[test]
    fn a_named_gather_takes_from_the_hex_the_player_picked_within_the_same_reach() {
        let mut core = game("new-game");
        set_player_hex(&mut core, 3, 0);
        // Field cells either side of the one underfoot, so a target that drifts is visible.
        core.write_overlay(4, 0, 1, 20, 20);
        core.write_overlay(2, 0, 1, 20, 20);

        // The untargeted gather still takes from the hex underfoot.
        core.gather().unwrap();
        assert_eq!(core.deposit_quantity((3, 0)), 47);
        cooldown(&mut core);

        // The named one takes from the neighbour that was named, and leaves the rest alone.
        core.gather_at(4, 0).unwrap();
        assert_eq!(
            (
                core.deposit_quantity((2, 0)),
                core.deposit_quantity((3, 0)),
                core.deposit_quantity((4, 0)),
            ),
            (20, 47, 19)
        );
        cooldown(&mut core);

        // Reach is the same predicate, so a hex an extractor here could not cover is refused.
        assert!(core.gather_at(6, 0).unwrap_err().contains("out of reach"));
        // So is ground that holds no field at all.
        assert!(core.gather_at(3, 1).unwrap_err().contains("out of reach"));
        // And the cooldown is the one cooldown, shared by both.
        core.gather_at(4, 0).unwrap();
        assert!(core.gather_at(2, 0).unwrap_err().contains("cooling down"));
        cooldown(&mut core);

        // A worked-out cell is refused rather than underflowed.
        core.write_overlay(2, 0, 1, 0, 20);
        assert!(core.gather_at(2, 0).unwrap_err().contains("worked out"));

        // Signal crystal is in the world, and the hand still cannot take it.
        cooldown(&mut core);
        core.write_overlay(4, 0, CRYSTAL, 8, 8);
        let refusal = core.gather_at(4, 0).unwrap_err();
        assert!(
            refusal.contains("cannot be gathered by hand"),
            "crystal refusal was {refusal}"
        );
        assert!(
            refusal.contains("extractor"),
            "name the machine, got {refusal}"
        );
        assert_eq!(core.deposit_quantity((4, 0)), 8);
        assert!(core.player.inventory.get(&CRYSTAL).is_none());

        // Every reachable field cell is nameable, and nothing outside the reach is.
        let origin = (3, 0);
        for &(dq, dr) in &DIRECTIONS {
            for steps in 1..=2 {
                let cell = (origin.0 + dq * steps, origin.1 + dr * steps);
                if core.field_at(cell.0, cell.1).is_none() {
                    continue;
                }
                cooldown(&mut core);
                let can_hand = core
                    .field_at(cell.0, cell.1)
                    .and_then(|res| core.item_definition(res.item_id))
                    .is_some_and(|i| i.hand_gather_steps.is_some());
                let named = core.gather_at(cell.0, cell.1).is_ok();
                assert_eq!(
                    named,
                    core.field_covered_at(origin, cell, EXTRACT_RADIUS)
                        && core.deposit_quantity(cell) > 0
                        && can_hand,
                    "named gather at {cell:?} disagreed with the shared reach predicate"
                );
            }
        }
    }

    /// Loading a container by hand is the exact mirror of unloading one, on the same contract:
    /// the quantity is a ceiling, a partial move succeeds, and nothing is ever destroyed.
    #[test]
    fn storing_moves_what_fits_and_leaves_the_rest_in_the_pack() {
        let mut core = game("new-game");
        core.researched.extend([1, 4]);
        core.player.inventory.insert(1, 30);
        set_player_hex(&mut core, 1, 3);
        core.place(0, 3, 4, 0, None).unwrap();
        let capacity = core.building_definition(4).unwrap().capacity.unwrap();

        // A ceiling, not a demand: asking for more than the container can hold moves what fits.
        core.store(0, 3, 1, 999).unwrap();
        let index = core.entity_at(0, 3).unwrap();
        assert_eq!(core.entities[index].inventory.get(&1), Some(&capacity));
        // Conservation: what left the pack is exactly what arrived, cost of the box included.
        assert_eq!(
            core.player.inventory.get(&1).copied().unwrap_or(0) + capacity + 3,
            30
        );
        assert!(core.events.iter().any(|event| event.contains("Stored")));

        // A full container refuses rather than silently dropping the overflow.
        assert!(core.store(0, 3, 1, 1).unwrap_err().contains("full"));
        // And the round trip is exact.
        let carried = core.player.inventory.get(&1).copied().unwrap_or(0);
        core.withdraw(0, 3, 1, capacity).unwrap();
        assert_eq!(core.player.inventory.get(&1), Some(&(carried + capacity)));
        assert_eq!(
            core.entities[index].inventory.get(&1).copied().unwrap_or(0),
            0
        );

        // Only containers, and only what the player is actually carrying.
        assert!(core
            .store(0, 3, 99, 1)
            .unwrap_err()
            .contains("not carrying"));
        assert!(core.store(2, 3, 1, 1).unwrap_err().contains("no building"));
        // Bounded and range-checked like every other edit.
        assert!(core.store(9, 9, 1, 1).unwrap_err().contains("build range"));
    }

    #[test]
    fn negative_coordinates_use_euclidean_chunk_division() {
        assert_eq!(floor_div(-1, 8), -1);
        assert_eq!(floor_div(-8, 8), -1);
        assert_eq!(floor_div(-9, 8), -2);
    }

    #[test]
    fn capacity_workload_is_deterministic_and_actually_produces() {
        let spec = capacity::quick_tiers()[1];
        let mut first = capacity::warm_core(&spec);
        let mut second = capacity::warm_core(&spec);
        first.advance_ticks(120);
        second.advance_ticks(120);
        assert_eq!(first.checksum(), second.checksum());
        // Pinned so a change to definitions, the workload, or the simulation cannot silently
        // invalidate comparisons against previously recorded tier numbers. A generator-version
        // bump moves this number while the workload does not — which is why the delivered total
        // and the entity count below are the assertions that say the run is the same run.
        assert_eq!(first.checksum(), 798_893_689);
        assert_eq!(first.entities.len(), spec.entities() as usize);
        // Every line must be running end to end, or the tiers would measure an idle blueprint.
        assert_eq!(first.delivered, u64::from(spec.lines) * 14);
    }

    /// A clock that advances a fixed amount per reading, so the ladder's arithmetic can be pinned
    /// without depending on how long a machine actually takes.
    struct StepClock {
        step_us: f64,
        readings: std::cell::Cell<u32>,
    }

    impl capacity::Clock for StepClock {
        fn now_us(&self) -> f64 {
            let reading = self.readings.get();
            self.readings.set(reading + 1);
            f64::from(reading) * self.step_us
        }
    }

    #[test]
    fn capacity_phases_are_reported_per_sample_against_the_supplied_clock() {
        let spec = capacity::quick_tiers()[0];
        let clock = StepClock {
            // Each phase reads the clock exactly twice, so one phase always spans one step.
            step_us: 1_000.0,
            readings: std::cell::Cell::new(0),
        };
        let tier = capacity::measure_tier_with(&spec, &clock, capacity::Budget::FIXED);
        // The tick phase spans one 1,000 µs step across `measured_ticks` samples.
        assert_eq!(tier.measured_ticks, spec.measured_ticks);
        assert_eq!(tier.tick_us, 1_000.0 / f64::from(spec.measured_ticks));
        assert_eq!(tier.frame_us, 1_000.0 / f64::from(spec.frames));
        assert_eq!(tier.snapshot_us, 1_000.0 / f64::from(spec.snapshots));
        assert_eq!(tier.ticks_per_second, 1e6 / tier.tick_us);
        // Every phase read the clock, and the workload itself is unchanged by the clock swap.
        assert_eq!(tier.entities, spec.entities() as usize);
        // Seven phases, each spanning exactly one pair of readings.
        assert_eq!(clock.readings.get(), 14);
    }

    /// A coarse clock must buy precision with more samples and nothing else: the tier's identity
    /// has to survive, or a browser record could not be compared against a native one.
    #[test]
    fn a_phase_budget_adds_samples_without_moving_the_workload() {
        let spec = capacity::quick_tiers()[1];
        let fixed = capacity::measure_tier_with(
            &spec,
            capacity::default_clock().as_ref(),
            capacity::Budget::FIXED,
        );
        // A step clock that only ever reports 500 µs per reading forces four repeats to reach a
        // 2,000 µs budget, without depending on how fast this machine is.
        let clock = StepClock {
            step_us: 500.0,
            readings: std::cell::Cell::new(0),
        };
        let budgeted = capacity::measure_tier_with(
            &spec,
            &clock,
            capacity::Budget {
                min_phase_us: 2_000.0,
            },
        );
        assert_eq!(budgeted.measured_ticks, spec.measured_ticks * 4);
        assert_eq!(
            budgeted.tick_us,
            2_000.0 / f64::from(budgeted.measured_ticks)
        );
        // The recorded identity of the tier is untouched by the extra samples.
        assert_eq!(budgeted.checksum, fixed.checksum);
        assert_eq!(budgeted.delivered, fixed.delivered);
        assert_eq!(budgeted.entities, fixed.entities);
        assert_eq!(budgeted.tiles, fixed.tiles);
    }

    #[test]
    fn capacity_ladder_measures_tiers_independently_and_reports_its_platform() {
        let specs = capacity::quick_tiers();
        let mut ladder = capacity::Ladder::new(specs.clone());
        let clock = capacity::default_clock();
        assert_eq!(ladder.len(), specs.len());
        assert!(ladder.measure(specs.len(), clock.as_ref()).is_none());
        // A partial run reports only what it measured, so an interrupted browser run still yields
        // an honest record rather than empty tiers.
        let first = ladder
            .measure(0, clock.as_ref())
            .expect("first tier measures");
        assert_eq!(ladder.report().tiers.len(), 1);
        // Re-measuring a tier replaces it instead of recording the same tier twice.
        let again = ladder
            .measure(0, clock.as_ref())
            .expect("first tier re-measures");
        assert_eq!(again.checksum, first.checksum);
        assert_eq!(ladder.report().tiers.len(), 1);

        ladder.measure(1, clock.as_ref()).expect("second tier");
        let report = ladder.report();
        assert_eq!(report.tiers.len(), 2);
        assert_eq!(report.platform, "native");
        assert_eq!(report.schema, capacity::REPORT_SCHEMA);
        assert!(capacity::format_table(&report).contains("native"));
    }

    /// The browser harness drives this factory over the ordinary worker RPC, so it must arrive in
    /// the same steady state the in-wasm phases measure, and its first delta must be a complete
    /// snapshot the host can adopt.
    #[test]
    fn capacity_round_trip_factory_starts_warm_and_sends_a_full_first_delta() {
        let spec = capacity::quick_tiers()[1];
        let mut factory = capacity::warm_factory(&spec);
        let warm = capacity::warm_core(&spec);
        assert_eq!(factory.checksum(), warm.checksum());
        assert!(warm.delivered > 0);

        let first: serde_json::Value =
            serde_json::from_str(&factory.snapshot_delta_json()).expect("delta parses");
        assert_eq!(first["base_revision"], 0);
        assert_eq!(first["revision"], 1);
        assert_eq!(first["buildings"]["replace"], true);
        assert_eq!(
            first["buildings"]["changed"]
                .as_array()
                .expect("a first delta carries the complete blueprint")
                .len(),
            spec.entities() as usize
        );

        factory
            .advance_json("[{\"type\":\"move_intent\",\"x\":0,\"y\":0}]", 1, 0)
            .expect("idle batch is accepted");
        let next: serde_json::Value =
            serde_json::from_str(&factory.snapshot_delta_json()).expect("delta parses");
        assert_eq!(next["base_revision"], 1);
        assert_eq!(next["revision"], 2);
        // The steady-state delta is a patch, not another complete blueprint: `replace` is skipped
        // when false, and only the entities that moved travel.
        assert!(next["buildings"]["replace"].is_null());
        let changed = next["buildings"]["changed"]
            .as_array()
            .expect("a steady-state frame changes entities");
        assert!(!changed.is_empty() && changed.len() < spec.entities() as usize);
    }

    #[test]
    fn capacity_ladder_reports_a_result_for_every_tier() {
        let specs = capacity::quick_tiers();
        let report = capacity::run(&specs);
        assert_eq!(report.schema, capacity::REPORT_SCHEMA);
        assert_eq!(report.tiers.len(), specs.len());
        for (tier, spec) in report.tiers.iter().zip(&specs) {
            assert_eq!(tier.entities, spec.entities() as usize);
            assert!(tier.tick_us > 0.0);
            assert!(tier.frame_us > 0.0);
            assert!(tier.full_compile_us > 0.0);
            assert!(tier.incremental_recompile_us > 0.0);
            assert!(tier.edit_us > 0.0);
            // A steady-state frame always carries at least the tick's changed groups.
            assert!(tier.delta_bytes > 0.0);
        }
        let table = capacity::format_table(&report);
        assert!(specs.iter().all(|spec| table.contains(spec.key)));
        assert!(capacity::format_json(&report).contains("\"schema\""));
    }
}
