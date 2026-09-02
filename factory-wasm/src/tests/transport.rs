use super::*;

#[test]
fn belts_carry_one_extractors_worth_hold_what_fits_and_report_when_blocked() {
    let mut core = game("factory-demo");
    let container = core
        .entities
        .iter()
        .position(|entity| entity.kind == BuildingKind::Container)
        .unwrap();
    let consumer = core
        .entities
        .iter()
        .position(|entity| entity.kind == BuildingKind::Consumer)
        .unwrap();
    core.graph[container] = Links::single(Some(consumer));
    core.entities[container].inventory.insert(3, 2);
    core.entities[container].inventory.insert(1, 1);
    core.transfer_cargo();
    assert_eq!(core.delivered_by_item.get(&1), Some(&1));
    assert_eq!(core.entities[container].inventory.get(&3), Some(&2));
    core.entities[container].cargo = Some(Cargo {
        item_id: 2,
        quantity: 1,
    });
    let before = core.entities[container].cargo;
    core.graph[container] = Links::default();
    core.transfer_cargo();
    assert_eq!(core.entities[container].cargo, before);

    // An item takes a whole belt's worth of time to cross a belt, and rests on that one belt for
    // every tick of it.
    //
    // The line here is built the way every line is built — from the source outward — which makes
    // ascending entity id run in flow order, which is exactly the arrangement that used to carry
    // an item from the first belt to the last inside a single tick. A hex of belt is 5.37 m of
    // conveyor now, and a conveyor moving two metres a second takes [`BELT_TRANSIT_TICKS`] to get
    // an item across it. The assertion is that the item is on exactly one belt for every one of
    // those ticks: in the lane while it travels, in the exit slot once it has arrived, never in
    // two places and never in none.
    let mut core = empty_world("new-game");
    let first = add_test_belt(&mut core, 0, 0, 0);
    let second = add_test_belt(&mut core, 1, 0, 0);
    let third = add_test_belt(&mut core, 2, 0, 0);
    let sink = add_test_entity(&mut core, 3, 0, 4, 0);
    core.compile_graph();
    assert_eq!(link_ids(&core, first), vec![second]);
    assert_eq!(link_ids(&core, second), vec![third]);
    assert_eq!(link_ids(&core, third), vec![sink]);

    let holding = |core: &Core| -> Vec<u32> {
        [first, second, third]
            .into_iter()
            .filter(|&id| {
                let entity = &core.entities[index_of(core, id)];
                entity.cargo.is_some() || !entity.lane.is_empty()
            })
            .collect()
    };

    put_cargo(&mut core, first, 1);
    for expected in [second, third] {
        for step in 0..BELT_TRANSIT_TICKS {
            core.transfer_cargo();
            core.tick += 1;
            assert_eq!(
                holding(&core),
                vec![expected],
                "the hand-on is immediate and the crossing that follows is not (step {step})"
            );
        }
    }
    core.transfer_cargo();
    assert!(holding(&core).is_empty());
    assert_eq!(
        core.entities[index_of(&core, sink)].inventory.get(&1),
        Some(&1),
        "and three belts later it arrives"
    );

    // A belt line carries what its speed and its item spacing say it carries, and no faster.
    //
    // The measurement is taken at the *end* of a line rather than at the start, because the number
    // that matters to a factory is what comes off a belt, not what a source can be persuaded to
    // push onto one. A container feeding as fast as it is allowed to, across a line long enough
    // for the head to have filled, delivers one item every [`BELT_SLOT_TICKS`] — which is
    // [`scale::belt_items_per_minute`], which is exactly one extractor's output. That ratio is
    // derived rather than tuned: see `scale::belt_cadence_follows_from_speed_and_spacing`.
    let mut core = empty_world("new-game");
    let source = add_test_entity(&mut core, 0, 0, 4, 0);
    let belts: Vec<u32> = (1..=4).map(|q| add_test_belt(&mut core, q, 0, 0)).collect();
    add_test_entity(&mut core, 5, 0, 5, 0);
    core.compile_graph();
    let source_index = index_of(&core, source);
    core.entities[source_index].inventory.insert(1, 10_000);

    // Long enough for the head of the line to have filled and the rate to have settled.
    let warmup = BELT_TRANSIT_TICKS * (belts.len() as u64 + 2);
    for _ in 0..warmup {
        core.transfer_cargo();
        core.tick += 1;
    }
    let before = core.delivered;
    let minute = u64::from(scale::TICKS_PER_SECOND as u32) * 60;
    for _ in 0..minute {
        core.transfer_cargo();
        core.tick += 1;
    }
    assert_eq!(
        core.delivered - before,
        scale::belt_items_per_minute() as u64
    );

    // A blocked belt backs up to exactly the number of items that fit along it, and stops.
    //
    // This is the other half of the cadence: the lane is a length of conveyor, not a queue, so it
    // holds what fits and refuses the rest back up the line. A belt that took an unbounded queue
    // would swallow a jammed factory's whole production and hand it over in one burst when the jam
    // cleared.
    let mut core = empty_world("new-game");
    let source = add_test_entity(&mut core, 0, 0, 4, 0);
    let belt = add_test_belt(&mut core, 1, 0, 0);
    core.compile_graph();
    let source_index = index_of(&core, source);
    core.entities[source_index].inventory.insert(1, 100);

    // The belt points at nothing, so nothing ever leaves it.
    for _ in 0..BELT_TRANSIT_TICKS * 10 {
        core.transfer_cargo();
        core.tick += 1;
    }
    let index = index_of(&core, belt);
    let held = core.entities[index].lane.len() + usize::from(core.entities[index].cargo.is_some());
    assert_eq!(held, BELT_LANE_SLOTS);
    assert_eq!(
        core.entities[source_index].inventory.get(&1),
        Some(&(100 - BELT_LANE_SLOTS as u32)),
        "and the rest never left the source"
    );

    // What a belt is carrying survives a save, and so does where along the belt it is.
    //
    // A lane item holds the tick it stepped on rather than a countdown, which is only sound
    // because the tick it is measured against is saved too. If either half were dropped, a
    // reloaded factory would either teleport a half-crossed line to its far end or strand it: this
    // asserts the crossing resumes exactly where it stopped, by checking the arrival tick rather
    // than merely the item count.
    let mut core = empty_world("new-game");
    let source = add_test_entity(&mut core, 0, 0, 4, 0);
    let belt = add_test_belt(&mut core, 1, 0, 0);
    add_test_entity(&mut core, 2, 0, 5, 0);
    core.compile_graph();
    let source_index = index_of(&core, source);
    core.entities[source_index].inventory.insert(1, 3);

    // Far enough in for the crossing to be visibly unfinished.
    for _ in 0..BELT_TRANSIT_TICKS / 2 {
        core.transfer_cargo();
        core.tick += 1;
    }
    let lane = core.entities[index_of(&core, belt)].lane.clone();
    assert!(!lane.is_empty(), "something is mid-crossing to save");

    let save = core.save_string().unwrap();
    let (definitions, technologies, scenarios) = catalogs();
    let mut restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.tick, core.tick);
    assert_eq!(restored.entities[index_of(&restored, belt)].lane, lane);
    assert_eq!(restored.checksum(), core.checksum());

    // And both run on to the same delivery on the same tick.
    for _ in 0..BELT_TRANSIT_TICKS * 3 {
        core.transfer_cargo();
        core.tick += 1;
        restored.transfer_cargo();
        restored.tick += 1;
        assert_eq!(restored.delivered, core.delivered);
    }
    assert!(core.delivered > 0, "and the line does deliver");
    assert_eq!(restored.checksum(), core.checksum());

    // A loaded belt reports when its output is blocked.
    let mut core = game("factory-demo");
    core.entities.clear();
    core.graph.clear();
    core.next_entity_id = 1;
    let first_id = add_test_belt(&mut core, 0, 0, 0);
    let second_id = add_test_belt(&mut core, 1, 0, 0);
    core.compile_graph();
    let first = core
        .entities
        .iter()
        .position(|entity| entity.id == first_id)
        .unwrap();
    let second = core
        .entities
        .iter()
        .position(|entity| entity.id == second_id)
        .unwrap();
    let cargo = Cargo {
        item_id: 1,
        quantity: 1,
    };
    core.entities[first].cargo = Some(cargo);
    // A full belt downstream, not merely an occupied one: a single item on the next belt is no
    // longer a jam now that a belt is five metres of conveyor with room for five things on it.
    core.entities[second].cargo = Some(cargo);
    core.entities[second].lane = (1..BELT_LANE_SLOTS)
        .map(|_| LaneItem { cargo, entered: 0 })
        .collect();
    assert_eq!(
        core.status_of(first, true, true, true, false),
        EntityStatus::OutputBlocked
    );

    core.entities[second].cargo = None;
    core.entities[second].lane.clear();
    assert_eq!(
        core.status_of(first, true, true, true, false),
        EntityStatus::Carrying
    );

    core.graph[first] = Links::default();
    assert_eq!(
        core.status_of(first, true, true, true, false),
        EntityStatus::OutputBlocked
    );
}

#[test]
fn a_composer_consumes_exact_inputs_and_backpressure_is_exact() {
    let mut core = game("new-game");
    grant_foundations(&mut core);
    core.insight = 8;
    core.research(3).unwrap();
    stock_for(&mut core, 3, 1);
    set_player_hex(&mut core, 1, 3);
    core.place(0, 4, 3, 0, Some(1)).unwrap();
    let composer = core.entity_at(0, 4).unwrap();
    core.graph[composer] = Links::default();
    core.entities[composer].inventory.extend([(11, 1), (19, 1)]);
    core.advance_composer(composer);
    assert!(core.entities[composer].inventory.is_empty());
    assert_eq!(
        core.entities[composer].reserved_inputs,
        BTreeMap::from([(11, 1), (19, 1)])
    );
    assert_eq!(core.entities[composer].cargo, None);
    for _ in 1..8 {
        core.advance_composer(composer);
    }
    assert_eq!(core.entities[composer].output_inventory.get(&2), Some(&1));
    assert!(core.entities[composer].reserved_inputs.is_empty());
    core.advance_composer(composer);
    assert_eq!(core.entities[composer].output_inventory.get(&2), Some(&1));

    // Ingredient capacity is per ingredient, not one pot the ingredients fight over.
    //
    // A composer stores twelve and a component takes an iron plate and a gear. Under the old
    // shared total, twelve iron plates filled the compartment and the gear slot — visibly empty,
    // visibly expected by the recipe — refused everything. Belts stopped delivering gears, the
    // hand refused to place them, and the only way to unwedge the machine was to take plates back
    // out. A four-ingredient recipe like concrete could not hold a working set of anything.
    //
    // Both routes in are pinned, because they used to fail together: `can_accept` is the belt and
    // `store_into` is the hand, and both ask `room_for_stock`.
    let mut core = game("new-game");
    grant_foundations(&mut core);
    core.insight = 8;
    core.research(3).unwrap();
    stock_for(&mut core, 3, 1);
    stock_for(&mut core, 4, 1);
    set_player_hex(&mut core, 1, 3);
    core.place(0, 4, 3, 0, Some(1)).unwrap();
    // Past the composer's own three hexes, which reach east and south of its anchor.
    core.place(2, 4, 4, 0, None).unwrap();
    let composer = core.entity_at(0, 4).unwrap();
    let store = core.entity_at(2, 4).unwrap();
    let capacity = core.building_definition(3).unwrap().capacity.unwrap();

    core.player.inventory.clear();
    core.player.inventory.insert(11, capacity);
    core.store_into(0, 4, StockKind::Input, 11, capacity)
        .unwrap();
    assert_eq!(
        core.entities[composer].input_inventory.get(&11),
        Some(&capacity)
    );

    // The plate slot is full and takes nothing more; the gear slot has the whole capacity.
    assert!(!core.can_accept(
        composer,
        Cargo {
            item_id: 11,
            quantity: 1
        }
    ));
    assert!(core.can_accept(
        composer,
        Cargo {
            item_id: 19,
            quantity: capacity
        }
    ));
    core.player.inventory.insert(19, capacity);
    core.store_into(0, 4, StockKind::Input, 19, capacity)
        .unwrap();
    assert_eq!(
        core.entities[composer].input_inventory.get(&19),
        Some(&capacity)
    );
    assert_eq!(core.player.inventory.get(&19), None);

    // A container's store is still one shared pool: that is the tier decision the player buys,
    // and per-item there would make a tier-one crate hold every item in the game at capacity.
    let shelf = core.building_definition(4).unwrap().capacity.unwrap();
    core.player.inventory.insert(11, shelf);
    core.store_into(2, 4, StockKind::Inventory, 11, shelf)
        .unwrap();
    assert!(!core.can_accept(
        store,
        Cargo {
            item_id: 19,
            quantity: 1
        }
    ));

    // Machine backpressure and consumer totals are exact.
    let mut core = game("factory-demo");
    let extractor = core
        .entities
        .iter()
        .position(|entity| entity.kind == BuildingKind::Extractor)
        .unwrap();
    core.graph[extractor] = Links::default();
    let resource_before = core.deposit_quantity((-4, 0));
    core.tick_many(400);
    let capacity = core.building_definition(1).unwrap().capacity.unwrap();
    assert_eq!(
        core.entities[extractor].output_inventory.get(&9),
        Some(&capacity)
    );
    assert_eq!(core.deposit_quantity((-4, 0)), resource_before - capacity);
    let container = core
        .entities
        .iter()
        .position(|entity| entity.kind == BuildingKind::Container)
        .unwrap();
    let consumer = core
        .entities
        .iter()
        .position(|entity| entity.kind == BuildingKind::Consumer)
        .unwrap();
    core.entities[container].inventory.insert(16, 7);
    core.graph[container] = Links::single(Some(consumer));
    for _ in 0..7 {
        core.transfer_cargo();
    }
    assert_eq!(core.delivered_by_item.get(&16), Some(&7));
    assert!(core.entities[container].inventory.is_empty());
}

/// A splitter compiles three outputs and serves them in rotation.
///
/// Both halves matter and they fail differently. Three edges is the graph claim — the flanks
/// are 60° either side of the facing and nothing else — and consecutive items leaving by
/// different branches is the tick claim. A splitter that compiled three edges but always
/// offered the first would be a belt that had learned to draw two extra decks.
#[test]
fn splitters_mergers_and_underpasses_serve_their_lanes_in_order() {
    let mut core = empty_world("new-game");
    let splitter = add_test_entity(&mut core, 0, 0, 24, 0);
    // Facing east, and the two headings 60° either side of east.
    let ahead = add_test_entity(&mut core, 1, 0, 4, 0);
    let left = add_test_entity(&mut core, 0, 1, 4, 0);
    let right = add_test_entity(&mut core, 1, -1, 4, 0);
    core.compile_graph();

    let mut expected = vec![ahead, left, right];
    expected.sort_unstable();
    assert_eq!(
        link_ids(&core, splitter),
        expected,
        "facing and both flanks"
    );

    // Three items, one per tick, so nothing is ever refused for want of room.
    for _ in 0..3 {
        put_cargo(&mut core, splitter, 1);
        core.transfer_cargo();
    }
    for target in [ahead, left, right] {
        assert_eq!(
            core.entities[index_of(&core, target)].inventory.get(&1),
            Some(&1),
            "every branch takes exactly one of three"
        );
    }

    // A jammed branch does not stall the others: the cursor stays where it is on a refusal
    // rather than advancing past a branch that took nothing.
    let capacity = core.building_definition(4).unwrap().capacity.unwrap();
    let jammed = index_of(&core, ahead);
    core.entities[jammed].inventory.insert(1, capacity);
    for _ in 0..2 {
        put_cargo(&mut core, splitter, 1);
        core.transfer_cargo();
    }
    assert_eq!(
        core.entities[index_of(&core, ahead)].inventory[&1],
        capacity
    );
    assert_eq!(core.entities[index_of(&core, left)].inventory[&1], 2);
    assert_eq!(core.entities[index_of(&core, right)].inventory[&1], 2);

    // A merger serves its feeders in rotation, and an ordinary belt in the same junction does not.
    //
    // The negative half is the whole point. Several lanes pointed into one hex compete every tick,
    // and the id order the game has always arbitrated by hands the win to the same lane forever —
    // which is a starved lane, not a tie-break. The merger is the definition that answers it, so
    // the test states both behaviours side by side rather than asserting the fair one alone.
    let served_order = |definition_id: DefinitionId| {
        let mut core = empty_world("new-game");
        let junction = add_test_entity(&mut core, 0, 0, definition_id, 0);
        let west = add_test_belt(&mut core, -1, 0, 0);
        let north = add_test_belt(&mut core, 0, -1, 1);
        let sink = add_test_entity(&mut core, 1, 0, 4, 0);
        core.compile_graph();
        assert_eq!(link_ids(&core, west), vec![junction]);
        assert_eq!(link_ids(&core, north), vec![junction]);
        assert_eq!(link_ids(&core, junction), vec![sink]);

        // Both lanes full every tick, so who goes first is arbitration and never availability.
        (0..4)
            .map(|_| {
                put_cargo(&mut core, west, 1);
                put_cargo(&mut core, north, 1);
                core.transfer_cargo();
                let served = if core.entities[index_of(&core, west)].cargo.is_none() {
                    west
                } else {
                    north
                };
                // The junction is emptied by hand rather than by ticks of transfers. What it
                // was handed is now on its lane with 5.37 m still to cross, and this test asks
                // which feeder wins the hex, not how long the cargo then spends on it. Left
                // loaded, every round after the first would be answered by the lane's spacing
                // rule instead of by the rotation.
                let junction_index = index_of(&core, junction);
                core.entities[junction_index].cargo = None;
                core.entities[junction_index].lane.clear();
                served
            })
            .collect::<Vec<u32>>()
    };

    // Feeders are walked from the one after the one served last, so two lanes alternate.
    let merger = served_order(25);
    assert_eq!(merger[0], merger[2]);
    assert_eq!(merger[1], merger[3]);
    assert_ne!(merger[0], merger[1], "a merger alternates");

    // The same junction built as an ordinary belt lets the lower entity id win every tick.
    let belt = served_order(2);
    assert_eq!(belt[0], belt[1]);
    assert_eq!(
        belt[0], belt[3],
        "an ordinary junction starves the other lane"
    );

    // Two underpasses on one heading carry a lane beneath the line between them.
    //
    // The crossed belt is the assertion: it keeps its own cargo, keeps its own output, and never
    // sees what passes over it. And the pair is not a placement mode — the exit is simply the
    // underpass that found no partner ahead of it, so it delivers like any other belt, and an
    // underpass alone behaves as one.
    let mut core = empty_world("new-game");
    let entrance = add_test_entity(&mut core, 0, 0, 26, 0);
    let exit = add_test_entity(&mut core, 2, 0, 26, 0);
    let landing = add_test_entity(&mut core, 3, 0, 4, 0);
    // The lane being crossed: it runs north through the hex between the pair.
    let crossed = add_test_belt(&mut core, 1, 0, 1);
    let crossed_sink = add_test_entity(&mut core, 1, 1, 4, 0);
    core.compile_graph();

    assert_eq!(
        link_ids(&core, entrance),
        vec![exit],
        "the entrance passes over the belt it crosses and binds to its partner"
    );
    assert_eq!(
        link_ids(&core, exit),
        vec![landing],
        "the exit found no partner ahead, so it delivers like any belt"
    );
    assert_eq!(link_ids(&core, crossed), vec![crossed_sink]);

    put_cargo(&mut core, entrance, 1);
    put_cargo(&mut core, crossed, 3);
    // A crossing is two hexes of travel: the entrance hands to its partner at once, and the
    // partner delivers once the cargo has crossed it — the same wait every belt in a line takes.
    for _ in 0..=BELT_TRANSIT_TICKS {
        core.transfer_cargo();
        core.tick += 1;
    }
    assert_eq!(
        core.entities[index_of(&core, landing)].inventory.get(&1),
        Some(&1),
        "the crossing cargo arrives on the far side"
    );
    assert_eq!(
        core.entities[index_of(&core, crossed)].cargo,
        None,
        "the crossed belt handed on its own cargo and never took the one passing over it"
    );
    assert_eq!(
        core.entities[index_of(&core, crossed_sink)]
            .inventory
            .get(&3),
        Some(&1),
        "and the crossed lane delivered its own, untouched"
    );

    // The hexes a crossing spans stay ordinary: the covered belt is a normal entity there, and
    // taking the partner away leaves the entrance an ordinary belt that binds to it.
    let removed = index_of(&core, exit);
    core.entities.remove(removed);
    core.compile_graph();
    assert_eq!(
        link_ids(&core, entrance),
        vec![crossed],
        "an underpass with no partner is a belt"
    );

    // One underpass drag places only a clear atomic pair around the crossing.
    let mut core = empty_world("new-game");
    core.set_creative(true);
    core.player.build_range = 1 << 20;
    set_player_hex(&mut core, 0, 0);
    let crossed = add_test_belt(&mut core, 3, 0, 1);
    core.compile_graph();

    let preview = core.line_preview((2, 0), (4, 0), 26, 0, None);
    assert_eq!(
        preview
            .iter()
            .map(|cell| (cell.q, cell.r))
            .collect::<Vec<_>>(),
        vec![(2, 0), (4, 0)],
        "the occupied middle is a tunnel span, not a placement"
    );
    assert!(preview.iter().all(|cell| cell.legal));
    core.place_line((2, 0), (4, 0), 26, 0, None).unwrap();

    let entrance = core.entity_at(2, 0).unwrap();
    let exit = core.entity_at(4, 0).unwrap();
    assert_eq!(core.entity_at(3, 0), Some(index_of(&core, crossed)));
    assert_eq!(core.entities[entrance].placed.orientation, 0);
    assert_eq!(core.entities[exit].placed.orientation, 0);
    assert_eq!(core.graph[entrance].primary(), Some(exit));

    // Fresh belts and pipes keep solids and fluids apart and tanks are filtered.
    let mut core = empty_world("new-game");
    let belt_id = add_test_entity(&mut core, 0, 0, 2, 0);
    let pipe_id = add_test_entity(&mut core, 1, 0, 32, 0);
    // A tank covers every hex within one of its anchor, so the two of them stand three apart
    // and the pipe hands into the western rim of the first rather than into its anchor.
    let water_tank_id = add_test_entity(&mut core, 3, 0, 34, 0);
    let oil_tank_id = add_test_entity(&mut core, 6, 0, 35, 0);
    let belt = index_of(&core, belt_id);
    let pipe = index_of(&core, pipe_id);
    let water_tank = index_of(&core, water_tank_id);
    let oil_tank = index_of(&core, oil_tank_id);

    assert!(core.can_accept(
        belt,
        Cargo {
            item_id: 1,
            quantity: 1
        }
    ));
    assert!(!core.can_accept(
        belt,
        Cargo {
            item_id: 10,
            quantity: 1
        }
    ));
    assert!(!core.can_accept(
        pipe,
        Cargo {
            item_id: 1,
            quantity: 1
        }
    ));
    assert!(core.can_accept(
        pipe,
        Cargo {
            item_id: 10,
            quantity: 1
        }
    ));
    assert!(core.can_accept(
        water_tank,
        Cargo {
            item_id: 10,
            quantity: 1
        }
    ));
    assert!(!core.can_accept(
        water_tank,
        Cargo {
            item_id: 28,
            quantity: 1
        }
    ));
    assert!(core.can_accept(
        oil_tank,
        Cargo {
            item_id: 28,
            quantity: 1
        }
    ));
    assert!(!core.can_accept(
        oil_tank,
        Cargo {
            item_id: 10,
            quantity: 1
        }
    ));

    core.compile_graph();
    assert!(link_ids(&core, belt_id).is_empty());
    assert_eq!(link_ids(&core, pipe_id), vec![water_tank_id]);
    put_cargo(&mut core, pipe_id, 10);
    core.transfer_cargo();
    assert_eq!(
        core.entities[water_tank].inventory.get(&10),
        Some(&1),
        "a pipe hands loose water into the filtered tank"
    );

    core.legacy_fluid_belts.insert(belt_id);
    assert!(core.can_accept(
        belt,
        Cargo {
            item_id: 10,
            quantity: 1
        }
    ));

    // A drag routes on all twelve headings, and takes the two-row period when it pays.
    //
    // Straight up the world column is the case that separates the search from the six-edge one it
    // replaced: four rows north is two corner steps or four edge steps, and the corner route is
    // the shorter run in entities even though the two price out the same. Research is what decides
    // which one the player gets, and the search reads it rather than branching on it.
    let mut core = game("new-game");
    // Raw rather than `set_creative`, which researches everything — and what is researched is
    // exactly the variable this test turns.
    core.creative = true;
    core.researched.insert(1);

    // Four rows north of (2, 0): `NORTH` is `(1, -2)`, so the destination is two of them.
    let locked = core.drag_route((2, 0), (4, -4), 2, 0, None);
    assert_eq!(
        locked.len(),
        5,
        "with the reach locked, a pure column is four edge steps"
    );
    assert!(
        locked
            .windows(2)
            .all(|pair| step_direction(pair[0], pair[1]).is_some_and(|step| step < NORTH)),
        "and every one of them is an edge, because no other heading was offered"
    );

    core.researched.insert(11);
    let unlocked = core.drag_route((2, 0), (4, -4), 2, 0, None);
    assert_eq!(
        unlocked,
        vec![(2, 0), (3, -2), (4, -4)],
        "researched, the same drag is two steps of the two-row period"
    );
    for pair in unlocked.windows(2) {
        assert_eq!(step_direction(pair[0], pair[1]), Some(NORTH));
    }

    // Due east is an edge heading, and no amount of research makes a corner step cheaper there.
    let east = core.drag_route((2, 0), (5, 0), 2, 0, None);
    assert_eq!(east, vec![(2, 0), (3, 0), (4, 0), (5, 0)]);
}
