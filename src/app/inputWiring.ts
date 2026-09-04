import { MOVEMENT_KEYS } from "../core/input";
import { required } from "../ui/dom";
import { isKeyboardFocusedControl, isTypingTarget } from "../input/focus";
import type { Runtime } from "./runtime";

export async function inputWiring(app: Runtime): Promise<void> {
  app.cursorStackX = 0;
  app.cursorStackY = 0;
  window.addEventListener("pointermove", (event) => {
    app.cursorStackX = event.clientX;
    app.cursorStackY = event.clientY;
    const cursor = required<HTMLElement>("cursor-stack");
    cursor.style.left = `${app.cursorStackX}px`;
    cursor.style.top = `${app.cursorStackY}px`;
  });
  app.SPILL_NOTE =
    "What fits goes back to your pack. Anything that does not fit falls at the site, and ground items disappear after about a minute of simulation time.";
  app.CANCEL_NOTE =
    "The progress so far is lost. The ingredients go back to the machine's own ingredient slot, and its fuel and finished goods are left alone.";
  window.addEventListener("keydown", (event) => {
    if (event.code === "Escape" && app.stackDrag) {
      app.endStackDrag();
      event.preventDefault();
      return;
    }
    // A question owns the keyboard entirely while it is up. It has its own two buttons and its own
    // `Escape`, and a build key that fired past it would edit the world the player is being asked
    // about — so unlike the two panels below, there is no key that reaches through it.
    if (app.confirmDialog.open) return;
    if (app.researchDialog.open || app.skillsDialog.open) {
      if (
        ((app.researchDialog.open && event.code === "KeyO") ||
          (app.skillsDialog.open && event.code === "KeyK")) &&
        !isTypingTarget(event.target) &&
        !event.repeat &&
        !event.ctrlKey &&
        !event.metaKey &&
        !event.altKey
      ) {
        event.preventDefault();
        app.panels.close();
      }
      return;
    }
    if (isTypingTarget(event.target)) return;
    // Space presses a button the keyboard tabbed to. A mouse-focused button must not keep it:
    // activation happens on keyup, so returning here would both skip recenter and click the control.
    if (event.code === "Space" && isKeyboardFocusedControl(event.target))
      return;
    // Undo is the one binding that keeps its modifier, because every other application uses it.
    if ((event.ctrlKey || event.metaKey) && event.code === "KeyZ") {
      event.preventDefault();
      app.enqueue({
        type: app.boundaryTool.active
          ? "undo_boundary"
          : app.groundTool.active
            ? "undo_ground"
            : "undo",
      });
      return;
    }
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    if (
      app.boundaryTool.active &&
      ["Escape", "KeyR", "Delete", "Backspace"].includes(event.code)
    ) {
      event.preventDefault();
      if (event.code === "Escape") app.boundaryTool.escape();
      else if (event.code === "KeyR")
        app.boundaryTool.cycleAction(event.shiftKey);
      else app.boundaryTool.selectRemoval();
      return;
    }
    // The earthworks tray answers three questions — what work, what shape, how much — so each gets a
    // key rather than a trip to the panel. The brackets and backslash sit together under one hand and
    // are the only unclaimed keys near it; the digits belong to the hotbar.
    if (
      app.groundTool.active &&
      [
        "Escape",
        "KeyR",
        "Delete",
        "Backspace",
        "BracketLeft",
        "BracketRight",
        "Backslash",
        "Minus",
        "Equal",
      ].includes(event.code)
    ) {
      event.preventDefault();
      if (event.code === "Escape") app.groundTool.escape();
      else if (event.code === "KeyR")
        app.groundTool.cycleAction(event.shiftKey);
      else if (event.code === "BracketLeft") app.groundTool.cycleShape(true);
      else if (event.code === "BracketRight") app.groundTool.cycleShape(false);
      else if (event.code === "Backslash") app.groundTool.toggleOutline();
      else if (event.code === "Minus") app.groundTool.nudgeDepth(-1);
      else if (event.code === "Equal") app.groundTool.nudgeDepth(1);
      else app.groundTool.selectStrip();
      return;
    }
    if (event.code === "Backspace" || event.code === "Delete") {
      event.preventDefault();
      if (!event.repeat) app.deleteBuildingUnderCursorOrSelected();
      return;
    }
    if (event.code in MOVEMENT_KEYS) {
      event.preventDefault();
      if (!app.pressedMovement.has(event.code)) {
        app.pressedMovement.add(event.code);
        app.enqueue(app.currentMovementIntent(event.shiftKey));
      }
      return;
    }
    // Shift is a gait, not a key: it changes an intent already in flight, so it has to resend one.
    // Held on its own it does nothing, which is what makes it safe to press at any time.
    if (event.code === "ShiftLeft" || event.code === "ShiftRight") {
      app.runningHeld = true;
      if (app.pressedMovement.size)
        app.enqueue(app.currentMovementIntent(true));
      return;
    }
    if (event.code === "Escape") {
      if (app.worldSetup.dismiss()) return;
      app.selectTool("inspect");
      if (app.panels.isOpen(app.INVENTORY_PANEL)) app.packDeclined = true;
      app.closePanels();
    }
    // Space centres the camera, which is what the button beside it does and what a player who has
    // panned away needs most.
    else if (event.code === "Space") app.renderer.recenter();
    else if (event.code === "ArrowLeft") app.orbitView(-1);
    else if (event.code === "ArrowRight") app.orbitView(1);
    else if (event.code === "ArrowUp") app.tiltView(1);
    else if (event.code === "ArrowDown") app.tiltView(-1);
    else if (event.code === "KeyM") app.preferences.toggleMuted();
    else if (event.code in app.PANEL_KEYS)
      app.togglePanel(app.PANEL_KEYS[event.code] as string);
    else if (event.code === "KeyF") {
      // Held rather than tapped. A swing has to be worked through natively before it pays, so the
      // repeat cannot outrun the simulation however fast the frames arrive.
      app.gatherHeld = true;
      app.enqueue({ type: "gather" });
    } else if (event.code === "KeyX") app.enqueue({ type: "deposit" });
    else if (event.code === "KeyR")
      app.rotateUnderCursorOrPending(event.shiftKey);
    else if (event.code === "KeyG") {
      if (app.groundTool.active) app.groundTool.close();
      else app.groundTool.open();
    } else if (event.code === "KeyQ") app.pickToolUnderCursor();
    else if (event.code === "KeyE") app.selectTool("erase");
    else if (/^Digit[1-9]$/.test(event.code)) {
      // A digit is a slot, not an index into the catalogue. Which building it builds is the
      // player's arrangement, and it is theirs to change.
      const slot = Number(event.code.slice(-1)) - 1;
      const value = app.hotbar[slot] ?? null;
      if (value === null) {
        app.showFeedback(
          `Slot ${slot + 1} is empty — pin something from Build (B)`,
        );
        event.preventDefault();
        return;
      }
      app.selectTool(value);
      event.preventDefault();
      return;
    } else return;
    event.preventDefault();
  });
  window.addEventListener("keyup", (event) => {
    if (
      app.confirmDialog.open ||
      app.researchDialog.open ||
      app.skillsDialog.open
    )
      return;
    if (
      event.code === "Space" &&
      !isTypingTarget(event.target) &&
      !isKeyboardFocusedControl(event.target)
    ) {
      // Buttons fire on Space keyup. Recenter already handled keydown; this stops the click.
      event.preventDefault();
    }
    if (event.code === "KeyF") app.gatherHeld = false;
    if (event.code === "ShiftLeft" || event.code === "ShiftRight") {
      app.runningHeld = event.shiftKey;
      if (app.pressedMovement.size)
        app.enqueue(app.currentMovementIntent(app.runningHeld));
      return;
    }
    if (!app.pressedMovement.delete(event.code)) return;
    event.preventDefault();
    // Stopping is sent on the same frame the key comes up. Coalescing the release made every stop
    // read as a slide, which is the kind of latency a player feels without being able to name it.
    app.enqueue(app.currentMovementIntent(event.shiftKey));
  });
  window.addEventListener("blur", () => {
    app.endStackDrag();
    app.gatherHeld = false;
    app.runningHeld = false;
    app.stopAiming();
    if (!app.pressedMovement.size) return;
    app.pressedMovement.clear();
    app.enqueue(app.currentMovementIntent());
  });
  app.canvas.addEventListener("pointermove", (event) => {
    // Aiming survives panning and dragging: the player keeps facing the pointer whatever else the
    // pointer is doing. Touch never aims, because a finger that is not on the glass points nowhere.
    if (event.pointerType !== "touch")
      app.aimPointer = { x: event.clientX, y: event.clientY };
    if (app.panPointer?.id === event.pointerId) {
      const dx = event.clientX - app.panPointer.x;
      const dy = event.clientY - app.panPointer.y;
      if (Math.abs(dx) + Math.abs(dy) > 1) app.panPointer.moved = true;
      app.panPointer.x = event.clientX;
      app.panPointer.y = event.clientY;
      if (app.panPointer.mode === "pan") {
        app.renderer.panBy(dx, dy);
      } else {
        app.renderer.lookBy(dx, dy);
        app.syncHoverWithCamera();
        if (app.pressedMovement.size)
          app.enqueue(app.currentMovementIntent(app.runningHeld));
      }
      return;
    }
    const coordinate = app.renderer.pick(event.clientX, event.clientY);
    if (app.harvestPointer?.id === event.pointerId) {
      // The hold walks to the hex under the cursor and keeps working from there. Selecting it is what
      // makes the target visible, which matters more here than for a click: the gesture repeats.
      if (
        coordinate.q !== app.harvestPointer.q ||
        coordinate.r !== app.harvestPointer.r
      ) {
        app.harvestPointer = {
          id: event.pointerId,
          q: coordinate.q,
          r: coordinate.r,
        };
        app.selected = coordinate;
        app.renderer.setSelection(coordinate);
        app.renderInspector();
      }
      app.hover = coordinate;
      app.refreshHoverPreview();
      return;
    }
    if (app.dragBuild?.id === event.pointerId) {
      if (
        coordinate.q === app.dragBuild.to.q &&
        coordinate.r === app.dragBuild.to.r
      )
        return;
      app.dragBuild.to = coordinate;
      void app.refreshDragPreview();
      return;
    }
    app.hover = coordinate;
    const vertexPoint = app.renderer.pickWorld(event.clientX, event.clientY);
    app.boundaryTool.hover(coordinate, vertexPoint);
    app.groundTool.hover(coordinate, vertexPoint);
    app.refreshHoverPreview();
  });
  app.canvas.addEventListener("pointerdown", (event) => {
    if (app.boundaryTool.active && event.button === 2) {
      event.preventDefault();
      app.boundaryTool.clear();
      return;
    }
    if (app.groundTool.active && event.button === 2) {
      event.preventDefault();
      app.groundTool.clear();
      return;
    }
    // The map is the outside surface for every workspace. Any deliberate world gesture clears the
    // overlay first; right-click harvesting and middle-button camera movement follow the same
    // expectation as an ordinary click rather than leaving a panel covering the action.
    app.closePanels();
    if (event.button === 2) {
      if (app.snapshot?.player.hand) {
        const dropHex = app.renderer.pick(event.clientX, event.clientY);
        app.enqueue({
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
      const harvest = app.renderer.pick(event.clientX, event.clientY);
      app.harvestPointer = { id: event.pointerId, q: harvest.q, r: harvest.r };
      app.selected = harvest;
      app.renderer.setSelection(harvest);
      app.enqueue({ type: "gather_at", ...harvest });
      app.renderInspector();
      // Captured last: capture is what keeps the gesture alive off the canvas, not what makes the
      // press mean something. Taking it first would let a refused capture swallow the first harvest
      // while still leaving the hold armed.
      app.canvas.setPointerCapture(event.pointerId);
      event.preventDefault();
      return;
    }
    if (event.button === 1) {
      app.panPointer = {
        id: event.pointerId,
        x: event.clientX,
        y: event.clientY,
        moved: false,
        mode: event.ctrlKey ? "pan" : "look",
      };
      app.canvas.setPointerCapture(event.pointerId);
      event.preventDefault();
      return;
    }
    if (event.button !== 0 || !app.draggableTool() || app.snapshot?.player.hand)
      return;
    const from = app.renderer.pick(event.clientX, event.clientY);
    app.dragBuild = {
      id: event.pointerId,
      from,
      to: from,
      erasing: app.tool === "erase",
    };
    app.canvas.setPointerCapture(event.pointerId);
    void app.refreshDragPreview();
  });
  app.canvas.addEventListener("pointerup", (event) => {
    if (app.panPointer?.id === event.pointerId) {
      app.suppressMapClick = app.panPointer.moved;
      app.canvas.releasePointerCapture(event.pointerId);
      app.panPointer = null;
      app.syncHoverWithCamera();
      return;
    }
    if (app.harvestPointer?.id === event.pointerId) {
      app.canvas.releasePointerCapture(event.pointerId);
      // Releasing ends the hold. The harvest began on the press and repeated every frame since.
      app.harvestPointer = null;
      return;
    }
    if (app.dragBuild?.id !== event.pointerId) return;
    const { from, to, erasing } = app.dragBuild;
    app.endDrag(event.pointerId);
    // A drag that never left its starting hex is an ordinary click; the click handler runs it.
    if (from.q === to.q && from.r === to.r) return;
    app.suppressMapClick = true;
    app.selected = to;
    app.renderer.setSelection(to);
    if (erasing) {
      void app.eraseLine(from, to);
      return;
    }
    app.enqueue({
      type: "place_line",
      q: from.q,
      r: from.r,
      to_q: to.q,
      to_r: to.r,
      definition_id: app.tool as number,
      orientation: app.orientation,
      recipe_id: app.recipeFor(app.tool),
    });
  });
  app.canvas.addEventListener("pointercancel", (event) => {
    // A cancelled pointer never sends `pointerup`, and a held harvest that outlived its gesture
    // would keep working a hex with nothing holding the button down.
    if (app.panPointer?.id === event.pointerId) app.panPointer = null;
    if (app.harvestPointer?.id === event.pointerId) app.harvestPointer = null;
    app.endDrag(event.pointerId);
  });
  app.canvas.addEventListener("pointerleave", () => {
    app.stopAiming();
    if (!app.panPointer && !app.harvestPointer && !app.dragBuild) {
      app.hover = null;
      app.refreshHoverPreview();
    }
  });
  app.canvas.addEventListener("contextmenu", (event) => event.preventDefault());
  app.canvas.addEventListener(
    "wheel",
    (event) => {
      event.preventDefault();
      app.renderer.zoomAt(
        event.clientX,
        event.clientY,
        event.deltaY < 0 ? 1.12 : 0.89,
      );
      app.syncHoverWithCamera();
    },
    { passive: false },
  );
  app.canvas.addEventListener("click", (event) => {
    if (app.suppressMapClick) {
      app.suppressMapClick = false;
      return;
    }
    const coordinate = app.renderer.pick(event.clientX, event.clientY);
    if (app.boundaryTool.active) {
      app.boundaryTool.pick(
        coordinate,
        app.renderer.pickWorld(event.clientX, event.clientY),
      );
      return;
    }
    if (app.groundTool.active) {
      app.groundTool.pick(
        coordinate,
        app.renderer.pickWorld(event.clientX, event.clientY),
      );
      return;
    }
    if (app.snapshot?.player.hand) {
      const placed =
        event.ctrlKey || event.metaKey ? 1 : app.snapshot.player.hand.quantity;
      app.enqueue({
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
      app.tool === "inspect" &&
      app.selected !== null &&
      app.selected.q === coordinate.q &&
      app.selected.r === coordinate.r;
    app.selected = coordinate;
    app.renderer.setSelection(coordinate);
    if (repeat) app.enqueue({ type: "walk_to", ...coordinate });
    // Empty ground keeps native's answer: a sweep with the erase tool crosses far more nothing than
    // something, and a local complaint on every miss would be noise the old path never made.
    else if (app.tool === "erase") {
      if (app.buildingAt(coordinate)) app.eraseBuilding(coordinate);
      else app.enqueue({ type: "erase", ...coordinate });
    } else if (app.tool === "rotate")
      app.enqueue({ type: "rotate", ...coordinate });
    else if (app.tool === "upgrade")
      app.enqueue({ type: "upgrade", ...coordinate });
    else if (typeof app.tool === "number") {
      app.enqueue({
        type: "place",
        ...coordinate,
        definition_id: app.tool,
        orientation: app.orientation,
        recipe_id: app.recipeFor(app.tool),
      });
    }
    app.renderInspector();
  });
}
