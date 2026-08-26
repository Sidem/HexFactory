import defaultDefinitions from "../data/definitions.json";
import defaultScenarios from "../data/scenarios.json";
import defaultTechnologies from "../data/technologies.json";

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
import { runDiagnostics } from "./diagnostics";
import type {
  AdminTab,
  DiffChange,
  EntityEditTarget,
  HistoryEntry,
  ValidationIssue,
} from "./types";

const DRAFT_STORAGE_KEY = "hexfactory_admin_draft_v1";

export class AdminStore {
  public definitions: Definitions;
  public technologies: Technologies;
  public scenarios: Scenarios;

  public baselineDefinitions: Definitions;
  public baselineTechnologies: Technologies;
  public baselineScenarios: Scenarios;

  public activeTab: AdminTab = "items";
  public searchQuery = "";
  public selectedFilter = "all";
  public editingTarget: EntityEditTarget = null;
  public diagnostics: ValidationIssue[] = [];

  private history: HistoryEntry[] = [];
  private historyIndex = -1;
  private maxHistory = 40;
  private listeners = new Set<() => void>();

  constructor() {
    this.baselineDefinitions = structuredClone(
      defaultDefinitions as unknown as Definitions,
    );
    this.baselineTechnologies = structuredClone(
      defaultTechnologies as unknown as Technologies,
    );
    this.baselineScenarios = structuredClone(
      defaultScenarios as unknown as Scenarios,
    );

    // Initialize with baseline or restore from localStorage draft if present
    this.definitions = structuredClone(this.baselineDefinitions);
    this.technologies = structuredClone(this.baselineTechnologies);
    this.scenarios = structuredClone(this.baselineScenarios);

    this.restoreFromStorage();
    this.pushHistory("Initial state");
    this.recomputeDiagnostics();
  }

  public subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify(): void {
    this.recomputeDiagnostics();
    this.persistToStorage();
    for (const listener of this.listeners) {
      listener();
    }
  }

  public setTab(tab: AdminTab): void {
    this.activeTab = tab;
    this.searchQuery = "";
    this.selectedFilter = "all";
    this.editingTarget = null;
    this.notify();
  }

  public setSearchQuery(query: string): void {
    this.searchQuery = query;
    this.notify();
  }

  public setSelectedFilter(filter: string): void {
    this.selectedFilter = filter;
    this.notify();
  }

  public setEditingTarget(target: EntityEditTarget): void {
    this.editingTarget = target;
    this.notify();
  }

  public recomputeDiagnostics(): void {
    this.diagnostics = runDiagnostics(this.definitions, this.technologies);
  }

  /* -------------------------------------------------------------------------- */
  /*                              HISTORY / UNDO / REDO                         */
  /* -------------------------------------------------------------------------- */

  private pushHistory(label: string): void {
    // Truncate any forward history if we're branching
    if (this.historyIndex < this.history.length - 1) {
      this.history = this.history.slice(0, this.historyIndex + 1);
    }

    this.history.push({
      label,
      timestamp: Date.now(),
      definitions: structuredClone(this.definitions),
      technologies: structuredClone(this.technologies),
      scenarios: structuredClone(this.scenarios),
    });

    if (this.history.length > this.maxHistory) {
      this.history.shift();
    }
    this.historyIndex = this.history.length - 1;
  }

  public canUndo(): boolean {
    return this.historyIndex > 0;
  }

  public canRedo(): boolean {
    return this.historyIndex < this.history.length - 1;
  }

  public undo(): boolean {
    if (!this.canUndo()) return false;
    this.historyIndex -= 1;
    const entry = this.history[this.historyIndex];
    if (entry) {
      this.definitions = structuredClone(entry.definitions);
      this.technologies = structuredClone(entry.technologies);
      this.scenarios = structuredClone(entry.scenarios);
      this.notify();
      return true;
    }
    return false;
  }

  public redo(): boolean {
    if (!this.canRedo()) return false;
    this.historyIndex += 1;
    const entry = this.history[this.historyIndex];
    if (entry) {
      this.definitions = structuredClone(entry.definitions);
      this.technologies = structuredClone(entry.technologies);
      this.scenarios = structuredClone(entry.scenarios);
      this.notify();
      return true;
    }
    return false;
  }

  /* -------------------------------------------------------------------------- */
  /*                              ITEM MUTATIONS                                */
  /* -------------------------------------------------------------------------- */

  public saveItem(item: ItemDefinition): void {
    const idx = this.definitions.items.findIndex((i) => i.id === item.id);
    if (idx >= 0) {
      this.definitions.items[idx] = structuredClone(item);
      this.pushHistory(`Update item: ${item.name}`);
    } else {
      this.definitions.items.push(structuredClone(item));
      this.pushHistory(`Create item: ${item.name}`);
    }
    this.editingTarget = null;
    this.notify();
  }

  public deleteItem(id: number): void {
    const item = this.definitions.items.find((i) => i.id === id);
    this.definitions.items = this.definitions.items.filter((i) => i.id !== id);
    this.pushHistory(`Delete item: ${item?.name ?? id}`);
    if (
      this.editingTarget?.type === "item" &&
      this.editingTarget.data.id === id
    ) {
      this.editingTarget = null;
    }
    this.notify();
  }

  public duplicateItem(id: number): ItemDefinition | null {
    const source = this.definitions.items.find((i) => i.id === id);
    if (!source) return null;
    const nextId = this.getNextItemId();
    const cloned: ItemDefinition = {
      ...structuredClone(source),
      id: nextId,
      key: `${source.key}-copy`,
      name: `${source.name} (Copy)`,
    };
    this.definitions.items.push(cloned);
    this.pushHistory(`Duplicate item: ${cloned.name}`);
    this.notify();
    return cloned;
  }

  public getNextItemId(): number {
    const max = this.definitions.items.reduce((m, i) => Math.max(m, i.id), 0);
    return max + 1;
  }

  /* -------------------------------------------------------------------------- */
  /*                             RECIPE MUTATIONS                               */
  /* -------------------------------------------------------------------------- */

  public saveRecipe(recipe: RecipeDefinition): void {
    const idx = this.definitions.recipes.findIndex((r) => r.id === recipe.id);
    if (idx >= 0) {
      this.definitions.recipes[idx] = structuredClone(recipe);
      this.pushHistory(`Update recipe: ${recipe.name}`);
    } else {
      this.definitions.recipes.push(structuredClone(recipe));
      this.pushHistory(`Create recipe: ${recipe.name}`);
    }
    this.editingTarget = null;
    this.notify();
  }

  public deleteRecipe(id: number): void {
    const recipe = this.definitions.recipes.find((r) => r.id === id);
    this.definitions.recipes = this.definitions.recipes.filter(
      (r) => r.id !== id,
    );
    this.pushHistory(`Delete recipe: ${recipe?.name ?? id}`);
    if (
      this.editingTarget?.type === "recipe" &&
      this.editingTarget.data.id === id
    ) {
      this.editingTarget = null;
    }
    this.notify();
  }

  public duplicateRecipe(id: number): RecipeDefinition | null {
    const source = this.definitions.recipes.find((r) => r.id === id);
    if (!source) return null;
    const nextId = this.getNextRecipeId();
    const cloned: RecipeDefinition = {
      ...structuredClone(source),
      id: nextId,
      key: `${source.key}-copy`,
      name: `${source.name} (Copy)`,
    };
    this.definitions.recipes.push(cloned);
    this.pushHistory(`Duplicate recipe: ${cloned.name}`);
    this.notify();
    return cloned;
  }

  public getNextRecipeId(): number {
    const max = this.definitions.recipes.reduce((m, r) => Math.max(m, r.id), 0);
    return max + 1;
  }

  /* -------------------------------------------------------------------------- */
  /*                            BUILDING MUTATIONS                              */
  /* -------------------------------------------------------------------------- */

  public saveBuilding(building: BuildingDefinition): void {
    const idx = this.definitions.buildings.findIndex(
      (b) => b.id === building.id,
    );
    if (idx >= 0) {
      this.definitions.buildings[idx] = structuredClone(building);
      this.pushHistory(`Update building: ${building.name}`);
    } else {
      this.definitions.buildings.push(structuredClone(building));
      this.pushHistory(`Create building: ${building.name}`);
    }
    this.editingTarget = null;
    this.notify();
  }

  public deleteBuilding(id: number): void {
    const building = this.definitions.buildings.find((b) => b.id === id);
    this.definitions.buildings = this.definitions.buildings.filter(
      (b) => b.id !== id,
    );
    // Remove references to this building in upgrades_to or tech unlocks
    for (const b of this.definitions.buildings) {
      if (b.upgrades_to === id) delete b.upgrades_to;
    }
    for (const t of this.technologies.technologies) {
      t.unlocks = t.unlocks.filter((uid) => uid !== id);
    }
    this.pushHistory(`Delete building: ${building?.name ?? id}`);
    if (
      this.editingTarget?.type === "building" &&
      this.editingTarget.data.id === id
    ) {
      this.editingTarget = null;
    }
    this.notify();
  }

  public duplicateBuilding(id: number): BuildingDefinition | null {
    const source = this.definitions.buildings.find((b) => b.id === id);
    if (!source) return null;
    const nextId = this.getNextBuildingId();
    const cloned: BuildingDefinition = {
      ...structuredClone(source),
      id: nextId,
      key: `${source.key}-copy`,
      name: `${source.name} (Copy)`,
    };
    delete cloned.upgrades_to;
    this.definitions.buildings.push(cloned);
    this.pushHistory(`Duplicate building: ${cloned.name}`);
    this.notify();
    return cloned;
  }

  public getNextBuildingId(): number {
    const max = this.definitions.buildings.reduce(
      (m, b) => Math.max(m, b.id),
      0,
    );
    return max + 1;
  }

  /* -------------------------------------------------------------------------- */
  /*                            REQUEST MUTATIONS                               */
  /* -------------------------------------------------------------------------- */

  public saveRequest(request: RequestDefinition): void {
    const idx = this.definitions.requests.findIndex((r) => r.id === request.id);
    if (idx >= 0) {
      this.definitions.requests[idx] = structuredClone(request);
      this.pushHistory(`Update request: ${request.name}`);
    } else {
      this.definitions.requests.push(structuredClone(request));
      this.pushHistory(`Create request: ${request.name}`);
    }
    this.editingTarget = null;
    this.notify();
  }

  public deleteRequest(id: number): void {
    const request = this.definitions.requests.find((r) => r.id === id);
    this.definitions.requests = this.definitions.requests.filter(
      (r) => r.id !== id,
    );
    this.pushHistory(`Delete request: ${request?.name ?? id}`);
    if (
      this.editingTarget?.type === "request" &&
      this.editingTarget.data.id === id
    ) {
      this.editingTarget = null;
    }
    this.notify();
  }

  public duplicateRequest(id: number): RequestDefinition | null {
    const source = this.definitions.requests.find((r) => r.id === id);
    if (!source) return null;
    const nextId = this.getNextRequestId();
    const cloned: RequestDefinition = {
      ...structuredClone(source),
      id: nextId,
      key: `${source.key}-copy`,
      name: `${source.name} (Copy)`,
    };
    this.definitions.requests.push(cloned);
    this.pushHistory(`Duplicate request: ${cloned.name}`);
    this.notify();
    return cloned;
  }

  public getNextRequestId(): number {
    const max = this.definitions.requests.reduce(
      (m, r) => Math.max(m, r.id),
      0,
    );
    return max + 1;
  }

  /* -------------------------------------------------------------------------- */
  /*                          TECHNOLOGY MUTATIONS                              */
  /* -------------------------------------------------------------------------- */

  public saveTechnology(technology: TechnologyDefinition): void {
    const idx = this.technologies.technologies.findIndex(
      (t) => t.id === technology.id,
    );
    if (idx >= 0) {
      this.technologies.technologies[idx] = structuredClone(technology);
      this.pushHistory(`Update technology: ${technology.name}`);
    } else {
      this.technologies.technologies.push(structuredClone(technology));
      this.pushHistory(`Create technology: ${technology.name}`);
    }
    this.editingTarget = null;
    this.notify();
  }

  public deleteTechnology(id: number): void {
    const tech = this.technologies.technologies.find((t) => t.id === id);
    this.technologies.technologies = this.technologies.technologies.filter(
      (t) => t.id !== id,
    );
    // Remove from prerequisites
    for (const t of this.technologies.technologies) {
      t.prerequisites = t.prerequisites.filter((pid) => pid !== id);
    }
    this.pushHistory(`Delete technology: ${tech?.name ?? id}`);
    if (
      this.editingTarget?.type === "technology" &&
      this.editingTarget.data.id === id
    ) {
      this.editingTarget = null;
    }
    this.notify();
  }

  public getNextTechnologyId(): number {
    const max = this.technologies.technologies.reduce(
      (m, t) => Math.max(m, t.id),
      0,
    );
    return max + 1;
  }

  /* -------------------------------------------------------------------------- */
  /*                              IMPORT & RESET                                */
  /* -------------------------------------------------------------------------- */

  public importDefinitions(newDefs: Definitions): void {
    this.definitions = structuredClone(newDefs);
    this.pushHistory("Import definitions.json");
    this.notify();
  }

  public importTechnologies(newTechs: Technologies): void {
    this.technologies = structuredClone(newTechs);
    this.pushHistory("Import technologies.json");
    this.notify();
  }

  public revertToBaseline(): void {
    this.definitions = structuredClone(this.baselineDefinitions);
    this.technologies = structuredClone(this.baselineTechnologies);
    this.scenarios = structuredClone(this.baselineScenarios);
    this.history = [];
    this.historyIndex = -1;
    this.pushHistory("Revert to default data");
    this.notify();
  }

  /* -------------------------------------------------------------------------- */
  /*                              DIFF & DIRTY                                  */
  /* -------------------------------------------------------------------------- */

  public getDirtyCount(): number {
    return this.getDiffSummary().length;
  }

  public getDiffSummary(): DiffChange[] {
    const changes: DiffChange[] = [];

    // Diff items
    const baseItems = new Map(
      this.baselineDefinitions.items.map((i) => [i.id, i]),
    );
    const currItems = new Map(this.definitions.items.map((i) => [i.id, i]));

    for (const [id, item] of currItems) {
      const base = baseItems.get(id);
      if (!base) {
        changes.push({
          entityType: "item",
          changeType: "added",
          id,
          name: item.name,
          details: [`Added new item "${item.name}" (${item.key})`],
        });
      } else {
        const diffs = this.diffObjects(base, item);
        if (diffs.length > 0) {
          changes.push({
            entityType: "item",
            changeType: "modified",
            id,
            name: item.name,
            details: diffs,
          });
        }
      }
    }
    for (const [id, base] of baseItems) {
      if (!currItems.has(id)) {
        changes.push({
          entityType: "item",
          changeType: "deleted",
          id,
          name: base.name,
          details: [`Deleted item "${base.name}" (#${id})`],
        });
      }
    }

    // Diff recipes
    const baseRecipes = new Map(
      this.baselineDefinitions.recipes.map((r) => [r.id, r]),
    );
    const currRecipes = new Map(this.definitions.recipes.map((r) => [r.id, r]));

    for (const [id, recipe] of currRecipes) {
      const base = baseRecipes.get(id);
      if (!base) {
        changes.push({
          entityType: "recipe",
          changeType: "added",
          id,
          name: recipe.name,
          details: [`Added new recipe "${recipe.name}" (${recipe.key})`],
        });
      } else {
        const diffs = this.diffObjects(base, recipe);
        if (diffs.length > 0) {
          changes.push({
            entityType: "recipe",
            changeType: "modified",
            id,
            name: recipe.name,
            details: diffs,
          });
        }
      }
    }
    for (const [id, base] of baseRecipes) {
      if (!currRecipes.has(id)) {
        changes.push({
          entityType: "recipe",
          changeType: "deleted",
          id,
          name: base.name,
          details: [`Deleted recipe "${base.name}" (#${id})`],
        });
      }
    }

    // Diff buildings
    const baseBuildings = new Map(
      this.baselineDefinitions.buildings.map((b) => [b.id, b]),
    );
    const currBuildings = new Map(
      this.definitions.buildings.map((b) => [b.id, b]),
    );

    for (const [id, building] of currBuildings) {
      const base = baseBuildings.get(id);
      if (!base) {
        changes.push({
          entityType: "building",
          changeType: "added",
          id,
          name: building.name,
          details: [`Added new building "${building.name}" (${building.key})`],
        });
      } else {
        const diffs = this.diffObjects(base, building);
        if (diffs.length > 0) {
          changes.push({
            entityType: "building",
            changeType: "modified",
            id,
            name: building.name,
            details: diffs,
          });
        }
      }
    }
    for (const [id, base] of baseBuildings) {
      if (!currBuildings.has(id)) {
        changes.push({
          entityType: "building",
          changeType: "deleted",
          id,
          name: base.name,
          details: [`Deleted building "${base.name}" (#${id})`],
        });
      }
    }

    // Diff requests
    const baseRequests = new Map(
      this.baselineDefinitions.requests.map((r) => [r.id, r]),
    );
    const currRequests = new Map(
      this.definitions.requests.map((r) => [r.id, r]),
    );

    for (const [id, req] of currRequests) {
      const base = baseRequests.get(id);
      if (!base) {
        changes.push({
          entityType: "request",
          changeType: "added",
          id,
          name: req.name,
          details: [`Added new request "${req.name}" (${req.key})`],
        });
      } else {
        const diffs = this.diffObjects(base, req);
        if (diffs.length > 0) {
          changes.push({
            entityType: "request",
            changeType: "modified",
            id,
            name: req.name,
            details: diffs,
          });
        }
      }
    }
    for (const [id, base] of baseRequests) {
      if (!currRequests.has(id)) {
        changes.push({
          entityType: "request",
          changeType: "deleted",
          id,
          name: base.name,
          details: [`Deleted request "${base.name}" (#${id})`],
        });
      }
    }

    // Diff technologies
    const baseTechs = new Map(
      this.baselineTechnologies.technologies.map((t) => [t.id, t]),
    );
    const currTechs = new Map(
      this.technologies.technologies.map((t) => [t.id, t]),
    );

    for (const [id, tech] of currTechs) {
      const base = baseTechs.get(id);
      if (!base) {
        changes.push({
          entityType: "technology",
          changeType: "added",
          id,
          name: tech.name,
          details: [`Added new technology "${tech.name}" (${tech.key})`],
        });
      } else {
        const diffs = this.diffObjects(base, tech);
        if (diffs.length > 0) {
          changes.push({
            entityType: "technology",
            changeType: "modified",
            id,
            name: tech.name,
            details: diffs,
          });
        }
      }
    }
    for (const [id, base] of baseTechs) {
      if (!currTechs.has(id)) {
        changes.push({
          entityType: "technology",
          changeType: "deleted",
          id,
          name: base.name,
          details: [`Deleted technology "${base.name}" (#${id})`],
        });
      }
    }

    return changes;
  }

  private diffObjects(base: unknown, current: unknown): string[] {
    const diffs: string[] = [];
    if (
      !base ||
      !current ||
      typeof base !== "object" ||
      typeof current !== "object"
    ) {
      if (JSON.stringify(base) !== JSON.stringify(current)) {
        diffs.push(
          `Changed from ${JSON.stringify(base)} to ${JSON.stringify(current)}`,
        );
      }
      return diffs;
    }

    const b = base as Record<string, unknown>;
    const c = current as Record<string, unknown>;
    const allKeys = new Set([...Object.keys(b), ...Object.keys(c)]);

    for (const key of allKeys) {
      const valB = b[key];
      const valC = c[key];
      if (JSON.stringify(valB) !== JSON.stringify(valC)) {
        if (valB === undefined) {
          diffs.push(`Set ${key} = ${JSON.stringify(valC)}`);
        } else if (valC === undefined) {
          diffs.push(`Removed ${key}`);
        } else {
          diffs.push(
            `Modified ${key}: ${JSON.stringify(valB)} ➔ ${JSON.stringify(valC)}`,
          );
        }
      }
    }
    return diffs;
  }

  /* -------------------------------------------------------------------------- */
  /*                         LOCAL STORAGE PERSISTENCE                          */
  /* -------------------------------------------------------------------------- */

  private persistToStorage(): void {
    try {
      const payload = JSON.stringify({
        definitions: this.definitions,
        technologies: this.technologies,
        scenarios: this.scenarios,
        timestamp: Date.now(),
      });
      localStorage.setItem(DRAFT_STORAGE_KEY, payload);
    } catch {
      // Ignore storage errors (quota/disabled)
    }
  }

  private restoreFromStorage(): void {
    try {
      const raw = localStorage.getItem(DRAFT_STORAGE_KEY);
      if (!raw) return;
      const data = JSON.parse(raw);
      if (data && data.definitions && data.technologies) {
        this.definitions = data.definitions;
        this.technologies = data.technologies;
        if (data.scenarios) this.scenarios = data.scenarios;
      }
    } catch {
      // Fall back to baseline
    }
  }
}
