import type {
  BuildingDefinition,
  Definitions,
  ItemDefinition,
  RecipeDefinition,
  RequestDefinition,
  Scenarios,
  Technologies,
  TechnologyDefinition,
} from "../core/types";

export type AdminTab =
  | "items"
  | "recipes"
  | "buildings"
  | "requests"
  | "technologies"
  | "chains"
  | "diagnostics"
  | "raw-json";

export type IssueSeverity = "error" | "warning" | "info";

export interface ValidationIssue {
  id: string;
  severity: IssueSeverity;
  category: string;
  entity: "item" | "recipe" | "building" | "request" | "technology" | "general";
  entityId?: number;
  message: string;
  field?: string;
}

export interface HistoryEntry {
  label: string;
  timestamp: number;
  definitions: Definitions;
  technologies: Technologies;
  scenarios: Scenarios;
}

export type EntityEditTarget =
  | { type: "item"; data: ItemDefinition; isNew: boolean }
  | { type: "recipe"; data: RecipeDefinition; isNew: boolean }
  | { type: "building"; data: BuildingDefinition; isNew: boolean }
  | { type: "request"; data: RequestDefinition; isNew: boolean }
  | { type: "technology"; data: TechnologyDefinition; isNew: boolean }
  | null;

export interface DiffChange {
  entityType: "item" | "recipe" | "building" | "request" | "technology";
  changeType: "added" | "modified" | "deleted";
  id: number;
  name: string;
  details: string[];
}
