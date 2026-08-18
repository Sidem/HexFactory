import { axialToPixel } from "@hexlife/embed/hex";

import { TERRAIN_INFO } from "../core/terrain";
import type {
  Definitions,
  FactorySnapshot,
  ItemDefinition,
  WorldPoint,
} from "../core/types";
import { BUILDING_COLORS } from "./CanvasFactoryRenderer";
import { WORLD_SCALE, homeBearing } from "./landmarks";

/**
 * How far from the player the minimap window reaches, in hex steps. Wide enough that a factory and
 * the ground it was sited on fit in one glance, narrow enough that a belt is still a belt.
 */
const MINIMAP_RADIUS_HEXES = 32;

/**
 * The second view of the same snapshot. It derives nothing native has not published: surveyed
 * chunks, terrain bands, buildings, the landing hub, and the player.
 *
 * It draws when a snapshot arrives rather than on every animation frame. The player only moves when
 * a snapshot says so, so redrawing more often would cost frames to show the same picture — and the
 * unmeasured half of a browser frame is exactly the half a second canvas lands in.
 */
export class MinimapRenderer {
  private readonly context: CanvasRenderingContext2D;
  private readonly itemsById: ReadonlyMap<number, ItemDefinition>;
  private snapshot: FactorySnapshot | null = null;
  private home: WorldPoint | null = null;

  constructor(
    private readonly canvas: HTMLCanvasElement,
    definitions: Definitions,
  ) {
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Canvas 2D is unavailable");
    this.context = context;
    this.itemsById = new Map(definitions.items.map((item) => [item.id, item]));
    new ResizeObserver(() => this.draw()).observe(canvas);
  }

  setSnapshot(snapshot: FactorySnapshot, home: WorldPoint | null): void {
    this.snapshot = snapshot;
    this.home = home;
    this.draw();
  }

  draw(): void {
    const ratio = window.devicePixelRatio || 1;
    const size = Math.max(1, this.canvas.clientWidth);
    if (this.canvas.width !== Math.floor(size * ratio)) {
      this.canvas.width = Math.floor(size * ratio);
      this.canvas.height = Math.floor(size * ratio);
    }
    const ctx = this.context;
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    // Unsurveyed world is the ground state, exactly as it is in the main view: what is drawn on top
    // of it is the part the simulation has actually generated.
    ctx.fillStyle = "#18242f";
    ctx.fillRect(0, 0, size, size);
    const snapshot = this.snapshot;
    if (!snapshot) return;

    const player = snapshot.player;
    const reach = axialToPixel({ q: MINIMAP_RADIUS_HEXES, r: 0 }, WORLD_SCALE, {
      x: 0,
      y: 0,
    }).x;
    const scale = size / 2 / reach;
    const half = size / 2;
    const project = (point: WorldPoint): WorldPoint => ({
      x: half + (point.x - player.x) * scale,
      y: half + (point.y - player.y) * scale,
    });
    const onMap = (point: WorldPoint, margin: number): boolean =>
      point.x >= -margin &&
      point.y >= -margin &&
      point.x <= size + margin &&
      point.y <= size + margin;

    ctx.fillStyle = TERRAIN_INFO.lowland.fill;
    for (const chunk of snapshot.chunks) {
      const origin = project(chunk);
      const span = chunk.span * scale;
      if (!onMap(origin, span)) continue;
      ctx.fillRect(origin.x, origin.y, span, span);
    }

    // Impassable bands take their bright rim colour rather than their fill, so water and cliff read
    // as edges of the walkable world at this size instead of as slightly different greys.
    const cell = Math.max(2, WORLD_SCALE * 1.9 * scale);
    for (const region of snapshot.terrain) {
      const point = project(region);
      if (!onMap(point, cell)) continue;
      const band = TERRAIN_INFO[region.terrain];
      ctx.fillStyle = band.passable ? band.fill : band.stroke;
      ctx.fillRect(point.x - cell / 2, point.y - cell / 2, cell, cell);
    }

    for (const resource of snapshot.resources) {
      const point = project(resource);
      if (!onMap(point, cell)) continue;
      const color = this.itemsById.get(resource.item_id)?.color ?? "#fff";
      // A worked-out cell stays on the map as a dim scar so a depleted vein is still a place.
      ctx.globalAlpha = resource.quantity === 0 ? 0.35 : 1;
      ctx.fillStyle = resource.quantity === 0 ? "#6a6560" : color;
      ctx.fillRect(point.x - cell / 4, point.y - cell / 4, cell / 2, cell / 2);
      ctx.globalAlpha = 1;
    }

    const mark = Math.max(3, cell);
    for (const building of snapshot.buildings) {
      const point = project(
        axialToPixel(building, WORLD_SCALE, { x: 0, y: 0 }),
      );
      if (!onMap(point, mark)) continue;
      ctx.fillStyle = BUILDING_COLORS[building.kind];
      const width = building.kind === "hub" ? mark * 2 : mark;
      ctx.fillRect(point.x - width / 2, point.y - width / 2, width, width);
      if (building.kind !== "hub") continue;
      ctx.strokeStyle = "#fff3c0";
      ctx.lineWidth = 1.5;
      ctx.strokeRect(point.x - width / 2, point.y - width / 2, width, width);
    }

    this.drawPlayer(half, size);
    this.drawHomeEdge(project, size);
    ctx.strokeStyle = "#5d7a72";
    ctx.lineWidth = 1;
    ctx.strokeRect(0.5, 0.5, size - 1, size - 1);
  }

  private drawPlayer(half: number, size: number): void {
    if (!this.snapshot) return;
    const ctx = this.context;
    const { facing_x: facingX, facing_y: facingY } = this.snapshot.player;
    const reach = Math.max(6, size * 0.05);
    ctx.strokeStyle = "#f4f7f2";
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(half, half);
    ctx.lineTo(
      half + (facingX / 1000) * reach,
      half + (facingY / 1000) * reach,
    );
    ctx.stroke();
    ctx.fillStyle = "#f4f7f2";
    ctx.strokeStyle = "#142028";
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.arc(half, half, Math.max(2.5, size * 0.022), 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();
  }

  /** A chevron at the edge when home is past it, so the minimap keeps answering out of range. */
  private drawHomeEdge(
    project: (point: WorldPoint) => WorldPoint,
    size: number,
  ): void {
    if (!this.snapshot || !this.home) return;
    const point = project(this.home);
    const margin = 7;
    if (
      point.x >= margin &&
      point.y >= margin &&
      point.x <= size - margin &&
      point.y <= size - margin
    )
      return;
    const bearing = homeBearing(this.snapshot.player, this.home);
    if (!bearing) return;
    const ctx = this.context;
    ctx.save();
    ctx.translate(
      Math.min(size - margin, Math.max(margin, point.x)),
      Math.min(size - margin, Math.max(margin, point.y)),
    );
    ctx.rotate(Math.atan2(bearing.y, bearing.x));
    ctx.fillStyle = "#f6c85f";
    ctx.beginPath();
    ctx.moveTo(6, 0);
    ctx.lineTo(-4, 5);
    ctx.lineTo(-4, -5);
    ctx.closePath();
    ctx.fill();
    ctx.restore();
  }
}
