//! inventory — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Core {
    /// Whether a building's stock is the player's to reach into.
    ///
    /// Every kind that keeps an `inventory` a hand could sensibly hold: a box, and the three
    /// machines that stand around holding fuel and inputs. A belt's cargo is a position on a lane
    /// rather than a store, the hub's intake is the contract, and an extractor, pole, or bridge
    /// keeps nothing — so those are refused rather than silently doing nothing.
    pub(crate) fn stock_is_reachable_by_hand(kind: BuildingKind) -> bool {
        matches!(
            kind,
            BuildingKind::Extractor
                | BuildingKind::Pump
                | BuildingKind::Container
                | BuildingKind::Composer
                | BuildingKind::Generator
                | BuildingKind::Boiler
        )
    }

    /// Resolve the building a hand transfer names, at the range every other edit is held to.
    pub(crate) fn hand_transfer_target(&self, q: i32, r: i32, verb: &str) -> Result<usize, String> {
        if !self.within_build_range_of_target(q, r) {
            return Err(format!("{verb} target is outside build range"));
        }
        let index = self.entity_at(q, r).ok_or("nothing to reach into there")?;
        if !Self::stock_is_reachable_by_hand(self.entities[index].kind) {
            return Err("that building has no stock you can reach".into());
        }
        Ok(index)
    }

    /// Move stock out of a building and into the player's pack. A bounded command beside `place`
    /// and `erase`, range-checked exactly as they are. The requested quantity is a ceiling, not a
    /// demand: what actually moves is limited by what the building holds and by what the player can
    /// still carry, so a partial withdrawal succeeds and destroys nothing.
    ///
    /// **Only free stock comes back.** `inventory` is exactly that — inputs a running craft has
    /// claimed have already moved to `reserved_inputs`, and energy already released from a coal
    /// sits in `fuel_charge`. Neither is reachable, which is what keeps "take the coal back out of
    /// a burner" honest: the unburned lumps are yours, the heat already in the firebox is spent.
    #[cfg(test)]
    pub(crate) fn withdraw(
        &mut self,
        q: i32,
        r: i32,
        item_id: ItemId,
        quantity: u32,
    ) -> Result<(), String> {
        self.withdraw_from(q, r, StockKind::Auto, item_id, quantity)
    }

    pub(crate) fn withdraw_from(
        &mut self,
        q: i32,
        r: i32,
        stock: StockKind,
        item_id: ItemId,
        quantity: u32,
    ) -> Result<(), String> {
        if !self.creative && self.is_fluid(item_id) {
            return Err("loose fluid needs a pipe or a barrel".into());
        }
        let index = self.hand_transfer_target(q, r, "withdraw")?;
        let stock = if stock == StockKind::Auto {
            self.stock_kind_for_item(index, item_id)
                .or_else(|| {
                    (self.stock_quantity(index, StockKind::Output, item_id) > 0)
                        .then_some(StockKind::Output)
                })
                .unwrap_or(StockKind::Inventory)
        } else {
            stock
        };
        let stored = self.stock_quantity(index, stock, item_id);
        if stored == 0 {
            return Err("this building holds none of that item".into());
        }
        let moved = quantity.min(stored).min(self.player_room_for(item_id));
        if moved == 0 {
            return Err("carrying capacity is full".into());
        }
        self.subtract_stock(index, stock, item_id, moved);
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
    /// The price of an edit that replaces one cost row with another, netted per item and applied
    /// all or nothing.
    ///
    /// Both halves are checked before either is moved — the same rule `erase` keeps — which is what
    /// stops an edit and its undo from minting items between them. Netting rather than paying the
    /// two halves separately is what lets a player with a full pack make an edit that costs them
    /// nothing: the difference is what the change actually costs, so the difference is what travels.
    ///
    /// Creative is neither billed nor credited, so both maps stay empty and nothing moves — the same
    /// answer `place` and `erase_refund` give, and the reason a ladder walked up and erased at the
    /// top still balances at zero.
    pub(crate) fn charge_difference(
        &mut self,
        charge_row: &[Ingredient],
        credit_row: &[Ingredient],
    ) -> Result<(), String> {
        let mut charge: BTreeMap<ItemId, u32> = BTreeMap::new();
        let mut credit: BTreeMap<ItemId, u32> = BTreeMap::new();
        if !self.creative {
            add_ingredients(&mut charge, charge_row);
            add_ingredients(&mut credit, credit_row);
        }
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
        // Only when the step actually hands something back. An edit whose new cost contains the old
        // one — which is the shape a ladder should have — returns nothing, and refusing it because
        // the pack is full would be refusing an edit that does not touch the pack.
        if !back.is_empty() && !self.player_can_carry(&back) {
            return Err("no room to carry what this would return".into());
        }
        for (item_id, quantity) in &owed {
            subtract_item(&mut self.player.inventory, *item_id, *quantity);
        }
        add_inventory(&mut self.player.inventory, &back);
        Ok(())
    }

    /// One atomic legality check for the cells a taller tier would newly occupy.
    ///
    /// This is the alternative to reserving an envelope at initial placement: nothing is held
    /// empty in advance, so the moment of growth is the moment the ground has to be proved. It
    /// asks of the new cells exactly what `placement_legality` asks of a fresh site — free of
    /// buildings, free of the player, buildable terrain, no boundary through the shape — and it
    /// asks the level-pad question of the *whole* enlarged footprint, because a machine that grew
    /// onto a slope is as unstandable as one placed there.
    ///
    /// Refusing here also protects the ports. An output ray leaves the anchor and skips the
    /// building's own cells, so a longer footprint changes where it first meets something else —
    /// unless the ground it grew onto was empty, which is what this refuses to assume.
    pub(crate) fn upgrade_growth_legality(
        &self,
        index: usize,
        next: &BuildingDefinition,
        current: &[Coordinate],
        grown: &[Coordinate],
        next_envelope: &[Coordinate],
        next_clearance: &[Coordinate],
    ) -> Result<(), String> {
        let held: BTreeSet<(i32, i32)> = current.iter().map(|cell| (cell.q, cell.r)).collect();
        let growth: Vec<Coordinate> = grown
            .iter()
            .copied()
            .filter(|cell| !held.contains(&(cell.q, cell.r)))
            .collect();
        if self.boundary_crosses_footprint(grown) {
            return Err("A boundary crosses this building footprint; remove it first".into());
        }
        if self.boundary_crosses_footprint(next_envelope) {
            return Err(
                "A boundary crosses this building's service envelope; remove it first".into(),
            );
        }
        for cell in &growth {
            // Own envelope is the reserved path: the cell was held empty at placement, so growth
            // does not re-ask occupancy. Anything else — a neighbour, another envelope, a rotor —
            // is the atomic path, and a refusal here leaves the building unchanged.
            match self.reservation_conflict(cell.q, cell.r, next.kind, Some(index), false) {
                Ok(()) => {}
                Err(reason) if reason.contains("occupied hex") => {
                    return Err(format!(
                        "{} needs more room than this one has; clear the hexes beside it",
                        next.name
                    ));
                }
                Err(reason) => return Err(reason),
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
            // Only the definition's own water rule, not the belt-on-a-bridge exemption: a bridge
            // carries transport it does not own, and growing a machine over one would take the
            // crossing away from the line already using it.
            let shallow_support = next.placement_rule == PlacementRule::Shallows
                && self.terrain_at(cell.q, cell.r) == Terrain::ShallowWater;
            if self.terrain_blocks_construction(cell.q, cell.r) && !shallow_support {
                return Err("environment blocks construction".into());
            }
        }
        for cell in next_envelope {
            if held.contains(&(cell.q, cell.r)) {
                continue;
            }
            self.reservation_conflict(cell.q, cell.r, next.kind, Some(index), false)?;
            let shallow_support = next.placement_rule == PlacementRule::Shallows
                && self.terrain_at(cell.q, cell.r) == Terrain::ShallowWater;
            if self.terrain_blocks_construction(cell.q, cell.r) && !shallow_support {
                return Err("environment blocks construction".into());
            }
        }
        for cell in next_clearance {
            self.reservation_conflict(cell.q, cell.r, next.kind, Some(index), true)?;
            if let Some(other) = self.entity_at(cell.q, cell.r) {
                if other != index && !Self::is_low_infrastructure(self.entities[other].kind) {
                    return Err("building footprint overlaps an occupied hex".into());
                }
            }
        }
        let elevations: Vec<_> = grown
            .iter()
            .map(|cell| self.ground_elevation_at(cell.q, cell.r))
            .collect();
        if let (Some(low), Some(high)) = (
            elevations.iter().min().copied(),
            elevations.iter().max().copied(),
        ) {
            if high - low > self.pad_step_limit(next.foundation_class) {
                return Err("This ground is too uneven; level a pad for this footprint".into());
            }
        }
        Ok(())
    }

    pub(crate) fn upgrade(&mut self, q: i32, r: i32) -> Result<(), String> {
        if !self.within_build_range_of_target(q, r) {
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
        // An upgrade keeps the entity's heading, and the ladder pins both definitions to the same
        // orientation axis, so both halves of the netting are priced at that one heading.
        let orientation = self.entities[index].placed.orientation;
        let refund = current.cost_at(orientation).to_vec();
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
            if !self.stock_fits_capacity(index, capacity) {
                return Err(format!(
                    "{} holds more than the next tier stores",
                    current.name
                ));
            }
        }
        // A taller tier may claim more ground than the one standing here. Judge the whole enlarged
        // footprint once, before anything is charged or written: the ladder guarantees the cells it
        // already occupies are kept, so what is left to prove is that the new ones are free, legal
        // and level enough to stand on together with the old. Every refusal below leaves the
        // building exactly as it was.
        let current_cells = self.entity_footprint(&self.entities[index]);
        let next_placed = PlacedBuilding {
            definition_id: next_id,
            ..self.entities[index].placed
        };
        let grown = self.footprint_for(next_placed, orientation);
        let next_envelope = self.envelope_for(next_placed, orientation);
        let next_clearance = self.clearance_for(next_placed, orientation);
        self.upgrade_growth_legality(
            index,
            &next,
            &current_cells,
            &grown,
            &next_envelope,
            &next_clearance,
        )?;
        // Netted per item, so the two halves of the price never travel through the pack. A player
        // upgrading with a full pack is charged the difference and asked to carry the difference,
        // which is what an in-place edit actually costs them.
        let old_links = self.graph_links_by_id();
        // Both footprints, because the graph has to forget rays that used to cross a cell the
        // building now covers as surely as it has to recompile the ones that touched it before.
        let changed_cells: BTreeSet<(i32, i32)> = current_cells
            .iter()
            .chain(grown.iter())
            .map(|cell| (cell.q, cell.r))
            .collect();
        self.charge_difference(next.cost_at(orientation), &refund)?;
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

    /// Put stock from the player's pack into a building. The exact mirror of `withdraw`, and it
    /// keeps the same contract: the requested quantity is a ceiling, not a demand, so what actually
    /// moves is limited by what the player holds and by the room the building has left. A partial
    /// store succeeds and destroys nothing.
    ///
    /// **What a building will take is `accepts_item` — the same predicate a belt is held to.** A
    /// hand feeding a smelter is the same event as a lane feeding it, so a machine that refuses
    /// iron ore off a belt refuses it off a palm too, and there is one place where "a furnace takes
    /// its recipe's inputs and anything that burns" is written down. The room is asked separately,
    /// which is what lets a refusal say whether the building had no use for the item or simply no
    /// space left — two different problems the player fixes two different ways.
    #[cfg(test)]
    pub(crate) fn store(
        &mut self,
        q: i32,
        r: i32,
        item_id: ItemId,
        quantity: u32,
    ) -> Result<(), String> {
        self.store_into(q, r, StockKind::Auto, item_id, quantity)
    }

    pub(crate) fn store_into(
        &mut self,
        q: i32,
        r: i32,
        stock: StockKind,
        item_id: ItemId,
        quantity: u32,
    ) -> Result<(), String> {
        let index = self.hand_transfer_target(q, r, "store")?;
        let held = self.player.inventory.get(&item_id).copied().unwrap_or(0);
        if held == 0 {
            return Err("you are not carrying any of that item".into());
        }
        let stock = if stock == StockKind::Auto {
            self.stock_kind_for_item(index, item_id)
                .ok_or("this building has no use for that")?
        } else {
            stock
        };
        if !self.stock_accepts_item(index, stock, item_id) {
            return Err("this building has no use for that".into());
        }
        let moved = quantity
            .min(held)
            .min(self.room_for_stock(index, stock, item_id));
        if moved == 0 {
            return Err("this building is full".into());
        }
        subtract_item(&mut self.player.inventory, item_id, moved);
        self.add_stock(
            index,
            stock,
            Cargo {
                item_id,
                quantity: moved,
            },
        );
        let id = self.entities[index].id;
        self.dirty.entities.push(id);
        let name = self
            .item_definition(item_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| format!("item {item_id}"));
        self.events.push(format!("Stored {moved} × {name}"));
        Ok(())
    }

    pub(crate) fn pickup_player_stack(
        &mut self,
        item_id: ItemId,
        quantity: u32,
    ) -> Result<(), String> {
        if self.player.hand.is_some() {
            return Err("your hand is already holding a stack".into());
        }
        let held = self.player.inventory.get(&item_id).copied().unwrap_or(0);
        let moved = quantity.min(held).min(self.stack_size(item_id));
        if moved == 0 {
            return Err("you are not carrying any of that item".into());
        }
        subtract_item(&mut self.player.inventory, item_id, moved);
        self.player.hand = Some(Cargo {
            item_id,
            quantity: moved,
        });
        Ok(())
    }

    pub(crate) fn pickup_building_stack(
        &mut self,
        q: i32,
        r: i32,
        stock: StockKind,
        item_id: ItemId,
        quantity: u32,
    ) -> Result<(), String> {
        if !self.creative && self.is_fluid(item_id) {
            return Err("loose fluid needs a pipe or a barrel".into());
        }
        if self.player.hand.is_some() {
            return Err("your hand is already holding a stack".into());
        }
        if matches!(stock, StockKind::Auto) {
            return Err("pick a named building compartment".into());
        }
        let index = self.hand_transfer_target(q, r, "pick up")?;
        let stored = self.stock_quantity(index, stock, item_id);
        let moved = quantity.min(stored).min(self.stack_size(item_id));
        if moved == 0 {
            return Err("this compartment holds none of that item".into());
        }
        self.subtract_stock(index, stock, item_id, moved);
        self.player.hand = Some(Cargo {
            item_id,
            quantity: moved,
        });
        self.dirty.entities.push(self.entities[index].id);
        Ok(())
    }

    pub(crate) fn place_player_stack(&mut self, quantity: u32) -> Result<(), String> {
        let hand = self.player.hand.ok_or("your hand is empty")?;
        let moved = quantity
            .min(hand.quantity)
            .min(self.player_room_for(hand.item_id));
        if moved == 0 {
            return Err("carrying capacity is full".into());
        }
        *self.player.inventory.entry(hand.item_id).or_default() += moved;
        if moved == hand.quantity {
            self.player.hand = None;
        } else if let Some(held) = &mut self.player.hand {
            held.quantity -= moved;
        }
        Ok(())
    }

    pub(crate) fn place_building_stack(
        &mut self,
        q: i32,
        r: i32,
        stock: StockKind,
        quantity: u32,
    ) -> Result<(), String> {
        if matches!(stock, StockKind::Auto) {
            return Err("pick a named building compartment".into());
        }
        let hand = self.player.hand.ok_or("your hand is empty")?;
        let index = self.hand_transfer_target(q, r, "place")?;
        if !self.stock_accepts_item(index, stock, hand.item_id) {
            return Err("that item does not belong in this compartment".into());
        }
        let moved =
            quantity
                .min(hand.quantity)
                .min(self.room_for_stock(index, stock, hand.item_id));
        if moved == 0 {
            return Err("this compartment is full".into());
        }
        self.add_stock(
            index,
            stock,
            Cargo {
                item_id: hand.item_id,
                quantity: moved,
            },
        );
        if moved == hand.quantity {
            self.player.hand = None;
        } else if let Some(held) = &mut self.player.hand {
            held.quantity -= moved;
        }
        self.dirty.entities.push(self.entities[index].id);
        Ok(())
    }

    pub(crate) fn drop_player_stack(
        &mut self,
        q: i32,
        r: i32,
        quantity: u32,
    ) -> Result<(), String> {
        let hand = self.player.hand.ok_or("your hand is empty")?;
        if !self.within_build_range_of_target(q, r) {
            return Err("that hex is out of reach".into());
        }
        self.ensure_neighborhood(self.player.x, self.player.y);
        self.ensure_tile(q, r);
        if self.terrain_blocks_movement(q, r) {
            return Err("items cannot land on impassable terrain".into());
        }
        let moved = quantity.min(hand.quantity);
        if moved == 0 {
            return Err("nothing to drop".into());
        }
        let item_id = hand.item_id;
        if moved == hand.quantity {
            self.player.hand = None;
        } else if let Some(held) = &mut self.player.hand {
            held.quantity -= moved;
        }
        self.add_ground_item(q, r, item_id, moved);
        let name = self
            .item_definition(item_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| format!("item {item_id}"));
        self.events.push(format!("Dropped {moved} × {name}"));
        Ok(())
    }

    /// Add deterministic world-owned cargo at one hex. Player drops and demolished transport both
    /// use this path, so stacking, lifetime refresh, dirty tracking, saves, and wire snapshots
    /// cannot disagree about what counts as an item on the ground.
    pub(crate) fn add_ground_item(&mut self, q: i32, r: i32, item_id: ItemId, quantity: u32) {
        if quantity == 0 {
            return;
        }
        let despawn_tick = self.tick + GROUND_ITEM_LIFETIME_TICKS;
        if let Some(existing) = self
            .ground_items
            .iter_mut()
            .find(|item| item.q == q && item.r == r && item.item_id == item_id)
        {
            existing.quantity += quantity;
            existing.despawn_tick = despawn_tick;
        } else {
            let id = self.next_ground_item_id;
            self.next_ground_item_id = self.next_ground_item_id.wrapping_add(1);
            self.ground_items.push(GroundItem {
                id,
                q,
                r,
                item_id,
                quantity,
                despawn_tick,
            });
        }
        self.dirty.ground_items = true;
    }
}
