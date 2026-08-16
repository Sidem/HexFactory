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
} from "../core/types";

const HEX_SIZE = 35;

export class CanvasFactoryRenderer {
  private readonly context: CanvasRenderingContext2D;
  private snapshot: FactorySnapshot | null = null;
  private hover: AxialCoordinate | null = null;

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
    this.draw();
  }

  setHover(coordinate: AxialCoordinate | null): void {
    this.hover = coordinate;
    this.draw();
  }

  pick(clientX: number, clientY: number): AxialCoordinate {
    const rect = this.canvas.getBoundingClientRect();
    const point = { x: clientX - rect.left, y: clientY - rect.top };
    return pixelToAxial(point, HEX_SIZE, this.origin());
  }

  draw(): void {
    const ratio = window.devicePixelRatio || 1;
    const width = Math.max(1, this.canvas.clientWidth);
    const height = Math.max(1, this.canvas.clientHeight);
    if (
      this.canvas.width !== width * ratio ||
      this.canvas.height !== height * ratio
    ) {
      this.canvas.width = width * ratio;
      this.canvas.height = height * ratio;
    }
    const ctx = this.context;
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.clearRect(0, 0, width, height);
    this.drawGrid();
    if (!this.snapshot) return;
    for (const resource of this.snapshot.resources) {
      const center = axialToPixel(resource, HEX_SIZE, this.origin());
      drawHex(ctx, center, HEX_SIZE * 0.86, "#573d2b", "#aa7444");
      ctx.fillStyle = "#e6a85c";
      ctx.beginPath();
      ctx.arc(center.x, center.y, 7, 0, Math.PI * 2);
      ctx.fill();
    }
    for (const building of this.snapshot.buildings) this.drawBuilding(building);
    if (this.hover) {
      drawHex(
        ctx,
        axialToPixel(this.hover, HEX_SIZE, this.origin()),
        HEX_SIZE * 0.92,
        "#ffffff12",
        "#f5c451",
      );
    }
  }

  private drawGrid(): void {
    const ctx = this.context;
    const origin = this.origin();
    for (let r = -5; r <= 5; r += 1) {
      for (let q = -7; q <= 7; q += 1) {
        drawHex(
          ctx,
          axialToPixel({ q, r }, HEX_SIZE, origin),
          HEX_SIZE * 0.96,
          "#141922",
          "#252d3a",
        );
      }
    }
  }

  private drawBuilding(building: EntitySnapshot): void {
    const ctx = this.context;
    const center = axialToPixel(building, HEX_SIZE, this.origin());
    const colors: Record<EntitySnapshot["kind"], string> = {
      extractor: "#b55d43",
      belt: "#3e536d",
      composer: "#7257a5",
      container: "#91723b",
      consumer: "#3b7b67",
    };
    drawHex(ctx, center, HEX_SIZE * 0.83, colors[building.kind], "#d9e2ee");
    const direction = axialNeighbor({ q: 0, r: 0 }, building.orientation);
    const tip = axialToPixel(direction, HEX_SIZE * 0.42, {
      x: center.x,
      y: center.y,
    });
    ctx.strokeStyle = "#edf4ff";
    ctx.lineWidth = 3;
    ctx.beginPath();
    ctx.moveTo(center.x, center.y);
    ctx.lineTo(tip.x, tip.y);
    ctx.stroke();
    ctx.fillStyle = "#edf4ff";
    ctx.beginPath();
    ctx.arc(tip.x, tip.y, 3.5, 0, Math.PI * 2);
    ctx.fill();
    if (building.progress_total > 0 && building.progress > 0) {
      ctx.strokeStyle = "#f5c451";
      ctx.lineWidth = 4;
      ctx.beginPath();
      ctx.arc(
        center.x,
        center.y,
        HEX_SIZE * 0.62,
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
      ctx.font = "bold 12px system-ui";
      ctx.textAlign = "center";
      ctx.fillText(String(quantity), center.x, center.y + 5);
    }
    if (building.cargo) {
      const color =
        this.definitions.items.find(
          (item) => item.id === building.cargo?.item_id,
        )?.color ?? "#fff";
      ctx.fillStyle = color;
      ctx.strokeStyle = "#10141a";
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(center.x, center.y, 8, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
    }
  }

  private origin(): PixelPoint {
    return {
      x: this.canvas.clientWidth / 2,
      y: this.canvas.clientHeight / 2 - 15,
    };
  }
}

function drawHex(
  context: CanvasRenderingContext2D,
  center: PixelPoint,
  size: number,
  fill: string,
  stroke: string,
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
  context.lineWidth = 1.2;
  context.stroke();
}
