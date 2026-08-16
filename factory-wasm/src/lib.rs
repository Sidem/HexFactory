use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use wasm_bindgen::prelude::*;

type ItemId = u16;
type RecipeId = u16;
type DefinitionId = u16;
type TechnologyId = u16;

const SAVE_PREFIX: &str = "HXF1\n";
const SAVE_VERSION: u16 = 1;
const WORLD_GENERATOR_VERSION: u16 = 1;
const MAX_COMMANDS_PER_BATCH: usize = 8;
const DIRECTIONS: [(i32, i32); 6] = [(1, 0), (0, 1), (-1, 1), (-1, 0), (0, -1), (1, -1)];

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
    terrain: Terrain,
    resource: Option<ResourceState>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct PlayerState {
    q: i32,
    r: i32,
    facing: u8,
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
    q: i32,
    r: i32,
    terrain: Terrain,
}

#[derive(Serialize)]
struct ResourceSnapshot {
    q: i32,
    r: i32,
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
}

#[derive(Serialize)]
struct PlacementPreview {
    legal: bool,
    reason: String,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum InputCommand {
    Move {
        direction: u8,
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
                q: scenario.player_spawn.q,
                r: scenario.player_spawn.r,
                facing: scenario.player_facing,
                inventory,
                action_cooldown: 0,
                build_range: scenario.build_range,
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
        core.ensure_neighborhood(scenario.player_spawn.q, scenario.player_spawn.r);
        for resource in &scenario.resources {
            core.ensure_tile(resource.q, resource.r);
            let tile = core.tiles.get_mut(&(resource.q, resource.r)).unwrap();
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
            core.tiles.get_mut(&(placed.q, placed.r)).unwrap().terrain = Terrain::Ground;
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
        if !self.scenario.generated_environment {
            return TileState {
                q,
                r,
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
            return TileState {
                q,
                r,
                terrain,
                resource: resource.map(|(item_id, quantity)| ResourceState {
                    item_id,
                    quantity,
                    initial_quantity: quantity,
                }),
            };
        }
        let hash = coordinate_hash(self.seed, q, r);
        let near_landing = axial_distance(q, r, 0, 0) <= 7;
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
            terrain,
            resource,
        }
    }

    fn ensure_tile(&mut self, q: i32, r: i32) {
        let size = self.scenario.chunk_size;
        self.generate_chunk(floor_div(q, size), floor_div(r, size));
    }

    fn ensure_neighborhood(&mut self, q: i32, r: i32) {
        let size = self.scenario.chunk_size;
        let center = (floor_div(q, size), floor_div(r, size));
        self.generate_chunk(center.0, center.1);
        for (dq, dr) in DIRECTIONS {
            self.generate_chunk(center.0 + dq, center.1 + dr);
        }
    }

    fn compile_graph(&mut self) {
        let occupied: BTreeMap<(i32, i32), usize> = self
            .entities
            .iter()
            .enumerate()
            .map(|(index, entity)| ((entity.placed.q, entity.placed.r), index))
            .collect();
        self.graph = self
            .entities
            .iter()
            .map(|entity| {
                let (dq, dr) = DIRECTIONS[usize::from(entity.placed.orientation % 6)];
                occupied
                    .get(&(entity.placed.q + dq, entity.placed.r + dr))
                    .copied()
            })
            .collect();
    }

    fn tick_many(&mut self, count: u32) {
        if count > 0 {
            self.events.clear();
        }
        for _ in 0..count {
            self.player.action_cooldown = self.player.action_cooldown.saturating_sub(1);
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
        let available = self
            .tiles
            .get(&(q, r))
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
            .get_mut(&(q, r))
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

    fn move_player(&mut self, direction: u8) -> Result<(), String> {
        if direction >= 6 {
            return Err("direction must be in 0..6".into());
        }
        self.player.facing = direction;
        if self.player.action_cooldown > 0 {
            return Err("movement cooling down".into());
        }
        let (dq, dr) = DIRECTIONS[usize::from(direction)];
        let q = self.player.q + dq;
        let r = self.player.r + dr;
        self.ensure_neighborhood(q, r);
        let terrain = self.tiles.get(&(q, r)).unwrap().terrain;
        if terrain != Terrain::Ground {
            return Err(match terrain {
                Terrain::Water => "water blocks movement",
                Terrain::Rock => "rock blocks movement",
                Terrain::Ground => unreachable!(),
            }
            .into());
        }
        if self.entities.iter().any(|entity| {
            entity.placed.q == q
                && entity.placed.r == r
                && self
                    .building_definition(entity.placed.definition_id)
                    .map(|definition| definition.blocks_movement)
                    .unwrap_or(true)
        }) {
            return Err("a building blocks movement".into());
        }
        self.player.q = q;
        self.player.r = r;
        self.player.action_cooldown = 1;
        Ok(())
    }

    fn gather(&mut self) -> Result<(), String> {
        if self.player.action_cooldown > 0 {
            return Err("action cooling down".into());
        }
        let (dq, dr) = DIRECTIONS[usize::from(self.player.facing % 6)];
        let candidates = [
            (self.player.q, self.player.r),
            (self.player.q + dq, self.player.r + dr),
        ];
        for (q, r) in candidates {
            self.ensure_tile(q, r);
            let Some(resource) = self
                .tiles
                .get_mut(&(q, r))
                .and_then(|tile| tile.resource.as_mut())
            else {
                continue;
            };
            if resource.quantity == 0 {
                continue;
            }
            resource.quantity -= 1;
            *self.player.inventory.entry(resource.item_id).or_default() += 1;
            self.player.action_cooldown = 2;
            self.events
                .push(format!("Gathered item {}", resource.item_id));
            if resource.quantity == 0 {
                self.events.push(format!("Deposit at {q},{r} depleted"));
            }
            return Ok(());
        }
        Err("no finite resource here or ahead".into())
    }

    fn deposit_inventory(&mut self) -> Result<(), String> {
        let hub = self
            .entities
            .iter()
            .find(|entity| entity.kind == BuildingKind::Hub);
        let Some(hub) = hub else {
            return Err("this scenario has no landing hub".into());
        };
        if axial_distance(self.player.q, self.player.r, hub.placed.q, hub.placed.r) > 1 {
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
        if axial_distance(self.player.q, self.player.r, q, r) > self.player.build_range {
            return Err("placement is outside build range".into());
        }
        if self
            .entities
            .iter()
            .any(|entity| entity.placed.q == q && entity.placed.r == r)
        {
            return Err("hex is occupied".into());
        }
        if self.player.q == q && self.player.r == r {
            return Err("the player occupies this hex".into());
        }
        let generated_tile;
        let tile = if let Some(tile) = self.tiles.get(&(q, r)) {
            tile
        } else {
            generated_tile = self.generated_tile(q, r);
            &generated_tile
        };
        if tile.terrain != Terrain::Ground {
            return Err("terrain blocks construction".into());
        }
        if definition.placement_rule == PlacementRule::Resource
            && tile
                .resource
                .as_ref()
                .map(|resource| resource.quantity)
                .unwrap_or(0)
                == 0
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
        self.ensure_tile(q, r);
        self.placement_legality(q, r, definition_id, orientation, recipe_id, true)?;
        let definition = self.building_definition(definition_id).unwrap().clone();
        for ingredient in &definition.construction_cost {
            subtract_item(
                &mut self.player.inventory,
                ingredient.item_id,
                ingredient.quantity,
            );
        }
        self.entities.push(Entity {
            id: self.next_entity_id,
            placed: PlacedBuilding {
                q,
                r,
                definition_id,
                orientation,
                recipe_id,
                scenario_owned: false,
            },
            kind: definition.kind,
            cargo: None,
            inventory: BTreeMap::new(),
            reserved_inputs: BTreeMap::new(),
            progress: 0,
        });
        self.next_entity_id += 1;
        self.compile_graph();
        self.events.push(format!("Placed {}", definition.name));
        Ok(())
    }

    fn erase(&mut self, q: i32, r: i32) -> Result<(), String> {
        if axial_distance(self.player.q, self.player.r, q, r) > self.player.build_range {
            return Err("erase target is outside build range".into());
        }
        let index = self
            .entities
            .iter()
            .position(|entity| entity.placed.q == q && entity.placed.r == r)
            .ok_or("no building to erase")?;
        if self.entities[index].placed.scenario_owned {
            return Err("scenario-owned objects are protected".into());
        }
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
        self.compile_graph();
        self.events.push(format!("Recovered {}", definition.name));
        Ok(())
    }

    fn rotate(&mut self, q: i32, r: i32) -> Result<(), String> {
        if axial_distance(self.player.q, self.player.r, q, r) > self.player.build_range {
            return Err("rotate target is outside build range".into());
        }
        let entity = self
            .entities
            .iter_mut()
            .find(|entity| entity.placed.q == q && entity.placed.r == r)
            .ok_or("no building to rotate")?;
        if entity.placed.scenario_owned {
            return Err("scenario-owned objects are protected".into());
        }
        entity.placed.orientation = (entity.placed.orientation + 1) % 6;
        self.compile_graph();
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
                InputCommand::Move { direction } => self.move_player(direction),
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
                    .tiles
                    .get(&(entity.placed.q, entity.placed.r))
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
                        floor_div(entity.placed.q, self.scenario.chunk_size) == chunk_q
                            && floor_div(entity.placed.r, self.scenario.chunk_size) == chunk_r
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
                .map(|tile| TileSnapshot {
                    q: tile.q,
                    r: tile.r,
                    terrain: tile.terrain,
                })
                .collect(),
            resources: self
                .tiles
                .values()
                .filter_map(|tile| {
                    let resource = tile.resource.as_ref()?;
                    Some(ResourceSnapshot {
                        q: tile.q,
                        r: tile.r,
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
        hash_i32(&mut hash, self.player.q);
        hash_i32(&mut hash, self.player.r);
        hash_u32(&mut hash, u32::from(self.player.facing));
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
            if !building_ids.contains(&building.definition_id)
                || building.orientation >= 6
                || !occupied.insert((building.q, building.r))
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
        if entity.kind != definition.kind
            || entity.placed.orientation >= 6
            || !coordinates.insert((entity.placed.q, entity.placed.r))
            || !entity_ids.insert(entity.id)
        {
            return Err("save contains invalid entity state".into());
        }
    }
    if state.player.facing >= 6
        || state.player.build_range != scenario.build_range
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
    if unique_tiles.len() != state.tiles.len() {
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

fn axial_distance(aq: i32, ar: i32, bq: i32, br: i32) -> u32 {
    let dq = aq - bq;
    let dr = ar - br;
    let ds = -dq - dr;
    ((dq.abs() + dr.abs() + ds.abs()) / 2) as u32
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
    fn six_direction_movement_facing_blocking_and_cadence_are_native() {
        let mut core = game("new-game");
        core.player.q = 10;
        core.player.r = 10;
        core.ensure_neighborhood(10, 10);
        for direction in 0..6 {
            let start = (core.player.q, core.player.r);
            let target = (
                start.0 + DIRECTIONS[direction as usize].0,
                start.1 + DIRECTIONS[direction as usize].1,
            );
            core.ensure_tile(target.0, target.1);
            core.tiles.get_mut(&target).unwrap().terrain = Terrain::Ground;
            core.move_player(direction).unwrap();
            assert_eq!(core.player.facing, direction);
            assert_eq!(
                (core.player.q, core.player.r),
                (
                    start.0 + DIRECTIONS[direction as usize].0,
                    start.1 + DIRECTIONS[direction as usize].1
                )
            );
            assert!(core.move_player(direction).is_err());
            cooldown(&mut core);
        }
        core.player.q = 1;
        core.player.r = 0;
        core.player.action_cooldown = 0;
        assert!(core.move_player(5).is_err());
        assert_eq!(core.player.facing, 5);
    }

    #[test]
    fn gathering_depletes_finite_resources_and_conserves_items() {
        let mut core = game("new-game");
        core.player.q = 3;
        core.player.r = 0;
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
            .contains("terrain"));
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
        core.player.q = 100;
        core.player.r = 100;
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
    fn extractor_stops_exactly_when_its_deposit_empties() {
        let mut core = game("new-game");
        core.researched.insert(2);
        core.player.inventory.insert(1, 4);
        core.player.inventory.insert(3, 1);
        core.player.q = 3;
        core.player.r = 1;
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
        core.player.q = 1;
        core.player.r = 0;
        core.deposit_inventory().unwrap();
        core.research(1).unwrap();
        core.research(2).unwrap();
        core.research(3).unwrap();
        core.player.inventory.insert(1, 30);
        core.player.inventory.insert(3, 8);
        core.player.q = 3;
        core.player.r = 1;
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
            save.replacen("\"definition_version\":2", "\"definition_version\":999", 1);
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
