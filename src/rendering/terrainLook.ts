import { TERRAIN_INFO, TERRAIN_ORDER } from "../core/terrain";
import type { Terrain, TerrainSnapshot } from "../core/types";
import { hexCorner, hexPath } from "./hexDraw";
import type { PixelPoint } from "@hexlife/embed/hex";

/**
 * Changing this constant regenerates every baked terrain tile. The tiles are presentation, so
 * the version is not a save, generator, or wire number.
 */
export const TERRAIN_TILE_VERSION = 1;

const TILE_SIZE = 128;

/**
 * Elevation order used only to decide which side of a band boundary wears the fringe. Cliff sits
 * above highland so a cliff hex becomes the landform edge rather than a neighbouring highland
 * drawing down onto it.
 */
export const BAND_RANK: Record<Terrain, number> = {
  deep_water: 0,
  shallow_water: 1,
  shore: 2,
  lowland: 3,
  hills: 4,
  highland: 5,
  cliff: 6,
};

export interface HexLook {
  /** One of the six hex rotations, so a baked tile does not stamp a grid. */
  rotation: number;
  /** In-band luminance nudge, in −1…1. */
  jitter: number;
  /** How many scattered detail marks this cell carries, 0–3. */
  marks: number;
  salt: number;
}

/**
 * Integer mix of a hex coordinate. Presentation only: the value must never become an input to
 * anything native, because a host hash in a checksum would make the picture part of the run.
 */
export function hexHash(q: number, r: number): number {
  let hash = (Math.imul(q, 374761393) + Math.imul(r, 668265263)) | 0;
  hash = Math.imul(hash ^ (hash >>> 13), 1274126177);
  return (hash ^ (hash >>> 16)) >>> 0;
}

export function hexLook(q: number, r: number): HexLook {
  const hash = hexHash(q, r);
  return {
    rotation: hash % 6,
    jitter: ((hash >>> 3) & 255) / 127.5 - 1,
    marks: (hash >>> 11) % 4,
    salt: hash,
  };
}

export function tileKey(q: number, r: number): string {
  return `${q},${r}`;
}

export function indexTerrain(
  tiles: readonly TerrainSnapshot[],
): Map<string, Terrain> {
  const map = new Map<string, Terrain>();
  for (const tile of tiles) map.set(tileKey(tile.q, tile.r), tile.terrain);
  return map;
}

/**
 * Band of a surveyed hex. Lowland is the default fill and is deliberately omitted from the
 * terrain group, so a surveyed hex with no entry is lowland — the same rule the inspector uses.
 */
export function surveyedBand(
  map: Map<string, Terrain>,
  q: number,
  r: number,
): Terrain {
  return map.get(tileKey(q, r)) ?? "lowland";
}

export function fringeToward(from: Terrain, toward: Terrain): boolean {
  return BAND_RANK[from] > BAND_RANK[toward];
}

export function remainingRatio(quantity: number, initial: number): number {
  if (initial <= 0) return quantity > 0 ? 1 : 0;
  return Math.min(1, Math.max(0, quantity / initial));
}

export interface DepletionLook {
  remaining: number;
  desaturate: number;
  scars: number;
}

/**
 * How worn a field cell looks. Flora uses the same numbers the other way: as `quantity` climbs
 * back toward `initial_quantity`, the scars fade and the colour returns.
 */
export function depletionLook(
  quantity: number,
  initial: number,
): DepletionLook {
  const remaining = remainingRatio(quantity, initial);
  const worn = 1 - remaining;
  return {
    remaining,
    desaturate: worn,
    scars: worn <= 0 ? 0 : worn >= 1 ? 4 : Math.ceil(worn * 4),
  };
}

/** Offscreen tiles baked once at startup. Not PNGs, and not a checksum input. */
export class TerrainTiles {
  private readonly canvases = new Map<Terrain, HTMLCanvasElement>();
  private readonly fields = new Map<Terrain, HTMLCanvasElement>();

  constructor() {
    for (const band of TERRAIN_ORDER) {
      this.canvases.set(band, bakeBand(band, true));
      this.fields.set(band, bakeBand(band, false));
    }
  }

  tile(band: Terrain): HTMLCanvasElement {
    const canvas = this.canvases.get(band);
    if (!canvas) throw new Error(`missing terrain tile for ${band}`);
    return canvas;
  }

  /** Unclipped noise, for filling a surveyed chunk that is implicit lowland. */
  field(band: Terrain): HTMLCanvasElement {
    const canvas = this.fields.get(band);
    if (!canvas) throw new Error(`missing terrain field for ${band}`);
    return canvas;
  }
}

export function drawTerrainCell(
  ctx: CanvasRenderingContext2D,
  center: PixelPoint,
  size: number,
  band: Terrain,
  look: HexLook,
  tiles: TerrainTiles,
  neighbors: Array<Terrain | undefined>,
): void {
  const info = TERRAIN_INFO[band];
  const radius = size * 1.02;
  ctx.save();
  ctx.translate(center.x, center.y);
  ctx.rotate((look.rotation * Math.PI) / 3);
  const tile = tiles.tile(band);
  const dim = radius * 2;
  ctx.drawImage(tile, -dim / 2, -dim / 2, dim, dim);
  ctx.restore();

  if (look.jitter !== 0) {
    hexPath(ctx, center, radius);
    ctx.fillStyle =
      look.jitter > 0
        ? `rgba(255,255,255,${look.jitter * 0.07})`
        : `rgba(0,0,0,${-look.jitter * 0.09})`;
    ctx.fill();
  }

  drawDetailMarks(ctx, center, size, band, look);

  for (let direction = 0; direction < 6; direction += 1) {
    const neighbor = neighbors[direction];
    if (neighbor === undefined) continue;
    if (fringeToward(band, neighbor))
      drawFringe(ctx, center, radius, direction, band, neighbor, look);
    else if (neighbor === "lowland" && fringeToward("lowland", band))
      drawFringe(ctx, center, radius, direction, "lowland", band, look);
  }

  hexPath(ctx, center, radius);
  ctx.strokeStyle = info.stroke;
  ctx.lineWidth = 1.1;
  ctx.stroke();
}

export function drawDepletion(
  ctx: CanvasRenderingContext2D,
  center: PixelPoint,
  size: number,
  look: HexLook,
  quantity: number,
  initial: number,
): void {
  const worn = depletionLook(quantity, initial);
  if (worn.desaturate <= 0) return;
  hexPath(ctx, center, size * 0.97);
  ctx.fillStyle = `rgba(36, 38, 34, ${worn.desaturate * 0.48})`;
  ctx.fill();
  ctx.save();
  hexPath(ctx, center, size * 0.97);
  ctx.clip();
  ctx.strokeStyle = `rgba(28, 22, 16, ${0.35 + worn.desaturate * 0.4})`;
  ctx.lineWidth = Math.max(1.2, size * 0.055);
  ctx.lineCap = "round";
  for (let scar = 0; scar < worn.scars; scar += 1) {
    const bit = (look.salt >>> (scar * 5)) & 31;
    const angle = ((bit / 31) * Math.PI * 2 + scar * 0.9) % (Math.PI * 2);
    const span = size * (0.22 + ((bit >> 2) % 5) * 0.05);
    const ox = Math.cos(angle + 1.2) * size * 0.18;
    const oy = Math.sin(angle + 1.2) * size * 0.18;
    ctx.beginPath();
    ctx.moveTo(
      center.x + ox - Math.cos(angle) * span,
      center.y + oy - Math.sin(angle) * span,
    );
    ctx.lineTo(
      center.x + ox + Math.cos(angle) * span,
      center.y + oy + Math.sin(angle) * span,
    );
    ctx.stroke();
  }
  ctx.restore();
}

function drawFringe(
  ctx: CanvasRenderingContext2D,
  center: PixelPoint,
  size: number,
  direction: number,
  from: Terrain,
  toward: Terrain,
  look: HexLook,
): void {
  const depth = from === "cliff" ? 0.4 : 0.26;
  const wobble = ((look.salt >>> (direction * 3)) & 7) / 7 - 0.5;
  const a = hexCorner(center, size, direction);
  const b = hexCorner(center, size, direction + 1);
  const mid = {
    x: (a.x + b.x) / 2 + (center.x - (a.x + b.x) / 2) * (depth + wobble * 0.08),
    y: (a.y + b.y) / 2 + (center.y - (a.y + b.y) / 2) * (depth + wobble * 0.08),
  };
  const insetA = {
    x: a.x + (center.x - a.x) * depth,
    y: a.y + (center.y - a.y) * depth,
  };
  const insetB = {
    x: b.x + (center.x - b.x) * depth,
    y: b.y + (center.y - b.y) * depth,
  };
  const lower = TERRAIN_INFO[toward];
  ctx.beginPath();
  ctx.moveTo(a.x, a.y);
  ctx.lineTo(b.x, b.y);
  ctx.lineTo(insetB.x, insetB.y);
  ctx.lineTo(mid.x, mid.y);
  ctx.lineTo(insetA.x, insetA.y);
  ctx.closePath();
  ctx.fillStyle = withAlpha(lower.stroke, from === "cliff" ? 0.42 : 0.34);
  ctx.fill();
  ctx.strokeStyle = withAlpha(TERRAIN_INFO[from].stroke, 0.85);
  ctx.lineWidth = Math.max(1.4, size * (from === "cliff" ? 0.07 : 0.045));
  ctx.beginPath();
  ctx.moveTo(a.x, a.y);
  ctx.lineTo(b.x, b.y);
  ctx.stroke();
}

function drawDetailMarks(
  ctx: CanvasRenderingContext2D,
  center: PixelPoint,
  size: number,
  band: Terrain,
  look: HexLook,
): void {
  if (look.marks === 0) return;
  if (band === "deep_water" || band === "shallow_water") return;
  ctx.save();
  hexPath(ctx, center, size * 0.92);
  ctx.clip();
  ctx.fillStyle = withAlpha(TERRAIN_INFO[band].stroke, 0.45);
  ctx.strokeStyle = withAlpha(TERRAIN_INFO[band].stroke, 0.4);
  ctx.lineWidth = Math.max(1, size * 0.035);
  ctx.lineCap = "round";
  for (let mark = 0; mark < look.marks; mark += 1) {
    const bit = (look.salt >>> (8 + mark * 6)) & 63;
    const angle = (bit / 63) * Math.PI * 2;
    const dist = size * (0.18 + ((bit >> 3) % 5) * 0.07);
    const x = center.x + Math.cos(angle) * dist;
    const y = center.y + Math.sin(angle) * dist;
    if (band === "shore") {
      ctx.beginPath();
      ctx.arc(x, y, Math.max(0.8, size * 0.04), 0, Math.PI * 2);
      ctx.fill();
    } else if (band === "cliff" || band === "highland") {
      ctx.beginPath();
      ctx.arc(x, y, Math.max(1, size * 0.055), 0, Math.PI * 2);
      ctx.fill();
    } else {
      ctx.beginPath();
      ctx.moveTo(x, y + size * 0.06);
      ctx.lineTo(x + size * 0.03, y - size * 0.07);
      ctx.stroke();
    }
  }
  ctx.restore();
}

export function drawWaterShimmer(
  ctx: CanvasRenderingContext2D,
  center: PixelPoint,
  size: number,
  look: HexLook,
  now: number,
  reducedMotion: boolean,
): void {
  const phase = reducedMotion
    ? look.salt * 0.001
    : now / 680 + look.salt * 0.0002;
  ctx.save();
  hexPath(ctx, center, size * 0.9);
  ctx.clip();
  ctx.strokeStyle = `rgba(190, 228, 255, ${0.14 + 0.1 * (0.5 + 0.5 * Math.sin(phase))})`;
  ctx.lineWidth = Math.max(1, size * 0.045);
  ctx.lineCap = "round";
  for (let ridge = 0; ridge < 2; ridge += 1) {
    const offset =
      (ridge - 0.4) * size * 0.22 + Math.sin(phase + ridge) * size * 0.06;
    ctx.beginPath();
    ctx.moveTo(center.x - size * 0.38, center.y + offset);
    ctx.quadraticCurveTo(
      center.x,
      center.y + offset - size * 0.08,
      center.x + size * 0.38,
      center.y + offset,
    );
    ctx.stroke();
  }
  ctx.restore();
}

function bakeBand(band: Terrain, clipHex: boolean): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = TILE_SIZE;
  canvas.height = TILE_SIZE;
  const ctx = canvas.getContext("2d");
  if (!ctx) return canvas;
  const center = { x: TILE_SIZE / 2, y: TILE_SIZE / 2 };
  const radius = TILE_SIZE / 2;
  if (clipHex) {
    hexPath(ctx, center, radius);
    ctx.clip();
  }
  const rgb = rgbOf(TERRAIN_INFO[band].fill);
  const seed =
    (TERRAIN_TILE_VERSION * 9176 + TERRAIN_ORDER.indexOf(band) * 7919) | 0;
  const image = ctx.createImageData(TILE_SIZE, TILE_SIZE);
  const data = image.data;
  for (let y = 0; y < TILE_SIZE; y += 1) {
    for (let x = 0; x < TILE_SIZE; x += 1) {
      const n =
        0.52 * valueNoise(x / 28, y / 28, seed) +
        0.32 * valueNoise(x / 14, y / 14, seed ^ 0x9e37) +
        0.16 * valueNoise(x / 7, y / 7, seed ^ 0x85eb);
      const dx = x - center.x;
      const dy = y - center.y;
      const edge = Math.max(0, Math.hypot(dx, dy) / radius - 0.58);
      let tone = (n - 0.5) * 36;
      if (n > 0.68) tone += 18;
      if (n < 0.32) tone -= 16;
      tone -= edge * 42;
      const i = (y * TILE_SIZE + x) * 4;
      data[i] = clampByte(rgb[0] + tone);
      data[i + 1] = clampByte(rgb[1] + tone);
      data[i + 2] = clampByte(rgb[2] + tone);
      data[i + 3] = 255;
    }
  }
  ctx.putImageData(image, 0, 0);
  return canvas;
}

function valueNoise(x: number, y: number, seed: number): number {
  const x0 = Math.floor(x);
  const y0 = Math.floor(y);
  const fx = fade(x - x0);
  const fy = fade(y - y0);
  const v00 = hash2(x0, y0, seed);
  const v10 = hash2(x0 + 1, y0, seed);
  const v01 = hash2(x0, y0 + 1, seed);
  const v11 = hash2(x0 + 1, y0 + 1, seed);
  return (
    v00 * (1 - fx) * (1 - fy) +
    v10 * fx * (1 - fy) +
    v01 * (1 - fx) * fy +
    v11 * fx * fy
  );
}

function hash2(x: number, y: number, seed: number): number {
  let hash = Math.imul(x, 374761393) ^ Math.imul(y, 668265263) ^ seed;
  hash = Math.imul(hash ^ (hash >>> 13), 1274126177);
  return ((hash ^ (hash >>> 16)) >>> 0) / 4294967296;
}

function fade(t: number): number {
  return t * t * (3 - 2 * t);
}

function rgbOf(color: string): [number, number, number] {
  const hex = color.replace("#", "").slice(0, 6);
  return [
    Number.parseInt(hex.slice(0, 2), 16),
    Number.parseInt(hex.slice(2, 4), 16),
    Number.parseInt(hex.slice(4, 6), 16),
  ];
}

function clampByte(value: number): number {
  return Math.max(0, Math.min(255, Math.round(value)));
}

function withAlpha(color: string, alpha: number): string {
  const [r, g, b] = rgbOf(color);
  return `rgba(${r},${g},${b},${alpha})`;
}
