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
import type {
  FactorySnapshot,
  NativeInputCommand,
  PlacementPreview,
} from "./core/types";
import { CanvasFactoryRenderer } from "./rendering/CanvasFactoryRenderer";
import "./styles.css";

type Tool = "inspect" | "erase" | "rotate" | number;

const SAVE_KEY = "hexfactory:hxf1:v2";
const DIRECTION_NAMES = [
  "East",
  "Southeast",
  "Southwest",
  "West",
  "Northwest",
  "Northeast",
];
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

let snapshot = host.snapshot();
let playing = true;
let tool: Tool = "inspect";
let orientation: HexDirection = 0;
let selected: { q: number; r: number } | null = null;
let hover: { q: number; r: number } | null = null;
let hoverPreview: PlacementPreview | null = null;
let accumulator = 0;
let previousTime = performance.now();
let feedbackTimer = 0;
let lastEvent = "";
let panPointer: { id: number; x: number; y: number; moved: boolean } | null =
  null;
let suppressMapClick = false;
const pressedMovement = new Set<string>();
let movementRevision = 0;
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
  renderer.setSnapshot(snapshot);
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
  if (latestEvent && latestEvent !== lastEvent) showFeedback(latestEvent);
  lastEvent = latestEvent;
  const victory = required<HTMLDivElement>("victory");
  victory.hidden = !snapshot.victory;
  if (!previousVictory && snapshot.victory)
    showFeedback("Landing objective complete — free play continues");
}

function renderInventory(): void {
  const element = required<HTMLDivElement>("inventory");
  element.replaceChildren();
  for (const item of host.definitions.items) {
    const quantity = snapshot.player.inventory[String(item.id)] ?? 0;
    const row = document.createElement("div");
    row.className = "inventory-item";
    row.innerHTML = `<span class="swatch" style="--item-color:${item.color}"></span><span>${item.name}</span><strong>${quantity}</strong>`;
    element.append(row);
  }
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
    button.innerHTML = `<span>${availability.locked ? "◇" : definition.icon}</span><small>${availability.locked ? definition.name : `${definition.name} · ${availability.costLabel}`}</small>`;
    button.title = `${definition.description} ${availability.costLabel}`;
  }
}

function renderTechnologies(): void {
  const list = required<HTMLDivElement>("technology-list");
  list.replaceChildren();
  for (const technology of host.technologies.technologies) {
    const state = technologyAvailability(technology, snapshot);
    const button = document.createElement("button");
    button.type = "button";
    button.dataset.technologyId = String(technology.id);
    button.disabled =
      state.complete || !state.prerequisitesMet || !state.affordable;
    button.className = state.complete
      ? "complete"
      : state.prerequisitesMet && state.affordable
        ? "available"
        : "";
    button.setAttribute(
      "aria-label",
      `Research ${technology.name} for ${technology.cost} insight`,
    );
    button.innerHTML = `<strong>${technology.name}</strong><span>${technology.description}</span><small>${state.complete ? "Complete" : !state.prerequisitesMet ? "Prerequisite locked" : `${technology.cost} insight`}</small>`;
    list.append(button);
  }
}

function renderInspector(): void {
  const element = required<HTMLDivElement>("selection-value");
  if (!selected) {
    element.textContent = "Select a hex on the map.";
    return;
  }
  const building = snapshot.buildings.find(({ footprint }) =>
    footprint.some(({ q, r }) => q === selected?.q && r === selected?.r),
  );
  const selectedWorld = axialToPixel(selected, 1024, { x: 0, y: 0 });
  const resource = snapshot.resources.find(
    ({ x, y, radius }) =>
      Math.hypot(x - selectedWorld.x, y - selectedWorld.y) <= radius,
  );
  const lines = [`Build hex ${selected.q}, ${selected.r}`];
  if (resource) {
    const item = host.definitions.items.find(
      ({ id }) => id === resource.item_id,
    );
    lines.push(
      `${item?.name ?? "Resource"}: ${resource.quantity} / ${resource.initial_quantity}`,
    );
  }
  if (building) {
    const definition = host.definitions.buildings.find(
      ({ id }) => id === building.definition_id,
    );
    const stored = building.inventory.reduce(
      (sum, item) => sum + item.quantity,
      0,
    );
    lines.push(`${definition?.name ?? building.kind} · ${building.status}`);
    lines.push(
      `Direction ${building.orientation} · stored ${stored}${building.cargo ? ` · cargo ${building.cargo.quantity}` : ""}`,
    );
    if (building.scenario_owned) lines.push("Protected scenario object");
  }
  element.textContent = lines.join("\n");
}

function renderObjective(): void {
  const item = host.definitions.items.find(
    ({ id }) => id === snapshot.objective.item_id,
  );
  required<HTMLElement>("objective-detail").textContent = snapshot.victory
    ? `Complete: ${snapshot.objective.delivered} ${item?.name ?? "items"} delivered. Continue building freely.`
    : `Deliver ${snapshot.objective.required} ${item?.name ?? "items"} to the landing hub. Progress: ${snapshot.objective.delivered}.`;
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
  let detail = "Move toward a glowing resource deposit and gather a sample.";
  if (snapshot.victory) {
    title = "Factory online";
    detail =
      "The landing directive is complete. Expand, optimize, or inspect the running line.";
  } else if (snapshot.scenario === "factory-demo") {
    title = "Trace the material flow";
    detail =
      "Follow cargo from extractor to receiver. Pause or single-step to inspect arbitration.";
  } else if (!researched.has(1) && snapshot.insight >= 3) {
    title = "Unlock Field Logistics";
    detail =
      "You have enough insight. Research Field Logistics to add belts to the construction dock.";
  } else if (!researched.has(1) && ore + crystals === 0) {
    title = "Gather your first material";
    detail =
      "Walk beside a glowing deposit, then gather. Resource circles show their remaining amount.";
  } else if (!researched.has(1)) {
    title = "Deliver materials for insight";
    detail =
      "Return to the gold landing hub and deliver your cargo. Three ore fund the first breakthrough.";
  } else if (!researched.has(2) && snapshot.insight >= 5) {
    title = "Automate extraction";
    detail =
      "Research Automated Extraction, then place an extractor directly on a resource deposit.";
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
  } else {
    title = "Compose three components";
    detail =
      "Route ore into a composer, point its output toward the hub, and keep the line supplied.";
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
  playButton.title = playing ? "Pause simulation" : "Play simulation";
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
  required<HTMLElement>("selected-tool-value").textContent =
    definition?.name ?? titleCase(String(next));
  renderer.setBuildMode(next !== "inspect");
  renderer.setBuildFootprint(
    definition?.footprint ?? [{ q: 0, r: 0 }],
    orientation,
  );
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
required<HTMLButtonElement>("gather").addEventListener("click", () =>
  enqueue({ type: "gather" }),
);
required<HTMLButtonElement>("deposit").addEventListener("click", () =>
  enqueue({ type: "deposit" }),
);
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

window.addEventListener("keydown", (event) => {
  if (
    event.ctrlKey ||
    event.metaKey ||
    event.altKey ||
    isTypingTarget(event.target)
  )
    return;
  if (event.code in MOVEMENT_KEYS) {
    event.preventDefault();
    if (!pressedMovement.has(event.code)) {
      pressedMovement.add(event.code);
      movementRevision += 1;
      enqueue(movementIntent(pressedMovement));
    }
    return;
  }
  if (event.code === "Escape") {
    selectTool("inspect");
    closePanels();
  } else if (event.code === "Space") setPlaying(!playing);
  else if (event.code === "KeyF") enqueue({ type: "gather" });
  else if (event.code === "KeyX") enqueue({ type: "deposit" });
  else if (event.code === "KeyR") rotateNewBuilding();
  else if (/^Digit[1-4]$/.test(event.code)) {
    const buildable = host.definitions.buildings.filter(
      ({ buildable }) => buildable,
    );
    const definition = buildable[Number(event.code.at(-1)) - 1];
    if (definition) selectTool(definition.id);
  } else return;
  event.preventDefault();
});

window.addEventListener("keyup", (event) => {
  if (!pressedMovement.delete(event.code)) return;
  event.preventDefault();
  movementRevision += 1;
  const revision = movementRevision;
  window.setTimeout(() => {
    if (revision === movementRevision) enqueue(movementIntent(pressedMovement));
  }, 110);
});

window.addEventListener("blur", () => {
  if (!pressedMovement.size) return;
  pressedMovement.clear();
  movementRevision += 1;
  enqueue(movementIntent(pressedMovement));
});

canvas.addEventListener("pointermove", (event) => {
  if (panPointer?.id === event.pointerId) {
    const dx = event.clientX - panPointer.x;
    const dy = event.clientY - panPointer.y;
    if (Math.abs(dx) + Math.abs(dy) > 1) panPointer.moved = true;
    renderer.panBy(dx, dy);
    panPointer.x = event.clientX;
    panPointer.y = event.clientY;
    return;
  }
  hover = renderer.pick(event.clientX, event.clientY);
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
  }
});
canvas.addEventListener("pointerup", (event) => {
  if (panPointer?.id === event.pointerId) {
    suppressMapClick = panPointer.moved;
    canvas.releasePointerCapture(event.pointerId);
    panPointer = null;
  }
});
canvas.addEventListener("pointerleave", () => {
  if (!panPointer) {
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
    const definition = host.definitions.buildings.find(({ id }) => id === tool);
    enqueue({
      type: "place",
      ...coordinate,
      definition_id: tool,
      orientation,
      recipe_id:
        definition?.kind === "composer"
          ? host.definitions.recipes[0]?.id
          : undefined,
    });
  }
  renderInspector();
});

function rotateNewBuilding(): void {
  orientation = rotateHexDirection(orientation, 1);
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

function frame(now: number): void {
  const elapsed = Math.min(250, now - previousTime);
  previousTime = now;
  if (playing) accumulator += elapsed * Number(speedInput.value);
  if (!advancePending) {
    const commands = input.drain();
    const ticks = playing ? Math.min(20, Math.floor(accumulator / 1000)) : 0;
    if (commands.length || ticks > 0) {
      accumulator -= ticks * 1000;
      advancePending = true;
      void host
        .advance(commands, ticks)
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

for (const toggle of document.querySelectorAll<HTMLButtonElement>(
  ".panel-toggle",
)) {
  toggle.addEventListener("click", () => {
    const target = document.getElementById(toggle.dataset.panelTarget ?? "");
    if (!target) return;
    const opening = !target.classList.contains("open");
    closePanels(target);
    target.classList.toggle("open", opening);
    toggle.setAttribute("aria-expanded", String(opening));
  });
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
    movementRevision += 1;
    enqueue(movementIntent(pressedMovement));
  };
  const stop = (event: PointerEvent): void => {
    event.preventDefault();
    if (!pressedMovement.delete(code)) return;
    movementRevision += 1;
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
