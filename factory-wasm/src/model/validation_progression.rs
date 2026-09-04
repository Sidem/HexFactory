fn validate_technologies(
    definitions: &DefinitionsInput,
    technologies: &TechnologiesInput,
) -> Result<(), String> {
    if technologies.version == 0 {
        return Err("technology version must be positive".into());
    }
    for (label, groups) in [
        ("branch", &technologies.branches),
        ("stage", &technologies.stages),
    ] {
        if groups.is_empty() || groups.len() > 64 {
            return Err(format!(
                "technology {label} registry requires 1 to 64 entries"
            ));
        }
        let mut keys = BTreeSet::new();
        for group in groups {
            // `order` is a u32 on both sides. Equal orders are valid; key is the stable tie-breaker.
            let _order = group.order;
            if !group
                .key
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_lowercase)
                || !group
                    .key
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                || group.name.trim().is_empty()
                || group.description.trim().is_empty()
                || !keys.insert(&group.key)
            {
                return Err(format!(
                    "technology {label} registry has an invalid or duplicate entry"
                ));
            }
        }
    }
    if technologies.technologies.len() > 1024 {
        return Err("technology catalog exceeds 1024 entries".into());
    }
    let branches: BTreeSet<_> = technologies
        .branches
        .iter()
        .map(|group| &group.key)
        .collect();
    let stages: BTreeSet<_> = technologies.stages.iter().map(|group| &group.key).collect();
    let mut keys = BTreeSet::new();
    unique_positive_ids(
        technologies
            .technologies
            .iter()
            .map(|technology| technology.id),
        "technology",
    )?;
    let ids: BTreeSet<_> = technologies
        .technologies
        .iter()
        .map(|value| value.id)
        .collect();
    let building_ids: BTreeSet<_> = definitions.buildings.iter().map(|value| value.id).collect();
    let boundary_ids: BTreeSet<_> = definitions
        .boundaries
        .iter()
        .map(|value| value.id)
        .collect();
    for technology in &technologies.technologies {
        if technology.key.trim().is_empty()
            || technology.name.trim().is_empty()
            || technology.description.trim().is_empty()
            || !keys.insert(&technology.key)
            || !branches.contains(&technology.branch)
            || !stages.contains(&technology.stage)
            || technology
                .prerequisites
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != technology.prerequisites.len()
            || !valid_technology_grant(technology)
            || !valid_technology_effects(
                technology,
                &building_ids,
                &boundary_ids,
                &definitions
                    .surfaces
                    .iter()
                    .map(|surface| surface.id)
                    .collect(),
            )
        {
            return Err(format!("technology {} is incomplete", technology.id));
        }
        if technology.prerequisites.iter().any(|id| !ids.contains(id)) {
            return Err(format!(
                "technology {} has an unknown prerequisite",
                technology.id
            ));
        }
    }
    for building in &definitions.buildings {
        if let Some(id) = building.unlock_technology_id {
            if !ids.contains(&id) {
                return Err(format!(
                    "building {} has an unknown unlock requirement",
                    building.id
                ));
            }
        }
    }
    for boundary in &definitions.boundaries {
        if let Some(id) = boundary.unlock_technology_id {
            if !ids.contains(&id) {
                return Err(format!(
                    "boundary {} has an unknown unlock requirement",
                    boundary.id
                ));
            }
        }
    }
    for surface in &definitions.surfaces {
        if let Some(id) = surface.unlock_technology_id {
            if !technologies.technologies.iter().any(|technology| technology.id == id && technology.effects.iter().any(|effect| matches!(effect, TechnologyEffect::UnlockSurface { surface_id } if *surface_id == surface.id))) {
                return Err(format!("surface {} has an invalid unlock requirement", surface.id));
            }
        }
    }
    let mut complete = BTreeSet::new();
    loop {
        let before = complete.len();
        for technology in &technologies.technologies {
            if technology
                .prerequisites
                .iter()
                .all(|id| complete.contains(id))
            {
                complete.insert(technology.id);
            }
        }
        if complete.len() == technologies.technologies.len() {
            break;
        }
        if complete.len() == before {
            return Err("technology graph must be acyclic".into());
        }
    }
    Ok(())
}

fn valid_technology_grant(technology: &TechnologyDefinition) -> bool {
    match &technology.grant {
        TechnologyGrant::Purchase => technology.cost > 0,
        TechnologyGrant::ContractStage { key, name } => {
            technology.cost == 0
                && key.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                && key
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                && !name.trim().is_empty()
        }
    }
}

fn valid_technology_effects(
    technology: &TechnologyDefinition,
    building_ids: &BTreeSet<DefinitionId>,
    boundary_ids: &BTreeSet<DefinitionId>,
    surface_ids: &BTreeSet<DefinitionId>,
) -> bool {
    let mut buildings = BTreeSet::new();
    for building_id in technology.building_unlocks() {
        if !building_ids.contains(&building_id) || !buildings.insert(building_id) {
            return false;
        }
    }
    let mut boundaries = BTreeSet::new();
    for boundary_id in technology.boundary_unlocks() {
        if !boundary_ids.contains(&boundary_id) || !boundaries.insert(boundary_id) {
            return false;
        }
    }
    let mut surfaces = BTreeSet::new();
    technology.effects.iter().all(|effect| {
        if let TechnologyEffect::UnlockSurface { surface_id } = effect {
            return surface_ids.contains(surface_id) && surfaces.insert(*surface_id);
        }
        matches!(
            effect,
            TechnologyEffect::UnlockBuilding { .. } | TechnologyEffect::UnlockBoundary { .. }
        )
    })
}

fn validate_scenarios(
    definitions: &DefinitionsInput,
    technologies: &TechnologiesInput,
    scenarios: &ScenariosInput,
) -> Result<(), String> {
    if scenarios.version == 0 {
        return Err("scenario catalog version must be positive".into());
    }
    unique_positive_ids(
        scenarios.scenarios.iter().map(|scenario| scenario.id),
        "scenario",
    )?;
    let item_ids: BTreeSet<_> = definitions.items.iter().map(|value| value.id).collect();
    let building_ids: BTreeSet<_> = definitions.buildings.iter().map(|value| value.id).collect();
    let recipe_ids: BTreeSet<_> = definitions.recipes.iter().map(|value| value.id).collect();
    let technology_ids: BTreeSet<_> = technologies
        .technologies
        .iter()
        .map(|value| value.id)
        .collect();
    let mut keys = BTreeSet::new();
    for scenario in &scenarios.scenarios {
        if scenario.key.trim().is_empty()
            || scenario.name.trim().is_empty()
            || scenario.description.trim().is_empty()
            || scenario.version == 0
            || scenario.chunk_size <= 0
            || scenario.player_facing >= 6
            || scenario.build_range == 0
            || scenario.carry_slots == 0
            || !keys.insert(scenario.key.clone())
        {
            return Err(format!("scenario {} is incomplete", scenario.id));
        }
        // A contract is the scenario's whole purpose, so an empty stage, an empty bill, a zero
        // line, or an item this build does not have is a scenario that can never be finished
        // rather than a scenario that is merely odd.
        let contract = &scenario.contract;
        if contract.key.trim().is_empty()
            || contract.name.trim().is_empty()
            || contract.stages.is_empty()
            || contract.stages.iter().any(|stage| {
                stage.key.trim().is_empty()
                    || stage.name.trim().is_empty()
                    || stage.brief.trim().is_empty()
                    || stage.reads.trim().is_empty()
                    || stage.requirements.is_empty()
                    || stage
                        .requirements
                        .iter()
                        .any(|need| need.quantity == 0 || !item_ids.contains(&need.item_id))
            })
        {
            return Err(format!(
                "scenario {} has an unfinishable contract",
                scenario.id
            ));
        }
        let mut occupied = BTreeSet::new();
        for building in &scenario.buildings {
            let definition = definitions
                .buildings
                .iter()
                .find(|definition| definition.id == building.definition_id);
            let footprint_clear = definition.map(|definition| {
                definition.footprint.iter().all(|offset| {
                    let turns = if building.orientation >= NORTH {
                        building.orientation - NORTH
                    } else {
                        building.orientation
                    };
                    let offset = rotate_coordinate(*offset, turns);
                    occupied.insert((building.q + offset.q, building.r + offset.r))
                })
            });
            if !building_ids.contains(&building.definition_id)
                || !definition
                    .is_some_and(|value| value.orientation_axis.allows(building.orientation))
                || footprint_clear != Some(true)
                || building
                    .recipe_id
                    .is_some_and(|id| !recipe_ids.contains(&id))
            {
                return Err(format!("scenario {} has an invalid building", scenario.id));
            }
        }
        if scenario
            .resources
            .iter()
            .any(|resource| resource.quantity == 0 || !item_ids.contains(&resource.item_id))
            || scenario
                .initial_inventory
                .iter()
                .any(|item| item.quantity == 0 || !item_ids.contains(&item.item_id))
            || scenario
                .initial_researched
                .iter()
                .any(|id| !technology_ids.contains(id))
        {
            return Err(format!(
                "scenario {} has invalid initial state",
                scenario.id
            ));
        }
        // A scenario that hands the player more than they can carry would start unplayable, so the
        // carrying rule is checked against the starting pack rather than discovered during play.
        let mut initial = BTreeMap::new();
        add_ingredients(&mut initial, &scenario.initial_inventory);
        let initial_slots: u32 = initial
            .iter()
            .map(|(item_id, &quantity)| {
                let stack = definitions
                    .items
                    .iter()
                    .find(|item| item.id == *item_id)
                    .map(|item| item.stack_size)
                    .unwrap_or(1)
                    .max(1);
                quantity.div_ceil(stack)
            })
            .sum();
        if initial_slots > scenario.carry_slots {
            return Err(format!(
                "scenario {} starts the player over their carrying capacity",
                scenario.id
            ));
        }
    }
    Ok(())
}

fn validate_saved_state(
    definitions: &DefinitionsInput,
    technologies: &TechnologiesInput,
    scenario: &ScenarioDefinition,
    state: &SavedState,
    legacy_skills: bool,
) -> Result<(), String> {
    validate_saved_boundaries(definitions, &state.boundaries)?;
    validate_saved_ground(definitions, &state.ground)?;
    hydrology::validate_saved_water(&state.water)?;
    geomorphology::validate_saved_stress(&state.bank_stress)?;
    validate_skill_state(technologies, &state.skills)?;
    let item_ids: BTreeSet<_> = definitions.items.iter().map(|value| value.id).collect();
    let technology_ids: BTreeSet<_> = technologies
        .technologies
        .iter()
        .map(|value| value.id)
        .collect();
    let mut coordinates = BTreeMap::new();
    let mut entity_ids = BTreeSet::new();
    let mut active_workshops = 0;
    for entity in &state.entities {
        let definition = definitions
            .buildings
            .iter()
            .find(|value| value.id == entity.placed.definition_id)
            .ok_or("save references an unknown building")?;
        if definition.manual_work && !entity.disabled {
            active_workshops += 1;
            if active_workshops > 1 {
                return Err("save contains multiple attended workshops".into());
            }
        }
        if definition.recipe_ids.is_some()
            && entity.placed.recipe_id.is_some_and(|id| {
                !definitions
                    .recipes
                    .iter()
                    .any(|recipe| recipe.id == id && definition.supports_recipe(recipe))
            })
        {
            return Err("save contains an unsupported workshop recipe".into());
        }
        let footprint_valid = definition.footprint.iter().all(|offset| {
            let turns = if entity.placed.orientation >= NORTH {
                entity.placed.orientation - NORTH
            } else {
                entity.placed.orientation
            };
            let offset = rotate_coordinate(*offset, turns);
            let cell = (entity.placed.q + offset.q, entity.placed.r + offset.r);
            match coordinates.get(&cell).copied() {
                None => {
                    coordinates.insert(cell, entity.kind);
                    true
                }
                Some(BuildingKind::Bridge) if entity.kind == BuildingKind::Belt => {
                    coordinates.insert(cell, entity.kind);
                    true
                }
                _ => false,
            }
        });
        let footprint: BTreeSet<(i32, i32)> = definition
            .footprint
            .iter()
            .map(|offset| {
                let turns = if entity.placed.orientation >= NORTH {
                    entity.placed.orientation - NORTH
                } else {
                    entity.placed.orientation
                };
                let offset = rotate_coordinate(*offset, turns);
                (entity.placed.q + offset.q, entity.placed.r + offset.r)
            })
            .collect();
        let allowed_outputs: BTreeSet<ItemId> = entity
            .placed
            .recipe_id
            .and_then(|id| definitions.recipes.iter().find(|recipe| recipe.id == id))
            .map(|recipe| recipe.outputs().map(|output| output.item_id).collect())
            .unwrap_or_else(|| definition.output_item_id.into_iter().collect());
        let routes_valid = state.output_routes.get(&entity.id).is_none_or(|routes| {
            routes.len() <= MAX_LINKS
                && routes.iter().all(|(&item_id, route)| {
                    if !allowed_outputs.contains(&item_id)
                        || usize::from(route.direction) >= DIRECTIONS.len()
                    {
                        return false;
                    }
                    let cell = (entity.placed.q + route.q, entity.placed.r + route.r);
                    let (dq, dr) = DIRECTIONS[usize::from(route.direction)];
                    footprint.contains(&cell) && !footprint.contains(&(cell.0 + dq, cell.1 + dr))
                })
        });
        if entity.kind != definition.kind
            || !definition
                .orientation_axis
                .allows(entity.placed.orientation)
            || !footprint_valid
            || !routes_valid
            || !entity_ids.insert(entity.id)
            || entity
                .inventory
                .keys()
                .chain(entity.input_inventory.keys())
                .chain(entity.fuel_inventory.keys())
                .chain(entity.output_inventory.keys())
                .chain(entity.reserved_inputs.keys())
                .any(|item| !item_ids.contains(item))
        {
            return Err("save contains invalid entity state".into());
        }
    }
    if state
        .output_routes
        .keys()
        .any(|entity_id| !entity_ids.contains(entity_id))
    {
        return Err("save contains output routes for an unknown entity".into());
    }
    if state.legacy_fluid_belts.iter().any(|id| {
        !state
            .entities
            .iter()
            .any(|entity| entity.id == *id && entity.kind == BuildingKind::Belt)
    }) {
        return Err("save contains an invalid legacy fluid belt".into());
    }
    let (carry, reach) = research_bonuses(technologies, &state.researched);
    let skills = state.skills.bonuses(technologies);
    let (carry_slots_bonus, build_range_bonus) =
        (carry + skills.carry_slots, reach + skills.build_range);
    let earned_carry_slots = scenario
        .carry_slots
        .saturating_add(carry_slots_bonus)
        .min(MAX_CARRY_SLOTS);
    let earned_build_range = scenario
        .build_range
        .saturating_add(build_range_bonus)
        .saturating_mul(HEX_X as u32);
    if !(-1000..=1000).contains(&state.player.facing_x)
        || !(-1000..=1000).contains(&state.player.facing_y)
        || !(-1000..=1000).contains(&state.player.move_x)
        || !(-1000..=1000).contains(&state.player.move_y)
        || state.player.build_range != earned_build_range
        // A range rather than an equality: creative may widen the pack, so the earned
        // scenario-plus-research number is the floor a save may not go under and
        // `MAX_CARRY_SLOTS` is the ceiling it may not go over. Which value inside that range is
        // right for this run is the checksum's answer, not this function's.
        || state.player.carry_slots < earned_carry_slots
        || state.player.carry_slots > MAX_CARRY_SLOTS
        || state
            .player
            .inventory
            .keys()
            .any(|item| !item_ids.contains(item))
        || state.player.hand.is_some_and(|hand| {
            hand.quantity == 0
                || !item_ids.contains(&hand.item_id)
                || definitions
                    .items
                    .iter()
                    .find(|item| item.id == hand.item_id)
                    .is_none_or(|item| hand.quantity > item.stack_size)
        })
        || state
            .researched
            .iter()
            .any(|id| !technology_ids.contains(id) && !(legacy_skills && technologies.skills.iter().any(|skill| skill.legacy_technology_id == Some(*id))))
        || match state.player.action_cooldown {
            0 => state.pending_gather.is_some() || state.pending_ground.is_some(),
            _ => state.pending_gather.is_some() == state.pending_ground.is_some(),
        }
    {
        return Err("save contains invalid player or research state".into());
    }
    // A board is restored rather than redrawn, so it is checked instead: a slot naming a row this
    // build no longer ships, a duplicate slot, or one holding more than it ever asked for would all
    // survive the checksum and then be drawn as a request nobody can read.
    let mut posted = BTreeSet::new();
    for slot in &state.requests {
        if !definitions
            .requests
            .iter()
            .any(|request| request.id == slot.request_id)
        {
            return Err("save references an unknown hub request".into());
        }
        if !posted.insert(slot.request_id) {
            return Err("save contains invalid hub request state".into());
        }
    }
    // Progress now belongs to the project rather than the slot, so it is checked here: a count
    // above the bill, or one standing against a project already paid for, would survive the
    // checksum and then read as a project permanently one delivery from completion.
    for (id, &delivered) in &state.request_delivered {
        let definition = definitions
            .requests
            .iter()
            .find(|request| request.id == *id)
            .ok_or("save references an unknown hub request")?;
        if delivered > definition.quantity
            || state.request_fills.get(id).copied().unwrap_or_default() > 0
        {
            return Err("save contains invalid hub request state".into());
        }
    }
    if state.requests.len() > REQUEST_SLOTS
        || state
            .request_rounds
            .keys()
            .any(|id| !definitions.requests.iter().any(|request| request.id == *id))
        || state
            .request_fills
            .keys()
            .any(|id| !definitions.requests.iter().any(|request| request.id == *id))
    {
        return Err("save contains invalid hub request state".into());
    }
    let unique_tiles: BTreeSet<_> = state.tiles.iter().map(|tile| (tile.q, tile.r)).collect();
    if unique_tiles.len() != state.tiles.len()
        || state.tiles.iter().any(|tile| tile.resource.is_none())
    {
        return Err("save contains duplicate or empty overlay tiles".into());
    }
    for ground_item in &state.ground_items {
        if ground_item.quantity == 0 || !item_ids.contains(&ground_item.item_id) {
            return Err("save contains invalid ground item state".into());
        }
    }
    Ok(())
}

fn research_bonuses(
    technologies: &TechnologiesInput,
    researched: &BTreeSet<TechnologyId>,
) -> (u32, u32) {
    let mut legacy = SkillsState::default();
    for skill in &technologies.skills {
        if skill
            .legacy_technology_id
            .is_some_and(|id| researched.contains(&id))
        {
            legacy.granted.insert(skill.id);
        }
    }
    let initial = legacy.bonuses(technologies);
    technologies
        .technologies
        .iter()
        .filter(|technology| researched.contains(&technology.id))
        .fold(
            (initial.carry_slots, initial.build_range),
            |(carry_slots, build_range), technology| {
                (
                    carry_slots.saturating_add(technology.carry_slots_bonus()),
                    build_range.saturating_add(technology.build_range_bonus()),
                )
            },
        )
}

fn unique_positive_ids(ids: impl Iterator<Item = u16>, label: &str) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if id == 0 || !seen.insert(id) {
            return Err(format!("{label} ids must be positive and unique"));
        }
    }
    Ok(())
}
