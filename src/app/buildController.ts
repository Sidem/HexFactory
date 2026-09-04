import {
  buildingAvailability,
  costAt,
  costLines,
  type CostLine,
} from "../core/availability";
import { supportsRecipe } from "../core/definitions";
import { recipeOutputs } from "../core/recipes";
import type { BuildingDefinition, RecipeDefinition } from "../core/types";
import { BUILDING_COLORS } from "../rendering/FactoryRenderer";
import {
  buildingEmblemSvg,
  clearEmblem,
  emblemRank,
  hasBuildingEmblem,
  paintEmblem,
  recipeCategoryAccent,
  recipeCategoryEmblemSvg,
} from "../rendering/emblems";
import { part, required, syncChildren } from "../ui/dom";
import type { Tool } from "./runtime";
import { Runtime } from "./runtime";

declare module "./runtime" {
  interface Runtime {
    renderHotbarSlots(): void;
    assignHotbarSlot(slot: number, value: Tool | null): void;
    pinToHotbar(value: Tool): void;
    renderHotbar(): void;
    catalogueVisible(
      definition: BuildingDefinition,
      reach: Map<number, number>,
    ): boolean;
    buildMatches(
      definition: BuildingDefinition,
      group: Runtime["BUILD_GROUPS"][number],
      query: string,
    ): boolean;
    renderBuildScope(hidden: number, query: string): void;
    renderBuildPanel(): void;
    createBuildCard(key: string): HTMLElement;
    heldOrientationFor(definition: BuildingDefinition): number;
    paintBuildingEmblem(box: HTMLElement, definition: BuildingDefinition): void;
    fillBuildCard(card: HTMLElement, definition: BuildingDefinition): void;
    renderIngredientRow(
      container: HTMLElement,
      ingredients: {
        item_id: number;
        quantity: number;
      }[],
      label: string,
      supply?: CostLine[],
    ): void;
    fillIngredients(
      list: HTMLElement,
      ingredients: {
        item_id: number;
        quantity: number;
      }[],
      supply?: CostLine[],
    ): void;
    renderCardRecipes(
      container: HTMLElement,
      definition: BuildingDefinition,
    ): void;
    describeRecipe(recipe: RecipeDefinition): string;
    machinesForRecipe(recipe: RecipeDefinition): BuildingDefinition[];
    itemsMatching(query: string): Set<number>;
    recipeMatches(recipe: RecipeDefinition, query: string): boolean;
    renderRecipePanel(): void;
    renderRecipeGroup(id: string, recipes: RecipeDefinition[]): void;
    createLookupRow(key: string): HTMLElement;
    fillLookupRow(row: HTMLElement, recipe: RecipeDefinition): void;
  }
}

Runtime.prototype.renderHotbarSlots = function renderHotbarSlots(
  this: Runtime,
): void {
  const container = required<HTMLDivElement>("hotbar-slots");
  const slots = syncChildren(
    container,
    this.hotbar.map((_, slot) => String(slot)),
    () => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "hotbar-slot";
      button.innerHTML =
        '<span></span><small></small><i class="hotbar-key" aria-hidden="true"></i><b class="hotbar-clear" aria-hidden="true">×</b>';
      return button;
    },
  );
  this.hotbar.forEach((value, slot) => {
    const button = slots[slot] as HTMLButtonElement | undefined;
    if (!button) return;
    const definition =
      typeof value === "number"
        ? this.host.definitions.buildings.find(({ id }) => id === value)
        : undefined;
    const fixed =
      typeof value === "string" ? this.TOOL_LABELS[value] : undefined;
    part(button, ".hotbar-key").textContent = String(slot + 1);
    button.classList.toggle("empty", value === null);
    button.classList.toggle("drop-target", this.hotbarDragOver === slot);
    button.draggable = value !== null;
    button.dataset.slot = String(slot);
    if (value === null) {
      delete button.dataset.tool;
      button.disabled = false;
      button.removeAttribute("aria-disabled");
      button.classList.remove("active", "unaffordable", "locked");
      clearEmblem(part(button, "span")).textContent = "";
      part(button, "small").textContent = "Empty";
      button.title = `Slot ${slot + 1} is empty — pin something from the build catalogue (B)`;
      button.setAttribute("aria-label", `Hotbar slot ${slot + 1}, empty`);
      return;
    }
    button.dataset.tool = String(value);
    button.classList.toggle("active", String(value) === String(this.tool));
    if (definition) {
      const availability = buildingAvailability(
        definition,
        this.snapshot,
        this.host.definitions.items,
        this.heldOrientationFor(definition),
      );
      // A locked slot refuses the build, but it must never refuse the ×. `disabled` swallows every
      // pointer event inside the button, including the clear affordance and the drag, so a pin the
      // player made before the research existed was stuck on the bar with no gesture that could
      // take it off. `aria-disabled` says the same thing to a screen reader and leaves the button
      // reachable; the click handler is what declines to select it.
      button.disabled = false;
      button.setAttribute("aria-disabled", String(availability.locked));
      button.classList.toggle("unaffordable", !availability.affordable);
      button.classList.toggle("locked", availability.locked);
      this.paintBuildingEmblem(part(button, "span"), definition);
      part(button, "small").textContent = definition.name;
      button.title = availability.locked
        ? `${definition.name} — locked by research`
        : `${definition.name} · ${availability.costLabel}`;
      button.setAttribute(
        "aria-label",
        `Hotbar slot ${slot + 1}: build ${definition.name}`,
      );
      return;
    }
    button.disabled = false;
    button.removeAttribute("aria-disabled");
    button.classList.remove("unaffordable", "locked");
    // A pinned tool keeps its text glyph rather than borrowing a machine emblem: a mode you enter
    // and a machine you place are different kinds of thing, and the bar should say which is which.
    clearEmblem(part(button, "span")).textContent = fixed?.icon ?? "?";
    part(button, "small").textContent = fixed?.name ?? String(value);
    button.title = fixed?.name ?? String(value);
    button.setAttribute(
      "aria-label",
      `Hotbar slot ${slot + 1}: ${fixed?.name ?? String(value)}`,
    );
  });
};

Runtime.prototype.assignHotbarSlot = function assignHotbarSlot(
  this: Runtime,
  slot: number,
  value: Tool | null,
): void {
  if (slot < 0 || slot >= this.HOTBAR_SLOTS) return;
  if (value !== null)
    this.hotbar = this.hotbar.map((existing) =>
      String(existing) === String(value) ? null : existing,
    );
  this.hotbar[slot] = value;
  this.saveHotbar();
  this.renderHotbarSlots();
  this.renderBuildPanel();
};

Runtime.prototype.pinToHotbar = function pinToHotbar(
  this: Runtime,
  value: Tool,
): void {
  if (this.hotbar.some((existing) => String(existing) === String(value))) {
    this.showFeedback("Already on the bar");
    return;
  }
  const free = this.hotbar.indexOf(null);
  const slot = free === -1 ? this.HOTBAR_SLOTS - 1 : free;
  this.assignHotbarSlot(slot, value);
  const definition =
    typeof value === "number"
      ? this.host.definitions.buildings.find(({ id }) => id === value)
      : undefined;
  this.showFeedback(
    `${definition?.name ?? this.TOOL_LABELS[String(value)]?.name ?? "Tool"} pinned to slot ${slot + 1}`,
  );
};

Runtime.prototype.renderHotbar = function renderHotbar(this: Runtime): void {
  for (const button of this.toolShelf.querySelectorAll<HTMLButtonElement>(
    ":scope > button[data-tool]",
  ))
    button.classList.toggle(
      "active",
      (button.dataset.tool ?? "inspect") === String(this.tool),
    );
  this.renderHotbarSlots();
};

Runtime.prototype.catalogueVisible = function catalogueVisible(
  this: Runtime,
  definition: BuildingDefinition,
  reach: Map<number, number>,
): boolean {
  if (this.showAllBuildings) return true;
  const technology = definition.unlock_technology_id;
  if (technology === undefined) return true;
  return (
    (reach.get(technology) ?? Number.MAX_SAFE_INTEGER) <= this.DISCLOSURE_REACH
  );
};

Runtime.prototype.buildMatches = function buildMatches(
  this: Runtime,
  definition: BuildingDefinition,
  group: (typeof this.BUILD_GROUPS)[number],
  query: string,
): boolean {
  return `${definition.name} ${definition.description} ${group.title} ${group.blurb}`
    .toLowerCase()
    .includes(query);
};

Runtime.prototype.renderBuildScope = function renderBuildScope(
  this: Runtime,
  hidden: number,
  query: string,
): void {
  const scope = required<HTMLButtonElement>("build-scope");
  scope.hidden = query.length > 0;
  scope.textContent = this.showAllBuildings
    ? "Show what is in reach"
    : hidden > 0
      ? `Show everything (${hidden} locked)`
      : "Show everything";
  scope.setAttribute("aria-pressed", String(this.showAllBuildings));
};

Runtime.prototype.renderBuildPanel = function renderBuildPanel(
  this: Runtime,
): void {
  const root = required<HTMLDivElement>("build-groups");
  const buildable = this.host.definitions.buildings.filter(
    (definition) => definition.buildable,
  );
  const reach = this.technologyReach();
  const query = this.buildSearch.trim().toLowerCase();
  this.renderBuildScope(
    buildable.filter((definition) => !this.catalogueVisible(definition, reach))
      .length,
    query,
  );
  if (!root.childElementCount)
    for (const group of this.BUILD_GROUPS) {
      const section = document.createElement("section");
      section.className = "build-group";
      section.dataset.group = group.key;
      section.innerHTML = `<h3>${group.title}</h3><p>${group.blurb}</p><div class="build-cards"></div>`;
      root.append(section);
    }
  let shown = 0;
  for (const group of this.BUILD_GROUPS) {
    const section = root.querySelector<HTMLElement>(
      `[data-group="${group.key}"]`,
    );
    if (!section) continue;
    const definitions = buildable.filter((definition) =>
      group.holds(definition)
        ? query
          ? this.buildMatches(definition, group, query)
          : this.catalogueVisible(definition, reach)
        : false,
    );
    shown += definitions.length;
    section.hidden = definitions.length === 0;
    const cards = syncChildren(
      part<HTMLElement>(section, ".build-cards"),
      definitions.map(({ id }) => String(id)),
      this.createBuildCard,
    );
    definitions.forEach((definition, index) => {
      const card = cards[index];
      if (card) this.fillBuildCard(card, definition);
    });
  }
  // An empty catalogue that says nothing reads as a broken panel. Say which search emptied it.
  const empty = required<HTMLParagraphElement>("build-empty");
  empty.hidden = shown > 0;
  empty.textContent = query
    ? `Nothing in the catalogue matches “${this.buildSearch.trim()}”.`
    : "Nothing to build yet.";
};

Runtime.prototype.createBuildCard = function createBuildCard(
  this: Runtime,
  key: string,
): HTMLElement {
  const card = document.createElement("article");
  card.className = "build-card";
  card.draggable = true;
  card.dataset.definitionId = key;
  card.innerHTML = `
    <header>
      <i class="build-stamp"></i>
      <div class="build-card-title">
        <strong></strong>
        <span class="build-chips"></span>
      </div>
      <button type="button" class="build-pin" data-pin>Pin</button>
    </header>
    <p class="build-card-copy"></p>
    <div class="build-cost"></div>
    <div class="build-recipes"></div>`;
  return card;
};

Runtime.prototype.heldOrientationFor = function heldOrientationFor(
  this: Runtime,
  definition: BuildingDefinition,
): number {
  return definition.id === this.tool ? this.orientation : 0;
};

Runtime.prototype.paintBuildingEmblem = function paintBuildingEmblem(
  this: Runtime,
  box: HTMLElement,
  definition: BuildingDefinition,
): void {
  paintEmblem(box, {
    key: definition.key,
    markup: buildingEmblemSvg(definition.key),
    accent: BUILDING_COLORS[definition.kind] ?? "#8fd4ff",
    rank: emblemRank(definition.tier),
    text: hasBuildingEmblem(definition.key) ? undefined : definition.icon,
  });
};

Runtime.prototype.fillBuildCard = function fillBuildCard(
  this: Runtime,
  card: HTMLElement,
  definition: BuildingDefinition,
): void {
  const availability = buildingAvailability(
    definition,
    this.snapshot,
    this.host.definitions.items,
    this.heldOrientationFor(definition),
  );
  card.classList.toggle("locked", availability.locked);
  card.classList.toggle("unaffordable", !availability.affordable);
  card.classList.toggle("active", definition.id === this.tool);
  card.classList.toggle("pinned", this.hotbar.includes(definition.id));
  this.paintBuildingEmblem(part<HTMLElement>(card, ".build-stamp"), definition);
  part(card, "strong").textContent = definition.name;
  part(card, ".build-card-copy").textContent = definition.description;
  const chips = part<HTMLElement>(card, ".build-chips");
  const labels: string[] = [];
  if (availability.locked) {
    const technology = this.host.technologies.technologies.find(
      ({ id }) => id === definition.unlock_technology_id,
    );
    labels.push(`Needs ${technology?.name ?? "research"}`);
  }
  if ((definition.tier ?? 0) > 0)
    labels.push(`Tier ${(definition.tier ?? 0) + 1}`);
  if (definition.extract_radius !== undefined)
    labels.push(`Reaches ${definition.extract_radius}`);
  if (definition.supply_radius !== undefined)
    labels.push(`Supplies ${definition.supply_radius}`);
  if (definition.pole_reach !== undefined)
    labels.push(`Links ${definition.pole_reach}`);
  if (definition.capacity !== undefined)
    labels.push(`Holds ${definition.capacity}`);
  if (definition.power_output) labels.push(`+${definition.power_output} power`);
  if (definition.power_draw) labels.push(`−${definition.power_draw} power`);
  if (definition.orientation_axis === "corner") labels.push("Six corners");
  if (definition.orientation_axis === "any") labels.push("Twelve headings");
  if (definition.splits) labels.push("Fans out");
  if (definition.merges) labels.push("Takes in turn");
  if (definition.underpass_span !== undefined)
    labels.push(`Drag pair · spans ${definition.underpass_span}`);
  if (definition.transport_medium === "fluid") labels.push("Fluids only");
  if (definition.accepted_item_ids?.length === 1) {
    const item = this.itemById(definition.accepted_item_ids[0]);
    labels.push(`${item?.name ?? "Filtered"} only`);
  }
  const chipNodes = syncChildren(chips, labels, () => {
    const chip = document.createElement("span");
    chip.className = "build-chip";
    return chip;
  });
  labels.forEach((label, index) => {
    const node = chipNodes[index];
    if (node) node.textContent = label;
  });
  this.renderIngredientRow(
    part<HTMLElement>(card, ".build-cost"),
    // The bill at the heading being quoted, so the row and the availability beside it can never
    // describe two different prices.
    costAt(definition, this.heldOrientationFor(definition)),
    "Costs",
    availability.cost,
  );
  this.renderCardRecipes(part<HTMLElement>(card, ".build-recipes"), definition);
};

Runtime.prototype.renderIngredientRow = function renderIngredientRow(
  this: Runtime,
  container: HTMLElement,
  ingredients: {
    item_id: number;
    quantity: number;
  }[],
  label: string,
  supply?: CostLine[],
): void {
  container.hidden = ingredients.length === 0;
  if (!container.childElementCount) {
    const caption = document.createElement("span");
    caption.className = "ingredient-label";
    const list = document.createElement("span");
    list.className = "ingredient-list";
    container.append(caption, list);
  }
  part(container, ".ingredient-label").textContent = label;
  this.fillIngredients(
    part<HTMLElement>(container, ".ingredient-list"),
    ingredients,
    supply,
  );
};

Runtime.prototype.fillIngredients = function fillIngredients(
  this: Runtime,
  list: HTMLElement,
  ingredients: {
    item_id: number;
    quantity: number;
  }[],
  supply?: CostLine[],
): void {
  const nodes = syncChildren(
    list,
    ingredients.map(({ item_id }) => String(item_id)),
    () => {
      const entry = document.createElement("span");
      entry.className = "ingredient";
      return entry;
    },
  );
  ingredients.forEach(({ item_id, quantity }, index) => {
    const node = nodes[index];
    if (!node) return;
    const line = supply?.[index];
    this.paintChip(node, item_id, {
      count: line ? undefined : quantity,
      progress: line ? { have: line.held, need: line.required } : undefined,
      shortfall: line?.shortfall,
      short: true,
    });
  });
};

Runtime.prototype.renderCardRecipes = function renderCardRecipes(
  this: Runtime,
  container: HTMLElement,
  definition: BuildingDefinition,
): void {
  const recipes = this.recipeChoices(definition);
  container.hidden = recipes.length === 0;
  const rows = syncChildren(
    container,
    recipes.map(({ id }) => String(id)),
    () => {
      const row = document.createElement("button");
      row.type = "button";
      row.className = "recipe-row";
      row.innerHTML =
        '<i class="recipe-emblem"></i><span class="ingredient-list recipe-in"></span><i class="recipe-arrow" aria-hidden="true">→</i><span class="ingredient-list recipe-out"></span><small class="recipe-meta"></small>';
      return row;
    },
  );
  const chosen = this.recipeFor(definition.id);
  recipes.forEach((recipe, index) => {
    const row = rows[index];
    if (!row) return;
    row.dataset.definitionId = String(definition.id);
    row.dataset.recipeId = String(recipe.id);
    row.classList.toggle("chosen", recipe.id === chosen);
    paintEmblem(part<HTMLElement>(row, ".recipe-emblem"), {
      key: recipe.category,
      accent: recipeCategoryAccent(recipe.category),
      markup: recipeCategoryEmblemSvg(recipe.category),
    });
    // Inputs are a quantity the player may be expected to supply — early machines are hand-fed
    // through Put long before a belt reaches them — so they are priced against the pack. The
    // output is a result and is only ever an amount.
    this.fillIngredients(
      part<HTMLElement>(row, ".recipe-in"),
      recipe.inputs,
      costLines(recipe.inputs, this.snapshot),
    );
    this.fillIngredients(
      part<HTMLElement>(row, ".recipe-out"),
      recipeOutputs(recipe),
    );
    const meta = [
      `${recipe.duration * (definition.duration_multiplier ?? 1)} ticks`,
    ];
    if (definition.manual_work) meta.push("player work");
    if (recipe.fuel) meta.push(`${recipe.fuel} fuel`);
    part(row, ".recipe-meta").textContent = meta.join(" · ");
    row.setAttribute(
      "aria-label",
      `${recipe.name}: ${this.describeRecipe(recipe)}. ${meta.join(", ")}`,
    );
  });
};

Runtime.prototype.describeRecipe = function describeRecipe(
  this: Runtime,
  recipe: RecipeDefinition,
): string {
  const name = (item_id: number): string =>
    this.host.definitions.items.find(({ id }) => id === item_id)?.name ??
    `item ${item_id}`;
  const inputs = recipe.inputs
    .map(({ item_id, quantity }) => `${quantity} ${name(item_id)}`)
    .join(" and ");
  return `${inputs} makes ${recipeOutputs(recipe)
    .map(({ item_id, quantity }) => `${quantity} ${name(item_id)}`)
    .join(" and ")}`;
};

Runtime.prototype.machinesForRecipe = function machinesForRecipe(
  this: Runtime,
  recipe: RecipeDefinition,
): BuildingDefinition[] {
  return this.host.definitions.buildings.filter(
    (definition) => definition.buildable && supportsRecipe(definition, recipe),
  );
};

Runtime.prototype.itemsMatching = function itemsMatching(
  this: Runtime,
  query: string,
): Set<number> {
  const called = this.host.definitions.items.filter((item) =>
    `${item.name} ${item.key}`.toLowerCase().includes(query),
  );
  // A description is prose, and prose names other materials: the component's blurb mentions the
  // gear it is built from, which would file "Compose component" under Made by for a search for
  // gears. So the blurb is read only when nothing is actually called that, where its reach is the
  // difference between an oblique answer and an empty panel.
  const matched =
    called.length > 0
      ? called
      : this.host.definitions.items.filter((item) =>
          item.description.toLowerCase().includes(query),
        );
  return new Set(matched.map(({ id }) => id));
};

Runtime.prototype.recipeMatches = function recipeMatches(
  this: Runtime,
  recipe: RecipeDefinition,
  query: string,
): boolean {
  return `${recipe.name} ${recipe.description} ${recipe.category} ${this.machinesForRecipe(
    recipe,
  )
    .map(({ name }) => name)
    .join(" ")}`
    .toLowerCase()
    .includes(query);
};

Runtime.prototype.renderRecipePanel = function renderRecipePanel(
  this: Runtime,
): void {
  const query = this.recipeSearch.trim().toLowerCase();
  const recipes = this.host.definitions.recipes;
  const named = query ? this.itemsMatching(query) : new Set<number>();
  const makes = recipes.filter((recipe) =>
    recipeOutputs(recipe).some(({ item_id }) => named.has(item_id)),
  );
  const uses = recipes.filter((recipe) =>
    recipe.inputs.some(({ item_id }) => named.has(item_id)),
  );
  // A recipe already answered on one side is not repeated as a weaker match on the other.
  const claimed = new Set([...makes, ...uses].map(({ id }) => id));
  const rest = query
    ? recipes.filter(
        (recipe) =>
          !claimed.has(recipe.id) && this.recipeMatches(recipe, query),
      )
    : recipes;
  this.renderRecipeGroup("recipe-makes", makes);
  this.renderRecipeGroup("recipe-uses", uses);
  this.renderRecipeGroup("recipe-rest", rest);
  required<HTMLElement>("recipe-rest-title").textContent = query
    ? "Other matches"
    : "Every recipe";
  const empty = required<HTMLParagraphElement>("recipe-empty");
  empty.hidden = makes.length + uses.length + rest.length > 0;
  empty.textContent = `Nothing makes or uses “${this.recipeSearch.trim()}”.`;
};

Runtime.prototype.renderRecipeGroup = function renderRecipeGroup(
  this: Runtime,
  id: string,
  recipes: RecipeDefinition[],
): void {
  const section = required<HTMLElement>(id);
  section.hidden = recipes.length === 0;
  const rows = syncChildren(
    part<HTMLElement>(section, ".recipe-list"),
    recipes.map(({ id: recipe }) => String(recipe)),
    this.createLookupRow,
  );
  recipes.forEach((recipe, index) => {
    const row = rows[index];
    if (row) this.fillLookupRow(row, recipe);
  });
};

Runtime.prototype.createLookupRow = function createLookupRow(
  this: Runtime,
  key: string,
): HTMLElement {
  const row = document.createElement("button");
  row.type = "button";
  row.className = "recipe-row lookup-row";
  row.dataset.recipeId = key;
  row.innerHTML =
    '<i class="recipe-emblem"></i><strong class="lookup-name"></strong><small class="recipe-meta"></small><span class="lookup-flow"><span class="ingredient-list lookup-in"></span><i class="recipe-arrow" aria-hidden="true">→</i><span class="ingredient-list lookup-out"></span></span><small class="lookup-machines"></small>';
  return row;
};

Runtime.prototype.fillLookupRow = function fillLookupRow(
  this: Runtime,
  row: HTMLElement,
  recipe: RecipeDefinition,
): void {
  paintEmblem(part<HTMLElement>(row, ".recipe-emblem"), {
    key: recipe.category,
    accent: recipeCategoryAccent(recipe.category),
    markup: recipeCategoryEmblemSvg(recipe.category),
  });
  part(row, ".lookup-name").textContent = recipe.name;
  // Plain amounts on both sides: this is a reference, not a bill the player is being asked to pay.
  this.fillIngredients(part<HTMLElement>(row, ".lookup-in"), recipe.inputs);
  this.fillIngredients(
    part<HTMLElement>(row, ".lookup-out"),
    recipeOutputs(recipe),
  );
  part<HTMLElement>(row, ".recipe-arrow").hidden = recipe.inputs.length === 0;
  const meta = [`${recipe.duration} ticks`];
  if (recipe.fuel) meta.push(`${recipe.fuel} fuel`);
  part(row, ".recipe-meta").textContent = meta.join(" · ");
  // The row builds the first machine research has actually reached, so clicking it hands over
  // something placeable rather than a refusal. The list still names every machine that runs it.
  const machines = this.machinesForRecipe(recipe);
  const reachable = machines.find(
    (definition) =>
      !buildingAvailability(
        definition,
        this.snapshot,
        this.host.definitions.items,
      ).locked,
  );
  row.dataset.definitionId = String(reachable?.id ?? machines[0]?.id ?? "");
  row.classList.toggle("locked", reachable === undefined);
  part(row, ".lookup-machines").textContent = machines.length
    ? `Runs on ${machines.map(({ name }) => name).join(" · ")}`
    : "No machine runs this yet";
  row.title = recipe.description;
  row.setAttribute(
    "aria-label",
    `${recipe.name}: ${this.describeRecipe(recipe)}. ${meta.join(", ")}`,
  );
};
