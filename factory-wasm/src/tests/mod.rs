mod construction;
mod contract;
mod earthworks;
mod economy;
mod graph;
mod machines;
mod petroleum;
mod player;
mod power;
mod save;
mod stock;
mod throughput;
mod transport;
mod walls;
mod wire_format;
mod world;
mod worldgen;

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
