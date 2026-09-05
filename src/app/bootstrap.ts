import { BoundaryTool } from "../ui/boundaries";
import { GroundTool } from "../ui/ground";
import { FeedbackAudio } from "../audio/feedback";
import { FactoryHost } from "../core/FactoryHost";
import { FrameClock, SIMULATION_TICKS_PER_SECOND } from "../core/frameClock";
import { SkillsView } from "../ui/skills";
import { ResearchTree } from "../ui/researchTree";
import { BoundedInputQueue } from "../core/input";
import { AUTOSAVE_SLOT_NAME } from "../core/saveSlots";
import { CORNER_START } from "../core/directions";
import type { BuildingKind } from "../core/types";
import { MinimapRenderer } from "../rendering/MinimapRenderer";
import { ThreeFactoryRenderer } from "../rendering/three/ThreeFactoryRenderer";
import {
  defaultGraphicsProfile,
  GRAPHICS_STORAGE_KEY,
  parseGraphicsProfile,
} from "../rendering/three/quality";
import { required } from "../ui/dom";
import { PanelController } from "../ui/panels";
import { ConfirmDialog } from "../ui/confirm";
import { PreferencesController } from "./preferences";
import { SaveUi } from "./saveUi";
import { WorldSetup } from "./worldSetup";
import type { BuildGroupKey } from "./runtime";
import type { Runtime } from "./runtime";

export async function bootstrap(app: Runtime): Promise<void> {
  app.NORTH = CORNER_START;
  app.FOG_FILL = "#18242f";
  app.FOG_STROKE = "#7fe0c0";
  app.STATUS_TONE = {
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
  app.PANEL_KEYS = {
    KeyI: "inventory-panel",
    KeyO: "research-panel",
    KeyK: "skills-panel",
    KeyP: "quest-panel",
    KeyB: "build-panel",
    KeyL: "recipe-panel",
    KeyC: "creative-panel",
  };
  app.SILENT_EVENTS = new Set(["action cooling down"]);
  app.canvas = required<HTMLCanvasElement>("factory-canvas");
  required<HTMLElement>("simulation-rate").textContent =
    `Simulation: ${SIMULATION_TICKS_PER_SECOND} ticks per second`;
  app.toolShelf = required<HTMLDivElement>("tool-shelf");
  app.feedback = required<HTMLDivElement>("feedback");
  app.creativeChip = required<HTMLButtonElement>("creative-chip");
  app.creativeSlotsInput = required<HTMLInputElement>("creative-slots");
  app.creativeClear = required<HTMLButtonElement>("creative-clear");
  app.creativeItems = required<HTMLDivElement>("creative-items");
  app.titleContinue = required<HTMLButtonElement>("title-continue");
  app.titleStartGame = required<HTMLButtonElement>("title-start-game");
  app.saveFileInput = required<HTMLInputElement>("save-file-input");
  app.sessionMainMenu = required<HTMLButtonElement>("session-main-menu");
  app.input = new BoundedInputQueue();
  app.audio = new FeedbackAudio();
  app.host = await FactoryHost.create();
  app.worldSetup = new WorldSetup(app.host, app.canvas);
  app.storedGraphics = parseGraphicsProfile(
    localStorage.getItem(GRAPHICS_STORAGE_KEY),
  );
  app.initialGraphics = app.storedGraphics ?? defaultGraphicsProfile();
  app.renderer = new ThreeFactoryRenderer(
    app.canvas,
    app.host.definitions,
    app.initialGraphics,
  );
  app.preferences = new PreferencesController(app.audio, app.renderer);
  app.boundaryTool = new BoundaryTool(
    required<HTMLElement>("boundary-panel"),
    app.host,
    app.renderer,
    app.enqueue,
    () => {
      app.selectTool("inspect");
      app.closePanels();
      app.groundTool.close(false);
    },
  );
  app.groundTool = new GroundTool(
    required<HTMLElement>("ground-panel"),
    app.host,
    app.renderer,
    app.enqueue,
    () => {
      app.selectTool("inspect");
      app.closePanels();
      app.boundaryTool.close(false);
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
      app.canvas.dispatchEvent(new Event("hexfactory:test-context-cycle")),
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
      diagnosticsOutput.textContent = JSON.stringify(
        app.renderer.getDiagnostics(),
      );
    });
    document.body.append(captureDiagnostics, diagnosticsOutput);
  }
  app.minimap = new MinimapRenderer(
    required<HTMLCanvasElement>("minimap"),
    app.host.definitions,
  );
  app.researchDialog = required<HTMLDialogElement>("research-panel");
  app.skillsDialog = required<HTMLDialogElement>("skills-panel");
  app.skillsView = new SkillsView(
    app.skillsDialog,
    app.host.technologies,
    (id) => app.enqueue({ type: "purchase_skill", skill_id: id }),
  );
  app.confirmDialog = new ConfirmDialog(
    required<HTMLDialogElement>("confirm-dialog"),
    () => {
      app.gatherHeld = false;
      app.harvestPointer = null;
      app.runningHeld = false;
      app.pressedMovement.clear();
      app.stopAiming();
      app.endStackDrag();
      app.enqueue(app.currentMovementIntent());
    },
  );
  app.panels = new PanelController(document, localStorage, (id, open) => {
    if ((id !== "research-panel" && id !== "skills-panel") || !open) return;
    app.gatherHeld = false;
    app.harvestPointer = null;
    app.runningHeld = false;
    app.stopAiming();
    app.pressedMovement.clear();
    app.enqueue(app.currentMovementIntent());
    if (id === "research-panel") {
      app.researchTree.onOpen();
      const currentRun = app.snapshot;
      void app.host
        .worldParams()
        .then((params) => {
          // A load/reset may finish while the worker is replying. Never show another run's notice.
          if (
            currentRun.scenario !== app.snapshot.scenario ||
            currentRun.seed !== app.snapshot.seed ||
            app.snapshot.tick < currentRun.tick
          )
            return;
          const oil = app.host.definitions.items.find(
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
    } else app.skillsView.update(app.snapshot);
  });
  app.researchTree = new ResearchTree(
    app.researchDialog,
    app.host.technologies,
    app.host.definitions,
    (id) => app.enqueue({ type: "research", technology_id: id }),
  );
  app.snapshot = app.host.snapshot();
  app.saveUi = new SaveUi({
    load: (slot) => void app.loadSlot(slot),
    export: (slot) => void app.exportSlotFile(slot),
    refresh: (message) => app.updateContinueState(message),
  });
  app.runName = AUTOSAVE_SLOT_NAME;
  app.tool = "inspect";
  app.orientation = 0;
  app.selected = null;
  app.standingHex = null;
  app.besideBuilding = null;
  app.hover = null;
  app.hoverPreview = null;
  app.frameClock = new FrameClock(performance.now());
  app.feedbackTimer = 0;
  app.lastEvent = "";
  app.autoSavePending = false;
  app.lastAutoSaveTime = performance.now();
  app.AUTOSAVE_INTERVAL_MS = 60000;
  app.savedTick = 0;
  app.savedAt = Date.now();
  app.UNSAVED_CLOSE_GRACE_MS = 30000;
  app.run = null;
  app.runElapsedMs = 0;
  app.panPointer = null;
  app.harvestPointer = null;
  app.suppressMapClick = false;
  app.dragBuild = null;
  app.dragPreviewPending = false;
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
    if (!app.dragOwnsPointer()) return;
    event.preventDefault();
    getSelection()?.removeAllRanges();
  });
  app.gatherHeld = false;
  app.selectedRecipes = new Map<number, number>();
  app.inspectorRecipeKey = "";
  app.pressedMovement = new Set<string>();
  app.runningHeld = false;
  app.aimPointer = null;
  app.aimDegrees = null;
  app.landingHub = null;
  app.landingHubWorld = "";
  app.advancePending = false;
  app.previewPending = false;
  app.previewRequested = false;
  app.previewRevision = 0;
  app.BUILD_GROUP_BY_KIND = {
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
  app.BUILD_GROUPS = [
    {
      key: "extraction",
      title: "Extraction",
      blurb: "Take raw material out of the ground and the water.",
      holds: ({ kind }) => app.BUILD_GROUP_BY_KIND[kind] === "extraction",
    },
    {
      key: "transport",
      title: "Transport",
      blurb:
        "Drag from source to destination. Belts carry solids and sealed barrels; pipes carry loose water and crude. Paired underpasses cross an occupied lane without mixing it.",
      holds: ({ kind }) => app.BUILD_GROUP_BY_KIND[kind] === "transport",
    },
    {
      key: "processing",
      title: "Processing",
      blurb:
        "Turn one material into another. Each station lists the recipes it supports.",
      holds: ({ kind }) => app.BUILD_GROUP_BY_KIND[kind] === "processing",
    },
    {
      key: "storage",
      title: "Storage",
      blurb: "Buffer a line, and hold stock you can take back by hand.",
      holds: ({ kind }) => app.BUILD_GROUP_BY_KIND[kind] === "storage",
    },
    {
      key: "power",
      title: "Power",
      blurb:
        "Make electricity and carry it. Machines draw; belts and boxes do not.",
      holds: ({ kind }) => app.BUILD_GROUP_BY_KIND[kind] === "power",
    },
  ];
  app.HOTBAR_SLOTS = 9;
  app.HOTBAR_KEY = "hexfactory:hotbar:v1";
  app.DEFAULT_HOTBAR = [28, 27, 2, 4, 1, 3, 12, 13, 8];
  app.hotbar = app.loadHotbar();
  app.showAllBuildings = false;
  app.buildSearch = "";
  app.recipeSearch = "";
  app.hotbarDragOver = null;
  app.holdersChip = new WeakMap<HTMLElement, HTMLElement>();
  app.CREATIVE_FILL = 4294967295;
  app.TOOL_LABELS = {
    erase: { icon: "⌫", name: "Erase" },
    rotate: { icon: "↻", name: "Edit" },
    upgrade: { icon: "▲", name: "Upgrade" },
    inspect: { icon: "⌖", name: "Inspect" },
  };
  app.DISCLOSURE_REACH = 2;
  app.HAND_REACHABLE = new Set<string>([
    "extractor",
    "pump",
    "container",
    "composer",
    "generator",
    "boiler",
  ]);
  app.selectedOutputProduct = new Map<number, number>();
  required<HTMLElement>("inspect-output-products").addEventListener(
    "click",
    (event) => {
      const button = (event.target as Element).closest<HTMLElement>(
        "[data-item-id]",
      );
      const building = app.selected ? app.buildingAt(app.selected) : undefined;
      const itemId = Number(button?.dataset.itemId);
      if (!building || !Number.isInteger(itemId)) return;
      app.selectedOutputProduct.set(building.id, itemId);
      app.renderOutputRouting(building);
    },
  );
  required<HTMLElement>("inspect-output-ports").addEventListener(
    "click",
    (event) => {
      const button = (event.target as Element).closest<HTMLElement>(
        ".inspect-output-port",
      );
      const building = app.selected ? app.buildingAt(app.selected) : undefined;
      if (!button || !building) return;
      const itemId = app.selectedOutputProduct.get(building.id);
      if (!itemId) return;
      app.enqueue({
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
  app.INVENTORY_PANEL = "inventory-panel";
  app.packOfferedFor = null;
  app.packDeclined = false;
  app.SWITCHABLE = new Set<string>([
    "extractor",
    "pump",
    "composer",
    "generator",
    "boiler",
  ]);
  app.PROJECT_LABEL = {
    posted: "On the board",
    available: "Ready to post",
    complete: "Done",
    locked: "Not yet makeable",
  };
}
