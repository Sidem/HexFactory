import { describe, expect, it } from "vitest";

import {
  buildingAvailability,
  costLines,
  heldQuantity,
  technologyAvailability,
} from "../src/core/availability";
import definitionData from "../src/data/definitions.json";
import technologies from "../src/data/technologies.json";
import { orderTechnologies, technologyContext } from "../src/ui/research";
import {
  RESEARCH_ICON_KEYS,
  researchIconSvg,
} from "../src/rendering/researchIcons";
import {
  layoutResearch,
  researchAncestors,
  researchBenefits,
  researchMatches,
  researchNeighbor,
  RESEARCH_NODE_WIDTH,
  RESEARCH_NODE_HEIGHT,
} from "../src/ui/researchGraph";
import type {
  BuildingDefinition,
  Definitions,
  FactorySnapshot,
} from "../src/core/types";

const definitions = definitionData as Definitions;

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
  it("provides a distinct accessible emblem for every authored technology", () => {
    expect([...RESEARCH_ICON_KEYS].sort()).toEqual(
      technologies.technologies.map((tech) => tech.key).sort(),
    );
    const icons = technologies.technologies.map((tech) =>
      researchIconSvg(tech.key),
    );
    expect(new Set(icons).size).toBe(technologies.technologies.length);
    for (const icon of icons) expect(icon).toContain('aria-hidden="true"');
    expect(researchIconSvg("future-technology")).toContain(
      'viewBox="0 0 32 32"',
    );
  });

  it("refuses malformed graph dependencies instead of recursing indefinitely", () => {
    const first = technologies.technologies[0]!;
    expect(() =>
      layoutResearch({
        ...technologies,
        technologies: [{ ...first, prerequisites: [first.id] }],
      }),
    ).toThrow("acyclic");
    expect(() =>
      layoutResearch({
        ...technologies,
        technologies: [{ ...first, prerequisites: [999] }],
      }),
    ).toThrow("Unknown prerequisite 999");
  });
  it("lays out every node and prerequisite deterministically with no overlapping cards", () => {
    const layout = layoutResearch(technologies);
    const reversed = {
      ...technologies,
      technologies: [...technologies.technologies].reverse(),
    };
    expect(layoutResearch(reversed)).toEqual(layout);
    expect(layout.nodes).toHaveLength(19);
    expect(layout.edges).toHaveLength(
      technologies.technologies.reduce(
        (count, node) => count + node.prerequisites.length,
        0,
      ),
    );
    for (const node of layout.nodes) {
      expect(node.x + RESEARCH_NODE_WIDTH).toBeLessThanOrEqual(layout.width);
      expect(node.y + RESEARCH_NODE_HEIGHT).toBeLessThanOrEqual(layout.height);
      for (const other of layout.nodes.filter(
        (candidate) => candidate.id !== node.id,
      ))
        expect(
          Math.abs(node.x - other.x) >= RESEARCH_NODE_WIDTH ||
            Math.abs(node.y - other.y) >= RESEARCH_NODE_HEIGHT,
        ).toBe(true);
    }
    for (const edge of layout.edges)
      expect(
        layout.nodes.find((node) => node.id === edge.from)!.rank,
      ).toBeLessThan(layout.nodes.find((node) => node.id === edge.to)!.rank);
    // Every drawn segment must stay outside unrelated icon hit targets.
    for (const edge of layout.edges) {
      const [x1, y1, lane, x2, y2] = edge.path
        .match(/[\d.]+/g)!
        .map(Number) as [number, number, number, number, number];
      for (const node of layout.nodes.filter(
        (node) => node.id !== edge.from && node.id !== edge.to,
      )) {
        for (const [ax, ay, bx, by] of [
          [x1, y1, x1, lane],
          [x1, lane, x2, lane],
          [x2, lane, x2, y2],
        ] as [number, number, number, number][]) {
          const crosses =
            ax === bx
              ? ax > node.x &&
                ax < node.x + RESEARCH_NODE_WIDTH &&
                Math.max(ay, by) > node.y &&
                Math.min(ay, by) < node.y + RESEARCH_NODE_HEIGHT
              : ay > node.y &&
                ay < node.y + RESEARCH_NODE_HEIGHT &&
                Math.max(ax, bx) > node.x &&
                Math.min(ax, bx) < node.x + RESEARCH_NODE_WIDTH;
          expect(
            crosses,
            `${edge.from}→${edge.to} crosses icon ${node.id}`,
          ).toBe(false);
        }
      }
    }
    expect(researchAncestors(14, technologies)).toEqual(
      new Set([14, 13, 8, 5, 2, 12]),
    );
    expect(researchNeighbor(layout.nodes, 1, "ArrowRight")).toBeDefined();
    expect(researchNeighbor(layout.nodes, 1, "ArrowLeft")).toBeDefined();
  });

  it("searches unlocks and player effects as well as research names", () => {
    const corner = technologies.technologies.find(
      (technology) => technology.id === 11,
    )!;
    expect(researchBenefits(corner, definitions).join(" ")).toContain(
      "Six corner headings",
    );
    expect(
      researchMatches(corner, "underpass", technologies, definitions),
    ).toBe(true);
    expect(
      researchMatches(
        technologies.technologies[17]!,
        "cargo slots",
        technologies,
        definitions,
      ),
    ).toBe(true);
    expect(
      researchMatches(corner, "unobtainium", technologies, definitions),
    ).toBe(false);
  });
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
