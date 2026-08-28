import "./ground.css";
import { pixelToAxial } from "@hexlife/embed/hex";
import type { FactoryHost } from "../core/FactoryHost";
import type {
  FactorySnapshot,
  GroundAction,
  GroundEdit,
  GroundPreview,
  GroundShape,
  Ingredient,
  NativeInputCommand,
} from "../core/types";
import { UNTREATED_MOVEMENT } from "../core/definitions";
import { part, syncChildren } from "./dom";
import type { FactoryRenderer } from "../rendering/FactoryRenderer";
import { WORLD_SCALE } from "../rendering/landmarks";

interface ActionSpec {
  readonly action: GroundAction;
  readonly icon: string;
  readonly label: string;
  readonly hint: string;
  /** What the Apply button promises. `{n}` is the number of hexes that would actually change. */
  readonly verb: string;
}

/**
 * The five verbs, in the order a yard is actually built: level the site, cut and fill it square,
 * then surface it. `clear` sits beside `pave` because taking a surface up is the same decision as
 * laying one down, made the other way.
 */
const ACTIONS: readonly ActionSpec[] = [
  {
    action: "pave",
    icon: "▦",
    label: "Pave",
    hint: "Lay a surface. Walking across it is faster, and it can go over anything already standing.",
    verb: "Pave {n}",
  },
  {
    action: "clear",
    icon: "⌫",
    label: "Strip",
    hint: "Take the surface back up and recover exactly what it cost. The grade underneath is untouched.",
    verb: "Strip {n}",
  },
  {
    action: "raise",
    icon: "▲",
    label: "Raise",
    hint: "Fill one step, using spoil from ground cut elsewhere. Nothing is raised out of nothing.",
    verb: "Raise {n}",
  },
  {
    action: "lower",
    icon: "▼",
    label: "Lower",
    hint: "Cut one step. What comes out goes on the spoil heap, ready to fill somewhere else.",
    verb: "Lower {n}",
  },
  {
    action: "level",
    icon: "═",
    label: "Level",
    hint: "Even every selected hex onto the grade of the first one picked, cutting and filling to match.",
    verb: "Level {n}",
  },
];

const SHAPES: readonly { value: GroundShape; label: string }[] = [
  { value: "cell", label: "One hex" },
  { value: "path", label: "Line of hexes" },
  { value: "area", label: "Rectangle of hexes" },
];

/**
 * A persistent, nonmodal earthworks tray. Every number in it is a native answer to the exact edit
 * the Apply button would send: the preview and the commit are one transaction, so what the tray
 * quotes is what the world charges.
 */
export class GroundTool {
  private opened = false;
  private action: GroundAction = "pave";
  private surface: number;
  private cover = false;
  private start: { q: number; r: number } | null = null;
  private target: { q: number; r: number } | null = null;
  private choosingEnd = false;
  private preview: GroundPreview | null = null;
  private revision = 0;
  private pending = false;
  private requested = false;
  private snapshot: FactorySnapshot | null = null;
  private inventorySignature = "";
  private readonly panel: HTMLElement;
  private readonly opener: HTMLButtonElement;
  private readonly actions: HTMLElement;
  private readonly palette: HTMLElement;
  private readonly shape: HTMLSelectElement;
  private readonly q: HTMLInputElement;
  private readonly r: HTMLInputElement;
  private readonly toQ: HTMLInputElement;
  private readonly toR: HTMLInputElement;
  private readonly apply: HTMLButtonElement;
  private readonly status: HTMLElement;
  private readonly bill: HTMLElement;
  private readonly hint: HTMLElement;
  private readonly spoilValue: HTMLElement;
  private readonly spoilFill: HTMLElement;
  private readonly spoilGauge: HTMLElement;
  private readonly move: HTMLElement;
  private readonly coverBox: HTMLElement;
  private readonly coverInput: HTMLInputElement;
  private readonly coverText: HTMLElement;
  private readonly retaining: HTMLElement;

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
      <header><div><small>CONSTRUCTION · EARTHWORKS</small><h2 id="ground-heading">Ground works</h2></div><button type="button" data-close aria-label="Close ground works">×</button></header>
      <div class="ground-actions" role="group" aria-label="Ground work to carry out">${ACTIONS.map(
        (spec) =>
          `<button type="button" data-action="${spec.action}" aria-pressed="false" title="${spec.hint}"><span aria-hidden="true">${spec.icon}</span>${spec.label}</button>`,
      ).join("")}</div>
      <div class="ground-palette" role="group" aria-label="Surface material" data-palette></div>
      <p data-hint></p>
      <div class="ground-fields"><label>Selection<select data-shape>${SHAPES.map(
        (shape) => `<option value="${shape.value}">${shape.label}</option>`,
      ).join(
        "",
      )}</select></label><label>Grade limit<input value="±3 steps" readonly tabindex="-1" aria-label="Each hex may be cut or filled at most three steps from its own natural grade"></label></div>
      <details class="ground-precise"><summary>Precise selection</summary><div class="ground-fields ground-target">
        <label>From Q<input data-q type="number" step="1" min="-100000" max="100000" value="0"></label>
        <label>From R<input data-r type="number" step="1" min="-100000" max="100000" value="0"></label>
        <label>To Q<input data-to-q type="number" step="1" min="-100000" max="100000" value="0"></label>
        <label>To R<input data-to-r type="number" step="1" min="-100000" max="100000" value="0"></label>
      </div></details>
      <div class="ground-spoil"><span>Spoil heap</span><span class="ground-gauge"><i data-spoil-fill style="width: 0%"></i></span><b data-spoil>0</b></div>
      <p class="ground-move" data-move hidden></p>
      <p class="ground-status" data-status role="status" aria-live="polite"></p>
      <p class="ground-bill" data-bill></p>
      <p class="ground-retaining" data-retaining hidden></p>
      <label class="ground-cover" data-cover hidden><input type="checkbox" data-cover-input><span data-cover-text></span></label>
      <div class="ground-panel-actions"><button type="button" data-apply disabled>Apply</button><button type="button" data-clear>New selection</button><button type="button" data-undo title="Undo the last ground works edit (Ctrl+Z while this tool is open)">Undo</button></div>
      <small class="ground-help">Click a hex to start. For a line or a rectangle, click again to finish it. R cycles the work, Shift+R goes back, Delete jumps to Strip. Esc cancels a selection; Esc again exits. Nothing is spent, dug or recovered before Apply.</small>`;
    const get = <T extends HTMLElement>(selector: string): T =>
      root.querySelector<T>(selector)!;
    root.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        this.escape();
      }
    });
    this.actions = get(".ground-actions");
    this.palette = get("[data-palette]");
    this.shape = get("[data-shape]");
    this.q = get("[data-q]");
    this.r = get("[data-r]");
    this.toQ = get("[data-to-q]");
    this.toR = get("[data-to-r]");
    this.apply = get("[data-apply]");
    this.status = get("[data-status]");
    this.bill = get("[data-bill]");
    this.hint = get("[data-hint]");
    this.spoilValue = get("[data-spoil]");
    this.spoilFill = get("[data-spoil-fill]");
    this.spoilGauge = this.spoilFill.parentElement as HTMLElement;
    this.move = get("[data-move]");
    this.coverBox = get("[data-cover]");
    this.coverInput = get("[data-cover-input]");
    this.coverText = get("[data-cover-text]");
    this.retaining = get("[data-retaining]");
    this.buildPalette();
    this.opener.addEventListener("click", () =>
      this.opened ? this.close() : this.open(),
    );
    get("[data-close]").addEventListener("click", () => this.close());
    get("[data-clear]").addEventListener("click", () => this.clear());
    get<HTMLDetailsElement>(".ground-precise").addEventListener(
      "toggle",
      (event) => {
        if (!(event.currentTarget as HTMLDetailsElement).open)
          this.panel.scrollTop = 0;
      },
    );
    get("[data-undo]").addEventListener("click", () => {
      this.enqueue({ type: "undo_ground" });
    });
    this.actions.addEventListener("click", (event) => {
      const action = (event.target as HTMLElement).closest<HTMLElement>(
        "[data-action]",
      )?.dataset.action;
      if (action) this.selectAction(action as GroundAction);
    });
    this.shape.addEventListener("change", () => this.clear());
    for (const input of [this.q, this.r, this.toQ, this.toR])
      input.addEventListener("input", () => this.readCoordinates());
    // The acknowledgement is per selection, not per session: it is re-asked the moment the edit it
    // was given for stops being the edit on the table.
    this.coverInput.addEventListener("change", () => {
      this.cover = this.coverInput.checked;
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
      if (this.enqueue({ type: "ground_edit", ...edit })) {
        this.preview = null;
        this.apply.disabled = true;
        this.status.textContent = "Working the ground…";
      }
    });
    this.selectAction("pave");
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
    if (this.snapshot) {
      const cell = pixelToAxial(this.snapshot.player, WORLD_SCALE);
      for (const input of [this.q, this.toQ]) input.value = String(cell.q);
      for (const input of [this.r, this.toR]) input.value = String(cell.r);
    }
    this.clear();
    this.panel.querySelector<HTMLButtonElement>("[data-action]")?.focus();
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

  /** `R` walks the five verbs. Shift walks them backwards, the way rotation already does. */
  cycleAction(reverse: boolean): void {
    const index = ACTIONS.findIndex((spec) => spec.action === this.action);
    const next = (index + (reverse ? ACTIONS.length - 1 : 1)) % ACTIONS.length;
    this.selectAction(ACTIONS[next]!.action);
  }

  selectStrip(): void {
    this.selectAction("clear");
  }

  clear(): void {
    this.start = null;
    this.target = null;
    this.choosingEnd = false;
    this.cover = false;
    this.coverInput.checked = false;
    this.panel.scrollTop = 0;
    this.refresh();
  }

  pick(cell: { q: number; r: number }): void {
    if (this.shape.value !== "cell" && this.choosingEnd) {
      this.target = cell;
      this.choosingEnd = false;
      this.toQ.value = String(cell.q);
      this.toR.value = String(cell.r);
    } else {
      // A fresh selection is a fresh question, so a covering already agreed to does not carry over.
      this.cover = false;
      this.coverInput.checked = false;
      this.start = cell;
      this.target = cell;
      this.q.value = String(cell.q);
      this.r.value = String(cell.r);
      this.toQ.value = String(cell.q);
      this.toR.value = String(cell.r);
      this.choosingEnd = this.shape.value !== "cell";
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
    this.toQ.value = String(cell.q);
    this.toR.value = String(cell.r);
    this.refresh();
  }

  update(snapshot: FactorySnapshot): void {
    const signature = `${snapshot.player.x},${snapshot.player.y}:${JSON.stringify(snapshot.player.inventory)}`;
    const changed =
      this.snapshot?.ground !== snapshot.ground ||
      this.snapshot?.spoil !== snapshot.spoil ||
      this.snapshot?.researched !== snapshot.researched ||
      this.snapshot?.player.creative !== snapshot.player.creative ||
      this.inventorySignature !== signature ||
      this.snapshot?.events !== snapshot.events;
    this.snapshot = snapshot;
    this.inventorySignature = signature;
    // Snapshots arrive every frame and a preview is answered once. Redrawing the heap from the
    // snapshot alone would erase the projection a frame after it appeared, which is the one number
    // a player reads before committing a cut: keep the standing preview's answer while it stands.
    this.drawSpoil(snapshot.spoil, this.preview?.spoil ?? snapshot.spoil);
    if (changed && this.opened) {
      this.buildPalette();
      if (this.start) this.refresh();
    }
  }

  private selectAction(action: GroundAction): void {
    this.action = action;
    this.cover = false;
    this.coverInput.checked = false;
    for (const button of this.actions.querySelectorAll<HTMLElement>(
      "[data-action]",
    ))
      button.setAttribute(
        "aria-pressed",
        String(button.dataset.action === action),
      );
    this.palette.hidden = action !== "pave";
    this.hint.textContent =
      ACTIONS.find((spec) => spec.action === action)?.hint ?? "";
    this.refresh();
  }

  /**
   * The shelf of surfaces, priced against what is actually in the pack. Affordability is redrawn
   * whenever the pack changes so the shelf never quotes a price the player can no longer meet.
   */
  private buildPalette(): void {
    const inventory = this.snapshot?.player.inventory ?? {};
    const creative = this.snapshot?.player.creative === true;
    const surfaces = this.host.definitions.surfaces;
    const buttons = syncChildren(
      this.palette,
      surfaces.map((surface) => String(surface.id)),
      (key) => {
        const button = document.createElement("button");
        button.type = "button";
        button.dataset.surface = key;
        button.innerHTML =
          '<span class="ground-material-name"></span><span class="ground-pace"></span><small class="ground-price"></small><small class="ground-material-hint"></small>';
        button.addEventListener("click", () => {
          this.surface = Number(key);
          this.cover = false;
          this.coverInput.checked = false;
          this.buildPalette();
          this.refresh();
        });
        return button;
      },
    );
    surfaces.forEach((surface, index) => {
      const button = buttons[index]!;
      button.title = surface.description;
      button.setAttribute("aria-pressed", String(surface.id === this.surface));
      const name = part(button, ".ground-material-name");
      name.textContent = surface.name;
      const pace = part(button, ".ground-pace");
      pace.textContent = `+${surface.movement - UNTREATED_MOVEMENT}% pace`;
      const price = part(button, ".ground-price");
      const technology = this.host.technologies.technologies.find(
        (technology) => technology.id === surface.unlock_technology_id,
      );
      const locked =
        !creative &&
        surface.unlock_technology_id !== undefined &&
        !this.snapshot?.researched.includes(surface.unlock_technology_id);
      const base = surfaces.find((base) => base.id === surface.base_surface_id);
      part(button, ".ground-material-hint").textContent = locked
        ? `Research ${technology?.name ?? "the required technology"}`
        : base
          ? `Lay ${base.name.toLowerCase()} first · base recovered on stripping`
          : "";
      button.classList.toggle("locked", locked);
      const short = surface.construction_cost.some(
        (item) => (inventory[item.item_id] ?? 0) < item.quantity,
      );
      price.classList.toggle("short", short && !creative);
      price.textContent = creative
        ? "Free · creative mode"
        : surface.construction_cost.length
          ? `${this.names(surface.construction_cost, true)} per hex`
          : "No materials needed";
    });
  }

  private readCoordinates(): void {
    const inputs = [this.q, this.r, this.toQ, this.toR];
    if (inputs.some((input) => !input.validity.valid || !input.value)) {
      this.revision += 1;
      this.preview = null;
      this.apply.disabled = true;
      this.status.textContent =
        "Enter whole hex coordinates between −100000 and 100000.";
      this.renderer.setGroundPreview(null);
      return;
    }
    this.start = { q: Number(this.q.value), r: Number(this.r.value) };
    this.target =
      this.shape.value === "cell"
        ? this.start
        : { q: Number(this.toQ.value), r: Number(this.toR.value) };
    this.choosingEnd = false;
    this.refresh();
  }

  private edit(): GroundEdit | null {
    if (!this.start || !this.target) return null;
    const target = this.shape.value === "cell" ? this.start : this.target;
    return {
      ...this.start,
      to_q: target.q,
      to_r: target.r,
      shape: this.shape.value as GroundShape,
      definition_id: this.surface,
      action: this.action,
      cover: this.cover,
    };
  }

  private names(items: readonly Ingredient[], owned = false): string {
    return items
      .map((item) => {
        const name =
          this.host.definitions.items.find((d) => d.id === item.item_id)
            ?.name ?? "items";
        const have = this.snapshot?.player.inventory[item.item_id] ?? 0;
        return `${item.quantity} ${name}${owned ? ` (have ${have})` : ""}`;
      })
      .join(" + ");
  }

  /** The heap as a bar, scaled so a small heap still reads as something rather than as nothing. */
  private drawSpoil(spoil: number, projected = spoil): void {
    const scale = Math.max(spoil, projected, 12);
    this.spoilValue.textContent =
      projected === spoil
        ? String(spoil)
        : `${spoil} → ${projected} load${projected === 1 ? "" : "s"}`;
    this.spoilFill.style.width = `${Math.round((projected / scale) * 100)}%`;
    this.spoilGauge.classList.toggle("empty", projected === 0);
  }

  private refresh(): void {
    this.revision += 1;
    this.preview = null;
    this.apply.disabled = true;
    this.apply.textContent = "Apply";
    this.bill.textContent = "";
    this.move.hidden = true;
    this.retaining.hidden = true;
    this.drawSpoil(this.snapshot?.spoil ?? 0);
    if (!this.start || !this.opened) {
      this.coverBox.hidden = true;
      this.status.classList.remove("blocked");
      this.status.textContent =
        this.shape.value === "cell"
          ? "Click the hex to work."
          : this.action === "level"
            ? "Click the hex whose grade everything else should match, then the far end."
            : "Click the first hex, then the far end. Up to 32 hexes at a time.";
      this.renderer.setGroundPreview(null);
      return;
    }
    this.status.textContent = "Checking the ground…";
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
        const preview = await this.host.groundPreview(edit);
        if (revision !== this.revision) continue;
        this.preview = preview;
        this.describe(edit, preview);
        this.renderer.setGroundPreview(preview);
      } catch (error) {
        if (revision === this.revision) {
          this.status.textContent = `Preview unavailable: ${String(error)}`;
          this.status.classList.add("blocked");
          this.renderer.setGroundPreview(null);
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

  /** Turn one native answer into the whole tray: the heap, the bill, the warnings and the verb. */
  private describe(edit: GroundEdit, preview: GroundPreview): void {
    const creative = this.snapshot?.player.creative === true;
    this.drawSpoil(this.snapshot?.spoil ?? 0, preview.spoil);
    this.move.hidden = !preview.cut && !preview.fill;
    this.move.textContent = `Cut ${preview.cut} · Fill ${preview.fill}`;
    this.move.classList.toggle("down", preview.fill > preview.cut);
    // The covering question survives its own refusal: native withholds the edit until it is
    // answered, and the answer is a tick the player has to reach for.
    this.coverBox.hidden = preview.covers === 0;
    this.coverText.replaceChildren();
    if (preview.covers > 0) {
      const title = document.createElement("b");
      title.textContent = `Seals ${preview.covers} deposit${preview.covers === 1 ? "" : "s"}`;
      this.coverText.append(
        title,
        "A sealed deposit cannot be gathered or extracted until the surface comes back up. Nothing is lost: strip the surface and it returns exactly as it was.",
      );
    }
    this.retaining.hidden = preview.retaining === 0;
    this.retaining.textContent = `${preview.retaining} hex${preview.retaining === 1 ? "" : "es"} would be left too steep to walk onto from one side. Cut the ground beside it to keep the way open.`;
    this.status.textContent =
      preview.error ??
      (this.choosingEnd
        ? `Choose the far end. ${preview.changes} hex${preview.changes === 1 ? "" : "es"} would change.`
        : preview.changes === 0
          ? "This ground already matches. Nothing to spend, dig or recover."
          : `${preview.changes} hex${preview.changes === 1 ? "" : "es"} will change. Hex ${edit.q}, ${edit.r}${edit.shape === "cell" ? "" : ` → ${edit.to_q}, ${edit.to_r}`}.`);
    this.status.classList.toggle("blocked", !!preview.error);
    // A refused edit was never priced — native stops before the bill — so quoting one here would be
    // inventing a number. The refusal is the whole answer until it is dealt with.
    this.bill.textContent = preview.error
      ? ""
      : `${
          preview.cost.length
            ? `Use ${this.names(preview.cost, true)}`
            : creative
              ? "Creative mode · materials are free"
              : "No materials needed"
        }${preview.refund.length ? ` · Recover ${this.names(preview.refund)}` : ""}`;
    const spec = ACTIONS.find((entry) => entry.action === this.action);
    this.apply.textContent = (spec?.verb ?? "Apply {n}").replace(
      "{n}",
      preview.changes
        ? `${preview.changes} hex${preview.changes === 1 ? "" : "es"}`
        : "selection",
    );
  }
}
