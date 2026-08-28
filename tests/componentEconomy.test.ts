import { describe, expect, it } from "vitest";
import definitions from "../src/data/definitions.json";
import scenarios from "../src/data/scenarios.json";
import balance from "../fixtures/balance.json";

describe("mechanical component and founding bill", () => {
  it("trades three ore-only components for one plate-mounted gear without tripling ore", () => {
    const recipe = definitions.recipes.find(({ key }) => key === "component")!;
    expect(recipe.inputs).toEqual([
      { item_id: 11, quantity: 1 },
      { item_id: 19, quantity: 1 },
    ]);
    expect(recipe.output).toEqual({ item_id: 2, quantity: 1 });
    expect(recipe.duration).toBe(8); // Existing paid jobs retain their output and duration.
    const stage = scenarios.scenarios.find(({ key }) => key === "new-game")!
      .contract.stages[0]!;
    expect(stage.requirements).toEqual([{ item_id: 2, quantity: 1 }]);
    const opening = balance.contracts.find(
      ({ stage }) => stage === "components",
    )!.opening;
    expect(opening.buildings).toEqual(["manual-workshop", "primitive-furnace"]);
    expect(opening.technologies).toEqual([]);
    expect(opening.insight).toBe(0);
    expect(opening.gathers).toEqual([
      { item: "ore", quantity: 6 },
      { item: "stone", quantity: 8 },
      { item: "clay", quantity: 4 },
      { item: "wood", quantity: 4 },
    ]);
    expect(opening.fuel_energy).toBe(240);
    expect(opening.fuel_items).toBe(2);
    expect(opening.gather_total).toBe(24);
    expect(opening.machine_ticks).toBe(60);
    expect(opening.player_work_ticks).toBe(64);
  });

  it("preserves the processed request return per expanded gather", () => {
    const request = balance.requests.find(
      ({ request }) => request === "component-batch",
    )!;
    expect(request.quantity).toBe(8); // No partially delivered legacy row becomes invalid.
    expect(request.gather_total).toBe(60);
    expect(request.insight).toBe(98);
    expect(request.insight_per_gather_milli).toBeGreaterThanOrEqual(1625);
    expect(request.repeat_insight).toBe(request.insight);
  });
});
