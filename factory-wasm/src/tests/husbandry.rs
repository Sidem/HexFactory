use super::*;

fn station(recipe_key: &str) -> (Core, usize) {
    let mut core = game("new-game");
    core.herds.clear();
    core.rebuild_herd_schedule();
    core.researched.insert(7);
    let recipe = core
        .definitions
        .recipes
        .iter()
        .find(|r| r.key == recipe_key)
        .unwrap()
        .clone();
    let building = core
        .definitions
        .buildings
        .iter()
        .find(|b| b.supports_recipe(&recipe))
        .unwrap()
        .id;
    set_player_hex(&mut core, 4, 1);
    stock_for(&mut core, building, 1);
    core.place(4, 0, building, 0, Some(recipe.id)).unwrap();
    let index = core.entity_at(4, 0).unwrap();
    for input in recipe.inputs {
        core.entities[index]
            .input_inventory
            .insert(input.item_id, 10);
    }
    (core, index)
}

fn grazer(core: &mut Core, cell: (i32, i32), count: u16) -> u32 {
    let id = core.next_herd_id;
    core.next_herd_id += 1;
    let point = axial_world(cell.0, cell.1);
    core.herds.insert(
        id,
        Herd {
            id,
            species: core.definitions.species[0].id,
            from: point,
            to: point,
            left_tick: core.tick,
            arrive_tick: core.tick + 100,
            count,
            drive: Drive::Rest,
            hunger: 0,
            thirst: 0,
            alarm: 0,
            healthy_ticks: 0,
            shortage_ticks: 0,
            hunt_tick: 0,
        },
    );
    core.rebuild_herd_schedule();
    id
}

#[test]
fn station_requires_physical_surplus_and_preserves_the_breeding_pair() {
    let (mut core, index) = station("managed-harvest");
    let work = core.pasture_work_cell(index);
    let id = grazer(&mut core, work, 3);
    assert!(core.pasture_cell_open(index));
    core.advance_ticks(25);
    assert_eq!(core.herds[&id].count, 2);
    let item = core.definitions.species[0].carcass_item;
    assert_eq!(core.entities[index].output_inventory.get(&item), Some(&1));
    core.advance_ticks(50);
    assert_eq!(core.herds[&id].count, 2);
    assert_eq!(core.entities[index].progress, 0);
    assert!(core
        .pasture_station_note(index)
        .unwrap()
        .contains("Reserve protected"));
}

#[test]
fn harvest_waits_for_a_departed_animal_and_never_loses_reserved_inputs() {
    let (mut core, index) = station("managed-harvest");
    let work = core.pasture_work_cell(index);
    let id = grazer(&mut core, work, 4);
    core.advance_ticks(2);
    let progress = core.entities[index].progress;
    let reserved = core.entities[index].reserved_inputs.clone();
    assert!(progress > 0);
    let away = axial_world(work.0 + 2, work.1);
    core.herds.get_mut(&id).unwrap().from = away;
    core.herds.get_mut(&id).unwrap().to = away;
    core.advance_ticks(25);
    assert_eq!(core.entities[index].progress, progress);
    assert_eq!(core.entities[index].reserved_inputs, reserved);
    assert_eq!(core.herds[&id].count, 4);
    assert!(core.entities[index].output_inventory.is_empty());
}

#[test]
fn mowing_consumes_real_grass_and_restoration_stops_on_healthy_ground() {
    let (mut core, index) = station("mow-pasture");
    let cell = core.pasture_work_cell(index);
    let before = core.grass_stock(cell.0, cell.1);
    core.advance_ticks(51);
    assert_eq!(core.grass_stock(cell.0, cell.1), before - 100);
    let feed = core.definitions.species[0].feed_item;
    assert_eq!(core.entities[index].output_inventory.get(&feed), Some(&1));
    let (mut core, index) = station("restore-pasture");
    core.advance_ticks(80);
    assert_eq!(core.entities[index].progress, 0);
    let cell = core.pasture_work_cell(index);
    core.graze_grass(cell.0, cell.1, 200);
    core.advance_ticks(81);
    assert!(core.grass_stock(cell.0, cell.1) > core.grass_limit(cell.0, cell.1) - 200);
    assert_eq!(core.entities[index].output_inventory.get(&feed), Some(&1));
}

#[test]
fn feed_is_packed_placed_and_lures_without_automatic_pickup() {
    let mut core = game("new-game");
    set_player_hex(&mut core, 4, 0);
    let before = core.grass_stock(4, 0);
    core.cut_feed(4, 0).unwrap();
    assert_eq!(core.grass_stock(4, 0), before - 100);
    let item = core.definitions.species[0].feed_item;
    assert_eq!(core.player.inventory.get(&item), Some(&1));
    core.place_feed(4, 0).unwrap();
    core.player.move_x = 1000;
    core.tick += 40;
    core.collect_ground_items();
    assert_eq!(
        core.ground_items
            .iter()
            .find(|i| i.item_id == item)
            .unwrap()
            .quantity,
        1
    );
    let id = grazer(&mut core, (3, 0), 3);
    let mut herd = core.herds.remove(&id).unwrap();
    let target = core.decide_herd(&mut herd);
    assert_eq!(target, axial_world(4, 0));
}

#[test]
fn station_rotations_keep_the_work_cell_inside_the_reserved_apron() {
    let (mut core, index) = station("managed-harvest");
    for orientation in 0..6 {
        core.entities[index].placed.orientation = orientation;
        let work = core.pasture_work_cell(index);
        let envelope = core.envelope_for(core.entities[index].placed, orientation);
        assert!(envelope.iter().any(|c| (c.q, c.r) == work));
    }
}

#[test]
fn lone_animals_reunite_without_inventing_population() {
    let mut core = game("new-game");
    core.herds.clear();
    let a = grazer(&mut core, (4, 0), 1);
    let b = grazer(&mut core, (4, 0), 1);
    core.advance_ticks(100);
    assert_eq!(core.herds.len(), 1);
    assert_eq!(core.herds.values().next().unwrap().count, 2);
    assert!(core.dirty.herds.contains(&a) && core.dirty.herds.contains(&b));
}

#[test]
fn a_closed_fence_blocks_feed_attraction_and_station_work() {
    let (mut core, index) = station("managed-harvest");
    core.set_creative(true);
    let cell = core.pasture_work_cell(index);
    let id = grazer(&mut core, cell, 3);
    core.edit_boundaries(&edge_edit(cell.0, cell.1, 0)).unwrap();
    assert!(!core.pasture_cell_open(index));
    assert!(core
        .pasture_feeder(cell, core.definitions.species[0].feed_item)
        .is_none());
    core.advance_ticks(25);
    assert_eq!(core.herds[&id].count, 3);
    assert_eq!(core.entities[index].progress, 0);
}

#[test]
fn inspecting_the_apron_names_the_station_and_its_working_cell() {
    let (core, index) = station("managed-harvest");
    let work = core.pasture_work_cell(index);
    let preview = core.pasture_preview(work.0, work.1);
    assert!(preview.station_note.unwrap().contains("Working cell"));
    assert_eq!(preview.work_point, Some(axial_world(work.0, work.1)));
}

#[test]
fn a_stocked_station_draws_a_reachable_herd_to_its_apron() {
    let (mut core, index) = station("managed-harvest");
    let cell = core.pasture_work_cell(index);
    let id = grazer(&mut core, (cell.0 - 1, cell.1), 4);
    let mut herd = core.herds.remove(&id).unwrap();
    assert_eq!(core.decide_herd(&mut herd), axial_world(cell.0, cell.1));
}
