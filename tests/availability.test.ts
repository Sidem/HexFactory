import { describe, expect, it } from "vitest";

import {
  buildingAvailability,
  costLines,
  heldQuantity,
  technologyAvailability,
} from "../src/core/availability";
import definitions from "../src/data/definitions.json";
import technologies from "../src/data/technologies.json";
import { orderTechnologies, technologyContext } from "../src/ui/research";
import type { BuildingDefinition, FactorySnapshot } from "../src/core/types";

/**
 * The shortfall is plain arithmetic over a snapshot, so it is tested without a DOM. That is the
 * point of it living in `src/core`: the reason a build card says "no" is a fact about the game
 * state, not a fact about a card.
 */
const carrying = (inventory: Record<string, number>): FactorySnapshot =>
  ({
    researched: [1, 2, 3, 4, 5, 6, 7, 8],
    insight: 0,
    player: { inventory },
  }) as unknown as FactorySnapshot;

const building = (key: string): BuildingDefinition =>
  definitions.buildings.find(
    (candidate) => candidate.key === key,
  ) as BuildingDefinition;

describe("research presentation", () => {
  it("uses the native answer even when catalog arithmetic would allow a purchase", () => {
    const technology = technologies.technologies[0]!;
    const snapshot = carrying({});
    snapshot.insight = 100;
    snapshot.research_availability = [
      {
        technology_id: technology.id,
        complete: false,
        missing_prerequisites: [2],
        insight_shortfall: 7,
      },
    ];
    expect(technologyAvailability(technology, snapshot)).toEqual({
      known: true,
      complete: false,
      prerequisitesMet: false,
      affordable: false,
      missingPrerequisites: [2],
      insightShortfall: 7,
    });
    snapshot.research_availability = [];
    const absent = technologyAvailability(technology, snapshot);
    expect(absent.known).toBe(false);
    expect(absent.prerequisitesMet).toBe(false);
    expect(absent.affordable).toBe(false);
  });

  it("orders by authored branches and stages without changing nodes or their purchase rules", () => {
    const before = structuredClone(technologies);
    const ordered = orderTechnologies(technologies.technologies, technologies);
    expect(ordered.map(({ id }) => id).sort((a, b) => a - b)).toEqual(
      before.technologies.map(({ id }) => id),
    );
    expect(technologies).toEqual(before);
    expect(
      orderTechnologies([...technologies.technologies].reverse(), technologies),
    ).toEqual(ordered);
    const logistics = ordered.filter(({ branch }) => branch === "logistics");
    expect(logistics.map(({ key }) => key)).toEqual([
      "field-logistics",
      "storage-planning",
      "corner-transport",
      "belt-junctions",
      "grade-separation",
    ]);
    expect(technologyContext(logistics[0]!, technologies)).toBe(
      "Logistics · Foundations",
    );
  });
});

describe("cost lines", () => {
  it("reports what is held against every line, and by how much it falls short", () => {
    const snapshot = carrying({ "1": 2 });
    expect(
      costLines(
        [
          { item_id: 1, quantity: 5 },
          { item_id: 2, quantity: 3 },
        ],
        snapshot,
      ),
    ).toEqual([
      { item_id: 1, required: 5, held: 2, shortfall: 3 },
      { item_id: 2, required: 3, held: 0, shortfall: 3 },
    ]);
  });

  it("floors a surplus at zero rather than reporting a negative shortfall", () => {
    expect(
      costLines([{ item_id: 1, quantity: 2 }], carrying({ "1": 9 })),
    ).toEqual([{ item_id: 1, required: 2, held: 9, shortfall: 0 }]);
  });

  it("treats an item the player has never carried as zero held", () => {
    expect(heldQuantity(carrying({}), 7)).toBe(0);
  });
});

describe("building availability", () => {
  it("names which line of a cost is short instead of only that it is unaffordable", () => {
    const belt = building("belt");
    const [line] = belt.construction_cost;
    if (!line) throw new Error("a belt is expected to cost something");
    const short = buildingAvailability(
      belt,
      carrying({ [String(line.item_id)]: line.quantity - 1 }),
      definitions.items,
    );
    expect(short.affordable).toBe(false);
    expect(short.cost).toEqual([
      {
        item_id: line.item_id,
        required: line.quantity,
        held: line.quantity - 1,
        shortfall: 1,
      },
    ]);
  });

  it("derives affordability from the lines, so the two can never disagree", () => {
    const belt = building("belt");
    const [line] = belt.construction_cost;
    if (!line) throw new Error("a belt is expected to cost something");
    const paid = buildingAvailability(
      belt,
      carrying({ [String(line.item_id)]: line.quantity }),
      definitions.items,
    );
    expect(paid.affordable).toBe(true);
    expect(paid.cost.every(({ shortfall }) => shortfall === 0)).toBe(true);
  });

  it("gives a scenario-only building an empty bill that is affordable by definition", () => {
    const free = definitions.buildings.find(
      ({ construction_cost }) => construction_cost.length === 0,
    ) as BuildingDefinition | undefined;
    if (!free) return;
    const availability = buildingAvailability(
      free,
      carrying({}),
      definitions.items,
    );
    expect(availability.cost).toEqual([]);
    expect(availability.affordable).toBe(true);
    expect(availability.costLabel).toBe("Scenario only");
  });
});
