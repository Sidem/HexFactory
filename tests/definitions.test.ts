import { describe, expect, it } from "vitest";

import {
  validateDefinitions,
  supportsRecipe,
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
    const crystal = typedDefinitions.items.find(({ key }) => key === "crystal");
    expect(crystal?.hand_gather_steps).toBeUndefined();
    const wood = typedDefinitions.items.find(({ key }) => key === "wood");
    expect(wood?.hand_gather_steps).toBe(15);
    const ore = typedDefinitions.items.find(({ key }) => key === "ore");
    expect(ore?.hand_gather_steps).toBe(45);
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
      "corner-transport",
      "machine-tiers",
      "transmission",
      "grid-engineering",
      "shallow-crossings",
      "belt-junctions",
      "grade-separation",
      "expanded-pack",
      "surveyed-construction",
    ]);
    expect(
      technologies.technologies.find(({ key }) => key === "expanded-pack"),
    ).toMatchObject({ carry_slots_bonus: 4 });
    expect(
      technologies.technologies.find(
        ({ key }) => key === "surveyed-construction",
      ),
    ).toMatchObject({ build_range_bonus: 3 });
  });

  it("keeps an upgrade ladder a taller version of the same machine", () => {
    const buildings = typedDefinitions.buildings;
    const ladders = buildings.filter(
      ({ upgrades_to }) => upgrades_to !== undefined,
    );
    expect(ladders.length).toBeGreaterThan(0);
    for (const building of ladders) {
      const next = buildings.find(({ id }) => id === building.upgrades_to);
      expect(next, `${building.key} names a real next tier`).toBeDefined();
      expect(next?.kind).toBe(building.kind);
      expect(next?.tier ?? 0).toBeGreaterThan(building.tier ?? 0);
      expect(next?.footprint).toEqual(building.footprint);
    }
    // Reach is the flagship upgrade, so it has to actually grow.
    const extractor = buildings.find(({ key }) => key === "extractor");
    const deep = buildings.find(({ key }) => key === "extractor-ii");
    expect(extractor?.extract_radius ?? 1).toBeLessThan(
      deep?.extract_radius ?? 0,
    );

    const broken = structuredClone(typedDefinitions);
    const target = broken.buildings.find(({ key }) => key === "extractor-ii");
    if (target) target.tier = 0;
    expect(() => validateDefinitions(broken)).toThrow(/not a higher tier/);
  });

  it("lets only a single-cell definition claim the two-row period", () => {
    const belt = typedDefinitions.buildings.find(({ key }) => key === "belt");
    expect(belt?.orientation_axis).toBe("any");
    expect(belt?.footprint).toHaveLength(1);
    // And the reach is priced for what it buys — twice a belt, for twice a belt's span — and
    // gated behind its own research, so one definition covering both periods still hands the
    // player the second one only when they have earned it.
    const total = (cost: { quantity: number }[] | undefined) =>
      (cost ?? []).reduce((sum, { quantity }) => sum + quantity, 0);
    expect(total(belt?.corner_construction_cost)).toBe(
      total(belt?.construction_cost) * 2,
    );
    expect(belt?.corner_technology_id).toBeDefined();
    expect(belt?.corner_technology_id).not.toBe(belt?.unlock_technology_id);

    const wide = structuredClone(typedDefinitions);
    wide.buildings
      .find(({ key }) => key === "belt")
      ?.footprint.push({ q: 1, r: 0 });
    expect(() => validateDefinitions(wide)).toThrow(/two-row period/);

    // An any-axis definition that gates none of its headings is refused too.
    const ungated = structuredClone(typedDefinitions);
    const target = ungated.buildings.find(({ key }) => key === "belt");
    if (target) delete target.corner_technology_id;
    expect(() => validateDefinitions(ungated)).toThrow(/gates none of them/);
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
    expect(pump?.extract_radius).toBe(1);
    expect(pump?.output_item_id).toBe(
      definitions.items.find(({ key }) => key === "water")?.id,
    );
    expect(
      definitions.buildings.find(({ key }) => key === "extractor")
        ?.extract_radius,
    ).toBe(1);
    expect(
      definitions.buildings.find(({ key }) => key === "bridge"),
    ).toMatchObject({
      kind: "bridge",
      placement_rule: "shallows",
    });
    // The three junction definitions, and what each of them claims about compiled edges.
    expect(
      definitions.buildings.find(({ key }) => key === "splitter"),
    ).toMatchObject({ kind: "belt", splits: true });
    expect(
      definitions.buildings.find(({ key }) => key === "merger"),
    ).toMatchObject({ kind: "belt", merges: true });
    expect(
      definitions.buildings.find(({ key }) => key === "underpass"),
    ).toMatchObject({ kind: "belt", underpass_span: 4 });
    for (const pole of definitions.buildings.filter(
      ({ kind }) => kind === "pole",
    )) {
      expect(pole.supply_radius).toBeGreaterThan(0);
      expect(pole.pole_reach).toBeGreaterThan(pole.supply_radius ?? 0);
    }

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
      "manual-workshop",
      "primitive-smelting",
      "smelting",
    ]);
    for (const recipe of definitions.recipes)
      expect(categories.has(recipe.category), recipe.key).toBe(true);
  });

  it("rejects a recipe no machine can run and a machine that claims the wrong category", () => {
    const orphan = structuredClone(definitions);
    orphan.recipes.find(({ key }) => key === "circuit")!.category = "alchemy";
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

  it("validates explicit primitive capabilities and bounded attended work", () => {
    const workshop = typedDefinitions.buildings.find(
      ({ key }) => key === "manual-workshop",
    )!;
    const recipes = typedDefinitions.recipes.filter((recipe) =>
      supportsRecipe(workshop, recipe),
    );
    expect(recipes.map(({ key }) => key)).toEqual([
      "component",
      "timber",
      "gear",
      "frame",
      "transport-kit",
      "iron-wire",
    ]);
    for (const patch of [
      { recipe_ids: [] },
      { recipe_ids: [8, 8] },
      { recipe_ids: [9999] },
      { duration_multiplier: 0 },
      { duration_multiplier: 61 },
      { power_draw: 1 },
      { recipe_ids: [2] },
    ]) {
      const invalid = structuredClone(typedDefinitions);
      Object.assign(
        invalid.buildings.find(({ id }) => id === workshop.id)!,
        patch,
      );
      expect(() => validateDefinitions(invalid)).toThrow(/invalid/);
    }
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

    const excessiveBonus = structuredClone(technologies);
    excessiveBonus.technologies[0]!.build_range_bonus = 999;
    expect(() =>
      validateTechnologies(excessiveBonus, typedDefinitions),
    ).toThrow(/invalid/);
  });
});
