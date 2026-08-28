import "./boundaries.css";
import { axialToPixel, pixelToAxial } from "@hexlife/embed/hex";
import type { FactoryHost } from "../core/FactoryHost";
import type {
  BoundaryEdit,
  BoundaryPreview,
  FactorySnapshot,
  NativeInputCommand,
  WorldPoint,
} from "../core/types";
import type { FactoryRenderer } from "../rendering/FactoryRenderer";
import { WORLD_SCALE } from "../rendering/landmarks";

/** Picking is presentation; canonical edge identity and every transaction are native answers. */
export function nearestBoundaryDirection(
  cell: { q: number; r: number },
  point: WorldPoint,
): number {
  const center = axialToPixel(cell, WORLD_SCALE);
  const angle = Math.atan2(point.y - center.y, point.x - center.x);
  return ((Math.round(angle / (Math.PI / 3)) % 6) + 6) % 6;
}

/** A persistent, nonmodal construction tray. Controls are created once and patched in place. */
export class BoundaryTool {
  private opened = false;
  private start: { q: number; r: number } | null = null;
  private target: { q: number; r: number } | null = null;
  private choosingEnd = false;
  private preview: BoundaryPreview | null = null;
  private revision = 0;
  private pending = false;
  private requested = false;
  private snapshot: FactorySnapshot | null = null;
  private inventorySignature = "";
  private readonly panel: HTMLElement;
  private readonly opener: HTMLButtonElement;
  private readonly material: HTMLSelectElement;
  private readonly shape: HTMLSelectElement;
  private readonly direction: HTMLSelectElement;
  private readonly q: HTMLInputElement;
  private readonly r: HTMLInputElement;
  private readonly apply: HTMLButtonElement;
  private readonly status: HTMLElement;
  private readonly bill: HTMLElement;
  private readonly description: HTMLElement;

  constructor(
    root: HTMLElement,
    private readonly host: FactoryHost,
    private readonly renderer: FactoryRenderer,
    private readonly enqueue: (command: NativeInputCommand) => boolean,
    private readonly activate: () => void,
  ) {
    this.opener = document.getElementById(
      "open-boundaries",
    ) as HTMLButtonElement;
    this.panel = root;
    root.innerHTML = `
      <header><div><small>CONSTRUCTION · WOODWORK</small><h2 id="boundary-heading">Fences & gates</h2></div><button type="button" data-close aria-label="Close boundary tool">×</button></header>
      <div class="boundary-fields">
        <label>Action<select data-material></select></label>
        <label>Selection<select data-shape><option value="edge">Single edge</option><option value="area">Enclose an area</option></select></label>
      </div>
      <p data-description></p>
      <details class="boundary-precise"><summary>Precise placement</summary><div class="boundary-fields boundary-target">
        <label>Hex Q<input data-q type="number" step="1" min="-100000" max="100000" value="0"></label>
        <label>Hex R<input data-r type="number" step="1" min="-100000" max="100000" value="0"></label>
        <label>Edge<select data-direction><option value="0">East</option><option value="1">Southeast</option><option value="2">Southwest</option><option value="3">West</option><option value="4">Northwest</option><option value="5">Northeast</option></select></label>
      </div>
      </details>
      <p class="boundary-existing" data-existing></p>
      <p class="boundary-status" data-status role="status" aria-live="polite"></p>
      <p class="boundary-bill" data-bill></p>
      <div class="boundary-actions"><button type="button" data-apply disabled>Apply</button><button type="button" data-clear>New selection</button><button type="button" data-undo title="Undo the last boundary edit (Ctrl+Z while this tool is open)">Undo</button></div>
      <small class="boundary-help">Click near a hex edge. R changes its side. For an enclosure, choose two corner hexes, then Apply. Esc cancels a selection; Esc again exits. No materials are spent before Apply.</small>`;
    const get = <T extends HTMLElement>(selector: string): T =>
      root.querySelector<T>(selector)!;
    root.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        this.escape();
      }
    });
    this.material = get("[data-material]");
    for (const definition of host.definitions.boundaries) {
      this.material.add(new Option(definition.name, String(definition.id)));
    }
    for (const [value, name] of [
      ["open", "Open gate"],
      ["close", "Close gate"],
      ["remove", "Remove boundary"],
    ])
      this.material.add(new Option(name, value));
    this.shape = get("[data-shape]");
    this.direction = get("[data-direction]");
    this.q = get("[data-q]");
    this.r = get("[data-r]");
    this.apply = get("[data-apply]");
    this.status = get("[data-status]");
    this.bill = get("[data-bill]");
    this.description = get("[data-description]");
    this.opener.addEventListener("click", () =>
      this.opened ? this.close() : this.open(),
    );
    get("[data-close]").addEventListener("click", () => this.close());
    get("[data-clear]").addEventListener("click", () => this.clear());
    get<HTMLDetailsElement>(".boundary-precise").addEventListener(
      "toggle",
      (event) => {
        if (!(event.currentTarget as HTMLDetailsElement).open)
          this.panel.scrollTop = 0;
      },
    );
    get("[data-undo]").addEventListener("click", () => {
      this.enqueue({ type: "undo_boundary" });
    });
    this.material.addEventListener("change", () => {
      if (!this.areaAllowed()) this.shape.value = "edge";
      this.choosingEnd = false;
      this.refresh();
    });
    this.shape.addEventListener("change", () => this.clear());
    this.direction.addEventListener("change", () => this.refresh());
    for (const input of [this.q, this.r])
      input.addEventListener("input", () => {
        if (
          !this.q.validity.valid ||
          !this.r.validity.valid ||
          !this.q.value ||
          !this.r.value
        ) {
          this.revision += 1;
          this.preview = null;
          this.apply.disabled = true;
          this.status.textContent =
            "Enter whole hex coordinates between −100000 and 100000.";
          this.renderer.setBoundaryPreview(null);
          return;
        }
        this.start = { q: Number(this.q.value), r: Number(this.r.value) };
        this.target = this.start;
        this.choosingEnd = this.shape.value === "area";
        this.refresh();
      });
    this.apply.addEventListener("click", () => {
      const edit = this.edit();
      if (
        !edit ||
        !this.preview ||
        this.preview.error ||
        this.preview.changes === 0 ||
        this.pending
      )
        return;
      if (this.enqueue({ type: "boundary_edit", ...edit })) {
        this.preview = null;
        this.apply.disabled = true;
        this.status.textContent = "Applying boundary edit…";
      }
    });
  }

  get active(): boolean {
    return this.opened;
  }

  private areaAllowed(): boolean {
    const definition = this.host.definitions.boundaries.find(
      (d) => String(d.id) === this.material.value,
    );
    return (
      this.material.value === "remove" || (!!definition && !definition.gate)
    );
  }

  open(): void {
    this.activate();
    this.opened = true;
    this.panel.hidden = false;
    this.opener.setAttribute("aria-expanded", "true");
    this.opener.classList.add("active");
    this.renderer.setBuildMode(true);
    if (this.snapshot) {
      const cell = pixelToAxial(this.snapshot.player, WORLD_SCALE);
      this.q.value = String(cell.q);
      this.r.value = String(cell.r);
    }
    this.clear();
    this.material.focus();
  }

  close(restoreFocus = true): void {
    if (!this.opened) return;
    this.opened = false;
    this.panel.hidden = true;
    this.opener.setAttribute("aria-expanded", "false");
    this.opener.classList.remove("active");
    this.clear();
    this.renderer.setBuildMode(false);
    if (restoreFocus) this.opener.focus();
  }

  escape(): void {
    if (this.start) this.clear();
    else this.close();
  }

  selectRemoval(): void {
    this.material.value = "remove";
    this.refresh();
  }

  rotate(reverse: boolean): void {
    this.direction.value = String(
      (Number(this.direction.value) + (reverse ? 5 : 1)) % 6,
    );
    this.refresh();
  }

  clear(): void {
    this.start = null;
    this.target = null;
    this.choosingEnd = false;
    this.panel.scrollTop = 0;
    this.refresh();
  }

  pick(cell: { q: number; r: number }, point: WorldPoint): void {
    if (this.shape.value === "area" && this.choosingEnd) {
      this.target = cell;
      this.choosingEnd = false;
    } else {
      this.start = cell;
      this.target = cell;
      this.q.value = String(cell.q);
      this.r.value = String(cell.r);
      this.direction.value = String(nearestBoundaryDirection(cell, point));
      this.choosingEnd = this.shape.value === "area";
    }
    this.refresh();
  }

  hover(cell: { q: number; r: number }): void {
    if (
      !this.opened ||
      !this.choosingEnd ||
      (this.target?.q === cell.q && this.target.r === cell.r)
    )
      return;
    this.target = cell;
    this.refresh();
  }

  update(snapshot: FactorySnapshot): void {
    const signature = `${snapshot.player.x},${snapshot.player.y}:${JSON.stringify(snapshot.player.inventory)}`;
    const changed =
      this.snapshot?.boundaries !== snapshot.boundaries ||
      this.inventorySignature !== signature ||
      this.snapshot?.events !== snapshot.events;
    this.snapshot = snapshot;
    this.inventorySignature = signature;
    if (changed && this.opened && this.start) this.refresh();
  }

  private edit(): BoundaryEdit | null {
    if (!this.start || !this.target) return null;
    const action = this.material.value;
    return {
      ...this.start,
      to_q: this.target.q,
      to_r: this.target.r,
      direction: Number(this.direction.value),
      area: this.shape.value === "area",
      definition_id: Number(action) || 0,
      action:
        action === "open" || action === "close" || action === "remove"
          ? action
          : "build",
    };
  }

  private refresh(): void {
    this.revision += 1;
    this.preview = null;
    this.apply.disabled = true;
    this.apply.textContent = "Apply";
    this.bill.textContent = "";
    this.panel.querySelector<HTMLElement>("[data-existing]")!.textContent = "";
    this.shape.disabled = !this.areaAllowed();
    this.direction.disabled = this.shape.value === "area";
    const definition = this.host.definitions.boundaries.find(
      (d) => String(d.id) === this.material.value,
    );
    this.description.textContent =
      definition?.description ??
      (this.material.value === "remove"
        ? "Recover exactly the materials paid. Sandbox-built boundaries recover nothing. Buildings and deposits stay untouched."
        : "Manual gates use no power. A crossing must be clear of the player and live transport before it can close.");
    if (!this.start || !this.opened) {
      this.status.textContent =
        this.shape.value === "area"
          ? "Choose the first corner hex. Maximum 32 hexes per enclosure."
          : "Choose an edge on the map. Precise placement also accepts hex coordinates.";
      this.renderer.setBoundaryPreview(null);
      return;
    }
    this.status.textContent = "Checking native construction rules…";
    this.requested = true;
    if (!this.pending) void this.resolve();
  }

  private async resolve(): Promise<void> {
    this.pending = true;
    while (this.requested) {
      this.requested = false;
      const revision = this.revision;
      const edit = this.edit();
      if (!edit || !this.opened) continue;
      try {
        const preview = await this.host.boundaryPreview(edit);
        if (revision !== this.revision) continue;
        this.preview = preview;
        const edge = preview.edges.length === 1 ? preview.edges[0] : null;
        const existing =
          edge &&
          this.snapshot?.boundaries.find(
            (b) =>
              b.q === edge.q &&
              b.r === edge.r &&
              b.direction === edge.direction,
          );
        const definition =
          existing &&
          this.host.definitions.boundaries.find(
            (d) => d.id === existing.definition_id,
          );
        this.panel.querySelector<HTMLElement>("[data-existing]")!.textContent =
          existing
            ? `Current: ${definition?.name ?? "Boundary"}${definition?.gate ? (existing.open ? " · Open" : " · Closed") : ""}`
            : edge
              ? "Current: empty edge"
              : "";

        this.status.textContent =
          preview.error ??
          (this.choosingEnd
            ? `Choose the second corner. Preview: ${preview.edges.length} perimeter edges.`
            : preview.changes === 0
              ? "Already matches this selection. Nothing to spend or recover."
              : `${preview.changes} edge${preview.changes === 1 ? "" : "s"} will change. Hex ${edit.q}, ${edit.r}${edit.area ? ` → ${edit.to_q}, ${edit.to_r}` : ` · ${this.direction.selectedOptions[0]?.text} edge`}. Floor space stays free.`);
        this.status.classList.toggle("blocked", !!preview.error);
        const names = (items: BoundaryPreview["cost"], owned = false): string =>
          items
            .map(
              (i) =>
                `${i.quantity} ${this.host.definitions.items.find((d) => d.id === i.item_id)?.name ?? "items"}${owned ? ` (have ${this.snapshot?.player.inventory[i.item_id] ?? 0})` : ""}`,
            )
            .join(" + ");
        this.bill.textContent = `${preview.cost.length ? `Use ${names(preview.cost, true)}` : this.snapshot?.player.creative ? "Creative mode · materials are free" : "No materials needed"}${preview.refund.length ? ` · Recover ${names(preview.refund)}` : ""}`;
        this.apply.textContent =
          this.material.value === "remove"
            ? "Remove selection"
            : this.material.value === "open"
              ? "Open gate"
              : this.material.value === "close"
                ? "Close gate"
                : `Build ${preview.changes || "selection"}`;
        this.renderer.setBoundaryPreview(preview);
      } catch (error) {
        if (revision === this.revision) {
          this.status.textContent = `Preview unavailable: ${String(error)}`;
          this.renderer.setBoundaryPreview(null);
        }
      }
    }
    this.pending = false;
    this.apply.disabled =
      !this.preview ||
      !!this.preview.error ||
      this.preview.changes === 0 ||
      this.choosingEnd;
  }
}
