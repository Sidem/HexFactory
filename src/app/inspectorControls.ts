import { supportsRecipe } from "../core/definitions";
import { productionNote } from "../ui/production";
import { nextAction } from "../core/guidance";
import type {
  BuildingDefinition,
  EntitySnapshot,
  FactorySnapshot,
  NativeInputCommand,
  RecipeDefinition,
} from "../core/types";
import { part, required, syncChildren } from "../ui/dom";
import type { Tool } from "./runtime";
import { Runtime } from "./runtime";

declare module "./runtime" {
  interface Runtime {
    renderInspectorHub(building: EntitySnapshot | undefined): void;
    renderInspectorSwitch(building: EntitySnapshot | undefined): void;
    renderInspectorTier(building: EntitySnapshot | undefined): void;
    costSummary(definition: BuildingDefinition): string;
    recipeChoices(
      definition: BuildingDefinition | undefined,
    ): RecipeDefinition[];
    fillRecipeOptions(
      select: HTMLSelectElement,
      choices: RecipeDefinition[],
    ): void;
    renderInspectorRecipe(building: EntitySnapshot | undefined): void;
    renderRecipePicker(): void;
    renderContract(): void;
    renderRequests(): void;
    renderProjectCatalogue(): void;
    renderNextAction(): void;
    showFeedback(message: string): void;
    syncSessionInputs(next: FactorySnapshot): void;
    selectTool(next: Tool): void;
    enqueue(command: NativeInputCommand): boolean;
    refreshHoverPreview(): void;
  }
}

Runtime.prototype.renderInspectorHub = function renderInspectorHub(
  this: Runtime,
  building: EntitySnapshot | undefined,
): void {
  const hubCard = required<HTMLElement>("inspect-hub");
  if (building?.kind !== "hub") {
    hubCard.hidden = true;
    return;
  }
  hubCard.hidden = false;
  const contract = this.snapshot.contract;
  required<HTMLElement>("inspect-hub-contract-kicker").textContent =
    contract.complete
      ? `${contract.name} complete`
      : `${contract.name} · stage ${contract.stage + 1} of ${contract.stages}`;
  // Not the stage brief. Mission control already carries that paragraph word for word, and a new
  // player's first two panels reading identically is the duplication the v0.43 audit called out —
  // it costs a screenful and teaches that one of the two panels is redundant. What the inspector
  // can say that mission control cannot is whether the pack in the player's hands is any use here,
  // because this is the object the delivery actually happens at. The brief stays one press away.
  const wanted = contract.requirements.some(
    (need) =>
      need.required > need.delivered &&
      (this.snapshot.player.inventory[String(need.item_id)] ??
        this.snapshot.player.inventory[need.item_id] ??
        0) > 0,
  );
  const contractNote = required<HTMLElement>("inspect-hub-contract-note");
  contractNote.textContent = contract.complete
    ? "The hub project is complete. Standing requests remain open for research insight."
    : wanted
      ? "You are carrying material this stage wants. Deliver it below."
      : "Nothing in your pack fits this stage yet. Mission control (M) has the brief.";
  contractNote.title = contract.complete ? "" : contract.stage_brief;
  const contractList = required<HTMLElement>("inspect-hub-contract");
  const contractRows = syncChildren(
    contractList,
    contract.requirements.map(({ item_id }) => String(item_id)),
    () => {
      const row = document.createElement("li");
      row.className = "inspect-hub-line inspect-hub-contract-line";
      row.innerHTML = `<span class="inspect-hub-item chip-host"></span><span class="inspect-hub-purpose">Hub build</span><button type="button" class="inspect-hub-deliver inspect-hub-contract-deliver">Deliver</button>`;
      return row;
    },
  );
  contract.requirements.forEach((need, index) => {
    const row = contractRows[index];
    if (!row) return;
    const carried =
      this.snapshot.player.inventory[String(need.item_id)] ??
      this.snapshot.player.inventory[need.item_id] ??
      0;
    const remaining = Math.max(0, need.required - need.delivered);
    this.paintChip(part<HTMLElement>(row, ".inspect-hub-item"), need.item_id, {
      progress: { have: need.delivered, need: need.required },
      meter: true,
      shortfall: remaining,
    });
    const button = part<HTMLButtonElement>(row, ".inspect-hub-deliver");
    button.dataset.itemId = String(need.item_id);
    button.disabled = carried === 0 || remaining === 0;
    button.classList.toggle("ready", carried > 0 && remaining > 0);
    button.textContent =
      remaining === 0
        ? "Delivered"
        : carried > 0
          ? "Deliver"
          : `Need ${remaining}`;
    button.title =
      carried > 0
        ? "Deliver this requested material from your pack"
        : `Carry ${remaining} more to advance the hub contract`;
  });
  contractList.hidden = contract.complete || contractRows.length === 0;
  const list = required<HTMLElement>("inspect-hub-requests");
  // The snapshot carries the whole catalogue so the projects panel can browse it; the hub's own
  // panel is still just the board, which is the part you can deliver into right now.
  const requests = this.snapshot.requests.filter(
    (request) => request.state === "posted",
  );
  const rows = syncChildren(
    list,
    requests.map((request) => request.key),
    () => {
      const row = document.createElement("li");
      row.className = "inspect-hub-line";
      // No brief line. The same sentence is already printed against the same request in mission
      // control, and here it pushed the Deliver button — the only thing this panel can do that the
      // other cannot — down the list. It rides on the row's tooltip instead, so nothing is lost.
      row.innerHTML = `<span class="inspect-hub-item chip-host"></span><span class="inspect-hub-price"></span><button type="button" class="inspect-hub-deliver">Deliver</button>`;
      return row;
    },
  );
  requests.forEach((request, index) => {
    const row = rows[index];
    if (!row) return;
    const carried =
      this.snapshot.player.inventory[String(request.item_id)] ??
      this.snapshot.player.inventory[request.item_id] ??
      0;
    // What is owed is the bill less what has already been handed over — progress belongs to the
    // project now, so a row reposted after a skip asks only for the remainder.
    const owed = Math.max(0, request.required - request.delivered);
    const haveEnough = carried >= owed;
    this.paintChip(
      part<HTMLElement>(row, ".inspect-hub-item"),
      request.item_id,
      {
        progress: {
          have: Math.min(request.required, request.delivered + carried),
          need: request.required,
        },
        meter: true,
        shortfall: Math.max(0, owed - carried),
      },
    );
    part(row, ".inspect-hub-price").textContent = `+${request.insight} ◆`;
    row.title = request.brief;
    const button = part<HTMLButtonElement>(row, ".inspect-hub-deliver");
    button.dataset.itemId = String(request.item_id);
    button.disabled = !haveEnough;
    button.classList.toggle("ready", haveEnough);
    button.textContent = haveEnough ? "Complete" : `Need ${owed - carried}`;
    button.title = haveEnough
      ? `Deliver ${owed} ${request.name} to earn insight — this project pays once`
      : `You need ${owed - carried} more ${request.name} in your pack`;
  });
};

Runtime.prototype.renderInspectorSwitch = function renderInspectorSwitch(
  this: Runtime,
  building: EntitySnapshot | undefined,
): void {
  const button = required<HTMLButtonElement>("inspect-power-switch");
  // Protected objects are protected here too — native refuses, so the host does not offer.
  const switchable =
    Boolean(building) &&
    this.SWITCHABLE.has(building?.kind ?? "") &&
    !building?.scenario_owned;
  button.hidden = !switchable;
  if (!switchable || !building) return;
  const off = building.status === "switched off";
  const manual = this.host.definitions.buildings.find(
    ({ id }) => id === building.definition_id,
  )?.manual_work;
  button.dataset.q = String(building.q);
  button.dataset.r = String(building.r);
  button.dataset.enable = off ? "1" : "0";
  button.classList.toggle("is-off", off);
  button.textContent = manual
    ? off
      ? building.progress > 0
        ? "Resume work"
        : "Work one batch"
      : "Pause work"
    : off
      ? "Switch on"
      : "Switch off";
  button.title = manual
    ? "Stand within one hex and stop walking or gathering. One batch per press; pausing preserves ingredients and progress. Dismantling cancels and refunds ingredients."
    : off
      ? "Resume this machine — it keeps everything it was holding"
      : "Stop this machine without losing its stock, progress, or charge";
};

Runtime.prototype.renderInspectorTier = function renderInspectorTier(
  this: Runtime,
  building: EntitySnapshot | undefined,
): void {
  const card = required<HTMLElement>("inspect-tier");
  const chip = required<HTMLElement>("inspect-tier-chip");
  const reach = required<HTMLElement>("inspect-reach");
  const button = required<HTMLButtonElement>("inspect-upgrade");
  const definition = building
    ? this.host.definitions.buildings.find(
        ({ id }) => id === building.definition_id,
      )
    : undefined;
  const next = definition?.upgrades_to
    ? this.host.definitions.buildings.find(
        ({ id }) => id === definition.upgrades_to,
      )
    : undefined;
  const tier = definition?.tier ?? 0;
  // Nothing to say about a base-tier building with no ladder above it, and an empty ruled box is
  // exactly what the readability pass took out.
  card.hidden = !definition || (tier === 0 && !next);
  if (card.hidden) return;
  chip.textContent = `Tier ${tier + 1}`;
  // An extractor's reach and a pole's coverage are the same sentence about the same lattice, so
  // they share the line rather than each getting a row the other building leaves empty.
  const radius = definition?.extract_radius ?? definition?.supply_radius;
  reach.hidden = radius === undefined;
  if (radius !== undefined)
    reach.textContent =
      definition?.supply_radius === undefined
        ? `Reaches ${radius} ${radius === 1 ? "hex" : "hexes"}`
        : `Supplies ${radius} hexes · links ${definition.pole_reach ?? radius}`;
  button.hidden = !next || Boolean(building?.scenario_owned);
  if (!next || !building) return;
  const unlocked =
    next.unlock_technology_id === undefined ||
    this.snapshot.researched.includes(next.unlock_technology_id);
  button.disabled = !unlocked;
  button.dataset.q = String(building.q);
  button.dataset.r = String(building.r);
  button.textContent = unlocked ? `Upgrade to ${next.name}` : "Upgrade locked";
  button.setAttribute(
    "aria-label",
    unlocked
      ? `Upgrade to ${next.name} for ${this.costSummary(next)}`
      : `${next.name} is locked by research`,
  );
  button.title = unlocked ? `Costs ${this.costSummary(next)}` : "";
};

Runtime.prototype.costSummary = function costSummary(
  this: Runtime,
  definition: BuildingDefinition,
): string {
  return (
    definition.construction_cost
      .map(({ item_id, quantity }) => {
        const item = this.host.definitions.items.find(
          ({ id }) => id === item_id,
        );
        return `${quantity} ${item?.name ?? `item ${item_id}`}`;
      })
      .join(", ") || "nothing"
  );
};

Runtime.prototype.recipeChoices = function recipeChoices(
  this: Runtime,
  definition: BuildingDefinition | undefined,
): RecipeDefinition[] {
  if (!definition?.recipe_category) return [];
  return this.host.definitions.recipes.filter((recipe) =>
    supportsRecipe(definition, recipe),
  );
};

Runtime.prototype.fillRecipeOptions = function fillRecipeOptions(
  this: Runtime,
  select: HTMLSelectElement,
  choices: RecipeDefinition[],
): void {
  const options = syncChildren(
    select,
    choices.map(({ id }) => String(id)),
    () => document.createElement("option"),
  );
  choices.forEach((recipe, index) => {
    const option = options[index] as HTMLOptionElement;
    option.value = String(recipe.id);
    option.textContent = recipe.name;
    option.title = recipe.description;
  });
};

Runtime.prototype.renderInspectorRecipe = function renderInspectorRecipe(
  this: Runtime,
  building: EntitySnapshot | undefined,
): void {
  const note = required<HTMLElement>("production-note");
  const message = productionNote(building, this.host.definitions);
  note.hidden = !message;
  note.textContent = message;
  const wrapper = required<HTMLElement>("inspector-recipe");
  const select = required<HTMLSelectElement>("machine-recipe");
  const definition = building
    ? this.host.definitions.buildings.find(
        ({ id }) => id === building.definition_id,
      )
    : undefined;
  const choices = this.recipeChoices(definition);
  // Nothing to choose between is not a choice worth showing.
  wrapper.hidden = !building || building.scenario_owned || choices.length < 2;
  if (wrapper.hidden || !building) {
    this.inspectorRecipeKey = "";
    return;
  }
  const key = `${building.id}:${building.recipe_id ?? 0}`;
  if (key === this.inspectorRecipeKey) return;
  this.inspectorRecipeKey = key;
  this.fillRecipeOptions(select, choices);
  select.value = String(building.recipe_id ?? choices[0]?.id ?? "");
  select.dataset.q = String(building.q);
  select.dataset.r = String(building.r);
};

Runtime.prototype.renderRecipePicker = function renderRecipePicker(
  this: Runtime,
): void {
  const wrapper = required<HTMLElement>("recipe-picker");
  const select = required<HTMLSelectElement>("recipe");
  const definition =
    typeof this.tool === "number"
      ? this.host.definitions.buildings.find(({ id }) => id === this.tool)
      : undefined;
  const choices = this.recipeChoices(definition);
  wrapper.hidden = choices.length === 0;
  if (!choices.length) return;
  this.fillRecipeOptions(select, choices);
  select.value = String(this.recipeFor(this.tool) ?? choices[0]?.id ?? "");
};

Runtime.prototype.renderContract = function renderContract(
  this: Runtime,
): void {
  const contract = this.snapshot.contract;
  const demo = this.snapshot.scenario === "factory-demo";
  const lines = contract.requirements.map((need) => ({
    need,
    name: this.itemById(need.item_id)?.name ?? `Item ${need.item_id}`,
  }));
  const progress = contract.complete
    ? 1
    : lines.length === 0
      ? 0
      : lines.reduce(
          (total, { need }) =>
            total + need.delivered / Math.max(1, need.required),
          0,
        ) / lines.length;
  required<HTMLElement>("mission-kicker").textContent = contract.complete
    ? contract.name
    : `${contract.name} · ${contract.stage + 1} of ${contract.stages}`;
  required<HTMLElement>("mission-title").textContent = contract.complete
    ? `${contract.name} complete — free build enabled`
    : demo
      ? "Observe the compiled production line"
      : contract.stage_name;
  // The header names the thing behind the number. `0 / 3` on its own never said what the three
  // were, which is the whole complaint the playtest recorded against it.
  required<HTMLElement>("objective-value").textContent = demo
    ? "LIVE"
    : contract.complete
      ? "DONE"
      : lines
          .map(
            ({ need, name }) => `${need.delivered} / ${need.required} ${name}`,
          )
          .join(" · ");
  required<HTMLElement>("mission-progress-fill").style.width = demo
    ? "100%"
    : `${Math.min(100, progress * 100)}%`;
  document
    .querySelector<HTMLElement>(".mission-strip")
    ?.setAttribute(
      "aria-label",
      `Current mission: ${contract.complete ? contract.name : contract.stage_name}. ${required<HTMLElement>("objective-value").textContent}. Open mission control.`,
    );
  const detail = contract.complete
    ? `${contract.name} complete. The landing hub is built and the world stays open — expand, optimize, or start something larger.`
    : contract.stage_brief;
  required<HTMLElement>("objective-detail").textContent = contract.complete
    ? "Free play continues."
    : `${contract.stage_name} · ${lines.map(({ need, name }) => `${need.delivered}/${need.required} ${name}`).join(", ")}`;
  required<HTMLElement>("contract-kicker").textContent = contract.complete
    ? contract.name
    : `${contract.name} · stage ${contract.stage + 1} of ${contract.stages}`;
  required<HTMLElement>("quest-detail").textContent = detail;
  const bill = required<HTMLElement>("contract-bill");
  const rows = syncChildren(
    bill,
    lines.map(({ need }) => String(need.item_id)),
    () => {
      const row = document.createElement("li");
      row.className = "contract-line chip-host";
      return row;
    },
  );
  lines.forEach(({ need }, index) => {
    const row = rows[index];
    if (!row) return;
    // A bill is a fetch list, and a bare colour swatch is not an identity in a catalogue with
    // three greys in it. It gets the same chip everything else does, glyph and all.
    this.paintChip(row, need.item_id, {
      progress: { have: need.delivered, need: need.required },
      meter: true,
      shortfall: Math.max(0, need.required - need.delivered),
    });
  });
};

Runtime.prototype.renderRequests = function renderRequests(
  this: Runtime,
): void {
  const board = required<HTMLElement>("request-board");
  const posted = this.snapshot.requests.filter(
    (request) => request.state === "posted",
  );
  const rows = syncChildren(
    board,
    posted.map((request) => request.key),
    () => {
      const row = document.createElement("li");
      row.className = "request-line";
      row.innerHTML = `<span class="request-item chip-host"></span><span class="request-price"></span><small class="request-brief"></small>`;
      return row;
    },
  );
  posted.forEach((request, index) => {
    const row = rows[index];
    if (!row) return;
    const carried =
      this.snapshot.player.inventory[String(request.item_id)] ??
      this.snapshot.player.inventory[request.item_id] ??
      0;
    const owed = Math.max(0, request.required - request.delivered);
    // Same chip as the bill and the pack: what has been handed over plus what is in the pack,
    // against the bill — so a project part-filled before a skip does not read as untouched.
    this.paintChip(part<HTMLElement>(row, ".request-item"), request.item_id, {
      progress: {
        have: Math.min(request.required, request.delivered + carried),
        need: request.required,
      },
      meter: true,
      shortfall: Math.max(0, owed - carried),
    });
    part(row, ".request-price").textContent = `+${request.insight} ◆`;
    part(row, ".request-brief").textContent = request.brief;
  });
  required<HTMLElement>("requests-detail").hidden = rows.length === 0;
  this.renderProjectCatalogue();
};

Runtime.prototype.renderProjectCatalogue = function renderProjectCatalogue(
  this: Runtime,
): void {
  const list = required<HTMLElement>("project-catalogue-list");
  const projects = this.snapshot.requests;
  const rows = syncChildren(
    list,
    projects.map((project) => project.key),
    () => {
      const row = document.createElement("li");
      row.className = "request-line project-line";
      row.innerHTML = `<span class="request-item chip-host"></span><span class="request-price"></span><span class="project-state"></span><button type="button" class="project-post">Post</button><small class="request-brief"></small>`;
      return row;
    },
  );
  let done = 0;
  let remaining = 0;
  projects.forEach((project, index) => {
    const row = rows[index];
    if (!row) return;
    if (project.state === "complete") done += 1;
    else remaining += project.insight;
    row.dataset.state = project.state;
    this.paintChip(part<HTMLElement>(row, ".request-item"), project.item_id, {
      progress: { have: project.delivered, need: project.required },
      meter: project.delivered > 0,
    });
    part(row, ".request-price").textContent = `+${project.insight} ◆`;
    part(row, ".project-state").textContent = this.PROJECT_LABEL[project.state];
    part(row, ".request-brief").textContent = project.brief;
    const post = part<HTMLButtonElement>(row, ".project-post");
    // The snapshot names projects by key and the command takes an id, so the definitions are the
    // join. A row whose key is not in the catalogue cannot be posted rather than posting nothing.
    const definition = this.host.definitions.requests.find(
      (value) => value.key === project.key,
    );
    post.dataset.projectId =
      definition === undefined ? "" : String(definition.id);
    post.disabled = project.state !== "available" || definition === undefined;
    post.hidden = project.state === "complete";
    post.title =
      project.state === "locked"
        ? `You cannot produce ${project.name} yet`
        : `Post ${project.name} to the board`;
  });
  required<HTMLElement>("project-catalogue-count").textContent =
    `${done}/${projects.length} · ${remaining} ◆ left`;
};

Runtime.prototype.renderNextAction = function renderNextAction(
  this: Runtime,
): void {
  const guidance = nextAction(
    this.snapshot,
    this.host.definitions,
    this.host.technologies,
  );
  required<HTMLElement>("next-action-title").textContent = guidance.title;
  required<HTMLElement>("next-action-detail").textContent = guidance.detail;
  required<HTMLElement>("next-step-title").textContent = guidance.title;
  required<HTMLElement>("next-step-detail").textContent = guidance.detail;
};

Runtime.prototype.showFeedback = function showFeedback(
  this: Runtime,
  message: string,
): void {
  if (!message) return;
  this.feedback.textContent = message;
  this.feedback.classList.add("visible");
  window.clearTimeout(this.feedbackTimer);
  this.feedbackTimer = window.setTimeout(
    () => this.feedback.classList.remove("visible"),
    2200,
  );
};

Runtime.prototype.syncSessionInputs = function syncSessionInputs(
  this: Runtime,
  next: FactorySnapshot,
): void {
  this.confirmDialog.dismiss();
  this.endStackDrag();
  this.packOfferedFor = null;
  this.packDeclined = false;
  this.worldSetup.showSession(next);
};

Runtime.prototype.selectTool = function selectTool(
  this: Runtime,
  next: Tool,
): void {
  this.boundaryTool.close(false);
  this.groundTool.close(false);
  this.tool = next;
  this.renderer.setBuildMode(next !== "inspect");
  this.renderRecipePicker();
  this.renderHotbar();
  // Picking up a corner-only tool with an eastward heading held would carry an orientation the definition
  // cannot take, so the pending heading is snapped onto the new tool's axis. `setOrientation` does
  // the rest: the label, the footprint preview, and the refreshed legality all follow from it.
  const { start, end } = this.orientationRange(next);
  this.setOrientation(
    this.orientation >= start && this.orientation < end
      ? this.orientation
      : start,
  );
};

Runtime.prototype.enqueue = function enqueue(
  this: Runtime,
  command: NativeInputCommand,
): boolean {
  const accepted = this.input.enqueue(command);
  if (!accepted)
    this.showFeedback(
      "Input queue full; command deferred by the bounded host limit",
    );
  return accepted;
};

Runtime.prototype.refreshHoverPreview = function refreshHoverPreview(
  this: Runtime,
): void {
  this.previewRevision += 1;
  this.previewRequested = true;
  if (!this.previewPending) void this.flushHoverPreview();
};
