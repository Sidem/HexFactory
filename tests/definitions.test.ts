import { describe, expect, it } from "vitest";

import {
  validateDefinitions,
  validateTechnologies,
} from "../src/core/definitions";
import type { Definitions } from "../src/core/types";
import definitions from "../src/data/definitions.json";
import technologies from "../src/data/technologies.json";
import { isItemIconKey } from "../src/rendering/icons";

describe("data-defined content", () => {
  const typedDefinitions = definitions as Definitions;

  it("accepts dynamic items, recipes, construction metadata, and the technology DAG", () => {
    expect(() => validateDefinitions(definitions)).not.toThrow();
    expect(() =>
      validateTechnologies(technologies, typedDefinitions),
    ).not.toThrow();
    expect(definitions.recipes[0]?.inputs).toEqual([
      { item_id: 1, quantity: 2 },
    ]);
    expect(
      definitions.buildings.find(({ key }) => key === "extractor")
        ?.placement_rule,
    ).toBe("resource");
    expect(
      definitions.buildings.find(({ key }) => key === "composer")?.footprint,
    ).toHaveLength(2);
    // Every item draws with a glyph the icon set actually has, rather than falling back to ore.
    for (const item of definitions.items)
      expect(isItemIconKey(item.icon), `${item.key} icon`).toBe(true);
    expect(technologies.technologies.map(({ key }) => key)).toEqual([
      "field-logistics",
      "automated-extraction",
      "composition",
      "storage-planning",
      "material-processing",
      "mechanical-shaping",
      "hydrology",
      "on-site-power",
      "sited-generation",
      "steam-works",
    ]);
  });

  it("gives every material a source and every recipe a machine that runs it", () => {
    // The eight raw resources the world produces. Water is the only one that is not a field.
    for (const key of [
      "ore",
      "copper-ore",
      "coal",
      "stone",
      "sand",
      "clay",
      "wood",
      "water",
    ])
      expect(
        definitions.items.some((item) => item.key === key),
        key,
      ).toBe(true);
    const pump = definitions.buildings.find(({ key }) => key === "pump");
    expect(pump?.placement_rule).toBe("water");
    expect(pump?.output_item_id).toBe(
      definitions.items.find(({ key }) => key === "water")?.id,
    );

    // Fuel is a property of the item, so no recipe may name one as the thing it burns.
    const fuels = definitions.items.filter(({ fuel_value }) => fuel_value);
    expect(fuels.map(({ key }) => key)).toEqual(["coal", "wood", "charcoal"]);
    // Charcoal is reachable without coal, or a player landing away from a coal field could not
    // bootstrap smelting at all.
    const charcoal = definitions.recipes.find(({ key }) => key === "charcoal");
    expect(charcoal?.fuel ?? 0).toBe(0);

    // Every recipe belongs to a category some machine can be assigned, and every machine that runs
    // recipes claims exactly one category.
    const categories = new Set(
      definitions.buildings
        .filter(({ kind }) => kind === "composer")
        .map(({ recipe_category }) => recipe_category),
    );
    expect([...categories].sort()).toEqual([
      "assembly",
      "crushing",
      "cutting",
      "firing",
      "smelting",
    ]);
    for (const recipe of definitions.recipes)
      expect(categories.has(recipe.category), recipe.key).toBe(true);
  });

  it("rejects a recipe no machine can run and a machine that claims the wrong category", () => {
    const orphan = structuredClone(definitions);
    orphan.recipes[0]!.category = "alchemy";
    expect(() => validateDefinitions(orphan)).toThrow(/no building runs/);

    const miscategorised = structuredClone(definitions);
    (
      miscategorised.buildings.find(({ key }) => key === "container") as {
        recipe_category?: string;
      }
    ).recipe_category = "assembly";
    expect(() => validateDefinitions(miscategorised)).toThrow(
      /does not match its kind/,
    );
  });

  it("rejects duplicate IDs, invalid costs, unknown unlocks, and cycles", () => {
    const duplicate = structuredClone(definitions);
    duplicate.items[1]!.id = duplicate.items[0]!.id;
    expect(() => validateDefinitions(duplicate)).toThrow(/positive and unique/);

    const badCost = structuredClone(definitions);
    badCost.buildings[0]!.construction_cost[0]!.item_id = 999;
    expect(() => validateDefinitions(badCost)).toThrow(/invalid cost/);

    // Every item needs a stack size, because carrying capacity is measured in stacks.
    const unstackable = structuredClone(definitions);
    unstackable.items[0]!.stack_size = 0;
    expect(() => validateDefinitions(unstackable)).toThrow(/incomplete/);

    const badUnlock = structuredClone(technologies);
    badUnlock.technologies[0]!.unlocks = [999];
    expect(() => validateTechnologies(badUnlock, typedDefinitions)).toThrow(
      /invalid/,
    );

    const cycle = structuredClone(technologies);
    cycle.technologies[0]!.prerequisites = [3];
    expect(() => validateTechnologies(cycle, typedDefinitions)).toThrow(
      /acyclic/,
    );
  });
});
