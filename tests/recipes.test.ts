import { describe, expect, it } from "vitest";
import json from "../src/data/definitions.json";
import type { Definitions } from "../src/core/types";
import { validateDefinitions } from "../src/core/definitions";
import {
  productionRecipe,
  recipeOutputs,
  recipeShare,
  recipeYield,
} from "../src/core/recipes";

describe("joint production contracts", () => {
  function joint(): Definitions {
    const definitions = structuredClone(json) as Definitions;
    const recipe = definitions.recipes[0]!;
    recipe.co_products = [{ item_id: 3, quantity: 2 }];
    recipe.cost_allocation = [70, 30];
    // The test product is an otherwise raw item with no path back to this recipe.
    return definitions;
  }
  it("names every product and allocates exactly one batch cost", () => {
    const definitions = joint();
    const recipe = definitions.recipes[0]!;
    expect(recipeOutputs(recipe)).toHaveLength(2);
    expect(recipeYield(recipe, 3)).toBe(2);
    expect(
      recipeShare(recipe, recipe.output.item_id) + recipeShare(recipe, 3),
    ).toBe(100);
    expect(productionRecipe(definitions, 3)).toBe(recipe);
  });
  it("refuses missing shares, duplicate outputs and recursive routes", () => {
    const definitions = joint();
    definitions.recipes[0]!.cost_allocation = [100, 30];
    expect(() => validateDefinitions(definitions)).toThrow(/cost shares/);
    definitions.recipes[0]!.cost_allocation = [70, 30];
    definitions.recipes[0]!.co_products = [definitions.recipes[0]!.output];
    expect(() => validateDefinitions(definitions)).toThrow(/duplicate/);
    definitions.recipes[0]!.co_products = [definitions.recipes[0]!.inputs[0]!];
    expect(() => validateDefinitions(definitions)).toThrow(
      /production route|cycle/,
    );

    // A batch no compatible station could hold is refused too, and so is a specialized source
    // naming an item the catalogue does not have.
    const oversized = structuredClone(json) as Definitions;
    oversized.buildings.find(
      (building) => building.key === "refinery",
    )!.capacity = 3;
    expect(() => validateDefinitions(oversized)).toThrow(/batch exceeds/);

    const unknownSource = structuredClone(json) as Definitions;
    unknownSource.items.find(
      (item) => item.key === "crude-oil",
    )!.extraction_building_id = undefined;
    unknownSource.buildings.find(
      (building) => building.key === "oil-well",
    )!.output_item_id = 99999;
    expect(() => validateDefinitions(unknownSource)).toThrow(
      /known output item/,
    );
  });
  it("requires ordered alternatives and selects an available route independently of catalogue order", () => {
    const definitions = structuredClone(json) as Definitions;
    const first = definitions.recipes[0]!;
    const alternative = { ...first, id: 1001, key: "alternate" };
    definitions.recipes.push(alternative);
    expect(() => validateDefinitions(definitions)).toThrow(/production route/);
    definitions.items.find(
      (item) => item.id === first.output.item_id,
    )!.production_routes = [1001, first.id];
    expect(() => validateDefinitions(definitions)).not.toThrow();
    expect(productionRecipe(definitions, first.output.item_id)?.id).toBe(1001);
    expect(
      productionRecipe(
        definitions,
        first.output.item_id,
        (recipe) => recipe.id === first.id,
      )?.id,
    ).toBe(first.id);
    definitions.recipes.reverse();
    expect(productionRecipe(definitions, first.output.item_id)?.id).toBe(1001);
  });
});
