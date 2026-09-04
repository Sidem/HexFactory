import "./ground.css";
import { pixelToAxial } from "@hexlife/embed/hex";
import type { FactoryHost } from "../core/FactoryHost";
import type {
  BoundaryAnchor,
  FactorySnapshot,
  GroundAction,
  GroundEdit,
  GroundPreview,
  GroundReference,
  GroundShape,
  Ingredient,
  NativeInputCommand,
  WorldPoint,
} from "../core/types";
import { UNTREATED_MOVEMENT } from "../core/definitions";
import { CORNER_NAMES, nearestVertex, sameVertex } from "../core/lattice";
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
 * The six verbs put the intent-first grade tool ahead of precise cut, fill and levelling. Surface
 * removal sits beside paving because taking one up is the same decision made the other way.
 */
const ACTIONS: readonly ActionSpec[] = [
  {
    action: "smooth",
    icon: "≈",
    label: "Smooth",
    hint: "Make the selection walkable from the first picked hex. High steps are cut first; low ground is filled only where the route needs it.",
    verb: "Smooth {n}",
  },
  {
    action: "level",
    icon: "═",
    label: "Level",
    hint: "Even every selected hex onto one exact grade. Choose which grade below.",
    verb: "Level {n}",
  },
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
    hint: "Fill by the chosen depth, using spoil from ground cut elsewhere. Nothing is raised out of nothing, and a hex without room for the whole depth takes what it has room for.",
    verb: "Raise {n}",
  },
  {
    action: "lower",
    icon: "▼",
    label: "Lower",
    hint: "Cut by the chosen depth. What comes out goes on the spoil heap, ready to fill somewhere else. One cut takes a cliff face down and the hex walks like any other.",
    verb: "Lower {n}",
  },
];

/** The verbs whose depth is a number the player chooses rather than a fixed single step. */
const DEEP: readonly GroundAction[] = ["raise", "lower"];

interface ShapeSpec {
  readonly base: ShapeBase;
  readonly icon: string;
  readonly label: string;
  readonly hint: string;
  /** What this shape sends when it is filled, and when it is outlined. */
  readonly filled: GroundShape;
  readonly outline: GroundShape | null;
  /** Whether the two anchors are lattice vertices rather than whole hexes. */
  readonly vertices: boolean;
  /** What to say once the first anchor is down and the second one is still moving. */
  readonly finish: string;
}

type ShapeBase = "cell" | "path" | "rect" | "circle";

/**
 * Four shapes and one modifier, rather than six buttons.
 *
 * An outline is the perimeter of its own fill on exactly the same two anchors, so a floor and the
 * kerb around it — or a plaza and its rim — are the same drag with one toggle flipped. Presenting
 * them as six peers would hide that, and would make a player who had already framed the yard they
 * wanted re-drag it to floor it. A hex and a line have no interior, so for them the toggle has
 * nothing to say and goes away.
 */
const SHAPES: readonly ShapeSpec[] = [
  {
    base: "cell",
    icon: "⬢",
    label: "Hex",
    hint: "One hex, on its own.",
    filled: "cell",
    outline: null,
    vertices: false,
    finish: "",
  },
  {
    base: "path",
    icon: "╱",
    label: "Line",
    hint: "A straight run of hexes between two you pick.",
    filled: "path",
    outline: null,
    vertices: false,
    finish: "Choose the far end",
  },
  {
    base: "rect",
    icon: "▭",
    label: "Rectangle",
    hint: "A true rectangle drawn between two hex corners, taking in every hex it touches — the same rectangle a walled yard is drawn on.",
    filled: "rect",
    outline: "frame",
    vertices: true,
    finish: "Choose the opposite corner",
  },
  {
    base: "circle",
    icon: "◯",
    label: "Circle",
    hint: "A disc measured from its centre out to a hex on its rim, so the radius is a distance you count on the map.",
    filled: "disc",
    outline: "ring",
    vertices: false,
    finish: "Choose a hex on the rim",
  },
];

/** Which grade a level evens onto, in the order a site is usually worked. */
const REFERENCES: readonly {
  value: GroundReference;
  label: string;
  hint: string;
}[] = [
  {
    value: "first",
    label: "First picked",
    hint: "Match the grade of the hex the selection started on.",
  },
  {
    value: "lowest",
    label: "Lowest",
    hint: "Cut everything down to the deepest hex in the selection. Spoil comes out.",
  },
  {
    value: "highest",
    label: "Highest",
    hint: "Fill everything up to the highest hex in the selection. Spoil goes in.",
  },
];

const cornerOptions = CORNER_NAMES.map(
  (name, index) => `<option value="${index}">${name}</option>`,
).join("");

/**
 * A persistent, nonmodal earthworks tray. Every number in it is a native answer to the exact edit
 * the Apply button would send: the preview and the commit are one transaction, so what the tray
 * quotes is what the world charges.
 */
export class GroundTool {
  private opened = false;
  private action: GroundAction = "smooth";
  private surface: number;
  private cover = false;
  private base: ShapeBase = "path";
  /** Take the perimeter of the shape rather than the whole of it. Ignored where there is no inside. */
  private outline = false;
  /** How many steps one raise or lower moves the ground. Native clamps it; the tray offers 1–3. */
  private depth = 1;
  private reference: GroundReference = "first";
  /**
   * Both ends of the selection. A hex, a line or a circle is anchored on hexes and the corner is
   * ignored; a rectangle is anchored on two lattice vertices, the same two a wall would be drawn
   * between, so a yard and the fence around it can be laid on exactly the same rectangle.
   */
  private start: BoundaryAnchor | null = null;
  private target: BoundaryAnchor | null = null;
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
  private readonly shapes: HTMLElement;
  private readonly outlineBox: HTMLElement;
  private readonly outlineInput: HTMLInputElement;
  private readonly grade: HTMLElement;
  private readonly depthRow: HTMLElement;
  private readonly referenceRow: HTMLElement;
  private readonly q: HTMLInputElement;
  private readonly r: HTMLInputElement;
  private readonly toQ: HTMLInputElement;
  private readonly toR: HTMLInputElement;
  private readonly corner: HTMLSelectElement;
  private readonly toCorner: HTMLSelectElement;
  private readonly cornerFields: readonly HTMLElement[];
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
  private readonly obstructed: HTMLElement;

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
      <div class="ground-shapes" role="group" aria-label="Shape of the selection">${SHAPES.map(
        (shape) =>
          `<button type="button" data-shape="${shape.base}" aria-pressed="false" title="${shape.hint}"><span aria-hidden="true">${shape.icon}</span>${shape.label}</button>`,
      ).join("")}</div>
      <label class="ground-outline" data-outline hidden><input type="checkbox" data-outline-input><span><b>Outline only</b>Take just the hexes on the edge of the shape, one hex thick, on the very same two anchors.</span></label>
      <div class="ground-grade" data-grade hidden>
        <div class="ground-choice" data-depth hidden role="group" aria-label="How far each hex moves">
          <span>Depth</span>${[1, 2, 3]
            .map(
              (steps) =>
                `<button type="button" data-depth-steps="${steps}" aria-pressed="false" title="Move each hex ${steps} step${steps === 1 ? "" : "s"}, as far as its own ±3 limit allows">${steps}</button>`,
            )
            .join("")}<small>step limit ±3</small>
        </div>
        <div class="ground-choice" data-reference hidden role="group" aria-label="Which grade to level onto">
          <span>Match</span>${REFERENCES.map(
            (entry) =>
              `<button type="button" data-reference="${entry.value}" aria-pressed="false" title="${entry.hint}">${entry.label}</button>`,
          ).join("")}
        </div>
      </div>
      <details class="ground-precise"><summary>Precise selection</summary><div class="ground-fields ground-target">
        <label>From Q<input data-q type="number" step="1" min="-100000" max="100000" value="0"></label>
        <label>From R<input data-r type="number" step="1" min="-100000" max="100000" value="0"></label>
        <label data-corner-field hidden>Corner<select data-corner>${cornerOptions}</select></label>
        <label>To Q<input data-to-q type="number" step="1" min="-100000" max="100000" value="0"></label>
        <label>To R<input data-to-r type="number" step="1" min="-100000" max="100000" value="0"></label>
        <label data-corner-field hidden>To corner<select data-to-corner>${cornerOptions}</select></label>
      </div></details>
      <div class="ground-spoil"><span>Spoil heap</span><span class="ground-gauge"><i data-spoil-fill style="width: 0%"></i></span><b data-spoil>0</b></div>
      <p class="ground-move" data-move hidden></p>
      <p class="ground-status" data-status role="status" aria-live="polite"></p>
      <p class="ground-bill" data-bill></p>
      <p class="ground-retaining" data-retaining hidden></p>
      <p class="ground-obstructed" data-obstructed hidden></p>
      <label class="ground-cover" data-cover hidden><input type="checkbox" data-cover-input><span data-cover-text></span></label>
      <div class="ground-panel-actions"><button type="button" data-apply disabled>Apply</button><button type="button" data-clear>New selection</button><button type="button" data-undo title="Undo the last ground works edit (Ctrl+Z while this tool is open)">Undo</button></div>
      <small class="ground-help">Click to start a selection, then click again to finish it. R cycles the work and Shift+R goes back; [ and ] cycle the shape, \\ toggles the outline, − and = set the depth, and Delete jumps to Strip. Esc cancels a selection; Esc again exits. Nothing is spent, dug or recovered before Apply.</small>`;
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
    this.shapes = get(".ground-shapes");
    this.outlineBox = get("[data-outline]");
    this.outlineInput = get("[data-outline-input]");
    this.grade = get("[data-grade]");
    this.depthRow = get("[data-depth]");
    this.referenceRow = get("[data-reference]");
    this.q = get("[data-q]");
    this.r = get("[data-r]");
    this.toQ = get("[data-to-q]");
    this.toR = get("[data-to-r]");
    this.corner = get("[data-corner]");
    this.toCorner = get("[data-to-corner]");
    this.cornerFields = [
      ...root.querySelectorAll<HTMLElement>("[data-corner-field]"),
    ];
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
    this.obstructed = get("[data-obstructed]");
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
    this.shapes.addEventListener("click", (event) => {
      const base = (event.target as HTMLElement).closest<HTMLElement>(
        "[data-shape]",
      )?.dataset.shape;
      if (base) this.selectShape(base as ShapeBase);
    });
    // Flipping the outline keeps the anchors: the player has already said where, and is now saying
    // how much of it. Re-picking the same two corners to floor a yard they just framed is exactly
    // the busywork the toggle exists to remove.
    this.outlineInput.addEventListener("change", () => {
      this.outline = this.outlineInput.checked;
      this.refresh();
    });
    this.depthRow.addEventListener("click", (event) => {
      const steps = (event.target as HTMLElement).closest<HTMLElement>(
        "[data-depth-steps]",
      )?.dataset.depthSteps;
      if (steps) this.setDepth(Number(steps));
    });
    this.referenceRow.addEventListener("click", (event) => {
      const reference = (event.target as HTMLElement).closest<HTMLElement>(
        "[data-reference]",
      )?.dataset.reference;
      if (reference) this.setReference(reference as GroundReference);
    });
    for (const input of [this.q, this.r, this.toQ, this.toR])
      input.addEventListener("input", () => this.readCoordinates());
    for (const control of [this.corner, this.toCorner])
      control.addEventListener("change", () => this.readCoordinates());
    this.syncFields();
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
    this.selectAction("smooth");
  }

  get active(): boolean {
    return this.opened;
  }

  private get spec(): ShapeSpec {
    return SHAPES.find((entry) => entry.base === this.base) ?? SHAPES[0]!;
  }

  /** The mode actually on the wire: a shape, plus whether only its perimeter was asked for. */
  private get shape(): GroundShape {
    const spec = this.spec;
    return (this.outline && spec.outline) || spec.filled;
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

  /** `[` and `]` walk the four shapes, the way `R` walks the verbs. */
  cycleShape(reverse: boolean): void {
    const index = SHAPES.findIndex((entry) => entry.base === this.base);
    const next = (index + (reverse ? SHAPES.length - 1 : 1)) % SHAPES.length;
    this.selectShape(SHAPES[next]!.base);
  }

  /** `\` flips fill and outline without disturbing the anchors already down. */
  toggleOutline(): void {
    if (!this.spec.outline) return;
    this.outline = !this.outline;
    this.outlineInput.checked = this.outline;
    this.refresh();
  }

  /** `−` and `=` set how far a raise or lower moves the ground. */
  nudgeDepth(by: number): void {
    if (!DEEP.includes(this.action)) return;
    this.setDepth(this.depth + by);
  }

  private selectShape(base: ShapeBase): void {
    if (base === this.base) return;
    this.base = base;
    // A vertex anchor and a hex anchor are not the same point, and a circle reads its second anchor
    // as a rim rather than as a corner, so the anchors do not survive a change of shape.
    this.syncFields();
    this.clear();
  }

  private setDepth(steps: number): void {
    const depth = Math.min(3, Math.max(1, steps));
    if (depth === this.depth) return;
    this.depth = depth;
    this.syncFields();
    this.refresh();
  }

  private setReference(reference: GroundReference): void {
    if (reference === this.reference) return;
    this.reference = reference;
    this.syncFields();
    this.refresh();
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

  /** Where the pointer landed, as the shape on the table understands it: a hex, or a hex corner. */
  private anchorAt(
    cell: { q: number; r: number },
    point: WorldPoint,
  ): BoundaryAnchor {
    return this.spec.vertices ? nearestVertex(point) : { ...cell, corner: 0 };
  }

  pick(cell: { q: number; r: number }, point: WorldPoint): void {
    const anchor = this.anchorAt(cell, point);
    if (this.base !== "cell" && this.choosingEnd) {
      this.target = anchor;
      this.choosingEnd = false;
    } else {
      // A fresh selection is a fresh question, so a covering already agreed to does not carry over.
      this.cover = false;
      this.coverInput.checked = false;
      this.start = anchor;
      this.target = anchor;
      this.choosingEnd = this.base !== "cell";
    }
    this.writeCoordinates();
    this.refresh();
  }

  hover(cell: { q: number; r: number }, point: WorldPoint): void {
    if (!this.opened || !this.choosingEnd) return;
    const anchor = this.anchorAt(cell, point);
    if (
      this.spec.vertices
        ? sameVertex(this.target, anchor)
        : this.target?.q === anchor.q && this.target.r === anchor.r
    )
      return;
    this.target = anchor;
    this.writeCoordinates();
    this.refresh();
  }

  /** Which controls belong to the shape and the verb on the table, and which are pressed. */
  private syncFields(): void {
    const spec = this.spec;
    for (const button of this.shapes.querySelectorAll<HTMLElement>(
      "[data-shape]",
    ))
      button.setAttribute(
        "aria-pressed",
        String(button.dataset.shape === this.base),
      );
    // A hex and a line are already one hex thick, so there is nothing for the toggle to take away.
    this.outlineBox.hidden = !spec.outline;
    this.outlineInput.checked = this.outline && !!spec.outline;
    const deep = DEEP.includes(this.action);
    const levelling = this.action === "level";
    this.depthRow.hidden = !deep;
    this.referenceRow.hidden = !levelling;
    this.grade.hidden = !deep && !levelling;
    for (const button of this.depthRow.querySelectorAll<HTMLElement>(
      "[data-depth-steps]",
    ))
      button.setAttribute(
        "aria-pressed",
        String(Number(button.dataset.depthSteps) === this.depth),
      );
    for (const button of this.referenceRow.querySelectorAll<HTMLElement>(
      "[data-reference]",
    ))
      button.setAttribute(
        "aria-pressed",
        String(button.dataset.reference === this.reference),
      );
    for (const field of this.cornerFields) field.hidden = !spec.vertices;
    this.cornerFields[0]?.parentElement?.classList.toggle(
      "vertices",
      spec.vertices,
    );
  }

  /** Push the picked anchors back into the number fields, so both ways in agree. */
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
    this.syncFields();
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
    this.start = {
      q: Number(this.q.value),
      r: Number(this.r.value),
      corner: Number(this.corner.value),
    };
    this.target =
      this.base === "cell"
        ? this.start
        : {
            q: Number(this.toQ.value),
            r: Number(this.toR.value),
            corner: Number(this.toCorner.value),
          };
    this.choosingEnd = false;
    this.refresh();
  }

  private edit(): GroundEdit | null {
    if (!this.start || !this.target) return null;
    const target = this.base === "cell" ? this.start : this.target;
    return {
      q: this.start.q,
      r: this.start.r,
      corner: this.start.corner,
      to_q: target.q,
      to_r: target.r,
      to_corner: target.corner,
      shape: this.shape,
      definition_id: this.surface,
      action: this.action,
      cover: this.cover,
      steps: this.depth,
      reference: this.reference,
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
    this.obstructed.hidden = true;
    this.drawSpoil(this.snapshot?.spoil ?? 0);
    // A rectangle is pinned to lattice vertices, so the pins go up the moment the first corner is
    // taken — before there is any rectangle to price, and the same pins a wall would be drawn on.
    this.renderer.setBoundaryAnchors(
      !this.opened || !this.start || !this.spec.vertices
        ? []
        : sameVertex(this.start, this.target)
          ? [this.start]
          : [this.start, this.target!],
    );
    if (!this.start || !this.opened) {
      this.coverBox.hidden = true;
      this.status.classList.remove("blocked");
      this.status.textContent = this.opening();
      this.renderer.setGroundPreview(null);
      return;
    }
    this.status.textContent = "Checking the ground…";
    this.requested = true;
    if (!this.pending) void this.resolve();
  }

  /** What to ask for before there is anything selected, in the words of the shape on the table. */
  private opening(): string {
    const bound = "Up to 64 hexes at a time.";
    switch (this.base) {
      case "cell":
        return "Click the hex to work.";
      case "path":
        if (this.action === "smooth")
          return "Click accessible ground to set the starting height, then click across the steep slope.";
        return this.action === "level"
          ? "Click the hex whose grade everything else should match, then the far end."
          : `Click the first hex, then the far end. ${bound}`;
      case "rect":
        return `Click one corner, then the opposite one. Every hex the rectangle touches is taken in${this.outline ? ", and only its edge is worked" : ""}. ${bound}`;
      default:
        return `Click the centre, then a hex on the rim${this.outline ? ". Only the rim itself is worked" : ""}. ${bound}`;
    }
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
    this.retaining.textContent = `${preview.retaining} hex${preview.retaining === 1 ? "" : "es"} would still have an impassable edge at the selection boundary or a skipped obstacle. Widen the selection to blend that edge too.`;
    // One obstacle in a selection is a note beside the work, not a refusal of it: native passes the
    // hex over and grades the rest. Naming the first one gives the player somewhere to look.
    const stuck = preview.cells.find((cell) => cell.blocked);
    this.obstructed.hidden = preview.blocked === 0 || !stuck;
    if (stuck)
      this.obstructed.textContent = `${preview.blocked} hex${preview.blocked === 1 ? " is" : "es are"} passed over. Hex ${stuck.q}, ${stuck.r}: ${stuck.blocked}.`;
    this.status.textContent =
      preview.error ??
      (this.choosingEnd
        ? `${this.spec.finish}. ${preview.changes} hex${preview.changes === 1 ? "" : "es"} would change.`
        : preview.changes === 0
          ? "This ground already matches. Nothing to spend, dig or recover."
          : this.action === "smooth"
            ? `${preview.changes} hex${preview.changes === 1 ? "" : "es"} will change into a walkable grade from the first picked hex. ${this.where(edit)}`
            : `${preview.changes} hex${preview.changes === 1 ? "" : "es"} will change. ${this.where(edit)}`);
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
    const depth = DEEP.includes(this.action)
      ? ` by ${this.depth} step${this.depth === 1 ? "" : "s"}`
      : this.action === "level"
        ? ` to the ${REFERENCES.find((entry) => entry.value === this.reference)?.label.toLowerCase() ?? ""}`
        : "";
    this.apply.textContent =
      (spec?.verb ?? "Apply {n}").replace(
        "{n}",
        preview.changes
          ? `${preview.changes} hex${preview.changes === 1 ? "" : "es"}`
          : "selection",
      ) + depth;
  }

  /** Where the selection sits, said the way the shape that made it was drawn. */
  private where(edit: GroundEdit): string {
    switch (this.base) {
      case "cell":
        return `Hex ${edit.q}, ${edit.r}.`;
      case "rect":
        return `${this.outline ? "Frame" : "Rectangle"} ${CORNER_NAMES[edit.corner]?.toLowerCase() ?? ""} of ${edit.q}, ${edit.r} → ${CORNER_NAMES[edit.to_corner]?.toLowerCase() ?? ""} of ${edit.to_q}, ${edit.to_r}.`;
      case "circle": {
        const radius = Math.max(
          Math.abs(edit.to_q - edit.q),
          Math.abs(edit.to_r - edit.r),
          Math.abs(edit.to_q - edit.q + edit.to_r - edit.r),
        );
        return `${this.outline ? "Ring" : "Disc"} of radius ${radius} around ${edit.q}, ${edit.r}.`;
      }
      default:
        return `Hex ${edit.q}, ${edit.r} → ${edit.to_q}, ${edit.to_r}.`;
    }
  }
}
