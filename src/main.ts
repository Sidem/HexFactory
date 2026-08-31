import { BoundaryTool } from "./ui/boundaries";
import { GroundTool } from "./ui/ground";
import {
  axialToPixel,
  pixelToAxial,
  rotateHexDirection,
  type HexDirection,
} from "@hexlife/embed/hex";

import {
  buildingAvailability,
  costAt,
  costLines,
  heldQuantity,
  type CostLine,
} from "./core/availability";
import { cueForEvent, FeedbackAudio } from "./audio/feedback";
import { halfTransfer } from "./core/commands";
import { supportsRecipe } from "./core/definitions";
import { recipeOutputs } from "./core/recipes";
import { productionNote } from "./ui/production";
import { FactoryHost } from "./core/FactoryHost";
import { FrameClock, SIMULATION_TICKS_PER_SECOND } from "./core/frameClock";
import { nextAction } from "./core/guidance";
import { SkillsView } from "./ui/skills";
import { ResearchTree } from "./ui/researchTree";
import { BoundedInputQueue, MOVEMENT_KEYS, movementIntent } from "./core/input";
import {
  AUTOSAVE_SLOT_NAME,
  CATALOG_DOWNLOAD_NAME,
  catalogDocument,
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
  saveFileName,
  slotFromPayload,
  slotsFromFileText,
  slotsNewestFirst,
  uniqueSlotName,
  unsavedRunAtRisk,
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
import {
  bandAt,
  TERRAIN_INFO,
  TERRAIN_ORDER,
  terrainAccess,
} from "./core/terrain";
import {
  CORNER_START,
  DIRECTION_NAMES,
  rotateAnyOrientation,
  TRANSPORT_DIRECTIONS,
} from "./core/directions";
import type {
  BuildingDefinition,
  BuildingKind,
  EntitySnapshot,
  FactorySnapshot,
  ItemDefinition,
  NativeInputCommand,
  PlacementPreview,
  ProjectState,
  RecipeDefinition,
  StockKind,
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
import {
  buildingEmblemSvg,
  clearEmblem,
  emblemRank,
  hasBuildingEmblem,
  paintEmblem,
  recipeCategoryAccent,
  recipeCategoryEmblemSvg,
} from "./rendering/emblems";
import { itemIconSvg } from "./rendering/icons";
import {
  createItemChip,
  fillItemChip,
  itemTooltip,
  type ItemChipView,
} from "./rendering/itemChip";
import {
  buildingBeside,
  findLandingHub,
  homeBearing,
  WORLD_SCALE,
} from "./rendering/landmarks";
import { MinimapRenderer } from "./rendering/MinimapRenderer";
import { ThreeFactoryRenderer } from "./rendering/three/ThreeFactoryRenderer";
import {
  defaultGraphicsProfile,
  GRAPHICS_STORAGE_KEY,
  parseGraphicsProfile,
} from "./rendering/three/quality";
import { part, required, syncChildren } from "./ui/dom";
import { PanelController } from "./ui/panels";
import { ConfirmDialog } from "./ui/confirm";
import { machineStockSlots } from "./ui/stockSlots";
import { WorldParameterForm } from "./ui/worldParameters";
import {
  applyChanges,
  PREVIEW_HEIGHT,
  PREVIEW_WIDTH,
  WorldPreviewPanel,
  type PreviewItemLook,
  type RepairChoice,
} from "./ui/worldPreview";
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
  KeyK: "skills-panel",
  KeyP: "quest-panel",
  KeyB: "build-panel",
  KeyL: "recipe-panel",
  KeyC: "creative-panel",
};
/**
 * A refusal the world itself already shows. The ring around the player is the swing filling, so
 * repeating it as a message strip toast on every frame of a held harvest is noise.
 */
const SILENT_EVENTS = new Set(["action cooling down"]);
const canvas = required<HTMLCanvasElement>("factory-canvas");
const soundButton = required<HTMLButtonElement>("sound");
const muteInput = required<HTMLInputElement>("mute");
const reduceMotionInput = required<HTMLInputElement>("reduce-motion");
const graphicsProfileInput = required<HTMLSelectElement>("graphics-profile");
/** Comfort settings are preferences about a room, so they live beside the hotbar, not in a save. */
const MOTION_KEY = "hexfactory:reduced-motion:v1";
required<HTMLElement>("simulation-rate").textContent =
  `Simulation: ${SIMULATION_TICKS_PER_SECOND} ticks per second`;
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
const titleScenarioChoices = required<HTMLDivElement>("title-scenario-choices");
const titleSaveNameInput = required<HTMLInputElement>("title-save-name");
const titleCreativeNote = required<HTMLParagraphElement>("title-creative-note");
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
const saveFileInput = required<HTMLInputElement>("save-file-input");
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
const boundaryTool = new BoundaryTool(
  required<HTMLElement>("boundary-panel"),
  host,
  renderer,
  enqueue,
  () => {
    selectTool("inspect");
    closePanels();
    groundTool.close(false);
  },
);
const groundTool = new GroundTool(
  required<HTMLElement>("ground-panel"),
  host,
  renderer,
  enqueue,
  () => {
    selectTool("inspect");
    closePanels();
    boundaryTool.close(false);
  },
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
const researchDialog = required<HTMLDialogElement>("research-panel");
const skillsDialog = required<HTMLDialogElement>("skills-panel");
const skillsView = new SkillsView(skillsDialog, host.technologies, (id) =>
  enqueue({ type: "purchase_skill", skill_id: id }),
);
const confirmDialog = new ConfirmDialog(
  required<HTMLDialogElement>("confirm-dialog"),
  () => {
    gatherHeld = false;
    harvestPointer = null;
    runningHeld = false;
    pressedMovement.clear();
    stopAiming();
    endStackDrag();
    enqueue(currentMovementIntent());
  },
);
const panels = new PanelController(document, localStorage, (id, open) => {
  if ((id !== "research-panel" && id !== "skills-panel") || !open) return;
  gatherHeld = false;
  harvestPointer = null;
  runningHeld = false;
  stopAiming();
  pressedMovement.clear();
  enqueue(currentMovementIntent());
  if (id === "research-panel") {
    researchTree.onOpen();
    const currentRun = snapshot;
    void host
      .worldParams()
      .then((params) => {
        // A load/reset may finish while the worker is replying. Never show another run's notice.
        if (
          currentRun.scenario !== snapshot.scenario ||
          currentRun.seed !== snapshot.seed ||
          snapshot.tick < currentRun.tick
        )
          return;
        const oil = host.definitions.items.find(
          (item) => item.key === "crude-oil",
        );
        const note = required("research-world-note");
        const hasOil =
          !oil ||
          params.site_rules.some(
            (rule) => rule.item_id === oil.id && rule.weight > 0,
          );
        note.hidden = hasOil;
        note.textContent = hasOil
          ? ""
          : "This world keeps its original deposits and has no generated oil sites. Petroleum is optional: keep your existing factory, or start a new world to explore oil and asphalt.";
      })
      .catch(() => {
        /* The worker already reports unavailable world parameters. */
      });
  } else skillsView.update(snapshot);
});
const researchTree = new ResearchTree(
  researchDialog,
  host.technologies,
  host.definitions,
  (id) => enqueue({ type: "research", technology_id: id }),
);

let snapshot = host.snapshot();
/** Which named slot Save will overwrite, if any. Presentation only — the catalog is the store. */
let selectedSaveId: string | null = null;
/**
 * What this run's save is called. The title screen asks once, and everything that writes — the
 * auto-save and the Save button alike — targets that one name, so a run keeps a single catalogue
 * entry rather than scattering across "Auto-save" plus whatever the player typed later.
 */
let runName = AUTOSAVE_SLOT_NAME;
let tool: Tool = "inspect";
let orientation = 0;
let selected: { q: number; r: number } | null = null;
/**
 * The hex the player stands on, and the building they stand beside, as cell keys. Walking up to a
 * machine selects it, and these two are what keep that from being a per-frame decision: the scan
 * runs on the step that crosses a hex boundary, and the selection follows only when the building in
 * reach actually changes.
 */
let standingHex: string | null = null;
let besideBuilding: string | null = null;
let hover: { q: number; r: number } | null = null;
let hoverPreview: PlacementPreview | null = null;
const frameClock = new FrameClock(performance.now());
let feedbackTimer = 0;
let lastEvent = "";
let autoSavePending = false;
let lastAutoSaveTime = performance.now();
const AUTOSAVE_INTERVAL_MS = 60_000;
/**
 * How much of this run is not on disk, and how long it has been that way.
 *
 * `savedTick` is the tick the newest successful write covered; a world that has just been generated
 * or loaded counts as covered, because nothing has happened in it that the catalogue does not have.
 * The close guard reads both numbers — see `unsavedRunAtRisk`.
 */
let savedTick = 0;
let savedAt = Date.now();
/** Half the auto-save interval. Less than this at risk is not worth stopping a leaving player. */
const UNSAVED_CLOSE_GRACE_MS = 30_000;
/**
 * The run clock, and the time it has counted.
 *
 * `runElapsedMs` accrues whenever the factory does. Since the factory cannot be paused, excluding a
 * menu interval would make the report claim less time than the corresponding native ticks consumed.
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

/** Whether a pointer gesture currently owns the screen, whichever of them it is. */
function dragOwnsPointer(): boolean {
  return (
    stackDrag !== null ||
    panPointer !== null ||
    harvestPointer !== null ||
    dragBuild !== null
  );
}

/*
 * A drag starts on a slot or on the map and then travels across labels, headings and readouts, and
 * the browser's default reading of that journey is a text selection: the player finishes a belt run
 * or a stack move looking at a blue smear of their own interface, which they then have to click
 * somewhere empty to be rid of.
 *
 * `selectstart` is the one moment the browser asks before it begins highlighting, and it fires
 * after the `pointerdown` that armed the gesture — so by the time this runs, the drag has already
 * said what it is, and the question can be answered by asking rather than by remembering. That is
 * the whole reason it is done here instead of switching `user-select` off at each drag's start and
 * back on at its end: an end that got missed would leave the interface unselectable for the rest of
 * the session, which is a worse fault than the one being fixed. Anything left highlighted from
 * before goes at the same time, so beginning a drag clears the smear rather than adding to it.
 */
document.addEventListener("selectstart", (event) => {
  if (!dragOwnsPointer()) return;
  event.preventDefault();
  getSelection()?.removeAllRanges();
});

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
      "Drag from source to destination. Belts carry solids and sealed barrels; pipes carry loose water and crude. Paired underpasses cross an occupied lane without mixing it.",
    holds: ({ kind }) => BUILD_GROUP_BY_KIND[kind] === "transport",
  },
  {
    key: "processing",
    title: "Processing",
    blurb:
      "Turn one material into another. Each station lists the recipes it supports.",
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
 * What the bar starts with: the early game in the order it is met, then power, then the junction
 * that turns one line into a network. Anything else is a pin away.
 */
const DEFAULT_HOTBAR: (Tool | null)[] = [28, 27, 2, 4, 1, 3, 12, 13, 8];
/** Which slot each definition sits in, or null for an empty slot. Presentation only — never saved
 * with the game, never hashed: it is a preference about a keyboard, not a fact about a factory. */
let hotbar: (Tool | null)[] = loadHotbar();
/** Panel scope, not game state: which side of progressive disclosure each catalogue is showing. */
let showAllBuildings = false;
/** The live catalogue search. Panel state, deliberately not persisted: a filter that survives a
    reload is a catalogue that looks broken until the player notices the box. */
let buildSearch = "";
/** The live recipe-lookup search, on the same terms as {@link buildSearch}. */
let recipeSearch = "";
/** The slot a drag is currently over, so the drop target can be shown before the pointer lands. */
let hotbarDragOver: number | null = null;

function loadHotbar(): (Tool | null)[] {
  // Through the same sieve the stored bar goes through. A milestone that retires a definition id
  // leaves the default naming it too, and a default is not more trustworthy than a preference —
  // v0.25.1 retired the riser and the ninth slot rendered as `?18` until this asked the catalogue.
  const defaults = Array.from({ length: HOTBAR_SLOTS }, (_, slot) =>
    sanitiseSlot(DEFAULT_HOTBAR[slot] ?? null),
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
  run = startRun(Date.now(), next.tick);
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
    part<HTMLElement>(row, ".run-label").textContent =
      checkpoint.label + (checkpoint.optional ? " (optional)" : "");
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
  boundaryTool.update(snapshot);
  groundTool.update(snapshot);
  syncHoverWithCamera();
  minimap.setSnapshot(snapshot, landingHub);
  renderHomeReadout();
  required<HTMLElement>("scenario-value").textContent = snapshot.scenario_name;
  required<HTMLElement>("tick-value").textContent =
    snapshot.tick.toLocaleString();
  skillsView.update(snapshot);
  required<HTMLElement>("skill-points-value").textContent = String(
    snapshot.skills.points,
  );
  required<HTMLElement>("skills-chip").setAttribute(
    "aria-label",
    `Skills: ${snapshot.skills.points} Skill Point${snapshot.skills.points === 1 ? "" : "s"} (K)`,
  );
  required<HTMLElement>("skills-chip").classList.toggle(
    "has-points",
    snapshot.skills.points > 0,
  );
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
    renderRecipePanel();
    renderTechnologies();
    renderContract();
    renderRequests();
    renderNextAction();
    // Both kinds of change can complete a checkpoint: the first iron is a pack change and the
    // first powered composer is a factory one, so the clock reads whenever either moved.
    evaluateRun(next);
  }
  // Before the inspector renders, so the machine the player just walked up to is on the panel in
  // the same pass rather than a tick later.
  syncStandingSelection(
    previous === next || previous.buildings !== next.buildings,
  );
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
 * Walking up to a machine opens it. The player is beside at most one building at a time as far as
 * this is concerned, and the selection follows the step that brings a new one into reach rather than
 * every frame, so a hex chosen by hand while standing there keeps the panel until the player moves
 * away and back.
 *
 * Two guards keep the work off the frame: the scan runs only when the player crosses into a new hex
 * or the building set changes, and the selection is only replaced when the building in reach is a
 * different one. Walking clear of everything leaves the last selection standing — there is nothing
 * to put on the panel in its place, and blanking it would be a second thing walking does.
 */
function syncStandingSelection(buildingsChanged: boolean): void {
  const standing = pixelToAxial(snapshot.player, WORLD_SCALE);
  const hex = `${standing.q},${standing.r}`;
  if (hex === standingHex && !buildingsChanged) return;
  standingHex = hex;
  const cell = buildingBeside(snapshot);
  const key = cell ? `${cell.q},${cell.r}` : null;
  if (key === besideBuilding) return;
  besideBuilding = key;
  if (!cell) return;
  selected = cell;
  renderer.setSelection(cell);
  panels.revealInspector();
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
    cell.dataset.stackSource = "player";
    cell.dataset.itemId = stack ? String(stack.item_id) : "";
    cell.dataset.quantity = stack ? String(stack.quantity) : "0";
    cell.tabIndex = 0;
    cell.title =
      item && stack
        ? itemTooltip(item, item.name, { count: stack.quantity })
        : "Empty carrying slot";
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

  // The top bar is the glanceable pack: the player can see what changed without opening a panel.
  // It is deliberately only a preview of native's published carry stacks; the full grid remains I.
  const peek = required<HTMLElement>("inventory-peek");
  const visible = stacks.slice(0, 3);
  const preview = syncChildren(
    peek,
    visible.map((stack, index) => `${index}-${stack.item_id}`),
    () => {
      const holder = document.createElement("span");
      holder.className = "inventory-peek-slot chip-host";
      return holder;
    },
  );
  visible.forEach((stack, index) => {
    const holder = preview[index];
    if (!holder) return;
    paintChip(holder, stack.item_id, {
      count: stack.quantity,
      named: false,
      short: true,
    });
    const item = itemById(stack.item_id);
    if (item) {
      holder.title = itemTooltip(item, item.name, { count: stack.quantity });
    }
  });
  peek.dataset.overflow =
    stacks.length > visible.length ? `+${stacks.length - visible.length}` : "";
  peek.classList.toggle("empty", stacks.length === 0);

  // A lifted drag owns the floating stack, because it is carrying something native has not been
  // told about yet: the pickup is only sent when the drop lands.
  if (stackDrag?.lifted) return;
  const cursor = required<HTMLElement>("cursor-stack");
  const hand = snapshot.player.hand ?? undefined;
  cursor.hidden = !hand;
  if (hand) {
    paintChip(cursor, hand.item_id, {
      count: hand.quantity,
      named: false,
      short: true,
    });
    const item = itemById(hand.item_id);
    if (item) {
      cursor.title = itemTooltip(item, item.name, { count: hand.quantity });
    }
  }
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
  // Creative is a different game, so a creative run is not a comparable one. The mark is applied
  // here rather than at each switch because all three ways in — the title screen, the panel toggle
  // and the C key — arrive as the same snapshot, and the mark survives switching creative back off.
  if (creative && run) {
    const marked = taintRun(run, "creative");
    if (marked !== run) {
      run = marked;
      writeRun(localStorage, run);
      renderRun();
    }
  }
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
      button.removeAttribute("aria-disabled");
      button.classList.remove("active", "unaffordable", "locked");
      clearEmblem(part(button, "span")).textContent = "";
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
        heldOrientationFor(definition),
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
      paintBuildingEmblem(part(button, "span"), definition);
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

/**
 * Whether a search names this definition.
 *
 * Description and group are searched as well as the name, because the player who needs search most
 * is the one who remembers what a machine *does* — "crush", "power", "under" — rather than what it
 * is called.
 */
function buildMatches(
  definition: BuildingDefinition,
  group: (typeof BUILD_GROUPS)[number],
  query: string,
): boolean {
  return `${definition.name} ${definition.description} ${group.title} ${group.blurb}`
    .toLowerCase()
    .includes(query);
}

/**
 * A search looks past progressive disclosure: typing a name is an explicit request for that
 * machine, and answering it with silence because the machine is still locked would be answering a
 * question nobody asked. The reach toggle would then be a control with no effect, so it steps out
 * of the way until the box is empty again.
 */
function renderBuildScope(hidden: number, query: string): void {
  const scope = required<HTMLButtonElement>("build-scope");
  scope.hidden = query.length > 0;
  scope.textContent = showAllBuildings
    ? "Show what is in reach"
    : hidden > 0
      ? `Show everything (${hidden} locked)`
      : "Show everything";
  scope.setAttribute("aria-pressed", String(showAllBuildings));
}

function renderBuildPanel(): void {
  const root = required<HTMLDivElement>("build-groups");
  const buildable = host.definitions.buildings.filter(
    (definition) => definition.buildable,
  );
  const reach = technologyReach();
  const query = buildSearch.trim().toLowerCase();
  renderBuildScope(
    buildable.filter((definition) => !catalogueVisible(definition, reach))
      .length,
    query,
  );
  if (!root.childElementCount)
    for (const group of BUILD_GROUPS) {
      const section = document.createElement("section");
      section.className = "build-group";
      section.dataset.group = group.key;
      section.innerHTML = `<h3>${group.title}</h3><p>${group.blurb}</p><div class="build-cards"></div>`;
      root.append(section);
    }
  let shown = 0;
  for (const group of BUILD_GROUPS) {
    const section = root.querySelector<HTMLElement>(
      `[data-group="${group.key}"]`,
    );
    if (!section) continue;
    const definitions = buildable.filter((definition) =>
      group.holds(definition)
        ? query
          ? buildMatches(definition, group, query)
          : catalogueVisible(definition, reach)
        : false,
    );
    shown += definitions.length;
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
  // An empty catalogue that says nothing reads as a broken panel. Say which search emptied it.
  const empty = required<HTMLParagraphElement>("build-empty");
  empty.hidden = shown > 0;
  empty.textContent = query
    ? `Nothing in the catalogue matches “${buildSearch.trim()}”.`
    : "Nothing to build yet.";
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

/**
 * The heading a price should be quoted at for this definition.
 *
 * Only the tool actually in hand has a heading: a belt costs one ore or two depending on which way
 * the player is pointing it, and the catalogue would be lying if every other card quoted the price
 * of a heading nobody is holding. So the active tool is priced at the live orientation and every
 * other row at the edge price, which is the one it is bought at unless the player turns it.
 */
function heldOrientationFor(definition: BuildingDefinition): number {
  return definition.id === tool ? orientation : 0;
}

/**
 * Draw a building's emblem into a fixed box.
 *
 * One function, so the catalogue card and the hotbar slot cannot drift apart: a machine has to look
 * the same in both places, or the pin a player just made is unrecognisable on the bar a second
 * later. Tier rides as a rank badge rather than as a second drawing, so Extractor II is visibly the
 * extractor. A definition the emblem library has never seen falls back to the generic plate
 * carrying that definition's own short code — adding a building to `definitions.json` yields a
 * plain button, never an empty one.
 */
function paintBuildingEmblem(
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
}

function fillBuildCard(
  card: HTMLElement,
  definition: BuildingDefinition,
): void {
  const availability = buildingAvailability(
    definition,
    snapshot,
    host.definitions.items,
    heldOrientationFor(definition),
  );
  card.classList.toggle("locked", availability.locked);
  card.classList.toggle("unaffordable", !availability.affordable);
  card.classList.toggle("active", definition.id === tool);
  card.classList.toggle("pinned", hotbar.includes(definition.id));
  paintBuildingEmblem(part<HTMLElement>(card, ".build-stamp"), definition);
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
  if (definition.orientation_axis === "any") labels.push("Twelve headings");
  if (definition.splits) labels.push("Fans out");
  if (definition.merges) labels.push("Takes in turn");
  if (definition.underpass_span !== undefined)
    labels.push(`Drag pair · spans ${definition.underpass_span}`);
  if (definition.transport_medium === "fluid") labels.push("Fluids only");
  if (definition.accepted_item_ids?.length === 1) {
    const item = itemById(definition.accepted_item_ids[0]);
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

  renderIngredientRow(
    part<HTMLElement>(card, ".build-cost"),
    // The bill at the heading being quoted, so the row and the availability beside it can never
    // describe two different prices.
    costAt(definition, heldOrientationFor(definition)),
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
        '<i class="recipe-emblem"></i><span class="ingredient-list recipe-in"></span><i class="recipe-arrow" aria-hidden="true">→</i><span class="ingredient-list recipe-out"></span><small class="recipe-meta"></small>';
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
    paintEmblem(part<HTMLElement>(row, ".recipe-emblem"), {
      key: recipe.category,
      accent: recipeCategoryAccent(recipe.category),
      markup: recipeCategoryEmblemSvg(recipe.category),
    });
    // Inputs are a quantity the player may be expected to supply — early machines are hand-fed
    // through Put long before a belt reaches them — so they are priced against the pack. The
    // output is a result and is only ever an amount.
    fillIngredients(
      part<HTMLElement>(row, ".recipe-in"),
      recipe.inputs,
      costLines(recipe.inputs, snapshot),
    );
    fillIngredients(
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
  return `${inputs} makes ${recipeOutputs(recipe)
    .map(({ item_id, quantity }) => `${quantity} ${name(item_id)}`)
    .join(" and ")}`;
}

/**
 * The recipe lookup.
 *
 * The catalogue answers "what can this machine do", which is the question you ask once you have
 * already decided which machine to look at. This answers the one a stuck player actually has —
 * "where does a gear come from", "what is copper wire for" — by searching every recipe from both
 * sides of the arrow at once and saying which machine runs each. It is a reading of the shipped
 * definitions and nothing else: no command is sent from here, and the click at the end of it is the
 * same tool selection the catalogue already makes.
 */
/** Every buildable machine that may run this recipe, in catalog order. */
function machinesForRecipe(recipe: RecipeDefinition): BuildingDefinition[] {
  return host.definitions.buildings.filter(
    (definition) => definition.buildable && supportsRecipe(definition, recipe),
  );
}

/**
 * The items a search names. Matching the item first is what lets one query answer both halves of
 * the question: typing "gear" finds the recipe that makes a gear and every recipe that spends one,
 * neither of which has the word in its own name.
 */
function itemsMatching(query: string): Set<number> {
  const called = host.definitions.items.filter((item) =>
    `${item.name} ${item.key}`.toLowerCase().includes(query),
  );
  // A description is prose, and prose names other materials: the component's blurb mentions the
  // gear it is built from, which would file "Compose component" under Made by for a search for
  // gears. So the blurb is read only when nothing is actually called that, where its reach is the
  // difference between an oblique answer and an empty panel.
  const matched =
    called.length > 0
      ? called
      : host.definitions.items.filter((item) =>
          item.description.toLowerCase().includes(query),
        );
  return new Set(matched.map(({ id }) => id));
}

/** Whether a search names the recipe itself — its name, its process, or a machine that runs it. */
function recipeMatches(recipe: RecipeDefinition, query: string): boolean {
  return `${recipe.name} ${recipe.description} ${recipe.category} ${machinesForRecipe(
    recipe,
  )
    .map(({ name }) => name)
    .join(" ")}`
    .toLowerCase()
    .includes(query);
}

function renderRecipePanel(): void {
  const query = recipeSearch.trim().toLowerCase();
  const recipes = host.definitions.recipes;
  const named = query ? itemsMatching(query) : new Set<number>();
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
        (recipe) => !claimed.has(recipe.id) && recipeMatches(recipe, query),
      )
    : recipes;
  renderRecipeGroup("recipe-makes", makes);
  renderRecipeGroup("recipe-uses", uses);
  renderRecipeGroup("recipe-rest", rest);
  required<HTMLElement>("recipe-rest-title").textContent = query
    ? "Other matches"
    : "Every recipe";
  const empty = required<HTMLParagraphElement>("recipe-empty");
  empty.hidden = makes.length + uses.length + rest.length > 0;
  empty.textContent = `Nothing makes or uses “${recipeSearch.trim()}”.`;
}

function renderRecipeGroup(id: string, recipes: RecipeDefinition[]): void {
  const section = required<HTMLElement>(id);
  section.hidden = recipes.length === 0;
  const rows = syncChildren(
    part<HTMLElement>(section, ".recipe-list"),
    recipes.map(({ id: recipe }) => String(recipe)),
    createLookupRow,
  );
  recipes.forEach((recipe, index) => {
    const row = rows[index];
    if (row) fillLookupRow(row, recipe);
  });
}

function createLookupRow(key: string): HTMLElement {
  const row = document.createElement("button");
  row.type = "button";
  row.className = "recipe-row lookup-row";
  row.dataset.recipeId = key;
  row.innerHTML =
    '<i class="recipe-emblem"></i><strong class="lookup-name"></strong><small class="recipe-meta"></small><span class="lookup-flow"><span class="ingredient-list lookup-in"></span><i class="recipe-arrow" aria-hidden="true">→</i><span class="ingredient-list lookup-out"></span></span><small class="lookup-machines"></small>';
  return row;
}

function fillLookupRow(row: HTMLElement, recipe: RecipeDefinition): void {
  paintEmblem(part<HTMLElement>(row, ".recipe-emblem"), {
    key: recipe.category,
    accent: recipeCategoryAccent(recipe.category),
    markup: recipeCategoryEmblemSvg(recipe.category),
  });
  part(row, ".lookup-name").textContent = recipe.name;
  // Plain amounts on both sides: this is a reference, not a bill the player is being asked to pay.
  fillIngredients(part<HTMLElement>(row, ".lookup-in"), recipe.inputs);
  fillIngredients(part<HTMLElement>(row, ".lookup-out"), recipeOutputs(recipe));
  part<HTMLElement>(row, ".recipe-arrow").hidden = recipe.inputs.length === 0;
  const meta = [`${recipe.duration} ticks`];
  if (recipe.fuel) meta.push(`${recipe.fuel} fuel`);
  part(row, ".recipe-meta").textContent = meta.join(" · ");
  // The row builds the first machine research has actually reached, so clicking it hands over
  // something placeable rather than a refusal. The list still names every machine that runs it.
  const machines = machinesForRecipe(recipe);
  const reachable = machines.find(
    (definition) =>
      !buildingAvailability(definition, snapshot, host.definitions.items)
        .locked,
  );
  row.dataset.definitionId = String(reachable?.id ?? machines[0]?.id ?? "");
  row.classList.toggle("locked", reachable === undefined);
  part(row, ".lookup-machines").textContent = machines.length
    ? `Runs on ${machines.map(({ name }) => name).join(" · ")}`
    : "No machine runs this yet";
  row.title = recipe.description;
  row.setAttribute(
    "aria-label",
    `${recipe.name}: ${describeRecipe(recipe)}. ${meta.join(", ")}`,
  );
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

function renderTechnologies(): void {
  researchTree.update(snapshot);
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
 * The kinds a hand can reach into, mirroring `stock_is_reachable_by_hand` in the core.
 *
 * A copy of a native rule, and deliberately so: this decides whether a button is drawn, native
 * decides whether the transfer happens, and native is the authority. Getting this list wrong shows
 * a control that earns a refusal — a cosmetic bug. Leaving it out would show one on every belt.
 */
const HAND_REACHABLE = new Set<string>([
  "extractor",
  "pump",
  "container",
  "composer",
  "generator",
  "boiler",
]);

interface StockCompartment {
  stock: Exclude<StockKind, "auto">;
  label: string;
  accepts: boolean;
  expected: number[];
  entries: { item_id: number; quantity: number }[];
}

/**
 * The compartments one building publishes: what it takes in, what it burns, what it holds, what it
 * has made.
 *
 * Derived from the snapshot and the recipe rather than from a list of building kinds, which is the
 * whole point of it being one function. Three features ask this question — the inspector draws the
 * compartments, the pack opens itself beside a building that takes items, and a demolition names
 * what is inside before it removes it — and a kind list would have had to be edited in three places
 * every time a machine was added, with two of them silently wrong until someone noticed.
 */
function stockCompartments(building: EntitySnapshot): StockCompartment[] {
  const definition = host.definitions.buildings.find(
    ({ id }) => id === building.definition_id,
  );
  const recipe = host.definitions.recipes.find(
    ({ id }) => id === building.recipe_id,
  );
  const waterId = host.definitions.items.find(
    (item) => item.key === "water",
  )?.id;
  const compartments: StockCompartment[] = [];
  if (building.kind === "container")
    compartments.push({
      stock: "inventory",
      label: "Storage",
      accepts: true,
      expected: definition?.accepted_item_ids ?? [],
      entries: building.inventory,
    });
  else {
    const hasInputs =
      building.kind === "boiler" ||
      Boolean(recipe?.inputs.length) ||
      Boolean(building.input_inventory?.length);
    const firebox =
      (building.kind === "generator" || building.kind === "boiler") &&
      (definition?.power_source === undefined ||
        definition.power_source === "burner");
    const hasFuel =
      Boolean(recipe?.fuel) ||
      firebox ||
      Boolean(building.fuel_inventory?.length);
    const hasOutput =
      building.kind === "composer" ||
      building.kind === "extractor" ||
      building.kind === "pump" ||
      Boolean(building.output_inventory?.length);
    if (hasInputs)
      compartments.push({
        stock: "input",
        label: "Ingredient",
        accepts: true,
        expected: [
          ...(recipe?.inputs.map((input) => input.item_id) ?? []),
          ...(building.kind === "boiler" && waterId ? [waterId] : []),
        ],
        entries: building.input_inventory ?? [],
      });
    if (hasFuel)
      compartments.push({
        stock: "fuel",
        label: "Fuel",
        accepts: true,
        expected: [],
        entries: building.fuel_inventory ?? [],
      });
    if (hasOutput)
      compartments.push({
        stock: "output",
        label: "Output",
        accepts: false,
        expected: [
          ...(recipe
            ? recipeOutputs(recipe).map((output) => output.item_id)
            : []),
          ...(definition?.output_item_id ? [definition.output_item_id] : []),
        ],
        entries: building.output_inventory ?? [],
      });
  }
  return compartments;
}

function renderInspectorActions(building: EntitySnapshot | undefined): void {
  const container = required<HTMLElement>("inspect-stock");
  const list = required<HTMLDivElement>("inspector-actions");
  if (!building || !HAND_REACHABLE.has(building.kind)) {
    container.hidden = true;
    syncChildren(list, [], () => document.createElement("section"));
    return;
  }
  const definition = host.definitions.buildings.find(
    ({ id }) => id === building.definition_id,
  );
  const compartments = stockCompartments(building);
  container.hidden = compartments.length === 0;
  const cards = syncChildren(
    list,
    compartments.map(({ stock }) => stock),
    () => {
      const card = document.createElement("section");
      card.className = "machine-compartment";
      card.innerHTML = `<div class="machine-compartment-header"><span></span><span class="machine-compartment-count"></span></div><div class="machine-stock-grid" role="list"></div>`;
      return card;
    },
  );
  compartments.forEach(
    ({ stock, label, accepts, expected, entries }, index) => {
      const card = cards[index];
      if (!card) return;
      card.className = `machine-compartment ${stock}`;
      part<HTMLElement>(card, ".machine-compartment-header span").textContent =
        label;
      const total = entries.reduce((sum, entry) => sum + entry.quantity, 0);
      // Native bounds ingredients and fuel per item and the rest as one pool, so the count has to
      // read differently: `12 each` is the promise that a stocked slot never crowds out an empty
      // one, where `n / 12` would still claim the compartment is a single shared budget.
      const perItem = stock === "input" || stock === "fuel";
      const capacity = definition?.capacity;
      part<HTMLElement>(card, ".machine-compartment-count").textContent =
        capacity === undefined
          ? String(total)
          : perItem
            ? `${total} · ${capacity} each`
            : `${total} / ${capacity}`;
      const layout = machineStockSlots(
        entries,
        expected,
        accepts,
        capacity,
        perItem,
      );
      const slots = part<HTMLElement>(card, ".machine-stock-grid");
      const cells = syncChildren(
        slots,
        layout.map((slot) => slot.key),
        () => {
          const cell = document.createElement("div");
          cell.className = "machine-stock-slot chip-host";
          cell.setAttribute("role", "listitem");
          cell.tabIndex = 0;
          return cell;
        },
      );
      cells.forEach((cell, slot) => {
        const entry = layout[slot];
        if (!entry) return;
        const filled = entry.quantity > 0;
        cell.classList.toggle("filled", filled);
        cell.classList.toggle("ghost", Boolean(entry.ghost));
        cell.dataset.stackSource = "building";
        cell.dataset.stock = stock;
        cell.dataset.accepts = entry.accepts ? "1" : "0";
        cell.dataset.q = String(building.q);
        cell.dataset.r = String(building.r);
        cell.dataset.itemId =
          filled && entry.item_id ? String(entry.item_id) : "";
        cell.dataset.quantity = String(entry.quantity);
        paintChip(cell, entry.item_id, {
          count: filled ? entry.quantity : undefined,
          named: false,
          short: true,
        });
        const item = itemById(entry.item_id);
        const slotTooltip =
          item && filled
            ? `${label}: ${itemTooltip(item, item.name, { count: entry.quantity })}${item.fluid ? "\nLoose fluid — use a pipe or barrel station" : ""}`
            : item
              ? `Empty ${label.toLowerCase()} slot for ${item.name}\n${item.description}`
              : `Empty ${label.toLowerCase()} slot`;
        cell.title = slotTooltip;
        cell.setAttribute(
          "aria-label",
          item && filled
            ? `${label}: ${item.name}, ${entry.quantity}`
            : item
              ? `Empty ${label.toLowerCase()} slot for ${item.name}`
              : `Empty ${label.toLowerCase()} slot`,
        );
      });
    },
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
  void building;
  required<HTMLElement>("inspect-load").hidden = true;
}

/** The product last chosen on each entity; presentation state only, never factory truth. */
const selectedOutputProduct = new Map<number, number>();

function renderOutputRouting(building: EntitySnapshot | undefined): void {
  const section = required<HTMLElement>("inspect-output-routing");
  const routes = building?.output_routes ?? [];
  if (!building || routes.length === 0) {
    section.hidden = true;
    return;
  }
  section.hidden = false;
  const remembered = selectedOutputProduct.get(building.id);
  const selectedItem = routes.some(({ item_id }) => item_id === remembered)
    ? remembered!
    : routes[0]!.item_id;
  selectedOutputProduct.set(building.id, selectedItem);
  const selectedRoute = routes.find(({ item_id }) => item_id === selectedItem)!;
  const item = itemById(selectedItem);
  section.style.setProperty("--item-color", item?.color ?? "var(--gold)");

  const products = required<HTMLElement>("inspect-output-products");
  const productButtons = syncChildren(
    products,
    routes.map(({ item_id }) => String(item_id)),
    () => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "inspect-output-product chip-host";
      return button;
    },
  );
  routes.forEach((route, index) => {
    const button = productButtons[index]!;
    const routeItem = itemById(route.item_id);
    button.dataset.itemId = String(route.item_id);
    button.classList.toggle("active", route.item_id === selectedItem);
    button.style.setProperty("--item-color", routeItem?.color ?? "var(--gold)");
    button.setAttribute("aria-pressed", String(route.item_id === selectedItem));
    button.setAttribute(
      "aria-label",
      `Route ${routeItem?.name ?? `item ${route.item_id}`}`,
    );
    paintChip(button, route.item_id, { named: true, short: true });
  });

  required<HTMLElement>("inspect-output-summary").textContent =
    `${item?.name ?? `Item ${selectedItem}`} · ${DIRECTION_NAMES[selectedRoute.direction] ?? "Output"} from q${selectedRoute.q}, r${selectedRoute.r}`;
  const status = required<HTMLElement>("inspect-output-status");
  status.textContent = selectedRoute.target_id ? "Connected" : "Open";
  status.classList.toggle("open", !selectedRoute.target_id);

  const footprint = building.footprint;
  const footprintKeys = new Set(footprint.map(({ q, r }) => `${q},${r}`));
  const centers = footprint.map((cell) => ({
    cell,
    point: axialToPixel(cell, 26, { x: 0, y: 0 }),
  }));
  const minY = Math.min(...centers.map(({ point }) => point.y));
  const maxY = Math.max(...centers.map(({ point }) => point.y));
  const minX = Math.min(...centers.map(({ point }) => point.x));
  const maxX = Math.max(...centers.map(({ point }) => point.x));
  const midX = (minX + maxX) / 2;
  const map = required<HTMLElement>("inspect-output-map");
  map.style.height = `${Math.max(92, maxY - minY + 78)}px`;

  const cells = syncChildren(
    required<HTMLElement>("inspect-output-cells"),
    footprint.map(({ q, r }) => `${q},${r}`),
    () => {
      const cell = document.createElement("div");
      cell.className = "inspect-output-cell";
      return cell;
    },
  );
  centers.forEach(({ cell, point }, index) => {
    const element = cells[index]!;
    element.style.setProperty("--output-x", `${point.x - midX}px`);
    element.style.setProperty("--output-y", `${point.y - minY + 39}px`);
    element.classList.toggle(
      "active",
      cell.q === selectedRoute.q && cell.r === selectedRoute.r,
    );
    element.textContent = `q${cell.q}\nr${cell.r}`;
  });

  const ports = centers.flatMap(({ cell, point }) =>
    TRANSPORT_DIRECTIONS.slice(0, 6).flatMap((direction, index) => {
      if (footprintKeys.has(`${cell.q + direction.q},${cell.r + direction.r}`))
        return [];
      const step = axialToPixel(direction, 26, { x: 0, y: 0 });
      const length = Math.hypot(step.x, step.y) || 1;
      return [
        {
          q: cell.q,
          r: cell.r,
          direction: index,
          x: point.x - midX + (step.x / length) * 30,
          y: point.y - minY + 39 + (step.y / length) * 30,
          angle: Math.atan2(step.y, step.x),
        },
      ];
    }),
  );
  const portButtons = syncChildren(
    required<HTMLElement>("inspect-output-ports"),
    ports.map(({ q, r, direction }) => `${q},${r},${direction}`),
    () => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "inspect-output-port";
      button.innerHTML = "<span>›</span>";
      return button;
    },
  );
  ports.forEach((port, index) => {
    const button = portButtons[index]!;
    const active =
      port.q === selectedRoute.q &&
      port.r === selectedRoute.r &&
      port.direction === selectedRoute.direction;
    button.dataset.q = String(port.q);
    button.dataset.r = String(port.r);
    button.dataset.direction = String(port.direction);
    button.style.setProperty("--output-x", `${port.x}px`);
    button.style.setProperty("--output-y", `${port.y}px`);
    button.style.setProperty("--output-angle", `${port.angle}rad`);
    button.classList.toggle("active", active);
    button.setAttribute("aria-pressed", String(active));
    button.setAttribute(
      "aria-label",
      `Send ${item?.name ?? "product"} ${DIRECTION_NAMES[port.direction]} from footprint tile q${port.q}, r${port.r}`,
    );
    button.title = `${DIRECTION_NAMES[port.direction]} from q${port.q}, r${port.r}`;
  });
}

required<HTMLElement>("inspect-output-products").addEventListener(
  "click",
  (event) => {
    const button = (event.target as Element).closest<HTMLElement>(
      "[data-item-id]",
    );
    const building = selected ? buildingAt(selected) : undefined;
    const itemId = Number(button?.dataset.itemId);
    if (!building || !Number.isInteger(itemId)) return;
    selectedOutputProduct.set(building.id, itemId);
    renderOutputRouting(building);
  },
);

required<HTMLElement>("inspect-output-ports").addEventListener(
  "click",
  (event) => {
    const button = (event.target as Element).closest<HTMLElement>(
      ".inspect-output-port",
    );
    const building = selected ? buildingAt(selected) : undefined;
    if (!button || !building) return;
    const itemId = selectedOutputProduct.get(building.id);
    if (!itemId) return;
    enqueue({
      type: "set_output_route",
      q: building.q,
      r: building.r,
      item_id: itemId,
      output_q: Number(button.dataset.q),
      output_r: Number(button.dataset.r),
      direction: Number(button.dataset.direction),
    });
  },
);

const INVENTORY_PANEL = "inventory-panel";

/** The hex the pack was last offered beside, so the offer is made once per building, not per frame. */
let packOfferedFor: string | null = null;
/** Set once the player closes the pack themselves. After that the game never opens it again. */
let packDeclined = false;

/**
 * Whether the layout has room for the pack and the inspector at once.
 *
 * Read off a custom property rather than matched against a width written here, so the breakpoint
 * stays in the stylesheet where the rest of the layout keeps it and cannot drift out of step with
 * the rule that actually hides the inspector.
 */
function panelsFitAbreast(): boolean {
  return (
    getComputedStyle(document.documentElement)
      .getPropertyValue("--panels-abreast")
      .trim() === "1"
  );
}

/**
 * Open the pack beside a building that can take something out of it.
 *
 * Which buildings those are is derived from the compartments the snapshot publishes — the same
 * derivation the inspector draws and the demolition prompt reads — and not from a list of kinds. A
 * kind list would be a fourth place to remember every time a machine is added, and the two silent
 * ways it can be wrong are exactly the two that matter: a new machine the pack refuses to open for,
 * and an old one it opens for pointlessly.
 *
 * Only an explicit close declines future offers. Clicking the world also clears panels; that
 * automatic closure must not be mistaken for the player declining assistance.
 */
function offerPackBeside(building: EntitySnapshot | undefined): void {
  const open = panels.isOpen(INVENTORY_PANEL);
  const takes =
    building !== undefined &&
    stockCompartments(building).some(({ accepts }) => accepts);
  const key = takes && building ? `${building.q},${building.r}` : null;
  if (key === packOfferedFor) return;
  packOfferedFor = key;
  // Narrow layouts put the two panels in the same space, so opening the pack would take away the
  // machine the player just selected. There the pack stays behind its key.
  if (key === null || packDeclined || open || !panelsFitAbreast()) return;
  panels.reveal(INVENTORY_PANEL);
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
    renderOutputRouting(undefined);
    renderInspectorLoad(undefined);
    renderInspectorTier(undefined);
    renderInspectorRecipe(undefined);
    renderInspectorHub(undefined);
    offerPackBeside(undefined);
    return;
  }
  const building = selected ? buildingAt(selected) : undefined;
  offerPackBeside(building);
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
  // The band says what the generator drew; the grade on top of it says what the player has since
  // made of the hex. Only the pair answers "can I stand here", which is the question this panel is
  // being asked, so a quarried cliff stops reading Impassable the moment its face is down.
  const grade =
    snapshot.ground.find(({ q, r }) => q === selected?.q && r === selected?.r)
      ?.elevation ?? 0;
  const band = terrain ? bandAt(terrain, grade) : undefined;

  if (building) {
    kicker.textContent = "Building";
    title.textContent = definition?.name ?? titleCase(building.kind);
    status.hidden = false;
    status.textContent =
      definition?.manual_work && building.status === "switched off"
        ? building.progress > 0
          ? "work paused"
          : "awaiting player work"
        : building.status;
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
    paintHexFace(
      hex,
      band?.fill ?? fieldItem.color,
      band?.stroke ?? "#f4f7f5",
      !(band?.passable ?? true),
    );
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
    cargo.hidden = building.kind !== "belt" || !building.cargo;
    if (building.kind === "belt" && building.cargo)
      paintChip(
        required<HTMLElement>("inspect-cargo-chip"),
        building.cargo.item_id,
        { count: building.cargo.quantity },
      );
  }

  renderInspectorActions(building);
  renderOutputRouting(building);
  renderInspectorLoad(building);
  renderInspectorTier(building);
  renderInspectorRecipe(building);
  renderInspectorHub(building);
}

/**
 * The landing hub is the physical place where both progression loops are delivered, so inspecting
 * it presents both of them. Previously only standing requests had actions here; the founding
 * contract was visible in distant chrome but absent from the object that receives it.
 */
function renderInspectorHub(building: EntitySnapshot | undefined): void {
  const hubCard = required<HTMLElement>("inspect-hub");
  if (building?.kind !== "hub") {
    hubCard.hidden = true;
    return;
  }
  hubCard.hidden = false;

  const contract = snapshot.contract;
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
      (snapshot.player.inventory[String(need.item_id)] ??
        snapshot.player.inventory[need.item_id] ??
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
      snapshot.player.inventory[String(need.item_id)] ??
      snapshot.player.inventory[need.item_id] ??
      0;
    const remaining = Math.max(0, need.required - need.delivered);
    paintChip(part<HTMLElement>(row, ".inspect-hub-item"), need.item_id, {
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
  const requests = snapshot.requests.filter(
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
      snapshot.player.inventory[String(request.item_id)] ??
      snapshot.player.inventory[request.item_id] ??
      0;
    // What is owed is the bill less what has already been handed over — progress belongs to the
    // project now, so a row reposted after a skip asks only for the remainder.
    const owed = Math.max(0, request.required - request.delivered);
    const haveEnough = carried >= owed;

    paintChip(part<HTMLElement>(row, ".inspect-hub-item"), request.item_id, {
      progress: {
        have: Math.min(request.required, request.delivered + carried),
        need: request.required,
      },
      meter: true,
      shortfall: Math.max(0, owed - carried),
    });

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
  const manual = host.definitions.buildings.find(
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
  return host.definitions.recipes.filter((recipe) =>
    supportsRecipe(definition, recipe),
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
  const note = required<HTMLElement>("production-note");
  const message = productionNote(building, host.definitions);
  note.hidden = !message;
  note.textContent = message;
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
  const posted = snapshot.requests.filter(
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
      snapshot.player.inventory[String(request.item_id)] ??
      snapshot.player.inventory[request.item_id] ??
      0;
    const owed = Math.max(0, request.required - request.delivered);
    // Same chip as the bill and the pack: what has been handed over plus what is in the pack,
    // against the bill — so a project part-filled before a skip does not read as untouched.
    paintChip(part<HTMLElement>(row, ".request-item"), request.item_id, {
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
  renderProjectCatalogue();
}

/** How a project's state reads on a catalogue row, and whether it can be posted from there. */
const PROJECT_LABEL: Record<ProjectState, string> = {
  posted: "On the board",
  available: "Ready to post",
  complete: "Done",
  locked: "Not yet makeable",
};

/**
 * The whole bill of work.
 *
 * Finite demand only works if it is legible. Three slots out of twenty-two is a peephole, and a
 * player who cannot see the rest has no way to tell whether the insight they are about to spend is
 * replaceable — so the catalogue lists every project, what it pays, and where it stands, and lets
 * the player pull one onto the board rather than waiting for it to come round.
 */
function renderProjectCatalogue(): void {
  const list = required<HTMLElement>("project-catalogue-list");
  const projects = snapshot.requests;
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
    paintChip(part<HTMLElement>(row, ".request-item"), project.item_id, {
      progress: { have: project.delivered, need: project.required },
      meter: project.delivered > 0,
    });
    part(row, ".request-price").textContent = `+${project.insight} ◆`;
    part(row, ".project-state").textContent = PROJECT_LABEL[project.state];
    part(row, ".request-brief").textContent = project.brief;
    const post = part<HTMLButtonElement>(row, ".project-post");
    // The snapshot names projects by key and the command takes an id, so the definitions are the
    // join. A row whose key is not in the catalogue cannot be posted rather than posting nothing.
    const definition = host.definitions.requests.find(
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
  part(soundButton, ".utility-icon").textContent = value ? "♪̸" : "♪";
  part(soundButton, ".utility-label").textContent = value ? "Muted" : "Sound";
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

function syncSessionInputs(next: FactorySnapshot): void {
  confirmDialog.dismiss();
  endStackDrag();
  packOfferedFor = null;
  packDeclined = false;
  scenarioInput.value = next.scenario;
  showTitleScenario(next.scenario);
  seedInput.value = String(next.seed);
  titleSeedInput.value = String(next.seed);
  titleCreativeInput.checked = next.player.creative;
  showCreativeNote();
  void syncWorldInputs();
}

function selectTool(next: Tool): void {
  boundaryTool.close(false);
  groundTool.close(false);
  tool = next;
  renderer.setBuildMode(next !== "inspect");
  renderRecipePicker();
  renderHotbar();
  // Picking up a corner-only tool with an eastward heading held would carry an orientation the definition
  // cannot take, so the pending heading is snapped onto the new tool's axis. `setOrientation` does
  // the rest: the label, the footprint preview, and the refreshed legality all follow from it.
  const { start, end } = orientationRange(next);
  setOrientation(
    orientation >= start && orientation < end ? orientation : start,
  );
}

function enqueue(command: NativeInputCommand): boolean {
  const accepted = input.enqueue(command);
  if (!accepted)
    showFeedback(
      "Input queue full; command deferred by the bounded host limit",
    );
  return accepted;
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
  // Vertex tools follow the pointer within a hex, so they are told even when the hex has not moved.
  const point = renderer.pickWorld(aimPointer.x, aimPointer.y);
  boundaryTool.hover(coordinate, point);
  groundTool.hover(coordinate, point);
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

required<HTMLButtonElement>("build-scope").addEventListener("click", () => {
  showAllBuildings = !showAllBuildings;
  renderBuildPanel();
});
/*
 * The dock's overflow cues.
 *
 * The shelf has always scrolled sideways on a narrow window with its scrollbar hidden, so slots and
 * the catalogue opener could be off the edge with nothing saying so. This measures the real scroll
 * position and lets the stylesheet fade the edge that has content behind it and reveal the matching
 * nudge. Measurement, not a guess at a breakpoint: the dock's width depends on how many slots are
 * filled and on the window, and a breakpoint would be wrong on one of those the moment it is right
 * on the other.
 */
{
  const shelf = required<HTMLDivElement>("tool-shelf");
  const dock = shelf.closest<HTMLElement>(".build-dock");
  const update = (): void => {
    if (!dock) return;
    const slack = shelf.scrollWidth - shelf.clientWidth;
    dock.classList.toggle("overflow-start", shelf.scrollLeft > 2);
    dock.classList.toggle("overflow-end", shelf.scrollLeft < slack - 2);
  };
  shelf.addEventListener("scroll", update, { passive: true });
  // Width is the only thing that moves the answer. The shelf always carries the same nine slots and
  // the same fixed tools, so its content width is settled at load; what changes is how much room
  // the window leaves it, and that is exactly what a resize observer reports. Watching content
  // instead would re-measure on every repaint of a caption, which is a forced layout per frame in
  // exchange for a fact that cannot have changed.
  new ResizeObserver(update).observe(shelf);
  for (const nudge of document.querySelectorAll<HTMLButtonElement>(
    ".shelf-nudge",
  ))
    nudge.addEventListener("click", () => {
      shelf.scrollBy({
        left: nudge.dataset.nudge === "back" ? -160 : 160,
        behavior: "smooth",
      });
    });
  update();
}
{
  const search = required<HTMLInputElement>("build-search");
  search.addEventListener("input", () => {
    buildSearch = search.value;
    renderBuildPanel();
  });
  // Escape in a filled box clears the filter: a player who has just typed one is asking to undo it,
  // not to leave. In an empty box it hands the keyboard back to the world, so the next Escape
  // closes the panel the way it does everywhere else — a focused text field otherwise swallows
  // Escape completely, and a panel that will not close is the more surprising of the two.
  search.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    if (search.value === "") {
      search.blur();
      return;
    }
    search.value = "";
    buildSearch = "";
    renderBuildPanel();
  });
}
{
  const search = required<HTMLInputElement>("recipe-search");
  search.addEventListener("input", () => {
    recipeSearch = search.value;
    renderRecipePanel();
  });
  // Escape behaves as it does in the catalogue box: it undoes the filter first, and only then
  // hands the keyboard back so the next press closes the panel.
  search.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    if (search.value === "") {
      search.blur();
      return;
    }
    search.value = "";
    recipeSearch = "";
    renderRecipePanel();
  });
}
/*
 * A looked-up recipe is still a build. Clicking a row hands over the machine that runs it, already
 * set to that recipe, which is the same gesture the catalogue's own recipe rows make — the lookup
 * would otherwise be the one place in the game that tells you the answer and leaves you to go find
 * the machine yourself.
 */
required<HTMLElement>("recipe-results").addEventListener("click", (event) => {
  const row = (event.target as Element).closest<HTMLElement>(".lookup-row");
  if (!row) return;
  const definition = host.definitions.buildings.find(
    ({ id }) => id === Number(row.dataset.definitionId),
  );
  if (!definition) {
    showFeedback("No machine in the catalogue runs that recipe");
    return;
  }
  if (
    buildingAvailability(definition, snapshot, host.definitions.items).locked
  ) {
    showFeedback(`${definition.name} is still locked by research`);
    return;
  }
  const recipeId = Number(row.dataset.recipeId);
  selectedRecipes.set(definition.id, recipeId);
  selectTool(definition.id);
  closePanels();
  showFeedback(
    `Holding ${definition.name} set to ${
      host.definitions.recipes.find(({ id }) => id === recipeId)?.name ??
      "that recipe"
    } — click or drag on the world to place`,
  );
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
required<HTMLButtonElement>("turn").addEventListener("click", () =>
  rotateNewBuilding(),
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
// Delegated, because both hub lists come and go as deliveries complete stages and requests.
required<HTMLElement>("inspect-hub").addEventListener("click", (event) => {
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
});
// Delegated for the same reason: catalogue rows are patched in place as projects complete.
required<HTMLElement>("project-catalogue-list").addEventListener(
  "click",
  (event) => {
    const post = (event.target as HTMLElement).closest<HTMLButtonElement>(
      ".project-post",
    );
    if (!post || post.disabled) return;
    const requestId = Number(post.dataset.projectId);
    if (Number.isInteger(requestId) && requestId > 0)
      enqueue({ type: "post_request", request_id: requestId });
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
/** What Start scenario will generate. Native validates it again on arrival. */
let pendingWorld: WorldParams | null = null;

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
// rebuilt for, which is the same rule the catalogue and the research list live under. The two
// mountings are the same form: neither owns the values, both report a whole set, and both are
// shown whatever the other reported.
/** What the preview panel needs from an item, looked up once per draw rather than kept in a copy. */
function previewItemLook(itemId: number): PreviewItemLook | undefined {
  const item = host.definitions.items.find((entry) => entry.id === itemId);
  return item ? { name: item.name, color: item.color } : undefined;
}

const worldPreviewPanels: WorldPreviewPanel[] = [
  new WorldPreviewPanel(
    "world-preview",
    previewItemLook,
    () => requestWorldPreview(),
    applyPreviewRepair,
  ),
  new WorldPreviewPanel(
    "title-world-preview",
    previewItemLook,
    () => requestWorldPreview(),
    applyPreviewRepair,
  ),
];

const worldParameterForm = new WorldParameterForm(
  worldParameterFields,
  "world-param",
  (next) => showWorldParams(next),
  worldPreviewPanels[0],
);
const titleWorldParameterForm = new WorldParameterForm(
  titleWorldParameterFields,
  "title-world-param",
  (next) => showWorldParams(next),
  worldPreviewPanels[1],
);

/**
 * The seed the preview draws, read from the same field the Start button reads. A world is its
 * parameters *and* its seed, so a preview of a different seed would be a picture of a world nobody
 * is about to generate.
 */
function previewSeed(): number {
  const parsed = Number(titleSeedInput.value);
  return Number.isFinite(parsed)
    ? Math.abs(Math.trunc(parsed)) % 4294967296
    : 0;
}

let worldPreviewTimer: number | undefined;
let worldPreviewTicket = 0;

/**
 * Redraw the preview, at most once per idle moment.
 *
 * Debounced because a slider drag is a stream of edits and each one is a raster, and ticketed
 * because the worker answers in order but a drag can outrun it — a picture that arrives after the
 * parameters moved on is a picture of a world the player has already left.
 */
function requestWorldPreview(): void {
  if (worldPreviewTimer !== undefined) clearTimeout(worldPreviewTimer);
  worldPreviewTimer = window.setTimeout(() => {
    void drawWorldPreview();
  }, 120);
}

async function drawWorldPreview(): Promise<void> {
  const params = pendingWorld;
  if (!params) return;
  // Both forms exist from boot and only one is ever shown, so rastering for the hidden one would be
  // asking the generator to draw a picture nobody is looking at.
  const panels = worldPreviewPanels.filter((panel) => panel.visible);
  if (panels.length === 0) return;
  worldPreviewTicket += 1;
  const ticket = worldPreviewTicket;
  const seed = previewSeed();
  // Asked for together rather than one after the other. The worker runs a queue either way, so this
  // costs nothing extra — but it puts every panel of a request in front of the next request, where a
  // sequential loop would let a slider drag keep starving whichever panel came last.
  await Promise.all(
    panels.map(async (panel) => {
      try {
        const preview = await host.worldPreview(
          params,
          seed,
          PREVIEW_WIDTH,
          PREVIEW_HEIGHT,
          panel.hexesAcross,
        );
        if (ticket === worldPreviewTicket) panel.draw(preview, params);
      } catch (error) {
        if (ticket !== worldPreviewTicket) return;
        // Native refuses a set the Start button would also refuse, so this is the panel saying what
        // is wrong with the parameters rather than the host reporting a worker fault.
        panel.showError(error instanceof Error ? error.message : String(error));
      }
    }),
  );
}

/**
 * Scenario as a card each rather than a dropdown. The shipped list already carries a sentence
 * about every scenario, and a bare name does not tell a first-time player what they are choosing.
 * The session panel keeps its select — that one is a running game's control, not a first
 * impression, and the two stay in step through the handlers below.
 */
const titleScenarioChoiceInputs = new Map<string, HTMLInputElement>();
for (const scenario of host.scenarios.scenarios) {
  const card = document.createElement("label");
  card.className = "choice-card";
  const choice = document.createElement("input");
  choice.type = "radio";
  choice.name = "title-scenario";
  choice.value = scenario.key;
  choice.checked = scenario.key === scenarioInput.value;
  choice.addEventListener("change", () => {
    if (choice.checked) scenarioInput.value = scenario.key;
  });
  const body = document.createElement("span");
  body.className = "choice-card-body";
  const name = document.createElement("strong");
  name.textContent = scenario.name;
  const note = document.createElement("small");
  note.textContent = scenario.description;
  body.append(name, note);
  card.append(choice, body);
  titleScenarioChoices.append(card);
  titleScenarioChoiceInputs.set(scenario.key, choice);
}

/** The scenario the title screen is offering, falling back to the panel's own pick. */
function titleScenarioKey(): string {
  for (const [key, choice] of titleScenarioChoiceInputs) {
    if (choice.checked) return key;
  }
  return scenarioInput.value;
}

function showTitleScenario(key: string): void {
  for (const [candidate, choice] of titleScenarioChoiceInputs) {
    choice.checked = candidate === key;
  }
}

/**
 * Apply a repair the preview offered. Both halves are already verified against a real bootstrap
 * pass; this is only how they land on the same fields the player already has.
 */
function applyPreviewRepair(choice: RepairChoice): void {
  if (choice.kind === "seed") {
    seedInput.value = String(choice.seed);
    titleSeedInput.value = String(choice.seed);
    requestWorldPreview();
    return;
  }
  if (!pendingWorld) return;
  showWorldParams(applyChanges(pendingWorld, choice.changes));
}

function showWorldParams(params: WorldParams): void {
  pendingWorld = params;
  worldParameterForm.setValues(params);
  titleWorldParameterForm.setValues(params);
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
  requestWorldPreview();
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
  showTitleScenario(scenarioInput.value);
});
seedInput.addEventListener("input", () => {
  titleSeedInput.value = seedInput.value;
  requestWorldPreview();
});
titleSeedInput.addEventListener("input", () => {
  seedInput.value = titleSeedInput.value;
  requestWorldPreview();
});
titleSeedRandom.addEventListener("click", () => {
  const randomized = Math.floor(Math.random() * 4294967295);
  seedInput.value = String(randomized);
  titleSeedInput.value = String(randomized);
  requestWorldPreview();
});

/** The one place the run's save name is set, so the panel field and the catalogue cannot disagree. */
function setRunName(name: string): void {
  runName = name;
  saveNameInput.value = name;
}

/**
 * What the mode switch is promising, in the present tense. The card's copy explains what creative
 * does; this line says what the player is currently choosing, which is the part that changes.
 */
function showCreativeNote(): void {
  titleCreativeNote.textContent = titleCreativeInput.checked
    ? "Creative run: the clock still counts, but the run is marked as not comparable and earns no achievements."
    : "Standard run: everything is built and earned, and the run time counts.";
}

titleCreativeInput.addEventListener("change", showCreativeNote);

// The top bar belongs to a running factory. Behind the title screen it is a strip of controls for a
// game the player has not chosen yet, so the shell drops the row entirely rather than dimming it —
// the renderer watches the canvas for resizes, so the reclaimed height is picked up on its own.
function setTitleOpen(open: boolean): void {
  document.body.classList.toggle("title-open", open);
}

function openTitleScreen(): void {
  titleScreen.classList.add("open");
  titleResume.hidden = false;
  setTitleOpen(true);
  // A blank field means "name this one for me". Carrying the running factory's name over would
  // make the obvious next click overwrite the save the player just walked away from.
  titleSaveNameInput.value = "";
  showCreativeNote();
  updateContinueState();
  // The panels are built at boot but only raster while they are on screen, so opening the screen is
  // the moment the first picture can be drawn.
  requestWorldPreview();
}

function closeTitleScreen(): void {
  titleScreen.classList.remove("open");
  titleResume.hidden = false;
  setTitleOpen(false);
  canvas.focus();
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
  if (!showSaves) requestWorldPreview();
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
  const scenario = titleScenarioKey();
  // A typed name is an instruction — if it matches a slot, the player means that slot. A defaulted
  // one is not, so it steps aside rather than overwriting a factory nobody asked to replace.
  const typed = titleSaveNameInput.value.trim();
  const fallback =
    host.scenarios.scenarios.find((entry) => entry.key === scenario)?.name ??
    AUTOSAVE_SLOT_NAME;
  try {
    const next = await host.newGame(
      scenario,
      seed,
      pendingWorld ?? undefined,
      titleCreativeInput.checked,
    );
    setRunName(
      typed || uniqueSlotName(fallback, readCatalog(localStorage).slots),
    );
    selectedSaveId = null;
    beginRun(next);
    update(next);
    syncSessionInputs(next);
    renderer.recenter();
    // Nothing has happened in a world this new, so there is nothing for the close guard to save.
    markSaved(next.tick);
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
    markSaved(next.tick);
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
  run = startRun(Date.now(), snapshot.tick);
  writeRun(localStorage, run);
  renderRun();
  required<HTMLElement>("run-status").textContent = "Timer reset.";
});

required<HTMLButtonElement>("save").addEventListener("click", async () => {
  // Read before the round trip, so the mark never claims more of the run is written than is.
  const tick = snapshot.tick;
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
      named || selected?.name || runName || snapshot.scenario_name || "Save";
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
    markSaved(tick);
    selectedSaveId = drafted.id;
    // Saving under a name adopts it: the auto-save follows the player rather than continuing to
    // write to the name they just moved away from.
    setRunName(drafted.name);
    updateContinueState(`Saved “${drafted.name}”.`);
    showFeedback("Game saved");
    offerSaveFile(drafted);
  } catch (error) {
    updateContinueState(`Save failed: ${String(error)}`);
  }
});

/**
 * The slot landed; ask whether a copy should go to disk as well.
 *
 * A named slot lives in this browser's storage, which the browser may clear without asking anyone.
 * The offer is made here because this is the one moment the player has already said the run is worth
 * keeping — leaving it to the Export button means only players who already know about it are safe.
 * The accept click is also the user gesture the file picker needs, so the file dialog opens straight
 * from the answer rather than being refused for want of an activation.
 */
function offerSaveFile(slot: SaveSlot): void {
  confirmDialog.ask(
    {
      title: `Saved “${slot.name}”`,
      note: "That save is in this browser, and clearing site data removes it. Keep a copy as a file too?",
      accept: "Save to file",
      cancel: "Browser only",
    },
    () => void exportSlotFile(slot),
  );
}
required<HTMLButtonElement>("continue").addEventListener("click", () => {
  const slot = latestCompatible(
    readCatalog(localStorage).slots,
    currentBuild(),
  );
  if (slot) void loadSlot(slot);
});
required<HTMLButtonElement>("export-save").addEventListener("click", () => {
  void exportCurrentSave();
});
required<HTMLButtonElement>("import-save").addEventListener("click", () => {
  openSaveFilePicker();
});
required<HTMLButtonElement>("title-export-saves").addEventListener(
  "click",
  () => {
    void exportAllSaves();
  },
);
required<HTMLButtonElement>("title-import-saves").addEventListener(
  "click",
  () => {
    openSaveFilePicker();
  },
);
saveFileInput.addEventListener("change", () => {
  void importSaveFiles(saveFileInput.files);
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
  // The refusal a locked slot used to make by being unclickable, made in words instead — and made
  // here, so the × above it stays live. The catalogue says the same sentence for the same reason.
  if (button.getAttribute("aria-disabled") === "true") {
    showFeedback("That building is still locked by research");
    return;
  }
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
    closePanels();
    showFeedback(
      `Holding ${host.definitions.buildings.find(({ id }) => id === definitionId)?.name ?? "building"} — click or drag on the world to place`,
    );
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
  closePanels();
  showFeedback(
    `Holding ${host.definitions.buildings.find(({ id }) => id === definitionId)?.name ?? "building"} — click or drag on the world to place`,
  );
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
function stackGesture(event: MouseEvent): void {
  const slot = (event.target as Element).closest<HTMLElement>(
    "[data-stack-source]",
  );
  if (!slot) return;
  event.preventDefault();
  const source = slot.dataset.stackSource;
  if (source !== "player" && source !== "building") return;
  const hand = snapshot.player.hand ?? undefined;
  const right = event.type === "contextmenu" || event.button === 2;
  const itemId = Number(slot.dataset.itemId);
  const available = Number(slot.dataset.quantity) || 0;
  const quantity =
    event.ctrlKey || event.metaKey
      ? 1
      : right
        ? halfTransfer(available)
        : available;

  // A held stack turns every accepting slot into a destination. Right-click and Ctrl-click place
  // one; a normal left-click places as much as the destination has room for. Output is never a
  // drop target — native will not take a hand-placed brick back into a kiln's product buffer.
  if (hand) {
    if (source === "building" && slot.dataset.accepts === "0") return;
    const placed = right || event.ctrlKey || event.metaKey ? 1 : hand.quantity;
    if (source === "player") {
      enqueue({ type: "place_player_stack", quantity: placed });
      return;
    }
    const stock = slot.dataset.stock as Exclude<StockKind, "auto">;
    enqueue({
      type: "place_building_stack",
      q: Number(slot.dataset.q),
      r: Number(slot.dataset.r),
      stock,
      quantity: placed,
    });
    return;
  }

  if (!Number.isInteger(itemId) || itemId <= 0 || quantity <= 0) return;
  if (source === "building" && itemById(itemId)?.fluid) {
    showFeedback(
      "Loose fluid moves through pipes. Use a barrel station before handling it as an item.",
    );
    return;
  }
  if (event.shiftKey) {
    const quickQuantity =
      event.ctrlKey || event.metaKey
        ? 1
        : right
          ? halfTransfer(available)
          : available;
    if (source === "building") {
      enqueue({
        type: "withdraw",
        q: Number(slot.dataset.q),
        r: Number(slot.dataset.r),
        stock: slot.dataset.stock as Exclude<StockKind, "auto">,
        item_id: itemId,
        quantity: quickQuantity,
      });
      return;
    }
    const target = selected ? buildingAt(selected) : undefined;
    if (!target || !HAND_REACHABLE.has(target.kind)) return;
    enqueue({
      type: "store",
      q: target.q,
      r: target.r,
      stock: "auto",
      item_id: itemId,
      quantity: quickQuantity,
    });
    return;
  }

  if (source === "player") {
    enqueue({ type: "pickup_player_stack", item_id: itemId, quantity });
  } else {
    enqueue({
      type: "pickup_building_stack",
      q: Number(slot.dataset.q),
      r: Number(slot.dataset.r),
      stock: slot.dataset.stock as Exclude<StockKind, "auto">,
      item_id: itemId,
      quantity,
    });
  }
}

/**
 * A stack being dragged from one slot to another.
 *
 * Nothing is sent to native while a drag is in flight. The pickup and the placement are enqueued
 * together when the drop lands, so a drag released over nothing is not an undo of anything — it
 * simply never happened, and the stack is still exactly where the player pressed. That is what makes
 * "released over nothing returns the stack" true rather than merely usually true: there is no window
 * in which the item is in a hand the player did not ask for, so a dropped connection, a closed
 * panel, or a demolished machine mid-gesture cannot strand it.
 *
 * The gesture rides on the same native commands as the clicks: press-lift is `pickup_*`, and
 * release-place is `place_*`, with the same modifier rules for how much moves. Pointer events rather
 * than mouse events, so a finger drags a stack the same way a mouse does.
 */
interface StackDrag {
  readonly pointerId: number;
  readonly source: "player" | "building";
  readonly origin: HTMLElement;
  readonly itemId: number;
  readonly quantity: number;
  readonly pickup: NativeInputCommand;
  readonly startX: number;
  readonly startY: number;
  lifted: boolean;
}

/** How far the pointer travels before a press becomes a drag rather than a click. */
const STACK_DRAG_LIFT = 6;

let stackDrag: StackDrag | null = null;
/** A completed drag swallows the click the browser synthesises after it. */
let stackDragHandledClick = false;

// Consume the drag's synthetic click even when its target is outside either grid. A fresh press
// always starts a new gesture, so a browser that emits no click cannot swallow the next real one.
window.addEventListener(
  "pointerdown",
  () => {
    stackDragHandledClick = false;
  },
  true,
);
window.addEventListener(
  "click",
  (event) => {
    if (!stackDragHandledClick) return;
    stackDragHandledClick = false;
    event.preventDefault();
    event.stopImmediatePropagation();
  },
  true,
);

/** Every slot the two grids are currently showing, drag source and drop target alike. */
function stackSlots(): HTMLElement[] {
  return ["inventory", "inspector-actions"].flatMap((id) =>
    Array.from(
      required<HTMLElement>(id).querySelectorAll<HTMLElement>(
        "[data-stack-source]",
      ),
    ),
  );
}

/**
 * Whether this drag could land on this slot, and why not when it could not.
 *
 * The reason is written here rather than left to native because a drop that quietly does nothing is
 * the failure this whole gesture exists to remove. A slot that cannot take the stack never lights up
 * in the first place, and a release on one says so.
 */
function stackDropRefusal(drag: StackDrag, slot: HTMLElement): string | null {
  if (slot === drag.origin) return "";
  const source = slot.dataset.stackSource;
  if (source === "player") {
    // The pack is one pool, not an arrangement — native has no notion of which slot a stack sits in,
    // so a drag inside it would be a gesture with nothing to change.
    return drag.source === "player" ? "" : null;
  }
  if (source !== "building") return "";
  if (slot.dataset.accepts === "0")
    return "That compartment does not take items";
  const held = Number(slot.dataset.itemId);
  if (held > 0 && held !== drag.itemId)
    return `That slot is holding ${itemById(held)?.name ?? "something else"}`;
  return null;
}

/** Light the slots this drag could land on, and mark the one under the pointer. */
function paintStackDropTargets(
  drag: StackDrag | null,
  over?: Element | null,
): void {
  for (const slot of stackSlots()) {
    const allowed = drag !== null && stackDropRefusal(drag, slot) === null;
    slot.classList.toggle("drop-ready", allowed);
    slot.classList.toggle("drop-over", allowed && slot === over);
  }
}

/** Put the floating stack away and let the next frame's render own the cursor again. */
function endStackDrag(): void {
  if (stackDrag?.lifted) {
    required<HTMLElement>("cursor-stack").hidden = !snapshot.player.hand;
    paintStackDropTargets(null);
    document.body.classList.remove("dragging-stack");
  }
  stackDrag = null;
}

for (const id of ["inventory", "inspector-actions"]) {
  const grid = required<HTMLElement>(id);
  grid.addEventListener("click", stackGesture);
  grid.addEventListener("contextmenu", stackGesture);
  grid.addEventListener("pointerdown", (event) => {
    // Only the primary button drags. The secondary one already means "half", and taking it over
    // would cost the player a gesture they have been using since the panel existed.
    if (
      event.button !== 0 ||
      !event.isPrimary ||
      snapshot.player.hand ||
      stackDrag
    )
      return;
    const slot = (event.target as Element).closest<HTMLElement>(
      "[data-stack-source]",
    );
    const source = slot?.dataset.stackSource;
    if (!slot || (source !== "player" && source !== "building")) return;
    const itemId = Number(slot.dataset.itemId);
    const available = Number(slot.dataset.quantity) || 0;
    if (!Number.isInteger(itemId) || itemId <= 0 || available <= 0) return;
    if (source === "building" && itemById(itemId)?.fluid) {
      showFeedback(
        "Loose fluid cannot be lifted by hand — connect a pipe or empty it into a barrel.",
      );
      return;
    }
    stackDrag = {
      pointerId: event.pointerId,
      source,
      origin: slot,
      itemId,
      quantity: event.ctrlKey || event.metaKey ? 1 : available,
      // The inspector's keyed slot can be reused for another building during a drag. Freeze the
      // source address at the press; never take it from that mutable element at release.
      pickup:
        source === "player"
          ? {
              type: "pickup_player_stack",
              item_id: itemId,
              quantity: event.ctrlKey || event.metaKey ? 1 : available,
            }
          : {
              type: "pickup_building_stack",
              q: Number(slot.dataset.q),
              r: Number(slot.dataset.r),
              stock: slot.dataset.stock as Exclude<StockKind, "auto">,
              item_id: itemId,
              quantity: event.ctrlKey || event.metaKey ? 1 : available,
            },
      startX: event.clientX,
      startY: event.clientY,
      lifted: false,
    };
  });
}

window.addEventListener("pointermove", (event) => {
  const drag = stackDrag;
  if (!drag || event.pointerId !== drag.pointerId) return;
  if (!drag.lifted) {
    const travelled =
      Math.abs(event.clientX - drag.startX) +
      Math.abs(event.clientY - drag.startY);
    if (travelled < STACK_DRAG_LIFT) return;
    // The slot may have been repainted between the press and the lift, so the amount is re-read.
    // The element itself survives — the grids are keyed and patched in place — but its contents do
    // not, and lifting more than is there would make the drop a refusal at the far end.
    if (Number(drag.origin.dataset.quantity) < drag.quantity) {
      stackDrag = null;
      return;
    }
    drag.lifted = true;
    document.body.classList.add("dragging-stack");
    const cursor = required<HTMLElement>("cursor-stack");
    cursor.hidden = false;
    paintChip(cursor, drag.itemId, {
      count: drag.quantity,
      named: false,
      short: true,
    });
  }
  event.preventDefault();
  paintStackDropTargets(
    drag,
    document
      .elementFromPoint(event.clientX, event.clientY)
      ?.closest("[data-stack-source]"),
  );
});

window.addEventListener("pointerup", (event) => {
  const drag = stackDrag;
  if (!drag || event.pointerId !== drag.pointerId) return;
  const lifted = drag.lifted;
  const slot = lifted
    ? document
        .elementFromPoint(event.clientX, event.clientY)
        ?.closest<HTMLElement>("[data-stack-source]")
    : null;
  endStackDrag();
  // An unlifted press is a click, and the click handler is about to run with the gesture the player
  // actually made. Only a real drag swallows it.
  if (!lifted) return;
  stackDragHandledClick = true;
  if (!slot) {
    showFeedback("Stack returned — drop it on a slot to move it");
    return;
  }
  const refusal = stackDropRefusal(drag, slot);
  if (refusal !== null) {
    if (refusal) showFeedback(refusal);
    return;
  }
  const place: NativeInputCommand =
    slot.dataset.stackSource === "player"
      ? { type: "place_player_stack", quantity: drag.quantity }
      : {
          type: "place_building_stack",
          q: Number(slot.dataset.q),
          r: Number(slot.dataset.r),
          stock: slot.dataset.stock as Exclude<StockKind, "auto">,
          quantity: drag.quantity,
        };
  if (!input.enqueueBatch([drag.pickup, place]))
    showFeedback("Too many commands — stack stayed where it was. Try again.");
});

// A cancelled pointer — the browser taking over for a gesture, a window losing focus — is a release
// over nothing, and lands in the same place: nothing was sent, so nothing has to be put back.
window.addEventListener("pointercancel", (event) => {
  if (stackDrag?.pointerId === event.pointerId) endStackDrag();
});

let cursorStackX = 0;
let cursorStackY = 0;
window.addEventListener("pointermove", (event) => {
  cursorStackX = event.clientX;
  cursorStackY = event.clientY;
  const cursor = required<HTMLElement>("cursor-stack");
  cursor.style.left = `${cursorStackX}px`;
  cursor.style.top = `${cursorStackY}px`;
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

/**
 * Demolish one building, asking first when it is holding something.
 *
 * Native no longer refuses a recovery the pack cannot hold — it carries what fits and drops the
 * rest at the site — so the one thing left to get right is that the player knows before it happens
 * rather than after. What is inside is named, and so is the clock the remainder falls onto, because
 * a minute is not long enough to discover by being surprised by it.
 *
 * An empty building is still one press. The question is asked about stock the player deliberately
 * put somewhere, so a belt's cargo in transit does not raise it: that has always spilled, it is one
 * item, and a prompt on every belt would make clearing a line unusable.
 */
function eraseBuilding(target: { q: number; r: number }): void {
  const building = buildingAt(target);
  if (!building) {
    showFeedback("No building selected to delete");
    return;
  }
  selected = target;
  renderer.setSelection(target);
  const erase = (): void =>
    void enqueue({ type: "erase", q: target.q, r: target.r });
  const held = heldStock(building);
  if (held.length === 0 && building.progress === 0) {
    erase();
    return;
  }
  const name =
    host.definitions.buildings.find(({ id }) => id === building.definition_id)
      ?.name ?? "building";
  confirmDialog.ask(
    {
      title: `Demolish the ${name}?`,
      rows: held.map((entry) => ({
        text: `${entry.quantity} × ${itemById(entry.item_id)?.name ?? "item"}`,
        paint: (host_) =>
          void paintChip(host_, entry.item_id, { named: false }),
      })),
      note: SPILL_NOTE,
      accept: "Demolish",
      cancel: "Keep it",
    },
    erase,
  );
}

/** Everything one building is holding that the player put there, newest question first. */
function heldStock(
  building: EntitySnapshot,
): { item_id: number; quantity: number }[] {
  if (!HAND_REACHABLE.has(building.kind)) return [];
  return stockCompartments(building)
    .flatMap(({ entries }) => entries)
    .filter(({ quantity }) => quantity > 0);
}

const SPILL_NOTE =
  "What fits goes back to your pack. Anything that does not fit falls at the site, and ground items disappear after about a minute of simulation time.";

/**
 * A removal drag, asked about once for the whole sweep.
 *
 * One prompt per building would make clearing a factory unusable, and no prompt at all would make
 * the sweep the one route that empties a row of full containers without saying so. So the question
 * is asked once, over the totals, and the drag is either taken or dropped entire.
 */
async function eraseLine(
  from: { q: number; r: number },
  to: { q: number; r: number },
): Promise<void> {
  // Ask for the released endpoints, not the last asynchronous hover preview, which can still be
  // in flight. A fast sweep must not silently demolish stock outside an older preview.
  let cells;
  try {
    cells = await host.linePreview(from.q, from.r, to.q, to.r);
  } catch (error) {
    showFeedback(`Removal cancelled: ${String(error)}`);
    return;
  }
  const send = (): void =>
    void enqueue({
      type: "erase_line",
      q: from.q,
      r: from.r,
      to_q: to.q,
      to_r: to.r,
    });
  const seen = new Set<number>();
  const totals = new Map<number, number>();
  let buildings = 0;
  for (const cell of cells.filter(({ legal }) => legal)) {
    const building = buildingAt(cell);
    if (!building || seen.has(building.id)) continue;
    seen.add(building.id);
    const held = heldStock(building);
    if (held.length === 0 && building.progress === 0) continue;
    buildings += 1;
    for (const entry of held)
      totals.set(
        entry.item_id,
        (totals.get(entry.item_id) ?? 0) + entry.quantity,
      );
  }
  if (buildings === 0) {
    send();
    return;
  }
  confirmDialog.ask(
    {
      title:
        buildings === 1
          ? "Demolish 1 building with stock inside?"
          : `Demolish ${buildings} buildings with stock inside?`,
      rows: [...totals].map(([itemId, quantity]) => ({
        text: `${quantity} × ${itemById(itemId)?.name ?? "item"}`,
        paint: (holder: HTMLElement) =>
          void paintChip(holder, itemId, { named: false }),
      })),
      note: SPILL_NOTE,
      accept: "Demolish",
      cancel: "Keep them",
    },
    send,
  );
}

/** Delete the hovered building when there is one, otherwise the selected building. */
function deleteBuildingUnderCursorOrSelected(): void {
  const target = hover && buildingAt(hover) ? hover : selected;
  if (!target) {
    showFeedback("No building selected to delete");
    return;
  }
  eraseBuilding(target);
}

window.addEventListener("keydown", (event) => {
  if (event.code === "Escape" && stackDrag) {
    endStackDrag();
    event.preventDefault();
    return;
  }
  // A question owns the keyboard entirely while it is up. It has its own two buttons and its own
  // `Escape`, and a build key that fired past it would edit the world the player is being asked
  // about — so unlike the two panels below, there is no key that reaches through it.
  if (confirmDialog.open) return;
  if (researchDialog.open || skillsDialog.open) {
    if (
      ((researchDialog.open && event.code === "KeyO") ||
        (skillsDialog.open && event.code === "KeyK")) &&
      !isTypingTarget(event.target) &&
      !event.repeat &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.altKey
    ) {
      event.preventDefault();
      panels.close();
    }
    return;
  }
  if (isTypingTarget(event.target)) return;
  // Space presses a button the keyboard tabbed to. A mouse-focused button must not keep it:
  // activation happens on keyup, so returning here would both skip recenter and click the control.
  if (event.code === "Space" && isKeyboardFocusedControl(event.target)) return;
  // Undo is the one binding that keeps its modifier, because every other application uses it.
  if ((event.ctrlKey || event.metaKey) && event.code === "KeyZ") {
    event.preventDefault();
    enqueue({
      type: boundaryTool.active
        ? "undo_boundary"
        : groundTool.active
          ? "undo_ground"
          : "undo",
    });
    return;
  }
  if (event.ctrlKey || event.metaKey || event.altKey) return;
  if (
    boundaryTool.active &&
    ["Escape", "KeyR", "Delete", "Backspace"].includes(event.code)
  ) {
    event.preventDefault();
    if (event.code === "Escape") boundaryTool.escape();
    else if (event.code === "KeyR") boundaryTool.cycleAction(event.shiftKey);
    else boundaryTool.selectRemoval();
    return;
  }
  if (
    groundTool.active &&
    ["Escape", "KeyR", "Delete", "Backspace"].includes(event.code)
  ) {
    event.preventDefault();
    if (event.code === "Escape") groundTool.escape();
    else if (event.code === "KeyR") groundTool.cycleAction(event.shiftKey);
    else groundTool.selectStrip();
    return;
  }
  if (event.code === "Backspace" || event.code === "Delete") {
    event.preventDefault();
    if (!event.repeat) deleteBuildingUnderCursorOrSelected();
    return;
  }
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
    if (panels.isOpen(INVENTORY_PANEL)) packDeclined = true;
    closePanels();
  }
  // Space centres the camera, which is what the button beside it does and what a player who has
  // panned away needs most.
  else if (event.code === "Space") renderer.recenter();
  else if (event.code === "Comma") orbitView(-1);
  else if (event.code === "Period") orbitView(1);
  else if (event.code === "KeyM") setMuted(!audio.isMuted);
  else if (event.code in PANEL_KEYS)
    togglePanel(PANEL_KEYS[event.code] as string);
  else if (event.code === "KeyF") {
    // Held rather than tapped. A swing has to be worked through natively before it pays, so the
    // repeat cannot outrun the simulation however fast the frames arrive.
    gatherHeld = true;
    enqueue({ type: "gather" });
  } else if (event.code === "KeyX") enqueue({ type: "deposit" });
  else if (event.code === "KeyR") rotateUnderCursorOrPending(event.shiftKey);
  else if (event.code === "KeyG") {
    if (groundTool.active) groundTool.close();
    else groundTool.open();
  } else if (event.code === "KeyQ") pickToolUnderCursor();
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
  if (confirmDialog.open || researchDialog.open || skillsDialog.open) return;
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
  endStackDrag();
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
  const vertexPoint = renderer.pickWorld(event.clientX, event.clientY);
  boundaryTool.hover(coordinate, vertexPoint);
  groundTool.hover(coordinate, vertexPoint);
  refreshHoverPreview();
});
canvas.addEventListener("pointerdown", (event) => {
  if (boundaryTool.active && event.button === 2) {
    event.preventDefault();
    boundaryTool.clear();
    return;
  }
  if (groundTool.active && event.button === 2) {
    event.preventDefault();
    groundTool.clear();
    return;
  }
  // The map is the outside surface for every workspace. Any deliberate world gesture clears the
  // overlay first; right-click harvesting and middle-button panning follow the same expectation as
  // an ordinary click rather than leaving a panel covering the action.
  closePanels();
  if (event.button === 2) {
    if (snapshot?.player.hand) {
      const dropHex = renderer.pick(event.clientX, event.clientY);
      enqueue({
        type: "drop_player_stack",
        q: dropHex.q,
        r: dropHex.r,
        quantity: 1,
      });
      event.preventDefault();
      return;
    }
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
  if (event.button !== 0 || !draggableTool() || snapshot?.player.hand) return;
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
  if (erasing) {
    void eraseLine(from, to);
    return;
  }
  enqueue({
    type: "place_line",
    q: from.q,
    r: from.r,
    to_q: to.q,
    to_r: to.r,
    definition_id: tool as number,
    orientation,
    recipe_id: recipeFor(tool),
  });
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
  if (boundaryTool.active) {
    boundaryTool.pick(
      coordinate,
      renderer.pickWorld(event.clientX, event.clientY),
    );
    return;
  }
  if (groundTool.active) {
    groundTool.pick(
      coordinate,
      renderer.pickWorld(event.clientX, event.clientY),
    );
    return;
  }
  if (snapshot?.player.hand) {
    const placed =
      event.ctrlKey || event.metaKey ? 1 : snapshot.player.hand.quantity;
    enqueue({
      type: "drop_player_stack",
      q: coordinate.q,
      r: coordinate.r,
      quantity: placed,
    });
    return;
  }
  // Read the old selection before it is replaced: the second click on a hex is the walk gesture, and
  // it is only free to mean that under `inspect`, where every other tool's second click already
  // means place, erase, rotate, or upgrade again.
  const repeat =
    tool === "inspect" &&
    selected !== null &&
    selected.q === coordinate.q &&
    selected.r === coordinate.r;
  selected = coordinate;
  renderer.setSelection(coordinate);
  if (repeat) enqueue({ type: "walk_to", ...coordinate });
  // Empty ground keeps native's answer: a sweep with the erase tool crosses far more nothing than
  // something, and a local complaint on every miss would be noise the old path never made.
  else if (tool === "erase") {
    if (buildingAt(coordinate)) eraseBuilding(coordinate);
    else enqueue({ type: "erase", ...coordinate });
  } else if (tool === "rotate") enqueue({ type: "rotate", ...coordinate });
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
      const definition =
        erasing || typeof tool !== "number"
          ? undefined
          : host.definitions.buildings.find(({ id }) => id === tool);
      required<HTMLElement>("placement-value").textContent = erasing
        ? `Remove ${legal} of ${cells.length}`
        : definition?.underpass_span !== undefined && legal === 2
          ? `Build paired portals · ${definition.underpass_span}-hex reach`
          : (cells.find((cell) => !cell.legal && cell.reason)?.reason ??
            `Build ${legal} of ${cells.length}`);
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
function rotateUnderCursorOrPending(reverse = false): void {
  if (typeof tool === "number" || tool === "inspect") {
    const target = hover ?? selected;
    const existing =
      typeof tool === "number" ? null : target && buildingAt(target);
    if (existing && target) {
      enqueue({ type: "rotate", q: target.q, r: target.r, reverse });
      return;
    }
  }
  rotateNewBuilding(reverse ? -1 : 1);
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
  switch (definition?.orientation_axis) {
    case "corner":
      return { start: NORTH, end: DIRECTION_NAMES.length };
    case "any":
      return { start: 0, end: DIRECTION_NAMES.length };
    default:
      return { start: 0, end: NORTH };
  }
}

/**
 * Whether the pending tool may actually be built at that heading yet.
 *
 * The axis says which headings exist and research says which are paid for; `place` asks both, so
 * rotation asks both. Only the corner gate can differ between headings — a building the player has
 * not unlocked at all is not in the dock to be turned.
 */
function orientationAllowed(tool: Tool, orientation: number): boolean {
  if (typeof tool !== "number") return true;
  const definition = host.definitions.buildings.find(({ id }) => id === tool);
  if (!definition || orientation < NORTH) return true;
  return (
    definition.corner_technology_id === undefined ||
    snapshot.researched.includes(definition.corner_technology_id)
  );
}

/**
 * One press of `R`, on the axis the pending tool builds on. A tool with one family walks its own
 * six; a tool with both walks all twelve in angular order, which `rotateAnyOrientation` owns and
 * `rotationMatchesNativeAngularOrder` pins against the shared direction fixture.
 *
 * A heading whose research is not paid for is stepped over rather than held, the way native's own
 * rotation steps over it: the two-row reach is a thing the player earns, so `R` walks the six edges
 * until then and all twelve afterwards. The card still names the locked heading's technology — that
 * is where the reach is advertised — but the key that turns a belt never stops on one.
 */
function rotateNewBuilding(step = 1): void {
  const { start, end } = orientationRange(tool);
  if (end - start === DIRECTION_NAMES.length) {
    let next = orientation;
    for (let press = 0; press < DIRECTION_NAMES.length; press += 1) {
      next = rotateAnyOrientation(next, step);
      if (orientationAllowed(tool, next)) break;
    }
    setOrientation(next);
    return;
  }
  // A tool with a single family stays inside it. `rotateHexDirection` still turns the six edges,
  // so the package keeps owning the geometry it knows.
  setOrientation(
    start === 0
      ? rotateHexDirection(orientation as HexDirection, step)
      : start + ((orientation - start + step + (end - start)) % (end - start)),
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
 * Open one workspace surface at a time. Pack, research, construction, mission control, creative,
 * timer, and menu are modes of attention rather than windows to arrange. Letting them all remain
 * open made each collapse to a few unreadable lines and left repeated clicks with surprising
 * results. The persistent inspector is not `open` on wide screens and therefore stays beside the
 * chosen workspace; the right rail hides it while its own menu or timer is open.
 */
function togglePanel(id: string): void {
  if (id === INVENTORY_PANEL && panels.isOpen(id)) packDeclined = true;
  boundaryTool.close(false);
  groundTool.close(false);
  panels.toggle(id);
  // The session rail carries the second copy of the world form. Its preview cannot raster while the
  // panel is closed, so opening one is the other moment a picture becomes drawable.
  requestWorldPreview();
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
  const budget = frameClock.update(now, {
    // Player time accrues only while the player has work. A standing walk goal is work: nobody is
    // holding a key while native steers, so without this the route would be planned, drawn, and
    // then never walked.
    playerActive:
      pressedMovement.size > 0 ||
      snapshot.player.action_cooldown > 0 ||
      snapshot.player.walk_goal !== null,
    playerTicksPerSecond: host.playerTicksPerSecond,
  });
  // The timer and simulation share the same real-time interval; neither has a player pause state.
  if (run) runElapsedMs += budget.elapsed;
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
    const { ticks, playerSteps } = budget;
    if (commands.length || ticks > 0 || playerSteps > 0) {
      frameClock.consume(ticks, playerSteps);
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
    !titleScreen.classList.contains("open") &&
    now - lastAutoSaveTime >= AUTOSAVE_INTERVAL_MS
  ) {
    lastAutoSaveTime = now;
    void triggerAutoSave();
  }
  renderer.setGathering(gatherHeld || harvestPointer !== null);
  // An orbit sweep slides the world under a stationary pointer, so the highlight is re-read until
  // the camera lands even when no simulation snapshot arrived during that frame.
  if (renderer.cameraSettling) syncHoverWithCamera();
  renderer.renderFrame(now);
  requestAnimationFrame(frame);
}

async function triggerAutoSave(silent = true): Promise<void> {
  if (autoSavePending || titleScreen.classList.contains("open")) return;
  autoSavePending = true;
  const tick = snapshot.tick;
  try {
    const payload = await host.save();
    const build = currentBuild();
    // The run's own name, not a shared "Auto-save" bucket: the player named this factory, and an
    // auto-save is that factory, so it lands in that factory's slot instead of a second one.
    const drafted = slotFromPayload(payload, runName, build, Date.now());
    if (!drafted) return;
    const { slots, error } = readCatalog(localStorage);
    if (error) return;
    const nextSlots = replaceNamedSlot(slots, drafted);
    writeCatalog(localStorage, nextSlots);
    lastAutoSaveTime = performance.now();
    markSaved(tick);
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

/** Everything the catalogue now holds is covered up to `tick`. */
function markSaved(tick: number): void {
  savedTick = tick;
  savedAt = Date.now();
}

/**
 * The last chance to keep a run, and the only prompt a page is allowed to ask for.
 *
 * The auto-save fired here rarely finishes: it is a worker round trip and a storage write, and the
 * tab is already leaving. So a close can drop up to a whole auto-save interval of factory. When that
 * much is at stake the browser's own leave prompt is raised — calling `preventDefault` is the entire
 * request, the wording belongs to the browser, and a page cannot say more than that. A player who
 * stays is told what to press, because the browser's dialog says nothing about saving.
 */
window.addEventListener("beforeunload", (event) => {
  if (titleScreen.classList.contains("open")) return;
  void triggerAutoSave();
  const atRisk = unsavedRunAtRisk({
    tick: snapshot.tick,
    savedTick,
    savedAt,
    now: Date.now(),
    graceMs: UNSAVED_CLOSE_GRACE_MS,
  });
  if (!atRisk) return;
  event.preventDefault();
  // Timers are frozen while the leave prompt is up, and the page is gone if the player goes through
  // with it, so this only ever reaches somebody who stayed.
  window.setTimeout(() => {
    showFeedback("Not saved yet — open the game menu and press Save.");
  }, 0);
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
  const titleStatus = required("title-save-status");
  titleStatus.textContent = message ?? error ?? "";
  titleStatus.hidden = !titleStatus.textContent;
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
  paintSaveSlotList(required("save-slots"), slots, build, "save-slot");
}

function renderTitleSaveSlots(slots: SaveSlot[], build: CurrentBuild): void {
  paintSaveSlotList(
    required("title-save-slots"),
    slots,
    build,
    "save-slot title-save-slot",
  );
}

function paintSaveSlotList(
  board: HTMLElement,
  slots: SaveSlot[],
  build: CurrentBuild,
  rowClass: string,
): void {
  const ordered = slotsNewestFirst(slots);
  const rows = syncChildren(
    board,
    ordered.map((slot) => slot.id),
    () => {
      const row = document.createElement("li");
      row.className = rowClass;
      row.innerHTML = `<button type="button" class="save-slot-select"><strong></strong><span class="save-slot-when"></span><span class="save-slot-config"></span><span class="save-slot-versions"></span><span class="save-slot-issue"></span></button><div class="save-slot-actions"><button type="button" class="save-slot-load">Load</button><button type="button" class="save-slot-export">Export</button><button type="button" class="save-slot-delete">Delete</button></div>`;
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
    const exported = part<HTMLButtonElement>(row, ".save-slot-export");
    exported.dataset.slotId = slot.id;
    exported.setAttribute("aria-label", `Export ${slot.name}`);
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
    // The catalogue already holds exactly this state, so the close guard starts from clean.
    markSaved(next.tick);
    selectedSaveId = slot.id;
    setRunName(slot.name);
    showFeedback(`Restored “${slot.name}”`);
    closePanels();
    closeTitleScreen();
    updateContinueState(`Restored “${slot.name}”.`);
  } catch (error) {
    updateContinueState(`Load rejected: ${String(error)}`);
  }
}

function handleSaveSlotClick(event: Event): void {
  const target = event.target as HTMLElement;
  const load = target.closest<HTMLButtonElement>(".save-slot-load");
  const exported = target.closest<HTMLButtonElement>(".save-slot-export");
  const remove = target.closest<HTMLButtonElement>(".save-slot-delete");
  const select = target.closest<HTMLButtonElement>(".save-slot-select");
  const id = (load ?? exported ?? remove ?? select)?.dataset.slotId;
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
  if (exported) {
    void exportSlotFile(slot);
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
}

required<HTMLElement>("save-slots").addEventListener(
  "click",
  handleSaveSlotClick,
);
required<HTMLElement>("title-save-slots").addEventListener(
  "click",
  handleSaveSlotClick,
);

function downloadTextFile(filename: string, text: string): void {
  const blob = new Blob([text], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.rel = "noopener";
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

interface SaveFilePickerWindow {
  showSaveFilePicker?: (options: {
    suggestedName?: string;
    types?: Array<{
      description?: string;
      accept: Record<string, string[]>;
    }>;
  }) => Promise<{
    createWritable: () => Promise<{
      write: (data: string) => Promise<void>;
      close: () => Promise<void>;
    }>;
  }>;
}

async function exportTextFile(
  filename: string,
  text: string,
  kind: "save" | "catalog",
): Promise<boolean> {
  const picker = (window as SaveFilePickerWindow).showSaveFilePicker;
  if (typeof picker === "function") {
    try {
      const handle = await picker({
        suggestedName: filename,
        types:
          kind === "catalog"
            ? [
                {
                  description: "HexFactory save list",
                  accept: { "application/json": [".json"] },
                },
              ]
            : [
                {
                  description: "HexFactory save",
                  accept: { "text/plain": [".hxf1"] },
                },
              ],
      });
      const writable = await handle.createWritable();
      await writable.write(text);
      await writable.close();
      return true;
    } catch (error) {
      if (error instanceof DOMException && error.name === "AbortError") {
        return false;
      }
    }
  }
  downloadTextFile(filename, text);
  return true;
}

async function exportSlotFile(slot: SaveSlot): Promise<void> {
  const wrote = await exportTextFile(
    saveFileName(slot.name),
    slot.payload,
    "save",
  );
  if (!wrote) return;
  updateContinueState(`Exported “${slot.name}”.`);
  showFeedback(`Exported “${slot.name}”`);
}

async function exportCurrentSave(): Promise<void> {
  try {
    const payload = await host.save();
    const build = currentBuild();
    const named =
      saveNameInput.value.trim() || runName || snapshot.scenario_name || "Save";
    const drafted = slotFromPayload(payload, named, build, Date.now());
    if (!drafted) {
      updateContinueState("Export failed: the envelope was not readable HXF1.");
      return;
    }
    await exportSlotFile(drafted);
  } catch (error) {
    updateContinueState(`Export failed: ${String(error)}`);
  }
}

async function exportAllSaves(): Promise<void> {
  const { slots, error } = readCatalog(localStorage);
  if (error) {
    updateContinueState(error);
    return;
  }
  if (slots.length === 0) {
    updateContinueState("No local save yet.");
    return;
  }
  const wrote = await exportTextFile(
    CATALOG_DOWNLOAD_NAME,
    catalogDocument(slots),
    "catalog",
  );
  if (!wrote) return;
  const noun = slots.length === 1 ? "save" : "saves";
  updateContinueState(`Exported ${slots.length} ${noun}.`);
  showFeedback(`Exported ${slots.length} ${noun}`);
}

function openSaveFilePicker(): void {
  saveFileInput.value = "";
  saveFileInput.click();
}

async function importSaveFiles(files: FileList | null): Promise<void> {
  if (!files || files.length === 0) return;
  const build = currentBuild();
  const read = readCatalog(localStorage);
  if (read.error) {
    updateContinueState(read.error);
    return;
  }
  let next = read.slots;
  const names: string[] = [];
  const problems: string[] = [];
  for (const file of files) {
    let text: string;
    try {
      text = await file.text();
    } catch (error) {
      problems.push(`${file.name}: ${String(error)}`);
      continue;
    }
    const imported = slotsFromFileText(text, build, { fileName: file.name });
    if (imported.error || imported.slots.length === 0) {
      problems.push(`${file.name}: ${imported.error ?? "no save found"}`);
      continue;
    }
    for (const slot of imported.slots) {
      const named = {
        ...slot,
        name: uniqueSlotName(slot.name, next),
      };
      next = [...next, named];
      names.push(named.name);
    }
  }
  if (names.length > 0) {
    try {
      writeCatalog(localStorage, next);
    } catch (error) {
      updateContinueState(
        `Could not keep the imported save in this browser: ${String(error)}. The file is still on disk.`,
      );
      return;
    }
  }
  const importedNote =
    names.length === 1
      ? `Imported “${names[0]}”.`
      : names.length > 1
        ? `Imported ${names.length} saves.`
        : "";
  const problemNote = problems.length > 0 ? problems.join(" ") : "";
  const message = [importedNote, problemNote].filter(Boolean).join(" ");
  updateContinueState(message || "Nothing was imported.");
  // The session status line is behind the title screen, so a toast is how an import
  // from Saved games reports success or a refused file.
  if (message) showFeedback(message);
}

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
    target instanceof HTMLAnchorElement ||
    (target instanceof HTMLElement && target.tagName === "SUMMARY")
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
  if (target instanceof Element && target.closest("dialog[open]")) return;
  if (!isPointerActivatedControl(target) || isTypingTarget(target)) return;
  if (target instanceof HTMLElement) target.blur();
});

function titleCase(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

function reportWorkerError(error: unknown): void {
  showFeedback(`Simulation worker error: ${String(error)}`);
}

/**
 * Clear the screen. This is the reset it always was — `Escape`, a new game, and a load all call it,
 * and all three should still leave nothing open. What changed in v0.20.1 is that opening a panel
 * stopped calling it.
 */
function closePanels(except?: HTMLElement): void {
  panels.close(except);
}

/*
 * A dropdown holds the keys while it is being used, because arrow keys and letters are how an
 * option is chosen. It hands them straight back once a choice is made, so picking a recipe never
 * leaves the player unable to walk.
 */
document.addEventListener("change", (event) => {
  if (
    event.target instanceof HTMLSelectElement &&
    !event.target.closest("dialog[open]")
  )
    event.target.blur();
});

// A close button closes the panel it is in and nothing else. Clearing the screen is Escape's job.
panels.bind();
// Capture before the panel controller changes the class, so only explicit close/toggle actions
// decline automatic pack opening. Selecting a different machine remains helpful.
document.addEventListener(
  "click",
  (event) => {
    const target = event.target instanceof Element ? event.target : null;
    const close = target?.closest("#inventory-panel .panel-close");
    const toggle = target?.closest('[data-panel-target="inventory-panel"]');
    if (close || (toggle && panels.isOpen(INVENTORY_PANEL)))
      packDeclined = true;
  },
  true,
);

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

renderTerrainLegend();
panels.restore();
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
