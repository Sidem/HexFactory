//! configuration — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Core {
    /// Give the machine at this hex a different recipe. Bounded and range-checked like every other
    /// edit, and it enforces the same category rule placement does, so a kiln can no more be
    /// reassigned to a circuit than it could be built with one.
    ///
    /// A machine mid-craft is refused rather than reassigned: its reserved inputs belong to the job
    /// it is running, and deciding what happens to a part-finished one is a question worth its own
    /// pass — the same reason `withdraw` reaches into a machine's free stock and never into
    /// `reserved_inputs`.
    pub(crate) fn set_recipe(&mut self, q: i32, r: i32, recipe_id: RecipeId) -> Result<(), String> {
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

    pub(crate) fn output_items(&self, index: usize) -> Vec<ItemId> {
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
    pub(crate) fn default_output_route(&self, index: usize) -> OutputRoute {
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

    pub(crate) fn set_output_route(
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
    pub(crate) fn set_enabled(&mut self, q: i32, r: i32, enabled: bool) -> Result<(), String> {
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

    pub(crate) fn is_manual_workshop(&self, index: usize) -> bool {
        self.building_definition(self.entities[index].placed.definition_id)
            .is_some_and(|definition| definition.manual_work)
    }

    pub(crate) fn can_work_here(&self, index: usize) -> bool {
        self.player.move_x == 0
            && self.player.move_y == 0
            && self.player.walk_goal.is_none()
            && self.player.action_cooldown == 0
            && self.within_hex_range_of_entity(index, 1)
    }

    /// The kinds that have work a switch can suspend: anything that extracts, crafts, pumps, or
    /// burns. The same list every arm of the tick consults through `entity_running`.
    pub(crate) fn can_be_switched(kind: BuildingKind) -> bool {
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
    pub(crate) fn entity_running(&self, index: usize) -> bool {
        !self.entities[index].disabled
    }

    pub(crate) fn rotate(&mut self, q: i32, r: i32, reverse: bool) -> Result<(), String> {
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
}
