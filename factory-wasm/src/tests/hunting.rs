use super::*;

fn encounter() -> (Core, u32) {
    let mut core = bare_game("new-game");
    let id = *core.herds.keys().next().unwrap();
    let herd = core.herds.get_mut(&id).unwrap();
    herd.count = 4;
    herd.arrive_tick = core.tick + 100;
    core.player.x = herd.from.0 - 3000;
    core.player.y = herd.from.1;
    core.rebuild_herd_schedule();
    (core, id)
}

#[test]
fn approach_aim_and_release_produces_one_carcass_then_scatters() {
    let (mut core, id) = encounter();
    core.alarm_near_player();
    assert_eq!(
        core.herds[&id].alarm, 0,
        "approaching hunting reach is not a chase"
    );
    assert!(core.hunt_preview(id).unwrap().ready);
    if let Some(path) = std::env::var_os("HEXFACTORY_HUNT_SMOKE_SAVE") {
        std::fs::write(path, core.save_string().unwrap()).unwrap();
    }
    core.player.walk_goal = Some(Coordinate { q: 0, r: 0 });
    core.player.move_x = 1000;
    core.hunt_herd(id).unwrap();
    assert!(core.player.walk_goal.is_none());
    assert_eq!(core.player.move_x, 0);
    assert_eq!(core.herds[&id].alarm, 0);
    core.advance_ticks(9);
    assert_eq!(core.herds[&id].count, 4);
    core.advance_ticks(1);
    assert_eq!(core.herds[&id].count, 3);
    assert!(core.herds[&id].alarm > 30);
    let item = core.definitions.species[0].carcass_item;
    assert_eq!(
        core.ground_items
            .iter()
            .filter(|i| i.item_id == item)
            .map(|i| i.quantity)
            .sum::<u32>(),
        1
    );
    assert!(
        !core.hunt_preview(id).unwrap().ready,
        "cannot repeatedly farm a startled herd"
    );
}

#[test]
fn moving_target_resolves_mid_leg_at_the_advertised_tick_and_replays() {
    let (mut core, id) = encounter();
    core.herds.get_mut(&id).unwrap().to.0 += 1000;
    core.hunt_herd(id).unwrap();
    core.advance_ticks(4);
    let save = core.save_string().unwrap();
    let (definitions, technologies, scenarios) = catalogs();
    let mut restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    let expected = core.herds[&id].position(10);
    core.advance_ticks(6);
    restored.advance_ticks(6);
    assert_eq!(core.herds[&id].count, 3);
    assert_eq!(core.herds[&id].from, expected);
    assert_eq!(core.checksum(), restored.checksum());
}

#[test]
fn movement_cancels_aim_without_killing_or_alarming() {
    let (mut core, id) = encounter();
    core.hunt_herd(id).unwrap();
    core.set_move_intent(1000, 0).unwrap();
    core.advance_player();
    core.advance_ticks(10);
    assert_eq!(core.herds[&id].count, 4);
    assert_eq!(core.herds[&id].hunt_tick, 0);
    assert!(core.events.iter().any(|e| e == "Hunt cancelled"));
}

#[test]
fn preview_and_command_agree_on_range_and_only_one_aim() {
    let (mut core, id) = encounter();
    core.player.x -= 3000;
    let preview = core.hunt_preview(id).unwrap();
    assert!(!preview.ready);
    assert_eq!(core.hunt_herd(id).unwrap_err(), preview.reason);
    core.player.x += 3000;
    core.hunt_herd(id).unwrap();
    assert!(core.hunt_herd(id).is_err());
    core.cancel_hunt();
    assert!(core.hunt_preview(id).unwrap().ready);
}

#[test]
fn version_46_migrates_without_losing_legacy_transport_or_hunting_state() {
    let mut core = bare_game("factory-demo");
    let belt = core
        .entities
        .iter()
        .find(|e| e.kind == BuildingKind::Belt)
        .unwrap()
        .id;
    core.legacy_fluid_belts.insert(belt);
    let save = core.save_string().unwrap();
    let previous = save
        .replacen(
            &format!("\"save_version\":{SAVE_VERSION}"),
            "\"save_version\":46",
            1,
        )
        .replacen(
            &format!("\"definition_version\":{}", core.definitions.version),
            "\"definition_version\":31",
            1,
        );
    let (definitions, technologies, scenarios) = catalogs();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &previous).unwrap();
    assert_eq!(restored.checksum(), core.checksum());
    assert_eq!(restored.legacy_fluid_belts, core.legacy_fluid_belts);
}

#[test]
fn driving_stops_the_player_and_scatters_without_a_carcass() {
    let (mut core, id) = encounter();
    core.player.walk_goal = Some(Coordinate { q: 0, r: 0 });
    core.player.move_x = 1000;
    core.drive_herd(id).unwrap();
    assert!(core.player.walk_goal.is_none());
    assert_eq!(core.player.move_x, 0);
    core.advance_ticks(1);
    assert_eq!(core.herds[&id].count, 4);
    assert!(core.herds[&id].alarm > 0);
    assert!(core
        .ground_items
        .iter()
        .all(|i| i.item_id != core.definitions.species[0].carcass_item));
}

#[test]
fn closing_a_fence_during_aim_blocks_the_kill_at_release() {
    let mut core = empty_world("new-game");
    core.set_creative(true);
    let id = core.next_herd_id;
    core.next_herd_id += 1;
    let point = axial_world(1, 0);
    core.herds.insert(
        id,
        Herd {
            id,
            species: core.definitions.species[0].id,
            from: point,
            to: point,
            left_tick: 0,
            arrive_tick: 100,
            count: 3,
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
    set_player_hex(&mut core, 0, 0);
    core.hunt_herd(id).unwrap();
    core.edit_boundaries(&edge_edit(0, 0, 0)).unwrap();
    core.advance_ticks(10);
    assert_eq!(core.herds[&id].count, 3);
    assert_eq!(core.herds[&id].hunt_tick, 0);
}
