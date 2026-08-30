import {
  axialToPixel,
  pixelToAxial,
  type AxialCoordinate,
} from "@hexlife/embed/hex";

import { TERRAIN_INFO, TERRAIN_ORDER } from "../../core/terrain";
import type { ReachRadii } from "../FactoryRenderer";
import type {
  BuildingDefinition,
  ChunkSnapshot,
  Definitions,
  EntitySnapshot,
  FactorySnapshot,
  ItemDefinition,
  LinePreviewCell,
  PlacementPreview,
  Terrain,
} from "../../core/types";
import {
  BUILDING_SHAPES,
  buildingStamp,
  silhouetteOf,
  trimOf,
  type SilhouetteKey,
} from "../buildingLook";
import { ITEM_ICON_KEYS, bakeItemIcon } from "../icons";
import { WORLD_SCALE } from "../landmarks";
import { hexLook, TerrainTiles } from "../terrainLook";
import { parseRgba } from "./color";
import {
  compileProgram,
  createBuffer,
  createTexture,
  createVertexArray,
  Uniforms,
  QUAD,
} from "./program";
import {
  CHUNK_FS,
  CHUNK_VS,
  FOG_FS,
  FOG_VS,
  HEX_FS,
  HEX_VS,
  SPRITE_FS,
  SPRITE_VS,
} from "./shaders";

const COVERAGE_SIZE = 512;
const FOG_COLOR = parseRgba("#18242fee");
const LOWLAND_FILL = parseRgba(TERRAIN_INFO.lowland.fill);
const HEX_STRIDE = 18;
const SPRITE_STRIDE = 12;
const CHUNK_STRIDE = 3;

export type BuildingColorTable = Record<EntitySnapshot["kind"], string>;

export interface WorldGlOverlays {
  hover: AxialCoordinate | null;
  selection: AxialCoordinate | null;
  placement: PlacementPreview | null;
  dragPath: LinePreviewCell[];
  buildMode: boolean;
  gridToggled: boolean;
  buildFootprint: AxialCoordinate[];
  buildReach: ReachRadii | null;
  gathering: boolean;
}

/**
 * GPU world: terrain, fog, resources, grid, coverage, building stamps. Camera is a uniform, so
 * walking does not restamp the mosaic. Presentation only.
 */
export class WorldGl {
  private readonly tiles = new TerrainTiles();
  private readonly itemsById: ReadonlyMap<number, ItemDefinition>;
  private readonly buildingsById: ReadonlyMap<number, BuildingDefinition>;
  private readonly bandIndex = new Map<Terrain, number>(
    TERRAIN_ORDER.map((band, index) => [band, index]),
  );

  private readonly hexProgram: WebGLProgram;
  private readonly spriteProgram: WebGLProgram;
  private readonly chunkProgram: WebGLProgram;
  private readonly fogProgram: WebGLProgram;
  private readonly hexU: Uniforms;
  private readonly spriteU: Uniforms;
  private readonly chunkU: Uniforms;
  private readonly fogU: Uniforms;
  private readonly hexVao: WebGLVertexArrayObject;
  private readonly overlayVao: WebGLVertexArrayObject;
  private readonly spriteVao: WebGLVertexArrayObject;
  private readonly chunkVao: WebGLVertexArrayObject;
  private readonly fogVao: WebGLVertexArrayObject;
  private readonly hexInstances: WebGLBuffer;
  private readonly overlayInstances: WebGLBuffer;
  private readonly spriteInstances: WebGLBuffer;
  private readonly chunkInstances: WebGLBuffer;
  private readonly tileArray: WebGLTexture;
  private readonly fieldTex: WebGLTexture;
  private readonly stampAtlas: WebGLTexture;
  private readonly iconAtlas: WebGLTexture;
  private readonly coverageTex: WebGLTexture;
  private readonly stampIndex = new Map<
    string,
    { u0: number; v0: number; u1: number; v1: number }
  >();
  private readonly iconIndex = new Map<
    string,
    { u0: number; v0: number; u1: number; v1: number }
  >();

  private hexData = new Float32Array(HEX_STRIDE * 512);
  private hexCount = 0;
  private overlayData = new Float32Array(HEX_STRIDE * 64);
  private overlayCount = 0;
  private spriteData = new Float32Array(SPRITE_STRIDE * 256);
  private spriteCount = 0;
  private spriteStampCount = 0;
  private chunkData = new Float32Array(CHUNK_STRIDE * 64);
  private chunkCount = 0;
  private chunkKey = "";
  private lastTerrain: FactorySnapshot["terrain"] | null = null;
  private lastResources: FactorySnapshot["resources"] | null = null;
  private lastBuildings: FactorySnapshot["buildings"] | null = null;
  private lastGround: FactorySnapshot["ground"] | null = null;
  private lastStage = -1;
  private lastReduced = false;
  private coverageWorldMin: [number, number] = [0, 0];
  private coverageWorldSpan: [number, number] = [1, 1];
  private lost = false;

  constructor(
    private readonly gl: WebGL2RenderingContext,
    definitions: Definitions,
    private readonly buildingColors: BuildingColorTable,
    private readonly baseHexSize: number,
  ) {
    this.itemsById = new Map(definitions.items.map((item) => [item.id, item]));
    this.buildingsById = new Map(
      definitions.buildings.map((building) => [building.id, building]),
    );
    this.hexProgram = compileProgram(gl, HEX_VS, HEX_FS, "hex");
    this.spriteProgram = compileProgram(gl, SPRITE_VS, SPRITE_FS, "sprite");
    this.chunkProgram = compileProgram(gl, CHUNK_VS, CHUNK_FS, "chunk");
    this.fogProgram = compileProgram(gl, FOG_VS, FOG_FS, "fog");
    this.hexU = new Uniforms(gl, this.hexProgram);
    this.spriteU = new Uniforms(gl, this.spriteProgram);
    this.chunkU = new Uniforms(gl, this.chunkProgram);
    this.fogU = new Uniforms(gl, this.fogProgram);
    const quad = createBuffer(gl);
    gl.bindBuffer(gl.ARRAY_BUFFER, quad);
    gl.bufferData(gl.ARRAY_BUFFER, QUAD, gl.STATIC_DRAW);
    this.hexInstances = createBuffer(gl);
    this.overlayInstances = createBuffer(gl);
    this.spriteInstances = createBuffer(gl);
    this.chunkInstances = createBuffer(gl);
    this.hexVao = this.makeHexVao(quad, this.hexInstances);
    this.overlayVao = this.makeHexVao(quad, this.overlayInstances);
    this.spriteVao = this.makeSpriteVao(quad);
    this.chunkVao = this.makeChunkVao(quad);
    this.fogVao = this.makeFogVao(quad);
    this.tileArray = this.uploadTileArray();
    this.fieldTex = this.uploadCanvas(this.tiles.field("lowland"), true);
    this.stampAtlas = this.uploadCanvas(packStampAtlas(this.stampIndex), false);
    this.iconAtlas = this.uploadCanvas(packIconAtlas(this.iconIndex), false);
    this.coverageTex = createTexture(gl);
    gl.bindTexture(gl.TEXTURE_2D, this.coverageTex);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texImage2D(
      gl.TEXTURE_2D,
      0,
      gl.R8,
      COVERAGE_SIZE,
      COVERAGE_SIZE,
      0,
      gl.RED,
      gl.UNSIGNED_BYTE,
      new Uint8Array(COVERAGE_SIZE * COVERAGE_SIZE),
    );
    gl.canvas.addEventListener("webglcontextlost", (event) => {
      event.preventDefault();
      this.lost = true;
    });
  }

  get isLost(): boolean {
    return this.lost;
  }

  draw(
    snapshot: FactorySnapshot,
    origin: { x: number; y: number },
    width: number,
    height: number,
    ratio: number,
    zoom: number,
    now: number,
    reducedMotion: boolean,
    overlays: WorldGlOverlays,
  ): void {
    if (this.lost) return;
    const gl = this.gl;
    const dw = Math.max(1, Math.floor(width * ratio));
    const dh = Math.max(1, Math.floor(height * ratio));
    if (gl.canvas.width !== dw || gl.canvas.height !== dh) {
      gl.canvas.width = dw;
      gl.canvas.height = dh;
    }
    gl.viewport(0, 0, dw, dh);
    gl.disable(gl.DEPTH_TEST);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.clearColor(FOG_COLOR[0], FOG_COLOR[1], FOG_COLOR[2], 1);
    gl.clear(gl.COLOR_BUFFER_BIT);

    const hex = this.baseHexSize * zoom * ratio;
    const scale = hex / WORLD_SCALE;
    const originPx: [number, number] = [origin.x * ratio, origin.y * ratio];
    const resolution: [number, number] = [dw, dh];
    const time = reducedMotion ? 0 : now;

    this.syncChunks(snapshot.chunks);
    this.syncStatic(snapshot, reducedMotion);
    this.syncOverlay(snapshot, origin, width, height, zoom, overlays);

    this.drawChunks(originPx, resolution, scale);
    this.drawHexBatches(originPx, resolution, scale, hex, time);
    this.drawSprites(originPx, resolution, scale, hex);
    this.drawFog(originPx, resolution, scale);
  }

  private syncStatic(snapshot: FactorySnapshot, reducedMotion: boolean): void {
    if (
      snapshot.terrain === this.lastTerrain &&
      snapshot.resources === this.lastResources &&
      snapshot.buildings === this.lastBuildings &&
      snapshot.ground === this.lastGround &&
      snapshot.contract.stage === this.lastStage &&
      reducedMotion === this.lastReduced
    )
      return;
    this.lastTerrain = snapshot.terrain;
    this.lastResources = snapshot.resources;
    this.lastBuildings = snapshot.buildings;
    this.lastGround = snapshot.ground;
    this.lastStage = snapshot.contract.stage;
    this.lastReduced = reducedMotion;
    // The flat view has no height to show a quarried cliff with, so the one thing it must get right
    // is the hatch: a face the player has already taken down must stop being drawn as a wall. Every
    // other consequence of a grade is stated by the overlay drawn over the top of this batch.
    const quarried = new Set(
      snapshot.ground
        .filter((cell) => cell.elevation < 0)
        .map((cell) => `${cell.q},${cell.r}`),
    );
    this.hexCount = 0;
    for (const region of snapshot.terrain) {
      const info = TERRAIN_INFO[region.terrain];
      const passable =
        info.passable ||
        (region.terrain === "cliff" && quarried.has(`${region.q},${region.r}`));
      const look = hexLook(region.q, region.r);
      this.pushHex(
        region.x,
        region.y,
        parseRgba(info.fill),
        parseRgba(info.stroke),
        1,
        (look.rotation * Math.PI) / 3,
        (this.bandIndex.get(region.terrain) ?? 3) + 1,
        0,
        look.jitter,
        look.salt,
        passable ? 0 : 1,
        region.terrain === "deep_water" || region.terrain === "shallow_water"
          ? 1
          : 0,
      );
    }
    for (const resource of snapshot.resources) {
      if (resource.quantity <= 0) continue;
      const item = this.itemsById.get(resource.item_id);
      if (item?.key === "wood") continue;
      const color = parseRgba(item?.color ?? "#ffffff");
      const fill: [number, number, number, number] = [
        color[0],
        color[1],
        color[2],
        0.25,
      ];
      const remaining =
        resource.initial_quantity > 0
          ? resource.quantity / resource.initial_quantity
          : 1;
      this.pushHex(
        resource.x,
        resource.y,
        fill,
        color,
        0.62,
        0,
        0,
        reducedMotion ? 0 : 1,
        -(1 - remaining) * 0.45,
        0,
        0,
        0,
      );
    }
    for (const building of snapshot.buildings) {
      const definition = this.buildingsById.get(building.definition_id);
      const color = parseRgba(this.buildingColors[building.kind]);
      if (building.status === "no power" || building.status === "brownout")
        color[3] *= 0.72;
      const stroke = parseRgba(trimOf(definition?.tier ?? 0).stroke);
      for (const cell of building.footprint) {
        if (cell.q === building.q && cell.r === building.r) continue;
        const point = axialToPixel(cell, WORLD_SCALE, { x: 0, y: 0 });
        this.pushHex(
          point.x,
          point.y,
          color,
          parseRgba("#dce7ef"),
          0.78,
          0,
          0,
          0,
          0,
          0,
          0,
          0,
        );
      }
      const world = axialToPixel(building, WORLD_SCALE, { x: 0, y: 0 });
      this.pushHex(world.x, world.y, color, stroke, 0.8, 0, 0, 0, 0, 0, 0, 0);
    }
    this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.hexInstances);
    this.gl.bufferData(
      this.gl.ARRAY_BUFFER,
      this.hexData.subarray(0, this.hexCount * HEX_STRIDE),
      this.gl.DYNAMIC_DRAW,
    );
    this.packSprites(snapshot);
  }

  private syncOverlay(
    snapshot: FactorySnapshot,
    origin: { x: number; y: number },
    width: number,
    height: number,
    zoom: number,
    overlays: WorldGlOverlays,
  ): void {
    this.overlayCount = 0;
    const need =
      overlays.buildMode ||
      overlays.gridToggled ||
      overlays.hover !== null ||
      overlays.selection !== null ||
      overlays.dragPath.length > 0 ||
      overlays.buildReach !== null ||
      overlays.gathering;
    if (need)
      this.packOverlayHexes(snapshot, origin, width, height, zoom, overlays);
    this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.overlayInstances);
    this.gl.bufferData(
      this.gl.ARRAY_BUFFER,
      this.overlayData.subarray(0, this.overlayCount * HEX_STRIDE),
      this.gl.DYNAMIC_DRAW,
    );
  }

  private packOverlayHexes(
    snapshot: FactorySnapshot,
    origin: { x: number; y: number },
    width: number,
    height: number,
    zoom: number,
    overlays: WorldGlOverlays,
  ): void {
    const hexSize = this.baseHexSize * zoom;
    if (overlays.buildMode || overlays.gridToggled) {
      const corners = [
        pixelToAxial({ x: 0, y: 0 }, hexSize, origin),
        pixelToAxial({ x: width, y: 0 }, hexSize, origin),
        pixelToAxial({ x: 0, y: height }, hexSize, origin),
        pixelToAxial({ x: width, y: height }, hexSize, origin),
      ];
      const minQ = Math.min(...corners.map(({ q }) => q)) - 3;
      const maxQ = Math.max(...corners.map(({ q }) => q)) + 3;
      const minR = Math.min(...corners.map(({ r }) => r)) - 3;
      const maxR = Math.max(...corners.map(({ r }) => r)) + 3;
      const stroke = parseRgba("#9bb9af2d");
      const fill = parseRgba("#00000000");
      for (let q = minQ; q <= maxQ; q += 1) {
        for (let r = minR; r <= maxR; r += 1) {
          const world = axialToPixel({ q, r }, WORLD_SCALE, { x: 0, y: 0 });
          this.pushOverlay(
            world.x,
            world.y,
            fill,
            stroke,
            0.97,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
          );
        }
      }
    }
    const rings: {
      center: AxialCoordinate;
      radius: number;
      kind: "extract" | "supply" | "link";
    }[] = [];
    if (
      overlays.buildReach !== null &&
      overlays.hover &&
      !overlays.dragPath.length
    ) {
      const { extract, supply, link } = overlays.buildReach;
      if (extract !== null)
        rings.push({
          center: overlays.hover,
          radius: extract,
          kind: "extract",
        });
      if (supply !== null)
        rings.push({ center: overlays.hover, radius: supply, kind: "supply" });
      if (link !== null)
        rings.push({ center: overlays.hover, radius: link, kind: "link" });
    }
    if (overlays.selection) {
      const selected = snapshot.buildings.findLast((building) =>
        building.footprint.some(
          (cell) =>
            cell.q === overlays.selection!.q &&
            cell.r === overlays.selection!.r,
        ),
      );
      const definition = selected
        ? this.buildingsById.get(selected.definition_id)
        : undefined;
      const center = selected ? { q: selected.q, r: selected.r } : null;
      if (center && definition?.extract_radius !== undefined)
        rings.push({
          center,
          radius: definition.extract_radius,
          kind: "extract",
        });
      if (center && definition?.supply_radius !== undefined)
        rings.push({
          center,
          radius: definition.supply_radius,
          kind: "supply",
        });
      if (center && definition?.pole_reach !== undefined)
        rings.push({ center, radius: definition.pole_reach, kind: "link" });
    }
    if (overlays.gathering) {
      rings.push({
        center: pixelToAxial(snapshot.player, WORLD_SCALE, { x: 0, y: 0 }),
        radius: snapshot.player.extract_radius,
        kind: "extract",
      });
    }
    for (const ring of rings) {
      const fill =
        ring.kind === "extract"
          ? parseRgba("#76e0aa16")
          : ring.kind === "supply"
            ? parseRgba("#8fd4ff10")
            : parseRgba("#00000000");
      const rim =
        ring.kind === "extract"
          ? parseRgba("#76e0aa88")
          : ring.kind === "supply"
            ? parseRgba("#8fd4ff70")
            : parseRgba("#f5d572aa");
      for (let dq = -ring.radius; dq <= ring.radius; dq += 1) {
        for (let dr = -ring.radius; dr <= ring.radius; dr += 1) {
          const cell = { q: ring.center.q + dq, r: ring.center.r + dr };
          const distance =
            (Math.abs(cell.q - ring.center.q) +
              Math.abs(cell.r - ring.center.r) +
              Math.abs(cell.q - ring.center.q + (cell.r - ring.center.r))) /
            2;
          if (distance > ring.radius) continue;
          const world = axialToPixel(cell, WORLD_SCALE, { x: 0, y: 0 });
          this.pushOverlay(
            world.x,
            world.y,
            fill,
            distance === ring.radius ? rim : parseRgba("#00000000"),
            0.95,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
          );
        }
      }
    }
    if (overlays.dragPath.length) {
      for (const cell of overlays.dragPath) {
        const stroke = parseRgba(cell.legal ? "#76e0aa" : "#ff7b78");
        const fill: [number, number, number, number] = [
          stroke[0],
          stroke[1],
          stroke[2],
          0.13,
        ];
        const world = axialToPixel(cell, WORLD_SCALE, { x: 0, y: 0 });
        this.pushOverlay(
          world.x,
          world.y,
          fill,
          stroke,
          0.88,
          0,
          0,
          0,
          0,
          0,
          0,
          0,
        );
      }
    } else if (overlays.hover) {
      const stroke = parseRgba(
        overlays.placement
          ? overlays.placement.legal
            ? "#76e0aa"
            : "#ff7b78"
          : "#e9f0f7",
      );
      const fill: [number, number, number, number] = [
        stroke[0],
        stroke[1],
        stroke[2],
        0.09,
      ];
      const footprint = overlays.buildMode
        ? overlays.buildFootprint
        : [{ q: 0, r: 0 }];
      for (const offset of footprint) {
        const world = axialToPixel(
          {
            q: overlays.hover.q + offset.q,
            r: overlays.hover.r + offset.r,
          },
          WORLD_SCALE,
          { x: 0, y: 0 },
        );
        this.pushOverlay(
          world.x,
          world.y,
          fill,
          stroke,
          0.88,
          0,
          0,
          0,
          0,
          0,
          0,
          0,
        );
      }
    }
    if (overlays.selection) {
      const world = axialToPixel(overlays.selection, WORLD_SCALE, {
        x: 0,
        y: 0,
      });
      this.pushOverlay(
        world.x,
        world.y,
        parseRgba("#ffffff08"),
        parseRgba("#f5d572"),
        0.91,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
      );
    }
  }

  private packSprites(snapshot: FactorySnapshot): void {
    this.spriteCount = 0;
    const growth = snapshot.contract.stage;
    for (const building of snapshot.buildings) {
      const definition = this.buildingsById.get(building.definition_id);
      const key = silhouetteOf(
        building.kind,
        definition?.recipe_category ?? definition?.source_category,
        definition?.power_source,
      );
      const uv = this.stampIndex.get(
        `${key}|${definition?.tier ?? 0}|${building.kind === "hub" ? growth : 0}`,
      );
      if (!uv) continue;
      const world = axialToPixel(building, WORLD_SCALE, { x: 0, y: 0 });
      this.pushSprite(world.x, world.y, uv, [1, 1, 1, 1], (236 / 128) * 0.8);
    }
    this.spriteStampCount = this.spriteCount;
    for (const resource of snapshot.resources) {
      if (resource.quantity <= 0) continue;
      const item = this.itemsById.get(resource.item_id);
      if (item?.key === "wood") continue;
      const uv = this.iconIndex.get(item?.icon ?? "ore");
      if (!uv) continue;
      this.pushSprite(
        resource.x,
        resource.y,
        uv,
        parseRgba(item?.color ?? "#fff"),
        0.9,
      );
    }
    this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.spriteInstances);
    this.gl.bufferData(
      this.gl.ARRAY_BUFFER,
      this.spriteData.subarray(0, this.spriteCount * SPRITE_STRIDE),
      this.gl.DYNAMIC_DRAW,
    );
  }

  private drawChunks(
    origin: [number, number],
    resolution: [number, number],
    scale: number,
  ): void {
    if (this.chunkCount === 0) return;
    const gl = this.gl;
    gl.useProgram(this.chunkProgram);
    this.chunkU.vec2("u_origin", origin);
    this.chunkU.vec2("u_resolution", resolution);
    this.chunkU.f("u_scale", scale);
    this.chunkU.vec4("u_fill", LOWLAND_FILL);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.fieldTex);
    this.chunkU.i("u_field", 0);
    gl.bindVertexArray(this.chunkVao);
    gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, this.chunkCount);
    gl.bindVertexArray(null);
  }

  private drawHexBatches(
    origin: [number, number],
    resolution: [number, number],
    scale: number,
    hex: number,
    time: number,
  ): void {
    if (this.hexCount === 0 && this.overlayCount === 0) return;
    const gl = this.gl;
    gl.useProgram(this.hexProgram);
    this.hexU.vec2("u_origin", origin);
    this.hexU.vec2("u_resolution", resolution);
    this.hexU.f("u_scale", scale);
    this.hexU.f("u_hex", hex);
    this.hexU.f("u_time", time);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D_ARRAY, this.tileArray);
    this.hexU.i("u_tiles", 0);
    if (this.hexCount > 0) {
      gl.bindVertexArray(this.hexVao);
      gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, this.hexCount);
    }
    if (this.overlayCount > 0) {
      gl.bindVertexArray(this.overlayVao);
      gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, this.overlayCount);
    }
    gl.bindVertexArray(null);
  }

  private drawSprites(
    origin: [number, number],
    resolution: [number, number],
    scale: number,
    hex: number,
  ): void {
    if (this.spriteCount === 0) return;
    const gl = this.gl;
    gl.useProgram(this.spriteProgram);
    this.spriteU.vec2("u_origin", origin);
    this.spriteU.vec2("u_resolution", resolution);
    this.spriteU.f("u_scale", scale);
    this.spriteU.f("u_hex", hex);
    this.spriteU.i("u_atlas", 0);
    gl.bindVertexArray(this.spriteVao);
    const stamps = this.spriteStampCount;
    if (stamps > 0) {
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, this.stampAtlas);
      gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, stamps);
    }
    if (this.spriteCount > stamps) {
      gl.bindTexture(gl.TEXTURE_2D, this.iconAtlas);
      bindSpriteAttribs(gl, this.spriteInstances, stamps * SPRITE_STRIDE * 4);
      gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, this.spriteCount - stamps);
      bindSpriteAttribs(gl, this.spriteInstances, 0);
    }
    gl.bindVertexArray(null);
  }

  private drawFog(
    origin: [number, number],
    resolution: [number, number],
    scale: number,
  ): void {
    const gl = this.gl;
    gl.useProgram(this.fogProgram);
    this.fogU.vec2("u_origin", origin);
    this.fogU.vec2("u_resolution", resolution);
    this.fogU.f("u_scale", scale);
    this.fogU.vec2("u_worldMin", this.coverageWorldMin);
    this.fogU.vec2("u_worldSpan", this.coverageWorldSpan);
    this.fogU.vec4("u_fog", FOG_COLOR);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.coverageTex);
    this.fogU.i("u_coverage", 0);
    gl.bindVertexArray(this.fogVao);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
    gl.bindVertexArray(null);
  }

  private syncChunks(chunks: ChunkSnapshot[]): void {
    const key = `${chunks.length}:${chunks[0]?.x ?? 0}:${chunks[0]?.y ?? 0}:${chunks.at(-1)?.x ?? 0}:${chunks.at(-1)?.span ?? 0}`;
    if (key === this.chunkKey) return;
    this.chunkKey = key;
    this.chunkCount = 0;
    this.chunkData = grow(this.chunkData, chunks.length * CHUNK_STRIDE);
    for (const chunk of chunks) {
      write(this.chunkData, this.chunkCount * CHUNK_STRIDE, [
        chunk.x,
        chunk.y,
        chunk.span,
      ]);
      this.chunkCount += 1;
    }
    this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.chunkInstances);
    this.gl.bufferData(
      this.gl.ARRAY_BUFFER,
      this.chunkData.subarray(0, this.chunkCount * CHUNK_STRIDE),
      this.gl.DYNAMIC_DRAW,
    );
    this.uploadCoverage(chunks);
  }

  private pushHex(
    x: number,
    y: number,
    fill: [number, number, number, number],
    stroke: [number, number, number, number],
    sizeScale: number,
    rotation: number,
    layer: number,
    pulse: number,
    jitter: number,
    salt: number,
    hatch: number,
    water: number,
  ): void {
    this.hexData = grow(this.hexData, (this.hexCount + 1) * HEX_STRIDE);
    write(this.hexData, this.hexCount * HEX_STRIDE, [
      x,
      y,
      ...fill,
      ...stroke,
      sizeScale,
      rotation,
      layer,
      pulse,
      jitter,
      salt,
      hatch,
      water,
    ]);
    this.hexCount += 1;
  }

  private pushOverlay(
    x: number,
    y: number,
    fill: [number, number, number, number],
    stroke: [number, number, number, number],
    sizeScale: number,
    rotation: number,
    layer: number,
    pulse: number,
    jitter: number,
    salt: number,
    hatch: number,
    water: number,
  ): void {
    this.overlayData = grow(
      this.overlayData,
      (this.overlayCount + 1) * HEX_STRIDE,
    );
    write(this.overlayData, this.overlayCount * HEX_STRIDE, [
      x,
      y,
      ...fill,
      ...stroke,
      sizeScale,
      rotation,
      layer,
      pulse,
      jitter,
      salt,
      hatch,
      water,
    ]);
    this.overlayCount += 1;
  }

  private pushSprite(
    x: number,
    y: number,
    uv: { u0: number; v0: number; u1: number; v1: number },
    tint: [number, number, number, number],
    dim: number,
  ): void {
    this.spriteData = grow(
      this.spriteData,
      (this.spriteCount + 1) * SPRITE_STRIDE,
    );
    write(this.spriteData, this.spriteCount * SPRITE_STRIDE, [
      x,
      y,
      uv.u0,
      uv.v0,
      uv.u1,
      uv.v1,
      ...tint,
      dim,
      dim,
    ]);
    this.spriteCount += 1;
  }

  private uploadCoverage(chunks: ChunkSnapshot[]): void {
    const gl = this.gl;
    const data = new Uint8Array(COVERAGE_SIZE * COVERAGE_SIZE);
    if (!chunks.length) {
      this.coverageWorldMin = [0, 0];
      this.coverageWorldSpan = [1, 1];
      gl.bindTexture(gl.TEXTURE_2D, this.coverageTex);
      gl.texSubImage2D(
        gl.TEXTURE_2D,
        0,
        0,
        0,
        COVERAGE_SIZE,
        COVERAGE_SIZE,
        gl.RED,
        gl.UNSIGNED_BYTE,
        data,
      );
      return;
    }
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const chunk of chunks) {
      minX = Math.min(minX, chunk.x);
      minY = Math.min(minY, chunk.y);
      maxX = Math.max(maxX, chunk.x + chunk.span);
      maxY = Math.max(maxY, chunk.y + chunk.span);
    }
    const pad = Math.max(chunks[0]?.span ?? 1, 1) * 0.12;
    minX -= pad;
    minY -= pad;
    maxX += pad;
    maxY += pad;
    const spanX = Math.max(1, maxX - minX);
    const spanY = Math.max(1, maxY - minY);
    this.coverageWorldMin = [minX, minY];
    this.coverageWorldSpan = [spanX, spanY];
    for (const chunk of chunks) {
      const x0 = Math.floor(((chunk.x - minX) / spanX) * COVERAGE_SIZE);
      const y0 = Math.floor(((chunk.y - minY) / spanY) * COVERAGE_SIZE);
      const x1 = Math.ceil(
        ((chunk.x + chunk.span - minX) / spanX) * COVERAGE_SIZE,
      );
      const y1 = Math.ceil(
        ((chunk.y + chunk.span - minY) / spanY) * COVERAGE_SIZE,
      );
      for (let y = Math.max(0, y0); y < Math.min(COVERAGE_SIZE, y1); y += 1) {
        for (let x = Math.max(0, x0); x < Math.min(COVERAGE_SIZE, x1); x += 1) {
          data[y * COVERAGE_SIZE + x] = 255;
        }
      }
    }
    gl.bindTexture(gl.TEXTURE_2D, this.coverageTex);
    gl.texSubImage2D(
      gl.TEXTURE_2D,
      0,
      0,
      0,
      COVERAGE_SIZE,
      COVERAGE_SIZE,
      gl.RED,
      gl.UNSIGNED_BYTE,
      data,
    );
  }

  private makeHexVao(
    quad: WebGLBuffer,
    instances: WebGLBuffer,
  ): WebGLVertexArrayObject {
    const gl = this.gl;
    const vao = createVertexArray(gl);
    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, quad);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    bindHexAttribs(gl, instances, 0);
    gl.bindVertexArray(null);
    return vao;
  }

  private makeSpriteVao(quad: WebGLBuffer): WebGLVertexArrayObject {
    const gl = this.gl;
    const vao = createVertexArray(gl);
    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, quad);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    bindSpriteAttribs(gl, this.spriteInstances, 0);
    gl.bindVertexArray(null);
    return vao;
  }

  private makeChunkVao(quad: WebGLBuffer): WebGLVertexArrayObject {
    const gl = this.gl;
    const vao = createVertexArray(gl);
    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, quad);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.chunkInstances);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 2, gl.FLOAT, false, CHUNK_STRIDE * 4, 0);
    gl.vertexAttribDivisor(1, 1);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 1, gl.FLOAT, false, CHUNK_STRIDE * 4, 8);
    gl.vertexAttribDivisor(2, 1);
    gl.bindVertexArray(null);
    return vao;
  }

  private makeFogVao(quad: WebGLBuffer): WebGLVertexArrayObject {
    const gl = this.gl;
    const vao = createVertexArray(gl);
    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, quad);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    gl.bindVertexArray(null);
    return vao;
  }

  private uploadTileArray(): WebGLTexture {
    const gl = this.gl;
    const size = this.tiles.tile("lowland").width;
    const texture = createTexture(gl);
    gl.bindTexture(gl.TEXTURE_2D_ARRAY, texture);
    gl.texParameteri(gl.TEXTURE_2D_ARRAY, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D_ARRAY, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D_ARRAY, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D_ARRAY, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texStorage3D(
      gl.TEXTURE_2D_ARRAY,
      1,
      gl.RGBA8,
      size,
      size,
      TERRAIN_ORDER.length,
    );
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, 1);
    TERRAIN_ORDER.forEach((band, index) => {
      gl.texSubImage3D(
        gl.TEXTURE_2D_ARRAY,
        0,
        0,
        0,
        index,
        size,
        size,
        1,
        gl.RGBA,
        gl.UNSIGNED_BYTE,
        this.tiles.tile(band),
      );
    });
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, 0);
    return texture;
  }

  private uploadCanvas(
    canvas: HTMLCanvasElement,
    repeat: boolean,
  ): WebGLTexture {
    const gl = this.gl;
    const texture = createTexture(gl);
    gl.bindTexture(gl.TEXTURE_2D, texture);
    const wrap = repeat ? gl.REPEAT : gl.CLAMP_TO_EDGE;
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, wrap);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, wrap);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, 1);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, canvas);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, 0);
    return texture;
  }
}

function bindHexAttribs(
  gl: WebGL2RenderingContext,
  buffer: WebGLBuffer,
  byteOffset: number,
): void {
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  const stride = HEX_STRIDE * 4;
  for (const [index, size, offset] of [
    [1, 2, 0],
    [2, 4, 2],
    [3, 4, 6],
    [4, 4, 10],
    [5, 4, 14],
  ] as const) {
    gl.enableVertexAttribArray(index);
    gl.vertexAttribPointer(
      index,
      size,
      gl.FLOAT,
      false,
      stride,
      byteOffset + offset * 4,
    );
    gl.vertexAttribDivisor(index, 1);
  }
}

function bindSpriteAttribs(
  gl: WebGL2RenderingContext,
  buffer: WebGLBuffer,
  byteOffset: number,
): void {
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  const stride = SPRITE_STRIDE * 4;
  for (const [index, size, offset] of [
    [1, 2, 0],
    [2, 4, 2],
    [3, 4, 6],
    [4, 2, 10],
  ] as const) {
    gl.enableVertexAttribArray(index);
    gl.vertexAttribPointer(
      index,
      size,
      gl.FLOAT,
      false,
      stride,
      byteOffset + offset * 4,
    );
    gl.vertexAttribDivisor(index, 1);
  }
}

function grow(data: Float32Array, need: number): Float32Array {
  if (data.length >= need) return data;
  let size = data.length || 32;
  while (size < need) size *= 2;
  const next = new Float32Array(size);
  next.set(data);
  return next;
}

function write(data: Float32Array, offset: number, values: number[]): void {
  data.set(values, offset);
}

function packStampAtlas(
  index: Map<string, { u0: number; v0: number; u1: number; v1: number }>,
): HTMLCanvasElement {
  const keys = Object.keys(BUILDING_SHAPES) as SilhouetteKey[];
  const entries: { id: string; canvas: HTMLCanvasElement }[] = [];
  for (const key of keys) {
    for (let tier = 0; tier <= 2; tier += 1) {
      const growths = key === "hub" ? [0, 1, 2] : [0];
      for (const growth of growths) {
        entries.push({
          id: `${key}|${tier}|${growth}`,
          canvas: buildingStamp(key, tier, growth),
        });
      }
    }
  }
  const cols = 8;
  const cell = 256;
  const rows = Math.max(1, Math.ceil(entries.length / cols));
  const atlas = document.createElement("canvas");
  atlas.width = cols * cell;
  atlas.height = rows * cell;
  const ctx = atlas.getContext("2d");
  if (!ctx) return atlas;
  entries.forEach((entry, i) => {
    const col = i % cols;
    const row = Math.floor(i / cols);
    const pad = (cell - entry.canvas.width) / 2;
    ctx.drawImage(entry.canvas, col * cell + pad, row * cell + pad);
    index.set(entry.id, {
      u0: (col * cell) / atlas.width,
      v0: 1 - (row * cell) / atlas.height,
      u1: ((col + 1) * cell) / atlas.width,
      v1: 1 - ((row + 1) * cell) / atlas.height,
    });
  });
  return atlas;
}

function packIconAtlas(
  index: Map<string, { u0: number; v0: number; u1: number; v1: number }>,
): HTMLCanvasElement {
  const cell = 64;
  const cols = 4;
  const rows = 4;
  const atlas = document.createElement("canvas");
  atlas.width = cols * cell;
  atlas.height = rows * cell;
  const ctx = atlas.getContext("2d");
  if (!ctx) return atlas;
  ITEM_ICON_KEYS.forEach((icon, i) => {
    const col = i % cols;
    const row = Math.floor(i / cols);
    ctx.drawImage(bakeItemIcon(icon, cell), col * cell, row * cell);
    index.set(icon, {
      u0: (col * cell) / atlas.width,
      v0: 1 - (row * cell) / atlas.height,
      u1: ((col + 1) * cell) / atlas.width,
      v1: 1 - ((row + 1) * cell) / atlas.height,
    });
  });
  return atlas;
}
