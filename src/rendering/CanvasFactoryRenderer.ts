import {
  axialNeighbor,
  axialToPixel,
  pixelToAxial,
  rotateAxial,
  type AxialCoordinate,
  type PixelPoint,
} from "@hexlife/embed/hex";

import { TERRAIN_INFO } from "../core/terrain";
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
  cargoTravel,
  drawBuildingLook,
  facingTip,
  NORTH,
  PLAYER_BODY,
  PLAYER_RING,
  spanEnd,
} from "./buildingLook";
import { drawParts } from "./shapeGrammar";
import { drawHex, hexPath } from "./hexDraw";
import { drawItemIcon } from "./icons";
import { WORLD_SCALE, homeBearing } from "./landmarks";
import {
  drawDepletion,
  drawTerrainCell,
  drawWaterShimmer,
  hexLook,
  indexTerrain,
  surveyedBand,
  TerrainTiles,
} from "./terrainLook";

export const BASE_HEX_SIZE = 22;

/** One colour per building kind, shared with the minimap so a machine reads the same on both. */
export const BUILDING_COLORS: Record<EntitySnapshot["kind"], string> = {
  extractor: "#b75e45",
  belt: "#415b78",
  composer: "#765bae",
  container: "#a07c3e",
  consumer: "#3c806a",
  hub: "#d1a945",
  pump: "#2f7d9c",
  pole: "#c8b56b",
  generator: "#d4a017",
  boiler: "#a85c32",
};

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
    const before = this.pick(point, width, height);
    this.zoom = Math.max(0.55, Math.min(2.2, this.zoom * factor));
    const projected = this.project(before, width, height);
    this.pan.x += point.x - projected.x;
    this.pan.y += point.y - projected.y;
    this.following = false;
  }
}

export class CanvasFactoryRenderer {
  readonly camera = new HexCamera();
  private readonly context: CanvasRenderingContext2D;
  private readonly itemsById: ReadonlyMap<number, ItemDefinition>;
  private readonly buildingsById: ReadonlyMap<number, BuildingDefinition>;
  private readonly reducedMotion = matchMedia(
    "(prefers-reduced-motion: reduce)",
  ).matches;
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
  private dragPath: LinePreviewCell[] = [];
  private veil: HTMLCanvasElement | null = null;
  private terrainLayer: HTMLCanvasElement | null = null;
  private terrainLayerKey = "";
  private readonly tiles = new TerrainTiles();
  private now = 0;

  constructor(
    private readonly canvas: HTMLCanvasElement,
    definitions: Definitions,
  ) {
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Canvas 2D is unavailable");
    this.context = context;
    this.itemsById = new Map(definitions.items.map((item) => [item.id, item]));
    this.buildingsById = new Map(
      definitions.buildings.map((building) => [building.id, building]),
    );
    new ResizeObserver(() => this.draw()).observe(canvas);
  }

  setSnapshot(snapshot: FactorySnapshot): void {
    this.snapshot = snapshot;
    this.camera.follow(snapshot.player);
    this.draw();
  }

  setHome(point: WorldPoint | null): void {
    this.home = point;
  }

  setHover(
    coordinate: AxialCoordinate | null,
    placement: PlacementPreview | null = null,
  ): void {
    this.hover = coordinate;
    this.placement = placement;
    this.draw();
  }

  setSelection(coordinate: AxialCoordinate | null): void {
    this.selection = coordinate;
    this.draw();
  }

  setBuildMode(active: boolean): void {
    this.buildMode = active;
    this.draw();
  }

  /**
   * The cells the in-progress drag covers, exactly as native resolved them. Presentation only: the
   * renderer draws this list and never computes a path of its own.
   */
  setDragPath(cells: LinePreviewCell[]): void {
    this.dragPath = cells;
    this.draw();
  }

  setBuildFootprint(footprint: AxialCoordinate[], orientation: number): void {
    this.buildFootprint = footprint.map((cell) =>
      rotateAxial(cell, orientation, { q: 0, r: 0 }),
    );
    this.draw();
  }

  toggleGrid(): boolean {
    this.gridToggled = !this.gridToggled;
    this.draw();
    return this.gridToggled;
  }

  pick(clientX: number, clientY: number): AxialCoordinate {
    const rect = this.canvas.getBoundingClientRect();
    return this.camera.pick(
      { x: clientX - rect.left, y: clientY - rect.top },
      this.canvas.clientWidth,
      this.canvas.clientHeight,
    );
  }

  /** The world position a pointer is over, for the `aim` the host sends from it. */
  pickWorld(clientX: number, clientY: number): WorldPoint {
    const rect = this.canvas.getBoundingClientRect();
    return this.camera.worldAt(
      { x: clientX - rect.left, y: clientY - rect.top },
      this.canvas.clientWidth,
      this.canvas.clientHeight,
    );
  }

  panBy(x: number, y: number): void {
    this.camera.panBy(x, y);
    this.draw();
  }

  zoomAt(clientX: number, clientY: number, factor: number): void {
    const rect = this.canvas.getBoundingClientRect();
    this.camera.zoomAt(
      factor,
      { x: clientX - rect.left, y: clientY - rect.top },
      this.canvas.clientWidth,
      this.canvas.clientHeight,
    );
    this.draw();
  }

  recenter(): void {
    if (this.snapshot) this.camera.recenter(this.snapshot.player);
    this.draw();
  }

  renderFrame(now: number): void {
    this.now = now;
    if (!this.reducedMotion) this.draw();
  }

  draw(): void {
    const ratio = window.devicePixelRatio || 1;
    const width = Math.max(1, this.canvas.clientWidth);
    const height = Math.max(1, this.canvas.clientHeight);
    if (
      this.canvas.width !== Math.floor(width * ratio) ||
      this.canvas.height !== Math.floor(height * ratio)
    ) {
      this.canvas.width = Math.floor(width * ratio);
      this.canvas.height = Math.floor(height * ratio);
    }
    const ctx = this.context;
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.clearRect(0, 0, width, height);
    const gradient = ctx.createRadialGradient(
      width * 0.5,
      height * 0.45,
      30,
      width * 0.5,
      height * 0.5,
      Math.max(width, height),
    );
    gradient.addColorStop(0, "#1a3a32");
    gradient.addColorStop(0.55, "#10261f");
    gradient.addColorStop(1, "#081410");
    ctx.fillStyle = gradient;
    ctx.fillRect(0, 0, width, height);
    if (!this.snapshot) return;
    const size = BASE_HEX_SIZE * this.camera.zoom;
    this.drawEnvironment(width, height, size);
    if (this.buildMode || this.gridToggled) this.drawGrid(width, height, size);
    if (this.buildMode) this.drawBuildRange(width, height);
    for (const building of this.snapshot.buildings)
      this.drawBuilding(building, width, height, size);
    this.drawFog(width, height, ratio);
    this.drawPlayer(width, height, size);
    this.drawHomeMarker(width, height);
    if (this.selection)
      drawHex(
        ctx,
        this.camera.project(this.selection, width, height),
        size * 0.91,
        "#ffffff08",
        "#f5d572",
        3,
      );
    // A drag replaces the single-cell hover: the run it would build is the thing to look at.
    if (this.dragPath.length) this.drawDragPath(width, height, size);
    else if (this.hover) {
      const stroke = this.placement
        ? this.placement.legal
          ? "#76e0aa"
          : "#ff7b78"
        : "#e9f0f7";
      const footprint = this.buildMode ? this.buildFootprint : [{ q: 0, r: 0 }];
      for (const offset of footprint) {
        drawHex(
          ctx,
          this.camera.project(
            { q: this.hover.q + offset.q, r: this.hover.r + offset.r },
            width,
            height,
          ),
          size * 0.88,
          `${stroke}18`,
          stroke,
          2,
        );
      }
    }
  }

  /**
   * The run the current drag would build, cell by cell, in the native headings. Cells the drag
   * cannot use are drawn in the refusal colour rather than hidden, so a run that stops short of the
   * cursor shows where and not merely that.
   */
  private drawDragPath(width: number, height: number, size: number): void {
    const ctx = this.context;
    for (const cell of this.dragPath) {
      const center = this.camera.project(cell, width, height);
      if (!visible(center, size, width, height)) continue;
      const stroke = cell.legal ? "#76e0aa" : "#ff7b78";
      drawHex(ctx, center, size * 0.88, `${stroke}22`, stroke, 2);
      if (!cell.legal) continue;
      // The same heading mark placed buildings carry, so a previewed run reads like the run it
      // becomes rather than like a selection.
      const tip = facingTip(center, size, cell.orientation, 0.34);
      ctx.strokeStyle = stroke;
      ctx.lineWidth = Math.max(2, size * 0.07);
      ctx.beginPath();
      ctx.moveTo(center.x, center.y);
      ctx.lineTo(tip.x, tip.y);
      ctx.stroke();
    }
  }

  private drawEnvironment(width: number, height: number, size: number): void {
    if (!this.snapshot) return;
    const ctx = this.context;
    this.blitTerrain(width, height, size);
    for (const region of this.snapshot.terrain) {
      if (region.terrain !== "deep_water" && region.terrain !== "shallow_water")
        continue;
      const center = this.camera.project(
        { q: region.q, r: region.r },
        width,
        height,
      );
      if (!visible(center, size, width, height)) continue;
      drawWaterShimmer(
        ctx,
        center,
        size,
        hexLook(region.q, region.r),
        this.now,
        this.reducedMotion,
      );
    }
    for (const resource of this.snapshot.resources) {
      const center = this.camera.project(
        { q: resource.q, r: resource.r },
        width,
        height,
      );
      if (!visible(center, size, width, height)) continue;
      const look = hexLook(resource.q, resource.r);
      drawDepletion(
        ctx,
        center,
        size,
        look,
        resource.quantity,
        resource.initial_quantity,
      );
      const item = this.itemsById.get(resource.item_id);
      const color = item?.color ?? "#fff";
      // Remaining amount is not a label on every ore hex. It belongs on the cell under the
      // cursor, or on a cell that has already been drawn from — the glyph already names the
      // material, and the inspector has the exact count.
      const hovered =
        this.hover !== null &&
        this.hover.q === resource.q &&
        this.hover.r === resource.r;
      const drawnFrom = resource.quantity < resource.initial_quantity;
      if (resource.quantity > 0) {
        const pulse = this.reducedMotion ? 0 : Math.sin(this.now / 450) * 0.03;
        drawHex(ctx, center, size * (0.62 + pulse), `${color}40`, color, 1.4);
        drawItemIcon(ctx, item?.icon ?? "ore", color, center.x, center.y, size);
      }
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

  /**
   * Static terrain is painted once per camera/survey change and blitted every frame. Water shimmer
   * and field marks stay outside the layer so motion does not rebuild the mosaic.
   */
  private blitTerrain(width: number, height: number, size: number): void {
    if (!this.snapshot) return;
    const origin = this.camera.origin(width, height);
    const key = [
      width,
      height,
      this.camera.zoom.toFixed(3),
      origin.x.toFixed(1),
      origin.y.toFixed(1),
      this.snapshot.terrain.length,
      this.snapshot.chunks.length,
    ].join(":");
    if (!this.terrainLayer)
      this.terrainLayer = document.createElement("canvas");
    const layer = this.terrainLayer;
    const ratio = window.devicePixelRatio || 1;
    if (
      layer.width !== Math.floor(width * ratio) ||
      layer.height !== Math.floor(height * ratio)
    ) {
      layer.width = Math.floor(width * ratio);
      layer.height = Math.floor(height * ratio);
      this.terrainLayerKey = "";
    }
    if (this.terrainLayerKey !== key) {
      const fog = layer.getContext("2d");
      if (!fog) return;
      fog.setTransform(ratio, 0, 0, ratio, 0, 0);
      fog.clearRect(0, 0, width, height);
      this.paintTerrainLayer(fog, width, height, size);
      this.terrainLayerKey = key;
    }
    this.context.drawImage(layer, 0, 0, width, height);
  }

  private paintTerrainLayer(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    size: number,
  ): void {
    if (!this.snapshot) return;
    const terrain = indexTerrain(this.snapshot.terrain);
    const chunks = this.snapshot.chunks;
    const worldOrigin = { x: 0, y: 0 };
    ctx.beginPath();
    for (const chunk of chunks) {
      const origin = this.camera.projectWorld(chunk, width, height);
      const span =
        chunk.span * ((BASE_HEX_SIZE * this.camera.zoom) / WORLD_SCALE);
      if (
        origin.x > width + span ||
        origin.y > height + span ||
        origin.x + span < -span ||
        origin.y + span < -span
      )
        continue;
      ctx.rect(origin.x, origin.y, span, span);
    }
    ctx.fillStyle = TERRAIN_INFO.lowland.fill;
    ctx.fill();
    ctx.save();
    ctx.clip();
    ctx.globalAlpha = 0.28;
    ctx.drawImage(this.tiles.field("lowland"), 0, 0, width, height);
    ctx.restore();
    const bandAt = (q: number, r: number) => {
      const world = axialToPixel({ q, r }, WORLD_SCALE, worldOrigin);
      if (!isSurveyed(chunks, world)) return undefined;
      return surveyedBand(terrain, q, r);
    };
    for (const region of this.snapshot.terrain) {
      const center = this.camera.project(
        { q: region.q, r: region.r },
        width,
        height,
      );
      if (!visible(center, size, width, height)) continue;
      const neighbors: Array<ReturnType<typeof bandAt>> = [];
      for (let direction = 0; direction < 6; direction += 1) {
        const next = axialNeighbor({ q: region.q, r: region.r }, direction);
        neighbors.push(bandAt(next.q, next.r));
      }
      drawTerrainCell(
        ctx,
        center,
        size,
        region.terrain,
        hexLook(region.q, region.r),
        this.tiles,
        neighbors,
      );
      // Impassable ground is drawn as a category before it is drawn as a material. Cliff against
      // highland was two greys a step apart and the only way to tell them apart was to walk into
      // one; the hatch says "you cannot stand here" whatever the band underneath it happens to be.
      const band = TERRAIN_INFO[region.terrain];
      if (!band.passable) this.drawImpassable(center, size, band.stroke, ctx);
    }
  }

  /**
   * The shared mark for ground the player cannot stand on: a hatch inside the hex and a brighter,
   * heavier rim around it. Which bands get it is native's rule, read from the passability table
   * `fixtures/terrain-passability.json` pins — the renderer never decides that a grey means cliff.
   */
  private drawImpassable(
    center: PixelPoint,
    size: number,
    stroke: string,
    ctx: CanvasRenderingContext2D = this.context,
  ): void {
    const radius = size * 0.97;
    ctx.save();
    hexPath(ctx, center, radius);
    ctx.clip();
    ctx.strokeStyle = `${stroke}59`;
    ctx.lineWidth = Math.max(1, size * 0.075);
    ctx.beginPath();
    const step = Math.max(4, size * 0.34);
    for (let offset = -radius; offset <= radius * 3; offset += step) {
      ctx.moveTo(center.x - radius + offset, center.y - radius);
      ctx.lineTo(center.x - radius + offset - 2 * radius, center.y + radius);
    }
    ctx.stroke();
    ctx.restore();
    hexPath(ctx, center, radius);
    ctx.strokeStyle = stroke;
    ctx.lineWidth = Math.max(1.5, size * 0.085);
    ctx.stroke();
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
   * Veil every part of the viewport the simulation has not generated yet. The surveyed area is
   * punched out of an offscreen veil so overlapping chunk edges cannot leave seams, and the
   * frontier of the surveyed world is drawn as a dashed edge on top.
   */
  private drawFog(width: number, height: number, ratio: number): void {
    if (!this.snapshot) return;
    const chunks = this.snapshot.chunks;
    if (!chunks.length) return;
    const scale = (BASE_HEX_SIZE * this.camera.zoom) / WORLD_SCALE;
    const surveyed = chunks.map((chunk) => {
      const origin = this.camera.projectWorld(chunk, width, height);
      return { chunk, x: origin.x, y: origin.y, size: chunk.span * scale };
    });
    const veil = this.veilCanvas();
    const target = {
      width: Math.floor(width * ratio),
      height: Math.floor(height * ratio),
    };
    if (veil.width !== target.width || veil.height !== target.height) {
      veil.width = target.width;
      veil.height = target.height;
    }
    const fog = veil.getContext("2d");
    if (!fog) return;
    fog.setTransform(ratio, 0, 0, ratio, 0, 0);
    fog.clearRect(0, 0, width, height);
    // A cool slate lighter than the surveyed ground, so fog reads as unknown rather than as night.
    fog.fillStyle = "#18242fee";
    fog.fillRect(0, 0, width, height);
    fog.strokeStyle = "#a9d8ff22";
    fog.lineWidth = 2;
    fog.beginPath();
    for (let x = -height; x < width; x += 26) {
      fog.moveTo(x, 0);
      fog.lineTo(x + height, height);
    }
    fog.stroke();
    fog.globalCompositeOperation = "destination-out";
    fog.filter = "blur(13px)";
    fog.fillStyle = "#000";
    for (const region of surveyed) {
      // The blur reaches past the rect, so keep a margin when culling offscreen chunks.
      if (
        region.x > width + 24 ||
        region.y > height + 24 ||
        region.x + region.size < -24 ||
        region.y + region.size < -24
      )
        continue;
      fog.fillRect(region.x, region.y, region.size, region.size);
    }
    fog.filter = "none";
    fog.globalCompositeOperation = "source-over";
    this.context.drawImage(veil, 0, 0, width, height);

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

  private veilCanvas(): HTMLCanvasElement {
    this.veil ??= document.createElement("canvas");
    return this.veil;
  }

  private drawGrid(width: number, height: number, size: number): void {
    const corners = [
      this.camera.pick({ x: 0, y: 0 }, width, height),
      this.camera.pick({ x: width, y: 0 }, width, height),
      this.camera.pick({ x: 0, y: height }, width, height),
      this.camera.pick({ x: width, y: height }, width, height),
    ];
    const minQ = Math.min(...corners.map(({ q }) => q)) - 3;
    const maxQ = Math.max(...corners.map(({ q }) => q)) + 3;
    const minR = Math.min(...corners.map(({ r }) => r)) - 3;
    const maxR = Math.max(...corners.map(({ r }) => r)) + 3;
    for (let q = minQ; q <= maxQ; q += 1) {
      for (let r = minR; r <= maxR; r += 1) {
        drawHex(
          this.context,
          this.camera.project({ q, r }, width, height),
          size * 0.97,
          "transparent",
          "#9bb9af2d",
        );
      }
    }
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
    for (const cell of building.footprint) {
      const cellCenter = this.camera.project(cell, width, height);
      if (!visible(cellCenter, size, width, height)) continue;
      if (cell.q === building.q && cell.r === building.r) continue;
      drawHex(ctx, cellCenter, size * 0.78, color, "#dce7ef", 1.4);
    }
    const center = this.camera.project(building, width, height);
    if (!visible(center, size, width, height)) return;
    // A riser's whole point is the span, so it is drawn before the body: a thin gantry reaching
    // across the seam to the hex two rows away, under the building rather than over it.
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
    drawBuildingLook(ctx, {
      building,
      definition,
      center,
      size,
      color,
      now: this.now,
      reducedMotion: this.reducedMotion,
      tier: definition?.tier,
    });
    const tip = facingTip(center, size, building.orientation);
    ctx.strokeStyle = "#f3f7fa";
    ctx.lineWidth = Math.max(2, size * 0.08);
    ctx.beginPath();
    ctx.moveTo(center.x, center.y);
    ctx.lineTo(tip.x, tip.y);
    ctx.stroke();
    ctx.fillStyle = "#f5fbf8";
    ctx.font = `900 ${Math.max(8, size * 0.23)}px system-ui`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(
      definition?.icon ?? building.kind.slice(0, 3).toUpperCase(),
      center.x,
      center.y,
    );
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
      const travel = cargoTravel(this.now, this.reducedMotion, building.id);
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
   * The wait between one field action and the next, drawn where the action happens instead of
   * written in the message strip. A refusal the player can see coming is not an error message: the
   * ring closes as the cooldown drains, so holding the harvest key reads as a rhythm rather than as
   * a stream of "cooling down" toasts.
   *
   * Both numbers are native. `action_cooldown` is the wait still outstanding and
   * `action_cooldown_total` is what a fresh one is worth, so the host draws a proportion it was
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
