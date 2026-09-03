//! catalog — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Core {
    pub(crate) fn building_definition(&self, id: DefinitionId) -> Option<&BuildingDefinition> {
        self.definitions
            .buildings
            .iter()
            .find(|value| value.id == id)
    }

    pub(crate) fn item_definition(&self, id: ItemId) -> Option<&ItemDefinition> {
        self.definitions.items.iter().find(|value| value.id == id)
    }

    pub(crate) fn recipe(&self, id: RecipeId) -> Option<&RecipeDefinition> {
        self.definitions.recipes.iter().find(|value| value.id == id)
    }

    /// What to call an item in something the player reads. Numbered only when the definitions have
    /// nothing to say, which a validated catalogue never does.
    pub(crate) fn item_name(&self, item: ItemId) -> String {
        self.item_definition(item)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| format!("item {item}"))
    }

    pub(crate) fn stack_size(&self, item: ItemId) -> u32 {
        self.item_definition(item)
            .map(|definition| definition.stack_size)
            .unwrap_or(1)
            .max(1)
    }

    pub(crate) fn is_fluid(&self, item: ItemId) -> bool {
        self.item_definition(item)
            .is_some_and(|definition| definition.fluid)
    }

    pub(crate) fn transport_medium(&self, index: usize) -> TransportMedium {
        self.building_definition(self.entities[index].placed.definition_id)
            .map_or(TransportMedium::Solid, |definition| {
                definition.transport_medium
            })
    }

    pub(crate) fn transport_accepts(&self, index: usize, item: ItemId) -> bool {
        // Saved pre-pipe transport remains a working compatibility line. The migration records
        // exactly those stable ids; newly placed belts never enter this set.
        if self.legacy_fluid_belts.contains(&self.entities[index].id) {
            return true;
        }
        match self.transport_medium(index) {
            TransportMedium::Solid => !self.is_fluid(item),
            TransportMedium::Fluid => self.is_fluid(item),
        }
    }

    /// Whether a transport entity can ever hand an item to this target. Capacity and current stock
    /// stay dynamic; this only removes permanently dead joins such as a fresh belt into a pipe or a
    /// solid belt into a water-only tank.
    pub(crate) fn transport_target_compatible(&self, source: usize, target: usize) -> bool {
        if self.entities[source].kind != BuildingKind::Belt {
            return true;
        }
        let Some(target_definition) =
            self.building_definition(self.entities[target].placed.definition_id)
        else {
            return false;
        };
        if self.entities[target].kind == BuildingKind::Belt {
            return self.definitions.items.iter().any(|item| {
                self.transport_accepts(source, item.id) && self.transport_accepts(target, item.id)
            });
        }
        target_definition
            .accepted_item_ids
            .as_ref()
            .is_none_or(|accepted| {
                accepted
                    .iter()
                    .any(|&item| self.transport_accepts(source, item))
            })
    }

    /// The placement-time half of `transport_target_compatible`: the source has a definition but
    /// no stable entity id yet, while an existing target may still be a grandfathered liquid belt.
    pub(crate) fn prospective_transport_target_compatible(
        &self,
        source: &BuildingDefinition,
        target: usize,
    ) -> bool {
        let source_accepts = |item: ItemId| match source.transport_medium {
            TransportMedium::Solid => !self.is_fluid(item),
            TransportMedium::Fluid => self.is_fluid(item),
        };
        let Some(target_definition) =
            self.building_definition(self.entities[target].placed.definition_id)
        else {
            return false;
        };
        if self.entities[target].kind == BuildingKind::Belt {
            return self
                .definitions
                .items
                .iter()
                .any(|item| source_accepts(item.id) && self.transport_accepts(target, item.id));
        }
        target_definition
            .accepted_item_ids
            .as_ref()
            .is_none_or(|accepted| accepted.iter().any(|&item| source_accepts(item)))
    }

    /// How many slots an inventory occupies: one per part-filled stack of each item. This is the
    /// whole of the carrying rule — the inventory itself stays an `item_id → quantity` map, so
    /// nothing about the save format, the checksum, or transfer ordering changes with it.
    pub(crate) fn slots_used(&self, inventory: &BTreeMap<ItemId, u32>) -> u32 {
        inventory
            .iter()
            .map(|(&item, &quantity)| {
                let stack = self.stack_size(item);
                quantity.div_ceil(stack)
            })
            .sum()
    }

    pub(crate) fn player_snapshot(&self) -> PlayerSnapshot {
        let cooldown_total = if self.player.action_cooldown > 0 {
            self.last_action_cooldown_total
                .max(self.player.action_cooldown)
        } else {
            self.last_action_cooldown_total.max(GATHER_COOLDOWN_STEPS)
        };
        PlayerSnapshot {
            state: self.player.clone(),
            carry_stacks: self.carry_stacks(),
            radius: PLAYER_RADIUS,
            action_cooldown_total: cooldown_total,
            extract_radius: EXTRACT_RADIUS as u32,
            creative: self.creative,
            walk_path: self.walk_path.clone(),
        }
    }

    /// The carried inventory laid out one slot at a time, in item id order and full stacks first.
    pub(crate) fn carry_stacks(&self) -> Vec<Ingredient> {
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
    pub(crate) fn player_can_carry(&self, additions: &BTreeMap<ItemId, u32>) -> bool {
        if !self.creative
            && additions
                .iter()
                .any(|(&item, &quantity)| quantity > 0 && self.is_fluid(item))
        {
            return false;
        }
        let mut prospective = self.player.inventory.clone();
        add_inventory(&mut prospective, additions);
        self.slots_used(&prospective) <= self.player.carry_slots
    }

    /// How many more of one item the player can take. A part-filled stack absorbs its remainder for
    /// free; past that, each free slot is worth a whole stack.
    pub(crate) fn player_room_for(&self, item_id: ItemId) -> u32 {
        if !self.creative && self.is_fluid(item_id) {
            return 0;
        }
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

    /// Split a recovery into what the pack can still hold and what will not fit.
    ///
    /// Deliberately a walk over a working copy rather than a sum of `player_room_for` calls: each
    /// item taken consumes slots the next item would otherwise have been offered, so per-item
    /// answers would promise the same free slot twice. `BTreeMap` order makes the split itself
    /// deterministic, which matters because the remainder becomes ground items in the checksum.
    pub(crate) fn split_by_carry(
        &self,
        additions: &BTreeMap<ItemId, u32>,
    ) -> (BTreeMap<ItemId, u32>, BTreeMap<ItemId, u32>) {
        let mut prospective = self.player.inventory.clone();
        let mut carried = BTreeMap::new();
        let mut spilled = BTreeMap::new();
        for (&item, &quantity) in additions {
            if !self.creative && self.is_fluid(item) {
                spilled.insert(item, quantity);
                continue;
            }
            let stack = self.stack_size(item);
            let held = prospective.get(&item).copied().unwrap_or(0);
            let free_slots = self
                .player
                .carry_slots
                .saturating_sub(self.slots_used(&prospective));
            let partial = match held % stack {
                0 => 0,
                filled => stack - filled,
            };
            let room = partial.saturating_add(free_slots.saturating_mul(stack));
            let take = quantity.min(room);
            if take > 0 {
                *prospective.entry(item).or_default() += take;
                carried.insert(item, take);
            }
            if quantity > take {
                spilled.insert(item, quantity - take);
            }
        }
        (carried, spilled)
    }

    /// Set creative mode while constructing or resetting a run.
    ///
    /// Switching it on researches the whole tree. That is the entire implementation of "everything
    /// is unlocked": every gate in this file — `technology_met`, `category_unlocked`,
    /// `placement_legality`, and the availability the host draws its build panel from — already asks
    /// `researched`, so teaching the settlement everything unlocks all of it through the paths the
    /// ordinary game uses rather than through a second set of creative-only exceptions.
    ///
    /// Running-game commands cannot call this path: game mode is fixed after world creation.
    pub(crate) fn set_creative(&mut self, enabled: bool) {
        if self.creative == enabled {
            return;
        }
        self.creative = enabled;
        if enabled {
            self.grant_creative_skills();
            let known = self.researched.len();
            for technology in &self.technologies.technologies {
                self.researched.insert(technology.id);
            }
            self.apply_research_effects();
            if self.researched.len() != known {
                self.refill_requests();
            }
            self.events.push("Creative mode on".into());
        } else {
            self.events.push("Creative mode off".into());
        }
    }

    /// Put an item into the pack out of nowhere. Creative only.
    ///
    /// Capacity still applies. A grant that would overflow the pack is trimmed to what fits rather
    /// than refused outright, so holding the button on a full pack tops it up and stops, and the
    /// carrying rule stays the one thing every route into the inventory obeys.
    pub(crate) fn grant(&mut self, item_id: ItemId, quantity: u32) -> Result<(), String> {
        if !self.creative {
            return Err("granting items needs creative mode".into());
        }
        if self.item_definition(item_id).is_none() {
            return Err("unknown item".into());
        }
        let room = self.player_room_for(item_id);
        let granted = quantity.min(room);
        if granted == 0 {
            return Err("no room to carry that".into());
        }
        *self.player.inventory.entry(item_id).or_default() += granted;
        let name = self
            .item_definition(item_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_default();
        self.events.push(format!("Granted {granted} {name}"));
        Ok(())
    }

    /// Destroy carried stock. Creative only. `item_id: None` empties the pack.
    pub(crate) fn discard(&mut self, item_id: Option<ItemId>, quantity: u32) -> Result<(), String> {
        if !self.creative {
            return Err("discarding items needs creative mode".into());
        }
        let Some(item_id) = item_id else {
            if self.player.inventory.is_empty() {
                return Err("nothing to discard".into());
            }
            self.player.inventory.clear();
            self.events.push("Pack cleared".into());
            return Ok(());
        };
        let held = self.player.inventory.get(&item_id).copied().unwrap_or(0);
        if held == 0 {
            return Err("nothing to discard".into());
        }
        // A quantity of zero means the whole stack, so the host can offer "drop all of this" without
        // first having to read back how much of it is held.
        let dropped = if quantity == 0 {
            held
        } else {
            quantity.min(held)
        };
        subtract_item(&mut self.player.inventory, item_id, dropped);
        let name = self
            .item_definition(item_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_default();
        self.events.push(format!("Discarded {dropped} {name}"));
        Ok(())
    }

    /// Widen or narrow the pack. Creative only.
    ///
    /// The scenario plus researched bonuses is the floor: creative may hand out room, never take
    /// away room the run earned. `MAX_CARRY_SLOTS` is the ceiling. Narrowing below what is already
    /// carried is refused rather than dropping the difference, because there is no honest place
    /// for stranded stock to go.
    pub(crate) fn set_carry_slots(&mut self, slots: u32) -> Result<(), String> {
        if !self.creative {
            return Err("resizing the pack needs creative mode".into());
        }
        if slots < self.earned_carry_slots() || slots > MAX_CARRY_SLOTS {
            return Err("that pack size is out of range".into());
        }
        if slots < self.slots_used(&self.player.inventory) {
            return Err("too much carried for a pack that small".into());
        }
        if slots == self.player.carry_slots {
            return Ok(());
        }
        self.player.carry_slots = slots;
        self.events.push(format!("Pack resized to {slots} slots"));
        Ok(())
    }
}
