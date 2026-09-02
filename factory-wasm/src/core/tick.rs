//! tick — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Core {
    pub(crate) fn graph_links_by_id(&self) -> BTreeMap<u32, LinkIds> {
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

    pub(crate) fn recompile_graph_components(
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

    pub(crate) fn tick_many(&mut self, count: u32) {
        if count > 0 {
            self.events.clear();
        }
        self.advance_ticks(count);
    }

    pub(crate) fn advance_ticks(&mut self, count: u32) {
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

    pub(crate) fn advance_ground_items(&mut self) {
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
    pub(crate) fn advance_player_steps(&mut self, count: u32) {
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

    pub(crate) fn collect_ground_items(&mut self) {
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

    pub(crate) fn advance_machines(&mut self) {
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

    pub(crate) fn advance_extractor(&mut self, index: usize) {
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
    pub(crate) fn advance_pump(&mut self, index: usize) {
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

    pub(crate) fn draw_pump_source(&mut self, source: WaterSourceSnapshot) -> bool {
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

    pub(crate) fn fuel_value(&self, item_id: ItemId) -> u32 {
        self.item_definition(item_id)
            .and_then(|item| item.fuel_value)
            .unwrap_or(0)
    }

    /// The lowest-id item a machine holding this inventory may burn. Never the quantity a recipe
    /// input reserves: steel names coal in its `inputs`, and a smelter that burned the very coal it
    /// was waiting on would starve itself on its own recipe. One predicate serves both the tick
    /// that burns and the status line that explains why nothing burned.
    pub(crate) fn burnable_item(
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
    pub(crate) fn charge_fuel(
        &mut self,
        index: usize,
        required: u32,
        inputs: &[Ingredient],
    ) -> bool {
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

    pub(crate) fn advance_composer(&mut self, index: usize) {
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
    pub(crate) fn cargo_on_offer(&self, source: usize) -> Option<(Cargo, StockKind)> {
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
}
