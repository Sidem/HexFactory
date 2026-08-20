import { axialToPixel } from "@hexlife/embed/hex";

import { TERRAIN_INFO } from "../core/terrain";
import type {
  Definitions,
  FactorySnapshot,
  ItemDefinition,
  WorldPoint,
} from "../core/types";
import {
  BUILDING_COLORS,
  MAX_DEVICE_PIXEL_RATIO,
} from "./CanvasFactoryRenderer";
import { parseRgba } from "./gl/color";
import {
  compileProgram,
  createBuffer,
  createVertexArray,
  QUAD,
  Uniforms,
} from "./gl/program";
import { MAP_FS, MAP_VS } from "./gl/shaders";
import { WORLD_SCALE, homeBearing } from "./landmarks";

/**
 * How far from the player the minimap window reaches, in hex steps. Wide enough that a factory and
 * the ground it was sited on fit in one glance, narrow enough that a belt is still a belt.
 */
const MINIMAP_RADIUS_HEXES = 32;
const FOG = parseRgba("#18242f");
const LOWLAND = parseRgba(TERRAIN_INFO.lowland.fill);
const STRIDE = 10;
const CELL_WORLD = WORLD_SCALE * 1.9;

/**
 * The second view of the same snapshot. It derives nothing native has not published: surveyed
 * chunks, terrain bands, buildings, the landing hub, and the player.
 *
 * World geometry is GPU-instanced and rebuilt only when those lists change. Walking updates the
 * player uniform, so the map recentres without restamping every cell.
 */
export class MinimapRenderer {
  private readonly gl: WebGL2RenderingContext;
  private readonly itemsById: ReadonlyMap<number, ItemDefinition>;
  private readonly program: WebGLProgram;
  private readonly uniforms: Uniforms;
  private readonly vao: WebGLVertexArrayObject;
  private readonly worldBuffer: WebGLBuffer;
  private readonly markBuffer: WebGLBuffer;
  private readonly markVao: WebGLVertexArrayObject;
  private snapshot: FactorySnapshot | null = null;
  private home: WorldPoint | null = null;
  private worldData = new Float32Array(STRIDE * 256);
  private worldCount = 0;
  private markData = new Float32Array(STRIDE * 8);
  private markCount = 0;
  private lastChunks: FactorySnapshot["chunks"] | null = null;
  private lastTerrain: FactorySnapshot["terrain"] | null = null;
  private lastResources: FactorySnapshot["resources"] | null = null;
  private lastBuildings: FactorySnapshot["buildings"] | null = null;
  private lost = false;

  constructor(
    private readonly canvas: HTMLCanvasElement,
    definitions: Definitions,
  ) {
    const gl = canvas.getContext("webgl2", {
      alpha: false,
      antialias: false,
      powerPreference: "low-power",
      premultipliedAlpha: false,
    });
    if (!gl) throw new Error("WebGL2 is unavailable");
    this.gl = gl;
    this.itemsById = new Map(definitions.items.map((item) => [item.id, item]));
    this.program = compileProgram(gl, MAP_VS, MAP_FS, "minimap");
    this.uniforms = new Uniforms(gl, this.program);
    const quad = createBuffer(gl);
    gl.bindBuffer(gl.ARRAY_BUFFER, quad);
    gl.bufferData(gl.ARRAY_BUFFER, QUAD, gl.STATIC_DRAW);
    this.worldBuffer = createBuffer(gl);
    this.markBuffer = createBuffer(gl);
    this.vao = this.makeVao(quad, this.worldBuffer);
    this.markVao = this.makeVao(quad, this.markBuffer);
    gl.canvas.addEventListener("webglcontextlost", (event) => {
      event.preventDefault();
      this.lost = true;
    });
    new ResizeObserver(() => this.draw()).observe(canvas);
  }

  setSnapshot(snapshot: FactorySnapshot, home: WorldPoint | null): void {
    this.snapshot = snapshot;
    this.home = home;
    this.draw();
  }

  draw(): void {
    if (this.lost) return;
    const snapshot = this.snapshot;
    const gl = this.gl;
    const css = Math.max(1, this.canvas.clientWidth);
    const ratio = Math.min(
      window.devicePixelRatio || 1,
      MAX_DEVICE_PIXEL_RATIO,
    );
    const dw = Math.max(1, Math.floor(css * ratio));
    if (gl.canvas.width !== dw || gl.canvas.height !== dw) {
      gl.canvas.width = dw;
      gl.canvas.height = dw;
    }
    gl.viewport(0, 0, dw, dw);
    gl.disable(gl.DEPTH_TEST);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.clearColor(FOG[0], FOG[1], FOG[2], 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    if (!snapshot) return;

    const reach = axialToPixel({ q: MINIMAP_RADIUS_HEXES, r: 0 }, WORLD_SCALE, {
      x: 0,
      y: 0,
    }).x;
    const scale = dw / 2 / reach;
    this.syncWorld(snapshot, scale);
    this.packMarks(snapshot, scale, dw);

    gl.useProgram(this.program);
    this.uniforms.vec2("u_player", [snapshot.player.x, snapshot.player.y]);
    this.uniforms.vec2("u_resolution", [dw, dw]);
    this.uniforms.f("u_scale", scale);
    if (this.worldCount > 0) {
      gl.bindVertexArray(this.vao);
      gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, this.worldCount);
    }
    gl.bindVertexArray(this.markVao);
    gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, this.markCount);
    gl.bindVertexArray(null);
  }

  private syncWorld(snapshot: FactorySnapshot, scale: number): void {
    if (
      snapshot.chunks === this.lastChunks &&
      snapshot.terrain === this.lastTerrain &&
      snapshot.resources === this.lastResources &&
      snapshot.buildings === this.lastBuildings
    )
      return;
    this.lastChunks = snapshot.chunks;
    this.lastTerrain = snapshot.terrain;
    this.lastResources = snapshot.resources;
    this.lastBuildings = snapshot.buildings;
    this.worldCount = 0;
    const minPx = 1 / scale;
    for (const chunk of snapshot.chunks) {
      const half = chunk.span / 2;
      this.pushWorld(
        chunk.x + half,
        chunk.y + half,
        Math.max(half, minPx),
        Math.max(half, minPx),
        LOWLAND,
        0,
        0,
      );
    }
    const cell = Math.max(minPx, CELL_WORLD / 2);
    for (const region of snapshot.terrain) {
      const band = TERRAIN_INFO[region.terrain];
      this.pushWorld(
        region.x,
        region.y,
        cell,
        cell,
        parseRgba(band.passable ? band.fill : band.stroke),
        0,
        0,
      );
    }
    for (const resource of snapshot.resources) {
      const color =
        resource.quantity === 0
          ? parseRgba("#6a6560")
          : parseRgba(this.itemsById.get(resource.item_id)?.color ?? "#fff");
      if (resource.quantity === 0) color[3] = 0.35;
      this.pushWorld(resource.x, resource.y, cell / 2, cell / 2, color, 0, 0);
    }
    const mark = Math.max(3 / scale, cell * 2);
    for (const building of snapshot.buildings) {
      const point = axialToPixel(building, WORLD_SCALE, { x: 0, y: 0 });
      const width = building.kind === "hub" ? mark * 2 : mark;
      const color = parseRgba(BUILDING_COLORS[building.kind]);
      if (building.kind === "hub")
        this.pushWorld(
          point.x,
          point.y,
          width / 2 + 1 / scale,
          width / 2 + 1 / scale,
          parseRgba("#fff3c0"),
          0,
          0,
        );
      this.pushWorld(point.x, point.y, width / 2, width / 2, color, 0, 0);
    }
    this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.worldBuffer);
    this.gl.bufferData(
      this.gl.ARRAY_BUFFER,
      this.worldData.subarray(0, this.worldCount * STRIDE),
      this.gl.DYNAMIC_DRAW,
    );
  }

  private packMarks(
    snapshot: FactorySnapshot,
    scale: number,
    dw: number,
  ): void {
    const player = snapshot.player;
    const radius = Math.max(2.5, (dw / scale) * 0.022);
    const reach = Math.max(6 / scale, (dw / scale) * 0.05);
    const fx = player.facing_x / 1000;
    const fy = player.facing_y / 1000;
    const angle = Math.atan2(fy, fx);
    this.markData.fill(0);
    writeInstance(this.markData, 0, [
      player.x + (fx * reach) / 2,
      player.y + (fy * reach) / 2,
      reach / 2,
      1 / scale,
      0.957,
      0.969,
      0.949,
      1,
      angle,
      0,
    ]);
    writeInstance(this.markData, 1, [
      player.x,
      player.y,
      radius,
      radius,
      0.957,
      0.969,
      0.949,
      1,
      0,
      1,
    ]);
    this.markCount = 2;
    const home = this.homeMarker(snapshot, scale, dw);
    if (home) {
      writeInstance(this.markData, 2, home);
      this.markCount = 3;
    }
    this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.markBuffer);
    this.gl.bufferData(
      this.gl.ARRAY_BUFFER,
      this.markData.subarray(0, this.markCount * STRIDE),
      this.gl.DYNAMIC_DRAW,
    );
  }

  private homeMarker(
    snapshot: FactorySnapshot,
    scale: number,
    dw: number,
  ): number[] | null {
    if (!this.home) return null;
    const dx = this.home.x - snapshot.player.x;
    const dy = this.home.y - snapshot.player.y;
    const half = dw / 2;
    const sx = half + dx * scale;
    const sy = half + dy * scale;
    const margin = 7;
    if (sx >= margin && sy >= margin && sx <= dw - margin && sy <= dw - margin)
      return null;
    const bearing = homeBearing(snapshot.player, this.home);
    if (!bearing) return null;
    const clampedX = Math.min(dw - margin, Math.max(margin, sx));
    const clampedY = Math.min(dw - margin, Math.max(margin, sy));
    const worldX = snapshot.player.x + (clampedX - half) / scale;
    const worldY = snapshot.player.y + (clampedY - half) / scale;
    return [
      worldX,
      worldY,
      6 / scale,
      5 / scale,
      0.965,
      0.784,
      0.373,
      1,
      Math.atan2(bearing.y, bearing.x),
      0,
    ];
  }

  private pushWorld(
    x: number,
    y: number,
    extX: number,
    extY: number,
    color: [number, number, number, number],
    angle: number,
    shape: number,
  ): void {
    this.worldData = grow(this.worldData, (this.worldCount + 1) * STRIDE);
    writeInstance(this.worldData, this.worldCount, [
      x,
      y,
      extX,
      extY,
      ...color,
      angle,
      shape,
    ]);
    this.worldCount += 1;
  }

  private makeVao(
    quad: WebGLBuffer,
    instances: WebGLBuffer,
  ): WebGLVertexArrayObject {
    const gl = this.gl;
    const vao = createVertexArray(gl);
    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, quad);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, instances);
    const stride = STRIDE * 4;
    for (const [index, size, offset] of [
      [1, 2, 0],
      [2, 2, 2],
      [3, 4, 4],
      [4, 2, 8],
    ] as const) {
      gl.enableVertexAttribArray(index);
      gl.vertexAttribPointer(index, size, gl.FLOAT, false, stride, offset * 4);
      gl.vertexAttribDivisor(index, 1);
    }
    gl.bindVertexArray(null);
    return vao;
  }
}

function writeInstance(
  data: Float32Array,
  index: number,
  values: number[],
): void {
  data.set(values, index * STRIDE);
}

function grow(data: Float32Array, need: number): Float32Array {
  if (data.length >= need) return data;
  let size = data.length || 32;
  while (size < need) size *= 2;
  const next = new Float32Array(size);
  next.set(data);
  return next;
}
