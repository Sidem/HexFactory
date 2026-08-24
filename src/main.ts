import {
  axialToPixel,
  rotateHexDirection,
  type HexDirection,
} from "@hexlife/embed/hex";

import {
  buildingAvailability,
  costLines,
  heldQuantity,
  technologyAvailability,
  type CostLine,
} from "./core/availability";
import { cueForEvent, FeedbackAudio } from "./audio/feedback";
import { halfTransfer } from "./core/commands";
import { FactoryHost } from "./core/FactoryHost";
import { nextAction } from "./core/guidance";
import { BoundedInputQueue, MOVEMENT_KEYS, movementIntent } from "./core/input";
import {
  AUTOSAVE_SLOT_NAME,
  compatibility,
  describeMismatches,
  formatConfig,
  formatSavedAt,
  formatVersions,
  importLegacySlots,
  latestCompatible,
  parseHxf1,
  readCatalog,
  removeSlot,
  replaceNamedSlot,
  SAVE_VERSION,
  slotFromPayload,
  slotsNewestFirst,
  upsertSlot,
  writeCatalog,
  type CurrentBuild,
  type SaveSlot,
} from "./core/saveSlots";
import {
  formatElapsed,
  formatRunReport,
  isRunComplete,
  OPENING_CHECKPOINTS,
  readRun,
  recordCheckpoints,
  startRun,
  taintRun,
  writeRun,
  type CheckpointContext,
  type RunTimings,
} from "./core/checkpoints";
import { TERRAIN_INFO, TERRAIN_ORDER, terrainAccess } from "./core/terrain";
import { CORNER_START, DIRECTION_NAMES } from "./core/directions";
import type {
  BuildingDefinition,
  BuildingKind,
  EntitySnapshot,
  FactorySnapshot,
  ItemDefinition,
  NativeInputCommand,
  PlacementPreview,
  RecipeDefinition,
  TechnologyDefinition,
  WorldParams,
  WorldPoint,
} from "./core/types";
import {
  BUILDING_COLORS,
  isSurveyed,
  type FactoryRenderer,
  type GraphicsProfile,
  type RendererDiagnostics,
} from "./rendering/FactoryRenderer";
import { itemIconSvg } from "./rendering/icons";
import {
  createItemChip,
  fillItemChip,
  type ItemChipView,
} from "./rendering/itemChip";
import { findLandingHub, homeBearing } from "./rendering/landmarks";
import { MinimapRenderer } from "./rendering/MinimapRenderer";
import { ThreeFactoryRenderer } from "./rendering/three/ThreeFactoryRenderer";
import {
  defaultGraphicsProfile,
  GRAPHICS_STORAGE_KEY,
  parseGraphicsProfile,
} from "./rendering/three/quality";
import "./styles.css";

type Tool = "inspect" | "erase" | "rotate" | "upgrade" | number;

/**
 * What this build will load. Native still refuses on these numbers; the catalog only reports them.
 * `SAVE_VERSION` is the one literal because native does not publish it.
 */
function currentBuild(): CurrentBuild {
  return {
    versions: {
      save: SAVE_VERSION,
      world: snapshot.world_version,
      definitions: host.definitions.version,
      technology: host.technologies.version,
    },
    scenarios: host.scenarios.scenarios.map((scenario) => ({
      key: scenario.key,
      name: scenario.name,
      version: scenario.version,
    })),
    worldPresets: host.worldPresets,
  };
}
/** The first orientation index off the six-edge table. Matches `NORTH` in the core. */
const NORTH = CORNER_START;
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
  // A stop the player chose reads the same as one the factory fell into, because it is the same
  // fact: this machine is not working. What differs is the fix, and the toggle beside it says so.
  "switched off": "stop",
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
  KeyC: "creative-panel",
};
/**
 * A refusal the world itself already shows. The ring around the player is the swing filling, so
 * repeating it as a message strip toast on every frame of a held harvest is noise.
 */
const SILENT_EVENTS = new Set(["action cooling down"]);
const canvas = required<HTMLCanvasElement>("factory-canvas");
const playButton = required<HTMLButtonElement>("play");
const soundButton = required<HTMLButtonElement>("sound");
const muteInput = required<HTMLInputElement>("mute");
const reduceMotionInput = required<HTMLInputElement>("reduce-motion");
const graphicsProfileInput = required<HTMLSelectElement>("graphics-profile");
/** Comfort settings are preferences about a room, so they live beside the hotbar, not in a save. */
const MOTION_KEY = "hexfactory:reduced-motion:v1";
const speedInput = required<HTMLSelectElement>("speed");
const scenarioInput = required<HTMLSelectElement>("scenario");
const seedInput = required<HTMLInputElement>("seed");
const saveNameInput = required<HTMLInputElement>("save-name");
const worldPresetInput = required<HTMLSelectElement>("world-preset");
const worldPresetDescription = required<HTMLParagraphElement>(
  "world-preset-description",
);
const worldParameterFields = required<HTMLDivElement>("world-parameter-fields");
const toolShelf = required<HTMLDivElement>("tool-shelf");
const feedback = required<HTMLDivElement>("feedback");
const creativeChip = required<HTMLButtonElement>("creative-chip");
const creativeEnabledInput = required<HTMLInputElement>("creative-enabled");
const creativeSlotsInput = required<HTMLInputElement>("creative-slots");
const creativeClear = required<HTMLButtonElement>("creative-clear");
const creativeItems = required<HTMLDivElement>("creative-items");

const titleScreen = required<HTMLElement>("title-screen");
const titleContinue = required<HTMLButtonElement>("title-continue");
const titleContinueSub = required<HTMLElement>("title-continue-sub");
const titleTabSaves = required<HTMLButtonElement>("title-tab-saves");
const titleTabNew = required<HTMLButtonElement>("title-tab-new");
const titleResume = required<HTMLButtonElement>("title-resume");
const titleSavesBadge = required<HTMLElement>("title-saves-badge");
const titleSavesView = required<HTMLElement>("title-saves-view");
const titleNewGameView = required<HTMLElement>("title-new-game-view");
const titleScenarioInput = required<HTMLSelectElement>("title-scenario");
const titleSeedInput = required<HTMLInputElement>("title-seed");
const titleSeedRandom = required<HTMLButtonElement>("title-seed-random");
const titleWorldPresetInput = required<HTMLSelectElement>("title-world-preset");
const titleWorldPresetDescription = required<HTMLParagraphElement>(
  "title-world-preset-description",
);
const titleWorldParameterFields = required<HTMLDivElement>(
  "title-world-parameter-fields",
);
const titleWorldParametersReset = required<HTMLButtonElement>(
  "title-world-parameters-reset",
);
const titleStartGame = required<HTMLButtonElement>("title-start-game");
const titleCreativeInput = required<HTMLInputElement>("title-creative");
const titleMuteInput = required<HTMLInputElement>("title-mute");
const titleReduceMotionInput = required<HTMLInputElement>(
  "title-reduce-motion",
);
const titleGraphicsProfileInput = required<HTMLSelectElement>(
  "title-graphics-profile",
);
const sessionMainMenu = required<HTMLButtonElement>("session-main-menu");
const input = new BoundedInputQueue();
const audio = new FeedbackAudio();
const host = await FactoryHost.create();
const storedGraphics = parseGraphicsProfile(
  localStorage.getItem(GRAPHICS_STORAGE_KEY),
);
const initialGraphics = storedGraphics ?? defaultGraphicsProfile();
const renderer: FactoryRenderer = new ThreeFactoryRenderer(
  canvas,
  host.definitions,
  initialGraphics,
);
if (
  import.meta.env.DEV &&
  new URLSearchParams(location.search).has("context-test")
) {
  const cycleContext = document.createElement("button");
  cycleContext.type = "button";
  cycleContext.textContent = "Cycle WebGL context";
  cycleContext.style.cssText =
    "position:fixed;z-index:10000;right:12px;top:72px;padding:8px";
  cycleContext.addEventListener("click", () =>
    canvas.dispatchEvent(new Event("hexfactory:test-context-cycle")),
  );
  document.body.append(cycleContext);
}
if (
  import.meta.env.DEV &&
  new URLSearchParams(location.search).has("diagnostics")
) {
  const captureDiagnostics = document.createElement("button");
  const diagnosticsOutput = document.createElement("output");
  captureDiagnostics.type = "button";
  captureDiagnostics.textContent = "Capture renderer diagnostics";
  captureDiagnostics.style.cssText =
    "position:fixed;z-index:10000;right:12px;top:72px;padding:8px";
  diagnosticsOutput.id = "renderer-diagnostics";
  diagnosticsOutput.style.cssText =
    "position:fixed;z-index:10000;right:12px;top:116px;max-width:480px;padding:8px;background:#071110;color:#dcefe9";
  captureDiagnostics.addEventListener("click", () => {
    diagnosticsOutput.textContent = JSON.stringify(renderer.getDiagnostics());
  });
  document.body.append(captureDiagnostics, diagnosticsOutput);
}
const minimap = new MinimapRenderer(
  required<HTMLCanvasElement>("minimap"),
  host.definitions,
);

let snapshot = host.snapshot();
/** Which named slot Save will overwrite, if any. Presentation only — the catalog is the store. */
let selectedSaveId: string | null = null;
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
let autoSavePending = false;
let lastAutoSaveTime = performance.now();
const AUTOSAVE_INTERVAL_MS = 60_000;
/**
 * The run clock, and the time it has counted.
 *
 * `runElapsedMs` accrues only while the factory is live and the title screen is closed, which makes
 * it in-game time rather than wall time. That is the convention a run should be measured in — a
 * player who opened the menu to read a recipe did not spend that time playing, and a clock that
 * charged them for it would make every comparison a test of how fast someone reads.
 */
let run: RunTimings | null = null;
let runElapsedMs = 0;
/**
 * The button-held camera gesture. The middle button, or shift with the left — never the right one.
 *
 * The right button used to pan as well as harvest, and the two readings of one gesture had to be
 * told apart by a drift threshold: a hold that wandered a few pixels stopped being a harvest. That
 * is a rule a hand cannot see, and it fires exactly when the player is working a hex for several
 * seconds. Panning has the middle button to itself now, so the ambiguity is gone rather than
 * arbitrated.
 */
let panPointer: {
  id: number;
  x: number;
  y: number;
  moved: boolean;
} | null = null;
/**
 * The hex a held right-click keeps working, tracked to the cursor rather than fixed at the press.
 *
 * Nothing competes for this gesture any more, so dragging is free to mean something useful: the
 * hold follows the pointer and works whatever hex it is over, which is how a player clears a seam
 * without releasing and pressing again on every cell.
 */
let harvestPointer: { id: number; q: number; r: number } | null = null;
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
let runningHeld = false;
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
type BuildGroupKey =
  | "extraction"
  | "transport"
  | "processing"
  | "storage"
  | "power";
const BUILD_GROUP_BY_KIND = {
  extractor: "extraction",
  belt: "transport",
  composer: "processing",
  container: "storage",
  consumer: null,
  hub: null,
  pump: "extraction",
  pole: "power",
  generator: "power",
  boiler: "power",
  bridge: "transport",
} satisfies Record<BuildingKind, BuildGroupKey | null>;
const BUILD_GROUPS: {
  key: BuildGroupKey;
  title: string;
  blurb: string;
  holds: (definition: BuildingDefinition) => boolean;
}[] = [
  {
    key: "extraction",
    title: "Extraction",
    blurb: "Take raw material out of the ground and the water.",
    holds: ({ kind }) => BUILD_GROUP_BY_KIND[kind] === "extraction",
  },
  {
    key: "transport",
    title: "Transport",
    blurb:
      "Move cargo. Belts run along hex edges; risers use the six corner headings between them.",
    holds: ({ kind }) => BUILD_GROUP_BY_KIND[kind] === "transport",
  },
  {
    key: "processing",
    title: "Processing",
    blurb:
      "Turn one material into another. Each machine runs one category of recipe.",
    holds: ({ kind }) => BUILD_GROUP_BY_KIND[kind] === "processing",
  },
  {
    key: "storage",
    title: "Storage",
    blurb: "Buffer a line, and hold stock you can take back by hand.",
    holds: ({ kind }) => BUILD_GROUP_BY_KIND[kind] === "storage",
  },
  {
    key: "power",
    title: "Power",
    blurb:
      "Make electricity and carry it. Machines draw; belts and boxes do not.",
    holds: ({ kind }) => BUILD_GROUP_BY_KIND[kind] === "power",
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
/** Panel scope, not game state: which side of progressive disclosure each catalogue is showing. */
let showAllTechnologies = false;
let showAllBuildings = false;
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

/**
 * Flatten a snapshot into the facts a checkpoint predicate asks about.
 *
 * Item and building keys are resolved here rather than in the checkpoint module so the predicates
 * name `crystal` and `composer` instead of the ids those happen to hold. An id is a wire detail
 * that a definitions bump may reassign; a key is the thing the design actually means.
 */
function checkpointContext(next: FactorySnapshot): CheckpointContext {
  const carried: Record<string, number> = {};
  for (const item of host.definitions.items) {
    const held =
      next.player.inventory[String(item.id)] ??
      next.player.inventory[item.id] ??
      0;
    if (held > 0) carried[item.key] = held;
  }
  const buildingKeys = new Map(
    host.definitions.buildings.map((definition) => [
      definition.id,
      definition.key,
    ]),
  );
  return {
    tick: next.tick,
    contractStage: next.contract.stage,
    researchedCount: next.researched.length,
    carried,
    buildings: next.buildings.map((entity) => ({
      key: buildingKeys.get(entity.definition_id) ?? "",
      kind: entity.kind,
      status: entity.status,
      powered:
        (entity.power_charge ?? 0) > 0 || (entity.power_satisfied ?? 0) > 0,
    })),
  };
}

function evaluateRun(next: FactorySnapshot): void {
  if (!run) return;
  if (isRunComplete(run)) return;
  const result = recordCheckpoints(run, checkpointContext(next), runElapsedMs);
  if (result.reached.length === 0) return;
  run = result.run;
  writeRun(localStorage, run);
  const last = result.reached.at(-1);
  const checkpoint = last
    ? OPENING_CHECKPOINTS.find(({ id }) => id === last.id)
    : undefined;
  if (last && checkpoint)
    showFeedback(`${checkpoint.label} — ${formatElapsed(last.elapsedMs)}`);
  renderRun();
}

/** Start the clock over. A fresh scenario is a fresh run; nothing else may reset it silently. */
function beginRun(next: FactorySnapshot): void {
  runElapsedMs = 0;
  run = startRun(Date.now(), next.tick, Number(speedInput.value));
  writeRun(localStorage, run);
  renderRun();
}

function renderRun(): void {
  const conditions = required<HTMLElement>("run-conditions");
  const tainted = (run?.taints.length ?? 0) > 0;
  conditions.textContent = !run
    ? "No run timed yet. Start a scenario to begin the clock."
    : tainted
      ? `${run.startedSpeed} tps · not comparable (${run.taints.join(", ")})`
      : `${run.startedSpeed} tps · clean`;
  conditions.classList.toggle("run-tainted", tainted);
  // Keyed in place rather than rebuilt, for the reason every other list here is: a row replaced
  // between pointerdown and pointerup eats the click that was already on its way.
  const rows = syncChildren(
    required<HTMLElement>("run-checkpoints"),
    OPENING_CHECKPOINTS.map(({ id }) => id),
    () => {
      const row = document.createElement("li");
      row.className = "run-row";
      const time = document.createElement("strong");
      time.className = "run-time";
      const label = document.createElement("span");
      label.className = "run-label";
      const note = document.createElement("small");
      note.className = "run-note";
      row.append(time, label, note);
      return row;
    },
  );
  const byId = new Map(
    (run?.records ?? []).map((record) => [record.id, record]),
  );
  OPENING_CHECKPOINTS.forEach((checkpoint, index) => {
    const row = rows[index];
    if (!row) return;
    const record = byId.get(checkpoint.id);
    row.classList.toggle("run-reached", record !== undefined);
    part<HTMLElement>(row, ".run-time").textContent = record
      ? formatElapsed(record.elapsedMs)
      : "--:--";
    part<HTMLElement>(row, ".run-label").textContent = checkpoint.label;
    part<HTMLElement>(row, ".run-note").textContent = record
      ? `tick ${record.tick.toLocaleString()}`
      : checkpoint.note;
  });
}

function update(next: FactorySnapshot): void {
  const previousVictory = snapshot.victory;
  const previous = snapshot;
  snapshot = next;
  refreshLandingHub();
  renderer.setHome(landingHub);
  renderer.setSnapshot(snapshot);
  syncHoverWithCamera();
  minimap.setSnapshot(snapshot, landingHub);
  renderHomeReadout();
  required<HTMLElement>("scenario-value").textContent = snapshot.scenario_name;
  required<HTMLElement>("tick-value").textContent =
    snapshot.tick.toLocaleString();
  required<HTMLElement>("insight-value").textContent =
    snapshot.insight.toLocaleString();
  required<HTMLElement>("position-value").textContent =
    `${(snapshot.player.x / 1024).toFixed(1)}, ${(snapshot.player.y / 1024).toFixed(1)}`;
  required<HTMLElement>("surveyed-value").textContent =
    snapshot.chunks.length.toLocaleString();
  required<HTMLElement>("checksum-value").textContent = snapshot.checksum
    .toString(16)
    .padStart(8, "0")
    .toUpperCase();
  // Walking changes the player every frame. Rebuilding every panel for that is the hitch on a
  // weak machine: the factory HUD only moves when the factory does.
  const packChanged = !sameCarry(previous.player, next.player);
  // Switching creative off re-prices every card, and a wider pack changes what fits, so both count
  // as the factory moving even on a tick where nothing was built.
  const creativeChanged =
    previous.player.creative !== next.player.creative ||
    previous.player.carry_slots !== next.player.carry_slots;
  const factoryChanged =
    previous === next ||
    creativeChanged ||
    previous.tick !== next.tick ||
    previous.insight !== next.insight ||
    previous.victory !== next.victory ||
    previous.buildings !== next.buildings ||
    previous.resources !== next.resources ||
    previous.researched !== next.researched ||
    previous.contract !== next.contract ||
    previous.requests !== next.requests ||
    previous.events !== next.events;
  if (packChanged || factoryChanged) {
    renderInventory();
    renderCreative();
    renderHotbar();
    renderBuildPanel();
    renderTechnologies();
    renderContract();
    renderRequests();
    renderNextAction();
    // Both kinds of change can complete a checkpoint: the first iron is a pack change and the
    // first powered composer is a factory one, so the clock reads whenever either moved.
    evaluateRun(next);
  }
  if (factoryChanged || selected) renderInspector();
  const latestEvent = snapshot.events.at(-1) ?? "";
  if (
    latestEvent &&
    latestEvent !== lastEvent &&
    !SILENT_EVENTS.has(latestEvent)
  ) {
    showFeedback(latestEvent);
    // Sound comes from the same event the toast does, so a delivery made by a belt and one made
    // by hand are the same thing heard as well as read.
    const cue = cueForEvent(latestEvent);
    if (cue) audio.play(cue);
  }
  lastEvent = latestEvent;
  const victory = required<HTMLDivElement>("victory");
  victory.hidden = !snapshot.victory;
  if (!previousVictory && snapshot.victory)
    showFeedback("Founding contract complete — free play continues");
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
 * The way home, in words and as a labelled compass under the minimap. Keeping the pointer in that
 * fixed navigation frame leaves the world and its hover surface unobstructed.
 */
function renderHomeReadout(): void {
  const element = required<HTMLElement>("home-readout");
  const text = required<HTMLElement>("home-readout-text");
  if (!landingHub) {
    element.classList.remove("away");
    text.textContent = "No landing hub in this world";
    return;
  }
  const bearing = homeBearing(snapshot.player, landingHub);
  element.classList.toggle("away", bearing !== null);
  if (bearing) {
    const degrees = (Math.atan2(bearing.x, -bearing.y) * 180) / Math.PI;
    element.style.setProperty("--home-bearing", `${degrees}deg`);
  }
  text.textContent = bearing
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

/** The item definition behind an id, or `undefined` when the catalogue has no such row. */
function sameCarry(
  previous: FactorySnapshot["player"],
  next: FactorySnapshot["player"],
): boolean {
  const a = previous.carry_stacks;
  const b = next.carry_stacks;
  if (a === b) return true;
  if (a.length !== b.length) return false;
  return a.every(
    (stack, index) =>
      stack.item_id === b[index]?.item_id &&
      stack.quantity === b[index]?.quantity,
  );
}

function itemById(itemId: number | undefined): ItemDefinition | undefined {
  return itemId === undefined
    ? undefined
    : host.definitions.items.find(({ id }) => id === itemId);
}

/**
 * The one chip inside a holder, created on first use and patched from then on.
 *
 * Static markup names a holder rather than spelling a chip out, so `createItemChip` stays the only
 * place the chip's shape is written down — which is the whole point of the component. A chip is
 * never rebuilt, so a holder inside a list carrying a control is safe by construction.
 */
const holdersChip = new WeakMap<HTMLElement, HTMLElement>();
function paintChip(
  holder: HTMLElement,
  itemId: number | undefined,
  view: ItemChipView = {},
): HTMLElement {
  let chip = holdersChip.get(holder);
  if (!chip) {
    chip = createItemChip();
    holder.append(chip);
    holdersChip.set(holder, chip);
  }
  fillItemChip(chip, itemById(itemId), itemId, view);
  return chip;
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
      return cell;
    },
  );
  cells.forEach((cell, index) => {
    const stack = stacks[index];
    cell.classList.toggle("filled", Boolean(stack));
    // A slot is dense: the glyph is the identity and the name would not fit, so it rides the
    // chip's own label rather than a second aria string written here.
    paintChip(cell, stack?.item_id, {
      count: stack?.quantity,
      named: false,
      short: true,
    });
    const item = itemById(stack?.item_id);
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

/**
 * The quantity "Fill" asks for: every number a `u32` can hold.
 *
 * Native trims a grant to the room left, so the host does not have to work out what fits — asking
 * for the maximum and letting the simulation answer keeps the carrying rule in the one place that
 * owns it. Anything larger would not survive the trip through the command as an unsigned 32-bit int.
 */
const CREATIVE_FILL = 4_294_967_295;

/**
 * The creative panel: one switch, one pack size, and one row per material.
 *
 * Every control here reads its state out of the snapshot rather than out of the click that changed
 * it. A command native refuses — a grant with no room, a pack size that would strand stock — leaves
 * the snapshot alone, so the control springs back on the next frame instead of the interface
 * carrying on as though the simulation had agreed with it.
 */
function renderCreative(): void {
  const { creative, carry_slots } = snapshot.player;
  creativeChip.classList.toggle("creative-on", creative);
  creativeChip.title = creative
    ? "Creative mode is on (C)"
    : "Creative mode (C)";
  creativeEnabledInput.checked = creative;
  creativeSlotsInput.value = String(carry_slots);
  for (const control of [creativeSlotsInput, creativeClear])
    control.disabled = !creative;

  const rows = syncChildren(
    creativeItems,
    creative ? host.definitions.items.map(({ id }) => String(id)) : [],
    () => {
      const row = document.createElement("div");
      row.className = "creative-item";
      row.setAttribute("role", "listitem");
      // The holder the chip is painted into and the box the buttons live in are made once, here,
      // so neither list reconciles against the other: `syncChildren` deletes any child it does not
      // own, and a chip and a button row sharing one parent would take turns deleting each other.
      const holder = document.createElement("div");
      holder.className = "creative-item-chip";
      const actions = document.createElement("div");
      actions.className = "creative-item-actions";
      row.append(holder, actions);
      return row;
    },
  );
  rows.forEach((row, index) => {
    const item = host.definitions.items[index];
    const holder = row.firstElementChild as HTMLElement | null;
    const actions = row.lastElementChild as HTMLElement | null;
    if (!item || !holder || !actions) return;
    paintChip(holder, item.id, {
      count: heldQuantity(snapshot, item.id),
      named: true,
    });
    // Three amounts cover what anybody actually reaches for: one, a stack, and as much as the pack
    // will take. Native clamps each to the room left, so these are ceilings rather than promises —
    // which is why "Fill" can be an absurd number rather than a quantity the host has to work out.
    const amounts: { label: string; title: string; quantity: number }[] = [
      { label: "+1", title: `Give 1 ${item.name}`, quantity: 1 },
      {
        label: `+${item.stack_size}`,
        title: `Give one stack of ${item.name}`,
        quantity: item.stack_size,
      },
      {
        label: "Fill",
        title: `Fill the pack with ${item.name}`,
        quantity: CREATIVE_FILL,
      },
    ];
    const buttons = syncChildren(
      actions,
      amounts.map(({ label }) => label),
      () => document.createElement("button"),
    );
    buttons.forEach((button, slot) => {
      const amount = amounts[slot];
      if (!amount || !(button instanceof HTMLButtonElement)) return;
      button.type = "button";
      button.textContent = amount.label;
      button.title = amount.title;
      button.setAttribute("aria-label", amount.title);
      button.dataset.itemId = String(item.id);
      button.dataset.quantity = String(amount.quantity);
    });
  });
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
/**
 * Which buildings the catalogue leads with.
 *
 * Unlocked machines, plus what the next affordable research would unlock — the locks a player has
 * a live reason to read. Every late machine at minute zero is a wall, not a catalogue, and the
 * complete list is still one press away.
 */
function catalogueVisible(
  definition: BuildingDefinition,
  reach: Map<number, number>,
): boolean {
  if (showAllBuildings) return true;
  const technology = definition.unlock_technology_id;
  if (technology === undefined) return true;
  return (reach.get(technology) ?? Number.MAX_SAFE_INTEGER) <= DISCLOSURE_REACH;
}

function renderBuildPanel(): void {
  const root = required<HTMLDivElement>("build-groups");
  const buildable = host.definitions.buildings.filter(
    (definition) => definition.buildable,
  );
  const reach = technologyReach();
  const hidden = buildable.filter(
    (definition) => !catalogueVisible(definition, reach),
  ).length;
  const scope = required<HTMLButtonElement>("build-scope");
  scope.textContent = showAllBuildings
    ? "Show what is in reach"
    : hidden > 0
      ? `Show everything (${hidden} locked)`
      : "Show everything";
  scope.setAttribute("aria-pressed", String(showAllBuildings));
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
    const definitions = buildable.filter(
      (definition) =>
        group.holds(definition) && catalogueVisible(definition, reach),
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
  if (definition.supply_radius !== undefined)
    labels.push(`Supplies ${definition.supply_radius}`);
  if (definition.pole_reach !== undefined)
    labels.push(`Links ${definition.pole_reach}`);
  if (definition.capacity !== undefined)
    labels.push(`Holds ${definition.capacity}`);
  if (definition.power_output) labels.push(`+${definition.power_output} power`);
  if (definition.power_draw) labels.push(`−${definition.power_draw} power`);
  if (definition.orientation_axis === "corner") labels.push("Six corners");
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
    availability.cost,
  );
  renderCardRecipes(part<HTMLElement>(card, ".build-recipes"), definition);
}

/** A labelled run of item chips — the shape every cost and every recipe side uses. */
function renderIngredientRow(
  container: HTMLElement,
  ingredients: { item_id: number; quantity: number }[],
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
  fillIngredients(
    part<HTMLElement>(container, ".ingredient-list"),
    ingredients,
    supply,
  );
}

/**
 * One run of ingredients as chips.
 *
 * Passing `supply` states what the player holds against each line, which is the whole of defect
 * one: a card that says "no" without saying which line is short sends the player to another panel
 * to find out. Because the shortfall is a state of the chip, this is one argument at every site
 * that names a quantity the player might be expected to supply rather than four bespoke
 * treatments.
 */
function fillIngredients(
  list: HTMLElement,
  ingredients: { item_id: number; quantity: number }[],
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
    paintChip(node, item_id, {
      count: line ? undefined : quantity,
      progress: line ? { have: line.held, need: line.required } : undefined,
      shortfall: line?.shortfall,
      short: true,
    });
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
    // Inputs are a quantity the player may be expected to supply — early machines are hand-fed
    // through Put long before a belt reaches them — so they are priced against the pack. The
    // output is a result and is only ever an amount.
    fillIngredients(
      part<HTMLElement>(row, ".recipe-in"),
      recipe.inputs,
      costLines(recipe.inputs, snapshot),
    );
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
/**
 * How many unresearched technologies stand between the player and each one, counting itself.
 *
 * Zero is researched, one is available now, two is available after one more. This is the whole of
 * progressive disclosure: both catalogues lead with what the player can reach, and both hand over
 * everything behind one control. It is a distance over the shipped graph rather than a curated
 * list, so a new technology needs no thought here at all.
 */
const DISCLOSURE_REACH = 2;

function technologyReach(): Map<number, number> {
  const all = host.technologies.technologies;
  const researched = new Set(snapshot.researched);
  const depth = new Map<number, number>();
  const measure = (id: number, guard: Set<number>): number => {
    const known = depth.get(id);
    if (known !== undefined) return known;
    if (researched.has(id)) {
      depth.set(id, 0);
      return 0;
    }
    // A cycle is refused natively, so this only guards against a catalogue that never loaded.
    if (guard.has(id)) return Number.MAX_SAFE_INTEGER;
    guard.add(id);
    const technology = all.find((value) => value.id === id);
    const behind = (technology?.prerequisites ?? []).reduce(
      (deepest, prerequisite) =>
        Math.max(deepest, measure(prerequisite, guard)),
      0,
    );
    guard.delete(id);
    const value = behind + 1;
    depth.set(id, value);
    return value;
  };
  for (const technology of all) measure(technology.id, new Set());
  return depth;
}

/**
 * Which technologies the research panel shows by default: everything the player has, everything
 * they can take now, and everything one more breakthrough opens. What that leaves out is the far
 * end of a tree they have no live choice about, which is what made minute zero read as the whole
 * locked game. The full tree stays one press away, because planning is a real reason to want it
 * and hiding it would be a different defect.
 */
function visibleTechnologies(): TechnologyDefinition[] {
  const all = host.technologies.technologies;
  if (showAllTechnologies) return all;
  const reach = technologyReach();
  return all.filter(
    (technology) =>
      (reach.get(technology.id) ?? Number.MAX_SAFE_INTEGER) <= DISCLOSURE_REACH,
  );
}

function renderTechnologies(): void {
  const list = required<HTMLDivElement>("technology-list");
  const technologies = visibleTechnologies();
  const hidden = host.technologies.technologies.length - technologies.length;
  const scope = required<HTMLButtonElement>("research-scope");
  scope.textContent = showAllTechnologies
    ? "Show what is in reach"
    : hidden > 0
      ? `Show the full tree (${hidden} more)`
      : "Show the full tree";
  scope.setAttribute("aria-pressed", String(showAllTechnologies));
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
        (id) =>
          host.technologies.technologies.find((value) => value.id === id)
            ?.name ?? `#${id}`,
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

/** Which way stock moves across a transfer row, and everything that differs because of it. */
const TRANSFER: Record<
  "take" | "put",
  { label: string; command: "withdraw" | "store"; describe: string }
> = {
  take: { label: "Take", command: "withdraw", describe: "out" },
  put: { label: "Put", command: "store", describe: "in" },
};

type TransferDirection = keyof typeof TRANSFER;

/**
 * Both halves of hand transfer, as one function.
 *
 * Take and Put were near-identical: the same row markup, the same glyph call, the same button
 * class, differing only in the data source, the button label, and the command. Two copies of one
 * function is how the two halves drift, and the fractional deposit is the proof — it belongs to
 * both and would otherwise be written twice.
 *
 * The full amount stays the default, because it is what the gesture meant before and what it means
 * now; half and one are further buttons beside it. Native already carries a quantity on `store` and
 * `withdraw`, already clamps it to what the building holds, to what the player can still carry,
 * and to what there is room for, and already reports how much actually moved — the host has only
 * ever sent the maximum. So this adds a number to a command that has always taken one, and no rule
 * about capacity moves to the host.
 *
 * Single is the one amount none of the others can express. Half of three is two, and full is three;
 * a player topping a firebox up by one lump, or pulling one plate off a pile to see what it is, had
 * no control that meant one. Both partial buttons hide when they would only repeat a neighbour:
 * half of one is the whole pile, and of two, half is already one.
 *
 * Patched in place like every list that carries a control: a `replaceChildren` here would drop the
 * press between pointerdown and pointerup.
 */
function renderTransferRows(
  list: HTMLElement,
  entries: { item_id: number; quantity: number }[],
  direction: TransferDirection,
  building: EntitySnapshot | undefined,
  actionable: boolean,
): void {
  const { label, describe } = TRANSFER[direction];
  const rows = syncChildren(
    list,
    entries.map(({ item_id }) => String(item_id)),
    () => {
      const row = document.createElement("div");
      row.className = "inspect-stock-row";
      const holder = document.createElement("div");
      holder.className = "inspect-stock-item chip-host";
      const controls = document.createElement("div");
      controls.className = "transfer-controls";
      const all = document.createElement("button");
      all.type = "button";
      all.className = "withdraw-button";
      const half = document.createElement("button");
      half.type = "button";
      half.className = "withdraw-button transfer-half";
      half.textContent = "½";
      const one = document.createElement("button");
      one.type = "button";
      one.className = "withdraw-button transfer-one";
      one.textContent = "1";
      controls.append(all, half, one);
      row.append(holder, controls);
      return row;
    },
  );
  entries.forEach(({ item_id, quantity }, index) => {
    const row = rows[index];
    if (!row) return;
    const chip = paintChip(part<HTMLElement>(row, ".chip-host"), item_id, {
      count: quantity,
    });
    const name = itemById(item_id)?.name ?? `Item ${item_id}`;
    const controls = part<HTMLElement>(row, ".transfer-controls");
    controls.hidden = !actionable;
    for (const button of controls.querySelectorAll<HTMLButtonElement>(
      "button",
    )) {
      const half = button.classList.contains("transfer-half");
      const one = button.classList.contains("transfer-one");
      const amount = half ? halfTransfer(quantity) : one ? 1 : quantity;
      button.dataset.direction = direction;
      button.dataset.itemId = String(item_id);
      button.dataset.quantity = String(amount);
      button.dataset.q = String(building?.q ?? 0);
      button.dataset.r = String(building?.r ?? 0);
      // Hidden rather than disabled: a control that would move exactly what the button beside it
      // moves is not a lesser option, it is the same option twice. Of two, half is already one, so
      // the single button only earns its place from three up.
      button.hidden = half ? quantity < 2 : one && quantity < 3;
      if (!half && !one) button.textContent = label;
      button.setAttribute(
        "aria-label",
        `${label} ${amount} ${name} ${describe}`,
      );
    }
    chip.title = `${name}: ${quantity}`;
  });
}

/**
 * The kinds a hand can reach into, mirroring `stock_is_reachable_by_hand` in the core.
 *
 * A copy of a native rule, and deliberately so: this decides whether a button is drawn, native
 * decides whether the transfer happens, and native is the authority. Getting this list wrong shows
 * a control that earns a refusal — a cosmetic bug. Leaving it out would show one on every belt.
 */
const HAND_REACHABLE = new Set<string>([
  "container",
  "composer",
  "generator",
  "boiler",
]);

/**
 * What the inspected hex is holding, and what the hand can take back out of it.
 *
 * Take used to belong to containers alone, which made a burner a one-way slot: fuel went in and
 * only demolition got it back. Every kind that holds stock the player can see now grows the
 * control, because seeing stock you cannot touch is the part that read as a bug.
 *
 * A composer still shows its reserved inputs and still will not hand them back — but that costs no
 * rule here, because native keeps reserved inputs in a different map from `inventory`. What this
 * list shows is free stock, so what it offers is exactly what native will give.
 */
function renderInspectorActions(building: EntitySnapshot | undefined): void {
  const stored = building?.inventory ?? [];
  required<HTMLElement>("inspect-stock").hidden = stored.length === 0;
  renderTransferRows(
    required<HTMLDivElement>("inspector-actions"),
    stored,
    "take",
    building,
    HAND_REACHABLE.has(building?.kind ?? ""),
  );
}

/**
 * What the player can put in, so moving stock into a machine is the same gesture as taking it out
 * and sits directly beneath it.
 *
 * Filtered to what the building has a use for, because a pack of twenty item types against a
 * firebox that burns two of them is a list the player has to read rather than act on. The filter is
 * a courtesy and not a rule — native refuses the rest anyway, and says which reason it refused for.
 */
function renderInspectorLoad(building: EntitySnapshot | undefined): void {
  const carried = HAND_REACHABLE.has(building?.kind ?? "")
    ? snapshot.player.carry_stacks.filter(({ item_id }) =>
        acceptsByHand(building, item_id),
      )
    : [];
  // One row per item, not one per stack: a Put moves everything of that item that fits.
  const totals = new Map<number, number>();
  for (const { item_id, quantity } of carried)
    totals.set(item_id, (totals.get(item_id) ?? 0) + quantity);
  required<HTMLElement>("inspect-load").hidden = totals.size === 0;
  renderTransferRows(
    required<HTMLDivElement>("inspector-load"),
    [...totals]
      .sort(([a], [b]) => a - b)
      .map(([item_id, quantity]) => ({ item_id, quantity })),
    "put",
    building,
    true,
  );
}

/**
 * Whether this building has any use for this item, mirroring `accepts_item` in the core.
 *
 * A container takes anything. A firebox takes fuel — and only a burner has one, which is why a wind
 * turbine offers nothing however much coal the player is carrying. A machine takes the inputs of
 * the recipe it is currently set to, and nothing else: an unset composer is a machine with no job,
 * so there is nothing it is waiting for.
 */
function acceptsByHand(
  building: EntitySnapshot | undefined,
  itemId: number,
): boolean {
  if (!building) return false;
  if (building.kind === "container") return true;
  const burnable = Boolean(itemById(itemId)?.fuel_value);
  if (building.kind === "generator" || building.kind === "boiler") {
    const definition = host.definitions.buildings.find(
      ({ id }) => id === building.definition_id,
    );
    // A plant with a power source that is not a firebox has nowhere to put fuel at all.
    const firebox =
      definition?.power_source === undefined ||
      definition.power_source === "burner";
    // And a boiler drinks, which is the one thing it takes that it does not burn.
    const drinks =
      building.kind === "boiler" && itemById(itemId)?.key === "water";
    return (firebox && burnable) || drinks;
  }
  if (building.recipe_id === undefined || building.recipe_id === null)
    return false;
  const recipe = host.definitions.recipes.find(
    ({ id }) => id === building.recipe_id,
  );
  // Fuel counts for a machine that burns as well as crafts, on the same reasoning as the plant:
  // it is not in `inputs`, so a recipe that needs heat has to admit fuel some other way.
  return (
    (recipe?.inputs ?? []).some(({ item_id }) => item_id === itemId) ||
    (burnable && (recipe?.fuel ?? 0) > 0)
  );
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
    renderInspectorHub(undefined);
    return;
  }
  const building = selected ? buildingAt(selected) : undefined;
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
    paintChip(required<HTMLElement>("inspect-field-chip"), resource.item_id, {
      progress: {
        have: resource.quantity,
        need: resource.initial_quantity,
      },
      meter: true,
    });
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
  machine.hidden = !building || building.kind === "hub";
  if (building && building.kind !== "hub") {
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
    // A machine that banks electricity shows its own bank; anything else on the network — a
    // generator, a pole — shows the grid it is part of. One meter, and in both cases the number
    // the player would act on: a bank that keeps hitting zero wants more generation, and a grid
    // whose draw is over its supply says which.
    const banks = Boolean(building.power_capacity);
    required<HTMLElement>("inspect-power-label").textContent = banks
      ? "Charge"
      : "Power";
    setMeter(
      required<HTMLElement>("inspect-power-meter"),
      required<HTMLElement>("inspect-power-fill"),
      required<HTMLElement>("inspect-power-amount"),
      banks ? (building.power_charge ?? 0) : (building.power_satisfied ?? 0),
      banks ? (building.power_capacity ?? 0) : (building.power_demand ?? 0),
      banks || Boolean(building.power_demand),
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
    renderInspectorSwitch(building);
    const cargo = required<HTMLElement>("inspect-cargo");
    cargo.hidden = !building.cargo;
    if (building.cargo)
      paintChip(
        required<HTMLElement>("inspect-cargo-chip"),
        building.cargo.item_id,
        { count: building.cargo.quantity },
      );
  }

  renderInspectorActions(building);
  renderInspectorLoad(building);
  renderInspectorTier(building);
  renderInspectorRecipe(building);
  renderInspectorHub(building);
}

/**
 * Standing requests menu when inspecting the landing hub.
 *
 * Lets the player review available requests, see their carried quantities, and complete requests
 * directly from the landing hub's inspector panel.
 */
function renderInspectorHub(building: EntitySnapshot | undefined): void {
  const hubCard = required<HTMLElement>("inspect-hub");
  if (building?.kind !== "hub") {
    hubCard.hidden = true;
    return;
  }
  hubCard.hidden = false;
  const list = required<HTMLElement>("inspect-hub-requests");
  const requests = snapshot.requests;
  const rows = syncChildren(
    list,
    requests.map((request) => request.key),
    () => {
      const row = document.createElement("li");
      row.className = "inspect-hub-line";
      row.innerHTML = `<span class="inspect-hub-item chip-host"></span><span class="inspect-hub-price"></span><button type="button" class="inspect-hub-deliver">Deliver</button><small class="inspect-hub-brief"></small>`;
      return row;
    },
  );
  requests.forEach((request, index) => {
    const row = rows[index];
    if (!row) return;
    const carried =
      snapshot.player.inventory[String(request.item_id)] ??
      snapshot.player.inventory[request.item_id] ??
      0;
    const haveEnough = carried >= request.required;

    paintChip(part<HTMLElement>(row, ".inspect-hub-item"), request.item_id, {
      progress: { have: carried, need: request.required },
      meter: true,
      shortfall: Math.max(0, request.required - carried),
    });

    const definition = host.definitions.requests.find(
      (value) => value.key === request.key,
    );
    const later = definition?.repeat_insight;
    part(row, ".inspect-hub-price").textContent =
      later !== undefined && later !== request.insight
        ? `+${request.insight} ◆ then +${later}`
        : `+${request.insight} ◆`;
    part(row, ".inspect-hub-brief").textContent = request.brief;

    const button = part<HTMLButtonElement>(row, ".inspect-hub-deliver");
    button.dataset.itemId = String(request.item_id);
    button.disabled = !haveEnough;
    button.classList.toggle("ready", haveEnough);
    button.textContent = haveEnough
      ? "Complete"
      : `Need ${request.required - carried}`;
    button.title = haveEnough
      ? `Deliver ${request.required} ${request.name} to earn insight`
      : `You need ${request.required - carried} more ${request.name} in your pack`;
  });
}

/**
 * The kinds that have work a switch can suspend, mirroring `can_be_switched` in the core. A belt is
 * a lane, a container a shelf, a pole a wire — none of them consume, produce, or burn, so a toggle
 * on them would be a control that changes nothing.
 */
const SWITCHABLE = new Set<string>([
  "extractor",
  "pump",
  "composer",
  "generator",
  "boiler",
]);

/**
 * The manual on/off switch for a working machine.
 *
 * A burner with coal in it burns that coal whether or not anything downstream wants the power, so
 * "stop this while I rebuild the line it feeds" had exactly one answer before: demolish it and pay
 * to rebuild. This is the other answer. Off is total and free — no work, no draw, no fuel — and it
 * keeps everything the machine was holding, so switching back on resumes rather than restarts.
 *
 * The button reads its current state off `status` rather than off a flag of its own, because native
 * already publishes `switched off` as the machine's status and a second source would be a second
 * thing to get out of step. What it sends is the state it wants, not a flip, so a doubled press
 * settles instead of cancelling.
 */
function renderInspectorSwitch(building: EntitySnapshot | undefined): void {
  const button = required<HTMLButtonElement>("inspect-power-switch");
  // Protected objects are protected here too — native refuses, so the host does not offer.
  const switchable =
    Boolean(building) &&
    SWITCHABLE.has(building?.kind ?? "") &&
    !building?.scenario_owned;
  button.hidden = !switchable;
  if (!switchable || !building) return;
  const off = building.status === "switched off";
  button.dataset.q = String(building.q);
  button.dataset.r = String(building.r);
  button.dataset.enable = off ? "1" : "0";
  button.classList.toggle("is-off", off);
  button.textContent = off ? "Switch on" : "Switch off";
  button.title = off
    ? "Resume this machine — it keeps everything it was holding"
    : "Stop this machine without losing its stock, progress, or charge";
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

/**
 * The contract, in the three places it belongs: the permanent mission header, the panel that
 * explains it, and the completion banner.
 *
 * Every number here is published. The bar is the mean of the stage's lines against their own
 * requirements — both halves of each proportion come from native, so the host never infers a
 * maximum by watching a value climb.
 */
function renderContract(): void {
  const contract = snapshot.contract;
  const demo = snapshot.scenario === "factory-demo";
  const lines = contract.requirements.map((need) => ({
    need,
    name: itemById(need.item_id)?.name ?? `Item ${need.item_id}`,
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
    paintChip(row, need.item_id, {
      progress: { have: need.delivered, need: need.required },
      meter: true,
      shortfall: Math.max(0, need.required - need.delivered),
    });
  });
}

/**
 * The hub's request board.
 *
 * Every number on a row is published: what is wanted, how much has arrived, and — the reason the
 * board exists at all — what filling it pays, stated before anything is handed over. The insight
 * that used to appear from a delivery of anything at all is now something the player can read off
 * the wall before walking anywhere.
 *
 * The list carries a control, so it is patched in place rather than rebuilt: a `replaceChildren`
 * here would drop the press between pointerdown and pointerup, which is the defect `syncChildren`
 * exists for.
 */
function renderRequests(): void {
  const board = required<HTMLElement>("request-board");
  const rows = syncChildren(
    board,
    snapshot.requests.map((request) => request.key),
    () => {
      const row = document.createElement("li");
      row.className = "request-line";
      row.innerHTML = `<span class="request-item chip-host"></span><span class="request-price"></span><small class="request-brief"></small>`;
      return row;
    },
  );
  snapshot.requests.forEach((request, index) => {
    const row = rows[index];
    if (!row) return;
    const carried =
      snapshot.player.inventory[String(request.item_id)] ??
      snapshot.player.inventory[request.item_id] ??
      0;
    // Same chip as the bill and the pack: reflects how many units the player carries against
    // what the request asks for, so it is obvious when you have enough to complete it.
    paintChip(part<HTMLElement>(row, ".request-item"), request.item_id, {
      progress: { have: carried, need: request.required },
      meter: true,
      shortfall: Math.max(0, request.required - carried),
    });
    const definition = host.definitions.requests.find(
      (value) => value.key === request.key,
    );
    const later = definition?.repeat_insight;
    part(row, ".request-price").textContent =
      later !== undefined && later !== request.insight
        ? `+${request.insight} ◆ then +${later}`
        : `+${request.insight} ◆`;
    part(row, ".request-brief").textContent = request.brief;
  });
  required<HTMLElement>("requests-detail").hidden = rows.length === 0;
}

/**
 * The next step, in the panel and in the permanent chrome, from one derivation.
 *
 * `nextAction` reads the contract and the catalogues rather than a branch ladder, so this is only
 * layout. See `src/core/guidance.ts` for why the script had to go.
 */
function renderNextAction(): void {
  const guidance = nextAction(snapshot, host.definitions, host.technologies);
  required<HTMLElement>("next-action-title").textContent = guidance.title;
  required<HTMLElement>("next-action-detail").textContent = guidance.detail;
  required<HTMLElement>("next-step-title").textContent = guidance.title;
  required<HTMLElement>("next-step-detail").textContent = guidance.detail;
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

/**
 * Sound is optional to the player, never absent from the product. The control is in permanent
 * chrome beside pause for the same reason: a game that only makes noise is a game somebody has to
 * mute by leaving.
 */
function setMuted(value: boolean): void {
  audio.setMuted(value);
  muteInput.checked = value;
  titleMuteInput.checked = value;
  soundButton.textContent = value ? "♪̸" : "♪";
  soundButton.setAttribute("aria-pressed", String(!value));
  soundButton.setAttribute(
    "aria-label",
    value ? "Unmute feedback sounds" : "Mute feedback sounds",
  );
  soundButton.title = value
    ? "Unmute feedback sounds (M)"
    : "Mute feedback sounds (M)";
}

function setReducedMotion(value: boolean): void {
  reduceMotionInput.checked = value;
  titleReduceMotionInput.checked = value;
  renderer.setReducedMotion(value);
  try {
    window.localStorage.setItem(MOTION_KEY, value ? "1" : "0");
  } catch {
    // The preference is lost, the session is not.
  }
}

function setGraphicsProfile(value: GraphicsProfile): void {
  graphicsProfileInput.value = value;
  titleGraphicsProfileInput.value = value;
  renderer.setGraphicsProfile(value);
  try {
    window.localStorage.setItem(GRAPHICS_STORAGE_KEY, value);
  } catch {
    // The preference is lost, the factory is not.
  }
}

function loadReducedMotion(): boolean {
  try {
    return window.localStorage.getItem(MOTION_KEY) === "1";
  } catch {
    return false;
  }
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
  titleScenarioInput.value = next.scenario;
  seedInput.value = String(next.seed);
  titleSeedInput.value = String(next.seed);
  titleCreativeInput.checked = next.player.creative;
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

/**
 * Keep the tile under a stationary mouse highlighted when following, orbiting, or zooming moves
 * the camera beneath it. Pointer movement is not emitted when only the scene moves, so retaining
 * an axial coordinate here would leave the old highlight behind in the world.
 */
function syncHoverWithCamera(): void {
  if (!aimPointer || panPointer || harvestPointer || dragBuild) return;
  const coordinate = renderer.pick(aimPointer.x, aimPointer.y);
  if (hover?.q === coordinate.q && hover.r === coordinate.r) return;
  hover = coordinate;
  refreshHoverPreview();
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
soundButton.addEventListener("click", () => setMuted(!audio.isMuted));
muteInput.addEventListener("change", () => setMuted(muteInput.checked));
reduceMotionInput.addEventListener("change", () =>
  setReducedMotion(reduceMotionInput.checked),
);
graphicsProfileInput.addEventListener("change", () => {
  const profile = parseGraphicsProfile(graphicsProfileInput.value);
  if (profile) setGraphicsProfile(profile);
});
titleGraphicsProfileInput.addEventListener("change", () => {
  const profile = parseGraphicsProfile(titleGraphicsProfileInput.value);
  if (profile) setGraphicsProfile(profile);
});
required<HTMLButtonElement>("research-scope").addEventListener("click", () => {
  showAllTechnologies = !showAllTechnologies;
  renderTechnologies();
});
required<HTMLButtonElement>("build-scope").addEventListener("click", () => {
  showAllBuildings = !showAllBuildings;
  renderBuildPanel();
});
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
// Delegated, because the hub menu rows come and go with every fill.
required<HTMLElement>("inspect-hub-requests").addEventListener(
  "click",
  (event) => {
    const deliver = (event.target as HTMLElement).closest<HTMLButtonElement>(
      ".inspect-hub-deliver",
    );
    if (!deliver || deliver.disabled) return;
    const itemId = Number(deliver.dataset.itemId);
    if (Number.isInteger(itemId)) {
      enqueue({ type: "deposit", item_id: itemId });
    } else {
      enqueue({ type: "deposit" });
    }
  },
);
required<HTMLButtonElement>("recenter").addEventListener("click", () =>
  renderer.recenter(),
);
required<HTMLButtonElement>("orbit-left").addEventListener("click", () =>
  orbitView(-1),
);
required<HTMLButtonElement>("orbit-right").addEventListener("click", () =>
  orbitView(1),
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
type WorldScalar = Exclude<keyof WorldParams, "site_rules">;

const WORLD_PARAMETER_FIELDS: {
  key: WorldScalar;
  label: string;
  min: number;
  max: number;
}[] = [
  { key: "elevation_coarse_cell", label: "Landform scale", min: 1, max: 1024 },
  { key: "elevation_fine_cell", label: "Detail scale", min: 1, max: 1024 },
  {
    key: "elevation_coarse_weight",
    label: "Landform share %",
    min: 0,
    max: 100,
  },
  { key: "moisture_cell", label: "Moisture scale", min: 1, max: 1024 },
  { key: "richness_cell", label: "Richness scale", min: 1, max: 1024 },
  { key: "water_level", label: "Sea level", min: 0, max: 65535 },
  { key: "shore_level", label: "Shore level", min: 0, max: 65535 },
  { key: "hills_level", label: "Hills level", min: 0, max: 65535 },
  { key: "highland_level", label: "Highland level", min: 0, max: 65535 },
  { key: "cliff_step", label: "Cliff steepness", min: 1, max: 65535 },
  { key: "deep_water_moisture", label: "Deep water", min: -1, max: 65535 },
  { key: "site_cell", label: "Deposit spacing", min: 1, max: 1024 },
  { key: "site_jitter", label: "Deposit wander", min: 0, max: 16 },
  { key: "river_cell", label: "River spacing", min: 1, max: 1024 },
  { key: "river_width", label: "River width", min: 0, max: 65535 },
  { key: "river_max_elevation", label: "River ceiling", min: 0, max: 65535 },
  { key: "ocean_level", label: "Ocean cut", min: 0, max: 65535 },
];

/** What Start scenario will generate. Native validates it again on arrival. */
let pendingWorld: WorldParams | null = null;
const worldParameterInputs = new Map<WorldScalar, HTMLInputElement>();
const titleWorldParameterInputs = new Map<WorldScalar, HTMLInputElement>();

for (const preset of host.worldPresets) {
  const option = document.createElement("option");
  option.value = preset.key;
  option.textContent = preset.name;
  worldPresetInput.append(option);

  const titleOption = document.createElement("option");
  titleOption.value = preset.key;
  titleOption.textContent = preset.name;
  titleWorldPresetInput.append(titleOption);
}
// A hand-edited parameter set is no preset, and saying so is what keeps the picker honest about
// what is about to be generated.
const customOption = document.createElement("option");
customOption.value = "custom";
customOption.textContent = "Custom";
customOption.hidden = true;
worldPresetInput.append(customOption);

const titleCustomOption = document.createElement("option");
titleCustomOption.value = "custom";
titleCustomOption.textContent = "Custom";
titleCustomOption.hidden = true;
titleWorldPresetInput.append(titleCustomOption);

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
    titleCustomOption.hidden = false;
    worldPresetInput.value = "custom";
    titleWorldPresetInput.value = "custom";
    const matching = titleWorldParameterInputs.get(field.key);
    if (matching && matching !== control) matching.value = String(value);
  });
  label.append(control);
  worldParameterFields.append(label);
  worldParameterInputs.set(field.key, control);

  const titleLabel = document.createElement("label");
  titleLabel.textContent = field.label;
  const titleControl = document.createElement("input");
  titleControl.type = "number";
  titleControl.min = String(field.min);
  titleControl.max = String(field.max);
  titleControl.setAttribute("aria-label", field.label);
  titleControl.addEventListener("input", () => {
    if (!pendingWorld) return;
    const value = Number(titleControl.value);
    if (!Number.isSafeInteger(value)) return;
    pendingWorld = { ...pendingWorld, [field.key]: value };
    customOption.hidden = false;
    titleCustomOption.hidden = false;
    worldPresetInput.value = "custom";
    titleWorldPresetInput.value = "custom";
    const matching = worldParameterInputs.get(field.key);
    if (matching && matching !== titleControl) matching.value = String(value);
  });
  titleLabel.append(titleControl);
  titleWorldParameterFields.append(titleLabel);
  titleWorldParameterInputs.set(field.key, titleControl);
}

function showWorldParams(params: WorldParams): void {
  pendingWorld = params;
  for (const [key, control] of worldParameterInputs) {
    control.value = String(params[key]);
  }
  for (const [key, control] of titleWorldParameterInputs) {
    control.value = String(params[key]);
  }
  const preset = host.presetKeyFor(params);
  customOption.hidden = preset !== undefined;
  titleCustomOption.hidden = preset !== undefined;
  worldPresetInput.value = preset ?? "custom";
  titleWorldPresetInput.value = preset ?? "custom";
  const desc =
    host.worldPresets.find((entry) => entry.key === (preset ?? "custom"))
      ?.description ?? "Hand-tuned parameters.";
  worldPresetDescription.textContent = desc;
  titleWorldPresetDescription.textContent = desc;
}

worldPresetInput.addEventListener("change", () => {
  const preset = host.worldPresets.find(
    (entry) => entry.key === worldPresetInput.value,
  );
  if (preset) showWorldParams(structuredClone(preset.params));
});

titleWorldPresetInput.addEventListener("change", () => {
  const preset = host.worldPresets.find(
    (entry) => entry.key === titleWorldPresetInput.value,
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

titleWorldParametersReset.addEventListener("click", () => {
  const preset =
    host.worldPresets.find(
      (entry) => entry.key === titleWorldPresetInput.value,
    ) ?? host.worldPresets[0];
  if (preset) showWorldParams(structuredClone(preset.params));
});

scenarioInput.addEventListener("input", () => {
  titleScenarioInput.value = scenarioInput.value;
});
titleScenarioInput.addEventListener("input", () => {
  scenarioInput.value = titleScenarioInput.value;
});
seedInput.addEventListener("input", () => {
  titleSeedInput.value = seedInput.value;
});
titleSeedInput.addEventListener("input", () => {
  seedInput.value = titleSeedInput.value;
});
titleSeedRandom.addEventListener("click", () => {
  const randomized = Math.floor(Math.random() * 4294967295);
  seedInput.value = String(randomized);
  titleSeedInput.value = String(randomized);
});

function openTitleScreen(): void {
  titleScreen.classList.add("open");
  titleResume.hidden = false;
  setPlaying(false);
  updateContinueState();
}

function closeTitleScreen(): void {
  titleScreen.classList.remove("open");
  titleResume.hidden = false;
  canvas.focus();
  setPlaying(true);
}

function switchTitleTab(tab: "saves" | "new"): void {
  const showSaves = tab === "saves";
  titleTabSaves.classList.toggle("active", showSaves);
  titleTabSaves.setAttribute("aria-selected", String(showSaves));
  titleTabNew.classList.toggle("active", !showSaves);
  titleTabNew.setAttribute("aria-selected", String(!showSaves));
  titleSavesView.hidden = !showSaves;
  titleSavesView.classList.toggle("active", showSaves);
  titleNewGameView.hidden = showSaves;
  titleNewGameView.classList.toggle("active", !showSaves);
}

titleTabSaves.addEventListener("click", () => switchTitleTab("saves"));
titleTabNew.addEventListener("click", () => switchTitleTab("new"));
titleResume.addEventListener("click", () => closeTitleScreen());
sessionMainMenu.addEventListener("click", () => {
  closePanels();
  openTitleScreen();
});
titleMuteInput.addEventListener("change", () =>
  setMuted(titleMuteInput.checked),
);
titleReduceMotionInput.addEventListener("change", () =>
  setReducedMotion(titleReduceMotionInput.checked),
);

titleContinue.addEventListener("click", () => {
  const slot = latestCompatible(
    readCatalog(localStorage).slots,
    currentBuild(),
  );
  if (slot) {
    void loadSlot(slot);
    closeTitleScreen();
  }
});

titleStartGame.addEventListener("click", async () => {
  input.clear();
  const parsedSeed = Number(titleSeedInput.value);
  const seed =
    Number.isSafeInteger(parsedSeed) &&
    parsedSeed >= 0 &&
    parsedSeed <= 0xffffffff
      ? parsedSeed
      : undefined;
  try {
    const next = await host.newGame(
      titleScenarioInput.value,
      seed,
      pendingWorld ?? undefined,
      titleCreativeInput.checked,
    );
    beginRun(next);
    update(next);
    syncSessionInputs(next);
    renderer.recenter();
    closeTitleScreen();
    closePanels();
  } catch (error) {
    reportWorkerError(error);
  }
});

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
    // A new run started from inside a creative session stays creative. The switch is in the panel
    // two rails over; making the player find it again after every restart would be the interface
    // forgetting something it was told.
    const next = await host.newGame(
      scenarioInput.value,
      seed,
      pendingWorld ?? undefined,
      snapshot.player.creative,
    );
    beginRun(next);
    update(next);
    syncSessionInputs(next);
    renderer.recenter();
    setPlaying(true);
    closePanels();
  } catch (error) {
    reportWorkerError(error);
  }
});
// Every creative control sends a command and then waits: none of them writes the state it is
// showing. `renderCreative` sets each one from the next snapshot, so a refusal native reports —
// a pack size that would strand carried stock, a grant with nowhere to go — shows up as the
// control returning to what the simulation actually holds, with the reason in the toast.
creativeEnabledInput.addEventListener("change", () => {
  enqueue({ type: "set_creative", enabled: creativeEnabledInput.checked });
});
creativeSlotsInput.addEventListener("change", () => {
  const slots = Number(creativeSlotsInput.value);
  if (!Number.isSafeInteger(slots) || slots < 1) {
    creativeSlotsInput.value = String(snapshot.player.carry_slots);
    return;
  }
  enqueue({ type: "set_carry_slots", slots });
});
creativeClear.addEventListener("click", () => {
  enqueue({ type: "discard" });
});
creativeItems.addEventListener("click", (event) => {
  const button = (event.target as HTMLElement).closest("button");
  if (!button) return;
  const item_id = Number(button.dataset.itemId);
  const quantity = Number(button.dataset.quantity);
  if (!Number.isSafeInteger(item_id) || !Number.isSafeInteger(quantity)) return;
  enqueue({ type: "grant", item_id, quantity });
});

// Ticks bought at a different price are a different run. The clock keeps counting either way; it
// just stops claiming the result can be compared against one that did not move the slider.
speedInput.addEventListener("change", () => {
  if (!run || Number(speedInput.value) === run.startedSpeed) return;
  run = taintRun(run, "speed-changed");
  writeRun(localStorage, run);
  renderRun();
});

required<HTMLButtonElement>("run-copy").addEventListener("click", async () => {
  const status = required<HTMLElement>("run-status");
  if (!run) {
    status.textContent = "Nothing timed yet.";
    return;
  }
  const report = formatRunReport(run);
  try {
    await navigator.clipboard.writeText(report);
    status.textContent = "Report copied.";
  } catch {
    // Clipboard permission is not guaranteed, and losing the report to a denied prompt would be
    // worse than a fallback that asks the player to copy it themselves.
    status.textContent = report;
  }
});

required<HTMLButtonElement>("run-reset").addEventListener("click", () => {
  runElapsedMs = 0;
  run = startRun(Date.now(), snapshot.tick, Number(speedInput.value));
  writeRun(localStorage, run);
  renderRun();
  required<HTMLElement>("run-status").textContent = "Timer reset.";
});

required<HTMLButtonElement>("save").addEventListener("click", async () => {
  try {
    const payload = await host.save();
    const build = currentBuild();
    const named = saveNameInput.value.trim();
    const selected = selectedSaveId
      ? readCatalog(localStorage).slots.find(
          (slot) => slot.id === selectedSaveId,
        )
      : undefined;
    const overwriteName =
      named || selected?.name || snapshot.scenario_name || "Save";
    const drafted = slotFromPayload(
      payload,
      overwriteName,
      build,
      Date.now(),
      selected &&
        (!named ||
          named.toLocaleLowerCase() === selected.name.toLocaleLowerCase())
        ? selected.id
        : undefined,
    );
    if (!drafted) {
      updateContinueState("Save failed: the envelope was not readable HXF1.");
      return;
    }
    const { slots, error } = readCatalog(localStorage);
    if (error) {
      updateContinueState(error);
      return;
    }
    const nextSlots =
      drafted.id === selected?.id
        ? upsertSlot(slots, drafted)
        : replaceNamedSlot(slots, drafted);
    writeCatalog(localStorage, nextSlots);
    selectedSaveId = drafted.id;
    saveNameInput.value = drafted.name;
    updateContinueState(`Saved “${drafted.name}”.`);
    showFeedback("Game saved");
  } catch (error) {
    updateContinueState(`Save failed: ${String(error)}`);
  }
});
required<HTMLButtonElement>("continue").addEventListener("click", () => {
  const slot = latestCompatible(
    readCatalog(localStorage).slots,
    currentBuild(),
  );
  if (slot) void loadSlot(slot);
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
required<HTMLButtonElement>("inspect-power-switch").addEventListener(
  "click",
  (event) => {
    const button = event.currentTarget as HTMLButtonElement;
    enqueue({
      type: "set_enabled",
      q: Number(button.dataset.q),
      r: Number(button.dataset.r),
      // The state the press is asking for, read off the button rather than off the machine: by the
      // time this lands the snapshot may have moved, and a flip would then land the wrong way up.
      enabled: button.dataset.enable === "1",
    });
  },
);
// One listener for both lists, because the row that raised the press already says which way the
// stock is moving. The button carries the amount, so the full, half, and single controls are one
// path.
for (const id of ["inspector-actions", "inspector-load"])
  required<HTMLDivElement>(id).addEventListener("click", (event) => {
    const button = (event.target as Element).closest<HTMLButtonElement>(
      "button[data-item-id]",
    );
    const direction = button?.dataset.direction;
    if (!button || (direction !== "take" && direction !== "put")) return;
    enqueue({
      type: TRANSFER[direction].command,
      q: Number(button.dataset.q),
      r: Number(button.dataset.r),
      item_id: Number(button.dataset.itemId),
      quantity: Number(button.dataset.quantity),
    });
  });

function currentMovementIntent(running = false): NativeInputCommand {
  return movementIntent(pressedMovement, running, (x, y) =>
    renderer.screenMovement(x, y),
  );
}

function orbitView(step: -1 | 1): void {
  renderer.orbitBy(step);
  syncHoverWithCamera();
  if (pressedMovement.size) enqueue(currentMovementIntent(runningHeld));
}

window.addEventListener("keydown", (event) => {
  if (isTypingTarget(event.target)) return;
  // Space presses a button the keyboard tabbed to. A mouse-focused button must not keep it:
  // activation happens on keyup, so returning here would both skip recenter and click the control.
  if (event.code === "Space" && isKeyboardFocusedControl(event.target)) return;
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
      enqueue(currentMovementIntent(event.shiftKey));
    }
    return;
  }
  // Shift is a gait, not a key: it changes an intent already in flight, so it has to resend one.
  // Held on its own it does nothing, which is what makes it safe to press at any time.
  if (event.code === "ShiftLeft" || event.code === "ShiftRight") {
    runningHeld = true;
    if (pressedMovement.size) enqueue(currentMovementIntent(true));
    return;
  }
  if (event.code === "Escape") {
    if (titleScreen.classList.contains("open") && !titleResume.hidden) {
      closeTitleScreen();
      return;
    }
    selectTool("inspect");
    closePanels();
  }
  // Space centres the camera, which is what the button beside it does and what a player who has
  // panned away needs most. Pause moved to T rather than fighting it for the key.
  else if (event.code === "Space") renderer.recenter();
  else if (event.code === "Comma") orbitView(-1);
  else if (event.code === "Period") orbitView(1);
  else if (event.code === "KeyT") setPlaying(!playing);
  else if (event.code === "KeyM") setMuted(!audio.isMuted);
  else if (event.code in PANEL_KEYS)
    togglePanel(PANEL_KEYS[event.code] as string);
  else if (event.code === "KeyF") {
    // Held rather than tapped. A swing has to be worked through natively before it pays, so the
    // repeat cannot outrun the simulation however fast the frames arrive.
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
  if (
    event.code === "Space" &&
    !isTypingTarget(event.target) &&
    !isKeyboardFocusedControl(event.target)
  ) {
    // Buttons fire on Space keyup. Recenter already handled keydown; this stops the click.
    event.preventDefault();
  }
  if (event.code === "KeyF") gatherHeld = false;
  if (event.code === "ShiftLeft" || event.code === "ShiftRight") {
    runningHeld = event.shiftKey;
    if (pressedMovement.size) enqueue(currentMovementIntent(runningHeld));
    return;
  }
  if (!pressedMovement.delete(event.code)) return;
  event.preventDefault();
  // Stopping is sent on the same frame the key comes up. Coalescing the release made every stop
  // read as a slide, which is the kind of latency a player feels without being able to name it.
  enqueue(currentMovementIntent(event.shiftKey));
});

window.addEventListener("blur", () => {
  gatherHeld = false;
  runningHeld = false;
  stopAiming();
  if (!pressedMovement.size) return;
  pressedMovement.clear();
  enqueue(currentMovementIntent());
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
  if (harvestPointer?.id === event.pointerId) {
    // The hold walks to the hex under the cursor and keeps working from there. Selecting it is what
    // makes the target visible, which matters more here than for a click: the gesture repeats.
    if (
      coordinate.q !== harvestPointer.q ||
      coordinate.r !== harvestPointer.r
    ) {
      harvestPointer = {
        id: event.pointerId,
        q: coordinate.q,
        r: coordinate.r,
      };
      selected = coordinate;
      renderer.setSelection(coordinate);
      renderInspector();
    }
    hover = coordinate;
    refreshHoverPreview();
    return;
  }
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
  if (event.button === 2) {
    // A right press starts working the hex under it straight away and keeps working it while the
    // button is down; the frame loop repeats it and the swing already running paces the repeat,
    // exactly as a held F is paced. Dragging moves the hold to the next hex rather than cancelling
    // it — the camera is on the middle button and no longer wants this gesture.
    const harvest = renderer.pick(event.clientX, event.clientY);
    harvestPointer = { id: event.pointerId, q: harvest.q, r: harvest.r };
    selected = harvest;
    renderer.setSelection(harvest);
    enqueue({ type: "gather_at", ...harvest });
    renderInspector();
    // Captured last: capture is what keeps the gesture alive off the canvas, not what makes the
    // press mean something. Taking it first would let a refused capture swallow the first harvest
    // while still leaving the hold armed.
    canvas.setPointerCapture(event.pointerId);
    event.preventDefault();
    return;
  }
  if (event.button === 1 || event.shiftKey) {
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
    syncHoverWithCamera();
    return;
  }
  if (harvestPointer?.id === event.pointerId) {
    canvas.releasePointerCapture(event.pointerId);
    // Releasing ends the hold. The harvest began on the press and repeated every frame since.
    harvestPointer = null;
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
  if (harvestPointer?.id === event.pointerId) harvestPointer = null;
  endDrag(event.pointerId);
});
canvas.addEventListener("pointerleave", () => {
  stopAiming();
  if (!panPointer && !harvestPointer && !dragBuild) {
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
    syncHoverWithCamera();
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
      typeof tool === "number" ? null : target && buildingAt(target);
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
  const building = target ? buildingAt(target) : undefined;
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

/** The top entity at a cell; bridge supports are placed before the transport they carry. */
function buildingAt(coordinate: {
  q: number;
  r: number;
}): EntitySnapshot | undefined {
  return snapshot.buildings.findLast(({ footprint }) =>
    footprint.some(({ q, r }) => q === coordinate.q && r === coordinate.r),
  );
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
    // Corner headings are closed under 60° rotation. Definitions remain single-cell until one
    // genuinely needs a wider footprint, so this is currently exact and future-proof.
    orientation >= NORTH ? orientation - NORTH : orientation,
  );
  // Placing a pole shows what it would light before it is paid for, which is the difference
  // between choosing where a pole goes and finding out afterwards.
  renderer.setBuildReach(definition ?? null);
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
  return definition?.orientation_axis === "corner"
    ? { start: NORTH, end: DIRECTION_NAMES.length }
    : { start: 0, end: NORTH };
}

function rotateNewBuilding(): void {
  const { start, end } = orientationRange(tool);
  // Rotation stays on the tool's own axis: a belt walks the six edges and a riser flips between
  // six corners. `rotateHexDirection` still turns the six edges, so the package keeps owning the
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

/**
 * Open or close one panel, independently of every other one.
 *
 * Opening any panel used to close all the others, and that was not a policy that could simply be
 * deleted: the pack, research, catalogue, and objective panels all sat at one origin, so removing
 * the exclusivity would have stacked four panels on top of each other. The rails are what make
 * this a layout question rather than a flag — see `.panel-rail` in `src/styles.css`.
 *
 * Below the rail breakpoint there is only room for one at a time, so the old behaviour is what the
 * narrow layout keeps.
 */
const ONE_PANEL_AT_A_TIME = window.matchMedia("(max-width: 720px)");

function togglePanel(id: string): void {
  const target = document.getElementById(id);
  if (!target) return;
  const opening = !target.classList.contains("open");
  if (opening && ONE_PANEL_AT_A_TIME.matches) closePanels(target);
  target.classList.toggle("open", opening);
  syncPanelToggles();
  savePanelState();
}

/**
 * Which panels are open, in `localStorage`, on exactly the terms the hotbar arrangement already
 * sets: never saved with the game, never hashed, never sent. It is a preference about a screen,
 * not a fact about a factory.
 */
const PANEL_KEY = "hexfactory:panels:v1";

function openPanelIds(): string[] {
  return [...document.querySelectorAll<HTMLElement>(".glass-panel.open")].map(
    ({ id }) => id,
  );
}

function savePanelState(): void {
  try {
    localStorage.setItem(PANEL_KEY, JSON.stringify(openPanelIds()));
  } catch {
    // A browser with storage refused is a browser that opens panels fresh, not a broken one.
  }
}

function loadPanelState(): void {
  let stored: unknown;
  try {
    stored = JSON.parse(localStorage.getItem(PANEL_KEY) ?? "[]");
  } catch {
    return;
  }
  if (!Array.isArray(stored)) return;
  // A stored id is validated against the live document, exactly as a stored hotbar slot is
  // validated against the live catalogue: a panel that no longer exists is dropped.
  const ids = stored.filter(
    (id): id is string =>
      typeof id === "string" &&
      document.getElementById(id)?.classList.contains("glass-panel") === true,
  );
  const restore = ONE_PANEL_AT_A_TIME.matches ? ids.slice(0, 1) : ids;
  for (const id of restore) document.getElementById(id)?.classList.add("open");
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
  // In-game time: the run clock stops with the factory and behind the title screen, so reading a
  // recipe with the game paused costs a player nothing.
  if (run && playing && !titleScreen.classList.contains("open"))
    runElapsedMs += elapsed;
  if (playing) accumulator += elapsed * Number(speedInput.value);
  // Walking is paced by native's cadence against elapsed real time, not by the tick the factory
  // happens to be running, so a paused or slowed factory no longer pins the player in place. The
  // same clock spends the work one field action costs, and a swing only pays out on the step that
  // finishes it, so it has to keep running for a player standing still working a hex.
  if (pressedMovement.size || snapshot.player.action_cooldown > 0)
    playerAccumulator += elapsed * host.playerTicksPerSecond;
  else playerAccumulator = 0;
  if (!advancePending) {
    // A held gather repeats at frame rate and is paced natively by the swing already running, so
    // the player holds the key instead of tapping it once per unit. A held right-click is the same
    // idea aimed at a named hex, and it outranks the untargeted one: if both are held, the hex the
    // player is pointing at is the one they chose.
    if (!input.size) {
      if (harvestPointer)
        input.enqueue({
          type: "gather_at",
          q: harvestPointer.q,
          r: harvestPointer.r,
        });
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
  if (
    playing &&
    !titleScreen.classList.contains("open") &&
    now - lastAutoSaveTime >= AUTOSAVE_INTERVAL_MS
  ) {
    lastAutoSaveTime = now;
    void triggerAutoSave();
  }
  renderer.setGathering(gatherHeld || harvestPointer !== null);
  renderer.renderFrame(now);
  requestAnimationFrame(frame);
}

async function triggerAutoSave(silent = true): Promise<void> {
  if (autoSavePending || titleScreen.classList.contains("open")) return;
  autoSavePending = true;
  try {
    const payload = await host.save();
    const build = currentBuild();
    const drafted = slotFromPayload(
      payload,
      AUTOSAVE_SLOT_NAME,
      build,
      Date.now(),
    );
    if (!drafted) return;
    const { slots, error } = readCatalog(localStorage);
    if (error) return;
    const nextSlots = replaceNamedSlot(slots, drafted);
    writeCatalog(localStorage, nextSlots);
    lastAutoSaveTime = performance.now();
    updateContinueState();
    if (!silent) showFeedback("Factory auto-saved");
  } catch {
    // Non-fatal if auto-save fails (e.g. quota or blocked storage)
  } finally {
    autoSavePending = false;
  }
}

document.addEventListener("visibilitychange", () => {
  if (
    document.visibilityState === "hidden" &&
    !titleScreen.classList.contains("open")
  ) {
    void triggerAutoSave();
  }
});

window.addEventListener("pagehide", () => {
  if (!titleScreen.classList.contains("open")) {
    void triggerAutoSave();
  }
});

window.addEventListener("beforeunload", () => {
  if (!titleScreen.classList.contains("open")) {
    void triggerAutoSave();
  }
});

function updateContinueState(message?: string): void {
  const build = currentBuild();
  let slots: SaveSlot[] = [];
  let imported = 0;
  let error: string | undefined;
  try {
    const pulled = importLegacySlots(localStorage, build);
    imported = pulled.imported;
    const read =
      imported > 0 ? { slots: pulled.slots } : readCatalog(localStorage);
    slots = read.slots;
    error = "error" in read ? read.error : undefined;
  } catch (caught) {
    error = `Save list failed: ${String(caught)}`;
  }
  const compatible = latestCompatible(slots, build);
  required<HTMLButtonElement>("continue").disabled = !compatible;
  titleContinue.disabled = !compatible;
  titleContinueSub.textContent = compatible
    ? `Restore “${compatible.name}”`
    : "No saved factory found";
  titleSavesBadge.textContent = String(slots.length);
  renderSaveSlots(slots, build);
  renderTitleSaveSlots(slots, build);
  // Read off the build rather than typed into the markup. The literal in index.html still said
  // "Definitions 11" two catalog bumps later, because nothing was keeping it honest.
  required<HTMLElement>("title-envelope-info").textContent =
    `Save ${build.versions.save} · Definitions ${build.versions.definitions} · World ${build.versions.world}`;
  const importedNote =
    imported > 0
      ? `Imported ${imported} previous run${imported === 1 ? "" : "s"} from an older slot. `
      : "";
  const scenarioVersion =
    host.scenarios.scenarios.find(
      (scenario) => scenario.key === snapshot.scenario,
    )?.version ?? 0;
  required<HTMLElement>("save-status").textContent =
    message ??
    importedNote +
      (error
        ? error
        : compatible
          ? `Continue loads “${compatible.name}”. This build is ${formatVersions({ ...build.versions, scenario: scenarioVersion })}.`
          : slots.length > 0
            ? "Saved runs are listed below. None of them can load in this build."
            : "No local save yet.");
}

function renderSaveSlots(slots: SaveSlot[], build: CurrentBuild): void {
  const board = required<HTMLElement>("save-slots");
  const ordered = slotsNewestFirst(slots);
  const rows = syncChildren(
    board,
    ordered.map((slot) => slot.id),
    () => {
      const row = document.createElement("li");
      row.className = "save-slot";
      row.innerHTML = `<button type="button" class="save-slot-select"><strong></strong><span class="save-slot-when"></span><span class="save-slot-config"></span><span class="save-slot-versions"></span><span class="save-slot-issue"></span></button><button type="button" class="save-slot-load">Load</button><button type="button" class="save-slot-delete">Delete</button>`;
      return row;
    },
  );
  ordered.forEach((slot, index) => {
    const row = rows[index];
    if (!row) return;
    const envelope = parseHxf1(slot.payload);
    const check = envelope
      ? compatibility(envelope, build)
      : {
          compatible: false,
          mismatches: [
            {
              field: "save",
              expected: "a readable HXF1 file",
              found: "unreadable",
            },
          ],
        };
    row.classList.toggle("selected", slot.id === selectedSaveId);
    row.classList.toggle("incompatible", !check.compatible);
    part(row, "strong").textContent = slot.name;
    part(row, ".save-slot-when").textContent = formatSavedAt(slot.savedAt);
    part(row, ".save-slot-config").textContent = formatConfig(slot.config);
    part(row, ".save-slot-versions").textContent = formatVersions(
      slot.versions,
    );
    part(row, ".save-slot-issue").textContent = check.compatible
      ? ""
      : describeMismatches(check.mismatches);
    const select = part<HTMLButtonElement>(row, ".save-slot-select");
    select.dataset.slotId = slot.id;
    select.setAttribute("aria-pressed", String(slot.id === selectedSaveId));
    select.setAttribute("aria-label", `Select save ${slot.name}`);
    const load = part<HTMLButtonElement>(row, ".save-slot-load");
    load.dataset.slotId = slot.id;
    load.disabled = !check.compatible;
    load.setAttribute("aria-label", `Load ${slot.name}`);
    const remove = part<HTMLButtonElement>(row, ".save-slot-delete");
    remove.dataset.slotId = slot.id;
    remove.setAttribute("aria-label", `Delete ${slot.name}`);
  });
}

function renderTitleSaveSlots(slots: SaveSlot[], build: CurrentBuild): void {
  const board = required<HTMLElement>("title-save-slots");
  const ordered = slotsNewestFirst(slots);
  const rows = syncChildren(
    board,
    ordered.map((slot) => slot.id),
    () => {
      const row = document.createElement("li");
      row.className = "save-slot title-save-slot";
      row.innerHTML = `<button type="button" class="save-slot-select"><strong></strong><span class="save-slot-when"></span><span class="save-slot-config"></span><span class="save-slot-versions"></span><span class="save-slot-issue"></span></button><button type="button" class="save-slot-load">Load</button><button type="button" class="save-slot-delete">Delete</button>`;
      return row;
    },
  );
  ordered.forEach((slot, index) => {
    const row = rows[index];
    if (!row) return;
    const envelope = parseHxf1(slot.payload);
    const check = envelope
      ? compatibility(envelope, build)
      : {
          compatible: false,
          mismatches: [
            {
              field: "save",
              expected: "a readable HXF1 file",
              found: "unreadable",
            },
          ],
        };
    row.classList.toggle("selected", slot.id === selectedSaveId);
    row.classList.toggle("incompatible", !check.compatible);
    part(row, "strong").textContent = slot.name;
    part(row, ".save-slot-when").textContent = formatSavedAt(slot.savedAt);
    part(row, ".save-slot-config").textContent = formatConfig(slot.config);
    part(row, ".save-slot-versions").textContent = formatVersions(
      slot.versions,
    );
    part(row, ".save-slot-issue").textContent = check.compatible
      ? ""
      : describeMismatches(check.mismatches);
    const select = part<HTMLButtonElement>(row, ".save-slot-select");
    select.dataset.slotId = slot.id;
    select.setAttribute("aria-pressed", String(slot.id === selectedSaveId));
    select.setAttribute("aria-label", `Select save ${slot.name}`);
    const load = part<HTMLButtonElement>(row, ".save-slot-load");
    load.dataset.slotId = slot.id;
    load.disabled = !check.compatible;
    load.setAttribute("aria-label", `Load ${slot.name}`);
    const remove = part<HTMLButtonElement>(row, ".save-slot-delete");
    remove.dataset.slotId = slot.id;
    remove.setAttribute("aria-label", `Delete ${slot.name}`);
  });
}

async function loadSlot(slot: SaveSlot): Promise<void> {
  try {
    input.clear();
    const next = await host.load(slot.payload);
    // A load is a discontinuity the clock cannot see across: whatever it counted belongs to a
    // different sitting. The run stays, so checkpoints keep landing, but it is marked uncomparable
    // rather than quietly presented as a clean time.
    if (!run) beginRun(next);
    if (run) {
      run = taintRun(run, "loaded-save");
      writeRun(localStorage, run);
      renderRun();
    }
    update(next);
    syncSessionInputs(next);
    renderer.recenter();
    selectedSaveId = slot.id;
    saveNameInput.value = slot.name;
    showFeedback(`Restored “${slot.name}”`);
    closePanels();
    closeTitleScreen();
    updateContinueState(`Restored “${slot.name}”.`);
  } catch (error) {
    updateContinueState(`Load rejected: ${String(error)}`);
  }
}

required<HTMLElement>("save-slots").addEventListener("click", (event) => {
  const target = event.target as HTMLElement;
  const load = target.closest<HTMLButtonElement>(".save-slot-load");
  const remove = target.closest<HTMLButtonElement>(".save-slot-delete");
  const select = target.closest<HTMLButtonElement>(".save-slot-select");
  const id = (load ?? remove ?? select)?.dataset.slotId;
  if (!id) return;
  const { slots, error } = readCatalog(localStorage);
  if (error) {
    updateContinueState(error);
    return;
  }
  const slot = slots.find((entry) => entry.id === id);
  if (!slot) return;
  if (load) {
    void loadSlot(slot);
    return;
  }
  if (remove) {
    if (!window.confirm(`Delete “${slot.name}”? This cannot be undone.`))
      return;
    if (slot.sourceKey) localStorage.removeItem(slot.sourceKey);
    writeCatalog(localStorage, removeSlot(slots, slot.id));
    if (selectedSaveId === slot.id) {
      selectedSaveId = null;
      if (saveNameInput.value === slot.name) saveNameInput.value = "";
    }
    updateContinueState(`Deleted “${slot.name}”.`);
    return;
  }
  selectedSaveId = slot.id;
  saveNameInput.value = slot.name;
  updateContinueState();
});

required<HTMLElement>("title-save-slots").addEventListener("click", (event) => {
  const target = event.target as HTMLElement;
  const load = target.closest<HTMLButtonElement>(".save-slot-load");
  const remove = target.closest<HTMLButtonElement>(".save-slot-delete");
  const select = target.closest<HTMLButtonElement>(".save-slot-select");
  const id = (load ?? remove ?? select)?.dataset.slotId;
  if (!id) return;
  const { slots, error } = readCatalog(localStorage);
  if (error) {
    updateContinueState(error);
    return;
  }
  const slot = slots.find((entry) => entry.id === id);
  if (!slot) return;
  if (load) {
    void loadSlot(slot);
    return;
  }
  if (remove) {
    if (!window.confirm(`Delete “${slot.name}”? This cannot be undone.`))
      return;
    if (slot.sourceKey) localStorage.removeItem(slot.sourceKey);
    writeCatalog(localStorage, removeSlot(slots, slot.id));
    if (selectedSaveId === slot.id) {
      selectedSaveId = null;
      if (saveNameInput.value === slot.name) saveNameInput.value = "";
    }
    updateContinueState(`Deleted “${slot.name}”.`);
    return;
  }
  selectedSaveId = slot.id;
  saveNameInput.value = slot.name;
  updateContinueState();
});

/*
 * Whether a key belongs to the focused control instead of to the world.
 *
 * Only a field that consumes what you type does. A button keeps focus after it is clicked, and
 * counting that as typing left every binding dead until the canvas was clicked again: pressing a
 * panel toggle meant you could no longer walk, and Space no longer recentred. The world owns the
 * keys unless the player is actually filling something in.
 */
function isTypingTarget(target: EventTarget | null): boolean {
  if (
    target instanceof HTMLInputElement ||
    target instanceof HTMLSelectElement ||
    target instanceof HTMLTextAreaElement
  )
    return true;
  return target instanceof HTMLElement && target.isContentEditable;
}

/*
 * The one exception, and the reason it is narrow. A control the keyboard itself reached keeps its
 * own Space, so the panels stay operable without a mouse; `:focus-visible` is what tells that
 * apart from a button the mouse merely left focused, which is the case the world takes back.
 */
function isKeyboardFocusedControl(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (!isPointerActivatedControl(target)) return false;
  try {
    return target.matches(":focus-visible");
  } catch {
    return false;
  }
}

function isPointerActivatedControl(target: EventTarget | null): boolean {
  if (
    target instanceof HTMLButtonElement ||
    target instanceof HTMLAnchorElement
  )
    return true;
  return (
    target instanceof HTMLInputElement &&
    (target.type === "checkbox" || target.type === "radio")
  );
}

window.addEventListener("pointerup", (event) => {
  // A clicked button keeps focus, and Space then activates it instead of recentring. Give the
  // keys back to the world once the pointer is done; a tabbed control still has :focus-visible.
  const target = event.target;
  if (!isPointerActivatedControl(target) || isTypingTarget(target)) return;
  if (target instanceof HTMLElement) target.blur();
});

function titleCase(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

function reportWorkerError(error: unknown): void {
  setPlaying(false);
  showFeedback(`Simulation worker error: ${String(error)}`);
}

/**
 * Clear the screen. This is the reset it always was — `Escape`, a new game, and a load all call it,
 * and all three should still leave nothing open. What changed in v0.20.1 is that opening a panel
 * stopped calling it.
 */
function closePanels(except?: HTMLElement): void {
  for (const panel of document.querySelectorAll<HTMLElement>(
    ".glass-panel.open",
  )) {
    if (panel === except) continue;
    panel.classList.remove("open");
  }
  syncPanelToggles();
  savePanelState();
}

for (const toggle of document.querySelectorAll<HTMLButtonElement>(
  ".panel-toggle",
)) {
  toggle.addEventListener("click", () =>
    togglePanel(toggle.dataset.panelTarget ?? ""),
  );
}

/*
 * A dropdown holds the keys while it is being used, because arrow keys and letters are how an
 * option is chosen. It hands them straight back once a choice is made, so picking a speed or a
 * recipe never leaves the player unable to walk.
 */
document.addEventListener("change", (event) => {
  if (event.target instanceof HTMLSelectElement) event.target.blur();
});

// A close button closes the panel it is in and nothing else. Clearing the screen is Escape's job.
for (const close of document.querySelectorAll<HTMLButtonElement>(
  ".panel-close",
)) {
  close.addEventListener("click", () => {
    close.closest<HTMLElement>(".glass-panel")?.classList.remove("open");
    syncPanelToggles();
    savePanelState();
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
    enqueue(currentMovementIntent());
  };
  const stop = (event: PointerEvent): void => {
    event.preventDefault();
    if (!pressedMovement.delete(code)) return;
    enqueue(currentMovementIntent());
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
loadPanelState();
// Crossing down into the narrow layout leaves whichever panel is topmost in the rail, because
// below that width there is only room for one.
ONE_PANEL_AT_A_TIME.addEventListener("change", (event) => {
  if (!event.matches) return;
  const [first] = openPanelIds();
  const keep = first ? document.getElementById(first) : null;
  closePanels(keep ?? undefined);
});
setMuted(audio.isMuted);
setReducedMotion(loadReducedMotion());
setGraphicsProfile(initialGraphics);
// A reload is a discontinuity for the same reason a load is: the tab was gone for an unknown
// stretch. The records survive so the ladder is not lost, and the run says why it cannot be raced.
run = readRun(localStorage);
if (run && run.records.length > 0) run = taintRun(run, "loaded-save");
renderRun();
update(snapshot);
syncSessionInputs(snapshot);
updateContinueState();
const initialCompatible = latestCompatible(
  readCatalog(localStorage).slots,
  currentBuild(),
);
if (initialCompatible) {
  switchTitleTab("saves");
} else {
  switchTitleTab("new");
}
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
      run: () => {
        timings: RunTimings | null;
        elapsedMs: number;
        report: string;
      };
      renderer: () => RendererDiagnostics;
      orbit: (step: -1 | 1) => void;
      profile: (profile?: GraphicsProfile) => GraphicsProfile;
      pick: (
        x: number,
        y: number,
      ) => {
        axial: { q: number; r: number };
        world: WorldPoint;
      };
    };
  }
}

window.__hexFactory = {
  snapshot: () => host.snapshot(),
  renderer: () => renderer.getDiagnostics(),
  orbit: (step) => orbitView(step),
  profile: (profile) => {
    if (profile) setGraphicsProfile(profile);
    return renderer.getGraphicsProfile();
  },
  pick: (x, y) => ({
    axial: renderer.pick(x, y),
    world: renderer.pickWorld(x, y),
  }),
  // The clock, readable. A scripted run needs the elapsed figure while it is still running, not
  // only the records that have already landed.
  run: () => ({
    timings: run,
    elapsedMs: runElapsedMs,
    report: run ? formatRunReport(run) : "",
  }),
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
    // The scripted path starts a run too, so a timed opening can be driven from the console
    // without a human hand on the keyboard.
    beginRun(next);
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
