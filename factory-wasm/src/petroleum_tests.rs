use super::*;

fn test_core() -> (Core, ScenariosInput) {
    let definitions =
        serde_json::from_str(include_str!("../../src/data/definitions.json")).unwrap();
    let technologies =
        serde_json::from_str(include_str!("../../src/data/technologies.json")).unwrap();
    let mut scenarios: ScenariosInput =
        serde_json::from_str(include_str!("../../src/data/scenarios.json")).unwrap();
    let scenario = &mut scenarios.scenarios[0];
    scenario.generated_environment = false;
    scenario.buildings.clear();
    scenario.resources.clear();
    scenario.initial_inventory.clear();
    // Petroleum machines stand on real ground now — a refinery alone covers nineteen hexes — so
    // the player waits well clear of every plot and reaches it from there. Neither reach nor where
    // someone happens to be standing is what these tests are about.
    scenario.player_spawn = Coordinate { q: 0, r: 8 };
    scenario.build_range = 32;
    let mut core = Core::new(&definitions, &technologies, scenario, None, None).unwrap();
    core.set_creative(true);
    core.power_unmetered = true;
    (core, scenarios)
}

fn at(core: &Core, q: i32, r: i32) -> usize {
    core.entities
        .iter()
        .position(|entity| entity.placed.q == q && entity.placed.r == r)
        .unwrap()
}

#[test]
fn petroleum_joint_batch_waits_for_all_outputs_and_resumes_without_losing_inputs() {
    let (mut core, scenarios) = test_core();
    core.place(2, 0, 30, 0, Some(18)).unwrap();
    let index = at(&core, 2, 0);
    core.entities[index].input_inventory.insert(28, 8);
    core.entities[index].output_inventory.insert(30, 22);
    let before = core.checksum();
    core.advance_composer(index);
    assert_eq!(
        core.checksum(),
        before,
        "two free slots cannot hold a four-unit batch"
    );
    assert!(!core.power_work_wanted(index));
    core.subtract_stock(index, StockKind::Output, 30, 2);
    core.advance_composer(index);
    assert_eq!(core.entities[index].input_inventory[&28], 4);
    assert_eq!(core.entities[index].reserved_inputs[&28], 4);
    let save = core.save_string().unwrap();
    let mut restored =
        Core::from_save(&core.definitions, &core.technologies, &scenarios, &save).unwrap();
    restored.power_unmetered = true;
    for _ in 0..60 {
        core.advance_composer(index);
        restored.advance_composer(index);
    }
    assert_eq!(core.checksum(), restored.checksum());
    assert_eq!(core.entities[index].output_inventory[&29], 2);
    assert_eq!(core.entities[index].output_inventory[&30], 22);
    assert_eq!(core.entities[index].input_inventory[&28], 4);
    assert!(core.entities[index].reserved_inputs.is_empty());
    core.subtract_stock(index, StockKind::Output, 30, 4);
    for _ in 0..30 {
        core.advance_composer(index);
    }
    assert_eq!(core.entities[index].output_inventory[&29], 4);
    assert_eq!(core.entities[index].output_inventory[&30], 20);
    assert!(core.entities[index].input_inventory.is_empty());
}

#[test]
fn petroleum_well_refuses_other_fields_and_ordinary_extractors_refuse_oil() {
    let (mut core, _) = test_core();
    core.write_overlay(2, 0, CRUDE_OIL, 20, 20);
    core.write_overlay(2, -1, IRON_ORE, 20, 20);
    assert!(core
        .place(2, 0, 1, 0, None)
        .unwrap_err()
        .contains("oil well"));
    assert!(core
        .place(2, -1, 29, 0, None)
        .unwrap_err()
        .contains("Crude oil"));
    core.place(2, 0, 29, 0, None).unwrap();
    let index = at(&core, 2, 0);
    assert_eq!(core.extractor_deposit(index), Some((2, 0)));
    for _ in 0..400 {
        core.advance_extractor(index);
    }
    assert_eq!(
        core.entities[index].output_inventory.get(&CRUDE_OIL),
        Some(&20)
    );
    assert_eq!(core.deposit_quantity((2, -1)), 20);
    assert_eq!(core.extractor_deposit(index), None);
}

#[test]
fn petroleum_joint_outputs_and_reserved_jobs_are_refunded_and_dirty_tracked() {
    let (mut core, scenarios) = test_core();
    core.place(2, 0, 30, 0, Some(18)).unwrap();
    core.set_creative(false);
    let index = at(&core, 2, 0);
    core.entities[index].input_inventory.insert(28, 8);
    let mut factory = Factory {
        definitions: core.definitions.clone(),
        technologies: core.technologies.clone(),
        scenarios,
        core,
        snapshot_revision: 0,
        baseline: None,
    };
    factory.build_delta();
    let mut previous = factory.core.snapshot();
    for _ in 0..32 {
        factory.core.advance_composer(index);
        let current = factory.core.snapshot();
        let oracle = SnapshotDelta::between(
            factory.snapshot_revision,
            factory.snapshot_revision + 1,
            &previous,
            &current,
        );
        let actual = factory.build_delta();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(oracle).unwrap()
        );
        previous = current;
    }
    assert_eq!(factory.core.entities[index].reserved_inputs[&28], 4);
    assert_eq!(factory.core.entities[index].output_inventory[&30], 2);
    factory.core.erase(2, 0).unwrap();
    assert_eq!(factory.core.player.inventory.get(&28), None);
    assert!(factory
        .core
        .ground_items
        .iter()
        .any(|item| item.item_id == 28 && item.quantity == 4));
    assert_eq!(factory.core.player.inventory[&29], 2);
    assert_eq!(factory.core.player.inventory[&30], 2);
}

#[test]
fn refinery_products_leave_independent_exterior_footprint_ports_and_round_trip() {
    let (mut core, scenarios) = test_core();
    let container = core
        .definitions
        .buildings
        .iter()
        .find(|building| building.key == "container")
        .unwrap()
        .id;
    core.place(2, 0, 30, 0, Some(18)).unwrap();
    // The refinery covers every hex within two of its anchor, so its ports are on that rim and the
    // buildings they feed stand a hex beyond it.
    core.place(5, 0, container, 0, None).unwrap();
    core.place(-1, 2, container, 0, None).unwrap();
    assert_eq!(
        core.set_output_route(2, 0, 29, 2, 0, 3).unwrap_err(),
        "output port is on an internal footprint seam"
    );
    core.set_output_route(2, 0, 29, 4, 0, 0).unwrap();
    core.set_output_route(0, 1, 30, 0, 1, 2).unwrap();
    let refinery = at(&core, 2, 0);
    core.entities[refinery].output_inventory.insert(29, 2);
    core.entities[refinery].output_inventory.insert(30, 2);

    let routes = core.entity_snapshot(refinery).output_routes;
    assert_eq!(
        routes,
        vec![
            OutputRouteSnapshot {
                item_id: 29,
                q: 4,
                r: 0,
                direction: 0,
                target_id: Some(core.entities[at(&core, 5, 0)].id),
            },
            OutputRouteSnapshot {
                item_id: 30,
                q: 0,
                r: 1,
                direction: 2,
                target_id: Some(core.entities[at(&core, -1, 2)].id),
            },
        ]
    );
    for _ in 0..4 {
        core.transfer_cargo();
    }
    assert_eq!(core.entities[at(&core, 5, 0)].inventory.get(&29), Some(&2));
    assert_eq!(core.entities[at(&core, -1, 2)].inventory.get(&30), Some(&2));
    assert!(core.entities[refinery].output_inventory.is_empty());

    let save = core.save_string().unwrap();
    let mut restored =
        Core::from_save(&core.definitions, &core.technologies, &scenarios, &save).unwrap();
    let restored_refinery = at(&restored, 2, 0);
    assert_eq!(
        restored.entity_snapshot(restored_refinery).output_routes,
        routes
    );
    assert_eq!(restored.checksum(), core.checksum());
}

#[test]
fn petroleum_powered_chain_routes_both_products_and_makes_asphalt() {
    let (mut core, _) = test_core();
    core.power_unmetered = false;
    // The same chain as before, spaced around what each machine now stands on: the well's five
    // hexes hand east into the refinery's nineteen, which hand east into the splitter, which feeds
    // the mixer ahead of it and the burner beside it. Every link is still one hex of contact.
    core.write_overlay(-5, 1, CRUDE_OIL, 40, 40);
    core.place(-5, 1, 29, 0, None).unwrap();
    core.place(-1, 1, 30, 0, Some(18)).unwrap();
    core.place(2, 1, 24, 0, None).unwrap();
    core.place(4, 0, 31, 0, Some(19)).unwrap();
    let generator = core
        .definitions
        .buildings
        .iter()
        .find(|building| building.key == "burner-generator")
        .unwrap()
        .id;
    let pole = core
        .definitions
        .buildings
        .iter()
        .find(|building| building.key == "pole")
        .unwrap()
        .id;
    let container = core
        .definitions
        .buildings
        .iter()
        .find(|building| building.key == "container")
        .unwrap()
        .id;
    core.place(2, 2, generator, 0, None).unwrap();
    // Two poles reach the whole line: the western one lights the well and the refinery, the eastern
    // one the splitter, the mixer and the burner, and they are five hexes apart against a reach of
    // six.
    core.place(-2, -1, pole, 0, None).unwrap();
    core.place(2, 0, pole, 0, None).unwrap();
    core.place(6, 0, container, 0, None).unwrap();
    let mixer = at(&core, 4, 0);
    let burner = at(&core, 2, 2);
    core.entities[mixer].input_inventory.insert(17, 18);
    core.entities[burner].fuel_inventory.insert(COAL, 4);
    core.advance_ticks(1200);
    let stored = &core.entities[at(&core, 6, 0)].inventory;
    assert!(
        stored.get(&31).copied().unwrap_or(0) >= 8,
        "a powered well/refinery/splitter/mixer line must deliver asphalt: {}",
        serde_json::to_string(&core.snapshot().buildings).unwrap()
    );
    let refined = core.entities[burner]
        .fuel_inventory
        .get(&30)
        .copied()
        .unwrap_or(0);
    assert!(
        refined > 0,
        "the splitter must deliver the refinery's other output to the burner"
    );
    assert!(core.produced.values().sum::<u64>() > 0);
}

#[test]
fn petroleum_roads_require_research_and_base_and_refund_both_layers() {
    let (mut core, _) = test_core();
    core.set_creative(false);
    core.researched.remove(&22);
    core.player.inventory = BTreeMap::from([(17, 1), (31, 2)]);
    let mut edit = GroundEdit {
        q: 0,
        r: 2,
        to_q: 0,
        to_r: 2,
        corner: 0,
        to_corner: 0,
        definition_id: 6,
        shape: GroundShape::Cell,
        action: GroundAction::Pave,
        steps: 1,
        reference: GroundReference::First,
        cover: false,
    };
    assert!(core
        .ground_preview(&edit)
        .error
        .unwrap()
        .contains("Asphalt Roads"));
    core.researched.insert(22);
    assert!(core
        .ground_preview(&edit)
        .error
        .unwrap()
        .contains("Gravel yard"));
    edit.definition_id = 2;
    core.edit_ground(&edit).unwrap();
    edit.definition_id = 6;
    let preview = core.ground_preview(&edit);
    assert_eq!(
        preview.cost,
        vec![Ingredient {
            item_id: 31,
            quantity: 2
        }]
    );
    assert!(
        preview.refund.is_empty(),
        "the base is retained, not refunded into the pack"
    );
    core.edit_ground(&edit).unwrap();
    assert_eq!(core.movement_factor_at(0, 2), 150);
    assert_eq!(
        core.ground[&(0, 2)].paid,
        vec![
            Ingredient {
                item_id: 17,
                quantity: 1
            },
            Ingredient {
                item_id: 31,
                quantity: 2
            }
        ]
    );
    let digest = core.checksum();
    assert!(core
        .edit_ground(&edit)
        .unwrap_err()
        .contains("nothing spent"));
    assert_eq!(
        core.checksum(),
        digest,
        "re-paving the same surface cannot mint a refund"
    );
    edit.action = GroundAction::Clear;
    core.edit_ground(&edit).unwrap();
    assert_eq!(core.player.inventory, BTreeMap::from([(17, 1), (31, 2)]));
    core.undo_ground().unwrap();
    assert_eq!(core.surface_at(0, 2), 6);
    assert!(core.player.inventory.is_empty());
}

#[test]
fn petroleum_migration_verifies_original_checksum_and_does_not_reroll_legacy_sites() {
    let (mut core, scenarios) = test_core();
    core.world_params
        .site_rules
        .retain(|rule| rule.item_id != CRUDE_OIL);
    core.fields = WorldFields::new(&core.world_params, core.seed);
    core.place(2, 0, 3, 0, Some(1)).unwrap();
    let mut envelope: serde_json::Value = serde_json::from_str(
        core.save_string()
            .unwrap()
            .strip_prefix(SAVE_PREFIX)
            .unwrap(),
    )
    .unwrap();
    envelope["save_version"] = 31.into();
    envelope["definition_version"] = 25.into();
    envelope["technology_version"] = 13.into();
    envelope["world_generator_version"] = 9.into();
    envelope["checksum"] = core.checksum_for_world(9).into();
    let save = format!("{SAVE_PREFIX}{envelope}");
    let restored =
        Core::from_save(&core.definitions, &core.technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.world_params, core.world_params);
    assert_eq!(restored.checksum(), core.checksum());
    envelope["state"]["insight"] = 999.into();
    assert!(Core::from_save(
        &core.definitions,
        &core.technologies,
        &scenarios,
        &format!("{SAVE_PREFIX}{envelope}")
    )
    .err()
    .unwrap()
    .contains("checksum"));
}

#[test]
fn petroleum_survey_finds_accessible_oil_on_every_shipped_preset() {
    for preset in world_presets() {
        let report = survey::run(
            preset.key,
            &preset.params,
            survey::default_seed(),
            survey::DEFAULT_RADIUS,
        );
        let oil = report
            .patches
            .iter()
            .find(|patch| patch.item_id == CRUDE_OIL)
            .unwrap();
        assert!(
            oil.nearest_workable_patch.is_some(),
            "{} must expose a buildable oil site",
            preset.key
        );
        assert!(oil.largest_patch >= 7);
    }
}

/// Reproducible travel-time comparison, not a wall-clock or human playtest benchmark.
#[test]
fn petroleum_road_journeys_keep_gravel_useful_and_make_long_routes_faster() {
    for distance in [6, 24, 60] {
        let mut measured = Vec::new();
        for surface in [0, 2, 6] {
            let (mut core, _) = test_core();
            // An isolated, level corridor excludes generation and production from the comparison.
            if surface != 0 {
                for q in -1..=distance + 1 {
                    for r in -1..=1 {
                        core.ground.insert(
                            (q, r),
                            GroundCell {
                                q,
                                r,
                                surface,
                                elevation: 0,
                                paid: vec![],
                            },
                        );
                    }
                }
            }
            core.walk_to(distance, 0).unwrap();
            let mut steps = 0;
            while core.player.walk_goal.is_some() && steps < 10_000 {
                core.advance_player_steps(1);
                steps += 1;
            }
            assert!(core.player.walk_goal.is_none());
            assert_eq!(world_to_axial(core.player.x, core.player.y), (distance, 0));
            measured.push(steps);
        }
        assert!(measured[0] > measured[1] && measured[1] > measured[2]);
        println!("journey {distance} hexes: raw {} / gravel {} / asphalt {} player steps at {PLAYER_TICKS_PER_SECOND} Hz", measured[0], measured[1], measured[2]);
    }
}

#[test]
fn petroleum_loading_pre_masonry_world_does_not_require_new_limestone_guarantee() {
    let (catalog, _) = test_core();
    let scenarios: ScenariosInput =
        serde_json::from_str(include_str!("../../src/data/scenarios.json")).unwrap();
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|scenario| scenario.key == "new-game")
        .unwrap();
    let mut params = default_world_params();
    params
        .site_rules
        .retain(|rule| rule.item_id != LIMESTONE && rule.item_id != CRUDE_OIL);
    assert!(
        Core::new(
            &catalog.definitions,
            &catalog.technologies,
            scenario,
            None,
            Some(params.clone())
        )
        .is_err(),
        "new games still require a valid opening"
    );
    let core = Core::initialize(
        &catalog.definitions,
        &catalog.technologies,
        scenario,
        None,
        Some(params),
        false,
    )
    .unwrap();
    let mut envelope: serde_json::Value = serde_json::from_str(
        core.save_string()
            .unwrap()
            .strip_prefix(SAVE_PREFIX)
            .unwrap(),
    )
    .unwrap();
    envelope["save_version"] = 29.into();
    envelope["definition_version"] = 23.into();
    envelope["technology_version"] = 12.into();
    envelope["world_generator_version"] = 8.into();
    envelope["checksum"] = core.checksum_for_world(8).into();
    let restored = Core::from_save(
        &catalog.definitions,
        &catalog.technologies,
        &scenarios,
        &format!("{SAVE_PREFIX}{envelope}"),
    )
    .unwrap();
    assert_eq!(restored.world_params, core.world_params);
    assert_eq!(restored.checksum(), core.checksum());
    let reloaded = Core::from_save(
        &catalog.definitions,
        &catalog.technologies,
        &scenarios,
        &restored.save_string().unwrap(),
    )
    .unwrap();
    assert_eq!(reloaded.checksum(), restored.checksum());
}
