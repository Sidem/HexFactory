import {
  axialNeighbor,
  axialToPixel,
  type PixelPoint,
} from "@hexlife/embed/hex";

import type {
  BuildingDefinition,
  BuildingKind,
  EntitySnapshot,
  PowerSource,
} from "../core/types";
import { hexPath } from "./hexDraw";

export type SilhouetteKey =
  | BuildingKind
  | "assembly"
  | "smelting"
  | "firing"
  | "cutting"
  | "crushing"
  | PowerSource;

export interface BuildingTrim {
  stroke: string;
  width: number;
}

/**
 * Look from the definition, not from a drawing per id. Composer-kind machines split on
 * `recipe_category`; generators split on `power_source`. A new tier must not need a new function.
 */
export function silhouetteOf(
  kind: BuildingKind,
  recipeCategory?: string | null,
  powerSource?: PowerSource,
): SilhouetteKey {
  if (kind === "composer") {
    if (recipeCategory === "smelting") return "smelting";
    if (recipeCategory === "firing") return "firing";
    if (recipeCategory === "cutting") return "cutting";
    if (recipeCategory === "crushing") return "crushing";
    return "assembly";
  }
  if (kind === "generator") return powerSource ?? "burner";
  return kind;
}

/**
 * Trim carries `tier`. A taller tier is brighter and heavier edging on the same silhouette, which
 * is what makes a tier a data row: the upgrade is legible on the map without a second drawing.
 */
export function trimOf(tier = 0): BuildingTrim {
  if (tier <= 0) return { stroke: "#dce7ef", width: 1.4 };
  if (tier === 1) return { stroke: "#e2c15a", width: 1.85 };
  return { stroke: "#8fd4ff", width: 2.2 };
}

export interface BuildingLookInput {
  building: EntitySnapshot;
  definition?: BuildingDefinition;
  center: PixelPoint;
  size: number;
  color: string;
  now: number;
  reducedMotion: boolean;
  /** The definition's tier. Absent is the base tier, which is what every v0.13 building was. */
  tier?: number;
}

export function drawBuildingLook(
  ctx: CanvasRenderingContext2D,
  input: BuildingLookInput,
): void {
  const { building, definition, center, size, color, now, reducedMotion } =
    input;
  const trim = trimOf(input.tier ?? 0);
  const key = silhouetteOf(
    building.kind,
    definition?.recipe_category,
    definition?.power_source,
  );
  const cycle = workCycle(building, now, reducedMotion);
  ctx.save();
  if (building.status === "no power" || building.status === "brownout")
    ctx.globalAlpha = 0.72;
  hexPath(ctx, center, size * 0.8);
  ctx.fillStyle = color;
  ctx.fill();
  ctx.strokeStyle = trim.stroke;
  ctx.lineWidth = trim.width;
  ctx.stroke();
  drawSilhouette(ctx, key, center, size, trim, cycle);
  ctx.restore();
}

export function cargoTravel(
  now: number,
  reducedMotion: boolean,
  buildingId: number,
): number {
  if (reducedMotion) return 0.72;
  return 0.16 + ((now / 820 + buildingId * 0.17) % 0.68);
}

export function workCycle(
  building: EntitySnapshot,
  now: number,
  reducedMotion: boolean,
): number {
  if (building.progress_total > 0 && building.progress > 0)
    return building.progress / building.progress_total;
  if (reducedMotion) return 0;
  if (
    building.status === "extracting" ||
    building.status === "generating" ||
    building.status === "pumping"
  )
    return (now / 700 + building.id * 0.13) % 1;
  return 0;
}

function drawSilhouette(
  ctx: CanvasRenderingContext2D,
  key: SilhouetteKey,
  center: PixelPoint,
  size: number,
  trim: BuildingTrim,
  cycle: number,
): void {
  const x = center.x;
  const y = center.y;
  ctx.strokeStyle = trim.stroke;
  ctx.fillStyle = trim.stroke;
  ctx.lineWidth = Math.max(1.4, size * 0.06);
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  switch (key) {
    case "smelting":
      ctx.fillStyle = `rgba(255, 148, 64, ${0.18 + cycle * 0.45})`;
      ctx.beginPath();
      ctx.arc(x, y + size * 0.06, size * 0.22, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = trim.stroke;
      ctx.strokeRect(
        x - size * 0.08,
        y - size * 0.42,
        size * 0.16,
        size * 0.28,
      );
      break;
    case "firing":
      ctx.beginPath();
      ctx.arc(x, y + size * 0.08, size * 0.28, Math.PI, 0);
      ctx.stroke();
      ctx.fillStyle = `rgba(255, 96, 48, ${0.12 + cycle * 0.4})`;
      ctx.beginPath();
      ctx.arc(x, y + size * 0.1, size * 0.14, 0, Math.PI * 2);
      ctx.fill();
      break;
    case "cutting": {
      const angle = -Math.PI / 2 + cycle * Math.PI * 2;
      ctx.beginPath();
      ctx.arc(x, y, size * 0.22, 0, Math.PI * 2);
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(x, y);
      ctx.lineTo(
        x + Math.cos(angle) * size * 0.26,
        y + Math.sin(angle) * size * 0.26,
      );
      ctx.stroke();
      break;
    }
    case "crushing": {
      const gap = 0.08 + Math.sin(cycle * Math.PI) * 0.07;
      ctx.beginPath();
      ctx.moveTo(x - size * 0.28, y - size * 0.18);
      ctx.lineTo(x - size * gap, y + size * 0.22);
      ctx.moveTo(x + size * 0.28, y - size * 0.18);
      ctx.lineTo(x + size * gap, y + size * 0.22);
      ctx.stroke();
      break;
    }
    case "assembly":
      ctx.strokeRect(x - size * 0.2, y - size * 0.2, size * 0.4, size * 0.4);
      if (cycle > 0) {
        const spin = cycle * Math.PI * 2;
        ctx.beginPath();
        ctx.arc(
          x + Math.cos(spin) * size * 0.12,
          y + Math.sin(spin) * size * 0.12,
          size * 0.05,
          0,
          Math.PI * 2,
        );
        ctx.fill();
      }
      break;
    case "extractor": {
      const pulse = 1 + cycle * 0.12;
      ctx.beginPath();
      ctx.arc(x, y, size * 0.16 * pulse, 0, Math.PI * 2);
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(x, y - size * 0.08);
      ctx.lineTo(x, y + size * 0.28 * pulse);
      ctx.stroke();
      break;
    }
    case "belt":
      break;
    case "container":
      ctx.strokeRect(
        x - size * 0.22,
        y - size * 0.18,
        size * 0.44,
        size * 0.36,
      );
      ctx.beginPath();
      ctx.moveTo(x - size * 0.22, y - size * 0.04);
      ctx.lineTo(x + size * 0.22, y - size * 0.04);
      ctx.stroke();
      break;
    case "consumer":
      ctx.beginPath();
      ctx.moveTo(x - size * 0.22, y - size * 0.16);
      ctx.lineTo(x + size * 0.22, y);
      ctx.lineTo(x - size * 0.22, y + size * 0.16);
      ctx.stroke();
      break;
    case "hub":
      hexPath(ctx, center, size * 0.42);
      ctx.stroke();
      break;
    case "pump":
      ctx.beginPath();
      ctx.arc(x, y - size * 0.04, size * 0.16, 0, Math.PI * 2);
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(x, y + size * 0.12);
      ctx.lineTo(x, y + size * 0.3);
      ctx.stroke();
      if (cycle > 0) {
        ctx.globalAlpha = 0.35 + cycle * 0.4;
        ctx.beginPath();
        ctx.arc(
          x,
          y + size * (0.18 + cycle * 0.08),
          size * 0.06,
          0,
          Math.PI * 2,
        );
        ctx.fill();
        ctx.globalAlpha = 1;
      }
      break;
    case "pole":
      ctx.beginPath();
      ctx.moveTo(x, y + size * 0.28);
      ctx.lineTo(x, y - size * 0.32);
      ctx.moveTo(x - size * 0.16, y - size * 0.2);
      ctx.lineTo(x + size * 0.16, y - size * 0.2);
      ctx.stroke();
      break;
    case "burner":
      ctx.strokeRect(
        x - size * 0.18,
        y - size * 0.08,
        size * 0.36,
        size * 0.28,
      );
      ctx.fillStyle = `rgba(255, 160, 48, ${0.15 + cycle * 0.4})`;
      ctx.fillRect(x - size * 0.1, y - size * 0.32, size * 0.2, size * 0.22);
      break;
    case "wind": {
      const spin = cycle * Math.PI * 2;
      ctx.beginPath();
      ctx.moveTo(x, y + size * 0.28);
      ctx.lineTo(x, y - size * 0.08);
      ctx.stroke();
      for (let blade = 0; blade < 3; blade += 1) {
        const angle = spin + (blade * Math.PI * 2) / 3;
        ctx.beginPath();
        ctx.moveTo(x, y - size * 0.08);
        ctx.lineTo(
          x + Math.cos(angle) * size * 0.28,
          y - size * 0.08 + Math.sin(angle) * size * 0.28,
        );
        ctx.stroke();
      }
      break;
    }
    case "hydro": {
      const spin = cycle * Math.PI * 2;
      ctx.beginPath();
      ctx.arc(x, y, size * 0.22, 0, Math.PI * 2);
      ctx.stroke();
      for (let vane = 0; vane < 4; vane += 1) {
        const angle = spin + (vane * Math.PI) / 2;
        ctx.beginPath();
        ctx.moveTo(x, y);
        ctx.lineTo(
          x + Math.cos(angle) * size * 0.22,
          y + Math.sin(angle) * size * 0.22,
        );
        ctx.stroke();
      }
      break;
    }
    case "turbine":
      ctx.beginPath();
      ctx.arc(x, y, size * 0.2, 0, Math.PI * 2);
      ctx.stroke();
      ctx.strokeRect(
        x - size * 0.08,
        y - size * 0.36,
        size * 0.16,
        size * 0.16,
      );
      if (cycle > 0) {
        ctx.fillStyle = `rgba(180, 220, 255, ${0.2 + cycle * 0.35})`;
        ctx.fill();
      }
      break;
    case "boiler":
      ctx.strokeRect(x - size * 0.24, y - size * 0.14, size * 0.48, size * 0.3);
      ctx.beginPath();
      ctx.arc(
        x - size * 0.08,
        y - size * 0.28 - cycle * size * 0.06,
        size * 0.06,
        0,
        Math.PI * 2,
      );
      ctx.arc(
        x + size * 0.1,
        y - size * 0.34 - cycle * size * 0.04,
        size * 0.045,
        0,
        Math.PI * 2,
      );
      ctx.stroke();
      break;
    case "generator":
      ctx.strokeRect(
        x - size * 0.18,
        y - size * 0.18,
        size * 0.36,
        size * 0.36,
      );
      break;
    default:
      break;
  }
}

/** The first orientation index off the six-edge table. Matches `NORTH` in the core. */
export const NORTH = 6;
/** The two-row period. There is no third vertical heading, so these are named rather than indexed. */
const DUE_NORTH = { q: 1, r: -2 };
const DUE_SOUTH = { q: -1, r: 2 };
/**
 * A two-row step spans `3 · size` of world distance against `√3 · size` for a unit step, so a
 * vertical heading drawn at the edge scale would reach √3 times as far. The tip is a heading
 * indicator, not a measurement, so it is shortened to read the same length as the other six.
 */
const VERTICAL_TIP_SCALE = 1 / Math.sqrt(3);

/**
 * Where a building's heading points, in pixels.
 *
 * The six edge headings are the package's own neighbours. The two vertical ones are not neighbours
 * at all, so they are named here rather than asked of `axialNeighbor`, which only knows six. Both
 * go through the same `axialToPixel`, which is what puts a riser's tip on the same screen column
 * as its own centre — the property the whole direction is for.
 */
export function facingTip(
  center: PixelPoint,
  size: number,
  orientation: number,
  reach = 0.39,
): PixelPoint {
  if (orientation >= NORTH) {
    const direction = orientation === NORTH ? DUE_NORTH : DUE_SOUTH;
    return axialToPixel(direction, size * reach * VERTICAL_TIP_SCALE, center);
  }
  return axialToPixel(
    axialNeighbor({ q: 0, r: 0 }, orientation),
    size * reach,
    center,
  );
}

/**
 * The far end of a riser's span: the centre of the hex two rows away that it actually reaches.
 * Drawn as the gantry, so the seam it crosses reads as a short bridge over the crack between two
 * hexes rather than as a tile that is half of something.
 */
export function spanEnd(
  center: PixelPoint,
  size: number,
  orientation: number,
): PixelPoint {
  return axialToPixel(
    orientation === NORTH ? DUE_NORTH : DUE_SOUTH,
    size,
    center,
  );
}
