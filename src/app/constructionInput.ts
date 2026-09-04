import { rotateHexDirection, type HexDirection } from "@hexlife/embed/hex";
import { DIRECTION_NAMES, rotateAnyOrientation } from "../core/directions";
import type { EntitySnapshot } from "../core/types";
import { required } from "../ui/dom";
import type { Tool } from "./runtime";
import { Runtime } from "./runtime";

declare module "./runtime" {
  interface Runtime {
    heldStock(building: EntitySnapshot): {
      item_id: number;
      quantity: number;
    }[];
    cancelCraft(building: EntitySnapshot, then?: () => void): void;
    eraseLine(
      from: {
        q: number;
        r: number;
      },
      to: {
        q: number;
        r: number;
      },
    ): Promise<void>;
    deleteBuildingUnderCursorOrSelected(): void;
    draggableTool(): boolean;
    recipeFor(value: Tool): number | undefined;
    refreshDragPreview(): Promise<void>;
    endDrag(pointerId: number): void;
    rotateUnderCursorOrPending(reverse?: boolean): void;
    pickToolUnderCursor(): void;
    buildingAt(coordinate: {
      q: number;
      r: number;
    }): EntitySnapshot | undefined;
    setOrientation(next: number): void;
    orientationRange(tool: Tool): {
      start: number;
      end: number;
    };
    orientationAllowed(tool: Tool, orientation: number): boolean;
    rotateNewBuilding(step?: number): void;
    stopAiming(): void;
    sendAim(): void;
  }
}

Runtime.prototype.heldStock = function heldStock(
  this: Runtime,
  building: EntitySnapshot,
): {
  item_id: number;
  quantity: number;
}[] {
  if (!this.HAND_REACHABLE.has(building.kind)) return [];
  return this.stockCompartments(building)
    .flatMap(({ entries }) => entries)
    .filter(({ quantity }) => quantity > 0);
};

Runtime.prototype.cancelCraft = function cancelCraft(
  this: Runtime,
  building: EntitySnapshot,
  then?: () => void,
): void {
  const recipe = this.host.definitions.recipes.find(
    ({ id }) => id === building.recipe_id,
  );
  this.confirmDialog.ask(
    {
      title: recipe ? `Cancel the ${recipe.name} craft?` : "Cancel the craft?",
      rows: (recipe?.inputs ?? []).map((input) => ({
        text: `${input.quantity} × ${this.itemById(input.item_id)?.name ?? "item"}`,
        paint: (host_) =>
          void this.paintChip(host_, input.item_id, { named: false }),
      })),
      note: this.CANCEL_NOTE,
      accept: "Cancel the craft",
      cancel: "Keep working",
    },
    () => {
      this.enqueue({ type: "cancel_craft", q: building.q, r: building.r });
      then?.();
    },
  );
};

Runtime.prototype.eraseLine =
  /**
   * A removal drag, asked about once for the whole sweep.
   *
   * One prompt per building would make clearing a factory unusable, and no prompt at all would make
   * the sweep the one route that empties a row of full containers without saying so. So the question
   * is asked once, over the totals, and the drag is either taken or dropped entire.
   */
  async function eraseLine(
    this: Runtime,
    from: {
      q: number;
      r: number;
    },
    to: {
      q: number;
      r: number;
    },
  ): Promise<void> {
    // Ask for the released endpoints, not the last asynchronous hover preview, which can still be
    // in flight. A fast sweep must not silently demolish stock outside an older preview.
    let cells;
    try {
      cells = await this.host.linePreview(from.q, from.r, to.q, to.r);
    } catch (error) {
      this.showFeedback(`Removal cancelled: ${String(error)}`);
      return;
    }
    const send = (): void =>
      void this.enqueue({
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
      const building = this.buildingAt(cell);
      if (!building || seen.has(building.id)) continue;
      seen.add(building.id);
      const held = this.heldStock(building);
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
    this.confirmDialog.ask(
      {
        title:
          buildings === 1
            ? "Demolish 1 building with stock inside?"
            : `Demolish ${buildings} buildings with stock inside?`,
        rows: [...totals].map(([itemId, quantity]) => ({
          text: `${quantity} × ${this.itemById(itemId)?.name ?? "item"}`,
          paint: (holder: HTMLElement) =>
            void this.paintChip(holder, itemId, { named: false }),
        })),
        note: this.SPILL_NOTE,
        accept: "Demolish",
        cancel: "Keep them",
      },
      send,
    );
  };

Runtime.prototype.deleteBuildingUnderCursorOrSelected =
  function deleteBuildingUnderCursorOrSelected(this: Runtime): void {
    const target =
      this.hover && this.buildingAt(this.hover) ? this.hover : this.selected;
    if (!target) {
      this.showFeedback("No building selected to delete");
      return;
    }
    this.eraseBuilding(target);
  };

Runtime.prototype.draggableTool = function draggableTool(
  this: Runtime,
): boolean {
  if (this.tool === "erase") return true;
  if (typeof this.tool !== "number") return false;
  const definition = this.host.definitions.buildings.find(
    ({ id }) => id === this.tool,
  );
  return definition?.footprint.length === 1;
};

Runtime.prototype.recipeFor = function recipeFor(
  this: Runtime,
  value: Tool,
): number | undefined {
  const definition =
    typeof value === "number"
      ? this.host.definitions.buildings.find(({ id }) => id === value)
      : undefined;
  const choices = this.recipeChoices(definition);
  if (!choices.length || !definition) return undefined;
  const chosen = this.selectedRecipes.get(definition.id);
  return chosen !== undefined && choices.some(({ id }) => id === chosen)
    ? chosen
    : choices[0]?.id;
};

Runtime.prototype.refreshDragPreview =
  /**
   * Ask native what the current drag would do and hand the answer straight to the renderer. The host
   * never resolves the path itself, so the preview and the eventual command cannot disagree.
   */
  async function refreshDragPreview(this: Runtime): Promise<void> {
    if (!this.dragBuild || this.dragPreviewPending) return;
    this.dragPreviewPending = true;
    try {
      while (this.dragBuild) {
        const { from, to, erasing } = this.dragBuild;
        const cells = await this.host.linePreview(
          from.q,
          from.r,
          to.q,
          to.r,
          erasing ? undefined : (this.tool as number),
          this.orientation,
          erasing ? undefined : this.recipeFor(this.tool),
        );
        if (!this.dragBuild) break;
        this.renderer.setDragPath(cells);
        const legal = cells.filter((cell) => cell.legal).length;
        const definition =
          erasing || typeof this.tool !== "number"
            ? undefined
            : this.host.definitions.buildings.find(
                ({ id }) => id === this.tool,
              );
        required<HTMLElement>("placement-value").textContent = erasing
          ? `Remove ${legal} of ${cells.length}`
          : definition?.underpass_span !== undefined && legal === 2
            ? `Build paired portals · ${definition.underpass_span}-hex reach`
            : (cells.find((cell) => !cell.legal && cell.reason)?.reason ??
              `Build ${legal} of ${cells.length}`);
        if (this.dragBuild.to.q === to.q && this.dragBuild.to.r === to.r) break;
      }
    } catch (error) {
      this.showFeedback(`Drag preview failed: ${String(error)}`);
    } finally {
      this.dragPreviewPending = false;
    }
  };

Runtime.prototype.endDrag = function endDrag(
  this: Runtime,
  pointerId: number,
): void {
  if (this.dragBuild?.id !== pointerId) return;
  this.dragBuild = null;
  this.renderer.setDragPath([]);
  if (this.canvas.hasPointerCapture(pointerId))
    this.canvas.releasePointerCapture(pointerId);
  required<HTMLElement>("placement-value").textContent =
    this.hoverPreview?.reason ?? "";
};

Runtime.prototype.rotateUnderCursorOrPending =
  function rotateUnderCursorOrPending(this: Runtime, reverse = false): void {
    if (typeof this.tool === "number" || this.tool === "inspect") {
      const target = this.hover ?? this.selected;
      const existing =
        typeof this.tool === "number"
          ? null
          : target && this.buildingAt(target);
      if (existing && target) {
        this.enqueue({ type: "rotate", q: target.q, r: target.r, reverse });
        return;
      }
    }
    this.rotateNewBuilding(reverse ? -1 : 1);
  };

Runtime.prototype.pickToolUnderCursor = function pickToolUnderCursor(
  this: Runtime,
): void {
  const target = this.hover ?? this.selected;
  const building = target ? this.buildingAt(target) : undefined;
  if (!building) {
    this.showFeedback("Nothing under the cursor to copy");
    return;
  }
  const definition = this.host.definitions.buildings.find(
    ({ id }) => id === building.definition_id,
  );
  if (!definition?.buildable) {
    this.showFeedback(`${definition?.name ?? "That"} cannot be built`);
    return;
  }
  this.selectTool(definition.id);
  this.setOrientation(building.orientation);
  this.showFeedback(`Copied ${definition.name}`);
};

Runtime.prototype.buildingAt = function buildingAt(
  this: Runtime,
  coordinate: {
    q: number;
    r: number;
  },
): EntitySnapshot | undefined {
  return this.snapshot.buildings.findLast(({ footprint }) =>
    footprint.some(({ q, r }) => q === coordinate.q && r === coordinate.r),
  );
};

Runtime.prototype.setOrientation = function setOrientation(
  this: Runtime,
  next: number,
): void {
  this.orientation = next;
  required<HTMLElement>("orientation-value").textContent =
    `${DIRECTION_NAMES[this.orientation]} · R`;
  const definition =
    typeof this.tool === "number"
      ? this.host.definitions.buildings.find(({ id }) => id === this.tool)
      : undefined;
  this.renderer.setBuildFootprint(
    [
      ...(definition?.footprint ?? [{ q: 0, r: 0 }]),
      ...(definition?.service_envelope ?? []),
      ...(definition?.overhead_clearance ?? []),
    ],
    // Corner headings are closed under 60° rotation. Definitions remain single-cell until one
    // genuinely needs a wider footprint, so this is currently exact and future-proof.
    this.orientation >= this.NORTH
      ? this.orientation - this.NORTH
      : this.orientation,
  );
  // Placing a pole shows what it would light before it is paid for, which is the difference
  // between choosing where a pole goes and finding out afterwards.
  this.renderer.setBuildReach(definition ?? null);
  this.refreshHoverPreview();
};

Runtime.prototype.orientationRange = function orientationRange(
  this: Runtime,
  tool: Tool,
): {
  start: number;
  end: number;
} {
  const definition =
    typeof tool === "number"
      ? this.host.definitions.buildings.find(({ id }) => id === tool)
      : undefined;
  switch (definition?.orientation_axis) {
    case "corner":
      return { start: this.NORTH, end: DIRECTION_NAMES.length };
    case "any":
      return { start: 0, end: DIRECTION_NAMES.length };
    default:
      return { start: 0, end: this.NORTH };
  }
};

Runtime.prototype.orientationAllowed = function orientationAllowed(
  this: Runtime,
  tool: Tool,
  orientation: number,
): boolean {
  if (typeof tool !== "number") return true;
  const definition = this.host.definitions.buildings.find(
    ({ id }) => id === tool,
  );
  if (!definition || orientation < this.NORTH) return true;
  return (
    definition.corner_technology_id === undefined ||
    this.snapshot.researched.includes(definition.corner_technology_id)
  );
};

Runtime.prototype.rotateNewBuilding = function rotateNewBuilding(
  this: Runtime,
  step = 1,
): void {
  const { start, end } = this.orientationRange(this.tool);
  if (end - start === DIRECTION_NAMES.length) {
    let next = this.orientation;
    for (let press = 0; press < DIRECTION_NAMES.length; press += 1) {
      next = rotateAnyOrientation(next, step);
      if (this.orientationAllowed(this.tool, next)) break;
    }
    this.setOrientation(next);
    return;
  }
  // A tool with a single family stays inside it. `rotateHexDirection` still turns the six edges,
  // so the package keeps owning the geometry it knows.
  this.setOrientation(
    start === 0
      ? rotateHexDirection(this.orientation as HexDirection, step)
      : start +
          ((this.orientation - start + step + (end - start)) % (end - start)),
  );
};

Runtime.prototype.stopAiming = function stopAiming(this: Runtime): void {
  this.aimPointer = null;
  this.aimDegrees = null;
};

Runtime.prototype.sendAim = function sendAim(this: Runtime): void {
  if (!this.aimPointer) return;
  const target = this.renderer.pickWorld(this.aimPointer.x, this.aimPointer.y);
  const dx = target.x - this.snapshot.player.x;
  const dy = target.y - this.snapshot.player.y;
  if (dx === 0 && dy === 0) return;
  const degrees = Math.round((Math.atan2(dy, dx) * 180) / Math.PI);
  if (degrees === this.aimDegrees) return;
  // A full queue leaves the bearing unrecorded, so the next frame tries again.
  if (this.input.enqueue({ type: "aim", x: target.x, y: target.y }))
    this.aimDegrees = degrees;
};
