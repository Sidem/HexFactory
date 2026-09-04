//! commands — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Core {
    pub(crate) fn apply_commands(&mut self, commands_json: &str) -> Result<(), String> {
        let commands: Vec<InputCommand> =
            serde_json::from_str(commands_json).map_err(|error| error.to_string())?;
        self.apply_command_batch(commands, true)
    }

    /// One frame of native work: the host's bounded command batch, the player steps that frame's
    /// real time is worth, and the simulation ticks its speed setting is worth. The two counts are
    /// separate because the player and the factory now run on separate clocks.
    pub(crate) fn advance(
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

    pub(crate) fn apply_command_batch(
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
                InputCommand::GroundEdit { edit } => self.begin_groundwork(edit),
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
                InputCommand::CancelCraft { q, r } => self.cancel_craft(q, r),
                InputCommand::SetEnabled { q, r, enabled } => self.set_enabled(q, r, enabled),
                InputCommand::Undo => self.undo(),
                InputCommand::PurchaseSkill { skill_id } => self.purchase_skill(skill_id),
                InputCommand::Research { technology_id } => self.research(technology_id),
                InputCommand::SkipRequest { slot } => self.skip_request(slot),
                InputCommand::PostRequest { request_id } => self.post_request(request_id),
                // Kept in the wire vocabulary for old command recordings, but game mode is chosen
                // only while creating a world and is immutable once that world is running.
                InputCommand::SetCreative { enabled } => {
                    if enabled == self.creative {
                        Ok(())
                    } else {
                        Err("Creative mode is chosen when starting a new game".into())
                    }
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
    pub(crate) fn fuel_ready(&self, entity: &Entity) -> bool {
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
    pub(crate) fn status_of(
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

    pub(crate) fn generator_status(&self, index: usize) -> EntityStatus {
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
}
