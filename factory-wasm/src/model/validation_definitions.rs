fn validate_all(
    definitions: &DefinitionsInput,
    technologies: &TechnologiesInput,
    scenarios: &ScenariosInput,
) -> Result<(), String> {
    validate_definitions(definitions)?;
    validate_technologies(definitions, technologies)?;
    validate_skills(technologies)?;
    for milestone in &technologies.skill_milestones {
        if let SkillEvent::ContractStage { key } = &milestone.event {
            if !scenarios
                .scenarios
                .iter()
                .any(|s| s.contract.stages.iter().any(|stage| &stage.key == key))
            {
                return Err("skill milestone references an unknown commission".into());
            }
        }
    }
    validate_research_budget(definitions, technologies)?;
    validate_scenarios(definitions, technologies, scenarios)
}

/// The finite catalogue has to be able to pay for everything the tree sells.
///
/// While the board reposted paid rows this question could not be asked: income was unbounded, so
/// the answer was trivially yes and the catalogue's size meant nothing. Finite demand turns the
/// catalogue into a budget, and a budget that does not cover the shipped research would strand a
/// run with technologies it can see, needs, and can never buy — a defect no test of an individual
/// price would catch, because every price in it would be defensible on its own.
///
/// The margin is required rather than merely reported. The plan asks for "an explicit surplus for
/// route choice": a catalogue that funds the tree to the last insight would technically pass while
/// forcing one exact purchase order, which is not a choice.
fn validate_research_budget(
    definitions: &DefinitionsInput,
    technologies: &TechnologiesInput,
) -> Result<(), String> {
    let income: u64 = definitions
        .requests
        .iter()
        .map(|request| u64::from(request.insight))
        .sum();
    let research: u64 = technologies
        .technologies
        .iter()
        .map(|technology| u64::from(technology.cost))
        .sum();
    if income < research {
        return Err(format!(
            "the project catalogue pays {income} insight but research costs {research}: \
             finite demand would strand the tree"
        ));
    }
    if income * 4 < research * 5 {
        return Err(format!(
            "the project catalogue pays {income} insight against {research} of research: \
             too little surplus to leave the purchase order to the player"
        ));
    }
    Ok(())
}
fn validate_definitions(definitions: &DefinitionsInput) -> Result<(), String> {
    validate_boundaries(definitions)?;
    validate_surfaces(definitions)?;
    if definitions.version == 0 {
        return Err("definition version must be positive".into());
    }
    unique_positive_ids(definitions.items.iter().map(|item| item.id), "item")?;
    unique_positive_ids(definitions.recipes.iter().map(|recipe| recipe.id), "recipe")?;
    unique_positive_ids(
        definitions.buildings.iter().map(|building| building.id),
        "building",
    )?;
    unique_positive_ids(
        definitions.requests.iter().map(|request| request.id),
        "request",
    )?;
    let item_ids: BTreeSet<_> = definitions.items.iter().map(|item| item.id).collect();
    // Requests are the only thing in the game that pays insight, and insight is the only thing that
    // buys research. A catalogue with none of them is a catalogue where nothing can ever be learned.
    if definitions.requests.is_empty() {
        return Err("no hub requests: nothing would ever pay insight".into());
    }
    for request in &definitions.requests {
        if request.key.trim().is_empty()
            || request.name.trim().is_empty()
            || request.brief.trim().is_empty()
            || request.quantity == 0
            || request.insight == 0
            || !item_ids.contains(&request.item_id)
        {
            return Err(format!("request {} is incomplete", request.id));
        }
    }
    for item in &definitions.items {
        if item.key.trim().is_empty()
            || item.name.trim().is_empty()
            || item.color.trim().is_empty()
            || item.icon.trim().is_empty()
            || item.description.trim().is_empty()
            || item.stack_size == 0
        {
            return Err(format!(
                "item {} has incomplete display/value data",
                item.id
            ));
        }
    }
    // A fuel item has to be worth burning, or a machine could consume one for nothing.
    for item in &definitions.items {
        if item.fuel_value == Some(0)
            || item.regrowth_ticks == Some(0)
            || item.hand_gather_steps == Some(0)
            || item.extract_steps == Some(0)
        {
            return Err(format!(
                "item {} has a zero fuel, regrowth, hand gather, or extract rate",
                item.id
            ));
        }
    }
    // Every material the world can actually generate must name an extraction rate, because an
    // extractor may be stood on any of them. Without this a new site rule would silently inherit
    // whatever cadence its building carried, which is exactly the flat rate this replaced — and it
    // would do it quietly, on one material, long after the row was written.
    let generated: BTreeSet<ItemId> = world_presets()
        .iter()
        .flat_map(|preset| preset.params.site_rules.iter())
        .map(|rule| rule.item_id)
        .collect();
    for item_id in generated {
        let Some(item) = definitions.items.iter().find(|item| item.id == item_id) else {
            return Err(format!("world presets name unknown item {item_id}"));
        };
        if item.extract_steps.is_none() {
            return Err(format!(
                "item {} ({}) can be generated as a field but names no extract_steps",
                item.id, item.key
            ));
        }
    }
    for building in &definitions.buildings {
        if building.extract_speed == Some(0) {
            return Err(format!("building {} has a zero extract speed", building.id));
        }
        // Anything a belt or a hand can load has to say how much it holds. The capacity lookup
        // falls back to "unbounded", which is a sensible default for a kind that stores nothing and
        // a silent one for a kind that stores plenty — a burner-generator shipped without this line
        // and swallowed an unlimited stack of coal, because nothing anywhere had to notice.
        if Core::stock_is_reachable_by_hand(building.kind) && building.capacity.is_none() {
            return Err(format!(
                "building {} ({}) holds stock but names no capacity",
                building.id, building.key
            ));
        }
    }
    for recipe in &definitions.recipes {
        if recipe.key.trim().is_empty()
            || recipe.name.trim().is_empty()
            || recipe.description.trim().is_empty()
            || recipe.category.trim().is_empty()
            || recipe.duration == 0
            || recipe.inputs.is_empty()
            || recipe.output.quantity == 0
        {
            return Err(format!("recipe {} is incomplete", recipe.id));
        }
        // A recipe no machine can be assigned is content that cannot be reached, which is a defect
        // in the catalog rather than something to discover in play.
        if !definitions
            .buildings
            .iter()
            .any(|building| building.supports_recipe(recipe))
        {
            return Err(format!(
                "recipe {} has category {}, which no building runs",
                recipe.id, recipe.category
            ));
        }
        for ingredient in recipe.inputs.iter().chain(recipe.outputs()) {
            if ingredient.quantity == 0 || !item_ids.contains(&ingredient.item_id) {
                return Err(format!("recipe {} references an invalid item", recipe.id));
            }
        }
    }
    for building in &definitions.buildings {
        if building.key.trim().is_empty()
            || building.name.trim().is_empty()
            || building.description.trim().is_empty()
            || building.icon.trim().is_empty()
        {
            return Err(format!("building {} is incomplete", building.id));
        }
        if matches!(building.kind, BuildingKind::Extractor | BuildingKind::Pump)
            && building.cadence.unwrap_or(0) == 0
        {
            return Err(format!("source {} requires a cadence", building.id));
        }
        if building.kind == BuildingKind::Pump
            && !building
                .output_item_id
                .is_some_and(|item_id| item_ids.contains(&item_id))
        {
            return Err(format!("pump {} requires a known output item", building.id));
        }
        if building
            .output_item_id
            .is_some_and(|item_id| !item_ids.contains(&item_id))
        {
            return Err(format!(
                "source {} requires a known output item",
                building.id
            ));
        }
        // A machine that runs recipes needs a category, and one that does not must not claim one.
        if (building.kind == BuildingKind::Composer) != building.recipe_category.is_some() {
            return Err(format!(
                "building {} has a recipe category that does not match its kind",
                building.id
            ));
        }
        if let Some(ids) = &building.recipe_ids {
            if building.kind != BuildingKind::Composer
                || ids.is_empty()
                || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
                || ids
                    .iter()
                    .any(|id| !definitions.recipes.iter().any(|recipe| recipe.id == *id))
            {
                return Err(format!(
                    "building {} has invalid recipe capabilities",
                    building.id
                ));
            }
        }
        if let Some(multiplier) = building.duration_multiplier {
            if building.kind != BuildingKind::Composer
                || !(1..=60).contains(&multiplier)
                || definitions
                    .recipes
                    .iter()
                    .filter(|recipe| building.supports_recipe(recipe))
                    .any(|recipe| recipe.duration.checked_mul(multiplier).is_none())
            {
                return Err(format!(
                    "building {} has invalid recipe duration multiplier",
                    building.id
                ));
            }
        }
        if building.manual_work
            && (building.kind != BuildingKind::Composer
                || building.recipe_ids.is_none()
                || building.power_draw.unwrap_or(0) != 0
                || definitions
                    .recipes
                    .iter()
                    .any(|recipe| building.supports_recipe(recipe) && recipe.fuel != 0))
        {
            return Err(format!(
                "building {} has invalid manual work capabilities",
                building.id
            ));
        }
        if building.placement_rule == PlacementRule::Water
            && !matches!(
                building.kind,
                BuildingKind::Pump | BuildingKind::Boiler | BuildingKind::Generator
            )
        {
            return Err(format!(
                "building {} places on water but cannot draw from a basin",
                building.id
            ));
        }
        if building.placement_rule == PlacementRule::Shallows
            && building.kind != BuildingKind::Bridge
        {
            return Err(format!(
                "building {} places on shallows but is not a bridge",
                building.id
            ));
        }
        if building.kind == BuildingKind::Generator
            && (building.power_source.is_none() || building.power_output.unwrap_or(0) == 0)
        {
            return Err(format!(
                "generator {} needs a power source and an output",
                building.id
            ));
        }
        let footprint: BTreeSet<_> = building
            .footprint
            .iter()
            .map(|cell| (cell.q, cell.r))
            .collect();
        if footprint.len() != building.footprint.len()
            || !footprint.contains(&(0, 0))
            || footprint.len() > MAX_FOOTPRINT_CELLS
        {
            return Err(format!("building {} has an invalid footprint", building.id));
        }
        // One building is one connected thing. Two lobes with a gap between them would still map
        // every cell into the occupancy index, but reach, routing and the ground pad would all be
        // measuring a shape the player cannot see as a single machine — and the gap cell would be
        // walkable ground inside a building. Contiguity is cheap to state here and impossible to
        // recover later, once a save holds the disconnected entity.
        if !footprint_is_contiguous(&footprint) {
            return Err(format!(
                "building {} has a footprint in disconnected pieces",
                building.id
            ));
        }
        let envelope = unique_offsets(&building.service_envelope, "service envelope", building.id)?;
        let clearance = unique_offsets(
            &building.overhead_clearance,
            "overhead clearance",
            building.id,
        )?;
        if envelope.len() > MAX_ENVELOPE_CELLS {
            return Err(format!(
                "building {} has an invalid service envelope",
                building.id
            ));
        }
        if clearance.len() > MAX_CLEARANCE_CELLS {
            return Err(format!(
                "building {} has an invalid overhead clearance",
                building.id
            ));
        }
        for cell in envelope.iter().chain(clearance.iter()) {
            if footprint.contains(cell) {
                return Err(format!(
                    "building {} reserves a cell it already occupies",
                    building.id
                ));
            }
        }
        if envelope.iter().any(|cell| clearance.contains(cell)) {
            return Err(format!(
                "building {} uses the same cell as envelope and clearance",
                building.id
            ));
        }
        if !envelope.is_empty() {
            let mut with_envelope = footprint.clone();
            with_envelope.extend(envelope.iter().copied());
            if !footprint_is_contiguous(&with_envelope) {
                return Err(format!(
                    "building {} has a service envelope in disconnected pieces",
                    building.id
                ));
            }
        }
        if !clearance.is_empty() {
            let mut with_clearance = footprint.clone();
            with_clearance.extend(clearance.iter().copied());
            if !footprint_is_contiguous(&with_clearance) {
                return Err(format!(
                    "building {} has overhead clearance in disconnected pieces",
                    building.id
                ));
            }
        }
        // No shipped definition needs a multi-cell corner-heading footprint yet. Keep the narrow
        // rule until a real definition asks for the extra path and can test it. The test is "may
        // face a corner", not "faces only corners": an any-axis definition reaches the same
        // untested path the moment it is rotated onto a vertex heading. Envelope and clearance
        // rotate the same way, so they stay empty on that axis too.
        if building.orientation_axis.allows(NORTH)
            && (building.footprint.len() != 1
                || !building.service_envelope.is_empty()
                || !building.overhead_clearance.is_empty())
        {
            return Err(format!(
                "building {} spans the two-row period, which only a single-cell footprint can do",
                building.id
            ));
        }
        if let Some(radius) = building.extract_radius {
            if !matches!(building.kind, BuildingKind::Extractor | BuildingKind::Pump) {
                return Err(format!(
                    "building {} claims a source reach but is not an extractor or pump",
                    building.id
                ));
            }
            if radius == 0 || radius > MAX_EXTRACT_RADIUS {
                return Err(format!(
                    "extractor {} needs a reach in 1..={MAX_EXTRACT_RADIUS}",
                    building.id
                ));
            }
        }
        if let Some(radius) = building.supply_radius {
            if building.kind != BuildingKind::Pole {
                return Err(format!(
                    "building {} claims a supply radius but is not a pole",
                    building.id
                ));
            }
            if radius == 0 || radius > MAX_POLE_SUPPLY_RADIUS {
                return Err(format!(
                    "pole {} needs a supply radius in 1..={MAX_POLE_SUPPLY_RADIUS}",
                    building.id
                ));
            }
        }
        // A pole that supplies further than it can pass current on is a pole that cannot be
        // chained at its own coverage — a line of them would leave dark gaps between lit discs.
        if let (Some(radius), Some(link)) = (building.supply_radius, building.pole_reach) {
            if link < radius {
                return Err(format!(
                    "pole {} reaches less far than it supplies",
                    building.id
                ));
            }
        }
        // Every pole states both of its distances. The defaults above exist so the rule has one
        // definition, not so a data row can stay silent: the host draws the coverage ring straight
        // off this file, and a pole that named no radius would be drawn at a radius nobody chose.
        if building.kind == BuildingKind::Pole
            && (building.supply_radius.is_none() || building.pole_reach.is_none())
        {
            return Err(format!(
                "pole {} must name both the distance it supplies and the distance it links",
                building.id
            ));
        }
        for ingredient in building
            .construction_cost
            .iter()
            .chain(building.corner_construction_cost.iter().flatten())
        {
            if ingredient.quantity == 0 || !item_ids.contains(&ingredient.item_id) {
                return Err(format!("building {} has an invalid cost", building.id));
            }
        }
        // A corner price and a corner gate are answers to a question a building that cannot face a
        // corner is never asked. Refusing them here keeps the data row honest about what it does,
        // rather than carrying a number nothing ever reads.
        if (building.corner_construction_cost.is_some() || building.corner_technology_id.is_some())
            && !building.orientation_axis.allows(NORTH)
        {
            return Err(format!(
                "building {} names a corner price or gate but cannot face a corner",
                building.id
            ));
        }
        // The whole point of retiring the riser is that the two-row reach stays a research step. An
        // any-axis definition without its own corner gate would hand the player that reach at the
        // first belt they place.
        if building.orientation_axis == OrientationAxis::Any
            && building.corner_technology_id.is_none()
        {
            return Err(format!(
                "building {} takes every heading but gates none of them",
                building.id
            ));
        }
        // Bounded, because an unbounded span is a belt that costs nothing per hex.
        if let Some(span) = building.underpass_span {
            if span == 0 || span > MAX_UNDERPASS_SPAN {
                return Err(format!(
                    "building {} spans {span}, outside 1..={MAX_UNDERPASS_SPAN}",
                    building.id
                ));
            }
        }
        if building.transport_medium != TransportMedium::Solid
            && building.kind != BuildingKind::Belt
        {
            return Err(format!(
                "building {} has a transport medium but is not transport",
                building.id
            ));
        }
        if let Some(ids) = &building.accepted_item_ids {
            if building.kind != BuildingKind::Container
                || ids.is_empty()
                || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
                || ids.iter().any(|id| !item_ids.contains(id))
            {
                return Err(format!(
                    "building {} has an invalid storage filter",
                    building.id
                ));
            }
        }
        // Splitting, merging, and spanning are all rules about compiled transport edges, and a
        // building that is not transport compiles none.
        if (building.splits || building.merges || building.underpass_span.is_some())
            && building.kind != BuildingKind::Belt
        {
            return Err(format!(
                "building {} is not transport but claims a transport rule",
                building.id
            ));
        }
        // One entity, one arbitration rule. A definition that both fans out and rotates its feeders
        // would have two answers for which link a single item takes.
        if building.splits && building.merges {
            return Err(format!(
                "building {} cannot both split and merge",
                building.id
            ));
        }
    }
    recipes::validate_routes(definitions)?;
    validate_upgrade_ladders(definitions)?;
    Ok(())
}

/// What an upgrade ladder has to be, checked once at load so `upgrade` itself stays a short
/// command rather than a second copy of the placement rules.
///
/// A tier is a data row, and these are the constraints that make that true: an upgrade may only
/// grow a building into a taller version of itself, never turn it into a different machine. Kind,
/// recipe category, and footprint are all pinned, which is what lets the command preserve
/// contents, orientation, and connections without asking whether any of them still apply. The
/// strictly increasing tier is what makes the ladder finite, so a chain can never cycle.
fn validate_upgrade_ladders(definitions: &DefinitionsInput) -> Result<(), String> {
    for building in &definitions.buildings {
        let Some(next_id) = building.upgrades_to else {
            continue;
        };
        let Some(next) = definitions
            .buildings
            .iter()
            .find(|candidate| candidate.id == next_id)
        else {
            return Err(format!(
                "building {} upgrades to unknown building {next_id}",
                building.id
            ));
        };
        if next.tier <= building.tier {
            return Err(format!(
                "building {} upgrades to {next_id}, which is not a higher tier",
                building.id
            ));
        }
        if next.kind != building.kind
            || next.recipe_category != building.recipe_category
            || next.recipe_ids != building.recipe_ids
            || next.manual_work != building.manual_work
        {
            return Err(format!(
                "building {} upgrades into a different machine, not a higher tier of itself",
                building.id
            ));
        }
        if next.orientation_axis != building.orientation_axis {
            return Err(format!(
                "building {} upgrades onto a different orientation axis",
                building.id
            ));
        }
        if next.foundation_class != building.foundation_class {
            return Err(format!(
                "building {} upgrades onto a different foundation class",
                building.id
            ));
        }
        let footprint: BTreeSet<_> = building
            .footprint
            .iter()
            .map(|cell| (cell.q, cell.r))
            .collect();
        let next_footprint: BTreeSet<_> =
            next.footprint.iter().map(|cell| (cell.q, cell.r)).collect();
        // A tier may take more ground; it may never give up ground it already stands on. Growing
        // into free cells leaves every existing cell, and therefore every connection bound to one,
        // exactly where it was — `upgrade` refuses unless the new cells are empty, so an output ray
        // that used to leave the footprint at some cell still leaves it at the same one. Shrinking
        // or sliding would strand a belt against a hex the building no longer occupies, which is
        // the failure this rule has always been about.
        if !footprint.is_subset(&next_footprint) {
            return Err(format!(
                "building {} upgrades off a cell it stands on, which would move its connections",
                building.id
            ));
        }
        if !next.buildable {
            return Err(format!(
                "building {} upgrades to {next_id}, which cannot be constructed",
                building.id
            ));
        }
    }
    Ok(())
}
