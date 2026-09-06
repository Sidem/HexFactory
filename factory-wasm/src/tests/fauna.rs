use super::*;

#[test]
fn herds_are_saved_mid_leg_without_resource_coupling() {
    let mut core = bare_game("new-game");
    assert!(!core.herds.is_empty());
    core.tick_many(137);
    let save = core.save_string().unwrap();
    let (definitions, technologies, scenarios) = catalogs();
    let mut restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(core.herds, restored.herds);
    core.tick_many(501);
    restored.tick_many(501);
    assert_eq!(core.herds, restored.herds);
    assert_eq!(core.checksum(), restored.checksum());
    assert!(core.resource_snapshots().iter().all(|r| r.item_id != 36));
}

#[test]
fn resting_herds_publish_nothing_between_arrivals() {
    let mut factory = test_factory("new-game");
    factory.build_delta();
    factory.core.dirty = SnapshotDirty::default();
    factory.core.advance_herds();
    let delta = factory.build_delta();
    assert!(delta.herds.is_none());
    assert!(delta.resources.is_none());
}

/// Grass is a stock, not a capacity rule.
///
/// Nothing in the model says how many animals a hex supports. Eating draws a sparse deficit down,
/// regrowth pays it back on its own cadence, and a stripped hex simply scores worse than its
/// neighbours in [`Core::decide_herd`] — which is the whole of what used to be `population_capacity`.
#[test]
fn grazing_draws_grass_down_and_regrowth_pays_it_back() {
    let mut core = bare_game("new-game");
    let (q, r) = hexes_in_radius((0, 0), 12)
        .into_iter()
        .find(|&(q, r)| core.grass_limit(q, r) > 0)
        .expect("the generated world has pasture near the landing site");
    let limit = core.grass_limit(q, r);
    assert_eq!(core.grass_stock(q, r), limit, "pristine pasture is full");
    assert!(
        !core.grazed.contains_key(&(q, r)),
        "a hex nothing has eaten off costs no map entry"
    );

    assert_eq!(core.graze_grass(q, r, limit), limit);
    assert_eq!(core.grass_stock(q, r), 0);
    assert_eq!(
        core.graze_grass(q, r, limit),
        0,
        "a stripped hex has nothing left to give"
    );

    // Regrowth only runs on its own cadence, and only over hexes something has actually eaten.
    core.tick = 100;
    core.regrow_grass();
    let after_one = core.grass_stock(q, r);
    assert!(after_one > 0 && after_one < limit, "partial recovery");
    for step in 2..=200u64 {
        core.tick = step * 100;
        core.regrow_grass();
    }
    assert_eq!(core.grass_stock(q, r), limit, "pasture recovers in full");
    assert!(
        !core.grazed.contains_key(&(q, r)),
        "a hex back at its limit leaves the sparse map again"
    );
}

/// A closed ring of fence is a pen, without anything in the fauna model knowing what a pen is.
///
/// Herds walk by committing one adjacent leg at a time, and every leg is checked by
/// [`Core::herd_leg_clear`] against the same boundary authority that stops the player. Six edges is
/// therefore the whole containment rule: there is no direction left that clears.
#[test]
fn a_closed_ring_of_fence_contains_every_leg() {
    let mut core = empty_world("new-game");
    core.scenario.generated_environment = false;
    core.compile_graph();
    core.set_creative(true);
    let centre = axial_world(0, 0);
    for direction in 0..6u8 {
        assert!(
            core.herd_leg_clear(
                centre,
                axial_world(
                    DIRECTIONS[direction as usize].0,
                    DIRECTIONS[direction as usize].1
                )
            ),
            "open ground should walk before the fence goes up"
        );
    }
    for direction in 0..6u8 {
        core.edit_boundaries(&edge_edit(0, 0, direction)).unwrap();
    }
    for &(dq, dr) in &DIRECTIONS {
        assert!(
            !core.herd_leg_clear(centre, axial_world(dq, dr)),
            "a fenced ring let a leg out through {dq},{dr}"
        );
    }
}

/// Flight is the one drive the player causes, so it is the one that has to answer to them.
#[test]
fn the_player_close_by_alarms_a_herd_and_sends_it_away() {
    let mut core = bare_game("new-game");
    let id = *core.herds.keys().next().expect("the world seeds herds");
    let start = core.herds[&id].position(core.tick);
    set_player_hex(&mut core, 0, 0);
    core.player.x = start.0;
    core.player.y = start.1 - 200;
    core.alarm_near_player();
    assert!(core.herds[&id].alarm > 0, "the player went unnoticed");

    let mut herd = core.herds.remove(&id).expect("herd exists");
    let target = core.decide_herd(&mut herd);
    assert_eq!(herd.drive, Drive::Flee, "alarm outranks every other need");
    let before = squared_distance(start.0, start.1, core.player.x, core.player.y);
    let after = squared_distance(target.0, target.1, core.player.x, core.player.y);
    assert!(
        after > before,
        "a fleeing herd committed a leg toward the player: {before} -> {after}"
    );
}

/// A thirsty herd walks to water, and "water" means the same thing to it as to the ecology.
///
/// [`ecology::beside_fresh_water`] is the independent oracle here: the herd steers by
/// [`Core::drinking_bank`], which asks the narrower question — fresh, standing, and not behind a
/// closed fence — so anywhere a herd will drink must also read as watered ground.
#[test]
fn a_thirsty_herd_commits_its_leg_to_a_drinking_bank() {
    let mut core = bare_game("new-game");
    let (bank, stand) = hexes_in_radius((0, 0), 40)
        .into_iter()
        .find_map(|(q, r)| {
            if !core.drinking_bank(q, r) || !core.walkable_hex(q, r) {
                return None;
            }
            let stand = DIRECTIONS
                .iter()
                .map(|&(dq, dr)| (q + dq, r + dr))
                .find(|&(sq, sr)| {
                    !core.drinking_bank(sq, sr)
                        && core.herd_leg_clear(axial_world(sq, sr), axial_world(q, r))
                })?;
            Some(((q, r), stand))
        })
        .expect("the generated world has reachable fresh water near the landing site");
    assert!(
        ecology::beside_fresh_water(&core, bank.0, bank.1),
        "a hex a herd will drink at is not watered ground"
    );

    let mut herd = Herd {
        id: u32::MAX,
        species: core.definitions.species[0].id,
        from: axial_world(stand.0, stand.1),
        to: axial_world(stand.0, stand.1),
        left_tick: core.tick,
        arrive_tick: core.tick,
        count: 2,
        drive: Drive::Rest,
        hunger: 0,
        thirst: 900,
        alarm: 0,
        healthy_ticks: 0,
        shortage_ticks: 0,
        hunt_tick: 0,
    };
    let target = core.decide_herd(&mut herd);
    assert_eq!(herd.drive, Drive::Thirst);
    assert_eq!(
        world_to_axial(target.0, target.1),
        bank,
        "thirst did not commit the adjacent leg to the water"
    );
}

/// A need the herd cannot meet where it stands has to move it, or the world quietly empties.
///
/// Standing still is what a bounded search returns when nothing inside it scores, and a thirsty
/// herd that stands still dies on a timer the player never touched. So the herd migrates instead:
/// it commits a leg down the drainage, which is the one direction that cannot dead-end, because a
/// watershed has no local minimum to strand it in.
///
/// The hex is chosen with no bank anywhere in its search radius as the crow flies, which is
/// strictly stronger than none it can walk to — whatever route exists, the search cannot see one.
#[test]
fn thirst_with_no_water_in_reach_migrates_instead_of_waiting() {
    let mut core = bare_game("new-game");
    let (q, r) = hexes_in_radius((0, 0), 14)
        .into_iter()
        .find(|&(q, r)| {
            core.walkable_hex(q, r)
                && core.water_depth_at(q, r) == 0
                && !core.runtime.occupied.contains_key(&(q, r))
                && hydrology::WaterField::surveyed(&core, q, r)
                && !hexes_in_radius((q, r), 10)
                    .iter()
                    .any(|&(bq, br)| core.drinking_bank(bq, br))
        })
        .expect("the opening world has dry ground with no bank inside a herd's reach");
    let here = axial_world(q, r);
    let mut herd = Herd {
        id: u32::MAX,
        species: core.definitions.species[0].id,
        from: here,
        to: here,
        left_tick: core.tick,
        arrive_tick: core.tick,
        count: 3,
        drive: Drive::Rest,
        hunger: 0,
        thirst: 900,
        alarm: 0,
        healthy_ticks: 0,
        shortage_ticks: 0,
        hunt_tick: 0,
    };
    let target = core.decide_herd(&mut herd);
    assert_eq!(herd.drive, Drive::Thirst);
    assert_ne!(target, here, "a thirsty herd stood still and waited to die");
    assert!(
        core.herd_leg_clear(here, target),
        "migration committed a leg the herd cannot walk"
    );
}

/// The sea is not a drink, and the guide that sends migrating herds downstream has to know it.
///
/// Every drainage ends in the ocean. A guide that stopped at the first water it found therefore
/// named salt for any herd on the coastal side of a divide, and the animals walked to the beach,
/// stood at the edge of water they will not touch, and ran their shortage timers out in sight of it.
/// So the walk answers only for fresh water and gives up where the drainage turns salt — checked
/// here as an invariant over the opening world rather than for one hex, because which hexes drain
/// to the sea is the generator's business and not this test's.
#[test]
fn the_migration_guide_never_names_salt_water() {
    let core = bare_game("new-game");
    let salt = |q: i32, r: i32| {
        core.water_depth_at(q, r) > 0 && core.water_surface_at(q, r) <= scale::SEA_LEVEL_QUANTA
    };
    let mut guided = 0;
    for (q, r) in hexes_in_radius((0, 0), 30) {
        let Some(goal) = core.downstream_water((q, r)) else {
            continue;
        };
        guided += 1;
        assert!(
            !salt(goal.0, goal.1),
            "the drainage from {q},{r} sent a thirsty herd to the sea at {goal:?}"
        );
        assert!(
            core.water_depth_at(goal.0, goal.1) > 0
                || core.drinking_bank(goal.0, goal.1)
                || core
                    .ground_spine
                    .river_bench_class_at(goal.0, goal.1)
                    .is_some(),
            "the guide named {goal:?}, which is neither water, a bank, nor a river bench"
        );
    }
    assert!(guided > 0, "no hex near the landing site drains anywhere");
}

/// The seeded world has to outlive its own opening.
///
/// [`Core::seed_herds`] places nothing further from a drink than the animal can walk — walk, not
/// measure, because the rivers here are cut into their benches and water three hexes off across
/// the drop is water the herd will die in sight of. So the opening population is viable before the
/// player has touched anything. Wildlife may still collapse afterwards — that is the point of grass
/// being a stock and of a fence stopping a leg — but only because somebody caused it.
#[test]
fn seeded_wildlife_outlives_the_opening() {
    let mut core = bare_game("new-game");
    let before: u32 = core.herds.values().map(|h| u32::from(h.count)).sum();
    assert!(before > 0, "the generated world seeds no wildlife at all");
    core.tick_many(9000);
    let after: u32 = core.herds.values().map(|h| u32::from(h.count)).sum();
    assert!(
        after >= before,
        "the opening world's wildlife declined on its own: {before} animals -> {after}"
    );
}

/// Wildlife is not a deposit, and the extractor graph must never be able to see it.
#[test]
fn wildlife_never_enters_the_resource_or_extractor_graph() {
    let mut core = bare_game("new-game");
    core.tick_many(240);
    let carcass = core.definitions.species[0].carcass_item;
    for herd in core.herds.values() {
        let (q, r) = world_to_axial(herd.position(core.tick).0, herd.position(core.tick).1);
        assert!(
            core.field_at(q, r)
                .is_none_or(|field| field.item_id != carcass),
            "a herd's hex advertised itself as a field of {carcass}"
        );
        // A herd may happen to be standing on ore. What must not happen is a candidate that only
        // exists because the animals are there, so every hex an extractor is offered has to be
        // justified by a field that is still a field once the herd walks off it.
        for candidate in core.deposit_candidates(q, r, 2) {
            let field = core
                .field_at(candidate.0, candidate.1)
                .expect("an extractor was offered a hex with no field on it");
            assert_ne!(field.item_id, carcass, "wildlife was offered as a deposit");
        }
    }
    assert!(
        core.resource_snapshots()
            .iter()
            .all(|row| row.item_id != carcass),
        "a carcass reached the resources group"
    );
}
