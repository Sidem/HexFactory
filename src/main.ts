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
  WorldParams,
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

type Tool = "inspect" | "erase" | "rotate" | "upgrade" | number;

/**
 * The stored save's compatibility, not just its save version.
 *
 * Native refuses a load on four numbers, not one — the save version, the world generator version,
 * the definition version, and the technology version — so all four belong in the key. v0.16
 * learned half of this: it left `SAVE_VERSION` at 7, took the generator to 6, and named both,
 * because a v7/w5 envelope is refused for naming no world parameters. v0.17 moves only the
 * definition version, which the old key did not carry at all, so a v0.16 save would have sat
 * under an unchanged key behind a Continue button that could only fail. Naming every number the
 * envelope refuses on retires an incompatible save instead of offering it.
 */
const SAVE_KEY = "hexfactory:hxf1:v7w6d8t4";
/**
 * The eight routing headings, in the core's own order. The six edges keep their indices; north and
 * south are appended, which is why every saved orientation still names the direction it always
 * did. The integer never reaches the player — this table is the only thing they read.
 */
const DIRECTION_NAMES = [
  "East",
  "Southeast",
  "Southwest",
  "West",
  "Northwest",
  "Northeast",
  "North",
  "South",
];
/** The first orientation index off the six-edge table. Matches `NORTH` in the core. */
const NORTH = 6;
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
  KeyB: "build-panel",
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
const worldPresetInput = required<HTMLSelectElement>("world-preset");
const worldPresetDescription = required<HTMLParagraphElement>(
  "world-preset-description",
);
const worldParameterFields = required<HTMLDivElement>("world-parameter-fields");
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
let orientation = 0;
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
/**
 * The button-held camera gesture, and — for the right button — the hex it is holding over.
 *
 * `harvest` is the hex a held right-click keeps working. It is fixed at the press rather than
 * tracked to the cursor, because the same gesture pans: once the pointer has genuinely travelled,
 * this is a pan and no longer a harvest. `origin` is what that travel is measured from, and it is
 * deliberately a separate, slacker threshold from `moved`. Panning wants to answer the very first
 * pixel; a hold lasting several seconds must survive the pixel or two of jitter a hand puts into a
 * held mouse button, or the harvest would cancel itself.
 */
let panPointer: {
  id: number;
  x: number;
  y: number;
  moved: boolean;
  originX: number;
  originY: number;
  harvest: { q: number; r: number } | null;
} | null = null;
/** How far a held right-click may drift, in pixels, before it becomes a pan instead. */
const HARVEST_HOLD_SLOP = 5;
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

/**
 * How the catalogue is grouped, in the order a player meets these things.
 *
 * The dock used to be every buildable definition in id order, which by v0.14 was twenty buttons of
 * three-letter stamps — a list that grows every milestone and explains nothing. The grouping is
 * derived from `kind`, so a new definition lands in the right section by being what it is; nothing
 * here is a per-building special case.
 */
const BUILD_GROUPS: {
  key: string;
  title: string;
  blurb: string;
  holds: (definition: BuildingDefinition) => boolean;
}[] = [
  {
    key: "extraction",
    title: "Extraction",
    blurb: "Take raw material out of the ground and the water.",
    holds: ({ kind }) => kind === "extractor" || kind === "pump",
  },
  {
    key: "transport",
    title: "Transport",
    blurb:
      "Move cargo. Belts run along the hex edges; risers run due north and south.",
    holds: ({ kind }) => kind === "belt",
  },
  {
    key: "processing",
    title: "Processing",
    blurb:
      "Turn one material into another. Each machine runs one category of recipe.",
    holds: ({ kind }) => kind === "composer",
  },
  {
    key: "storage",
    title: "Storage",
    blurb: "Buffer a line, and hold stock you can take back by hand.",
    holds: ({ kind }) => kind === "container",
  },
  {
    key: "power",
    title: "Power",
    blurb:
      "Make electricity and carry it. Machines draw; belts and boxes do not.",
    holds: ({ kind }) =>
      kind === "generator" || kind === "boiler" || kind === "pole",
  },
];
const HOTBAR_SLOTS = 9;
const HOTBAR_KEY = "hexfactory:hotbar:v1";
/**
 * What the bar starts with: the early game in the order it is met, then the two things a player
 * reaches for constantly once power lands. Anything else is a pin away.
 */
const DEFAULT_HOTBAR: (Tool | null)[] = [2, 1, 3, 4, 7, 8, 12, 13, 18];
/** Which slot each definition sits in, or null for an empty slot. Presentation only — never saved
 * with the game, never hashed: it is a preference about a keyboard, not a fact about a factory. */
let hotbar: (Tool | null)[] = loadHotbar();
/** The slot a drag is currently over, so the drop target can be shown before the pointer lands. */
let hotbarDragOver: number | null = null;

function loadHotbar(): (Tool | null)[] {
  const defaults = Array.from(
    { length: HOTBAR_SLOTS },
    (_, slot) => DEFAULT_HOTBAR[slot] ?? null,
  );
  try {
    const stored: unknown = JSON.parse(
      window.localStorage.getItem(HOTBAR_KEY) ?? "null",
    );
    // A stored bar is taken whole, empty slots included: a slot the player deliberately cleared
    // must not refill itself with a default on the next load.
    if (!Array.isArray(stored)) return defaults;
    return Array.from({ length: HOTBAR_SLOTS }, (_, slot) =>
      sanitiseSlot(stored[slot]),
    );
  } catch {
    // A corrupt or unreadable preference is not worth failing a boot over.
    return defaults;
  }
}

/**
 * Whatever came out of storage, reduced to something this build actually has. Definitions are
 * dynamic and a milestone can retire an id, so a stored slot naming one is dropped rather than
 * left to render as a blank button that selects nothing.
 */
function sanitiseSlot(value: unknown): Tool | null {
  if (value === "erase" || value === "rotate" || value === "upgrade")
    return value;
  if (typeof value !== "number") return null;
  const definition = host.definitions.buildings.find(({ id }) => id === value);
  return definition?.buildable ? value : null;
}

function saveHotbar(): void {
  try {
    window.localStorage.setItem(HOTBAR_KEY, JSON.stringify(hotbar));
  } catch {
    // Private-mode storage refusals must not break the bar for the session in front of us.
  }
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
  renderBuildPanel();
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

/** The label a slot or card shows for a tool that is not a building. */
const TOOL_LABELS: Record<string, { icon: string; name: string }> = {
  erase: { icon: "⌫", name: "Erase" },
  rotate: { icon: "↻", name: "Edit" },
  upgrade: { icon: "▲", name: "Upgrade" },
  inspect: { icon: "⌖", name: "Inspect" },
};

/**
 * The nine customizable slots, patched in place like every other list that carries a control.
 *
 * A slot is a button so it is reachable by keyboard and by click; the digit it answers to is drawn
 * on it, so the binding is visible rather than something to memorize. Filled slots are draggable —
 * onto another slot to move, off the bar to clear.
 */
function renderHotbarSlots(): void {
  const container = required<HTMLDivElement>("hotbar-slots");
  const slots = syncChildren(
    container,
    hotbar.map((_, slot) => String(slot)),
    () => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "hotbar-slot";
      button.innerHTML =
        '<span></span><small></small><i class="hotbar-key" aria-hidden="true"></i><b class="hotbar-clear" aria-hidden="true">×</b>';
      return button;
    },
  );
  hotbar.forEach((value, slot) => {
    const button = slots[slot] as HTMLButtonElement | undefined;
    if (!button) return;
    const definition =
      typeof value === "number"
        ? host.definitions.buildings.find(({ id }) => id === value)
        : undefined;
    const fixed = typeof value === "string" ? TOOL_LABELS[value] : undefined;
    part(button, ".hotbar-key").textContent = String(slot + 1);
    button.classList.toggle("empty", value === null);
    button.classList.toggle("drop-target", hotbarDragOver === slot);
    button.draggable = value !== null;
    button.dataset.slot = String(slot);
    if (value === null) {
      delete button.dataset.tool;
      button.disabled = false;
      button.classList.remove("active", "unaffordable", "locked");
      part(button, "span").textContent = "";
      part(button, "small").textContent = "Empty";
      button.title = `Slot ${slot + 1} is empty — pin something from the build catalogue (B)`;
      button.setAttribute("aria-label", `Hotbar slot ${slot + 1}, empty`);
      return;
    }
    button.dataset.tool = String(value);
    button.classList.toggle("active", String(value) === String(tool));
    if (definition) {
      const availability = buildingAvailability(
        definition,
        snapshot,
        host.definitions.items,
      );
      button.disabled = availability.locked;
      button.classList.toggle("unaffordable", !availability.affordable);
      button.classList.toggle("locked", availability.locked);
      part(button, "span").textContent = availability.locked
        ? "◇"
        : definition.icon;
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
    button.classList.remove("unaffordable", "locked");
    part(button, "span").textContent = fixed?.icon ?? "?";
    part(button, "small").textContent = fixed?.name ?? String(value);
    button.title = fixed?.name ?? String(value);
    button.setAttribute(
      "aria-label",
      `Hotbar slot ${slot + 1}: ${fixed?.name ?? String(value)}`,
    );
  });
}

/** Put a tool in a slot, moving it out of any slot it already occupied so it cannot be in two. */
function assignHotbarSlot(slot: number, value: Tool | null): void {
  if (slot < 0 || slot >= HOTBAR_SLOTS) return;
  if (value !== null)
    hotbar = hotbar.map((existing) =>
      String(existing) === String(value) ? null : existing,
    );
  hotbar[slot] = value;
  saveHotbar();
  renderHotbarSlots();
  renderBuildPanel();
}

/** Pin to the first free slot, or to the last one when the bar is full. */
function pinToHotbar(value: Tool): void {
  if (hotbar.some((existing) => String(existing) === String(value))) {
    showFeedback("Already on the bar");
    return;
  }
  const free = hotbar.indexOf(null);
  const slot = free === -1 ? HOTBAR_SLOTS - 1 : free;
  assignHotbarSlot(slot, value);
  const definition =
    typeof value === "number"
      ? host.definitions.buildings.find(({ id }) => id === value)
      : undefined;
  showFeedback(
    `${definition?.name ?? TOOL_LABELS[String(value)]?.name ?? "Tool"} pinned to slot ${slot + 1}`,
  );
}

/**
 * The bar: the four fixed tools, then the nine slots.
 *
 * The fixed tools are static markup and only ever change which of them is lit. Everything that
 * depends on the snapshot — cost, affordability, research locks — belongs to the slots, because
 * that is where buildings live now.
 */
function renderHotbar(): void {
  for (const button of toolShelf.querySelectorAll<HTMLButtonElement>(
    ":scope > button[data-tool]",
  ))
    button.classList.toggle(
      "active",
      (button.dataset.tool ?? "inspect") === String(tool),
    );
  renderHotbarSlots();
}

/**
 * The construction catalogue: every buildable definition, grouped by what it is for, with its cost
 * and — for a machine — every recipe it can run, written as materials rather than as a name in a
 * dropdown.
 *
 * The sections themselves are static, created once, because `BUILD_GROUPS` is constant. Only the
 * cards inside them are patched, and they are patched rather than rebuilt for the usual reason:
 * every card carries a Pin control.
 */
function renderBuildPanel(): void {
  const root = required<HTMLDivElement>("build-groups");
  if (!root.childElementCount)
    for (const group of BUILD_GROUPS) {
      const section = document.createElement("section");
      section.className = "build-group";
      section.dataset.group = group.key;
      section.innerHTML = `<h3>${group.title}</h3><p>${group.blurb}</p><div class="build-cards"></div>`;
      root.append(section);
    }
  for (const group of BUILD_GROUPS) {
    const section = root.querySelector<HTMLElement>(
      `[data-group="${group.key}"]`,
    );
    if (!section) continue;
    const definitions = host.definitions.buildings.filter(
      (definition) => definition.buildable && group.holds(definition),
    );
    section.hidden = definitions.length === 0;
    const cards = syncChildren(
      part<HTMLElement>(section, ".build-cards"),
      definitions.map(({ id }) => String(id)),
      createBuildCard,
    );
    definitions.forEach((definition, index) => {
      const card = cards[index];
      if (card) fillBuildCard(card, definition);
    });
  }
}

function createBuildCard(key: string): HTMLElement {
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
}

function fillBuildCard(
  card: HTMLElement,
  definition: BuildingDefinition,
): void {
  const availability = buildingAvailability(
    definition,
    snapshot,
    host.definitions.items,
  );
  card.classList.toggle("locked", availability.locked);
  card.classList.toggle("unaffordable", !availability.affordable);
  card.classList.toggle("active", definition.id === tool);
  card.classList.toggle("pinned", hotbar.includes(definition.id));
  const stamp = part<HTMLElement>(card, ".build-stamp");
  stamp.textContent = availability.locked ? "◇" : definition.icon;
  stamp.style.setProperty(
    "--stamp-color",
    BUILDING_COLORS[definition.kind] ?? "#8fd4ff",
  );
  part(card, "strong").textContent = definition.name;
  part(card, ".build-card-copy").textContent = definition.description;

  const chips = part<HTMLElement>(card, ".build-chips");
  const labels: string[] = [];
  if (availability.locked) {
    const technology = host.technologies.technologies.find(
      ({ id }) => id === definition.unlock_technology_id,
    );
    labels.push(`Needs ${technology?.name ?? "research"}`);
  }
  if ((definition.tier ?? 0) > 0)
    labels.push(`Tier ${(definition.tier ?? 0) + 1}`);
  if (definition.extract_radius !== undefined)
    labels.push(`Reaches ${definition.extract_radius}`);
  if (definition.capacity !== undefined)
    labels.push(`Holds ${definition.capacity}`);
  if (definition.power_output) labels.push(`+${definition.power_output} power`);
  if (definition.power_draw) labels.push(`−${definition.power_draw} power`);
  if (definition.orientation_axis === "vertical") labels.push("North / south");
  const chipNodes = syncChildren(chips, labels, () => {
    const chip = document.createElement("span");
    chip.className = "build-chip";
    return chip;
  });
  labels.forEach((label, index) => {
    const node = chipNodes[index];
    if (node) node.textContent = label;
  });

  renderIngredientRow(
    part<HTMLElement>(card, ".build-cost"),
    definition.construction_cost,
    "Costs",
  );
  renderCardRecipes(part<HTMLElement>(card, ".build-recipes"), definition);
}

/** A labelled run of item glyphs with counts — the shape every cost and every recipe side uses. */
function renderIngredientRow(
  container: HTMLElement,
  ingredients: { item_id: number; quantity: number }[],
  label: string,
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
  fillIngredients(
    part<HTMLElement>(container, ".ingredient-list"),
    ingredients,
  );
}

function fillIngredients(
  list: HTMLElement,
  ingredients: { item_id: number; quantity: number }[],
): void {
  const nodes = syncChildren(
    list,
    ingredients.map(({ item_id }) => String(item_id)),
    () => {
      const entry = document.createElement("span");
      entry.className = "ingredient";
      entry.innerHTML =
        '<span class="inspect-item-glyph"></span><b></b><small></small>';
      return entry;
    },
  );
  ingredients.forEach(({ item_id, quantity }, index) => {
    const node = nodes[index];
    if (!node) return;
    const item = host.definitions.items.find(({ id }) => id === item_id);
    setItemGlyph(part(node, ".inspect-item-glyph"), item?.icon, item?.color);
    part(node, "b").textContent = `×${quantity}`;
    part(node, "small").textContent = item?.name ?? `Item ${item_id}`;
  });
}

/**
 * Every recipe this machine can run, as materials in and materials out.
 *
 * A recipe used to be a name in a `<select>` — `Steel`, `Circuit` — which says nothing about what
 * it consumes, what it takes, or whether it burns fuel. Written as glyphs with an arrow between
 * them, the same twelve-glyph set the pack and the fields use, it is readable without being
 * learned. Clicking a row picks that recipe for the pending building, which is the choice the
 * select made and is now made where the reason for it is visible.
 */
function renderCardRecipes(
  container: HTMLElement,
  definition: BuildingDefinition,
): void {
  const recipes = recipeChoices(definition);
  container.hidden = recipes.length === 0;
  const rows = syncChildren(
    container,
    recipes.map(({ id }) => String(id)),
    () => {
      const row = document.createElement("button");
      row.type = "button";
      row.className = "recipe-row";
      row.innerHTML =
        '<span class="ingredient-list recipe-in"></span><i class="recipe-arrow" aria-hidden="true">→</i><span class="ingredient-list recipe-out"></span><small class="recipe-meta"></small>';
      return row;
    },
  );
  const chosen = recipeFor(definition.id);
  recipes.forEach((recipe, index) => {
    const row = rows[index];
    if (!row) return;
    row.dataset.definitionId = String(definition.id);
    row.dataset.recipeId = String(recipe.id);
    row.classList.toggle("chosen", recipe.id === chosen);
    fillIngredients(part<HTMLElement>(row, ".recipe-in"), recipe.inputs);
    fillIngredients(part<HTMLElement>(row, ".recipe-out"), [recipe.output]);
    const meta = [`${recipe.duration} ticks`];
    if (recipe.fuel) meta.push(`${recipe.fuel} fuel`);
    part(row, ".recipe-meta").textContent = meta.join(" · ");
    row.setAttribute(
      "aria-label",
      `${recipe.name}: ${describeRecipe(recipe)}. ${meta.join(", ")}`,
    );
  });
}

/** The same recipe in words, for a screen reader and for a tooltip. */
function describeRecipe(recipe: RecipeDefinition): string {
  const name = (item_id: number): string =>
    host.definitions.items.find(({ id }) => id === item_id)?.name ??
    `item ${item_id}`;
  const inputs = recipe.inputs
    .map(({ item_id, quantity }) => `${quantity} ${name(item_id)}`)
    .join(" and ");
  return `${inputs} makes ${recipe.output.quantity} ${name(recipe.output.item_id)}`;
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

/**
 * The put-into-container controls: one row per stack the player is carrying, so moving stock into
 * a box is the same gesture as taking it out and sits directly beneath it.
 *
 * `quantity` is the whole carried amount, exactly as the Take rows send the whole stored amount.
 * Native clamps it to what the container has room for and reports how much actually moved, so the
 * host never has to know the capacity rule. Patched in place like every list that carries a
 * control — a `replaceChildren` here would drop the press between pointerdown and pointerup.
 */
function renderInspectorLoad(building: EntitySnapshot | undefined): void {
  const section = required<HTMLElement>("inspect-load");
  const list = required<HTMLDivElement>("inspector-load");
  const carried =
    building?.kind === "container" ? snapshot.player.carry_stacks : [];
  // One row per item, not one per stack: a Put moves everything of that item that fits.
  const totals = new Map<number, number>();
  for (const { item_id, quantity } of carried)
    totals.set(item_id, (totals.get(item_id) ?? 0) + quantity);
  section.hidden = totals.size === 0;
  const entries = [...totals].sort(([a], [b]) => a - b);
  const rows = syncChildren(
    list,
    entries.map(([item_id]) => String(item_id)),
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
  entries.forEach(([itemId, quantity], index) => {
    const row = rows[index];
    if (!row) return;
    const item = host.definitions.items.find(({ id }) => id === itemId);
    const name = item?.name ?? `Item ${itemId}`;
    setItemGlyph(part(row, ".inspect-item-glyph"), item?.icon, item?.color);
    part(row, "strong").textContent = name;
    part(row, ".inspect-stock-item > span:last-child").textContent =
      String(quantity);
    const button = part<HTMLButtonElement>(row, "button");
    button.dataset.itemId = String(itemId);
    button.dataset.quantity = String(quantity);
    button.dataset.q = String(building?.q ?? 0);
    button.dataset.r = String(building?.r ?? 0);
    button.textContent = "Put";
    button.setAttribute("aria-label", `Put ${quantity} ${name} in`);
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
    renderInspectorLoad(undefined);
    renderInspectorTier(undefined);
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
  renderInspectorLoad(building);
  renderInspectorTier(building);
  renderInspectorRecipe(building);
}

/**
 * What tier this building is, how far it reaches, and what the next step up costs. Every number
 * comes from the definition table the host already holds, so a tier costs no snapshot field and
 * no wire change — the entity publishes its `definition_id` and the ladder is read from that.
 */
function renderInspectorTier(building: EntitySnapshot | undefined): void {
  const card = required<HTMLElement>("inspect-tier");
  const chip = required<HTMLElement>("inspect-tier-chip");
  const reach = required<HTMLElement>("inspect-reach");
  const button = required<HTMLButtonElement>("inspect-upgrade");
  const definition = building
    ? host.definitions.buildings.find(({ id }) => id === building.definition_id)
    : undefined;
  const next = definition?.upgrades_to
    ? host.definitions.buildings.find(({ id }) => id === definition.upgrades_to)
    : undefined;
  const tier = definition?.tier ?? 0;
  // Nothing to say about a base-tier building with no ladder above it, and an empty ruled box is
  // exactly what the readability pass took out.
  card.hidden = !definition || (tier === 0 && !next);
  if (card.hidden) return;
  chip.textContent = `Tier ${tier + 1}`;
  const radius = definition?.extract_radius;
  reach.hidden = radius === undefined;
  if (radius !== undefined)
    reach.textContent = `Reaches ${radius} ${radius === 1 ? "hex" : "hexes"}`;
  button.hidden = !next || Boolean(building?.scenario_owned);
  if (!next || !building) return;
  const unlocked =
    next.unlock_technology_id === undefined ||
    snapshot.researched.includes(next.unlock_technology_id);
  button.disabled = !unlocked;
  button.dataset.q = String(building.q);
  button.dataset.r = String(building.r);
  button.textContent = unlocked ? `Upgrade to ${next.name}` : "Upgrade locked";
  button.setAttribute(
    "aria-label",
    unlocked
      ? `Upgrade to ${next.name} for ${costSummary(next)}`
      : `${next.name} is locked by research`,
  );
  button.title = unlocked ? `Costs ${costSummary(next)}` : "";
}

/** A construction cost written the way the dock writes one. */
function costSummary(definition: BuildingDefinition): string {
  return (
    definition.construction_cost
      .map(({ item_id, quantity }) => {
        const item = host.definitions.items.find(({ id }) => id === item_id);
        return `${quantity} ${item?.name ?? `item ${item_id}`}`;
      })
      .join(", ") || "nothing"
  );
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
  void syncWorldInputs();
}

function selectTool(next: Tool): void {
  tool = next;
  renderer.setBuildMode(next !== "inspect");
  renderRecipePicker();
  renderHotbar();
  // Picking up a riser with an eastward heading held would carry an orientation the definition
  // cannot take, so the pending heading is snapped onto the new tool's axis. `setOrientation` does
  // the rest: the label, the footprint preview, and the refreshed legality all follow from it.
  const { start, end } = orientationRange(next);
  setOrientation(
    orientation >= start && orientation < end ? orientation : start,
  );
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
/**
 * The scalar parameters the new-world flow exposes, in the order the two questions are actually
 * asked: how big is a landform, then how much of the world each band covers. The resource table is
 * the fourth kind of parameter and is not edited here — a preset supplies it whole.
 *
 * Ranges are the native validator's, restated so a form cannot offer a value native will refuse.
 */
type WorldScalar = Exclude<keyof WorldParams, "field_rules">;

const WORLD_PARAMETER_FIELDS: {
  key: WorldScalar;
  label: string;
  min: number;
  max: number;
}[] = [
  { key: "elevation_coarse_cell", label: "Landform scale", min: 1, max: 64 },
  { key: "elevation_fine_cell", label: "Detail scale", min: 1, max: 64 },
  {
    key: "elevation_coarse_weight",
    label: "Landform share %",
    min: 0,
    max: 100,
  },
  { key: "moisture_cell", label: "Moisture scale", min: 1, max: 64 },
  { key: "richness_cell", label: "Richness scale", min: 1, max: 64 },
  { key: "vein_cell", label: "Vein scale", min: 1, max: 64 },
  { key: "water_level", label: "Sea level", min: 0, max: 65535 },
  { key: "shore_level", label: "Shore level", min: 0, max: 65535 },
  { key: "hills_level", label: "Hills level", min: 0, max: 65535 },
  { key: "highland_level", label: "Highland level", min: 0, max: 65535 },
  { key: "cliff_step", label: "Cliff steepness", min: 1, max: 65535 },
  { key: "deep_water_moisture", label: "Deep water", min: -1, max: 65535 },
];

/** What Start scenario will generate. Native validates it again on arrival. */
let pendingWorld: WorldParams | null = null;
const worldParameterInputs = new Map<WorldScalar, HTMLInputElement>();

for (const preset of host.worldPresets) {
  const option = document.createElement("option");
  option.value = preset.key;
  option.textContent = preset.name;
  worldPresetInput.append(option);
}
// A hand-edited parameter set is no preset, and saying so is what keeps the picker honest about
// what is about to be generated.
const customOption = document.createElement("option");
customOption.value = "custom";
customOption.textContent = "Custom";
customOption.hidden = true;
worldPresetInput.append(customOption);

// Built once and only ever written to. A form rebuilt under a pointer loses the control it was
// rebuilt for, which is the same rule the catalogue and the research list live under.
for (const field of WORLD_PARAMETER_FIELDS) {
  const label = document.createElement("label");
  label.textContent = field.label;
  const control = document.createElement("input");
  control.type = "number";
  control.min = String(field.min);
  control.max = String(field.max);
  control.setAttribute("aria-label", field.label);
  control.addEventListener("input", () => {
    if (!pendingWorld) return;
    const value = Number(control.value);
    if (!Number.isSafeInteger(value)) return;
    pendingWorld = { ...pendingWorld, [field.key]: value };
    customOption.hidden = false;
    worldPresetInput.value = "custom";
  });
  label.append(control);
  worldParameterFields.append(label);
  worldParameterInputs.set(field.key, control);
}

function showWorldParams(params: WorldParams): void {
  pendingWorld = params;
  for (const [key, control] of worldParameterInputs) {
    control.value = String(params[key]);
  }
  const preset = host.presetKeyFor(params);
  customOption.hidden = preset !== undefined;
  worldPresetInput.value = preset ?? "custom";
  worldPresetDescription.textContent =
    host.worldPresets.find((entry) => entry.key === worldPresetInput.value)
      ?.description ?? "Hand-tuned parameters.";
}

worldPresetInput.addEventListener("change", () => {
  const preset = host.worldPresets.find(
    (entry) => entry.key === worldPresetInput.value,
  );
  if (preset) showWorldParams(structuredClone(preset.params));
});

/** The world the running game was generated from, read back from native rather than remembered. */
async function syncWorldInputs(): Promise<void> {
  try {
    showWorldParams(structuredClone(await host.worldParams()));
  } catch (error) {
    reportWorkerError(error);
  }
}

required<HTMLButtonElement>("world-parameters-reset").addEventListener(
  "click",
  () => {
    const preset =
      host.worldPresets.find((entry) => entry.key === worldPresetInput.value) ??
      host.worldPresets[0];
    if (preset) showWorldParams(structuredClone(preset.params));
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
    const next = await host.newGame(
      scenarioInput.value,
      seed,
      pendingWorld ?? undefined,
    );
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
  // The × on a filled slot clears it rather than selecting it.
  const clear = (event.target as Element).closest<HTMLElement>(".hotbar-clear");
  if (clear) {
    const slot = Number(
      clear.closest<HTMLElement>("[data-slot]")?.dataset.slot ?? -1,
    );
    if (slot >= 0) {
      assignHotbarSlot(slot, null);
      showFeedback(`Slot ${slot + 1} cleared`);
    }
    event.stopPropagation();
    return;
  }
  const button = (event.target as Element).closest<HTMLButtonElement>(
    "button[data-tool]",
  );
  if (!button || button.disabled) return;
  const value = button.dataset.tool ?? "inspect";
  selectTool(/^\d+$/.test(value) ? Number(value) : (value as Tool));
});

/**
 * Dragging on the bar. A slot dragged onto another slot swaps with it; a slot dragged off the bar
 * entirely is cleared, which is the gesture a player already expects from a hotbar.
 */
toolShelf.addEventListener("dragstart", (event) => {
  const slot = (event.target as Element).closest<HTMLElement>("[data-slot]");
  if (!slot || !event.dataTransfer) return;
  event.dataTransfer.effectAllowed = "move";
  event.dataTransfer.setData("text/hexfactory-slot", slot.dataset.slot ?? "");
});
toolShelf.addEventListener("dragover", (event) => {
  const slot = (event.target as Element).closest<HTMLElement>("[data-slot]");
  if (!slot) return;
  event.preventDefault();
  const index = Number(slot.dataset.slot);
  if (hotbarDragOver === index) return;
  hotbarDragOver = index;
  renderHotbarSlots();
});
toolShelf.addEventListener("dragleave", (event) => {
  if (
    (event.target as Element).closest("[data-slot]") &&
    hotbarDragOver !== null
  ) {
    hotbarDragOver = null;
    renderHotbarSlots();
  }
});
toolShelf.addEventListener("drop", (event) => {
  const target = (event.target as Element).closest<HTMLElement>("[data-slot]");
  hotbarDragOver = null;
  if (!target || !event.dataTransfer) return;
  event.preventDefault();
  const slot = Number(target.dataset.slot);
  const fromCatalogue = event.dataTransfer.getData("text/hexfactory-build");
  if (fromCatalogue) {
    assignHotbarSlot(slot, Number(fromCatalogue));
    return;
  }
  const fromSlot = Number(event.dataTransfer.getData("text/hexfactory-slot"));
  if (!Number.isInteger(fromSlot) || fromSlot === slot) {
    renderHotbarSlots();
    return;
  }
  // A swap rather than an insert, so no other binding shifts under the player's fingers.
  const moved = hotbar[fromSlot] ?? null;
  hotbar[fromSlot] = hotbar[slot] ?? null;
  hotbar[slot] = moved;
  saveHotbar();
  renderHotbarSlots();
  renderBuildPanel();
});
toolShelf.addEventListener("dragend", () => {
  if (hotbarDragOver === null) return;
  hotbarDragOver = null;
  renderHotbarSlots();
});

const buildGroups = required<HTMLDivElement>("build-groups");
buildGroups.addEventListener("click", (event) => {
  const target = event.target as Element;
  const recipeRow = target.closest<HTMLElement>(".recipe-row");
  if (recipeRow) {
    const definitionId = Number(recipeRow.dataset.definitionId);
    selectedRecipes.set(definitionId, Number(recipeRow.dataset.recipeId));
    selectTool(definitionId);
    renderBuildPanel();
    return;
  }
  const card = target.closest<HTMLElement>(".build-card");
  if (!card) return;
  const definitionId = Number(card.dataset.definitionId);
  if (target.closest("[data-pin]")) {
    pinToHotbar(definitionId);
    return;
  }
  if (card.classList.contains("locked")) {
    showFeedback("That building is still locked by research");
    return;
  }
  selectTool(definitionId);
  renderBuildPanel();
});
buildGroups.addEventListener("dragstart", (event) => {
  const card = (event.target as Element).closest<HTMLElement>(".build-card");
  if (!card || !event.dataTransfer) return;
  event.dataTransfer.effectAllowed = "copy";
  event.dataTransfer.setData(
    "text/hexfactory-build",
    card.dataset.definitionId ?? "",
  );
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
required<HTMLButtonElement>("inspect-upgrade").addEventListener(
  "click",
  (event) => {
    const button = event.currentTarget as HTMLButtonElement;
    enqueue({
      type: "upgrade",
      q: Number(button.dataset.q),
      r: Number(button.dataset.r),
    });
  },
);
required<HTMLDivElement>("inspector-load").addEventListener(
  "click",
  (event) => {
    const button = (event.target as Element).closest<HTMLButtonElement>(
      "button[data-item-id]",
    );
    if (!button) return;
    enqueue({
      type: "store",
      q: Number(button.dataset.q),
      r: Number(button.dataset.r),
      item_id: Number(button.dataset.itemId),
      quantity: Number(button.dataset.quantity),
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
    // A digit is a slot, not an index into the catalogue. Which building it builds is the
    // player's arrangement, and it is theirs to change.
    const slot = Number(event.code.slice(-1)) - 1;
    const value = hotbar[slot] ?? null;
    if (value === null) {
      showFeedback(`Slot ${slot + 1} is empty — pin something from Build (B)`);
      event.preventDefault();
      return;
    }
    selectTool(value);
    event.preventDefault();
    return;
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
    // Measured from the press, not from the last frame, so slow drift accumulates and cancels the
    // harvest rather than creeping across the map one sub-threshold step at a time.
    if (
      Math.abs(event.clientX - panPointer.originX) +
        Math.abs(event.clientY - panPointer.originY) >
      HARVEST_HOLD_SLOP
    )
      panPointer.harvest = null;
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
    // A right press starts working the hex under it straight away and keeps working it while the
    // button is down; the frame loop repeats it and the native action cooldown paces the repeat,
    // exactly as a held F is paced. Dragging out of the hex turns the gesture back into a pan.
    const harvest =
      event.button === 2 ? renderer.pick(event.clientX, event.clientY) : null;
    panPointer = {
      id: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      moved: false,
      originX: event.clientX,
      originY: event.clientY,
      harvest,
    };
    if (harvest) {
      selected = harvest;
      renderer.setSelection(harvest);
      enqueue({ type: "gather_at", ...harvest });
      renderInspector();
    }
    // Captured last: capture is what keeps the gesture alive off the canvas, not what makes the
    // press mean something. Taking it first would let a refused capture swallow the first harvest
    // while still leaving the hold armed.
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
    // Releasing ends the hold. The harvest began on the press and repeated every frame since.
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
canvas.addEventListener("pointercancel", (event) => {
  // A cancelled pointer never sends `pointerup`, and a held harvest that outlived its gesture
  // would keep working a hex with nothing holding the button down.
  if (panPointer?.id === event.pointerId) panPointer = null;
  endDrag(event.pointerId);
});
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
  else if (tool === "upgrade") enqueue({ type: "upgrade", ...coordinate });
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
  selectTool(definition.id);
  setOrientation(building.orientation);
  showFeedback(`Copied ${definition.name}`);
}

function setOrientation(next: number): void {
  orientation = next;
  required<HTMLElement>("orientation-value").textContent =
    `${DIRECTION_NAMES[orientation]} · R`;
  const definition =
    typeof tool === "number"
      ? host.definitions.buildings.find(({ id }) => id === tool)
      : undefined;
  renderer.setBuildFootprint(
    definition?.footprint ?? [{ q: 0, r: 0 }],
    // A vertical heading has no 60° rotation, and the footprint that carries one is a single cell
    // by definition, so the preview asks for no turns rather than an impossible number of them.
    orientation >= NORTH ? 0 : orientation,
  );
  refreshHoverPreview();
}

/**
 * The orientations the pending tool may take. Native owns the rule; this reads the same definition
 * field it reads, so the dock can never offer a heading placement would refuse.
 */
function orientationRange(tool: Tool): { start: number; end: number } {
  const definition =
    typeof tool === "number"
      ? host.definitions.buildings.find(({ id }) => id === tool)
      : undefined;
  return definition?.orientation_axis === "vertical"
    ? { start: NORTH, end: DIRECTION_NAMES.length }
    : { start: 0, end: NORTH };
}

function rotateNewBuilding(): void {
  const { start, end } = orientationRange(tool);
  // Rotation stays on the tool's own axis: a belt walks the six edges and a riser flips between
  // north and south. `rotateHexDirection` still turns the six, so the package keeps owning the
  // geometry it knows.
  setOrientation(
    start === 0
      ? rotateHexDirection(orientation as HexDirection, 1)
      : start + ((orientation - start + 1) % (end - start)),
  );
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
    // player holds the key instead of tapping it once per unit. A held right-click is the same
    // idea aimed at a named hex, and it outranks the untargeted one: if both are held, the hex the
    // player is pointing at is the one they chose.
    if (!input.size) {
      if (panPointer?.harvest)
        input.enqueue({ type: "gather_at", ...panPointer.harvest });
      else if (gatherHeld) input.enqueue({ type: "gather" });
    }
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
