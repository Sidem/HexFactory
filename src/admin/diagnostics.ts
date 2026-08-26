import { validateDefinitions, validateTechnologies } from "../core/definitions";
import type {
  BuildingDefinition,
  Definitions,
  ItemDefinition,
  RecipeDefinition,
  Technologies,
  TechnologyDefinition,
} from "../core/types";
import { isItemIconKey } from "../rendering/icons";
import type { ValidationIssue } from "./types";

export function runDiagnostics(
  definitions: Definitions,
  technologies: Technologies,
): ValidationIssue[] {
  const issues: ValidationIssue[] = [];

  // 1. Run core validators
  try {
    validateDefinitions(definitions);
  } catch (err) {
    issues.push({
      id: "core-definitions-error",
      severity: "error",
      category: "Schema & Invariants",
      entity: "general",
      message: err instanceof Error ? err.message : String(err),
    });
  }

  try {
    validateTechnologies(technologies, definitions);
  } catch (err) {
    issues.push({
      id: "core-technologies-error",
      severity: "error",
      category: "Technology Tree",
      entity: "technology",
      message: err instanceof Error ? err.message : String(err),
    });
  }

  // Build ID index sets
  const itemMap = new Map<number, ItemDefinition>();
  const itemKeys = new Map<string, number>();
  for (const item of definitions.items) {
    if (itemMap.has(item.id)) {
      issues.push({
        id: `duplicate-item-id-${item.id}`,
        severity: "error",
        category: "Identity",
        entity: "item",
        entityId: item.id,
        message: `Duplicate item ID ${item.id}`,
      });
    }
    itemMap.set(item.id, item);

    if (itemKeys.has(item.key)) {
      issues.push({
        id: `duplicate-item-key-${item.key}`,
        severity: "error",
        category: "Identity",
        entity: "item",
        entityId: item.id,
        message: `Duplicate item key "${item.key}" (shared with item #${itemKeys.get(item.key)})`,
      });
    }
    itemKeys.set(item.key, item.id);

    if (!isItemIconKey(item.icon)) {
      issues.push({
        id: `invalid-item-icon-${item.id}`,
        severity: "warning",
        category: "Rendering",
        entity: "item",
        entityId: item.id,
        message: `Item "${item.name}" uses unknown icon key "${item.icon}"`,
        field: "icon",
      });
    }

    if (
      !item.color.startsWith("#") ||
      (item.color.length !== 7 && item.color.length !== 4)
    ) {
      issues.push({
        id: `invalid-item-color-${item.id}`,
        severity: "warning",
        category: "Rendering",
        entity: "item",
        entityId: item.id,
        message: `Item "${item.name}" color "${item.color}" is not a valid hex color (#rrggbb)`,
        field: "color",
      });
    }
  }

  const recipeMap = new Map<number, RecipeDefinition>();
  const recipeKeys = new Map<string, number>();
  for (const recipe of definitions.recipes) {
    if (recipeMap.has(recipe.id)) {
      issues.push({
        id: `duplicate-recipe-id-${recipe.id}`,
        severity: "error",
        category: "Identity",
        entity: "recipe",
        entityId: recipe.id,
        message: `Duplicate recipe ID ${recipe.id}`,
      });
    }
    recipeMap.set(recipe.id, recipe);

    if (recipeKeys.has(recipe.key)) {
      issues.push({
        id: `duplicate-recipe-key-${recipe.key}`,
        severity: "error",
        category: "Identity",
        entity: "recipe",
        entityId: recipe.id,
        message: `Duplicate recipe key "${recipe.key}"`,
      });
    }
    recipeKeys.set(recipe.key, recipe.id);

    // Validate inputs
    for (const input of recipe.inputs) {
      if (!itemMap.has(input.item_id)) {
        issues.push({
          id: `recipe-${recipe.id}-bad-input-${input.item_id}`,
          severity: "error",
          category: "Referential Integrity",
          entity: "recipe",
          entityId: recipe.id,
          message: `Recipe "${recipe.name}" references non-existent input item #${input.item_id}`,
          field: "inputs",
        });
      }
    }

    // Validate output
    if (!itemMap.has(recipe.output.item_id)) {
      issues.push({
        id: `recipe-${recipe.id}-bad-output-${recipe.output.item_id}`,
        severity: "error",
        category: "Referential Integrity",
        entity: "recipe",
        entityId: recipe.id,
        message: `Recipe "${recipe.name}" references non-existent output item #${recipe.output.item_id}`,
        field: "output",
      });
    }
  }

  const buildingMap = new Map<number, BuildingDefinition>();
  const buildingKeys = new Map<string, number>();
  const buildingCategories = new Set<string>();

  for (const building of definitions.buildings) {
    if (buildingMap.has(building.id)) {
      issues.push({
        id: `duplicate-building-id-${building.id}`,
        severity: "error",
        category: "Identity",
        entity: "building",
        entityId: building.id,
        message: `Duplicate building ID ${building.id}`,
      });
    }
    buildingMap.set(building.id, building);

    if (buildingKeys.has(building.key)) {
      issues.push({
        id: `duplicate-building-key-${building.key}`,
        severity: "error",
        category: "Identity",
        entity: "building",
        entityId: building.id,
        message: `Duplicate building key "${building.key}"`,
      });
    }
    buildingKeys.set(building.key, building.id);

    if (building.recipe_category) {
      buildingCategories.add(building.recipe_category);
    }

    // Validate costs
    for (const cost of building.construction_cost) {
      if (!itemMap.has(cost.item_id)) {
        issues.push({
          id: `building-${building.id}-bad-cost-${cost.item_id}`,
          severity: "error",
          category: "Referential Integrity",
          entity: "building",
          entityId: building.id,
          message: `Building "${building.name}" references non-existent cost item #${cost.item_id}`,
          field: "construction_cost",
        });
      }
    }

    if (building.corner_construction_cost) {
      for (const cost of building.corner_construction_cost) {
        if (!itemMap.has(cost.item_id)) {
          issues.push({
            id: `building-${building.id}-bad-corner-cost-${cost.item_id}`,
            severity: "error",
            category: "Referential Integrity",
            entity: "building",
            entityId: building.id,
            message: `Building "${building.name}" references non-existent corner cost item #${cost.item_id}`,
            field: "corner_construction_cost",
          });
        }
      }
    }

    if (
      building.kind === "pump" &&
      building.output_item_id &&
      !itemMap.has(building.output_item_id)
    ) {
      issues.push({
        id: `building-${building.id}-bad-pump-output-${building.output_item_id}`,
        severity: "error",
        category: "Referential Integrity",
        entity: "building",
        entityId: building.id,
        message: `Pump "${building.name}" references non-existent output item #${building.output_item_id}`,
        field: "output_item_id",
      });
    }
  }

  // Check recipes matching building categories
  for (const recipe of definitions.recipes) {
    if (!buildingCategories.has(recipe.category)) {
      issues.push({
        id: `orphan-recipe-${recipe.id}`,
        severity: "error",
        category: "Economy Balance",
        entity: "recipe",
        entityId: recipe.id,
        message: `Recipe "${recipe.name}" has category "${recipe.category}", but no composer building is assigned to that category`,
        field: "category",
      });
    }
  }

  // Check tech references
  const techMap = new Map<number, TechnologyDefinition>();
  for (const tech of technologies.technologies) {
    techMap.set(tech.id, tech);
  }

  for (const building of definitions.buildings) {
    if (
      building.unlock_technology_id !== undefined &&
      !techMap.has(building.unlock_technology_id)
    ) {
      issues.push({
        id: `building-${building.id}-bad-unlock-tech-${building.unlock_technology_id}`,
        severity: "error",
        category: "Referential Integrity",
        entity: "building",
        entityId: building.id,
        message: `Building "${building.name}" has unknown unlock technology ID #${building.unlock_technology_id}`,
        field: "unlock_technology_id",
      });
    }
    if (
      building.corner_technology_id !== undefined &&
      !techMap.has(building.corner_technology_id)
    ) {
      issues.push({
        id: `building-${building.id}-bad-corner-tech-${building.corner_technology_id}`,
        severity: "error",
        category: "Referential Integrity",
        entity: "building",
        entityId: building.id,
        message: `Building "${building.name}" has unknown corner technology ID #${building.corner_technology_id}`,
        field: "corner_technology_id",
      });
    }
  }

  // Check requests
  const requestKeys = new Map<string, number>();
  for (const request of definitions.requests) {
    if (requestKeys.has(request.key)) {
      issues.push({
        id: `duplicate-request-key-${request.key}`,
        severity: "error",
        category: "Identity",
        entity: "request",
        entityId: request.id,
        message: `Duplicate request key "${request.key}"`,
      });
    }
    requestKeys.set(request.key, request.id);

    if (!itemMap.has(request.item_id)) {
      issues.push({
        id: `request-${request.id}-bad-item-${request.item_id}`,
        severity: "error",
        category: "Referential Integrity",
        entity: "request",
        entityId: request.id,
        message: `Hub request "${request.name}" references non-existent item #${request.item_id}`,
        field: "item_id",
      });
    }
  }

  // Supply chain sanity: items that have no production source and no gather ability
  const producedItemIds = new Set<number>();
  for (const recipe of definitions.recipes) {
    producedItemIds.add(recipe.output.item_id);
  }
  for (const building of definitions.buildings) {
    if (building.kind === "pump" && building.output_item_id) {
      producedItemIds.add(building.output_item_id);
    }
  }

  for (const item of definitions.items) {
    const isGatherable =
      item.hand_gather_steps !== undefined || item.extract_steps !== undefined;
    const isProduced = producedItemIds.has(item.id);
    if (!isGatherable && !isProduced) {
      issues.push({
        id: `unobtainable-item-${item.id}`,
        severity: "warning",
        category: "Supply Chain",
        entity: "item",
        entityId: item.id,
        message: `Item "${item.name}" (#${item.id}) cannot be harvested or crafted by any recipe/pump`,
      });
    }
  }

  return issues;
}
