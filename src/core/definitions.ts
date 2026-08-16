import type { Definitions, Technologies } from "./types";

const KINDS = new Set([
  "extractor",
  "belt",
  "composer",
  "container",
  "consumer",
  "hub",
]);
const PLACEMENT_RULES = new Set(["ground", "resource"]);

export function validateDefinitions(
  value: unknown,
): asserts value is Definitions {
  if (!value || typeof value !== "object")
    throw new TypeError("definitions must be an object");
  const data = value as Partial<Definitions>;
  if (!positiveInteger(data.version))
    throw new TypeError("definitions require a positive version");
  if (
    !Array.isArray(data.items) ||
    !Array.isArray(data.recipes) ||
    !Array.isArray(data.buildings)
  ) {
    throw new TypeError(
      "definitions require item, recipe, and building arrays",
    );
  }
  uniqueIds(data.items, "item");
  uniqueIds(data.recipes, "recipe");
  uniqueIds(data.buildings, "building");
  const itemIds = new Set(data.items.map((item) => item.id));
  for (const item of data.items) {
    if (
      !item.key ||
      !item.name ||
      !item.color ||
      !item.icon ||
      !item.description ||
      !positiveInteger(item.insight_value)
    )
      throw new TypeError(`item ${item.id} is incomplete`);
  }
  for (const recipe of data.recipes) {
    if (
      !recipe.key ||
      !recipe.name ||
      !recipe.description ||
      !positiveInteger(recipe.duration) ||
      !recipe.inputs.length
    ) {
      throw new TypeError(`recipe ${recipe.id} is incomplete`);
    }
    for (const ingredient of [...recipe.inputs, recipe.output]) {
      if (
        !itemIds.has(ingredient.item_id) ||
        !positiveInteger(ingredient.quantity)
      ) {
        throw new TypeError(`recipe ${recipe.id} has an invalid ingredient`);
      }
    }
  }
  for (const building of data.buildings) {
    if (
      !building.key ||
      !building.name ||
      !building.description ||
      !building.icon ||
      !KINDS.has(building.kind) ||
      !PLACEMENT_RULES.has(building.placement_rule) ||
      !Array.isArray(building.construction_cost) ||
      !Array.isArray(building.footprint) ||
      !building.footprint.length ||
      typeof building.buildable !== "boolean" ||
      typeof building.blocks_movement !== "boolean"
    ) {
      throw new TypeError(`building ${building.id} is incomplete`);
    }
    const footprint = new Set(
      building.footprint.map(({ q, r }) => `${q},${r}`),
    );
    if (
      footprint.size !== building.footprint.length ||
      !footprint.has("0,0") ||
      building.footprint.some(
        ({ q, r }) => !Number.isInteger(q) || !Number.isInteger(r),
      )
    )
      throw new TypeError(`building ${building.id} has an invalid footprint`);
    for (const ingredient of building.construction_cost) {
      if (
        !itemIds.has(ingredient.item_id) ||
        !positiveInteger(ingredient.quantity)
      )
        throw new TypeError(`building ${building.id} has an invalid cost`);
    }
  }
}

export function validateTechnologies(
  value: unknown,
  definitions: Definitions,
): asserts value is Technologies {
  if (!value || typeof value !== "object")
    throw new TypeError("technologies must be an object");
  const data = value as Partial<Technologies>;
  if (!positiveInteger(data.version) || !Array.isArray(data.technologies))
    throw new TypeError("technologies require a version and array");
  uniqueIds(data.technologies, "technology");
  const ids = new Set(data.technologies.map(({ id }) => id));
  const buildingIds = new Set(definitions.buildings.map(({ id }) => id));
  for (const technology of data.technologies) {
    if (
      !technology.key ||
      !technology.name ||
      !technology.description ||
      !positiveInteger(technology.cost) ||
      technology.prerequisites.some((id) => !ids.has(id)) ||
      technology.unlocks.some((id) => !buildingIds.has(id))
    )
      throw new TypeError(`technology ${technology.id} is invalid`);
  }
  const completed = new Set<number>();
  while (completed.size < data.technologies.length) {
    const before = completed.size;
    for (const technology of data.technologies)
      if (technology.prerequisites.every((id) => completed.has(id)))
        completed.add(technology.id);
    if (completed.size === before)
      throw new TypeError("technology graph must be acyclic");
  }
  for (const building of definitions.buildings)
    if (
      building.unlock_technology_id !== undefined &&
      !ids.has(building.unlock_technology_id)
    )
      throw new TypeError(`building ${building.id} has an invalid unlock`);
}

function uniqueIds(values: Array<{ id: number }>, label: string): void {
  const ids = new Set<number>();
  for (const value of values) {
    if (!positiveInteger(value.id) || ids.has(value.id))
      throw new TypeError(`${label} IDs must be positive and unique`);
    ids.add(value.id);
  }
}

function positiveInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) > 0;
}
