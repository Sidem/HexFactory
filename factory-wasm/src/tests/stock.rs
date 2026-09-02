use super::*;

/// Transport is bought a batch at a time, and the price boundary that introduced kits conserves.
///
/// A line used to be paid for one raw ore per segment, so laying belt never touched the factory
/// it existed to serve. The kit puts a plate and a length of timber behind every segment and
/// hands four back at once, which is what makes a long run affordable without making a short one
/// free. Every transport building is billed the same way, so no member of the family is a cheaper
/// spelling of another.
///
/// The other half is compatibility. `erase_refund` quotes the *current* bill, so a belt bought
/// under definition 16 hands back a kit rather than the ore that paid for it. That is exactly
/// what rebuilding it costs — dismantling and relaying a legacy line is still free — and no
/// recipe turns a kit back into ore, so the boundary cannot be farmed for raw material.
#[test]
fn the_pack_is_a_slot_rule_that_transport_erasure_and_withdrawal_all_obey() {
    let (definitions, technologies, scenarios) = catalogs();
    let core = game("new-game");

    // One batch: a plate and a length of timber for four kits, by hand or by machine.
    let recipe = core.recipe(15).expect("the kit recipe").clone();
    assert_eq!(recipe.output.item_id, 24);
    assert_eq!(recipe.output.quantity, 4);
    assert!(core
        .building_definition(28)
        .unwrap()
        .supports_recipe(&recipe));
    assert!(core
        .building_definition(3)
        .unwrap()
        .supports_recipe(&recipe));

    // Belt, splitter, merger and underpass are all billed in kits, and a vertex heading still
    // costs strictly more than the edge one it would otherwise dominate.
    for definition_id in [2, 24, 25, 26] {
        let building = core.building_definition(definition_id).unwrap();
        let kits = |orientation: u8| {
            building
                .cost_at(orientation)
                .iter()
                .find(|cost| cost.item_id == 24)
                .map(|cost| cost.quantity)
                .unwrap_or(0)
        };
        assert!(kits(0) > 0, "{} is billed in kits", building.key);
        assert!(
            kits(NORTH) > kits(0),
            "{} pays extra for the two-row reach",
            building.key
        );
    }

    // A factory built when a belt cost one ore, read back under the revised catalog.
    let (mut legacy, _, _) = catalogs();
    let belt = legacy
        .buildings
        .iter_mut()
        .find(|building| building.id == 2)
        .unwrap();
    belt.construction_cost = vec![Ingredient {
        item_id: 1,
        quantity: 1,
    }];
    belt.corner_construction_cost = Some(vec![Ingredient {
        item_id: 1,
        quantity: 2,
    }]);
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|scenario| scenario.key == "new-game")
        .unwrap();
    let mut old = Core::new(&legacy, &technologies, scenario, None, None).unwrap();
    old.researched.insert(1);
    old.player.inventory.insert(1, 1);
    set_player_hex(&mut old, 1, 3);
    old.place(0, 3, 2, 0, None).unwrap();
    assert_eq!(old.player.inventory.get(&1).copied().unwrap_or(0), 0);

    let save = old.save_string().unwrap();
    let mut restored =
        Core::from_save(&definitions, &technologies, &scenarios, &save).expect("legacy factory");
    restored.erase(0, 3).unwrap();
    assert_eq!(
        restored.player.inventory.get(&1).copied().unwrap_or(0),
        0,
        "the boundary mints no raw material"
    );
    assert_eq!(restored.player.inventory.get(&24), Some(&1));
    // And that refund is exactly a rebuild, so a legacy line can still be moved for nothing.
    restored.place(0, 3, 2, 0, None).unwrap();
    assert_eq!(restored.player.inventory.get(&24).copied().unwrap_or(0), 0);

    // One overlap rule answers both placement questions.
    // Fields are hex cells. Placement and the extractor's cached candidates share
    // `field_covered_at`, so a resolved reference cannot drift from the rule that allowed
    // the building. Cliffs occupy their own hex and do not make the neighbour unbuildable.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 4]);
    stock_for(&mut core, 1, 1);

    let (hex_x, hex_y) = axial_world(3, 0);
    set_player_hex(&mut core, 3, 1);
    assert!(
        core.resource_at_world(hex_x, hex_y).is_some(),
        "a field cell must be reachable from its own hex"
    );
    core.place(3, 0, 1, 0, None).unwrap();

    let index = core.entity_at(3, 0).unwrap();
    assert_eq!(core.extractor_deposit(index), Some((3, 0)));
    assert_eq!(
        core.deposit_candidates(3, 0, EXTRACT_RADIUS),
        core.deposit_links[&core.entities[index].id]
    );

    let mut ground = legacy_band_game("new-game");
    ground.researched.extend([1, 2, 3, 4]);
    ground.player.inventory.insert(24, 20);
    // The clearing's own blocked hex is (2, 1) — the landing cliff at (1, -1) is under the hub's
    // seven hexes now — and the lowland beside it stays buildable.
    assert!(ground.terrain_blocks_construction(2, 1));
    ground.place(2, 0, 2, 0, None).unwrap();
    assert!(ground
        .place(2, 1, 2, 0, None)
        .unwrap_err()
        .contains("environment"));

    // Carrying capacity is a slot rule over the ordinary inventory.
    let mut core = game("new-game");
    let slots = core.player.carry_slots;
    assert!(slots > 0);
    let stack = core.stack_size(1);

    // Capacity is expressed in stacks of the item's own size, not in item count.
    core.player.inventory.insert(1, stack);
    assert_eq!(core.slots_used(&core.player.inventory), 1);
    core.player.inventory.insert(1, stack + 1);
    assert_eq!(core.slots_used(&core.player.inventory), 2);
    assert_eq!(core.player_room_for(1), (slots - 2) * stack + stack - 1);

    // Filling the pack refuses further gathering rather than silently overflowing it.
    core.player.inventory.insert(1, slots * stack);
    assert_eq!(core.player_room_for(1), 0);
    set_player_hex(&mut core, 3, 0);
    assert!(core.gather().unwrap_err().contains("capacity"));
    // A different item has no room either, because every slot is spoken for.
    assert_eq!(core.player_room_for(3), 0);

    // The stacks the host draws come from native, one entry per occupied slot.
    core.player.inventory.insert(1, stack + 3);
    core.player.inventory.insert(3, 1);
    assert_eq!(
        core.carry_stacks(),
        vec![
            Ingredient {
                item_id: 1,
                quantity: stack
            },
            Ingredient {
                item_id: 1,
                quantity: 3
            },
            Ingredient {
                item_id: 3,
                quantity: 1
            },
        ]
    );

    // A full pack no longer refuses a demolition, and nothing is destroyed when it does not.
    //
    // The refusal sounded protective and was not: a full pack and a full container had no order of
    // operations that emptied either, so the building the player wanted gone simply stayed. The
    // recovery splits instead — what fits is carried, what does not falls at the site — and the
    // removal preview promises the same thing, so a drag cannot show a cell it will refuse on
    // release.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 4]);
    stock_for(&mut core, 4, 1);
    set_player_hex(&mut core, 1, 0);
    core.place(2, 0, 4, 0, None).unwrap();
    let index = core.entity_at(2, 0).unwrap();
    core.entities[index].inventory.insert(3, 9);

    // A pack with no room left in it at all.
    let stack = core.stack_size(1);
    core.player
        .inventory
        .insert(1, core.player.carry_slots * stack);
    let held_before = core.player.inventory.clone();
    assert!(core
        .erase_line_preview((2, 0), (2, 0))
        .iter()
        .all(|cell| cell.legal));

    core.erase(2, 0).unwrap();
    assert_eq!(
        core.player.inventory, held_before,
        "nothing was carried, because nothing could be"
    );
    assert_eq!(
        core.ground_items
            .iter()
            .map(|item| ((item.q, item.r), item.item_id, item.quantity))
            .collect::<Vec<_>>(),
        vec![((2, 0), 3, 9), ((2, 0), 16, 3)],
        "and nothing was destroyed either: the whole recovery is on the ground at the site"
    );

    // With room, the same recovery comes back to the pack and leaves no litter.
    core.player.inventory.clear();
    core.ground_items.clear();
    stock_for(&mut core, 4, 1);
    core.place(2, 0, 4, 0, None).unwrap();
    let rebuilt = core.entity_at(2, 0).unwrap();
    core.entities[rebuilt].inventory.insert(3, 9);
    core.erase(2, 0).unwrap();
    assert_eq!(core.player.inventory.get(&16), Some(&3));
    assert_eq!(core.player.inventory.get(&3), Some(&9));
    assert!(core.ground_items.is_empty());

    // Withdrawing moves what fits and leaves the rest in the container.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 4]);
    stock_for(&mut core, 4, 1);
    set_player_hex(&mut core, 1, 0);
    core.place(2, 0, 4, 0, None).unwrap();
    let index = core.entity_at(2, 0).unwrap();
    core.entities[index].inventory.insert(2, 12);

    // Out of range, a building with no reachable store, and an item the container does not
    // hold are all refused. The hub is the interesting refusal: it has an intake, but that
    // intake is the contract, not a shelf.
    assert!(core.withdraw(2, 0, 1, 1).unwrap_err().contains("none"));
    assert!(core.withdraw(9, 9, 2, 1).unwrap_err().contains("range"));
    assert!(core
        .withdraw(0, 0, 2, 1)
        .unwrap_err()
        .contains("no stock you can reach"));

    // The request is a ceiling: what moves is limited by the stock and by carrying space.
    core.withdraw(2, 0, 2, 5).unwrap();
    assert_eq!(core.player.inventory.get(&2), Some(&5));
    assert_eq!(core.entities[index].inventory.get(&2), Some(&7));

    // Filling the pack stops the transfer without destroying what stayed behind.
    let stack = core.stack_size(1);
    core.player
        .inventory
        .insert(1, core.player.carry_slots * stack);
    core.player.inventory.remove(&2);
    assert!(core.withdraw(2, 0, 2, 7).unwrap_err().contains("capacity"));
    assert_eq!(core.entities[index].inventory.get(&2), Some(&7));

    // A partial withdrawal takes exactly what the part-filled stack still has room for, and
    // says how much moved rather than pretending the request was met.
    core.player
        .inventory
        .insert(1, (core.player.carry_slots - 1) * stack);
    core.player.inventory.insert(2, 6);
    core.withdraw(2, 0, 2, 99).unwrap();
    assert_eq!(core.player.inventory.get(&2), Some(&core.stack_size(2)));
    assert_eq!(core.entities[index].inventory.get(&2), Some(&3));
    assert_eq!(core.events.last().unwrap(), "Withdrew 4 × Component");
}

/// The hand reaches into working machines, not only into boxes.
///
/// Before v0.24 a burner was a one-way slot: coal went in, and the only way to get it back was
/// to demolish the building. That made a mis-aimed belt permanently expensive and made the
/// obvious recovery — take the fuel back out and put it somewhere useful — impossible. This
/// pins the rule that replaced it: the four kinds that hold stock a player can see are the four
/// a player can reach into, in both directions, and a firebox is one of them.
#[test]
fn stock_moves_between_hand_machine_and_container_without_leaving_native_state() {
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 5]);
    core.player.build_range = 1 << 20;
    core.player.inventory.insert(6, 20);
    core.player.inventory.insert(8, 40);
    set_player_hex(&mut core, 0, 3);
    stock_for(&mut core, 8, 1);
    core.place(0, 4, 8, 0, Some(6)).unwrap();
    let kiln = core.entity_at(0, 4).unwrap();

    core.player.inventory.clear();
    core.player.inventory.insert(8, 16);
    core.player.inventory.insert(5, 16);
    core.store_into(0, 4, StockKind::Input, 8, 16).unwrap();
    core.store_into(0, 4, StockKind::Fuel, 5, 16).unwrap();
    assert_eq!(core.entities[kiln].input_inventory.get(&8), Some(&16));
    assert_eq!(core.entities[kiln].fuel_inventory.get(&5), Some(&16));

    core.tick_many(100);
    assert_eq!(core.entities[kiln].output_inventory.get(&14), Some(&15));
    assert_eq!(core.entities[kiln].input_inventory.get(&8), Some(&6));
    assert_eq!(
        core.status_of(kiln, true, true, true, false),
        EntityStatus::OutputBlocked
    );

    // Cursor stack moves all half and single without leaving native state.
    let (definitions, technologies, scenarios) = catalogs();
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 5]);
    core.player.build_range = 1 << 20;
    core.player.inventory.insert(6, 20);
    core.player.inventory.insert(8, 20);
    core.player.inventory.insert(5, 11);
    set_player_hex(&mut core, 0, 3);
    stock_for(&mut core, 8, 1);
    core.place(0, 4, 8, 0, Some(6)).unwrap();

    core.pickup_player_stack(5, 6).unwrap();
    assert_eq!(
        core.player.hand,
        Some(Cargo {
            item_id: 5,
            quantity: 6
        })
    );
    core.place_building_stack(0, 4, StockKind::Fuel, 1).unwrap();
    assert_eq!(
        core.player.hand,
        Some(Cargo {
            item_id: 5,
            quantity: 5
        })
    );
    core.place_building_stack(0, 4, StockKind::Fuel, 5).unwrap();
    assert_eq!(core.player.hand, None);
    let kiln = core.entity_at(0, 4).unwrap();
    assert_eq!(core.entities[kiln].fuel_inventory.get(&5), Some(&6));

    core.pickup_building_stack(0, 4, StockKind::Fuel, 5, 3)
        .unwrap();
    assert_eq!(
        core.player.hand,
        Some(Cargo {
            item_id: 5,
            quantity: 3
        })
    );
    core.player.build_range = core.scenario.build_range.saturating_mul(HEX_X as u32);
    let saved = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &saved).unwrap();
    assert_eq!(restored.player.hand, core.player.hand);
    assert_eq!(restored.checksum(), core.checksum());

    // A hand reaches into the machines that hold stock.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 8]);
    core.player.build_range = 1 << 20;
    stock_for(&mut core, 13, 1);
    core.player.inventory.insert(24, 20);
    set_player_hex(&mut core, 0, 0);
    core.place(3, 0, 13, 0, None).unwrap();
    core.place(5, 0, 2, 0, None).unwrap();
    let burner = core.entity_at(3, 0).unwrap();
    let capacity = core.building_definition(13).unwrap().capacity.unwrap();
    assert_eq!(capacity, 12, "the firebox is bounded, not a well");

    // A firebox takes fuel by hand and gives it back — the recovery that demolition used to be
    // the only route to.
    core.player.inventory.clear();
    core.player.inventory.insert(5, 20);
    core.store(3, 0, 5, 999).unwrap();
    assert_eq!(
        core.entities[burner].fuel_inventory.get(&5),
        Some(&capacity)
    );
    assert_eq!(core.player.inventory.get(&5), Some(&(20 - capacity)));
    // Bounded: the thirteenth lump has nowhere to go and says so.
    assert!(core.store(3, 0, 5, 1).unwrap_err().contains("full"));
    core.withdraw(3, 0, 5, 5).unwrap();
    assert_eq!(
        core.entities[burner].fuel_inventory.get(&5),
        Some(&(capacity - 5))
    );

    // A refusal distinguishes "wrong item" from "no space": ore is not fuel, and a burner that
    // cannot burn it should never have been able to swallow it.
    core.player.inventory.insert(1, 5);
    assert!(core.store(3, 0, 1, 1).unwrap_err().contains("no use for"));
    // A belt is a lane, not a shelf. Nothing to reach into, in either direction.
    assert!(core
        .store(5, 0, 5, 1)
        .unwrap_err()
        .contains("no stock you can reach"));
    assert!(core
        .withdraw(5, 0, 5, 1)
        .unwrap_err()
        .contains("no stock you can reach"));

    // The switch is a pause, not a partial demolition.
    //
    // A burner with coal in it burns that coal whether or not anything downstream wants the power,
    // so "stop this machine while I rebuild the line it feeds" had no answer except erasing it and
    // paying to rebuild. This pins the answer: switched off is real saved state, it suspends the
    // work *and* the draw, it keeps everything the machine was holding, and switching back on
    // resumes rather than restarts.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 8]);
    // Kept, because a save is only valid at the scenario's own reach: the long arm is a
    // scaffold for building the scene, and the scene has to be put back before it is saved.
    let scenario_reach = core.player.build_range;
    core.player.build_range = 1 << 20;
    stock_for(&mut core, 13, 1);
    core.player.inventory.insert(24, 20);
    set_player_hex(&mut core, 0, 0);
    core.place(3, 0, 13, 0, None).unwrap();
    core.place(5, 0, 2, 0, None).unwrap();
    let burner = core.entity_at(3, 0).unwrap();
    core.player.inventory.clear();
    core.player.inventory.insert(5, 12);
    core.store(3, 0, 5, 12).unwrap();

    // Only work can be switched: a belt has none, so the toggle refuses rather than lying.
    assert!(core
        .set_enabled(5, 0, false)
        .unwrap_err()
        .contains("no work to switch off"));
    // Bounded and range-checked like every other edit.
    core.player.build_range = scenario_reach;
    assert!(core
        .set_enabled(99, 99, false)
        .unwrap_err()
        .contains("range"));
    core.player.build_range = 1 << 20;

    core.set_enabled(3, 0, false).unwrap();
    assert!(core.entities[burner].disabled);
    // The flags say "fuelled, powered, running well" — the switch still wins, because it is
    // the one status the player chose rather than one the factory fell into.
    assert_eq!(
        core.status_of(burner, true, true, true, false),
        EntityStatus::SwitchedOff
    );
    assert_eq!(core.events.last().unwrap(), "Switched Burner generator off");
    // Idempotent by construction: the command carries the state it wants, so a doubled press
    // is refused instead of flipping the machine back on.
    assert!(core
        .set_enabled(3, 0, false)
        .unwrap_err()
        .contains("already switched off"));

    // The point of the switch: a stopped burner stops eating.
    let fuel_before = core.entities[burner]
        .fuel_inventory
        .get(&5)
        .copied()
        .unwrap();
    let charge_before = core.entities[burner].fuel_charge;
    core.tick_many(200);
    assert_eq!(
        core.entities[burner]
            .fuel_inventory
            .get(&5)
            .copied()
            .unwrap(),
        fuel_before,
        "a switched-off burner burns nothing"
    );
    assert_eq!(core.entities[burner].fuel_charge, charge_before);

    // And it survives a save, because a factory that silently restarted on reload would be a
    // worse bug than the one the switch fixes.
    let (definitions, technologies, scenarios) = catalogs();
    core.player.build_range = scenario_reach;
    let saved = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &saved).unwrap();
    let reloaded = restored.entity_at(3, 0).unwrap();
    assert!(restored.entities[reloaded].disabled);
    assert_eq!(restored.checksum(), core.checksum());

    // Switching back on resumes: the fuel that was held is still there to burn.
    core.player.build_range = 1 << 20;
    core.set_enabled(3, 0, true).unwrap();
    assert_eq!(core.events.last().unwrap(), "Switched Burner generator on");
    assert_ne!(
        core.status_of(burner, true, true, true, false),
        EntityStatus::SwitchedOff
    );
    assert_eq!(
        core.entities[burner]
            .fuel_inventory
            .get(&5)
            .copied()
            .unwrap(),
        fuel_before
    );
}

/// An upgrade grows a building in place: contents, heading, and connections all survive, and
/// the ladder conserves items exactly. The round trip is the assertion that matters — an
/// upgrade that paid out more than it took in would be a duplication exploit, which is the
/// same failure `erase`'s carry-then-spill split exists to prevent: every item is either in the
/// pack or on the ground, and none is in both.
#[test]
fn an_upgrade_preserves_contents_and_reach_storing_and_gathering_stay_bounded() {
    let mut core = game("new-game");
    core.researched.extend([1, 4, 12]);
    // Everything the ladder can possibly charge, so the test measures conservation and not
    // whether the player happened to be able to afford a step.
    for item_id in [1, 3, 6, 11, 16, 19, 24, 25] {
        core.player.inventory.insert(item_id, 60);
    }
    core.player.carry_slots = 99;
    let before = core.player.inventory.clone();

    set_player_hex(&mut core, 1, 3);
    core.place(0, 3, 4, 2, None).unwrap();
    // Give it contents and a downstream connection to preserve.
    let index = core.entity_at(0, 3).unwrap();
    let id = core.entities[index].id;
    core.entities[index].inventory.insert(5, 9);
    core.place(0, 4, 2, 0, None).unwrap();
    let linked_before = core.graph[core.entity_at(0, 4).unwrap()];

    core.upgrade(0, 3).unwrap();

    let index = core.entity_at(0, 3).unwrap();
    assert_eq!(
        core.entities[index].id, id,
        "the entity is edited, not replaced"
    );
    assert_eq!(core.entities[index].placed.definition_id, 20);
    assert_eq!(
        core.entities[index].placed.orientation, 2,
        "heading survives"
    );
    assert_eq!(
        core.entities[index].inventory.get(&5),
        Some(&9),
        "stock survives"
    );
    assert_eq!(
        core.graph[core.entity_at(0, 4).unwrap()],
        linked_before,
        "the belt feeding it still points at it"
    );
    assert!(core.events.iter().any(|event| event.contains("Upgraded")));

    // The ladder ends: a tier with no `upgrades_to` says so rather than failing quietly.
    assert!(core
        .upgrade(0, 3)
        .unwrap_err()
        .contains("already at its highest tier"));

    // Round trip. Erasing the upgraded container hands back exactly the sum of both payments,
    // so the player's pack returns to where it started — plus only the stock the container was
    // holding, which erase has always returned and which no step of the ladder created.
    core.erase(0, 3).unwrap();
    core.erase(0, 4).unwrap();
    let mut expected = before.clone();
    *expected.entry(5).or_default() += 9;
    assert_eq!(
        core.player.inventory, expected,
        "place → upgrade → erase must be item-neutral"
    );

    // The same holds for the reach ladder, which charges a different item set.
    let mut ore = game("new-game");
    ore.researched.extend([1, 2, 12]);
    for item_id in [1, 3, 6, 11, 16, 19, 20] {
        ore.player.inventory.insert(item_id, 60);
    }
    ore.player.carry_slots = 99;
    let before = ore.player.inventory.clone();
    // Clear of the ground the deeper tier grows onto: it takes (3, 1) as well as the two hexes
    // the first tier stands on.
    set_player_hex(&mut ore, 4, 1);
    ore.place(3, 0, 1, 0, None).unwrap();
    ore.upgrade(3, 0).unwrap();
    assert_eq!(
        ore.entities[ore.entity_at(3, 0).unwrap()]
            .placed
            .definition_id,
        19
    );
    ore.erase(3, 0).unwrap();
    assert_eq!(ore.player.inventory, before);

    // Reach is the flagship upgrade, so it has to be a number the definition owns — and the hand
    // must not inherit it. The predicate stays single; only its argument moves.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 12]);
    stock_for(&mut core, 1, 1);
    stock_for(&mut core, 19, 1);
    // Clear of the ground the deeper tier grows onto.
    set_player_hex(&mut core, 4, 1);
    core.place(3, 0, 1, 0, None).unwrap();

    let shallow = core.entity_at(3, 0).unwrap();
    core.extractor_deposit(shallow);
    let shallow_reach = core.deposit_links[&core.entities[shallow].id].clone();
    assert_eq!(shallow_reach, core.deposit_candidates(3, 0, 1));

    core.upgrade(3, 0).unwrap();
    assert_eq!(
        core.deposit_links.get(&core.entities[shallow].id),
        None,
        "a change of reach must drop the list resolved against the old one"
    );
    let deep = core.entity_at(3, 0).unwrap();
    core.extractor_deposit(deep);
    let deep_reach = core.deposit_links[&core.entities[deep].id].clone();
    assert_eq!(deep_reach, core.deposit_candidates(3, 0, 2));
    assert!(
        deep_reach.len() >= shallow_reach.len(),
        "a deeper extractor can only ever cover more"
    );
    assert_eq!(core.extract_radius_of(1), EXTRACT_RADIUS);
    assert_eq!(core.extract_radius_of(19), 2);
    assert_eq!(core.building_definition(1).unwrap().extract_radius, Some(1));
    assert_eq!(
        core.building_definition(11).unwrap().extract_radius,
        Some(1)
    );
    assert_eq!(core.player_snapshot().extract_radius, EXTRACT_RADIUS as u32);

    // The hand is unchanged. A gather still reaches exactly one hex, whatever is built on it.
    let (x, y) = axial_world(3, 0);
    let by_hand = core.resource_at_world(x, y);
    assert!(by_hand.map_or(true, |cell| axial_distance((3, 0), cell) <= EXTRACT_RADIUS));

    // And a definition may not claim an unbounded arm.
    let (mut definitions, _, _) = catalogs();
    let index = definitions
        .buildings
        .iter()
        .position(|building| building.id == 19)
        .unwrap();
    definitions.buildings[index].extract_radius = Some(MAX_EXTRACT_RADIUS + 1);
    assert!(validate_definitions(&definitions)
        .unwrap_err()
        .contains("reach in 1..="));

    // A right-click names the hex. That is a different thing from facing-weighted targeting, and
    // the difference is the whole reason this is allowed: the player chose the cell, on screen,
    // so the number that moves is the one they pointed at. Reach is unchanged.
    let mut core = game("new-game");
    set_player_hex(&mut core, 3, 0);
    // Field cells either side of the one underfoot, so a target that drifts is visible.
    core.write_overlay(4, 0, 1, 20, 20);
    core.write_overlay(2, 0, 1, 20, 20);

    // The untargeted gather still takes from the hex underfoot.
    core.gather().unwrap();
    cooldown(&mut core);
    assert_eq!(core.deposit_quantity((3, 0)), 47);

    // The named one takes from the neighbour that was named, and leaves the rest alone.
    core.gather_at(4, 0).unwrap();
    cooldown(&mut core);
    assert_eq!(
        (
            core.deposit_quantity((2, 0)),
            core.deposit_quantity((3, 0)),
            core.deposit_quantity((4, 0)),
        ),
        (20, 47, 19)
    );

    // Reach is the same predicate, so a hex an extractor here could not cover is refused.
    assert!(core.gather_at(6, 0).unwrap_err().contains("out of reach"));
    // So is ground that holds no field at all.
    assert!(core.gather_at(3, 1).unwrap_err().contains("out of reach"));
    // And the cooldown is the one cooldown, shared by both.
    core.gather_at(4, 0).unwrap();
    assert!(core.gather_at(2, 0).unwrap_err().contains("cooling down"));
    cooldown(&mut core);

    // A worked-out cell is refused rather than underflowed.
    core.write_overlay(2, 0, 1, 0, 20);
    assert!(core.gather_at(2, 0).unwrap_err().contains("worked out"));

    // Signal crystal is in the world, and the hand still cannot take it.
    cooldown(&mut core);
    core.write_overlay(4, 0, CRYSTAL, 8, 8);
    let refusal = core.gather_at(4, 0).unwrap_err();
    assert!(
        refusal.contains("cannot be gathered by hand"),
        "crystal refusal was {refusal}"
    );
    assert!(
        refusal.contains("extractor"),
        "name the machine, got {refusal}"
    );
    assert_eq!(core.deposit_quantity((4, 0)), 8);
    assert!(core.player.inventory.get(&CRYSTAL).is_none());

    // Every reachable field cell is nameable, and nothing outside the reach is.
    let origin = (3, 0);
    for &(dq, dr) in &DIRECTIONS {
        for steps in 1..=2 {
            let cell = (origin.0 + dq * steps, origin.1 + dr * steps);
            if core.field_at(cell.0, cell.1).is_none() {
                continue;
            }
            cooldown(&mut core);
            let can_hand = core
                .field_at(cell.0, cell.1)
                .and_then(|res| core.item_definition(res.item_id))
                .is_some_and(|i| i.hand_gather_steps.is_some());
            let named = core.gather_at(cell.0, cell.1).is_ok();
            assert_eq!(
                named,
                core.field_covered_at(origin, cell, EXTRACT_RADIUS)
                    && core.deposit_quantity(cell) > 0
                    && can_hand,
                "named gather at {cell:?} disagreed with the shared reach predicate"
            );
        }
    }

    // Loading a container by hand is the exact mirror of unloading one, on the same contract:
    // the quantity is a ceiling, a partial move succeeds, and nothing is ever destroyed.
    let mut core = game("new-game");
    core.researched.extend([1, 4]);
    core.player.inventory.insert(1, 30);
    stock_for(&mut core, 4, 1);
    set_player_hex(&mut core, 1, 3);
    core.place(0, 3, 4, 0, None).unwrap();
    let capacity = core.building_definition(4).unwrap().capacity.unwrap();

    // A ceiling, not a demand: asking for more than the container can hold moves what fits.
    core.store(0, 3, 1, 999).unwrap();
    let index = core.entity_at(0, 3).unwrap();
    assert_eq!(core.entities[index].inventory.get(&1), Some(&capacity));
    // Conservation: what left the pack is exactly what arrived. The box is billed in timber
    // rather than ore now, so the thirty the pack started with are all still accounted for.
    assert_eq!(
        core.player.inventory.get(&1).copied().unwrap_or(0) + capacity,
        30
    );
    assert!(core.events.iter().any(|event| event.contains("Stored")));

    // A full container refuses rather than silently dropping the overflow.
    assert!(core.store(0, 3, 1, 1).unwrap_err().contains("full"));
    // And the round trip is exact.
    let carried = core.player.inventory.get(&1).copied().unwrap_or(0);
    core.withdraw(0, 3, 1, capacity).unwrap();
    assert_eq!(core.player.inventory.get(&1), Some(&(carried + capacity)));
    assert_eq!(
        core.entities[index].inventory.get(&1).copied().unwrap_or(0),
        0
    );

    // Only what the player is actually carrying, and only into something actually there.
    assert!(core
        .store(0, 3, 99, 1)
        .unwrap_err()
        .contains("not carrying"));
    assert!(core
        .store(2, 3, 1, 1)
        .unwrap_err()
        .contains("nothing to reach into"));
    // Bounded and range-checked like every other edit.
    assert!(core.store(9, 9, 1, 1).unwrap_err().contains("build range"));

    // Negative coordinates use euclidean chunk division.
    assert_eq!(floor_div(-1, 8), -1);
    assert_eq!(floor_div(-8, 8), -1);
    assert_eq!(floor_div(-9, 8), -2);
}

#[test]
fn dropped_items_land_are_picked_up_despawn_and_survive_a_save() {
    let mut core = game("new-game");
    set_player_hex(&mut core, 0, 0);
    core.player.hand = Some(Cargo {
        item_id: 1,
        quantity: 10,
    });

    // Dropping onto an adjacent passable hex
    core.drop_player_stack(0, 1, 6).unwrap();
    assert_eq!(
        core.player.hand,
        Some(Cargo {
            item_id: 1,
            quantity: 4
        })
    );
    assert_eq!(core.ground_items.len(), 1);
    assert_eq!(core.ground_items[0].q, 0);
    assert_eq!(core.ground_items[0].r, 1);
    assert_eq!(core.ground_items[0].item_id, 1);
    assert_eq!(core.ground_items[0].quantity, 6);
    assert_eq!(
        core.ground_items[0].despawn_tick,
        GROUND_ITEM_LIFETIME_TICKS
    );

    // Dropping more onto the same hex stacks and refreshes despawn tick
    core.advance_ticks(50);
    core.drop_player_stack(0, 1, 4).unwrap();
    assert_eq!(core.player.hand, None);
    assert_eq!(core.ground_items.len(), 1);
    assert_eq!(core.ground_items[0].quantity, 10);
    assert_eq!(
        core.ground_items[0].despawn_tick,
        50 + GROUND_ITEM_LIFETIME_TICKS
    );

    // Gathering at hex picks up ground item
    core.gather_at(0, 1).unwrap();
    assert_eq!(core.player.inventory.get(&1), Some(&10));
    assert_eq!(core.ground_items.len(), 0);

    // Drop again to test auto-collect on walk and despawn
    core.player.hand = Some(Cargo {
        item_id: 1,
        quantity: 5,
    });
    core.drop_player_stack(0, 1, 5).unwrap();
    assert_eq!(core.ground_items.len(), 1);

    // Advance 30 ticks past the drop cooldown and walk over (0, 1)
    core.advance_ticks(30);
    core.player.move_x = 100;
    set_player_hex(&mut core, 0, 1);
    core.advance_player_steps(1);
    assert_eq!(core.ground_items.len(), 0);
    assert_eq!(core.player.inventory.get(&1), Some(&15));

    // Test despawn after 600 ticks
    core.player.hand = Some(Cargo {
        item_id: 2,
        quantity: 3,
    });
    core.drop_player_stack(0, 1, 3).unwrap();
    assert_eq!(core.ground_items.len(), 1);
    // 599 ticks: still there
    core.advance_ticks(599);
    assert_eq!(core.ground_items.len(), 1);
    // 1 more tick (600 ticks total): despawned
    core.advance_ticks(1);
    assert_eq!(core.ground_items.len(), 0);

    // Ground items save and restore.
    let (definitions, technologies, scenarios) = catalogs();
    let mut core = game("new-game");
    set_player_hex(&mut core, 0, 0);
    core.player.hand = Some(Cargo {
        item_id: 1,
        quantity: 7,
    });
    core.drop_player_stack(0, 1, 7).unwrap();
    let before_ground = core.ground_items.clone();
    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.ground_items, before_ground);
}

/// An item with two honest destinations reaches both, and a full ingredient buffer no longer
/// starves the firebox behind it.
///
/// Coal is billed by `Smelt steel` *and* burns, and one resolver had to answer for both. Select
/// `Melt glass` and coal was fuel; select `Smelt steel` and the same lump became an ingredient, so
/// a named Fuel slot refused it — a player saw one smelter take coal and its neighbour refuse it
/// for no visible reason. Worse, automatic delivery could only ever answer `Input`, so a belt of
/// coal filled the ingredient buffer to capacity and then backed up with an empty firebox: a
/// belt-fed steel smelter could not run at all.
///
/// The precedence is not the defect and does not move. Feeding steel still fills its bill before
/// its firebox. What changed is that precedence stops speaking for a compartment the player named,
/// and yields once the compartment it prefers is full.
#[test]
fn an_item_that_is_both_ingredient_and_fuel_reaches_either_compartment_by_hand_and_by_belt() {
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 5, 8]);
    core.player.build_range = 1 << 20;
    set_player_hex(&mut core, 0, 3);
    stock_for(&mut core, 7, 1);
    core.place(0, 5, 7, 0, Some(5)).unwrap();
    let smelter = core.entity_at(0, 5).unwrap();

    // By hand, into the slot the player pointed at, with steel selected.
    core.player.inventory.clear();
    core.player.inventory.insert(5, 24);
    core.store_into(0, 5, StockKind::Fuel, 5, 4).unwrap();
    core.store_into(0, 5, StockKind::Input, 5, 4).unwrap();
    assert_eq!(core.entities[smelter].fuel_inventory.get(&5), Some(&4));
    assert_eq!(core.entities[smelter].input_inventory.get(&5), Some(&4));

    // The cursor stack obeys the same rule the hand does.
    core.pickup_player_stack(5, 2).unwrap();
    core.place_building_stack(0, 5, StockKind::Fuel, 2).unwrap();
    assert_eq!(core.entities[smelter].fuel_inventory.get(&5), Some(&6));

    // Automatic delivery is unchanged while the bill can still take the item: inputs first.
    core.store(0, 5, 5, 2).unwrap();
    assert_eq!(core.entities[smelter].input_inventory.get(&5), Some(&6));
    assert_eq!(core.entities[smelter].fuel_inventory.get(&5), Some(&6));

    // A withdrawal with no named compartment finds the coal wherever it actually sits.
    core.entities[smelter].input_inventory.clear();
    core.withdraw(0, 5, 5, 6).unwrap();
    assert_eq!(core.entities[smelter].fuel_inventory.get(&5), None);

    // And precedence yields once the ingredient buffer is full, so a belt keeps feeding the
    // firebox instead of backing up against a machine that could never light.
    let capacity = core
        .building_definition(7)
        .and_then(|definition| definition.capacity)
        .unwrap();
    core.entities[smelter].input_inventory.insert(5, capacity);
    core.entities[smelter].fuel_inventory.clear();
    let cargo = Cargo {
        item_id: 5,
        quantity: 1,
    };
    assert!(core.can_accept(smelter, cargo));
    assert_eq!(
        core.delivery_stock_for_item(smelter, 5, 1),
        Some(StockKind::Fuel)
    );
    core.accept(smelter, cargo);
    assert_eq!(core.entities[smelter].fuel_inventory.get(&5), Some(&1));

    // With both compartments full the belt still backs up rather than voiding the cargo.
    core.entities[smelter].fuel_inventory.insert(5, capacity);
    assert!(!core.can_accept(smelter, cargo));

    // Glass bills no coal, so coal there is fuel and nothing else — an ingredient slot that never
    // wanted it still says so.
    core.entities[smelter].input_inventory.clear();
    core.entities[smelter].fuel_inventory.clear();
    core.set_recipe(0, 5, 4).unwrap();
    assert!(core.stock_admits_item(smelter, StockKind::Fuel, 5));
    assert!(!core.stock_admits_item(smelter, StockKind::Input, 5));

    // A burner-only machine is untouched: it has a firebox and no bill, and iron ore is neither.
    core.player.inventory.clear();
    stock_for(&mut core, 13, 1);
    let (q, r) = try_place_near(&mut core, (0, 3), 13);
    let generator = core.entity_at(q, r).unwrap();
    assert!(core.stock_admits_item(generator, StockKind::Fuel, 5));
    assert!(!core.stock_admits_item(generator, StockKind::Input, 5));
    assert!(!core.stock_admits_item(generator, StockKind::Fuel, 1));
}
