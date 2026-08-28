import { describe, expect, it } from "vitest";

import { nextAction } from "../src/core/guidance";
import type {
  Definitions,
  FactorySnapshot,
  RequestSnapshot,
  Technologies,
} from "../src/core/types";
import definitionsJson from "../src/data/definitions.json";
import scenariosJson from "../src/data/scenarios.json";
import technologiesJson from "../src/data/technologies.json";

const definitions = definitionsJson as unknown as Definitions;
const technologies = technologiesJson as unknown as Technologies;

interface ScenarioStage {
  key: string;
  name: string;
  brief: string;
  requirements: { item_id: number; quantity: number }[];
}

interface ScenarioShape {
  key: string;
  contract: { key: string; name: string; stages: ScenarioStage[] };
}

function shippedScenario(key: string): ScenarioShape {
  const found = (
    scenariosJson as unknown as { scenarios: ScenarioShape[] }
  ).scenarios.find((scenario) => scenario.key === key);
  if (!found) throw new Error(`the ${key} scenario is missing`);
  return found;
}

const newGame = shippedScenario("new-game");
const CRAFTED = new Set(
  definitions.recipes.map((recipe) => recipe.output.item_id),
);

/**
 * The hub's board, modelled the way native draws it: the least-used rows whose item the player
 * could actually supply, three at a time.
 *
 * The walk below only ever gathers by hand, so "could supply" here is "comes out of the ground" —
 * which is the state the opening is in, and the state the funding step has to work in. Filling a
 * row takes it off the board and the next one takes its slot.
 */
function boardFor(filled: string[]): RequestSnapshot[] {
  return definitions.requests
    .filter((request) => {
      const item = definitions.items.find(
        (value) => value.id === request.item_id,
      );
      return (
        !CRAFTED.has(request.item_id) &&
        Boolean(item?.hand_gather_steps) &&
        !filled.includes(request.key)
      );
    })
    .slice(0, 3)
    .map((request) => ({
      key: request.key,
      name: request.name,
      brief: request.brief,
      item_id: request.item_id,
      delivered: 0,
      required: request.quantity,
      insight: request.insight,
      state: "posted" as const,
    }));
}

/**
 * A snapshot is a large object and this suite only reads a corner of it, so the rest is a fixed
 * empty world. What varies between cases is exactly what the guidance is allowed to look at.
 */
function snapshotAt(state: {
  stage: number;
  researched: number[];
  insight: number;
  inventory: Record<string, number>;
  buildings: { definition_id: number; kind: string }[];
  delivered?: Record<number, number>;
  filled?: string[];
}): FactorySnapshot {
  const stage = newGame.contract.stages[state.stage];
  const carry = Object.entries(state.inventory).map(([item, quantity]) => ({
    item_id: Number(item),
    quantity,
  }));
  return {
    scenario: "new-game",
    scenario_name: "New game",
    world_version: 6,
    seed: 1,
    tick: 0,
    checksum: 0,
    delivered: 0,
    delivered_by_item: [],
    insight: state.insight,
    victory: stage === undefined,
    contract: {
      key: newGame.contract.key,
      name: newGame.contract.name,
      stage: state.stage,
      stages: newGame.contract.stages.length,
      stage_key: stage?.key ?? "",
      stage_name: stage?.name ?? "",
      stage_brief: stage?.brief ?? "",
      requirements: (stage?.requirements ?? []).map((need) => ({
        item_id: need.item_id,
        delivered: Math.min(
          need.quantity,
          state.delivered?.[need.item_id] ?? 0,
        ),
        required: need.quantity,
      })),
      complete: stage === undefined,
    },
    requests: boardFor(state.filled ?? []),
    player: {
      x: 0,
      y: 0,
      facing_x: 1000,
      facing_y: 0,
      move_x: 0,
      move_y: 0,
      creative: false,
      inventory: state.inventory,
      action_cooldown: 0,
      build_range: 8870,
      carry_slots: 8,
      carry_stacks: carry,
      radius: 580,
      action_cooldown_total: 15,
      extract_radius: 1,
      walk_goal: null,
      walk_path: [],
    },
    researched: state.researched,
    research_availability: technologies.technologies.map((technology) => ({
      technology_id: technology.id,
      complete: state.researched.includes(technology.id),
      missing_prerequisites: technology.prerequisites.filter(
        (id) => !state.researched.includes(id),
      ),
      insight_shortfall: Math.max(0, technology.cost - state.insight),
    })),
    chunks: [],
    terrain: [],
    resources: [],
    buildings: state.buildings.map((building, index) => ({
      id: index + 1,
      q: index,
      r: 0,
      x: 0,
      y: 0,
      radius: 1024,
      definition_id: building.definition_id,
      kind: building.kind as FactorySnapshot["buildings"][number]["kind"],
      orientation: 0,
      status: "idle",
      progress: 0,
      progress_total: 0,
      inventory: [],
      cargo: null,
      recipe_id: null,
      scenario_owned: false,
      footprint: [{ q: index, r: 0 }],
    })),
    ground_items: [],
    events: [],
  };
}

describe("guidance derived from the rules rather than scripted against them", () => {
  /**
   * The defect this milestone is built around: after Automated Extraction the old script told the
   * player to build a supply line out of extractors and belts, and an extractor draws four power.
   * `power_progress` returns zero off a network and On-site Power is a separate branch the script
   * never named, so the game recommended a factory that could not run.
   *
   * This walks the guide the way a player would — doing exactly what it says, one step at a time —
   * and refuses to accept any step whose prerequisites are not already met in the state that
   * produced it. A guide that outruns its own rules cannot survive the loop.
   */
  it("never names a step the rules would refuse in the state that produced it", () => {
    const state = {
      stage: 0,
      researched: [] as number[],
      insight: 0,
      inventory: {} as Record<string, number>,
      buildings: [] as { definition_id: number; kind: string }[],
      delivered: {} as Record<number, number>,
      filled: [] as string[],
    };
    const researched = new Set<number>();
    const seen: string[] = [];

    for (let step = 0; step < 40; step += 1) {
      const snapshot = snapshotAt(state);
      const guidance = nextAction(snapshot, definitions, technologies);
      seen.push(guidance.key);
      expect(guidance.title.length).toBeGreaterThan(0);
      expect(guidance.detail.length).toBeGreaterThan(0);

      if (guidance.key.startsWith("research:")) {
        const key = guidance.key.slice("research:".length);
        const technology = technologies.technologies.find(
          (value) => value.key === key,
        );
        expect(
          technology,
          `guidance named an unknown technology ${key}`,
        ).toBeDefined();
        if (!technology) break;
        // Achievable in this state: prerequisites met, and paid for.
        for (const prerequisite of technology.prerequisites)
          expect(researched.has(prerequisite)).toBe(true);
        expect(state.insight).toBeGreaterThanOrEqual(technology.cost);
        researched.add(technology.id);
        state.researched = [...researched];
        state.insight -= technology.cost;
        continue;
      }

      if (guidance.key.startsWith("build:") || guidance.key === "power") {
        const key = guidance.key.startsWith("build:")
          ? guidance.key.slice("build:".length)
          : "burner-generator";
        const building = definitions.buildings.find(
          (value) => value.key === key,
        );
        expect(
          building,
          `guidance named an unknown building ${key}`,
        ).toBeDefined();
        if (!building) break;
        expect(building.buildable).toBe(true);
        // The whole point: a build step may only be named once its technology is researched.
        if (building.unlock_technology_id !== undefined)
          expect(researched.has(building.unlock_technology_id)).toBe(true);
        state.buildings = [
          ...state.buildings,
          { definition_id: building.id, kind: building.kind },
        ];
        continue;
      }

      if (guidance.key.startsWith("gather:")) {
        // Gathering is always available to a player with a free slot, which is the state the loop
        // is in here.
        state.inventory = { ...state.inventory, "1": 8, "8": 8 };
        continue;
      }

      if (
        guidance.key.startsWith("fill-request:") ||
        guidance.key.startsWith("deliver-request:")
      ) {
        const key = guidance.key.slice(guidance.key.indexOf(":") + 1);
        const request = definitions.requests.find((value) => value.key === key);
        expect(
          request,
          `guidance named an unknown request ${key}`,
        ).toBeDefined();
        if (!request) break;
        // Achievable in this state, which is the whole point of the walk: the row is posted on the
        // board this very snapshot carried, and its item is something a hand can take out of the
        // ground. A guide that names a row the hub is not asking for is a guide that cannot be
        // followed, and one that names a crafted item before any machine exists is worse.
        expect(snapshot.requests.some((posted) => posted.key === key)).toBe(
          true,
        );
        expect(CRAFTED.has(request.item_id)).toBe(false);
        state.filled = [...state.filled, key];
        state.insight += request.insight;
        state.inventory = { ...state.inventory, "1": 8, "8": 8 };
        continue;
      }

      if (guidance.key === "workshop") {
        const line = snapshot.contract.requirements.find(
          (need) => need.delivered < need.required,
        )!;
        state.inventory = {
          ...state.inventory,
          [String(line.item_id)]: line.required,
        };
        continue;
      }
      if (guidance.key === "deliver" || guidance.key === "supply") {
        const line = snapshot.contract.requirements.find(
          (need) => need.delivered < need.required,
        );
        if (!line) break;
        state.delivered = {
          ...state.delivered,
          [line.item_id]: line.required,
        };
        const outstanding = snapshot.contract.requirements.some(
          (need) =>
            need.item_id !== line.item_id && need.delivered < need.required,
        );
        if (!outstanding) {
          state.stage += 1;
          if (state.stage === 1) {
            for (const id of [1, 2, 4, 8]) researched.add(id);
            state.researched = [...researched];
          }
          state.delivered = {};
        }
        state.inventory = { ...state.inventory, [String(line.item_id)]: 4 };
        continue;
      }

      if (guidance.key === "complete") break;
      throw new Error(`unhandled guidance step ${guidance.key}`);
    }

    // The walk has to actually finish the contract, or the loop above proved nothing.
    expect(seen).toContain("complete");
    // Starter automation is granted by the opening commission, so the walk must not tell the
    // player to buy belts, extractors or power. Industrial firing still costs insight.
    expect(seen.some((key) => key.startsWith("research:on-site-power"))).toBe(
      false,
    );
    expect(seen.some((key) => key.startsWith("research:field-logistics"))).toBe(
      false,
    );
    const processing = seen.indexOf("research:material-processing");
    const kiln = seen.indexOf("build:kiln");
    expect(processing).toBeGreaterThanOrEqual(0);
    expect(kiln).toBeGreaterThan(processing);
    expect(seen[0]).toBe("build:primitive-furnace");
    expect(seen.indexOf("build:manual-workshop")).toBeGreaterThan(0);
    expect(seen.indexOf("workshop")).toBeLessThan(processing);
  });

  it("names one posted request, with its price, rather than the accounting behind it", () => {
    // "Fund Field Logistics" is not something a player can do, and neither is "gather something"
    // now that the hub pays only for what it asked for. Filling a named row is.
    const opening = nextAction(
      snapshotAt({
        stage: 1,
        researched: [1, 2, 4, 8],
        insight: 0,
        inventory: {},
        buildings: [],
      }),
      definitions,
      technologies,
    );
    expect(opening.key).toBe("fill-request:ore-assay");
    expect(opening.detail).toContain("10 insight");
    expect(opening.detail).toContain("landing hub");

    // Carrying the outstanding units changes the answer, because now the hub is one walk away.
    const carrying = nextAction(
      snapshotAt({
        stage: 1,
        researched: [1, 2, 4, 8],
        insight: 0,
        inventory: { "1": 10 },
        buildings: [],
      }),
      definitions,
      technologies,
    );
    expect(carrying.key).toBe("deliver-request:ore-assay");
  });

  it("stops asking for anything once the contract is finished", () => {
    const done = nextAction(
      snapshotAt({
        stage: newGame.contract.stages.length,
        researched: [1, 2, 3],
        insight: 40,
        inventory: {},
        buildings: [],
      }),
      definitions,
      technologies,
    );
    expect(done.key).toBe("complete");
  });

  it("keeps an existing industrial route instead of asking for a redundant primitive station", () => {
    const action = nextAction(
      snapshotAt({
        stage: 0,
        researched: [1, 2, 3, 5, 8],
        insight: 0,
        inventory: { "1": 6 },
        buildings: [
          { definition_id: 3, kind: "composer" },
          { definition_id: 7, kind: "composer" },
          { definition_id: 13, kind: "generator" },
        ],
      }),
      definitions,
      technologies,
    );
    expect(action.key).toBe("supply");
  });

  it("names primitive construction suppliers before the first generator or a replacement kiln", () => {
    const state = {
      stage: 1,
      researched: [1, 2, 3, 4, 5, 8],
      insight: 0,
      inventory: {},
      buildings: [{ definition_id: 28, kind: "composer" }],
    };
    expect(nextAction(snapshotAt(state), definitions, technologies).key).toBe(
      "build:primitive-furnace",
    );
    state.buildings.push({ definition_id: 27, kind: "composer" });
    expect(nextAction(snapshotAt(state), definitions, technologies).key).toBe(
      "power",
    );
    state.buildings.push({ definition_id: 13, kind: "generator" });
    expect(nextAction(snapshotAt(state), definitions, technologies).key).toBe(
      "build:kiln",
    );
    // A legacy factory may have power but no furnace left. Its brick-only bill still needs plate.
    state.buildings = state.buildings.filter(
      (building) => building.definition_id !== 27,
    );
    const snapshot = snapshotAt(state);
    snapshot.contract.requirements = [
      { item_id: 14, delivered: 0, required: 3 },
    ];
    expect(nextAction(snapshot, definitions, technologies).key).toBe(
      "build:primitive-furnace",
    );
    snapshot.player.inventory["11"] = 1;
    expect(nextAction(snapshot, definitions, technologies).key).toBe(
      "build:kiln",
    );
  });

  it("uses loaded workshop stock and prioritizes delivery over gathering more raw inputs", () => {
    const snapshot = snapshotAt({
      stage: 0,
      researched: [],
      insight: 0,
      inventory: {},
      buildings: [
        { definition_id: 28, kind: "composer" },
        { definition_id: 27, kind: "composer" },
      ],
    });
    snapshot.buildings[0]!.recipe_id = 1;
    snapshot.buildings[0]!.input_inventory = [
      { item_id: 11, quantity: 1 },
      { item_id: 19, quantity: 1 },
    ];
    snapshot.buildings[1]!.recipe_id = 2;
    snapshot.buildings[1]!.input_inventory = [{ item_id: 1, quantity: 6 }];
    expect(nextAction(snapshot, definitions, technologies).key).toBe(
      "workshop",
    );
    snapshot.buildings[0]!.input_inventory = [];
    snapshot.player.inventory["2"] = 1;
    snapshot.buildings = [];
    expect(nextAction(snapshot, definitions, technologies).key).toBe("deliver");
  });

  it("puts a full pack ahead of everything, because it blocks the rest", () => {
    const full = nextAction(
      snapshotAt({
        stage: 0,
        researched: [],
        insight: 0,
        inventory: {
          "1": 20,
          "3": 10,
          "4": 20,
          "5": 20,
          "6": 20,
          "7": 20,
          "8": 20,
          "9": 20,
        },
        buildings: [],
      }),
      definitions,
      technologies,
    );
    expect(full.key).toBe("pack-full");
  });
});
