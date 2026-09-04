import { exportTextFile } from "../core/fileExport";
import {
  CATALOG_DOWNLOAD_NAME,
  catalogDocument,
  readCatalog,
  replaceNamedSlot,
  saveFileName,
  slotFromPayload,
  slotsFromFileText,
  uniqueSlotName,
  writeCatalog,
  type SaveSlot,
} from "../core/saveSlots";
import { taintRun, writeRun } from "../core/checkpoints";
import { Runtime } from "./runtime";

declare module "./runtime" {
  interface Runtime {
    togglePanel(id: string): void;
    frame(now: number): void;
    triggerAutoSave(silent?: boolean): Promise<void>;
    markSaved(tick: number): void;
    updateContinueState(message?: string): void;
    loadSlot(slot: SaveSlot): Promise<void>;
    exportSlotFile(slot: SaveSlot): Promise<void>;
    exportCurrentSave(): Promise<void>;
    exportAllSaves(): Promise<void>;
    openSaveFilePicker(): void;
    importSaveFiles(files: FileList | null): Promise<void>;
    titleCase(value: string): string;
    reportWorkerError(error: unknown): void;
    closePanels(except?: HTMLElement): void;
  }
}

Runtime.prototype.togglePanel = function togglePanel(
  this: Runtime,
  id: string,
): void {
  if (id === "creative-panel" && !this.snapshot.player.creative) return;
  if (id === this.INVENTORY_PANEL && this.panels.isOpen(id))
    this.packDeclined = true;
  this.boundaryTool.close(false);
  this.groundTool.close(false);
  this.panels.toggle(id);
  // The session rail carries the second copy of the world form. Its preview cannot raster while the
  // panel is closed, so opening one is the other moment a picture becomes drawable.
  this.worldSetup.requestPreview();
};

Runtime.prototype.frame = function frame(this: Runtime, now: number): void {
  const budget = this.frameClock.update(now, {
    // Player time accrues only while the player has work. A standing walk goal is work: nobody is
    // holding a key while native steers, so without this the route would be planned, drawn, and
    // then never walked.
    playerActive:
      this.pressedMovement.size > 0 ||
      this.snapshot.player.action_cooldown > 0 ||
      this.snapshot.player.walk_goal !== null,
    playerTicksPerSecond: this.host.playerTicksPerSecond,
  });
  // The timer and simulation share the same real-time interval; neither has a player pause state.
  if (this.run) this.runElapsedMs += budget.elapsed;
  if (!this.advancePending) {
    // A held gather repeats at frame rate and is paced natively by the swing already running, so
    // the player holds the key instead of tapping it once per unit. A held right-click is the same
    // idea aimed at a named hex, and it outranks the untargeted one: if both are held, the hex the
    // player is pointing at is the one they chose.
    if (!this.input.size) {
      if (this.harvestPointer)
        this.input.enqueue({
          type: "gather_at",
          q: this.harvestPointer.q,
          r: this.harvestPointer.r,
        });
      else if (this.gatherHeld) this.input.enqueue({ type: "gather" });
    }
    // Last into the batch, so the cursor outranks the walk direction for this frame's facing.
    this.sendAim();
    const commands = this.input.drain();
    const { ticks, playerSteps } = budget;
    if (commands.length || ticks > 0 || playerSteps > 0) {
      this.frameClock.consume(ticks, playerSteps);
      this.advancePending = true;
      void this.host
        .advance(commands, ticks, playerSteps)
        .then(this.update)
        .catch(this.reportWorkerError)
        .finally(() => {
          this.advancePending = false;
        });
    }
  }
  if (
    !this.worldSetup.isOpen() &&
    now - this.lastAutoSaveTime >= this.AUTOSAVE_INTERVAL_MS
  ) {
    this.lastAutoSaveTime = now;
    void this.triggerAutoSave();
  }
  this.renderer.setGathering(this.gatherHeld || this.harvestPointer !== null);
  // An orbit sweep slides the world under a stationary pointer, so the highlight is re-read until
  // the camera lands even when no simulation snapshot arrived during that frame.
  if (this.renderer.cameraSettling) this.syncHoverWithCamera();
  this.renderer.renderFrame(now);
  requestAnimationFrame(this.frame);
};

Runtime.prototype.triggerAutoSave = async function triggerAutoSave(
  this: Runtime,
  silent = true,
): Promise<void> {
  if (this.autoSavePending || this.worldSetup.isOpen()) return;
  this.autoSavePending = true;
  const tick = this.snapshot.tick;
  try {
    const payload = await this.host.save();
    const build = this.currentBuild();
    // The run's own name, not a shared "Auto-save" bucket: the player named this factory, and an
    // auto-save is that factory, so it lands in that factory's slot instead of a second one.
    const drafted = slotFromPayload(payload, this.runName, build, Date.now());
    if (!drafted) return;
    const { slots, error } = readCatalog(localStorage);
    if (error) return;
    const nextSlots = replaceNamedSlot(slots, drafted);
    writeCatalog(localStorage, nextSlots);
    this.lastAutoSaveTime = performance.now();
    this.markSaved(tick);
    this.updateContinueState();
    if (!silent) this.showFeedback("Factory auto-saved");
  } catch {
    // Non-fatal if auto-save fails (e.g. quota or blocked storage)
  } finally {
    this.autoSavePending = false;
  }
};

Runtime.prototype.markSaved = function markSaved(
  this: Runtime,
  tick: number,
): void {
  this.savedTick = tick;
  this.savedAt = Date.now();
};

Runtime.prototype.updateContinueState = function updateContinueState(
  this: Runtime,
  message?: string,
): void {
  const scenarioVersion =
    this.host.scenarios.scenarios.find(
      (scenario) => scenario.key === this.snapshot.scenario,
    )?.version ?? 0;
  this.saveUi.update(this.currentBuild(), scenarioVersion, message);
};

Runtime.prototype.loadSlot = async function loadSlot(
  this: Runtime,
  slot: SaveSlot,
): Promise<void> {
  try {
    this.input.clear();
    const next = await this.host.load(slot.payload);
    // A load is a discontinuity the clock cannot see across: whatever it counted belongs to a
    // different sitting. The run stays, so checkpoints keep landing, but it is marked uncomparable
    // rather than quietly presented as a clean time.
    if (!this.run) this.beginRun(next);
    if (this.run) {
      this.run = taintRun(this.run, "loaded-save");
      writeRun(localStorage, this.run);
      this.renderRun();
    }
    this.update(next);
    this.syncSessionInputs(next);
    this.renderer.recenter();
    // The catalogue already holds exactly this state, so the close guard starts from clean.
    this.markSaved(next.tick);
    this.saveUi.select(slot);
    this.setRunName(slot.name);
    this.showFeedback(`Restored “${slot.name}”`);
    this.closePanels();
    this.worldSetup.close();
    this.updateContinueState(`Restored “${slot.name}”.`);
  } catch (error) {
    this.updateContinueState(`Load rejected: ${String(error)}`);
  }
};

Runtime.prototype.exportSlotFile = async function exportSlotFile(
  this: Runtime,
  slot: SaveSlot,
): Promise<void> {
  const wrote = await exportTextFile(
    saveFileName(slot.name),
    slot.payload,
    "save",
  );
  if (!wrote) return;
  this.updateContinueState(`Exported “${slot.name}”.`);
  this.showFeedback(`Exported “${slot.name}”`);
};

Runtime.prototype.exportCurrentSave = async function exportCurrentSave(
  this: Runtime,
): Promise<void> {
  try {
    const payload = await this.host.save();
    const build = this.currentBuild();
    const named =
      this.saveUi.name || this.runName || this.snapshot.scenario_name || "Save";
    const drafted = slotFromPayload(payload, named, build, Date.now());
    if (!drafted) {
      this.updateContinueState(
        "Export failed: the envelope was not readable HXF1.",
      );
      return;
    }
    await this.exportSlotFile(drafted);
  } catch (error) {
    this.updateContinueState(`Export failed: ${String(error)}`);
  }
};

Runtime.prototype.exportAllSaves = async function exportAllSaves(
  this: Runtime,
): Promise<void> {
  const { slots, error } = readCatalog(localStorage);
  if (error) {
    this.updateContinueState(error);
    return;
  }
  if (slots.length === 0) {
    this.updateContinueState("No local save yet.");
    return;
  }
  const wrote = await exportTextFile(
    CATALOG_DOWNLOAD_NAME,
    catalogDocument(slots),
    "catalog",
  );
  if (!wrote) return;
  const noun = slots.length === 1 ? "save" : "saves";
  this.updateContinueState(`Exported ${slots.length} ${noun}.`);
  this.showFeedback(`Exported ${slots.length} ${noun}`);
};

Runtime.prototype.openSaveFilePicker = function openSaveFilePicker(
  this: Runtime,
): void {
  this.saveFileInput.value = "";
  this.saveFileInput.click();
};

Runtime.prototype.importSaveFiles = async function importSaveFiles(
  this: Runtime,
  files: FileList | null,
): Promise<void> {
  if (!files || files.length === 0) return;
  const build = this.currentBuild();
  const read = readCatalog(localStorage);
  if (read.error) {
    this.updateContinueState(read.error);
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
      this.updateContinueState(
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
  this.updateContinueState(message || "Nothing was imported.");
  // The session status line is behind the title screen, so a toast is how an import
  // from Saved games reports success or a refused file.
  if (message) this.showFeedback(message);
};

Runtime.prototype.titleCase = function titleCase(
  this: Runtime,
  value: string,
): string {
  return value.charAt(0).toUpperCase() + value.slice(1);
};

Runtime.prototype.reportWorkerError = function reportWorkerError(
  this: Runtime,
  error: unknown,
): void {
  this.showFeedback(`Simulation worker error: ${String(error)}`);
};

Runtime.prototype.closePanels = function closePanels(
  this: Runtime,
  except?: HTMLElement,
): void {
  this.panels.close(except);
};
