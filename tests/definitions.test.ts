import { describe, expect, it } from "vitest";

import {
  validateDefinitions,
  validateTechnologies,
} from "../src/core/definitions";
import type { Definitions } from "../src/core/types";
import definitions from "../src/data/definitions.json";
import technologies from "../src/data/technologies.json";

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
    expect(technologies.technologies.map(({ key }) => key)).toEqual([
      "field-logistics",
      "automated-extraction",
      "composition",
      "storage-planning",
    ]);
  });

  it("rejects duplicate IDs, invalid costs, unknown unlocks, and cycles", () => {
    const duplicate = structuredClone(definitions);
    duplicate.items[1]!.id = duplicate.items[0]!.id;
    expect(() => validateDefinitions(duplicate)).toThrow(/positive and unique/);

    const badCost = structuredClone(definitions);
    badCost.buildings[0]!.construction_cost[0]!.item_id = 999;
    expect(() => validateDefinitions(badCost)).toThrow(/invalid cost/);

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
