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
  | "boiler";
export type Terrain =
  | "deep_water"
  | "shallow_water"
  | "shore"
  | "lowland"
  | "hills"
  | "highland"
  | "cliff";
export type PlacementRule = "ground" | "resource" | "water" | "elevated";
export type PowerSource = "burner" | "wind" | "hydro" | "turbine";
/**
 * Which routing headings a definition may be built at. `edge` is the six hex edges and the
 * default; `vertical` is due north and due south, the two-row period a riser spans.
 */
export type OrientationAxis = "edge" | "vertical";

export interface Ingredient {
  item_id: number;
  quantity: number;
}

export interface ItemDefinition {
  id: number;
  key: string;
  name: string;
  color: string;
  icon: string;
  description: string;
  /** How many of this item fill one carried slot. The rule itself lives in Rust. */
  stack_size: number;
  /** Energy one unit releases when burned, for an item that is fuel. */
  fuel_value?: number;
  /** Ticks between one unit of regrowth and the next, for a resource that is flora. */
  regrowth_ticks?: number;
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
  /** Where this definition sits on its own upgrade ladder. Absent means the base tier. */
  tier?: number;
  /** The definition an `upgrade` turns this one into, if it has a next tier. */
  upgrades_to?: number;
  /** How many hexes this extractor reaches, counting its own. Absent means one. */
  extract_radius?: number;
  construction_cost: Ingredient[];
  unlock_technology_id?: number;
  placement_rule: PlacementRule;
  buildable: boolean;
  blocks_movement: boolean;
  footprint: AxialCoordinate[];
}

export interface Definitions {
  version: number;
  items: ItemDefinition[];
  recipes: RecipeDefinition[];
  buildings: BuildingDefinition[];
  requests: RequestDefinition[];
}

/**
 * One standing order the landing hub can post. The only thing in the game that pays insight, and it
 * says what it pays before anything is handed over.
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

export interface TechnologyDefinition {
  id: number;
  key: string;
  name: string;
  description: string;
  prerequisites: number[];
  cost: number;
  unlocks: number[];
}

export interface Technologies {
  version: number;
  technologies: TechnologyDefinition[];
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
 * One row of the generator's resource table. Rows are evaluated in declared order and the first
 * match wins, so the order is part of the world: clay is the leftover of the band wood takes
 * first, and it says so by being the later row.
 *
 * `-1` on a gate means the rule is not asking about that channel at all.
 */
export interface FieldRule {
  terrain: Terrain;
  item_id: number;
  moisture_min: number;
  richness_min: number;
  vein_min: number;
  base: number;
  spread: number;
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
  vein_cell: number;
  water_level: number;
  shore_level: number;
  hills_level: number;
  highland_level: number;
  cliff_step: number;
  deep_water_moisture: number;
  field_rules: FieldRule[];
}

/** A named parameter set. The preset is what a player picks; the parameters are behind it. */
export interface WorldPreset {
  key: string;
  name: string;
  description: string;
  params: WorldParams;
}

export type Cargo = Ingredient;

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
  inventory: Ingredient[];
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
  footprint: AxialCoordinate[];
}

export interface WorldPoint {
  x: number;
  y: number;
}

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

export interface TerrainSnapshot extends WorldPoint {
  q: number;
  r: number;
  radius: number;
  terrain: Terrain;
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
   * What a fresh action cooldown is worth. The wait is drawn as `action_cooldown` against this,
   * so the host never infers the maximum by watching the value fall.
   */
  action_cooldown_total: number;
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
export interface RequestSnapshot {
  key: string;
  name: string;
  brief: string;
  item_id: number;
  /** Already clamped natively to `required`, so a bar is two published numbers. */
  delivered: number;
  required: number;
  insight: number;
}

export interface ContractRequirement {
  item_id: number;
  /** Already clamped natively to `required`, so a bar is two published numbers. */
  delivered: number;
  required: number;
}

export interface FactorySnapshot {
  scenario: string;
  scenario_name: string;
  world_version: number;
  seed: number;
  tick: number;
  checksum: number;
  delivered: number;
  delivered_by_item: Ingredient[];
  insight: number;
  victory: boolean;
  contract: ContractSnapshot;
  /** The hub's request board, in slot order. */
  requests: RequestSnapshot[];
  player: PlayerSnapshot;
  researched: number[];
  chunks: ChunkSnapshot[];
  terrain: TerrainSnapshot[];
  resources: ResourceSnapshot[];
  buildings: EntitySnapshot[];
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

export interface FactorySnapshotDelta
  extends Partial<
    Omit<FactorySnapshot, "tick" | "checksum" | "buildings" | "resources">
  > {
  base_revision: number;
  revision: number;
  tick: number;
  checksum: number;
  buildings?: BuildingsPatch;
  resources?: ResourcesPatch;
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
}

export type NativeInputCommand =
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
  | { type: "deposit" }
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
  | { type: "rotate"; q: number; r: number }
  /**
   * Grow a building into the next tier of itself. Contents, heading, and connections survive
   * because native edits the entity in place rather than replacing it, and the price is netted
   * against the old construction cost so a ladder conserves items exactly.
   */
  | { type: "upgrade"; q: number; r: number }
  /**
   * Take stock out of a container by hand. `quantity` is a ceiling: native moves what the
   * container holds and what the player can still carry, and reports how much actually moved.
   */
  | {
      type: "withdraw";
      q: number;
      r: number;
      item_id: number;
      quantity: number;
    }
  /**
   * Put stock into a container by hand — the mirror of `withdraw`, on the same contract. `quantity`
   * is a ceiling: native moves what the player holds and what the container has room for.
   */
  | {
      type: "store";
      q: number;
      r: number;
      item_id: number;
      quantity: number;
    }
  /**
   * Give a machine a different job. Native enforces the same category rule placement does, and
   * refuses a machine that is mid-craft.
   */
  | { type: "set_recipe"; q: number; r: number; recipe_id: number }
  | { type: "undo" }
  | { type: "research"; technology_id: number }
  /**
   * Pass on one posted request. The row goes behind everything the player has not seen yet and
   * another takes its slot, so a material they cannot find never holds the board hostage. Whatever
   * was already delivered against it is forfeited, which is native's rule and not the host's.
   */
  | { type: "skip_request"; slot: number };

export interface NativeFactory {
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
  ): void;
  /** The parameters the current world was generated from. */
  world_params_json(): string;
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
