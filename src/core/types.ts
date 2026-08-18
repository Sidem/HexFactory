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
  insight_value: number;
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
  /** Electricity this machine draws every tick while it is on a network. */
  power_draw?: number;
  /** Electricity this generator offers every tick it is live. */
  power_output?: number;
  /** Hex reach from this machine to a pole. */
  power_reach?: number;
  /** Hex reach from this pole to another pole. */
  pole_reach?: number;
  /** How a generator makes electricity. */
  power_source?: PowerSource;
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
  objective: { item_id: number; delivered: number; required: number };
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
   * Give a machine a different job. Native enforces the same category rule placement does, and
   * refuses a machine that is mid-craft.
   */
  | { type: "set_recipe"; q: number; r: number; recipe_id: number }
  | { type: "undo" }
  | { type: "research"; technology_id: number };

export interface NativeFactory {
  tick(count: number): void;
  reset(): void;
  new_game(scenarioKey: string, seedOverride?: number): void;
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
