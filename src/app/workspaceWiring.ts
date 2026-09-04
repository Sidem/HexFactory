import { buildingAvailability } from "../core/availability";
import {
  AUTOSAVE_SLOT_NAME,
  latestCompatible,
  readCatalog,
  replaceNamedSlot,
  slotFromPayload,
  uniqueSlotName,
  upsertSlot,
  writeCatalog,
} from "../core/saveSlots";
import { formatRunReport, startRun, writeRun } from "../core/checkpoints";
import { required } from "../ui/dom";
import type { Tool } from "./runtime";
import type { Runtime } from "./runtime";

export async function workspaceWiring(app: Runtime): Promise<void> {
  required<HTMLButtonElement>("build-scope").addEventListener("click", () => {
    app.showAllBuildings = !app.showAllBuildings;
    app.renderBuildPanel();
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
      app.buildSearch = search.value;
      app.renderBuildPanel();
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
      app.buildSearch = "";
      app.renderBuildPanel();
    });
  }
  {
    const search = required<HTMLInputElement>("recipe-search");
    search.addEventListener("input", () => {
      app.recipeSearch = search.value;
      app.renderRecipePanel();
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
      app.recipeSearch = "";
      app.renderRecipePanel();
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
    const definition = app.host.definitions.buildings.find(
      ({ id }) => id === Number(row.dataset.definitionId),
    );
    if (!definition) {
      app.showFeedback("No machine in the catalogue runs that recipe");
      return;
    }
    if (
      buildingAvailability(definition, app.snapshot, app.host.definitions.items)
        .locked
    ) {
      app.showFeedback(`${definition.name} is still locked by research`);
      return;
    }
    const recipeId = Number(row.dataset.recipeId);
    app.selectedRecipes.set(definition.id, recipeId);
    app.selectTool(definition.id);
    app.closePanels();
    app.showFeedback(
      `Holding ${definition.name} set to ${
        app.host.definitions.recipes.find(({ id }) => id === recipeId)?.name ??
        "that recipe"
      } — click or drag on the world to place`,
    );
  });
  required<HTMLButtonElement>("reset").addEventListener("click", () => {
    app.input.clear();
    void app.host
      .reset()
      .then((next) => {
        app.update(next);
        app.renderer.recenter();
      })
      .catch(app.reportWorkerError);
  });
  required<HTMLButtonElement>("turn").addEventListener("click", () =>
    app.rotateNewBuilding(),
  );
  // The dock's gather and deliver carry `data-native-action`, so they are wired here and only here:
  // a second listener bound to the same button by id sent the command twice.
  for (const button of document.querySelectorAll<HTMLButtonElement>(
    "[data-native-action]",
  )) {
    button.addEventListener("click", () => {
      const type = button.dataset.nativeAction;
      if (type === "gather" || type === "deposit") app.enqueue({ type });
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
      app.enqueue({ type: "deposit", item_id: itemId });
    } else {
      app.enqueue({ type: "deposit" });
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
        app.enqueue({ type: "post_request", request_id: requestId });
    },
  );
  required<HTMLButtonElement>("recenter").addEventListener("click", () =>
    app.renderer.recenter(),
  );
  required<HTMLButtonElement>("orbit-left").addEventListener("click", () =>
    app.orbitView(-1),
  );
  required<HTMLButtonElement>("orbit-right").addEventListener("click", () =>
    app.orbitView(1),
  );
  required<HTMLButtonElement>("toggle-grid").addEventListener(
    "click",
    (event) => {
      const visible = app.renderer.toggleGrid();
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
  app.worldSetup.bind({
    reportWorkerError: app.reportWorkerError,
    refreshContinue: () => app.updateContinueState(),
  });
  app.sessionMainMenu.addEventListener("click", () => {
    app.closePanels();
    app.worldSetup.open();
  });
  app.titleContinue.addEventListener("click", () => {
    const slot = latestCompatible(
      readCatalog(localStorage).slots,
      app.currentBuild(),
    );
    if (slot) {
      void app.loadSlot(slot);
    }
  });
  app.titleStartGame.addEventListener("click", async () => {
    app.input.clear();
    const scenario = app.worldSetup.scenario;
    // A typed name is an instruction — if it matches a slot, the player means that slot. A defaulted
    // one is not, so it steps aside rather than overwriting a factory nobody asked to replace.
    const typed = app.worldSetup.saveName;
    const fallback =
      app.host.scenarios.scenarios.find((entry) => entry.key === scenario)
        ?.name ?? AUTOSAVE_SLOT_NAME;
    try {
      const next = await app.host.newGame(
        scenario,
        app.worldSetup.seed,
        app.worldSetup.params ?? undefined,
        app.worldSetup.creative,
      );
      app.setRunName(
        typed || uniqueSlotName(fallback, readCatalog(localStorage).slots),
      );
      app.saveUi.clearSelection();
      app.beginRun(next);
      app.update(next);
      app.syncSessionInputs(next);
      app.renderer.recenter();
      // Nothing has happened in a world this new, so there is nothing for the close guard to save.
      app.markSaved(next.tick);
      app.worldSetup.close();
      app.closePanels();
    } catch (error) {
      app.reportWorkerError(error);
    }
  });
  required<HTMLButtonElement>("new-game").addEventListener(
    "click",
    async () => {
      app.input.clear();
      try {
        // A new run started from inside a creative session stays creative. The switch is in the panel
        // two rails over; making the player find it again after every restart would be the interface
        // forgetting something it was told.
        const next = await app.host.newGame(
          app.worldSetup.sessionScenario,
          app.worldSetup.sessionSeed,
          app.worldSetup.params ?? undefined,
          app.snapshot.player.creative,
        );
        app.beginRun(next);
        app.update(next);
        app.syncSessionInputs(next);
        app.renderer.recenter();
        app.markSaved(next.tick);
        app.closePanels();
      } catch (error) {
        app.reportWorkerError(error);
      }
    },
  );
  // Every creative control sends a command and then waits: none of them writes the state it is
  // showing. `renderCreative` sets each one from the next snapshot, so a refusal native reports —
  // a pack size that would strand carried stock, a grant with nowhere to go — shows up as the
  // control returning to what the simulation actually holds, with the reason in the toast.
  app.creativeSlotsInput.addEventListener("change", () => {
    const slots = Number(app.creativeSlotsInput.value);
    if (!Number.isSafeInteger(slots) || slots < 1) {
      app.creativeSlotsInput.value = String(app.snapshot.player.carry_slots);
      return;
    }
    app.enqueue({ type: "set_carry_slots", slots });
  });
  app.creativeClear.addEventListener("click", () => {
    app.enqueue({ type: "discard" });
  });
  for (const [id, action] of [
    ["creative-flood", "flood"],
    ["creative-drain", "drain"],
  ] as const) {
    required<HTMLButtonElement>(id).addEventListener("click", () => {
      if (!app.selected) {
        app.showFeedback("Select a surveyed hex first");
        return;
      }
      const quanta = Number(
        required<HTMLInputElement>("creative-water-depth").value,
      );
      if (!Number.isSafeInteger(quanta) || quanta < 1 || quanta > 32) {
        app.showFeedback("Water depth must be 1–32 quanta");
        return;
      }
      app.enqueue({
        type: "water_edit",
        q: app.selected.q,
        r: app.selected.r,
        action,
        quanta,
      });
    });
  }
  app.creativeItems.addEventListener("click", (event) => {
    const button = (event.target as HTMLElement).closest("button");
    if (!button) return;
    const item_id = Number(button.dataset.itemId);
    const quantity = Number(button.dataset.quantity);
    if (!Number.isSafeInteger(item_id) || !Number.isSafeInteger(quantity))
      return;
    app.enqueue({ type: "grant", item_id, quantity });
  });
  required<HTMLButtonElement>("run-copy").addEventListener(
    "click",
    async () => {
      const status = required<HTMLElement>("run-status");
      if (!app.run) {
        status.textContent = "Nothing timed yet.";
        return;
      }
      const report = formatRunReport(app.run);
      try {
        await navigator.clipboard.writeText(report);
        status.textContent = "Report copied.";
      } catch {
        // Clipboard permission is not guaranteed, and losing the report to a denied prompt would be
        // worse than a fallback that asks the player to copy it themselves.
        status.textContent = report;
      }
    },
  );
  required<HTMLButtonElement>("run-reset").addEventListener("click", () => {
    app.runElapsedMs = 0;
    app.run = startRun(Date.now(), app.snapshot.tick);
    writeRun(localStorage, app.run);
    app.renderRun();
    required<HTMLElement>("run-status").textContent = "Timer reset.";
  });
  required<HTMLButtonElement>("save").addEventListener("click", async () => {
    // Read before the round trip, so the mark never claims more of the run is written than is.
    const tick = app.snapshot.tick;
    try {
      const payload = await app.host.save();
      const build = app.currentBuild();
      const named = app.saveUi.name;
      const selected = app.saveUi.selectedId
        ? readCatalog(localStorage).slots.find(
            (slot) => slot.id === app.saveUi.selectedId,
          )
        : undefined;
      const overwriteName =
        named ||
        selected?.name ||
        app.runName ||
        app.snapshot.scenario_name ||
        "Save";
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
        app.updateContinueState(
          "Save failed: the envelope was not readable HXF1.",
        );
        return;
      }
      const { slots, error } = readCatalog(localStorage);
      if (error) {
        app.updateContinueState(error);
        return;
      }
      const nextSlots =
        drafted.id === selected?.id
          ? upsertSlot(slots, drafted)
          : replaceNamedSlot(slots, drafted);
      writeCatalog(localStorage, nextSlots);
      app.markSaved(tick);
      app.saveUi.select(drafted);
      // Saving under a name adopts it: the auto-save follows the player rather than continuing to
      // write to the name they just moved away from.
      app.setRunName(drafted.name);
      app.updateContinueState(`Saved “${drafted.name}”.`);
      app.showFeedback("Game saved");
      app.offerSaveFile(drafted);
    } catch (error) {
      app.updateContinueState(`Save failed: ${String(error)}`);
    }
  });
  required<HTMLButtonElement>("continue").addEventListener("click", () => {
    const slot = latestCompatible(
      readCatalog(localStorage).slots,
      app.currentBuild(),
    );
    if (slot) void app.loadSlot(slot);
  });
  required<HTMLButtonElement>("export-save").addEventListener("click", () => {
    void app.exportCurrentSave();
  });
  required<HTMLButtonElement>("import-save").addEventListener("click", () => {
    app.openSaveFilePicker();
  });
  required<HTMLButtonElement>("title-export-saves").addEventListener(
    "click",
    () => {
      void app.exportAllSaves();
    },
  );
  required<HTMLButtonElement>("title-import-saves").addEventListener(
    "click",
    () => {
      app.openSaveFilePicker();
    },
  );
  app.saveFileInput.addEventListener("change", () => {
    void app.importSaveFiles(app.saveFileInput.files);
  });
  app.toolShelf.addEventListener("click", (event) => {
    // The × on a filled slot clears it rather than selecting it.
    const clear = (event.target as Element).closest<HTMLElement>(
      ".hotbar-clear",
    );
    if (clear) {
      const slot = Number(
        clear.closest<HTMLElement>("[data-slot]")?.dataset.slot ?? -1,
      );
      if (slot >= 0) {
        app.assignHotbarSlot(slot, null);
        app.showFeedback(`Slot ${slot + 1} cleared`);
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
      app.showFeedback("That building is still locked by research");
      return;
    }
    const value = button.dataset.tool ?? "inspect";
    app.selectTool(/^\d+$/.test(value) ? Number(value) : (value as Tool));
  });
  /**
   * Dragging on the bar. A slot dragged onto another slot swaps with it; a slot dragged off the bar
   * entirely is cleared, which is the gesture a player already expects from a hotbar.
   */
  app.toolShelf.addEventListener("dragstart", (event) => {
    const slot = (event.target as Element).closest<HTMLElement>("[data-slot]");
    if (!slot || !event.dataTransfer) return;
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/hexfactory-slot", slot.dataset.slot ?? "");
  });
  app.toolShelf.addEventListener("dragover", (event) => {
    const slot = (event.target as Element).closest<HTMLElement>("[data-slot]");
    if (!slot) return;
    event.preventDefault();
    const index = Number(slot.dataset.slot);
    if (app.hotbarDragOver === index) return;
    app.hotbarDragOver = index;
    app.renderHotbarSlots();
  });
  app.toolShelf.addEventListener("dragleave", (event) => {
    if (
      (event.target as Element).closest("[data-slot]") &&
      app.hotbarDragOver !== null
    ) {
      app.hotbarDragOver = null;
      app.renderHotbarSlots();
    }
  });
  app.toolShelf.addEventListener("drop", (event) => {
    const target = (event.target as Element).closest<HTMLElement>(
      "[data-slot]",
    );
    app.hotbarDragOver = null;
    if (!target || !event.dataTransfer) return;
    event.preventDefault();
    const slot = Number(target.dataset.slot);
    const fromCatalogue = event.dataTransfer.getData("text/hexfactory-build");
    if (fromCatalogue) {
      app.assignHotbarSlot(slot, Number(fromCatalogue));
      return;
    }
    const fromSlot = Number(event.dataTransfer.getData("text/hexfactory-slot"));
    if (!Number.isInteger(fromSlot) || fromSlot === slot) {
      app.renderHotbarSlots();
      return;
    }
    // A swap rather than an insert, so no other binding shifts under the player's fingers.
    const moved = app.hotbar[fromSlot] ?? null;
    app.hotbar[fromSlot] = app.hotbar[slot] ?? null;
    app.hotbar[slot] = moved;
    app.saveHotbar();
    app.renderHotbarSlots();
    app.renderBuildPanel();
  });
  app.toolShelf.addEventListener("dragend", () => {
    if (app.hotbarDragOver === null) return;
    app.hotbarDragOver = null;
    app.renderHotbarSlots();
  });
}
