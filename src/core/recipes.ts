import type { Definitions, Ingredient, RecipeDefinition } from "./types";

/** Catalogue presentation only. Native owns production and unlock decisions. */
export function recipeOutputs(recipe: RecipeDefinition): Ingredient[] {
  return [recipe.output, ...(recipe.co_products ?? [])];
}

export function recipeYield(recipe: RecipeDefinition, item: number): number {
  return (
    recipeOutputs(recipe).find((output) => output.item_id === item)?.quantity ??
    0
  );
}

export function recipeShare(recipe: RecipeDefinition, item: number): number {
  const index = recipeOutputs(recipe).findIndex(
    (output) => output.item_id === item,
  );
  return index < 0 ? 0 : (recipe.cost_allocation?.[index] ?? 100);
}

/** Explicit preference, not catalogue order or a theoretical cheapest route. */
export function productionRoutes(
  definitions: Definitions,
  item: number,
): RecipeDefinition[] {
  const producers = definitions.recipes.filter(
    (recipe) => recipeYield(recipe, item) > 0,
  );
  const order = definitions.items.find(
    (value) => value.id === item,
  )?.production_routes;
  return order !== undefined
    ? order
        .map((id) => producers.find((recipe) => recipe.id === id)!)
        .filter(Boolean)
    : producers.sort((a, b) => a.id - b.id);
}

export function productionRecipe(
  definitions: Definitions,
  item: number,
  available?: (recipe: RecipeDefinition) => boolean,
): RecipeDefinition | undefined {
  const routes = productionRoutes(definitions, item);
  return (available && routes.find(available)) || routes[0];
}
