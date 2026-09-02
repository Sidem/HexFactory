//! snapshots — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Core {
    pub(crate) fn stock_snapshot(&self, index: usize, stock: StockKind) -> Vec<Ingredient> {
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
    pub(crate) fn entity_snapshot(&mut self, index: usize) -> EntitySnapshot {
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
    pub(crate) fn entity_snapshots(&mut self) -> Vec<EntitySnapshot> {
        let mut indices: Vec<usize> = (0..self.entities.len()).collect();
        indices.sort_by_key(|&index| self.entities[index].id);
        indices
            .into_iter()
            .map(|index| self.entity_snapshot(index))
            .collect()
    }

    /// The generated chunk set with its per-chunk entity counts. Counting in one pass over the
    /// blueprint keeps this linear; asking each chunk to filter the whole blueprint did not.
    pub(crate) fn chunk_snapshots(&self) -> Vec<ChunkSnapshot> {
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
    pub(crate) fn tile_snapshot(&self, q: i32, r: i32) -> TileSnapshot {
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
    pub(crate) fn chunk_terrain_snapshots(&self, chunk_q: i32, chunk_r: i32) -> Vec<TileSnapshot> {
        hexes_in_chunk(chunk_q, chunk_r, self.scenario.chunk_size)
            .map(|(q, r)| self.tile_snapshot(q, r))
            .collect()
    }

    pub(crate) fn terrain_snapshots(&self) -> Vec<TileSnapshot> {
        let mut tiles = Vec::new();
        for &(chunk_q, chunk_r) in &self.generated_chunks {
            tiles.extend(self.chunk_terrain_snapshots(chunk_q, chunk_r));
        }
        tiles
    }

    /// One field cell's snapshot, looked up by tile key. Used by the incremental path, which knows
    /// which cells moved but not where they sit in the overlay.
    pub(crate) fn resource_snapshot(&self, key: (i32, i32)) -> Option<ResourceSnapshot> {
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
    pub(crate) fn resource_snapshots(&self) -> Vec<ResourceSnapshot> {
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

    pub(crate) fn delivered_by_item_snapshot(&self) -> Vec<Ingredient64> {
        self.delivered_by_item
            .iter()
            .map(|(&item_id, &quantity)| Ingredient64 { item_id, quantity })
            .collect()
    }

    pub(crate) fn contract_snapshot(&self) -> ContractSnapshot {
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
    pub(crate) fn request_snapshots(&self) -> Vec<RequestSnapshot> {
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
    pub(crate) fn snapshot(&mut self) -> Snapshot {
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

    pub(crate) fn checksum(&self) -> u32 {
        self.checksum_for_world(WORLD_GENERATOR_VERSION)
    }
}
