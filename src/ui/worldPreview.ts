/**
 * The live picture of a world nobody has generated yet.
 *
 * Every other control on the new-world form describes a parameter in a sentence. This draws the
 * consequence: native rasters the same `terrain_at` the start button will run, and this paints the
 * bytes it sends back. That is why the preview is a wasm export rather than a generator written
 * here — a second implementation on this side would be a picture of a world nobody plays.
 *
 * The pure half of the module is the palette and the raster. Vitest runs in node with no DOM, so
 * anything that could be wrong about a colour or a pixel lives in a function a test can call.
 */

import { TERRAIN_INFO, TERRAIN_ORDER } from "../core/terrain";
import type {
  Terrain,
  WorldParams,
  WorldPreview,
  WorldPreviewChange,
  WorldPreviewNeed,
  WorldPreviewRepair,
} from "../core/types";
import { WORLD_PARAMETER_FIELDS } from "./worldParameters";

/**
 * The raster native is asked for. Fixed rather than measured from the element: the panel is a
 * flexible width in a scrolling column, and re-rastering on every resize would ask the generator to
 * work for a layout pass. The canvas is scaled to fit by CSS.
 */
export const PREVIEW_WIDTH = 256;
export const PREVIEW_HEIGHT = 144;

/**
 * What the fills are composited over. The band colours are translucent because they are drawn over
 * the map in play; a preview has nothing under it but the panel, so the panel's own ground stands in
 * — see `.world-param-pinned` in `src/styles.css`.
 */
export const PREVIEW_BACKDROP = "#0c1b18";

export interface PreviewZoom {
  key: string;
  label: string;
  /** The span the width frames, in hexes. */
  hexesAcross: number;
  /** What this zoom is the right one for looking at. */
  note: string;
}

/**
 * Three fixed spans rather than one.
 *
 * A span wide enough to show a landform is far too wide to show a two-hex river, and a span that
 * scaled itself to `elevation_coarse_cell` would hide the one slider it scaled with — move landform
 * scale and the picture would come back identical. So the zoom is the player's, and every parameter
 * has a zoom it is visible at.
 */
export const PREVIEW_ZOOMS: readonly PreviewZoom[] = [
  {
    key: "region",
    label: "Region",
    hexesAcross: 2048,
    note: "continents, seas and coastlines",
  },
  {
    key: "area",
    label: "Area",
    hexesAcross: 384,
    note: "rivers, cliffs and how ground is mixed",
  },
  {
    key: "close",
    label: "Close",
    hexesAcross: 64,
    note: "single deposits and the ground you land on",
  },
];

interface Rgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

/** `#rgb`, `#rrggbb` and `#rrggbbaa`. Anything else is opaque black rather than a thrown error. */
export function parseHexColor(color: string): Rgba {
  const hex = color.trim().replace(/^#/, "");
  const wide =
    hex.length === 3 || hex.length === 4
      ? [...hex].map((digit) => digit + digit).join("")
      : hex;
  if (!/^[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(wide)) {
    return { r: 0, g: 0, b: 0, a: 255 };
  }
  const value = Number.parseInt(wide.slice(0, 6), 16);
  return {
    r: (value >> 16) & 255,
    g: (value >> 8) & 255,
    b: value & 255,
    a: wide.length === 8 ? Number.parseInt(wide.slice(6), 16) : 255,
  };
}

/** `source` laid over `backdrop` by its own alpha, as an opaque colour. */
export function flatten(source: Rgba, backdrop: Rgba): Rgba {
  const mix = (top: number, under: number): number =>
    Math.round((top * source.a + under * (255 - source.a)) / 255);
  return {
    r: mix(source.r, backdrop.r),
    g: mix(source.g, backdrop.g),
    b: mix(source.b, backdrop.b),
    a: 255,
  };
}

/**
 * The band colours as the preview paints them: one opaque RGBA entry per band, indexed by the byte
 * native sends. The order is `TERRAIN_ORDER`, which is the native declaration order that
 * `fixtures/terrain-passability.json` pins on both sides of the wire — so this array is the wire
 * format's only reader and needs no table of its own.
 */
export function terrainPalette(backdrop = PREVIEW_BACKDROP): Uint8ClampedArray {
  const under = parseHexColor(backdrop);
  const palette = new Uint8ClampedArray(TERRAIN_ORDER.length * 4);
  TERRAIN_ORDER.forEach((terrain, index) => {
    const { r, g, b, a } = flatten(
      parseHexColor(TERRAIN_INFO[terrain].fill),
      under,
    );
    palette.set([r, g, b, a], index * 4);
  });
  return palette;
}

/**
 * One byte per hex into one RGBA pixel per hex.
 *
 * A byte outside the palette is painted as the backdrop rather than dropped: it would mean native
 * had grown a band this build does not know, and a hole in the picture says so where a silent
 * substitution would not.
 */
export function previewPixels(
  cells: Uint8Array,
  palette = terrainPalette(),
): Uint8ClampedArray {
  const pixels = new Uint8ClampedArray(cells.length * 4);
  const bands = palette.length / 4;
  for (let index = 0; index < cells.length; index += 1) {
    const band = cells[index] ?? 0;
    if (band >= bands) {
      pixels.set([0, 0, 0, 0], index * 4);
      continue;
    }
    const at = band * 4;
    pixels[index * 4] = palette[at] ?? 0;
    pixels[index * 4 + 1] = palette[at + 1] ?? 0;
    pixels[index * 4 + 2] = palette[at + 2] ?? 0;
    pixels[index * 4 + 3] = palette[at + 3] ?? 0;
  }
  return pixels;
}

/**
 * The drawn radius at which a deposit stops being a dot and becomes a disc. Below it a circle is
 * mostly antialiased edge, and a few thousand of those hide the terrain they are drawn over.
 */
export const SITE_DOT_RADIUS = 2;

/**
 * The deposits in a window, in the terms the caption uses.
 *
 * A window wide enough to frame a coastline holds more deposits than native will send, and an empty
 * list then means "too many to draw" rather than "none". Saying which is the whole point of the
 * sentence — a preview that reported zero deposits for a world full of them would be worse than one
 * that reported nothing at all.
 */
export function describeDeposits(preview: {
  total: number;
  dense: boolean;
}): string {
  if (preview.dense) {
    return preview.total > 0
      ? `${String(preview.total)} deposits, too dense to plot at this zoom`
      : "deposits too dense to plot at this zoom";
  }
  return `${String(preview.total)} deposit${preview.total === 1 ? "" : "s"}`;
}

/** What the panel needs to know about a material, and nothing else. The definitions stay outside. */
export interface PreviewItemLook {
  name: string;
  color: string;
}

/** The sentence a refused world is refused with, or null when the bootstrap pass was satisfied. */
export function unmetWarning(
  unmet: readonly number[],
  look: (itemId: number) => PreviewItemLook | undefined,
): string | null {
  if (unmet.length === 0) return null;
  const names = unmet.map(
    (itemId) => look(itemId)?.name ?? `item ${String(itemId)}`,
  );
  // Native refuses to generate this set, so the panel says so here rather than letting the player
  // find out by pressing Start.
  return `No room for ${names.join(", ")} — this world cannot be started.`;
}

/** "a", "a and b", "a, b and c" — a list a sentence can hold. */
export function joinWords(words: readonly string[]): string {
  if (words.length < 2) return words[0] ?? "";
  return `${words.slice(0, -1).join(", ")} and ${String(words[words.length - 1])}`;
}

/** A band as the player has seen it named everywhere else, lowercased for mid-sentence use. */
function bandName(band: string): string {
  const info = TERRAIN_INFO[band as Terrain] as
    | (typeof TERRAIN_INFO)[Terrain]
    | undefined;
  return (info?.name ?? band).toLowerCase();
}

/**
 * What a refused world is actually short of, as sentences the player can act on.
 *
 * Split on `ground` rather than per material, because that is the split that changes the advice.
 * Ground the opening does not hold at all is a parameter problem and no seed will find it; ground it
 * holds but never in a workable patch is exactly what another seed tends to fix. Two sentences at
 * most: a list of six materials with a clause each is a wall nobody reads.
 */
export function describeNeeds(
  needs: readonly WorldPreviewNeed[],
  look: (itemId: number) => PreviewItemLook | undefined,
): string[] {
  const sentences: string[] = [];
  const say = (
    group: readonly WorldPreviewNeed[],
    tail: (bands: string, count: number) => string,
  ): void => {
    if (group.length === 0) return;
    const names = joinWords(
      group.map((need) => look(need.item_id)?.name ?? `item ${need.item_id}`),
    );
    const bands = joinWords([
      ...new Set(group.flatMap((need) => need.bands.map(bandName))),
    ]);
    sentences.push(`${names} ${tail(bands, group.length)}`);
  };
  const verb = (count: number): string => (count === 1 ? "sits" : "sit");
  say(
    needs.filter((need) => !need.ground),
    (bands, count) =>
      `${verb(count)} on ${bands}, and there is none of that ground near the landing site — no seed will find any, so a band cut or the landform scale has to move.`,
  );
  say(
    needs.filter((need) => need.ground),
    (bands, count) =>
      `${verb(count)} on ${bands}, which is there but never in a patch big enough to work — another seed or a closer deposit spacing would seat them.`,
  );
  return sentences;
}

/**
 * One parameter change under the name and unit the form shows it in.
 *
 * Native names the field and the two numbers; everything a player reads comes from the form's own
 * table, which is why the wire carries a diff and not a sentence. A field this build has no entry
 * for still reports something honest rather than nothing.
 */
export function describeChange(
  change: WorldPreviewChange,
  params: WorldParams,
): string {
  const field = WORLD_PARAMETER_FIELDS.find(
    (entry) => entry.key === change.field,
  );
  if (!field) return `${change.field} ${change.from} → ${change.to}`;
  const show = (value: number): number =>
    field.scale ? field.scale.fromParam(value, params) : value;
  return `${field.label} ${show(change.from)} → ${show(change.to)}`;
}

/**
 * The refused-world paragraph: the verdict, then why, or empty when the bootstrap pass was
 * satisfied. Kept as one string so the live region announces a world that cannot start as a
 * single update rather than as a warning and then a second hint.
 */
export function refusedStatus(
  preview: Pick<WorldPreview, "unmet" | "needs">,
  look: (itemId: number) => PreviewItemLook | undefined,
): string {
  const warning = unmetWarning(preview.unmet, look);
  if (!warning) return "";
  return [warning, ...describeNeeds(preview.needs, look)].join(" ");
}

/** Button copy for the two verified ways out, or null on a half that was not found. */
export function repairLabels(
  repair: WorldPreviewRepair | null,
  params: WorldParams,
): { seed: string | null; params: string | null } {
  if (!repair) return { seed: null, params: null };
  return {
    seed: repair.seed === null ? null : `Try seed ${String(repair.seed)}`,
    params:
      repair.changes.length === 0
        ? null
        : `Fix ${joinWords(repair.changes.map((change) => describeChange(change, params)))}`,
  };
}

/**
 * The host's application of a parameter repair: the named knobs, and nothing else. Native already
 * verified the result; this is only how a diff becomes a `WorldParams` the form can show.
 */
export function applyChanges(
  params: WorldParams,
  changes: readonly WorldPreviewChange[],
): WorldParams {
  const next = { ...params };
  for (const change of changes) {
    const field = WORLD_PARAMETER_FIELDS.find(
      (entry) => entry.key === change.field,
    );
    if (!field) continue;
    next[field.key] = change.to;
  }
  return next;
}

/** Which repair the player pressed. Applying it is the form's business, not the panel's. */
export type RepairChoice =
  | { kind: "seed"; seed: number }
  | { kind: "params"; changes: WorldPreviewChange[] };

/**
 * The preview as a control: a canvas, a zoom picker, the lines that explain them, and the two
 * buttons a refused world can offer.
 *
 * Built once and only ever written to, like every other control on this form. Nothing here decides
 * *when* to redraw — the panel reports a zoom change or a repair press and waits to be handed a
 * picture, so the one place that debounces and dispatches stays the one place that does.
 */
export class WorldPreviewPanel {
  readonly element: HTMLElement;
  private readonly canvas: HTMLCanvasElement;
  private readonly context: CanvasRenderingContext2D | null;
  private readonly status: HTMLElement;
  private readonly caption: HTMLElement;
  private readonly repairs: HTMLElement;
  private readonly seedButton: HTMLButtonElement;
  private readonly paramsButton: HTMLButtonElement;
  private readonly zoomButtons = new Map<string, HTMLButtonElement>();
  private readonly palette = terrainPalette();
  private zoom: PreviewZoom = PREVIEW_ZOOMS[0] as PreviewZoom;
  private seedChoice: number | null = null;
  private paramChanges: WorldPreviewChange[] = [];

  constructor(
    idPrefix: string,
    private readonly look: (itemId: number) => PreviewItemLook | undefined,
    private readonly onZoomChange: (zoom: PreviewZoom) => void,
    private readonly onRepair: (choice: RepairChoice) => void,
  ) {
    this.element = document.createElement("div");
    this.element.className = "world-preview";

    this.canvas = document.createElement("canvas");
    this.canvas.className = "world-preview-canvas";
    this.canvas.width = PREVIEW_WIDTH;
    this.canvas.height = PREVIEW_HEIGHT;
    this.canvas.setAttribute("role", "img");
    this.canvas.setAttribute("aria-label", "World preview");
    this.context = this.canvas.getContext("2d");

    const zooms = document.createElement("div");
    zooms.className = "world-preview-zooms";
    zooms.setAttribute("role", "group");
    zooms.setAttribute("aria-label", "Preview zoom");
    for (const zoom of PREVIEW_ZOOMS) {
      const button = document.createElement("button");
      button.type = "button";
      button.id = `${idPrefix}-zoom-${zoom.key}`;
      button.className = "world-preview-zoom";
      button.textContent = zoom.label;
      button.title = `${String(zoom.hexesAcross)} hexes across · ${zoom.note}`;
      button.setAttribute("aria-pressed", String(zoom === this.zoom));
      button.addEventListener("click", () => this.setZoom(zoom));
      this.zoomButtons.set(zoom.key, button);
      zooms.append(button);
    }

    this.caption = document.createElement("p");
    this.caption.className = "world-preview-caption";
    this.status = document.createElement("p");
    this.status.className = "world-preview-status";
    // Polite rather than off: this is where a world that cannot be generated says so, and that is
    // worth announcing without the player having to go looking for it.
    this.status.setAttribute("aria-live", "polite");

    this.repairs = document.createElement("div");
    this.repairs.className = "world-preview-repairs";
    this.repairs.setAttribute("role", "group");
    this.repairs.setAttribute("aria-label", "Fix this world");
    this.repairs.hidden = true;
    this.seedButton = this.buildRepairButton(`${idPrefix}-repair-seed`, () => {
      if (this.seedChoice !== null) {
        this.onRepair({ kind: "seed", seed: this.seedChoice });
      }
    });
    this.paramsButton = this.buildRepairButton(
      `${idPrefix}-repair-params`,
      () => {
        if (this.paramChanges.length > 0) {
          this.onRepair({ kind: "params", changes: this.paramChanges });
        }
      },
    );
    this.repairs.append(this.seedButton, this.paramsButton);

    this.element.append(
      zooms,
      this.canvas,
      this.caption,
      this.status,
      this.repairs,
    );
    this.showCaption();
  }

  /** The span the next request should ask native for. */
  get hexesAcross(): number {
    return this.zoom.hexesAcross;
  }

  /**
   * Whether the picture is on screen at all, so a form nobody is looking at costs no raster.
   *
   * Asked of the canvas rather than of `element`. The title screen lays this panel out with
   * `display: contents` so its parts can join the surrounding grid, and an element with no box of
   * its own reports no client rects — the wrapper would answer "hidden" while the picture inside it
   * was plainly visible.
   */
  get visible(): boolean {
    return this.canvas.getClientRects().length > 0;
  }

  /** Paint a raster native has just sent back. */
  draw(preview: WorldPreview, params: WorldParams): void {
    const context = this.context;
    if (!context) return;
    const pixels = previewPixels(preview.cells, this.palette);
    if (pixels.length === preview.width * preview.height * 4) {
      // Sized from what arrived rather than from what was asked for: native clamps the window, so
      // a picture smaller than the canvas is a legal answer and not a reason to stretch it.
      this.canvas.width = preview.width;
      this.canvas.height = preview.height;
      context.putImageData(
        new ImageData(pixels, preview.width, preview.height),
        0,
        0,
      );
    }
    this.drawSites(context, preview);
    const deposits = describeDeposits(preview);
    this.canvas.setAttribute(
      "aria-label",
      `World preview, ${String(this.zoom.hexesAcross)} hexes across, ${deposits}`,
    );
    this.showCaption(deposits);
    this.status.textContent = refusedStatus(preview, this.look);
    this.status.classList.toggle("is-warning", preview.unmet.length > 0);
    this.showRepairs(preview.repair, params);
  }

  /** Say why there is no picture. A refused parameter set is the usual reason. */
  showError(message: string): void {
    this.status.textContent = message;
    this.status.classList.add("is-warning");
    this.showRepairs(null, null);
  }

  private drawSites(
    context: CanvasRenderingContext2D,
    preview: WorldPreview,
  ): void {
    context.save();
    context.lineWidth = 1;
    context.globalAlpha = 0.85;
    context.strokeStyle = "#06100ecc";
    for (const site of preview.sites) {
      context.fillStyle = this.look(site.item_id)?.color ?? "#d8e6df";
      if (site.radius < SITE_DOT_RADIUS) {
        // Under a couple of pixels a circle is a smear of half-lit edges. One square pixel per
        // deposit reads as the density it is, and leaves the terrain under it visible.
        context.fillRect(Math.round(site.x), Math.round(site.y), 1, 1);
        continue;
      }
      context.beginPath();
      context.arc(site.x, site.y, site.radius, 0, Math.PI * 2);
      context.fill();
      context.stroke();
    }
    context.restore();
  }

  private buildRepairButton(
    id: string,
    onClick: () => void,
  ): HTMLButtonElement {
    const button = document.createElement("button");
    button.type = "button";
    button.id = id;
    button.className = "world-preview-repair";
    button.hidden = true;
    button.addEventListener("click", onClick);
    return button;
  }

  /**
   * The two buttons, patched in place. They are built once because a list of controls that is
   * thrown away and rebuilt under a pointer loses the control that was just pressed.
   */
  private showRepairs(
    repair: WorldPreviewRepair | null,
    params: WorldParams | null,
  ): void {
    const labels = params
      ? repairLabels(repair, params)
      : { seed: null, params: null };
    this.seedChoice = repair?.seed ?? null;
    this.paramChanges = repair?.changes.slice() ?? [];
    this.seedButton.hidden = labels.seed === null;
    this.seedButton.textContent = labels.seed ?? "";
    this.seedButton.title = labels.seed ?? "";
    this.paramsButton.hidden = labels.params === null;
    this.paramsButton.textContent = labels.params ?? "";
    this.paramsButton.title = labels.params ?? "";
    this.repairs.hidden = this.seedButton.hidden && this.paramsButton.hidden;
  }

  private setZoom(zoom: PreviewZoom): void {
    if (zoom === this.zoom) return;
    this.zoom = zoom;
    for (const [key, button] of this.zoomButtons) {
      button.setAttribute("aria-pressed", String(key === zoom.key));
    }
    this.showCaption();
    this.onZoomChange(zoom);
  }

  private showCaption(deposits?: string): void {
    const tail = deposits === undefined ? "" : ` · ${deposits}`;
    this.caption.textContent = `${String(this.zoom.hexesAcross)} hexes across · ${this.zoom.note}${tail}`;
  }
}
