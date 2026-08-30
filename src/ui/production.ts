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
  // There used to be a note here for a machine wedged by its own ingredient buffer: one full
  // ingredient left no room for the others, and the only way out was to take stock back into the
  // pack. Ingredient capacity is per ingredient now, so a stocked slot cannot crowd out an empty
  // one and the note has nothing left to describe.
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
