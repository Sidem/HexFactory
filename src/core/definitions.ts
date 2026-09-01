import { validateSkills } from "./skills";
import { DIRECTIONS } from "./lattice";
import { productionRoutes, recipeOutputs } from "./recipes";
import type {
  BuildingDefinition,
  Definitions,
  RecipeDefinition,
  Technologies,
  TechnologyDefinition,
  TechnologyEffect,
} from "./types";

/** Presentation mirror of native capability validation; never grants a recipe itself. */
export function supportsRecipe(
  building: BuildingDefinition,
  recipe: RecipeDefinition,
): boolean {
  return (
    building.kind === "composer" &&
    (building.recipe_ids
      ? building.recipe_ids.includes(recipe.id)
      : building.recipe_category === recipe.category)
  );
}

/**
 * The most cells a definition's footprint may claim: the complete two-ring hexagon. Native's
 * `MAX_FOOTPRINT_CELLS`.
 *
 * Nineteen is the largest structure Phase 8's physical catalogue asks for, and it is a shape rather
 * than a round number, so a definition that reaches the ceiling is still one readable building. The
 * host mirrors the bound because it draws a preview cell and an occupancy key per footprint cell
 * too; a definition file may not make either of those unbounded on this side either.
 */
const MAX_FOOTPRINT_CELLS = 19;
const MAX_ENVELOPE_CELLS = MAX_FOOTPRINT_CELLS;
const MAX_CLEARANCE_CELLS = MAX_FOOTPRINT_CELLS;

/**
 * True when every cell of a definition's footprint is reachable from its anchor through the six
 * edge steps. Native's `footprint_is_contiguous`.
 *
 * Asked of the authored offsets only. Rotation by whole sixths is a symmetry of this lattice, so a
 * contiguous footprint stays contiguous at every heading, and translating it to a placement anchor
 * cannot separate it either.
 */
function reservationCells(
  cells: { q: number; r: number }[] | undefined,
  buildingId: number,
  label: string,
  max: number,
): Set<string> {
  if (cells === undefined) return new Set();
  if (
    !Array.isArray(cells) ||
    cells.some(({ q, r }) => !Number.isInteger(q) || !Number.isInteger(r))
  )
    throw new TypeError(`building ${buildingId} has an invalid ${label}`);
  const unique = new Set(cells.map(({ q, r }) => `${q},${r}`));
  if (unique.size !== cells.length || unique.size > max)
    throw new TypeError(`building ${buildingId} has an invalid ${label}`);
  return unique;
}

function footprintIsContiguous(cells: ReadonlySet<string>): boolean {
  const reached = new Set(["0,0"]);
  const frontier: [number, number][] = [[0, 0]];
  for (let cell = frontier.pop(); cell; cell = frontier.pop()) {
    for (const [dq, dr] of DIRECTIONS) {
      const step: [number, number] = [cell[0] + dq!, cell[1] + dr!];
      const key = `${step[0]},${step[1]}`;
      if (cells.has(key) && !reached.has(key)) {
        reached.add(key);
        frontier.push(step);
      }
    }
  }
  return reached.size === cells.size;
}

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
const FOUNDATION_CLASSES = new Set(["pad", "span", "retaining"]);
const POWER_SOURCES = new Set(["burner", "wind", "hydro", "turbine"]);
const ORIENTATION_AXES = new Set(["edge", "corner", "any"]);
/** The axes on which a definition may face a vertex heading, and so name a corner price. */
const CORNER_AXES = new Set(["corner", "any"]);
/** Matches `MAX_EXTRACT_RADIUS` in the core. The rule itself is native's. */
const MAX_EXTRACT_RADIUS = 4;
/** Matches `MAX_UNDERPASS_SPAN` in the core. The rule itself is native's. */
export const MAX_UNDERPASS_SPAN = 4;
/** Matches `UNTREATED_MOVEMENT` in the core: raw ground is the hundred everything else is a
 * percentage of. */
export const UNTREATED_MOVEMENT = 100;
/** Matches `MAX_SURFACE_MOVEMENT` in the core. The rule itself is native's. */
export const MAX_SURFACE_MOVEMENT = 150;
/** Matches `MAX_GRADE_STEPS` in the core: how far a hex may be cut or filled from its own grade. */
export const MAX_GRADE_STEPS = 3;

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
  if (!Array.isArray(data.boundaries))
    throw new TypeError("definitions require boundaries");
  uniqueIds(data.boundaries, "boundary");
  const boundaryKeys = new Set<string>();
  for (const boundary of data.boundaries) {
    if (
      !boundary.key ||
      boundaryKeys.has(boundary.key) ||
      !boundary.name ||
      !boundary.description ||
      (boundary.family !== "fence" && boundary.family !== "wall") ||
      typeof boundary.gate !== "boolean" ||
      !Array.isArray(boundary.construction_cost) ||
      boundary.construction_cost.length === 0 ||
      boundary.construction_cost.some(
        (i) =>
          !itemIds.has(i.item_id) ||
          !positiveInteger(i.quantity) ||
          i.quantity > 1000,
      ) ||
      new Set(boundary.construction_cost.map((i) => i.item_id)).size !==
        boundary.construction_cost.length
    )
      throw new TypeError("Invalid boundary definition or construction bill");
    boundaryKeys.add(boundary.key);
  }
  if (!Array.isArray(data.surfaces))
    throw new TypeError("definitions require surfaces");
  uniqueIds(data.surfaces, "surface");
  const surfaceKeys = new Set<string>();
  for (const surface of data.surfaces) {
    if (
      !surface.key ||
      surfaceKeys.has(surface.key) ||
      !surface.name ||
      !surface.description ||
      // The same window native enforces: a surface slower than raw ground would be a trap dressed
      // as a road, and one above the ceiling would outrun the route search's heuristic.
      !positiveInteger(surface.movement) ||
      surface.movement < UNTREATED_MOVEMENT ||
      surface.movement > MAX_SURFACE_MOVEMENT ||
      !Array.isArray(surface.construction_cost) ||
      surface.construction_cost.some(
        (i) =>
          !itemIds.has(i.item_id) ||
          !positiveInteger(i.quantity) ||
          i.quantity > 1000,
      ) ||
      new Set(surface.construction_cost.map((i) => i.item_id)).size !==
        surface.construction_cost.length
    )
      throw new TypeError("Invalid surface definition or construction bill");
    surfaceKeys.add(surface.key);
    if (
      surface.base_surface_id !== undefined &&
      !data.surfaces.some(
        (base) =>
          base.id === surface.base_surface_id &&
          base.id !== surface.id &&
          base.base_surface_id === undefined,
      )
    )
      throw new TypeError(
        "Surface base must be a different, single-layer surface",
      );
  }
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
      !positiveInteger(request.insight)
    )
      throw new TypeError(`request ${request.id} is incomplete`);
  }
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
    if (!data.buildings.some((building) => supportsRecipe(building, recipe)))
      throw new TypeError(
        `recipe ${recipe.id} has category ${recipe.category}, which no building runs`,
      );
    const outputs = recipeOutputs(recipe);
    if (
      new Set(outputs.map((output) => output.item_id)).size !==
        outputs.length ||
      outputs.length > 8 ||
      new Set(recipe.inputs.map((input) => input.item_id)).size !==
        recipe.inputs.length
    )
      throw new TypeError(
        `recipe ${recipe.id} has duplicate or excessive ingredients`,
      );
    if (
      (outputs.length > 1 || recipe.cost_allocation !== undefined) &&
      (recipe.cost_allocation?.length !== outputs.length ||
        recipe.cost_allocation.some((share) => !positiveInteger(share)) ||
        recipe.cost_allocation.reduce((sum, share) => sum + share, 0) !== 100)
    )
      throw new TypeError(
        `recipe ${recipe.id} requires positive cost shares summing to 100`,
      );
    if (
      data.buildings.some(
        (building) =>
          supportsRecipe(building, recipe) &&
          (building.capacity ?? Number.MAX_SAFE_INTEGER) <
            outputs.reduce((sum, output) => sum + output.quantity, 0),
      )
    )
      throw new TypeError(
        `recipe ${recipe.id} output batch exceeds machine capacity`,
      );
    for (const ingredient of [...recipe.inputs, ...outputs]) {
      if (
        !itemIds.has(ingredient.item_id) ||
        !positiveInteger(ingredient.quantity)
      ) {
        throw new TypeError(`recipe ${recipe.id} has an invalid ingredient`);
      }
    }
  }
  for (const item of data.items) {
    const producers = data.recipes.filter((recipe) =>
      recipeOutputs(recipe).some((output) => output.item_id === item.id),
    );
    const routes = item.production_routes;
    if (producers.length > 1 && routes === undefined)
      throw new TypeError(
        `item ${item.id} requires an explicit production route policy`,
      );
    if (
      routes !== undefined &&
      (new Set(routes).size !== routes.length ||
        routes.some((id) => !producers.some((recipe) => recipe.id === id)))
    )
      throw new TypeError(
        `item ${item.id} requires a valid explicit production route policy`,
      );
    if (
      item.extraction_building_id !== undefined &&
      !data.buildings.some(
        (building) =>
          building.id === item.extraction_building_id &&
          building.kind === "extractor" &&
          building.output_item_id === item.id,
      )
    )
      throw new TypeError(`item ${item.id} has an invalid extraction building`);
  }
  const visiting = new Set<number>();
  const checked = new Set<number>();
  const visit = (item: number): void => {
    if (visiting.has(item))
      throw new TypeError(`recipe cycle through item ${item}`);
    if (checked.has(item)) return;
    visiting.add(item);
    for (const recipe of productionRoutes(data as Definitions, item))
      for (const input of recipe.inputs) visit(input.item_id);
    visiting.delete(item);
    checked.add(item);
  };
  for (const item of data.items) visit(item.id);
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
      footprint.size > MAX_FOOTPRINT_CELLS ||
      building.footprint.some(
        ({ q, r }) => !Number.isInteger(q) || !Number.isInteger(r),
      )
    )
      throw new TypeError(`building ${building.id} has an invalid footprint`);
    // One building is one connected thing. Two lobes with a gap between them would still occupy
    // every cell, but the gap would read as walkable ground inside a building, and reach, routing
    // and the ground pad would all be measuring a shape the player cannot see as one machine.
    if (!footprintIsContiguous(footprint))
      throw new TypeError(
        `building ${building.id} has a footprint in disconnected pieces`,
      );
    if (
      building.foundation_class !== undefined &&
      !FOUNDATION_CLASSES.has(building.foundation_class)
    )
      throw new TypeError(
        `building ${building.id} has an unknown foundation class`,
      );
    const envelopeCells = reservationCells(
      building.service_envelope,
      building.id,
      "service envelope",
      MAX_ENVELOPE_CELLS,
    );
    const clearanceCells = reservationCells(
      building.overhead_clearance,
      building.id,
      "overhead clearance",
      MAX_CLEARANCE_CELLS,
    );
    for (const key of envelopeCells)
      if (footprint.has(key))
        throw new TypeError(
          `building ${building.id} reserves a cell it already occupies`,
        );
    for (const key of clearanceCells) {
      if (footprint.has(key))
        throw new TypeError(
          `building ${building.id} reserves a cell it already occupies`,
        );
      if (envelopeCells.has(key))
        throw new TypeError(
          `building ${building.id} uses the same cell as envelope and clearance`,
        );
    }
    if (
      envelopeCells.size &&
      !footprintIsContiguous(new Set([...footprint, ...envelopeCells]))
    )
      throw new TypeError(
        `building ${building.id} has a service envelope in disconnected pieces`,
      );
    if (
      clearanceCells.size &&
      !footprintIsContiguous(new Set([...footprint, ...clearanceCells]))
    )
      throw new TypeError(
        `building ${building.id} has overhead clearance in disconnected pieces`,
      );
    // A machine that runs recipes needs a category, and one that does not must not claim one.
    if ((building.kind === "composer") !== Boolean(building.recipe_category))
      throw new TypeError(
        `building ${building.id} has a recipe category that does not match its kind`,
      );
    if (
      building.recipe_ids !== undefined &&
      (building.kind !== "composer" ||
        !Array.isArray(building.recipe_ids) ||
        !building.recipe_ids.length ||
        new Set(building.recipe_ids).size !== building.recipe_ids.length ||
        building.recipe_ids.some(
          (id) => !data.recipes!.some((recipe) => recipe.id === id),
        ))
    )
      throw new TypeError(
        `building ${building.id} has invalid recipe capabilities`,
      );
    if (
      building.duration_multiplier !== undefined &&
      (building.kind !== "composer" ||
        !positiveInteger(building.duration_multiplier) ||
        building.duration_multiplier > 60 ||
        data.recipes.some(
          (recipe) =>
            supportsRecipe(building, recipe) &&
            recipe.duration * building.duration_multiplier! > 0xffffffff,
        ))
    )
      throw new TypeError(
        `building ${building.id} has invalid recipe duration multiplier`,
      );
    if (
      building.manual_work !== undefined &&
      typeof building.manual_work !== "boolean"
    )
      throw new TypeError(
        `building ${building.id} has invalid manual work flag`,
      );
    if (
      building.manual_work &&
      (building.kind !== "composer" ||
        !building.recipe_ids ||
        (building.power_draw ?? 0) !== 0 ||
        data.recipes.some(
          (recipe) =>
            supportsRecipe(building, recipe) && (recipe.fuel ?? 0) !== 0,
        ))
    )
      throw new TypeError(
        `building ${building.id} has invalid manual work capabilities`,
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
      building.output_item_id !== undefined &&
      !itemIds.has(building.output_item_id)
    )
      throw new TypeError(`source ${building.id} requires a known output item`);
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
      (building.footprint.length !== 1 ||
        (building.service_envelope?.length ?? 0) > 0 ||
        (building.overhead_clearance?.length ?? 0) > 0)
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
    if (
      building.transport_medium !== undefined &&
      (building.kind !== "belt" ||
        !["solid", "fluid"].includes(building.transport_medium))
    )
      throw new TypeError(
        `building ${building.id} has an invalid transport medium`,
      );
    if (
      building.accepted_item_ids !== undefined &&
      (building.kind !== "container" ||
        building.accepted_item_ids.length === 0 ||
        new Set(building.accepted_item_ids).size !==
          building.accepted_item_ids.length ||
        building.accepted_item_ids.some((id) => !itemIds.has(id)))
    )
      throw new TypeError(
        `building ${building.id} has an invalid storage filter`,
      );
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
 * An upgrade may only grow a building into a taller version of itself. Kind, recipe category and
 * orientation axis are pinned across a step, and the footprint may only grow, which is what lets
 * the command preserve contents, orientation, and connections without asking whether any of them
 * still apply. The strictly increasing tier is what keeps a ladder finite.
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
      JSON.stringify(next.recipe_ids ?? null) !==
        JSON.stringify(building.recipe_ids ?? null) ||
      Boolean(next.manual_work) !== Boolean(building.manual_work) ||
      (next.orientation_axis ?? "edge") !==
        (building.orientation_axis ?? "edge") ||
      (next.foundation_class ?? "pad") !== (building.foundation_class ?? "pad")
    )
      throw new TypeError(
        `building ${building.id} upgrades into a different machine, not a higher tier of itself`,
      );
    if (!next.buildable)
      throw new TypeError(
        `building ${building.id} upgrades to ${next.id}, which cannot be constructed`,
      );
    // A tier may take more ground; it may never give up ground it already stands on. Growing into
    // free cells leaves every existing cell, and therefore every connection bound to one, exactly
    // where it was — native refuses the upgrade unless the new cells are empty, so an output ray
    // that used to leave the footprint at some cell still leaves it at the same one. Shrinking or
    // sliding would strand a belt against a hex the building no longer occupies.
    const cells = new Set(next.footprint.map(({ q, r }) => `${q},${r}`));
    if (building.footprint.some(({ q, r }) => !cells.has(`${q},${r}`)))
      throw new TypeError(
        `building ${building.id} upgrades off a cell it stands on, which would move its connections`,
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
  validateSkills(data);
  if (!positiveInteger(data.version) || !Array.isArray(data.technologies))
    throw new TypeError("technologies require a version and array");
  for (const [label, groups] of [
    ["branch", data.branches],
    ["stage", data.stages],
  ] as const) {
    if (!Array.isArray(groups) || groups.length === 0 || groups.length > 64)
      throw new TypeError(
        `technology ${label} registry requires 1 to 64 entries`,
      );
    const keys = new Set<string>();
    for (const group of groups) {
      if (
        !group ||
        typeof group.key !== "string" ||
        !/^[a-z][a-z0-9-]*$/.test(group.key) ||
        typeof group.name !== "string" ||
        !group.name.trim() ||
        typeof group.description !== "string" ||
        !group.description.trim() ||
        !Number.isInteger(group.order) ||
        group.order < 0 ||
        group.order > 0xffffffff ||
        keys.has(group.key)
      )
        throw new TypeError(
          `technology ${label} registry has an invalid or duplicate entry`,
        );
      keys.add(group.key);
    }
  }
  if (data.technologies.length > 1024)
    throw new TypeError("technology catalog exceeds 1024 entries");
  const branches = new Set(data.branches!.map(({ key }) => key));
  const stages = new Set(data.stages!.map(({ key }) => key));
  uniqueIds(data.technologies, "technology");
  const keys = new Set<string>();
  const ids = new Set(data.technologies.map(({ id }) => id));
  const buildingIds = new Set(definitions.buildings.map(({ id }) => id));
  const boundaryIds = new Set(definitions.boundaries.map(({ id }) => id));
  for (const technology of data.technologies) {
    if (
      !technology.key ||
      !technology.name ||
      !technology.description ||
      keys.has(technology.key) ||
      !branches.has(technology.branch) ||
      !stages.has(technology.stage) ||
      !Array.isArray(technology.prerequisites) ||
      !Array.isArray(technology.effects) ||
      new Set(technology.prerequisites).size !==
        technology.prerequisites.length ||
      !validGrant(technology) ||
      technology.prerequisites.some((id) => !ids.has(id)) ||
      !validEffects(
        technology.effects,
        buildingIds,
        boundaryIds,
        new Set(definitions.surfaces.map(({ id }) => id)),
      )
    )
      throw new TypeError(`technology ${technology.id} is invalid`);
    keys.add(technology.key);
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
  for (const boundary of definitions.boundaries)
    if (
      boundary.unlock_technology_id !== undefined &&
      !ids.has(boundary.unlock_technology_id)
    )
      throw new TypeError(`boundary ${boundary.id} has an invalid unlock`);
  for (const surface of definitions.surfaces)
    if (
      surface.unlock_technology_id !== undefined &&
      !data.technologies.some(
        (technology) =>
          technology.id === surface.unlock_technology_id &&
          technology.effects.some(
            (effect) =>
              effect.kind === "unlock_surface" &&
              effect.surface_id === surface.id,
          ),
      )
    )
      throw new TypeError(`surface ${surface.id} has an invalid unlock`);
}

export function technologyPurchasable(
  technology: TechnologyDefinition,
): boolean {
  return technology.grant?.kind !== "contract_stage";
}

export function technologyGrantLabel(
  technology: TechnologyDefinition,
): string | undefined {
  return technology.grant?.kind === "contract_stage"
    ? technology.grant.name
    : undefined;
}

export function technologyBuildingUnlocks(
  technology: TechnologyDefinition,
): number[] {
  return technology.effects.flatMap((effect) =>
    effect.kind === "unlock_building" ? [effect.building_id] : [],
  );
}

export function technologyBoundaryUnlocks(
  technology: TechnologyDefinition,
): number[] {
  return technology.effects.flatMap((effect) =>
    effect.kind === "unlock_boundary" ? [effect.boundary_id] : [],
  );
}

export function technologyCarrySlotsBonus(
  technology: TechnologyDefinition,
): number {
  return technology.effects.reduce(
    (total, effect) =>
      effect.kind === "carry_slots" ? total + effect.amount : total,
    0,
  );
}

export function technologyBuildRangeBonus(
  technology: TechnologyDefinition,
): number {
  return technology.effects.reduce(
    (total, effect) =>
      effect.kind === "build_range" ? total + effect.amount : total,
    0,
  );
}

function validGrant(technology: TechnologyDefinition): boolean {
  const grant = technology.grant;
  if (grant === undefined || grant.kind === "purchase")
    return positiveInteger(technology.cost);
  if (grant.kind !== "contract_stage") return false;
  return (
    technology.cost === 0 &&
    /^[a-z][a-z0-9-]*$/.test(grant.key) &&
    grant.name.trim().length > 0
  );
}

function validEffects(
  effects: TechnologyEffect[],
  buildingIds: Set<number>,
  boundaryIds: Set<number>,
  surfaceIds: Set<number>,
): boolean {
  const buildings = new Set<number>();
  const boundaries = new Set<number>();
  const surfaces = new Set<number>();
  for (const effect of effects) {
    if (effect.kind === "unlock_building") {
      if (
        !buildingIds.has(effect.building_id) ||
        buildings.has(effect.building_id)
      )
        return false;
      buildings.add(effect.building_id);
      continue;
    }
    if (effect.kind === "unlock_boundary") {
      if (
        !boundaryIds.has(effect.boundary_id) ||
        boundaries.has(effect.boundary_id)
      )
        return false;
      boundaries.add(effect.boundary_id);
      continue;
    }

    if (effect.kind === "unlock_surface") {
      if (!surfaceIds.has(effect.surface_id) || surfaces.has(effect.surface_id))
        return false;
      surfaces.add(effect.surface_id);
      continue;
    }
    return false;
  }
  return true;
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
