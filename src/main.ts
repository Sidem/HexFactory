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
import {
  CanvasFactoryRenderer,
  isSurveyed,
} from "./rendering/CanvasFactoryRenderer";
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
const pressedMovement = new Set<string>();
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
  if (!isSurveyed(snapshot.chunks, selectedWorld))
    lines.push("Unsurveyed — travel here to lift the fog");
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
  let detail =
    "The hatched fog is unsurveyed world. Walk toward it to reveal terrain, then gather from a glowing deposit.";
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
  } else if (event.code === "Space") setPlaying(!playing);
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
  if (!pressedMovement.size) return;
  pressedMovement.clear();
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

function recipeFor(value: Tool): number | undefined {
  const definition =
    typeof value === "number"
      ? host.definitions.buildings.find(({ id }) => id === value)
      : undefined;
  return definition?.kind === "composer"
    ? host.definitions.recipes[0]?.id
    : undefined;
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

function frame(now: number): void {
  const elapsed = Math.min(250, now - previousTime);
  previousTime = now;
  if (playing) accumulator += elapsed * Number(speedInput.value);
  if (!advancePending) {
    // A held gather repeats at frame rate and is paced natively by the action cooldown, so the
    // player holds the key instead of tapping it once per unit.
    if (gatherHeld && !input.size) input.enqueue({ type: "gather" });
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
