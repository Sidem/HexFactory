import "./ground.css";
import type { FactoryHost } from "../core/FactoryHost";
import type {
  FactorySnapshot,
  GroundPreview,
  Ingredient,
  NativeInputCommand,
} from "../core/types";
import { UNTREATED_MOVEMENT } from "../core/definitions";
import { part, syncChildren } from "./dom";
import type { FactoryRenderer } from "../rendering/FactoryRenderer";
import {
  brushLine,
  groundBrushEdit,
  type BrushHex,
  type GroundBrushMode,
} from "./groundBrush";

const MODES: readonly {
  mode: GroundBrushMode;
  icon: string;
  label: string;
  hint: string;
}[] = [
  {
    mode: "grade",
    icon: "≈",
    label: "Grade",
    hint: "Sample a good height, then blend nearby ground into a walkable slope.",
  },
  {
    mode: "surface",
    icon: "▦",
    label: "Surface",
    hint: "Paint the selected road or paving material directly onto the ground.",
  },
  {
    mode: "strip",
    icon: "⌫",
    label: "Strip",
    hint: "Brush a prepared surface away and recover exactly what it cost.",
  },
];

interface BrushStroke {
  readonly pointerId: number;
  readonly datum: BrushHex;
  readonly painted: Set<string>;
  last: BrushHex;
}

/**
 * The ground brush exposes the result a player wants, not six native operations. Grade samples the
 * pressed height and blends toward it; Surface and Strip paint immediately. Every stamp is still one
 * bounded native transaction, so quantities, range, spoil, obstacles and undo remain simulation truth.
 */
export class GroundTool {
  private opened = false;
  private mode: GroundBrushMode = "grade";
  /** 0, 1 and 2 mean 1, 7 and 19 affected hexes per stamp. */
  private radius = 1;
  private surface: number;
  private cover = false;
  private brush: BrushStroke | null = null;
  /** Where the last stroke finished, while its report still owns the status line. */
  private strokeEnd: BrushHex | null = null;
  private hovered: BrushHex | null = null;
  private preview: GroundPreview | null = null;
  private revision = 0;
  private pending = false;
  private requested = false;
  private snapshot: FactorySnapshot | null = null;
  private inventorySignature = "";
  private lastMessage = "";
  private readonly panel: HTMLElement;
  private readonly opener: HTMLButtonElement;
  private readonly modes: HTMLElement;
  private readonly palette: HTMLElement;
  private readonly sizes: HTMLElement;
  private readonly status: HTMLElement;
  private readonly spoilValue: HTMLElement;
  private readonly spoilFill: HTMLElement;
  private readonly spoilGauge: HTMLElement;
  private readonly coverBox: HTMLElement;
  private readonly coverInput: HTMLInputElement;

  constructor(
    root: HTMLElement,
    private readonly host: FactoryHost,
    private readonly renderer: FactoryRenderer,
    private readonly enqueue: (command: NativeInputCommand) => boolean,
    private readonly activate: () => void,
  ) {
    this.opener = document.getElementById("open-ground") as HTMLButtonElement;
    this.panel = root;
    this.surface = host.definitions.surfaces[0]?.id ?? 0;
    root.innerHTML = `
      <header><div><small>CONSTRUCTION · GROUND BRUSH</small><h2 id="ground-heading">Shape the ground</h2></div><button type="button" data-close aria-label="Close ground brush">×</button></header>
      <div class="ground-modes" role="group" aria-label="What the brush paints">${MODES.map(
        ({ mode, icon, label, hint }) =>
          `<button type="button" data-mode="${mode}" aria-pressed="false" title="${hint}"><span aria-hidden="true">${icon}</span><b>${label}</b><small>${hint}</small></button>`,
      ).join("")}</div>
      <div class="ground-palette" role="group" aria-label="Surface material" data-palette hidden></div>
      <label class="ground-cover" data-cover hidden><input type="checkbox" data-cover-input><span><b>Cover deposits</b>Allow this surface stroke to seal resource fields. Stripping uncovers them unchanged.</span></label>
      <div class="ground-size" role="group" aria-label="Brush size"><span>Brush size</span>${[
        [0, "1 hex"],
        [1, "7 hexes"],
        [2, "19 hexes"],
      ]
        .map(
          ([radius, label]) =>
            `<button type="button" data-radius="${radius}" aria-pressed="false">${label}</button>`,
        )
        .join("")}</div>
      <div class="ground-spoil"><span>Spoil heap</span><span class="ground-gauge"><i data-spoil-fill style="width:0%"></i></span><b data-spoil>0</b></div>
      <p class="ground-status" data-status role="status" aria-live="polite"></p>
      <div class="ground-panel-actions"><button type="button" data-undo title="Undo the last brush stamp (Ctrl+Z while this tool is open)">Undo last patch</button></div>
      <small class="ground-help"><b>Press and drag directly on the world.</b> Grade samples the height under the first press, then makes every painted patch reachable from it. [ and ] change brush size; R cycles Grade, Surface and Strip. Nothing waits for an Apply button.</small>`;
    const get = <T extends HTMLElement>(selector: string): T =>
      root.querySelector<T>(selector)!;
    this.modes = get(".ground-modes");
    this.palette = get("[data-palette]");
    this.sizes = get(".ground-size");
    this.status = get("[data-status]");
    this.spoilValue = get("[data-spoil]");
    this.spoilFill = get("[data-spoil-fill]");
    this.spoilGauge = this.spoilFill.parentElement as HTMLElement;
    this.coverBox = get("[data-cover]");
    this.coverInput = get("[data-cover-input]");
    this.opener.addEventListener("click", () =>
      this.opened ? this.close() : this.open(),
    );
    get("[data-close]").addEventListener("click", () => this.close());
    get("[data-undo]").addEventListener("click", () =>
      this.enqueue({ type: "undo_ground" }),
    );
    this.modes.addEventListener("click", (event) => {
      const mode = (event.target as HTMLElement).closest<HTMLElement>(
        "[data-mode]",
      )?.dataset.mode as GroundBrushMode | undefined;
      if (mode) this.selectMode(mode);
    });
    this.sizes.addEventListener("click", (event) => {
      const radius = (event.target as HTMLElement).closest<HTMLElement>(
        "[data-radius]",
      )?.dataset.radius;
      if (radius !== undefined) this.setRadius(Number(radius));
    });
    this.coverInput.addEventListener("change", () => {
      this.cover = this.coverInput.checked;
      this.refreshPreview();
    });
    this.buildPalette();
    this.syncControls();
  }

  get active(): boolean {
    return this.opened;
  }

  open(): void {
    this.activate();
    this.opened = true;
    this.panel.hidden = false;
    this.opener.setAttribute("aria-expanded", "true");
    this.opener.classList.add("active");
    this.renderer.setBuildMode(true);
    this.syncControls();
    this.modes.querySelector<HTMLButtonElement>("[data-mode=grade]")?.focus();
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
    if (this.brush) this.clear();
    else this.close();
  }

  clear(): void {
    this.brush = null;
    this.strokeEnd = null;
    this.hovered = null;
    this.refreshPreview();
    this.status.classList.remove("blocked");
    this.status.textContent = this.instruction();
  }

  cycleAction(reverse: boolean): void {
    const index = MODES.findIndex(({ mode }) => mode === this.mode);
    const next = (index + (reverse ? MODES.length - 1 : 1)) % MODES.length;
    this.selectMode(MODES[next]!.mode);
  }

  /** `[` and `]` widen and narrow the brush, which is the only size this tool has. */
  cycleSize(reverse: boolean): void {
    this.setRadius(this.radius + (reverse ? -1 : 1));
  }

  selectStrip(): void {
    this.selectMode("strip");
  }

  /**
   * The footprint under the cursor, priced by the same native transaction a press would commit. A
   * brush whose reach is invisible until it fires is guesswork, so this follows the pointer rather
   * than waiting for a click, and stands down mid-stroke where each stamp is already showing itself.
   */
  hover(cell: BrushHex): void {
    if (!this.opened || this.brush) return;
    if (this.hovered?.q === cell.q && this.hovered.r === cell.r) return;
    // Aiming somewhere new is what ends the last stroke's report. The hex the stroke finished on is
    // not somewhere new, and that is exactly where the footprint is redrawn the moment it ends.
    if (
      this.strokeEnd &&
      (this.strokeEnd.q !== cell.q || this.strokeEnd.r !== cell.r)
    )
      this.strokeEnd = null;
    this.hovered = { ...cell };
    this.refreshPreview();
  }

  beginBrush(pointerId: number, cell: BrushHex): boolean {
    if (!this.opened) return false;
    // The stroke changes the ground under the drawn footprint, so the standing picture of untouched
    // ground goes with the press rather than lingering a frame behind the first stamp.
    this.strokeEnd = null;
    this.hovered = null;
    this.refreshPreview();
    this.brush = {
      pointerId,
      datum: { ...cell },
      last: { ...cell },
      painted: new Set(),
    };
    if (this.mode === "grade") {
      this.status.classList.remove("blocked");
      this.status.textContent = `Height sampled at ${cell.q}, ${cell.r}. Keep dragging to blend the slope.`;
    } else {
      this.paintCentre(cell);
    }
    return true;
  }

  paintBrush(pointerId: number, cell: BrushHex): boolean {
    const brush = this.brush;
    if (!brush || brush.pointerId !== pointerId) return false;
    for (const centre of brushLine(brush.last, cell).slice(1)) {
      if (!this.paintCentre(centre)) break;
      brush.last = centre;
    }
    return true;
  }

  endBrush(pointerId: number): boolean {
    if (!this.brush || this.brush.pointerId !== pointerId) return false;
    const { painted, last } = this.brush;
    this.brush = null;
    this.strokeEnd = painted.size > 0 ? { ...last } : null;
    if (painted.size > 0)
      this.status.textContent = `Stroke complete · ${painted.size} brush patch${painted.size === 1 ? "" : "es"}. Ctrl+Z undoes the last patch.`;
    // The pointer is still where the stroke ended and will not move on its own, so the footprint is
    // asked for here rather than waiting for a gesture that may never come.
    this.hover(last);
    return true;
  }

  update(snapshot: FactorySnapshot): void {
    const inventorySignature = `${snapshot.player.creative}:${snapshot.researched.join(",")}:${JSON.stringify(snapshot.player.inventory)}`;
    this.snapshot = snapshot;
    // Snapshots arrive every frame and a preview is answered once: the projected heap a player is
    // reading has to survive the frames between, or the number it shows would flicker back.
    this.drawSpoil(snapshot.spoil, this.preview?.spoil ?? snapshot.spoil);
    if (inventorySignature !== this.inventorySignature) {
      this.inventorySignature = inventorySignature;
      this.buildPalette();
    }
    const message = snapshot.events.at(-1);
    if (!this.opened || !message || message === this.lastMessage) return;
    this.lastMessage = message;
    this.status.textContent = message;
    this.status.classList.toggle(
      "blocked",
      !/^(Graded|Prepared|Undid|Water found)/.test(message),
    );
  }

  /** Drop the standing picture and ask for the one the cursor is now over, newest answer wins. */
  private refreshPreview(): void {
    this.revision += 1;
    this.preview = null;
    if (!this.opened || !this.hovered) {
      this.renderer.setGroundPreview(null);
      this.drawSpoil(this.snapshot?.spoil ?? 0);
      return;
    }
    this.requested = true;
    if (!this.pending) void this.resolve();
  }

  private async resolve(): Promise<void> {
    this.pending = true;
    while (this.requested) {
      this.requested = false;
      const revision = this.revision;
      const cell = this.hovered;
      if (!cell || !this.opened) continue;
      // A hovering grade brush samples the hex it is over, because pressing there is exactly what
      // would make that hex the datum. The picture is the one the press would produce.
      const edit = groundBrushEdit(
        cell,
        cell,
        this.radius,
        this.mode,
        this.surface,
        this.cover,
      );
      try {
        const preview = await this.host.groundPreview(edit);
        if (revision !== this.revision) continue;
        this.preview = preview;
        this.renderer.setGroundPreview(preview);
        this.describe(preview);
      } catch (error) {
        if (revision !== this.revision) continue;
        this.renderer.setGroundPreview(null);
        this.status.textContent = `Preview unavailable: ${String(error)}`;
        this.status.classList.add("blocked");
      }
    }
    this.pending = false;
  }

  /** What this one stamp would do, in the order a player asks it: how much, what it moves, what it costs. */
  private describe(preview: GroundPreview): void {
    this.drawSpoil(this.snapshot?.spoil ?? 0, preview.spoil);
    if (preview.error) {
      this.status.textContent = preview.error;
      this.status.classList.add("blocked");
      return;
    }
    // The stroke that just finished is the more useful answer where the cursor still sits, so the
    // redrawn footprint keeps its picture and its heap but leaves that sentence standing.
    if (this.strokeEnd) return;
    this.status.classList.remove("blocked");
    if (preview.changes === 0) {
      this.status.textContent = this.instruction();
      return;
    }
    const label =
      MODES.find(({ mode }) => mode === this.mode)?.label ?? "Brush";
    const earth =
      preview.cut || preview.fill
        ? ` · cut ${preview.cut}, fill ${preview.fill}`
        : "";
    const bill = preview.cost.length
      ? ` · ${this.names(preview.cost, true)}`
      : preview.refund.length
        ? ` · recovers ${this.names(preview.refund)}`
        : "";
    const passed = preview.blocked
      ? ` · ${preview.blocked} passed over`
      : preview.retaining
        ? ` · ${preview.retaining} edge${preview.retaining === 1 ? "" : "s"} still too steep — brush wider`
        : "";
    this.status.textContent = `${label} ${preview.changes} hex${preview.changes === 1 ? "" : "es"}${earth}${bill}${passed}.`;
  }

  private paintCentre(centre: BrushHex): boolean {
    const brush = this.brush;
    if (!brush) return false;
    const key = `${centre.q},${centre.r}`;
    if (brush.painted.has(key)) return true;
    const edit = groundBrushEdit(
      centre,
      brush.datum,
      this.radius,
      this.mode,
      this.surface,
      this.cover,
    );
    if (!this.enqueue({ type: "ground_edit", ...edit })) {
      this.status.textContent =
        "The brush is catching up… keep moving slowly over this patch.";
      return false;
    }
    brush.painted.add(key);
    this.status.classList.remove("blocked");
    this.status.textContent = `${this.mode === "grade" ? "Blending" : this.mode === "surface" ? "Surfacing" : "Stripping"} around ${centre.q}, ${centre.r}…`;
    return true;
  }

  private selectMode(mode: GroundBrushMode): void {
    this.brush = null;
    this.strokeEnd = null;
    this.mode = mode;
    this.syncControls();
  }

  private setRadius(radius: number): void {
    this.radius = Math.max(0, Math.min(2, radius));
    this.syncControls();
  }

  private syncControls(): void {
    for (const button of this.modes.querySelectorAll<HTMLElement>(
      "[data-mode]",
    ))
      button.setAttribute(
        "aria-pressed",
        String(button.dataset.mode === this.mode),
      );
    for (const button of this.sizes.querySelectorAll<HTMLElement>(
      "[data-radius]",
    ))
      button.setAttribute(
        "aria-pressed",
        String(Number(button.dataset.radius) === this.radius),
      );
    this.palette.hidden = this.mode !== "surface";
    this.coverBox.hidden = this.mode !== "surface";
    this.status.classList.remove("blocked");
    this.status.textContent = this.instruction();
    // Every control here changes what the footprint under the cursor would do, so each of them
    // re-asks native rather than leaving the drawn picture describing the previous setting.
    this.refreshPreview();
  }

  private instruction(): string {
    if (this.mode === "grade")
      return "Press on a good height, then drag across rough ground. The brush blends a walkable grade as you move.";
    if (this.mode === "strip")
      return "Press and drag to lift prepared surfaces. Recovered material returns to your pack.";
    const name = this.host.definitions.surfaces.find(
      ({ id }) => id === this.surface,
    )?.name;
    return `Press and drag to paint ${name?.toLowerCase() ?? "the selected surface"}.`;
  }

  private names(items: readonly Ingredient[], owned = false): string {
    return items
      .map((item) => {
        const name =
          this.host.definitions.items.find((entry) => entry.id === item.item_id)
            ?.name ?? "items";
        const have = this.snapshot?.player.inventory[item.item_id] ?? 0;
        return `${item.quantity} ${name}${owned ? ` (have ${have})` : ""}`;
      })
      .join(" + ");
  }

  private drawSpoil(spoil: number, projected = spoil): void {
    const scale = Math.max(spoil, projected, 12);
    this.spoilValue.textContent =
      projected === spoil ? String(spoil) : `${spoil} → ${projected}`;
    this.spoilFill.style.width = `${Math.round((projected / scale) * 100)}%`;
    this.spoilGauge.classList.toggle("empty", projected === 0);
  }

  private buildPalette(): void {
    const inventory = this.snapshot?.player.inventory ?? {};
    const creative = this.snapshot?.player.creative === true;
    const surfaces = this.host.definitions.surfaces;
    const buttons = syncChildren(
      this.palette,
      surfaces.map(({ id }) => String(id)),
      (key) => {
        const button = document.createElement("button");
        button.type = "button";
        button.dataset.surface = key;
        button.innerHTML =
          '<span class="ground-material-name"></span><span class="ground-pace"></span><small class="ground-price"></small><small class="ground-material-hint"></small>';
        button.addEventListener("click", () => {
          this.surface = Number(key);
          this.mode = "surface";
          this.buildPalette();
          this.syncControls();
        });
        return button;
      },
    );
    surfaces.forEach((surface, index) => {
      const button = buttons[index]!;
      button.title = surface.description;
      button.setAttribute("aria-pressed", String(surface.id === this.surface));
      part(button, ".ground-material-name").textContent = surface.name;
      part(button, ".ground-pace").textContent =
        `+${surface.movement - UNTREATED_MOVEMENT}% pace`;
      const technology = this.host.technologies.technologies.find(
        ({ id }) => id === surface.unlock_technology_id,
      );
      const locked =
        !creative &&
        surface.unlock_technology_id !== undefined &&
        !this.snapshot?.researched.includes(surface.unlock_technology_id);
      const base = surfaces.find(({ id }) => id === surface.base_surface_id);
      part(button, ".ground-material-hint").textContent = locked
        ? `Research ${technology?.name ?? "the required technology"}`
        : base
          ? `Needs ${base.name.toLowerCase()} first`
          : "";
      button.classList.toggle("locked", locked);
      const short = surface.construction_cost.some(
        ({ item_id, quantity }) => (inventory[item_id] ?? 0) < quantity,
      );
      const price = part(button, ".ground-price");
      price.classList.toggle("short", short && !creative);
      price.textContent = creative
        ? "Free · creative mode"
        : surface.construction_cost.length
          ? `${this.names(surface.construction_cost, true)} / hex`
          : "No materials needed";
    });
  }
}
