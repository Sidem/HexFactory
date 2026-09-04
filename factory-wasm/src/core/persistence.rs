//! persistence — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Core {
    pub(crate) fn checksum_for_world(&self, world_version: u16) -> u32 {
        let mut hash = 0x811c9dc5u32;
        hash_bytes(&mut hash, self.scenario.key.as_bytes());
        hash_u32(&mut hash, u32::from(world_version));
        hash_u32(&mut hash, self.seed);
        hash_world_params(&mut hash, &self.world_params);
        hash_u64(&mut hash, self.tick);
        hash_u64(&mut hash, self.delivered);
        hash_u64(&mut hash, self.insight);
        hash_u32(&mut hash, u32::from(self.victory));
        hash_i32(&mut hash, self.player.x);
        hash_i32(&mut hash, self.player.y);
        hash_i32(&mut hash, i32::from(self.player.facing_x));
        hash_i32(&mut hash, i32::from(self.player.facing_y));
        hash_i32(&mut hash, i32::from(self.player.move_x));
        hash_i32(&mut hash, i32::from(self.player.move_y));
        hash_u32(&mut hash, self.player.action_cooldown);
        // The swing that counter is measuring, so the two cannot be separated by an edit or by a
        // save. An idle player hashes nothing here, which is what keeps a file written before the
        // harvest became work — where no swing could be in flight — checksumming to the same value
        // it did then.
        if let Some(target) = self.pending_gather {
            hash_i32(&mut hash, target.q);
            hash_i32(&mut hash, target.r);
        }
        if let Some(edit) = &self.pending_ground {
            hash_u32(&mut hash, u32::MAX - 35);
            edit.hash_into(&mut hash);
        }
        // Where a walk is headed, on the same terms as the swing above: it is an order the
        // simulation is still executing, so a run carrying one is not the same run as one standing
        // still, and a player who is not walking hashes nothing here. The route itself is derived
        // and is deliberately absent — it is rebuilt from this goal, so hashing it would be hashing
        // the same fact twice and pinning the search's internals into the save format.
        if let Some(goal) = self.player.walk_goal {
            hash_i32(&mut hash, goal.q);
            hash_i32(&mut hash, goal.r);
        }
        // Both of these are now run state rather than scenario state: creative changes what a
        // construction costs, and creative can widen the pack. A save that carried either without
        // hashing it could come back describing a different run than the one that was saved.
        hash_u32(&mut hash, self.player.carry_slots);
        hash_u32(&mut hash, u32::from(self.creative));
        for (&item, &quantity) in &self.player.inventory {
            hash_u32(&mut hash, u32::from(item));
            hash_u32(&mut hash, quantity);
        }
        if let Some(hand) = self.player.hand {
            hash_u32(&mut hash, u32::MAX - 20);
            hash_u32(&mut hash, u32::from(hand.item_id));
            hash_u32(&mut hash, hand.quantity);
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
            hash_u32(&mut hash, entity.fuel_charge);
            hash_u32(&mut hash, entity.power_charge);
            hash_u32(&mut hash, entity.burn_progress);
            hash_u32(&mut hash, u32::from(entity.disabled));
            // Where each junction is in its rotation. A factory that reloaded with these reset
            // would deal its next round differently from the one that was saved, so they are as
            // much of the run's state as a machine's progress is.
            hash_u32(&mut hash, u32::from(entity.route_cursor));
            hash_u32(&mut hash, entity.merge_cursor);
            hash_inventory(&mut hash, &entity.inventory);
            hash_inventory(&mut hash, &entity.reserved_inputs);
            if !entity.input_inventory.is_empty() {
                hash_u32(&mut hash, u32::MAX - 21);
                hash_inventory(&mut hash, &entity.input_inventory);
            }
            if !entity.fuel_inventory.is_empty() {
                hash_u32(&mut hash, u32::MAX - 22);
                hash_inventory(&mut hash, &entity.fuel_inventory);
            }
            if !entity.output_inventory.is_empty() {
                hash_u32(&mut hash, u32::MAX - 23);
                hash_inventory(&mut hash, &entity.output_inventory);
            }
            if let Some(cargo) = entity.cargo {
                hash_u32(&mut hash, u32::from(cargo.item_id));
                hash_u32(&mut hash, cargo.quantity);
            } else {
                hash_u32(&mut hash, 0);
            }
            // What a belt is still carrying, and how far along it each item has got. Two factories
            // that agree about the exit slots and disagree about the four items behind them are not
            // the same factory, and they will not stay in step for a second: the tick each item
            // stepped on is what decides when it arrives. Written only when there is a lane, so
            // every checksum in the game that has no belt in flight is the one it always was.
            if !entity.lane.is_empty() {
                hash_u32(&mut hash, u32::MAX - 24);
                for item in &entity.lane {
                    hash_u32(&mut hash, u32::from(item.cargo.item_id));
                    hash_u32(&mut hash, item.cargo.quantity);
                    hash_u64(&mut hash, item.entered);
                }
            }
        }
        if !self.output_routes.is_empty() {
            hash_u32(&mut hash, u32::MAX - 31);
            for (&entity_id, routes) in &self.output_routes {
                hash_u32(&mut hash, entity_id);
                for (&item_id, route) in routes {
                    hash_u32(&mut hash, u32::from(item_id));
                    hash_i32(&mut hash, route.q);
                    hash_i32(&mut hash, route.r);
                    hash_u32(&mut hash, u32::from(route.direction));
                }
                hash_u32(&mut hash, u32::MAX);
            }
        }
        if !self.legacy_fluid_belts.is_empty() {
            hash_u32(&mut hash, u32::MAX - 32);
            for &entity_id in &self.legacy_fluid_belts {
                hash_u32(&mut hash, entity_id);
            }
        }
        for (&item, &quantity) in &self.delivered_by_item {
            hash_u32(&mut hash, u32::from(item));
            hash_u64(&mut hash, quantity);
        }
        hash_u32(&mut hash, u32::MAX - 2);
        hash_u64(&mut hash, self.contract_stage as u64);
        for (&item, &quantity) in &self.contract_contributed {
            hash_u32(&mut hash, u32::from(item));
            hash_u64(&mut hash, quantity);
        }
        hash_u32(&mut hash, u32::MAX - 3);
        for state in &self.requests {
            hash_u32(&mut hash, u32::from(state.request_id));
        }
        hash_u32(&mut hash, u32::MAX - 25);
        for (&request, &delivered) in &self.request_delivered {
            hash_u32(&mut hash, u32::from(request));
            hash_u32(&mut hash, delivered);
        }
        hash_u32(&mut hash, u32::MAX - 4);
        for (&request, &rounds) in &self.request_rounds {
            hash_u32(&mut hash, u32::from(request));
            hash_u32(&mut hash, rounds);
        }
        hash_u32(&mut hash, u32::MAX - 5);
        for (&request, &fills) in &self.request_fills {
            hash_u32(&mut hash, u32::from(request));
            hash_u32(&mut hash, fills);
        }
        if !self.ground_items.is_empty() {
            hash_u32(&mut hash, u32::MAX - 24);
            for item in &self.ground_items {
                hash_u32(&mut hash, item.id);
                hash_i32(&mut hash, item.q);
                hash_i32(&mut hash, item.r);
                hash_u32(&mut hash, u32::from(item.item_id));
                hash_u32(&mut hash, item.quantity);
                hash_u64(&mut hash, item.despawn_tick);
            }
        }
        if !self.boundaries.is_empty() {
            hash_u32(&mut hash, u32::MAX - 29);
            hash_u32(&mut hash, self.boundary_state_hash());
        }
        // Guarded on emptiness for the same reason: a run that has never touched the ground hashes
        // exactly what it hashed a release ago, so v0.37 files keep their checksums.
        if !self.ground.is_empty() || self.spoil != 0 {
            hash_u32(&mut hash, u32::MAX - 30);
            hash_u32(&mut hash, self.ground_state_hash());
        }
        // Guarded on the same rule once more. Disturbed water is the departure set and nothing else,
        // so a world at its generated equilibrium hashes what it hashed before this field existed,
        // and every save 38 file keeps the checksum it was written with. The envelope moves when a
        // player can first create a departure, not when native learns to carry one.
        if !self.water.is_empty() {
            hash_u32(&mut hash, u32::MAX - 33);
            self.water.hash_into(&mut hash);
        }
        if !self.bank_stress.is_empty() {
            hash_u32(&mut hash, u32::MAX - 34);
            self.bank_stress.hash_into(&mut hash);
        }
        self.skills.hash(&mut hash);
        hash
    }

    pub(crate) fn save_string(&self) -> Result<String, String> {
        let state = SavedState {
            seed: self.seed,
            world_params: self.world_params.clone(),
            generated_chunks: self
                .generated_chunks
                .iter()
                .map(|&(q, r)| Coordinate { q, r })
                .collect(),
            tiles: self.tiles.values().cloned().collect(),
            entities: self.entities.clone(),
            output_routes: self.output_routes.clone(),
            legacy_fluid_belts: self.legacy_fluid_belts.clone(),
            player: self.player.clone(),
            pending_gather: self.pending_gather,
            pending_ground: self.pending_ground.clone(),
            researched: self.researched.clone(),
            skills: self.skills.clone(),
            next_entity_id: self.next_entity_id,
            tick: self.tick,
            delivered: self.delivered,
            delivered_by_item: self.delivered_by_item.clone(),
            insight: self.insight,
            victory: self.victory,
            contract_stage: self.contract_stage,
            contract_contributed: self.contract_contributed.clone(),
            requests: self.requests.clone(),
            request_rounds: self.request_rounds.clone(),
            request_fills: self.request_fills.clone(),
            request_delivered: self.request_delivered.clone(),
            produced: self.produced.clone(),
            creative: self.creative,
            boundaries: self.boundary_snapshot(),
            ground: self.ground_snapshot(),
            water: self.water.cells(),
            bank_stress: self.bank_stress.cells(),
            spoil: self.spoil,
            ground_items: self.ground_items.clone(),
            next_ground_item_id: self.next_ground_item_id,
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

    pub(crate) fn from_save(
        definitions: &DefinitionsInput,
        technologies: &TechnologiesInput,
        scenarios: &ScenariosInput,
        save: &str,
    ) -> Result<Self, String> {
        let json = save
            .strip_prefix(SAVE_PREFIX)
            .ok_or("save must begin with HXF1")?;
        // Verify the original world stamp before moving a legacy run onto the current envelope.
        // Its saved site table is unchanged: adding oil must not reroll an existing landscape.
        let original: serde_json::Value =
            serde_json::from_str(json).map_err(|error| error.to_string())?;
        let original_world = original
            .get("world_generator_version")
            .and_then(serde_json::Value::as_u64)
            .and_then(|version| u16::try_from(version).ok())
            .ok_or("save has no valid world version")?;
        let original_save_version = original
            .get("save_version")
            .and_then(serde_json::Value::as_u64)
            .and_then(|version| u16::try_from(version).ok())
            .ok_or("save has no valid save version")?;
        if original_save_version <= 36 && SAVE_VERSION >= 37 {
            return Err(
                "this factory was built at one square metre per hex; export the file to keep a copy. New worlds use a 25 m² hex"
                    .into(),
            );
        }
        let migrated = save_migrations::migrate(json, SAVE_VERSION)?;
        // Only saves from before the one-component founding bill need that state repair. Newer
        // adjacent migrations (including 44 -> 45's pending-ground field) must not replay it just
        // because the JSON envelope was rewritten.
        let legacy_component_bill = original_save_version < 33;
        let envelope: SaveEnvelope = serde_json::from_str(&migrated)
            .map_err(|error| format!("malformed HXF1 save: {error}"))?;
        if envelope.world_generator_version != WORLD_GENERATOR_VERSION {
            return Err(
                "this factory stands on a world this build no longer generates; export the file to keep a copy"
                    .into(),
            );
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
        let mut core = Core::initialize(
            definitions,
            technologies,
            scenario,
            Some(envelope.state.seed),
            Some(envelope.state.world_params.clone()),
            false,
        )?;
        validate_saved_state(
            definitions,
            technologies,
            scenario,
            &envelope.state,
            legacy_component_bill,
        )?;
        let restored_legacy_fluid_belts = envelope.state.legacy_fluid_belts.clone();
        core.seed = envelope.state.seed;
        core.world_params = envelope.state.world_params;
        // The lattice and the bootstrap table are derived from exactly these two, so they are
        // rebuilt the moment either moves rather than carried in the file.
        core.fields = WorldFields::new(&core.world_params, core.seed, &core.ground_spine);
        core.generated_chunks = envelope
            .state
            .generated_chunks
            .iter()
            .map(|coordinate| (coordinate.q, coordinate.r))
            .collect();
        core.ground_spine
            .rebuild_cache(&core.generated_chunks, core.scenario.chunk_size);
        core.tiles = envelope
            .state
            .tiles
            .into_iter()
            .map(|tile| ((tile.q, tile.r), tile))
            .collect();
        core.deposit_links.clear();
        // Regrowth is derived from the overlay the save just restored, so it is recovered here
        // rather than carried in the file.
        core.rebuild_flora_regrowth();
        // Undo history is session state, not saved state: a restored save has nothing to take back.
        core.undo_stack.clear();
        core.entities = envelope.state.entities;
        core.output_routes = envelope.state.output_routes;
        if original_save_version >= SAVE_VERSION {
            core.legacy_fluid_belts = restored_legacy_fluid_belts.clone();
        }
        // A save records entities in stable id order; sorting makes that an invariant of the loaded
        // core rather than a property of the file. Entity order is not a simulation input — the
        // checksum and every arbitration order sort by id — so this cannot change a result.
        core.entities.sort_by_key(|entity| entity.id);
        core.player = envelope.state.player;
        core.pending_gather = envelope.state.pending_gather;
        core.pending_ground = envelope.state.pending_ground;
        core.researched = envelope.state.researched;
        core.skills = envelope.state.skills;
        // Restored directly rather than through set_creative: the saved researched set is the
        // checksum truth. A migrated creative save is upgraded only after that original truth has
        // been verified below.
        core.creative = envelope.state.creative;
        core.next_entity_id = envelope.state.next_entity_id;
        core.tick = envelope.state.tick;
        core.delivered = envelope.state.delivered;
        core.delivered_by_item = envelope.state.delivered_by_item;
        core.insight = envelope.state.insight;
        core.victory = envelope.state.victory;
        core.contract_stage = envelope.state.contract_stage;
        core.contract_contributed = envelope.state.contract_contributed;
        // The board is restored, never redrawn: `Core::new` posted one for a fresh run and this run
        // is not fresh. A redraw would hand a finished game three requests it may already have
        // filled, and the checksum below would be the first thing to say so.
        core.requests = envelope.state.requests;
        core.request_rounds = envelope.state.request_rounds;
        core.request_fills = envelope.state.request_fills;
        core.request_delivered = envelope.state.request_delivered;
        core.last_action_cooldown_total = core.player.action_cooldown;
        core.produced = envelope.state.produced;
        core.boundaries = envelope
            .state
            .boundaries
            .into_iter()
            .map(|b| (b.segment, b))
            .collect();
        core.ground = envelope
            .state
            .ground
            .into_iter()
            .map(|cell| ((cell.q, cell.r), cell))
            .collect();
        core.water = hydrology::DisturbedWater::from_cells(&envelope.state.water);
        core.bank_stress = geomorphology::BankStress::from_cells(&envelope.state.bank_stress);
        core.spoil = envelope.state.spoil;
        core.ground_items = envelope.state.ground_items;
        core.next_ground_item_id = envelope.state.next_ground_item_id.max(
            core.ground_items
                .iter()
                .map(|item| item.id.saturating_add(1))
                .max()
                .unwrap_or(1),
        );
        core.events = vec!["HXF1 save restored".into()];
        if core.checksum_for_world(original_world) != envelope.checksum {
            return Err("save checksum does not match its native state".into());
        }
        // A v35 checksum knew nothing about this compatibility set. Apply it only after that
        // original state has passed tamper detection; the next v36 save hashes the new fact.
        if original_save_version < SAVE_VERSION {
            core.legacy_fluid_belts = restored_legacy_fluid_belts;
        }
        // Verify saved facts before rebuilding derived topology and route caches.
        core.compile_graph();
        // v0.33 asks for one component instead of three. Honor existing contributions only after
        // verifying their saved checksum, through the ordinary consumption/grant path. Completed
        // commissions are never replayed and any surplus stays credited at the hub.
        if legacy_component_bill && core.scenario.key == "new-game" {
            core.advance_contract_with_rewards(false);
        }
        if legacy_component_bill {
            core.migrate_player_skills();
        }
        // Creative means the whole current tree, including technologies added after the save was
        // written. Verify the saved state first, then extend it through the ordinary capability
        // path; this preserves tamper detection without leaving an older creative world partially
        // locked. A current save already containing the whole tree is unchanged.
        if core.creative {
            core.grant_creative_skills();
            for technology in &core.technologies.technologies {
                core.researched.insert(technology.id);
            }
            core.apply_research_effects();
        }
        core.player.move_x = 0;
        core.player.move_y = 0;
        Ok(core)
    }
}
