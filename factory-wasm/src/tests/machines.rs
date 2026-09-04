use super::*;

/// Fuel is a property of the item, so a smelting recipe never names one and coal, charcoal, and
/// wood are interchangeable at different values. The one case that has to be got right is a
/// recipe that names a fuel item as an input: steel takes two coal as carbon, and a smelter
/// that burned those two would starve itself on its own recipe.
#[test]
fn machines_draw_on_the_stock_and_terrain_beside_them_and_flora_grows_back() {
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 5]);
    core.player.inventory.insert(1, 40);
    core.player.inventory.insert(6, 40);
    set_player_hex(&mut core, 0, 3);
    stock_for(&mut core, 7, 1);
    core.place(0, 4, 7, 0, Some(2)).unwrap();
    let smelter = core.entity_at(0, 4).unwrap();

    // Inputs but no fuel: the smelter holds everything and says exactly why it is stopped.
    core.entities[smelter].inventory.insert(1, 4);
    core.tick_many(30);
    assert_eq!(core.entities[smelter].progress, 0);
    assert_eq!(
        core.entity_snapshot(smelter).status,
        EntityStatus::OutOfFuel
    );
    assert_eq!(core.entities[smelter].inventory.get(&1), Some(&4));

    // One coal is 160 energy against an 80-energy craft, so the change is banked.
    core.entities[smelter].inventory.insert(5, 1);
    core.tick_many(30);
    assert_eq!(core.entities[smelter].output_inventory.get(&11), Some(&2));
    assert_eq!(core.entities[smelter].fuel_charge, 0);
    assert_eq!(core.entities[smelter].inventory.get(&5), None);
    assert_eq!(core.entities[smelter].inventory.get(&1), None);

    // Steel, whose inputs name coal. Exactly the two it needs must not be burned.
    core.player.inventory.insert(1, 40);
    core.player.inventory.insert(6, 40);
    stock_for(&mut core, 7, 1);
    core.place(0, 6, 7, 0, Some(5)).unwrap();
    let steel = core.entity_at(0, 6).unwrap();
    core.entities[steel].inventory.insert(11, 2);
    core.entities[steel].inventory.insert(5, 2);
    core.tick_many(30);
    assert_eq!(core.entities[steel].progress, 0);
    assert_eq!(core.entity_snapshot(steel).status, EntityStatus::OutOfFuel);
    assert_eq!(core.entities[steel].inventory.get(&5), Some(&2));

    // A third coal is surplus, and surplus is what burns.
    core.entities[steel].inventory.insert(5, 3);
    core.tick_many(40);
    assert_eq!(core.entities[steel].output_inventory.get(&23), Some(&1));
    assert_eq!(core.entities[steel].inventory.get(&5), None);

    // Flora is the one source that comes back, which is what gives wood and ore different
    // strategic weight. Regrowth walks a set of cut cells rather than the world, and that set is
    // derived from the overlay — so a save records the tiles and the set is rebuilt from them.
    let (definitions, technologies, scenarios) = catalogs();
    let mut core = game("new-game");
    let cell = (-3, 1);
    let initial = core.deposit_quantity(cell);
    set_player_hex(&mut core, cell.0, cell.1);
    core.gather().unwrap();
    cooldown(&mut core);
    assert_eq!(core.deposit_quantity(cell), initial - 1);
    assert!(core.flora_regrowth.contains(&cell));

    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.flora_regrowth, core.flora_regrowth);

    let ticks = core
        .item_definition(WOOD)
        .unwrap()
        .regrowth_ticks
        .expect("wood regrows");
    core.tick_many(ticks);
    assert_eq!(core.deposit_quantity(cell), initial);
    // Back to what generation gave it, so the cut cell leaves the set. What is left on the set is
    // the hex the healed stand seeded: a wood growing back past the line a cut used to be is the
    // only way the ground carrying flora ever gains a hex.
    assert!(!core.flora_regrowth.contains(&cell));
    let seedling = core
        .flora_regrowth
        .iter()
        .copied()
        .find(|&key| {
            DIRECTIONS
                .iter()
                .any(|&(dq, dr)| (cell.0 + dq, cell.1 + dr) == key)
        })
        .expect("the healed stand seeded a neighbour");
    assert_eq!(
        core.deposit_quantity(seedling),
        1,
        "and it starts as one tree"
    );
    assert_eq!(core.flora_regrowth.len(), 1, "one seed, not a ring of them");

    // Ore is finite: cutting into a deposit never puts it in the set at all.
    cooldown(&mut core);
    set_player_hex(&mut core, 3, 0);
    core.gather().unwrap();
    cooldown(&mut core);
    assert_eq!(core.deposit_quantity((3, 0)), 47);
    assert_eq!(core.flora_regrowth.len(), 1, "and ore never joins the set");

    // A pump is a source without a deposit: it draws from the basin beside it, writes nothing into
    // the overlay, and the basin never runs down. Away from water it is refused outright, which is
    // what makes a basin a reason to build somewhere.
    let mut core = legacy_band_game("new-game");
    core.researched.extend([1, 2, 5, 7]);
    core.player.inventory.insert(11, 20);
    core.player.inventory.insert(14, 20);
    set_player_hex(&mut core, 2, 0);
    assert!(core.terrain_at(2, 1).is_water());
    stock_for(&mut core, 11, 1);
    core.place(3, 1, 11, 0, None).unwrap();
    let index = core.entity_at(3, 1).unwrap();
    core.tick_many(6);
    assert_eq!(core.entities[index].output_inventory.get(&10), Some(&3));
    assert_eq!(core.entity_snapshot(index).status, EntityStatus::Pumping);
    assert!(core.tiles.get(&(2, 1)).is_none());
    assert!(core
        .place(3, -1, 11, 0, None)
        .unwrap_err()
        .contains("beside open water"));

    // A bridge supports transport on shallows and refuses deep water.
    // Real relief: a bridge needs water to span, and water is where the generated bed is
    // low. See `field_game`.
    let mut core = field_game("new-game");
    core.researched.extend([1, 11, 15]);
    core.player.inventory.insert(1, 10);
    core.player.inventory.insert(6, 10);
    core.player.inventory.insert(16, 10);
    core.player.inventory.insert(24, 10);
    let shallow = (-24..=24)
        .flat_map(|q| (-24..=24).map(move |r| (q, r)))
        .find(|&(q, r)| core.terrain_at(q, r) == Terrain::ShallowWater)
        .expect("the new-game landscape has shallow water");
    let deep = (-512..=512)
        .flat_map(|q| (-512..=512).map(move |r| (q, r)))
        .find(|&(q, r)| core.terrain_at(q, r) == Terrain::DeepWater)
        .expect("the new-game landscape has deep water");

    set_player_hex(&mut core, shallow.0 + 2, shallow.1);
    core.place(shallow.0, shallow.1, 23, 0, None).unwrap();
    core.place(shallow.0, shallow.1, 2, 0, None).unwrap();
    assert_eq!(
        core.entities
            .iter()
            .filter(|entity| { entity.placed.q == shallow.0 && entity.placed.r == shallow.1 })
            .count(),
        2,
        "the support and transport are distinct entities"
    );
    // One click of rotation is one *angle* along, and a belt takes all twelve headings now, so
    // the step out of due east is the vertex heading that sits between east and the next edge
    // rather than that edge itself.
    core.rotate(shallow.0, shallow.1, false).unwrap();
    assert_eq!(
        core.entities[core.entity_at(shallow.0, shallow.1).unwrap()]
            .placed
            .orientation,
        8
    );
    let (definitions, technologies, scenarios) = catalogs();
    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(
        restored
            .entities
            .iter()
            .filter(|entity| entity.placed.q == shallow.0 && entity.placed.r == shallow.1)
            .count(),
        2,
        "a bridge and its transport survive a save"
    );
    assert_eq!(
        core.entities[core.entity_at(shallow.0, shallow.1).unwrap()].kind,
        BuildingKind::Belt
    );
    // A bridge supports the two-row reach as well, and for the same reason: what it permits is
    // a transport *kind* on a ford, and a heading is not a different kind.
    core.erase(shallow.0, shallow.1).unwrap();
    core.place(shallow.0, shallow.1, 2, NORTH, None).unwrap();
    assert_eq!(
        core.entities[core.entity_at(shallow.0, shallow.1).unwrap()]
            .placed
            .orientation,
        NORTH
    );
    core.erase(shallow.0, shallow.1).unwrap();
    assert_eq!(
        core.entities[core.entity_at(shallow.0, shallow.1).unwrap()].kind,
        BuildingKind::Bridge
    );

    set_player_hex(&mut core, deep.0 + 2, deep.1);
    assert!(core.place(deep.0, deep.1, 23, 0, None).is_err());
    assert_eq!(Terrain::ShallowWater.blocks_construction(), true);
}

/// A kiln and a smelter are the same `BuildingKind` running different recipe categories, so the
/// rule that keeps a circuit out of a kiln is one field and one check — asked once at placement
/// and again at reassignment, because a machine that could be reassigned past the rule would
/// make the rule decorative.
#[test]
fn a_machine_runs_only_its_own_category_and_is_reassigned_only_between_crafts() {
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 5, 6]);
    core.player.inventory.insert(1, 40);
    core.player.inventory.insert(6, 40);
    core.player.inventory.insert(8, 20);
    set_player_hex(&mut core, 0, 3);
    assert!(core
        .place(0, 4, 8, 0, Some(2))
        .unwrap_err()
        .contains("cannot run a smelting recipe"));
    stock_for(&mut core, 8, 1);
    core.place(0, 4, 8, 0, Some(6)).unwrap();
    let index = core.entity_at(0, 4).unwrap();

    assert!(core
        .set_recipe(0, 4, 2)
        .unwrap_err()
        .contains("cannot run a smelting recipe"));
    core.set_recipe(0, 4, 7).unwrap();
    assert_eq!(core.entities[index].placed.recipe_id, Some(7));

    // Mid-craft it keeps the job it is running: the inputs it reserved belong to that job.
    core.entities[index].inventory.insert(9, 12);
    core.tick_many(2);
    assert!(core.entities[index].progress > 0);
    assert!(core.set_recipe(0, 4, 6).unwrap_err().contains("mid-craft"));

    // Explicit recipe capabilities replace categories without unlocking the whole category.
    let mut core = game("new-game");
    let kiln = core
        .definitions
        .buildings
        .iter_mut()
        .find(|b| b.id == 8)
        .unwrap();
    kiln.recipe_ids = Some(vec![2, 8]);
    kiln.unlock_technology_id = None;
    let kiln = core.building_definition(8).unwrap();
    assert!(kiln.supports_recipe(core.recipe(2).unwrap()));
    assert!(kiln.supports_recipe(core.recipe(8).unwrap()));
    assert!(!kiln.supports_recipe(core.recipe(6).unwrap()));
    assert!(!core.item_reachable(23, 0));
    core.player.inventory.extend([(1, 40), (6, 40), (8, 20)]);
    set_player_hex(&mut core, 0, 3);
    stock_for(&mut core, 8, 1);
    core.place(0, 4, 8, 0, Some(2)).unwrap();
    assert!(core.item_reachable(11, 0));
    core.set_recipe(0, 4, 8).unwrap();
    assert!(core.set_recipe(0, 4, 6).is_err());
}

/// The explicit way out of the dead end the test above establishes.
///
/// A composer mid-craft refuses reassignment and holds its reserved ingredients out of reach of
/// `withdraw`, which left demolition as the only exit from a job the player no longer wants. This
/// is the abort rule that replaces it: the refund is exactly one batch of the running recipe, back
/// into the ingredient compartment it came from; fuel and finished goods do not move; and the
/// machine afterwards is one that is simply between crafts again — including across a save, since
/// progress and the reservation are both checksummed.
#[test]
fn a_cancelled_craft_returns_its_reserved_inputs_and_leaves_fuel_and_output_alone() {
    let (definitions, technologies, scenarios) = catalogs();
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 5, 6]);
    core.player.inventory.extend([(1, 40), (6, 40), (8, 20)]);
    set_player_hex(&mut core, 0, 3);
    stock_for(&mut core, 8, 1);
    core.place(0, 4, 8, 0, Some(6)).unwrap();
    core.set_recipe(0, 4, 7).unwrap();
    let index = core.entity_at(0, 4).unwrap();

    // Bounded and refused exactly like the reassignment it stands beside. A machine with nothing
    // running is a refusal rather than a quiet success: the host draws this control from
    // `progress`, so the two must never disagree about whether there is a craft to abandon.
    assert!(core.cancel_craft(1 << 20, 0).is_err());
    assert!(core.cancel_craft(0, 2).unwrap_err().contains("no machine"));
    assert!(core
        .cancel_craft(0, 4)
        .unwrap_err()
        .contains("not mid-craft"));

    // Char wood bills six wood and no heat, so the coal in the firebox is there to be left alone
    // rather than to be spent, and the charcoal is a craft that already finished.
    core.entities[index].input_inventory.insert(9, 12);
    core.entities[index].fuel_inventory.insert(5, 3);
    core.entities[index].output_inventory.insert(15, 1);
    core.tick_many(2);
    assert!(core.entities[index].progress > 0);
    assert_eq!(core.entities[index].reserved_inputs.get(&9), Some(&6));
    assert_eq!(core.entities[index].input_inventory.get(&9), Some(&6));

    core.cancel_craft(0, 4).unwrap();
    assert_eq!(core.entities[index].progress, 0);
    assert!(core.entities[index].reserved_inputs.is_empty());
    assert_eq!(core.entities[index].input_inventory.get(&9), Some(&12));
    assert_eq!(core.entities[index].fuel_inventory.get(&5), Some(&3));
    assert_eq!(core.entities[index].output_inventory.get(&15), Some(&1));
    assert!(core
        .events
        .iter()
        .any(|event| event.contains("Cancelled the craft") && event.contains("Wood")));

    // Between crafts again, which is the whole point: the reassignment refused a moment ago now
    // goes through, and the machine is not carrying anything from the job it abandoned.
    core.set_recipe(0, 4, 6).unwrap();
    core.set_recipe(0, 4, 7).unwrap();

    // Exact across a save. Metering is a harness hook that no file carries, so both sides are put
    // on the same footing before the comparison — the claim is about the cancelled machine, not
    // about the hook.
    core.power_unmetered = false;
    let save = core.save_string().unwrap();
    let mut restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.checksum(), core.checksum());
    let restored_index = restored.entity_at(0, 4).unwrap();
    assert_eq!(
        restored.entities[restored_index].input_inventory.get(&9),
        Some(&12)
    );
    core.tick_many(30);
    restored.tick_many(30);
    assert_eq!(restored.checksum(), core.checksum());
}

#[test]
fn skills_are_finite_atomic_and_isolated_from_research() {
    let mut core = game("new-game");
    core.advance(r#"[{"type":"set_creative","enabled":true}]"#, 0, 0)
        .unwrap();
    assert!(
        !core.creative,
        "a running standard game cannot become creative"
    );
    assert!(core
        .events
        .iter()
        .any(|event| event.contains("starting a new game")));
    let start = core.checksum();
    assert!(core.purchase_skill(1).is_err());
    assert!(core.purchase_skill(999).is_err());
    assert_eq!(core.checksum(), start);
    core.observe_skill_event(SkillEvent::WorkshopCraft);
    core.observe_skill_event(SkillEvent::WorkshopCraft);
    assert_eq!(core.skills.points, 1);
    let insight = core.insight;
    let carry = core.player.carry_slots;
    let reach = core.player.build_range;
    core.purchase_skill(1).unwrap();
    assert_eq!(core.player.carry_slots, carry + 4);
    assert_eq!(core.player.build_range, reach);
    assert_eq!(core.insight, insight);
    let bought = core.checksum();
    assert!(core.purchase_skill(1).is_err());
    assert!(core.purchase_skill(2).is_err());
    assert_eq!(core.checksum(), bought);
    core.observe_skill_event(SkillEvent::ContractStage {
        key: "components".into(),
    });
    core.purchase_skill(2).unwrap();
    assert_eq!(core.player.build_range, reach + 3 * HEX_X as u32);
    // Four points for proving the line, less the one the first rank of reach cost.
    assert_eq!(core.skills.points, 3);
    core.observe_skill_event(SkillEvent::PoweredCraft);
    core.observe_skill_event(SkillEvent::PoweredCraft);
    assert_eq!(core.skills.points, 9);
    let (definitions, technologies, scenarios) = catalogs();
    let mut restored = Core::from_save(
        &definitions,
        &technologies,
        &scenarios,
        &core.save_string().unwrap(),
    )
    .unwrap();
    assert_eq!(restored.checksum(), core.checksum());
    restored.observe_skill_event(SkillEvent::WorkshopCraft);
    assert_eq!(restored.checksum(), core.checksum());

    // Skills creative grants cannot mint milestones after returning.
    let mut core = game("new-game");
    core.observe_skill_event(SkillEvent::WorkshopCraft);
    core.purchase_skill(1).unwrap();
    core.set_creative(true);
    assert_eq!(core.skills.purchased, BTreeSet::from([1]));
    assert_eq!(
        core.skills.granted,
        BTreeSet::from([2, 3, 4, 5, 6, 7, 8, 9, 10, 11])
    );
    core.set_creative(false);
    core.observe_skill_event(SkillEvent::PoweredCraft);
    core.observe_skill_event(SkillEvent::ContractStage {
        key: "components".into(),
    });
    assert_eq!(core.skills.points, 0);
    assert_eq!(core.skills.completed, BTreeSet::from([1]));
    let (definitions, technologies, scenarios) = catalogs();
    let mut restored = Core::from_save(
        &definitions,
        &technologies,
        &scenarios,
        &core.save_string().unwrap(),
    )
    .unwrap();
    restored.observe_skill_event(SkillEvent::PoweredCraft);
    assert_eq!(restored.checksum(), core.checksum());
}

#[test]
fn the_field_survey_opens_the_same_distance_in_every_direction() {
    let size = game("new-game").scenario.chunk_size;
    let far = 4_000 * size;
    let unsurveyed = |core: &Core, from: (i32, i32)| {
        hexes_in_radius(from, core.survey_radius())
            .into_iter()
            .find(|cell| {
                !core
                    .generated_chunks
                    .contains(&(floor_div(cell.0, size), floor_div(cell.1, size)))
            })
    };

    // Where inside a chunk the player stood used to decide how far ahead the world opened:
    // rings were centred on the containing chunk, so an edge cell had one cell of margin in
    // front of it and fifteen behind. Every local position now owes the same radius, and it is
    // that equality — not the chunk count — that the player reads as an even frontier.
    for local in [0, 1, size / 2, size - 1] {
        let mut narrow = game("new-game");
        assert_eq!(narrow.survey_rings(), 1);
        assert_eq!(narrow.survey_radius(), size + size / 2);
        let cell = (far + local, far + local);
        let (x, y) = axial_world(cell.0, cell.1);
        narrow.ensure_neighborhood(x, y);
        assert_eq!(unsurveyed(&narrow, cell), None, "local offset {local}");
    }

    let mut wide = game("new-game");
    wide.observe_skill_event(SkillEvent::WorkshopCraft);
    wide.purchase_skill(3).unwrap();
    assert_eq!(wide.survey_rings(), 2);
    assert_eq!(wide.survey_radius(), 2 * size + size / 2);
    // Learning it pays out where you stand rather than on the next step.
    let opened = wide.generated_chunks.len();
    assert!(opened > game("new-game").generated_chunks.len());
    let cell = (far, far);
    let (x, y) = axial_world(cell.0, cell.1);
    wide.ensure_neighborhood(x, y);
    assert_eq!(unsurveyed(&wide, cell), None);
    let reached = wide.generated_chunks.len();
    // Generation is idempotent per chunk, so surveying the same ground twice moves nothing —
    // which is what lets the purchase re-survey a neighbourhood that is already half open.
    let settled = wide.checksum();
    wide.ensure_neighborhood(x, y);
    assert_eq!(wide.generated_chunks.len(), reached);
    assert_eq!(wide.checksum(), settled);

    // The wider survey is derived from the skill, never stored: a reload rebuilds it from the
    // purchased set, and the surveyed world it produced is the one the checksum was taken over.
    let (definitions, technologies, scenarios) = catalogs();
    let restored = Core::from_save(
        &definitions,
        &technologies,
        &scenarios,
        &wide.save_string().unwrap(),
    )
    .unwrap();
    assert_eq!(restored.survey_rings(), 2);
    assert_eq!(restored.checksum(), wide.checksum());
}

#[test]
fn skills_observe_real_power_and_commission_work_and_preserve_widened_packs() {
    let mut demo = bare_game("factory-demo");
    demo.power_unmetered = false;
    demo.tick_many(400);
    assert!(demo.skills.completed.contains(&3));
    let points = demo.skills.points;
    demo.tick_many(400);
    assert_eq!(demo.skills.points, points);
    let mut core = game("new-game");
    let component = core.scenario.contract.stages[0].requirements[0].item_id;
    core.player.inventory.insert(component, 1);
    core.deposit_item(Some(component)).unwrap();
    assert_eq!(core.skills.points, 4);
    assert!(core.skills.completed.contains(&2));
    core.advance_contract();
    assert_eq!(core.skills.points, 4);
    // A widened legacy pack is a floor, not four extra slots above the creative ceiling.
    core.player.carry_slots = MAX_CARRY_SLOTS;
    let availability = core.skill_availability(&core.technologies.skills[0]);
    assert_eq!(availability.current_value, MAX_CARRY_SLOTS);
    assert_eq!(availability.resulting_value, MAX_CARRY_SLOTS);
    core.purchase_skill(1).unwrap();
    assert_eq!(core.player.carry_slots, availability.resulting_value);

    // Skills deltas follow native state and catalogues reject cycles and short budgets.
    let mut factory = test_factory("new-game");
    let mut previous = factory.core.snapshot();
    factory.build_delta();
    factory.core.observe_skill_event(SkillEvent::WorkshopCraft);
    assert_delta_matches_full_diff(&mut factory, &mut previous, "workshop milestone");
    factory.core.purchase_skill(2).unwrap();
    assert_delta_matches_full_diff(&mut factory, &mut previous, "skill purchase");
    factory.core.tick_many(5);
    assert_delta_matches_full_diff(&mut factory, &mut previous, "idle skills");
    let (_, technologies, _) = catalogs();
    validate_skills(&technologies).unwrap();
    let mut invalid = technologies.clone();
    invalid.skills[0].prerequisites = vec![2];
    invalid.skills[1].prerequisites = vec![1];
    assert!(validate_skills(&invalid).is_err());
    let mut invalid = technologies.clone();
    invalid.skill_milestones.clear();
    assert!(validate_skills(&invalid).is_err());
    let mut invalid = technologies.clone();
    invalid.skills[0].effect = SkillEffect::CarrySlots { amount: 999 };
    assert!(validate_skills(&invalid).is_err());
}

#[test]
fn primitive_capabilities_are_validated_and_the_first_machines_pay_for_themselves() {
    let (definitions, _, _) = catalogs();
    for ids in [vec![], vec![8, 8], vec![9999], vec![2]] {
        let mut invalid = definitions.clone();
        invalid
            .buildings
            .iter_mut()
            .find(|building| building.id == 28)
            .unwrap()
            .recipe_ids = Some(ids);
        assert!(validate_definitions(&invalid).is_err());
    }
    for multiplier in [0, 61, u32::MAX] {
        let mut invalid = definitions.clone();
        invalid
            .buildings
            .iter_mut()
            .find(|building| building.id == 28)
            .unwrap()
            .duration_multiplier = Some(multiplier);
        assert!(validate_definitions(&invalid).is_err());
    }

    // Primitive furnace uses local fuel without power and recovers its build cost.
    let mut core = primitive_test_core();
    let original = core.player.inventory.clone();
    core.place(0, 4, 27, 0, Some(2)).unwrap();
    let index = core.entity_at(0, 4).unwrap();
    assert_eq!(core.entity_snapshot(index).status, EntityStatus::OutOfFuel);
    core.store(0, 4, 1, 2).unwrap();
    core.store(0, 4, 9, 2).unwrap();
    assert_eq!(
        core.entity_snapshot(index).status,
        EntityStatus::WaitingForInputs
    );
    core.tick_many(1);
    assert_eq!(core.entity_snapshot(index).status, EntityStatus::Composing);
    core.tick_many(19);
    assert_eq!(core.entities[index].output_inventory.get(&11), Some(&1));
    assert_eq!(core.entity_snapshot(index).status, EntityStatus::OutOfFuel);
    assert_eq!(core.entities[index].fuel_charge, 0);
    assert_eq!(core.entities[index].power_charge, 0);
    assert!(core.set_recipe(0, 4, 5).is_err());
    core.erase(0, 4).unwrap();
    let mut expected = original;
    subtract_item(&mut expected, 1, 2);
    subtract_item(&mut expected, 9, 2);
    expected.insert(11, 1);
    assert_eq!(core.player.inventory, expected);
    // No one-time gift or researched unlock is required to rebuild.
    core.place(0, 4, 27, 0, Some(2)).unwrap();
    core.erase(0, 4).unwrap();
    assert_eq!(core.player.inventory, expected);

    // Mechanical component commission is repeatable without research or power.
    for (fuel, quantity) in [(COAL, 2), (WOOD, 6)] {
        let mut core = primitive_test_core();
        core.player.inventory = BTreeMap::from([(STONE, 8), (CLAY, 4), (WOOD, 4), (IRON_ORE, 6)]);
        *core.player.inventory.entry(fuel).or_default() += quantity;
        core.place(0, 4, 27, 0, Some(2)).unwrap();
        core.place(1, 3, 28, 0, Some(11)).unwrap();
        core.store(0, 4, IRON_ORE, 6).unwrap();
        core.store(0, 4, fuel, quantity).unwrap();
        core.tick_many(60);
        core.withdraw(0, 4, 11, 3).unwrap();
        core.store(1, 3, 11, 2).unwrap();
        core.set_enabled(1, 3, true).unwrap();
        core.tick_many(32);
        assert_eq!(core.skills.points, 1);
        assert!(core.skills.completed.contains(&1));
        if std::env::var_os("UPDATE_SKILL_BROWSER_FIXTURES").is_some() {
            std::fs::create_dir_all("target/skills-browser").unwrap();
            std::fs::write(
                "target/skills-browser/earned.hxf1",
                core.save_string().unwrap(),
            )
            .unwrap();
        }
        core.withdraw(1, 3, 19, 1).unwrap();
        core.set_recipe(1, 3, 1).unwrap();
        core.store(1, 3, 11, 1).unwrap();
        core.store(1, 3, 19, 1).unwrap();
        core.set_enabled(1, 3, true).unwrap();
        core.tick_many(7);
        let (definitions, technologies, scenarios) = catalogs();
        let mut resumed = Core::from_save(
            &definitions,
            &technologies,
            &scenarios,
            &core.save_string().unwrap(),
        )
        .unwrap();
        core.tick_many(25);
        resumed.tick_many(25);
        assert_eq!(core.checksum(), resumed.checksum());
        core.withdraw(1, 3, 2, 1).unwrap();
        assert_eq!(core.player.inventory, BTreeMap::from([(2, 1)]));
        assert!(core.researched.is_empty());
        assert_eq!(core.insight, 0);
        set_player_hex(&mut core, 0, -1);
        core.deposit_inventory().unwrap();
        assert_eq!(core.contract_stage, 1);
        assert_eq!(core.researched, BTreeSet::from([1, 2, 4, 8]));
        assert_eq!(core.insight, 0);
        set_player_hex(&mut core, 0, 3);
        core.erase(0, 4).unwrap();
        core.erase(1, 3).unwrap();
        assert_eq!(
            core.player.inventory,
            BTreeMap::from([(STONE, 8), (CLAY, 4), (WOOD, 4)])
        );
        core.place(0, 4, 27, 0, Some(2)).unwrap();
        core.place(1, 3, 28, 0, Some(11)).unwrap();
    }
}

#[test]
fn manual_workshop_requires_attendance_and_runs_exactly_one_batch() {
    let mut core = primitive_test_core();
    core.place(0, 4, 28, 0, Some(8)).unwrap();
    let index = core.entity_at(0, 4).unwrap();
    assert!(core.set_enabled(0, 4, true).is_err());
    core.store(0, 4, 9, 4).unwrap();
    core.tick_many(100);
    assert_eq!(core.entities[index].progress, 0);
    assert!(core.entities[index].output_inventory.is_empty());
    core.set_enabled(0, 4, true).unwrap();
    core.tick_many(10);
    assert_eq!(core.entities[index].progress, 10);
    core.set_move_intent(1000, 0).unwrap();
    core.tick_many(1);
    assert!(core.entities[index].disabled);
    assert_eq!(core.entities[index].progress, 10);
    assert!(core.set_enabled(0, 4, true).is_err());
    core.set_move_intent(0, 0).unwrap();
    core.set_enabled(0, 4, true).unwrap();
    core.tick_many(14);
    assert_eq!(core.entities[index].output_inventory.get(&16), Some(&2));
    assert!(core.entities[index].disabled);
    core.tick_many(100);
    assert_eq!(core.entities[index].output_inventory.get(&16), Some(&2));
    assert_eq!(core.entities[index].input_inventory.get(&9), Some(&3));
    set_player_hex(&mut core, 0, 2);
    assert!(core.set_enabled(0, 4, true).is_err());
    set_player_hex(&mut core, 0, 3);
    core.player.action_cooldown = 1;
    assert!(core.set_enabled(0, 4, true).is_err());

    // Manual workshop jobs resume after save and cancel without losing reserved inputs.
    let mut core = primitive_test_core();
    let original = core.player.inventory.clone();
    core.place(0, 4, 28, 0, Some(8)).unwrap();
    core.store(0, 4, 9, 2).unwrap();
    core.set_enabled(0, 4, true).unwrap();
    core.tick_many(7);
    let (definitions, technologies, scenarios) = catalogs();
    let mut restored = Core::from_save(
        &definitions,
        &technologies,
        &scenarios,
        &core.save_string().unwrap(),
    )
    .unwrap();
    assert_eq!(restored.checksum(), core.checksum());
    core.tick_many(17);
    restored.tick_many(17);
    assert_eq!(restored.checksum(), core.checksum());
    restored.set_enabled(0, 4, true).unwrap();
    restored.tick_many(2);
    restored.set_enabled(0, 4, false).unwrap();
    assert!(restored
        .set_recipe(0, 4, 11)
        .unwrap_err()
        .contains("mid-craft"));
    restored.erase(0, 4).unwrap();
    let mut expected = original;
    subtract_item(&mut expected, 9, 1);
    expected.insert(16, 2);
    assert_eq!(restored.player.inventory, expected);

    // Manual workshop permit is exclusive and blocked starts leave state unchanged.
    let mut core = primitive_test_core();
    core.place(0, 4, 28, 0, Some(8)).unwrap();
    // Two benches, side by side rather than overlapping: a workshop stands on two hexes.
    core.place(-2, 4, 28, 0, Some(8)).unwrap();
    core.store(0, 4, 9, 2).unwrap();
    core.store(-2, 4, 9, 2).unwrap();
    core.set_enabled(0, 4, true).unwrap();
    core.tick_many(5);
    let first = core.entity_at(0, 4).unwrap();
    core.set_enabled(-2, 4, true).unwrap();
    assert!(core.entities[first].disabled);
    core.tick_many(24);
    assert_eq!(core.entities[first].progress, 5);
    let second = core.entity_at(-2, 4).unwrap();
    core.entities[second].output_inventory.insert(16, 24);
    let before = core.checksum();
    assert!(core.set_enabled(-2, 4, true).unwrap_err().contains("full"));
    assert_eq!(core.checksum(), before);

    // Manual workshop dirty deltas cover permits progress completion and erasure.
    let mut factory = test_factory("new-game");
    factory.core = primitive_test_core();
    let _ = factory.snapshot_json();
    let mut previous = factory.core.snapshot();
    factory.core.place(0, 4, 28, 0, Some(8)).unwrap();
    factory.core.store(0, 4, 9, 3).unwrap();
    assert_delta_matches_full_diff(&mut factory, &mut previous, "workshop placed and loaded");
    factory.core.set_enabled(0, 4, true).unwrap();
    for tick in 0..24 {
        factory.core.tick_many(1);
        assert_delta_matches_full_diff(&mut factory, &mut previous, &format!("manual tick {tick}"));
    }
    factory.core.set_enabled(0, 4, true).unwrap();
    factory.core.tick_many(3);
    factory.core.set_move_intent(1000, 0).unwrap();
    factory.core.tick_many(1);
    assert_delta_matches_full_diff(&mut factory, &mut previous, "movement pauses work");
    factory.core.erase(0, 4).unwrap();
    assert_delta_matches_full_diff(&mut factory, &mut previous, "cancel with reserved refund");
}

#[test]
fn legacy_factories_keep_their_state_and_the_repriced_bills_conserve() {
    let (mut legacy, technologies, scenarios) = catalogs();
    legacy.version = 15;
    legacy.buildings.retain(|building| building.id < 27);
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|scenario| scenario.key == "factory-demo")
        .unwrap();
    let mut old = Core::new(&legacy, &technologies, scenario, None, None).unwrap();
    old.tick_many(123);
    // Written by the current runtime, then relabelled as the envelope it is standing in for, so
    // the file walks every released definition step (15 -> 16 -> 17) on the way back in.
    let json = old.save_string().unwrap().replacen(
        &format!("\"save_version\":{SAVE_VERSION}"),
        "\"save_version\":17",
        1,
    );
    let (definitions, _, _) = catalogs();
    assert_refused_as_legacy_scale(Core::from_save(
        &definitions,
        &technologies,
        &scenarios,
        &json,
    ));

    // Essential and industrial stations are billed in manufactured parts, and erase hands back
    // exactly that bill. The pump adds kiln-fired brick; the kiln itself never requires brick.
    //
    // Both halves matter. The first is the design: not one of them is a box of raw ore any more,
    // and the primitive furnace/workshop start the parts chain before industrial power, so the
    // bootstrap stays open. The second is the safety property
    // that lets the first be changed at all — a refund that equals the rebuild cost can be taken
    // as often as you like and never pays.
    let (definitions, _, _) = catalogs();
    let bill = |key: &str| -> Vec<(ItemId, u32)> {
        definitions
            .buildings
            .iter()
            .find(|building| building.key == key)
            .unwrap_or_else(|| panic!("building {key} exists"))
            .construction_cost
            .iter()
            .map(|ingredient| (ingredient.item_id, ingredient.quantity))
            .collect()
    };
    // Plate, gear, frame, timber and drawn iron wire — and no signal crystal in front of the
    // composer, which was the one early building gated behind a thirty-two-hex walk.
    assert_eq!(bill("extractor"), [(11, 2), (19, 1), (16, 2)]);
    assert_eq!(bill("composer"), [(11, 2), (19, 1), (20, 1)]);
    assert_eq!(bill("container"), [(16, 3)]);
    assert_eq!(bill("pole"), [(16, 1), (25, 1)]);
    assert_eq!(bill("burner-generator"), [(11, 1), (20, 1), (25, 2)]);
    assert_eq!(bill("smelter"), [(6, 6), (11, 2)]);
    assert_eq!(bill("kiln"), [(6, 6), (8, 2), (11, 1)]);
    assert_eq!(bill("cutter"), [(6, 4), (11, 2), (19, 1)]);
    assert_eq!(bill("crusher"), [(6, 6), (11, 2), (19, 1)]);
    assert_eq!(bill("pump"), [(11, 2), (19, 1), (14, 3)]);
    // The two tier bills that still read as a box of raw ore, and the generator that shared the
    // boiler's bill. A deep extractor is the first station to ask for both a gear and a frame;
    // a deep container is the shallow one's timber and plate again, not ore; and a river wheel
    // is rotor, gearing and bracing, with nothing fired and nothing laid in brick.
    assert_eq!(
        bill("extractor-ii"),
        [(11, 2), (19, 2), (20, 1), (3, 1), (6, 2)]
    );
    assert_eq!(bill("container-ii"), [(11, 3), (16, 5), (6, 2)]);
    assert_eq!(bill("hydro-generator"), [(11, 4), (19, 1), (20, 1)]);
    // No raw ore is left in any bill in the catalogue: every station is bought with something
    // that was made.
    for building in &definitions.buildings {
        assert!(
            !building
                .construction_cost
                .iter()
                .any(|ingredient| ingredient.item_id == 1),
            "{} still bills raw ore",
            building.key
        );
    }
    // The hydro generator and the boiler are both unlocked in the power tier and no longer
    // quote the same parts, so picking one is a decision rather than a coin flip.
    assert_ne!(bill("hydro-generator"), bill("boiler"));

    let mut core = legacy_band_game("new-game");
    core.researched.extend([1, 2, 3, 4, 8]);
    core.player.carry_slots = 99;
    core.player.build_range = 1 << 20;
    // Well clear of every plot below, because a station now covers several hexes and someone
    // standing on one of them is a placement failure rather than a priced bill.
    set_player_hex(&mut core, 0, 8);
    let round_trip = |core: &mut Core, definition_id: DefinitionId, q: i32, r: i32, recipe| {
        core.player.inventory.clear();
        stock_for(core, definition_id, 1);
        let paid = core.player.inventory.clone();
        core.place(q, r, definition_id, 0, recipe).unwrap();
        assert!(
            core.player.inventory.is_empty(),
            "an exact bill for definition {definition_id} is spent exactly"
        );
        core.erase(q, r).unwrap();
        assert_eq!(
            core.player.inventory, paid,
            "erasing definition {definition_id} returns its bill, and only its bill"
        );
    };
    round_trip(&mut core, 1, 3, 0, None);
    round_trip(&mut core, 4, 0, 3, None);
    // West of the hub's own seven hexes, which the composer's three would otherwise reach into.
    round_trip(&mut core, 3, -3, 0, Some(1));
    // The pole and the burner go wherever the clearing has room; their bills are the subject
    // here, not their geometry.
    for (definition_id, recipe) in [(7, Some(2)), (8, Some(6)), (9, Some(8)), (10, Some(9))] {
        core.researched.extend([5, 6]);
        round_trip(&mut core, definition_id, 0, 4, recipe);
    }
    for definition_id in [11, 12, 13] {
        core.researched.extend([5, 6, 7]);
        core.player.inventory.clear();
        stock_for(&mut core, definition_id, 1);
        let paid = core.player.inventory.clone();
        let (q, r) = try_place_near(&mut core, (3, 0), definition_id);
        assert!(core.player.inventory.is_empty());
        core.erase(q, r).unwrap();
        assert_eq!(core.player.inventory, paid);
    }
    // The repriced ones pay and refund like every other station. The river wheel goes wherever
    // there is room — dry ground makes it produce nothing, which is not what is under test —
    // and the deep extractor goes on the ore field the shallow one came off.
    core.researched.extend([9, 11, 12]);
    round_trip(&mut core, 19, 3, 0, None);
    for definition_id in [15, 20] {
        core.player.inventory.clear();
        stock_for(&mut core, definition_id, 1);
        let paid = core.player.inventory.clone();
        let (q, r) = try_place_near(&mut core, (3, 0), definition_id);
        assert!(core.player.inventory.is_empty());
        core.erase(q, r).unwrap();
        assert_eq!(core.player.inventory, paid);
    }

    // Iron wire is what the first generator and the first pole are wound with, so it has to be
    // makeable before either of them exists — which means by hand at the manual workshop, with no
    // research and no power, as well as at the composer the workshop stands in for.
    let mut core = primitive_test_core();
    core.player.inventory.insert(11, 1);
    core.place(0, 4, 28, 0, Some(16)).unwrap();
    let index = core.entity_at(0, 4).unwrap();
    core.store(0, 4, 11, 1).unwrap();
    core.set_enabled(0, 4, true).unwrap();
    // Four times industrial craft time, like every other job the workshop takes.
    core.tick_many(24);
    assert_eq!(
        core.entities[index].output_inventory.get(&25),
        Some(&2),
        "one plate draws into two lengths of wire"
    );

    // The same recipe at the bench it was written for, at its own speed.
    let mut powered = game("new-game");
    powered.researched.extend([1, 2, 3]);
    stock_for(&mut powered, 3, 1);
    *powered.player.inventory.entry(11).or_insert(0) += 1;
    powered.place(-3, 1, 3, 0, Some(16)).unwrap();
    let composer = powered.entity_at(-3, 1).unwrap();
    powered.store(-3, 1, 11, 1).unwrap();
    powered.tick_many(6);
    assert_eq!(
        powered.entities[composer].output_inventory.get(&25),
        Some(&2)
    );
}

/// Flora does not come back on a timer. It comes back from flora.
///
/// The cadence an item declares is what an *intact* neighbourhood grows at; a thinned one grows
/// proportionally slower, and one with nothing standing in it does not grow at all. That is the
/// difference between a wood being a renewable resource and a wood hex being a vending machine:
/// cutting into an edge repairs itself, and flattening a stand costs the stand.
#[test]
fn flora_spreads_from_standing_flora_and_a_clear_cut_stays_cut() {
    let mut core = game("new-game");
    let cadence = core
        .item_definition(WOOD)
        .unwrap()
        .regrowth_ticks
        .expect("wood regrows");
    // `game` plants exactly one wood cell, so `(-3, 1)` is its own whole neighbourhood.
    assert_eq!(core.deposit_quantity((-3, 1)), 14);

    // Thinned by one, with all but one unit of its neighbourhood still standing: the item's own
    // cadence, unchanged. A rule that slowed this case would have doubled every recovery in the
    // game the first time anybody took a single tree.
    core.write_overlay(-3, 1, WOOD, 13, 14);
    core.tick_many(cadence);
    assert_eq!(core.deposit_quantity((-3, 1)), 14);

    // Taken to nothing, and there is no longer anything within a ring to seed it. Waiting eight
    // full cadences changes nothing, because waiting is not what regrows a wood.
    core.write_overlay(-3, 1, WOOD, 0, 14);
    core.tick_many(cadence * 8);
    assert_eq!(core.deposit_quantity((-3, 1)), 0);
    assert!(
        core.flora_regrowth.contains(&(-3, 1)),
        "a cut cell stays on the roster; it is simply not growing"
    );

    // A standing neighbour is a seed source, and the same dead hex starts to come back from it.
    core.write_overlay(-2, 1, WOOD, 14, 14);
    core.tick_many(cadence * 4);
    assert!(
        core.deposit_quantity((-3, 1)) > 0,
        "a cell beside standing wood recovers"
    );
    // And the neighbour is not consumed by seeding. Spread is a rate that standing wood sets, not
    // a stock it spends: a wood does not thin itself out by growing back the hex beside it.
    assert_eq!(core.deposit_quantity((-2, 1)), 14);

    // Between "never" and "at the item's cadence" the rule is a gradient, and the gradient is the
    // whole design: two hexes cut to exactly the same depth recover at different speeds because of
    // what is standing around them. `thin` has only itself; `sheltered` has a full stand beside it.
    let (thin, sheltered, shelter) = ((20, 20), (30, 30), (31, 30));
    core.write_overlay(thin.0, thin.1, WOOD, 7, 14);
    core.write_overlay(sheltered.0, sheltered.1, WOOD, 7, 14);
    core.write_overlay(shelter.0, shelter.1, WOOD, 14, 14);
    core.tick_many(cadence * 6);
    assert!(
        core.deposit_quantity(sheltered) > core.deposit_quantity(thin),
        "the sheltered hex should outgrow the exposed one: {} vs {}",
        core.deposit_quantity(sheltered),
        core.deposit_quantity(thin)
    );
    assert!(
        core.deposit_quantity(thin) > 7,
        "the exposed hex still recovers from its own standing wood, only slower"
    );
}

/// A deposit worked to nothing is not a deposit to look at. Nothing is standing, so nothing is
/// published and the hex reads as ordinary ground — for ore and for a cleared stand alike. What
/// separates them is what happens next: ore never grows back, so its ground grades like any other,
/// while a stand is still coming back and its ground is still spoken for.
#[test]
fn a_spent_deposit_reads_and_grades_as_ordinary_ground() {
    let mut core = ground_world();
    core.write_overlay(2, 0, IRON_ORE, 3, 48);
    assert!(
        core.resource_snapshots()
            .iter()
            .any(|row| (row.q, row.r) == (2, 0)),
        "a deposit with stock left is published"
    );
    assert!(core
        .ground_preview(&ground_edit(2, 0, GroundAction::Lower))
        .error
        .unwrap()
        .contains("deposit"));

    // Worked out. Ore declares no regrowth cadence, so there is nothing left here to publish, to
    // draw, or to protect the ground with.
    core.write_overlay(2, 0, IRON_ORE, 0, 48);
    assert!(core.deposit_exhausted(2, 0));
    assert_eq!(
        core.resource_snapshot((2, 0)),
        None,
        "a spent deposit publishes no field, so nothing draws an empty bar over it"
    );
    assert!(
        !core
            .resource_snapshots()
            .iter()
            .any(|row| (row.q, row.r) == (2, 0)),
        "and it is gone from the whole-world list too"
    );
    assert!(
        core.dirty.resources_replace,
        "the host has to be told to drop the row it is already holding"
    );
    assert_eq!(
        core.ground_preview(&ground_edit(2, 0, GroundAction::Lower))
            .error,
        None,
        "a spent deposit no longer protects ground it is not measured in"
    );

    // A stand cut to nothing publishes nothing either — an empty hex looks like ordinary ground
    // whatever emptied it — but it is not *exhausted*, so it stays on the regrowth roster and the
    // ground under it is still a deposit's ground. The row comes back on its own with the first tree.
    core.write_overlay(-2, 0, WOOD, 0, 14);
    assert!(!core.deposit_exhausted(-2, 0));
    assert_eq!(core.resource_snapshot((-2, 0)), None);
    assert!(core.flora_regrowth.contains(&(-2, 0)));
    assert!(core
        .ground_preview(&ground_edit(-2, 0, GroundAction::Lower))
        .error
        .unwrap()
        .contains("deposit"));
    core.write_overlay(-2, 0, WOOD, 1, 14);
    assert_eq!(
        core.resource_snapshot((-2, 0)).map(|row| row.quantity),
        Some(1),
        "one tree back is a field again"
    );

    // So is a paved one. `deposit_quantity` reads zero under paving because paving hides what is
    // there; exhaustion reads the stored quantity instead, so a sealed deposit is never mistaken
    // for a worked-out one and stripping the surface still hands it back.
    core.write_overlay(3, 0, COPPER_ORE, 12, 40);
    core.player.inventory = BTreeMap::from([(item_id(&core, "gravel"), 4)]);
    core.edit_ground(&GroundEdit {
        cover: true,
        ..ground_edit(3, 0, GroundAction::Pave)
    })
    .unwrap();
    assert_eq!(core.deposit_quantity((3, 0)), 0);
    assert!(!core.deposit_exhausted(3, 0));
}

/// A wood gains ground, and only where ground will hold it.
///
/// The spread is fed by recovery rather than by standing wood: a stand that has just filled itself
/// back in is expanding, and an expansion does not stop at the line a cut used to be. So the seed
/// goes to the most sheltered empty hex beside it — and never onto a hex that already holds a
/// deposit, is paved, is built on, or is under water, because those are not empty ground.
#[test]
fn a_healed_stand_seeds_empty_ground_and_refuses_ground_that_is_not_empty() {
    let mut core = game("new-game");
    let cadence = core
        .item_definition(WOOD)
        .unwrap()
        .regrowth_ticks
        .expect("wood regrows");
    let cell = (-3, 1);
    let capacity = core.deposit_quantity(cell);
    let ring: Vec<(i32, i32)> = DIRECTIONS
        .iter()
        .map(|&(dq, dr)| (cell.0 + dq, cell.1 + dr))
        .collect();
    let empty: Vec<(i32, i32)> = ring
        .iter()
        .copied()
        .filter(|&(q, r)| core.buried_field_at(q, r).is_none())
        .collect();
    assert!(
        !empty.is_empty(),
        "the opening stand has bare ground beside it to grow into"
    );

    // Cut one tree and let it come back. Filling the gap is what seeds the next hex.
    core.write_overlay(cell.0, cell.1, WOOD, capacity - 1, capacity);
    core.tick_many(cadence);
    assert_eq!(core.deposit_quantity(cell), capacity);
    let seeded: Vec<(i32, i32)> = empty
        .iter()
        .copied()
        .filter(|&(q, r)| core.deposit_quantity((q, r)) > 0)
        .collect();
    assert_eq!(seeded.len(), 1, "one seed goes out, not a ring of them");
    let seedling = seeded[0];
    assert_eq!(core.deposit_quantity(seedling), 1);
    assert_eq!(
        core.field_at(seedling.0, seedling.1).map(|f| f.item_id),
        Some(WOOD),
        "and the new stand is the flora that seeded it"
    );
    assert!(core.flora_regrowth.contains(&seedling));

    // Ground that is not empty refuses it. Every remaining bare hex is taken — one by paving, one by
    // a building — and the healed stand beside them has nowhere left to put a tree.
    let mut core = game("new-game");
    core.set_creative(true);
    for &(q, r) in &empty {
        assert!(
            core.can_take_root(q, r),
            "bare ground takes root at {q},{r}"
        );
        core.write_overlay(q, r, WOOD, 0, 1);
        assert!(
            !core.can_take_root(q, r),
            "a hex already carrying a deposit is not empty ground at {q},{r}"
        );
    }
    assert_eq!(
        core.colonisation_target(cell.0, cell.1, WOOD),
        None,
        "a stand with no empty ground beside it seeds nothing"
    );
}
