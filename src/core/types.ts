import type { AxialCoordinate } from "@hexlife/embed/hex";

export type BuildingKind =
  | "extractor"
  | "belt"
  | "composer"
  | "container"
  | "consumer";

export interface ItemDefinition {
  id: number;
  key: string;
  name: string;
  color: string;
}

export interface Ingredient {
  item_id: number;
  quantity: number;
}

export interface RecipeDefinition {
  id: number;
  key: string;
  name: string;
  inputs: Ingredient[];
  output: Ingredient;
  duration: number;
}

export interface BuildingDefinition {
  id: number;
  key: string;
  name: string;
  kind: BuildingKind;
  cadence?: number;
  capacity?: number;
}

export interface Definitions {
  items: ItemDefinition[];
  recipes: RecipeDefinition[];
  buildings: BuildingDefinition[];
}

export type Cargo = Ingredient;

export interface EntitySnapshot extends AxialCoordinate {
  id: number;
  definition_id: number;
  kind: BuildingKind;
  orientation: number;
  recipe_id?: number;
  cargo?: Cargo;
  inventory: Ingredient[];
  progress: number;
  progress_total: number;
  status: string;
  next_id?: number;
}

export interface ResourceSnapshot extends AxialCoordinate {
  item_id: number;
}

export interface FactorySnapshot {
  tick: number;
  checksum: number;
  delivered: number;
  chunks: Array<{ chunk_q: number; chunk_r: number; entity_count: number }>;
  resources: ResourceSnapshot[];
  buildings: EntitySnapshot[];
}

export interface NativeFactory {
  tick(count: number): void;
  reset(): void;
  checksum(): number;
  tick_count(): bigint;
  snapshot_json(): string;
  place(
    q: number,
    r: number,
    definitionId: number,
    orientation: number,
    recipeId?: number,
  ): boolean;
  erase(q: number, r: number): boolean;
  rotate(q: number, r: number): boolean;
  free(): void;
}
