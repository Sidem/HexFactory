use super::*;

#[test]
fn extractor_stops_exactly_when_its_deposit_empties() {
    let mut core = game("new-game");
    core.researched.insert(2);
    stock_for(&mut core, 1, 1);
    set_player_hex(&mut core, 3, 1);
    core.write_overlay(3, 0, 1, 2, 48);
    core.place(3, 0, 1, 0, None).unwrap();
    // Iron's own figure, not the building's cadence: a tier-one extractor spends 30 ticks on
    // one unit of ore, which is twice what the hand spends on the same cell.
    for _ in 0..2 {
        core.tick_many(30);
        let index = core
            .entities
            .iter()
            .position(|entity| entity.placed.q == 3)
            .unwrap();
        assert_eq!(core.entities[index].output_inventory.get(&1), Some(&1));
        core.entities[index].output_inventory.clear();
    }
    core.tick_many(100);
    let entity = core
        .entities
        .iter()
        .find(|entity| entity.placed.q == 3)
        .unwrap();
    assert_eq!(core.deposit_quantity((3, 0)), 0);
    assert_eq!(core.produced.get(&1), Some(&2));
    assert_eq!(entity.progress, 0);

    // Resolved deposit references match a full tile scan and survive generation.
    let mut core = game("new-game");
    core.researched.insert(2);
    stock_for(&mut core, 1, 1);
    set_player_hex(&mut core, 3, 1);
    core.place(3, 0, 1, 0, None).unwrap();
    let index = core
        .entities
        .iter()
        .position(|entity| entity.placed.q == 3 && entity.placed.r == 0)
        .unwrap();
    let scan = |core: &Core| {
        let (x, y) = axial_world(3, 0);
        core.resource_at_world(x, y)
    };

    let expected = scan(&core);
    assert_eq!(core.extractor_deposit(index), expected);
    assert_eq!(expected, Some((3, 0)));
    // The second lookup is served from the cache and must not drift from the scan.
    assert_eq!(core.extractor_deposit(index), scan(&core));
    assert_eq!(core.deposit_links.len(), 1);

    // Generating tiles invalidates every resolved reference, and the extractor re-resolves.
    core.generate_chunk(-9, 7);
    assert!(core.deposit_links.is_empty());
    assert_eq!(core.extractor_deposit(index), scan(&core));

    // A drained field cell falls through to the scan's next choice without re-resolving.
    core.write_overlay(3, 0, 1, 0, 48);
    assert_eq!(core.extractor_deposit(index), scan(&core));
    assert_eq!(core.extractor_deposit(index), None);

    // Erasing the extractor releases its entry rather than leaking one per placement.
    core.erase(3, 0).unwrap();
    assert!(core.deposit_links.is_empty());
}

#[test]
fn research_is_atomic_published_delta_tracked_and_paid_for_in_insight() {
    let mut core = game("new-game");
    let insight = core.insight;
    assert!(core.research(1).unwrap_err().contains("Prove the line"));
    assert!(core.researched.is_empty());
    set_player_hex(&mut core, 0, -1);
    core.player.inventory.insert(2, 3);
    core.deposit_inventory().unwrap();
    assert_eq!(core.contract_stage, 1);
    assert_eq!(core.insight, insight);
    for id in [1, 2, 4, 8] {
        assert!(core.researched.contains(&id), "granted technology {id}");
    }
    assert!(core.events.iter().any(|event| event.contains("grants")));
    assert!(core
        .technology(1)
        .unwrap()
        .building_unlocks()
        .eq([2].into_iter()));
    core.player.inventory.insert(24, 1);
    core.place(2, 0, 2, 0, None).unwrap();
    assert!(core.research(1).unwrap_err().contains("already researched"));
    core.insight = 8;
    core.research(3).unwrap();
    assert_eq!(core.insight, 0);
    assert!(core.researched.contains(&3));

    // Research is atomic validates prerequisites and unlocks.
    let mut core = game("new-game");
    core.insight = 20;
    assert!(core.research(3).unwrap_err().contains("prerequisites"));
    assert_eq!(core.insight, 20);
    grant_foundations(&mut core);
    core.research(3).unwrap();
    assert_eq!(core.insight, 12);
    core.player.inventory.insert(24, 1);
    core.place(2, 0, 2, 0, None).unwrap();
    assert!(core.research(3).is_err());

    // Published research availability is the atomic purchase answer.
    for insight in [0, 2, 3, 100] {
        for prerequisite in [false, true] {
            for technology in &catalogs().1.technologies {
                let mut core = game("new-game");
                core.insight = insight;
                if prerequisite {
                    core.researched.extend([1, 2, 4, 5, 8]);
                }
                let row = core.research_availability(technology);
                assert_eq!(row.technology_id, technology.id);
                assert_eq!(
                    row.insight_shortfall,
                    u64::from(technology.cost).saturating_sub(insight)
                );
                let expected = technology.purchasable()
                    && !row.complete
                    && row.missing_prerequisites.is_empty()
                    && row.insight_shortfall == 0;
                let before = core.checksum();
                assert_eq!(core.research(technology.id).is_ok(), expected);
                if expected {
                    assert_eq!(core.insight, insight - u64::from(technology.cost));
                    assert!(core.research_availability(technology).complete);
                    let paid = core.checksum();
                    assert!(core.research(technology.id).is_err());
                    assert_eq!(core.checksum(), paid);
                } else {
                    assert_eq!(core.checksum(), before);
                }
            }
        }
    }

    // Research availability deltas follow income purchases and creative without quiet resends.
    let mut factory = test_factory("new-game");
    let _ = factory.snapshot_json();
    let quiet = factory.snapshot_delta_json();
    assert!(!quiet.contains("research_availability"));
    let mut previous = factory.core.snapshot();
    grant_foundations(&mut factory.core);
    factory.core.insight = 6;
    assert_delta_matches_full_diff(
        &mut factory,
        &mut previous,
        "first research becomes affordable",
    );
    factory.core.research(5).unwrap();
    assert_delta_matches_full_diff(
        &mut factory,
        &mut previous,
        "purchase consumes insight and opens prerequisites",
    );
    factory.core.set_creative(true);
    assert_delta_matches_full_diff(&mut factory, &mut previous, "creative grants research");
    factory.core.set_creative(false);
    assert_delta_matches_full_diff(
        &mut factory,
        &mut previous,
        "leaving creative keeps knowledge",
    );
    assert!(!factory
        .snapshot_delta_json()
        .contains("research_availability"));

    // Skills permanently expand cargo space and build range.
    let mut core = game("new-game");
    core.insight = 100;
    let starting_slots = core.player.carry_slots;
    let starting_range = core.player.build_range;

    grant_foundations(&mut core);
    core.observe_skill_event(SkillEvent::WorkshopCraft);
    core.purchase_skill(1).unwrap();
    assert_eq!(core.player.carry_slots, starting_slots + 4);
    core.observe_skill_event(SkillEvent::PoweredCraft);
    core.purchase_skill(2).unwrap();
    assert_eq!(core.player.build_range, starting_range + 3 * HEX_X as u32);

    let save = core.save_string().unwrap();
    let (definitions, technologies, scenarios) = catalogs();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.player.carry_slots, starting_slots + 4);
    assert_eq!(
        restored.player.build_range,
        starting_range + 3 * HEX_X as u32
    );
}

#[test]
fn compiling_is_incremental_and_matches_the_full_graph() {
    let mut core = bare_game("factory-demo");
    core.power_unmetered = false;
    let mut index = core
        .entities
        .iter()
        .position(|entity| (entity.placed.q, entity.placed.r) == (-4, 0))
        .unwrap();
    let mut path = Vec::new();
    loop {
        path.push((core.entities[index].placed.q, core.entities[index].placed.r));
        let Some(next) = core.graph[index].primary() else {
            break;
        };
        index = next;
    }
    // The chain is the same chain, one hop shorter at two of its links: the extractor stands on
    // (-3, 0) and the cutter on (0, 1), so the belts that used to occupy those hexes are gone
    // and the machines hand straight to what follows them.
    assert_eq!(
        path,
        vec![(-4, 0), (-2, 0), (-2, 1), (-1, 1), (1, 1), (2, 1), (3, 1)]
    );
    core.tick_many(400);
    let produced = core.produced.get(&WOOD).copied().unwrap_or(0);
    let stock_in_system = |item: ItemId| -> u64 {
        core.entities
            .iter()
            .map(|entity| {
                // Everything the belt is holding, not only its exit slot: an item halfway along
                // a lane is still in the factory, and leaving it out would make the conveyor
                // look like a place where timber goes missing.
                Core::belt_contents(entity)
                    .filter(|cargo| cargo.item_id == item)
                    .map(|cargo| u64::from(cargo.quantity))
                    .sum::<u64>()
                    + u64::from(entity.inventory.get(&item).copied().unwrap_or(0))
                    + u64::from(entity.input_inventory.get(&item).copied().unwrap_or(0))
                    + u64::from(entity.fuel_inventory.get(&item).copied().unwrap_or(0))
                    + u64::from(entity.output_inventory.get(&item).copied().unwrap_or(0))
                    + u64::from(entity.reserved_inputs.get(&item).copied().unwrap_or(0))
            })
            .sum()
    };
    let delivered = core.delivered_by_item.get(&16).copied().unwrap_or(0);
    assert_eq!(
        produced * 2,
        stock_in_system(WOOD) * 2 + stock_in_system(16) + delivered
    );
    assert!(
        delivered > 0,
        "the metered demo must deliver timber, not merely hold cargo"
    );

    // Incremental recompile matches full graph and skips unrelated components.
    let mut core = game("factory-demo");
    add_test_belt(&mut core, 100, 100, 0);
    add_test_belt(&mut core, 101, 100, 0);
    core.compile_graph();

    let index = core
        .entities
        .iter()
        .position(|entity| (entity.placed.q, entity.placed.r) == (-2, 0))
        .unwrap();
    let old_links = core.graph_links_by_id();
    let id = core.entities[index].id;
    let changed_cells = BTreeSet::from([(-2, 0)]);
    core.entities[index].placed.orientation = 1;

    let recompiled =
        core.recompile_graph_components(&old_links, &changed_cells, &BTreeSet::from([id]));
    assert!(recompiled > 0);
    assert!(recompiled < core.entities.len());
    let incremental = core.graph_links_by_id();
    core.compile_graph();
    assert_eq!(core.graph_links_by_id(), incremental);
    assert_eq!(
        incremental.get(&(core.next_entity_id - 2)),
        old_links.get(&(core.next_entity_id - 2))
    );

    // Incremental recompile handles component splits and merges.
    let mut core = game("new-game");
    core.entities.clear();
    core.graph.clear();
    core.next_entity_id = 1;
    let left = add_test_belt(&mut core, 0, 0, 0);
    let bridge = add_test_belt(&mut core, 1, 0, 0);
    let right = add_test_belt(&mut core, 2, 0, 0);
    core.compile_graph();
    assert_eq!(sole_link(&core.graph_links_by_id(), left), Some(bridge));
    assert_eq!(sole_link(&core.graph_links_by_id(), bridge), Some(right));

    let old_links = core.graph_links_by_id();
    let bridge_index = core
        .entities
        .iter()
        .position(|entity| entity.id == bridge)
        .unwrap();
    core.entities.remove(bridge_index);
    let changed_cells = BTreeSet::from([(1, 0)]);
    let recompiled =
        core.recompile_graph_components(&old_links, &changed_cells, &BTreeSet::from([bridge]));
    assert_eq!(recompiled, 2);
    assert_eq!(sole_link(&core.graph_links_by_id(), left), None);
    let incremental_split = core.graph_links_by_id();
    core.compile_graph();
    assert_eq!(core.graph_links_by_id(), incremental_split);

    let old_links = core.graph_links_by_id();
    let replacement = add_test_belt(&mut core, 1, 0, 0);
    let recompiled =
        core.recompile_graph_components(&old_links, &changed_cells, &BTreeSet::from([replacement]));
    assert_eq!(recompiled, 3);
    assert_eq!(
        sole_link(&core.graph_links_by_id(), left),
        Some(replacement)
    );
    assert_eq!(
        sole_link(&core.graph_links_by_id(), replacement),
        Some(right)
    );
    let incremental_merge = core.graph_links_by_id();
    core.compile_graph();
    assert_eq!(core.graph_links_by_id(), incremental_merge);

    // Runtime indexes match the blueprint after full and incremental compiles.
    fn assert_index(core: &Core) {
        assert_eq!(core.runtime.occupied, core.occupied_entities());

        let mut order: Vec<usize> = (0..core.entities.len()).collect();
        order.sort_by_key(|&index| core.entities[index].id);
        assert_eq!(core.runtime.entity_order, order);
        assert_eq!(
            core.runtime.transport_order,
            order
                .iter()
                .copied()
                .filter(|&index| !core.graph[index].is_empty())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            core.runtime.machine_order,
            order
                .iter()
                .copied()
                .filter(|&index| matches!(
                    core.entities[index].kind,
                    BuildingKind::Extractor | BuildingKind::Composer | BuildingKind::Pump
                ))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            core.runtime.power_order,
            order
                .iter()
                .copied()
                .filter(|&index| core.power_of[index].is_some())
                .collect::<Vec<_>>()
        );
        for target in 0..core.entities.len() {
            let expected = order
                .iter()
                .copied()
                .filter(|&source| core.graph[source].iter().any(|value| value == target))
                .collect::<Vec<_>>();
            assert_eq!(core.runtime.feeders[target], expected);
        }
    }

    let mut core = game("factory-demo");
    assert_index(&core);
    let index = core
        .entities
        .iter()
        .position(|entity| entity.kind == BuildingKind::Belt)
        .unwrap();
    let old_links = core.graph_links_by_id();
    let id = core.entities[index].id;
    let cell = (core.entities[index].placed.q, core.entities[index].placed.r);
    core.entities[index].placed.orientation = (core.entities[index].placed.orientation + 1) % 6;
    core.recompile_graph_components(&old_links, &BTreeSet::from([cell]), &BTreeSet::from([id]));
    assert_index(&core);
}
