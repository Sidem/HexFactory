import { halfTransfer } from "../core/commands";
import { movementIntent } from "../core/input";
import { type SaveSlot } from "../core/saveSlots";
import type { NativeInputCommand, StockKind } from "../core/types";
import { required } from "../ui/dom";
import type { StackDrag } from "./runtime";
import { Runtime } from "./runtime";

declare module "./runtime" {
  interface Runtime {
    syncHoverWithCamera(): void;
    flushHoverPreview(): Promise<void>;
    setRunName(name: string): void;
    offerSaveFile(slot: SaveSlot): void;
    stackGesture(event: MouseEvent): void;
    stackSlots(): HTMLElement[];
    stackDropRefusal(drag: StackDrag, slot: HTMLElement): string | null;
    paintStackDropTargets(drag: StackDrag | null, over?: Element | null): void;
    endStackDrag(): void;
    currentMovementIntent(running?: boolean): NativeInputCommand;
    orbitView(step: -1 | 1): void;
    tiltView(step: -1 | 1): void;
    eraseBuilding(target: { q: number; r: number }): void;
  }
}

Runtime.prototype.syncHoverWithCamera = function syncHoverWithCamera(
  this: Runtime,
): void {
  if (
    !this.aimPointer ||
    this.panPointer ||
    this.harvestPointer ||
    this.dragBuild
  )
    return;
  const coordinate = this.renderer.pick(this.aimPointer.x, this.aimPointer.y);
  // Vertex tools follow the pointer within a hex, so they are told even when the hex has not moved.
  const point = this.renderer.pickWorld(this.aimPointer.x, this.aimPointer.y);
  this.boundaryTool.hover(coordinate, point);
  this.groundTool.hover(coordinate, point);
  if (this.hover?.q === coordinate.q && this.hover.r === coordinate.r) return;
  this.hover = coordinate;
  this.refreshHoverPreview();
};

Runtime.prototype.flushHoverPreview = async function flushHoverPreview(
  this: Runtime,
): Promise<void> {
  this.previewPending = true;
  while (this.previewRequested) {
    this.previewRequested = false;
    const revision = this.previewRevision;
    const coordinate = this.hover;
    const definitionId = typeof this.tool === "number" ? this.tool : null;
    const direction = this.orientation;
    if (!coordinate || definitionId === null) {
      this.hoverPreview = null;
    } else {
      try {
        const result = await this.host.placementPreview(
          coordinate.q,
          coordinate.r,
          definitionId,
          direction,
          this.recipeFor(definitionId),
        );
        if (revision === this.previewRevision) this.hoverPreview = result;
      } catch (error) {
        if (revision === this.previewRevision)
          this.showFeedback(`Placement preview failed: ${String(error)}`);
      }
    }
    if (revision === this.previewRevision) {
      this.renderer.setHover(this.hover, this.hoverPreview);
      required<HTMLElement>("placement-value").textContent =
        this.hoverPreview?.reason ?? "";
    }
  }
  this.previewPending = false;
};

Runtime.prototype.setRunName = function setRunName(
  this: Runtime,
  name: string,
): void {
  this.runName = name;
  this.saveUi.setName(name);
};

Runtime.prototype.offerSaveFile = function offerSaveFile(
  this: Runtime,
  slot: SaveSlot,
): void {
  this.confirmDialog.ask(
    {
      title: `Saved “${slot.name}”`,
      note: "That save is in this browser, and clearing site data removes it. Keep a copy as a file too?",
      accept: "Save to file",
      cancel: "Browser only",
    },
    () => void this.exportSlotFile(slot),
  );
};

Runtime.prototype.stackGesture = function stackGesture(
  this: Runtime,
  event: MouseEvent,
): void {
  const slot = (event.target as Element).closest<HTMLElement>(
    "[data-stack-source]",
  );
  if (!slot) return;
  event.preventDefault();
  const source = slot.dataset.stackSource;
  if (source !== "player" && source !== "building") return;
  const hand = this.snapshot.player.hand ?? undefined;
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
      this.enqueue({ type: "place_player_stack", quantity: placed });
      return;
    }
    const stock = slot.dataset.stock as Exclude<StockKind, "auto">;
    this.enqueue({
      type: "place_building_stack",
      q: Number(slot.dataset.q),
      r: Number(slot.dataset.r),
      stock,
      quantity: placed,
    });
    return;
  }
  if (!Number.isInteger(itemId) || itemId <= 0 || quantity <= 0) return;
  if (source === "building" && this.itemById(itemId)?.fluid) {
    this.showFeedback(
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
      this.enqueue({
        type: "withdraw",
        q: Number(slot.dataset.q),
        r: Number(slot.dataset.r),
        stock: slot.dataset.stock as Exclude<StockKind, "auto">,
        item_id: itemId,
        quantity: quickQuantity,
      });
      return;
    }
    const target = this.selected ? this.buildingAt(this.selected) : undefined;
    if (!target || !this.HAND_REACHABLE.has(target.kind)) return;
    this.enqueue({
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
    this.enqueue({ type: "pickup_player_stack", item_id: itemId, quantity });
  } else {
    this.enqueue({
      type: "pickup_building_stack",
      q: Number(slot.dataset.q),
      r: Number(slot.dataset.r),
      stock: slot.dataset.stock as Exclude<StockKind, "auto">,
      item_id: itemId,
      quantity,
    });
  }
};

Runtime.prototype.stackSlots = function stackSlots(
  this: Runtime,
): HTMLElement[] {
  return ["inventory", "inspector-actions"].flatMap((id) =>
    Array.from(
      required<HTMLElement>(id).querySelectorAll<HTMLElement>(
        "[data-stack-source]",
      ),
    ),
  );
};

Runtime.prototype.stackDropRefusal = function stackDropRefusal(
  this: Runtime,
  drag: StackDrag,
  slot: HTMLElement,
): string | null {
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
    return `That slot is holding ${this.itemById(held)?.name ?? "something else"}`;
  return null;
};

Runtime.prototype.paintStackDropTargets = function paintStackDropTargets(
  this: Runtime,
  drag: StackDrag | null,
  over?: Element | null,
): void {
  for (const slot of this.stackSlots()) {
    const allowed = drag !== null && this.stackDropRefusal(drag, slot) === null;
    slot.classList.toggle("drop-ready", allowed);
    slot.classList.toggle("drop-over", allowed && slot === over);
  }
};

Runtime.prototype.endStackDrag = function endStackDrag(this: Runtime): void {
  if (this.stackDrag?.lifted) {
    required<HTMLElement>("cursor-stack").hidden = !this.snapshot.player.hand;
    this.paintStackDropTargets(null);
    document.body.classList.remove("dragging-stack");
  }
  this.stackDrag = null;
};

Runtime.prototype.currentMovementIntent = function currentMovementIntent(
  this: Runtime,
  running = false,
): NativeInputCommand {
  return movementIntent(this.pressedMovement, running, (x, y) =>
    this.renderer.screenMovement(x, y),
  );
};

Runtime.prototype.orbitView = function orbitView(
  this: Runtime,
  step: -1 | 1,
): void {
  this.renderer.orbitBy(step);
  this.syncHoverWithCamera();
  if (this.pressedMovement.size)
    this.enqueue(this.currentMovementIntent(this.runningHeld));
};

Runtime.prototype.tiltView = function tiltView(
  this: Runtime,
  step: -1 | 1,
): void {
  this.renderer.tiltBy(step);
  this.syncHoverWithCamera();
};

Runtime.prototype.eraseBuilding = function eraseBuilding(
  this: Runtime,
  target: {
    q: number;
    r: number;
  },
): void {
  const building = this.buildingAt(target);
  if (!building) {
    this.showFeedback("No building selected to delete");
    return;
  }
  this.selected = target;
  this.renderer.setSelection(target);
  const erase = (): void =>
    void this.enqueue({ type: "erase", q: target.q, r: target.r });
  const held = this.heldStock(building);
  if (held.length === 0 && building.progress === 0) {
    erase();
    return;
  }
  const name =
    this.host.definitions.buildings.find(
      ({ id }) => id === building.definition_id,
    )?.name ?? "building";
  this.confirmDialog.ask(
    {
      title: `Demolish the ${name}?`,
      rows: held.map((entry) => ({
        text: `${entry.quantity} × ${this.itemById(entry.item_id)?.name ?? "item"}`,
        paint: (host_) =>
          void this.paintChip(host_, entry.item_id, { named: false }),
      })),
      note: this.SPILL_NOTE,
      accept: "Demolish",
      cancel: "Keep it",
    },
    erase,
  );
};
