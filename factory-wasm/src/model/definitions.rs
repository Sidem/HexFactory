fn default_footprint() -> Vec<Coordinate> {
    vec![Coordinate { q: 0, r: 0 }]
}

#[derive(Clone, Deserialize)]
struct DefinitionsInput {
    #[serde(default)]
    boundaries: Vec<BoundaryDefinition>,
    #[serde(default)]
    surfaces: Vec<SurfaceDefinition>,
    version: u16,
    items: Vec<ItemDefinition>,
    recipes: Vec<RecipeDefinition>,
    buildings: Vec<BuildingDefinition>,
    /// What the landing hub is willing to pay insight for, and how much. See
    /// [`Core::refill_requests`] for how a row becomes a posted request.
    requests: Vec<RequestDefinition>,
}

/// One standing order the landing hub can post: a named quantity of one item, for a stated
/// number of insight.
///
/// Insight used to be a property of the item — every delivery paid `insight_value × quantity`,
/// whatever the hub had any use for — which made the eight raw materials differ less than their
/// geography claims and left the player with no way to find out what anything was worth except by
/// handing it over. A request states the price *before* the delivery, and it is the only thing in
/// the game that pays insight at all.
///
/// It also pays **once**. A row used to repost after it was filled, at a decayed price for the raw
/// surveys and at full price for every processed row, which made insight an unbounded income: the
/// answer to "can I afford the deepest branch" was always yes, given enough repetitions of the one
/// delivery the player had already automated. A project is now a finite piece of practical work —
/// a stated bill, a stated price, completed exactly once — so the catalogue is a budget rather than
/// a tap. What bounds research is what the hub still has left to learn.
#[derive(Clone, Deserialize)]
struct RequestDefinition {
    id: RequestId,
    key: String,
    name: String,
    /// One sentence saying why the hub wants it. Shown on the board, so it is content rather than
    /// a comment.
    brief: String,
    item_id: ItemId,
    quantity: u32,
    /// What completing this project pays, once. Priced against the raw gathers underneath the item —
    /// see the `requests` section of `fixtures/balance.json`, which reports exactly that ratio.
    insight: u32,
}

#[derive(Clone, Deserialize)]
struct ItemDefinition {
    id: ItemId,
    key: String,
    name: String,
    color: String,
    icon: String,
    description: String,
    /// How many of this item occupy one carried slot. Carrying capacity is a rule over the
    /// player's ordinary `item_id → quantity` map rather than a stored array of slots, so the save
    /// format, the checksum inputs, and every ordering guarantee are unchanged by it.
    stack_size: u32,
    /// Loose bulk liquid. It may occupy native machine stock and pipe cargo, but it does not enter
    /// a player's pack or a newly built solid belt. Filled barrels are ordinary non-fluid items.
    #[serde(default)]
    fluid: bool,
    /// Energy one unit releases when burned. Fuel is a property of the item, never an entry in a
    /// recipe's `inputs`: naming a fuel in a recipe would force one recipe per fuel and hardcode
    /// the bootstrap path, where this way coal and charcoal are the same recipe at different
    /// values and every fuel added later is too.
    #[serde(default)]
    fuel_value: Option<u32>,
    /// Ticks between one unit of regrowth and the next, for a resource that is flora rather than
    /// ore. A harvested cell climbs back toward the quantity generation gave it and stops there,
    /// which is what makes wood renewable while every ore field is finite.
    #[serde(default)]
    regrowth_ticks: Option<u32>,
    /// Root and cover resistance when this item is a living field resource. Ordinary cargo leaves
    /// it at zero; geomorphology reads it only while a non-empty field stands on the bank.
    #[serde(default)]
    erosion_resistance: u16,
    /// Player-clock steps between hand gathers of this item. Absent means the hand cannot take it
    /// at all: water is pumped, signal crystal is extracted. Fifteen is wood, and no material is
    /// faster — that is the restated invariant `fixtures/balance.json` pins.
    #[serde(default)]
    hand_gather_steps: Option<u32>,
    /// Simulation ticks a tier-one extractor spends on one unit of this material, before its own
    /// `extract_speed` scales it. Extraction rate is a property of what is being dug, for the same
    /// reason `hand_gather_steps` is: coal and sand are not the same work, and a single building
    /// cadence said they were.
    ///
    /// The figures are set against the hand at the default ten ticks per second, where a tier-one
    /// extractor takes twice as long as a hand on the same material. That inverts the rule v0.23
    /// shipped — the hand used to be the thing that could never outrun a machine. A slower machine
    /// that works unattended is still the better deal, and it makes automation a question of how
    /// many you can afford to run rather than of raw speed.
    ///
    /// Absent means an extractor cannot resolve a rate for it and falls back to the building's own
    /// `cadence`, which is what a pump does: water is the one source with no per-material figure
    /// because the pump is the only thing that draws it.
    #[serde(default)]
    extract_steps: Option<u32>,
    #[serde(default)]
    production_routes: Option<Vec<RecipeId>>,
    #[serde(default)]
    extraction_building_id: Option<DefinitionId>,
}

#[derive(Clone, Deserialize)]
struct RecipeDefinition {
    id: RecipeId,
    key: String,
    name: String,
    description: String,
    /// Which machines may run this. A kiln and a smelter are the same `BuildingKind` with
    /// different recipe categories — one field and one check at recipe assignment, rather than a
    /// new kind and a new tick path for every machine the material tree adds.
    category: String,
    inputs: Vec<Ingredient>,
    output: Ingredient,
    #[serde(default)]
    co_products: Vec<Ingredient>,
    #[serde(default)]
    cost_allocation: Vec<u32>,
    duration: u32,
    /// Energy one craft consumes, paid from whatever fuel item the machine has been fed. Zero for
    /// every recipe that needs no heat, which is what keeps charcoal reachable without coal.
    #[serde(default)]
    fuel: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct Ingredient {
    item_id: ItemId,
    quantity: u32,
}

#[derive(Clone, Deserialize)]
struct BuildingDefinition {
    id: DefinitionId,
    key: String,
    name: String,
    kind: BuildingKind,
    description: String,
    icon: String,
    #[serde(default)]
    cadence: Option<u32>,
    #[serde(default)]
    capacity: Option<u32>,
    /// The recipe category this machine can be assigned, for a composer-kind building. A kiln
    /// cannot be given a circuit recipe because its category does not match, not because a new
    /// `BuildingKind` exists for it.
    #[serde(default)]
    recipe_category: Option<String>,
    /// An explicit capability list replaces the category match for primitive equipment. It
    /// reuses recipe identities, so teaching a workshop timber does not create a second recipe.
    #[serde(default)]
    recipe_ids: Option<Vec<RecipeId>>,
    /// Manual stations run one batch only while the player is attending them. The existing
    /// disabled flag is their saved work permit; placement starts with that permit off.
    #[serde(default)]
    manual_work: bool,
    #[serde(default)]
    duration_multiplier: Option<u32>,
    /// What a pump produces. Data-defined for the same reason recipes are: a source building's
    /// output is content, not a branch in the tick.
    #[serde(default)]
    output_item_id: Option<ItemId>,
    /// Electricity this machine spends per tick of work. Zero or absent: no draw.
    ///
    /// A *rate against progress*, not against the clock. A machine that is blocked, starved, or
    /// out of recipe spends nothing, which is the difference between this and a per-tick tax: one
    /// craft costs `power_draw × duration` however long the machine stood idle first.
    #[serde(default)]
    power_draw: Option<u32>,
    /// Electricity offered every tick this generator is live, and the rate at which its fuel is
    /// worth that electricity: a generator running flat out spends exactly one unit of fuel energy
    /// per tick, so `power_output` is also the grid energy one fuel unit buys.
    #[serde(default)]
    power_output: Option<u32>,
    /// How far this pole supplies the machines around it.
    #[serde(default)]
    supply_radius: Option<u32>,
    /// How far this pole links to the next pole. Longer than `supply_radius` because spanning
    /// distance is what a line of poles is for.
    #[serde(default)]
    pole_reach: Option<u32>,
    #[serde(default)]
    power_source: Option<PowerSource>,
    /// Which orientations this building may take. Absent means the six hex edges, which is what
    /// every building built before v0.14 takes.
    #[serde(default)]
    orientation_axis: OrientationAxis,
    /// What one of the six corner headings costs, when that differs from `construction_cost`.
    ///
    /// The price of the two-row period, and the whole reason a belt and a riser can be one
    /// definition. A corner step covers `3 · size` against `√3 · size`, so charging it the edge
    /// price would make it strictly dominant; charging it here keeps the old riser's economics
    /// exactly while retiring the second building. Absent means the heading costs what every other
    /// heading on this definition costs, which is true of everything that is not transport.
    #[serde(default)]
    corner_construction_cost: Option<Vec<Ingredient>>,
    /// The technology this definition's corner headings wait behind, separately from the
    /// technology that unlocks the definition itself.
    ///
    /// A capability, not a building. The belt is the first thing the player ever builds and the
    /// two-row reach is a mid-game unlock, so the two cannot be the same gate — and inventing a
    /// second belt definition to carry the second gate is exactly the split this replaces.
    #[serde(default)]
    corner_technology_id: Option<TechnologyId>,
    /// Whether this transport building also rays its two flanks, and round-robins its cargo
    /// between every output that will take it.
    ///
    /// One flag rather than a `BuildingKind`, on the same terms a kiln is a composer: a splitter's
    /// *source* is not different, only the number of edges it compiles. The tick is unchanged —
    /// `transfer_cargo` still walks compiled edges — so this adds outputs to the graph and no path
    /// to the loop.
    #[serde(default)]
    splits: bool,
    /// Whether this transport building accepts from its feeders in rotation rather than in entity
    /// id order, so no lane that shares a junction can starve another.
    #[serde(default)]
    merges: bool,
    /// How many hexes this building's output ray may pass *over* before it binds.
    ///
    /// An underpass, and the only thing in the game whose ray does not stop at the first occupied
    /// cell it meets. Absent — every other building — means the ray binds to whatever it first
    /// reaches, which is the rule the transport graph has always had. Bounded by
    /// `MAX_UNDERPASS_SPAN` at load, because an unbounded span is a belt that costs nothing per
    /// hex.
    #[serde(default)]
    underpass_span: Option<u32>,
    /// Which cargo family a belt-kind transport carries. Existing definitions default to solid;
    /// pipes reuse the compiled graph and arbitration with a fluid-only acceptance boundary.
    #[serde(default)]
    transport_medium: TransportMedium,
    /// Optional exact filter for a container. Tanks name one loose fluid; an ordinary shelf omits
    /// the field and remains general storage.
    #[serde(default)]
    accepted_item_ids: Option<Vec<ItemId>>,
    /// Where this definition sits on its own upgrade ladder. Presentation reads it for trim; the
    /// simulation only ever compares it, and never branches on it.
    #[serde(default)]
    tier: u8,
    /// The definition `upgrade` turns this one into. A ladder is a chain of these, so a tier is a
    /// data row rather than a kind, a tick path, or a drawing.
    #[serde(default)]
    upgrades_to: Option<DefinitionId>,
    /// How many hex steps this extractor reaches, counting its own cell. Absent means
    /// `EXTRACT_RADIUS`. This is what makes reach the flagship upgrade: a longer arm is one number
    /// in this file, visible on the map, changing a decision the player already made.
    #[serde(default)]
    extract_radius: Option<u32>,
    /// How fast this extractor works its material, as a percentage of the item's `extract_steps`.
    /// Absent or 100 is the tier-one baseline: twice as long as the hand. 200 halves the cycle and
    /// puts the machine level with the hand; anything above that beats it.
    ///
    /// A percentage rather than a per-tier cadence because the ladder is the point — the same
    /// eight material figures are shared by every tier, so a new extractor is one number here and
    /// never a second table that can drift out of step with the first.
    #[serde(default)]
    extract_speed: Option<u32>,
    construction_cost: Vec<Ingredient>,
    #[serde(default)]
    unlock_technology_id: Option<TechnologyId>,
    placement_rule: PlacementRule,
    buildable: bool,
    blocks_movement: bool,
    #[serde(default = "default_footprint")]
    footprint: Vec<Coordinate>,
    /// How this building sits on uneven ground. Absent means a level pad: the occupied foundation
    /// may not span more than [`MAX_BUILD_STEP`] (legacy) or [`scale::MAX_BUILD_STEP_QUANTA`]
    /// (physical). `span` may follow a slope a player can still walk; `retaining` is the exception
    /// for walls, stairs and prepared foundations that create the grade they sit on.
    #[serde(default)]
    foundation_class: FoundationClass,
    /// Cells reserved at placement that are not solid occupancy. Neighbours cannot occupy them;
    /// a later upgrade may grow onto them without a second occupancy check. The player may still
    /// walk through. Empty means the atomic growth path: prove the extra cells at upgrade time.
    #[serde(default)]
    service_envelope: Vec<Coordinate>,
    /// Cells this building reserves in the air without occupying the ground. A turbine rotor is
    /// the type case: belts, poles and bridges may pass underneath, machines may not. Empty means
    /// the occupied footprint is the whole of what this building claims.
    #[serde(default)]
    overhead_clearance: Vec<Coordinate>,
}

/// How a building's occupied foundation may sit on finished grade.
///
/// Walking and construction no longer share one threshold. Ordinary machines need a pad; a belt
/// or a stair can follow a walkable slope; a retaining wall is the thing that *makes* the face.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum FoundationClass {
    #[default]
    Pad,
    Span,
    Retaining,
}

impl BuildingDefinition {
    fn supports_recipe(&self, recipe: &RecipeDefinition) -> bool {
        self.kind == BuildingKind::Composer
            && self.recipe_ids.as_ref().map_or_else(
                || self.recipe_category.as_deref() == Some(recipe.category.as_str()),
                |ids| ids.contains(&recipe.id),
            )
    }

    fn recipe_duration(&self, recipe: &RecipeDefinition) -> u32 {
        recipe.duration * self.duration_multiplier.unwrap_or(1)
    }

    /// What one of this building costs when built at that heading.
    ///
    /// The single place the two-row price lives. Every charge, refund, preview budget, and upgrade
    /// netting goes through here, so a corner belt is priced the same whichever of those five paths
    /// reaches it — the way the riser's own `construction_cost` row used to guarantee by existing.
    fn cost_at(&self, orientation: u8) -> &[Ingredient] {
        match &self.corner_construction_cost {
            Some(cost) if is_corner_heading(orientation) => cost,
            _ => &self.construction_cost,
        }
    }

    /// The technology this building waits behind at that heading: its own gate, and — on a corner —
    /// the separate gate the two-row reach waits behind.
    fn gates_at(&self, orientation: u8) -> [Option<TechnologyId>; 2] {
        let corner = if is_corner_heading(orientation) {
            self.corner_technology_id
        } else {
            None
        };
        [self.unlock_technology_id, corner]
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum BuildingKind {
    Extractor,
    Belt,
    Composer,
    Container,
    Consumer,
    Hub,
    /// Draws from water terrain rather than from a field cell, and never depletes it. That is why
    /// it is a kind of its own and the smelter, kiln, cutter, and crusher are not: they are all a
    /// composer running a recipe, and a pump is a different source.
    Pump,
    Pole,
    Generator,
    Boiler,
    /// A support deck on shallow water. Terrain stays water; this entity is what permits a
    /// transport building to occupy an otherwise unbuildable ford.
    Bridge,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum TransportMedium {
    #[default]
    Solid,
    Fluid,
}

/// Whether a building of this kind could ever take a delivered item — whatever it holds, whatever
/// recipe it is given, whatever the hub is asking for today.
///
/// This is the *static* question, over the kind alone, and it is deliberately a separate predicate
/// from `accepts_item`. That one answers *would you want this one item, right now*, which changes
/// with a recipe, a fuel, or a contract, and construction must not be decided by an answer that can
/// change a tick later. This one never changes, so a graph edge into such a target is a dead edge
/// worth refusing to compile and worth refusing to build.
fn never_accepts_deliveries(kind: BuildingKind) -> bool {
    matches!(
        kind,
        BuildingKind::Extractor | BuildingKind::Pump | BuildingKind::Pole | BuildingKind::Bridge
    )
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum PowerSource {
    Burner,
    Wind,
    Hydro,
    Turbine,
}

/// Which of the twelve routing headings a definition may be built at.
///
/// `Edge` is the six hex edges and the default, so every definition that predates tiers keeps
/// exactly the orientations it had. `Corner` is the six vertex headings, for anything that spans
/// only the two-row period. `Any` is both, and is what the belt takes.
///
/// The axis is a price as much as a permission. A vertex heading covers `3 · size` of world
/// distance against `√3 · size` for an edge step, so a heading a definition may take for free
/// would be strictly dominant. `Edge` and `Corner` answer that by being separate definitions with
/// separate `construction_cost` rows; `Any` answers it inside one definition, with
/// `corner_construction_cost` and `corner_technology_id`.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum OrientationAxis {
    #[default]
    Edge,
    Corner,
    /// Both families, so rotation walks all twelve headings in clockwise order.
    ///
    /// This is what makes a belt and a riser one building rather than two. The reason the axes
    /// were separated — that a corner heading covers `3 · size` against `√3 · size` and would be
    /// strictly dominant at a belt's price — is answered by `corner_construction_cost` instead:
    /// the heading still costs what it covers, so the choice stays a real one while the player
    /// builds, drags, and rotates a single thing. A definition on this axis must also name the
    /// research its corner headings wait behind, or the two-row reach would arrive with the first
    /// belt of the game.
    Any,
}

impl OrientationAxis {
    /// The half-open range of orientation indices this axis allows.
    fn range(self) -> std::ops::Range<u8> {
        match self {
            Self::Edge => 0..NORTH,
            Self::Corner => NORTH..TRANSPORT_DIRECTIONS.len() as u8,
            Self::Any => 0..TRANSPORT_DIRECTIONS.len() as u8,
        }
    }

    fn allows(self, orientation: u8) -> bool {
        self.range().contains(&orientation)
    }

    /// The next orientation one `rotate` along. Rotation stays inside the axis, so edge and corner
    /// definitions each walk six headings in clockwise order.
    ///
    /// `Any` walks all twelve, and walks them in *angular* order rather than in table order. The
    /// table lists the six edges and then the six corners, so stepping its indices would turn a
    /// belt through every edge before it reached the first corner — six presses of `R` to nudge a
    /// heading by 30°. The two interleavings below are that ordering and nothing more: a corner
    /// heading sits in the 30° gap after edge `e` at `NORTH + (e + 2) % 6`, and the edge after that
    /// corner is `(k + 5) % 6`. `rotation_walks_every_heading_once_in_angular_order` pins both
    /// against the world vectors rather than against these expressions.
    fn next(self, orientation: u8) -> u8 {
        if self == Self::Any {
            return if orientation < NORTH {
                NORTH + (orientation + 2) % 6
            } else {
                (orientation - NORTH + 5) % 6
            };
        }
        let range = self.range();
        let span = range.end - range.start;
        let offset = orientation.wrapping_sub(range.start);
        range.start + (offset.wrapping_add(1) % span)
    }

    fn previous(self, orientation: u8) -> u8 {
        if self == Self::Any {
            return if orientation < NORTH {
                NORTH + (orientation + 1) % 6
            } else {
                (orientation - NORTH + 4) % 6
            };
        }
        let range = self.range();
        let span = range.end - range.start;
        let offset = orientation.wrapping_sub(range.start);
        range.start + (offset.wrapping_add(span - 1) % span)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Link {
    /// `None` is the legacy/default route shared by every offered item. A named item is one
    /// independently configured product outlet.
    item_id: Option<ItemId>,
    target: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LinkId {
    item_id: Option<ItemId>,
    target_id: u32,
}

/// One entity's outgoing edges named by stable entity id rather than by vector index.
///
/// What an incremental recompile carries across an edit: erasing shifts every index after the hole,
/// so the edges that were *not* affected have to survive as ids and be resolved back afterwards.
type LinkIds = [Option<LinkId>; MAX_LINKS];

/// One entity's outgoing transport edges, in the order they were compiled.
///
/// Ordinary transport has exactly one and the whole game had exactly one before splitters existed,
/// which is why `primary` is kept as its own word: everything that asks "where does this belt
/// deliver" — the snapshot's `next_id`, the blocked-output status, the connecting deck the renderer
/// draws — is still asking about the first edge, and reads the same on a building that will never
/// have a second.
///
/// Fixed width and `Copy`. See `MAX_LINKS`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Links {
    edges: [Option<Link>; MAX_LINKS],
}

impl Links {
    /// The one-edge graph every non-splitting building compiles.
    fn single(target: Option<usize>) -> Self {
        let mut links = Self::default();
        if let Some(target) = target {
            links.edges[0] = Some(Link {
                item_id: None,
                target,
            });
        }
        links
    }

    /// Every distinct target this entity delivers to, in compile order.
    ///
    /// Two products may use the same belt. Reverse feeder indexes still need that source once,
    /// not once per product, or merger arbitration would silently weight the source.
    fn iter(self) -> impl Iterator<Item = usize> {
        self.edges
            .into_iter()
            .enumerate()
            .filter_map(move |(index, edge)| {
                let edge = edge?;
                (!self.edges[..index]
                    .iter()
                    .flatten()
                    .any(|previous| previous.target == edge.target))
                .then_some(edge.target)
            })
    }

    /// The edges this item may actually take. Once a product has a named route, the wildcard is
    /// no longer a fallback for it — a disconnected configured port must stay disconnected.
    fn iter_for(self, item_id: ItemId) -> impl Iterator<Item = usize> {
        let named = self
            .edges
            .iter()
            .flatten()
            .any(|edge| edge.item_id == Some(item_id));
        self.edges.into_iter().flatten().filter_map(move |edge| {
            (edge.item_id == Some(item_id) || (!named && edge.item_id.is_none()))
                .then_some(edge.target)
        })
    }

    /// The first outgoing edge, which for everything but a splitter is the only one.
    fn primary(self) -> Option<usize> {
        self.edges[0].map(|edge| edge.target)
    }

    fn is_empty(self) -> bool {
        self.edges[0].is_none()
    }

    /// Add one edge, keeping the slots packed from the front.
    ///
    /// A repeated target is dropped rather than stored twice. A splitter whose flank ray reaches
    /// the same building its facing ray reached has *one* consumer, not two, and storing it twice
    /// would hand that consumer two of every three items — a round robin that silently weights
    /// itself by geometry.
    fn push(&mut self, target: usize) {
        self.push_item(None, target);
    }

    fn push_item(&mut self, item_id: Option<ItemId>, target: usize) {
        if self
            .edges
            .iter()
            .flatten()
            .any(|existing| existing.item_id == item_id && existing.target == target)
        {
            return;
        }
        if let Some(slot) = self.edges.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(Link { item_id, target });
        }
    }
}

/// A product outlet stored relative to the entity anchor, in world orientation.
///
/// The cell is one real footprint tile and the direction is one of its six exterior sides. It is
/// saved and checksummed: two otherwise equal refineries that send fuel in different directions
/// are different factories and must reload that way.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
struct OutputRoute {
    q: i32,
    r: i32,
    direction: u8,
}

/// Whether a routing heading is one of the six vertex headings rather than one of the six edges.
///
/// The one predicate for "does this heading span the two-row period", asked by the cost rule, by
/// the drag router's step weights, and by the flank rule. `NORTH` is the boundary and always was;
/// this names it so the comparison is not spelled out at each call site.
fn is_corner_heading(orientation: u8) -> bool {
    orientation >= NORTH && usize::from(orientation) < TRANSPORT_DIRECTIONS.len()
}

/// The two headings 60° either side of this one, inside its own family.
///
/// A splitter's flanks. Rotation here is *within the six* the heading belongs to — an edge heading
/// flanks to edges and a corner heading to corners — because 60° either side of a heading is the
/// pair of headings that share its period. Taking a flank across families would hand a belt-priced
/// splitter a two-row output, which is the same dominance `corner_construction_cost` exists to
/// price.
fn flanks_of(orientation: u8) -> [u8; 2] {
    let base = if is_corner_heading(orientation) {
        NORTH
    } else {
        0
    };
    let offset = orientation - base;
    [base + (offset + 1) % 6, base + (offset + 5) % 6]
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum PlacementRule {
    Ground,
    Resource,
    /// Buildable ground with open water inside `PUMP_RADIUS`.
    Water,
    /// Hills or highland — the same bands iron, coal, and copper already occupy.
    Elevated,
    /// On a shallow-water hex. Deep water remains a barrier and terrain itself is unchanged.
    Shallows,
}

#[derive(Clone, Deserialize)]
struct TechnologiesInput {
    version: u16,
    branches: Vec<ProgressionGroup>,
    stages: Vec<ProgressionGroup>,
    technologies: Vec<TechnologyDefinition>,
    skills: Vec<SkillDefinition>,
    skill_milestones: Vec<SkillMilestone>,
}

/// Authored presentation metadata. Never a purchase gate or saved simulation state.
#[derive(Clone, Deserialize)]
struct ProgressionGroup {
    key: String,
    name: String,
    description: String,
    order: u32,
}

#[derive(Clone, Deserialize)]
struct TechnologyDefinition {
    id: TechnologyId,
    key: String,
    name: String,
    description: String,
    branch: String,
    stage: String,
    prerequisites: Vec<TechnologyId>,
    cost: u32,
    #[serde(default)]
    effects: Vec<TechnologyEffect>,
    /// Insight purchase unless a contract stage grants this on completion.
    #[serde(default)]
    grant: TechnologyGrant,
}

/// A supported native capability this technology grants when complete.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TechnologyEffect {
    UnlockBuilding { building_id: DefinitionId },
    UnlockBoundary { boundary_id: DefinitionId },
    UnlockSurface { surface_id: DefinitionId },
    CarrySlots { amount: u32 },
    BuildRange { amount: u32 },
}

/// How this technology enters the researched set. Purchases spend insight;
/// contract-stage grants are issued by native on stage completion and cannot be bought.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TechnologyGrant {
    #[default]
    Purchase,
    ContractStage {
        key: String,
        name: String,
    },
}

impl TechnologyDefinition {
    fn purchasable(&self) -> bool {
        matches!(self.grant, TechnologyGrant::Purchase)
    }

    fn building_unlocks(&self) -> impl Iterator<Item = DefinitionId> + '_ {
        self.effects.iter().filter_map(|effect| match effect {
            TechnologyEffect::UnlockBuilding { building_id } => Some(*building_id),
            _ => None,
        })
    }

    fn boundary_unlocks(&self) -> impl Iterator<Item = DefinitionId> + '_ {
        self.effects.iter().filter_map(|effect| match effect {
            TechnologyEffect::UnlockBoundary { boundary_id } => Some(*boundary_id),
            _ => None,
        })
    }

    fn carry_slots_bonus(&self) -> u32 {
        self.effects
            .iter()
            .filter_map(|effect| match effect {
                TechnologyEffect::CarrySlots { amount } => Some(*amount),
                _ => None,
            })
            .fold(0, u32::saturating_add)
    }

    fn build_range_bonus(&self) -> u32 {
        self.effects
            .iter()
            .filter_map(|effect| match effect {
                TechnologyEffect::BuildRange { amount } => Some(*amount),
                _ => None,
            })
            .fold(0, u32::saturating_add)
    }
}

#[derive(Clone, Deserialize)]
struct ScenariosInput {
    version: u16,
    scenarios: Vec<ScenarioDefinition>,
}

#[derive(Clone, Deserialize)]
struct ScenarioDefinition {
    id: u16,
    key: String,
    name: String,
    description: String,
    version: u16,
    seed: u32,
    /// The preset this scenario generates under when the caller names none. A scenario that
    /// generates no environment does not need one.
    #[serde(default)]
    world_preset: Option<String>,
    chunk_size: i32,
    generated_environment: bool,
    player_spawn: Coordinate,
    player_facing: u8,
    build_range: u32,
    /// How many stacks the player can carry at once. Containers exist to solve this.
    carry_slots: u32,
    /// What the landing hub is actually asking for, in order. A scenario states a demand rather
    /// than a single delivery total, because a founding project is the thing that gives an economy
    /// a reason to exist and one item's counter cannot express it.
    contract: ContractDefinition,
    #[serde(default)]
    initial_inventory: Vec<Ingredient>,
    #[serde(default)]
    initial_researched: Vec<TechnologyId>,
    #[serde(default)]
    resources: Vec<ScenarioResource>,
    buildings: Vec<PlacedBuilding>,
}

/// The landing hub's standing demand: an ordered list of stages, each a bill of materials.
///
/// A stage is not a quest generator and not a wall. It is one bounded thing the hub is building,
/// stated as data so it can be delivered against, saved, checksummed, and read on screen without
/// any of the three re-deriving what the other two believe.
#[derive(Clone, Deserialize)]
struct ContractDefinition {
    key: String,
    name: String,
    stages: Vec<ContractStage>,
}

#[derive(Clone, Deserialize)]
struct ContractStage {
    key: String,
    name: String,
    /// One paragraph the host can put in front of the player. Native owns it so the sentence and
    /// the bill can never disagree about which stage is current.
    brief: String,
    /// What completing this stage does to the hub on screen, in words, so the drawing has
    /// something to be checked against the same way `TierStep::reads` does.
    reads: String,
    requirements: Vec<Ingredient>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct Coordinate {
    q: i32,
    r: i32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct ScenarioResource {
    q: i32,
    r: i32,
    item_id: ItemId,
    quantity: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct PlacedBuilding {
    q: i32,
    r: i32,
    definition_id: DefinitionId,
    orientation: u8,
    #[serde(default)]
    recipe_id: Option<RecipeId>,
    #[serde(default)]
    scenario_owned: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct Cargo {
    item_id: ItemId,
    quantity: u32,
}

/// One item crossing a belt's lane, and the tick it stepped onto it.
///
/// The tick is stored rather than a countdown so that nothing has to be decremented every tick: a
/// lane is pure arithmetic against `Core::tick`, which keeps a hundred thousand belts free when
/// nothing about them is changing, keeps a delta snapshot from re-sending every belt every tick,
/// and lets the host extrapolate an item's position between snapshots from a number that does not
/// go stale.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct LaneItem {
    cargo: Cargo,
    entered: u64,
}
