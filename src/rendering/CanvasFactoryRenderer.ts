import {
  HEX_DIRECTIONS,
  axialNeighbor,
  axialToPixel,
  pixelToAxial,
  type AxialCoordinate,
  type PixelPoint,
} from "@hexlife/embed/hex";

import type {
  Definitions,
  EntitySnapshot,
  FactorySnapshot,
  PlacementPreview,
} from "../core/types";

const BASE_HEX_SIZE = 31;

export class HexCamera {
  center: AxialCoordinate = { q: 0, r: 0 };
  pan: PixelPoint = { x: 0, y: 0 };
  zoom = 1;
  following = true;

  origin(width: number, height: number): PixelPoint {
    const centerPixel = axialToPixel(this.center, BASE_HEX_SIZE * this.zoom, {
      x: 0,
      y: 0,
    });
    return {
      x: width / 2 + this.pan.x - centerPixel.x,
      y: height / 2 + this.pan.y - centerPixel.y,
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

  follow(coordinate: AxialCoordinate): void {
    if (!this.following) return;
    this.center = { ...coordinate };
    this.pan = { x: 0, y: 0 };
  }

  recenter(coordinate: AxialCoordinate): void {
    this.following = true;
    this.center = { ...coordinate };
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
    const anchor = this.pick(point, width, height);
    this.zoom = Math.max(0.55, Math.min(2.2, this.zoom * factor));
    const projected = this.project(anchor, width, height);
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
    ctx.fillStyle = "#0b1116";
    ctx.fillRect(0, 0, width, height);
    if (!this.snapshot) return;
    const size = BASE_HEX_SIZE * this.camera.zoom;
    for (const tile of this.snapshot.terrain) {
      const center = this.camera.project(tile, width, height);
      if (!visible(center, size, width, height)) continue;
      const colors = {
        ground: ["#17231f", "#2d3c34"],
        water: ["#17314a", "#2e6384"],
        rock: ["#31343a", "#626872"],
      } as const;
      drawHex(
        ctx,
        center,
        size * 0.97,
        colors[tile.terrain][0],
        colors[tile.terrain][1],
      );
    }
    for (const resource of this.snapshot.resources) {
      if (resource.quantity === 0) continue;
      const center = this.camera.project(resource, width, height);
      if (!visible(center, size, width, height)) continue;
      const item = this.definitions.items.find(
        ({ id }) => id === resource.item_id,
      );
      ctx.fillStyle = item?.color ?? "#fff";
      ctx.strokeStyle = "#0a0d10";
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(center.x, center.y, Math.max(4, size * 0.22), 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
      ctx.fillStyle = "#f4f7f5";
      ctx.font = `700 ${Math.max(9, size * 0.28)}px system-ui`;
      ctx.textAlign = "center";
      ctx.fillText(String(resource.quantity), center.x, center.y - size * 0.42);
    }
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
      drawHex(
        ctx,
        this.camera.project(this.hover, width, height),
        size * 0.88,
        `${stroke}18`,
        stroke,
        2,
      );
    }
  }

  private drawBuilding(
    building: EntitySnapshot,
    width: number,
    height: number,
    size: number,
  ): void {
    const ctx = this.context;
    const center = this.camera.project(building, width, height);
    if (!visible(center, size, width, height)) return;
    const colors: Record<EntitySnapshot["kind"], string> = {
      extractor: "#b75e45",
      belt: "#415b78",
      composer: "#765bae",
      container: "#a07c3e",
      consumer: "#3c806a",
      hub: "#d1a945",
    };
    drawHex(ctx, center, size * 0.78, colors[building.kind], "#dce7ef", 1.4);
    const direction = axialNeighbor({ q: 0, r: 0 }, building.orientation);
    const tip = axialToPixel(direction, size * 0.39, center);
    ctx.strokeStyle = "#f3f7fa";
    ctx.lineWidth = Math.max(2, size * 0.08);
    ctx.beginPath();
    ctx.moveTo(center.x, center.y);
    ctx.lineTo(tip.x, tip.y);
    ctx.stroke();
    ctx.fillStyle = "#f3f7fa";
    ctx.beginPath();
    ctx.arc(tip.x, tip.y, Math.max(2.5, size * 0.09), 0, Math.PI * 2);
    ctx.fill();
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
    if (building.cargo) {
      const color =
        this.definitions.items.find(
          (item) => item.id === building.cargo?.item_id,
        )?.color ?? "#fff";
      const phase = this.reducedMotion ? 0.5 : (this.now % 700) / 700;
      const cargoCenter = {
        x: center.x + (tip.x - center.x) * (phase * 0.5),
        y: center.y + (tip.y - center.y) * (phase * 0.5),
      };
      ctx.fillStyle = color;
      ctx.strokeStyle = "#10141a";
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(
        cargoCenter.x,
        cargoCenter.y,
        Math.max(5, size * 0.21),
        0,
        Math.PI * 2,
      );
      ctx.fill();
      ctx.stroke();
    }
  }

  private drawPlayer(width: number, height: number, size: number): void {
    if (!this.snapshot) return;
    const ctx = this.context;
    const center = this.camera.project(this.snapshot.player, width, height);
    const direction = axialNeighbor(
      { q: 0, r: 0 },
      this.snapshot.player.facing,
    );
    const tip = axialToPixel(direction, size * 0.53, center);
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
