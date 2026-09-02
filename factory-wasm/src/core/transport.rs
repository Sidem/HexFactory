//! transport — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Core {
    /// Move one cargo out of `source` and into `target`, with no question left to ask.
    pub(crate) fn hand_over(
        &mut self,
        source: usize,
        target: usize,
        cargo: Cargo,
        stock: StockKind,
    ) {
        match stock {
            StockKind::Auto => self.entities[source].cargo = None,
            _ => self.subtract_stock(source, stock, cargo.item_id, cargo.quantity),
        }
        let (source_id, target_id) = (self.entities[source].id, self.entities[target].id);
        self.dirty.entities.push(source_id);
        self.dirty.entities.push(target_id);
        self.accept(target, cargo);
    }

    /// Whether this definition serves its feeders in rotation instead of in entity id order.
    pub(crate) fn is_merger(&self, index: usize) -> bool {
        self.building_definition(self.entities[index].placed.definition_id)
            .is_some_and(|definition| definition.merges)
    }

    /// One tick of deliveries along the compiled graph.
    ///
    /// Two passes, because two different junctions want two different answers to "who goes first".
    ///
    /// **Mergers first.** A belt holds one cargo, so several lanes pointed into one hex are
    /// competing every tick, and the historical rule — sort by entity id, first proposal claims the
    /// target — hands the win to the same lane forever. That is not a tie-break, it is a starved
    /// lane: whichever feeder happened to be built first drinks the junction dry. A merger walks
    /// its feeders from the one *after* the one it served last, so a junction of two full lanes
    /// alternates and a junction of three cycles. Ordinary belts keep the id order exactly, so
    /// nothing that worked before behaves differently.
    ///
    /// **Everything else second**, in ascending entity id, unchanged. A splitter differs only in
    /// having more than one compiled output to offer its cargo to, and it offers them starting from
    /// its own cursor so consecutive items go to different branches.
    ///
    /// Either pass, only a belt's *exit slot* is on offer: everything else it is carrying is still
    /// somewhere along its 5.37 m of conveyor, and gets there by [`Core::advance_belt_lanes`],
    /// which runs first so an item that finished crossing this tick can leave on it.
    pub(crate) fn transfer_cargo(&mut self) {
        self.advance_belt_lanes();
        self.runtime.clear_transfer_scratch();
        if self.runtime.merger_targets.is_empty() {
            self.transfer_along_links();
            return;
        }
        for target_offset in 0..self.runtime.merger_targets.len() {
            let target = self.runtime.merger_targets[target_offset];
            let cursor = self.entities[target].merge_cursor;
            // Start after the id served last and wrap. An id that no longer exists simply means
            // every feeder sorts after it, which is the same as starting from the beginning.
            let start = self.runtime.feeders[target]
                .iter()
                .position(|&source| self.entities[source].id > cursor)
                .unwrap_or(0);
            let feeder_count = self.runtime.feeders[target].len();
            for offset in 0..feeder_count {
                let source = self.runtime.feeders[target][(start + offset) % feeder_count];
                if self.runtime.delivered[source] {
                    continue;
                }
                let Some((cargo, stock)) = self.cargo_on_offer(source) else {
                    continue;
                };
                if !self.can_accept(target, cargo) {
                    // A full merger refuses every feeder, so there is nothing left to try and no
                    // reason to move the cursor: the rotation resumes where it stopped.
                    break;
                }
                self.hand_over(source, target, cargo, stock);
                self.entities[target].merge_cursor = self.entities[source].id;
                self.runtime.claimed[target] = true;
                self.runtime.delivered[source] = true;
                break;
            }
        }

        // Pass two: everything else, in the entity id order arbitration has always used.
        self.transfer_along_links();
    }

    /// Move every belt's queue along: an item that has finished crossing its hex leaves the lane and
    /// waits in the exit slot for someone to take it.
    ///
    /// This is the whole of a belt's motion, and it costs one comparison per belt per tick because
    /// nothing is counted down — a lane item carries the tick it stepped on, and crossing is over
    /// when [`BELT_TRANSIT_TICKS`] have passed since. A belt nobody is feeding does no work and
    /// reports itself unchanged, so an idle line neither ticks nor re-sends.
    ///
    /// Only one item leaves the lane per tick, which is the one the exit slot can hold. A blocked
    /// belt therefore backs up: the items behind finish crossing, find the slot taken, and wait
    /// where they are, which is what the player is looking at when a line jams.
    pub(crate) fn advance_belt_lanes(&mut self) {
        for offset in 0..self.runtime.belt_order.len() {
            let index = self.runtime.belt_order[offset];
            let entity = &self.entities[index];
            if entity.cargo.is_some() {
                continue;
            }
            let Some(head) = entity.lane.first() else {
                continue;
            };
            if self.tick.saturating_sub(head.entered) < BELT_TRANSIT_TICKS {
                continue;
            }
            let arrived = self.entities[index].lane.remove(0);
            self.entities[index].cargo = Some(arrived.cargo);
            let id = self.entities[index].id;
            self.dirty.entities.push(id);
        }
    }

    /// Everything a belt is holding, exit slot first, for the paths that have to account for all of
    /// it at once — erasing the belt, and hashing it.
    pub(crate) fn belt_contents(entity: &Entity) -> impl Iterator<Item = Cargo> + '_ {
        entity
            .cargo
            .into_iter()
            .chain(entity.lane.iter().map(|item| item.cargo))
    }

    /// Every source that has not already delivered offers its cargo along its compiled edges, in
    /// ascending entity id — the arbitration order the game has always had.
    ///
    /// A splitter is the only thing here with more than one edge, and it starts from its own cursor
    /// so consecutive items leave by different branches.
    pub(crate) fn transfer_along_links(&mut self) {
        for source_offset in 0..self.runtime.transport_order.len() {
            let source = self.runtime.transport_order[source_offset];
            if self.runtime.delivered[source] || self.graph[source].is_empty() {
                continue;
            }
            let Some((cargo, stock)) = self.cargo_on_offer(source) else {
                continue;
            };
            let links = self.graph[source];
            let outputs: Vec<usize> = links.iter_for(cargo.item_id).collect();
            if outputs.is_empty() {
                continue;
            }
            let cursor = usize::from(self.entities[source].route_cursor);
            for offset in 0..outputs.len() {
                let slot = (cursor + offset) % outputs.len();
                let target = outputs[slot];
                // Merging targets were settled by the rotation pass, including the case where every
                // feeder was refused. Re-offering here would put the id order back in front of it.
                if self.runtime.claimed[target]
                    || self.runtime.mergers[target]
                    || !self.can_accept(target, cargo)
                {
                    continue;
                }
                self.hand_over(source, target, cargo, stock);
                // The next item starts at the branch after the one this item took, which is the
                // whole of the round robin. Left where it is on a blocked branch, so a splitter
                // with one jammed output keeps feeding the other rather than stalling every other
                // item against the jam.
                self.entities[source].route_cursor = ((slot + 1) % outputs.len()) as u8;
                self.runtime.claimed[target] = true;
                self.runtime.delivered[source] = true;
                break;
            }
        }
    }

    /// Whether this building has any use for that item, ignoring whether it has room for one.
    ///
    /// Split from `can_accept` so the two questions a delivery asks — *would you want this* and
    /// *have you space* — can be asked apart. A belt only ever needs both at once, but a hand
    /// transfer has to tell "a burner does not eat iron" from "the burner is full", and answering
    /// with one bit made those the same refusal.
    pub(crate) fn accepts_item(&self, target: usize, item_id: ItemId) -> bool {
        let entity = &self.entities[target];
        match entity.kind {
            BuildingKind::Belt => self.transport_accepts(target, item_id),
            BuildingKind::Container => self
                .building_definition(entity.placed.definition_id)
                .and_then(|definition| definition.accepted_item_ids.as_ref())
                .is_none_or(|ids| ids.contains(&item_id)),
            BuildingKind::Consumer => true,
            BuildingKind::Composer => {
                let Some(recipe) = entity.placed.recipe_id.and_then(|id| self.recipe(id)) else {
                    return false;
                };
                // A machine takes its recipe's inputs, and — when the recipe needs heat — anything
                // that burns. Fuel is not in `inputs`, so this is where a belt of coal is allowed
                // into a smelter without every smelting recipe having to name a fuel.
                let burns = recipe.fuel > 0 && self.fuel_value(item_id) > 0;
                burns || recipe.inputs.iter().any(|input| input.item_id == item_id)
            }
            // The hub takes what it asked for and nothing else, by belt exactly as by hand. A line
            // pointed at it backs up once the board and the contract are satisfied, which is a
            // legible answer — the belt shows it — where silently voiding the cargo was not.
            BuildingKind::Hub => self.hub_demand(item_id) > 0,
            BuildingKind::Extractor
            | BuildingKind::Pump
            | BuildingKind::Pole
            | BuildingKind::Bridge => false,
            // Fuel goes only where fuel is burned. A wind turbine keeps an `inventory` like every
            // other generator and has no firebox to spend it in, so coal delivered to one used to
            // sit there forever — a belt could quietly bury a stack in a machine that would never
            // touch it. A boiler additionally drinks, and that is the only thing it does not burn.
            BuildingKind::Generator | BuildingKind::Boiler => {
                let burns = self.fuel_value(item_id) > 0
                    && matches!(
                        self.building_definition(entity.placed.definition_id)
                            .and_then(|definition| definition.power_source),
                        Some(PowerSource::Burner) | None
                    );
                burns || (entity.kind == BuildingKind::Boiler && item_id == WATER_ITEM)
            }
        }
    }

    /// Resolve the one compartment that may receive this item. Inputs outrank fuel deliberately:
    /// coal is an ingredient in steel as well as something that burns, and feeding that recipe must
    /// not silently divert its bill into the firebox.
    pub(crate) fn stock_kind_for_item(&self, target: usize, item_id: ItemId) -> Option<StockKind> {
        let entity = &self.entities[target];
        match entity.kind {
            BuildingKind::Container => Some(StockKind::Inventory),
            BuildingKind::Composer => {
                let recipe = entity.placed.recipe_id.and_then(|id| self.recipe(id))?;
                if recipe.inputs.iter().any(|input| input.item_id == item_id) {
                    Some(StockKind::Input)
                } else if recipe.fuel > 0 && self.fuel_value(item_id) > 0 {
                    Some(StockKind::Fuel)
                } else {
                    None
                }
            }
            BuildingKind::Boiler if item_id == WATER_ITEM => Some(StockKind::Input),
            BuildingKind::Generator | BuildingKind::Boiler
                if self.fuel_value(item_id) > 0
                    && matches!(
                        self.building_definition(entity.placed.definition_id)
                            .and_then(|definition| definition.power_source),
                        Some(PowerSource::Burner) | None
                    ) =>
            {
                Some(StockKind::Fuel)
            }
            _ => None,
        }
    }

    pub(crate) fn stock_accepts_item(
        &self,
        target: usize,
        stock: StockKind,
        item_id: ItemId,
    ) -> bool {
        match stock {
            StockKind::Auto => self.stock_kind_for_item(target, item_id).is_some(),
            StockKind::Output => false,
            named => self.stock_kind_for_item(target, item_id) == Some(named),
        }
    }

    /// Quantity visible in one compartment. Version-15 machine stock still lives in `inventory`;
    /// classifying it here lets an old kiln immediately present clay as input and coal as fuel while
    /// preserving the old save checksum until either stack is next moved.
    pub(crate) fn stock_quantity(&self, target: usize, stock: StockKind, item_id: ItemId) -> u32 {
        let entity = &self.entities[target];
        let explicit = match stock {
            StockKind::Inventory => entity.inventory.get(&item_id),
            StockKind::Input => entity.input_inventory.get(&item_id),
            StockKind::Fuel => entity.fuel_inventory.get(&item_id),
            StockKind::Output => entity.output_inventory.get(&item_id),
            StockKind::Auto => None,
        }
        .copied()
        .unwrap_or(0);
        let legacy = if stock != StockKind::Inventory
            && self.stock_kind_for_item(target, item_id) == Some(stock)
        {
            entity.inventory.get(&item_id).copied().unwrap_or(0)
        } else {
            0
        };
        let cargo = if stock == StockKind::Output {
            entity
                .cargo
                .filter(|cargo| cargo.item_id == item_id)
                .map_or(0, |cargo| cargo.quantity)
        } else {
            0
        };
        explicit.saturating_add(legacy).saturating_add(cargo)
    }

    pub(crate) fn stock_total(&self, target: usize, stock: StockKind) -> u32 {
        let entity = &self.entities[target];
        let explicit = match stock {
            StockKind::Inventory => inventory_total(&entity.inventory),
            StockKind::Input => inventory_total(&entity.input_inventory),
            StockKind::Fuel => inventory_total(&entity.fuel_inventory),
            StockKind::Output => inventory_total(&entity.output_inventory),
            StockKind::Auto => 0,
        };
        let legacy = if matches!(stock, StockKind::Input | StockKind::Fuel) {
            entity
                .inventory
                .iter()
                .filter(|&(&item_id, _)| self.stock_kind_for_item(target, item_id) == Some(stock))
                .map(|(_, &quantity)| quantity)
                .sum()
        } else {
            0
        };
        let cargo = if stock == StockKind::Output {
            entity.cargo.map_or(0, |cargo| cargo.quantity)
        } else {
            0
        };
        explicit.saturating_add(legacy).saturating_add(cargo)
    }

    pub(crate) fn add_stock(&mut self, target: usize, stock: StockKind, cargo: Cargo) {
        let map = match stock {
            StockKind::Inventory => &mut self.entities[target].inventory,
            StockKind::Input => &mut self.entities[target].input_inventory,
            StockKind::Fuel => &mut self.entities[target].fuel_inventory,
            StockKind::Output => &mut self.entities[target].output_inventory,
            StockKind::Auto => return,
        };
        *map.entry(cargo.item_id).or_default() += cargo.quantity;
    }

    pub(crate) fn subtract_stock(
        &mut self,
        target: usize,
        stock: StockKind,
        item_id: ItemId,
        quantity: u32,
    ) {
        let explicit = match stock {
            StockKind::Inventory => self.entities[target]
                .inventory
                .get(&item_id)
                .copied()
                .unwrap_or(0),
            StockKind::Input => self.entities[target]
                .input_inventory
                .get(&item_id)
                .copied()
                .unwrap_or(0),
            StockKind::Fuel => self.entities[target]
                .fuel_inventory
                .get(&item_id)
                .copied()
                .unwrap_or(0),
            StockKind::Output => self.entities[target]
                .output_inventory
                .get(&item_id)
                .copied()
                .unwrap_or(0),
            StockKind::Auto => 0,
        };
        let from_explicit = explicit.min(quantity);
        if from_explicit > 0 {
            match stock {
                StockKind::Inventory => {
                    subtract_item(&mut self.entities[target].inventory, item_id, from_explicit)
                }
                StockKind::Input => subtract_item(
                    &mut self.entities[target].input_inventory,
                    item_id,
                    from_explicit,
                ),
                StockKind::Fuel => subtract_item(
                    &mut self.entities[target].fuel_inventory,
                    item_id,
                    from_explicit,
                ),
                StockKind::Output => subtract_item(
                    &mut self.entities[target].output_inventory,
                    item_id,
                    from_explicit,
                ),
                StockKind::Auto => {}
            }
        }
        let remainder = quantity - from_explicit;
        if remainder == 0 {
            return;
        }
        if stock == StockKind::Output
            && self.entities[target]
                .cargo
                .is_some_and(|cargo| cargo.item_id == item_id && cargo.quantity <= remainder)
        {
            self.entities[target].cargo = None;
        } else if stock != StockKind::Inventory {
            subtract_item(&mut self.entities[target].inventory, item_id, remainder);
        }
    }

    /// How much more this compartment will hold of *this item*.
    ///
    /// Ingredient and fuel compartments are bounded per item: every ingredient a recipe names gets
    /// the definition's whole capacity to itself, and so does every fuel. They used to share one
    /// undifferentiated total, which made a composer refuse its second ingredient the moment the
    /// first one filled the buffer — twelve iron plates in a twelve-capacity machine left the empty
    /// gear slot unfillable, and the only way out was to take plates back into the pack. A recipe
    /// with four ingredients could not hold a working set of any of them.
    ///
    /// The output compartment and a container's store stay one shared pool, and deliberately so: a
    /// recipe's whole batch has to fit in the former before any input is reserved, and the latter is
    /// the storage decision the player is actually making when they choose a container's tier.
    pub(crate) fn room_for_stock(&self, target: usize, stock: StockKind, item_id: ItemId) -> u32 {
        let entity = &self.entities[target];
        let capacity = self
            .building_definition(entity.placed.definition_id)
            .and_then(|definition| definition.capacity)
            .unwrap_or(u32::MAX);
        let held = match stock {
            StockKind::Input | StockKind::Fuel => self.stock_quantity(target, stock, item_id),
            StockKind::Inventory | StockKind::Output | StockKind::Auto => {
                self.stock_total(target, stock)
            }
        };
        capacity.saturating_sub(held)
    }

    /// Whether that capacity would hold everything this entity is holding, under exactly the rule
    /// `room_for_stock` applies: per item where a compartment is bounded per item, one pool where it
    /// is shared. Asked when a tier changes, so the two answers cannot drift apart.
    pub(crate) fn stock_fits_capacity(&self, index: usize, capacity: u32) -> bool {
        // Only a container is capped on `inventory` as a pool. A machine's legacy version-15 stock
        // still lives there, but it is classified into input and fuel below and capped per item.
        let shared = if self.entities[index].kind == BuildingKind::Container {
            self.stock_total(index, StockKind::Inventory)
        } else {
            0
        };
        shared.max(self.stock_total(index, StockKind::Output)) <= capacity
            && [StockKind::Input, StockKind::Fuel]
                .into_iter()
                .flat_map(|stock| self.stock_snapshot(index, stock))
                .all(|held| held.quantity <= capacity)
    }

    pub(crate) fn can_accept(&self, target: usize, cargo: Cargo) -> bool {
        let entity = &self.entities[target];
        if !self.accepts_item(target, cargo.item_id) {
            return false;
        }
        match entity.kind {
            // Two rules, and both of them are the conveyor rather than the bookkeeping. A hex of
            // belt holds [`BELT_LANE_SLOTS`] items because that is how many are on 5.37 m of moving
            // conveyor at once, and it will not take another until [`BELT_SLOT_TICKS`] have passed
            // since the last one stepped on, because the space behind that item has not cleared
            // yet. The second rule is what sets belt throughput; the first is what lets a blocked
            // line back up instead of stopping dead at its head, and it is derived so that it never
            // bites first — see `scale::a_belt_has_room_for_everything_in_flight_at_cadence`.
            BuildingKind::Belt => {
                entity.lane.len() + usize::from(entity.cargo.is_some()) < BELT_LANE_SLOTS
                    && entity.lane.last().is_none_or(|item| {
                        self.tick.saturating_sub(item.entered) >= BELT_SLOT_TICKS
                    })
            }
            BuildingKind::Consumer => true,
            BuildingKind::Hub => self.hub_demand(cargo.item_id) >= u64::from(cargo.quantity),
            _ => self
                .stock_kind_for_item(target, cargo.item_id)
                .is_some_and(|stock| {
                    self.room_for_stock(target, stock, cargo.item_id) >= cargo.quantity
                }),
        }
    }

    pub(crate) fn accept(&mut self, target: usize, cargo: Cargo) {
        match self.entities[target].kind {
            // Onto the far end of the lane, not into the hand-off slot: the item has just stepped
            // onto the belt and has the whole hex still to cross.
            BuildingKind::Belt => {
                let entered = self.tick;
                self.entities[target].lane.push(LaneItem { cargo, entered });
            }
            BuildingKind::Composer
            | BuildingKind::Container
            | BuildingKind::Generator
            | BuildingKind::Boiler => {
                if let Some(stock) = self.stock_kind_for_item(target, cargo.item_id) {
                    self.add_stock(target, stock, cargo);
                }
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

    pub(crate) fn deliver_to_hub(&mut self, item_id: ItemId, quantity: u32) {
        self.delivered += u64::from(quantity);
        *self.delivered_by_item.entry(item_id).or_default() += u64::from(quantity);
        self.credit_requests(item_id, quantity);
        *self.contract_contributed.entry(item_id).or_default() += u64::from(quantity);
        self.advance_contract();
    }
}
