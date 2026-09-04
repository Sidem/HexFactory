//! deposits — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Core {
    /// `entities` is always ordered by stable id: initial ids are assigned in sorted-anchor order,
    /// placement appends the next monotonic id, erasing preserves relative order, and restoring a
    /// save re-sorts. So one marked id resolves in log time rather than a scan of the blueprint.
    pub(crate) fn index_of_entity(&self, id: u32) -> Option<usize> {
        self.entities
            .binary_search_by_key(&id, |entity| entity.id)
            .ok()
    }

    pub(crate) fn entity_at(&self, q: i32, r: i32) -> Option<usize> {
        // `occupied_entities` inserts in stable-id order, so a support below transport is replaced
        // by the later transport index just as the former reverse scan required.
        self.runtime.occupied.get(&(q, r)).copied()
    }

    pub(crate) fn bridge_at(&self, q: i32, r: i32) -> bool {
        self.entities.iter().any(|entity| {
            entity.kind == BuildingKind::Bridge
                && self
                    .entity_footprint(entity)
                    .iter()
                    .any(|cell| cell.q == q && cell.r == r)
        })
    }

    /// Whether an extractor (or a gather) at this hex can draw from `cell`. One predicate: the
    /// cell is a field hex inside the reach it was asked about. Placement and the cached candidate
    /// list share it, so a resolved reference cannot drift from the rule that allowed the building.
    ///
    /// Reach is a parameter rather than a constant because it is the flagship upgrade. It is still
    /// one predicate and one rule — the caller says how far *it* reaches, and a hand gather and a
    /// tier-1 extractor standing on the same hex are asking the same question at two reaches, not
    /// two questions.
    pub(crate) fn field_covered_at(
        &self,
        extractor: (i32, i32),
        cell: (i32, i32),
        radius: i32,
    ) -> bool {
        axial_distance(extractor, cell) <= radius && self.field_at(cell.0, cell.1).is_some()
    }

    /// How far an extractor built from this definition reaches, counting its own cell. Absent in
    /// the data means the base reach, so the tier-0 extractor is exactly what it always was.
    pub(crate) fn extract_radius_of(&self, definition_id: DefinitionId) -> i32 {
        self.building_definition(definition_id)
            .and_then(|definition| definition.extract_radius)
            .map(|radius| radius as i32)
            .unwrap_or(EXTRACT_RADIUS)
    }

    /// The field cell a player standing at this world point draws from: their own hex while it
    /// still holds stock, otherwise the nearest covered neighbour. `gather` and placement both go
    /// through here, so one rule decides what a hex reaches.
    /// The field cell the *player* draws from at this world point. The hand reaches `EXTRACT_RADIUS`
    /// and no tier changes that: an upgrade is something the player builds, not something they grow.
    pub(crate) fn resource_at_world(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        let (q, r) = world_to_axial(x, y);
        self.deposit_candidates(q, r, EXTRACT_RADIUS)
            .into_iter()
            .find(|&key| self.deposit_quantity(key) > 0)
    }

    /// Every field cell something at `(q, r)` reaching `radius` covers, ordered nearest first and
    /// then by tile key — the exact order `resource_at_world` resolves. Remaining quantity is
    /// deliberately not part of the ordering, so one resolved list stays correct for the whole life
    /// of the field.
    pub(crate) fn deposit_candidates(&self, q: i32, r: i32, radius: i32) -> Vec<(i32, i32)> {
        let origin = (q, r);
        let mut candidates: Vec<(i64, (i32, i32))> = hexes_in_radius(origin, radius)
            .into_iter()
            .filter(|&cell| self.field_covered_at(origin, cell, radius))
            .map(|cell| {
                let (x, y) = axial_world(q, r);
                let (cx, cy) = axial_world(cell.0, cell.1);
                (squared_distance(x, y, cx, cy), cell)
            })
            .collect();
        candidates.sort_unstable();
        candidates.into_iter().map(|(_, key)| key).collect()
    }

    pub(crate) fn deposit_quantity(&self, key: (i32, i32)) -> u32 {
        // The surface gate has to be here as well as in `field_at`, because a partly worked deposit
        // is answered from the tile overlay before `field_at` is ever consulted.
        if self.surface_at(key.0, key.1) != 0 {
            return 0;
        }
        if let Some(resource) = self.tiles.get(&key).and_then(|tile| tile.resource.as_ref()) {
            return resource.quantity;
        }
        self.field_at(key.0, key.1)
            .map(|field| field.quantity)
            .unwrap_or(0)
    }

    pub(crate) fn write_overlay(
        &mut self,
        q: i32,
        r: i32,
        item_id: ItemId,
        quantity: u32,
        initial: u32,
    ) {
        let (x, y) = axial_world(q, r);
        let terrain = self.terrain_at(q, r);
        self.tiles.insert(
            (q, r),
            TileState {
                q,
                r,
                x,
                y,
                radius: HEX_RADIUS as u32,
                terrain,
                resource: Some(ResourceState {
                    item_id,
                    quantity,
                    initial_quantity: initial,
                }),
            },
        );
        // Every overlay write is where a cell enters or leaves regrowth, so the set stays exact
        // without anything scanning the world for it.
        if quantity < initial && self.regrowth_ticks(item_id).is_some() {
            self.flora_regrowth.insert((q, r));
        } else {
            self.flora_regrowth.remove(&(q, r));
        }
        // A cell that just went out is a row the host has to stop drawing, and the resources group
        // carries no removal, so the group is resent whole. For ore that happens once in a deposit's
        // life. A stand of trees can empty again every time it is worked back to nothing, which is
        // bounded below by the regrowth cadence: one resend per starving extractor per unit grown.
        // That is the cost of a cleared hex looking like ordinary ground, and it is the thing to
        // measure before trading it for a tombstone in the wire.
        if quantity == 0 {
            self.dirty.resources_replace = true;
        }
    }

    /// How often one unit of this item grows back, for a resource that is flora. `None` for every
    /// ore, which is what makes ore finite.
    pub(crate) fn regrowth_ticks(&self, item_id: ItemId) -> Option<u32> {
        self.item_definition(item_id)
            .and_then(|item| item.regrowth_ticks)
            .filter(|&ticks| ticks > 0)
    }

    /// A cell whose deposit is worked down to nothing, whether or not it will come back.
    ///
    /// Publishing asks this and no more. An emptied hex is ordinary ground while it is empty: no
    /// field to draw, no quantity to report, no bar reading `0/3`. Ore stays that way forever and a
    /// cleared stand does not — the row returns by itself when the first unit grows back — but
    /// neither is a deposit the host should be drawing today.
    pub(crate) fn deposit_empty(&self, q: i32, r: i32) -> bool {
        self.tiles
            .get(&(q, r))
            .and_then(|tile| tile.resource.as_ref())
            .is_some_and(|resource| resource.quantity == 0)
    }

    /// A deposit that is spent and will not come back.
    ///
    /// Ore has no regrowth cadence, so a hex worked to nothing is not a deposit any more: it is
    /// ordinary ground that happens to remember what it used to hold. The overlay entry has to
    /// stay, because it is the only record that the generated deposit was taken — deleting it
    /// would hand the hex its full generated quantity back on the next read. So the *entry* is not
    /// the question anyone should ask; this is. Grading reads it, because moving ground under a
    /// stand that is still growing back would erase a deposit the world is in the middle of
    /// returning, while a dead ore hex has nothing left to protect. [`Self::deposit_empty`] is the
    /// weaker question, and the one publishing asks.
    ///
    /// A sealed deposit is deliberately not exhausted. Paving suppresses a deposit, and lifting the
    /// paving hands back what went under it; only the pick empties one.
    pub(crate) fn deposit_exhausted(&self, q: i32, r: i32) -> bool {
        self.tiles
            .get(&(q, r))
            .and_then(|tile| tile.resource.as_ref())
            .is_some_and(|resource| {
                resource.quantity == 0 && self.regrowth_ticks(resource.item_id).is_none()
            })
    }

    /// Rebuild the regrowth set from the overlay. It is a pure function of the stored tiles and the
    /// item definitions, so a save records the tiles and this recovers the set — the file never
    /// carries derived state.
    pub(crate) fn rebuild_flora_regrowth(&mut self) {
        self.flora_regrowth = self
            .tiles
            .iter()
            .filter_map(|(&key, tile)| {
                let resource = tile.resource.as_ref()?;
                // A sealed cell leaves the set: paving suppresses regrowth exactly as it suppresses
                // access, and stripping the surface rebuilds the set and lets it grow again.
                (resource.quantity < resource.initial_quantity
                    && self.surface_at(key.0, key.1) == 0
                    && self.regrowth_ticks(resource.item_id).is_some())
                .then_some(key)
            })
            .collect();
    }

    /// What is standing in a cell's own neighbourhood, and what that ground could hold: the cell
    /// itself and its six neighbours, counting only cells whose deposit is this same flora.
    ///
    /// This is the seed pressure a cut cell recovers under. Both halves come from `field_at`, so
    /// paving suppresses a hex as a seed source exactly as it suppresses it as a deposit, and both
    /// are generated rather than surveyed facts, so how far the player has walked never enters the
    /// answer. A neighbourhood of ground that never carried this flora contributes nothing to
    /// either total, which is why an edge stand beside a meadow is not penalised for the meadow.
    fn seed_pressure(&self, q: i32, r: i32, item_id: ItemId) -> (u32, u32) {
        let mut standing = 0;
        let mut capacity = 0;
        let ring = DIRECTIONS.iter().map(|&(dq, dr)| (q + dq, r + dr));
        for cell in std::iter::once((q, r)).chain(ring) {
            let Some(field) = self.field_at(cell.0, cell.1) else {
                continue;
            };
            if field.item_id != item_id {
                continue;
            }
            capacity += field.initial_quantity;
            standing += self.deposit_quantity(cell);
        }
        (standing, capacity)
    }

    /// How many of an item's regrowth intervals one unit of growth costs at this cell.
    ///
    /// Flora comes from flora. A cell whose neighbourhood is intact pays one interval — exactly the
    /// cadence the item declares — and a cell whose neighbourhood has been thinned pays
    /// proportionally more, because proportionally less is standing to seed it. `None` is a
    /// neighbourhood with nothing left standing at all: a clear-cut that took the last tree for a
    /// ring around does not come back on a timer, and that is the point of the rule. Cutting into
    /// the edge of a wood is cheap and self-repairing; flattening one costs the wood.
    ///
    /// It is a rate rather than a die roll, so the answer stays a pure function of the world and
    /// enters the checksum without an RNG or a stored accumulator.
    fn growth_intervals(&self, q: i32, r: i32, item_id: ItemId) -> Option<u64> {
        let (standing, capacity) = self.seed_pressure(q, r, item_id);
        if standing == 0 {
            return None;
        }
        // Rounded rather than rounded up. A ceiling would read "one unit short of full" as half
        // speed and silently double every recovery in the game the moment a single tree was cut.
        Some(u64::from((capacity + standing / 2) / standing).max(1))
    }

    /// Whether ground carrying no deposit of its own could hold flora.
    ///
    /// Every clause is a physical refusal rather than a taste: an unsurveyed hex has no facts to
    /// read and growth may not open the world to get them, a hex that already holds something is
    /// not empty ground, paving and a footprint are things laid over it, and rock under standing
    /// water or under a cliff face is not soil. A hex that passes all of them is meadow or soil
    /// nobody is using, which is the only place a wood has ever had to go.
    pub(crate) fn can_take_root(&self, q: i32, r: i32) -> bool {
        crate::hydrology::WaterField::surveyed(self, q, r)
            && self.buried_field_at(q, r).is_none()
            && self.surface_at(q, r) == 0
            && !self.runtime.occupied.contains_key(&(q, r))
            && self.water_depth_at(q, r) == 0
            && self.generated_ground_at(q, r).substrate != Substrate::Rock
            && !self.terrain_at(q, r).blocks_construction()
    }

    /// Where a stand that has just filled itself back up puts its next tree, if anywhere.
    ///
    /// Among the neighbours that could hold it, the seed goes to the most sheltered — the candidate
    /// with the most of this same flora already standing around it. That is where a wood actually
    /// thickens first, and it makes an edge advance as a rounded front instead of a line marching
    /// down the direction table. Ties go to the lower direction index, so the answer is a pure
    /// function of the world and needs no die.
    pub(crate) fn colonisation_target(
        &self,
        q: i32,
        r: i32,
        item_id: ItemId,
    ) -> Option<(i32, i32)> {
        let mut best: Option<((i32, i32), u32)> = None;
        for &(dq, dr) in &DIRECTIONS {
            let cell = (q + dq, r + dr);
            if !self.can_take_root(cell.0, cell.1) {
                continue;
            }
            let (standing, _) = self.seed_pressure(cell.0, cell.1, item_id);
            if best.is_none_or(|(_, most)| standing > most) {
                best = Some((cell, standing));
            }
        }
        best.map(|(cell, _)| cell)
    }

    /// Put a new stand on empty ground beside one that has just healed.
    ///
    /// This is the only way the set of hexes carrying flora ever grows, and it is deliberately fed
    /// by recovery rather than by standing wood: a forest at the extent the generator drew it is at
    /// its equilibrium and stays there, while one that is *growing back* is expanding, and an
    /// expansion does not stop neatly at the line a cut used to be. So woods spread where the player
    /// has been working them and nowhere else, and the world does not silently fill in with trees
    /// around somebody who never touched it.
    ///
    /// The new stand starts at one unit and is given its parent's capacity, so it is a seedling on
    /// ground that will hold a wood the size of the wood that seeded it — and from the next tick it
    /// is an ordinary regrowth cell, slowed by its own thin neighbourhood exactly like any other.
    fn colonise_from(&mut self, from: (i32, i32), item_id: ItemId, capacity: u32) {
        let Some(cell) = self.colonisation_target(from.0, from.1, item_id) else {
            return;
        };
        self.write_overlay(cell.0, cell.1, item_id, 1, capacity);
        self.dirty.resources.push(cell);
        // Ground that carried nothing now carries a deposit, so every extractor's candidate list
        // and status may read differently.
        self.mark_all_entities_dirty();
        self.events
            .push(format!("Flora spread to {},{}", cell.0, cell.1));
    }

    /// Grow every cut flora cell back by one unit, on the cadence its item declares slowed by how
    /// much of its neighbourhood is standing. Walking the marked set rather than the world is the
    /// same sparsity rule the rest of the tick follows: an untouched forest is not in the set, and
    /// a fully regrown cell leaves it.
    pub(crate) fn regrow_flora(&mut self) {
        if self.flora_regrowth.is_empty() {
            return;
        }
        let due: Vec<(i32, i32)> = self
            .flora_regrowth
            .iter()
            .copied()
            .filter(|key| {
                let Some(ticks) = self
                    .tiles
                    .get(key)
                    .and_then(|tile| tile.resource.as_ref())
                    .map(|resource| resource.item_id)
                    .and_then(|item_id| self.regrowth_ticks(item_id).map(|ticks| (item_id, ticks)))
                else {
                    return false;
                };
                let (item_id, interval) = (ticks.0, u64::from(ticks.1));
                // The item's own cadence is the cheap outer gate, so the seven-cell neighbourhood
                // is only read on the ticks that could actually grow something. A cell whose
                // neighbourhood cannot seed it stays in the set at that same cost, because a
                // neighbour regrowing is what would let it start again.
                if self.tick % interval != 0 {
                    return false;
                }
                self.growth_intervals(key.0, key.1, item_id)
                    .is_some_and(|intervals| (self.tick / interval) % intervals == 0)
            })
            .collect();
        for key in due {
            let Some(resource) = self.tiles.get(&key).and_then(|tile| tile.resource.as_ref())
            else {
                continue;
            };
            let (item_id, quantity, initial) = (
                resource.item_id,
                resource.quantity,
                resource.initial_quantity,
            );
            if quantity >= initial {
                self.flora_regrowth.remove(&key);
                continue;
            }
            self.write_overlay(key.0, key.1, item_id, quantity + 1, initial);
            self.dirty.resources.push(key);
            if quantity + 1 >= initial {
                self.colonise_from(key, item_id, initial);
            }
            if quantity == 0 {
                // A cell that had been cut to nothing can restart an extractor that reported it
                // exhausted, and every extractor covering it may now report a different status.
                self.mark_all_entities_dirty();
                self.events
                    .push(format!("Flora at {},{} regrew", key.0, key.1));
            }
        }
    }

    /// The one water source a pump resolves inside its data-defined reach.
    ///
    /// Nearest wins, then tile key. The answer names finite standing water by remaining depth and
    /// a river by its replenishing discharge class. Physical sources must be surveyed: a pump may
    /// not draw through fog, and querying one never claims a gameplay chunk.
    pub(crate) fn pump_source_within_reach(
        &self,
        q: i32,
        r: i32,
        radius: i32,
    ) -> Option<WaterSourceSnapshot> {
        hexes_in_radius((q, r), radius)
            .into_iter()
            .filter(|&(cell_q, cell_r)| {
                let size = self.scenario.chunk_size;
                self.generated_chunks
                    .contains(&(floor_div(cell_q, size), floor_div(cell_r, size)))
                    && self.water_depth_at(cell_q, cell_r) > 0
            })
            .min_by_key(|&(cell_q, cell_r)| {
                (axial_distance((q, r), (cell_q, cell_r)), cell_q, cell_r)
            })
            .map(|(cell_q, cell_r)| {
                let generated = self.generated_ground_at(cell_q, cell_r);
                let available = self.water_depth_of(generated, cell_q, cell_r) as u32;
                let discharge = generated.hydrology.discharge_class;
                WaterSourceSnapshot {
                    q: cell_q,
                    r: cell_r,
                    available,
                    discharge,
                    rate: if discharge == 0 {
                        available.min(1)
                    } else {
                        u32::from(discharge)
                    },
                }
            })
    }

    /// Whether open water sits inside the caller's data-defined reach.
    ///
    /// The legacy band fixture keeps its old answer. Every running physical world reads actual
    /// depth, so a drained lake stops a pump and a flooded meadow can site one.
    pub(crate) fn water_within_reach(&self, q: i32, r: i32, radius: i32) -> bool {
        if self.ground_is_physical() {
            self.pump_source_within_reach(q, r, radius).is_some()
        } else {
            hexes_in_radius((q, r), radius)
                .into_iter()
                .any(|(cell_q, cell_r)| self.terrain_at(cell_q, cell_r).is_water())
        }
    }

    /// The deposit an extractor draws from this tick, resolved from its cached candidate list
    /// instead of a scan over every generated tile. `generate_chunk` drops the cache whenever new
    /// tiles appear, so a reference can never outlive the tile set it was resolved against.
    pub(crate) fn extractor_deposit(&mut self, index: usize) -> Option<(i32, i32)> {
        let id = self.entities[index].id;
        if !self.deposit_links.contains_key(&id) {
            let placed = self.entities[index].placed;
            let radius = self.extract_radius_of(placed.definition_id);
            let candidates = self.deposit_candidates(placed.q, placed.r, radius);
            self.deposit_links.insert(id, candidates);
        }
        self.deposit_links[&id]
            .iter()
            .copied()
            .find(|&key| self.extractable_deposit(self.entities[index].placed.definition_id, key))
    }

    /// The material an extractor is working right now, read without touching the cache.
    ///
    /// `extractor_deposit` resolves the same answer but has to be able to populate `deposit_links`,
    /// so it needs `&mut self` and cannot be called from a snapshot. This reads the cache and
    /// answers `None` when it is cold, which is only ever true before the entity's first tick.
    pub(crate) fn extractor_material(&self, index: usize) -> Option<ItemId> {
        self.deposit_links
            .get(&self.entities[index].id)?
            .iter()
            .copied()
            .find(|&key| self.extractable_deposit(self.entities[index].placed.definition_id, key))
            .and_then(|key| self.field_at(key.0, key.1))
            .map(|field| field.item_id)
    }

    /// One extraction cycle in ticks: the material's own figure, scaled by what is digging it.
    ///
    /// Resolved per tick from the deposit actually being worked, so an arm spanning two materials
    /// runs each at its own rate rather than at whichever one it happened to see first. Falls back
    /// to the building's `cadence` when the material names no figure — that is the pump and water.
    pub(crate) fn extract_cycle(
        &self,
        definition_id: DefinitionId,
        item_id: Option<ItemId>,
    ) -> u32 {
        let definition = self.building_definition(definition_id);
        let fallback = definition.and_then(|value| value.cadence).unwrap_or(1);
        let Some(steps) = item_id
            .and_then(|id| self.item_definition(id))
            .and_then(|item| item.extract_steps)
        else {
            return fallback;
        };
        let speed = definition
            .and_then(|value| value.extract_speed)
            .unwrap_or(100)
            .max(1);
        // Rounded up, and never zero: a tier makes a cycle shorter, not free. A zero-length cycle
        // would emit one unit every tick whatever the material said.
        ((steps * 100 + speed - 1) / speed).max(1)
    }

    /// What one full cycle of this entity costs in ticks — a source's cadence, a composer's recipe
    /// duration, and zero for everything that does not run a cycle at all. Published as
    /// `progress_total` so the host draws a proportion it was given, and asked again by `upgrade`,
    /// because a tier may change the cadence under a part-finished job.
    pub(crate) fn progress_total(&self, index: usize) -> u32 {
        let entity = &self.entities[index];
        match entity.kind {
            BuildingKind::Extractor => {
                self.extract_cycle(entity.placed.definition_id, self.extractor_material(index))
            }
            BuildingKind::Pump => self
                .building_definition(entity.placed.definition_id)
                .and_then(|definition| definition.cadence)
                .unwrap_or(1),
            BuildingKind::Composer => entity
                .placed
                .recipe_id
                .and_then(|id| self.recipe(id))
                .map(|recipe| {
                    self.building_definition(entity.placed.definition_id)
                        .map_or(recipe.duration, |definition| {
                            definition.recipe_duration(recipe)
                        })
                })
                .unwrap_or(0),
            _ => 0,
        }
    }

    pub(crate) fn technology(&self, id: TechnologyId) -> Option<&TechnologyDefinition> {
        self.technologies
            .technologies
            .iter()
            .find(|value| value.id == id)
    }

    pub(crate) fn earned_carry_slots(&self) -> u32 {
        let (legacy, _) = research_bonuses(&self.technologies, &self.researched);
        let skills = self.skills.bonuses(&self.technologies);
        let carry_slots = legacy.saturating_add(skills.carry_slots);
        self.scenario
            .carry_slots
            .saturating_add(carry_slots)
            .min(MAX_CARRY_SLOTS)
    }

    pub(crate) fn earned_build_range(&self) -> u32 {
        let (_, legacy) = research_bonuses(&self.technologies, &self.researched);
        let skills = self.skills.bonuses(&self.technologies);
        let build_range = legacy.saturating_add(skills.build_range);
        self.scenario
            .build_range
            .saturating_add(build_range)
            .saturating_mul(HEX_X as u32)
    }

    /// How far the world opens around a hex the player reaches, in rings of chunks.
    ///
    /// Derived rather than stored, under the rule every other derived value follows: the skills
    /// that widen it are already saved and validated, so a saved copy of this could only ever
    /// disagree with them. It is also not a `PlayerState` field for a second reason — the surveyed
    /// world lives in `generated_chunks`, which is a checksum input, and a stored radius would be a
    /// second, unhashed account of the same thing.
    pub(crate) fn survey_rings(&self) -> u32 {
        BASE_SURVEY_RINGS
            .saturating_add(self.skills.bonuses(&self.technologies).survey_rings)
            .min(BASE_SURVEY_RINGS + MAX_SURVEY_RING_BONUS)
    }

    /// Levels owned in the mobility ladder. Each level multiplies pace by 5/4.
    ///
    /// Derived rather than stored, for [`Core::survey_rings`]'s reason: the skills that raise it
    /// are already saved and validated, so a saved copy could only ever disagree with them. It is
    /// also not written back into [`PlayerState`] by [`Core::apply_research_effects`] — unlike the
    /// pack, which creative mode may have widened past what was earned, there is no editor for
    /// pace and so nothing a floor would have to protect.
    pub(crate) fn move_speed_level(&self) -> u32 {
        self.skills
            .bonuses(&self.technologies)
            .move_speed_levels
            .min(MAX_MOVE_SPEED_LEVEL)
    }

    /// Apply the mobility ladder to one surface pace without compounding integer rounding.
    pub(crate) fn apply_move_speed(&self, speed: i32) -> i32 {
        let level = self.move_speed_level();
        let numerator = 125_i64.pow(level);
        let denominator = 100_i64.pow(level);
        (i64::from(speed) * numerator / denominator) as i32
    }

    /// Apply earned skills through the same native player fields placement and carrying use.
    /// Pack size is a floor because creative mode may have widened it further; build range has no
    /// separate editor and is therefore exactly the researched value. Survey range is not here at
    /// all: `survey_rings` reads the skills directly, so there is nothing to write back.
    pub(crate) fn apply_research_effects(&mut self) {
        self.player.carry_slots = self.player.carry_slots.max(self.earned_carry_slots());
        self.player.build_range = self.earned_build_range();
    }
}
