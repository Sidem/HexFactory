#[derive(Serialize)]
struct PlacementPreview {
    legal: bool,
    reason: String,
}

/// One cell of a drag preview. The host draws these and nothing else: it never derives the path,
/// the heading, or the legality itself, so what is shown during a drag is what `place_line` and
/// `erase_line` will do with the same endpoints.
#[derive(Serialize)]
struct LinePreviewCell {
    q: i32,
    r: i32,
    orientation: u8,
    legal: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

/// A building's native stock compartment. `Auto` exists only at the command boundary for quick
/// transfers; explicit slot clicks always name the field they are interacting with.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum StockKind {
    #[default]
    Auto,
    Inventory,
    Input,
    Fuel,
    Output,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum InputCommand {
    BoundaryEdit {
        #[serde(flatten)]
        edit: BoundaryEdit,
    },
    UndoBoundary,
    GroundEdit {
        #[serde(flatten)]
        edit: GroundEdit,
    },
    UndoGround,
    /// Creative hydrology probe: move a bounded depth at one surveyed cell, then let native settle
    /// it. Earthworks and pumps use the same internal edit path without becoming host-side loops.
    WaterEdit {
        q: i32,
        r: i32,
        action: hydrology::WaterAction,
        quanta: u16,
    },
    MoveIntent {
        x: i16,
        y: i16,
    },
    /// Point the player at a world position — the point under the host's cursor. The host sends a
    /// target and never a heading: facing is a checksum input, so normalizing a continuous pointer
    /// angle in host floating point would be TypeScript deciding a value the simulation hashes.
    Aim {
        x: i32,
        y: i32,
    },
    Gather,
    /// Harvest one named hex inside the player's own reach. The target is explicit — the player
    /// right-clicked it — which is why this is not the facing-weighted targeting the gather
    /// invariant refuses.
    GatherAt {
        q: i32,
        r: i32,
    },
    Deposit {
        #[serde(default)]
        item_id: Option<ItemId>,
    },
    Place {
        q: i32,
        r: i32,
        definition_id: DefinitionId,
        orientation: u8,
        #[serde(default)]
        recipe_id: Option<RecipeId>,
    },
    /// One drag, resolved natively. The host sends only the two endpoints it dragged between; the
    /// path, the per-cell orientation, the legality, and the cost are all resolved here.
    PlaceLine {
        q: i32,
        r: i32,
        to_q: i32,
        to_r: i32,
        definition_id: DefinitionId,
        orientation: u8,
        #[serde(default)]
        recipe_id: Option<RecipeId>,
    },
    Erase {
        q: i32,
        r: i32,
    },
    EraseLine {
        q: i32,
        r: i32,
        to_q: i32,
        to_r: i32,
    },
    Rotate {
        q: i32,
        r: i32,
        #[serde(default)]
        reverse: bool,
    },
    /// Route one product through one exterior side of one footprint tile. The building target and
    /// the port cell are both explicit so a multi-cell refinery is never reduced to its anchor.
    SetOutputRoute {
        q: i32,
        r: i32,
        item_id: ItemId,
        output_q: i32,
        output_r: i32,
        direction: u8,
    },
    /// Grow a building into the next tier of itself, keeping its contents, its heading, and its
    /// connections. Bounded and range-checked like every other edit.
    Upgrade {
        q: i32,
        r: i32,
    },
    /// Take stock out of a container by hand. Bounded and range-checked like every other edit.
    Withdraw {
        q: i32,
        r: i32,
        item_id: ItemId,
        quantity: u32,
        #[serde(default)]
        stock: StockKind,
    },
    /// Put stock into a container by hand — the mirror of `Withdraw`, on the same contract.
    Store {
        q: i32,
        r: i32,
        item_id: ItemId,
        quantity: u32,
        #[serde(default)]
        stock: StockKind,
    },
    /// Lift a bounded amount out of the pack and hold it on the cursor.
    PickupPlayerStack {
        item_id: ItemId,
        quantity: u32,
    },
    /// Lift a bounded amount out of one named building compartment.
    PickupBuildingStack {
        q: i32,
        r: i32,
        stock: StockKind,
        item_id: ItemId,
        quantity: u32,
    },
    /// Return some or all of the cursor stack to the pack.
    PlacePlayerStack {
        quantity: u32,
    },
    /// Put some or all of the cursor stack into one named building compartment.
    PlaceBuildingStack {
        q: i32,
        r: i32,
        stock: StockKind,
        quantity: u32,
    },
    /// Drop some or all of the cursor stack onto the ground in the world.
    DropPlayerStack {
        q: i32,
        r: i32,
        quantity: u32,
    },
    /// Give a machine a different job. With fourteen recipes across five machine categories,
    /// erasing and rebuilding to change one assignment is friction the material base would add to
    /// every layout decision.
    SetRecipe {
        q: i32,
        r: i32,
        recipe_id: RecipeId,
    },
    /// Abandon a part-finished craft, returning its reserved ingredients to the machine's own
    /// ingredient compartment. The way out of a stopped manual workshop that is not demolition.
    CancelCraft {
        q: i32,
        r: i32,
    },
    Undo,
    PurchaseSkill {
        skill_id: u16,
    },
    Research {
        technology_id: TechnologyId,
    },
    /// Switch a machine off, or back on. Bounded and range-checked like every other edit.
    ///
    /// The state is carried, not toggled: the host sends what it wants the machine to *be*, so a
    /// press that arrives twice — a doubled tap, a replayed frame — lands on the same answer. A
    /// toggle would not, and this queue is allowed to coalesce.
    SetEnabled {
        q: i32,
        r: i32,
        enabled: bool,
    },
    /// Pass on one posted request, so the hub asks for something else in that slot.
    SkipRequest {
        slot: usize,
    },
    /// Ask the hub for one named project, taking a board slot for it. The catalogue is finite, so
    /// which project is posted has to be the player's choice rather than the draw order's.
    PostRequest {
        request_id: RequestId,
    },
    /// Legacy wire spelling retained for recorded command compatibility. A running world refuses
    /// any mode change; creative is selected only when that world is created.
    SetCreative {
        enabled: bool,
    },
    /// Put an item straight into the pack, out of nowhere. Creative only, and bounded by the pack
    /// like every other way stock arrives: what will not fit is not granted.
    Grant {
        item_id: ItemId,
        quantity: u32,
    },
    /// Take an item straight back out of the pack and destroy it. Creative only. `item_id: None`
    /// empties the pack entirely, mirroring `Deposit`, so clearing it is one command rather than one
    /// per stack against a batch that holds eight.
    Discard {
        #[serde(default)]
        item_id: Option<ItemId>,
        #[serde(default)]
        quantity: u32,
    },
    /// Widen or narrow the pack. Creative only, bounded by the scenario's own number below and
    /// `MAX_CARRY_SLOTS` above, and refused outright while it would strand stock already carried.
    SetCarrySlots {
        slots: u32,
    },
    /// Walk to a hex the player pointed at, finding the way there natively.
    ///
    /// The host sends a destination and never a route, for the same reason `Aim` sends a target and
    /// never a heading and a drag sends two endpoints and never a line: the path is a checksum input
    /// and a collision question, so resolving it in TypeScript would be the host deciding a value
    /// the simulation hashes and then walks the player through.
    WalkTo {
        q: i32,
        r: i32,
    },
}
