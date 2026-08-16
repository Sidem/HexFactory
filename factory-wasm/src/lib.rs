use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use wasm_bindgen::prelude::*;

type ItemId = u16;
type RecipeId = u16;
type DefinitionId = u16;

const DIRECTIONS: [(i32, i32); 6] = [(1, 0), (0, 1), (-1, 1), (-1, 0), (0, -1), (1, -1)];

#[derive(Clone, Deserialize)]
struct DefinitionsInput {
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
}

#[derive(Clone, Deserialize)]
struct RecipeDefinition {
    id: RecipeId,
    key: String,
    name: String,
    inputs: Vec<Ingredient>,
    output: Ingredient,
    duration: u32,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
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
    #[serde(default)]
    cadence: Option<u32>,
    #[serde(default)]
    capacity: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum BuildingKind {
    Extractor,
    Belt,
    Composer,
    Container,
    Consumer,
}

#[derive(Clone, Deserialize)]
struct BlueprintInput {
    chunk_size: i32,
    resources: Vec<ResourceNode>,
    buildings: Vec<PlacedBuilding>,
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct ResourceNode {
    q: i32,
    r: i32,
    item_id: ItemId,
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
struct PlacedBuilding {
    q: i32,
    r: i32,
    definition_id: DefinitionId,
    orientation: u8,
    #[serde(default)]
    recipe_id: Option<RecipeId>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct Cargo {
    item_id: ItemId,
    quantity: u32,
}

#[derive(Clone)]
struct Entity {
    id: u32,
    placed: PlacedBuilding,
    kind: BuildingKind,
    cargo: Option<Cargo>,
    inventory: BTreeMap<ItemId, u32>,
    progress: u32,
}

#[derive(Serialize)]
struct Snapshot {
    tick: u64,
    checksum: u32,
    delivered: u64,
    chunks: Vec<ChunkSnapshot>,
    resources: Vec<ResourceNode>,
    buildings: Vec<EntitySnapshot>,
}

#[derive(Serialize)]
struct ChunkSnapshot {
    chunk_q: i32,
    chunk_r: i32,
    entity_count: usize,
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
    cargo: Option<Cargo>,
    inventory: Vec<Ingredient>,
    progress: u32,
    progress_total: u32,
    status: &'static str,
    next_id: Option<u32>,
}

struct Core {
    definitions: DefinitionsInput,
    blueprint: BlueprintInput,
    entities: Vec<Entity>,
    graph: Vec<Option<usize>>,
    chunks: BTreeMap<(i32, i32), Vec<u32>>,
    tick: u64,
    delivered: u64,
    produced: BTreeMap<ItemId, u64>,
}

impl Core {
    fn from_json(definitions_json: &str, blueprint_json: &str) -> Result<Self, String> {
        let definitions: DefinitionsInput =
            serde_json::from_str(definitions_json).map_err(|error| error.to_string())?;
        let blueprint: BlueprintInput =
            serde_json::from_str(blueprint_json).map_err(|error| error.to_string())?;
        validate_definitions(&definitions)?;
        validate_blueprint(&definitions, &blueprint)?;
        let mut core = Self {
            definitions,
            blueprint,
            entities: Vec::new(),
            graph: Vec::new(),
            chunks: BTreeMap::new(),
            tick: 0,
            delivered: 0,
            produced: BTreeMap::new(),
        };
        core.reset_runtime();
        Ok(core)
    }

    fn reset_runtime(&mut self) {
        self.blueprint.buildings.sort_by_key(|placed| {
            (
                placed.q,
                placed.r,
                placed.definition_id,
                placed.orientation,
                placed.recipe_id,
            )
        });
        self.entities = self
            .blueprint
            .buildings
            .iter()
            .enumerate()
            .map(|(index, placed)| Entity {
                id: index as u32 + 1,
                placed: *placed,
                kind: self.building_definition(placed.definition_id).unwrap().kind,
                cargo: None,
                inventory: BTreeMap::new(),
                progress: 0,
            })
            .collect();
        self.tick = 0;
        self.delivered = 0;
        self.produced.clear();
        self.compile_graph();
    }

    fn building_definition(&self, id: DefinitionId) -> Option<&BuildingDefinition> {
        self.definitions
            .buildings
            .iter()
            .find(|definition| definition.id == id)
    }

    fn recipe(&self, id: RecipeId) -> Option<&RecipeDefinition> {
        self.definitions
            .recipes
            .iter()
            .find(|recipe| recipe.id == id)
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
        self.chunks.clear();
        let chunk_size = self.blueprint.chunk_size;
        for entity in &self.entities {
            let key = (
                floor_div(entity.placed.q, chunk_size),
                floor_div(entity.placed.r, chunk_size),
            );
            self.chunks.entry(key).or_default().push(entity.id);
        }
    }

    fn tick_many(&mut self, count: u32) {
        for _ in 0..count {
            self.advance_machines();
            self.transfer_cargo();
            self.tick += 1;
        }
    }

    fn advance_machines(&mut self) {
        for index in 0..self.entities.len() {
            let kind = self.entities[index].kind;
            match kind {
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
        let placed = self.entities[index].placed;
        let Some(resource) = self
            .blueprint
            .resources
            .iter()
            .find(|resource| resource.q == placed.q && resource.r == placed.r)
        else {
            return;
        };
        let cadence = self
            .building_definition(placed.definition_id)
            .and_then(|definition| definition.cadence)
            .unwrap_or(1);
        self.entities[index].progress += 1;
        if self.entities[index].progress >= cadence {
            let item_id = resource.item_id;
            self.entities[index].cargo = Some(Cargo {
                item_id,
                quantity: 1,
            });
            self.entities[index].progress = 0;
            *self.produced.entry(item_id).or_default() += 1;
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
        if self.entities[index].progress == recipe.duration {
            self.entities[index].cargo = Some(Cargo {
                item_id: recipe.output.item_id,
                quantity: recipe.output.quantity,
            });
            self.entities[index].progress = 0;
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
                let quantity = self.entities[index]
                    .inventory
                    .get_mut(&ingredient.item_id)
                    .unwrap();
                *quantity -= ingredient.quantity;
            }
            self.entities[index]
                .inventory
                .retain(|_, quantity| *quantity > 0);
            self.entities[index].progress = 1;
        }
    }

    fn transfer_cargo(&mut self) {
        let proposals: Vec<(usize, usize, Cargo, bool)> = self
            .entities
            .iter()
            .enumerate()
            .filter_map(|(source, entity)| {
                let target = self.graph[source]?;
                let (cargo, from_inventory) = if entity.kind == BuildingKind::Container {
                    let (&item_id, _) = entity
                        .inventory
                        .iter()
                        .find(|(_, quantity)| **quantity > 0)?;
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
                Some((source, target, cargo, from_inventory))
            })
            .collect();
        let mut claimed = BTreeSet::new();
        for (source, target, cargo, from_inventory) in proposals {
            if claimed.contains(&target) || !self.can_accept(target, cargo) {
                continue;
            }
            if from_inventory {
                let quantity = self.entities[source]
                    .inventory
                    .get_mut(&cargo.item_id)
                    .expect("proposal inventory exists");
                *quantity -= cargo.quantity;
                self.entities[source]
                    .inventory
                    .retain(|_, quantity| *quantity > 0);
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
            BuildingKind::Consumer => true,
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
            BuildingKind::Consumer => self.delivered += u64::from(cargo.quantity),
            BuildingKind::Extractor => unreachable!("extractors reject cargo"),
        }
    }

    fn place(
        &mut self,
        q: i32,
        r: i32,
        definition_id: DefinitionId,
        orientation: u8,
        recipe_id: Option<RecipeId>,
    ) -> Result<bool, String> {
        if self
            .entities
            .iter()
            .any(|entity| entity.placed.q == q && entity.placed.r == r)
        {
            return Ok(false);
        }
        let definition = self
            .building_definition(definition_id)
            .ok_or_else(|| format!("unknown building definition {definition_id}"))?;
        if orientation >= 6 {
            return Err("orientation must be in 0..6".into());
        }
        if definition.kind == BuildingKind::Composer {
            let id = recipe_id.ok_or_else(|| "composer requires a recipe".to_string())?;
            if self.recipe(id).is_none() {
                return Err(format!("unknown recipe {id}"));
            }
        }
        self.blueprint.buildings.push(PlacedBuilding {
            q,
            r,
            definition_id,
            orientation,
            recipe_id,
        });
        self.reset_runtime();
        Ok(true)
    }

    fn erase(&mut self, q: i32, r: i32) -> bool {
        let before = self.blueprint.buildings.len();
        self.blueprint
            .buildings
            .retain(|building| building.q != q || building.r != r);
        if self.blueprint.buildings.len() == before {
            return false;
        }
        self.reset_runtime();
        true
    }

    fn rotate(&mut self, q: i32, r: i32) -> bool {
        let Some(building) = self
            .blueprint
            .buildings
            .iter_mut()
            .find(|building| building.q == q && building.r == r)
        else {
            return false;
        };
        building.orientation = (building.orientation + 1) % 6;
        self.reset_runtime();
        true
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            tick: self.tick,
            checksum: self.checksum(),
            delivered: self.delivered,
            chunks: self
                .chunks
                .iter()
                .map(|(&(chunk_q, chunk_r), entities)| ChunkSnapshot {
                    chunk_q,
                    chunk_r,
                    entity_count: entities.len(),
                })
                .collect(),
            resources: self.blueprint.resources.clone(),
            buildings: self
                .entities
                .iter()
                .enumerate()
                .map(|(index, entity)| {
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
        }
    }

    fn status(&self, entity: &Entity) -> &'static str {
        match entity.kind {
            BuildingKind::Extractor if entity.cargo.is_some() => "output blocked",
            BuildingKind::Extractor if entity.progress > 0 => "extracting",
            BuildingKind::Composer if entity.cargo.is_some() => "output blocked",
            BuildingKind::Composer if entity.progress > 0 => "composing",
            BuildingKind::Composer => "waiting for inputs",
            BuildingKind::Container if inventory_total(&entity.inventory) > 0 => "buffered",
            BuildingKind::Belt if entity.cargo.is_some() => "carrying",
            BuildingKind::Consumer => "consuming",
            _ => "idle",
        }
    }

    fn checksum(&self) -> u32 {
        let mut hash = 0x811c9dc5u32;
        hash_u64(&mut hash, self.tick);
        hash_u64(&mut hash, self.delivered);
        for entity in &self.entities {
            hash_u32(&mut hash, entity.id);
            hash_i32(&mut hash, entity.placed.q);
            hash_i32(&mut hash, entity.placed.r);
            hash_u32(&mut hash, u32::from(entity.placed.definition_id));
            hash_u32(&mut hash, u32::from(entity.placed.orientation));
            hash_u32(&mut hash, entity.progress);
            if let Some(cargo) = entity.cargo {
                hash_u32(&mut hash, u32::from(cargo.item_id));
                hash_u32(&mut hash, cargo.quantity);
            } else {
                hash_u32(&mut hash, 0);
            }
            for (&item, &quantity) in &entity.inventory {
                hash_u32(&mut hash, u32::from(item));
                hash_u32(&mut hash, quantity);
            }
            hash_u32(&mut hash, u32::MAX);
        }
        hash
    }
}

#[wasm_bindgen]
pub struct Factory {
    core: Core,
}

#[wasm_bindgen]
impl Factory {
    #[wasm_bindgen(constructor)]
    pub fn new(definitions_json: &str, blueprint_json: &str) -> Result<Factory, JsValue> {
        Core::from_json(definitions_json, blueprint_json)
            .map(|core| Factory { core })
            .map_err(|error| JsValue::from_str(&error))
    }

    pub fn tick(&mut self, count: u32) {
        self.core.tick_many(count);
    }

    pub fn reset(&mut self) {
        self.core.reset_runtime();
    }

    pub fn checksum(&self) -> u32 {
        self.core.checksum()
    }

    pub fn tick_count(&self) -> u64 {
        self.core.tick
    }

    pub fn snapshot_json(&self) -> String {
        serde_json::to_string(&self.core.snapshot()).expect("snapshot is serializable")
    }

    pub fn place(
        &mut self,
        q: i32,
        r: i32,
        definition_id: u16,
        orientation: u8,
        recipe_id: Option<u16>,
    ) -> Result<bool, JsValue> {
        self.core
            .place(q, r, definition_id, orientation, recipe_id)
            .map_err(|error| JsValue::from_str(&error))
    }

    pub fn erase(&mut self, q: i32, r: i32) -> bool {
        self.core.erase(q, r)
    }

    pub fn rotate(&mut self, q: i32, r: i32) -> bool {
        self.core.rotate(q, r)
    }
}

fn validate_definitions(definitions: &DefinitionsInput) -> Result<(), String> {
    unique_positive_ids(definitions.items.iter().map(|item| item.id), "item")?;
    unique_positive_ids(definitions.recipes.iter().map(|recipe| recipe.id), "recipe")?;
    unique_positive_ids(
        definitions.buildings.iter().map(|building| building.id),
        "building",
    )?;
    let item_ids: BTreeSet<_> = definitions.items.iter().map(|item| item.id).collect();
    for item in &definitions.items {
        if item.key.trim().is_empty() || item.name.trim().is_empty() || item.color.trim().is_empty()
        {
            return Err(format!("item {} has incomplete display data", item.id));
        }
    }
    for recipe in &definitions.recipes {
        if recipe.key.trim().is_empty() || recipe.name.trim().is_empty() {
            return Err(format!("recipe {} has no key/name", recipe.id));
        }
        if recipe.duration == 0 || recipe.inputs.is_empty() || recipe.output.quantity == 0 {
            return Err(format!(
                "recipe {} has invalid quantities or duration",
                recipe.id
            ));
        }
        for ingredient in recipe.inputs.iter().chain(std::iter::once(&recipe.output)) {
            if ingredient.quantity == 0 || !item_ids.contains(&ingredient.item_id) {
                return Err(format!("recipe {} references an invalid item", recipe.id));
            }
        }
    }
    for building in &definitions.buildings {
        if building.key.trim().is_empty() || building.name.trim().is_empty() {
            return Err(format!("building {} has no key/name", building.id));
        }
        if building.kind == BuildingKind::Extractor && building.cadence.unwrap_or(0) == 0 {
            return Err(format!("extractor {} requires a cadence", building.id));
        }
    }
    Ok(())
}

fn validate_blueprint(
    definitions: &DefinitionsInput,
    blueprint: &BlueprintInput,
) -> Result<(), String> {
    if blueprint.chunk_size <= 0 {
        return Err("chunk_size must be positive".into());
    }
    let building_ids: BTreeSet<_> = definitions.buildings.iter().map(|item| item.id).collect();
    let recipe_ids: BTreeSet<_> = definitions.recipes.iter().map(|item| item.id).collect();
    let item_ids: BTreeSet<_> = definitions.items.iter().map(|item| item.id).collect();
    let mut occupied = BTreeSet::new();
    for building in &blueprint.buildings {
        if !occupied.insert((building.q, building.r)) {
            return Err(format!(
                "duplicate building at {},{}",
                building.q, building.r
            ));
        }
        if !building_ids.contains(&building.definition_id) || building.orientation >= 6 {
            return Err("building references an invalid definition or orientation".into());
        }
        if let Some(recipe_id) = building.recipe_id {
            if !recipe_ids.contains(&recipe_id) {
                return Err(format!("unknown recipe {recipe_id}"));
            }
        }
    }
    if blueprint
        .resources
        .iter()
        .any(|node| !item_ids.contains(&node.item_id))
    {
        return Err("resource references an invalid item".into());
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

fn floor_div(value: i32, divisor: i32) -> i32 {
    value.div_euclid(divisor)
}

fn inventory_total(inventory: &BTreeMap<ItemId, u32>) -> u32 {
    inventory.values().sum()
}

fn hash_u32(hash: &mut u32, value: u32) {
    for byte in value.to_le_bytes() {
        *hash ^= u32::from(byte);
        *hash = hash.wrapping_mul(0x01000193);
    }
}

fn hash_i32(hash: &mut u32, value: i32) {
    hash_u32(hash, value as u32);
}

fn hash_u64(hash: &mut u32, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u32::from(byte);
        *hash = hash.wrapping_mul(0x01000193);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFINITIONS: &str = include_str!("../../src/data/definitions.json");
    const BLUEPRINT: &str = include_str!("../../src/data/blueprint.json");

    fn demo() -> Core {
        Core::from_json(DEFINITIONS, BLUEPRINT).unwrap()
    }

    fn building(q: i32, r: i32, definition_id: u16, orientation: u8) -> PlacedBuilding {
        PlacedBuilding {
            q,
            r,
            definition_id,
            orientation,
            recipe_id: None,
        }
    }

    fn blueprint(buildings: Vec<PlacedBuilding>, resources: Vec<ResourceNode>) -> String {
        serde_json::to_string(&BlueprintInputForTest {
            chunk_size: 16,
            resources,
            buildings,
        })
        .unwrap()
    }

    #[derive(Serialize)]
    struct BlueprintInputForTest {
        chunk_size: i32,
        resources: Vec<ResourceNode>,
        buildings: Vec<PlacedBuilding>,
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
        assert_eq!(fixture[0]["name"], "east");
        assert_eq!(fixture[5]["name"], "northeast");
    }

    #[test]
    fn turning_path_compiles_in_geometric_order() {
        let core = demo();
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
    }

    #[test]
    fn unblocked_transport_conserves_every_extracted_ore() {
        let map = blueprint(
            vec![
                building(0, 0, 1, 0),
                building(1, 0, 2, 0),
                building(2, 0, 4, 0),
            ],
            vec![ResourceNode {
                q: 0,
                r: 0,
                item_id: 1,
            }],
        );
        let mut core = Core::from_json(DEFINITIONS, &map).unwrap();
        core.tick_many(24);
        let in_system: u64 = core
            .entities
            .iter()
            .map(|entity| {
                u64::from(entity.cargo.map(|cargo| cargo.quantity).unwrap_or(0))
                    + u64::from(entity.inventory.get(&1).copied().unwrap_or(0))
            })
            .sum();
        assert_eq!(core.produced.get(&1).copied().unwrap_or(0), in_system);
    }

    #[test]
    fn backpressure_preserves_cargo_and_stops_the_extractor_without_duplication() {
        let mut definitions: serde_json::Value = serde_json::from_str(DEFINITIONS).unwrap();
        definitions["buildings"][3]["capacity"] = 1.into();
        let map = blueprint(
            vec![
                building(0, 0, 1, 0),
                building(1, 0, 2, 0),
                building(2, 0, 4, 0),
            ],
            vec![ResourceNode {
                q: 0,
                r: 0,
                item_id: 1,
            }],
        );
        let mut core = Core::from_json(&definitions.to_string(), &map).unwrap();
        core.tick_many(100);
        let produced = core.produced.get(&1).copied().unwrap();
        let stored: u64 = core
            .entities
            .iter()
            .map(|entity| {
                u64::from(entity.cargo.map(|cargo| cargo.quantity).unwrap_or(0))
                    + u64::from(entity.inventory.get(&1).copied().unwrap_or(0))
            })
            .sum();
        assert_eq!(produced, stored);
        assert_eq!(stored, 3);
    }

    #[test]
    fn composer_consumes_exact_quantities_and_waits_the_recipe_duration() {
        let mut placed = building(0, 0, 3, 0);
        placed.recipe_id = Some(1);
        let map = blueprint(vec![placed], vec![]);
        let mut core = Core::from_json(DEFINITIONS, &map).unwrap();
        core.entities[0].inventory.insert(1, 2);
        core.tick_many(5);
        assert_eq!(core.entities[0].inventory.get(&1), None);
        assert_eq!(core.entities[0].cargo, None);
        core.tick_many(1);
        assert_eq!(
            core.entities[0].cargo,
            Some(Cargo {
                item_id: 2,
                quantity: 1
            })
        );
    }

    #[test]
    fn container_holds_real_quantities_and_releases_lowest_item_id_first() {
        let map = blueprint(vec![building(0, 0, 4, 0), building(1, 0, 2, 0)], vec![]);
        let mut core = Core::from_json(DEFINITIONS, &map).unwrap();
        core.entities[0].inventory.insert(2, 2);
        core.entities[0].inventory.insert(1, 1);
        core.tick_many(1);
        assert_eq!(
            core.entities[1].cargo,
            Some(Cargo {
                item_id: 1,
                quantity: 1
            })
        );
        assert_eq!(core.entities[0].inventory.get(&2), Some(&2));
    }

    #[test]
    fn consumer_delivery_total_is_exact() {
        let map = blueprint(vec![building(0, 0, 4, 0), building(1, 0, 5, 0)], vec![]);
        let mut core = Core::from_json(DEFINITIONS, &map).unwrap();
        core.entities[0].inventory.insert(2, 7);
        core.tick_many(10);
        assert_eq!(core.delivered, 7);
        assert!(core.entities[0].inventory.is_empty());
    }

    #[test]
    fn reset_and_replay_have_the_same_checksum() {
        let mut core = demo();
        core.tick_many(150);
        let checksum = core.checksum();
        let delivered = core.delivered;
        core.reset_runtime();
        core.tick_many(150);
        assert_eq!(core.checksum(), checksum);
        assert_eq!(core.delivered, delivered);
    }

    #[test]
    fn blueprint_insertion_order_cannot_change_arbitration_or_checksum() {
        let normal: BlueprintInput = serde_json::from_str(BLUEPRINT).unwrap();
        let mut reversed = normal.buildings.clone();
        reversed.reverse();
        let reverse_json = blueprint(reversed, normal.resources);
        let mut a = demo();
        let mut b = Core::from_json(DEFINITIONS, &reverse_json).unwrap();
        a.tick_many(300);
        b.tick_many(300);
        assert_eq!(a.checksum(), b.checksum());
        assert_eq!(a.delivered, b.delivered);
    }

    #[test]
    fn negative_coordinates_use_euclidean_chunk_division() {
        assert_eq!(floor_div(-1, 16), -1);
        assert_eq!(floor_div(-16, 16), -1);
        assert_eq!(floor_div(-17, 16), -2);
    }
}
