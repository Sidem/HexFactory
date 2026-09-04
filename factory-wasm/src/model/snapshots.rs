#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct Snapshot {
    boundaries: Vec<Boundary>,
    ground: Vec<GroundCell>,
    /// Cells whose standing water has left the generated equilibrium. Sparse, like `ground`: the
    /// tile still carries the generated depth, published once, and the host adds this departure
    /// exactly as native does.
    water: Vec<hydrology::WaterCell>,
    spoil: u64,
    scenario: String,
    scenario_name: String,
    world_version: u16,
    seed: u32,
    tick: u64,
    checksum: u32,
    /// How many ticks an item takes to cross one belt hex. Published for the same reason the
    /// player's radius and the action cooldown total are: the host draws an item partway along a
    /// conveyor, and the fraction it draws has to be measured against the number the simulation
    /// actually uses rather than one the renderer keeps its own copy of.
    belt_transit_ticks: u32,
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
    terrain: Vec<TileSnapshot>,
    habitats: Vec<HabitatSnapshot>,
    resources: Vec<ResourceSnapshot>,
    buildings: Vec<EntitySnapshot>,
    #[serde(default)]
    ground_items: Vec<GroundItem>,
    events: Vec<String>,
}

/// The same native answer used by the atomic purchase command. Derived, never saved or hashed.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct ResearchAvailability {
    technology_id: TechnologyId,
    complete: bool,
    missing_prerequisites: Vec<TechnologyId>,
    insight_shortfall: u64,
}

/// The player as the host sees it: the saved state plus the carried stacks resolved against the
/// native stack rule. The host draws `carry_stacks` one slot at a time and pads to `carry_slots`,
/// so the grid is presentation over a native answer rather than the same arithmetic written twice.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct PlayerSnapshot {
    #[serde(flatten)]
    state: PlayerState,
    carry_stacks: Vec<Ingredient>,
    /// Collision and drawing radius in world units. Published so the host draws the body that
    /// native actually walks, rather than a hardcoded fraction of the hex size.
    radius: i32,
    /// What a fresh action cooldown is worth. The host draws the wait as a proportion of this, so
    /// it never has to infer the maximum by watching a number count down.
    action_cooldown_total: u32,
    /// What the hand can gather, in hexes. Published so its held-action ring is native truth.
    extract_radius: u32,
    /// Whether this run is creative. It rides with the player because it is a fact about what the
    /// player may spend and carry, and because the host needs it in the same breath as `carry_slots`
    /// to decide whether to draw prices, refunds, and the creative panel's controls at all.
    creative: bool,
    /// The hexes still ahead on the current walk, nearest first and ending on `walk_goal`; empty
    /// when no walk is running. Published rather than re-derived host-side for the reason
    /// `carry_stacks` and `radius` are: the host draws the route the simulation is going to take,
    /// not a second opinion about it computed from the same goal by different arithmetic.
    walk_path: Vec<Coordinate>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct Ingredient64 {
    item_id: ItemId,
    quantity: u64,
}

/// The contract as the host sees it: which stage is current, what that stage is asking for, and
/// how much of each line the hub has already been given. `stage` is also how far the hub has grown,
/// so the drawing and the sentence come from the same number.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct ContractSnapshot {
    key: String,
    name: String,
    /// How many stages are finished, which is the index of the current one while any remain.
    stage: u16,
    stages: u16,
    stage_key: String,
    stage_name: String,
    stage_brief: String,
    /// Every line of the current stage's bill, with what the hub holds against it. Empty once the
    /// whole contract is complete.
    requirements: Vec<ContractRequirement>,
    complete: bool,
}

/// One posted request as the hub is holding it: which row occupies this slot.
///
/// How much has arrived against that row is *not* here. Progress belongs to the project, in
/// `Core::request_delivered`, so passing on a project and calling it back later does not throw away
/// the goods already handed over. Under finite demand that forfeit would be permanent, and a board
/// that quietly destroys deliveries is not a board a player can experiment with.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct RequestState {
    request_id: RequestId,
}

/// Where a project stands for the player who is looking at the catalogue.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ProjectState {
    /// The player cannot yet make what it asks for.
    Locked,
    /// Makeable, and not currently occupying a board slot.
    Available,
    /// On the board now.
    Posted,
    /// Finished. It has paid, and it will never be posted again.
    Complete,
}

/// One line of the project catalogue as the host sees it. Everything needed to draw the row travels
/// with it — the price above all, because a price the player has to discover by delivering is the
/// defect this whole system exists to remove.
///
/// The catalogue is published whole, not just the three posted slots, for the same reason: with a
/// finite budget the player has to be able to see what is left to earn and what it will pay before
/// choosing what to build. A board that only ever shows three rotating rows would hide the shape of
/// the remaining economy behind a draw order.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct RequestSnapshot {
    key: String,
    name: String,
    brief: String,
    item_id: ItemId,
    delivered: u32,
    required: u32,
    insight: u32,
    state: ProjectState,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct ContractRequirement {
    item_id: ItemId,
    /// Contributed toward this stage, already clamped to what the line asks for. The host draws a
    /// proportion from two published numbers rather than inferring a maximum.
    delivered: u32,
    required: u32,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct ChunkSnapshot {
    chunk_q: i32,
    chunk_r: i32,
    entity_count: usize,
    /// World-space origin and side length of the generated square this chunk owns. A chunk is the
    /// unit of world generation, so these bounds are exactly the surveyed area: everything outside
    /// the reported chunks is world the simulation has not generated yet. The square is the
    /// bounding box of the chunk's hexes on the single axial lattice.
    x: i32,
    y: i32,
    span: i32,
}

/// One surveyed cell of generated ground, as the host draws it.
///
/// Every cell of every surveyed chunk appears, including plain lowland. The band used to be the
/// whole payload and a lowland tile carried no information, so it was skipped and the host defaulted
/// the gaps; a per-cell height has no default, so the omission cannot survive it. What keeps that
/// affordable is the group being a patch rather than a resend — a newly surveyed chunk travels once
/// and is never repeated — and delta coding, which prices a neighbouring cell at the hop to it.
///
/// `height`, `water_depth` and the substrate are generated facts and nothing else: the earthwork the
/// player paid for arrives separately in the ground group, and the water they moved arrives
/// separately in the water group. The host adds each overlay exactly as native does. That is what
/// lets this list be published once and never revisited.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct TileSnapshot {
    q: i32,
    r: i32,
    x: i32,
    y: i32,
    radius: u32,
    terrain: Terrain,
    /// Generated bed elevation in the active source's native height unit: signed, absolute, and
    /// with sea level at zero once the physical source is the one answering.
    height: i32,
    /// What the bed is made of, independent of the water standing on it.
    substrate: Substrate,
    /// Standing water above the bed in the same unit as `height`. Zero is dry ground.
    water_depth: i32,
    /// Integer drainage class at this cell. Zero is still water or none at all.
    discharge: u8,
}

/// Exact native fertile-riverbank truth for one surveyed cell. A zero capacity is used only as an
/// incremental tombstone; complete snapshots contain the sparse positive set.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct HabitatSnapshot {
    q: i32,
    r: i32,
    x: i32,
    y: i32,
    radius: u32,
    capacity: u16,
    discharge: u8,
}

/// One field cell. `q`/`r` is its identity: the tile key it is stored under, and what the host
/// addresses it by in a patch. It deliberately carries no separate id — a `u64` packed from the
/// two coordinates used to travel beside them, and JSON numbers are IEEE-754 doubles, so every
/// such id past 2^53 arrived at the host rounded. Whole columns of the field collapsed onto one
/// value, and patching by it rewrote unrelated cells with a copy of the harvested one.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct ResourceSnapshot {
    q: i32,
    r: i32,
    x: i32,
    y: i32,
    radius: u32,
    item_id: ItemId,
    quantity: u32,
    initial_quantity: u32,
}

/// The water cell a pump has resolved, and the native rate that limits it.
///
/// `discharge` zero names finite standing water: `available` is the depth left and pumping moves
/// the departure. A non-zero discharge names a replenishing river and is the number of withdrawals
/// that cell can supply per tick, arbitrated by stable entity id.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct WaterSourceSnapshot {
    q: i32,
    r: i32,
    available: u32,
    discharge: u8,
    rate: u32,
}

/// Why a machine is doing what it is doing, as the inspector says it.
///
/// This is a closed set, and naming it as one is what lets the binary wire carry a byte where JSON
/// carried up to nineteen characters per entity per delta. The serialized spelling is the contract:
/// these strings are what the host renders, so a variant may not be renamed without changing what
/// the player reads. Wire codes are the declaration order and are pinned by
/// `fixtures/snapshot-delta-wire.json`.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
enum EntityStatus {
    #[serde(rename = "output blocked")]
    OutputBlocked,
    #[serde(rename = "deposit depleted")]
    DepositDepleted,
    #[serde(rename = "extracting")]
    Extracting,
    #[serde(rename = "no water in reach")]
    NoWaterInReach,
    #[serde(rename = "pumping")]
    Pumping,
    #[serde(rename = "composing")]
    Composing,
    #[serde(rename = "out of fuel")]
    OutOfFuel,
    #[serde(rename = "waiting for inputs")]
    WaitingForInputs,
    #[serde(rename = "buffered")]
    Buffered,
    #[serde(rename = "carrying")]
    Carrying,
    #[serde(rename = "receiving")]
    Receiving,
    #[serde(rename = "landing hub")]
    LandingHub,
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "no power")]
    NoPower,
    #[serde(rename = "generating")]
    Generating,
    #[serde(rename = "brownout")]
    Brownout,
    #[serde(rename = "no boiler")]
    NoBoiler,
    /// Switched off by hand. It outranks every other reason a machine is not working, because it
    /// is the only one the player chose: "out of fuel" on a burner they deliberately stopped would
    /// send them looking for a problem that is not there.
    #[serde(rename = "switched off")]
    SwitchedOff,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct EntitySnapshot {
    id: u32,
    q: i32,
    r: i32,
    definition_id: DefinitionId,
    kind: BuildingKind,
    orientation: u8,
    recipe_id: Option<RecipeId>,
    scenario_owned: bool,
    cargo: Option<Cargo>,
    /// What this belt is still carrying across its own hex, oldest first, each with the tick it
    /// stepped on. `cargo` is the item that has finished crossing and is waiting to be handed on.
    ///
    /// The host draws each of these at `(tick - entered) / belt_transit_ticks` of the way over the
    /// belt. It is published as the entry tick rather than as a fraction on purpose: a fraction
    /// changes every tick, so every belt in the factory would be a changed entity in every delta,
    /// and a line standing still would cost as much to send as one that just started.
    ///
    /// Omitted when empty, which is every machine, container and idle belt in the game.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    lane: Vec<LaneItem>,
    inventory: Vec<Ingredient>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    input_inventory: Vec<Ingredient>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fuel_inventory: Vec<Ingredient>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    output_inventory: Vec<Ingredient>,
    /// One effective port per product this building can make. Defaults are published too, so the
    /// inspector never has to reconstruct where a multi-cell building's facing exits its hull.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    output_routes: Vec<OutputRouteSnapshot>,
    /// Present only on a pump with standing water in reach. It names the cell the deterministic
    /// resolver chose and the limiting rate the tick enforces.
    #[serde(skip_serializing_if = "Option::is_none")]
    water_source: Option<WaterSourceSnapshot>,
    progress: u32,
    progress_total: u32,
    /// Energy the machine is holding, and what one craft of its recipe costs. Both are published
    /// so the inspector can say "out of fuel" for the reason the machine actually stopped rather
    /// than re-deriving the fuel rule in the host.
    ///
    /// Omitted when zero, which is what they are for every belt, container, and fuel-free machine.
    /// Sent unconditionally they cost two numbers per entity per delta — 86 KB at the largest
    /// measured tier, against a boundary priced at about 10 µs/KB — to say "this is not a furnace"
    /// about entities that never will be.
    #[serde(skip_serializing_if = "is_zero")]
    fuel_charge: u32,
    #[serde(skip_serializing_if = "is_zero")]
    fuel_required: u32,
    /// Network supply and demand, both sent so the host draws a proportion it was given.
    #[serde(skip_serializing_if = "is_zero")]
    power_satisfied: u32,
    #[serde(skip_serializing_if = "is_zero")]
    power_demand: u32,
    /// Electricity this machine is holding, against the buffer it fills to. Published for the same
    /// reason `fuel_charge` is: "brownout" is a word, and a bank draining is the picture.
    #[serde(skip_serializing_if = "is_zero")]
    power_charge: u32,
    #[serde(skip_serializing_if = "is_zero")]
    power_capacity: u32,
    status: EntityStatus,
    next_id: Option<u32>,
    /// The compiled outputs *after* the first, which only a splitter ever has.
    ///
    /// `next_id` stays the primary edge so every reader that predates junctions — the connecting
    /// deck, the inspector's downstream line, the hover trace — is unchanged on every building that
    /// will never have a second output. Omitted when empty, which is every belt, riser, underpass,
    /// merger, and machine in the game: sent unconditionally it would cost a length on every entity
    /// of every delta to say "this is not a splitter".
    #[serde(skip_serializing_if = "Vec::is_empty")]
    branch_ids: Vec<u32>,
    footprint: Vec<Coordinate>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct OutputRouteSnapshot {
    item_id: ItemId,
    q: i32,
    r: i32,
    direction: u8,
    target_id: Option<u32>,
}

/// A per-entity buildings patch. `changed` carries inserted and modified entities and `removed`
/// carries the ids the host must drop, both in ascending stable-id order. Group-level dirty
/// tracking cannot help a running factory, because one moving item resends every building.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct BuildingsDelta {
    /// Set only on a full delta, where `changed` is the complete list rather than a patch.
    #[serde(skip_serializing_if = "is_false")]
    replace: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    changed: Vec<EntitySnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    removed: Vec<u32>,
}

/// A per-cell terrain patch, addressed by tile key.
///
/// Generation is the only thing that adds a tile and nothing ever changes or removes one, so an
/// incremental patch is exactly the chunks surveyed since the host last heard — the phase brief's
/// "publish newly surveyed height chunks once". `replace` is set only by a full snapshot, where the
/// host holds nothing to patch.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct TerrainDelta {
    #[serde(skip_serializing_if = "is_false")]
    replace: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    changed: Vec<TileSnapshot>,
}

/// A per-deposit resources patch, addressed by tile key. Resource tiles are inserted by
/// world generation and updated by extraction and gathering; the tile map has no removal path, so
/// the patch needs no removal list. Generation is the only thing that adds a deposit, and it sets
/// `replace`, so an incremental patch never disturbs the order the host already holds.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct ResourcesDelta {
    /// Set when `changed` is the complete list rather than a patch.
    #[serde(skip_serializing_if = "is_false")]
    replace: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    changed: Vec<ResourceSnapshot>,
}

/// A sparse per-cell habitat patch. Capacity zero removes a cell that ceased to qualify.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct HabitatsDelta {
    #[serde(skip_serializing_if = "is_false")]
    replace: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    changed: Vec<HabitatSnapshot>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

/// Which parts of the next snapshot may differ from the one the host already holds, marked where
/// state is mutated rather than discovered by diffing a freshly materialized snapshot.
///
/// This is derived presentation state: it is never saved, hashed, or checksummed, and it can never
/// change a simulation result. Every mark is a hint to rebuild one entry, and the emitted delta is
/// still filtered against the baseline the host was actually sent — so marking something that did
/// not change costs one wasted rebuild, never a wrong frame. Missing a mark would be a defect, so
/// `dirty_tracked_deltas_match_a_full_snapshot_diff` pins the whole set against a full diff.
/// Marks are appended, not inserted into an ordered set: the tick loop makes thousands of them per
/// frame, and an ordered insert costs a tree descent each time where a push costs nothing. Order
/// and uniqueness are what the delta needs, and it gets both from one sort at emit time — see
/// `drain_marks`.
#[derive(Clone, Debug, Default)]
struct SnapshotDirty {
    boundaries: bool,
    /// Set when a surface or grade changed. Sparse and small, so the group is resent whole.
    ground: bool,
    /// Set when a water departure changed. Sparse and small, so the group is resent whole.
    water: bool,
    /// Stable entity ids whose snapshot may differ, including newly placed ones.
    entities: Vec<u32>,
    /// Stable entity ids the host must drop.
    removed: BTreeSet<u32>,
    /// Tile keys of deposits whose quantity may differ.
    resources: Vec<(i32, i32)>,
    /// Set when generation may have added deposits, so the resources group is resent whole and the
    /// host's ordering stays exactly the native one.
    resources_replace: bool,
    /// Chunk keys generation has surveyed since the host last heard. Terrain only ever grows, and it
    /// grows a whole chunk at a time, so the chunk key is the whole mark: the tiles it names have
    /// never been published and every other tile in the world is already correct at the host.
    terrain: Vec<(i32, i32)>,
    /// Cells whose current ground, water, surface, or occupancy may have changed habitat capacity.
    habitats: Vec<(i32, i32)>,
    /// Set when the generated chunk set or any chunk's entity count may differ.
    chunks: bool,
    /// Set when dropped ground items change.
    ground_items: bool,
}

/// Take a mark list as the ascending, duplicate-free order the delta must travel in.
fn drain_marks<T: Ord>(marks: &mut Vec<T>) -> Vec<T> {
    let mut marks = std::mem::take(marks);
    marks.sort_unstable();
    marks.dedup();
    marks
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct SnapshotDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    boundaries: Option<Vec<Boundary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ground: Option<Vec<GroundCell>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    spoil: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    water: Option<Vec<hydrology::WaterCell>>,
    base_revision: u64,
    revision: u64,
    tick: u64,
    checksum: u32,
    /// See [`Snapshot::belt_transit_ticks`]. Sent in the header of every delta rather than behind a
    /// group bit: it is a constant, it costs one byte, and a host that joined mid-run needs it to
    /// draw the very first belt it is told about.
    belt_transit_ticks: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    scenario: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scenario_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    world_version: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivered: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivered_by_item: Option<Vec<Ingredient64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    insight: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    victory: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contract: Option<ContractSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    requests: Option<Vec<RequestSnapshot>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    player: Option<PlayerSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    researched: Option<Vec<TechnologyId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    research_availability: Option<Vec<ResearchAvailability>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skills: Option<SkillsSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chunks: Option<Vec<ChunkSnapshot>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    terrain: Option<TerrainDelta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    habitats: Option<HabitatsDelta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resources: Option<ResourcesDelta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    buildings: Option<BuildingsDelta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ground_items: Option<Vec<GroundItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    events: Option<Vec<String>>,
}

impl SnapshotDelta {
    fn full(base_revision: u64, revision: u64, current: &Snapshot) -> Self {
        Self {
            base_revision,
            revision,
            tick: current.tick,
            checksum: current.checksum,
            belt_transit_ticks: current.belt_transit_ticks,
            scenario: Some(current.scenario.clone()),
            scenario_name: Some(current.scenario_name.clone()),
            world_version: Some(current.world_version),
            seed: Some(current.seed),
            delivered: Some(current.delivered),
            delivered_by_item: Some(current.delivered_by_item.clone()),
            insight: Some(current.insight),
            victory: Some(current.victory),
            contract: Some(current.contract.clone()),
            requests: Some(current.requests.clone()),
            player: Some(current.player.clone()),
            researched: Some(current.researched.clone()),
            research_availability: Some(current.research_availability.clone()),
            skills: Some(current.skills.clone()),
            chunks: Some(current.chunks.clone()),
            terrain: Some(TerrainDelta {
                replace: true,
                changed: current.terrain.clone(),
            }),
            habitats: Some(HabitatsDelta {
                replace: true,
                changed: current.habitats.clone(),
            }),
            resources: Some(ResourcesDelta {
                replace: true,
                changed: current.resources.clone(),
            }),
            buildings: Some(BuildingsDelta {
                replace: true,
                changed: current.buildings.clone(),
                removed: Vec::new(),
            }),
            ground_items: Some(current.ground_items.clone()),
            boundaries: Some(current.boundaries.clone()),
            ground: Some(current.ground.clone()),
            spoil: Some(current.spoil),
            water: Some(current.water.clone()),
            events: Some(current.events.clone()),
        }
    }

    /// The reference diff between two complete snapshots. The shipped path no longer materializes a
    /// complete snapshot per frame, so this is retained as the oracle the dirty-tracked builder is
    /// pinned against — see `dirty_tracked_deltas_match_a_full_snapshot_diff`.
    #[cfg(test)]
    fn between(base_revision: u64, revision: u64, previous: &Snapshot, current: &Snapshot) -> Self {
        Self {
            base_revision,
            revision,
            tick: current.tick,
            checksum: current.checksum,
            belt_transit_ticks: current.belt_transit_ticks,
            scenario: changed(&previous.scenario, &current.scenario),
            scenario_name: changed(&previous.scenario_name, &current.scenario_name),
            world_version: changed_copy(previous.world_version, current.world_version),
            seed: changed_copy(previous.seed, current.seed),
            delivered: changed_copy(previous.delivered, current.delivered),
            delivered_by_item: changed(&previous.delivered_by_item, &current.delivered_by_item),
            insight: changed_copy(previous.insight, current.insight),
            victory: changed_copy(previous.victory, current.victory),
            contract: changed(&previous.contract, &current.contract),
            requests: changed(&previous.requests, &current.requests),
            player: changed(&previous.player, &current.player),
            researched: changed(&previous.researched, &current.researched),
            research_availability: changed(
                &previous.research_availability,
                &current.research_availability,
            ),
            skills: changed(&previous.skills, &current.skills),
            chunks: changed(&previous.chunks, &current.chunks),
            terrain: terrain_delta(&previous.terrain, &current.terrain),
            habitats: habitat_delta(&previous.habitats, &current.habitats),
            resources: resources_delta(&previous.resources, &current.resources),
            buildings: buildings_delta(&previous.buildings, &current.buildings),
            ground_items: changed(&previous.ground_items, &current.ground_items),
            boundaries: changed(&previous.boundaries, &current.boundaries),
            ground: changed(&previous.ground, &current.ground),
            spoil: changed_copy(previous.spoil, current.spoil),
            water: changed(&previous.water, &current.water),
            events: changed(&previous.events, &current.events),
        }
    }
}

/// The reference terrain diff, retained alongside `SnapshotDelta::between` as the oracle for the
/// dirty-tracked builder. A tile is never altered or removed once generation publishes it, so this
/// is exactly the cells the previous snapshot did not have — the chunks surveyed in between, in the
/// order the surveyed-chunk set already holds them.
#[cfg(test)]
fn terrain_delta(previous: &[TileSnapshot], current: &[TileSnapshot]) -> Option<TerrainDelta> {
    let before: BTreeSet<(i32, i32)> = previous.iter().map(|tile| (tile.q, tile.r)).collect();
    let changed: Vec<TileSnapshot> = current
        .iter()
        .filter(|tile| !before.contains(&(tile.q, tile.r)))
        .copied()
        .collect();
    (!changed.is_empty()).then_some(TerrainDelta {
        replace: false,
        changed,
    })
}

#[cfg(test)]
fn habitat_delta(
    previous: &[HabitatSnapshot],
    current: &[HabitatSnapshot],
) -> Option<HabitatsDelta> {
    let key = |cell: &HabitatSnapshot| (cell.q, cell.r);
    let old: BTreeMap<_, _> = previous.iter().map(|cell| (key(cell), cell)).collect();
    let new: BTreeMap<_, _> = current.iter().map(|cell| (key(cell), cell)).collect();
    let mut changed = Vec::new();
    for (&cell, value) in &new {
        if old.get(&cell).copied() != Some(*value) {
            changed.push(**value);
        }
    }
    for (&(q, r), old) in &old {
        if !new.contains_key(&(q, r)) {
            changed.push(HabitatSnapshot {
                q,
                r,
                x: old.x,
                y: old.y,
                radius: old.radius,
                capacity: 0,
                discharge: 0,
            });
        }
    }
    changed.sort_unstable_by_key(|cell| (cell.q, cell.r));
    (!changed.is_empty()).then_some(HabitatsDelta {
        replace: false,
        changed,
    })
}

/// The reference resources diff, retained alongside `SnapshotDelta::between` as the oracle for the
/// dirty-tracked builder. A changed deposit set means generation ran, which resends the group whole
/// so the host's ordering stays exactly the native one; otherwise only altered deposits travel.
#[cfg(test)]
fn resources_delta(
    previous: &[ResourceSnapshot],
    current: &[ResourceSnapshot],
) -> Option<ResourcesDelta> {
    let key = |resource: &ResourceSnapshot| (resource.q, resource.r);
    let before: BTreeSet<(i32, i32)> = previous.iter().map(key).collect();
    let after: BTreeSet<(i32, i32)> = current.iter().map(key).collect();
    if before != after {
        return Some(ResourcesDelta {
            replace: true,
            changed: current.to_vec(),
        });
    }
    let existing: BTreeMap<(i32, i32), &ResourceSnapshot> = previous
        .iter()
        .map(|resource| (key(resource), resource))
        .collect();
    let changed: Vec<ResourceSnapshot> = current
        .iter()
        .filter(|resource| existing.get(&key(resource)) != Some(resource))
        .copied()
        .collect();
    (!changed.is_empty()).then_some(ResourcesDelta {
        replace: false,
        changed,
    })
}

/// Both snapshots list buildings in ascending stable entity id order, so one linear pass finds
/// every insert, update, and removal without comparing the arrays as a whole.
#[cfg(test)]
fn buildings_delta(
    previous: &[EntitySnapshot],
    current: &[EntitySnapshot],
) -> Option<BuildingsDelta> {
    let mut changed: Vec<EntitySnapshot> = Vec::new();
    let mut removed: Vec<u32> = Vec::new();
    let mut before = previous.iter().peekable();
    let mut after = current.iter().peekable();
    loop {
        match (before.peek(), after.peek()) {
            (Some(old), Some(new)) => match old.id.cmp(&new.id) {
                Ordering::Less => {
                    removed.push(old.id);
                    before.next();
                }
                Ordering::Greater => {
                    changed.push((*new).clone());
                    after.next();
                }
                Ordering::Equal => {
                    if old != new {
                        changed.push((*new).clone());
                    }
                    before.next();
                    after.next();
                }
            },
            (Some(old), None) => {
                removed.push(old.id);
                before.next();
            }
            (None, Some(new)) => {
                changed.push((*new).clone());
                after.next();
            }
            (None, None) => break,
        }
    }
    (!changed.is_empty() || !removed.is_empty()).then_some(BuildingsDelta {
        replace: false,
        changed,
        removed,
    })
}

#[cfg(test)]
fn changed<T: Clone + PartialEq>(previous: &T, current: &T) -> Option<T> {
    (previous != current).then(|| current.clone())
}

#[cfg(test)]
fn changed_copy<T: Copy + PartialEq>(previous: T, current: T) -> Option<T> {
    (previous != current).then_some(current)
}
