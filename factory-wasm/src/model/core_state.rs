struct Core {
    boundaries: BTreeMap<Segment, Boundary>,
    boundary_hash_cache: RefCell<Option<u32>>,
    boundary_undo: Vec<BoundaryUndo>,
    /// Prepared ground: surface and graded elevation, sparse over the untreated world.
    ground: BTreeMap<(i32, i32), GroundCell>,
    /// Memoized digest of `ground` and `spoil`. Derived state under the same rule as
    /// `boundary_hash_cache`: never saved, never hashed, and the uncached walk is its oracle.
    ground_hash_cache: RefCell<Option<u32>>,
    ground_undo: Vec<GroundUndo>,
    /// Water that has left the depth the generator publishes, and only that. An untouched world
    /// carries none: the ocean, the lakes and the rivers are answers `terra` computes, not saved
    /// entities. See `hydrology` for why departure rather than depth is the thing stored.
    water: hydrology::DisturbedWater,
    /// Outside-bank stress accumulated only at coarse geomorphic epochs. Sparse saved state; a
    /// straight or untouched world carries none.
    bank_stress: geomorphology::BankStress,
    /// Rated river withdrawals already granted this tick, by source cell. Derived tick-local
    /// arbitration: cleared before each machine pass, never saved or checksummed.
    water_draws: BTreeMap<(i32, i32), u32>,
    /// Excavated material held for fill, in whole steps of one hex.
    ///
    /// Cut adds, fill spends, and nothing else touches it. Making raising ground *cost* something
    /// that can only come from lowering ground is what stops levelling being an infinite source of
    /// terrain, on the same rule that closed the insight loop in v0.35.0.
    spoil: u64,
    definitions: DefinitionsInput,
    technologies: TechnologiesInput,
    scenario: ScenarioDefinition,
    seed: u32,
    /// What the world is generated from. Saved and checksummed beside the seed, because the two
    /// answer the same question together and neither answers it alone.
    world_params: WorldParams,
    /// The resource field derived from `world_params` and `seed`, with its site lattice and its
    /// bootstrap table cached. Derived state under the same rule as `deposit_links`: never saved,
    /// never hashed, never checksummed, and rebuilt whenever the world it is derived from changes.
    fields: WorldFields,
    /// Generated bed, substrate and initial hydrology behind the current presentation. Derived
    /// from the same world identity as `fields`, cached only for surveyed chunks, and never saved,
    /// hashed or checksummed. The uncached source is its oracle.
    ground_spine: GroundSpine,
    generated_chunks: BTreeSet<(i32, i32)>,
    tiles: BTreeMap<(i32, i32), TileState>,
    /// Deposit references resolved per extractor entity id, so a running extractor never scans the
    /// tile map. Derived cache only: it is rebuilt from tiles on demand and never saved or hashed.
    deposit_links: BTreeMap<u32, Vec<(i32, i32)>>,
    /// The scenario's hand-placed resources, keyed by tile. `field_at` is asked once per hex of
    /// every surveyed chunk when a complete snapshot is built, so scanning the scenario's list for
    /// each of them made that snapshot O(hexes × placed resources) — 3.9× slower at the largest
    /// measured tier, which places one per line. Derived from the scenario definition, so it is
    /// never saved, hashed, or checksummed.
    scenario_resources: BTreeMap<(i32, i32), ResourceState>,
    /// Flora cells standing below the quantity generation gave them. Regrowth walks this set
    /// rather than the world, so a forest costs nothing until somebody cuts it and nothing again
    /// once it has grown back. Derived state under the same rule as `deposit_links`: it is a pure
    /// function of the overlay and the item definitions, so it is rebuilt on load rather than
    /// saved, and it is never hashed or checksummed.
    flora_regrowth: BTreeSet<(i32, i32)>,
    entities: Vec<Entity>,
    /// Per-entity, per-product outlet choices keyed by stable entity id. Empty means the legacy
    /// facing outlet for every product. Real saved state; compiled graph edges remain derived.
    output_routes: BTreeMap<u32, BTreeMap<ItemId, OutputRoute>>,
    /// Stable ids of belt-kind entities created before fluid transport existed. They retain the
    /// old accept-any-cargo behavior so a migrated factory keeps running; no new placement enters
    /// this set. Saved and checksummed because it changes transfer eligibility.
    legacy_fluid_belts: BTreeSet<u32>,
    ground_items: Vec<GroundItem>,
    next_ground_item_id: u32,
    graph: Vec<Links>,
    /// Stable hot-path orders and reverse transport edges derived from `entities` and `graph`.
    /// Rebuilt after edits and loads; never saved, hashed, or checksummed.
    runtime: RuntimeIndex,
    /// Per-entity power network id (`None` = not on a network). Derived like `graph`.
    power_of: Vec<Option<u32>>,
    /// Last tick's supply and demand per network id.
    power_supply: BTreeMap<u32, u32>,
    power_demand: BTreeMap<u32, u32>,
    /// Capacity harness only: consumers run at full speed so the ladder still measures transport.
    power_unmetered: bool,
    player: PlayerState,
    /// The field hex the player is currently working, while a swing is in flight.
    ///
    /// A harvest is work, and work takes time *before* it pays. `action_cooldown` used to be a wait
    /// imposed after an instant take, which handed the player the first unit of every material the
    /// moment they pressed the button and only then made them wait — the one gather in a session
    /// that was free was the first one. The counter now measures the swing that is still running
    /// and this is the hex it will land on, so the ring the host already draws is progress toward a
    /// unit rather than a debt against one already banked.
    ///
    /// Saved and checksummed beside `action_cooldown`, because the two are one fact: a save that
    /// carried the remaining work without what it is working on would come back counting down to
    /// nothing.
    pending_gather: Option<Coordinate>,
    /// Earthwork command whose resolved volume the player clock is still paying for.
    /// Saved and checksummed with `action_cooldown`; nothing changes until the counter reaches zero.
    pending_ground: Option<GroundEdit>,
    /// The hexes still ahead of the player on the current walk, nearest first, ending on
    /// `player.walk_goal`. Derived state under the same rule as `deposit_links`: it is a pure
    /// function of the goal, the terrain, and the occupied cells, so it is rebuilt whenever the
    /// topology changes and on load, and it is never saved, hashed, or checksummed.
    walk_path: Vec<Coordinate>,
    /// Player-clock steps the current walk has made no ground. Derived session state: a walk that
    /// reloads mid-stall simply gets its second to prove itself again.
    walk_stall: u32,
    /// Where the player stood at the top of the last walk step, so `walk_stall` measures ground
    /// actually covered rather than intent issued. Derived like `walk_stall`.
    walk_last_position: (i32, i32),
    /// Whether this run builds for free with everything unlocked. Saved and checksummed: it changes
    /// what a construction costs and what an erase gives back, so two runs that differ only in this
    /// are not the same run, and a save that lost it would come back priced.
    ///
    /// It is deliberately narrow. Creative changes what the *player* may spend and carry; it does
    /// not touch power, recipe timing, belt throughput, machine behaviour, or what the hub pays. A
    /// factory built in creative runs exactly as one built in a priced run does, which is the whole
    /// point of testing in it.
    creative: bool,
    researched: BTreeSet<TechnologyId>,
    skills: SkillsState,
    next_entity_id: u32,
    tick: u64,
    delivered: u64,
    delivered_by_item: BTreeMap<ItemId, u64>,
    insight: u64,
    victory: bool,
    /// How many contract stages the hub has finished. Saved and checksummed: it is the state a
    /// founding project consists of, and the host draws the hub's growth from it.
    contract_stage: usize,
    /// What the hub has been given since the contract started, less what completed stages consumed.
    /// Every hub delivery lands here, not only the items the current stage names, so a player who
    /// automates a line early is credited for it when the stage that wants it arrives.
    contract_contributed: BTreeMap<ItemId, u64>,
    /// The requests the hub has posted, in slot order. Saved and checksummed: which standing orders
    /// are open, and how far each one has been filled, is as much a run's progress as the contract
    /// stage is, and it is the only thing that pays insight.
    requests: Vec<RequestState>,
    /// How many times each request has left the board — filled or passed on. It is also the draw
    /// order: the least-used eligible row is posted first, so fresh content leads and old standing
    /// orders come round again once there is nothing new left to post.
    request_rounds: BTreeMap<RequestId, u32>,
    /// How many times each request has been *paid*. Skip increments `request_rounds` so the row
    /// goes behind unseen content; it must not retire the project, so fills are counted apart.
    /// Saved and checksummed: a project with a fill against it is finished for this run and is
    /// never posted again.
    request_fills: BTreeMap<RequestId, u32>,
    /// How much has been handed over against each project so far, whether or not it is posted now.
    /// Saved and checksummed: under finite demand this is a run's unfinished work, and losing it on
    /// a pass would destroy goods the player cannot re-earn the reward for.
    request_delivered: BTreeMap<RequestId, u32>,
    produced: BTreeMap<ItemId, u64>,
    /// What the current (or last) swing was worth when it started. Snapshot-only: the host draws
    /// the work still outstanding against this, and a save mid-gather republishes the remaining
    /// count so the ring resumes where it stood. Never saved, hashed, or checksummed.
    last_action_cooldown_total: u32,
    events: Vec<String>,
    /// Derived presentation state: what has changed since the host's last delta. Never saved,
    /// hashed, or checksummed.
    dirty: SnapshotDirty,
    /// Ids of entities this session constructed, most recent last, so one misplacement can be taken
    /// back. Derived session state under the same rule as `deposit_links` and `dirty`: never saved,
    /// hashed, or checksummed, so a loaded save starts with nothing to undo. Undo runs the ordinary
    /// erase path, which is why it cannot invent a refund the erase tests do not already pin.
    undo_stack: Vec<u32>,
}

impl Core {
    fn new(
        definitions: &DefinitionsInput,
        technologies: &TechnologiesInput,
        scenario: &ScenarioDefinition,
        seed_override: Option<u32>,
        world_params: Option<WorldParams>,
    ) -> Result<Self, String> {
        Self::initialize(
            definitions,
            technologies,
            scenario,
            seed_override,
            world_params,
            true,
        )
    }

    /// Saved worlds validate their stored state, not a newer release's opening promises.
    fn initialize(
        definitions: &DefinitionsInput,
        technologies: &TechnologiesInput,
        scenario: &ScenarioDefinition,
        seed_override: Option<u32>,
        world_params: Option<WorldParams>,
        require_opening: bool,
    ) -> Result<Self, String> {
        let seed = seed_override.unwrap_or(scenario.seed);
        let world_params = match world_params {
            Some(params) => params,
            None => scenario
                .world_preset
                .as_deref()
                .map(|key| preset_params(key).ok_or_else(|| format!("unknown world preset {key}")))
                .transpose()?
                .unwrap_or_else(default_world_params),
        };
        world_params.validate(definitions)?;
        let ground_spine =
            GroundSpine::physical(&world_params, seed, scenario.generated_environment);
        let fields = WorldFields::new(&world_params, seed, &ground_spine);
        // A world whose opening cannot be placed is refused here rather than papered over. It is
        // the one generator failure a validator cannot see — `validate` is asked before a seed
        // exists — and shipping it would mean a run that cannot reach its own first extractor.
        if require_opening && scenario.generated_environment {
            if let Some(&(item_id, gave_up_at)) = fields.unmet.first() {
                return Err(format!(
                    "this world guarantees no item {item_id} within {gave_up_at} hexes of the \
                     landing site"
                ));
            }
        }
        let mut inventory = BTreeMap::new();
        add_ingredients(&mut inventory, &scenario.initial_inventory);
        let mut core = Self {
            definitions: definitions.clone(),
            technologies: technologies.clone(),
            scenario: scenario.clone(),
            seed,
            world_params,
            fields,
            ground_spine,
            boundaries: BTreeMap::new(),
            boundary_hash_cache: RefCell::new(None),
            boundary_undo: Vec::new(),
            ground: BTreeMap::new(),
            ground_hash_cache: RefCell::new(None),
            ground_undo: Vec::new(),
            water: hydrology::DisturbedWater::new(),
            bank_stress: geomorphology::BankStress::new(),
            water_draws: BTreeMap::new(),
            spoil: 0,
            generated_chunks: BTreeSet::new(),
            tiles: BTreeMap::new(),
            deposit_links: BTreeMap::new(),
            scenario_resources: scenario
                .resources
                .iter()
                .map(|resource| {
                    (
                        (resource.q, resource.r),
                        ResourceState {
                            item_id: resource.item_id,
                            quantity: resource.quantity,
                            initial_quantity: resource.quantity,
                        },
                    )
                })
                .collect(),
            flora_regrowth: BTreeSet::new(),
            entities: Vec::new(),
            output_routes: BTreeMap::new(),
            legacy_fluid_belts: BTreeSet::new(),
            ground_items: Vec::new(),
            next_ground_item_id: 1,
            graph: Vec::new(),
            runtime: RuntimeIndex::default(),
            power_of: Vec::new(),
            power_supply: BTreeMap::new(),
            power_demand: BTreeMap::new(),
            power_unmetered: false,
            player: PlayerState {
                x: axial_world(scenario.player_spawn.q, scenario.player_spawn.r).0,
                y: axial_world(scenario.player_spawn.q, scenario.player_spawn.r).1,
                facing_x: world_direction(scenario.player_facing).0,
                facing_y: world_direction(scenario.player_facing).1,
                move_x: 0,
                move_y: 0,
                inventory,
                hand: None,
                action_cooldown: 0,
                build_range: scenario.build_range.saturating_mul(HEX_X as u32),
                carry_slots: scenario.carry_slots,
                walk_goal: None,
            },
            pending_gather: None,
            pending_ground: None,
            walk_path: Vec::new(),
            walk_stall: 0,
            walk_last_position: (0, 0),
            creative: false,
            researched: scenario.initial_researched.iter().copied().collect(),
            skills: SkillsState::default(),
            next_entity_id: 1,
            tick: 0,
            delivered: 0,
            delivered_by_item: BTreeMap::new(),
            insight: 0,
            victory: false,
            contract_stage: 0,
            contract_contributed: BTreeMap::new(),
            requests: Vec::new(),
            request_rounds: BTreeMap::new(),
            request_fills: BTreeMap::new(),
            request_delivered: BTreeMap::new(),
            produced: BTreeMap::new(),
            last_action_cooldown_total: 0,
            events: vec![format!("{} ready", scenario.name)],
            dirty: SnapshotDirty::default(),
            undo_stack: Vec::new(),
        };
        core.apply_research_effects();
        core.ensure_neighborhood(core.player.x, core.player.y);
        for resource in &scenario.resources {
            core.ensure_tile(resource.q, resource.r);
            core.write_overlay(
                resource.q,
                resource.r,
                resource.item_id,
                resource.quantity,
                resource.quantity,
            );
        }
        let mut buildings = scenario.buildings.clone();
        buildings.sort_by_key(placed_sort_key);
        for placed in buildings {
            core.ensure_tile(placed.q, placed.r);
            let manual_work = core
                .building_definition(placed.definition_id)
                .is_some_and(|definition| definition.manual_work);
            let kind = core
                .building_definition(placed.definition_id)
                .ok_or_else(|| format!("unknown building definition {}", placed.definition_id))?
                .kind;
            core.entities.push(Entity {
                id: core.next_entity_id,
                placed,
                kind,
                cargo: None,
                inventory: BTreeMap::new(),
                input_inventory: BTreeMap::new(),
                fuel_inventory: BTreeMap::new(),
                output_inventory: BTreeMap::new(),
                reserved_inputs: BTreeMap::new(),
                progress: 0,
                fuel_charge: 0,
                power_charge: 0,
                burn_progress: 0,
                disabled: manual_work,
                route_cursor: 0,
                merge_cursor: 0,
                lane: Vec::new(),
            });
            core.next_entity_id += 1;
        }
        core.compile_graph();
        core.refill_requests();
        Ok(core)
    }

    // `advance_power_plants` used to live here: one pass that burned a unit of fuel per plant per
    // tick whenever its network had any demand at all, so one extractor cost a burner exactly what
    // five composers did. Its work is now `burn_for_output`, charged against energy the grid
    // actually delivered, which is why there is no longer a separate plant phase in the tick.

}
