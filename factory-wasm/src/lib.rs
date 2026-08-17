use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use wasm_bindgen::prelude::*;

type ItemId = u16;
type RecipeId = u16;
type DefinitionId = u16;
type TechnologyId = u16;

const SAVE_PREFIX: &str = "HXF1\n";
const SAVE_VERSION: u16 = 3;
const WORLD_GENERATOR_VERSION: u16 = 2;
const MAX_COMMANDS_PER_BATCH: usize = 8;
/// A drag is one bounded command, so the run it expands into has to be bounded too. This is the
/// native cap on cells a single `place_line` or `erase_line` may touch.
const MAX_LINE_CELLS: usize = 32;
/// How many constructions back one session can be taken. Derived state, so it costs nothing saved.
const MAX_UNDO_DEPTH: usize = 64;
const GRAPH_TRACE_LIMIT: i32 = 8;
const DIRECTIONS: [(i32, i32); 6] = [(1, 0), (0, 1), (-1, 1), (-1, 0), (0, -1), (1, -1)];
const HEX_X: i32 = 1774;
const HEX_Y: i32 = 1536;
const FEATURE_SPACING: i32 = 2048;
/// World units the player covers per player step. Paced by `PLAYER_TICKS_PER_SECOND`, not by the
/// simulation tick, so the walk keeps one speed at every simulation speed.
const PLAYER_SPEED: i32 = 150;
/// The player's own cadence, in steps per real second. Walking used to run inside the simulation
/// tick, which made it stop when the factory paused and crawl at a low speed multiplier. It is
/// still integer, still native, and still deterministic — a given step count always produces the
/// same position — it is simply no longer measured in factory time.
const PLAYER_TICKS_PER_SECOND: u32 = 30;
const PLAYER_RADIUS: i32 = 360;
const BUILDING_RADIUS: i32 = 690;
const GATHER_RANGE: i32 = 1450;
const HUB_RANGE: i32 = 1900;
/// Placement asks one question of a deposit and of an obstacle alike: does the hex a building would
/// occupy overlap the feature's circle, and by enough to matter? `placement_overlap` is that single
/// rule, and these two depths are the only difference between the two answers.
///
/// A hex step is 1774 world units, so the hex lattice's covering radius — the furthest any world
/// point can sit from the nearest hex centre — is 1774/√3 ≈ 1024. A deposit is therefore reachable
/// from some hex only while `deposit radius + BUILDING_RADIUS - depth` stays above that, and zero
/// keeps the smallest generated deposit (radius 520, so a reach of 1210) minable from somewhere.
/// The obstacle depth is what stops a rock that merely grazes a hex from making it unbuildable.
const DEPOSIT_COVERAGE_DEPTH: i32 = 0;
const OBSTACLE_INTRUSION_DEPTH: i32 = 400;

fn default_footprint() -> Vec<Coordinate> {
    vec![Coordinate { q: 0, r: 0 }]
}

#[derive(Clone, Deserialize)]
struct DefinitionsInput {
    version: u16,
    items: Vec<ItemDefinition>,
    recipes: Vec<RecipeDefinition>,
    buildings: Vec<BuildingDefinition>,
}

#[derive(Clone, Deserialize)]
struct ItemDefinition {
    id: ItemId,
    key: String,
    name: String,
    color: String,
    icon: String,
    description: String,
    insight_value: u32,
    /// How many of this item occupy one carried slot. Carrying capacity is a rule over the
    /// player's ordinary `item_id → quantity` map rather than a stored array of slots, so the save
    /// format, the checksum inputs, and every ordering guarantee are unchanged by it.
    stack_size: u32,
}

#[derive(Clone, Deserialize)]
struct RecipeDefinition {
    id: RecipeId,
    key: String,
    name: String,
    description: String,
    inputs: Vec<Ingredient>,
    output: Ingredient,
    duration: u32,
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
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum PlacementRule {
    Ground,
    Resource,
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
    chunk_size: i32,
    generated_environment: bool,
    player_spawn: Coordinate,
    player_facing: u8,
    build_range: u32,
    /// How many stacks the player can carry at once. Containers exist to solve this.
    carry_slots: u32,
    objective_item_id: ItemId,
    objective_quantity: u32,
    #[serde(default)]
    initial_inventory: Vec<Ingredient>,
    #[serde(default)]
    initial_researched: Vec<TechnologyId>,
    #[serde(default)]
    resources: Vec<ScenarioResource>,
    buildings: Vec<PlacedBuilding>,
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Terrain {
    Ground,
    Water,
    Rock,
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
    objective: ObjectiveSnapshot,
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
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct Ingredient64 {
    item_id: ItemId,
    quantity: u64,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct ObjectiveSnapshot {
    item_id: ItemId,
    delivered: u64,
    required: u32,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct ChunkSnapshot {
    chunk_q: i32,
    chunk_r: i32,
    entity_count: usize,
    /// World-space origin and side length of the generated square this chunk owns. A chunk is the
    /// unit of world generation, so these bounds are exactly the surveyed area: everything outside
    /// the reported chunks is world the simulation has not generated yet.
    x: i32,
    y: i32,
    span: i32,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct TileSnapshot {
    x: i32,
    y: i32,
    radius: u32,
    terrain: Terrain,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct ResourceSnapshot {
    id: u64,
    x: i32,
    y: i32,
    radius: u32,
    item_id: ItemId,
    quantity: u32,
    initial_quantity: u32,
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
    status: String,
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

/// A per-deposit resources patch, keyed by the stable deposit id. Resource tiles are inserted by
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

#[derive(Debug, Serialize)]
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
    objective: Option<ObjectiveSnapshot>,
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
            objective: Some(current.objective),
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
            objective: changed_copy(previous.objective, current.objective),
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
    let before: BTreeSet<u64> = previous.iter().map(|resource| resource.id).collect();
    let after: BTreeSet<u64> = current.iter().map(|resource| resource.id).collect();
    if before != after {
        return Some(ResourcesDelta {
            replace: true,
            changed: current.to_vec(),
        });
    }
    let existing: BTreeMap<u64, &ResourceSnapshot> = previous
        .iter()
        .map(|resource| (resource.id, resource))
        .collect();
    let changed: Vec<ResourceSnapshot> = current
        .iter()
        .filter(|resource| existing.get(&resource.id) != Some(resource))
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
    Gather,
    Deposit,
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
    /// Take stock out of a container by hand. Bounded and range-checked like every other edit.
    Withdraw {
        q: i32,
        r: i32,
        item_id: ItemId,
        quantity: u32,
    },
    Undo,
    Research {
        technology_id: TechnologyId,
    },
}

struct Core {
    definitions: DefinitionsInput,
    technologies: TechnologiesInput,
    scenario: ScenarioDefinition,
    seed: u32,
    generated_chunks: BTreeSet<(i32, i32)>,
    tiles: BTreeMap<(i32, i32), TileState>,
    /// Deposit references resolved per extractor entity id, so a running extractor never scans the
    /// tile map. Derived cache only: it is rebuilt from tiles on demand and never saved or hashed.
    deposit_links: BTreeMap<u32, Vec<(i32, i32)>>,
    entities: Vec<Entity>,
    graph: Vec<Option<usize>>,
    player: PlayerState,
    researched: BTreeSet<TechnologyId>,
    next_entity_id: u32,
    tick: u64,
    delivered: u64,
    delivered_by_item: BTreeMap<ItemId, u64>,
    insight: u64,
    victory: bool,
    produced: BTreeMap<ItemId, u64>,
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
    ) -> Result<Self, String> {
        let seed = seed_override.unwrap_or(scenario.seed);
        let mut inventory = BTreeMap::new();
        add_ingredients(&mut inventory, &scenario.initial_inventory);
        let mut core = Self {
            definitions: definitions.clone(),
            technologies: technologies.clone(),
            scenario: scenario.clone(),
            seed,
            generated_chunks: BTreeSet::new(),
            tiles: BTreeMap::new(),
            deposit_links: BTreeMap::new(),
            entities: Vec::new(),
            graph: Vec::new(),
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
            produced: BTreeMap::new(),
            events: vec![format!("{} ready", scenario.name)],
            dirty: SnapshotDirty::default(),
            undo_stack: Vec::new(),
        };
        core.ensure_neighborhood(core.player.x, core.player.y);
        for resource in &scenario.resources {
            core.ensure_tile(resource.q, resource.r);
            let tile = core.tiles.get_mut(&(resource.q, resource.r)).unwrap();
            let (x, y) = axial_world(resource.q, resource.r);
            tile.x = x;
            tile.y = y;
            tile.radius = 720;
            tile.terrain = Terrain::Ground;
            tile.resource = Some(ResourceState {
                item_id: resource.item_id,
                quantity: resource.quantity,
                initial_quantity: resource.quantity,
            });
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
            });
            core.next_entity_id += 1;
        }
        core.compile_graph();
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
        PlayerSnapshot {
            state: self.player.clone(),
            carry_stacks: self.carry_stacks(),
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
                        let offset = rotate_coordinate(*offset, orientation);
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
        self.entities.iter().position(|entity| {
            self.entity_footprint(entity)
                .iter()
                .any(|cell| cell.q == q && cell.r == r)
        })
    }

    /// Whether a building occupying the hex at this world point covers `feature`'s deposit well
    /// enough to draw from it — the same overlap rule an obstacle is tested with, at its own depth.
    fn deposit_covered_at(&self, x: i32, y: i32, feature: &TileState) -> bool {
        placement_overlap(
            x,
            y,
            BUILDING_RADIUS,
            feature.x,
            feature.y,
            feature.radius as i32,
            DEPOSIT_COVERAGE_DEPTH,
        )
    }

    fn resource_at_world(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        self.tiles
            .iter()
            .filter(|(_, feature)| {
                feature
                    .resource
                    .as_ref()
                    .map(|resource| resource.quantity > 0)
                    .unwrap_or(false)
                    && self.deposit_covered_at(x, y, feature)
            })
            .min_by_key(|(_, feature)| squared_distance(x, y, feature.x, feature.y))
            .map(|(key, _)| *key)
    }

    /// Every deposit covering a world point, ordered nearest first and then by tile key — the exact
    /// order `resource_at_world` resolves. Remaining quantity is deliberately not part of the
    /// ordering, so one resolved list stays correct for the whole life of the deposits under it.
    fn deposit_candidates(&self, x: i32, y: i32) -> Vec<(i32, i32)> {
        let mut candidates: Vec<(i64, (i32, i32))> = self
            .tiles
            .iter()
            .filter(|(_, feature)| feature.resource.is_some())
            .filter_map(|(key, feature)| {
                self.deposit_covered_at(x, y, feature)
                    .then(|| (squared_distance(x, y, feature.x, feature.y), *key))
            })
            .collect();
        candidates.sort_unstable();
        candidates.into_iter().map(|(_, key)| key).collect()
    }

    fn deposit_quantity(&self, key: (i32, i32)) -> u32 {
        self.tiles
            .get(&key)
            .and_then(|tile| tile.resource.as_ref())
            .map(|resource| resource.quantity)
            .unwrap_or(0)
    }

    /// The deposit an extractor draws from this tick, resolved from its cached candidate list
    /// instead of a scan over every generated tile. `generate_chunk` drops the cache whenever new
    /// tiles appear, so a reference can never outlive the tile set it was resolved against.
    fn extractor_deposit(&mut self, index: usize) -> Option<(i32, i32)> {
        let id = self.entities[index].id;
        if !self.deposit_links.contains_key(&id) {
            let placed = self.entities[index].placed;
            let (x, y) = axial_world(placed.q, placed.r);
            let candidates = self.deposit_candidates(x, y);
            self.deposit_links.insert(id, candidates);
        }
        self.deposit_links[&id]
            .iter()
            .copied()
            .find(|&key| self.deposit_quantity(key) > 0)
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
                let tile = self.generated_tile(q, r);
                // A chunk of plain ground adds nothing to either group, so both marks are narrowed
                // to a tile that actually appears in one. Generation is the only path that adds to
                // either: resending resources whole keeps the host's order exactly the native one,
                // so later patches can address deposits in place.
                self.dirty.terrain |= tile.terrain != Terrain::Ground;
                self.dirty.resources_replace |= tile.resource.is_some();
                self.tiles.insert((q, r), tile);
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

    fn generated_tile(&self, q: i32, r: i32) -> TileState {
        let hash = coordinate_hash(self.seed, q, r);
        let jitter_x = (hash & 0x3ff) as i32 - 512;
        let jitter_y = ((hash >> 10) & 0x3ff) as i32 - 512;
        let (mut x, mut y) = (
            q * FEATURE_SPACING + jitter_x,
            r * FEATURE_SPACING + jitter_y,
        );
        let mut radius = 520 + ((hash >> 20) % 360);
        if !self.scenario.generated_environment {
            return TileState {
                q,
                r,
                x,
                y,
                radius,
                terrain: Terrain::Ground,
                resource: None,
            };
        }
        let guaranteed = match (q, r) {
            (3, 0) => Some((Terrain::Ground, Some((1, 48)))),
            (4, -2) => Some((Terrain::Ground, Some((1, 36)))),
            (-2, 2) => Some((Terrain::Ground, Some((3, 32)))),
            (2, 1) | (2, 2) | (1, 2) => Some((Terrain::Water, None)),
            (1, -1) | (2, -1) => Some((Terrain::Rock, None)),
            _ => None,
        };
        if let Some((terrain, resource)) = guaranteed {
            (x, y) = axial_world(q, r);
            radius = if resource.is_some() { 720 } else { 660 };
            return TileState {
                q,
                r,
                x,
                y,
                radius,
                terrain,
                resource: resource.map(|(item_id, quantity)| ResourceState {
                    item_id,
                    quantity,
                    initial_quantity: quantity,
                }),
            };
        }
        let near_landing = squared_distance(x, y, 0, 0) <= i64::from(HEX_X * 7).pow(2);
        let terrain = if near_landing {
            Terrain::Ground
        } else if hash % 31 == 0 {
            Terrain::Water
        } else if hash % 23 == 0 {
            Terrain::Rock
        } else {
            Terrain::Ground
        };
        let resource = if terrain != Terrain::Ground || near_landing {
            None
        } else if hash % 67 == 1 {
            Some(ResourceState {
                item_id: 1,
                quantity: 24 + (hash % 25),
                initial_quantity: 24 + (hash % 25),
            })
        } else if hash % 149 == 2 {
            Some(ResourceState {
                item_id: 3,
                quantity: 16 + (hash % 17),
                initial_quantity: 16 + (hash % 17),
            })
        } else {
            None
        };
        TileState {
            q,
            r,
            x,
            y,
            radius,
            terrain,
            resource,
        }
    }

    fn ensure_tile(&mut self, q: i32, r: i32) {
        let size = self.scenario.chunk_size;
        self.generate_chunk(floor_div(q, size), floor_div(r, size));
    }

    fn ensure_neighborhood(&mut self, x: i32, y: i32) {
        let size = self.scenario.chunk_size;
        let cell_x = floor_div(x, FEATURE_SPACING);
        let cell_y = floor_div(y, FEATURE_SPACING);
        let center = (floor_div(cell_x, size), floor_div(cell_y, size));
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
        let (dq, dr) = DIRECTIONS[usize::from(entity.placed.orientation % 6)];
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
            self.player.action_cooldown = self.player.action_cooldown.saturating_sub(1);
            self.advance_machines();
            self.transfer_cargo();
            self.tick += 1;
        }
    }

    /// Walk the player on its own cadence. Movement deliberately no longer rides the simulation
    /// tick: a paused factory should not pin the player in place, and a 0.25× factory should not
    /// make walking feel broken. Frame-coupled movement stays refused — the host sends a step
    /// count, not a delta — so the same command sequence still reproduces the same position and the
    /// same checksum.
    fn advance_player_steps(&mut self, count: u32) {
        for _ in 0..count {
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
        let cadence = self
            .building_definition(definition_id)
            .and_then(|definition| definition.cadence)
            .unwrap_or(1);
        self.entities[index].progress += 1;
        if self.entities[index].progress < cadence {
            return;
        }
        let resource_key = resource_key.expect("available resource key exists");
        let resource = self
            .tiles
            .get_mut(&resource_key)
            .and_then(|tile| tile.resource.as_mut())
            .expect("available resource exists");
        resource.quantity -= 1;
        let item_id = resource.item_id;
        let depleted = resource.quantity == 0;
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
            self.events.push(format!("Deposit at {q},{r} depleted"));
        }
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
            self.entities[index].progress += 1;
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
        if can_start {
            let id = self.entities[index].id;
            self.dirty.entities.push(id);
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
                let accepts = recipe
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
            BuildingKind::Consumer | BuildingKind::Hub => true,
            BuildingKind::Extractor => false,
        }
    }

    fn accept(&mut self, target: usize, cargo: Cargo) {
        match self.entities[target].kind {
            BuildingKind::Belt => self.entities[target].cargo = Some(cargo),
            BuildingKind::Composer | BuildingKind::Container => {
                *self.entities[target]
                    .inventory
                    .entry(cargo.item_id)
                    .or_default() += cargo.quantity;
            }
            BuildingKind::Consumer => {
                self.delivered += u64::from(cargo.quantity);
                *self.delivered_by_item.entry(cargo.item_id).or_default() +=
                    u64::from(cargo.quantity);
                self.check_victory();
            }
            BuildingKind::Hub => self.deliver_to_hub(cargo.item_id, cargo.quantity),
            BuildingKind::Extractor => unreachable!("extractors reject cargo"),
        }
    }

    fn deliver_to_hub(&mut self, item_id: ItemId, quantity: u32) {
        self.delivered += u64::from(quantity);
        *self.delivered_by_item.entry(item_id).or_default() += u64::from(quantity);
        let value = self
            .item_definition(item_id)
            .map(|item| item.insight_value)
            .unwrap_or(0);
        self.insight += u64::from(value) * u64::from(quantity);
        self.check_victory();
    }

    fn check_victory(&mut self) {
        let delivered = self
            .delivered_by_item
            .get(&self.scenario.objective_item_id)
            .copied()
            .unwrap_or(0);
        if !self.victory && delivered >= u64::from(self.scenario.objective_quantity) {
            self.victory = true;
            self.events
                .push("Landing objective complete — free play continues".into());
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

    fn advance_player(&mut self) {
        let dx = i32::from(self.player.move_x) * PLAYER_SPEED / 1000;
        let dy = i32::from(self.player.move_y) * PLAYER_SPEED / 1000;
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

    fn player_blocked(&self, x: i32, y: i32) -> bool {
        let feature_collision = self.tiles.values().any(|feature| {
            feature.terrain != Terrain::Ground
                && circles_overlap(
                    x,
                    y,
                    PLAYER_RADIUS,
                    feature.x,
                    feature.y,
                    feature.radius as i32,
                )
        });
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
        let target_x = self.player.x + i32::from(self.player.facing_x) * (GATHER_RANGE / 2) / 1000;
        let target_y = self.player.y + i32::from(self.player.facing_y) * (GATHER_RANGE / 2) / 1000;
        let key = self
            .tiles
            .iter()
            .filter(|(_, feature)| {
                feature
                    .resource
                    .as_ref()
                    .map(|resource| resource.quantity > 0)
                    .unwrap_or(false)
                    && squared_distance(target_x, target_y, feature.x, feature.y)
                        <= i64::from(GATHER_RANGE + feature.radius as i32).pow(2)
            })
            .min_by_key(|(_, feature)| squared_distance(target_x, target_y, feature.x, feature.y))
            .map(|(key, _)| *key)
            .ok_or("no finite resource within gathering reach")?;
        let gathered = self.tiles[&key]
            .resource
            .as_ref()
            .expect("selected resource exists")
            .item_id;
        if self.player_room_for(gathered) == 0 {
            return Err("carrying capacity is full".into());
        }
        let feature = self.tiles.get_mut(&key).expect("selected resource exists");
        let resource = feature.resource.as_mut().expect("selected resource exists");
        resource.quantity -= 1;
        let (item_id, depleted) = (resource.item_id, resource.quantity == 0);
        self.dirty.resources.push(key);
        *self.player.inventory.entry(item_id).or_default() += 1;
        self.player.action_cooldown = 2;
        self.events.push(format!("Gathered item {item_id}"));
        if depleted {
            // Any extractor covering this deposit may now report a different status.
            self.mark_all_entities_dirty();
            self.events.push("Deposit depleted".into());
        }
        Ok(())
    }

    fn deposit_inventory(&mut self) -> Result<(), String> {
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
        let cargo: Vec<(ItemId, u32)> = self
            .player
            .inventory
            .iter()
            .map(|(&item, &quantity)| (item, quantity))
            .collect();
        self.player.inventory.clear();
        for (item, quantity) in cargo {
            self.deliver_to_hub(item, quantity);
        }
        self.events
            .push("Delivered inventory to landing hub".into());
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
        if orientation >= 6 {
            return Err("orientation must be in 0..6".into());
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
            if self.entity_at(cell.q, cell.r).is_some() {
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
            if self.tiles.values().any(|feature| {
                feature.terrain != Terrain::Ground
                    && placement_overlap(
                        cell_x,
                        cell_y,
                        BUILDING_RADIUS,
                        feature.x,
                        feature.y,
                        feature.radius as i32,
                        OBSTACLE_INTRUSION_DEPTH,
                    )
            }) {
                return Err("environment blocks construction".into());
            }
        }
        if definition.placement_rule == PlacementRule::Resource
            && self.resource_at_world(anchor_x, anchor_y).is_none()
        {
            return Err("extractors require a non-empty deposit".into());
        }
        if definition.kind == BuildingKind::Composer {
            let id = recipe_id.ok_or("composer requires a recipe")?;
            if self.recipe(id).is_none() {
                return Err(format!("unknown recipe {id}"));
            }
        }
        if check_cost && !has_ingredients(&self.player.inventory, &definition.construction_cost) {
            return Err("construction cost is not available".into());
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
        let cells = hex_line(from, to);
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
    fn line_preview(
        &self,
        from: (i32, i32),
        to: (i32, i32),
        definition_id: DefinitionId,
        orientation: u8,
    ) -> Vec<LinePreviewCell> {
        let Some(definition) = self.building_definition(definition_id) else {
            return Vec::new();
        };
        let routed = definition.kind == BuildingKind::Belt;
        let cost = definition.construction_cost.clone();
        let cells = hex_line(from, to);
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
                        .placement_legality(q, r, definition_id, cell_orientation, None, false)
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
        hex_line(from, to)
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

    /// One drag of removal, resolved exactly as `place_line` resolves construction.
    fn erase_line(&mut self, from: (i32, i32), to: (i32, i32)) -> Result<(), String> {
        let cells = hex_line(from, to);
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
        let next_orientation = (self.entities[index].placed.orientation + 1) % 6;
        let next_footprint = self.footprint_for(self.entities[index].placed, next_orientation);
        if next_footprint.iter().any(|cell| {
            self.entities.iter().enumerate().any(|(other, entity)| {
                other != index
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
                InputCommand::Gather => self.gather(),
                InputCommand::Deposit => self.deposit_inventory(),
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
                InputCommand::Withdraw {
                    q,
                    r,
                    item_id,
                    quantity,
                } => self.withdraw(q, r, item_id, quantity),
                InputCommand::Undo => self.undo(),
                InputCommand::Research { technology_id } => self.research(technology_id),
            };
            if let Err(error) = result {
                self.events.push(error);
            }
        }
        Ok(())
    }

    /// `deposit_available` is whether any deposit covering this extractor still holds stock. It is
    /// passed in rather than searched for: resolving it through the cached candidate list keeps a
    /// snapshot linear in entity count, where the equivalent tile scan made it quadratic.
    fn status_of(&self, entity: &Entity, deposit_available: bool) -> String {
        match entity.kind {
            BuildingKind::Extractor if entity.cargo.is_some() => "output blocked".into(),
            BuildingKind::Extractor if !deposit_available => "deposit depleted".into(),
            BuildingKind::Extractor if entity.progress > 0 => "extracting".into(),
            BuildingKind::Composer if entity.cargo.is_some() => "output blocked".into(),
            BuildingKind::Composer if entity.progress > 0 => "composing".into(),
            BuildingKind::Composer => "waiting for inputs".into(),
            BuildingKind::Container if inventory_total(&entity.inventory) > 0 => "buffered".into(),
            BuildingKind::Belt if entity.cargo.is_some() => "carrying".into(),
            BuildingKind::Consumer => "receiving".into(),
            BuildingKind::Hub => "landing hub".into(),
            _ => "idle".into(),
        }
    }

    /// One entity's snapshot. Every path that reports an entity to the host — the complete
    /// snapshot and the incremental delta alike — builds it here, so the sparse path cannot drift
    /// from the full one.
    fn entity_snapshot(&mut self, index: usize) -> EntitySnapshot {
        // Resolving through the cached candidate list rather than scanning the tile map is what
        // keeps this O(1) in world size. The cache is derived state, so filling it changes nothing.
        let deposit_available = self.entities[index].kind == BuildingKind::Extractor
            && self.extractor_deposit(index).is_some();
        let entity = &self.entities[index];
        let progress_total = match entity.kind {
            BuildingKind::Extractor => self
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
        };
        EntitySnapshot {
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
            status: self.status_of(entity, deposit_available),
            next_id: self.graph[index].map(|target| self.entities[target].id),
            footprint: self.entity_footprint(entity),
        }
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
        let span = size.saturating_mul(FEATURE_SPACING);
        let mut counts: BTreeMap<(i32, i32), usize> = BTreeMap::new();
        for entity in &self.entities {
            let (x, y) = axial_world(entity.placed.q, entity.placed.r);
            let chunk = (
                floor_div(floor_div(x, FEATURE_SPACING), size),
                floor_div(floor_div(y, FEATURE_SPACING), size),
            );
            *counts.entry(chunk).or_default() += 1;
        }
        self.generated_chunks
            .iter()
            .map(|&(chunk_q, chunk_r)| ChunkSnapshot {
                chunk_q,
                chunk_r,
                x: chunk_q.saturating_mul(span),
                y: chunk_r.saturating_mul(span),
                span,
                entity_count: counts.get(&(chunk_q, chunk_r)).copied().unwrap_or(0),
            })
            .collect()
    }

    fn terrain_snapshots(&self) -> Vec<TileSnapshot> {
        self.tiles
            .values()
            .filter(|tile| tile.terrain != Terrain::Ground)
            .map(|tile| TileSnapshot {
                x: tile.x,
                y: tile.y,
                radius: tile.radius,
                terrain: tile.terrain,
            })
            .collect()
    }

    /// One deposit's snapshot, looked up by tile key. Used by the incremental path, which knows
    /// which deposits moved but not where they sit in the tile map.
    fn resource_snapshot(&self, key: (i32, i32)) -> Option<ResourceSnapshot> {
        resource_snapshot_of(key, self.tiles.get(&key)?)
    }

    /// Every deposit, in tile order. This walks the map rather than looking each tile up again,
    /// because it visits all of them.
    fn resource_snapshots(&self) -> Vec<ResourceSnapshot> {
        self.tiles
            .iter()
            .filter_map(|(&key, tile)| resource_snapshot_of(key, tile))
            .collect()
    }

    fn delivered_by_item_snapshot(&self) -> Vec<Ingredient64> {
        self.delivered_by_item
            .iter()
            .map(|(&item_id, &quantity)| Ingredient64 { item_id, quantity })
            .collect()
    }

    fn objective_snapshot(&self) -> ObjectiveSnapshot {
        ObjectiveSnapshot {
            item_id: self.scenario.objective_item_id,
            delivered: self
                .delivered_by_item
                .get(&self.scenario.objective_item_id)
                .copied()
                .unwrap_or(0),
            required: self.scenario.objective_quantity,
        }
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
            objective: self.objective_snapshot(),
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
            hash_i32(&mut hash, tile.x);
            hash_i32(&mut hash, tile.y);
            hash_u32(&mut hash, tile.radius);
            hash_u32(
                &mut hash,
                match tile.terrain {
                    Terrain::Ground => 0,
                    Terrain::Water => 1,
                    Terrain::Rock => 2,
                },
            );
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
        hash
    }

    fn save_string(&self) -> Result<String, String> {
        let state = SavedState {
            seed: self.seed,
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
        )?;
        validate_saved_state(definitions, technologies, scenario, &envelope.state)?;
        core.seed = envelope.state.seed;
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
    objective: ObjectiveSnapshot,
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
            objective: snapshot.objective,
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
            objective: take_changed_copy(&mut baseline.objective, core.objective_snapshot()),
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
        let core =
            Core::new(&definitions, &technologies, scenario, seed_override).map_err(js_error)?;
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
    ) -> Result<(), JsValue> {
        let scenario = self
            .scenarios
            .scenarios
            .iter()
            .find(|scenario| scenario.key == scenario_key)
            .ok_or_else(|| js_error(format!("unknown scenario {scenario_key}")))?;
        self.core = Core::new(
            &self.definitions,
            &self.technologies,
            scenario,
            seed_override,
        )
        .map_err(js_error)?;
        self.baseline = None;
        Ok(())
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
    ) -> String {
        let cells = self
            .core
            .line_preview((q, r), (to_q, to_r), definition_id, orientation);
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
    let item_ids: BTreeSet<_> = definitions.items.iter().map(|item| item.id).collect();
    for item in &definitions.items {
        if item.key.trim().is_empty()
            || item.name.trim().is_empty()
            || item.color.trim().is_empty()
            || item.icon.trim().is_empty()
            || item.description.trim().is_empty()
            || item.insight_value == 0
            || item.stack_size == 0
        {
            return Err(format!(
                "item {} has incomplete display/value data",
                item.id
            ));
        }
    }
    for recipe in &definitions.recipes {
        if recipe.key.trim().is_empty()
            || recipe.name.trim().is_empty()
            || recipe.description.trim().is_empty()
            || recipe.duration == 0
            || recipe.inputs.is_empty()
            || recipe.output.quantity == 0
        {
            return Err(format!("recipe {} is incomplete", recipe.id));
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
        if building.kind == BuildingKind::Extractor && building.cadence.unwrap_or(0) == 0 {
            return Err(format!("extractor {} requires a cadence", building.id));
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
        for ingredient in &building.construction_cost {
            if ingredient.quantity == 0 || !item_ids.contains(&ingredient.item_id) {
                return Err(format!("building {} has an invalid cost", building.id));
            }
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
            || scenario.objective_quantity == 0
            || !item_ids.contains(&scenario.objective_item_id)
            || !keys.insert(scenario.key.clone())
        {
            return Err(format!("scenario {} is incomplete", scenario.id));
        }
        let mut occupied = BTreeSet::new();
        for building in &scenario.buildings {
            let definition = definitions
                .buildings
                .iter()
                .find(|definition| definition.id == building.definition_id);
            let footprint_clear = definition.map(|definition| {
                definition.footprint.iter().all(|offset| {
                    let offset = rotate_coordinate(*offset, building.orientation);
                    occupied.insert((building.q + offset.q, building.r + offset.r))
                })
            });
            if !building_ids.contains(&building.definition_id)
                || building.orientation >= 6
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
    let mut coordinates = BTreeSet::new();
    let mut entity_ids = BTreeSet::new();
    for entity in &state.entities {
        let definition = definitions
            .buildings
            .iter()
            .find(|value| value.id == entity.placed.definition_id)
            .ok_or("save references an unknown building")?;
        let footprint_valid = definition.footprint.iter().all(|offset| {
            let offset = rotate_coordinate(*offset, entity.placed.orientation);
            coordinates.insert((entity.placed.q + offset.q, entity.placed.r + offset.r))
        });
        if entity.kind != definition.kind
            || entity.placed.orientation >= 6
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
    let unique_tiles: BTreeSet<_> = state.tiles.iter().map(|tile| (tile.q, tile.r)).collect();
    if unique_tiles.len() != state.tiles.len() || state.tiles.iter().any(|tile| tile.radius == 0) {
        return Err("save contains duplicate tiles".into());
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

fn axial_distance(from: (i32, i32), to: (i32, i32)) -> i32 {
    let dq = to.0 - from.0;
    let dr = to.1 - from.1;
    (dq.abs() + dr.abs() + (dq + dr).abs()) / 2
}

/// The direction index that steps from one hex to an adjacent one, or `None` if they are not
/// neighbours.
fn step_direction(from: (i32, i32), to: (i32, i32)) -> Option<u8> {
    let delta = (to.0 - from.0, to.1 - from.1);
    DIRECTIONS
        .iter()
        .position(|direction| *direction == delta)
        .map(|index| index as u8)
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

/// The one overlap rule placement uses, for deposits and obstacles alike. `depth` is how far the
/// two circles must interpenetrate before the answer flips: zero is ordinary contact, and a larger
/// depth ignores a graze. See `DEPOSIT_COVERAGE_DEPTH` and `OBSTACLE_INTRUSION_DEPTH` — the two
/// checks previously disagreed about the question itself, not merely about a threshold, which made
/// a deposit between two hex centres unminable while a rock between two hex centres blocked both.
fn placement_overlap(ax: i32, ay: i32, ar: i32, bx: i32, by: i32, br: i32, depth: i32) -> bool {
    let reach = (ar + br - depth).max(0);
    squared_distance(ax, ay, bx, by) < i64::from(reach).pow(2)
}

fn resource_snapshot_of(key: (i32, i32), tile: &TileState) -> Option<ResourceSnapshot> {
    let resource = tile.resource.as_ref()?;
    Some(ResourceSnapshot {
        id: feature_id(key.0, key.1),
        x: tile.x,
        y: tile.y,
        radius: tile.radius,
        item_id: resource.item_id,
        quantity: resource.quantity,
        initial_quantity: resource.initial_quantity,
    })
}

fn feature_id(q: i32, r: i32) -> u64 {
    (u64::from(q as u32) << 32) | u64::from(r as u32)
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
    /// and a record must say which one it is.
    pub const REPORT_SCHEMA: u32 = 3;
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
        /// Mean serialized delta payload crossing the worker boundary per frame.
        pub delta_bytes: f64,
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
            chunk_size: 8,
            // Terrain is uniform ground so a tier measures transport and machines, not the
            // incidental obstacle layout of a generated seed.
            generated_environment: false,
            // Away from every line, so the idle player never blocks a footprint.
            player_spawn: Coordinate { q: -6, r: -6 },
            player_facing: 0,
            build_range: BUILD_RANGE_HEXES,
            // The workload's player never picks anything up, so this only has to be valid.
            carry_slots: 12,
            objective_item_id: 2,
            // Never reached, so victory cannot change the measured workload partway through.
            objective_quantity: u32::MAX,
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
        let mut core =
            Core::new(&definitions, &technologies, &scenario, None).expect("capacity core builds");
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
        let _ = factory.snapshot_delta_json();
        let mut bytes = 0usize;
        let (frame_us, frames) = phase(clock, budget, spec.frames, || {
            for _ in 0..spec.frames {
                // No player steps: the capacity workload measures the factory, and the idle player
                // has no movement intent to spend them on anyway.
                if factory.advance_json(IDLE_COMMANDS, 1, 0).is_err() {
                    panic!("capacity frame commands must be accepted");
                }
                bytes += factory.snapshot_delta_json().len();
            }
        });
        (frame_us, mean(bytes as f64, frames))
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
            "{:<8}{:>7}{:>10}{:>11}{:>10}{:>12}{:>12}{:>11}{:>10}{:>13}{:>12}{:>13}{:>10}",
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
            "compile us",
            "recompile us",
            "edit us",
        )
    }

    pub fn table_row(tier: &TierResult) -> String {
        format!(
            "{:<8}{:>7}{:>10}{:>11.1}{:>10.0}{:>12.1}{:>12.1}{:>11.1}{:>10.0}{:>13.0}{:>12.1}{:>13.1}{:>10.1}",
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
        let core = Core::new(&definitions, &technologies, &scenario, None).unwrap();
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
        *previous = current;
    }

    fn game(key: &str) -> Core {
        let (definitions, technologies, scenarios) = catalogs();
        let scenario = scenarios
            .scenarios
            .iter()
            .find(|value| value.key == key)
            .unwrap();
        Core::new(&definitions, &technologies, scenario, None).unwrap()
    }

    fn cooldown(core: &mut Core) {
        core.tick_many(2);
    }

    fn set_player_hex(core: &mut Core, q: i32, r: i32) {
        (core.player.x, core.player.y) = axial_world(q, r);
        core.ensure_neighborhood(core.player.x, core.player.y);
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
        assert_eq!(actual, DIRECTIONS);
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
    }

    #[test]
    fn continuous_movement_intent_and_collision_are_native() {
        let mut core = game("new-game");
        set_player_hex(&mut core, 10, 10);
        core.tiles
            .values_mut()
            .for_each(|feature| feature.terrain = Terrain::Ground);
        let start = (core.player.x, core.player.y);
        core.set_move_intent(707, -707).unwrap();
        core.advance_player_steps(3);
        assert_eq!(core.player.x, start.0 + 318);
        assert_eq!(core.player.y, start.1 - 318);
        assert_eq!((core.player.facing_x, core.player.facing_y), (707, -707));
        core.set_move_intent(0, 0).unwrap();
        core.advance_player_steps(3);
        assert_eq!(
            (core.player.x, core.player.y),
            (start.0 + 318, start.1 - 318)
        );
        assert!(core.set_move_intent(1001, 0).is_err());

        let rock_x = core.player.x + PLAYER_SPEED + PLAYER_RADIUS;
        let key = (999, 999);
        core.tiles.insert(
            key,
            TileState {
                q: key.0,
                r: key.1,
                x: rock_x,
                y: core.player.y,
                radius: 300,
                terrain: Terrain::Rock,
                resource: None,
            },
        );
        let blocked_x = core.player.x;
        core.set_move_intent(1000, 0).unwrap();
        core.advance_player_steps(1);
        assert_eq!(core.player.x, blocked_x);
    }

    #[test]
    fn the_player_walks_on_its_own_cadence_not_the_factorys() {
        // The complaint this answers: the player stopped when the factory paused and crawled at a
        // low speed multiplier, because walking ran inside the simulation tick.
        let mut core = game("new-game");
        set_player_hex(&mut core, 10, 10);
        core.tiles
            .values_mut()
            .for_each(|feature| feature.terrain = Terrain::Ground);
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
            set_player_hex(core, 10, 10);
            core.tiles
                .values_mut()
                .for_each(|feature| feature.terrain = Terrain::Ground);
        }
        for _ in 0..4 {
            slow.advance(IDLE_MOVE_EAST, 1, 8).unwrap();
            fast.advance(IDLE_MOVE_EAST, 16, 8).unwrap();
        }
        assert_eq!(slow.player.x, fast.player.x);
        assert_eq!(slow.player.y, fast.player.y);
        assert_eq!(Factory::player_ticks_per_second(), PLAYER_TICKS_PER_SECOND);
    }

    #[test]
    fn gathering_depletes_finite_resources_and_conserves_items() {
        let mut core = game("new-game");
        set_player_hex(&mut core, 3, 0);
        let before = core.tiles[&(3, 0)].resource.as_ref().unwrap().quantity;
        for _ in 0..before {
            core.gather().unwrap();
            cooldown(&mut core);
        }
        assert_eq!(core.player.inventory.get(&1), Some(&before));
        assert_eq!(core.tiles[&(3, 0)].resource.as_ref().unwrap().quantity, 0);
        assert!(core.gather().is_err());
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
        assert!(core.place(2, 0, 2, 0, None).unwrap_err().contains("cost"));
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
        assert!(core.events.iter().any(|event| event.contains("cost")));

        // A drag that can place nothing at all fails as the single placement would have.
        let mut empty = game("new-game");
        empty.researched.extend([1, 2, 3, 4]);
        assert!(empty
            .place_line((2, 0), (4, 1), 2, 0, None)
            .unwrap_err()
            .contains("cost"));
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

        let preview = core.line_preview((2, 0), (4, 1), 2, 0);
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
        // The defect this pins: a deposit was tested by whether the hex centre fell inside it, an
        // obstacle by whether two circles touched at all. A hex step is 1774 world units, so the
        // first test made a deposit sitting between two hex centres unminable from either, while
        // the second let one rock between two hex centres block both.
        let mut core = game("new-game");
        core.researched.extend([1, 2, 3, 4]);
        core.player.inventory.insert(1, 20);
        core.player.inventory.insert(3, 10);

        // A deposit displaced most of the way to the next hex is still minable from a hex.
        let (hex_x, hex_y) = axial_world(3, 0);
        let displaced = core.tiles.get_mut(&(3, 0)).unwrap();
        displaced.x = hex_x + 900;
        displaced.radius = 520;
        core.deposit_links.clear();
        set_player_hex(&mut core, 3, 1);
        assert!(
            core.resource_at_world(hex_x, hex_y).is_some(),
            "a deposit within one hex step must be reachable from that hex"
        );
        core.place(3, 0, 1, 0, None).unwrap();

        // The extractor's cached candidate list resolves the same deposit the placement rule used;
        // the two are the same predicate, so they cannot drift apart.
        let index = core.entity_at(3, 0).unwrap();
        assert_eq!(core.extractor_deposit(index), Some((3, 0)));

        // An obstacle that merely grazes a hex no longer makes it unbuildable, and one that
        // genuinely intrudes still does.
        let mut ground = game("new-game");
        ground.researched.extend([1, 2, 3, 4]);
        ground.player.inventory.insert(1, 20);
        let (cell_x, cell_y) = axial_world(2, 0);
        let grazing = (777, 777);
        ground.tiles.insert(
            grazing,
            TileState {
                q: grazing.0,
                r: grazing.1,
                x: cell_x + BUILDING_RADIUS + 660 - OBSTACLE_INTRUSION_DEPTH + 1,
                y: cell_y,
                radius: 660,
                terrain: Terrain::Rock,
                resource: None,
            },
        );
        ground.place(2, 0, 2, 0, None).unwrap();
        ground.erase(2, 0).unwrap();
        ground.tiles.get_mut(&grazing).unwrap().x =
            cell_x + BUILDING_RADIUS + 660 - OBSTACLE_INTRUSION_DEPTH - 1;
        assert!(ground
            .place(2, 0, 2, 0, None)
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
        core.player.inventory.insert(3, 1);
        set_player_hex(&mut core, 3, 1);
        core.tiles
            .get_mut(&(3, 0))
            .unwrap()
            .resource
            .as_mut()
            .unwrap()
            .quantity = 2;
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
        assert_eq!(core.tiles[&(3, 0)].resource.as_ref().unwrap().quantity, 0);
        assert_eq!(core.produced.get(&1), Some(&2));
        assert_eq!(entity.progress, 0);
    }

    #[test]
    fn resolved_deposit_references_match_a_full_tile_scan_and_survive_generation() {
        let mut core = game("new-game");
        core.researched.insert(2);
        core.player.inventory.insert(1, 8);
        core.player.inventory.insert(3, 2);
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

        // A drained deposit falls through to the scan's next choice without re-resolving.
        core.tiles
            .get_mut(&(3, 0))
            .unwrap()
            .resource
            .as_mut()
            .unwrap()
            .quantity = 0;
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
        let resource_before = core.tiles[&(-4, 0)].resource.as_ref().unwrap().quantity;
        core.tick_many(100);
        assert_eq!(core.entities[extractor].cargo.unwrap().quantity, 1);
        assert_eq!(
            core.tiles[&(-4, 0)].resource.as_ref().unwrap().quantity,
            resource_before - 1
        );
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
    fn complete_native_progression_reaches_persistent_victory() {
        let mut core = game("new-game");
        core.player.inventory.insert(1, 8);
        core.player.inventory.insert(3, 4);
        set_player_hex(&mut core, 1, 0);
        core.deposit_inventory().unwrap();
        core.research(1).unwrap();
        core.research(2).unwrap();
        core.research(3).unwrap();
        core.player.inventory.insert(1, 30);
        core.player.inventory.insert(3, 8);
        set_player_hex(&mut core, 3, 1);
        core.place(3, 0, 1, 3, None).unwrap();
        core.place(2, 0, 2, 3, None).unwrap();
        core.place(1, 0, 3, 3, Some(1)).unwrap();
        core.tick_many(500);
        assert!(core.victory);
        let checksum = core.checksum();
        core.tick_many(1);
        assert!(core.victory);
        assert_ne!(core.checksum(), checksum);
    }

    #[test]
    fn hxf1_round_trip_and_resume_match_uninterrupted_run() {
        let (definitions, technologies, scenarios) = catalogs();
        let mut uninterrupted = game("factory-demo");
        uninterrupted.tick_many(120);
        let save = uninterrupted.save_string().unwrap();
        assert!(save.starts_with(SAVE_PREFIX));
        let mut resumed = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
        uninterrupted.tick_many(180);
        resumed.tick_many(180);
        assert_eq!(uninterrupted.checksum(), resumed.checksum());
        assert_eq!(uninterrupted.delivered, resumed.delivered);
        assert!(Core::from_save(&definitions, &technologies, &scenarios, "bad").is_err());
        let incompatible =
            save.replacen("\"definition_version\":4", "\"definition_version\":999", 1);
        assert!(Core::from_save(&definitions, &technologies, &scenarios, &incompatible).is_err());
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
        let mut a = Core::new(&definitions, &technologies, scenario, None).unwrap();
        let mut b = Core::new(&definitions, &technologies, &reversed, None).unwrap();
        a.tick_many(300);
        b.tick_many(300);
        assert_eq!(a.checksum(), b.checksum());
        let expected = a.checksum();
        let mut replay = Core::new(&definitions, &technologies, scenario, None).unwrap();
        replay.tick_many(300);
        assert_eq!(replay.checksum(), expected);
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
        let span = 8 * FEATURE_SPACING;
        assert!(!snapshot.chunks.is_empty());
        for chunk in &snapshot.chunks {
            assert_eq!(chunk.span, span);
            assert_eq!(chunk.x, chunk.chunk_q * span);
            assert_eq!(chunk.y, chunk.chunk_r * span);
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
        let (far_x, far_y) = (span * 4, span * 4);
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
        core.player.inventory.insert(3, 4);
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
        set_player_hex(&mut factory.core, 4, -2);
        factory
            .core
            .tiles
            .get_mut(&(4, -2))
            .unwrap()
            .resource
            .as_mut()
            .unwrap()
            .quantity = 2;
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

        // Gathering, through the tick the deposit runs dry and one rejected attempt after it.
        for round in 0..3 {
            factory
                .core
                .advance(r#"[{"type":"gather"}]"#, 2, 0)
                .unwrap();
            check(&mut factory, &format!("gather attempt {round}"));
        }
        assert_eq!(
            factory.core.tiles[&(4, -2)]
                .resource
                .as_ref()
                .unwrap()
                .quantity,
            0
        );

        // Delivery and research: insight, delivered totals, the objective, and unlocks.
        set_player_hex(&mut factory.core, 1, 0);
        check(&mut factory, "walking to the landing hub");
        factory
            .core
            .advance(r#"[{"type":"deposit"}]"#, 1, 0)
            .unwrap();
        check(&mut factory, "delivering inventory to the hub");
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

        // Travel into unsurveyed world: terrain, deposits, chunk bounds, and every extractor's
        // resolved deposit reference at once.
        for (label, command) in [
            ("east", r#"[{"type":"move_intent","x":1000,"y":0}]"#),
            ("south", r#"[{"type":"move_intent","x":0,"y":1000}]"#),
        ] {
            // Distance now comes from the player's own cadence rather than from the tick count.
            factory.core.advance(command, 60, 120).unwrap();
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
        core.player.inventory.insert(3, 2);
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
        core.player.inventory.insert(3, 20);
        set_player_hex(&mut core, 3, 1);
        core.place(3, 0, 1, 0, None).unwrap();
        let index = core.entity_at(3, 0).unwrap();

        let scanned = |core: &Core| {
            let (x, y) = axial_world(core.entities[index].placed.q, core.entities[index].placed.r);
            core.resource_at_world(x, y)
                .and_then(|key| core.tiles.get(&key))
                .and_then(|tile| tile.resource.as_ref())
                .map(|resource| resource.quantity)
                .unwrap_or(0)
                > 0
        };

        for _ in 0..3 {
            let expected = scanned(&core);
            assert_eq!(core.extractor_deposit(index).is_some(), expected);
            let entity = core.entities[index].clone();
            assert_eq!(
                core.status_of(&entity, expected),
                core.entity_snapshot(index).status
            );
            core.tick_many(20);
        }

        // Draining the deposit must flip both the scan and the cached reference together.
        core.tiles
            .get_mut(&(3, 0))
            .unwrap()
            .resource
            .as_mut()
            .unwrap()
            .quantity = 0;
        assert!(!scanned(&core));
        assert!(core.extractor_deposit(index).is_none());
        core.entities[index].cargo = None;
        assert_eq!(core.entity_snapshot(index).status, "deposit depleted");
    }

    #[test]
    fn combined_advance_preserves_command_events_through_native_ticks() {
        let mut core = game("new-game");
        core.player.inventory.insert(1, 8);
        core.player.inventory.insert(3, 4);
        set_player_hex(&mut core, 1, 0);
        core.advance(r#"[{"type":"deposit"}]"#, 1, 0).unwrap();
        assert_eq!(core.tick, 1);
        assert!(core
            .events
            .iter()
            .any(|event| event.contains("Delivered inventory")));
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
        // invalidate comparisons against previously recorded tier numbers.
        assert_eq!(first.checksum(), 1_693_021_923);
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
