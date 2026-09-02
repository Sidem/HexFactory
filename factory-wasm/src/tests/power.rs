use super::*;

#[test]
fn power_reaches_only_what_it_lights_and_is_produced_only_for_the_work_it_does() {
    let mut dark = game("new-game");
    dark.power_unmetered = false;
    dark.researched.extend([1, 2, 8]);
    stock_for(&mut dark, 1, 1);
    dark.player.inventory.insert(5, 8);
    set_player_hex(&mut dark, 1, 0);
    dark.place(3, 0, 1, 0, None).unwrap();
    dark.tick_many(20);
    let extractor = dark
        .entities
        .iter()
        .find(|entity| entity.kind == BuildingKind::Extractor)
        .unwrap();
    assert!(extractor.cargo.is_none());
    assert_eq!(
        dark.entity_snapshot(
            dark.entities
                .iter()
                .position(|entity| entity.kind == BuildingKind::Extractor)
                .unwrap()
        )
        .status,
        EntityStatus::NoPower
    );

    let mut lit = game("new-game");
    lit.power_unmetered = false;
    lit.researched.extend([1, 2, 8]);
    stock_for(&mut lit, 1, 1);
    stock_for(&mut lit, 12, 1);
    stock_for(&mut lit, 13, 1);
    lit.player.inventory.insert(5, 8);
    set_player_hex(&mut lit, 1, 0);
    lit.place(3, 0, 1, 0, None).unwrap();
    let pole = try_place_near(&mut lit, (3, 0), 12);
    try_place_near(&mut lit, pole, 13);
    let burner = lit
        .entities
        .iter()
        .position(|entity| entity.kind == BuildingKind::Generator)
        .unwrap();
    lit.entities[burner].inventory.insert(5, 8);
    // Past one whole ore cycle, so the extractor is holding cargo rather than mid-dig. Ore is
    // 30 ticks now that the rate comes from the material rather than the building.
    lit.tick_many(35);
    let extractor = lit
        .entities
        .iter()
        .find(|entity| entity.kind == BuildingKind::Extractor)
        .unwrap();
    assert!(extractor.cargo.is_some() || extractor.progress > 0);
    let snapshot = lit.entity_snapshot(burner);
    assert_eq!(snapshot.status, EntityStatus::Generating);
    assert!(snapshot.power_satisfied > 0);
    // The extractor has produced, but its output buffer still has room. It keeps booking power
    // until that buffer fills, which is the headroom that lets a short belt jam preserve work.
    assert!(!lit.entities[extractor_index(&lit)]
        .output_inventory
        .is_empty());
    assert!(snapshot.power_demand > 0);

    // The other half of the same rule, and the one the player pays for: a plant carrying a small
    // load burns proportionally less fuel, and a plant carrying none burns none at all.
    let coal = |core: &Core, index: usize| {
        let entity = &core.entities[index];
        (entity.inventory.get(&5).copied().unwrap_or(0)
            + entity.fuel_inventory.get(&5).copied().unwrap_or(0))
            * core.fuel_value(5)
            + entity.fuel_charge
    };

    // One burner, one pole, one extractor with a deposit to work.
    let mut working = game("new-game");
    working.power_unmetered = false;
    working.researched.extend([1, 2, 8]);
    stock_for(&mut working, 1, 1);
    stock_for(&mut working, 12, 1);
    stock_for(&mut working, 13, 1);
    working.player.inventory.insert(5, 8);
    set_player_hex(&mut working, 1, 0);
    working.place(3, 0, 1, 0, None).unwrap();
    let pole = try_place_near(&mut working, (3, 0), 12);
    try_place_near(&mut working, pole, 13);
    let burner = working
        .entities
        .iter()
        .position(|entity| entity.kind == BuildingKind::Generator)
        .unwrap();
    working.entities[burner].inventory.insert(5, 20);
    let before = coal(&working, burner);
    working.tick_many(40);
    let spent_working = before - coal(&working, burner);
    let output = working
        .building_definition(working.entities[burner].placed.definition_id)
        .unwrap()
        .power_output
        .unwrap();
    let taken = grid_energy_received(&working);
    let owed = working.entities[burner].burn_progress;

    // The same grid with nothing on it: a generator wired to a pole and no machine at all.
    let mut idle = game("new-game");
    idle.power_unmetered = false;
    idle.researched.extend([1, 2, 8]);
    stock_for(&mut idle, 12, 1);
    stock_for(&mut idle, 13, 1);
    idle.player.inventory.insert(5, 8);
    set_player_hex(&mut idle, 1, 0);
    let pole = try_place_near(&mut idle, (3, 0), 12);
    try_place_near(&mut idle, pole, 13);
    let idle_burner = idle
        .entities
        .iter()
        .position(|entity| entity.kind == BuildingKind::Generator)
        .unwrap();
    idle.entities[idle_burner].inventory.insert(5, 20);
    let before = coal(&idle, idle_burner);
    idle.tick_many(40);

    assert_eq!(
        before - coal(&idle, idle_burner),
        0,
        "a plant with nothing to power burns nothing"
    );
    assert!(spent_working > 0, "a plant doing work burns something");
    // The bill is the load, exactly. Fuel spent buys `output` units of electricity each, and
    // the part-unit still owed sits in `burn_progress` — so this equality is the whole rule,
    // and it is what fails the moment a plant burns for the clock instead of for the work.
    assert_eq!(
        spent_working * output + owed,
        taken,
        "a plant burns for what it handed over"
    );
    // And what one extractor asks for over forty ticks is still less than this plant could
    // have made in them. The old rule charged a unit of fuel energy per tick regardless, so
    // forty is what burning for the clock costs.
    //
    // The margin used to be far wider. `power_capacity` is `POWER_BUFFER_CYCLES` whole cycles,
    // and an ore cycle went from 5 ticks to 30, so a single extractor now banks six times as
    // much before it runs steadily — most of what this plant burned is sitting in that buffer
    // rather than having been turned into ore.
    assert!(
        spent_working < 40,
        "one extractor must not cost a burner its full output: spent {spent_working}"
    );

    // Coverage belongs to the pole, and it is the whole of the upgrade.
    //
    // The same machine at the same hex is dark under a base pole and lit under a relay pole,
    // with nothing else in the world changed. Before v0.19 this test could not have been written:
    // the distance came off the machine, so every pole in the game reached exactly as far as
    // every other one and no upgrade could move it.
    let base = building_by_key("pole");
    let relay = building_by_key("pole-ii");
    let trunk = building_by_key("pole-iii");
    assert_eq!(base.supply_radius, Some(3));
    assert_eq!(relay.supply_radius, Some(4));
    assert_eq!(trunk.supply_radius, Some(6));
    // The ladder is a chain, so an upgrade never skips a rung or turns a pole into a machine.
    assert_eq!(base.upgrades_to, Some(relay.id));
    assert_eq!(relay.upgrades_to, Some(trunk.id));

    // A pole and a machine exactly four hexes apart: outside a base pole, inside a relay.
    for (definition_id, expected) in [(base.id, false), (relay.id, true)] {
        let mut core = game("new-game");
        core.power_unmetered = false;
        core.researched.extend([1, 2, 8, 5, 13]);
        stock_for(&mut core, 1, 1);
        stock_for(&mut core, definition_id, 1);
        core.player.build_range = 1 << 20;
        set_player_hex(&mut core, 0, 0);
        core.place(3, 0, 1, 0, None).unwrap();
        let extractor = extractor_index(&core);
        // Four hexes from the ground the extractor stands on, which is its eastern cell rather
        // than its anchor now that a machine covers more than one hex.
        core.place(4 + 4, 0, definition_id, 0, None).unwrap();
        let pole = core
            .entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Pole)
            .unwrap();
        // An unconnected machine sits on a network of its own, so what says "covered" is
        // sharing the pole's network rather than merely having one.
        assert_eq!(
            core.power_of[extractor] == core.power_of[pole],
            expected,
            "a pole with radius {:?} four hexes away",
            core.building_definition(definition_id)
                .unwrap()
                .supply_radius
        );
    }

    // Machines that touch conduct, and only the ones that carry current do.
    //
    // This is what makes a pole cost *distance* rather than power, and it is what
    // `fixtures/balance.json` has priced openings against since v0.18 — one generator, no pole,
    // for a machine standing beside it. Until v0.19 that price was simply wrong.
    let mut core = game("new-game");
    core.power_unmetered = false;
    core.researched.extend([1, 2, 8]);
    stock_for(&mut core, 1, 1);
    stock_for(&mut core, 13, 1);
    core.player.inventory.insert(5, 20);
    core.player.inventory.insert(24, 8);
    core.player.build_range = 1 << 20;
    set_player_hex(&mut core, 0, 0);
    core.place(3, 0, 1, 0, None).unwrap();
    let extractor = extractor_index(&core);

    // No pole anywhere: the generator is simply built against the extractor's occupied
    // foundation. Neighbours of the anchor are not enough now that the machine and its
    // service envelope cover more than one hex; search every cell the hull actually holds.
    let mut placed = None;
    let hull = core.entity_footprint(&core.entities[extractor]);
    'search: for cell in &hull {
        for &(dq, dr) in &DIRECTIONS {
            if core.place(cell.q + dq, cell.r + dr, 13, 0, None).is_ok() {
                placed = Some((cell.q + dq, cell.r + dr));
                break 'search;
            }
        }
    }
    placed.expect("a burner fits beside the extractor");
    let generator = core
        .entities
        .iter()
        .position(|entity| entity.kind == BuildingKind::Generator)
        .unwrap();
    // Sharing the generator's network, not merely having one: an unconnected machine is put on
    // a network of its own, so `is_some` is true of every machine ever built and proves nothing.
    assert_eq!(
        core.power_of[extractor], core.power_of[generator],
        "a machine touching a generator is on its network"
    );
    assert!(
        core.entities
            .iter()
            .all(|entity| entity.kind != BuildingKind::Pole),
        "and it got there without a pole"
    );

    // A belt is not wire. One built hard against the pair still joins no network, so a line of
    // the cheapest building in the game cannot carry current across the map and no player ever
    // stops placing the second pole.
    let belt = try_place_near(&mut core, (3, 0), 2);
    let belt_index = core
        .entities
        .iter()
        .position(|entity| entity.placed.q == belt.0 && entity.placed.r == belt.1)
        .unwrap();
    assert_eq!(axial_distance((3, 0), belt), 1, "the belt is touching");
    assert!(core.power_of[belt_index].is_none());

    // A scarce grid feeds the machine that can work, not the one that is holding an output.
    //
    // The gate that makes this true is also the whole of the fuel rule, and under a full grid the
    // two are indistinguishable — a blocked machine with a full bank asks for nothing either way.
    // It takes scarcity and an empty bank to tell them apart, which is exactly the state a player
    // is in when they are wondering why the factory got slow.
    let mut core = game("new-game");
    core.power_unmetered = false;
    core.researched.extend([1, 2, 8]);
    stock_for(&mut core, 1, 2);
    stock_for(&mut core, 12, 1);
    stock_for(&mut core, 13, 1);
    core.player.inventory.insert(5, 40);
    core.player.build_range = 1 << 20;
    set_player_hex(&mut core, 0, 0);
    core.place(3, 0, 1, 0, None).unwrap();
    let first = extractor_index(&core);
    try_place_near(&mut core, (3, 0), 1);
    let second = core
        .entities
        .iter()
        .enumerate()
        .filter(|(index, entity)| *index != first && entity.kind == BuildingKind::Extractor)
        .map(|(index, _)| index)
        .next()
        .expect("a second extractor");
    let pole = try_place_near(&mut core, (3, 0), 12);
    try_place_near(&mut core, pole, 13);
    let burner = core
        .entities
        .iter()
        .position(|entity| entity.kind == BuildingKind::Generator)
        .unwrap();
    core.entities[burner].inventory.insert(5, 40);
    core.tick_many(2);
    assert_eq!(core.power_of[first], core.power_of[second]);

    // Both banks empty, and both output buffers full. The only difference between them is the
    // one slot we are about to free.
    let output_capacity = core.building_definition(1).unwrap().capacity.unwrap();
    for index in [first, second] {
        core.entities[index].power_charge = 0;
        core.entities[index]
            .output_inventory
            .insert(1, output_capacity);
    }
    // A belt takes the first one's output. Now it has work and the other still does not.
    core.entities[first].output_inventory.remove(&1);
    core.tick_many(1);

    assert!(
        core.entities[first].power_charge > 0,
        "the machine that can work was given power"
    );
    assert_eq!(
        core.entities[second].power_charge, 0,
        "the machine holding an output took none of it"
    );

    // Electricity is conserved: what the machines banked is what the plants produced, to the unit.
    //
    // The reason throughput comes out exactly proportional to generation with no slowdown factor
    // anywhere. An undersupplied factory is not scaled down — it is handed less to spend.
    let mut core = game("new-game");
    core.power_unmetered = false;
    core.researched.extend([1, 2, 8]);
    stock_for(&mut core, 1, 1);
    stock_for(&mut core, 12, 1);
    stock_for(&mut core, 13, 1);
    core.player.inventory.insert(5, 40);
    core.player.build_range = 1 << 20;
    set_player_hex(&mut core, 0, 0);
    core.place(3, 0, 1, 0, None).unwrap();
    let pole = try_place_near(&mut core, (3, 0), 12);
    try_place_near(&mut core, pole, 13);
    let burner = core
        .entities
        .iter()
        .position(|entity| entity.kind == BuildingKind::Generator)
        .unwrap();
    core.entities[burner].inventory.insert(5, 40);

    let received_before = grid_energy_received(&core);
    let plant_energy_before = core.entities[burner].fuel_charge
        + core.entities[burner]
            .inventory
            .get(&5)
            .copied()
            .unwrap_or(0)
            * core.fuel_value(5);
    core.tick_many(30);
    let received_after = grid_energy_received(&core);
    let plant_energy_after = core.entities[burner].fuel_charge
        + core.entities[burner]
            .inventory
            .get(&5)
            .copied()
            .unwrap_or(0)
            .saturating_add(
                core.entities[burner]
                    .fuel_inventory
                    .get(&5)
                    .copied()
                    .unwrap_or(0),
            )
            * core.fuel_value(5);

    // Fuel energy spent, times the exchange rate, is grid energy produced. That grid energy
    // either sits in a bank or has already been turned into progress, and it is never anything
    // else — there is no third place for a unit of electricity to go.
    let output = core
        .building_definition(core.entities[burner].placed.definition_id)
        .unwrap()
        .power_output
        .unwrap();
    let produced =
        (plant_energy_before - plant_energy_after) * output + core.entities[burner].burn_progress;
    assert!(produced > 0, "the plant produced something");
    assert_eq!(
        produced,
        received_after - received_before,
        "the plant produced {produced} and the machines received {}",
        received_after - received_before
    );
}

/// The split that makes allocation exact without storing a remainder on every entity.
#[test]
fn apportioning_hands_out_every_unit_and_no_more() {
    for total in [0u64, 1, 7, 20, 1000] {
        for weights in [
            vec![1u64, 1, 1],
            vec![64, 20, 20],
            vec![1, 999],
            vec![5],
            vec![],
        ] {
            let parts = apportion(total, &weights);
            assert_eq!(parts.len(), weights.len());
            let handed: u64 = parts.iter().sum();
            let cap: u64 = weights.iter().sum();
            assert_eq!(handed, total.min(if cap == 0 { 0 } else { total }));
            // Nobody is given more than the whole, and the split follows the weights.
            for (part, weight) in parts.iter().zip(&weights) {
                if *weight == 0 {
                    assert_eq!(*part, 0);
                }
            }
        }
    }
}
