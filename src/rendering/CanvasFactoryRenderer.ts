import {
  axialToPixel,
  pixelToAxial,
  rotateAxial,
  type AxialCoordinate,
  type PixelPoint,
} from "@hexlife/embed/hex";

import type {
  BuildingDefinition,
  ChunkSnapshot,
  Definitions,
  EntitySnapshot,
  FactorySnapshot,
  ItemDefinition,
  LinePreviewCell,
  PlacementPreview,
  WorldPoint,
} from "../core/types";
import {
  BASE_HEX_SIZE,
  BUILDING_COLORS,
  MAX_DEVICE_PIXEL_RATIO,
  type FactoryRenderer,
  type GraphicsProfile,
  type ReachRadii,
  type RendererDiagnostics,
} from "./FactoryRenderer";
import {
  cargoTravel,
  facingTip,
  NORTH,
  PLAYER_BODY,
  PLAYER_RING,
  partsFor,
  silhouetteOf,
  spanEnd,
  stallMark,
  trimOf,
  workCycle,
} from "./buildingLook";
import { isStill, drawParts } from "./shapeGrammar";
import type { ShapePart } from "./shapeGrammar";
import { drawItemIcon } from "./icons";
import { WORLD_SCALE, homeBearing } from "./landmarks";
import { WorldGl } from "./gl/WorldGl";

export { BASE_HEX_SIZE, BUILDING_COLORS, MAX_DEVICE_PIXEL_RATIO };

const TREE_TRUNK: readonly ShapePart[] = [
  { part: "mast", x: 0, y: 0.16, scale: 0.18 },
];
const TREE_CANOPY: readonly ShapePart[] = [
  { part: "rotor", x: 0, y: -0.1, scale: 0.3, count: 5 },
];

/**
 * True when a world point lies inside a chunk the native simulation has generated. Chunks are the
 * unit of world generation, so anything outside them is unexplored world drawn as fog.
 */
export function isSurveyed(
  chunks: ChunkSnapshot[],
  point: WorldPoint,
): boolean {
  return chunks.some(
    (chunk) =>
      point.x >= chunk.x &&
      point.x < chunk.x + chunk.span &&
      point.y >= chunk.y &&
      point.y < chunk.y + chunk.span,
  );
}

export class HexCamera {
  center: WorldPoint = { x: 0, y: 0 };
  pan: PixelPoint = { x: 0, y: 0 };
  zoom = 1;
  following = true;

  origin(width: number, height: number): PixelPoint {
    const scale = (BASE_HEX_SIZE * this.zoom) / WORLD_SCALE;
    return {
      x: width / 2 + this.pan.x - this.center.x * scale,
      y: height / 2 + this.pan.y - this.center.y * scale,
    };
  }

  pick(point: PixelPoint, width: number, height: number): AxialCoordinate {
    return pixelToAxial(
      point,
      BASE_HEX_SIZE * this.zoom,
      this.origin(width, height),
    );
  }

  project(
    coordinate: AxialCoordinate,
    width: number,
    height: number,
  ): PixelPoint {
    return axialToPixel(
      coordinate,
      BASE_HEX_SIZE * this.zoom,
      this.origin(width, height),
    );
  }

  projectWorld(point: WorldPoint, width: number, height: number): PixelPoint {
    const origin = this.origin(width, height);
    const scale = (BASE_HEX_SIZE * this.zoom) / WORLD_SCALE;
    return { x: origin.x + point.x * scale, y: origin.y + point.y * scale };
  }

  /**
   * The world position under a viewport point — {@link projectWorld} inverted. It is what an aim
   * carries: the host names the point the cursor is over and native turns it into a facing.
   */
  worldAt(point: PixelPoint, width: number, height: number): WorldPoint {
    const origin = this.origin(width, height);
    const scale = (BASE_HEX_SIZE * this.zoom) / WORLD_SCALE;
    return {
      x: Math.round((point.x - origin.x) / scale),
      y: Math.round((point.y - origin.y) / scale),
    };
  }

  follow(point: WorldPoint): void {
    if (!this.following) return;
    this.center = { ...point };
    this.pan = { x: 0, y: 0 };
  }

  recenter(point: WorldPoint): void {
    this.following = true;
    this.center = { ...point };
    this.pan = { x: 0, y: 0 };
  }

  panBy(x: number, y: number): void {
    this.following = false;
    this.pan.x += x;
    this.pan.y += y;
  }

  zoomAt(
    factor: number,
    point: PixelPoint,
    width: number,
    height: number,
  ): void {
    // A follow zoom is about the player, not the cursor. Anchoring to the pointer and dropping
    // follow made the wheel feel like a pan, and Space had to put the camera back every time.
    if (this.following) {
      this.zoom = Math.max(0.55, Math.min(2.2, this.zoom * factor));
      this.pan = { x: 0, y: 0 };
      return;
    }
    const before = this.pick(point, width, height);
    this.zoom = Math.max(0.55, Math.min(2.2, this.zoom * factor));
    const projected = this.project(before, width, height);
    this.pan.x += point.x - projected.x;
    this.pan.y += point.y - projected.y;
  }
}

export class CanvasFactoryRenderer implements FactoryRenderer {
  readonly camera = new HexCamera();
  private readonly context: CanvasRenderingContext2D;
  private readonly overlay: HTMLCanvasElement;
  private readonly world: WorldGl;
  private readonly itemsById: ReadonlyMap<number, ItemDefinition>;
  private readonly buildingsById: ReadonlyMap<number, BuildingDefinition>;
  /**
   * Motion is off when the system asks for it off, or when the player does. The system preference
   * is the default and is never overridden downward: a player who set it at the operating system
   * has already answered, and an in-game switch that could turn animation back on for them would
   * be the game arguing with an accessibility setting.
   */
  private readonly systemReducedMotion = matchMedia(
    "(prefers-reduced-motion: reduce)",
  ).matches;
  private forcedReducedMotion = false;
  private get reducedMotion(): boolean {
    return this.systemReducedMotion || this.forcedReducedMotion;
  }
  private snapshot: FactorySnapshot | null = null;
  /**
   * Where the landing hub stands, so the view can always say which way home is. Resolved by the
   * host from the snapshot rather than scanned for here every frame — the hub does not move.
   */
  private home: WorldPoint | null = null;
  private hover: AxialCoordinate | null = null;
  private selection: AxialCoordinate | null = null;
  private placement: PlacementPreview | null = null;
  private buildMode = false;
  private gridToggled = false;
  private buildFootprint: AxialCoordinate[] = [{ q: 0, r: 0 }];
  private buildReach: ReachRadii | null = null;
  private gathering = false;
  private dragPath: LinePreviewCell[] = [];
  private now = 0;
  private cargoTickAt = 0;
  private cargoTickMs = 250;
  private needsDraw = true;
  private layoutDirty = true;
  private layout = { width: 1, height: 1, ratio: 1, left: 0, top: 0 };
  private profile: GraphicsProfile = "low";

  constructor(
    private readonly canvas: HTMLCanvasElement,
    definitions: Definitions,
  ) {
    const gl = canvas.getContext("webgl2", {
      alpha: false,
      antialias: false,
      powerPreference: "high-performance",
      premultipliedAlpha: false,
    });
    if (!gl) throw new Error("WebGL2 is unavailable");
    this.world = new WorldGl(gl, definitions, BUILDING_COLORS, BASE_HEX_SIZE);
    this.overlay = document.createElement("canvas");
    this.overlay.className = "factory-overlay";
    this.overlay.setAttribute("aria-hidden", "true");
    canvas.insertAdjacentElement("afterend", this.overlay);
    const context = this.overlay.getContext("2d");
    if (!context) throw new Error("Canvas 2D overlay is unavailable");
    this.context = context;
    this.itemsById = new Map(definitions.items.map((item) => [item.id, item]));
    this.buildingsById = new Map(
      definitions.buildings.map((building) => [building.id, building]),
    );
    new ResizeObserver(() => {
      this.layoutDirty = true;
      this.markDirty();
    }).observe(canvas);
  }

  setSnapshot(snapshot: FactorySnapshot): void {
    const receivedAt = performance.now();
    if (
      !this.snapshot ||
      snapshot.seed !== this.snapshot.seed ||
      snapshot.scenario !== this.snapshot.scenario ||
      snapshot.tick < this.snapshot.tick
    ) {
      this.cargoTickAt = receivedAt;
    } else if (snapshot.tick > this.snapshot.tick) {
      const measured =
        (receivedAt - this.cargoTickAt) / (snapshot.tick - this.snapshot.tick);
      if (measured >= 16 && measured <= 4_000) this.cargoTickMs = measured;
      this.cargoTickAt = receivedAt;
    }
    this.snapshot = snapshot;
    this.camera.follow(snapshot.player);
    this.markDirty();
  }

  setHome(point: WorldPoint | null): void {
    this.home = point;
  }

  /** The player's own answer, on top of the system's. It can only ever reduce motion further. */
  setReducedMotion(value: boolean): void {
    this.forcedReducedMotion = value;
    this.markDirty();
  }

  get motionReduced(): boolean {
    return this.reducedMotion;
  }

  /** The flat renderer has no orbit to sweep, so its view is never mid-turn. */
  get cameraSettling(): boolean {
    return false;
  }

  setHover(
    coordinate: AxialCoordinate | null,
    placement: PlacementPreview | null = null,
  ): void {
    this.hover = coordinate;
    this.placement = placement;
    this.markDirty();
  }

  setSelection(coordinate: AxialCoordinate | null): void {
    this.selection = coordinate;
    this.markDirty();
  }

  setBuildMode(active: boolean): void {
    this.buildMode = active;
    this.markDirty();
  }

  /**
   * The cells the in-progress drag covers, exactly as native resolved them. Presentation only: the
   * renderer draws this list and never computes a path of its own.
   */
  setDragPath(cells: LinePreviewCell[]): void {
    this.dragPath = cells;
    this.markDirty();
  }

  setBuildFootprint(footprint: AxialCoordinate[], orientation: number): void {
    this.buildFootprint = footprint.map((cell) =>
      rotateAxial(cell, orientation, { q: 0, r: 0 }),
    );
    this.markDirty();
  }

  /** Every reach the pending definition states, passed through without deriving a default. */
  setBuildReach(definition: BuildingDefinition | null): void {
    this.buildReach = definition
      ? {
          extract: definition.extract_radius ?? null,
          supply: definition.supply_radius ?? null,
          link: definition.pole_reach ?? null,
        }
      : null;
    this.markDirty();
  }

  setGathering(active: boolean): void {
    if (this.gathering === active) return;
    this.gathering = active;
    this.markDirty();
  }

  toggleGrid(): boolean {
    this.gridToggled = !this.gridToggled;
    this.markDirty();
    return this.gridToggled;
  }

  pick(clientX: number, clientY: number): AxialCoordinate {
    this.syncLayout();
    return this.camera.pick(
      { x: clientX - this.layout.left, y: clientY - this.layout.top },
      this.layout.width,
      this.layout.height,
    );
  }

  /** The world position a pointer is over, for the `aim` the host sends from it. */
  pickWorld(clientX: number, clientY: number): WorldPoint {
    this.syncLayout();
    return this.camera.worldAt(
      { x: clientX - this.layout.left, y: clientY - this.layout.top },
      this.layout.width,
      this.layout.height,
    );
  }

  /** Development fallback only: its unrotated camera already shares the native axes. */
  screenMovement(x: number, y: number): WorldPoint {
    return { x, y };
  }

  panBy(x: number, y: number): void {
    this.camera.panBy(x, y);
    this.markDirty();
  }

  zoomAt(clientX: number, clientY: number, factor: number): void {
    this.syncLayout();
    this.camera.zoomAt(
      factor,
      { x: clientX - this.layout.left, y: clientY - this.layout.top },
      this.layout.width,
      this.layout.height,
    );
    this.markDirty();
  }

  recenter(): void {
    if (this.snapshot) this.camera.recenter(this.snapshot.player);
    this.markDirty();
  }

  /** Development fallback only: the legacy flat renderer has no orbit. */
  orbitBy(): void {}

  setGraphicsProfile(profile: GraphicsProfile): void {
    this.profile = profile;
  }

  getGraphicsProfile(): GraphicsProfile {
    return this.profile;
  }

  getDiagnostics(): RendererDiagnostics {
    return {
      name: "Legacy hybrid WebGL2/Canvas",
      profile: this.profile,
      drawCalls: 0,
      triangles: 0,
      geometries: 0,
      textures: 0,
      cpuPreparationUs: 0,
      contextLost: this.world.isLost,
      pixelRatio: this.layout.ratio,
      frameP95Us: 0,
      frameSamples: 0,
    };
  }

  dispose(): void {
    this.overlay.remove();
  }

  renderFrame(now: number): void {
    this.now = now;
    if (this.needsDraw || !this.reducedMotion) this.draw();
    this.needsDraw = false;
  }

  private markDirty(): void {
    this.needsDraw = true;
  }

  /**
   * Cache the canvas box once per resize. Reading `getBoundingClientRect` after a draw invalidates
   * layout, and doing that from `pickWorld` every frame is what made aiming hitch.
   */
  private syncLayout(): void {
    if (!this.layoutDirty) return;
    const rect = this.canvas.getBoundingClientRect();
    this.layout.left = rect.left;
    this.layout.top = rect.top;
    this.layout.width = Math.max(1, this.canvas.clientWidth);
    this.layout.height = Math.max(1, this.canvas.clientHeight);
    this.layout.ratio = Math.min(
      window.devicePixelRatio || 1,
      MAX_DEVICE_PIXEL_RATIO,
    );
    this.layoutDirty = false;
  }

  draw(): void {
    this.syncLayout();
    const { width, height, ratio } = this.layout;
    const overlay = this.overlay;
    const targetW = Math.floor(width * ratio);
    const targetH = Math.floor(height * ratio);
    if (overlay.width !== targetW || overlay.height !== targetH) {
      overlay.width = targetW;
      overlay.height = targetH;
    }
    const ctx = this.context;
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.clearRect(0, 0, width, height);
    if (!this.snapshot) return;
    const size = BASE_HEX_SIZE * this.camera.zoom;
    const origin = this.camera.origin(width, height);
    this.world.draw(
      this.snapshot,
      origin,
      width,
      height,
      ratio,
      this.camera.zoom,
      this.now,
      this.reducedMotion,
      {
        hover: this.hover,
        selection: this.selection,
        placement: this.placement,
        dragPath: this.dragPath,
        buildMode: this.buildMode,
        gridToggled: this.gridToggled,
        buildFootprint: this.buildFootprint,
        buildReach: this.buildReach,
        gathering: this.gathering,
      },
    );
    if (this.buildMode) this.drawBuildRange(width, height);
    this.drawForest(width, height, size);
    for (const building of this.snapshot.buildings)
      this.drawBuilding(building, width, height, size);
    this.drawGroundItems(width, height, size);
    this.drawFog(width, height);
    this.drawEnvironment(width, height, size);
    this.drawPlayer(width, height, size);
    this.drawHomeMarker(width, height);
    if (this.dragPath.length) this.drawDragPath(width, height, size);
  }

  /**
   * The run the current drag would build, cell by cell, in the native headings. Cells the drag
   * cannot use are drawn in the refusal colour rather than hidden, so a run that stops short of the
   * cursor shows where and not merely that.
   */
  private drawDragPath(width: number, height: number, size: number): void {
    const ctx = this.context;
    for (const cell of this.dragPath) {
      if (!cell.legal) continue;
      const center = this.camera.project(cell, width, height);
      if (!visible(center, size, width, height)) continue;
      const tip = facingTip(center, size, cell.orientation, 0.34);
      ctx.strokeStyle = "#76e0aa";
      ctx.lineWidth = Math.max(2, size * 0.07);
      ctx.beginPath();
      ctx.moveTo(center.x, center.y);
      ctx.lineTo(tip.x, tip.y);
      ctx.stroke();
    }
  }

  private drawEnvironment(width: number, height: number, size: number): void {
    if (!this.snapshot) return;
    for (const resource of this.snapshot.resources) {
      const center = this.camera.project(
        { q: resource.q, r: resource.r },
        width,
        height,
      );
      if (!visible(center, size, width, height)) continue;
      const item = this.itemsById.get(resource.item_id);
      const hovered =
        this.hover !== null &&
        this.hover.q === resource.q &&
        this.hover.r === resource.r;
      const drawnFrom = resource.quantity < resource.initial_quantity;
      if (hovered) {
        this.drawFieldLabel(
          center,
          size,
          item?.name ?? "Resource",
          resource.quantity,
          true,
        );
      } else if (drawnFrom && size >= 16) {
        this.drawFieldLabel(center, size, null, resource.quantity, false);
      }
    }
  }

  /** One deterministic tree per remaining wood unit, so cutting and regrowth redraw the forest. */
  private drawForest(width: number, height: number, size: number): void {
    if (!this.snapshot) return;
    for (const resource of this.snapshot.resources) {
      const item = this.itemsById.get(resource.item_id);
      if (item?.key !== "wood" || resource.quantity <= 0) continue;
      const center = this.camera.project(resource, width, height);
      if (!visible(center, size, width, height)) continue;
      for (let unit = 0; unit < resource.quantity; unit += 1) {
        const angle = forestHash(resource.q, resource.r, unit) * Math.PI * 2;
        const distance =
          (0.08 + forestHash(resource.r, resource.q, unit + 31) * 0.28) * size;
        const tree = {
          x: center.x + Math.cos(angle) * distance,
          y: center.y + Math.sin(angle) * distance,
        };
        drawParts(this.context, TREE_TRUNK, tree, size * 0.68, "#7c5a34", 0);
        drawParts(this.context, TREE_CANOPY, tree, size * 0.68, "#8fc56a", 0);
      }
    }
  }

  /**
   * Which way the landing hub is, whenever it is not on screen. A minimap answers this only while
   * home is still on the minimap; this answers it at any distance, which is what turns walking to
   * the survey frontier into a decision rather than a risk.
   */
  private drawHomeMarker(width: number, height: number): void {
    if (!this.snapshot || !this.home) return;
    const target = this.camera.projectWorld(this.home, width, height);
    const margin = 44;
    if (
      target.x >= margin &&
      target.y >= margin &&
      target.x <= width - margin &&
      target.y <= height - margin
    )
      return;
    const bearing = homeBearing(this.snapshot.player, this.home);
    if (!bearing) return;
    const x = Math.min(width - margin, Math.max(margin, target.x));
    const y = Math.min(height - margin, Math.max(margin, target.y));
    const angle = Math.atan2(bearing.y, bearing.x);
    const ctx = this.context;
    ctx.save();
    ctx.translate(x, y);
    ctx.rotate(angle);
    ctx.fillStyle = "#f6c85f";
    ctx.strokeStyle = "#2a2208";
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(17, 0);
    ctx.lineTo(-9, 11);
    ctx.lineTo(-4, 0);
    ctx.lineTo(-9, -11);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();
    ctx.restore();
    const label = `⌂ ${bearing.hexes} hex`;
    ctx.font = "700 11px system-ui";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    const labelWidth = ctx.measureText(label).width + 12;
    const labelX = x - bearing.x * 26;
    const labelY = y - bearing.y * 26;
    ctx.fillStyle = "#07110ef2";
    ctx.beginPath();
    ctx.roundRect(labelX - labelWidth / 2, labelY - 9, labelWidth, 18, 7);
    ctx.fill();
    ctx.fillStyle = "#f6c85f";
    ctx.fillText(label, labelX, labelY);
  }

  /**
   * A single field's remaining amount. Hover includes the item name because the glyph alone
   * does not say "Signal crystal"; depleted cells keep the count so a draw is visible.
   */
  private drawFieldLabel(
    center: PixelPoint,
    size: number,
    name: string | null,
    quantity: number,
    prominent: boolean,
  ): void {
    const ctx = this.context;
    const text = name ? `${name}  ${quantity}` : String(quantity);
    const fontSize = prominent
      ? Math.max(10, size * 0.28)
      : Math.max(9, size * 0.22);
    ctx.font = `700 ${fontSize}px system-ui`;
    ctx.textAlign = "center";
    ctx.textBaseline = "alphabetic";
    const width = Math.max(24, ctx.measureText(text).width + 12);
    const height = prominent ? 16 : 14;
    const y = center.y + size * (prominent ? 0.42 : 0.28);
    ctx.fillStyle = prominent ? "#07110ef2" : "#07110ee8";
    ctx.beginPath();
    ctx.roundRect(center.x - width / 2, y, width, height, 6);
    ctx.fill();
    ctx.fillStyle = "#f4f7f5";
    ctx.fillText(text, center.x, y + height - 4);
  }

  /**
   * The surveyed world's edge, from native chunk bounds. The veil itself is the WebGL coverage
   * pass; this is only the dashed frontier so overlapping chunks cannot invent a second geometry.
   */
  private drawFog(width: number, height: number): void {
    if (!this.snapshot) return;
    const chunks = this.snapshot.chunks;
    if (!chunks.length) return;
    const scale = (BASE_HEX_SIZE * this.camera.zoom) / WORLD_SCALE;
    const surveyed = chunks.map((chunk) => {
      const origin = this.camera.projectWorld(chunk, width, height);
      return { chunk, x: origin.x, y: origin.y, size: chunk.span * scale };
    });
    const generated = new Set(
      chunks.map(({ chunk_q, chunk_r }) => `${chunk_q},${chunk_r}`),
    );
    const ctx = this.context;
    ctx.strokeStyle = "#7fe0c088";
    ctx.lineWidth = 2;
    ctx.setLineDash([9, 8]);
    ctx.beginPath();
    for (const { chunk, x, y, size } of surveyed) {
      if (x > width || y > height || x + size < 0 || y + size < 0) continue;
      const frontier = (dq: number, dr: number): boolean =>
        !generated.has(`${chunk.chunk_q + dq},${chunk.chunk_r + dr}`);
      if (frontier(-1, 0)) {
        ctx.moveTo(x, y);
        ctx.lineTo(x, y + size);
      }
      if (frontier(1, 0)) {
        ctx.moveTo(x + size, y);
        ctx.lineTo(x + size, y + size);
      }
      if (frontier(0, -1)) {
        ctx.moveTo(x, y);
        ctx.lineTo(x + size, y);
      }
      if (frontier(0, 1)) {
        ctx.moveTo(x, y + size);
        ctx.lineTo(x + size, y + size);
      }
    }
    ctx.stroke();
    ctx.setLineDash([]);
  }

  private drawBuildRange(width: number, height: number): void {
    if (!this.snapshot) return;
    const center = this.camera.projectWorld(
      this.snapshot.player,
      width,
      height,
    );
    const radius =
      (this.snapshot.player.build_range * BASE_HEX_SIZE * this.camera.zoom) /
      WORLD_SCALE;
    this.context.strokeStyle = "#f2cc6577";
    this.context.lineWidth = 2;
    this.context.setLineDash([7, 7]);
    this.context.beginPath();
    this.context.arc(center.x, center.y, radius, 0, Math.PI * 2);
    this.context.stroke();
    this.context.setLineDash([]);
  }

  private drawBuilding(
    building: EntitySnapshot,
    width: number,
    height: number,
    size: number,
  ): void {
    const ctx = this.context;
    const color = BUILDING_COLORS[building.kind];
    const definition = this.buildingsById.get(building.definition_id);
    const center = this.camera.project(building, width, height);
    if (!visible(center, size, width, height)) return;
    if (building.orientation >= NORTH) {
      const far = spanEnd(center, size, building.orientation);
      ctx.strokeStyle = `${color}cc`;
      ctx.lineWidth = Math.max(2, size * 0.16);
      ctx.lineCap = "round";
      ctx.beginPath();
      ctx.moveTo(center.x, center.y);
      ctx.lineTo(far.x, far.y);
      ctx.stroke();
    }
    const growth =
      building.kind === "hub" ? (this.snapshot?.contract.stage ?? 0) : 0;
    const key = silhouetteOf(
      building.kind,
      definition?.recipe_category,
      definition?.power_source,
    );
    const moving = partsFor(key, definition?.tier ?? 0, growth).filter(
      (part) => !isStill(part),
    );
    if (moving.length)
      drawParts(
        ctx,
        moving,
        center,
        size,
        trimOf(definition?.tier).stroke,
        workCycle(building, this.now, this.reducedMotion),
      );
    const tip = facingTip(center, size, building.orientation);
    ctx.strokeStyle = "#f3f7fa";
    ctx.lineWidth = Math.max(2, size * 0.08);
    ctx.beginPath();
    ctx.moveTo(center.x, center.y);
    ctx.lineTo(tip.x, tip.y);
    ctx.stroke();
    // The stamp is a label, not the machine. Drawn across the middle in bright white it covered the
    // anatomy it was labelling, and a playtest found it doing all the identifying work at ordinary
    // zoom — which made every machine the same drawing with different letters on it. It sits under
    // the body now, smaller and quieter, so the shape is what the eye reaches first.
    ctx.fillStyle = "#dbe9e2c9";
    ctx.font = `800 ${Math.max(7, size * 0.2)}px system-ui`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(
      definition?.icon ?? building.kind.slice(0, 3).toUpperCase(),
      center.x,
      center.y + size * 0.46,
    );
    // Why a machine is doing nothing, where it is doing nothing. Published status, one dot.
    const stall = stallMark(building.status);
    if (stall) {
      ctx.fillStyle = stall;
      ctx.beginPath();
      ctx.arc(
        center.x - size * 0.44,
        center.y - size * 0.44,
        Math.max(2.5, size * 0.13),
        0,
        Math.PI * 2,
      );
      ctx.fill();
      ctx.strokeStyle = "#07100fcc";
      ctx.lineWidth = 1;
      ctx.stroke();
    }
    if (building.progress_total > 0 && building.progress > 0) {
      ctx.strokeStyle = "#f5d572";
      ctx.lineWidth = Math.max(2, size * 0.1);
      ctx.beginPath();
      ctx.arc(
        center.x,
        center.y,
        size * 0.61,
        -Math.PI / 2,
        -Math.PI / 2 +
          (Math.PI * 2 * building.progress) / building.progress_total,
      );
      ctx.stroke();
    }
    const quantity = building.inventory.reduce(
      (sum, item) => sum + item.quantity,
      0,
    );
    if (quantity > 0) {
      ctx.fillStyle = "#07100fdd";
      ctx.beginPath();
      ctx.arc(
        center.x + size * 0.47,
        center.y - size * 0.4,
        size * 0.22,
        0,
        Math.PI * 2,
      );
      ctx.fill();
      ctx.fillStyle = "#fff";
      ctx.font = `bold ${Math.max(9, size * 0.25)}px system-ui`;
      ctx.fillText(
        String(quantity),
        center.x + size * 0.47,
        center.y - size * 0.4,
      );
    }
    if (building.cargo) {
      const item = this.itemsById.get(building.cargo.item_id);
      const travel = cargoTravel(
        this.now - this.cargoTickAt,
        this.cargoTickMs,
        this.reducedMotion,
        building.status === "output blocked",
      );
      const cargoPoint = {
        x: center.x + (tip.x - center.x) * travel,
        y: center.y + (tip.y - center.y) * travel,
      };
      ctx.beginPath();
      ctx.arc(
        cargoPoint.x,
        cargoPoint.y,
        Math.max(6, size * 0.2),
        0,
        Math.PI * 2,
      );
      ctx.fillStyle = item?.color ?? "#fff";
      ctx.shadowColor = item?.color ?? "#fff";
      ctx.shadowBlur = 10;
      ctx.fill();
      ctx.shadowBlur = 0;
      if (item)
        drawItemIcon(
          ctx,
          item.icon,
          item.color,
          cargoPoint.x,
          cargoPoint.y,
          size * 0.55,
        );
    }
  }

  private drawPlayer(width: number, height: number, size: number): void {
    if (!this.snapshot) return;
    const player = this.snapshot.player;
    const center = this.camera.projectWorld(player, width, height);
    const scale = size / WORLD_SCALE;
    const radius = player.radius * scale;
    const length = radius * 1.15;
    const tip = {
      x: center.x + (player.facing_x / 1000) * length,
      y: center.y + (player.facing_y / 1000) * length,
    };
    const ctx = this.context;
    // The player is a part list like every machine is, so the world reads as one visual system
    // rather than three that happen to share a palette. The heading tick below stays outside the
    // grammar for the same reason a building's does: it is an indicator, not anatomy.
    drawParts(ctx, PLAYER_RING, center, radius, "#72e2b477", 0);
    drawParts(ctx, PLAYER_BODY, center, radius, "#142028", 0);
    ctx.beginPath();
    ctx.arc(tip.x, tip.y, Math.max(3, size * 0.08), 0, Math.PI * 2);
    ctx.fillStyle = "#ef6f61";
    ctx.fill();
    ctx.textBaseline = "alphabetic";
    ctx.strokeStyle = "#ef6f61";
    ctx.lineWidth = 4;
    ctx.beginPath();
    ctx.moveTo(center.x, center.y);
    ctx.lineTo(tip.x, tip.y);
    ctx.stroke();
    this.drawActionCooldown(center, radius);
  }

  /**
   * The swing the player is working, drawn where the work happens instead of written in the message
   * strip. A refusal the player can see coming is not an error message: the ring closes as the work
   * is spent and the unit lands on the step that completes it, so holding the harvest key reads as
   * a rhythm rather than as a stream of "cooling down" toasts.
   *
   * Both numbers are native. `action_cooldown` is the work still outstanding and
   * `action_cooldown_total` is what the whole swing is worth, so the host draws a proportion it was
   * given rather than inferring a maximum from a value it watched fall.
   */
  private drawActionCooldown(center: PixelPoint, radius: number): void {
    if (!this.snapshot) return;
    const { action_cooldown: remaining, action_cooldown_total: total } =
      this.snapshot.player;
    if (remaining <= 0 || total <= 0) return;
    const ready = Math.min(1, Math.max(0, 1 - remaining / total));
    const ctx = this.context;
    const ring = radius * 1.55;
    ctx.save();
    ctx.lineCap = "round";
    ctx.lineWidth = Math.max(2, radius * 0.22);
    ctx.strokeStyle = "#0b1a1633";
    ctx.beginPath();
    ctx.arc(center.x, center.y, ring, 0, Math.PI * 2);
    ctx.stroke();
    ctx.strokeStyle = "#f5d572";
    ctx.beginPath();
    ctx.arc(
      center.x,
      center.y,
      ring,
      -Math.PI / 2,
      -Math.PI / 2 + Math.PI * 2 * ready,
    );
    ctx.stroke();
    ctx.restore();
  }

  private drawGroundItems(width: number, height: number, size: number): void {
    if (!this.snapshot?.ground_items) return;
    const ctx = this.context;
    for (const item of this.snapshot.ground_items) {
      const center = this.camera.project(item, width, height);
      if (!visible(center, size, width, height)) continue;
      const def = this.itemsById.get(item.item_id);
      const remainingTicks =
        item.despawn_tick > this.snapshot.tick
          ? item.despawn_tick - this.snapshot.tick
          : 0;
      if (remainingTicks < 100 && Math.floor(this.now / 150) % 2 === 0) {
        continue;
      }
      const bob = this.reducedMotion
        ? 0
        : Math.sin(this.now / 300 + item.id) * 3;
      const point = { x: center.x, y: center.y + bob };
      ctx.beginPath();
      ctx.arc(point.x, point.y, Math.max(6, size * 0.22), 0, Math.PI * 2);
      ctx.fillStyle = def?.color ?? "#ffffff";
      ctx.shadowColor = def?.color ?? "#ffffff";
      ctx.shadowBlur = 8;
      ctx.fill();
      ctx.shadowBlur = 0;
      if (def) {
        drawItemIcon(ctx, def.icon, def.color, point.x, point.y, size * 0.55);
      }
      if (item.quantity > 1) {
        ctx.fillStyle = "#ffffff";
        ctx.font = `bold ${Math.max(10, Math.round(size * 0.24))}px monospace`;
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(
          String(item.quantity),
          point.x + size * 0.22,
          point.y - size * 0.22,
        );
      }
    }
  }
}

function visible(
  point: PixelPoint,
  margin: number,
  width: number,
  height: number,
): boolean {
  return (
    point.x >= -margin &&
    point.y >= -margin &&
    point.x <= width + margin &&
    point.y <= height + margin
  );
}

function forestHash(q: number, r: number, unit: number): number {
  let value = Math.imul(q, 0x45d9f3b) ^ Math.imul(r, 0x119de1f3) ^ unit;
  value = Math.imul(value ^ (value >>> 16), 0x45d9f3b);
  value ^= value >>> 16;
  return (value >>> 0) / 0x1_0000_0000;
}
