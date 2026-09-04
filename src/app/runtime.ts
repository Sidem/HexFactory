import type { BoundaryTool } from "../ui/boundaries";
import type { GroundTool } from "../ui/ground";
import type { FeedbackAudio } from "../audio/feedback";
import type { FactoryHost } from "../core/FactoryHost";
import type { FrameClock } from "../core/frameClock";
import type { SkillsView } from "../ui/skills";
import type { ResearchTree } from "../ui/researchTree";
import type { BoundedInputQueue } from "../core/input";
import { type SaveSlot } from "../core/saveSlots";
import { type RunTimings } from "../core/checkpoints";
import type {
  BuildingDefinition,
  FactorySnapshot,
  NativeInputCommand,
  PlacementPreview,
  ProjectState,
  StockKind,
  WorldPoint,
} from "../core/types";
import {
  type FactoryRenderer,
  type GraphicsProfile,
  type RendererDiagnostics,
} from "../rendering/FactoryRenderer";
import type { MinimapRenderer } from "../rendering/MinimapRenderer";
import type { PanelController } from "../ui/panels";
import type { ConfirmDialog } from "../ui/confirm";
import type { PreferencesController } from "./preferences";
import type { SaveUi } from "./saveUi";
import type { WorldSetup } from "./worldSetup";

export type Tool = "inspect" | "erase" | "rotate" | "upgrade" | number;

/**
 * How the catalogue is grouped, in the order a player meets these things.
 *
 * The dock used to be every buildable definition in id order, which by v0.14 was twenty buttons of
 * three-letter stamps — a list that grows every milestone and explains nothing. The grouping is
 * derived from `kind`, so a new definition lands in the right section by being what it is; nothing
 * here is a per-building special case.
 */
export type BuildGroupKey =
  | "extraction"
  | "transport"
  | "processing"
  | "storage"
  | "power";

export interface StockCompartment {
  stock: Exclude<StockKind, "auto">;
  label: string;
  accepts: boolean;
  expected: number[];
  entries: {
    item_id: number;
    quantity: number;
  }[];
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
export interface StackDrag {
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
        axial: {
          q: number;
          r: number;
        };
        world: WorldPoint;
      };
    };
  }
}

export class Runtime {
  NORTH!: 6;
  FOG_FILL!: "#18242f";
  FOG_STROKE!: "#7fe0c0";
  STATUS_TONE!: Record<string, "live" | "wait" | "stop" | "hub">;
  PANEL_KEYS!: Record<string, string>;
  SILENT_EVENTS!: Set<string>;
  canvas!: HTMLCanvasElement;
  toolShelf!: HTMLDivElement;
  feedback!: HTMLDivElement;
  creativeChip!: HTMLButtonElement;
  creativeSlotsInput!: HTMLInputElement;
  creativeClear!: HTMLButtonElement;
  creativeItems!: HTMLDivElement;
  titleContinue!: HTMLButtonElement;
  titleStartGame!: HTMLButtonElement;
  saveFileInput!: HTMLInputElement;
  sessionMainMenu!: HTMLButtonElement;
  input!: BoundedInputQueue;
  audio!: FeedbackAudio;
  host!: FactoryHost;
  worldSetup!: WorldSetup;
  storedGraphics!: GraphicsProfile | null;
  initialGraphics!: GraphicsProfile;
  renderer!: FactoryRenderer;
  preferences!: PreferencesController;
  boundaryTool!: BoundaryTool;
  groundTool!: GroundTool;
  minimap!: MinimapRenderer;
  researchDialog!: HTMLDialogElement;
  skillsDialog!: HTMLDialogElement;
  skillsView!: SkillsView;
  confirmDialog!: ConfirmDialog;
  panels!: PanelController;
  researchTree!: ResearchTree;
  snapshot!: FactorySnapshot;
  saveUi!: SaveUi;
  runName!: string;
  tool!: Tool;
  orientation!: number;
  selected!: {
    q: number;
    r: number;
  } | null;
  standingHex!: string | null;
  besideBuilding!: string | null;
  hover!: {
    q: number;
    r: number;
  } | null;
  hoverPreview!: PlacementPreview | null;
  frameClock!: FrameClock;
  feedbackTimer!: number;
  lastEvent!: string;
  autoSavePending!: boolean;
  lastAutoSaveTime!: number;
  AUTOSAVE_INTERVAL_MS!: 60000;
  savedTick!: number;
  savedAt!: number;
  UNSAVED_CLOSE_GRACE_MS!: 30000;
  run!: RunTimings | null;
  runElapsedMs!: number;
  panPointer!: {
    id: number;
    x: number;
    y: number;
    moved: boolean;
    mode: "look" | "pan";
  } | null;
  harvestPointer!: {
    id: number;
    q: number;
    r: number;
  } | null;
  suppressMapClick!: boolean;
  dragBuild!: {
    id: number;
    from: {
      q: number;
      r: number;
    };
    to: {
      q: number;
      r: number;
    };
    erasing: boolean;
  } | null;
  dragPreviewPending!: boolean;
  gatherHeld!: boolean;
  selectedRecipes!: Map<number, number>;
  inspectorRecipeKey!: string;
  pressedMovement!: Set<string>;
  runningHeld!: boolean;
  aimPointer!: {
    x: number;
    y: number;
  } | null;
  aimDegrees!: number | null;
  landingHub!: WorldPoint | null;
  landingHubWorld!: string;
  advancePending!: boolean;
  previewPending!: boolean;
  previewRequested!: boolean;
  previewRevision!: number;
  BUILD_GROUP_BY_KIND!: {
    extractor: "extraction";
    belt: "transport";
    composer: "processing";
    container: "storage";
    consumer: null;
    hub: null;
    pump: "extraction";
    pole: "power";
    generator: "power";
    boiler: "power";
    bridge: "transport";
  };
  BUILD_GROUPS!: {
    key: BuildGroupKey;
    title: string;
    blurb: string;
    holds: (definition: BuildingDefinition) => boolean;
  }[];
  HOTBAR_SLOTS!: 9;
  HOTBAR_KEY!: "hexfactory:hotbar:v1";
  DEFAULT_HOTBAR!: (Tool | null)[];
  hotbar!: (Tool | null)[];
  showAllBuildings!: boolean;
  buildSearch!: string;
  recipeSearch!: string;
  hotbarDragOver!: number | null;
  holdersChip!: WeakMap<HTMLElement, HTMLElement>;
  CREATIVE_FILL!: 4294967295;
  TOOL_LABELS!: Record<
    string,
    {
      icon: string;
      name: string;
    }
  >;
  DISCLOSURE_REACH!: 2;
  HAND_REACHABLE!: Set<string>;
  selectedOutputProduct!: Map<number, number>;
  INVENTORY_PANEL!: "inventory-panel";
  packOfferedFor!: string | null;
  packDeclined!: boolean;
  SWITCHABLE!: Set<string>;
  PROJECT_LABEL!: Record<ProjectState, string>;
  buildGroups!: HTMLDivElement;
  STACK_DRAG_LIFT!: 6;
  stackDrag!: StackDrag | null;
  stackDragHandledClick!: boolean;
  cursorStackX!: number;
  cursorStackY!: number;
  SPILL_NOTE!: "What fits goes back to your pack. Anything that does not fit falls at the site, and ground items disappear after about a minute of simulation time.";
  CANCEL_NOTE!: "The progress so far is lost. The ingredients go back to the machine's own ingredient slot, and its fuel and finished goods are left alone.";
  initialCompatible!: SaveSlot | undefined;

  constructor() {
    this.currentBuild = this.currentBuild.bind(this);
    this.dragOwnsPointer = this.dragOwnsPointer.bind(this);
    this.loadHotbar = this.loadHotbar.bind(this);
    this.sanitiseSlot = this.sanitiseSlot.bind(this);
    this.saveHotbar = this.saveHotbar.bind(this);
    this.checkpointContext = this.checkpointContext.bind(this);
    this.evaluateRun = this.evaluateRun.bind(this);
    this.beginRun = this.beginRun.bind(this);
    this.renderRun = this.renderRun.bind(this);
    this.update = this.update.bind(this);
    this.refreshLandingHub = this.refreshLandingHub.bind(this);
    this.syncStandingSelection = this.syncStandingSelection.bind(this);
    this.renderHomeReadout = this.renderHomeReadout.bind(this);
    this.sameCarry = this.sameCarry.bind(this);
    this.itemById = this.itemById.bind(this);
    this.paintChip = this.paintChip.bind(this);
    this.renderInventory = this.renderInventory.bind(this);
    this.renderCreative = this.renderCreative.bind(this);
    this.renderHotbarSlots = this.renderHotbarSlots.bind(this);
    this.assignHotbarSlot = this.assignHotbarSlot.bind(this);
    this.pinToHotbar = this.pinToHotbar.bind(this);
    this.renderHotbar = this.renderHotbar.bind(this);
    this.catalogueVisible = this.catalogueVisible.bind(this);
    this.buildMatches = this.buildMatches.bind(this);
    this.renderBuildScope = this.renderBuildScope.bind(this);
    this.renderBuildPanel = this.renderBuildPanel.bind(this);
    this.createBuildCard = this.createBuildCard.bind(this);
    this.heldOrientationFor = this.heldOrientationFor.bind(this);
    this.paintBuildingEmblem = this.paintBuildingEmblem.bind(this);
    this.fillBuildCard = this.fillBuildCard.bind(this);
    this.renderIngredientRow = this.renderIngredientRow.bind(this);
    this.fillIngredients = this.fillIngredients.bind(this);
    this.renderCardRecipes = this.renderCardRecipes.bind(this);
    this.describeRecipe = this.describeRecipe.bind(this);
    this.machinesForRecipe = this.machinesForRecipe.bind(this);
    this.itemsMatching = this.itemsMatching.bind(this);
    this.recipeMatches = this.recipeMatches.bind(this);
    this.renderRecipePanel = this.renderRecipePanel.bind(this);
    this.renderRecipeGroup = this.renderRecipeGroup.bind(this);
    this.createLookupRow = this.createLookupRow.bind(this);
    this.fillLookupRow = this.fillLookupRow.bind(this);
    this.technologyReach = this.technologyReach.bind(this);
    this.renderTechnologies = this.renderTechnologies.bind(this);
    this.stockCompartments = this.stockCompartments.bind(this);
    this.renderInspectorActions = this.renderInspectorActions.bind(this);
    this.renderInspectorLoad = this.renderInspectorLoad.bind(this);
    this.renderOutputRouting = this.renderOutputRouting.bind(this);
    this.panelsFitAbreast = this.panelsFitAbreast.bind(this);
    this.offerPackBeside = this.offerPackBeside.bind(this);
    this.renderInspector = this.renderInspector.bind(this);
    this.renderInspectorHub = this.renderInspectorHub.bind(this);
    this.renderInspectorSwitch = this.renderInspectorSwitch.bind(this);
    this.renderInspectorTier = this.renderInspectorTier.bind(this);
    this.costSummary = this.costSummary.bind(this);
    this.recipeChoices = this.recipeChoices.bind(this);
    this.fillRecipeOptions = this.fillRecipeOptions.bind(this);
    this.renderInspectorRecipe = this.renderInspectorRecipe.bind(this);
    this.renderRecipePicker = this.renderRecipePicker.bind(this);
    this.renderContract = this.renderContract.bind(this);
    this.renderRequests = this.renderRequests.bind(this);
    this.renderProjectCatalogue = this.renderProjectCatalogue.bind(this);
    this.renderNextAction = this.renderNextAction.bind(this);
    this.showFeedback = this.showFeedback.bind(this);
    this.syncSessionInputs = this.syncSessionInputs.bind(this);
    this.selectTool = this.selectTool.bind(this);
    this.enqueue = this.enqueue.bind(this);
    this.refreshHoverPreview = this.refreshHoverPreview.bind(this);
    this.syncHoverWithCamera = this.syncHoverWithCamera.bind(this);
    this.flushHoverPreview = this.flushHoverPreview.bind(this);
    this.setRunName = this.setRunName.bind(this);
    this.offerSaveFile = this.offerSaveFile.bind(this);
    this.stackGesture = this.stackGesture.bind(this);
    this.stackSlots = this.stackSlots.bind(this);
    this.stackDropRefusal = this.stackDropRefusal.bind(this);
    this.paintStackDropTargets = this.paintStackDropTargets.bind(this);
    this.endStackDrag = this.endStackDrag.bind(this);
    this.currentMovementIntent = this.currentMovementIntent.bind(this);
    this.orbitView = this.orbitView.bind(this);
    this.tiltView = this.tiltView.bind(this);
    this.eraseBuilding = this.eraseBuilding.bind(this);
    this.heldStock = this.heldStock.bind(this);
    this.cancelCraft = this.cancelCraft.bind(this);
    this.eraseLine = this.eraseLine.bind(this);
    this.deleteBuildingUnderCursorOrSelected =
      this.deleteBuildingUnderCursorOrSelected.bind(this);
    this.draggableTool = this.draggableTool.bind(this);
    this.recipeFor = this.recipeFor.bind(this);
    this.refreshDragPreview = this.refreshDragPreview.bind(this);
    this.endDrag = this.endDrag.bind(this);
    this.rotateUnderCursorOrPending =
      this.rotateUnderCursorOrPending.bind(this);
    this.pickToolUnderCursor = this.pickToolUnderCursor.bind(this);
    this.buildingAt = this.buildingAt.bind(this);
    this.setOrientation = this.setOrientation.bind(this);
    this.orientationRange = this.orientationRange.bind(this);
    this.orientationAllowed = this.orientationAllowed.bind(this);
    this.rotateNewBuilding = this.rotateNewBuilding.bind(this);
    this.stopAiming = this.stopAiming.bind(this);
    this.sendAim = this.sendAim.bind(this);
    this.togglePanel = this.togglePanel.bind(this);
    this.frame = this.frame.bind(this);
    this.triggerAutoSave = this.triggerAutoSave.bind(this);
    this.markSaved = this.markSaved.bind(this);
    this.updateContinueState = this.updateContinueState.bind(this);
    this.loadSlot = this.loadSlot.bind(this);
    this.exportSlotFile = this.exportSlotFile.bind(this);
    this.exportCurrentSave = this.exportCurrentSave.bind(this);
    this.exportAllSaves = this.exportAllSaves.bind(this);
    this.openSaveFilePicker = this.openSaveFilePicker.bind(this);
    this.importSaveFiles = this.importSaveFiles.bind(this);
    this.titleCase = this.titleCase.bind(this);
    this.reportWorkerError = this.reportWorkerError.bind(this);
    this.closePanels = this.closePanels.bind(this);
  }
}
