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
  const notes: string[] = [];
  const reactants = recipe.inputs.filter(({ item_id }) =>
    definitions.items.some(
      (item) => item.id === item_id && (item.fuel_value ?? 0) > 0,
    ),
  );
  if (recipe.fuel && reactants.length)
    notes.push(
      `${recipe.description} Recipe ingredients (${reactants.map((item) => `${item.quantity} ${name(item.item_id)}`).join(" + ")}) are consumed in the product. Fuel / heat is a separate supply.`,
    );
  const leftovers = (building.input_inventory ?? []).filter(
    (entry) =>
      entry.quantity > 0 &&
      !recipe.inputs.some((input) => input.item_id === entry.item_id),
  );
  if (leftovers.length)
    notes.push(
      `Left from the previous recipe: ${leftovers.map((entry) => `${entry.quantity} ${name(entry.item_id)}`).join(", ")}. Take these items from the Recipe ingredients compartment to reuse them.`,
    );
  if (outputs.length < 2) return notes.join(" ");
  const batch = outputs
    .map((output) => `${output.quantity} ${name(output.item_id)}`)
    .join(" + ");
  const stored = (building.output_inventory ?? [])
    .filter((entry) => entry.quantity > 0)
    .map((entry) => `${entry.quantity} ${name(entry.item_id)}`)
    .join(", ");
  notes.push(
    building.status === "output blocked"
      ? `Output buffer blocked${stored ? ` — holding ${stored}` : ""}. Free space for the whole batch (${batch}). Take output below or connect every product port; no inputs are consumed while blocked.`
      : `Each batch makes ${batch} into one shared buffer. In Product outputs, choose each product and click the exact outside footprint port it should use. Refined fuel runs burners and boilers; bitumen feeds asphalt.`,
  );
  return notes.join(" ");
}
