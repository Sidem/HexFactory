import {
  latestCompatible,
  readCatalog,
  unsavedRunAtRisk,
} from "../core/saveSlots";
import { formatRunReport, readRun, taintRun } from "../core/checkpoints";
import { renderTerrainLegend } from "../ui/terrainLegend";
import { isPointerActivatedControl, isTypingTarget } from "../input/focus";
import type { Runtime } from "./runtime";

export async function lifecycleWiring(app: Runtime): Promise<void> {
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden" && !app.worldSetup.isOpen()) {
      void app.triggerAutoSave();
    }
  });
  window.addEventListener("pagehide", () => {
    if (!app.worldSetup.isOpen()) {
      void app.triggerAutoSave();
    }
  });
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
    if (app.worldSetup.isOpen()) return;
    void app.triggerAutoSave();
    const atRisk = unsavedRunAtRisk({
      tick: app.snapshot.tick,
      savedTick: app.savedTick,
      savedAt: app.savedAt,
      now: Date.now(),
      graceMs: app.UNSAVED_CLOSE_GRACE_MS,
    });
    if (!atRisk) return;
    event.preventDefault();
    // Timers are frozen while the leave prompt is up, and the page is gone if the player goes through
    // with it, so this only ever reaches somebody who stayed.
    window.setTimeout(() => {
      app.showFeedback("Not saved yet — open the game menu and press Save.");
    }, 0);
  });
  app.saveUi.bind({
    load: (slot) => void app.loadSlot(slot),
    export: (slot) => void app.exportSlotFile(slot),
    refresh: (message) => app.updateContinueState(message),
  });
  window.addEventListener("pointerup", (event) => {
    // A clicked button keeps focus, and Space then activates it instead of recentring. Give the
    // keys back to the world once the pointer is done; a tabbed control still has :focus-visible.
    const target = event.target;
    if (target instanceof Element && target.closest("dialog[open]")) return;
    if (!isPointerActivatedControl(target) || isTypingTarget(target)) return;
    if (target instanceof HTMLElement) target.blur();
  });
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
  app.panels.bind();
  // Capture before the panel controller changes the class, so only explicit close/toggle actions
  // decline automatic pack opening. Selecting a different machine remains helpful.
  document.addEventListener(
    "click",
    (event) => {
      const target = event.target instanceof Element ? event.target : null;
      const close = target?.closest("#inventory-panel .panel-close");
      const toggle = target?.closest('[data-panel-target="inventory-panel"]');
      if (close || (toggle && app.panels.isOpen(app.INVENTORY_PANEL)))
        app.packDeclined = true;
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
      if (app.pressedMovement.has(code)) return;
      app.pressedMovement.add(code);
      app.enqueue(app.currentMovementIntent());
    };
    const stop = (event: PointerEvent): void => {
      event.preventDefault();
      if (!app.pressedMovement.delete(code)) return;
      app.enqueue(app.currentMovementIntent());
    };
    button.addEventListener("pointerdown", start);
    button.addEventListener("pointerup", stop);
    button.addEventListener("pointercancel", stop);
  }
  renderTerrainLegend();
  app.panels.restore();
  app.preferences.applyInitial(app.initialGraphics);
  // A reload is a discontinuity for the same reason a load is: the tab was gone for an unknown
  // stretch. The records survive so the ladder is not lost, and the run says why it cannot be raced.
  app.run = readRun(localStorage);
  if (app.run && app.run.records.length > 0)
    app.run = taintRun(app.run, "loaded-save");
  app.renderRun();
  app.update(app.snapshot);
  app.syncSessionInputs(app.snapshot);
  app.updateContinueState();
  app.initialCompatible = latestCompatible(
    readCatalog(localStorage).slots,
    app.currentBuild(),
  );
  app.worldSetup.showTab(app.initialCompatible ? "saves" : "new");
  app.selectTool("inspect");
  requestAnimationFrame(app.frame);
  // The moment the shell can answer: the worker is up, Wasm is instantiated, the first snapshot is
  // drawn, and the title screen takes input. `scripts/startup-budget.mjs` budgets the payload that
  // gets here; this mark is how a browser says how long that payload actually took.
  performance.mark("hexfactory:ready");
  window.__hexFactory = {
    snapshot: () => app.host.snapshot(),
    renderer: () => app.renderer.getDiagnostics(),
    orbit: (step) => app.orbitView(step),
    profile: (profile) => {
      if (profile) app.preferences.setGraphicsProfile(profile);
      return app.renderer.getGraphicsProfile();
    },
    pick: (x, y) => ({
      axial: app.renderer.pick(x, y),
      world: app.renderer.pickWorld(x, y),
    }),
    // The clock, readable. A scripted run needs the elapsed figure while it is still running, not
    // only the records that have already landed.
    run: () => ({
      timings: app.run,
      elapsedMs: app.runElapsedMs,
      report: app.run ? formatRunReport(app.run) : "",
    }),
    step: async (count = 1) => {
      const next = await app.host.tick(count);
      app.update(next);
      return next;
    },
    reset: async () => {
      const next = await app.host.reset();
      app.update(next);
      return next;
    },
    newGame: async (scenario = "new-game", seed) => {
      const next = await app.host.newGame(scenario, seed);
      // The scripted path starts a run too, so a timed opening can be driven from the console
      // without a human hand on the keyboard.
      app.beginRun(next);
      app.update(next);
      app.syncSessionInputs(next);
      return next;
    },
    save: () => app.host.save(),
    load: async (save) => {
      const next = await app.host.load(save);
      app.update(next);
      app.syncSessionInputs(next);
      return next;
    },
  };
}
