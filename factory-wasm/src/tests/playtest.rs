use super::*;

#[test]
fn pole_preview_and_build_stop_at_the_same_material_budget() {
    let mut core = ground_world();
    core.researched.extend([1, 2, 8]);
    stock_for(&mut core, 12, 2);
    let preview = core.line_preview((0, 0), (18, 0), 12, 0, None);
    assert_eq!(preview.iter().filter(|cell| cell.legal).count(), 2);
    assert!(preview[2].reason.as_ref().unwrap().contains("materials"));
    core.place_line((0, 0), (18, 0), 12, 0, None).unwrap();
    assert_eq!(core.entities.len(), 2);
    assert_eq!(core.entities[1].placed.q, 6);
}

#[test]
fn switching_away_from_steel_preserves_recoverable_inputs_and_separate_fuel() {
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    core.place(2, 0, 7, 0, Some(5)).unwrap();
    let index = core.entity_at(2, 0).unwrap();
    core.entities[index].input_inventory.insert(COAL, 2);
    core.entities[index].fuel_inventory.insert(COAL, 3);
    core.set_recipe(2, 0, 2).unwrap();
    let snapshot = core.entity_snapshot(index);
    assert_eq!(
        snapshot.input_inventory,
        vec![Ingredient {
            item_id: COAL,
            quantity: 2
        }]
    );
    assert_eq!(
        snapshot.fuel_inventory,
        vec![Ingredient {
            item_id: COAL,
            quantity: 3
        }]
    );
}

#[test]
fn bridge_drags_skip_banks_and_explain_deep_water_and_belt_support() {
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    for q in 1..=3 {
        core.water.set(q, 0, hydrology::WaterDelta::new(1));
    }
    let preview = core.line_preview((0, 0), (4, 0), 23, 0, None);
    assert_eq!(
        preview
            .iter()
            .map(|cell| (cell.q, cell.r))
            .collect::<Vec<_>>(),
        vec![(1, 0), (2, 0), (3, 0)]
    );
    assert!(preview.iter().all(|cell| cell.legal));
    assert!(core
        .placement_legality(1, 0, 2, 0, None, false)
        .unwrap_err()
        .contains("bridge"));
    core.place_line((0, 0), (4, 0), 23, 0, None).unwrap();
    assert_eq!(core.entities.len(), 3);
    assert!(!core
        .events
        .iter()
        .any(|event| event.starts_with("Run stopped")));
    assert!(core.place(2, 0, 2, 0, None).is_ok());
    assert!(core
        .place(0, 0, 23, 0, None)
        .unwrap_err()
        .contains("need water"));
    core.water.set(
        6,
        0,
        hydrology::WaterDelta::new(scale::WADE_LIMIT_QUANTA as i16),
    );
    assert!(core
        .place(6, 0, 23, 0, None)
        .unwrap_err()
        .contains("too deep"));
}

#[test]
fn pole_drags_space_each_tier_and_match_preview_and_power_links() {
    let core = ground_world();
    let poles: Vec<_> = core
        .definitions
        .buildings
        .iter()
        .filter(|definition| definition.kind == BuildingKind::Pole && definition.buildable)
        .map(|definition| (definition.id, definition.pole_reach.unwrap()))
        .collect();
    for (id, reach_in_hexes) in poles {
        let mut core = ground_world();
        core.set_creative(true);
        reach(&mut core);
        let reach_in_hexes = reach_in_hexes as i32;
        let end = 2 * reach_in_hexes + 2;
        let preview = core.line_preview((0, 0), (end, 0), id, 0, None);
        let promised: Vec<_> = preview.iter().map(|cell| (cell.q, cell.r)).collect();
        assert_eq!(
            promised,
            vec![
                (0, 0),
                (reach_in_hexes, 0),
                (reach_in_hexes * 2, 0),
                (end, 0)
            ]
        );
        assert!(preview.iter().all(|cell| cell.legal));
        core.place_line((0, 0), (end, 0), id, 0, None).unwrap();
        assert_eq!(core.entities.len(), 4);
        for pair in promised.windows(2) {
            assert!(core.power_linked(
                core.entity_at(pair[0].0, pair[0].1).unwrap(),
                core.entity_at(pair[1].0, pair[1].1).unwrap()
            ));
        }
    }
}

#[test]
fn pole_drag_shortens_spans_around_obstacles_and_reuses_anchors() {
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    core.place(0, 0, 12, 0, None).unwrap();
    core.place(6, 0, 4, 0, None).unwrap();
    core.place(13, 0, 4, 0, None).unwrap();
    let preview = core.line_preview((0, 0), (13, 0), 12, 0, None);
    let promised: Vec<_> = preview.iter().map(|cell| (cell.q, cell.r)).collect();
    assert_eq!(promised, vec![(0, 0), (5, 0), (11, 0), (12, 0)]);
    assert!(preview.iter().all(|cell| cell.legal));
    core.place_line((0, 0), (13, 0), 12, 0, None).unwrap();
    assert_eq!(
        core.entities
            .iter()
            .filter(|entity| entity.kind == BuildingKind::Pole)
            .count(),
        4
    );
    assert!(!core
        .events
        .iter()
        .any(|event| event.starts_with("Run stopped")));
}

#[test]
fn equal_pressure_flora_spreads_in_every_direction_and_replays() {
    let mut core = ground_world();
    let mut counts = [0; 6];
    // Isolated recovering stands across an open meadow all have six equally sheltered choices.
    // A compass-index tie break sends every one east; spatial preferences must cover all six.
    for q in -4..=4 {
        for r in -4..=4 {
            let cell = (q * 4, r * 4);
            core.ensure_neighborhood(axial_world(cell.0, cell.1).0, axial_world(cell.0, cell.1).1);
            core.write_overlay(cell.0, cell.1, WOOD, 2, 2);
            let target = core.colonisation_target(cell.0, cell.1, WOOD).unwrap();
            let direction = DIRECTIONS
                .iter()
                .position(|&(dq, dr)| target == (cell.0 + dq, cell.1 + dr))
                .unwrap();
            counts[direction] += 1;
        }
    }
    assert!(
        counts.iter().all(|&count| count >= 5),
        "spatial ties must spread across the meadow: {counts:?}"
    );
    // Save replay uses the real scenario ground; ground_world deliberately substitutes a
    // flat generator only for the directional distribution fixture above.
    let mut core = field_game("new-game");
    core.write_overlay(-3, 1, WOOD, 1, 2);
    let (definitions, technologies, scenarios) = catalogs();
    let restored = Core::from_save(
        &definitions,
        &technologies,
        &scenarios,
        &core.save_string().unwrap(),
    )
    .unwrap();
    for q in -4..=4 {
        for r in -4..=4 {
            assert_eq!(
                core.colonisation_target(q * 4, r * 4, WOOD),
                restored.colonisation_target(q * 4, r * 4, WOOD)
            );
        }
    }
}
