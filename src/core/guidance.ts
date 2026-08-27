import { supportsRecipe } from "./definitions";
import { technologyAvailability } from "./availability";
import type {
  BuildingDefinition,
  Definitions,
  FactorySnapshot,
  RequestSnapshot,
  RecipeDefinition,
  Technologies,
  TechnologyDefinition,
} from "./types";

/**
 * What to do next, derived from the rules rather than scripted against them.
 *
 * The v0.17 guidance was a ladder of hand-written branches, and it had exactly the defect a script
 * eventually gets: after Automated Extraction it told the player to build a supply line out of
 * extractors and belts, and an extractor draws four power. `power_progress` returns zero off a
 * network and On-site Power is a separate branch the script never named, so the game recommended a
 * factory that could not run. **No scripted guide may outrun the rules it is explaining.**
 *
 * So this is not a script. It reads the contract the hub is actually asking for, expands that bill
 * through the shipped recipe tree, collects the machines those recipes need and the technologies
 * those machines sit behind — including the power branch, because a machine that draws and a world
 * that generates none is the same defect in data form — and then reports the first prerequisite the
 * player has not met. Every answer it can give is therefore achievable in the state that produced
 * it: a research whose prerequisites hold and whose cost is paid, or a building whose technology is
 * already researched.
 *
 * It is presentation, not truth: it recomputes nothing the simulation owns. Every fact it uses is
 * either published in the snapshot or read from the same catalogues native validated at boot.
 */
export interface Guidance {
  /** A stable identifier for the step, so a test can name one without matching prose. */
  key: string;
  title: string;
  detail: string;
}

/** The dependency set behind a bill: what has to exist before any of it can be delivered. */
interface Requirements {
  /** Buildings whose recipe category the tree runs through, cheapest per category. */
  machines: BuildingDefinition[];
  /** Raw items the tree bottoms out in — what a hand or an extractor has to supply. */
  raw: number[];
  /** Technologies those machines sit behind, with their ancestors. */
  technologies: number[];
  /** True when anything in the set draws power, so the plan needs a generator to be a plan. */
  needsPower: boolean;
}

export function nextAction(
  snapshot: FactorySnapshot,
  definitions: Definitions,
  technologies: Technologies,
): Guidance {
  const researched = new Set(snapshot.researched);
  const contract = snapshot.contract;

  if (snapshot.scenario === "factory-demo")
    return {
      key: "demo",
      title: "Trace the material flow",
      detail:
        "Follow cargo from extractor to receiver. Pause or single-step to inspect arbitration.",
    };

  if (contract.complete)
    return {
      key: "complete",
      title: "Factory online",
      detail:
        "The founding contract is finished and the hub is built. Expand, optimize, or inspect the running line.",
    };

  if (snapshot.player.carry_stacks.length >= snapshot.player.carry_slots)
    return {
      key: "pack-full",
      title: "Your pack is full",
      detail:
        "Deliver at the landing hub, or build a container and take stacks back out of it from the inspector.",
    };

  const outstanding = contract.requirements.filter(
    (need) => need.delivered < need.required,
  );
  const wanted = outstanding.map((need) => need.item_id);
  const needs = expand(
    wanted,
    definitions,
    technologies,
    new Set(snapshot.buildings.map((building) => building.definition_id)),
  );

  // 1. Research, in dependency order. Only a technology whose prerequisites are all met can be
  //    named, so the step is always one the research panel will actually accept.
  const missing = needs.technologies
    .map((id) => technologies.technologies.find((value) => value.id === id))
    .filter((value): value is TechnologyDefinition => value !== undefined)
    .filter((value) => !researched.has(value.id));
  const ready = missing.find(
    (technology) =>
      technologyAvailability(technology, snapshot).prerequisitesMet,
  );
  if (ready) {
    const availability = technologyAvailability(ready, snapshot);
    if (availability.affordable)
      return {
        key: `research:${ready.key}`,
        title: `Research ${ready.name}`,
        detail: `${ready.description} You have the ${ready.cost} insight it costs.`,
      };
    // Funding is not an instruction a player can carry out, and neither is "gather something" now
    // that the hub only pays for what it posted. The step names one row of the board: which item,
    // how much of it is still wanted, and what filling it pays.
    const short = availability.insightShortfall;
    const closest = [...snapshot.requests].sort(
      (a, b) => stillToFind(a, snapshot) - stillToFind(b, snapshot),
    )[0];
    if (closest) {
      const item = definitions.items.find(
        (value) => value.id === closest.item_id,
      );
      const name = item?.name ?? `item ${closest.item_id}`;
      const left = closest.required - closest.delivered;
      const funding = `${closest.name} pays ${closest.insight} insight; ${ready.name} costs ${ready.cost} and you are ${short} short.`;
      return stillToFind(closest, snapshot) === 0
        ? {
            key: `deliver-request:${closest.key}`,
            title: `Deliver ${name.toLowerCase()} to the landing hub`,
            detail: `You are carrying the ${left} the hub is still waiting for. ${funding}`,
          }
        : {
            key: `fill-request:${closest.key}`,
            title: `Fill the hub's request for ${name.toLowerCase()}`,
            detail: `${closest.brief} ${stillToFind(closest, snapshot)} more, then deliver at the landing hub. ${funding}`,
          };
    }
    return {
      key: `gather-for:${ready.key}`,
      title: "Gather material for insight",
      detail: `Walk onto a field and gather, then deliver it at the landing hub. ${ready.name} costs ${ready.cost} insight and you are ${short} short.`,
    };
  }

  // 2. Power, before the machines that need it. This is the step the scripted guidance skipped:
  //    a machine off a network makes nothing, so a plan that names one without a generator is a
  //    plan the rules refuse.
  const built = snapshot.buildings;
  const draws =
    needs.needsPower ||
    built.some(
      (entity) =>
        (definitionOf(entity.definition_id, definitions)?.power_draw ?? 0) > 0,
    );
  const generates = built.some(
    (entity) =>
      (definitionOf(entity.definition_id, definitions)?.power_output ?? 0) > 0,
  );
  if (draws && !generates) {
    const generator = cheapestGenerator(definitions);
    if (generator && researched.has(generator.unlock_technology_id ?? -1))
      return {
        key: "power",
        title: `Build a ${generator.name.toLowerCase()}`,
        detail:
          "Every machine here draws power, and a machine off a network makes nothing at all. Place a generator within reach of the line, or link them with a pole, and feed it any fuel item.",
      };
  }

  // 3. The machines themselves, in tree order, so a chain is built from its inputs outward.
  const missingMachine = needs.machines.find(
    (machine) =>
      !built.some((entity) => entity.definition_id === machine.id) &&
      (machine.unlock_technology_id === undefined ||
        researched.has(machine.unlock_technology_id)),
  );
  if (missingMachine) {
    // A machine whose recipes burn is a machine that stands idle without fuel, and "out of fuel"
    // is a status rather than a missing input, so nothing in the recipe row would ever say so.
    const burns = definitions.recipes.some(
      (recipe) =>
        supportsRecipe(missingMachine, recipe) && (recipe.fuel ?? 0) > 0,
    );
    return {
      key: `build:${missingMachine.key}`,
      title: `Build a ${missingMachine.name.toLowerCase()}`,
      detail: `${missingMachine.description} This station can make part of the hub's bill.${
        burns ? " Keep coal, charcoal, or wood in its fuel compartment." : ""
      }`,
    };
  }

  const line = outstanding[0];
  const item = definitions.items.find((value) => value.id === line?.item_id);
  const carrying = line
    ? (snapshot.player.inventory[String(line.item_id)] ?? 0)
    : 0;
  if (line && carrying > 0)
    return {
      key: "deliver",
      title: `Deliver ${item?.name ?? "the bill"}`,
      detail: `The hub wants ${line.required - line.delivered} more. You are carrying ${carrying}: walk to the landing hub and deliver.`,
    };

  // 4. Material. A raw line the player is neither carrying nor extracting is the reason a chain
  //    stands idle, and the answer is the hand until an extractor covers it.
  const extractors = built.filter(
    (entity) =>
      definitionOf(entity.definition_id, definitions)?.kind === "extractor",
  ).length;
  const missingRaw = needs.raw.find(
    (item) =>
      (snapshot.player.inventory[String(item)] ?? 0) === 0 &&
      !built.some(
        (entity) =>
          needs.machines.some(
            (machine) => machine.id === entity.definition_id,
          ) &&
          ((entity.input_inventory ?? entity.inventory ?? []).some(
            (stock) => stock.item_id === item && stock.quantity > 0,
          ) ||
            (definitions.recipes
              .find((recipe) => recipe.id === entity.recipe_id)
              ?.inputs.some((input) => input.item_id === item) &&
              (entity.progress > 0 ||
                (entity.output_inventory ?? []).some(
                  (stock) => stock.quantity > 0,
                )))),
      ),
  );
  if (missingRaw !== undefined && extractors === 0) {
    const item = definitions.items.find((value) => value.id === missingRaw);
    return {
      key: `gather:${item?.key ?? missingRaw}`,
      title: `Find ${item?.name ?? "raw material"}`,
      detail:
        `${item?.description ?? ""} Terrain is the material map, so walk the band it belongs to, then gather — or place an extractor on the field and let it work.`.trim(),
    };
  }

  // 5. Everything the bill needs exists. What is left is the delivery itself.
  const workshop = needs.machines.find((machine) => machine.manual_work);
  if (workshop)
    return {
      key: "workshop",
      title: "Work at the manual workshop",
      detail:
        "Inspect the workshop, choose a recipe and load its ingredients. Stand within one hex and press Work one batch. Take the output and carry it to the hub; walking or gathering pauses work.",
    };
  return {
    key: "supply",
    title: `Supply ${item?.name ?? "the landing hub"}`,
    detail: line
      ? `${line.delivered} of ${line.required} delivered. Point the line's output at the landing hub, or carry it there yourself.`
      : "Keep the line supplied and pointed at the landing hub.",
  };
}

/**
 * How many more of a request's item the player has to find before they can finish it — what is
 * still wanted, less what is already in the pack. Zero means the delivery is the only step left,
 * which is what decides whether the guide says "gather" or "walk to the hub".
 */
function stillToFind(
  request: RequestSnapshot,
  snapshot: FactorySnapshot,
): number {
  const carried = snapshot.player.inventory[String(request.item_id)] ?? 0;
  return Math.max(0, request.required - request.delivered - carried);
}

/**
 * Expand a bill into everything that has to exist before it can be delivered.
 *
 * The walk is the same one `balance.rs` runs over a contract stage, which is not a coincidence:
 * one of them prices the bill and the other explains it, and they would be worth nothing if they
 * disagreed about what the bill needs.
 */
function expand(
  wanted: number[],
  definitions: Definitions,
  technologies: Technologies,
  installed: ReadonlySet<number>,
): Requirements {
  const machines: BuildingDefinition[] = [];
  const raw: number[] = [];
  const seen = new Set<number>();

  const walk = (itemId: number): void => {
    if (seen.has(itemId)) return;
    seen.add(itemId);
    const recipe = definitions.recipes.find(
      (value) => value.output.item_id === itemId,
    );
    if (!recipe) {
      raw.push(itemId);
      const item = definitions.items.find((value) => value.id === itemId);
      if (!item?.hand_gather_steps) {
        const extractor = cheapestExtractor(definitions);
        if (extractor && !machines.some((value) => value.id === extractor.id))
          machines.push(extractor);
      }
      return;
    }
    const machine = cheapestFor(recipe, definitions, installed);
    if (machine && !machines.some((value) => value.id === machine.id))
      machines.push(machine);
    // Inputs first in the returned order, so "build the smelter" comes before "build the composer
    // it feeds" — a chain is easiest to build from its source outward.
    for (const input of recipe.inputs) walk(input.item_id);
  };
  for (const item of wanted) walk(item);
  machines.reverse();

  const needsPower = machines.some((machine) => (machine.power_draw ?? 0) > 0);
  const needed: number[] = [];
  const add = (id: number | undefined): void => {
    if (id === undefined || needed.includes(id)) return;
    needed.push(id);
  };
  // The power branch is not any recipe's category, so nothing below would ever reach it. It is
  // named here for the same reason the balance report names it: a machine that draws power and a
  // world with no generator in it is a factory that cannot run. It goes first because a player who
  // unlocks the machines before the network builds a line and then watches it stand still.
  if (
    needsPower &&
    !definitions.buildings.some(
      (building) =>
        installed.has(building.id) && (building.power_output ?? 0) > 0,
    )
  )
    add(cheapestGenerator(definitions)?.unlock_technology_id);
  for (const machine of machines) add(machine.unlock_technology_id);
  return {
    machines,
    raw,
    technologies: withAncestors(needed, technologies),
    needsPower,
  };
}

/**
 * Every technology in the set, preceded by everything it depends on, in an order where a
 * prerequisite always comes before what needs it.
 *
 * The graph is validated acyclic natively, so this terminates; the `seen` guard is what makes it
 * terminate on a graph that is not, rather than recursing until the stack gives out.
 */
function withAncestors(ids: number[], technologies: Technologies): number[] {
  const ordered: number[] = [];
  const seen = new Set<number>();
  const walk = (id: number): void => {
    if (seen.has(id)) return;
    seen.add(id);
    const technology = technologies.technologies.find(
      (value) => value.id === id,
    );
    for (const prerequisite of technology?.prerequisites ?? [])
      walk(prerequisite);
    ordered.push(id);
  };
  for (const id of ids) walk(id);
  return ordered;
}

function definitionOf(
  id: number,
  definitions: Definitions,
): BuildingDefinition | undefined {
  return definitions.buildings.find((building) => building.id === id);
}

/** The cheapest buildable machine that supports this recipe, by expanded construction cost. */
function cheapestFor(
  recipe: RecipeDefinition,
  definitions: Definitions,
  installed: ReadonlySet<number>,
): BuildingDefinition | undefined {
  return definitions.buildings
    .filter(
      (building) => building.buildable && supportsRecipe(building, recipe),
    )
    .sort(
      (a, b) =>
        Number(installed.has(b.id)) - Number(installed.has(a.id)) ||
        cost(a, definitions) - cost(b, definitions),
    )[0];
}

function cheapestExtractor(
  definitions: Definitions,
): BuildingDefinition | undefined {
  return definitions.buildings
    .filter((building) => building.buildable && building.kind === "extractor")
    .sort((a, b) => cost(a, definitions) - cost(b, definitions))[0];
}

function cheapestGenerator(
  definitions: Definitions,
): BuildingDefinition | undefined {
  return definitions.buildings
    .filter(
      (building) => building.buildable && (building.power_output ?? 0) > 0,
    )
    .sort((a, b) => cost(a, definitions) - cost(b, definitions))[0];
}

/**
 * What a building costs in raw material, expanded through the whole recipe tree.
 *
 * Counting the lines of the bill gave the same ordering as this while every station was a pile of
 * ore, and stopped giving it the moment stations were billed in parts: a composer is four items
 * and eleven units of raw material, a manual workshop is six items and six units. Sorting by item
 * count sent a player with nothing built to the dearer of the two because its bill was shorter.
 *
 * This is `balance.rs`'s `raw_units` computed independently and to less precision — enough to
 * order two bills, not to price one.
 */
function cost(building: BuildingDefinition, definitions: Definitions): number {
  return building.construction_cost.reduce(
    (total, line) =>
      total + line.quantity * rawCost(line.item_id, definitions, new Set()),
    0,
  );
}

function rawCost(
  itemId: number,
  definitions: Definitions,
  seen: ReadonlySet<number>,
): number {
  // A recipe that reaches its own output is priced as raw rather than recursed into. The catalogue
  // is validated acyclic natively; this is what makes a broken one return a number anyway.
  if (seen.has(itemId)) return 1;
  const recipe = definitions.recipes.find(
    (value) => value.output.item_id === itemId,
  );
  if (!recipe) return 1;
  const next = new Set(seen).add(itemId);
  const inputs = recipe.inputs.reduce(
    (total, input) =>
      total + input.quantity * rawCost(input.item_id, definitions, next),
    0,
  );
  return inputs / Math.max(recipe.output.quantity, 1);
}
