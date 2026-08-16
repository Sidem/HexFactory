use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use wasm_bindgen::prelude::*;

type ItemId = u16;
type RecipeId = u16;
type DefinitionId = u16;
type TechnologyId = u16;

const SAVE_PREFIX: &str = "HXF1\n";
const SAVE_VERSION: u16 = 2;
const WORLD_GENERATOR_VERSION: u16 = 2;
const MAX_COMMANDS_PER_BATCH: usize = 8;
const GRAPH_TRACE_LIMIT: i32 = 8;
const DIRECTIONS: [(i32, i32); 6] = [(1, 0), (0, 1), (-1, 1), (-1, 0), (0, -1), (1, -1)];
const HEX_X: i32 = 1774;
const HEX_Y: i32 = 1536;
const FEATURE_SPACING: i32 = 2048;
const PLAYER_SPEED: i32 = 300;
const PLAYER_RADIUS: i32 = 360;
const BUILDING_RADIUS: i32 = 690;
const GATHER_RANGE: i32 = 1450;
const HUB_RANGE: i32 = 1900;

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

#[derive(Serialize)]
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
    player: PlayerState,
    researched: Vec<TechnologyId>,
    chunks: Vec<ChunkSnapshot>,
    terrain: Vec<TileSnapshot>,
    resources: Vec<ResourceSnapshot>,
    buildings: Vec<EntitySnapshot>,
    events: Vec<String>,
}

#[derive(Serialize)]
struct Ingredient64 {
    item_id: ItemId,
    quantity: u64,
}

#[derive(Serialize)]
struct ObjectiveSnapshot {
    item_id: ItemId,
    delivered: u64,
    required: u32,
}

#[derive(Serialize)]
struct ChunkSnapshot {
    chunk_q: i32,
    chunk_r: i32,
    entity_count: usize,
}

#[derive(Serialize)]
struct TileSnapshot {
    x: i32,
    y: i32,
    radius: u32,
    terrain: Terrain,
}

#[derive(Serialize)]
struct ResourceSnapshot {
    id: u64,
    x: i32,
    y: i32,
    radius: u32,
    item_id: ItemId,
    quantity: u32,
    initial_quantity: u32,
}

#[derive(Serialize)]
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

#[derive(Serialize)]
struct PlacementPreview {
    legal: bool,
    reason: String,
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
    Erase {
        q: i32,
        r: i32,
    },
    Rotate {
        q: i32,
        r: i32,
    },
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

    fn entity_at(&self, q: i32, r: i32) -> Option<usize> {
        self.entities.iter().position(|entity| {
            self.entity_footprint(entity)
                .iter()
                .any(|cell| cell.q == q && cell.r == r)
        })
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
                    && squared_distance(x, y, feature.x, feature.y)
                        <= i64::from(feature.radius).pow(2)
            })
            .min_by_key(|(_, feature)| squared_distance(x, y, feature.x, feature.y))
            .map(|(key, _)| *key)
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
        let size = self.scenario.chunk_size;
        for local_r in 0..size {
            for local_q in 0..size {
                let q = chunk_q * size + local_q;
                let r = chunk_r * size + local_r;
                self.tiles.insert((q, r), self.generated_tile(q, r));
            }
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
        recompiled
    }

    fn tick_many(&mut self, count: u32) {
        if count > 0 {
            self.events.clear();
        }
        for _ in 0..count {
            self.player.action_cooldown = self.player.action_cooldown.saturating_sub(1);
            self.advance_player();
            self.advance_machines();
            self.transfer_cargo();
            self.tick += 1;
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
        let (extractor_x, extractor_y) = axial_world(q, r);
        let resource_key = self.resource_at_world(extractor_x, extractor_y);
        let available = self
            .tiles
            .get(&resource_key.unwrap_or((i32::MIN, i32::MIN)))
            .and_then(|tile| tile.resource.as_ref())
            .map(|resource| resource.quantity)
            .unwrap_or(0);
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
        let resource = self
            .tiles
            .get_mut(&resource_key.expect("available resource key exists"))
            .and_then(|tile| tile.resource.as_mut())
            .expect("available resource exists");
        resource.quantity -= 1;
        let item_id = resource.item_id;
        self.entities[index].cargo = Some(Cargo {
            item_id,
            quantity: 1,
        });
        self.entities[index].progress = 0;
        *self.produced.entry(item_id).or_default() += 1;
        if resource.quantity == 0 {
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
        let feature = self.tiles.get_mut(&key).expect("selected resource exists");
        let resource = feature.resource.as_mut().expect("selected resource exists");
        resource.quantity -= 1;
        *self.player.inventory.entry(resource.item_id).or_default() += 1;
        self.player.action_cooldown = 2;
        self.events
            .push(format!("Gathered item {}", resource.item_id));
        if resource.quantity == 0 {
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
                    && circles_overlap(
                        cell_x,
                        cell_y,
                        BUILDING_RADIUS,
                        feature.x,
                        feature.y,
                        feature.radius as i32,
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
        let changed_cells = self
            .footprint_for(placed, orientation)
            .into_iter()
            .map(|cell| (cell.q, cell.r))
            .collect();
        self.recompile_graph_components(&old_links, &changed_cells, &BTreeSet::from([id]));
        self.events.push(format!("Placed {}", definition.name));
        Ok(())
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
        let old_links = self.graph_links_by_id();
        let changed_cells = self
            .entity_footprint(&self.entities[index])
            .into_iter()
            .map(|cell| (cell.q, cell.r))
            .collect();
        let entity = self.entities.remove(index);
        let definition = self
            .building_definition(entity.placed.definition_id)
            .unwrap()
            .clone();
        add_ingredients(&mut self.player.inventory, &definition.construction_cost);
        add_inventory(&mut self.player.inventory, &entity.inventory);
        add_inventory(&mut self.player.inventory, &entity.reserved_inputs);
        if let Some(cargo) = entity.cargo {
            *self.player.inventory.entry(cargo.item_id).or_default() += cargo.quantity;
        }
        self.recompile_graph_components(&old_links, &changed_cells, &BTreeSet::from([entity.id]));
        self.events.push(format!("Recovered {}", definition.name));
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
        if commands.len() > MAX_COMMANDS_PER_BATCH {
            return Err(format!(
                "input batch exceeds the native limit of {MAX_COMMANDS_PER_BATCH}"
            ));
        }
        self.events.clear();
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
                InputCommand::Erase { q, r } => self.erase(q, r),
                InputCommand::Rotate { q, r } => self.rotate(q, r),
                InputCommand::Research { technology_id } => self.research(technology_id),
            };
            if let Err(error) = result {
                self.events.push(error);
            }
        }
        Ok(())
    }

    fn status(&self, entity: &Entity) -> String {
        match entity.kind {
            BuildingKind::Extractor if entity.cargo.is_some() => "output blocked".into(),
            BuildingKind::Extractor
                if self
                    .resource_at_world(
                        axial_world(entity.placed.q, entity.placed.r).0,
                        axial_world(entity.placed.q, entity.placed.r).1,
                    )
                    .and_then(|key| self.tiles.get(&key))
                    .and_then(|tile| tile.resource.as_ref())
                    .map(|resource| resource.quantity)
                    .unwrap_or(0)
                    == 0 =>
            {
                "deposit depleted".into()
            }
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

    fn snapshot(&self) -> Snapshot {
        let mut indices: Vec<usize> = (0..self.entities.len()).collect();
        indices.sort_by_key(|&index| self.entities[index].id);
        let chunks = self
            .generated_chunks
            .iter()
            .map(|&(chunk_q, chunk_r)| ChunkSnapshot {
                chunk_q,
                chunk_r,
                entity_count: self
                    .entities
                    .iter()
                    .filter(|entity| {
                        let (x, y) = axial_world(entity.placed.q, entity.placed.r);
                        floor_div(floor_div(x, FEATURE_SPACING), self.scenario.chunk_size)
                            == chunk_q
                            && floor_div(floor_div(y, FEATURE_SPACING), self.scenario.chunk_size)
                                == chunk_r
                    })
                    .count(),
            })
            .collect();
        Snapshot {
            scenario: self.scenario.key.clone(),
            scenario_name: self.scenario.name.clone(),
            world_version: WORLD_GENERATOR_VERSION,
            seed: self.seed,
            tick: self.tick,
            checksum: self.checksum(),
            delivered: self.delivered,
            delivered_by_item: self
                .delivered_by_item
                .iter()
                .map(|(&item_id, &quantity)| Ingredient64 { item_id, quantity })
                .collect(),
            insight: self.insight,
            victory: self.victory,
            objective: ObjectiveSnapshot {
                item_id: self.scenario.objective_item_id,
                delivered: self
                    .delivered_by_item
                    .get(&self.scenario.objective_item_id)
                    .copied()
                    .unwrap_or(0),
                required: self.scenario.objective_quantity,
            },
            player: self.player.clone(),
            researched: self.researched.iter().copied().collect(),
            chunks,
            terrain: self
                .tiles
                .values()
                .filter(|tile| tile.terrain != Terrain::Ground)
                .map(|tile| TileSnapshot {
                    x: tile.x,
                    y: tile.y,
                    radius: tile.radius,
                    terrain: tile.terrain,
                })
                .collect(),
            resources: self
                .tiles
                .iter()
                .filter_map(|(&(q, r), tile)| {
                    let resource = tile.resource.as_ref()?;
                    Some(ResourceSnapshot {
                        id: feature_id(q, r),
                        x: tile.x,
                        y: tile.y,
                        radius: tile.radius,
                        item_id: resource.item_id,
                        quantity: resource.quantity,
                        initial_quantity: resource.initial_quantity,
                    })
                })
                .collect(),
            buildings: indices
                .into_iter()
                .map(|index| {
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
                        status: self.status(entity),
                        next_id: self.graph[index].map(|target| self.entities[target].id),
                        footprint: self.entity_footprint(entity),
                    }
                })
                .collect(),
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
        core.entities = envelope.state.entities;
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

#[wasm_bindgen]
pub struct Factory {
    definitions: DefinitionsInput,
    technologies: TechnologiesInput,
    scenarios: ScenariosInput,
    core: Core,
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
        Ok(())
    }

    pub fn apply_commands_json(&mut self, commands_json: &str) -> Result<(), JsValue> {
        self.core.apply_commands(commands_json).map_err(js_error)
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

    pub fn snapshot_json(&self) -> String {
        serde_json::to_string(&self.core.snapshot()).expect("snapshot is serializable")
    }

    pub fn save_string(&self) -> Result<String, JsValue> {
        self.core.save_string().map_err(js_error)
    }

    pub fn load_string(&mut self, save: &str) -> Result<(), JsValue> {
        self.core = Core::from_save(&self.definitions, &self.technologies, &self.scenarios, save)
            .map_err(js_error)?;
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

fn squared_distance(ax: i32, ay: i32, bx: i32, by: i32) -> i64 {
    let dx = i64::from(ax) - i64::from(bx);
    let dy = i64::from(ay) - i64::from(by);
    dx * dx + dy * dy
}

fn circles_overlap(ax: i32, ay: i32, ar: i32, bx: i32, by: i32, br: i32) -> bool {
    squared_distance(ax, ay, bx, by) < i64::from(ar + br).pow(2)
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
        core.tick_many(3);
        assert_eq!(core.player.x, start.0 + 636);
        assert_eq!(core.player.y, start.1 - 636);
        assert_eq!((core.player.facing_x, core.player.facing_y), (707, -707));
        core.set_move_intent(0, 0).unwrap();
        core.tick_many(3);
        assert_eq!(
            (core.player.x, core.player.y),
            (start.0 + 636, start.1 - 636)
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
        core.tick_many(1);
        assert_eq!(core.player.x, blocked_x);
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
            save.replacen("\"definition_version\":3", "\"definition_version\":999", 1);
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
}
