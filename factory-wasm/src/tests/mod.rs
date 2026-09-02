mod petroleum;

use super::*;

const DEFINITIONS: &str = include_str!("../../../src/data/definitions.json");
const TECHNOLOGIES: &str = include_str!("../../../src/data/technologies.json");
const SCENARIOS: &str = include_str!("../../../src/data/scenarios.json");

fn assert_refused_as_legacy_scale(result: Result<Core, String>) {
    match result {
        Ok(_) => panic!("1 m² saves cannot load after the scale break"),
        Err(err) => assert!(
            err.contains("one square metre"),
            "expected a scale-break refusal, got {err}"
        ),
    }
}

fn catalogs() -> (DefinitionsInput, TechnologiesInput, ScenariosInput) {
    let definitions = serde_json::from_str(DEFINITIONS).unwrap();
    let technologies = serde_json::from_str(TECHNOLOGIES).unwrap();
    let scenarios = serde_json::from_str(SCENARIOS).unwrap();
    validate_all(&definitions, &technologies, &scenarios).unwrap();
    (definitions, technologies, scenarios)
}

/// The bounded idle batch the host sends on a frame with no held key.
const IDLE: &str = r#"[{"type":"move_intent","x":0,"y":0}]"#;
/// The bounded batch the host sends on a frame with the east movement key held.
const IDLE_MOVE_EAST: &str = r#"[{"type":"move_intent","x":1000,"y":0}]"#;

fn test_factory(key: &str) -> Factory {
    let (definitions, technologies, scenarios) = catalogs();
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|value| value.key == key)
        .unwrap()
        .clone();
    let mut core = Core::new(&definitions, &technologies, &scenario, None, None).unwrap();
    // Same fixture as `game`: these tests name the hexes they build on, so the ground under
    // them is authored rather than generated.
    level_opening(&mut core);
    Factory {
        definitions,
        technologies,
        scenarios,
        core,
        snapshot_revision: 0,
        baseline: None,
    }
}

/// Assert that the delta the shipped builder produces from its dirty marks is byte-identical to
/// the one a full diff of two complete snapshots would have produced, then advance the oracle.
fn assert_delta_matches_full_diff(factory: &mut Factory, previous: &mut Snapshot, step: &str) {
    let current = factory.core.snapshot();
    let base_revision = factory.snapshot_revision;
    let oracle = SnapshotDelta::between(base_revision, base_revision + 1, previous, &current);
    let actual = factory.build_delta();
    assert_eq!(
        serde_json::to_string(&actual).unwrap(),
        serde_json::to_string(&oracle).unwrap(),
        "dirty-tracked delta diverged from the full snapshot diff after {step}"
    );
    // The binary wire has to carry exactly this object and nothing less. Round-tripping here
    // rather than in a test of its own means every delta this run produces is covered — a full
    // replace, an incremental patch, a removal list, terrain arriving, a deposit running dry,
    // a fuelled machine mid-craft — which is the entity and group variety a hand-written
    // fixture cannot enumerate. `fixtures/snapshot-delta-wire.json` pins the other half: that
    // the TypeScript decoder reads the same bytes the same way.
    assert_eq!(
        wire::decode::decode_delta(&wire::encode_delta(&actual)),
        actual,
        "binary wire round trip lost part of the delta after {step}"
    );
    *previous = current;
}

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

/// Every unit of electricity the grid has handed to a machine, wherever it now sits.
///
/// Three places and only three: still banked, already turned into progress, or turned into a
/// finished thing. A machine's `progress` resets when it produces, so the last of those has to
/// be counted from the cargo or conservation would read as a leak every time something came
/// out of a machine.
fn grid_energy_received(core: &Core) -> u32 {
    core.entities
        .iter()
        .enumerate()
        .map(|(index, entity)| {
            let Some(definition) = core.building_definition(entity.placed.definition_id) else {
                return entity.power_charge;
            };
            let draw = definition.power_draw.unwrap_or(0);
            // A finished cycle cost whatever that entity's cycle actually is. Asking
            // `progress_total` rather than the building's `cadence` is what keeps this honest
            // now that an extractor's cycle comes from the material it is standing on: the
            // flat cadence would price a coal cycle at a fifth of what the machine paid.
            let buffered_cycles = match entity.kind {
                BuildingKind::Composer => entity
                    .placed
                    .recipe_id
                    .and_then(|id| core.recipe(id))
                    .map_or(0, |recipe| {
                        entity
                            .output_inventory
                            .get(&recipe.output.item_id)
                            .copied()
                            .unwrap_or(0)
                            / recipe.output.quantity.max(1)
                    }),
                BuildingKind::Extractor | BuildingKind::Pump => {
                    inventory_total(&entity.output_inventory)
                }
                _ => 0,
            } + u32::from(entity.cargo.is_some());
            let finished = buffered_cycles * core.progress_total(index) * draw;
            entity.power_charge + entity.progress * draw + finished
        })
        .sum()
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

fn building_by_key(key: &str) -> BuildingDefinition {
    let (definitions, _, _) = catalogs();
    definitions
        .buildings
        .iter()
        .find(|building| building.key == key)
        .unwrap_or_else(|| panic!("building {key} exists"))
        .clone()
}
/// Stock the player with `times` copies of what `definition_id` is billed, on top of whatever
/// they already hold.
///
/// A test that only needs a station *standing* says what it is building rather than what that
/// is made of, so a repricing pass moves this one function instead of every test that builds
/// something. A test that is about a price — that a bill is deducted, refused, or refunded —
/// still names the bill, because there the bill is the subject.
fn stock_for(core: &mut Core, definition_id: DefinitionId, times: u32) {
    let bill = core
        .building_definition(definition_id)
        .unwrap_or_else(|| panic!("building definition {definition_id} exists"))
        .construction_cost
        .clone();
    for ingredient in bill {
        *core.player.inventory.entry(ingredient.item_id).or_insert(0) +=
            ingredient.quantity * times;
    }
}

fn extractor_index(core: &Core) -> usize {
    core.entities
        .iter()
        .position(|entity| entity.kind == BuildingKind::Extractor)
        .unwrap()
}

/// The cells the mechanics suite draws from, and the reason they are a fixture.
///
/// These are the eight the landing clearing used to guarantee. They stopped being geography in
/// v0.21 — the generator places the opening outside the clearing now, and inside it there is
/// nothing at all — but a test about belts, power, gathering reach, or an upgrade wants a
/// deposit at a hex it can *name*. Standing those on generated ground would turn every one of
/// them into a test about the generator, and a tuning pass would break forty tests that are
/// not about tuning. Writing them into the overlay is exactly what "only the overlay is state"
/// already means, and it is what a scenario file does for a hand-authored map.
///
/// Stone sits on the cliff at `(1, -1)`, which nothing can stand on: it is taken from the hex
/// beside it, and that is what `extraction_reach_comes_from_the_definition` is checking.
const TEST_FIELD: [(i32, i32, ItemId, u32); 8] = [
    (3, 0, IRON_ORE, 48),
    (-2, 2, CRYSTAL, 32),
    (0, -3, COPPER_ORE, 40),
    (2, -3, COAL, 28),
    (1, -1, STONE, 40),
    (1, 3, SAND, 30),
    (-1, 3, CLAY, 26),
    (-3, 1, WOOD, 14),
];

fn bare_game(key: &str) -> Core {
    let (definitions, technologies, scenarios) = catalogs();
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|value| value.key == key)
        .unwrap();
    let mut core = Core::new(&definitions, &technologies, scenario, None, None).unwrap();
    // Isolated machine tests are not the power suite. They opt into the constraint.
    core.power_unmetered = true;
    core
}

/// Stand the mechanics suite on level ground, for the same reason [`TEST_FIELD`] plants the
/// deposits it gathers from — one layer further down.
///
/// These tests name the hexes they use: place at `(3, 0)`, drag a belt to `(4, 1)`, reach from
/// `(0, 0)`. Under Phase 8 those hexes stand on real generated relief, so half a metre of
/// valley moving turned a dozen tests about belts, power and undo into tests about the
/// generator. That is exactly the trade [`TEST_FIELD`] refused for deposits and then left open
/// for height.
///
/// The flattening is the generator's own `generated_environment: false` branch rather than a
/// pad written into the earthwork overlay. The overlay was tried first and is the wrong seam:
/// it is player state that erase and undo are *defined* to unwind, so the fixture and the
/// mechanics under test were reaching for the same field. This stays the physical source —
/// `ground_is_physical` still holds — with a flat height field under it.
///
/// A test whose subject *is* generated ground wants [`field_game`] and its real relief.
fn level_opening(core: &mut Core) {
    core.scenario.generated_environment = false;
    core.ground_spine = GroundSpine::physical(&core.world_params, core.seed, false);
    core.fields = WorldFields::new(&core.world_params, core.seed, &core.ground_spine);
    core.ground_spine
        .rebuild_cache(&core.generated_chunks, core.scenario.chunk_size);
}

/// [`TEST_FIELD`]'s deposits over the world's own relief. For tests whose subject *is* the
/// ground: water finding a cut, a ford, a quarried cliff, a pump in a basin.
fn field_game(key: &str) -> Core {
    let mut core = bare_game(key);
    for &(q, r, item_id, quantity) in &TEST_FIELD {
        core.write_overlay(q, r, item_id, quantity, quantity);
    }
    core.dirty = SnapshotDirty::default();
    core
}

fn game(key: &str) -> Core {
    let mut core = field_game(key);
    level_opening(&mut core);
    core.dirty = SnapshotDirty::default();
    core
}

/// A bounded compatibility fixture for rules whose subject is one of the old presentation
/// bands, not Phase 8 generation. Production worlds never construct this source after save 37.
fn legacy_band_game(key: &str) -> Core {
    // Deliberately not `game`: the legacy source ships its own flat clearing, and a levelling
    // delta measured against the physical bed would be meaningless once the spine is swapped.
    let mut core = field_game(key);
    core.ground_spine = GroundSpine::legacy(
        &core.world_params,
        core.seed,
        core.scenario.generated_environment,
    );
    core.fields = WorldFields::new(&core.world_params, core.seed, &core.ground_spine);
    core
}

/// Every save written before the physical world is refused, and refused with a way out.
///
/// Nine migration tests used to stand here, one per rung of the old ladder. The scale break
/// retired all of them at once: a file from any of those versions is turned away at the
/// envelope now, so each of those tests had become this one assertion followed by unreachable
/// legacy code. This is what is left, and it is the whole claim.
#[test]
fn a_pre_physical_save_is_refused_with_an_export_offered() {
    assert_pre_physical_save_is_refused();
}

fn assert_pre_physical_save_is_refused() {
    let (definitions, technologies, scenarios) = catalogs();
    let legacy = format!("{SAVE_PREFIX}{{\"save_version\":36,\"world_generator_version\":10}}");
    let error = match Core::from_save(&definitions, &technologies, &scenarios, &legacy) {
        Ok(_) => panic!("a one-square-metre save crossed the physical-world boundary"),
        Err(error) => error,
    };
    assert!(error.contains("one square metre"), "{error}");
    assert!(error.contains("export"), "{error}");
    assert!(error.contains("25 m²"), "{error}");
}

/// The rows actually posted, in slot order. The snapshot carries the whole catalogue now, so
/// "the board" is a filter over it rather than the list itself.
fn posted_board(core: &Core) -> Vec<String> {
    core.request_snapshots()
        .iter()
        .filter(|request| request.state == ProjectState::Posted)
        .map(|request| request.key.clone())
        .collect()
}

/// A project's id from the key the catalogue calls it by.
fn project_id(core: &Core, key: &str) -> RequestId {
    core.definitions
        .requests
        .iter()
        .find(|request| request.key == key)
        .map(|request| request.id)
        .expect("a project in the catalogue")
}

/// The four starter automation technologies the hub grants after Prove the line.
fn grant_foundations(core: &mut Core) {
    core.researched.extend([1, 2, 4, 8]);
    core.apply_research_effects();
}

/// Work a swing through to the step it lands on, the way a player does — on their own clock,
/// with the factory untouched. Spends whatever the last gather actually cost, so a coal seam
/// and a wood cell share one helper.
fn cooldown(core: &mut Core) {
    let remaining = core.player.action_cooldown.max(1);
    core.advance_player_steps(remaining);
}

fn set_player_hex(core: &mut Core, q: i32, r: i32) {
    (core.player.x, core.player.y) = axial_world(q, r);
    core.ensure_neighborhood(core.player.x, core.player.y);
}

fn try_place_near(core: &mut Core, origin: (i32, i32), definition_id: DefinitionId) -> (i32, i32) {
    for radius in 1..=6 {
        for dq in -radius..=radius {
            for dr in -radius..=radius {
                if axial_distance((0, 0), (dq, dr)) != radius {
                    continue;
                }
                let q = origin.0 + dq;
                let r = origin.1 + dr;
                if core.place(q, r, definition_id, 0, None).is_ok() {
                    return (q, r);
                }
            }
        }
    }
    panic!("no legal site for definition {definition_id} near {origin:?}");
}

fn add_test_belt(core: &mut Core, q: i32, r: i32, orientation: u8) -> u32 {
    add_test_entity(core, q, r, 2, orientation)
}

/// One entity dropped straight into the world, past terrain, cost, and research.
///
/// The junction tests are about what the graph compiles and how the tick arbitrates between
/// compiled edges. Going through `place` would make each of them also a test of where the
/// new-game landscape happens to have six flat hexes in the right arrangement.
fn add_test_entity(
    core: &mut Core,
    q: i32,
    r: i32,
    definition_id: DefinitionId,
    orientation: u8,
) -> u32 {
    let id = core.next_entity_id;
    core.next_entity_id += 1;
    let kind = core.building_definition(definition_id).unwrap().kind;
    core.entities.push(Entity {
        id,
        placed: PlacedBuilding {
            q,
            r,
            definition_id,
            orientation,
            recipe_id: None,
            scenario_owned: false,
        },
        kind,
        cargo: None,
        inventory: BTreeMap::new(),
        input_inventory: BTreeMap::new(),
        fuel_inventory: BTreeMap::new(),
        output_inventory: BTreeMap::new(),
        reserved_inputs: BTreeMap::new(),
        progress: 0,
        fuel_charge: 0,
        power_charge: 0,
        burn_progress: 0,
        disabled: false,
        route_cursor: 0,
        merge_cursor: 0,
        lane: Vec::new(),
    });
    id
}

/// A world with nothing in it, so a junction test states its whole factory in six lines.
fn empty_world(key: &str) -> Core {
    let mut core = game(key);
    core.entities.clear();
    core.graph.clear();
    core.next_entity_id = 1;
    core
}

fn index_of(core: &Core, id: u32) -> usize {
    core.entities
        .iter()
        .position(|entity| entity.id == id)
        .unwrap_or_else(|| panic!("entity {id} is gone"))
}

/// Every target this entity compiled, by id and in ascending order.
fn link_ids(core: &Core, id: u32) -> Vec<u32> {
    let mut ids: Vec<u32> = core.graph[index_of(core, id)]
        .iter()
        .map(|target| core.entities[target].id)
        .collect();
    ids.sort_unstable();
    ids
}

fn put_cargo(core: &mut Core, id: u32, item_id: ItemId) {
    let index = index_of(core, id);
    core.entities[index].cargo = Some(Cargo {
        item_id,
        quantity: 1,
    });
}

/// The one output this entity compiled, asserting on the way past that it compiled no others.
/// Every caller here builds ordinary belts, so a branch appearing on one would be a defect the
/// old `Option<usize>` graph could not have expressed and this helper would otherwise hide.
fn sole_link(links: &BTreeMap<u32, LinkIds>, id: u32) -> Option<u32> {
    let ids = links[&id];
    assert!(
        ids[1..].iter().all(Option::is_none),
        "entity {id} compiled a branch"
    );
    ids[0].map(|link| link.target_id)
}

#[test]
fn native_and_host_agree_on_directions_passability_heights_and_hexes() {
    let fixture: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../../../fixtures/hex-directions.json")).unwrap();
    let actual: Vec<(i32, i32)> = fixture
        .iter()
        .map(|entry| {
            (
                entry["q"].as_i64().unwrap() as i32,
                entry["r"].as_i64().unwrap() as i32,
            )
        })
        .collect();
    assert_eq!(actual, TRANSPORT_DIRECTIONS);

    // Which bands the player cannot stand on is native's rule, and since v0.12.3 the renderer
    // draws that category before it draws the material — so the host holds a copy of the rule and
    // a copy is a thing that drifts. This is the `fixtures/hex-directions.json` idiom applied to
    // it: Rust asserts the file against the predicates, `tests/host.test.ts` asserts it against
    // `src/core/terrain.ts`, and neither side may move without the other.
    #[derive(Deserialize)]
    struct PassabilityEntry {
        terrain: Terrain,
        passable: bool,
        buildable: bool,
    }
    #[derive(Deserialize)]
    struct PhysicalEntry {
        substrate: String,
        slope: i32,
        water_depth: i32,
        passable: bool,
        buildable: bool,
    }
    #[derive(Deserialize)]
    struct PassabilityFixture {
        bands: Vec<PassabilityEntry>,
        physical: Vec<PhysicalEntry>,
    }

    const BANDS: [Terrain; 7] = [
        Terrain::DeepWater,
        Terrain::ShallowWater,
        Terrain::Shore,
        Terrain::Lowland,
        Terrain::Hills,
        Terrain::Highland,
        Terrain::Cliff,
    ];
    // A band added to the enum makes this match non-exhaustive, which is what sends whoever
    // added it to `BANDS` above and to the fixture beside it.
    for band in BANDS {
        match band {
            Terrain::DeepWater
            | Terrain::ShallowWater
            | Terrain::Shore
            | Terrain::Lowland
            | Terrain::Hills
            | Terrain::Highland
            | Terrain::Cliff => {}
        }
    }

    let fixture: PassabilityFixture =
        serde_json::from_str(include_str!("../../../fixtures/terrain-passability.json")).unwrap();
    assert_eq!(
        fixture.bands.len(),
        BANDS.len(),
        "a band has no fixture entry"
    );
    for (index, (entry, band)) in fixture.bands.iter().zip(BANDS).enumerate() {
        assert_eq!(entry.terrain, band, "fixture is in declaration order");
        // `world_preview_bytes` sends a band as its position in this list and nothing else, so
        // the row a host reads a preview byte through is pinned to the cast that wrote it.
        assert_eq!(band as u8, index as u8, "{band:?} moved in the declaration");
        assert_eq!(
            entry.passable,
            !band.blocks_movement(),
            "{band:?} passability disagrees with the fixture"
        );
        assert_eq!(
            entry.buildable,
            !band.blocks_construction(),
            "{band:?} buildability disagrees with the fixture"
        );
        let finished = FinishedGround {
            generated: GeneratedGround::from_legacy_band(band),
            earthwork: GroundDelta::default(),
            erosion: GroundDelta::default(),
            surface: 0,
        };
        assert_eq!(
            entry.passable,
            !finished.blocks_movement(),
            "{band:?} changed while passing through the ground spine"
        );
        assert_eq!(
            entry.buildable,
            !finished.blocks_construction(),
            "{band:?} changed while passing through the ground spine"
        );
    }
    assert!(
        fixture.physical.iter().any(|entry| entry.water_depth > 0)
            && fixture.physical.iter().any(|entry| entry.slope > 0),
        "physical cases must exercise both halves of access"
    );
    for entry in fixture.physical {
        // Substrate is deliberately present even though it does not block today. Adding a
        // resistance rule later must move this exhaustive match and both fixture readers.
        match entry.substrate.as_str() {
            "soil" | "sand" | "meadow" | "rock" => {}
            other => panic!("unknown substrate {other}"),
        }
        assert_eq!(
            entry.passable,
            entry.slope <= scale::MAX_WALK_STEP_QUANTA
                && entry.water_depth < scale::WADE_LIMIT_QUANTA
        );
        assert_eq!(
            entry.buildable,
            entry.slope <= scale::MAX_BUILD_STEP_QUANTA && entry.water_depth == 0
        );
    }

    // The wire's `height` is an integer in whatever unit the active ground source counts in, and
    // the renderer has to turn it into a scene height. That conversion is a copy of a native fact,
    // and a copy is a thing that drifts — so it goes through the `fixtures/hex-directions.json`
    // idiom rather than through a constant somebody remembers to change.
    //
    // `height_unit` is the one that matters at the compatibility boundary. Production still builds
    // `GroundSpine::legacy`, whose height is a presentation band step; when the physical source
    // activates the same field becomes a 0.25 m quantum, the number stays an integer, and nothing
    // in the payload announces that the world got seventeen times taller. This test is what makes
    // that switch reach `src/rendering/sceneScale.ts` in the same commit.
    #[derive(Deserialize)]
    struct SceneScale {
        height_unit: String,
        height_quantum_mm: i32,
        cell_circumradius_mm: i32,
        max_walk_step: i32,
        relief_min: i32,
        relief_max: i32,
    }

    let fixture: SceneScale =
        serde_json::from_str(include_str!("../../../fixtures/scene-scale.json")).unwrap();
    assert_eq!(fixture.height_quantum_mm, scale::HEIGHT_QUANTUM_MM);
    assert_eq!(fixture.cell_circumradius_mm, scale::CELL_CIRCUMRADIUS_MM);
    // Read out of a real Core rather than declared here, so the fixture answers to the source
    // production constructs and not to a second opinion about which one that is.
    let physical = test_factory("new-game").core.ground_is_physical();
    assert_eq!(
        fixture.height_unit,
        if physical { "quantum" } else { "legacy_step" },
        "the fixture names a height unit the shipped ground source does not publish"
    );
    assert_eq!(
        fixture.max_walk_step,
        if physical {
            scale::MAX_WALK_STEP_QUANTA
        } else {
            MAX_WALK_STEP
        },
        "the renderer's cliff threshold is not the step the player can climb"
    );
    // The full reach of finished ground: the generated bed's own range, opened at both ends by
    // everything the player is allowed to dig or pile on top of it. The camera brackets its
    // depth range with this, so a summit that native can generate is a summit the camera can
    // still draw and still pick.
    let (relief_min, relief_max) = if physical {
        (
            scale::BED_MIN_QUANTA - scale::EARTHWORK_LIMIT_QUANTA,
            scale::BED_MAX_QUANTA + scale::EARTHWORK_LIMIT_QUANTA,
        )
    } else {
        let steps = i32::from(MAX_GRADE_STEPS);
        (
            ground_spine::legacy_band_elevation(Terrain::DeepWater) - steps,
            ground_spine::legacy_band_elevation(Terrain::Cliff) + steps,
        )
    };
    assert_eq!(fixture.relief_min, relief_min);
    assert_eq!(fixture.relief_max, relief_max);

    // A preview pixel is turned into a world point and the point into the hex holding it. The
    // round trip is what makes that a picture of the map rather than of a sheared rhombus, and it
    // has to hold on both sides of the origin — truncating division is exactly the bug that would
    // pass the northern half and shear the southern one.
    for q in -40..=40 {
        for r in -40..=40 {
            let (x, y) = axial_world(q, r);
            assert_eq!(
                hex_at_world(i64::from(x), i64::from(y)),
                (q, r),
                "centre of hex {q},{r}"
            );
        }
    }
}

/// The preview exists so a player can see a parameter set before playing it, which is only
/// worth anything if it is the set that gets played. These are the properties that make it one
/// picture of one world rather than a second generator that happens to look similar.
#[test]
fn world_preview_rasters_the_world_the_run_would_generate() {
    let factory = test_factory("new-game");
    let params = factory.core.world_params.clone();
    let seed = factory.core.seed;
    let json = serde_json::to_string(&params).unwrap();
    let (width, height) = (64u32, 48u32);
    let cells = factory
        .preview_cells(&json, seed, width, height, 512)
        .expect("a shipped parameter set rasters");
    assert_eq!(cells.len(), (width * height) as usize);
    assert!(
        cells.iter().all(|&band| band <= Terrain::Cliff as u8),
        "a preview byte is a band index"
    );
    // The window is centred on the landing site, so the middle pixel is the clearing that
    // `terrain_at` forces there. That pins the centring and the encoding together.
    let centre = (height / 2 * width + width / 2) as usize;
    assert_eq!(cells[centre], Terrain::Lowland as u8);

    // The picture is of these parameters and not of a cached world: raising the sea to just
    // under the shore cut has to flood ground that was dry.
    let water = |cells: &[u8]| {
        cells
            .iter()
            .filter(|&&band| {
                band == Terrain::DeepWater as u8 || band == Terrain::ShallowWater as u8
            })
            .count()
    };
    let flooded = WorldParams {
        water_level: params.shore_level - 1,
        ..params.clone()
    };
    let risen = factory
        .preview_cells(
            &serde_json::to_string(&flooded).unwrap(),
            seed,
            width,
            height,
            512,
        )
        .unwrap();
    assert!(water(&risen) > water(&cells), "a risen sea floods nothing");

    // A set `Core::new` would refuse is refused here too, rather than drawn or divided by. A
    // slider mid-drag is the caller this is for.
    let broken = WorldParams {
        site_cell: 0,
        ..params.clone()
    };
    assert!(factory
        .preview_cells(
            &serde_json::to_string(&broken).unwrap(),
            seed,
            width,
            height,
            512
        )
        .is_err());

    // Deposits are reported as lattice centres rather than sampled, so what pins them is the
    // lattice: `site_cell` is how far apart sites stand, and a window of fixed size holds fewer
    // of them when they stand further apart.
    let factory = test_factory("new-game");
    let params = factory.core.world_params.clone();
    let seed = factory.core.seed;
    let read = |params: &WorldParams, across: u32| -> PreviewSites {
        factory
            .preview_sites(
                &serde_json::to_string(params).unwrap(),
                seed,
                64,
                48,
                across,
            )
            .expect("a shipped parameter set reports sites")
    };

    let shipped = read(&params, 64);
    assert!(!shipped.sites.is_empty(), "a shipped world holds deposits");
    assert_eq!(shipped.total as usize, shipped.sites.len());
    assert!(!shipped.dense);
    // `Core::new` built this world, so its opening is met — a preview claiming otherwise would
    // be warning about a world that starts fine.
    assert!(shipped.unmet.is_empty());

    let sparse = read(
        &WorldParams {
            site_cell: params.site_cell * 2,
            ..params.clone()
        },
        64,
    );
    assert!(
        sparse.total < shipped.total,
        "doubling the lattice left the window as crowded"
    );

    // Wide enough to hold more deposits than are worth drawing: the count still travels, the
    // list does not, and `dense` is what tells the two apart from a world with no deposits.
    let wide = read(&params, MAX_PREVIEW_SPAN);
    assert!(wide.dense);
    assert!(wide.sites.is_empty());
    assert!(wide.unmet.is_empty(), "the bootstrap verdict still travels");
}

/// A parameter set that drowns the highlands, which is the shape of the complaint this
/// diagnosis exists for: the iron and stone rules seat their centres on ground the world no
/// longer has near the landing site.
fn drowned_params(base: &WorldParams) -> WorldParams {
    WorldParams {
        water_level: 50_000,
        shore_level: 52_000,
        hills_level: 58_000,
        highland_level: 62_000,
        ..base.clone()
    }
}

#[test]
fn a_world_that_opens_is_diagnosed_repaired_and_free_of_legacy_band_cuts() {
    let factory = test_factory("new-game");
    let params = factory.core.world_params.clone();
    assert!(
        bootstraps(&params, 7),
        "the shipped parameters have to open, or the rest of this proves nothing"
    );
    let (needs, repair) = factory.preview_diagnosis(&params, 7, &[]);
    // Not merely empty: nothing was searched. A repair ladder run over a world nobody is
    // stuck in would be two dozen bootstrap passes behind every slider drag.
    assert!(needs.is_empty());
    assert!(repair.is_none());

    // Physical opening outcrops do not depend on legacy band cuts.
    let factory = test_factory("new-game");
    let params = drowned_params(&factory.core.world_params);
    let spine = GroundSpine::physical(&params, 7, true);
    let (_, unmet) = bootstrap_sites(&params, 7, &spine);
    assert!(
        unmet.is_empty(),
        "physical outcrops must not inherit absent legacy bands: {unmet:?}"
    );
    let (needs, repair) = factory.preview_diagnosis(&params, 7, &[]);
    assert!(needs.is_empty());
    assert!(repair.is_none());

    // Physical opening outcrops survive legacy band controls.
    let factory = test_factory("new-game");
    let base = factory.core.world_params.clone();
    let params = drowned_params(&base);
    let spine = GroundSpine::physical(&params, 7, true);
    let (_, unmet) = bootstrap_sites(&params, 7, &spine);
    assert!(unmet.is_empty(), "physical opening lost {unmet:?}");

    // A sparse site lattice is repaired by a verified change.
    let factory = test_factory("new-game");
    let params = WorldParams {
        site_cell: 128,
        ..factory.core.world_params.clone()
    };
    let spine = GroundSpine::physical(&params, 7, true);
    let (_, unmet) = bootstrap_sites(&params, 7, &spine);
    let unmet: Vec<ItemId> = unmet.iter().map(|&(item_id, _)| item_id).collect();
    assert!(!unmet.is_empty());
    let (_, repair) = factory.preview_diagnosis(&params, 7, &unmet);
    let repair = repair.expect("a sparse lattice has a verified way out");
    let mut fixed = params.clone();
    for change in &repair.changes {
        assert_eq!(read_world_scalar(&params, change.field), Some(change.from));
        write_world_scalar(&mut fixed, change.field, change.to);
    }
    assert!(fixed.validate(&factory.definitions).is_ok());
    assert!(bootstraps(&fixed, repair.seed.unwrap_or(7)));
}

fn read_world_scalar(params: &WorldParams, field: &str) -> Option<i32> {
    WORLD_SCALARS
        .iter()
        .find(|&&(name, _)| name == field)
        .map(|&(_, read)| read(params))
}

fn write_world_scalar(params: &mut WorldParams, field: &str, value: i32) {
    let slot = match field {
        "elevation_coarse_cell" => &mut params.elevation_coarse_cell,
        "elevation_fine_cell" => &mut params.elevation_fine_cell,
        "elevation_coarse_weight" => &mut params.elevation_coarse_weight,
        "moisture_cell" => &mut params.moisture_cell,
        "richness_cell" => &mut params.richness_cell,
        "water_level" => &mut params.water_level,
        "shore_level" => &mut params.shore_level,
        "hills_level" => &mut params.hills_level,
        "highland_level" => &mut params.highland_level,
        "cliff_step" => &mut params.cliff_step,
        "deep_water_moisture" => &mut params.deep_water_moisture,
        "site_cell" => &mut params.site_cell,
        "site_jitter" => &mut params.site_jitter,
        "river_cell" => &mut params.river_cell,
        "river_width" => &mut params.river_width,
        "river_max_elevation" => &mut params.river_max_elevation,
        "ocean_level" => &mut params.ocean_level,
        other => panic!("a repair named a field nothing can set: {other}"),
    };
    *slot = value;
}

#[test]
fn chunk_generation_is_seeded_cached_and_invertible() {
    let mut a = game("new-game");
    let mut b = game("new-game");
    a.generate_chunk(8, -4);
    a.generate_chunk(-6, 3);
    b.generate_chunk(-6, 3);
    b.generate_chunk(8, -4);
    assert_eq!(a.checksum(), b.checksum());
    assert_eq!(coordinate_hash(1213486160, 81, -33), 166_969_415);
    assert_ne!(
        coordinate_hash(1213486160, 81, -33),
        coordinate_hash(1213486161, 81, -33)
    );
    // The site lattice is a cache, and a cache is exactly where order-dependence gets into a
    // generator: `a` walked one chunk first and `b` the other, so their lattices were filled
    // in different orders. Every cell they both hold has to agree, and the cached answer has
    // to be the uncached one — the two halves of "derived state, and derived from what".
    for (&cell, &site) in a.fields.sites.borrow().iter() {
        assert_eq!(site, a.fields.site_uncached(cell, &a.ground_spine));
        if let Some(&other) = b.fields.sites.borrow().get(&cell) {
            assert_eq!(site, other);
        }
    }

    // The cache pays for the site model and must not change it. `field_at` is asked over a disc
    // wide enough to cross many lattice cells, warm and cold, and the two must never disagree.
    let params = preset_params("continental").unwrap();
    let seed = survey::default_seed();
    let spine = GroundSpine::physical(&params, seed, true);
    let warm = WorldFields::new(&params, seed, &spine);
    for (q, r) in hexes_in_radius((14, -9), 24) {
        let cold = WorldFields::new(&params, seed, &spine);
        assert_eq!(
            warm.field_at(q, r, true, &spine),
            cold.field_at(q, r, true, &spine),
            "the cache changed the world at {q},{r}"
        );
        let cell = (
            floor_div(q, params.site_cell),
            floor_div(r, params.site_cell),
        );
        assert_eq!(warm.site_at(cell, &spine), warm.site_uncached(cell, &spine));
    }
    // And the cheap water test the fast path opens with agrees with the band decision it
    // skips, clearing included. If it ever did not, `field_at` would drop deposits silently.
    for (q, r) in hexes_in_radius((0, 0), 40) {
        assert_eq!(
            is_water_at(&params, seed, q, r),
            terrain_at(&params, seed, q, r, true).is_water(),
            "the cheap water test disagrees at {q},{r}"
        );
    }

    // World to axial inverts axial world and rounds to the nearest hex.
    for q in -12..=12 {
        for r in -12..=12 {
            let (x, y) = axial_world(q, r);
            assert_eq!(world_to_axial(x, y), (q, r));
        }
    }
    let (x, y) = axial_world(3, -2);
    assert_eq!(world_to_axial(x + 200, y - 150), (3, -2));
}

#[test]
fn materials_are_generated_where_geography_says_and_harvested_within_a_radius() {
    // The one test that must see an untouched world: the claim is that an unmined field costs
    // nothing stored, and a fixture that pre-writes eight tiles would answer it in advance.
    let mut core = bare_game("new-game");
    assert!(core.ground_is_physical());
    assert!(!core
        .generated_ground_at(0, 0)
        .hydrology
        .depth_quanta
        .is_positive());
    assert!(!core.terrain_blocks_movement(0, 0));
    // The clearing holds no field at all now: the eight hardcoded cells it used to carry were
    // a sample platter, and the opening is placed by the generator outside it.
    for cell in hexes_in_radius((0, 0), LANDING_CLEAR_RADIUS) {
        assert_eq!(core.field_at(cell.0, cell.1), None);
    }
    let cell = *core
        .fields
        .bootstrap
        .values()
        .map(|site| site.center)
        .min()
        .as_ref()
        .expect("a new world guarantees an opening");
    let quantity = core
        .field_at(cell.0, cell.1)
        .expect("a site centre")
        .quantity;
    assert!(quantity > 0);
    assert_eq!(core.deposit_quantity(cell), quantity);
    // Unmined field is derived: the overlay is empty until something is taken, but the
    // snapshot still reports the cell so the host can draw it.
    assert!(core.tiles.is_empty());
    core.ensure_neighborhood(axial_world(cell.0, cell.1).0, axial_world(cell.0, cell.1).1);
    assert!(core
        .resource_snapshots()
        .iter()
        .any(|resource| resource.q == cell.0
            && resource.r == cell.1
            && resource.quantity == quantity));
    let before = core.checksum();
    set_player_hex(&mut core, cell.0, cell.1);
    core.gather().unwrap();
    cooldown(&mut core);
    assert_eq!(core.deposit_quantity(cell), quantity - 1);
    assert_eq!(
        core.tiles[&cell].resource.as_ref().unwrap().quantity,
        quantity - 1
    );
    assert_ne!(core.checksum(), before);

    // An extractor harvests every field cell inside its radius.
    let mut core = game("new-game");
    core.researched.insert(2);
    stock_for(&mut core, 1, 1);
    set_player_hex(&mut core, 3, 1);
    // Two ore cells one step apart, written into the overlay because the clearing generates
    // none: this is a test about which cell inside a reach is drawn from first, and standing
    // it on geography would make it a test about geography.
    core.write_overlay(3, 0, 1, 48, 48);
    core.write_overlay(4, 0, 1, 3, 3);
    core.place(3, 0, 1, 0, None).unwrap();
    let index = core.entity_at(3, 0).unwrap();
    let candidates = core.deposit_candidates(3, 0, EXTRACT_RADIUS);
    assert_eq!(candidates[0], (3, 0));
    assert!(candidates.contains(&(4, 0)));
    assert_eq!(core.extractor_deposit(index), Some((3, 0)));
    core.write_overlay(3, 0, 1, 0, 48);
    assert_eq!(core.extractor_deposit(index), Some((4, 0)));

    // Geography is still the material map. A deposit is a site rather than a per-hex decision now,
    // so what a band holds is the set of rules that may *reach* into it — the member table — and
    // this asserts that set exactly, band by band.
    // Real relief: the subject here *is* the generated ground, so `game`'s level opening
    // would leave nothing to measure.
    let core = field_game("new-game");
    assert!(core.ground_is_physical());

    let mut seen: BTreeMap<Terrain, BTreeSet<ItemId>> = BTreeMap::new();
    let mut land = 0u32;
    let mut fields = 0u32;
    for q in -80..80 {
        for r in -80..80 {
            // The clearing is deliberately not geography, so it is not evidence about which
            // band holds what.
            if axial_distance((0, 0), (q, r)) <= LANDING_CLEAR_RADIUS {
                continue;
            }
            let terrain = core.terrain_at(q, r);
            if !terrain.is_water() {
                land += 1;
            }
            if let Some(field) = core.fields.field_at(q, r, true, &core.ground_spine) {
                fields += 1;
                seen.entry(terrain).or_default().insert(field.item_id);
            }
        }
    }
    // A field is a place. Barren ground has to be the common case, or the landscape is a
    // carpet and a site is stumbled over rather than chosen. The floor keeps a weight change
    // from emptying a band by accident.
    assert!(land > 0);
    assert!(
        fields * 100 < land * 22,
        "fields too dense: {fields} of {land} land hexes"
    );
    assert!(
        fields * 100 > land * 3,
        "fields too sparse: {fields} of {land} land hexes"
    );
    // Physical ground no longer promises that every old presentation band occurs in one local
    // sample. What remains authoritative here is that every authored raw material appears on
    // dry ground and water itself is pumped rather than mined.
    let generated: BTreeSet<ItemId> = seen.values().flatten().copied().collect();
    for item_id in [
        IRON_ORE, COPPER_ORE, COAL, STONE, SAND, CLAY, WOOD, LIMESTONE, CRUDE_OIL,
    ] {
        assert!(
            generated.contains(&item_id),
            "sample generated no item {item_id}"
        );
    }
    // Crystal is deliberately remote and rare; the opening sample is allowed not to contain it.
    // Water is pumped, not mined, which is why a basin can never be emptied. `validate` refuses
    // a rule that names a water band, and this is that refusal seen from the world.
    assert!(!seen.contains_key(&Terrain::DeepWater));
    assert!(!seen.contains_key(&Terrain::ShallowWater));

    // Sandy-looking tiles are the shore band. Clay may still sit on them, but sand has to be
    // what a player walking a beach finds first — not a regional ocean they never reach.
    // Real relief: a shore is a fact about generated ground. See `field_game`.
    let core = field_game("new-game");
    let mut shore = 0u32;
    let mut sand = 0u32;
    let mut clay = 0u32;
    for q in -160..160 {
        for r in -160..160 {
            if axial_distance((0, 0), (q, r)) <= LANDING_CLEAR_RADIUS {
                continue;
            }
            if core.terrain_at(q, r) != Terrain::Shore {
                continue;
            }
            shore += 1;
            let Some(field) = core.fields.field_at(q, r, true, &core.ground_spine) else {
                continue;
            };
            match field.item_id {
                SAND => sand += 1,
                CLAY => clay += 1,
                _ => {}
            }
        }
    }
    assert!(
        shore > 40,
        "the sample has to hold a real shore, saw {shore} shore hexes"
    );
    assert!(
        sand > 0,
        "sandy tiles held no sand at all ({clay} clay on {shore} shore hexes)"
    );
    assert!(
        sand >= clay,
        "sand should be the common field on shore, saw {sand} sand vs {clay} clay on {shore} \
             shore hexes"
    );
}

/// The seed is no longer the only thing a world can differ by. Two parameter sets on the same
/// seed have to be different *landforms*, not the same landform with the cuts moved.
#[test]
fn two_parameter_sets_on_one_seed_are_different_landforms() {
    let seed = survey::default_seed();
    let continental = preset_params("continental").unwrap();
    let basin = preset_params("basin").unwrap();
    // The landing disc is an opening, not a landform: both presets fade toward the same
    // local blend there. The claim is about the world beyond it.
    let inner = landing_radius(&continental).max(landing_radius(&basin)) + 8;
    let outer = inner + 48;
    let mut differing = 0u32;
    let mut hexes = 0u32;
    for q in -outer..outer {
        for r in -outer..outer {
            let distance = axial_distance((0, 0), (q, r));
            if distance <= inner || distance > outer {
                continue;
            }
            hexes += 1;
            if terrain_at(&continental, seed, q, r, true) != terrain_at(&basin, seed, q, r, true) {
                differing += 1;
            }
        }
    }
    assert!(
        differing * 100 > hexes * 60,
        "only {differing} of {hexes} hexes differ between two parameter sets"
    );

    // And the sliders that used to decide what a world looked like no longer reach the landform
    // at all. Feature scale and the four band levels described a world cut out of noise by
    // thresholds; the physical world is a surface with a height, and moving a threshold under it
    // moves nothing. Every hex within a radius of 24 answers exactly the same either way.
    let altered = WorldParams {
        elevation_coarse_cell: 4,
        water_level: 50_000,
        shore_level: 52_000,
        hills_level: 58_000,
        highland_level: 62_000,
        ..continental.clone()
    };
    let first = GroundSpine::physical(&continental, seed, true);
    let second = GroundSpine::physical(&altered, seed, true);
    for (q, r) in hexes_in_radius((0, 0), 24) {
        assert_eq!(
            first.generated_at(q, r),
            second.generated_at(q, r),
            "legacy band/scale sliders leaked into the physical landform at {q},{r}"
        );
    }
}

/// What the opening promises, asserted rather than assumed.
///
/// The eight hardcoded clearing cells are gone, so the guarantee is now something the
/// generator has to *find*: a patch of each material, in its window, big enough to stand an
/// extractor in. Every preset generates the field materials somewhere in the sample, and the
/// seven guaranteed ones land where they were promised — which is what makes the first hour
/// playable rather than just survivable.
///
/// Sand and crystal are deliberately not guaranteed. Sand goes where the ocean gate says a
/// coast is, and crystal is the reason to leave.
#[test]
fn every_preset_opens_a_workable_world_on_any_seed() {
    let (definitions, _, _) = catalogs();
    for preset in world_presets() {
        let params = preset.params.clone();
        params
            .validate(&definitions)
            .unwrap_or_else(|error| panic!("preset {} is invalid: {error}", preset.key));
        let report = survey::run(
            preset.key,
            &params,
            survey::default_seed(),
            survey::DEFAULT_RADIUS,
        );
        for material in &report.materials {
            let nearest = match material.nearest {
                Some(value) => value,
                None if (material.item_id == SAND || material.item_id == CRYSTAL)
                    && report.radius < survey::landscape_radius(params.elevation_coarse_cell) =>
                {
                    // Sand sits on the regional ocean; crystal is the reason to leave. A
                    // 96-hex opening sample of a 512-hex landform often never reaches either,
                    // and that is the world working.
                    continue;
                }
                None => panic!(
                    "preset {} generates no {} anywhere in a {}-hex sample",
                    preset.key, material.name, report.hexes
                ),
            };
            let ceiling = if material.item_id == CRYSTAL || material.item_id == SAND {
                survey::DEFAULT_RADIUS as u32
            } else {
                40 + BOOTSTRAP_WIDEN_CAP as u32
            };
            assert!(
                nearest <= ceiling,
                "preset {}: nearest {} is {nearest} hexes from the landing site",
                preset.key,
                material.name
            );
        }
        for (row, &(item_id, _, ceiling)) in report.bootstrap.iter().zip(&BOOTSTRAP_GUARANTEES) {
            assert_eq!(row.item_id, item_id);
            let walk = row.edge.unwrap_or_else(|| {
                panic!(
                    "preset {} cannot place its guaranteed {}",
                    preset.key, row.name
                )
            });
            // The ceiling is the window's, plus whatever widening the seed needed. The floor
            // is what keeps a guaranteed disc out of the clearing and is never widened.
            assert!(
                walk > LANDING_CLEAR_RADIUS as u32
                    && walk <= (ceiling + BOOTSTRAP_WIDEN_CAP) as u32,
                "preset {}: guaranteed {} is {walk} hexes out",
                preset.key,
                row.name
            );
            assert!(
                row.hexes >= WORKABLE_PATCH_HEXES,
                "preset {}: guaranteed {} is {} hexes, which no extractor can fill from",
                preset.key,
                row.name,
                row.hexes
            );
        }
        // Barren ground stays the common case under every preset, or a site is stumbled over
        // rather than chosen. This is the v0.15 density floor and ceiling, per preset.
        let fields: u32 = report.materials.iter().map(|entry| entry.cells).sum();
        assert!(
            fields * 100 < report.land_hexes * 22 && fields * 100 > report.land_hexes * 3,
            "preset {}: {fields} fields on {} land hexes",
            preset.key,
            report.land_hexes
        );
    }

    // The patch fill is a second pass over the same cells the material counts walked, and every
    // mean, the purity share, and the workable-patch distance are all divided out of its totals.
    // A fill that lost a hex, followed a neighbour of another material, or visited one twice would
    // move all of them at once and none of them visibly, so the accounting is asserted directly
    // rather than inferred from a figure looking plausible.
    //
    // This is the measurement Landforms and Fields v0.21 is tuned against. It has to be trusted
    // before the generator moves, which is why it lands in the same commit as the before figures
    // and ahead of any generation rule.
    let seed = survey::default_seed();
    for preset in world_presets() {
        let report = survey::run(preset.key, &preset.params, seed, 48);
        let mut counted = 0u32;
        let mut pure = 0u32;
        for (material, patch) in report.materials.iter().zip(&report.patches) {
            assert_eq!(
                material.item_id, patch.item_id,
                "preset {}: the two material tables are in different orders",
                preset.key
            );
            assert_eq!(
                patch.hexes, material.cells,
                "preset {}: the {} fill visited {} hexes against {} counted cells",
                preset.key, material.name, patch.hexes, material.cells
            );
            assert_eq!(
                patch.patches == 0,
                patch.hexes == 0,
                "preset {}: {} has {} patches over {} hexes",
                preset.key,
                material.name,
                patch.patches,
                patch.hexes
            );
            assert!(
                patch.largest_patch <= patch.hexes && patch.truncated_patches <= patch.patches,
                "preset {}: {} reports a largest patch of {} and {} truncated of {} over {} \
                     hexes",
                preset.key,
                material.name,
                patch.largest_patch,
                patch.truncated_patches,
                patch.patches,
                patch.hexes
            );
            // A workable patch is at least seven hexes, so claiming one means the largest
            // patch is at least that big, and no patch can start nearer than the nearest cell.
            match patch.nearest_workable_patch {
                Some(distance) => {
                    assert!(
                        patch.largest_patch >= 7,
                        "preset {}: {} claims a workable patch with a largest patch of {}",
                        preset.key,
                        material.name,
                        patch.largest_patch
                    );
                    assert!(
                        distance >= material.nearest.expect("a patch implies a cell"),
                        "preset {}: {} puts a workable patch at {distance}, nearer than its \
                             nearest cell",
                        preset.key,
                        material.name
                    );
                }
                None => assert!(
                    patch.largest_patch < 7,
                    "preset {}: {} has a {}-hex patch and reports none workable",
                    preset.key,
                    material.name,
                    patch.largest_patch
                ),
            }
            counted += patch.hexes;
            pure += patch.purity_per_mille * patch.hexes / 1000;
        }
        assert!(
            counted > 0,
            "preset {} generates nothing at all",
            preset.key
        );
        // The whole-sample purity is the same count divided by the same denominator, so it has
        // to agree with the per-material shares to within their rounding.
        let overall = report.purity_per_mille * counted / 1000;
        assert!(
            overall.abs_diff(pure) <= report.patches.len() as u32,
            "preset {}: whole-sample purity implies {overall} pure hexes against {pure} from \
                 the material rows",
            preset.key
        );
    }

    // **The number this milestone exists for.**
    //
    // A deposit used to be decided per hex from independent noise channels, so along every
    // iron/coal boundary the two alternated hex by hex and an extractor covered both and cleanly
    // worked neither. Purity is the share of resource hexes whose radius-1 disc holds exactly one
    // material, and the measured before figures were `continental` 532, `archipelago` 474,
    // `highlands` 662, `basin` 631 — every preset failing, the wettest failing hardest.
    //
    // It is asserted at 950 rather than at whatever the presets happen to reach, because the
    // point is the model and not the tuning: a rule table that could not clear this bar would
    // mean the lattice had stopped being the thing that decides what a patch is made of.
    let seed = survey::default_seed();
    for preset in world_presets() {
        let report = survey::run(preset.key, &preset.params, seed, survey::DEFAULT_RADIUS);
        assert!(
            report.purity_per_mille >= 950,
            "preset {}: purity is {} per mille",
            preset.key,
            report.purity_per_mille
        );
        // A patch worth automating, per material an extractor is stood on for its own sake.
        // Forests are the one that is measured in area rather than in throughput, so their
        // bar is the deep extractor's disc rather than the base one's.
        for (item_id, floor) in [
            (IRON_ORE, 19),
            (COAL, 19),
            (COPPER_ORE, 19),
            (STONE, 19),
            (WOOD, 61),
        ] {
            let patch = report
                .patches
                .iter()
                .find(|entry| entry.item_id == item_id)
                .expect("every generated item has a row");
            assert!(
                patch.largest_patch >= floor,
                "preset {}: the largest {} patch is {} hexes",
                preset.key,
                patch.name,
                patch.largest_patch
            );
        }
    }

    // The opening is a promise about every seed, not about the shipped one.
    //
    // A guarantee that only holds on the seed it was tuned against is not a guarantee, and the
    // bootstrap pass is the one part of generation that can fail outright — it widens a window in
    // fixed steps and then gives up, and `Core::new` refuses a world it gave up on. So the claim
    // is checked where it would break: every preset, ten seeds, including the presets whose bands
    // are scarce enough to make a window hard to fill.
    let (definitions, technologies, scenarios) = catalogs();
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|value| value.key == "new-game")
        .unwrap();
    for preset in world_presets() {
        for step in 0..10u32 {
            let seed = survey::default_seed().wrapping_add(step.wrapping_mul(0x9E3779B1));
            let spine = GroundSpine::physical(&preset.params, seed, true);
            let fields = WorldFields::new(&preset.params, seed, &spine);
            assert!(
                fields.unmet.is_empty(),
                "preset {} on seed {seed} cannot place {:?}",
                preset.key,
                fields.unmet
            );
            let placed: BTreeMap<ItemId, (u32, u32)> = fields
                .guarantees(&spine)
                .into_iter()
                .map(|(item_id, walk, hexes)| (item_id, (walk, hexes)))
                .collect();
            for &(item_id, floor, _) in &BOOTSTRAP_GUARANTEES {
                let (walk, hexes) = placed[&item_id];
                // The floor is never widened: a guaranteed disc that reached inside the
                // clearing would put a deposit where field suppression deletes it.
                assert!(
                    walk >= floor as u32,
                    "preset {} on seed {seed}: item {item_id} is {walk} hexes out, inside its \
                         floor of {floor}",
                    preset.key
                );
                assert!(
                    hexes >= WORKABLE_PATCH_HEXES,
                    "preset {} on seed {seed}: item {item_id} is {hexes} hexes",
                    preset.key
                );
            }
            // Crystal is the reason to leave, so nothing may guarantee it.
            assert!(!placed.contains_key(&CRYSTAL));
            Core::new(
                &definitions,
                &technologies,
                scenario,
                Some(seed),
                Some(preset.params.clone()),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "preset {} on seed {seed} is unplayable: {error}",
                    preset.key
                )
            });
        }
    }

    // A large landform must not strand the player on the 7-hex clearing. The landing disc fades
    // toward the opening blend and lifts a sea-spawn origin, so the first two dozen hexes stay
    // mostly walkable on every seed of every preset.
    for preset in world_presets() {
        for step in 0..10u32 {
            let seed = survey::default_seed().wrapping_add(step.wrapping_mul(0x9E3779B1));
            let mut blocked = 0u32;
            let mut hexes = 0u32;
            for (q, r) in hexes_in_radius((0, 0), 24) {
                if axial_distance((0, 0), (q, r)) <= LANDING_CLEAR_RADIUS {
                    continue;
                }
                hexes += 1;
                if terrain_at(&preset.params, seed, q, r, true).blocks_movement() {
                    blocked += 1;
                }
            }
            assert!(
                blocked * 100 < hexes * 40,
                "preset {} on seed {seed}: {blocked} of {hexes} hexes in the first 24 are \
                     impassable",
                preset.key
            );
        }
    }
}

/// A world's identity is its seed *and* its parameters, so a scalar the checksum does not read
/// is a scalar two different worlds can silently share. Every one of them is moved, one at a
/// time, and the hash has to move with it.
#[test]
fn world_parameters_are_checksummed_validated_and_restored_with_their_sites() {
    let base = preset_params("continental").unwrap();
    let hash_of = |params: &WorldParams| {
        let mut hash = 0x811c9dc5u32;
        hash_world_params(&mut hash, params);
        hash
    };
    let baseline = hash_of(&base);
    let mut moved: Vec<WorldParams> = Vec::new();
    for shift in [
        |p: &mut WorldParams| p.elevation_coarse_cell += 1,
        |p: &mut WorldParams| p.elevation_fine_cell += 1,
        |p: &mut WorldParams| p.elevation_coarse_weight += 1,
        |p: &mut WorldParams| p.moisture_cell += 1,
        |p: &mut WorldParams| p.richness_cell += 1,
        |p: &mut WorldParams| p.water_level += 1,
        |p: &mut WorldParams| p.shore_level += 1,
        |p: &mut WorldParams| p.hills_level += 1,
        |p: &mut WorldParams| p.highland_level += 1,
        |p: &mut WorldParams| p.cliff_step += 1,
        |p: &mut WorldParams| p.deep_water_moisture += 1,
        |p: &mut WorldParams| p.site_cell += 1,
        |p: &mut WorldParams| p.site_jitter += 1,
        |p: &mut WorldParams| p.river_cell += 1,
        |p: &mut WorldParams| p.river_width += 1,
        |p: &mut WorldParams| p.river_max_elevation += 1,
        |p: &mut WorldParams| p.ocean_level += 1,
        |p: &mut WorldParams| p.site_rules[0].weight += 1,
        |p: &mut WorldParams| p.site_rules[0].radius_min += 1,
        |p: &mut WorldParams| p.site_rules[0].radius_max += 1,
        |p: &mut WorldParams| p.site_rules[0].site_min += 1,
        |p: &mut WorldParams| p.site_rules[0].yield_core += 1,
        |p: &mut WorldParams| p.site_rules[0].yield_rim += 1,
        |p: &mut WorldParams| p.site_rules[0].yield_jitter += 1,
        |p: &mut WorldParams| p.site_rules[0].member_water_within += 1,
        |p: &mut WorldParams| p.site_rules[0].center_ocean = true,
        |p: &mut WorldParams| p.site_rules[0].center_shore = true,
        |p: &mut WorldParams| p.site_rules[0].member.push(Terrain::Cliff),
        |p: &mut WorldParams| p.site_rules[0].item_id = CRYSTAL,
        |p: &mut WorldParams| p.site_rules[0].terrain = Terrain::Shore,
    ] {
        let mut params = base.clone();
        shift(&mut params);
        assert_ne!(
            hash_of(&params),
            baseline,
            "a world parameter changed and the checksum did not"
        );
        moved.push(params);
    }
    // And no two of them collide, which is the failure a per-field test on its own cannot see.
    let mut hashes: Vec<u32> = moved.iter().map(hash_of).collect();
    let total = hashes.len();
    hashes.sort_unstable();
    hashes.dedup();
    assert_eq!(hashes.len(), total, "two parameter changes hash the same");

    // A site's yield falls from its core to its rim, which is what makes the middle of a field
    // worth aiming an extractor at rather than any hex of it being as good as any other.
    let params = preset_params("continental").unwrap();
    let seed = survey::default_seed();
    let spine = GroundSpine::physical(&params, seed, true);
    let fields = WorldFields::new(&params, seed, &spine);
    let mut compared = 0u32;
    let mut core_wins = 0u32;
    for cell in (-8..8).flat_map(|q| (-8..8).map(move |r| (q, r))) {
        let Some(site) = fields.site_at(cell, &spine) else {
            continue;
        };
        let rule = &params.site_rules[site.rule];
        if rule.yield_core == rule.yield_rim || site.radius < 2 {
            continue;
        }
        let Some(center) = fields.field_at(site.center.0, site.center.1, true, &spine) else {
            continue;
        };
        for rim in hexes_in_radius(site.center, site.radius)
            .into_iter()
            .filter(|&cell| axial_distance(site.center, cell) == site.radius)
        {
            let Some(edge) = fields.field_at(rim.0, rim.1, true, &spine) else {
                continue;
            };
            if edge.item_id != center.item_id {
                continue;
            }
            compared += 1;
            core_wins += u32::from(center.quantity > edge.quantity);
        }
    }
    assert!(compared > 20, "only {compared} core/rim pairs to compare");
    // Jitter is deliberately allowed to invert a single pair; a gradient it could hide would
    // be a gradient no player could read.
    assert!(
        core_wins * 100 > compared * 85,
        "the core beat the rim in only {core_wins} of {compared} pairs"
    );

    // A parameter set that is not a world at all is refused before one is built from it. What this
    // deliberately does not try to catch is a set that is a world but an unplayable one — that is
    // what the survey measures, and no validator can decide it.
    let (definitions, technologies, scenarios) = catalogs();
    let base = preset_params("continental").unwrap();
    // One valid row, so each case below differs from a world by exactly the thing it names.
    let one_rule = || SiteRule {
        terrain: Terrain::Hills,
        item_id: IRON_ORE,
        weight: 1,
        radius_min: 1,
        radius_max: 2,
        site_min: ANY,
        yield_core: 4,
        yield_rim: 2,
        yield_jitter: 1,
        member: Vec::new(),
        member_water_within: 0,
        center_ocean: false,
        center_shore: false,
    };
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|value| value.key == "new-game")
        .unwrap();
    let refused = [
        WorldParams {
            elevation_coarse_cell: 0,
            ..base.clone()
        },
        WorldParams {
            elevation_coarse_weight: 140,
            ..base.clone()
        },
        // Bands out of order do not make a band rare; they make it unreachable.
        WorldParams {
            hills_level: 10_000,
            ..base.clone()
        },
        WorldParams {
            site_rules: Vec::new(),
            ..base.clone()
        },
        WorldParams {
            site_rules: vec![SiteRule {
                item_id: 9999,
                ..one_rule()
            }],
            ..base.clone()
        },
        // Yield is `interpolated + hash % yield_jitter`, so a zero jitter is a division by zero.
        WorldParams {
            site_rules: vec![SiteRule {
                yield_jitter: 0,
                ..one_rule()
            }],
            ..base.clone()
        },
        // A radius of zero is a deposit that is not anywhere, and an inverted range would make
        // `radius_max - radius_min + 1` wrap.
        WorldParams {
            site_rules: vec![SiteRule {
                radius_min: 4,
                radius_max: 2,
                ..one_rule()
            }],
            ..base.clone()
        },
        WorldParams {
            site_rules: vec![SiteRule {
                radius_max: MAX_SITE_RADIUS + 1,
                ..one_rule()
            }],
            ..base.clone()
        },
        // A water band would make the cheap water test `field_at` opens with unsound, and a
        // deposit in a basin is nothing a pump or an extractor could reach anyway.
        WorldParams {
            site_rules: vec![SiteRule {
                member: vec![Terrain::Hills, Terrain::DeepWater],
                ..one_rule()
            }],
            ..base.clone()
        },
        // Every row weighted zero is a table that generates nothing at all.
        WorldParams {
            site_rules: vec![SiteRule {
                weight: 0,
                ..one_rule()
            }],
            ..base.clone()
        },
        WorldParams {
            site_jitter: MAX_SITE_JITTER + 1,
            ..base.clone()
        },
    ];
    for params in refused {
        assert!(
            Core::new(&definitions, &technologies, scenario, None, Some(params)).is_err(),
            "a parameter set that is not a world must be refused"
        );
    }
    assert!(preset_params("no-such-preset").is_none());

    // A world's parameters survive the round trip, and the world that comes back is the one that
    // was saved rather than the scenario's default.
    let (definitions, technologies, scenarios) = catalogs();
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|value| value.key == "new-game")
        .unwrap();
    let basin = preset_params("basin").unwrap();
    let mut core = Core::new(
        &definitions,
        &technologies,
        scenario,
        None,
        Some(basin.clone()),
    )
    .unwrap();
    assert_ne!(core.world_params, default_world_params());
    core.tick_many(30);
    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.world_params, basin);
    assert_eq!(restored.checksum(), core.checksum());
    // The default-parameter core is the same scenario, the same seed, and a different world.
    let default = Core::new(&definitions, &technologies, scenario, None, None).unwrap();
    assert_eq!(default.seed, core.seed);
    assert_ne!(
        default.checksum(),
        Core::new(&definitions, &technologies, scenario, None, Some(basin),)
            .unwrap()
            .checksum()
    );
}

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
    // Back to what generation gave it, so it costs nothing again until somebody cuts it.
    assert!(core.flora_regrowth.is_empty());

    // Ore is finite: cutting into a deposit never puts it in the set at all.
    cooldown(&mut core);
    set_player_hex(&mut core, 3, 0);
    core.gather().unwrap();
    cooldown(&mut core);
    assert_eq!(core.deposit_quantity((3, 0)), 47);
    assert!(core.flora_regrowth.is_empty());

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

fn primitive_test_core() -> Core {
    let mut core = game("new-game");
    core.power_unmetered = false;
    core.player.inventory.clear();
    core.player
        .inventory
        .extend([(6, 20), (8, 10), (9, 20), (1, 10)]);
    set_player_hex(&mut core, 0, 3);
    core
}

#[test]
fn skills_are_finite_atomic_and_isolated_from_research() {
    let mut core = game("new-game");
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
    assert_eq!(core.skills.points, 0);
    core.observe_skill_event(SkillEvent::PoweredCraft);
    core.observe_skill_event(SkillEvent::PoweredCraft);
    assert_eq!(core.skills.points, 1);
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
    assert_eq!(core.skills.granted, BTreeSet::from([2, 3]));
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
    assert_eq!(core.skills.points, 1);
    assert!(core.skills.completed.contains(&2));
    core.advance_contract();
    assert_eq!(core.skills.points, 1);
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

#[test]
fn movement_intent_aim_and_cadence_are_native() {
    let mut core = legacy_band_game("new-game");
    // Stay inside the landing clearing so derived water and cliffs cannot interrupt the walk.
    set_player_hex(&mut core, 0, 3);
    let start = (core.player.x, core.player.y);
    core.set_move_intent(707, -707).unwrap();
    core.advance_player_steps(3);
    let step = 707 * PLAYER_SPEED / 1000;
    assert_eq!(core.player.x, start.0 + 3 * step);
    assert_eq!(core.player.y, start.1 - 3 * step);
    assert_eq!((core.player.facing_x, core.player.facing_y), (707, -707));
    core.set_move_intent(0, 0).unwrap();
    core.advance_player_steps(3);
    assert_eq!(
        (core.player.x, core.player.y),
        (start.0 + 3 * step, start.1 - 3 * step)
    );
    assert!(core.set_move_intent(1001, 0).is_err());

    // A guaranteed landing cliff still blocks: stand just west of (1, -1) and walk east.
    let (cliff_x, cliff_y) = axial_world(1, -1);
    core.player.x = cliff_x - HEX_X / 2 - 20;
    core.player.y = cliff_y;
    let blocked_x = core.player.x;
    core.set_move_intent(1000, 0).unwrap();
    core.advance_player_steps(1);
    assert_eq!(core.player.x, blocked_x);
    assert_eq!(core.terrain_at(1, -1), Terrain::Cliff);

    // Shallows are a 5 m/s ford: walkable, not buildable, and the gait does not matter once
    // you are in the water. Deep water stays a wall.
    assert!(!Terrain::ShallowWater.blocks_movement());
    assert!(Terrain::ShallowWater.blocks_construction());
    assert!(Terrain::DeepWater.blocks_movement());
    assert!(Terrain::DeepWater.blocks_construction());

    let mut core = legacy_band_game("new-game");
    set_player_hex(&mut core, 2, 1);
    assert_eq!(core.terrain_at(2, 1), Terrain::ShallowWater);
    let start = (core.player.x, core.player.y);
    let ford = PLAYER_SPEED / 5;

    core.set_move_intent(1000, 0).unwrap();
    core.advance_player_steps(1);
    assert_eq!(core.player.x, start.0 + ford);

    core.player.x = start.0;
    core.set_move_intent(600, 0).unwrap();
    core.advance_player_steps(1);
    assert_eq!(
        core.player.x,
        start.0 + ford,
        "wading is 5 m/s at any gait, not 3/5 of it"
    );

    // Still not a building site: the player can stand in it, a pump cannot.
    set_player_hex(&mut core, 0, 3);
    core.researched.extend([1, 2, 5, 7]);
    core.player.inventory.insert(11, 20);
    core.player.inventory.insert(14, 20);
    assert!(core
        .place(2, 1, 11, 0, None)
        .unwrap_err()
        .contains("environment blocks construction"));

    // Facing became something the player aims rather than a side effect of walking, so the command
    // that sets it has to resolve as natively as the movement it sits beside: the host names a
    // world point and this turns it into the vector the checksum hashes.
    let mut core = game("new-game");
    set_player_hex(&mut core, 0, 3);
    let (x, y) = (core.player.x, core.player.y);

    core.set_aim(x + 5_000, y).unwrap();
    assert_eq!((core.player.facing_x, core.player.facing_y), (1000, 0));
    core.set_aim(x, y - 5_000).unwrap();
    assert_eq!((core.player.facing_x, core.player.facing_y), (0, -1000));

    // A diagonal resolves to a unit vector, not to whatever delta the host happened to send,
    // and pushing the same direction ten times further does not change the answer.
    core.set_aim(x - 3_000, y + 3_000).unwrap();
    assert_eq!((core.player.facing_x, core.player.facing_y), (-707, 707));
    core.set_aim(x - 30_000, y + 30_000).unwrap();
    assert_eq!((core.player.facing_x, core.player.facing_y), (-707, 707));

    // A cursor resting exactly on the player names no direction, so the last one stands.
    core.set_aim(x, y).unwrap();
    assert_eq!((core.player.facing_x, core.player.facing_y), (-707, 707));
    assert!(core.set_aim(x + (MAX_AIM_DISTANCE as i32) + 1, y).is_err());

    // What an aim resolves to is ordinary player state: it is saved, and the save validator
    // that bounds facing accepts it, because native produced it rather than the host.
    let (definitions, technologies, scenarios) = catalogs();
    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(
        (restored.player.facing_x, restored.player.facing_y),
        (-707, 707)
    );

    // What keeps a pointer aiming and a touch layout facing the way it walks, with no stored
    // aiming mode for the save format and the checksum to carry: both commands write facing, and
    // whichever the host sent last in the batch is the one that stands.
    let mut core = game("new-game");
    set_player_hex(&mut core, 0, 3);
    let (x, y) = (core.player.x, core.player.y);
    let batch = format!(
        r#"[{{"type":"move_intent","x":1000,"y":0}},{{"type":"aim","x":{x},"y":{}}}]"#,
        y - 4_000
    );
    core.advance(&batch, 0, 0).unwrap();
    assert_eq!((core.player.facing_x, core.player.facing_y), (0, -1000));

    // A frame with no aim in it — every frame of the touch layout — still faces the walk.
    core.advance(IDLE_MOVE_EAST, 0, 0).unwrap();
    assert_eq!((core.player.facing_x, core.player.facing_y), (1000, 0));

    // Integer square root is exact on squares and truncates between them.
    assert_eq!(integer_sqrt(0), 0);
    assert_eq!(integer_sqrt(-9), 0);
    for root in [1_i64, 2, 3, 1_000, 46_341, 3_037_000_499] {
        assert_eq!(integer_sqrt(root * root), root);
        assert_eq!(integer_sqrt(root * root - 1), root - 1);
    }

    // The player walks on its own cadence not the factorys.
    // The complaint this answers: the player stopped when the factory paused and crawled at a
    // low speed multiplier, because walking ran inside the simulation tick.
    let mut core = game("new-game");
    set_player_hex(&mut core, 0, 3);
    let start = (core.player.x, core.player.y);
    core.set_move_intent(1000, 0).unwrap();

    // A paused factory advances no ticks at all, and the player still walks.
    core.advance(IDLE_MOVE_EAST, 0, 10).unwrap();
    assert_eq!(core.tick, 0);
    assert_eq!(core.player.x, start.0 + 10 * PLAYER_SPEED);

    // Ticking the factory without spending player steps moves nothing.
    let held = core.player.x;
    core.advance("[]", 30, 0).unwrap();
    assert_eq!(core.tick, 30);
    assert_eq!(core.player.x, held);

    // The same step count always covers the same ground, whatever the factory is doing, so a
    // replay of the same commands and counts still reproduces the same position.
    let mut slow = game("new-game");
    let mut fast = game("new-game");
    for core in [&mut slow, &mut fast] {
        set_player_hex(core, 0, 3);
    }
    for _ in 0..4 {
        slow.advance(IDLE_MOVE_EAST, 1, 8).unwrap();
        fast.advance(IDLE_MOVE_EAST, 16, 8).unwrap();
    }
    assert_eq!(slow.player.x, fast.player.x);
    assert_eq!(slow.player.y, fast.player.y);
    assert_eq!(Factory::player_ticks_per_second(), PLAYER_TICKS_PER_SECOND);

    // A hexagon is 25 m², the walk is 15 m/s, the run is 25 m/s. Native stores one step size — the
    // run, at intent 1000 — and the host sends 600 for the walk, which is exactly 3/5 of full
    // intent. Neighbour spacing is still `HEX_X` world units, now read as 5.373 m.
    //
    // The gait ratio is the structural half and holds at any speed; the pinned constant is the
    // half that carries the decision. `PLAYER_SPEED` stayed at 275 across the rescale, so a hex
    // still takes about 0.36 s to cross at a walk and the metre figures moved instead.
    const WALK_INTENT: i32 = 600;
    let walk = WALK_INTENT * PLAYER_SPEED / 1000;
    assert_eq!(walk * 5, PLAYER_SPEED * 3);
    assert_eq!(PLAYER_SPEED, 275);

    // Metres a second, out of world units a step: 30 steps a second over `HEX_X` units of
    // 5.373 m. Integer throughout, and the run lands on 25 m/s to the metre.
    let run_mm_s =
        PLAYER_SPEED as i64 * PLAYER_TICKS_PER_SECOND as i64 * crate::scale::CELL_SPACING_MM as i64
            / HEX_X as i64;
    assert_eq!(run_mm_s / 1_000, 24);
    assert_eq!((run_mm_s + 500) / 1_000, 25);
    let walk_mm_s =
        walk as i64 * PLAYER_TICKS_PER_SECOND as i64 * crate::scale::CELL_SPACING_MM as i64
            / HEX_X as i64;
    assert_eq!((walk_mm_s + 500) / 1_000, 15);
}

/// Build a wall out of containers, standing next to each cell so the test is about the route
/// rather than about build reach, and free so it is not about costs either.
fn wall(core: &mut Core, cells: &[(i32, i32)]) {
    core.set_creative(true);
    for &(q, r) in cells {
        core.place(q, r, 4, 0, None)
            .unwrap_or_else(|error| panic!("wall segment at {q},{r}: {error}"));
    }
}

/// The six neighbours of a hex, which is what it takes to shut one off from the world.
fn ring(q: i32, r: i32) -> Vec<(i32, i32)> {
    DIRECTIONS.iter().map(|(dq, dr)| (q + dq, r + dr)).collect()
}

/// A route is only a route if it is a chain of neighbours the player can stand on, starting
/// beside where they are and ending on what they asked for.
fn assert_route_is_walkable(core: &Core, from: (i32, i32), goal: (i32, i32)) {
    let path = &core.walk_path;
    assert!(!path.is_empty(), "a route to {goal:?} should have cells");
    assert_eq!((path[path.len() - 1].q, path[path.len() - 1].r), goal);
    let mut previous = from;
    for cell in path {
        assert_eq!(
            axial_distance(previous, (cell.q, cell.r)),
            1,
            "{previous:?} -> {cell:?} is not one step"
        );
        assert!(
            core.walkable_hex(cell.q, cell.r),
            "{cell:?} is not ground the player can stand on"
        );
        previous = (cell.q, cell.r);
    }
}

/// The whole gesture, end to end: a click names a hex, native finds the way, and the player
/// walks it without another command being sent.
#[test]
fn a_click_routes_walks_and_replans_around_what_blocks_it() {
    let mut core = game("new-game");
    // Outside the hub's seven hexes: the hub blocks movement, so a player standing inside it
    // would be measuring collision rather than walking.
    set_player_hex(&mut core, 2, 0);
    core.walk_to(6, 0).unwrap();
    assert_eq!(core.player.walk_goal, Some(Coordinate { q: 6, r: 0 }));
    assert_route_is_walkable(&core, (2, 0), (6, 0));

    // No further input at all — the run below sends an empty batch every frame.
    for _ in 0..40 {
        core.advance("[]", 0, 5).unwrap();
        if core.player.walk_goal.is_none() {
            break;
        }
    }
    assert_eq!(world_to_axial(core.player.x, core.player.y), (6, 0));
    // Arrival ends the walk and drops the intent, so the player stops rather than drifting on.
    assert_eq!(core.player.walk_goal, None);
    assert!(core.walk_path.is_empty());
    assert_eq!((core.player.move_x, core.player.move_y), (0, 0));

    // The route goes round what blocks it. A wall the player built themselves is as real to the
    // search as a cliff is, because both answer the same `walkable_hex`.
    let mut core = legacy_band_game("new-game");
    set_player_hex(&mut core, 1, 0);
    let barrier = [(3, -1), (3, 0), (3, 1)];
    wall(&mut core, &barrier);

    core.walk_to(6, 0).unwrap();
    assert_route_is_walkable(&core, (1, 0), (6, 0));
    for cell in &core.walk_path {
        assert!(
            !barrier.contains(&(cell.q, cell.r)),
            "the route runs through the wall at {cell:?}"
        );
    }
    assert!(
        core.walk_path.len() > axial_distance((1, 0), (6, 0)) as usize,
        "a route round a wall cannot be as short as the straight line it replaces"
    );

    // Water is not scenery to a route. Shallows are walkable and are therefore never refused, but
    // `player_step` fords them at a fifth speed, so the search charges five and takes the long dry
    // way — which is the way the player would have taken, and five times faster.
    // The complaint this answers: the shortest route and the fastest route are not the same
    // route once water is on the map, and the shortest one wades.
    assert_eq!(
        WALK_SHALLOW_COST,
        WALK_STEP_COST * (PLAYER_SPEED / (PLAYER_SPEED / 5)) as u32,
        "the ford's price to the route is the fraction of speed the ford actually costs"
    );

    let mut core = legacy_band_game("new-game");
    set_player_hex(&mut core, 0, 2);
    assert_eq!(core.terrain_at(1, 2), Terrain::ShallowWater);
    assert_eq!(core.terrain_at(2, 2), Terrain::ShallowWater);

    core.walk_to(3, 2).unwrap();
    assert_route_is_walkable(&core, (0, 2), (3, 2));
    for cell in &core.walk_path {
        assert_ne!(
            core.terrain_at(cell.q, cell.r),
            Terrain::ShallowWater,
            "the route wades at {cell:?} when dry ground was cheaper"
        );
    }
    // Three hexes wading costs eleven; four hexes round the south of the water costs four. A
    // search that costed every hex the same would have returned the three, and the player would
    // have spent two of them crossing at 5 m/s.
    assert_eq!(core.walk_path.len(), 4);
    assert_eq!(axial_distance((0, 2), (3, 2)), 3);

    // Three refusals, each an event rather than a silent no-op: the player pointed at something and
    // is owed an answer about it.
    let mut core = legacy_band_game("new-game");
    set_player_hex(&mut core, 1, 0);
    let standing = (core.player.x, core.player.y);

    // Ground the player cannot stand on at all.
    assert_eq!(core.terrain_at(9, 0), Terrain::Cliff);
    assert!(core.walk_to(9, 0).unwrap_err().contains("No way through"));

    // Ground that is fine in itself and walled off from the world.
    wall(&mut core, &ring(4, 0));
    assert!(core.walkable_hex(4, 0));
    assert!(core.walk_to(4, 0).unwrap_err().contains("No way through"));

    // Further than a click is allowed to mean.
    assert!(core
        .walk_to(1 + MAX_WALK_DISTANCE + 1, 0)
        .unwrap_err()
        .contains("too far"));

    assert_eq!(core.player.walk_goal, None);
    assert_eq!((core.player.x, core.player.y), standing);
    assert_eq!((core.player.move_x, core.player.move_y), (0, 0));

    // Clicking your own feet cancels rather than searching, which is the useful reading of it and
    // the cheapest.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.walk_to(6, 0).unwrap();
    assert!(core.player.walk_goal.is_some());
    core.walk_to(1, 0).unwrap();
    assert_eq!(core.player.walk_goal, None);
    assert!(core.walk_path.is_empty());

    // The moment the player touches the movement keys they are driving. Both the key going down
    // and the key coming back up cancel, because both are the host saying the player is steering.
    for batch in [IDLE_MOVE_EAST, IDLE] {
        let mut core = game("new-game");
        set_player_hex(&mut core, 1, 0);
        core.walk_to(6, 0).unwrap();
        core.advance("[]", 0, 5).unwrap();
        assert!(
            core.player.walk_goal.is_some(),
            "{batch} should interrupt a walk in flight"
        );

        core.advance(batch, 0, 1).unwrap();
        assert_eq!(core.player.walk_goal, None);
        assert!(core.walk_path.is_empty());
    }

    // A wall raised across a live route is answered when it is raised, not when the player reaches
    // it — the drawn ribbon and the walk are the same path, so a stale one would be the host
    // promising a walk that cannot happen.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.walk_to(6, 0).unwrap();
    assert_eq!(
        core.walk_path.len(),
        5,
        "the clear route is the straight one"
    );

    core.advance("[]", 0, 3).unwrap();
    wall(&mut core, &[(3, -1), (3, 0), (3, 1)]);
    assert_eq!(core.player.walk_goal, Some(Coordinate { q: 6, r: 0 }));
    assert!(
        core.walk_path.len() > 5,
        "the route should have been rebuilt round the new wall"
    );

    // Now shut the destination off entirely. The goal stands until the next player step, which
    // is the one place a walk is allowed to end — and it says why.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.walk_to(4, 0).unwrap();
    wall(&mut core, &ring(4, 0));
    assert_eq!(core.player.walk_goal, Some(Coordinate { q: 4, r: 0 }));
    assert!(core.walk_path.is_empty());

    core.advance("[]", 0, 1).unwrap();
    assert_eq!(core.player.walk_goal, None);
    assert!(
        core.events.iter().any(|event| event.contains("blocked")),
        "a walk that cannot finish has to say so: {:?}",
        core.events
    );

    // Where the player is headed is state the run carries: it is hashed, it is saved, and it comes
    // back walking. The route is not saved — it is rebuilt against the world that loaded, which is
    // the only version of this that cannot come back describing a corridor that no longer exists.
    let (definitions, technologies, scenarios) = catalogs();
    let mut core = game("new-game");
    // Outside the hub's seven hexes, so the walk being saved is a walk rather than a collision.
    set_player_hex(&mut core, 2, 0);
    let idle = core.checksum();
    core.walk_to(6, 0).unwrap();
    assert_ne!(
        core.checksum(),
        idle,
        "a player walking somewhere is not the same run as one standing still"
    );

    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.player.walk_goal, Some(Coordinate { q: 6, r: 0 }));
    assert_eq!(restored.walk_path, core.walk_path);
    assert_eq!(restored.checksum(), core.checksum());

    // And it keeps going, which is the whole point of saving it.
    let mut resumed = restored;
    for _ in 0..40 {
        resumed.advance("[]", 0, 5).unwrap();
        if resumed.player.walk_goal.is_none() {
            break;
        }
    }
    assert_eq!(world_to_axial(resumed.player.x, resumed.player.y), (6, 0));

    // The search is simulation, so it answers the same way every time. Ties break on `(f, g, q, r)`
    // rather than on whatever order a heap happened to pop, which is what makes this true rather
    // than usually true.
    let batch = r#"[{"type":"walk_to","q":6,"r":0}]"#;
    let mut first = game("new-game");
    let mut second = game("new-game");
    for core in [&mut first, &mut second] {
        set_player_hex(core, 1, 0);
        wall(core, &[(3, -1), (3, 0), (3, 1)]);
        core.advance(batch, 0, 0).unwrap();
    }
    assert_eq!(first.walk_path, second.walk_path);
    for _ in 0..20 {
        first.advance("[]", 2, 5).unwrap();
        second.advance("[]", 2, 5).unwrap();
    }
    assert_eq!(first.checksum(), second.checksum());
    assert_eq!(
        (first.player.x, first.player.y),
        (second.player.x, second.player.y)
    );

    // Thinking about a route must not change the world. `terrain_at` is a pure function of the
    // parameters and the seed, and the search deliberately never calls `ensure_tile` — if it did,
    // considering a hex would survey it, and `generated_chunks` is a checksum input.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    let before = core.checksum();
    let chunks = core.generated_chunks.clone();

    let here = world_to_axial(core.player.x, core.player.y);
    for goal in [(6, 0), (0, 2), (9, 0), (1 + MAX_WALK_DISTANCE, 0)] {
        let _ = core.walk_route(here, goal);
    }
    assert_eq!(core.generated_chunks, chunks);
    assert_eq!(core.checksum(), before);
}

#[test]
fn gathering_is_bounded_by_reach_cooldown_and_what_the_hex_holds() {
    let mut core = game("new-game");
    set_player_hex(&mut core, 3, 0);
    let before = core.deposit_quantity((3, 0));
    for _ in 0..before {
        core.gather().unwrap();
        cooldown(&mut core);
    }
    assert_eq!(core.player.inventory.get(&1), Some(&before));
    assert_eq!(core.deposit_quantity((3, 0)), 0);
    assert!(core.gather().is_err());

    // A gather takes from the hex the player is standing on, wherever they stand inside it and
    // whichever way they face. The old target was pushed half a gather range along the facing and
    // then resolved to the nearest field cell, so stepping off-centre inside your own hex silently
    // moved the harvest to the neighbour ahead: the number under your feet stayed put while a
    // different hex counted down. Nothing on screen shows facing, so that was unattributable.
    for (facing_x, facing_y) in [(1000, 0), (-1000, 0), (500, 866), (-500, -866)] {
        for offset in [-880, -400, 0, 400, 880] {
            let mut core = game("new-game");
            set_player_hex(&mut core, 3, 0);
            // Field cells on both sides, so a target that drifts either way is visible.
            core.write_overlay(4, 0, 1, 20, 20);
            core.write_overlay(2, 0, 1, 20, 20);
            core.player.x += offset;
            core.player.facing_x = facing_x;
            core.player.facing_y = facing_y;
            core.gather().unwrap();
            cooldown(&mut core);
            assert_eq!(
                (
                    core.deposit_quantity((2, 0)),
                    core.deposit_quantity((3, 0)),
                    core.deposit_quantity((4, 0)),
                ),
                (20, 47, 20),
                "offset {offset} facing {facing_x},{facing_y} took from the wrong hex"
            );
        }
    }

    // Reach is exactly what an extractor on the same hex would cover, and it does not depend on
    // facing. Standing on the field takes from it; standing one step away still reaches it, which
    // is what lets a player work a field edge; two steps away is out of reach from every angle.
    for &(dq, dr) in &DIRECTIONS {
        for steps in 0..=2 {
            for facing in 0..6u8 {
                let mut core = game("new-game");
                let (x, y) = axial_world(3 + dq * steps, dr * steps);
                core.player.x = x;
                core.player.y = y;
                (core.player.facing_x, core.player.facing_y) = world_direction(facing);
                core.ensure_neighborhood(core.player.x, core.player.y);
                let reached = core.gather().is_ok();
                cooldown(&mut core);
                // One step out only reaches back if no nearer field cell outbids (3,0); the
                // rule is the shared candidate list, so ask it rather than restating it.
                let expected = core.resource_at_world(x, y) == Some((3, 0));
                assert_eq!(
                    reached && core.deposit_quantity((3, 0)) == 47,
                    expected,
                    "step {steps} along {dq},{dr} facing {facing}"
                );
                if steps == 2 {
                    assert_eq!(core.deposit_quantity((3, 0)), 48, "reach ran past one hex");
                }
            }
        }
    }

    // The cooldown between two gathers runs on the player's clock, not the factory's. It used to
    // be decremented once per simulation tick, so pausing froze it outright — one gather, then
    // "action cooling down" for as long as the factory stayed paused — and the harvest rate
    // otherwise rode the speed setting, six times faster at 60 tps than at 4.
    let mut core = game("new-game");
    set_player_hex(&mut core, 3, 0);
    core.gather().unwrap();
    assert!(core.gather().is_err(), "the cooldown has to hold at all");
    // The factory is paused for the whole of this: not one tick is advanced.
    let total = core.player.action_cooldown;
    assert!(total > 1, "iron ore is slower than a single step");
    core.advance_player_steps(total - 1);
    assert!(core.gather().is_err(), "cleared early");
    core.advance_player_steps(1);
    // The step that cleared the counter is the step the first swing landed on: one unit, paid
    // at the end of the work rather than at the start of it.
    assert_eq!(core.deposit_quantity((3, 0)), 47);
    core.gather().unwrap();
    cooldown(&mut core);
    assert_eq!(core.tick, 0);
    assert_eq!(core.deposit_quantity((3, 0)), 46);

    // And running the factory on its own no longer clears it.
    let mut core = game("new-game");
    set_player_hex(&mut core, 3, 0);
    core.gather().unwrap();
    core.tick_many(240);
    assert!(
        core.gather().is_err(),
        "factory time paid the player's debt"
    );

    // The first harvest of a session used to be free. The counter was a debt charged *after* an
    // instant take, so the button banked a unit the moment it went down and only then made the
    // player wait — the one gather in a run that cost nothing was the first one, and the ring drew
    // a wait for work that had already been paid out.
    //
    // It now measures the swing itself. Nothing moves until the work is spent, the deposit and the
    // pack change in the same step, and a swing the player walks out of reach of pays nothing —
    // harvesting is work over a hex, not a toll on the hex you were last standing beside.
    let (definitions, technologies, scenarios) = catalogs();
    let mut core = game("new-game");
    set_player_hex(&mut core, 3, 0);
    let initial = core.deposit_quantity((3, 0));

    core.gather().unwrap();
    let work = core.player.action_cooldown;
    assert!(work > 1, "iron ore is more than a single step of work");
    assert_eq!(
        (
            core.player.inventory.get(&IRON_ORE),
            core.deposit_quantity((3, 0))
        ),
        (None, initial),
        "the press alone moved something"
    );
    core.advance_player_steps(work - 1);
    assert_eq!(
        (
            core.player.inventory.get(&IRON_ORE),
            core.deposit_quantity((3, 0))
        ),
        (None, initial),
        "paid before the work was finished"
    );
    core.advance_player_steps(1);
    assert_eq!(
        (
            core.player.inventory.get(&IRON_ORE).copied(),
            core.deposit_quantity((3, 0))
        ),
        (Some(1), initial - 1),
        "the deposit and the pack move together, at the end"
    );

    // A swing carries across a save, because the counter that is running is saved and what it
    // is working on has to be saved with it.
    core.gather().unwrap();
    core.advance_player_steps(work / 2);
    let save = core.save_string().unwrap();
    let mut resumed = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    resumed.advance_player_steps(work);
    core.advance_player_steps(work);
    assert_eq!(
        (
            resumed.player.inventory.get(&IRON_ORE).copied(),
            resumed.deposit_quantity((3, 0))
        ),
        (Some(2), initial - 2),
        "a resumed swing has to land"
    );
    assert_eq!(
        resumed.checksum(),
        core.checksum(),
        "the resumed swing and the uninterrupted one are the same run"
    );

    // Walking out of reach cancels it. Reach is the same predicate the start asked, so a swing
    // can never land on a cell an extractor standing here could not work.
    core.gather().unwrap();
    core.advance_player_steps(work / 2);
    set_player_hex(&mut core, 9, 0);
    core.advance_player_steps(work);
    assert_eq!(
        (
            core.player.inventory.get(&IRON_ORE).copied(),
            core.deposit_quantity((3, 0))
        ),
        (Some(2), initial - 2),
        "a harvest the player walked away from still paid"
    );
}

#[test]
fn placement_and_drag_build_exactly_what_the_rules_allow_and_undo_takes_it_back() {
    let mut core = legacy_band_game("new-game");
    core.player.inventory.insert(1, 100);
    core.player.inventory.insert(3, 100);
    core.player.inventory.insert(24, 100);
    assert!(core.place(2, 0, 2, 0, None).unwrap_err().contains("locked"));
    core.researched.extend([1, 2, 3, 4]);
    assert!(core
        .place(2, 1, 2, 0, None)
        .unwrap_err()
        .contains("environment"));
    assert!(core
        .place(20, 20, 2, 0, None)
        .unwrap_err()
        .contains("range"));
    core.player.inventory.clear();
    assert!(core
        .place(2, 0, 2, 0, None)
        .unwrap_err()
        .contains("Transport kit"));
    core.player.inventory.insert(11, 8);
    core.player.inventory.insert(19, 7);
    // Extractor wants plate, a gear and timber; this hand is holding the first two. Naming the
    // missing item is the message; "construction cost is not available" did not say which.
    assert!(core.place(3, 0, 1, 0, None).unwrap_err().contains("Timber"));
    core.player.inventory.clear();
    core.player.inventory.insert(24, 3);
    core.place(2, 0, 2, 0, None).unwrap();
    assert!(core
        .place(2, 0, 2, 0, None)
        .unwrap_err()
        .contains("occupied"));
    // Occupied foundation plus the reserved growth hex both have to be constructible, so the
    // empty-deposit case is asked of a pad that is inland of the water that refused (2, 1).
    let deposit = core.place(4, -3, 1, 0, None).unwrap_err();
    assert!(deposit.contains("deposit"), "{deposit}");
    set_player_hex(&mut core, 100, 100);
    core.player.inventory.insert(24, 2);
    let checksum_before_preview = core.checksum();
    assert!(core.placement_legality(101, 100, 2, 0, None, true).is_ok());
    assert_eq!(core.checksum(), checksum_before_preview);
    assert!(core
        .placement_legality(100, 100, 2, 0, None, true)
        .unwrap_err()
        .contains("player"));

    // The six corner vectors are one rotational family, not six hand-written special cases.
    let corners = &TRANSPORT_DIRECTIONS[usize::from(NORTH)..];
    for index in 0..corners.len() {
        let (q, r) = corners[index];
        assert_eq!(corners[(index + 1) % corners.len()], (-r, q + r));
    }
    // The six edges keep their indices, which is what makes every saved orientation, every
    // fixture, and every existing drag mean the same thing after the table grew.
    assert_eq!(TRANSPORT_DIRECTIONS[..DIRECTIONS.len()], DIRECTIONS);
    // Adjacency stays six. A boiler must never reach two rows.
    assert_eq!(DIRECTIONS.len(), 6);

    // Every corner heading resolves symmetrically, and no target in a wide lattice window gives
    // two headings the same full two-row close. The resolver still carries an explicit tie-break.
    use OrientationAxis::{Corner, Edge};
    for &(dq, dr) in &TRANSPORT_DIRECTIONS[usize::from(NORTH)..] {
        assert_eq!(
            line_between((0, 0), (dq * 3, dr * 3), Corner),
            vec![(0, 0), (dq, dr), (dq * 2, dr * 2), (dq * 3, dr * 3)]
        );
    }
    for q in -64..=64 {
        for r in -64..=64 {
            let remaining = axial_distance((0, 0), (q, r));
            let candidates = TRANSPORT_DIRECTIONS[usize::from(NORTH)..]
                .iter()
                .filter(|&&(dq, dr)| axial_distance((dq, dr), (q, r)) == remaining - 2)
                .count();
            assert!(candidates <= 1, "corner drag tie at {q},{r}");
        }
    }
    // Bounded like every other drag.
    assert_eq!(
        line_between((0, 0), (900, -1800), Corner).len(),
        MAX_LINE_CELLS
    );
    // And the property that keeps every existing test meaningful: the edge axis is the old
    // resolver, untouched.
    for &to in &[(3, 0), (4, 1), (5, 3), (0, -6), (-3, 2)] {
        assert_eq!(line_between((0, 0), to, Edge), hex_line((0, 0), to));
    }

    // A drag resolves one turn and stays bounded.
    // A straight run along a hex axis.
    assert_eq!(
        hex_line((0, 0), (3, 0)),
        vec![(0, 0), (1, 0), (2, 0), (3, 0)]
    );
    // An off-axis run turns exactly once rather than staircasing, so a belt line between two
    // endpoints carries the fewest direction changes it can.
    assert_eq!(
        hex_line((2, 0), (4, 1)),
        vec![(2, 0), (3, 0), (4, 0), (4, 1)]
    );
    let turns = hex_line((0, 0), (5, 3))
        .windows(2)
        .filter_map(|pair| step_direction(pair[0], pair[1]))
        .collect::<Vec<_>>()
        .windows(2)
        .filter(|step| step[0] != step[1])
        .count();
    assert_eq!(turns, 1);
    // Both endpoints are always included, and a single-cell drag is a single placement.
    assert_eq!(hex_line((-3, 2), (-3, 2)), vec![(-3, 2)]);
    // One command can only ever expand into a bounded run.
    assert_eq!(hex_line((0, 0), (900, 0)).len(), MAX_LINE_CELLS);
    assert_eq!(step_direction((0, 0), (0, 1)), Some(1));
    assert_eq!(step_direction((0, 0), (4, 4)), None);

    // One drag builds exactly what the equivalent placements build.
    // The path and per-cell headings `a_drag_resolves_one_turn_and_stays_bounded` pins, written
    // out so this test does not re-derive them from the code it is checking.
    let equivalent = [((2, 0), 0u8), ((3, 0), 0), ((4, 0), 1), ((4, 1), 1)];

    let mut dragged = game("new-game");
    dragged.researched.extend([1, 2, 3, 4]);
    dragged.player.inventory.insert(24, 100);
    dragged.place_line((2, 0), (4, 1), 2, 0, None).unwrap();

    let mut individual = game("new-game");
    individual.researched.extend([1, 2, 3, 4]);
    individual.player.inventory.insert(24, 100);
    for ((q, r), orientation) in equivalent {
        individual.place(q, r, 2, orientation, None).unwrap();
    }

    // Same world, same blueprint, same materials spent: a drag is exactly its placements.
    assert_eq!(dragged.checksum(), individual.checksum());
    assert_eq!(dragged.entities.len(), individual.entities.len());
    // The drag routed the run itself — every belt points at its successor and the last one
    // keeps the run's heading — so the player never oriented a segment by hand.
    let headings: Vec<u8> = dragged
        .entities
        .iter()
        .filter(|entity| !entity.placed.scenario_owned)
        .map(|entity| entity.placed.orientation)
        .collect();
    assert_eq!(headings, vec![0, 0, 1, 1]);
    // One drag reports one result, not one per cell.
    assert_eq!(dragged.events.last().unwrap(), "Placed 4 × Belt");

    // A drag builds what it legally can and reports why it stopped.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 4]);
    // Enough for two of the four cells the drag covers.
    core.player.inventory.insert(24, 2);
    core.place_line((2, 0), (4, 1), 2, 0, None).unwrap();
    assert_eq!(
        core.entities
            .iter()
            .filter(|entity| !entity.placed.scenario_owned)
            .count(),
        2
    );
    assert_eq!(core.player.inventory.get(&24).copied().unwrap_or(0), 0);
    // Running out of materials part-way is reported, and what was affordable still stands.
    assert!(core
        .events
        .iter()
        .any(|event| event.contains("Transport kit")));

    // A drag that can place nothing at all fails as the single placement would have.
    let mut empty = game("new-game");
    empty.researched.extend([1, 2, 3, 4]);
    assert!(empty
        .place_line((2, 0), (4, 1), 2, 0, None)
        .unwrap_err()
        .contains("Transport kit"));
    assert!(empty
        .entities
        .iter()
        .all(|entity| entity.placed.scenario_owned));

    // A drag preview is what the drag builds.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 4]);
    // Materials for two of the four cells, so the preview has to show the run stopping.
    core.player.inventory.insert(24, 2);

    let preview = core.line_preview((2, 0), (4, 1), 2, 0, None);
    assert_eq!(preview.len(), 4);
    let promised: Vec<(i32, i32, u8)> = preview
        .iter()
        .filter(|cell| cell.legal)
        .map(|cell| (cell.q, cell.r, cell.orientation))
        .collect();
    assert_eq!(promised.len(), 2);
    // The preview spends materials as it walks, so it marks the exact cell the run stops at
    // rather than implying the whole line is affordable.
    assert!(!preview[2].legal && !preview[3].legal);

    core.place_line((2, 0), (4, 1), 2, 0, None).unwrap();
    let built: Vec<(i32, i32, u8)> = core
        .entities
        .iter()
        .filter(|entity| !entity.placed.scenario_owned)
        .map(|entity| (entity.placed.q, entity.placed.r, entity.placed.orientation))
        .collect();
    assert_eq!(built, promised);

    // Removal previews the same way: only cells actually holding something removable.
    let erasable = core.erase_line_preview((2, 0), (4, 1));
    assert_eq!(
        erasable
            .iter()
            .filter(|cell| cell.legal)
            .map(|cell| (cell.q, cell.r))
            .collect::<Vec<_>>(),
        vec![(2, 0), (3, 0)]
    );

    // A belt drag routes around an occupied hex.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 4]);
    stock_for(&mut core, 4, 1);
    core.player.inventory.insert(24, 100);
    core.place(3, 0, 4, 0, None).unwrap();

    let preview = core.line_preview((2, 0), (4, 0), 2, 0, None);
    assert_eq!(preview.first().map(|cell| (cell.q, cell.r)), Some((2, 0)));
    assert_eq!(preview.last().map(|cell| (cell.q, cell.r)), Some((4, 0)));
    assert!(preview.iter().all(|cell| cell.legal));
    assert!(preview.iter().all(|cell| (cell.q, cell.r) != (3, 0)));
    assert!(preview.len() > 3, "the obstacle requires a shortest detour");

    let promised: Vec<(i32, i32, u8)> = preview
        .iter()
        .map(|cell| (cell.q, cell.r, cell.orientation))
        .collect();
    core.place_line((2, 0), (4, 0), 2, 0, None).unwrap();
    let built: Vec<(i32, i32, u8)> = core
        .entities
        .iter()
        .filter(|entity| entity.kind == BuildingKind::Belt)
        .map(|entity| (entity.placed.q, entity.placed.r, entity.placed.orientation))
        .collect();
    assert_eq!(built, promised);

    let mut creative = game("new-game");
    creative.creative = true;
    creative.researched.extend([1, 2, 3, 4]);
    creative.place(3, 0, 4, 0, None).unwrap();
    assert!(creative
        .line_preview((2, 0), (4, 0), 2, 0, None)
        .iter()
        .all(|cell| cell.legal));

    // One drag removes the run it covers.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 4]);
    core.player.inventory.insert(24, 100);
    core.place_line((2, 0), (4, 1), 2, 0, None).unwrap();
    let spent = *core.player.inventory.get(&24).unwrap();
    core.erase_line((2, 0), (4, 1)).unwrap();
    assert!(core
        .entities
        .iter()
        .all(|entity| entity.placed.scenario_owned));
    // Removal refunds through the ordinary erase path, so a built-then-removed run is free.
    assert_eq!(core.player.inventory.get(&24), Some(&(spent + 4)));
    assert_eq!(core.events.last().unwrap(), "Recovered 4 buildings");
    // A drag across empty ground reports the same refusal a single erase would.
    assert!(core
        .erase_line((2, 0), (4, 1))
        .unwrap_err()
        .contains("no building"));

    // Undo takes back the last construction through the erase path.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 4]);
    core.player.inventory.insert(24, 100);
    // Building opens the world around what you build, and that opening is not a construction:
    // no undo takes back ground you have already seen. So survey the far end of the drag
    // before the baseline is taken, or this test would be measuring the survey rather than
    // the undo. Under the old chunk-ring survey this happened to be unnecessary — `(4, 1)`
    // shares a chunk with `(2, 0)` — which made the omission invisible rather than correct.
    let (far_x, far_y) = axial_world(4, 1);
    core.ensure_neighborhood(far_x, far_y);
    let before = core.checksum();

    core.place(2, 0, 2, 0, None).unwrap();
    core.undo().unwrap();
    // Undo is exactly an erase of what was just built, so the world returns to where it was.
    assert_eq!(core.checksum(), before);
    assert_eq!(core.events.last().unwrap(), "Undid the last construction");

    // It unwinds a drag one construction at a time, most recent first.
    core.place_line((2, 0), (4, 1), 2, 0, None).unwrap();
    for _ in 0..4 {
        core.undo().unwrap();
    }
    assert_eq!(core.checksum(), before);
    assert!(core.undo().unwrap_err().contains("nothing to undo"));

    // A construction already removed by hand is skipped rather than undoing something else.
    core.place(2, 0, 2, 0, None).unwrap();
    core.place(3, 0, 2, 0, None).unwrap();
    core.erase(3, 0).unwrap();
    core.undo().unwrap();
    assert!(core
        .entities
        .iter()
        .all(|entity| entity.placed.scenario_owned));

    // Undo history is session state: a save carries none of it, so a restored game has nothing
    // to take back and cannot erase across a load boundary.
    core.place(2, 0, 2, 0, None).unwrap();
    let (definitions, technologies, scenarios) = catalogs();
    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert!(restored.undo_stack.is_empty());
    assert_eq!(restored.checksum(), core.checksum());
}

/// Creative is one switch with three consequences: everything is known, nothing is charged, and
/// nothing is handed back. Each is checked against the ordinary path rather than a creative-only
/// one, because the whole value of a creative test bed is that it builds the same factory.
#[test]
fn creative_unlocks_grants_resizes_and_survives_a_save() {
    let mut core = legacy_band_game("new-game");
    // A locked building with an empty pack: refused for both reasons before the switch.
    let locked = core.place(2, 0, 2, 0, None).unwrap_err();
    assert!(locked.contains("locked by research"));
    core.set_creative(true);

    let every_technology: BTreeSet<TechnologyId> = core
        .technologies
        .technologies
        .iter()
        .map(|technology| technology.id)
        .collect();
    assert_eq!(core.researched, every_technology);

    assert!(core.player.inventory.is_empty());
    core.place(2, 0, 2, 0, None).unwrap();
    assert!(
        core.player.inventory.is_empty(),
        "creative construction must not reach into the pack"
    );

    // And recovers no construction cost, so a full pack can never refuse an erase. The belt's
    // in-transit cargo still spills into the world instead of being destroyed.
    let index = core.entity_at(2, 0).unwrap();
    core.entities[index].cargo = Some(Cargo {
        item_id: 3,
        quantity: 1,
    });
    core.grant(1, core.player_room_for(1)).unwrap();
    assert_eq!(
        core.slots_used(&core.player.inventory),
        core.player.carry_slots
    );
    let full = core.player.inventory.clone();
    core.erase(2, 0).unwrap();
    assert_eq!(core.player.inventory, full);
    assert_eq!(core.ground_items[0].item_id, 3);
    assert_eq!(core.ground_items[0].quantity, 1);

    // Placement's other rules are untouched: creative is free, not lawless.
    assert!(core
        .place(2, 1, 2, 0, None)
        .unwrap_err()
        .contains("environment"));

    // Leaving creative restores the prices and the refunds. What the settlement learned stays
    // learned, because a technology is knowledge rather than a purchase.
    let mut core = game("new-game");
    core.set_creative(true);
    core.set_creative(false);
    assert_eq!(core.researched.len(), core.technologies.technologies.len());
    assert!(core.place(2, 0, 2, 0, None).unwrap_err().contains("need"));
    assert!(core.grant(1, 1).unwrap_err().contains("creative"));
    assert!(core.discard(Some(1), 1).unwrap_err().contains("creative"));
    assert!(core.set_carry_slots(40).unwrap_err().contains("creative"));

    // Granting is a route into the pack like any other, so it obeys the one carrying rule: what
    // fits arrives, what does not is not invented, and an empty grant says so rather than lying.
    let mut core = game("new-game");
    core.set_creative(true);
    let stack = core.stack_size(1);
    let slots = core.player.carry_slots;

    core.grant(1, 5).unwrap();
    assert_eq!(core.player.inventory.get(&1), Some(&5));
    // Asking for far more than the pack holds tops it up to exactly full rather than refusing.
    core.grant(1, u32::MAX).unwrap();
    assert_eq!(core.player.inventory.get(&1), Some(&(stack * slots)));
    assert_eq!(core.slots_used(&core.player.inventory), slots);
    assert!(core.grant(1, 1).unwrap_err().contains("no room"));
    assert!(core.grant(9_999, 1).unwrap_err().contains("unknown item"));

    // Zero means the whole stack; a named quantity takes that much and no more.
    core.discard(Some(1), 3).unwrap();
    assert_eq!(core.player.inventory.get(&1), Some(&(stack * slots - 3)));
    // A part-emptied stack still occupies its slot, so nothing else fits until a whole one goes.
    assert!(core.grant(3, 1).unwrap_err().contains("no room"));
    core.discard(Some(1), stack).unwrap();
    core.grant(3, 4).unwrap();
    core.discard(Some(1), 0).unwrap();
    assert_eq!(core.player.inventory.get(&1), None);
    assert_eq!(core.player.inventory.get(&3), Some(&4));
    // Clearing the pack is one command, not one per stack against a batch that holds eight.
    core.discard(None, 0).unwrap();
    assert!(core.player.inventory.is_empty());
    assert!(core.discard(None, 0).unwrap_err().contains("nothing"));

    // The pack may be widened, within bounds, and never so far down that carried stock is stranded.
    let mut core = game("new-game");
    let scenario_slots = core.player.carry_slots;
    core.set_creative(true);
    let earned_slots = core.player.carry_slots;
    assert!(earned_slots > scenario_slots);

    core.set_carry_slots(MAX_CARRY_SLOTS).unwrap();
    assert_eq!(core.player.carry_slots, MAX_CARRY_SLOTS);
    assert!(core
        .set_carry_slots(MAX_CARRY_SLOTS + 1)
        .unwrap_err()
        .contains("out of range"));
    assert!(core
        .set_carry_slots(earned_slots - 1)
        .unwrap_err()
        .contains("out of range"));

    // Narrowing under what is already carried is refused rather than dropping the difference.
    // One item per slot, one more than the researched pack holds.
    for item_id in 1..=(earned_slots as ItemId + 1) {
        core.grant(item_id, 1).unwrap();
    }
    assert!(core
        .set_carry_slots(earned_slots)
        .unwrap_err()
        .contains("too much carried"));
    core.discard(None, 0).unwrap();
    core.set_carry_slots(earned_slots).unwrap();

    // Both halves of creative are run state now, so both survive a save and both are hashed. A file
    // with either edited out no longer describes the run it came from.
    let (definitions, technologies, scenarios) = catalogs();
    let mut core = game("new-game");
    core.set_creative(true);
    core.set_carry_slots(64).unwrap();
    core.place(2, 0, 2, 0, None).unwrap();

    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert!(restored.creative);
    assert_eq!(restored.player.carry_slots, 64);
    assert_eq!(restored.checksum(), core.checksum());

    // Neither is a free field: the checksum is what makes them run state rather than a note.
    let priced = save.replace("\"creative\":true", "\"creative\":false");
    assert!(
        Core::from_save(&definitions, &technologies, &scenarios, &priced)
            .err()
            .unwrap()
            .contains("checksum")
    );
    let narrowed = save.replace("\"carry_slots\":64", "\"carry_slots\":63");
    assert!(
        Core::from_save(&definitions, &technologies, &scenarios, &narrowed)
            .err()
            .unwrap()
            .contains("checksum")
    );
    // And the range check still refuses a pack outside what any run may have.
    let absurd = save.replace("\"carry_slots\":64", "\"carry_slots\":9999");
    assert!(
        Core::from_save(&definitions, &technologies, &scenarios, &absurd)
            .err()
            .unwrap()
            .contains("invalid player or research state")
    );
}

#[test]
fn erasing_refunds_spills_and_never_leaves_an_uncompilable_graph() {
    let mut core = game("new-game");
    core.researched.insert(1);
    core.player.inventory.insert(24, 2);
    core.place(2, 0, 2, 0, None).unwrap();
    let index = core
        .entities
        .iter()
        .position(|entity| entity.placed.q == 2)
        .unwrap();
    core.entities[index].cargo = Some(Cargo {
        item_id: 3,
        quantity: 1,
    });
    core.erase(2, 0).unwrap();
    assert_eq!(core.player.inventory.get(&24), Some(&2));
    assert_eq!(core.player.inventory.get(&3), None);
    assert_eq!(
        core.ground_items,
        vec![GroundItem {
            id: 1,
            q: 2,
            r: 0,
            item_id: 3,
            quantity: 1,
            despawn_tick: GROUND_ITEM_LIFETIME_TICKS,
        }]
    );
    assert!(core.erase(0, 0).unwrap_err().contains("protected"));

    // A belt may not be built into something that can never take an item, and no such edge is
    // compiled if one arises anyway.
    //
    // The old game answered this at delivery time, which meant it never answered it at all: the
    // line looked connected, compiled an edge, and quietly backed up. The static question gets its
    // own predicate so the answer cannot change with a recipe or a contract the way `accepts_item`
    // can, construction refuses by name and by hex, and only transport is held to its heading —
    // a machine that happens to face a pole is still a perfectly good machine.
    let mut core = game("new-game");
    core.set_creative(true);
    set_player_hex(&mut core, 1, 3);
    core.place(0, 3, 12, 0, None).unwrap();

    // Heading 4 is due north on the routing table, so from (0, 4) it points straight at the
    // pole. Pointed anywhere else the same belt on the same hex is fine.
    assert!(core.placement_legality(0, 4, 2, 0, None, false).is_ok());
    let refused = core
        .placement_legality(0, 4, 2, 4, None, false)
        .unwrap_err();
    assert!(refused.contains("Pole"), "names the building: {refused}");
    assert!(refused.contains("0, 3"), "names the hex: {refused}");
    assert!(
        refused.contains("never takes items"),
        "names the reason: {refused}"
    );
    let preview = core.line_preview((0, 5), (0, 4), 2, 4, None);
    let tip = preview
        .iter()
        .find(|cell| cell.q == 0 && cell.r == 4)
        .unwrap();
    assert!(!tip.legal);
    assert!(tip.reason.as_ref().unwrap().contains("Pole at 0, 3"));
    assert!(
        core.placement_legality(0, 4, 4, 4, None, false).is_ok(),
        "a container facing the same pole is not transport and is not refused"
    );

    // Nothing about the runtime question moved: a pole was never a delivery target and still
    // is not, asked the way the tick asks it.
    let pole = core.entity_at(0, 3).unwrap();
    assert!(!core.accepts_item(pole, 3));

    // And an edge into such a target is not compiled even when one arises anyway — building
    // the pole second, where no placement rule could have refused it.
    let mut core = empty_world("new-game");
    add_test_belt(&mut core, 0, 0, 0);
    add_test_entity(&mut core, 1, 0, 12, 0);
    core.compile_graph();
    assert!(
        core.graph[0].is_empty(),
        "the belt shows no downstream rather than a connection that never delivers"
    );

    // Demolishing a building with something in it no longer stops at a full pack.
    //
    // What fits comes back, what does not falls at the site on the ordinary ground-item clock, and
    // the two together are exactly what the building held — that split is the conservation law.
    // Refusing instead was the worse trade: a full pack and a full building had no order of
    // operations that emptied either, so the building the player wanted gone simply stayed. The
    // host warns first and says the ground items are on a timer, so the loss is a decision.
    let mut core = game("new-game");
    core.researched.extend([1, 4, 12]);
    set_player_hex(&mut core, 1, 3);
    core.player.inventory.insert(16, 3);
    core.place(0, 3, 4, 0, None).unwrap();

    // Three stacks inside, and room in the pack for exactly one of them.
    let stack = core.stack_size(3);
    let index = core.entity_at(0, 3).unwrap();
    core.entities[index].inventory.insert(3, stack * 3);
    core.player.inventory.clear();
    core.player.carry_slots = 1;

    core.erase(0, 3)
        .expect("a full pack no longer blocks a demolition");
    assert_eq!(core.player.inventory.get(&3), Some(&stack));
    assert_eq!(
        core.player.inventory.get(&16),
        None,
        "the construction cost had no slot left to come back into"
    );
    assert_eq!(
        core.ground_items
            .iter()
            .map(|item| (item.item_id, item.quantity, item.despawn_tick))
            .collect::<Vec<_>>(),
        vec![
            (3, stack * 2, GROUND_ITEM_LIFETIME_TICKS),
            (16, 3, GROUND_ITEM_LIFETIME_TICKS),
        ],
        "the remainder falls at the site, on the clock the confirmation states"
    );
    assert!(
        core.events
            .iter()
            .any(|event| event.contains("would not fit your pack")),
        "and the player is told, not left to notice"
    );

    // Temporarily blocked targets still compile and allow belts.
    for definition_id in [3, 4] {
        let mut core = game("new-game");
        core.set_creative(true);
        // Clear of the composer's three hexes, which reach south and east of their anchor.
        set_player_hex(&mut core, 2, 2);
        core.place(0, 3, definition_id, 0, (definition_id == 3).then_some(1))
            .unwrap();
        let target = core.entity_at(0, 3).unwrap();
        if definition_id == 4 {
            core.entities[target].inventory.insert(3, 60);
        } else {
            core.entities[target].placed.recipe_id = None;
        }
        // The belt comes in from the north-west, on ground neither footprint stands on.
        assert!(core.placement_legality(0, 2, 2, 1, None, false).is_ok());
        core.place(0, 2, 2, 1, None).unwrap();
        let belt = core.entity_at(0, 2).unwrap();
        assert_eq!(core.graph[belt].primary(), Some(target));
    }

    // Demolition overflow round trips and can be collected.
    let (definitions, technologies, scenarios) = catalogs();
    let mut core = game("new-game");
    core.set_creative(true);
    core.place(2, 0, 4, 0, None).unwrap();
    let container = core.entity_at(2, 0).unwrap();
    core.entities[container].inventory.insert(3, 60);
    core.player.inventory.clear();
    core.player
        .inventory
        .insert(1, core.player.carry_slots * core.stack_size(1));
    core.set_creative(false);
    core.erase(2, 0).unwrap();
    let save = core.save_string().unwrap();
    let mut restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.checksum(), core.checksum());
    assert_eq!(restored.ground_items, core.ground_items);
    restored.player.inventory.clear();
    set_player_hex(&mut restored, 2, 0);
    restored.tick += 30;
    restored.player.move_x = 1;
    restored.collect_ground_items();
    assert!(restored.ground_items.is_empty());
    assert_eq!(restored.player.inventory.get(&3), Some(&60));
    assert_eq!(restored.player.inventory.get(&16), Some(&3));
}

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

#[test]
fn footprints_occupy_turn_reserve_and_upgrade_as_one_building() {
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3]);
    stock_for(&mut core, 3, 1);
    core.place(-3, 1, 3, 0, Some(1)).unwrap();
    let composer = core
        .snapshot()
        .buildings
        .into_iter()
        .find(|entity| entity.definition_id == 3)
        .unwrap();
    assert_eq!(
        composer.footprint,
        vec![
            Coordinate { q: -3, r: 1 },
            Coordinate { q: -2, r: 1 },
            Coordinate { q: -3, r: 2 }
        ]
    );
    assert!(core
        .place(-2, 1, 2, 0, None)
        .unwrap_err()
        .contains("footprint"));
    core.erase(-3, 2).unwrap();
    assert!(core.entity_at(-3, 1).is_none());

    // A one-hex build reach still reaches a two-cell machine from the far lobe, even when the
    // command names the anchor. Reach is the Minkowski sum of the footprint with the range disc,
    // not a disc around one of its tiles.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3]);
    stock_for(&mut core, 3, 1);
    core.place(-3, 1, 3, 0, Some(1)).unwrap();
    // One hex of world-unit reach: beside the far cell, out of range of the anchor alone.
    core.player.build_range = HEX_X as u32;
    set_player_hex(&mut core, -3, 3);
    assert!(core.entity_at(-3, 1).is_some());
    core.erase(-3, 1).unwrap();
    assert!(core.entity_at(-3, 1).is_none());
    assert!(core.entity_at(-3, 2).is_none());
}

/// Every cell within `radius` steps of the anchor, as definition-relative offsets.
fn disc_offsets(radius: i32) -> Vec<(i32, i32)> {
    let mut cells = Vec::new();
    for q in -radius..=radius {
        for r in -radius..=radius {
            if axial_distance((0, 0), (q, r)) <= radius {
                cells.push((q, r));
            }
        }
    }
    cells
}

/// Give a live definition a footprint the shipped catalogue does not contain.
///
/// Phase 8 reauthors thirty buildings into multi-cell plants; the machinery that has to hold
/// them is being proved here first, against shapes no `definitions.json` carries yet. Editing
/// the catalogue in the Core is what `a_footprint_needs_ground_no_steeper_than_a_walk_can_climb`
/// already does, and it keeps the shipped file the subject of its own tests.
fn set_test_footprint(core: &mut Core, definition_id: DefinitionId, cells: &[(i32, i32)]) {
    core.definitions
        .buildings
        .iter_mut()
        .find(|building| building.id == definition_id)
        .expect("a building to reshape")
        .footprint = cells.iter().map(|&(q, r)| Coordinate { q, r }).collect();
}

fn set_test_envelope(core: &mut Core, definition_id: DefinitionId, cells: &[(i32, i32)]) {
    core.definitions
        .buildings
        .iter_mut()
        .find(|building| building.id == definition_id)
        .expect("a building to reshape")
        .service_envelope = cells.iter().map(|&(q, r)| Coordinate { q, r }).collect();
}

fn set_test_clearance(core: &mut Core, definition_id: DefinitionId, cells: &[(i32, i32)]) {
    core.definitions
        .buildings
        .iter_mut()
        .find(|building| building.id == definition_id)
        .expect("a building to reshape")
        .overhead_clearance = cells.iter().map(|&(q, r)| Coordinate { q, r }).collect();
}

fn set_test_foundation(core: &mut Core, definition_id: DefinitionId, class: FoundationClass) {
    core.definitions
        .buildings
        .iter_mut()
        .find(|building| building.id == definition_id)
        .expect("a building to reshape")
        .foundation_class = class;

    // The two-ring hexagon is the largest shape a definition may claim, and standing one is not a
    // special case: all nineteen cells enter the occupancy index, the snapshot publishes all
    // nineteen, and an erase aimed at the rim takes the whole building.
    let mut core = ground_world();
    core.researched.extend([1, 2, 3]);
    stock_for(&mut core, 3, 1);
    core.player.build_range = 1 << 20;
    set_player_hex(&mut core, 0, 6);
    let cells = disc_offsets(2);
    assert_eq!(cells.len(), MAX_FOOTPRINT_CELLS);
    set_test_footprint(&mut core, 3, &cells);

    core.place(-4, 0, 3, 0, Some(1)).unwrap();
    let index = core.entity_at(-4, 0).expect("the plant stands");
    for &(q, r) in &cells {
        assert_eq!(
            core.entity_at(-4 + q, r),
            Some(index),
            "cell ({q}, {r}) belongs to the plant"
        );
    }
    let published = core
        .snapshot()
        .buildings
        .into_iter()
        .find(|entity| entity.definition_id == 3)
        .expect("the plant is published");
    assert_eq!(published.footprint.len(), MAX_FOOTPRINT_CELLS);

    // The rim, not the anchor: an erase names a hex the player can see, and every hex the
    // building covers is that building.
    core.erase(-6, 0).unwrap();
    assert!(cells
        .iter()
        .all(|&(q, r)| core.entity_at(-4 + q, r).is_none()));

    // A multi-cell footprint turns with its heading. Rotation by whole sixths is the only turn
    // this lattice has, which is why the validator keeps the twelve-heading transport axis
    // single-cell: a thirty-degree turn is not a symmetry of the grid and could not land a second
    // cell on a hex at all.
    let mut core = ground_world();
    core.researched.extend([1, 2, 3]);
    core.player.build_range = 1 << 20;
    set_player_hex(&mut core, 0, 6);
    let offsets = [(0, 0), (1, 0), (2, 0), (0, 1)];
    set_test_footprint(&mut core, 3, &offsets);

    let mut shapes: BTreeSet<Vec<(i32, i32)>> = BTreeSet::new();
    for orientation in 0..6u8 {
        stock_for(&mut core, 3, 1);
        core.place(-4, 0, 3, orientation, Some(1)).unwrap();
        let index = core.entity_at(-4, 0).expect("the plant stands");
        let standing: Vec<(i32, i32)> = core
            .entity_footprint(&core.entities[index])
            .into_iter()
            .map(|cell| (cell.q, cell.r))
            .collect();
        let expected: Vec<(i32, i32)> = offsets
            .iter()
            .map(|&(q, r)| {
                let turned = rotate_coordinate(Coordinate { q, r }, orientation);
                (-4 + turned.q, turned.r)
            })
            .collect();
        assert_eq!(standing, expected, "heading {orientation}");
        let mut sorted = standing;
        sorted.sort();
        shapes.insert(sorted);
        core.erase(-4, 0).unwrap();
    }
    assert_eq!(shapes.len(), 6, "six headings, six distinct shapes");

    // The ceiling and the contiguity rule are properties of the catalogue, so they are checked
    // where a definition file is read rather than where a building is placed.
    let shaped = |cells: Vec<(i32, i32)>| {
        let mut definitions: DefinitionsInput = serde_json::from_str(DEFINITIONS).unwrap();
        definitions
            .buildings
            .iter_mut()
            .find(|building| building.id == 3)
            .expect("the composer")
            .footprint = cells
            .into_iter()
            .map(|(q, r)| Coordinate { q, r })
            .collect();
        definitions
    };

    assert_eq!(validate_definitions(&shaped(disc_offsets(2))), Ok(()));

    let mut oversized = disc_offsets(2);
    // Contiguous, and one cell past the ceiling: the shape is legal and only the size is not.
    oversized.push((3, 0));
    assert!(validate_definitions(&shaped(oversized))
        .unwrap_err()
        .contains("invalid footprint"));

    assert!(validate_definitions(&shaped(vec![(0, 0), (3, 0)]))
        .unwrap_err()
        .contains("disconnected pieces"));

    // A definition may not reserve a cell it occupies or disconnect.
    let mutate = |edit: fn(&mut BuildingDefinition)| {
        let mut definitions: DefinitionsInput = serde_json::from_str(DEFINITIONS).unwrap();
        let building = definitions
            .buildings
            .iter_mut()
            .find(|building| building.id == 4)
            .expect("the container");
        edit(building);
        definitions
    };

    assert_eq!(
        validate_definitions(&mutate(|building| {
            building.service_envelope = vec![Coordinate { q: 1, r: 0 }];
        })),
        Ok(())
    );
    assert!(validate_definitions(&mutate(|building| {
        building.service_envelope = vec![Coordinate { q: 0, r: 0 }];
    }))
    .unwrap_err()
    .contains("already occupies"));
    assert!(validate_definitions(&mutate(|building| {
        building.service_envelope = vec![Coordinate { q: 3, r: 0 }];
    }))
    .unwrap_err()
    .contains("disconnected pieces"));
    assert!(validate_definitions(&mutate(|building| {
        building.overhead_clearance = vec![Coordinate { q: 1, r: 0 }];
        building.service_envelope = vec![Coordinate { q: 1, r: 0 }];
    }))
    .unwrap_err()
    .contains("envelope and clearance"));

    // A taller tier may take more ground than the one it replaces, and it keeps every port it
    // had.
    //
    // That falls out of the growth rule rather than being enforced a second time. An output ray
    // binds to the first cell off the footprint, so the only growth that could take a port away
    // is growth into the very hex the ray binds at — and that hex is occupied by the thing being
    // fed, which is exactly what the check refuses. A building can gain an adjacency by getting
    // bigger; it cannot lose one.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 12]);
    for item_id in [1, 3, 6, 11, 16, 19, 20] {
        core.player.inventory.insert(item_id, 60);
    }
    core.player.carry_slots = 99;
    core.player.build_range = 1 << 20;
    set_player_hex(&mut core, 3, 2);
    // A superset of the extractor's own two hexes, growing south onto free ground.
    set_test_footprint(&mut core, 19, &[(0, 0), (1, 0), (0, 1)]);

    stock_for(&mut core, 2, 1);
    core.place(3, 0, 1, 0, None).unwrap();
    // The extractor stands on (3, 0) and (4, 0), so the hex its output ray binds at is the
    // first one past its own eastern cell.
    core.place(5, 0, 2, 0, None).unwrap();
    let extractor = core.entity_at(3, 0).expect("the extractor stands");
    let belt = core.entity_at(5, 0).expect("the belt stands");
    let fed = core.entities[belt].id;
    assert_eq!(
        core.graph[extractor].primary(),
        Some(belt),
        "the extractor feeds the belt in front of it"
    );

    core.upgrade(3, 0).unwrap();
    let extractor = core.entity_at(3, 0).expect("the deeper extractor stands");
    assert_eq!(core.entities[extractor].placed.definition_id, 19);
    assert_eq!(
        core.entity_at(3, 1),
        Some(extractor),
        "the taller tier took the free hex beside it"
    );
    let target = core.graph[extractor]
        .primary()
        .expect("it is still feeding something");
    assert_eq!(
        core.entities[target].id, fed,
        "and it is the same belt it was feeding"
    );

    // The growth check is one atomic question asked before anything is charged or written. A tier
    // that cannot fit leaves the building, the neighbour in its way and the player's pack exactly
    // as they were.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 12]);
    for item_id in [1, 3, 6, 11, 16, 19, 20] {
        core.player.inventory.insert(item_id, 60);
    }
    core.player.carry_slots = 99;
    reach(&mut core);
    set_player_hex(&mut core, 3, 2);
    // A superset of the extractor's own two hexes, growing east into the belt it feeds.
    set_test_footprint(&mut core, 19, &[(0, 0), (1, 0), (2, 0)]);

    stock_for(&mut core, 2, 1);
    core.place(3, 0, 1, 0, None).unwrap();
    core.place(5, 0, 2, 0, None).unwrap();
    let extractor = core.entity_at(3, 0).expect("the extractor stands");
    let belt = core.entity_at(5, 0).expect("the belt stands");
    let before = core.player.inventory.clone();

    let refusal = core.upgrade(3, 0).unwrap_err();
    assert!(refusal.contains("needs more room"), "{refusal}");
    assert_eq!(core.entities[extractor].placed.definition_id, 1);
    assert_eq!(
        core.entity_at(5, 0),
        Some(belt),
        "the neighbour is untouched"
    );
    assert_eq!(
        core.player.inventory, before,
        "a refused upgrade is not charged"
    );

    // Ground the pair could not stand on together is the same refusal, asked of the whole
    // enlarged footprint rather than of the cell being grown onto.
    core.erase(5, 0).unwrap();
    core.set_creative(true);
    reach(&mut core);
    for _ in 0..MAX_GRADE_STEPS {
        core.edit_ground(&ground_edit(5, 0, GroundAction::Lower))
            .unwrap();
    }
    let refusal = core.upgrade(3, 0).unwrap_err();
    assert!(refusal.contains("level a pad"), "{refusal}");
    assert_eq!(core.entities[extractor].placed.definition_id, 1);

    // Occupied foundation, service envelope and overhead clearance are three different claims.
    //
    // Envelope is reserved empty ground: neighbours cannot occupy it, belts included, but the
    // player can walk through. Clearance is air: a belt may pass under a rotor, a machine may not.
    // Neither claim enters the occupancy index, so output rays still bind at the first occupied
    // cell off the hull.
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    set_test_envelope(&mut core, 4, &[(1, 0)]);
    set_test_clearance(&mut core, 17, &[(1, 0)]);
    set_test_footprint(&mut core, 17, &[(0, 0)]);

    core.place(0, 0, 4, 0, None).unwrap();
    let container = core.entity_at(0, 0).expect("the crate stands");
    assert_eq!(core.entity_at(1, 0), None, "envelope is not occupancy");
    assert!(
        core.walkable_hex(1, 0),
        "the player can walk the reserved service hex"
    );
    let reserved = core.place(1, 0, 4, 0, None).unwrap_err();
    assert!(
        reserved.contains("reserved around the container"),
        "{reserved}"
    );
    let belt_on_envelope = core.place(1, 0, 2, 0, None).unwrap_err();
    assert!(
        belt_on_envelope.contains("reserved around the container"),
        "{belt_on_envelope}"
    );
    assert_eq!(core.entity_at(0, 0), Some(container));

    core.erase(0, 0).unwrap();
    core.place(3, 0, 17, 0, None).unwrap();
    let turbine = core.entity_at(3, 0).expect("the turbine stands");
    assert_eq!(core.entity_at(4, 0), None, "clearance is not occupancy");
    assert!(
        core.walkable_hex(4, 0),
        "the ground under a rotor stays open"
    );
    core.place(4, 0, 2, 0, None).unwrap();
    assert!(
        core.entity_at(4, 0).is_some(),
        "a belt may pass under the rotor"
    );
    core.erase(4, 0).unwrap();
    let machine = core.place(4, 0, 4, 0, None).unwrap_err();
    assert!(machine.contains("overhead clearance"), "{machine}");
    assert_eq!(core.entity_at(3, 0), Some(turbine));

    // An upgrade into a cell reserved at placement does not re-ask occupancy: the envelope held
    // it empty. Growing outside that envelope is still the atomic check.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 12]);
    for item_id in [1, 3, 6, 11, 16, 19, 20] {
        core.player.inventory.insert(item_id, 60);
    }
    core.player.carry_slots = 99;
    core.player.build_range = 1 << 20;
    set_player_hex(&mut core, 3, 2);
    set_test_footprint(&mut core, 19, &[(0, 0), (1, 0), (0, 1)]);

    stock_for(&mut core, 2, 1);
    core.place(3, 0, 1, 0, None).unwrap();
    let extractor = core.entity_at(3, 0).expect("the extractor stands");
    assert_eq!(
        core.entity_at(3, 1),
        None,
        "the reserved growth hex is not occupied yet"
    );
    core.upgrade(3, 0).unwrap();
    assert_eq!(core.entities[extractor].placed.definition_id, 19);
    assert_eq!(
        core.entity_at(3, 1),
        Some(extractor),
        "the taller tier took the hex its envelope reserved"
    );
}

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

#[test]
fn the_founding_contract_advances_stage_by_stage_and_carries_its_surplus() {
    let mut core = game("new-game");
    core.power_unmetered = false;
    set_player_hex(&mut core, 1, 0);
    // Research is funded by filling what the hub posted, one board row at a time. The opening
    // three are ore, stone, and wood, and each is worth ten insight.
    for (item, quantity) in [(1, 10), (6, 10), (9, 10)] {
        core.player.inventory.insert(item, quantity);
        core.deposit_inventory().unwrap();
    }
    assert_eq!(core.insight, 30);
    grant_foundations(&mut core);
    core.research(3).unwrap();
    stock_for(&mut core, 1, 1);
    stock_for(&mut core, 3, 1);
    stock_for(&mut core, 12, 2);
    stock_for(&mut core, 13, 1);
    core.player.inventory.insert(5, 16);
    core.player.inventory.insert(24, 8);
    set_player_hex(&mut core, 4, 2);
    // The same westward line, laid out around what each machine now stands on. The hub covers
    // every hex within one of the origin, so the line starts three further east and the
    // composer hands into the hub's eastern rim, which is what closes the stage. An extractor
    // is placed on its deposit rather than beside it, so the ore is written under the anchor
    // it moved to: this is a test about stages, not about where a generator puts iron.
    core.write_overlay(6, 0, 1, 2, 48);
    core.place(6, 0, 1, 3, None).unwrap();
    core.place(4, 0, 2, 3, None).unwrap();
    core.place(3, 0, 3, 3, Some(1)).unwrap();
    let composer = core.entity_at(3, 0).unwrap();
    core.entities[composer]
        .input_inventory
        .extend([(11, 1), (19, 1)]);
    set_player_hex(&mut core, 5, 2);
    let pole = try_place_near(&mut core, (6, 0), 12);
    let burner = try_place_near(&mut core, pole, 13);
    try_place_near(&mut core, (3, 0), 12);
    let _ = burner;
    if let Some(burner) = core
        .entities
        .iter_mut()
        .find(|entity| entity.kind == BuildingKind::Generator)
    {
        burner.inventory.insert(5, 16);
    }
    core.tick_many(500);
    // The running line closes the first stage, and closing it is deliberately not the end of
    // the contract: the hub has grown once, and free play has not been declared yet.
    assert_eq!(core.contract_stage, 1);
    assert!(!core.victory);
    assert_eq!(core.contract_snapshot().stage_key, "foundry");
    // The foundry module, delivered by hand. What this pins is the stage machinery, not a
    // second smelting line: the bill is two items from two chains, and both have to arrive.
    set_player_hex(&mut core, 0, -1);
    core.player.inventory.insert(11, 16);
    core.deposit_inventory().unwrap();
    assert_eq!(core.contract_stage, 1, "half a bill is not a stage");
    assert!(!core.victory);
    core.player.inventory.insert(14, 20);
    core.deposit_inventory().unwrap();
    assert_eq!(core.contract_stage, 2);
    assert!(core.victory);
    // Nothing is left to ask for, and the requirement list says so rather than repeating the
    // last bill at full.
    assert!(core.contract_snapshot().requirements.is_empty());
    assert!(core.contract_snapshot().complete);
    let checksum = core.checksum();
    core.tick_many(1);
    assert!(core.victory);
    assert_ne!(core.checksum(), checksum);

    // A stage consumes its bill and carries the surplus to the next one.
    let mut core = game("new-game");
    set_player_hex(&mut core, 0, -1);
    // Everything the whole contract asks for, in one delivery, plus one component too many.
    // The hub takes a later stage's materials as well as the current one's, which is the
    // surplus rule: a line automated early is credited when the stage that wants it arrives.
    core.player.inventory.insert(2, 2);
    core.player.inventory.insert(11, 16);
    core.player.inventory.insert(14, 20);
    core.deposit_inventory().unwrap();
    for id in [1, 2, 4, 8] {
        assert!(
            core.researched.contains(&id),
            "closing the opening commission grants {id}"
        );
    }
    // Both stages close in the same delivery, which is the reason the advance loops rather
    // than closing one stage per arriving item.
    assert_eq!(core.contract_stage, 2);
    assert!(core.victory);
    // Each stage consumed exactly its own bill, and the second component was never taken at
    // all: the hub accepts what it asked for and leaves the rest in the pack.
    assert_eq!(core.contract_contributed.get(&2), Some(&0));
    assert_eq!(core.contract_contributed.get(&11), Some(&0));
    assert_eq!(core.contract_contributed.get(&14), Some(&0));
    assert_eq!(core.player.inventory.get(&2), Some(&1));
    // A finished contract does not close the hub. The board is still posting, filling a row is
    // still what pays, and no stage index runs off the end of the list.
    let insight = core.insight;
    core.player.inventory.insert(1, 10);
    core.deposit_inventory().unwrap();
    assert!(core.insight > insight);
    assert_eq!(core.contract_stage, 2);
}

/// The price is posted, and it is paid on completion — never before, and never for anything the
/// hub did not ask for.
#[test]
fn the_board_posts_pays_passes_and_saves_what_the_player_could_make() {
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    let board = |core: &Core| -> Vec<String> {
        core.request_snapshots()
            .iter()
            .filter(|request| request.state == ProjectState::Posted)
            .map(|request| request.key.clone())
            .collect()
    };
    assert_eq!(board(&core), ["ore-assay", "cliff-stone", "cordwood"]);
    // Half a request is worth nothing. This is the whole difference from the currency it
    // replaced, where five ore was five insight and the player never saw the rate.
    core.player.inventory.insert(1, 5);
    core.deposit_inventory().unwrap();
    assert_eq!(core.insight, 0);
    assert_eq!(core.request_snapshots()[0].delivered, 5);
    core.player.inventory.insert(1, 5);
    core.deposit_inventory().unwrap();
    assert_eq!(core.insight, 10);
    // The slot that was filled holds the next row, in its own place: the board does not
    // shuffle, and it does not repost the row that was just paid for while others are unseen.
    assert_eq!(board(&core), ["clay-survey", "cliff-stone", "cordwood"]);
    assert_eq!(core.request_rounds.get(&1), Some(&1));
    assert_eq!(core.request_fills.get(&1), Some(&1));
    // And the row it paid for is retired, not merely off the board: the catalogue still
    // carries it so the player can see the work is done.
    let paid = core
        .request_snapshots()
        .into_iter()
        .find(|request| request.key == "ore-assay")
        .expect("a filled project stays in the catalogue");
    assert_eq!(paid.state, ProjectState::Complete);
    assert_eq!(paid.delivered, 0, "a retired project holds no progress");

    // Passing a row costs it a place in the queue, not its first-fill bonus. Skip used to share
    // `request_rounds` with payment, which would have turned "I have not found this yet" into
    // two insight for ten gathers.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.skip_request(0).unwrap();
    assert_eq!(core.request_rounds.get(&1), Some(&1));
    assert!(core.request_fills.get(&1).is_none());
    core.requests[0] = RequestState { request_id: 1 };
    let before = core.insight;
    core.player.inventory.insert(1, 10);
    core.deposit_inventory().unwrap();
    assert_eq!(
        core.insight - before,
        10,
        "a skipped row still pays its first fill"
    );
    assert_eq!(core.request_fills.get(&1), Some(&1));

    // A filled project is finished, and finished is for good. Delivering its item again is
    // ordinary freight into the hub, not a second payment.
    //
    // This is the shape the catalogue used to have inverted. A raw row paid ten once and two for
    // ever after, so the board was a tap: slow, dull, and unbounded, which meant no amount of
    // research could ever actually be priced. Demand is a bill now.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.player.inventory.insert(1, 10);
    core.deposit_inventory().unwrap();
    assert_eq!(core.insight, 10);
    assert_eq!(core.request_fills.get(&1), Some(&1));
    // Force ore-assay back into a slot by hand — nothing in the game can do this — and the
    // hub still refuses to buy what it has already commissioned.
    core.requests[0] = RequestState { request_id: 1 };
    core.player.inventory.insert(1, 10);
    core.deposit_inventory().unwrap();
    assert_eq!(core.insight, 10, "a second delivery buys nothing");
    assert_eq!(core.request_fills.get(&1), Some(&1));

    // Passing a part-filled project keeps what was handed over. Progress belongs to the project,
    // not to the slot it happened to be posted in.
    //
    // Under repeatable demand a skip that dropped the count cost a few minutes. Under a finite
    // catalogue it would destroy goods whose reward can never be earned again, so the count moved
    // off the board and onto the project itself.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.player.inventory.insert(1, 6);
    core.deposit_inventory().unwrap();
    assert_eq!(core.insight, 0);
    assert_eq!(core.project_delivered(1), 6);
    core.skip_request(0).unwrap();
    assert_eq!(core.project_delivered(1), 6, "the skip kept the ore");
    // Post it again and the remaining four finish it at the full price.
    core.post_request(project_id(&core, "ore-assay")).unwrap();
    core.player.inventory.insert(1, 4);
    core.deposit_inventory().unwrap();
    assert_eq!(core.insight, 10);
    assert_eq!(core.project_delivered(1), 0);

    // Posting is the player's choice, and the catalogue is the whole board. A finite bill has to
    // be browsable or a row that funds nothing else could hide behind two that do.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    // Commit ore to the first slot, so an untouched slot is the cheapest thing to displace.
    let committed = project_id(&core, "ore-assay");
    let wanted = project_id(&core, "clay-survey");
    core.player.inventory.insert(1, 4);
    core.deposit_inventory().unwrap();
    assert_eq!(core.project_delivered(committed), 4);
    core.post_request(wanted).unwrap();
    let posted: Vec<_> = core.requests.iter().map(|slot| slot.request_id).collect();
    assert!(posted.contains(&wanted), "clay-survey took a slot");
    assert!(
        posted.contains(&committed),
        "the part-filled row was displaced ahead of an untouched one, got {posted:?}"
    );
    assert_eq!(
        core.post_request(wanted),
        Err("Clay survey is already on the board".to_owned())
    );
    assert!(core.post_request(9999).is_err());

    // Part-delivered goods survive a save. They are the one thing in the request system a player
    // cannot re-earn, so losing them across a reload would be losing the work outright.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    let ore = project_id(&core, "ore-assay");
    core.player.inventory.insert(1, 6);
    core.deposit_inventory().unwrap();
    core.skip_request(0).unwrap();
    assert!(!posted_board(&core).contains(&"ore-assay".to_owned()));
    let (definitions, technologies, scenarios) = catalogs();
    let resumed = Core::from_save(
        &definitions,
        &technologies,
        &scenarios,
        &core.save_string().unwrap(),
    )
    .unwrap();
    assert_eq!(resumed.project_delivered(ore), 6);
    assert_eq!(core.checksum(), resumed.checksum());
    // And the checksum notices: progress is saved state, so a file that lost it is a different
    // game rather than the same game rounded.
    let with = core.checksum();
    core.request_delivered.remove(&ore);
    assert_ne!(with, core.checksum());

    // The board closes when the hub has nothing left to ask for. A finite catalogue that quietly
    // reposted its last row for ever would be the tap again, wearing a bill's clothes.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    for request in &core.definitions.requests {
        core.request_fills.insert(request.id, 1);
    }
    core.requests.clear();
    core.refill_requests();
    assert!(core.requests.is_empty(), "nothing is left to post");
    assert!(core
        .request_snapshots()
        .iter()
        .all(|request| request.state == ProjectState::Complete));

    // The hub takes what it asked for and leaves the rest in the pack — by hand and by belt, at one
    // predicate, so a line cannot void cargo the key would have refused.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.player.inventory.insert(3, 6);
    assert!(core
        .deposit_inventory()
        .unwrap_err()
        .contains("not asking for anything"));
    assert_eq!(core.player.inventory.get(&3), Some(&6));
    let hub = core
        .entities
        .iter()
        .position(|entity| entity.kind == BuildingKind::Hub)
        .expect("the landing hub");
    assert!(!core.can_accept(
        hub,
        Cargo {
            item_id: 3,
            quantity: 1
        }
    ));
    assert!(core.can_accept(
        hub,
        Cargo {
            item_id: 1,
            quantity: 1
        }
    ));
    // Ten ore is the whole standing order, so the eleventh has nowhere to go either.
    core.player.inventory.insert(1, 10);
    core.deposit_inventory().unwrap();
    assert!(!core.can_accept(
        hub,
        Cargo {
            item_id: 1,
            quantity: 1
        }
    ));

    // The board is drawn from the rules, so it can never post something the rules refuse.
    let mut core = game("new-game");
    assert!(core.item_reachable(1, 0), "ore is in the ground");
    assert!(
        !core.item_reachable(11, 0),
        "a plate needs a smelter nobody may build yet"
    );
    assert!(
        !core.item_reachable(10, 0),
        "water needs a pump, and water is nobody's field"
    );
    assert!(
        !core.item_reachable(CRYSTAL, 0),
        "signal crystal is machine only until an extractor is unlocked"
    );
    // Passing every slot repeatedly walks the whole eligible list. Nothing that needs a machine
    // may appear in it, however far up the catalogue that row stands.
    for _ in 0..12 {
        for slot in 0..REQUEST_SLOTS {
            let item = core.request_snapshots()[slot].item_id;
            assert!(
                core.item_reachable(item, 0),
                "the board posted item {item}, which cannot be produced yet"
            );
            core.skip_request(slot).unwrap();
        }
    }
    core.insight = 100;
    grant_foundations(&mut core);
    core.research(5).unwrap();
    assert!(core.item_reachable(11, 0), "the smelter unlocks the plate");
    assert!(
        core.item_reachable(CRYSTAL, 0),
        "an extractor unlocks the crystal field"
    );

    // Passing a row costs it a place in the queue, and costs the player whatever they had already
    // put against it. It is a decision, not a free reroll.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.player.inventory.insert(1, 5);
    core.deposit_inventory().unwrap();
    assert_eq!(core.request_snapshots()[0].delivered, 5);
    core.skip_request(0).unwrap();
    assert_eq!(core.request_snapshots()[0].key, "clay-survey");
    assert_eq!(core.request_snapshots()[0].delivered, 0);
    assert_eq!(core.insight, 0);
    assert!(core.skip_request(9).unwrap_err().contains("no request"));

    // Once a smelter is unlocked, a free slot is reserved for the deepest reachable row rather
    // than the next unseen ore assay. The other two slots still cycle, and nothing unmakeable is
    // posted — reservation walks the same `item_reachable` predicate the rest of the board does.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.insight = 100;
    grant_foundations(&mut core);
    core.research(5).unwrap();
    assert!(core.item_reachable(11, 0));
    let before: Vec<String> = posted_board(&core);
    assert!(
        before.iter().all(|key| {
            let item = core
                .definitions
                .requests
                .iter()
                .find(|request| request.key == *key)
                .map(|request| request.item_id)
                .unwrap();
            core.item_depth(item) == 0
        }),
        "the opening board is raw, got {before:?}"
    );
    core.player.inventory.insert(1, 10);
    core.deposit_inventory().unwrap();
    let after: Vec<String> = posted_board(&core);
    let depths: Vec<u32> = after
        .iter()
        .map(|key| {
            let item = core
                .definitions
                .requests
                .iter()
                .find(|request| request.key == *key)
                .map(|request| request.item_id)
                .unwrap();
            core.item_depth(item)
        })
        .collect();
    assert!(
        depths.iter().any(|&depth| depth > 0),
        "the freed slot should post the deepest reachable row, got {after:?} at {depths:?}"
    );
    for request in core
        .request_snapshots()
        .iter()
        .filter(|request| request.state == ProjectState::Posted)
    {
        assert!(
            core.item_reachable(request.item_id, 0),
            "reserved slot posted item {}, which cannot be produced",
            request.item_id
        );
    }
}

#[test]
fn the_hub_takes_delivery_from_every_footprint_cell_and_saves_its_board() {
    let mut core = game("new-game");
    // Give player iron ore (id 1) and wood (id 8). Both are standing requests in new game.
    core.player.inventory.insert(1, 10);
    core.player.inventory.insert(8, 10);
    set_player_hex(&mut core, 0, 1);
    // Deliver only iron ore
    core.deposit_item(Some(1)).unwrap();
    // Iron ore was delivered, wood remains in pack
    assert_eq!(core.player.inventory.get(&1), None);
    assert_eq!(core.player.inventory.get(&8), Some(&10));

    // A delivery is in range of the landing hub when the player stands beside *any* cell it
    // occupies. The hub is seven hexes; measuring from the anchor alone made the far lobes
    // decorative — you could stand next to them and still be told to walk closer.
    let mut core = game("new-game");
    let hub = core
        .entities
        .iter()
        .find(|entity| entity.kind == BuildingKind::Hub)
        .expect("the landing hub");
    assert_eq!(
        core.entity_footprint(hub),
        vec![
            Coordinate { q: 0, r: 0 },
            Coordinate { q: 1, r: 0 },
            Coordinate { q: 0, r: 1 },
            Coordinate { q: -1, r: 1 },
            Coordinate { q: -1, r: 0 },
            Coordinate { q: 0, r: -1 },
            Coordinate { q: 1, r: -1 },
        ]
    );

    // Beside the south-east lobe, two hexes from the origin. The old origin-circle refused this.
    core.player.inventory.insert(1, 1);
    set_player_hex(&mut core, 0, 2);
    core.deposit_item(Some(1)).unwrap();
    assert_eq!(core.player.inventory.get(&1), None);

    // Beside the south-west lobe.
    core.player.inventory.insert(1, 1);
    set_player_hex(&mut core, -2, 2);
    core.deposit_item(Some(1)).unwrap();
    assert_eq!(core.player.inventory.get(&1), None);

    // Three hexes past the south-east lobe is past a two-hex reach from every occupied cell.
    core.player.inventory.insert(1, 1);
    set_player_hex(&mut core, 0, 4);
    assert!(core
        .deposit_item(Some(1))
        .unwrap_err()
        .contains("beside the landing hub"));

    // A board is saved state, restored rather than redrawn.
    let (definitions, technologies, scenarios) = catalogs();
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.player.inventory.insert(1, 10);
    core.player.inventory.insert(6, 4);
    core.deposit_inventory().unwrap();
    let before = core.request_snapshots();
    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.request_snapshots(), before);
    assert_eq!(restored.request_rounds, core.request_rounds);
    assert_eq!(restored.request_fills, core.request_fills);
    assert_eq!(restored.insight, 10);
    // A row this build does not ship would survive the file and then be drawn as a request
    // nobody can read, so the loader refuses it before the checksum ever gets the chance.
    let forged = save.replace("\"request_id\":4", "\"request_id\":9999");
    assert_ne!(forged, save);
    let refusal = Core::from_save(&definitions, &technologies, &scenarios, &forged)
        .err()
        .expect("a forged board is refused");
    assert!(refusal.contains("unknown hub request"), "{refusal}");
}

#[test]
fn a_save_resumes_and_replays_in_a_deterministic_order() {
    let (definitions, technologies, scenarios) = catalogs();
    let mut uninterrupted = game("factory-demo");
    // Metered on both sides, which is the shipped rule and the only way this test is honest.
    // `power_unmetered` is a harness hook that no save carries, so a resumed core always comes
    // back metered; leaving the running one unmetered compared two different games. It passed
    // until v0.19 only because a fully supplied grid used to make the two paths agree by
    // arithmetic — with banked energy they no longer do, and the resume is exactly what should
    // catch that.
    uninterrupted.power_unmetered = false;
    uninterrupted.tick_many(120);
    let save = uninterrupted.save_string().unwrap();
    assert!(save.starts_with(SAVE_PREFIX));
    let mut resumed = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    uninterrupted.tick_many(180);
    resumed.tick_many(180);
    assert_eq!(uninterrupted.checksum(), resumed.checksum());
    assert_eq!(uninterrupted.delivered, resumed.delivered);
    assert!(Core::from_save(&definitions, &technologies, &scenarios, "bad").is_err());
    // Written against the live version rather than a literal, so bumping a version is a
    // one-line change in one place and this test keeps testing the rejection it names.
    let incompatible = save.replacen(
        &format!("\"definition_version\":{}", definitions.version),
        "\"definition_version\":999",
        1,
    );
    assert!(Core::from_save(&definitions, &technologies, &scenarios, &incompatible).is_err());
    // Version 16 is the previous envelope. Technology catalog 7 has neither capability row, so
    // an empty fresh run can be spelled exactly as that release did and must migrate to the
    // same checksum without being granted research.
    let previous_source = game("new-game").save_string().unwrap();
    let previous_envelope = previous_source
        .replacen("\"technology_version\":8", "\"technology_version\":7", 1)
        .replacen("\"definition_version\":16", "\"definition_version\":15", 1)
        .replacen(
            &format!("\"save_version\":{SAVE_VERSION}"),
            "\"save_version\":16",
            1,
        );
    assert_refused_as_legacy_scale(Core::from_save(
        &definitions,
        &technologies,
        &scenarios,
        &previous_envelope,
    ));
    let baseline =
        Core::from_save(&definitions, &technologies, &scenarios, &previous_source).unwrap();
    assert_eq!(baseline.player.walk_goal, None);
    // Everything older still is. There is no migration for it, and reading one as a newer
    // spelling of the same thing is exactly what the boundary refuses to do.
    let unmigratable = save.replacen(
        &format!("\"save_version\":{SAVE_VERSION}"),
        "\"save_version\":13",
        1,
    );
    assert!(
        Core::from_save(&definitions, &technologies, &scenarios, &unmigratable).is_err(),
        "a version-13 save must be refused rather than read with six-direction orientations"
    );
    // v0.16 takes the generator to 6 because `WorldParams` entered the envelope and the
    // checksum. A version-5 envelope names no parameters at all, so it cannot be read as the
    // default set — it is rejected.
    let old_world = save.replacen(
        &format!("\"world_generator_version\":{WORLD_GENERATOR_VERSION}"),
        &format!(
            "\"world_generator_version\":{}",
            WORLD_GENERATOR_VERSION - 1
        ),
        1,
    );
    assert!(Core::from_save(&definitions, &technologies, &scenarios, &old_world).is_err());
    // The parameters are checksummed, so editing them in a saved file is caught as tampering
    // rather than quietly regenerating a different world under the same overlay.
    let edited_params = save.replacen("\"water_level\":18000", "\"water_level\":19000", 1);
    assert_ne!(edited_params, save, "the save carries its world parameters");
    assert!(Core::from_save(&definitions, &technologies, &scenarios, &edited_params).is_err());

    // Reset replay and scenario insertion order are deterministic.
    let (definitions, technologies, scenarios) = catalogs();
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|value| value.key == "factory-demo")
        .unwrap();
    let mut reversed = scenario.clone();
    reversed.buildings.reverse();
    let mut a = Core::new(&definitions, &technologies, scenario, None, None).unwrap();
    let mut b = Core::new(&definitions, &technologies, &reversed, None, None).unwrap();
    a.tick_many(300);
    b.tick_many(300);
    assert_eq!(a.checksum(), b.checksum());
    let expected = a.checksum();
    let mut replay = Core::new(&definitions, &technologies, scenario, None, None).unwrap();
    replay.tick_many(300);
    assert_eq!(replay.checksum(), expected);
}

/// Every status spelling the host can render. The wire carries the index, so a reordering here
/// is a wire break; the fixture is what makes that break visible in both languages at once.
const WIRE_STATUSES: [(EntityStatus, &str); 18] = [
    (EntityStatus::OutputBlocked, "output blocked"),
    (EntityStatus::DepositDepleted, "deposit depleted"),
    (EntityStatus::Extracting, "extracting"),
    (EntityStatus::NoWaterInReach, "no water in reach"),
    (EntityStatus::Pumping, "pumping"),
    (EntityStatus::Composing, "composing"),
    (EntityStatus::OutOfFuel, "out of fuel"),
    (EntityStatus::WaitingForInputs, "waiting for inputs"),
    (EntityStatus::Buffered, "buffered"),
    (EntityStatus::Carrying, "carrying"),
    (EntityStatus::Receiving, "receiving"),
    (EntityStatus::LandingHub, "landing hub"),
    (EntityStatus::Idle, "idle"),
    (EntityStatus::NoPower, "no power"),
    (EntityStatus::Generating, "generating"),
    (EntityStatus::Brownout, "brownout"),
    (EntityStatus::NoBoiler, "no boiler"),
    (EntityStatus::SwitchedOff, "switched off"),
];

const WIRE_KINDS: [(BuildingKind, &str); 11] = [
    (BuildingKind::Extractor, "extractor"),
    (BuildingKind::Belt, "belt"),
    (BuildingKind::Composer, "composer"),
    (BuildingKind::Container, "container"),
    (BuildingKind::Consumer, "consumer"),
    (BuildingKind::Hub, "hub"),
    (BuildingKind::Pump, "pump"),
    (BuildingKind::Pole, "pole"),
    (BuildingKind::Generator, "generator"),
    (BuildingKind::Boiler, "boiler"),
    (BuildingKind::Bridge, "bridge"),
];

const WIRE_TERRAIN: [(Terrain, &str); 7] = [
    (Terrain::DeepWater, "deep_water"),
    (Terrain::ShallowWater, "shallow_water"),
    (Terrain::Shore, "shore"),
    (Terrain::Lowland, "lowland"),
    (Terrain::Hills, "hills"),
    (Terrain::Highland, "highland"),
    (Terrain::Cliff, "cliff"),
];

#[test]
fn entity_status_spellings_are_what_the_host_renders() {
    // The enum exists so the wire can carry a byte, but what reaches the player is still the
    // string. Renaming a variant is allowed; changing its spelling changes the game's text.
    for (status, spelling) in WIRE_STATUSES {
        assert_eq!(
            serde_json::to_value(status).unwrap(),
            serde_json::Value::String(spelling.to_owned()),
            "status spelling changed"
        );
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Deltas chosen to walk the whole surface of the encoding rather than to look like a frame:
/// an empty group mask, every scalar group at once, both patch kinds carrying entries, and the
/// replace form with nothing in it.
fn wire_fixture_cases() -> Vec<(&'static str, SnapshotDelta)> {
    // A closure rather than a value: every case below fills a different handful of groups in
    // and leaves the rest absent, and `..` moves what it spreads from.
    let empty = || SnapshotDelta {
        boundaries: None,
        ground: None,
        spoil: None,
        water: None,
        base_revision: 0,
        revision: 1,
        tick: 0,
        checksum: 0,
        belt_transit_ticks: BELT_TRANSIT_TICKS as u32,
        scenario: None,
        scenario_name: None,
        world_version: None,
        seed: None,
        delivered: None,
        delivered_by_item: None,
        insight: None,
        victory: None,
        contract: None,
        requests: None,
        player: None,
        researched: None,
        research_availability: None,
        skills: None,
        chunks: None,
        terrain: None,
        resources: None,
        buildings: None,
        ground_items: None,
        events: None,
    };

    // A frame that changed nothing but the clock. The mask is zero and the body is empty, which
    // is the case a quiet factory spends most of its frames in.
    let quiet = SnapshotDelta {
        base_revision: 41,
        revision: 42,
        tick: 1_000_000,
        checksum: 0xdead_beef,
        ..empty()
    };

    // Every scalar group, with negative coordinates and multi-byte varints, so a decoder that
    // reads a field in the wrong order or forgets to zigzag cannot pass.
    let scalars = SnapshotDelta {
        base_revision: 2,
        revision: 3,
        tick: 300,
        checksum: 7,
        scenario: Some("new-game".to_owned()),
        scenario_name: Some("New game".to_owned()),
        world_version: Some(5),
        seed: Some(4_294_967_295),
        // Exactly 2^53 - 1. The invariant is that nothing wider than that travels as a number,
        // and the host still receives these as JavaScript numbers, so the boundary itself is
        // the largest value worth pinning — a fixture above it would pin rounding, not the
        // encoding.
        delivered: Some(9_007_199_254_740_991),
        delivered_by_item: Some(vec![
            Ingredient64 {
                item_id: 1,
                quantity: 1_000_000_000_000,
            },
            Ingredient64 {
                item_id: 300,
                quantity: 0,
            },
        ]),
        insight: Some(64_000),
        victory: Some(true),
        // A multi-line bill with one line over-delivered and one untouched, so a decoder that
        // loses the count, swaps `delivered` and `required`, or reads the trailing flag before
        // the list cannot pass.
        contract: Some(ContractSnapshot {
            key: "founding".to_owned(),
            name: "Founding contract".to_owned(),
            stage: 1,
            stages: 2,
            stage_key: "foundry".to_owned(),
            stage_name: "Raise the foundry module".to_owned(),
            stage_brief: "Plate and brick, from two landscapes.".to_owned(),
            requirements: vec![
                ContractRequirement {
                    item_id: 11,
                    delivered: 16,
                    required: 16,
                },
                ContractRequirement {
                    item_id: 14,
                    delivered: 0,
                    required: 20,
                },
            ],
            complete: false,
        }),
        // A board with one row part-filled and one untouched, so a decoder that loses the
        // count, swaps `delivered` and `required`, or reads the price before the numbers cannot
        // pass. The brief carries the multi-byte case the events list carries too. Two
        // different states, either side of the numbers, so a decoder that drops the state byte
        // or reads it at the wrong offset fails here rather than in the panel.
        requests: Some(vec![
            RequestSnapshot {
                key: "plate-stock".to_owned(),
                name: "Plate stock".to_owned(),
                brief: "Smelted iron — not ore.".to_owned(),
                item_id: 11,
                delivered: 3,
                required: 8,
                insight: 22,
                state: ProjectState::Posted,
            },
            RequestSnapshot {
                key: "cliff-stone".to_owned(),
                name: "Cliff stone".to_owned(),
                brief: "Cut stone for the apron.".to_owned(),
                item_id: 6,
                delivered: 0,
                required: 10,
                insight: 10,
                state: ProjectState::Complete,
            },
        ]),
        player: Some(PlayerSnapshot {
            state: PlayerState {
                x: -123_456,
                y: 654_321,
                facing_x: -1000,
                facing_y: 866,
                move_x: 0,
                move_y: -1,
                inventory: BTreeMap::from([(1, 40), (3, 20), (65_535, 1)]),
                hand: Some(Cargo {
                    item_id: 5,
                    quantity: 3,
                }),
                action_cooldown: 5,
                build_range: 4096,
                carry_slots: 12,
                walk_goal: Some(Coordinate { q: -70, r: 12 }),
            },
            carry_stacks: vec![
                Ingredient {
                    item_id: 1,
                    quantity: 40,
                },
                Ingredient {
                    item_id: 3,
                    quantity: 20,
                },
            ],
            radius: 580,
            action_cooldown_total: 6,
            extract_radius: 1,
            creative: true,
            // A route that steps in every direction the delta coding has to carry, ending on
            // the goal above, so the fixture pins the chain rather than a straight line.
            walk_path: vec![
                Coordinate { q: -74, r: 14 },
                Coordinate { q: -73, r: 14 },
                Coordinate { q: -73, r: 13 },
                Coordinate { q: -72, r: 13 },
                Coordinate { q: -72, r: 12 },
                Coordinate { q: -71, r: 12 },
                Coordinate { q: -70, r: 12 },
            ],
        }),
        researched: Some(vec![1, 2, 3, 4]),
        research_availability: Some(vec![
            ResearchAvailability {
                technology_id: 1,
                complete: true,
                insight_shortfall: 0,
                missing_prerequisites: vec![],
            },
            ResearchAvailability {
                technology_id: 300,
                complete: false,
                insight_shortfall: 70_000,
                missing_prerequisites: vec![5, 256],
            },
        ]),
        skills: Some(SkillsSnapshot {
            state: SkillsState {
                points: 2,
                purchased: BTreeSet::from([1]),
                granted: BTreeSet::from([300]),
                completed: BTreeSet::from([2, 400]),
                sandbox: true,
            },
            availability: vec![SkillAvailability {
                skill_id: 301,
                complete: false,
                points_shortfall: 128,
                current_value: 6,
                resulting_value: 10,
                missing_prerequisites: vec![300],
            }],
        }),
        chunks: Some(vec![
            ChunkSnapshot {
                chunk_q: 0,
                chunk_r: 0,
                entity_count: 3,
                x: -8192,
                y: -8192,
                span: 16_384,
            },
            ChunkSnapshot {
                chunk_q: -2,
                chunk_r: 1,
                entity_count: 0,
                x: -40_960,
                y: 8192,
                span: 16_384,
            },
        ]),
        events: Some(vec![
            "Gathered Iron ore".to_owned(),
            // Multi-byte UTF-8, because the string length is written in bytes and a decoder
            // that reads it as characters would desynchronise the rest of the buffer.
            "Delivered 3 × Steel — objective met".to_owned(),
        ]),
        ground_items: Some(vec![
            GroundItem {
                id: 1,
                q: -2,
                r: 5,
                item_id: 11,
                quantity: 4,
                despawn_tick: 900,
            },
            GroundItem {
                id: 2,
                q: 10,
                r: -3,
                item_id: 6,
                quantity: 1,
                despawn_tick: 600,
            },
        ]),
        ..empty()
    };

    // Both patches carrying entries: a bare belt beside a machine with every option set, a
    // removal list, a deposit patch over negative coordinates, and terrain.
    let patches = SnapshotDelta {
        base_revision: 10,
        revision: 11,
        tick: 512,
        checksum: 0x0102_0304,
        // A patch rather than a replace, a summit beside a flooded basin, and a height that
        // steps down by more than a byte of zigzag between the two: the pair pins the height
        // delta coding, a signed absolute bed, standing water and a drainage class at once.
        terrain: Some(TerrainDelta {
            replace: false,
            changed: vec![
                TileSnapshot {
                    q: -3,
                    r: -4,
                    x: -8_870,
                    y: -6_144,
                    radius: 1024,
                    terrain: Terrain::Cliff,
                    height: 4_212,
                    substrate: Substrate::Rock,
                    water_depth: 0,
                    discharge: 0,
                },
                TileSnapshot {
                    q: -2,
                    r: -4,
                    x: -7_096,
                    y: -6_144,
                    radius: 1024,
                    terrain: Terrain::DeepWater,
                    height: -37,
                    substrate: Substrate::Sand,
                    water_depth: 41,
                    discharge: 7,
                },
            ],
        }),
        resources: Some(ResourcesDelta {
            replace: false,
            changed: vec![
                ResourceSnapshot {
                    q: -32,
                    r: 0,
                    x: -56_768,
                    y: 0,
                    radius: 1024,
                    item_id: 1,
                    quantity: 0,
                    initial_quantity: 48,
                },
                ResourceSnapshot {
                    q: -32,
                    r: 3,
                    x: -54_107,
                    y: 4_608,
                    radius: 1024,
                    item_id: 2,
                    quantity: 17,
                    initial_quantity: 60,
                },
            ],
        }),
        buildings: Some(BuildingsDelta {
            replace: false,
            changed: vec![
                EntitySnapshot {
                    id: 7,
                    q: 2,
                    r: 0,
                    definition_id: 2,
                    kind: BuildingKind::Belt,
                    orientation: 3,
                    recipe_id: None,
                    scenario_owned: false,
                    cargo: None,
                    lane: Vec::new(),
                    inventory: Vec::new(),
                    input_inventory: Vec::new(),
                    fuel_inventory: Vec::new(),
                    output_inventory: Vec::new(),
                    output_routes: Vec::new(),
                    water_source: None,
                    progress: 0,
                    progress_total: 0,
                    fuel_charge: 0,
                    fuel_required: 0,
                    power_satisfied: 0,
                    power_demand: 0,
                    // A belt sets no high flag, so its flag field is still the one byte it was
                    // before the field became a uvarint. That is the whole point of the change
                    // and this entity is what pins it.
                    power_charge: 0,
                    power_capacity: 0,
                    status: EntityStatus::Idle,
                    next_id: None,
                    // No outputs at all, which is the empty branch list — the case every
                    // entity that is not a splitter encodes.
                    branch_ids: Vec::new(),
                    footprint: vec![Coordinate { q: 2, r: 0 }],
                },
                // A belt mid-run: one item finished crossing and waiting at the exit, three
                // more strung out behind it, and the last of those stepped on so long ago that
                // its elapsed count needs a second byte — the jammed lane the cadence exists
                // to make visible. Its lane flag is the highest entity bit there is, so this
                // is also the widest flag field the encoder writes.
                EntitySnapshot {
                    id: 12,
                    q: 3,
                    r: 0,
                    definition_id: 2,
                    kind: BuildingKind::Belt,
                    orientation: 3,
                    recipe_id: None,
                    scenario_owned: false,
                    cargo: Some(Cargo {
                        item_id: 1,
                        quantity: 1,
                    }),
                    lane: vec![
                        LaneItem {
                            cargo: Cargo {
                                item_id: 1,
                                quantity: 1,
                            },
                            entered: 300,
                        },
                        LaneItem {
                            cargo: Cargo {
                                item_id: 4,
                                quantity: 2,
                            },
                            entered: 495,
                        },
                        LaneItem {
                            cargo: Cargo {
                                item_id: 1,
                                quantity: 1,
                            },
                            entered: 512,
                        },
                    ],
                    inventory: Vec::new(),
                    input_inventory: Vec::new(),
                    fuel_inventory: Vec::new(),
                    output_inventory: Vec::new(),
                    output_routes: Vec::new(),
                    water_source: None,
                    progress: 0,
                    progress_total: 0,
                    fuel_charge: 0,
                    fuel_required: 0,
                    power_satisfied: 0,
                    power_demand: 0,
                    power_charge: 0,
                    power_capacity: 0,
                    status: EntityStatus::OutputBlocked,
                    next_id: Some(4_294_967_295),
                    branch_ids: Vec::new(),
                    footprint: vec![Coordinate { q: 3, r: 0 }],
                },
                EntitySnapshot {
                    id: 4_294_967_295,
                    q: -1,
                    r: 6,
                    definition_id: 3,
                    kind: BuildingKind::Composer,
                    orientation: 5,
                    recipe_id: Some(11),
                    scenario_owned: true,
                    cargo: Some(Cargo {
                        item_id: 4,
                        quantity: 2,
                    }),
                    lane: Vec::new(),
                    inventory: vec![
                        Ingredient {
                            item_id: 1,
                            quantity: 6,
                        },
                        Ingredient {
                            item_id: 5,
                            quantity: 300,
                        },
                    ],
                    input_inventory: vec![Ingredient {
                        item_id: 2,
                        quantity: 12,
                    }],
                    fuel_inventory: vec![Ingredient {
                        item_id: 5,
                        quantity: 7,
                    }],
                    output_inventory: vec![Ingredient {
                        item_id: 4,
                        quantity: 9,
                    }],
                    output_routes: vec![OutputRouteSnapshot {
                        item_id: 4,
                        q: -1,
                        r: 6,
                        direction: 5,
                        target_id: Some(7),
                    }],
                    // Synthetic every-field case: pins signed source offsets and the finite /
                    // replenishing rate payload without adding another entity to the fixture.
                    water_source: Some(WaterSourceSnapshot {
                        q: -3,
                        r: 8,
                        available: 12,
                        discharge: 3,
                        rate: 3,
                    }),
                    progress: 17,
                    progress_total: 40,
                    fuel_charge: 250,
                    fuel_required: 100,
                    power_satisfied: 8,
                    power_demand: 12,
                    // Both high bits set, so this entity's flag field is two bytes and the
                    // fixture carries a decoder that has to widen past the old fixed byte.
                    power_charge: 96,
                    power_capacity: 360,
                    status: EntityStatus::Composing,
                    next_id: Some(9),
                    // A full branch list, carrying both a small id and the largest one a u32
                    // holds, so the decoder is pinned at both ends of the range it must widen
                    // across. This entity is the fixture's every-field-at-its-limit case.
                    branch_ids: vec![4, 4_294_967_295],
                    // A multi-cell footprint, coded against the entity's own hex.
                    footprint: vec![
                        Coordinate { q: -1, r: 6 },
                        Coordinate { q: 0, r: 6 },
                        Coordinate { q: -1, r: 7 },
                    ],
                },
            ],
            removed: vec![1, 2, 900],
        }),
        ..empty()
    };

    // The full-replace form both patches take on the first frame, a reset, a new game, and a
    // load — here with nothing in it, so the replace flag is what is being read rather than
    // the entries after it.
    let replace = SnapshotDelta {
        boundaries: Some(Vec::new()),
        base_revision: 0,
        revision: 1,
        tick: 0,
        checksum: 1,
        resources: Some(ResourcesDelta {
            replace: true,
            changed: Vec::new(),
        }),
        buildings: Some(BuildingsDelta {
            replace: true,
            changed: Vec::new(),
            removed: Vec::new(),
        }),
        events: Some(Vec::new()),
        ..empty()
    };

    let boundaries = SnapshotDelta {
        boundaries: Some(vec![Boundary {
            segment: Segment {
                q: -4,
                r: 7,
                chord: 2,
            },
            definition_id: 2,
            open: true,
            paid: vec![Ingredient {
                item_id: 15,
                quantity: 2,
            }],
        }]),
        ..empty()
    };
    // Prepared ground carries a signed elevation beside an unsigned surface id, and the two are
    // encoded differently. A cut cell and a paved cell in the same case is what pins that: swap
    // the two readers and the cut hex comes back as a huge surface id rather than as an error.
    let ground = SnapshotDelta {
        ground: Some(vec![
            GroundCell {
                q: 2,
                r: -3,
                surface: 4,
                elevation: 0,
                erosion: 1,
                paid: vec![Ingredient {
                    item_id: 15,
                    quantity: 1,
                }],
            },
            GroundCell {
                q: -1,
                r: 0,
                surface: 0,
                elevation: -2,
                erosion: 0,
                paid: Vec::new(),
            },
        ]),
        spoil: Some(6),
        ..empty()
    };
    // Departure is signed, like a cut's elevation: a flooded cell and a drained one in the same
    // case is what pins the reader. Swap it for an unsigned varint and the drained hex comes
    // back as a huge positive depth rather than as an error.
    let water = SnapshotDelta {
        water: Some(vec![
            hydrology::WaterCell {
                q: 2,
                r: -3,
                departure: 6,
            },
            hydrology::WaterCell {
                q: -1,
                r: 0,
                departure: -4,
            },
        ]),
        ..empty()
    };

    vec![
        ("boundaries with paid recovery", boundaries),
        ("prepared ground and spoil", ground),
        ("disturbed water", water),
        ("a quiet frame", quiet),
        ("every scalar group", scalars),
        ("both patches with entries", patches),
        ("the empty full replace", replace),
    ]
}

/// The one artifact both languages are pinned to, in the same role
/// `fixtures/hex-directions.json` plays for the direction table.
///
/// Rust asserts it encodes these deltas to exactly these bytes and serializes them to exactly
/// this JSON. `tests/snapshotWire.test.ts` asserts the shipped TypeScript decoder turns those
/// same bytes back into that same JSON. Together they say the binary path delivers what the
/// JSON path delivered, which is the whole claim of the encoding.
///
/// Regenerate with `UPDATE_WIRE_FIXTURE=1 cargo test wire_fixture` and read the diff: a change
/// here is a wire break, and the decoder on the other side has to move with it.
#[test]
fn the_cross_language_fixtures_pin_the_format_and_the_economy() {
    let cases: Vec<serde_json::Value> = wire_fixture_cases()
        .into_iter()
        .map(|(name, delta)| {
            serde_json::json!({
                "name": name,
                "bytes": hex_encode(&wire::encode_delta(&delta)),
                "delta": serde_json::to_value(&delta).unwrap(),
            })
        })
        .collect();
    let generated = serde_json::json!({
        "magic": std::str::from_utf8(&wire::WIRE_MAGIC).unwrap(),
        "version": wire::WIRE_VERSION,
        "kinds": WIRE_KINDS.map(|(_, name)| name).to_vec(),
        "terrain": WIRE_TERRAIN.map(|(_, name)| name).to_vec(),
        "statuses": WIRE_STATUSES.map(|(_, name)| name).to_vec(),
        "cases": cases,
    });

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures/snapshot-delta-wire.json");
    if std::env::var("UPDATE_WIRE_FIXTURE").is_ok() {
        let mut text = serde_json::to_string_pretty(&generated).unwrap();
        text.push('\n');
        std::fs::write(&path, text).unwrap();
    }
    let recorded: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).expect(
        "fixtures/snapshot-delta-wire.json exists — regenerate with UPDATE_WIRE_FIXTURE=1",
    ))
    .unwrap();
    assert_eq!(
        generated, recorded,
        "the wire format moved; the TypeScript decoder has to move with it"
    );

    // The economy's own fixture, in the role `fixtures/hex-directions.json` plays for the
    // direction table and `fixtures/snapshot-delta-wire.json` plays for the wire.
    //
    // Balance was the one system here with no representation: the costs were data, but every
    // figure that decides whether the data works — items per minute, what a generator carries,
    // what a building costs once its inputs are expanded to raw materials — existed nowhere and
    // was checked by nothing. This is that file. Rust computes it from the shipped catalogues and
    // `tests/balance.test.ts` recomputes the cost trees in TypeScript against the same
    // `definitions.json`, so the recorded numbers are pinned by two independent expansions rather
    // than by one implementation agreeing with its own output.
    //
    // Regenerate with `UPDATE_BALANCE_FIXTURE=1 cargo test balance_fixture`, then
    // `npx prettier --write fixtures/balance.json` because serde and prettier disagree about
    // short arrays, and read the diff: a change here is a change to what the game plays like, and
    // it should be one somebody meant. The comparison is over parsed JSON, so the formatting pass
    // cannot change what the test asserts.
    let report = balance::compute();
    let generated = serde_json::to_value(&report).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/balance.json");
    if std::env::var("UPDATE_BALANCE_FIXTURE").is_ok() {
        // The report, not the `Value` built from it: serde orders a `Value`'s keys
        // alphabetically and the struct in declaration order, and only the second keeps a
        // regenerated fixture diffable against the one it replaces.
        let mut text = serde_json::to_string_pretty(&report).unwrap();
        text.push('\n');
        std::fs::write(&path, text).unwrap();
    }
    let recorded: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&path)
            .expect("fixtures/balance.json exists — regenerate with UPDATE_BALANCE_FIXTURE=1"),
    )
    .unwrap();
    assert_eq!(
        generated, recorded,
        "the economy moved; say so in the plan and regenerate the fixture"
    );
}

/// The stated curve, and the proof that stating it is not the same as describing it.
///
/// Two rules, both claims about the data rather than about taste. A tier costs strictly more
/// than the tier it upgrades from, and a machine costs no less than a machine of the same kind
/// whose technology it is unlocked behind. The negative case is the point: price a cutter in one stone and the curve breaks, because a cutter
/// two technologies past a smelter costs less than the smelter.
#[test]
fn the_economy_holds_at_every_step_of_the_curve() {
    let report = balance::compute();
    assert!(!report.curve.is_empty());
    for step in &report.curve {
        assert!(
            step.holds,
            "{} ({}) follows {} ({}) by {} and does not cost more",
            step.building,
            step.effort_milli,
            step.follows,
            step.follows_effort_milli,
            step.relation
        );
    }

    let mut broken: DefinitionsInput = serde_json::from_str(DEFINITIONS).unwrap();
    let technologies: TechnologiesInput = serde_json::from_str(TECHNOLOGIES).unwrap();
    let cutter = broken
        .buildings
        .iter_mut()
        .find(|building| building.key == "cutter")
        .expect("the cutter is in the catalogue");
    cutter.construction_cost = vec![Ingredient {
        item_id: STONE,
        quantity: 1,
    }];
    let broken = balance::compute_from(broken, technologies);
    assert!(
        broken.curve.iter().any(|step| !step.holds),
        "a cheaper-than-its-predecessor building has to fail the curve"
    );

    // The two rates a player compares without being told they are comparing them: their own
    // hands, and the first machine that replaces them.
    //
    // These are measured against the same wall clock, and the order between them is a design
    // decision that has now been made twice in opposite directions. Through v0.16 the hand ran at
    // 300 items a minute against an extractor's 120, so the first automation in the game was two
    // and a half times slower than doing it yourself, which read as a punishment. v0.17 made them
    // equal and v0.23 pinned the hand as never faster.
    //
    // This inverts that on purpose. A tier-one extractor is *half* the hand on the same material
    // and the deep extractor is what draws level. The trade is no longer speed — it is that the
    // machine works while the player is somewhere else, so automation becomes a question of how
    // many you can afford to run and to power rather than of raw rate. The reason the old rule
    // existed still holds and is still guarded: what must never happen is an upgrade that leaves
    // a player slower than their own hands with no way up.
    let report = balance::compute();
    let rate_for = |building: &str, item: &str| -> u64 {
        report
            .machines
            .iter()
            .find(|machine| {
                machine.building == building && machine.output_item.as_deref() == Some(item)
            })
            .map(|machine| machine.per_minute_milli)
            .unwrap_or(0)
    };
    assert!(
        !report.reference.hand_gathers.is_empty(),
        "the hand still takes something"
    );
    for gather in &report.reference.hand_gathers {
        let hand = u64::from(gather.items_per_minute) * 1000;
        let tier_one = rate_for("extractor", &gather.item);
        let deep = rate_for("extractor-ii", &gather.item);
        // Half, within what a whole number of ticks allows: sand and clay want 13.33 ticks and
        // are given 13, which is the only material pair the ladder does not hit exactly.
        assert!(
            tier_one * 100 > hand * 45 && tier_one * 100 < hand * 55,
            "{} tier one at {tier_one} is not half the hand at {hand}",
            gather.item
        );
        // The way up has to exist, or this is the v0.16 punishment again with extra steps.
        assert!(
            deep * 100 > hand * 94,
            "{} deep extractor at {deep} does not reach the hand at {hand}",
            gather.item
        );
    }
    // Crystal is the one material with an extraction rate and no hand rate, and it is the
    // slowest thing dug: twice the ore it shares the highland with.
    assert_eq!(
        rate_for("extractor", "crystal") * 2,
        rate_for("extractor", "ore"),
        "crystal takes twice as long as iron"
    );
    let wood = report
        .reference
        .hand_gathers
        .iter()
        .find(|gather| gather.item == "wood")
        .expect("wood is the fastest hand");
    assert!(
        report
            .reference
            .hand_gathers
            .iter()
            .any(|gather| gather.item == "ore" && gather.items_per_minute < wood.items_per_minute),
        "hard rock has to be slower than wood"
    );
    assert!(
        !report
            .reference
            .hand_gathers
            .iter()
            .any(|gather| gather.item == "crystal"),
        "signal crystal is machine only"
    );
    // Both work the same seven cells: reach is what an upgrade buys, never what a hand grows.
    assert_eq!(report.reference.cells_in_reach.first(), Some(&7));

    // A fuel recipe that hands back more energy than it was given is a perpetual motion machine.
    //
    // Charcoal was exactly that: two wood at two energy each into one charcoal at eight, for a
    // kiln that needs no fuel of its own. Wood regrows, so the char recipe was an unbounded free
    // power source sitting one technology into the tree. Real pyrolysis burns part of the charge
    // to cook the rest and keeps a quarter to a half of the wood's energy — a kiln is bought for
    // **density**, four times the energy in one belt slot, and never for energy itself.
    //
    // So the band is two-sided. Above 1000 the world makes energy from nothing; below 250 the
    // kiln burns more than the worst real one and nobody would run it. Fuel is a property of the
    // item, so this is the one place the round trip can be checked at all — nothing in a recipe
    // row knows what its inputs burn for.
    let report = balance::compute();
    let converted: Vec<_> = report
        .fuel
        .iter()
        .filter(|entry| entry.recipe.is_some())
        .collect();
    assert!(!converted.is_empty(), "some fuel is crafted");
    for entry in converted {
        let gain = entry.gain_milli.unwrap_or(0);
        assert!(
            gain <= 1000,
            "{} returns {} energy for {} — a kiln is not a power source",
            entry.item,
            entry.output_energy,
            entry.input_energy
        );
        assert!(
            gain >= 250,
            "{} returns {} energy for {} — worse than the worst kiln anyone has built",
            entry.item,
            entry.output_energy,
            entry.input_energy
        );
    }

    // Processing has to pay for itself, and the request board is where it is paid.
    //
    // A request that pays no better per gather than a raw one is a request nobody would ever build
    // a machine for: the smelter costs research, construction, power, and fuel, and the hub would
    // be offering the same rate for two ore as for the plate they became. So every row whose item
    // comes out of a recipe has to beat every row whose item comes out of the ground — measured
    // through the whole tree, fuel included, which is the only comparison that is not a guess.
    let report = balance::compute();
    let raw: Vec<_> = report
        .requests
        .iter()
        .filter(|request| request.machine_ticks == 0)
        .collect();
    let processed: Vec<_> = report
        .requests
        .iter()
        .filter(|request| request.machine_ticks > 0)
        .collect();
    assert!(raw.len() >= 7, "the eight opening materials, less water");
    assert!(processed.len() >= 10, "a ladder, not one processed row");
    let best_raw = raw
        .iter()
        .map(|request| request.insight_per_gather_milli)
        .max()
        .expect("a raw request");
    for request in processed {
        assert!(
            request.insight_per_gather_milli > best_raw,
            "{} pays {} insight per thousand gathers and the best raw row pays {} — nobody \
                 would build the machine",
            request.request,
            request.insight_per_gather_milli,
            best_raw
        );
    }

    // The finite catalogue pays for the whole purchasable tree, with room to spare.
    //
    // This is the safeguard that makes finite demand shippable at all. Once a project retires
    // there is no tap to fall back on, so if the catalogue ever priced below the tree a player
    // could deliver every commission the hub will ever make and still be locked out of research
    // with nothing left to sell. The surplus is deliberate: it has to be possible to spend on the
    // wrong branch first and still finish, or the "choice" of what to research is a quiz.
    let report = balance::compute();
    let budget = &report.budget;
    assert!(
        budget.project_insight >= budget.research_cost,
        "{} projects pay {} against {} of research",
        budget.projects,
        budget.project_insight,
        budget.research_cost
    );
    assert!(
        budget.surplus_ratio_milli >= 1250,
        "the catalogue pays {} for {} of research — {}x, and a wrong purchase order strands \
             the player",
        budget.project_insight,
        budget.research_cost,
        budget.surplus_ratio_milli as f64 / 1000.0
    );
    // The grants are not bought, and counting them as cost would flatter the ratio above.
    assert!(
        !budget.granted_technologies.is_empty(),
        "the contract stages still hand technologies over"
    );

    // Every raw project the hub will ever post, added up, is less than the technology tree.
    //
    // Raw rows are the bootstrap and nothing more. Because each pays exactly once, this sum *is*
    // the entire lifetime income of hand-gathering, so the assertion is now a hard floor rather
    // than a rate comparison: a player who never builds a machine cannot finish the tree, and the
    // processed rows are the only way across.
    let report = balance::compute();
    let tree: u32 = {
        let technologies: TechnologiesInput = serde_json::from_str(TECHNOLOGIES).unwrap();
        technologies
            .technologies
            .iter()
            .map(|technology| technology.cost)
            .sum()
    };
    let raw_cycle: u32 = report
        .requests
        .iter()
        .filter(|request| request.machine_ticks == 0)
        .map(|request| request.insight)
        .sum();
    assert!(
        raw_cycle < tree,
        "one cycle of raw requests pays {raw_cycle} and the tree costs {tree}"
    );
    assert!(tree >= 113, "the tree grew, it must not have shrunk");

    // Every material the economy bottoms out in can actually be had, from the site the game
    // starts you on, under the preset it starts you in.
    //
    // Two separate questions, and the second is the one that bites: stone is generated on cliffs
    // that nothing can stand on, so "the world holds some" and "you can reach some" are different
    // claims and only the second one makes it a material rather than scenery.
    let report = balance::compute();
    assert!(report.access.len() >= 9, "eight fields and water");
    for material in &report.access {
        if material.material == "sand" || material.material == "crystal" {
            // Sand is the regional ocean and crystal is the reason to leave. Neither is
            // guaranteed, and a 96-hex sample of a 512-hex landform often never reaches them.
            continue;
        }
        assert!(
            material.reachable,
            "{} is required by {} rows and nothing can reach any of it",
            material.material, material.required_by
        );
        assert!(
            material.nearest_generated.is_some(),
            "{} is required by {} rows and the default world generates none",
            material.material,
            material.required_by
        );
        // A guaranteed material is guaranteed as a *patch*, not as a cell: the clearing holds
        // nothing now, so a promise that could be kept with one hex would be no promise.
        assert!(
            material.guaranteed_walk.is_none() || material.guaranteed_hexes >= WORKABLE_PATCH_HEXES,
            "{} is guaranteed as {} hexes, which no extractor can fill from",
            material.material,
            material.guaranteed_hexes
        );
    }
    // Water is nobody's field: a pump makes it out of terrain, so it is the one raw material
    // the opening does not guarantee and it still has to be within reach of the landing site.
    let water = report
        .access
        .iter()
        .find(|material| material.material == "water")
        .expect("water is a raw material");
    assert_eq!(water.guaranteed_walk, None);
    let first_pump_ceiling = BOOTSTRAP_GUARANTEES
        .iter()
        .find(|&&(item_id, _, _)| item_id == CLAY)
        .map(|&(_, _, ceiling)| ceiling as u32)
        .unwrap();
    assert!(water.nearest_generated.unwrap_or(u32::MAX) <= first_pump_ceiling);

    // The founding contract has to be a founding *project*, and that is a claim about its bill.
    //
    // Three components prove one chain out of one landscape, which was the whole of v0.13's
    // objective and is deliberately only the first stage now. What the milestone asserts is that
    // the project the hub actually builds cannot be paid for out of that chain: it needs more than
    // one raw material, it costs strictly more, and — like every powered machine in this game — it
    // cannot be run at all without an On-site Power branch nothing else forces.
    let report = balance::compute();
    let founding: Vec<_> = report
        .contracts
        .iter()
        .filter(|stage| stage.scenario == "new-game")
        .collect();
    assert!(
        founding.len() >= 2,
        "a single-stage contract is the old objective wearing a new name"
    );
    let first = founding.first().expect("a first stage");
    let last = founding.last().expect("a last stage");
    assert!(
        last.raw_materials >= 2,
        "the founding project bottoms out in {} raw material(s); a project that needs one \
             landscape is a longer version of the opening",
        last.raw_materials
    );
    assert!(
        last.opening.gather_total > first.opening.gather_total,
        "the project must cost more than the beat that proves the line"
    );
    for stage in &founding {
        let needs_power = stage.opening.buildings.iter().any(|key| {
            let (definitions, _, _) = catalogs();
            definitions
                .buildings
                .iter()
                .any(|building| building.key == *key && building.power_draw.unwrap_or(0) > 0)
        });
        assert!(
            !needs_power
                || stage
                    .opening
                    .technologies
                    .iter()
                    .any(|key| key == "on-site-power"),
            "{} is payable without power, so the guidance may lead somewhere the rules refuse",
            stage.stage
        );
    }
    assert!(first.opening.technologies.is_empty());
    assert!(first.opening.player_work_ticks > 0);

    // An opening that needs a machine the rules will not run is not an opening.
    //
    // `power_progress` returns zero off a network, so a plan naming a smelter and no generator is
    // a plan for a factory that stands still. This is the same defect the scripted next action
    // had, asserted here against the numbers rather than against the sentence.
    let report = balance::compute();
    let definitions: DefinitionsInput =
        serde_json::from_str(include_str!("../../../src/data/definitions.json")).unwrap();
    let building = |key: &str| {
        definitions
            .buildings
            .iter()
            .find(|building| building.key == key)
            .expect("opening names a shipped building")
    };
    let openings = report
        .openings
        .iter()
        .chain(report.contracts.iter().map(|stage| &stage.opening));
    for opening in openings {
        let draws = opening
            .buildings
            .iter()
            .any(|key| building(key).power_draw.unwrap_or(0) > 0);
        if !draws {
            continue;
        }
        assert!(
            opening
                .buildings
                .iter()
                .any(|key| building(key).power_output.unwrap_or(0) > 0),
            "{} draws power and generates none",
            opening.name
        );
    }

    // A generator whose upkeep eats its own output is not a generator.
    //
    // A boiler drinks one water every tick it runs and a turbine is dead without one beside it,
    // so the pumps are part of the plant whether or not the definition file says so. Through
    // v0.16 the pump made one water every six ticks, which is six pumps drawing 24 of the
    // turbine's 48 before a single machine ran — leaving the mid-game workhorse behind a hydro
    // generator that cost exactly the same and needed neither fuel nor plumbing.
    let report = balance::compute();
    for plant in &report.power {
        assert!(
            plant.net_output > 0,
            "{} produces {} and spends {} keeping itself fed",
            plant.building,
            plant.output,
            plant.upkeep_draw
        );
    }
    // The one that burns fuel and drinks water is the one that carries the most, or the cost
    // of running it buys nothing.
    let best_free = report
        .power
        .iter()
        .filter(|plant| plant.fuel_energy_per_tick == 0)
        .map(|plant| plant.net_output)
        .max()
        .unwrap_or(0);
    let steam = report
        .power
        .iter()
        .find(|plant| plant.source == "turbine")
        .expect("steam is in the catalogue");
    assert!(
        steam.net_output > best_free,
        "steam nets {} against {} for a generator that needs nothing",
        steam.net_output,
        best_free
    );

    // Research is funded out of distinct projects, each counted once at its posted price.
    //
    // A funding line used to name one request and grind it, so the harness quoted a length that
    // no longer exists: the hub buys a given bill exactly once. An opening's cost is now a
    // shopping list — whole projects, cheapest per item first, until the insight is raised — and
    // this recomputes that list from the catalogue rather than trusting the field.
    let report = balance::compute();
    let (definitions, _, _) = catalogs();
    let mut multi = 0;
    let openings = report
        .openings
        .iter()
        .chain(report.contracts.iter().map(|stage| &stage.opening));
    for opening in openings {
        if opening.insight == 0 {
            assert_eq!(
                opening.insight_items, 0,
                "{} pays for nothing",
                opening.name
            );
            assert!(opening.insight_projects.is_empty());
            continue;
        }
        let named: Vec<_> = opening
            .insight_projects
            .iter()
            .map(|key| {
                definitions
                    .requests
                    .iter()
                    .find(|request| &request.key == key)
                    .expect("a funding line names a standing project")
            })
            .collect();
        let unique: BTreeSet<_> = opening.insight_projects.iter().collect();
        assert_eq!(
            unique.len(),
            opening.insight_projects.len(),
            "{} funds itself by delivering the same project twice",
            opening.name
        );
        let raised: u32 = named.iter().map(|request| request.insight).sum();
        assert!(
            raised >= opening.insight,
            "{} raises {} against a bill of {}",
            opening.name,
            raised,
            opening.insight
        );
        // Whole projects, so the list must not carry a row it did not need to reach the bill.
        let without = raised - named.last().expect("a funded line names one").insight;
        assert!(
            without < opening.insight,
            "{} lists a project it does not need",
            opening.name
        );
        assert_eq!(
            opening.insight_items,
            named.iter().map(|request| request.quantity).sum::<u32>(),
            "{} funds {} insight off {:?}",
            opening.name,
            opening.insight,
            opening.insight_projects
        );
        if named.len() > 1 {
            multi += 1;
        }
    }
    assert!(
        multi > 0,
        "no opening needs a second project, so the finite count is not being measured"
    );

    // A technology the hub grants for finishing a commission is not free, and an opening that needs
    // one has to deliver the commission first.
    //
    // Four technologies cannot be bought at any price: the contract stage hands them over. Pricing
    // them at their `insight` cost of zero told the harness the early stations were unlocked from a
    // standing start, which skipped the delivery every real opening makes before it places one.
    // The resolver now folds the owed stage's bill into the opening it blocks — and a stage does
    // not commission itself, or nothing would ever resolve.
    let report = balance::compute();
    let (_, technologies, _) = catalogs();
    let granted: BTreeSet<&str> = technologies
        .technologies
        .iter()
        .filter_map(|technology| match &technology.grant {
            TechnologyGrant::ContractStage { key, .. } => Some(key.as_str()),
            TechnologyGrant::Purchase => None,
        })
        .collect();
    assert!(
        !granted.is_empty(),
        "the founding commission grants technologies, or this measures nothing"
    );
    for stage in &report.contracts {
        assert!(
            !stage.opening.commissions.contains(&stage.stage),
            "{} commissions itself",
            stage.stage
        );
    }
    let openings = report
        .openings
        .iter()
        .chain(report.contracts.iter().map(|stage| &stage.opening));
    let mut owed = 0;
    for opening in openings {
        for key in &opening.commissions {
            assert!(
                granted.contains(key.as_str()),
                "{} waits on {key}, which grants nothing",
                opening.name
            );
            owed += 1;
        }
    }
    assert!(
        owed > 0,
        "no opening waits on a commission, so the grant is still being priced at zero"
    );
}

#[test]
fn deltas_send_only_what_changed_and_match_a_full_snapshot_diff() {
    let mut core = game("new-game");
    let previous = core.snapshot();
    core.tick_many(1);
    let current = core.snapshot();
    let delta = SnapshotDelta::between(7, 8, &previous, &current);
    assert_eq!(delta.base_revision, 7);
    assert_eq!(delta.revision, 8);
    assert_eq!(delta.tick, 1);
    assert!(delta.terrain.is_none());
    assert!(delta.resources.is_none());
    assert!(delta.buildings.is_none());
    assert!(delta.events.is_some());
    let json = serde_json::to_string(&delta).unwrap();
    assert!(!json.contains("\"terrain\""));
    assert!(!json.contains("\"resources\""));
    assert!(!json.contains("\"buildings\""));

    // Generated chunk bounds report the surveyed world area.
    let mut core = game("new-game");
    let snapshot = core.snapshot();
    let size = core.scenario.chunk_size;
    assert!(!snapshot.chunks.is_empty());
    for chunk in &snapshot.chunks {
        let (x, y, span) = chunk_world_bounds(chunk.chunk_q, chunk.chunk_r, size);
        assert_eq!(chunk.x, x);
        assert_eq!(chunk.y, y);
        assert_eq!(chunk.span, span);
    }
    let contains = |chunk: &ChunkSnapshot, x: i32, y: i32| {
        (chunk.x..chunk.x + chunk.span).contains(&x) && (chunk.y..chunk.y + chunk.span).contains(&y)
    };
    // The player always stands inside surveyed world.
    assert!(snapshot
        .chunks
        .iter()
        .any(|chunk| contains(chunk, core.player.x, core.player.y)));
    // Distant world stays unreported, which is what the host renders as fog.
    let (far_q, far_r) = (size * 4, size * 4);
    let (far_x, far_y) = axial_world(far_q, far_r);
    assert!(!snapshot
        .chunks
        .iter()
        .any(|chunk| contains(chunk, far_x, far_y)));

    // Travelling there surveys it, so the fogged area shrinks as the player explores.
    core.ensure_neighborhood(far_x, far_y);
    let explored = core.snapshot();
    assert!(explored.chunks.len() > snapshot.chunks.len());
    assert!(explored
        .chunks
        .iter()
        .any(|chunk| contains(chunk, far_x, far_y)));

    // Buildings delta sends only the entities that changed.
    let mut core = game("new-game");
    core.researched.insert(2);
    stock_for(&mut core, 1, 1);
    set_player_hex(&mut core, 3, 1);
    core.place(3, 0, 1, 0, None).unwrap();
    add_test_belt(&mut core, 4, 1, 0);
    core.compile_graph();

    // One tick advances only the extractor's progress; the hub and the belt are untouched.
    let previous = core.snapshot();
    core.tick_many(1);
    let current = core.snapshot();
    let patch = buildings_delta(&previous.buildings, &current.buildings).unwrap();
    assert!(!patch.replace);
    assert!(patch.removed.is_empty());
    assert_eq!(
        patch
            .changed
            .iter()
            .map(|entity| entity.id)
            .collect::<Vec<_>>(),
        vec![2]
    );
    assert!(current.buildings.len() > patch.changed.len());
    let json = serde_json::to_string(&SnapshotDelta::between(0, 1, &previous, &current)).unwrap();
    assert!(json.len() < serde_json::to_string(&current.buildings).unwrap().len());

    // Erasing reports the id instead of resending every surviving entity.
    let previous = current;
    core.erase(3, 0).unwrap();
    let current = core.snapshot();
    let patch = buildings_delta(&previous.buildings, &current.buildings).unwrap();
    assert_eq!(patch.removed, vec![2]);
    assert!(patch.changed.is_empty());

    // A full delta stays a complete replacement, so a host with no prior state is correct.
    let full = SnapshotDelta::full(0, 1, &current).buildings.unwrap();
    assert!(full.replace);
    assert_eq!(full.changed, current.buildings);

    // The shipped delta is built from marks made where state is mutated, not by diffing two
    // complete snapshots, so a missed mark would silently strand the host on stale state. This
    // pins the builder against the full diff it replaces, step by step, across every path that
    // touches a snapshot group: quiet frames, ticks, gathering to depletion, hub delivery,
    // research, placement, rotation, erasure, and travel into unsurveyed world.
    let mut factory = test_factory("new-game");
    // Setup pokes happen before the baseline is taken, so the checked run only exercises real
    // native paths. Shrinking a guaranteed deposit lets the run reach depletion. The starting
    // pack stays inside the carrying rule, or the gathering steps below would be refused.
    factory.core.player.inventory.insert(1, 40);
    factory.core.player.inventory.insert(2, 3);
    factory.core.player.inventory.insert(3, 20);
    factory.core.player.inventory.insert(6, 8);
    set_player_hex(&mut factory.core, 4, -2);
    factory.core.write_overlay(4, -2, 1, 2, 36);
    // The clearing generates nothing since v0.21, so the deposit the extractor further down
    // stands on is written here rather than found. Same reasoning as `TEST_FIELD`: this is a
    // test about which marks a delta carries, not about where a generator puts iron.
    factory.core.write_overlay(6, 0, 1, 48, 48);
    let surveyed_at_start = factory.core.generated_chunks.len();

    // Establish the baseline exactly as the worker does on its first frame.
    let _ = factory.snapshot_json();
    let mut previous = factory.core.snapshot();
    let mut check = |factory: &mut Factory, step: &str| {
        assert_delta_matches_full_diff(factory, &mut previous, step);
    };

    factory.core.advance("[]", 0, 0).unwrap();
    check(&mut factory, "an empty frame");
    factory.core.advance(IDLE, 1, 1).unwrap();
    check(&mut factory, "one idle tick");

    // Gathering, through the frame the deposit runs dry and one rejected attempt after it.
    // The cooldown between attempts is paid in player steps, because that is the clock the
    // player's own actions run on — the factory ticks here only exercise the tick paths.
    for round in 0..3 {
        factory
            .core
            .advance(r#"[{"type":"gather"}]"#, 2, 60)
            .unwrap();
        check(&mut factory, &format!("gather attempt {round}"));
    }
    assert_eq!(factory.core.deposit_quantity((4, -2)), 0);

    // Delivery and research: insight, delivered totals, the objective, and unlocks.
    set_player_hex(&mut factory.core, 1, 0);
    check(&mut factory, "walking to the landing hub");
    factory
        .core
        .advance(r#"[{"type":"deposit"}]"#, 1, 0)
        .unwrap();
    check(&mut factory, "delivering inventory to the hub");
    // Prove the line grants the four starter technologies; Composition is still an insight
    // purchase. Insight is compared against the baseline rather than marked, so a direct
    // change is exactly what the host would see from any native path that moves it.
    assert_eq!(factory.core.researched.len(), 4);
    factory.core.insight += 8;
    check(&mut factory, "funding the research");
    factory
        .core
        .advance(r#"[{"type":"research","technology_id":3}]"#, 1, 0)
        .unwrap();
    check(&mut factory, "researching composition");
    assert_eq!(factory.core.researched.len(), 5);

    // Player state is compared against the baseline rather than marked, so restocking directly
    // is exactly what the host would see from any native path that changes inventory.
    // Kept inside the carrying rule, so the erase further down still has somewhere to refund to.
    stock_for(&mut factory.core, 1, 1);
    stock_for(&mut factory.core, 3, 1);
    factory.core.player.inventory.insert(24, 8);
    check(&mut factory, "restocking the player");

    // Construction: inserted entities, recompiled transport, and per-chunk entity counts.
    // The build site stands off the hub's seven hexes: the composer's three would otherwise
    // reach into them. The line still runs west and the composer still hands into the hub's
    // eastern rim; only the empty ground between the machines moved.
    set_player_hex(&mut factory.core, 4, 2);
    check(&mut factory, "walking to the build site");
    factory.core.place(6, 0, 1, 3, None).unwrap();
    check(&mut factory, "placing an extractor");
    factory.core.place(4, 0, 2, 3, None).unwrap();
    check(&mut factory, "placing a belt");
    factory.core.place(3, 0, 3, 3, Some(1)).unwrap();
    check(&mut factory, "placing a composer");

    // The factory running: machine progress, cargo transfer, hub deliveries, and victory.
    for round in 0..8 {
        factory.core.advance(IDLE, 20, 0).unwrap();
        check(&mut factory, &format!("running the factory, round {round}"));
    }
    assert!(factory.core.delivered > 0, "the scripted run must produce");

    // Edits against a live blueprint, including orientations that split and rejoin components.
    for turn in 0..6 {
        factory.core.rotate(4, 0, false).unwrap();
        check(&mut factory, &format!("rotating a belt, turn {turn}"));
    }
    factory.core.erase(4, 0).unwrap();
    check(&mut factory, "erasing a belt");
    factory.core.advance(IDLE, 5, 0).unwrap();
    check(&mut factory, "ticking with the belt gone");
    factory.core.place(4, 0, 2, 3, None).unwrap();
    check(&mut factory, "replacing the belt");

    // Cutting flora and letting it grow back. Regrowth is the one thing that changes a deposit
    // without an extractor or a player touching it that frame, so it has to mark what it moved.
    set_player_hex(&mut factory.core, -3, 1);
    check(&mut factory, "walking to the flora");
    factory
        .core
        .advance(r#"[{"type":"gather"}]"#, 1, GATHER_COOLDOWN_STEPS)
        .unwrap();
    check(&mut factory, "cutting flora");
    let regrowth = factory
        .core
        .item_definition(WOOD)
        .unwrap()
        .regrowth_ticks
        .expect("wood regrows");
    factory.core.advance(IDLE, regrowth, 0).unwrap();
    check(&mut factory, "flora growing back");
    assert!(
        factory.core.flora_regrowth.is_empty(),
        "the cut cell must have grown back inside its own cadence"
    );

    // Travel into unsurveyed world: terrain, deposits, chunk bounds, and every extractor's
    // resolved deposit reference at once. The neighborhood generator is the same one walking
    // uses; a far hex is used so derived water or cliffs cannot stall the survey.
    for (label, (q, r)) in [("east", (24, 0)), ("south", (24, 16))] {
        set_player_hex(&mut factory.core, q, r);
        factory.core.advance(IDLE, 1, 1).unwrap();
        check(
            &mut factory,
            &format!("travelling {label} into unsurveyed world"),
        );
    }
    factory.core.advance(IDLE, 1, 1).unwrap();
    check(&mut factory, "standing still again");
    assert!(
        factory.core.generated_chunks.len() > surveyed_at_start,
        "the scripted run must survey new world"
    );

    // A load replaces the core the baseline described, so the host is sent a complete
    // replacement rather than a patch against state that no longer exists.
    let save = factory.core.save_string().unwrap();
    factory.load_string(&save).unwrap();
    let delta = factory.build_delta();
    assert!(
        delta
            .buildings
            .expect("full delta carries buildings")
            .replace
    );
    assert!(
        delta
            .resources
            .expect("full delta carries resources")
            .replace
    );
    assert!(delta.terrain.is_some());
    assert!(delta.chunks.is_some());
    assert!(delta.player.is_some());

    // World generation invalidates resolved deposit references, so it must invalidate the entity
    // snapshots derived from them in the same breath. Today's deposit radii are smaller than the
    // tile spacing, so a generated deposit does not in fact reach an existing extractor and the
    // scripted equivalence run cannot observe this — which is exactly why the coupling is pinned
    // here directly rather than left to depend on that geometry holding.
    let mut core = game("new-game");
    core.researched.insert(2);
    stock_for(&mut core, 1, 1);
    set_player_hex(&mut core, 3, 1);
    core.place(3, 0, 1, 0, None).unwrap();
    let index = core.entity_at(3, 0).unwrap();
    core.extractor_deposit(index);
    assert_eq!(core.deposit_links.len(), 1);

    core.dirty = SnapshotDirty::default();
    core.generate_chunk(-9, 7);

    assert!(core.deposit_links.is_empty(), "references are re-resolved");
    let marked: Vec<u32> = core.entities.iter().map(|entity| entity.id).collect();
    assert_eq!(
        drain_marks(&mut core.dirty.entities),
        marked,
        "every entity snapshot derived from a deposit is suspect too"
    );
    assert!(core.dirty.chunks, "the surveyed chunk set grew");

    // An extractor's reported status is resolved through its cached deposit reference instead of
    // a scan over every generated tile. The two must agree exactly, including after the deposit
    // under it runs dry.
    let mut core = game("new-game");
    core.researched.extend([1, 2]);
    stock_for(&mut core, 1, 1);
    set_player_hex(&mut core, 3, 1);
    core.place(3, 0, 1, 0, None).unwrap();
    let index = core.entity_at(3, 0).unwrap();

    let scanned = |core: &Core| {
        let (x, y) = axial_world(core.entities[index].placed.q, core.entities[index].placed.r);
        core.resource_at_world(x, y)
            .map(|key| core.deposit_quantity(key))
            .unwrap_or(0)
            > 0
    };

    for _ in 0..3 {
        let expected = scanned(&core);
        assert_eq!(core.extractor_deposit(index).is_some(), expected);
        assert_eq!(
            core.status_of(index, expected, true, true, false),
            core.entity_snapshot(index).status
        );
        core.tick_many(20);
    }

    // Draining the field must flip both the scan and the cached reference together.
    core.write_overlay(3, 0, 1, 0, 48);
    assert!(!scanned(&core));
    assert!(core.extractor_deposit(index).is_none());
    core.entities[index].cargo = None;
    assert_eq!(
        core.entity_snapshot(index).status,
        EntityStatus::DepositDepleted
    );

    // Combined advance preserves command events through native ticks.
    let mut core = game("new-game");
    core.player.inventory.insert(1, 8);
    core.player.inventory.insert(3, 4);
    set_player_hex(&mut core, 1, 0);
    core.advance(r#"[{"type":"deposit"}]"#, 1, 0).unwrap();
    assert_eq!(core.tick, 1);
    // Eight ore, because the opening board asks for ore and nobody has asked for crystal yet.
    assert!(core
        .events
        .iter()
        .any(|event| event.contains("Delivered 8 to the landing hub")));
    assert_eq!(core.player.inventory.get(&3), Some(&4));

    // Malformed technology graphs and locked forged commands are rejected.
    let (definitions, mut technologies, scenarios) = catalogs();
    technologies.technologies[1].prerequisites = vec![3];
    assert!(validate_technologies(&definitions, &technologies).is_err());
    let mut core = game("new-game");
    core.player.inventory.insert(1, 100);
    core.apply_commands(r#"[{"type":"place","q":2,"r":0,"definition_id":2,"orientation":0}]"#)
        .unwrap();
    assert!(core.entities.iter().all(|entity| entity.placed.q != 2));
    assert!(core.events[0].contains("locked"));
    assert!(validate_scenarios(&definitions, &catalogs().1, &scenarios).is_ok());

    // Progression registries reject missing duplicate and unknown references.
    let (definitions, technologies, _) = catalogs();
    for change in 0..9 {
        let mut invalid = technologies.clone();
        match change {
            0 => invalid.branches.clear(),
            1 => invalid.stages.push(invalid.stages[0].clone()),
            2 => invalid.branches[0].key = "Bad key".into(),
            3 => invalid.stages[0].name = " ".into(),
            4 => invalid.technologies[0].branch = "missing".into(),
            5 => invalid.technologies[0].stage = "missing".into(),
            6 => invalid.technologies[1].key = invalid.technologies[0].key.clone(),
            7 => invalid.technologies[1].prerequisites = vec![1, 1],
            _ => invalid.branches = vec![invalid.branches[0].clone(); 65],
        }
        assert!(
            validate_technologies(&definitions, &invalid).is_err(),
            "case {change}"
        );
    }
}

/// A belt at a vertex heading routes two rows, and the hexes it spans stay free. This is the
/// whole answer to north-south transport: a direction-table row, resolved by the ray-cast the
/// graph compiler already was, with no sub-hex occupancy anywhere.
#[test]
fn rotation_and_the_two_row_reach_are_priced_gated_and_angular() {
    let mut core = game("new-game");
    core.researched.extend([1, 4, 11]);
    stock_for(&mut core, 4, 1);
    core.player.inventory.insert(24, 40);

    // A belt at (0, 3) facing north reaches (1, 1) — the same world column, two rows up.
    set_player_hex(&mut core, 1, 2);
    core.place(1, 1, 4, 0, None).unwrap();
    set_player_hex(&mut core, 1, 3);
    core.place(0, 3, 2, NORTH, None).unwrap();

    let belt = core.entity_at(0, 3).unwrap();
    let container = core.entity_at(1, 1).unwrap();
    assert_eq!(
        core.graph[belt],
        Links::single(Some(container)),
        "a north-facing belt must bind to what sits two rows above it"
    );
    // The seam it spans is two ordinary hexes, and neither is occupied by anything.
    assert_eq!(core.entity_at(0, 2), None);
    assert_eq!(core.entity_at(1, 2), None);
    // So they stay buildable, and the belt never claims them for collision either.
    assert!(core.placement_legality(0, 2, 2, 0, None, true).is_ok());
    assert!(!core.building_definition(2).unwrap().blocks_movement);
    // It occupies exactly one hex.
    assert_eq!(core.entity_footprint(&core.entities[belt]).len(), 1);

    // Rotation on the any axis walks all twelve headings once each, in angular order.
    //
    // The point of a single belt definition is that `R` nudges a heading by 30°, not that it
    // cycles a table. So this checks the *world vectors*, not the indices: consecutive headings
    // turn one twelfth of a circle clockwise, and twelve presses return to where they started.
    let mut core = game("new-game");
    core.researched.extend([1, 11]);
    core.player.inventory.insert(24, 40);
    set_player_hex(&mut core, 1, 3);
    core.place(0, 3, 2, 0, None).unwrap();

    let heading = |core: &Core| {
        core.entities[core.entity_at(0, 3).unwrap()]
            .placed
            .orientation
    };
    // Pointy-top axial, at unit size: a hex at (q, r) sits at `x = √3·(q + r/2)`, `y = 1.5·r`,
    // with `y` running south. The world angle of a heading is the angle of the vector it moves
    // along, growing clockwise from due east.
    let angle = |orientation: u8| {
        let (dq, dr) = TRANSPORT_DIRECTIONS[usize::from(orientation)];
        let (dq, dr) = (f64::from(dq), f64::from(dr));
        (1.5 * dr).atan2(3f64.sqrt() * (dq + dr / 2.0))
    };

    let mut seen = vec![heading(&core)];
    for _ in 0..11 {
        core.rotate(0, 3, false).unwrap();
        let now = heading(&core);
        let step = (angle(now) - angle(*seen.last().unwrap())).rem_euclid(std::f64::consts::TAU);
        assert!(
            (step - std::f64::consts::TAU / 12.0).abs() < 1e-9,
            "one press turned {step} radians, not 30°"
        );
        seen.push(now);
    }
    seen.sort_unstable();
    assert_eq!(seen, (0..12).collect::<Vec<u8>>(), "every heading, once");

    core.rotate(0, 3, false).unwrap();
    assert_eq!(heading(&core), 0, "twelve presses return to the start");
    core.rotate(0, 3, true).unwrap();
    assert_eq!(
        heading(&core),
        7,
        "and reverse rotation is the inverse press: 30° back from due east"
    );

    // Rotation offers a heading on the same terms `place` does: researched, and paid for.
    //
    // A belt bought at an edge heading and turned onto a vertex one would otherwise be the two-row
    // reach at the price of the short step — the exact dominance `corner_construction_cost` exists
    // to prevent — and `R` pressed before the research would hand it over for nothing at all.
    let mut core = game("new-game");
    core.researched.insert(1);
    core.player.inventory.insert(24, 8);
    set_player_hex(&mut core, 1, 3);
    core.place(0, 3, 2, 0, None).unwrap();

    let heading = |core: &Core| {
        core.entities[core.entity_at(0, 3).unwrap()]
            .placed
            .orientation
    };
    let kits = |core: &Core| core.player.inventory.get(&24).copied().unwrap_or(0);
    let paid = kits(&core);

    // Unresearched, `R` walks the six edges and steps straight over the vertex headings between
    // them, so the reach is not something a key the player already has can reach.
    for expected in 1..=5u8 {
        core.rotate(0, 3, false).unwrap();
        assert_eq!(heading(&core), expected);
    }
    core.rotate(0, 3, false).unwrap();
    assert_eq!(heading(&core), 0, "six presses close the edge ring");
    assert_eq!(kits(&core), paid, "and none of them cost anything");

    core.researched.insert(11);
    core.rotate(0, 3, false).unwrap();
    assert_eq!(
        heading(&core),
        NORTH + 2,
        "researched, the vertex heading is the very next one"
    );
    assert_eq!(
        kits(&core),
        paid - 1,
        "and turning onto it is charged the difference"
    );
    core.rotate(0, 3, true).unwrap();
    assert_eq!(heading(&core), 0);
    assert_eq!(
        kits(&core),
        paid,
        "turning back off it returns that difference"
    );

    // The difference is a real price, so a pack that cannot cover it is refused — and the belt
    // is left facing where it was rather than turned half way onto a heading nobody paid for.
    core.player.inventory.remove(&24);
    assert!(core.rotate(0, 3, false).unwrap_err().contains("need"));
    assert_eq!(heading(&core), 0);

    // Orientation is an axis the definition owns, and on the any axis that axis prices and gates
    // itself. The two-row reach costs what it covers and waits behind its own research, which is
    // what lets a belt and a riser be one building without the reach being free.
    let mut core = game("new-game");
    core.researched.extend([1]);
    core.player.inventory.insert(24, 40);
    set_player_hex(&mut core, 1, 3);

    // The belt's own unlock is done, so what refuses a vertex heading is the corner gate alone.
    assert!(core.placement_legality(0, 3, 2, 0, None, true).is_ok());
    assert!(core
        .placement_legality(0, 3, 2, NORTH, None, true)
        .unwrap_err()
        .contains("locked"));
    core.researched.insert(11);
    assert!(core.placement_legality(0, 3, 2, NORTH, None, true).is_ok());

    // An edge-only definition still refuses the vertex headings outright, by range.
    assert!(core
        .placement_legality(0, 3, 4, NORTH, None, true)
        .unwrap_err()
        .contains("oriented in 0..6"));

    // And the price is a data row, not a mechanism: the two-row heading simply costs more.
    let belt = core.building_definition(2).unwrap();
    let edge = belt.cost_at(0).to_vec();
    let corner = belt.cost_at(NORTH).to_vec();
    assert_ne!(edge, corner, "the reach a corner buys is not free");
    assert_eq!(
        corner.iter().map(|cost| cost.quantity).sum::<u32>(),
        edge.iter().map(|cost| cost.quantity).sum::<u32>() * 2,
        "a corner belt costs twice the belt, the way the riser's own row used to say"
    );

    // No definition needs a multi-cell corner footprint yet, so that untested combination is
    // still refused at load — for anything that may face a corner, not only for corner-only.
    let (mut definitions, _, _) = catalogs();
    let index = definitions
        .buildings
        .iter()
        .position(|building| building.id == 2)
        .unwrap();
    definitions.buildings[index]
        .footprint
        .push(Coordinate { q: 1, r: 0 });
    assert!(validate_definitions(&definitions)
        .unwrap_err()
        .contains("two-row period"));

    // And an any-axis definition that gates none of its headings is refused too, which is what
    // keeps the reach a research step rather than a property of the first belt of the game.
    let (mut definitions, _, _) = catalogs();
    let index = definitions
        .buildings
        .iter()
        .position(|building| building.id == 2)
        .unwrap();
    definitions.buildings[index].corner_technology_id = None;
    assert!(validate_definitions(&definitions)
        .unwrap_err()
        .contains("gates none of them"));
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
fn capacity_workload_is_deterministic_and_actually_produces() {
    let spec = capacity::quick_tiers()[1];
    let mut first = capacity::warm_core(&spec);
    let mut second = capacity::warm_core(&spec);
    first.advance_ticks(120);
    second.advance_ticks(120);
    assert_eq!(first.checksum(), second.checksum());
    // Pinned so a change to definitions, the workload, or the simulation cannot silently
    // invalidate comparisons against previously recorded tier numbers. A generator-version
    // bump moves this number while the workload does not — as did v0.14 adding the splitter's
    // and merger's arbitration cursors to `checksum` — which is why the delivered total and
    // the entity count below are the assertions that say the run is the same run.
    //
    // 841_205_484 → 3_799_495_709 when sand left the ocean gate and sat on the shore band.
    // The workload's shape, entity count, and delivered total did not move.
    //
    // 3_799_495_709 → 2_222_187_037 when a belt began holding what it was handed until the tick
    // after — see `just_received`. The workload's shape and entity count did not move; a line's
    // cargo now spends a tick on each belt it crosses instead of the whole line in one, so this
    // window's delivered total is the one below.
    //
    // 2_222_187_037 → 3_614_679_184 when project progress moved off the board slot and onto the
    // project, so `checksum` hashes `request_delivered` instead of a per-slot count. The
    // workload's shape, entity count, and delivered total did not move.
    //
    // 3_614_679_184 → 23_080_823 when limestone entered the site table and world generator 9
    // entered the checksum. The workload's shape, entity count, and delivered total did not move.
    // Petroleum roads adds the oil site rule and world generator 10; the transport workload
    // and its delivered total remain unchanged.
    //
    // 1_951_253_762 → 360_047_202 when machines took their physical footprints and the line was
    // respaced around them. The entity count, the chain's hop count and the delivered total did
    // not move — only the empty ground between the machines, and so their coordinates.
    //
    // 360_047_202 → 3_227_239_126 when a belt became 5.37 m of conveyor an item takes
    // `BELT_TRANSIT_TICKS` to cross. The workload's shape, entity count and delivered total did
    // not move — the line is extraction-bound either way — but the pipeline is eight belts and
    // 216 ticks longer to fill, so the warmup moved with it and the lanes are now hashed state.
    //
    // 3_227_239_126 → 2_303_878_214 when a survey began opening a disc around the player's own
    // hex instead of a ring of the chunk lattice. `generated_chunks` is hashed, and this tier
    // opens a different set of them — the same world either way, since `tier_scenario` sets
    // `generated_environment: false` and there is no terrain here to change. The workload's
    // shape, entity count and delivered total did not move.
    //
    // 2_303_878_214 → 1_013_018_297 when the same pass moved `WORLD_GENERATOR_VERSION` to 12,
    // which `checksum_for_world` hashes first. Nothing in the workload moved with it; this is
    // the stamp, not the state.
    assert_eq!(first.checksum(), 1_013_018_297);
    assert_eq!(first.entities.len(), spec.entities() as usize);
    // Every line must be running end to end, or the tiers would measure an idle blueprint.
    // Four per line rather than fourteen: the line is now extraction-bound, because a
    // tier-one extractor spends 30 ticks per ore against the 5 this workload was calibrated
    // against. The ladder still times the same entity count moving the same cargo, but a tier
    // number recorded before this change was measured at a different cargo cadence and is not
    // comparable — `docs/BENCHMARKS.md` says so beside the affected rows.
    assert_eq!(first.delivered, u64::from(spec.lines) * 4);
}

/// A clock that advances a fixed amount per reading, so the ladder's arithmetic can be pinned
/// without depending on how long a machine actually takes.
struct StepClock {
    step_us: f64,
    readings: std::cell::Cell<u32>,
}

impl capacity::Clock for StepClock {
    fn now_us(&self) -> f64 {
        let reading = self.readings.get();
        self.readings.set(reading + 1);
        f64::from(reading) * self.step_us
    }
}

/// Every figure the ladder reports is arithmetic over a clock it was handed, so the whole of it
/// can be pinned without depending on how long a machine actually takes.
#[test]
fn capacity_is_measured_per_phase_and_per_tier_against_a_supplied_clock() {
    // Capacity phases are reported per sample against the supplied clock.
    let spec = capacity::quick_tiers()[0];
    let clock = StepClock {
        // Each phase reads the clock exactly twice, so one phase always spans one step.
        step_us: 1_000.0,
        readings: std::cell::Cell::new(0),
    };
    let tier = capacity::measure_tier_with(&spec, &clock, capacity::Budget::FIXED);
    // The tick phase spans one 1,000 µs step across `measured_ticks` samples.
    assert_eq!(tier.measured_ticks, spec.measured_ticks);
    assert_eq!(tier.tick_us, 1_000.0 / f64::from(spec.measured_ticks));
    assert_eq!(tier.frame_us, 1_000.0 / f64::from(spec.frames));
    assert_eq!(tier.snapshot_us, 1_000.0 / f64::from(spec.snapshots));
    assert_eq!(tier.ticks_per_second, 1e6 / tier.tick_us);
    // Every phase read the clock, and the workload itself is unchanged by the clock swap.
    assert_eq!(tier.entities, spec.entities() as usize);
    // Seven phases, each spanning exactly one pair of readings.
    assert_eq!(clock.readings.get(), 14);

    // A coarse clock must buy precision with more samples and nothing else: the tier's identity
    // has to survive, or a browser record could not be compared against a native one.
    let spec = capacity::quick_tiers()[1];
    let fixed = capacity::measure_tier_with(
        &spec,
        capacity::default_clock().as_ref(),
        capacity::Budget::FIXED,
    );
    // A step clock that only ever reports 500 µs per reading forces four repeats to reach a
    // 2,000 µs budget, without depending on how fast this machine is.
    let clock = StepClock {
        step_us: 500.0,
        readings: std::cell::Cell::new(0),
    };
    let budgeted = capacity::measure_tier_with(
        &spec,
        &clock,
        capacity::Budget {
            min_phase_us: 2_000.0,
        },
    );
    assert_eq!(budgeted.measured_ticks, spec.measured_ticks * 4);
    assert_eq!(
        budgeted.tick_us,
        2_000.0 / f64::from(budgeted.measured_ticks)
    );
    // The recorded identity of the tier is untouched by the extra samples.
    assert_eq!(budgeted.checksum, fixed.checksum);
    assert_eq!(budgeted.delivered, fixed.delivered);
    assert_eq!(budgeted.entities, fixed.entities);
    assert_eq!(budgeted.tiles, fixed.tiles);

    // Capacity ladder measures tiers independently and reports its platform.
    let specs = capacity::quick_tiers();
    let mut ladder = capacity::Ladder::new(specs.clone());
    let clock = capacity::default_clock();
    assert_eq!(ladder.len(), specs.len());
    assert!(ladder.measure(specs.len(), clock.as_ref()).is_none());
    // A partial run reports only what it measured, so an interrupted browser run still yields
    // an honest record rather than empty tiers.
    let first = ladder
        .measure(0, clock.as_ref())
        .expect("first tier measures");
    assert_eq!(ladder.report().tiers.len(), 1);
    // Re-measuring a tier replaces it instead of recording the same tier twice.
    let again = ladder
        .measure(0, clock.as_ref())
        .expect("first tier re-measures");
    assert_eq!(again.checksum, first.checksum);
    assert_eq!(ladder.report().tiers.len(), 1);

    ladder.measure(1, clock.as_ref()).expect("second tier");
    let report = ladder.report();
    assert_eq!(report.tiers.len(), 2);
    assert_eq!(report.platform, "native");
    assert_eq!(report.schema, capacity::REPORT_SCHEMA);
    assert!(capacity::format_table(&report).contains("native"));

    // The browser harness drives this factory over the ordinary worker RPC, so it must arrive in
    // the same steady state the in-wasm phases measure, and its first delta must be a complete
    // snapshot the host can adopt.
    let spec = capacity::quick_tiers()[1];
    let mut factory = capacity::warm_factory(&spec);
    let warm = capacity::warm_core(&spec);
    assert_eq!(factory.checksum(), warm.checksum());
    assert!(warm.delivered > 0);

    let first: serde_json::Value =
        serde_json::from_str(&factory.snapshot_delta_json()).expect("delta parses");
    assert_eq!(first["base_revision"], 0);
    assert_eq!(first["revision"], 1);
    assert_eq!(first["buildings"]["replace"], true);
    assert_eq!(
        first["buildings"]["changed"]
            .as_array()
            .expect("a first delta carries the complete blueprint")
            .len(),
        spec.entities() as usize
    );

    factory
        .advance_json("[{\"type\":\"move_intent\",\"x\":0,\"y\":0}]", 1, 0)
        .expect("idle batch is accepted");
    let next: serde_json::Value =
        serde_json::from_str(&factory.snapshot_delta_json()).expect("delta parses");
    assert_eq!(next["base_revision"], 1);
    assert_eq!(next["revision"], 2);
    // The steady-state delta is a patch, not another complete blueprint: `replace` is skipped
    // when false, and only the entities that moved travel.
    assert!(next["buildings"]["replace"].is_null());
    let changed = next["buildings"]["changed"]
        .as_array()
        .expect("a steady-state frame changes entities");
    assert!(!changed.is_empty() && changed.len() < spec.entities() as usize);

    // Capacity ladder reports a result for every tier.
    let specs = capacity::quick_tiers();
    let report = capacity::run(&specs);
    assert_eq!(report.schema, capacity::REPORT_SCHEMA);
    assert_eq!(report.tiers.len(), specs.len());
    for (tier, spec) in report.tiers.iter().zip(&specs) {
        assert_eq!(tier.entities, spec.entities() as usize);
        assert!(tier.tick_us > 0.0);
        assert!(tier.frame_us > 0.0);
        assert!(tier.full_compile_us > 0.0);
        assert!(tier.incremental_recompile_us > 0.0);
        assert!(tier.edit_us > 0.0);
        // A steady-state frame always carries at least the tick's changed groups.
        assert!(tier.delta_bytes > 0.0);
    }
    let table = capacity::format_table(&report);
    assert!(specs.iter().all(|spec| table.contains(spec.key)));
    assert!(capacity::format_json(&report).contains("\"schema\""));
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
/// The east edge of hex `q, r`, spelled the way the vertex lattice spells it: the run from the
/// hex's north-east corner to its south-east one.
fn boundary_edit(q: i32, r: i32) -> BoundaryEdit {
    edge_edit(q, r, 0)
}

/// The shared edge of hex `q, r` in `DIRECTIONS[direction]`, as a two-vertex run.
fn edge_edit(q: i32, r: i32, direction: u8) -> BoundaryEdit {
    BoundaryEdit {
        q,
        r,
        corner: (direction + 1) % 6,
        to_q: q,
        to_r: r,
        to_corner: (direction + 2) % 6,
        shape: BoundaryShape::Line,
        definition_id: 1,
        action: BoundaryAction::Build,
    }
}

/// A run from one lattice vertex to another, both named by hex and corner.
fn line_edit(from: (i32, i32, u8), to: (i32, i32, u8)) -> BoundaryEdit {
    BoundaryEdit {
        q: from.0,
        r: from.1,
        corner: from.2,
        to_q: to.0,
        to_r: to.1,
        to_corner: to.2,
        shape: BoundaryShape::Line,
        definition_id: 1,
        action: BoundaryAction::Build,
    }
}

#[test]
fn boundaries_are_canonical_atomic_conserving_and_block_what_crosses_them() {
    let mut core = empty_world("new-game");
    core.compile_graph();
    core.player.x = -3 * HEX_X;
    core.player.y = 0;
    let timber = core
        .definitions
        .items
        .iter()
        .find(|i| i.key == "timber")
        .unwrap()
        .id;
    let wire = core
        .definitions
        .items
        .iter()
        .find(|i| i.key == "iron-wire")
        .unwrap()
        .id;
    core.player.inventory = BTreeMap::from([(timber, 20), (wire, 2)]);
    let initial = core.player.inventory.clone();
    let edit = boundary_edit(0, 0);
    let checksum = core.checksum();
    let preview = core.boundary_preview(&edit);
    assert_eq!(preview.error, None);
    assert_eq!(
        preview.cost,
        vec![Ingredient {
            item_id: timber,
            quantity: 2
        }]
    );
    assert_eq!(core.checksum(), checksum);
    core.edit_boundaries(&edit).unwrap();
    assert_eq!(core.boundaries.len(), 1);
    assert_eq!(core.player.inventory[&timber], 18);
    assert_eq!(core.boundary_preview(&edit).changes, 0);
    let checksum = core.checksum();
    assert!(core.edit_boundaries(&edit).is_err());
    assert_eq!(checksum, core.checksum());
    let gate = BoundaryEdit {
        definition_id: 2,
        ..edit.clone()
    };
    assert_eq!(
        core.boundary_preview(&gate).cost,
        vec![Ingredient {
            item_id: wire,
            quantity: 1
        }]
    );
    core.edit_boundaries(&gate).unwrap();
    assert!(core.boundaries.values().next().unwrap().open);
    core.undo_boundary().unwrap();
    assert!(!core.boundaries.values().next().unwrap().open);
    core.undo_boundary().unwrap();
    assert!(core.boundaries.is_empty());
    assert_eq!(core.player.inventory, initial);
    core.set_creative(true);
    core.edit_boundaries(&edit).unwrap();
    core.set_creative(false);
    let before_remove = core.player.inventory.clone();
    core.edit_boundaries(&BoundaryEdit {
        action: BoundaryAction::Remove,
        ..edit
    })
    .unwrap();
    assert_eq!(core.player.inventory, before_remove);

    // Boundaries are canonical bounded atomic and reject unsafe sites.
    let mut core = empty_world("new-game");
    core.scenario.generated_environment = false;
    core.compile_graph();
    core.player.x = -3 * HEX_X;
    core.player.y = 0;
    core.set_creative(true);
    let edit = boundary_edit(0, 0);
    core.edit_boundaries(&edit).unwrap();
    // The same chord named from the neighbour that shares it is the same record, not a second.
    let reverse = edge_edit(1, 0, 3);
    assert_eq!(core.boundary_preview(&reverse).changes, 0);
    assert_eq!(core.boundaries.len(), 1);
    core.undo_boundary().unwrap();
    let yard = BoundaryEdit {
        shape: BoundaryShape::Yard,
        to_q: 1,
        to_corner: 3,
        ..edit.clone()
    };
    let preview = core.boundary_preview(&yard);
    assert_eq!(preview.error, None);
    let sides = preview.segments.len();
    assert!(sides >= 4);
    core.edit_boundaries(&yard).unwrap();
    assert_eq!(core.boundaries.len(), sides);
    core.undo_boundary().unwrap();
    let checksum = core.checksum();
    for invalid in [
        // A rectangle far past the segment budget is refused before anything is priced.
        BoundaryEdit {
            to_q: 99_999,
            ..yard.clone()
        },
        BoundaryEdit {
            q: i32::MIN,
            ..edit.clone()
        },
        BoundaryEdit {
            corner: 6,
            ..edit.clone()
        },
        BoundaryEdit {
            q: 99,
            r: 99,
            to_q: 99,
            to_r: 99,
            ..edit.clone()
        },
        // A rectangle with no extent is a point, which the yard shape cannot draw.
        BoundaryEdit {
            to_q: 0,
            to_corner: 1,
            ..yard.clone()
        },
    ] {
        assert!(core.boundary_preview(&invalid).error.is_some());
        assert!(core.edit_boundaries(&invalid).is_err());
        assert_eq!(core.checksum(), checksum);
    }
    core.set_creative(false);
    core.player.inventory.clear();
    let before = core.checksum();
    assert!(core.edit_boundaries(&yard).is_err());
    assert_eq!(core.checksum(), before);
    // Anchors stop one hex short of the limit, so canonicalizing a chord onto its neighbour can
    // never mint a record that save loading would reject.
    assert!(core
        .boundary_preview(&BoundaryEdit {
            q: -100_000,
            to_q: -100_000,
            ..edit.clone()
        })
        .error
        .unwrap()
        .contains("coordinate range"));
    core.set_creative(true);
    core.player.x = HEX_X / 2;
    assert!(core
        .boundary_preview(&edit)
        .error
        .unwrap()
        .contains("Step away"));

    // Boundaries block manual and click walks and gates replan routes.
    let mut core = empty_world("new-game");
    core.compile_graph();
    core.player.x = 0;
    core.player.y = 0;
    core.set_creative(true);
    let edit = boundary_edit(0, 0);
    core.walk_to(2, 0).unwrap();
    core.edit_boundaries(&edit).unwrap();
    assert_ne!(core.walk_path.first().map(|c| (c.q, c.r)), Some((1, 0)));
    core.set_move_intent(1000, 0).unwrap();
    core.advance_player_steps(30);
    assert!(core.player.x < HEX_X / 2);
    core.player.x = 0;
    core.player.y = 0;
    core.edit_boundaries(&BoundaryEdit {
        definition_id: 2,
        ..edit.clone()
    })
    .unwrap();
    core.walk_to(2, 0).unwrap();
    assert_eq!(core.walk_path.first().map(|c| (c.q, c.r)), Some((1, 0)));
    core.advance_player_steps(80);
    assert_eq!(world_to_axial(core.player.x, core.player.y), (2, 0));
    core.edit_boundaries(&BoundaryEdit {
        action: BoundaryAction::Close,
        ..edit.clone()
    })
    .unwrap();
    assert!(core.boundary_blocks_segment(axial_world(0, 0), axial_world(1, 0)));
    core.edit_boundaries(&BoundaryEdit {
        action: BoundaryAction::Open,
        ..edit
    })
    .unwrap();
    assert!(!core.boundary_blocks_segment(axial_world(0, 0), axial_world(1, 0)));

    // Boundaries protect transport and recompile future connections without losing cargo.
    let mut core = empty_world("new-game");
    core.compile_graph();
    core.player.x = -3 * HEX_X;
    core.player.y = 0;
    core.set_creative(true);
    let edit = boundary_edit(0, 0);
    core.edit_boundaries(&edit).unwrap();
    let a = add_test_entity(&mut core, 0, 0, 2, 0);
    let b = add_test_entity(&mut core, 1, 0, 2, 0);
    core.compile_graph();
    assert!(link_ids(&core, a).is_empty());
    let cargo = Cargo {
        item_id: 1,
        quantity: 1,
    };
    let a_index = index_of(&core, a);
    core.entities[a_index].cargo = Some(cargo);
    core.edit_boundaries(&BoundaryEdit {
        action: BoundaryAction::Remove,
        ..edit.clone()
    })
    .unwrap();
    assert_eq!(link_ids(&core, a), vec![b]);
    let graph = core.graph.clone();
    core.compile_graph();
    assert_eq!(graph, core.graph);
    assert_eq!(core.entities[index_of(&core, a)].cargo, Some(cargo));
    let checksum = core.checksum();
    assert!(core
        .edit_boundaries(&edit)
        .unwrap_err()
        .contains("transport"));
    assert_eq!(checksum, core.checksum());
    assert!(core.undo_boundary().unwrap_err().contains("transport"));

    // Boundaries save migrate validate and dirty deltas match the full oracle.
    let mut core = game("new-game");
    let old = core
        .save_string()
        .unwrap()
        .replace("\"save_version\":29", "\"save_version\":28")
        .replace("\"definition_version\":23", "\"definition_version\":22");
    let (definitions, technologies, scenarios) = catalogs();
    let migrated = Core::from_save(&definitions, &technologies, &scenarios, &old).unwrap();
    assert_eq!(migrated.checksum(), core.checksum());
    core.player.x = -3 * HEX_X;
    core.player.y = 0;
    core.set_creative(true);
    let edit = boundary_edit(-2, -2);
    core.edit_boundaries(&edit).unwrap();
    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.boundaries, core.boundaries);
    assert_eq!(restored.checksum(), core.checksum());
    assert!(restored.boundary_undo.is_empty());
    // Every boundary written before the vertex lattice spelled its chord `direction` and only
    // ever held the three shared edges, which are the same three chords under the same
    // identity. Old saves therefore load in place, byte for byte, with no state rewrite.
    let legacy = save
        .replace("\"save_version\":34", "\"save_version\":32")
        .replace("\"chord\":", "\"direction\":");
    assert!(legacy.contains("\"direction\":"));
    let loaded = Core::from_save(&definitions, &technologies, &scenarios, &legacy).unwrap();
    assert_eq!(loaded.boundaries, core.boundaries);
    assert_eq!(loaded.checksum(), core.checksum());
    let previous = core.snapshot();
    let baseline = SnapshotBaseline::from_snapshot(&previous);
    core.dirty = SnapshotDirty::default();
    core.edit_boundaries(&BoundaryEdit {
        action: BoundaryAction::Remove,
        ..edit
    })
    .unwrap();
    let current = core.snapshot();
    let mut factory = Factory {
        definitions,
        technologies,
        scenarios,
        core,
        snapshot_revision: 0,
        baseline: Some(baseline),
    };
    let delta = factory.build_delta();
    assert_eq!(delta, SnapshotDelta::between(0, 1, &previous, &current));
    assert_eq!(delta.boundaries, Some(Vec::new()));
    assert!(factory.build_delta().boundaries.is_none());

    // Boundaries cover all six sides vertices and keep the source digest exact.
    let mut core = empty_world("new-game");
    core.scenario.generated_environment = false;
    core.compile_graph();
    core.player.x = 0;
    core.player.y = 0;
    core.set_creative(true);
    for direction in 0..6 {
        let edit = edge_edit(0, 0, direction);
        core.edit_boundaries(&edit).unwrap();
        let (q, r) = DIRECTIONS[direction as usize];
        let other = axial_world(q, r);
        assert!(core.boundary_blocks_segment((0, 0), other));
        assert!(core.boundary_blocks_segment(other, (0, 0)));
        assert!(core.boundary_blocks_player(other.0 / 2, other.1 / 2));
        let reverse = edge_edit(q, r, (direction + 3) % 6);
        assert_eq!(core.boundary_preview(&reverse).changes, 0);
        assert_eq!(core.boundary_state_hash(), core.uncached_boundary_hash());
        assert_eq!(core.boundary_state_hash(), core.uncached_boundary_hash());
    }
    for (q, r) in TRANSPORT_DIRECTIONS {
        assert!(core.boundary_blocks_segment((0, 0), axial_world(q, r)));
    }
    assert!(core.walk_route((0, 0), (2, 0)).is_none());
    core.edit_boundaries(&BoundaryEdit {
        definition_id: 2,
        ..boundary_edit(0, 0)
    })
    .unwrap();
    assert!(core.walk_route((0, 0), (2, 0)).is_some());
    assert_eq!(core.boundary_state_hash(), core.uncached_boundary_hash());
    let hash = core.checksum();
    *core.boundary_hash_cache.borrow_mut() = None;
    assert_eq!(core.checksum(), hash);
    core.undo_boundary().unwrap();
    assert_eq!(core.boundary_state_hash(), core.uncached_boundary_hash());
    let mut invalid = core.boundary_snapshot();
    invalid.push(invalid[0].clone());
    assert!(validate_saved_boundaries(&core.definitions, &invalid).is_err());
    invalid = core.boundary_snapshot();
    invalid[0].paid = vec![Ingredient {
        item_id: 1,
        quantity: 1000,
    }];
    assert!(validate_saved_boundaries(&core.definitions, &invalid).is_err());
    invalid = core.boundary_snapshot();
    invalid[0].open = true;
    assert!(validate_saved_boundaries(&core.definitions, &invalid).is_err());
    let yard = BoundaryEdit {
        q: -2,
        r: -2,
        corner: 0,
        to_q: 0,
        to_r: 0,
        to_corner: 3,
        shape: BoundaryShape::Yard,
        ..boundary_edit(0, 0)
    };
    // The player is standing on the rectangle's own edge; step off it before walling it.
    core.player.x = 4 * HEX_X;
    let sides = core.boundary_preview(&yard);
    assert_eq!(sides.error, None);
    // A closed rectangle: every vertex it visits is entered once and left once.
    let mut visits: BTreeMap<(i32, i32), usize> = BTreeMap::new();
    for segment in &sides.segments {
        let (a, b) = segment.ends();
        *visits.entry(a).or_default() += 1;
        *visits.entry(b).or_default() += 1;
    }
    assert!(visits.values().all(|&n| n == 2));

    // The point of anchoring on vertices: a wall can hold one heading for a long run. Twelve
    // headings leave every lattice vertex, thirty degrees apart, and each has to draw exactly
    // straight for at least the twenty segments this phase is graded on.
    //
    // Only six of the twelve repeat one chord over and over — the honeycomb is not a lattice under
    // its own edges, so the other six alternate two chord lengths and are no less straight for it.
    // The test is collinearity, not sameness: every vertex the run touches lies on the ray, and
    // each one is further along it than the last.
    let mut core = empty_world("new-game");
    core.scenario.generated_environment = false;
    core.compile_graph();
    core.player.x = 40 * HEX_X;
    core.player.y = 0;
    core.set_creative(true);
    // Creative mode recomputes reach from earned skills; a twenty-segment run outruns it.
    core.player.build_range = 200 * HEX_X as u32;
    let start = (0, 0, 0u8);
    let origin = corner_world(start.0, start.1, start.2);
    let mut headings = BTreeSet::new();
    for corner in 0..6u8 {
        for hex in corner_hexes(start.0, start.1, start.2) {
            let Some(local) = (0..6u8).find(|&k| corner_world(hex.0, hex.1, k) == origin) else {
                continue;
            };
            if corner == local {
                continue;
            }
            let step = corner_world(hex.0, hex.1, corner);
            let (dx, dy) = (step.0 - origin.0, step.1 - origin.1);
            if !headings.insert((dx, dy)) {
                continue;
            }
            // Aim further and further along the ray, stopping at the first vertex on it whose
            // run is long enough to grade.
            let mut run = None;
            for reach in 1..=64 {
                let far = (origin.0 + dx * reach, origin.1 + dy * reach);
                let end = nearest_corner(far.0, far.1);
                if corner_world(end.0, end.1, end.2) != far {
                    continue;
                }
                let preview = core.boundary_preview(&line_edit(start, end));
                assert_eq!(preview.error, None, "heading {dx}, {dy} at {far:?}");
                if preview.segments.len() >= 20 {
                    run = Some((far, preview));
                    break;
                }
            }
            let (far, preview) = run.expect("twenty segments on this heading");
            let mut at = origin;
            for segment in &preview.segments {
                let (a, b) = segment.ends();
                assert!(a == at || b == at, "heading {dx}, {dy} broke at {at:?}");
                let next = if a == at { b } else { a };
                let (ax, ay) = (i64::from(next.0 - origin.0), i64::from(next.1 - origin.1));
                assert_eq!(
                    ax * i64::from(dy) - ay * i64::from(dx),
                    0,
                    "heading {dx}, {dy} left the line at {next:?}"
                );
                assert!(
                    i64::from(next.0 - at.0) * i64::from(dx)
                        + i64::from(next.1 - at.1) * i64::from(dy)
                        > 0
                );
                at = next;
            }
            assert_eq!(at, far);
        }
    }
    assert_eq!(headings.len(), 12);

    // Boundaries refuse full pack refunds and unfunded undo without changing state.
    let mut core = empty_world("new-game");
    core.compile_graph();
    core.player.x = -3 * HEX_X;
    core.player.y = 0;
    let timber = core
        .definitions
        .items
        .iter()
        .find(|i| i.key == "timber")
        .unwrap()
        .id;
    core.player.inventory = BTreeMap::from([(timber, 2)]);
    let edit = boundary_edit(0, 0);
    core.edit_boundaries(&edit).unwrap();
    core.player.inventory = BTreeMap::from([(
        IRON_ORE,
        core.stack_size(IRON_ORE) * core.player.carry_slots,
    )]);
    let remove = BoundaryEdit {
        action: BoundaryAction::Remove,
        ..edit
    };
    let checksum = core.checksum();
    assert!(core.edit_boundaries(&remove).unwrap_err().contains("room"));
    assert_eq!(core.checksum(), checksum);
    core.player.inventory.clear();
    core.edit_boundaries(&remove).unwrap();
    assert_eq!(core.player.inventory[&timber], 2);
    core.player.inventory.clear();
    let checksum = core.checksum();
    assert!(core.undo_boundary().unwrap_err().contains("materials"));
    assert_eq!(core.checksum(), checksum);
    core.player.inventory.insert(timber, 2);
    core.undo_boundary().unwrap();
    assert_eq!(core.boundaries.len(), 1);
    assert!(core.player.inventory.is_empty());

    // Boundaries protect multicell placement rotation and live gate crossings.
    let mut core = empty_world("new-game");
    core.scenario.generated_environment = false;
    core.compile_graph();
    core.player.x = -3 * HEX_X;
    core.player.y = 0;
    core.set_creative(true);
    let edit = BoundaryEdit {
        definition_id: 2,
        ..edge_edit(0, 0, 1)
    };
    core.edit_boundaries(&edit).unwrap();
    let container = core
        .definitions
        .buildings
        .iter_mut()
        .find(|d| d.id == 4)
        .unwrap();
    container.footprint = vec![Coordinate { q: 0, r: 0 }, Coordinate { q: 1, r: 0 }];
    assert!(core.placement_legality(0, 0, 4, 1, None, true).is_err());
    add_test_entity(&mut core, 0, 0, 4, 0);
    core.compile_graph();
    assert!(core.rotate(0, 0, false).unwrap_err().contains("boundary"));
    core.edit_boundaries(&BoundaryEdit {
        action: BoundaryAction::Remove,
        ..edit
    })
    .unwrap();
    core.rotate(0, 0, false).unwrap();
    let checksum = core.checksum();
    assert!(core.undo_boundary().unwrap_err().contains("building"));
    assert_eq!(core.checksum(), checksum);
    core.rotate(0, 0, true).unwrap();
    core.undo_boundary().unwrap();
    core.entities.clear();
    core.compile_graph();
    let a = add_test_entity(&mut core, 0, 0, 2, 1);
    let b = add_test_entity(&mut core, 0, 1, 2, 1);
    core.compile_graph();
    assert_eq!(link_ids(&core, a), vec![b]);
    let checksum = core.checksum();
    assert!(core
        .edit_boundaries(&BoundaryEdit {
            action: BoundaryAction::Close,
            ..edit
        })
        .unwrap_err()
        .contains("transport"));
    assert_eq!(core.checksum(), checksum);
}

#[test]
fn masonry_walls_need_fired_masonry_and_pay_cement() {
    let mut core = empty_world("new-game");
    core.compile_graph();
    core.player.x = -3 * HEX_X;
    core.player.y = 0;
    let brick = item_id(&core, "brick");
    let cement = item_id(&core, "cement");
    let timber = item_id(&core, "timber");
    core.player.inventory = BTreeMap::from([(brick, 12), (cement, 4), (timber, 8)]);
    let brick_wall = core
        .definitions
        .boundaries
        .iter()
        .find(|d| d.key == "brick-wall")
        .unwrap()
        .id;
    let timber_wall = core
        .definitions
        .boundaries
        .iter()
        .find(|d| d.key == "timber-wall")
        .unwrap()
        .id;
    let masonry = core
        .technologies
        .technologies
        .iter()
        .find(|t| t.key == "fired-masonry")
        .unwrap()
        .id;
    let edit = BoundaryEdit {
        definition_id: brick_wall,
        ..boundary_edit(0, 0)
    };
    assert!(core
        .boundary_preview(&edit)
        .error
        .as_deref()
        .unwrap()
        .contains("Fired Masonry"));
    core.edit_boundaries(&BoundaryEdit {
        definition_id: timber_wall,
        ..edit.clone()
    })
    .unwrap();
    assert_eq!(core.player.inventory[&timber], 4);
    core.edit_boundaries(&BoundaryEdit {
        action: BoundaryAction::Remove,
        ..edit.clone()
    })
    .unwrap();
    assert_eq!(core.player.inventory[&timber], 8);
    core.insight = 8;
    core.researched.extend([5, 7]);
    core.research(masonry).unwrap();
    core.edit_boundaries(&edit).unwrap();
    assert_eq!(core.player.inventory[&brick], 9);
    assert_eq!(core.player.inventory[&cement], 3);
    assert_eq!(
        core.boundaries.values().next().unwrap().definition_id,
        brick_wall
    );
}

/// A flat, empty, deposit-free world to grade in. Generated terrain is switched off so that a
/// test about prepared ground is not also a test about where the noise put a hill.
fn ground_world() -> Core {
    let mut core = bare_game("new-game");
    core.scenario.generated_environment = false;
    core.ground_spine = GroundSpine::physical(&core.world_params, core.seed, false);
    core.fields = WorldFields::new(&core.world_params, core.seed, &core.ground_spine);
    core.entities.clear();
    core.graph.clear();
    core.next_entity_id = 1;
    // Off to one side: nobody can grade the hex they are standing on, and these tests are about
    // the ground rather than about where the player happens to be.
    set_player_hex(&mut core, 0, -5);
    core.compile_graph();
    core.dirty = SnapshotDirty::default();
    reach(&mut core);
    core
}

/// Reach far enough that no ground test is accidentally a test about build range. It is set
/// after any creative toggle, because granting research recomputes the earned range.
fn reach(core: &mut Core) {
    core.player.build_range = 1 << 20;
}

fn ground_edit(q: i32, r: i32, action: GroundAction) -> GroundEdit {
    GroundEdit {
        q,
        r,
        to_q: q,
        to_r: r,
        corner: 0,
        to_corner: 0,
        shape: GroundShape::Cell,
        definition_id: 2,
        action,
        steps: 1,
        reference: GroundReference::First,
        cover: false,
    }
}

fn item_id(core: &Core, key: &str) -> ItemId {
    core.definitions
        .items
        .iter()
        .find(|i| i.key == key)
        .unwrap()
        .id
}

/// The whole surface contract in one pass: a preview costs nothing and moves nothing, a commit
/// spends exactly the declared bill, stripping the surface hands the same bill back, undo is
/// priced by the same arithmetic in reverse, and repainting what is already there is refused
/// rather than charged.
#[test]
fn ground_works_conserve_spoil_gate_routes_and_survive_a_save() {
    let mut core = ground_world();
    let gravel = item_id(&core, "gravel");
    core.player.inventory = BTreeMap::from([(gravel, 6)]);
    let initial = core.player.inventory.clone();
    let checksum = core.checksum();

    let edit = GroundEdit {
        to_q: 2,
        shape: GroundShape::Path,
        ..ground_edit(0, 0, GroundAction::Pave)
    };
    let preview = core.ground_preview(&edit);
    assert_eq!(preview.error, None);
    assert_eq!(preview.changes, 3);
    assert_eq!(
        preview.cost,
        vec![Ingredient {
            item_id: gravel,
            quantity: 3
        }]
    );
    assert_eq!(preview.cut, 0);
    assert_eq!(preview.fill, 0);
    assert_eq!(preview.covers, 0);
    assert_eq!(core.checksum(), checksum, "a preview changes nothing");
    assert_eq!(core.player.inventory, initial);

    core.edit_ground(&edit).unwrap();
    assert_eq!(core.ground.len(), 3);
    assert_eq!(core.player.inventory, BTreeMap::from([(gravel, 3)]));
    assert_eq!(core.surface_at(1, 0), 2);
    assert_eq!(core.movement_factor_at(1, 0), 120);
    assert_ne!(core.checksum(), checksum);

    // Repainting the same surface is a no-op, and a no-op costs nothing.
    let idle = core.ground_preview(&edit);
    assert_eq!(idle.changes, 0);
    assert!(idle.cost.is_empty());
    assert!(core.edit_ground(&edit).unwrap_err().contains("nothing"));
    assert_eq!(core.player.inventory, BTreeMap::from([(gravel, 3)]));

    let clear = GroundEdit {
        action: GroundAction::Clear,
        ..edit.clone()
    };
    assert_eq!(
        core.ground_preview(&clear).refund,
        vec![Ingredient {
            item_id: gravel,
            quantity: 3
        }]
    );
    core.edit_ground(&clear).unwrap();
    assert!(
        core.ground.is_empty(),
        "untreated ground leaves the overlay"
    );
    assert_eq!(core.player.inventory, initial);
    assert_eq!(core.checksum(), checksum, "the world came back exactly");

    // Undo re-lays what the clear took up, and buys it again at the same price.
    core.undo_ground().unwrap();
    assert_eq!(core.ground.len(), 3);
    assert_eq!(core.player.inventory, BTreeMap::from([(gravel, 3)]));
    core.undo_ground().unwrap();
    assert!(core.ground.is_empty());
    assert_eq!(core.player.inventory, initial);
    assert_eq!(core.checksum(), checksum);

    // Fill is dug, never conjured. This is the exploit check: raising ground with an empty ledger
    // is refused, the ledger conserves exactly one step per step in both directions, undo restores
    // the count the edit found rather than minting one, and neither the grade bound nor the ledger
    // can be walked past by repeating the edit.
    let mut core = ground_world();
    let checksum = core.checksum();
    assert_eq!(core.spoil, 0);

    let raise = ground_edit(0, 0, GroundAction::Raise);
    let refusal = core.ground_preview(&raise).error;
    assert!(
        refusal.as_deref().is_some_and(|m| m.contains("spoil")),
        "{refusal:?}"
    );
    assert!(core.edit_ground(&raise).is_err());
    assert_eq!(core.checksum(), checksum);

    let lower = ground_edit(3, 0, GroundAction::Lower);
    let step = core.grade_step_delta(1) as u64;
    let edits = u64::try_from(scale::EARTHWORK_LIMIT_QUANTA).unwrap() / step;
    for n in 1..=edits {
        core.edit_ground(&lower).unwrap();
        assert_eq!(core.spoil, step * n);
    }
    assert_eq!(
        core.ground_elevation_at(3, 0),
        -scale::EARTHWORK_LIMIT_QUANTA
    );
    assert!(core.edit_ground(&lower).unwrap_err().contains("full"));
    assert_eq!(
        core.spoil,
        u64::try_from(scale::EARTHWORK_LIMIT_QUANTA).unwrap()
    );

    // One step of fill spends exactly one step of spoil.
    core.edit_ground(&raise).unwrap();
    assert_eq!(
        core.spoil,
        u64::try_from(scale::EARTHWORK_LIMIT_QUANTA).unwrap() - step
    );
    assert_eq!(core.ground_elevation_at(0, 0), step as i32);
    core.undo_ground().unwrap();
    assert_eq!(
        core.spoil,
        u64::try_from(scale::EARTHWORK_LIMIT_QUANTA).unwrap(),
        "undoing fill returns the spoil it spent"
    );
    assert_eq!(core.ground_elevation_at(0, 0), 0);

    // Levelling evens onto the first cell of the selection and balances against the ledger.
    let level = GroundEdit {
        to_q: 3,
        shape: GroundShape::Path,
        action: GroundAction::Level,
        ..ground_edit(0, 0, GroundAction::Level)
    };
    let preview = core.ground_preview(&level);
    assert_eq!(preview.error, None);
    assert_eq!(
        preview.fill,
        u32::try_from(scale::EARTHWORK_LIMIT_QUANTA).unwrap(),
        "the pit is filled back to the first cell"
    );
    assert_eq!(preview.cut, 0);
    assert_eq!(preview.spoil, 0);
    core.edit_ground(&level).unwrap();
    assert_eq!(core.spoil, 0);
    assert!(core.ground.is_empty(), "level ground leaves the overlay");
    assert_eq!(core.checksum(), checksum, "the ledger balances to zero");

    // The selection modes, and the one property that makes an outline an outline: it is exactly
    // the hexes of its own filled shape that touch something outside it. Deriving the outline from
    // the fill rather than drawing it with geometry of its own is what makes it one hex thick at
    // every size, with no rounding rule that could disagree with the fill's.
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    let cells = |edit: &GroundEdit| -> Vec<(i32, i32)> {
        let preview = core.ground_preview(edit);
        assert_eq!(preview.error, None);
        preview.cells.iter().map(|cell| (cell.q, cell.r)).collect()
    };

    // A circle is dragged from its centre out to a rim hex, so its radius is a distance the
    // player can count on the map rather than a number typed into a field.
    let disc = GroundEdit {
        to_q: 2,
        shape: GroundShape::Disc,
        ..ground_edit(0, 0, GroundAction::Pave)
    };
    let filled = cells(&disc);
    let rim = cells(&GroundEdit {
        shape: GroundShape::Ring,
        ..disc.clone()
    });
    assert_eq!(filled.len(), 19, "1 + 3n(n + 1) hexes at radius two");
    assert_eq!(rim.len(), 12, "6n hexes at radius two");
    assert!(rim.iter().all(|&cell| axial_distance((0, 0), cell) == 2));

    // A rectangle and its frame share both anchors, so a floor and the kerb round it are the
    // same drag with one button changed.
    let rect = GroundEdit {
        to_q: 3,
        to_r: 3,
        corner: 4,
        to_corner: 1,
        shape: GroundShape::Rect,
        ..ground_edit(0, 0, GroundAction::Pave)
    };
    let area = cells(&rect);
    let frame = cells(&GroundEdit {
        shape: GroundShape::Frame,
        ..rect.clone()
    });
    assert!(
        area.len() > frame.len(),
        "this rectangle has an interior to leave out: {} vs {}",
        area.len(),
        frame.len()
    );

    for (fill, outline) in [(filled, rim), (area, frame)] {
        let inside: BTreeSet<(i32, i32)> = fill.iter().copied().collect();
        let edge: BTreeSet<(i32, i32)> = fill
            .iter()
            .copied()
            .filter(|&(q, r)| {
                DIRECTIONS
                    .iter()
                    .any(|&(dq, dr)| !inside.contains(&(q + dq, r + dr)))
            })
            .collect();
        assert_eq!(outline.into_iter().collect::<BTreeSet<_>>(), edge);
    }

    // Both circular modes are bounded by arithmetic rather than by a scan, so an over-wide drag
    // is refused before a single hex is enumerated.
    let wide = core.ground_preview(&GroundEdit { to_q: 5, ..disc });
    assert!(wide.error.unwrap().contains("too wide"));
    assert!(wide.cells.is_empty());

    // What a selection has to do when it cannot be applied: stay on screen, and say which hex is
    // the problem. One obstacle used to erase the whole footprint it was standing in, which left
    // the player a refusal and no picture of what it was about.
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    core.write_overlay(2, 0, WOOD, 9, 14);

    let lower = GroundEdit {
        to_q: 4,
        shape: GroundShape::Path,
        ..ground_edit(0, 0, GroundAction::Lower)
    };
    let preview = core.ground_preview(&lower);
    assert_eq!(preview.error, None, "one obstacle no longer refuses four");
    assert_eq!(preview.cells.len(), 5, "the footprint is drawn whole");
    assert_eq!(preview.blocked, 1);
    assert_eq!(preview.changes, 4);
    let stuck = preview
        .cells
        .iter()
        .find(|cell| (cell.q, cell.r) == (2, 0))
        .unwrap();
    assert!(stuck
        .blocked
        .as_deref()
        .is_some_and(|reason| reason.contains("deposit")));
    assert_eq!(stuck.change, 0, "a blocked hex moves no ground");
    core.edit_ground(&lower).unwrap();
    assert_eq!(core.spoil, 8);
    assert_eq!(core.ground_elevation_at(2, 0), 0, "the deposit sat still");

    // A refusal about the selection as a whole keeps its footprint too: that picture is how the
    // player works out where the spoil has to come from.
    let starved = core.ground_preview(&GroundEdit {
        steps: 3,
        ..GroundEdit {
            action: GroundAction::Raise,
            ..lower.clone()
        }
    });
    assert!(starved.error.unwrap().contains("spoil"));
    assert_eq!(starved.cells.len(), 5);

    // Depth is one number rather than three gestures, and a hex without room for the whole cut
    // takes what it has room for instead of refusing the pass. Prepare a cell two quanta shy
    // of the physical eight-metre limit so the final 1.5 m request exercises that clamp.
    for _ in 0..4 {
        core.edit_ground(&GroundEdit {
            steps: 3,
            ..ground_edit(3, 0, GroundAction::Lower)
        })
        .unwrap();
    }
    core.edit_ground(&GroundEdit {
        steps: 2,
        ..ground_edit(3, 0, GroundAction::Lower)
    })
    .unwrap();
    let deep = GroundEdit {
        steps: 3,
        ..ground_edit(3, 0, GroundAction::Lower)
    };
    assert_eq!(core.ground_preview(&deep).cut, 2, "clamped, not refused");
    core.edit_ground(&deep).unwrap();
    assert_eq!(
        core.ground_elevation_at(3, 0),
        -scale::EARTHWORK_LIMIT_QUANTA
    );
    assert!(core.edit_ground(&deep).unwrap_err().contains("full"));

    // Levelling names its datum. The same three hexes even onto the lowest, the highest, or the
    // one the drag started on, and the spoil ledger is what tells the three apart.
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    // A stepped profile: 0, -0.5 m, -1.0 m across three hexes.
    core.edit_ground(&ground_edit(1, 0, GroundAction::Lower))
        .unwrap();
    core.edit_ground(&GroundEdit {
        steps: 2,
        ..ground_edit(2, 0, GroundAction::Lower)
    })
    .unwrap();
    assert_eq!(core.spoil, 6);

    let level = GroundEdit {
        to_q: 2,
        shape: GroundShape::Path,
        action: GroundAction::Level,
        ..ground_edit(0, 0, GroundAction::Level)
    };
    let lowest = core.ground_preview(&GroundEdit {
        reference: GroundReference::Lowest,
        ..level.clone()
    });
    assert_eq!(lowest.error, None);
    assert_eq!((lowest.cut, lowest.fill), (6, 0), "down to the deepest cut");
    assert_eq!(lowest.spoil, 12, "and the heap keeps what came out");

    let highest = core.ground_preview(&GroundEdit {
        reference: GroundReference::Highest,
        ..level.clone()
    });
    assert_eq!(
        (highest.cut, highest.fill),
        (0, 6),
        "up to the untouched hex"
    );
    assert_eq!(highest.spoil, 0, "which spends the heap instead");

    // The default is still the hex the drag started on, so an edit written before this control
    // existed means exactly what it meant.
    let first = core.ground_preview(&level);
    assert_eq!((first.cut, first.fill), (0, 6));

    core.edit_ground(&GroundEdit {
        reference: GroundReference::Lowest,
        ..level
    })
    .unwrap();
    for q in 0..=2 {
        assert_eq!(core.ground_elevation_at(q, 0), -4);
    }
    assert_eq!(core.spoil, 12);

    // The route search prices travel time, so a longer prepared way beats a shorter raw one, and a
    // step nobody can climb stops the route and the body alike.
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    set_player_hex(&mut core, 0, 0);

    // Untreated, the shortest way is the straight one.
    core.walk_to(5, 0).unwrap();
    assert_eq!(core.walk_path.len(), 5);
    assert!(core.walk_path.iter().all(|cell| cell.r == 0));

    // Concrete is a third faster, so five paved hexes and one raw one beat five raw ones.
    core.edit_ground(&GroundEdit {
        to_q: 4,
        to_r: 1,
        shape: GroundShape::Path,
        definition_id: 5,
        ..ground_edit(0, 1, GroundAction::Pave)
    })
    .unwrap();
    assert_eq!(core.movement_factor_at(2, 1), 130);
    assert_eq!(
        core.walk_step_cost((1, 1), 2, 1),
        WALK_STEP_COST * 100 / 130
    );
    core.walk_to(5, 0).unwrap();
    assert_eq!(core.walk_path.len(), 6, "the paved way is one hex longer");
    assert!(core.walk_path.iter().any(|cell| (cell.q, cell.r) == (3, 1)));

    // The player walks it at the speed the route was priced at.
    set_player_hex(&mut core, 2, 1);
    core.player.walk_goal = None;
    core.walk_path.clear();
    core.set_move_intent(1000, 0).unwrap();
    let start = core.player.x;
    core.advance_player_steps(1);
    assert_eq!(core.player.x - start, PLAYER_SPEED * 130 / 100);

    // A wall taller than anyone can climb is a wall to the route and to the body.
    set_player_hex(&mut core, 0, 0);
    core.set_move_intent(0, 0).unwrap();
    core.edit_ground(&GroundEdit {
        steps: 3,
        ..ground_edit(-2, 0, GroundAction::Lower)
    })
    .unwrap();
    core.edit_ground(&GroundEdit {
        steps: 3,
        ..ground_edit(-1, 0, GroundAction::Raise)
    })
    .unwrap();
    assert_eq!(core.ground_elevation_at(-1, 0), 6);
    assert!(core.grade_blocks((0, 0), (-1, 0)));
    assert!(core.grade_blocks((-1, 0), (-2, 0)), "a wall is symmetric");
    assert!(core.walk_to(-1, 0).is_err());
    let (blocked_x, blocked_y) = axial_world(-1, 0);
    assert!(core.player_blocked(blocked_x, blocked_y));
    // Four quanta is still a walkable one-metre slope, not a wall.
    core.edit_ground(&ground_edit(-1, 0, GroundAction::Lower))
        .unwrap();
    assert!(!core.grade_blocks((0, 0), (-1, 0)));
    core.walk_to(-1, 0).unwrap();

    // Covering a deposit is deliberate, reversible and lossless. It is confirmed before it happens,
    // it suppresses hands, extractors, the published snapshot and regrowth without harvesting a
    // single unit, and stripping the surface hands back exactly what was sealed.
    let mut core = ground_world();
    core.write_overlay(2, 0, WOOD, 9, 14);
    core.rebuild_flora_regrowth();
    assert!(core.flora_regrowth.contains(&(2, 0)));
    core.player.inventory = BTreeMap::from([(item_id(&core, "gravel"), 4)]);

    let pave = ground_edit(2, 0, GroundAction::Pave);
    let warned = core.ground_preview(&pave);
    assert_eq!(warned.covers, 1);
    assert!(warned.error.unwrap().contains("Confirm covering"));
    assert!(core.edit_ground(&pave).is_err());

    let confirmed = GroundEdit {
        cover: true,
        ..pave
    };
    assert_eq!(core.ground_preview(&confirmed).error, None);
    core.edit_ground(&confirmed).unwrap();
    assert_eq!(core.field_at(2, 0), None, "a sealed deposit is unreachable");
    assert_eq!(core.deposit_quantity((2, 0)), 0);
    assert!(!core
        .resource_snapshots()
        .iter()
        .any(|row| (row.q, row.r) == (2, 0)));
    assert!(
        !core.flora_regrowth.contains(&(2, 0)),
        "sealing suppresses regrowth without harvesting"
    );
    // Nothing was taken: the overlay still holds every unit that was left.
    assert_eq!(core.tiles[&(2, 0)].resource.as_ref().unwrap().quantity, 9);
    core.advance_ticks(600);
    assert_eq!(core.tiles[&(2, 0)].resource.as_ref().unwrap().quantity, 9);

    core.edit_ground(&GroundEdit {
        action: GroundAction::Clear,
        ..confirmed
    })
    .unwrap();
    assert_eq!(core.deposit_quantity((2, 0)), 9, "the remainder comes back");
    assert!(core.flora_regrowth.contains(&(2, 0)));

    // Grading never moves a deposit, and an extractor at work is not paved over from under.
    assert!(core
        .ground_preview(&ground_edit(2, 0, GroundAction::Lower))
        .error
        .unwrap()
        .contains("deposit"));
    core.set_creative(true);
    reach(&mut core);
    core.write_overlay(3, 0, WOOD, 5, 5);
    assert_eq!(core.place(3, 0, 1, 0, None), Ok(()));
    core.compile_graph();
    assert!(core.field_covered_at((3, 0), (2, 0), core.extract_radius_of(1)));
    assert!(core
        .ground_preview(&confirmed)
        .error
        .unwrap()
        .contains("extractor"));

    // A footprint needs a pad flatter than the steepest slope a player may still walk.
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    let container = core
        .definitions
        .buildings
        .iter_mut()
        .find(|d| d.id == 4)
        .unwrap();
    container.footprint = vec![Coordinate { q: 0, r: 0 }, Coordinate { q: 1, r: 0 }];
    assert_eq!(core.placement_legality(0, 0, 4, 0, None, true), Ok(()));

    for _ in 0..2 {
        core.edit_ground(&ground_edit(4, 0, GroundAction::Lower))
            .unwrap();
    }
    core.edit_ground(&ground_edit(1, 0, GroundAction::Raise))
        .unwrap();
    assert_eq!(core.ground_elevation_at(1, 0), scale::MAX_BUILD_STEP_QUANTA);
    assert_eq!(core.placement_legality(0, 0, 4, 0, None, true), Ok(()));

    // A one-metre slope is still walkable, but a foundation now needs the flatter pad contract.
    core.edit_ground(&ground_edit(1, 0, GroundAction::Raise))
        .unwrap();
    assert_eq!(core.ground_elevation_at(1, 0), scale::MAX_WALK_STEP_QUANTA);
    assert!(core
        .placement_legality(0, 0, 4, 0, None, true)
        .unwrap_err()
        .contains("level a pad"));

    // A span foundation may follow a slope a player can still walk; the pad class may not.
    set_test_foundation(&mut core, 4, FoundationClass::Span);
    assert_eq!(core.placement_legality(0, 0, 4, 0, None, true), Ok(()));
    set_test_foundation(&mut core, 4, FoundationClass::Pad);

    // Levelling the pair onto the first cell's grade is exactly what makes the site legal.
    core.edit_ground(&GroundEdit {
        to_q: 1,
        shape: GroundShape::Path,
        action: GroundAction::Level,
        ..ground_edit(0, 0, GroundAction::Level)
    })
    .unwrap();
    assert_eq!(core.ground_elevation_at(1, 0), 0);
    assert_eq!(core.placement_legality(0, 0, 4, 0, None, true), Ok(()));

    // Prepared ground survives a save, migrates forward from a file that never had any, refuses a
    // state the definitions cannot explain, and its dirty-tracked delta matches the full oracle.
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    core.edit_ground(&GroundEdit {
        to_q: 2,
        shape: GroundShape::Path,
        ..ground_edit(0, 0, GroundAction::Pave)
    })
    .unwrap();
    core.edit_ground(&ground_edit(0, 2, GroundAction::Lower))
        .unwrap();
    assert_eq!(core.spoil, 2);

    // Reach is a scenario property the loader checks against the catalogue rather than a
    // simulation result, so the borrowed test reach goes back before anything is written.
    core.player.build_range = core.earned_build_range();
    let save = core.save_string().unwrap();
    let (definitions, technologies, scenarios) = catalogs();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.ground, core.ground);
    assert_eq!(restored.spoil, core.spoil);
    assert_eq!(restored.checksum(), core.checksum());

    // The old one-square-metre ground cannot be reconstructed as physical drainage, even when
    // it happens to carry no prepared cells. The catalogue keeps the file exportable and the
    // native boundary refuses to pretend it can be resumed.
    let mut untouched = ground_world();
    untouched.player.build_range = untouched.earned_build_range();
    let plain = untouched.save_string().unwrap();
    let old = plain.replace(
        &format!("\"save_version\":{SAVE_VERSION}"),
        "\"save_version\":36",
    );
    let error = match Core::from_save(&definitions, &technologies, &scenarios, &old) {
        Ok(_) => panic!("legacy ground crossed the physical compatibility boundary"),
        Err(error) => error,
    };
    assert!(error.contains("export"), "{error}");

    let mut invalid = core.ground_snapshot();
    invalid[0].elevation = i16::try_from(scale::EARTHWORK_LIMIT_QUANTA + 1).unwrap();
    assert!(validate_saved_ground(&definitions, &invalid).is_err());
    let mut invalid = core.ground_snapshot();
    invalid[0].surface = 99;
    assert!(validate_saved_ground(&definitions, &invalid).is_err());
    let mut invalid = core.ground_snapshot();
    invalid.push(invalid[0].clone());
    assert!(validate_saved_ground(&definitions, &invalid).is_err());
    let mut invalid = core.ground_snapshot();
    invalid[0].paid = vec![Ingredient {
        item_id: 1,
        quantity: 1,
    }];
    invalid[0].surface = 0;
    assert!(
        validate_saved_ground(&definitions, &invalid).is_err(),
        "untreated ground cannot carry a paid bill"
    );

    // The digest is pure, and the cache is only ever an echo of it.
    assert_eq!(core.ground_state_hash(), core.uncached_ground_hash());

    // The dirty-tracked delta is what a full diff of two snapshots would have said, and nothing
    // is resent once the host has it.
    let mut factory = test_factory("new-game");
    factory.core = ground_world();
    factory.core.set_creative(true);
    reach(&mut factory.core);
    let mut previous = factory.core.snapshot();
    factory.build_delta();
    factory
        .core
        .edit_ground(&ground_edit(0, 0, GroundAction::Lower))
        .unwrap();
    assert_delta_matches_full_diff(&mut factory, &mut previous, "a cut");
    factory
        .core
        .edit_ground(&ground_edit(0, 1, GroundAction::Pave))
        .unwrap();
    assert_delta_matches_full_diff(&mut factory, &mut previous, "a paved cell");
    factory.core.undo_ground().unwrap();
    assert_delta_matches_full_diff(&mut factory, &mut previous, "an undo");
    let quiet = factory.build_delta();
    assert!(quiet.ground.is_none());
    assert!(quiet.spoil.is_none());
    assert!(quiet.water.is_none());
}

/// A flood is a sparse overlay, like a grade: the tile still carries the generated depth, and
/// the delta carries only the cells that left it. Returning to equilibrium sends the empty list
/// so the host drops the overlay rather than keeping the last flood it saw.
#[test]
fn a_disturbed_depth_is_what_the_delta_publishes() {
    let mut factory = test_factory("new-game");
    factory.core.set_creative(true);
    let (q, r) = {
        let size = factory.core.scenario.chunk_size;
        factory
            .core
            .generated_chunks
            .iter()
            .copied()
            .flat_map(|(chunk_q, chunk_r)| hexes_in_chunk(chunk_q, chunk_r, size))
            .find(|&(cell_q, cell_r)| factory.core.water_depth_at(cell_q, cell_r) == 0)
            .expect("the opening surveys dry ground")
    };
    factory.core.water.set(q, r, hydrology::WaterDelta::new(6));
    factory.core.settle_water(&[(q, r)]);
    let _ = factory.snapshot_json();
    let mut previous = factory.core.snapshot();
    assert!(
        !previous.water.is_empty(),
        "a flood is a departure the snapshot carries"
    );

    let seeds: Vec<(i32, i32)> = factory
        .core
        .water
        .cells()
        .iter()
        .map(|cell| (cell.q, cell.r))
        .collect();
    for &(cell_q, cell_r) in &seeds {
        factory
            .core
            .water
            .set(cell_q, cell_r, hydrology::WaterDelta::new(0));
    }
    factory.core.settle_water(&seeds);
    assert!(
        factory.core.water.is_empty(),
        "forgetting every departure is the equilibrium"
    );
    assert_delta_matches_full_diff(&mut factory, &mut previous, "draining the flood");
    let quiet = factory.build_delta();
    assert!(quiet.water.is_none());
}

/// The generated world is exactly as passable after this release as before it. Every pair of
/// walkable bands is within one climbable step, which is the whole reason `natural_elevation`
/// has the values it does, and it is asserted here rather than trusted.
#[test]
fn no_terrain_walls_itself_off_and_a_quarried_cliff_stops_being_a_wall() {
    let bands = [
        Terrain::DeepWater,
        Terrain::ShallowWater,
        Terrain::Shore,
        Terrain::Lowland,
        Terrain::Hills,
        Terrain::Highland,
        Terrain::Cliff,
    ];
    for &a in &bands {
        for &b in &bands {
            if a.blocks_movement() || b.blocks_movement() {
                continue;
            }
            assert!(
                (natural_elevation(a) - natural_elevation(b)).abs() <= MAX_WALK_STEP,
                "{a:?} and {b:?} would be walled off from each other"
            );
        }
    }
    // A run that has never touched the ground contributes nothing to the checksum, which is what
    // keeps a file written a release ago checksumming to the value it did then.
    let core = ground_world();
    assert!(core.ground.is_empty());
    assert_eq!(core.spoil, 0);
    assert_eq!(core.movement_factor_at(0, 0), UNTREATED_MOVEMENT);
    // The heuristic floor is the cheapest step the fastest legal surface can produce, so the
    // route search never overestimates and never returns a route that is not the cheapest.
    assert_eq!(
        MIN_WALK_STEP_COST,
        WALK_STEP_COST * UNTREATED_MOVEMENT / MAX_SURFACE_MOVEMENT
    );
    assert!(MIN_WALK_STEP_COST <= WALK_STEP_COST * UNTREATED_MOVEMENT / MAX_SURFACE_MOVEMENT);

    // The one wall the player may take apart, end to end.
    //
    // A cliff is impassable until somebody quarries it. Nothing may be laid on a face that is
    // still standing, one cut brings that face level with the highland beside it, and after the
    // cut the hex walks and builds like any other ground — with the rock that came out of it on
    // the spoil heap rather than gone. The band the generator drew never moves: the whole change
    // lives in the overlay, so a world nobody has dug is exactly as passable as it always was.
    let mut core = legacy_band_game("new-game");
    reach(&mut core);
    // The nearest cliff face outside the landing hub's own seven hexes.
    assert_eq!(core.terrain_at(2, -1), Terrain::Cliff);
    assert!(core.terrain_blocks_movement(2, -1));
    assert!(core.terrain_blocks_construction(2, -1));
    assert!(!core.walkable_hex(2, -1));
    assert_eq!(core.spoil, 0);

    let pave = core.ground_preview(&ground_edit(2, -1, GroundAction::Pave));
    assert!(
        pave.error
            .as_deref()
            .is_some_and(|error| error.contains("Cut this cliff down first")),
        "paving a standing cliff said {:?}",
        pave.error
    );

    core.edit_ground(&ground_edit(2, -1, GroundAction::Lower))
        .unwrap();
    assert!(core.cliff_quarried(2, -1));
    assert_eq!(core.terrain_at(2, -1), Terrain::Cliff);
    assert_eq!(
        core.ground_elevation_at(2, -1),
        natural_elevation(Terrain::Highland)
    );
    assert!(!core.terrain_blocks_movement(2, -1));
    assert!(!core.terrain_blocks_construction(2, -1));
    assert!(core.walkable_hex(2, -1));
    // Quarried rock leaves as spoil, on the same ledger every other cut pays into.
    assert_eq!(core.spoil, 1);

    // Undo is the edit run backwards, so the wall comes back and takes its spoil with it.
    core.undo_ground().unwrap();
    assert!(!core.cliff_quarried(2, -1));
    assert!(core.terrain_blocks_movement(2, -1));
    assert!(core.terrain_blocks_construction(2, -1));
    assert_eq!(core.spoil, 0);
    assert!(core.ground.is_empty());
}

#[test]
fn geomorphic_state_round_trips_and_keeps_the_next_epoch_deterministic() {
    let (definitions, technologies, scenarios) = catalogs();
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|scenario| scenario.key == "new-game")
        .unwrap();
    let mut first = Core::new(&definitions, &technologies, scenario, None, None).unwrap();
    first.ground.insert(
        (0, 0),
        GroundCell {
            q: 0,
            r: 0,
            surface: 0,
            elevation: 0,
            erosion: -1,
            paid: Vec::new(),
        },
    );
    first.bank_stress = geomorphology::BankStress::from_cells(&[geomorphology::StressCell {
        q: 2,
        r: -1,
        stress: 17,
    }]);
    let saved = first.save_string().unwrap();
    let mut restored = Core::from_save(&definitions, &technologies, &scenarios, &saved).unwrap();
    assert_eq!(restored.checksum(), first.checksum());
    assert_eq!(restored.ground, first.ground);
    assert_eq!(restored.bank_stress, first.bank_stress);

    let first_epoch = first.run_geomorphic_epoch();
    let restored_epoch = restored.run_geomorphic_epoch();
    assert_eq!(restored_epoch, first_epoch);
    assert_eq!(restored.checksum(), first.checksum());
}

#[test]
fn save_40_adopts_empty_geomorphology_without_changing_its_checksum() {
    let (definitions, technologies, scenarios) = catalogs();
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|scenario| scenario.key == "new-game")
        .unwrap();
    let core = Core::new(&definitions, &technologies, scenario, None, None).unwrap();
    let save_41 = core.save_string().unwrap();
    let save_40 = save_41
        .replacen("\"save_version\":41", "\"save_version\":40", 1)
        .replacen("\"definition_version\":30", "\"definition_version\":29", 1)
        .replacen(",\"bank_stress\":[]", "", 1);
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save_40).unwrap();
    assert!(restored.bank_stress.is_empty());
    assert_eq!(restored.checksum(), core.checksum());
}
