import "./boundaries.css";
import { axialToPixel, pixelToAxial } from "@hexlife/embed/hex";
import type { FactoryHost } from "../core/FactoryHost";
import type {
  BoundaryAction,
  BoundaryAnchor,
  BoundaryDefinition,
  BoundaryEdit,
  BoundaryPreview,
  FactorySnapshot,
  Ingredient,
  NativeInputCommand,
  WorldPoint,
} from "../core/types";
import {
  CORNER_NAMES,
  headingLabel,
  nearestVertex,
  sameVertex,
} from "../core/lattice";
import type { FactoryRenderer } from "../rendering/FactoryRenderer";
import { WORLD_SCALE } from "../rendering/landmarks";

type Verb = BoundaryAction;

/**
 * What the player is drawing, which is not quite what native is asked for. Native knows two shapes:
 * a straight run between two lattice vertices, and the rectangle two vertices bound. `edge` is the
 * third thing a player wants and the one they wanted most often before straight runs existed — one
 * side of one hex — expressed as the run between that side's two corners.
 */
type Selection = "edge" | "line" | "yard";

interface VerbSpec {
  readonly action: Verb;
  readonly icon: string;
  readonly label: string;
  readonly hint: string;
  readonly verb: string;
}

/**
 * Four verbs, always in the same place. Place is how a yard goes up; Open and Close are how you
 * walk through it; Strip takes it back. R cycles them the way the ground brush already does.
 */
const VERBS: readonly VerbSpec[] = [
  {
    action: "build",
    icon: "▮",
    label: "Place",
    hint: "Lay a fence, wall or gate along the selection. Identical construction is free.",
    verb: "Place {n}",
  },
  {
    action: "open",
    icon: "⌜",
    label: "Open",
    hint: "Swing selected gates open. Walking and transport can cross an open gate.",
    verb: "Open {n}",
  },
  {
    action: "close",
    icon: "⌝",
    label: "Close",
    hint: "Shut selected gates. The line must be clear of you and of live transport.",
    verb: "Close {n}",
  },
  {
    action: "remove",
    icon: "⌫",
    label: "Strip",
    hint: "Take the boundary down and recover exactly what it cost.",
    verb: "Strip {n}",
  },
];

const SELECTIONS: readonly {
  value: Selection;
  label: string;
  /** What to do next, before anything is picked. */
  prompt: string;
}[] = [
  {
    value: "edge",
    label: "One side of a hex",
    prompt: "Click near the side of a hex. One click, one segment.",
  },
  {
    value: "line",
    label: "Straight run",
    prompt:
      "Click a corner to anchor the run, then its far end. Twelve headings run dead straight.",
  },
  {
    value: "yard",
    label: "Rectangular yard",
    prompt:
      "Click one corner of the yard, then the opposite one. The rectangle snaps to whole hexes.",
  },
];

/** Picking is presentation; canonical segment identity and every transaction are native answers. */
export function nearestBoundaryDirection(
  cell: { q: number; r: number },
  point: WorldPoint,
): number {
  const center = axialToPixel(cell, WORLD_SCALE);
  const angle = Math.atan2(point.y - center.y, point.x - center.x);
  return ((Math.round(angle / (Math.PI / 3)) % 6) + 6) % 6;
}

/** The two corners of one hex side, in the order native names them. */
function edgeAnchors(
  cell: { q: number; r: number },
  direction: number,
): [BoundaryAnchor, BoundaryAnchor] {
  return [
    { ...cell, corner: (direction + 1) % 6 },
    { ...cell, corner: (direction + 2) % 6 },
  ];
}

const cornerOptions = CORNER_NAMES.map(
  (name, index) => `<option value="${index}">${name}</option>`,
).join("");

/**
 * A persistent, nonmodal enclosure tray. Every number in it is a native answer to the exact edit
 * Apply would send: the preview and the commit are one transaction.
 */
export class BoundaryTool {
  private opened = false;
  private action: Verb = "build";
  private definitionId: number;
  private start: BoundaryAnchor | null = null;
  private target: BoundaryAnchor | null = null;
  private choosingEnd = false;
  private preview: BoundaryPreview | null = null;
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
  private readonly direction: HTMLSelectElement;
  private readonly corner: HTMLSelectElement;
  private readonly toCorner: HTMLSelectElement;
  private readonly q: HTMLInputElement;
  private readonly r: HTMLInputElement;
  private readonly toQ: HTMLInputElement;
  private readonly toR: HTMLInputElement;
  private readonly edgeField: HTMLElement;
  private readonly cornerField: HTMLElement;
  private readonly endFields: HTMLElement;
  private readonly apply: HTMLButtonElement;
  private readonly status: HTMLElement;
  private readonly bill: HTMLElement;
  private readonly hint: HTMLElement;
  private readonly heading: HTMLElement;
  private readonly existing: HTMLElement;

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
    this.definitionId = host.definitions.boundaries[0]?.id ?? 0;
    root.innerHTML = `
      <header><div><small>CONSTRUCTION · ENCLOSURES</small><h2 id="boundary-heading">Fences & walls</h2></div><button type="button" data-close aria-label="Close enclosure tool">×</button></header>
      <div class="boundary-actions" role="group" aria-label="Enclosure work">${VERBS.map(
        (spec) =>
          `<button type="button" data-action="${spec.action}" aria-pressed="false" title="${spec.hint}"><span aria-hidden="true">${spec.icon}</span>${spec.label}</button>`,
      ).join("")}</div>
      <div class="boundary-palette" role="group" aria-label="Fence, wall or gate" data-palette></div>
      <p data-hint></p>
      <div class="boundary-fields"><label>Selection<select data-shape>${SELECTIONS.map(
        (shape) => `<option value="${shape.value}">${shape.label}</option>`,
      ).join("")}</select></label></div>
      <p class="boundary-heading-readout" data-heading hidden></p>
      <details class="boundary-precise"><summary>Precise placement</summary><div class="boundary-fields boundary-target">
        <label>Hex Q<input data-q type="number" step="1" min="-100000" max="100000" value="0"></label>
        <label>Hex R<input data-r type="number" step="1" min="-100000" max="100000" value="0"></label>
        <label data-edge-field>Edge<select data-direction><option value="0">East</option><option value="1">Southeast</option><option value="2">Southwest</option><option value="3">West</option><option value="4">Northwest</option><option value="5">Northeast</option></select></label>
        <label data-corner-field hidden>Corner<select data-corner>${cornerOptions}</select></label>
      </div><div class="boundary-fields boundary-target" data-end-fields hidden>
        <label>To Q<input data-to-q type="number" step="1" min="-100000" max="100000" value="0"></label>
        <label>To R<input data-to-r type="number" step="1" min="-100000" max="100000" value="0"></label>
        <label>To corner<select data-to-corner>${cornerOptions}</select></label>
      </div></details>
      <p class="boundary-existing" data-existing></p>
      <p class="boundary-status" data-status role="status" aria-live="polite"></p>
      <p class="boundary-bill" data-bill></p>
      <div class="boundary-panel-actions"><button type="button" data-apply disabled>Apply</button><button type="button" data-clear>New selection</button><button type="button" data-undo title="Undo the last enclosure edit (Ctrl+Z while this tool is open)">Undo</button></div>
      <small class="boundary-help">R cycles the work, Shift+R goes back, Delete jumps to Strip. Esc cancels a selection; Esc again exits. Nothing is spent before Apply.</small>`;
    const get = <T extends HTMLElement>(selector: string): T =>
      root.querySelector<T>(selector)!;
    root.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        this.escape();
      }
    });
    this.actions = get(".boundary-actions");
    this.palette = get("[data-palette]");
    this.shape = get("[data-shape]");
    this.direction = get("[data-direction]");
    this.corner = get("[data-corner]");
    this.toCorner = get("[data-to-corner]");
    this.q = get("[data-q]");
    this.r = get("[data-r]");
    this.toQ = get("[data-to-q]");
    this.toR = get("[data-to-r]");
    this.edgeField = get("[data-edge-field]");
    this.cornerField = get("[data-corner-field]");
    this.endFields = get("[data-end-fields]");
    this.apply = get("[data-apply]");
    this.status = get("[data-status]");
    this.bill = get("[data-bill]");
    this.hint = get("[data-hint]");
    this.heading = get("[data-heading]");
    this.existing = get("[data-existing]");
    this.buildPalette();
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
    this.actions.addEventListener("click", (event) => {
      const action = (event.target as HTMLElement).closest<HTMLElement>(
        "[data-action]",
      )?.dataset.action;
      if (action) this.selectAction(action as Verb);
    });
    this.shape.addEventListener("change", () => {
      this.syncFields();
      this.clear();
    });
    for (const control of [this.direction, this.corner, this.toCorner])
      control.addEventListener("change", () => this.readCoordinates());
    for (const input of [this.q, this.r, this.toQ, this.toR])
      input.addEventListener("input", () => this.readCoordinates());
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
        this.status.textContent = "Raising the enclosure…";
      }
    });
    this.syncFields();
    this.selectAction("build");
  }

  get active(): boolean {
    return this.opened;
  }

  private get selection(): Selection {
    return this.shape.value as Selection;
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

  cycleAction(reverse: boolean): void {
    const index = VERBS.findIndex((spec) => spec.action === this.action);
    const next = (index + (reverse ? VERBS.length - 1 : 1)) % VERBS.length;
    this.selectAction(VERBS[next]!.action);
  }

  selectRemoval(): void {
    this.selectAction("remove");
  }

  clear(): void {
    this.start = null;
    this.target = null;
    this.choosingEnd = false;
    this.panel.scrollTop = 0;
    this.refresh();
  }

  /**
   * One click of the map. A hex side is picked whole, because that is one decision; a run or a yard
   * is two vertices, and the first click leaves the second one following the pointer.
   */
  pick(cell: { q: number; r: number }, point: WorldPoint): void {
    if (this.selection === "edge") {
      const direction = nearestBoundaryDirection(cell, point);
      [this.start, this.target] = edgeAnchors(cell, direction);
      this.choosingEnd = false;
      this.direction.value = String(direction);
      this.writeCoordinates();
      this.refresh();
      return;
    }
    const vertex = nearestVertex(point);
    if (this.choosingEnd) {
      this.target = vertex;
      this.choosingEnd = false;
    } else {
      this.start = vertex;
      this.target = vertex;
      this.choosingEnd = true;
    }
    this.writeCoordinates();
    this.refresh();
  }

  hover(_cell: { q: number; r: number }, point: WorldPoint): void {
    if (!this.opened || !this.choosingEnd) return;
    const vertex = nearestVertex(point);
    if (sameVertex(this.target, vertex)) return;
    this.target = vertex;
    this.writeCoordinates();
    this.refresh();
  }

  update(snapshot: FactorySnapshot): void {
    const signature = `${snapshot.player.x},${snapshot.player.y}:${JSON.stringify(snapshot.player.inventory)}:${snapshot.researched.join(",")}`;
    const changed =
      this.snapshot?.boundaries !== snapshot.boundaries ||
      this.inventorySignature !== signature ||
      this.snapshot?.events !== snapshot.events;
    this.snapshot = snapshot;
    this.inventorySignature = signature;
    if (changed && this.opened) {
      this.buildPalette();
      if (this.start) this.refresh();
    }
  }

  /** Which precise-placement controls belong to the shape on the table. */
  private syncFields(): void {
    const edge = this.selection === "edge";
    this.edgeField.hidden = !edge;
    this.cornerField.hidden = edge;
    this.endFields.hidden = edge;
  }

  private selectAction(action: Verb): void {
    this.action = action;
    for (const button of this.actions.querySelectorAll<HTMLElement>(
      "[data-action]",
    ))
      button.setAttribute(
        "aria-pressed",
        String(button.dataset.action === action),
      );
    this.palette.hidden = action !== "build";
    this.hint.textContent =
      VERBS.find((spec) => spec.action === action)?.hint ?? "";
    this.refresh();
  }

  private researched(id: number | undefined): boolean {
    if (id === undefined) return true;
    if (this.snapshot?.player.creative) return true;
    return this.snapshot?.researched.includes(id) === true;
  }

  private buildPalette(): void {
    const inventory = this.snapshot?.player.inventory ?? {};
    const creative = this.snapshot?.player.creative === true;
    // Native builds gates one segment at a time, so a shape that can draw more than one refuses
    // them outright rather than letting a run be priced and then turned away.
    const many = this.selection !== "edge";
    this.palette.replaceChildren(
      ...this.host.definitions.boundaries.map((definition) => {
        const button = document.createElement("button");
        button.type = "button";
        button.dataset.definition = String(definition.id);
        button.title = definition.description;
        button.setAttribute(
          "aria-pressed",
          String(definition.id === this.definitionId),
        );
        const locked = !this.researched(definition.unlock_technology_id);
        const gatedRun = many && definition.gate;
        button.disabled = gatedRun;
        button.classList.toggle("locked", locked);
        const swatch = document.createElement("i");
        swatch.setAttribute("aria-hidden", "true");
        swatch.style.background = this.swatch(definition);
        const name = document.createElement("span");
        name.textContent = definition.name;
        const kind = document.createElement("span");
        kind.className = "boundary-kind";
        kind.textContent = gatedRun
          ? "Gate · one side at a time"
          : locked
            ? this.lockLabel(definition)
            : definition.gate
              ? "Gate · crossing"
              : definition.family === "wall"
                ? "Wall · opaque"
                : "Fence · see-through";
        const price = document.createElement("small");
        price.className = "boundary-price";
        const short = definition.construction_cost.some(
          (item) => (inventory[item.item_id] ?? 0) < item.quantity,
        );
        price.classList.toggle("short", short && !creative && !locked);
        price.textContent = locked
          ? this.lockLabel(definition)
          : creative
            ? "Free · creative mode"
            : `${this.names(definition.construction_cost, true)} per segment`;
        button.append(swatch, name, kind, price);
        button.addEventListener("click", () => {
          this.definitionId = definition.id;
          this.buildPalette();
          this.refresh();
        });
        return button;
      }),
    );
  }

  private swatch(definition: BoundaryDefinition): string {
    const item = this.host.definitions.items.find(
      (entry) => entry.id === definition.construction_cost[0]?.item_id,
    );
    return item?.color ?? "#c8aa7c";
  }

  private lockLabel(definition: BoundaryDefinition): string {
    const technology = this.host.technologies.technologies.find(
      (entry) => entry.id === definition.unlock_technology_id,
    );
    return technology
      ? `Research ${technology.name}`
      : "Locked behind research";
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

  /** Push the picked vertices back into the number fields, so both ways in agree. */
  private writeCoordinates(): void {
    if (this.start) {
      this.q.value = String(this.start.q);
      this.r.value = String(this.start.r);
      this.corner.value = String(this.start.corner);
    }
    if (this.target) {
      this.toQ.value = String(this.target.q);
      this.toR.value = String(this.target.r);
      this.toCorner.value = String(this.target.corner);
    }
  }

  /** Typed coordinates are a selection like any other: same anchors, same native transaction. */
  private readCoordinates(): void {
    const edge = this.selection === "edge";
    const inputs = edge
      ? [this.q, this.r]
      : [this.q, this.r, this.toQ, this.toR];
    if (inputs.some((input) => !input.validity.valid || !input.value)) {
      this.revision += 1;
      this.preview = null;
      this.apply.disabled = true;
      this.status.textContent =
        "Enter whole hex coordinates between −100000 and 100000.";
      this.renderer.setBoundaryPreview(null);
      this.renderer.setBoundaryAnchors([]);
      return;
    }
    const cell = { q: Number(this.q.value), r: Number(this.r.value) };
    if (edge) {
      [this.start, this.target] = edgeAnchors(
        cell,
        Number(this.direction.value),
      );
    } else {
      this.start = { ...cell, corner: Number(this.corner.value) };
      this.target = {
        q: Number(this.toQ.value),
        r: Number(this.toR.value),
        corner: Number(this.toCorner.value),
      };
    }
    this.choosingEnd = false;
    this.refresh();
  }

  private edit(): BoundaryEdit | null {
    if (!this.start || !this.target) return null;
    return {
      q: this.start.q,
      r: this.start.r,
      corner: this.start.corner,
      to_q: this.target.q,
      to_r: this.target.r,
      to_corner: this.target.corner,
      shape: this.selection === "yard" ? "yard" : "line",
      definition_id: this.definitionId,
      action: this.action,
    };
  }

  /** The bearing a run is on, and whether the lattice can hold it dead straight. */
  private describeHeading(): void {
    const bearing =
      this.selection === "yard" || !this.start || !this.target
        ? null
        : headingLabel(this.start, this.target);
    this.heading.hidden = !bearing;
    if (!bearing) return;
    // Native reaches any vertex: off the twelve headings its chain staircases toward the far end
    // rather than refusing. That is a fine wall and a surprising one, so it is said in advance.
    this.heading.textContent = bearing.exact
      ? `Heading ${bearing.bearing} · dead straight.`
      : `Just off ${bearing.bearing} · this run steps toward the far end. Twelve headings leave a corner dead straight.`;
    this.heading.classList.toggle("approximate", !bearing.exact);
  }

  private refresh(): void {
    this.revision += 1;
    this.preview = null;
    this.apply.disabled = true;
    this.apply.textContent = "Apply";
    this.bill.textContent = "";
    this.existing.textContent = "";
    this.palette.hidden = this.action !== "build";
    if (this.opened) this.buildPalette();
    this.describeHeading();
    this.renderer.setBoundaryAnchors(
      !this.opened || !this.start
        ? []
        : this.selection === "edge" || sameVertex(this.start, this.target)
          ? [this.start]
          : [this.start, this.target!],
    );
    if (!this.start || !this.opened) {
      this.status.textContent =
        SELECTIONS.find((shape) => shape.value === this.selection)?.prompt ??
        "";
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
        const only =
          preview.segments.length === 1 ? preview.segments[0]! : null;
        const existing =
          only &&
          this.snapshot?.boundaries.find(
            (b) => b.q === only.q && b.r === only.r && b.chord === only.chord,
          );
        const definition =
          existing &&
          this.host.definitions.boundaries.find(
            (d) => d.id === existing.definition_id,
          );
        this.existing.textContent = existing
          ? `Current: ${definition?.name ?? "Boundary"}${definition?.gate ? (existing.open ? " · Open" : " · Closed") : ""}`
          : only
            ? "Current: empty line"
            : "";
        const spec = VERBS.find((entry) => entry.action === this.action);
        const segments = `${preview.segments.length} segment${preview.segments.length === 1 ? "" : "s"}`;
        this.status.textContent =
          preview.error ??
          (this.choosingEnd
            ? `${this.selection === "yard" ? "Choose the opposite corner" : "Choose the far end"}. Preview: ${segments}.`
            : preview.changes === 0
              ? "Already matches this selection. Nothing to spend or recover."
              : `${preview.changes} of ${segments} will change. Floor space stays free.`);
        this.status.classList.toggle("blocked", !!preview.error);
        this.bill.textContent = `${preview.cost.length ? `Use ${this.names(preview.cost, true)}` : this.snapshot?.player.creative ? "Creative mode · materials are free" : "No materials needed"}${preview.refund.length ? ` · Recover ${this.names(preview.refund)}` : ""}`;
        this.apply.textContent = (spec?.verb ?? "Apply {n}").replace(
          "{n}",
          String(preview.changes || "selection"),
        );
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
