import type { NativeInputCommand, StockKind } from "../core/types";
import { required } from "../ui/dom";
import type { Runtime } from "./runtime";

export async function buildWiring(app: Runtime): Promise<void> {
  app.buildGroups = required<HTMLDivElement>("build-groups");
  app.buildGroups.addEventListener("click", (event) => {
    const target = event.target as Element;
    const recipeRow = target.closest<HTMLElement>(".recipe-row");
    if (recipeRow) {
      const definitionId = Number(recipeRow.dataset.definitionId);
      app.selectedRecipes.set(definitionId, Number(recipeRow.dataset.recipeId));
      app.selectTool(definitionId);
      app.closePanels();
      app.showFeedback(
        `Holding ${app.host.definitions.buildings.find(({ id }) => id === definitionId)?.name ?? "building"} — click or drag on the world to place`,
      );
      return;
    }
    const card = target.closest<HTMLElement>(".build-card");
    if (!card) return;
    const definitionId = Number(card.dataset.definitionId);
    if (target.closest("[data-pin]")) {
      app.pinToHotbar(definitionId);
      return;
    }
    if (card.classList.contains("locked")) {
      app.showFeedback("That building is still locked by research");
      return;
    }
    app.selectTool(definitionId);
    app.closePanels();
    app.showFeedback(
      `Holding ${app.host.definitions.buildings.find(({ id }) => id === definitionId)?.name ?? "building"} — click or drag on the world to place`,
    );
  });
  app.buildGroups.addEventListener("dragstart", (event) => {
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
    if (typeof app.tool !== "number") return;
    app.selectedRecipes.set(app.tool, Number(select.value));
    app.refreshHoverPreview();
  });
  required<HTMLSelectElement>("machine-recipe").addEventListener(
    "change",
    (event) => {
      const select = event.currentTarget as HTMLSelectElement;
      const q = Number(select.dataset.q);
      const r = Number(select.dataset.r);
      const recipe_id = Number(select.value);
      const assign = (): void =>
        void app.enqueue({ type: "set_recipe", q, r, recipe_id });
      const building = app.buildingAt({ q, r });
      if (!building || building.progress === 0) {
        assign();
        return;
      }
      // Native refuses a reassignment mid-craft, so this used to be a control that silently did
      // nothing. It now asks, and on a yes clears the craft ahead of the new assignment. The list
      // snaps back first because a `<select>` showing a recipe the machine never took would state an
      // assignment that is not true — the snapshot puts it forward again once native agrees.
      select.value = String(building.recipe_id ?? "");
      app.cancelCraft(building, assign);
    },
  );
  required<HTMLButtonElement>("inspect-cancel-craft").addEventListener(
    "click",
    (event) => {
      const button = event.currentTarget as HTMLButtonElement;
      const building = app.buildingAt({
        q: Number(button.dataset.q),
        r: Number(button.dataset.r),
      });
      if (building && building.progress > 0) app.cancelCraft(building);
    },
  );
  required<HTMLButtonElement>("inspect-upgrade").addEventListener(
    "click",
    (event) => {
      const button = event.currentTarget as HTMLButtonElement;
      app.enqueue({
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
      app.enqueue({
        type: "set_enabled",
        q: Number(button.dataset.q),
        r: Number(button.dataset.r),
        // The state the press is asking for, read off the button rather than off the machine: by the
        // time this lands the snapshot may have moved, and a flip would then land the wrong way up.
        enabled: button.dataset.enable === "1",
      });
    },
  );
  app.STACK_DRAG_LIFT = 6;
  app.stackDrag = null;
  app.stackDragHandledClick = false;
  // Consume the drag's synthetic click even when its target is outside either grid. A fresh press
  // always starts a new gesture, so a browser that emits no click cannot swallow the next real one.
  window.addEventListener(
    "pointerdown",
    () => {
      app.stackDragHandledClick = false;
    },
    true,
  );
  window.addEventListener(
    "click",
    (event) => {
      if (!app.stackDragHandledClick) return;
      app.stackDragHandledClick = false;
      event.preventDefault();
      event.stopImmediatePropagation();
    },
    true,
  );
  for (const id of ["inventory", "inspector-actions"]) {
    const grid = required<HTMLElement>(id);
    grid.addEventListener("click", app.stackGesture);
    grid.addEventListener("contextmenu", app.stackGesture);
    grid.addEventListener("pointerdown", (event) => {
      // Only the primary button drags. The secondary one already means "half", and taking it over
      // would cost the player a gesture they have been using since the panel existed.
      if (
        event.button !== 0 ||
        !event.isPrimary ||
        app.snapshot.player.hand ||
        app.stackDrag
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
      if (source === "building" && app.itemById(itemId)?.fluid) {
        app.showFeedback(
          "Loose fluid cannot be lifted by hand — connect a pipe or empty it into a barrel.",
        );
        return;
      }
      app.stackDrag = {
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
    const drag = app.stackDrag;
    if (!drag || event.pointerId !== drag.pointerId) return;
    if (!drag.lifted) {
      const travelled =
        Math.abs(event.clientX - drag.startX) +
        Math.abs(event.clientY - drag.startY);
      if (travelled < app.STACK_DRAG_LIFT) return;
      // The slot may have been repainted between the press and the lift, so the amount is re-read.
      // The element itself survives — the grids are keyed and patched in place — but its contents do
      // not, and lifting more than is there would make the drop a refusal at the far end.
      if (Number(drag.origin.dataset.quantity) < drag.quantity) {
        app.stackDrag = null;
        return;
      }
      drag.lifted = true;
      document.body.classList.add("dragging-stack");
      const cursor = required<HTMLElement>("cursor-stack");
      cursor.hidden = false;
      app.paintChip(cursor, drag.itemId, {
        count: drag.quantity,
        named: false,
        short: true,
      });
    }
    event.preventDefault();
    app.paintStackDropTargets(
      drag,
      document
        .elementFromPoint(event.clientX, event.clientY)
        ?.closest("[data-stack-source]"),
    );
  });
  window.addEventListener("pointerup", (event) => {
    const drag = app.stackDrag;
    if (!drag || event.pointerId !== drag.pointerId) return;
    const lifted = drag.lifted;
    const slot = lifted
      ? document
          .elementFromPoint(event.clientX, event.clientY)
          ?.closest<HTMLElement>("[data-stack-source]")
      : null;
    app.endStackDrag();
    // An unlifted press is a click, and the click handler is about to run with the gesture the player
    // actually made. Only a real drag swallows it.
    if (!lifted) return;
    app.stackDragHandledClick = true;
    if (!slot) {
      app.showFeedback("Stack returned — drop it on a slot to move it");
      return;
    }
    const refusal = app.stackDropRefusal(drag, slot);
    if (refusal !== null) {
      if (refusal) app.showFeedback(refusal);
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
    if (!app.input.enqueueBatch([drag.pickup, place]))
      app.showFeedback(
        "Too many commands — stack stayed where it was. Try again.",
      );
  });
  // A cancelled pointer — the browser taking over for a gesture, a window losing focus — is a release
  // over nothing, and lands in the same place: nothing was sent, so nothing has to be put back.
  window.addEventListener("pointercancel", (event) => {
    if (app.stackDrag?.pointerId === event.pointerId) app.endStackDrag();
  });
}
