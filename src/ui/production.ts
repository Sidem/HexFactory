import type { Definitions, EntitySnapshot } from "../core/types";
import { recipeOutputs } from "../core/recipes";

/** A small persistent explanation beside the actual machine inventory, never a tick model. */
export function productionNote(
  building: EntitySnapshot | undefined,
  definitions: Definitions,
): string {
  const recipe = definitions.recipes.find(
    (recipe) => recipe.id === building?.recipe_id,
  );
  if (!building || !recipe) return "";
  const outputs = recipeOutputs(recipe);
  const name = (id: number): string =>
    definitions.items.find((item) => item.id === id)?.name ?? `Item ${id}`;
  const definition = definitions.buildings.find(
    (definition) => definition.id === building.definition_id,
  );
  const ingredients = building.input_inventory ?? [];
  const missing = recipe.inputs.filter(
    (input) =>
      (ingredients.find((entry) => entry.item_id === input.item_id)?.quantity ??
        0) < input.quantity,
  );
  if (
    building.status === "waiting for inputs" &&
    missing.length &&
    ingredients.reduce((sum, entry) => sum + entry.quantity, 0) >=
      (definition?.capacity ?? Infinity)
  )
    return `Ingredient buffer full, but this recipe still needs ${missing.map((input) => name(input.item_id)).join(" and ")}. Take some of the other ingredients back into your pack to make room. The capacity is shared by all ingredients.`;
  if (outputs.length < 2) return "";
  const batch = outputs
    .map((output) => `${output.quantity} ${name(output.item_id)}`)
    .join(" + ");
  const stored = (building.output_inventory ?? [])
    .filter((entry) => entry.quantity > 0)
    .map((entry) => `${entry.quantity} ${name(entry.item_id)}`)
    .join(", ");
  return building.status === "output blocked"
    ? `Output buffer blocked${stored ? ` — holding ${stored}` : ""}. Free space for the whole batch (${batch}). Take output below or connect storage for both products; no inputs are consumed while blocked.`
    : `Each batch makes ${batch}. Both share one outlet and one buffer. Keep both moving: use storage, or a splitter beside compatible consumers. Refined fuel runs burners and boilers; bitumen feeds asphalt.`;
}
