use super::*;

pub mod junction;
pub mod steady;

/// Monotonic microseconds. Only differences between readings are meaningful, and a platform's
/// reading may be quantized — the browser clamps `performance.now` unless the page is
/// cross-origin isolated — so every phase below times many samples at once.
pub trait Clock {
    fn now_us(&self) -> f64;
}

#[cfg(not(target_arch = "wasm32"))]
pub struct SystemClock(std::time::Instant);

#[cfg(not(target_arch = "wasm32"))]
impl SystemClock {
    pub fn new() -> Self {
        Self(std::time::Instant::now())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Clock for SystemClock {
    fn now_us(&self) -> f64 {
        self.0.elapsed().as_secs_f64() * 1e6
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn performance_now() -> f64;
}

/// `performance.now` in both a window and a worker global scope, converted to microseconds.
#[cfg(target_arch = "wasm32")]
pub struct PerformanceClock;

#[cfg(target_arch = "wasm32")]
impl Clock for PerformanceClock {
    fn now_us(&self) -> f64 {
        performance_now() * 1e3
    }
}

/// How long a phase must run before its mean is trusted.
///
/// A native clock resolves nanoseconds, so a fixed sample count is enough and the budget is
/// zero. A browser clamps `performance.now` to 100 µs unless the page is cross-origin
/// isolated, which is coarser than most of the phases below; there, a phase repeats its sample
/// block until it has run long enough for that step to be a rounding error. Only the sample
/// count changes, never the workload, so both records stay per-unit comparable.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    pub min_phase_us: f64,
}

impl Budget {
    /// Run each phase exactly once through its sample block.
    pub const FIXED: Budget = Budget { min_phase_us: 0.0 };

    /// 20 ms, which holds a 100 µs clock step to 0.5% of a phase.
    pub const CLAMPED_CLOCK: Budget = Budget {
        min_phase_us: 20_000.0,
    };
}

/// Time one phase, repeating its sample block until the budget is met, and report the mean
/// cost per sample together with the number of samples that produced it.
fn phase(
    clock: &dyn Clock,
    budget: Budget,
    samples_per_block: u32,
    mut block: impl FnMut(),
) -> (f64, u32) {
    let start = clock.now_us();
    let mut samples = 0u32;
    loop {
        block();
        samples = samples.saturating_add(samples_per_block);
        let elapsed = (clock.now_us() - start).max(0.0);
        if elapsed >= budget.min_phase_us || samples_per_block == 0 {
            return (mean(elapsed, samples), samples);
        }
    }
}

pub fn default_clock() -> Box<dyn Clock> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        Box::new(SystemClock::new())
    }
    #[cfg(target_arch = "wasm32")]
    {
        Box::new(PerformanceClock)
    }
}

const DEFINITIONS: &str = include_str!("../../src/data/definitions.json");
const TECHNOLOGIES: &str = include_str!("../../src/data/technologies.json");

const EXTRACTOR: DefinitionId = 1;
const BELT: DefinitionId = 2;
const COMPOSER: DefinitionId = 3;
const CONTAINER: DefinitionId = 4;
const CONSUMER: DefinitionId = 5;
const COMPONENT_RECIPE: RecipeId = 1;
const ORE: ItemId = 1;

/// Report format version, so recorded JSON stays interpretable as the metric set changes.
/// Version 2 adds `checksum_us`, which the sparse-snapshot release needed to see. Version 3
/// adds `platform`, because the same ladder now runs natively and as wasm in a browser worker
/// and a record must say which one it is. Version 4 adds `delta_json_bytes`, and changes what
/// `delta_bytes` means: it is now the binary wire payload the game ships rather than the JSON
/// one, so the two figures are not comparable across the boundary between schema 3 and 4.
pub const REPORT_SCHEMA: u32 = 4;
/// Lines sit three rows apart so one line's three-cell composer cannot touch the next.
const ROW_PITCH: i32 = 3;
/// How far east of its anchor each multi-cell machine in the workload reaches, so the line is
/// spaced by the catalogue's own footprints rather than by a remembered one-cell world.
const EXTRACTOR_CELLS: i32 = 2;
const COMPOSER_CELLS: i32 = 2;
/// The first belt of a line, and so the workload's rotate target. It is a belt rather than the
/// extractor it sits beside, because rotating a source is a different edit to rotating a link.
const EDIT_TARGET_Q: i32 = EXTRACTOR_CELLS;
/// Large enough that no deposit empties inside a measured run, so every tier measures the same
/// steady state rather than a decaying one.
const DEPOSIT_QUANTITY: u32 = 1_000_000;
/// Reach far past the generated blueprint so edit measurements are never range-rejected.
const BUILD_RANGE_HEXES: u32 = 100_000;
/// The bounded idle batch the host sends on a frame with no held key.
const IDLE_COMMANDS: &str = "[{\"type\":\"move_intent\",\"x\":0,\"y\":0}]";
/// Rotation restores a belt's original orientation every six edits.
const ROTATION_CYCLE: u32 = 6;

/// Which blueprint a tier repeats.
///
/// The two shapes measure different things and neither stands in for the other. `Line` is the
/// straight `extractor → belts → composer → belt → container → belt → consumer` chain every
/// recorded ladder is expressed in, where transport is a directed chain with no arbitration in it
/// at all. `Junction` is the dense splitter/merger/underpass unit in [`junction`], where the
/// junction primitives carry the whole of the workload's throughput. A tier names one; nothing
/// infers it from an entity count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Line,
    Junction,
}

/// One measured tier: `lines` independent repeats of the layout's blueprint.
///
/// Sample budgets shrink as tiers grow so a complete run stays interactive; per-unit results
/// stay comparable because every metric is reported per tick, per frame, or per edit.
#[derive(Clone, Copy, Debug)]
pub struct TierSpec {
    pub key: &'static str,
    pub layout: Layout,
    pub lines: u32,
    pub belt_span: u32,
    pub warmup_ticks: u32,
    pub measured_ticks: u32,
    pub frames: u32,
    pub snapshots: u32,
    pub edits: u32,
}

impl TierSpec {
    /// Entities per repeat of the blueprint: for a line, its extractor, transport belts, composer,
    /// output belt, container, delivery belt and consumer; for a junction, the fixed
    /// [`junction::ENTITIES_PER_UNIT`].
    pub fn entities_per_line(&self) -> u32 {
        match self.layout {
            Layout::Line => self.belt_span + 6,
            Layout::Junction => junction::ENTITIES_PER_UNIT,
        }
    }

    pub fn entities(&self) -> u32 {
        self.lines * self.entities_per_line()
    }
}

/// Measured cost for one tier. Every field is a primitive so the report stays a stable,
/// machine-readable record.
#[derive(Clone, Debug, Serialize)]
pub struct TierResult {
    pub key: String,
    pub lines: u32,
    pub entities: usize,
    pub tiles: usize,
    pub chunks: usize,
    /// Ticks actually timed. Equal to the tier's tick budget under `Budget::FIXED`, and a
    /// multiple of it when a coarse clock made the phase repeat.
    pub measured_ticks: u32,
    /// Mean cost of one simulation tick with no snapshot or serialization.
    pub tick_us: f64,
    pub ticks_per_second: f64,
    /// Mean cost of building one complete native snapshot, before serialization. The shipped
    /// frame no longer pays this — it is the host's first frame, and the baseline the
    /// incremental delta is measured against.
    pub snapshot_us: f64,
    /// Mean cost of one native checksum. Every delta carries one, so this is a floor under the
    /// frame that no amount of snapshot sparsity can remove.
    pub checksum_us: f64,
    /// Mean cost of one worker frame: bounded command batch, one tick, and a serialized delta.
    pub frame_us: f64,
    pub frames_per_second: f64,
    /// Mean encoded delta payload crossing the worker boundary per frame, in the binary wire
    /// format the game ships.
    pub delta_bytes: f64,
    /// What the same frames would have cost as JSON, which is what they did cost until the
    /// binary wire replaced it. Recorded beside `delta_bytes` so the encoding's saving is a
    /// measured ratio in the record rather than an inference from two different runs.
    pub delta_json_bytes: f64,
    /// Mean cost of one full deterministic transport compile.
    pub full_compile_us: f64,
    /// Mean cost of the incremental transport machinery alone, for the same edit: stable-ID
    /// link capture plus affected-component recompilation. Directly comparable to
    /// `full_compile_us`.
    pub incremental_recompile_us: f64,
    /// Mean cost of one complete public rotate edit, including legality checks. The difference
    /// from `incremental_recompile_us` is what the edit path spends outside transport.
    pub edit_us: f64,
    /// Native checksum after the measured tick phase, pinning the workload against drift.
    pub checksum: u32,
    pub delivered: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub schema: u32,
    pub crate_version: String,
    pub profile: String,
    /// `native` or `wasm32`. The build reports it rather than the caller, so a record cannot
    /// claim a platform it was not measured on.
    pub platform: String,
    pub tiers: Vec<TierResult>,
}

fn platform() -> String {
    if cfg!(target_arch = "wasm32") {
        "wasm32".into()
    } else {
        "native".into()
    }
}

/// The recorded tier ladder. It spans one line to a blueprint far past anything the current
/// game asks a player to build, so the measurement shows where cost stops being linear.
pub fn default_tiers() -> Vec<TierSpec> {
    vec![
        tier("line", 1, 2000, 400, 400, 60),
        tier("small", 16, 1000, 200, 200, 60),
        tier("medium", 64, 400, 100, 100, 60),
        tier("wide", 128, 240, 60, 60, 60),
        tier("large", 256, 120, 40, 40, 30),
        tier("xlarge", 512, 60, 20, 20, 12),
    ]
}

/// A reduced ladder for smoke coverage inside the test gate.
pub fn quick_tiers() -> Vec<TierSpec> {
    vec![tier("line", 1, 20, 5, 5, 6), tier("small", 16, 20, 5, 5, 6)]
}

fn tier(
    key: &'static str,
    lines: u32,
    measured_ticks: u32,
    frames: u32,
    snapshots: u32,
    edits: u32,
) -> TierSpec {
    tier_on(
        Layout::Line,
        key,
        lines,
        measured_ticks,
        frames,
        snapshots,
        edits,
    )
}

/// The same tier on a named layout. Every field but `layout` means what it always did, and
/// `warmup_ticks` is the layout-independent floor — a workload that needs more says so itself.
fn tier_on(
    layout: Layout,
    key: &'static str,
    lines: u32,
    measured_ticks: u32,
    frames: u32,
    snapshots: u32,
    edits: u32,
) -> TierSpec {
    TierSpec {
        key,
        layout,
        lines,
        belt_span: 6,
        // Long enough for the first components to reach the consumer, so every tier is timed
        // with cargo actually moving. The belt run sets this floor now that a hex of belt is
        // 5.37 m of conveyor: eight belts at `BELT_TRANSIT_TICKS` is 216 ticks of travel on its
        // own, on top of the 30 ticks a tier-one extractor spends on one ore and the craft
        // between them. The measured window is unchanged, and so is what it measures — the line
        // is extraction-bound either way — but it now starts after a longer pipeline has
        // filled.
        warmup_ticks: 400,
        measured_ticks,
        frames,
        snapshots,
        edits,
    }
}

fn catalogs() -> (DefinitionsInput, TechnologiesInput) {
    let mut definitions: DefinitionsInput =
        serde_json::from_str(DEFINITIONS).expect("shipped definitions parse");
    // This synthetic transport workload keeps its historical two-ore/eight-tick recipe.
    // v0.33's gameplay component needs upstream smelting and gears; silently swapping that
    // into this isolated line would benchmark a stalled machine and invalidate old records.
    let recipe = definitions
        .recipes
        .iter_mut()
        .find(|recipe| recipe.id == COMPONENT_RECIPE)
        .unwrap();
    recipe.inputs = vec![Ingredient {
        item_id: ORE,
        quantity: 2,
    }];
    recipe.duration = 8;
    recipe.output = Ingredient {
        item_id: 2,
        quantity: 1,
    };
    recipe.fuel = 0;
    (
        definitions,
        serde_json::from_str(TECHNOLOGIES).expect("shipped technologies parse"),
    )
}

fn placed(
    q: i32,
    r: i32,
    definition_id: DefinitionId,
    recipe_id: Option<RecipeId>,
) -> PlacedBuilding {
    PlacedBuilding {
        q,
        r,
        definition_id,
        // Every line runs east, so compiled transport is a straight directed chain.
        orientation: 0,
        recipe_id,
        // Left unowned so the edit phase can exercise the ordinary player rotate path.
        scenario_owned: false,
    }
}

/// Build the synthetic scenario for a tier. It is an ordinary scenario definition, validated by
/// the same rules as the shipped catalog.
pub(crate) fn tier_scenario(spec: &TierSpec) -> ScenarioDefinition {
    let (resources, buildings) = match spec.layout {
        Layout::Line => line_blueprint(spec),
        Layout::Junction => junction::blueprint(spec.lines),
    };
    ScenarioDefinition {
        id: 1,
        key: format!("capacity-{}", spec.key),
        name: format!("Capacity tier {}", spec.key),
        description: "Synthetic steady-state capacity workload".into(),
        version: 1,
        seed: 2_071_003_907,
        // Generation is off below, so a preset would name a table nothing reads.
        world_preset: None,
        chunk_size: 8,
        // Terrain is uniform lowland so a tier measures transport and machines, not the
        // incidental obstacle layout of a generated seed.
        generated_environment: false,
        // Away from every line, so the idle player never blocks a footprint.
        player_spawn: Coordinate { q: -6, r: -6 },
        player_facing: 0,
        build_range: BUILD_RANGE_HEXES,
        // The workload's player never picks anything up, so this only has to be valid.
        carry_slots: 12,
        contract: ContractDefinition {
            key: "capacity".into(),
            name: "Capacity workload".into(),
            stages: vec![ContractStage {
                key: "steady-state".into(),
                name: "Run the line".into(),
                brief: "A measured workload rather than a game.".into(),
                reads: "nothing — the harness draws no hub".into(),
                // Never reached, so a completed stage cannot change the measured workload
                // partway through. The harness delivers into a consumer in any case, and a
                // consumer is deliberately not the hub.
                requirements: vec![Ingredient {
                    item_id: 2,
                    quantity: u32::MAX,
                }],
            }],
        },
        initial_inventory: Vec::new(),
        // Only what the layout actually places. The line blueprint's set is unchanged, because a
        // researched technology is core state and widening it would move every recorded checksum.
        initial_researched: match spec.layout {
            Layout::Line => vec![1, 2, 3, 4],
            Layout::Junction => junction::RESEARCHED.to_vec(),
        },
        resources,
        buildings,
    }
}

/// The straight-line blueprint: `spec.lines` independent chains, [`ROW_PITCH`] rows apart.
fn line_blueprint(spec: &TierSpec) -> (Vec<ScenarioResource>, Vec<PlacedBuilding>) {
    let mut resources = Vec::new();
    let mut buildings = Vec::new();
    for line in 0..spec.lines {
        let r = line as i32 * ROW_PITCH;
        resources.push(ScenarioResource {
            q: 0,
            r,
            item_id: ORE,
            quantity: DEPOSIT_QUANTITY,
        });
        buildings.push(placed(0, r, EXTRACTOR, None));
        // Machines stand on more than their anchor now, so the line is laid out from each
        // one's eastern edge rather than from its anchor. The belt span, the building count
        // and the order of the chain are unchanged; only the empty ground between them moved.
        let belt_start = EXTRACTOR_CELLS;
        for q in belt_start..belt_start + spec.belt_span as i32 {
            buildings.push(placed(q, r, BELT, None));
        }
        let composer_q = belt_start + spec.belt_span as i32;
        buildings.push(placed(composer_q, r, COMPOSER, Some(COMPONENT_RECIPE)));
        let tail_q = composer_q + COMPOSER_CELLS;
        buildings.push(placed(tail_q, r, BELT, None));
        buildings.push(placed(tail_q + 1, r, CONTAINER, None));
        buildings.push(placed(tail_q + 2, r, BELT, None));
        buildings.push(placed(tail_q + 3, r, CONSUMER, None));
    }
    (resources, buildings)
}

/// A warmed core for a tier, advanced far enough that cargo is already flowing.
pub(crate) fn warm_core(spec: &TierSpec) -> Core {
    let (definitions, mut technologies) = catalogs();
    technologies.skills.clear();
    technologies.skill_milestones.clear();
    let scenario = tier_scenario(spec);
    validate_all(
        &definitions,
        &technologies,
        &ScenariosInput {
            version: 1,
            scenarios: vec![scenario.clone()],
        },
    )
    .expect("capacity scenario is valid");
    let mut core = Core::new(&definitions, &technologies, &scenario, None, None)
        .expect("capacity core builds");
    // The ladder measures transport, not the power constraint. Unmetered supply keeps
    // delivered totals and the tick path honest without adding a pole per line.
    core.power_unmetered = true;
    core.advance_ticks(spec.warmup_ticks);
    core
}

/// A warmed `Factory` for a tier, ready for the host to drive over the ordinary worker RPC.
/// The browser harness measures its round trip through exactly this object, so the boundary
/// cost is measured against the same steady state the in-wasm phases are.
pub(crate) fn warm_factory(spec: &TierSpec) -> Factory {
    let (definitions, mut technologies) = catalogs();
    technologies.skills.clear();
    technologies.skill_milestones.clear();
    let scenario = tier_scenario(spec);
    Factory {
        definitions,
        technologies,
        scenarios: ScenariosInput {
            version: 1,
            scenarios: vec![scenario],
        },
        core: warm_core(spec),
        snapshot_revision: 0,
        baseline: None,
    }
}

pub fn measure_tier(spec: &TierSpec) -> TierResult {
    measure_tier_with(spec, default_clock().as_ref(), Budget::FIXED)
}

pub fn measure_tier_with(spec: &TierSpec, clock: &dyn Clock, budget: Budget) -> TierResult {
    let mut core = warm_core(spec);
    let entities = core.entities.len();
    let tiles = core.tiles.len();
    let chunks = core.generated_chunks.len();

    let (tick_us, measured_ticks) = phase(clock, budget, spec.measured_ticks, || {
        core.advance_ticks(spec.measured_ticks)
    });

    let (snapshot_us, _) = phase(clock, budget, spec.snapshots, || {
        for _ in 0..spec.snapshots {
            let snapshot = core.snapshot();
            std::hint::black_box(&snapshot);
        }
    });

    let (checksum_us, _) = phase(clock, budget, spec.snapshots, || {
        for _ in 0..spec.snapshots {
            std::hint::black_box(core.checksum());
        }
    });

    // Pinned on its own core, advanced exactly once through the tier's tick budget. A browser
    // run repeats the timed phase and therefore ends somewhere else entirely; taking the
    // workload's identity from here is what keeps its checksum comparable to a native record.
    let (checksum, delivered) = pinned_state(spec);

    let (frame_us, delta_bytes) = measure_frames(spec, clock, budget);
    let delta_json_bytes = measure_json_payload(spec);
    let full_compile_us = measure_full_compile(spec, clock, budget);
    let incremental_recompile_us = measure_recompiles(spec, clock, budget);
    let edit_us = measure_edits(spec, clock, budget);

    TierResult {
        key: spec.key.into(),
        lines: spec.lines,
        entities,
        tiles,
        chunks,
        measured_ticks,
        tick_us,
        ticks_per_second: rate(tick_us),
        snapshot_us,
        checksum_us,
        frame_us,
        frames_per_second: rate(frame_us),
        delta_bytes,
        delta_json_bytes,
        full_compile_us,
        incremental_recompile_us,
        edit_us,
        checksum,
        delivered,
    }
}

/// The tier's identity: the checksum and delivered total after exactly one tick budget from a
/// warm core. Recorded rather than timed, so it cannot move with the sample count.
fn pinned_state(spec: &TierSpec) -> (u32, u64) {
    let mut core = warm_core(spec);
    core.advance_ticks(spec.measured_ticks);
    (core.checksum(), core.delivered)
}

/// One worker frame, measured through the exact entry points the host RPC calls.
fn measure_frames(spec: &TierSpec, clock: &dyn Clock, budget: Budget) -> (f64, f64) {
    let mut factory = warm_factory(spec);
    // The first delta is a complete snapshot; take it outside the measurement so the reported
    // payload is the steady-state per-frame cost.
    let _ = factory.snapshot_delta_bytes();
    let mut bytes = 0usize;
    let (frame_us, frames) = phase(clock, budget, spec.frames, || {
        for _ in 0..spec.frames {
            // No player steps: the capacity workload measures the factory, and the idle player
            // has no movement intent to spend them on anyway.
            if factory.advance_json(IDLE_COMMANDS, 1, 0).is_err() {
                panic!("capacity frame commands must be accepted");
            }
            bytes += factory.snapshot_delta_bytes().len();
        }
    });
    (frame_us, mean(bytes as f64, frames))
}

/// The same frames' payload had they been encoded as JSON, which is what they were until the
/// binary wire landed.
///
/// A second factory rather than a second call, because building a delta consumes the dirty
/// marks and advances the baseline, so one frame cannot be asked for both encodings. The
/// workload is deterministic, so this run produces the identical sequence of deltas — and it is
/// untimed, because what is wanted from it is the byte count the shipped encoding is measured
/// against, not the cost of an encoding the game no longer performs.
fn measure_json_payload(spec: &TierSpec) -> f64 {
    let mut factory = warm_factory(spec);
    let _ = factory.snapshot_delta_json();
    let mut bytes = 0usize;
    for _ in 0..spec.frames {
        if factory.advance_json(IDLE_COMMANDS, 1, 0).is_err() {
            panic!("capacity frame commands must be accepted");
        }
        bytes += factory.snapshot_delta_json().len();
    }
    mean(bytes as f64, spec.frames)
}

/// The full deterministic compile used on load and restore, as the incremental baseline.
fn measure_full_compile(spec: &TierSpec, clock: &dyn Clock, budget: Budget) -> f64 {
    let mut core = warm_core(spec);
    let samples = spec.edits.max(1);
    phase(clock, budget, samples, || {
        for _ in 0..samples {
            core.compile_graph();
        }
    })
    .0
}

/// The complete public rotate path. Rotating a belt through all six orientations covers edits
/// that merge and split neighbouring components, not only the cheap self-contained case.
fn measure_edits(spec: &TierSpec, clock: &dyn Clock, budget: Budget) -> f64 {
    let mut core = warm_core(spec);
    let edits = rotation_edits(spec);
    if edits == 0 {
        return 0.0;
    }
    phase(clock, budget, edits, || {
        for edit in 0..edits {
            // Spread edits across lines so no single component stays warm in cache.
            core.rotate(EDIT_TARGET_Q, edit_row(spec, edit), false)
                .expect("capacity belt rotates");
        }
    })
    .0
}

/// The incremental transport machinery alone, driving the same rotations. Isolating it from
/// the edit path's legality checks is what makes the comparison against a full compile fair.
fn measure_recompiles(spec: &TierSpec, clock: &dyn Clock, budget: Budget) -> f64 {
    let mut core = warm_core(spec);
    let edits = rotation_edits(spec);
    if edits == 0 {
        return 0.0;
    }
    // Entity lookup is part of the edit path, not the transport machinery, so resolve targets
    // before timing. No entity is added or removed here, so the indices stay valid.
    let targets: Vec<(usize, u32)> = (0..edits)
        .map(|edit| {
            let index = core
                .entity_at(EDIT_TARGET_Q, edit_row(spec, edit))
                .expect("capacity belt exists");
            (index, core.entities[index].id)
        })
        .collect();

    phase(clock, budget, edits, || {
        for &(index, id) in &targets {
            let old_links = core.graph_links_by_id();
            let old_footprint = core.entity_footprint(&core.entities[index]);
            let orientation = (core.entities[index].placed.orientation + 1) % 6;
            let next_footprint = core.footprint_for(core.entities[index].placed, orientation);
            core.entities[index].placed.orientation = orientation;
            let changed_cells = old_footprint
                .into_iter()
                .chain(next_footprint)
                .map(|cell| (cell.q, cell.r))
                .collect();
            core.recompile_graph_components(&old_links, &changed_cells, &BTreeSet::from([id]));
        }
    })
    .0
}

fn rotation_edits(spec: &TierSpec) -> u32 {
    spec.edits - (spec.edits % ROTATION_CYCLE)
}

fn edit_row(spec: &TierSpec, edit: u32) -> i32 {
    ((edit / ROTATION_CYCLE) % spec.lines) as i32 * ROW_PITCH
}

pub fn run(specs: &[TierSpec]) -> Report {
    run_with(specs, |_| {})
}

/// Run the ladder, reporting each tier as it completes so a long run shows progress.
pub fn run_with(specs: &[TierSpec], mut observe: impl FnMut(&TierResult)) -> Report {
    let clock = default_clock();
    let mut ladder = Ladder::new(specs.to_vec());
    for index in 0..ladder.len() {
        let result = ladder
            .measure(index, clock.as_ref())
            .expect("ladder index is in range");
        observe(&result);
    }
    ladder.report()
}

/// The ladder as resumable state: one tier is measured per call, and the report is assembled
/// from whatever has been measured so far.
///
/// A native run has no reason to stop between tiers, but a browser one does — the harness
/// reports each tier as it lands and yields to the event loop in between — so both drive the
/// ladder through this one type instead of two loops that could drift apart.
pub struct Ladder {
    specs: Vec<TierSpec>,
    tiers: Vec<TierResult>,
    budget: Budget,
}

impl Ladder {
    pub fn new(specs: Vec<TierSpec>) -> Self {
        Self {
            specs,
            tiers: Vec::new(),
            budget: Budget::FIXED,
        }
    }

    /// Give every phase a minimum duration, for a platform whose clock is too coarse to time
    /// a fixed sample block.
    pub fn with_budget(mut self, budget: Budget) -> Self {
        self.budget = budget;
        self
    }

    pub fn len(&self) -> usize {
        self.specs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    pub fn spec(&self, index: usize) -> Option<&TierSpec> {
        self.specs.get(index)
    }

    pub fn specs(&self) -> &[TierSpec] {
        &self.specs
    }

    /// Measure one tier and retain it for the report. Measuring the same index twice replaces
    /// the earlier result rather than recording the tier twice.
    pub fn measure(&mut self, index: usize, clock: &dyn Clock) -> Option<TierResult> {
        let spec = *self.specs.get(index)?;
        let result = measure_tier_with(&spec, clock, self.budget);
        match self.tiers.iter_mut().find(|tier| tier.key == spec.key) {
            Some(existing) => *existing = result.clone(),
            None => self.tiers.push(result.clone()),
        }
        Some(result)
    }

    pub fn report(&self) -> Report {
        Report {
            schema: REPORT_SCHEMA,
            crate_version: env!("CARGO_PKG_VERSION").into(),
            profile: if cfg!(debug_assertions) {
                "debug".into()
            } else {
                "release".into()
            },
            platform: platform(),
            tiers: self.tiers.clone(),
        }
    }
}

pub fn format_json(report: &Report) -> String {
    serde_json::to_string_pretty(report).expect("report is serializable")
}

pub fn table_header() -> String {
    format!(
        "{:<8}{:>7}{:>10}{:>11}{:>10}{:>12}{:>12}{:>11}{:>10}{:>13}{:>13}{:>12}{:>13}{:>10}",
        "tier",
        "lines",
        "entities",
        "tick us",
        "ticks/s",
        "snapshot us",
        "checksum us",
        "frame us",
        "frames/s",
        "delta bytes",
        "json bytes",
        "compile us",
        "recompile us",
        "edit us",
    )
}

pub fn table_row(tier: &TierResult) -> String {
    format!(
            "{:<8}{:>7}{:>10}{:>11.1}{:>10.0}{:>12.1}{:>12.1}{:>11.1}{:>10.0}{:>13.0}{:>13.0}{:>12.1}{:>13.1}{:>10.1}",
            tier.key,
            tier.lines,
            tier.entities,
            tier.tick_us,
            tier.ticks_per_second,
            tier.snapshot_us,
            tier.checksum_us,
            tier.frame_us,
            tier.frames_per_second,
            tier.delta_bytes,
            tier.delta_json_bytes,
            tier.full_compile_us,
            tier.incremental_recompile_us,
            tier.edit_us,
        )
}

pub fn format_table(report: &Report) -> String {
    let mut lines = vec![
        format!(
            "HexFactory capacity tiers — factory-wasm {} ({} {} profile)",
            report.crate_version, report.platform, report.profile
        ),
        table_header(),
    ];
    lines.extend(report.tiers.iter().map(table_row));
    lines.join("\n")
}

fn mean(total: f64, samples: u32) -> f64 {
    if samples == 0 {
        0.0
    } else {
        total / f64::from(samples)
    }
}

fn rate(microseconds: f64) -> f64 {
    if microseconds <= 0.0 {
        0.0
    } else {
        1e6 / microseconds
    }
}

/// The browser entry point for the same ladder, built only by `--features bench`.
///
/// The harness drives one tier per call so the page can report progress, and can hand back a
/// warmed `Factory` for the tier so the host can measure what the game actually pays per
/// frame: the worker RPC round trip around these same native phases.
#[cfg(all(target_arch = "wasm32", feature = "bench"))]
#[wasm_bindgen]
pub struct CapacityBench {
    ladder: Ladder,
    clock: PerformanceClock,
}

#[cfg(all(target_arch = "wasm32", feature = "bench"))]
#[wasm_bindgen]
impl CapacityBench {
    #[wasm_bindgen(constructor)]
    pub fn new(quick: bool) -> CapacityBench {
        CapacityBench {
            ladder: Ladder::new(if quick {
                quick_tiers()
            } else {
                default_tiers()
            })
            .with_budget(Budget::CLAMPED_CLOCK),
            clock: PerformanceClock,
        }
    }

    pub fn tier_count(&self) -> usize {
        self.ladder.len()
    }

    /// `{ key, lines, entities }` for every tier, so the page can list the run before it
    /// starts instead of discovering its shape as results arrive.
    pub fn tiers_json(&self) -> String {
        let tiers: Vec<serde_json::Value> = self
            .ladder
            .specs()
            .iter()
            .map(|spec| {
                serde_json::json!({
                    "key": spec.key,
                    "lines": spec.lines,
                    "entities": spec.entities(),
                    // The host times its round trip over the same frame budget the in-wasm
                    // frame phase uses, so the two costs describe the same amount of work.
                    "frames": spec.frames,
                })
            })
            .collect();
        serde_json::Value::Array(tiers).to_string()
    }

    /// Measure one tier, returning its `TierResult` as JSON.
    pub fn measure(&mut self, index: usize) -> Result<String, JsValue> {
        let result = self
            .ladder
            .measure(index, &self.clock)
            .ok_or_else(|| js_error(format!("no capacity tier at index {index}")))?;
        serde_json::to_string(&result).map_err(|error| js_error(error.to_string()))
    }

    /// A warmed factory for the tier, in the same steady state the in-wasm phases measure.
    pub fn factory(&self, index: usize) -> Result<Factory, JsValue> {
        let spec = self
            .ladder
            .spec(index)
            .ok_or_else(|| js_error(format!("no capacity tier at index {index}")))?;
        Ok(warm_factory(spec))
    }

    pub fn report_json(&self) -> String {
        format_json(&self.ladder.report())
    }
}
