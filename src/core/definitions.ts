import type { BuildingDefinition, Definitions, Technologies } from "./types";

const KINDS = new Set([
  "extractor",
  "belt",
  "composer",
  "container",
  "consumer",
  "hub",
  "pump",
  "pole",
  "generator",
  "boiler",
  "bridge",
]);
const PLACEMENT_RULES = new Set([
  "ground",
  "resource",
  "water",
  "elevated",
  "shallows",
]);
const POWER_SOURCES = new Set(["burner", "wind", "hydro", "turbine"]);
const ORIENTATION_AXES = new Set(["edge", "corner", "any"]);
/** The axes on which a definition may face a vertex heading, and so name a corner price. */
const CORNER_AXES = new Set(["corner", "any"]);
/** Matches `MAX_EXTRACT_RADIUS` in the core. The rule itself is native's. */
const MAX_EXTRACT_RADIUS = 4;
/** Matches `MAX_UNDERPASS_SPAN` in the core. The rule itself is native's. */
export const MAX_UNDERPASS_SPAN = 4;

export function validateDefinitions(
  value: unknown,
): asserts value is Definitions {
  if (!value || typeof value !== "object")
    throw new TypeError("definitions must be an object");
  const data = value as Partial<Definitions>;
  if (!positiveInteger(data.version))
    throw new TypeError("definitions require a positive version");
  if (
    !Array.isArray(data.items) ||
    !Array.isArray(data.recipes) ||
    !Array.isArray(data.buildings) ||
    !Array.isArray(data.requests)
  ) {
    throw new TypeError(
      "definitions require item, recipe, building, and request arrays",
    );
  }
  uniqueIds(data.items, "item");
  uniqueIds(data.recipes, "recipe");
  uniqueIds(data.buildings, "building");
  uniqueIds(data.requests, "request");
  const itemIds = new Set(data.items.map((item) => item.id));
  for (const item of data.items) {
    if (
      !item.key ||
      !item.name ||
      !item.color ||
      !item.icon ||
      !item.description ||
      !positiveInteger(item.stack_size) ||
      (item.hand_gather_steps !== undefined &&
        !positiveInteger(item.hand_gather_steps))
    )
      throw new TypeError(`item ${item.id} is incomplete`);
  }
  // Requests are the only thing that pays insight, and insight is the only thing that buys
  // research. A catalogue with none of them is one where nothing could ever be learned.
  if (!data.requests.length)
    throw new TypeError("no hub requests: nothing would ever pay insight");
  for (const request of data.requests) {
    if (
      !request.key ||
      !request.name ||
      !request.brief ||
      !itemIds.has(request.item_id) ||
      !positiveInteger(request.quantity) ||
      !positiveInteger(request.insight) ||
      (request.repeat_insight !== undefined &&
        !positiveInteger(request.repeat_insight))
    )
      throw new TypeError(`request ${request.id} is incomplete`);
  }
  const categories = new Set(
    data.buildings
      .map((building) => building.recipe_category)
      .filter((category): category is string => Boolean(category)),
  );
  for (const recipe of data.recipes) {
    if (
      !recipe.key ||
      !recipe.name ||
      !recipe.description ||
      !recipe.category ||
      !positiveInteger(recipe.duration) ||
      !recipe.inputs.length
    ) {
      throw new TypeError(`recipe ${recipe.id} is incomplete`);
    }
    // A recipe no machine can be assigned is unreachable content, which is a defect in the
    // catalog rather than something to discover in play.
    if (!categories.has(recipe.category))
      throw new TypeError(
        `recipe ${recipe.id} has category ${recipe.category}, which no building runs`,
      );
    for (const ingredient of [...recipe.inputs, recipe.output]) {
      if (
        !itemIds.has(ingredient.item_id) ||
        !positiveInteger(ingredient.quantity)
      ) {
        throw new TypeError(`recipe ${recipe.id} has an invalid ingredient`);
      }
    }
  }
  for (const building of data.buildings) {
    if (
      !building.key ||
      !building.name ||
      !building.description ||
      !building.icon ||
      !KINDS.has(building.kind) ||
      !PLACEMENT_RULES.has(building.placement_rule) ||
      !Array.isArray(building.construction_cost) ||
      !Array.isArray(building.footprint) ||
      !building.footprint.length ||
      typeof building.buildable !== "boolean" ||
      typeof building.blocks_movement !== "boolean"
    ) {
      throw new TypeError(`building ${building.id} is incomplete`);
    }
    const footprint = new Set(
      building.footprint.map(({ q, r }) => `${q},${r}`),
    );
    if (
      footprint.size !== building.footprint.length ||
      !footprint.has("0,0") ||
      building.footprint.some(
        ({ q, r }) => !Number.isInteger(q) || !Number.isInteger(r),
      )
    )
      throw new TypeError(`building ${building.id} has an invalid footprint`);
    // A machine that runs recipes needs a category, and one that does not must not claim one.
    if ((building.kind === "composer") !== Boolean(building.recipe_category))
      throw new TypeError(
        `building ${building.id} has a recipe category that does not match its kind`,
      );
    if (
      building.kind === "pump" &&
      !(
        building.output_item_id !== undefined &&
        itemIds.has(building.output_item_id)
      )
    )
      throw new TypeError(`pump ${building.id} requires a known output item`);
    if (
      building.kind === "generator" &&
      !(
        building.power_source !== undefined &&
        POWER_SOURCES.has(building.power_source) &&
        building.power_output !== undefined &&
        building.power_output > 0
      )
    )
      throw new TypeError(`generator ${building.id} needs a source and output`);
    if (building.placement_rule === "shallows" && building.kind !== "bridge")
      throw new TypeError(
        `building ${building.id} places on shallows but is not a bridge`,
      );
    if (
      building.orientation_axis !== undefined &&
      !ORIENTATION_AXES.has(building.orientation_axis)
    )
      throw new TypeError(
        `building ${building.id} has an unknown orientation axis`,
      );
    // No shipped definition needs a multi-cell corner-heading footprint yet. Native keeps the
    // same deliberately narrow rule; the catalog should not reach an untested combination.
    if (
      CORNER_AXES.has(building.orientation_axis ?? "edge") &&
      building.footprint.length !== 1
    )
      throw new TypeError(
        `building ${building.id} spans the two-row period, which only a single-cell footprint can do`,
      );
    // A corner price and a corner gate are answers to a question a building that cannot face a
    // corner is never asked.
    if (
      (building.corner_construction_cost !== undefined ||
        building.corner_technology_id !== undefined) &&
      !CORNER_AXES.has(building.orientation_axis ?? "edge")
    )
      throw new TypeError(
        `building ${building.id} names a corner price or gate but cannot face a corner`,
      );
    // The two-row reach stays a research step. Without its own gate, an any-axis definition would
    // hand the player that reach at the first belt they place.
    if (
      building.orientation_axis === "any" &&
      building.corner_technology_id === undefined
    )
      throw new TypeError(
        `building ${building.id} takes every heading but gates none of them`,
      );
    if (building.underpass_span !== undefined) {
      if (
        !positiveInteger(building.underpass_span) ||
        building.underpass_span > MAX_UNDERPASS_SPAN
      )
        throw new TypeError(
          `building ${building.id} needs a span in 1..=${MAX_UNDERPASS_SPAN}`,
        );
    }
    // Splitting, merging, and spanning are all rules about compiled transport edges, and a
    // building that is not transport compiles none.
    if (
      (building.splits === true ||
        building.merges === true ||
        building.underpass_span !== undefined) &&
      building.kind !== "belt"
    )
      throw new TypeError(
        `building ${building.id} is not transport but claims a transport rule`,
      );
    // One entity, one arbitration rule: a definition that both fans out and rotates its feeders
    // would have two answers for which link a single item takes.
    if (building.splits === true && building.merges === true)
      throw new TypeError(
        `building ${building.id} cannot both split and merge`,
      );
    if (building.extract_radius !== undefined) {
      if (building.kind !== "extractor" && building.kind !== "pump")
        throw new TypeError(
          `building ${building.id} claims a source reach but is not an extractor or pump`,
        );
      if (
        !positiveInteger(building.extract_radius) ||
        building.extract_radius > MAX_EXTRACT_RADIUS
      )
        throw new TypeError(
          `extractor ${building.id} needs a reach in 1..=${MAX_EXTRACT_RADIUS}`,
        );
    }
    for (const ingredient of [
      ...building.construction_cost,
      ...(building.corner_construction_cost ?? []),
    ]) {
      if (
        !itemIds.has(ingredient.item_id) ||
        !positiveInteger(ingredient.quantity)
      )
        throw new TypeError(`building ${building.id} has an invalid cost`);
    }
  }
  validateUpgradeLadders(data.buildings);
}

/**
 * An upgrade may only grow a building into a taller version of itself. Kind, recipe category,
 * footprint, and orientation axis are all pinned across a step, which is what lets the command
 * preserve contents, orientation, and connections without asking whether any of them still apply.
 * The strictly increasing tier is what keeps a ladder finite.
 */
function validateUpgradeLadders(buildings: BuildingDefinition[]): void {
  const byId = new Map(buildings.map((building) => [building.id, building]));
  for (const building of buildings) {
    if (building.upgrades_to === undefined) continue;
    const next = byId.get(building.upgrades_to);
    if (!next)
      throw new TypeError(
        `building ${building.id} upgrades to unknown building ${building.upgrades_to}`,
      );
    if ((next.tier ?? 0) <= (building.tier ?? 0))
      throw new TypeError(
        `building ${building.id} upgrades to ${next.id}, which is not a higher tier`,
      );
    if (
      next.kind !== building.kind ||
      next.recipe_category !== building.recipe_category ||
      (next.orientation_axis ?? "edge") !==
        (building.orientation_axis ?? "edge")
    )
      throw new TypeError(
        `building ${building.id} upgrades into a different machine, not a higher tier of itself`,
      );
    if (!next.buildable)
      throw new TypeError(
        `building ${building.id} upgrades to ${next.id}, which cannot be constructed`,
      );
    const cells = (definition: BuildingDefinition): string =>
      definition.footprint
        .map(({ q, r }) => `${q},${r}`)
        .sort()
        .join(" ");
    if (cells(next) !== cells(building))
      throw new TypeError(
        `building ${building.id} upgrades to a different footprint, which would move its connections`,
      );
  }
}

export function validateTechnologies(
  value: unknown,
  definitions: Definitions,
): asserts value is Technologies {
  if (!value || typeof value !== "object")
    throw new TypeError("technologies must be an object");
  const data = value as Partial<Technologies>;
  if (!positiveInteger(data.version) || !Array.isArray(data.technologies))
    throw new TypeError("technologies require a version and array");
  uniqueIds(data.technologies, "technology");
  const ids = new Set(data.technologies.map(({ id }) => id));
  const buildingIds = new Set(definitions.buildings.map(({ id }) => id));
  for (const technology of data.technologies) {
    if (
      !technology.key ||
      !technology.name ||
      !technology.description ||
      !positiveInteger(technology.cost) ||
      (technology.carry_slots_bonus !== undefined &&
        (!positiveInteger(technology.carry_slots_bonus) ||
          technology.carry_slots_bonus > 240)) ||
      (technology.build_range_bonus !== undefined &&
        (!positiveInteger(technology.build_range_bonus) ||
          technology.build_range_bonus > 96)) ||
      technology.prerequisites.some((id) => !ids.has(id)) ||
      technology.unlocks.some((id) => !buildingIds.has(id))
    )
      throw new TypeError(`technology ${technology.id} is invalid`);
  }
  const completed = new Set<number>();
  while (completed.size < data.technologies.length) {
    const before = completed.size;
    for (const technology of data.technologies)
      if (technology.prerequisites.every((id) => completed.has(id)))
        completed.add(technology.id);
    if (completed.size === before)
      throw new TypeError("technology graph must be acyclic");
  }
  for (const building of definitions.buildings)
    if (
      building.unlock_technology_id !== undefined &&
      !ids.has(building.unlock_technology_id)
    )
      throw new TypeError(`building ${building.id} has an invalid unlock`);
}

function uniqueIds(values: Array<{ id: number }>, label: string): void {
  const ids = new Set<number>();
  for (const value of values) {
    if (!positiveInteger(value.id) || ids.has(value.id))
      throw new TypeError(`${label} IDs must be positive and unique`);
    ids.add(value.id);
  }
}

function positiveInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) > 0;
}
