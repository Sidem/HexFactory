import type { Definitions } from "./types";

const KINDS = new Set([
  "extractor",
  "belt",
  "composer",
  "container",
  "consumer",
]);

export function validateDefinitions(
  value: unknown,
): asserts value is Definitions {
  if (!value || typeof value !== "object")
    throw new TypeError("definitions must be an object");
  const data = value as Partial<Definitions>;
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
    if (!item.key || !item.name || !item.color)
      throw new TypeError(`item ${item.id} is incomplete`);
  }
  for (const recipe of data.recipes) {
    if (
      !recipe.key ||
      !recipe.name ||
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
    if (!building.key || !building.name || !KINDS.has(building.kind)) {
      throw new TypeError(`building ${building.id} is incomplete`);
    }
  }
}

function uniqueIds(values: Array<{ id: number }>, label: string): void {
  const ids = new Set<number>();
  for (const value of values) {
    if (!positiveInteger(value.id) || ids.has(value.id)) {
      throw new TypeError(`${label} IDs must be positive and unique`);
    }
    ids.add(value.id);
  }
}

function positiveInteger(value: number): boolean {
  return Number.isSafeInteger(value) && value > 0;
}
