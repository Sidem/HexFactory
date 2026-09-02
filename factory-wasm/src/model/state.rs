/// Ticks an item spends crossing one belt hex, from [`scale::belt_transit_ticks`].
const BELT_TRANSIT_TICKS: u64 = scale::belt_transit_ticks() as u64;
/// Items one belt hex holds while they cross it, from [`scale::belt_lane_slots`].
const BELT_LANE_SLOTS: usize = scale::belt_lane_slots() as usize;
/// The gap a belt insists on between two items entering it, from [`scale::belt_slot_ticks`].
///
/// This is the number that sets belt throughput — one item every five ticks, 120 a minute, exactly
/// one extractor — and it is derived from the belt's speed and the spacing of the items on it
/// rather than chosen. See `scale::belt_cadence_follows_from_speed_and_spacing`.
const BELT_SLOT_TICKS: u64 = scale::belt_slot_ticks() as u64;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct GroundItem {
    pub(crate) id: u32,
    pub(crate) q: i32,
    pub(crate) r: i32,
    pub(crate) item_id: ItemId,
    pub(crate) quantity: u32,
    pub(crate) despawn_tick: u64,
}

/// Ticks a dropped item stays on the ground before disappearing (1 minute = 600 ticks at 10 TPS).
pub(crate) const GROUND_ITEM_LIFETIME_TICKS: u64 = 600;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Terrain {
    DeepWater,
    ShallowWater,
    Shore,
    Lowland,
    /// The band between lowland and highland. v0.11 read one raised band; the material base needs
    /// two, because copper belongs to rolling ground and iron and coal to the tops, and a player
    /// who cannot see the difference cannot choose a site from the terrain.
    Hills,
    Highland,
    Cliff,
}

impl Terrain {
    fn blocks_movement(self) -> bool {
        // Shallows are a ford, not a wall: the player can wade them at 5 m/s. Construction still
        // refuses them, which is why `blocks_construction` is a separate predicate and not this
        // one reused. Deep water and cliff stay impassable.
        matches!(self, Terrain::DeepWater | Terrain::Cliff)
    }

    fn blocks_construction(self) -> bool {
        matches!(
            self,
            Terrain::DeepWater | Terrain::ShallowWater | Terrain::Cliff
        )
    }

    fn is_water(self) -> bool {
        matches!(self, Terrain::DeepWater | Terrain::ShallowWater)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct ResourceState {
    item_id: ItemId,
    quantity: u32,
    initial_quantity: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct TileState {
    q: i32,
    r: i32,
    x: i32,
    y: i32,
    radius: u32,
    terrain: Terrain,
    resource: Option<ResourceState>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct PlayerState {
    x: i32,
    y: i32,
    facing_x: i16,
    facing_y: i16,
    move_x: i16,
    move_y: i16,
    inventory: BTreeMap<ItemId, u32>,
    /// The stack currently carried by the pointer. It is outside the pack's slot count but remains
    /// native-owned inventory: picking it up removes it from its source, placing it commits it to a
    /// destination, and a save in between loses neither quantity nor identity.
    #[serde(default)]
    hand: Option<Cargo>,
    action_cooldown: u32,
    build_range: u32,
    /// Slots the player can carry, from the scenario. Like `build_range` it is a fixed scenario
    /// property rather than a simulation result, so it is validated against the scenario on load
    /// instead of being hashed into the checksum.
    carry_slots: u32,
    /// The hex an autonomous walk is headed for, if one is running.
    ///
    /// This is the whole of the walk's *state*. The route to it is not: a path is a derived answer
    /// about a world that can change under it, and `Core::walk_path` rebuilds it from this goal
    /// whenever the world does, under the same rule as every other derived index. Saving the goal
    /// and rebuilding the route is also the only version of this that survives a reload honestly —
    /// a saved route would come back describing a corridor that the loaded factory may no longer
    /// have, and the player would watch themselves walk into a wall they built before saving.
    ///
    /// Saved and checksummed beside `move_x`/`move_y`, for the reason those are: it is an input the
    /// simulation is still executing, and two runs that differ only in where the player is headed
    /// will not stay identical for long.
    #[serde(default)]
    walk_goal: Option<Coordinate>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct Entity {
    id: u32,
    placed: PlacedBuilding,
    kind: BuildingKind,
    cargo: Option<Cargo>,
    inventory: BTreeMap<ItemId, u32>,
    /// Native storage compartments. `inventory` remains the general store used by containers and
    /// by version-15 machine saves; new machine deliveries go only to these named buffers.
    #[serde(default)]
    input_inventory: BTreeMap<ItemId, u32>,
    #[serde(default)]
    fuel_inventory: BTreeMap<ItemId, u32>,
    #[serde(default)]
    output_inventory: BTreeMap<ItemId, u32>,
    reserved_inputs: BTreeMap<ItemId, u32>,
    progress: u32,
    /// Energy left in the machine from fuel it has already burned. Real state: it is saved,
    /// hashed, and checksummed, because a smelter that is a quarter of the way through a coal is
    /// not the same machine as one that has just been fed.
    #[serde(default)]
    fuel_charge: u32,
    /// Electricity this machine has been given and has not spent yet. Real state for the same
    /// reason `fuel_charge` is: a smelter holding two crafts' worth of power is not the same
    /// machine as one that has just been connected, and the difference survives a save.
    #[serde(default)]
    power_charge: u32,
    /// A generator's progress toward its next whole unit of fuel energy, numerator over
    /// `power_output`. A plant carrying a tenth of the load burns a tenth of the coal, and this is
    /// where the other nine tenths of the unit waits rather than being rounded away.
    #[serde(default)]
    burn_progress: u32,
    /// Switched off by hand. Real state, saved and hashed: a smelter the player deliberately
    /// stopped is not the same machine as one that happens to be out of inputs this tick, and the
    /// difference has to survive a save or every reload would silently restart the factory.
    ///
    /// Suspension is *total and free*. A disabled machine does no work, draws no electricity, asks
    /// for none to bank, and burns no fuel — which is the whole point of the switch: it is how a
    /// player stops a burner eating coal while they rebuild the line it feeds. What it keeps is
    /// everything it was holding: stock, reserved inputs, part-finished progress, banked charge.
    /// Switching back on resumes rather than restarts.
    #[serde(default)]
    disabled: bool,
    /// Which of a splitter's compiled outputs gets the next item it can take.
    ///
    /// Real state, saved and hashed on exactly the terms `fuel_charge` is: a splitter that has just
    /// fed its left branch is not the same machine as one that has just fed its right, and a reload
    /// that forgot which would re-bias every junction in the factory toward the same branch. An
    /// index into the compiled link list, so it is meaningless — and unread — on anything else.
    #[serde(default)]
    route_cursor: u8,
    /// The id of the feeder a merger served last, so the next one it serves is the next id round
    /// the ring rather than the lowest.
    ///
    /// Stored as the feeder's *id* and not as a slot, because a merger's feeders are whatever
    /// happens to point at it: a lane erased and rebuilt changes the set, and a rotation that
    /// counted slots would silently restart. Real state for the same reason `route_cursor` is.
    #[serde(default)]
    merge_cursor: u32,
    /// Items still crossing a belt, oldest first, each stamped with the tick it stepped on.
    ///
    /// A belt hex is 5.37 m of conveyor, and an item takes [`BELT_TRANSIT_TICKS`] to cross it. That
    /// is a latency, not a throughput: a belt that could only hold the one item it hands on would
    /// move twenty-two items a minute and no chain in the game would run. So the hex holds
    /// [`BELT_LANE_SLOTS`] of them at once, spaced [`BELT_SLOT_TICKS`] apart, which is what a real
    /// conveyor does — items sit on it in a line rather than teleporting one at a time.
    ///
    /// `cargo` remains the exit slot: an item that has finished crossing leaves the lane and waits
    /// there to be handed on, so everything that offers, subtracts, splits or merges cargo goes on
    /// reading exactly one item per belt and did not have to learn about lanes.
    ///
    /// Real state, saved and hashed: a belt with four items halfway along it is not a belt with one
    /// at the end, and a reload that forgot would evaporate the contents of every line in the
    /// factory.
    #[serde(default)]
    lane: Vec<LaneItem>,
}
