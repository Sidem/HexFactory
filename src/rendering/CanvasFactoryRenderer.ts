import {
  HEX_DIRECTIONS,
  axialNeighbor,
  axialToPixel,
  pixelToAxial,
  rotateAxial,
  type AxialCoordinate,
  type PixelPoint,
} from "@hexlife/embed/hex";

import type {
  Definitions,
  EntitySnapshot,
  FactorySnapshot,
  PlacementPreview,
  WorldPoint,
} from "../core/types";

const BASE_HEX_SIZE = 31;
const WORLD_SCALE = 1024;

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
  private readonly reducedMotion = matchMedia(
    "(prefers-reduced-motion: reduce)",
  ).matches;
  private snapshot: FactorySnapshot | null = null;
  private hover: AxialCoordinate | null = null;
  private selection: AxialCoordinate | null = null;
  private placement: PlacementPreview | null = null;
  private buildMode = false;
  private gridToggled = false;
  private buildFootprint: AxialCoordinate[] = [{ q: 0, r: 0 }];
  private now = 0;

  constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly definitions: Definitions,
  ) {
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Canvas 2D is unavailable");
    this.context = context;
    new ResizeObserver(() => this.draw()).observe(canvas);
  }

  setSnapshot(snapshot: FactorySnapshot): void {
    this.snapshot = snapshot;
    this.camera.follow(snapshot.player);
    this.draw();
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
    gradient.addColorStop(0, "#182720");
    gradient.addColorStop(1, "#091114");
    ctx.fillStyle = gradient;
    ctx.fillRect(0, 0, width, height);
    if (!this.snapshot) return;
    const size = BASE_HEX_SIZE * this.camera.zoom;
    this.drawEnvironment(width, height, size);
    if (this.buildMode || this.gridToggled) this.drawGrid(width, height, size);
    if (this.buildMode) this.drawBuildRange(width, height);
    for (const building of this.snapshot.buildings)
      this.drawBuilding(building, width, height, size);
    this.drawPlayer(width, height, size);
    if (this.selection)
      drawHex(
        ctx,
        this.camera.project(this.selection, width, height),
        size * 0.91,
        "#ffffff08",
        "#f5d572",
        3,
      );
    if (this.hover) {
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

  private drawEnvironment(width: number, height: number, size: number): void {
    if (!this.snapshot) return;
    const ctx = this.context;
    const scale = size / WORLD_SCALE;
    for (const region of this.snapshot.terrain) {
      const center = this.camera.projectWorld(region, width, height);
      const radius = region.radius * scale;
      if (!visible(center, radius, width, height)) continue;
      const water = region.terrain === "water";
      ctx.fillStyle = water ? "#163c57cc" : "#464a4dcc";
      ctx.strokeStyle = water ? "#39789c" : "#777d80";
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(center.x, center.y, radius, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
    }
    for (const resource of this.snapshot.resources) {
      if (resource.quantity === 0) continue;
      const center = this.camera.projectWorld(resource, width, height);
      const radius = resource.radius * scale;
      if (!visible(center, radius, width, height)) continue;
      const item = this.definitions.items.find(
        ({ id }) => id === resource.item_id,
      );
      const pulse = this.reducedMotion ? 0 : Math.sin(this.now / 450) * 2;
      ctx.fillStyle = `${item?.color ?? "#ffffff"}55`;
      ctx.strokeStyle = item?.color ?? "#fff";
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(center.x, center.y, radius + pulse, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
      ctx.fillStyle = "#f4f7f5";
      ctx.font = `700 ${Math.max(10, size * 0.28)}px system-ui`;
      ctx.textAlign = "center";
      ctx.fillText(
        `${item?.icon ?? "RES"} ${resource.quantity}`,
        center.x,
        center.y + 4,
      );
    }
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
    const colors: Record<EntitySnapshot["kind"], string> = {
      extractor: "#b75e45",
      belt: "#415b78",
      composer: "#765bae",
      container: "#a07c3e",
      consumer: "#3c806a",
      hub: "#d1a945",
    };
    for (const cell of building.footprint) {
      const cellCenter = this.camera.project(cell, width, height);
      if (visible(cellCenter, size, width, height))
        drawHex(
          ctx,
          cellCenter,
          size * 0.78,
          colors[building.kind],
          "#dce7ef",
          1.4,
        );
    }
    const center = this.camera.project(building, width, height);
    if (!visible(center, size, width, height)) return;
    const direction = axialNeighbor({ q: 0, r: 0 }, building.orientation);
    const tip = axialToPixel(direction, size * 0.39, center);
    ctx.strokeStyle = "#f3f7fa";
    ctx.lineWidth = Math.max(2, size * 0.08);
    ctx.beginPath();
    ctx.moveTo(center.x, center.y);
    ctx.lineTo(tip.x, tip.y);
    ctx.stroke();
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
      ctx.fillStyle = "#fff";
      ctx.font = `bold ${Math.max(10, size * 0.34)}px system-ui`;
      ctx.textAlign = "center";
      ctx.fillText(String(quantity), center.x, center.y + 4);
    }
  }

  private drawPlayer(width: number, height: number, size: number): void {
    if (!this.snapshot) return;
    const player = this.snapshot.player;
    const center = this.camera.projectWorld(player, width, height);
    const length = size * 0.53;
    const tip = {
      x: center.x + (player.facing_x / 1000) * length,
      y: center.y + (player.facing_y / 1000) * length,
    };
    const ctx = this.context;
    ctx.fillStyle = "#f4f7f2";
    ctx.strokeStyle = "#142028";
    ctx.lineWidth = 3;
    ctx.beginPath();
    ctx.arc(center.x, center.y, size * 0.3, 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();
    ctx.strokeStyle = "#ef6f61";
    ctx.lineWidth = 4;
    ctx.beginPath();
    ctx.moveTo(center.x, center.y);
    ctx.lineTo(tip.x, tip.y);
    ctx.stroke();
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

function drawHex(
  context: CanvasRenderingContext2D,
  center: PixelPoint,
  size: number,
  fill: string,
  stroke: string,
  lineWidth = 1,
): void {
  context.beginPath();
  for (let corner = 0; corner < HEX_DIRECTIONS.length; corner += 1) {
    const angle = ((60 * corner - 30) * Math.PI) / 180;
    const x = center.x + size * Math.cos(angle);
    const y = center.y + size * Math.sin(angle);
    if (corner === 0) context.moveTo(x, y);
    else context.lineTo(x, y);
  }
  context.closePath();
  context.fillStyle = fill;
  context.fill();
  context.strokeStyle = stroke;
  context.lineWidth = lineWidth;
  context.stroke();
}
