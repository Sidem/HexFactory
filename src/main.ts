import {
  axialToPixel,
  rotateHexDirection,
  type HexDirection,
} from "@hexlife/embed/hex";

import {
  buildingAvailability,
  technologyAvailability,
} from "./core/availability";
import { FactoryHost } from "./core/FactoryHost";
import { BoundedInputQueue, MOVEMENT_KEYS, movementIntent } from "./core/input";
import { TERRAIN_INFO, TERRAIN_ORDER, terrainAccess } from "./core/terrain";
import type {
  BuildingDefinition,
  EntitySnapshot,
  FactorySnapshot,
  NativeInputCommand,
  PlacementPreview,
  RecipeDefinition,
  WorldPoint,
} from "./core/types";
import {
  BUILDING_COLORS,
  CanvasFactoryRenderer,
  isSurveyed,
} from "./rendering/CanvasFactoryRenderer";
import { itemIconSvg } from "./rendering/icons";
import { findLandingHub, homeBearing } from "./rendering/landmarks";
import { MinimapRenderer } from "./rendering/MinimapRenderer";
import "./styles.css";

type Tool = "inspect" | "erase" | "rotate" | number;

const SAVE_KEY = "hexfactory:hxf1:v6";
const DIRECTION_NAMES = [
  "East",
  "Southeast",
  "Southwest",
  "West",
  "Northwest",
  "Northeast",
];
const FOG_FILL = "#18242f";
const FOG_STROKE = "#7fe0c0";
const STATUS_TONE: Record<string, "live" | "wait" | "stop" | "hub"> = {
  extracting: "live",
  composing: "live",
  pumping: "live",
  generating: "live",
  carrying: "live",
  receiving: "live",
  idle: "wait",
  "waiting for inputs": "wait",
  buffered: "wait",
  "output blocked": "stop",
  "deposit depleted": "stop",
  "out of fuel": "stop",
  "no power": "stop",
  brownout: "stop",
  "no water in reach": "stop",
  "no boiler": "stop",
  "landing hub": "hub",
};
/**
 * Which key opens which panel. `I` and `O` are the pack and the research tree, `P` is the objective
 * and the controls reference, and the inspector is deliberately absent: it is the one panel that
 * stays on the world rather than waiting behind a key.
 */
const PANEL_KEYS: Record<string, string> = {
  KeyI: "inventory-panel",
  KeyO: "research-panel",
  KeyP: "quest-panel",
};
/**
 * A refusal the world itself already shows. The cooldown ring around the player says the wait is
 * running, so repeating it as a message strip toast on every frame of a held harvest is noise.
 */
const SILENT_EVENTS = new Set(["action cooling down"]);
const canvas = required<HTMLCanvasElement>("factory-canvas");
const playButton = required<HTMLButtonElement>("play");
const speedInput = required<HTMLSelectElement>("speed");
const scenarioInput = required<HTMLSelectElement>("scenario");
const seedInput = required<HTMLInputElement>("seed");
const toolShelf = required<HTMLDivElement>("tool-shelf");
const feedback = required<HTMLDivElement>("feedback");
const input = new BoundedInputQueue();
const host = await FactoryHost.create();
const renderer = new CanvasFactoryRenderer(canvas, host.definitions);
const minimap = new MinimapRenderer(
  required<HTMLCanvasElement>("minimap"),
  host.definitions,
);

let snapshot = host.snapshot();
let playing = true;
let tool: Tool = "inspect";
let orientation: HexDirection = 0;
let selected: { q: number; r: number } | null = null;
let hover: { q: number; r: number } | null = null;
let hoverPreview: PlacementPreview | null = null;
let accumulator = 0;
/**
 * Real time owed to the player's own cadence. The factory's accumulator is scaled by the speed
 * setting and stops while paused; this one is not and does not, because everything the player does
 * themselves runs at one rate whatever the factory is doing. A player who is neither walking nor
 * waiting out an action accrues nothing, so an idle frame still costs no worker round trip.
 */
let playerAccumulator = 0;
let previousTime = performance.now();
let feedbackTimer = 0;
let lastEvent = "";
let panPointer: { id: number; x: number; y: number; moved: boolean } | null =
  null;
let suppressMapClick = false;
/**
 * The in-progress construction or removal drag. Only the two endpoints are ever held here — the
 * path between them belongs to native, which resolves it for both the preview and the command.
 */
let dragBuild: {
  id: number;
  from: { q: number; r: number };
  to: { q: number; r: number };
  erasing: boolean;
} | null = null;
let dragPreviewPending = false;
let gatherHeld = false;
/**
 * Which recipe each machine definition is currently set to build with, so a choice survives
 * switching tools. Presentation state: native still validates the category on every placement.
 */
const selectedRecipes = new Map<number, number>();
/** The inspected machine and assignment the recipe select was last built for. */
let inspectorRecipeKey = "";
const pressedMovement = new Set<string>();
/**
 * Where the pointer last was, in client coordinates, while it was over the world. The aim is
 * recomputed from it every frame rather than only on pointer movement, because a stationary cursor
 * and a walking player is a changing bearing. A touch layout leaves this null and never aims, so
 * there the walk direction still decides which way the player faces.
 */
let aimPointer: { x: number; y: number } | null = null;
/** The whole-degree bearing the last aim was sent for, so a frame that changes nothing sends nothing. */
let aimDegrees: number | null = null;
/**
 * Where the landing hub stands, cached per world rather than rescanned every frame: the hub does
 * not move, and the scan is over every entity in the factory.
 */
let landingHub: WorldPoint | null = null;
let landingHubWorld = "";
let advancePending = false;
let previewPending = false;
let previewRequested = false;
let previewRevision = 0;

for (const definition of host.definitions.buildings.filter(
  ({ buildable }) => buildable,
)) {
  const button = document.createElement("button");
  button.type = "button";
  button.dataset.tool = String(definition.id);
  button.setAttribute("aria-label", `Build ${definition.name}`);
  button.innerHTML = `<span>${definition.icon}</span><small>${definition.name}</small>`;
  toolShelf.append(button);
}

function update(next: FactorySnapshot): void {
  const previousVictory = snapshot.victory;
  snapshot = next;
  refreshLandingHub();
  renderer.setHome(landingHub);
  renderer.setSnapshot(snapshot);
  minimap.setSnapshot(snapshot, landingHub);
  renderHomeReadout();
  required<HTMLElement>("scenario-value").textContent = snapshot.scenario_name;
  required<HTMLElement>("tick-value").textContent =
    snapshot.tick.toLocaleString();
  required<HTMLElement>("insight-value").textContent =
    snapshot.insight.toLocaleString();
  required<HTMLElement>("objective-value").textContent =
    snapshot.scenario === "factory-demo"
      ? "LIVE"
      : `${snapshot.objective.delivered} / ${snapshot.objective.required}`;
  required<HTMLElement>("position-value").textContent =
    `${(snapshot.player.x / 1024).toFixed(1)}, ${(snapshot.player.y / 1024).toFixed(1)}`;
  required<HTMLElement>("surveyed-value").textContent =
    snapshot.chunks.length.toLocaleString();
  required<HTMLElement>("checksum-value").textContent = snapshot.checksum
    .toString(16)
    .padStart(8, "0")
    .toUpperCase();
  renderInventory();
  renderHotbar();
  renderTechnologies();
  renderInspector();
  renderObjective();
  renderNextAction();
  const latestEvent = snapshot.events.at(-1) ?? "";
  if (
    latestEvent &&
    latestEvent !== lastEvent &&
    !SILENT_EVENTS.has(latestEvent)
  )
    showFeedback(latestEvent);
  lastEvent = latestEvent;
  const victory = required<HTMLDivElement>("victory");
  victory.hidden = !snapshot.victory;
  if (!previousVictory && snapshot.victory)
    showFeedback("Landing objective complete — free play continues");
}

/**
 * The landing hub's world position, recomputed only when the world itself changes. A new game with
 * a different seed is a different world even under the same scenario key, so both are in the key.
 */
function refreshLandingHub(): void {
  const key = `${snapshot.scenario}:${snapshot.seed}`;
  if (key === landingHubWorld) return;
  landingHubWorld = key;
  landingHub = findLandingHub(snapshot);
}

/**
 * The way home, in words. The minimap answers this while home is on it and the marker on the edge
 * of the view answers it at any distance; this is the same answer for a screen reader, and it is
 * the reason the minimap is allowed to be a picture.
 */
function renderHomeReadout(): void {
  const element = required<HTMLElement>("home-readout");
  if (!landingHub) {
    element.textContent = "No landing hub in this world";
    return;
  }
  const bearing = homeBearing(snapshot.player, landingHub);
  element.textContent = bearing
    ? `Landing hub · ${bearing.hexes} hex ${DIRECTION_NAMES[bearing.direction]}`
    : "Landing hub · you are here";
}

/**
 * Reconcile a keyed list of children in place, reusing the element already rendered for each key.
 *
 * Rebuilding a list with `replaceChildren` on every snapshot update destroys the element the
 * pointer is on between pointerdown and pointerup. The browser then retargets the click to the
 * container, a delegated `closest()` finds nothing, and the action is silently dropped — which is
 * exactly why research clicks went nowhere. Any list that carries a control must be patched, not
 * rebuilt.
 */
function syncChildren(
  container: HTMLElement,
  keys: string[],
  create: (key: string) => HTMLElement,
): HTMLElement[] {
  const existing = new Map<string, HTMLElement>();
  for (const child of Array.from(container.children)) {
    const element = child as HTMLElement;
    const key = element.dataset.key;
    if (key !== undefined && !existing.has(key)) existing.set(key, element);
    else element.remove();
  }
  const ordered = keys.map((key) => {
    const reused = existing.get(key);
    if (reused) {
      existing.delete(key);
      return reused;
    }
    const created = create(key);
    created.dataset.key = key;
    return created;
  });
  for (const stale of existing.values()) stale.remove();
  ordered.forEach((element, index) => {
    if (container.children[index] !== element)
      container.insertBefore(element, container.children[index] ?? null);
  });
  return ordered;
}

/**
 * The cargo pack as a slot grid. Native resolves which stacks the player is carrying — the host
 * lays them out and pads the remainder with empty slots, so the stacking rule is not written twice.
 */
function renderInventory(): void {
  const element = required<HTMLDivElement>("inventory");
  const stacks = snapshot.player.carry_stacks;
  const slots = Math.max(snapshot.player.carry_slots, stacks.length);
  const cells = syncChildren(
    element,
    Array.from({ length: slots }, (_, index) => `slot-${index}`),
    () => {
      const cell = document.createElement("div");
      cell.className = "inventory-slot";
      cell.setAttribute("role", "listitem");
      cell.innerHTML = `<span class="swatch"></span><small></small><strong></strong>`;
      return cell;
    },
  );
  cells.forEach((cell, index) => {
    const stack = stacks[index];
    const item = stack
      ? host.definitions.items.find(({ id }) => id === stack.item_id)
      : undefined;
    cell.classList.toggle("filled", Boolean(stack));
    cell.style.setProperty("--item-color", item?.color ?? "transparent");
    const icon = part(cell, "small");
    icon.innerHTML = item && stack ? itemIconSvg(item.icon, item.color) : "";
    part(cell, "strong").textContent = stack ? String(stack.quantity) : "";
    cell.setAttribute(
      "aria-label",
      item && stack
        ? `${item.name}: ${stack.quantity} of ${item.stack_size}`
        : "Empty carrying slot",
    );
  });
  required<HTMLElement>("carry-value").textContent =
    `${stacks.length} / ${snapshot.player.carry_slots}`;
  required<HTMLElement>("carry-detail").textContent =
    `${stacks.length} of ${snapshot.player.carry_slots} slots carried.`;
}

function renderHotbar(): void {
  for (const button of toolShelf.querySelectorAll<HTMLButtonElement>(
    "button[data-tool]",
  )) {
    const value = button.dataset.tool ?? "inspect";
    button.classList.toggle("active", value === String(tool));
    if (!/^\d+$/.test(value)) continue;
    const definition = host.definitions.buildings.find(
      ({ id }) => id === Number(value),
    );
    if (!definition) continue;
    const availability = buildingAvailability(
      definition,
      snapshot,
      host.definitions.items,
    );
    button.disabled = availability.locked;
    button.classList.toggle("unaffordable", !availability.affordable);
    button.classList.toggle("locked", availability.locked);
    // Text, not `innerHTML`: replacing the inner nodes on every snapshot detaches whichever one
    // the pointer went down on, and the delegated click then resolves to nothing.
    part(button, "span").textContent = availability.locked
      ? "◇"
      : definition.icon;
    part(button, "small").textContent = availability.locked
      ? definition.name
      : `${definition.name} · ${availability.costLabel}`;
    button.title = `${definition.description} ${availability.costLabel}`;
  }
}

/**
 * The research tree, patched in place — see {@link syncChildren} for why rebuilding it lost clicks.
 *
 * Every entry now says what it unlocks, what it costs, and, when it is unavailable, which of the
 * two reasons applies and how far off it is. A locked technology that explains nothing is the same
 * defect as a control that needs explaining.
 */
function renderTechnologies(): void {
  const list = required<HTMLDivElement>("technology-list");
  const technologies = host.technologies.technologies;
  const buttons = syncChildren(
    list,
    technologies.map(({ id }) => String(id)),
    () => {
      const button = document.createElement("button");
      button.type = "button";
      button.innerHTML =
        '<strong></strong><span class="technology-detail"></span><span class="technology-unlocks"></span><small></small>';
      return button;
    },
  );
  technologies.forEach((technology, index) => {
    const button = buttons[index] as HTMLButtonElement;
    const state = technologyAvailability(technology, snapshot);
    const missing = technology.prerequisites
      .filter((id) => !snapshot.researched.includes(id))
      .map(
        (id) => technologies.find((value) => value.id === id)?.name ?? `#${id}`,
      );
    const unlocks = technology.unlocks
      .map(
        (id) =>
          host.definitions.buildings.find((value) => value.id === id)?.name ??
          `#${id}`,
      )
      .join(", ");
    const status = state.complete
      ? "Researched"
      : missing.length
        ? `Needs ${missing.join(" and ")}`
        : state.affordable
          ? `Research for ${technology.cost} insight`
          : `${technology.cost} insight · you have ${snapshot.insight}`;
    button.dataset.technologyId = String(technology.id);
    button.disabled =
      state.complete || !state.prerequisitesMet || !state.affordable;
    button.className = state.complete
      ? "complete"
      : state.prerequisitesMet && state.affordable
        ? "available"
        : "";
    part(button, "strong").textContent = technology.name;
    part(button, ".technology-detail").textContent = technology.description;
    part(button, ".technology-unlocks").textContent = unlocks
      ? `Unlocks ${unlocks}`
      : "";
    part(button, "small").textContent = status;
    button.setAttribute("aria-label", `${technology.name}. ${status}.`);
    button.title = unlocks
      ? `${technology.description} Unlocks ${unlocks}.`
      : technology.description;
  });
}

/**
 * The take-from-container controls for the inspected hex, patched in place for the same reason the
 * research list is. `quantity` is the whole stored amount: native clamps it to what the container
 * holds and to what the player can still carry, and reports how much actually moved.
 */
function paintHexFace(
  hex: HTMLElement,
  fill: string,
  stroke: string,
  impassable: boolean,
): void {
  hex.style.setProperty("--band-fill", fill);
  hex.style.setProperty("--band-stroke", stroke);
  hex.classList.toggle("impassable", impassable);
}

function setMeter(
  row: HTMLElement,
  fill: HTMLElement,
  amount: HTMLElement,
  current: number,
  total: number,
  visible: boolean,
): void {
  row.hidden = !visible;
  if (!visible) return;
  const ratio = total > 0 ? Math.min(1, Math.max(0, current / total)) : 0;
  fill.style.width = `${ratio * 100}%`;
  amount.textContent = `${current} / ${total}`;
}

function setItemGlyph(
  element: HTMLElement,
  icon: string | undefined,
  color: string | undefined,
): void {
  element.style.setProperty("--item-color", color ?? "transparent");
  element.innerHTML = icon && color ? itemIconSvg(icon, color) : "";
}

/**
 * The take-from-container controls for the inspected hex, patched in place for the same reason the
 * research list is. `quantity` is the whole stored amount: native clamps it to what the container
 * holds and to what the player can still carry, and reports how much actually moved. A composer
 * still shows its reserved inputs, but only a container grows a Take — reserved inputs belong to
 * the job that reserved them.
 */
function renderInspectorActions(building: EntitySnapshot | undefined): void {
  const list = required<HTMLDivElement>("inspector-actions");
  const stock = required<HTMLElement>("inspect-stock");
  const stored = building?.inventory ?? [];
  const canTake = building?.kind === "container";
  stock.hidden = stored.length === 0;
  const rows = syncChildren(
    list,
    stored.map(({ item_id }) => String(item_id)),
    () => {
      const row = document.createElement("div");
      row.className = "inspect-stock-row";
      row.innerHTML = `<div class="inspect-stock-item"><span class="inspect-item-glyph"></span><strong></strong><span></span></div>`;
      const button = document.createElement("button");
      button.type = "button";
      button.className = "withdraw-button";
      row.append(button);
      return row;
    },
  );
  stored.forEach((entry, index) => {
    const row = rows[index];
    if (!row) return;
    const item = host.definitions.items.find(({ id }) => id === entry.item_id);
    const name = item?.name ?? `Item ${entry.item_id}`;
    setItemGlyph(part(row, ".inspect-item-glyph"), item?.icon, item?.color);
    part(row, "strong").textContent = name;
    part(row, ".inspect-stock-item > span:last-child").textContent = String(
      entry.quantity,
    );
    const button = part<HTMLButtonElement>(row, "button");
    button.hidden = !canTake;
    button.dataset.itemId = String(entry.item_id);
    button.dataset.quantity = String(entry.quantity);
    button.dataset.q = String(building?.q ?? 0);
    button.dataset.r = String(building?.r ?? 0);
    button.textContent = "Take";
    button.setAttribute("aria-label", `Take ${entry.quantity} ${name}`);
  });
}

function renderInspector(): void {
  const empty = required<HTMLElement>("inspect-empty");
  const sheet = required<HTMLElement>("inspect-sheet");
  const kicker = required<HTMLElement>("inspect-kicker");
  const title = required<HTMLElement>("inspect-title");
  const status = required<HTMLElement>("inspect-status");
  if (!selected) {
    empty.hidden = false;
    sheet.hidden = true;
    kicker.textContent = "World inspector";
    title.textContent = "Select a hex";
    status.hidden = true;
    renderInspectorActions(undefined);
    renderInspectorRecipe(undefined);
    return;
  }
  const building = snapshot.buildings.find(({ footprint }) =>
    footprint.some(({ q, r }) => q === selected?.q && r === selected?.r),
  );
  const selectedWorld = axialToPixel(selected, 1024, { x: 0, y: 0 });
  // Field cells are addressed by their tile key, exactly as the native patch addresses them.
  const resource = snapshot.resources.find(
    ({ q, r }) => q === selected?.q && r === selected?.r,
  );
  const surveyed = isSurveyed(snapshot.chunks, selectedWorld);
  const definition = building
    ? host.definitions.buildings.find(({ id }) => id === building.definition_id)
    : undefined;
  const fieldItem = resource
    ? host.definitions.items.find(({ id }) => id === resource.item_id)
    : undefined;

  empty.hidden = true;
  sheet.hidden = false;
  required<HTMLElement>("inspect-q").textContent = String(selected.q);
  required<HTMLElement>("inspect-r").textContent = String(selected.r);

  const field = required<HTMLElement>("inspect-field");
  // The actual field is what a new player needs first. Band potentials belong on empty
  // ground; listing them above an iron cell is how the purple hex stayed anonymous.
  if (resource) {
    field.hidden = false;
    field.classList.toggle("inspect-field-solo", !building);
    field.style.setProperty("--item-color", fieldItem?.color ?? "transparent");
    setItemGlyph(
      required<HTMLElement>("inspect-field-glyph"),
      fieldItem?.icon,
      fieldItem?.color,
    );
    required<HTMLElement>("inspect-field-name").textContent =
      fieldItem?.name ?? "Resource";
    setMeter(
      required<HTMLElement>("inspect-field-meter"),
      required<HTMLElement>("inspect-field-fill"),
      required<HTMLElement>("inspect-field-amount"),
      resource.quantity,
      resource.initial_quantity,
      true,
    );
  } else {
    field.hidden = true;
  }

  const terrain = surveyed
    ? (snapshot.terrain.find(
        ({ q, r }) => q === selected?.q && r === selected?.r,
      )?.terrain ?? "lowland")
    : undefined;
  const band = terrain ? TERRAIN_INFO[terrain] : undefined;

  if (building) {
    kicker.textContent = "Building";
    title.textContent = definition?.name ?? titleCase(building.kind);
    status.hidden = false;
    status.textContent = building.status;
    status.className = `inspect-status ${STATUS_TONE[building.status] ?? "wait"}`;
  } else if (resource) {
    kicker.textContent = "Field";
    title.textContent = fieldItem?.name ?? "Resource";
    if (fieldItem?.regrowth_ticks) {
      status.hidden = false;
      status.textContent = "regrows";
      status.className = "inspect-status live";
    } else {
      status.hidden = true;
    }
  } else if (!surveyed) {
    kicker.textContent = "Unsurveyed";
    title.textContent = "Fog";
    status.hidden = true;
  } else {
    kicker.textContent = "Ground";
    title.textContent = band?.name ?? "Lowland";
    status.hidden = true;
  }

  const hex = required<HTMLElement>("inspect-hex");
  const mark = required<HTMLElement>("inspect-portrait-mark");
  const facingTick = required<HTMLElement>("inspect-facing-tick");
  if (building) {
    paintHexFace(hex, BUILDING_COLORS[building.kind], "#dce7ef", false);
    mark.textContent = definition?.icon ?? "";
    facingTick.hidden = false;
    facingTick.className = `inspect-facing-tick dir-${building.orientation}`;
  } else if (resource && fieldItem) {
    paintHexFace(hex, fieldItem.color, "#f4f7f5", false);
    setItemGlyph(mark, fieldItem.icon, fieldItem.color);
    facingTick.hidden = true;
  } else if (band) {
    paintHexFace(hex, band.fill, band.stroke, !band.passable);
    mark.textContent = "";
    facingTick.hidden = true;
  } else {
    paintHexFace(hex, FOG_FILL, FOG_STROKE, false);
    mark.textContent = "";
    facingTick.hidden = true;
  }

  const place = required<HTMLElement>("inspect-place");
  if (surveyed && band) {
    place.hidden = false;
    paintHexFace(
      required<HTMLElement>("inspect-band-swatch"),
      band.fill,
      band.stroke,
      !band.passable,
    );
    required<HTMLElement>("inspect-band-name").textContent = band.name;
    const access = required<HTMLElement>("inspect-access");
    access.textContent = terrainAccess(band);
    access.classList.toggle("impassable-label", !band.passable);
    required<HTMLElement>("inspect-band-note").textContent =
      resource || building ? "" : band.note;
  } else if (!surveyed) {
    place.hidden = false;
    paintHexFace(
      required<HTMLElement>("inspect-band-swatch"),
      FOG_FILL,
      FOG_STROKE,
      false,
    );
    required<HTMLElement>("inspect-band-name").textContent = "Unsurveyed";
    const access = required<HTMLElement>("inspect-access");
    access.textContent = "Fog";
    access.classList.remove("impassable-label");
    required<HTMLElement>("inspect-band-note").textContent =
      "Travel here to lift the fog";
  } else {
    place.hidden = true;
  }

  const machine = required<HTMLElement>("inspect-machine");
  machine.hidden = !building;
  if (building) {
    setMeter(
      required<HTMLElement>("inspect-progress-meter"),
      required<HTMLElement>("inspect-progress-fill"),
      required<HTMLElement>("inspect-progress-amount"),
      building.progress,
      building.progress_total,
      building.progress_total > 0,
    );
    setMeter(
      required<HTMLElement>("inspect-fuel-meter"),
      required<HTMLElement>("inspect-fuel-fill"),
      required<HTMLElement>("inspect-fuel-amount"),
      building.fuel_charge ?? 0,
      building.fuel_required ?? 0,
      Boolean(building.fuel_required),
    );
    setMeter(
      required<HTMLElement>("inspect-power-meter"),
      required<HTMLElement>("inspect-power-fill"),
      required<HTMLElement>("inspect-power-amount"),
      building.power_satisfied ?? 0,
      building.power_demand ?? 0,
      Boolean(building.power_demand),
    );
    for (const spoke of required<HTMLElement>("inspect-compass").children) {
      const tick = spoke as HTMLElement;
      tick.classList.toggle(
        "on",
        Number(tick.dataset.dir) === building.orientation,
      );
    }
    required<HTMLElement>("inspect-facing-name").textContent =
      DIRECTION_NAMES[building.orientation] ?? `Facing ${building.orientation}`;
    required<HTMLElement>("inspect-protected").hidden =
      !building.scenario_owned;
    const cargo = required<HTMLElement>("inspect-cargo");
    const cargoItem = building.cargo
      ? host.definitions.items.find(({ id }) => id === building.cargo?.item_id)
      : undefined;
    cargo.hidden = !building.cargo;
    if (building.cargo) {
      setItemGlyph(
        required<HTMLElement>("inspect-cargo-glyph"),
        cargoItem?.icon,
        cargoItem?.color,
      );
      required<HTMLElement>("inspect-cargo-name").textContent =
        cargoItem?.name ?? `Item ${building.cargo.item_id}`;
      required<HTMLElement>("inspect-cargo-count").textContent = String(
        building.cargo.quantity,
      );
    }
  }

  renderInspectorActions(building);
  renderInspectorRecipe(building);
}

/** The recipes a machine definition may be assigned, in catalog order. */
function recipeChoices(
  definition: BuildingDefinition | undefined,
): RecipeDefinition[] {
  if (!definition?.recipe_category) return [];
  return host.definitions.recipes.filter(
    ({ category }) => category === definition.recipe_category,
  );
}

function fillRecipeOptions(
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
}

/**
 * The recipe of the machine under the inspector, changeable between crafts. Rebuilt only when the
 * inspected machine or its assignment actually changes: patching a `<select>` on every snapshot
 * would fight a player who has the list open, which is the same defect the research panel had in a
 * different shape.
 */
function renderInspectorRecipe(building: EntitySnapshot | undefined): void {
  const wrapper = required<HTMLElement>("inspector-recipe");
  const select = required<HTMLSelectElement>("machine-recipe");
  const definition = building
    ? host.definitions.buildings.find(({ id }) => id === building.definition_id)
    : undefined;
  const choices = recipeChoices(definition);
  // Nothing to choose between is not a choice worth showing.
  wrapper.hidden = !building || building.scenario_owned || choices.length < 2;
  if (wrapper.hidden || !building) {
    inspectorRecipeKey = "";
    return;
  }
  const key = `${building.id}:${building.recipe_id ?? 0}`;
  if (key === inspectorRecipeKey) return;
  inspectorRecipeKey = key;
  fillRecipeOptions(select, choices);
  select.value = String(building.recipe_id ?? choices[0]?.id ?? "");
  select.dataset.q = String(building.q);
  select.dataset.r = String(building.r);
}

/** The recipe the pending machine will be built with. */
function renderRecipePicker(): void {
  const wrapper = required<HTMLElement>("recipe-picker");
  const select = required<HTMLSelectElement>("recipe");
  const definition =
    typeof tool === "number"
      ? host.definitions.buildings.find(({ id }) => id === tool)
      : undefined;
  const choices = recipeChoices(definition);
  wrapper.hidden = choices.length === 0;
  if (!choices.length) return;
  fillRecipeOptions(select, choices);
  select.value = String(recipeFor(tool) ?? choices[0]?.id ?? "");
}

function renderObjective(): void {
  const item = host.definitions.items.find(
    ({ id }) => id === snapshot.objective.item_id,
  );
  const detail = snapshot.victory
    ? `Complete: ${snapshot.objective.delivered} ${item?.name ?? "items"} delivered. Continue building freely.`
    : `Deliver ${snapshot.objective.required} ${item?.name ?? "items"} to the landing hub. Progress: ${snapshot.objective.delivered}.`;
  // The same sentence in both places it belongs: the completion banner and the objective panel.
  required<HTMLElement>("objective-detail").textContent = detail;
  required<HTMLElement>("quest-detail").textContent = detail;
  const progress = Math.min(
    100,
    (snapshot.objective.delivered / Math.max(1, snapshot.objective.required)) *
      100,
  );
  required<HTMLElement>("mission-progress-fill").style.width =
    snapshot.scenario === "factory-demo" ? "100%" : `${progress}%`;
  required<HTMLElement>("mission-title").textContent = snapshot.victory
    ? "Landing directive complete — free build enabled"
    : snapshot.scenario === "factory-demo"
      ? "Observe the compiled production line"
      : "Establish component production";
}

function renderNextAction(): void {
  const ore = snapshot.player.inventory["1"] ?? 0;
  const components = snapshot.player.inventory["2"] ?? 0;
  const crystals = snapshot.player.inventory["3"] ?? 0;
  const researched = new Set(snapshot.researched);
  let title = "Survey the landing zone";
  let detail =
    "The hatched fog is unsurveyed world. Walk toward it to reveal terrain, then gather from an ore or crystal field.";
  if (snapshot.victory) {
    title = "Factory online";
    detail =
      "The landing directive is complete. Expand, optimize, or inspect the running line.";
  } else if (snapshot.scenario === "factory-demo") {
    title = "Trace the material flow";
    detail =
      "Follow cargo from extractor to receiver. Pause or single-step to inspect arbitration.";
  } else if (
    snapshot.player.carry_stacks.length >= snapshot.player.carry_slots
  ) {
    // A full pack blocks gathering and recovery both, so it outranks whatever came next.
    title = "Your pack is full";
    detail =
      "Deliver at the landing hub, or build a container and take stacks back out of it from the inspector.";
  } else if (!researched.has(1) && snapshot.insight >= 3) {
    title = "Unlock Field Logistics";
    detail =
      "You have enough insight. Research Field Logistics to add belts to the construction dock.";
  } else if (!researched.has(1) && ore + crystals === 0) {
    title = "Gather your first material";
    detail =
      "Walk onto an ore or crystal field, then gather. Hover a field to read its name and remaining amount.";
  } else if (!researched.has(1)) {
    title = "Deliver materials for insight";
    detail =
      "Return to the gold landing hub and deliver your cargo. Three ore fund the first breakthrough.";
  } else if (!researched.has(2) && snapshot.insight >= 5) {
    title = "Automate extraction";
    detail =
      "Research Automated Extraction, then place an extractor on a field hex. It harvests every cell within one step.";
  } else if (!researched.has(2)) {
    title = "Fund Automated Extraction";
    detail =
      "Gather and deliver more raw material until you have five insight.";
  } else if (!researched.has(3) && snapshot.insight >= 8) {
    title = "Unlock Composition";
    detail =
      "Research Composition to unlock the two-hex composer and the final production path.";
  } else if (!researched.has(3)) {
    title = "Build the supply line";
    detail =
      "Use extractors and directional belts to automate deliveries and earn eight insight.";
  } else if (components > 0) {
    title = "Deliver completed components";
    detail =
      "Bring components to the landing hub and deliver them to finish the directive.";
  } else if (!researched.has(5) && snapshot.insight >= 6) {
    title = "Unlock Material Processing";
    detail =
      "Research it for the smelter and the kiln. A kiln chars wood into fuel, and that fuel is what a smelter burns to make plate.";
  } else if (!researched.has(5)) {
    title = "Compose three components";
    detail =
      "Route ore into a composer, point its output toward the hub, and keep the line supplied. Six insight also unlocks smelting.";
  } else {
    title = "Build the material base";
    detail =
      "Terrain is the material map: iron and coal in highland, copper in hills, sand and clay on shores, stone at cliffs, wood in moist lowland. Belt fuel into a smelter and it burns whatever arrives.";
  }
  required<HTMLElement>("next-action-title").textContent = title;
  required<HTMLElement>("next-action-detail").textContent = detail;
}

function showFeedback(message: string): void {
  if (!message) return;
  feedback.textContent = message;
  feedback.classList.add("visible");
  window.clearTimeout(feedbackTimer);
  feedbackTimer = window.setTimeout(
    () => feedback.classList.remove("visible"),
    2200,
  );
}

function setPlaying(value: boolean): void {
  playing = value;
  playButton.textContent = playing ? "Ⅱ" : "▶";
  playButton.setAttribute("aria-pressed", String(playing));
  playButton.setAttribute(
    "aria-label",
    playing ? "Pause simulation" : "Play simulation",
  );
  playButton.title = playing ? "Pause simulation (T)" : "Play simulation (T)";
}

function syncSessionInputs(next: FactorySnapshot): void {
  scenarioInput.value = next.scenario;
  seedInput.value = String(next.seed);
}

function selectTool(next: Tool): void {
  tool = next;
  const definition =
    typeof next === "number"
      ? host.definitions.buildings.find(({ id }) => id === next)
      : undefined;
  renderer.setBuildMode(next !== "inspect");
  renderer.setBuildFootprint(
    definition?.footprint ?? [{ q: 0, r: 0 }],
    orientation,
  );
  renderRecipePicker();
  renderHotbar();
  refreshHoverPreview();
}

function enqueue(command: NativeInputCommand): void {
  if (!input.enqueue(command))
    showFeedback(
      "Input queue full; command deferred by the bounded host limit",
    );
}

function refreshHoverPreview(): void {
  previewRevision += 1;
  previewRequested = true;
  if (!previewPending) void flushHoverPreview();
}

async function flushHoverPreview(): Promise<void> {
  previewPending = true;
  while (previewRequested) {
    previewRequested = false;
    const revision = previewRevision;
    const coordinate = hover;
    const definitionId = typeof tool === "number" ? tool : null;
    const direction = orientation;
    if (!coordinate || definitionId === null) {
      hoverPreview = null;
    } else {
      try {
        const result = await host.placementPreview(
          coordinate.q,
          coordinate.r,
          definitionId,
          direction,
          recipeFor(definitionId),
        );
        if (revision === previewRevision) hoverPreview = result;
      } catch (error) {
        if (revision === previewRevision)
          showFeedback(`Placement preview failed: ${String(error)}`);
      }
    }
    if (revision === previewRevision) {
      renderer.setHover(hover, hoverPreview);
      required<HTMLElement>("placement-value").textContent =
        hoverPreview?.reason ?? "";
    }
  }
  previewPending = false;
}

playButton.addEventListener("click", () => setPlaying(!playing));
required<HTMLButtonElement>("step").addEventListener("click", () => {
  setPlaying(false);
  void host.tick(1).then(update).catch(reportWorkerError);
});
required<HTMLButtonElement>("reset").addEventListener("click", () => {
  input.clear();
  void host
    .reset()
    .then((next) => {
      update(next);
      renderer.recenter();
    })
    .catch(reportWorkerError);
});
required<HTMLButtonElement>("turn").addEventListener(
  "click",
  rotateNewBuilding,
);
// The dock's gather and deliver carry `data-native-action`, so they are wired here and only here:
// a second listener bound to the same button by id sent the command twice.
for (const button of document.querySelectorAll<HTMLButtonElement>(
  "[data-native-action]",
)) {
  button.addEventListener("click", () => {
    const type = button.dataset.nativeAction;
    if (type === "gather" || type === "deposit") enqueue({ type });
  });
}
required<HTMLButtonElement>("recenter").addEventListener("click", () =>
  renderer.recenter(),
);
required<HTMLButtonElement>("toggle-grid").addEventListener(
  "click",
  (event) => {
    const visible = renderer.toggleGrid();
    const button = event.currentTarget as HTMLButtonElement;
    button.setAttribute("aria-pressed", String(visible));
    button.setAttribute(
      "aria-label",
      visible ? "Hide construction grid" : "Show construction grid",
    );
    button.title = visible
      ? "Hide construction grid"
      : "Show construction grid";
  },
);
required<HTMLButtonElement>("new-game").addEventListener("click", async () => {
  input.clear();
  const parsedSeed = Number(seedInput.value);
  const seed =
    Number.isSafeInteger(parsedSeed) &&
    parsedSeed >= 0 &&
    parsedSeed <= 0xffffffff
      ? parsedSeed
      : undefined;
  try {
    const next = await host.newGame(scenarioInput.value, seed);
    update(next);
    syncSessionInputs(next);
    renderer.recenter();
    setPlaying(true);
    closePanels();
  } catch (error) {
    reportWorkerError(error);
  }
});
required<HTMLButtonElement>("save").addEventListener("click", async () => {
  try {
    localStorage.setItem(SAVE_KEY, await host.save());
    updateContinueState("HXF1 save stored locally.");
    showFeedback("Game saved");
  } catch (error) {
    updateContinueState(`Save failed: ${String(error)}`);
  }
});
required<HTMLButtonElement>("continue").addEventListener("click", async () => {
  const save = localStorage.getItem(SAVE_KEY);
  if (!save) return;
  try {
    input.clear();
    const next = await host.load(save);
    update(next);
    syncSessionInputs(next);
    renderer.recenter();
    showFeedback("Native HXF1 save restored");
    closePanels();
  } catch (error) {
    updateContinueState(`Continue rejected: ${String(error)}`);
  }
});

toolShelf.addEventListener("click", (event) => {
  const button = (event.target as Element).closest<HTMLButtonElement>(
    "button[data-tool]",
  );
  if (!button || button.disabled) return;
  const value = button.dataset.tool ?? "inspect";
  selectTool(/^\d+$/.test(value) ? Number(value) : (value as Tool));
});
required<HTMLDivElement>("technology-list").addEventListener(
  "click",
  (event) => {
    const button = (event.target as Element).closest<HTMLButtonElement>(
      "button[data-technology-id]",
    );
    if (!button || button.disabled) return;
    enqueue({
      type: "research",
      technology_id: Number(button.dataset.technologyId),
    });
  },
);
required<HTMLSelectElement>("recipe").addEventListener("change", (event) => {
  const select = event.currentTarget as HTMLSelectElement;
  if (typeof tool !== "number") return;
  selectedRecipes.set(tool, Number(select.value));
  refreshHoverPreview();
});
required<HTMLSelectElement>("machine-recipe").addEventListener(
  "change",
  (event) => {
    const select = event.currentTarget as HTMLSelectElement;
    enqueue({
      type: "set_recipe",
      q: Number(select.dataset.q),
      r: Number(select.dataset.r),
      recipe_id: Number(select.value),
    });
  },
);
required<HTMLDivElement>("inspector-actions").addEventListener(
  "click",
  (event) => {
    const button = (event.target as Element).closest<HTMLButtonElement>(
      "button[data-item-id]",
    );
    if (!button) return;
    enqueue({
      type: "withdraw",
      q: Number(button.dataset.q),
      r: Number(button.dataset.r),
      item_id: Number(button.dataset.itemId),
      quantity: Number(button.dataset.quantity),
    });
  },
);

window.addEventListener("keydown", (event) => {
  if (isTypingTarget(event.target)) return;
  // Undo is the one binding that keeps its modifier, because every other application uses it.
  if ((event.ctrlKey || event.metaKey) && event.code === "KeyZ") {
    event.preventDefault();
    enqueue({ type: "undo" });
    return;
  }
  if (event.ctrlKey || event.metaKey || event.altKey) return;
  if (event.code in MOVEMENT_KEYS) {
    event.preventDefault();
    if (!pressedMovement.has(event.code)) {
      pressedMovement.add(event.code);
      enqueue(movementIntent(pressedMovement));
    }
    return;
  }
  if (event.code === "Escape") {
    selectTool("inspect");
    closePanels();
  }
  // Space centres the camera, which is what the button beside it does and what a player who has
  // panned away needs most. Pause moved to T rather than fighting it for the key.
  else if (event.code === "Space") renderer.recenter();
  else if (event.code === "KeyT") setPlaying(!playing);
  else if (event.code in PANEL_KEYS)
    togglePanel(PANEL_KEYS[event.code] as string);
  else if (event.code === "KeyF") {
    // Held rather than tapped. The native action cooldown already paces this, so the repeat
    // cannot outrun the simulation.
    gatherHeld = true;
    enqueue({ type: "gather" });
  } else if (event.code === "KeyX") enqueue({ type: "deposit" });
  else if (event.code === "KeyR") rotateUnderCursorOrPending();
  else if (event.code === "KeyQ") pickToolUnderCursor();
  else if (event.code === "KeyE") selectTool("erase");
  else if (/^Digit[1-9]$/.test(event.code)) {
    const buildable = host.definitions.buildings.filter(
      ({ buildable }) => buildable,
    );
    const definition = buildable[Number(event.code.at(-1)) - 1];
    if (definition) selectTool(definition.id);
  } else return;
  event.preventDefault();
});

window.addEventListener("keyup", (event) => {
  if (event.code === "KeyF") gatherHeld = false;
  if (!pressedMovement.delete(event.code)) return;
  event.preventDefault();
  // Stopping is sent on the same frame the key comes up. Coalescing the release made every stop
  // read as a slide, which is the kind of latency a player feels without being able to name it.
  enqueue(movementIntent(pressedMovement));
});

window.addEventListener("blur", () => {
  gatherHeld = false;
  stopAiming();
  if (!pressedMovement.size) return;
  pressedMovement.clear();
  enqueue(movementIntent(pressedMovement));
});

canvas.addEventListener("pointermove", (event) => {
  // Aiming survives panning and dragging: the player keeps facing the pointer whatever else the
  // pointer is doing. Touch never aims, because a finger that is not on the glass points nowhere.
  if (event.pointerType !== "touch")
    aimPointer = { x: event.clientX, y: event.clientY };
  if (panPointer?.id === event.pointerId) {
    const dx = event.clientX - panPointer.x;
    const dy = event.clientY - panPointer.y;
    if (Math.abs(dx) + Math.abs(dy) > 1) panPointer.moved = true;
    renderer.panBy(dx, dy);
    panPointer.x = event.clientX;
    panPointer.y = event.clientY;
    return;
  }
  const coordinate = renderer.pick(event.clientX, event.clientY);
  if (dragBuild?.id === event.pointerId) {
    if (coordinate.q === dragBuild.to.q && coordinate.r === dragBuild.to.r)
      return;
    dragBuild.to = coordinate;
    void refreshDragPreview();
    return;
  }
  hover = coordinate;
  refreshHoverPreview();
});
canvas.addEventListener("pointerdown", (event) => {
  if (event.button === 1 || event.button === 2 || event.shiftKey) {
    panPointer = {
      id: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      moved: false,
    };
    canvas.setPointerCapture(event.pointerId);
    event.preventDefault();
    return;
  }
  if (event.button !== 0 || !draggableTool()) return;
  const from = renderer.pick(event.clientX, event.clientY);
  dragBuild = {
    id: event.pointerId,
    from,
    to: from,
    erasing: tool === "erase",
  };
  canvas.setPointerCapture(event.pointerId);
  void refreshDragPreview();
});
canvas.addEventListener("pointerup", (event) => {
  if (panPointer?.id === event.pointerId) {
    suppressMapClick = panPointer.moved;
    canvas.releasePointerCapture(event.pointerId);
    panPointer = null;
    return;
  }
  if (dragBuild?.id !== event.pointerId) return;
  const { from, to, erasing } = dragBuild;
  endDrag(event.pointerId);
  // A drag that never left its starting hex is an ordinary click; the click handler runs it.
  if (from.q === to.q && from.r === to.r) return;
  suppressMapClick = true;
  selected = to;
  renderer.setSelection(to);
  enqueue(
    erasing
      ? { type: "erase_line", q: from.q, r: from.r, to_q: to.q, to_r: to.r }
      : {
          type: "place_line",
          q: from.q,
          r: from.r,
          to_q: to.q,
          to_r: to.r,
          definition_id: tool as number,
          orientation,
          recipe_id: recipeFor(tool),
        },
  );
});
canvas.addEventListener("pointercancel", (event) => endDrag(event.pointerId));
canvas.addEventListener("pointerleave", () => {
  stopAiming();
  if (!panPointer && !dragBuild) {
    hover = null;
    refreshHoverPreview();
  }
});
canvas.addEventListener("contextmenu", (event) => event.preventDefault());
canvas.addEventListener(
  "wheel",
  (event) => {
    event.preventDefault();
    renderer.zoomAt(
      event.clientX,
      event.clientY,
      event.deltaY < 0 ? 1.12 : 0.89,
    );
  },
  { passive: false },
);
canvas.addEventListener("click", (event) => {
  if (suppressMapClick) {
    suppressMapClick = false;
    return;
  }
  const coordinate = renderer.pick(event.clientX, event.clientY);
  selected = coordinate;
  renderer.setSelection(coordinate);
  if (tool === "erase") enqueue({ type: "erase", ...coordinate });
  else if (tool === "rotate") enqueue({ type: "rotate", ...coordinate });
  else if (typeof tool === "number") {
    enqueue({
      type: "place",
      ...coordinate,
      definition_id: tool,
      orientation,
      recipe_id: recipeFor(tool),
    });
  }
  renderInspector();
});

/**
 * Whether the selected tool can be dragged into a run. Erasure always can; construction can when
 * the definition occupies a single hex, because a run of multi-hex footprints would overlap itself.
 */
function draggableTool(): boolean {
  if (tool === "erase") return true;
  if (typeof tool !== "number") return false;
  const definition = host.definitions.buildings.find(({ id }) => id === tool);
  return definition?.footprint.length === 1;
}

/**
 * The recipe a placement of this tool carries. It is whichever of the definition's own category
 * the player chose, defaulting to the first — never simply "the first recipe in the catalog",
 * which since the material base would hand a kiln a smelting job that native then refuses.
 */
function recipeFor(value: Tool): number | undefined {
  const definition =
    typeof value === "number"
      ? host.definitions.buildings.find(({ id }) => id === value)
      : undefined;
  const choices = recipeChoices(definition);
  if (!choices.length || !definition) return undefined;
  const chosen = selectedRecipes.get(definition.id);
  return chosen !== undefined && choices.some(({ id }) => id === chosen)
    ? chosen
    : choices[0]?.id;
}

/**
 * Ask native what the current drag would do and hand the answer straight to the renderer. The host
 * never resolves the path itself, so the preview and the eventual command cannot disagree.
 */
async function refreshDragPreview(): Promise<void> {
  if (!dragBuild || dragPreviewPending) return;
  dragPreviewPending = true;
  try {
    while (dragBuild) {
      const { from, to, erasing } = dragBuild;
      const cells = await host.linePreview(
        from.q,
        from.r,
        to.q,
        to.r,
        erasing ? undefined : (tool as number),
        orientation,
        erasing ? undefined : recipeFor(tool),
      );
      if (!dragBuild) break;
      renderer.setDragPath(cells);
      const legal = cells.filter((cell) => cell.legal).length;
      required<HTMLElement>("placement-value").textContent = erasing
        ? `Remove ${legal} of ${cells.length}`
        : `Build ${legal} of ${cells.length}`;
      if (dragBuild.to.q === to.q && dragBuild.to.r === to.r) break;
    }
  } catch (error) {
    showFeedback(`Drag preview failed: ${String(error)}`);
  } finally {
    dragPreviewPending = false;
  }
}

function endDrag(pointerId: number): void {
  if (dragBuild?.id !== pointerId) return;
  dragBuild = null;
  renderer.setDragPath([]);
  if (canvas.hasPointerCapture(pointerId))
    canvas.releasePointerCapture(pointerId);
  required<HTMLElement>("placement-value").textContent =
    hoverPreview?.reason ?? "";
}

/**
 * One rotation idea, not two: with a build tool held this turns the pending building, and otherwise
 * it turns the building under the cursor.
 */
function rotateUnderCursorOrPending(): void {
  if (typeof tool === "number" || tool === "inspect") {
    const target = hover ?? selected;
    const existing =
      typeof tool === "number"
        ? null
        : target &&
          snapshot.buildings.find(({ footprint }) =>
            footprint.some(({ q, r }) => q === target.q && r === target.r),
          );
    if (existing && target) {
      enqueue({ type: "rotate", q: target.q, r: target.r });
      return;
    }
  }
  rotateNewBuilding();
}

/**
 * Adopt whatever is under the cursor as the active tool, orientation included, so repeating an
 * existing building never means hunting for it in the dock.
 */
function pickToolUnderCursor(): void {
  const target = hover ?? selected;
  const building =
    target &&
    snapshot.buildings.find(({ footprint }) =>
      footprint.some(({ q, r }) => q === target.q && r === target.r),
    );
  if (!building) {
    showFeedback("Nothing under the cursor to copy");
    return;
  }
  const definition = host.definitions.buildings.find(
    ({ id }) => id === building.definition_id,
  );
  if (!definition?.buildable) {
    showFeedback(`${definition?.name ?? "That"} cannot be built`);
    return;
  }
  setOrientation(building.orientation as HexDirection);
  selectTool(definition.id);
  showFeedback(`Copied ${definition.name}`);
}

function setOrientation(next: HexDirection): void {
  orientation = next;
  required<HTMLElement>("orientation-value").textContent =
    `${DIRECTION_NAMES[orientation]} · R`;
  const definition =
    typeof tool === "number"
      ? host.definitions.buildings.find(({ id }) => id === tool)
      : undefined;
  renderer.setBuildFootprint(
    definition?.footprint ?? [{ q: 0, r: 0 }],
    orientation,
  );
  refreshHoverPreview();
}

function rotateNewBuilding(): void {
  setOrientation(rotateHexDirection(orientation, 1));
}

function stopAiming(): void {
  aimPointer = null;
  aimDegrees = null;
}

/**
 * At most one aim per frame, and only when the bearing has actually moved.
 *
 * The command carries the world point under the cursor and native resolves the facing vector from
 * it, so the host names a target and never a heading — facing is a checksum input. It is recomputed
 * every frame rather than only on pointer movement because a stationary cursor and a walking player
 * is a changing bearing, and the whole-degree threshold is what stops that costing a worker round
 * trip per frame while the player stands still.
 *
 * It is enqueued last, immediately before the batch is drained, which is what makes an aim outrank
 * the walk direction that `move_intent` also writes.
 */
function sendAim(): void {
  if (!aimPointer) return;
  const target = renderer.pickWorld(aimPointer.x, aimPointer.y);
  const dx = target.x - snapshot.player.x;
  const dy = target.y - snapshot.player.y;
  if (dx === 0 && dy === 0) return;
  const degrees = Math.round((Math.atan2(dy, dx) * 180) / Math.PI);
  if (degrees === aimDegrees) return;
  // A full queue leaves the bearing unrecorded, so the next frame tries again.
  if (input.enqueue({ type: "aim", x: target.x, y: target.y }))
    aimDegrees = degrees;
}

/** Open one panel, closing whichever other one was open, or close it if it already was. */
function togglePanel(id: string): void {
  const target = document.getElementById(id);
  if (!target) return;
  const opening = !target.classList.contains("open");
  closePanels(target);
  target.classList.toggle("open", opening);
  syncPanelToggles();
}

function syncPanelToggles(): void {
  for (const toggle of document.querySelectorAll<HTMLButtonElement>(
    ".panel-toggle",
  )) {
    const target = document.getElementById(toggle.dataset.panelTarget ?? "");
    toggle.setAttribute(
      "aria-expanded",
      String(target?.classList.contains("open") ?? false),
    );
  }
}

/**
 * The band legend. It leads with the category rather than the colour — hatched swatches are the
 * ground the player cannot stand on — and both halves come from the same table the renderer draws
 * from, so the legend cannot describe a world the map is not drawing.
 */
function renderTerrainLegend(): void {
  const element = required<HTMLDivElement>("terrain-legend");
  for (const terrain of TERRAIN_ORDER) {
    const band = TERRAIN_INFO[terrain];
    const row = document.createElement("div");
    row.setAttribute("role", "listitem");
    const swatch = document.createElement("i");
    swatch.style.setProperty("--band-fill", band.fill);
    swatch.style.setProperty("--band-stroke", band.stroke);
    if (!band.passable) swatch.className = "impassable";
    const name = document.createElement("span");
    name.textContent = band.name;
    const access = document.createElement("small");
    access.textContent = terrainAccess(band);
    if (!band.passable) access.className = "impassable-label";
    row.append(swatch, name, access);
    element.append(row);
  }
}

function frame(now: number): void {
  const elapsed = Math.min(250, now - previousTime);
  previousTime = now;
  if (playing) accumulator += elapsed * Number(speedInput.value);
  // Walking is paced by native's cadence against elapsed real time, not by the tick the factory
  // happens to be running, so a paused or slowed factory no longer pins the player in place. The
  // same clock counts down the cooldown between one field action and the next, so it has to keep
  // running for a player who is standing still waiting to gather again.
  if (pressedMovement.size || snapshot.player.action_cooldown > 0)
    playerAccumulator += elapsed * host.playerTicksPerSecond;
  else playerAccumulator = 0;
  if (!advancePending) {
    // A held gather repeats at frame rate and is paced natively by the action cooldown, so the
    // player holds the key instead of tapping it once per unit.
    if (gatherHeld && !input.size) input.enqueue({ type: "gather" });
    // Last into the batch, so the cursor outranks the walk direction for this frame's facing.
    sendAim();
    const commands = input.drain();
    const ticks = playing ? Math.min(20, Math.floor(accumulator / 1000)) : 0;
    const playerSteps = Math.min(20, Math.floor(playerAccumulator / 1000));
    if (commands.length || ticks > 0 || playerSteps > 0) {
      accumulator -= ticks * 1000;
      playerAccumulator -= playerSteps * 1000;
      advancePending = true;
      void host
        .advance(commands, ticks, playerSteps)
        .then(update)
        .catch(reportWorkerError)
        .finally(() => {
          advancePending = false;
        });
    }
  }
  renderer.renderFrame(now);
  requestAnimationFrame(frame);
}

function updateContinueState(message?: string): void {
  const hasSave = localStorage.getItem(SAVE_KEY)?.startsWith("HXF1\n") ?? false;
  required<HTMLButtonElement>("continue").disabled = !hasSave;
  required<HTMLElement>("save-status").textContent =
    message ??
    (hasSave ? "A compatible local save is available." : "No local save yet.");
}

function isTypingTarget(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLSelectElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLButtonElement ||
    target instanceof HTMLAnchorElement
  );
}

function titleCase(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

function reportWorkerError(error: unknown): void {
  setPlaying(false);
  showFeedback(`Simulation worker error: ${String(error)}`);
}

function closePanels(except?: HTMLElement): void {
  for (const panel of document.querySelectorAll<HTMLElement>(
    ".glass-panel.open",
  )) {
    if (panel === except) continue;
    panel.classList.remove("open");
  }
  syncPanelToggles();
}

for (const toggle of document.querySelectorAll<HTMLButtonElement>(
  ".panel-toggle",
)) {
  toggle.addEventListener("click", () =>
    togglePanel(toggle.dataset.panelTarget ?? ""),
  );
}

for (const close of document.querySelectorAll<HTMLButtonElement>(
  ".panel-close",
)) {
  close.addEventListener("click", () => {
    close.closest<HTMLElement>(".glass-panel")?.classList.remove("open");
    closePanels();
  });
}

for (const button of document.querySelectorAll<HTMLButtonElement>(
  "[data-move-key]",
)) {
  const code = button.dataset.moveKey ?? "";
  const start = (event: PointerEvent): void => {
    event.preventDefault();
    button.setPointerCapture(event.pointerId);
    if (pressedMovement.has(code)) return;
    pressedMovement.add(code);
    enqueue(movementIntent(pressedMovement));
  };
  const stop = (event: PointerEvent): void => {
    event.preventDefault();
    if (!pressedMovement.delete(code)) return;
    enqueue(movementIntent(pressedMovement));
  };
  button.addEventListener("pointerdown", start);
  button.addEventListener("pointerup", stop);
  button.addEventListener("pointercancel", stop);
}

function required<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) throw new Error(`Missing #${id}`);
  return element as T;
}

function part<T extends HTMLElement>(root: HTMLElement, selector: string): T {
  const element = root.querySelector<T>(selector);
  if (!element) throw new Error(`Missing ${selector}`);
  return element;
}

renderTerrainLegend();
update(snapshot);
syncSessionInputs(snapshot);
updateContinueState();
selectTool("inspect");
requestAnimationFrame(frame);

declare global {
  interface Window {
    __hexFactory?: {
      snapshot: () => FactorySnapshot;
      step: (count?: number) => Promise<FactorySnapshot>;
      reset: () => Promise<FactorySnapshot>;
      newGame: (scenario?: string, seed?: number) => Promise<FactorySnapshot>;
      save: () => Promise<string>;
      load: (save: string) => Promise<FactorySnapshot>;
    };
  }
}

window.__hexFactory = {
  snapshot: () => host.snapshot(),
  step: async (count = 1) => {
    setPlaying(false);
    const next = await host.tick(count);
    update(next);
    return next;
  },
  reset: async () => {
    const next = await host.reset();
    update(next);
    return next;
  },
  newGame: async (scenario = "new-game", seed) => {
    const next = await host.newGame(scenario, seed);
    update(next);
    syncSessionInputs(next);
    return next;
  },
  save: () => host.save(),
  load: async (save) => {
    const next = await host.load(save);
    update(next);
    syncSessionInputs(next);
    return next;
  },
};
