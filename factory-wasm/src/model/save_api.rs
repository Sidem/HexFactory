#[derive(Serialize, Deserialize)]
struct SaveEnvelope {
    save_version: u16,
    world_generator_version: u16,
    definition_version: u16,
    technology_version: u16,
    scenario_key: String,
    scenario_version: u16,
    checksum: u32,
    state: SavedState,
}

#[derive(Serialize, Deserialize)]
struct SavedState {
    #[serde(default)]
    boundaries: Vec<Boundary>,
    #[serde(default)]
    ground: Vec<GroundCell>,
    /// Cells whose water has left the generated equilibrium. Defaulted, because a file written
    /// before water could be disturbed describes a world that never departed from it — which is
    /// exactly an empty set, not a missing one.
    #[serde(default)]
    water: Vec<hydrology::WaterCell>,
    /// Non-zero outside-bank stress. Version 40 had none and therefore defaults to the empty set.
    #[serde(default)]
    bank_stress: Vec<geomorphology::StressCell>,
    #[serde(default)]
    spoil: u64,
    seed: u32,
    /// Beside the seed, because a world is both. The overlay a save carries is only meaningful
    /// against the generation it was cut from.
    world_params: WorldParams,
    generated_chunks: Vec<Coordinate>,
    tiles: Vec<TileState>,
    entities: Vec<Entity>,
    #[serde(default)]
    output_routes: BTreeMap<u32, BTreeMap<ItemId, OutputRoute>>,
    #[serde(default)]
    legacy_fluid_belts: BTreeSet<u32>,
    player: PlayerState,
    /// The hex a swing in flight is working. Optional and defaulted, because a save written before
    /// a harvest cost work has no swing to carry and reads back as an idle player — and because
    /// `checksum` hashes an absent one as nothing, such a file still checksums to what it did when
    /// it was written.
    #[serde(default)]
    pending_gather: Option<Coordinate>,
    /// Ground work in flight. Absent before save 45, which means the idle state.
    #[serde(default)]
    pending_ground: Option<GroundEdit>,
    researched: BTreeSet<TechnologyId>,
    #[serde(default)]
    skills: SkillsState,
    next_entity_id: u32,
    tick: u64,
    delivered: u64,
    delivered_by_item: BTreeMap<ItemId, u64>,
    insight: u64,
    victory: bool,
    contract_stage: usize,
    contract_contributed: BTreeMap<ItemId, u64>,
    requests: Vec<RequestState>,
    request_rounds: BTreeMap<RequestId, u32>,
    #[serde(default)]
    request_fills: BTreeMap<RequestId, u32>,
    /// Progress against each project, moved out of the posted slots at save 27 so a pass no longer
    /// destroys it. Defaulted rather than required: the migration writes it, and a file that
    /// somehow lacks it describes a run with nothing part-delivered.
    #[serde(default)]
    request_delivered: BTreeMap<RequestId, u32>,
    produced: BTreeMap<ItemId, u64>,
    /// Whether the run was creative. Checksummed like the rest of this struct, so it cannot be
    /// edited out of a file to turn a creative run back into a priced one.
    creative: bool,
    #[serde(default)]
    ground_items: Vec<GroundItem>,
    #[serde(default)]
    next_ground_item_id: u32,
}

/// The snapshot state the host was last sent, retained so the next delta can be built from the
/// core's dirty marks instead of a freshly materialized snapshot.
///
/// The cheap groups are kept by value and compared directly. Buildings are kept keyed by stable id,
/// so one marked entity costs one rebuild and one comparison rather than a rebuild of the whole
/// blueprint. Terrain and resources are kept by neither: generation is the only path that adds
/// either, and it marks them, so the marks alone are exact.
#[derive(Clone, Debug)]
struct SnapshotBaseline {
    boundaries: Vec<Boundary>,
    ground: Vec<GroundCell>,
    water: Vec<hydrology::WaterCell>,
    spoil: u64,
    scenario: String,
    scenario_name: String,
    world_version: u16,
    seed: u32,
    delivered: u64,
    delivered_by_item: Vec<Ingredient64>,
    insight: u64,
    victory: bool,
    contract: ContractSnapshot,
    requests: Vec<RequestSnapshot>,
    player: PlayerSnapshot,
    researched: Vec<TechnologyId>,
    research_availability: Vec<ResearchAvailability>,
    skills: SkillsSnapshot,
    chunks: Vec<ChunkSnapshot>,
    habitats: BTreeMap<(i32, i32), HabitatSnapshot>,
    buildings: BTreeMap<u32, EntitySnapshot>,
    ground_items: Vec<GroundItem>,
    events: Vec<String>,
}

impl SnapshotBaseline {
    fn from_snapshot(snapshot: &Snapshot) -> Self {
        Self {
            scenario: snapshot.scenario.clone(),
            scenario_name: snapshot.scenario_name.clone(),
            world_version: snapshot.world_version,
            seed: snapshot.seed,
            delivered: snapshot.delivered,
            delivered_by_item: snapshot.delivered_by_item.clone(),
            insight: snapshot.insight,
            victory: snapshot.victory,
            contract: snapshot.contract.clone(),
            requests: snapshot.requests.clone(),
            player: snapshot.player.clone(),
            researched: snapshot.researched.clone(),
            research_availability: snapshot.research_availability.clone(),
            skills: snapshot.skills.clone(),
            chunks: snapshot.chunks.clone(),
            habitats: snapshot
                .habitats
                .iter()
                .map(|cell| ((cell.q, cell.r), *cell))
                .collect(),
            buildings: snapshot
                .buildings
                .iter()
                .map(|entity| (entity.id, entity.clone()))
                .collect(),
            boundaries: snapshot.boundaries.clone(),
            ground: snapshot.ground.clone(),
            water: snapshot.water.clone(),
            spoil: snapshot.spoil,
            ground_items: snapshot.ground_items.clone(),
            events: snapshot.events.clone(),
        }
    }
}

/// Advance one baseline field, yielding the delta entry only when it actually changed.
fn take_changed<T: Clone + PartialEq>(baseline: &mut T, current: T) -> Option<T> {
    (*baseline != current).then(|| {
        baseline.clone_from(&current);
        current
    })
}

fn take_changed_copy<T: Copy + PartialEq>(baseline: &mut T, current: T) -> Option<T> {
    (*baseline != current).then(|| {
        *baseline = current;
        current
    })
}

#[wasm_bindgen]
pub struct Factory {
    definitions: DefinitionsInput,
    technologies: TechnologiesInput,
    scenarios: ScenariosInput,
    core: Core,
    snapshot_revision: u64,
    baseline: Option<SnapshotBaseline>,
}

impl Factory {

}

/// The largest preview native will raster, per side. A preview is a picture on a settings panel
/// rather than a viewport, and one pixel of it costs seven elevations, so the ceiling lives here
/// rather than in whatever asks for one.
const MAX_PREVIEW_SIDE: u32 = 480;
/// The widest span a preview may frame, in hexes. Sized well above the largest shipped landform
/// cell rather than to a round number: a preview that could not frame one landform would be a
/// picture of noise.
const MAX_PREVIEW_SPAN: u32 = 16_384;
/// How many deposits a preview will plot before it reports a count instead.
///
/// Deposits stand a dozen hexes apart, so a window wide enough to frame a coastline holds tens of
/// thousands of them. Drawn, that is a texture rather than a map, and sent, it is a megabyte of JSON
/// per slider nudge. Past this the overlay says how many there are and leaves the terrain visible.
const MAX_PREVIEW_SITES: usize = 1_200;
/// The most lattice cells a preview will walk. A span of `MAX_PREVIEW_SPAN` over a `site_cell` of
/// one is a legal parameter set and tens of millions of cells; the count above is a property of what
/// came out, and this is the bound on the looking.
const MAX_PREVIEW_SITE_CELLS: i64 = 262_144;

/// One deposit site as a preview draws it: where its centre lands in preview pixels, how far it
/// reaches there, and what it holds.
#[derive(Serialize)]
struct PreviewSite {
    item_id: ItemId,
    x: i32,
    y: i32,
    radius: i32,
}

/// Why one guarantee could not be placed, in the terms the panel explains it in.
#[derive(Serialize)]
struct PreviewNeed {
    item_id: ItemId,
    /// The bands a rule could seat this material's centre in.
    bands: Vec<Terrain>,
    /// Whether the opening holds any of those bands at all. False is "this world has no such ground
    /// near the landing site", which no seed will fix; true is "the ground is there and no patch on
    /// it was big enough", which one often will.
    ground: bool,
}

/// One knob a repair turns, named as the form names it so the host can label it without a table of
/// its own.
#[derive(Serialize)]
struct PreviewChange {
    field: &'static str,
    from: i32,
    to: i32,
}

/// A verified way out of a world that cannot be started. Both halves are optional and both may be
/// present: they are two different prices, and which one is worth paying is the player's call.
#[derive(Serialize)]
struct PreviewRepair {
    /// A seed that opens the world with every parameter left where the player put it.
    seed: Option<u32>,
    /// Parameter changes that open the world with the seed left alone. Empty when the ladder found
    /// nothing, which is itself worth saying: it means the shape of this world is the problem.
    changes: Vec<PreviewChange>,
}

#[derive(Serialize)]
struct PreviewSites {
    sites: Vec<PreviewSite>,
    /// Deposits the window holds, which is not always how many of them are in `sites`.
    total: u32,
    /// Whether the window holds more deposits than are worth drawing, or more than were counted.
    /// Set either way, so an empty `sites` never has to be read as "this world has no deposits".
    dense: bool,
    /// Materials the bootstrap pass could not place anywhere. `Core::new` refuses a world over
    /// exactly this list, so it travels with the picture rather than being discovered on start.
    unmet: Vec<ItemId>,
    /// What each of those materials was looking for. Empty whenever `unmet` is.
    needs: Vec<PreviewNeed>,
    /// A way out, when one was found. Searched only for a world that is already refused, so a
    /// parameter set that opens costs nothing to preview.
    repair: Option<PreviewRepair>,
}

/// Every scalar a parameter set carries, under the name the form gives it. `site_rules` is the one
/// field left out: it is a table rather than a knob, and nothing here moves it.
const WORLD_SCALARS: [(&str, fn(&WorldParams) -> i32); 17] = [
    ("elevation_coarse_cell", |p| p.elevation_coarse_cell),
    ("elevation_fine_cell", |p| p.elevation_fine_cell),
    ("elevation_coarse_weight", |p| p.elevation_coarse_weight),
    ("moisture_cell", |p| p.moisture_cell),
    ("richness_cell", |p| p.richness_cell),
    ("water_level", |p| p.water_level),
    ("shore_level", |p| p.shore_level),
    ("hills_level", |p| p.hills_level),
    ("highland_level", |p| p.highland_level),
    ("cliff_step", |p| p.cliff_step),
    ("deep_water_moisture", |p| p.deep_water_moisture),
    ("site_cell", |p| p.site_cell),
    ("site_jitter", |p| p.site_jitter),
    ("river_cell", |p| p.river_cell),
    ("river_width", |p| p.river_width),
    ("river_max_elevation", |p| p.river_max_elevation),
    ("ocean_level", |p| p.ocean_level),
];

/// What a repair did, as a diff rather than as a list the repair writes for itself. A move that
/// turned a knob nobody expected still reports that knob, which is the property worth having: the
/// button says what it is about to change because the change is read off the result.
fn world_changes(before: &WorldParams, after: &WorldParams) -> Vec<PreviewChange> {
    WORLD_SCALARS
        .iter()
        .filter_map(|&(field, read)| {
            let (from, to) = (read(before), read(after));
            (from != to).then_some(PreviewChange { field, from, to })
        })
        .collect()
}

impl Factory {

}

#[wasm_bindgen]
impl Factory {

}

fn parse_json<T: for<'de> Deserialize<'de>>(json: &str) -> Result<T, JsValue> {
    serde_json::from_str(json).map_err(|error| js_error(error.to_string()))
}

fn js_error(error: impl AsRef<str>) -> JsValue {
    JsValue::from_str(error.as_ref())
}

/// What the host may name a world with: a preset key, or a complete parameter set. Both are the
/// same table read at two depths — the preset is the usable surface and the parameter set is the
/// maintainable one — so the caller picking one is not picking a different mechanism.
#[derive(Deserialize)]
#[serde(untagged)]
enum WorldParamsInput {
    Preset { preset: String },
    Params(Box<WorldParams>),
}

/// `None` means "whatever the scenario names", which is how every call site that does not care
/// about generation stays unaware that parameters exist.
fn parse_world_params(json: Option<&str>) -> Result<Option<WorldParams>, JsValue> {
    let Some(json) = json.map(str::trim).filter(|json| !json.is_empty()) else {
        return Ok(None);
    };
    world_params_from_json(json).map(Some).map_err(js_error)
}

/// The same read with the failure left as a string.
///
/// A `JsValue` can only be constructed inside wasm — building one on the host aborts the process —
/// so anything a native test drives has to fail in `String` and be wrapped at the export.
fn world_params_from_json(json: &str) -> Result<WorldParams, String> {
    let input: WorldParamsInput = serde_json::from_str(json)
        .map_err(|error| format!("malformed world parameters: {error}"))?;
    Ok(match input {
        WorldParamsInput::Preset { preset } => {
            preset_params(&preset).ok_or_else(|| format!("unknown world preset {preset}"))?
        }
        WorldParamsInput::Params(params) => *params,
    })
}
