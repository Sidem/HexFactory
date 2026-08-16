import type { AxialCoordinate } from "@hexlife/embed/hex";

export type BuildingKind =
  | "extractor"
  | "belt"
  | "composer"
  | "container"
  | "consumer"
  | "hub";
export type Terrain = "ground" | "water" | "rock";
export type PlacementRule = "ground" | "resource";

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
}

export interface RecipeDefinition {
  id: number;
  key: string;
  name: string;
  description: string;
  inputs: Ingredient[];
  output: Ingredient;
  duration: number;
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
  construction_cost: Ingredient[];
  unlock_technology_id?: number;
  placement_rule: PlacementRule;
  buildable: boolean;
  blocks_movement: boolean;
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
  recipe_id?: number;
  scenario_owned: boolean;
  cargo?: Cargo;
  inventory: Ingredient[];
  progress: number;
  progress_total: number;
  status: string;
  next_id?: number;
}

export interface ResourceSnapshot extends AxialCoordinate {
  item_id: number;
  quantity: number;
  initial_quantity: number;
}

export interface TerrainSnapshot extends AxialCoordinate {
  terrain: Terrain;
}

export interface PlayerSnapshot extends AxialCoordinate {
  facing: number;
  inventory: Record<string, number>;
  action_cooldown: number;
  build_range: number;
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
  chunks: Array<{ chunk_q: number; chunk_r: number; entity_count: number }>;
  terrain: TerrainSnapshot[];
  resources: ResourceSnapshot[];
  buildings: EntitySnapshot[];
  events: string[];
}

export interface PlacementPreview {
  legal: boolean;
  reason: string;
}

export type NativeInputCommand =
  | { type: "move"; direction: number }
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
  | { type: "erase"; q: number; r: number }
  | { type: "rotate"; q: number; r: number }
  | { type: "research"; technology_id: number };

export interface NativeFactory {
  tick(count: number): void;
  reset(): void;
  new_game(scenarioKey: string, seedOverride?: number): void;
  apply_commands_json(commands: string): void;
  placement_preview_json(
    q: number,
    r: number,
    definitionId: number,
    orientation: number,
    recipeId?: number,
  ): string;
  snapshot_json(): string;
  save_string(): string;
  load_string(save: string): void;
  checksum(): number;
  tick_count(): bigint;
  free(): void;
}
