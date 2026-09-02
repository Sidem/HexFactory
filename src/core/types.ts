import type { AxialCoordinate } from "@hexlife/embed/hex";

export type BuildingKind =
  | "extractor"
  | "belt"
  | "composer"
  | "container"
  | "consumer"
  | "hub"
  | "pump"
  | "pole"
  | "generator"
  | "boiler"
  | "bridge";
export type Terrain =
  | "deep_water"
  | "shallow_water"
  | "shore"
  | "lowland"
  | "hills"
  | "highland"
  | "cliff";
/**
 * What a cell's bed is made of, independent of the water standing on it. A player-readable material
 * family rather than an altitude band: `Terrain` still ships as the presentation the shaders read,
 * but legality and look increasingly come from this plus height and water depth.
 */
export type Substrate = "sand" | "meadow" | "soil" | "rock";
export type PlacementRule =
  | "ground"
  | "resource"
  | "water"
  | "elevated"
  | "shallows";
/**
 * How a building's occupied foundation may sit on finished grade. Absent means a level pad.
 * `span` may follow a walkable slope; `retaining` is the exception for walls, stairs and
 * prepared foundations.
 */
export type FoundationClass = "pad" | "span" | "retaining";
export type PowerSource = "burner" | "wind" | "hydro" | "turbine";
/**
 * Which routing headings a definition may be built at. `edge` is the six hex edges and the
 * default; `corner` is the six vertex headings, each spanning the two-row period; `any` is all
 * twelve, which is what lets one belt definition carry both periods instead of two definitions
 * carrying one each. An `any` definition prices its corner headings separately, so the longer
 * reach still costs what it covers.
 */
export type OrientationAxis = "edge" | "corner" | "any";

export interface Ingredient {
  item_id: number;
  quantity: number;
}

export interface ItemDefinition {
  erosion_resistance?: number;
  id: number;
  key: string;
  name: string;
  color: string;
  icon: string;
  description: string;
  /** How many of this item fill one carried slot. The rule itself lives in Rust. */
  stack_size: number;
  /** Loose liquid: pipe cargo and machine stock, never a fresh belt or player-carried stack. */
  fluid?: boolean;
  /** Energy one unit releases when burned, for an item that is fuel. */
  fuel_value?: number;
  /** Ticks between one unit of regrowth and the next, for a resource that is flora. */
  regrowth_ticks?: number;
  /**
   * Player-clock steps between hand gathers of this item. Absent means the hand cannot take it
   * at all — water is pumped, signal crystal is extracted.
   */
  hand_gather_steps?: number;
  /**
   * Simulation ticks a tier-one extractor spends on one unit of this material, before the
   * building's own `extract_speed` scales it. Absent means no extractor prices it and the machine
   * falls back to its own cadence, which is water and the pump.
   */
  extract_steps?: number;
  /** Explicit preference order when more than one recipe makes this item. */
  production_routes?: number[];
  /** A specialized field source; ordinary extractors cannot harvest this item. */
  extraction_building_id?: number;
}

export interface RecipeDefinition {
  id: number;
  key: string;
  name: string;
  description: string;
  /** Which machines may run this. A kiln cannot be given a smelting recipe. */
  category: string;
  inputs: Ingredient[];
  output: Ingredient;
  /** Produced atomically with the primary output, into the shared output buffer. */
  co_products?: Ingredient[];
  /** Percentage shares, in output/co-product order. Required for joint outputs; sum is 100. */
  cost_allocation?: number[];
  duration: number;
  /** Energy one craft consumes, paid from whatever fuel the machine has been fed. */
  fuel?: number;
}

export interface BuildingDefinition {
  id: number;
  key: string;
  name: string;
  kind: BuildingKind;
  description: string;
  icon: string;
  cadence?: number;
  capacity?: number;
  /** The recipe category a composer-kind machine may be assigned. */
  recipe_category?: string;
  /** Authored appearance for specialized sources; never a simulation rule. */
  source_category?: string;
  /** Explicit supported recipe IDs, replacing the category match on primitive equipment. */
  recipe_ids?: number[];
  /** One attended batch per work command; native owns the work permit and progress. */
  manual_work?: boolean;
  duration_multiplier?: number;
  /** What a source building produces, for a pump. */
  output_item_id?: number;
  /** Electricity this machine spends per tick of work. Idle time is free. */
  power_draw?: number;
  /** Electricity this generator offers every tick it is live. */
  power_output?: number;
  /** How far this pole supplies the machines around it. Poles always name it. */
  supply_radius?: number;
  /** How far this pole links to the next pole. Poles always name it. */
  pole_reach?: number;
  /** How a generator makes electricity. */
  power_source?: PowerSource;
  /** Which headings this building may take. Absent means the six hex edges. */
  orientation_axis?: OrientationAxis;
  /**
   * What one of the six corner headings costs, when that differs from `construction_cost`. A
   * corner step covers twice the ground, so it is priced at what it covers — which is what lets
   * the belt carry both periods without the longer one being strictly the better buy.
   */
  corner_construction_cost?: Ingredient[];
  /** The research this definition's corner headings wait behind, separately from the building. */
  corner_technology_id?: number;
  /** Whether this transport also feeds the two headings either side of the one it faces. */
  splits?: boolean;
  /** Whether this transport takes from its feeders in rotation rather than in entity id order. */
  merges?: boolean;
  /** How many hexes this building's output may pass over before it binds to its partner. */
  underpass_span?: number;
  /** Cargo family for a belt-kind transport. Omitted means an ordinary solid belt. */
  transport_medium?: "solid" | "fluid";
  /** Exact contents of a filtered container such as a water or oil tank. */
  accepted_item_ids?: number[];
  /** Where this definition sits on its own upgrade ladder. Absent means the base tier. */
  tier?: number;
  /** The definition an `upgrade` turns this one into, if it has a next tier. */
  upgrades_to?: number;
  /** How many hexes this extractor or pump reaches, counting its own. */
  extract_radius?: number;
  /**
   * How fast this extractor works its material, as a percentage of the item's `extract_steps`.
   * 100 is the tier-one baseline of twice the hand's time; 200 is level with the hand.
   */
  extract_speed?: number;
  construction_cost: Ingredient[];
  unlock_technology_id?: number;
  placement_rule: PlacementRule;
  buildable: boolean;
  blocks_movement: boolean;
  footprint: AxialCoordinate[];
  /** Absent means a level pad. */
  foundation_class?: FoundationClass;
  /** Reserved cells that are not solid occupancy. A later upgrade may grow onto them. */
  service_envelope?: AxialCoordinate[];
  /** Air reservation: belts may pass underneath, machines may not. */
  overhead_clearance?: AxialCoordinate[];
}

export interface Definitions {
  boundaries: BoundaryDefinition[];
  surfaces: SurfaceDefinition[];
  version: number;
  items: ItemDefinition[];
  recipes: RecipeDefinition[];
  buildings: BuildingDefinition[];
  requests: RequestDefinition[];
}

/**
 * One project the landing hub can post. The only thing in the game that pays insight, and it says
 * what it pays before anything is handed over.
 *
 * It also pays once. The catalogue is a bill of finite work, so the sum of every `insight` here is
 * the total research budget of a save — which is why `validate_research_budget` can assert the tree
 * is affordable at all.
 */
export interface RequestDefinition {
  id: number;
  key: string;
  name: string;
  brief: string;
  item_id: number;
  quantity: number;
  insight: number;
}

/** Authored presentation only; neither order nor stage imposes a gameplay gate. */
export interface ProgressionGroup {
  key: string;
  name: string;
  description: string;
  order: number;
}

/** A supported native capability this technology grants when complete. */
export type TechnologyEffect =
  | { kind: "unlock_building"; building_id: number }
  | { kind: "unlock_boundary"; boundary_id: number }
  | { kind: "unlock_surface"; surface_id: number }
  | { kind: "carry_slots"; amount: number }
  | { kind: "build_range"; amount: number };

/**
 * How this technology enters the researched set. Omitted or `purchase` is an
 * insight spend. A contract-stage grant is issued by native on stage completion
 * and cannot be bought.
 */
export type TechnologyGrant =
  | { kind: "purchase" }
  | { kind: "contract_stage"; key: string; name: string };

export interface TechnologyDefinition {
  id: number;
  key: string;
  name: string;
  description: string;
  branch: string;
  stage: string;
  prerequisites: number[];
  cost: number;
  effects: TechnologyEffect[];
  grant?: TechnologyGrant;
}

export type SkillEffect = {
  kind: "carry_slots" | "build_range" | "survey_range";
  amount: number;
};
export interface SkillDefinition {
  id: number;
  key: string;
  name: string;
  description: string;
  branch: "carrying" | "construction" | "surveying";
  prerequisites: number[];
  cost: number;
  effect: SkillEffect;
  legacy_technology_id?: number;
}
export interface SkillMilestone {
  id: number;
  key: string;
  name: string;
  description: string;
  points: number;
  event:
    | { kind: "workshop_craft" | "powered_craft" }
    | { kind: "contract_stage"; key: string };
}
export interface SkillsSnapshot {
  points: number;
  purchased: number[];
  granted: number[];
  completed: number[];
  sandbox: boolean;
  availability: {
    skill_id: number;
    complete: boolean;
    points_shortfall: number;
    current_value: number;
    resulting_value: number;
    missing_prerequisites: number[];
  }[];
}

export interface Technologies {
  version: number;
  branches: ProgressionGroup[];
  stages: ProgressionGroup[];
  technologies: TechnologyDefinition[];
  skills: SkillDefinition[];
  skill_milestones: SkillMilestone[];
}

/** Native purchase predicates, published together and reused by every research view. */
export interface ResearchAvailability {
  technology_id: number;
  complete: boolean;
  missing_prerequisites: number[];
  insight_shortfall: number;
}

export interface ScenarioDefinition {
  id: number;
  key: string;
  name: string;
  description: string;
  version: number;
  seed: number;
}

export interface Scenarios {
  version: number;
  scenarios: ScenarioDefinition[];
}

/**
 * One row of the generator's resource table: what a *deposit* is made of, how wide it is, and
 * where its centre may stand.
 *
 * A deposit is a **site** rather than a per-hex decision, so a row no longer competes with the
 * others hex by hex — the lattice picks one rule per site, which is what makes a patch one
 * material by construction. Row order is not a generation input.
 *
 * `-1` on `site_min` means the rule is not asking about the richness channel at all.
 */
export interface SiteRule {
  terrain: Terrain;
  item_id: number;
  /** Relative share among the rules eligible for a band. Zero means never. */
  weight: number;
  /** Inclusive radius range in hexes. A disc of radius R holds 3R² + 3R + 1 hexes. */
  radius_min: number;
  radius_max: number;
  site_min: number;
  /** Yield at the centre and at the rim, interpolated by distance and then jittered. */
  yield_core: number;
  yield_rim: number;
  yield_jitter: number;
  /** Bands a hex must itself be in to belong to the site. Empty means the rule's own band. */
  member: Terrain[];
  /** A member hex must also be this many hexes from water. `0` disables it. */
  member_water_within: number;
  /** The centre must stand against ocean rather than against any pond. */
  center_ocean: boolean;
  /** The centre must stand next to the shore band — lake and sea beaches both qualify. */
  center_shore: boolean;
}

/**
 * The parameters a world is generated from. Simulation truth, unlike the shape grammar: these
 * travel in the save envelope and the checksum, so two worlds sharing a seed and differing here
 * are different worlds.
 *
 * Feature scale and threshold are separate axes. Raising `water_level` makes *more* water, not
 * *bigger* water — bigger water comes from `elevation_coarse_cell`.
 */
export interface WorldParams {
  elevation_coarse_cell: number;
  elevation_fine_cell: number;
  elevation_coarse_weight: number;
  moisture_cell: number;
  richness_cell: number;
  water_level: number;
  shore_level: number;
  hills_level: number;
  highland_level: number;
  cliff_step: number;
  deep_water_moisture: number;
  /** How far apart deposits stand, and how far a site centre may wander inside its own cell. */
  site_cell: number;
  site_jitter: number;
  /** How far apart rivers run, how wide one is (`0` is a world without them), where they stop. */
  river_cell: number;
  river_width: number;
  river_max_elevation: number;
  /** The cut the coarse elevation octave alone is read against when a rule asks for ocean. */
  ocean_level: number;
  site_rules: SiteRule[];
}

/**
 * One deposit as a preview draws it: where its centre lands in preview pixels, how far it reaches
 * there, and what it holds. Native reports centres rather than per-pixel samples because a patch is
 * smaller than a pixel at any zoom wide enough to frame a landform.
 */
export interface WorldPreviewSite {
  item_id: number;
  x: number;
  y: number;
  radius: number;
}

/**
 * A rectangle of generated terrain for a parameter set nobody has played yet, straight from the
 * generator the start button will run. `cells` is one byte per pixel in row-major order, holding the
 * band's index in `TERRAIN_ORDER` — the declaration order `fixtures/terrain-passability.json` pins
 * on both sides of the wire.
 *
 * `unmet` names materials the bootstrap pass could not place. `Core::new` refuses a world over
 * exactly that list, so it travels with the picture rather than being discovered at start.
 */
export interface WorldPreview {
  width: number;
  height: number;
  cells: Uint8Array;
  sites: WorldPreviewSite[];
  /** Deposits the window holds, which is not always how many of them are in `sites`. */
  total: number;
  /**
   * Whether the window holds more deposits than native was willing to send. `sites` is then empty
   * without the world being empty, which is the case this flag exists to keep apart.
   */
  dense: boolean;
  unmet: number[];
  /** What each unmet material was looking for. Empty whenever `unmet` is. */
  needs: WorldPreviewNeed[];
  /** A verified way out of a refused world, or null when there is none and when none was needed. */
  repair: WorldPreviewRepair | null;
}

/**
 * Why one material could not be placed: the bands its rules could seat a centre in, and whether the
 * opening holds any of that ground at all.
 *
 * The two cases want different sentences. `ground: false` is "this world has no such ground near the
 * landing site", which no seed will fix; `ground: true` is "the ground is there and no patch on it
 * was big enough", which a reroll often will.
 */
export interface WorldPreviewNeed {
  item_id: number;
  /** Band keys, spelled as `TERRAIN_ORDER` in `src/core/terrain.ts` spells them. */
  bands: string[];
  ground: boolean;
}

/**
 * One knob a repair turns, under the same field name {@link WorldParams} uses — which is what lets
 * the host label it from its own parameter table instead of native shipping prose.
 */
export interface WorldPreviewChange {
  field: string;
  from: number;
  to: number;
}

/**
 * A way out of a world that cannot be started, every candidate of it checked against a real
 * bootstrap pass before it was offered. Both halves may be present: they are two different prices,
 * and which is worth paying is the player's call.
 */
export interface WorldPreviewRepair {
  /** A seed that opens the world with every parameter left where the player put it. */
  seed: number | null;
  /** Changes that open the world with the seed left alone. Empty when the search found none. */
  changes: WorldPreviewChange[];
}

/** A named parameter set. The preset is what a player picks; the parameters are behind it. */
export interface WorldPreset {
  key: string;
  name: string;
  description: string;
  params: WorldParams;
}

export type Cargo = Ingredient;

/**
 * One item still crossing a belt hex. `entered` is the simulation tick it stepped on, so the host
 * can draw it at `(tick - entered) / belt_transit_ticks` of the way over the belt without the
 * fraction itself travelling on the wire.
 */
export interface LaneItem {
  cargo: Cargo;
  entered: number;
}

export interface OutputRouteSnapshot extends AxialCoordinate {
  item_id: number;
  /** One of the six exterior footprint sides, clockwise from east. */
  direction: number;
  /** The native compiled target, or null when this port currently reaches nothing. */
  target_id?: number | null;
}

/** The native-resolved water cell a pump draws, including the rate that limits it. */
export interface WaterSourceSnapshot extends AxialCoordinate {
  /** Standing depth for finite water, or the current river depth. */
  available: number;
  /** Zero for a finite pond; non-zero for a replenishing river source. */
  discharge: number;
  /** Maximum source withdrawals per simulation tick. */
  rate: number;
}

export interface EntitySnapshot extends AxialCoordinate {
  id: number;
  definition_id: number;
  kind: BuildingKind;
  orientation: number;
  /**
   * Absent as `null`, not as a missing key: native sends these three as `Option` without skipping
   * the empty case, so the JSON path has always delivered an explicit `null` and the binary
   * decoder reproduces it. Every reader treats the two the same — `?? 0`, or a truthiness test —
   * but the declaration should say what actually arrives.
   */
  recipe_id?: number | null;
  scenario_owned: boolean;
  cargo?: Cargo | null;
  /**
   * Items still crossing this belt, oldest first. Omitted when empty, which is every machine,
   * container and idle belt. `cargo` is the item that has finished crossing and waits to be
   * handed on.
   */
  lane?: LaneItem[];
  inventory: Ingredient[];
  input_inventory?: Ingredient[];
  fuel_inventory?: Ingredient[];
  output_inventory?: Ingredient[];
  /** Effective route for every product, including unchanged legacy-facing defaults. */
  output_routes?: OutputRouteSnapshot[];
  /** Present on a pump while a native-resolved water cell remains in reach. */
  water_source?: WaterSourceSnapshot | null;
  progress: number;
  progress_total: number;
  /**
   * Energy the machine is holding, and what one craft of its recipe costs. Both are omitted from
   * the wire when zero, which is what they are for everything that is not a furnace.
   */
  fuel_charge?: number;
  fuel_required?: number;
  /**
   * Network supply and demand this entity is on, both published so the host can draw a
   * proportion. Omitted when the entity is not on a power network.
   */
  power_satisfied?: number;
  power_demand?: number;
  /**
   * Electricity this machine has banked, against the buffer it fills to. A machine buys work out
   * of this rather than out of a network ratio, so a draining bank is what a brownout looks like.
   */
  power_charge?: number;
  power_capacity?: number;
  status: string;
  next_id?: number | null;
  /**
   * The outputs after the first, which only a splitter ever has. Absent or empty on everything
   * else, so a lane that fans out is visible in the snapshot rather than inferred from geometry.
   */
  branch_ids?: number[];
  footprint: AxialCoordinate[];
}

export interface WorldPoint {
  x: number;
  y: number;
}

export type StockKind = "auto" | "inventory" | "input" | "fuel" | "output";

/**
 * One field cell. `q`/`r` is its identity — the tile key native stores it under and the key a
 * patch addresses it by. There is deliberately no separate numeric id: a packed 64-bit one cannot
 * survive JSON, which carries numbers as doubles.
 */
export interface ResourceSnapshot extends WorldPoint {
  q: number;
  r: number;
  radius: number;
  item_id: number;
  quantity: number;
  initial_quantity: number;
}

/**
 * One surveyed cell of generated ground.
 *
 * Every cell of every surveyed chunk appears, plain lowland included: the band used to be the whole
 * payload, so a lowland tile carried nothing and was skipped, but a per-cell height has no default
 * for the host to fill a gap with.
 *
 * `height` and `water_depth` are the *generated* bed in native height units — signed, absolute, sea
 * level at zero. Whatever the player cut or filled arrives separately in the ground group, and
 * whatever water they moved arrives separately in the water group. The host adds each overlay
 * exactly as native does. That is what lets a tile be published once and never revisited.
 */
export interface TerrainSnapshot extends WorldPoint {
  q: number;
  r: number;
  radius: number;
  terrain: Terrain;
  height: number;
  substrate: Substrate;
  water_depth: number;
  discharge: number;
}

/**
 * A generated world chunk. `x`/`y`/`span` are the native world-space square the chunk owns, so the
 * reported chunks are exactly the surveyed world and everything else is unexplored.
 */
export interface ChunkSnapshot extends WorldPoint {
  chunk_q: number;
  chunk_r: number;
  entity_count: number;
  span: number;
}

export interface PlayerSnapshot extends WorldPoint {
  facing_x: number;
  facing_y: number;
  move_x: number;
  move_y: number;
  inventory: Record<string, number>;
  /** The native-owned stack currently attached to the pointer, outside the pack's slots. */
  hand?: Cargo | null;
  /**
   * Work still outstanding on the field action in flight, in player steps. It is the swing itself
   * rather than a wait after one: nothing is taken until this reaches zero.
   */
  action_cooldown: number;
  build_range: number;
  /** How many stacks the player can carry at once. */
  carry_slots: number;
  /**
   * The carried inventory laid out one entry per occupied slot, resolved natively. The host draws
   * these and pads to `carry_slots`; it never re-derives the stacking rule for itself.
   */
  carry_stacks: Ingredient[];
  /** Collision and drawing radius in native world units. */
  radius: number;
  /**
   * What a whole swing is worth. Progress is drawn as `action_cooldown` against this, so the host
   * never infers the maximum by watching the value fall.
   */
  action_cooldown_total: number;
  /** How many hexes the hand can gather across, published by native for the held-action ring. */
  extract_radius: number;
  /**
   * Whether this run is creative: everything researched, construction free, and nothing recovered
   * when a building comes back up. Native owns the flag and every consequence of it; the host reads
   * it only to decide what to draw — the creative panel's controls, and whether a price is worth
   * showing at all.
   */
  creative: boolean;
  /**
   * Where an autonomous walk is headed, or `null` when the player is standing or steering. Native
   * owns it: it is saved with the run and hashed into the checksum, so a walk survives a reload the
   * way a held key never could.
   */
  walk_goal: AxialCoordinate | null;
  /**
   * The remaining route to {@link walk_goal}, nearest hex first, as the hexes native will actually
   * steer through — replanned natively whenever the world changes under it. The host draws this and
   * never searches for a route of its own, so the ribbon on screen cannot promise a way through that
   * the simulation would not take.
   */
  walk_path: AxialCoordinate[];
}

/**
 * The landing hub's standing demand. Native owns the stage, the bill, and the sentence in front of
 * it, so the mission header, the panel, and the drawing of the hub cannot disagree about which
 * project is current. `stage` doubles as how far the hub has grown.
 */
export interface ContractSnapshot {
  key: string;
  name: string;
  stage: number;
  stages: number;
  stage_key: string;
  stage_name: string;
  stage_brief: string;
  /** The current stage's bill. Empty once every stage is complete. */
  requirements: ContractRequirement[];
  complete: boolean;
}

/**
 * One posted request as the hub is holding it. Everything the row needs to draw travels with it —
 * the price above all, because a price a player can only discover by delivering is the defect the
 * board exists to remove.
 */
/**
 * Where a project stands. The snapshot publishes the whole catalogue, not just the board, so the
 * player can see what is left to do and choose what to post next.
 *
 * `locked` is "you cannot make this yet" rather than "this is hidden": the row is shown greyed so
 * a finite catalogue reads as a bill of work with an end, which is the point of it being finite.
 */
export type ProjectState = "locked" | "available" | "posted" | "complete";

export interface RequestSnapshot {
  key: string;
  name: string;
  brief: string;
  item_id: number;
  /** Already clamped natively to `required`, so a bar is two published numbers. */
  delivered: number;
  required: number;
  insight: number;
  state: ProjectState;
}

export interface ContractRequirement {
  item_id: number;
  /** Already clamped natively to `required`, so a bar is two published numbers. */
  delivered: number;
  required: number;
}

export interface GroundItemSnapshot extends AxialCoordinate {
  id: number;
  item_id: number;
  quantity: number;
  despawn_tick: number;
}

export interface FactorySnapshot {
  boundaries: Boundary[];
  ground: GroundCell[];
  /**
   * Cells whose standing water has left the generated equilibrium. Sparse, like {@link ground}:
   * the tile still carries the generated depth, and the host adds this departure exactly as native
   * does.
   */
  water: WaterCell[];
  /** Steps of earth dug and not yet placed. The only thing fill can be paid from. */
  spoil: number;
  scenario: string;
  scenario_name: string;
  world_version: number;
  seed: number;
  tick: number;
  checksum: number;
  /**
   * Ticks an item takes to cross one belt hex. Native publishes the number the simulation uses so
   * the host does not keep its own copy. A live snapshot always carries it; tests may omit it and
   * the renderer then uses the derived cadence of 27.
   */
  belt_transit_ticks?: number;
  delivered: number;
  delivered_by_item: Ingredient[];
  insight: number;
  victory: boolean;
  contract: ContractSnapshot;
  /** The hub's request board, in slot order. */
  requests: RequestSnapshot[];
  player: PlayerSnapshot;
  researched: number[];
  research_availability: ResearchAvailability[];
  skills: SkillsSnapshot;
  chunks: ChunkSnapshot[];
  terrain: TerrainSnapshot[];
  resources: ResourceSnapshot[];
  buildings: EntitySnapshot[];
  ground_items: GroundItemSnapshot[];
  events: string[];
}

/**
 * The per-entity buildings patch. `changed` carries inserted and modified entities and `removed`
 * the ids to drop, both in ascending native entity id order. `replace` marks a complete list.
 */
export interface BuildingsPatch {
  replace?: boolean;
  changed?: EntitySnapshot[];
  removed?: number[];
}

/**
 * The per-deposit resources patch. `changed` carries deposits whose quantity moved, addressed by
 * their tile key. Deposits are never removed, and the only path that adds one — world generation
 * — sets `replace` and sends the complete list, so an incremental patch always addresses deposits
 * the host already holds and never disturbs their order.
 */
export interface ResourcesPatch {
  replace?: boolean;
  changed?: ResourceSnapshot[];
}

/**
 * The per-cell terrain patch. Generation is the only thing that adds a tile and nothing ever
 * changes or removes one, so `changed` is exactly the chunks surveyed since the host last heard.
 * `replace` is set only by a full snapshot, where the host holds nothing to patch.
 */
export interface TerrainPatch {
  replace?: boolean;
  changed?: TerrainSnapshot[];
}

export interface FactorySnapshotDelta
  extends Partial<
    Omit<
      FactorySnapshot,
      "tick" | "checksum" | "buildings" | "resources" | "terrain"
    >
  > {
  base_revision: number;
  revision: number;
  tick: number;
  checksum: number;
  buildings?: BuildingsPatch;
  resources?: ResourcesPatch;
  terrain?: TerrainPatch;
}

export interface PlacementPreview {
  legal: boolean;
  reason: string;
}

/**
 * One cell of a drag preview, resolved natively. The host draws these and derives nothing: the
 * path, the per-cell heading, and the legality all come from the same native code that will run
 * the drag, so the preview cannot promise a line the drag will not build.
 */
export interface LinePreviewCell {
  q: number;
  r: number;
  orientation: number;
  legal: boolean;
  /** Native placement refusal for this resolved heading; preview RPC only, not snapshot wire. */
  reason?: string;
}

export type NativeInputCommand =
  | ({ type: "boundary_edit" } & BoundaryEdit)
  | { type: "undo_boundary" }
  | ({ type: "ground_edit" } & GroundEdit)
  | { type: "undo_ground" }
  /** Creative-only bounded water disturbance; native resolves and settles the affected region. */
  | {
      type: "water_edit";
      q: number;
      r: number;
      action: "flood" | "drain";
      quanta: number;
    }
  | { type: "move_intent"; x: number; y: number }
  /**
   * Point the player at a world position — the point under the cursor. The host sends the target
   * and never the heading: facing is native, checksummed state, so the unit vector is resolved in
   * Rust from the delta to the player rather than in host floating point. A frame that sends no
   * aim leaves facing to the walk direction, which is what the touch layout relies on.
   */
  | { type: "aim"; x: number; y: number }
  | { type: "gather" }
  /**
   * Harvest one named hex. The player right-clicked it, so the target is explicit and on screen —
   * which is what separates this from the facing-weighted targeting the gather rule refuses.
   * Reach is still native's, and still the same predicate an extractor on that hex would use.
   */
  | { type: "gather_at"; q: number; r: number }
  /**
   * Walk to a hex the player clicked a second time. The host sends a destination and never a route:
   * the search, the cost of crossing water, and the steering all belong to native, so what is drawn
   * is what will happen. Any `move_intent` afterwards — including the zero one a key release sends —
   * hands control back to the player.
   */
  | { type: "walk_to"; q: number; r: number }
  | { type: "deposit"; item_id?: number }
  | {
      type: "place";
      q: number;
      r: number;
      definition_id: number;
      orientation: number;
      recipe_id?: number;
    }
  /**
   * One drag of construction. The host sends only the endpoints it dragged between: the path, the
   * per-cell orientation, the legality, and the cost are resolved natively, so a drag can never
   * become a host-side loop over cells.
   */
  | {
      type: "place_line";
      q: number;
      r: number;
      to_q: number;
      to_r: number;
      definition_id: number;
      orientation: number;
      recipe_id?: number;
    }
  | { type: "erase"; q: number; r: number }
  | { type: "erase_line"; q: number; r: number; to_q: number; to_r: number }
  | { type: "rotate"; q: number; r: number; reverse?: boolean }
  | {
      type: "set_output_route";
      q: number;
      r: number;
      item_id: number;
      output_q: number;
      output_r: number;
      direction: number;
    }
  /**
   * Grow a building into the next tier of itself. Contents, heading, and connections survive
   * because native edits the entity in place rather than replacing it, and the price is netted
   * against the old construction cost so a ladder conserves items exactly.
   */
  | { type: "upgrade"; q: number; r: number }
  /**
   * Take stock out of a building by hand. `quantity` is a ceiling: native moves what the building
   * holds and what the player can still carry, and reports how much actually moved.
   *
   * Not containers only: a hand reaches into anything that holds stock the player can see — a
   * container, a composer, a burner, a boiler — so coal can come back out of a firebox that has
   * not spent it yet. What never comes back is stock the machine has already committed: reserved
   * inputs and banked fuel charge are not inventory, and native does not offer them.
   */
  | {
      type: "withdraw";
      q: number;
      r: number;
      item_id: number;
      quantity: number;
      stock?: StockKind;
    }
  /**
   * Put stock into a building by hand — the mirror of `withdraw`, on the same contract. `quantity`
   * is a ceiling: native moves what the player holds and what the building has room for, and
   * refuses an item the building has no use for separately from one it simply has no room for.
   */
  | {
      type: "store";
      q: number;
      r: number;
      item_id: number;
      quantity: number;
      stock?: StockKind;
    }
  | { type: "pickup_player_stack"; item_id: number; quantity: number }
  | {
      type: "pickup_building_stack";
      q: number;
      r: number;
      stock: Exclude<StockKind, "auto">;
      item_id: number;
      quantity: number;
    }
  | { type: "place_player_stack"; quantity: number }
  | {
      type: "place_building_stack";
      q: number;
      r: number;
      stock: Exclude<StockKind, "auto">;
      quantity: number;
    }
  | { type: "drop_player_stack"; q: number; r: number; quantity: number }
  /**
   * Give a machine a different job. Native enforces the same category rule placement does, and
   * refuses a machine that is mid-craft.
   */
  | { type: "set_recipe"; q: number; r: number; recipe_id: number }
  /**
   * Abandon a part-finished craft. Native returns the reserved ingredients to the machine's own
   * ingredient compartment in full, leaves fuel and output alone, and refuses a machine that is
   * not mid-craft. The confirmation belongs to the host; every unit of the accounting is native's.
   */
  | { type: "cancel_craft"; q: number; r: number }
  /**
   * Switch a working machine off, or back on. Carries the state it wants rather than a toggle, so
   * a doubled press or a replayed command lands on the same answer instead of flipping the
   * machine back. Native refuses the kinds that have no work to stop — a belt, a shelf, a wire.
   *
   * Off is total and free: no work, no draw, no fuel burned. What it keeps is everything the
   * machine was holding, so switching back on resumes rather than restarts.
   */
  | { type: "set_enabled"; q: number; r: number; enabled: boolean }
  | { type: "undo" }
  | { type: "research"; technology_id: number }
  | { type: "purchase_skill"; skill_id: number }
  /**
   * Pass on one posted project. The row goes behind everything the player has not seen yet and
   * another takes its slot, so a material they cannot find never holds the board hostage. What has
   * already been handed over is kept: progress belongs to the project, not to the slot, and under
   * finite demand a forfeit would destroy goods whose reward cannot be earned twice.
   */
  | { type: "skip_request"; slot: number }
  /**
   * Pull one project out of the catalogue and onto the board, displacing whichever posted row has
   * the least committed to it. Names the project rather than a slot: the player picked a project,
   * and which slot it lands in is native's business.
   */
  | { type: "post_request"; request_id: number }
  /**
   * Turn creative mode on, or back off. Carried rather than toggled, like set_enabled, so a
   * doubled press lands on the same answer. Turning it on researches the whole tree — permanently:
   * leaving creative restores the prices and the refunds but not the ignorance.
   */
  | { type: "set_creative"; enabled: boolean }
  /**
   * Put an item straight into the pack. Creative only. quantity is a ceiling, not a promise:
   * native grants what the carrying rule leaves room for and says so when that is nothing.
   */
  | { type: "grant"; item_id: number; quantity: number }
  /**
   * Destroy carried stock. Creative only. Omitting item_id empties the pack, and a quantity of
   * zero drops the whole of the named stack, so neither needs the host to read the pack back first.
   */
  | { type: "discard"; item_id?: number; quantity?: number }
  /**
   * Widen or narrow the pack. Creative only. The scenario's own number is the floor and native's
   * ceiling is the top; a size that would strand carried stock is refused rather than dropping it.
   */
  | { type: "set_carry_slots"; slots: number };

export interface NativeFactory {
  boundary_preview_json(edit: string): string;
  ground_preview_json(edit: string): string;
  tick(count: number): void;
  reset(): void;
  /**
   * `worldParamsJson` is either a preset key (`{"preset":"basin"}`) or a complete parameter set.
   * Omitting it generates whatever the scenario names, which is how every caller that does not
   * care about generation stays unaware that parameters exist.
   */
  new_game(
    scenarioKey: string,
    seedOverride?: number,
    worldParamsJson?: string,
    creative?: boolean,
  ): void;
  /** The parameters the current world was generated from. */
  world_params_json(): string;
  /**
   * A terrain raster for a parameter set that has not been generated yet: one byte per pixel,
   * holding the band's index in the `Terrain` declaration order. `hexesAcross` is the span the
   * width frames; a pixel is square in world units, so a taller preview shows more world.
   */
  world_preview_bytes(
    worldParamsJson: string,
    seed: number,
    width: number,
    height: number,
    hexesAcross: number,
  ): Uint8Array;
  /** Where the deposit lattice puts a site inside that same window. See {@link WorldPreview}. */
  world_preview_sites_json(
    worldParamsJson: string,
    seed: number,
    width: number,
    height: number,
    hexesAcross: number,
  ): string;
  apply_commands_json(commands: string): void;
  advance_json(commands: string, count: number, playerSteps: number): void;
  placement_preview_json(
    q: number,
    r: number,
    definitionId: number,
    orientation: number,
    recipeId?: number,
  ): string;
  line_preview_json(
    q: number,
    r: number,
    toQ: number,
    toR: number,
    definitionId: number,
    orientation: number,
    recipeId?: number,
  ): string;
  erase_line_preview_json(
    q: number,
    r: number,
    toQ: number,
    toR: number,
  ): string;
  snapshot_json(): string;
  /**
   * The shipped delta path: a compact binary buffer the worker transfers rather than clones. See
   * `src/core/snapshotWire.ts` for the format and why it replaced the JSON one.
   */
  snapshot_delta_bytes(): Uint8Array;
  /** The same delta as JSON. Retained as the encoder's oracle and for the capacity ladder. */
  snapshot_delta_json(): string;
  save_string(): string;
  load_string(save: string): void;
  checksum(): number;
  tick_count(): bigint;
  free(): void;
}

export type BoundaryFamily = "fence" | "wall";
export interface BoundaryDefinition {
  erosion_resistance?: number;
  id: number;
  key: string;
  name: string;
  description: string;
  family: BoundaryFamily;
  gate: boolean;
  unlock_technology_id?: number;
  construction_cost: Ingredient[];
}
/**
 * One straight boundary: a chord of hex `q, r` between two of its six corners.
 *
 * Chords `0`, `1` and `2` are the edges the hex shares with its east, south-east and south-west
 * neighbours — the only boundaries that existed before the vertex lattice, under the same numbers.
 * `6`–`11` are the short diagonals and `12`–`14` the long ones; those run through the hex's
 * interior, and they are what lets a wall hold a heading past a hex centre. Chords `3`–`5` are the
 * hex's other three shared edges and are always rewritten onto the neighbour that owns them, so
 * they never appear in a snapshot.
 */
export interface BoundarySegment {
  q: number;
  r: number;
  chord: number;
}
export interface Boundary extends BoundarySegment {
  definition_id: number;
  open: boolean;
  paid: Ingredient[];
}
export type BoundaryAction = "build" | "remove" | "open" | "close";
/** A straight run between two lattice vertices, or the four sides of the rectangle they define. */
export type BoundaryShape = "line" | "yard";
/** A lattice vertex, named by a hex and which of its six corners. */
export interface BoundaryAnchor {
  q: number;
  r: number;
  corner: number;
}
export interface BoundaryEdit extends BoundaryAnchor {
  to_q: number;
  to_r: number;
  to_corner: number;
  shape: BoundaryShape;
  definition_id: number;
  action: BoundaryAction;
}
export interface BoundaryPreview {
  segments: BoundarySegment[];
  changes: number;
  cost: Ingredient[];
  refund: Ingredient[];
  error: string | null;
}

/** One surface a hex can be finished with. `movement` is a percentage of untreated ground. */
export interface SurfaceDefinition {
  erosion_resistance?: number;
  unlock_technology_id?: number;
  base_surface_id?: number;
  id: number;
  key: string;
  name: string;
  description: string;
  movement: number;
  construction_cost: Ingredient[];
}

/**
 * One prepared hex. A cell exists only while it differs from untouched ground, so an absent cell
 * and a cell with surface 0 and elevation 0 mean the same thing and native never publishes the
 * latter.
 */
export interface GroundCell {
  q: number;
  r: number;
  /** 0 for untreated ground, otherwise a {@link SurfaceDefinition} id. */
  surface: number;
  /** Steps above or below the hex's natural grade, bounded by native's `MAX_GRADE_STEPS`. */
  elevation: number;
  /** Slow live erosion (negative) or deposition (positive), separate from paid earthwork. */
  erosion?: number;
  paid: Ingredient[];
}

/**
 * One cell whose standing water has left the generated equilibrium. A cell exists only while it
 * differs, so an absent cell and a cell with departure 0 mean the same thing and native never
 * publishes the latter.
 */
export interface WaterCell {
  q: number;
  r: number;
  /** Signed quanta away from the depth the generator publishes here. */
  departure: number;
}

export type GroundAction = "pave" | "clear" | "raise" | "lower" | "level";
/**
 * Six modes over two anchors. `rect` and `frame` are drawn on the world rather than on the axial
 * grid: two lattice vertices, and every hex the rectangle between them touches. They share their
 * anchors and their snapping with the walled yard, so a floor and the wall around it land on exactly
 * the same rectangle. `disc` and `ring` are dragged from a centre hex out to a rim hex, so the
 * radius is a distance the player counts on the map rather than a number typed into a field.
 *
 * `frame` and `ring` are the hex-adjacency perimeters of `rect` and `disc` — the cells of the fill
 * that touch something outside it. Deriving an outline from its own fill is what keeps it one hex
 * thick at every size, with no rounding rule that could disagree with the fill's.
 */
export type GroundShape = "cell" | "path" | "rect" | "frame" | "disc" | "ring";

/** Which grade a {@link GroundAction} of `level` evens onto. Ignored by every other verb. */
export type GroundReference = "first" | "lowest" | "highest";

export interface GroundEdit {
  q: number;
  r: number;
  to_q: number;
  to_r: number;
  /**
   * Which corner of `q, r` a `rect` or `frame` is anchored on. The other shapes name whole hexes,
   * and `disc`/`ring` read `to_q, to_r` as a rim hex rather than a second anchor.
   */
  corner: number;
  to_corner: number;
  shape: GroundShape;
  definition_id: number;
  action: GroundAction;
  /**
   * Acknowledge that this edit seals a deposit. Native refuses an edit that would cover one until
   * the host says the player has seen the warning, so covering can never be an accident.
   */
  cover: boolean;
  /**
   * How many steps one raise or lower moves the ground, clamped natively to `1..=MAX_GRADE_STEPS`.
   * A cell without room for the whole depth takes what it has room for rather than refusing.
   */
  steps: number;
  reference: GroundReference;
}

/** One cell of a ground preview, resolved by the same native transaction that will commit it. */
export interface GroundPreviewCell {
  q: number;
  r: number;
  surface: number;
  elevation: number;
  /** Steps this edit moves the cell, signed. Zero for a cell the edit only paves. */
  change: number;
  covers: boolean;
  /** True where the finished grade leaves a step no walk can climb. */
  retained: boolean;
  /**
   * Why this one cell cannot take the edit, if it cannot. One obstacle no longer refuses the whole
   * selection: the rest of the footprint is resolved and drawn around it.
   */
  blocked: string | null;
}

export interface GroundPreview {
  /**
   * Every selected cell, whatever the outcome. A refusal keeps its footprint — that picture is how
   * the player works out what to change.
   */
  cells: GroundPreviewCell[];
  changes: number;
  cost: Ingredient[];
  refund: Ingredient[];
  cut: number;
  fill: number;
  /** The spoil ledger after this edit. Fill is dug, never conjured. */
  spoil: number;
  covers: number;
  retaining: number;
  /** How many selected cells carry a `blocked` reason and will be passed over. */
  blocked: number;
  /** Set only when the edit as a whole cannot proceed — material, research, or the spoil ledger. */
  error: string | null;
}
